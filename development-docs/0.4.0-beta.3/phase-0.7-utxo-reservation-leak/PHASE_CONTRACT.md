# Phase 0.7 — a failed `createAction` must not strand the UTXOs it reserved

**Opened 2026-08-21.** Owner decision: fix in beta.3, scheduled after Phase 0.6.
Ticket: `../TICKET_createaction_strands_utxos_on_error.md` (carries the full evidence).

## 1. Goal

A `createAction` that fails for **any** reason releases the UTXOs it reserved, so the coins remain
spendable. A `createAction` that succeeds keeps them reserved exactly as it does today.

## 2. Done means

- [x] The reservation is released by a **scope guard**, not by editing return sites. `Drop` releases
      unless something explicitly commits, so it is correct across all 19 current exits, any future
      one, and a panic.
- [x] A `monitor/` sweeper releases `pending-%` reservations older than N minutes with no matching
      `PENDING_TRANSACTIONS` entry — the guard cannot survive a process kill, and today's incident
      would still have stranded coins across the restart.
- [x] ⛔ The sweeper **verifies the outpoint is unspent** before releasing. A `pending-` row whose
      transaction actually broadcast under a real txid must NOT be un-spent, or the wallet will
      attempt a double-spend. This is the one way this fix can lose money, and it is the reason the
      sweeper is not a plain `UPDATE … WHERE created_at < …`.
- [x] Unit test at the repository layer; integration test that drives a real failing `createAction`.
- [x] Concurrency regression: two simultaneous `createAction`s must still not select the same UTXO.
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

## 3a. Measured results — 2026-08-22 (Windows)

Implemented in `rust-wallet`; all evidence below was produced on this machine against the **dev**
wallet (31401). Every RED was driven on a real binary and *seen* to fail.

### What the pre-fix code actually did (corrections to §1–§5)

The ticket and this contract were written before the code was re-read line by line. Four claims
did not survive verification:

1. **`sign_action` is not "the only current release."** The placeholder is also resolved inside
   `create_action_internal` itself (the auto-sign path) and restored on the broadcast-failure path.
2. **A release primitive already existed** — `output_repo.rs :: restore_by_spending_description`,
   already used on broadcast failure. The guard reuses it rather than adding a new one.
3. **🚨 A sweeper already existed, in the unsafe form.** `restore_pending_placeholders()` was a
   blanket `UPDATE … WHERE spending_description LIKE 'pending-%'` with **no age filter and no
   on-chain check**, run unconditionally at every startup from `main.rs`. So A3's premise — "the
   reservation persists forever, which is exactly today's incident" — was **wrong for the restart
   case**: a restart already released stranded reservations, blindly. The real gap was *within a
   long-running process*, and the real defect was that the existing recovery could un-spend a
   broadcast transaction. That function is now deleted and replaced by the verified sweeper.
4. **`pending-backup-` does not leak.** `rollback_backup` is called at all 9 of its error returns.
   `LIKE 'pending-%'` matches it (and `pending-unpub-`), so the sweeper covers all three
   namespaces — a process kill can still strand any of them. `consolidate-` is *not* matched and is
   therefore still uncovered (noted, out of scope).

Also corrected: **two of the three A2 paths this contract suggested cannot strand at all.**
`outputDescription` is capped at 2000 bytes and validated *before* UTXO selection; basket-name
validation is likewise pre-selection. Neither ever reserves, so neither exercises the guard. A2 was
re-driven against paths proven in-window by their `🔒 Reserved` → `♻️ released` log pairing.

### Evidence

