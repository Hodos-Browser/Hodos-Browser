# G2: Session Restore — Detailed Implementation Plan

**Status**: Not Started  
**Complexity**: Medium  
**Estimated Time**: 4-6 hours  
**Dependencies**: None

---

## Executive Summary

Implement session save/restore so users can reopen their tabs from the previous browsing session. This is a highly requested feature that significantly improves UX for power users.

---

## Current State Analysis

### What Exists
- **UI**: Toggle in `GeneralSettings.tsx` — "Restore previous session"
- **Persistence**: `SettingsManager::SetRestoreSessionOnStart(bool)` saves to `settings.json`
- **TabManager**: Manages all open tabs with full state (URL, title, hwnd)

### What's Missing
- No `session.json` file to persist tabs on shutdown
- No shutdown hook to save session
- No startup logic to restore tabs
- No handling of crash recovery

---

## Architecture Design

### Session File Format

**Location**: `%APPDATA%/HodosBrowser/{profile}/session.json`

```json
{
  "version": 1,
  "savedAt": "2026-02-28T18:30:00Z",
  "cleanShutdown": true,
  "activeTabIndex": 2,
  "tabs": [
    {
      "url": "https://example.com/page1",
      "title": "Page 1"
    },
    {
      "url": "https://example.com/page2",
      "title": "Page 2"
    },
    {
      "url": "https://github.com/user/repo",
      "title": "GitHub - repo"
    }
  ]
}
```

### Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Save timing | On shutdown only (Phase 1) | Simplicity; crash recovery is Phase 3 |
| URL filtering | Skip internal URLs | Don't restore settings/127.0.0.1 pages |
| Restore vs homepage | Restore replaces homepage | Matches Chrome/Brave behavior |
| Tab creation | Sequential with delay | CEF browser creation limits |

---

## Phase 1: Save Session on Shutdown (2-3 hours)

### Step 1: Create SessionManager Class

**File**: `include/core/SessionManager.h`

```cpp
#pragma once

#include <string>
#include <vector>

struct SavedTab {
    std::string url;
    std::string title;
};

struct SessionData {
    int version = 1;
    std::string savedAt;
    bool cleanShutdown = false;
    int activeTabIndex = 0;
    std::vector<SavedTab> tabs;
};

class SessionManager {
public:
    static SessionManager& GetInstance();
    
    // Initialize with profile path
    void Initialize(const std::string& profilePath);
    
    // Save current session to disk
    bool SaveSession();
    
    // Load session from disk (returns empty if none/corrupt)
    SessionData LoadSession();
    
    // Clear saved session (after successful restore)
    void ClearSession();
    
    // Check if session file exists
    bool HasSavedSession() const;

private:
    SessionManager() = default;
    std::string sessionFilePath_;
    
    static bool ShouldSaveUrl(const std::string& url);
    static std::string GetCurrentTimestamp();
};
```

**File**: `src/core/SessionManager.cpp`

