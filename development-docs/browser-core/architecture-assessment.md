# Architecture Cohesiveness Assessment

**Created**: 2026-02-19
**Status**: Complete (Phase A.5)
**Purpose**: Assessment of communication patterns, thread safety, global state, and architectural debt across all three layers.

---

## Executive Summary

The Hodos Browser has **strong architectural separation** across its three layers (React frontend -> C++ CEF shell -> Rust wallet backend), but has accumulated **consistency issues** and **architectural debt** from rapid Phase 2 development. The system is functionally sound but shows pattern divergence that could compound maintenance burden.

---

## 1. Three Communication Patterns

The architecture uses three different communication mechanisms:

| Mechanism | Used For | Thread Safety | Timeout | Error Handling |
|-----------|----------|---------------|---------|----------------|
| **WinHTTP (sync)** | Domain permissions, BSV price, wallet status, cert fields | `std::mutex` per singleton | 5 seconds | Silent defaults (0.0 price, "unknown" trust, false status) |
| **CefURLRequest (async)** | Wallet HTTP requests (createAction, sign, broadcast) | `CefRefPtr<>` (RAII) | 45s/60s/120s | JSON error to frontend via `compare_exchange_strong` atomic |
| **Direct fetch()** | Frontend -> Rust (bypasses C++ entirely) | tokio runtime | Implicit (HTTP) | Rust's `Result<>` + HTTP status codes |

### When Each Is Used

- **WinHTTP sync**: C++ singletons that need quick lookups on IO thread (DomainPermissionCache, BSVPriceCache, WalletStatusCache, cert field checks). Short 5s timeout is intentional — shouldn't block IO thread.
- **CefURLRequest async**: Wallet operations initiated by web content (BRC-100 auth, createAction, sign). Longer timeouts because operations involve blockchain broadcast.
- **Direct fetch()**: Frontend panels/overlays calling Rust directly for management operations (wallet status, sync, domain permissions list, export).

### Problem: Direct Frontend Calls Bypass Architecture

Frontend makes **direct fetch() calls** to `localhost:3301`, bypassing the C++ interceptor layer:

```
WalletPanelPage.tsx    -> fetch('http://localhost:3301/wallet/status')
WalletPanel.tsx        -> fetch('http://localhost:3301/wallet/sync-status')
WalletOverlayRoot.tsx  -> fetch('http://localhost:3301/listActions')
DomainPermissionsTab   -> fetch('http://localhost:3301/domain/permissions/all')
```

**Affected endpoints**: `/wallet/status`, `/wallet/sync-status`, `/wallet/sync-status/seen`, `/listActions`, `/wallet/addresses`, `/listCertificates`, `/wallet/export`, `/domain/permissions/*`

**Impact**:
- These calls skip domain permission checking (moot for internal panels, but violates the architecture principle)
- No benefit from C++ caching layer (BSVPriceCache never used)
- Inconsistent error handling across patterns
- **Mitigating factor**: All are localhost-only from trusted overlay subprocesses, so security risk is minimal

---

## 2. C++ Singleton Inventory

### Network-Backed Singletons (WinHTTP)

| Singleton | Location | Cache TTL | Failure Behavior |
|-----------|----------|-----------|------------------|
| `DomainPermissionCache` | HttpRequestInterceptor.cpp:51 | Until invalidated | Returns "unknown" (safe — shows modal) |
| `BSVPriceCache` | HttpRequestInterceptor.cpp:287 | 5 minutes | **Returns 0.0 (UNSAFE — breaks USD conversion)** |
| `WalletStatusCache` | HttpRequestInterceptor.cpp:188 | 30 seconds | Returns false (safe — suppresses wallet modal) |

### In-Memory Singletons (No Network)

| Singleton | Location | State |
|-----------|----------|-------|
| `PendingRequestManager` | PendingAuthRequest.h:20 | `map<requestId, PendingAuthRequest>` with mutex |
| `SessionManager` | SessionManager.h:19 | `map<browserId, BrowserSession>` with mutex |
| `NoWalletNotificationTracker` | HttpRequestInterceptor.cpp:528 | `set<domain>` with mutex |
| `CookieBlockManager` | CookieBlockManager.h | SQLite + in-memory sets with shared_mutex |
| `TabManager` | TabManager.h | `map<tabId, Tab*>` (UI thread only) |
| `HistoryManager` | HistoryManager.h | SQLite handle with mutex |
| `BookmarkManager` | BookmarkManager.h | SQLite handle with mutex |

