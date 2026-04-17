# On-Chain Wallet Backup System

Automatically backs up the encrypted wallet database as a PushDrop UTXO on-chain. Recoverable from the 12-word mnemonic alone — zero coins required for recovery.

## Architecture

```
Write path:
  DB tables → JSON payload → optimize/strip → gzip → AES-256-GCM encrypt → PushDrop tx (1000 sats) + P2PKH marker (546 sats) → broadcast

Read path (recovery):
  Mnemonic → derive master key → derive backup address (BRC-42, invoice "1-wallet-backup-1", index -3)
  → query WoC for marker UTXOs at address → fetch parent tx → parse PushDrop script → decrypt → gunzip → JSON → restore DB
```

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **PushDrop** (not OP_RETURN) | Spendable — recovers 1000 sats on each update. OP_RETURN burns forever |
| **Deterministic address** | BRC-42 self-counterparty, same address every time. No coordination needed |
| **Zero-coin recovery** | Read-only chain queries via WoC. User needs only mnemonic |
| **Gzip before encrypt** | Compression on plaintext (96-97% ratio). Ciphertext doesn't compress |
| **AES-256-GCM** | Key = SHA256(master_privkey \|\| "hodos-wallet-backup-v1"). No extra password |
| **Marker output** | 546-sat P2PKH at the backup address. Discovery anchor — scan this address, find the marker, get the PushDrop txid |

## Storage Format

Each backup is a single transaction with:
- **Output 0**: PushDrop token (1000 sats) containing the encrypted backup payload
- **Output 1**: P2PKH marker (546 sats) at the deterministic backup address for discovery
- **Output 2**: Change (returned to wallet)
- **Service fee**: 1000 sats to Hodos treasury (standard for all wallet transactions)

The PushDrop script embeds the encrypted data in the unlocking script using `OP_PUSH` opcodes, making it spendable by the backup key holder.

## Backup Payload

### Included Tables

| Table | Notes |
|-------|-------|
| wallet | Excludes mnemonic (re-entered on recovery). Includes PIN salt, DPAPI blob, current_index, backed_up flag |
| users | Identity key mapping |
| addresses | HD address cache (stripped — see optimizations below) |
| output_baskets | Basket definitions |
| transactions | With sliding window (see optimizations) |
| outputs | With spent-output stripping (see optimizations) |
| proven_txs | Immutable proof records (merkle_path stripped — re-fetchable) |
| proven_tx_reqs | Proof lifecycle tracking (raw_tx/input_beef stripped, history capped to 5 entries) |
| certificates | BRC-52 identity certificates |
| certificate_fields | Certificate field values |
| output_tags / output_tag_map | Output tagging |
| tx_labels / tx_labels_map | Transaction labels |
| commissions | Service fee tracking |
| settings | Wallet configuration |
| sync_states | Multi-device sync state |
| domain_permissions | Per-site wallet permissions |
| cert_field_permissions | Certificate field access control |

### Excluded (stripped before backup)

| Data | Reason |
|------|--------|
| `parent_transactions` | Raw tx cache, re-fetchable from WoC |
| `block_headers` | Header cache, re-fetchable on demand |
| `proven_txs.merkle_path` | Merkle proofs, re-fetchable from ARC/WoC |
| `proven_tx_reqs.raw_tx` | Raw tx bytes, re-fetchable |
| `proven_tx_reqs.input_beef` | BEEF data, re-fetchable |
| `mnemonic` | Never backed up — user re-enters on recovery |

## Payload Optimizations (Implemented)

These run in `backup.rs::prepare_backup_payload()` before compression:

### 1. Spent-Output Time-Tiered Strip

Drops spent HD-self outputs older than 7 days whose owning address is no longer flagged for UTXO sync.

**Retention rules (drop only if ALL fail):**
1. Active UTXOs (spendable=1) — always keep
2. BRC-42 counterparty outputs (sender_identity_key set) — always keep (non-recoverable from master key)
3. Non-standard derivation (PushDrop, tokens, master-direct) — always keep
4. Recent records (updated_at within 7 days) — keep for in-flight operations
5. Owning address still pending UTXO check — keep
6. Otherwise — drop (spent HD-self output, old, dead address)

**Constants:** `SPENT_OUTPUT_BACKUP_RETENTION_SECS = 7 days`

### 2. Address Time-Tiered Strip

Drops "operationally dead" addresses — used, no spendable outputs, not pending sync, older than 30 days.

**Always keeps:** special indices (master at -1, external at -2), unused addresses, pending addresses, addresses with active UTXOs, addresses newer than 30 days.

