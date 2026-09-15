# Phase 10b — one Approve resolves one request, everywhere a call can arrive · PHASE CONTRACT

**Workstream:** critical advisories (money path) · **Source:** `../../CRITICAL_UPDATES.md` §1.1 (CU-1) + §2 (CU-8, CU-9)
**Status:** ⬜ NOT STARTED — stub from the template; the kickoff fills `D-n`, verifies every symbol, and turns each ⬜ into a run.
**Opened:** 2026-09-15 · **Owner:** Matthew Archbold · **Platforms:** C++ shared (`HttpRequestInterceptor.cpp`) + React + Rust — both platforms; Mac rebuilds and eyeballs the modal
**Standard:** `../../HARNESS.md`.

---

## 0. Plan-vs-tree delta — filled at kickoff 2026-09-15, base `b63aacf`

| # | Delta |
|---|---|
| `D-1` | **Confirmed, Rust side sound.** `state.rs :: mint_pending_payment_approval` (`:301`) and `consume_and_verify` (`:339`): single-use, body-sha256-bound, `APPROVAL_TTL_SECS`. `request_gate.rs :: dispatch_payment_with_amount`: no `X-Requesting-Domain` ⇒ `Proceed` (`:1036-1043`); the `X-User-Approved` replay arm (`:1050-1082`) consumes the token and returns `Proceed` **without re-running the cap** — by design for one request, the defect once N siblings each carry their own |
| `D-2` | **Confirmed, C++ fan-out.** `tryHandlePendingResponse` (`HttpRequestInterceptor.cpp:3103`) injects `X-User-Approved` into `headersOnApprove` for every non-connect prompt (`:3133-3137`). `openPaymentConfirmationModal` (`:1464`) — and its five kind siblings (`:1474-1530`) — do `addRequest` + `CreateNotificationOverlayTask(type, domain, extraParams)`: one entry and one modal per request, **and the overlay is never given the requestId** (query string is `type=&domain=` + extras, `simple_app.cpp:1182`; keep-alive replace by `window.showNotification(query)`, `:1217`). `handleAuthResponse` (`:3617-3660`) resolves the named entry then `popAllForDomain(domain)` and resumes every sibling; `resumeInternalResponse` (`:3229`) replays each sibling's stored `headersOnApprove` (`:3263-3278`). Legacy overload `:3666` and `getRequestIdForDomain` (`PendingAuthRequest.h:222`) iterate an `unordered_map` ⇒ the fallback picks an **arbitrary** pending entry, not the most recent |
| `D-3` | **Confirmed, React + IPC.** `simple_handler.cpp` `brc100_auth_response` arm (`:5115`) reads `responseData.value("requestId", "")` (`:5127`) and falls back to `getRequestIdForDomain(g_pendingModalDomain)` (`:5188-5197`). In `BRC100AuthOverlayRoot.tsx` **all 16** `brc100_auth_response` senders (`:862` … `:1464`) omit `requestId`; the only sender that carries one is the Chromium `permission_response` (`:1818`), which reads `params.get('requestId')` (`:576`). The payment modal reads `satoshis` / `cents` from the query (`:599-600`) — the amount shown is the amount of *whichever request posted the overlay last* |
| `D-4` | **Arrival paths enumerated (decision 3).** (1) IPC `wallet_call` (`simple_handler.cpp:2587` → `HandleIpcWalletCall` → forward worker → 202 → `tryHandlePendingResponse`, `:2296`). (2) HTTP BRC-100 surface (`AsyncHTTPClient::OnRequestComplete`, `:3718` → `:3757`). (3) BRC-121: a **separate registry** — `s_brc121_pending_approvals` url → approvalId (`:4381`), set at `:5126`; on Approve `simple_handler.cpp:5287` arms **one** URL (`MarkBrc121PaymentApproved(pendingReq.endpoint)`) and `TriggerPendingBrc121Reloads(domain)` (`:5295`) reloads every pending URL for the domain — the un-armed ones re-run the gate fresh, so BRC-121 already behaves as *queue-per-URL*; its residual defect is that `pendingReq` comes from the same requestId-or-domain-fallback, so the armed URL may not be the one the user saw. (4) Internal paymail / PeerPay: no domain header ⇒ `Proceed` (never prompts); when a **page** calls `/wallet/paymail/send` or `/wallet/peerpay/send` it arrives by (1)/(2) and joins the same 202 path. (5) ⚠️ **A fifth site not in the plan:** the `add_domain_permission` / `add_domain_permission_advanced` IPC arms drain `popAllForDomain` and resume each entry with a synthetic approve (`simple_handler.cpp:5450`, `:5528` via `ResumeDrainedApprovedRequest`, `:2916`) — a *kind* entry pending at that moment resumes with its stored token too |
| `D-5` | **CU-8 confirmed.** Engine path: `get_session_counters_snapshot` (read lock, `:1148`) → `decide` (`:1158`) → `increment_payment_rate_counter` + `record_spending` (two separate write locks, `:1167-1174`). Nothing holds a lock across snapshot-decide-record. `state.rs` has a `#[cfg(test)]` module (`:637`) and `request_gate.rs` one (`:1365`) to extend |
| `D-6` | **CU-9 confirmed, XS.** Key is `(original_url, satoshis)` (`handlers.rs:18712`, `main.rs:456`). The lookup (`:18410-18470`) sits **after** `dispatch_payment`, so a second site is still permission-gated — the leak is the unbroadcast BEEF handed across sites, as §2 says. `req.server_pubkey_hex` (`:18319`) and `X-Requesting-Domain` (`:18309`) are both in hand for the key |
| `D-7` | ⛔ **Rig trap confirmed on the tree.** `IsInternalOrigin` (`:1167`) is true for **any** `127.0.0.1` or `localhost` host, **any port**, so a loopback test dApp is never gated (`simple_handler.cpp:2534`, `:2378`, `:2455`). `OriginFromUrl` (`PortConfig.h:184`) yields `host[:port]` and the match is host-terminated ⇒ `testdapp.localhost:<port>` is **external**. Chromium resolves `*.localhost` to loopback without a hosts entry — verified at rig time; hosts-file alias is the fallback. `LegacyWalletGateMatch` (`PortConfig.h:545`) concerns the *wallet* URL, not the page origin, and is untouched |
| `D-8` | **§2a against the tree.** Option **S** has **no scaffold**: the overlay receives no ids today (`D-2`), the notification overlay is one keep-alive page replaced by injection, and a list modal re-enters Phase 7a's small-screen row (10 rows ⇒ a 2-px button strip, measured). Option **Q** has scaffold: `PendingRequestManager` already holds the queue and `addRequestIfFirstForDomain` already implements "second arrival waits" for connect prompts; missing is only "post the next kind prompt's overlay when the current resolves". Option **D** is the smallest diff. All three need `D-2`/`D-3`'s requestId plumbing first (that plumbing alone closes `P10b-A1`/`A5`; §2a decides only what happens to the *other* prompts) |

