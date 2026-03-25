# Wallet Service Fee — Implementation Plan

**Created**: 2026-03-25
**Status**: Ready for implementation
**Decision**: Static BSV address, fixed 1000 satoshis per transaction

---

## Overview

Add a fixed 1000-satoshi service fee output to every outgoing transaction. The fee goes to a static company BSV address as a standard P2PKH output. The company monitors and sweeps this address periodically.

**Why static address?** Using an identity pubkey with BRC-42 derivation would require MessageBox/PeerPay notification for every transaction so the company could derive the spending key. A static address avoids all that complexity — it's just one extra `tx.add_output()` call.

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Fee amount | Fixed 1000 sats | Simple, no price dependency, always above 546-sat dust limit (~$0.04 at $40/BSV) |
| Collection method | Static P2PKH address | No PeerPay/MessageBox/BRC-42 needed |
| Fee exemptions | None (charge everything) | Simplicity for MVP — can add exemptions later |

---

## Prerequisites

- **COMPLETE** — Company treasury wallet created via Hodos Browser (Option A)
- **Address**: `1Q1A2rq6trBdptd3t6n53vB79mRN6JHEFT` (master address, index -1)
- **Public Key**: `029dce5a3602abd74ddf0a9dd1a9e80dff2eb997bc0b01af70b5cde0e7d5166dba`
- **Wallet DB**: `wallet.db` at `%APPDATA%/HodosBrowser/wallet/` — rename to `wallet_company.db` when not in use
- **Mnemonic + PIN**: stored securely by Matt

---

## Implementation Steps

### Step 1: Add Constants

**File**: `rust-wallet/src/handlers.rs` (~line 28, near existing fee constants)

```rust
// ============================================================================
// Wallet Service Fee
// ============================================================================

/// Company BSV address for service fee collection.
/// Standard P2PKH — company monitors and sweeps periodically.
pub const HODOS_FEE_ADDRESS: &str = "1Q1A2rq6trBdptd3t6n53vB79mRN6JHEFT";

/// Fixed service fee in satoshis added to every outgoing transaction.
/// Must be >= 546 (dust limit). Currently ~$0.04 at $40/BSV.
pub const HODOS_SERVICE_FEE_SATS: i64 = 1000;
```

Both `pub` so `certificate_handlers.rs` can reference them.

### Step 2: Add Helper Function

**File**: `rust-wallet/src/handlers.rs` (near `address_to_script` at line 5664)

No new function needed — the existing `address_to_script()` (line 5664) already does exactly what we need. It takes a BSV address and returns a 25-byte P2PKH locking script. We just call it with our constant:

```rust
let fee_script = address_to_script(HODOS_FEE_ADDRESS)
    .expect("HODOS_FEE_ADDRESS is invalid — this is a compile-time constant, fix it");
```

The `.expect()` is safe because the address is a hardcoded constant — if it's wrong, we want to panic loudly during the first transaction, not silently skip the fee.

### Step 3: Modify `create_action_internal` (handlers.rs)

Five touch points in the main transaction builder:

#### 3a. Fee output in size estimation (~line 3760)

After collecting `output_script_lengths` from request outputs, add the fee output:

```rust
// Account for Hodos service fee output in size estimation
output_script_lengths.push(25); // P2PKH locking script = 25 bytes
```

This ensures the miner fee estimate accounts for the extra output bytes.

#### 3b. Include service fee in `total_needed` (~line 3795)

```rust
// Before:
// let total_needed = total_output + estimated_fee;

// After:
let total_needed = total_output + estimated_fee + HODOS_SERVICE_FEE_SATS;
```

This ensures UTXO selection fetches enough satoshis to cover the service fee.

#### 3c. Send-max deduction (~line 4148)

```rust
// Before:
// total_output = total_input - estimated_fee;

// After:
total_output = total_input - estimated_fee - HODOS_SERVICE_FEE_SATS;
```

When sending max, the user gets `balance - miner fee - service fee`.

#### 3d. Add fee output to transaction (~line 4384)

After the loop that adds request outputs (`tx.add_output(...)`) and before the change calculation:

```rust
// Add Hodos service fee output
let fee_script = address_to_script(HODOS_FEE_ADDRESS)
    .expect("HODOS_FEE_ADDRESS constant is invalid");
tx.add_output(TxOutput::new(HODOS_SERVICE_FEE_SATS, fee_script));
log::info!("   💰 Added Hodos service fee output: {} satoshis to {}", HODOS_SERVICE_FEE_SATS, HODOS_FEE_ADDRESS);
```

#### 3e. Adjust change calculation (~line 4387)

```rust
// Before:
// let change = total_input - total_output - estimated_fee;

// After:
let change = total_input - total_output - estimated_fee - HODOS_SERVICE_FEE_SATS;
```

### Step 4: Modify `publish_certificate` (certificate_handlers.rs)

**File**: `rust-wallet/src/handlers/certificate_handlers.rs`

#### 4a. Fee estimation (~line 2730 area)

Add 25 to the output script lengths used for fee estimation (wherever `estimated_fee` is calculated), to account for the extra output.

#### 4b. Add fee output (~line 2771, after certificate output)

```rust
// After: tx.add_output(certificate_output);

// Add Hodos service fee output
let fee_script = crate::handlers::address_to_script(crate::handlers::HODOS_FEE_ADDRESS)
    .expect("HODOS_FEE_ADDRESS constant is invalid");
tx.add_output(TxOutput::new(crate::handlers::HODOS_SERVICE_FEE_SATS, fee_script));
log::info!("   💰 Added Hodos service fee: {} satoshis", crate::handlers::HODOS_SERVICE_FEE_SATS);
```

#### 4c. Adjust change calculation (~line 2777)

