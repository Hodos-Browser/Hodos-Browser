# Browser History Feature Implementation (Revised)

## Overview

This document outlines the implementation strategy for adding browser history functionality to the Hodos Browser. The implementation leverages CEF's built-in history database and is handled entirely in the CEF C++ layer, keeping the Rust wallet backend exclusively for BRC-100 wallet operations.

## Architecture Decision

### CEF's Built-in History Database

CEF (Chromium Embedded Framework) automatically creates and manages a History SQLite database when initialized with a user data directory. This database contains:

- **Location**: `{user_data_path}/History`
- **Format**: SQLite database
- **Schema**: Standard Chromium history schema
- **Management**: Automatically populated by CEF during navigation

### Implementation Approach

**✅ Use CEF's existing History database** (read-only queries from C++)
- Access via direct SQLite connection in C++ layer
- No need to manually track navigation (CEF does this automatically)
- Standard Chromium schema (well-documented and stable)

**✅ Optional: Create metadata database** for custom features
- Separate SQLite database for user annotations, tags, categories
- Managed in CEF C++ layer
- Links to CEF's history via URL

**❌ Do NOT use Rust backend** for browser features
- Rust-wallet backend remains exclusively for BRC-100 wallet operations
- All browser feature logic in CEF C++ layer
- Direct V8 JavaScript bindings (no HTTP API)

## CEF History Database Schema

### Chromium's Built-in Tables

CEF automatically maintains these tables in the History database:

```sql
-- urls table (automatically managed by CEF)
CREATE TABLE urls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL UNIQUE,
    title TEXT,
    visit_count INTEGER DEFAULT 0,
    typed_count INTEGER DEFAULT 0,
    last_visit_time INTEGER NOT NULL,
    hidden INTEGER DEFAULT 0
);

-- visits table (automatically managed by CEF)
CREATE TABLE visits (
    id INTEGER PRIMARY KEY AUTOINCERATE,
    url INTEGER NOT NULL,  -- Foreign key to urls.id
    visit_time INTEGER NOT NULL,
    from_visit INTEGER,
    transition INTEGER NOT NULL,
    segment_id INTEGER,
    visit_duration INTEGER DEFAULT 0
);

-- keyword_search_terms (optional)
CREATE TABLE keyword_search_terms (
    keyword_id INTEGER NOT NULL,
    url_id INTEGER NOT NULL,
    term TEXT NOT NULL,
    normalized_term TEXT NOT NULL
);
```

**Note**: These tables are created and populated automatically by CEF. Do not modify them directly.

### Optional: Custom Metadata Database

Create a separate database for custom features only:

```sql
-- File: {user_data_path}/HistoryMetadata
CREATE TABLE history_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL UNIQUE,
    tags TEXT,              -- JSON array of tags
    notes TEXT,             -- User notes
    category TEXT,          -- Custom category
    archived INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_metadata_url ON history_metadata(url);
CREATE INDEX idx_metadata_archived ON history_metadata(archived);
```

## Implementation Architecture

### Layer 1: CEF C++ History Manager

**File**: `cef-native/include/core/HistoryManager.h`