**§2a status:** the b63aacf recommendation (S with a cap, Q fallback) is buildable; the tree adds two costs not in the contract — S starts from zero id plumbing, and inherits 7a's height risk. Owner decides at hand-back (see the kickoff summary).

## 1. Goal

One click approves exactly the request whose amount the user saw; a burst that crosses a limit produces
prompts the user can understand, never N silent signatures behind one prompt; and payments within the user's
limits are as silent as they are today.

## 2. Done means

- [ ] Every approve/deny message carries the `requestId` it answers; a message without one is rejected
- [ ] `popAllForDomain` fan-out remains **only** for connect-type prompts (`domain_approval`, `brc100_auth`, `manifest_connect_bundle`), where siblings are re-issued *without* a token and re-evaluated
- [ ] Kind prompts (payment, rate-limit, protocol, certificate, key) never resume a sibling with its own token
- [ ] Superseded prompts are handled per the design decision below — the user sees the burst **once**
- [ ] The counter snapshot, decision and record happen under one write lock (CU-8)
- [ ] The 402 reuse cache key includes the requesting domain and the server key (CU-9)

## 2a. The design question — the owner's burst requirement, made concrete

👤 Owner: fix bursts everywhere; do not spam the user; still answer the question by notifying the user once.

How the auto-approve engine and a burst meet today (from `CRITICAL_UPDATES.md` §1.1, verified at kickoff):

- **Within limits, a burst is already silent and stays silent.** Five $0.50 payments under a $1 per-tx cap and
  a $10 session cap ⇒ five `Silent` decisions, five 200s, no modal. Nothing here changes that; row `P10b-A3`
  guards it. Legitimate bursts (a site buying N resources at once, a batch payer) are exactly this case.
- **The defect is the burst that *crosses* a limit.** Session cap $10, five $3 payments: three go silent, the
  fourth and fifth each return 202 and each opens a modal; the second modal *replaces* the first on screen. One
  click, and both are signed because each sibling replays its own token. The user saw one amount.