**Constants:** `ADDRESS_BACKUP_RETENTION_SECS = 30 days`

### 3. Transaction Sliding Window

Drops completed transactions older than 60 days that have no active spendable outputs. Non-completed transactions (sending, failed, etc.) are always kept.

Also drops orphaned `proven_tx_reqs` whose parent transaction was dropped.

**Constants:** `TX_BACKUP_RETENTION_SECS = 60 days`

### 4. Proven TX Req History Cap

The `history` field on proven_tx_reqs is an unbounded JSON audit log. Capped to last 5 entries (sorted by timestamp descending) to prevent monotonic growth.

### 5. Block Headers + Parent Transactions

Cleared entirely. Both are pure caches re-fetchable on demand from WoC/ARC.

## UTXO Consolidation (Implemented)

Reduces backup cost by reducing the number of UTXOs (fewer inputs = smaller backup transaction).

### Lazy Consolidation in Sends

When building a normal send transaction, up to 10 extra small UTXOs (each <= 5000 sats) are added as inputs alongside the required inputs. The extra value flows into the change output. Zero privacy cost — looks like a normal spend.

**Config:** `max_extra_inputs = 10`, `lazy_consolidation_threshold = 5000 sats`

### Auto Dust Consolidation Task

`TaskConsolidateDust` runs daily. Sweeps confirmed UTXOs <= 1000 sats into a single consolidated output when 20+ dust UTXOs accumulate. Creates a standard P2PKH transaction with a change address.

**Config:** `dust_threshold_sats = 1000`, minimum 20 UTXOs to trigger

## Automatic Triggers

### Event-Based (3-minute debounce)

After significant wallet events (transactions, certificate operations), `AppState::request_backup_check()` sets a flag. The monitor checks: if 3 minutes have passed since the last event with no new events, a backup runs. Hard cap: 10 minutes from the first event (prevents infinite deferral from rapid-fire events).

**Trigger threshold:** Events involving > $3 USD equivalent.

### Periodic Safety Net

`TaskBackup` runs every 3 hours regardless of events. Only broadcasts if the DB hash has changed since the last backup (dirty check).

### Dirty-Flag Debouncing

A SHA256 hash of the backup payload is computed and stored. If the hash matches the last backup, no broadcast occurs — the on-chain backup is already current.

## Recovery Flow

1. User enters 12-word mnemonic
2. Wallet derives master key → backup address (BRC-42, invoice "1-wallet-backup-1")
3. Queries WoC for UTXOs at backup address — finds the 546-sat marker
4. Fetches the marker's parent transaction — extracts PushDrop output
5. Parses PushDrop script → extracts encrypted payload bytes
6. Derives AES key from master private key → decrypts → gunzips → JSON
7. Imports all entities into fresh database
8. Re-derives master address, default basket, DPAPI blob
9. Triggers full UTXO sync to discover any transactions that occurred after the backup

## Orphan Sweep

Each backup transaction spends the PREVIOUS backup's PushDrop + marker outputs as inputs. This means only the latest backup exists as a UTXO — older ones are automatically consumed.

The orphan sweep logic in the backup builder:
1. Queries the backup address for all UTXOs
2. Adds any existing PushDrop tokens and markers as inputs (reclaims their sats)
3. Creates new PushDrop + marker outputs with the fresh backup data

## Key Files

| File | Purpose |
|------|---------|
| `rust-wallet/src/backup.rs` | Serialization, encryption, compression, optimization, payload structs |
| `rust-wallet/src/handlers.rs` | `/wallet/backup` endpoint, `/wallet/recover` endpoint, on-chain backup builder |
| `rust-wallet/src/monitor/task_backup.rs` | Periodic backup trigger, dirty-check, HTTP call to local `/wallet/backup` |
| `rust-wallet/src/main.rs` | `AppState::request_backup_check()` — event-based trigger flag |

## Critical Invariants

1. **PushDrop tokens MUST be preserved in backup** — not discoverable by address scan, only by spending
2. **BRC-42 counterparty outputs MUST be preserved** — can't re-derive from master key alone (e.g. PeerPay received UTXOs)
3. **Recovery code is fragile** — the backup stripping depends on recovery logic that handles missing data gracefully. Changes to strip rules must be tested against recovery.
4. **Block headers and parent_transactions are NOT in backup** — recovery triggers background re-fetch
5. **Mnemonic is NEVER in the backup** — user must re-enter it. This is by design (backup on-chain = mnemonic would be in the clear if encryption is broken)
