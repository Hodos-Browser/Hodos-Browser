# File & Database Inventory

**Created**: 2026-02-19
**Status**: Complete (Phase A.2)
**Purpose**: Comprehensive catalog of everything the browser creates at runtime — persistent files, databases, in-memory singletons, and frontend storage.

---

## 1. Directory Layout at Runtime

```
%APPDATA%/HodosBrowser/                          ← Root (set by CefSettings.root_cache_path)
├── wallet/
│   ├── wallet.db                                ← Rust: SQLite, sole wallet storage
│   ├── wallet.db-wal                            ← SQLite WAL (auto-managed)
│   └── wallet.db-shm                            ← SQLite shared memory (auto-managed)
│
├── Default/                                     ← CEF profile (CefSettings.cache_path)
│   ├── Cookies                                  ← CEF: cookie jar (SQLite)
│   ├── History                                  ← CEF: Chromium browsing history (auto)
│   ├── Web Data                                 ← CEF: form/autocomplete data (auto)
│   ├── HodosHistory                             ← C++: custom history DB (SQLite WAL)
│   ├── bookmarks.db                             ← C++: custom bookmarks DB (SQLite WAL)
│   ├── cookie_blocks.db                         ← C++: cookie blocking rules (SQLite WAL)
│   ├── Cache/                                   ← CEF: HTTP cache (auto-managed)
│   ├── Session Storage/                         ← CEF: session/local storage blobs
│   ├── Service Workers/                         ← CEF: service worker storage
│   ├── GPUCache/                                ← CEF: GPU shader cache
│   └── debug.log                                ← CEF: internal log
│
└── (no other directories)

<executable_dir>/
└── debug_output.log                             ← C++: Logger singleton, append mode
```

---

## 2. Persistent Databases — Detailed

### 2.1 Rust: wallet.db

| Property | Value |
|----------|-------|
| **Path** | `%APPDATA%/HodosBrowser/wallet/wallet.db` |
| **Schema Version** | V4 (V1 consolidated + V2/V3/V4 incremental) |
| **Created By** | `WalletDatabase::new()` in `database/connection.rs` |
| **WAL Mode** | Enabled |
| **Foreign Keys** | Enabled |
| **Busy Timeout** | 5 seconds |
| **Lock** | `Arc<Mutex<WalletDatabase>>` — single writer |

**Tables (28 active)**:

| Table | Purpose | Added |
|-------|---------|-------|
| `schema_version` | Migration tracking | V1 |
| `wallets` | Master key (mnemonic, PIN salt, DPAPI blob) | V1 |
| `addresses` | HD-derived addresses cache | V1 |
| `users` | Multi-user identity (userId, identity_key) | V1 (V17 origin) |
| `transactions` | TX records (single `status` column) | V1 |
| `transaction_inputs` | TX input data | V1 |
| `transaction_outputs` | TX output data | V1 |
| `outputs` | Primary UTXO tracking (replaces legacy `utxos`) | V1 (V18 origin) |
| `output_baskets` | Output categorization (was `baskets`) | V1 |
| `output_tags` | Output tagging | V1 |
| `output_tag_map` | Tag ↔ output associations | V1 |
| `tx_labels` | Transaction labeling | V1 (V19 origin) |
| `tx_labels_map` | Label ↔ TX associations | V1 |
| `proven_txs` | Immutable merkle proof records (BLOB) | V1 (V16 origin) |
| `proven_tx_reqs` | Proof acquisition lifecycle | V1 |
| `certificates` | BRC-52 identity certificates | V1 |
| `certificate_fields` | Certificate field data | V1 |
| `parent_transactions` | Raw TX cache for BEEF building | V1 |
| `block_headers` | Cached block headers | V1 |
| `messages` | BRC-33 message relay (local) | V1 |
| `relay_messages` | BRC-33 relay storage | V1 |
| `settings` | Persistent configuration | V1 |
| `sync_states` | Multi-device sync state | V1 |
| `commissions` | Fee tracking per transaction | V1 |
| `monitor_events` | Background task event logging | V1 |
| `derived_key_cache` | PushDrop signing cache | V1 |
| `domain_permissions` | Approved domains + spending limits (USD cents) | V3 |
| `cert_field_permissions` | Certificate field disclosure permissions | V3 |

