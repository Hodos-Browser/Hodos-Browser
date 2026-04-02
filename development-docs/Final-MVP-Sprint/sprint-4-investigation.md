# Sprint 4 Investigation: B-6 (Single-Instance) + B-3 (Uninstall Cleanup)

**Date:** 2026-04-01
**Status:** Research complete, ready for implementation planning

---

## B-6: Single-Instance Forwarding

### Current Behavior

When a user double-clicks `HodosBrowserShell.exe` while the browser is already running, they see:

> **"Hodos Browser - Profile Locked"**
> Profile "Default" is already in use by another instance.
> Close the other instance first, or launch with a different profile.

Users expect a new window to open (like Chrome/Firefox).

### Current Implementation

**Profile lock mechanism** (`ProfileLock.cpp`):
- Windows: `CreateFileA()` with `FILE_FLAG_DELETE_ON_CLOSE` on `<profile_path>/profile.lock`
- macOS: `flock()` with `LOCK_EX | LOCK_NB` on same file
- Lock acquired at startup (`cef_browser_shell.cpp:2778`), released at shutdown (`cef_browser_shell.cpp:3168`)
- On failure: `MessageBoxA()` error dialog, `return 1`

**Crash recovery is solid:**
- Windows: `FILE_FLAG_DELETE_ON_CLOSE` auto-deletes lock on crash
- macOS: Kernel auto-releases `flock()` on process exit
- No stale lock problem exists

**Multi-window already works** within a single process:
- `WindowManager::CreateFullWindow()` creates new shell HWND + header + overlays
- Each window gets auto-incrementing ID with 30px offset
- Tab tear-off creates new windows via same mechanism
- 11 overlay HWNDs per window (wallet, settings, omnibox, etc.)

**Multi-profile launches work** via `ProfileManager::LaunchWithProfile()`:
- Spawns entirely new process with `--profile="Profile_N"` argument
- Each profile has its own lock file, databases, settings
- Completely independent processes

### How Other Browsers Handle This

| Browser | Mechanism | URL Forwarding |
|---------|-----------|----------------|
| **Chrome** | Named mutex + `WM_COPYDATA` message to existing window | Yes — full command-line args |
| **Firefox** | Profile lock file + `-remote` IPC system | Yes — URLs forwarded |
| **Electron** | `app.requestSingleInstanceLock()` + `second-instance` event | Yes — argv + custom data |
| **CEF apps** | Typically mutex or named pipe (must check before CEF subprocess spawns) | Varies |

### Recommended Approach: Named Pipe

**Why named pipe over mutex + WM_COPYDATA:**
- Named pipes support bidirectional communication (can send response back)
- More robust than FindWindow + SendMessage (window might not exist yet)
- Supports sending arbitrary data (URLs, command-line args, JSON)
- `FILE_FLAG_FIRST_PIPE_INSTANCE` provides atomic "am I first?" check

**Flow:**

```
Second instance starts
├── Try CreateNamedPipe("\\.\pipe\hodos-browser-{profileId}", FILE_FLAG_FIRST_PIPE_INSTANCE)
│   ├── SUCCESS → I'm the first instance, continue normal startup
│   └── ERROR_ACCESS_DENIED or ERROR_PIPE_BUSY → Another instance owns the pipe
│       ├── Connect to pipe as client
│       ├── Send command: {"action": "new_window", "url": "...", "args": [...]}
│       ├── Wait for ACK (with 5-second timeout)
│       └── Exit cleanly (no error dialog)
│
First instance (pipe server thread)
├── Background thread: ConnectNamedPipe() → ReadFile() → parse JSON
├── Post to UI thread: WindowManager::CreateFullWindow()
├── If URL provided: TabManager::CreateTab(url, ...)
├── SetForegroundWindow() to bring existing window forward
└── Write ACK back to pipe → DisconnectNamedPipe() → loop
```

### Implementation Plan

**Files to modify:**

| File | Changes |
|------|---------|
| `cef_browser_shell.cpp` | Replace `AcquireProfileLock()` check with pipe-based single-instance check. Start pipe server thread on success. |
| NEW: `SingleInstance.h/.cpp` | Named pipe server/client. `TryAcquire()` returns bool. `SendToRunning()` forwards args. Background listener thread. |
| `ProfileLock.cpp/.h` | Keep as-is — still needed for SQLite corruption prevention. Pipe handles instance forwarding, lock handles data integrity. |
| `WindowManager.cpp` | No changes — `CreateFullWindow()` already works |
| `simple_app.cpp` | Minor: handle URL arg in `OnContextInitialized()` for pipe-forwarded requests |

