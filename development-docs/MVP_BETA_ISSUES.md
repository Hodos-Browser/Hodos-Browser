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
**Root cause:** Installer (`hodos-browser.iss`) has NO `[UninstallDelete]` section. Runtime-generated files are not tracked by Inno Setup's uninstall log.
**Implementation plan:**
- *Part 1 — Always delete (logs in install dir):*
  - `[UninstallDelete]`: `{app}\debug.log`, `{app}\debug_output.log`, `{app}\startup_log.txt`, `{app}\test_debug.log`, `{app}\*.log`
  - `[InstallDelete]`: Same log files (clean stale files on upgrade/reinstall)
- *Part 2 — Check if browser is running:*
  - Add `[Code]` section: `InitializeUninstall()` checks for `HodosBrowserShell.exe` via tasklist. Prompt user to close. Option to force-kill after confirmation.
- *Part 3 — Optional "Delete browsing data?" checkbox:*
  - Default: UNCHECKED (preserve data, like Chrome)
  - If checked: delete `{userappdata}\HodosBrowser\Default\` (cache, history, bookmarks, settings, cookies)
  - Delete per-profile dirs too: `{userappdata}\HodosBrowser\Profile_*`
  - Delete `{userappdata}\HodosBrowser\profiles.json`
  - **NEVER auto-delete wallet data** (`{userappdata}\HodosBrowser\wallet\wallet.db`). If "delete data" is checked, show EXTRA warning: "Your wallet may contain funds. This cannot be undone."
  - Clean WinSparkle registry: `HKCU\Software\Marston Enterprises\Hodos Browser\`
- *Part 4 — Reduce log verbosity:*
  - Set `log_severity = LOGSEVERITY_WARNING` in release builds (`cef_browser_shell.cpp:2754`)
  - Consider removing or conditionalizing `startup_log.txt` writes in `simple_app.cpp`
**Complete runtime file inventory:**
  - Install dir: `debug_output.log`, `debug.log`, `startup_log.txt`, `test_debug.log`
  - `%APPDATA%\HodosBrowser\profiles.json`
  - `%APPDATA%\HodosBrowser\Default\`: `settings.json`, `bookmarks.db`, `cookie_blocks.db`, `HodosHistory`, `adblock_settings.json`, `fingerprint_settings.json`, `session.json`, `profile.lock`, `cache/` (CEF), `Default/` (CEF cookies/localStorage)
  - `%APPDATA%\HodosBrowser\wallet\wallet.db` — **CRITICAL: private keys, NEVER auto-delete**
  - Same structure repeated for each `Profile_N/`
**Key risks:**
- **Accidentally deleting wallet** (CRITICAL): wallet path must NEVER appear in `[UninstallDelete]`. Separate explicit prompt with "funds may be lost" warning.
- **Uninstall while browser running**: Locked files prevent deletion, leaves partial state. Mitigated by running-process check in `[Code]`.
- **Multi-profile cleanup**: Must enumerate all `Profile_N` directories, not just Default.
**Key file:** `installer/hodos-browser.iss`
**macOS notes:** macOS uses drag-to-trash uninstall. `~/Library/Application Support/HodosBrowser/` persists (standard macOS behavior). No equivalent issue.
**Sprint:** 4

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
**Root cause:** `AcquireProfileLock()` in `cef_browser_shell.cpp:2778` uses OS-level exclusive file lock (`FILE_FLAG_DELETE_ON_CLOSE` on Windows, `flock()` on macOS). Profile lock prevents SQLite corruption from concurrent CEF cache access. Multi-window within same process already works via `WindowManager::CreateFullWindow()`.
**Fix approach:** Named pipe single-instance forwarding. Keep ProfileLock for data integrity (separate concern).
**Implementation plan:**
- *Step 1 — New `SingleInstance.h/.cpp`:*
  - `TryAcquireInstance(profileId)` → tries `CreateNamedPipe("\\.\pipe\hodos-browser-{profileId}", FILE_FLAG_FIRST_PIPE_INSTANCE)`. Returns true if first instance (becomes server), false if another instance owns pipe.
  - `SendToRunningInstance(profileId, json)` → connects as client, sends `{"action":"new_window","url":"..."}`, waits 5s for ACK, exits.
  - `StartListenerThread()` → background thread: `ConnectNamedPipe()` → `ReadFile()` → parse JSON → `PostMessage(g_hwnd, WM_APP+1, ...)` → loop.
- *Step 2 — Integrate in `cef_browser_shell.cpp`:*
  - AFTER `CefExecuteProcess()` (line 2732) — CEF subprocesses return here, only browser process continues.
  - BEFORE `AcquireProfileLock()` (line 2778) — try pipe first.
  - Flow: `TryAcquireInstance()` → if false → `SendToRunningInstance()` → exit cleanly (no error dialog). If true → continue to `AcquireProfileLock()` → start pipe listener thread.
- *Step 3 — Handle `WM_APP+1` in `ShellWindowProc`:*
  - Call `WindowManager::CreateFullWindow()` (same as tab tear-off path).
  - If URL provided, `TabManager::CreateTab(url, ...)` in the new window.
  - `SetForegroundWindow()` + `FlashWindow()` fallback to bring window to front.
- *Step 4 — Keep ProfileLock unchanged* — pipe handles instance forwarding, lock handles data integrity. Two separate concerns.
**Key risks:**
- **CEF subprocess confusion**: CEF spawns renderer/GPU/utility subprocesses that call `WinMain()`. Pipe check MUST go after `CefExecuteProcess()` which returns early for subprocesses (already at line 2732). This is the #1 gotcha.
- **Pipe hijacking**: Mitigated by `FILE_FLAG_FIRST_PIPE_INSTANCE` (atomic, only first creator succeeds).
- **Race condition (two instances simultaneously)**: `FILE_FLAG_FIRST_PIPE_INSTANCE` is atomic — only one wins. Loser becomes client.
- **Stale pipe after crash**: Windows auto-cleans named pipes on process exit. No stale pipe risk.
- **`SetForegroundWindow` restrictions**: Windows limits which processes can steal focus. Client calls `AllowSetForegroundWindow(serverPid)` before sending pipe message. Fallback: `FlashWindow()` to flash taskbar.
**Sprint 3 dependency:** Sprint 3 (B-1 frameless) changes window style in `CreateFullWindow()`. B-6's new windows inherit the frameless style automatically — no special handling needed. Sprint 3 must be done BEFORE Sprint 4.
**Key files:** `cef_browser_shell.cpp` (startup flow, WndProc), NEW `SingleInstance.h/.cpp`, `ProfileLock.cpp/.h` (unchanged), `WindowManager.cpp` (unchanged — `CreateFullWindow()` already works)
**macOS notes:** Named pipes don't exist on macOS. Use `applicationShouldHandleReopen:hasVisibleWindows:` NSApplicationDelegate method (fires when user clicks dock icon while app is running). Also implement `application:openURLs:` for URL scheme forwarding. Neither is currently implemented in `cef_browser_shell_mac.mm`. Simpler than Windows — just call macOS equivalent of `CreateFullWindow()` from delegate.
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
**Fix (two commits):**
- *Commit 1 — Bot signal fixes:*
  - Added `challenges.cloudflare.com` + `cf-turnstile.com` to fingerprint bypass list (`FingerprintProtection.h`)
  - Added Cloudflare exceptions to `hodos-unbreak.txt` (scriptlet `#@#+js()` + cosmetic `$generichide` + script network exceptions for `static.cloudflareinsights.com`, `cdnjs.cloudflare.com`)
  - Fixed `navigator.plugins` from empty `[]` to realistic Chrome 136 plugin array (5 PDF plugins with `PluginArray.prototype`, indexed + named access)
  - Set `navigator.webdriver = false` explicitly
  - Injected `window.chrome` stub on external pages (runtime, loadTimes, csi)
  - Restricted `window.hodosBrowser` to internal pages only (external pages get only BRC-100 + cefMessage)
  - Bumped adblock CONFIG_VERSION 6→7
- *Commit 2 — Brave-style subtle farbling refactor:*
  - Removed `navigator.hardwareConcurrency` override (was random 2-8 — cross-referenced by anti-fraud against real CPU performance)
  - Removed `navigator.deviceMemory` override (was hardcoded 8GB — inconsistent with real RAM)
  - Removed WebGL `getParameter` vendor/renderer spoofing (was hardcoded NVIDIA string — mismatched real GPU extensions)
  - Reduced canvas/WebGL pixel farble rate from 10% to 3% (subtler, less detectable)
  - Shrunk fingerprint bypass list from 37 to 9 domains — only CAPTCHA/challenge services (Cloudflare, reCAPTCHA, hCaptcha). Auth, banking, e-commerce no longer need bypass with subtle farbling.
  - Design principle: Brave-style "imperceptible perturbation" — small LSB noise on high-entropy APIs (canvas, WebGL pixels, audio), no hardware value spoofing that creates detectable inconsistencies.
**Verified:** Cloudflare Turnstile security check passes. WhatsOnChain loads without challenge loop.
**Note:** Cloudflare tester reports CEF 136 as "unsupported" (requires Chrome 144+) but the security check still passes. CEF upgrade tracked separately.
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
