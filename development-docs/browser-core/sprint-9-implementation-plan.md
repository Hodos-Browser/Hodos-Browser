# Sprint 9: Settings Persistence + Profile Import — Implementation Plan

**Created**: 2026-02-24
**Updated**: 2026-02-25 (Sprint 9 COMPLETE)
**Author**: Edwin (AI Assistant)
**Status**: **✅ COMPLETE**
**Actual Duration**: 2 days

---

## Progress Tracker

| Component | Status | Notes |
|-----------|--------|-------|
| **9a: Settings Persistence** | ✅ Complete | Tested 2026-02-25, settings persist across restart |
| **9b: Profile Import** | ✅ Complete | 4200 Chrome history entries imported successfully |
| **9c: Clear Browsing Data** | ✅ Complete | UI improvements implemented 2026-02-25 |
| **9d: Multi-Profile Support** | ✅ Complete | ProfilePickerOverlayRoot working 2026-02-25 14:53 MST |

### Sprint 9a Files Created/Modified

**New Files:**
- `cef-native/include/core/SettingsManager.h` — Header with settings structs + singleton
- `cef-native/src/core/SettingsManager.cpp` — Implementation (Load/Save/Setters)
- `frontend/src/hooks/useSettings.ts` — React hook for settings IPC

**Modified Files:**
- `cef-native/CMakeLists.txt` — Added SettingsManager sources
- `cef-native/cef_browser_shell.cpp` — Init SettingsManager at startup
- `cef-native/src/handlers/simple_handler.cpp` — IPC handlers (settings_get_all, settings_set, settings_update_all)
- `cef-native/src/handlers/simple_render_process_handler.cpp` — Response handler (settings_response)
- `frontend/src/pages/SettingsOverlayRoot.tsx` — Full settings UI with tabs

### Sprint 9b Files Created/Modified

**New Files:**
- `cef-native/include/core/ProfileImporter.h` — Header with DetectedProfile, ImportResult structs
- `cef-native/src/core/ProfileImporter.cpp` — Full implementation (detect, import bookmarks, import history)
- `frontend/src/hooks/useImport.ts` — React hook for import IPC

**Modified Files:**
- `cef-native/CMakeLists.txt` — Added ProfileImporter sources
- `cef-native/src/handlers/simple_handler.cpp` — IPC handlers (import_detect_profiles, import_bookmarks, import_history, import_all)
- `cef-native/src/handlers/simple_render_process_handler.cpp` — Response handlers (import_profiles_result, import_complete)
- `frontend/src/pages/SettingsOverlayRoot.tsx` — Added Import tab with profile cards and buttons

### Sprint 9b Bugs Fixed

1. **Windows `CopyFile` macro conflict** — Windows API `CopyFile` is a macro that expands to `CopyFileA`/`CopyFileW`. Renamed our method to `CopyFilePortable`.

2. **String/int type mismatch in IPC** — Frontend sent `String(10000)` for maxEntries, but C++ used `GetInt(1)` which returned 0 for string types. Added type checking: `if (args->GetType(1) == VTYPE_STRING)`.

3. **SQLite query debugging** — Added extensive logging to diagnose why `SELECT` returned 0 rows when `COUNT(*)` returned 4200. Root cause was bug #2.

### Sprint 9c Files Modified

**Modified Files:**
- `frontend/src/components/HistoryPanel.tsx` — Added time range selector, pagination (20/page), confirmation dialog
- `frontend/src/components/CookiesPanel.tsx` — Added sort dropdown (blocked first/most/largest), pagination, renamed "Delete Selected" → "Delete Cookie"
- `frontend/src/components/CachePanel.tsx` — Added cookie deletion warning dialog ("You'll be signed out...")

**Features Implemented:**
1. **Time Range Selector** — Clear history from: Last hour / Last 24 hours / Last 7 days / All time
2. **Cookie Warning** — Non-scary dialog before deleting all cookies: "You'll need to log back in to sites like Google, YouTube, Twitter..."
3. **Cookie Sort** — Sort by: Default, Blocked first, Most cookies, Largest
4. **Pagination** — Both History and Cookies panels show 20 items per page with navigation
5. **Button Text Fix** — "Delete Selected" → "Delete Cookie" (clearer for single-select mode)

### Sprint 9d Files Created/Modified

**New Files:**
- `cef-native/include/core/ProfileManager.h` — Header with ProfileInfo struct, singleton
- `cef-native/src/core/ProfileManager.cpp` — Full implementation (CRUD, launch with --profile=)
- `frontend/src/hooks/useProfiles.ts` — React hook for profile IPC
- `frontend/src/pages/ProfilePickerOverlayRoot.tsx` — Profile picker overlay UI

**Modified Files:**
- `cef-native/CMakeLists.txt` — Added ProfileManager sources
- `cef-native/cef_browser_shell.cpp` — Added ProfilePanelOverlayWndProc with full keyboard handling
- `cef-native/src/handlers/simple_app.cpp` — CreateProfilePanelOverlay, ShowProfilePanelOverlay, HideProfilePanelOverlay
- `cef-native/src/handlers/simple_handler.cpp` — IPC handlers (profiles_get_all, create, rename, delete, switch), OnAfterCreated for profilepanel role
- `cef-native/src/handlers/simple_handler.h` — GetProfilePanelBrowser() static method
- `frontend/src/pages/MainBrowserView.tsx` — Profile button in toolbar

