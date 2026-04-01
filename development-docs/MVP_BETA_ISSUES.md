# MVP Beta Issues Tracker

Issues discovered during beta testing. Priority: P0 = must fix before release, P1 = should fix, P2 = nice to have.

---

## Open Issues

### B-1: Windows title bar / shell frame visible (P1)
**Reported:** 2026-03-24 (v0.1.0-beta.1)
**Description:** The default Windows chrome (grey title bar with minimize/maximize/close buttons) is visible above the browser UI. These controls should be integrated into the app's tab bar area, and the native frame should be removed (frameless window).
**Impact:** Looks unprofessional — not like a production browser.
**Fix area:** `cef_browser_shell.cpp` — window creation flags (`WS_OVERLAPPEDWINDOW` → frameless), custom hit-testing for min/max/close, drag region for tab bar.
**Research notes (2026-03-31):**
- Main window is created with `WS_OVERLAPPEDWINDOW` in `cef_browser_shell.cpp` (~line 600+) which includes the native Windows frame (title bar, min/max/close)
- Fix requires: (1) Change to `WS_POPUP` or `WS_OVERLAPPEDWINDOW` minus `WS_CAPTION` for frameless, (2) Implement `WM_NCHITTEST` custom hit-testing so the tab bar area acts as a drag region, (3) Add custom min/max/close buttons to the React header in `MainBrowserView.tsx`, (4) Handle `WM_NCCALCSIZE` to remove non-client area
- Overlay HWNDs already use `WS_POPUP` (no frame) — only the main window has this issue
- Reference: Chromium/Electron frameless window patterns. CEF has no built-in frameless API — must be done at the Win32 level
- Risk: Touches window creation, hit-testing, and drag regions — high sensitivity area. Tab tear-off and multi-window both depend on HWND sizing. Test thoroughly.
- macOS: Not affected — `cef_browser_shell_mac.mm` uses `NSWindow` with `titlebarAppearsTransparent` + `NSFullSizeContentViewWindowMask` (already frameless-like)

### B-2: Startup & shutdown performance + audio continues after close (P1)
**Reported:** 2026-03-24 (v0.1.0-beta.1) | **Expanded:** 2026-04-01
**Description:** Multiple performance issues with startup and shutdown:
1. Audio continues playing for several seconds after browser window closes
2. Shutdown takes 8-12s due to sequential server shutdown before browser close
3. Header HWND loads slower than webview (user sees tab content before toolbar)
4. 888KB single React bundle — all 15 routes eagerly imported
**Impact:** Unprofessional UX — user thinks app didn't close properly. Slow startup feels unpolished.
**Root cause (shutdown):** `ShutdownApplication()` runs `StopWalletServer()` (5-7s) + `StopAdblockServer()` (3-5s) sequentially BEFORE closing CEF browsers. Overlay browsers never call `SetAudioMuted(true)` — audio continues in renderer processes during blocking wait.
**Root cause (startup):** All 15 React routes eagerly imported in `App.tsx`. MUI adds ~150KB. 18 `useEffect` hooks fire on mount. 5 overlay HWNDs pre-created before window is shown. No Vite code splitting.
**Fix approach:**
- Shutdown: Mute all browsers → close wallet-facing overlays first → parallelize server shutdown → close remaining browsers
- Startup: `React.lazy()` for overlay routes, `manualChunks` in Vite, defer non-critical hooks, move overlay creation to after window show via `CefPostDelayedTask`
**Key files:** `cef_browser_shell.cpp` (ShutdownApplication ~line 430, overlay pre-creation ~line 3037), `simple_handler.cpp` (OnBeforeClose ~line 1309), `App.tsx`, `MainBrowserView.tsx`, `vite.config.ts`
**macOS notes:** macOS is MISSING `SaveSession()` and `ClearBrowsingDataOnExit()` (both TODO). Same audio mute fix needed. Same React code splitting applies automatically.
**Sprint:** 2

### B-3: Uninstall leaves files in install directory / reinstall fails (P1)
**Reported:** 2026-03-24 (v0.1.0-beta.1)
**Description:** After uninstall, `AppData\Local\Programs\HodosBrowser` still exists with leftover runtime files (debug.log, debug_output.log, startup_log.txt, test_debug.log). This causes reinstall to fail — user had to manually delete the folder before installing again.
**Impact:** Blocks reinstall/upgrade flow. Beta testers will hit this.
**Fix area:** `installer/hodos-browser.iss`:
- Add `[UninstallDelete]` section to remove known runtime files (logs, CEF cache)
- Add `[InstallDelete]` to clean stale files on upgrade/reinstall
- Add optional "Delete browsing data?" checkbox in uninstaller (default unchecked, like Chrome)
- Reduce log verbosity for release builds (currently writes debug-level logs)

