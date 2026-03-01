# PS3: Clear Data on Exit

**Status**: Not Started
**Complexity**: Medium
**Estimated Phases**: 2
**Depends on**: PS2 (reuses clearing logic)

---

## Current State

- UI exists in `PrivacySettings.tsx` — "Clear data on exit" toggle switch
- Setting persists to `settings.json` via `SettingsManager` as `privacy.clearDataOnExit`
- **Not implemented**: No shutdown hook reads this setting — data is never cleared on exit

---

## What Needs to Happen

### Phase 1: Clear on Clean Shutdown

**Goal**: When the browser closes normally and the setting is enabled, clear browsing data automatically.

**Changes needed**:
- [ ] Hook into browser shutdown — either `WM_CLOSE` in `WndProc` or `CefLifeSpanHandler::OnBeforeClose`
- [ ] Read `SettingsManager::GetClearDataOnExit()` during shutdown
- [ ] If enabled, reuse PS2's clearing logic (history, cookies, cache)
- [ ] Ensure clearing completes before CEF shuts down (synchronous or wait for callback)
- [ ] macOS: hook into `applicationShouldTerminate:` or `windowWillClose:` equivalent

**Design decisions**:
- Where to hook: `WM_CLOSE` runs before CEF shutdown begins — good place to clear while CEF APIs are still available
- Synchronous vs async: clearing must complete before process exits. May need to block the close handler briefly.
- Should this clear the same data as the "Clear now" button? (Recommended: yes — same scope)
- What if clearing fails or hangs? Timeout after N seconds and close anyway

**C++ implementation sketch**:
```cpp
// In WndProc, WM_CLOSE handler:
if (SettingsManager::GetInstance().GetPrivacySettings().clearDataOnExit) {
    HistoryManager::GetInstance().ClearHistory();
    CefCookieManager::GetGlobalManager()->DeleteCookies("", "", nullptr);
    // Cache clearing...
}
// Then proceed with normal shutdown
```

### Phase 2: Selective Clear on Exit (Optional/Future)

**Goal**: Let users choose what gets cleared on exit (independent of the "Clear now" options).

**Changes needed**:
- [ ] Add sub-options under the toggle: "Clear history", "Clear cookies", "Clear cache"
- [ ] Persist each sub-option in settings
- [ ] Shutdown hook reads the granular settings

---

## Edge Cases

- **Crash exit**: Data won't be cleared if the process is killed. This is expected — crash recovery (G2 Phase 3) would conflict anyway.
- **Multiple profiles**: Each profile's `clearDataOnExit` setting is independent. Only clear data for profiles that have it enabled.
- **Multiple windows**: If multiple windows are open for the same profile, only clear when the LAST window closes.
- **Session restore conflict**: If both "Restore previous session" (G2) and "Clear data on exit" are enabled, session URLs should be saved BEFORE data is cleared. Or: show a warning in settings that these two options conflict.

---

## Test Checklist

- [ ] Enable "Clear data on exit" → visit sites → close browser → reopen → history is empty
- [ ] Verify cookies are cleared (sites require re-login after restart)
- [ ] Disable toggle → close → reopen → history preserved
- [ ] Verify setting persists (the toggle itself isn't cleared)
- [ ] Verify wallet data is NOT cleared
- [ ] Verify other profiles' data is NOT cleared
- [ ] Kill process (crash) → reopen → data still exists (only clean shutdown clears)

---

**Last Updated**: 2026-02-28
