# Database — Wallet Data Access Layer

> SQLite-backed persistence for the Rust wallet backend. All wallet state lives here: keys, outputs, transactions, certificates, permissions, and configuration.

## Overview

This module provides the complete data access layer for the HodosBrowser wallet. It uses `rusqlite` for synchronous SQLite access with WAL mode, foreign keys, and a 5-second busy timeout. The database file lives at `<app_data>/wallet/wallet.db`.

**Architecture**: Repository pattern — each table group has a dedicated `*Repository` struct that borrows a `&Connection`. The central `WalletDatabase` owns the connection and manages migrations, PIN/mnemonic caching, and wallet creation orchestration.

**Security invariant**: Mnemonics are stored encrypted (PIN + PBKDF2/AES-GCM or DPAPI/Keychain). The plaintext mnemonic is only held in `WalletDatabase.cached_mnemonic` while the wallet is unlocked.

## Key Files

| File | Purpose |
|------|---------|
| `mod.rs` | Module exports — re-exports all 16 repositories and 18 models |
| `connection.rs` | `WalletDatabase` — connection wrapper, migration runner, PIN/mnemonic cache, wallet creation/recovery orchestration, startup checks |
| `models.rs` | All data structs (18 models matching database tables) |
| `migrations.rs` | Consolidated V1 schema + incremental V2–V11 migrations |
| `migration.rs` | One-time JSON→SQLite migration (legacy `wallet.json`/`actions.json`) |
| `helpers.rs` | Key derivation helpers: `get_master_private_key_from_db`, `get_master_public_key_from_db`, `derive_key_for_output`, format converters |

## Models

| Model | Table | Key Fields | Notes |
|-------|-------|------------|-------|
| `Wallet` | `wallets` | `mnemonic`, `pin_salt`, `mnemonic_dpapi`, `current_index`, `backed_up` | Single row; mnemonic is encrypted |
| `User` | `users` | `identity_key` (master pubkey hex), `active_storage` | Default user = wallet's master pubkey |
| `Address` | `addresses` | `wallet_id`, `index`, `address`, `public_key`, `pending_utxo_check` | index: -1=master, -2=external, 0+=derived |
| `Output` | `outputs` | `user_id`, `txid`, `vout`, `satoshis`, `spendable`, `spent_by`, `derivation_prefix/suffix`, `locking_script` (BLOB) | Primary UTXO tracking table |
| `ParentTransaction` | `parent_transactions` | `txid`, `raw_hex` | Raw tx cache for BEEF building |
| `BlockHeader` | `block_headers` | `block_hash`, `height`, `header_hex` | Cached for TSC proof enhancement |
| `ProvenTx` | `proven_txs` | `txid`, `height`, `merkle_path` (BLOB), `raw_tx` (BLOB) | Immutable — never updated after creation |
| `ProvenTxReq` | `proven_tx_reqs` | `txid`, `status`, `attempts`, `proven_tx_id` FK | Proof acquisition lifecycle |
| `Basket` | `output_baskets` | `user_id`, `name` (normalized) | `"default"` reserved for change outputs |
| `OutputTag` / `OutputTagMap` | `output_tags` / `output_tag_map` | `tag` (normalized), `output_id` FK | Many-to-many, soft delete |
| `TxLabel` / `TxLabelMap` | `tx_labels` / `tx_labels_map` | `label` (normalized), `transaction_id` FK | Many-to-many, soft delete |
| `Commission` | `commissions` | `transaction_id` (unique), `satoshis`, `is_redeemed` | One commission per transaction max |
| `Setting` | `settings` | `chain`, `max_output_script`, `sender_display_name`, default limits | Singleton row |
| `SyncState` | `sync_states` | `user_id`, `status`, `ref_num`, `sync_map` (JSON) | Multi-device sync tracking |
| `DomainPermission` | `domain_permissions` | `user_id`, `domain`, `trust_level`, spending limits | Per-site wallet permissions |
| `CertFieldPermission` | `cert_field_permissions` | `domain_permission_id`, `cert_type`, `field_name` | Which cert fields a domain can see |
| `RelayMessage` | `relay_messages` | `recipient`, `message_box`, `sender`, `body` | BRC-33 PeerServ message relay (defined in `message_relay_repo.rs`) |
| `ReceivedPayment` | `peerpay_received` | `message_id` (unique), `amount_satoshis`, `source`, `dismissed` | PeerPay + address sync notifications (defined in `peerpay_repo.rs`) |

