# Spent-Input Reconcile — Phase 2 Design (v2, post-adversarial-review)

> The shared `reconcile_spent_inputs` primitive that fixes BOTH the regular-send
> `"500: Missing inputs"` stuck-spend loop AND the backup-token divergence, by
> reconciling a phantom UTXO (DB says spendable, chain says spent) against the
> chain and recovering the successor's wallet-owned change.
>
> Parent: [`RECONCILE_SPENT_INPUTS_PLAN.md`](./RECONCILE_SPENT_INPUTS_PLAN.md).
> Supersedes the backup-only [`FIX_A_RECONCILE_PLAN.md`](./FIX_A_RECONCILE_PLAN.md);
> composes with [`FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md`](./FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md).

**Status:** design **v2** — revised after a 4-agent adversarial review (2026-07-10)
that found 4 CRITICAL-class defects in v1 (§8 review log). **§9 decisions SETTLED**
(A=1-conf+canonical-check, B=general-reorg-ticket-not-reconcile-special, C=retry-hook+status)
after a reorg-model research pass. **Ready for the phased build** (commits 1–3 are
behavior-neutral extractions; 4 is the money fix) pending owner "go".
**Scope:** `rust-wallet/` (+ one small frontend retry hook, §2a). No schema change,
no crypto/derivation change (Invariants #2/#3 hold). Ships in a `0.3.0-beta.XX`
before the 0.4.0 chromium bump.

---

## 0. Research basis (Phase 1 — verified, condensed)

**Owner decisions locked:** reactive-first (no proactive self-healer yet);
post-failure (no pre-flight validation); providers promoted for *reads only*
(BananaBlocks fallback behind WoC for spent-check + BEEF; keep GorillaPool ARC
primary for broadcast; hold ARCADE).

**D1 — Derivation (linchpin).** Change is **always self-derived** at invoice
`"2-receive address-{index}"`, plain decimal index (`handlers.rs:5486-5500`) →
index-enumerable. Recovery = derive `"2-receive address-{i}"` self-pubkey → P2PKH
script → exact byte-compare to an on-chain output; match → signable via
`derive_key_for_output(db, "2-receive address", "{i}", None)` (`helpers.rs:111-123`).

**D2 — Gap-scan anchor.** Change consumes indices via **`MAX(addresses.index)+1`**
(`handlers.rs:5452-5462`), over `index>=0` only (`address_repo.rs:156-159`). Scan
anchors on `MAX(addresses.index)`, **symmetric** window `[max−back, max+gap_limit]`.
Dual-counter cleanup tracked separately ([`FOLLOWUP_NEXT_INDEX_UNIFICATION.md`](./FOLLOWUP_NEXT_INDEX_UNIFICATION.md)).

**D3 — Cache-first picks a *candidate*; derivation is re-verified before insert.**
Change addresses are in the `addresses` table (`handlers.rs:5529`), so exact-script
match finds the index fast — **but the `addresses` table has no derivation-method
column**, so on a mnemonic-recovered wallet an `index>=0` row may be a **BIP32**
address, not BRC-42 (review D-K1). Therefore the cache match only proposes a
*candidate index N*; the primitive **always re-derives `"2-receive address-{N}"` and
byte-compares to the output before inserting**, and skips on mismatch. This also
keeps us clear of the disabled speculative scan's phantom class (`recovery.rs:195-199`):
we only ever insert an output that (a) is a real output of a confirmation-gated tx and
(b) verifiably derives from a self index we hold the key for.

**D4 — Backup anchor (do NOT touch; confirmed airtight by review).** Backup address =
BRC-42 self invoice `"1-wallet-backup-1"`, stored in `addresses` at **index −3**
(`connection.rs:389-420`) — the deterministic recovery anchor. Backup *outputs*: the
token (vout0) is a **PushDrop** (never matches a P2PKH compare), the marker (vout1) is
P2PKH at the −3 address (`index<0` → skipped), the change (vout2) is a normal
`"2-receive address-{i}"`. `"2-receive address-{i}"` can never collide with
`"1-wallet-backup-1"` (different invoices → ~2⁻¹⁶⁰). The primitive **skips any match
with `index<0` or prefix `"1-wallet-backup"`.**

**D5 — Authoritative spent-signal (fail-closed core).** Two positive, cross-validated
signals; WoC (TAAL) + BananaBlocks (GorillaPool) are **independent infra**:

| Signal | Spent | Unspent | Phantom parent |
|---|---|---|---|
| WoC single `/tx/{txid}/{vout}/spent` | `200 {txid:Y,...}` | `404` | `400`/other |
| BananaBlocks `/txo/{txid}/{vout}/spend` | `200 {spent:true, spentTxid:Y}` | `200 {spent:false}` | `404` |

**⚠ Status-code specifics (400/404 shapes) are Phase-1 assumptions pending a live
probe; the *safety* holds regardless because "not a clean positive → no-op."** Traps:
ARC `460`≠`462`; ARC returns 200 for `SEEN_IN_ORPHAN_MEMPOOL`; WoC **bulk**
`/utxos/spent` can't distinguish unspent from phantom → **single endpoint only**;
never infer spent (or unspent) from absence.

**D6 — Existing handlers unified (reuse-first).** Three divergent Missing-inputs sites
the primitive replaces: (1) `create_action` (`handlers.rs:6382-6398`) **restores the
input** → the live loop; (2) `certificate_handlers.rs:3013-3069` treats WoC `404` as
"spent" (fail-OPEN) via the *wrong* endpoint (`/outspend/`), marks `spent_by="unknown"`,
no change recovery; (3) `do_onchain_backup`.

---

## 1. The shared primitive

```rust
async fn reconcile_spent_inputs(
    state: &AppState,
    candidate_outpoints: &[(String, u32)],
) -> ReconcileReport   // { marked_spent, change_recovered, changed }
```

Per candidate `(txid, vout)`:

1. **Authoritative spent-check** — `check_outpoint_spent -> Spent(Y) | Unspent | Unknown`
   via a **total decision table keyed on the provider pair** (extract from
   `handlers.rs:12967-12998`, add BananaBlocks). Rules:
   - **To conclude `Spent(Y)` (the mutating direction): require WoC `200`+valid-hex-txid
     AND BananaBlocks non-contradicting** (either `spent:true` with matching txid, or
     BananaBlocks errored/unavailable). A single provider's positive is **not** enough
     if the other actively contradicts (review D-P1). `200` without a valid hex txid →
     `Unknown` (never the `"unknown"` sentinel; review D-P3).
   - `404`/`spent:false` (agreed) → `Unspent`.
   - Phantom-parent / disagreement / network error / any non-clean-positive → `Unknown`.
   - `Unspent`/`Unknown` → **do nothing** (fail closed).
2. **Fetch spending tx `Y`** (`services::WalletServices`; BananaBlocks `/beef` fallback,
   skip 36-byte prefix) and **verify `SHA256d(raw)==Y` before parsing** (reuse
   `ParentTransactionRepository::verify_txid`) — closes fallback-provider poisoning
   (review D-K2). Fail closed on fetch/parse/verify error.
3. **Confirmation gate (BLOCKING) = 1 confirmation + canonical merkle check** — mutate only
   if `Y` is mined (≥1 conf) **and its merkle proof passes `verify_tsc_proof_against_block`**
   (`cache_helpers.rs:168-237`, by height vs the block's merkle root — the exact check that
   already gates our own proofs). This is the **consistent** bar: we mark our own inputs spent
   at depth-0 and treat `confirmations>0` as confirmed (`task_check_for_proofs.rs:431`), so a
   deeper gate for a phantom would be stricter than how we treat our own money. Below 1 conf →
   read-only. Reorg posture: a reorg reverts `Y` to pending and it re-mines like any tx (step 7).
4. **Recover ALL wallet-owned self-derived outputs of `Y`** — by *derivation match*, NOT by
   "which vout is the change." Recovers change AND self-sends to generated receive addresses
   (normal send owns 1; send-to-self owns several; a backup owns its funding change while its
   PushDrop token (non-P2PKH) + `-3` marker are skipped). **Not recovered here:** counterparty
   / BRC-29 / PeerPay receipts — they arrive via MessageBox/sync and would be mis-keyed; the
   self-derivation scan simply won't match them (correct). Per output of `Y`:
   - a. **Cache-first candidate:** exact-script match vs `addresses` (reuse
     `reconcile_backup_tx:13820-13851`) → candidate index `N`.
   - b. **Uncached:** bounded gap-scan, anchor `MAX(index)`, window `[max−back, max+gap_limit]`.
   - c. **Verify-before-insert (BLOCKING):** re-derive `"2-receive address-{N}"` → P2PKH
     → byte-equal the output's script. **Skip** any `index<0` / `"1-wallet-backup"` /
     non-verifying match (guards BIP32 wrong-key + backup anchor — review D-K1/D4).
   - d. **Positive-unspent required to insert:** re-run step 1 on `(Y, vout)`; insert
     **only** on a positive `Unspent`. `Unknown` → **skip** (do not insert on absence —
     review D-P2). Insert via `upsert_received_utxo_with_confirmed` (correct prefix/suffix
     from the *verified* index, real `confirmed`, `spendable=1`).
5. **Mark the phantom spent** — `mark_spent(txid, vout, Y)` (`output_repo.rs:736`) sets
   `spending_description=Y`, `spendable=0`. **Do NOT insert a synthetic `transactions`
   row for `Y`** (pollutes history + thrashes `TaskCheckForProofs`/`TaskSendWaiting`/
   `TaskUnFail` on empty `raw_tx` — review D-R2/D-I5). `Y` as `spending_description` is a
   *real successor txid* → durable against `restore_pending_placeholders` (`LIKE 'pending-%'`)
   and `TaskReviewStatus` #3 (keys on the failed *local* txid).
6. **Atomicity (corrected):** network reads (steps 1–4a/b/d) are **best-effort
   preconditions** and cannot share a lock scope with the write (they `await`). The
   atomic scope wraps **only the final DB write burst** — insert verified change +
   `mark_spent` + single `balance_cache.invalidate()` — under the DB lock, **re-asserting
   `spendable=1` at write time**. Residual TOCTOU vs the Monitor is accepted and cleaned
   by sync-reconcile (review D-R2).
7. **Reorg posture — consistent with the wallet's existing model, NOT reconcile-special.**
   A reorg does not delete `Y`; like any valid tx it reverts to pending and re-mines into the
   new chain, so the phantom's spent status survives. Research confirmed both (i) our wallet
   and (ii) `@bsv/wallet-toolbox` spend **optimistically at depth-0** and never make the user
   wait N confs; the toolbox's reorg answer is `reproveProven` (refresh the proof *in place*,
   tx stays `completed`), never an un-mark. The v1 `proven_tx_req`-for-`Y` "insurance" is
   **removed — it was a no-op** (`TaskUnFail` only reverses our own `failed` sends). We add **no
   reconcile-specific reversal.** The real gap — our wallet has **no reprove-on-reorg for ANY
   completed tx** — is a **general** hardening ticket ([`FOLLOWUP_REORG_HANDLING.md`](./FOLLOWUP_REORG_HANDLING.md)),
   out of this sprint; it backstops reconcile and every other tx symmetrically.