```rust
// Before:
// let change_amount = total_input - certificate_output_amount - fee;

// After:
let change_amount = total_input - certificate_output_amount - fee - crate::handlers::HODOS_SERVICE_FEE_SATS;
```

### Step 5: Modify `unpublish_certificate_core` (certificate_handlers.rs)

Same pattern as Step 4.

#### 5a. Fee estimation

Account for extra output in fee calculation.

#### 5b. Add fee output (~line 4425, before change output)

```rust
// Add Hodos service fee output
let fee_script = crate::handlers::address_to_script(crate::handlers::HODOS_FEE_ADDRESS)
    .expect("HODOS_FEE_ADDRESS constant is invalid");
tx.add_output(TxOutput::new(crate::handlers::HODOS_SERVICE_FEE_SATS, fee_script));
log::info!("   💰 Added Hodos service fee: {} satoshis", crate::handlers::HODOS_SERVICE_FEE_SATS);
```

#### 5c. Adjust change calculation (~line 4382)

```rust
// Before:
// let change_amount = total_in - estimated_fee;

// After:
let change_amount = total_in - estimated_fee - crate::handlers::HODOS_SERVICE_FEE_SATS;
```

### Step 6: Exclude Fee Output from Response Mapping

The service fee is an internal wallet output — callers (BRC-100 SDK, browser frontend) should not see it in the response.

#### 6a. StoredAction outputs (~line 4874)

The `StoredAction.outputs` array is saved to the DB. Including the fee output here is fine — it's part of the real transaction. **No change needed.**

#### 6b. CreateActionResponse outputs (~line 5440)

The response to the caller should only include the outputs they requested. Change the mapping to only take `req.outputs.len()` outputs:

```rust
// Before:
// let response_outputs: Vec<CreateActionResponseOutput> = tx.outputs.iter().enumerate().map(...)

// After:
let num_request_outputs = req.outputs.len();
let response_outputs: Vec<CreateActionResponseOutput> = tx.outputs.iter()
    .take(num_request_outputs)  // Only request outputs — skip service fee + change
    .enumerate()
    .map(|(i, output)| {
        CreateActionResponseOutput {
            vout: i as u32,
            satoshis: output.value,
            script_length: output.script_pubkey.len(),
            script_offset: 0,
        }
    }).collect();
```

### Step 7: Record Commission in Database

After the transaction is saved to the DB, record the fee in the existing `commissions` table. This happens in `create_action_internal` after `tx_repo.add_transaction()` succeeds (~line 4888):

```rust
// Record Hodos service fee as commission
{
    let commission_repo = CommissionRepository::new(db.connection());
    let commission = Commission {
        commission_id: None,
        user_id: state.current_user_id,
        transaction_id: tx_db_id,
        satoshis: HODOS_SERVICE_FEE_SATS,
        key_offset: "hodos-service-fee".to_string(),
        is_redeemed: false,
        locking_script: Some(hex::encode(&address_to_script(HODOS_FEE_ADDRESS).unwrap())),
        created_at: 0,  // CommissionRepository sets this
        updated_at: 0,
    };
    if let Err(e) = commission_repo.create(&commission) {
        log::warn!("   ⚠️  Failed to record service fee commission: {}", e);
        // Non-fatal — tx already in DB, just missing commission record
    }
}
```

Same pattern for both certificate handlers (publish + unpublish) after their transaction is saved.

### Step 8: Commission Cleanup on Broadcast Failure

The existing broadcast failure rollback paths (~line 5270 and ~line 5292 in handlers.rs, plus equivalent in certificate_handlers.rs) already delete ghost outputs and restore inputs. Add commission cleanup alongside:

```rust
// In each broadcast failure rollback path:
let _ = commission_repo.delete_by_transaction_id(tx_db_id);
```

This is defensive — if the transaction record itself is deleted, the commission FK should cascade. But explicit cleanup is safer.

---

## Output Order in Transaction

After implementation, every transaction will have this output structure:

```
Output 0:     Request output (payment, PushDrop, etc.)
Output 1..N:  Additional request outputs (if any)
Output N+1:   Hodos service fee (1000 sats → company address)
Output N+2:   Change (if above dust limit)
```

---

## Files Changed

| File | Changes |
|------|---------|
| `rust-wallet/src/handlers.rs` | 2 constants (`pub`), ~15 lines in `create_action_internal` (5 touch points), response output filtering, commission recording |
| `rust-wallet/src/handlers/certificate_handlers.rs` | ~6 lines each in `publish_certificate` + `unpublish_certificate_core` (fee output + change adjustment) |

**No new files. No new dependencies. No schema changes. No frontend changes.**

---

## Testing Checklist

- [ ] `cargo check` passes
- [ ] Standard send (`/transaction/send`): raw tx on WhatsOnChain shows fee output to company address
- [ ] Send max: user receives `balance - miner_fee - 1000`
- [ ] PeerPay send: fee output present (routes through `create_action_internal`)
- [ ] Paymail send: fee output present (routes through `create_action_internal`)
- [ ] Certificate publish: fee output present
- [ ] Certificate unpublish: fee output present
- [ ] Insufficient balance (<1546 sats): error message is clear
- [ ] Commission record exists in `commissions` table after successful tx
- [ ] Commission deleted on broadcast failure
- [ ] `CreateActionResponse.outputs` does NOT include the fee output
- [ ] `StoredAction.outputs` in DB DOES include the fee output (it's real)

---

## Future Enhancements

1. **Dynamic fee** — use `PriceCache` to target a fixed USD amount
2. **Fee exemptions** — skip for internal ops (backup, consolidation)
3. **UI transparency** — show "Service fee: 1000 sats" in send confirmation
4. **Analytics** — endpoint to query total fees collected
5. **Configurable** — move fee amount to `settings` table instead of constant