**Dead tables (NOT created in V1 fresh-DB)**:
- `utxos` (replaced by `outputs`)
- `merkle_proofs` (replaced by `proven_txs`)
- `transaction_labels` (replaced by `tx_labels`)
- `domain_whitelist` (replaced by `domain_permissions`)

### 2.2 C++: HodosHistory

| Property | Value |
|----------|-------|
| **Path** | `%APPDATA%/HodosBrowser/Default/HodosHistory` |
| **Created By** | `HistoryManager::Initialize()` in `cef_browser_shell.cpp` |
| **WAL Mode** | Enabled |
| **Busy Timeout** | 5 seconds |

**Tables**:
- `urls` — id, url, title, visit_count, last_visit_time
- `visits` — id, url, visit_time, transition

### 2.3 C++: bookmarks.db

| Property | Value |
|----------|-------|
| **Path** | `%APPDATA%/HodosBrowser/Default/bookmarks.db` |
| **Created By** | `BookmarkManager::Initialize()` in `cef_browser_shell.cpp` |
| **WAL Mode** | Enabled |
| **Foreign Keys** | Enabled |

**Tables**:
- `bookmark_folders` — id, name, parent_id, position, created_at
- `bookmarks` — id, url, title, folder_id, favicon_url, position, created_at
- `bookmark_tags` — id, bookmark_id, tag

**Default data**: Auto-creates "Favorites" root folder on first init.

### 2.4 C++: cookie_blocks.db

| Property | Value |
|----------|-------|
| **Path** | `%APPDATA%/HodosBrowser/Default/cookie_blocks.db` |
| **Created By** | `CookieBlockManager::Initialize()` in `cef_browser_shell.cpp` |
| **Thread Safety** | `shared_mutex` (read-heavy workload) |

**Tables**:
- `meta` — key, value
- `blocked_domains` — id, domain, is_wildcard, source, created_at
- `allowed_third_party` — id, domain
- `block_log` — id, cookie_domain, page_url, reason, blocked_at

**Default data**: Populates from `DefaultTrackerList.h` on first run.
**In-memory cache**: Loads blocked domains into `unordered_set` + wildcard suffixes for O(1) IO-thread lookups.

### 2.5 CEF-Managed (Auto-Created)

| Database/Dir | Purpose | Notes |
|-------------|---------|-------|
| `Cookies` | Session/persistent cookies | SQLite, CEF-managed |
| `History` | Chromium browsing history | CEF auto-creates (separate from HodosHistory) |
| `Web Data` | Form data, autocomplete | CEF-managed |
| `Cache/` | HTTP response cache | File-based, CEF eviction policies |
| `Session Storage/` | localStorage/sessionStorage blobs | LevelDB-backed |
| `Service Workers/` | SW registration + cache | CEF-managed |
| `GPUCache/` | Compiled shader cache | CEF-managed |

---

## 3. Log Files

| File | Location | Created By | Mode |
|------|----------|-----------|------|
| `debug_output.log` | Executable directory | `Logger::Initialize()` (C++ singleton) | Append, flushed per write |
| `debug.log` | `Default/` | CEF settings | CEF-managed |

---

## 4. User-Created Files (Backup/Export)

| File | Extension | Created By | Trigger |
|------|-----------|-----------|---------|
| Encrypted backup | `.hodos-wallet` | `backup.rs` | `POST /wallet/backup` |
| JSON export | `.json` | `backup.rs` | `POST /wallet/export` (non-sensitive data) |

---

## 5. C++ In-Memory Singletons

All thread-safe, no persistent storage. Backed by REST calls to Rust or CEF APIs.

