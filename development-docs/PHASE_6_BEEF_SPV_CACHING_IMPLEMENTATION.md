# Phase 6: BEEF/SPV Caching - Detailed Implementation Plan (REVISED)

> **Status**: Ready for Implementation
> **Last Updated**: 2025-12-02 (Revised)
> **Goal**: Cache parent transactions, Merkle proofs, and block headers to eliminate on-demand API calls during BEEF building

## Overview

Currently, `signAction()` fetches parent transactions, TSC Merkle proofs, and block headers on-demand from WhatsOnChain API during transaction signing. This causes:
- **Delays**: Multiple sequential API calls during signing
- **API Rate Limiting**: Risk of hitting rate limits during high-volume usage
- **Network Dependency**: Cannot build BEEF transactions offline
- **Redundant Fetches**: Same parent transactions fetched repeatedly

Phase 6 implements caching for all three data types, enabling:
- ✅ Fast BEEF building from cache (no API calls during signing)
- ✅ Background pre-fetching of proofs for confirmed transactions
- ✅ Automatic cache population when UTXOs are synced
- ✅ Fallback to API if cache is missing

## Critical Fixes Applied

### ✅ Fix #1: Schema Migration - Make `utxo_id` Nullable
**Problem**: Schema requires `utxo_id NOT NULL`, but we need to cache parent transactions from external sources (not in our wallet).

**Solution**: Create schema migration v3 to make `utxo_id` nullable:
```sql
ALTER TABLE parent_transactions
  ALTER COLUMN utxo_id INTEGER;  -- Remove NOT NULL constraint
```

**Implementation**: Add `create_schema_v3()` migration function.

### ✅ Fix #2: Function Signature Consistency
**Problem**: `enhance_tsc_with_height()` signature doesn't match usage.

**Solution**: Always pass `block_header_repo` parameter and update all call sites.

### ✅ Fix #3: Database Lock Management
**Problem**: Holding database lock for entire loop blocks other operations.

**Solution**: Release lock between iterations or use connection-per-operation pattern.

### ✅ Fix #4: Error Type Consistency
**Problem**: Mixed `Result<T, String>` and `Result<T, rusqlite::Error>` won't compile with `?` operator.

**Solution**: Create unified error type or use explicit error conversion.

## Current State Analysis

### Database Schema (Needs Migration) ⚠️

**1. `parent_transactions` table** - **REQUIRES MIGRATION**
```sql
-- CURRENT (v1):
utxo_id INTEGER NOT NULL,  -- ❌ Blocks external parent transactions

-- AFTER MIGRATION (v3):
utxo_id INTEGER,  -- ✅ Nullable - allows external parent transactions
```

**2. `merkle_proofs` table** ✅ (No changes needed)
```sql
CREATE TABLE merkle_proofs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_txn_id INTEGER NOT NULL,
    block_height INTEGER NOT NULL,
    tx_index INTEGER NOT NULL,
    target_hash TEXT NOT NULL,
    nodes TEXT NOT NULL,  -- JSON array of node hashes
    cached_at INTEGER NOT NULL,
    FOREIGN KEY (parent_txn_id) REFERENCES parent_transactions(id) ON DELETE CASCADE,
    UNIQUE(parent_txn_id)
)
```

**3. `block_headers` table** ✅ (No changes needed)
```sql
CREATE TABLE block_headers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    block_hash TEXT NOT NULL UNIQUE,
    height INTEGER NOT NULL UNIQUE,
    header_hex TEXT NOT NULL,
    cached_at INTEGER NOT NULL,
    UNIQUE(block_hash, height)
)
```

### Current Implementation

**Location**: `rust-wallet/src/handlers.rs` - `sign_action()` function (lines 2988-3206)

**Current Flow:**
1. Loop through `input_utxos` (parent transaction IDs)
2. For each UTXO:
   - Fetch parent transaction hex from API: `GET /tx/{txid}/hex`
   - Verify TXID matches
   - Add to BEEF structure
   - Fetch TSC proof: `GET /tx/{txid}/proof/tsc` (with retry logic for null proofs)
   - If proof exists, fetch block header: `GET /block/hash/{hash}` (to get height)
   - Add enhanced TSC proof to BEEF with height
