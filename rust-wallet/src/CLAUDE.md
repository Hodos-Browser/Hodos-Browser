# Rust Wallet Source Modules
> HTTP handlers, caching, external API clients, and supporting modules for the wallet backend

**Last Updated:** 2026-08-03

## Overview

This directory contains the core source modules for the Actix-web wallet server. `main.rs` bootstraps the app state and HTTP server on **127.0.0.1:31301** (release) / **31401** when `HODOS_DEV=1` — see `main.rs :: wallet_port`. The canonical cross-layer port table lives in `cef-native/include/core/PortConfig.h`; the adblock engine uses 31302 / 31402.

`handlers.rs` (~19,270 lines) declares **98 `pub async fn`** handler-shaped functions, **96** of which are wired to routes. It re-exports 10 more from `handlers/certificate_handlers.rs` (~6,210 lines, 11 `pub async fn`). `permission_service/handlers.rs` adds 3.

**Totals as registered in `main.rs`: 110 `.route(...)` registrations covering 109 distinct handler functions** (`/getVersion` is registered for both GET and POST). Split: 96 from `handlers.rs` + 10 from `handlers/certificate_handlers.rs` + 3 from `permission_service/handlers.rs`.

The remaining modules provide caching, external API integration (WhatsOnChain, ARC, JungleBus, Bitails, MessageBox, Paymail, BSV Overlay), BEEF transaction parsing, wallet recovery, on-chain backup, and the Rust permission engine wiring.

Subdirectories `crypto/`, `database/`, `transaction/`, `certificate/`, `script/`, and `monitor/` each have their own CLAUDE.md. **`handlers/`, `services/`, `overlay/`, and `permission_service/` do NOT** — they are documented inline here.

## Files

