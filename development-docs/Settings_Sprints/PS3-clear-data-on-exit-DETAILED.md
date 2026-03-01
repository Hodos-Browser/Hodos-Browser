# PS3: Clear Data on Exit — Detailed Implementation Plan

**Status**: Not Started  
**Complexity**: Medium  
**Estimated Time**: 2-3 hours  
**Dependencies**: PS2 (reuses clearing logic — or can be built independently)

---

## Executive Summary

When the browser closes and "Clear data on exit" is enabled, automatically clear browsing history, cookies, and cache. The setting already exists in the UI and persists — this sprint wires it to actual shutdown behavior.

---

## Current State Analysis

### What Exists
- **UI**: Toggle in `PrivacySettings.tsx` — "Clear data on exit"
- **Persistence**: `SettingsManager::SetClearDataOnExit()` saves `privacy.clearDataOnExit`
- **Clearing APIs**: `HistoryManager::DeleteAllHistory()`, `CefCookieManager::DeleteCookies()`

### What's Missing
- No shutdown hook reads `clearDataOnExit` setting
- Data is never automatically cleared

---

## Architecture Design

### When to Clear

**Hook Point**: `WM_CLOSE` handler in `WndProc` — before CEF shutdown begins.

This is the right place because:
1. CEF APIs are still available
2. All tabs are still open (can enumerate)
3. Runs before any CEF cleanup starts

### What to Clear

| Data Type | Method | Notes |
|-----------|--------|-------|
| History | `HistoryManager::GetInstance().DeleteAllHistory()` | Sync, fast |
| Cookies | `CefCookieManager::GetGlobalManager()->DeleteCookies("", "", callback)` | Async |
| Cache | See research below | Complex |
| localStorage/IndexedDB | Execute JS or delete files | Complex |

