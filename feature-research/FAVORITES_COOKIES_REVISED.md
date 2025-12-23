# Favorites and Cookies Features Implementation (Revised)

## Overview

This document covers implementation for Favorites (Speed Dial) and Cookies Management features. Both are handled entirely in the CEF C++ layer, keeping the Rust wallet backend exclusively for BRC-100 wallet operations.

---

# Part 1: Favorites (Speed Dial)

## Architecture

**Favorites are an extension of the Bookmarks system** with special visual presentation and behavior.

### Implementation Approach

- ✅ Extend BookmarkManager with favorites functionality
- ✅ Add `is_favorite` flag to bookmark records
- ✅ Store thumbnails/screenshots in bookmark database
- ✅ All logic in CEF C++ layer
- ❌ No Rust backend involvement

## Database Extension

### Extend Bookmarks Table

Add favorites-specific columns to the existing bookmarks table:

```sql
-- Add to existing Bookmarks database
ALTER TABLE bookmarks ADD COLUMN is_favorite INTEGER DEFAULT 0;
ALTER TABLE bookmarks ADD COLUMN thumbnail TEXT;  -- Base64 encoded image
ALTER TABLE bookmarks ADD COLUMN thumbnail_updated INTEGER;
ALTER TABLE bookmarks ADD COLUMN favorite_order INTEGER;

CREATE INDEX IF NOT EXISTS idx_bookmarks_is_favorite ON bookmarks(is_favorite);
CREATE INDEX IF NOT EXISTS idx_bookmarks_favorite_order ON bookmarks(favorite_order);
```

## CEF C++ Implementation

**File**: `cef-native/include/core/BookmarkManager.h` (extend existing)

```cpp
// Add to existing BookmarkManager class

class BookmarkManager {
public:
    // ... existing bookmark methods ...

    // Favorites operations
    std::vector<Bookmark> GetFavorites();
    bool AddToFavorites(const std::string& bookmark_id);
    bool RemoveFromFavorites(const std::string& bookmark_id);
    bool UpdateThumbnail(const std::string& bookmark_id, const std::string& thumbnail);
    bool ReorderFavorites(const std::vector<std::string>& bookmark_ids);

    // Smart suggestions based on visit patterns
    std::vector<Bookmark> GetFavoriteSuggestions(int limit);
    int AutoPopulateFavorites(int count);

private:
    // ... existing members ...
};
```

**File**: `cef-native/src/core/BookmarkManager.cpp` (add methods)

```cpp
std::vector<Bookmark> BookmarkManager::GetFavorites() {
    std::vector<Bookmark> favorites;

    if (!db_) return favorites;

    const char* sql = R"(
        SELECT id FROM bookmarks
        WHERE is_favorite = 1 AND is_folder = 0
        ORDER BY favorite_order ASC
    )";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) return favorites;

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        std::string id = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));
        favorites.push_back(GetBookmark(id));
    }

    sqlite3_finalize(stmt);
    return favorites;
}

bool BookmarkManager::AddToFavorites(const std::string& bookmark_id) {
    if (!db_) return false;

    // Get next favorite order
    const char* order_sql = "SELECT COALESCE(MAX(favorite_order), -1) + 1 FROM bookmarks WHERE is_favorite = 1";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(db_, order_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return false;

    int order = 0;
    if (sqlite3_step(stmt) == SQLITE_ROW) {
        order = sqlite3_column_int(stmt, 0);
    }
    sqlite3_finalize(stmt);

    // Update bookmark
    const char* update_sql = "UPDATE bookmarks SET is_favorite = 1, favorite_order = ? WHERE id = ?";
    rc = sqlite3_prepare_v2(db_, update_sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) return false;

    sqlite3_bind_int(stmt, 1, order);
    sqlite3_bind_text(stmt, 2, bookmark_id.c_str(), -1, SQLITE_STATIC);

    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    return rc == SQLITE_DONE;
}

bool BookmarkManager::RemoveFromFavorites(const std::string& bookmark_id) {
    if (!db_) return false;

    const char* sql = "UPDATE bookmarks SET is_favorite = 0, favorite_order = NULL WHERE id = ?";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return false;

    sqlite3_bind_text(stmt, 1, bookmark_id.c_str(), -1, SQLITE_STATIC);
    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    return rc == SQLITE_DONE;
}

bool BookmarkManager::UpdateThumbnail(const std::string& bookmark_id, const std::string& thumbnail) {
    if (!db_) return false;

    auto now = std::chrono::system_clock::now().time_since_epoch().count();

    const char* sql = "UPDATE bookmarks SET thumbnail = ?, thumbnail_updated = ? WHERE id = ?";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return false;

    sqlite3_bind_text(stmt, 1, thumbnail.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_int64(stmt, 2, now);
    sqlite3_bind_text(stmt, 3, bookmark_id.c_str(), -1, SQLITE_STATIC);

    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    return rc == SQLITE_DONE;
}

std::vector<Bookmark> BookmarkManager::GetFavoriteSuggestions(int limit) {
    std::vector<Bookmark> suggestions;

    if (!db_) return suggestions;

    const char* sql = R"(
        SELECT id FROM bookmarks
        WHERE is_favorite = 0 AND is_folder = 0 AND url IS NOT NULL
        ORDER BY visit_count DESC, last_visited DESC
        LIMIT ?
    )";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) return suggestions;

    sqlite3_bind_int(stmt, 1, limit);

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        std::string id = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));
        suggestions.push_back(GetBookmark(id));
    }

    sqlite3_finalize(stmt);
    return suggestions;
}

int BookmarkManager::AutoPopulateFavorites(int count) {
    auto suggestions = GetFavoriteSuggestions(count);
    int added = 0;

    for (const auto& bookmark : suggestions) {
        if (AddToFavorites(bookmark.id)) {
            added++;
        }
    }

    return added;
}
```

