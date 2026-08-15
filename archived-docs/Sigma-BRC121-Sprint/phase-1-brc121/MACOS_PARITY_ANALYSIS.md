# Phase 1 BRC-121 — macOS parity analysis

**Date:** 2026-05-08
**Scope:** All BRC-121 changes shipped during Phase 1.
**Goal:** Identify what carries over to macOS as-is, what needs macOS-specific verification, and what (if anything) needs macOS-specific implementation.

---

## TL;DR

**No macOS-specific implementation work is required.** Every file we touched compiles for both platforms; every API we use is cross-platform. Three real verification risks worth a focused test pass on Mac, all in the "expected to work but unproven" bucket — none are "this is broken until we write Mac code."

---

## Files modified — platform impact matrix

| File | Lines changed | Platform impact |
|---|---|---|
| `rust-wallet/src/handlers.rs` | +250 (`pay_402`, `broadcast_nosend`) | None — pure Rust, cross-platform |
| `rust-wallet/src/main.rs` | +2 routes | None — Actix is cross-platform |
| `rust-wallet/tests/tier12_brc121_pay_402_test.rs` | +120 (new test file) | None — Rust unit tests |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | ~+450 (Async402ResourceHandler, Async402HTTPClient, registry, TryHandleBrc121_402 rewrite, InstallAsync402HandlerIfPending) | **Cross-platform CEF APIs only.** No `#ifdef _WIN32` blocks added. |
| `cef-native/include/core/HttpRequestInterceptor.h` | +20 (declarations) | None — declarations only |
| `cef-native/include/core/AdblockCache.h` | +14 (`GetResourceHandler` + `OnResourceResponse` overrides on `DeferredAdblockHandler`, extern decl of `InstallAsync402HandlerIfPending`) | Cross-platform CEF APIs only |
| `cef-native/src/handlers/simple_handler.cpp` | +50 (`CookieFilterResourceHandler::OnResourceResponse` and `GetResourceHandler` overrides, `popAllForDomain` calls in approval/invalidate IPC handlers, fallback-to-`text/html`-on-no-`Content-Type`, `CookieFilterResourceHandler` is now also returned for HTTPS cache-allowed path) | Cross-platform CEF APIs only |
| `frontend/src/components/DomainPermissionsTab.tsx` | +9 (cache invalidation IPC after revoke) | None — React/TS |
| `frontend/src/components/wallet/ApprovedSitesTab.tsx` | +4 (cache invalidation IPC after reset-all) | None — React/TS |
| `demos/brc121-402/` | new folder (Express demo) | None — Node.js |

**No `cef_browser_shell_mac.mm`, `simple_handler_mac.mm`, `WindowManager_mac.mm`, `TabManager_mac.mm`, or other macOS-only file required modification.**

---

## CEF APIs we used — all cross-platform

All of these have identical behavior on Win + macOS in CEF 136:

- `CefResourceHandler` (Open, GetResponseHeaders, ReadResponse, Cancel)
- `CefURLRequest` / `CefURLRequestClient`
- `CefRequest` / `CefResponse` (SetURL, SetMethod, SetHeaderMap, SetFlags, SetMimeType, etc.)
- `CefRequest::HeaderMap` (multimap of CefString)
- `UR_FLAG_DISABLE_CACHE` (request flag)
- `CefPostTask(TID_IO, ...)` / `CefPostTask(TID_UI, ...)` / `CefPostTask(TID_FILE_USER_BLOCKING, ...)`
- `CefBrowser::GetMainFrame()->LoadURL` (programmatic navigation)
- `CefProcessMessage` (IPC for `payment_success_indicator` and `domain_permission_invalidate`)

---

## App-level singletons we used — all cross-platform

- `DomainPermissionCache` — `HttpRequestInterceptor.cpp`, no platform code
- `BSVPriceCache` — same
- `WalletStatusCache` — same
- `SessionManager` — same
- `PendingRequestManager` — `PendingAuthRequest.h` (header-only), no platform code
- `SyncHttpClient` — explicit Win (WinHTTP) / macOS (libcurl) impls, **already cross-platform** at the API level. We use `Post()` only.
- `SimpleHandler::GetHeaderBrowser()`, `SimpleHandler::GetNotificationBrowser()` — cross-platform

---

## Notification overlay — architecturally symmetric

Our modal flows (`domain_approval`, `payment_confirmation`, `rate_limit_exceeded`) and the right-click "Manage Permissions" form (`edit_permissions`) all dispatch through `CreateNotificationOverlay`:

| Platform | File | Signature | Render |
|---|---|---|---|
| Windows | `simple_app.cpp:1150` | `CreateNotificationOverlay(HINSTANCE, type, domain, extraParams)` | OSR via `MyOverlayRenderHandler` + `WS_POPUP` |
| macOS | `cef_browser_shell_mac.mm:3453` | `CreateNotificationOverlay(type, domain, extraParams)` | OSR via `MyOverlayRenderHandler` + `NSWindow` (NotificationOverlayWindow) |

