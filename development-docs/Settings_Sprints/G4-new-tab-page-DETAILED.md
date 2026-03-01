# G4: New Tab Page — Detailed Implementation Plan

**Status**: Not Started  
**Complexity**: Medium-High  
**Estimated Time**: 6-10 hours (4 phases)  
**Dependencies**: G1 (search engine for search bar), HistoryManager (for most-visited)

---

## Executive Summary

Create a branded new tab page with search bar and most-visited site tiles. Currently, new tabs open an external URL (`metanetapps.com`) — this sprint replaces that with a custom Hodos page.

---

## Current State Analysis

### What Exists
- **New tab**: Opens `metanetapps.com` (hardcoded in `TabManager::CreateTab()`)
- **Homepage**: Configurable, defaults to `coingeek.com`
- **HistoryManager**: Tracks visits with `visit_count` — can query most-visited
- **No new tab page component** in frontend

### What's Missing
- No `NewTabPage.tsx` component
- No `/newtab` route
- No IPC for fetching most-visited sites
- No `hodos://newtab` URL mapping
- TabManager hardcodes external URL

---

## Phase 1: Basic New Tab Page (3-4 hours)

### Step 1: Create NewTabPage Component

**File**: `frontend/src/pages/NewTabPage.tsx`

```tsx
import React, { useState, useEffect, useRef } from 'react';
import { useSettings } from '../hooks/useSettings';
import { getSearchUrl, SearchEngine } from '../utils/searchEngines';
import './NewTabPage.css';

interface MostVisitedSite {
  url: string;
  title: string;
  favicon?: string;
  visitCount: number;
}

const NewTabPage: React.FC = () => {
  const { settings } = useSettings();
  const [searchQuery, setSearchQuery] = useState('');
  const [mostVisited, setMostVisited] = useState<MostVisitedSite[]>([]);
  const [blockedCount, setBlockedCount] = useState(0);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Auto-focus search bar on mount
  useEffect(() => {
    searchInputRef.current?.focus();
  }, []);

  // Fetch most-visited sites
  useEffect(() => {
    window.cefMessage?.send('get_most_visited', [8]); // Request top 8

    const handleMessage = (event: MessageEvent) => {
      if (event.data.type === 'most_visited_response') {
        setMostVisited(event.data.sites || []);
      }
      if (event.data.type === 'blocked_count_today') {
        setBlockedCount(event.data.count || 0);
      }
    };

    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, []);

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    if (!searchQuery.trim()) return;

    const query = searchQuery.trim();
    const searchEngine = (settings.browser.searchEngine || 'google') as SearchEngine;

    // Check if it's a URL
    const isUrl = /^(https?:\/\/|www\.)/.test(query) || 
                  /^[a-z0-9]+([\-\.]{1}[a-z0-9]+)*\.[a-z]{2,}$/i.test(query);

    if (isUrl) {
      const url = query.startsWith('http') ? query : `https://${query}`;
      window.cefMessage?.send('navigate', [url]);
    } else {
      const searchUrl = getSearchUrl(searchEngine, query);
      window.cefMessage?.send('navigate', [searchUrl]);
    }
  };

  const handleSiteClick = (url: string) => {
    window.cefMessage?.send('navigate', [url]);
  };

  // Get favicon URL (using Google's service for Phase 1)
  const getFaviconUrl = (url: string): string => {
    try {
      const domain = new URL(url).hostname;
      return `https://www.google.com/s2/favicons?domain=${domain}&sz=64`;
    } catch {
      return ''; // Fallback handled in CSS
    }
  };

  return (
    <div className="newtab-container">
      {/* Logo */}
      <div className="newtab-logo">
        <img src="/assets/hodos-logo.png" alt="Hodos" />
      </div>

      {/* Search Bar */}
      <form className="newtab-search" onSubmit={handleSearch}>
        <span className="search-icon">🔍</span>
        <input
          ref={searchInputRef}
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Search or enter URL"
          className="search-input"
        />
      </form>

      {/* Most Visited Grid */}
      <div className="newtab-tiles">
        {mostVisited.map((site, index) => (
          <div
            key={site.url}
            className="newtab-tile"
            onClick={() => handleSiteClick(site.url)}
            role="button"
            tabIndex={0}
          >
            <div className="tile-icon">
              <img 
                src={getFaviconUrl(site.url)} 
                alt="" 
                onError={(e) => {
                  (e.target as HTMLImageElement).style.display = 'none';
                }}
              />
              <span className="tile-icon-fallback">🌐</span>
            </div>
            <span className="tile-title">{site.title || site.url}</span>
          </div>
        ))}
      </div>

      {/* Privacy Stats */}
      <div className="newtab-stats">
        🛡️ {blockedCount.toLocaleString()} trackers blocked today
      </div>
    </div>
  );
};

export default NewTabPage;
```

### Step 2: Create CSS

**File**: `frontend/src/pages/NewTabPage.css`

```css
.newtab-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  background-color: #121212;
  color: #e0e0e0;
  padding: 40px 20px;
  box-sizing: border-box;
}