| File | Purpose |
|------|---------|
| `main.rs` | Entry point: `wallet_port()` (31301 / 31401), `app_dir_name()` (HodosBrowser / HodosBrowserDev), `enforce_dev_safeguard()`, `DerivedKeyInfo`, `AppState` struct (18 fields), `domain_trust_mw` universal Actix middleware (Phase 2.6-G), Actix-web server init, **110 route registrations**, DPAPI/Keychain auto-unlock, Monitor startup |
| `lib.rs` | Library exports for `cargo test` — declares 19 `pub mod`s (`arc_status`, `crypto`, `certificate`, `database`, `transaction`, `recovery`, `action_storage`, `json_storage`, `cache_errors`, `utxo_fetcher`, `beef`, `script`, `balance_cache`, `price_cache`, `overlay`, `services`, `permission_service`, `manifest`, `reconcile`) + `pub use crypto::brc2` and `crypto::aesgcm_custom` |
| `handlers.rs` | 98 `pub async fn` handlers (96 routed) grouped by protocol area (see Handler Groups below). Also `check_domain_approved()` (29 call sites), constants `DEFAULT_SATS_PER_KB`, `MIN_FEE_SATS`, `HODOS_FEE_ADDRESS`, `HODOS_SERVICE_FEE_SATS`, `YOURS_LEGACY_RECEIVE_PROTOCOL`, `PAY402_REUSE_TTL_MS`. Two `#[cfg(test)]` modules: `address_to_script_tests`, `yours_legacy_addresses_tests` |
| `handlers/certificate_handlers.rs` | BRC-52 certificate handlers — 11 `pub async fn`, 10 re-exported through `handlers.rs` and routed. **Note the path**: it is `handlers/certificate_handlers.rs`, NOT `certificate/handlers.rs` (`certificate/` holds parsing/verification types only) |
| `action_storage.rs` | `ActionStatus` (7 variants), `TransactionStatus` (8), `ProvenTxReqStatus` (9) enums; legacy `ActionStorage` (HashMap-backed, superseded by database) |
| `arc_status.rs` | Centralized ARC miner response classification: `ArcTxStatus` enum, `mark_inputs_suspected()`, `is_fatal_broadcast_error()`, `is_frozen_input_error()`, `is_double_spend_error()`; consts `SUSPECTED_DOUBLE_SPEND_PREFIX` (`"dss:"`), `CONFIRMED_DOUBLE_SPEND` |
| `auth_session.rs` | `AuthSessionManager` / `AuthSession` — BRC-103/104 session tracking, 24-hour expiry (`now + 24*60*60`), `cleanup_expired_sessions` |
| `authfetch.rs` | `AuthFetchClient` — BRC-103/104 authenticated HTTP client for MessageBox and overlay services |
| `backup.rs` | Encrypted wallet export/import + on-chain backup. `EncryptedBackup`, `BackupPayload` and **22 `Backup*` row structs** (wallet, user, address, basket, transaction, output, proven_tx, proven_tx_req, certificate, certificate_field, output_tag, output_tag_map, tx_label, tx_label_map, commission, setting, sync_state, parent_transaction, block_header, domain_permission, cert_field_permission). Fns: `collect_payload`, `encrypt_backup`, `decrypt_backup`, `serialize_for_onchain`, `compress_for_onchain`, `encrypt_compressed`, `deserialize_from_onchain` |
| `balance_cache.rs` | `BalanceCache` — in-memory balance with 60s TTL, stale fallback, and invalidation API |
| `beef.rs` | BRC-62 BEEF parser: V1/V2/Atomic markers, `MerkleProof`, `ParsedTransaction`, raw TX extraction, `tsc_proof_to_bump`, `parse_bump_hex_to_tsc` |
| `beef_helpers.rs` | BEEF building helpers: `build_beef_for_txid()` with ancestry walk (`MAX_BEEF_ANCESTORS = 50`), `fetch_transaction_for_beef()` |
| `cache_errors.rs` | `CacheError` enum (Database, Api, InvalidData, HexDecode, Json) and `CacheResult<T>` alias |
| `cache_helpers.rs` | Shared SPV cache functions: `fetch_parent_transaction_from_api()`, `fetch_tsc_proof_from_api()`, `fetch_and_cache_block_header()`, `get_cached_block_height()`, `get_utxo_id_from_db()` — all take `&WalletServices` and route through the provider chains. Plus `verify_txid()` and `verify_tsc_proof_against_block()` (latter still uses a raw `reqwest::Client`) |
| `fee_rate_cache.rs` | `FeeRateCache` — 1-hour TTL (`CACHE_TTL_SECONDS = 3600`), fetches `https://arc.gorillapool.io/v1/policy`, defaults to `DEFAULT_SATS_PER_KB = 1000`, sanity range 100–10,000 sat/KB, `FEE_RATE_VALIDATION_HASH` |
| `identity_resolver.rs` | `IdentityResolver` — resolves identity keys to names/avatars via BSV Overlay Services (BRC-52 certificates), `CACHE_TTL_SECS = 600` (10 min), 3 `STATIC_OVERLAY_ENDPOINTS` (US/EU/AP `*.bsvb.tech/lookup`), certifier + cert-type constants (MetaNet, SocialCert; Twitter/Discord/Email/GovID/Registrant) |
| `json_storage.rs` | Legacy JSON file storage (`Wallet`, `AddressInfo`). Superseded by database; kept for backward compatibility |
| `manifest.rs` | Phase 2.6-G Rust port of the C++ `ManifestFetcher`. `manifest_url()`, `parse_manifest()` (pure + lenient, never panics), `fetch_manifest()`. Types: `Manifest`, `ManifestProtocol`, `ManifestBasket`, `ManifestCertificate`, `ManifestSpending`, `ManifestCounterparty` |
| `message_relay.rs` | `Message`, `MessageStore`, `MessageStoreStats` — in-memory BRC-33 PeerServ message relay (recipient → box → messages). Used by messagebox/monitor |
| `messagebox.rs` | `MessageBoxClient` — BRC-2 encrypted messaging via `MESSAGEBOX_URL = "https://messagebox.babbage.systems"`, uses AuthFetch for transport |
| `paymail.rs` | `PaymailClient` — bsvalias capability discovery (`CAP_P2P_DESTINATION`, `CAP_P2P_RECEIVE_TX`, `CAP_PUBLIC_PROFILE`), `CAPABILITY_CACHE_TTL_SECS = 3600`, `SRV_OVERRIDES`. Handles HandCash `$alias` shorthand |
| `price_cache.rs` | `PriceCache` — BSV/USD, `CACHE_TTL_SECONDS = 300`. Provider order is **WhatsOnChain → CoinGecko → MEXC** (`fetch_bsv_price`); `validate_price` enforces a $0.01–$10,000 sanity range. Persists the last good value to the `bsv_price_cache` SQLite table (`load_persisted` on cold start), so `get_stale()` survives restarts |
| `reconcile.rs` | Wallet-Hardening WS1 spent-input reconcile primitives. `SpentStatus`, `UnspentProbe`, `ReconcileReport`; `check_outpoint_spent()`, `check_outpoint_unspent()`, `derive_receive_address()`, `recover_change_index()`, `parse_tx_outputs()`, `verify_raw_txid()`. c1 is live; c2/c3 dormant |
| `recovery.rs` | `derive_private_key_bip32()` for legacy `m/{index}` outputs, `recover_wallet_from_mnemonic()` with gap-limit scanning (BIP32 + BRC-42), `derive_key_at_path()`, `derive_address_at_path()`, `pubkey_to_address()`, `address_to_p2pkh_script()`. External-wallet sweep: `ExternalWalletConfig`, `ExternalUTXO`, `ExternalScanResult`, `scan_external_wallet()`, `build_sweep_transactions()` |
| `utxo_fetcher.rs` | `UTXO` struct; `fetch_utxos_for_address()`, `fetch_utxos_single_address_with_unconfirmed()`, `fetch_all_utxos()` (bulk, `BULK_BATCH_SIZE = 20`) with exponential backoff (`MAX_RETRIES = 3`, `INITIAL_DELAY_MS = 1000`), `address_has_history()` for gap-limit scanning |

