# PS2: Clear Browsing Data

**Status**: Not Started
**Complexity**: Medium
**Estimated Phases**: 2

---

## Current State

- UI exists in `PrivacySettings.tsx` — "Clear browsing data now" button under "Browsing Data" card
- Button sends `clear_browsing_data` IPC via `window.cefMessage?.send('clear_browsing_data', [])`
- **Not implemented**: No IPC handler exists in `simple_handler.cpp` — button does nothing

---

## What Needs to Happen

### Phase 1: Basic Clear All

**Goal**: Clicking the button clears history, cookies, and cache.

**Changes needed**:
- [ ] Add `clear_browsing_data` IPC handler in `simple_handler.cpp`
- [ ] Clear history: call `HistoryManager::GetInstance().ClearHistory()` (or add method if it doesn't exist)
- [ ] Clear cookies: use `CefCookieManager::GetGlobalManager()->DeleteCookies("", "", callback)` to delete all cookies
- [ ] Clear cache: use `CefBrowserHost::GetRequestContext()->ClearCertificateExceptions()` and `ClearSchemeHandlerFactories()`, or use `--disk-cache-dir` and delete the cache directory
- [ ] Send confirmation back to frontend (success/failure)
- [ ] Show toast/snackbar in UI confirming data was cleared

**Design decisions**:
- Clear everything at once or separate options? (Phase 1: clear all. Phase 2: granular)
- Should it close all tabs first? (Recommended: no — just clear the data stores)
- Should it clear the block log too? (Probably not — that's privacy protection data, not browsing data)
- What about localStorage/sessionStorage for web pages? (CEF may need `ClearSchemeHandlerFactories` or `RequestContext` clearing)

**C++ implementation sketch**:
```cpp
} else if (message_name == "clear_browsing_data") {
    // Clear history
    HistoryManager::GetInstance().ClearHistory();

    // Clear cookies
    CefCookieManager::GetGlobalManager()->DeleteCookies("", "", nullptr);

    // Clear cache (via request context)
    // Research: CefRequestContext::ClearCertificateExceptions,
    //           CefRequestContext::ClearHttpAuthCredentials

    // Notify frontend
    SendProcessMessage(PID_RENDERER, "browsing_data_cleared");
}
```

### Phase 2: Granular Clearing (Optional/Future)

**Goal**: Let users choose what to clear (history only, cookies only, cache only, or combination).

**Changes needed**:
- [ ] Update UI: checkboxes for History, Cookies, Cache, Site Data
- [ ] Time range selector (last hour, last day, last week, all time)
- [ ] Pass selection as IPC args: `clear_browsing_data` with `["history", "cookies"]`
- [ ] C++ handler reads args and selectively clears
- [ ] `HistoryManager::ClearHistoryRange(start, end)` for time-based clearing

---

## Architecture Considerations

**CEF cookie clearing**: `CefCookieManager::DeleteCookies(url, name, callback)` — empty strings for both means delete all. This is asynchronous; the callback fires when complete.

**Cache clearing**: CEF doesn't have a direct "clear cache" API. Options:
1. Delete files in `cache_path` directory (risky while browser is running)
2. `CefRequestContext::ClearCertificateExceptions()` — clears cert cache only
3. Start CEF with `--aggressive-cache-discard` flag
4. Use `CefURLRequest` cache bypass
5. Research CEF 136 APIs for proper cache clearing

**History clearing**: `HistoryManager` uses SQLite. Need to verify `ClearHistory()` method exists or add it.

---

## Test Checklist

- [ ] Visit several sites → click "Clear browsing data" → verify history is empty
- [ ] Log into a site → clear data → site requires re-login (cookies cleared)
- [ ] Verify block log and blocked domains are NOT cleared
- [ ] Verify settings are NOT cleared
- [ ] Verify wallet data is NOT cleared (separate DB)
- [ ] Button shows feedback (toast/snackbar) after clearing
- [ ] No crash or hang during clearing

---

**Last Updated**: 2026-02-28
