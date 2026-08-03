# Rust Wallet Backend Layer

**Last Updated:** 2026-08-03

## Responsibility

Actix-web HTTP server providing wallet operations, BRC-100 protocol endpoints, cryptographic signing, the permission decision engine, and SQLite database storage. This is the security-critical layer: Rust was chosen for compile-time memory safety guarantees and secure memory clearing of private keys. Private keys never leave this process.

## Build & Run

```powershell
cd rust-wallet
cargo build --release    # Build only
cargo test               # Run tests
cargo check              # Fast type-check without building
```

**To run the dev server**, use the launcher script from the project root (sets `HODOS_DEV=1` automatically):
```powershell
.\dev-wallet.ps1         # Windows (PowerShell)
./dev-wallet.sh          # Mac/Linux
```

**⚠️ NEVER use bare `cargo run --release`** — the dev safeguard will block it. Dev builds detect they are running from `target/release/` and refuse to start without `HODOS_DEV=1` to prevent hitting the production database.

**Ports** (`main.rs :: wallet_port`): binds `127.0.0.1:31301` in release, `127.0.0.1:31401` under `HODOS_DEV=1`, so a dev wallet and the installed wallet can run side by side. The cross-layer source of truth for every port is `cef-native/include/core/PortConfig.h` (adblock engine is 31302 / 31402). Any reference to "3301"/"3302" is wrong.

**Dev/prod guard** (`main.rs :: enforce_dev_prod_isolation`, runs first in `main()`): a dev-build path without `HODOS_DEV=1` aborts; a stray `HODOS_DEV=1` on a non-dev-build binary is scrubbed and forced to prod.

**Dev storage**: `%APPDATA%/HodosBrowserDev/wallet/wallet.db`
**Production storage**: `%APPDATA%/HodosBrowser/wallet/wallet.db`

## Invariants

1. **Private keys never leave this process** — all signing happens here
2. **Do not change crypto/signing/derivation logic** without asking — `src/crypto/` is security-critical
3. **Do not change database schema** without asking — migrations in `src/database/migrations.rs`
4. **Do not change `AppState` struct** without understanding all handlers that depend on it
5. **Memory safety is non-negotiable** — Rust's ownership model prevents use-after-free and buffer overflows in key-handling code; do not introduce `unsafe` blocks without asking
6. **Permission decisions are Rust-native** — `crates/hodos_permission_engine` is the sole decision engine. The C++ `PermissionEngine` / `SessionManager` were deleted in Phase 2.6-H; do not reintroduce a second engine.

## Entry Points

| File | Purpose |
|------|---------|
| `src/main.rs` | Binary crate (`hodos-wallet`). `main()` builds `AppState`, registers 110 HTTP endpoint bindings, wraps the app in the `domain_trust_mw` permission middleware, and binds `127.0.0.1:wallet_port()`. Declares 30 private modules. |
| `src/lib.rs` | Library crate exposing 19 modules (`crypto`, `database`, `beef`, `recovery`, `overlay`, `services`, `permission_service`, `manifest`, `reconcile`, …) for the integration tests in `tests/` |
| `src/handlers.rs` | ~19.3k lines; the bulk of the HTTP handlers (`health`, `get_public_key`, `well_known_auth`, `create_action`, `sign_action`, `pay_402`, …). Handler-by-group breakdown lives in `src/CLAUDE.md`. |
| `src/handlers/certificate_handlers.rs` | Certificate handlers reached through `handlers.rs` re-exports: `acquire_certificate`, `list_certificates`, `prove_certificate`, `relinquish_certificate`, `discover_by_identity_key`, `discover_by_attributes`, `publish_certificate`, `unpublish_certificate`, `admin_prepare_unpublish`, `cleanup_overlay_certificates`, `auto_unpublish_certificate_pub` |

### `AppState` (`main.rs :: AppState`) — 18 fields

`database`, `auth_sessions`, `balance_cache`, `fee_rate_cache`, `price_cache`, `services`, `ship_cache`, `utxo_selection_lock`, `create_action_lock`, `derived_key_cache`, `current_user_id`, `shutdown`, `sync_status`, `backup_check_needed`, `recovery_just_completed`, `pay402_reuse`, `permission`, plus the `request_backup_check()` helper surface.

