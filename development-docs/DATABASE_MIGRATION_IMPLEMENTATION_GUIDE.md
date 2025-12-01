# Database Migration & Implementation Guide

> **Status**: Ready for Implementation
> **Last Updated**: 2025-01-XX
> **Purpose**: Complete step-by-step guide for migrating HodosBrowser wallet from JSON to SQLite database

## Executive Summary

This guide consolidates all database migration planning into a single, actionable implementation plan. We will migrate from JSON file storage to SQLite database, add support for baskets, certificates, messages, and implement UTXO/parent transaction/Merkle proof caching.

**Key Decisions**:
- ✅ **SQLite** for wallet database (embedded, single-file, ACID-compliant)
- ✅ **Add baskets now** - Required for token management
- ✅ **Add certificates table now** - Prevents migration issues later
- ✅ **Add messages table now** - BRC-33 persistence
- ✅ **Store custom instructions** - Delete after confirmation
- ⏳ **Caching implementation** - Separate phase after migration

---

## Table of Contents

1. [Current State Analysis](#current-state-analysis)
2. [Database Architecture](#database-architecture)
3. [Complete Database Schema](#complete-database-schema)
4. [Implementation Phases](#implementation-phases)
5. [Step-by-Step Implementation](#step-by-step-implementation)
6. [Migration Strategy](#migration-strategy)
7. [Testing Plan](#testing-plan)

---

## Current State Analysis

### Current Storage (JSON Files)

**Location**: `%APPDATA%/HodosBrowser/wallet/`

**Files**:
- `wallet.json` - Wallet identity, addresses, mnemonic
- `actions.json` - Transaction history (StoredAction records)
- `domainWhitelist.json` - Approved domains

### Current Limitations

1. **UTXO Management**:
   - ❌ No UTXO caching - fetches from WhatsOnChain on every transaction
   - ❌ Sequential API calls (one per address)
   - ❌ No relationship tracking (which UTXOs spent/received)

2. **BEEF/SPV Building**:
   - ❌ No parent transaction storage - fetches on every BEEF build
   - ❌ No Merkle proof storage - fetches TSC proofs on every transaction
   - ❌ Example: 3-input transaction = **9 API calls** just to build BEEF!

3. **Data Management**:
   - ❌ No indexing - slow lookups
   - ❌ No transactions - partial writes possible
   - ❌ Memory-intensive - entire files loaded

4. **Missing Features**:
   - ❌ No baskets - can't organize tokens
   - ❌ No certificate storage - BRC-52 not supported
   - ❌ Messages in-memory only - BRC-33 not persistent
   - ❌ Custom instructions not stored

---

## Database Architecture

### Technology: SQLite

**Why SQLite**:
- ✅ Embedded - no separate installation
- ✅ Single file - easy backup (`wallet.db`)
- ✅ ACID-compliant - transaction support
- ✅ Cross-platform - Windows, macOS, Linux
- ✅ Mature - used by Electrum, Bitcoin Core

**Rust Library**: `rusqlite` with `migrations` feature

### Database Location

```
%APPDATA%/HodosBrowser/wallet/
├── wallet.db          # SQLite database
├── wallet.db-wal      # Write-ahead log (auto-created)
├── wallet.json        # Legacy (backup during migration)
└── actions.json       # Legacy (backup during migration)
```

### Database Initialization

**On Wallet Startup**:
1. Check if `wallet.db` exists
2. If not: create database + run migrations
3. If exists: open database + check schema version
4. Run any pending migrations

---

## Complete Database Schema

### Schema Version 1 (Initial)

#### 1. `wallets` Table
```sql
CREATE TABLE wallets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mnemonic TEXT NOT NULL,           -- Encrypted mnemonic seed phrase
    current_index INTEGER NOT NULL DEFAULT 0,
    backed_up BOOLEAN NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,      -- Unix timestamp
    updated_at INTEGER NOT NULL       -- Unix timestamp
);

CREATE INDEX idx_wallets_id ON wallets(id);
```

#### 2. `addresses` Table
```sql
CREATE TABLE addresses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    wallet_id INTEGER NOT NULL,
    index INTEGER NOT NULL,           -- HD derivation index (0, 1, 2...)
    address TEXT NOT NULL UNIQUE,     -- BSV address (P2PKH)
    public_key TEXT NOT NULL,         -- Hex-encoded public key
    used BOOLEAN NOT NULL DEFAULT 0,  -- Has this address received funds?
    balance INTEGER NOT NULL DEFAULT 0, -- Cached balance in satoshis
    created_at INTEGER NOT NULL,

    FOREIGN KEY (wallet_id) REFERENCES wallets(id) ON DELETE CASCADE,
    UNIQUE(wallet_id, index)
);

CREATE INDEX idx_addresses_wallet_id ON addresses(wallet_id);
CREATE INDEX idx_addresses_address ON addresses(address);
CREATE INDEX idx_addresses_index ON addresses(wallet_id, index);
```

#### 3. `baskets` Table
```sql
CREATE TABLE baskets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,                -- User-friendly name (e.g., "ToolBSV Tokens")
    description TEXT,                   -- Optional description
    token_type TEXT,                    -- Token type identifier (e.g., "BRC-20", "BRC-721", "custom")
    protocol_id TEXT,                   -- Protocol identifier if applicable
    created_at INTEGER NOT NULL,       -- Unix timestamp
    last_used INTEGER,                 -- Last time tokens from this basket were used

    UNIQUE(name)                        -- Prevent duplicate basket names
);

CREATE INDEX idx_baskets_name ON baskets(name);
```

#### 4. `utxos` Table
```sql
CREATE TABLE utxos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    address_id INTEGER NOT NULL,
    basket_id INTEGER,                 -- Basket assignment (NULL = unassigned)
    txid TEXT NOT NULL,               -- Parent transaction ID
    vout INTEGER NOT NULL,            -- Output index in transaction
    satoshis INTEGER NOT NULL,        -- Amount in satoshis
    script TEXT NOT NULL,             -- Hex-encoded locking script
    first_seen INTEGER NOT NULL,      -- Unix timestamp when first discovered
    last_updated INTEGER NOT NULL,    -- Unix timestamp of last update
    is_spent BOOLEAN NOT NULL DEFAULT 0, -- Marked as spent when used
    spent_txid TEXT,                  -- Transaction that spent this UTXO
    spent_at INTEGER,                 -- When this UTXO was spent

    FOREIGN KEY (address_id) REFERENCES addresses(id) ON DELETE CASCADE,
    FOREIGN KEY (basket_id) REFERENCES baskets(id) ON DELETE SET NULL,
    UNIQUE(txid, vout)
);

CREATE INDEX idx_utxos_address_id ON utxos(address_id);
CREATE INDEX idx_utxos_txid_vout ON utxos(txid, vout);
CREATE INDEX idx_utxos_is_spent ON utxos(is_spent) WHERE is_spent = 0;
CREATE INDEX idx_utxos_address_unspent ON utxos(address_id, is_spent) WHERE is_spent = 0;
CREATE INDEX idx_utxos_basket_id ON utxos(basket_id);
CREATE INDEX idx_utxos_basket_unspent ON utxos(basket_id, is_spent) WHERE is_spent = 0;
```

#### 5. `parent_transactions` Table
```sql
CREATE TABLE parent_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    utxo_id INTEGER NOT NULL,
    txid TEXT NOT NULL UNIQUE,
    raw_hex TEXT NOT NULL,            -- Full transaction in hex
    cached_at INTEGER NOT NULL,       -- Unix timestamp

    FOREIGN KEY (utxo_id) REFERENCES utxos(id) ON DELETE CASCADE,
    UNIQUE(txid)
);

CREATE INDEX idx_parent_txns_txid ON parent_transactions(txid);
CREATE INDEX idx_parent_txns_utxo_id ON parent_transactions(utxo_id);
```

#### 6. `merkle_proofs` Table
```sql
CREATE TABLE merkle_proofs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_txn_id INTEGER NOT NULL,
    block_height INTEGER NOT NULL,
    tx_index INTEGER NOT NULL,        -- Transaction index in block
    target_hash TEXT NOT NULL,        -- Block hash
    nodes TEXT NOT NULL,              -- JSON array of merkle path nodes
    cached_at INTEGER NOT NULL,

    FOREIGN KEY (parent_txn_id) REFERENCES parent_transactions(id) ON DELETE CASCADE,
    UNIQUE(parent_txn_id)
);

CREATE INDEX idx_merkle_proofs_block_height ON merkle_proofs(block_height);
CREATE INDEX idx_merkle_proofs_parent_txn ON merkle_proofs(parent_txn_id);
```

#### 7. `block_headers` Table
```sql
CREATE TABLE block_headers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    block_hash TEXT NOT NULL UNIQUE,
    height INTEGER NOT NULL UNIQUE,
    header_hex TEXT NOT NULL,         -- 80-byte block header
    cached_at INTEGER NOT NULL,

    UNIQUE(block_hash, height)
);

CREATE INDEX idx_block_headers_hash ON block_headers(block_hash);
CREATE INDEX idx_block_headers_height ON block_headers(height);
```

#### 8. `transactions` Table
```sql
CREATE TABLE transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    txid TEXT NOT NULL UNIQUE,
    reference_number TEXT NOT NULL UNIQUE,
    raw_tx TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL,             -- "created", "signed", "unconfirmed", "pending", "confirmed", "aborted", "failed"
    is_outgoing BOOLEAN NOT NULL,
    satoshis INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    block_height INTEGER,
    confirmations INTEGER NOT NULL DEFAULT 0,
    version INTEGER NOT NULL DEFAULT 1,
    lock_time INTEGER NOT NULL DEFAULT 0,
    custom_instructions TEXT,          -- JSON: BRC-29 custom instructions (delete after confirmation)

    UNIQUE(txid),
    UNIQUE(reference_number)
);

CREATE INDEX idx_transactions_txid ON transactions(txid);
CREATE INDEX idx_transactions_reference ON transactions(reference_number);
CREATE INDEX idx_transactions_status ON transactions(status);
CREATE INDEX idx_transactions_timestamp ON transactions(timestamp DESC);
```

#### 9. `transaction_labels` Table
```sql
CREATE TABLE transaction_labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_id INTEGER NOT NULL,
    label TEXT NOT NULL,

    FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE,
    UNIQUE(transaction_id, label)
);

CREATE INDEX idx_transaction_labels_tx_id ON transaction_labels(transaction_id);
CREATE INDEX idx_transaction_labels_label ON transaction_labels(label);
```

#### 10. `transaction_inputs` Table
```sql
CREATE TABLE transaction_inputs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_id INTEGER NOT NULL,
    txid TEXT NOT NULL,               -- Previous transaction ID
    vout INTEGER NOT NULL,            -- Previous output index
    satoshis INTEGER NOT NULL,        -- Amount from previous output
    script TEXT,                      -- Input script (if available)

    FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE
);

CREATE INDEX idx_tx_inputs_tx_id ON transaction_inputs(transaction_id);
CREATE INDEX idx_tx_inputs_prev_tx ON transaction_inputs(txid, vout);
```

#### 11. `transaction_outputs` Table
```sql
CREATE TABLE transaction_outputs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_id INTEGER NOT NULL,
    vout INTEGER NOT NULL,            -- Output index in transaction
    satoshis INTEGER NOT NULL,
    script TEXT,
    address TEXT,                     -- Decoded address (if P2PKH)

    FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE,
    UNIQUE(transaction_id, vout)
);

CREATE INDEX idx_tx_outputs_tx_id ON transaction_outputs(transaction_id);
CREATE INDEX idx_tx_outputs_address ON transaction_outputs(address);
```

#### 12. `certificates` Table (BRC-52)
```sql
CREATE TABLE certificates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    certificate_txid TEXT NOT NULL UNIQUE,  -- Transaction ID of certificate
    identity_key TEXT NOT NULL,             -- Identity key that owns certificate
    attributes TEXT,                         -- JSON: Certificate attributes
    acquired_at INTEGER NOT NULL,            -- Unix timestamp when acquired
    relinquished BOOLEAN NOT NULL DEFAULT 0, -- Has certificate been relinquished?
    relinquished_at INTEGER                  -- When certificate was relinquished
);

CREATE INDEX idx_certificates_identity_key ON certificates(identity_key);
CREATE INDEX idx_certificates_txid ON certificates(certificate_txid);
CREATE INDEX idx_certificates_active ON certificates(identity_key, relinquished) WHERE relinquished = 0;
```

#### 13. `messages` Table (BRC-33)
```sql
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_box TEXT NOT NULL,         -- Message box name (e.g., "coinflip_inbox")
    sender TEXT NOT NULL,               -- Sender's identity key
    recipient TEXT NOT NULL,            -- Recipient's identity key
    body TEXT NOT NULL,                 -- Message content (JSON)
    received_at INTEGER NOT NULL,       -- Unix timestamp
    acknowledged BOOLEAN NOT NULL DEFAULT 0, -- Has message been acknowledged?
    acknowledged_at INTEGER             -- When message was acknowledged
);

CREATE INDEX idx_messages_recipient_box ON messages(recipient, message_box);
CREATE INDEX idx_messages_unacknowledged ON messages(recipient, acknowledged) WHERE acknowledged = 0;
```

#### 14. `domain_whitelist` Table
```sql
CREATE TABLE domain_whitelist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain TEXT NOT NULL UNIQUE,
    added_at INTEGER NOT NULL,
    last_used INTEGER
);

CREATE INDEX idx_domain_whitelist_domain ON domain_whitelist(domain);
```

#### 15. `schema_version` Table
```sql
CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY
);
```

---

## Implementation Phases

### Phase 1: Database Foundation ✅ (Current)
**Goal**: Set up database infrastructure and schema

**Tasks**:
- [ ] Add `rusqlite` dependency to `Cargo.toml`
- [ ] Create `rust-wallet/src/database/` module structure
- [ ] Implement database connection management
- [ ] Create migration system (schema versioning)
- [ ] Implement all 15 tables from schema
- [ ] Test schema creation

**Deliverables**:
- ✅ Database module structure
- ✅ Complete schema implementation
- ✅ Migration system working

### Phase 2: Data Migration
**Goal**: Migrate existing JSON data to database

**Tasks**:
- [ ] Implement JSON → SQLite migration script
- [ ] Migrate `wallet.json` → `wallets` + `addresses` tables
- [ ] Migrate `actions.json` → `transactions` + related tables
- [ ] Implement dual-mode support (read DB, fallback to JSON)
- [ ] Test migration with real wallet data
- [ ] Verify data integrity

**Deliverables**:
- ✅ Migration script
- ✅ All JSON data in database
- ✅ Backward compatibility maintained

### Phase 3: Core Functionality Migration
**Goal**: Replace JSON storage with database operations

**Tasks**:
- [ ] Create repository pattern for each table
- [ ] Implement address CRUD operations
- [ ] Implement transaction CRUD operations
- [ ] Update wallet handlers to use database
- [ ] Remove JSON file dependencies (keep as backup)
- [ ] Test all wallet operations

**Deliverables**:
- ✅ All handlers using database
- ✅ JSON files no longer required
- ✅ All tests passing

### Phase 4: UTXO Management
**Goal**: Implement UTXO caching and sync

**Tasks**:
- [ ] Implement UTXO repository
- [ ] Create UTXO sync service (fetch from WhatsOnChain)
- [ ] Implement UTXO selection algorithm
- [ ] Mark UTXOs as spent when used
- [ ] Background sync process (every 5 minutes)
- [ ] Detect new incoming UTXOs

**Deliverables**:
- ✅ UTXO caching working
- ✅ Background sync running
- ✅ `createAction` uses cached UTXOs

### Phase 5: BEEF/SPV Caching
**Goal**: Cache parent transactions and Merkle proofs

**Tasks**:
- [ ] Implement parent transaction caching
- [ ] Implement Merkle proof caching
- [ ] Implement block header caching
- [ ] Update UTXO sync to fetch parent transactions
- [ ] Update UTXO sync to fetch Merkle proofs
- [ ] Update `signAction` to use cached data
- [ ] Implement cache refresh on reorgs

**Deliverables**:
- ✅ Parent transactions cached
- ✅ Merkle proofs cached
- ✅ BEEF building uses cache (no API calls)

### Phase 6: Basket Implementation
**Goal**: Implement token basket management

**Tasks**:
- [ ] Implement basket repository
- [ ] Add basket assignment to `internalizeAction`
- [ ] Add basket assignment to `createAction`
- [ ] Implement `listOutputs` with basket filtering
- [ ] Add basket balance queries
- [ ] Update UI to display baskets

**Deliverables**:
- ✅ Baskets working
- ✅ UTXOs assigned to baskets
- ✅ Basket queries functional

### Phase 7: Additional Features
**Goal**: Complete remaining features

**Tasks**:
- [ ] Implement custom instructions storage (delete after confirmation)
- [ ] Migrate BRC-33 messages to database
- [ ] Implement certificate storage (BRC-52)
- [ ] Performance optimization
- [ ] Add database indexes
- [ ] Connection pooling if needed

**Deliverables**:
- ✅ All features complete
- ✅ Performance optimized
- ✅ Production ready

---

## Step-by-Step Implementation

### Step 1: Add Dependencies

**File**: `rust-wallet/Cargo.toml`

```toml
[dependencies]
rusqlite = { version = "0.30", features = ["bundled", "migrations"] }
```

### Step 2: Create Database Module Structure

```
rust-wallet/src/
├── database/
│   ├── mod.rs              # Main database module
│   ├── connection.rs       # Database connection & initialization
│   ├── migrations.rs       # Schema migrations
│   ├── models.rs           # Rust structs matching tables
│   ├── wallet_repo.rs      # Wallet CRUD operations
│   ├── address_repo.rs      # Address CRUD operations
│   ├── utxo_repo.rs        # UTXO CRUD operations
│   ├── transaction_repo.rs # Transaction CRUD operations
│   ├── basket_repo.rs      # Basket CRUD operations
│   ├── cache_repo.rs       # Parent tx & proof caching
│   └── message_repo.rs     # BRC-33 message operations
```

### Step 3: Implement Database Connection

**File**: `rust-wallet/src/database/connection.rs`

```rust
use rusqlite::{Connection, Result};
use std::path::PathBuf;

pub struct WalletDatabase {
    conn: Connection,
}

impl WalletDatabase {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Open or create database
        let conn = Connection::open(&db_path)?;

        // Enable WAL mode for better concurrency
        conn.execute("PRAGMA journal_mode=WAL", [])?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys=ON", [])?;

        // Set busy timeout
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        let db = WalletDatabase { conn };

        // Run migrations
        db.migrate()?;

        Ok(db)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn migrate(&self) -> Result<()> {
        // Implementation in migrations.rs
        // ...
    }
}
```

### Step 4: Implement Migrations

**File**: `rust-wallet/src/database/migrations.rs`

```rust
use rusqlite::Connection;

impl WalletDatabase {
    pub fn migrate(&self) -> Result<()> {
        // Create schema_version table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            )",
            [],
        )?;

        // Check current version
        let current_version: i32 = self.conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Apply migrations
        if current_version < 1 {
            self.create_schema_v1()?;
            self.conn.execute(
                "INSERT INTO schema_version (version) VALUES (1)",
                [],
            )?;
        }

        Ok(())
    }

    fn create_schema_v1(&self) -> Result<()> {
        // Create all 15 tables
        // (See schema section above for SQL)
        // ...
    }
}
```

### Step 5: Implement Repositories

**Example**: `rust-wallet/src/database/address_repo.rs`

```rust
use rusqlite::{Connection, Result};
use super::models::Address;

pub struct AddressRepository<'a> {
    conn: &'a Connection,
}

impl<'a> AddressRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        AddressRepository { conn }
    }

    pub fn create(&self, address: &Address) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO addresses (wallet_id, index, address, public_key, used, balance, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                address.wallet_id,
                address.index,
                address.address,
                address.public_key,
                address.used,
                address.balance,
                address.created_at,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_by_address(&self, address: &str) -> Result<Option<Address>> {
        // Query implementation
        // ...
    }

    pub fn get_all(&self, wallet_id: i64) -> Result<Vec<Address>> {
        // Query implementation
        // ...
    }
}
```

### Step 6: Update Main.rs

**File**: `rust-wallet/src/main.rs`

```rust
use crate::database::WalletDatabase;
use std::path::PathBuf;

// Initialize database on startup
let db_path = get_wallet_db_path(); // %APPDATA%/HodosBrowser/wallet/wallet.db
let db = WalletDatabase::new(db_path)?;

// Add to AppState
AppState {
    storage: Mutex::new(JsonStorage::new(...)), // Keep during migration
    action_storage: Mutex::new(ActionStorage::new(...)), // Keep during migration
    database: Arc::new(Mutex::new(db)), // NEW
    // ...
}
```

### Step 7: Migration Script

**File**: `rust-wallet/src/database/migration.rs`

```rust
pub fn migrate_json_to_database(
    db: &WalletDatabase,
    wallet_json_path: &Path,
    actions_json_path: &Path,
) -> Result<()> {
    // 1. Read wallet.json
    // 2. Insert into wallets table
    // 3. Insert addresses into addresses table
    // 4. Read actions.json
    // 5. Insert transactions into transactions table
    // 6. Insert transaction inputs/outputs
    // 7. Insert transaction labels
    // 8. Verify data integrity
    // ...
}
```

---

## Migration Strategy

### Dual-Mode Period

**Phase 1**: Read from database, fallback to JSON
```rust
// Try database first
match db.get_address(address) {
    Ok(Some(addr)) => Ok(addr),
    Ok(None) => {
        // Fallback to JSON
        json_storage.get_address(address)
    }
    Err(e) => {
        log::warn!("Database error, falling back to JSON: {}", e);
        json_storage.get_address(address)
    }
}
```

**Phase 2**: Write to both (database + JSON backup)
```rust
// Write to database
db.create_address(&address)?;

// Also write to JSON (backup)
json_storage.add_address(address)?;
```

**Phase 3**: Database only
```rust
// Only write to database
db.create_address(&address)?;
```

### Migration Safety

- ✅ All new columns are **nullable** - existing data unaffected
- ✅ All new tables are **empty** - no data migration needed initially
- ✅ JSON files kept as backup during transition
- ✅ Rollback possible by reverting to JSON reading

---

## Testing Plan

### Unit Tests
- [ ] Database connection and initialization
- [ ] Schema creation and migrations
- [ ] Repository CRUD operations
- [ ] Data integrity (foreign keys)

### Integration Tests
- [ ] JSON → Database migration
- [ ] Dual-mode reading (DB + JSON fallback)
- [ ] UTXO sync and caching
- [ ] BEEF building with cached data

### Performance Tests
- [ ] Query performance with large datasets
- [ ] UTXO selection algorithm
- [ ] BEEF building speed (cached vs API)

### Data Integrity Tests
- [ ] Migration preserves all data
- [ ] Foreign key constraints work
- [ ] Transaction rollback on errors

---

## Next Steps

1. **Start Phase 1**: Database foundation
   - Add `rusqlite` dependency
   - Create database module structure
   - Implement schema

2. **Test Schema**: Verify all tables create correctly

3. **Begin Phase 2**: Data migration
   - Write migration script
   - Test with sample data

4. **Continue Phases**: Follow phase order above

---

## References

- [metanet-desktop](https://github.com/bsv-blockchain/metanet-desktop) - Reference implementation
- `RUST_WALLET_DB_ARCHITECTURE.md` - Complete schema details
- `BASKET_IMPLEMENTATION_PLAN.md` - Basket design details
- `DATABASE_SCHEMA_DECISIONS.md` - All schema decisions
- SQLite Documentation: https://www.sqlite.org/docs.html
- rusqlite Documentation: https://docs.rs/rusqlite/