**Features Implemented:**
1. **Profile CRUD** — Create, rename, delete profiles stored in `profiles.json`
2. **Profile Switching** — Launches new browser instance with `--profile="Name"` argument
3. **Profile Picker Overlay** — Lists profiles with avatars, colors, create form
4. **Avatar Support** — Custom avatar images (base64) or auto-initial with color
5. **Per-Profile Data** — Each profile gets own history, cookies, bookmarks directory

### Sprint 9d Critical Lessons: CEF Overlay Keyboard Input

**Problem:** Text input didn't work in profile picker overlay (file input worked, text didn't).

**Root Cause:** Missing keyboard event forwarding in WndProc.

**CEF Overlay Keyboard Input Checklist:**

1. **HWND Style:** `WS_POPUP | WS_VISIBLE` (NOT just `WS_POPUP`)

2. **Browser Settings:**
   ```cpp
   settings.javascript_access_clipboard = STATE_ENABLED;
   settings.javascript_dom_paste = STATE_ENABLED;
   ```

3. **WndProc (CRITICAL):**
   ```cpp
   case WM_MOUSEACTIVATE:
       return MA_ACTIVATE;  // NOT MA_NOACTIVATE!
   
   case WM_LBUTTONDOWN:
       SetFocus(hwnd);  // Windows focus
       browser->GetHost()->SetFocus(true);  // CEF focus
       browser->GetHost()->SendMouseClickEvent(...);
       return 0;
   
   case WM_KEYDOWN:
   case WM_KEYUP:
   case WM_CHAR:
       // Forward ALL keyboard events to CEF browser
       browser->GetHost()->SendKeyEvent(key_event);
       return 0;
   ```

4. **OnAfterCreated:**
   ```cpp
   browser->GetHost()->SetFocus(true);
   // Delayed WasResized + Invalidate after 150ms
   ```

5. **React Inputs:** Use native `<input>` elements, NOT MUI TextField

6. **File Inputs:** Use VISIBLE file inputs, not hidden+click trigger

**Reference:** `WalletOverlayWndProc` in `cef_browser_shell.cpp` has working pattern

### UI Architecture Rule (Learned 9d)

**NEVER add inline panels/menus/dropdowns to MainBrowserView.tsx (header_hwnd)**

All panels must be overlays in separate CEF subprocesses:
- Security: Isolated V8 contexts
- Performance: Doesn't block main browser thread
- Consistency: Same UX pattern across all panels

---

## Overview

Sprint 9 has four components:
1. **Settings Persistence (9a)** — Save browser settings to JSON, restore on startup
2. **Profile Import (9b)** — Import bookmarks and history from Chrome/Brave/Edge
3. **Clear Browsing Data (9c)** — Let users clear history, cache, cookies
4. **Multi-Profile Support (9d)** — Create/switch profiles like Chrome, with profile picker UI

Components 9a-9c are independent. Component 9d builds on 9a-9b.

**Research Document:** [sprint-9-profile-account-research.md](./sprint-9-profile-account-research.md)

### Why This Sprint Matters

Users switching browsers face friction: their years of bookmarks, history, and familiar workflows don't transfer automatically. Sprint 9 makes Hodos adoption frictionless by:
- Auto-detecting existing browser profiles (Chrome, Brave, Edge)
- One-click import of bookmarks + history
- Settings that persist across restarts (like users expect)

### Import Button Strategy

- **Production (post-MVP)**: Import wizard during first-run setup only
- **Development/Testing**: Button in Settings → Developer Tools for ongoing testing
- Feature flag `#ifdef DEBUG` or build config controls visibility

### What About YouTube Preferences, Site Personalization?

YouTube recommendations, Google preferences, etc. are **server-side** (tied to your Google account), not local files. If the user:
1. Can log into google.com (SSL + cookies working) ✓
2. Has their Google session cookies (from import OR fresh login) ✓

...then YouTube "just works" with their personalized feed. We don't need to import YouTube-specific data.

---

## Sprint 9a: Settings Persistence (Day 1)

### Goal
User changes a setting → it persists across browser restarts.

### Architecture Decision
**JSON file** (not SQLite) — human-readable, no migrations, simple.

**Location**: 
- Windows: `%APPDATA%/HodosBrowser/settings.json`
- macOS: `~/Library/Application Support/HodosBrowser/settings.json`

### Settings Schema

```json
{
  "version": 1,
  "browser": {
    "homepage": "about:blank",
    "searchEngine": "google",
    "zoomLevel": 0.0,
    "showBookmarkBar": false,
    "downloadsPath": "",
    "restoreSessionOnStart": false
  },
  "privacy": {
    "adBlockEnabled": true,
    "thirdPartyCookieBlocking": true,
    "doNotTrack": false,
    "clearDataOnExit": false
  },
  "wallet": {
    "autoApproveEnabled": true,
    "defaultPerTxLimitCents": 10,
    "defaultPerSessionLimitCents": 300,
    "defaultRateLimitPerMin": 10
  }
}
```

### Implementation Steps

#### Step 1: Create SettingsManager Singleton (C++)

**File: `cef-native/include/core/SettingsManager.h`**

