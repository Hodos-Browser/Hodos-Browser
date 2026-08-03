# Database — Wallet Data Access Layer

> SQLite-backed persistence for the Rust wallet backend. All wallet state lives here: keys, outputs, transactions, certificates, permissions, and configuration.
>
> **Last Updated:** 2026-08-03 (inventory verified against code)

## Overview

This module provides the complete data access layer for the HodosBrowser wallet. It uses `rusqlite` for synchronous SQLite access with WAL mode, foreign keys, a 5-second busy timeout, and `synchronous=FULL` (per-commit fsync — deliberate for a money DB; an earlier commit set `NORMAL` and was reverted 2026-06-29). The database file lives at `<app_data>/wallet/wallet.db` (`main.rs` builds the path; `%APPDATA%/HodosBrowser/` in production, `%APPDATA%/HodosBrowserDev/` under `HODOS_DEV=1`).

**Architecture**: Repository pattern — each table group has a dedicated `*Repository` struct that borrows a `&Connection`. The central `WalletDatabase` owns the connection and manages migrations, PIN/mnemonic caching, and wallet creation orchestration.

**Security invariant**: Mnemonics are stored encrypted (PIN + PBKDF2/AES-GCM or DPAPI/Keychain). The plaintext mnemonic is only held in `WalletDatabase.cached_mnemonic` while the wallet is unlocked.

**Directory inventory**: 26 `.rs` files — 20 repository modules plus `connection.rs`, `migrations.rs`, `migration.rs`, `models.rs`, `helpers.rs`, `mod.rs`.

## Key Files

| File | Purpose |
|------|---------|
| `mod.rs` | Module exports — re-exports 20 repositories and the 21 structs in `models.rs` (plus `RelayMessage`, `MessageRelayStats`, `PermissionAuditEntry` from their repo modules) |
| `connection.rs` | `WalletDatabase` — connection wrapper, migration runner, PIN/mnemonic cache, wallet creation/recovery orchestration, startup checks and startup column repairs |
| `models.rs` | Data structs — 21 structs matching database tables |
| `migrations.rs` | Consolidated V1 schema + incremental migrations `migrate_v1_to_v2` … `migrate_v22_to_v23` (22 incremental functions) |
| `migration.rs` | One-time JSON→SQLite migration (legacy `wallet.json`/`actions.json`). `migrate_json_to_database` is exported from `mod.rs` but has no remaining in-tree caller — legacy/dormant |
| `helpers.rs` | Key derivation helpers: `get_master_private_key_from_db`, `get_master_public_key_from_db`, `derive_key_for_output`, plus format converters `address_to_address_info`, `output_to_fetcher_utxo` |

## Models

`models.rs` defines 21 structs. Five more model-shaped structs live next to the repository that owns them (noted below).

