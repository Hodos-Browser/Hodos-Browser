# Rust Wallet Database Architecture

> **Status**: ✅ **Phase 1-9 Complete** | ⏳ Phase 8 (Browser DB) Deferred
> **Last Updated**: December 6, 2025
> **Target**: SQLite database for HodosBrowser wallet data

## Executive Summary

This document defines the database schema and architecture for migrating HodosBrowser wallet from JSON file storage to SQLite. The database will be created and managed entirely in the Rust wallet codebase, initialized during first run (or installation), and will store wallet data, UTXOs, transaction history, and cached BEEF/SPV data.

## Database Technology: SQLite

**Why SQLite:**
- ✅ **Embedded**: No separate installation required
- ✅ **Zero-config**: Single file database
- ✅ **ACID-compliant**: Transaction support built-in
- ✅ **Cross-platform**: Works on Windows, macOS, Linux
- ✅ **Lightweight**: Minimal dependencies
- ✅ **Mature**: Battle-tested in production wallets (Electrum, Bitcoin Core)

**Rust Libraries:**
- Primary: `rusqlite` (most popular SQLite crate for Rust)
- Alternative: `sqlx` (async SQL, can use SQLite)
- Recommended: `rusqlite` with `migrations` feature

## Database Location

**Windows:**
```
%APPDATA%/HodosBrowser/wallet/wallet.db
```

**File Structure:**
```
%APPDATA%/HodosBrowser/wallet/
├── wallet.db            # SQLite database (primary storage)
├── wallet.db-wal        # SQLite write-ahead log (auto-created)
├── wallet.db-shm        # SQLite shared memory (auto-created)
├── wallet.json          # Legacy (kept for compatibility, not used)
└── actions.json         # Legacy (kept for compatibility, not used)
```

**Backup Files**: Saved to user-specified locations (requires file picker dialog in frontend).

## Database Initialization

### Creation Flow

```rust
// rust-wallet/src/database.rs

use rusqlite::{Connection, Result};
use std::path::PathBuf;

pub struct WalletDatabase {
    conn: Connection,
}

impl WalletDatabase {
    /// Initialize database at first run
    /// Creates database file if it doesn't exist
    /// Runs all migrations
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

        // Run migrations
        let db = WalletDatabase { conn };
        db.migrate()?;

        Ok(db)
    }

    /// Run database migrations
    fn migrate(&self) -> Result<()> {
        // Migration 1: Create base schema
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

        // Apply migrations in order
        if current_version < 1 {
            self.create_schema_v1()?;
            self.conn.execute(
                "INSERT INTO schema_version (version) VALUES (1)",
                [],
            )?;
        }

        if current_version < 2 {
            self.create_schema_v2()?;
            self.conn.execute(
                "INSERT INTO schema_version (version) VALUES (2)",
                [],
            )?;
        }

        Ok(())
    }
}
```

### When to Create Database

**Option 1: Lazy Initialization (Recommended)**
- Create database when Rust wallet daemon starts
- Check if `wallet.db` exists
- If not exists: create database + run migrations
- If exists: open database + check schema version

**Option 2: Explicit Initialization**
- Create database when wallet is first created
- After user creates wallet in UI
- Before any data is stored

**Recommended**: Option 1 - Create database on daemon startup

## Database Schema

### Entity Relationship Diagram