### Subdirectories without their own CLAUDE.md

| Dir | Files | Contents |
|-----|-------|----------|
| `handlers/` | 1 | `certificate_handlers.rs` — see Files table above |
| `services/` | 4 + `providers/` (8) | Phase 1.6d indexer facade. `mod.rs` → `WalletServices` (7 per-op provider chains) + `WalletServicesStats`; `provider.rs` → `IndexerProvider` trait, `IndexerError`, `BlockKey`, `BlockHeader`, `TxStatus`, `TxState`, `OutspendStatus`, `BroadcastResult`, `ProviderOp`; `collection.rs` → `ProviderCollection` fallback runner; `call_class.rs` → `CallClass` (IndexerSync 8s / IndexerAsync 15s / IndexerBulk 30s / ThirdPartyNoFallback 240s). **7 providers**: `arc_gorillapool.rs`, `arc_taal.rs`, `gorillapool_mapi.rs`, `gorillapool_ordinals.rs`, `whatsonchain.rs`, `junglebus.rs`, `bitails.rs` |
| `overlay/` | 2 | `mod.rs` — SHIP-discovery + overlay submit/lookup for `TOPIC_IDENTITY = "tm_identity"`; `submit_to_identity_overlay`, `submit_to_topic`, `lookup_published_certificate`, `lookup_certificates_by_identity_key`, `OverlayCertificateOutput`. Public API takes `&Arc<ShipDiscoveryCache>`, not `&AppState`. `ship_cache.rs` — `ShipDiscoveryCache` SWR cache (`FRESH_TTL` 300s, `STALE_TTL` 1800s, `CacheStatus`); no-poison invariant (empty fetch results never stored) |
| `permission_service/` | 6 + `context_builder/` (1) | Actix wrapper around the pure `hodos_permission_engine` crate. `mod.rs`, `state.rs` (`PermissionService`, `PendingApproval`, `SessionCounters`, `ApprovalConsumeError`), `request_gate.rs` (`domain_trust_gate`, `dispatch_privacy_perimeter`, `dispatch_payment`, `dispatch_scoped_grant`, `dispatch_cert_disclosure`, `GateOutcome`, `PaymentCall`, `ScopedCall`, `is_protected_basket`, `X_BROWSER_ID`), `audit.rs`, `context_builder.rs` + `context_builder/sensitive_cert_fields.rs`, `handlers.rs` (3 routed handlers) |

## Handler Groups

**109 routed handler functions across 110 route registrations.** Counts below sum to 109.

