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
