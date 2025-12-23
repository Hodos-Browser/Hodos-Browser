# Browser Cookies Management Feature Implementation

## Overview

This document provides a comprehensive implementation strategy for adding cookie management functionality to the Hodos Browser. Cookies are already handled automatically by CEF (Chromium Embedded Framework), but this feature adds user-facing tools to view, manage, delete, and control cookies with enhanced privacy features.

## CEF Cookie Architecture

### Built-in Cookie Support

CEF automatically manages cookies through Chromium's underlying cookie system:

- **Automatic Storage**: Cookies are stored in CEF's user data directory
- **Database Format**: SQLite database (Cookies file)
- **Persistence**: Cookies persist across browser sessions
- **Standards Compliant**: Full support for HTTP cookies, secure cookies, SameSite attributes

### Cookie Storage Location

Cookies are stored in the CEF user data directory:
```
%APPDATA%/HodosBrowser/
├── Cookies              # SQLite database
├── Cookies-journal      # SQLite journal
└── ...
```

### CEF Cookie Manager API

CEF provides the `CefCookieManager` class for programmatic cookie access:

```cpp
// Get the global cookie manager
CefRefPtr<CefCookieManager> manager = CefCookieManager::GetGlobalManager(nullptr);

// Access cookies
manager->VisitAllCookies(visitor);
manager->VisitUrlCookies(url, includeHttpOnly, visitor);

// Set/Delete cookies
manager->SetCookie(url, cookie, callback);
manager->DeleteCookies(url, cookie_name, callback);
```

## Database Schema

### Chromium Cookie Database Structure

The Cookies SQLite database has the following schema:

```sql
-- Chromium's cookies table (read-only reference)
CREATE TABLE cookies (
    creation_utc INTEGER NOT NULL,
    host_key TEXT NOT NULL,
    top_frame_site_key TEXT NOT NULL,
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    encrypted_value BLOB DEFAULT '',
    path TEXT NOT NULL,
    expires_utc INTEGER NOT NULL,
    is_secure INTEGER NOT NULL,
    is_httponly INTEGER NOT NULL,
    last_access_utc INTEGER NOT NULL,
    has_expires INTEGER NOT NULL,
    is_persistent INTEGER NOT NULL,
    priority INTEGER NOT NULL,
    samesite INTEGER NOT NULL,
    source_scheme INTEGER NOT NULL,
    source_port INTEGER NOT NULL,
    is_same_party INTEGER NOT NULL,
    last_update_utc INTEGER NOT NULL,
    UNIQUE (host_key, top_frame_site_key, name, path, source_scheme, source_port)
);
```

### Custom Cookie Metadata Table

Create a separate table for cookie management metadata:

```sql
CREATE TABLE cookie_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    host_key TEXT NOT NULL,
    cookie_name TEXT NOT NULL,
    blocked INTEGER DEFAULT 0,
    whitelist INTEGER DEFAULT 0,
    notes TEXT,
    created_at INTEGER NOT NULL,
    UNIQUE(host_key, cookie_name)
);

CREATE INDEX idx_cookie_metadata_host ON cookie_metadata(host_key);
CREATE INDEX idx_cookie_metadata_blocked ON cookie_metadata(blocked);
```

### Cookie Preferences Table

Store user preferences for cookie handling:

```sql
CREATE TABLE cookie_preferences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain TEXT UNIQUE NOT NULL,
    block_all INTEGER DEFAULT 0,
    block_third_party INTEGER DEFAULT 0,
    allow_session_only INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_cookie_preferences_domain ON cookie_preferences(domain);
```

## Implementation Architecture

### Layer 1: CEF Native Cookie Access

**File**: `cef-native/include/handlers/CookieManager.h`