**Key design decisions:**

1. **Keep ProfileLock separate from SingleInstance** — They serve different purposes. ProfileLock prevents SQLite corruption. SingleInstance provides UX (forward to running instance instead of error).

2. **Pipe name includes profile ID** — `\\.\pipe\hodos-browser-Default` so different profiles are independent. Profile_1 can still launch as a separate process.

3. **Pipe server runs on dedicated thread** — NOT on CEF UI thread. Posts `CefPostTask(TID_UI, ...)` to create window on UI thread.

4. **Timeout on client side** — 5-second timeout on `WaitNamedPipe()`. If server doesn't respond (crashed but pipe lingers), fall back to normal startup.

### Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| **Pipe hijacking** — attacker creates pipe first | MEDIUM | Use `FILE_FLAG_FIRST_PIPE_INSTANCE` (atomic). Validate pipe creator via `GetNamedPipeServerProcessId()`. |
| **Pipe server thread crash** — blocks new windows | LOW | Client has 5-second timeout, falls back to error dialog (current behavior). Server thread has try/catch. |
| **Race condition** — two instances launch simultaneously | LOW | `FILE_FLAG_FIRST_PIPE_INSTANCE` is atomic. Only one succeeds. Loser becomes client. |
| **CEF subprocess confusion** — CEF spawns renderer subprocesses that also hit the pipe check | HIGH | Must check BEFORE `CefExecuteProcess()`. CEF subprocesses have `--type=` arg — skip pipe check if `--type` is present. |
| **Stale pipe after crash** — OS cleans up pipes on process exit, but edge cases exist | LOW | Client timeout + fallback. Named pipes are cleaned by kernel on process exit. |
| **SetForegroundWindow fails** — Windows restricts which processes can steal focus | MEDIUM | Use `AllowSetForegroundWindow()` from client before sending pipe message. Or use `FlashWindow()` as fallback. |
| **macOS equivalent needed** — No named pipes on macOS | MEDIUM | Use Unix domain socket or `NSDistributedNotificationCenter`. Or `applicationShouldHandleReopen:` delegate. Separate implementation needed. |

### Critical Risk: CEF Subprocess Detection

This is the #1 gotcha. CEF spawns multiple subprocesses (renderer, GPU, utility) that call `WinMain()`. These subprocesses must NOT try to connect to the pipe or create windows.

**Solution:** Check for `--type=` in command line before pipe logic:
```cpp
// In WinMain(), before anything else:
int exit_code = CefExecuteProcess(main_args, nullptr, nullptr);
if (exit_code >= 0) return exit_code;  // This was a subprocess, exit

// Only the browser process reaches here
// Now do single-instance check
```

This is already the pattern in `cef_browser_shell.cpp:2732` — `CefExecuteProcess()` returns >= 0 for subprocesses. The pipe check goes AFTER this line.

### Critical Dependency: Primary Window Transfer (done 2026-04-02)

The `WINDOW_CLOSE_PRIMARY_TRANSFER.md` implementation changes the multi-window architecture in ways that B-6 must account for:

**What changed:**
- `GetWindow(0)` replaced with `GetPrimaryWindow()` across 34 call sites
- `g_hwnd` now points to whichever window is primary (not always window 0)
- `TransferPrimaryWindow()` migrates overlays and global HWNDs when primary window closes
- `primary_window_id_` is explicitly tracked in WindowManager

**Impact on B-6:**

1. **PostMessage target:** The pipe listener thread posts `PostMessage(g_hwnd, WM_APP+1, ...)`. Since `g_hwnd` now tracks the current primary (updated by `TransferPrimaryWindow()`), this is safe. But the WM_APP+1 handler should use `GetPrimaryWindow()` explicitly for any window lookups, not assume window 0.

2. **SetForegroundWindow:** Must use `GetPrimaryWindow()->hwnd`, not `GetWindow(0)->hwnd`. After primary transfer, window 0 may not exist.

