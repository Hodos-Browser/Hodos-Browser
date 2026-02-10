# Wallet Architecture

## Overview

The Hodos Browser wallet is a Rust-based HTTP server (Actix-web) running on `localhost:3301`. It handles all cryptographic operations, key management, transaction building/signing, and BRC-100 protocol endpoints. Private keys never leave this process.

## Architecture

```
React Frontend (Port 5137)
    │ window.hodosBrowser.*
    ▼
C++ CEF Shell (HTTP Interception)
    │ Forwards wallet requests to localhost:3301
    ▼
Rust Wallet Backend (Port 3301)
    │ Actix-web HTTP server
    │ SQLite database (wallet.db, schema V24)
    │ Background Monitor (7 tasks)
    ▼
Bitcoin SV Blockchain
    │ WhatsOnChain API (UTXO lookup, proofs)
    │ ARC / GorillaPool (transaction broadcast)
    ▼
On-chain verification
```

## Core Components

### AppState (src/main.rs)

Shared state accessible to all HTTP handlers:

| Field | Type | Purpose |
|-------|------|---------|
| `database` | `Arc<Mutex<WalletDatabase>>` | SQLite connection (single writer) |
| `balance_cache` | `BalanceCache` | In-memory balance with instant invalidation |
| `current_user_id` | `i64` | Active user ID (default: 1) |
| `shutdown` | `CancellationToken` | Graceful shutdown signal (Ctrl+C) |
| `auth_sessions` | `Arc<Mutex<HashMap>>` | BRC-103/104 auth session state |
| `message_store` | `Arc<Mutex<HashMap>>` | BRC-33 in-memory message relay |
| `pending_transactions` | `Arc<Mutex<HashMap>>` | Two-phase sign: createAction → signAction |
| `fee_rate_cache` | `FeeRateCache` | Cached fee rates from MAPI |

### Database Layer (src/database/)

SQLite with WAL mode, foreign keys enabled. Schema managed through numbered migrations (V1–V24).

**Repository pattern**: Each table group has a dedicated repository struct:

| Repository | Tables | Purpose |
|------------|--------|---------|
| `WalletRepository` | wallets | Master key storage, HD index |
| `UserRepository` | users | Identity mapping (pubkey → userId) |
| `AddressRepository` | addresses | HD address derivation cache |
| `OutputRepository` | outputs | **Primary UTXO tracking** — spendable/spent_by model |
| `TransactionRepository` | transactions | Transaction lifecycle (new_status) |
| `ProvenTxRepository` | proven_txs | Immutable merkle proof records |
| `ProvenTxReqRepository` | proven_tx_reqs | Proof acquisition lifecycle |
| `TxLabelRepository` | tx_labels, tx_labels_map | Normalized transaction labels |
| `TagRepository` | output_tags, output_tag_map | Output tagging/basket assignment |
| `CertificateRepository` | certificates, certificate_fields | BRC-52 identity certificates |
| `CommissionRepository` | commissions | Fee tracking per transaction |
| `SettingsRepository` | settings | Persistent wallet configuration |
| `SyncStateRepository` | sync_states | Multi-device sync state |

### Cryptography (src/crypto/)

| Module | Purpose |
|--------|---------|
| `brc42.rs` | ECDH-based child key derivation (Type-42) |
| `brc43.rs` | Invoice number format: `{securityLevel}-{protocolID}-{keyID}` |
| `signing.rs` | SHA-256, HMAC-SHA256, ECDSA signing |
| `aesgcm_custom.rs` | AES-256-GCM encryption (BRC-2) |
| `brc2.rs` | BRC-2 encrypt/decrypt with BRC-42 key derivation |
| `mod.rs` | Key derivation routing, public key computation |

### Key Derivation (src/database/helpers.rs)

`derive_key_for_output()` is the single entry point for all signing. It reads derivation fields directly from the output record:

| `derivation_prefix` | `derivation_suffix` | `sender_identity_key` | Derivation Path |
|---------------------|---------------------|----------------------|-----------------|
| `"2-receive address"` | `"{index}"` | `None` | BRC-42 self-derivation (standard) |
| `"bip32"` | `"{index}"` | `None` | Legacy BIP32 HD (`m/{index}`) |
| `NULL` | `NULL` | `None` | Master private key directly |
| any | any | `Some(pubkey)` | BRC-42 counterparty derivation |

### Transaction Lifecycle

```
createAction (build + select UTXOs)
    → new_status: 'unsigned'
    → inputs reserved (spent_by set)
    → outputs created (spendable=0)

signAction (sign + broadcast)
    → new_status: 'sending' → 'unproven'
    → proven_tx_req created
    → Monitor acquires proof → 'completed'

On failure:
    → new_status: 'failed'
    → ghost outputs deleted
    → reserved inputs restored (spendable=1)
    → balance cache invalidated
```

### Status System

Single `new_status` column (V15+):

| Status | Meaning |
|--------|---------|
| `unprocessed` | Created, not signed |
| `unsigned` | Awaiting signatures (two-phase) |
| `nosend` | Signed but app broadcasts (overlay) |
| `sending` | Being broadcast |
| `unproven` | Broadcast, awaiting merkle proof |
| `completed` | Has merkle proof (confirmed on-chain) |
| `failed` | Broadcast failed or rejected |

### BEEF/SPV (src/beef.rs, src/beef_helpers.rs)

Transactions are broadcast in BEEF (Background Evaluation Extended Format) which bundles SPV proofs:

