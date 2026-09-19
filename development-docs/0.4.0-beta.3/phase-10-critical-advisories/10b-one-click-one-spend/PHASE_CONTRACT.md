# Phase 10b — one Approve resolves one request, everywhere a call can arrive · PHASE CONTRACT

**Workstream:** critical advisories (money path) · **Source:** `../../CRITICAL_UPDATES.md` §1.1 (CU-1) + §2 (CU-8, CU-9)
**Status:** ✅ **LANDED on Windows 2026-09-15** — every row GREEN with its RED observed; `P10b-A7`'s T2 half deliberately not run (recorded below). Kickoff deltas `D-1`..`D-8` verified on `b63aacf`.
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

- [x] Every approve/deny message carries the `requestId` it answers; a message without one is rejected
- [x] `popAllForDomain` fan-out remains **only** for connect-type prompts (`domain_approval`, `brc100_auth`, `manifest_connect_bundle`), where siblings are re-issued *without* a token and re-evaluated
- [x] Kind prompts (payment, rate-limit, protocol, certificate, key) never resume a sibling with its own token
- [x] Superseded prompts are handled per the design decision below — the user sees the burst **once**
- [x] The counter snapshot, decision and record happen under one write lock (CU-8)
- [x] The 402 reuse cache key includes the requesting domain and the server key (CU-9)

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

### RED observed — `P10b-A1` / `A2`, 2026-09-15, pre-fix build (dev browser `HodosBrowser.exe` of 2026-09-14, dev wallet 31401)

**Rig (deviation from the kickoff plan, recorded):** the wallet bridge and `window.CWI` are injected on **https**
pages only (`simple_render_process_handler.cpp`, "Secure context (https://) only"), so `http://testdapp.localhost` gets
no bridge (measured: `typeof window.__hodos_walletCall === "undefined"`). Instead of installing a local CA, the Phase 0.5
method was used: a real external https page (`https://example.com/`) in the dev tab, rig helpers injected over CDP
(`scratchpad/p10b_lib.js`), approve clicked with React `element.click()` on the notification overlay
(`scratchpad/p10b_click.py`). Connect prompt approved through the real modal; then `example.com`'s row set to
`perTxLimitCents: 1` so the test payments are over the cap but pay the dev wallet's **own** address (cost = fees).

```
fireBurst(<dev address>, [150000, 130000])         both calls concurrent, over the 1-cent cap
wallet  16:34:01.261  engine Prompt (payment) minted approval id=7c27a19b… reason=per_tx_limit
wallet  16:34:01.263  engine Prompt (payment) minted approval id=91c69eba… reason=per_tx_limit
modal   "example.com is requesting a payment  $0.02  130.000k sats … Deny  Modify Limits  Approve"   ← ONE amount shown
click   Approve ×1
C++     16:34:20.895  brc100_auth_response … User approved auth request
C++     16:34:20.895  Resolving 1 queued request(s) for domain: example.com (approved)
C++     16:34:20.895  kInternal resume for queued sibling req-378548071-3 endpoint=/createAction
wallet  16:34:20.896  X-User-Approved consumed (payment) … id=7c27a1…   + "skipping spending-limit defense-in-depth"
wallet  16:34:22.886  X-User-Approved consumed (payment) … id=91c69e…   + "skipping spending-limit defense-in-depth"
tx      97feb0f8ee1c64c87c5acc34dba845f33a74879555006e2a841ae228ec04b3ee   150,000 sats   ← NEVER SHOWN
tx      a19fa3d9b76f1e155b4fec7c48b01f57127a89b5ad7b42205c63c5c6aa21bd7a   130,000 sats   ← the amount shown
```

One click, two broadcasts, one of them an amount the user never saw. CU-1 as written, on today's tree.

### GREEN — the fix, measured 2026-09-15 on the rebuilt dev browser + dev wallet