## V8 JavaScript Bindings

**File**: `cef-native/src/handlers/simple_render_process_handler.cpp` (extend)

```cpp
void SimpleRenderProcessHandler::OnContextCreated(...) {
    // ... existing code ...

    // Add favorites to bookmarks namespace
    CefRefPtr<CefV8Value> bookmarks_obj = ...; // from previous implementation

    bookmarks_obj->SetValue("getFavorites",
        CefV8Value::CreateFunction("getFavorites", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    bookmarks_obj->SetValue("addToFavorites",
        CefV8Value::CreateFunction("addToFavorites", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    bookmarks_obj->SetValue("removeFromFavorites",
        CefV8Value::CreateFunction("removeFromFavorites", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    bookmarks_obj->SetValue("updateThumbnail",
        CefV8Value::CreateFunction("updateThumbnail", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    bookmarks_obj->SetValue("getSuggestions",
        CefV8Value::CreateFunction("getSuggestions", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);
}
```

## Frontend Implementation

**File**: `frontend/src/hooks/useFavorites.ts`

```typescript
import { useState, useEffect, useCallback } from 'react';

export function useFavorites() {
  const [favorites, setFavorites] = useState<Bookmark[]>([]);
  const [loading, setLoading] = useState(false);

  const loadFavorites = useCallback(() => {
    setLoading(true);
    try {
      const data = window.hodosBrowser.bookmarks.getFavorites();
      setFavorites(data);
    } catch (err) {
      console.error('Failed to load favorites:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadFavorites();
  }, [loadFavorites]);

  const addToFavorites = useCallback((bookmarkId: string) => {
    window.hodosBrowser.bookmarks.addToFavorites(bookmarkId);
    loadFavorites();
  }, [loadFavorites]);

  const removeFromFavorites = useCallback((bookmarkId: string) => {
    window.hodosBrowser.bookmarks.removeFromFavorites(bookmarkId);
    setFavorites(prev => prev.filter(f => f.id !== bookmarkId));
  }, []);

  return {
    favorites,
    loading,
    loadFavorites,
    addToFavorites,
    removeFromFavorites
  };
}
```

**File**: `frontend/src/components/FavoritesGrid.tsx`