| Group | Count | Key Handlers | Source |
|-------|-------|-------------|--------|
| **Server Control** | 3 | `health`, `shutdown`, `brc100_status` | `handlers.rs` |
| **BRC-100 Identity** | 5 | `get_version` (GET+POST), `get_public_key`, `is_authenticated`, `wait_for_authentication`, `well_known_auth` | `handlers.rs` |
| **BRC-100 Crypto** | 6 | `create_hmac`, `verify_hmac`, `encrypt`, `decrypt`, `create_signature`, `verify_signature` | `handlers.rs` |
| **BRC-72 Key Linkage** | 2 | `reveal_counterparty_key_linkage`, `reveal_specific_key_linkage` | `handlers.rs` |
| **Actions (dApp-facing)** | 7 | `create_action`, `sign_action`, `process_action`, `abort_action`, `list_actions`, `internalize_action`, `update_confirmations_endpoint` | `handlers.rs` |
| **Internal Send** | 1 | `send_transaction` — see "Two send paths" below | `handlers.rs` |
| **Outputs** | 3 | `list_outputs` (with BEEF building), `relinquish_output`, `list_token_outputs` | `handlers.rs` |
| **Blockchain** | 3 | `get_height`, `get_header_for_height`, `get_network` | `handlers.rs` |
| **Certificates** | 10 | `acquire_certificate`, `list_certificates`, `prove_certificate`, `relinquish_certificate`, `discover_by_identity_key`, `discover_by_attributes`, `publish_certificate`, `unpublish_certificate`, `cleanup_overlay_certificates`, `admin_prepare_unpublish` | `handlers/certificate_handlers.rs` |
| **Debug** | 3 | `debug_validate_beef`, `debug_repair_nosend`, `debug_broadcast_nosend` | `handlers.rs` |
| **Wallet Mgmt** | 19 | `wallet_status`, `wallet_create`, `wallet_delete`, `wallet_balance`, `wallet_sync`, `wallet_backup`, `wallet_backup_onchain`, `wallet_backup_onchain_verify`, `wallet_recover_onchain`, `wallet_restore`, `wallet_unlock`, `wallet_recover`, `wallet_recover_external`, `wallet_rescan`, `wallet_cleanup`, `wallet_consolidate_dust`, `wallet_export`, `wallet_import`, `wallet_activity` | `handlers.rs` |
| **Addresses** | 5 | `generate_address`, `get_all_addresses`, `get_current_address`, `yours_legacy_addresses`, `address_to_script` | `handlers.rs` |
| **Legacy BIE1** | 2 | `encrypt_bie1_handler`, `decrypt_bie1_handler` (Phase 2 Step 3c.2 — ECIES Electrum for Yours-era dApps) | `handlers.rs` |
| **Domain Perms (top-level)** | 8 | `get_domain_permission`, `set_domain_permission`, `delete_domain_permission`, `list_domain_permissions`, `check_cert_permissions`, `approve_cert_fields`, `revoke_cert_fields`, `domain_permissions_reset_all` | `handlers.rs` |
| **Domain Sub-Perms (V18 child tables)** | 9 | `grant`/`revoke`/`list` × `protocol_permission`, `basket_permission`, `counterparty_permission` | `handlers.rs` |
| **Permission Sessions** | 3 | `session_approve`, `session_revoke`, `session_close` | `permission_service/handlers.rs` |
| **Messages (BRC-33)** | 3 | `send_message`, `list_messages`, `acknowledge_message` | `handlers.rs` |
| **PeerPay (BRC-29)** | 5 | `peerpay_send`, `peerpay_check`, `peerpay_status`, `peerpay_dismiss`, `peerpay_outbox_retry` | `handlers.rs` |
| **BRC-121 (HTTP 402)** | 2 | `pay_402`, `broadcast_nosend` | `handlers.rs` |
| **Paymail** | 2 | `paymail_send`, `paymail_resolve` | `handlers.rs` |
| **Recipient Resolution** | 2 | `recipient_resolve` (unified: identity/paymail/BSV address), `recipient_suggest` | `handlers.rs` |
| **Settings / Price / Sync** | 6 | `get_bsv_price`, `get_sync_status`, `mark_sync_seen`, `wallet_settings_get`, `wallet_settings_set`, `reveal_mnemonic` | `handlers.rs` |

**Declared but NOT routed:** `handlers.rs :: update_confirmations` and `handlers.rs :: do_onchain_backup` are internal helpers (the routed endpoint is `update_confirmations_endpoint`); `handlers/certificate_handlers.rs :: auto_unpublish_certificate_pub` is called from the Monitor, not from a route.

### Two send paths — `send_transaction` vs `create_action`

These are **distinct entry points and must not be conflated**:

