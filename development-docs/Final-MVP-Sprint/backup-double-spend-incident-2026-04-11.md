# Backup Double-Spend Cascade Incident — 2026-04-11

**Status:** DB FIXED 2026-04-11 ~19:47 UTC via SQL surgery (option 1a). The three underlying bugs (A, B, C) are still UNFIXED — must not trigger any backup until at least Bug A is patched. Currently in "research and understand the bugs" phase.

**Wallet:** Treasury (the real wallet that receives Hodos service fees). On-chain balance is correct; this is purely a local DB inconsistency.

**Branch:** `post-beta3-cleanup`. The P0 #1 strip code (commit pending — see plan file `~/.claude/plans/sequential-tumbling-spindle.md`) is in the working tree but UNCOMMITTED.

---

## Executive summary

While testing the P0 #1 spent-output strip on the treasury wallet, a back-to-back-broadcast + wallet-restart pattern triggered three separate, pre-existing bugs in the backup creation and monitor pipeline:

1. **Bug A — Orphan-sweep relies on a stale WoC index.** The backup creator queries WoC's `/address/{addr}/unspent/all` endpoint to find orphaned markers from prior backup cycles. WoC's address-unspent index lags the chain by ~30s–5min after a fresh broadcast. During that lag window, an output that was already validly spent on chain still appears unspent in the address index, and the orphan-sweep will try to consume it as an input — causing a guaranteed double-spend.

2. **Bug B — Broadcast handler trusts ARC's `status:200` response without verifying the returned txid.** When a broadcast tx conflicts with one already in mempool, ARC returns `status:200, txStatus:SEEN_ON_NETWORK, txid:<the existing tx>`. Our handler interprets `status:200` as success and stores the *attempted* txid as broadcasted, even though ARC was actually telling us "I'm rejecting your tx because I already have one with these inputs, here's its txid." This is the documented ARC txid mismatch bug (`feedback_arc_txid_mismatch.md`) but the broadcast path doesn't apply the workaround.

3. **Bug C — TaskCheckForProofs received DOUBLE_SPEND_ATTEMPTED for the winning tx and cascaded.** TaskCheckForProofs queried ARC for both `260f65fd` (legitimate loser) and `61c03ecc` (legitimate winner — got mined later) at 18:04:30. ARC returned DOUBLE_SPEND_ATTEMPTED for BOTH. The task treats DOUBLE_SPEND_ATTEMPTED as terminal and immediately marks the tx failed (delete outputs, restore inputs). For `61c03ecc` this was wrong — it was the actually-winning tx. **WHY ARC returned DOUBLE_SPEND_ATTEMPTED for `61c03ecc` is currently unknown.** Possibilities (to be researched, NOT confirmed): (a) ARC marks both txs in a conflict as DOUBLE_SPEND_ATTEMPTED until a block resolves it; (b) the same ARC txid mismatch bug from `feedback_arc_txid_mismatch.md` corrupted the response association so we got `260f65fd`'s status when querying for `61c03ecc`; (c) ARC's `DOUBLE_SPEND_ATTEMPTED` has a different semantic than I assumed. **Need to research bsv-rust-sdk + wallet-toolbox-rs handling, ARC API docs, and any preserved raw ARC responses before proposing a fix for Bug C.**

**Combined impact on this incident:** the legitimate prior backup `61c03ecc...` (which actually got mined and is the current valid backup on chain) had its local DB record gutted. Its three outputs were all deleted. Vout 2 (change at a normal receive address) was later rediscovered by `TaskSyncPending` from address sync. Vouts 0 (PushDrop) and 1 (marker, special wallet-backup address not in normal sync) are still missing from local DB. Meanwhile `1a24b540`'s outputs (which `61c03ecc` validly consumed on chain) were incorrectly *restored* to spendable=1 in the local DB by the cascade rollback.

**Funds are safe.** No coins lost. The chain is correct. The local DB is wrong.

---

## Detailed timeline

All times UTC, 2026-04-11.