Both:
- Build the same URL: `http://127.0.0.1:5137/brc100-auth?type=...&domain=...&...`
- Use the same React entrypoint (`BRC100AuthOverlayRoot.tsx`)
- Implement keep-alive via `window.showNotification(queryString)` JS injection
- Pass the type parameter transparently to React, which decides what UI to render

**Implication:** All five notification types we use (`domain_approval`, `payment_confirmation`, `rate_limit_exceeded`, `no_wallet`, `edit_permissions`) work on macOS without any C++ changes. The React side already handles them; the dispatch is symmetric.

The `CreateNotificationOverlayTask::Execute()` already has the platform conditional:
```cpp
#ifdef _WIN32
    extern HINSTANCE g_hInstance;
    CreateNotificationOverlay(g_hInstance, type_, domain_, extraParams_);
#elif defined(__APPLE__)
    CreateNotificationOverlay(type_, domain_, extraParams_);
#endif
```

This was pre-existing code; we didn't change it. It just works.

---

## Real risks worth verifying on macOS

### Risk 1: zstd auto-decompression in CefURLRequest (medium)

We confirmed that `now.bsvblockchain.tech` returns `Content-Encoding: zstd` (Cloudflare's newest compression). On Windows our diagnostic log showed the body delivered to `OnDownloadData` was already decoded HTML (`<!DOCTYPE html>...`). This means CEF's network stack on Windows is auto-decompressing zstd.

**On macOS, CEF uses Chromium's network stack the same way — zstd should also auto-decompress.** But because this is a relatively new Chromium feature (2024), it's worth verifying empirically on Mac. If macOS CEF does NOT auto-decompress zstd, the body bytes we forward to the page would be raw zstd, and the article would render as garbage.

**How to detect on Mac:**
- Look for `🌐 Async402: body preview:` log line.
- If it starts with `<!DOCTYPE html>` → decompression worked, ship it.
- If it starts with `[hex first 64B] 28b52ffd...` (zstd magic prefix `28 b5 2f fd`) → CefURLRequest didn't decompress, page will render garbage.

**If broken on Mac:** the fix is to disable our `Content-Encoding` strip when the upstream uses an algorithm the URL stack didn't unwind, OR explicitly negotiate `Accept-Encoding: gzip, br` on the outgoing request to avoid zstd. We can probe and decide. Phase 1.5 deliverable if it bites.

### Risk 2: `CefResourceHandler::Open` deferred-callback semantics on macOS (low)

The fix that resolved the page-render-as-text bug relied on `Open()` returning with `handle_request=false` and storing the `CefCallback` until our upstream response arrived. CEF's resource-handler protocol is the same on both platforms, but this is a less-trodden path. Worth confirming `GetResponseHeaders` waits for `openCallback_->Continue()` on macOS the same way it does on Windows.

**How to detect:** if `GetResponseHeaders` log shows `status=0 mime='text/plain (502 fallback)'` instead of the real `status=200 mime='text/html'`, the deferred-callback isn't being respected on Mac.

**If broken on Mac:** we'd need to find the macOS-specific quirk. CEF source is the same codebase; this would be unusual.

### Risk 3: `frame->LoadURL` programmatic-reload behavior on macOS (low)

We use `frame->LoadURL` from `Brc121ReloadTask` to trigger the paid-retry navigation after registering the `PaidRetryContext`. On Windows this works cleanly. macOS's window/frame system uses `NSView`-hosted browsers (per `TabManager_mac.mm`) and `frame->LoadURL` is documented as cross-platform, but it's worth confirming the reload navigation hits `GetResourceRequestHandler` → `InstallAsync402HandlerIfPending` chain on Mac just like on Win.

**How to detect:** look for `💰 BRC-121: installing Async402ResourceHandler for ...` log line. If absent after a "BRC-121 paid (txid=...) — registered paid retry; triggering reload" line, the reload navigation isn't going through our handler chain on Mac.

**If broken on Mac:** could fall back to issuing the paid request directly via `CefURLRequest` from C++ without going through a navigation reload (but that was the architecture we explicitly moved away from since it's harder to deliver the response back into the page). Investigation needed if it manifests.

---

## Lower-risk things still worth a quick verify on Mac

- **Domain permission cache invalidation IPC** — pure C++ + IPC, but the wallet panel is a separate overlay on Mac (NSWindow). The IPC route is the same. Should work.
- **Right-click "Manage Site Permissions" menu** — `simple_handler.cpp` context menu code is platform-neutral; the menu rendering goes through CEF which renders identically on both.
- **`payment_success_indicator` IPC chain** — sends `CefProcessMessage` to the header browser's main frame, which is windowed CEF on both platforms. Identical.
- **Wallet → Async402 → broadcast-nosend round trip** — `SyncHttpClient::Post` already has libcurl impl on macOS, so no concerns at HTTP layer.

---

## macOS test plan (when you get to a Mac)

Before testing on Mac, do this on Windows once to capture a known-good baseline:
1. Visit `https://now.bsvblockchain.tech/articles/<slug>` fresh.
2. Approve domain in modal.
3. Article renders.
4. Right-click → Manage Permissions → revoke.
5. Visit again → modal fires.
6. Approve → article renders.

Then run the same sequence on Mac. Pass criteria:

| Step | Win | Mac (target) |
|---|---|---|
| 1: 402 detected, modal fires | ✓ | Should fire identically |
| 2: Modal approve → reload → handler installs → 200 → article renders | ✓ | Should render identically — biggest risk is **zstd** (article appears as garbage if Mac CEF doesn't auto-decompress) |
| 3: Activity shows `sending` → eventually `completed` | ✓ | Identical (Rust monitor pipeline is platform-agnostic) |
| 4: Right-click manage permissions → form opens → revoke | ✓ | Mac uses NSPanel for the form; same React component |
| 5: Re-visit → modal fires (not silently auto-paid) | ✓ | Fixed via `popAllForDomain` drain on invalidate; identical on both |
| 6: Re-approve → article renders | ✓ | Identical |

Auxiliary smoke tests on Mac:
- Localhost demo server (`demos/brc121-402/`) round-trip — eliminates Cloudflare/zstd as variables.
- Standard verification basket (youtube, x, github) — confirms no regression to non-402 traffic, since `CookieFilterResourceHandler` is now returned for all HTTP/HTTPS.
- PeerPay regression — confirms BRC-29 path (which `pay_402` reuses) still works.

---

## Build prerequisites for macOS

No new dependencies. Existing macOS build setup per `build-instructions/MACOS_BUILD_INSTRUCTIONS.md` covers everything.

The Rust changes need a `cargo build --release` on Mac. Frontend changes auto-load from Vite dev server.

---

## Summary

**Nothing to write for macOS.** Every BRC-121 file in this phase compiles unchanged for both platforms, uses cross-platform CEF/Rust/React APIs, and routes through pre-existing platform-conditional infrastructure (notification overlay, SyncHttpClient) that's already symmetric.

The risks are all "behavior should be the same but is unverified" — chief among them whether macOS CEF auto-decompresses zstd. That's a single empirical test (`body preview:` log line on Mac) and if it fails the fix is small (probe Accept-Encoding negotiation or per-encoding header-strip).

Worth one focused 30-minute Mac test pass once everything else is settled. Don't preemptively write Mac code; just verify and patch only what surfaces.

---

# Polish-pass additions (2026-05-09)

The polish work shipped on top of original scope (cache, placeholder, failure page, auto-retry, reuse-don't-recreate, WalletStatusCache hardening — see `README.md` "Polish work shipped on top of original scope") added six new files and modified ~10 more. Same "no Mac-specific code" verdict, plus a few new things to verify.

## Polish files — platform impact

| File | Lines | Platform impact |
|---|---|---|
| `cef-native/include/core/PaidContentCache.h` | new ~85 | None — header-only, std::sqlite3 + std::mutex |
| `cef-native/src/core/PaidContentCache.cpp` | new ~310 | None — pure C++, sqlite3 (already cross-platform per CMakeLists) |
| `cef-native/include/core/CachedContentResourceHandler.h` | new ~125 | None — header-only, CefResourceHandler/CefResourceRequestHandler (cross-platform CEF API) |
| `frontend/src/pages/PaymentPendingPage.tsx` | new ~70 | None — React |
| `frontend/src/pages/PaymentFailedPage.tsx` | new ~115 | None — React |
| `frontend/src/hooks/usePaidCache.ts` | new ~95 | None — React/IPC |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | +~150 (Async402 retry, registries, WalletStatusCache enum, Pay402ReuseEntry) | Cross-platform CEF/std only. WalletStatusCache fetch already had `#ifdef _WIN32` (WinHTTP) / else (libcurl via SyncHttpClient) — both branches updated to return Status enum. |
| `cef-native/src/handlers/simple_handler.cpp` | +~80 (OnLoadError swap branches, paid_cache IPC handlers) | Cross-platform |
| `cef-native/cef_browser_shell.cpp` (Win) + `cef_browser_shell_mac.mm` (Mac) | +~10 each | Both platforms updated identically — `PaidContentCache::Initialize(profile_cache)` alongside BookmarkManager |
| `rust-wallet/src/handlers.rs` | +~80 (Pay402ReuseEntry, lookup/storage in pay_402, drain in broadcast_nosend) | None — pure Rust |
| `rust-wallet/src/main.rs` | +1 field on AppState, +1 init line | None — pure Rust |

**No new platform-conditional blocks.** PaidContentCache uses `#ifdef _WIN32` only for the path separator (same pattern as BookmarkManager); SQLite + mutex + chrono are cross-platform.

## New risks worth verifying on macOS

### Polish Risk 1: SQLite path resolution on macOS (very low)

`PaidContentCache::Initialize(profile_path)` builds the DB path as `profile_path + "/paid_content_cache.db"` on non-Windows. The Mac launcher calls `Initialize(profile_cache)` from `cef_browser_shell_mac.mm:4710` (next to `BookmarkManager::Initialize`), and `profile_cache` is the same `~/Library/Application Support/HodosBrowserDev/Default` path that BookmarkManager uses. If BookmarkManager works on Mac, PaidContentCache will too — they share the path resolution. Verify by checking the log line `Initializing PaidContentCache at: ...` on Mac startup.

### Polish Risk 2: CefResponse::HeaderMap iteration order (very low)

The cache stores response headers as a JSON object via `HeadersToJson`. Multimap iteration order is implementation-defined; we concatenate same-name values with `, ` to merge. Should produce equivalent output on both platforms but worth a sanity check that cached pages render identically on Mac after a reload.

### Polish Risk 3: `frame->ExecuteJavaScript` for `window.location.replace` (low)

The back-button history fix uses `frame->ExecuteJavaScript("window.location.replace(...)", frame->GetURL(), 0)` on UI thread. CEF's ExecuteJavaScript is cross-platform but works on the live document; if `/payment-pending` hasn't fully committed when approval fires, the JS might execute against an old document. Worth verifying on Mac by checking the log: `💰 BRC-121: location.replace ...` should appear after approval and the tab should navigate to the article without the placeholder lingering.

## macOS smoke test plan — polish coverage

After running the Phase 1 baseline smoke (above), run these polish-specific checks:

### Cache (5 minutes)
1. Visit `https://now.bsvblockchain.tech/articles/<slug>` → article loads, log shows `PaidContentCache PUT: <url> (... bytes)`.
2. Soft reload (Cmd+R) → article renders **without** `payment_success_indicator fired` in the log; cache hit shows `💰 PaidContentCache HIT: serving ... bytes from disk for <url>`. No new tx.
3. Hard reload (Cmd+Shift+R) → log does NOT show cache HIT (Chromium adds `Cache-Control: no-cache`); article re-fetches, re-pays, re-broadcasts.
4. Settings → Privacy → toggle "Cache paid content" off → soft reload → re-pays.
5. Cache & Storage → "Clear Paid Content" → soft reload → re-pays.

### Placeholder + reject (3 minutes)
1. Revoke the domain in Approved Sites.
2. Visit a paid article → expect dark Hodos placeholder (small spinning gold logo top-left + "Waiting for your approval"), modal centered. **NO** "Failed to load" text. Log: `💰 BRC-121: swapping failed-load for placeholder (...)`.
3. Click Reject → tab navigates back (or `about:blank` if no history). Log: `💰 BRC-121: rejected — going back`.

### Auto-retry + failure page + Try Again with reuse (5 minutes)
1. Revoke the domain again. Visit a paid article. Approve.
2. If upstream returns 431 (Cloudflare flakiness), expect log: `💰 BRC-121: server returned status=431 — auto-retry 1/1 for ...`. ~50% of the time the retry succeeds and the article loads silently.
3. If both attempts fail (rare), tab navigates to `/payment-failed`. Page shows "{domain} rejected the payment. Your sats are safe..." with Try Again + Go Back buttons.
4. Click Try Again **within 25 seconds** → log: `💸 pay_402 REUSE: returning existing nosend tx <txid> for <url> (... sats, age <Nms>)`. Same txid as the failed attempt. No new nosend tx in the wallet activity.
5. Click Try Again **after 30+ seconds** → log: `💸 pay_402` (no REUSE line). New txid minted. Acceptable — past freshness window.

### WalletStatusCache (passive — observe in production)
- Look for `💰 BRC-121: no wallet — falling through to native 402` in the log when a 402 is detected. Before the polish fix, this could persist for 30 s after a single transient timeout. After the fix, only persists 2 s if the cause was a fetch failure (timeout, network error). Hard to trigger deliberately on Mac — observe over time.

## Things NOT new on Mac

- Notification overlay path is unchanged — same `BRC100AuthOverlayRoot.tsx` handles the modal.
- IPC routing (`paid_cache_clear`, `paid_cache_get_size`) goes through the same `simple_handler.cpp::OnProcessMessageReceived` dispatch + render-process JS injection as cookie/cache IPCs — both already work on Mac.
- React Router routes (`/payment-pending`, `/payment-failed`) are eager-loaded, so no chunk-fetch race on first hit.
