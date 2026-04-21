# Chain Truth Hardening — Research Project

> **Goal**: Our DB must always reflect the truth of what is on chain. Design a resilient system that handles ambiguous miner responses, API staleness, timing gaps, and nonstandard output types without compounding errors.

> **Status**: Research phase. Do not implement until plan is complete and reviewed.

## What Triggered This

1. **TaskValidateUtxos bug (2026-04-19)**: `reconcile_for_derivation()` wrongly marked 72 valid outputs as externally-spent ($21.27 recovered). Root cause: WoC's confirmed-only UTXO endpoint misses unconfirmed outputs; absence was treated as "definitely spent" with no second verification.

2. **Deggen's Arcade/Teranode incident (2026-04-18)**: BSV Browser lost funds when a Teranode propagation endpoint returned false "input already spent" errors. Wallet marked sends as failed → allowed re-spend → real double-spends in local DB. shruggr: "treating service errors as rejections" is the upstream cause.

3. **Backup task HTTP 500**: 637 consecutive failures since 2026-04-18 17:45. On-chain backups not working.

## Core Principle

**Never treat the absence of data as proof of anything. Service errors ≠ consensus rejections. Bulk API results ≠ complete truth. The only authoritative check is querying a specific txid or output directly.**

---

## Current System State (2026-04-19 snapshot)

### Broadcast Chain (handlers.rs — `broadcast_transaction()`)
1. GorillaPool ARC — BEEF→EF, no auth. Falls through on ANNOUNCED_TO_NETWORK.
2. TAAL ARC — BEEF→EF, Bearer token auth. **Key expires May 15, 2026.**
3. GorillaPool mAPI — raw tx, no auth.
4. WhatsOnChain — raw tx, no auth.

### ARC Response Classification (arc_status.rs)
Built in miner response sprint (2026-04-03, commit 657ca8e):
- SEEN_ON_NETWORK → success
- ANNOUNCED_TO_NETWORK → fall through to next broadcaster
- SEEN_IN_ORPHAN_MEMPOOL → hard error (was incorrectly treated as OK before)
- MINED_IN_STALE_BLOCK → explicit handling
- QUEUED / RECEIVED → accepted
- HTTP 460-475 → specific error codes (frozen inputs, fee too low, tx too large)
- "missing inputs" → NOT treated as double-spend (safer default)
- Double-spend detection → only on explicit DOUBLE_SPEND status

### Monitor Tasks (monitor/mod.rs)
| Task | Interval | Status | Risk Level |
|------|----------|--------|------------|
| TaskCheckForProofs | 60s | ACTIVE | Medium — queries ARC/WoC for proof, handles various statuses |
| TaskSendWaiting | 120s | ACTIVE | High — re-broadcasts stuck txs, could create duplicates |
| TaskFailAbandoned | 300s | ACTIVE | Medium — fails stuck unsigned/unprocessed txs |
| TaskUnFail | 300s | ACTIVE | High — recovers failed txs by checking chain, re-marks inputs as spent |
| TaskReviewStatus | 60s | ACTIVE | Low — consistency checks, restores spendable flags |
| TaskSyncPending | 30s | ACTIVE (reconcile disabled) | Medium — discovers new UTXOs, reconcile was bug source |
| TaskValidateUtxos | 600s | **DISABLED** | Was the bug — false external-spend marking |
| TaskCheckPeerPay | 60s | ACTIVE | Low — MessageBox polling |
| TaskBackup | 10800s | ACTIVE but **FAILING (HTTP 500)** | High — 637 consecutive errors |
| TaskPurge | 3600s | ACTIVE | Low — cleanup old events |

### Known Bugs / Gaps
1. `reconcile_for_derivation()` — disabled but not fixed. Needs individual spent-check before marking.
2. Backup task — HTTP 500 crash. Cause unknown.
3. 75 restored outputs have NULL basket_id (should be 'default').
4. TAAL ARC key expires May 15 — no renewal plan.
5. PushDrop outputs invisible to address-based UTXO queries (by design).
6. Unconfirmed outputs missed by confirmed-only endpoints.

### Format Rules
- ARC miners → EF (Extended Format / BRC-30 hex)
- Overlays → BEEF V1 bytes
- SDK responses → Atomic BEEF (V1 inside)
- Never send BEEF V1/V2 to ARC (chokes on PushDrop scripts)

### Known ARC Quirks
- Returns wrong txid from BEEF (picks parent, not subject tx)
- SEEN_IN_ORPHAN_MEMPOOL = graveyard, not waiting room
- Stores tx data only 2 days
- ANNOUNCED_TO_NETWORK = ambiguous (SDK treats as success, we fall through)