## Extension Points

| To Add | Where |
|--------|-------|
| New HTTP endpoint | Add handler fn in `src/handlers.rs`, register route in `src/main.rs` |
| New BRC protocol | Add module in `src/crypto/`, import in `handlers.rs` |
| New database table | Add migration in `src/database/migrations.rs`, bump the runner in `src/database/connection.rs`, add repo in `src/database/` |
| New background task | Add `task_*.rs` in `src/monitor/`, declare it in `monitor/mod.rs`, add a field to `TaskSchedule`, and dispatch it in the `Monitor::run` tick loop |
| New permission rule | Extend the cascade in `crates/hodos_permission_engine/src/matrix_c.rs` (pure, unit-tested), then wire the dispatcher in `src/permission_service/request_gate.rs` |
| New indexer provider | Add a provider in `src/services/providers/`, register it in the chain in `src/services/mod.rs` |

## Key Files

| File | Identifiers |
|------|-------------|
| `src/main.rs` | `AppState`, `main()`, `wallet_port()`, `domain_trust_mw`, route registration |
| `src/handlers.rs` | `health`, `get_public_key`, `well_known_auth`, `create_action`, `sign_action`, `pay_402`, `broadcast_nosend`, `list_certificates`, `acquire_certificate`, `HODOS_FEE_ADDRESS`, `HODOS_SERVICE_FEE_SATS`, `DEFAULT_SATS_PER_KB`, `MIN_FEE_SATS`, `estimate_transaction_size`, `calculate_fee`, `estimate_fee_for_transaction` |
| `src/handlers/certificate_handlers.rs` | `publish_certificate`, `unpublish_certificate`, `unpublish_certificate_core`, `acquire_certificate_direct`, `acquire_certificate_issuance`, `create_certificate_transaction` |
| `src/crypto/` | 11 public modules: `brc42`, `brc43`, `brc2`, `bie1`, `key_linkage`, `ghash`, `aesgcm_custom`, `pin`, `dpapi`, `keys`, `signing` (+ private `aesgcm_custom_test`) |
| `src/crypto/brc42.rs` | `derive_child_private_key`, `derive_child_public_key` |
| `src/crypto/brc43.rs` | `InvoiceNumber`, `SecurityLevel`, `normalize_protocol_id` |
| `src/crypto/signing.rs` | `sha256`, `hmac_sha256`, `verify_hmac_sha256` |
| `src/crypto/key_linkage.rs` | BRC-72 counterparty + specific key linkage revelation |
| `src/database/mod.rs` | `WalletDatabase` + 20 repositories (see roster below) |
| `src/database/helpers.rs` | `get_master_private_key_from_db`, `get_master_public_key_from_db`, `derive_key_for_output` (signing entry point), `address_to_address_info`, `output_to_fetcher_utxo` |
| `src/database/migrations.rs` | `create_schema_v1` (consolidated schema) + `migrate_v1_to_v2` … `migrate_v22_to_v23` |
| `src/database/connection.rs` | `WalletDatabase`, `migrate()` — the version-gated migration runner |
| `src/database/proven_tx_repo.rs` | `ProvenTxRepository`: `insert_or_get`, `get_by_txid`, `get_merkle_proof_as_tsc`, `link_transaction` — immutable proof records |
| `src/database/proven_tx_req_repo.rs` | `ProvenTxReqRepository`: `create`, `get_by_txid`, `update_status`, `link_proven_tx`, `add_history_note` — proof lifecycle tracking |
| `src/database/permission_audit_repo.rs` | `PermissionAuditEntry`, `PermissionAuditRepository` — the long-lived `permission_audit_log` surface (V20) |
| `src/recovery.rs` | `derive_private_key_bip32` (legacy BIP32 `m/{index}`), `recover_wallet_from_mnemonic`, `derive_key_at_path`, `derive_address_at_path`, `scan_external_wallet`, `build_sweep_transactions`, `pubkey_to_address`, `address_to_p2pkh_script` |
| `src/action_storage.rs` | `ActionStatus`, `TransactionStatus`, `ProvenTxReqStatus` enums |
| `src/monitor/mod.rs` | `Monitor`, `TaskSchedule` (14 interval fields), `log_monitor_event` — background scheduler on a 30s tick with a `db_available()` gate |
| `src/permission_service/` | `PermissionService`, `domain_trust_gate`, `dispatch_privacy_perimeter`, `dispatch_scoped_grant`, `dispatch_cert_disclosure`, `dispatch_payment`, `is_protected_basket`, `GateOutcome`, `ScopedCall`, `PaymentCall`, `PendingApproval`, `SessionCounters`, session handlers |
| `crates/hodos_permission_engine/` | Pure decision crate: `decide()` in `src/lib.rs`, cascade in `src/matrix_c.rs`, `PermissionContext`/`CallKind`/`TrustLevel` in `src/context.rs`, `PermissionDecision`/`EngineReason`/`PromptType` in `src/decision.rs`. 40 unit tests. |
| `src/manifest.rs` | Rust port of the C++ `ManifestFetcher` — `fetch_manifest` (3s cap, 64 KB cap) + pure lenient `parse_manifest` for `.well-known/wallet-manifest.json` |
| `src/reconcile.rs` | Wallet-Hardening WS1 spent-input primitives. `check_outpoint_spent` is live (two-provider WoC + GorillaPool signal rule); `recover_change_index` and `reconcile_spent_inputs` are unwired. |
| `src/authfetch.rs` | BRC-103 AuthFetch client: 401 challenge-response with ECDSA signing, nonce exchange |
| `src/messagebox.rs` | MessageBox API client — BRC-2 encrypted send/receive/acknowledge via `messagebox.babbage.systems` |
| `src/paymail.rs` | Paymail (bsvalias) resolution client |
| `src/identity_resolver.rs` | Identity resolution via BSV Overlay Services (BRC-52 certificates) |
| `src/overlay/mod.rs` | SHIP-discovery + overlay submit/lookup for `tm_identity`: `submit_to_identity_overlay`, `submit_to_topic`, `lookup_published_certificate`, `lookup_certificates_by_identity_key`, `OverlayCertificateOutput`. Public API takes `&Arc<ShipDiscoveryCache>`, not `&AppState`. |
| `src/overlay/ship_cache.rs` | `ShipDiscoveryCache` — SWR cache for SHIP host discovery (fresh < 5 min / stale 5–30 min spawns bg refresh / very-stale ≥ 30 min blocks). No-poison invariant: empty fetch results never overwrite or store. 9 unit tests. |
| `src/services/` | `WalletServices` facade: `CallClass` per-call-class timeouts, `ProviderCollection`, and 7 providers in `providers/` (`whatsonchain`, `arc_gorillapool`, `arc_taal`, `gorillapool_mapi`, `gorillapool_ordinals`, `junglebus`, `bitails`) |
| `src/arc_status.rs` | Centralized ARC miner response status classification (txStatus ladder) |
| `src/beef.rs` | BEEF parser: `tsc_proof_to_bump`, `parse_bump_hex_to_tsc`, `compute_merkle_root_from_tsc`, `validate_beef_v1_hex`, `validate_beef_ancestry`, `read_node_offset` |
| `src/beef_helpers.rs` | Recursive BEEF building with ancestry chain and proof fetching |
| `src/transaction/sighash.rs` | BSV ForkID SIGHASH implementation |
| `src/script/` | Bitcoin script `parser.rs` + `pushdrop.rs` (BRC-48) |
| `src/certificate/` | BRC-52 certificate `parser.rs`, `verifier.rs`, `selective_disclosure.rs`, `types.rs` |
| `src/balance_cache.rs` | `BalanceCache` — in-memory balance with instant invalidation |
| `src/fee_rate_cache.rs` | `FeeRateCache` — ARC `/v1/policy` mining fee rate, 1-hour TTL, falls back to `DEFAULT_SATS_PER_KB` |
| `src/price_cache.rs` | BSV/USD price cache (WhatsOnChain + CoinGecko + MEXC), persisted to the V21 `bsv_price_cache` table so a cold start has a fallback |
| `src/backup.rs` | Database backup / restore utilities |
| `src/auth_session.rs` | `AuthSessionManager` — BRC-103 server-side session state |
| `src/utxo_fetcher.rs`, `src/json_storage.rs`, `src/cache_errors.rs`, `src/cache_helpers.rs` | UTXO fetching, legacy JSON storage shim, unified cache error types + helpers |

