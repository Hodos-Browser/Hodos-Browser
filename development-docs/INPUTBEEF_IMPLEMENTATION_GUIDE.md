# inputBEEF Implementation Guide

## Status: IMPLEMENTED (Dec 26, 2024)

---

## Executive Summary

The `createAction` endpoint now fully supports the `inputBEEF` field and `inputs` field. This enables collaborative transactions where apps provide their own UTXOs (e.g., ANYONECANPAY signature patterns).

~~The `createAction` endpoint currently **completely ignores the `inputBEEF` field** and is missing the `inputs` field from the request struct. This causes failures when apps provide their own UTXOs for collaborative transactions (e.g., ANYONECANPAY signature patterns).~~

**Root Cause**: Apps like beta.zanaadu.com send:
```json
{
  "inputBEEF": [2, 0, 190, 239, ...],  // BEEF containing source transaction
  "inputs": [{"outpoint": {"txid": "abc...", "vout": 0}, ...}],  // References to UTXOs in inputBEEF
  "outputs": [...]
}
```

The wallet:
1. Doesn't have `inputs` field in `CreateActionRequest` struct
2. Never processes `inputBEEF` even though the field exists
3. Tries to build transaction from scratch using wallet's own UTXOs

---

## What is inputBEEF?

**inputBEEF** is a BRC-62 BEEF (Background Evaluation Extended Format) data structure containing:
- Source transactions for the inputs the caller wants to spend
- Merkle proofs (BUMPs) for SPV verification
- Arranged in dependency order (parents first)

