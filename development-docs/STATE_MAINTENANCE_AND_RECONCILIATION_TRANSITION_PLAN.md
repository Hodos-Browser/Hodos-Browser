# State Maintenance & Reconciliation — Transition Plan

**Created**: 2026-02-03
**Status**: In Progress — Phases 1-6 ✅ Complete, Phases 7-8 Pending
**Objective**: Evolve the Hodos Rust wallet database and state management to align with the BSV SDK wallet-toolbox, enabling future multi-user support, cloud sync, and wallet export/import interoperability.
**See also**: `WALLET_BACKUP_AND_RECOVERY_PLAN.md` — backup file export, on-chain encrypted backup, and cloud sync scaffolding (interleaved with this plan's phases).

---

## Table of Contents

1. [Current vs Target Overview](#1-current-vs-target-overview)
2. [Complete Table Mapping](#2-complete-table-mapping)
3. [Status System Transition](#3-status-system-transition)
4. [Phased Implementation Plan](#4-phased-implementation-plan)
   - [Phase 1: Status Consolidation + UnFail](#phase-1-status-consolidation--unfail-foundation)
   - [Phase 2: Proven Transaction Model](#phase-2-proven-transaction-model)
   - [Phase 3: Multi-User Foundation](#phase-3-multi-user-foundation)
   - [Phase 4: Output Model Transition](#phase-4-output-model-transition)
   - [Phase 5: Labels, Commissions, Supporting Tables](#phase-5-labels-commissions-supporting-tables)
   - [Phase 6: Monitor Pattern](#phase-6-monitor-pattern-background-services)
   - [Phase 7: Per-Output Key Derivation](#phase-7-per-output-key-derivation)
   - [Phase 8: Cleanup](#phase-8-cleanup--deprecated-table-removal)
5. [Data Migration Strategy](#5-data-migration-strategy)
6. [Export/Import Interoperability](#6-exportimport-interoperability)

---

## 1. Current vs Target Overview

### Table Count

| Category | Hodos Current | wallet-toolbox Target | Notes |
|----------|:---:|:---:|-------|
| Core wallet | 2 (wallets, addresses) | 1 (users) | Address table eliminated; per-output derivation |
| Transactions | 4 (transactions, tx_inputs, tx_outputs, tx_labels) | 3 (transactions, tx_labels, tx_labels_map) | Inputs/outputs implicit in rawTx |
| Outputs/UTXOs | 1 (utxos) | 1 (outputs) | Major restructure |
| Proofs | 3 (parent_txs, merkle_proofs, block_headers) | 2 (proven_txs, proven_tx_reqs) | Merge + add request tracking |
| Baskets & Tags | 3 (baskets, output_tags, output_tag_map) | 3 (output_baskets, output_tags, output_tags_map) | Add userId, UTXO targets |
| Certificates | 2 (certificates, certificate_fields) | 2 (certificates, certificate_fields) | Add userId |
| New tables needed | — | 4 (commissions, monitor_events, settings, sync_states) | Multi-user/sync support |
| Browser-specific (keep) | 4 (domain_whitelist, messages, relay_messages, derived_key_cache) | N/A | Not in wallet-toolbox |

### Architecture Shift

```
CURRENT                                    TARGET
─────────────────────                      ─────────────────────
HD Address Table                           Per-Output Key Derivation
  address index → key                       derivationPrefix/Suffix → key

Dual Status System                         Single TransactionStatus
  ActionStatus (7 values)                    9 values: completed, unprocessed,
  + broadcast_status (4 values)              sending, unproven, unsigned,
                                             nosend, nonfinal, failed, unfail

UTXO model (is_spent + spent_txid)         Output model (spendable + spentBy FK)
  Reservation: pending-{timestamp}           Reservation: spendable=false, spentBy=txId

External API = source of truth             Local wallet = source of truth
  WhatsOnChain for balance                   Balance from local outputs
  API sync every 5 min                       Local tracking + proof verification

Cache tables (parent_txs + merkle_proofs)  Immutable proven_txs + mutable proven_tx_reqs
  Mutable, re-fetched                        Immutable proofs, retry lifecycle

Single-user                                Multi-user (userId on all tables)

Ad-hoc background services                 Monitor pattern with named tasks
  3 independent tokio tasks                  13 specialized tasks with triggers
```

---

## 2. Complete Table Mapping

### 2.1 wallets → users

| Hodos Column | Type | wallet-toolbox Column | Type | Migration Action |
|---|---|---|---|---|
| id | INTEGER PK | userId | INTEGER PK AUTO | Rename |
| mnemonic | TEXT | *(not in users table)* | — | Move to secure storage or encrypted field |
| current_index | INTEGER | *(eliminated)* | — | Remove (per-output derivation replaces HD index) |
| backed_up | BOOLEAN | *(not in users table)* | — | Move to settings or keep as extension |
| created_at | INTEGER (unix) | created_at | TIMESTAMP | Convert format |
| *(new)* | — | identityKey | TEXT(130) UNIQUE | Add: master public key as identity |
| *(new)* | — | activeStorage | TEXT | Add: for sync support |
| *(new)* | — | updated_at | TIMESTAMP | Add |

**Notes**: The wallets table stores mnemonic directly. The wallet-toolbox users table stores an identityKey (public key) and delegates private key storage to the application layer. We need to decide where mnemonic goes — likely an encrypted separate table or OS keychain.

### 2.2 addresses → (eliminated in Phase 7)

| Hodos Column | Disposition |
|---|---|
| id, wallet_id, index, address, public_key | Replaced by per-output derivationPrefix/derivationSuffix on outputs table |
| used, balance | Computed from outputs (spendable, satoshis) |
| pending_utxo_check | Eliminated (no address-based UTXO scanning) |

**Notes**: This is the most disruptive change. It affects key derivation for signing, UTXO sync, balance calculation, and address generation. Deferred to Phase 7.

### 2.3 utxos → outputs

| Hodos Column | Type | wallet-toolbox Column | Type | Migration Action |
|---|---|---|---|---|
| id | INTEGER PK | outputId | INTEGER PK AUTO | Rename |
| address_id | INTEGER FK nullable | *(eliminated)* | — | Remove (replaced by derivation fields) |
| basket_id | INTEGER FK nullable | basketId | INTEGER FK nullable | Keep |
| txid | TEXT | txid | TEXT(64) nullable | Keep |
| vout | INTEGER | vout | INTEGER | Keep |
| satoshis | INTEGER | satoshis | BIGINT | Widen type |
| script | TEXT (hex) | lockingScript | BINARY nullable | Change encoding |
| first_seen | INTEGER | created_at | TIMESTAMP | Rename + convert |
| last_updated | INTEGER | updated_at | TIMESTAMP | Rename + convert |
| is_spent | BOOLEAN | *(eliminated)* | — | Replace with spendable + spentBy |
| spent_txid | TEXT nullable | *(eliminated)* | — | Replace with spentBy FK |
| spent_at | INTEGER nullable | *(eliminated)* | — | Remove |
| custom_instructions | TEXT nullable | customInstructions | TEXT(2500) nullable | Rename |
| status | TEXT | *(eliminated)* | — | Status tracked via parent transaction's status |
| output_description | TEXT nullable | outputDescription | TEXT(2048) nullable | Rename |
| *(new)* | — | userId | INTEGER FK | Add |
| *(new)* | — | transactionId | INTEGER FK | Add: links output to creating transaction |
| *(new)* | — | spendable | BOOLEAN default false | Add: replaces is_spent |
| *(new)* | — | change | BOOLEAN default false | Add: identifies change outputs |
| *(new)* | — | spentBy | INTEGER FK nullable | Add: FK to spending transaction |
| *(new)* | — | providedBy | TEXT(130) | Add: 'you', 'storage', 'you-and-storage' |
| *(new)* | — | purpose | TEXT(20) | Add |
| *(new)* | — | type | TEXT(50) | Add |
| *(new)* | — | senderIdentityKey | TEXT(130) nullable | Add |
| *(new)* | — | derivationPrefix | TEXT(200) nullable | Add: replaces address-based derivation |
| *(new)* | — | derivationSuffix | TEXT(200) nullable | Add: replaces address-based derivation |
| *(new)* | — | sequenceNumber | INTEGER nullable | Add |
| *(new)* | — | spendingDescription | TEXT(2048) nullable | Add |
| *(new)* | — | scriptLength | BIGINT nullable | Add |
| *(new)* | — | scriptOffset | BIGINT nullable | Add |

**Unique constraint changes**: (txid, vout) → (transactionId, vout, userId)

### 2.4 transactions → transactions

| Hodos Column | Type | wallet-toolbox Column | Type | Migration Action |
|---|---|---|---|---|
| id | INTEGER PK | transactionId | INTEGER PK AUTO | Rename |
| txid | TEXT UNIQUE | txid | TEXT(64) nullable | Keep (nullable in SDK) |
| reference_number | TEXT UNIQUE | reference | TEXT(64) UNIQUE | Rename |
| raw_tx | TEXT | rawTx | BINARY nullable | Change encoding |
| description | TEXT | description | TEXT(2048) | Keep |
| status | TEXT (ActionStatus) | status | TEXT(64) (TransactionStatus) | **Consolidate** (see Section 3) |
| broadcast_status | TEXT | *(eliminated)* | — | **Consolidated into status** |
| is_outgoing | BOOLEAN | isOutgoing | BOOLEAN | Keep |
| satoshis | INTEGER | satoshis | BIGINT default 0 | Widen type |
| timestamp | INTEGER | created_at | TIMESTAMP | Rename + convert |
| block_height | INTEGER nullable | *(eliminated)* | — | Tracked via proven_txs.height |
| confirmations | INTEGER | *(eliminated)* | — | Remove (merkle proof replaces confirmation counting) |
| version | INTEGER | version | INTEGER nullable | Keep |
| lock_time | INTEGER | lockTime | INTEGER nullable | Rename |
| custom_instructions | TEXT | *(eliminated)* | — | Remove (on outputs now) |
| *(new)* | — | userId | INTEGER FK | Add |
| *(new)* | — | provenTxId | INTEGER FK nullable | Add: links to immutable proof |
| *(new)* | — | inputBEEF | BINARY nullable | Add: store the input BEEF |
| *(new)* | — | updated_at | TIMESTAMP | Add |

### 2.5 parent_transactions + merkle_proofs → proven_txs (merge)

| Hodos Source | wallet-toolbox Column | Migration Action |
|---|---|---|
| parent_transactions.txid | proven_txs.txid | Direct map |
| parent_transactions.raw_hex | proven_txs.rawTx | TEXT → BINARY encoding |
| merkle_proofs.block_height | proven_txs.height | Direct map |
| merkle_proofs.tx_index | proven_txs.index | Direct map |
| merkle_proofs.nodes (JSON) | proven_txs.merklePath (BINARY) | Convert JSON nodes → binary MerklePath |
| *(computed from header)* | proven_txs.blockHash | Populate from block_headers table |
| *(computed from header)* | proven_txs.merkleRoot | Populate from block_headers table |
| parent_transactions.cached_at | proven_txs.created_at | Convert |

**Key behavior**: proven_txs records are **immutable** — once created, never updated. This is enforced by `mergeExisting()` returning false.

### 2.6 (new) proven_tx_reqs

Entirely new table. No Hodos equivalent.

| Column | Type | Purpose |
|---|---|---|
| provenTxReqId | INTEGER PK AUTO | Primary key |
| provenTxId | INTEGER FK nullable | Links to proven_txs once proof acquired |
| status | TEXT(16) default 'unknown' | ProvenTxReqStatus lifecycle |
| attempts | INTEGER default 0 | Retry counter (max 8) |
| notified | BOOLEAN default false | Whether owning tx was notified |
| txid | TEXT(64) UNIQUE | Transaction being proven |
| batch | TEXT(64) nullable | Grouping key |
| history | TEXT default '{}' | JSON timestamped state transition log |
| notify | TEXT default '{}' | JSON list of transaction IDs to notify |
| rawTx | BINARY | Raw transaction bytes |
| inputBEEF | BINARY nullable | Input BEEF for re-broadcast |
| created_at, updated_at | TIMESTAMP | Timestamps |

### 2.7 baskets → output_baskets

| Hodos Column | wallet-toolbox Column | Migration Action |
|---|---|---|
| id | basketId | Rename |
| name | name | Keep (already normalized) |
| description | *(not in SDK)* | Keep as extension |
| token_type | *(not in SDK)* | Keep as extension |
| protocol_id | *(not in SDK)* | Keep as extension |
| created_at | created_at | Convert format |
| last_used | *(not in SDK)* | Keep as extension |
| *(new)* | userId | Add FK |
| *(new)* | numberOfDesiredUTXOs (default 6) | Add |
| *(new)* | minimumDesiredUTXOValue (default 10000) | Add |
| *(new)* | isDeleted (default false) | Add |
| *(new)* | updated_at | Add |

### 2.8 transaction_labels → tx_labels + tx_labels_map

**Current**: Single table with (transaction_id, label) pairs.
**Target**: Separate label entity + mapping table (like output_tags pattern).

| Current | Target |
|---|---|
| transaction_labels.label | tx_labels.label (deduplicated entity) |
| transaction_labels.transaction_id | tx_labels_map.transactionId |
| — | tx_labels.txLabelId, userId, isDeleted |
| — | tx_labels_map.txLabelId, isDeleted |

### 2.9 transaction_inputs + transaction_outputs → (eliminated)

**Current**: Separate tables storing parsed inputs and outputs per transaction.
**Target**: Stored implicitly in `transactions.rawTx` (binary). Inputs/outputs parsed on demand from the raw transaction bytes.

**Migration**: Keep old tables until Phase 8 cleanup. New code reads from rawTx instead.

### 2.10 certificates → certificates

| Hodos Column | wallet-toolbox Column | Migration Action |
|---|---|---|
| id | certificateId | Rename |
| certificate_txid | *(not in SDK)* | Keep as extension (our outpoint reference) |
| identity_key | *(derived from userId)* | Remove (replaced by userId FK) |
| type | type | Keep |
| serial_number | serialNumber | Rename |
| certifier | certifier | Keep |
| subject | subject | Keep |
| verifier | verifier | Keep |
| revocation_outpoint | revocationOutpoint | Rename |
| signature | signature | Keep |
| is_deleted | isDeleted | Rename |
| attributes | *(eliminated)* | Already migrated to certificate_fields in v7 |
| acquired_at | created_at | Rename |
| relinquished, relinquished_at | *(not in SDK)* | Keep as extension or track via isDeleted |
| *(new)* | userId | Add FK |
| *(new)* | updated_at | Add |

### 2.11 certificate_fields → certificate_fields

| Hodos Column | wallet-toolbox Column | Migration Action |
|---|---|---|
| id | *(no separate PK in SDK)* | Keep for SQLite compat |
| certificate_id | certificateId | Rename |
| field_name | fieldName | Rename |
| field_value | fieldValue | Rename |
| master_key | masterKey | Rename (default '' in SDK) |
| created_at | created_at | Convert format |
| updated_at | updated_at | Convert format |
| *(new)* | userId | Add FK |

### 2.12 output_tags → output_tags

| Hodos Column | wallet-toolbox Column | Migration Action |
|---|---|---|
| id | outputTagId | Rename |
| tag | tag | Keep |
| is_deleted | isDeleted | Rename |
| created_at | created_at | Convert |
| updated_at | updated_at | Convert |
| *(new)* | userId | Add FK |

### 2.13 output_tag_map → output_tags_map

| Hodos Column | wallet-toolbox Column | Migration Action |
|---|---|---|
| id | *(no separate PK in SDK)* | Keep for SQLite compat |
| output_id | outputId | Rename |
| output_tag_id | outputTagId | Rename |
| is_deleted | isDeleted | Rename |
| created_at | created_at | Convert |
| updated_at | updated_at | Convert |

### 2.14 Tables to Keep (Browser-Specific)

These tables have no wallet-toolbox equivalent and are kept as-is:

| Table | Purpose | Changes |
|---|---|---|
| domain_whitelist | BRC-100 domain permissions | None |
| messages | BRC-33 message inbox | None |
| relay_messages | BRC-33 message relay | None |
| derived_key_cache | PushDrop signing cache | Eventually replaced by per-output derivation (Phase 7) |
| block_headers | Cached block headers | Eventually managed by chaintracker service |

### 2.15 New Tables Required

| Table | Phase | Purpose |
|---|---|---|
| proven_tx_reqs | Phase 2 | Proof acquisition lifecycle tracking |
| users | Phase 3 | Multi-user identity management |
| commissions | Phase 5 | Fee tracking per transaction |
| monitor_events | Phase 6 | Background task event logging |
| settings | Phase 5 | Persistent configuration (chain, dbtype, limits) |
| sync_states | Phase 5 | Multi-device synchronization state |

---

## 3. Status System Transition

### Current: Two Overlapping Systems

```
ActionStatus enum (status column)        broadcast_status column
─────────────────────────────────        ──────────────────────
Created    → tx created, not signed      pending   → never broadcast
Signed     → signed, not broadcast       broadcast → sent to network
Unconfirmed → broadcast, no confirms     confirmed → mined
Pending    → 1-5 confirmations           failed    → rejected
Confirmed  → 6+ confirmations
Aborted    → cancelled
Failed     → broadcast failed
```

### Target: Single TransactionStatus

```
TransactionStatus (single status column)
────────────────────────────────────────
unprocessed → created, no processing started
unsigned    → created, awaiting signatures (two-phase)
sending     → actively being broadcast
unproven    → broadcast, accepted, no merkle proof yet
completed   → has merkle proof (proven)
nosend      → intentionally not broadcast (local-only)
nonfinal    → time-locked, not yet final
failed      → broadcast failed or rejected
unfail      → previously failed, being re-verified
```

### Migration Mapping

| ActionStatus | broadcast_status | → TransactionStatus |
|---|---|---|
| Created | pending | unprocessed |
| Signed | pending | unsigned |
| Signed | broadcast | sending |
| Unconfirmed | broadcast | unproven |
| Pending | broadcast | unproven |
| Confirmed | confirmed | completed |
| Aborted | pending | failed |
| Aborted | broadcast | failed |
| Failed | failed | failed |
| *(any)* | *(any inconsistent)* | Use broadcast_status as primary signal |

### Consolidation Rules

For existing data where the two fields may be inconsistent, `broadcast_status` takes priority because it's what the safety-critical queries (UTXO selection, balance) actually filter on:

1. If `broadcast_status = 'confirmed'` → `completed` (regardless of ActionStatus)
2. If `broadcast_status = 'failed'` → `failed`
3. If `broadcast_status = 'broadcast'` → `unproven`
4. If `broadcast_status = 'pending'` and `status = 'Created'` → `unprocessed`
5. If `broadcast_status = 'pending'` and `status = 'Signed'` → `unsigned`
6. If `broadcast_status = 'pending'` and `status = 'Aborted'` → `failed`

### Impact on Queries

Every query that currently filters on `broadcast_status` must be updated:

| Current Query Pattern | New Query Pattern |
|---|---|
| `WHERE broadcast_status = 'broadcast'` | `WHERE status IN ('sending', 'unproven')` |
| `WHERE broadcast_status = 'confirmed'` | `WHERE status = 'completed'` |
| `WHERE broadcast_status = 'failed'` | `WHERE status = 'failed'` |
| `WHERE broadcast_status NOT IN ('pending', 'failed')` | `WHERE status IN ('completed', 'unproven', 'sending')` |
| `WHERE broadcast_status = 'pending' AND age > X` | `WHERE status IN ('unprocessed', 'unsigned') AND age > X` |

### UTXO Spendability Filter (Critical)

Current (`utxo_repo.rs` get_unspent_by_addresses):
```sql
LEFT JOIN transactions t ON u.txid = t.txid
WHERE u.is_spent = 0
  AND (t.broadcast_status IS NULL OR t.broadcast_status NOT IN ('pending', 'failed'))
```

Target:
```sql
LEFT JOIN transactions t ON o.transactionId = t.transactionId
WHERE o.spendable = 1
  AND t.status IN ('completed', 'unproven', 'sending')
```

---

## 4. Phased Implementation Plan

### Phase 1: Status Consolidation + UnFail Foundation ✅ COMPLETE

**Goal**: Replace the dual status system with single TransactionStatus. Add the UnFail delay mechanism.

**Why first**: This is the foundation for everything. It fixes the most dangerous bug (premature failure cleanup) and simplifies the status model that all subsequent phases depend on.

**Completed**: 2026-02-03 | **Migration**: V15 | **Branch**: wallet-toolbox-alignment

#### Implementation Notes

- V15 migration adds `new_status` and `failed_at` columns to transactions table
- `update_broadcast_status()` kept as compatibility shim — internally maps old status names to new `new_status` values (e.g., `"confirmed"` → `"completed"`, `"broadcast"` → `"unproven"`)
- Old `status` and `broadcast_status` columns preserved for rollback safety
- ARC poller queries `WHERE new_status IN ('sending', 'unproven')`
- Balance/UTXO queries filter on `new_status` instead of `broadcast_status`
- UnFail delay deferred to Phase 6 (Monitor pattern) for proper implementation

#### Schema Changes (Migration v15)

```sql
-- Step 1: Add new consolidated status column with SDK-compatible values
ALTER TABLE transactions ADD COLUMN new_status TEXT DEFAULT 'unprocessed';

-- Step 2: Migrate data using broadcast_status as primary signal
UPDATE transactions SET new_status = 'completed' WHERE broadcast_status = 'confirmed';
UPDATE transactions SET new_status = 'failed' WHERE broadcast_status = 'failed';
UPDATE transactions SET new_status = 'unproven' WHERE broadcast_status = 'broadcast';
UPDATE transactions SET new_status = 'unprocessed' WHERE broadcast_status = 'pending' AND status = 'created';
UPDATE transactions SET new_status = 'unsigned' WHERE broadcast_status = 'pending' AND status = 'signed';
UPDATE transactions SET new_status = 'failed' WHERE broadcast_status = 'pending' AND status IN ('aborted', 'failed');
-- Catch-all for any remaining
UPDATE transactions SET new_status = 'unprocessed' WHERE new_status = 'unprocessed' AND broadcast_status = 'pending';

-- Step 3: Add failed_at timestamp for UnFail mechanism
ALTER TABLE transactions ADD COLUMN failed_at INTEGER;
-- Backfill: set failed_at = timestamp for existing failed transactions
UPDATE transactions SET failed_at = timestamp WHERE new_status = 'failed';

-- Step 4: Create index on new_status
CREATE INDEX idx_transactions_new_status ON transactions(new_status);
```

**Note**: Old `status` and `broadcast_status` columns are kept during this phase for safety. They are ignored by new code but available for rollback.

#### Rust Code Changes

| File | Change |
|---|---|
| `src/action_storage.rs` | Added `TransactionStatus` enum matching SDK values alongside existing `ActionStatus` |
| `src/database/transaction_repo.rs` | All methods: read/write `new_status` instead of `status`+`broadcast_status`. `update_broadcast_status()` kept as shim mapping to `new_status` values |
| `src/database/utxo_repo.rs` | Updated `get_unspent_by_addresses()` and `calculate_balance()` LEFT JOIN to filter on `new_status` instead of `broadcast_status` |
| `src/handlers.rs` | All status transitions: use single `new_status` update. Map: createAction→`unprocessed`, signAction→`unsigned`→`sending`, broadcast success→`unproven`, broadcast fail→`failed` |
| `src/arc_status_poller.rs` | Query `WHERE new_status IN ('sending', 'unproven')`. Update to `completed` or `failed` |
| `src/main.rs` | Startup cleanup: query `new_status` instead of `broadcast_status` |

#### Testing Results

- Fresh DB: V15 migration creates `new_status` column, `failed_at` column, and index
- Existing DB: All 311 transactions correctly migrated (298 failed, 13 unproven)
- Balance unchanged at 644,544 sats after migration
- All status transitions verified through BEEF transaction testing

#### Risk: LOW-MEDIUM
- Migration is additive (new column, old columns preserved)
- Rollback: revert code to read old columns
- Testing: verify balance calculation matches before/after

---

### Phase 2: Proven Transaction Model ✅ COMPLETE

**Goal**: Add `proven_txs` and `proven_tx_reqs` tables. Merge existing parent_transactions + merkle_proofs into proven_txs. Establish immutable proof records.

**Why second**: The proven transaction model is the foundation for reliable proof tracking, UnFail recovery (re-checking proofs), and the monitor pattern's TaskCheckForProofs.

**Completed**: 2026-02-04 | **Migration**: V16 | **Branch**: wallet-toolbox-alignment

#### Implementation Notes

- V16 migration creates `proven_txs` and `proven_tx_reqs` tables, adds `proven_tx_id` FK to `transactions`
- Data migration converts existing `merkle_proofs` + `parent_transactions` into `proven_txs` records
  - TSC JSON reconstructed from merkle_proofs fields (block_height, tx_index, target_hash, nodes)
  - Serialized to bytes for merkle_path BLOB storage
  - Raw tx hex decoded to bytes for raw_tx BLOB storage
- BEEF construction reads proofs from `proven_txs` instead of `merkle_proofs`
- `proven_tx_reqs` created on every broadcast in `sign_action()` with status `'sending'`
- ARC poller creates `proven_txs` records when transactions reach MINED status
- Cache sync creates `proven_txs` records from WhatsOnChain TSC proofs
- Old `parent_transactions` table preserved as raw tx cache for BEEF building
- Old `merkle_proofs` table preserved but no longer written to

#### Bug Found & Fixed During Testing

**cache_sync / ARC poller race condition**: When `cache_sync` created a `proven_txs` record before the ARC poller ran, the transaction's `new_status` remained `'unproven'` and `proven_tx_reqs.status` remained `'sending'` because:
1. `cache_sync` wasn't updating transaction status or `proven_tx_reqs` after proof creation
2. ARC poller got 404 from ARC API (transaction not tracked by ARC) and silently skipped

**Fix applied to both services**:
- `cache_sync.rs`: Now updates `new_status` to `'completed'` and `proven_tx_reqs` to `'completed'` after creating `proven_txs` record
- `arc_status_poller.rs`: Added early reconciliation check — before querying ARC, checks if `proven_txs` record already exists and reconciles statuses directly

#### Schema Changes (Migration v16)

```sql
-- Create proven_txs table (immutable proof records)
CREATE TABLE IF NOT EXISTS proven_txs (
    provenTxId INTEGER PRIMARY KEY AUTOINCREMENT,
    txid TEXT NOT NULL UNIQUE,
    height INTEGER NOT NULL,
    tx_index INTEGER NOT NULL,
    merkle_path BLOB NOT NULL,
    raw_tx BLOB NOT NULL,
    block_hash TEXT NOT NULL DEFAULT '',
    merkle_root TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_proven_txs_txid ON proven_txs(txid);

-- Create proven_tx_reqs table (mutable proof request tracking)
CREATE TABLE IF NOT EXISTS proven_tx_reqs (
    provenTxReqId INTEGER PRIMARY KEY AUTOINCREMENT,
    proven_tx_id INTEGER REFERENCES proven_txs(provenTxId),
    status TEXT NOT NULL DEFAULT 'unknown',
    attempts INTEGER NOT NULL DEFAULT 0,
    notified INTEGER NOT NULL DEFAULT 0,
    txid TEXT NOT NULL UNIQUE,
    batch TEXT,
    history TEXT NOT NULL DEFAULT '{}',
    notify TEXT NOT NULL DEFAULT '{}',
    raw_tx BLOB NOT NULL,
    input_beef BLOB,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_proven_tx_reqs_status ON proven_tx_reqs(status);
CREATE INDEX IF NOT EXISTS idx_proven_tx_reqs_txid ON proven_tx_reqs(txid);

-- Add provenTxId FK to transactions table
ALTER TABLE transactions ADD COLUMN proven_tx_id INTEGER REFERENCES proven_txs(provenTxId);

-- Data migration from merkle_proofs + parent_transactions → proven_txs (done in Rust)
-- Link existing transactions to proven_txs records
```

#### Rust Code Changes

| File | Change |
|---|---|
| `src/database/models.rs` | Added `ProvenTx` and `ProvenTxReq` model structs |
| `src/database/proven_tx_repo.rs` | **New file**: `insert_or_get()` (immutable insert-or-return-existing), `get_by_txid()`, `get_by_id()`, `get_merkle_proof_as_tsc()`, `link_transaction()` |
| `src/database/proven_tx_req_repo.rs` | **New file**: `create()`, `get_by_txid()`, `update_status()`, `increment_attempts()`, `get_pending()`, `link_proven_tx()`, `add_history_note()` |
| `src/database/mod.rs` | Export new modules and types |
| `src/database/migrations.rs` | Added `create_schema_v16()` with data migration from merkle_proofs |
| `src/database/connection.rs` | Added V16 migration runner |
| `src/action_storage.rs` | Added `ProvenTxReqStatus` enum |
| `src/arc_status_poller.rs` | Added `create_proven_tx_from_arc()`, early reconciliation check for existing proofs |
| `src/cache_sync.rs` | Write to `proven_txs` instead of `merkle_proofs`, update tx/req status after proof creation |
| `src/beef_helpers.rs` | Read proofs from `proven_txs` via `get_merkle_proof_as_tsc()` instead of `merkle_proofs` |
| `src/handlers.rs` | Rewrote `cache_arc_merkle_proof()` to create `proven_txs` records, updated `sign_action()` proof reads, create `proven_tx_req` on broadcast |

#### Testing Results

- **Fresh DB**: All tables created with correct columns, indexes, and auto-indexes
- **V15→V16 Migration**: 22 proof records migrated to `proven_txs` (matched `merkle_proofs` count exactly). Balance unchanged at 644,544 sats
- **Data Reconciliation**: 12 transactions had proofs but weren't marked `completed` (pre-existing data inconsistency from before Phase 1). Reconciled via manual UPDATE
- **Runtime**: `proven_tx_req` created with status `'sending'` on broadcast. After mining, `cache_sync` created `proven_txs` record and updated all statuses to `'completed'`
- **BEEF Construction**: Multiple transaction types tested successfully:
  - BRC-29 payment transactions with `proven_txs` merkle proofs
  - TODO app create/complete with inputBEEF and ancestry chains
  - Two-phase signing (createAction → signAction)
  - All showed `✅ Using proven_txs Merkle proof` in logs

#### Risk: LOW
- Purely additive (new tables, new FK column)
- Old parent_transactions and merkle_proofs tables untouched
- Can run both old and new code paths in parallel during transition

---

### Phase 3: Multi-User Foundation

**Goal**: Add `users` table and `userId` foreign keys to core tables. Create a default user for the existing wallet.

**Why third**: Phases 4-7 need userId on tables they modify. Adding it now means we don't have to retroactively add it later.

#### Schema Changes (Migration v17)

```sql
-- Create users table
CREATE TABLE IF NOT EXISTS users (
    userId INTEGER PRIMARY KEY AUTOINCREMENT,
    identity_key TEXT NOT NULL UNIQUE,
    active_storage TEXT NOT NULL DEFAULT 'local',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Create default user from existing wallet's master public key
-- (done in Rust migration code, not pure SQL, because we need to derive the identity key)

-- Add userId to existing tables
ALTER TABLE transactions ADD COLUMN user_id INTEGER REFERENCES users(userId);
ALTER TABLE baskets ADD COLUMN user_id INTEGER REFERENCES users(userId);
ALTER TABLE output_tags ADD COLUMN user_id INTEGER REFERENCES users(userId);
ALTER TABLE certificates ADD COLUMN user_id INTEGER REFERENCES users(userId);
ALTER TABLE certificate_fields ADD COLUMN user_id INTEGER REFERENCES users(userId);

-- Backfill userId for all existing data (set to default user's userId)
-- (done in Rust migration code after creating default user)
```

#### Rust Code Changes

| File | Change |
|---|---|
| `src/database/models.rs` | Add `User` model struct |
| `src/database/user_repo.rs` | **New file**: `create()`, `get_by_identity_key()`, `get_default()` |
| `src/database/migrations.rs` | v17: create users table, derive identity key from master public key, create default user, add userId columns, backfill |
| `src/main.rs` | AppState: add current_user_id field |
| All repos | Add userId parameter where needed (backward-compatible: default to current user) |

#### Rust Code Changes (Implemented)

| File | Change |
|---|---|
| `src/database/models.rs` | Added `User` model struct with fields: `user_id`, `identity_key`, `active_storage`, `created_at`, `updated_at` |
| `src/database/user_repo.rs` | **New file**: `UserRepository` with `create()`, `get_by_id()`, `get_by_identity_key()`, `get_default()`, `update_active_storage()` |
| `src/database/migrations.rs` | Added `create_schema_v17()`: creates users table, derives identity_key from mnemonic, creates default user, adds user_id columns to 5 tables, backfills existing data, creates indexes |
| `src/database/connection.rs` | Added V17 migration runner |
| `src/database/mod.rs` | Export `user_repo` module, `User` model, and `UserRepository` |
| `src/main.rs` | Added `current_user_id: i64` to `AppState`, initialized from default user on startup |

#### Testing Results

- [x] **Fresh DB**: Users table created, default user created when wallet is created
- [x] **V16→V17 Migration**: user_id columns added to transactions/baskets/output_tags/certificates/certificate_fields, existing data backfilled with user_id=1
- [x] **Runtime**: `AppState.current_user_id` populated correctly on startup (logs show `👤 Default user ID: 1`)

#### Risk: LOW
- Additive only
- All existing data gets the single default user's ID
- No behavioral changes — just plumbing for multi-user

---

### Phase 4: Output Model Transition

**Goal**: Restructure `utxos` table to match wallet-toolbox `outputs` table. Replace `is_spent`/`spent_txid` with `spendable`/`spentBy`. Add per-output metadata fields.

**Why fourth**: This is the largest single change. It requires the status consolidation (Phase 1), proven_txs (Phase 2), and userId (Phase 3) to be in place.

#### Schema Changes (Migration v18)

```sql
-- Create new outputs table matching wallet-toolbox schema
CREATE TABLE IF NOT EXISTS outputs (
    outputId INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(userId),
    transaction_id INTEGER REFERENCES transactions(id),
    basket_id INTEGER REFERENCES baskets(id),
    spendable INTEGER NOT NULL DEFAULT 0,
    change INTEGER NOT NULL DEFAULT 0,
    vout INTEGER NOT NULL,
    satoshis INTEGER NOT NULL,
    provided_by TEXT NOT NULL DEFAULT 'you',
    purpose TEXT NOT NULL DEFAULT '',
    type TEXT NOT NULL DEFAULT '',
    output_description TEXT,
    txid TEXT,
    sender_identity_key TEXT,
    derivation_prefix TEXT,
    derivation_suffix TEXT,
    custom_instructions TEXT,
    spent_by INTEGER REFERENCES transactions(id),
    sequence_number INTEGER,
    spending_description TEXT,
    script_length INTEGER,
    script_offset INTEGER,
    locking_script BLOB,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX idx_outputs_spendable ON outputs(spendable);
CREATE UNIQUE INDEX idx_outputs_tx_vout_user ON outputs(transaction_id, vout, user_id);
CREATE INDEX idx_outputs_basket ON outputs(basket_id);
CREATE INDEX idx_outputs_txid ON outputs(txid);

-- Migrate data from utxos → outputs
INSERT INTO outputs (
    user_id, transaction_id, basket_id, spendable, change, vout, satoshis,
    provided_by, purpose, type, output_description, txid,
    custom_instructions, locking_script, created_at, updated_at
)
SELECT
    (SELECT userId FROM users LIMIT 1),       -- default user
    t.id,                                       -- link to transaction if exists
    u.basket_id,
    CASE
        WHEN u.is_spent = 1 THEN 0             -- spent = not spendable
        WHEN u.status = 'failed' THEN 0        -- failed = not spendable
        ELSE 1                                   -- unspent + not failed = spendable
    END,
    0,                                           -- change flag (unknown for legacy data)
    u.vout,
    u.satoshis,
    'you',
    '',
    '',
    u.output_description,
    u.txid,
    u.custom_instructions,
    CAST(u.script AS BLOB),
    u.first_seen,
    u.last_updated
FROM utxos u
LEFT JOIN transactions t ON t.txid = u.txid;

-- Set spentBy for spent outputs
UPDATE outputs SET spent_by = (
    SELECT t.id FROM transactions t
    INNER JOIN utxos u ON u.spent_txid = t.txid
    WHERE u.txid = outputs.txid AND u.vout = outputs.vout
) WHERE spendable = 0;

-- Populate derivation_prefix/suffix from address data where possible
-- (done in Rust migration code using address index → derivation params)
```

#### Rust Code Changes

| File | Change |
|---|---|
| `src/database/utxo_repo.rs` → `src/database/output_repo.rs` | **Rewrite**: new `OutputRepository` with methods matching wallet-toolbox patterns: `get_spendable_by_basket()`, `allocate_change_input()` (3-tier selection in SQLite transaction), `mark_spent_by()`, `restore_spendable()`, `calculate_balance()` |
| `src/database/models.rs` | Replace `Utxo` struct with `Output` struct matching new columns |
| `src/handlers.rs` | createAction: use `allocate_change_input()` instead of `get_unspent_by_addresses()` + `mark_spent()`. signAction: update `spentBy` instead of `spent_txid`. Remove `pending-{timestamp}` placeholder system — use `spendable=false, spentBy=transactionId` instead |
| `src/utxo_sync.rs` | Rewrite sync to work with outputs table. Update cleanup to use `spendable`/`spentBy` instead of `is_spent`/`spent_txid` |
| `src/balance_cache.rs` | Update query source |
| `src/main.rs` | Remove `utxo_selection_lock` (dead code) |

#### UTXO Reservation Change

```
CURRENT                                    TARGET
pending-{timestamp} placeholder            spendable=false, spentBy=transactionId

createAction:                              createAction:
  mark_spent(txid, 'pending-1706900000')     allocate_change_input(userId, basketId,
                                               targetSats, transactionId)
                                             → atomically sets spendable=false,
                                               spentBy=transactionId in DB transaction

signAction:                                signAction:
  update spent_txid from pending→real        (already correct — spentBy points to tx)

cleanup stale:                             cleanup stale:
  WHERE spent_txid LIKE 'pending-%'          WHERE spendable=0 AND spentBy IN (
    AND spent_at < now - 300                   SELECT transactionId FROM transactions
                                               WHERE status IN ('unprocessed','unsigned')
                                               AND created_at < now - 300
                                             )
  → restore is_spent=0, spent_txid=NULL      → restore spendable=1, spentBy=NULL
```

#### Risk: HIGH
- Most impactful structural change
- Affects createAction, signAction, balance, UTXO selection — all critical paths
- Mitigation: keep old `utxos` table during transition, run both old and new code paths with comparison logging, cut over only when confident

#### Phase 4A Implementation (Schema + Data Migration)

**Status: COMPLETE**

| File | Change |
|---|---|
| `src/database/models.rs` | Added `Output` struct with all wallet-toolbox columns |
| `src/database/migrations.rs` | Added `create_schema_v18()`: creates outputs table, migrates data, populates derivation_prefix/suffix, verifies migration |
| `src/database/connection.rs` | Added V18 migration runner |
| `src/database/mod.rs` | Exported `Output` model |

**Testing Queries:**
```sql
-- Verify row counts match
SELECT 'utxos' as tbl, COUNT(*) as cnt FROM utxos
UNION ALL
SELECT 'outputs', COUNT(*) FROM outputs;

-- Verify spendable counts match
SELECT 'unspent_utxos' as metric, COUNT(*) as cnt FROM utxos WHERE is_spent = 0
UNION ALL
SELECT 'spendable_outputs', COUNT(*) FROM outputs WHERE spendable = 1;

-- Verify balance matches
SELECT 'utxo_balance' as metric, COALESCE(SUM(satoshis), 0) as sats FROM utxos WHERE is_spent = 0
UNION ALL
SELECT 'output_balance', COALESCE(SUM(satoshis), 0) FROM outputs WHERE spendable = 1;

-- Check derivation info populated
SELECT derivation_prefix, derivation_suffix, COUNT(*) as cnt
FROM outputs
GROUP BY derivation_prefix, derivation_suffix;

-- Verify spent_by FK populated for spent outputs
SELECT COUNT(*) as spent_with_fk FROM outputs WHERE spendable = 0 AND spent_by IS NOT NULL;
```

#### Phase 4B Implementation (Read Path + Comparison Logging)

**Status: COMPLETE**

| File | Change |
|---|---|
| `src/database/output_repo.rs` | Created `OutputRepository` with read methods: `get_by_id()`, `get_by_txid_vout()`, `get_spendable_by_user()`, `get_spendable_by_basket()`, `get_spendable_by_basket_with_tags()`, `calculate_balance()`, `calculate_total_balance()`, `count_spendable()`, `get_locking_script_hex()` |
| `src/database/mod.rs` | Exported `output_repo`, `OutputRepository` |
| `src/handlers.rs` | Added comparison logging at all balance calculation and UTXO selection points |

**Comparison Logging Added:**
- `get_balance` handler (line ~1810): Compares `utxo_repo.calculate_balance()` vs `output_repo.calculate_balance()`
- `get_balance` updated balance path (line ~2000): Same comparison after API fetch
- `createAction` UTXO selection (line ~3340): Compares count and balance from both tables
- `createAction` re-read under lock (line ~3435): Same comparison after re-read

**Log Markers:**
- `⚠️  Phase 4B DISCREPANCY` — logged at WARN level when values differ
- `✓ Phase 4B:` — logged at DEBUG level when values match

#### Phase 4C Implementation (Dual-Write to Both Tables)

**Status: COMPLETE**

| File | Change |
|---|---|
| `src/database/output_repo.rs` | Added write methods: `insert_output()`, `update_txid()`, `update_txid_batch()`, `mark_spent()`, `mark_multiple_spent()`, `delete_by_txid()`, `restore_by_spending_description()`, `update_spending_description_batch()`, `restore_pending_placeholders()` |
| `src/handlers.rs` | Added dual-write calls at all UTXO write locations (28 total) |

**Write Locations Updated (createAction):**
- Line ~3491: Wallet UTXO reservation (`mark_multiple_spent`)
- Line ~3532: User-provided input reservation (`mark_multiple_spent`)
- Line ~4155: Change output insert (`insert_output`)
- Line ~4234: Basket output insert (`insert_output`)
- Line ~4527: Change output txid update after signing
- Line ~4542: Basket output txid updates after signing
- Line ~4556, ~4572: Reserved input spent_txid update (placeholder → real txid)
- Line ~4632-4662: Broadcast failure cleanup (delete + restore)

**Write Locations Updated (signAction):**
- Line ~6233: Change UTXO txid update (unsigned → signed)
- Line ~6284: Reserved UTXO spent_txid update
- Line ~6302: Fallback mark_multiple_spent
- Line ~6526: External spend detection
- Line ~6588: Broadcast failure cleanup

**Write Locations Updated (Other Handlers):**
- Line ~7715-7723: Send transaction failure cleanup
- Line ~8925: Internalize action basket insertion
- Line ~10229: Ghost UTXO cleanup

**Key Mappings:**
- `utxo_repo.insert_output_with_basket()` → `output_repo.insert_output()`
- `utxo_repo.mark_spent()` → `output_repo.mark_spent()`
- `utxo_repo.mark_multiple_spent()` → `output_repo.mark_multiple_spent()`
- `utxo_repo.update_utxo_txid()` → `output_repo.update_txid()`
- `utxo_repo.update_txid()` → `output_repo.update_txid_batch()`
- `utxo_repo.update_spent_txid_batch()` → `output_repo.update_spending_description_batch()`
- `utxo_repo.delete_by_txid()` → `output_repo.delete_by_txid()`
- `utxo_repo.restore_spent_by_txid()` → `output_repo.restore_by_spending_description()`

**Testing:** ✅ VERIFIED 2026-02-06

Tested dual-write with transaction create/sign/broadcast cycle:
- `✅ Phase 4C: Inserted output` - Change output inserted into both tables
- `✅ Phase 4C: Updated 1 output(s) txid` - Txid reconciliation worked
- `🗑️ Phase 4C: Deleted 1 output(s)` - Cleanup on broadcast failure worked

Both tables now stay in sync for new transactions. Historical discrepancy exists for pre-Phase 4C data but will resolve as old UTXOs are spent.

**Note:** During testing, discovered 17 stale 'unproven' transactions forming broken chains. These were cleaned up with:
```sql
UPDATE transactions
SET new_status = 'failed', broadcast_status = 'failed', failed_at = strftime('%s', 'now')
WHERE new_status = 'unproven';
```
**TODO (post-Phase 4):** Add staleness detection to auto-mark old unproven transactions as failed.

#### Phase 4D: Cutover to Outputs Table

**Status: IN PROGRESS** (2026-02-05)

**Goal:** Switch read paths from `utxos` to `outputs` as the source of truth.

**Changes Required:**

| Location | Current (utxos) | New (outputs) | Status |
|----------|-----------------|---------------|--------|
| `get_balance` handler | `utxo_repo.calculate_balance(address_ids)` | `output_repo.calculate_balance(user_id)` | ✅ Done |
| `createAction` UTXO selection | `utxo_repo.get_unspent_by_addresses(address_ids)` | `output_repo.get_spendable_by_user(user_id)` | ✅ Done |
| `createAction` re-read under lock | `utxo_repo.get_unspent_by_addresses(address_ids)` | `output_repo.get_spendable_by_user(user_id)` | ✅ Done |
| `listOutputs` handler | `utxo_repo.get_unspent_by_basket()` | `output_repo.get_spendable_by_basket()` | ✅ Done |
| Basket/tag queries | `utxo_repo.get_unspent_by_basket_with_tags()` | `output_repo.get_spendable_by_basket_with_tags()` | ✅ Done |
| `relinquishOutput` handler | `utxo_repo.get_by_txid_vout()` + `remove_from_basket()` | `output_repo.get_by_txid_vout()` + `remove_from_basket()` | ✅ Done |

**Key Differences:**
- Uses `user_id` instead of `address_ids[]` for filtering
- Returns `Output` structs instead of `Utxo` structs
- Added `output_to_fetcher_utxo()` adapter in `database/helpers.rs` to convert `Output` → `UTXO` format for signing code
- Added `remove_from_basket()` method to `OutputRepository`

**Verification:**
- Phase 4D comparison logging: outputs is primary, utxos is secondary
- Fallback to legacy utxos table if outputs query fails
- Logs warnings when outputs and utxos counts/balances differ

**Implementation Notes (2026-02-05):**
1. `output_to_fetcher_utxo()` adapter function handles derivation_prefix/suffix → address_index mapping:
   - `"2-receive address"` prefix with numeric suffix → positive index
   - NULL derivation → -1 (master pubkey)
   - Other prefixes → -2 (BRC-29 custom derivation)
2. Locking script conversion: `Output.locking_script` (bytes) → hex string via `hex::encode()`
3. Dual-write maintained: changes to outputs also written to utxos for Phase 4E validation

#### Phase 4E: Cleanup

**Status: COMPLETE** (2026-02-06)

**Goal:** Remove deprecated utxos code after cutover is stable.

**Changes Made:**

| File | Change |
|------|--------|
| `src/database/utxo_repo.rs` | **Deleted** - entire file removed |
| `src/database/models.rs` | Removed `Utxo` struct, updated `Output` documentation |
| `src/database/mod.rs` | Removed `utxo_repo` module and `Utxo` export (already done in 4D) |
| `src/database/helpers.rs` | Removed `utxo_to_fetcher_utxo()` function |
| `src/database/output_repo.rs` | Removed Phase 4C/4D comments, added `get_all_by_user()` for backup |
| `src/handlers.rs` | Removed all `UtxoRepository` usages (internalizeAction, relinquishOutput, ghost cleanup), removed dead `filter_utxos_by_tags()` function |
| `src/main.rs` | Removed dual-write cleanup, consolidated to `OutputRepository` only |
| `src/utxo_sync.rs` | Removed `UtxoRepository`, switched to `OutputRepository` for all sync operations |
| `src/backup.rs` | Changed from `UtxoRepository` to `OutputRepository` for export |
| `src/handlers/certificate_handlers.rs` | Removed all `UtxoRepository` usages |

**Result:** The `outputs` table is now the sole source of truth. All deprecated `utxos` table code has been removed. Build passes with only warnings.

---

### Phase 5: Labels, Commissions, Supporting Tables ✅ COMPLETE

**Goal**: Restructure transaction_labels to tx_labels + tx_labels_map pattern. Add commissions, settings, sync_states tables.

**Completed**: 2026-02-07 | **Migration**: V19 | **Branch**: wallet-toolbox-alignment

#### Schema Changes (Migration v19)

```sql
-- Create tx_labels entity table
CREATE TABLE IF NOT EXISTS tx_labels (
    txLabelId INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(userId),
    label TEXT NOT NULL,
    is_deleted INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(label, user_id)
);

-- Create tx_labels_map junction table
CREATE TABLE IF NOT EXISTS tx_labels_map (
    txLabelId INTEGER NOT NULL REFERENCES tx_labels(txLabelId),
    transaction_id INTEGER NOT NULL REFERENCES transactions(id),
    is_deleted INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(txLabelId, transaction_id)
);
CREATE INDEX idx_tx_labels_map_tx ON tx_labels_map(transaction_id);

-- Migrate existing transaction_labels data
INSERT OR IGNORE INTO tx_labels (user_id, label, is_deleted, created_at, updated_at)
SELECT DISTINCT (SELECT userId FROM users LIMIT 1), label, 0,
    strftime('%s','now'), strftime('%s','now')
FROM transaction_labels;

INSERT OR IGNORE INTO tx_labels_map (txLabelId, transaction_id, is_deleted, created_at, updated_at)
SELECT tl.txLabelId, tla.transaction_id, 0, strftime('%s','now'), strftime('%s','now')
FROM transaction_labels tla
INNER JOIN tx_labels tl ON tl.label = tla.label;

-- Create commissions table
CREATE TABLE IF NOT EXISTS commissions (
    commissionId INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(userId),
    transaction_id INTEGER NOT NULL UNIQUE REFERENCES transactions(id),
    satoshis INTEGER NOT NULL,
    key_offset TEXT NOT NULL,
    is_redeemed INTEGER NOT NULL DEFAULT 0,
    locking_script BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Create settings table
CREATE TABLE IF NOT EXISTS settings (
    storage_identity_key TEXT NOT NULL,
    storage_name TEXT NOT NULL,
    chain TEXT NOT NULL DEFAULT 'main',
    dbtype TEXT NOT NULL DEFAULT 'sqlite',
    max_output_script INTEGER NOT NULL DEFAULT 500000,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Create sync_states table
CREATE TABLE IF NOT EXISTS sync_states (
    syncStateId INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(userId),
    storage_identity_key TEXT NOT NULL DEFAULT '',
    storage_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'unknown',
    init INTEGER NOT NULL DEFAULT 0,
    ref_num TEXT NOT NULL UNIQUE,
    sync_map TEXT NOT NULL,
    sync_when INTEGER,
    satoshis INTEGER,
    error_local TEXT,
    error_other TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX idx_sync_states_status ON sync_states(status);
```

#### Rust Code Changes (Implemented)

| File | Change |
|---|---|
| `src/database/models.rs` | Added `TxLabel`, `TxLabelMap`, `Commission`, `Setting`, `SyncState` model structs |
| `src/database/migrations.rs` | Added `create_schema_v19()`: creates tx_labels, tx_labels_map, commissions, settings, sync_states tables; migrates transaction_labels data |
| `src/database/connection.rs` | Added V19 migration runner |
| `src/database/tx_label_repo.rs` | **New file**: `TxLabelRepository` with `find_or_insert()`, `get_labels_for_transaction()`, `assign_label_to_transaction()`, `remove_label_from_transaction()`, `get_all_labels()`, `delete_label()` |
| `src/database/commission_repo.rs` | **New file**: `CommissionRepository` with `create()`, `get_by_id()`, `get_by_transaction_id()`, `get_unredeemed()`, `mark_redeemed()`, `get_total_unredeemed()`, `delete_by_transaction_id()` |
| `src/database/settings_repo.rs` | **New file**: `SettingsRepository` with `get()`, `upsert()`, `ensure_defaults()`, `get_chain()`, `set_chain()`, `get_max_output_script()`, `set_storage()` |
| `src/database/sync_state_repo.rs` | **New file**: `SyncStateRepository` with `create()`, `get_by_id()`, `get_by_ref_num()`, `get_by_user()`, `get_pending()`, `update_status()`, `update_sync_map()`, `mark_synced()`, `mark_error()`, `mark_init_complete()`, `cleanup_old()` |
| `src/database/mod.rs` | Export new modules and model types |
| `src/database/tag_repo.rs` | Updated `get_labels_for_transaction()` to use new tables with fallback to old `transaction_labels` |
| `src/database/transaction_repo.rs` | Updated label read code in `get_by_txid()` to use new tables with fallback |

#### Implementation Notes

- V19 migration creates 5 new tables and migrates existing `transaction_labels` data
- Data migration normalizes labels (trim + lowercase) during copy
- Label reads use new `tx_labels` + `tx_labels_map` tables with fallback to old `transaction_labels`
- Label writes still go to old `transaction_labels` table for now (can be updated in follow-up)
- New tables (`commissions`, `settings`, `sync_states`) are empty — ready for future use
- Old `transaction_labels` table preserved for rollback safety

#### Testing (TODO for User)

- [ ] **Fresh DB**: V19 migration creates all new tables with correct columns and indexes
- [ ] **V18→V19 Migration**: Label data correctly migrated to tx_labels + tx_labels_map
- [ ] **Runtime**: Label reads from new tables, fallback to old if empty

#### Risk: LOW
- Mostly additive
- Label migration is straightforward
- Commissions, settings, sync_states are new tables with no existing data

---

### Phase 6: Monitor Pattern (Background Services)

**Goal**: Restructure background services to match the wallet-toolbox monitor pattern with named tasks, configurable intervals, and event logging. Add on-demand UTXO sync endpoint. Improve broadcast reliability with transient/permanent error classification and retry logic.

**Status**: ✅ COMPLETE (2026-02-07) | **Migrations**: V20-V22 | **Branch**: wallet-toolbox-alignment

#### Architecture Change

```
CURRENT                                    TARGET
───────                                    ──────
utxo_sync.rs (DISABLED, was 5 min)        Monitor with individual tasks:
arc_status_poller.rs (60s)                  TaskCheckForProofs (proof acquisition)
cache_sync.rs (10 min)                      TaskSendWaiting (crash recovery)
                                            TaskFailAbandoned (fail stuck txs)
                                            TaskUnFail (recover false failures)
                                            TaskReviewStatus (status → outputs)
                                            TaskPurge (cleanup old data)

utxo_sync.rs (on-demand only)             POST /wallet/sync endpoint
balance via API polling                    Balance from local outputs + cache invalidation
broadcast (1 attempt per service)          Retry with transient/permanent error classification
```

#### Design Decisions (from planning discussion 2026-02-07)

1. **Full Monitor pattern**: All 6 named tasks implemented, not a simplified subset
2. **On-demand UTXO sync**: New `POST /wallet/sync` endpoint replaces periodic sync. Frontend will call this later (separate phase)
3. **Balance strategy (Option A)**: Backend tracks sats only with immediate cache invalidation on output changes. Frontend continues to handle exchange rate (CryptoCompare + CoinGecko). Frontend polling enablement deferred to a later phase
4. **Broadcast retry**: Classify errors as transient vs permanent. Retry transient errors (up to 2 additional attempts per broadcaster with backoff). Never retry permanent errors. TaskSendWaiting handles crash recovery only (orphaned `sending` status txs)
5. **Pending address expiry**: Changed from 24 hours to 10 days. New addresses remain `pending_utxo_check=1` until a UTXO is found OR 10 days pass
6. **Frontend changes deferred**: Sync button, balance polling enablement, and price caching are separate phase work

#### Ghost Transaction Safety Rules

The Phase 4 ghost output bug taught us that background services must NEVER create or destroy output records without careful state verification. These rules apply to all Phase 6 tasks:

1. **TaskSendWaiting** must verify the transaction's outputs and inputs are still in the expected state before re-broadcasting. If outputs were already cleaned up (ghost outputs deleted, inputs restored), do NOT re-broadcast — the tx would double-spend
2. **TaskFailAbandoned** must use the same cleanup sequence as the current broadcast failure handler: (a) mark tx `failed`, (b) delete ghost change outputs, (c) restore input UTXOs, (d) invalidate balance cache — in that exact order
3. **TaskUnFail** must verify a proof exists on-chain BEFORE changing any output state. If proof found: mark `completed`, set outputs spendable. If no proof: leave as `failed`, do NOT touch outputs
4. **TaskReviewStatus** propagates status changes but must NEVER create or delete outputs — only update `spendable` flags on existing outputs
5. **All tasks**: Log every output state change at INFO level with txid + outputId for audit trail

#### 6A: Schema Changes (Migration V20)

```sql
-- Create monitor_events table for task event logging
CREATE TABLE IF NOT EXISTS monitor_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event TEXT NOT NULL,
    details TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX idx_monitor_events_event ON monitor_events(event);
CREATE INDEX idx_monitor_events_created ON monitor_events(created_at);
```

No other schema changes needed — the existing tables (transactions, outputs, proven_txs, proven_tx_reqs, addresses) already have all required columns from Phases 1-5.

#### 6B: Monitor Module Structure

New `src/monitor/` module with the following structure:

```
src/monitor/
├── mod.rs                      # Monitor struct, task registry, run loop
├── task_check_for_proofs.rs    # Proof acquisition (replaces arc_status_poller + cache_sync)
├── task_send_waiting.rs        # Crash recovery for orphaned 'sending' txs
├── task_fail_abandoned.rs      # Fail stuck unprocessed/unsigned txs
├── task_unfail.rs              # Re-check failed txs for on-chain proof
├── task_review_status.rs       # Propagate proof status → tx status → output spendable
└── task_purge.rs               # Cleanup old failed txs and spent outputs
```

**Monitor struct** (`mod.rs`):
```rust
pub struct Monitor {
    state: web::Data<AppState>,
    client: reqwest::Client,
}

impl Monitor {
    pub fn new(state: web::Data<AppState>) -> Self;
    pub async fn run(&self);  // Main loop: runs each task on its interval
    fn log_event(&self, event: &str, details: Option<&str>);  // Write to monitor_events
}
```

**Run loop design**: Single tokio task with a 30-second tick. Each tick checks which tasks are due based on their individual intervals. Tasks run sequentially within each tick to avoid concurrent DB access issues.

#### 6C: Task Specifications

##### TaskCheckForProofs (replaces arc_status_poller.rs + cache_sync.rs)

**Interval**: 60 seconds
**Purpose**: Acquire merkle proofs for unproven transactions

**Logic**:
1. Query `transactions WHERE new_status IN ('sending', 'unproven')` — these need proofs
2. For each, check `proven_tx_reqs` status:
   - If `proven_tx_reqs.proven_tx_id IS NOT NULL` → proof already exists, reconcile (step 5)
   - If attempts >= 8 → skip (TaskFailAbandoned will handle)
3. Query ARC API: `GET /v1/tx/{txid}` for status
   - If MINED: create `proven_txs` record from ARC merklePath, link to `proven_tx_reqs`, increment attempts
   - If not found on ARC: query WhatsOnChain TSC endpoint as fallback
   - If found on WoC: create `proven_txs` record from TSC proof
   - If not found anywhere: increment `proven_tx_reqs.attempts`, add history note
4. On proof acquisition: update `proven_tx_reqs.status = 'completed'`, link `proven_tx_id`
5. Reconcile: update `transactions.new_status = 'completed'`, set `transactions.proven_tx_id`

**Migrated from**: `arc_status_poller.rs` (ARC polling + reconciliation) and `cache_sync.rs` (WhatsOnChain TSC fetch)

##### TaskSendWaiting (crash recovery only)

**Interval**: 120 seconds
**Purpose**: Re-broadcast transactions stuck in `sending` status due to app crash or network drop

**Logic**:
1. Query `transactions WHERE new_status = 'sending' AND created_at < (now - 120 seconds)`
   - The 120-second delay ensures we don't interfere with an active broadcast
2. For each stuck transaction:
   a. **Verify output state is intact**: Check that ghost change outputs still exist AND input outputs are still reserved (spendable=0, spent_by=this tx). If outputs were already cleaned up → mark tx `failed`, do NOT broadcast
   b. Get raw tx bytes from `transactions.raw_tx` or `proven_tx_reqs.raw_tx`
   c. If no raw tx available → mark tx `failed` (nothing to broadcast)
   d. Attempt broadcast using `broadcast_transaction()` (which now has retry logic — see 6E)
   e. On success → update status to `unproven`
   f. On permanent failure → run full failure cleanup (delete ghost outputs, restore inputs, invalidate cache, mark `failed`)
   g. On transient failure after all retries exhausted → same as permanent failure cleanup
3. Max 3 crash-recovery attempts per transaction (tracked via `proven_tx_reqs.attempts` or a counter). After 3 failed recovery attempts → mark `failed` permanently

**Ghost safety**: Step 2a is critical. If the app crashed mid-cleanup, outputs may be in an inconsistent state. Always verify before broadcasting.

##### TaskFailAbandoned (fail stuck transactions)

**Interval**: 300 seconds (5 minutes)
**Purpose**: Fail transactions that were created but never completed signing/broadcasting

**Logic**:
1. Query `transactions WHERE new_status IN ('unprocessed', 'unsigned') AND created_at < (now - 300 seconds)`
2. For each:
   a. Mark `new_status = 'failed'`, set `failed_at = now`
   b. Delete any ghost change outputs created for this tx: `DELETE FROM outputs WHERE transaction_id = ? AND change = 1`
   c. Restore input outputs: `UPDATE outputs SET spendable = 1, spent_by = NULL WHERE spent_by = ?`
   d. Invalidate balance cache
   e. Log to monitor_events: "TaskFailAbandoned: failed tx {txid}, restored {n} inputs"

**Replaces**: Startup cleanup logic in `main.rs` that marks stale pending txs as failed

##### TaskUnFail (recover false failures)

**Interval**: 300 seconds (5 minutes)
**Purpose**: Re-check recently failed transactions — they may have actually succeeded on-chain despite broadcast error

**Logic**:
1. Query `transactions WHERE new_status = 'failed' AND failed_at IS NOT NULL AND failed_at > (now - 1800 seconds)`
   - Only check txs failed within the last 30 minutes (UnFail window)
2. For each:
   a. Check if `proven_txs` record exists for this txid → if so, it was mined
   b. If no proven_txs: query ARC `GET /v1/tx/{txid}` for status
   c. If no ARC result: query WhatsOnChain for the txid
   d. If found on-chain (mined):
      - Create `proven_txs` record if not exists
      - Update `new_status = 'completed'`
      - Re-create change outputs if they were deleted (use raw_tx to parse outputs)
      - Mark change outputs as `spendable = 1`
      - Mark input outputs as `spendable = 0` (they were spent on-chain)
      - Invalidate balance cache
      - Log: "TaskUnFail: recovered tx {txid} — was marked failed but found on-chain"
   e. If NOT found on-chain and `failed_at < (now - 1800)`:
      - Transaction is permanently failed
      - Ensure cleanup was done (ghost outputs deleted, inputs restored)
      - Log: "TaskUnFail: confirmed failure for tx {txid} after 30-min window"

**Ghost safety**: Step 2d is the most dangerous operation — re-creating outputs for a tx we thought failed. Must parse raw_tx to get exact output values/scripts. Only do this if we have confirmed on-chain proof (merkle path).

##### TaskReviewStatus (status propagation)

**Interval**: 60 seconds
**Purpose**: Ensure consistency between proven_tx_reqs → transactions → outputs

**Logic**:
1. **proven_tx_reqs → transactions**: Find `proven_tx_reqs WHERE status = 'completed' AND notified = 0`. For each, ensure the linked transaction has `new_status = 'completed'` and `proven_tx_id` set. Mark `notified = 1`
2. **transactions → outputs**: Find `transactions WHERE new_status = 'completed'`. For each, ensure all outputs with `transaction_id = ?` have `spendable = 1` (unless spent by another tx). This catches outputs that weren't updated during proof acquisition
3. **Failed tx cleanup verification**: Find `transactions WHERE new_status = 'failed' AND failed_at < (now - 1800)`. Verify ghost outputs are deleted and inputs restored. Fix any inconsistencies found
4. Log summary: "TaskReviewStatus: reconciled {n} proofs, {m} outputs, {k} failed cleanups"

**Ghost safety**: This task ONLY updates `spendable` flags and FKs on existing outputs. It never creates or deletes output rows.

##### TaskPurge (cleanup old data)

**Interval**: 3600 seconds (1 hour)
**Purpose**: Remove old data that's no longer needed

**Logic**:
1. Delete `monitor_events WHERE created_at < (now - 7 days)` — keep 1 week of event history
2. Delete `proven_tx_reqs WHERE status = 'completed' AND notified = 1 AND updated_at < (now - 30 days)` — completed proof requests older than 30 days (the proven_txs record is kept permanently)
3. Future: configurable retention for old failed transactions, spent outputs, etc. (not implemented in Phase 6 — just the infrastructure)

#### 6D: On-Demand UTXO Sync Endpoint

**New endpoint**: `POST /wallet/sync`

Extracts the UTXO sync logic currently embedded in the balance handler (`handlers.rs` lines 1892-2010) into a dedicated endpoint.

**Request**: `POST /wallet/sync` (no body required)
**Response**: `{ "synced_addresses": N, "new_utxos": M, "balance": S }`

**Logic**:
1. Get all addresses with `pending_utxo_check = 1` for current user
2. Also include master address (`index = -1`) if it has `pending_utxo_check = 1`
3. For each pending address, fetch UTXOs from WhatsOnChain API
4. Insert new UTXOs as outputs (with derivation_prefix/suffix from address data)
5. Clear `pending_utxo_check` flag on addresses where UTXOs were found OR where the scan completed successfully (even with 0 UTXOs)
6. Invalidate and update balance cache
7. Return summary

**Optional query parameter**: `?full=true` — syncs ALL addresses, not just pending ones. Useful for recovery or manual full refresh.

**Pending address lifecycle** (updated):
- New address created → `pending_utxo_check = 1`
- `/wallet/sync` runs → scans address → clears flag (whether UTXOs found or not)
- If address scan fails (API error) → flag stays `1`, will retry on next sync call
- Stale pending expiry: addresses with `pending_utxo_check = 1` AND `created_at < (now - 10 days)` are auto-cleared
  - Changed from 24 hours to 10 days — 24h was too aggressive for addresses that haven't received funds yet
  - Rationale: a user might generate an address, share it, and receive payment days later

**Changes to balance handler**: Remove the inline UTXO sync logic from `wallet_balance()`. The balance handler should ONLY read from cache/DB, never trigger API calls. UTXO syncing is now the responsibility of `POST /wallet/sync`.

#### 6E: Broadcast Retry with Error Classification

Improve `broadcast_transaction()` in `handlers.rs` to distinguish transient vs permanent failures and retry appropriately.

**Error classification**:

| Error Type | Examples | Action |
|---|---|---|
| **Permanent** (never retry) | `ERROR: 16: mandatory-script-verify-flag-failed`, `Missing inputs`, `txn-mempool-conflict`, `dust`, BEEF validation (ARC 460-469), double-spend (`competingTxs` in ARC response) | Return error immediately, mark `failed` |
| **Transient** (retry with backoff) | Network timeout, HTTP 500/502/503, connection refused, `SEEN_IN_ORPHAN_MEMPOOL`, DNS failure | Retry up to 2 additional times per broadcaster |
| **Already known** (treat as success) | HTTP 409, `txn-already-in-mempool`, `txn-already-known`, `duplicate` | Return success |

**Retry strategy within broadcast_transaction()**:
```
For each broadcaster (ARC, GorillaPool mAPI, WhatsOnChain):
    attempt 1 → if transient failure → wait 2s → attempt 2 → if transient failure → wait 4s → attempt 3
    if permanent failure → skip remaining attempts for this broadcaster, try next broadcaster
    if all 3 attempts fail with transient errors → try next broadcaster

Total worst case: 3 broadcasters × 3 attempts = 9 attempts (with ~18s of backoff)
```

**Implementation approach**:
1. Add `BroadcastError` enum: `Permanent(String)` vs `Transient(String)`
2. Update `broadcast_to_arc()`, `broadcast_to_gorillapool()`, `broadcast_to_whatsonchain()` to return `Result<String, BroadcastError>` instead of `Result<String, String>`
3. Add retry loop in `broadcast_transaction()` that respects error classification
4. On permanent error from ANY broadcaster → stop all retries, return error immediately (the tx is fundamentally broken)
5. On transient error → retry with backoff, then try next broadcaster

**Ghost safety**: The retry logic is contained within the existing `broadcast_transaction()` call. The caller's failure handler (ghost output cleanup, input restoration) only runs AFTER all retry attempts are exhausted. No change to the cleanup sequence.

#### 6F: Balance Cache Improvements

**Immediate cache invalidation** — invalidate balance cache whenever outputs change:

| Operation | Current Invalidation | Phase 6 Invalidation |
|---|---|---|
| `output_repo.insert_output()` | None | `balance_cache.invalidate()` |
| `output_repo.mark_spent()` | None | `balance_cache.invalidate()` |
| `output_repo.delete_by_txid()` | Only on broadcast failure | `balance_cache.invalidate()` |
| `output_repo.restore_spent_by_txid()` | Only on broadcast failure | `balance_cache.invalidate()` |
| `POST /wallet/sync` | Yes (already does) | Yes |
| Monitor tasks (any output change) | N/A | `balance_cache.invalidate()` |

**Implementation**: Add `balance_cache` parameter to `OutputRepository` methods that modify output state, or call invalidation at the handler/monitor level after each output-modifying operation.

**Remove inline UTXO sync from balance handler**: The current `wallet_balance()` handler does UTXO API fetching inline (lines 1813-2010). This should be removed — balance should only read from local DB/cache. UTXO discovery is now `POST /wallet/sync` only.

#### Rust Code Changes Summary

| File | Change |
|---|---|
| `src/monitor/mod.rs` | **New module**: Monitor struct with task registry, 30s tick loop, event logging to `monitor_events` table |
| `src/monitor/task_check_for_proofs.rs` | **New**: Merges ARC poller + cache_sync. Uses proven_tx_reqs lifecycle, queries ARC then WoC for proofs |
| `src/monitor/task_send_waiting.rs` | **New**: Crash recovery for orphaned `sending` txs. Verifies output state before re-broadcast |
| `src/monitor/task_fail_abandoned.rs` | **New**: Fails `unprocessed`/`unsigned` txs older than 5 min. Full ghost cleanup |
| `src/monitor/task_unfail.rs` | **New**: Re-checks failed txs (within 30-min window) for on-chain proof. Recovers false failures |
| `src/monitor/task_review_status.rs` | **New**: Ensures consistency across proven_tx_reqs → transactions → outputs |
| `src/monitor/task_purge.rs` | **New**: Cleans up old monitor_events (7d) and completed proven_tx_reqs (30d) |
| `src/handlers.rs` | Add `POST /wallet/sync` endpoint. Remove inline UTXO sync from `wallet_balance()`. Add `BroadcastError` enum and retry logic to `broadcast_transaction()`. Classify errors in each broadcaster function |
| `src/database/address_repo.rs` | Change `clear_stale_pending_addresses()` from 24h to 10 days (240h) |
| `src/database/output_repo.rs` | Add balance cache invalidation calls to write methods |
| `src/database/migrations.rs` | Add `create_schema_v20()` for monitor_events table |
| `src/database/connection.rs` | Add V20 migration runner |
| `src/balance_cache.rs` | No structural changes (30s TTL + invalidation already works) |
| `src/utxo_sync.rs` | Refactor into `sync_pending_addresses()` function callable from sync endpoint. Remove periodic loop |
| `src/arc_status_poller.rs` | **Removed**: logic moved to `task_check_for_proofs` |
| `src/cache_sync.rs` | **Removed**: logic moved to `task_check_for_proofs` |
| `src/main.rs` | Start Monitor instead of individual background services. Register `/wallet/sync` route |

#### Implementation Order

Implement in sub-phases to minimize risk:

1. **6A**: Migration V20 (monitor_events table) + Monitor module skeleton with empty tasks
2. **6B**: TaskCheckForProofs — migrate arc_status_poller + cache_sync logic. Run in parallel with old services, compare results
3. **6C**: TaskFailAbandoned + TaskUnFail + TaskReviewStatus — migrate cleanup/reconciliation logic
4. **6D**: TaskSendWaiting — crash recovery with output state verification
5. **6E**: TaskPurge — simple cleanup task
6. **6F**: Broadcast retry with error classification (can be done independently of monitor)
7. **6G**: POST /wallet/sync endpoint + remove inline UTXO sync from balance handler + update pending expiry to 10 days
8. **6H**: Balance cache invalidation on output writes
9. **6I**: Remove old services (arc_status_poller.rs, cache_sync.rs), update main.rs to use Monitor only

Each sub-phase should be tested before proceeding. Sub-phases 6F, 6G, 6H can be done in parallel with 6B-6E since they're independent.

#### Testing Plan

| Test | Verification |
|---|---|
| Monitor startup | Monitor starts, logs "Monitor started with 6 tasks", ticks every 30s |
| TaskCheckForProofs | Unproven tx gets proof → status changes to `completed` |
| TaskSendWaiting | Simulate crash (kill during broadcast) → tx stuck in `sending` → monitor recovers it |
| TaskFailAbandoned | Create tx but don't sign → after 5 min, tx marked `failed`, outputs restored |
| TaskUnFail | Force-fail a tx that was actually mined → within 30 min, monitor recovers it |
| TaskReviewStatus | Manually desync proven_tx_reqs/transactions → monitor reconciles |
| TaskPurge | Insert old monitor_events → verify cleanup after 7 days |
| POST /wallet/sync | Generate new address → call sync → verify UTXO scan runs |
| Broadcast retry | Simulate network timeout → verify retry with backoff → verify permanent error stops all retries |
| Balance cache | Insert output → verify cache invalidated → next balance read recalculates |
| Pending expiry | Create address, don't fund it → verify pending flag stays for 10 days |
| Ghost safety | Kill app mid-broadcast → verify no ghost outputs after monitor recovery |

#### Deferred to Later Phases

| Item | Reason |
|---|---|
| Frontend sync button | Frontend changes are separate phase |
| Frontend balance polling enablement | `useBalance.ts` has polling commented out — re-enable with appropriate interval in frontend phase |
| Frontend price caching | Add 5-min TTL cache for exchange rate when polling is enabled |
| POST /wallet/rebroadcast/{txid} | Not needed — permanent failures should create new txs, transient failures handled by retry logic |

#### Risk: MEDIUM
- Behavioral change in background processing — mitigated by running new tasks alongside old services before cutover
- Broadcast retry logic touches the critical broadcast path — mitigated by error classification (permanent errors bail immediately, same as today)
- Ghost transaction risk in TaskUnFail and TaskSendWaiting — mitigated by explicit output state verification before any action
- Balance cache invalidation adds overhead — mitigated by invalidation being a simple flag set (no computation)

#### Implementation Results (2026-02-07)

All 9 sub-phases (6A-6I) implemented and tested. Additional migrations V21 and V22 were added during testing to fix data issues discovered in production:

**V21**: Patch `proven_txs` merkle_path BLOBs — old code paths stored TSC JSON without the `height` field required for BEEF/BUMP construction. V21 injects height from the `proven_txs.height` column into each BLOB.

**V22**: Fix array-format BLOBs — WhatsOnChain's TSC proof API sometimes returns `[{...}]` (array) instead of `{...}` (object). Old storage code saved the array directly. `serde_json::Value::as_object_mut()` silently fails on arrays, preventing height injection. V22 normalizes arrays to objects and re-injects height.

**Additional fixes applied during testing**:
- FK constraint in `update_txid()`: Phase 4's output-to-transaction FK broke the DELETE+INSERT pattern. Fixed by detaching outputs before DELETE and re-linking after INSERT.
- SEEN_IN_ORPHAN_MEMPOOL: Separated from normal mempool handling with 30-min timeout and WoC on-chain verification before failing.
- UTXO reconciliation: `POST /wallet/sync` was missing the reconciliation step from the old `utxo_sync.rs`. Added `reconcile_for_derivation()` to detect outputs spent on-chain but still marked spendable in the DB.
- ARC txid verification: Added mismatch warning when ARC returns a different txid than expected (indicates broken BEEF ancestry).

**Deferred items confirmed**:
- Frontend sync button, balance polling enablement, and price caching remain deferred to a separate frontend phase
- POST /wallet/rebroadcast endpoint not needed — retry logic integrated into broadcast_transaction()

---

### Phase 7: Per-Output Key Derivation

**Goal**: Eliminate the `addresses` table. Move to per-output key derivation using `derivationPrefix`/`derivationSuffix` on the outputs table. Replace address-based UTXO sync with local output tracking.

**Why last**: This is the most structurally disruptive change. It affects how keys are derived for signing, how UTXOs are discovered, and how balance is calculated. All other phases should be stable before this.

#### Architecture Change

```
CURRENT                                    TARGET
───────                                    ──────
HD wallet: m/{index}                       Per-output derivation:
addresses table tracks indices               derivationPrefix + derivationSuffix
UTXO sync: scan addresses via API            → derive unique key per output
Balance: sum UTXOs by address IDs            Output tracking: wallet creates outputs
                                             locally during createAction/signAction
                                             Balance: sum spendable outputs locally
                                             External sync: only for recovery/import
```

#### Schema Changes (Migration v21)

```sql
-- Backfill derivation data on outputs from addresses table
-- (done in Rust code: for each output, look up address_id → address.index,
--  compute derivationPrefix/Suffix from the index)

-- After backfill is verified complete:
-- addresses table is no longer read by new code
-- Keep for rollback safety; remove in Phase 8
```

#### Rust Code Changes

| File | Change |
|---|---|
| `src/database/helpers.rs` | `derive_private_key_for_output()`: new function that takes derivationPrefix/Suffix instead of address index |
| `src/handlers.rs` | createAction: store derivationPrefix/Suffix on each output created. signAction: derive signing key from output's derivation fields instead of address index |
| `src/utxo_sync.rs` | Convert to recovery-only: scan addresses only during wallet import/recovery, not periodic sync. Normal balance comes from local outputs |
| `src/database/address_repo.rs` | Deprecated — only used for recovery flow |
| `src/balance_cache.rs` | Simplify: balance = SUM(satoshis) FROM outputs WHERE spendable=1 AND user_id=? |

#### Derivation Mapping

For existing HD wallet outputs:
```
address.index = N
→ derivationPrefix = "m/{N}" (or BRC-42 equivalent)
→ derivationSuffix = "" (simple HD) or specific key ID
```

For BRC-42 derived outputs:
```
→ derivationPrefix = "{securityLevel}-{protocolID}"
→ derivationSuffix = "{keyID}"
→ Key derived via BRC-42 ECDH with counterparty
```

#### Risk: HIGH
- Changes key derivation path for signing
- Changes how balance is computed
- Changes UTXO discovery model
- Mitigation: extensive testing with known test vectors. Keep addresses table and old derivation code as fallback. Run old and new derivation in parallel, assert they produce the same keys

---

### Phase 8: Cleanup & Deprecated Table Removal

**Goal**: Remove deprecated tables and old columns that were preserved for rollback safety.

#### Schema Changes (Migration v22)

```sql
-- Remove old status columns from transactions
-- (SQLite doesn't support DROP COLUMN before 3.35.0, so recreate table if needed)

-- Remove deprecated tables
DROP TABLE IF EXISTS transaction_inputs;
DROP TABLE IF EXISTS transaction_outputs;
DROP TABLE IF EXISTS transaction_labels;  -- replaced by tx_labels + tx_labels_map
DROP TABLE IF EXISTS parent_transactions;  -- replaced by proven_txs
DROP TABLE IF EXISTS merkle_proofs;        -- replaced by proven_txs

-- Remove addresses table (if Phase 7 is confirmed stable)
-- DROP TABLE IF EXISTS addresses;  -- CAUTION: only after Phase 7 is proven in production

-- Remove old columns (via table recreation if SQLite version requires)
-- transactions: drop old 'status', 'broadcast_status', 'block_height', 'confirmations'
-- utxos table: drop entirely (replaced by outputs)
```

#### Risk: LOW (if all previous phases are stable)
- Purely removing dead code/tables
- Only run after sufficient production time on new code paths

---

## 5. Data Migration Strategy

### Principles

1. **Additive first**: New tables/columns are added before old ones are removed
2. **Dual-write period**: During transition phases, write to both old and new structures
3. **Shadow comparison**: New code paths run alongside old ones, results compared
4. **Reversible**: Each phase preserves old data for rollback
5. **Existing wallets preserved**: Users should not need to re-create wallets

### Per-Phase Migration Summary

| Phase | Migration Type | Reversibility | Data at Risk | Status |
|---|---|---|---|---|
| 1 (Status) | Column addition + data transform | Full rollback (old columns kept) | None | ✅ Complete |
| 2 (Proven Txs) | New tables + data copy | Full rollback (old tables kept) | None | ✅ Complete |
| 3 (Users) | New table + column additions | Full rollback | None | Pending |
| 4 (Outputs) | New table + data migration | Rollback via old utxos table | **Medium**: UTXO state is critical | Pending |
| 5 (Labels etc) | New tables + data copy | Full rollback | None | Pending |
| 6 (Monitor) | New table + code restructure (monitor pattern, broadcast retry, sync endpoint) | Full rollback (revert code, drop monitor_events) | **Low**: background processing change, broadcast retry | Pending |
| 7 (Derivation) | Data backfill on outputs | Rollback via addresses table | **High**: key derivation is critical | Pending |
| 8 (Cleanup) | Table/column drops | **Not reversible** — backup required | N/A (dead data) | Pending |

### Backup Requirements

- **Before Phase 4**: Full SQLite database backup (UTXO state migration)
- **Before Phase 7**: Full SQLite database backup (key derivation migration)
- **Before Phase 8**: Full SQLite database backup (last chance before table drops)

---

## 6. Export/Import Interoperability

### Goal

A user should be able to export their wallet data from Hodos and import it into a wallet built with the BSV SDK wallet-toolbox (and vice versa).

### What's Needed

After all 8 phases are complete, the Hodos database will have tables that closely mirror the wallet-toolbox schema. An export/import tool would:

1. **Export from Hodos**: Read outputs, transactions, proven_txs, certificates, baskets, tags, labels from SQLite → serialize to a portable JSON format matching wallet-toolbox entity shapes
2. **Import to wallet-toolbox**: Deserialize JSON → insert into Knex-managed database using the toolbox's merge methods (mergeNew/mergeExisting)
3. **Reverse direction**: wallet-toolbox export → Hodos import via equivalent Rust deserialization

### Schema Alignment Checklist (post Phase 8)

| wallet-toolbox Table | Hodos Equivalent | Export-Compatible |
|---|---|---|
| users | users | Yes (after Phase 3) |
| transactions | transactions | Yes (after Phase 1+4) |
| outputs | outputs | Yes (after Phase 4+7) |
| proven_txs | proven_txs | Yes (after Phase 2) |
| proven_tx_reqs | proven_tx_reqs | Yes (after Phase 2) |
| output_baskets | output_baskets | Yes (after Phase 4) |
| output_tags + map | output_tags + map | Yes (after Phase 3) |
| tx_labels + map | tx_labels + map | Yes (after Phase 5) |
| certificates + fields | certificates + fields | Yes (after Phase 3) |
| commissions | commissions | Yes (after Phase 5) |
| settings | settings | Yes (after Phase 5) |
| sync_states | sync_states | Yes (after Phase 5) |

### Fields That Won't Map Directly

| Hodos Extension | Disposition |
|---|---|
| wallets.mnemonic | Not exported (security). User re-enters mnemonic on import |
| baskets.description, token_type, protocol_id | Hodos extensions. Stored as custom metadata on export |
| certificates.certificate_txid | Hodos extension. Mapped to a custom field |
| domain_whitelist | Browser-specific. Not exported |
| messages, relay_messages | Browser-specific. Not exported |

---

*End of transition plan. Each phase is reviewed and approved individually before implementation begins.*

*Phase 1 completed 2026-02-03. Phase 2 completed 2026-02-04. Phase 3 completed 2026-02-05. Phase 4 completed 2026-02-06. Phase 5 completed 2026-02-07. Next: Phase 6 (Monitor Pattern — planned 2026-02-07, 9 sub-phases: 6A-6I).*