3. **New windows are secondary:** `CreateFullWindow()` creates secondary windows. Overlays stay with the primary. This is correct — matches tab tear-off behavior. If the user later closes the primary, `TransferPrimaryWindow()` handles overlay migration automatically.

4. **No new risk for B-6:** The primary transfer system is transparent to B-6. B-6 just calls `CreateFullWindow()` which creates a new window with the correct (frameless) style and a new tab. The primary/secondary distinction is handled by the existing WindowManager infrastructure.

**Updated flow diagram:**
```
Pipe listener thread receives "new_window" command
├── PostMessage(g_hwnd, WM_APP+1, url_data)  // g_hwnd = current primary
│
ShellWindowProc receives WM_APP+1
├── BrowserWindow* primary = WindowManager::GetInstance().GetPrimaryWindow()
├── BrowserWindow* newWin = WindowManager::GetInstance().CreateFullWindow(true)
├── If url: TabManager::CreateTab(url, newWin->hwnd, ...)
├── SetForegroundWindow(primary->hwnd)  // bring to front
└── FlashWindow(primary->hwnd, TRUE)    // fallback if focus steal blocked
```

---

## B-3: Uninstall Cleanup / Reinstall Failure

### Current Behavior

After uninstall, `AppData\Local\Programs\HodosBrowser` still contains:
- `debug.log`, `debug_output.log`, `startup_log.txt`, `test_debug.log`
- CEF cache directories
- Profile databases and settings

This causes reinstall to fail — user must manually delete the folder.

### Current Installer

**File:** `installer/hodos-browser.iss` (Inno Setup)

**What's missing:**
- No `[UninstallDelete]` section at all
- No `[InstallDelete]` section for upgrade cleanup
- No "delete browsing data?" prompt
- No check for running browser before uninstall

### Complete Inventory of Runtime Files

**Install directory** (`{app}` = `%LOCALAPPDATA%\Programs\HodosBrowser`):

| File | Created by | Safe to delete on uninstall? |
|------|-----------|------------------------------|
| `debug_output.log` | Logger class | Yes — always |
| `debug.log` | CEF internal | Yes — always |
| `startup_log.txt` | simple_app.cpp | Yes — always |
| `test_debug.log` | Unknown/legacy | Yes — always |

