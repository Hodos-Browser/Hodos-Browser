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
| `P10a-A1` | Unit: Atomic BEEF whose header names tx **A** (mined) and whose last tx is fabricated **B** paying our derived key ⇒ **rejected**, `store_derived_utxo` not called | Same envelope on the pre-fix code ⇒ `store_derived_utxo` called with **B's** value — observed in the test before the fix | the test constructs both transactions and asserts the *call*, not a log line; the subject hash is computed by the test independently of `beef.rs` | T1 | ✅ **GREEN, RED seen** 2026-09-15 — RED on `57812cf`: `CREDITED 1000000 sats at <A's txid>:0`; GREEN: `NoMatchingOutput` (details below the table) |
| `P10a-A2` | Unit: valid envelope + 1 trailing byte ⇒ rejected; plain (non-Atomic) BEEF where Atomic is required ⇒ rejected | pre-fix: trailing byte accepted (`from_bytes` never checks the cursor); plain BEEF **already** rejected by `from_atomic_beef_bytes`'s magic check but **accepted by `internalize_action`**'s plain-BEEF arms | same; the plain-BEEF half is asserted at the `internalize_action` boundary | T1 (+T2 for the plain-BEEF half) | ✅ **GREEN, RED seen** — trailing byte: RED `CREDITED 700 sats`, GREEN `NotAtomicBeef("…trailing byte(s)…")`; plain BEEF: RED HTTP 200 from `/internalizeAction`, GREEN 400 `ERR_INVALID_BEEF` (the poller already refused it) |
| `P10a-A3` | Unit: a receive for an existing `txid:vout` ⇒ refused, row byte-identical | pre-fix: UPDATE branch rewrites derivation fields and `spendable` | row compared before/after by value | T1 | ✅ **GREEN, RED seen** — RED (fix stashed): `overwrite accepted: Ok(())`; GREEN: `Err("…refusing to overwrite")`, row byte-identical, identical re-delivery a no-op |
| `P10a-A4` | Unit/T2: stale unconfirmed row whose txid is mined but whose stored value ≠ chain ⇒ **not** promoted, flagged | pre-fix: `mark_output_confirmed` called | the promotion path is driven with a stubbed chain answer whose output differs from the row | T1/T2 | ✅ **GREEN, RED seen** — T2 RED (pre-fix wallet): phantom `2cf90ef4…:1` value+1 ⇒ `Marked output … as confirmed`, `confirmed=1`; T2 GREEN (fixed wallet): `NOT promoted`, row deleted, red notification; T1 `chain_output_matches` ×4 |
| `P10a-A5` | **Live, two wallets:** a genuine PeerPay (a few hundred sats) from wallet B ⇒ credited once in wallet A, correct amount, `peerpay_received` once; the message `amount` cross-check passes | Send the same envelope with the message `amount` edited ⇒ rejected (the cross-check has teeth) | wallet A's `outputs` row and the MessageBox message id; both wallets on dev ports | T2 | 🟡 **INCOMPLETE — poller half OWED** (`PAYMENT_TEST_BATCH.md` M9). Real send made 2026-09-15 (`3798109e…4ab55`, 613,685 sats) but the sender's MessageBox message is 1.76 MB and rejected with 413 (ticket filed); credited **once, correct amount, genuine 495 KB envelope** via `/internalizeAction` instead — the accept-side control holds. Amount cross-check teeth: T1 |
| `P10a-A6` | `internalize_action`: subject mismatch ⇒ error; nothing credited ⇒ non-200 | pre-fix: 200 with `total_received == 0` | the HTTP status and body, not the log | T1/T2 | ✅ **GREEN, RED seen** — T2 probe, pre-fix: stranger's mined tx ⇒ 200 `unconfirmed` (+ a `transactions` row); mismatch ⇒ warn-only then the fabricated bytes sent to WoC (400 `ERR_BROADCAST_FAILED`). Fixed: 400 `ERR_NO_OUTPUTS_OWNED` / `ERR_SUBJECT_MISMATCH` (no broadcast) / `ERR_INVALID_BEEF`; nothing written |

**Two-sided rows:** A1 (reject fabricated) and A5 (accept genuine) are each other's control.

### RED observed — 2026-09-15, on `57812cf` (extraction only, defect intact)

`cargo test --release task_check_peerpay`, tests written against the correct behaviour, run on today's logic:

```
a1_credit_is_read_from_the_subject_transaction_not_the_last_one ... FAILED
  subject A pays us nothing, yet: CREDITED 1000000 sats at fb91db3f…3b1549:0   ← A's txid, B's value