**Tip-resolution is discovery-only** (review D-P5): the marker/address unspent query may
*propose* candidate successor outpoints, but spent-ness of each is decided solely by the
single-outpoint check in step 1. Never treat "absent from an address list" as spent.

---

## 2. Triggers (reactive-only this sprint)

### 2a. Regular send — `create_action` broadcast failure (THE critical path) — REWORKED
v1 was wrong here on three counts (review D-I1/2/3): the phantom is **already
`spendable=0`** (reserved to the failed txid) when the failure block runs, so `mark_spent`
(guarded `WHERE spendable=1`) would **no-op**; in-handler rebuild **deadlocks** on the
held `create_action_lock` (`handlers.rs:4523`, non-reentrant); and the `changed` branch
would leak the other reserved inputs + commission. Corrected flow at the Missing-inputs
branch (`handlers.rs:6366`), only when `is_fatal_broadcast_error` matches Missing-inputs
and **not** `is_double_spend_error`:
1. Delete ghost change (unchanged, `:6346-6364`).
2. **Restore all selected inputs to `spendable=1`** (`restore_spent_by_txid(final_txid)` /
   `restore_by_spending_description(placeholder)`) — so reconcile's `mark_spent` guard is
   satisfiable and the *good* (non-phantom) inputs are re-selectable.