3. Add signed transaction as main transaction
4. Serialize to Atomic BEEF format

**Performance Issues:**
- Sequential API calls: ~200-500ms per parent transaction
- 3 API calls per parent: transaction + proof + block header
- No caching between transactions
- Complex nested error handling

## Implementation Steps

### Step 0: Schema Migration (CRITICAL - Do First!)

**File**: `rust-wallet/src/database/migrations.rs`

Add schema version 3 migration:

```rust
/// Create schema version 3 (make parent_transactions.utxo_id nullable)
///
/// Allows caching parent transactions from external sources (not in our wallet).
pub fn create_schema_v3(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 3...");

    // SQLite doesn't support ALTER COLUMN directly, so we need to:
    // 1. Create new table with nullable utxo_id
    // 2. Copy data
    // 3. Drop old table
    // 4. Rename new table

    info!("   Step 1: Creating temporary parent_transactions table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS parent_transactions_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            utxo_id INTEGER,
            txid TEXT NOT NULL UNIQUE,
            raw_hex TEXT NOT NULL,
            cached_at INTEGER NOT NULL,
            FOREIGN KEY (utxo_id) REFERENCES utxos(id) ON DELETE CASCADE
        )",
        [],
    )?;

    info!("   Step 2: Copying data from old table...");
    conn.execute(
        "INSERT INTO parent_transactions_new (id, utxo_id, txid, raw_hex, cached_at)
         SELECT id, utxo_id, txid, raw_hex, cached_at FROM parent_transactions",
        [],
    )?;

    info!("   Step 3: Dropping old table...");
    conn.execute("DROP TABLE parent_transactions", [])?;

    info!("   Step 4: Renaming new table...");
    conn.execute("ALTER TABLE parent_transactions_new RENAME TO parent_transactions", [])?;

    // Recreate indexes
    conn.execute("CREATE INDEX IF NOT EXISTS idx_parent_txns_txid ON parent_transactions(txid)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_parent_txns_utxo_id ON parent_transactions(utxo_id)", [])?;

    info!("   ✅ Schema version 3 migration complete");
    Ok(())
}
```

**Update `connection.rs` migration logic** to apply v3 migration:
```rust
// Apply migration to version 3
if current_version < 3 {
    info!("   Applying migration to version 3...");
    match migrations::create_schema_v3(&self.conn) {
        Ok(()) => {
            info!("   Inserting schema version 3...");
            self.conn.execute(
                "INSERT INTO schema_version (version) VALUES (3)",
                [],
            )?;
            info!("   ✅ Migration to version 3 complete");
        }
        Err(e) => {
            error!("❌ Migration to version 3 failed: {}", e);
            return Err(e);
        }
    }
}
```

### Step 1: Create Unified Error Type

**File**: `rust-wallet/src/cache_errors.rs` (new file)

```rust
use std::fmt;

#[derive(Debug)]
pub enum CacheError {
    Database(rusqlite::Error),
    Api(String),
    InvalidData(String),
    HexDecode(hex::FromHexError),
    Json(serde_json::Error),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CacheError::Database(e) => write!(f, "Database error: {}", e),
            CacheError::Api(e) => write!(f, "API error: {}", e),
            CacheError::InvalidData(e) => write!(f, "Invalid data: {}", e),
            CacheError::HexDecode(e) => write!(f, "Hex decode error: {}", e),
            CacheError::Json(e) => write!(f, "JSON error: {}", e),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<rusqlite::Error> for CacheError {
    fn from(err: rusqlite::Error) -> Self {
        CacheError::Database(err)
    }
}

impl From<hex::FromHexError> for CacheError {
    fn from(err: hex::FromHexError) -> Self {
        CacheError::HexDecode(err)
    }
}

impl From<serde_json::Error> for CacheError {
    fn from(err: serde_json::Error) -> Self {
        CacheError::Json(err)
    }
}

pub type CacheResult<T> = Result<T, CacheError>;
```

### Step 2: Create Database Repository Modules

**File**: `rust-wallet/src/database/parent_transaction_repo.rs`