a1_subject_absent_from_the_bundle_is_rejected ... FAILED
  subject C is not in the bundle, yet: CREDITED 1000000 sats at a8209218…b1808a:0
a2_trailing_byte_after_the_bundle_is_rejected ... FAILED
  trailing byte accepted: CREDITED 700 sats at 249ff285…ba255a:0
genuine_envelope_is_credited_with_the_subject_value ... ok          ← the control passes pre-fix too
a5_declared_amount_mismatch_is_rejected ... ok                       ← new check, not a pre-existing behaviour
test result: FAILED. 2 passed; 3 failed
```

The first line is CU-3 exactly: the value of the fabricated last transaction, filed under the mined subject's txid.

`P10a-A6` RED, T2 — pre-fix dev wallet (`57812cf` binary, `HODOS_DEV=1`, 31401), `scratchpad/p10a_a6_probe.py`
posting envelopes built from one real mined transaction `2cf90ef4…4186` (block 966893, a stranger's) with no
`X-Requesting-Domain`:

```
self_consistent   HTTP 200  {"txid":"2cf90ef4…4186","status":"unconfirmed"}   ← log: "Total received: 0 … No outputs belong to our wallet!"
subject_mismatch  HTTP 400  ERR_BROADCAST_FAILED … provider whatsonchain returned status 400   ← log: "⚠️ Subject TXID mismatch: expected 2cf9…, got 3ebe…" then the FABRICATED bytes were sent to WoC
trailing_byte     HTTP 200  (accepted)
plain_beef        HTTP 200  (accepted where BRC-100 says tx is AtomicBEEF)
```

Side effect to undo after the fix: the three 200s each wrote a `transactions` row (`id 788`, status `unproven`, 0 sats) into the dev DB.

`P10a-A4` RED, T2 — same pre-fix dev wallet. `scratchpad/p10a_a4_phantom.py insert` wrote an `outputs` row for the
same mined `2cf90ef4…4186:1` with `satoshis = 99637620` (chain: **99637619**), `confirmed = 0`, `created_at` two hours
old. On the next `TaskSyncPending` tick after the startup sweep:

```
INFO task_sync_pending    ✅ Stale tx 2cf90ef4ac84cfe7... is now confirmed (4 confirmations)
INFO output_repo          ✅ Marked output 2cf90ef4ac84cfe7:1 as confirmed
inspect → ('2cf90ef4…4186', 1, 99637620, confirmed=1, spendable=1)
```

A row whose value the chain contradicts was promoted into the coin-selectable set on the strength of the txid alone. Row deleted immediately after (`cleanup`).

`P10a-A3` RED, T1 — `git stash push -- rust-wallet/src/handlers.rs` (fix removed, test kept), `cargo test a3_receive_for_an_existing_output`:

```
store_derived_utxo_tests::a3_receive_for_an_existing_output_never_rewrites_the_row ... FAILED
  overwrite accepted: Ok(())      ← a second message for the same txid:vout with a different sender/derivation/value returned Ok and rewrote the row