### Repository roster (`src/database/mod.rs`) — 20 repositories

`WalletRepository`, `AddressRepository`, `TransactionRepository`, `ParentTransactionRepository`, `BlockHeaderRepository`, `ProvenTxRepository`, `ProvenTxReqRepository`, `BasketRepository`, `TagRepository`, `CertificateRepository`, `MessageRelayRepository`, `UserRepository`, `OutputRepository`, `TxLabelRepository`, `CommissionRepository`, `SettingsRepository`, `SyncStateRepository`, `DomainPermissionRepository`, `PeerPayRepository`, `PermissionAuditRepository`.

Method-level detail for each repo lives in `src/database/CLAUDE.md`.

## Permission Engine (Rust-native)

The permission **decision** engine lives in this layer, not in C++.

| Piece | Location |
|-------|----------|
| Pure decision crate | `crates/hodos_permission_engine` — `decide()` in `src/lib.rs`, Matrix-C cascade in `src/matrix_c.rs`. No I/O, no globals, 40 unit tests. |
| Actix wrapper | `src/permission_service/` — `PermissionService` state, `context_builder`, `request_gate` dispatchers, `audit` writer, session `handlers` |
| Wiring | `src/main.rs :: domain_trust_mw`, installed via `.wrap(middleware::from_fn(domain_trust_mw))`. `GateOutcome::EarlyReturn` short-circuits with a 202 connect prompt or 403 block; the handler never runs. |
| Audit trail | `permission_audit_log` table (V20), via `PermissionAuditRepository` |

