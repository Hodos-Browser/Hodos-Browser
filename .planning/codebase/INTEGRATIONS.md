# External Integrations

**Analysis Date:** 2026-01-24

## APIs & External Services

**Blockchain Data (Bitcoin SV):**
- **WhatsOnChain** - Primary blockchain explorer and API
  - SDK/Client: reqwest 0.11 HTTP client (`rust-wallet/Cargo.toml`)
  - Base URL: `https://api.whatsonchain.com/v1/bsv/main/`
  - Endpoints used:
    - `/address/{address}/unspent` - Fetch UTXOs (`rust-wallet/src/utxo_fetcher.rs`)
    - `/tx/hash/{txid}` - Get transaction details
    - `/tx/{txid}/hex` - Get raw transaction hex (`rust-wallet/src/cache_helpers.rs`)
    - `/tx/{txid}/proof/tsc` - Get Merkle proofs (TSC format) (`rust-wallet/src/cache_helpers.rs`)
    - `/tx/{txid}/outspend/{vout}` - Check if output is spent (`rust-wallet/src/handlers.rs`)
    - `/chain/info` - Get blockchain info (`rust-wallet/src/handlers.rs`)
    - `/block/height/{height}` - Get block by height (`rust-wallet/src/handlers.rs`)
    - `/block/{hash}/header` - Get block header (`rust-wallet/src/handlers.rs`)
    - `/tx/raw` - Broadcast transaction (`rust-wallet/src/handlers.rs`)
  - Retry Logic: Exponential backoff on 500/502/503/504 errors (`rust-wallet/src/utxo_fetcher.rs`)
  - Rate limits: None enforced client-side

**Mining Pool APIs:**
- **GorillaPool** - Alternative transaction broadcasting via Merchant API (MAPI)
  - Endpoint: `https://mapi.gorillapool.io/mapi/tx` (`rust-wallet/src/handlers.rs`)
  - Purpose: Broadcast transactions with fee estimation support
  - Note: Fee quote integration planned (TODO comment in handlers.rs)

- **TAAL (Merchant API)** - Fee rate provider
  - Endpoint: `https://merchantapi.taal.com/mapi/feeQuote`
  - Status: Documented but not yet integrated (`rust-wallet/src/handlers.rs`)
  - Purpose: Dynamic fee rate fetching (currently hardcoded to 1 sat/byte)

**Price Data:**
- **CryptoCompare** - USD price for BSV
  - Endpoint: `https://min-api.cryptocompare.com/data/price?fsym=BSV&tsyms=USD` (`frontend/src/hooks/useBalance.ts`)
  - Integration: Fetch API from frontend
  - Fallback: CoinGecko API

- **CoinGecko** - Alternative price source
  - Endpoint: `https://api.coingecko.com/api/v3/simple/price?ids=bitcoin-sv&vs_currencies=usd` (`frontend/src/hooks/useBalance.ts`)
  - Integration: Fetch API from frontend
  - Purpose: Fallback if CryptoCompare fails

## Data Storage

**Databases:**
- **SQLite** (wallet.db) - Primary wallet data store
  - Connection: File-based at `%APPDATA%/HodosBrowser/wallet/wallet.db` (`rust-wallet/src/main.rs`)
  - Client: rusqlite 0.30 with bundled feature (`rust-wallet/Cargo.toml`)
  - Migrations: Embedded in `rust-wallet/src/database/migrations.rs`
  - Repositories: wallet_repo, address_repo, utxo_repo, transaction_repo, certificate_repo, merkle_proof_repo, block_header_repo, tag_repo, basket_repo, message_relay_repo

- **SQLite** (history.db) - Browser history (separate from wallet)
  - Connection: File-based at `%APPDATA%/HodosBrowser/Default/` (CEF layer)
  - Client: SQLite3 via C++ (`cef-native/src/core/HistoryManager.cpp`)
  - Purpose: Browsing history, separate from wallet data

**File Storage:**
- Local filesystem - Wallet backups
  - Location: User-selected directory (`frontend/src/components/panels/BackupModal.tsx`)
  - Format: Raw wallet.db file copy (`rust-wallet/src/backup.rs`)

**Caching:**
- In-memory balance cache - Arc<Mutex<BalanceCache>> in AppState (`rust-wallet/src/balance_cache.rs`)
  - TTL: Configurable per implementation
  - Purpose: Reduce blockchain API calls for frequent balance checks
