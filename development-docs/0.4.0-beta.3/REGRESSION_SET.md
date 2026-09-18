# beta.3 standing regression set

Run **in full at every phase boundary**, not only in the phase that owns the code. This is what
"not breaking anything else" means concretely.

> Each check carries its own RED. A check that has never been seen to fail is not a check — see
> `HARNESS.md` §2. Where a RED is destructive, use a scratch profile, never the production one.

---

## R-INTEXT — internal never prompts, external always gates ⭐

**The load-bearing one this sprint.** Phases 0.5 and 5 both touch this boundary, and Phase 5 rewrites
the predicate every request passes through.

**The mechanism, so the check tests the right thing:** the discriminator is **not** the target — both
the wallet UI and a dApp address `127.0.0.1`. It is the **caller's frame origin**, re-derived in the
browser process at `simple_handler.cpp :: OnProcessMessageReceived` (`wallet_call` arm) from
`frame->GetURL()`, which the renderer cannot forge. Internal origin ⇒ **no** `X-Requesting-Domain` ⇒
`domain_trust_mw` (`rust-wallet/src/main.rs:65-75`) passes through ungated. External ⇒ header present
⇒ permission engine.

| | |
|---|---|
| **GREEN a** | A send initiated by the user in the wallet UI completes with **no modal** |
| **GREEN b** | The same operation from an external page, over cap, produces a **202 + modal** |
| **RED** | Each half is the other's control. Force internal-as-external (stub the origin derivation to always emit the header) → (a) must start prompting. Force external-as-internal (suppress the header) → (b) must go silent. **Both must be observed.** |
| **SUBJECT** | Rust log: **absence** of `X-Requesting-Domain` for (a), **presence with the exact page host** for (b). Reading the C++ side alone proves nothing — the assertion is what Rust received. |
| **Tier** | T2 |

⚠️ `IsInternalOrigin("")` returns **`true`** (`HttpRequestInterceptor.cpp:1016`). A frame URL with no
`://` therefore collapses to internal. Any change near origin derivation must re-check this, and it
is the one path by which external can silently become internal.

## R-GOLD — the gold pill payment indicator

| | |
|---|---|
| **GREEN** | An auto-approved payment shows the **gold pill** on the originating tab |
| **RED** | Stub `OnWalletCallSuccess`'s emit → no pill. Confirm both createAction silent-approve **and** the BRC-121 paid-retry (`firePaymentSuccessIpc()`) paths |
| **SUBJECT** | Correct **tab**: `Tab::id` ≠ `CefBrowser::GetIdentifier()` — translate via `TabManager::GetTabIdForBrowserIdentifier`. A pill on the wrong tab is a failure |
| **Tier** | T2/T3 |

It is a **gold pill**, never a "green dot". It is the user's primary visual safeguard against silent
payment abuse and must survive every refactor.

## R-CLOSE — overlay close guards

| | |
|---|---|
| **GREEN a** | Native file dialog open → no overlay closes (`g_file_dialog_active`) |
| **GREEN b** | Wallet overlay in an unsafe state (mnemonic shown, PIN entry) survives focus loss (`g_wallet_overlay_prevent_close`) |
| **RED** | Clear each flag → the overlay closes. Observe per flag, not once for both |
| **SUBJECT** | Both close paths: the overlay's own `WM_ACTIVATE` **and** the main WndProc's `WM_ACTIVATEAPP` — and, for dropdown overlays, the `WH_MOUSE_LL` hook, which is a **third** path |
| **Tier** | T3 |

⚠️ Known gap feeding Phase 1: **no** `*MouseHookProc` consults `g_file_dialog_active`. The guard is
honoured on the WndProc paths only.

## R-PERIM — the four privacy-perimeter gates

| | |
|---|---|
| **GREEN** | identity-key reveal · key-linkage reveal · sensitive cert fields · over-cap spend each behave per `matrix_c.rs` |
| **RED** | Per gate, flip its precondition and observe the opposite outcome. Sensitive cert fields must prompt **unconditionally** — if that one ever goes silent, it is a defect, not a setting |
| **SUBJECT** | The Rust decision (`PermissionDecision` kind + reason), not the UI's appearance. A modal that renders is not proof the engine decided to prompt |
| **Tier** | T1 (engine) + T2 (end-to-end) |

## R-COUNT — per-session counters

| | |
|---|---|
| **GREEN** | Per-session spend counters reset on tab close — **by design** |
| **RED** | Spend to just under the session cap, close the tab, reopen: the counter must be reset. If it persists, `POST /wallet/session/close` did not fire |
| **SUBJECT** | `PermissionService.session_counters` (`permission_service/state.rs`), not a UI total |
| **Tier** | T2 |

## R-UPDATE — the update path still applies

| | |
|---|---|
| **GREEN** | A staged update applies N−1 → N and the browser relaunches healthy |
| **RED** | Corrupt the staged installer after signing → the apply must **refuse and roll back**, not proceed |
| **SUBJECT** | The **real** N−1 → N transition, not a synthetic pair. `scripts/test-apply-forward.ps1` / `test-apply-rollback.ps1` drive our helper — they do **not** cover Sparkle or WinSparkle |
| **Tier** | T2 |

Auto-update must never force a reinstall and must never brick an install.

---

## R-DUST — no *incidental* path may spend a 1-satoshi output

> ⚠️ **Renamed 2026-09-08 by the Phase 8 adversarial review (`F1`).** It read *"no path may spend a
> 1-satoshi output"*, which was an **overclaim**: a fifth path exists and is deliberately uncovered —
> see the exclusion below. The title now matches what the GREEN row actually proves.

| | |
|---|---|
| **GREEN** | With a 1-satoshi output in the default pool, no path that selects outputs *on the user's behalf* puts it in a transaction's inputs: not the daily dust consolidator, not coin selection (either pass), not `send_max`, not the external-wallet sweep |
| **⛔ EXCLUDED, deliberately** | **`create_action`'s `user_inputs`** — a dApp naming an outpoint explicitly (`handlers.rs:5366-5381`) never reaches the selector, and the wallet **will sign it** (`sign_action`, `:7600-7740`). ⛔ Do **not** close this with a value floor: a deliberate ordinal transfer *is* a dApp naming a 1-sat outpoint, so a blanket refusal would make beta.4 sprint 2 unimplementable. ⚠️ The correct guard is the one **BRC-147 rule 2** specifies — *"a general 'pay' or auto-pay grant MUST NOT authorize spending them… enforced in the Rust permission engine"* — which does **not exist today**. ⇒ beta.4 sprint 1, tracked on `R-CLASSIFY`'s route list |
| **RED** | Remove `is_token_reserved_value` from the site under test and re-run: the candidate/selection set must be seen to **grow by exactly that output**. ⛔ Assert the **count rises**, never merely that the output is absent — an absence proves nothing if the fixture never reached the filter |
| **SUBJECT** | The **candidate or selection set itself** — `is_consolidation_candidate`, `select_utxos_greedy`, `select_all_spendable`, `split_token_reserved` — or the **serialised transaction's input outpoints**. ⛔ Never a broadcast result, a balance total, or a log line: all three are identical either way |
| **Tier** | T1 |