**What changed (design: 👤 owner's Queue, §2a).** Every prompt that owns a modal is registered with the
overlay type and its extras; the post carries its `requestId` (and `queuedFromSite`). A prompt that arrives while
another is on screen **waits** instead of replacing it, and the next is posted when the shown one is answered
(`overlay_close`, both platforms) or times out. The answer must name the request it answers — a
`brc100_auth_response` without a `requestId` is refused and resolves nothing. Sibling fan-out survives **only**
for connect prompts (`popConnectForDomain`), including the fifth site found at kickoff
(`add_domain_permission` / `_advanced`). The payment and rate-limit modals show "1 of N requests from this site"
when others wait.

| Row | GREEN run | RED |
|---|---|---|
| `P10b-A1` | Same burst, same amounts, same click: modal showed **150.000k sats**; Approve ×1 ⇒ **one** `X-User-Approved consumed`, **one** transaction `734f1dd57d97…` (150,000), **no** sibling-resume line. C++: `⏳ payment_confirmation for example.com queued behind the prompt on screen (req-379257944-2)` | pre-fix, above: 2 consumed, 2 txids, one amount shown |
| `P10b-A2` | After that click the queued request took the overlay by itself — `⏭️ Showing next queued prompt payment_confirmation … (req-379257944-2, 0 more waiting)` — modal showed **130.000k sats**, its own amount. **Deny** ⇒ site got `createAction failed: User rejected authentication`, still exactly one transaction, balance moved only by the approved one | pre-fix it was broadcast with no second click (the 150,000 above) |
| `P10b-A3` | Cap restored to $1.00; **5 concurrent** payments (20k–24k sats) ⇒ **5** `payment.auto_approved` audit lines, **0** `consent.prompt_shown`, **5** `💰 OnWalletCallSuccess … cefBrowserId=2 → tabId=1` (gold pill, right tab), 5 transactions | the same page and amounts at a 1-cent cap prompt every time (`A1`, same session) |
| `P10b-A4` | Permission revoked + C++ cache invalidated from an internal origin; **3 concurrent** `getVersion` ⇒ **1** `🔒 Domain approval needed`, 2 × `Modal already pending … queued`; one **Allow** ⇒ `Drained 3 pending request(s) … (3 resumed, 0 BRC-121)`, all three promises `RESOLVED` | `P0.8-A4`'s negative control (atomic check-and-add removed ⇒ three overlays in 56 ms, measured 2026-08-21) |
| `P10b-A5` | With the 130,000-sat modal up, injected `cefMessage.send('brc100_auth_response', [JSON.stringify({approved:true})])` over CDP ⇒ `🛡️ brc100_auth_response without requestId from role notification — refused, nothing resolved (10b)`; no approval consumed, transaction count unchanged, modal still up | pre-fix the same id-less message is exactly what the React modal sent, and it resolved **both** payments (the A1 RED) |
| `P10b-A6` | T1, `state.rs :: a6_one_lock_decision_never_exceeds_the_session_cap`: 20 threads × 90 cents against a 1000-cent session cap ⇒ **exactly 11 Silent, 25 rounds running**, counters `spent=990, count=11` | `a6_red_three_step_sequence_lets_the_whole_burst_go_silent` drives the **production** primitives in the pre-10b order with a barrier after the snapshot ⇒ **20 of 20 silent** (printed). Negative control on the fix: split the lock again (snapshot, sleep 10 ms, re-lock) ⇒ GREEN test FAILED `left: 20, right: 11` |
| `P10b-A7` | T1, `handlers.rs :: pay402_reuse_key_tests`: two sites / two server keys ⇒ different keys; same site + server + url ⇒ same key (control); fields cannot collide | Negative control: key reverted to `original_url` alone ⇒ `a7_another_site_…` and `a7_another_server_key_…` **FAILED**, the same-site control stayed green. ⚠️ **T2 not run** — a live two-site 402 would mint two unbroadcast nosend transactions and reserve coins; recorded as T1-only, not as a pass |
| `P10b-A8` | Every `CreateNotificationOverlayTask` post now carries a requestId (grep, 9 sites): queue helper, show-next, both connect openers, manifest bundle, both BRC-121 posts. The two that do not — `wallet_unavailable`, `no_wallet` — are informational cards that send **no** `brc100_auth_response` (grep). Arrival paths: IPC `wallet_call`, HTTP `Open()`, BRC-121 (own registry, queue-per-URL), internal paymail/PeerPay (no domain ⇒ never prompts), and the connect-approval drain now connect-only | a path left on the domain fallback is the RED — `getRequestIdForDomain` has **one** caller left, `sendAuthRequestDataToOverlay`, on the dead `overlay_show_brc100_auth` chain (see residuals) |

### Residuals — found while doing this, not fixed here

1. ⛔ **`overlay_show_brc100_auth` is dead code.** `initWindowBridge.ts` only ever *reads* `window.pendingBRC100AuthRequest`; nothing sets it. That chain keeps `storePendingAuthRequest` and the last `getRequestIdForDomain` caller alive. **Reported, not deleted** (working rule 3).
2. The audit line `consent.prompt_shown` is written when a prompt is *raised*, not when it is displayed — a queued prompt logs it too. `R-INTEXT` counts these, so changing it is an instrument edit and belongs in its own commit.
3. `💰 OnWalletCallSuccess` (the gold pill) also fires for a **user-approved** payment, not only an auto-approved one. Pre-existing; the pill means "a payment succeeded on this tab".
4. Payments under one cent record `cents=0` (measured: 20k sats ⇒ `payment.auto_approved cents=0`) — CU-7's truncation, already scheduled for beta.4.
5. A **cross-domain** kind prompt still replaces the screen when a *connect* prompt is posted (connect openers do not queue). The invariant holds — the replaced prompt cannot be resolved by the new modal's click — but it is invisible until it times out.
6. `domain_permission_invalidate` drains **all** pending entries for a domain, including kind prompts, without answering them; those callers wait for the timeout. Pre-existing.

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
| preflight -Full | **PASS** — all checks ran (T0 gates, cargo test ×2, hodos_tests, frontend build, T1e/T1f/T1g) | 2026-09-15 | Windows |
| preflight -NegativeControl | n/a — no gate pattern or baseline touched (rule 6) | 2026-09-15 | Windows |
| regression set | ⬜ runs at the Phase 10 boundary (after 10c), T2 halves included; `R-ONE-CLICK-ONE-SPEND` added in its own commit `5102335` | | |
| adversarial review | ⬜ one panel over 10a+10b+10c+10d after 10c lands | | |


---

## 4c. 🍎 `P10b-A5` VISUAL half — macOS 2026-09-19. ⭐ **The "1 of N" line that `W6` reported missing is FIXED and has now been SEEN.**

Rendered the real `payment_confirmation` modal with the exact parameters
`HttpRequestInterceptor.cpp:5530-5536` supplies, at the DPI matrix's small-screen cell.

| Assertion | Result |
|---|---|
| ⭐ the **"1 of N"** line | ✅ **PRESENT**: *"1 of 2 requests from this site — each is approved separately"*. Windows' `W6` sitting (2026-09-16) found it never rendered, because `queuedFromSite` was frozen at enqueue; **10e `0bc64d4` fixed it**, and this is its first observed render. Driven by the `queuedFromSite` URL param (`BRC100AuthOverlayRoot.tsx:585,1695,1991`) |
| 7a clipping (`P7a-A2` shape) | ✅ at **1366×768, 1366×600 and 1366×500**: Deny / Modify Limits / Approve **100 % visible** at every height; card bottom never exceeds the viewport |
| legibility | ✅ amount, domain, the exceeded-limit sentence and all three buttons render cleanly; screenshot read, nothing clipped or overlapping |

### ⬜ NOT run — the queue half, stated rather than fudged

*"The second modal appears after the first click"* is **not measured**. That needs two genuinely queued
requests through the C++ queue, and this dev wallet's balance is **0**, so no real pair could be raised.
What was measured is the modal's **rendering** with the parameters C++ supplies — **not**
`PendingRequestManager` sequencing.
⭐ **For whoever runs it: it costs zero satoshis.** Answer **Deny** on both prompts — the approval gate
runs before the spend, so the queue, the "1 of N" line and the second-modal behaviour are all exercised
without moving money. The Windows run spent real money only because it clicked Approve.

### 🐞 Found by looking at the rendered modal — a money-screen formatting defect, NOT fixed

`BRC100AuthOverlayRoot.tsx:838-845`:

```ts
} else if (sats >= 1000) {
  return (sats / 1000).toFixed(3) + 'k sats';
```

📏 **130,000 sats renders as `130.000k sats`** — on the payment approval modal, the one screen where the
user decides whether to spend. Every amount from **1,000 to 99,999,999 sats** is affected (`1,500` →
`1.500k sats`), and `.toFixed(3)` implies three digits of precision that do not exist.
✅ **FIXED 2026-09-19 (owner said fix it).** The middle branch is deleted; everything below 1 BSV is now
`sats.toLocaleString() + ' sats'`. Measured across every boundary on the real modal: 700 → `700 sats`,
1,000 → `1,000 sats`, 1,500 → `1,500 sats`, **130,000 → `130,000 sats`**, 99,999,999 →
`99,999,999 sats`, 100,000,000 → `1.00000000 BSV` (the ≥ 1 BSV branch deliberately kept — a unit change
genuinely helps at that size). `tsc --noEmit` clean; screenshot re-read at 1366×768.
⭐ `toLocaleString()` also makes the grouping follow the user's locale rather than a hardcoded `.`, so it
can never collide with the decimal separator again. Cross-platform: Windows gets the fix on rebase.