- `beef.rs`: BEEF parser, TSC proof ↔ BUMP conversion
- `beef_helpers.rs`: Recursive ancestry chain building with proof fetching
- `parent_transactions` table: Raw tx cache for BEEF building
- `proven_txs` table: Immutable merkle proof records

## Background Services — Monitor Pattern

The Monitor (`src/monitor/mod.rs`) runs as a single tokio task with a 30-second tick loop. It checks `CancellationToken` for shutdown and uses `try_lock()` to avoid blocking user HTTP requests.

| Task | Interval | Purpose |
|------|----------|---------|
| TaskCheckForProofs | 60s | Acquire merkle proofs (ARC → WoC fallback) |
| TaskSendWaiting | 120s | Crash recovery for stuck `sending` txs |
| TaskFailAbandoned | 300s | Fail stuck unprocessed/unsigned txs, clean ghost outputs |
| TaskUnFail | 300s | Recover false failures (6-hour window, on-chain check) |
| TaskReviewStatus | 60s | Status consistency: proven_tx_reqs → transactions → outputs |
| TaskPurge | 3600s | Cleanup old events (7d) and completed proof requests (30d) |
| TaskSyncPending | 30s | UTXO sync for addresses with `pending_utxo_check=1` |

### Ghost Transaction Safety Rules

1. Background tasks never create output records — only sync from API via `/wallet/sync`
2. Delete ghost outputs BEFORE restoring inputs on failure
3. TaskUnFail does NOT re-create deleted outputs — relies on `/wallet/sync`
4. Always invalidate balance cache after output changes
5. Cleanup order: mark failed → delete ghost outputs → restore inputs → invalidate cache

## UTXO Synchronization

Two mechanisms:

1. **Periodic (TaskSyncPending)**: Monitor checks addresses with `pending_utxo_check=1` every 30s
2. **On-demand (`POST /wallet/sync`)**: Frontend or manual trigger, supports `?full=true` for all addresses

The sync endpoint:
- Fetches UTXOs from WhatsOnChain for target addresses
- Inserts new outputs via `upsert_received_utxo()`
- Reconciles stale outputs: marks DB outputs not found in API as `external-spend`
- Invalidates balance cache

## API Endpoints

### Wallet Operations
| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Health check |
| GET | `/wallet/status` | Wallet initialization status |
| GET | `/wallet/balance` | Cached balance (instant) |
| POST | `/wallet/sync` | On-demand UTXO sync with reconciliation |

### BRC-100 Protocol (26/28 methods)
| Method | Path | Purpose |
|--------|------|---------|
| POST | `/getPublicKey` | Identity/derived public key |
| POST | `/.well-known/auth` | BRC-103/104 mutual authentication |
| POST | `/createAction` | Build + sign transactions |
| POST | `/signAction` | Complete two-phase signing |
| POST | `/listOutputs` | Query outputs by basket/tag |
| POST | `/listCertificates` | Query identity certificates |
| POST | `/acquireCertificate` | Acquire new certificate |
| POST | `/encrypt` | BRC-2 AES-256-GCM encryption |
| POST | `/decrypt` | BRC-2 decryption |

### Fee Calculation

Dynamic size-based fees (not hardcoded):
- Default: 1 sat/byte (1000 sat/KB)
- Minimum: 200 satoshis
- Two-pass: estimate → select UTXOs → recalculate with actual inputs

## Database Schema (V24)

Current migration version: **V24** (24 migrations total).

### Active Tables

| Table | Purpose |
|-------|---------|
| wallets | Master key storage (mnemonic, HD index) |
| users | Identity mapping (master pubkey → userId) |
| addresses | HD address derivation cache |
| transactions | Transaction lifecycle with `new_status` |
| outputs | **Primary** — wallet-toolbox compatible UTXO tracking |
| parent_transactions | Raw tx cache for BEEF building |
| block_headers | Cached block headers |
| proven_txs | Immutable merkle proof records |
| proven_tx_reqs | Proof acquisition lifecycle tracking |
| baskets | Output categorization by user |
| output_tags | Tag definitions |
| output_tag_map | Output ↔ tag junction (FK to outputs) |
| certificates | BRC-52 identity certificates |
| certificate_fields | Certificate field values (encrypted) |
| tx_labels | Deduplicated label entities per user |
| tx_labels_map | Label ↔ transaction junction |
| commissions | Fee tracking per transaction |
| settings | Persistent wallet configuration |
| sync_states | Multi-device synchronization state |
| monitor_events | Background task event logging |
| transaction_inputs | Transaction input details |
| transaction_outputs | Transaction output details |

### Tables Dropped in V24

| Table | Reason |
|-------|--------|
| merkle_proofs | Replaced by `proven_txs` (V16) |
| domain_whitelist | JSON file used instead |
| transaction_labels | Replaced by `tx_labels`/`tx_labels_map` (V19) |

### Tables Deferred for Future Cleanup

| Table | Reason Still Exists |
|-------|-------------------|
| utxos | ~10 live code references in handlers.rs, cache_helpers.rs |
| transactions.status/broadcast_status | ~15 live references to update_broadcast_status() |

## Security Model

1. **Private keys never leave Rust** — all signing in `crypto/` module
2. **Memory safety** — Rust ownership model, no `unsafe` blocks in key-handling code
3. **Process isolation** — wallet runs as separate process from browser
4. **Parameterized SQL** — all queries use rusqlite params, no string interpolation
5. **App-scoped identity keys** — BRC-103/104 returns derived keys per app, preventing cross-app tracking
6. **Balance cache** — invalidated immediately on any output-modifying operation