```typescript
import React from 'react';
import { useFavorites } from '../hooks/useFavorites';
import { Grid, Card, CardMedia, CardContent, Typography, IconButton, Box } from '@mui/material';
import { Close } from '@mui/icons-material';

export function FavoritesGrid() {
  const { favorites, removeFromFavorites } = useFavorites();

  return (
    <Box sx={{ p: 3 }}>
      <Typography variant="h4" sx={{ mb: 3 }}>Favorites</Typography>

      <Grid container spacing={2}>
        {favorites.map(fav => (
          <Grid item xs={12} sm={6} md={4} lg={3} key={fav.id}>
            <Card sx={{ position: 'relative', cursor: 'pointer' }}>
              <IconButton
                sx={{ position: 'absolute', top: 4, right: 4, bgcolor: 'background.paper' }}
                size="small"
                onClick={() => removeFromFavorites(fav.id)}
              >
                <Close fontSize="small" />
              </IconButton>

              {fav.thumbnail ? (
                <CardMedia component="img" height="140" image={fav.thumbnail} alt={fav.title} />
              ) : (
                <Box sx={{ height: 140, display: 'flex', alignItems: 'center', justifyContent: 'center', bgcolor: 'grey.200' }}>
                  <Typography variant="h3">{fav.title.charAt(0).toUpperCase()}</Typography>
                </Box>
              )}

              <CardContent onClick={() => window.hodosBrowser.navigation.navigate(fav.url)}>
                <Typography variant="body1" noWrap>{fav.title}</Typography>
                <Typography variant="caption" color="text.secondary" noWrap>{fav.url}</Typography>
              </CardContent>
            </Card>
          </Grid>
        ))}
      </Grid>
    </Box>
  );
}
```

---

# Part 2: Cookies Management

## Architecture

CEF automatically manages cookies through its CefCookieManager API. We need to expose this to the frontend for user control.

### Implementation Approach

- ✅ Use CEF's built-in CefCookieManager
- ✅ Wrap CEF cookie APIs in C++ layer
- ✅ Expose to JavaScript via V8 bindings
- ✅ Optional: Store user preferences in separate database
- ❌ No Rust backend involvement

## CEF C++ Cookie Manager

**File**: `cef-native/include/core/CookieManager.h`

```cpp
#ifndef COOKIE_MANAGER_H
#define COOKIE_MANAGER_H

#include "include/cef_cookie.h"
#include <sqlite3.h>
#include <string>
#include <vector>
#include <functional>

struct CookieInfo {
    std::string name;
    std::string value;
    std::string domain;
    std::string path;
    int64_t creation;
    int64_t expires;
    int64_t last_access;
    bool secure;
    bool http_only;
    int same_site;
};

struct CookiePreference {
    std::string domain;
    bool block_all;
    bool block_third_party;
    bool allow_session_only;
};

// Visitor for enumerating cookies
class CookieVisitor : public CefCookieVisitor {
public:
    using Callback = std::function<void(const std::vector<CookieInfo>&)>;

    explicit CookieVisitor(Callback callback);

    bool Visit(const CefCookie& cookie, int count, int total, bool& deleteCookie) override;

private:
    std::vector<CookieInfo> cookies_;
    Callback callback_;

    IMPLEMENT_REFCOUNTING(CookieVisitor);
};

class HodosCookieManager {
public:
    static HodosCookieManager& GetInstance();

    bool Initialize(const std::string& user_data_path);

    // Get cookies (async operations)
    void GetAllCookies(CookieVisitor::Callback callback);
    void GetCookiesForUrl(const std::string& url, CookieVisitor::Callback callback);

    // Delete operations (sync)
    bool DeleteCookie(const std::string& url, const std::string& name);
    bool DeleteAllCookies();
    bool DeleteCookiesForDomain(const std::string& domain);

    // Preferences (stored in separate database)
    bool SetPreference(const CookiePreference& pref);
    CookiePreference GetPreference(const std::string& domain);
    std::vector<CookiePreference> GetAllPreferences();
    bool DeletePreference(const std::string& domain);

private:
    HodosCookieManager() = default;
    ~HodosCookieManager();

    CefRefPtr<CefCookieManager> GetCefCookieManager();

    sqlite3* preferences_db_;
    std::string preferences_db_path_;

    bool OpenPreferencesDatabase();
    void ClosePreferencesDatabase();
};

#endif // COOKIE_MANAGER_H
```

**File**: `cef-native/src/core/CookieManager.cpp`

