# Multi-Instance & Profile Testing Strategy

**Created**: 2026-02-25
**Context**: Sprint 9d raised questions about dev vs production behavior for profile switching
**Related**: working-notes.md #9 (Dev Environment vs Production / Multi-Instance Behavior)

---

## The Problem

In development:
- Frontend runs on `localhost:5137` (Vite dev server)
- CEF shell connects to dev server
- Launching "new instance" opens a new CEF process pointing to same dev server
- This works but doesn't test production behavior

In production:
- Frontend bundled into static files
- Need to decide: embedded via custom scheme handler OR bundled HTTP server
- New instance = new `HodosBrowser.exe` process
- Profile switching = launch new process with `--profile="Name"` arg

**Core question**: How do we test production-like multi-instance behavior during development?

---

## Research Findings

### How Chrome Handles It

Chrome uses two command-line arguments:
```
chrome.exe --user-data-dir="C:\Users\xxx\AppData\Local\Google\Chrome\User Data" --profile-directory="Profile 1"
```

- `--user-data-dir`: Base directory for all profiles
- `--profile-directory`: Subdirectory name for specific profile

Multiple instances can run simultaneously with different `--profile-directory` values sharing the same `--user-data-dir`. Chrome manages locking at the profile level, not the app level.

### CEF Multi-Profile Support

CEF has no built-in profile picker. Multi-profile requires:
1. Custom `CefRequestContext` per profile (manages cookies, cache, localStorage)
2. Pass context to `CefBrowserHost::CreateBrowser()`
3. Build your own profile picker UI

**Our current approach** (ProfileManager + `--profile=` arg) is correct — it's the same pattern Chrome uses.

### Single-Instance vs Multi-Instance

**Option A: Single-Instance (Mutex Lock)**
```cpp
HANDLE hMutex = CreateMutex(NULL, TRUE, L"HodosBrowser.Instance.Mutex");
if (GetLastError() == ERROR_ALREADY_EXISTS) {
    // Find existing window, activate it, pass command to it
    return 0;
}
```
- First instance owns mutex
- Second instance exits immediately (or passes URL to first via IPC)
- Used by: some text editors, Slack