| Time | Event | Local DB state | Chain state |
|---|---|---|---|
| 17:39:40 | Old wallet broadcasts `1a24b540...` (1st manual backup, 90d threshold so 0 records dropped) | new tx created, outputs 0/1/2 spendable | propagating |
| 17:50ish | WoC indexes `1a24b540...` | (no change) | `1a24b540:0,1,2` unspent at backup address |
| 18:02:49 | Old wallet auto-backup creates `61c03ecc...` consuming `1a24b540:0,1,2`. Inserted in DB, broadcasted via ARC. | tx 375 inserted; `1a24b540:0,1,2` marked spent_by=375; `61c03ecc:0,1,2` inserted as spendable | `61c03ecc` in mempool, `1a24b540:0,1,2` consumed |
| ~18:03 | I send `/shutdown` to old wallet | (state intact) | `61c03ecc` still propagating to WoC's address index |
| 18:03:55 | New wallet starts (rebuilt with 7d strip + pending guard) | (DB unchanged from old wallet's last write) | unchanged |
| 18:04:07 | I trigger backup. New wallet calls `wallet_backup_onchain`. Step 5c queries WoC for unspent UTXOs at backup-marker address. **WoC returns BOTH `61c03ecc:1` AND `1a24b540:1` as unspent** (address index is ~80 seconds stale on the spend by `61c03ecc`). | being read | `1a24b540:1` is actually spent by `61c03ecc` |
| 18:04:08 | Step 5c finds `61c03ecc` (DB primary) in WoC list → "DB backup confirmed on-chain — using it". Step 5d filters orphans (anything not equal to primary) → finds `1a24b540:1` → fetches its tx data → adds to `extra_markers` to be swept as an input. | unchanged | unchanged |
| 18:04:09 | New wallet builds `260f65fd...` with inputs: `61c03ecc:0`, `61c03ecc:1`, `1a24b540:1`. Calls strip code → drops 64 records (the strip works correctly, this is the verification we wanted). Calls ARC. **ARC returns `status:200, txStatus:SEEN_ON_NETWORK, txid:61c03ecc` — meaning "I'm not accepting your tx; here's the existing one with these inputs."** Broadcast handler reads `status:200` and treats it as success. Logs `🎉 ARC broadcast successful: ARC accepted: 61c03eccebd8c3d2... (SEEN_ON_NETWORK)` followed by `✅ On-chain backup broadcast successful: 260f65fdd4121bdb...`. | tx 376 inserted; `260f65fd:0,1,2` outputs created; `61c03ecc:0,1` and `1a24b540:1` marked spent_by=376 | unchanged — `260f65fd` was rejected by ARC, only `61c03ecc` is in mempool |
| 18:04:30 | TaskCheckForProofs runs. Queries ARC for both `260f65fd` and `61c03ecc`. ARC returns `DOUBLE_SPEND_ATTEMPTED` for BOTH (BSV first-seen conflict, both flagged until block resolves). Task marks BOTH as failed. mark_failed for `260f65fd`: deletes its 3 outputs, restores its 3 inputs → restores `61c03ecc:0,1` AND `1a24b540:1` to spendable. mark_failed for `61c03ecc`: deletes its 3 outputs, restores its 3 inputs → restores `1a24b540:0,1,2` to spendable. | DB now has: `1a24b540:0,1,2 spendable=1`, `61c03ecc:0,1,2 deleted`, `260f65fd:0,1,2 deleted` | unchanged — `61c03ecc` is still the actual winning tx in mempool |
| 18:04:54 | TaskSyncPending fetches new UTXOs at receive addresses. Discovers `61c03ecc:2` (the change output, at a normal P2PKH receive address) and inserts as spendable. | DB has `61c03ecc:2` back as spendable, derivation_prefix='2-receive address' (NOT '1-wallet-backup' — it's a normal change output from sync's perspective). `61c03ecc:0` (PushDrop) and `61c03ecc:1` (marker) are STILL missing because they're at non-standard / non-sync addresses. | unchanged |
| 18:11:17 | `61c03ecc` is mined in block `0000000000000000105f7f7521a654c63f73f45570d05539e9449e9a9fae111f` | unchanged | `61c03ecc` confirmed |
| 18:12:40 | TaskValidateUtxos runs. Marks `1a24b540:2` as `external-spend` (TaskValidateUtxos correctly notices it's been consumed on chain by `61c03ecc`). Marks `61c03ecc:2` as confirmed. | `1a24b540:2 spendable=0 spending_description='external-spend'`; `1a24b540:0,1` STILL spendable=1 (validate task didn't fix them — possibly because they're at the special wallet-backup PushDrop / marker address which validate doesn't watch?) | unchanged |
| 18:14:56 | TaskUnFail re-checks `61c03ecc` on ARC. Now mined. Recovers tx → status='completed'. **Does NOT re-create the deleted change outputs (per documented Ghost Transaction Safety rule).** | tx 375 status back to 'completed', but outputs 0 and 1 still missing | unchanged |

## Current local DB state (incorrect)

| Output | Reality on chain | Local DB says |
|---|---|---|
| `1a24b540:0` (PushDrop) | spent by `61c03ecc` | **spendable=1, spent_by=NULL** ❌ |
| `1a24b540:1` (marker) | spent by `61c03ecc` | **spendable=1, spent_by=NULL** ❌ |
| `1a24b540:2` (change) | spent by `61c03ecc` | spendable=0, spending_description='external-spend' ✓ (set by TaskValidateUtxos at 18:12:40) |
| `61c03ecc:0` (PushDrop) | unspent (current valid backup PushDrop) | **MISSING** ❌ (deleted by Bug C cascade, never re-inserted) |
| `61c03ecc:1` (marker) | unspent (current valid backup marker) | **MISSING** ❌ (deleted by Bug C cascade, never re-inserted) |
| `61c03ecc:2` (change) | unspent (~15.26M sats change) | spendable=1, derivation_prefix='2-receive address' ✓ (re-inserted by TaskSyncPending at 18:04:54) |
| `260f65fd:*` (failed tx) | never on chain | EMPTY ✓ |

## Risk if we trigger another backup right now

1. The next backup attempt will call `wallet_backup_onchain`. Step 5c queries WoC's address-unspent endpoint. By now WoC should have caught up — `61c03ecc:1` is the only unspent marker (correct).
2. Step 5c finds the DB's primary in WoC: but the DB's primary is `61c03ecc` (per the transactions table). DB primary is found in WoC → "DB backup confirmed on-chain." But DB has `61c03ecc:0` and `61c03ecc:1` MISSING from outputs. The handler relies on `previous_pushdrop` and `previous_marker` being populated from the DB query at the top of the function. **If those queries return NULL (because the rows are missing), the new backup may fall through to "no previous backup" mode and start a fresh chain — leaving the existing `61c03ecc` PushDrop locked on chain forever (we lose its 1000+546 = 1546 sats per backup cycle, which is small but real).**
3. Worse: the funding-UTXO selection (Step 6, lines 11406+) calls `get_spendable_by_user` which would happily pick `1a24b540:0` (1000 sats) or `1a24b540:1` (546 sats) since they appear spendable in DB. The new tx would try to spend them → guaranteed double-spend → another cascade → another loop of damage.

**Conclusion: do not trigger any backup until the DB is fixed.**

---

## The three bugs in detail

### Bug A — Orphan-sweep relies on stale WoC address index

**Code:** `rust-wallet/src/handlers.rs:11291–11403`

**Design intent:** Find leftover backup markers from interrupted/aborted backup cycles and sweep them as inputs to the new backup, recovering their sats. The marker address is at a deterministic BRC-42 self-derivation, so we can query WoC for everything unspent at it. This handles a real edge case (interrupted broadcasts, version migrations).

**Why it queries WoC and not local DB:** the chain IS the source of truth. The whole purpose of orphan recovery is to recover state the local DB might have lost (stale wallets, fresh recoveries, code bugs in older versions). Local DB cannot be trusted to know about every marker that's ever been created at the address. Asking WoC is correct in principle.

**Why it failed:** WoC's `/address/{addr}/unspent/all` endpoint reads from an address-index database that is updated *behind* WoC's mempool. There is a propagation delay (typically 30 seconds to a few minutes) between a tx being accepted into mempool and the spends-of-its-inputs being reflected in the address-unspent index. During that window, the SAME WoC response can contain BOTH the new marker AND the about-to-be-superseded old marker.

The orphan-sweep doesn't validate this case. It treats every entry in the response as authoritative "unspent right now," and anything that isn't the DB's primary marker is assumed to be a real orphan. So during the propagation window, the *immediately previous* backup's marker shows up as a "false orphan" and gets added as an input to the new tx — guaranteeing a double-spend.

**The user's framing applies cleanly:** the chain is the source of truth. WoC is normally a window into the chain. But WoC's *address-unspent index* has a documented staleness — and the orphan-sweep's algorithm assumes that index is fresh.

### Bug A also affects RECOVERY (added 2026-04-11 ~21:30 UTC)

User insight: the same WoC address-index staleness affects the recovery flow, not just the orphan-sweep on backup creation. The propagation window is BIDIRECTIONAL.

**Sweep direction (the one we hit today):** during the propagation window of the *previous* backup, WoC's address-unspent index still shows the previous-previous marker as unspent. The orphan-sweep treats it as an orphan and tries to consume it as input → double-spend with the propagating previous backup.

**Recovery direction (newly identified):** during the propagation window of a *new* backup, a fresh `wallet_recover_onchain` call queries WoC for the unspent marker at the backup address. WoC returns the OLD (already-superseded) marker because the address index hasn't caught up. Recovery decrypts the OLD backup payload. The recovered wallet has stale tx history, stale derivation indices, and (worst case) missing PushDrop tokens or BRC-42-counterparty outputs that were created in the gap. **Funds aren't lost** (the actual UTXOs are on chain), but the wallet's local view is wrong and would re-derive over a stale base.

Both halves are the same root cause. The fix has to handle both.

**Proposed fix (covers both directions):** before trusting any candidate marker returned by WoC's address-unspent index, do an INDEPENDENT verification of its spend state via a different, more authoritative source.

- If the candidate's outpoint is reported SPENT by the independent check → there's a newer (or competing) backup that hasn't propagated to the address index yet
- In SWEEP direction: don't add the candidate as an input (it's not a real orphan)
- In RECOVERY direction: tell the user "found a backup but it's been superseded by a newer one that hasn't fully propagated; retrying..." with auto-retry every N minutes for up to M minutes (e.g., every 2 min for up to 30 min)
- After the retry budget is exhausted: surface a clear error to the user with manual options (try again later, or accept the older backup as a starting point)