```cpp
#ifndef HISTORY_MANAGER_H
#define HISTORY_MANAGER_H

#include "include/cef_base.h"
#include <sqlite3.h>
#include <string>
#include <vector>
#include <functional>

struct HistoryEntry {
    int64_t id;
    std::string url;
    std::string title;
    int visit_count;
    int64_t last_visit_time;
    int64_t visit_time;  // Specific visit time
    int transition;
};

struct HistorySearchParams {
    std::string search_term;
    int64_t start_time;
    int64_t end_time;
    int limit;
    int offset;
};

class HistoryManager {
public:
    static HistoryManager& GetInstance();

    // Initialize - gets path to CEF's History database
    bool Initialize(const std::string& user_data_path);

    // Query CEF's History database
    std::vector<HistoryEntry> GetHistory(int limit, int offset);
    std::vector<HistoryEntry> SearchHistory(const HistorySearchParams& params);
    HistoryEntry GetHistoryEntry(const std::string& url);

    // Delete operations (directly manipulate CEF's database)
    bool DeleteHistoryEntry(const std::string& url);
    bool DeleteAllHistory();
    bool DeleteHistoryRange(int64_t start_time, int64_t end_time);

    // Optional: Metadata operations (separate database)
    bool AddMetadata(const std::string& url, const std::string& tags, const std::string& notes);
    bool UpdateMetadata(const std::string& url, const std::string& tags, const std::string& notes);

private:
    HistoryManager() = default;
    ~HistoryManager();

    sqlite3* history_db_;       // CEF's History database
    sqlite3* metadata_db_;      // Optional custom metadata
    std::string history_db_path_;
    std::string metadata_db_path_;

    bool OpenDatabases();
    void CloseDatabases();
    static int64_t GetCurrentChromiumTime();
    static int64_t ChromiumTimeToUnix(int64_t chromium_time);
};

#endif // HISTORY_MANAGER_H
```

**File**: `cef-native/src/core/HistoryManager.cpp`

