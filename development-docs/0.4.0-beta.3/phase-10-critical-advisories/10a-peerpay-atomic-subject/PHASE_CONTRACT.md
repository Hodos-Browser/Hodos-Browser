# Phase 10a — a PeerPay credit is bound to the transaction the subject txid names · PHASE CONTRACT

**Workstream:** critical advisories (money path) · **Source:** `../../CRITICAL_UPDATES.md` §1.3 (CU-3) + §2 (CU-6)
**Status:** ⬜ NOT STARTED — stub from the template; the kickoff fills `D-n`, verifies every symbol, and turns each ⬜ into a run.
**Opened:** 2026-09-15 · **Owner:** Matthew Archbold · **Platforms:** Rust, both (Mac rebuilds + `cargo test`)
**Standard:** `../../HARNESS.md`.

---

## 0. Plan-vs-tree delta — filled at kickoff 2026-09-15, base `b63aacf`

Every cited symbol re-read on today's tree. Line numbers below are today's; symbols are what to grep.

| # | Delta |
|---|---|
| `D-1` | **Confirmed.** `beef.rs :: from_atomic_beef_bytes` (`:82`) reads the subject from the 36-byte header and hands `&bytes[36..]` to `from_bytes`; nothing ties the subject to any transaction. `main_transaction()` (`:232`) is `transactions.last()`. `from_bytes` never checks that the cursor reached the end ⇒ trailing bytes accepted. ⚠️ `from_bytes` itself also strips an Atomic header (`ATOMIC_BEEF_MARKER`, `:127`), so a plain-BEEF caller silently accepts Atomic envelopes |
| `D-2` | ⭐ **Reuse anchor:** `Beef::find_txid(txid)` (`beef.rs:297`) already computes sha256d of every transaction and returns the index of the one matching a display-format txid. The subject binding is that call plus a "none ⇒ reject" — no new hashing code |
| `D-3` | **Confirmed, poller.** `task_check_peerpay.rs`: parse `:230` → `main_transaction` `:240` → P2PKH match on the derived key `:268-284` → `check_tx_exists_on_chain(subject_txid)` `:319` → `store_derived_utxo(subject_txid, vout, sats, <last-tx script>)` `:372`. `amount` is read at `:180` and never used (grep: only the struct field and that line). ⚠️ The `Ok(false)` arm (`:325`) **broadcasts `main_tx_bytes`** — the last transaction — before storing; after the fix the broadcast must also be of the *subject* transaction |
| `D-4` | **Confirmed.** `handlers.rs :: store_derived_utxo` (`:7222`); UPDATE branch `:7270-7287` rewrites `sender_identity_key`, `derivation_prefix`, `derivation_suffix`, `custom_instructions` and sets `spendable = 1`. Exactly **two** callers: the poller (`task_check_peerpay.rs:372`) and `internalize_action` (`handlers.rs:12582`) |
| `D-5` | **Confirmed.** `check_tx_exists_on_chain` (`:6946`) asks `services.tx_status` — existence only, never outputs. Callers: poller `:319`, `internalize_action` `:12333`, and `:17418` (PeerPay outbox) |
| `D-6` | **Confirmed.** Stale promotion is `task_sync_pending.rs :: check_stale_unconfirmed` (`:388`): WoC `tx/hash/{txid}` → `confirmations > 0` → `mark_output_confirmed` (`:475`; `output_repo.rs:522`). Window `UNCONFIRMED_CHECK_SECS = 30 * 60` (`:28`). The two other `mark_output_confirmed` sites (`:221`, `:332`) are the address-sync upsert path, fed by WoC's UTXO list *for our own addresses* — a phantom row whose real on-chain script is someone else's can never match there, so only the stale path needs the output comparison |
| `D-7` | ⛔ **CU-6 is smaller than `CRITICAL_UPDATES.md` §2 says.** `internalize_action` computes `txid` from the **main (last) transaction's bytes** (`:12322-12326`), checks **that** txid on chain (`:12333`) and stores under it (`:12582`) — value, script and txid come from the same bytes, so a fabricated last transaction fails the on-chain check. What remains of CU-6: the subject mismatch is warn-only in **two** arms (`:12077` base64, `:12112` hex); plain BEEF and raw transactions are accepted where Atomic is expected; `total_received == 0` warns (`:12565`) and still returns 200 (`:12780`); basket-insertion outputs are stored with no ownership check (`:12606-12660`). It shares the parser fix and gets its own reject + non-200 |
| `D-8` | **Callers enumerated.** `from_atomic_beef_bytes`: `beef.rs:70` (base64 wrapper → `internalize_action :12062`), `handlers.rs:10555` (`extract_raw_tx_from_atomic_beef`, used by the paymail P2P submit at `:19173`), `handlers.rs:12097` (`internalize_action` hex arm), `task_check_peerpay.rs:230`. `main_transaction()`: those four plus `handlers.rs:12138/12168` (plain-BEEF arms), `identity_resolver.rs:253/342` (overlay certificate parsers — they call `Beef::from_bytes`, not the Atomic parser, and genuinely want the last transaction), `services/providers/gorillapool_mapi.rs:44` and `whatsonchain.rs:184` (provider BEEF responses). ⇒ Strictness (subject binding + trailing-byte check) goes into the **Atomic** parser only; `from_bytes` is left alone so provider and overlay parsing cannot regress |
| `D-9` | **Schedule confirmed.** `monitor/mod.rs`: `check_peerpay: 60` s (`:76`), tick 30 s (`:165`), first tick 5 s after start (`:193`), dispatch `:311`. The manual endpoint `POST /wallet/peerpay/check` (`handlers.rs:18098`) calls the same `task_check_peerpay::run`, so one fix covers both |
| `D-10` | **No test scaffold exists** for the poller (`task_check_peerpay.rs` has no `#[cfg(test)]`); parse → match → store is inline in `run` behind async chain calls. For `P10a-A1` to assert the *call*, the resolution step is extracted into a pure function (parse envelope, bind subject, find our output) shared by the poller and `internalize_action`; the RED is that function on today's logic. `ParsedTransaction` has no serializer — the tests carry a ~30-line raw-tx encoder; `Beef::new / set_main_transaction / to_atomic_beef_hex` (`beef.rs:251-634`) already build envelopes |
| `D-11` | **§2a mechanism exists.** `peerpay_received.notification_type` is `TEXT DEFAULT 'receive'` with `'failure'` already a second value (`peerpay_repo.rs:139-152`; badge splits by type at `:374`). A `'rejected'` row is **not** a schema change; the frontend badge needs one case. "Once per sender per session" = an in-memory set on the task, session = wallet process lifetime |