```
┌─────────────────┐
│    Wallets      │ 1
│─────────────────│
│ id (PK)         │──┐
│ mnemonic        │  │
│ current_index   │  │
│ backed_up       │  │
│ created_at      │  │
└─────────────────┘  │
                     │ 1
                     │
                     │ N
┌─────────────────┐  │     ┌──────────────────┐
│   Addresses     │◄─┘     │     UTXOs        │
│─────────────────│        │──────────────────│
│ id (PK)         │◄───────│ id (PK)          │
│ wallet_id (FK)  │     N  │ address_id (FK)  │
│ index           │        │ txid             │
│ address         │        │ vout             │
│ public_key      │        │ satoshis         │
│ used            │        │ script           │
│ balance         │        │ first_seen       │
│ created_at      │        │ last_updated     │
└─────────────────┘        │ is_spent         │
                           └──────────────────┘
                                    │
                                    │ 1
                                    │
                                    │ N
                           ┌──────────────────┐
                           │ Parent_Txns      │
                           │──────────────────│
                           │ id (PK)          │
                           │ utxo_id (FK)     │
                           │ txid             │
                           │ raw_hex          │
                           │ cached_at        │
                           └──────────────────┘
                                    │
                                    │ 1
                                    │
                                    │ N
                           ┌──────────────────┐
                           │ Merkle_Proofs    │
                           │──────────────────│
                           │ id (PK)          │
                           │ parent_txn_id(FK)│
                           │ block_height     │
                           │ tx_index         │
                           │ target_hash      │
                           │ nodes (JSON)     │
                           │ cached_at        │
                           └──────────────────┘

┌─────────────────┐        ┌──────────────────┐
│  Transactions   │        │  Block_Headers   │
│─────────────────│        │──────────────────│
│ id (PK)         │        │ id (PK)          │
│ txid            │        │ block_hash       │
│ reference_num   │        │ height           │
│ raw_tx          │        │ header_hex       │
│ status          │        │ cached_at        │
│ is_outgoing     │        └──────────────────┘
│ satoshis        │
│ timestamp       │
│ block_height    │
│ confirmations   │
└─────────────────┘
```

### Table Definitions

#### 1. `wallets` Table

Stores wallet metadata (replaces `wallet.json` structure).

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

**Notes:**
- Single row (one wallet per database)
- `mnemonic` should be encrypted at application level before storage
- `current_index` tracks next address to generate

#### 2. `addresses` Table

Stores HD wallet addresses (migrated from `wallet.json` addresses array).

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

**Migration from JSON:**
- Each entry in `wallet.json.addresses[]` becomes a row
- Maintains same `index` values

#### 3. `baskets` Table

Stores token baskets for organizing UTXOs by purpose/application.

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

**Usage:**
- Organizes UTXOs by application, token type, or purpose
- Enables selective spending from specific baskets
- Helps users understand token ownership

#### 4. `utxos` Table

Stores unspent transaction outputs (cached from blockchain API).

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

**Key Features:**
- Unique constraint on `(txid, vout)` - prevents duplicates
- Partial index on unspent UTXOs for fast queries
- Basket assignment for token organization
- Tracks spending information for transaction history

#### 5. `parent_transactions` Table

Caches parent transaction raw bytes (used for BEEF building).

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

**Usage:**
- Pre-fetched during UTXO sync
- Used by `signAction()` to build BEEF without API calls

#### 6. `merkle_proofs` Table

Stores TSC/BUMP Merkle proofs for SPV verification.

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

**Notes:**
- `nodes` stored as JSON array: `["node1", "node2", ...]`
- One proof per parent transaction (unique constraint)

#### 7. `block_headers` Table

Caches block header data for height resolution.

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

**Usage:**
- Cached during Merkle proof fetching
- Used to resolve block heights quickly

#### 8. `transactions` Table

Stores transaction history (migrated from `actions.json`).

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
    custom_instructions TEXT,          -- JSON: BRC-29 custom instructions (derivationPrefix, derivationSuffix, payee)

    UNIQUE(txid),
    UNIQUE(reference_number)
);

CREATE INDEX idx_transactions_txid ON transactions(txid);
CREATE INDEX idx_transactions_reference ON transactions(reference_number);
CREATE INDEX idx_transactions_status ON transactions(status);
CREATE INDEX idx_transactions_timestamp ON transactions(timestamp DESC);
```

**Migration from JSON:**
- Each entry in `actions.json.actions{}` becomes a row
- `labels` stored separately in `transaction_labels` table

#### 9. `transaction_labels` Table

Stores labels for transactions (many-to-many relationship).

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

**Usage:**
- Allows filtering transactions by labels
- Supports multiple labels per transaction

#### 10. `transaction_inputs` Table

Stores transaction input details.

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

Stores transaction output details.

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

#### 13. `certificates` Table (BRC-52)

Stores digital identity certificates for BRC-52 certificate management.

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

**Usage:**
- Stores BRC-52 identity certificates
- Links certificates to identity keys
- Tracks certificate lifecycle (acquired → relinquished)
- Supports certificate discovery and proof operations

**Note**: Adding this table now prevents migration issues later when implementing BRC-52 (Group C).

#### 14. `domain_whitelist` Table (Optional)

Stores whitelisted domains (currently in `domainWhitelist.json`).

```sql
CREATE TABLE domain_whitelist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain TEXT NOT NULL UNIQUE,
    added_at INTEGER NOT NULL,
    last_used INTEGER
);