```cpp
#include "include/core/SessionManager.h"
#include "include/core/TabManager.h"
#include <nlohmann/json.hpp>
#include <fstream>
#include <chrono>
#include <iomanip>
#include <sstream>

SessionManager& SessionManager::GetInstance() {
    static SessionManager instance;
    return instance;
}

void SessionManager::Initialize(const std::string& profilePath) {
#ifdef _WIN32
    sessionFilePath_ = profilePath + "\\session.json";
#else
    sessionFilePath_ = profilePath + "/session.json";
#endif
}

bool SessionManager::ShouldSaveUrl(const std::string& url) {
    // Skip internal URLs
    if (url.find("127.0.0.1") != std::string::npos) return false;
    if (url.find("localhost") != std::string::npos) return false;
    if (url.find("about:") == 0) return false;
    if (url.find("hodos://") == 0) return false; // Skip internal pages
    if (url.empty()) return false;
    return true;
}

std::string SessionManager::GetCurrentTimestamp() {
    auto now = std::chrono::system_clock::now();
    auto time = std::chrono::system_clock::to_time_t(now);
    std::stringstream ss;
    ss << std::put_time(std::gmtime(&time), "%Y-%m-%dT%H:%M:%SZ");
    return ss.str();
}

bool SessionManager::SaveSession() {
    if (sessionFilePath_.empty()) return false;
    
    auto& tabManager = TabManager::GetInstance();
    auto tabs = tabManager.GetAllTabs();
    
    nlohmann::json j;
    j["version"] = 1;
    j["savedAt"] = GetCurrentTimestamp();
    j["cleanShutdown"] = true;
    j["activeTabIndex"] = 0;
    
    nlohmann::json tabsArray = nlohmann::json::array();
    int activeIndex = 0;
    int savedCount = 0;
    
    for (size_t i = 0; i < tabs.size(); i++) {
        Tab* tab = tabs[i];
        if (!ShouldSaveUrl(tab->url)) continue;
        
        nlohmann::json tabObj;
        tabObj["url"] = tab->url;
        tabObj["title"] = tab->title;
        tabsArray.push_back(tabObj);
        
        if (tab->id == tabManager.GetActiveTabId()) {
            activeIndex = savedCount;
        }
        savedCount++;
    }
    
    j["tabs"] = tabsArray;
    j["activeTabIndex"] = activeIndex;
    
    try {
        std::ofstream file(sessionFilePath_);
        if (!file.is_open()) return false;
        file << j.dump(2);
        return true;
    } catch (...) {
        return false;
    }
}

SessionData SessionManager::LoadSession() {
    SessionData data;
    if (sessionFilePath_.empty()) return data;
    
    try {
        std::ifstream file(sessionFilePath_);
        if (!file.is_open()) return data;
        
        nlohmann::json j;
        file >> j;
        
        data.version = j.value("version", 1);
        data.savedAt = j.value("savedAt", "");
        data.cleanShutdown = j.value("cleanShutdown", false);
        data.activeTabIndex = j.value("activeTabIndex", 0);
        
        if (j.contains("tabs") && j["tabs"].is_array()) {
            for (const auto& tabObj : j["tabs"]) {
                SavedTab tab;
                tab.url = tabObj.value("url", "");
                tab.title = tabObj.value("title", "");
                if (!tab.url.empty()) {
                    data.tabs.push_back(tab);
                }
            }
        }
    } catch (...) {
        // Corrupt file — return empty
    }
    
    return data;
}

void SessionManager::ClearSession() {
    if (sessionFilePath_.empty()) return;
    std::remove(sessionFilePath_.c_str());
}

bool SessionManager::HasSavedSession() const {
    if (sessionFilePath_.empty()) return false;
    std::ifstream file(sessionFilePath_);
    return file.good();
}
```

### Step 2: Hook into Shutdown

**File**: `cef_browser_shell.cpp` — in `WndProc` WM_CLOSE handler

```cpp
case WM_CLOSE: {
    // Save session before closing
    auto& settings = SettingsManager::GetInstance();
    if (settings.GetBrowserSettings().restoreSessionOnStart) {
        SessionManager::GetInstance().SaveSession();
        LOG_INFO("Session saved on shutdown");
    }
    
    // Existing shutdown logic...
    DestroyWindow(hWnd);
    break;
}
```

---

## Phase 2: Restore Session on Startup (2-3 hours)

### Step 1: Restore Logic in Startup Sequence

**File**: `cef_browser_shell.cpp` — after `InitializeApp()` or similar startup point