```rust
use crate::cache_errors::{CacheError, CacheResult};
use super::models::ParentTransaction;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ParentTransactionRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ParentTransactionRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get cached parent transaction by TXID
    pub fn get_by_txid(&self, txid: &str) -> CacheResult<Option<ParentTransaction>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, utxo_id, txid, raw_hex, cached_at
             FROM parent_transactions
             WHERE txid = ?"
        )?;

        let result = stmt.query_row([txid], |row| {
            Ok(ParentTransaction {
                id: row.get(0)?,
                utxo_id: row.get(1)?,
                txid: row.get(2)?,
                raw_hex: row.get(3)?,
                cached_at: row.get(4)?,
            })
        });

        match result {
            Ok(tx) => Ok(Some(tx)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Cache a parent transaction (utxo_id can be None for external transactions)
    pub fn upsert(&self, utxo_id: Option<i64>, txid: &str, raw_hex: &str) -> CacheResult<i64> {
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Use INSERT OR REPLACE to handle duplicates
        self.conn.execute(
            "INSERT OR REPLACE INTO parent_transactions (utxo_id, txid, raw_hex, cached_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![utxo_id, txid, raw_hex, cached_at],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Get parent transaction ID by TXID (for linking merkle proofs)
    pub fn get_id_by_txid(&self, txid: &str) -> CacheResult<Option<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM parent_transactions WHERE txid = ?"
        )?;

        match stmt.query_row([txid], |row| row.get::<_, i64>(0)) {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Verify cached transaction TXID matches expected
    pub fn verify_txid(&self, txid: &str, raw_hex: &str) -> CacheResult<bool> {
        use sha2::{Sha256, Digest};
        let tx_bytes = hex::decode(raw_hex)?;
        let hash1 = Sha256::digest(&tx_bytes);
        let hash2 = Sha256::digest(&hash1);
        let calculated_txid: Vec<u8> = hash2.into_iter().rev().collect();
        let calculated_txid_hex = hex::encode(calculated_txid);
        Ok(calculated_txid_hex == txid)
    }
}
```

**File**: `rust-wallet/src/database/merkle_proof_repo.rs`

```rust
use crate::cache_errors::{CacheError, CacheResult};
use super::models::MerkleProof;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct MerkleProofRepository<'a> {
    conn: &'a Connection,
}

impl<'a> MerkleProofRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get cached Merkle proof for a parent transaction (by TXID)
    pub fn get_by_parent_txid(&self, txid: &str) -> CacheResult<Option<MerkleProof>> {
        let mut stmt = self.conn.prepare(
            "SELECT mp.id, mp.parent_txn_id, mp.block_height, mp.tx_index,
                    mp.target_hash, mp.nodes, mp.cached_at
             FROM merkle_proofs mp
             JOIN parent_transactions pt ON mp.parent_txn_id = pt.id
             WHERE pt.txid = ?"
        )?;

        let result = stmt.query_row([txid], |row| {
            let nodes_json: String = row.get(5)?;
            let nodes: Vec<String> = serde_json::from_str(&nodes_json)
                .unwrap_or_default();

            Ok(MerkleProof {
                id: row.get(0)?,
                parent_txn_id: row.get(1)?,
                block_height: row.get(2)?,
                tx_index: row.get(3)?,
                target_hash: row.get(4)?,
                nodes,
                cached_at: row.get(6)?,
            })
        });

        match result {
            Ok(proof) => Ok(Some(proof)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Cache a Merkle proof (TSC format from WhatsOnChain)
    pub fn upsert(
        &self,
        parent_txn_id: i64,
        block_height: u32,
        tx_index: u64,
        target_hash: &str,
        nodes_json: &str,
    ) -> CacheResult<i64> {
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Use INSERT OR REPLACE to handle duplicates
        self.conn.execute(
            "INSERT OR REPLACE INTO merkle_proofs
             (parent_txn_id, block_height, tx_index, target_hash, nodes, cached_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![parent_txn_id, block_height, tx_index, target_hash, nodes_json, cached_at],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Convert cached proof to TSC JSON format for BEEF building
    pub fn to_tsc_json(&self, proof: &MerkleProof) -> serde_json::Value {
        serde_json::json!({
            "index": proof.tx_index,
            "target": proof.target_hash,
            "nodes": proof.nodes,
            "height": proof.block_height,
        })
    }
}
```

**File**: `rust-wallet/src/database/block_header_repo.rs`

