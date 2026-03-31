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

### B-2: Audio continues playing after browser closes (P1)
**Reported:** 2026-03-24 (v0.1.0-beta.1)
**Description:** When closing the browser while a YouTube video is playing, audio continues for several seconds after the window disappears.
**Impact:** Unprofessional UX — user thinks the app didn't close properly.
**Fix area:** `cef_browser_shell.cpp` shutdown sequence — need to force-close all CEF browser instances (including renderer processes) before the main window closes. May need `browser->GetHost()->CloseBrowser(true)` for all active tabs + overlays, or `TerminateProcess` for child processes.

### B-3: Uninstall leaves files in install directory / reinstall fails (P1)
**Reported:** 2026-03-24 (v0.1.0-beta.1)
**Description:** After uninstall, `AppData\Local\Programs\HodosBrowser` still exists with leftover runtime files (debug.log, debug_output.log, startup_log.txt, test_debug.log). This causes reinstall to fail — user had to manually delete the folder before installing again.
**Impact:** Blocks reinstall/upgrade flow. Beta testers will hit this.
**Fix area:** `installer/hodos-browser.iss`:
- Add `[UninstallDelete]` section to remove known runtime files (logs, CEF cache)
- Add `[InstallDelete]` to clean stale files on upgrade/reinstall
- Add optional "Delete browsing data?" checkbox in uninstaller (default unchecked, like Chrome)
- Reduce log verbosity for release builds (currently writes debug-level logs)

---

## Fixed Issues

(None yet)

---

## Testing Notes

**Test sites:** youtube.com, x.com, github.com, google.com, amazon.com
**Test checklist:**
- [ ] Install on clean Windows machine
- [ ] Browser launches, UI loads
- [ ] Navigate to sites, video plays
- [ ] Wallet create/recover works
- [ ] Close browser cleanly (no orphan processes)
- [ ] Uninstall removes program files
- [ ] Reinstall preserves wallet data