| Model | Table | Key Fields | Notes |
|-------|-------|------------|-------|
| `Wallet` | `wallets` | `mnemonic`, `pin_salt`, `mnemonic_dpapi`, `current_index`, `backed_up` | Single row; mnemonic is encrypted |
| `User` | `users` | `identity_key` (master pubkey hex), `active_storage` | Default user = wallet's master pubkey |
| `Address` | `addresses` | `wallet_id`, `index`, `address`, `public_key`, `used`, `balance`, `pending_utxo_check` | index: -1=master, -2=external, -3=backup, 0+=derived |
| `Output` | `outputs` | `user_id`, `txid`, `vout`, `satoshis`, `spendable`, `change`, `spent_by`, `derivation_prefix/suffix`, `sender_identity_key`, `locking_script` (BLOB) | Primary UTXO tracking table. Note: the `confirmed` column (V14) is **not** a field on the struct — it is read/written by dedicated `OutputRepository` methods |
| `ParentTransaction` | `parent_transactions` | `txid`, `raw_hex`, `utxo_id` | Raw tx cache for BEEF building |
| `BlockHeader` | `block_headers` | `block_hash`, `height`, `header_hex` | Cached for TSC proof enhancement |
| `ProvenTx` | `proven_txs` | `txid`, `height`, `merkle_path` (BLOB), `raw_tx` (BLOB) | Immutable in normal operation; `replace_proof`/`delete_by_txid` exist for repair paths |
| `ProvenTxReq` | `proven_tx_reqs` | `txid`, `status`, `attempts`, `proven_tx_id` FK | Proof acquisition lifecycle |
| `Basket` | `output_baskets` | `user_id`, `name` (normalized) | `"default"` reserved for change outputs |
| `OutputTag` / `OutputTagMap` | `output_tags` / `output_tag_map` | `tag` (normalized), `output_id` FK | Many-to-many, soft delete |
| `TxLabel` / `TxLabelMap` | `tx_labels` / `tx_labels_map` | `label` (normalized), `transaction_id` FK | Many-to-many, soft delete |
| `Commission` | `commissions` | `transaction_id` (unique), `satoshis`, `is_redeemed` | One commission per transaction max |
| `Setting` | `settings` | `storage_identity_key`, `storage_name`, `chain`, `db_type`, `max_output_script`, `sender_display_name` | Singleton row. The default-limit / backup / disclosure columns on the table are **not** struct fields — reach them via `SettingsRepository::get_default_limits`, `get_backup_hash`, `get_last_backup_at` |
| `SyncState` | `sync_states` | `user_id`, `status`, `init`, `ref_num`, `sync_map` (JSON) | Multi-device sync tracking |
| `DomainPermission` | `domain_permissions` | `trust_level`, `per_tx_limit_cents`, `per_session_limit_cents`, `rate_limit_per_min`, `max_tx_per_session`, `identity_key_disclosure_allowed`, `bundled_scope_grant` | Per-site wallet permissions. `DomainPermission::defaults()` = 100 cents/tx ($1.00), 1000 cents/session ($10.00), 30 calls/min, 100 tx/session, both booleans false |
| `CertFieldPermission` | `cert_field_permissions` | `domain_permission_id`, `cert_type`, `field_name` | Which cert fields a domain can see |
| `DomainProtocolPermission` | `domain_protocol_permissions` | `protocol_security_level`, `protocol_name`, `key_id` (`"*"` = wildcard), `counterparty`, `expires_at`, `revoked_at` | V18 child table of `domain_permissions` (CASCADE) |
| `DomainBasketPermission` | `domain_basket_permissions` | `basket`, `access` (`read`\|`read_write`), `expires_at`, `revoked_at` | V18 child table (CASCADE) |
| `DomainCounterpartyPermission` | `domain_counterparty_permissions` | `counterparty` (hex compressed pubkey), `expires_at`, `revoked_at` | V18 child table (CASCADE), level-2 protocols |
| `RelayMessage` / `MessageRelayStats` | `relay_messages` | `recipient`, `message_box`, `sender`, `body` | BRC-33 PeerServ message relay (defined in `message_relay_repo.rs`) |
| `ReceivedPayment` | `peerpay_received` | `message_id` (unique), `amount_satoshis`, `source`, `notification_type`, `dismissed` | PeerPay + address-sync notifications (defined in `peerpay_repo.rs`) |
| `OutboxEntry` | `peerpay_outbox` | `txid` (unique), `recipient_pubkey_hex`, `payload_bytes`, `status`, `retry_count`, `next_retry_at` | MessageBox delivery retry queue (defined in `peerpay_repo.rs`) |
| `PermissionAuditEntry` | `permission_audit_log` | `approval_id`, `domain`, `endpoint`, `call_kind`, `engine_reason`, `decision`, `body_hash` (sha256 hex) | Audit surface for the Rust permission engine (defined in `permission_audit_repo.rs`) |

## WalletDatabase (`connection.rs`)

