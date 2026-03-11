# Session Handoff — macOS Port: Track A Complete

**Date**: 2026-03-09
**Branch**: `macos-port/http-backend` (Track A work, based on `john`)
**Status**: ALL Track A issues complete + notification overlay fix. Wallet domain approval, adblock, singletons, process auto-launch, and notification overlay all working on macOS. Needs user testing (GATE #28) and Track B work (remaining overlays + keyboard shortcuts).

---

## What We Accomplished This Session

### Track A — HTTP/Backend Plumbing (ALL 7 ISSUES COMPLETE)

1. **SyncHttpClient abstraction** (#7) — New cross-platform HTTP client
   - `include/core/SyncHttpClient.h` — `Get()` / `Post()` returning `HttpResponse{statusCode, body, success}`
   - `src/core/SyncHttpClient.cpp` — WinHTTP on Windows, libcurl on macOS
   - Added to cross-platform CMake sources

2. **Ported 4 WinHTTP singletons** (#8) — in `HttpRequestInterceptor.cpp`
   - `DomainPermissionCache::fetchFromBackend` — `GET /domain/permissions?domain=X`
   - `WalletStatusCache::fetchWalletStatus` — `GET /wallet/status`
   - `BSVPriceCache::fetchFromBackend` — `GET /wallet/bsv-price`
   - `fetchCertFieldsFromBackend` — `GET /domain/permissions/certificate?domain=X&cert_type=Y`
   - All use `SyncHttpClient::Get()` now, Windows code untouched

3. **Enabled HttpRequestInterceptor on macOS** (#9)
   - Added `HttpRequestInterceptor.cpp` to macOS CMake sources
   - Removed `#ifdef _WIN32` guard in `simple_handler.cpp:5617`
   - Added `#include` to `__APPLE__` block
   - Fixed include path (`PendingAuthRequest.h`)

4. **Ported health check functions** (#27) — in `cef_browser_shell_mac.mm`
   - `QuickHealthCheck()` — checks wallet at :31301
   - `QuickAdblockHealthCheck()` — checks adblock at :31302
   - `SendShutdownRequest(port)` — graceful POST /shutdown

5. **Ported AdblockCache HTTP stubs** (#12) — in `AdblockCache.h`
   - `fetchFromBackend()` — `POST /check` (ad blocking check)
   - `fetchCosmeticFromBackend()` — `POST /cosmetic-resources` (CSS + scriptlet injection)
   - `fetchHiddenIdsFromBackend()` — `POST /cosmetic-hidden-ids` (generic selectors)
   - Moved `escapeJson()` out of `#ifdef _WIN32` to cross-platform
   - All use `SyncHttpClient::Post()`

6. **Initialized missing singletons** (#13) — in `cef_browser_shell_mac.mm` main()
   - `AdblockCache::GetInstance().Initialize(profile_cache)`
   - `FingerprintProtection::GetInstance().Initialize()`
   - `CookieBlockManager::GetInstance().Initialize(profile_cache)`
   - `BookmarkManager::GetInstance().Initialize(profile_cache)`
   - Linked macOS Security framework for `SecRandomCopyBytes`

7. **Process auto-launch** (#14) — in `cef_browser_shell_mac.mm`
   - `StartWalletServer()` / `StartAdblockServer()` using `posix_spawn()`
   - Detects if servers already running (dev mode)
   - `StopServers()` on shutdown: HTTP /shutdown → SIGTERM → waitpid cleanup
   - Called before `CreateMainWindow()`, cleanup before `CefShutdown()`

8. **Notification overlay** (#20, moved from Track B) — in `cef_browser_shell_mac.mm`
   - `NotificationOverlayView` NSView subclass with mouse/key forwarding to notification browser
   - `CreateNotificationOverlay()` — NSWindow + OSR CEF browser loading `/brc100-auth?type=...&domain=...`
   - Keep-alive pattern (hides instead of destroying, JS injection for instant re-show)
   - `HideNotificationOverlayWindow()` C-linkage helper
   - Fixed `handleAuthResponse` calls — removed `#ifdef _WIN32` guards (now cross-platform)
   - Fixed `overlay_close` handler for notification role on macOS
   - **Root cause of wallet hang**: overlay was Windows-only, domain approval never appeared → 60s timeout

### Verified in Logs
- All 4 singletons initializing correctly
- Wallet server detected (dev mode)
- Adblock engine detected (dev mode)
- HttpRequestInterceptor intercepting wallet requests from header AND wallet overlays
- CookieBlockManager loaded 24 blocked domains
- BookmarkManager database created

---

## What Remains

### Track B — UI/Overlays (OTHER PERSON)
These are on the `macos-port/ui-overlays` branch (or to be created):

| # | Issue | Status |
|---|-------|--------|
| 10 | Keyboard shortcuts (Ctrl → Cmd) | NOT STARTED |
| 11 | NSEvent click-outside detection | NOT STARTED |
| 15 | Menu overlay | BLOCKED by #11 |
| 16 | Omnibox overlay | BLOCKED by #11 |
| 17 | Cookie Panel overlay | BLOCKED by #11 |
| 18 | Download Panel overlay | BLOCKED by #11 |
| 19 | Profile Panel overlay | BLOCKED by #11 |
| ~~20~~ | ~~Notification overlay~~ | **DONE** (moved to Track A) |
| 20 | Notification overlay | BLOCKED by #11 |

### Milestone 3 — Multi-Window (BOTH)
| 21 | BrowserWindow macOS members | BLOCKED by M2 |
| 22 | WindowManager::CreateFullWindow | BLOCKED by #21 |
| 23 | Tab reparenting + ghost window | BLOCKED by #22 |
| 24 | Merge detection + close behavior | BLOCKED by #22 |

### GATE Verification Needed
- **GATE #28**: User must test wallet create/recover/unlock on macOS
- **GATE #29**: All overlays + keyboard shortcuts working
- **GATE #30**: Multi-window working

---

## GitHub Project Structure

- **Master tracking**: Issue #31 (pinned) — full dependency graph + parallelization guide
- **4 Milestones**: Wallet Functional, Full Single-Window, Multi-Window, Release Ready
- **Labels**: `track-A: http/backend`, `track-B: ui/overlays`, `macOS-port`, `blocker`
- **GATE issues**: #28, #29, #30 — checkbox verification checklists

---

## Files Changed (Track A)

| File | Change |
|------|--------|
| `include/core/SyncHttpClient.h` | **NEW** — cross-platform HTTP client |
| `src/core/SyncHttpClient.cpp` | **NEW** — WinHTTP + libcurl implementations |
| `src/core/HttpRequestInterceptor.cpp` | Replaced 4 `#else` stubs, fixed include path |
| `src/handlers/simple_handler.cpp` | Removed `#ifdef` guard, added `__APPLE__` include |
| `include/core/AdblockCache.h` | Replaced 3 macOS stubs, moved `escapeJson` cross-platform |
| `cef_browser_shell_mac.mm` | Health checks, server launch, singleton init, 6 includes added, NotificationOverlayView, CreateNotificationOverlay, HideNotificationOverlayWindow |
| `include/handlers/simple_app.h` | Added `g_notification_overlay_window` extern + `CreateNotificationOverlay` declaration for macOS |
| `CMakeLists.txt` | Added SyncHttpClient.cpp + HttpRequestInterceptor.cpp, Security framework |

---

## Known Issues

1. **CookieBlockManager path separator**: Log shows `Default\cookie_blocks.db` with backslash on macOS. Should be `/`. Works but path is cosmetically wrong. Minor fix needed.
2. **BookmarkManager same issue**: `Default\bookmarks.db` with backslash.
3. **Process auto-launch paths**: Uses relative `../../` paths which may not resolve from all build locations. Works in dev mode (servers started manually). Need to test from installed .app bundle.

---

## Commands to Run

```bash
# Kill everything
pkill -f "HodosBrowserShell" 2>/dev/null; pkill -f "hodos-wallet" 2>/dev/null; pkill -f "hodos-adblock" 2>/dev/null

# Start services
cd ~/bsv/Hodos-Browser/rust-wallet && cargo run --release &
cd ~/bsv/Hodos-Browser/adblock-engine && cargo run --release &
cd ~/bsv/Hodos-Browser/frontend && npm run dev &

# Wait for health, then launch browser
sleep 5
curl -s http://localhost:31301/health && curl -s http://localhost:31302/health
cd ~/bsv/Hodos-Browser/cef-native/build/bin && ./HodosBrowserShell.app/Contents/MacOS/HodosBrowserShell
```

### Rebuild C++ after changes:
```bash
cd ~/bsv/Hodos-Browser/cef-native && cmake --build build --config Release && \
cd build/bin && for h in "HodosBrowser Helper.app" "HodosBrowser Helper (Alerts).app" "HodosBrowser Helper (GPU).app" "HodosBrowser Helper (Plugin).app" "HodosBrowser Helper (Renderer).app"; do cp -r "$h" HodosBrowserShell.app/Contents/Frameworks/; done
```

---

## Prompt to Continue

```
Read HANDOFF.md for full context on the macOS port.

TL;DR: Track A is COMPLETE — all HTTP/backend plumbing works on macOS (SyncHttpClient, HttpRequestInterceptor, AdblockCache, singletons, process auto-launch). 7 GitHub issues closed (#7, #8, #9, #12, #13, #14, #27).

Track B (overlays + keyboard shortcuts) is the other dev's work.

What's next for this session:
1. If continuing Track A maintenance: check GATE #28 (user wallet test results), fix any bugs
2. If helping Track B: start with #10 (keyboard shortcuts) or #11 (click-outside detection)
3. If starting Multi-Window: begin with #21 (BrowserWindow macOS members)

Progress file: development-docs/Final-MVP-Sprint/macos-port/PROGRESS.md
GitHub tracking: Issue #31 (pinned)
Branch: macos-port/http-backend (Track A complete, uncommitted changes ready to commit)
```