```rust
use crate::cache_errors::{CacheError, CacheResult};
use super::models::BlockHeader;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct BlockHeaderRepository<'a> {
    conn: &'a Connection,
}

impl<'a> BlockHeaderRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get cached block header by hash
    pub fn get_by_hash(&self, block_hash: &str) -> CacheResult<Option<BlockHeader>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, block_hash, height, header_hex, cached_at
             FROM block_headers
             WHERE block_hash = ?"
        )?;

        let result = stmt.query_row([block_hash], |row| {
            Ok(BlockHeader {
                id: row.get(0)?,
                block_hash: row.get(1)?,
                height: row.get(2)?,
                header_hex: row.get(3)?,
                cached_at: row.get(4)?,
            })
        });

        match result {
            Ok(header) => Ok(Some(header)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Get cached block header by height
    pub fn get_by_height(&self, height: u32) -> CacheResult<Option<BlockHeader>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, block_hash, height, header_hex, cached_at
             FROM block_headers
             WHERE height = ?"
        )?;

        let result = stmt.query_row([height], |row| {
            Ok(BlockHeader {
                id: row.get(0)?,
                block_hash: row.get(1)?,
                height: row.get(2)?,
                header_hex: row.get(3)?,
                cached_at: row.get(4)?,
            })
        });

        match result {
            Ok(header) => Ok(Some(header)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Cache a block header
    pub fn upsert(&self, block_hash: &str, height: u32, header_hex: &str) -> CacheResult<i64> {
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Use INSERT OR REPLACE to handle duplicates
        self.conn.execute(
            "INSERT OR REPLACE INTO block_headers (block_hash, height, header_hex, cached_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![block_hash, height, header_hex, cached_at],
        )?;

        Ok(self.conn.last_insert_rowid())
    }
}
```

### Step 3: Create Model Structs

**File**: `rust-wallet/src/database/models.rs` (add to existing file)

```rust
#[derive(Debug, Clone)]
pub struct ParentTransaction {
    pub id: i64,
    pub utxo_id: Option<i64>,  // ✅ Now nullable
    pub txid: String,
    pub raw_hex: String,
    pub cached_at: i64,
}

#[derive(Debug, Clone)]
pub struct MerkleProof {
    pub id: i64,
    pub parent_txn_id: i64,
    pub block_height: u32,
    pub tx_index: u64,
    pub target_hash: String,
    pub nodes: Vec<String>, // Parsed from JSON
    pub cached_at: i64,
}

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub id: i64,
    pub block_hash: String,
    pub height: u32,
    pub header_hex: String,
    pub cached_at: i64,
}
```

### Step 4: Create Helper Functions for API Fetching

**File**: `rust-wallet/src/cache_helpers.rs` (new file)

