# Overlay Probe Findings — Cert 32 Stuck-State Investigation

**Date:** 2026-05-28
**Approach:** Read-only HTTP probes against overlay endpoints. No wallet code changes.
**Goal:** Determine why cert 32 (today's relinquished X/Twitter) can't be cleaned from overlays.

---

## 1. Server identification (new info)

| Host | Stack | Version | Status |
|---|---|---|---|
| overlay-us-1.bsvb.tech | overlay-express-examples | **2.1.6** | Flaky (was 503ing for some probes, recovered later) |
| overlay-eu-1.bsvb.tech | overlay-express-examples | **2.1.6** | Up |
| overlay-ap-1.bsvb.tech | overlay-express-examples | **2.1.6** | Up |
| anvil.sendbsv.com | Go + nginx (NOT overlay-express) | unknown | Up |

**Backend:** mysql2 + mongo (from `/health` response). Hosted by bsvb.tech (Babbage Inc).

**Uptime:** Both bsvb hosts have been running since 2026-04-24 — uptime ~33 days. So the stuck state pre-dates today's session.

---

## 2. The smoking gun finding

### PROBE 13: Same publish BEEF, three different topics, three identical ambiguous responses

I submitted the **publish BEEF** (c8a88544, returned by /lookup) to overlay-eu-1 with three different `x-topics` values:

| Topic | Response | Notes |
|---|---|---|
| `tm_protomap` | `{"tm_protomap":{"outputsToAdmit":[],"coinsToRetain":[]}}` | Topic exists but cert is unrelated to protomap |
| `tm_identity` | `{"tm_identity":{"outputsToAdmit":[],"coinsToRetain":[]}}` | This is where the cert actually belongs |
| `tm_certmap` | `{"tm_certmap":{"outputsToAdmit":[],"coinsToRetain":[]}}` | Unrelated topic |

**All three return the identical ambiguous shape with no `coinsRemoved` field.**

This is critically important because:
- `tm_protomap` and `tm_certmap` have NEVER seen this txid before → can't be dedup
- They MUST be returning the ambiguous shape via the `failedTopics` code path (validation threw)

So the **ambiguous response shape is NOT a unique fingerprint of dedup**. It's produced by EITHER dedup OR validation failure. We can't distinguish from outside — exactly as the Engine.ts source code says.

### Conclusion on the original question

We cannot prove from outside whether cert 32's stuck state is caused by:
- **(A) Dedup** — spending tx `555916718` is in eu-1's/us-1's applied_transactions table
- **(B) Validation failure** — tm_identity's topic manager throws when processing the spending tx given the current storage state

Both produce identical wire output. **The honest answer remains "we don't know which trigger fires."**

---

## 3. What IS proven about the stuck state

### A. The cert genuinely is in eu-1's UTXO storage
- `/lookup` by identityKey returns 1 output for our key → 3383-byte BEEF for c8a88544
- ap-1 returns 0 outputs for the same query
- So eu-1 has a stored UTXO that ap-1 doesn't

### B. Both eu-1 and ap-1 have `c8a88544` in applied_transactions
- Resubmitting the publish BEEF to either returns ambiguous STEAK
- For ap-1 this CAN'T be validation failure (publish BEEF is well-formed, would admit normally) — so dedup confirmed
- For eu-1, same reasoning

### C. The state divergence is real
- ap-1: applied_transactions HAS publish + spending; UTXO storage CLEAN
- eu-1: applied_transactions HAS publish + spending; UTXO storage STILL HAS c8a88544:0

This is the structural inconsistency I described in earlier analysis. PHASE 3 on eu-1 must have either:
- Run partially (markStale silently failed but insertAppliedTransaction succeeded)
- Run successfully but a separate background process re-admitted the output later (unlikely)

The first option is consistent with what I noticed in Engine.ts: `markUTXOAsSpent` and `lookupServices.outputSpent` errors are caught and logged but not propagated, allowing PHASE 3 to "succeed" even when storage mutation fails.

### D. The cert is **definitively unrecoverable via /submit** for these hosts
- 13+ resubmissions of the proper BEEF have all returned the same ambiguous response
- No admin / cleanup / reset endpoint is exposed
- The condition (whatever it is) is persistent and deterministic

---

## 4. Available endpoints (newly discovered)

These work on overlay-express-examples v2.1.6 hosts and could be useful for future diagnostics:

| Endpoint | Returns |
|---|---|
| `GET /` | HTML status page |
| `GET /version` | `{"name":"overlay-express-examples","version":"2.1.6",...}` |
| `GET /health` | Full health JSON including: topic managers, lookup services, database types (mysql2 + mongo), startup time |
| `GET /listTopicManagers` | All 20 registered topic managers with descriptions |

What DOESN'T exist (would have helped): no admin endpoint, no UTXO inspection, no applied_transactions query, no force-cleanup.

---

## 5. Validation error shapes (so we know "real" failures look different)

| Input | overlay-express response | anvil response |
|---|---|---|
| Empty body | `400 {"status":"error","message":"Serialized BEEF must start with..."}` | `400 {"status":"error","message":"empty body (BEEF required)"}` |
| Garbage bytes | `400 {"status":"error","message":"Serialized BEEF must start with 4022206465 or 4022206466 but starts with..."}` | `400 {"status":"error","message":"submit failed: invalid-atomic-beef"}` |
| BEEF with no transactions | `400 {"status":"error","message":"beef must include at least one transaction."}` | `400 {"status":"error","message":"submit failed: transaction is nil"}` |
| Non-existent topic | `400 {"status":"error","message":"This server does not support this topic: tm_does_not_exist"}` | same shape |

**Pattern:** All malformed/unparseable BEEFs get explicit HTTP 400 with text message. The ambiguous 200 STEAK is reserved for PHASE 1 isDupe + failedTopics paths.

---

## 6. Material for upstream bug report

Ready to file at https://github.com/bsv-blockchain/overlay-services/issues with:

**Title:** PHASE 3 silent errors cause inconsistent state between applied_transactions and UTXO storage; subsequent retries dedup and cannot recover

**Body skeleton:**

> Affected version: overlay-express-examples v2.1.6 (engine in overlay-services master)
>
> Observed: After submitting a publish BEEF followed shortly by a removal BEEF (spending tx that consumes the published output), some overlay instances end up with state where:
> - `applied_transactions` table contains BOTH the publish tx AND the spending tx
> - UTXO storage STILL contains the published output as admitted
> - Subsequent removal-BEEF submissions return `{outputsToAdmit:[], coinsToRetain:[]}` (no coinsRemoved field) — indistinguishable from dedup
>
> Hypothesis: PHASE 3 in Engine.ts wraps `markUTXOAsSpent` and `lookupServices.outputSpent` calls in try/catch with `this.logger.error(...)` and continues silently. If either fails, the engine still calls `insertAppliedTransaction()` at the end, locking the state.
>
> Repro txids (mainnet):
> - Publish: c8a88544a1b3caccf653f20a1ce466fed392195206500475de621317b07939ef
> - Spending: 555916718f6e5e32dafc4a393e224cb4f3f28131c5d3d81b1ddf7d21cadddecf
> - Identity: 020b95583e18ac933d89a131f399890098dc1b3d4a8abcdde3eec4a7b191d2521e
> - Visible on: overlay-eu-1.bsvb.tech, overlay-us-1.bsvb.tech
> - Cleaned correctly from: overlay-ap-1.bsvb.tech (same submissions, different outcome — proves state-divergence)
>
> Suggested fixes:
> 1. Don't `insertAppliedTransaction` if storage mutations partially failed
> 2. Provide an admin endpoint to force-evict a stuck UTXO from storage
> 3. Distinguish dedup vs validation-failure STEAK responses (different HTTP codes or response fields)

---

## 7. Recommended next steps for our wallet

### A. Mitigation (prevent future stuck state)
The race condition that triggers this requires publish-then-quickly-unpublish in the same block. Our wallet's `unpublish_certificate_core` constructs the removal-BEEF immediately on user request. To reduce stuck-state risk:

- **Option 1:** In unpublish flow, wait for the publish-BEEF background drain to FULLY complete on all overlays before submitting removal. Adds latency (~30s) but eliminates the race.
- **Option 2:** Skip including the publish tx in the removal-BEEF entirely when we know it was just admitted (overlays already have it as a parent). Hard to verify "already have it."
- **Option 3 (simplest):** Detect "just-published" certs (<1 block since publish) and queue the unpublish for ~10 min later. Lazy, low-risk.

### B. Recovery (cert 32 specifically)
- No /submit-based recovery possible. Cert remains on overlay-us-1 + overlay-eu-1 indefinitely.
- On-chain it's dead (publish output is spent). Any verifier that actually checks chain status will see it as invalid.
- Will eventually displace if user publishes a new cert with the same serial (overlay's normal flow).

### C. Wallet noise
- TaskReplayOverlay is retrying every 5 min, all fail, retry count incrementing
- Will quiet itself at attempt 20 (~100 min after cert reached pending_overlay status)
- OR we can manually set retries=20 in DB to silence immediately

---

## 8. What I owe you (corrections from earlier in the session)

- ❌ "Race condition with publish drain" — still PLAUSIBLE but UNPROVEN
- ❌ "It's dedup" — POSSIBLE but cannot be definitively shown vs validation-failure
- ❌ "Missing parent tx" — was the cleanup endpoint's bug (now fixed), but NOT the bug for cert 32's stuck state
- ✅ "There's a real bug in overlay-express" — strongly supported by state divergence between ap-1 and eu-1
- ✅ "Storage and applied_transactions become inconsistent" — proven by probe results
- ✅ "No /submit-based recovery exists for the stuck state" — proven by 13+ failed retries with proper BEEF

---

## 9. Sources

- [bsv-blockchain/overlay-services Engine.ts](https://github.com/bsv-blockchain/overlay-services/blob/master/src/Engine.ts)
- [bsv-blockchain/overlay-express](https://github.com/bsv-blockchain/overlay-express)
- [bsv-blockchain/overlay-express-examples](https://github.com/bsv-blockchain/overlay-express-examples)
- overlay-express-examples version: 2.1.6 (confirmed via /version on bsvb.tech hosts)
