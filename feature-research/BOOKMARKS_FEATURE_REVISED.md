# Browser Bookmarks Feature Implementation (Revised)

## Overview

This document provides implementation strategy for adding bookmark functionality to the Hodos Browser. Unlike history and cookies which CEF provides, bookmarks must be implemented from scratch. The implementation is handled entirely in the CEF C++ layer, keeping the Rust wallet backend exclusively for BRC-100 wallet operations.

## Architecture Decision

### CEF Does NOT Provide Bookmarks

CEF/Chromium does not automatically manage bookmarks. We must implement:
- ✅ Custom SQLite database for bookmark storage
- ✅ All bookmark logic in CEF C++ layer
- ✅ Direct V8 JavaScript bindings (no HTTP API)
- ❌ No Rust backend involvement

### Implementation Location

**File**: `{user_data_path}/Bookmarks` (SQLite database)
**Code**: CEF C++ layer only
**Access**: Direct V8 bindings to JavaScript

## Database Schema

### Bookmarks SQLite Database

Create in CEF user data directory alongside History database:

```sql
-- Main bookmarks table
CREATE TABLE bookmarks (
    id TEXT PRIMARY KEY,           -- UUID
    url TEXT,                      -- NULL for folders
    title TEXT NOT NULL,
    description TEXT,
    favicon TEXT,                  -- Base64 or URL
    parent_id TEXT,                -- NULL for root level
    position INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_visited INTEGER,
    visit_count INTEGER DEFAULT 0,
    is_folder INTEGER DEFAULT 0,
    FOREIGN KEY (parent_id) REFERENCES bookmarks(id) ON DELETE CASCADE
);

CREATE INDEX idx_bookmarks_parent ON bookmarks(parent_id);
CREATE INDEX idx_bookmarks_position ON bookmarks(position);
CREATE INDEX idx_bookmarks_title ON bookmarks(title);
CREATE INDEX idx_bookmarks_url ON bookmarks(url);
CREATE INDEX idx_bookmarks_is_folder ON bookmarks(is_folder);

-- Tags (many-to-many)
CREATE TABLE bookmark_tags (
    bookmark_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    PRIMARY KEY (bookmark_id, tag),
    FOREIGN KEY (bookmark_id) REFERENCES bookmarks(id) ON DELETE CASCADE
);

CREATE INDEX idx_bookmark_tags_tag ON bookmark_tags(tag);
CREATE INDEX idx_bookmark_tags_bookmark_id ON bookmark_tags(bookmark_id);
```

## Implementation Architecture

### Layer 1: CEF C++ Bookmark Manager

**File**: `cef-native/include/core/BookmarkManager.h`

```cpp
#ifndef BOOKMARK_MANAGER_H
#define BOOKMARK_MANAGER_H

#include "include/cef_base.h"
#include <sqlite3.h>
#include <string>
#include <vector>

struct Bookmark {
    std::string id;
    std::string url;
    std::string title;
    std::string description;
    std::string favicon;
    std::string parent_id;
    int position;
    int64_t created_at;
    int64_t updated_at;
    int64_t last_visited;
    int visit_count;
    bool is_folder;
    std::vector<std::string> tags;
};

struct BookmarkFolder {
    Bookmark folder;
    std::vector<Bookmark> children;
};

class BookmarkManager {
public:
    static BookmarkManager& GetInstance();

    bool Initialize(const std::string& user_data_path);

    // CRUD operations
    std::string AddBookmark(const std::string& url,
                           const std::string& title,
                           const std::string& parent_id,
                           const std::vector<std::string>& tags,
                           bool is_folder = false);

    Bookmark GetBookmark(const std::string& id);
    std::vector<Bookmark> GetBookmarksInFolder(const std::string& parent_id);
    std::vector<Bookmark> SearchBookmarks(const std::string& query);
    std::vector<Bookmark> GetBookmarksByTag(const std::string& tag);

    bool UpdateBookmark(const std::string& id,
                       const std::string& title,
                       const std::string& url,
                       const std::vector<std::string>& tags);

    bool DeleteBookmark(const std::string& id);
    bool MoveBookmark(const std::string& id,
                     const std::string& new_parent_id,
                     int new_position);

    // Tag operations
    std::vector<std::string> GetAllTags();
    bool AddTag(const std::string& bookmark_id, const std::string& tag);
    bool RemoveTag(const std::string& bookmark_id, const std::string& tag);

    // Import/Export
    std::vector<Bookmark> ExportBookmarks();
    bool ImportBookmarks(const std::vector<Bookmark>& bookmarks);

    // Utility
    void IncrementVisitCount(const std::string& id);

private:
    BookmarkManager() = default;
    ~BookmarkManager();

    sqlite3* db_;
    std::string db_path_;

    bool OpenDatabase();
    void CloseDatabase();
    std::string GenerateUUID();
    int GetNextPosition(const std::string& parent_id);
    std::vector<std::string> GetTagsForBookmark(const std::string& bookmark_id);
    void SetTagsForBookmark(const std::string& bookmark_id, const std::vector<std::string>& tags);
};

#endif // BOOKMARK_MANAGER_H
```

