# BEEF Formats Summary

## Overview
This document summarizes the differences between BEEF, Atomic BEEF, and BEEF V2 based on the BRC specifications.

## Format Comparison

### 1. Standard BEEF (BRC-62)
**Reference**: [BRC-62](https://bsv.brc.dev/transactions/0062)

**Purpose**: Package Bitcoin transactions with their ancestry and optional SPV proofs for verification without a full blockchain.

**Structure**:
```
[Version Marker (4 bytes)]
  - V1: 0x0100beef
  - V2: 0x0200beef
[Number of BUMPs (varint)]
[BUMPs array]
[Number of Transactions (varint)]
[Transactions array]
  - V1: [raw_tx][bump_flag][bump_index?]
  - V2: [format_byte][bump_index?][raw_tx or txid]
```

**Use Cases**:
- `listOutputs` with `include='entire transactions'` → Returns standard BEEF
- General transaction packaging with ancestry
- SPV validation without full blockchain

**Our Implementation**: ✅ Uses BEEF V2 (0x0200beef) as default

---

### 2. Atomic BEEF (BRC-95)
**Reference**: [BRC-95](https://bsv.brc.dev/transactions/0095)

**Purpose**: Wrapper around standard BEEF for single transaction validation. Adds a subject TXID to identify which transaction is being validated.

**Structure**:
```
[Magic Prefix: 0x01010101 (4 bytes)]
[Subject TXID (32 bytes, big-endian)]
[Standard BEEF structure]
```

**Use Cases**:
- `signAction` response → Returns Atomic BEEF for the signed transaction
- Single transaction validation with its ancestry
- When you need to identify a specific transaction in the BEEF

**Our Implementation**: ✅ `to_atomic_beef_hex()` method exists

---

### 3. BEEF V2 (BRC-96)
**Reference**: [BRC-96](https://bsv.brc.dev/transactions/0096)

**Purpose**: Enhancement to BEEF that supports TXID-only transactions for efficiency.

**Key Features**:
- Version marker: `0x0200beef`
- Format bytes for transactions:
  - `0x00`: Raw transaction without BUMP
  - `0x01`: Raw transaction with BUMP index
  - `0x02`: TXID only (32 bytes) - for known/validated transactions

**Use Cases**:
- When a transaction is already known/validated, reference it by TXID only
- Reduces BEEF size when transactions are already available
- Used with `knownTxids` parameter in `listOutputs`

**Our Implementation**:
- ✅ Uses BEEF V2 as default
- ⏳ TXID-only (format 0x02) parsing exists but returns error
- ⏳ TXID-only serialization not yet implemented

---

### 4. Transaction Extended Format (BRC-30)
**Reference**: [BRC-30](https://bsv.brc.dev/transactions/0030)

**Purpose**: Alternative transaction format (NOT a packaging format like BEEF). Embeds previous output data (locking script and satoshis) in transaction inputs.

**Structure**:
```
[Version (4 bytes)]
[EF Marker: 0x0000000000EF (4 bytes)]
[Inputs with embedded previous output data]
[Outputs]
[Locktime]
```

**Use Cases**:
- Broadcast services can validate transactions without UTXO lookups
- Faster transaction validation
- Different from BEEF - this is an alternative transaction format

**Our Implementation**: ❌ Not implemented (different use case than BEEF)

---

## Implementation Status

| Format | Status | Notes |
|--------|--------|-------|
| Standard BEEF V1 | ✅ Supported | Can parse, but V2 is default |
| Standard BEEF V2 | ✅ Supported | Default format, fully implemented |
| Atomic BEEF | ✅ Supported | `to_atomic_beef_hex()` method exists |
| BEEF V2 TXID-only | ⏳ Partial | Can parse but returns error, serialization not implemented |
| Transaction EF | ❌ Not needed | Different format, not for BEEF use cases |

---

## For Our Implementation

### `listOutputs` BEEF Generation:
- ✅ Use **Standard BEEF V2** (not Atomic BEEF)
- ✅ Use format byte `0x00` (raw tx) when no BUMP available
- ✅ Use format byte `0x01` (raw tx + BUMP) when Merkle proof available
- ⏳ Future: Use format byte `0x02` (TXID-only) when transaction in `knownTxids`

### `signAction` BEEF Generation:
- ✅ Use **Atomic BEEF** (wraps standard BEEF)
- ✅ Already implemented correctly

---

## References
- [BRC-30: Transaction Extended Format](https://bsv.brc.dev/transactions/0030)
- [BRC-62: Background Evaluation Extended Format (BEEF)](https://bsv.brc.dev/transactions/0062)
- [BRC-95: Atomic BEEF Transactions](https://bsv.brc.dev/transactions/0095)
- [BRC-96: BEEF V2 Txid Only Extension](https://bsv.brc.dev/transactions/0096)