A 1-satoshi output is a token carrier (1Sat Ordinals, OpNS). Spending one into a larger output
**permanently destroys the asset** — BRC-147: *"a general 'pay' or auto-pay grant MUST NOT authorize
spending them."* The hazard is not hypothetical: `monitor/task_consolidate_dust` is **automatic and
daily**, and `monitor/task_sync_pending` files an incoming 1-satoshi payment into the spendable pool
within 30 seconds with no user action.

> ⚠️ **This invariant must survive beta.4, and it must survive it for a *different reason*.**
> Today it holds because of a **value floor**. beta.4 sprint 1 replaces that with classification on
> ingest. If the guard work makes `R-DUST` pass because a token never reaches the filter at all, the
> RED half stops being observable and the invariant has gone **vacuous** — re-base it onto the
> classifier rather than recording the green.

⚠️ **The floor is not a classifier.** It cannot tell a token from a stray 1-satoshi payment, and it
protects nothing at 2 satoshis or above. It is a floor beneath the selector, not a permission gate —
BRC-147's rule properly belongs in `hodos_permission_engine`, which is beta.4's work.

---

## R-ONE-CLICK-ONE-SPEND — one Approve signs exactly what it was shown

> ⭐ **Added 2026-09-15, in its own commit after Phase 10b landed** (`aaccd55` + `0b502e3` + `61b0796`), per working
> rule 6. From `CRITICAL_UPDATES.md` CU-1 and CU-8; source contract
> `phase-10-critical-advisories/10b-one-click-one-spend/PHASE_CONTRACT.md`.

The two halves are each other's control: a burst that crosses a limit must produce one signature per click, and a
burst that stays inside the limits must stay silent. A fix that satisfies one by breaking the other is the real risk.

| | |
|---|---|
| **GREEN (over-limit)** | An external site fires **2** concurrent payments over the per-transaction cap; the modal names one amount; **one** Approve ⇒ exactly **one** `X-User-Approved consumed`, **one** txid, and that txid's amount is the amount the modal showed. The other request then takes the overlay **by itself**, showing **its own** amount; it is only signed if it gets its own click |
| **GREEN (within limits)** | **5** concurrent payments inside the per-tx, session and rate limits ⇒ **5** silent txids, **0** `consent.prompt_shown`, **5** gold pills on the originating tab |
| **GREEN (connect)** | A site with no permission fires **3** calls ⇒ **1** connect modal; one Allow ⇒ all three resume (connect siblings are re-issued **without** a token and re-evaluated) |
| **GREEN (no id, no answer)** | A `brc100_auth_response` injected without a `requestId` resolves nothing and leaves the modal up |
| **GREEN (counters)** | `cargo test --lib a6_` — 20 concurrent payments against a session cap admit exactly the number that fits, repeated |
| **RED** | Measured 2026-09-15 on the pre-10b build: one Approve on a 130,000-sat modal ⇒ `Resolving 1 queued request(s)`, two approvals consumed, two broadcasts (the unseen one was 150,000 sats). Cheap re-reds that need no money: remove the requestId requirement in `simple_handler`'s `brc100_auth_response` arm (an id-less answer resolves again); revert `handleAuthResponse`'s connect-only gate (a kind sibling resumes); split `decide_and_record_payment`'s lock (`a6_one_lock…` fails `left: 20, right: 11`) |
| **SUBJECT** | The **transactions actually created** (`transactions` rows + WhatsOnChain) and the wallet's `X-User-Approved consumed` lines — never the HTTP status, never the number of resolved promises. The modal's rendered amount is read from the overlay over CDP, not assumed |
| **Tier** | T2 (real money, cents — `PAYMENT_TEST_BATCH.md`) + T1 for the counters and the 402 key |