When an app provides `inputBEEF`, it means:
1. The app has UTXOs it wants to spend (not the wallet's UTXOs)
2. The source transactions are in the BEEF for the wallet to extract
3. The wallet should include these inputs in the transaction

---

## Use Cases for inputBEEF

### 1. Collaborative Transactions (ANYONECANPAY)
- App pre-signs its input with `SIGHASH_SINGLE|ANYONECANPAY|FORKID`
- Wallet adds its inputs and signs with `SIGHASH_ALL|FORKID`
- Both parties' inputs end up in the same transaction

### 2. Wallet-less Transaction Construction
- App doesn't have signing capability
- Provides UTXOs and asks wallet to sign everything

### 3. Multi-party Transactions
- Multiple parties contribute inputs
- Each provides their inputs via inputBEEF

---

## TypeScript SDK Flow (Reference)

From `reference/ts-brc100/src/signer/methods/buildSignableTransaction.ts`:

```typescript
// Line 26: Parse inputBEEF into a Beef object
const inputBeef = args.inputBEEF ? Beef.fromBinary(args.inputBEEF) : undefined

// Lines 103-118: Handle user-provided inputs
if (argsInput) {
  // User supplied input with or without unlockingScript
  const hasUnlock = typeof argsInput.unlockingScript === 'string'
  const unlock = hasUnlock ? asBsvSdkScript(argsInput.unlockingScript!) : new Script()

  // Look up source transaction from BEEF
  const sourceTransaction = args.isSignAction
    ? inputBeef?.findTxid(argsInput.outpoint.txid)?.tx
    : undefined

  const inputToAdd: TransactionInput = {
    sourceTXID: argsInput.outpoint.txid,
    sourceOutputIndex: argsInput.outpoint.vout,
    sourceTransaction,  // From BEEF!
    unlockingScript: unlock,
    sequence: argsInput.sequenceNumber
  }
  tx.addInput(inputToAdd)
}
```

Key points:
1. **Parse BEEF first**: `Beef.fromBinary(args.inputBEEF)`
2. **Look up source tx**: `inputBeef?.findTxid(txid)?.tx`
3. **Handle missing gracefully**: Returns `undefined` if not found
4. **Two input types**: User inputs (from args.inputs) vs Storage inputs (wallet UTXOs)

---

## ~~Current~~ Previous Rust Implementation Issues (NOW FIXED)

### ~~Issue 1~~: Missing `inputs` Field - FIXED

~~File: `rust-wallet/src/handlers.rs` (lines 1816-1833)~~

**Now implemented**: `CreateActionRequest` has both `inputs` and `input_beef` fields with flexible format support.

### ~~Issue 2~~: inputBEEF Never Processed - FIXED

~~The `create_action` handler at line 1914 parses the request but never:~~
- ~~Reads `input_beef`~~
- ~~Parses it as BEEF~~
- ~~Looks up source transactions~~
- ~~Includes provided inputs~~

**Now implemented**: inputBEEF is parsed (both hex and byte array formats), source transactions are looked up, and user-provided inputs are included in the transaction.

### ~~Issue 3~~: Always Uses Wallet UTXOs - FIXED

~~Lines 1953-1998 always fetch wallet's own UTXOs regardless of what the app provided.~~

**Now implemented**: Wallet UTXOs are only fetched if user inputs don't cover the total needed amount.

---

## Implementation Plan

### Step 1: Add CreateActionInput Struct

Add to `handlers.rs`:

```rust
/// BRC-100 CreateAction input specification
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateActionInput {
    #[serde(rename = "outpoint")]
    pub outpoint: CreateActionOutpoint,

    /// Hex-encoded unlocking script (if pre-signed)
    #[serde(rename = "unlockingScript")]
    pub unlocking_script: Option<String>,

    /// Length of unlocking script (for fee calculation)
    #[serde(rename = "unlockingScriptLength")]
    pub unlocking_script_length: Option<usize>,

    /// Sequence number (default: 0xFFFFFFFF)
    #[serde(rename = "sequenceNumber")]
    pub sequence_number: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateActionOutpoint {
    pub txid: String,
    pub vout: u32,
}
```

### Step 2: Update CreateActionRequest

```rust
pub struct CreateActionRequest {
    #[serde(rename = "outputs")]
    pub outputs: Vec<CreateActionOutput>,

    /// User-provided inputs (optional)
    #[serde(rename = "inputs")]
    pub inputs: Option<Vec<CreateActionInput>>,  // NEW!

    #[serde(rename = "description")]
    pub description: Option<String>,

    #[serde(rename = "inputBEEF")]
    pub input_beef: Option<String>,  // Now will be processed

    // ... rest of fields
}
```

### Step 3: Create InputBEEF Parser Helper

Add new function:

```rust
/// Parse inputBEEF and extract source transaction for a given outpoint
fn get_source_transaction_from_beef(
    beef: &Beef,
    txid: &str,
) -> Option<Vec<u8>> {
    // Use existing Beef::find_txid method
    beef.find_txid(txid).map(|idx| beef.transactions[idx].clone())
}

/// Parse inputBEEF from hex string
fn parse_input_beef(hex_str: &str) -> Result<Beef, String> {
    Beef::from_hex(hex_str)
}
```

### Step 4: Modify create_action Handler Logic

```rust
pub async fn create_action(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    // ... existing parsing code ...

    // NEW: Parse inputBEEF if provided
    let input_beef = match &req.input_beef {
        Some(beef_hex) => {
            match Beef::from_hex(beef_hex) {
                Ok(beef) => {
                    log::info!("   Parsed inputBEEF: {} transactions", beef.transactions.len());
                    Some(beef)
                }
                Err(e) => {
                    log::error!("   Failed to parse inputBEEF: {}", e);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Invalid inputBEEF: {}", e)
                    }));
                }
            }
        }
        None => None,
    };

    // NEW: Process user-provided inputs
    let mut user_inputs: Vec<UserProvidedInput> = Vec::new();
    if let Some(inputs) = &req.inputs {
        for (i, input) in inputs.iter().enumerate() {
            log::info!("   Input {}: {}:{}", i, input.outpoint.txid, input.outpoint.vout);

            // Look up source transaction from BEEF
            let source_tx = input_beef.as_ref()
                .and_then(|beef| get_source_transaction_from_beef(beef, &input.outpoint.txid));

            if source_tx.is_none() && input.unlocking_script.is_none() {
                log::warn!("   Input {} has no source tx in BEEF and no unlocking script", i);
                // Might need to return error or fetch from network
            }

            user_inputs.push(UserProvidedInput {
                txid: input.outpoint.txid.clone(),
                vout: input.outpoint.vout,
                source_tx,
                unlocking_script: input.unlocking_script.clone(),
                sequence: input.sequence_number.unwrap_or(0xFFFFFFFF),
            });
        }
    }

    // Determine if we need wallet's UTXOs
    let need_wallet_utxos = user_inputs.is_empty() ||
        req.options.as_ref().map(|o| o.sign_and_process.unwrap_or(true)).unwrap_or(true);

    if need_wallet_utxos {
        // ... existing UTXO fetching code ...
    }

    // Build transaction with both user and wallet inputs
    // ...
}
```

### Step 5: Handle Pre-signed Inputs (ANYONECANPAY)

When an input has `unlocking_script`, it's pre-signed:

```rust
fn add_input_to_transaction(
    tx: &mut Transaction,
    input: &UserProvidedInput,
) {
    let outpoint = OutPoint::new(input.txid.clone(), input.vout);
    let mut tx_input = TxInput::new(outpoint);

    // Set sequence number
    tx_input.sequence = input.sequence;

    // If pre-signed, use provided script
    if let Some(unlock_hex) = &input.unlocking_script {
        let script_bytes = hex::decode(unlock_hex).expect("Invalid hex");
        tx_input.script_sig = script_bytes;
    }

    tx.add_input(tx_input);
}
```

### Step 6: Handle Missing Source Transaction

If BEEF doesn't contain the source transaction:

```rust
async fn get_source_transaction(
    beef: Option<&Beef>,
    txid: &str,
) -> Result<Option<Vec<u8>>, String> {
    // First try BEEF
    if let Some(beef) = beef {
        if let Some(idx) = beef.find_txid(txid) {
            return Ok(Some(beef.transactions[idx].clone()));
        }
    }

    // Fallback: fetch from WhatsOnChain
    log::info!("   Source tx {} not in BEEF, fetching from WoC", txid);
    match fetch_raw_transaction(txid).await {
        Ok(tx_bytes) => Ok(Some(tx_bytes)),
        Err(e) => {
            log::warn!("   Could not fetch source tx {}: {}", txid, e);
            Ok(None)  // Return None, don't fail the whole request
        }
    }
}
```

### Step 7: Update Response BEEF

Include user inputs in response BEEF:

```rust
fn build_response_beef(
    tx: &Transaction,
    input_beef: Option<&Beef>,
    wallet_input_txs: Vec<(String, Vec<u8>)>,  // (txid, raw_tx)
) -> Result<Beef, String> {
    let mut beef = Beef::new();

    // Add source transactions from inputBEEF (if provided)
    if let Some(ib) = input_beef {
        for (i, tx_bytes) in ib.transactions.iter().enumerate() {
            beef.add_parent_transaction(tx_bytes.clone());
            // Copy BUMP if available
            if let Some(bump_idx) = ib.tx_to_bump[i] {
                // TODO: Copy bump from input_beef
            }
        }
    }

    // Add wallet's input source transactions
    for (txid, tx_bytes) in wallet_input_txs {
        if beef.find_txid(&txid).is_none() {
            beef.add_parent_transaction(tx_bytes);
        }
    }

    // Add the main transaction
    beef.set_main_transaction(tx.serialize());

    Ok(beef)
}
```

---

## Broadcasting Decision Logic

From TypeScript SDK `validationHelpers.ts`:

| Option | Behavior |
|--------|----------|
| `noSend: true` | Don't broadcast, return signed tx |
| `acceptDelayedBroadcast: true` | Wallet can broadcast later |
| `acceptDelayedBroadcast: false` | Wallet must broadcast immediately |
| `sendWith: [...]` | Broadcast with other transactions |

Implementation:

```rust
fn should_broadcast(options: &Option<CreateActionOptions>) -> bool {
    let opts = match options {
        Some(o) => o,
        None => return true,  // Default: broadcast
    };

    // noSend explicitly prevents broadcast
    if opts.no_send.unwrap_or(false) {
        return false;
    }

    // acceptDelayedBroadcast: false means broadcast immediately
    if !opts.accept_delayed_broadcast.unwrap_or(true) {
        return true;
    }

    // Default: don't force immediate broadcast
    false
}
```

---

## Error Handling

### Missing Source Transaction
```rust
// If source tx not in BEEF and can't fetch, return error
if source_tx.is_none() && input.unlocking_script.is_none() {
    return HttpResponse::BadRequest().json(serde_json::json!({
        "error": format!("Input {}:{} not found in inputBEEF and no unlocking script provided",
            input.outpoint.txid, input.outpoint.vout)
    }));
}
```

### Invalid BEEF Format
```rust
match Beef::from_hex(beef_hex) {
    Ok(beef) => Some(beef),
    Err(e) => {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Invalid inputBEEF format: {}", e)
        }));
    }
}
```

### Insufficient Satoshis
When user provides inputs but total is less than outputs + fee:
```rust
let user_input_total: i64 = user_inputs.iter()
    .filter_map(|i| i.source_tx.as_ref())
    .filter_map(|tx| get_output_value(tx, i.vout))
    .sum();

if user_input_total < total_output + estimated_fee {
    // Need to add wallet UTXOs to cover difference
    let shortfall = total_output + estimated_fee - user_input_total;
    // ... fetch wallet UTXOs ...
}
```

---

## Testing Strategy

### 1. Unit Tests
- Parse various BEEF formats (V1, V2, Atomic)
- Find transaction by TXID in BEEF
- Handle missing transaction gracefully

### 2. Integration Tests
- Create action with inputBEEF and inputs
- Verify source transactions extracted correctly
- Verify pre-signed inputs preserved

### 3. Real-world Testing
- Test with beta.zanaadu.com
- Verify collaborative transactions work
- Check ANYONECANPAY scenarios

---

## Files to Modify

| File | Changes |
|------|---------|
| `rust-wallet/src/handlers.rs` | Add `CreateActionInput`, `CreateActionOutpoint` structs; Update `CreateActionRequest`; Modify `create_action` handler |
| `rust-wallet/src/beef.rs` | Already has `find_txid` - may need helper for extracting output values |
| `rust-wallet/src/lib.rs` | May need to export new types |

---

## Implementation Checklist

- [x] Add `CreateActionInput` and `CreateActionOutpoint` structs
- [x] Add `inputs` field to `CreateActionRequest`
- [x] Parse inputBEEF when present (supports both hex string and byte array formats)
- [x] Look up source transactions from BEEF
- [x] Handle pre-signed unlocking scripts
- [x] Handle missing source transactions (fetch from network) - partial, logs warning
- [x] Calculate input values from source transactions
- [x] Determine when wallet UTXOs needed
- [x] Build response BEEF with all source transactions (full chain with BUMPs)
- [x] Implement broadcast decision logic
- [x] Add error handling for edge cases
- [x] Custom outpoint deserializer for both object and string formats
- [ ] Update tests
- [ ] Test with real apps (beta.zanaadu.com) - blocked by registry state issue

---

## References

- [BRC-62: BEEF Format](https://github.com/bsv-blockchain/BRCs/blob/master/transactions/0062.md)
- [BRC-74: BUMP Format](https://github.com/bsv-blockchain/BRCs/blob/master/transactions/0074.md)
- [BRC-95: Atomic BEEF](https://github.com/bsv-blockchain/BRCs/blob/master/transactions/0095.md)
- [BRC-100: Wallet Interface](https://bsv.brc.dev/wallet/0100)
- TypeScript SDK: `reference/ts-brc100/src/signer/methods/buildSignableTransaction.ts`
- TypeScript BEEF: `reference/ts-brc100/node_modules/@bsv/sdk/dist/esm/src/transaction/Beef.js`

---

**Last Updated**: 2024-12-26
**Status**: IMPLEMENTED (testing blocked by Zanaadu registry state issue)

### Additional Implementation Notes (Dec 26, 2024)

#### Format Flexibility
The implementation handles multiple input formats used by real-world apps:
- `inputBEEF`: Both hex strings and byte arrays `[u8, u8, ...]`
- `outpoint`: Both object `{txid, vout}` and string `"txid.vout"` formats

#### Fee Calculation
Added dynamic fee calculation based on transaction size:
- `DEFAULT_SATS_PER_KB = 1000` (1 sat/byte)
- Two-pass calculation: estimate for UTXO selection, recalculate with actual inputs
- See `handlers.rs` fee utilities section

#### Full BEEF Chain
Response BEEF now includes ALL transactions and BUMPs from inputBEEF, not just direct parents. This ensures overlay servers can perform complete SPV verification.
