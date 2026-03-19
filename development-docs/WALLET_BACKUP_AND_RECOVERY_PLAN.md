# Wallet Backup & Recovery — Implementation Outline

**Created**: 2026-02-03
**Updated**: 2026-03-16 — Section 4 revised: PushDrop full wallet backup (see `ONCHAIN_FULL_BACKUP_RESEARCH.md`)
**Status**: Section 4 ready for implementation; Sections 3, 5 unchanged
**Related**: `STATE_MAINTENANCE_AND_RECONCILIATION_TRANSITION_PLAN.md` (Phases 2, 5, 7), `ONCHAIN_FULL_BACKUP_RESEARCH.md`
**Objective**: Enable full wallet recovery from mnemonic alone by combining three backup mechanisms — on-chain full backup (primary), local file export (user-initiated), and optional cloud sync.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Backup Data Model](#2-backup-data-model)
3. [Mechanism A: Local Encrypted Backup File](#3-mechanism-a-local-encrypted-backup-file)
4. [Mechanism B: On-Chain Encrypted Backup](#4-mechanism-b-on-chain-encrypted-backup)
5. [Mechanism C: Cloud Sync Scaffolding](#5-mechanism-c-cloud-sync-scaffolding)
6. [Recovery Flows](#6-recovery-flows)
7. [Implementation Phases](#7-implementation-phases)
8. [Relationship to Transition Plan](#8-relationship-to-transition-plan)
9. [Open Questions](#9-open-questions)

---

## 1. Overview

### The Problem

BRC-42 self-derived receive addresses (`"2-receive address-0"`, `"2-receive address-1"`, ...) are recoverable from a mnemonic alone by scanning sequential indices — the same recovery model as BIP32 HD wallets. However, counterparty-derived BRC-42 protocol outputs (where `senderIdentityKey != self`), PushDrop tokens, and certificates use non-sequential derivation — their derivation parameters (counterparty pubkey, protocol-specific prefix/suffix) cannot be guessed. Without a backup of those parameters, these outputs are lost if the database is lost.

### The Solution: Three Layers

```
┌─────────────────────────────────────────────────────────┐
│  Layer 1: On-Chain Full Wallet Backup (automatic)       │
│  Entire wallet DB compressed+encrypted in PushDrop UTXO │
│  at deterministic BRC-42 address. Mnemonic-only recovery│
│  No scanning needed. Built-in multi-device sync.        │
│  Cost: $0.08-$6/year for daily backups.                 │
├─────────────────────────────────────────────────────────┤
│  Layer 2: Local Encrypted Backup File (user-initiated)  │
│  Full wallet export in BSV SDK sync-compatible JSON.    │
│  User saves to USB, cloud drive, etc.                   │
├─────────────────────────────────────────────────────────┤
│  Layer 3: Cloud Sync (optional, simplified)             │
│  Same JSON format. On-chain backup serves as            │
│  primary sync mechanism; cloud is supplementary.        │
└─────────────────────────────────────────────────────────┘
```

All three layers use the **same JSON entity format** matching the BSV SDK wallet-toolbox sync protocol. Data serialized for one layer can be deserialized by any other.

> **Key Insight (2026-03-16)**: JSON wallet data compresses 96-97% with gzip (e.g., 500 KB → 18 KB). This makes full wallet on-chain backup feasible at negligible cost, eliminating the need for partial backups + scanning.

---

## 2. Backup Data Model

### Entity Format (shared across all three mechanisms)

The backup payload is a JSON object containing arrays of entities matching the wallet-toolbox shapes. This ensures compatibility with the BSV SDK sync system and future cloud sync.

```json
{
  "version": 1,
  "chain": "main",
  "created_at": "2026-02-03T12:00:00Z",
  "wallet_identity_key": "02abc...def",

  "users": [
    {
      "userId": 1,
      "identityKey": "02abc...def",
      "activeStorage": "local"
    }
  ],

  "transactions": [
    {
      "transactionId": 42,
      "userId": 1,
      "txid": "abc123...",
      "status": "completed",
      "reference": "ref-001",
      "isOutgoing": true,
      "satoshis": 50000,
      "description": "Payment to merchant",
      "rawTx": "<hex>",
      "inputBEEF": "<hex or null>",
      "provenTxId": 7
    }
  ],

  "outputs": [
    {
      "outputId": 100,
      "userId": 1,
      "transactionId": 42,
      "basketId": 3,
      "spendable": true,
      "change": false,
      "vout": 0,
      "satoshis": 45000,
      "txid": "abc123...",
      "derivationPrefix": "2-authrite",
      "derivationSuffix": "session-xyz",
      "senderIdentityKey": "03fed...cba",
      "customInstructions": null,
      "outputDescription": "Auth token",
      "lockingScript": "<hex>",
      "providedBy": "you",
      "purpose": "",
      "type": "custom",
      "spentBy": null
    }
  ],

  "proven_txs": [
    {
      "provenTxId": 7,
      "txid": "abc123...",
      "height": 850001,
      "index": 42,
      "merklePath": "<hex>",
      "rawTx": "<hex>",
      "blockHash": "0000...fff",
      "merkleRoot": "aaa...bbb"
    }
  ],

  "output_baskets": [
    {
      "basketId": 3,
      "userId": 1,
      "name": "auth-tokens",
      "numberOfDesiredUTXOs": 6,
      "minimumDesiredUTXOValue": 10000,
      "isDeleted": false
    }
  ],

  "certificates": [
    {
      "certificateId": 1,
      "userId": 1,
      "type": "identity",
      "serialNumber": "cert-001",
      "certifier": "03aaa...",
      "subject": "02abc...",
      "verifier": null,
      "revocationOutpoint": "txid.0",
      "signature": "3045...",
      "isDeleted": false,
      "fields": [
        { "fieldName": "name", "fieldValue": "<encrypted>", "masterKey": "<hex>" },
        { "fieldName": "email", "fieldValue": "<encrypted>", "masterKey": "<hex>" }
      ]
    }
  ],

  "output_tags": [ ... ],
  "tx_labels": [ ... ],
  "settings": { ... }
}
```

### What's Included vs Excluded

| Included | Excluded |
|---|---|
| All transactions (with rawTx) | Mnemonic (user re-enters on recovery) |
| All outputs (with derivation data) | Domain whitelist (browser-specific) |
| All proven_txs (with merkle proofs) | Messages / relay_messages |
| All certificates + fields | Balance cache (recomputed) |
| All baskets, tags, labels | Derived key cache (recomputed) |
| Settings | Monitor events (diagnostic only) |

### Backup Size Estimates

| Wallet Activity | Estimated Payload Size |
|---|---|
| Light use (50 transactions, 100 outputs) | ~50 KB |
| Moderate use (500 transactions, 1000 outputs) | ~500 KB |
| Heavy use (5000 transactions, 10000 outputs) | ~5 MB |
| Encrypted + compressed | ~40-60% of raw JSON |

---

## 3. Mechanism A: Local Encrypted Backup File

### User Flow

```
Export:
  User clicks "Export Wallet Backup" in settings
    → Wallet serializes all entities to JSON (format above)
    → Wallet encrypts with AES-256-GCM
    → Encryption key derived from user PIN/passphrase via HKDF-SHA256
      (DECISION: user PIN for file backup, NOT mnemonic — see Open Questions #1)
    → Save as .bsv-wallet file (encryption byte = 0x02)
    → User stores file wherever they want

Import:
  User clicks "Import Wallet Backup" in settings
    → Select .bsv-wallet file
    → Enter PIN/passphrase (same one used during export)
    → Decrypt
    → Validate: check wallet_identity_key matches derived master pubkey
    → Merge entities into database (insert or update, never overwrite newer data)
    → Recompute balance cache
```

### File Format

```
.bsv-wallet file structure:
┌──────────────────────────────────┐
│ Magic bytes: "BSVW" (4 bytes)    │
│ Version: uint8 (1 byte)          │
│ Encryption: uint8 (1 byte)       │
│   0x01 = AES-256-GCM, mnemonic  │
│   0x02 = AES-256-GCM, passphrase│
│ IV/Nonce: 12 bytes               │
│ Salt: 32 bytes (for key deriv.)  │
│ Encrypted payload: variable      │
│ Auth tag: 16 bytes (GCM tag)     │
└──────────────────────────────────┘
```

### Rust Implementation

```
New files:
  src/backup/mod.rs           — Module root
  src/backup/export.rs        — Serialize entities → encrypted file
  src/backup/import.rs        — Decrypt file → merge entities into DB
  src/backup/entities.rs      — Serde structs matching the JSON format
  src/backup/encryption.rs    — AES-256-GCM encrypt/decrypt, key derivation

Modified files:
  src/handlers.rs             — Add /wallet/export and /wallet/import endpoints
  src/main.rs                 — Register new routes
```

### Endpoints

```
POST /wallet/export
  → Returns encrypted .bsv-wallet file as binary response

POST /wallet/import
  Body: { file: <binary>, mnemonic: "<mnemonic>" }
  → Decrypts, validates, merges into database
  → Returns { imported_transactions: N, imported_outputs: N, ... }
```

---

## 4. Mechanism B: On-Chain Full Wallet Backup (PushDrop)

> **Updated 2026-03-16**: Revised approach using PushDrop with full wallet backup instead of OP_RETURN with partial data. See `ONCHAIN_FULL_BACKUP_RESEARCH.md` for feasibility analysis.

### Concept

Store the **entire encrypted wallet database** in a PushDrop UTXO at a deterministic BRC-42 self-counterparty address. This approach:

1. **Eliminates scanning** — Recovery checks one known address, not sequential scan
2. **Backs up everything** — Full wallet state, not just counterparty outputs
3. **Enables multi-device sync** — All devices check the same address
4. **Recovers satoshis** — PushDrop is spendable; each update reclaims previous sats

### Why Full Backup Instead of Partial?

The original plan backed up only counterparty-derived outputs (assuming self-derived could be scanned). However:

| Approach | Backup Size | Recovery Speed | Multi-Device Sync | Complexity |
|----------|-------------|----------------|-------------------|------------|
| **Partial (original)** | 2-15 KB | Minutes (scanning) | Manual | Higher |
| **Full (revised)** | 2-170 KB | Seconds (one lookup) | Built-in | Lower |

**Cost difference is negligible** (~$5/year more for full backup), but full backup provides dramatically simpler recovery and built-in sync.

### Design

**Address Derivation (BRC-42 Self-Counterparty)**:
```
Security Level: 2 (high security, authenticated)
Protocol ID:    "wallet-backup"
Key ID:         "1"
Counterparty:   "self"

derived_key = BRC-42(master_privkey, own_pubkey, "2-wallet-backup-1")
```

This is deterministic from the seed alone — no scanning required.

**Transaction Structure (PushDrop)**:
```
Input:  previous backup UTXO (or any wallet UTXO for first backup)
Output: PushDrop token to derived backup key
        
        <derived_pubkey> OP_CHECKSIG
        <compressed_encrypted_payload>
        
        Token amount: 1000 sats (recoverable on next update)
```

**Why PushDrop Over OP_RETURN**:

| Aspect | OP_RETURN | PushDrop |
|--------|-----------|----------|
| Spendable? | ❌ No (burned) | ✅ Yes (recover sats) |
| Outputs needed | 2 (marker + data) | 1 (combined) |
| Cost over time | Accumulates | Effectively free after first |
| Discovery | Scan for marker | Check derived address |

### Compression: The Key to Feasibility

JSON wallet data compresses **96-97%** with gzip:

| Wallet Size | Transactions | Outputs | Raw JSON | Compressed | Ratio |
|-------------|--------------|---------|----------|------------|-------|
| Light | 50 | 100 | 47 KB | **2.1 KB** | 4.5% |
| Moderate | 500 | 1,000 | 462 KB | **18.5 KB** | 4.0% |
| Heavy | 2,000 | 5,000 | 2.1 MB | **82 KB** | 3.9% |
| Very Heavy | 5,000 | 10,000 | 4.6 MB | **170 KB** | 3.7% |

**⚠️ CRITICAL: Compress BEFORE encrypting!**

Encrypted data doesn't compress. The correct order is:
```
JSON → gzip → AES-256-GCM → on-chain
```

Not:
```
JSON → AES-256-GCM → gzip → on-chain  ❌ (won't compress)
```

### Cost Analysis

**Fee Assumptions**:
- BSV relay fee: 0.25 sat/byte (conservative; often 0.1 or lower)
- BSV price: ~$40 USD
- Transaction overhead: ~200 bytes (inputs, outputs, signatures)

**Per-Backup Cost by Wallet Size**:

| Wallet Size | Data (compressed) | Total Tx Size | Fee @ 0.5 sat/byte | Fee @ 0.25 sat/byte | Fee @ 0.1 sat/byte |
|-------------|-------------------|---------------|--------------------|--------------------|-------------------|
| Light | 2.1 KB | 2.3 KB | 1,150 sats ($0.0005) | 575 sats ($0.0002) | 230 sats ($0.0001) |
| Moderate | 18.5 KB | 18.7 KB | 9,350 sats ($0.004) | 4,675 sats ($0.002) | 1,870 sats ($0.0008) |
| Heavy | 82 KB | 82.5 KB | 41,250 sats ($0.017) | 20,625 sats ($0.008) | 8,250 sats ($0.003) |
| Very Heavy | 170 KB | 170.7 KB | 85,350 sats ($0.034) | 42,675 sats ($0.017) | 17,070 sats ($0.007) |

**Annual Cost by Update Frequency** (at 0.25 sat/byte):

| Wallet Size | Per Backup | Hourly | Daily | Weekly | Monthly |
|-------------|------------|--------|-------|--------|---------|
| Light | $0.0002 | $1.75/yr | $0.08/yr | $0.01/yr | $0.002/yr |
| Moderate | $0.002 | $17.52/yr | $0.68/yr | $0.10/yr | $0.02/yr |
| Heavy | $0.008 | $70.08/yr | $3.01/yr | $0.43/yr | $0.10/yr |
| Very Heavy | $0.017 | $148.92/yr | $6.23/yr | $0.89/yr | $0.20/yr |

**Key Insight**: Even with daily backups, the cost is **$0.08 - $6.23 per year** — invisible to users.

### Backup Frequency Recommendations

**Tradeoff: Cost vs. Data Integrity**

More frequent backups = less data loss if device is lost, but higher cost.

| Frequency | Annual Cost (Moderate) | Max Data Loss Window | Recommendation |
|-----------|------------------------|----------------------|----------------|
| Every transaction | ~$50-100/yr | None | ❌ Overkill, unnecessary cost |
| Hourly | ~$17/yr | 1 hour | ❌ Still overkill for most users |
| **Daily** | **$0.68/yr** | 24 hours | ✅ **Recommended default** |
| Weekly | $0.10/yr | 7 days | ⚠️ Acceptable for light users |
| Monthly | $0.02/yr | 30 days | ❌ Too risky, certificates could be lost |

**Recommended Strategy: Event-Triggered + Daily Minimum**

```
TRIGGER backup when:
  ├── New certificate acquired
  ├── New counterparty-derived output confirmed
  ├── High-value transaction (> 1M sats)
  ├── Every 10 transactions (batch)
  └── 24 hours since last backup (if ANY changes exist)

DEBOUNCE:
  └── Wait 5-10 minutes after trigger before backing up
      (batches multiple rapid changes into single backup)
```

**Data Integrity Guarantees**:

| Event Type | Backup Latency | Risk of Loss |
|------------|----------------|--------------|
| Certificate acquired | 5-10 min | ✅ Minimal |
| Counterparty output | 5-10 min | ✅ Minimal |
| Regular payments | < 24 hours | ✅ Acceptable |
| Settings change | < 24 hours | ✅ Acceptable |
| Device lost same day as activity | < 24 hours of activity | ⚠️ Possible, but rare |

### Payload Format

**Full wallet backup** (same as local file export):

```json
{
  "version": 1,
  "chain": "main",
  "created_at": "2026-03-16T12:00:00Z",
  "backup_type": "full",
  "wallet_identity_key": "02abc...def",

  "transactions": [ ... ],
  "outputs": [ ... ],
  "proven_txs": [ ... ],
  "certificates": [ ... ],
  "certificate_fields": [ ... ],
  "output_baskets": [ ... ],
  "tx_labels": [ ... ],
  "output_tags": [ ... ],
  "settings": { ... }
}
```

**What's included**:
- All transactions (with rawTx for offline history)
- All outputs (with full derivation data)
- All proven_txs (with merkle proofs)
- All certificates + fields
- Baskets, tags, labels
- Settings

**What's excluded**:
- Mnemonic (user re-enters on recovery)
- Balance cache (recomputed)
- Derived key cache (recomputed)
- Browser-specific data (domain whitelist)

### Encryption

```rust
// Key derivation
let key = HKDF-SHA256(
    master_private_key,
    salt = "hodos-wallet-backup-v1",
    info = "aes-256-gcm"
);

// Encryption
let nonce = random_12_bytes();
let (ciphertext, tag) = AES-256-GCM::encrypt(key, nonce, compressed_json);

// On-chain format
let payload = nonce || ciphertext || tag;  // 12 + len + 16 bytes
```

Only the holder of the master private key (derived from mnemonic) can decrypt.

### Lifecycle

**Create/Update Backup**:
```
1. Check for existing backup UTXO at derived address
2. Serialize full wallet state to JSON
3. Compress with gzip (level 9)
4. Encrypt with AES-256-GCM (key from master privkey via HKDF)
5. Build PushDrop transaction:
   - Input: previous backup UTXO (or any wallet UTXO for first backup)
   - Output: PushDrop to backup address, 1000 sats, contains encrypted payload
6. Broadcast transaction
7. Update local `last_backup_txid`
```

**Recovery from Mnemonic Only**:
```
1. Derive master key from mnemonic
2. Derive backup address: BRC-42(master, self, "1-wallet-backup-1")
3. Query blockchain for UTXO at this address
4. If found:
   a. Fetch full transaction
   b. Extract PushDrop payload
   c. Decrypt using master private key
   d. Decompress JSON
   e. Initialize new wallet DB with recovered data
   f. Recompute balance cache
   → Full wallet restored in seconds!
5. If not found:
   a. Brand new wallet, OR
   b. No backups ever made (shouldn't happen after MVP)
   → Fall back to sequential scanning for self-derived addresses
```

**Multi-Device Sync**:
```
Device A creates transaction:
  1. Update local DB
  2. Trigger backup (per frequency rules)
  3. Broadcast (spends old backup UTXO, creates new)

Device B syncs:
  1. Check backup address for UTXO
  2. If txid != local last_sync_txid:
     a. Fetch and decrypt new backup
     b. Merge into local DB (newer updated_at wins)
     c. Update last_sync_txid
```

### Rust Implementation

```
New files:
  src/backup/onchain.rs           — PushDrop backup create/update/recover
  src/backup/onchain_pushdrop.rs  — PushDrop encode/decode for backup
  src/backup/compression.rs       — gzip compress/decompress

Modified files:
  src/monitor/task_onchain_backup.rs  — Background task: check triggers,
                                         debounce, create backup when needed
  src/main.rs                          — Register backup task with monitor
  src/handlers.rs                      — Recovery endpoint uses onchain restore
  src/backup/encryption.rs             — Add HKDF key derivation for backup
```

### Constants

```rust
/// BRC-42 protocol ID for backup address derivation
const BACKUP_PROTOCOL_ID: [u8; 2] = [1, 0];  // [1, "wallet-backup"]
const BACKUP_KEY_ID: &str = "1";
const BACKUP_INVOICE: &str = "1-wallet-backup-1";

/// Satoshis for backup UTXO (above dust, recoverable)
const BACKUP_TOKEN_AMOUNT: i64 = 1000;

/// HKDF salt for encryption key derivation
const BACKUP_ENCRYPTION_SALT: &[u8] = b"hodos-wallet-backup-v1";

/// Backup trigger thresholds
const BACKUP_TX_BATCH_SIZE: u32 = 10;  // Backup every N transactions
const BACKUP_DEBOUNCE_MS: u64 = 300_000;  // 5 minutes
const BACKUP_MAX_INTERVAL_HOURS: u32 = 24;  // At least daily if changes exist

/// High-value transaction threshold (triggers immediate backup)
const HIGH_VALUE_THRESHOLD_SATS: i64 = 1_000_000;  // 1M sats
```

### Edge Cases

**Concurrent Updates (Two Devices Backup Simultaneously)**:
```
Problem: Device A and B both try to spend the same backup UTXO

Solution:
1. Before building backup tx, fetch current UTXO at backup address
2. Build tx spending that specific UTXO
3. Broadcast
4. If broadcast fails (UTXO already spent):
   a. Wait 10-30 seconds
   b. Fetch new backup from chain
   c. Merge local changes with fetched data
   d. Retry backup with new UTXO as input
```

**Network Offline**:
```
1. Set dirty flag when backup is due but network unavailable
2. Queue backup request
3. When network returns, process queue
4. If multiple queued, only latest state needs backup
```

**First Backup (No Previous UTXO)**:
```
1. Use any spendable wallet UTXO as input
2. Create PushDrop output at backup address
3. Track txid for future updates
```

### Comparison: Original vs Revised Approach

| Aspect | Original (OP_RETURN, partial) | Revised (PushDrop, full) |
|--------|------------------------------|--------------------------|
| Data backed up | Counterparty outputs + certs only | **Entire wallet DB** |
| Recovery model | Scan self-derived + check backup | **Single address lookup** |
| Address type | BIP32 m/2147483647 | **BRC-42 self-counterparty** |
| Storage method | OP_RETURN + P2PKH marker | **PushDrop (single output)** |
| Sats recoverable? | No (OP_RETURN unspendable) | **Yes (spend on update)** |
| Multi-device sync | Manual / needs cloud | **Built-in via chain** |
| Size per backup | 2-15 KB | 2-170 KB |
| Annual cost (heavy user) | ~$1.50 | ~$3-7 |
| Complexity | Higher (scanning + backup) | **Lower (one mechanism)** |

**Verdict**: The revised approach costs marginally more (~$5/year) but provides dramatically simpler recovery and built-in multi-device sync

---

## 5. Mechanism C: Cloud Sync Scaffolding

### Purpose

Prepare the codebase for future cloud sync without implementing the transport layer. The data format and serialization code from Mechanism A is reused. Only the "push to server / pull from server" part is left as scaffolding.

### Rust Implementation

```
New files:
  src/sync/mod.rs             — Module root with SyncProvider trait
  src/sync/provider.rs        — trait SyncProvider {
                                   async fn push(data: BackupPayload) -> Result<()>;
                                   async fn pull(identity_key: &str) -> Result<BackupPayload>;
                                   async fn get_sync_state() -> Result<SyncState>;
                                 }
  src/sync/local_provider.rs  — Implements SyncProvider for local file (wraps Mechanism A)
  src/sync/remote_provider.rs — Stub/commented-out HTTP-based provider
                                 // TODO: implement when cloud sync server is available
                                 // Uses same BackupPayload JSON format

Modified files:
  src/database/sync_state_repo.rs  — Already created in transition plan Phase 5
  src/main.rs                       — SyncProvider registered but remote disabled
```

### SyncProvider Trait

```rust
/// Trait for wallet backup/sync providers.
/// All providers use the same BackupPayload format (BSV SDK compatible).
///
/// Implementations:
///   - LocalFileProvider: export/import .bsv-wallet files
///   - OnChainProvider: on-chain encrypted backup (Mechanism B)
///   - RemoteProvider: cloud sync (future, commented out)
pub trait SyncProvider: Send + Sync {
    /// Push current wallet state to backup destination
    async fn push(&self, payload: &BackupPayload) -> Result<(), SyncError>;

    /// Pull wallet state from backup destination
    async fn pull(&self, identity_key: &str) -> Result<Option<BackupPayload>, SyncError>;

    /// Check if backup is newer than local state
    async fn needs_sync(&self, local_updated_at: i64) -> Result<bool, SyncError>;
}
```

### Multi-Device Sync via On-Chain Backup

As you noted, the on-chain backup mechanism could serve as a basic cross-device sync:

```
Device A creates BRC-42 output
  → Device A updates on-chain backup
  → Device B scans backup address (periodic or on-demand)
  → Device B finds updated backup, decrypts
  → Device B now knows about Device A's output
```

This is eventual consistency with the blockchain as the sync transport. Not real-time, but functional. The cloud sync scaffolding could be wired to use the on-chain backup as its provider instead of (or alongside) a remote server.

---

## 6. Recovery Flows

### Flow 1: Mnemonic-Only Recovery (On-Chain Backup)

```
User has: mnemonic only (lost device, no backup file)

Step 1: Derive master key from mnemonic
Step 2: Scan BRC-42 self-derived addresses sequentially:
         For index 0, 1, 2, ... up to gap limit (20 consecutive empty):
           invoice_number = "2-receive address-{index}"
           child_key = BRC-42(master_privkey, master_pubkey, invoice_number)
           address = P2PKH(child_key)
           → Query WhatsOnChain for UTXOs at this address
         → Recovers all BRC-42 self-derived outputs (the standard case)
Step 2b: (Optional) Also scan legacy BIP32 addresses m/0, m/1, m/2...
         → Recovers any legacy BIP32 outputs from before BRC-42 migration
Step 3: Check backup address m/{BACKUP_INDEX}
         → If UTXO found: decrypt OP_RETURN payload
         → Recover non-sequential derivation data (counterparty BRC-42
           outputs where senderIdentityKey != self), certificates, baskets
Step 4: For each recovered counterparty-derived output:
         → Derive key using recovered derivationPrefix/Suffix/senderIdentityKey
         → Verify output exists on-chain (query by txid:vout)
         → If exists and unspent: add to wallet as spendable
Step 5: Recompute balance from all recovered outputs

Result: Full recovery — self-derived outputs from scanning + counterparty
        outputs from on-chain backup
```

**Key insight**: BRC-42 self-derivation with sequential indices (`"2-receive address-{0..N}"`)
is fully deterministic from the seed phrase — no backup needed for these. The on-chain backup
(Step 3) only needs to store counterparty-derived outputs and certificates. This keeps the
on-chain payload small.

### Flow 2: Mnemonic + Backup File Recovery

```
User has: mnemonic + .bsv-wallet file

Step 1: Derive master key from mnemonic
Step 2: Decrypt .bsv-wallet file
Step 3: Validate identity key matches
Step 4: Import all entities (transactions, outputs, proofs, certs)
Step 5: Verify on-chain state for each output
Step 6: Also check on-chain backup (may be newer than file)
         → Merge any newer data from on-chain backup
Step 7: Recompute balance

Result: Full recovery (file may be more complete than on-chain backup
        since it includes full transaction history, not just derivation data)
```

### Flow 3: Normal Startup (No Recovery)

```
Wallet starts normally with existing database

Step 1: Load database
Step 2: Check on-chain backup currency
         → If local data has non-HD outputs not yet backed up:
           set dirty flag for background backup task
Step 3: Resume normal operation
```

### Flow 4: External BIP32 Wallet Import (Future)

```
User has: BIP32 seed phrase or private key from another wallet (not Hodos)

Step 1: User enters external seed phrase/privkey and marks it as "BIP32"
Step 2: Derive BIP32 master key from seed
Step 3: Scan BIP32 addresses m/0, m/1, m/2... up to gap limit
         → Discover all UTXOs at BIP32-derived addresses
Step 4: For each discovered UTXO:
         → Create a transaction spending the BIP32 output
         → Send to a new BRC-42 self-derived address in the Hodos wallet
         → This effectively "sweeps" BIP32 funds into the BRC-42 system
Step 5: After sweep transactions confirm:
         → All funds are now at BRC-42 self-derived addresses
         → Fully recoverable from the Hodos seed phrase
         → No ongoing dependency on the external BIP32 seed

Result: External BIP32 wallet funds migrated into Hodos BRC-42 system.
        User only needs to remember their Hodos seed phrase going forward.
```

**Note**: This flow is not implemented in the current transition plan phases. It is documented
here for planning awareness. The BIP32 derivation code is preserved and separated during
Phase 7 of the transition plan specifically to support this future capability.

**One-time migration for existing Hodos users**: Existing Hodos wallets may have legacy BIP32
outputs from before the BRC-42 transition. These can use the same sweep mechanism — spend
BIP32 outputs to BRC-42 self-derived addresses. After migration, the BIP32 fallback in the
signing path is no longer needed.

---

## 7. Implementation Phases

This work is separate from but dependent on the main transition plan phases.

### Phase B1: Backup Entity Format + Local File Export/Import

**Depends on**: Transition Plan Phase 1 (status consolidation) at minimum. Ideally also Phase 3 (users table) so userId is available.

**Scope**:
- Define `BackupPayload` serde structs matching the JSON format
- Implement AES-256-GCM encryption/decryption with key derivation
- Implement `/wallet/export` endpoint (serialize → encrypt → file)
- Implement `/wallet/import` endpoint (decrypt → validate → merge)
- File format with magic bytes, version, encryption metadata

**Files**:
```
NEW:  src/backup/mod.rs
NEW:  src/backup/entities.rs
NEW:  src/backup/export.rs
NEW:  src/backup/import.rs
NEW:  src/backup/encryption.rs
MOD:  src/handlers.rs (add export/import endpoints)
MOD:  src/main.rs (register routes)
```

**Testing**:
- Create a wallet with transactions, outputs, certificates
- Export to .bsv-wallet file
- Create a fresh wallet from same mnemonic
- Import the file
- Verify all data matches original

---

### Phase B2: On-Chain Encrypted Backup

**Depends on**: Phase B1 (reuses encryption code), Transition Plan Phase 2 (proven_txs for proof tracking), Transition Plan Phase 4 (outputs table with derivation fields).

**Scope**:
- Implement backup transaction builder (P2PKH + OP_RETURN)
- Implement backup payload serializer (non-HD subset)
- Implement backup update logic (spend previous backup UTXO)
- Implement recovery scanner (find backup at known address, decrypt)
- Background task to update backup when dirty flag is set

**Files**:
```
NEW:  src/backup/onchain.rs
NEW:  src/backup/onchain_tx.rs
NEW:  src/monitor/task_onchain_backup.rs
MOD:  src/handlers.rs (recovery flow uses onchain restore)
MOD:  src/main.rs (register backup monitor task)
```

**Testing**:
- Create wallet, perform BRC-42 interactions
- Verify backup transaction broadcast with correct structure
- Verify backup UTXO at expected address
- Delete database, recover from mnemonic only
- Verify non-HD outputs recovered from on-chain backup
- Verify certificates recovered
- Verify backup update replaces previous (spends old UTXO)

---

### Phase B3: Cloud Sync Scaffolding

**Depends on**: Phase B1 (same payload format), Transition Plan Phase 5 (sync_states table).

**Scope**:
- Define `SyncProvider` trait
- Implement `LocalFileProvider` (wraps Phase B1 export/import)
- Implement `OnChainProvider` (wraps Phase B2)
- Stub `RemoteProvider` with commented-out HTTP implementation
- Wire `SyncProvider` into AppState

**Files**:
```
NEW:  src/sync/mod.rs
NEW:  src/sync/provider.rs
NEW:  src/sync/local_provider.rs
NEW:  src/sync/onchain_provider.rs
NEW:  src/sync/remote_provider.rs (commented out / stub)
MOD:  src/main.rs (register SyncProvider)
```

**Testing**:
- Verify LocalFileProvider produces same output as direct export
- Verify OnChainProvider triggers backup correctly
- Verify RemoteProvider compiles but is inactive

---

## 8. Relationship to Transition Plan

### Dependency Map

```
Transition Plan                    Backup Plan
───────────────                    ───────────
Phase 1: Status Consolidation ──────► Phase B1: Local File Export
Phase 2: Proven Transaction Model      (can start after Phase 1)
Phase 3: Multi-User Foundation ────►
Phase 4: Output Model Transition ──► Phase B2: On-Chain Backup
Phase 5: Supporting Tables ────────► Phase B3: Cloud Sync Scaffolding
Phase 6: Monitor Pattern ─────────► Phase B2: (backup task in monitor)
Phase 7: Per-Output Key Derivation   (benefits from all backup phases)
Phase 8: Cleanup
```

### Suggested Order

```
1. Transition Phase 1 (status consolidation)
2. Transition Phase 2 (proven_txs)
3. Transition Phase 3 (users)
4. Backup Phase B1 (local file export/import)     ← can start here
5. Transition Phase 4 (outputs model)
6. Transition Phase 5 (supporting tables)
7. Backup Phase B3 (sync scaffolding)
8. Transition Phase 6 (monitor pattern)
9. Backup Phase B2 (on-chain backup)               ← needs monitor + outputs
10. Transition Phase 7 (per-output key derivation)
11. Transition Phase 8 (cleanup)
```

Phase B2 (on-chain backup) is positioned after the monitor pattern (Phase 6) so it can run as a proper background task, and after the outputs model (Phase 4) so derivation fields are available.

---

## 9. MVP & Planning Notes (Feb 2026)

These notes shape what we build and test in the MVP and how we coordinate with the Phase-1 Initial Setup/Recovery UX sprint.

### Cloud Sync: Scaffolding Only for MVP

- **Implement**: Scaffolding only (e.g. `SyncProvider` trait, stub `RemoteProvider`). No cloud transport implementation or testing in MVP.
- **Goal**: Codebase is ready to plug in cloud sync later without reworking the backup format or serialization.

### Local File Backup: Plan and Test in MVP

- **Implement and test**: Local encrypted file export and import (Phase B1).
- **Goal**: Users can export a `.bsv-wallet` file and recover from it; we test this path end-to-end in MVP.

### External Wallet Import (TypeScript BSV/SDK Compatibility)

- **Planning**: As part of Phase-1 planning, export a backup from another wallet built with the TypeScript BSV/SDK and inspect the format.
- **Goal**: Mirror that format in our export (backup) and import code so we can recover from their exports and they can recover from ours. Ideally the format is just a JSON structure.
- **Consideration**: Handle **camelCase (TypeScript/JSON) ↔ snake_case (Rust/DB)** in file recovery. Our backup entity format should support round-tripping with SDK-style camelCase field names when deserializing; serialize to a consistent format (e.g. snake_case for our file, or match SDK exactly). Add tests that import a sample export from the other wallet.

### On-Chain Backup: Novel to Our Wallet; Compress Before Storing

- **Scope**: On-chain backup/recovery is novel to our wallet. We will only be able to test with our own backups (no third-party consumer in MVP).
- **Format**: Same JSON (or the non-HD subset) **encrypted** and stored on-chain. Option under consideration: **PushDrop token with self-counterparty** (so only we can read/reclaim); alternative is OP_RETURN as in Section 4 (simpler, unspendable).
- **Compression**: **Compress the payload before encrypting** and before putting it into the token/OP_RETURN to save space and cost (e.g. zstd or gzip). Decompress after decrypt during recovery. Size estimates in Section 2 already suggest ~40–60% reduction with compression; make compression a required step for on-chain backup.

### Coordination with Phase-1 Initial Setup/Recovery (UX)

- Phase-1 (UX) provides the UI for: create wallet, recover from mnemonic, **recover from backup file**.
- **Phase B1 (local file export/import)** should be implemented **before or in parallel with** Phase-1 so that “Recover from file” has a real backend. Phase B2 (on-chain) and B3 (cloud scaffolding) can follow.
- See `development-docs/UX_UI/phase-1-initial-setup-recovery.md` for the interface plan and triggers.

---

## 10. Open Questions

These will be answered during implementation:

1. ~~**Encryption key source**~~: **RESOLVED (2026-02-11)**
   - **On-chain backup (Mechanism B)**: Encryption key derived from mnemonic via HKDF-SHA256. Anyone with the mnemonic can decrypt — this is intentional, since mnemonic-only recovery must work without any other secret.
   - **Local file backup (Mechanism A)**: Encryption key derived from a **user-provided PIN/passphrase** (not the mnemonic). The PIN is created during wallet setup (Phase 1) and entered during file import. This means the `.bsv-wallet` file is useless without both the file AND the PIN — an extra layer of protection for exported files.
   - **Implication**: The file format encryption byte distinguishes the two: `0x01 = AES-256-GCM, mnemonic-derived` (on-chain), `0x02 = AES-256-GCM, user-PIN-derived` (file export).

2. **Backup update trigger**: Timer-based (daily), event-based (on confirmed non-HD output), or on wallet shutdown? Probably a combination.

3. **Backup UTXO amount**: Dust limit (546 sat) is cheapest but might be swept by cleanup. Consider a slightly higher amount (1000 sat) for safety.

4. **OP_RETURN vs PushDrop**: OP_RETURN is simpler and unspendable. PushDrop would make the data spendable but adds complexity. OP_RETURN is likely the right choice since we only need the data, not a spendable token.

5. **Backup history**: Keep only the latest backup, or maintain a chain of incremental backups? Latest-only is simpler and sufficient. The old backup UTXO is spent as input to the new one, so only the latest exists as a UTXO.

6. **Maximum payload size**: BSV supports very large transactions, but at what point should we split the backup across multiple OP_RETURNs? Likely not needed until thousands of BRC-42 interactions.

7. **Backup verification**: Should the wallet periodically verify it can decrypt its own on-chain backup? A self-test could catch encryption issues early.

8. **BIP32 wallet import UX**: When a user imports an external BIP32 wallet, should the sweep (BIP32 → BRC-42) be automatic or require user confirmation for each output? Automatic is simpler but the user may want to review before spending.

---

## 11. Future: External BIP32 Wallet Import

**Status**: Not yet scheduled — documented for planning awareness.

### Concept

Allow users to import funds from any BIP32-compatible wallet (ElectrumSV, HandCash legacy, etc.) into the Hodos BRC-42 system. The user provides a BIP32 seed phrase or extended private key, Hodos scans for UTXOs, and sweeps them to BRC-42 self-derived addresses.

### Why Sweep Instead of Dual-Manage

Maintaining both BIP32 and BRC-42 derivation paths permanently would require:
- Two scanning strategies during recovery
- Fallback logic in the signing hot path
- Two derivation systems to test and maintain

Sweeping to BRC-42 eliminates these costs. After sweep, the user only needs their Hodos seed phrase.

### Recovery Model After Phase 7

```
Hodos Recovery Scanner (from seed phrase):
  1. BRC-42 self-derivation scan: "2-receive address-{0..N}" (primary, gap limit 20)
  2. BIP32 legacy scan: m/{0..N} (secondary, for any un-migrated legacy outputs)
  3. On-chain backup at m/{BACKUP_INDEX} (counterparty outputs + certificates)

BIP32 code is preserved in a recovery module (separated from signing hot path
in Transition Plan Phase 7) specifically to support steps 2 and the external
wallet import flow.
```

### Dependencies

- Transition Plan Phase 7 (separates BIP32 code into recovery module)
- Backup Phase B2 (on-chain backup infrastructure for the sweep transactions)

---

---

## 12. Notes from State Maintenance Sprint (Phases 6-8, Feb 2026)

These notes capture things learned during the wallet-toolbox alignment sprint that are relevant to backup/recovery implementation. Review when starting Phase B1.

### All Transition Plan Dependencies Resolved

All transition plan phases are complete. For MVP, the many individual migrations were consolidated into a single migration; the resulting schema reflects the same state (formerly described as schema V24). The dependency map in Section 8 is fully satisfied — Phases B1, B2, and B3 can all be started without waiting on anything.

### Per-Output Key Derivation is Live (Phase 7)

`derive_key_for_output()` in `src/database/helpers.rs` is now the single signing entry point. It reads derivation fields directly from the output record:

| `derivation_prefix` | `derivation_suffix` | `sender_identity_key` | Path |
|---|---|---|---|
| `"2-receive address"` | `"{index}"` | NULL | BRC-42 self-derived (recoverable from seed scan) |
| `"bip32"` | `"{index}"` | NULL | Legacy BIP32 (recoverable from m/{index} scan) |
| NULL | NULL | NULL | Master key directly |
| any | any | Some(pubkey) | BRC-42 counterparty (NEEDS on-chain backup) |

**V23 migration** re-tagged all legacy BIP32 outputs with `derivation_prefix = "bip32"`, so the backup code can cleanly filter which outputs need on-chain backup vs which are seed-recoverable.

### BIP32 Code Separated into Recovery Module

Phase 7D moved `derive_private_key_bip32()` to `src/recovery.rs` (out of the signing hot path). This is exactly where the recovery scanner will live. However, `recover_wallet_from_mnemonic()` in that file still uses old patterns and needs updating for the outputs-based model before it's production-ready.

### Monitor Has Room for Backup Task

The Monitor now runs 7 tasks with graceful shutdown (CancellationToken) and DB lock contention avoidance (try_lock() canary). Adding `task_onchain_backup.rs` as task #8 is straightforward — just add the module and register it in the run loop. The backup task will automatically get clean shutdown and won't block user HTTP requests.

### Dropped Tables

These tables referenced in the plan no longer exist in the current schema:
- `transaction_labels` — consolidated; labels are now in `tx_labels` / `tx_labels_map` only
- `merkle_proofs` — consolidated; proof data is in `proven_txs` only

The backup entity format in Section 2 should serialize from `tx_labels`/`tx_labels_map` (not `transaction_labels`).

### Whitelist Not Yet in Database

The **domain whitelist** is not yet in the database; it is still backed by a JSON file only. At least one additional migration will be needed to add a whitelist table. Design is **deferred until Phase 2 (User Notifications)** in the UX_UI sprint, when we define what permission levels (e.g. spending levels, certificate levels) the user can allow per site. Doing the whitelist table design and migration at that stage keeps the schema aligned with the notification/permission model. See `development-docs/UX_UI/phase-2-user-notifications.md`.

### NoSend Transaction Semantics — Important for Recovery

`noSend=true` means the wallet doesn't broadcast, but the APP does via overlay network. Inputs from nosend txs ARE genuinely spent on-chain. During recovery:
- Do NOT treat nosend txs as "never broadcast" — their inputs are gone
- Externally-spent outputs are marked `spending_description = 'external-spend'`, `spent_by = NULL`, `spendable = 0`
- Recovery should preserve these markers rather than re-scanning all outputs as spendable

### Balance Cache

`state.balance_cache.invalidate()` must be called after any import/merge operation that modifies outputs. The balance cache has startup seeding and stale fallback, so recovery just needs to invalidate after import and the next balance read will recompute.

### Output Tag Map FK Fixed

The consolidated migration rebuilt `output_tag_map` with correct FK to `outputs(outputId)` instead of `utxos(id)`. The backup entity format for output tags should reference `outputId` (not the old utxo id).

### Existing backup.rs

There is already a `src/backup.rs` file with basic DB file copy logic. Phase B1 will likely replace or significantly expand this into the `src/backup/` module structure described in Section 3.

---

## 13. Research Findings — BSV SDK Sync Protocol (2026-02-14)

### SDK Sync Architecture

The BSV SDK wallet-toolbox uses a **chunk-based bidirectional replication protocol** over JSON-RPC 2.0:

- **Transport**: JSON-RPC 2.0 over HTTPS, single `POST /` endpoint
- **Authentication**: BRC-103/104 mutual authentication
- **Hosted services**: `storage.babbage.systems`, `store.txs.systems`
- **Trigger**: On-demand only (no automatic timer — `TaskSyncWhenIdle` is a stub)
- **Conflict resolution**: Last-write-wins based on `updated_at` timestamps

### Sync Entity Order (dependency chain)

```
1. provenTx        (immutable; never updated after creation)
2. provenTxReq     (references provenTx)
3. outputBasket    (must exist before outputs)
4. txLabel         (must exist before txLabelMaps)
5. outputTag       (must exist before outputTagMaps)
6. transaction     (references provenTx)
7. output          (references transaction, basket)
8. txLabelMap      (references txLabel, transaction)
9. outputTagMap    (references outputTag, output)
10. certificate    (must exist before certificateFields)
11. certificateField (references certificate)
12. commission     (references transaction)
```

### Key Protocol Methods

| Method | Purpose |
|--------|---------|
| `getSyncChunk` | Pull incremental data (paginated by offsets per entity type) |
| `processSyncChunk` | Push data, merge with ID remapping |
| `findOrInsertSyncStateAuth` | Initialize sync session |
| `makeAvailable` | Initialize storage, get table settings |

### SyncChunk Payload Shape

```json
{
  "fromStorageIdentityKey": "02abc...",
  "toStorageIdentityKey": "02def...",
  "userIdentityKey": "02abc...",
  "user": { "userId": 1, "identityKey": "02abc...", "activeStorage": "local" },
  "provenTxs": [ ... ],
  "transactions": [ ... ],
  "outputs": [ ... ],
  "certificates": [ ... ],
  "certificateFields": [ ... ],
  "outputBaskets": [ ... ],
  "txLabels": [ ... ],
  "txLabelMaps": [ ... ],
  "outputTags": [ ... ],
  "outputTagMaps": [ ... ],
  "commissions": [ ... ],
  "provenTxReqs": [ ... ]
}
```

All entity field names use **camelCase** (e.g., `transactionId`, `basketId`, `provenTxId`).
Binary data (rawTx, lockingScript, merklePath, inputBEEF) encoded as `number[]` arrays.
Timestamps use `created_at` / `updated_at` (snake_case exception).

### ID Remapping During Merge

Each entity type maintains an `idMap: Record<number, number>` mapping foreign IDs to local IDs.
When inserting an entity from a remote chunk:
1. Look up all FK references in the appropriate idMap (e.g., `output.transactionId` → `syncMap.transaction.idMap[foreignTxId]`)
2. Reset primary key to 0 (let local DB assign)
3. Insert and save the new local ID into the idMap

### Decision: Our Format Matches SDK

Our `BackupPayload` serde structs will use the **exact same field names and entity shapes** as the SDK's `Table*` interfaces. This gives us:
- Import/export compatibility with SDK wallets
- Shared serialization code between local file backup, on-chain backup, and future cloud sync
- The ability to act as a `StorageProvider` for SDK-compatible sync clients

### Cloud Sync Cost Estimate

| Scale | Infrastructure | Monthly Cost |
|-------|---------------|-------------|
| <100 users | Single VPS + SQLite | ~$5/mo |
| 100-1000 users | VPS + PostgreSQL | ~$15-30/mo |
| 1000+ users | Managed DB + load balancer | ~$50-100/mo |

---

## 14. External Wallet Recovery — Centbee (Deferred)

### Background

Centbee BSV wallet is **closing April 1, 2026**. Self-custodial wallet; users can recover funds independently with seed phrase + PIN.

### Derivation Scheme

| Property | Value |
|----------|-------|
| Mnemonic | 12 words, BIP39 standard |
| PIN | 4-digit, used as **BIP39 passphrase** (not separate encryption) |
| Seed | `PBKDF2-HMAC-SHA512(mnemonic, "mnemonic" + PIN, 2048)` |
| Receive path | `m/44'/0/0/{index}` |
| Change path | `m/44'/0/1/{index}` |
| Hardened levels | Only purpose `44'`; coin type `0` and account `0` are **normal** |
| Address format | P2PKH mainnet |

**Critical**: Wrong PIN → completely different addresses (PIN is baked into seed derivation).

### Recovery Implementation (when scheduled)

1. UI: "Recover from Centbee" button with mnemonic + PIN inputs
2. `mnemonic.to_seed(pin_string)` — PIN as BIP39 passphrase
3. Scan receive `m/44'/0/0/{i}` and change `m/44'/0/1/{i}` up to gap limit
4. Sweep all found UTXOs to user's Hodos BRC-42 address
5. Edge case: if no funds found, warn that PIN may be incorrect

### Sources

- Recovery tutorial by "Truth Machine" (Medium)
- BSV Hub "Mnemonic to BRC-100" tool
- BSV Wallet Derivation Paths reference (juniper.nz)
- ElectrumSV Issue #59

---

*End of backup and recovery outline. All transition plan dependencies are resolved — implementation can begin at any time.*
