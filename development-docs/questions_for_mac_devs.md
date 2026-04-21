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

## 5. Single-Instance Forwarding — macOS Delegate Methods (from B-6 Sprint 4)

**Windows fix:** Named pipe (`\\.\pipe\hodos-browser-{profileId}`) for single-instance forwarding. Second instance connects to pipe, sends "new_window" command, first instance calls `CreateFullWindow()`.

**macOS current state:** No single-instance handling at all. `cef_browser_shell_mac.mm` does NOT implement:
- `applicationShouldHandleReopen:hasVisibleWindows:` — fires when user clicks dock icon while app is already running
- `application:openURLs:` — fires when a URL scheme (`hodos://`) or file association opens while app is running

Both are standard `NSApplicationDelegate` methods. Without them, macOS behaves the same as Windows — second instance hits profile lock and shows error alert.

**What to implement:**
- Add `applicationShouldHandleReopen:hasVisibleWindows:` to the app delegate in `cef_browser_shell_mac.mm`
  - If `hasVisibleWindows == YES`: bring frontmost window to front (`[window makeKeyAndOrderFront:nil]`)
  - If `hasVisibleWindows == NO`: call macOS equivalent of `CreateFullWindow()` to create a new window
  - Return `NO` (we handled it ourselves)
- Add `application:openURLs:` to handle URL forwarding
  - Parse URL, create new tab in frontmost window via `TabManager::CreateTab(url, ...)`
- Consider `application:openFile:` for file associations (`.html`, `.pdf`)

**This is simpler than the Windows named pipe approach** — macOS provides the delegate callbacks natively. No IPC plumbing needed.

**Risk notes:**
- These delegate methods fire on the main thread — safe to call `WindowManager`/`TabManager` directly (unlike Windows where pipe listener runs on background thread and must `PostMessage`)
- Verify the app delegate class in `cef_browser_shell_mac.mm` — it may be a `CefAppDelegate` or custom class. The delegate methods must be added to whatever class is set as `NSApp.delegate`.
- `applicationShouldHandleReopen` does NOT fire on first launch — only on reactivation. No conflict with normal startup.

---

## 6. Fingerprint Protection — Verify Subtle Farbling on macOS (from B-5 Sprint 5)

**Windows fix (reference):** Refactored fingerprint protection to Brave-style subtle farbling:
- Removed `navigator.hardwareConcurrency`, `navigator.deviceMemory`, and WebGL vendor/renderer spoofing
- Reduced canvas/WebGL farble rate from 10% to 3%
- Added `navigator.plugins` (Chrome 136 realistic list), `navigator.webdriver = false`
- Injected `window.chrome` stub on external pages
- Restricted `window.hodosBrowser` to internal pages (external get BRC-100 + cefMessage only)

**macOS:** All JS changes are in cross-platform files (`FingerprintScript.h`, `FingerprintProtection.h`, `simple_render_process_handler.cpp`) — they apply automatically. However:
- **Verify:** `FingerprintProtection::GenerateSessionToken()` uses `SecRandomCopyBytes` on macOS (line ~55 in `FingerprintProtection.h`). Confirm this path works and produces good entropy.
- **Verify:** `window.chrome` stub injection — macOS CEF may already provide a `window.chrome` object (Chromium-based). If so, the `typeof window.chrome === 'undefined'` guard prevents double-injection. Just verify it doesn't conflict.
- **Test:** Visit `whatsOnChain.com` in macOS build — Cloudflare challenge should pass.

---

## Reference Files

| File | Purpose |
|------|---------|
| `cef_browser_shell_mac.mm` | macOS entry point, window lifecycle, overlay creation, `ShutdownBrowsers()` |
| `AutoUpdater_mac.mm` | Sparkle 2 integration (if exists) |
| `cef_browser_shell.cpp` | Windows reference implementation for all the above |
| `simple_handler.cpp` | Cross-platform IPC handlers (settings_set, context menu) |
| `development-docs/Final-MVP-Sprint/macos-port/` | Existing macOS port tracking docs |