```cpp
#include "include/core/HistoryManager.h"
#include <chrono>
#include <sstream>

HistoryManager& HistoryManager::GetInstance() {
    static HistoryManager instance;
    return instance;
}

HistoryManager::~HistoryManager() {
    CloseDatabases();
}

bool HistoryManager::Initialize(const std::string& user_data_path) {
    history_db_path_ = user_data_path + "/History";
    metadata_db_path_ = user_data_path + "/HistoryMetadata";

    return OpenDatabases();
}

bool HistoryManager::OpenDatabases() {
    // Open CEF's History database (read-write)
    int rc = sqlite3_open(history_db_path_.c_str(), &history_db_);
    if (rc != SQLITE_OK) {
        return false;
    }

    // Open/create metadata database
    rc = sqlite3_open(metadata_db_path_.c_str(), &metadata_db_);
    if (rc != SQLITE_OK) {
        sqlite3_close(history_db_);
        return false;
    }

    // Create metadata table if needed
    const char* create_metadata_sql = R"(
        CREATE TABLE IF NOT EXISTS history_metadata (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT NOT NULL UNIQUE,
            tags TEXT,
            notes TEXT,
            category TEXT,
            archived INTEGER DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_metadata_url ON history_metadata(url);
    )";

    char* err_msg = nullptr;
    rc = sqlite3_exec(metadata_db_, create_metadata_sql, nullptr, nullptr, &err_msg);
    if (rc != SQLITE_OK) {
        sqlite3_free(err_msg);
        CloseDatabases();
        return false;
    }

    return true;
}

void HistoryManager::CloseDatabases() {
    if (history_db_) {
        sqlite3_close(history_db_);
        history_db_ = nullptr;
    }
    if (metadata_db_) {
        sqlite3_close(metadata_db_);
        metadata_db_ = nullptr;
    }
}

std::vector<HistoryEntry> HistoryManager::GetHistory(int limit, int offset) {
    std::vector<HistoryEntry> entries;

    if (!history_db_) return entries;

    const char* sql = R"(
        SELECT u.id, u.url, u.title, u.visit_count, u.last_visit_time, v.visit_time, v.transition
        FROM urls u
        INNER JOIN visits v ON u.id = v.url
        WHERE u.hidden = 0
        ORDER BY v.visit_time DESC
        LIMIT ? OFFSET ?
    )";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(history_db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) return entries;

    sqlite3_bind_int(stmt, 1, limit);
    sqlite3_bind_int(stmt, 2, offset);

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        HistoryEntry entry;
        entry.id = sqlite3_column_int64(stmt, 0);
        entry.url = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 1));

        const char* title = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 2));
        entry.title = title ? title : "";

        entry.visit_count = sqlite3_column_int(stmt, 3);
        entry.last_visit_time = sqlite3_column_int64(stmt, 4);
        entry.visit_time = sqlite3_column_int64(stmt, 5);
        entry.transition = sqlite3_column_int(stmt, 6);

        entries.push_back(entry);
    }

    sqlite3_finalize(stmt);
    return entries;
}

std::vector<HistoryEntry> HistoryManager::SearchHistory(const HistorySearchParams& params) {
    std::vector<HistoryEntry> entries;

    if (!history_db_) return entries;

    std::stringstream sql;
    sql << "SELECT u.id, u.url, u.title, u.visit_count, u.last_visit_time, v.visit_time, v.transition "
        << "FROM urls u "
        << "INNER JOIN visits v ON u.id = v.url "
        << "WHERE u.hidden = 0";

    if (!params.search_term.empty()) {
        sql << " AND (u.url LIKE ? OR u.title LIKE ?)";
    }

    if (params.start_time > 0) {
        sql << " AND v.visit_time >= ?";
    }

    if (params.end_time > 0) {
        sql << " AND v.visit_time <= ?";
    }

    sql << " ORDER BY v.visit_time DESC LIMIT ? OFFSET ?";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(history_db_, sql.str().c_str(), -1, &stmt, nullptr);

    if (rc != SQLITE_OK) return entries;

    int param_index = 1;

    if (!params.search_term.empty()) {
        std::string pattern = "%" + params.search_term + "%";
        sqlite3_bind_text(stmt, param_index++, pattern.c_str(), -1, SQLITE_TRANSIENT);
        sqlite3_bind_text(stmt, param_index++, pattern.c_str(), -1, SQLITE_TRANSIENT);
    }

    if (params.start_time > 0) {
        sqlite3_bind_int64(stmt, param_index++, params.start_time);
    }

    if (params.end_time > 0) {
        sqlite3_bind_int64(stmt, param_index++, params.end_time);
    }

    sqlite3_bind_int(stmt, param_index++, params.limit);
    sqlite3_bind_int(stmt, param_index++, params.offset);

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        HistoryEntry entry;
        entry.id = sqlite3_column_int64(stmt, 0);
        entry.url = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 1));

        const char* title = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 2));
        entry.title = title ? title : "";

        entry.visit_count = sqlite3_column_int(stmt, 3);
        entry.last_visit_time = sqlite3_column_int64(stmt, 4);
        entry.visit_time = sqlite3_column_int64(stmt, 5);
        entry.transition = sqlite3_column_int(stmt, 6);

        entries.push_back(entry);
    }

    sqlite3_finalize(stmt);
    return entries;
}

bool HistoryManager::DeleteHistoryEntry(const std::string& url) {
    if (!history_db_) return false;

    // First get the url_id
    const char* get_id_sql = "SELECT id FROM urls WHERE url = ?";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(history_db_, get_id_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return false;

    sqlite3_bind_text(stmt, 1, url.c_str(), -1, SQLITE_STATIC);

    int64_t url_id = -1;
    if (sqlite3_step(stmt) == SQLITE_ROW) {
        url_id = sqlite3_column_int64(stmt, 0);
    }
    sqlite3_finalize(stmt);

    if (url_id < 0) return false;

    // Delete visits
    const char* delete_visits_sql = "DELETE FROM visits WHERE url = ?";
    rc = sqlite3_prepare_v2(history_db_, delete_visits_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return false;

    sqlite3_bind_int64(stmt, 1, url_id);
    sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    // Delete URL
    const char* delete_url_sql = "DELETE FROM urls WHERE id = ?";
    rc = sqlite3_prepare_v2(history_db_, delete_url_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return false;

    sqlite3_bind_int64(stmt, 1, url_id);
    sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    return true;
}

bool HistoryManager::DeleteAllHistory() {
    if (!history_db_) return false;

    const char* delete_sql = R"(
        DELETE FROM visits;
        DELETE FROM urls;
        DELETE FROM keyword_search_terms;
    )";

    char* err_msg = nullptr;
    int rc = sqlite3_exec(history_db_, delete_sql, nullptr, nullptr, &err_msg);

    if (rc != SQLITE_OK) {
        sqlite3_free(err_msg);
        return false;
    }

    return true;
}

bool HistoryManager::DeleteHistoryRange(int64_t start_time, int64_t end_time) {
    if (!history_db_) return false;

    const char* delete_visits_sql = "DELETE FROM visits WHERE visit_time >= ? AND visit_time <= ?";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(history_db_, delete_visits_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return false;

    sqlite3_bind_int64(stmt, 1, start_time);
    sqlite3_bind_int64(stmt, 2, end_time);
    sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    // Clean up orphaned URLs
    const char* cleanup_sql = "DELETE FROM urls WHERE id NOT IN (SELECT DISTINCT url FROM visits)";
    rc = sqlite3_prepare_v2(history_db_, cleanup_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) return false;

    sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    return true;
}

int64_t HistoryManager::GetCurrentChromiumTime() {
    // Chromium time: microseconds since January 1, 1601 UTC
    auto now = std::chrono::system_clock::now();
    auto unix_time = std::chrono::duration_cast<std::chrono::microseconds>(
        now.time_since_epoch()
    ).count();

    // Convert Unix epoch (1970) to Windows epoch (1601)
    // 11644473600 seconds = difference between epochs
    return unix_time + (11644473600LL * 1000000LL);
}

int64_t HistoryManager::ChromiumTimeToUnix(int64_t chromium_time) {
    return (chromium_time / 1000000) - 11644473600LL;
}
```

