# G2: Restore Previous Session

**Status**: COMPLETE (Phases 1+2, 2026-03-02)
**Complexity**: Medium
**Estimated Phases**: 2 (Phase 3 crash recovery deferred indefinitely)

---

## Decisions (2026-03-01)

- **Lazy tab loading**: YES — only active tab loads content on restore. Other tabs show title/URL in tab bar but don't fetch content until clicked. This is the Chrome approach and critical for startup performance with many tabs.
- **Restore vs homepage**: When restore is enabled, skip homepage. Don't open homepage as an extra tab.
- **Conflict with PS3 (Clear on Exit)**: If both "restore session" and "clear on exit" are enabled, disable restore and show warning in settings UI. Don't allow both simultaneously.
- **Phase 3 (crash recovery)**: Deferred post-MVP. Significantly more complex.

---

## Current State

- UI exists in `GeneralSettings.tsx` — toggle switch
- Setting persists to `settings.json` via `SettingsManager`
- **Not wired**: No session save or restore logic exists anywhere in the codebase
- Tabs are managed by `TabManager` in C++ but no persistence

---

## Implementation (COMPLETE 2026-03-02)

### Phase 1: Save Session on Shutdown

**`cef_browser_shell.cpp`** — `SaveSession()` function called at top of `ShutdownApplication()`:
- [x] Checks `restoreSessionOnStart` setting — only saves if enabled
- [x] Iterates `TabManager::GetAllTabs()`, collects URLs + titles
- [x] Filters internal URLs (`127.0.0.1:5137`, `about:blank`, empty)
- [x] Writes `{version, tabs[{url, title}], activeTabIndex}` to `{profile}/session.json`
- [x] Per-profile sessions via `ProfileManager::GetCurrentProfileDataPath()`

### Phase 2: Restore Session on Startup

**`simple_app.cpp`** — Session restore in `OnContextInitialized()`:
- [x] Reads `restoreSessionOnStart` from SettingsManager
- [x] Parses `session.json`, creates tabs via `TabManager::CreateTab()` for each saved URL
- [x] Switches to saved active tab index
- [x] Deletes `session.json` after restore (prevents stale restores)
- [x] Falls back to NTP tab if restore fails, file missing, or corrupt JSON
- [x] Also fixed stale `coingeek.com` fallback → now uses NTP (`http://127.0.0.1:5137/newtab`)

**Note**: All tabs load their real URLs immediately (not lazy). True lazy loading deferred — would require `Tab::is_loaded` flag + `SwitchToTab()` modification.

### macOS
- [x] Placeholder comment in `cef_browser_shell_mac.mm` `ShutdownApplication()` — no tabs on macOS yet

### Phase 3: Crash Recovery (Optional/Future)

**Goal**: Recover tabs even after a crash (not clean shutdown).

**Changes needed**:
- [ ] Save session periodically (every 30s?) or on tab open/close
- [ ] Detect crash on next startup (session file exists but no clean shutdown marker)
- [ ] Show "Restore previous session?" prompt
- [ ] Option to always restore vs ask

**Design decisions**:
- This is significantly more complex — may defer to post-MVP
- Chrome uses a combination of session files + crash detection
- Performance impact of frequent session saves

---

## Edge Cases to Consider

- Browser crash mid-save — partial session file
- Multiple windows/profiles — each saves independently
- Tabs with POST data or auth state — URL alone won't restore the page state
- Very long sessions (100+ tabs) — startup performance
- Tabs on internal pages (settings, history) — restore or skip?

---

## Test Checklist (ALL PASSING 2026-03-02)

- [x] Toggle ON → open 3-4 tabs → close → reopen → all tabs restored
- [x] Correct tab is active (same as when closed)
- [x] Tab titles show in tab bar
- [x] Toggle OFF → close → reopen → only NTP opens
- [x] No `session.json` → NTP opens (graceful fallback)
- [x] Corrupt `session.json` → NTP opens (no crash)
- [x] Session with only internal URLs → filtered → NTP on restart
- [x] `session.json` deleted after successful restore (no stale data)
- [x] C++ builds clean

---

**Last Updated**: 2026-03-02