The independent check needs an endpoint that doesn't read from the address-unspent index. Candidates to research:
1. WoC `/v1/bsv/main/tx/{txid}/{vout}` (outpoint detail) — does this report spent state?
2. WoC `/v1/bsv/main/script/{scriptHash}/history` — does this include mempool spends?
3. ARC has a `/v1/tx/{txid}` status endpoint that's mempool-aware — but ARC is broadcast/proof-only, not query-by-outpoint
4. A mempool-aware BSV indexer (Taal? GorillaPool? BSV Overlay?) might have an outpoint-spent endpoint

Need to research which API actually surfaces fresh spent-state information. The fix is conceptually clean but depends on finding the right endpoint.

### Fix options for Bug A

| Option | What it does | Tradeoffs |
|---|---|---|
| **A1 — Use a more authoritative WoC endpoint per outpoint** | After WoC returns the address-unspent list, for each "orphan candidate," call `/v1/bsv/main/tx/{txid}/{vout}/spent` (or equivalent) to verify the specific outpoint. Drop the candidate if any source says it's spent. | Adds N HTTP roundtrips per backup. Costs latency. May still hit staleness on the per-outpoint endpoint depending on which index it reads. |
| **A2 — Cooldown / "settling time" for previous backups** | Don't sweep any marker that was created within the last N minutes (configurable, e.g., 5 minutes). If we just made a backup minutes ago, don't second-guess the chain index. | Simple. Eliminates the immediate-previous case (which is the vast majority of incidents). Still allows true orphans (which are rare and never time-sensitive) to be swept. |
| **A3 — Cross-reference orphan candidates with local DB `outputs.spent_by`** | Before adding any candidate to `extra_markers`, check the local outputs table. If the candidate is marked `spent_by IS NOT NULL` in our DB (meaning we already know we spent it via our own broadcast), skip it. | The local DB DOES know about its own spends, because we wrote them when we broadcast. This is not "trust the DB instead of the chain" — it's "we already learned about a spend by being the cause of it, so don't re-attempt it." Cheap, no extra HTTP. |
| **A4 — Combine A2 + A3** | Both. Belt-and-suspenders. | Tiny code; very robust. |

