# Questions / Tasks for macOS Developers

Items identified during MVP beta fix sprints (2026-04-01) that need macOS-specific attention. These were intentionally skipped on Windows-only sprints because they touch `cef_browser_shell_mac.mm` (~3900 lines) and require a Mac to test.

---

## 1. Shutdown: Audio Mute + Parallel Server Stop (from B-2 Sprint 2)

**Windows fix (reference):** `ShutdownApplication()` in `cef_browser_shell.cpp` ~line 430 was reordered to:
1. Mute all browsers via `SetAudioMuted(true)` — stops audio immediately
2. Close wallet-facing overlays first (they talk to wallet server)
3. Parallelize `StopWalletServer()` + `StopAdblockServer()` in two threads, join
4. Close remaining browsers

**macOS current state:** `ShutdownBrowsers()` in `cef_browser_shell_mac.mm` ~line 3360:
- Does NOT mute audio before closing browsers
- Does NOT parallelize server shutdown (uses `SIGTERM`, not HTTP POST like Windows)
- Servers shut down sequentially — same 8-12s delay as Windows had

**What to port:**
- Add `browser->GetHost()->SetAudioMuted(true)` loop for all browsers before any `CloseBrowser` calls
- Parallelize wallet + adblock server shutdown (dispatch to background queue or `std::thread`)
- Test: play YouTube, close browser — audio should stop immediately, process should exit within 3s

**Risk notes:**
- macOS overlay lifecycle uses NSPanel + `resignKey`/`resignMain` — closing overlays during shutdown may trigger delegate callbacks. Verify no re-entrant issues.
- `SIGTERM` handling may differ from HTTP POST shutdown — verify both servers exit cleanly with parallel SIGTERM.

---

## 2. Shutdown: Missing SaveSession() and ClearBrowsingDataOnExit() (from B-2)

**Windows:** Both called in `ShutdownApplication()` before any `CloseBrowser` calls:
- `SaveSession()` — saves open windows/tabs to `session.json` for restore-on-start
- `ClearBrowsingDataOnExit()` — clears history/cookies/cache if `privacy.clearDataOnExit` setting is on

**macOS:** Both are marked `// TODO` in `ShutdownBrowsers()`. Neither is implemented.

**What to port:**
- Port `SaveSession()` logic — needs access to `TabManager` and `WindowManager` state before browsers close
- Port `ClearBrowsingDataOnExit()` — needs the header browser to still be alive (uses CEF cookie/cache APIs)
- Both must run BEFORE any `CloseBrowser` calls
- Reference the Windows implementation in `cef_browser_shell.cpp` ShutdownApplication()

---

## 3. Auto-Update Notifications Toggle (from B-10 Sprint 1)

**Windows:** New `browser.autoUpdateNotifications` setting (default `false`). WinSparkle auto-check only runs when both `autoUpdateEnabled` AND `autoUpdateNotifications` are true. Implemented in `cef_browser_shell.cpp` (Initialize) and `simple_handler.cpp` (settings_set handler).

**macOS:** Uses Sparkle 2 framework (`AutoUpdater_mac.mm`). Equivalent behavior needed:
- When notifications OFF: `SPUStandardUpdaterController.automaticallyChecksForUpdates = NO` (or equivalent Sparkle 2 API)
- When notifications ON: enable Sparkle 2 periodic checks
- Verify Sparkle 2 has a true "silent check" mode (WinSparkle's `_without_ui` still shows dialog on found update)

**Files:** `cef-native/src/core/AutoUpdater_mac.mm` (if it exists, otherwise `AutoUpdater.cpp` has `#elif __APPLE__` stubs)

---

## 4. Right-Click Paste (from B-8 Sprint 1) — Verify macOS Behavior

**Windows fix:** Paste in non-tab browsers (address bar) uses Win32 `OpenClipboard`/`GetClipboardData` to read clipboard natively, then injects text via JS. This was needed because `document.execCommand('paste')` is blocked and `navigator.clipboard.readText()` triggers a permission prompt.

**macOS:** The `OnContextMenuCommand` handler has a `#elif defined(__APPLE__)` path using `popen("pbpaste")` for clipboard reading. This should work but needs verification:
- Right-click address bar in macOS build — does Cut/Copy/Paste appear?
- Does Paste actually work? (pbpaste should bypass browser permission restrictions)
- Note: macOS also has native Cmd+V via NSResponder chain, so this is less critical

---

## Reference Files

| File | Purpose |
|------|---------|
| `cef_browser_shell_mac.mm` | macOS entry point, window lifecycle, overlay creation, `ShutdownBrowsers()` |
| `AutoUpdater_mac.mm` | Sparkle 2 integration (if exists) |
| `cef_browser_shell.cpp` | Windows reference implementation for all the above |
| `simple_handler.cpp` | Cross-platform IPC handlers (settings_set, context menu) |
| `development-docs/Final-MVP-Sprint/macos-port/` | Existing macOS port tracking docs |