- CU-8 widens it: concurrent calls can all read the counters before any records, so more than three go silent.

Options for what the user sees when a burst crosses a limit, for the owner to pick at kickoff:

| # | Shape | User sees | Site sees | Notes |
|---|---|---|---|---|
| **Q** | **Queue** kind prompts per domain; show the next when the current resolves | one modal at a time, each with its own amount, N modals for N over-limit requests | each request waits for its own answer | kindest to legitimate batch payers; N clicks for N over-limit payments is *correct*, not spam — each is real money |
| **D** | **Deny superseded** prompts immediately with a clear error; the site retries | one modal (the latest); earlier ones get a denial | earlier requests fail fast with "superseded"; a retry goes through the engine fresh | simplest, hardest to abuse; a site that batches must retry |
| **S** | **Summarise**: one modal listing all pending requests for the domain with their amounts and a total; Approve signs all *listed*, Deny denies all | one modal per burst, with the full list | all wait for one answer | matches "tell the user once"; the approval must bind to the *set* (a fresh request arriving after the modal opened is **not** included), and the modal must be re-rendered if the set changes |

⭐ Recommendation (2026-09-15, put to the owner; kickoff confirms): **S with a cap** as the target, Q as the fallback (list up to N, deny the rest with "retry"), because it is
the only shape where "one notification per burst" and "one click never signs something the user did not see"
are both true. Q is the fallback if S's set-binding proves fragile. D is the fallback if either slips.
⛔ Whatever is chosen, the invariant is the same: **a signature exists only for an amount the user saw on the
modal that produced the click.**

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-INTEXT` | internal never prompts, external always gates | this rewrites the prompt/resume path; the T2 halves **must run**, not be code-read |
| `R-COUNT` | session counters reset on tab close by design | CU-8 changes the locking around the counters; the reset behaviour must be unchanged (CU-7's persisted ledger is *not* this phase) |
| `R-GOLD` | gold pill on every auto-approved payment | the silent path is untouched, but the resume path emits into the same `OnWalletCallSuccess`; verify the pill still fires once per approved request, on the right tab |
| `R-PERIM` | the four privacy gates | key-reveal and cert prompts share the fan-out being removed; each must still prompt and resolve singly |

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — seen, and how | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P10b-A1` | Test dApp posts **2** `createAction` calls over the per-tx cap concurrently; one Approve ⇒ exactly **1** txid, the amount shown | Pre-fix, same page, same click ⇒ **2** txids (real money, cents; `PAYMENT_TEST_BATCH.md`) | Rust audit log lines + WhatsOnChain for both txids; the modal's rendered amount captured (CDP on the notification overlay) | T2 | ⬜ **planned:** static page at `http://testdapp.localhost:8765` (`D-7`) firing two `window.CWI.createAction` of **150** and **120** cents-equivalent sats against a **$1.00** per-tx cap; Approve once via a posted click / CDP on the overlay; count `payment.user_approved` audit lines and `transactions` rows. RED run first on today's build (2 txids, recorded in the batch register); GREEN after the requestId binding |
| `P10b-A2` | The second request is either shown next (Q), listed (S) or denied (D) — never broadcast without its own visible approval | — (this is A1's second half; pre-fix it *is* broadcast) | same | T2 | ⬜ **planned:** same run as A1; the second request's fate is read from the overlay log (`consent.prompt_shown` ×2 for Q, one list for S, `superseded` error for D) and the page's second promise |
| `P10b-A3` | **Non-regression:** 5 concurrent payments all within per-tx, session and rate limits ⇒ 5 silent txids, **0** modals, 5 gold pills | Lower the per-tx cap below the amount ⇒ the same 5 now prompt (proves the row can see a modal) | audit `payment.auto_approved` ×5; overlay never shown (log) | T2 | ⬜ **planned:** same page, 5 × 30 cents-equivalent; assert 5 `payment.auto_approved`, zero `consent.prompt_shown`, 5 `payment_success_indicator` IPC lines (R-GOLD); RED = cap set to $0.10 in the site's permission row ⇒ 5 prompts |
| `P10b-A4` | **Non-regression (connect):** a fresh site fires 3 calls before connecting ⇒ **1** connect modal; after Approve all 3 are re-evaluated fresh | Pre-fix behaviour is the same (this is the model being kept); RED = stub `addRequestIfFirstForDomain` to always add ⇒ 3 modals | log of `addRequestIfFirstForDomain` decisions | T2 | ⬜ **planned:** revoke the test dApp's permission row, fire 3 `getVersion`; assert one `consent.prompt_shown type=domain_approval` and three re-issues; RED = the P0.8-A4 negative control re-run (`wasFirstForDomain` forced true) ⇒ 3 overlays |
| `P10b-A5` | Approve message **without** `requestId` ⇒ rejected, nothing resolved | Pre-fix ⇒ resolved via the domain fallback | injected message via CDP on the overlay; C++ log line | T2 | ⬜ **planned:** with one payment prompt pending, `Runtime.evaluate` on the notification overlay: `cefMessage.send('brc100_auth_response', [JSON.stringify({approved:true})])` ⇒ today the payment goes through (RED; a 1-cent payment, recorded in the batch register); after ⇒ log `brc100_auth_response without requestId — refused`, prompt still pending |
| `P10b-A6` | CU-8: 20 concurrent within-limit calls whose *sum* crosses the session cap ⇒ exactly the count that fits is silent, the rest prompt; never more silent than the cap allows | Pre-fix: more than the cap's worth go silent (race observed with a counter dump) | Rust counters read under the lock after the burst | T1 (engine test) + T2 | ⬜ **planned:** T1 in `request_gate.rs` tests: 20 threads calling the (new) `decide_and_record` with a 1000-cent cap and 90-cent calls ⇒ exactly 11 `Silent`; RED = the same test against today's three-step sequence (seen >11). T2 = the test dApp fires 20 concurrent 90-cent calls under a $10 session cap ⇒ count `payment.auto_approved` (RED today: observed >11 is the defect; if the race does not surface in 3 runs, record "not reproduced at T2, reproduced at T1") |
| `P10b-A7` | CU-9: two connected sites request the same 402 URL + sats within 25 s ⇒ the second gets its own signed BEEF (or a denial), never the first site's | Pre-fix: the same unbroadcast BEEF is handed to the second site | cache key logged; BEEF bytes compared | T1/T2 | ⬜ **planned:** T1 on the extracted key function; T2 = two `curl` calls to `/wallet/pay402` on 31401 with different `X-Requesting-Domain` (both approved rows), same `original_url` + sats ⇒ today `"reused": true` on the second (RED seen, no broadcast — pay402 never broadcasts), after ⇒ distinct txids or 403 |
| `P10b-A8` | Every arrival path (IPC, HTTP BRC-100, BRC-121 retry, internal paymail/PeerPay) is enumerated and each is shown to carry the `requestId` binding | A path left on the fallback is the RED — the enumeration must find zero | grep + one probe per path | T0/T2 | ⬜ **planned:** the `D-4` enumeration is the list (five sites, not four); T0 = `grep -c getRequestIdForDomain` in `cef-native/src` reaches the number of *deliberately kept* sites (target: only `sendAuthRequestDataToOverlay`); T2 = one probe per path from the A1/A3/A7 runs plus one BRC-121 402 page |

**Two-sided rows:** A1 (over-limit ⇒ one signature per click) and A3 (within-limit ⇒ silent) are each other's
control, and together become `R-ONE-CLICK-ONE-SPEND` in `REGRESSION_SET.md` (own commit, rule 6).

## 5. Blast radius

`HttpRequestInterceptor.cpp` — the pending-request registry and every prompt kind's resume; `simple_handler.cpp`
IPC arm for `brc100_auth_response` and siblings; `BRC100AuthOverlayRoot.tsx` (every approve/deny sender; also the
DPI/modal-height work of 7a — a list-shaped modal must survive the small-screen row); Rust `request_gate.rs`
locking; `pay_402` cache. 🍎 Shared C++ + React ⇒ relay row, Mac eyes on the modal.

## 6. Out of scope

CU-7 (persisted spend ledger, rounding cents up) — decided at kickoff, likely beta.4. The loopback caller-auth
item. Changing default limits.

## 7. Rollback

Three commits (C++/React binding + fan-out; Rust lock; Rust cache key), each independently revertible; no schema.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1 -Full` — result + date below
- [ ] `scripts/preflight.ps1 -NegativeControl` — every T0 gate seen to fail
- [ ] `../../REGRESSION_SET.md` run at this boundary — T2 halves run
- [ ] `R-ONE-CLICK-ONE-SPEND` added in its own commit after the fix
- [ ] Adversarial review — four questions in writing
- [ ] Commit messages cite the row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight -Full | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