```cpp
#pragma once
#include <string>
#include <mutex>

struct BrowserSettings {
    std::string homepage = "about:blank";
    std::string searchEngine = "google";
    double zoomLevel = 0.0;
    bool showBookmarkBar = false;
    std::string downloadsPath;
    bool restoreSessionOnStart = false;
};

struct PrivacySettings {
    bool adBlockEnabled = true;
    bool thirdPartyCookieBlocking = true;
    bool doNotTrack = false;
    bool clearDataOnExit = false;
};

struct WalletSettings {
    bool autoApproveEnabled = true;
    int defaultPerTxLimitCents = 10;
    int defaultPerSessionLimitCents = 300;
    int defaultRateLimitPerMin = 10;
};

class SettingsManager {
public:
    static SettingsManager& GetInstance();
    
    void Load();
    void Save();
    
    // Getters
    const BrowserSettings& GetBrowserSettings() const;
    const PrivacySettings& GetPrivacySettings() const;
    const WalletSettings& GetWalletSettings() const;
    
    // Setters (auto-save after change)
    void SetHomepage(const std::string& url);
    void SetZoomLevel(double level);
    void SetAdBlockEnabled(bool enabled);
    // ... other setters
    
private:
    SettingsManager() = default;
    std::string GetSettingsFilePath();
    
    mutable std::mutex mutex_;
    BrowserSettings browser_;
    PrivacySettings privacy_;
    WalletSettings wallet_;
    int version_ = 1;
};
```

**File: `cef-native/src/core/SettingsManager.cpp`**

Key implementation notes:
- Use `nlohmann/json` (already in vcpkg) for JSON parsing
- `Load()` called once at startup from `wWinMain` / `main`
- `Save()` called after each setter
- Thread-safe via mutex
- Cross-platform path resolution:
  ```cpp
  #ifdef _WIN32
      wchar_t path[MAX_PATH];
      SHGetFolderPathW(NULL, CSIDL_APPDATA, NULL, 0, path);
      // path + "/HodosBrowser/settings.json"
  #elif defined(__APPLE__)
      // NSSearchPathForDirectoriesInDomains wrapper
  #endif
  ```

#### Step 2: Initialize SettingsManager

**File: `cef-native/cef_browser_shell.cpp`**

In `wWinMain()` (Windows) / `main()` (macOS), before browser creation:

```cpp
// Load settings early
SettingsManager::GetInstance().Load();
```

#### Step 3: Apply Settings on Startup

In `SimpleHandler::OnAfterCreated()` or similar:

```cpp
auto& settings = SettingsManager::GetInstance().GetBrowserSettings();
if (settings.zoomLevel != 0.0) {
    browser->GetHost()->SetZoomLevel(settings.zoomLevel);
}
```

#### Step 4: IPC for Settings UI

Add IPC handlers in `simple_handler.cpp`:

| Message | Direction | Purpose |
|---------|-----------|---------|
| `settings_get_all` | React → C++ | Get current settings JSON |
| `settings_set` | React → C++ | Update a setting key/value |
| `settings_response` | C++ → React | Return settings JSON |

```cpp
// In OnProcessMessageReceived
if (message_name == "settings_get_all") {
    // Serialize settings to JSON, send back via IPC
    CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("settings_response");
    response->GetArgumentList()->SetString(0, SettingsManager::GetInstance().ToJson());
    browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
}
else if (message_name == "settings_set") {
    std::string key = args->GetString(0).ToString();
    std::string value = args->GetString(1).ToString();
    SettingsManager::GetInstance().Set(key, value);
}
```

#### Step 5: Settings UI in React

**File: `frontend/src/pages/SettingsOverlayRoot.tsx`**

Add a "General" tab with:
- Homepage input
- Search engine dropdown
- Downloads path selector
- Checkboxes for boolean settings

Use `useEffect` to load settings on mount, call `cefMessage.send("settings_set", key, value)` on change.

### Verification Checklist (9a)