| Singleton | File | State | TTL | Backend |
|-----------|------|-------|-----|---------|
| `DomainPermissionCache` | `HttpRequestInterceptor.cpp:51` | `map<domain, Permission>` | Until invalidated | WinHTTP → `/domain/permissions` |
| `PendingRequestManager` | `PendingAuthRequest.h:20` | `map<requestId, PendingAuthRequest>` | Request lifecycle | N/A (in-flight requests) |
| `SessionManager` | `SessionManager.h:11` | `map<browserId, BrowserSession>` | Per-tab lifecycle | N/A (spending/rate tracking) |
| `BSVPriceCache` | `HttpRequestInterceptor.cpp:287` | `double priceUsd_` | 5 minutes | WinHTTP → `/wallet/bsv-price` |
| `WalletStatusCache` | `HttpRequestInterceptor.cpp:188` | `bool exists_` | 30 seconds | WinHTTP → `/wallet/status` |
| `TabManager` | `TabManager.h:36` | `map<tabId, Tab*>` | App lifetime | N/A |
| `GoogleSuggestService` | `GoogleSuggestService.h` | WinHTTP session handle | App lifetime | Google Suggest API |
| `HistoryManager` | `HistoryManager.h` | SQLite handle | App lifetime | HodosHistory DB |
| `BookmarkManager` | `BookmarkManager.h` | SQLite handle | App lifetime | bookmarks.db |
| `CookieBlockManager` | `CookieBlockManager.h` | SQLite + in-memory sets | App lifetime | cookie_blocks.db |

---

## 6. Rust In-Memory Caches (AppState)

```rust
pub struct AppState {
    pub database: Arc<Mutex<WalletDatabase>>,           // SQLite connection
    pub message_store: MessageStore,                     // BRC-33 relay (in-memory)
    pub auth_sessions: Arc<AuthSessionManager>,          // BRC-103/104 auth state
    pub balance_cache: Arc<BalanceCache>,                // Satoshi balance (RwLock<i64>)
    pub fee_rate_cache: Arc<FeeRateCache>,               // Sats/kB from ARC (1hr TTL)
    pub price_cache: Arc<PriceCache>,                    // BSV/USD (5min TTL, CryptoCompare+CoinGecko)
    pub utxo_selection_lock: Arc<tokio::sync::Mutex<()>>,// Prevents concurrent UTXO selection
    pub create_action_lock: Arc<tokio::sync::Mutex<()>>, // Serializes createAction flow
    pub derived_key_cache: Arc<Mutex<HashMap<...>>>,     // PushDrop signing params
    pub current_user_id: i64,                            // Default user (multi-user foundation)
    pub shutdown: CancellationToken,                     // Graceful shutdown signal
    pub sync_status: Arc<RwLock<SyncStatus>>,            // Recovery progress
}
```

None of these caches write to disk. `wallet.db` is the sole persistent storage.

---

## 7. Frontend Storage (localStorage)

| Key | Data | TTL | Written By | Read By |
|-----|------|-----|-----------|---------|
| `hodos:wallet:balance` | `{ balance: number, updatedAt: ts }` | 60s | `useBalance`, `useBackgroundBalancePoller` | WalletPanel, TransactionForm |
| `hodos:wallet:bsvPrice` | `{ price: number, updatedAt: ts }` | 600s | `useBalance`, `useBackgroundBalancePoller` | TransactionForm |
| `hodos_wallet_exists` | `'true'` | Session | `WalletPanelPage`, `MainBrowserView` | Quick-init (avoids `/wallet/status` call) |

**Design note**: localStorage is shared across CEF subprocesses (same origin). Balance/price cached here so wallet overlay subprocess has instant data on open. Main browser process runs `useBackgroundBalancePoller` to keep cache warm.

**No IndexedDB or sessionStorage used.**

---

## 8. Frontend → Backend Communication

### 8.1 REST Endpoints Called (fetch → localhost:3301)

**Wallet Management**: `/wallet/create`, `/wallet/recover`, `/wallet/import`, `/wallet/recover-external`, `/wallet/unlock`, `/wallet/status`, `/wallet/sync-status`, `/wallet/sync-status/seen`, `/wallet/backup`, `/wallet/export`, `/wallet/addresses`

**BRC-100 Operations**: `/listActions`, `/listCertificates`

**Domain Permissions**: `GET/POST/DELETE /domain/permissions`, `GET /domain/permissions/all`, `GET/POST /domain/permissions/certificate`

### 8.2 cefMessage.send() IPC Types

