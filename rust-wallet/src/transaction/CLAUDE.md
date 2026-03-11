# Transaction — Bitcoin SV Transaction Primitives

> Core transaction types, serialization, and BSV ForkID SIGHASH computation for the wallet backend.

## Overview

This module provides the low-level Bitcoin transaction building blocks used throughout the wallet. It defines the wire-format structures (`Transaction`, `TxInput`, `TxOutput`, `OutPoint`, `Script`), Bitcoin varint encoding/decoding, and the BSV-specific ForkID SIGHASH algorithm required for signing. These types are consumed directly by `handlers.rs` (for `create_action`/`sign_action`), `certificate_handlers.rs` (for certificate transactions), `recovery.rs` (for wallet recovery signing), and `task_unfail.rs` (for outpoint extraction during failure recovery).

**Security note**: This module handles transaction construction and hash computation but does NOT hold private keys. Signing occurs in `src/crypto/signing.rs` using sighashes produced here.

## Files

| File | Purpose |
|------|---------|
| `mod.rs` | Module root — re-exports core types, varint encode/decode functions, `extract_input_outpoints()` utility |
| `types.rs` | Core structs: `Transaction`, `TxInput`, `TxOutput`, `OutPoint`, `Script`, `TransactionError` |
| `sighash.rs` | BSV ForkID SIGHASH algorithm (`calculate_sighash`) with SIGHASH_ALL, SIGHASH_SINGLE, SIGHASH_ANYONECANPAY support |

## Key Exports

### Types (`types.rs`)

| Type | Description |
|------|-------------|
| `Transaction` | Complete Bitcoin transaction (version, inputs, outputs, lock_time). Serializes to wire format, computes txid via double-SHA256 |
| `TxInput` | Transaction input: `OutPoint` + `script_sig` + `sequence`. Default sequence `0xFFFFFFFF` |
| `TxOutput` | Transaction output: `value` (i64 satoshis) + `script_pubkey` (raw bytes) |
| `OutPoint` | Reference to a previous output: `txid` (hex string) + `vout` (u32). `txid_bytes()` reverses for wire format |
| `Script` | Bitcoin script wrapper with builders for P2PKH locking (`p2pkh_locking_script`) and unlocking (`p2pkh_unlocking_script`) scripts |
| `TransactionError` | Error enum: `InvalidFormat`, `InvalidScript`, `HexDecode` |
| `TransactionResult<T>` | Type alias for `Result<T, TransactionError>` |

### Functions (`mod.rs`)

| Function | Description |
|----------|-------------|
| `encode_varint(n: u64)` | Standard Bitcoin varint encoding (1/3/5/9 bytes) |
| `encode_varint_signed(n: i64)` | Signed varint matching TypeScript SDK's `writeVarIntNum` — negative values use two's complement |
| `decode_varint(data: &[u8])` | Decode varint from byte slice, returns `(value, bytes_consumed)` |
| `extract_input_outpoints(raw_tx_hex: &str)` | Parse raw transaction hex to extract `(txid, vout)` pairs for all inputs. Used by `TaskUnFail` to re-mark inputs as spent during false-failure recovery |

### SIGHASH (`sighash.rs`)

| Export | Description |
|--------|-------------|
| `calculate_sighash(tx, input_index, prev_script, prev_value, sighash_type)` | BSV ForkID SIGHASH computation — returns 32-byte double-SHA256 hash of the signing preimage |
| `SIGHASH_ALL` | `0x01` — sign all inputs and outputs |
| `SIGHASH_FORKID` | `0x40` — BSV fork replay protection flag |
| `SIGHASH_ALL_FORKID` | `0x41` — standard BSV signing mode (ALL + FORKID) |

## SIGHASH Algorithm

The ForkID SIGHASH (`sighash.rs`) implements BIP143-style signing used by BSV after the 2017 UAHF fork. The preimage is constructed as:

1. **Version** (4 bytes LE)
2. **hashPrevouts** — double-SHA256 of all input outpoints (or zero hash for ANYONECANPAY)
3. **hashSequence** — double-SHA256 of all input sequences (or zero hash for ANYONECANPAY/SINGLE/NONE)
4. **Input outpoint** — txid (32 bytes, reversed) + vout (4 bytes LE)
5. **Previous script** — varint length + script bytes
6. **Previous value** — satoshis of the UTXO being spent (8 bytes LE)
7. **Sequence** — input sequence number (4 bytes LE)
8. **hashOutputs** — double-SHA256 of all outputs (or single output for SIGHASH_SINGLE, or zero hash for NONE)
9. **Locktime** (4 bytes LE)
10. **SIGHASH type** (4 bytes LE)

The result is double-SHA256 hashed to produce the 32-byte sighash.

Internal helpers (not exported): `calculate_hash_prevouts`, `calculate_hash_sequence`, `calculate_hash_outputs`, `calculate_hash_outputs_single`.

## Usage

### Building and signing a transaction (from `handlers.rs`)

```rust
use crate::transaction::{Transaction, TxInput, TxOutput, OutPoint, Script};
use crate::transaction::{calculate_sighash, SIGHASH_ALL_FORKID};

// Build transaction
let mut tx = Transaction::new();
tx.add_input(TxInput::new(OutPoint::new(prev_txid, prev_vout)));
tx.add_output(TxOutput::new(amount, locking_script_bytes));

// Compute sighash for signing
let sighash = calculate_sighash(&tx, 0, &prev_locking_script, prev_value, SIGHASH_ALL_FORKID)?;

// Sign externally (in crypto module), then set unlocking script
let unlock = Script::p2pkh_unlocking_script(&signature_with_hashtype, &public_key);
tx.inputs[0].set_script(unlock.bytes);

// Serialize for broadcast
let raw_hex = tx.to_hex()?;
let txid = tx.txid()?;
```

### Varint encoding (from `certificate_handlers.rs`)

```rust
use crate::transaction::{encode_varint, encode_varint_signed};

let count_bytes = encode_varint(field_count as u64);
let signed_bytes = encode_varint_signed(-1i64); // 9-byte two's complement
```

### Extracting outpoints from raw tx (from `task_unfail.rs`)

```rust
use crate::transaction::extract_input_outpoints;

let outpoints = extract_input_outpoints(&stored_tx.raw_tx)?;
for (txid, vout) in outpoints {
    // Re-mark these inputs as spent during false-failure recovery
}
```

## Consumers

| File | What it uses |
|------|-------------|
| `src/handlers.rs` | `Transaction`, `TxInput`, `TxOutput`, `OutPoint`, `Script`, `calculate_sighash`, `SIGHASH_ALL_FORKID`, `get_transaction_fee()` |
| `src/handlers/certificate_handlers.rs` | All types + `encode_varint`, `encode_varint_signed`, `calculate_sighash`, `SIGHASH_ALL_FORKID` |
| `src/recovery.rs` | `Transaction`, `TxInput`, `TxOutput`, `OutPoint`, `Script` |
| `src/certificate/verifier.rs` | `encode_varint` |
| `src/monitor/task_unfail.rs` | `extract_input_outpoints` |

## Related

- `../crypto/signing.rs` — ECDSA signing using sighashes from this module
- `../crypto/brc42.rs` — Key derivation for inputs (determines which key signs)
- `../database/helpers.rs` — `derive_key_for_output()` resolves derivation path to signing key
- `../beef.rs` — BEEF format wraps signed transactions with Merkle proofs
- `../CLAUDE.md` — Wallet backend overview, fee calculation constants, full API endpoint list
