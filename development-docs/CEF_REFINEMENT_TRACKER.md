# CEF Refinement — Phase Tracker & Reference Guide

**Created**: 2026-02-11
**Last Updated**: 2026-02-11
**Purpose**: Phased checklist for stability, security, and architecture improvements to the C++ CEF browser shell, HTTP interceptor, overlay rendering, and C++/Rust communication layer.

**How to use this document**: Each CEF Refinement (CR) phase has a checklist. Check items off as they are implemented. UX_UI phase docs reference this tracker so that pre-phase planning considers the relevant CR prerequisites.

---

## Phase Summary

| Phase | Name | Scope | Status | UX Dependency |
|-------|------|-------|--------|---------------|
| **CR-1** | Critical Stability & Security | JS injection, hangs, buffer overflow, auth fixes | ✅ Complete | Do before or alongside UX Phase 0 |
| **CR-2** | Interceptor Architecture | Async wallet calls, per-request map, whitelist cache, thread safety | 📋 Planning | Must complete before UX Phase 2 |
| **CR-3** | Polish & Lifecycle | Overlay lifecycle, weak refs, debug cleanup, error status codes | 📋 Planning | Alongside UX Phase 2–3 |

**Status Legend:** 📋 Planning | 🔨 In Progress | ✅ Complete

---

## CR-1: Critical Stability & Security

**Goal**: Fix security vulnerabilities, crashes, and request hangs that affect the browser today.
**Effort**: ~1 day
**When**: Before any UX phase work begins.

### Checklist

- [x] **CR-1.1 — JS injection fix** (SECURITY) ✅ 2026-02-12
  - **Files**: `HttpRequestInterceptor.cpp:334-353,387-405`, `simple_render_process_handler.cpp:819-831`
  - **Problem**: `triggerDomainApprovalModal()` and `triggerBRC100AuthApprovalModal()` pass unescaped domain/body strings into `ExecuteJavaScript()`. A malicious site can inject JS with full wallet access via crafted POST body.
  - **Fix**: Added `escapeForJsSingleQuote()` helper in HttpRequestInterceptor.cpp. All interpolated values (domain, method, endpoint, body) wrapped.

- [x] **CR-1.2 — Rejected auth error response** (HANG) ✅ 2026-02-12
  - **File**: `simple_handler.cpp:1972-1978`
  - **Problem**: When user rejects auth, `g_pendingAuthRequest.isValid = false` is set but no error response is sent. `readCallback_->Continue()` is never called. Website request hangs forever.
  - **Fix**: Rejection branch now calls `handleAuthResponse()` with error JSON, which routes through existing handler path.

- [x] **CR-1.3 — Missing WasResized for BRC-100 auth overlay** (BLACK SCREEN) ✅ 2026-02-12
  - **File**: `simple_handler.cpp:585-595`
  - **Problem**: `OnAfterCreated` for `role_ == "brc100auth"` lacks the delayed `WasResized()`/`Invalidate()` task that every other overlay has. Auth overlay can render as black screen on first paint.
  - **Fix**: Added `CefPostDelayedTask` with `WasResized()`/`Invalidate(PET_VIEW)` at 150ms, matching other overlays.

- [x] **CR-1.4 — OnPaint buffer overflow** (CRASH) ✅ 2026-02-12
  - **File**: `my_overlay_render_handler.cpp:22-74,140-162`
  - **Problem**: Bitmap allocated at construction with fixed `width_`/`height_`. On resize, `OnPaint` receives new dimensions but bitmap is not reallocated. `memcpy` overflows if window grows.
  - **Fix**: Added dimension check before memcpy. If dimensions changed, old bitmap deleted and new one allocated with correct size.

- [x] **CR-1.5 — Wallet HTTP request timeout** (HANG) ✅ 2026-02-12
  - **File**: `HttpRequestInterceptor.cpp:773`
  - **Problem**: `CefURLRequest::Create()` to `localhost:3301` has no timeout. Hung wallet server = indefinite hang.
  - **Fix**: Added `std::atomic<bool> httpCompleted_` flag. 15s `CefPostDelayedTask` timer sends error and cancels request if not completed.