```rust
use crate::cache_errors::{CacheError, CacheResult};
use crate::database::{BlockHeaderRepository, MerkleProofRepository, ParentTransactionRepository};
use reqwest::Client;
use serde_json::Value;

/// Fetch parent transaction from WhatsOnChain API
pub async fn fetch_parent_transaction_from_api(
    client: &Client,
    txid: &str,
) -> CacheResult<String> {
    let tx_url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/hex", txid);
    let response = client.get(&tx_url).send().await
        .map_err(|e| CacheError::Api(format!("Failed to fetch parent tx {}: {}", txid, e)))?;

    if !response.status().is_success() {
        return Err(CacheError::Api(format!(
            "API returned status {} for tx {}", response.status(), txid
        )));
    }

    response.text().await
        .map_err(|e| CacheError::Api(format!("Failed to read parent tx response: {}", e)))
}

/// Fetch TSC Merkle proof from WhatsOnChain API (with retry logic for null proofs)
pub async fn fetch_tsc_proof_from_api(
    client: &Client,
    txid: &str,
) -> CacheResult<Option<Value>> {
    let proof_url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/proof/tsc", txid);

    // First attempt
    let response = client.get(&proof_url).send().await
        .map_err(|e| CacheError::Api(format!("Failed to fetch TSC proof for {}: {}", txid, e)))?;

    if !response.status().is_success() {
        return Err(CacheError::Api(format!(
            "TSC proof API returned status {}", response.status()
        )));
    }

    let proof_text = response.text().await
        .map_err(|e| CacheError::Api(format!("Failed to read TSC proof response: {}", e)))?;

    let tsc_json: Value = serde_json::from_str(&proof_text)?;

    // If null, retry once after delay (transaction might be confirming)
    if tsc_json.is_null() {
        log::warn!("   ⚠️  TSC proof is null - retrying after 2 seconds...");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let retry_response = client.get(&proof_url).send().await
            .map_err(|e| CacheError::Api(format!("Retry failed: {}", e)))?;

        if retry_response.status().is_success() {
            let retry_text = retry_response.text().await
                .map_err(|e| CacheError::Api(format!("Failed to read retry response: {}", e)))?;
            let retry_json: Value = serde_json::from_str(&retry_text)?;

            if retry_json.is_null() {
                return Ok(None); // Still null after retry
            }
            return Ok(Some(retry_json));
        }
        return Ok(None);
    }

    // Normalize array response to single object
    let tsc_obj = if tsc_json.is_array() {
        tsc_json.get(0).cloned().unwrap_or(tsc_json)
    } else {
        tsc_json
    };

    Ok(Some(tsc_obj))
}

/// Enhance TSC proof with block height (fetch from cache or API)
pub async fn enhance_tsc_with_height(
    client: &Client,
    block_header_repo: &BlockHeaderRepository,
    tsc_json: &Value,
) -> CacheResult<Value> {
    let target_hash = tsc_json["target"].as_str()
        .ok_or_else(|| CacheError::InvalidData("Missing target hash in TSC proof".to_string()))?;

    // Try cache first
    if let Some(header) = block_header_repo.get_by_hash(target_hash)? {
        let mut enhanced = tsc_json.clone();
        enhanced["height"] = serde_json::json!(header.height);
        return Ok(enhanced);
    }

    // Fetch from API
    let block_header_url = format!("https://api.whatsonchain.com/v1/bsv/main/block/hash/{}", target_hash);
    let response = client.get(&block_header_url).send().await
        .map_err(|e| CacheError::Api(format!("Failed to fetch block header: {}", e)))?;

    if !response.status().is_success() {
        return Err(CacheError::Api(format!(
            "Block header API returned status {}", response.status()
        )));
    }

    let header_json: Value = response.json().await
        .map_err(|e| CacheError::Api(format!("Failed to parse block header JSON: {}", e)))?;

    let height = header_json["height"].as_u64()
        .ok_or_else(|| CacheError::InvalidData("Missing height in block header".to_string()))? as u32;

    // Cache the header
    let header_hex = header_json["header"].as_str().unwrap_or("");
    block_header_repo.upsert(target_hash, height, header_hex)?;

    // Enhance TSC proof
    let mut enhanced = tsc_json.clone();
    enhanced["height"] = serde_json::json!(height);
    Ok(enhanced)
}

/// Verify that transaction bytes match expected TXID
pub fn verify_txid(tx_bytes: &[u8], expected_txid: &str) -> CacheResult<()> {
    use sha2::{Sha256, Digest};
    let hash1 = Sha256::digest(tx_bytes);
    let hash2 = Sha256::digest(&hash1);
    let calculated_txid: Vec<u8> = hash2.into_iter().rev().collect();
    let calculated_txid_hex = hex::encode(calculated_txid);

    if calculated_txid_hex != expected_txid {
        return Err(CacheError::InvalidData(format!(
            "TXID mismatch: expected {}, got {}", expected_txid, calculated_txid_hex
        )));
    }
    Ok(())
}

/// Get UTXO database ID for linking parent transactions
pub fn get_utxo_id_from_db(
    conn: &rusqlite::Connection,
    txid: &str,
    vout: u32,
) -> Result<Option<i64>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id FROM utxos WHERE txid = ? AND vout = ? AND is_spent = 0"
    )?;

    match stmt.query_row([txid, &vout.to_string()], |row| row.get::<_, i64>(0)) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}
```

### Step 5: Update signAction() to Use Cache (WITH LOCK MANAGEMENT)

**File**: `rust-wallet/src/handlers.rs` - `sign_action()` function

**Key Changes:**
1. ✅ Release database lock between iterations
2. ✅ Verify cached data (TXID check)
3. ✅ Handle partial cache scenarios
4. ✅ Preserve retry logic for TSC proofs
5. ✅ Use unified error types