```cpp
#ifndef COOKIE_MANAGER_H
#define COOKIE_MANAGER_H

#include "include/cef_cookie.h"
#include <vector>
#include <string>
#include <functional>

struct CookieInfo {
    std::string name;
    std::string value;
    std::string domain;
    std::string path;
    int64_t creation_time;
    int64_t expiry_time;
    int64_t last_access_time;
    bool secure;
    bool http_only;
    int same_site;
    int priority;
};

class CookieVisitor : public CefCookieVisitor {
public:
    using CookieCallback = std::function<void(const std::vector<CookieInfo>&)>;

    explicit CookieVisitor(CookieCallback callback);

    bool Visit(const CefCookie& cookie,
               int count,
               int total,
               bool& deleteCookie) override;

private:
    std::vector<CookieInfo> cookies_;
    CookieCallback callback_;

    IMPLEMENT_REFCOUNTING(CookieVisitor);
};

class HodosCookieManager {
public:
    static HodosCookieManager& GetInstance();

    void GetAllCookies(CookieVisitor::CookieCallback callback);
    void GetCookiesForUrl(const std::string& url,
                          bool includeHttpOnly,
                          CookieVisitor::CookieCallback callback);

    void DeleteCookie(const std::string& url,
                      const std::string& cookieName);
    void DeleteAllCookies();
    void DeleteCookiesForDomain(const std::string& domain);

    void SetCookie(const std::string& url, const CookieInfo& cookie);

private:
    HodosCookieManager() = default;
    CefRefPtr<CefCookieManager> GetCefCookieManager();
};

#endif // COOKIE_MANAGER_H
```

**File**: `cef-native/src/handlers/CookieManager.cpp`

```cpp
#include "include/handlers/CookieManager.h"
#include "include/cef_base.h"
#include <nlohmann/json.hpp>

using json = nlohmann::json;

CookieVisitor::CookieVisitor(CookieCallback callback)
    : callback_(callback) {}

bool CookieVisitor::Visit(const CefCookie& cookie,
                          int count,
                          int total,
                          bool& deleteCookie) {
    CookieInfo info;
    info.name = CefString(&cookie.name).ToString();
    info.value = CefString(&cookie.value).ToString();
    info.domain = CefString(&cookie.domain).ToString();
    info.path = CefString(&cookie.path).ToString();
    info.creation_time = cookie.creation.val;
    info.expiry_time = cookie.expires.val;
    info.last_access_time = cookie.last_access.val;
    info.secure = cookie.secure ? true : false;
    info.http_only = cookie.httponly ? true : false;
    info.same_site = static_cast<int>(cookie.same_site);
    info.priority = static_cast<int>(cookie.priority);

    cookies_.push_back(info);

    if (count == total - 1) {
        // Last cookie visited, invoke callback
        callback_(cookies_);
    }

    deleteCookie = false;
    return true;
}

HodosCookieManager& HodosCookieManager::GetInstance() {
    static HodosCookieManager instance;
    return instance;
}

CefRefPtr<CefCookieManager> HodosCookieManager::GetCefCookieManager() {
    return CefCookieManager::GetGlobalManager(nullptr);
}

void HodosCookieManager::GetAllCookies(CookieVisitor::CookieCallback callback) {
    CefRefPtr<CefCookieManager> manager = GetCefCookieManager();
    if (!manager) {
        callback({});
        return;
    }

    CefRefPtr<CookieVisitor> visitor = new CookieVisitor(callback);
    manager->VisitAllCookies(visitor);
}

void HodosCookieManager::GetCookiesForUrl(const std::string& url,
                                           bool includeHttpOnly,
                                           CookieVisitor::CookieCallback callback) {
    CefRefPtr<CefCookieManager> manager = GetCefCookieManager();
    if (!manager) {
        callback({});
        return;
    }

    CefRefPtr<CookieVisitor> visitor = new CookieVisitor(callback);
    manager->VisitUrlCookies(CefString(url), includeHttpOnly, visitor);
}

void HodosCookieManager::DeleteCookie(const std::string& url,
                                       const std::string& cookieName) {
    CefRefPtr<CefCookieManager> manager = GetCefCookieManager();
    if (!manager) return;

    manager->DeleteCookies(CefString(url), CefString(cookieName), nullptr);
}

void HodosCookieManager::DeleteAllCookies() {
    CefRefPtr<CefCookieManager> manager = GetCefCookieManager();
    if (!manager) return;

    manager->DeleteCookies(CefString(), CefString(), nullptr);
}

void HodosCookieManager::DeleteCookiesForDomain(const std::string& domain) {
    CefRefPtr<CefCookieManager> manager = GetCefCookieManager();
    if (!manager) return;

    std::string url = "http://" + domain;
    manager->DeleteCookies(CefString(url), CefString(), nullptr);
}

void HodosCookieManager::SetCookie(const std::string& url, const CookieInfo& cookie) {
    CefRefPtr<CefCookieManager> manager = GetCefCookieManager();
    if (!manager) return;

    CefCookie cef_cookie;
    CefString(&cef_cookie.name).FromString(cookie.name);
    CefString(&cef_cookie.value).FromString(cookie.value);
    CefString(&cef_cookie.domain).FromString(cookie.domain);
    CefString(&cef_cookie.path).FromString(cookie.path);
    cef_cookie.secure = cookie.secure ? 1 : 0;
    cef_cookie.httponly = cookie.http_only ? 1 : 0;
    cef_cookie.has_expires = 1;
    cef_cookie.expires.val = cookie.expiry_time;
    cef_cookie.same_site = static_cast<cef_cookie_same_site_t>(cookie.same_site);
    cef_cookie.priority = static_cast<cef_cookie_priority_t>(cookie.priority);

    manager->SetCookie(CefString(url), cef_cookie, nullptr);
}
```