3. `reconcile_spent_inputs(selected_inputs)` — marks the genuine phantom spent (`spending_
   description=Y`), inserts verified recovered change.
4. **Run the existing full cleanup on every branch** (commission delete `:6400-6408`, etc.).
5. If `report.changed` → **return a distinct retryable error `ERR_RECONCILED_RETRY`**; the
   **frontend retries `create_action` once from the top** (lock free, phantom now
   `spendable=0`/excluded, recovered change `spendable=1`). This mirrors the cert path's
   existing "please retry" contract and makes bounded-retry (guardrail #9) trivially correct
   — one reconcile per invocation, retry = a separate invocation. **In-handler rebuild is
   NOT attempted** (review D-I2).
6. If `!report.changed` → today's behavior (inputs already restored in step 2, return
   `ERR_BROADCAST_FAILED`) — preserves non-phantom (e.g. BEEF-`460`) handling.

> **Frontend hook (small, new):** the send flow must auto-retry once on
> `ERR_RECONCILED_RETRY`. Without it the user simply re-clicks Send and it works — but the
> one-retry hook makes it seamless. This is the only non-Rust change in the sprint.

### 2b. Certificate acquire — replace the fail-open retry
Replace `certificate_handlers.rs:3013-3069`: reconcile (fixing the `/outspend/` `404→spent`
fail-open and `spent_by="unknown"`), then **return the same retryable error** (the built tx
can't be re-broadcast — its input is spent; it needs a fresh top-level rebuild, which the
createAction-based caller already tolerates via the `broadcast_status=='failed'` check,
`certificate_handlers.rs:4238`). Net safety win.

