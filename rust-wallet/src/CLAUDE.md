# Rust Wallet Source Modules
> HTTP handlers, caching, external API clients, and supporting modules for the wallet backend

## Overview

This directory contains the core source modules for the Actix-web wallet server. `main.rs` bootstraps the app state and HTTP server on port 31301. `handlers.rs` (13k+ lines) contains all 70+ endpoint handlers organized by BRC-100 protocol area. The remaining modules provide caching, external API integration (WhatsOnChain, ARC, MessageBox, Paymail), BEEF transaction parsing, and wallet recovery.

Subdirectories `crypto/`, `database/`, `transaction/`, `certificate/`, `script/`, and `monitor/` each have their own CLAUDE.md with detailed documentation.

## Files

| File | Purpose |
|------|---------|
| `main.rs` | Entry point: `AppState` struct, Actix-web server init, route registration, DPAPI auto-unlock, Monitor startup |
| `lib.rs` | Library exports for `cargo test` — re-exports all submodules |
| `handlers.rs` | All 70+ HTTP endpoint handlers grouped by protocol area (see Handler Groups below) |
| `action_storage.rs` | `ActionStatus`, `TransactionStatus`, `ProvenTxReqStatus` enums; legacy `ActionStorage` (HashMap-backed, superseded by database) |
| `auth_session.rs` | `AuthSessionManager` — BRC-103/104 session tracking with 24-hour expiry |
| `authfetch.rs` | `AuthFetchClient` — BRC-103/104 authenticated HTTP client for MessageBox and overlay services |
| `backup.rs` | `EncryptedBackup`, `BackupPayload` — AES-GCM encrypted wallet export/import with all database entities |
| `balance_cache.rs` | `BalanceCache` — in-memory balance with 60s TTL, stale fallback, and invalidation API |
| `beef.rs` | BRC-62 BEEF parser: V1/V2/Atomic markers, `MerkleProof`, `ParsedTransaction`, raw TX extraction |
| `beef_helpers.rs` | BEEF building helpers: `build_beef_for_txid()` with ancestry walk (MAX_BEEF_ANCESTORS = 50), `fetch_transaction_for_beef()` |
| `cache_errors.rs` | `CacheError` enum (Database, Api, InvalidData, HexDecode, Json) and `CacheResult<T>` alias |
| `cache_helpers.rs` | Shared SPV cache functions: `fetch_parent_transaction_from_api()`, `fetch_tsc_proof_from_api()` (ARC primary, WoC fallback), `verify_txid()` |
| `fee_rate_cache.rs` | `FeeRateCache` — 1-hour TTL, fetches from ARC `/v1/policy`, defaults to 1000 sat/KB |
| `identity_resolver.rs` | `IdentityResolver` — resolves identity keys to names/avatars via BSV Overlay Services (BRC-52 certificates), 10-min cache |
| `json_storage.rs` | Legacy JSON file storage (`Wallet`, `AddressInfo`). Superseded by database; kept for backward compatibility |
| `message_relay.rs` | `MessageStore` — in-memory BRC-33 PeerServ message relay (recipient → box → messages) |
| `messagebox.rs` | `MessageBoxClient` — BRC-2 encrypted messaging via `messagebox.babbage.systems`, uses AuthFetch for transport |
| `paymail.rs` | `PaymailClient` — bsvalias capability discovery, P2P payment destination, public profile resolution. Handles HandCash `$alias` shorthand |
| `price_cache.rs` | `PriceCache` — BSV/USD from CryptoCompare (primary) + CoinGecko (fallback), 5-min TTL, $0.01–$10k sanity range |
| `recovery.rs` | `derive_private_key_bip32()` for legacy `m/{index}` outputs, `recover_wallet_from_mnemonic()` with gap-limit scanning (BIP32 + BRC-42) |
| `utxo_fetcher.rs` | `fetch_utxos_for_address()` — WhatsOnChain UTXO fetch with exponential backoff retry (max 3), `address_has_history()` for gap-limit scanning |

## Handler Groups (handlers.rs)

The 70+ handlers are organized by protocol area:

| Group | Key Handlers | Lines |
|-------|-------------|-------|
| **BRC-100 Identity** | `health`, `get_version`, `get_public_key`, `is_authenticated`, `wait_for_authentication`, `well_known_auth` | 150–746 |
| **BRC-100 Crypto** | `create_hmac`, `verify_hmac`, `encrypt`, `decrypt`, `create_signature`, `verify_signature` | 748–2700 |
| **Transactions** | `create_action`, `sign_action`, `process_action`, `internalize_action`, `abort_action`, `list_actions`, `update_confirmations` | 3234–9000 |
| **Wallet Mgmt** | `wallet_status`, `wallet_create`, `wallet_delete`, `wallet_balance`, `wallet_sync`, `wallet_backup`, `wallet_restore`, `wallet_unlock`, `wallet_recover`, `wallet_rescan`, `wallet_cleanup`, `wallet_export`, `wallet_import`, `wallet_activity` | 1802–12000 |
| **Addresses** | `generate_address`, `get_all_addresses`, `get_current_address` | 7756–8000 |
| **Outputs** | `list_outputs` (with BEEF building), `relinquish_output` | 11347+ |
| **Blockchain** | `get_height`, `get_header_for_height`, `get_network` | — |
| **Certificates** | delegates to `certificate/handlers.rs`: `list_certificates`, `acquire_certificate`, `prove_certificate`, `relinquish_certificate`, `discover_by_identity_key`, `discover_by_attributes` | — |
| **Domain Perms** | `get_domain_permission`, `set_domain_permission`, `delete_domain_permission`, `list_domain_permissions`, `check_cert_permissions`, `approve_cert_fields`, `domain_permissions_reset_all` | 8382–8700 |
| **Messages (BRC-33)** | `send_message`, `list_messages`, `acknowledge_message` | 8727–9000 |
| **PeerPay (BRC-29)** | `peerpay_send`, `peerpay_check`, `peerpay_status`, `peerpay_dismiss` | 12234+ |
| **Paymail** | `paymail_send`, `paymail_resolve`, `recipient_resolve` (unified: identity/paymail/BSV address) | end |
| **Settings/Price** | `get_bsv_price`, `wallet_settings_get`, `wallet_settings_set`, `reveal_mnemonic`, `get_sync_status`, `mark_sync_seen` | 8353+ |

## Key Types

### AppState (main.rs)
Global application state shared across all handlers via `web::Data<Arc<AppState>>`:
```rust
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub balance_cache: Arc<BalanceCache>,
    pub auth_sessions: Arc<AuthSessionManager>,
    pub fee_rate_cache: Arc<FeeRateCache>,
    pub price_cache: Arc<PriceCache>,
    pub user_id: Mutex<Option<i64>>,
    pub shutdown_token: CancellationToken,
    pub create_action_lock: Mutex<()>,
    pub derived_key_cache: DashMap<String, DerivedKeyInfo>,
    // ...
}
```

### Status Enums (action_storage.rs)
```rust
enum TransactionStatus {
    Completed, Unprocessed, Sending, Unproven,
    Unsigned, Nosend, Nonfinal, Failed
}

enum ProvenTxReqStatus {
    Unknown, Sending, Unsent, Nosend,
    Unproven, Invalid, Unmined, Callback, Completed
}
```

### BEEF Markers (beef.rs)
```rust
const BEEF_V1_MARKER: [u8; 4] = [0x01, 0x00, 0xbe, 0xef];
const BEEF_V2_MARKER: [u8; 4] = [0x02, 0x00, 0xbe, 0xef];  // default
const ATOMIC_BEEF_MARKER: [u8; 4] = [0x01, 0x01, 0x01, 0x01];  // BRC-95
```

## Key Constants

| Constant | Value | Location |
|----------|-------|----------|
| `DEFAULT_SATS_PER_KB` | 1000 | `fee_rate_cache.rs` |
| `MIN_FEE_SATS` | 200 | `handlers.rs` |
| `MAX_BEEF_ANCESTORS` | 50 | `beef_helpers.rs` |
| Balance cache TTL | 60 seconds | `balance_cache.rs` |
| Fee rate cache TTL | 1 hour | `fee_rate_cache.rs` |
| Price cache TTL | 5 minutes | `price_cache.rs` |
| Identity cache TTL | 10 minutes | `identity_resolver.rs` |
| AuthSession expiry | 24 hours | `auth_session.rs` |
| JSON payload limit | 10 MB | `main.rs` |
| BEEF payload limit | 100 MB | `main.rs` |
| Price sanity range | $0.01–$10,000 | `price_cache.rs` |
| Fee sanity range | 100–10,000 sat/KB | `fee_rate_cache.rs` |