### Layer 2: CEF V8 JavaScript Bindings

**File**: `cef-native/src/handlers/simple_render_process_handler.cpp` (extend existing)

```cpp
void SimpleRenderProcessHandler::OnContextCreated(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefRefPtr<CefV8Context> context) {

    // ... existing code ...

    // Add history namespace to window.hodosBrowser
    CefRefPtr<CefV8Value> history_obj = CefV8Value::CreateObject(nullptr, nullptr);

    // getHistory function
    history_obj->SetValue("get",
        CefV8Value::CreateFunction("get", new HistoryV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    // searchHistory function
    history_obj->SetValue("search",
        CefV8Value::CreateFunction("search", new HistoryV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    // deleteEntry function
    history_obj->SetValue("delete",
        CefV8Value::CreateFunction("delete", new HistoryV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    // clearAll function
    history_obj->SetValue("clearAll",
        CefV8Value::CreateFunction("clearAll", new HistoryV8Handler()),
        V8_PROPERTY_ATTRIBUTE_NONE);

    hodos_browser_obj->SetValue("history", history_obj, V8_PROPERTY_ATTRIBUTE_NONE);
}

// V8 Handler for history operations
class HistoryV8Handler : public CefV8Handler {
public:
    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        if (name == "get") {
            // arguments[0] = { limit, offset }
            int limit = 50;
            int offset = 0;

            if (arguments.size() > 0 && arguments[0]->IsObject()) {
                CefRefPtr<CefV8Value> params = arguments[0];
                if (params->HasValue("limit")) {
                    limit = params->GetValue("limit")->GetIntValue();
                }
                if (params->HasValue("offset")) {
                    offset = params->GetValue("offset")->GetIntValue();
                }
            }

            auto entries = HistoryManager::GetInstance().GetHistory(limit, offset);

            // Convert to V8 array
            retval = CefV8Value::CreateArray(entries.size());
            for (size_t i = 0; i < entries.size(); i++) {
                CefRefPtr<CefV8Value> entry_obj = CefV8Value::CreateObject(nullptr, nullptr);
                entry_obj->SetValue("url", CefV8Value::CreateString(entries[i].url), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("title", CefV8Value::CreateString(entries[i].title), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("visitCount", CefV8Value::CreateInt(entries[i].visit_count), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("visitTime", CefV8Value::CreateDouble(entries[i].visit_time), V8_PROPERTY_ATTRIBUTE_NONE);

                retval->SetValue(i, entry_obj);
            }

            return true;
        }
        else if (name == "delete") {
            if (arguments.size() > 0 && arguments[0]->IsString()) {
                std::string url = arguments[0]->GetStringValue().ToString();
                bool success = HistoryManager::GetInstance().DeleteHistoryEntry(url);
                retval = CefV8Value::CreateBool(success);
                return true;
            }
        }
        else if (name == "clearAll") {
            bool success = HistoryManager::GetInstance().DeleteAllHistory();
            retval = CefV8Value::CreateBool(success);
            return true;
        }

        return false;
    }

    IMPLEMENT_REFCOUNTING(HistoryV8Handler);
};
```