```cpp
#include "include/core/CookieManager.h"

CookieVisitor::CookieVisitor(Callback callback)
    : callback_(callback) {}

bool CookieVisitor::Visit(const CefCookie& cookie, int count, int total, bool& deleteCookie) {
    CookieInfo info;
    info.name = CefString(&cookie.name).ToString();
    info.value = CefString(&cookie.value).ToString();
    info.domain = CefString(&cookie.domain).ToString();
    info.path = CefString(&cookie.path).ToString();
    info.creation = cookie.creation.val;
    info.expires = cookie.expires.val;
    info.last_access = cookie.last_access.val;
    info.secure = cookie.secure != 0;
    info.http_only = cookie.httponly != 0;
    info.same_site = static_cast<int>(cookie.same_site);

    cookies_.push_back(info);

    if (count == total - 1) {
        callback_(cookies_);
    }

    deleteCookie = false;
    return true;
}

HodosCookieManager& HodosCookieManager::GetInstance() {
    static HodosCookieManager instance;
    return instance;
}

HodosCookieManager::~HodosCookieManager() {
    ClosePreferencesDatabase();
}

bool HodosCookieManager::Initialize(const std::string& user_data_path) {
    preferences_db_path_ = user_data_path + "/CookiePreferences";
    return OpenPreferencesDatabase();
}

bool HodosCookieManager::OpenPreferencesDatabase() {
    int rc = sqlite3_open(preferences_db_path_.c_str(), &preferences_db_);
    if (rc != SQLITE_OK) return false;

    const char* sql = R"(
        CREATE TABLE IF NOT EXISTS cookie_preferences (
            domain TEXT PRIMARY KEY,
            block_all INTEGER DEFAULT 0,
            block_third_party INTEGER DEFAULT 0,
            allow_session_only INTEGER DEFAULT 0
        );
    )";

    char* err_msg = nullptr;
    rc = sqlite3_exec(preferences_db_, sql, nullptr, nullptr, &err_msg);

    if (rc != SQLITE_OK) {
        sqlite3_free(err_msg);
        ClosePreferencesDatabase();
        return false;
    }

    return true;
}

void HodosCookieManager::ClosePreferencesDatabase() {
    if (preferences_db_) {
        sqlite3_close(preferences_db_);
        preferences_db_ = nullptr;
    }
}

CefRefPtr<CefCookieManager> HodosCookieManager::GetCefCookieManager() {
    return CefCookieManager::GetGlobalManager(nullptr);
}

void HodosCookieManager::GetAllCookies(CookieVisitor::Callback callback) {
    auto manager = GetCefCookieManager();
    if (!manager) {
        callback({});
        return;
    }

    CefRefPtr<CookieVisitor> visitor = new CookieVisitor(callback);
    manager->VisitAllCookies(visitor);
}

void HodosCookieManager::GetCookiesForUrl(const std::string& url, CookieVisitor::Callback callback) {
    auto manager = GetCefCookieManager();
    if (!manager) {
        callback({});
        return;
    }

    CefRefPtr<CookieVisitor> visitor = new CookieVisitor(callback);
    manager->VisitUrlCookies(CefString(url), true, visitor);
}

bool HodosCookieManager::DeleteCookie(const std::string& url, const std::string& name) {
    auto manager = GetCefCookieManager();
    if (!manager) return false;

    return manager->DeleteCookies(CefString(url), CefString(name), nullptr);
}

bool HodosCookieManager::DeleteAllCookies() {
    auto manager = GetCefCookieManager();
    if (!manager) return false;

    return manager->DeleteCookies(CefString(), CefString(), nullptr);
}

bool HodosCookieManager::DeleteCookiesForDomain(const std::string& domain) {
    auto manager = GetCefCookieManager();
    if (!manager) return false;

    std::string url = "http://" + domain;
    return manager->DeleteCookies(CefString(url), CefString(), nullptr);
}

bool HodosCookieManager::SetPreference(const CookiePreference& pref) {
    if (!preferences_db_) return false;

    const char* sql = R"(
        INSERT OR REPLACE INTO cookie_preferences (domain, block_all, block_third_party, allow_session_only)
        VALUES (?, ?, ?, ?)
    )";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(preferences_db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return false;

    sqlite3_bind_text(stmt, 1, pref.domain.c_str(), -1, SQLITE_STATIC);
    sqlite3_bind_int(stmt, 2, pref.block_all ? 1 : 0);
    sqlite3_bind_int(stmt, 3, pref.block_third_party ? 1 : 0);
    sqlite3_bind_int(stmt, 4, pref.allow_session_only ? 1 : 0);

    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    return rc == SQLITE_DONE;
}

std::vector<CookiePreference> HodosCookieManager::GetAllPreferences() {
    std::vector<CookiePreference> prefs;

    if (!preferences_db_) return prefs;

    const char* sql = "SELECT domain, block_all, block_third_party, allow_session_only FROM cookie_preferences";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(preferences_db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return prefs;

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        CookiePreference pref;
        pref.domain = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));
        pref.block_all = sqlite3_column_int(stmt, 1) != 0;
        pref.block_third_party = sqlite3_column_int(stmt, 2) != 0;
        pref.allow_session_only = sqlite3_column_int(stmt, 3) != 0;
        prefs.push_back(pref);
    }

    sqlite3_finalize(stmt);
    return prefs;
}
```

