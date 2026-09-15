# Phase 10c — a paymail host cannot change what the user approved · PHASE CONTRACT

**Workstream:** critical advisories (money path) · **Source:** `../../CRITICAL_UPDATES.md` §1.2 (CU-2)
**Status:** ⬜ NOT STARTED — stub from the template; the kickoff fills `D-n`, verifies every symbol, and turns each ⬜ into a run.
**Opened:** 2026-09-15 · **Owner:** Matthew Archbold · **Platforms:** Rust, both (Mac rebuilds + `cargo test`)
**Standard:** `../../HARNESS.md`.

> ⚠️ Paymail is a **live** send path: `frontend/src/components/TransactionForm.tsx` resolves `$handle` /
> `user@domain` recipients and the wallet exposes `POST /wallet/paymail/send` + `GET /wallet/paymail/resolve`.
> This is not a dormant feature being hardened for later.

---

## 0. Plan-vs-tree delta — filled at kickoff

`D-1`..: re-read `handlers.rs :: paymail_send` (gate on `amount_satoshis`, the P2P destination call, the
`CreateActionOutput`s built from `o.satoshis` / `o.script_hex`, the internal `create_action` request with no
`X-Requesting-Domain`), `paymail.rs` capability discovery (`.well-known/bsvalias`, scheme handling) and the P2P
destination/receive calls, and `request_gate.rs`'s treatment of internal calls. **Read the bsvalias P2P payment
destination spec** before coding: confirm the sum-equals-request rule is the spec's intent, not our inference
(rule 4, and the `CRITICAL_UPDATES.md` fix shape says so itself).

## 1. Goal

A paymail send signs outputs that total exactly the amount the user approved, to a host reached over HTTPS,
and nothing else — whatever the recipient's host returns.

## 2. Done means

- [ ] `sum(outputs.satoshis) == amount_satoshis` or the send is rejected before signing, with an error naming the mismatch
- [ ] Output count bounded; zero/negative values rejected
- [ ] Every capability URL on the send path must be `https://`
- [ ] The resolve path (`/wallet/paymail/resolve`, the form's name/avatar preview) is unchanged in behaviour

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-INTEXT` | internal never prompts | the paymail send is an *internal* `create_action`; the fix must reject in the handler, not by suddenly routing an internal call through the external gate (that would prompt the user's own send) |
| `R-DUST` | 1-sat floor | output validation must not reject legitimate small outputs the dust rules already allow |
| `R-GOLD` | gold pill | untouched (internal sends do not pill); run at the boundary |

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — seen, and how | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P10c-A1` | Stub host returns outputs totalling **10×** the request ⇒ rejected before signing; error names the mismatch; nothing in `outputs`/`transactions` | Pre-fix, same stub ⇒ a signed transaction (in `noSend`), i.e. the wallet was ready to broadcast 10× | the stub's served body logged; the wallet's `create_action` never reached, or reached with `noSend` and the tx inspected | T2 (`noSend`) | ⬜ |
| `P10c-A2` | Stub returns the exact amount split across **3** outputs ⇒ succeeds | Raise one of the three by 1 sat ⇒ rejected | tx outputs decoded and summed by the test | T2 (`noSend`) | ⬜ |
| `P10c-A3` | Stub `.well-known` advertises an `http://` P2P endpoint ⇒ rejected | Pre-fix ⇒ used | the request the stub *did not receive* (it listens on http and sees nothing) | T2 | ⬜ |
| `P10c-A4` | Output count above the bound, or a zero/negative output ⇒ rejected | Pre-fix ⇒ accepted | unit | T1 | ⬜ |
| `P10c-A5` | **Non-regression, real:** a paymail send to a genuine handle (a few hundred sats, owner's choice of recipient) completes as before and the resolve preview still shows name/avatar | — (A1 is the control for the reject side) | txid on chain; `PAYMENT_TEST_BATCH.md` | T2 (real money) | ⬜ |

**Two-sided rows:** A1 (mismatch rejected) and A2/A5 (exact sum accepted) are each other's control.

## 5. Blast radius

`paymail_send` only, plus `paymail.rs` URL handling (also used by resolve — do not break the preview). The
follow-up in `CRITICAL_UPDATES.md` (price the *built* transaction at the gate for every internal caller) is
recorded, not done.

## 6. Out of scope

Gate-side pricing of built transactions. Paymail *receive*. Any UI change.

## 7. Rollback

One Rust commit; revert restores the pre-fix handler.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1 -Full` — result + date below
- [ ] `scripts/preflight.ps1 -NegativeControl` — every T0 gate seen to fail
- [ ] `../../REGRESSION_SET.md` run at this boundary — T2 halves run
- [ ] Adversarial review — four questions in writing
- [ ] Commit messages cite the row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight -Full | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
