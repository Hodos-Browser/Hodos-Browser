# Wallet Backup & Recovery — Implementation Outline

**Created**: 2026-02-03
**Status**: Draft — design outline for review
**Related**: `STATE_MAINTENANCE_AND_RECONCILIATION_TRANSITION_PLAN.md` (Phases 2, 5, 7)
**Objective**: Enable full wallet recovery from mnemonic alone by combining three backup mechanisms — local file export, on-chain encrypted backup, and cloud sync scaffolding.

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
│  Layer 1: On-Chain Encrypted Backup (automatic)         │
│  Mnemonic-only recovery. Encrypted derivation data      │
│  stored in a BIP32 UTXO on the blockchain.              │
│  Always available. No user action needed.               │
├─────────────────────────────────────────────────────────┤
│  Layer 2: Local Encrypted Backup File (user-initiated)  │
│  Full wallet export in BSV SDK sync-compatible JSON.     │
│  User saves to USB, cloud drive, etc.                   │
├─────────────────────────────────────────────────────────┤
│  Layer 3: Cloud Sync (future, scaffolding only)         │
│  Same JSON format as Layer 2, pushed to remote storage. │
│  Infrastructure for multi-device sync.                  │
│  Commented out / behind feature flag until needed.      │
└─────────────────────────────────────────────────────────┘
```

All three layers use the **same JSON entity format** matching the BSV SDK wallet-toolbox sync protocol. Data serialized for one layer can be deserialized by any other.

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

## 4. Mechanism B: On-Chain Encrypted Backup

### Concept

Store encrypted derivation data for **counterparty-derived** outputs in a special UTXO on the blockchain. Self-derived BRC-42 outputs (`"2-receive address-{0..N}"`) are recoverable from the seed alone by sequential scanning and do NOT need on-chain backup. Only counterparty-derived outputs (where `senderIdentityKey != self`), certificates, and baskets need to be stored. The backup UTXO lives at a deterministic BIP32 address derived from the mnemonic, so it can be found during recovery by scanning sequential addresses.

### Design

**Backup address derivation**:
```
Backup address = m/{BACKUP_INDEX}
where BACKUP_INDEX is a well-known constant (e.g., 2147483647 = max i32)
This is high enough to never collide with normal HD address indices.
```

**Transaction structure**:
```
Input:  previous backup UTXO (or any wallet UTXO for first backup)
Output 0: P2PKH to backup address (dust amount, e.g., 546 sat)
            — this is the "marker" that recovery scanning finds
Output 1: OP_RETURN
            OP_FALSE OP_RETURN
            <protocol_flag: "hodos-backup-v1">
            <encrypted_payload>
```

**Encrypted payload contents** (subset of full backup — only what's NOT recoverable from seed scanning):
```json
{
  "version": 1,
  "updated_at": "2026-02-03T12:00:00Z",
  "counterparty_outputs": [
    {
      "txid": "abc123...",
      "vout": 0,
      "derivationPrefix": "2-authrite",
      "derivationSuffix": "session-xyz",
      "senderIdentityKey": "03fed...cba",
      "basketId": 3,
      "customInstructions": null
    }
  ],
  "certificates": [
    {
      "type": "identity",
      "serialNumber": "cert-001",
      "certifier": "03aaa...",
      "subject": "02abc...",
      "signature": "3045...",
      "fields": [ ... ]
    }
  ],
  "baskets": [
    { "name": "auth-tokens" }
  ]
}
```

**What's included vs excluded from on-chain backup**:

| Output Type | In on-chain backup? | Why |
|-------------|--------------------|----|
| BRC-42 self-derived (`"2-receive address-{N}"`, senderIdentityKey=NULL) | NO | Recoverable by sequential scanning from seed |
| Master key outputs (derivation=NULL) | NO | Derivable directly from seed |
| Legacy BIP32 outputs (`"bip32-{N}"`) | NO | Recoverable by BIP32 sequential scanning |
| Counterparty-derived BRC-42 (senderIdentityKey != NULL) | YES | Cannot be guessed — need prefix, suffix, and counterparty pubkey |
| Certificates | YES | Not derivable from seed |
| Basket definitions | YES | Metadata not derivable from seed |

**Note**: This selective approach keeps the on-chain payload small. Most wallet outputs are self-derived receive addresses which don't need backup.

**Encryption**:
```
key = HKDF-SHA256(master_private_key, salt="hodos-onchain-backup", info="v1")
payload = AES-256-GCM(key, nonce=random_12_bytes, plaintext=json_bytes)
on_chain_data = nonce || ciphertext || auth_tag
```

Only the holder of the master private key (derived from mnemonic) can decrypt.

### Lifecycle

```
Trigger: a non-HD output is confirmed (proven)
  OR: periodic timer (e.g., daily if any non-HD data changed)
  OR: wallet shutdown (if dirty flag set)

Create/Update:
  1. Gather all non-HD output derivation data + certificates
  2. Serialize to JSON, encrypt
  3. If previous backup UTXO exists:
       Spend it as input (reclaims the dust)
     Else:
       Use a normal wallet UTXO as input
  4. Create new backup UTXO + OP_RETURN
  5. Broadcast
  6. Track the backup UTXO txid locally (for next update)

Recovery:
  1. Derive backup address from mnemonic: m/{BACKUP_INDEX}
  2. Query blockchain for UTXOs at this address
  3. If found: fetch the transaction, extract OP_RETURN data
  4. Decrypt using master private key
  5. Import non-HD derivation data into database
  6. Now can derive keys for BRC-42 outputs, certificates, etc.
  7. Verify each recovered output still exists on-chain
```

### Rust Implementation

```
New files:
  src/backup/onchain.rs       — On-chain backup create/update/recover
  src/backup/onchain_tx.rs    — Build backup transaction (input + P2PKH + OP_RETURN)

Modified files:
  src/monitor/task_onchain_backup.rs  — Background task: check dirty flag,
                                         create/update backup when needed
  src/main.rs                          — Register backup task with monitor
  src/handlers.rs                      — Recovery endpoint uses onchain restore
```

### Constants

```rust
/// Well-known address index for on-chain backup UTXO
/// Max i32 value — will never collide with normal HD indices
const BACKUP_ADDRESS_INDEX: i32 = 2_147_483_647;

/// Minimum satoshis for backup UTXO (dust limit)
const BACKUP_DUST_AMOUNT: i64 = 546;

/// Protocol flag in OP_RETURN for backup identification
const BACKUP_PROTOCOL_FLAG: &str = "hodos-backup-v1";

/// HKDF salt for on-chain backup encryption key derivation
const BACKUP_ENCRYPTION_SALT: &[u8] = b"hodos-onchain-backup";
```

### Cost Analysis

| Scenario | Backup Size | Tx Size | Fee (~1 sat/byte) |
|---|---|---|---|
| 10 BRC-42 outputs + 2 certs | ~2 KB | ~2.5 KB | ~2,500 sat (~$0.001) |
| 100 BRC-42 outputs + 10 certs | ~15 KB | ~16 KB | ~16,000 sat (~$0.006) |
| 1000 BRC-42 outputs + 50 certs | ~120 KB | ~121 KB | ~121,000 sat (~$0.05) |
| Update frequency: weekly | | | ~$0.05-$2.50/year |

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