CREATE INDEX idx_domain_whitelist_domain ON domain_whitelist(domain);
```

#### 15. `messages` Table (BRC-33 Message Relay)

Stores peer-to-peer messages for BRC-33 message relay system.

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

**Usage:**
- Stores BRC-33 messages for peer-to-peer communication
- Supports multiple message boxes per recipient
- Tracks acknowledgment status for message cleanup

## Rust Implementation Structure

### Module Structure

```
rust-wallet/src/
├── database/
│   ├── mod.rs              # Main database module
│   ├── connection.rs       # Database connection & initialization
│   ├── migrations.rs       # Schema migrations
│   ├── models.rs           # Rust structs matching tables
│   ├── wallet_repo.rs      # Wallet CRUD operations
│   ├── address_repo.rs     # Address CRUD operations
│   ├── utxo_repo.rs        # UTXO CRUD operations
│   ├── transaction_repo.rs # Transaction CRUD operations
│   └── cache_repo.rs       # Parent tx & proof caching
```

### Example: Database Connection Module

```rust
// rust-wallet/src/database/connection.rs

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

        // Set busy timeout (wait up to 5 seconds if locked)
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

### Example: Address Repository

```rust
// rust-wallet/src/database/address_repo.rs

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

## Migration Strategy

### Phase 1: Dual-Mode (JSON + Database)

**Goal**: Support both JSON and database during transition.

```rust
pub enum StorageBackend {
    Json,      // Read/write to JSON files
    Database,  // Read/write to SQLite
    Hybrid,    // Read from JSON, write to database (migration mode)
}