### 2c. Backup — subsumes FIX_A
`do_onchain_backup` calls `reconcile_spent_inputs(funding_utxos)` **after selection, before
placeholder reservation** — inputs still `spendable=1`, so no D-I1 issue. Pass **only
`funding_utxos`** (never the separately-reserved `previous_pushdrop`/`previous_marker`
backup inputs — review D-I6), then re-select once and continue. **Wrap in
`utxo_selection_lock`** (backup holds neither lock today — BS-C1 — so reconcile-in-backup
would otherwise race a concurrent send; review D-R3).

> Proactive startup/Monitor self-healer remains **deferred**; when added it reuses this
> primitive + guardrails, idempotent + unlock-gated + provable no-op on a healthy DB.

---

## 2d. Worked example — the dev wallet's broken backup token (smoke-test spec)
DB tip `7c4423f4`; chain tip `ef67fd9e`@956718; phantom = `7c4423f4:2` (old funding change,
already spent by `ef67fd9e`). Backup runs (2c) → selects `7c4423f4:2` → reconcile:
1. `check_outpoint_spent(7c4423f4, 2)` → `Spent(ef67fd9e)` (WoC+BananaBlocks agree).
2. Fetch `ef67fd9e`, `verify_txid` OK, ≥1 conf + `verify_tsc_proof_against_block` OK.
3. Scan `ef67fd9e`'s outputs: `:0` PushDrop token → non-P2PKH → **skip**; `:1` marker at `-3`
   → `index<0` → **skip**; `:2` change → matches a receive index, derivation-verified → **insert
   spendable**.
4. `mark_spent(7c4423f4, 2, ef67fd9e)`.
5. Re-select → backup funds from real `ef67fd9e:2` → **succeeds**; the existing `adopt` logic
   handles the token/marker tip pointer. Relaunch → no-op (phantom already `spendable=0`).
**Expected:** loop broken, balance reflects the recovered funding, backup completes. This is
the first manual smoke; final confidence comes from running it on the actual diverged DB.

## 3. Lock contract (explicit — review D-R3)
The primitive itself does pure work + a final short DB-write burst; it does **not** acquire
`create_action_lock`. Callers own serialization: 2a already holds `create_action_lock`
(accept the multi-second stall on a single-user wallet, but **bound per-call network
timeouts** and cap per-output re-checks so the hold is bounded); 2c **must** hold
`utxo_selection_lock` around reconcile+re-select. Document this contract on the fn.

## 4. Reuse audit (composition, not new machinery)
| Step | Reuse | New |
|---|---|---|
| Spent-check | extract `check_outpoint_spent` (`handlers.rs:12967-12998`) | + BananaBlocks branch, total decision table, no-"unknown" |
| Fetch + verify `Y` | `services::WalletServices`, `ParentTransactionRepository::verify_txid` | + BananaBlocks `/beef` fallback |
| Parse outputs | `extract_output_value_and_script` (`handlers.rs:13643`) | — |
| Candidate index | `reconcile_backup_tx:13820-13851` | extract `recover_change_index` + **derivation re-verify** + bounded gap-scan |
| Insert change | `upsert_received_utxo_with_confirmed` (`output_repo.rs:441-471`) | — |
| Mark phantom | `mark_spent` (`output_repo.rs:736`) | (drop synthetic tx-row) |
| Restore inputs (2a) | `restore_spent_by_txid` / `restore_by_spending_description` | — |
| Classify | `arc_status::is_fatal_broadcast_error` / `is_double_spend_error` | — |