- UTXO cache - SQLite table with background sync (`rust-wallet/src/utxo_sync.rs`)
  - Sync interval: Configurable (background Tokio task)
- Merkle proof cache - SQLite table (`rust-wallet/src/database/merkle_proof_repo.rs`)
- Block header cache - SQLite table (`rust-wallet/src/database/block_header_repo.rs`)

## Authentication & Identity

**Auth Provider:**
- **Custom BRC-100 protocol suite** - Bitcoin SV-based authentication
  - Implementation: `rust-wallet/src/auth_session.rs`, `rust-wallet/src/handlers/certificate_handlers.rs`
  - Token storage: In-memory auth session manager in AppState
  - Session management: BRC-103/104 mutual authentication sessions
  - Protocols implemented:
    - BRC-42: ECDH-based child key derivation (`rust-wallet/src/crypto/brc42.rs`)
    - BRC-43: Invoice number format (`rust-wallet/src/crypto/brc43.rs`)
    - BRC-52: Identity certificate format with selective disclosure (`rust-wallet/src/certificate/`)
    - BRC-103/104: Mutual authentication (`rust-wallet/src/auth_session.rs`)

**OAuth Integrations:**
- None (Bitcoin-native authentication only)

## Monitoring & Observability

**Error Tracking:**
- Console logging only (development)
  - Frontend: `console.log`, `console.error` (`frontend/src/` files)
  - Rust: env_logger with log crate (`rust-wallet/Cargo.toml`)
  - C++: Custom Logger class with macros (`cef-native/src/core/Logger.cpp`)

**Analytics:**
- None (no telemetry/analytics services integrated)

**Logs:**
- stdout/stderr only
  - Rust: env_logger writes to stderr (`rust-wallet/src/main.rs`)
  - C++: LOG_INFO, LOG_DEBUG macros to stdout/file (`cef-native/src/core/Logger.cpp`)
  - Frontend: Browser console

## CI/CD & Deployment

**Hosting:**
- Native desktop application (no cloud hosting)
  - Platform: Windows/macOS local installation
  - Deployment: Manual builds distributed as executables

**CI Pipeline:**
- None detected (manual builds)
  - Build process documented in CLAUDE.md

## Environment Configuration

**Development:**
- Required env vars: None (environment-independent)
- Wallet location: `%APPDATA%/HodosBrowser/wallet/` (Windows), `~/Library/Application Support/HodosBrowser/wallet/` (macOS)
- Mock/stub services: None (uses mainnet BSV blockchain)
- Ports:
  - Frontend dev server: localhost:5137 (`frontend/vite.config.ts`)
  - Rust wallet backend: localhost:3301 (`rust-wallet/src/main.rs`)

**Staging:**
- Not applicable (no staging environment; production mainnet only)

**Production:**
- Secrets management: Private keys stored in wallet.db (encrypted at rest)
- Blockchain network: Bitcoin SV mainnet (hardcoded in `rust-wallet/src/handlers.rs`)
- Failover/redundancy: Manual fallback to GorillaPool if WhatsOnChain fails

## Webhooks & Callbacks

**Incoming:**
- None (no webhook endpoints; pull-based architecture)

**Outgoing:**
- None (no webhooks sent to external services)

## IPC/Message Bridging

**CEF Message Bridge:**
- **Frontend → C++**: `window.cefMessage.send()` (`frontend/src/bridge/initWindowBridge.ts`)
  - Implementation: `CefMessageSendHandler` in `cef-native/src/handlers/simple_render_process_handler.cpp`
  - Messages: 'navigate', 'overlay_show', 'overlay_close', 'overlay_show_brc100_auth', etc.

- **C++ → Rust**: HTTP requests to localhost:3301
  - Implementation: `AsyncWalletResourceHandler` in `cef-native/src/core/HttpRequestInterceptor.cpp`
  - Routing: Domain verification via `DomainVerifier` class
  - Auth: BRC-100 auth request queue (`g_pendingAuthRequest` global)

**window.hodosBrowser API:**
- **Browser → Wallet**: Injected JavaScript API
  - Namespaces: wallet, address, navigation, overlay, brc100
  - Type definitions: `frontend/src/types/hodosBrowser.d.ts`
  - V8 Injection: `simple_render_process_handler.cpp` OnContextCreated()

---

*Integration audit: 2026-01-24*
*Update when adding/removing external services*