---

## 3. Thread Safety Analysis

### Hot Path: IO Thread (HTTP Interception)

All access in `Open()` (IO thread) is properly protected:
1. `DomainPermissionCache::getPermission()` — mutex-protected
2. `WalletStatusCache::walletExists()` — mutex-protected
3. `SessionManager::getSession()` — mutex-protected
4. `BSVPriceCache::getPrice()` — mutex-protected
5. `CefURLRequest::Create()` — safe on any CEF thread (since CR-2)

### Cross-Thread Communication

```
IO Thread  --CefPostTask(TID_UI)--> UI Thread   (e.g., CreateNotificationOverlayTask)
UI Thread  --CefPostTask(TID_IO)--> IO Thread    (e.g., StartAsyncHTTPRequestTask)
Any Thread --CefPostDelayedTask-->  Timer->UI    (e.g., WalletTimeoutTask)
```

All cross-thread tasks use `CefRefPtr<>` for handler capture (ref-counted, safe).

### Known Issues

#### Issue 1: BSVPriceCache returns 0.0 on error (HIGH)
If CryptoCompare + CoinGecko both fail, price cache returns `0.0`. The auto-approve engine converts satoshis to USD via this price. With price=0.0, every transaction converts to $0.00 and auto-approves regardless of spending limits.

**Fix**: Cache last successful price; only return 0.0 if never fetched successfully.

#### Issue 2: SessionManager returns reference with lock released (LOW)
```cpp
BrowserSession& session = SessionManager::GetInstance().getSession(browserId, domain);
// mutex released here!
session.spentCents += 100;  // Race condition — no lock held
```
**Impact**: Spending counter can have races, but not safety-critical (worst case: off-by-one in limit check).

#### Issue 3: Unprotected global variables (LOW)

15+ unprotected globals in `cef_browser_shell.cpp`:
- `g_hwnd`, `g_header_hwnd`, `g_webview_hwnd` (HWNDs)
- `g_walletServerRunning` (bool, not atomic)
- `g_is_fullscreen` (bool, not atomic)
- `g_file_dialog_active` (bool, not atomic)

**Actual risk is low**: HWNDs are kernel handles (operations are thread-safe via Windows kernel). Boolean flags are mostly read-checked before non-critical operations. No crashes observed from these races.

---

## 4. Architectural Debt

### Debt 1: Overlapping Modal Patterns

Two patterns for showing approval modals coexist:

**Pattern A** (Original — domain approval, BRC-100 auth):
```
PendingRequestManager.addRequest() → CefPostTask → CreateNotificationOverlayTask
  → g_pendingModalDomain (legacy global) → overlay JS reads domain
```

**Pattern B** (Phase 2.3 — payment confirmation, modify limits):
```
PendingRequestManager.addRequest() → CefPostTask → AdvancedDomainPermissionTask
  → IPC with full params → overlay receives via extraParams URL
```

Pattern B is cleaner (no global state), but Pattern A persists for backward compatibility.

### Debt 2: json_storage.rs Name Misleading

`json_storage.rs` was used for pre-database storage (JSON files). Now only provides `AddressInfo` struct as a DTO for WhatsOnChain API calls. File name suggests active JSON storage which is misleading.

**Fix**: Rename to `address_dto.rs` or move struct to appropriate module.

### Debt 3: Hardcoded Configuration Values

Scattered across codebase with no central config:
- Domain permission cache timeout: 5000ms
- Wallet request timeout: 45000ms
- Recovery timeout: 120000ms
- Auth approval timeout: 60000ms
- Default per-tx limit: 10 cents ($0.10)
- Default per-session limit: 300 cents ($3.00)
- Rate limit window: 60 seconds

**Fix**: Extract to `Config.h` / `config.rs` constants.