| | `send_transaction` | `create_action` |
|---|---|---|
| Route | `POST /transaction/send` | `POST /createAction` |
| Caller | **Internal only** — the wallet-panel / React send UI | **dApp-facing** — BRC-100 clients via the IPC shim or `Open()` |
| Request type | `SendTransactionRequest` (`toAddress`, `amount`, `feeRate`, `sendMax`) | `CreateActionRequest` (BRC-100 shape: outputs, labels, `lockTime`, `version`, `inputBEEF`, …) |
| Domain gate | None — no `X-Requesting-Domain`, so `domain_trust_mw` passes it straight through | Full permission cascade (domain trust middleware + payment gate) |
| Payload cap | default (10 MB JSON) | 100 MB `PayloadConfig` (for `inputBEEF`) |
| Implementation | Validates address/amount, builds a `CreateActionRequest` with `no_send: true` + `accept_delayed_broadcast: false`, calls `create_action(...)` with a **synthetic domain-less `HttpRequest`** (`actix_web::test::TestRequest::default().to_http_request()`), then broadcasts the returned Atomic BEEF itself via `broadcast_transaction` | The BRC-100 handler proper |

`send_transaction` reuses the same transaction-building core, so both paths pay the 1000-sat Hodos service fee. The difference is **who is allowed to call them**: `send_transaction` deliberately synthesizes a request with no `X-Requesting-Domain`, which is exactly what makes it internal-only. Never expose `/transaction/send` to dApp origins — doing so would hand a caller an ungated spend path.

## Key Types

### AppState (main.rs)
Global application state shared across all handlers via `web::Data<AppState>`. **18 fields:**
```rust
pub struct AppState {
    pub database: Arc<Mutex<WalletDatabase>>,
    pub auth_sessions: Arc<AuthSessionManager>,
    pub balance_cache: Arc<balance_cache::BalanceCache>,
    pub fee_rate_cache: Arc<fee_rate_cache::FeeRateCache>,
    pub price_cache: Arc<price_cache::PriceCache>,
    pub services: Arc<services::WalletServices>,                       // 1.6d.B indexer facade
    pub ship_cache: Arc<overlay::ship_cache::ShipDiscoveryCache>,      // SHIP host SWR cache
    pub utxo_selection_lock: Arc<tokio::sync::Mutex<()>>,
    pub create_action_lock: Arc<tokio::sync::Mutex<()>>,
    pub derived_key_cache: Arc<Mutex<HashMap<String, DerivedKeyInfo>>>,
    pub current_user_id: i64,
    pub shutdown: tokio_util::sync::CancellationToken,
    pub sync_status: Arc<std::sync::RwLock<handlers::SyncStatus>>,
    pub backup_check_needed: Arc<Mutex<Option<(i64, i64)>>>,           // (first_event_ts, latest_event_ts)
    pub recovery_just_completed: Arc<std::sync::atomic::AtomicBool>,
    pub pay402_reuse: Arc<Mutex<HashMap<(String, i64), handlers::Pay402ReuseEntry>>>,
    pub permission: Arc<permission_service::PermissionService>,        // Rust permission engine wrapper
}
```
`impl AppState` also provides `request_backup_check()` and `request_backup_check_if_significant(satoshis)` (fires when the spend is ≥ $3.00 at the cached price).

`DerivedKeyInfo { invoice: String, counterparty_pubkey: Vec<u8> }` is also defined in `main.rs`.

> **Invariant:** do not add or remove `AppState` fields without auditing every handler that reads them.

### Status Enums (action_storage.rs)
```rust
enum ActionStatus {          // 7
    Created, Signed, Unconfirmed, Pending, Confirmed, Aborted, Failed
}

enum TransactionStatus {     // 8
    Completed, Unprocessed, Sending, Unproven,
    Unsigned, Nosend, Nonfinal, Failed
}

enum ProvenTxReqStatus {     // 9
    Unknown, Sending, Unsent, Nosend,
    Unproven, Invalid, Unmined, Callback, Completed
}
```

### BEEF Markers (beef.rs)
```rust
pub const BEEF_V1_MARKER: [u8; 4] = [0x01, 0x00, 0xbe, 0xef];
pub const BEEF_V2_MARKER: [u8; 4] = [0x02, 0x00, 0xbe, 0xef];
pub const ATOMIC_BEEF_MARKER: [u8; 4] = [0x01, 0x01, 0x01, 0x01];  // BRC-95
pub const BEEF_VERSION_MARKER: [u8; 4] = BEEF_V2_MARKER;           // default on write
```

## Key Constants