## WalletDatabase (`connection.rs`)

Central orchestrator. Owns the `Connection`, runs migrations on init, caches the plaintext mnemonic.

**PIN/mnemonic lifecycle:**
- `is_pin_protected()`, `is_unlocked()`, `unlock(pin)`, `get_cached_mnemonic()`
- `cache_mnemonic(mnemonic)` — after create/recover when mnemonic is known
- `clear_cached_mnemonic()` — after wallet deletion
- `try_dpapi_unlock()` — auto-unlock via DPAPI (Windows) / Keychain (macOS)
- `store_dpapi_blob(wallet_id, mnemonic)` — backfill DPAPI for pre-DPAPI wallets

**Wallet creation:**
- `create_wallet_with_first_address(pin)` — new wallet: generates mnemonic → user → default basket → BRC-42 address (index 0) → master address (index -1)
- `create_wallet_from_existing_mnemonic(mnemonic, pin)` — recovery flow: same orchestration but uses provided mnemonic, sets `backed_up=true`

**Startup checks:**
- `ensure_master_address_exists()` — backfills master pubkey address (index -1) for pre-existing wallets
- `ensure_default_basket_exists()` — backfills "default" basket for pre-existing wallets

## Repositories

### WalletRepository (`wallet_repo.rs`)
Manages the `wallets` table. Single-wallet design (first row = primary).

- `create_wallet(pin)` — Generates 12-word BIP39 mnemonic, encrypts with PIN (PBKDF2+AES-GCM) and DPAPI, returns `(wallet_id, plaintext_mnemonic)`
- `create_wallet_with_mnemonic(phrase, pin)` — Recovery flow: validates existing mnemonic, inserts with `backed_up=true`
- `get_primary_wallet()` — Returns first wallet (ORDER BY id ASC LIMIT 1)
- `get_by_id(id)`, `update_current_index(id, index)`, `mark_backed_up(id)`

### AddressRepository (`address_repo.rs`)
HD address derivation cache. Special indices: `-1` = master pubkey, `-2` = external/custom script.

- `create(address)`, `get_by_address(str)`, `get_by_wallet_and_index(wallet_id, index)`
- `get_all_by_wallet(wallet_id)`, `get_max_index(wallet_id)` — excludes special indices
- `get_pending_utxo_check(wallet_id)` — addresses with `pending_utxo_check=1` OR `index=-1`
- `clear_pending_utxo_check(id)`, `clear_pending_utxo_check_batch(ids)`, `set_all_pending_utxo_check(wallet_id)` — rescan support
- `clear_stale_pending_addresses(max_age_hours)` — time-based cleanup
- `get_or_create_external_address(wallet_id)` — placeholder for custom script outputs (index -2)

### OutputRepository (`output_repo.rs`)
**Primary UTXO tracking** — the sole source of truth for wallet balance. Replaces deprecated `utxos` table.

Key design: `spendable=1` means available (inverse of old `is_spent`). `spent_by` is FK to `transactions.id`. Locking scripts stored as BLOB, not hex.

**Read methods:**
- `get_by_id(id)`, `get_by_txid_vout(txid, vout)`
- `get_spendable_by_user(user_id)` — excludes `unsigned`/`failed` transaction outputs
- `get_spendable_confirmed_by_user(user_id)` — only `completed` status (for confirmed-preference UTXO selection)
- `get_all_by_user(user_id)` — all outputs (spendable and spent), used for backup/export
- `get_spendable_by_basket(basket_id)`, `get_spendable_by_basket_with_tags(basket_id, tag_ids, require_all)`
- `get_spendable_by_derivation(prefix, suffix)` — for UTXO sync reconciliation
- `calculate_balance(user_id)`, `calculate_total_balance()`, `count_spendable(user_id)`
- `get_locking_script_hex(output_id)` — converts BLOB to hex string