⭐ **Rig, so this is re-runnable.** The wallet bridge is injected on **https** pages only, so a loopback test page gets
none: drive a real external https page in the dev tab and inject the helpers over CDP
(`scratchpad/p10b_lib.js`, `p10b_click.py`; the pattern is Phase 0.5's). Set the site's
`perTxLimitCents` to **1** so an over-cap payment costs cents, and pay **the dev wallet's own address** so only fees
are spent. React `element.click()` on the overlay is the approve instrument (`HUMAN_TEST_QUEUE.md` says so;
`Input.dispatchMouseEvent` is barred).

⚠️ **What this check does NOT cover:** the modal being *legible* (that is the human row `W6`), and a cross-domain
kind prompt displaced by a connect prompt (residual 5 in the 10b contract — the invariant holds, the prompt is just
invisible until it times out).

---

## R-PEERPAY-DELIVERY — a PeerPay either delivers its message or never leaves the wallet

> ⭐ **Added 2026-09-15, in its own commit after Phase 10d landed (`ed51099`)**, per working rule 6. Owner-requested
> after the live failure: a PeerPay spent the change of a 433 KB on-chain backup, its message was refused by
> MessageBox (413 > 1 MiB) twenty times, and the recipient was never told. Source:
> `phase-10-critical-advisories/10d-peerpay-delivery/PHASE_CONTRACT.md`.

Two halves, so the free one runs at **every** boundary and the real-money one is **scheduled**, not skipped.

### Half 1 — every boundary (free: nothing leaves the wallet)

| | |
|---|---|
| **GREEN** | `cargo test --release` rows green: `peerpay_selection_tests` (bundle send skips the large-parent coin; ordinary send order unchanged; large-parent coin used last, not never; backup funding smallest single sufficient; 1-sat floor on the new selector), `peerpay_message_fits_tests` (the live 1,764,588-byte token is refused; ordinary token fits; boundary equals the server's rule; `wire_body_len` equals a real serde render), `permanence_tests`, `payment_claim_block_tests`. **Plus** the dev wallet with `HODOS_DEV=1 HODOS_MESSAGEBOX_MAX_BODY_BYTES=2000`: `POST /wallet/peerpay/send` 700 sats ⇒ **422 `ERR_PEERPAY_MESSAGE_TOO_LARGE`**, spendable sats equal before and after, 0 `pending-%` reservations, no new outbox row, the built txid **404** on WhatsOnChain |
| **RED** | Per test: remove the fix it guards (threshold → never large; size check → never refuse; selector → largest first; `is_permanent` → false; rename one claim-block field) ⇒ that test FAILS while its control stays green. For the dev-wallet run: build with the call site's check disabled (`peerpay_message_fits(0, usize::MAX)`) ⇒ the same send **broadcasts** (costs one small PeerPay — run it only when the refuse path's code changed, otherwise cite the last observation) |
| **SUBJECT** | The selection set and the size decision themselves (unit); for the dev run, the **dev DB snapshot before/after** and **WhatsOnChain for the txid** — never the HTTP status alone |
| **Tier** | T1 + T2 (no money on the GREEN) |

### Half 2 — release candidate, and any phase touching coin selection, backups, PeerPay or MessageBox (real money)

| | |
|---|---|
| **GREEN** | A wallet whose real on-chain backup is **> 220 KB** runs a backup, **then** sends a PeerPay of a few hundred sats to a second wallet, **no cap override** ⇒ every input's parent is under `large_parent_bytes()`; the message delivers first try (no outbox row); the recipient's poller credits it **once** with the right amount |
| **RED** | Observed live 2026-09-15 on the pre-10d build: backup `8142e84f…` then PeerPay `3798109e…` ⇒ 433 KB parent selected, 413 ×20, recipient never told. Re-running the RED needs a pre-10d build and strands a payment — do not re-run it; cite it |
| **SUBJECT** | Recipient's `outputs` + `peerpay_received` rows; WhatsOnChain sizes of every input's parent; the sender's outbox table |
| **Tier** | T2, `PAYMENT_TEST_BATCH.md` **M10** |

⛔ **Designed not to break anything else.** Nothing oversized is ever broadcast on purpose — the failing condition is
made by *shrinking the cap* for one dev launch. The override is read only under `HODOS_DEV=1`, which a production binary
scrubs. A refused send leaves no outbox row, notice or reserved coin. Each run asserts the wallet was left clean.

⚠️ **A dev wallet cannot run Half 2 honestly without a cap override**: its backup is small (78,944 B on 2026-09-15), under
the production large-parent line. The 10d dev run used a 250 KB cap (line 25 KB) to reproduce the proportions — valid for
the mechanism, not for the production numbers, which is why Half 2 is owed at the RC on a real-size wallet.

---

## Boundary run record

| Phase boundary | Date | R-INTEXT | R-GOLD | R-CLOSE | R-PERIM | R-COUNT | R-UPDATE |
|---|---|---|---|---|---|---|---|
| 0 → 0.5 | 2026-08-18 | ⬜ deferred to P0.5 (owns this boundary) | ⬜ live app | ⬜ live app | ⬜ live app | ⬜ live app | 🟡 T1 only — manifest↔copy round-trip + RED; real N−1→N apply owed at RC |
| 0.5 → 1 | | | | | | | |
| 1 → 2 | | | | | | | |
| 2 → 3 | 2026-08-26 | 🟢 **GREEN both halves** (see run log) | ⬜ needs a real payment | ⬜ live app, human | 🟢 T1 — 73 engine tests · ⬜ T2 e2e | ⬜ needs a payment | 🟡 T1 only (6 tests); real N−1→N owed at RC |
| 3 → 4 | 2026-09-01 | 🟢 **GREEN both halves** (run log) | ⬜ needs a real payment | 🟡 **PARTIAL** — the arms this phase touched are green; file-dialog arm + a clean prevent-close pair still owed | 🟢 T1 (preflight `T1a`) · ⬜ T2 e2e | ⬜ needs a payment | 🟡 T1 only; real N−1→N owed at RC |
| 4 → 5 | 2026-09-02 | 🟢🔴 **GREEN both halves + the injected RED, both directions — first time this sprint** (run log) | ⬜ needs a real payment | ⬜ not touched by this phase | 🟢 T1 (preflight `T1a`) · ⬜ T2 e2e | ⬜ needs a payment | 🟡 T1 only; real N−1→N owed at RC | |
| **10 → next** | **2026-09-15** | 🟢🔴 **GREEN both halves, exact subject** (run log) | ⬜ needs a real payment | ⬜ subject touched by 10b's `overlay_close` arms — owed, see run log | 🟢 T1 (75 engine tests) · ⬜ T2 e2e | 🟢 T1 (`a6_` pair) · ⬜ T2 needs a payment | 🟡 T1 only; real N−1→N owed at RC | **R-DUST 🟢 (16 tests) · R-PEERPAY-DELIVERY 🟢🔴 half 1 both tiers · R-ONE-CLICK-ONE-SPEND 🟢 T1** |
| **8 → next** | **2026-09-08** | ⬜ **subject untouched** — see run log; no gating, header or origin-derivation change | ⬜ needs a real payment | ⬜ subject untouched (no C++, no overlay) | 🟢 T1 (preflight `T1a`) · ⬜ T2 e2e | ⬜ needs a payment | 🟡 T1 only; real N−1→N owed at RC | **R-DUST 🟢🔴 GREEN + RED** |
| **11 (omnibox cluster) → next** | **2026-09-18** | ⬜ **subject untouched** — no gating, no header stamping, no origin derivation; the two C++ files carry one new IPC forward that moves a URL between our own browsers | ⬜ needs a real payment | 🟡 **PARTIAL, and this cluster touched it** — see run log | 🟢 T1 (preflight `T1a`, all gates at baseline) · ⬜ T2 e2e | ⬜ needs a payment | 🟡 T1 only; real N−1→N owed at RC | **P11 items 2/3/4 🟢🔴 GREEN + RED, both observed** |
| **11 (items 5–10) → next** | **2026-09-18** | ⬜ **subject untouched** — no gating, no header stamping, no origin derivation | ⬜ needs a real payment | 🟢 **the strongest R-CLOSE evidence this sprint has** — 8 overlays × 2 windows, and a control that turns 7 of 8 red on the torn-off window; see run log | 🟢 T1 (preflight `T1a`, all gates at baseline) · ⬜ T2 e2e | ⬜ needs a payment | 🟡 T1 only; real N−1→N owed at RC | **P11-5 🟢🔴 · P11-8 🟢🔴 (both instances) · P11-9 🟢🔴 · P11-6 verified · P11-7/10 ⬜ human** |

---

## Run log — 2 → 3 boundary (2026-08-26)

Phase 2 changed wallet HTTP timeouts on the money path and moved the balance call off the UI
thread, so this boundary matters more than a docs-only one would.

### R-INTEXT — 🟢 GREEN, both halves, correct SUBJECT

⛔ **This check was NOT RUNNABLE AS WRITTEN — at this or any previous boundary.** Its SUBJECT is
*"Rust log: absence of `X-Requesting-Domain` for (a), presence with the exact page host for (b)"* —
and nothing in `domain_trust_mw` ever logged it. Fixed: one `log::debug!` in
`rust-wallet/src/main.rs :: domain_trust_mw` records path + requesting domain. DEBUG, so it never
reaches a user (the wallet ships at `warn`), and **host only** — never path or query — matching the
browser side's `LogSafeUrl` rule, because that line records which site the user is talking to.

What Rust actually received in one session:

```
51 x requesting_domain=<none:internal>     <- wallet UI, startup, peerpay polling
 1 x requesting_domain=example.com         <- path=/getVersion, from the page
```

| | Observed |
|---|---|
| **(a) internal** | `window.__hodos_walletCall('/wallet/status')` from the first-party UI -> `OK`, **no** header, **no** modal |
| **(b) external** | `window.CWI.getVersion()` from `https://example.com` -> header present carrying **exactly the page host** -> `202 PENDING` -> `domain_approval` modal, owner-observed on screen |

Same target (`127.0.0.1`), opposite outcomes, discriminated only by the frame origin the renderer
cannot forge. **Each half is the other's control**, which is what this check asks for.

⭐ The owner clicked **Block/Deny** deliberately, not Approve: approving would make `example.com` a
standing approved domain and render every future run of this check **vacuous** — the trap that made
the P0.8 bitgenius test worthless. Verified it left no residue: `🔐 Domain example.com blocked
in-memory for this session`, and **no `example.com` row in `domain_permissions`**. Matches the P0.9
standard that prompt denials are temporary.

### 🎯 Incidental: the audit log was observed firing for the first time

Phase 2 shipped `audit-<pid>.log` with its wiring proven only by unit test and code read. The
consent prompt above produced, live:

```
[2026-08-26 14:23:14.503] consent.prompt_shown | example.com | type=domain_approval
```

⚠️ The **`payment.auto_approved`** half is still unobserved — it needs a real payment.

### R-PERIM — 🟢 T1 green, ⬜ T2 owed

`cargo test -p hodos_permission_engine`: **73 tests, all passing** (40 + 33 across the two suites).
That is the Matrix C decision logic, which is this check's stated SUBJECT ("the Rust decision, not
the UI's appearance"). The end-to-end half — flipping each of the four perimeter preconditions in a
live browser — is **not** run.

### R-UPDATE — 🟡 T1 only, unchanged from the 0 -> 0.5 boundary

6 update tests green in `hodos_tests` (+1 pre-existing skip, `UpdateStagerRig.StagesFromLocalFeed`).
`scripts/test-apply-forward.ps1` / `test-apply-rollback.ps1` exist but drive a **real staged
installer**; the genuine N−1 -> N apply remains owed at RC, as recorded at the previous boundary.

⚠️ Phase 2 did **not** touch the update path, but it did change the log its operators read, and
`SilentStateWriter` was one of the files whose process tag was wrong.

### ⬜ NOT RUN — and why, stated plainly rather than left blank

| Check | Why not |
|---|---|
| **R-GOLD** | Needs a **real auto-approved payment**. Cannot be faked: the point is that the pill appears without a modal, on the correct `Tab::id`. |
| **R-CLOSE** | T3, human. Needs a native file dialog held open and the wallet overlay driven into an unsafe state, per flag, across three separate close paths. |
| **R-COUNT** | Needs spending to just under the session cap, then a tab close/reopen. |

⛔ These three most directly guard the money path, and Phase 2 changed money-path timeouts.
**They are owed, not waived.** One real payment session would close R-GOLD, R-COUNT and the
`payment.auto_approved` audit line together.

---

## Run log — 3 → 4 boundary (2026-09-01)

Run after Phase 3.5 landed its **root** fix (overlay ownership follows the requesting window,
`3237068`). ⭐ The check that mattered most at this boundary is **R-CLOSE**, because 3.5 is the first
phase to change overlay *lifetime* rather than only positioning.

### R-INTEXT — 🟢 GREEN, both halves, correct SUBJECT

📏 Driven over CDP, asserted against the **Rust** log (the decision, not the UI):

| | Observed |
|---|---|
| **(a) internal** | `window.__hodos_walletCall('/wallet/status')` from the first-party UI → `"GET /wallet/status HTTP/1.1" 200`, no gate, no 202 |
| **(b) external** | `window.CWI.getVersion()` from `https://example.com` → `🛡️ engine Prompt (domain-trust) minted approval id=a5cb040a… for domain=example.com endpoint=/getVersion type=DomainApproval reason=new_domain_no_manifest` |

⭐ (b) carries **exactly the page host**, and the decision is the engine's `Prompt`, not a rendered
modal. ⚠️ The *injected* RED (force internal-as-external and vice versa) was **not** re-run here — it
needs a code change. The two halves are each other's control per this document's own framing.

### R-CLOSE — 🟡 PARTIAL, and honestly so

✅ **Covered, on this build, and these are the parts Phase 3.5 changed:**
- 📏 `P3.5-Z3` run directly: the menu overlay was `Vis=True` and **owned by the secondary window**
  when that window was destroyed → `IsWindow(overlay)` still true, all 14 overlays present (K26).
- 📏 Product log: `Re-owned 1 overlay(s) off a closing window` — the new safety net firing in situ.
- 📏 Click-outside close path exercised for wallet, profile and tab-list (`Hiding … — lost activation
  (click-outside)`), and the guard toggles observed both ways (`close prevention ENABLED` /
  `DISABLED`).

⬜ **Still owed, unchanged from the 0→0.5 and 2→3 boundaries:**
- the **`g_file_dialog_active`** arm — needs a native file dialog held open;
- a clean **GREEN/RED pair** on `g_wallet_overlay_prevent_close`. ⚠️ Two attempts to drive it failed
  on **sequencing**, not on the product: the first set the flag 2.6 s *after* the overlay had already
  been dismissed, and the second could not reliably force the overlay's own `WM_ACTIVATE`. Recorded
  as a probe limitation. ⛔ Not claimed as passed.
- the **`WH_MOUSE_LL`** third path.

### R-PERIM — 🟢 T1, ⬜ T2 e2e owed

📏 `preflight.ps1 -Full` → `T1a cargo test - rust-wallet` **PASS**, which is where the permission
engine's unit suite lives. Unchanged from the 2→3 boundary: the end-to-end T2 arm is still owed.

### R-GOLD / R-COUNT — ⬜ NOT RUN, same reason as every prior boundary

Both need a **real auto-approved payment**. ⛔ Cannot be faked: R-GOLD's whole point is that the pill
appears with no modal, on the correct `Tab::id`; R-COUNT needs a spend to just under the session cap
followed by a tab close/reopen. One payment closes both **and** the unobserved
`payment.auto_approved` audit line.

### R-UPDATE — 🟡 T1 only, unchanged

Real N−1 → N apply still owed at RC.

### ⭐ What this boundary actually establishes

Phase 3.5 changed **overlay ownership**, so the risk it carried was to R-CLOSE. That risk was
measured directly and the overlay survived. The gaps listed above are **pre-existing and identical to
the previous two boundaries** — they are not new debt created by this phase, and none of them is
blocked on it.

---

# Boundary run: 4 → 5, 2026-09-01 — 🟡 INCOMPLETE

Run after beta.3 Phase 4 (tab context menu, overlay #15) on the landing binary.
⛔ Recorded **INCOMPLETE**, not PASS: two checks need a real payment and three need a real mouse.

### What Phase 4 actually put at risk

Three of the six checks, and it is worth being precise about which:

| Check | Why this phase touched it |
|---|---|
| **R-COUNT** | *Close other tabs* / *close to the right* close **many** tabs at once. Each `CloseTab` fires `ClearRustPaymentSessionForBrowser`. This is the **first** path that ever fired it N times in a row |
| **R-CLOSE** | A 15th overlay is a 15th close path, and a new overlay is the easiest place to reintroduce Phase 3.5's defect |
| **R-GOLD** | every action resolves a *specific* tab, and `Tab::id` ≠ `CefBrowser::GetIdentifier()` |

### R-INTEXT — 🟢 external half re-observed on this build

📏 From `https://example.com/S1`, `window.__hodos_walletCall` → Rust log:
`🛡️ engine Prompt (domain-trust) minted approval id=21de4bfd… for domain=example.com
endpoint=/getVersion type=DomainApproval reason=new_domain_no_manifest`

⭐ Carries **exactly the page host**, and the SUBJECT is the engine's decision, not a rendered modal.
⚠️ The injected RED was not re-run (it needs a code change); the internal/external halves remain each
other's control, as at the 3.5 → 4 boundary.

### R-COUNT — 🟡 PARTIAL, and this is the check Phase 4 stressed

📏 `close_right` closing **4** tabs produced exactly **4** `POST /wallet/session/close`, all 200, with
**4 distinct** `browser_id`s (12, 16, 13, 14). The surviving tabs' ids are absent.
⭐ The discriminating observation is **4, not 1** — a bulk path that cleared once (say, for the active
browser) would have produced a single POST.

⛔ **Not the whole check.** The counters were at **zero**; what is measured is that the clearing fires
once per closed tab with the right ids, **not** that a non-zero counter was reset. The value half
needs a real spend. → `phase-4-tab-peripheral-parity/MEASUREMENTS.md` M8.

### R-CLOSE — 🟡 PARTIAL, one arm newly covered, three still owed

✅ **New this boundary:** the 15th overlay was verified to take the Phase 3.5 shape rather than
reintroduce its defect — `Show*Overlay(offset, targetWin)`, positioned against the requesting
window's HWNDs, ownership handed over on show and back on hide, and **added to
`ReleaseOverlaysOwnedBy`'s list** (which enumerates its overlays by name, so a 15th absent from it
would be unprotected). 📏 `winprobe` across open **and** dismiss: the secondary window stayed at
`Z11`, above the primary at `Z12`, in all three samples (M4).

⬜ **Still owed, unchanged from the 0→0.5, 2→3 and 3.5→4 boundaries** — pre-existing, not new debt:
- the `g_file_dialog_active` arm;
- a clean GREEN/RED pair on `g_wallet_overlay_prevent_close`;
- the **`WH_MOUSE_LL`** third path — ⚠️ now including the new `TabMenuMouseHookProc`, which
  **has not been executed**: `SendInput` mouse clicks are dropped in the agent environment, so every
  Phase 4 result was driven over CDP and never reached the overlay's WndProc or its mouse hook.

### R-PERIM — 🟢 T1, ⬜ T2 e2e owed

📏 `preflight.ps1 -Full` → `T1a cargo test - rust-wallet` **PASS**. Unchanged.

### R-GOLD — ⬜ NOT RUN, same reason as every prior boundary

Needs a real auto-approved payment. ⚠️ Phase 4 raises the stakes slightly: six new actions each
resolve a specific `Tab::id`, and the gold pill is keyed on the same id. The identity translation was
verified indirectly (every action logged the right-clicked `Tab::id`, and the A2 RED showed what the
wrong one looks like), but the pill itself was not observed.

### R-UPDATE — 🟡 T1 only, unchanged. Real N−1 → N apply still owed at RC.

### ⭐ What this boundary establishes

The risk Phase 4 carried was **R-CLOSE** (a new overlay) and **R-COUNT** (bulk close). Both were
measured directly: the overlay inherits Phase 3.5's ownership handling, and the bulk close fires the
session clear once per tab with the right browser ids. The remaining gaps are the same three that
have been owed since the 0 → 0.5 boundary, plus one genuinely new one — the new overlay's mouse
path, which needs a human with a mouse and is listed as owner item **O1/O2** in the phase contract.

---

## Run log — 4 → 5 boundary (2026-09-02)

Phase 5 rewrote `GetResourceRequestHandler`'s gate — **the predicate every network request in the
browser passes through** — so this is the boundary `R-INTEXT` was written for.

### R-INTEXT — 🟢🔴 GREEN **and** RED, both directions

⭐ **The injected RED has been owed at every prior boundary** (*"not re-run; it needs a code change"*
at both 2→3 and 3→4). Phase 5 was already changing that code, so it was finally run: two one-line
stubs, built, observed, **reverted, rebuilt, and the GREEN halves re-observed** to prove the revert.

| | 🟢 GREEN (shipped code) | 🔴 RED (stubbed) |
|---|---|---|
| **(a) internal** | 7 × `requesting_domain=<none:internal>`, **0 prompts** | Always stamp in `runIpcCallDirect` ⇒ 🚨 the wallet **prompts for its own backend calls**: `engine Prompt … domain=127.0.0.1:5137 endpoint=/wallet/peerpay/status` |
| **(b) external** | `requesting_domain=example.com` ⇒ `engine Prompt … endpoint=/getVersion` | Suppress in `startAsyncHTTPRequest` ⇒ `<none:internal>`, **`200` in 20 ms with the wallet's real answer**, no 202, no modal |

🎯 **SUBJECT:** every cell read from the **Rust** log — what the wallet received — never the C++ log
or the page. Revert verified twice: `grep` for the injected marker returns 0, and `git diff` against
the fix commit is empty.

⚠️ `IsInternalOrigin("")` still returns **`true`** (`HttpRequestInterceptor.cpp:1070`) — re-checked,
unchanged, and deliberately out of scope (W6, beta.4). Detail in
`phase-5-loopback-routing/MEASUREMENTS.md` M11.

### R-GOLD / R-COUNT — ⬜ NOT RUN, and this phase raises the stakes

⛔ Both need **one real auto-approved payment**, owed at every boundary of this sprint. Phase 5
changed the predicate that decides whether a request is intercepted **at all**, and the gold pill is
emitted from inside that interception path — so a URL that stopped being intercepted would stop
producing a pill for a payment that still happens.

📏 What partially covers it: the W3 shadow log recorded **one** gate disagreement across
`example.com` + `github.com` + `youtube.com` + `en.wikipedia.org` and every subresource, and that one
was the intended `new=no old=yes` on the crafted exploit URL. Nothing that was intercepted before
stopped being intercepted. That is evidence, **not** a substitute for the payment.

### R-CLOSE / R-PERIM — unchanged

`R-PERIM` T1 green via preflight `T1a`. `R-CLOSE` not touched by this phase (no overlay lifetime
change); still carries the file-dialog arm owed from the 3→4 boundary.

---

# Boundary run: 7d close, 2026-09-08 — 🟡 PARTIAL, and specifically so

Phase 7d = the management half of the consent surface (approved-sites search, the dual-store fix,
the close guard, the grant-list cap + limits collapse).

### What Phase 7d actually put at risk

| Change | Which invariant it could break |
|---|---|
| `MirrorSitePermissionToChromium` widened to 3 types | none in this set directly — but it **changes which store governs a permission**, the "gate that silently stops gating" shape |
| Two React close guards (MUI backdrop; overlay backdrop) | `R-CLOSE` |
| One prop gating layout on a component shared with 4 consent modals | not in this set — covered by `P7d-A11` |

### `R-INTEXT` — 🟢 **GREEN, both halves, correct SUBJECT.** Owed since the 7c boundary; now run.

⛔ **First attempt read zero.** The stated subject is the Rust log's record of `X-Requesting-Domain`,
and `grep` returned **0 lines**. That is not "no header" — `domain_trust_mw` logs at **debug** and the
wallet defaults to **info**, so the subject was *suppressed, not absent*. Reading that zero as
evidence would have been the exact vacuous-probe failure this sprint keeps paying for. Re-run with
`RUST_LOG=hodos_wallet=debug`.

```
10:51:32.027989  R-INTEXT trust: path=/wallet/status  requesting_domain=<none:internal>
10:51:32.030747  R-INTEXT trust: path=/wallet/status  requesting_domain=example.com
```

- **GREEN a** — every wallet-UI call carries `<none:internal>`: `/domain/permissions/all`,
  `/wallet/settings`, `/wallet/status`, `/wallet/bsv-price`, `/wallet/balance`, `/wallet/peerpay/status`.
- **GREEN b** — the call made from a real `https://example.com` page through the injected bridge
  arrives stamped with **the exact page host**.

⭐ **A control stronger than the stub the row asks for.** The two lines above are **the same path,
3 ms apart, with opposite verdicts.** No "always emits the header" implementation and no "never
emits it" implementation can produce that pair. Both failure modes the stubbed REDs target are ruled
out for that path by a natural A/B, on one build, without touching the code.

⬜ **Still owed:** the *stubbed* REDs (force internal-as-external and observe (a) start prompting;
force external-as-internal and observe (b) go silent). The natural A/B proves the discriminator is
live; it does not prove the downstream gate reacts. Not upgraded.

⚠️ **Observation, filed not fixed:** the external `__hodos_walletCall` reached Rust in 3 ms but its
client-side promise had not settled after 10 s. The R-INTEXT claim is unaffected (the subject is what
Rust received), but the round trip on that path is worth its own look.

### `R-CLOSE` — 🟢 **subjects untouched, and that is the finding**

`git diff a052033..HEAD -- cef-native/` touches **0** lines matching `WM_ACTIVATE`,
`prevent_close`, `file_dialog_active`, `MouseHookProc` or `HideWalletOverlay`. The only C++ change in
this phase is the content-setting mirror. So `R-CLOSE`'s defined GREENs (`g_file_dialog_active`,
`g_wallet_overlay_prevent_close`) are not at risk from Phase 7d — a **code reading**, labelled as one.