- [x] **CR-1.6 — Auth approval wait timeout** (HANG) ✅ 2026-02-12
  - **File**: `HttpRequestInterceptor.cpp:193-206`
  - **Problem**: Request pauses for user interaction. No timeout if user never responds, closes overlay, or overlay fails to load.
  - **Fix**: Added 60s `CefPostDelayedTask` timeout for both BRC-100 auth and domain approval modals. Reuses `httpCompleted_` flag.

- [x] **CR-1.7 — Domain approval handler not stored** (HANG) ✅ 2026-02-12
  - **File**: `HttpRequestInterceptor.cpp:329`
  - **Problem**: `triggerDomainApprovalModal()` sets `handler = nullptr`. `handleAuthResponse()` checks `handler != nullptr` and skips. All non-BRC-100 domain approvals fail to resume the original request.
  - **Fix**: Changed `handler = nullptr` to `handler = this` (valid implicit upcast to `CefResourceHandler`).

---

## CR-2: Interceptor Architecture

**Goal**: Refactor the HTTP interceptor for concurrent requests, thread safety, and proper async patterns. These are prerequisites for UX Phase 2 (User Notifications) which adds multiple notification types that all need concurrent pending request support.
**Effort**: ~4-5 days
**When**: After UX Phase 1, before UX Phase 2.

### Checklist

- [ ] **CR-2.1 — Move wallet HTTP calls off UI thread** (BLACK SCREEN ROOT CAUSE)
  - **File**: `simple_handler.cpp` — 7 call sites (lines 1213, 1257, 2190, 2234, 2278, 2365, 2504)
  - **Problem**: All `WalletService` calls are synchronous on CEF UI thread using WinHTTP. Slow wallet (broadcast timeout, DB lock) freezes entire browser — no painting, no input, all overlays go black.
  - **Fix**: Move each call to `CefPostTask(TID_IO, ...)` or `std::async`, post response back via `CefPostTask(TID_UI, ...)`.
  - **Also fix**: `WalletService.cpp` — add `WinHttpSetTimeouts` (15s connect, 30s send/receive).
  - **Effort**: 2-3 days (7 call sites, each needs async refactor + IPC response routing)

- [ ] **CR-2.2 — Replace `g_pendingAuthRequest` with per-request map**
  - **Files**: `PendingAuthRequest.h`, `HttpRequestInterceptor.cpp:22-25`, all consumers
  - **Problem**: Single global struct. Concurrent requests from different tabs clobber each other. Second request silently drops the first.
  - **Fix**: `std::map<uint64_t, PendingAuthRequest>` keyed by request ID. Update all consumers.
  - **Effort**: 4-6 hrs

- [ ] **CR-2.3 — Add mutex on interceptor global state**
  - **File**: `HttpRequestInterceptor.cpp:22-25`
  - **Problem**: `g_pendingAuthRequest` and `g_pendingModalDomain` accessed from IO and UI threads with no synchronization. Data race.
  - **Fix**: `std::mutex` protecting the pending request map, or ensure all access via `CefPostTask` to a single thread.
  - **Effort**: 1-2 hrs

- [ ] **CR-2.4 — Cache whitelist in memory**
  - **File**: `HttpRequestInterceptor.cpp:56-156` (`DomainVerifier`)
  - **Problem**: `DomainVerifier` reads `domainWhitelist.json` from disk on every intercepted request. Race with Rust-side writes causes "modal shows for whitelisted sites" bug.
  - **Fix**: Load whitelist into in-memory cache at startup. Update cache when whitelist changes (notification from Rust or file watcher). Eliminate per-request disk I/O.
  - **Note**: UX Phase 2 may move whitelist to DB entirely, which supersedes this. If Phase 2 is close, a simpler interim fix (add domain to C++ cache immediately on approval, before Rust round-trip) may suffice.
  - **Effort**: 4-6 hrs

- [ ] **CR-2.5 — Fix `requestCompleted_` / `readCallback_` thread race**
  - **File**: `HttpRequestInterceptor.cpp:250-307`
  - **Problem**: `onHTTPResponseReceived()` (UI thread) and `ReadResponse()` (IO thread) access shared state with no synchronization. Race can cause response to never be delivered.
  - **Fix**: Add `std::mutex` or `std::atomic` for `requestCompleted_`, and ensure `readCallback_->Continue()` is always called on the IO thread.
  - **Effort**: 3-4 hrs