**Recommendation: A4.** A3 alone is the cheapest fix and would have prevented this incident entirely. A2 adds defense-in-depth for the case where the DB doesn't know about a tx (e.g., crashed mid-broadcast). The combination is safe even if either heuristic has a hole.

### Bug B — Broadcast handler trusts ARC's status 200 without verifying txid

**Code:** somewhere in the broadcast path called by `wallet_backup_onchain` and other tx submitters. Approximately the same code that handles `📡 ARC response: HTTP 200 - {...}` log output. Need to grep for the exact location.

**The ARC behavior:** when ARC receives a tx that conflicts with one already in mempool, it does NOT return an error status. It returns:
```json
{"status":200,"title":"OK","txStatus":"SEEN_ON_NETWORK","txid":"<the existing competing tx's txid>","competingTxs":null}
```
ARC's intent appears to be "you sent me a tx involving these inputs, and I have one — here's the txid I have." It's not a successful acceptance of the new tx; it's a polite collision report that looks like a success.

**The wallet's behavior:** the broadcast handler reads `status: 200` and `title: "OK"` and concludes "broadcast successful." It logs the success and stores the *attempted* txid (`260f65fd`) as broadcasted. It does not compare the response's `txid` field against the txid we computed.

This is the same root cause as the existing `feedback_arc_txid_mismatch.md` — but in that case the workaround is "trust our own computed txid." The deeper rule should be: **when ARC returns a different txid than the one we sent, that's a collision signal and we should treat the broadcast as a soft-failure (don't insert the tx into local DB as `unproven`; treat it as a competing-tx situation and surface the conflict to the caller).**