---

## Research Questions

### 1. Should we adopt "optimistic retention"?

**The idea**: Instead of classifying every possible miner response and deciding immediately, treat ALL non-definitive responses as "uncertain" and verify against chain:

```
BROADCAST → got response → is it DEFINITELY mined? 
  YES → mark confirmed
  NO → keep as "unproven", poll for confirmation
    → after N minutes with no confirmation, THEN investigate
    → only mark failed if chain explicitly confirms double-spend
```

**Questions to answer**:
- What is the minimum reliable check? (`GET /tx/{txid}` returns 200 if mined, 404 if not?)
- How long should we wait before escalating from "unproven" to "investigating"?
- Does this simplify or complicate TaskCheckForProofs?
- Does this eliminate the need for complex ARC status classification?
- Can we replace TaskUnFail entirely (it already does this, but on a 6-hour delay)?

### 2. What about the reconciliation tasks?

**Current approach**: Fetch bulk UTXOs by address, mark missing ones as spent.
**Problem**: Bulk endpoint is unreliable (confirmed-only, PushDrop invisible, partial responses).

**Better approach?**: Instead of reconciling by address, verify each spendable output individually:
```
For each spendable output in DB:
  GET /tx/{txid}/out/{vout}/spent → 200 means spent, 404 means unspent
```

**Questions to answer**:
- Rate limiting: 37 spendable outputs × 1 request each = 37 API calls. At 0.3s each = 11 seconds. Acceptable?
- How often should we run this? Every 30 minutes? Every hour?
- Should we batch by age (check older outputs first, skip recently created)?
- Is this the same check that fixes #1 (backup) — backup outputs being wrongly spent?

### 3. Teranode / Arcade investigation

**What we know**:
- Arcade = broadcasting service that proxies to Teranode propagation endpoints
- Teranode = BSVA's next-gen node
- Deggen's incident: Arcade returned false "input already spent" → wallet DB corruption
- TAAL platform (platform.teranode.group) shutting down May 15

**Questions to answer**:
- Is Arcade a replacement for ARC, a wrapper around it, or something separate?
- Are Teranode propagation endpoints publicly available?
- Do they accept EF format (same as ARC)?
- Should we add Arcade as a broadcaster in our fallback chain?
- Does broadcasting to multiple services cause problems? (Same tx seen by multiple miners via different paths)
- What's the canonical way to handle disagreements between services?

### 4. Does multi-broadcaster cause problems?

**Current behavior**: We try GorillaPool ARC → TAAL ARC → mAPI → WoC. First success wins.

**Concern**: If broadcaster 1 actually accepted and propagated (but returned ambiguous response), and we then send to broadcaster 2, does that cause issues?

**Questions**:
- Is a tx being "seen twice" by the network harmful? (Probably not — idempotent by txid)
- Could it create conflicting status reports? (Broadcaster 1 says "accepted", broadcaster 2 says "already seen")
- What does "ANNOUNCED_TO_NETWORK" actually mean from GorillaPool? Already in mempool? Partially propagated?

### 5. The backup task HTTP 500

**Context**: Last successful backup was 2026-04-18 07:36:56. 637 errors since 17:45. TODO token transactions happened in between.

**Questions**:
- What changed between 07:36 (success) and 17:45 (first failure)?
- Is the 500 from the wallet's own backup endpoint, or from an external service?
- Does the backup handler have dependencies on outputs/baskets that were disrupted?
- Now that we've restored 72 outputs, will the backup work again?
- Is the backup handler trying to spend the previous backup's outputs (which were wrongly marked non-spendable)?

---

## Proposed Architecture Direction

### The "Verify, Don't Infer" Model

Instead of complex response classification:

**Broadcast**: Send to multiple broadcasters. Don't care about individual responses beyond "did it return 200". Mark tx as "unproven" regardless.

**Verification (replaces TaskValidateUtxos + simplifies TaskCheckForProofs)**:
- Poll `GET /tx/{txid}` every 60s for unproven txs
- If confirmed (has blockHeight/confirmations) → mark completed, store proof
- If still in mempool after 30 min → try re-broadcasting
- If not found after 6 hours → check if inputs are spent by OTHER tx → if yes, mark failed + restore inputs; if no, re-broadcast