⚠️ Phase 7d added **two new close guards that this invariant does not describe**, both React:
the MUI `<Dialog>` backdrop and the `edit_permissions` overlay backdrop. They are covered by
`P7d-A9`/`A10` (green, with REDs observed). 🚨 `R-CLOSE`'s SUBJECT line names three C++ paths and no
React one — which is exactly why the `edit_permissions` backdrop was missed on the first pass of item
1. **A future edit to this set should add the React layer to that SUBJECT.**

### `R-PERIM` — 🟢 T1, ⬜ T2 e2e owed — unchanged from prior boundaries
`matrix_c.rs` untouched this phase. `T1a` green in preflight.

### `R-GOLD` / `R-COUNT` — ⬜ NOT RUN, same reason as every prior boundary
No real payment happened this session. Not upgraded to a pass.

### `R-UPDATE` — 🟡 T1 only, unchanged. Real N−1 → N apply still owed at RC.

### ⭐ What this boundary establishes

The trust discriminator is **live and correctly two-sided on this build** — the first time both
halves of `R-INTEXT` have been observed together at any beta.3 boundary. It does **not** establish
that the downstream gate reacts to a forced flip; that stub remains owed before the release boundary.

---

## Run log — 8 → next boundary (2026-09-08)

Phase 8's first ticket is **Rust-only**: five files under `rust-wallet/src/`, no C++, no React, no
overlay, no IPC, no permission-engine change, no schema change. That shapes this boundary — most rows
are "subject untouched", and ⛔ **that claim is verified below rather than asserted**, because
"untouched" is the easiest way to never run anything.