### Layer 3: Frontend TypeScript Implementation

**File**: `frontend/src/types/history.d.ts`

```typescript
export interface HistoryEntry {
  url: string;
  title: string;
  visitCount: number;
  visitTime: number;  // Chromium timestamp (microseconds since 1601)
  transition: number;
}

export interface HistorySearchParams {
  search?: string;
  startTime?: number;
  endTime?: number;
  limit?: number;
  offset?: number;
}
```

**File**: `frontend/src/bridge/initWindowBridge.ts` (history functions)

```typescript
// Extend window.hodosBrowser with history namespace
declare global {
  interface Window {
    hodosBrowser: {
      // ... existing wallet methods ...
      history: {
        get: (params?: { limit?: number; offset?: number }) => Promise<HistoryEntry[]>;
        search: (params: HistorySearchParams) => Promise<HistoryEntry[]>;
        delete: (url: string) => Promise<boolean>;
        clearAll: () => Promise<boolean>;
        clearRange: (startTime: number, endTime: number) => Promise<boolean>;
      };
    };
  }
}

// Note: These functions are now SYNCHRONOUS native calls, not HTTP requests
// The CEF V8 bindings provide direct access to HistoryManager
```

**File**: `frontend/src/hooks/useHistory.ts`

```typescript
import { useState, useEffect, useCallback } from 'react';
import { HistoryEntry, HistorySearchParams } from '../types/history';

export function useHistory() {
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchHistory = useCallback(async (params: { limit?: number; offset?: number } = {}) => {
    setLoading(true);
    setError(null);
    try {
      // Direct synchronous call to CEF native function
      const entries = await window.hodosBrowser.history.get(params);
      setHistory(entries);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch history');
    } finally {
      setLoading(false);
    }
  }, []);

  const deleteEntry = useCallback(async (url: string) => {
    try {
      const success = await window.hodosBrowser.history.delete(url);
      if (success) {
        setHistory(prev => prev.filter(entry => entry.url !== url));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete entry');
    }
  }, []);

  const clearAllHistory = useCallback(async () => {
    try {
      const success = await window.hodosBrowser.history.clearAll();
      if (success) {
        setHistory([]);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to clear history');
    }
  }, []);

  const searchHistory = useCallback(async (params: HistorySearchParams) => {
    setLoading(true);
    setError(null);
    try {
      const results = await window.hodosBrowser.history.search(params);
      setHistory(results);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to search history');
    } finally {
      setLoading(false);
    }
  }, []);

  return {
    history,
    loading,
    error,
    fetchHistory,
    deleteEntry,
    clearAllHistory,
    searchHistory
  };
}
```

**File**: `frontend/src/components/HistoryPanel.tsx`