**Output validation (replaces reconcile_for_derivation)**:
- For each spendable output, check `GET /tx/{txid}/out/{vout}/spent`
- Only mark non-spendable if this endpoint returns 200 with a spending txid
- Record the spending txid in `spent_by`
- Never mark non-spendable based on absence from bulk endpoint

### Benefits
- Eliminates entire classes of bugs (false external-spend, false failure)
- No dependency on ARC status semantics (which change and are ambiguous)
- Works with any broadcaster (ARC, Arcade, Teranode, Bitails, WoC)
- Self-healing: if an output was wrongly marked, next validation cycle fixes it
- Simpler code: fewer special cases, fewer response parsers

### Costs
- More API calls (individual checks vs bulk)
- Slightly higher latency for confirmation detection
- Need to handle WoC rate limiting

---

---

## BSV SDK / wallet-toolbox Comparison (researched 2026-04-19)

### How They Handle Proof Acquisition (our TaskCheckForProofs)

**Event-driven, not polling.** They use Chaintracks block header subscriptions + ARC SSE push notifications. When a new block arrives, TaskCheckForProofs.checkNow is set. The 2-hour polling interval is a fallback, not the primary mechanism.

They also have `TaskArcSSE` — when ARC sends a `MINED` SSE event, the toolbox fetches the proof immediately. No polling required.

**Our gap**: We poll on a fixed 60s interval. We should investigate Chaintracks/SSE for event-driven proof acquisition, but polling is acceptable for now.

### How They Handle False Failures (our TaskUnFail)

**Two-stage recovery:**
1. `TaskReviewDoubleSpends` (every 12 min) — automatically checks `doubleSpend` status txs. Calls `getStatusForTxids()`. If the txid IS known to network → promotes to `unfail` status.
2. `TaskUnFail` (every 10 min) — processes `unfail` status txs. Fetches merkle path, if found → recovers to `unproven`/`unmined`.

**Key: nobody manually sets `unfail`.** The double-spend reviewer does it automatically. Our TaskUnFail combines both stages.

### How They Handle Broadcast Recovery (our TaskSendWaiting)

**8-second trigger, 5-min re-check for `sending`.** Much more aggressive than our 120s. Supports batch broadcasting (multiple txs in one BEEF). Has adaptive timing — when many txs pending, drops to 1s interval.

### How They Handle UTXO Validation (our TaskValidateUtxos)

**THEY DON'T. TaskReviewUtxos is DISABLED by default.** `trigger()` returns `{ run: false }` always. Manual-only.

They trust their status tracking instead and rely on:
- Correct classification at broadcast time
- `TaskReviewDoubleSpends` for false double-spend reports
- `TaskReviewProvenTxs` for reorg detection (audits merkle roots against chain)

**This validates our decision to disable TaskValidateUtxos.** The reference implementation doesn't do periodic UTXO reconciliation at all.

### How They Handle Broadcast Responses

**Double-spend verification before marking failed.** When ARC reports double-spend:
1. Call `getStatusForTxids([txid])` up to 3 times (1s waits between)
2. If txid IS known to network → upgrade to success (false alarm)
3. If truly unknown → examine inputs via `getScriptHashHistory()` to find competing txids
4. Only then mark as real double-spend

**This is the shruggr fix** — never trust a single service's double-spend report.

**UntilSuccess pattern:**
```
For each provider in order:
  Post BEEF → if success, STOP
  If service error → move provider to end of queue, try next
  If double-spend → collect it, try next (don't stop!)
After all providers:
  Aggregate: any success? → success
  All double-spend? → verify independently before marking failed
  All service errors? → keep as 'sending', retry next cycle
```

A single success from ANY provider overrides service errors from all others.

### How They Handle NoSend

**`TaskCheckNoSends` runs once per day.** Uses the same proof-checking logic but:
- Never increments attempt count (can sit at `nosend` indefinitely)
- Only checks for proof (did someone else broadcast it?)
- Never forcibly failed

**Our correction**: User noted we should base decisions on "did WE broadcast it" not on the noSend flag. The toolbox's approach aligns — nosend is just a status that means "we didn't broadcast, but someone else might have."

### Transaction Status Comparison

