# Phase 10a — a PeerPay credit is bound to the transaction the subject txid names · PHASE CONTRACT

**Workstream:** critical advisories (money path) · **Source:** `../../CRITICAL_UPDATES.md` §1.3 (CU-3) + §2 (CU-6)
**Status:** ⬜ NOT STARTED — stub from the template; the kickoff fills `D-n`, verifies every symbol, and turns each ⬜ into a run.
**Opened:** 2026-09-15 · **Owner:** Matthew Archbold · **Platforms:** Rust, both (Mac rebuilds + `cargo test`)
**Standard:** `../../HARNESS.md`.

---

## 0. Plan-vs-tree delta — filled at kickoff

`D-1`..: re-read `beef.rs :: from_atomic_beef_bytes` / `main_transaction`, `task_check_peerpay.rs` (parse → find
output → on-chain check → store), `handlers.rs :: check_tx_exists_on_chain` / `store_derived_utxo` (UPDATE branch),
`task_sync_pending.rs` stale promotion, `output_repo.rs :: mark_output_confirmed`, and `internalize_action`'s
subject-mismatch branch. Confirm the message `amount` is read and unused. Confirm the poller's schedule.

## 1. Goal

A payment the wallet credits from MessageBox is the output of the transaction whose hash is the declared
subject txid — never the last transaction in the bundle — and a coin already in the table is never overwritten
by a receive.

## 2. Done means

- [ ] A fabricated envelope (real mined subject, fabricated last tx) is **rejected**, credited nothing, and logged
- [ ] A genuine PeerPay from a second wallet is credited **once**, with the amount read from the subject tx
- [ ] Stale-row promotion compares the chain's output (value + script) with the stored row before confirming
- [ ] `internalize_action` rejects a subject mismatch and returns an error when nothing was credited
- [ ] Automatic PeerPay acceptance is **still on** (owner decision — no stopgap)

## 2a. Open decision — what the user sees when a fabricated payment is rejected

👤 Owner asked 2026-09-15. Recommendation (to confirm at kickoff): **reject, record, and tell the user once —
without a modal.** Always write an audit line and an Activity entry ("rejected an invalid incoming payment
from <sender key prefix>"), and raise **one** non-blocking notification per sender identity key per session.
Why not silent: a genuine sender with a broken wallet would otherwise never learn their payment was dropped, and
a user being targeted deserves to know. Why not a modal: the inbox is writable by anyone who knows the identity
key, so a modal per fake is an attention-DoS handed to the attacker. Row `P10a-A7` then reads: fabricated envelope
⇒ rejected + one notification; ten fabricated envelopes from one sender ⇒ still one notification (RED: notify
per envelope ⇒ ten).

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-DUST` | 1-sat floor at the four paths; token outputs not destroyed | this changes what enters `outputs` on receive; a stricter parser must not drop legitimate 1-sat-adjacent rows the guard reasons about |
| `R-PERIM` | the four privacy gates | untouched, run at the boundary |
| `R-COUNT` | session counters | untouched |

## 4. Evidence table

⛔ No empty RED or SUBJECT cells. A green result is reported with its red half or not at all.

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail, and how | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P10a-A1` | Unit: Atomic BEEF whose header names tx **A** (mined) and whose last tx is fabricated **B** paying our derived key ⇒ **rejected**, `store_derived_utxo` not called | Same envelope on the pre-fix code ⇒ `store_derived_utxo` called with **B's** value — observed in the test before the fix | the test constructs both transactions and asserts the *call*, not a log line; the subject hash is computed by the test independently of `beef.rs` | T1 | ⬜ |
| `P10a-A2` | Unit: valid envelope + 1 trailing byte ⇒ rejected; plain (non-Atomic) BEEF where Atomic is required ⇒ rejected | pre-fix: both accepted | same | T1 | ⬜ |
| `P10a-A3` | Unit: a receive for an existing `txid:vout` ⇒ refused, row byte-identical | pre-fix: UPDATE branch rewrites derivation fields and `spendable` | row compared before/after by value | T1 | ⬜ |
| `P10a-A4` | Unit/T2: stale unconfirmed row whose txid is mined but whose stored value ≠ chain ⇒ **not** promoted, flagged | pre-fix: `mark_output_confirmed` called | the promotion path is driven with a stubbed chain answer whose output differs from the row | T1/T2 | ⬜ |
| `P10a-A5` | **Live, two wallets:** a genuine PeerPay (a few hundred sats) from wallet B ⇒ credited once in wallet A, correct amount, `peerpay_received` once; the message `amount` cross-check passes | Send the same envelope with the message `amount` edited ⇒ rejected (the cross-check has teeth) | wallet A's `outputs` row and the MessageBox message id; both wallets on dev ports | T2 | ⬜ (real money, `PAYMENT_TEST_BATCH.md`) |
| `P10a-A6` | `internalize_action`: subject mismatch ⇒ error; nothing credited ⇒ non-200 | pre-fix: 200 with `total_received == 0` | the HTTP status and body, not the log | T1/T2 | ⬜ |

**Two-sided rows:** A1 (reject fabricated) and A5 (accept genuine) are each other's control.

## 5. Blast radius

`beef.rs` is shared by the PeerPay poller, `internalize_action`, the identity resolver and the overlay parsers —
a stricter parser changes all of them; enumerate the callers at kickoff. `store_derived_utxo` is also reached
from the manual PeerPay check endpoint. `task_sync_pending.rs` promotion serves *every* unconfirmed row, not
only PeerPay ones.

## 6. Out of scope

The PeerPay stopgap (declined). Overlay-certificate verification (CU-5). The loopback caller-auth item.

## 7. Rollback

One Rust commit; revert restores the pre-fix parser and promotion. No schema change.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1 -Full` — result + date below
- [ ] `scripts/preflight.ps1 -NegativeControl` — every T0 gate seen to fail
- [ ] `../../REGRESSION_SET.md` run at this boundary — result recorded (T2 halves run, not skipped)
- [ ] Adversarial review — four questions in writing
- [ ] Commit messages cite the row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight -Full | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