| Constant | Value | Location |
|----------|-------|----------|
| Wallet HTTP port | 31301 release / 31401 under `HODOS_DEV=1` | `main.rs :: wallet_port` |
| Data dir name | `HodosBrowser` / `HodosBrowserDev` | `main.rs :: app_dir_name` |
| `DEFAULT_SATS_PER_KB` | 1000 | `fee_rate_cache.rs` (and a `pub` copy in `handlers.rs`) |
| `MIN_FEE_SATS` | 200 | `handlers.rs` |
| `HODOS_FEE_ADDRESS` | `1Q1A2rq6trBdptd3t6n53vB79mRN6JHEFT` | `handlers.rs` |
| `HODOS_SERVICE_FEE_SATS` | 1000 | `handlers.rs` |
| `MAX_BEEF_ANCESTORS` | 50 | `beef_helpers.rs` |
| `PAY402_REUSE_TTL_MS` | 25,000 ms | `handlers.rs` |
| `BULK_BATCH_SIZE` / `MAX_RETRIES` | 20 / 3 (backoff from 1000 ms) | `utxo_fetcher.rs` |
| Balance cache TTL | 60 seconds | `balance_cache.rs` |
| Fee rate cache TTL | 1 hour (`CACHE_TTL_SECONDS = 3600`) | `fee_rate_cache.rs` |
| Price cache TTL | 5 minutes (`CACHE_TTL_SECONDS = 300`) | `price_cache.rs` |
| Identity cache TTL | 10 minutes (`CACHE_TTL_SECS = 600`) | `identity_resolver.rs` |
| Paymail capability cache TTL | 1 hour (`CAPABILITY_CACHE_TTL_SECS = 3600`) | `paymail.rs` |
| SHIP cache fresh / stale TTL | 300 s / 1800 s | `overlay/ship_cache.rs` |
| AuthSession expiry | 24 hours | `auth_session.rs` |
| JSON payload limit | 10 MB (`JsonConfig`) | `main.rs` |
| Default payload limit | 100 MB (`PayloadConfig`) | `main.rs` |
| Per-route 100 MB overrides | `/createAction`, `/signAction`, `/wallet/import` | `main.rs` |
| Price sanity range | $0.01–$10,000 | `price_cache.rs :: validate_price` |
| Fee sanity range | 100–10,000 sat/KB | `fee_rate_cache.rs` |
| Default spending limits | $1.00 per tx / $10.00 per session (100 / 1000 USD cents) | `database/migrations.rs` (`per_tx_limit_cents DEFAULT 100`) |
| `CallClass` timeouts | 8 s / 15 s / 30 s / 240 s | `services/call_class.rs` |
| `TOPIC_IDENTITY` | `tm_identity` | `overlay/mod.rs` |

## Critical Patterns

### The permission decision engine is Rust, not C++
The pure decision logic lives in the workspace crate `rust-wallet/crates/hodos_permission_engine` (`decide()` in `src/lib.rs`, the Matrix C cascade in `src/matrix_c.rs`, plus `context.rs` and `decision.rs`). `permission_service/` is the Actix wrapper (`PermissionService`, pending-approval map, audit log, per-CallKind context builders, the `request_gate` dispatchers). **The C++ `PermissionEngine` and `SessionManager` were deleted in Phase 2.6-H** — do not reintroduce a parallel C++ decision path.