Central orchestrator. Owns the `Connection`, runs migrations on init, caches the plaintext mnemonic.

**Connection/lifecycle:**
- `new(db_path)` — opens the DB, sets WAL + foreign keys + 5s busy timeout + `synchronous=FULL`, then runs `migrate()`
- `connection()`, `path()`, `checkpoint_truncate()`, `test_connection()`

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
- `ensure_backup_address_exists()` — backfills the on-chain backup address (index -3); requires the cached mnemonic
- `ensure_default_basket_exists()` — backfills "default" basket for pre-existing wallets

## Repositories

20 repository structs across 20 `*_repo.rs` files. All borrow `&'a Connection` except `PeerPayRepository`, which is a unit struct with static methods taking `conn` as the first argument.

### WalletRepository (`wallet_repo.rs`)
Manages the `wallets` table. Single-wallet design (first row = primary).

- `create_wallet(pin)` — Generates 12-word BIP39 mnemonic, encrypts with PIN (PBKDF2+AES-GCM) and DPAPI, returns `(wallet_id, plaintext_mnemonic)`
- `create_wallet_with_mnemonic(phrase, pin)` — Recovery flow: validates existing mnemonic, inserts with `backed_up=true`
- `get_primary_wallet()` — Returns first wallet (ORDER BY id ASC LIMIT 1)
- `get_by_id(id)`, `update_current_index(id, index)`, `mark_backed_up(id)`

### AddressRepository (`address_repo.rs`)
HD address derivation cache. Special indices: `-1` = master pubkey, `-2` = external/custom script, `-3` = on-chain backup address.

- `create(address)`, `get_by_address(str)`, `get_by_wallet_and_index(wallet_id, index)`
- `get_all_by_wallet(wallet_id)`, `get_max_index(wallet_id)` — `WHERE "index" >= 0`, so special indices are excluded
- `update_balance(address_id, balance)`, `mark_used(id)`
- `get_pending_utxo_check(wallet_id)` — addresses with `pending_utxo_check=1` OR `index=-1`
- `clear_pending_utxo_check(id)`, `clear_pending_utxo_check_batch(ids)`, `set_all_pending_utxo_check(wallet_id)` — rescan support
- `clear_stale_pending_addresses(max_age_hours)` — time-based cleanup
- `get_or_create_external_address(wallet_id)` — placeholder for custom script outputs (index -2)

### OutputRepository (`output_repo.rs`)
**Primary UTXO tracking** — the sole source of truth for wallet balance. Replaces the deprecated `utxos` table.

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
- `get_stale_unconfirmed(...)`, `get_suspected_double_spends(...)`

**Write methods:**
- `insert_output(...)` — new output with explicit fields
- `upsert_received_utxo(...)` — INSERT OR IGNORE for API-synced UTXOs; `upsert_received_utxo_with_confirmed(...)` and `upsert_received_utxo_with_derivation(...)` variants carry the confirmed flag / explicit BIP32-vs-BRC-42 method
- `mark_output_confirmed(...)`, `delete_unconfirmed_output(...)`
- `mark_spent(txid, vout, spending_txid)`, `mark_multiple_spent(outputs, spending_txid)`
- `update_txid(old, vout, new)`, `update_txid_batch(old, new)` — post-signing txid rename
- `update_derivation(...)`, `update_derivation_with_sender(...)`
- `update_spending_description_batch(placeholder, real_txid)` — replace placeholder with actual txid + set spent_by FK
- `link_outputs_to_transaction(txid, transaction_id)` — set `transaction_id` FK after tx saved
- `delete_by_txid(txid)`, `disable_by_txid(txid)`, `reenable_failed_outputs(...)` — cleanup / recovery for failed broadcasts
- `restore_by_spending_description(placeholder)`, `restore_spent_by_txid(txid)`, `restore_pending_placeholders()` — UTXO restoration on failure
- `assign_basket(output_id, basket_id)`, `remove_from_basket(output_id)`
- `confirm_double_spend(...)`, `clear_suspected_double_spend(...)`
- `cleanup_old_spent(days)` — delete spent outputs older than N days