### Fix options for Bug B

1. **B1 — Strict txid match check.** In the broadcast handler, after parsing ARC's response, compare `response.txid` against the txid we computed for the BEEF top-tx. If they differ, do NOT mark the broadcast as successful. Either:
   - (a) Treat it as an error and propagate up to the caller, OR
   - (b) Treat it as a "competing tx detected" condition and let the caller decide (e.g., backup creator could rebuild without the conflicting input)
2. **B2 — Status text + txStatus check.** SEEN_ON_NETWORK with no `competingTxs` field is the ambiguous response; check `txStatus` and any other ARC fields that could indicate a collision.

**Recommendation: B1(a)**. Simplest. The caller can always retry. Pretending success when ARC didn't accept our tx leads to bad DB state every time.

### Bug C — TaskCheckForProofs cascade kills the winning tx during the conflict window

**Code:** `rust-wallet/src/monitor/task_check_for_proofs.rs:174–177`
```rust
"DOUBLE_SPEND_ATTEMPTED" | "REJECTED" => {
    warn!("   ⚠️ {} status: {} — marking failed", txid, status);
    mark_failed(state, txid);
}
```

**Why DOUBLE_SPEND_ATTEMPTED is treated as terminal:** in normal operation, if our tx is reported double-spend, it means someone else's tx with the same inputs got there first and we lost the race. Recovery is impossible (we can't un-spend their tx). Marking ours failed and restoring inputs is correct *in that case*.

**Why the cascade is wrong:** during the BSV first-seen conflict window (between when the second tx is broadcast and when a block resolves the conflict), ARC marks BOTH txs as DOUBLE_SPEND_ATTEMPTED — each one is a "double-spend attempt" relative to the other. If TaskCheckForProofs runs during this window and queries both txs, it gets DOUBLE_SPEND_ATTEMPTED for both and marks both failed. The next block resolves the conflict and one tx wins, but by then we've already destroyed the local DB state for the winning tx.

In our incident, `61c03ecc` was the legitimate winning tx (broadcast first by old wallet), but TaskCheckForProofs ran 21 seconds after `260f65fd` was rejected, while both were still in the conflict window. ARC returned DOUBLE_SPEND_ATTEMPTED for `61c03ecc` (because `260f65fd` was a double-spend attempt against it), the task marked it failed, and the cascade gutted its DB record.

### Fix options for Bug C

1. **C1 — Wait for resolution before marking failed on DOUBLE_SPEND_ATTEMPTED.** If ARC says DOUBLE_SPEND_ATTEMPTED, schedule a re-check in N minutes instead of immediately marking failed. By the time the re-check runs, a block has likely resolved the conflict and ARC will return either MINED or a more definitive failure.
2. **C2 — Cross-verify with a block-explorer mined-status check before marking failed.** When ARC returns DOUBLE_SPEND_ATTEMPTED, query WoC's `/v1/bsv/main/tx/hash/{txid}` to see if it's already mined. If yes, recover; if no, schedule re-check.
3. **C3 — Distinguish "first-seen winner" vs "first-seen loser" via `competingTxs` field.** If ARC returns competing tx info, we can determine which is older. Skip mark_failed for the older tx.

**Recommendation: C2.** It's the smallest behavioral change (just one extra check before marking failed) and uses the chain (via WoC) as the source of truth, which aligns with the user's correct framing. C1 could be added on top as a safety net.