## V8 JavaScript Bindings

**File**: `cef-native/src/handlers/simple_render_process_handler.cpp` (extend)

```cpp
void SimpleRenderProcessHandler::OnContextCreated(...) {
    // ... existing code ...

    // Add cookies namespace
    CefRefPtr<CefV8Value> cookies_obj = CefV8Value::CreateObject(nullptr, nullptr);

    cookies_obj->SetValue("getAll",
        CefV8Value::CreateFunction("getAll", new CookiesV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    cookies_obj->SetValue("delete",
        CefV8Value::CreateFunction("delete", new CookiesV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    cookies_obj->SetValue("deleteAll",
        CefV8Value::CreateFunction("deleteAll", new CookiesV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    cookies_obj->SetValue("deleteDomain",
        CefV8Value::CreateFunction("deleteDomain", new CookiesV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    hodos_browser_obj->SetValue("cookies", cookies_obj, V8_PROPERTY_ATTRIBUTE_NONE);
}

// Note: GetAllCookies is async, would need callback mechanism or promise-based approach
```

## Frontend Implementation

**File**: `frontend/src/components/CookieManager.tsx`

```typescript
import React, { useState, useEffect } from 'react';
import {
  Box, Typography, List, ListItem, ListItemText, IconButton, Button, Accordion, AccordionSummary, AccordionDetails
} from '@mui/material';
import { Delete, ExpandMore } from '@mui/icons-material';

interface Cookie {
  name: string;
  value: string;
  domain: string;
  path: string;
  secure: boolean;
  httpOnly: boolean;
}

export function CookieManager() {
  const [cookies, setCookies] = useState<Map<string, Cookie[]>>(new Map());

  useEffect(() => {
    loadCookies();
  }, []);

  const loadCookies = () => {
    // Call native function (would need async handling)
    window.hodosBrowser.cookies.getAll((cookieList: Cookie[]) => {
      const grouped = new Map<string, Cookie[]>();
      cookieList.forEach(cookie => {
        if (!grouped.has(cookie.domain)) {
          grouped.set(cookie.domain, []);
        }
        grouped.get(cookie.domain)!.push(cookie);
      });
      setCookies(grouped);
    });
  };

  const handleDeleteDomain = (domain: string) => {
    window.hodosBrowser.cookies.deleteDomain(domain);
    loadCookies();
  };

  return (
    <Box sx={{ p: 2 }}>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 2 }}>
        <Typography variant="h5">Cookie Manager</Typography>
        <Button variant="contained" color="error" onClick={() => {
          window.hodosBrowser.cookies.deleteAll();
          setCookies(new Map());
        }}>
          Delete All Cookies
        </Button>
      </Box>

      {Array.from(cookies.keys()).map(domain => (
        <Accordion key={domain}>
          <AccordionSummary expandIcon={<ExpandMore />}>
            <Typography>{domain} ({cookies.get(domain)?.length} cookies)</Typography>
          </AccordionSummary>
          <AccordionDetails>
            <List>
              {cookies.get(domain)?.map((cookie, idx) => (
                <ListItem key={idx}>
                  <ListItemText
                    primary={cookie.name}
                    secondary={`${cookie.path} • ${cookie.secure ? 'Secure' : 'Not Secure'}`}
                  />
                </ListItem>
              ))}
            </List>
            <Button color="error" onClick={() => handleDeleteDomain(domain)}>
              Delete All for {domain}
            </Button>
          </AccordionDetails>
        </Accordion>
      ))}
    </Box>
  );
}
```

## Implementation Priority

### Favorites
1. Extend BookmarkManager
2. Add V8 bindings
3. Build FavoritesGrid component
4. Implement thumbnail capture (optional)

### Cookies
1. Create HodosCookieManager wrapper
2. Add V8 bindings (with async callback handling)
3. Build CookieManager component
4. Add preferences database

## Key Advantages

- **CEF Native**: All features in browser layer where they belong
- **No HTTP Overhead**: Direct function calls
- **Clean Separation**: Wallet backend stays pure
- **Performance**: Fast local database operations
- **Maintainability**: Single responsibility per component
