# Phase 10 adversarial panel — 10a, 10b, 10c, 10d

**Run:** 2026-09-15, Windows, after 10c landed (`8ee4643`). **Standard:** `../HARNESS.md` §6 — a
separate pass, by something that did not write the code, whose job is to **refute**; four questions
answered in writing; fan-out earned because every sub-phase here is on the money path.

**Shape:** four independent reviewers, one per sub-phase, each given the commits, the contract, the
code, and an instruction to hunt rather than confirm. ⛔ No finding below was accepted on a
reviewer's word — every one recorded as CONFIRMED was re-read in the tree by the session before it
was acted on, and the ones that could be tested now have tests with observed REDs.

---

## What the panel was worth

It found **one shipped regression created by this very phase**, **two exploitable holes in the
guards this phase added**, and **one hole in a fix that its own test could not see because the test
called a different configuration than production does**. None of those was visible from the green
evidence tables. That is the case for the rule.

The single most useful pattern, in three of the four reviews: **a test whose subject is not the
production call site.** 10d's coin-selection tests passed `consolidation = None`; production passes
`Some(&CONSOLIDATION_FOR_SENDS)`. 10b's counter test used one domain; the cap is per-domain. 10c's
accept-side control used a stub written to echo; real hosts need not echo.

---

## Fixed in this pass