---

## DB recovery plan (proposed — DO NOT RUN WITHOUT REVIEW)

The goal is to restore local DB to match chain reality:

### Actions needed

1. **Mark `1a24b540:0,1` as spent by `61c03ecc`:**
   ```sql
   UPDATE outputs
   SET spendable = 0,
       spent_by = (SELECT id FROM transactions WHERE txid = '61c03eccebd8c3d2bebd6b38bd4d875737572a0236e9183c8064ceb4d3a2f0d9'),
       spending_description = '61c03eccebd8c3d2bebd6b38bd4d875737572a0236e9183c8064ceb4d3a2f0d9',
       updated_at = strftime('%s','now')
   WHERE outputId IN (218167, 218168);
   ```
   (`1a24b540:2` is already correctly marked as `external-spend`, leave it.)

2. **Re-insert `61c03ecc:0` (PushDrop) as spendable:**
   - Need: `output_id` (auto), `user_id=1`, `transaction_id=375`, `txid='61c03ecc...'`, `vout=0`, `satoshis=1000`, `derivation_prefix='1-wallet-backup'`, `derivation_suffix='1'`, `spendable=1`, `change=0`, `provided_by='you'`, `purpose='backup'`, `output_type='custom'`, `basket_id=3` (the wallet-backup basket — verify ID), `locking_script` (the BLOB)
   - The locking_script BLOB is the PushDrop. We can either:
     - (a) Reconstruct it deterministically from the wallet's master key (the script is `<master_pubkey> OP_CHECKSIG OP_DROP OP_DROP <encrypted_payload> OP_DROP`)
     - (b) Fetch the raw tx from chain (`/v1/bsv/main/tx/hash/61c03ecc.../hex`) and parse out vout 0's locking script
     - (c) Both, and verify they match
   - **Option (b) is safest — chain is source of truth.**

3. **Re-insert `61c03ecc:1` (marker) as spendable:**
   - Same pattern, vout=1, satoshis=546, derivation_prefix='1-wallet-backup', derivation_suffix='marker', purpose='marker', output_type='p2pkh', locking_script from chain

4. **`61c03ecc:2` is already in DB correctly** (re-inserted by TaskSyncPending as a normal '2-receive address' output). Leave it.

### Risks of the recovery plan

- **Risk 1: Double-counting balance.** `61c03ecc:0` is 1000 sats and `61c03ecc:1` is 546 sats. If we add them and they're already accounted for elsewhere (e.g., as external receives), balance would be wrong. → Verify by checking `wallet_balance` before and after; the delta should be exactly +1546 sats.
- **Risk 2: PushDrop re-insertion uses wrong script encoding.** If the locking_script BLOB format is wrong, the next backup attempt to consume `61c03ecc:0` as input will fail signing. → Mitigation: copy the exact bytes from the chain via `/tx/hash/.../hex` and parse with the same parser the wallet uses.
- **Risk 3: basket_id is wrong.** The wallet-backup outputs need the correct basket FK. → Look up the existing wallet-backup basket id from the basket table before inserting.
- **Risk 4: We make a typo in SQL and corrupt unrelated data.** → Take a full DB file backup before any SQL is run. Run on a copy first if possible. Use `BEGIN; ... COMMIT;` with explicit verification of row counts at each step.

### Alternative recovery: trigger existing recovery code

The wallet has `wallet_recover_onchain` which decrypts the on-chain backup and rebuilds local state. We could:
1. Stop the wallet
2. Back up the wallet.db file
3. Delete the wallet (or use a fresh copy with same mnemonic)
4. Run `wallet_recover_onchain` with the treasury mnemonic
5. The recovery should pull `61c03ecc`'s payload and rebuild everything

**Risk:** this is the highest-stakes operation. The treasury wallet has 525+ outputs, 231 transactions, ~$thousands in value. Recovery flow has known fragility. If it goes wrong, we restore from the .db backup. But the failure mode could be silent (e.g., wrong basket assignments, missing tags) and only show up later.

**Recommendation:** prefer SQL surgery (option 1) over recovery (alternative). Smaller blast radius.

---

## Where we are right now (UPDATED 2026-04-11 ~19:50 UTC)