> **Removed:** `reconcile_for_derivation(...)` was **deleted 2026-04-20** (tombstone comment retained in `output_repo.rs`). It inferred "externally spent" from absence in a WoC bulk address query, which wrongly killed unconfirmed and PushDrop/nonstandard outputs — ~$15 of valid on-chain outputs were marked non-spendable. Any future reconciliation must verify per-output via `GET /tx/{txid}/{vout}/spent` and never infer spent-ness from a bulk query.

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
- `get_certificate_fields(certificate_id)`
- `list_certificates(type_filter, certifier_filter, subject_filter, is_deleted, limit, offset)` — paginated filtering
- `update_relinquished(type_, serial, certifier)` — soft delete
- `update_publish_status(...)`, `get_publish_info(...)` — on-chain publish state (`publish_status` / `publish_txid` / `publish_vout` columns, added by the startup repair block in `migrate()`)

### ProvenTxRepository (`proven_tx_repo.rs`)
Confirmed transaction + merkle proof records. Created by Monitor's `TaskCheckForProofs`. Treated as immutable in normal operation; the delete/replace methods exist only for proof-repair paths.

- `insert_or_get(txid, height, tx_index, merkle_path, raw_tx, block_hash, merkle_root)` — INSERT OR IGNORE + SELECT
- `get_by_txid(txid)`, `get_by_id(id)`
- `get_merkle_proof_as_tsc(txid)` — deserializes BLOB to JSON, normalizes array→object, injects height if missing
- `link_transaction(txid, proven_tx_id)` — sets `proven_tx_id` FK on transactions table
- `delete_by_txid(txid)`, `replace_proof(...)` — repair paths for bad/stale proofs

### ProvenTxReqRepository (`proven_tx_req_repo.rs`)
Proof acquisition lifecycle. Mutable records that progress through: `sending` → `unproven` → `completed` (or `failed`/`invalid`).

- `create(txid, raw_tx, input_beef, status)` — INSERT OR IGNORE
- `get_by_txid(txid)`, `get_pending()` — non-terminal status only
- `update_status(id, status)`, `increment_attempts(id)`, `link_proven_tx(id, proven_tx_id)`
- `delete_by_txid(txid)` — cleanup stale req when txid changes during two-phase signing
- `add_history_note(id, event, details)` — append timestamped entry to JSON history

### DomainPermissionRepository (`domain_permission_repo.rs`)
Per-site wallet permissions with spending limits, certificate field access control, and the V18 fine-grained sub-permission child tables. Has a `#[cfg(test)] mod tests`.

**Site row + cert fields:**
- `get_by_domain(user_id, domain)`, `upsert(perm)`, `update_trust_level(id, level)`
- `list_all(user_id)`, `delete(id)`, `reset_all_limits(user_id, per_tx, per_session, rate)`
- `get_approved_fields(domain_perm_id, cert_type)`, `approve_fields(...)`, `revoke_field(...)`
- `check_fields_approved(domain_perm_id, cert_type, fields)` — returns `(approved, unapproved)` vectors

**Protocol grants (`domain_protocol_permissions`):**
- `grant_protocol(...)`, `revoke_protocol(...)`, `list_protocols(...)`, `list_protocols_all(...)`, `is_protocol_granted(...)`

**Basket grants (`domain_basket_permissions`):**
- `grant_basket(...)`, `revoke_basket(...)`, `list_baskets(...)`, `list_baskets_all(...)`, `is_basket_granted(...)`

**Counterparty grants (`domain_counterparty_permissions`):**
- `grant_counterparty(...)`, `revoke_counterparty(...)`, `list_counterparties(...)`, `list_counterparties_all(...)`, `is_counterparty_granted(...)`