### B-4: Header scrollbar when dragging between monitors (P2 → P1)
**Reported:** 2026-03-31 | **Promoted:** 2026-04-01 (grouped with B-1 — same WndProc code)
**Description:** Dragging the browser window to a monitor with a different size/DPI causes a scrollbar to appear in the header (tab bar + toolbar). Maximizing on the new monitor fixes it.
**Impact:** Affects multi-monitor setups with different resolutions.
**Root cause:** `WM_DPICHANGED` is not handled in `cef_browser_shell.cpp` WndProc. When dragging between monitors, Windows sends `WM_DPICHANGED` with a suggested new rect, but the browser ignores it. The header HWND keeps its old dimensions until `WM_SIZE` fires (on maximize/manual resize). `GetHeaderHeightPx()` in `LayoutHelpers.h` uses `GetDpiForWindow()` which returns the correct DPI, but it's only called during `WM_SIZE`.
**Fix area:** Add `WM_DPICHANGED` handler in `cef_browser_shell.cpp` that calls `SetWindowPos` with the suggested rect from `lParam`, then triggers the same resize logic as `WM_SIZE`. ~15 lines.
**macOS notes:** macOS handles DPI changes automatically via `NSWindow` `backingScaleFactor`. No fix needed.
**Sprint:** 3 (before B-1)

### B-6: Second instance shows "profile locked" error instead of opening new window (P1)
**Reported:** 2026-04-01
**Description:** Launching a second browser instance shows "Profile is already in use" error dialog and exits. Users expect a new window to open (like Chrome/Firefox).
**Impact:** Confusing UX — users can't open new windows by double-clicking the app.
**Root cause:** `AcquireProfileLock()` in `cef_browser_shell.cpp:2727` uses OS-level exclusive file lock. This exists because CEF's cache directory can't be shared between processes (SQLite corruption). Multi-window within the same process already works (tab tear-off).
**Fix approach:** Single-instance forwarding via named pipe. First instance creates `\\.\pipe\hodos-browser-{profileId}` listener. Second instance connects, sends "new_window" command, exits. First instance calls `WindowManager::CreateFullWindow()`.
**Key files:** `cef_browser_shell.cpp`, `ProfileLock.cpp/.h`, `WindowManager.cpp`
**macOS notes:** Needs `applicationShouldHandleReopen:hasVisibleWindows:` and `application:openURLs:` delegate methods — currently not implemented.
**Sprint:** 4

### B-8: Right-click paste missing in address bar (P1)
**Reported:** 2026-04-01
**Description:** Right-clicking the address bar only shows "Inspect Element" — no Cut/Copy/Paste. Ctrl+V works.
**Impact:** Users who prefer mouse-based paste can't use it.
**Root cause:** `OnBeforeContextMenu()` in `simple_handler.cpp:6452` checks `isTab` — non-tab browsers (header) return early with only "Inspect Element". Editable field detection (`CM_TYPEFLAG_EDITABLE`) exists but only runs in the tab branch.
**Fix:** ~10 lines — check `CM_TYPEFLAG_EDITABLE` before the `!isTab` early return. Add Cut/Copy/Paste/Select All using existing `MENU_ID_CUSTOM_*` constants.
**Key file:** `simple_handler.cpp` (OnBeforeContextMenu ~line 6452)
**macOS notes:** Same CEF code — fix applies cross-platform. macOS also has native Cmd+V via Edit menu.
**Sprint:** 1

### B-9: Taskbar/installer icon needs dark background (P2)
**Reported:** 2026-04-01
**Description:** Gold octagon icon on transparent background looks washed out on Windows dark taskbar, Alt+Tab, and File Explorer.
**Impact:** Minor visual polish.
**Fix:** Create new `hodos.ico` with dark circle/rounded-square background behind gold octagon. Replace `cef-native/hodos.ico`. Asset-only — no code changes.
**macOS notes:** macOS dock has light shelf — transparent version looks fine. Optional .icns update.
**Sprint:** 1

### B-10: Auto-update notification toggle (P1)
**Reported:** 2026-04-01
**Description:** WinSparkle shows a dialog when an update is found during background checks, interrupting the user. Want option for silent updates (default).
**Impact:** Users are interrupted by update dialogs they didn't request.
**Fix:** New setting `browser.autoUpdateNotifications` (default `false`). When OFF, use `win_sparkle_check_update_without_ui()`. When ON, show native dialog. Add toggle in Settings > About.
**Key files:** `SettingsManager.h`, `AutoUpdater.cpp`, `simple_handler.cpp` (~line 2442), `AboutSettings.tsx`
**macOS notes:** Sparkle 2 has similar behavior — verify silent mode support separately.
**Sprint:** 1