**Option B: Multi-Instance per Profile (Chrome's model)**
- Each profile = separate process
- Multiple instances allowed if different profiles
- Lock at profile level, not app level
- Used by: Chrome, Firefox, Edge

**Recommendation**: Option B (Chrome's model) — we already started this with `--profile="Name"`

---

## Recommendations

### 1. Testing Strategy (General)

**Three test modes**:

| Mode | When | How |
|------|------|-----|
| **Dev** | Daily development | Vite dev server + CEF, hot reload, multiple windows connect to same server |
| **Stage** | Pre-release testing | Build frontend (`npm run build`), serve from `dist/`, CEF loads bundled files |
| **Prod-like** | Full integration | Packaged exe, no dev server, tests actual installer output |

**Recommendation**: Add a **Stage mode** for testing multi-instance behavior:
```powershell
# Build frontend to static files
cd frontend && npm run build

# Run CEF pointing to bundled files (not dev server)
$env:HODOS_USE_BUNDLED = "1"
./HodosBrowserShell.exe --profile="Default"
```

This lets you:
- Test profile switching launches actual new process
- Test startup profile picker with bundled UI
- Simulate production without full installer

### 2. Profile Switching (Running Instance)

**Current behavior**: Click profile → launches new `HodosBrowserShell.exe --profile="Name"`

**This is correct!** Chrome does the same thing. Each profile runs in its own process for:
- Security isolation
- Independent crash handling
- Clean separation of cookies/storage

**Dev testing improvement**: When `HODOS_USE_BUNDLED=1`:
- New process should also use bundled files
- Currently: new process connects to dev server (if running)
- Fix: Pass environment or use bundled path detection

**Implementation**:
```cpp
// In LaunchWithProfile()
std::wstring cmdLine = L"\"" + exePath + L"\" --profile=\"" + profileId + L"\"";

// Add: Pass bundled mode flag
if (g_using_bundled_frontend) {
    cmdLine += L" --bundled";
}
```

### 3. Startup Profile Picker

**When to show**: If user has 2+ profiles saved in `profiles.json`

**User flows**:

| Scenario | Behavior |
|----------|----------|
| 0 profiles | Create "Default" automatically, no picker |
| 1 profile | Load it automatically, no picker |
| 2+ profiles | Show profile picker overlay before main UI |
| User setting: "Always ask" | Show picker even with 1 profile |
| Command line `--profile="X"` | Skip picker, load specified profile |

**Implementation approach**:

```cpp
// In WinMain, before creating main browser:
int profileCount = ProfileManager::GetInstance().GetAllProfiles().size();
std::string cmdProfile = ProfileManager::ParseProfileArgument(cmdLine);

if (!cmdProfile.empty()) {
    // Command line specified profile — use it directly
    ProfileManager::GetInstance().SetCurrentProfileId(cmdProfile);
    CreateMainBrowser();
} else if (profileCount <= 1) {
    // 0 or 1 profile — auto-select, no picker
    CreateMainBrowser();
} else if (ProfileManager::GetInstance().ShouldShowPickerOnStartup()) {
    // Multiple profiles — show startup picker
    CreateStartupProfilePicker();  // Blocks until selection
    CreateMainBrowser();
} else {
    // Multiple profiles but "remember choice" was set
    CreateMainBrowser();  // Uses last-used profile
}
```

**Startup picker UI**: 
- Separate overlay window (same pattern as BackupOverlayRoot)
- Shows BEFORE main browser window
- Full-screen or centered modal
- "Remember my choice" checkbox → sets `ShouldShowPickerOnStartup(false)`

### 4. Profile-Level Locking

Prevent two instances from using the same profile:

```cpp
// When loading a profile, create a lock file
std::string lockFile = profilePath + "/profile.lock";

// Try to exclusively open the lock file
HANDLE hLock = CreateFile(lockFile.c_str(), GENERIC_WRITE, 0, NULL, 
                          CREATE_ALWAYS, FILE_FLAG_DELETE_ON_CLOSE, NULL);

if (hLock == INVALID_HANDLE_VALUE) {
    // Another instance is using this profile
    ShowError("This profile is already in use by another window.");
    return false;
}

// Lock held until process exits (FILE_FLAG_DELETE_ON_CLOSE)
```

---

## Implementation Plan

### Phase 1: Stage Mode (Testing Infrastructure)
- [ ] Add `HODOS_USE_BUNDLED` environment variable check
- [ ] When set, load from `frontend/dist/` instead of `localhost:5137`
- [ ] Pass mode flag when launching child processes

### Phase 2: Startup Profile Picker
- [ ] Create `StartupProfilePickerRoot.tsx` (full-screen, not toolbar overlay)
- [ ] Add C++ startup logic to show picker before main browser
- [ ] Add "Remember my choice" setting to `profiles.json`
- [ ] Handle `--profile="X"` command line to skip picker

### Phase 3: Profile Locking
- [ ] Add lock file creation when profile loads
- [ ] Check for existing lock before loading
- [ ] Show error if profile already in use
- [ ] Auto-cleanup via `FILE_FLAG_DELETE_ON_CLOSE`

### Phase 4: Dev/Stage/Prod Parity
- [ ] Document the three modes in README
- [ ] Add npm script: `npm run build:stage` that builds + launches CEF in bundled mode
- [ ] CI/CD: Run Stage mode tests before release

---

## Quick Reference: Testing Multi-Instance

**Dev mode** (current):
```powershell
# Terminal 1: Frontend dev server
cd frontend && npm run dev

# Terminal 2: CEF
cd cef-native/build/bin/Release && ./HodosBrowserShell.exe
```
- Profile switch opens new CEF window → connects to same dev server ✓
- Not production-realistic but fine for UI development

**Stage mode** (recommended):
```powershell
# Build frontend
cd frontend && npm run build

# Run with bundled flag
$env:HODOS_USE_BUNDLED = "1"
cd cef-native/build/bin/Release && ./HodosBrowserShell.exe --profile="Default"

# Second instance (different profile)
./HodosBrowserShell.exe --profile="Work"
```
- Tests actual multi-process behavior
- Tests profile switching launches real process
- Frontend loaded from bundled files

**Full integration** (pre-release):
```powershell
# Build installer (future)
.\build-installer.ps1

# Install to test machine
# Test fresh install, upgrade, multi-profile
```

---

## Summary

| Question | Answer |
|----------|--------|
| **How to test multi-instance in dev?** | Add Stage mode: `HODOS_USE_BUNDLED=1` loads bundled files |
| **Profile switching behavior?** | Launch new process with `--profile=` (Chrome's model) — correct! |
| **Startup with multiple profiles?** | Show profile picker overlay before main browser if 2+ profiles |
| **Prevent same profile twice?** | Profile-level lock file with `FILE_FLAG_DELETE_ON_CLOSE` |

**Effort estimate**: 
- Phase 1 (Stage mode): 2-4 hours
- Phase 2 (Startup picker): 4-6 hours  
- Phase 3 (Profile locking): 1-2 hours
- Phase 4 (Documentation): 1 hour

**Priority**: Phase 1 first (enables proper testing), then Phase 2 (user-facing feature).