- [ ] **CR-2.6 — Fix raw pointer in `AsyncHTTPClient`**
  - **File**: `HttpRequestInterceptor.cpp:611`
  - **Problem**: Raw `AsyncWalletResourceHandler*` bypasses CefRefPtr ref-counting. Use-after-free if handler destroyed before HTTP response.
  - **Fix**: Store `CefRefPtr<AsyncWalletResourceHandler>` instead of raw pointer.
  - **Effort**: 1 hr

---

## CR-3: Polish & Lifecycle

**Goal**: Fix overlay lifecycle issues, improve error reporting, clean up debug artifacts. Lower urgency — do alongside UX Phase 2-3 when touching these files anyway.
**Effort**: ~3-4 days total (can be spread across sprints)
**When**: Alongside UX Phase 2 and Phase 3.

### Checklist

- [ ] **CR-3.1 — Move whitelist to DB** (eliminates JSON file)
  - **Note**: Planned as part of UX Phase 2. Eliminates the C++/Rust dual-system mismatch entirely. C++ queries Rust server (via cached in-memory state) instead of reading a file.
  - **Effort**: Part of UX Phase 2

- [ ] **CR-3.2 — Adopt per-request context struct** (Brave `BraveRequestInfo` pattern)
  - **Problem**: Request state scattered across globals and handler members.
  - **Fix**: Bundle domain, method, endpoint, whitelisted status, handler ref into a single struct carried through the request lifecycle. Natural to build as UX Phase 2 adds new notification types.
  - **Effort**: Medium (part of Phase 2 work)

- [ ] **CR-3.3 — Add weak references for deferred callbacks**
  - **File**: `HttpRequestInterceptor.cpp` — `URLRequestCreationTask`, `DomainWhitelistTask`
  - **Problem**: Raw pointers passed between threads. Use-after-free if tab closes while request pending.
  - **Fix**: Use weak references (`CefRefPtr` + validity check) for deferred callbacks.
  - **Effort**: 3-4 hrs

- [ ] **CR-3.4 — Fix overlay browser lifecycle (close-before-destroy)**
  - **File**: `cef_browser_shell.cpp:567-572`, `ShutdownApplication()` (lines 187-215)
  - **Problem**: `CloseBrowser(false)` is async but `DestroyWindow()` called immediately. Leaks browser objects and renderer processes.
  - **Fix**: Wait for `OnBeforeClose` before destroying HWND, or use `CloseBrowser(true)` for cleanup paths.
  - **Effort**: 4-6 hrs

- [ ] **CR-3.5 — Remove debug overlay from production**
  - **File**: `simple_app.cpp:249-348`
  - **Problem**: `InjectHodosBrowserAPI` creates a visible debug div (z-index 9999, black background) visible to users.
  - **Fix**: Gate behind a debug flag or remove entirely.
  - **Effort**: 15 min

- [ ] **CR-3.6 — Fix `GetResponseHeaders` always returning 200**
  - **File**: `HttpRequestInterceptor.cpp:231`
  - **Problem**: Status hardcoded to 200 even on wallet errors. Websites can't detect errors via HTTP status.
  - **Fix**: Store actual status from wallet response and return it.
  - **Effort**: 1-2 hrs

- [ ] **CR-3.7 — Fix settings overlay stale pointer**
  - **File**: `simple_app.cpp:396-401`
  - **Problem**: Settings overlay is destroyed and recreated every toggle. Between `DestroyWindow` and new `OnBeforeClose`, `GetSettingsBrowser()` returns stale pointer.
  - **Fix**: Either adopt keep-alive pattern (like omnibox) or null the browser reference immediately on destroy.
  - **Effort**: 1-2 hrs

- [ ] **CR-3.8 — Reduce WalletService debug logging I/O**
  - **File**: `WalletService.cpp` throughout
  - **Problem**: Opens/closes `debug_output.log` dozens of times per request. Performance drag.
  - **Fix**: Use a singleton logger or buffer writes.
  - **Effort**: 1-2 hrs

- [ ] **CR-3.9 — Restrict localhost port redirection**
  - **File**: `HttpRequestInterceptor.cpp:806-825`
  - **Problem**: Regex `localhost:\d{4}` redirects any 4-digit port to 3301. Could expose wallet to unintended callers.
  - **Fix**: Only redirect known BRC-100 convention ports or exact match.
  - **Effort**: 30 min