- [ ] Change homepage → close browser → reopen → homepage persisted
- [ ] Change zoom level → persists
- [ ] Toggle ad blocking → persists
- [ ] `settings.json` file exists and is valid JSON
- [ ] Corrupt `settings.json` → browser uses defaults (doesn't crash)

---

## Sprint 9b: Profile Import (Day 2-3)

### Goal
Import bookmarks and history from Chrome/Brave/Edge.

### What We Import (MVP)

| Data | Source File | Method |
|------|-------------|--------|
| Bookmarks | `Bookmarks` (JSON) | Parse JSON, insert into BookmarkManager |
| History | `History` (SQLite) | Copy file, read `urls` table, insert into HistoryManager |

**NOT importing (security/complexity):**
- Cookies (requires DPAPI decrypt + re-encrypt for our format)
- Passwords (security risk, UX risk)
- Extensions (not compatible)

### Profile Detection

**Browser Profile Paths:**

| Browser | Windows Path | macOS Path |
|---------|--------------|------------|
| Chrome | `%LOCALAPPDATA%\Google\Chrome\User Data\Default\` | `~/Library/Application Support/Google/Chrome/Default/` |
| Brave | `%LOCALAPPDATA%\BraveSoftware\Brave-Browser\User Data\Default\` | `~/Library/Application Support/BraveSoftware/Brave-Browser/Default/` |
| Edge | `%LOCALAPPDATA%\Microsoft\Edge\User Data\Default\` | `~/Library/Application Support/Microsoft Edge/Default/` |

### Implementation Steps

#### Step 1: Profile Detector (C++)

**File: `cef-native/include/core/ProfileImporter.h`**

```cpp
#pragma once
#include <string>
#include <vector>

struct DetectedProfile {
    std::string browserName;  // "Chrome", "Brave", "Edge"
    std::string profilePath;
    bool hasBookmarks;
    bool hasHistory;
    int bookmarkCount;
    int historyCount;
};

class ProfileImporter {
public:
    static std::vector<DetectedProfile> DetectProfiles();
    
    static bool ImportBookmarks(const std::string& profilePath, 
                                 std::string& errorOut);
    static bool ImportHistory(const std::string& profilePath,
                               std::string& errorOut);
    
private:
    static std::string GetProfilePath(const std::string& browser);
    static int CountBookmarks(const std::string& bookmarksPath);
    static int CountHistoryEntries(const std::string& historyPath);
};
```

#### Step 2: Bookmark Import Implementation

```cpp
bool ProfileImporter::ImportBookmarks(const std::string& profilePath, 
                                       std::string& errorOut) {
    std::string bookmarksFile = profilePath + "/Bookmarks";
    
    // 1. Read JSON file
    std::ifstream file(bookmarksFile);
    if (!file.is_open()) {
        errorOut = "Could not open Bookmarks file";
        return false;
    }
    
    // 2. Parse JSON (nlohmann/json)
    nlohmann::json bookmarks;
    try {
        file >> bookmarks;
    } catch (...) {
        errorOut = "Invalid JSON in Bookmarks file";
        return false;
    }
    
    // 3. Walk the tree and insert into BookmarkManager
    auto& bar = bookmarks["roots"]["bookmark_bar"]["children"];
    for (auto& item : bar) {
        ImportBookmarkNode(item, BookmarkManager::GetInstance().GetBarFolder());
    }
    
    auto& other = bookmarks["roots"]["other"]["children"];
    for (auto& item : other) {
        ImportBookmarkNode(item, BookmarkManager::GetInstance().GetOtherFolder());
    }
    
    return true;
}

void ImportBookmarkNode(const nlohmann::json& node, BookmarkFolder* parent) {
    if (node["type"] == "url") {
        parent->AddBookmark(
            node["name"].get<std::string>(),
            node["url"].get<std::string>()
        );
    } else if (node["type"] == "folder") {
        auto* folder = parent->AddFolder(node["name"].get<std::string>());
        for (auto& child : node["children"]) {
            ImportBookmarkNode(child, folder);
        }
    }
}
```

#### Step 3: History Import Implementation

**CRITICAL**: Chrome locks its database while running. We MUST copy the file first.

```cpp
bool ProfileImporter::ImportHistory(const std::string& profilePath,
                                     std::string& errorOut) {
    std::string historyFile = profilePath + "/History";
    std::string tempCopy = GetTempPath() + "/chrome_history_import.db";
    
    // 1. Copy the file (Chrome may have it locked)
    if (!CopyFileW(ToWide(historyFile).c_str(), 
                   ToWide(tempCopy).c_str(), FALSE)) {
        errorOut = "Could not copy History file (is Chrome running?)";
        return false;
    }
    
    // 2. Open the copy with SQLite
    sqlite3* db;
    if (sqlite3_open_v2(tempCopy.c_str(), &db, 
                        SQLITE_OPEN_READONLY, nullptr) != SQLITE_OK) {
        errorOut = "Could not open History database";
        return false;
    }
    
    // 3. Query and import
    const char* sql = "SELECT url, title, last_visit_time FROM urls "
                      "ORDER BY last_visit_time DESC LIMIT 10000";
    sqlite3_stmt* stmt;
    sqlite3_prepare_v2(db, sql, -1, &stmt, nullptr);
    
    auto& historyMgr = HistoryManager::GetInstance();
    while (sqlite3_step(stmt) == SQLITE_ROW) {
        std::string url = (const char*)sqlite3_column_text(stmt, 0);
        std::string title = (const char*)sqlite3_column_text(stmt, 1);
        int64_t chromeTime = sqlite3_column_int64(stmt, 2);
        
        // Convert Chrome timestamp (microseconds since 1601-01-01)
        // to Unix timestamp (seconds since 1970-01-01)
        time_t unixTime = (chromeTime / 1000000) - 11644473600LL;
        
        historyMgr.AddEntry(url, title, unixTime);
    }
    
    sqlite3_finalize(stmt);
    sqlite3_close(db);
    
    // 4. Delete temp file
    DeleteFileW(ToWide(tempCopy).c_str());
    
    return true;
}
```

#### Step 4: IPC for Import UI

Add to `simple_handler.cpp`:

| Message | Direction | Purpose |
|---------|-----------|---------|
| `import_detect_profiles` | React → C++ | Scan for Chrome/Brave/Edge |
| `import_profiles_result` | C++ → React | List of detected profiles |
| `import_start` | React → C++ | Start import (params: browser, what) |
| `import_progress` | C++ → React | Progress update |
| `import_complete` | C++ → React | Success/failure result |

#### Step 5: Import UI in React

**File: `frontend/src/pages/SettingsOverlayRoot.tsx`**

Add "Import Data" section:

```tsx
const ImportSection = () => {
    const [profiles, setProfiles] = useState<DetectedProfile[]>([]);
    const [importing, setImporting] = useState(false);
    
    useEffect(() => {
        cefMessage.send("import_detect_profiles");
        window.addEventListener("message", handleProfilesResult);
        return () => window.removeEventListener("message", handleProfilesResult);
    }, []);
    
    const handleImport = (browser: string, what: string[]) => {
        setImporting(true);
        cefMessage.send("import_start", { browser, what });
    };
    
    return (
        <Box>
            <Typography variant="h6">Import from Another Browser</Typography>
            {profiles.map(p => (
                <Card key={p.browserName}>
                    <CardContent>
                        <Typography>{p.browserName}</Typography>
                        <Typography variant="body2">
                            {p.bookmarkCount} bookmarks, {p.historyCount} history entries
                        </Typography>
                    </CardContent>
                    <CardActions>
                        <Button onClick={() => handleImport(p.browserName, ["bookmarks"])}>
                            Import Bookmarks
                        </Button>
                        <Button onClick={() => handleImport(p.browserName, ["history"])}>
                            Import History
                        </Button>
                        <Button onClick={() => handleImport(p.browserName, ["bookmarks", "history"])}>
                            Import All
                        </Button>
                    </CardActions>
                </Card>
            ))}
            {importing && <CircularProgress />}
        </Box>
    );
};
```

### Verification Checklist (9b)

- [ ] Chrome not running → detect Chrome profile → shows bookmark/history counts
- [ ] Chrome running → detect Chrome profile → import still works (we copy the file)
- [ ] Import bookmarks from Chrome → appear in Hodos bookmark manager
- [ ] Import history from Chrome → appear in Hodos history (Ctrl+H)
- [ ] Brave profile detected and importable
- [ ] Edge profile detected and importable
- [ ] No profiles found → graceful "No browsers detected" message
- [ ] Corrupt files → graceful error, doesn't crash

---

## Cross-Platform Notes

### macOS Path Detection

```cpp
#ifdef _WIN32
    // Use SHGetFolderPathW + string append
#elif defined(__APPLE__)
    NSArray* paths = NSSearchPathForDirectoriesInDomains(
        NSApplicationSupportDirectory, NSUserDomainMask, YES);
    NSString* appSupport = [paths firstObject];
    // Chrome: appSupport + "/Google/Chrome/Default/"
#endif
```

### SQLite on macOS
- SQLite is available via system library on macOS
- Link with `-lsqlite3`
- Same API as Windows

### File Copy on macOS
```cpp
#ifdef _WIN32
    CopyFileW(src, dst, FALSE);
#elif defined(__APPLE__)
    [[NSFileManager defaultManager] copyItemAtPath:src toPath:dst error:nil];
#endif
```

---

## Files Changed Summary

| File | Changes |
|------|---------|
| **NEW** `cef-native/include/core/SettingsManager.h` | Settings singleton header |
| **NEW** `cef-native/src/core/SettingsManager.cpp` | Settings singleton impl |
| **NEW** `cef-native/include/core/ProfileImporter.h` | Profile import header |
| **NEW** `cef-native/src/core/ProfileImporter.cpp` | Profile import impl |
| `cef-native/cef_browser_shell.cpp` | Init SettingsManager |
| `cef-native/src/handlers/simple_handler.cpp` | IPC handlers for settings + import |
| `frontend/src/pages/SettingsOverlayRoot.tsx` | Settings UI + Import UI |

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Chrome DB locked | Import fails | Copy file first, read copy |
| Large history (>100K entries) | Slow import | Limit to 10K most recent |
| JSON parse failure | Crash | Try-catch with error message |
| SQLite version mismatch | Can't read Chrome DB | Unlikely; SQLite is backward-compatible |
| Cross-platform path bugs | Import fails on macOS | Test on both platforms |

---

---

## Sprint 9c: Clear Browsing Data (Day 2-3, parallel with 9b)

### Goal
Let users clear history, cache, cookies, and site data.

### What We Clear

| Data Type | Location | Method |
|-----------|----------|--------|
| **Browsing History** | `HistoryManager` (C++ in-memory + DB) | Clear entries from DB, reload manager |
| **Cache** | `%APPDATA%/HodosBrowser/Default/Cache/` | Delete folder, CEF recreates on next load |
| **Cookies** | `CefCookieManager` | `DeleteCookies(url, cookie_name, callback)` |
| **Site Permissions** | `domain_permissions` table (Rust) | `DELETE FROM domain_permissions` |
| **Site Settings** | `settings.json` (new in 9a) | Keep — these are browser settings, not site data |
| **Wallet Data** | `wallet.db` | **NEVER** — separate concern, not browsing data |

### Time Range Options

```
enum ClearTimeRange {
    LAST_HOUR,
    LAST_24_HOURS,
    LAST_7_DAYS,
    LAST_4_WEEKS,
    ALL_TIME
};
```

### Implementation Steps

#### Step 1: DataClearer Class (C++)

**File: `cef-native/include/core/DataClearer.h`**

```cpp
#pragma once
#include <string>
#include <functional>

enum class ClearTimeRange {
    LastHour,
    Last24Hours,
    Last7Days,
    Last4Weeks,
    AllTime
};

struct ClearOptions {
    bool clearHistory = false;
    bool clearCache = false;
    bool clearCookies = false;
    bool clearSitePermissions = false;
    ClearTimeRange timeRange = ClearTimeRange::AllTime;
};

class DataClearer {
public:
    static void Clear(const ClearOptions& options, 
                      std::function<void(bool success)> callback);
    
private:
    static bool ClearHistory(ClearTimeRange range);
    static bool ClearCache();
    static bool ClearCookies(ClearTimeRange range);
    static bool ClearSitePermissions();
    static int64_t GetCutoffTimestamp(ClearTimeRange range);
};
```

#### Step 2: Cache Clearing

```cpp
bool DataClearer::ClearCache() {
    // Get cache path
    std::string cachePath = GetHodosDataPath() + "/Default/Cache";
    
    // CEF must not be using it — safest approach:
    // 1. Send IPC to close all tabs/browsers
    // 2. Delete folder
    // 3. Restart CEF context
    
    // For simpler approach (less disruptive):
    // CEF internally manages cache; call CefRequestContext->ClearCache()
    // Not available in all CEF builds — check API
    
    // Fallback: delete on next startup
    std::ofstream flagFile(GetHodosDataPath() + "/clear_cache_on_start");
    flagFile << "1";
    return true;
}
```

#### Step 3: Cookie Clearing via CEF

```cpp
bool DataClearer::ClearCookies(ClearTimeRange range) {
    CefRefPtr<CefCookieManager> manager = 
        CefCookieManager::GetGlobalManager(nullptr);
    
    if (range == ClearTimeRange::AllTime) {
        // Delete all cookies
        manager->DeleteCookies("", "", nullptr);
    } else {
        // CEF DeleteCookies doesn't support time filtering
        // Need to enumerate and delete individually by creation date
        // Use CefCookieVisitor to filter
    }
    return true;
}
```

#### Step 4: IPC Messages

| Message | Direction | Purpose |
|---------|-----------|---------|
| `clear_data_start` | React → C++ | Start clearing with options JSON |
| `clear_data_progress` | C++ → React | Progress update ("Clearing history...") |
| `clear_data_complete` | C++ → React | Done, success/failure |

#### Step 5: UI in Settings

**Add "Privacy & Security" section in SettingsOverlayRoot.tsx:**

```tsx
const ClearDataSection = () => {
    const [options, setOptions] = useState<ClearOptions>({
        clearHistory: true,
        clearCache: true,
        clearCookies: true,
        clearSitePermissions: false,
        timeRange: 'all_time'
    });
    const [clearing, setClearing] = useState(false);
    
    const handleClear = () => {
        setClearing(true);
        cefMessage.send("clear_data_start", JSON.stringify(options));
    };
    
    return (
        <Box>
            <Typography variant="h6">Clear Browsing Data</Typography>
            
            <FormControl fullWidth margin="normal">
                <InputLabel>Time Range</InputLabel>
                <Select value={options.timeRange} onChange={...}>
                    <MenuItem value="last_hour">Last hour</MenuItem>
                    <MenuItem value="last_24_hours">Last 24 hours</MenuItem>
                    <MenuItem value="last_7_days">Last 7 days</MenuItem>
                    <MenuItem value="last_4_weeks">Last 4 weeks</MenuItem>
                    <MenuItem value="all_time">All time</MenuItem>
                </Select>
            </FormControl>
            
            <FormGroup>
                <FormControlLabel 
                    control={<Checkbox checked={options.clearHistory} />}
                    label="Browsing history"
                />
                <FormControlLabel 
                    control={<Checkbox checked={options.clearCache} />}
                    label="Cached images and files"
                />
                <FormControlLabel 
                    control={<Checkbox checked={options.clearCookies} />}
                    label="Cookies and site data"
                />
                <FormControlLabel 
                    control={<Checkbox checked={options.clearSitePermissions} />}
                    label="Site permissions (you'll need to re-approve sites)"
                />
            </FormGroup>
            
            <Button 
                variant="contained" 
                color="error"
                onClick={handleClear}
                disabled={clearing}
            >
                {clearing ? "Clearing..." : "Clear Data"}
            </Button>
        </Box>
    );
};
```

### Verification Checklist (9c)

- [ ] Clear history (all time) → Ctrl+H shows empty history
- [ ] Clear history (last hour) → only recent entries removed
- [ ] Clear cookies → logged out of all sites
- [ ] Clear site permissions → sites need re-approval for wallet
- [ ] Wallet data is NEVER touched
- [ ] UI shows progress feedback
- [ ] Confirmation before clearing (optional, could skip for speed)

---

## Sprint 9d: Multi-Profile Support (Day 3-4)

### Goal
Let users create multiple profiles (like Chrome), switch between them, and see which profile they're using.

### Key Concepts (from research)

**Profile** = Local data container (bookmarks, history, cookies, settings)
**Browser Account Sync** = Cloud sync (Google, Microsoft, etc.) — NOT implementing in MVP

Users can have multiple profiles without any cloud account. Website logins (x.com, YouTube) are per-profile via cookies.

### Architecture Decision: Wallet Scope

**Decision: Shared wallet across all profiles (MVP)**

Rationale:
- BSV wallet is a "system" feature, not a browsing feature
- Single backup covers everything
- Different website sessions but same wallet identity
- Can revisit for multi-wallet if users request it

### Profile Storage Structure

```
%APPDATA%/HodosBrowser/
├── Default/                 # First/default profile
│   ├── Bookmarks
│   ├── History
│   ├── Cookies
│   └── ...
├── Profile 2/               
│   └── ...
├── profiles.json            # Profile metadata
├── settings.json            # Global browser settings (9a)
└── wallet/                  # Shared wallet data
```

### profiles.json Schema

```json
{
  "version": 1,
  "lastUsedProfile": "Default",
  "showPickerOnStartup": false,
  "profiles": [
    {
      "id": "Default",
      "name": "Personal",
      "color": "#a67c00",
      "path": "Default",
      "createdAt": "2026-02-25T12:00:00Z"
    },
    {
      "id": "Profile 2",
      "name": "Work",
      "color": "#1a6b6a",
      "path": "Profile 2",
      "createdAt": "2026-02-25T14:00:00Z"
    }
  ]
}
```

### Implementation Steps

#### Step 1: ProfileManager Singleton (C++)

**File: `cef-native/include/core/ProfileManager.h`**

```cpp
#pragma once
#include <string>
#include <vector>
#include <mutex>

struct ProfileInfo {
    std::string id;
    std::string name;
    std::string color;
    std::string path;
    std::string createdAt;
};

class ProfileManager {
public:
    static ProfileManager& GetInstance();
    
    void Load();
    void Save();
    
    // Profile CRUD
    std::vector<ProfileInfo> GetAllProfiles();
    ProfileInfo GetCurrentProfile();
    bool CreateProfile(const std::string& name, const std::string& color);
    bool DeleteProfile(const std::string& id);
    bool RenameProfile(const std::string& id, const std::string& newName);
    bool SetProfileColor(const std::string& id, const std::string& color);
    
    // Switching
    void SetCurrentProfile(const std::string& id);
    std::string GetProfileDataPath(const std::string& id);
    
    // Startup behavior
    bool ShouldShowPickerOnStartup();
    void SetShowPickerOnStartup(bool show);
    
private:
    ProfileManager() = default;
    std::string GetProfilesFilePath();
    std::string GenerateProfileId();
    
    mutable std::mutex mutex_;
    std::vector<ProfileInfo> profiles_;
    std::string currentProfileId_;
    bool showPickerOnStartup_ = false;
};
```

#### Step 2: Profile-Aware Data Paths

Modify existing managers to use ProfileManager for data paths:

```cpp
// Before (hardcoded)
std::string historyPath = GetAppDataPath() + "/Default/History";

// After (profile-aware)
std::string historyPath = ProfileManager::GetInstance()
    .GetProfileDataPath(currentProfileId) + "/History";
```

**Files to modify:**
- `HistoryManager` (if exists) or wherever history is stored
- `BookmarkManager` (if exists)
- Cookie storage path in CEF initialization
- Any other per-profile data

#### Step 3: Profile Picker UI (First Launch / Startup)

**File: `frontend/src/pages/ProfilePickerOverlayRoot.tsx`**

```tsx
const ProfilePickerOverlay = () => {
    const [profiles, setProfiles] = useState<ProfileInfo[]>([]);
    
    useEffect(() => {
        cefMessage.send("profiles_get_all");
        // Listen for response
    }, []);
    
    const handleSelectProfile = (profileId: string) => {
        cefMessage.send("profiles_select", profileId);
        cefMessage.send("overlay_close");
    };
    
    const handleAddProfile = () => {
        // Show add profile form
    };
    
    return (
        <Box sx={{ p: 4, textAlign: 'center' }}>
            <Typography variant="h4" sx={{ mb: 4 }}>
                Who's using Hodos?
            </Typography>
            
            <Grid container spacing={2} justifyContent="center">
                {profiles.map(profile => (
                    <Grid item key={profile.id}>
                        <ProfileCard
                            profile={profile}
                            onClick={() => handleSelectProfile(profile.id)}
                        />
                    </Grid>
                ))}
                
                <Grid item>
                    <AddProfileCard onClick={handleAddProfile} />
                </Grid>
            </Grid>
        </Box>
    );
};

const ProfileCard = ({ profile, onClick }) => (
    <Card 
        onClick={onClick}
        sx={{ 
            width: 120, 
            cursor: 'pointer',
            border: `3px solid ${profile.color}`,
            '&:hover': { transform: 'scale(1.05)' }
        }}
    >
        <CardContent>
            <Avatar sx={{ bgcolor: profile.color, mx: 'auto', mb: 1 }}>
                {profile.name[0].toUpperCase()}
            </Avatar>
            <Typography align="center">{profile.name}</Typography>
        </CardContent>
    </Card>
);
```

#### Step 4: Profile Indicator in Header

**File: `frontend/src/components/MainBrowserView.tsx`** (or wherever header is)

Add profile avatar button to header bar:

```tsx
const ProfileButton = () => {
    const [currentProfile, setCurrentProfile] = useState<ProfileInfo | null>(null);
    const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
    
    // ... load current profile
    
    return (
        <>
            <IconButton 
                onClick={(e) => setAnchorEl(e.currentTarget)}
                sx={{ 
                    border: `2px solid ${currentProfile?.color || '#888'}`,
                    borderRadius: 2,
                    px: 1
                }}
            >
                <Avatar 
                    sx={{ 
                        width: 24, 
                        height: 24, 
                        bgcolor: currentProfile?.color,
                        fontSize: 12 
                    }}
                >
                    {currentProfile?.name?.[0] || '?'}
                </Avatar>
                <Typography sx={{ ml: 1, fontSize: 12 }}>
                    {currentProfile?.name}
                </Typography>
                <ArrowDropDown />
            </IconButton>
            
            <Menu anchorEl={anchorEl} open={Boolean(anchorEl)}>
                {profiles.map(p => (
                    <MenuItem 
                        key={p.id} 
                        onClick={() => handleSwitchProfile(p.id)}
                        selected={p.id === currentProfile?.id}
                    >
                        <Avatar sx={{ bgcolor: p.color, width: 20, height: 20, mr: 1 }}>
                            {p.name[0]}
                        </Avatar>
                        {p.name}
                        {p.id === currentProfile?.id && <Check sx={{ ml: 'auto' }} />}
                    </MenuItem>
                ))}
                <Divider />
                <MenuItem onClick={handleAddProfile}>
                    <Add sx={{ mr: 1 }} /> Add Profile
                </MenuItem>
                <MenuItem onClick={handleManageProfiles}>
                    <Settings sx={{ mr: 1 }} /> Manage Profiles
                </MenuItem>
            </Menu>
        </>
    );
};
```

#### Step 5: Profile Switching Logic

**Key behavior:** Switching profiles opens a NEW WINDOW in that profile.

```cpp
// In simple_handler.cpp IPC handler
if (message_name == "profiles_switch") {
    std::string profileId = args->GetString(0).ToString();
    
    // Save current profile state
    ProfileManager::GetInstance().SetCurrentProfile(profileId);
    
    // Launch new browser instance with profile
    LaunchNewInstanceWithProfile(profileId);
    
    // Note: current window stays open in old profile
    // (like Chrome behavior)
}

void LaunchNewInstanceWithProfile(const std::string& profileId) {
    // Get our exe path
    wchar_t exePath[MAX_PATH];
    GetModuleFileNameW(NULL, exePath, MAX_PATH);
    
    // Launch with profile argument
    std::wstring cmdLine = exePath;
    cmdLine += L" --profile-id=" + ToWide(profileId);
    
    // CreateProcess...
}
```

#### Step 6: Profile Management in Settings

**Add "Profiles" section in SettingsOverlayRoot.tsx:**

```tsx
const ProfilesSection = () => {
    const [profiles, setProfiles] = useState<ProfileInfo[]>([]);
    
    return (
        <Box>
            <Typography variant="h6">Profiles</Typography>
            
            <List>
                {profiles.map(p => (
                    <ListItem key={p.id}>
                        <Avatar sx={{ bgcolor: p.color, mr: 2 }}>
                            {p.name[0]}
                        </Avatar>
                        <ListItemText primary={p.name} />
                        <IconButton onClick={() => handleEdit(p.id)}>
                            <Edit />
                        </IconButton>
                        <IconButton 
                            onClick={() => handleDelete(p.id)}
                            disabled={profiles.length === 1}
                        >
                            <Delete />
                        </IconButton>
                    </ListItem>
                ))}
            </List>
            
            <Button 
                startIcon={<Add />} 
                onClick={handleAddProfile}
            >
                Add Profile
            </Button>
            
            <FormControlLabel
                control={<Switch checked={showPickerOnStartup} />}
                label="Show profile picker on startup"
                onChange={handleTogglePicker}
            />
        </Box>
    );
};
```

#### Step 7: IPC Messages

| Message | Direction | Purpose |
|---------|-----------|---------|
| `profiles_get_all` | React → C++ | Get all profiles |
| `profiles_list` | C++ → React | Return profiles array |
| `profiles_get_current` | React → C++ | Get current profile |
| `profiles_current` | C++ → React | Return current profile |
| `profiles_create` | React → C++ | Create new profile (name, color) |
| `profiles_delete` | React → C++ | Delete profile by ID |
| `profiles_rename` | React → C++ | Rename profile |
| `profiles_set_color` | React → C++ | Change profile color |
| `profiles_switch` | React → C++ | Switch to profile (opens new window) |
| `profiles_set_picker_startup` | React → C++ | Toggle startup picker |

### Verification Checklist (9d)

- [ ] First launch with no profiles → creates "Default" profile
- [ ] Create second profile → appears in picker and header dropdown
- [ ] Switch profile → opens new window in that profile
- [ ] New window → uses profile's bookmarks/history (not other profile's)
- [ ] Delete profile (non-last) → profile removed, data deleted
- [ ] Cannot delete last profile
- [ ] Rename profile → name updates in all UI locations
- [ ] Change profile color → color updates in header and picker
- [ ] "Show picker on startup" → persists and works
- [ ] Profile data isolation: visit site in Profile A, bookmark it → not visible in Profile B

### MVP Scope Boundaries

**In scope (9d MVP):**
- Create/delete/rename/recolor profiles
- Profile picker on startup (optional)
- Profile indicator in header with dropdown
- Switch profile = new window
- Per-profile: bookmarks, history, cookies

**Out of scope (post-MVP):**
- Browser account sync (Google/Microsoft account)
- Guest mode / incognito profile
- Container tabs within profiles
- Profile import from other Hodos installations
- Auto-switch profile for certain URLs

---

## Post-Sprint Tasks

After Sprint 9 is complete:
1. Update `development-docs/browser-core/CLAUDE.md` with Sprint 9 completion
2. Update `00-SPRINT-INDEX.md` status
3. Test against [test-site-basket.md](./test-site-basket.md) — Standard basket (15 min)
4. Consider Sprint 10 (cookie blocking + fingerprinting) or macOS port

---

## Pre-Sprint Validation: X.com Login Test

**Before starting Sprint 9 implementation, verify Sprint 1 SSL fixes work:**

1. Start all three services (wallet, frontend, CEF)
2. Navigate to x.com
3. Click "Sign in"
4. **Expected:** Login form loads without SSL errors
5. **If fail:** Debug SSL handler before proceeding

This validates the auth/login pipeline that imported profiles will need.

---

*This document was generated by Edwin based on implementation-plan.md, 01-chrome-brave-research.md, and architectural analysis of the existing codebase. Updated 2026-02-25 with clear data (9c) and import UX strategy.*