```cpp
void RestoreSessionIfEnabled(HWND parentHwnd, int x, int y, int width, int height) {
    auto& settings = SettingsManager::GetInstance();
    if (!settings.GetBrowserSettings().restoreSessionOnStart) {
        return; // Setting disabled
    }
    
    auto& sessionManager = SessionManager::GetInstance();
    if (!sessionManager.HasSavedSession()) {
        return; // No session to restore
    }
    
    SessionData session = sessionManager.LoadSession();
    if (session.tabs.empty()) {
        return; // Empty session
    }
    
    LOG_INFO("Restoring session with " + std::to_string(session.tabs.size()) + " tabs");
    
    auto& tabManager = TabManager::GetInstance();
    
    // Create tabs for each saved URL
    int firstTabId = -1;
    for (size_t i = 0; i < session.tabs.size(); i++) {
        const auto& savedTab = session.tabs[i];
        int tabId = tabManager.CreateTab(savedTab.url, parentHwnd, x, y, width, height);
        
        if (i == 0) {
            firstTabId = tabId;
        }
        
        // Switch to the active tab from the session
        if (static_cast<int>(i) == session.activeTabIndex) {
            tabManager.SwitchToTab(tabId);
        }
    }
    
    // Clear session after successful restore
    sessionManager.ClearSession();
    
    LOG_INFO("Session restored successfully");
}
```

### Step 2: Integrate with Initial Tab Creation

In the main window creation code, replace the default homepage tab creation with session-aware logic:

```cpp
// Instead of always creating homepage tab:
// tabManager.CreateTab(settings.GetBrowserSettings().homepage, ...);

// Do:
auto& sessionManager = SessionManager::GetInstance();
auto& settings = SettingsManager::GetInstance();

if (settings.GetBrowserSettings().restoreSessionOnStart && 
    sessionManager.HasSavedSession()) {
    RestoreSessionIfEnabled(parentHwnd, x, y, width, height);
} else {
    // No session to restore — open homepage
    tabManager.CreateTab(settings.GetBrowserSettings().homepage, parentHwnd, x, y, width, height);
}
```

---

## Phase 3: Crash Recovery (Future — Post-MVP)

### Design Overview

For crash recovery, we need:
1. **Periodic session save** (every 30-60 seconds)
2. **Crash detection** via `cleanShutdown` flag
3. **Recovery prompt** on startup if crash detected

### Implementation Sketch

```cpp
// On startup: if session exists but cleanShutdown=false
// Show prompt: "Your previous session ended unexpectedly. Restore tabs?"
// User can choose: Restore / Start Fresh

// Periodic save via CefPostDelayedTask:
void SchedulePeriodicSessionSave() {
    CefPostDelayedTask(TID_UI, base::BindOnce([]() {
        // Mark as dirty (not clean shutdown)
        SessionManager::GetInstance().SaveSessionDirty();
        SchedulePeriodicSessionSave(); // Reschedule
    }), 60000); // Every 60 seconds
}
```

**Decision**: Defer to post-MVP due to complexity and potential conflicts with "Clear data on exit".

---

## Gaps & Questions

| Gap | Resolution |
|-----|------------|
| Tab creation rate limiting | CEF handles this; create sequentially |
| Very large sessions (100+ tabs) | Lazy loading (defer to future) |
| Tabs with POST data | URL-only restore (acceptable limitation) |
| Conflict with "Clear data on exit" | If both enabled, clear happens AFTER session save |
| Multiple windows | Each saves independently (current single-window focus) |

---

## Test Checklist

### Basic Functionality
- [ ] Enable "Restore previous session" in settings
- [ ] Open 5 tabs → close browser → reopen → all 5 tabs restored
- [ ] Verify active tab is correctly restored
- [ ] Verify tab order is preserved

### Setting Disabled
- [ ] Disable "Restore previous session"
- [ ] Open 5 tabs → close browser → reopen → only homepage opens

### Edge Cases
- [ ] Internal URLs (127.0.0.1, hodos://settings) are NOT restored
- [ ] Empty session file → opens homepage
- [ ] Corrupt session file → opens homepage (graceful fallback)
- [ ] Setting persists across restarts

### Per-Profile
- [ ] Different profiles have independent sessions
- [ ] Profile A session doesn't affect Profile B

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `include/core/SessionManager.h` | **CREATE** | Session manager header |
| `src/core/SessionManager.cpp` | **CREATE** | Session manager implementation |
| `cef_browser_shell.cpp` | MODIFY | Shutdown hook + startup restore |
| `CMakeLists.txt` | MODIFY | Add SessionManager to build |

---

**Last Updated**: 2026-02-28
