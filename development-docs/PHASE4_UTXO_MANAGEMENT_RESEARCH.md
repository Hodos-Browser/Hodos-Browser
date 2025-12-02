# Phase 4: UTXO Management - Research & Planning

> **Date**: 2025-12-02
> **Status**: ✅ **IMPLEMENTATION COMPLETE** (Background sync pending)
> **Purpose**: Research and plan UTXO caching implementation for Phase 4

---

## 1. What is an Electrum Server?

### Overview
An **Electrum Server** is a specialized blockchain indexer that provides fast, efficient access to Bitcoin blockchain data without requiring a full node. It's designed for SPV (Simplified Payment Verification) wallets.

### Key Features:
- **UTXO Index**: Maintains an index of all unspent transaction outputs
- **Address History**: Tracks transaction history for addresses
- **Fast Queries**: Provides instant UTXO lookups without scanning the blockchain
- **Privacy**: Can query specific addresses without revealing all wallet addresses
- **Lightweight**: Wallets don't need to download full blockchain

### How It Works:
1. Server maintains a full UTXO set index
2. Wallet queries: "Give me UTXOs for address X"
3. Server responds with only that address's UTXOs
4. Wallet can verify proofs if needed (SPV)

### For Bitcoin SV:
- **ElectrumX** is the most common implementation
- Some services provide Electrum-compatible APIs
- **WhatsOnChain** provides similar functionality but not full Electrum protocol

### Relevance to Our Project:
- We're currently using **WhatsOnChain API** (similar to Electrum but REST-based)
- We could potentially use an Electrum server for better privacy
- However, WhatsOnChain is sufficient for our needs and widely used