### Debt 4: Incomplete macOS Support

WinHTTP-based singletons have `#else` stubs that return silent defaults:
- `DomainPermissionCache`: returns "unknown" (every domain shows modal)
- `BSVPriceCache`: returns 0.0 (breaks auto-approve)
- `WalletStatusCache`: returns false

No compile-time warning or runtime log for these stubs.

### Debt 5: Mixed Data Storage Responsibilities

| Data | C++ Layer | Rust Layer |
|------|-----------|------------|
| Browser history | `HodosHistory` (SQLite) | Not stored |
| Bookmarks | `bookmarks.db` (SQLite) | Not stored |
| Cookie block list | `cookie_blocks.db` (SQLite) | Not stored |
| Domain permissions | Not stored | `wallet.db` (SQLite) |
| Wallet data | Not stored | `wallet.db` (SQLite) |
| Certificates | Not stored | `wallet.db` (SQLite) |

Separation is clean but not documented. Cookie blocking list is in C++ because it needs O(1) IO-thread lookups.

---

## 5. Rust AppState (Thread Safety)

```rust
pub struct AppState {
    pub database: Arc<Mutex<WalletDatabase>>,           // Single writer
    pub message_store: MessageStore,                     // BRC-33 relay
    pub auth_sessions: Arc<AuthSessionManager>,          // BRC-103/104
    pub balance_cache: Arc<BalanceCache>,                // RwLock<i64>
    pub fee_rate_cache: Arc<FeeRateCache>,               // 1hr TTL
    pub price_cache: Arc<PriceCache>,                    // 5min TTL, RwLock
    pub utxo_selection_lock: Arc<tokio::sync::Mutex<()>>,
    pub create_action_lock: Arc<tokio::sync::Mutex<()>>,
    pub derived_key_cache: Arc<Mutex<HashMap<...>>>,
    pub current_user_id: i64,
    pub shutdown: CancellationToken,
    pub sync_status: Arc<RwLock<SyncStatus>>,
}
```

All fields are `Arc<>`-wrapped for safe concurrent access via tokio runtime. **Thread-safe by design.** Database uses single-writer `Mutex` (appropriate for SQLite WAL mode).

---

## 6. Risk Matrix

| Issue | Severity | Category | Recommendation |
|-------|----------|----------|----------------|
| BSVPriceCache returns 0.0 on error | **High** | Error Handling | Cache last successful price; add fallback |
| Direct frontend->Rust calls bypass C++ | Medium | Architecture | Accept for internal panels; document decision |
| Overlapping modal patterns | Medium | Maintainability | Consolidate to Pattern B over time |
| Hardcoded timeouts/limits | Low | Configuration | Extract to constants file |
| Global HWND state unprotected | Low | Thread Safety | Low risk (Windows kernel handles thread-safe ops) |
| SessionManager reference race | Low | Thread Safety | Store copy instead of reference |
| macOS stubs silently fail | Medium | Portability | Add compile-time or runtime warnings |
| json_storage.rs misleading name | Low | Documentation | Rename to address_dto.rs |

---

## 7. Recommendations (Prioritized)

### Immediate (Safety)
1. Fix BSVPriceCache to cache last successful price instead of returning 0.0
2. Document the three communication patterns and when to use each
3. Extract timeout/limit constants to central config

### Medium-Term (Architecture)
4. Consolidate modal patterns to single approach
5. Add macOS compile-time warnings for stub implementations
6. Rename json_storage.rs to clarify purpose

### Long-Term (Debt Reduction)
7. Evaluate routing all frontend calls through C++ bridge (adds caching, consistent error handling)
8. Create unified error response format across all communication patterns
9. Document SessionManager race condition acceptability

---

## Architecture Verdict

**Strength**: Clean three-layer separation with intentional boundaries; security model is sound (localhost-only, no key leakage).

**Weakness**: Three communication patterns evolved organically without clear guidelines. Direct frontend->Rust calls suggest the C++ bridge was becoming a development bottleneck.

**Status**: Production-viable. Would benefit from explicit architectural guidelines to prevent further debt accumulation.

---

**End of Document**