| ID | Result | Evidence |
|---|---|---|
| `P0.7-A1` | 🟢 GREEN + 🔴 RED seen | **RED (pre-fix binary):** `POST /transaction/send` to a valid-prefix/bad-checksum address → `400 Address checksum mismatch`, and `d03d8e9af60663a5:0` (**5,000,000 sats**) went to `spending_description='pending-1787411964292'`; spendable rows 43→42, 28,448,242→23,448,242 sats; `/wallet/balance` 28,446,696→23,446,696 once the 60s cache expired. **GREEN (fixed):** identical 400, balance unchanged 23,446,696→23,446,696, no new reservation; log shows `🔒 Reserved 1 UTXOs` → `♻️ createAction failed — released 1 reserved UTXO(s)`. |
| `P0.7-A2` | 🟢 GREEN ×4 | Four paths proven **in-window** (reserve-then-release in the log): address checksum (`pending-…-0`), invalid script hex (`-1`), output with neither script nor address (`-3`), invalid `customInstructions` (`-4`). Each returned its 400 and released. The gaps in the sequence numbers are the successful probe in between — evidence the per-reservation counter is working. |
| `P0.7-A3` | 🟢 GREEN | The A1 RED strand survived a **restart on the fixed binary** (proving the blanket startup restore is gone), then the periodic sweeper released it: `🧹 1 reservation(s) older than 15m — verifying on-chain before release` → `📊 Total UTXOs across all addresses: 37 … 287/287 addresses checked successfully` → `♻️ Released stale reservation d03d8e9af60663a5:0` → **5,000,000 sats returned to spendable**. Balance restored to 28,433,259. |
| `P0.7-A4` | 🟢 GREEN + 🔴 RED seen | Subject: `8012239517dc9c60:2` (1,448,668 sats), genuinely spent by `6e3fc79e…` **confirmed in block 963428**, disguised as a 1-hour-old `pending-` reservation. **GREEN (real check):** `⚠️ 8012239517dc9c60:2 (1448668 sats) is not in the on-chain unspent set — leaving reserved` → `🔒 1 reservation(s) withheld`; row untouched. **RED (unspent check stubbed to always-true):** `♻️ Released stale reservation 8012239517dc9c60:2` — the row became `spendable=1, spending_description=NULL`, i.e. the wallet was offering an output already spent in a confirmed block, and the next send would have double-spent it. Stub reverted and the row restored to `spendable=0, spending_description=6e3fc79e…, spent_by=711` immediately; no send was issued while the stubbed state existed. |
| `P0.7-A5` | 🟢 GREEN + 🔴 RED seen | Tested at the level where the race is actually prevented — `mark_multiple_spent`'s `AND spendable = 1`. Second reservation of the same outpoint claims **0** rows; the losing caller's guard frees **0** rows. **RED:** removing the predicate → second caller claims 1 and B's guard frees A's coin. Deliberately *not* "two concurrent sends picked different UTXOs", which a 43-UTXO wallet reports whether or not any reservation exists. |
| `P0.7-A6` | 🟢 GREEN | A `createAction` that succeeded mid-run signed, broadcast (`SEEN_ON_NETWORK`, later confirmed in block 963428) and resolved its placeholder to the real txid — `✅ Updated spending_description on 1 output(s): pending-1787412539… → 6e3fc79e0553acff`. No guard release fired for it. |

### Tests

- `output_repo.rs :: stale_reservation_tests` (7) + `reservation_race_tests` (2) — 9 new unit tests.
- Full suite **948 passed / 0 failed** (`cargo test --release`).
- Negative controls, each seen RED then reverted:
  - remove the age filter → `age_filter_excludes_fresh_reservations_and_includes_old_ones` fails (alone).
  - remove the placeholder scoping from the release → both R-NORACE release tests fail.
  - remove `AND spendable = 1` from `mark_multiple_spent` → both A5 tests fail.

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

✅ **Answered.** The second namespace is at `handlers.rs:13500` (not `:13423`), and it does **not**
leak: `rollback_backup` is called at all 9 of its error returns. A third namespace exists that this
contract did not know about — `pending-unpub-{ts}` (`certificate_handlers.rs:4524`). `LIKE 'pending-%'`
matches both, and the sweeper covers both, because a process kill can still strand either. The dust
consolidator's `consolidate-{ts}` is **not** matched and remains uncovered (noted, not in scope).

## 6. Out of scope

- The `429` confirmed-only balance under-report (separate, recorded, self-heals).
- Making address validation happen *before* UTXO selection. That would shrink the window but not
  close it — 18 other returns remain — and reordering validation on the money path is its own risk.
  Note it as an option, do not do it instead of the guard.

## 7. Rollback

Single commit, revertible. The sweeper can be disabled independently of the guard.