impl WalletStorage {
    pub fn migrate_json_to_database(&mut self, db: &WalletDatabase) -> Result<()> {
        // 1. Read wallet.json
        // 2. Insert into wallets table
        // 3. Insert addresses into addresses table
        // 4. Read actions.json
        // 5. Insert transactions into transactions table
        // 6. Mark migration as complete
    }
}
```

### Phase 2: Database-Only

After migration is complete:
- Remove JSON file reading
- Keep JSON files as backup (don't delete)
- All operations use database

## Performance Considerations

### Indexes

All foreign keys and frequently queried columns are indexed:
- `utxos(txid, vout)` - Primary lookup key
- `utxos(address_id, is_spent)` - Fast unspent query
- `transactions(timestamp DESC)` - Recent transactions

### Query Optimization

**Get Unspent UTXOs:**
```sql
-- Uses partial index for fast query
SELECT * FROM utxos
WHERE address_id = ? AND is_spent = 0
ORDER BY satoshis DESC;
```

**Get Transaction with Labels:**
```sql
SELECT t.*, GROUP_CONCAT(tl.label) as labels
FROM transactions t
LEFT JOIN transaction_labels tl ON t.id = tl.transaction_id
WHERE t.txid = ?
GROUP BY t.id;
```

### Connection Pooling

For future async operations:
- Use `sqlx` with connection pooling
- Or keep `rusqlite` with single connection (SQLite handles it)

## Security Considerations

### Encryption

**Mnemonic Storage:**
- Encrypt mnemonic before storing in database
- Use application-level encryption (AES-256-GCM)
- Store encryption key separately (OS keychain)

**Database File:**
- SQLite doesn't encrypt by default
- Consider SQLCipher extension for encryption
- Or rely on OS-level file encryption

### Backup Strategy

1. **File-Based Backup:**
   - User-specified location via file picker (frontend)
   - Copies database + WAL + SHM files
   - Creates safety backup before restore operations

2. **JSON Export:**
   - Exports non-sensitive data (addresses, transactions, UTXOs)
   - Useful for debugging and migration
   - No mnemonic or private keys included

3. **Recovery from Mnemonic:**
   - Re-derives addresses deterministically
   - Re-discovers UTXOs from blockchain
   - Uses gap limit (default: 20) to determine when to stop

**Frontend Integration Note:**
- Backup/restore endpoints require file picker dialog to let user choose backup location
- User selects destination path via frontend, which is passed to backend API

## Comparison with metanet-desktop

Based on the [metanet-desktop repository](https://github.com/bsv-blockchain/metanet-desktop):

**Similarities:**
- Both use Rust backend
- Both store wallet data locally
- Both need UTXO tracking
- Both need transaction history

**Differences:**
- metanet-desktop uses Tauri (desktop app framework)
- Our implementation is HTTP server daemon
- metanet-desktop may use different storage strategy
- Our focus: SQLite with caching for performance

## Next Steps

1. **Add Dependencies** to `Cargo.toml`:
   ```toml
   rusqlite = { version = "0.30", features = ["bundled", "migrations"] }
   ```

2. **Create Database Module**:
   - Implement connection.rs
   - Implement migrations.rs
   - Create repository pattern for each table

3. **Migration Script**:
   - Read wallet.json
   - Read actions.json
   - Insert into database
   - Verify data integrity

4. **Update Handlers**:
   - Replace JsonStorage with DatabaseRepository
   - Replace ActionStorage with TransactionRepository
   - Update UTXO fetcher to use database cache

5. **Testing**:
   - Unit tests for repositories
   - Integration tests for migrations
   - Performance tests for queries

---

---

## ✅ **Implementation Status** (2025-12-02)

### **Phase 1: Database Foundation** ✅ COMPLETE
- ✅ SQLite database initialization
- ✅ Schema migrations system
- ✅ 15 tables created (wallets, addresses, baskets, utxos, transactions, etc.)
- ✅ WAL mode and foreign keys enabled
- ✅ Database connection management

### **Phase 2: Data Migration** ✅ COMPLETE
- ✅ JSON to database migration script
- ✅ wallet.json → wallets + addresses tables
- ✅ actions.json → transactions + related tables
- ✅ One-time migration completed successfully

### **Phase 3: Core Functionality Migration** ✅ COMPLETE
- ✅ All handlers updated to use database
- ✅ Removed JSON file dependencies
- ✅ Wallet creation with mnemonic generation
- ✅ Address generation and storage
- ✅ Transaction storage and retrieval
- ✅ All API endpoints working with database

### **Phase 4: UTXO Management** ✅ COMPLETE
- ✅ UTXO repository (`utxo_repo.rs`) implemented
- ✅ UTXO caching in database
- ✅ Balance calculation from cache
- ✅ UTXO spending tracking
- ✅ Change address generation (privacy fix)
- ⏳ Background sync service (pending)
- ⏳ Periodic UTXO updates (pending)

### **Phase 5: BEEF/SPV Caching** ⏳ PENDING
- ⏳ Parent transaction caching
- ⏳ Merkle proof caching
- ⏳ Block header caching
- ⏳ TSC proof storage

### **Current Issues:**
- 🚨 **CRITICAL**: Transaction error handling - UI shows success when transaction fails
  - See `CHECKPOINT_TRANSACTION_ERROR_HANDLING.md` for details
- ⚠️ **Performance**: Wallet is slow - fetching from API on every balance check
  - Need to discuss optimization strategy (background sync, periodic updates)

### **Next Steps:**
1. **IMMEDIATE**: Fix transaction error handling (see checkpoint doc)
2. Implement background UTXO sync service
3. Add periodic UTXO update mechanism
4. Begin Phase 5: BEEF/SPV caching

---

**References:**
- SQLite Documentation: https://www.sqlite.org/docs.html
- rusqlite Documentation: https://docs.rs/rusqlite/
- metanet-desktop: https://github.com/bsv-blockchain/metanet-desktop