### `R-DUST` — 🟢🔴 **GREEN and RED. New this boundary, and the strongest form available.**

The floor has a single point of control, so it can be disabled in **one line** that no test knows
about: `TOKEN_RESERVED_SATS: i64 = 1` → `0`.

| With the constant at `0` | Count |
|---|---|
| 🔴 tests asserting the floor — **FAILED** | **11** |
| ✅ `without_the_floor_*` controls — still green (they assert the *unguarded* behaviour) | 5 |
| ✅ `token_reserved_exposure_tests` — still green; they measure **ingest**, not the floor | 3 |
| ⚠️ `candidate_predicate_is_stable_across_repeated_evaluation` — still green | 1 |

Restored to `1`: **458 lib + 526 bin, 0 failed.**

⭐ Stronger than the per-site control run at implementation time, because it removes the **whole
feature** from one place — a test that merely re-derived the predicate would survive this, and none
did. ⚠️ The last row **cannot fail with the feature removed** and is therefore not evidence
(`ADVERSARIAL_REVIEW.md` `F2`) ⇒ **11 is the evidence count, not 19.**

### `R-INTEXT` — ⬜ subject untouched. Verified, not assumed.

The discriminator is the caller's frame origin re-derived in C++, and the gate is
`domain_trust_mw` → `hodos_permission_engine`. Phase 8 changed **none** of that: no C++ file, no
header handling, no origin derivation, no engine rule.