```rust
// Before the loop, create HTTP client
let client = reqwest::Client::new();

// Loop through input UTXOs
for (i, utxo) in input_utxos.iter().enumerate() {
    log::info!("   📥 Processing parent tx {}/{}: {}", i + 1, input_utxos.len(), utxo.txid);

    // STEP 1: Try to get parent transaction from cache
    // ✅ Release lock after each operation
    let parent_tx_bytes = {
        let db = state.database.lock().unwrap();
        let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());

        match parent_tx_repo.get_by_txid(&utxo.txid)? {
            Some(cached) => {
                // ✅ Verify cached data
                if !parent_tx_repo.verify_txid(&utxo.txid, &cached.raw_hex)? {
                    log::warn!("   ⚠️  Cached parent tx {} failed TXID verification, fetching from API", utxo.txid);
                    // Fall through to API fetch
                } else {
                    log::info!("   ✅ Using cached parent tx {} (cached at {})", utxo.txid, cached.cached_at);
                    drop(db); // Release lock before hex decode
                    hex::decode(&cached.raw_hex)?
                }
            }
            None => {
                drop(db); // Release lock before API call
                log::info!("   🌐 Cache miss - fetching parent tx {} from API...", utxo.txid);
                // Fetch from API
                let parent_tx_hex = crate::cache_helpers::fetch_parent_transaction_from_api(&client, &utxo.txid).await?;
                let parent_tx_bytes = hex::decode(&parent_tx_hex)?;

                // Verify TXID
                crate::cache_helpers::verify_txid(&parent_tx_bytes, &utxo.txid)?;

                // Cache it for next time
                {
                    let db = state.database.lock().unwrap();
                    let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());
                    let utxo_id = crate::cache_helpers::get_utxo_id_from_db(db.connection(), &utxo.txid, utxo.vout)
                        .ok()  // Ignore DB errors, just don't link
                        .flatten();
                    parent_tx_repo.upsert(utxo_id, &utxo.txid, &parent_tx_hex)?;
                    log::info!("   💾 Cached parent tx {}", utxo.txid);
                }

                parent_tx_bytes
            }
        }
    };

    let tx_index = beef.add_parent_transaction(parent_tx_bytes);

    // STEP 2: Try to get Merkle proof from cache
    let enhanced_tsc = {
        let db = state.database.lock().unwrap();
        let merkle_proof_repo = crate::database::MerkleProofRepository::new(db.connection());

        match merkle_proof_repo.get_by_parent_txid(&utxo.txid)? {
            Some(cached_proof) => {
                log::info!("   ✅ Using cached Merkle proof for {} (height: {})", utxo.txid, cached_proof.block_height);
                drop(db); // Release lock
                merkle_proof_repo.to_tsc_json(&cached_proof)
            }
            None => {
                drop(db); // Release lock before API call
                log::info!("   🌐 Cache miss - fetching TSC proof from API...");

                // Fetch TSC proof from API (with retry logic)
                match crate::cache_helpers::fetch_tsc_proof_from_api(&client, &utxo.txid).await? {
                    Some(tsc_json) => {
                        // Get block height from block header (cache or API)
                        let db = state.database.lock().unwrap();
                        let block_header_repo = crate::database::BlockHeaderRepository::new(db.connection());
                        let enhanced_tsc = crate::cache_helpers::enhance_tsc_with_height(
                            &client,
                            &block_header_repo,
                            &tsc_json,
                        ).await?;

                        // Cache the proof
                        if let Some(parent_txn_id) = {
                            let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());
                            parent_tx_repo.get_id_by_txid(&utxo.txid)?
                        } {
                            let target_hash = enhanced_tsc["target"].as_str().unwrap_or("");
                            let nodes_json = serde_json::to_string(&enhanced_tsc["nodes"])?;
                            let block_height = enhanced_tsc["height"].as_u64().unwrap_or(0) as u32;
                            let tx_index = enhanced_tsc["index"].as_u64().unwrap_or(0);

                            merkle_proof_repo.upsert(parent_txn_id, block_height, tx_index, target_hash, &nodes_json)?;
                            log::info!("   💾 Cached Merkle proof for {}", utxo.txid);
                        }

                        enhanced_tsc
                    }
                    None => {
                        log::warn!("   ⚠️  TSC proof not available (tx not confirmed)");
                        serde_json::Value::Null  // Return null to skip proof
                    }
                }
            }
        }
    };

    // STEP 3: Add proof to BEEF
    if !enhanced_tsc.is_null() {
        beef.add_tsc_merkle_proof(&utxo.txid, tx_index, &enhanced_tsc)?;
        log::info!("   ✅ Added TSC Merkle proof (BUMP) to BEEF");
    }
}
```