.newtab-logo {
  margin-bottom: 40px;
}

.newtab-logo img {
  height: 80px;
  width: auto;
}

.newtab-search {
  display: flex;
  align-items: center;
  width: 100%;
  max-width: 600px;
  background-color: #2a2a2a;
  border: 1px solid #444;
  border-radius: 24px;
  padding: 12px 20px;
  margin-bottom: 40px;
  transition: border-color 0.2s, box-shadow 0.2s;
}

.newtab-search:focus-within {
  border-color: #a67c00;
  box-shadow: 0 0 0 2px rgba(166, 124, 0, 0.2);
}

.search-icon {
  font-size: 18px;
  margin-right: 12px;
  opacity: 0.6;
}

.search-input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: #e0e0e0;
  font-size: 16px;
}

.search-input::placeholder {
  color: #888;
}

.newtab-tiles {
  display: grid;
  grid-template-columns: repeat(4, 100px);
  gap: 24px;
  margin-bottom: 40px;
}

.newtab-tile {
  display: flex;
  flex-direction: column;
  align-items: center;
  cursor: pointer;
  padding: 12px;
  border-radius: 8px;
  transition: background-color 0.2s;
}

.newtab-tile:hover {
  background-color: rgba(255, 255, 255, 0.05);
}

.tile-icon {
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: #333;
  border-radius: 50%;
  margin-bottom: 8px;
  position: relative;
}

.tile-icon img {
  width: 32px;
  height: 32px;
  border-radius: 4px;
}

.tile-icon-fallback {
  position: absolute;
  font-size: 24px;
}

.tile-icon img:not([style*="display: none"]) + .tile-icon-fallback {
  display: none;
}

.tile-title {
  font-size: 12px;
  text-align: center;
  max-width: 90px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #aaa;
}

.newtab-stats {
  color: #666;
  font-size: 14px;
}
```

### Step 3: Add Route

**File**: `frontend/src/App.tsx`

```tsx
import NewTabPage from './pages/NewTabPage';

// In routes:
<Route path="/newtab" element={<NewTabPage />} />
```

### Step 4: Update TabManager Default URL

**File**: `src/core/TabManager.cpp`

```cpp
int TabManager::CreateTab(const std::string& url, HWND parent_hwnd, int x, int y, int width, int height) {
    CEF_REQUIRE_UI_THREAD();

    int tab_id = GetNextTabId();
    // CHANGED: Use local new tab page instead of external URL
    std::string tab_url = url.empty() ? "http://127.0.0.1:5137/newtab" : url;
    // ...
}
```

### Step 5: Add IPC Handler for Most-Visited

**File**: `simple_handler.cpp` — in `OnProcessMessageReceived()`

```cpp
} else if (message_name == "get_most_visited") {
    int limit = 8;
    if (args->GetSize() > 0 && args->GetType(0) == VTYPE_INT) {
        limit = args->GetInt(0);
    }
    
    // Query HistoryManager for most visited
    auto& history = HistoryManager::GetInstance();
    // Note: May need to add this method to HistoryManager
    auto sites = history.GetMostVisited(limit);
    
    // Build response
    CefRefPtr<CefProcessMessage> response = 
        CefProcessMessage::Create("most_visited_response");
    CefRefPtr<CefListValue> respArgs = response->GetArgumentList();
    
    CefRefPtr<CefListValue> sitesList = CefListValue::Create();
    for (size_t i = 0; i < sites.size(); i++) {
        CefRefPtr<CefDictionaryValue> site = CefDictionaryValue::Create();
        site->SetString("url", sites[i].url);
        site->SetString("title", sites[i].title);
        site->SetInt("visitCount", sites[i].visit_count);
        sitesList->SetDictionary(static_cast<int>(i), site);
    }
    respArgs->SetList(0, sitesList);
    
    browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
```

### Step 6: Add HistoryManager::GetMostVisited()

**File**: `include/core/HistoryManager.h`

```cpp
// Add this method declaration:
std::vector<HistoryEntry> GetMostVisited(int limit);
```

**File**: `src/core/HistoryManager.cpp`

```cpp
std::vector<HistoryEntry> HistoryManager::GetMostVisited(int limit) {
    std::vector<HistoryEntry> results;
    if (!history_db_) return results;
    
    const char* sql = R"(
        SELECT url, title, visit_count, last_visit_time
        FROM urls
        WHERE url NOT LIKE 'http://127.0.0.1%'
          AND url NOT LIKE 'hodos://%'
          AND url NOT LIKE 'about:%'
        GROUP BY url
        ORDER BY visit_count DESC
        LIMIT ?
    )";
    
    sqlite3_stmt* stmt;
    if (sqlite3_prepare_v2(history_db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
        sqlite3_bind_int(stmt, 1, limit);
        
        while (sqlite3_step(stmt) == SQLITE_ROW) {
            HistoryEntry entry;
            entry.url = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));
            entry.title = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 1));
            entry.visit_count = sqlite3_column_int(stmt, 2);
            entry.last_visit_time = sqlite3_column_int64(stmt, 3);
            results.push_back(entry);
        }
        sqlite3_finalize(stmt);
    }
    
    return results;
}
```

### Step 7: URL Display Mapping

**File**: Where URL display is handled (likely `simple_handler.cpp` or frontend)

Map internal URLs to display URLs:
- `http://127.0.0.1:5137/newtab` → `hodos://newtab`
- `http://127.0.0.1:5137/settings` → `hodos://settings`