⚠️ **The one interaction I checked and did not assume:** Phase 8 edits `create_action`, which *is*
R-INTEXT's money path. But it changes which UTXOs become **inputs**, and the payment gate reads
**output** amounts; for `send_max` the gated figure is `resolved_amount`, computed in
`send_transaction` (`:10132`) from `calculate_balance` — which Phase 8 does not touch. ⇒ The gate
sees the same number it saw before. Downstream of the gate, upstream of nothing it reads.

### `R-GOLD` / `R-COUNT` — ⬜ NOT RUN. Same reason as every prior boundary: no real payment.

Subject untouched regardless — the pill is emitted from `HttpRequestInterceptor.cpp ::
OnWalletCallSuccess` (C++, untouched) and counters live in `PermissionService.session_counters`
(untouched).

### `R-CLOSE` — ⬜ subject untouched. No C++, no overlay, no WndProc, no NSWindow.

⭐ **Carried finding, still owed:** `R-CLOSE`'s SUBJECT names three C++ paths and **no React one**,
which is why a React backdrop discarding unsaved edits was missed on the first pass of Phase 7d
item 1. That set should grow a React layer. Unchanged this boundary.

### `R-PERIM` — 🟢 T1 (preflight `T1a`, 458+526 tests) · ⬜ T2 e2e still owed.

