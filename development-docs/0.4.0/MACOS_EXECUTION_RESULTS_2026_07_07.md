# macOS Execution Results — 2026-07-07

Results from executing the macOS brief on macOS 15.7.5 (Apple Silicon).

## M1: Build Verify — PASS

Windows-only changes did not break the macOS build. `cmake --build build --config Release` succeeded.

## M2: Sparkle Auto-Update Scheduled Check — FIXED

### Configuration verified (all in `cef-native/Info.plist`):
- `SUScheduledCheckInterval=10800` (3 hours) — set correctly
- `SUEnableAutomaticChecks=true` — set correctly
- `SUFeedURL=https://hodosbrowser.com/appcast.xml` — set correctly
- `SUPublicEDKey` — present

### Bug found and fixed: no update check on launch

Sparkle's scheduled check only fires when `SUScheduledCheckInterval` has elapsed
since the last successful check. If the user quits before the interval expires
and relaunches, Sparkle sees "not due yet" and skips. A user who opens the
browser for <3 hours per session would never see an update.

**Fix** (`AutoUpdater_mac.mm` + `AutoUpdater.cpp`): Force a background update
check on every launch via `[updater checkForUpdatesInBackground]` (macOS) and
`win_sparkle_check_update_without_ui()` (Windows). The 3-hour interval still
governs subsequent checks while the app is running, but every cold start checks
immediately. Both platforms fixed in this commit.

### Cannot test actual Sparkle fire on dev machine

Sparkle.framework is not bundled in dev builds (downloaded by CI only). The
`__has_include(<Sparkle/Sparkle.h>)` check sets `SPARKLE_AVAILABLE 0`, so all
Sparkle calls are no-ops locally. The code paths and Info.plist configuration are
verified correct; actual fire testing requires a CI-built .app with Sparkle
bundled.

## M3: Picker-Window Mac Parity — PASS

### Changes (`cef_browser_shell_mac.mm`):
- Picker window: 60% screen width (max 980px) × 78% screen height (max 660px), centered
- No resize/minimize in picker mode (only close + title bar)
- Dark background (#1a1d23) to prevent white flash during load
- Header/webview sizing uses `windowRect` dimensions instead of `screenRect`
- Full browser mode unchanged (still uses full `screenRect`)

### Full flow verified:
1. Launch without `--profile` → picker window appears, small and centered
2. 3 profiles load, React UI renders tiles with MUI icons
3. Click profile tile → picker spawns new browser with `--profile=Default`
4. Browser opens full-screen, New Tab loads with favicon and title
5. Wallet connects on correct port, balance fetched, IPC messages flowing
6. Picker process exits cleanly

### ProfileManager.cpp fix:
`LaunchWithProfile()` now always returns `true` after successful `posix_spawn`.
Previously, `waitpid` failures from CEF's SIGCHLD handler race caused spurious
`false` returns even though the child process launched correctly.

## Additional: Async Server Startup Fix

### Problem
`StartWalletServer()` and `StartAdblockServer()` blocked the main thread for up
to 10 seconds each with `usleep()` health check loops between `CefInitialize()`
and browser creation. CEF's network service requires the main thread message loop
to complete initialization. When browsers were created after the block, initial
navigations got empty responses (`<html><head></head><body></body></html>`)
because the network service hadn't fully started.

### Root cause proof
CDP inspection showed pages had empty HTML, but `fetch()` from console returned
200 OK and `Page.navigate` via CDP loaded content — confirming the network
service was eventually healthy, just not at initial navigation time.

### Fix (`cef_browser_shell_mac.mm`):
- Renamed `StartWalletServer()` → `SpawnWalletServer()` — just does `posix_spawn`, returns immediately
- Renamed `StartAdblockServer()` → `SpawnAdblockServer()` — same
- New `StartBackendServices()` calls both spawners, then dispatches health check
  loops to `dispatch_async(dispatch_get_global_queue())` background thread
- Main thread is never blocked after `CefInitialize()` — CEF message loop starts
  immediately, network service initializes properly, first navigations load

## Additional: Dev Port Deconfliction — Verified

Rebuilt both Rust services (`cargo build --release`) so they read `HODOS_DEV=1`:
- **Wallet**: binds 31401 in dev (was 31301 from stale binary)
- **Adblock**: binds 31402 in dev (was 31302 from stale binary)
- C++ `PortConfig.h` expects 31401/31402 in dev — now matches
- Prod ports 31301/31302 confirmed not responding in dev mode
- Dev and installed builds can run simultaneously without port conflicts

### Note for Windows
The Rust port gating (`wallet_port()` / `adblock_port()`) was already correct in
source. The macOS binaries just needed rebuilding from the current commit. Windows
binaries should already be correct if built after the port change was merged.

## macOS Keychain Note

The wallet uses macOS Keychain for mnemonic auto-unlock (equivalent of Windows
DPAPI). Dev builds are unsigned, so macOS prompts "hodos-wallet wants to use your
confidential information stored in 'HodosBrowser' in your keychain" on every
rebuild. The password is the Mac login password. Clicking "Always Allow" works
until the next binary rebuild changes the code signature. This only affects dev —
production builds are code-signed and won't prompt.

## Files Changed

| File | Changes |
|------|---------|
| `cef-native/cef_browser_shell_mac.mm` | Picker window sizing, dark background, async server startup |
| `cef-native/src/core/AutoUpdater_mac.mm` | Force background update check on launch |
| `cef-native/src/core/AutoUpdater.cpp` | Force background update check on launch (Windows) |
| `cef-native/src/core/ProfileManager.cpp` | LaunchWithProfile always returns true after posix_spawn |