- [ ] **CR-3.10 — macOS OnPaint use-after-free** (macOS only)
  - **File**: `my_overlay_render_handler.cpp:255-259`
  - **Problem**: `dispatch_async` captures `CGImageRef` referencing CEF's buffer via non-copying data provider. Potential use-after-free.
  - **Fix**: Copy buffer data before dispatch_async.
  - **Effort**: 1 hr

---

## Reference: Whitelist Race Condition — Root Cause

### The Bug: Whitelisted Sites Still Show Auth Modal

**Concrete reproduction scenario:**

1. User visits `app.example.com` for the first time
2. Site makes `POST /.well-known/auth` — CEF intercepts, reads `domainWhitelist.json` (not found), shows modal
3. User approves with "whitelist" checked
4. Two things happen in parallel:
   - Auth response is forwarded to the site
   - `add_domain_to_whitelist` message is sent → posted to UI thread → HTTP POST to Rust → Rust updates in-memory cache → Rust writes to disk
5. Site immediately makes a second request (e.g., `POST /createAction`)
6. CEF intercepts, reads `domainWhitelist.json` from disk
7. **The Rust server has not yet written the file** (the HTTP request is still in flight)
8. Domain not found in file → **modal shows again**

### Contributing factors

- C++ reads from disk every time (no cache) — fixed by CR-2.4
- Rust caches in memory (fast) but writes to disk asynchronously
- C++ whitelist add is asynchronous (posted to UI thread, then HTTP to Rust)
- No acknowledgment flow — C++ doesn't wait for confirmation
- C++ and Rust have different `isPermanent` / `requestCount` semantics — eliminated by CR-3.1 (move to DB)

---

## Reference: Brave Browser Architecture Comparison

| Aspect | Brave | Hodos |
|--------|-------|-------|
| **HTTP interception** | Chromium network delegate (pre-connection) | CEF resource handler (post-routing) |
| **Wallet process** | In browser process (same PID) | Separate process on :3301 |
| **Wallet UI** | WebUI in browser process | CEF subprocess overlay |
| **IPC** | Mojo (zero-copy, type-safe) | HTTP JSON + CEF IPC messages |
| **C++ to Rust** | FFI via `cxx` crate (nanoseconds) | HTTP over loopback (milliseconds) |
| **Permission storage** | In-memory `HostContentSettingsMap` (disk-backed) | JSON file on disk (re-read every time) |
| **Pending requests** | Map keyed by `request_identifier` | Single global struct → per-request map (CR-2.2) |
| **Crash isolation** | Wallet crash = browser crash | Wallet crash = wallet down, browser survives |
| **Async approval** | Weak-pointer-based continuations | Raw pointer → weak refs (CR-3.3) |

### Key Lessons from Brave

1. **Per-request context struct** — Bundle all request state into a shared struct passed through the callback chain (CR-3.2)
2. **Callback chain pattern** — Extensible vector of callbacks for Phase 2 notification types
3. **Snapshot-at-request-start** — Read permission state once and carry it in context (prevents TOCTOU)
4. **In-memory permission cache** — Memory lookups, not disk I/O (CR-2.4)
5. **Weak references** — Prevent use-after-free on tab close (CR-3.3)
6. **Hodos's separate-process wallet is an advantage** — Do NOT compromise this architecture

---

## Reference: UX Phase 2 Notes

### Site Refresh After Auth Modal

> When a `.well-known/auth` call times out on the site's end while the user is approving via modal, the site may show an error state. After the user approves, trigger a **site refresh** to let the site retry its auth handshake.
>
> **When to refresh**: After approval of `.well-known/auth` or identity-related requests. NOT after `createAction` (would disrupt transaction flow).
>
> **Implementation**: After `handleAuthResponse()` completes for auth-type requests, send a message to the tab's browser to execute `location.reload()` or dispatch a custom event.

### Request Queue for Phase 2

The single-global `g_pendingAuthRequest` must be replaced (CR-2.2) before Phase 2. Phase 2 adds payment, signing, encryption, and certificate notifications — all need concurrent pending request support.

### Whitelist → DB Migration

When whitelist moves to DB in Phase 2 (CR-3.1):
- Eliminate the JSON file entirely
- C++ queries Rust server via cached in-memory state
- Single source of truth — no C++/Rust semantics mismatch
- Proper DB-level concurrency

---

*End of document.*