**§2a confirmed against the tree:** reject + audit + Activity entry + one non-blocking notification per sender is implementable with the existing notification table and no modal — recommendation stands; owner to confirm.

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
| `P10a-A1` | Unit: Atomic BEEF whose header names tx **A** (mined) and whose last tx is fabricated **B** paying our derived key ⇒ **rejected**, `store_derived_utxo` not called | Same envelope on the pre-fix code ⇒ `store_derived_utxo` called with **B's** value — observed in the test before the fix | the test constructs both transactions and asserts the *call*, not a log line; the subject hash is computed by the test independently of `beef.rs` | T1 | ⬜ **planned:** `cargo test` on the extracted resolver (`D-10`) — commit 1 extracts with today's logic and the test is RED (returns B's vout/value under A's txid); commit 2 binds the subject and the same test is GREEN |
| `P10a-A2` | Unit: valid envelope + 1 trailing byte ⇒ rejected; plain (non-Atomic) BEEF where Atomic is required ⇒ rejected | pre-fix: trailing byte accepted (`from_bytes` never checks the cursor); plain BEEF **already** rejected by `from_atomic_beef_bytes`'s magic check but **accepted by `internalize_action`**'s plain-BEEF arms | same; the plain-BEEF half is asserted at the `internalize_action` boundary | T1 | ⬜ **planned:** two unit tests; the plain-BEEF half via the extracted resolver called with `require_atomic = true` |
| `P10a-A3` | Unit: a receive for an existing `txid:vout` ⇒ refused, row byte-identical | pre-fix: UPDATE branch rewrites derivation fields and `spendable` | row compared before/after by value | T1 | ⬜ **planned:** in-memory SQLite (`WalletDatabase` test ctor, as `handlers.rs:9606` tests do): insert a row with `spendable = 0` and derivation X, call `store_derived_utxo` with derivation Y ⇒ RED today shows Y + `spendable = 1`; GREEN shows `Err` and the row unchanged |
| `P10a-A4` | Unit/T2: stale unconfirmed row whose txid is mined but whose stored value ≠ chain ⇒ **not** promoted, flagged | pre-fix: `mark_output_confirmed` called | the promotion path is driven with a stubbed chain answer whose output differs from the row | T1/T2 | ⬜ **planned:** extract the "does the chain's output match the row" decision from `check_stale_unconfirmed` into a pure function taking the fetched output (`value`, `script`) and the row; T1 on that; T2 = one dev-wallet run with a hand-inserted unconfirmed row naming a real mined txid:vout of someone else's value ⇒ log line "not promoted, mismatch" and `confirmed` still 0 |
| `P10a-A5` | **Live, two wallets:** a genuine PeerPay (a few hundred sats) from wallet B ⇒ credited once in wallet A, correct amount, `peerpay_received` once; the message `amount` cross-check passes | Send the same envelope with the message `amount` edited ⇒ rejected (the cross-check has teeth) | wallet A's `outputs` row and the MessageBox message id; both wallets on dev ports | T2 | ⬜ (real money, `PAYMENT_TEST_BATCH.md`) **planned:** Mac dev wallet → Windows dev wallet (relay ask), or the owner's second wallet; the RED half is a T1 on the resolver with `amount` ≠ output value, because editing a live MessageBox message means re-sending it |
| `P10a-A6` | `internalize_action`: subject mismatch ⇒ error; nothing credited ⇒ non-200 | pre-fix: 200 with `total_received == 0` | the HTTP status and body, not the log | T1/T2 | ⬜ **planned:** T2 with `curl` against the dev wallet on 31401: a mismatched Atomic envelope ⇒ today 200 (RED seen), after ⇒ 400 `ERR_SUBJECT_MISMATCH`; an envelope paying nobody we own ⇒ today 200 with `total_received: 0`, after ⇒ 400 |

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