**Write methods:**
- `insert_output(...)` — new output with explicit fields
- `upsert_received_utxo(user_id, txid, vout, satoshis, script_hex, address_index)` — INSERT OR IGNORE for API-synced UTXOs
- `upsert_received_utxo_with_derivation(...)` — recovery variant with explicit BIP32/BRC-42 method
- `mark_spent(txid, vout, spending_txid)`, `mark_multiple_spent(outputs, spending_txid)`
- `update_txid(old, vout, new)`, `update_txid_batch(old, new)` — post-signing txid rename
- `update_spending_description_batch(placeholder, real_txid)` — replace placeholder with actual txid + set spent_by FK
- `link_outputs_to_transaction(txid, transaction_id)` — set `transaction_id` FK after tx saved
- `delete_by_txid(txid)` — cleanup failed broadcasts
- `restore_by_spending_description(placeholder)`, `restore_spent_by_txid(txid)`, `restore_pending_placeholders()` — UTXO restoration on failure
- `reconcile_for_derivation(user_id, prefix, suffix, api_utxos, grace_secs)` — mark stale outputs as `external-spend`
- `assign_basket(output_id, basket_id)`, `remove_from_basket(output_id)`
- `cleanup_old_spent(days)` — delete spent outputs older than N days

### TransactionRepository (`transaction_repo.rs`)
Transaction records with status lifecycle and label management.

- `add_transaction(action, user_id)` — inserts transaction + labels (via `tx_labels`/`tx_labels_map`) + inputs + outputs
- `get_by_txid(txid)`, `get_by_reference(reference_number)` — full `StoredAction` with labels/inputs/outputs
- `set_transaction_status(txid, status)` — sets `failed_at` timestamp for Failed, clears it otherwise
- `update_status(txid, ActionStatus)` — legacy interface, converts to `TransactionStatus`
- `get_transaction_status(txid)` — returns `TransactionStatus` enum
- `get_broadcast_status(txid)` — returns raw status string
- `update_broadcast_status(txid, status_str)` — legacy broadcast status mapping
- `update_txid(reference, new_txid, new_raw_tx, user_id)` — replace entire tx record (two-phase signing); detaches and re-links output FKs
- `rename_txid(old, new)` — post-signing txid update
- `update_raw_tx(txid, raw_tx)` — critical for BEEF: signed tx replaces unsigned
- `update_confirmations(txid, confirmations, block_height)` — update confirmation count
- `get_raw_tx(txid)` — efficient raw tx fetch (no labels/inputs/outputs)
- `get_local_parent_tx(txid)` — unconfirmed parent for BEEF chain building
- `get_stale_pending_transactions(max_age_secs)` — for TaskFailAbandoned cleanup
- `list_transactions(label_filter, label_mode)` — with optional "any"/"all" label matching

### CertificateRepository (`certificate_repo.rs`)
BRC-52 identity certificates with fields. Stores type/serial/certifier as base64, pubkeys as hex.

- `insert_certificate_with_fields(cert)` — atomic insert of cert + all fields
- `get_by_identifiers(type_, serial, certifier)` — lookup with fields
- `list_certificates(type_filter, certifier_filter, subject_filter, is_deleted, limit, offset)` — paginated filtering
- `update_relinquished(type_, serial, certifier)` — soft delete

### ProvenTxRepository (`proven_tx_repo.rs`)
**Immutable** confirmed transaction + merkle proof records. Created by Monitor's `TaskCheckForProofs`.

- `insert_or_get(txid, height, tx_index, merkle_path, raw_tx, block_hash, merkle_root)` — INSERT OR IGNORE + SELECT
- `get_by_txid(txid)`, `get_by_id(id)`
- `get_merkle_proof_as_tsc(txid)` — deserializes BLOB to JSON, normalizes array→object, injects height if missing
- `link_transaction(txid, proven_tx_id)` — sets `proven_tx_id` FK on transactions table

### ProvenTxReqRepository (`proven_tx_req_repo.rs`)
Proof acquisition lifecycle. Mutable records that progress through: `sending` → `unproven` → `completed` (or `failed`/`invalid`).

- `create(txid, raw_tx, input_beef, status)` — INSERT OR IGNORE
- `get_by_txid(txid)`, `get_pending()` — non-terminal status only
- `update_status(id, status)`, `increment_attempts(id)`, `link_proven_tx(id, proven_tx_id)`
- `delete_by_txid(txid)` — cleanup stale req when txid changes during two-phase signing
- `add_history_note(id, event, details)` — append timestamped entry to JSON history

### DomainPermissionRepository (`domain_permission_repo.rs`)
Per-site wallet permissions with spending limits and certificate field access control.

