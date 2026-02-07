# Rust Wallet Backend Layer

## Responsibility

Actix-web HTTP server providing wallet operations, BRC-100 protocol endpoints, cryptographic signing, and SQLite database storage. This is the security-critical layer: Rust was chosen for compile-time memory safety guarantees and secure memory clearing of private keys. Private keys never leave this process.

## Build & Run (Windows)

```powershell
cd rust-wallet
cargo build --release    # Build
cargo run --release      # Run on localhost:3301
cargo test               # Run tests
cargo check              # Fast type-check without building
```

Server logs to console. Creates wallet DB at `%APPDATA%/HodosBrowser/wallet/wallet.db` on first run.

## Invariants

1. **Private keys never leave this process** — all signing happens here
2. **Do not change crypto/signing/derivation logic** without asking — `src/crypto/` is security-critical
3. **Do not change database schema** without asking — migrations in `src/database/migrations.rs`
4. **Do not change `AppState` struct** without understanding all handlers that depend on it
5. **Memory safety is non-negotiable** — Rust's ownership model prevents use-after-free and buffer overflows in key-handling code; do not introduce `unsafe` blocks without asking

## Entry Points

| File | Purpose |
|------|---------|
| `src/main.rs` | `main()`, initializes `AppState` (database, whitelist, message_store, auth_sessions, balance_cache), starts Actix-web on port 3301 |
| `src/handlers.rs` | All HTTP endpoint handlers: `health`, `get_public_key`, `well_known_auth`, `create_action`, `sign_action`, etc. |

## Extension Points

| To Add | Where |
|--------|-------|
| New HTTP endpoint | Add handler fn in `src/handlers.rs`, register route in `src/main.rs` |
| New BRC protocol | Add module in `src/crypto/`, import in `handlers.rs` |
| New database table | Add migration in `src/database/migrations.rs`, add repo in `src/database/` |
| New background task | Add task module in `src/monitor/`, register in `monitor/mod.rs` run loop |

## Key Files

| File | Identifiers |
|------|-------------|
| `src/main.rs` | `AppState`, `main()`, route registration |
| `src/handlers.rs` | `health`, `get_public_key`, `well_known_auth`, `create_action`, `sign_action`, `list_certificates`, `acquire_certificate`, fee calculation utilities |
| `src/crypto/brc42.rs` | `derive_child_private_key`, `derive_child_public_key` |
| `src/crypto/brc43.rs` | `InvoiceNumber`, `SecurityLevel`, `normalize_protocol_id` |
| `src/crypto/signing.rs` | `sha256`, `hmac_sha256`, `verify_hmac_sha256` |
| `src/database/mod.rs` | `WalletDatabase`, `WalletRepository`, `AddressRepository`, `OutputRepository`, `CertificateRepository`, `ProvenTxRepository`, `ProvenTxReqRepository`, `UserRepository`, `TxLabelRepository`, `CommissionRepository`, `SettingsRepository`, `SyncStateRepository` |
| `src/database/helpers.rs` | `get_master_private_key_from_db`, `get_master_public_key_from_db` |
| `src/database/proven_tx_repo.rs` | `ProvenTxRepository`: `insert_or_get`, `get_by_txid`, `get_merkle_proof_as_tsc`, `link_transaction` — immutable proof records |
| `src/database/proven_tx_req_repo.rs` | `ProvenTxReqRepository`: `create`, `get_by_txid`, `update_status`, `link_proven_tx`, `add_history_note` — proof lifecycle tracking |
| `src/monitor/mod.rs` | `Monitor` struct — background task scheduler (30s tick loop, 6 tasks, event logging) |
| `src/monitor/task_check_for_proofs.rs` | Proof acquisition from ARC + WhatsOnChain (replaces `arc_status_poller` + `cache_sync`) |
| `src/monitor/task_send_waiting.rs` | Crash recovery for orphaned `sending` transactions |
| `src/monitor/task_fail_abandoned.rs` | Fail stuck `unprocessed`/`unsigned` txs with ghost output cleanup |
| `src/monitor/task_unfail.rs` | Recover false failures by re-checking on-chain (30-min window) |
| `src/monitor/task_review_status.rs` | Status consistency: proven_tx_reqs → transactions → outputs |
| `src/monitor/task_purge.rs` | Cleanup old monitor_events (7d) and completed proof requests (30d) |
| `src/beef.rs` | BEEF parser, `tsc_proof_to_bump`, `parse_bump_hex_to_tsc` — Merkle proof format conversion |
| `src/beef_helpers.rs` | Recursive BEEF building with ancestry chain and proof fetching |
| `src/transaction/sighash.rs` | BSV ForkID SIGHASH implementation |
| `src/arc_status_poller.rs` | **Deprecated** (Phase 6I) — replaced by `monitor/task_check_for_proofs` |
| `src/cache_sync.rs` | **Deprecated** (Phase 6I) — replaced by `monitor/task_check_for_proofs` |
| `src/utxo_sync.rs` | **Deprecated** (Phase 6I) — replaced by `wallet_sync` handler + monitor |

