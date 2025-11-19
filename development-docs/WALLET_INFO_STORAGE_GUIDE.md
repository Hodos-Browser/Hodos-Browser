# Wallet Info Storage Guide

> **Status**: Planning Phase - Initial Analysis
> **Last Updated**: 2025-11-19

## Executive Summary

This document analyzes how HodosWallet currently manages wallet data and outlines the plan to migrate from JSON file storage to a database-backed system with proactive UTXO and BEEF/SPV data caching.

## Current Implementation Analysis

### 1. UTXO Fetching (Current State)

**Location**: `rust-wallet/src/utxo_fetcher.rs`

**Current Flow**:
```
createAction() → fetch_all_utxos() → For each address:
  → HTTP GET WhatsOnChain /address/{address}/unspent
  → Parse JSON response
  → Generate P2PKH scripts
  → Return UTXO list
```

**API Calls Made**:
- `/v1/bsv/main/address/{address}/unspent` - For each address (sequential)

**Performance Impact**:
- ⚠️ **Slow**: Multiple sequential API calls (one per address)
- ⚠️ **Network-dependent**: Fails if API is down
- ⚠️ **No caching**: Fetches every time, even if UTXOs unchanged
- ⚠️ **Rate limiting risk**: Multiple requests for wallet with many addresses

**UTXO Data Structure**:
```rust
pub struct UTXO {
    pub txid: String,        // Parent transaction ID
    pub vout: u32,           // Output index
    pub satoshis: i64,       // Amount
    pub script: String,      // Hex-encoded locking script
    pub address_index: u32,  // Which HD address owns this
}
```

### 2. BEEF/SPV Transaction Building (Current State)

**Location**: `rust-wallet/src/handlers.rs` - `signAction()` function (lines 2563-2782)

**Current Flow**:
```
signAction() → For each input UTXO:
  → HTTP GET /tx/{txid}/hex (parent transaction)
  → HTTP GET /tx/{txid}/proof/tsc (TSC Merkle proof)
  → HTTP GET /block/hash/{hash} (block header for height)
  → Convert TSC to BUMP format
  → Build BEEF with parent transactions + BUMPs
```

**API Calls Made Per Transaction**:
- `/v1/bsv/main/tx/{txid}/hex` - Parent transaction (1 per input)
- `/v1/bsv/main/tx/{txid}/proof/tsc` - TSC Merkle proof (1 per input)
- `/v1/bsv/main/block/hash/{hash}` - Block header for height (1 per input)

**Example**: A transaction with 3 inputs = **9 API calls** just to build BEEF!

**Performance Impact**:
- ⚠️ **Very Slow**: Multiple sequential API calls (blocking)
- ⚠️ **Retry logic needed**: Handles null proofs with 2-second delays
- ⚠️ **No caching**: Fetches same parent transactions repeatedly
- ⚠️ **Time-sensitive**: User waits for all API calls to complete

**Data Needed for BEEF**:
1. **Parent Transaction**: Raw transaction bytes (hex)
2. **TSC Merkle Proof**:
   - `height`: Block height (u32)
   - `index`: Transaction index in block (u64)
   - `target`: Block hash (String)
   - `nodes`: Merkle path nodes (Array of hex strings)
3. **Block Header** (for height resolution):
   - `height`: Block height (u32)

### 3. Current Data Storage

**JSON Files in `%APPDATA%/HodosBrowser/wallet/`**:

#### wallet.json
```json
{
  "mnemonic": "...",
  "addresses": [
    {
      "index": 0,
      "address": "...",
      "publicKey": "...",
      "used": false,
      "balance": 0
    }
  ],
  "currentIndex": 0,
  "backedUp": false
}
```

#### actions.json
```json
{
  "actions": {
    "txid1": {
      "txid": "...",
      "referenceNumber": "...",
      "rawTx": "...",
      "status": "confirmed",
      "inputs": [...],
      "outputs": [...],
      ...
    }
  }
}
```

**What's Missing**:
- ❌ No UTXO storage
- ❌ No parent transaction storage
- ❌ No Merkle proof storage
- ❌ No relationship tracking (which UTXOs spent, which received)

## Proposed Database Schema (Initial Outline)

### Tables Needed

1. **Addresses Table**
   - Store HD wallet addresses (currently in wallet.json)
   - Index: address, index

2. **UTXOs Table**
   - Store all unspent transaction outputs
   - Link to addresses
   - Include locking script, amount, confirmation status
   - Index: (txid, vout), address_id

3. **Parent Transactions Table**
   - Cache parent transaction raw bytes
   - Link to UTXOs that reference them
   - Index: txid