- `get_by_domain(user_id, domain)`, `upsert(perm)`, `update_trust_level(id, level)`
- `list_all(user_id)`, `delete(id)`, `reset_all_limits(user_id, per_tx, per_session, rate)`
- `get_approved_fields(domain_perm_id, cert_type)`, `approve_fields(...)`, `revoke_field(...)`
- `check_fields_approved(domain_perm_id, cert_type, fields)` — returns `(approved, unapproved)` vectors

### PeerPayRepository (`peerpay_repo.rs`)
Notification tracking for received payments (PeerPay and address sync). Static methods (no `&self`).

- `insert_received(conn, message_id, sender, amount, ...)` — INSERT OR IGNORE for deduplication
- `insert_address_sync_notification(conn, txid, vout, amount, ...)` — uses `utxo:{txid}:{vout}` as message_id
- `is_already_processed(conn, message_id)` — dedup check
- `get_undismissed(conn)`, `get_undismissed_summary(conn)` — for notification badge (count + total sats)
- `dismiss_all(conn)` — mark all as seen

### UserRepository (`user_repo.rs`)
Identity mapping (master pubkey → userId). Single-user wallets have one default user.

- `create(identity_key)` — creates user with `active_storage="local"`
- `get_by_id(user_id)`, `get_by_identity_key(identity_key)`
- `get_default()` — returns first user (ORDER BY userId ASC LIMIT 1)
- `update_active_storage(user_id, storage)` — update storage mode

### Other Repositories

**BasketRepository** (`basket_repo.rs`): Output categorization. Names normalized (trim+lowercase). `"default"` reserved for change. `"p "` prefix reserved (BRC-99).
- `find_or_insert(name, user_id)` — idempotent, normalizes input
- `find_by_name(name)`, `get_by_id(id)`

**TagRepository** (`tag_repo.rs`): Output tagging via `output_tags`/`output_tag_map`. Names normalized. Soft delete support.
- `find_or_insert(tag)`, `find_tag_ids(tags)`, `get_tags_for_output(output_id)`
- `assign_tag_to_output(output_id, tag_name)`, `remove_tag_from_output(output_id, tag_name)`
- `get_labels_for_transaction(tx_id)`, `get_labels_for_txid(txid)` — cross-table label queries

**TxLabelRepository** (`tx_label_repo.rs`): Transaction labels via `tx_labels`/`tx_labels_map`. Deduplicated per user, normalized, soft delete.
- `find_or_insert(user_id, label)`, `find_label_ids(user_id, labels)`
- `assign_label_to_transaction(user_id, tx_id, label)`, `remove_label_from_transaction(...)`
- `get_labels_for_transaction(tx_id)`, `get_all_labels(user_id)`, `delete_label(label_id)`

**CommissionRepository** (`commission_repo.rs`): Fee tracking per transaction (max one per tx).
- `create(commission)`, `get_by_id(id)`, `get_by_transaction_id(tx_id)`
- `get_unredeemed(user_id)`, `mark_redeemed(id)`, `get_total_unredeemed(user_id)`

**SettingsRepository** (`settings_repo.rs`): Singleton config row.
- `get()`, `upsert(setting)`, `ensure_defaults()`
- `get_chain()`, `set_chain(chain)`, `get_sender_display_name()`, `set_sender_display_name(name)`
- `get_max_output_script()`, `set_max_output_script(max_size)`
- `get_default_limits()`, `set_default_limits(per_tx, per_session, rate)`
- `set_storage(identity_key, name)` — update storage configuration

**SyncStateRepository** (`sync_state_repo.rs`): Multi-device sync tracking. Status: `unknown`→`syncing`→`synced`/`error`.
- `create(state)`, `get_by_id(id)`, `get_by_ref_num(ref_num)`, `get_by_user(user_id)`
- `update_status(id, status)`, `update_sync_map(id, json)`, `mark_synced(id, sats)`, `mark_error(id, ...)`

**ParentTransactionRepository** (`parent_transaction_repo.rs`): Raw tx cache for BEEF ancestry chains.
- `get_by_txid(txid)`, `upsert(utxo_id, txid, raw_hex)`, `verify_txid(txid, raw_hex)` — SHA256d verification

**BlockHeaderRepository** (`block_header_repo.rs`): Cached block headers.
- `get_by_hash(hash)`, `get_by_height(height)`, `upsert(hash, height, header_hex)`