### Universal domain-trust middleware (`main.rs :: domain_trust_mw`)
Registered with `.wrap(middleware::from_fn(domain_trust_mw))` ahead of every route:
- **No `X-Requesting-Domain` header** → internal (wallet-UI) call → pass straight through.
- Header present → `permission_service::domain_trust_gate(...)`:
  - approved → `Proceed` (the handler's own kind-specific gate, if any, runs after)
  - blocked → `403`
  - unknown → `202` (`domain_approval` / `manifest_connect_bundle`); C++ opens the connect modal, writes `trust=approved` synchronously on Approve, and re-issues.

This single choke-point makes Rust authoritative for domain trust across **both** transports (the IPC shim and direct-fetch `Open()`).

### Domain Permission Defense-in-Depth
Even with the middleware, `handlers.rs :: check_domain_approved` still guards individual handlers (29 call sites), and the four privacy-perimeter paths additionally route through `permission_service::request_gate::dispatch_privacy_perimeter` (21 `dispatch_*` call sites in `handlers.rs`). Internal requests omit `X-Requesting-Domain` and skip all of it.

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
```
// Fresh cache → use it
// Stale cache → return it anyway (prevents UI freeze)
// No cache → compute from DB
```
`PriceCache` goes one further: the last good price is persisted to the `bsv_price_cache` SQLite table and reloaded by `load_persisted()` on cold start, so `get_stale()` has a value even on the first request after a restart. (A dead price makes the permission engine return `Prompt(price_unavailable)` instead of `Silent`.)

### Derived Key Caching
`get_public_key(forSelf=true)` caches the derivation info (invoice + counterparty) in `derived_key_cache`. Later, `sign_action` uses this cache to find the correct BRC-42 key for PushDrop signing.

### App-Scoped Identity Keys
`well_known_auth` derives an app-scoped identity key via BRC-42 (fixed invoice `"2-identity"`, counterparty: the app's key) to prevent cross-domain tracking.

### Atomic Action Creation
`create_action` holds `create_action_lock` while selecting UTXOs, building the transaction, and inserting into the database. A separate `utxo_selection_lock` (tokio async mutex) prevents concurrent UTXO selection races. If the process crashes between UTXO selection and DB insert, `monitor/task_fail_abandoned` cleans up on restart.

### Recovery Sync Progress
`sync_status: Arc<RwLock<SyncStatus>>` tracks wallet recovery progress (addresses scanned, UTXOs found) and is polled by the frontend via `/wallet/sync-status`. Cleared by `/wallet/sync-status/seen`.

### External Wallet Recovery
`wallet_recover_external` (`/wallet/recover-external`) sweeps funds from external wallets. `recovery.rs :: scan_external_wallet` scans external derivation paths, and `build_sweep_transactions` sweeps found UTXOs into the Hodos wallet on-chain.

### On-Chain Backup
`backup.rs` supports both file backups (`encrypt_backup` / `decrypt_backup`) and on-chain backups (`serialize_for_onchain` → `compress_for_onchain` → `encrypt_compressed`, restored by `deserialize_from_onchain`). Routed via `/wallet/backup/onchain`, `/wallet/backup/onchain/verify`, `/wallet/recover/onchain`. `monitor/task_backup` drives the timer, and `AppState::request_backup_check_if_significant` nudges it after a ≥ $3.00 spend.

### Event-Debounced Backup Timer
`backup_check_needed: Option<(first_event_ts, latest_event_ts)>` — backup runs 3 minutes after the **latest** event, with a hard cap of 10 minutes from the **first**, so a burst of activity can't defer it forever.

### BRC-121 Retry Reuse
`pay402_reuse: HashMap<(url, sats), Pay402ReuseEntry>` holds the unbroadcast retry context for ~25 s (`PAY402_REUSE_TTL_MS`) so a paid retry that has to re-issue doesn't mint a second payment. `pay_402` mints a nosend BEEF; `broadcast_nosend` broadcasts only after the paid retry returns 200.

### BEEF Ancestry Limits
`build_beef_for_txid()` enforces `MAX_BEEF_ANCESTORS = 50` to prevent runaway ancestry walks. Confirmed transactions with BUMPs don't include parents (they already have a merkle proof).

### Provider Fallback Chains (`services/mod.rs :: WalletServices::new`)
Seven per-operation chains, tried in order:

| Op | Chain |
|----|-------|
| `get_raw_tx` | ARC GorillaPool → WhatsOnChain → JungleBus |
| `get_merkle_proof_tsc` | ARC GorillaPool → WhatsOnChain → JungleBus |
| `get_block_header` | WhatsOnChain → JungleBus |
| `tx_status` | ARC GorillaPool → WhatsOnChain → JungleBus → Bitails |
| `outspend` | WhatsOnChain → JungleBus |
| `fetch_utxos` | WhatsOnChain → GorillaPool Ordinals |
| `broadcast_beef` | ARC GorillaPool → ARC TAAL → GorillaPool mAPI → WhatsOnChain |

Bitails is deliberately **demoted off** the raw_tx / proof / header chains: it returns HTTP 500 instead of 404 for unknown txids, which poisons error messages. It is kept on `tx_status`, where its response shape is reliable.

### Background Tasks
`monitor/` registers **14 tasks** (`monitor/mod.rs :: TaskSchedule` has 14 fields and the run loop dispatches all 14 on a 30 s tick): `check_for_proofs` 60 s, `send_waiting` 120 s, `fail_abandoned` 300 s, `unfail` 300 s, `review_status` 60 s, `purge` 3600 s, `sync_pending` 30 s, `check_peerpay` 60 s, `backup` 10800 s, `replay_overlay` 300 s, `consolidate_dust` 86400 s, `verify_double_spend` 60 s, `retry_peerpay_outbox` 30 s, `refresh_ship_cache` 300 s. `MONITOR_STARTED: AtomicBool` prevents duplicate loops. See `monitor/CLAUDE.md` for per-task detail.

## External API Dependencies

| API | Module | Endpoint | Purpose |
|-----|--------|----------|---------|
| WhatsOnChain | `utxo_fetcher.rs`, `services/providers/whatsonchain.rs` | `/v1/bsv/main/address/{addr}/unspent` | UTXO fetch (1st tier) |
| WhatsOnChain | `services/providers/whatsonchain.rs` | `/v1/bsv/main/tx/{txid}/hex`, `/proof/tsc` | Raw TX + TSC proof (2nd tier) |
| WhatsOnChain | `price_cache.rs` | `/v1/bsv/main/exchangerate` | BSV/USD price (**primary**) |
| ARC (GorillaPool) | `fee_rate_cache.rs` | `https://arc.gorillapool.io/v1/policy` | Mining fee rate |
| ARC (GorillaPool) | `services/providers/arc_gorillapool.rs` | `/v1/tx/{txid}` + `/bump` | Raw TX, BUMP proof, tx status, broadcast (1st tier) |
| ARC (TAAL) | `services/providers/arc_taal.rs` | `https://arc.taal.com` | Broadcast fallback only |
| GorillaPool mAPI | `services/providers/gorillapool_mapi.rs` | `https://mapi.gorillapool.io` | Broadcast fallback |
| GorillaPool Ordinals | `services/providers/gorillapool_ordinals.rs` | `https://ordinals.gorillapool.io` | UTXO fallback |
| JungleBus | `services/providers/junglebus.rs` | `https://junglebus.gorillapool.io` | Raw TX / proof / header / outspend fallback |
| Bitails | `services/providers/bitails.rs` | `https://api.bitails.io` | `tx_status` only (demoted elsewhere) |
| CoinGecko | `price_cache.rs` | `/simple/price?ids=bitcoin-cash-sv&vs_currencies=usd` | BSV/USD price (2nd) |
| MEXC | `price_cache.rs` | `/api/v3/ticker/price?symbol=BSVUSDT` | BSV/USD price (3rd) |
| MessageBox | `messagebox.rs` | `https://messagebox.babbage.systems` | BRC-2 encrypted message relay |
| BSV Overlay | `overlay/mod.rs`, `identity_resolver.rs` | `overlay-{us,eu,ap}-1.bsvb.tech`, `users.bapp.dev` | SHIP discovery, cert publish/lookup, BRC-52 identity |
| Paymail hosts | `paymail.rs` | `/.well-known/bsvalias` | bsvalias capability discovery, P2P destinations |

## Adding a New Endpoint

1. Add the handler function in `handlers.rs` (or the appropriate submodule) following existing patterns.
2. Register the route in `main.rs` under the appropriate section comment. `domain_trust_mw` covers it automatically — no opt-in needed.
3. If it needs per-handler domain permission checking beyond trust, call `check_domain_approved()` at the top, or route it through the matching `permission_service::request_gate::dispatch_*` helper.
4. If it modifies balance, call `state.balance_cache.invalidate()`.
5. If it needs a large payload, wrap it in `web::resource(...).app_data(web::PayloadConfig::new(limit))` like `/createAction`.
6. If it's a *dApp-facing* money mover, it belongs on the `create_action` cascade — **not** on the internal `/transaction/send` path.

## Related

- `../CLAUDE.md` — Rust wallet layer overview, build instructions, invariants
- `../crates/hodos_permission_engine/` — pure permission decision crate (`decide()`, Matrix C cascade). No CLAUDE.md
- `database/CLAUDE.md` — SQLite schema, repositories, migrations
- `crypto/CLAUDE.md` — BRC-42, BRC-43, signing, encryption modules
- `certificate/CLAUDE.md` — BRC-52 certificate parsing/verification types (handlers live in `handlers/certificate_handlers.rs`)
- `transaction/CLAUDE.md` — Bitcoin SV transaction parsing and building
- `script/CLAUDE.md` — Script parsing and PushDrop encoding
- `monitor/CLAUDE.md` — Background task scheduler (14 tasks)
- `/CLAUDE.md` — Root project documentation with full architecture overview