---

## Phase 2: Homepage vs New Tab Separation (1-2 hours)

Add setting for new tab page behavior:

```typescript
// Options
type NewTabPageOption = 'default' | 'blank' | 'homepage' | string;

// Settings
{
  "browser": {
    "newTabPage": "default"  // 'default' | 'blank' | 'homepage' | custom URL
  }
}
```

### Frontend Setting

```tsx
<SettingsCard title="New Tab">
  <SettingRow
    label="New tab page"
    description="What opens when you create a new tab"
    control={
      <Select value={settings.browser.newTabPage || 'default'} ...>
        <MenuItem value="default">Hodos New Tab</MenuItem>
        <MenuItem value="blank">Blank Page</MenuItem>
        <MenuItem value="homepage">Homepage</MenuItem>
        <MenuItem value="custom">Custom URL...</MenuItem>
      </Select>
    }
  />
</SettingsCard>
```

### TabManager Logic

```cpp
std::string GetNewTabUrl() {
    auto& settings = SettingsManager::GetInstance().GetBrowserSettings();
    std::string option = settings.newTabPage;
    
    if (option.empty() || option == "default") {
        return "http://127.0.0.1:5137/newtab";
    } else if (option == "blank") {
        return "about:blank";
    } else if (option == "homepage") {
        return settings.homepage;
    } else {
        return option; // Custom URL
    }
}
```

---

## Phase 3: Right-Click "Set as Homepage" (1 hour)

Add context menu option to set current page as homepage.

### Add Menu Item

**File**: `simple_handler.cpp` — in context menu building

```cpp
// Add menu item ID
#define MENU_ID_SET_AS_HOMEPAGE 28001

// In OnBeforeContextMenu:
model->AddItem(MENU_ID_SET_AS_HOMEPAGE, "Set as Homepage");
```

### Handle Menu Click

```cpp
bool SimpleHandler::OnContextMenuCommand(...) {
    if (command_id == MENU_ID_SET_AS_HOMEPAGE) {
        std::string currentUrl = browser->GetMainFrame()->GetURL().ToString();
        SettingsManager::GetInstance().SetHomepage(currentUrl);
        
        // Show confirmation (send IPC to show toast)
        // ...
        
        return true;
    }
    // ...
}
```

---

## Phase 4: Polish (Future)

- Remove individual tiles (X button)
- Add custom shortcut tiles
- Background image selection
- Toggle privacy stats on/off

---

## Research Notes

### Favicon Approach

| Approach | Pros | Cons |
|----------|------|------|
| Google S2 API | Easy, reliable | Privacy concern (leaks URLs to Google) |
| DuckDuckGo favicon | Privacy-focused | Same concept as Google |
| Local caching | Best privacy | Complex implementation |

**Phase 1**: Use Google S2 for simplicity.  
**Future**: Implement local favicon cache.

### Privacy Stats Data Source

Get blocked count from `AdblockCache`:
```cpp
int totalBlocked = 0;
// Sum across all browsers or use a global counter
```

Consider adding a daily counter that resets at midnight.

---

## Test Checklist

### Phase 1
- [ ] Open new tab → Hodos new tab page appears
- [ ] Logo, search bar, and tiles render correctly
- [ ] Search bar auto-focuses
- [ ] Type query → searches with default search engine
- [ ] Type URL → navigates directly
- [ ] Click tile → navigates to site
- [ ] Most-visited tiles update based on browsing
- [ ] Address bar shows `hodos://newtab`
- [ ] Tab title shows "New Tab"

### Phase 2
- [ ] Set new tab to "Blank" → new tabs are empty
- [ ] Set new tab to "Homepage" → new tabs open homepage
- [ ] Set custom URL → new tabs open that URL

### Phase 3
- [ ] Right-click on page → "Set as Homepage" option appears
- [ ] Click it → homepage setting updates
- [ ] Confirmation shown (toast)

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `frontend/src/pages/NewTabPage.tsx` | **CREATE** | New tab page component |
| `frontend/src/pages/NewTabPage.css` | **CREATE** | Styling |
| `frontend/src/App.tsx` | MODIFY | Add /newtab route |
| `src/core/TabManager.cpp` | MODIFY | Change default URL |
| `src/core/HistoryManager.cpp` | MODIFY | Add GetMostVisited() |
| `src/handlers/simple_handler.cpp` | MODIFY | IPC handlers + context menu |
| `include/core/SettingsManager.h` | MODIFY | Add newTabPage setting |

---

**Last Updated**: 2026-02-28
