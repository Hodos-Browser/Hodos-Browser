# G2: Restore Previous Session

**Status**: Not Started
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

**Design decisions** (resolved):
- Restore vs homepage: **Skip homepage** when restore is enabled (decided 2026-03-01)
- What if session file is corrupt or URLs are invalid? Fall back to homepage.
- Tab creation timing: Create all tab HWNDs at once but only load active tab content
- Loading behavior: **Lazy load** — only active tab loads content (decided 2026-03-01). Other tabs show title/URL in tab bar. Content loads on first click. This requires TabManager to support "unloaded" tab state (HWND created, browser NOT navigated yet).

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