### Layer 2: Rust Backend API

**File**: `rust-wallet/src/database/cookie_metadata_repo.rs`

```rust
use rusqlite::{params, Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieMetadata {
    pub id: Option<i64>,
    pub host_key: String,
    pub cookie_name: String,
    pub blocked: bool,
    pub whitelist: bool,
    pub notes: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookiePreference {
    pub id: Option<i64>,
    pub domain: String,
    pub block_all: bool,
    pub block_third_party: bool,
    pub allow_session_only: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct CookieMetadataRepository {
    db_path: String,
}

impl CookieMetadataRepository {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

    pub fn initialize(&self) -> SqliteResult<()> {
        let conn = Connection::open(&self.db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cookie_metadata (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                host_key TEXT NOT NULL,
                cookie_name TEXT NOT NULL,
                blocked INTEGER DEFAULT 0,
                whitelist INTEGER DEFAULT 0,
                notes TEXT,
                created_at INTEGER NOT NULL,
                UNIQUE(host_key, cookie_name)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cookie_preferences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                domain TEXT UNIQUE NOT NULL,
                block_all INTEGER DEFAULT 0,
                block_third_party INTEGER DEFAULT 0,
                allow_session_only INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cookie_metadata_host ON cookie_metadata(host_key)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cookie_preferences_domain ON cookie_preferences(domain)",
            [],
        )?;

        Ok(())
    }

    pub fn add_metadata(
        &self,
        host_key: &str,
        cookie_name: &str,
        blocked: bool,
        whitelist: bool,
        notes: Option<&str>,
    ) -> SqliteResult<i64> {
        let conn = Connection::open(&self.db_path)?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT OR REPLACE INTO cookie_metadata
             (host_key, cookie_name, blocked, whitelist, notes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                host_key,
                cookie_name,
                if blocked { 1 } else { 0 },
                if whitelist { 1 } else { 0 },
                notes,
                now
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_metadata(&self, host_key: &str) -> SqliteResult<Vec<CookieMetadata>> {
        let conn = Connection::open(&self.db_path)?;

        let mut stmt = conn.prepare(
            "SELECT id, host_key, cookie_name, blocked, whitelist, notes, created_at
             FROM cookie_metadata
             WHERE host_key = ?1"
        )?;

        let metadata: Vec<CookieMetadata> = stmt
            .query_map(params![host_key], |row| {
                Ok(CookieMetadata {
                    id: Some(row.get(0)?),
                    host_key: row.get(1)?,
                    cookie_name: row.get(2)?,
                    blocked: row.get::<_, i32>(3)? != 0,
                    whitelist: row.get::<_, i32>(4)? != 0,
                    notes: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(metadata)
    }

    pub fn set_preference(
        &self,
        domain: &str,
        block_all: bool,
        block_third_party: bool,
        allow_session_only: bool,
    ) -> SqliteResult<()> {
        let conn = Connection::open(&self.db_path)?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT OR REPLACE INTO cookie_preferences
             (domain, block_all, block_third_party, allow_session_only, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                domain,
                if block_all { 1 } else { 0 },
                if block_third_party { 1 } else { 0 },
                if allow_session_only { 1 } else { 0 },
                now
            ],
        )?;

        Ok(())
    }

    pub fn get_preference(&self, domain: &str) -> SqliteResult<Option<CookiePreference>> {
        let conn = Connection::open(&self.db_path)?;

        let result = conn.query_row(
            "SELECT id, domain, block_all, block_third_party, allow_session_only, created_at, updated_at
             FROM cookie_preferences
             WHERE domain = ?1",
            params![domain],
            |row| {
                Ok(CookiePreference {
                    id: Some(row.get(0)?),
                    domain: row.get(1)?,
                    block_all: row.get::<_, i32>(2)? != 0,
                    block_third_party: row.get::<_, i32>(3)? != 0,
                    allow_session_only: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        );

        match result {
            Ok(pref) => Ok(Some(pref)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_all_preferences(&self) -> SqliteResult<Vec<CookiePreference>> {
        let conn = Connection::open(&self.db_path)?;

        let mut stmt = conn.prepare(
            "SELECT id, domain, block_all, block_third_party, allow_session_only, created_at, updated_at
             FROM cookie_preferences
             ORDER BY domain ASC"
        )?;

        let prefs: Vec<CookiePreference> = stmt
            .query_map([], |row| {
                Ok(CookiePreference {
                    id: Some(row.get(0)?),
                    domain: row.get(1)?,
                    block_all: row.get::<_, i32>(2)? != 0,
                    block_third_party: row.get::<_, i32>(3)? != 0,
                    allow_session_only: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(prefs)
    }

    pub fn delete_preference(&self, domain: &str) -> SqliteResult<()> {
        let conn = Connection::open(&self.db_path)?;

        conn.execute(
            "DELETE FROM cookie_preferences WHERE domain = ?1",
            params![domain],
        )?;

        Ok(())
    }
}
```