Dispatchers in `request_gate.rs`: `domain_trust_gate` (middleware entry), `dispatch_privacy_perimeter`, `dispatch_scoped_grant`, `dispatch_cert_disclosure`, `dispatch_payment`. Protected baskets (`is_protected_basket`) are never silenced by a bundled scope grant.

**Default spending limits**: $1.00 per transaction and $10.00 per session (100 / 1000 USD cents), set by `migrate_v11_to_v12` and stored on `settings` as `default_per_tx_limit_cents` / `default_per_session_limit_cents` (plus `default_rate_limit_per_min = 30`).

> The C++ `PermissionEngine` and `SessionManager` were **deleted** in Phase 2.6-H, along with the shadow-comparison infrastructure (`engine_shadow_log` dropped in V23). There is exactly one engine.

## Database Schema

**Current migration version: V23.** `migrations.rs` is the only authority: the highest migration function is `migrate_v22_to_v23`, and the runner in `connection.rs :: WalletDatabase::migrate` gates on `current_version < 23` before stamping `schema_version`.

New databases get the fully consolidated `create_schema_v1`; V2–V23 are incremental migrations for pre-existing databases. All migrations are idempotent (existence-checked `ALTER`, `CREATE TABLE IF NOT EXISTS`, `DROP TABLE IF EXISTS`).

**The per-table schema roster and the full V1→V23 migration ledger live in [`src/database/CLAUDE.md`](src/database/CLAUDE.md)** — this doc deliberately does not carry a second copy. Anything below is wallet-layer behavior that reads *from* the schema, not a description of it.

Load-bearing points for this layer:
- V17 added `identity_key_disclosure_allowed` on `domain_permissions` (gates `get_public_key({identityKey:true})` for external domains).
- V18 added the three sub-permission child tables (`domain_protocol_permissions`, `domain_basket_permissions`, `domain_counterparty_permissions`), all `ON DELETE CASCADE` from `domain_permissions(id)`.
- V19 added `default_identity_key_disclosure_allowed` on `settings`.
- V20 added `permission_audit_log` (kept) and `engine_shadow_log` (dropped in V23).
- V21 added `bsv_price_cache` — the persisted price fallback consumed by `price_cache.rs`.
- V22 added `bundled_scope_grant` on `domain_permissions`.
- V23 dropped `engine_shadow_log` (Phase 2.6-H cleanup).

### Output Model

The `outputs` table (`outputId` PK) is the sole source of truth for UTXO tracking. `spendable=1` means available; `spent_by` is an FK to `transactions(id)`; `locking_script` is a BLOB; `transaction_id` is nullable (NULL for externally-received outputs from address sync or PeerPay); `UNIQUE(txid, vout)`.