**MessageRelayRepository** (`message_relay_repo.rs`): BRC-33 PeerServ message relay. Includes tests.
- `send_message(recipient, box, sender, body)`, `list_messages(recipient, box)`, `acknowledge_messages(recipient, ids)`
- `cleanup_expired()`, `cleanup_old_messages(max_age_days)`, `get_stats()`

## Schema & Migrations

**Current version**: V11 (tracked in `schema_version` table). New databases get a fully consolidated V1 schema; V2–V11 are incremental migrations for pre-existing databases. All migrations are idempotent (check column/table existence before ALTER).

Migration runner in `WalletDatabase::migrate()` (`connection.rs:607`).

| Version | Purpose |
|---------|---------|
| V1 | Consolidated schema: all tables, indexes, constraints (replaces old V1–V24 incremental chain) |
| V2 | `pin_salt` column on wallets (PIN encryption) |
| V3 | `domain_permissions` + `cert_field_permissions` tables |
| V4 | `mnemonic_dpapi` BLOB column (Windows/macOS auto-unlock) |
| V5 | No-op (adblock settings moved to C++) |
| V6 | No-op (scriptlet settings moved to C++) |
| V7 | `peerpay_received` table |
| V8 | `source` column on `peerpay_received` (unified notifications) |
| V9 | `sender_display_name` column on settings |
| V10 | `default_per_tx_limit_cents`, `default_per_session_limit_cents`, `default_rate_limit_per_min` on settings |
| V11 | `price_usd_cents` column on transactions and peerpay_received |

## Relationships

```
wallets ──1:N──> addresses
    │
    └─ derives master pubkey ──> users (identity_key)
                                    │
                                    ├──1:N──> outputs ──N:1──> transactions
                                    │            │                  │
                                    │            ├──N:1──> output_baskets
                                    │            └──N:M──> output_tags (via output_tag_map)
                                    │
                                    ├──1:N──> transactions ──N:1──> proven_txs
                                    │            │                      ↑
                                    │            └──N:M──> tx_labels    │
                                    │            └──1:1──> commissions  │
                                    │                                   │
                                    │         proven_tx_reqs ──────────┘
                                    │
                                    ├──1:N──> certificates ──1:N──> certificate_fields
                                    ├──1:N──> domain_permissions ──1:N──> cert_field_permissions
                                    ├──1:N──> sync_states
                                    └──1:N──> output_baskets
```

## Key Derivation (helpers.rs)

`derive_key_for_output(db, prefix, suffix, sender_identity_key)` routes to the correct derivation:

| `derivation_prefix` | `derivation_suffix` | `sender_identity_key` | Derivation Method |
|---------------------|---------------------|-----------------------|-------------------|
| `NULL` | `NULL` | — | Master private key directly |
| `"2-receive address"` | `"{N}"` | `NULL` | BRC-42 self-derivation |
| `"bip32"` | `"{N}"` | `NULL` | Legacy BIP32 `m/{N}` |
| any | any | `Some(pubkey)` | BRC-42 counterparty derivation |
| any | any | `NULL` | BRC-42 self-derivation (custom invoice) |

## Conventions

- **All repositories** borrow `&Connection` with lifetime `'a` — they don't own the connection
- **Normalization**: Baskets, tags, and labels are always trimmed + lowercased before storage/lookup
- **Soft delete**: `output_tag_map`, `tx_labels`, `tx_labels_map`, `certificates` use `is_deleted` flags
- **Timestamps**: All `created_at`/`updated_at` are Unix epoch seconds (`i64`)
- **INSERT OR IGNORE**: Used for idempotent inserts (`outputs`, `proven_txs`, `proven_tx_reqs`, `peerpay_received`)
- **Error pattern**: Repository methods return `rusqlite::Result<T>` or `CacheResult<T>` (for cache-layer repos)
- **No ORMs**: All SQL is hand-written with `rusqlite::params![]` for type-safe binding

## Related

- [Root CLAUDE.md](/CLAUDE.md) — project architecture and invariants
- [Wallet Backend CLAUDE.md](/rust-wallet/CLAUDE.md) — handler layer, API endpoints, Monitor tasks, full schema table
- `src/handlers.rs` — HTTP handlers that call these repositories
- `src/monitor/` — background tasks that read/write via these repositories
- `src/crypto/` — key derivation called by `helpers.rs`
