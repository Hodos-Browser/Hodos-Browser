# macOS Port — Progress Dashboard

**Branch A**: `macos-port/http-backend` (Person A — HTTP/backend plumbing)
**Branch B**: `macos-port/ui-overlays` (Person B — UI/overlays)
**Base**: `john` (synced to `origin/main` at `7d7f287`)
**Master Tracking**: GitHub Issue #31

---

## Milestone 1: Wallet Functional (Target: Mar 16)

| # | Issue | Track | Status | Started | Completed |
|---|-------|-------|--------|---------|-----------|
| 7 | SyncHttpClient abstraction | A | DONE | 2026-03-09 | 2026-03-09 |
| 8 | Port 4 WinHTTP singletons | A | DONE | 2026-03-09 | 2026-03-09 |
| 27 | Port health check functions | A | DONE | 2026-03-09 | 2026-03-09 |
| 9 | Enable HttpRequestInterceptor on macOS | A | DONE | 2026-03-09 | 2026-03-09 |
| 28 | **GATE**: Wallet operations verified | — | NEEDS USER TEST | — | — |

## Milestone 2: Full Single-Window (Target: Mar 28)

| # | Issue | Track | Status | Started | Completed |
|---|-------|-------|--------|---------|-----------|
| 10 | Keyboard shortcuts (Cmd) | B | NOT STARTED | — | — |
| 11 | NSEvent click-outside detection | B | NOT STARTED | — | — |
| 12 | AdblockCache HTTP stubs | A | DONE | 2026-03-09 | 2026-03-09 |
| 13 | Singleton initialization | A | DONE | 2026-03-09 | 2026-03-09 |
| 14 | Process auto-launch | A | DONE | 2026-03-09 | 2026-03-09 |
| 15 | Menu overlay | B | BLOCKED by #11 | — | — |
| 16 | Omnibox overlay | B | BLOCKED by #11 | — | — |
| 17 | Cookie Panel overlay | B | BLOCKED by #11 | — | — |
| 18 | Download Panel overlay | B | BLOCKED by #11 | — | — |
| 19 | Profile Panel overlay | B | BLOCKED by #11 | — | — |
| 20 | Notification overlay | ~~B~~ A | DONE | 2026-03-09 | 2026-03-09 |
| 29 | **GATE**: Full single-window verified | — | NOT STARTED | — | — |

## Milestone 3: Multi-Window (Target: Apr 11)

| # | Issue | Track | Status | Started | Completed |
|---|-------|-------|--------|---------|-----------|
| 21 | BrowserWindow macOS members | B | BLOCKED by M2 | — | — |
| 22 | WindowManager::CreateFullWindow | B | BLOCKED by #21 | — | — |
| 23 | Tab reparenting + ghost window | B | BLOCKED by #22 | — | — |
| 24 | Merge detection + close behavior | B | BLOCKED by #22 | — | — |
| 30 | **GATE**: Multi-window verified | — | NOT STARTED | — | — |

## Milestone 4: Release Ready (Target: Apr 25)

| # | Issue | Track | Status | Started | Completed |
|---|-------|-------|--------|---------|-----------|
| 25 | Integration testing | Both | BLOCKED by M3 | — | — |
| 26 | Code signing + distribution | Both | BLOCKED by #25 | — | — |

---

## Velocity

| Date | Session | Issues Completed | Notes |
|------|---------|-----------------|-------|
| 2026-03-09 | Setup | 0 | Created 25 GitHub issues, 4 milestones, branch structure |
| 2026-03-09 | Track A - S1 | 7 | #7, #8, #27, #9, #12, #13, #14 — ALL Track A issues complete! |
| 2026-03-09 | Track A - S2 | 1 | #20 (Notification overlay) — fixed wallet hang, moved from Track B to A |
| 2026-03-09 | Track A - S3 | 0 | Cross-platform hardening: overlay IPC, Identity/BRC-100 API, path separators, devtools fix |

**Average velocity**: 8 issues/3 sessions
**Milestone 1 code**: COMPLETE (pending user test)
**Milestone 2 Track A**: COMPLETE (all 3 Track A issues + notification overlay + hardening)
**Track A status**: ALL DONE. 6 commits pushed. Only Track B overlay issues remain for M2.

---

## Current Focus

**Person A (this branch)**: ALL TRACK A ISSUES COMPLETE + NOTIFICATION OVERLAY
- SyncHttpClient abstraction (WinHTTP + libcurl)
- 4 wallet singletons ported to SyncHttpClient
- Health check + shutdown functions added to macOS
- HttpRequestInterceptor enabled on macOS
- AdblockCache 3 endpoints ported
- 4 singletons initialized (AdblockCache, FingerprintProtection, CookieBlockManager, BookmarkManager)
- Process auto-launch (posix_spawn wallet + adblock) with graceful shutdown
- Security framework linked for SecRandomCopyBytes
- Notification overlay (domain approval, no-wallet, payment confirmation) — fixed wallet hang
- handleAuthResponse calls now cross-platform
- Overlay IPC handlers enabled on macOS (overlay_hide, overlay_hide_settings, overlay_input)
- sendAuthRequestDataToOverlay enabled on macOS
- Identity API + BRC-100 API enabled in render process (was Windows-only stub)
- Path separator fix for CookieBlockManager and BookmarkManager DB paths
- DevTools popup fix (SetAsPopup Windows-only)

**Person B**: Starting on #10 (keyboard shortcuts) and #11 (click-outside detection)

---

## Files Changed (Track A, this session)

| File | Change |
|------|--------|
| `include/core/SyncHttpClient.h` | NEW — cross-platform HTTP client header |
| `src/core/SyncHttpClient.cpp` | NEW — WinHTTP (Windows) + libcurl (macOS) implementations |
| `src/core/HttpRequestInterceptor.cpp` | Replaced 4 `#else` stubs with SyncHttpClient calls, fixed include path |
| `src/handlers/simple_handler.cpp` | Removed `#ifdef _WIN32` guard around HttpRequestInterceptor, added include to `__APPLE__` block |
| `include/core/AdblockCache.h` | Replaced 3 macOS stubs with SyncHttpClient::Post() implementations, moved escapeJson cross-platform |
| `cef_browser_shell_mac.mm` | Added SyncHttpClient include, QuickHealthCheck(), QuickAdblockHealthCheck(), SendShutdownRequest() |
| `include/core/AdblockCache.h` | Replaced 3 macOS stubs, moved escapeJson cross-platform, added SyncHttpClient include |
| `cef_browser_shell_mac.mm` | Added SyncHttpClient + singleton includes, health checks, server launch/stop, singleton init |
| `CMakeLists.txt` | Added SyncHttpClient.cpp + HttpRequestInterceptor.cpp to macOS, linked Security framework |

---

## Risk Log

| Risk | Status | Mitigation |
|------|--------|------------|
| CEF OSR keyboard input on macOS overlays | OPEN | Copy wallet overlay pattern exactly |
| NSEvent click-outside (no WH_MOUSE_LL) | OPEN | Use addLocalMonitorForEventsMatchingMask: |
| Multi-window tab reparenting crashes | OPEN | Careful CEF lifecycle management |
| Code signing requires Apple Developer ID | OPEN | Need $99/yr account |
| Merge conflicts in shared files | LOW | Track A touched different regions than Track B will |
