# Phase 1 — BRC-121 Simple 402 Payments

**Status: COMPLETE.** Initial scope (~150 LOC) shipped at `0a73b98` (2026-05-08). Polish pass on top (cache + placeholder + failure page + auto-retry + reuse-don't-recreate + WalletStatusCache hardening) shipped in subsequent commits and grew the surface to ~1500 LOC + 6 new files. The polish was driven by real-world testing against `now.bsvblockchain.tech` exposing flaky upstream rejections and a "we pay every reload" UX issue not in the original spec.

Pure additive. Reuses **BRC-29 PeerPay derivation** (same protocol ID `3241645161d8`). No protocol unknowns. Independent of Phase 1.5 (which makes the auto-approve gates richer but doesn't change the BRC-29 payment plumbing).

> **Phase kickoff:** before any code is written, follow the kickoff workflow in root `CLAUDE.md` "Phase kickoff workflow" section: re-read this doc + linked sources, verify cited line numbers are still current, do a reuse-first audit, do a risk assessment, confirm test plan, hand back summary for confirmation. Don't go from this README to first commit without that step.

---

## Acceptance criteria

Localhost 402 demo server returns:
- HTTP `402 Payment Required`
- `x-bsv-sats: <amount>` header
- `x-bsv-server: <paymail or address>` header

Hodos:
1. Intercepts the 402 response
2. Builds a BRC-29 payment to the server's destination
3. Retries the original request with `x-bsv-payment: <BEEF>` header
4. Server validates BEEF → returns 200

**Round-trip <2 seconds.** No new prompts during steady state (auto-approved through existing payment gates).

---

## Reuse map (do not duplicate — call existing functions)

Before writing anything new, the kickoff review verifies these are still where this doc says they are. **Do not write new derivation, signing, or BEEF code.** Wire to what exists.

### BRC-29 derivation primitive

- `BRC-29 protocol ID` = `"3241645161d8"`. Already in use at `rust-wallet/src/handlers.rs:4348, 5991, 6077, 15299` and `monitor/task_check_peerpay.rs:201`. **Same constant; reuse the existing string.**
- `BRC-29 invoice number format` = `"2-3241645161d8-{prefix} {suffix}"` per `handlers.rs:4348`. Same primitive; reuse.
- `BRC-29 P2PKH script construction` lives in `handlers.rs` around line `4366` (search for "Created BRC-29 P2PKH script"). Reuse this builder.

### Payment-building entry points

- `create_action` at `rust-wallet/src/handlers.rs:3381` (large payload variant, 100 MB limit) — the canonical transaction builder. BRC-121 wraps a call into this with the right outputs.
- `create_action_internal` at `rust-wallet/src/handlers.rs:3577` — internal entry, also reachable.
- `peerpay_send` at `rust-wallet/src/handlers.rs:15224` — closest existing handler; the BRC-121 handler is a slimmer sibling. **Read this first to understand the shape, then write `pay_402` mirroring the structure.**
- `paymail_send` at `rust-wallet/src/handlers.rs:15637` — relevant if `x-bsv-server` is a paymail (most likely case). Reuses `paymail.rs`'s `PaymailClient` for capability discovery + P2P destination resolution.

### HTTP interception infrastructure

- `OnResourceResponse` at `cef-native/src/core/HttpRequestInterceptor.cpp:2056` — currently a no-op `return false`. **This is the entry point for 402 detection.** Fill it; do not write a parallel hook.
- `isWalletEndpoint` route table — add `/wallet/pay402` here. **Don't bypass the table; new routes go through it.**
- `AsyncWalletResourceHandler` — existing class. New endpoint reuses this handler shape.
- `SessionManager` per-tab spend cap + rate limit — already enforces auto-approve gates. **Just hit it normally; don't reinvent.**

### Payment success animation (preserve!)

- `payment_success_indicator` IPC at `HttpRequestInterceptor.cpp:1656-1681` already fires after every auto-approved successful payment. **The 402 path must trigger this through the same code path.** Don't introduce a parallel notification mechanism — just make sure your success path goes through `AsyncWalletResourceHandler::OnRequestComplete`-equivalent so the indicator fires.

### Service fee output

- Every outgoing tx already adds a 1000-sat output to `HODOS_FEE_ADDRESS` per the `HODOS_SERVICE_FEE_SATS` constant in `handlers.rs`. `create_action_internal` does this automatically. **The 402 handler must use `create_action_internal` (or `create_action`) so the fee is applied; do not bypass this.**

---

## Risk assessment

Things this phase could touch or break:

| Risk | Mitigation |
|---|---|
| Payment auto-approve regresses | Smoke test: existing `peerpay_send` flow still works after change |
| 402 path bypasses domain permission gates | Route through `isWalletEndpoint` route table; verify `check_domain_approved` fires |
| 402 path bypasses spend cap | Confirm SessionManager counters increment via `wasAutoApprovedPayment_ = true` flow |
| Payment animation doesn't fire on 402 success | Manual test: green-dot animates on the tab after 402 round-trip |
| `OnResourceResponse` change breaks other intercepted responses | Audit other call sites; ensure existing `return false` cases still hit |
| Win/Mac divergence | Test on both before merge; HTTP interception is platform-neutral, but the success path through animation needs both verified |
| BRC-29 protocol ID accidentally forked | Use the existing constant; do not redefine |
| Existing PeerPay flows break | Smoke test: send PeerPay payment after change; should still work |

---

## Implementation order

1. **Kickoff review** — verify all reuse-map line numbers still accurate (5 minutes of `grep`).
2. **`pay_402` Rust handler** — `handlers.rs`, mirroring `peerpay_send` shape but accepting `(server_paymail_or_address, sat_amount)`. Returns BEEF for the `x-bsv-payment` header.
3. **`/wallet/pay402` route** — `main.rs`, near peerpay routes.
4. **`isWalletEndpoint` entry** — `HttpRequestInterceptor.cpp`.
5. **`OnResourceResponse` 402 detection** — read `x-bsv-sats` + `x-bsv-server` headers; fire async `pay_402` call; on response, retry the original request with `x-bsv-payment` header.
6. **Auto-approve integration** — confirm 402 payments hit the existing `wasAutoApprovedPayment_` path; payment animation fires.
7. **Localhost demo server** — minimal Express/Actix server returning 402 for one path, validating BEEF, returning 200.
8. **Smoke test** — round-trip <2s on Win and Mac.

---

## Test plan

- **Unit:** `pay_402` handler builds correct BRC-29 output + service fee + change.
- **Integration:** localhost 402 demo round-trip succeeds; BEEF validates server-side.
- **Smoke:** existing PeerPay flow still works (regression check); auto-approve gates still respected; payment animation fires.
- **Cross-platform:** verify on both Windows and macOS builds before merge.

---

## What this phase does NOT do

- No new permission tables (Phase 1.5 territory).
- No V8 shim (Phase 2).
- No ordinal-aware logic (Phase 3).
- No demo writeups beyond the localhost test server (Phase 4).

---

## Polish work shipped on top of original scope

The original spec assumed a clean localhost demo. Real-world testing exposed gaps that needed to be closed before Phase 1 could be considered done. Each item preserves load-bearing safeguards (`payment_success_indicator`, "Always notify" toggle, privacy perimeter prompts, per-session counters).

### Paid Content Cache
**Problem:** every reload re-paid for the same article.
**Fix:** disk-backed SQLite at `<profile>/paid_content_cache.db`. URL-keyed entries with TTL from server `Cache-Control: max-age` (or `NULL` = forever-with-LRU-cap). 500 MB total LRU cap on `last_access`.
**Code:** `cef-native/src/core/PaidContentCache.cpp` (new). Write hook at `Async402ResourceHandler::onUpstreamComplete` after `firePaymentSuccessIpc`, gated by `status >= 200 && status < 300`. Read hook at top of `SimpleHandler::GetResourceRequestHandler` (single dispatch site). Hard-reload bypass via `Cache-Control: no-cache` request header (Chromium adds this on Ctrl+Shift+R). Toggle on `PrivacySettings.paidContentCacheEnabled` (default true). Clear button in Cache & Storage panel.

### PaymentPendingPage placeholder
**Problem:** when a 402 hit an unapproved domain, the modal popped on top of CEF's `data:text/html` "Failed to load" page — visually broken.
**Fix:** `OnLoadError` checks if the failed URL's domain has a pending BRC-121 reload (`HasPendingBrc121ReloadForDomain`). If yes, navigate to `http://127.0.0.1:5137/payment-pending?domain=…&sats=…` instead of the data: failed-load URL. The placeholder shows a small spinning gold Hodos logo + "Waiting for your approval" caption in the top-left so it doesn't compete with the centered modal.
**Code:** `frontend/src/pages/PaymentPendingPage.tsx`, `OnLoadError` swap in `simple_handler.cpp`, registry in `HttpRequestInterceptor.cpp`.

### PaymentFailedPage
**Problem:** when the paid retry exhausted retries with non-2xx (typically Cloudflare 431 against the BEEF base64 header), the user landed back on CEF's failed-load page with no indication that their sats were safe.
**Fix:** `Async402` registers the URL via `RegisterBrc121FailedUrl`. `OnLoadError` consumes the entry and navigates to `http://127.0.0.1:5137/payment-failed?domain=…&sats=…&status=…&originalUrl=…`. The page reads "{domain} rejected the payment. Your sats are safe — the transaction was not broadcast (HTTP 431)." with a Try Again button (re-navigates to original URL) and a Go Back button.
**Code:** `frontend/src/pages/PaymentFailedPage.tsx`, registry helpers in `HttpRequestInterceptor.cpp`, second `OnLoadError` branch in `simple_handler.cpp`.

### Auto-retry on 431 / 5xx
**Problem:** Cloudflare in front of `bsvblockchain.tech` returns transient HTTP 431 ("Request Header Fields Too Large") for the BEEF base64 retry header on roughly half the attempts. The same URL+headers succeed seconds later. Without retry, the user saw the failure page on every other visit.
**Fix:** `Async402ResourceHandler::onUpstreamComplete` retries once (`MAX_UPSTREAM_RETRIES = 1`) with a 250 ms backoff (`RETRY_DELAY_MS`) when status is 431 or 5xx, reusing the SAME paid retry context (no new nosend tx). `openCallback_` and `readCallback_` are held during the retry — CEF waits.
**Code:** `Async402ResourceHandler` in `HttpRequestInterceptor.cpp`.

### Reuse-don't-recreate (Rust)
**Problem:** when the user clicked Try Again on `/payment-failed`, `pay_402` fired again and minted a brand-new nosend tx. The previous unbroadcast nosend tx orphaned in the wallet — money-wise no loss (never broadcast), but it leaked unspent outputs into the nosend pile.
**Fix:** in-memory cache on `AppState.pay402_reuse` mapping `(URL, sats)` → full retry context (txid + BEEF + derivation prefix/suffix + time_ms + sender pubkey + vout). On `pay_402`, if the entry is within ~25 s AND the tx is still in `nosend` status, return the cached entry instead of calling `create_action`. TTL is bounded by the BRC-121 server-side `x-bsv-time` 30 s freshness window. Drained on `broadcast-nosend` success. Note: the BRC-121 retry headers are baked into the tx (different `time_ms` produces a different child pubkey → different output script → different tx), so reuse MUST replay the same headers.
**Code:** `Pay402ReuseEntry` struct + lookup/storage in `pay_402` and drain in `broadcast_nosend` (`rust-wallet/src/handlers.rs`); `pay402_reuse` field on `AppState` (`main.rs`).

### WalletStatusCache hardening (C++)
**Problem:** observed during testing — a single transient `/wallet/status` timeout (1 s WinHTTP) poisoned BRC-121 for 30 s with bogus "no wallet — falling through to native 402". The wallet was alive (peerpay/status worked through a different code path 2.7 s later) but the cache had stuck the wrong answer.
**Fix:** bumped timeout from 1 s to 3 s. Three-state `Status` enum (`Exists` / `DoesNotExist` / `FetchFailed`) with separate cache TTLs: 30 s for definitive HTTP-success answers, 2 s for transient fetch failures. The next 402 within 2 s of a transient failure now retries the wallet-status check rather than waiting 30 s.
**Code:** `WalletStatusCache` class in `HttpRequestInterceptor.cpp`.

### Eager-load Hodos error pages
**Problem:** the placeholder felt slow on first hit because `PaymentPendingPage` was lazy-loaded — the chunk had to fetch from Vite before rendering.
**Fix:** moved `PaymentPendingPage` and `PaymentFailedPage` out of `React.lazy` so they bundle with the main entry. ~3 kB cost on `index.js`, instant render.
**Code:** `frontend/src/App.tsx`.

### Back-button history fix (partial — known limitation)
**Problem:** after going through the unapproved-domain modal flow, back-from-article went to `/payment-pending` instead of the previous real page.
**Fix (partial):** `TriggerPendingBrc121Reloads` uses `frame->ExecuteJavaScript("window.location.replace(...)")` instead of `LoadURL` so `/payment-pending` is replaced in history rather than appended. `Brc121ReloadTask` gained a `replace_history` flag (default `false` to preserve auto-approve internal-reload semantics where the same URL is loaded — Chromium treats same-URL `LoadURL` as a refresh).
**Known limitation:** the rest of the BRC-121 reload chain (`pay_402`-triggered reload, paid-retry reload, `/payment-failed` swap, Try Again navigation) still appends to history. Back from a successfully-loaded article still walks through 3-5 intermediate entries before reaching the previous real page. Filed as a Phase 1.5 polish task.

---

## Updated test plan (post-polish)

- **Unit:** `pay_402` handler builds correct BRC-29 output + service fee + change.
- **Integration:** localhost 402 demo (or `bsvblockchain.tech` paid news) round-trip succeeds; BEEF validates server-side.
- **Cache:** first visit pays + caches; soft reload (Ctrl+R) serves cached bytes with no payment IPC, no `SessionManager` increment, no broadcast; hard reload (Ctrl+Shift+R) bypasses cache and re-pays.
- **Cache toggle/clear:** Settings → Privacy → "Cache paid content" off → reload re-pays. Cache & Storage → "Clear Paid Content" → reload re-pays.
- **Unapproved-domain placeholder:** revoke a domain in Approved Sites; visit a paid article → expect Hodos placeholder background (small spinning gold logo top-left + "Waiting for your approval") with modal centered. NO "Failed to load" text.
- **Approve from placeholder:** click Approve → article loads. Log: `💰 BRC-121: location.replace ...`.
- **Reject from placeholder:** click Reject → goes back to previous page (or `about:blank` if no history). Log: `💰 BRC-121: rejected — going back`.
- **Auto-retry:** if upstream returns 431/5xx, log shows `auto-retry 1/1 for ...`. Most retries succeed silently and the user sees the article load.
- **Failure page:** if both attempts fail, tab navigates to `/payment-failed` with "{domain} rejected the payment. Your sats are safe..." Log: `💰 BRC-121: swapping failed-load for /payment-failed (...)`.
- **Reuse-don't-recreate:** click Try Again on `/payment-failed` within ~25 s → log shows `💸 pay_402 REUSE: returning existing nosend tx <txid> for <url> (... sats, age <Nms>)`. Same txid as the failed attempt; no new nosend tx.
- **WalletStatusCache:** if `/wallet/status` times out once, the next 402 attempt within 2 s should retry rather than waiting 30 s. (Hard to test deliberately; observe in production logs.)
- **Smoke regression:** existing PeerPay flow still works; auto-approve gates respected; payment animation fires. Verify on real-world sites: youtube.com, x.com, github.com still load normally (no false 402 detection).
- **Cross-platform:** macOS smoke per `MACOS_PARITY_ANALYSIS.md` "Smoke test on macOS" section.

---

## References

- `../README.md` — sprint context
- `../_DRAFT_RECOVERED_PLAN.md` — original plan, includes deeper code citations
- `../ARCHITECTURE.md` — sprint diagrams
- Root `CLAUDE.md` — invariants, testing standards, **phase kickoff workflow**
- `rust-wallet/src/CLAUDE.md` — handler groups, BRC-29 references
- `cef-native/src/core/CLAUDE.md` — HTTP interception, auto-approve flow