Derivation fields are the source of truth for key derivation — no address-table lookup is needed. `derive_key_for_output(db, prefix, suffix, sender_identity_key)` routes:

- `derivation_prefix="2-receive address"`, `suffix="{index}"` → BRC-42 self-derivation (standard)
- `derivation_prefix="bip32"`, `suffix="{index}"` → legacy BIP32 HD derivation (`m/{index}`)
- `derivation_prefix=NULL`, `suffix=NULL` → master private key directly
- any prefix/suffix with `sender_identity_key=Some(pubkey)` → BRC-42 counterparty derivation

### Multi-User Foundation

The `users` table plus `user_id` foreign keys on the core tables. All existing data is linked to the default user (ID 1), whose `identity_key` is the wallet's master public key.

```
wallets table (mnemonic, HD derivation root)
    │
    ▼ derives master public key
users table (identity_key = master pubkey)
    │
    ▼ user_id FK
transactions, outputs, baskets, certificates, sync_states, …
```

`AppState.current_user_id` holds the active user ID for all operations.

### Status System

A single `status TEXT NOT NULL` column on `transactions` replaced the old dual `status` + `broadcast_status` pair. Values come from `action_storage.rs :: TransactionStatus` (8 variants):

| status | Meaning |
|--------|---------|
| unprocessed | Created, not yet handled |
| unsigned | Created but not yet signed |
| sending | Being broadcast |
| unproven | Broadcast, no merkle proof yet |
| completed | Has merkle proof (proven on-chain) |
| nosend | Signed but intentionally not broadcast (aborted / data carrier / BRC-121 paid retry) |
| nonfinal | Future locktime, not yet finalized |
| failed | Broadcast failed or rejected |

`TransactionStatus::from_legacy` maps the pre-unification `ActionStatus` + `broadcast_status` pair; `to_action_status` maps back for the JSON/`StoredAction` path.

## Background Services — Monitor Pattern

The Monitor (`src/monitor/mod.rs`) is the sole background task scheduler. It runs as a single tokio task with a 30-second tick loop, guarded by `MONITOR_STARTED` against duplicate loops, and gates most tasks behind `db_available()` so a busy DB defers rather than blocks. Intervals come from `TaskSchedule::default()`.

**14 tasks**, one module each in `src/monitor/`:

| Task | Interval | Purpose |
|------|----------|---------|
| TaskCheckForProofs | 60s | Acquire merkle proofs for unproven transactions (ARC → WhatsOnChain fallback) |
| TaskSendWaiting | 120s | Crash recovery for transactions stuck in `sending` status |
| TaskFailAbandoned | 300s | Fail stuck `unprocessed`/`unsigned` txs, clean up ghost outputs |
| TaskUnFail | 300s | Recover false failures by re-checking on-chain (6-hour window), re-marks inputs as spent |
| TaskReviewStatus | 60s | Ensure consistency across proven_tx_reqs → transactions → outputs |
| TaskPurge | 3600s | Cleanup old monitor_events (7d) and completed proof requests (30d) |
| TaskSyncPending | 30s | UTXO discovery for addresses with `pending_utxo_check=1`; tiered by address age (fresh 30s / recent 3m / old 30m bulk). Discovery only — never marks outputs spent. |
| TaskCheckPeerPay | 60s | Poll MessageBox for incoming BRC-29 payments (BRC-103 AuthFetch + BRC-2 decrypt), auto-accept via P2PKH verification |
| TaskBackup | 10800s (3h) | On-chain wallet backup via the HTTP endpoint; hash comparison skips no-op backups. Returns `BackupOutcome` (Broadcast / Skipped / Deferred / Failed). Fires sooner when `AppState.backup_check_needed` is set. |
| TaskReplayOverlay | 300s | Retry overlay notification for certs stuck at `publish_status = 'unpublished_pending_overlay'` |
| TaskConsolidateDust | 86400s (24h) | Consolidate UTXOs below 1000 sats once 20+ accumulate; opt-out via `disable_dust_consolidation` setting |
| TaskVerifyDoubleSpend | 60s | Independently verify ARC `DOUBLE_SPEND_ATTEMPTED` suspicions (`spending_description = 'dss:{txid}'`) against WhatsOnChain; restore false alarms |
| TaskRetryPeerPayOutbox | 30s | Retry MessageBox delivery for `peerpay_outbox` rows (60s ×10, then 120s ×10, then `exhausted`); actual retry governed by `next_retry_at` |
| TaskRefreshShipCache | 300s | Keeps `AppState.ship_cache` warm for `tm_identity`. Runs **outside** the `db_available()` gate (pure network + memory) so a busy DB never starves SHIP refresh. |