**User data directory** (`%APPDATA%\HodosBrowser\`):

| Path | Purpose | Delete on uninstall? |
|------|---------|---------------------|
| `profiles.json` | Profile list | Only if user opts in |
| `Default/settings.json` | User preferences | Only if user opts in |
| `Default/bookmarks.db` | Bookmarks | Only if user opts in |
| `Default/cookie_blocks.db` | Cookie rules | Only if user opts in |
| `Default/HodosHistory` | Browsing history | Only if user opts in |
| `Default/adblock_settings.json` | Per-site adblock | Only if user opts in |
| `Default/fingerprint_settings.json` | Per-site fingerprint | Only if user opts in |
| `Default/session.json` | Tab restore state | Only if user opts in |
| `Default/profile.lock` | Instance lock | Yes — always (auto-deleted) |
| `Default/cache/` | CEF browser cache | Only if user opts in |
| `Default/Default/` | CEF cookies, localStorage | Only if user opts in |
| `Profile_N/...` | Same structure per profile | Only if user opts in |

**Registry** (WinSparkle):
- `HKCU\Software\Marston Enterprises\Hodos Browser\` — auto-update state

**Wallet data** (`%APPDATA%\HodosBrowser\wallet/`):
- `wallet.db` — **CRITICAL: Contains private keys. NEVER auto-delete.**

### How Chrome Handles Uninstall

Chrome's uninstaller:
1. **Default:** Only removes program files. Leaves ALL user data in AppData intact.
2. **Optional checkbox:** "Also delete your browsing data?" (default unchecked)
3. If checked: Deletes `%LOCALAPPDATA%\Google\Chrome\User Data\` entirely
4. **Never deletes without asking** — user data preservation is paramount

### Implementation Plan

**Changes to `installer/hodos-browser.iss`:**

```ini
[UninstallDelete]
; --- Always delete: runtime logs in install directory ---
Type: files; Name: "{app}\debug.log"
Type: files; Name: "{app}\debug_output.log"
Type: files; Name: "{app}\startup_log.txt"
Type: files; Name: "{app}\test_debug.log"
Type: files; Name: "{app}\*.log"

[InstallDelete]
; --- Clean stale files on upgrade/reinstall ---
Type: files; Name: "{app}\debug.log"
Type: files; Name: "{app}\debug_output.log"
Type: files; Name: "{app}\startup_log.txt"

[Code]
// Optional "Delete browsing data?" dialog during uninstall
// Default: unchecked (preserve user data like Chrome)
// If checked: delete %APPDATA%\HodosBrowser\ (EXCLUDING wallet/)
// NEVER delete wallet data without explicit separate confirmation
```

**Key design decisions:**

1. **Always delete logs** — No user value, blocks reinstall
2. **Never auto-delete wallet data** — Contains private keys and real money
3. **Optional browsing data deletion** — Checkbox, default unchecked (Chrome pattern)
4. **Separate wallet deletion warning** — If user checks "delete data", show EXTRA warning about wallet: "Your wallet contains funds. Are you SURE?"
5. **Check if browser is running** — Prompt to close before uninstall

### Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| **Accidentally deleting wallet** | CRITICAL | Never include wallet path in `[UninstallDelete]`. Separate explicit prompt with balance check. |
| **Deleting user data without consent** | HIGH | Default unchecked checkbox. Two-step confirmation for browsing data. |
| **Uninstall while browser running** — locked files | MEDIUM | Add `[Code]` to check `tasklist` for HodosBrowserShell.exe, prompt to close. |
| **Partial uninstall on crash** | LOW | Inno Setup has built-in rollback. `[InstallDelete]` cleans stale files on next install attempt. |
| **Registry not cleaned** | LOW | Add `[UninstallDelete]` for WinSparkle registry key. Low impact if left. |
| **Multi-profile cleanup** | MEDIUM | If "delete data" checked, must enumerate all Profile_N directories, not just Default. Use wildcard: `{userappdata}\HodosBrowser\Profile_*` |

### Log Verbosity for Release Builds

Currently the browser writes debug-level logs in release mode. Should reduce:
- `Logger` level: Set minimum to INFO for release builds (skip DEBUG)
- `startup_log.txt`: Remove or make conditional on debug flag
- CEF `debug.log`: Set `log_severity = LOGSEVERITY_WARNING` in release

This reduces log file size and sensitivity of data written to disk.

---

## Implementation Order Recommendation

### B-3 first (lower risk, unblocks testing)

1. Add `[UninstallDelete]` + `[InstallDelete]` sections to `.iss` file
2. Add "is browser running?" check in `[Code]`
3. Add optional "delete browsing data?" checkbox (exclude wallet)
4. Test: install → use → uninstall → verify clean → reinstall → verify works

### B-6 second (higher complexity)

1. Create `SingleInstance.h/.cpp` with pipe server/client
2. Integrate into `cef_browser_shell.cpp` after `CefExecuteProcess()` check
3. Test: launch → launch again → verify new window opens
4. Test: launch → crash → launch again → verify no stale pipe
5. Test: launch with URL arg → verify URL opens in new tab

### Total estimate: B-3 is ~2 hours of work, B-6 is ~4-6 hours

---

## Key Code Locations Reference

| What | File | Line |
|------|------|------|
| Profile lock acquisition | `cef_browser_shell.cpp` | 2778 |
| Error dialog | `cef_browser_shell.cpp` | 2779-2784 |
| CefExecuteProcess (subprocess check) | `cef_browser_shell.cpp` | 2732 |
| ProfileLock Windows impl | `ProfileLock.cpp` | 14-34 |
| ProfileLock macOS impl | `ProfileLock.cpp` | 51-67 |
| Lock release | `cef_browser_shell.cpp` | 3168 |
| CreateFullWindow | `WindowManager.cpp` | 122-221 |
| LaunchWithProfile | `ProfileManager.cpp` | 346-416 |
| Parse --profile arg | `ProfileManager.cpp` | 428-465 |
| Installer script | `installer/hodos-browser.iss` | Full file |
| Logger init | `cef_browser_shell.cpp` | 2736 |
| CEF log file | `cef_browser_shell.cpp` | 2754 |
| CEF cache path | `cef_browser_shell.cpp` | 2810-2813 |
| Session save/restore | `cef_browser_shell.cpp:316` / `simple_app.cpp:366` | |
| macOS lock + error | `cef_browser_shell_mac.mm` | 3834-3843 |
