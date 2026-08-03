# Fix A — Targeted Backup-Funding Reconcile: Implementation Plan

> Heals wallets whose DB UTXO set is STALE vs the chain (a backup landed on-chain
> that the DB never recorded, so the DB's "spendable" funding output is a phantom
> already spent by that successor). Detects the divergence, asks the chain what
> spent the phantom, and — if the successor's change pays a wallet-owned address —
> records the real UTXO and marks the phantom spent, so backups can fund again.
>
> Companion: [`ONCHAIN_BACKUP_REVIEW.md`](./ONCHAIN_BACKUP_REVIEW.md) (§1 the field
> bug, §5 why `/wallet/sync` can't do this) and
> [`FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md`](./FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md)
> (prevents *new* divergences; Fix A heals *existing* ones).

**Status:** design complete, hardened against adversarial review. NOT implemented.
**Basis:** research agent + adversarial red-team + direct code verification.
**Scope:** `rust-wallet/` only. No schema change, no crypto change (Invariants #2/#3 hold).

---

## 1. Problem (recap)

`do_onchain_backup` selects funding from `spendable=1` DB rows and **never validates
they're still unspent on-chain** (`handlers.rs:13093-13158`, esp. `:13145`). When a
prior backup landed on-chain but wasn't recorded (see Fix B for how), the DB's
funding change output is a phantom → every backup builds a tx with a dead input →
`"Missing inputs"` → rollback → infinite retry. `adopt_onchain_backup`
(`handlers.rs:12678`) fixes the *token/marker* pointers but never the *funding*
output, and `/wallet/sync` is insert-only (no reconcile) so it can't clear it.

## 2. Design

A single shared routine, used by both an in-handler pass and a Monitor self-healer.

**`reconcile_diverged_backup_funding(state, client, candidate_outpoints) -> ReconcileReport`**

For each candidate `(txid, vout)` (the funding UTXOs about to be spent, or
backup-related spendables at startup):
1. **Prove spent-ness per-outpoint** via the authoritative endpoint
   `GET /v1/bsv/main/tx/{txid}/{vout}/spent` — logic already inline at
   `handlers.rs:12967-12998`, to be **extracted** as `check_outpoint_spent → {Spent(txid)/Unspent/Unknown}`.
   - `404` (Unspent) → **do nothing**.
   - error / non-200 / non-404 / malformed (Unknown) → **FAIL CLOSED: do nothing.**
   - `200 + spending_txid` → proceed.
2. **Fetch the spending tx** (reuse `adopt_onchain_backup`'s `/tx/{txid}/hex` fetch).
   Fail closed on any fetch/parse error.
3. **Confirmation-depth gate (RED-TEAM C1 — blocking):** only proceed to mutate if
   the spending tx is **confirmed (≥1, prefer ≥2–3)**. An unconfirmed successor is
   read-only — never mark the predecessor spent nor insert its change (a reorg would
   otherwise mark real coins unspendable + insert a phantom, permanently).
4. **Recover the real change (funding replacement)** — for outputs of the spending
   tx (positionally **vout ≥ 2** to avoid the marker at vout 1), match the locking
   script **exactly** against `recovery::address_to_p2pkh_script(&addr.address)` over
   `addr_repo.get_all_by_wallet` (reuse `reconcile_backup_tx:13820-13851`). Insert
   **only** when the match yields a real `index ≥ 0` via `upsert_received_utxo`
   (sets `derivation_prefix="2-receive address"`, `spendable=1`). **Never** insert
   with NULL/NULL, `"master"`, or a guessed index (RED-TEAM C3 — the index-3
   wrong-key poison class). No match → insert nothing (or `spendable=0` diagnostic).
5. **Mark the phantom spent** — pre-insert the spending tx row (`INSERT OR IGNORE`)
   so the FK resolves, then `mark_spent(txid, vout, spending_txid)` (`output_repo.rs:736`)
   → `spent_by` non-NULL so `TaskReviewStatus` can't resurrect it.
6. **Atomicity (RED-TEAM H2 — blocking):** steps 4+5 for a given generation run in
   **one DB-lock scope**, then a single `balance_cache.invalidate()`.

**Resolve to the TIP, not the direct spender (RED-TEAM H1):** for multi-hop
divergence, use the marker-address unspent query to find the *current* backup tip;
`/spent` only *detects* staleness. Follow-up passes (bounded) handle deeper chains.

### 2a. In-handler pass
Run the reconcile on the just-selected `funding_utxos` **after selection (`:13158`)
and before placeholder reservation (`:13165`)**; if it changed anything, **re-run
selection once** (guarded by a `reconciled_once` bool) so the real change is picked
and the phantom (now `spendable=0`) is excluded. If still insufficient, return the
existing `Insufficient funds` error — converting the infinite loop into a clean
terminal state.

### 2b. Startup / Monitor self-healer (heals already-broken field wallets)
Preferred home: a new Monitor task `task_reconcile_backup` (mirrors `task_backup.rs`),
run once shortly after startup + on an interval, gated by `db_available()`. Gets
AppState + balance-cache invalidation for free and heals wallets that diverge while
running. **Must be idempotent, bounded (network), wallet-unlock-gated, and provably a
no-op when the DB backup is confirmed on-chain** (RED-TEAM C2 — a flaky WoC at boot
must never mutate a healthy DB). **Ordering vs Fix B:** Fix B's `sending`-reconcile
runs FIRST, then Fix A (see Fix B doc §Composition).

## 3. Reuse audit (every primitive already exists)
| Step | Reuse | New |
|---|---|---|
| Prove spent | `handlers.rs:12967-12998` → extract `check_outpoint_spent` | extraction only |
| Fetch successor + read outputs | `adopt_onchain_backup:12678`, `extract_output_value_and_script:13643` | — |
| Parse inputs | `reconcile_backup_tx:13699-13713` | extract `parse_tx_inputs` |
| Ownership match → index | `reconcile_backup_tx:13820-13851` | — |
| Insert change spendable | `upsert_received_utxo` (`output_repo.rs:425`, derivation `:463-471`) | — |
| Mark phantom spent | `mark_spent` (`output_repo.rs:736`) + tx-row pre-insert | — |
| Cache invalidate | `balance_cache.invalidate()` | — |

Fix A is a **composition**, not new machinery.

## 4. Guardrails the plan MUST contain (from red-team — blocking)
1. **Fail closed on every ambiguous on-chain signal** (network err / non-200 / empty /
   truncated / absence-from-`/unspent/all`) → mutate nothing. Absence is never proof
   of spend.
2. **Positive spent-proof required** — mark spent only on `/tx/{txid}/{vout}/spent`=200
   from the non-address-index endpoint, in all branches.
3. **Confirmation-depth gate** — unconfirmed successor = read-only, no marks/inserts.
4. **Resolve to the chain TIP**, not the one-hop direct spender.
5. **Change derivation only by exact address-table script match** (index ≥ 0), vout≥2
   only — never NULL/NULL/master/guessed.
6. **Atomic insert-change + mark-phantom-spent** in one lock scope, `spent_by` set,
   single cache invalidate.
7. **`proven_tx_req` for any synthetic `spent_by` tx** so Monitor can restore inputs
   if the successor never confirms / reorgs.
8. **Self-healer: idempotent, bounded, unlock-gated, runs once before Monitor backup
   tick, provable no-op on a healthy DB.**
9. **Pass the real `confirmed` flag** to the change insert (`upsert_received_utxo_with_confirmed`).
10. **Tests** for each of the above (see §6).

## 5. Phased commits (production behavior changes only at Commit 4)
1. Extract `check_outpoint_spent` (pure refactor; repoint existing adopt branch).
2. Extract `parse_tx_inputs` + confirm `extract_output_value_and_script` reuse for any vout.
3. Add `reconcile_diverged_backup_funding` (fail-closed, confirmation-gated) — unwired.
4. Wire into `do_onchain_backup` (in-handler repair + bounded single re-selection).
5. Add `monitor/task_reconcile_backup` self-healer + register in `monitor/mod.rs`.
6. Docs: update `rust-wallet/CLAUDE.md` + this plan's status; note in the incident doc.

## 6. Test plan
**Unit (in-memory sqlite, no network):** parse inputs/outputs; ownership match returns
`Some(N)` for receive-addr, `None` for foreign, and **not** a positive index for -1/-3
(guards the wrong-key trap); `check_outpoint_spent` status mapping; **fail-closed** test
(Unknown/Unspent → zero mutations — the anti-regression for the removed
`reconcile_for_derivation`); happy path (phantom→spent, real change inserted spendable
with correct derivation); non-wallet successor change → phantom marked, nothing inserted;
**unconfirmed successor → no marks** (confirmation gate).
**Integration (mocked WoC):** full `do_onchain_backup` on a diverged fixture funds from
real change and succeeds; healthy wallet → reconcile is a no-op (regression guard);
multi-hop resolves over ≤2 passes; idempotency (run twice → no changes); **WoC-down at
startup → healthy DB unchanged**.
**Manual smoke (dev, `HODOS_DEV=1`):** reproduce the diverged dev wallet (DB tip
`7c4423f4`, on-chain `ef67fd9e`@956718); confirm heal logs "phantom spent / real change
inserted"; trigger backup → broadcasts funded by `ef67fd9e:2`; relaunch → no-op.
**Parity:** all Rust/portable (no `#ifdef`); run `cargo test` + smoke on Windows and a
seeded macOS dev build.

## 7. Open decisions
- Self-healer home: Monitor task (recommended) vs pure `main.rs` startup block (only if
  same-boot repair is needed before the first Monitor tick).
- Confirmation depth: 1 vs 2–3 (trade heal-latency vs reorg safety).
- In-handler reconcile: run unconditionally on selected funding (simplest, catches
  divergence the adopt heuristic misses) vs only when the adopt branch flags staleness.