| Ours | Theirs | Notes |
|------|--------|-------|
| unprocessed | unprocessed | Same |
| unsigned | unsigned | Same |
| sending | sending | Same |
| unproven | unproven | Same |
| completed | completed | Same |
| failed | failed | Same |
| nosend | nosend | Same |
| — | nonfinal | For nLockTime txs (we don't support) |
| — | unfail | Recovery trigger state (we handle in TaskUnFail directly) |
| — | doubleSpend | Separate from failed (we combine them) |

They also have a separate `ProvenTxReq` entity with 13 statuses tracking the proof lifecycle independently from the transaction. We combine both on a single record.

### Key Architectural Differences

1. **Event-driven vs polling** — they subscribe to block events, we poll
2. **Two-entity model** — ProvenTxReq (broadcast/proof) vs Transaction (user-facing). We use one table.
3. **Independent double-spend verification** — they verify ARC's claims. We take them at face value.
4. **No automated UTXO validation** — they trust status tracking. We had aggressive reconciliation (now disabled).
5. **Service degradation tracking** — failing providers moved to end of queue. We have fixed order.

---

## Teranode / Arcade Research (2026-04-19)

### What is Arcade?

**Arcade is NOT ARC.** It's a new Go-based service purpose-built for Teranode:
- GitHub: [bsv-blockchain/arcade](https://github.com/bsv-blockchain/arcade)
- Uses libp2p gossip (P2P-first) instead of RPC
- Provides ARC-compatible REST API (drop-in replacement)
- Not yet a publicly hosted service — you run it yourself
- Latest: v0.4.6, Go 1.26+, SQLite storage

### Teranode Propagation Endpoints

Not publicly available as hosted service yet. Arcade is the intended client-facing interface. You configure Arcade with `broadcast_urls` pointing to Teranode or existing ARC endpoints.

### CVE-2026-40069

The BSV SDK had the exact Deggen bug as an official CVE. ARC broadcaster treated INVALID/MALFORMED/ORPHAN/MINED_IN_STALE_BLOCK as success. Fixed in bsv-sdk v0.8.2+. **Our code already handles these correctly** in `arc_status.rs`.

### Multi-Broadcasting

Safe — transactions are idempotent by txid. The risk is interpretation, not transmission. The toolbox's `UntilSuccess` pattern proves this: it tries multiple providers and aggregates results.

### TAAL Key

Expires May 15, 2026 (26 days). No public replacement announced. GorillaPool ARC remains available. Arcade could replace it once hosted publicly.

### Format

Arcade claims ARC-compatible API. Example shows `POST /tx` with `text/plain` body (raw hex), vs ARC's `POST /v1/tx` with JSON. May need testing for EF format support.

---

## Revised Architecture Direction

### What We Should Adopt from the Toolbox

1. **Independent double-spend verification** (HIGH priority)
   - Before marking any tx as double-spend/failed, verify via `getStatusForTxids()` equivalent
   - Check WoC `GET /tx/{txid}` — if it returns 200, the tx is known to network → not a real failure
   - Only mark as double-spend if we can find a competing tx spending the same inputs

2. **No automated UTXO reconciliation** (DONE — already disabled)
   - The reference implementation validates this decision
   - If we re-enable, use individual `/tx/{txid}/out/{vout}/spent` checks, not bulk address queries

3. **Service error → retry, not fail** (MEDIUM priority)
   - Service errors should keep tx in `sending` status, retry next cycle
   - Only consensus rejections (verified double-spend) should mark as failed
   - Audit our `broadcast_transaction()` and `is_permanent_error()` against this principle

4. **Separate "doubleSpend" from "failed"** (CONSIDER)
   - They have `doubleSpend` as its own status, separate from `failed`
   - Allows TaskReviewDoubleSpends to find and verify them specifically
   - Would require schema change

5. **Service degradation tracking** (LOW priority)
   - Move failing providers to end of broadcast queue
   - Auto-recover when they start working again

### What We Should NOT Adopt

1. **Two-entity model (ProvenTxReq + Transaction)** — too much rework for marginal benefit
2. **Event-driven proofs** — nice to have but polling works fine for now
3. **Batch broadcasting** — we don't have the use case yet

### Decision: "Did we broadcast it?"

Per user's correction: base decisions on whether WE broadcast, not on the noSend flag:
- **We broadcast it** → we can verify mempool within seconds, we own the lifecycle, we poll for proof
- **We didn't broadcast it** → check once/day for proof (did the overlay broadcast it?), never forcibly fail

---

## Next Steps

1. **Investigate backup 500 error** — may be resolved by restored outputs, need to verify
2. **Audit broadcast_transaction()** — verify service-error-vs-rejection distinction matches toolbox pattern
3. **Add double-spend verification** — before marking failed, check WoC for the txid
4. **Design reconcile replacement** — individual `/tx/{txid}/out/{vout}/spent` checks
5. **TAAL key renewal** — 26 days until expiry
6. **Test Arcade compatibility** — once publicly hosted, test as additional broadcaster