**File**: `cef-native/src/core/BookmarkManager.cpp`

```cpp
#include "include/core/BookmarkManager.h"
#include <random>
#include <sstream>
#include <iomanip>
#include <chrono>

BookmarkManager& BookmarkManager::GetInstance() {
    static BookmarkManager instance;
    return instance;
}

BookmarkManager::~BookmarkManager() {
    CloseDatabase();
}

bool BookmarkManager::Initialize(const std::string& user_data_path) {
    db_path_ = user_data_path + "/Bookmarks";
    return OpenDatabase();
}

bool BookmarkManager::OpenDatabase() {
    int rc = sqlite3_open(db_path_.c_str(), &db_);
    if (rc != SQLITE_OK) {
        return false;
    }

    // Create tables
    const char* create_sql = R"(
        CREATE TABLE IF NOT EXISTS bookmarks (
            id TEXT PRIMARY KEY,
            url TEXT,
            title TEXT NOT NULL,
            description TEXT,
            favicon TEXT,
            parent_id TEXT,
            position INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            last_visited INTEGER,
            visit_count INTEGER DEFAULT 0,
            is_folder INTEGER DEFAULT 0,
            FOREIGN KEY (parent_id) REFERENCES bookmarks(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_bookmarks_parent ON bookmarks(parent_id);
        CREATE INDEX IF NOT EXISTS idx_bookmarks_position ON bookmarks(position);
        CREATE INDEX IF NOT EXISTS idx_bookmarks_title ON bookmarks(title);
        CREATE INDEX IF NOT EXISTS idx_bookmarks_url ON bookmarks(url);

        CREATE TABLE IF NOT EXISTS bookmark_tags (
            bookmark_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (bookmark_id, tag),
            FOREIGN KEY (bookmark_id) REFERENCES bookmarks(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_bookmark_tags_tag ON bookmark_tags(tag);
    )";

    char* err_msg = nullptr;
    rc = sqlite3_exec(db_, create_sql, nullptr, nullptr, &err_msg);

    if (rc != SQLITE_OK) {
        sqlite3_free(err_msg);
        CloseDatabase();
        return false;
    }

    return true;
}

void BookmarkManager::CloseDatabase() {
    if (db_) {
        sqlite3_close(db_);
        db_ = nullptr;
    }
}

std::string BookmarkManager::GenerateUUID() {
    std::random_device rd;
    std::mt19937_64 gen(rd());
    std::uniform_int_distribution<> dis(0, 15);

    const char* hex_chars = "0123456789abcdef";
    std::stringstream ss;

    for (int i = 0; i < 32; i++) {
        if (i == 8 || i == 12 || i == 16 || i == 20) {
            ss << '-';
        }
        ss << hex_chars[dis(gen)];
    }

    return ss.str();
}

std::string BookmarkManager::AddBookmark(
    const std::string& url,
    const std::string& title,
    const std::string& parent_id,
    const std::vector<std::string>& tags,
    bool is_folder) {

    if (!db_) return "";

    std::string id = GenerateUUID();
    auto now = std::chrono::system_clock::now().time_since_epoch().count();
    int position = GetNextPosition(parent_id);

    const char* sql = R"(
        INSERT INTO bookmarks (id, url, title, parent_id, position, created_at, updated_at, is_folder)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    )";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) return "";

    sqlite3_bind_text(stmt, 1, id.c_str(), -1, SQLITE_STATIC);
    sqlite3_bind_text(stmt, 2, url.c_str(), -1, SQLITE_STATIC);
    sqlite3_bind_text(stmt, 3, title.c_str(), -1, SQLITE_STATIC);

    if (parent_id.empty()) {
        sqlite3_bind_null(stmt, 4);
    } else {
        sqlite3_bind_text(stmt, 4, parent_id.c_str(), -1, SQLITE_STATIC);
    }

    sqlite3_bind_int(stmt, 5, position);
    sqlite3_bind_int64(stmt, 6, now);
    sqlite3_bind_int64(stmt, 7, now);
    sqlite3_bind_int(stmt, 8, is_folder ? 1 : 0);

    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE) return "";

    // Add tags
    SetTagsForBookmark(id, tags);

    return id;
}

Bookmark BookmarkManager::GetBookmark(const std::string& id) {
    Bookmark bookmark;

    if (!db_) return bookmark;

    const char* sql = "SELECT * FROM bookmarks WHERE id = ?";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return bookmark;

    sqlite3_bind_text(stmt, 1, id.c_str(), -1, SQLITE_STATIC);

    if (sqlite3_step(stmt) == SQLITE_ROW) {
        bookmark.id = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));

        const char* url = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 1));
        bookmark.url = url ? url : "";

        bookmark.title = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 2));

        const char* desc = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 3));
        bookmark.description = desc ? desc : "";

        const char* favicon = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 4));
        bookmark.favicon = favicon ? favicon : "";

        const char* parent = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 5));
        bookmark.parent_id = parent ? parent : "";

        bookmark.position = sqlite3_column_int(stmt, 6);
        bookmark.created_at = sqlite3_column_int64(stmt, 7);
        bookmark.updated_at = sqlite3_column_int64(stmt, 8);
        bookmark.last_visited = sqlite3_column_int64(stmt, 9);
        bookmark.visit_count = sqlite3_column_int(stmt, 10);
        bookmark.is_folder = sqlite3_column_int(stmt, 11) != 0;

        bookmark.tags = GetTagsForBookmark(id);
    }

    sqlite3_finalize(stmt);
    return bookmark;
}

std::vector<Bookmark> BookmarkManager::GetBookmarksInFolder(const std::string& parent_id) {
    std::vector<Bookmark> bookmarks;

    if (!db_) return bookmarks;

    const char* sql = parent_id.empty()
        ? "SELECT id FROM bookmarks WHERE parent_id IS NULL ORDER BY position ASC"
        : "SELECT id FROM bookmarks WHERE parent_id = ? ORDER BY position ASC";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) return bookmarks;

    if (!parent_id.empty()) {
        sqlite3_bind_text(stmt, 1, parent_id.c_str(), -1, SQLITE_STATIC);
    }

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        std::string id = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));
        bookmarks.push_back(GetBookmark(id));
    }

    sqlite3_finalize(stmt);
    return bookmarks;
}

std::vector<Bookmark> BookmarkManager::SearchBookmarks(const std::string& query) {
    std::vector<Bookmark> results;

    if (!db_) return results;

    const char* sql = R"(
        SELECT DISTINCT b.id
        FROM bookmarks b
        LEFT JOIN bookmark_tags bt ON b.id = bt.bookmark_id
        WHERE b.is_folder = 0 AND (
            b.title LIKE ? OR
            b.url LIKE ? OR
            b.description LIKE ? OR
            bt.tag LIKE ?
        )
        ORDER BY b.visit_count DESC, b.updated_at DESC
        LIMIT 50
    )";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) return results;

    std::string pattern = "%" + query + "%";
    sqlite3_bind_text(stmt, 1, pattern.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_text(stmt, 2, pattern.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_text(stmt, 3, pattern.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_text(stmt, 4, pattern.c_str(), -1, SQLITE_TRANSIENT);

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        std::string id = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));
        results.push_back(GetBookmark(id));
    }

    sqlite3_finalize(stmt);
    return results;
}

bool BookmarkManager::DeleteBookmark(const std::string& id) {
    if (!db_) return false;

    // SQLite CASCADE will handle child bookmarks and tags
    const char* sql = "DELETE FROM bookmarks WHERE id = ?";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return false;

    sqlite3_bind_text(stmt, 1, id.c_str(), -1, SQLITE_STATIC);
    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    return rc == SQLITE_DONE;
}

bool BookmarkManager::MoveBookmark(
    const std::string& id,
    const std::string& new_parent_id,
    int new_position) {

    if (!db_) return false;

    const char* sql = "UPDATE bookmarks SET parent_id = ?, position = ? WHERE id = ?";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return false;

    if (new_parent_id.empty()) {
        sqlite3_bind_null(stmt, 1);
    } else {
        sqlite3_bind_text(stmt, 1, new_parent_id.c_str(), -1, SQLITE_STATIC);
    }

    sqlite3_bind_int(stmt, 2, new_position);
    sqlite3_bind_text(stmt, 3, id.c_str(), -1, SQLITE_STATIC);

    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    return rc == SQLITE_DONE;
}

std::vector<std::string> BookmarkManager::GetAllTags() {
    std::vector<std::string> tags;

    if (!db_) return tags;

    const char* sql = "SELECT DISTINCT tag FROM bookmark_tags ORDER BY tag ASC";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return tags;

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        tags.push_back(reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0)));
    }

    sqlite3_finalize(stmt);
    return tags;
}

std::vector<std::string> BookmarkManager::GetTagsForBookmark(const std::string& bookmark_id) {
    std::vector<std::string> tags;

    if (!db_) return tags;

    const char* sql = "SELECT tag FROM bookmark_tags WHERE bookmark_id = ?";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return tags;

    sqlite3_bind_text(stmt, 1, bookmark_id.c_str(), -1, SQLITE_STATIC);

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        tags.push_back(reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0)));
    }

    sqlite3_finalize(stmt);
    return tags;
}

void BookmarkManager::SetTagsForBookmark(
    const std::string& bookmark_id,
    const std::vector<std::string>& tags) {

    if (!db_) return;

    // Clear existing tags
    const char* delete_sql = "DELETE FROM bookmark_tags WHERE bookmark_id = ?";
    sqlite3_stmt* stmt;

    sqlite3_prepare_v2(db_, delete_sql, -1, &stmt, nullptr);
    sqlite3_bind_text(stmt, 1, bookmark_id.c_str(), -1, SQLITE_STATIC);
    sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    // Add new tags
    const char* insert_sql = "INSERT INTO bookmark_tags (bookmark_id, tag) VALUES (?, ?)";

    for (const auto& tag : tags) {
        sqlite3_prepare_v2(db_, insert_sql, -1, &stmt, nullptr);
        sqlite3_bind_text(stmt, 1, bookmark_id.c_str(), -1, SQLITE_STATIC);
        sqlite3_bind_text(stmt, 2, tag.c_str(), -1, SQLITE_STATIC);
        sqlite3_step(stmt);
        sqlite3_finalize(stmt);
    }
}

int BookmarkManager::GetNextPosition(const std::string& parent_id) {
    if (!db_) return 0;

    const char* sql = parent_id.empty()
        ? "SELECT COALESCE(MAX(position), -1) + 1 FROM bookmarks WHERE parent_id IS NULL"
        : "SELECT COALESCE(MAX(position), -1) + 1 FROM bookmarks WHERE parent_id = ?";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) return 0;

    if (!parent_id.empty()) {
        sqlite3_bind_text(stmt, 1, parent_id.c_str(), -1, SQLITE_STATIC);
    }

    int position = 0;
    if (sqlite3_step(stmt) == SQLITE_ROW) {
        position = sqlite3_column_int(stmt, 0);
    }

    sqlite3_finalize(stmt);
    return position;
}

std::vector<Bookmark> BookmarkManager::ExportBookmarks() {
    std::vector<Bookmark> all_bookmarks;

    if (!db_) return all_bookmarks;

    const char* sql = "SELECT id FROM bookmarks ORDER BY position ASC";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return all_bookmarks;

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        std::string id = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));
        all_bookmarks.push_back(GetBookmark(id));
    }

    sqlite3_finalize(stmt);
    return all_bookmarks;
}
```