4. **Merkle Proofs Table**
   - Store TSC/BUMP Merkle proofs
   - Link to parent transactions
   - Include block height, tx_index, merkle path
   - Index: txid, block_height

5. **Transaction History Table**
   - Migrate from actions.json
   - Link to UTXOs (inputs/outputs)
   - Index: txid, timestamp

6. **Block Headers Cache**
   - Cache block header data for height resolution
   - Index: block_hash

## Implementation Steps (Broad Outline)

### Step 1: Database Setup
- [ ] Choose database solution (SQLite recommended for start)
- [ ] Design complete schema
- [ ] Create database connection layer
- [ ] Implement migration scripts

### Step 2: Data Migration
- [ ] Migrate wallet.json → Addresses table
- [ ] Migrate actions.json → Transaction History table
- [ ] Create backward compatibility layer (read both JSON and DB)

### Step 3: UTXO Storage & Sync
- [ ] Create UTXO table schema
- [ ] Implement UTXO sync process (background task)
- [ ] Update `createAction()` to use cached UTXOs
- [ ] Implement UTXO invalidation (mark as spent when used)

### Step 4: Parent Transaction Caching
- [ ] Create Parent Transactions table
- [ ] Implement parent tx fetch and cache during UTXO sync
- [ ] Update `signAction()` to use cached parent transactions
- [ ] Handle cache misses gracefully (fallback to API)

### Step 5: Merkle Proof Storage
- [ ] Create Merkle Proofs table
- [ ] Fetch and store TSC proofs during UTXO sync
- [ ] Update `signAction()` to use cached proofs
- [ ] Implement proof refresh logic (for reorgs)

### Step 6: Background Sync Process
- [ ] Implement periodic UTXO refresh (every N minutes)
- [ ] Detect new incoming UTXOs
- [ ] Fetch parent transactions and proofs for new UTXOs
- [ ] Handle blockchain reorgs (invalidate old data)

### Step 7: Performance Optimization
- [ ] Add database indexes
- [ ] Optimize queries
- [ ] Implement connection pooling
- [ ] Batch operations where possible

### Step 8: Cleanup
- [ ] Remove JSON file dependencies
- [ ] Remove API fallback code (or keep as backup)
- [ ] Performance testing
- [ ] Documentation

## Planning Questions to Answer

### Architecture Questions

1. **Database Choice**:
   - SQLite vs PostgreSQL vs embedded KV store?
   - What's the expected maximum number of addresses?
   - What's the expected maximum number of UTXOs?
   - Single-user or multi-user in future?

2. **Sync Strategy**:
   - How often should we sync UTXOs? (every 5 min? 1 min? on-demand?)
   - Should sync be background thread or separate process?
   - How to detect new UTXOs efficiently?
   - Should we sync all addresses or only used ones?

3. **BEEF/SPV Data**:
   - When to fetch parent transactions? (immediately on UTXO creation? lazy?)
   - When to fetch Merkle proofs? (same time as parent tx? separate pass?)
   - How to handle unconfirmed parent transactions? (no proof available yet)
   - Should we cache block headers too? (for height resolution)

4. **Data Integrity**:
   - How to detect when a UTXO is spent? (poll? webhook? confirmation status?)
   - How to handle blockchain reorgs? (invalidate proofs? full resync?)
   - How to verify Merkle proofs are still valid?
   - Should we store multiple versions of proofs (for reorgs)?

5. **Performance**:
   - What indexes are critical?
   - How large are Merkle proofs typically? (affects storage)
   - Should we compress stored data? (parent transactions, proofs)
   - Query patterns: most common operations?

6. **Migration Path**:
   - How to migrate existing JSON data without losing anything?
   - Should we keep JSON files as backup during transition?
   - How to handle users with existing wallets?
   - Rollback strategy if database migration fails?

7. **Storage Location**:
   - Keep in same directory? (`%APPDATA%/HodosBrowser/wallet/`)
   - Separate database file? (wallet.db)
   - Or database directory? (wallet_db/)
   - Backup strategy?

8. **Error Handling**:
   - What if database is corrupted?
   - What if API fails during sync?
   - Fallback to JSON? Fallback to API fetches?
   - Recovery procedures?

9. **Concurrency**:
   - How to handle simultaneous transaction creation?
   - Locking strategy for UTXO selection?
   - Transaction isolation levels needed?

10. **Monitoring & Debugging**:
    - How to track sync status?
    - How to detect sync failures?
    - Logging strategy for database operations?
    - Debug tools for inspecting stored data?

---

**Next Steps**:
1. Answer planning questions through research and design discussions
2. Review similar wallet implementations (Electrum, MetaMask backend patterns)
3. Design complete database schema
4. Create detailed implementation plan with timelines
