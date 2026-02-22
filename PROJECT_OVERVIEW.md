# HodosBrowser — Architecture & Project Overview

**Last Updated**: 2026-02-19
**Status**: Active development. BRC-100 Groups A & B complete. Browser core MVP sprints in progress.

> This document consolidates the former PROJECT_OVERVIEW.md, ARCHITECTURE.md, and WALLET_ARCHITECTURE.md into a single reference.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [C++ CEF Shell](#2-c-cef-shell)
3. [Rust Wallet Backend](#3-rust-wallet-backend)
4. [React Frontend](#4-react-frontend)
5. [Communication Patterns](#5-communication-patterns)
6. [Security Architecture](#6-security-architecture)
7. [Data Storage](#7-data-storage)
8. [Background Services](#8-background-services)
9. [BRC-100 Protocol](#9-brc-100-protocol)
10. [Development Status](#10-development-status)

---

## 1. Architecture Overview

Three layers with strict separation:

```
React Frontend (Port 5137)
    | window.hodosBrowser.*
    v
C++ CEF Shell (CEF 136)
    | HTTP interception -> localhost:3301 for wallet functions
    v
Rust Wallet Backend (Port 3301)
    | Actix-web, SQLite (wallet.db)
    v
Bitcoin SV Blockchain (WhatsOnChain, GorillaPool)
```

| Layer | Tech | Responsibility |
|-------|------|----------------|
| Frontend | React, Vite, TypeScript, MUI | UI, user interactions; never handles keys or signing |
| CEF Shell | C++17, CEF 136 | Browser engine, V8 injection, HTTP interception; browser data (history, bookmarks) |
| Wallet | Rust, Actix-web, SQLite | Crypto, signing, keys, BRC-100 protocol; private keys never leave this process |

**Process-per-overlay**: Settings, Wallet Panel, Backup Modal, BRC-100 Auth, and Notification overlays each run as separate CEF subprocesses with isolated V8 contexts.

---

## 2. C++ CEF Shell

### 2.1 Process Architecture

The browser runs 9 distinct processes:

```
Main Browser Process (cef_browser_shell.cpp)
    |-- Header Browser (React UI at port 5137)
    |-- WebView Browser (external web content)
    |-- Settings Overlay (WS_POPUP, own V8)
    |-- Wallet Overlay (WS_POPUP, own V8)
    |-- Backup Modal (WS_POPUP, own V8)
    |-- BRC100 Auth Modal (WS_POPUP, own V8)
    |-- Notification Overlay (keep-alive HWND, own V8)
    |-- Rust Wallet Backend (separate process, Port 3301)
```

### 2.2 C++ Singletons

| Singleton | File | Purpose |
|-----------|------|---------|
| `DomainPermissionCache` | HttpRequestInterceptor.cpp | In-memory cache of domain trust levels, backed by Rust DB |
| `PendingRequestManager` | PendingAuthRequest.h | Thread-safe map of pending auth/payment/cert requests |
| `SessionManager` | SessionManager.h | Per-browser session spending + rate tracking |
| `BSVPriceCache` | HttpRequestInterceptor.cpp | BSV/USD price for auto-approve (5-min TTL) |
| `WalletStatusCache` | HttpRequestInterceptor.cpp | Cached wallet exists/locked status |
| `Logger` | cef_browser_shell.cpp | Singleton file logger |

### 2.3 SimpleHandler (CEF Client)

`simple_handler.cpp` implements 10 CEF handler interfaces:
- `CefClient`, `CefLifeSpanHandler`, `CefLoadHandler`, `CefDisplayHandler`
- `CefKeyboardHandler`, `CefContextMenuHandler`, `CefRequestHandler`, `CefResourceRequestHandler`
- `CefDownloadHandler`, `CefFindHandler`

Context menus: 5 context types (page, selection, link, image, editable). All custom command IDs in `MENU_ID_USER_FIRST` range — CEF built-in IDs auto-disable after `model->Clear()`.

IPC dispatch: `OnProcessMessageReceived()` handles 30+ message types from React overlays and the header.

### 2.4 HTTP Interception Flow

```
Web request -> OnBeforeResourceLoad (HttpRequestInterceptor.cpp)
  -> Is it a wallet endpoint? -> AsyncWalletResourceHandler
    -> Is domain approved? -> DomainPermissionCache check
      -> Is it a payment endpoint? -> Auto-approve engine
        -> Check rate limit (SessionManager)
        -> Parse outputs, convert sats -> USD (BSVPriceCache)
        -> Check per-tx and per-session limits
        -> Auto-approve OR show payment confirmation notification
      -> Forward to Rust (localhost:3301) via CefURLRequest on IO thread
```

### 2.5 Notification Overlay

Keep-alive HWND pattern:
- HWND created once (pre-warmed during startup), reused via JS injection
- `window.showNotification(queryString)` for instant React state update
- `window.hideNotification()` + `SW_HIDE` to dismiss
- 4 notification types: `domain_approval`, `brc100_auth`, `payment_confirmation`, `certificate_disclosure`
- Atomic `compare_exchange_strong` on timeout vs response to prevent double-fire crashes

### 2.6 Window Hierarchy (Windows)

```
Main Shell (g_hwnd)
    |-- Header (g_header_hwnd) - WS_CHILD, React UI
    |-- WebView (g_webview_hwnd) - WS_CHILD, web content
    |-- Settings Overlay (g_settings_overlay_hwnd) - WS_POPUP, layered
    |-- Wallet Overlay (g_wallet_overlay_hwnd) - WS_POPUP, layered
    |-- Backup Modal (g_backup_overlay_hwnd) - WS_POPUP, layered
    |-- BRC100 Auth (g_brc100_auth_overlay_hwnd) - WS_POPUP, layered
    |-- Notification (g_notification_overlay_hwnd) - WS_POPUP, keep-alive
```

### 2.7 macOS Port

`cef_browser_shell_mac.mm` (1754 lines): NSWindow/NSView hierarchy, 5 overlay types, event forwarding. Build system supports macOS via CMake. See `development-docs/macos-port/MAC_PLATFORM_SUPPORT_PLAN.md`.

---

## 3. Rust Wallet Backend

### 3.1 AppState (`src/main.rs`)

Shared state accessible to all HTTP handlers:

| Field | Type | Purpose |
|-------|------|---------|
| `database` | `Arc<Mutex<WalletDatabase>>` | SQLite connection (single writer) |
| `balance_cache` | `BalanceCache` | In-memory balance with instant invalidation |
| `price_cache` | `Arc<PriceCache>` | BSV/USD price (CryptoCompare + CoinGecko, 5-min TTL) |
| `fee_rate_cache` | `FeeRateCache` | Cached fee rates from MAPI |
| `sync_status` | `Arc<RwLock<SyncStatus>>` | Wallet recovery/sync progress |
| `current_user_id` | `i64` | Active user ID (default: 1) |
| `shutdown` | `CancellationToken` | Graceful shutdown signal |
| `auth_sessions` | `Arc<Mutex<HashMap>>` | BRC-103/104 auth session state |
| `message_store` | `Arc<Mutex<HashMap>>` | BRC-33 in-memory message relay |
| `pending_transactions` | `Arc<Mutex<HashMap>>` | Two-phase sign: createAction -> signAction |

### 3.2 Database Layer (`src/database/`)

SQLite with WAL mode, foreign keys enabled. Consolidated V1 schema for fresh databases; incremental migrations for existing.

**Repository pattern** (18+ repositories across 23 files):

| Repository | Purpose |
|------------|---------|
| `WalletRepository` | Master key storage, HD index, DPAPI blob |
| `UserRepository` | Identity mapping (pubkey -> userId) |
| `AddressRepository` | HD address derivation cache |
| `OutputRepository` | Primary UTXO tracking (spendable/spent_by model) |
| `TransactionRepository` | Transaction lifecycle |
| `ProvenTxRepository` | Immutable merkle proof records |
| `ProvenTxReqRepository` | Proof acquisition lifecycle |
| `CertificateRepository` | BRC-52 identity certificates |
| `DomainPermissionRepository` | Per-domain trust levels, spending limits, rate limits |
| `TagRepository` | Output tagging/basket assignment |
| `CommissionRepository` | Fee tracking per transaction |
| `SettingsRepository` | Persistent wallet configuration |
| `SyncStateRepository` | Multi-device sync state |

### 3.3 Cryptography (`src/crypto/`)

11 modules:

| Module | Purpose |
|--------|---------|
| `brc42.rs` | ECDH-based child key derivation (Type-42) |
| `brc43.rs` | Invoice number format: `{securityLevel}-{protocolID}-{keyID}` |
| `signing.rs` | SHA-256, HMAC-SHA256, ECDSA signing |
| `aesgcm_custom.rs` | AES-256-GCM encryption (BRC-2) |
| `brc2.rs` | BRC-2 encrypt/decrypt with BRC-42 key derivation |
| `dpapi.rs` | Windows DPAPI encrypt/decrypt (macOS Keychain stub) |
| `pin.rs` | PIN-based encryption (AES-256-GCM + PBKDF2 600K iterations) |
| `keys.rs` | Key computation and derivation helpers |
| `ghash.rs` | GHASH for AES-GCM |
| `mod.rs` | Key derivation routing, public key computation |

### 3.4 Key Derivation

`derive_key_for_output()` in `database/helpers.rs` is the single entry point for all signing:

| `derivation_prefix` | `derivation_suffix` | `sender_identity_key` | Path |
|---------------------|---------------------|----------------------|------|
| `"2-receive address"` | `"{index}"` | `None` | BRC-42 self-derivation (standard) |
| `"bip32"` | `"{index}"` | `None` | Legacy BIP32 HD (`m/{index}`) |
| `NULL` | `NULL` | `None` | Master private key directly |
| any | any | `Some(pubkey)` | BRC-42 counterparty derivation |

### 3.5 Transaction Lifecycle

```
createAction (build + select UTXOs)
    -> status: 'unsigned'
    -> inputs reserved (spent_by set)
    -> outputs created (spendable=0)

signAction (sign + broadcast)
    -> status: 'sending' -> 'unproven'
    -> proven_tx_req created
    -> Monitor acquires proof -> 'completed'

On failure:
    -> status: 'failed'
    -> ghost outputs deleted
    -> reserved inputs restored (spendable=1)
    -> balance cache invalidated
```

### 3.6 Wallet Security

- **DPAPI auto-unlock**: Mnemonic stored twice — PIN-encrypted + DPAPI-encrypted. Startup: try DPAPI first, auto-cache mnemonic on success.
- **PIN encryption**: AES-256-GCM with PBKDF2 (600K iterations). PIN used during create/recover.
- **DPAPI backfill**: On PIN unlock, DPAPI blob stored for future auto-unlock.
- **Legacy wallets**: `pin_salt=NULL` -> plaintext auto-cached. PIN-protected without DPAPI -> locked until PIN.

### 3.7 API Endpoints

68+ handlers in `handlers.rs`. Key groups:

**Wallet Operations**: `health`, `wallet_status`, `wallet_create`, `wallet_recover`, `wallet_unlock`, `wallet_balance`, `wallet_backup`, `wallet_sync`, `wallet_sync_status`, `wallet_bsv_price`

**BRC-100 Protocol**: `well_known_auth`, `get_public_key`, `create_action`, `sign_action`, `create_hmac`, `create_signature`, `verify_hmac`, `verify_signature`, `list_outputs`, `list_certificates`, `acquire_certificate`, `prove_certificate`, `encrypt`, `decrypt`, `send_message`, `list_messages`, `acknowledge_message`

**Domain Permissions**: `get_domain_permission`, `add_domain_permission`, `delete_domain_permission`, `get_all_domain_permissions`, `get_cert_field_permissions`, `approve_cert_fields`

**Internal**: `send_transaction` (wallet panel send), `generate_address`

---

## 4. React Frontend

### 4.1 Application Structure

Single React codebase, multiple CEF instances. Route determines context:

| Route | Context | Purpose |
|-------|---------|---------|
| `/` | Header browser | Navigation toolbar, wallet/settings buttons |
| `/wallet` | Wallet overlay | Balance, send/receive, transaction history |
| `/settings` | Settings overlay | Browser and wallet settings |
| `/backup` | Backup modal | Mnemonic backup, file backup |
| `/brc100auth` | BRC100 auth overlay | Domain approval, auth approval, payment confirmation, cert disclosure |
| `/notification` | Notification overlay | Keep-alive overlay for payment/cert/rate notifications |

### 4.2 Key Components

| Component | Purpose |
|-----------|---------|
| `MainBrowserView.tsx` | Header with navigation bar, wallet/settings buttons |
| `WalletPanel.tsx` | Balance display, send/receive tabs, sync status |
| `TransactionForm.tsx` | Send form with BSV/USD conversion, validation |
| `DomainPermissionsTab.tsx` | Manage approved sites (edit limits, revoke) |
| `DomainPermissionForm.tsx` | Per-tx/per-session spending limits, rate limits |
| `BRC100AuthOverlayRoot.tsx` | Domain approval, auth, payment confirmation, cert disclosure |

### 4.3 Hooks

| Hook | Purpose |
|------|---------|
| `useHodosBrowser()` | `getIdentity`, `generateAddress`, `navigate`, `markBackedUp`, `goBack`, `goForward`, `reload` |
| `useBalance()` | Fetches balance + BSV price from Rust backend |
| `useBackgroundBalancePoller()` | Polls balance every 30s for auto-refresh |

### 4.4 Bridge (`initWindowBridge.ts`)

Defines `window.hodosBrowser.navigation` and `window.hodosBrowser.overlay` via `cefMessage.send()`.

---

## 5. Communication Patterns

### 5.1 Three Communication Paths

| Pattern | Direction | Mechanism | Used For |
|---------|-----------|-----------|----------|
| **CefURLRequest** (async) | C++ -> Rust | HTTP on IO thread | BRC-100 wallet endpoints (payment, auth, signing) |
| **WinHTTP** (sync) | C++ -> Rust | Synchronous HTTP | Domain permission lookups, price cache, wallet status |
| **Direct fetch** | React -> Rust | Frontend HTTP | Wallet panel operations (balance, send, backup) |

### 5.2 IPC (C++ <-> React)

```
React -> cefMessage.send("command", data)
  -> CefProcessMessage to browser process
    -> simple_handler.cpp OnProcessMessageReceived
      -> dispatch by message name
```

30+ IPC message types including: `navigate`, `overlay_show_*`, `overlay_close`, `brc100_auth_response`, `add_domain_permission`, `approve_cert_fields`, `tab_create`, `bookmark_add`.

---

## 6. Security Architecture

### 6.1 Process Isolation

- **Header browser**: Trusted React UI, isolated from web content
- **WebView browser**: Untrusted web content, HTTP interception active
- **Overlays**: Each in own process with own V8 context
- **Rust wallet**: Separate process, only accessible via localhost HTTP
- **Tab isolation**: Process-per-tab via CEF (Chromium's security model)

### 6.2 Domain Permission System

Two effective trust levels: **unknown** (show approval overlay) and **approved** (check spending limits).

Per-domain controls:
- Per-transaction spending limit (USD cents, default $0.10)
- Per-session spending limit (USD cents, default $3.00)
- Rate limiting (requests per minute, default 10)
- Certificate field disclosure tracking

### 6.3 Defense in Depth

1. **C++ layer**: DomainPermissionCache checks domain status before forwarding
2. **C++ auto-approve engine**: Rate limits, spending limits, payment confirmation notifications
3. **Rust layer**: `check_domain_approved()` validates `X-Requesting-Domain` header
4. **Rust spending check**: `create_action` verifies per-tx limit via price cache

### 6.4 Key Security Properties

1. Private keys never in JavaScript — all signing in Rust
2. DPAPI/Keychain encryption for mnemonic at rest
3. PIN encryption (AES-256-GCM + PBKDF2) as second layer
4. Parameterized SQL — no string interpolation
5. App-scoped identity keys — BRC-103/104 prevents cross-app tracking
6. Atomic timeout handling — `compare_exchange_strong` prevents double-fire crashes

---

## 7. Data Storage

### 7.1 File System Layout

| Platform | Root | Wallet DB | Browser Data |
|----------|------|-----------|--------------|
| Windows | `%APPDATA%/HodosBrowser/` | `wallet/wallet.db` | `Default/` (history, bookmarks, cookies) |
| macOS | `~/Library/Application Support/HodosBrowser/` | `wallet/wallet.db` | `Default/` |

### 7.2 Database Schema (Consolidated V1)

All 24 incremental migrations collapsed into single `create_schema_v1()` for fresh databases. Existing databases migrate incrementally.

**Active tables**: wallets, users, addresses, transactions, outputs, parent_transactions, block_headers, proven_txs, proven_tx_reqs, output_baskets, output_tags, output_tag_map, certificates, certificate_fields, commissions, settings, sync_states, monitor_events, transaction_inputs, transaction_outputs, domain_permissions, cert_field_permissions

### 7.3 Browser Data (C++ Layer)

History and bookmarks managed by C++ singletons (`HistoryManager`, `BookmarkManager`) with their own SQLite databases in `Default/`. Cookies managed by CEF's built-in cookie manager.

---

## 8. Background Services

### 8.1 Monitor Pattern

The Monitor (`src/monitor/mod.rs`) runs as a single tokio task with a 30-second tick loop:

| Task | Interval | Purpose |
|------|----------|---------|
| TaskCheckForProofs | 60s | Acquire merkle proofs (ARC -> WoC fallback) |
| TaskSendWaiting | 120s | Crash recovery for stuck `sending` txs |
| TaskFailAbandoned | 300s | Fail stuck unsigned/unprocessed txs |
| TaskUnFail | 300s | Recover false failures (6-hour window) |
| TaskReviewStatus | 60s | Status consistency across tables |
| TaskPurge | 3600s | Cleanup old events and proof requests |
| TaskSyncPending | 30s | UTXO sync for pending addresses |

Uses `CancellationToken` for graceful shutdown and `try_lock()` to avoid blocking user requests.

### 8.2 UTXO Synchronization

Two mechanisms:
1. **Periodic (TaskSyncPending)**: Checks addresses with `pending_utxo_check=1` every 30s
2. **On-demand (`POST /wallet/sync`)**: Frontend trigger, supports `?full=true` for all addresses

### 8.3 Price & Fee Caching

- **PriceCache** (Rust): CryptoCompare primary + CoinGecko fallback, 5-min TTL, thread-safe via `RwLock`
- **BSVPriceCache** (C++): WinHTTP to `/wallet/bsv-price`, 5-min TTL, used by auto-approve engine
- **FeeRateCache** (Rust): MAPI fee rates with TTL

---

## 9. BRC-100 Protocol

### 9.1 Implementation Status

| Group | Status | Description |
|-------|--------|-------------|
| **A: Authentication** | Complete | BRC-103/104 mutual auth, key derivation |
| **B: Transactions** | Complete | createAction, signAction, BRC-29 payments, BEEF/SPV |
| **C: Output Management** | Partial | listOutputs, baskets, tags |
| **D: Encryption** | Partial | BRC-2 AES-256-GCM encrypt/decrypt |
| **E: Certificates** | Partial | Schema ready, acquireCertificate, proveCertificate |
| **BRC-33 Messages** | Complete | sendMessage, listMessages, acknowledgeMessage |

### 9.2 Authentication Flow (BRC-104)

```
1. Client POST /.well-known/auth {initialNonce, identityKey}
2. Server: BRC-42 key derivation (ECDH shared secret -> HMAC -> child key)
3. Server: Sign concatenated nonces with derived key
4. Response: {version, nonce, yourNonce, signature}
```

### 9.3 BEEF/SPV

Transactions broadcast in BEEF (Background Evaluation Extended Format):
- `beef.rs`: Parser, TSC proof <-> BUMP conversion
- `beef_helpers.rs`: Recursive ancestry chain building
- `parent_transactions` table: Raw tx cache
- `proven_txs` table: Immutable merkle proof records

---

## 10. Development Status

### Completed
- BRC-100 Groups A & B (authentication + transactions)
- Database migration consolidation (V1 schema)
- DPAPI auto-unlock + PIN encryption
- Domain permission system + auto-approve engine
- Notification overlay (keep-alive, 4 types)
- Mnemonic recovery + Centbee sweep
- Defense-in-depth (C++ + Rust permission checks)
- Price cache migration (frontend -> backend)
- Branding/CSS (black + gold theme)

### In Progress (Browser Core MVP)
- SSL certificate handling + secure connection indicator
- Permission prompts (camera, mic, geolocation)
- Download handler
- Find-in-page, context menus, keyboard shortcuts
- Ad & tracker blocking (adblock-rust FFI)
- Light wallet polish

### Future
- macOS port (5-7 day sprint, see `development-docs/macos-port/`)
- Full wallet view (transaction history, output browser)
- Activity status indicator
- Settings persistence + profile import
- Certificate testing (needs certifier service)

---

*This document is maintained alongside the codebase. See `CLAUDE.md` for AI assistant context and invariants.*