The `*_all` variants include revoked/expired rows (management UI); the plain variants return active grants only.

### PeerPayRepository (`peerpay_repo.rs`)
Notification tracking for received payments, chain-verification retries, and the MessageBox outbox. Unit struct — all methods are static and take `conn` as the first argument. Has a `#[cfg(test)] mod tests`.

**Received notifications (`peerpay_received`):**
- `insert_received(conn, message_id, sender, amount, ...)` — INSERT OR IGNORE for deduplication
- `insert_address_sync_notification(conn, txid, vout, amount, ...)` — uses `utxo:{txid}:{vout}` as message_id
- `insert_failure_notification(conn, ...)` — the red failure notification variant (`notification_type='failure'`)
- `is_already_processed(conn, message_id)` — dedup check
- `get_undismissed(conn)`, `get_undismissed_summary(conn)`, `get_undismissed_summary_by_type(conn)` — notification badge data
- `dismiss_all(conn)`, `dismiss_by_txid_prefix(conn, ...)`

**Chain verification (`peerpay_pending_verification`, V15):**
- `upsert_pending_verification(...)`, `remove_pending_verification(...)`, `cleanup_expired_pending(...)`, `get_pending_retry_count(...)`

**Outbox (`peerpay_outbox`, V16):**
- `insert_outbox(...)`, `get_due_outbox_entries(...)`, `update_outbox_retry_failed(...)`, `mark_outbox_delivered(...)`, `reset_outbox_for_retry(...)`, `remove_delivered_outbox(...)`, `get_outbox_summary(...)`

### PermissionAuditRepository (`permission_audit_repo.rs`)
Audit surface for the Rust permission engine (`permission_audit_log`, V20). Consumed by `src/permission_service/audit.rs`, which hashes the request body to sha256 hex before writing — the raw body is never stored. 90-day retention. Has a `#[cfg(test)] mod tests`.

- `insert(entry)`, `mark_resolved(...)`, `get_by_approval_id(...)`
- `purge_older_than(cutoff)`, `count_recent(...)`

> The companion `engine_shadow_log` table (also V20) was **dropped in V23** when the C++ `PermissionEngine` and its shadow-comparison harness were deleted in Phase 2.6-H. All permission decisions are now Rust-native (`crates/hodos_permission_engine`), so there is no second engine to compare against.

### UserRepository (`user_repo.rs`)
Identity mapping (master pubkey → userId). Single-user wallets have one default user.

- `create(identity_key)` — creates user with `active_storage="local"`
- `get_by_id(user_id)`, `get_by_identity_key(identity_key)`
- `get_default()` — returns first user (ORDER BY userId ASC LIMIT 1)
- `update_active_storage(user_id, storage)` — update storage mode

### Other Repositories

**BasketRepository** (`basket_repo.rs`): Output categorization. Names normalized (trim+lowercase). `"default"` reserved for change. `"p "` prefix (p + space) reserved (BRC-99). Free function `validate_and_normalize_basket_name(name)` enforces the rules (1–300 UTF-8 bytes).
- `find_or_insert(name, user_id)` — idempotent, normalizes input
- `find_by_name(name)`, `get_by_id(id)`

**TagRepository** (`tag_repo.rs`): Output tagging via `output_tags`/`output_tag_map`. Names normalized by free function `validate_and_normalize_tag(name)`. Soft delete support.
- `find_or_insert(tag)`, `find_tag_ids(tags)`, `get_tags_for_output(output_id)`, `get_tag_ids_for_output(output_id)`
- `assign_tag_to_output(output_id, tag_name)`, `remove_tag_from_output(output_id, tag_name)`
- `get_labels_for_transaction(tx_id)`, `get_labels_for_txid(txid)` — cross-table label queries

