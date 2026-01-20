# External Integrations

**Analysis Date:** 2026-01-20

## APIs & External Services

**Blockchain Services:**
- WhatsOnChain - UTXO fetching for Bitcoin SV
  - SDK/Client: `reqwest` HTTP client in Rust
  - Endpoint: `https://api.whatsonchain.com/v1/bsv/main/address/{address}/unspent`
  - Usage: `rust-wallet/src/utxo_fetcher.rs`
  - Auth: None (public API)

**Price Data:**
- CryptoCompare - BSV/USD price conversion
  - SDK/Client: Native `fetch()` in JavaScript
  - Endpoint: `https://min-api.cryptocompare.com/data/price?fsym=BSV&tsyms=USD`
  - Usage: `frontend/src/components/TransactionForm.tsx`, `frontend/src/hooks/useBalance.ts`
  - Auth: None (public API)

**Planned but Not Yet Integrated:**
- Merchant API (MAPI) - Dynamic fee rates from BSV miners
  - Status: TODO in `rust-wallet/src/handlers.rs` (currently hardcoded to 1 sat/byte)
  - Provider: GorillaPool mentioned in comments

## Data Storage

**Databases:**
- SQLite 3 (bundled via rusqlite 0.30) - Primary wallet data store
  - Connection: Application-level path resolution
  - Location: `{APPDATA}/HodosBrowser/wallet/wallet.db` (Windows)
  - Location: `~/Library/Application Support/HodosBrowser/wallet/wallet.db` (macOS)
  - Schema: `rust-wallet/src/database/migrations.rs`, `rust-wallet/src/database/migrations_simple.rs`

- SQLite 3 (C++ layer) - Browser data persistence
  - Location: `{APPDATA}/HodosBrowser/Default/` (Windows)
  - Usage: History, bookmarks, cookies
  - Integration: via vcpkg (Windows) or Homebrew (macOS)

**File Storage:**
- Local filesystem only - No cloud storage integration
- Wallet backups: JSON export via `/wallet/backup` endpoint

**Caching:**
- In-memory balance cache via `Arc<Mutex<>>` in Rust (`rust-wallet/src/balance_cache.rs`)
- No Redis or external cache

## Authentication & Identity

**Auth Provider:**
- Custom BRC-100 protocol suite - Bitcoin SV authentication
  - Implementation: Fully custom in Rust (`rust-wallet/src/crypto/`, `rust-wallet/src/handlers.rs`)
  - Token storage: SQLite database (certificates, sessions)
  - Session management: `rust-wallet/src/auth_session.rs`

**BRC Protocol Implementations:**
- BRC-42 - ECDH key derivation (`rust-wallet/src/crypto/brc42.rs`)
- BRC-43 - Invoice number format (`rust-wallet/src/crypto/brc43.rs`)
- BRC-2 - Symmetric encryption AES-GCM (`rust-wallet/src/crypto/brc2.rs`)
- BRC-33 - Message relay protocol (`rust-wallet/src/message_relay.rs`)
- BRC-48 - Bitcoin script parsing (`rust-wallet/src/script.rs`)
- BRC-52 - Identity certificates (`rust-wallet/src/certificate/`)
- BRC-103/104 - Mutual authentication (`rust-wallet/src/auth_session.rs`)

**OAuth Integrations:**
- None (BRC-100 is self-sovereign identity)

## Monitoring & Observability

**Error Tracking:**
- None configured (logs to stdout/stderr only)

**Analytics:**
- None

**Logs:**
- Rust: `env_logger` with default "info" level (`rust-wallet/src/main.rs`)
- C++: Custom `Logger` class with timestamped output (`cef-native/cef_browser_shell.cpp`)
- Frontend: `console.log` statements (304+ throughout codebase)

## CI/CD & Deployment

**Hosting:**
- Desktop application - No cloud hosting
- Self-contained: CEF shell bundles frontend, connects to local Rust backend

**CI Pipeline:**
- Not configured (manual builds)

## Environment Configuration

**Development:**
- Required: Node.js 18+, Rust stable, CMake 3.15+, vcpkg (Windows) or Homebrew (macOS)
- Ports: Frontend dev server on 5137, Wallet API on 3301
- CEF binaries downloaded separately from cef-builds.spotifycdn.com

**Staging:**
- Not applicable (desktop application)

**Production:**
- Wallet: Release build via `cargo build --release`
- Frontend: Production bundle via `npm run build`
- CEF: Release build via `cmake --build build --config Release`
- Data locations:
  - Windows: `%APPDATA%/HodosBrowser/`
  - macOS: `~/Library/Application Support/HodosBrowser/`

## Webhooks & Callbacks

**Incoming:**
- None (desktop application, no server-side webhooks)

**Outgoing:**
- UTXO fetch calls to WhatsOnChain on wallet operations
- Price fetch calls to CryptoCompare on balance display

## Inter-Process Communication

**Frontend <-> CEF:**
- V8 JavaScript injection (`window.hodosBrowser.*` API)
- Message passing via `window.cefMessage.send()`
- Implementation: `cef-native/src/handlers/simple_render_process_handler.cpp`
- Bridge definition: `frontend/src/bridge/initWindowBridge.ts`

**CEF <-> Rust Wallet:**
- HTTP via WinHTTP (Windows) or libcurl (macOS)
- All wallet requests routed to `localhost:3301`
- Interception: `cef-native/src/core/HttpRequestInterceptor.cpp`

## Wallet HTTP Endpoints (localhost:3301)

**Core:**
- `GET /health` - Health check
- `GET /brc100/status` - BRC-100 protocol status
- `POST /getPublicKey` - Retrieve wallet public key
- `POST /isAuthenticated` - Check auth status
- `POST /.well-known/auth` - Well-known auth endpoint (BRC-100 standard)

**Actions/Transactions:**
- `POST /createAction` - Create transaction action (100MB BEEF support)
- `POST /signAction` - Sign action (100MB BEEF support)
- `POST /processAction` - Process signed action
- `POST /abortAction` - Abort action
- `POST /listActions` - List actions
- `POST /transaction/send` - Broadcast transaction

**Blockchain:**
- `POST /getHeight` - Current block height
- `POST /getHeaderForHeight` - Block header at height
- `POST /getNetwork` - Network info

**Certificates:**
- `POST /acquireCertificate` - Request certificate (BRC-52)
- `POST /listCertificates` - List certificates
- `POST /proveCertificate` - Prove certificate ownership
- `POST /relinquishCertificate` - Release certificate

**Wallet Management:**
- `GET /wallet/status` - Wallet status
- `GET /wallet/balance` - Current balance
- `POST /wallet/address/generate` - Generate new address
- `GET /wallet/addresses` - List all addresses
- `POST /wallet/backup` - Export wallet backup
- `POST /wallet/restore` - Restore from backup
- `POST /wallet/recover` - Recover from mnemonic

**Messaging:**
- `POST /sendMessage` - Send BRC-33 message
- `POST /listMessages` - List messages
- `POST /acknowledgeMessage` - Acknowledge receipt

**Domain Management:**
- `GET /domain/whitelist/check` - Check if domain whitelisted
- `POST /domain/whitelist/add` - Add domain to whitelist

---

*Integration audit: 2026-01-20*
*Update when adding/removing external services*
