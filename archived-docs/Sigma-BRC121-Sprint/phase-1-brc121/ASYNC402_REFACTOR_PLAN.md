# Phase 1 — Async402ResourceHandler refactor (cleanup + proper architecture)

## Context

After 6+ rounds of symptom patching (wire format, side-channel cooldown, modal pending-reload registry, frame->LoadRequest workaround, UR_FLAG_DISABLE_CACHE, diagnostic logging), we still don't have reliable BRC-121 page delivery. Each patch addressed a real bug but accumulated in `TryHandleBrc121_402` as scattered logic. CEF's LoadRequest navigation also doesn't fire our `OnResourceResponse` (confirmed in log — only `/favicon.ico` triggers the diagnostic, not the article URL).

Time to do it right. Mirror the `AsyncWalletResourceHandler` pattern that already works for wallet endpoints.

## Architecture

```
Page → server.example.com/paid → 402 + x-bsv-sats + x-bsv-server
                ↓
CookieFilterResourceHandler::OnResourceResponse (existing)
  detects 402 + BRC-121 headers, captures (browserId, url, request method, body, original headers)
  calls /wallet/pay402 (no_send=true — DON'T broadcast yet)
  stores retry context in pending registry keyed by (browserId, url)
  fires programmatic reload: frame->LoadURL(url)
  returns false (page briefly sees error, reload supersedes)
                ↓
CEF starts a fresh navigation for the reload
                ↓
SimpleHandler::GetResourceRequestHandler is called
  checks if (browserId, url) is in pending registry
  YES → returns Brc121PaidResourceHandler (new)
                ↓
Brc121PaidResourceHandler::GetResourceHandler
  pops retry context from registry
  returns Async402ResourceHandler with the captured 5 BRC-121 headers + payment metadata
                ↓
Async402ResourceHandler::Open()
  fires CefURLRequest to original URL with all 5 BRC-121 headers
  flags: UR_FLAG_DISABLE_CACHE | UR_FLAG_REPORT_LOAD_TIMING
  returns true with handle_request=true; readCallback held until response arrives
                ↓
AsyncHTTPClient::OnRequestComplete
  receives retry response
  if 200 → call /wallet/broadcast-nosend with txid (broadcast our nosend tx, ARC dedupes if server already did)
        → fire payment_success_indicator IPC (preserve green-dot animation)
        → store body, mark request complete, callback->Continue()
  if 4xx → don't broadcast (no point spending fees), store error body, callback->Continue()
                ↓
Async402ResourceHandler::ReadResponse delivers body bytes to the page
                ↓
Page renders article (or error if server rejected)
```

## What this fixes

| Symptom | Old fix | New fix |
|---|---|---|
| Wire format wrong | 5-header SetHeaderMap + return true | Async402ResourceHandler issues its own CefURLRequest with full header control |
| CEF retry strips headers | LoadRequest workaround | Custom CefURLRequest = no CEF middleware to strip |
| Cache hit replaying 402 | UR_FLAG_DISABLE_CACHE on CefRequest | UR_FLAG_DISABLE_CACHE on CefURLRequest inside handler |
| Loop guard needed | Side-channel cooldown map | Single-shot handler — pending registry pops on use, no re-entry possible |
| Page stuck on error | TriggerPendingBrc121Reloads (modal flow) | Programmatic reload from OnResourceResponse delivers via Async402ResourceHandler — same mechanism for both auto-approve and approval-modal cases |
| isMerge replay risk | no_send=true (server broadcasts) | no_send=true + broadcast-after-200 (we broadcast after server confirms acceptance, eliminating the race) |
| nosend race vs auto-fail | (deferred) | Broadcast-after-200 means tx hits chain immediately on success, no stuck nosend |

## Code to add

| File | Component | LOC |
|---|---|---|
| `cef-native/src/core/HttpRequestInterceptor.cpp` | `Async402ResourceHandler` class | ~250 |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | `Async402HTTPClient` (CefURLRequestClient) | ~80 |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | Pending paid retry registry (replaces existing reload registry) | ~50 |
| `cef-native/src/handlers/simple_handler.cpp` | `Brc121PaidResourceHandler` wrapper class | ~30 |
| `cef-native/src/handlers/simple_handler.cpp` | `GetResourceRequestHandler` lookup hook | ~10 |
| `rust-wallet/src/handlers.rs` | `/wallet/broadcast-nosend` endpoint | ~70 |
| `rust-wallet/src/main.rs` | Route registration | ~1 |

## Code to remove (cleanup)

| What | Where |
|---|---|
| Side-channel cooldown map (`s_brc121_paid`, `wasRecentlyPaid`, `markPaid`, `BRC121_PAID_COOLDOWN`) | `HttpRequestInterceptor.cpp` |
| `Brc121PaidNavigateTask` class | `HttpRequestInterceptor.cpp` |
| `frame->LoadRequest` invocation in `TryHandleBrc121_402` | `HttpRequestInterceptor.cpp` |
| `UR_FLAG_DISABLE_CACHE` on `paidRequest` (moves into handler) | `HttpRequestInterceptor.cpp` |
| Diagnostic log line "BRC-121 saw response status=" | `HttpRequestInterceptor.cpp` |
| `Brc121ReloadTask` if no longer needed (may keep for modal-approval reload) | `HttpRequestInterceptor.cpp` |
| `TriggerPendingBrc121Reloads` and `s_brc121_pending_reloads` registry — replaced by new pending paid retry registry that carries full headers | `HttpRequestInterceptor.cpp` |
| Header check loop guard at top of TryHandleBrc121_402 (no longer relevant) | `HttpRequestInterceptor.cpp` |

## Broadcast policy

- `pay_402` uses `no_send=true` again (don't broadcast at mint time).
- `Async402ResourceHandler` calls `/wallet/broadcast-nosend` only after the server returns 200, eliminating the isMerge race AND the noSend auto-fail race in one move.
- ARC dedupes if server also broadcast → no double-spend.
- New endpoint `/wallet/broadcast-nosend { txid }` looks up the tx, broadcasts via the existing `broadcast_transaction` helper, status nosend → sending → unproven → completed via existing Monitor pipeline.

## Test plan

- Phase 1 happy path: navigate to a fresh article on now.bsvblockchain.tech → 402 detected → modal NOT needed (domain approved) → handler installs → retry returns 200 with article HTML → broadcast fires → activity shows tx as sending → eventually completed.
- Approval-modal happy path: clear domain from approved → navigate → modal fires → approve → reload → handler installs → retry returns 200 → article renders → broadcast fires.
- Failure path: server hypothetically returns 4xx on retry → no broadcast happens (funds preserved) → page shows error.
- Regression: createAction-style BRC-100 calls still work (different handler, untouched).
- Regression: PeerPay still works.

## Memory cleanup

After this lands and tests pass:
- `reference_brc121_no_send_required.md` — supersede with note that broadcast-after-200 is the architectural answer.
- `feedback_dont_overscope_deferrals.md` — keep, lesson stands.
- `project_phase15_nosend_race.md` — supersede (race is solved, not deferred).
- All `feedback_*` from this work — keep the ones that captured real lessons.

## Why this is the right scope for Phase 1

The user's principle: build it right the first time, not patch symptoms. Async402ResourceHandler IS the right architecture. Doing it now:
- Removes ~200 LOC of accumulated patches
- Adds ~500 LOC of canonical code that mirrors an existing working pattern
- Solves multiple class of bugs in one move (header stripping, cache replay, loop, broadcast race)
- Sets up the broadcast-after-200 design the user explicitly wants
- Done correctly = "ship and forget" for Phase 1
