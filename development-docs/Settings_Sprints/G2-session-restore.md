# G2: Restore Previous Session

**Status**: Not Started
**Complexity**: Medium
**Estimated Phases**: 3

---

## Current State

- UI exists in `GeneralSettings.tsx` — toggle switch
- Setting persists to `settings.json` via `SettingsManager`
- **Not wired**: No session save or restore logic exists anywhere in the codebase
- Tabs are managed by `TabManager` in C++ but no persistence

---

## What Needs to Happen

### Phase 1: Save Session on Shutdown

**Goal**: When browser closes, save the list of open tab URLs.

**Changes needed**:
- [ ] Add session file: `%APPDATA%/HodosBrowser/{profile}/session.json`
- [ ] Hook into browser shutdown (WM_CLOSE handler or `OnBeforeClose`)
- [ ] Iterate `TabManager` tabs, collect URLs
- [ ] Write to JSON: `{ "tabs": ["url1", "url2", ...], "activeTabIndex": 0 }`
- [ ] Skip internal URLs (127.0.0.1, about:blank) or include them?

**Design decisions**:
- Where to hook: `WM_CLOSE` in `WndProc` vs `CefLifeSpanHandler::OnBeforeClose`?
- Should we save on every tab open/close (crash recovery) or only on clean shutdown?
- What about incognito/private tabs — should they be saved?
- Per-profile sessions (each profile has its own session file)

### Phase 2: Restore Session on Startup

**Goal**: When browser launches with the setting enabled, reopen saved tabs.

**Changes needed**:
- [ ] Read `restoreSessionOnStart` setting during startup (before first tab creation)
- [ ] Parse `session.json` if it exists
- [ ] Create tabs for each saved URL via `TabManager::CreateTab()`
- [ ] Set the active tab to the saved active index
- [ ] Delete/clear session file after successful restore (avoid stale restores)

**Design decisions**:
- Restore vs homepage: if restore is enabled, skip homepage? Or open homepage + restored tabs?
- What if session file is corrupt or URLs are invalid?
- Tab creation timing: all tabs at once or sequential? (CEF has browser creation limits)
- Loading behavior: load all tabs eagerly or only active tab (lazy load)?

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

## Test Checklist

- [ ] Open 5 tabs → close browser → reopen → all 5 tabs restored
- [ ] Verify active tab is correctly restored
- [ ] Toggle setting off → close → reopen → only homepage opens
- [ ] Verify session file is created in correct profile directory
- [ ] Verify internal URLs (hodos://settings) are handled gracefully
- [ ] Test with multiple profiles — each restores independently
- [ ] Crash simulation — kill process, verify next launch behavior

---

**Last Updated**: 2026-02-28
