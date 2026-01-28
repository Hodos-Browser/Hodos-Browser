# External Integrations

**Analysis Date:** 2026-01-28

## APIs & External Services

**Blockchain Data (WhatsOnChain):**
- Service: WhatsOnChain API (BSV mainnet)
- What it's used for: UTXO fetching, transaction lookup, proof retrieval, block headers
- SDK/Client: reqwest (raw HTTP client)
- Endpoints:
  - `GET https://api.whatsonchain.com/v1/bsv/main/address/{address}/unspent` - Fetch UTXOs (`rust-wallet/src/utxo_fetcher.rs`)
  - `GET https://api.whatsonchain.com/v1/bsv/main/tx/{txid}/hex` - Get raw transaction (`rust-wallet/src/cache_helpers.rs`)
  - `GET https://api.whatsonchain.com/v1/bsv/main/tx/{txid}/proof/tsc` - Get TSC proof (`rust-wallet/src/cache_helpers.rs`)
  - `GET https://api.whatsonchain.com/v1/bsv/main/block/hash/{blockhash}` - Get block headers (`rust-wallet/src/cache_helpers.rs`, `rust-wallet/src/cache_sync.rs`)
  - `GET https://api.whatsonchain.com/v1/bsv/main/tx/{txid}/outspend/{vout}` - Check UTXO spend status (`rust-wallet/src/handlers.rs`, `rust-wallet/src/handlers/certificate_handlers.rs`)
  - `GET https://api.whatsonchain.com/v1/bsv/main/tx/hash/{txid}` - Lookup transaction (`rust-wallet/src/handlers.rs`)
  - `GET https://api.whatsonchain.com/v1/bsv/main/chain/info` - Get chain info (`rust-wallet/src/handlers.rs`)
  - `GET https://api.whatsonchain.com/v1/bsv/main/block/height/{height}` - Get block by height (`rust-wallet/src/handlers.rs`)
  - `GET https://api.whatsonchain.com/v1/bsv/main/block/{blockhash}/header` - Get block header (`rust-wallet/src/handlers.rs`)
  - `POST https://api.whatsonchain.com/v1/bsv/main/tx/raw` - Broadcast transaction (`rust-wallet/src/handlers.rs`)
- Auth: None (public API)
- Retry logic: Exponential backoff for 500-class errors, max 3 retries, 1s initial delay

**Blockchain Broadcasting (GorillaPool):**
- Service: GorillaPool Merchant API (BSV transaction broadcasting)
- What it's used for: Primary transaction broadcasting endpoint
- SDK/Client: reqwest (raw HTTP client)
- Endpoint: `POST https://mapi.gorillapool.io/mapi/tx` (`rust-wallet/src/handlers.rs`)
- Auth: None (public API)
- Fallback: If GorillaPool fails, falls back to WhatsOnChain broadcast

**Transaction Fee Estimation (TAAL):**
- Service: TAAL Merchant API (fee quotes)
- What it's used for: Dynamic transaction fee rates (TODO - not currently integrated)
- SDK/Client: reqwest (planned)
- Endpoint: `https://merchantapi.taal.com/mapi/feeQuote` (referenced in `rust-wallet/src/handlers.rs` as TODO)
- Status: Commented as future integration - currently uses hardcoded fee rates

## Data Storage

**Databases:**
- Type: SQLite
- Location: `%APPDATA%/HodosBrowser/wallet/wallet.db`
- Client: rusqlite 0.30 (bundled) - Pure Rust SQLite driver
- Implementation: `rust-wallet/src/database/` module
  - Connection wrapper: `rust-wallet/src/database/connection.rs`
  - Repositories: `WalletRepository`, `AddressRepository`, `UtxoRepository`, `CertificateRepository`, `BlockHeaderRepository`, `TransactionRepository`, `BasketRepository`, etc.
- Features: WAL mode, foreign keys enabled, prepared statements
- Schema management: `rust-wallet/src/database/migrations.rs` and `rust-wallet/src/database/migrations_simple.rs`
- Tables: wallet, addresses, utxos, transactions, certificates, block_headers, merkle_proofs, parent_transactions, baskets, tags, message_relay entries

**Browser Data Storage:**
- CEF SQLite: `%APPDATA%/HodosBrowser/Default/` (managed by CEF layer)
  - History, bookmarks, cookies (separate from wallet DB)
- Implementation: `cef-native/src/core/HistoryManager.cpp` - SQLite wrapper for browser history

**File Storage:**
- Local filesystem only - no cloud storage
- Wallet directory: `%APPDATA%/HodosBrowser/wallet/`
- CEF data directory: `%APPDATA%/HodosBrowser/Default/`
- All storage is local to the user's machine

**Caching:**
- In-memory balance cache: `rust-wallet/src/balance_cache.rs` - `BalanceCache` struct
- Transaction cache: `rust-wallet/src/cache_sync.rs` - Background sync service
- No external caching service (Redis, Memcached, etc.)

## Authentication & Identity

**Auth Provider:**
- Custom implementation - BRC-100 protocol suite (BSV authentication standard)

**Authentication Flow:**
- BRC-103/104 Mutual Authentication: `rust-wallet/src/auth_session.rs` - `AuthSessionManager`
- Endpoints:
  - `POST /well_known_auth` - BRC-100 auth endpoint (`rust-wallet/src/handlers.rs`)
  - `POST /createAction` - Create authentication action
  - `POST /signAction` - Sign action with private key
  - `POST /getPublicKey` - Get public key for identity