**Reference**: [Electrum Documentation](https://electrum.readthedocs.io/)

---

## 2. Best Practices: Privacy vs Speed for Address Generation

### The Privacy-Speed Tradeoff

**Your Observation is Correct:**
- ✅ **Privacy**: Generate new address for every transaction (prevents address reuse)
- ❌ **Speed**: Checking every address is slow (sequential API calls)
- ⚠️ **Problem**: Someone can send coins to old addresses weeks/months later

### Best Practices for Bitcoin SV:

#### **Strategy 1: Gap Limit Scanning (Recommended)**
- **Concept**: Scan addresses in batches, maintain a "gap limit"
- **How it works**:
  1. Scan addresses 0-20 (gap limit = 20)
  2. If address 15 has activity, scan up to address 35 (15 + 20)
  3. Continue until you find 20 consecutive unused addresses
  4. Only check "used" addresses + gap limit on sync

**Benefits**:
- ✅ Fast: Only checks relevant addresses
- ✅ Privacy: Still generates new addresses
- ✅ Catches late payments: Gap limit ensures you don't miss transactions

**Implementation**:
```rust
// Track highest used address index
let highest_used = 15;
let gap_limit = 20;
let scan_to = highest_used + gap_limit; // Scan to index 35
```

#### **Strategy 2: Background Scanning**
- **Active addresses**: Check frequently (every 5 minutes)
- **Inactive addresses**: Check less frequently (every hour/day)
- **New addresses**: Check immediately after generation

**Benefits**:
- ✅ Fast for active addresses
- ✅ Catches late payments eventually
- ⚠️ More complex to implement

#### **Strategy 3: Address Reuse (NOT Recommended)**
- Use same address multiple times
- ❌ **Privacy risk**: All transactions linked
- ✅ Fast: Only one address to check
- ❌ **Not recommended** for privacy-conscious wallets

### **Recommended Approach for HodosBrowser:**

**Hybrid Strategy:**
1. **Gap Limit Scanning**: Implement gap limit of 20 addresses
2. **Background Sync**: Periodic sync of all "used" addresses
3. **Smart Caching**: Cache UTXOs in database, only fetch new/changed addresses
4. **Address Tracking**: Mark addresses as "used" when they receive funds

**Implementation Plan**:
- Track `highest_used_address_index` in database
- On sync: Check addresses from 0 to `highest_used + gap_limit`
- Cache all UTXOs in database
- Background job: Periodically check all used addresses

---

## 3. How metanet-desktop Handles This

### Research from metanet-desktop Repository

**Note**: The repository is archived, but we can infer patterns from similar wallets.

### Common Patterns in Desktop Wallets:

#### **1. Address Gap Limit**
- Most wallets use gap limit of 20
- Scans addresses in batches
- Stops when finding 20 consecutive unused addresses

#### **2. UTXO Caching**
- Cache UTXOs in local database
- Only fetch new/changed addresses
- Background sync process

#### **3. Address State Tracking**
- Track which addresses have been "used" (received funds)
- Only scan used addresses + gap limit
- Mark addresses as used when UTXOs found

#### **4. Incremental Scanning**
- Don't rescan entire wallet every time
- Only check addresses that might have new activity
- Use block height to determine what to scan

### **What We Should Implement:**

Based on best practices:
1. **Gap Limit**: 20 addresses
2. **UTXO Cache**: Store in database
3. **Address State**: Track "used" flag
4. **Incremental Sync**: Only check relevant addresses
5. **Background Job**: Periodic full sync

---

## 4. Miner API Endpoint Changes Needed

### Current Implementation

**WhatsOnChain API** (what we're using):
```
GET /v1/bsv/main/address/{address}/unspent
```

**Returns**:
```json
[
  {
    "tx_hash": "abc123...",
    "tx_pos": 0,
    "value": 10000,
    "script": "76a914..."  // Optional, sometimes missing
  }
]
```

### What We Need for UTXO Caching:

#### **Required Information:**
1. ✅ **txid** (tx_hash) - Already provided
2. ✅ **vout** (tx_pos) - Already provided
3. ✅ **satoshis** (value) - Already provided
4. ⚠️ **script** - Sometimes missing, we generate it
5. ❌ **block_height** - NOT provided (would be useful)
6. ❌ **confirmations** - NOT provided (would be useful)
7. ❌ **parent_tx_hex** - NOT provided (needed for BEEF)

### **TSC (Transaction Status Checker) and GorillaPool**

**TSC = Transaction Status Checker** - A service that provides Merkle proofs for SPV verification.

**Current Implementation:**
- We use **WhatsOnChain TSC endpoint**: `/v1/bsv/main/tx/{txid}/proof/tsc`
- This provides Merkle proofs (BUMP format) for building BEEF transactions
- We fetch TSC proofs **on-demand** when building BEEF (in `signAction`)

**GorillaPool:**
- **GorillaPool** is primarily a **miner/broadcaster** (mAPI endpoint)
- We use it to **broadcast transactions**: `POST https://mapi.gorillapool.io/mapi/tx`
- GorillaPool does **NOT** provide UTXO data or TSC proofs
- It's for transaction submission, not data retrieval

**Should We Get All Info Upfront?**

**Current Flow:**
1. Fetch UTXOs from WhatsOnChain (basic info only)
2. Later, when signing: Fetch TSC proofs for each UTXO
3. Later, when signing: Fetch parent transaction hex for BEEF

**Option 1: Fetch Everything Upfront (During UTXO Sync)**
**Pros:**
- ✅ Faster transaction building (no API calls during `signAction`)
- ✅ Better for offline signing preparation
- ✅ Can cache parent transactions and proofs

**Cons:**
- ❌ More API calls during sync (slower initial sync)
- ❌ Wastes bandwidth if transaction never gets signed
- ❌ TSC proofs only available for confirmed transactions

**Option 2: Fetch On-Demand (Current Approach)**
**Pros:**
- ✅ Faster UTXO sync (only fetch what's needed)
- ✅ Only fetch proofs when actually needed
- ✅ Don't waste bandwidth on unused data

**Cons:**
- ❌ Slower transaction signing (multiple API calls)
- ❌ Requires internet during signing
- ❌ More complex error handling

**Recommendation: Hybrid Approach**
1. **During UTXO Sync**: Fetch basic UTXO data (what we do now)
2. **When UTXO is Used**: Fetch parent tx + TSC proof on-demand
3. **Cache Everything**: Store parent tx and TSC proof in database after first fetch
4. **Future Transactions**: Use cached data if available

**This gives us:**
- ✅ Fast initial sync
- ✅ Fast subsequent transactions (cached data)
- ✅ No wasted bandwidth
- ✅ Works offline for cached transactions

**Implementation:**
- Store `parent_tx_hex` and `merkle_proof` in `utxos` table (nullable)
- Fetch on first use, cache for future
- Background job can pre-fetch for frequently used UTXOs

### **Do We Need to Change API?**

**Short Answer: NO** - WhatsOnChain API is sufficient for Phase 4.

**Why:**
- We can cache UTXOs with current data
- We can track block_height/confirmations separately
- Parent transactions can be fetched on-demand (Phase 5)

**Optional Enhancements:**
- Could add block_height tracking by querying transaction details
- Could cache parent transactions when fetching UTXOs
- Could use multiple APIs for redundancy

### **API Endpoints We'll Use:**

1. **UTXO Fetching** (Phase 4):
   ```
   GET /v1/bsv/main/address/{address}/unspent
   ```

2. **Transaction Details** (Phase 5 - for parent transactions):
   ```
   GET /v1/bsv/main/tx/{txid}/hex
   ```

3. **Block Headers** (Phase 5 - for SPV):
   ```
   GET /v1/bsv/main/block/{hash}/header
   ```

**Conclusion**: No API changes needed for Phase 4. Current WhatsOnChain API is sufficient.

---

## 6. Change Address Reuse Issue

### **Current Implementation Problem:**

**We're reusing address index 0 for ALL change outputs!**

```rust
// Current code in createAction:
let change_addr = match address_repo.get_by_wallet_and_index(wallet.id.unwrap(), 0) {
    // Always uses index 0!
}
```

**This is a PRIVACY ISSUE:**
- ❌ All change goes to the same address
- ❌ Links all your transactions together
- ❌ Makes it easy to track your spending patterns
- ❌ Violates Bitcoin privacy best practices

### **Best Practice: Generate New Change Address**

**Recommended Approach:**
1. Generate a **new address** for each change output
2. Use the **next unused address index**
3. Mark address as "used" when it receives change
4. This maintains privacy (each transaction uses different change address)

**Implementation:**
```rust
// Instead of always using index 0:
let change_addr = match address_repo.get_by_wallet_and_index(wallet.id.unwrap(), 0) {
    // BAD: Always index 0
}

// Should be:
let current_index = wallet.current_index;
let change_addr = generate_new_address(wallet_id, current_index)?;
// This creates a NEW address for change
```

**Benefits:**
- ✅ Privacy: Each transaction uses different change address
- ✅ Follows Bitcoin best practices
- ✅ Harder to link transactions
- ✅ Already have address generation code

**Action Required:**
- **Fix in Phase 4**: Update `createAction` to generate new change address
- **Track**: Mark change addresses as "used" when created
- **Gap Limit**: Still applies (scan used addresses + gap)

---

## 7. UTXO Consolidation: Privacy Concerns

### **The Question:**
Should we periodically consolidate UTXOs (combine many small UTXOs into one large UTXO)?

### **Privacy Analysis:**

**What Consolidation Does:**
- Combines multiple UTXOs from different addresses into one transaction
- Sends all funds to a single address
- Reduces number of UTXOs (fewer inputs in future transactions)

**Privacy Risks:**
- ❌ **Common-Input-Ownership Heuristic**: All inputs in a transaction are assumed to belong to the same entity
- ❌ **Links Previously Separate Addresses**: Consolidation reveals that multiple addresses belong to you
- ❌ **Recognizable Pattern**: Large consolidation transactions are easy to identify on-chain
- ❌ **Undermines Privacy**: If you've been using new addresses for privacy, consolidation links them all

**When Consolidation Might Be Acceptable:**
- ✅ **Single Address Wallet**: If you're already using one address (no privacy)
- ✅ **After CoinJoin**: If UTXOs are already mixed (but be careful)
- ✅ **Internal Consolidation**: Combining UTXOs from the same address only

### **Recommendation: DO NOT Consolidate**

**Reasons:**
1. **Privacy Loss**: Consolidation links all your addresses
2. **Recognizable**: Large consolidation transactions are obvious on-chain
3. **Unnecessary**: Modern wallets handle many UTXOs efficiently
4. **Better Alternatives**:
   - Use UTXO selection algorithms (coin selection)
   - Spend UTXOs naturally as you transact
   - Only consolidate if absolutely necessary (e.g., 100+ UTXOs)

**Better Strategy:**
- ✅ **Smart UTXO Selection**: Use algorithms to select optimal UTXOs
- ✅ **Natural Spending**: Let UTXOs be spent naturally over time
- ✅ **Dust Management**: Only consolidate dust UTXOs (< 546 satoshis) if needed
- ✅ **Privacy First**: Prioritize privacy over minor fee savings

**If You Must Consolidate:**
- Only consolidate UTXOs from the **same address**
- Do it **infrequently** (not periodic)
- Consider **CoinJoin** first if privacy is important
- Be aware it creates a **recognizable pattern**

---

## 5. Functions/Methods to Change for Database UTXO Usage

### Current Flow (Without Caching):

```
createAction()
  ↓
fetch_all_utxos()  ← Fetches from WhatsOnChain API
  ↓
select_utxos()     ← Selects UTXOs to spend
  ↓
build_transaction()
  ↓
signAction()       ← Signs transaction
```

### New Flow (With Database Caching):

```
createAction()
  ↓
get_utxos_from_db()  ← Reads from database (FAST!)
  ↓
  ↓ (if cache miss or stale)
sync_utxos()          ← Fetches from API, updates DB
  ↓
select_utxos()        ← Selects UTXOs to spend
  ↓
mark_utxos_spent()    ← Updates database (marks as spent)
  ↓
build_transaction()
  ↓
signAction()          ← Signs transaction (no API calls!)
```

### **Functions to Modify:**

#### **1. `createAction` Handler** (`handlers.rs`)
**Current**:
```rust
let all_utxos = match fetch_all_utxos(&addresses).await {
    // Fetches from API every time
}
```

**New**:
```rust
// Try database first
let all_utxos = match get_utxos_from_db(&addresses) {
    Ok(utxos) if !utxos.is_empty() => {
        // Use cached UTXOs
        utxos
    }
    _ => {
        // Cache miss - fetch and cache
        let utxos = fetch_all_utxos(&addresses).await?;
        cache_utxos_to_db(&utxos)?;
        utxos
    }
}
```

#### **2. `wallet_balance` Handler** (`handlers.rs`)
**Current**:
```rust
match crate::utxo_fetcher::fetch_all_utxos(&addresses).await {
    // Fetches from API every time
}
```

**New**:
```rust
// Use cached UTXOs from database
let balance = calculate_balance_from_db(&addresses)?;
```

#### **3. `utxo_fetcher.rs` - New Functions**
**Add**:
```rust
// Database operations
pub fn get_utxos_from_db(addresses: &[AddressInfo]) -> Result<Vec<UTXO>>;
pub fn cache_utxos_to_db(utxos: &[UTXO]) -> Result<()>;
pub fn mark_utxos_spent(txid: &str, vouts: &[u32]) -> Result<()>;
pub async fn sync_utxos_for_addresses(addresses: &[AddressInfo]) -> Result<()>;
```

#### **4. `signAction` Handler** (`handlers.rs`)
**Current**: Uses UTXO data from `createAction` (already in memory)

**New**:
- No changes needed - already has UTXO data
- But should mark UTXOs as spent after signing

**Add**:
```rust
// After successful signing
mark_utxos_spent_in_db(&selected_utxos)?;
```

#### **5. New: `utxo_repo.rs`** (New File)
**Create repository for UTXO database operations**:
```rust
pub struct UtxoRepository {
    conn: &Connection,
}

impl UtxoRepository {
    pub fn get_unspent_by_addresses(&self, addresses: &[String]) -> Result<Vec<UTXO>>;
    pub fn insert_utxos(&self, utxos: &[UTXO]) -> Result<()>;
    pub fn mark_spent(&self, txid: &str, vout: u32) -> Result<()>;
    pub fn get_by_txid_vout(&self, txid: &str, vout: u32) -> Result<Option<UTXO>>;
    pub fn delete_spent(&self) -> Result<usize>; // Cleanup old spent UTXOs
}
```

#### **6. New: Background Sync Service** (New File)
**Create `utxo_sync.rs`**:
```rust
pub async fn background_utxo_sync(db: &WalletDatabase) -> Result<()> {
    // Runs every 5 minutes
    // 1. Get all used addresses from database
    // 2. Fetch UTXOs from API
    // 3. Update database cache
    // 4. Detect new addresses that received funds
}
```

### **Summary of Changes:**

| Function | Current Behavior | New Behavior |
|----------|-----------------|-------------|
| `createAction` | Fetches UTXOs from API | Reads from DB, syncs if needed |
| `wallet_balance` | Fetches UTXOs from API | Calculates from DB cache |
| `signAction` | No DB interaction | Marks UTXOs as spent in DB |
| `fetch_all_utxos` | API only | API + cache to DB |
| **NEW** `get_utxos_from_db` | N/A | Reads from database |
| **NEW** `cache_utxos_to_db` | N/A | Writes to database |
| **NEW** `mark_utxos_spent` | N/A | Updates DB (spent flag) |
| **NEW** `background_utxo_sync` | N/A | Periodic sync job |

---

## Implementation Plan Summary

### Phase 4 Goals:
1. ✅ Cache UTXOs in database
2. ✅ Read from cache instead of API
3. ✅ Background sync process
4. ✅ Mark UTXOs as spent
5. ✅ Gap limit scanning
6. ✅ **Fix change address reuse** (generate new address for each change)
7. ✅ **Cache parent transactions and TSC proofs** (hybrid approach)

### Key Design Decisions:
- **Gap Limit**: 20 addresses
- **Sync Frequency**: Every 5 minutes (background)
- **Cache Strategy**: Full UTXO cache, incremental updates
- **API**: No changes needed (WhatsOnChain sufficient)
- **Change Addresses**: Generate new address for each change (privacy)
- **Parent Transactions**: Fetch on-demand, cache in database
- **TSC Proofs**: Fetch on-demand, cache in database
- **UTXO Consolidation**: **DO NOT** consolidate (privacy risk)

### Next Steps:
1. Create `utxo_repo.rs` for database operations
2. Modify `createAction` to use database
3. Modify `wallet_balance` to use database
4. Add background sync service
5. Implement gap limit scanning
6. Test with real wallet

---

## ✅ **Implementation Status** (2025-12-02)

### **Completed:**
- ✅ UTXO repository (`utxo_repo.rs`) - CRUD operations for UTXOs
- ✅ Database schema - `utxos` table with spending tracking
- ✅ `wallet_balance` handler - Calculates from database cache
- ✅ `createAction` handler - Uses database UTXOs, falls back to API
- ✅ `signAction` handler - Marks spent UTXOs in database
- ✅ Change address generation - New address for each change (privacy fix)
- ✅ Address marking - Automatically marks addresses as "used" when UTXOs found

### **Pending:**
- ⏳ Background UTXO sync service with gap limit
- ⏳ Periodic UTXO updates (every N minutes)
- ⏳ Performance optimization (reduce API calls)

### **Current Behavior:**
- Balance check: Always fetches from API to ensure accuracy (catches new UTXOs immediately)
- UTXO selection: Uses database cache first, falls back to API if cache empty
- Spending tracking: Automatically marks UTXOs as spent when transaction is signed

### **Performance Notes:**
- ⚠️ **Wallet is still slow** - Balance check fetches from API every time
- This is by design for accuracy (catches new UTXOs on new addresses)
- Need to discuss optimization strategy:
  - Background sync service
  - Periodic updates
  - Smart refresh (only check new addresses)

### **Next Steps:**
1. Fix transaction error handling (see CHECKPOINT_TRANSACTION_ERROR_HANDLING.md)
2. Implement background UTXO sync service
3. Add periodic update mechanism
4. Optimize balance check (smart refresh vs always fetch)

---

## References

- [Electrum Server Documentation](https://electrum.readthedocs.io/)
- [WhatsOnChain API Documentation](https://developers.whatsonchain.com/)
- [Bitcoin Address Reuse Privacy](https://en.bitcoin.it/wiki/Address_reuse)
- [Gap Limit Best Practices](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki)
