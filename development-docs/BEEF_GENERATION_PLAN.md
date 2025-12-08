# BEEF Generation for listOutputs - Implementation Plan

## Overview
Implement BEEF generation for `listOutputs` when `include='entire transactions'` is requested.

**Format**: Standard BEEF V2 (BRC-62/BRC-96), NOT Atomic BEEF (BRC-95)
- See [BEEF_FORMATS_SUMMARY.md](./BEEF_FORMATS_SUMMARY.md) for format differences
- References: [BRC-62](https://bsv.brc.dev/transactions/0062), [BRC-95](https://bsv.brc.dev/transactions/0095), [BRC-96](https://bsv.brc.dev/transactions/0096)

## Key Findings

### ✅ What We Have:
1. **Transaction Parser**: `ParsedTransaction::from_bytes()` exists in `beef.rs` - can parse raw transactions and extract inputs
2. **BEEF Building**: All BEEF methods exist (`add_parent_transaction()`, `add_tsc_merkle_proof()`, `to_bytes()`)
3. **Caching**: Parent transaction and Merkle proof caching infrastructure exists
4. **API Fetching**: Functions to fetch from WhatsOnChain API exist

### ⏳ What We Need:
1. **`find_txid()` method**: Need to check if transaction already in BEEF (or implement it)
2. **Unified transaction fetcher**: Function that checks `transactions` table, then `parent_transactions`, then API
3. **Recursive BEEF builder**: Function that recursively fetches parent transactions
4. **Integration**: Wire it into `list_outputs()` handler

## Comparison: signAction vs listOutputs BEEF

### Current Implementation (signAction)
**Context**: We're creating a NEW transaction and need to include its parent transactions.

**Process**:
1. ✅ We have input UTXOs (we know their `txid`s)
2. ✅ For each input UTXO, fetch parent transaction:
   - Check cache (`parent_transactions` table)
   - If not cached, fetch from API (WhatsOnChain)
   - Verify TXID matches
   - Cache for future use
3. ✅ Add parent transactions to BEEF using `beef.add_parent_transaction()`
4. ✅ Fetch Merkle proofs (TSC format) for parent transactions:
   - Check cache (`merkle_proofs` table)
   - If not cached, fetch from API
   - Convert TSC to BUMP format
   - Add to BEEF using `beef.add_tsc_merkle_proof()`
5. ✅ Add signed transaction as main transaction using `beef.set_main_transaction()`
6. ✅ Serialize to Atomic BEEF (BRC-95) using `beef.to_atomic_beef_hex()`

**Key Points**:
- We know the inputs upfront (from `createAction`)
- Parent transactions are fetched directly (one level deep)
- Main transaction is the one we just created

### Required Implementation (listOutputs)
**Context**: We're listing EXISTING outputs and need to include their transactions + ancestry.

**Process** (from ts-brc100 reference):
1. ⏳ We have output UTXOs (we know their `txid`s - these are the transactions that created the outputs)
2. ⏳ For each output's transaction:
   - Fetch transaction by `txid` (from `transactions` table or API)
   - Parse transaction to get its inputs
   - For each input, recursively fetch parent transactions
   - Merge all transactions into a single BEEF
3. ⏳ Fetch Merkle proofs for all transactions (if available)
4. ⏳ Serialize to standard BEEF (NOT Atomic - listOutputs returns standard BEEF)

**Key Differences**:
- We need to fetch the transaction that created each output (not just parent transactions)
- We need to recursively fetch parent transactions (multiple levels deep)
- Multiple outputs may share the same transaction (need to deduplicate)
- Returns standard BEEF (not Atomic BEEF)

## Implementation Plan

### Phase 1: Helper Function - Fetch Transaction by TXID
**Location**: `rust-wallet/src/cache_helpers.rs` or new `rust-wallet/src/beef_helpers.rs`

**Function**: `fetch_transaction_for_beef(txid: &str) -> Result<Vec<u8>, String>`
- Check `transactions` table for `raw_tx`
- If not found, check `parent_transactions` table
- If not found, fetch from API (WhatsOnChain)
- Verify TXID matches
- Return raw transaction bytes

### Phase 2: Helper Function - Recursively Build BEEF for Transaction
**Location**: `rust-wallet/src/beef_helpers.rs` (new file)

**Function**: `build_beef_for_txid(txid: &str, beef: &mut Beef, known_txids: &mut HashSet<String>) -> Result<(), String>`
- Check if transaction already in BEEF (`beef.find_txid()`)
- If not, fetch transaction
- Parse transaction to get inputs
- Add transaction to BEEF
- For each input, recursively call `build_beef_for_txid()` for parent
- Fetch Merkle proof if available
- Add Merkle proof to BEEF

**Algorithm** (from ts-brc100, adapted for Rust):
```rust
async fn build_beef_for_txid(
    txid: &str,
    beef: &mut Beef,
    known_txids: &mut HashSet<String>,
    db: &WalletDatabase,
    client: &reqwest::Client,
) -> Result<(), String> {
    // Check if already in BEEF (deduplication)
    if beef.find_txid(txid).is_some() {
        return Ok(()); // Already added
    }

    // Fetch transaction (from cache or API)
    let tx_bytes = fetch_transaction_for_beef(txid, db, client).await?;

    // Parse transaction to get inputs
    let parsed = crate::beef::ParsedTransaction::from_bytes(&tx_bytes)?;

    // Add transaction to BEEF (as parent, not main)
    let tx_index = beef.add_parent_transaction(tx_bytes.clone());

    // Fetch Merkle proof if available (similar to signAction logic)
    // Check cache first, then API
    // Add proof to BEEF using beef.add_tsc_merkle_proof()

    // Recursively fetch parent transactions
    for input in parsed.inputs {
        let parent_txid = input.prev_txid;
        // Recursively build BEEF for parent
        build_beef_for_txid(&parent_txid, beef, known_txids, db, client).await?;
    }

    Ok(())
}
```

### Phase 3: Integrate into listOutputs
**Location**: `rust-wallet/src/handlers.rs` - `list_outputs()` function

**Changes**:
1. Change `beef` back to `mut` (we'll be modifying it)
2. For each output in `paginated_utxos`:
   - If `include_transactions` is true:
     - Check if transaction already in BEEF (`beef.find_txid()`)
     - If not, call `build_beef_for_txid()` for the output's `txid`
3. After loop, serialize BEEF:
   - Use `beef.to_bytes()` (standard BEEF, not Atomic)
   - Return as hex string in response

### Phase 4: Transaction Parsing ✅ **SOLVED**
**Solution**: Use existing `ParsedTransaction::from_bytes()` from `rust-wallet/src/beef.rs`

**Found**: We already have `ParsedTransaction` struct with:
- `ParsedTransaction::from_bytes(bytes: &[u8])` - Parses raw transaction
- `ParsedInput` struct with `prev_txid: String` and `prev_vout: u32`
- This is exactly what we need to extract input TXIDs for recursive fetching

**Usage**:
```rust
let parsed = crate::beef::ParsedTransaction::from_bytes(&tx_bytes)?;
for input in parsed.inputs {
    let parent_txid = input.prev_txid;  // This is what we need!
    // Recursively fetch parent...
}
```

### Phase 5: Deduplication
**Issue**: Multiple outputs may be from the same transaction

**Solution**:
- Use `beef.find_txid()` to check if transaction already added
- Only fetch once per unique `txid`

## Dependencies

### Existing Code We Can Reuse:
- ✅ `Beef::new()` - Create empty BEEF
- ✅ `beef.add_parent_transaction()` - Add transaction
- ✅ `beef.add_tsc_merkle_proof()` - Add Merkle proof
- ✅ `beef.to_bytes()` - Serialize to standard BEEF
- ⏳ `beef.find_txid()` - Check if transaction already in BEEF (need to implement - see below)
- ✅ `ParentTransactionRepository` - Cache parent transactions
- ✅ `MerkleProofRepository` - Cache Merkle proofs
- ✅ `TransactionRepository::get_by_txid()` - Get transaction from database
- ✅ `cache_helpers::fetch_parent_transaction_from_api()` - Fetch from API
- ✅ `cache_helpers::fetch_tsc_proof_from_api()` - Fetch Merkle proof from API

### New Code Needed:
- ⏳ `fetch_transaction_for_beef()` - Unified transaction fetcher
- ⏳ `build_beef_for_txid()` - Recursive BEEF builder
- ✅ Transaction parser - **FOUND**: `ParsedTransaction::from_bytes()` exists!
- ⏳ Integration into `list_outputs()` handler

## Implementation Steps

### Step 0: Add `find_txid()` method to Beef
**Location**: `rust-wallet/src/beef.rs`

**Function**: `pub fn find_txid(&self, txid: &str) -> Option<usize>`
- Iterate through `self.transactions`
- For each transaction, calculate its TXID (double SHA-256, reversed)
- Compare with requested `txid`
- Return `Some(index)` if found, `None` otherwise

**Why**: Needed for deduplication - check if transaction already in BEEF before fetching

### Step 1: Create `beef_helpers.rs` module
**Location**: `rust-wallet/src/beef_helpers.rs` (new file)

**Functions**:
1. `fetch_transaction_for_beef(txid: &str, db: &WalletDatabase, client: &Client) -> Result<Vec<u8>, String>`
   - Check `transactions` table for `raw_tx` (by `txid`)
   - If not found, check `parent_transactions` table
   - If not found, fetch from API using `cache_helpers::fetch_parent_transaction_from_api()`
   - Verify TXID matches
   - Return raw transaction bytes

2. `build_beef_for_txid(txid: &str, beef: &mut Beef, db: &WalletDatabase, client: &Client) -> Result<(), String>`
   - Check if already in BEEF using `beef.find_txid()`
   - Fetch transaction using `fetch_transaction_for_beef()`
   - Parse using `ParsedTransaction::from_bytes()`
   - Add to BEEF using `beef.add_parent_transaction()`
   - Fetch Merkle proof (check cache, then API)
   - Add Merkle proof using `beef.add_tsc_merkle_proof()`
   - Recursively call `build_beef_for_txid()` for each input's parent TXID

### Step 2: Update `list_outputs()` handler
**Location**: `rust-wallet/src/handlers.rs`

**Changes**:
1. Change `beef` back to `mut` (we'll be modifying it)
2. For each output in `paginated_utxos`:
   - If `include_transactions` is true:
     - Check if transaction already in BEEF (`beef.find_txid(&utxo.txid)`)
     - If not, call `beef_helpers::build_beef_for_txid()` for the output's `txid`
3. After loop, serialize BEEF:
   - Use `beef.to_bytes()` (standard BEEF, not Atomic)
   - Convert to hex string
   - Return in response `BEEF` field

### Step 3: Test with real data
- Test with single output
- Test with multiple outputs from same transaction (deduplication)
- Test with outputs from different transactions
- Test with recursive parent transactions (multi-level)

## BEEF Format Differences (from BRC specs)

### Standard BEEF (BRC-62)
- **Format**: Version marker (0x0100beef for V1, 0x0200beef for V2) + BUMPs + Transactions
- **Use Case**: General transaction packaging with ancestry and SPV proofs
- **Serialization**: `beef.to_bytes()` → hex string
- **Reference**: [BRC-62](https://bsv.brc.dev/transactions/0062)

### Atomic BEEF (BRC-95)
- **Format**: Magic prefix (0x01010101) + Subject TXID (32 bytes, big-endian) + Standard BEEF
- **Use Case**: Single transaction validation with its ancestry (wraps standard BEEF)
- **Serialization**: `beef.to_atomic_beef_hex(txid)` → hex string
- **Reference**: [BRC-95](https://bsv.brc.dev/transactions/0095)
- **Used in**: `signAction` response (for single transaction validation)

### BEEF V2 (BRC-96)
- **Format**: Version marker 0x0200beef + Enhanced transaction format
- **Enhancement**: Supports TXID-only transactions (format byte 0x02) for efficiency
- **Use Case**: When you know a transaction is already known/validated, you can reference it by TXID only
- **Reference**: [BRC-96](https://bsv.brc.dev/transactions/0096)
- **Note**: We use BEEF V2 as default (matches TypeScript SDK)

### Transaction Extended Format (BRC-30)
- **Format**: Extended transaction format with previous output data embedded
- **Use Case**: Broadcast services can validate transactions without UTXO lookups
- **Note**: Different from BEEF - this is an alternative transaction format, not a packaging format
- **Reference**: [BRC-30](https://bsv.brc.dev/transactions/0030)

## Notes

- **Standard BEEF vs Atomic BEEF**:
  - `listOutputs` returns **standard BEEF** (not Atomic) - per BRC-100 spec
  - Use `beef.to_bytes()` not `beef.to_atomic_beef_hex()`
  - Standard BEEF can contain multiple transactions (all outputs' transactions + their parents)
  - Atomic BEEF is for single transaction validation (used in `signAction`)

- **BEEF V2 Support**:
  - We use BEEF V2 (0x0200beef) as default ✅
  - TXID-only transactions (format byte 0x02) are not yet implemented in our parser
  - For now, we'll always include full transactions (format byte 0x00 or 0x01)
  - Future optimization: Use TXID-only for known transactions when `knownTxids` is provided

- **Performance**:
  - Cache transactions and Merkle proofs aggressively
  - Deduplicate transactions (multiple outputs from same tx)
  - Consider async fetching for multiple transactions
  - Use BEEF V2 format byte 0x00 (raw tx without BUMP) when proof not available

- **Error Handling**:
  - If transaction fetch fails, skip that output's BEEF
  - If Merkle proof fetch fails, continue without proof (use format byte 0x00)
  - Log warnings but don't fail entire request

## Reference Implementation

**ts-brc100**: `reference/ts-brc100/src/storage/methods/listOutputsKnex.ts` lines 254-256
```typescript
if (vargs.includeTransactions && !beef.findTxid(o.txid!)) {
  await dsk.getValidBeefForKnownTxid(o.txid!, beef, undefined, vargs.knownTxids, trx)
}
```

**ts-brc100**: `reference/ts-brc100/src/storage/StorageProvider.ts` lines 454-498
- Shows recursive `getValidBeefForTxid()` implementation
- Handles `knownTxids` for deduplication
- Recursively fetches parent transactions