```typescript
import React, { useEffect, useState } from 'react';
import { useHistory } from '../hooks/useHistory';
import { Box, List, ListItem, ListItemText, TextField, IconButton, Typography, CircularProgress, Button } from '@mui/material';
import { Delete, Clear, Search } from '@mui/icons-material';

export function HistoryPanel() {
  const { history, loading, error, fetchHistory, deleteEntry, clearAllHistory, searchHistory } = useHistory();
  const [searchTerm, setSearchTerm] = useState('');

  useEffect(() => {
    fetchHistory({ limit: 100, offset: 0 });
  }, [fetchHistory]);

  const handleSearch = (term: string) => {
    setSearchTerm(term);
    if (term) {
      searchHistory({ search: term, limit: 100, offset: 0 });
    } else {
      fetchHistory({ limit: 100, offset: 0 });
    }
  };

  const formatDate = (chromiumTime: number) => {
    // Convert Chromium timestamp to JavaScript Date
    const unixTimestamp = (chromiumTime / 1000000) - 11644473600;
    const date = new Date(unixTimestamp * 1000);
    return date.toLocaleString();
  };

  return (
    <Box sx={{ p: 2 }}>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
        <Typography variant="h5">Browsing History</Typography>
        <Button
          variant="outlined"
          color="error"
          startIcon={<Clear />}
          onClick={clearAllHistory}
        >
          Clear All
        </Button>
      </Box>

      <TextField
        fullWidth
        placeholder="Search history..."
        value={searchTerm}
        onChange={(e) => handleSearch(e.target.value)}
        InputProps={{
          startAdornment: <Search sx={{ mr: 1, color: 'action.active' }} />
        }}
        sx={{ mb: 2 }}
      />

      {loading && <CircularProgress />}
      {error && <Typography color="error">{error}</Typography>}

      <List>
        {history.map((entry, index) => (
          <ListItem
            key={`${entry.url}-${index}`}
            secondaryAction={
              <IconButton edge="end" onClick={() => deleteEntry(entry.url)}>
                <Delete />
              </IconButton>
            }
          >
            <ListItemText
              primary={entry.title || entry.url}
              secondary={
                <>
                  <Typography component="span" variant="body2" color="text.secondary">
                    {entry.url}
                  </Typography>
                  <br />
                  <Typography component="span" variant="caption" color="text.secondary">
                    {formatDate(entry.visitTime)} • Visited {entry.visitCount} times
                  </Typography>
                </>
              }
            />
          </ListItem>
        ))}
      </List>
    </Box>
  );
}
```

## Implementation Steps

### Phase 1: CEF C++ Layer
1. Create HistoryManager class
2. Implement SQLite database access to CEF's History database
3. Add optional metadata database
4. Test database queries

### Phase 2: CEF V8 Bindings
1. Create V8 handler for history operations
2. Expose functions in OnContextCreated
3. Test JavaScript access to native functions

### Phase 3: Frontend Implementation
1. Define TypeScript interfaces
2. Create useHistory hook
3. Build HistoryPanel component
4. Test UI functionality

### Phase 4: Integration
1. Initialize HistoryManager with CEF user data path
2. Integrate with browser navigation
3. Test end-to-end functionality

## Key Advantages of This Approach

### Performance
- **No HTTP overhead**: Direct native function calls
- **Fast database access**: SQLite is extremely fast for reads
- **No serialization**: Data passed directly through V8

### Architecture
- **Clean separation**: Wallet backend stays pure
- **CEF native**: Browser features where they belong
- **Maintainability**: Single responsibility per component

### Reliability
- **CEF manages history**: Automatic population, no tracking needed
- **Standard schema**: Well-documented Chromium format
- **Battle-tested**: Proven database structure

## Performance Optimization

- Use prepared statements for repeated queries
- Index all search columns (already done by CEF)
- Implement pagination (limit/offset)
- Cache recent queries in C++ layer if needed
- Close database connections properly

## Security Considerations

- Validate all SQL parameters (prevent injection)
- Handle database locking properly
- Respect private/incognito mode (check CEF flags)
- Sanitize user input before queries

## Testing Strategy

### Unit Tests (C++)
- HistoryManager database operations
- SQL query correctness
- Timestamp conversions

### Integration Tests
- V8 bindings functionality
- End-to-end history retrieval
- Search functionality

### UI Tests
- Component rendering
- User interactions
- Error handling

## Troubleshooting

**Database locked errors**
- Ensure proper connection management
- Use WAL mode if needed: `PRAGMA journal_mode=WAL;`
- Close connections when not in use

**Empty history**
- Verify CEF user_data_path is set correctly
- Check that CEF is creating History database
- Ensure database path is correct

**Timestamp issues**
- Remember Chromium uses microseconds since 1601
- Convert properly for display (see ChromiumTimeToUnix)