```

### GREEN — same tests, fix in place (`cargo test --release`, bin target **546 passed, 0 failed**; lib 468)

`a1_credit_is_read_from_the_subject_transaction_not_the_last_one` ⇒ `NoMatchingOutput` · `a1_subject_absent_from_the_bundle_is_rejected` ⇒ `NotAtomicBeef("…not a transaction in the bundle")` · `a2_trailing_byte_after_the_bundle_is_rejected` ⇒ `NotAtomicBeef("…trailing byte(s)…")` · `genuine_envelope_is_credited_with_the_subject_value` (control, still ok) · `a5_declared_amount_mismatch_is_rejected` · `a3_receive_for_an_existing_output_never_rewrites_the_row` (Err + row byte-identical; identical re-delivery is a no-op) · `stale_promotion_tests::{a4_value_mismatch_is_not_a_match, a4_script_mismatch_is_not_a_match, a4_missing_vout_is_not_a_match, genuine_row_matches}`. The pre-existing `beef.rs :: test_beef_roundtrip` (real BRC-62 transactions through `to_atomic_beef_hex`) still passes, so the strict parser accepts a genuine envelope.

`P10a-A6` GREEN, T2 — fixed dev wallet binary, same probe, same envelopes:

```
self_consistent   HTTP 400  ERR_NO_OUTPUTS_OWNED   "No output of this transaction belongs to this wallet"
subject_mismatch  HTTP 400  ERR_SUBJECT_MISMATCH   — and the log shows NO broadcast attempt between the reject and the next request
trailing_byte     HTTP 400  ERR_INVALID_BEEF       "Atomic BEEF has 1 trailing byte(s) after the bundle"
plain_beef        HTTP 400  ERR_INVALID_BEEF       "tx must be Atomic BEEF (BRC-95 header + BEEF bundle)"
```

Nothing was written to `transactions` by any of the four (checked by count before/after).

`P10a-A4` GREEN, T2 — same phantom row re-inserted (`satoshis 99637620`, chain 99637619), fixed dev wallet, next stale tick:

```
INFO  ✅ Stale tx 2cf90ef4ac84cfe7... is now confirmed (5 confirmations)
WARN  🚫 Stale output 2cf90ef4ac84cfe7:1 — txid is mined but the chain's output differs from the stored row (99637620 sats) — NOT promoted
WARN  🔴 Unconfirmed output 2cf90ef4ac84cfe7:1 (99637620 sats) confirmed failed — …
inspect → outputs row gone; notification ('fail:2cf90ef4…:1', 'failure', 99637620)
```

The phantom took the same path as a dropped transaction: deleted, red notification, never coin-selectable. Notification row cleaned up after.

`P10a-A5` — **INCOMPLETE for the poller path, owed; the credit itself is GREEN through `internalize_action`.** 2026-09-15 the
owner sent **613,685 sats** (≈ $0.10) from the installed Windows wallet to the dev wallet's identity key
`020b9558…521e`: txid `3798109e5612bc7e8bc1ebffc02d5c60a0b6e0d96ac84bf916542a3157d4ab55`, on chain (vout 0 → the BRC-29
derived address, 1,000-sat treasury fee, change). The dev poller polled `payment_inbox` 26+ times and saw **0 messages**,
because the **sender's** MessageBox delivery fails every attempt:

```
sendMessage failed (413): ERR_MESSAGE_BODY_TOO_LARGE "Message bodies must not exceed 1048576 bytes."   (installed wallet log, outbox attempts 1–4+)
```

The queued payload is **1,764,588 bytes**: a 494,725-byte Atomic BEEF serialised as a **JSON array of integers** (~3.6×
inflation; base64 would be ~660 KB and would fit). Our tree does the same (`handlers.rs :: peerpay_send`,
`"transaction": tx_array`) — filed as `../../TICKET_peerpay_message_exceeds_messagebox_limit.md`. Not a 10a defect and not
fixed here; it means **no PeerPay from this sender can reach any recipient** until it is.

Recovery (real money, dev wallet): the sender's queued remittance (`derivationPrefix`/`Suffix`, read-only from the installed
DB) plus the BEEF were POSTed to the fixed dev wallet's `/internalizeAction` as a `wallet payment` remittance ⇒ **HTTP 200**,
`outputs` row `3798109e…:0 = 613,685 sats`, sender `029dce5a…`, derivation `2-3241645161d8 / <prefix> <suffix>`, balance
+613,685. So the strict parser and the subject binding accepted a **genuine 495 KB envelope from a real wallet** and credited
it **once with the correct amount** — the accept-side control A1 needed, through the second of the two callers. The poller
half of A5 (message `amount` cross-check live, `peerpay_received` once) is owed: either 🍎 Mac's wallet as sender (small
ancestry) or this sender after the ticket's fix; the amount cross-check's teeth are shown at T1 (`a5_declared_amount_mismatch_is_rejected`).

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
| preflight -Full | **PASS** — all checks ran and passed (T0 gates at baseline, T1a/T1b/T1c, T1d frontend build) | 2026-09-15 | Windows |
| preflight -NegativeControl | n/a — no gate pattern or baseline touched by 10a (rule 6) | 2026-09-15 | Windows |
| regression set | ⬜ runs at the Phase 10 boundary (after 10c), T2 halves included | | |
| adversarial review | ⬜ one panel over 10a+10b+10c after all three land (`HARNESS.md` §6) | | |

**Status 2026-09-15:** 10a landed on Windows as two Rust commits (`57812cf` extraction; fix commit follows). Rows A1–A4, A6 GREEN with RED seen; A5 poller half owed (`PAYMENT_TEST_BATCH.md` M9, sender ticket). 🍎 Mac: rebuild + `cargo test`; A5 as sender when the relay asks.