### Layer 2: CEF V8 JavaScript Bindings

**File**: `cef-native/src/handlers/simple_render_process_handler.cpp` (extend)

```cpp
void SimpleRenderProcessHandler::OnContextCreated(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefRefPtr<CefV8Context> context) {

    // ... existing code ...

    // Add bookmarks namespace
    CefRefPtr<CefV8Value> bookmarks_obj = CefV8Value::CreateObject(nullptr, nullptr);

    bookmarks_obj->SetValue("add",
        CefV8Value::CreateFunction("add", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    bookmarks_obj->SetValue("get",
        CefV8Value::CreateFunction("get", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    bookmarks_obj->SetValue("list",
        CefV8Value::CreateFunction("list", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    bookmarks_obj->SetValue("search",
        CefV8Value::CreateFunction("search", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    bookmarks_obj->SetValue("update",
        CefV8Value::CreateFunction("update", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    bookmarks_obj->SetValue("delete",
        CefV8Value::CreateFunction("delete", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    bookmarks_obj->SetValue("move",
        CefV8Value::CreateFunction("move", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    bookmarks_obj->SetValue("getTags",
        CefV8Value::CreateFunction("getTags", new BookmarksV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    hodos_browser_obj->SetValue("bookmarks", bookmarks_obj, V8_PROPERTY_ATTRIBUTE_NONE);
}

// V8 Handler implementation
class BookmarksV8Handler : public CefV8Handler {
public:
    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        auto& manager = BookmarkManager::GetInstance();

        if (name == "add") {
            // arguments[0] = { url, title, parentId, tags, isFolder }
            if (arguments.size() > 0 && arguments[0]->IsObject()) {
                auto params = arguments[0];

                std::string url = params->GetValue("url")->GetStringValue().ToString();
                std::string title = params->GetValue("title")->GetStringValue().ToString();
                std::string parent_id = params->HasValue("parentId")
                    ? params->GetValue("parentId")->GetStringValue().ToString()
                    : "";

                std::vector<std::string> tags;
                if (params->HasValue("tags") && params->GetValue("tags")->IsArray()) {
                    auto tags_array = params->GetValue("tags");
                    for (int i = 0; i < tags_array->GetArrayLength(); i++) {
                        tags.push_back(tags_array->GetValue(i)->GetStringValue().ToString());
                    }
                }

                bool is_folder = params->HasValue("isFolder")
                    ? params->GetValue("isFolder")->GetBoolValue()
                    : false;

                std::string id = manager.AddBookmark(url, title, parent_id, tags, is_folder);
                retval = CefV8Value::CreateString(id);
                return true;
            }
        }
        else if (name == "list") {
            // arguments[0] = folder_id (optional)
            std::string folder_id;
            if (arguments.size() > 0 && arguments[0]->IsString()) {
                folder_id = arguments[0]->GetStringValue().ToString();
            }

            auto bookmarks = manager.GetBookmarksInFolder(folder_id);

            // Convert to V8 array
            retval = CefV8Value::CreateArray(bookmarks.size());
            for (size_t i = 0; i < bookmarks.size(); i++) {
                retval->SetValue(i, BookmarkToV8(bookmarks[i]));
            }
            return true;
        }
        else if (name == "search") {
            if (arguments.size() > 0 && arguments[0]->IsString()) {
                std::string query = arguments[0]->GetStringValue().ToString();
                auto results = manager.SearchBookmarks(query);

                retval = CefV8Value::CreateArray(results.size());
                for (size_t i = 0; i < results.size(); i++) {
                    retval->SetValue(i, BookmarkToV8(results[i]));
                }
                return true;
            }
        }
        else if (name == "delete") {
            if (arguments.size() > 0 && arguments[0]->IsString()) {
                std::string id = arguments[0]->GetStringValue().ToString();
                bool success = manager.DeleteBookmark(id);
                retval = CefV8Value::CreateBool(success);
                return true;
            }
        }

        return false;
    }

private:
    CefRefPtr<CefV8Value> BookmarkToV8(const Bookmark& bookmark) {
        auto obj = CefV8Value::CreateObject(nullptr, nullptr);
        obj->SetValue("id", CefV8Value::CreateString(bookmark.id), V8_PROPERTY_ATTRIBUTE_NONE);
        obj->SetValue("url", CefV8Value::CreateString(bookmark.url), V8_PROPERTY_ATTRIBUTE_NONE);
        obj->SetValue("title", CefV8Value::CreateString(bookmark.title), V8_PROPERTY_ATTRIBUTE_NONE);
        obj->SetValue("isFolder", CefV8Value::CreateBool(bookmark.is_folder), V8_PROPERTY_ATTRIBUTE_NONE);
        obj->SetValue("visitCount", CefV8Value::CreateInt(bookmark.visit_count), V8_PROPERTY_ATTRIBUTE_NONE);

        // Tags array
        auto tags_array = CefV8Value::CreateArray(bookmark.tags.size());
        for (size_t i = 0; i < bookmark.tags.size(); i++) {
            tags_array->SetValue(i, CefV8Value::CreateString(bookmark.tags[i]));
        }
        obj->SetValue("tags", tags_array, V8_PROPERTY_ATTRIBUTE_NONE);

        return obj;
    }

    IMPLEMENT_REFCOUNTING(BookmarksV8Handler);
};
```