- Identity certificates: BRC-52 format with selective disclosure (`rust-wallet/src/certificate/`)
- Implementation approach: Self-contained - Rust wallet signs and verifies, no external OAuth provider

**Session Management:**
- In-memory: `AuthSessionManager` stores active auth sessions with timeout
- No persistent session storage

## Monitoring & Observability

**Error Tracking:**
- Not detected - no Sentry, Rollbar, or similar error tracking service integrated

**Logs:**
- Approach: Standard logging via `log` crate facade
- Implementation: `env_logger` for console output
- Rust wallet: Console logging with `log::info!`, `log::debug!`, `log::warn!`, `log::error!`
- CEF layer: Custom `Logger` class in `cef-native/cef_browser_shell.cpp` for Windows logging
- Frontend: Browser console via React
- Log level: Configured via `RUST_LOG` environment variable (default "info")

## CI/CD & Deployment

**Hosting:**
- Local desktop application - self-hosted on end-user machine
- No cloud hosting or managed services
- Development: Three-process local architecture (Rust backend, Frontend dev server, CEF browser)

**CI Pipeline:**
- Not detected - no GitHub Actions, GitLab CI, or other automated CI service configured

**Build Artifacts:**
- Windows: `cef-native/build/bin/Release/HodosBrowserShell.exe` - standalone executable
- macOS: `cef-native/build/bin/HodosBrowserShell.app` - app bundle with embedded CEF framework
- CEF runtime files automatically copied post-build

## Environment Configuration

**Required env vars (Development):**
- `APPDATA` - Windows application data path (auto-detected, used for wallet DB and browser data)
- `VCPKG_ROOT` - Path to vcpkg installation (CMake dependency manager, required for Windows builds)
- `RUST_LOG` - Log level for Rust wallet (optional, default "info")

**Required env vars (Production):**
- `APPDATA` - User's Windows application data directory (auto-detected)
- No other environment variables required for end-user runtime

**Secrets location:**
- Private keys: Stored in SQLite database at `%APPDATA%/HodosBrowser/wallet/wallet.db`, AES-GCM encrypted (`rust-wallet/src/crypto/aesgcm_custom.rs`)
- Master seed: Encrypted in database
- No environment variable-based secrets (all encrypted in database)

## Webhooks & Callbacks

**Incoming Webhooks:**
- None detected - wallet does not expose webhook endpoints for external push events

**Outgoing Webhooks:**
- None detected - wallet does not call external services for event notifications

**IPC & Internal Communication:**
- CEF to Rust: HTTP requests to `localhost:3301` via HttpRequestInterceptor (`cef-native/src/core/HttpRequestInterceptor.cpp`)
  - Wallet API calls intercepted and forwarded
  - Endpoint pattern: `/getPublicKey`, `/.well-known/auth`, `/createAction`, `/signAction`, etc.
- Frontend to CEF: V8 injection `window.hodosBrowser.*` and `cefMessage.send()` (`cef-native/src/handlers/simple_render_process_handler.cpp`)
- CEF to Frontend: Message passing via `CefMessageSendHandler` IPC

## Message Relay & Store

**BRC-33 Message Relay:**
- Implementation: `rust-wallet/src/message_relay.rs` - In-memory `MessageStore`
- Purpose: Store signed messages for later verification
- Database: `rust-wallet/src/database/message_relay_repo.rs` - Persisted to SQLite

**Domain Whitelist:**
- Implementation: `rust-wallet/src/domain_whitelist.rs` - `DomainWhitelistManager`
- Purpose: Track approved domains for authentication requests
- Storage: In-memory + database persistence

## Blockchain Integration Points

**Transaction Building & Signing:**
- Raw transaction building: Custom Rust implementation in `rust-wallet/src/transaction/`
- Signing algorithm: ForkID SIGHASH for BSV (`rust-wallet/src/transaction/sighash.rs`)
- Script parsing: `rust-wallet/src/script/` - PushDrop (BRC-48) support for data storage

**UTXO Management:**
- Fetcher: `rust-wallet/src/utxo_fetcher.rs` - Queries WhatsOnChain
- Repository: `rust-wallet/src/database/utxo_repo.rs` - Persists to SQLite
- Background sync: `rust-wallet/src/utxo_sync.rs` - Periodic UTXO refresh service
- Verification: UTXO spend status checked against WhatsOnChain

**BEEF Support:**
- BEEF (Background Evaluation Extended Format): `rust-wallet/src/beef.rs`
- Purpose: Atomic transaction format with SPV proofs
- Build helpers: `rust-wallet/src/beef_helpers.rs` - Used by `listOutputs` endpoint

**Child Key Derivation (BRC-42):**
- Implementation: `rust-wallet/src/crypto/brc42.rs`
- Functions: `derive_child_private_key`, `derive_child_public_key`
- Purpose: ECDH-based key derivation from master key + counterparty public key

**Invoice Number Format (BRC-43):**
- Implementation: `rust-wallet/src/crypto/brc43.rs`
- Format: `{securityLevel}-{protocolID}-{keyID}`
- Used for: Payment request tracking and protocol identification

---

*Integration audit: 2026-01-28*