### Step 6: Background Cache Population Service

**File**: `rust-wallet/src/cache_sync.rs` (new file)

```rust
use tokio::time::{sleep, Duration};
use crate::database::*;
use crate::cache_helpers;
use crate::cache_errors::CacheResult;
use actix_web::web;
use crate::AppState;
use reqwest::Client;

/// Background service to populate BEEF cache
pub async fn start_cache_sync_service(state: web::Data<AppState>) {
    let client = Client::new();

    loop {
        // Run every 10 minutes (configurable)
        sleep(Duration::from_secs(600)).await;

        log::info!("🔄 Starting BEEF cache sync...");

        match sync_cache_for_confirmed_utxos(&state, &client).await {
            Ok(count) => {
                log::info!("✅ Cache sync complete: {} proofs cached", count);
            }
            Err(e) => {
                log::error!("❌ Cache sync failed: {}", e);
            }
        }
    }
}

/// Sync cache for confirmed UTXOs that don't have proofs yet
async fn sync_cache_for_confirmed_utxos(
    state: &web::Data<AppState>,
    client: &Client,
) -> CacheResult<usize> {
    let mut cached_count = 0;
    const BATCH_SIZE: usize = 50;  // Limit to avoid rate limits

    // Get UTXOs without cached proofs
    let utxos_to_sync: Vec<(String, u32, Option<i64>)> = {
        let db = state.database.lock().unwrap();
        let mut stmt = db.connection().prepare(
            "SELECT DISTINCT u.txid, u.vout, u.id
             FROM utxos u
             LEFT JOIN parent_transactions pt ON pt.txid = u.txid
             LEFT JOIN merkle_proofs mp ON mp.parent_txn_id = pt.id
             WHERE u.is_spent = 0 AND mp.id IS NULL
             LIMIT ?"
        )?;

        let rows = stmt.query_map([BATCH_SIZE], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        result
    };

    log::info!("   Found {} UTXOs to sync", utxos_to_sync.len());

    // For each UTXO:
    for (txid, vout, utxo_id) in utxos_to_sync {
        // 1. Fetch parent transaction (if not cached)
        {
            let db = state.database.lock().unwrap();
            let parent_tx_repo = ParentTransactionRepository::new(db.connection());

            if parent_tx_repo.get_by_txid(&txid)?.is_none() {
                drop(db);

                match cache_helpers::fetch_parent_transaction_from_api(client, &txid).await {
                    Ok(parent_tx_hex) => {
                        let parent_tx_bytes = hex::decode(&parent_tx_hex)?;
                        cache_helpers::verify_txid(&parent_tx_bytes, &txid)?;

                        let db = state.database.lock().unwrap();
                        let parent_tx_repo = ParentTransactionRepository::new(db.connection());
                        parent_tx_repo.upsert(utxo_id, &txid, &parent_tx_hex)?;
                        log::debug!("   💾 Cached parent tx {}", txid);
                    }
                    Err(e) => {
                        log::warn!("   ⚠️  Failed to fetch parent tx {}: {}", txid, e);
                        continue;
                    }
                }
            }
        }

        // 2. Fetch TSC proof (if transaction is confirmed)
        {
            let db = state.database.lock().unwrap();
            let merkle_proof_repo = MerkleProofRepository::new(db.connection());

            if merkle_proof_repo.get_by_parent_txid(&txid)?.is_none() {
                drop(db);

                match cache_helpers::fetch_tsc_proof_from_api(client, &txid).await {
                    Ok(Some(tsc_json)) => {
                        let db = state.database.lock().unwrap();
                        let block_header_repo = BlockHeaderRepository::new(db.connection());
                        let enhanced_tsc = cache_helpers::enhance_tsc_with_height(
                            client,
                            &block_header_repo,
                            &tsc_json,
                        ).await?;

                        // Cache the proof
                        let parent_tx_repo = ParentTransactionRepository::new(db.connection());
                        if let Some(parent_txn_id) = parent_tx_repo.get_id_by_txid(&txid)? {
                            let target_hash = enhanced_tsc["target"].as_str().unwrap_or("");
                            let nodes_json = serde_json::to_string(&enhanced_tsc["nodes"])?;
                            let block_height = enhanced_tsc["height"].as_u64().unwrap_or(0) as u32;
                            let tx_index = enhanced_tsc["index"].as_u64().unwrap_or(0);

                            merkle_proof_repo.upsert(parent_txn_id, block_height, tx_index, target_hash, &nodes_json)?;
                            cached_count += 1;
                            log::debug!("   💾 Cached Merkle proof for {}", txid);
                        }
                    }
                    Ok(None) => {
                        // Transaction not confirmed yet, skip
                    }
                    Err(e) => {
                        log::warn!("   ⚠️  Failed to fetch TSC proof for {}: {}", txid, e);
                    }
                }
            }
        }

        // Rate limiting: small delay between requests
        sleep(Duration::from_millis(100)).await;
    }

    Ok(cached_count)
}
```