---

## Fixed Issues

### B-8: Right-click paste in address bar — FIXED (2026-04-01)
**Fix:** Added editable field detection (`CM_TYPEFLAG_EDITABLE`) before the `!isTab` early return in `OnBeforeContextMenu()`. Cut/Copy/Paste/Select All now appear for all editable fields in non-tab browsers (address bar, overlay inputs). Paste uses native Win32 `OpenClipboard`/`GetClipboardData` to read clipboard text, then injects into the focused element via JS — bypasses both `document.execCommand('paste')` (blocked in non-tab browsers) and `navigator.clipboard.readText()` (triggers permission prompt). Copy/Cut use `navigator.clipboard.writeText()` (no permission needed for writes).
**Files changed:** `simple_handler.cpp` (OnBeforeContextMenu + OnContextMenuCommand)

### B-10: Auto-update notification toggle — FIXED (2026-04-01)
**Fix:** Added new `browser.autoUpdateNotifications` setting (default `false`). Two toggles in Settings > About: "Check for updates automatically" (existing, default ON) and "Update notifications" (new, default OFF — suppresses periodic WinSparkle dialogs). WinSparkle auto-check only runs when both are enabled. Manual "Check for updates" button always works regardless.
**Files changed:** `SettingsManager.h`, `SettingsManager.cpp`, `cef_browser_shell.cpp` (Initialize), `simple_handler.cpp` (settings_set), `AboutSettings.tsx`, `useSettings.ts`

### B-5: Cloudflare bot detection blocks WhatsOnChain — FIXED (2026-04-01)
**Root cause:** Cumulative bot signals from privacy features — not TLS/H2 fingerprint drift. Six signals triggered Cloudflare: (1) `navigator.plugins = []` (empty = headless indicator), (2) fingerprint farbling on Cloudflare challenge pages themselves, (3) scriptlet injection breaking Cloudflare's JS verification, (4) no `navigator.webdriver = false`, (5) no `window.chrome` object, (6) `window.hodosBrowser` exposed to all pages.
**Fix:** Added Cloudflare domains (`challenges.cloudflare.com`, `cf-turnstile.com`) to fingerprint bypass list and `hodos-unbreak.txt`. Fixed `navigator.plugins` to return realistic Chrome 136 plugin array (5 PDF plugins). Set `navigator.webdriver = false`. Injected `window.chrome` stub on external pages. Restricted `window.hodosBrowser` to internal pages only (external pages get only BRC-100 + cefMessage). Bumped adblock CONFIG_VERSION 6→7.
**Files changed:** `FingerprintProtection.h`, `FingerprintScript.h`, `hodos-unbreak.txt`, `simple_render_process_handler.cpp`, `engine.rs`

### B-9: Taskbar icon dark background — FIXED (2026-04-01)
**Fix:** Replaced `cef-native/hodos.ico` with gold-on-black version (multi-resolution: 16/32/48/256px). Copy saved to `frontend/public/hodos.ico` and branding folder. No code changes — icon embedded via `hodos.rc` at build time.
**Files changed:** `cef-native/hodos.ico` (asset replacement)

---

## Sprint Schedule

| Sprint | Issues | Risk | Focus |
|--------|--------|------|-------|
| 1 | B-8, B-10, B-9 | LOW | Quick wins |
| 2 | B-2 (expanded) | MEDIUM | Startup/shutdown performance |
| 3 | B-4, B-1 | HIGH | Window management (frameless) |
| 4 | B-6, B-3 | MED-HIGH | Instance management + installer |
| 5 | B-5 | ~~HIGH UNCERTAINTY~~ DONE | Cloudflare — fixed 2026-04-01 |

See full implementation plan: `/home/archboldmatt/.claude/plans/polished-sniffing-engelbart.md`

---

## Testing Notes

**Test sites:** youtube.com, x.com, github.com, google.com, amazon.com, whatsonchain.com
**Test checklist:**
- [ ] Install on clean Windows machine
- [ ] Browser launches, UI loads (header visible within 1.5s)
- [ ] Navigate to sites, video plays
- [ ] Wallet create/recover works
- [ ] Close browser cleanly (no orphan processes, no lingering audio)
- [ ] Right-click address bar → paste works
- [ ] Auto-update notifications toggle works
- [ ] Multi-monitor drag → no scrollbar
- [ ] Frameless window: snap, drag, min/max/close work
- [ ] Double-click exe while running → new window (no error)
- [ ] Uninstall removes program files
- [ ] Reinstall after uninstall succeeds without manual cleanup
- [ ] Reinstall preserves wallet data
- [ ] WhatsOnChain loads without Cloudflare block