### Ghost Transaction Safety Rules

1. Background tasks never create output records — only sync from API via `/wallet/sync`
2. Delete ghost outputs BEFORE restoring inputs on failure
3. TaskUnFail does NOT re-create deleted outputs — relies on `/wallet/sync`
4. Always invalidate balance cache after output changes
5. Cleanup order: mark failed → delete ghost outputs → restore inputs → invalidate cache

### UTXO Sync

Two mechanisms:
1. **Periodic (TaskSyncPending)**: Monitor checks addresses with `pending_utxo_check=1`, tiered 30s / 3m / 30m by address age
2. **On-demand (`POST /wallet/sync`)**: Frontend or manual trigger, `?full=true` for all addresses

The sync endpoint:
- Fetches UTXOs from WhatsOnChain for target addresses
- Inserts new outputs via `upsert_received_utxo()`
- **Reconciles** stale outputs: marks DB outputs not found in API as `external-spend` (`spending_description = 'external-spend'`, `spendable = 0`)
- Invalidates balance cache

## Fee Calculation

Transaction fees are calculated dynamically based on size (not hardcoded):

| Constant/Function | Location | Purpose |
|-------------------|----------|---------|
| `DEFAULT_SATS_PER_KB` | `handlers.rs` | Default/fallback fee rate: 1000 sat/kb (1 sat/byte) |
| `MIN_FEE_SATS` | `handlers.rs` | Minimum fee floor: 200 satoshis |
| `estimate_transaction_size()` | `handlers.rs` | Calculate tx size from script lengths |
| `calculate_fee()` | `handlers.rs` | Compute fee from size and rate, clamped to `MIN_FEE_SATS` |
| `estimate_fee_for_transaction()` | `handlers.rs` | Estimate fee before tx is built |
| `FeeRateCache::get_rate()` | `fee_rate_cache.rs` | Live mining fee rate from ARC `/v1/policy` (`arc.gorillapool.io`), 1-hour TTL, integrity-hash validated, falls back to `DEFAULT_SATS_PER_KB` when ARC is unavailable. Consumed by `create_action` and the backup/consolidation builders via `state.fee_rate_cache`. |

### Service fee

Every outgoing transaction adds a **1000-satoshi** output to the Hodos treasury: `HODOS_SERVICE_FEE_SATS` / `HODOS_FEE_ADDRESS` (`1Q1A2rq6trBdptd3t6n53vB79mRN6JHEFT`), both `pub` in `handlers.rs`. Output order is: request outputs → service fee → change. Recorded in the `commissions` table; cleaned up on broadcast failure.

## API Endpoints

`main.rs` registers **110 HTTP endpoint bindings** — 107 `.route(path, …)` plus 3 `web::resource(...)` registrations that carry a 100 MB `PayloadConfig` (`/createAction`, `/signAction`, `/wallet/import`). That is **109 distinct handler functions** across **98 distinct paths** (`/getVersion` is bound to both GET and POST; `/domain/permissions` and `/domain/permissions/*` bind different handlers per verb; `/wallet/settings` binds GET + POST).

### Server control & BRC-100 identity

| Method | Path | Handler |
|--------|------|---------|
| GET | `/health` | `health` |
| POST | `/shutdown` | `shutdown` |
| GET | `/brc100/status` | `brc100_status` |
| GET, POST | `/getVersion` | `get_version` |
| POST | `/getPublicKey` | `get_public_key` |
| POST | `/isAuthenticated` | `is_authenticated` |
| POST | `/waitForAuthentication` | `wait_for_authentication` |
| POST | `/.well-known/auth` | `well_known_auth` |

### BRC-100 crypto