| ID | Sub-phase | What was wrong | Fix | RED observed |
|---|---|---|---|---|
| `F1-10c` | 10c | ⛔ **Regression this phase shipped.** `PaymailClient::resolve` proves a handle exists by asking for a **546-satoshi** destination. The new sum check ran on that probe, so a host that does not echo the probe amount was reported as an **invalid recipient** and could not be paid at all. BRFC `2a40af698840`'s own worked example answers a 1,000,100-satoshi request with 10,000 + 20,000, so this is documented host behaviour, not a hypothetical. The contract, `D-5`, the doc comment and the commit message all claimed the resolve path was untouched — a code reading, and it was wrong | `validate_p2p_outputs_shape` (everything except the total) split out; `p2p_destination_probe` uses it, `get_p2p_destination` keeps the full rule | ✅ `f1_probe_checks_shape_but_not_the_total` fails when the split is undone |
| `F2-10c` | 10c | The https requirement checked the URL we were **given**. `reqwest` 0.11 follows up to 10 redirects by default and permits an https→http hop unless `https_only` is set, which nothing in this wallet sets. A host could advertise `https://…` and answer `302 Location: http://…`; the check passed and the destination request travelled in cleartext for an on-path attacker to rewrite — the exact swap the check exists to prevent. Worse for `.well-known`, where a redirect to http exposes the whole capability set | `build_http_client()` with `redirect(Policy::none())`, extracted so the policy is testable rather than asserted | ✅ `f2_the_client_does_not_follow_a_redirect` drives a real socket; fails when the policy line is removed |
| `F3-10c` | 10c | A rejected P2P answer fell through to the **same host's** basic endpoint. A host that had just tried to bill 10× got a second attempt at the same payment and the user was told nothing about the first. The amount invariant survived (the basic branch hardcodes the approved amount), but the owner's rule — *"that payment must be rejected and user must be notified"* — did not | `PaymailError::HostViolation` + `is_host_violation()`; the send path answers it with 422 `ERR_PAYMAIL_HOST_REFUSED` instead of falling back. Unreachable-host errors still fall back | ✅ `f3_rule_breaches_are_marked_terminal` fails when the variant is reverted |
| `F5-10c` | 10c | `script` is host-supplied and was checked only for non-emptiness. Its length feeds fee estimation (`create_action_internal` sums `script_hex.len() / 2` and prices it at the live ARC rate), so a host honouring the satoshi total to the unit could still inflate **what leaves the wallet** by making the transaction enormous | `MAX_SCRIPT_HEX_LEN = 2_000` (1,000 bytes; a P2PKH script is 25) | ✅ `f5_script_length_is_bounded` fails when the bound is removed |
| `F7-10c` | 10c | The handler's `built_total` used `.sum()`, which panics in a debug build and wraps in release | saturating fold | — (arithmetic; the wrap direction already failed closed) |
| `F1-10d` | 10d | ⛔ **A hole in 10d's own fix, invisible to 10d's own tests.** `select_utxos_greedy`'s lazy-consolidation pass appends coins ≤ 5,000 sats **with no large-parent check**, and production passes `Some(&CONSOLIDATION_FOR_SENDS)` for every send including PeerPay. So a clean primary selection was followed by a small coin carrying the 433 KB backup parent, and the message was undeliverable again. `P10d-A3` makes such a coin *more* likely, not less: backup funding takes the smallest sufficient coin, so its change is biased small (8,057 sats on the live run; the whole 547–5,000 window is reachable). The A1 tests missed it because they pass `consolidation = None` | `if has_large_parent(utxo, parent_sizes) { continue; }` in the consolidation loop | ✅ `f1_consolidation_pass_also_skips_large_parents` fails with the line removed (`["report", "backup_change_small"]`), the other five selection tests staying green |
| `F6-10d` | 10d | Dismiss cleared the unread counts but not `hasOutboxWarnings`, so the yellow dot survived Dismiss until the next 10-second poll — a shorter version of the bug 10d exists to remove | `setHasOutboxWarnings(false)` in `handlePaymentDismissed` | — (one-line state fix; the DB half was already proven in 10d's `A5`) |
| `F1-10b` | 10b | ⛔ **Real money, and 10b created the window.** `AsyncWalletResourceHandler::handleAuthTimeout` did **not** pop the pending entry (its IPC twin `postIpcAuthTimeout` does). 10b then made prompts *wait* instead of replacing each other, and `takeNextQueuedPrompt` had no freshness test — so an entry whose transport had already returned "Approval timeout" to the page could reach the screen later, and Approve on it runs `resumeHttpCallbackResponse`, which re-issues the wallet call with `X-User-Approved` and **broadcasts**, into a response nobody is listening for | `setTimeoutRequestId` + pop in `handleAuthTimeout` (atomic, so a click a moment earlier still wins) and `ShowNextQueuedPrompt()` if it held the screen; plus `createdAt` on the entry and an age skip in `takeNextQueuedPrompt` | ⬜ **owed** — needs a live over-cap prompt left for 10 minutes. Recorded in `HUMAN_TEST_QUEUE.md` |

---

## CONFIRMED, not fixed — these need an owner decision

### 🔴 `F1-10a` / `F2-10a` — `internalizeAction`'s basket-insertion arm is ungated

**Verified in the tree by the session**, not taken on report.

`handlers.rs :: internalize_action`'s basket-insertion loop writes an `outputs` row for **any** output
index of the transaction, with **no ownership check** — only a bounds check. `output_repo.rs ::
insert_output` writes `spendable = 1`, and `calculate_balance` sums every `spendable = 1` row with no
basket filter. The loop also adds the value to `total_received` **before** the new
`ERR_NO_OUTPUTS_OWNED` gate, so that gate cannot fire for a basket insertion.

⇒ an already-approved dApp can take any mined transaction, wrap it as single-transaction Atomic BEEF,
and post it with a `basket insertion` spec. The wallet stores a stranger's output and **the displayed
balance rises by satoshis it never received**, repeatable per `txid:vout`.

Not theft: coin selection needs `derivation_prefix IS NOT NULL`, so the row is never spent. It is a
balance-integrity and ledger-integrity defect, and it also gives `F2-10a` its vehicle — a sender who
pre-registers the exact `txid:vout` it is about to pay makes the real PeerPay credit land on
`store_derived_utxo`'s "identical, no-op" arm, so the coin is counted, never spendable, and no error
is raised anywhere.

⚠️ **This is the CU-6 item the 10a contract's own `D-7` listed as remaining, and `P10a-A6` was marked
GREEN on a probe that never drove this arm.** The row should not stand as GREEN.

**Recommendation:** fix in beta.3, not beta.4 — it is small (require the output to be ours, the same
check the `wallet payment` arm already does) and the row is already claimed as done. 👤 Owner's call,
because closing it is scope the phase deliberately deferred.

### 🟠 `F2-10b` — one site can blank the consent surface for ten minutes

The three connect openers bypass `enqueuePrompt` and post their modal unconditionally, while
`takeNextQueuedPrompt` gates on a **global** "is anything shown". So: site A raises a payment prompt
(entry marked shown) → site B raises a connect prompt, which replaces A's modal on the single
keep-alive overlay → the user answers B → `ShowNextQueuedPrompt` sees A still flagged shown and
returns. A's modal is gone and never re-shown, and **every prompt from every site waits invisibly**
until A's 10-minute expiry. Same stall from `HideAllOverlays()` when the primary window closes.

Fails closed — nothing is approved — so it is denial of the consent surface, not a spend bypass. The
10b contract records it as residual 5 but describes it as one displaced prompt; it is global.

**Recommendation:** beta.3 if there is room, since it is a consent-surface outage a site can trigger;
the fix is routing the connect openers through `enqueuePrompt`, which is a design change, not a
one-liner. 👤 Owner's call.

### 🟠 `F4-10b` — the session spend cap resets when the domain changes

`state.rs :: snapshot_locked` keys counters on `browser_id` and resets the entry when the domain
changes, so `spent_cents` is per `(tab, current domain)` with no history. A page alternating between
two hostnames it controls resets the running total every switch, so the per-session dollar cap never
accumulates. Pre-existing, **not** introduced by CU-8, and `a6_one_lock_decision_never_exceeds_the_session_cap`
cannot see it because it uses one domain. CU-7's persisted ledger (already in beta.4) is the fix.

**Recommendation:** leave in beta.4 as planned; add the multi-domain arm to the test when CU-7 lands.

### 🟠 `F3-10a` — the "one notice per sender" dedupe is wrong in both directions

Keyed on sender identity only, not on reason, so a different later attack from a known sender is
log-only. And it is permanent rather than per-session: `insert_rejected_notification` is
`INSERT OR IGNORE` on `reject:{sender}` and `dismiss_all` leaves the row, so after a dismiss that
sender can never raise a visible notification again. Meanwhile any fresh key is a fresh sender, so
the attention-DoS the design was meant to prevent costs one keygen per notice.

**Recommendation:** beta.3 if F1-10a is being opened anyway (same file, same test rig); otherwise
beta.4. 👤 Owner's call.

### 🟠 `F4-10a` — stale promotion can now delete a genuine confirmed output

`get_stale_unconfirmed` defaults a NULL/empty `locking_script` to `""`, which can never equal the
chain script ⇒ mismatch ⇒ the row is deleted and a "payment failed" notice raised, for a mined coin.
Rows with empty scripts demonstrably exist — `handlers.rs` carries a repair routine whose whole job is
to find them. The old failure mode was "promote wrongly"; the new one is "delete a real coin".

**Recommendation:** beta.3, small: skip the comparison (leave the row alone) when the stored script is
empty, rather than treating absence as mismatch. 👤 Owner's call — it is a one-line change to code
this phase touched, so it is arguably already in scope.

### 🟡 `F2-10d` — the size check models the route limit, not the whole request

`wire_body_len` is exact for `message.body`, and the reviewer re-derived the arithmetic and agreed.
But the relay also runs `bodyParser.json({ limit: 1 MiB })` over the **whole** request, and our body
sits inside a wrapper of ~223 bytes. There is a ~223-byte window where we say OK and express returns
413 **after** the broadcast.

**Recommendation:** subtract a margin (≥ 256 bytes) in `peerpay_message_fits`. Small and safe; not
done here only because it changes the boundary that `a2_boundary_matches_the_server_rule` pins, and
rule 6 says the instrument and the change do not move together.

### 🟡 `F3-10d` — no row has ever seen a real claim block

`payment_claim_block_tests` pins **field names**; the one live block carried seeded derivation
strings. The production builder reads the derivation values back out of the stored payload with
`.unwrap_or("")`, so a parse failure emits a well-formed block full of empty strings. beta.5's Tools
tab is specified to parse exactly this.

**Recommendation:** fold into `PAYMENT_TEST_BATCH.md` M11's sitting — copy the block from a real
refused send and assert the identity key and non-empty derivation strings. Added there.

### 🟡 Smaller, recorded, not acted on

- `F8-10b` — one Approve triggers N BRC-121 reloads for the domain; the un-armed ones re-run the gate
  and an under-cap one auto-pays. Each is within the user's own limits and each was a navigation the
  user made, so not CU-1, but it is not in the contract.
- `F7-10b` — the `X-User-Approved` replay arm still does two separate counter writes outside
  `decide_and_record_payment`'s lock. Under-counts, so it fails safe.
- `F6-10b` — the CU-9 key's stated reason ("a NUL cannot occur in these fields") is false for two of
  three fields; the property holds only because field 1 is header-derived. Comment is wrong, code is
  fine.
- `F3-10b` — `isConnectPromptType` and `isDomainTrustPrompt` disagree about `brc100_auth`. Currently
  unreachable (`PromptType::Brc100Auth` is never constructed), one enum arm from re-opening CU-1.
  ⇒ either delete the arm or add the type to `isDomainTrustPrompt`.
- `F5-10b` — the prompt queue is unbounded and globally FIFO, so a flood delays other sites' prompts.
  With `F1-10b` fixed the HTTP path now drains, which removes the permanent version.
- `F5-10a` — the message `amount` cross-check has no adversarial teeth (the attacker writes the
  field); it catches a broken honest sender. The real security is the subject binding. The contract
  overstates it.
- `F6-10a` — `extract_raw_tx_from_atomic_beef` still reads the **last** transaction rather than the
  subject, contradicting the commit message's "every caller now agrees".
- `F9-10d` — four docs cite `main.rs :: enforce_dev_prod_isolation`; the function is
  `enforce_dev_safeguard`.

---

## Evidence-table corrections the panel forced

| Row | Correction |
|---|---|
| `P10a-A6` | ⛔ **Not GREEN.** The probe drove four envelope shapes and never a `basket insertion` output spec — the arm `D-7` listed as remaining CU-6 work, and the one that is holed |
| `P10a-A2` | The GREEN block cites `test_beef_roundtrip` as the accept-side control. That test uses `to_bytes`/`from_bytes` (V2) and never touches the strict Atomic parser. The real accept-side evidence is `A5`'s 495 KB internalize |
| `P10a-A7` | Cited in the commit message; has no row in §4 at all |
| `P10a-A5` | Its unit RED is a tautology (a test for code that did not exist). Correctly labelled as such already |
| `P10b-A6` | Header says "T1 + T2"; only T1 exists |
| `P10b-A8` | The BRC-121 arm is a code read presented as a probe; no 402 run appears in the GREEN table |
| `P10d-A1` | ⚠️ **Wrong subject** — tests pass `consolidation = None`, production passes `Some`. Fixed by `F1-10d`'s test |
| `P10d-A6` | Subject is the builder, not an emitted block |
| `P10c-A4` | Its RED is structural, not observed. Acceptable only because `A1`'s live RED shows the same absence |

---

## Where the panel found nothing

Worth recording, because these are the properties most likely to be assumed:

- **No remaining live path where one answer resolves more than one money request** (10b). Every
  resolve/pop site was enumerated and classified; connect fan-out provably carries no approval token.
- **No deadlock or lock-ordering defect** in `PendingRequestManager`; no callback runs under the lock.
- **`requestId` cannot be forged** from page content — the role gate refuses any sender but the
  notification/auth overlays — and is single-use.
- **The dev-only MessageBox cap override cannot reach production** — verified end to end through
  `enforce_dev_safeguard`.
- **PeerPay Retry cannot double-spend or double-credit.** Same bytes, same txid; only the
  notification can duplicate.
- **The refuse-before-broadcast size check measures the same bytes it later sends**, and the
  arithmetic of `wire_body_len` was independently re-derived and matched.
- **R-DUST holds on both new selectors.**
- **No paymail producer of outputs skips validation**, and the service fee is correctly excluded from
  the approved total.
- **No integer overflow in 10a's amount comparisons.**