**TxLabelRepository** (`tx_label_repo.rs`): Transaction labels via `tx_labels`/`tx_labels_map`. Deduplicated per user, normalized by free function `validate_and_normalize_label(name)`, soft delete.
- `find_or_insert(user_id, label)`, `get_by_id(id)`, `find_label_ids(user_id, labels)`
- `assign_label_to_transaction(user_id, tx_id, label)`, `assign_labels_to_transaction(...)`, `remove_label_from_transaction(...)`
- `get_labels_for_transaction(tx_id)`, `get_labels_for_txid(txid)`, `get_all_labels(user_id)`, `delete_label(label_id)`

**CommissionRepository** (`commission_repo.rs`): Fee tracking per transaction (max one per tx).
- `create(commission)`, `get_by_id(id)`, `get_by_transaction_id(tx_id)`, `get_all()`
- `get_unredeemed(user_id)`, `mark_redeemed(id)`, `get_total_unredeemed(user_id)`, `delete_by_transaction_id(tx_id)`

**SettingsRepository** (`settings_repo.rs`): Singleton config row.
- `get()`, `upsert(setting)`, `ensure_defaults()`
- `get_chain()`, `set_chain(chain)`, `get_sender_display_name()`, `set_sender_display_name(name)`
- `get_max_output_script()`, `set_max_output_script(max_size)`
- `get_default_limits()`, `set_default_limits(per_tx, per_session, rate)` — defaults are 100 cents/tx and 1000 cents/session ($1.00 / $10.00), set by V12
- `get_backup_hash()`, `set_backup_hash(hash)`, `get_last_backup_at()`, `set_last_backup_at(ts)`
- `set_storage(identity_key, name)` — update storage configuration

**SyncStateRepository** (`sync_state_repo.rs`): Multi-device sync tracking. Status: `unknown`→`syncing`→`synced`/`error`.
- `create(state)`, `get_by_id(id)`, `get_by_ref_num(ref_num)`, `get_by_user(user_id)`, `get_pending()`
- `update_status(id, status)`, `update_sync_map(id, json)`, `mark_synced(id, sats)`, `mark_error(id, ...)`, `mark_init_complete(id)`
- `delete(id)`, `cleanup_old(...)`

**ParentTransactionRepository** (`parent_transaction_repo.rs`): Raw tx cache for BEEF ancestry chains.
- `get_by_txid(txid)`, `get_id_by_txid(txid)`, `upsert(utxo_id, txid, raw_hex)`, `verify_txid(txid, raw_hex)` — SHA256d verification

**BlockHeaderRepository** (`block_header_repo.rs`): Cached block headers.
- `get_by_hash(hash)`, `get_by_height(height)`, `upsert(hash, height, header_hex)`

**MessageRelayRepository** (`message_relay_repo.rs`): BRC-33 PeerServ message relay. Has a `#[cfg(test)] mod tests`.
- `send_message(recipient, box, sender, body)`, `list_messages(recipient, box)`, `acknowledge_messages(recipient, ids)`
- `cleanup_expired()`, `cleanup_old_messages(max_age_days)`, `get_stats()`

## Schema & Migrations

**Current version**: **V23** (tracked in the `schema_version` table; the runner reads `MAX(version)`). New databases get the consolidated V1 schema and then run every incremental migration V2→V23 in the same pass, because `current_version` starts at 0 and each guard is `current_version < N`. All migrations are idempotent (check column/table existence, or use `IF NOT EXISTS` / `DROP … IF EXISTS`).

Migration runner: `connection.rs :: WalletDatabase::migrate`. Migration bodies: `migrations.rs :: create_schema_v1` + `migrate_vN_to_vN+1` (22 incremental functions, `migrate_v1_to_v2` … `migrate_v22_to_v23`).