### What NOT to Clear
- Wallet data (separate database, sensitive)
- Settings (the toggle itself must persist!)
- Bookmarks (user's curated data)
- Block log (privacy protection data)
- Blocked domains list (user customizations)

---

## Research: CEF Cache Clearing

From research, CEF doesn't have a simple "clear cache" API. Options:

1. **`CefRequestContext::ClearHttpAuthCredentials()`** — only clears auth cache
2. **Delete cache directory** — risky while browser is running
3. **Recreate CefRequestContext** — too disruptive at shutdown
4. **`CefBrowser::ReloadIgnoreCache()`** — per-page, not useful here

**Decision**: For MVP, focus on history and cookies. Cache clearing can be added in Phase 2 by deleting the cache directory on next startup (detected via a flag file).

---

## Phase 1: Clear History and Cookies on Shutdown (2 hours)

### Step 1: Create Clearing Function

**File**: Add to `cef_browser_shell.cpp` or create `BrowsingDataCleaner.cpp`

```cpp
#include "include/core/HistoryManager.h"
#include "include/cef_cookie.h"

// Callback for async cookie deletion
class DeleteCookiesCallback : public CefDeleteCookiesCallback {
public:
    DeleteCookiesCallback(std::function<void()> onComplete) 
        : onComplete_(onComplete) {}
    
    void OnComplete(int num_deleted) override {
        LOG_INFO("Deleted " + std::to_string(num_deleted) + " cookies on exit");
        if (onComplete_) onComplete_();
    }
    
private:
    std::function<void()> onComplete_;
    IMPLEMENT_REFCOUNTING(DeleteCookiesCallback);
};

void ClearBrowsingDataOnExit() {
    LOG_INFO("Clearing browsing data on exit...");
    
    // Clear history (synchronous)
    HistoryManager::GetInstance().DeleteAllHistory();
    LOG_INFO("History cleared");
    
    // Clear cookies (asynchronous)
    auto cookieManager = CefCookieManager::GetGlobalManager();
    if (cookieManager) {
        // Empty strings = delete ALL cookies
        cookieManager->DeleteCookies("", "", new DeleteCookiesCallback([]() {
            LOG_INFO("Cookies cleared on exit");
        }));
        
        // Flush to ensure deletion is persisted
        cookieManager->FlushStore(nullptr);
    }
    
    LOG_INFO("Browsing data cleared on exit");
}
```

### Step 2: Hook into Shutdown

**File**: `cef_browser_shell.cpp` — in `WndProc`

```cpp
case WM_CLOSE: {
    auto& settings = SettingsManager::GetInstance();
    
    // Session restore (G2) — save BEFORE clearing
    if (settings.GetBrowserSettings().restoreSessionOnStart) {
        SessionManager::GetInstance().SaveSession();
    }
    
    // Clear data on exit (PS3) — clear AFTER session save
    if (settings.GetPrivacySettings().clearDataOnExit) {
        ClearBrowsingDataOnExit();
    }
    
    // Proceed with normal shutdown
    DestroyWindow(hWnd);
    break;
}
```

### Order Matters: Session Save vs Clear Data

If both "Restore previous session" and "Clear data on exit" are enabled:
1. **First**: Save session (URLs are saved)
2. **Then**: Clear data (history/cookies deleted)
3. **On next startup**: Session restores tabs, but history is empty

This is the expected behavior — session restore saves URLs, not browsing data.

---

## Phase 2: Cache Clearing (Future Enhancement)

### Approach: Flag File + Next-Startup Delete

1. On shutdown with `clearDataOnExit=true`, create a flag file: `{profile}/.clear_cache_on_start`
2. On next startup, check for flag file
3. If present, delete cache directory and remove flag
4. Then continue normal startup

```cpp
// On shutdown:
void FlagCacheClearOnStartup() {
    std::ofstream flag(profilePath + "/.clear_cache_on_start");
}

// On startup:
void ClearCacheIfFlagged() {
    std::string flagPath = profilePath + "/.clear_cache_on_start";
    if (std::filesystem::exists(flagPath)) {
        std::filesystem::remove_all(profilePath + "/cache");
        std::filesystem::remove(flagPath);
        LOG_INFO("Cache cleared on startup (flagged from previous exit)");
    }
}
```

**Decision**: Defer to post-MVP. History + cookies cover the main use case.

---

## Phase 3: Selective Clear Options (Future Enhancement)

Let users choose what gets cleared on exit:
- [ ] Clear history
- [ ] Clear cookies  
- [ ] Clear cache
- [ ] Clear site data (localStorage/IndexedDB)

This would require:
1. New settings: `clearHistoryOnExit`, `clearCookiesOnExit`, `clearCacheOnExit`
2. UI with multiple checkboxes
3. Selective clearing logic

**Decision**: Defer to post-MVP.

---

## Edge Cases & Conflicts

### Crash Exit
Data won't be cleared if the browser crashes (process killed). This is expected — `WM_CLOSE` isn't called on crashes.

### Multiple Windows/Profiles
Each profile's `clearDataOnExit` setting is independent. Only clear data for profiles that have it enabled.

### Conflict with Session Restore

| Session Restore | Clear on Exit | Behavior |
|-----------------|---------------|----------|
| OFF | OFF | Normal — data persists |
| OFF | ON | Data cleared — fresh start each time |
| ON | OFF | Session restores, data persists |
| ON | ON | Session restores, but history/cookies empty |

**Note**: Consider showing a UI warning if both are enabled: "Session restore will reopen your tabs, but browsing history and cookies will be cleared."

---

## Test Checklist

### Basic Functionality
- [ ] Enable "Clear data on exit"
- [ ] Browse several sites (accumulate history + cookies)
- [ ] Close browser → reopen
- [ ] History should be empty
- [ ] Sites should require re-login (cookies cleared)

### Setting Disabled
- [ ] Disable "Clear data on exit"
- [ ] Close browser → reopen
- [ ] History and cookies preserved

### What Should NOT Be Cleared
- [ ] The toggle setting itself persists
- [ ] Wallet data persists
- [ ] Bookmarks persist
- [ ] Other settings persist

### Conflict Scenarios
- [ ] Both session restore + clear on exit enabled → tabs restore, history empty
- [ ] Browser crash (kill process) → data NOT cleared (expected)

---

## Files to Modify

| File | Changes |
|------|---------|
| `cef_browser_shell.cpp` | Add `ClearBrowsingDataOnExit()`, hook in WM_CLOSE |

---

## Async Considerations

Cookie deletion is async via `CefDeleteCookiesCallback`. For clean shutdown:

**Option A**: Block shutdown until callback fires (add timeout)
```cpp
std::atomic<bool> cookiesCleared = false;
cookieManager->DeleteCookies("", "", new DeleteCookiesCallback([&]() {
    cookiesCleared = true;
}));

// Wait up to 2 seconds
auto start = std::chrono::steady_clock::now();
while (!cookiesCleared && 
       std::chrono::steady_clock::now() - start < std::chrono::seconds(2)) {
    CefDoMessageLoopWork();
}
```

**Option B**: Fire and forget — let CEF cleanup handle it
```cpp
cookieManager->DeleteCookies("", "", nullptr);
cookieManager->FlushStore(nullptr);
// Proceed with shutdown immediately
```

**Recommendation**: Option B for simplicity. `FlushStore()` ensures deletion is persisted before CEF shuts down.

---

**Last Updated**: 2026-02-28