## Critical Patterns

### Database Lock Scoping
Database locks must be dropped before any `await`:
```rust
let result = {
    let db = db.lock().unwrap();
    db.connection().execute(...)
}; // lock dropped here
external_api_call().await; // safe
```

### Stale Cache Fallback
`BalanceCache` returns stale data rather than blocking on DB:
```rust
// Fresh cache → use it
// Stale cache → return it anyway (prevents UI freeze)
// No cache → compute from DB
```

### Domain Permission Defense-in-Depth
`check_domain_approved()` guards handlers even though C++ also checks permissions. The `X-Requesting-Domain` header is added by C++ — internal requests (wallet panel) omit it and skip the check.

### Derived Key Caching
`get_public_key(forSelf=true)` caches the derivation info (invoice + counterparty) in `derived_key_cache`. Later, `sign_action` uses this cache to find the correct BRC-42 key for PushDrop signing.

### App-Scoped Identity Keys
`well_known_auth` derives an app-scoped identity key via BRC-42 (`invoice: "2-identity"`, counterparty: app's key) to prevent cross-domain tracking.

### Atomic Action Creation
`create_action` holds `create_action_lock` while selecting UTXOs, building the transaction, and inserting into the database. If the process crashes between UTXO selection and DB insert, `monitor/task_fail_abandoned` cleans up on restart.

### BEEF Ancestry Limits
`build_beef_for_txid()` enforces `MAX_BEEF_ANCESTORS = 50` to prevent runaway ancestry walks. Confirmed transactions with BUMPs don't include parents (they already have merkle proof).

## External API Dependencies

| API | Module | Endpoint | Purpose |
|-----|--------|----------|---------|
| WhatsOnChain | `utxo_fetcher.rs`, `cache_helpers.rs` | `/v1/bsv/main/address/{addr}/unspent` | UTXO fetch |
| WhatsOnChain | `cache_helpers.rs` | `/v1/bsv/main/tx/{txid}/hex` | Raw TX fetch |
| WhatsOnChain | `cache_helpers.rs` | `/v1/bsv/main/tx/{txid}/proof/tsc` | TSC merkle proof |
| ARC (GorillaPool) | `fee_rate_cache.rs` | `/v1/policy` | Mining fee rate |
| ARC (GorillaPool) | `cache_helpers.rs` | `/v1/tx/{txid}/bump` | BUMP merkle proof |
| CryptoCompare | `price_cache.rs` | `/data/price?fsym=BSV&tsyms=USD` | BSV/USD price (primary) |
| CoinGecko | `price_cache.rs` | `/simple/price?ids=bitcoin-sv&vs_currencies=usd` | BSV/USD price (fallback) |
| MessageBox | `messagebox.rs` | `messagebox.babbage.systems` | BRC-2 encrypted message relay |
| BSV Overlay | `identity_resolver.rs` | US/EU overlay endpoints | BRC-52 identity certificate lookup |
| Paymail hosts | `paymail.rs` | `/.well-known/bsvalias` | bsvalias capability discovery |

## Adding a New Endpoint

1. Add handler function in `handlers.rs` following existing patterns
2. Register route in `main.rs` under the appropriate `.service()` block
3. If it needs domain permission checking, call `check_domain_approved()` at the top
4. If it modifies balance, call `state.balance_cache.invalidate()`
5. If it needs a large payload, add `.app_data(web::PayloadConfig::new(limit))`

## Related

- `../CLAUDE.md` — Rust wallet layer overview, build instructions, invariants
- `database/CLAUDE.md` — SQLite schema, repositories, migrations
- `crypto/CLAUDE.md` — BRC-42, BRC-43, signing, encryption modules
- `certificate/CLAUDE.md` — BRC-52 certificate management
- `transaction/CLAUDE.md` — Bitcoin SV transaction parsing and building
- `script/CLAUDE.md` — Script parsing and PushDrop encoding
- `monitor/` — Background task scheduler (7 tasks: proofs, sync, PeerPay, cleanup)
- `/CLAUDE.md` — Root project documentation with full architecture overview