| Version | Purpose |
|---------|---------|
| V1 | Consolidated schema: 28 tables + indexes + constraints (`create_schema_v1`, replaces the old incremental chain) |
| V2 | `pin_salt` column on wallets (PIN encryption) |
| V3 | `domain_permissions` + `cert_field_permissions` tables |
| V4 | `mnemonic_dpapi` BLOB column (Windows DPAPI / macOS Keychain auto-unlock) |
| V5 | No-op (adblock settings moved to C++ AdblockCache) |
| V6 | No-op (scriptlet settings moved to C++ AdblockCache) |
| V7 | `peerpay_received` table |
| V8 | `source` column + index on `peerpay_received` (unified notifications) |
| V9 | `sender_display_name` column on settings |
| V10 | `default_per_tx_limit_cents`, `default_per_session_limit_cents`, `default_rate_limit_per_min` on settings |
| V11 | `price_usd_cents` column on transactions and peerpay_received |
| V12 | `max_tx_per_session` on `domain_permissions` + `default_max_tx_per_session` on settings; **rewrites the settings defaults to 100 / 1000 / 30** ($1.00 per tx, $10.00 per session, 30 calls/min) |
| V13 | `recipient` + `recipient_name` columns on transactions (send-to autocomplete) |
| V14 | `confirmed` column + index on outputs; `notification_type` column on `peerpay_received` (receive vs failure) |
| V15 | `peerpay_pending_verification` table (chain-validation retry tracking) |
| V16 | `peerpay_outbox` table (MessageBox delivery retry) |
| V17 | **Phase 1.5 Step 1.** `identity_key_disclosure_allowed` on `domain_permissions` — gates `getPublicKey({identityKey:true})` for external domains |
| V18 | **Phase 1.5 Step 2.** Three child tables of `domain_permissions`: `domain_protocol_permissions`, `domain_basket_permissions`, `domain_counterparty_permissions`. FK CASCADE, UNIQUE logical keys, nullable `expires_at` / `revoked_at` |
| V19 | **Phase 1.5 Step 5.** `default_identity_key_disclosure_allowed` on settings (default 1) — initial state of the bundle checkbox on first-visit modals |
| V20 | **Phase 2.6-A.5.** `permission_audit_log` (long-lived, 90-day retention, sha256 `body_hash`) + `engine_shadow_log` (temporary C++-vs-Rust comparison table) |
| V21 | `bsv_price_cache` table — single-row (`CHECK (id = 1)`) persistent last-known-good BSV/USD price, so a cold start with both upstream price feeds down still has a fallback |
| V22 | **Phase 2.6-D Fix #4.** `bundled_scope_grant` column on `domain_permissions` — silences ProtocolUse/BasketAccess prompts for the domain (protected baskets still prompt) |
| V23 | **Phase 2.6-H cleanup.** Drops `engine_shadow_log`; `permission_audit_log` is kept |

### Startup repair blocks (`WalletDatabase::migrate`, after V23)

The runner ends with five unconditional column-repair checks that patch DBs where a migration recorded its version but the `ALTER TABLE` never landed (staging-merge damage). Each is a `PRAGMA table_info` check + conditional `ALTER`:

1. `domain_permissions.max_tx_per_session`
2. `settings.default_max_tx_per_session`
3. `certificates.publish_status` / `publish_txid` / `publish_vout`
4. `transactions.recipient` / `recipient_name`
5. `settings.backup_hash` / `last_backup_at`

These are the **only** source of the certificate-publish and backup-hash columns — there is no numbered migration for them.

### Table inventory

V1 creates 28 tables: `wallets`, `addresses`, `parent_transactions`, `block_headers`, `transaction_inputs`, `transaction_outputs`, `messages`, `relay_messages`, `users`, `proven_txs`, `proven_tx_reqs`, `transactions`, `certificates`, `certificate_fields`, `output_baskets`, `outputs`, `output_tags`, `output_tag_map`, `tx_labels`, `tx_labels_map`, `commissions`, `monitor_events`, `sync_states`, `settings`, `derived_key_cache`, `domain_permissions`, `cert_field_permissions`, `peerpay_received`.