**Overlay Control**: `overlay_close`, `overlay_show_settings`, `overlay_show_backup`, `settings_close`, `cookie_panel_show`, `cookie_panel_hide`

**Tabs**: `tab_create`, `tab_close`, `tab_switch`, `get_tab_list`

**Navigation**: `navigate`, `navigate_back`, `navigate_forward`, `navigate_reload`

**BRC-100 Auth**: `brc100_auth_response`, `add_domain_permission`, `add_domain_permission_advanced`, `approve_cert_fields`

**Omnibox**: `omnibox_create`, `omnibox_show`, `omnibox_hide`, `omnibox_update_query`, `omnibox_autocomplete`

**Cookies**: `cookie_get_all`, `cookie_delete`, `cookie_delete_domain`, `cookie_delete_all`, `cache_clear`, `cache_get_size`, `cookie_block_domain`, `cookie_unblock_domain`, `cookie_get_blocklist`, `cookie_allow_third_party`, `cookie_remove_third_party_allow`, `cookie_get_block_log`, `cookie_clear_block_log`, `cookie_get_blocked_count`, `cookie_reset_blocked_count`

**Bookmarks**: `bookmark_add`, `bookmark_get`, `bookmark_update`, `bookmark_remove`, `bookmark_search`, `bookmark_get_all`, `bookmark_is_bookmarked`, `bookmark_get_all_tags`, `bookmark_update_last_accessed`, `bookmark_folder_create`, `bookmark_folder_list`, `bookmark_folder_update`, `bookmark_folder_remove`, `bookmark_folder_get_tree`

**Other**: `toggle_wallet_panel`, `mark_wallet_backed_up`

### 8.3 React Routes (CEF Subprocess Pages)

| Route | Component | Context |
|-------|-----------|---------|
| `/` | `MainBrowserView` | Main browser window |
| `/history` | `HistoryPage` | Tab content |
| `/wallet-panel` | `WalletPanelPage` | Overlay subprocess |
| `/settings` | `SettingsOverlayRoot` | Overlay subprocess |
| `/wallet` | `WalletOverlayRoot` | Overlay subprocess (5 tabs) |
| `/backup` | `BackupOverlayRoot` | Overlay subprocess |
| `/brc100-auth` | `BRC100AuthOverlayRoot` | Keep-alive overlay subprocess |
| `/omnibox` | `OmniboxOverlayRoot` | Overlay subprocess |
| `/cookie-panel` | `CookiePanelOverlayRoot` | Overlay subprocess |

---

## 9. Dead Code / Legacy Files

| Item | Location | Status | Reason Kept |
|------|----------|--------|-------------|
| `json_storage.rs` | `rust-wallet/src/` | Dead code | Legacy migration support only (`migration.rs`) |
| `action_storage.rs` | `rust-wallet/src/` | Dead code | Legacy BRC-100 draft; imported but never used |
| `identity.json` read | `simple_render_process_handler.cpp` | Read-only legacy | Checks for old MetanetDesktop identity; never writes |
| `AddressInfo` struct | `json_storage.rs` | Still used | Referenced by `handlers.rs` and `helpers.rs` as a data struct (should be moved) |

---

## 10. Initialization Order

### C++ (cef_browser_shell.cpp WinMain)
```
1. Logger::Initialize()          → debug_output.log
2. CefInitialize()               → Default/ profile created
3. HistoryManager::Initialize()  → HodosHistory DB
4. CookieBlockManager::Initialize() → cookie_blocks.db + default trackers
5. BookmarkManager::Initialize() → bookmarks.db + "Favorites" folder
6. StartWalletServer()           → Rust process launched + Job Object
7. CefRunMessageLoop()           → Singletons lazy-init on first use
```

### Rust (main.rs)
```
1. Get APPDATA path, create wallet/ directory
2. WalletDatabase::new()         → wallet.db (migrations run)
3. Try DPAPI unlock              → auto-cache mnemonic if possible
4. Initialize caches             → balance, fee_rate, price
5. Start HTTP server             → localhost:3301
6. Spawn Monitor                 → 7 background tasks on intervals
```

---

**End of Document**