**File**: `rust-wallet/src/handlers.rs` (add cookie endpoints)

```rust
#[derive(Serialize)]
pub struct CookieResponse {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub creation_time: i64,
    pub expiry_time: i64,
    pub last_access_time: i64,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: i32,
}

#[derive(Deserialize)]
pub struct CookiePreferenceRequest {
    pub domain: String,
    pub block_all: bool,
    pub block_third_party: bool,
    pub allow_session_only: bool,
}

pub async fn get_cookie_preferences(
    cookie_metadata_repo: web::Data<CookieMetadataRepository>,
) -> ActixResult<HttpResponse> {
    match cookie_metadata_repo.get_all_preferences() {
        Ok(preferences) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "preferences": preferences
        }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))),
    }
}

pub async fn set_cookie_preference(
    data: web::Json<CookiePreferenceRequest>,
    cookie_metadata_repo: web::Data<CookieMetadataRepository>,
) -> ActixResult<HttpResponse> {
    match cookie_metadata_repo.set_preference(
        &data.domain,
        data.block_all,
        data.block_third_party,
        data.allow_session_only,
    ) {
        Ok(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "success": true
        }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))),
    }
}

pub async fn delete_cookie_preference(
    path: web::Path<String>,
    cookie_metadata_repo: web::Data<CookieMetadataRepository>,
) -> ActixResult<HttpResponse> {
    let domain = path.into_inner();

    match cookie_metadata_repo.delete_preference(&domain) {
        Ok(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "success": true
        }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))),
    }
}
```