---

## 5. Guardrails (BLOCKING)
1. **Fail closed** on ambiguous read → mutate nothing.
2. **Positive spent-proof required** (single-outpoint `/spent`+`/spend`); **cross-validate
   both providers non-contradicting for the mutating direction**; never infer from absence.
3. **Confirmation gate = 1 conf + `verify_tsc_proof_against_block`** before any mark/insert
   (consistent with our own depth-0 optimistic spends — §1 step 3).
4. **Verify-before-insert:** re-derive `"2-receive address-{N}"` + byte-compare; `index≥0`
   only; **skip −3 / `"1-wallet-backup"` / BIP32 / non-verifying**; never NULL/master/guessed.
5. **No speculative/absence inserts:** insert only an output that is a real output of a
   verified, confirmation-gated `Y`, derivation-verified, **and positively Unspent**.
6. **Scan all outputs** of `Y`.
7. **Atomic final-write burst** only (network reads are preconditions); re-assert
   `spendable=1` at write; single cache invalidate; **no synthetic `transactions` row**.
8. **Reorg:** consistent with our optimistic depth-0 model (a reorg re-mines `Y` like any
   tx); **no fictional reversal claim, no reconcile-special undo**; general reprove-on-reorg
   is a separate wallet-wide ticket ([`FOLLOWUP_REORG_HANDLING.md`](./FOLLOWUP_REORG_HANDLING.md)).
9. **Bounded:** gap-scan window ≤50; **retry is a separate top-level invocation** (return
   `ERR_RECONCILED_RETRY`), never in-handler rebuild.
10. **Fallback-only providers**; nothing GorillaPool-backed is a sole authority.
11. **Preserve non-phantom behavior:** `!changed` → today's restore/return unchanged.
12. **Explicit lock contract** (§3); 2c holds `utxo_selection_lock`.

---

## 6. Phased commits (behavior changes at 4/5)
1. Extract `check_outpoint_spent` (total decision table, BananaBlocks, no-"unknown", txid-verify helper) — repoint existing adopt branch; unit tests.
2. Extract `recover_change_index` (cache candidate + **derivation re-verify** + bounded gap-scan, skip −3/backup/BIP32) — unwired; unit tests.
3. Add `reconcile_spent_inputs` (fail-closed, conf-gated, positive-unspent insert, atomic write-burst, no synthetic row) — unwired; unit tests.
4. Wire `create_action` (restore-first → reconcile → full cleanup → `ERR_RECONCILED_RETRY`) + **frontend one-retry hook** — the critical fix.
5. Wire cert (replace fail-open, return-retry) + `do_onchain_backup` (funding-only, `utxo_selection_lock`).
6. (Decision B) optional reconcile-reversal Monitor task. Docs.

---

## 7. Test plan (additions from review in **bold**)
**Unit:** decision-table mapping incl. **provider-disagreement→Unknown**, **200-without-hex-
txid→Unknown**; `recover_change_index` returns verified `Some(N)` for BRC-42, **`None` for a
BIP32 index-≥0 row (wrong-key guard)**, `None` for −1/−3/foreign; fail-closed (Unknown/Unspent
→ zero mutations); **positive-unspent-required insert (Unknown→no insert)**; happy path
(phantom→`spending_description=Y`, verified change inserted); unconfirmed successor→no marks;
backup-output-in-`Y`→skipped; **`verify_txid` mismatch→fail closed**.
**Integration (mocked WoC+BananaBlocks):** `create_action` diverged fixture → **restore-first
→ reconcile → `ERR_RECONCILED_RETRY` → frontend-retry succeeds**; **healthy wallet→no-op**;
**provider-disagreement→no-op**; **`mark_spent` sticks (spendable=0, `spending_description=Y`)
and survives a `TaskReviewStatus`/`restore_pending_placeholders` pass** (loop-free terminal);
multi-hop ≤2 passes; idempotent across repeated sends.
**Manual smoke (dev):** field divergence (`7c4423f4`→`ef67fd9e`@956718); reconcile logs; retry
send funds from recovered change; relaunch no-op. **Real fixture: beta.26 user's send logs
(need the ARC line).**
**Reorg (Decision B, if built):** confirm a reversal task restores a `reconciled:{Y}` phantom
when `Y` is orphaned.
**Parity:** portable Rust; `cargo test` + smoke on Windows and seeded macOS.

---