Later migrations add 8 more: `peerpay_pending_verification` (V15), `peerpay_outbox` (V16), `domain_protocol_permissions` / `domain_basket_permissions` / `domain_counterparty_permissions` (V18), `permission_audit_log` + `engine_shadow_log` (V20 — the latter dropped in V23), `bsv_price_cache` (V21). Plus `schema_version`, created by the runner itself.

Net: **36 live tables** after a full migrate (28 V1 + 8 added − 1 dropped + `schema_version`).

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
                                    │                            ├──1:N──> domain_protocol_permissions
                                    │                            ├──1:N──> domain_basket_permissions
                                    │                            └──1:N──> domain_counterparty_permissions
                                    ├──1:N──> sync_states
                                    └──1:N──> output_baskets
```

All four `domain_permissions` child tables use `ON DELETE CASCADE` — revoking a site nukes every sub-permission it holds.

## Key Derivation (helpers.rs)

`derive_key_for_output(db, prefix, suffix, sender_identity_key)` routes to the correct derivation:

| `derivation_prefix` | `derivation_suffix` | `sender_identity_key` | Derivation Method |
|---------------------|---------------------|-----------------------|-------------------|
| `NULL` | `NULL` | — | Master private key directly |
| `"master"` | `"{N}"` | — | Master private key directly (index -1 / service-fee address) |
| `"bip32"` | `"{N}"` | — | Legacy BIP32 `m/{N}` via `recovery::derive_private_key_bip32` |
| `"2-receive address"` | `"{N}"` | `NULL` | BRC-42 self-derivation (invoice `"{prefix}-{suffix}"`) |
| any other | any | `Some(pubkey)` | BRC-42 counterparty derivation |
| any other | any | `NULL` | BRC-42 self-derivation (custom invoice) |
| exactly one of prefix/suffix `NULL` | — | — | Logs a warning, falls back to the master private key |

## Conventions

- **All repositories** borrow `&Connection` with lifetime `'a` — they don't own the connection. Exception: `PeerPayRepository` is a unit struct with static methods that take `conn` explicitly
- **Normalization**: Baskets, tags, and labels are always trimmed + lowercased before storage/lookup; each has a `validate_and_normalize_*` free function in its repo module
- **Soft delete**: `output_tag_map`, `output_tags`, `tx_labels`, `tx_labels_map`, `certificates`, `output_baskets` use `is_deleted` flags; the V18 domain sub-permission tables use a nullable `revoked_at` timestamp instead (audit-friendly)
- **Timestamps**: All `created_at`/`updated_at` are Unix epoch seconds (`i64`)
- **INSERT OR IGNORE**: Used for idempotent inserts (`outputs`, `proven_txs`, `proven_tx_reqs`, `peerpay_received`)
- **Error pattern**: Repository methods return `rusqlite::Result<T>` or `CacheResult<T>` (for cache-layer repos)
- **No ORMs**: All SQL is hand-written with `rusqlite::params![]` for type-safe binding
- **Tests**: four modules carry `#[cfg(test)] mod tests` — `domain_permission_repo.rs`, `message_relay_repo.rs`, `peerpay_repo.rs`, `permission_audit_repo.rs`

## Related

- [Root CLAUDE.md](/CLAUDE.md) — project architecture and invariants
- [Wallet Backend CLAUDE.md](/rust-wallet/CLAUDE.md) — handler layer, API endpoints, Monitor tasks. **Its schema/migration section is stale** (it headlines V19 and lists a V20–V24 chain that does not exist). `migrations.rs` is the only authority; this file mirrors it
- `src/permission_service/` + `crates/hodos_permission_engine/` — the Rust permission decision engine that writes `permission_audit_log`
- `src/handlers.rs` — HTTP handlers that call these repositories
- `src/monitor/` — background tasks that read/write via these repositories
- `src/crypto/` — key derivation called by `helpers.rs`