**Endpoint Registration**:

```rust
.route("/cookies/preferences", web::get().to(get_cookie_preferences))
.route("/cookies/preferences", web::post().to(set_cookie_preference))
.route("/cookies/preferences/{domain}", web::delete().to(delete_cookie_preference))
```

### Layer 3: Frontend TypeScript Implementation

**File**: `frontend/src/types/cookie.d.ts`

```typescript
export interface Cookie {
  name: string;
  value: string;
  domain: string;
  path: string;
  creation_time: number;
  expiry_time: number;
  last_access_time: number;
  secure: boolean;
  http_only: boolean;
  same_site: number;
}

export interface CookiePreference {
  domain: string;
  block_all: boolean;
  block_third_party: boolean;
  allow_session_only: boolean;
  created_at?: number;
  updated_at?: number;
}
```

**File**: `frontend/src/bridge/initWindowBridge.ts` (add cookie functions)

```typescript
window.hodosBrowser.cookies = {
  getAll: async (): Promise<Cookie[]> => {
    return new Promise((resolve, reject) => {
      window.cefMessage.send('get_all_cookies', []);
      window.onGetAllCookiesResponse = (data: any) => {
        if (data.success) {
          resolve(data.cookies);
        } else {
          reject(new Error(data.error || 'Failed to get cookies'));
        }
      };
    });
  },

  getForUrl: async (url: string): Promise<Cookie[]> => {
    return new Promise((resolve, reject) => {
      window.cefMessage.send('get_cookies_for_url', [{ url }]);
      window.onGetCookiesForUrlResponse = (data: any) => {
        if (data.success) {
          resolve(data.cookies);
        } else {
          reject(new Error(data.error || 'Failed to get cookies for URL'));
        }
      };
    });
  },

  delete: async (url: string, name: string): Promise<boolean> => {
    return new Promise((resolve, reject) => {
      window.cefMessage.send('delete_cookie', [{ url, name }]);
      window.onDeleteCookieResponse = (data: any) => {
        if (data.success) {
          resolve(true);
        } else {
          reject(new Error(data.error || 'Failed to delete cookie'));
        }
      };
    });
  },

  deleteAll: async (): Promise<boolean> => {
    return new Promise((resolve, reject) => {
      window.cefMessage.send('delete_all_cookies', []);
      window.onDeleteAllCookiesResponse = (data: any) => {
        if (data.success) {
          resolve(true);
        } else {
          reject(new Error(data.error || 'Failed to delete all cookies'));
        }
      };
    });
  },

  deleteForDomain: async (domain: string): Promise<boolean> => {
    return new Promise((resolve, reject) => {
      window.cefMessage.send('delete_cookies_for_domain', [{ domain }]);
      window.onDeleteCookiesForDomainResponse = (data: any) => {
        if (data.success) {
          resolve(true);
        } else {
          reject(new Error(data.error || 'Failed to delete cookies for domain'));
        }
      };
    });
  },

  getPreferences: async (): Promise<CookiePreference[]> => {
    return new Promise((resolve, reject) => {
      window.cefMessage.send('get_cookie_preferences', []);
      window.onGetCookiePreferencesResponse = (data: any) => {
        if (data.success) {
          resolve(data.preferences);
        } else {
          reject(new Error(data.error || 'Failed to get preferences'));
        }
      };
    });
  },

  setPreference: async (preference: CookiePreference): Promise<boolean> => {
    return new Promise((resolve, reject) => {
      window.cefMessage.send('set_cookie_preference', [preference]);
      window.onSetCookiePreferenceResponse = (data: any) => {
        if (data.success) {
          resolve(true);
        } else {
          reject(new Error(data.error || 'Failed to set preference'));
        }
      };
    });
  }
};
```

**File**: `frontend/src/components/CookieManager.tsx`