## Database Schema (V22)

Current migration version: **V22**. Migrations in `src/database/migrations.rs`, runner in `src/database/connection.rs`.

| Table | Purpose | Phase |
|-------|---------|-------|
| wallets | Master key storage (mnemonic, HD index) | Original |
| users | Identity mapping (master pubkey → userId). Default user created from wallet. | V17 |
| addresses | HD address derivation cache | Original |
| transactions | Transaction records, `new_status` (V15), `proven_tx_id` FK (V16), `user_id` FK (V17) | V15-V17 |
| outputs | **Primary** — wallet-toolbox compatible output tracking with `spendable`/`spent_by` | V18 |
| utxos | **Deprecated** — no longer used. Code removed in Phase 4E. Table kept for rollback safety | Original |
| parent_transactions | Raw tx cache for BEEF building | Original |
| merkle_proofs | **Deprecated** — no longer written to. Replaced by `proven_txs` (V16) | Original |
| block_headers | Cached block headers | Original |
| proven_txs | **Immutable** proof records (merkle path + raw tx). Created by Monitor task_check_for_proofs | V16 |
| proven_tx_reqs | Proof acquisition lifecycle tracking. Created on broadcast, completed when proof acquired | V16 |
| baskets | Output categorization, `user_id` FK (V17) | V14/V17 |
| output_tags / output_tag_map | Output tagging, `user_id` FK (V17) | V14/V17 |
| certificates / certificate_fields | BRC-52 identity certificates, `user_id` FK (V17) | V7/V17 |
| domain_whitelist | BRC-100 app permissions | Original |
| tx_labels / tx_labels_map | Transaction labels (normalized pattern, replaces `transaction_labels`) | V19 |
| commissions | Fee tracking per transaction | V19 |
| settings | Persistent wallet configuration (chain, dbtype, limits) | V19 |
| sync_states | Multi-device synchronization state | V19 |
| monitor_events | Background task event logging (Monitor pattern) | V20 |
| transaction_labels | **Deprecated** — reads fallback only. Replaced by `tx_labels`/`tx_labels_map` (V19) | Original |

### Migrations V20-V22 (Phase 6)

| Migration | Purpose |
|-----------|---------|
| V20 | Create `monitor_events` table with indexes for event logging |
| V21 | Patch `proven_txs` merkle_path BLOBs — inject missing `height` field from column |
| V22 | Fix array-format BLOBs in `proven_txs` — normalize `[{...}]` to `{...}` and inject height |

### Output Model (V18 - Phase 4 Complete)

The `outputs` table is now the sole source of truth for UTXO tracking (Phase 4 completed 2026-02-06).
The old `utxos` table code has been removed; only `OutputRepository` is used.

| Old (`utxos`) | New (`outputs`) | Notes |
|---------------|-----------------|-------|
| `is_spent` | `spendable` | Inverted: spendable=1 means available |
| `spent_txid` | `spent_by` | FK to transactions.id instead of text |
| `address_id` | `derivation_prefix`/`derivation_suffix` | Self-contained derivation info |
| - | `transaction_id` | FK to creating transaction |
| - | `user_id` | FK for multi-user support |
| `script` (hex) | `locking_script` (BLOB) | Binary format |