### Step 7: Integrate Cache Sync Service

**File**: `rust-wallet/src/main.rs`

```rust
// After database initialization
let state_clone = state.clone();
tokio::spawn(async move {
    crate::cache_sync::start_cache_sync_service(state_clone).await;
});
```

### Step 8: Update Database Module Exports

**File**: `rust-wallet/src/database/mod.rs`

```rust
pub mod parent_transaction_repo;
pub mod merkle_proof_repo;
pub mod block_header_repo;

pub use parent_transaction_repo::*;
pub use merkle_proof_repo::*;
pub use block_header_repo::*;
```

### Step 9: Update lib.rs or main.rs for Cache Errors

**File**: `rust-wallet/src/lib.rs` or `rust-wallet/src/main.rs`

```rust
pub mod cache_errors;
pub mod cache_helpers;
pub mod cache_sync;
```

## Implementation Order (REVISED)

1. ✅ **Step 0**: Schema migration v3 (make utxo_id nullable) - **DO THIS FIRST**
2. ✅ **Step 1**: Create unified error type (`cache_errors.rs`)
3. ✅ **Step 2**: Create repository modules (parent_transaction_repo, merkle_proof_repo, block_header_repo)
4. ✅ **Step 3**: Create model structs (add to models.rs)
5. ✅ **Step 4**: Create helper functions (`cache_helpers.rs`)
6. ✅ **Step 5**: Update signAction() to use cache (with proper lock management)
7. ✅ **Step 6**: Create background cache sync service
8. ✅ **Step 7**: Integrate cache sync service in main.rs
9. ✅ **Step 8**: Update module exports
10. ⏸️ **Step 9**: Optional - cache during UTXO sync
11. ✅ **Step 10**: Testing
12. ✅ **Step 11**: Error handling improvements

## Expected Performance Improvements

**Before (Current):**
- 3 parent transactions: ~600-1500ms (3 API calls × 3 parents)
- Sequential API calls block signing process

**After (Cached):**
- 3 parent transactions: ~5-10ms (database lookups only)
- Background service pre-populates cache
- API calls only on cache miss
- Lock released between operations (no blocking)

## Success Criteria

- ✅ `signAction()` checks cache before making API calls
- ✅ Cache hit rate > 80% after warm-up period
- ✅ BEEF building time reduced by > 90% for cached transactions
- ✅ Background service successfully pre-populates cache
- ✅ Graceful fallback to API on cache miss
- ✅ No transaction failures due to cache issues
- ✅ Database locks released promptly (no blocking)
- ✅ External parent transactions cached correctly (utxo_id = NULL)

## Future Enhancements (Post-Phase 6)

- **Cache TTL**: Add expiration for cached data
- **Reorg Detection**: Invalidate cache on blockchain reorganization
- **Compression**: Compress cached raw transaction bytes
- **Metrics**: Track cache hit/miss rates
- **Prefetching**: More aggressive pre-fetching strategies
- **Parallel API Calls**: Fetch multiple parent transactions in parallel on cache miss