| Method | Path | Handler |
|--------|------|---------|
| POST | `/createHmac` | `create_hmac` |
| POST | `/verifyHmac` | `verify_hmac` |
| POST | `/encrypt` | `encrypt` |
| POST | `/decrypt` | `decrypt` |
| POST | `/createSignature` | `create_signature` |
| POST | `/verifySignature` | `verify_signature` |
| POST | `/revealCounterpartyKeyLinkage` | `reveal_counterparty_key_linkage` |
| POST | `/revealSpecificKeyLinkage` | `reveal_specific_key_linkage` |
| POST | `/wallet/encrypt-bie1` | `encrypt_bie1_handler` |
| POST | `/wallet/decrypt-bie1` | `decrypt_bie1_handler` |

### Actions & outputs

| Method | Path | Handler |
|--------|------|---------|
| POST | `/createAction` | `create_action` *(100 MB payload)* |
| POST | `/signAction` | `sign_action` *(100 MB payload)* |
| POST | `/processAction` | `process_action` |
| POST | `/abortAction` | `abort_action` |
| POST | `/listActions` | `list_actions` |
| POST | `/internalizeAction` | `internalize_action` |
| POST | `/updateConfirmations` | `update_confirmations_endpoint` |
| POST | `/listOutputs` | `list_outputs` |
| POST | `/relinquishOutput` | `relinquish_output` |
| GET | `/wallet/tokens` | `list_token_outputs` |
| POST | `/transaction/send` | `send_transaction` |

### Blockchain

| Method | Path | Handler |
|--------|------|---------|
| POST | `/getHeight` | `get_height` |
| POST | `/getHeaderForHeight` | `get_header_for_height` |
| POST | `/getNetwork` | `get_network` |

### Certificates (BRC-52)

| Method | Path | Handler |
|--------|------|---------|
| POST | `/acquireCertificate` | `acquire_certificate` |
| POST | `/listCertificates` | `list_certificates` |
| POST | `/proveCertificate` | `prove_certificate` |
| POST | `/relinquishCertificate` | `relinquish_certificate` |
| POST | `/discoverByIdentityKey` | `discover_by_identity_key` |
| POST | `/discoverByAttributes` | `discover_by_attributes` |
| POST | `/wallet/certificate/publish` | `publish_certificate` |
| POST | `/wallet/certificate/unpublish` | `unpublish_certificate` |
| POST | `/wallet/certificate/cleanup` | `cleanup_overlay_certificates` |
| POST | `/admin/prepare-unpublish` | `admin_prepare_unpublish` |

### Wallet management

| Method | Path | Handler |
|--------|------|---------|
| GET | `/wallet/status` | `wallet_status` |
| POST | `/wallet/create` | `wallet_create` |
| POST | `/wallet/delete` | `wallet_delete` |
| GET | `/wallet/balance` | `wallet_balance` |
| POST | `/wallet/sync` | `wallet_sync` — on-demand UTXO sync with reconciliation |
| POST | `/wallet/unlock` | `wallet_unlock` |
| POST | `/wallet/backup` | `wallet_backup` |
| POST | `/wallet/backup/onchain` | `wallet_backup_onchain` |
| POST | `/wallet/backup/onchain/verify` | `wallet_backup_onchain_verify` |
| POST | `/wallet/recover` | `wallet_recover` |
| POST | `/wallet/recover-external` | `wallet_recover_external` |
| POST | `/wallet/recover/onchain` | `wallet_recover_onchain` |
| POST | `/wallet/restore` | `wallet_restore` |
| POST | `/wallet/rescan` | `wallet_rescan` |
| POST | `/wallet/cleanup` | `wallet_cleanup` |
| POST | `/wallet/consolidate-dust` | `wallet_consolidate_dust` |
| POST | `/wallet/export` | `wallet_export` |
| POST | `/wallet/import` | `wallet_import` *(100 MB payload)* |
| POST | `/wallet/reveal-mnemonic` | `reveal_mnemonic` |
| GET | `/wallet/activity` | `wallet_activity` |
| GET, POST | `/wallet/settings` | `wallet_settings_get` / `wallet_settings_set` |
| GET | `/wallet/bsv-price` | `get_bsv_price` |
| GET | `/wallet/sync-status` | `get_sync_status` |
| POST | `/wallet/sync-status/seen` | `mark_sync_seen` |

### Addresses