No permission-engine change. ⚠️ Worth stating plainly: **BRC-147 rule 2 belongs in this engine and is
not there** — a general pay grant is not stopped from spending a token carrier named as a
`user_inputs` outpoint. That is the `R-DUST` exclusion above and it is beta.4 sprint 1 work, not a
regression introduced here.

### `R-UPDATE` — 🟡 T1 only, unchanged. Real N−1 → N apply still owed at RC.

---

### ⚠️ Owed at this boundary, and honestly so

| Item | State |
|---|---|
| Live-wallet run of the floor | ⬜ **never run.** By design (contract §4), but it means "the selector excluded it" is proven and "the broadcast tx lacks it" is not |
| A send whose balance is mostly 1-sat outputs | ⬜ **not exercised.** The floor can turn a previously-successful send into `insufficient funds`; intended, but untested against a live wallet |
| `R-INTEXT` injected REDs (stub form) | ⬜ carried from 7d |
| DPI matrix cells #4/#6/#9 | ⬜ carried, neither platform |
| Mac `R4` from the 7d round | ⬜ carried |
| Independent adversarial pass | ⬜ Phase 8's review was written by the session that wrote the code |

⛔ **Production was not touched.** The owner's installed browser, wallet (`31301`, `/health` ok) and
adblock (`31302`) were running throughout; every check above ran against the build tree or the test
harness. No dev stack was started, so no port or data directory was contended.

---

# Boundary run: Phase 10 → next, 2026-09-15 — 🟡 INCOMPLETE (honestly)

Run after 10a/10b/10c/10d landed and after the four-reviewer adversarial panel
(`phase-10-critical-advisories/ADVERSARIAL_PANEL.md`) produced fixes of its own. Recorded
**INCOMPLETE**, not PASS: three checks need a real payment and one needs ten real minutes.

### What Phase 10 put at risk

| Check | Why this phase touched it |
|---|---|
| **R-INTEXT** | 10c's whole defect is that a paymail send is an **internal** `create_action`, so nothing re-priced the built transaction. The fix had to reject in the handler without routing an internal call through the external gate |
| **R-ONE-CLICK-ONE-SPEND** | 10b is its source; the panel then changed the timeout path underneath it |
| **R-PEERPAY-DELIVERY** | 10d is its source; the panel found a hole in the selection half (`F1-10d`) |
| **R-COUNT** | CU-8 rewrote how the counters are read and written |
| **R-CLOSE** | 10b added `ShowNextQueuedPrompt()` to both platforms' `overlay_close` arms |

### R-INTEXT — 🟢 GREEN, both halves, and this is the exact SUBJECT the check asks for

📏 One dev-browser session, asserted against the **Rust** side, not the UI:

```
86 x  R-INTEXT trust: path=…  requesting_domain=<none:internal>
 1 x  R-INTEXT trust: path=/getVersion  requesting_domain=example.com
```

and exactly **one** engine decision in the whole session:

```
🛡️ engine Prompt (domain-trust) minted approval id=8430eff5… for domain=example.com
   endpoint=/getVersion type=DomainApproval reason=new_domain_no_manifest
```

⭐ The discriminating observation is **86 against 1**: the wallet UI's own traffic reached Rust with
no requesting domain and produced no decision at all, while one call from `https://example.com`
carried **exactly the page host** and was gated. Same target (`127.0.0.1`), opposite outcomes,
discriminated only by the frame origin the renderer cannot forge.

⛔ The prompt was **not approved** — approving would make `example.com` a standing approved domain and
render every future run of this check vacuous (the P0.8 trap). Verified clean afterwards: **no
`example.com` row in `domain_permissions`**.
⚠️ The *injected* RED (force internal-as-external and back) was not re-run; it needs a code change.
The two halves remain each other's control, as at every prior boundary.
⚠️ `RUST_LOG=hodos_wallet=debug` puts the middleware line in the **log file**, not stderr —
`duplicate_to_stderr(Duplicate::Info)`. Reading stderr alone shows zero and means nothing.

### R-PEERPAY-DELIVERY half 1 — 🟢 GREEN, T1 **and** T2, with a working instrument control

