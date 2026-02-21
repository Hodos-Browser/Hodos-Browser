# CEF Refinement — Phase Tracker & Reference Guide

**Created**: 2026-02-11
**Last Updated**: 2026-02-19
**Purpose**: Phased checklist for stability, security, and architecture improvements to the C++ CEF browser shell, HTTP interceptor, overlay rendering, and C++/Rust communication layer.

**How to use this document**: Each CEF Refinement (CR) phase has a checklist. Check items off as they are implemented. UX_UI phase docs reference this tracker so that pre-phase planning considers the relevant CR prerequisites.

---

## Phase Summary

| Phase | Name | Scope | Status | UX Dependency |
|-------|------|-------|--------|---------------|
| **CR-1** | Critical Stability & Security | JS injection, hangs, buffer overflow, auth fixes | ✅ Complete | Do before or alongside UX Phase 0 |
| **CR-2** | Interceptor Architecture | Async wallet calls, per-request map, whitelist cache, thread safety | ✅ Complete | Completed during UX Phase 2 |
| **CR-3** | Polish & Lifecycle | Overlay lifecycle, weak refs, debug cleanup, error status codes | ✅ Mostly Complete | 13/15 done, 2 partial (by design) |

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
  - **Fix**: Added `std::atomic<bool> httpCompleted_` flag with `compare_exchange_strong` guard in both response handlers (prevents double `readCallback_->Continue()` crash). 45s `CefPostDelayedTask` timer sends error and cancels request if not completed. `WalletTimeoutTask` uses `CefRefPtr` (not raw pointer) to prevent use-after-free on delayed fire.

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

- [x] **CR-2.1 — Move wallet HTTP calls off UI thread** (BLACK SCREEN ROOT CAUSE) ✅ 2026-02-16
  - **File**: `HttpRequestInterceptor.cpp` — `AsyncWalletResourceHandler` + `CefURLRequest` on IO thread
  - **Solution**: All BRC-100 wallet calls now route through async `CefURLRequest` on IO thread via `StartAsyncHTTPRequestTask`. No more WinHTTP on UI thread.

- [x] **CR-2.2 — Replace `g_pendingAuthRequest` with per-request map** ✅ 2026-02-16
  - **Files**: `PendingAuthRequest.h` — `PendingRequestManager` singleton with `std::map<std::string, PendingAuthRequest>` keyed by unique requestId (`req-{timestamp}-{counter}`).
  - **Solution**: Thread-safe map with `addRequest`, `popRequest`, `getRequest`, `hasPendingForDomain`, `getRequestIdForDomain`. All consumers updated.

- [x] **CR-2.3 — Add mutex on interceptor global state** ✅ 2026-02-16
  - **Solution**: `std::mutex` + `std::lock_guard` on all `PendingRequestManager` methods. Thread-safe by construction.

- [x] **CR-2.4 — Cache whitelist in memory** ✅ 2026-02-16
  - **Solution**: `DomainVerifier` removed entirely. Replaced by `DomainPermissionCache` singleton — in-memory cache backed by Rust DB via sync WinHTTP with `invalidate(domain)` on permission changes. JSON file eliminated.

- [x] **CR-2.5 — Fix `requestCompleted_` / `readCallback_` thread race** ✅ 2026-02-16
  - **Solution**: `std::atomic<bool> httpCompleted_` with `compare_exchange_strong` in both response and timeout paths. Prevents double `readCallback_->Continue()` crash.

- [x] **CR-2.6 — Fix raw pointer in `AsyncHTTPClient`** ✅ 2026-02-16
  - **Solution**: `CefRefPtr<AsyncWalletResourceHandler> parent_` — prevents use-after-free if handler destroyed before HTTP response.

---

## CR-3: Polish & Lifecycle

**Goal**: Fix overlay lifecycle issues, improve error reporting, clean up debug artifacts. Lower urgency — do alongside UX Phase 2-3 when touching these files anyway.
**Effort**: ~3-4 days total (can be spread across sprints)
**When**: Alongside UX Phase 2 and Phase 3.

### Checklist

- [x] **CR-3.1 — Move whitelist to DB** (eliminates JSON file) ✅ 2026-02-16
  - **Solution**: `domain_whitelist.rs` deleted. `domain_permissions` table in SQLite with full CRUD. `DomainPermissionCache` in C++ queries Rust DB. JSON file eliminated entirely.

- [ ] **CR-3.2 — Adopt per-request context struct** (Brave `BraveRequestInfo` pattern) — PARTIAL
  - **Status**: State spread across `AsyncWalletResourceHandler` + `SessionManager`. No unified context struct yet, but per-request tracking via `PendingRequestManager` covers most use cases. Revisit if complexity grows.

- [x] **CR-3.3 — Add weak references for deferred callbacks** ✅ 2026-02-16
  - **Solution**: All deferred callbacks use `CefRefPtr` (ref-counted). `URLRequestCreationTask` replaced by `StartAsyncHTTPRequestTask` with proper ref counting.

- [x] **CR-3.4 — Fix overlay browser lifecycle (close-before-destroy)** ✅ 2026-02-17
  - **Solution**: `OnBeforeClose` properly nullifies browser refs. HWND cleanup gated on browser close. Notification overlay uses keep-alive pattern (no destroy/recreate cycle).

- [x] **CR-3.5 — Remove debug overlay from production** ✅ 2026-02-16
  - **Solution**: Debug overlay not enabled in production paths.

- [ ] **CR-3.6 — Fix `GetResponseHeaders` always returning 200** — PARTIAL (by design)
  - **Status**: Hardcoded 200 is intentional — errors are returned in JSON body (BRC-100 convention). Could revisit for HTTP standards compliance but not blocking.

- [x] **CR-3.7 — Fix settings overlay stale pointer** ✅ 2026-02-17
  - **Solution**: HWND validity check + explicit `nullptr` on destroy. Keep-alive pattern adopted for notification overlay.

- [x] **CR-3.8 — Reduce WalletService debug logging I/O** ✅ 2026-02-12
  - **Solution**: Singleton `Logger` class with minimal conditional logging.

- [x] **CR-3.9 — Restrict localhost port redirection** ✅ 2026-02-16
  - **Solution**: Only redirects localhost/127.0.0.1 → 3301. External BRC-104 passes through unmodified.

- [ ] **CR-3.10 — macOS OnPaint use-after-free** (macOS only) — N/A
  - **Status**: No macOS build in production yet. Will be addressed during macOS sprint.

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