```typescript
import React, { useState, useEffect } from 'react';
import {
  Box,
  Typography,
  List,
  ListItem,
  ListItemText,
  IconButton,
  Button,
  Chip,
  TextField,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Switch,
  FormControlLabel,
  Accordion,
  AccordionSummary,
  AccordionDetails
} from '@mui/material';
import { Delete, ExpandMore, Shield, Lock } from '@mui/icons-material';
import { Cookie, CookiePreference } from '../types/cookie';

export function CookieManager() {
  const [cookies, setCookies] = useState<Cookie[]>([]);
  const [groupedCookies, setGroupedCookies] = useState<Map<string, Cookie[]>>(new Map());
  const [preferences, setPreferences] = useState<CookiePreference[]>([]);
  const [searchTerm, setSearchTerm] = useState('');
  const [dialogOpen, setDialogOpen] = useState(false);
  const [selectedDomain, setSelectedDomain] = useState<string | null>(null);

  useEffect(() => {
    loadCookies();
    loadPreferences();
  }, []);

  const loadCookies = async () => {
    try {
      const allCookies = await window.hodosBrowser.cookies.getAll();
      setCookies(allCookies);

      // Group by domain
      const grouped = new Map<string, Cookie[]>();
      allCookies.forEach(cookie => {
        const domain = cookie.domain;
        if (!grouped.has(domain)) {
          grouped.set(domain, []);
        }
        grouped.get(domain)!.push(cookie);
      });
      setGroupedCookies(grouped);
    } catch (err) {
      console.error('Failed to load cookies:', err);
    }
  };

  const loadPreferences = async () => {
    try {
      const prefs = await window.hodosBrowser.cookies.getPreferences();
      setPreferences(prefs);
    } catch (err) {
      console.error('Failed to load preferences:', err);
    }
  };

  const handleDeleteCookie = async (domain: string, name: string) => {
    try {
      const url = `http://${domain}`;
      await window.hodosBrowser.cookies.delete(url, name);
      await loadCookies();
    } catch (err) {
      console.error('Failed to delete cookie:', err);
    }
  };

  const handleDeleteAll = async () => {
    if (confirm('Are you sure you want to delete all cookies?')) {
      try {
        await window.hodosBrowser.cookies.deleteAll();
        await loadCookies();
      } catch (err) {
        console.error('Failed to delete all cookies:', err);
      }
    }
  };

  const handleDeleteDomain = async (domain: string) => {
    try {
      await window.hodosBrowser.cookies.deleteForDomain(domain);
      await loadCookies();
    } catch (err) {
      console.error('Failed to delete domain cookies:', err);
    }
  };

  const filteredDomains = Array.from(groupedCookies.keys()).filter(domain =>
    domain.toLowerCase().includes(searchTerm.toLowerCase())
  );

  const formatDate = (timestamp: number) => {
    if (timestamp === 0) return 'Session';
    const date = new Date(timestamp / 1000);
    return date.toLocaleString();
  };

  return (
    <Box sx={{ p: 2 }}>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
        <Typography variant="h5">Cookie Manager</Typography>
        <Button
          variant="contained"
          color="error"
          onClick={handleDeleteAll}
        >
          Delete All Cookies
        </Button>
      </Box>

      <TextField
        fullWidth
        placeholder="Search by domain..."
        value={searchTerm}
        onChange={(e) => setSearchTerm(e.target.value)}
        sx={{ mb: 2 }}
      />

      <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
        Total: {cookies.length} cookies across {groupedCookies.size} domains
      </Typography>

      {filteredDomains.map(domain => (
        <Accordion key={domain}>
          <AccordionSummary expandIcon={<ExpandMore />}>
            <Box sx={{ display: 'flex', alignItems: 'center', width: '100%', justifyContent: 'space-between' }}>
              <Box sx={{ display: 'flex', alignItems: 'center' }}>
                <Typography sx={{ mr: 2 }}>{domain}</Typography>
                <Chip
                  label={`${groupedCookies.get(domain)?.length || 0} cookies`}
                  size="small"
                />
              </Box>
              <Button
                size="small"
                color="error"
                onClick={(e) => {
                  e.stopPropagation();
                  handleDeleteDomain(domain);
                }}
              >
                Delete All
              </Button>
            </Box>
          </AccordionSummary>
          <AccordionDetails>
            <List>
              {groupedCookies.get(domain)?.map((cookie, index) => (
                <ListItem
                  key={`${cookie.name}-${index}`}
                  secondaryAction={
                    <IconButton edge="end" onClick={() => handleDeleteCookie(domain, cookie.name)}>
                      <Delete />
                    </IconButton>
                  }
                >
                  <ListItemText
                    primary={
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                        {cookie.name}
                        {cookie.secure && <Lock fontSize="small" color="primary" />}
                        {cookie.http_only && <Shield fontSize="small" color="action" />}
                      </Box>
                    }
                    secondary={
                      <>
                        <Typography component="span" variant="body2">
                          Value: {cookie.value.substring(0, 50)}{cookie.value.length > 50 ? '...' : ''}
                        </Typography>
                        <br />
                        <Typography component="span" variant="caption" color="text.secondary">
                          Path: {cookie.path} • Expires: {formatDate(cookie.expiry_time)}
                        </Typography>
                      </>
                    }
                  />
                </ListItem>
              ))}
            </List>
          </AccordionDetails>
        </Accordion>
      ))}
    </Box>
  );
}
```

## Implementation Steps

### Phase 1: CEF Integration
1. Implement CookieManager class in C++
2. Create CookieVisitor for cookie enumeration
3. Expose cookie functions to JavaScript bridge
4. Test cookie retrieval

### Phase 2: Backend Metadata
1. Create cookie metadata repository
2. Implement preference storage
3. Add HTTP endpoints
4. Test preference management

### Phase 3: Frontend UI
1. Create TypeScript types
2. Extend bridge API
3. Build CookieManager component
4. Implement domain grouping
5. Add search and filter

### Phase 4: Privacy Features
1. Implement cookie blocking
2. Add whitelist functionality
3. Create preference UI
4. Test blocking logic

### Phase 5: Advanced Features
1. Export cookie data
2. Import cookies
3. Cookie analytics
4. Privacy dashboard

## Features

### Core Features
- View all cookies
- Group cookies by domain
- Delete individual cookies
- Delete all cookies for a domain
- Delete all cookies
- Search and filter cookies

### Privacy Features
- Block third-party cookies
- Session-only cookies
- Domain-specific preferences
- Cookie whitelist/blacklist
- Secure/HttpOnly indicators

### Advanced Features
- Cookie expiration tracking
- Cookie size analysis
- Domain statistics
- Privacy score
- Export/import cookies

## Performance Optimization

- Lazy load cookie lists
- Virtual scrolling for large lists
- Indexed search
- Cached cookie counts
- Background deletion

## Security Considerations

- HttpOnly cookies hidden from JavaScript
- Secure cookie indicators
- Privacy mode handling
- Cookie encryption (CEF built-in)
- Third-party cookie blocking

## Testing Strategy

### Unit Tests
- Cookie enumeration
- Deletion logic
- Preference storage

### Integration Tests
- CEF cookie manager integration
- End-to-end cookie operations
- Privacy feature testing

### UI Tests
- Component rendering
- User interactions
- Error handling

## Privacy Enhancements

### Cookie Consent Management
- First-party vs third-party detection
- Auto-block third-party cookies
- Per-site cookie preferences
- Cookie audit trail

### Cookie Analytics
- Track cookie usage
- Identify tracking cookies
- Privacy score per domain
- Cookie lifetime analysis

## Future Enhancements

1. Cookie import/export (JSON/Netscape format)
2. Cookie sync across devices
3. Advanced filtering (by expiry, type, size)
4. Cookie timeline visualization
5. Privacy recommendations
6. Automatic cookie cleanup
7. Cookie whitelist templates
8. Integration with privacy extensions