- **DB fix complete (option 1a — SQL surgery).** Live wallet.db swapped at 19:47 UTC. Backups preserved at `/tmp/wallet_incident_20260411/wallet.db.backup-incident-pre-fix` (+ -wal, -shm). Live wallet restarted, balance reconciles, no cascade re-triggered. Detailed verification:
  - 527 outputs total (was 525, +2 for re-inserted `61c03ecc:0,1`)
  - `1a24b540:0,1` marked spent_by=375 with spending_description set to `61c03ecc...`
  - `61c03ecc:0` (PushDrop, outputId 221223) spendable=1, derivation_prefix='1-wallet-backup', suffix='1', 1000 sats, locking_script extracted from `transactions.raw_tx` BLOB and cross-verified by double-SHA256 against the txid
  - `61c03ecc:1` (marker, outputId 221224) spendable=1, derivation_prefix='1-wallet-backup', suffix='marker', 546 sats, locking_script byte-identical to the reference 1a24b540:1 marker (deterministic BRC-42 self-derivation address)
  - `/wallet/balance` reports 47,652,440 sats (= total spendable 47,653,986 minus wallet-backup 1,546). Math reconciles.
- **Wallet process:** Background task `bo8bkmz95` (started 19:47 UTC after the swap). Running. Monitor running. **Do NOT trigger any backup** — Bug A is unfixed.
- **Strip code:** Still in working tree at `rust-wallet/src/backup.rs:1000+`. NOT committed. Working correctly in-memory (verified on the failed-and-rolled-back 260f65fd attempt: dropped exactly 64 records).
- **Plan file:** `~/.claude/plans/sequential-tumbling-spindle.md` — has STATUS BLOCKED header at the top.

### Next phase: research before fixing the bugs

Per user direction (reasonable framing — we made too many assumptions about ARC behavior in the original Bug C analysis):

1. **Read bsv-rust-sdk and wallet-toolbox-rs broadcast and proof-checking code.** Look for:
   - How they handle ARC `txStatus: SEEN_ON_NETWORK` when the returned txid differs from the broadcast txid
   - What semantic they assign to `DOUBLE_SPEND_ATTEMPTED`
   - Whether they cross-verify before marking failed
   - Whether they check WoC's address-unspent index when sweeping orphans
2. **Find ARC API documentation** for the meaning of `txStatus` values (especially DOUBLE_SPEND_ATTEMPTED and the SEEN_ON_NETWORK + different-txid combination)
3. **Look at the stored ARC raw responses** in our log files for evidence of which interpretation is correct
4. **Then propose fixes** for Bugs A, B, C with each fix grounded in concrete sibling-implementation reference

The assumption that "BSV first-seen marks BOTH txs as DOUBLE_SPEND_ATTEMPTED in mempool" is currently UNVERIFIED. It might be true. It might be wrong. Do not act on it.

## Resume checklist (for future Claude session if context resets)

1. Read this file end-to-end.
2. Read `~/.claude/plans/sequential-tumbling-spindle.md` for the strip code spec.
3. Re-snapshot DB (`/tmp/wallet_now*.db`) and confirm it still matches the "Current local DB state (incorrect)" table above. If it has drifted (e.g., monitor task ran and changed something), update the table before proceeding.
4. Decide with the user: (a) SQL surgery to fix DB, or (b) recovery flow, or (c) just trash this test wallet and switch to a new one.
5. After DB is fixed: do NOT trigger another backup yet. Fix Bug A first (orphan-sweep) so the next backup doesn't re-trigger the cascade. Bugs B and C can be filed as follow-ups but Bug A is the gating fix.
6. After Bug A fix: trigger one backup, verify chain state and DB match. THEN commit the strip.
7. Do NOT proceed to P0 #2 (address strip) until P0 #1 is committed and the wallet is in a good state.

## Out of scope for this incident — to be filed separately

- The post-beta3-cleanup.md doc should get a new entry for the three bugs (A/B/C) as discovered post-beta.3 issues
- A memory entry should be created so any future Claude session investigating backup issues immediately surfaces this incident
- The wallet-efficiency-and-bsv-alignment.md plan should be updated to note that "the strip itself is verified working in-memory but on-chain end-to-end testing is blocked on Bug A fix"
- Long-term: `cleanup_old_spent()` is dead code (never called) — should either be wired in or deleted (was noted in the original plan file)

---

*Written 2026-04-11 ~18:30 UTC. Last action taken: read the orphan-sweep + TaskCheckForProofs code. No DB modifications, no broadcasts, no commits since the incident was discovered.*