| Method | Path | Handler |
|--------|------|---------|
| POST | `/wallet/address/generate` | `generate_address` |
| GET | `/wallet/addresses` | `get_all_addresses` |
| GET | `/wallet/address/current` | `get_current_address` |
| POST | `/wallet/yours-legacy-addresses` | `yours_legacy_addresses` |
| POST | `/wallet/address-to-script` | `address_to_script` |

### Domain permissions

| Method | Path | Handler |
|--------|------|---------|
| GET / POST / DELETE | `/domain/permissions` | `get_domain_permission` / `set_domain_permission` / `delete_domain_permission` |
| GET | `/domain/permissions/all` | `list_domain_permissions` |
| GET / POST / DELETE | `/domain/permissions/certificate` | `check_cert_permissions` / `approve_cert_fields` / `revoke_cert_fields` |
| GET / POST / DELETE | `/domain/permissions/protocol` | `list_protocol_permissions` / `grant_protocol_permission` / `revoke_protocol_permission` |
| GET / POST / DELETE | `/domain/permissions/basket` | `list_basket_permissions` / `grant_basket_permission` / `revoke_basket_permission` |
| GET / POST / DELETE | `/domain/permissions/counterparty` | `list_counterparty_permissions` / `grant_counterparty_permission` / `revoke_counterparty_permission` |
| POST | `/domain/permissions/reset-all` | `domain_permissions_reset_all` |
| POST | `/wallet/session-approve` | `permission_service::handlers::session_approve` |
| POST | `/wallet/session-revoke` | `permission_service::handlers::session_revoke` |
| POST | `/wallet/session/close` | `permission_service::handlers::session_close` |

### Messaging, PeerPay & Paymail

| Method | Path | Handler |
|--------|------|---------|
| POST | `/sendMessage` | `send_message` |
| POST | `/listMessages` | `list_messages` |
| POST | `/acknowledgeMessage` | `acknowledge_message` |
| POST | `/wallet/peerpay/send` | `peerpay_send` — send BSV via BRC-29 to an identity key |
| POST | `/wallet/peerpay/check` | `peerpay_check` |
| GET | `/wallet/peerpay/status` | `peerpay_status` — notification badge data (unread count) |
| POST | `/wallet/peerpay/dismiss` | `peerpay_dismiss` |
| POST | `/wallet/peerpay/outbox-retry` | `peerpay_outbox_retry` |
| POST | `/wallet/paymail/send` | `paymail_send` |
| GET | `/wallet/paymail/resolve` | `paymail_resolve` |
| GET | `/wallet/recipient/resolve` | `recipient_resolve` — unified identity / paymail / BSV address |
| GET | `/wallet/recipient/suggest` | `recipient_suggest` |

### BRC-121 (402 paywall)

| Method | Path | Handler |
|--------|------|---------|
| POST | `/wallet/pay402` | `pay_402` — mints a nosend BRC-29 BEEF and emits the 5 retry headers |
| POST | `/wallet/broadcast-nosend` | `broadcast_nosend` — broadcasts after the paid retry returns 200 |

### Debug

| Method | Path | Handler |
|--------|------|---------|
| POST | `/wallet/debug/validate-beef` | `debug_validate_beef` |
| POST | `/wallet/debug/repair-nosend` | `debug_repair_nosend` |
| POST | `/wallet/debug/broadcast-nosend` | `debug_broadcast_nosend` |

## Tests

14 integration test files in `tests/` — ten `tier3`–`tier12` coverage tiers plus `beef_crypto_cert_test`, `sdk_interop_test`, `sighash_transaction_test`, `diagnostic_test` — alongside in-module unit tests. The permission engine crate carries 40 unit tests of its own; `overlay/ship_cache.rs` carries 9. See `tests/CLAUDE.md`.

## Related

- [`src/CLAUDE.md`](src/CLAUDE.md) — module map, handler groups, key types and constants
- [`src/database/CLAUDE.md`](src/database/CLAUDE.md) — schema roster, migration ledger, repository methods
- [`src/crypto/CLAUDE.md`](src/crypto/CLAUDE.md) — crypto module exports and SDK interop rules
- [`src/monitor/CLAUDE.md`](src/monitor/CLAUDE.md) — per-task detail
- [`tests/CLAUDE.md`](tests/CLAUDE.md) — integration test suite
