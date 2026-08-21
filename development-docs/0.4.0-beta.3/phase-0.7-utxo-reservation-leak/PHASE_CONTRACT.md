# Phase 0.7 — a failed `createAction` must not strand the UTXOs it reserved

**Opened 2026-08-21.** Owner decision: fix in beta.3, scheduled after Phase 0.6.
Ticket: `../TICKET_createaction_strands_utxos_on_error.md` (carries the full evidence).

## 1. Goal

A `createAction` that fails for **any** reason releases the UTXOs it reserved, so the coins remain
spendable. A `createAction` that succeeds keeps them reserved exactly as it does today.

## 2. Done means

- [ ] The reservation is released by a **scope guard**, not by editing return sites. `Drop` releases
      unless something explicitly commits, so it is correct across all 19 current exits, any future
      one, and a panic.
- [ ] A `monitor/` sweeper releases `pending-%` reservations older than N minutes with no matching
      `PENDING_TRANSACTIONS` entry — the guard cannot survive a process kill, and today's incident
      would still have stranded coins across the restart.
- [ ] ⛔ The sweeper **verifies the outpoint is unspent** before releasing. A `pending-` row whose
      transaction actually broadcast under a real txid must NOT be un-spent, or the wallet will
      attempt a double-spend. This is the one way this fix can lose money, and it is the reason the
      sweeper is not a plain `UPDATE … WHERE created_at < …`.
- [ ] Unit test at the repository layer; integration test that drives a real failing `createAction`.
- [ ] Concurrency regression: two simultaneous `createAction`s must still not select the same UTXO.
      The reservation exists for that reason and a release bug that widens the race is worse than
      the leak.

## 3. Evidence table

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier |
|---|---|---|---|---|
| `P0.7-A1` | `POST /transaction/send` with a **valid-prefix / invalid-checksum** address returns 400 and the wallet balance is **unchanged afterwards** | ⛔ Pre-fix the balance DROPS by the selected UTXO and never recovers. **Already observed 2026-08-21** — three probes stranded 20,403,314 sats | `/wallet/balance` before vs after, plus `select … from outputs where spending_description like 'pending-%'` | T1 |
| `P0.7-A2` | Every other early-return path in `create_action_internal` behaves the same | ⛔ Revert the guard → each strands again | Drive at least: bad basket name, over-long output description, bad script hex | T1 |
| `P0.7-A3` | A `createAction` killed **mid-flight** (process kill between reserve and sign) has its reservation swept within N minutes | ⛔ Disable the sweeper → the reservation persists forever, which is exactly today's incident | The sweeper task, on a restarted wallet | T2 |
| `P0.7-A4` | ⛔ A `pending-` row whose tx DID broadcast is **NOT** released by the sweeper | ⛔ Stub the unspent check → the sweeper un-spends a real spend and the next send double-spends | On-chain unspent status of the outpoint | T1 |
| `P0.7-A5` | Two concurrent `createAction`s still never select the same UTXO | ⛔ Remove the reservation entirely → both select it | Concurrency harness; **design it so the correct answer is not a number the harness would report anyway** | T1 |
| `P0.7-A6` | A successful send is unchanged — no early release, no double-spend | Pre-existing behaviour; this is the regression half | `R1`-style owner send | T2 |

## 4. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-NODOUBLE` | The wallet never offers an already-spent output as spendable | 🚨 **The central risk.** A release that does not verify on-chain state converts a leak into a double-spend |
| `R-NORACE` | Two concurrent createActions never select the same UTXO | The reservation is what prevents this; releasing it too eagerly re-opens the race |
| `R-INTEXT` | Internal never prompts, external always gates | Untouched, but `create_action_internal` is on the money path — re-run the P0.5 pairing |

## 5. Blast radius

`rust-wallet/src/handlers.rs :: create_action_internal` (reservation), `sign_action` (the only
current release), `rust-wallet/src/database/output_repo.rs :: mark_multiple_spent` /
`update_spending_description_batch`, and a new `monitor/` task.

⚠️ `handlers.rs:13423` uses a second placeholder namespace, `pending-backup-{ts}`, for the on-chain
backup path. **Check whether it leaks the same way** and whether the sweeper should cover it — the
prefix differs, so a `pending-%` LIKE matches it and a `pending-` exact-prefix match may not.

## 6. Out of scope

- The `429` confirmed-only balance under-report (separate, recorded, self-heals).
- Making address validation happen *before* UTXO selection. That would shrink the window but not
  close it — 18 other returns remain — and reordering validation on the money path is its own risk.
  Note it as an option, do not do it instead of the guard.

## 7. Rollback

Single commit, revertible. The sweeper can be disabled independently of the guard.