Derivation mapping from address index:
- `index >= 0` → `derivation_prefix="2-receive address"`, `derivation_suffix="{index}"`
- `index == -1` → NULL (master pubkey, no derivation)
- `index < -1` → Parsed from `custom_instructions` (BRC-29)

### Multi-User Foundation (V17)

Phase 3 of wallet-toolbox alignment. Adds `users` table and `user_id` foreign keys to core tables.
All existing data is linked to the default user (ID 1), whose `identity_key` is the wallet's master public key.

```
wallets table (mnemonic, HD derivation root)
    │
    ▼ derives master public key
users table (identity_key = master pubkey)
    │
    ▼ user_id FK
transactions, baskets, certificates, etc.
```

`AppState.current_user_id` holds the active user ID for all operations.

### Status System (V15+)

Single `new_status` column replaces old dual `status` + `broadcast_status`:

| new_status | Meaning |
|------------|---------|
| unprocessed | Created, not signed |
| unsigned | Awaiting signatures (two-phase) |
| sending | Being broadcast |
| unproven | Broadcast, no merkle proof yet |
| completed | Has merkle proof (proven on-chain) |
| failed | Broadcast failed or rejected |

## Background Services — Monitor Pattern (Phase 6)

The Monitor (`src/monitor/mod.rs`) is the sole background task scheduler, replacing the deprecated `arc_status_poller`, `cache_sync`, and `utxo_sync` services. It runs as a single tokio task with a 30-second tick loop.

| Task | Interval | Purpose |
|------|----------|---------|
| TaskCheckForProofs | 60s | Acquire merkle proofs for unproven transactions (ARC → WoC fallback) |
| TaskSendWaiting | 120s | Crash recovery for transactions stuck in `sending` status |
| TaskFailAbandoned | 300s | Fail stuck `unprocessed`/`unsigned` txs, clean up ghost outputs |
| TaskUnFail | 300s | Recover false failures by re-checking on-chain (30-min window) |
| TaskReviewStatus | 60s | Ensure consistency across proven_tx_reqs → transactions → outputs |
| TaskPurge | 3600s | Cleanup old monitor_events (7d) and completed proof requests (30d) |

### Ghost Transaction Safety Rules

1. Background tasks never create output records — only sync from API via `/wallet/sync`
2. Delete ghost outputs BEFORE restoring inputs on failure
3. TaskUnFail does NOT re-create deleted outputs — relies on `/wallet/sync`
4. Always invalidate balance cache after output changes
5. Cleanup order: mark failed → delete ghost outputs → restore inputs → invalidate cache

### UTXO Sync

UTXO synchronization is on-demand via `POST /wallet/sync` (not periodic). The endpoint:
- Fetches UTXOs from WhatsOnChain for pending (or all with `?full=true`) addresses
- Inserts new outputs via `upsert_received_utxo()`
- **Reconciles** stale outputs: marks DB outputs not found in API as externally spent
- Invalidates balance cache

## Fee Calculation

Transaction fees are calculated dynamically based on size (not hardcoded):

| Constant/Function | Purpose |
|-------------------|---------|
| `DEFAULT_SATS_PER_KB` | Default fee rate: 1000 sat/kb (1 sat/byte) |
| `MIN_FEE_SATS` | Minimum fee: 200 satoshis |
| `estimate_transaction_size()` | Calculate tx size from script lengths |
| `calculate_fee()` | Compute fee from size and rate |
| `estimate_fee_for_transaction()` | Estimate fee before tx is built |

**Future**: MAPI integration for dynamic fee rates (see TODO in `handlers.rs`)

## API Endpoints (subset)

| Method | Path | Handler |
|--------|------|---------|
| GET | `/health` | `health` |
| GET | `/brc100/status` | `brc100_status` |
| POST | `/getPublicKey` | `get_public_key` |
| POST | `/.well-known/auth` | `well_known_auth` |
| POST | `/createAction` | `create_action` |
| POST | `/signAction` | `sign_action` |
| POST | `/listCertificates` | `list_certificates` |
| GET | `/wallet/status` | `wallet_status` |
| GET | `/wallet/balance` | `get_balance` |
| POST | `/wallet/sync` | `wallet_sync` — on-demand UTXO sync with reconciliation |