T1: `peerpay_selection_tests` (6, including the panel's new `f1_consolidation_pass_also_skips_large_parents`),
`peerpay_message_fits_tests` (4), `permanence_tests` (1), `payment_claim_block_tests` (2) — all green.

T2, dev wallet with `HODOS_MESSAGEBOX_MAX_BODY_BYTES=2000`, `POST /wallet/peerpay/send` 700 sats:

| | Before | After |
|---|---|---|
| spendable sats | 29,087,233 | **29,087,233** |
| `peerpay_outbox` rows | 0 | **0** |
| `pending-%` reservations | 0 | **0** |
| `transactions` rows | 529 | 530 — the refused attempt, `status = failed`, `failed_at` set |

⇒ **422 `ERR_PEERPAY_MESSAGE_TOO_LARGE`** (`messageBytes: 6755`, `capBytes: 2000`), no funds moved, no
outbox row, no reserved coin. The built txid `56bb940b…f4bf` returns **404** on WhatsOnChain while the
day's accept-side control txid `48b77e68…052a` returns **200** — so the 404 is the transaction's
absence, not a broken query. That control is the point: a 404 alone proves nothing about the instrument.

⭐ Worth recording rather than glossing: a refused send **does** leave a `transactions` row, marked
`failed`. That is correct and deliberate (the activity log must show every spend attempt) and the
contract's "leaves the wallet clean" means no outbox row, no notice, no reserved coin — not no record.

### R-ONE-CLICK-ONE-SPEND — 🟢 T1; T2 was run inside 10b and is cited, not re-run

`a6_red_three_step_sequence_lets_the_whole_burst_go_silent` + `a6_one_lock_decision_never_exceeds_the_session_cap`
green; `pay402_reuse_key_tests` (4) green. The T2 halves (one click ⇒ one txid; the queued request then
showing its own amount) were measured live during 10b with their RED, and re-running them costs real
cents — cited, per this document's own rule for expensive REDs.
⚠️ **New debt from the panel:** `F4-10b` — the session cap resets when the requesting domain changes,
so `a6_one_lock…` (single domain) cannot see a page alternating between two hostnames it controls.
Pre-existing, fixed by CU-7's persisted ledger in beta.4; the multi-domain arm goes in with it.

### R-DUST — 🟢 T1, 16 tests

Including `a3_smallest_sufficient_never_takes_a_token_reserved_output`, which is the arm 10d added.
The floor still holds on both new selectors.

### R-PERIM — 🟢 T1 (75 engine tests), ⬜ T2 e2e owed — unchanged from every prior boundary

### R-CLOSE — ⬜ OWED, and this boundary **added** to it

10b posts `ShowNextQueuedPrompt()` from both platforms' `overlay_close` arms, so the close path is no
longer only a dismissal — it advances the consent queue. None of the three close paths was exercised
here (`SendInput` clicks are dropped in the agent session). ⚠️ The panel's `F2-10b` is exactly this
seam: a connect prompt displacing a shown kind prompt parks the whole queue for ten minutes. Human
row **W7** covers the money half; `F2-10b` needs an owner decision on scheduling.

### R-GOLD / R-COUNT / R-UPDATE — ⬜ NOT RUN, same reasons as every prior boundary

R-GOLD and R-COUNT need a real auto-approved payment (`PAYMENT_TEST_BATCH.md` M1/M2); R-UPDATE's real
N−1 → N apply is owed at the RC. ⛔ Owed, not waived.

### What this boundary establishes

The two invariants Phase 10 could most plausibly have broken — internal-never-prompts and
PeerPay-never-leaves-undeliverable — were measured directly, both with working controls, and both
hold. Everything still open is either pre-existing debt identical to the previous four boundaries, or
newly *recorded* debt from the adversarial panel, which is a better place for it than undiscovered.


---

## Run log — 11 (omnibox cluster) → next boundary (2026-09-18)

Phase 11 items 2, 3 and 4 changed the omnibox overlay's **lifetime** and the address bar's key
handling. Nothing on the money path, nothing on the trust boundary — which is what makes R-CLOSE the
one row that actually matters here.

### R-CLOSE — 🟡 PARTIAL, and this cluster is the reason it is listed

The omnibox is a dropdown overlay, so its close guards are R-CLOSE's **third** path: the
`WH_MOUSE_LL` click-outside hook, plus the unconditional-looking window-message hides.

**What was measured, and it is the useful half:** overlay **visibility itself**, at the Win32 layer
(`IsWindowVisible` on the real `CEFOmniboxOverlayWindow` HWND), across every dismissal path the
header owns — Enter, Escape, empty input, blur, and the overlay's own click. 🟢 All hide and **stay**
hidden; 🔴 with the fix reverted, two of them re-show themselves ~110 ms later. That is a stronger
statement about this overlay's lifetime than any prior boundary has made.

**What is owed:** ⛔ the `WH_MOUSE_LL` click-outside arm **cannot be driven from the agent session at
all** (`SendInput` clicks are dropped), and the three window-message hides (move, resize, app focus
loss) were not exercised. ⚠️ Item 4 also introduced a **stale-true** window on `omniboxOpenRef` for
exactly those three paths — documented in the phase README, cost bounded at one dead Tab press.
Human row **W9** covers the mouse and keyboard halves.

⭐ **The wallet arms of R-CLOSE were not touched** — `g_file_dialog_active` and
`g_wallet_overlay_prevent_close` are untouched by all three commits, and `HideOmniboxOverlay()` was
deliberately left with its existing signature (phase README, item 2).

### R-INTEXT / R-PERIM / R-DUST / R-ONE-CLICK-ONE-SPEND / R-PEERPAY-DELIVERY — 🟢 T1

`scripts/preflight.ps1 -Full` → **PASS**, all eight T0 gates at baseline (`G11` 59, `G12` 4, rest 0)
and all seven T1 suites green including `T1c hodos_tests` and `T1d` the frontend build. ⬜ Their T2
halves are unchanged pre-existing debt, identical to every prior boundary.

### R-GOLD / R-COUNT / R-UPDATE — ⬜ NOT RUN, same reasons as every prior boundary

⛔ Owed, not waived.

### What this boundary establishes

The change that could most plausibly have broken something here was item 4's **blur → hide**: a
suggestion click blurs the header for ~18 ms (item 3's measurement), so the dropdown could have
vanished under the mouse before `onClick` ran and broken clicking outright. That was written as an
explicit regression row (`item4check.py` R6) rather than assumed, and measured green — the click still
navigates and item 3's number is unchanged. `R7` does the same for Enter.


---

## Run log — 11 (items 5–10) → next boundary (2026-09-18)

Items 5–10 changed **overlay window ownership evidence** (none — measurement only), **Chromium
preferences** (item 9), and **cross-surface state invalidation** (item 8). Nothing on the money path
and nothing on the trust boundary.

### R-CLOSE — 🟢 GREEN, and this is the best evidence this invariant has had

Item 5 swept **8 overlays × 2 windows** — including a **torn-off** window, which no prior boundary
has tested — reading `IsWindowVisible`, `IsIconic` and `EnumWindows` z-order on the real HWNDs.
16/16: every overlay opened over the window that asked for it and that window stayed visible and in
front. 🔴 With `OwnOverlayToRequestingWindow` disabled and the shell rebuilt, **7 of 8 send the
torn-off window behind the other one and none affect the original** — the asymmetry the fix exists
to remove.

⚠️ Still owed, unchanged: the `WH_MOUSE_LL` click-outside arm cannot be driven from the agent
session, and the wallet arms (`g_file_dialog_active`, `g_wallet_overlay_prevent_close`) were not
exercised — they are also untouched by every commit in this batch.

### R-INTEXT / R-PERIM / R-DUST / R-ONE-CLICK-ONE-SPEND / R-PEERPAY-DELIVERY — 🟢 T1

`scripts/preflight.ps1 -Full` → **PASS** twice in this session, all eight T0 gates at baseline
(`G11` 59, `G12` 4, rest 0) and all seven T1 suites green. ⬜ T2 halves unchanged pre-existing debt.

⭐ **`G8` is load-bearing for item 6** and is green at 0/0: it is the gate that keeps the overlay
mouse-coordinate conversion in place, which is the fix item 6 turned out to already have.

### 🚨 New privacy finding recorded rather than buried — item 9

Chromium autofill was **live** and had recorded 6 rows of the owner's real form input to
`<profile>/Default/Web Data`, while the code claimed it was disabled. Fixed, with RED/GREEN on the
same table. ⚠️ The **installed** build carries the same defect until the next release; not read, not
touched.

### R-GOLD / R-COUNT / R-UPDATE — ⬜ NOT RUN, same reasons as every prior boundary

⛔ Owed, not waived.

### What this boundary establishes

Three fixes landed with a control each, and **three separate instrument defects were caught and
recorded** — a probe asserting the wrong subject, a probe attributing ownership from overlapping
geometry, and a probe whose editor browser navigated mid-run so the edit was never sent. ⭐ All three
produced confident REDs; none would have been caught by reading the verdict line. That is the same
family as the four farbling harnesses this harness exists for, and it is the reason every row in this
batch carries its control.