## 8. Adversarial review log — v1 → v2 (2026-07-10, 4 agents)
| ID | Sev | Finding | v2 resolution |
|---|---|---|---|
| D-R1 | **CRIT** | Reorg "insurance" fictional (`TaskUnFail` can't reverse a foreign `spent_by`; no reorg task). | Resolved by consistency (follow-up research): a reorg re-mines `Y` like any tx; we spend optimistically at depth-0 already; gate = **1 conf + `verify_tsc_proof_against_block`**; drop the proven_tx_req claim; general reprove-on-reorg = separate wallet-wide ticket, not reconcile-special. |
| D-I1 | **CRIT** | In `create_action` the phantom is already `spendable=0` → `mark_spent` no-ops → never marked. | 2a step 2: **restore inputs to `spendable=1` before reconcile.** |
| D-I2 | **CRIT** | In-handler rebuild deadlocks (`create_action_lock` non-reentrant); no build helper. | 2a: **return `ERR_RECONCILED_RETRY`; frontend retries from top.** |
| D-K1 | **HIGH** | Cache-first match → hardcoded `"2-receive address"` prefix poisons **BIP32** index-≥0 rows on recovered wallets (wrong-key). | **Verify-before-insert** (re-derive + byte-compare); skip non-verifying (guardrail #4). |
| D-P1 | **HIGH** | "Either-positive" made disagreement→fail-closed unreachable; a single lagging provider could mark a live coin spent. | **Require both providers non-contradicting for the mutating direction** (guardrail #2). |
| D-P2 | **HIGH** | Insert "still unspent" re-check was absence-based → can plant a fresh phantom. | Insert requires **positive Unspent**; `Unknown`→skip; D3 softened (bounded self-heal). |
| D-R2 | **HIGH** | "One lock scope" impossible (network `await`); synthetic `Y` tx-row pollutes + thrashes Monitor. | Atomic = final write burst only; **no synthetic tx row**; re-assert `spendable=1` at write. |
| D-R3 | **HIGH** | Lock context differs per trigger; 2c (backup) races a concurrent send. | **Explicit lock contract** (§3); 2c holds `utxo_selection_lock`. |
| D-I3 | HIGH | `changed` branch leaked good inputs + commission → spurious insufficient funds. | 2a: **full cleanup + input restore on every branch.** |
| D-K2 | MED | Fetched `Y` not txid-verified → fallback-provider poisoning. | **`verify_txid(raw)==Y` before parsing.** |
| D-P3 | MED | `unwrap_or("unknown")` reintroduces fail-open. | 200-without-hex-txid → `Unknown`. |
| D-P5 | MED | Tip-resolution reused an address/absence query. | **Discovery-only**; spent-ness by single-outpoint check. |
| D-K3 | LOW | Multi-device change minted elsewhere → heal-miss (marks phantom spent, under-recovers). | Documented limitation; deferred proactive healer / rescan is backstop. |
| D-K4 | LOW | Master (−1) change skipped. | Documented deliberate skip (consistent with #4). |
**Confirmed sound by review:** backup anchor airtight (D4), ownership-match prevents inserting
foreign change, 2a Missing-inputs-only gating, `/spent` (not `/outspend/`) endpoint choice.

---

## 9. Owner decisions — SETTLED (2026-07-10, after reorg research)
**A. Confirmation gate = 1 confirmation + `verify_tsc_proof_against_block`.** Consistent with
our own optimistic depth-0 spending and `confirmations>0`-means-confirmed; research confirmed
`@bsv/wallet-toolbox` also uses no N-conf gate. Heals immediately (Y is ~always already mined),
balance immediate. No new depth constant. **(Owner-aligned; confirm.)**
**B. No reconcile-specific reversal.** Reorg handling is a **general** wallet gap (we have none
for any completed tx) → separate ticket [`FOLLOWUP_REORG_HANDLING.md`](./FOLLOWUP_REORG_HANDLING.md)
(a `TaskReproveOnReorg` mirroring wallet-toolbox `reproveProven`; reuses `verify_tsc_proof_against_block`
+ `replace_proof`/`mark_failed`; no crypto/schema change). Out of this sprint. **(Owner-aligned.)**
**C. Frontend one-retry hook on `ERR_RECONCILED_RETRY`** + inline "Updating your balance…" status
+ short success toast, non-blocking. The sprint's only non-Rust change. **(Owner: yes.)**
Minor: gap-scan `back`/`gap_limit` defaults (propose back=`gap_limit`=20, cap 50) — widening
`back` mitigates D-K3.