### Layer 3: Frontend Implementation

**File**: `frontend/src/hooks/useBookmarks.ts`

```typescript
import { useState, useCallback, useEffect } from 'react';

interface Bookmark {
  id: string;
  url: string;
  title: string;
  isFolder: boolean;
  visitCount: number;
  tags: string[];
}

export function useBookmarks(folderId?: string) {
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadBookmarks = useCallback(async () => {
    setLoading(true);
    try {
      // Direct synchronous native call
      const data = window.hodosBrowser.bookmarks.list(folderId);
      setBookmarks(data);
    } catch (err) {
      setError('Failed to load bookmarks');
    } finally {
      setLoading(false);
    }
  }, [folderId]);

  useEffect(() => {
    loadBookmarks();
  }, [loadBookmarks]);

  const addBookmark = useCallback(async (params: {
    url: string;
    title: string;
    parentId?: string;
    tags?: string[];
    isFolder?: boolean;
  }) => {
    try {
      const id = window.hodosBrowser.bookmarks.add(params);
      await loadBookmarks();
      return id;
    } catch (err) {
      setError('Failed to add bookmark');
      throw err;
    }
  }, [loadBookmarks]);

  const deleteBookmark = useCallback(async (id: string) => {
    try {
      window.hodosBrowser.bookmarks.delete(id);
      setBookmarks(prev => prev.filter(b => b.id !== id));
    } catch (err) {
      setError('Failed to delete bookmark');
    }
  }, []);

  const searchBookmarks = useCallback(async (query: string) => {
    setLoading(true);
    try {
      const results = window.hodosBrowser.bookmarks.search(query);
      setBookmarks(results);
    } catch (err) {
      setError('Failed to search bookmarks');
    } finally {
      setLoading(false);
    }
  }, []);

  return {
    bookmarks,
    loading,
    error,
    loadBookmarks,
    addBookmark,
    deleteBookmark,
    searchBookmarks
  };
}
```

## Implementation Steps

### Phase 1: CEF C++ Layer
1. Create BookmarkManager class
2. Implement SQLite database operations
3. Test CRUD operations

### Phase 2: V8 Bindings
1. Create BookmarksV8Handler
2. Expose functions in OnContextCreated
3. Test JavaScript access

### Phase 3: Frontend
1. Create TypeScript types
2. Build useBookmarks hook
3. Create BookmarksPanel component

### Phase 4: Integration
1. Initialize BookmarkManager with user data path
2. Test end-to-end functionality
3. Add import/export features

## Key Advantages

- **No HTTP overhead**: Direct native calls
- **Fast SQLite access**: Optimized for local operations
- **Clean separation**: Wallet backend stays pure
- **Full control**: Custom schema and features
