# On-Chain Wallet Backup — Implementation Plan

**Created**: 2026-03-25
**Status**: Ready for implementation
**Research**: `ONCHAIN_FULL_BACKUP_RESEARCH.md`
**Existing infra**: `backup.rs` (serialization), `script/pushdrop.rs` (encode/decode), `crypto/aesgcm_custom.rs` (AES-256-GCM)

---

## Overview

Automatically back up the entire wallet database as an encrypted, compressed PushDrop UTXO at a deterministic BRC-42 address. Recovery from mnemonic only — no coins required to restore.

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Storage format | PushDrop (spendable) + P2PKH marker | Recovers sats on each update; marker enables on-chain discovery |
| Address derivation | BRC-42 self-counterparty, invoice `"1-wallet-backup-1"` | Deterministic from mnemonic, same across all devices |
| Address DB index | -3 (special index) | Follows pattern: -1 = master, -2 = external, -3 = backup |
| PushDrop token amount | 1000 sats | Above dust, recoverable on next update |
| Marker amount | 546 sats (dust limit) | Smallest amount indexed by block explorers |
| Compression | gzip level 9 BEFORE encryption | 96-97% reduction; encrypted data doesn't compress |
| Encryption | AES-256-GCM, key from SHA256(master_privkey \|\| salt) | No password needed — mnemonic-derived key |
| Serialization | Reuse existing `BackupPayload` from `backup.rs` | Already has all entity types mapped |
| Update frequency | On shutdown + event-triggered + periodic safety net | Dirty flag prevents wasted backups when nothing changed |
| Recovery cost | Zero coins | Read-only chain query — no transaction needed |
| Service fee | Waived | Wallet backup is infrastructure protecting the user |

---

## On-Chain Discovery: P2PKH Marker Output

### Problem

PushDrop outputs use a nonstandard script format (`<pubkey> OP_CHECKSIG <data> OP_DROP`). Block explorers like WhatsOnChain only index standard script types (P2PKH, P2SH) by address. This means a PushDrop output **cannot be discovered** by querying `GET /address/{addr}/unspent` — it is invisible to address-based UTXO lookups.

This is a critical problem for recovery: the user has only their mnemonic, from which we can derive the backup address, but we cannot find the PushDrop UTXO at that address.

### Solution: Dual-Output Pattern

Each backup transaction creates **two outputs** in the same transaction:

```
Output 0: PushDrop — encrypted backup data (1000 sats, nonstandard script)
Output 1: P2PKH marker — standard output at backup address (546 sats, indexed by explorers)
Output 2: Change (if above dust)
```

The P2PKH marker at output 1 serves as a **discoverable flag**. On recovery:

1. Derive backup address from mnemonic (BRC-42 self, `"1-wallet-backup-1"`)
2. Query WhatsOnChain: `GET /address/{backup_address}/unspent/all`
3. The marker UTXO is returned with its **txid**
4. The PushDrop is always at **vout 0** of the same txid
5. Fetch full tx: `GET /tx/{txid}/hex` → parse output 0 → PushDrop → decrypt

The marker adds only 546 sats and ~25 bytes to each backup transaction. Both the PushDrop and marker are spent as inputs on subsequent backups, recovering both amounts.

### Why not other approaches?

| Alternative | Rejected Because |
|-------------|-----------------|
| Query by script hash | WhatsOnChain does not index nonstandard scripts by hash |
| OP_RETURN + P2PKH | OP_RETURN is unspendable — sats burned on every update |
| Overlay/indexer service | Adds external dependency; not available from mnemonic alone |
| Store txid off-chain | Defeats the purpose of mnemonic-only recovery |

### BRC Consideration

This dual-output pattern (data in nonstandard script + discoverable marker in standard script) is a general solution for any protocol that stores data in PushDrop/nonstandard outputs but needs on-chain discoverability. It should be documented as a recommended pattern in any BRC specification for on-chain data storage with recovery requirements.

---

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| No wallet exists | Skip — nothing to back up |
| Insufficient funds (< ~1600 sats) | Skip silently, log warning. Retry on next trigger |
| First backup (no previous UTXO) | Use wallet UTXOs as input, create new PushDrop output |
| Subsequent backup | Spend previous backup UTXO as input (recovers sats) + wallet UTXOs if needed |
| App crash (no shutdown hook) | Periodic 30-min backup covers this |
| Backup data > 200 KB compressed | Log warning, proceed anyway (PushDrop has no hard limit) |
| Recovery with no backup on-chain | Fall through to fresh wallet (same as today) |
| Concurrent devices | Double-spend on backup UTXO → retry after pulling latest |

---

## Data Flow

### Backup (Write)
```
SQLite DB
  → BackupPayload (reuse backup.rs serialization)
  → JSON string (serde_json)
  → gzip level 9 (flate2)
  → AES-256-GCM encrypt (key = HKDF(master_privkey, "hodos-wallet-backup-v1"))
  → PushDrop encode (pubkey = backup address derived key)
  → Transaction (spend old backup UTXO if exists + wallet UTXOs for fee)
  → Broadcast
  → Save backup txid:vout locally
```

### Recovery (Read)
```
Mnemonic
  → Master private key (BIP39)
  → Derive backup address: BRC-42(master_priv, master_pub, "1-wallet-backup-1")
  → Query WhatsOnChain for UTXO at backup address (FREE — no coins needed)
  → Fetch full transaction hex
  → Parse PushDrop script → extract encrypted payload
  → AES-256-GCM decrypt (key = HKDF(master_privkey, "hodos-wallet-backup-v1"))
  → gzip decompress
  → JSON parse → BackupPayload
  → import_to_db() (reuse existing backup.rs import)
  → Done — full wallet restored
```

---

## Implementation Sprints

### Sprint 1: Address Derivation + DB Storage

**Goal**: Derive the backup address deterministically and store it as index -3.

**Files**:
- `database/connection.rs` — `ensure_backup_address_exists()` (similar to `ensure_master_address_exists()`)
- `database/address_repo.rs` — include index -3 in pending UTXO check queries (so we can detect if a backup exists on recovery)

**Steps**:
1. In `WalletDatabase::migrate()` or a new startup check, derive backup address:
   ```
   invoice = "1-wallet-backup-1"
   backup_privkey = BRC-42(master_privkey, master_pubkey, invoice)  // self-counterparty
   backup_pubkey = backup_privkey * G
   backup_address = P2PKH(backup_pubkey)
   ```
2. Store in addresses table with `index = -3`, `pending_utxo_check = false`
3. Store the backup private key derivation info so we can sign the PushDrop spend later

**Test**: Create wallet, verify address at index -3 exists and is deterministic (same mnemonic → same address).

---

### Sprint 2: Serialization + Compression + Encryption Pipeline

**Goal**: Build the pipeline that takes the current DB state and produces encrypted compressed bytes.

**Files**:
- `backup.rs` — add `serialize_for_onchain()` and `deserialize_from_onchain()` functions
- `Cargo.toml` — add `flate2` dependency for gzip

**Steps**:
1. Add `flate2 = "1"` to Cargo.toml
2. `serialize_for_onchain(db, master_privkey) -> Result<Vec<u8>>`:
   - Call existing `build_backup_payload(db)` (already in backup.rs)
   - Exclude mnemonic from payload (set to empty string — recovery user already has it)
   - `serde_json::to_vec(&payload)`
   - Compress: `flate2::write::GzEncoder::new(Vec::new(), Compression::best())`
   - Derive encryption key: `HKDF-SHA256(master_privkey, salt="hodos-wallet-backup-v1")`
     - Use `sha2::Sha256` for HKDF (we have `sha2` already). If `hkdf` crate is too heavy, simple approach: `SHA256(master_privkey || "hodos-wallet-backup-v1")` as the 32-byte key
   - Encrypt: AES-256-GCM with random 12-byte nonce
   - Return: `nonce(12) || ciphertext || tag(16)`
3. `deserialize_from_onchain(encrypted_bytes, master_privkey) -> Result<BackupPayload>`:
   - Derive same key
   - Split: nonce(12), ciphertext+tag
   - Decrypt AES-256-GCM
   - Decompress gzip
   - `serde_json::from_slice::<BackupPayload>()`

**Test**: Round-trip — serialize a wallet, verify deserialize produces identical payload.

---

### Sprint 3: PushDrop Transaction Builder

**Goal**: Build and broadcast the backup transaction.

**Files**:
- `handlers.rs` — new `wallet_backup_onchain()` handler (POST `/wallet/backup/onchain`)
- `script/pushdrop.rs` — reuse existing `encode()` for PushDrop script construction

**Steps**:
1. New async handler `wallet_backup_onchain`:
   a. Check wallet exists and is unlocked
   b. Serialize + compress + encrypt (Sprint 2 pipeline)
   c. Derive backup key pair (BRC-42 self, invoice `"1-wallet-backup-1"`)
   d. Build PushDrop locking script: `encode(&[encrypted_bytes], &backup_pubkey, LockPosition::Before)`
   e. Check for existing backup UTXO at backup address (query outputs table for index -3 UTXOs)
   f. **If previous backup UTXO exists**: spend it as input (recovers sats)
   g. Select additional wallet UTXOs if needed to cover: token_amount(1000) + miner_fee + service_fee(1000)
   h. Build transaction:
      - Inputs: [previous backup UTXO (if exists)] + [wallet UTXOs]
      - Output 0: PushDrop backup (1000 sats)
      - Output 1: Hodos service fee (1000 sats)
      - Output 2: Change (if above dust)
   i. Sign all inputs (P2PK for backup UTXO input, P2PKH for wallet inputs)
   j. Build BEEF + broadcast
   k. Track new backup output in outputs table (basket: "wallet-backup", derivation: "1-wallet-backup", index -3)
   l. Save `last_backup_txid` in settings table
   m. Return success with txid

2. Register route in `main.rs`: `.route("/wallet/backup/onchain", web::post().to(wallet_backup_onchain))`

**Important**: This handler should NOT use `create_action_internal` — it needs custom PushDrop output construction and P2PK signing for the backup UTXO input (same reason as certificate handlers).

**Test**: Trigger backup, verify transaction on WhatsOnChain shows PushDrop output at backup address.

---

### Sprint 4: Recovery from Chain

**Goal**: During wallet recovery, check for on-chain backup before falling back to empty wallet.

**Files**:
- `handlers.rs` — modify `wallet_recover` flow
- `backup.rs` — add `recover_from_onchain()` function
- `recovery.rs` — integrate on-chain backup check into recovery flow

**Steps**:
1. `recover_from_onchain(mnemonic) -> Result<Option<BackupPayload>>`:
   a. Derive master key from mnemonic
   b. Derive backup address (BRC-42 self, invoice `"1-wallet-backup-1"`)
   c. Query WhatsOnChain: `GET /v1/bsv/main/address/{backup_address}/unspent`
   d. If no UTXO found → return `Ok(None)` (no backup exists)
   e. If UTXO found → fetch full tx: `GET /v1/bsv/main/tx/{txid}/hex`
   f. Parse transaction, find the PushDrop output
   g. Decode PushDrop → extract encrypted bytes
   h. Decrypt + decompress → BackupPayload
   i. Return `Ok(Some(payload))`

2. Modify `wallet_recover` handler:
   - After creating wallet from mnemonic
   - Call `recover_from_onchain(mnemonic)`
   - If `Some(payload)`: call existing `import_to_db()` to populate all entities
   - If `None`: continue with normal recovery flow (BIP32 scanning)

**No coins needed** — this is all read-only API queries.

**Test**: Back up wallet → delete wallet.db → recover from mnemonic → verify all transactions, outputs, certificates restored.

---

### Sprint 5: Automatic Backup Triggers

**Goal**: Trigger backup automatically so users don't have to think about it.

**Files**:
- `handlers.rs` — modify `shutdown` handler to trigger backup before shutting down
- `monitor/mod.rs` — add periodic backup check
- `database/settings_repo.rs` — track `last_backup_at` and `backup_dirty` flag

**Trigger points**:

| Trigger | Where | Mechanism |
|---------|-------|-----------|
| **Shutdown** | `shutdown` handler | Check dirty flag → back up if dirty → then shutdown |
| **After transaction** | End of `create_action_internal` | Set `backup_dirty = 1` in settings |
| **After certificate change** | End of publish/unpublish | Set `backup_dirty = 1` |
| **Manual** | POST `/wallet/backup/onchain` | User-triggered via wallet panel (ignores dirty flag) |
| **Periodic safety net** | Monitor task `TaskBackup` (1-2 hours) | Check dirty flag → back up if dirty. Only purpose: catch crashes where shutdown never fires |

**Dirty flag**: `backup_dirty` in the settings table.
- Set to `1` after any state-changing operation (transaction, certificate, settings change)
- Checked before any automatic backup — if `0`, skip (no wasted backups)
- Cleared to `0` after successful backup
- Manual backup ignores the flag (user explicitly requested)

**Shutdown backup flow**:
```rust
pub async fn shutdown(data: web::Data<AppState>, _body: web::Bytes) -> HttpResponse {
    log::info!("🛑 /shutdown received");

    // Attempt on-chain backup before shutting down
    // Skip if: no wallet, wallet locked, insufficient funds
    match try_onchain_backup(&data).await {
        Ok(Some(txid)) => log::info!("   ✅ Shutdown backup broadcast: {}", txid),
        Ok(None) => log::info!("   ⏭️  Shutdown backup skipped (no wallet/funds/changes)"),
        Err(e) => log::warn!("   ⚠️  Shutdown backup failed: {} (proceeding with shutdown)", e),
    }

    data.shutdown.cancel();
    HttpResponse::Ok().json(serde_json::json!({ "status": "shutting_down" }))
}
```

**Dirty flag**: Set `backup_dirty = 1` in settings after any state-changing operation. Clear after successful backup. The monitor task checks this flag every 30 min.

**Debounce**: The periodic monitor task naturally debounces (1-2 hour interval). Shutdown backup fires immediately. Manual backup fires immediately.

**Cross-platform**: gzip (`flate2`) and AES-256-GCM produce identical bytes on all platforms. Compress on Windows, decompress on macOS — no issues. All done in the Rust wallet layer.

**Test**: Make a transaction → close browser → reopen → verify backup exists on-chain.

---

### Sprint 6: Frontend UI (Optional — Can Defer)

**Goal**: Show backup status in wallet settings and allow manual trigger.

- Settings page: "Last backed up: [timestamp]" or "Never backed up"
- Manual backup button
- Recovery flow: show "Restoring from on-chain backup..." during recovery
- Badge/indicator if backup is stale (> 24 hours with changes)

---

## New Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `flate2` | `1` | gzip compression/decompression |

All other dependencies already exist: `aes-gcm`, `sha2`, `rand`, `bs58`, `hex`, `serde_json`, `reqwest`.

For HKDF key derivation, we can use `SHA256(master_privkey_bytes || b"hodos-wallet-backup-v1")` — this is simple and secure enough for a single-purpose key derivation where the input is already a 256-bit secret. A full HKDF crate is unnecessary.

---

## Database Changes

### Settings Table
Add two columns (or use the existing JSON-style settings):
- `last_backup_txid` TEXT — txid of most recent backup transaction
- `last_backup_at` INTEGER — Unix timestamp of last successful backup
- `backup_dirty` INTEGER DEFAULT 0 — 1 if wallet state changed since last backup

### Addresses Table
- Index -3 reserved for backup address (created on wallet creation, like -1 for master)

### No new tables needed.

---

## Payload Size Optimization

The backup payload must be kept small to minimize on-chain costs. Two categories of optimization are applied:

1. **Exclusions** — backup-of-backup data that would cause recursive growth
2. **Stripping** — data that can be re-fetched from the blockchain for free during recovery

### Exclusions (backup-of-backup prevention)

Without these filters, each backup includes the previous backup's transaction data, causing exponential growth (84 KB → 444 KB → 624 KB → ...). Backup transactions are excluded from the payload entirely:

| Table | Filter | Reason |
|-------|--------|--------|
| `transactions` | `reference_number NOT LIKE 'backup-%'` | Backup tx records contain huge raw_tx of previous backups |
| `outputs` | `derivation_prefix != '1-wallet-backup'` | PushDrop outputs contain the full encrypted payload |
| `parent_transactions` | txid not in backup txs | Raw hex cache of backup txs |
| `proven_txs` | txid not in backup txs | Merkle proofs + raw_tx of backup txs |
| `proven_tx_reqs` | txid not in backup txs | Proof tracking for backup txs |

### Stripping re-fetchable data

The following data is stripped from the on-chain payload and re-fetched from WhatsOnChain during recovery. This is the biggest size win — raw transaction bytes and merkle proofs dominate payload size but are permanently available on-chain for free.

| Stripped Data | Records | Re-fetch Method | Recovery Time |
|---------------|---------|-----------------|---------------|
| `transactions.raw_tx` (confirmed only — has `proven_tx_id`) | ~22 | WhatsOnChain `GET /tx/{txid}/hex` per tx | ~5-10s (parallel) |
| `proven_txs.raw_tx` | ~22 | Same API, same txids | Already covered above |
| `proven_txs.merkle_path` | ~22 | WhatsOnChain `GET /tx/{txid}/proof/tsc` per tx | ~5-10s (parallel) |
| `proven_tx_reqs.raw_tx` + `input_beef` | ~28 | Same txids | Already covered |
| `parent_transactions` (entire table) | ~33 | WhatsOnChain `GET /tx/{txid}/hex` per txid | ~5-10s (parallel) |
| `outputs.locking_script` (spent only — `spendable = 0`) | ~30 | Parse from raw_tx fetch | Covered by raw_tx fetch |

**Stripping is done in `serialize_for_onchain()` AFTER `collect_payload()`**, so file-based backups (`wallet_backup`) are not affected — they still include everything.

**What is NOT stripped** (essential data that cannot be re-derived):
- Transaction metadata (txid, status, satoshis, description, labels, timestamps)
- Output derivation info (prefix, suffix, sender_identity_key — needed for key derivation)
- Spendable output locking scripts (needed for spending)
- Unconfirmed transaction raw_tx (not yet mined — could be lost)
- Certificates and certificate fields
- Addresses and public keys
- Settings, baskets, tags, labels
- Domain permissions

### Size impact (real-world measurements)

Wallet with 26 transactions, 47 outputs, 49 addresses, 4 certificates:

| Optimization Stage | Compressed Size | Notes |
|--------------------|----------------|-------|
| No optimization | 84 KB | First implementation |
| + Backup exclusions | 85 KB | Prevents recursive growth but doesn't reduce current size |
| + Strip confirmed raw_tx | 65 KB | Biggest single win |
| + Strip proven_tx_reqs raw data | 39 KB | |
| + Strip merkle_paths | **21 KB** | Final size — essential wallet data floor |

### Recovery re-fetch plan (Sprint 4)

After restoring from on-chain backup, a background task re-fetches all stripped data:

1. Collect all txids from restored `transactions` and `proven_txs` tables
2. For each txid (parallel, batched):
   a. `GET /v1/bsv/main/tx/{txid}/hex` → populate `transactions.raw_tx`, `proven_txs.raw_tx`, `parent_transactions`
   b. `GET /v1/bsv/main/tx/{txid}/proof/tsc` → populate `proven_txs.merkle_path`
   c. Parse raw tx → restore `outputs.locking_script` for spent outputs
3. Rebuild `proven_tx_reqs.raw_tx` from transaction raw bytes
4. Total time: ~10-15 seconds for typical wallet

The wallet is **immediately functional** after restore — UTXOs, keys, certificates, addresses are all present. The re-fetch only backfills history display data and BEEF cache.

---

## Transaction Output Order (Backup Tx)

```
Input 0:     Previous PushDrop UTXO (P2PK — spend old backup data) [if exists]
Input 1:     Previous marker UTXO (P2PKH — spend old marker) [if exists]
Input 2..N:  Wallet UTXOs (P2PKH — funding)

Output 0:    PushDrop backup (1000 sats — encrypted data, nonstandard script)
Output 1:    P2PKH marker (546 sats — at backup address, for on-chain discovery)
Output 2:    Change (if above dust → new change address)
```

First backup (no previous UTXOs): inputs are only wallet UTXOs.
Subsequent backups: previous PushDrop + marker are spent as inputs, recovering both amounts.

**No Hodos service fee** — wallet backups are infrastructure protecting the user's funds.

---

## Cost Summary (Real-World Measurements)

| Operation | Cost (sats) | Cost (USD at $40 BSV) | Frequency |
|-----------|-------------|----------------------|-----------|
| First backup | ~3,100 (1000 token + ~2,100 fee) | ~$0.0012 | Once |
| Subsequent backup | ~2,100 (fee only — old token recovered) | ~$0.0008 | Per trigger |
| Recovery | **0** (read-only API query) | $0.00 | On demand |
| Service fee | **Waived** | — | — |

Based on ARC dynamic fee rate of 100 sat/KB with ~21 KB compressed payload. Annual cost for daily backups: **~$0.30**.

---

## Files Changed Per Sprint

| Sprint | New Files | Modified Files |
|--------|-----------|----------------|
| 1 | — | `connection.rs`, `address_repo.rs` |
| 2 | — | `backup.rs`, `Cargo.toml` |
| 3 | — | `handlers.rs`, `main.rs` |
| 4 | — | `handlers.rs`, `backup.rs`, `recovery.rs` |
| 5 | `monitor/task_backup.rs` | `handlers.rs`, `monitor/mod.rs`, `settings_repo.rs` |
| 6 | — | Frontend files (deferred) |

---

## Backup Timing Risk Matrix & Decisions (2026-04-03)

### Problem Statement

The backup "soon" flag had two bugs:
1. Flag didn't reset on new events — second event within 3 minutes was silently ignored
2. Flag was cleared even when backup skipped (e.g., pending transactions) — backup wouldn't retry until next 3-hour periodic cycle

### Key Insight

**Waiting for confirmation before backup is MORE dangerous than backing up unproven transactions.** The shutdown backup already had no pending-tx guard — it would backup unproven transactions on every app close.

### Full Risk Matrix

**Scenario key**: "Wait" = defer backup while unproven txs exist (old behavior). "Include" = backup with unproven txs (new behavior). Severity = what happens if user must recover from this backup.

#### A. Regular P2PKH Send

| # | Scenario | Strategy | Recovery Outcome | Severity | Safeguards |
|---|----------|----------|------------------|----------|------------|
| A1 | Send confirmed, backed up | Either | Perfect recovery | None | — |
| A2 | Send unproven, backup includes it, tx confirms | Include | Good — tx on-chain, proof acquired later, change discovered by sync | **Low** | TaskCheckForProofs, TaskSyncPending |
| A3 | Send unproven, backup includes it, tx FAILS | Include | Ghost change output inflates balance. Inputs marked spent but tx never mined | **Medium** | TaskCheckForProofs marks failed ≤6hr → deletes ghost + restores inputs. TaskValidateUtxos marks ghost external-spend ≤30min |
| A4 | Send unproven, backup defers, user shuts down | Wait | Stale backup: inputs appear unspent but are spent on-chain | **Medium** | TaskSyncPending reconciles on next run |
| A5 | Send unproven, flag cleared without backup, 3hr gap | Wait (old bug) | Same as A4 but 3-hour exposure window | **Medium-High** | Same safeguards, larger window |

#### B. Regular P2PKH Receive (Address Sync)

| # | Scenario | Strategy | Recovery Outcome | Severity | Safeguards |
|---|----------|----------|------------------|----------|------------|
| B1 | Receive confirmed, backed up | Either | Perfect | None | — |
| B2 | Receive unproven, backup includes, confirms | Include | Good — output on-chain, proof later | **Low** | TaskCheckForProofs, TaskSyncPending |
| B3 | Receive unproven, backup includes, FAILS | Include | Balance inflated temporarily | **Low** | TaskValidateUtxos marks external-spend ≤30min |
| B4 | Receive confirmed, no backup ran (old bug) | Wait | Receive missing from backup | **Very Low** | BIP32 recovery rediscovers via address scan |

#### C. PeerPay Send

| # | Scenario | Strategy | Recovery Outcome | Severity | Safeguards |
|---|----------|----------|------------------|----------|------------|
| C1 | Send confirmed, backed up | Either | Perfect | None | — |
| C2 | Send unproven, backup includes, confirms | Include | Good — same as A2 | **Low** | TaskCheckForProofs |
| C3 | Send unproven, backup includes, FAILS | Include | Ghost cleanup needed — same as A3 | **Medium** | TaskCheckForProofs timeout → mark_failed() |
| C4 | Send, no backup ran (old bug) | Wait | Inputs shown spendable but spent on-chain | **Medium** | TaskSyncPending/TaskValidateUtxos reconcile |

#### D. PeerPay Receive (Auto-Accept) — BRC-42 Derived

| # | Scenario | Strategy | Recovery Outcome | Severity | Safeguards |
|---|----------|----------|------------------|----------|------------|
| D1 | Receive confirmed, backed up | Either | Perfect | None | — |
| D2 | Receive, no backup ran (old bug) | Wait | **BRC-42 output LOST** — mnemonic recovery won't find it (BRC-42 scanning disabled) | **HIGH** | **None** — only prior on-chain backup can save it |
| D3 | Receive unproven, backup includes, confirms | Include | Good — output on-chain, proof comes later | **Low** | TaskCheckForProofs |
| D4 | Receive unproven, backup includes, FAILS | Include | Ghost BRC-42 output — can't be spent (BEEF fails), dead weight | **Low** | BEEF construction fails gracefully, no corruption |

#### E. PushDrop/Token Create (Certificate Publish, Token Mint)

| # | Scenario | Strategy | Recovery Outcome | Severity | Safeguards |
|---|----------|----------|------------------|----------|------------|
| E1 | Token confirmed, backed up | Either | Perfect | None | — |
| E2 | Token unproven, backup includes, confirms | Include | Good — token on-chain, proof later | **Low** | TaskCheckForProofs |
| E3 | Token unproven, backup includes, FAILS | Include | Ghost token in wallet, can't be spent. Inputs marked spent | **Medium-High** | TaskCheckForProofs marks failed ≤6hr → deletes ghost + restores inputs. Note: TaskValidateUtxos SKIPS token outputs |
| E4 | Token confirmed, no backup ran (old bug) | Wait | **Token PERMANENTLY LOST** — not in backup, can't rediscover via address scan, TaskValidateUtxos skips it | **CRITICAL** | **No safeguard** |
| E5 | Token unproven, backup defers, confirms, then backup runs | Wait (working) | Perfect | None | — |

#### F. PushDrop/Token Receive

| # | Scenario | Strategy | Recovery Outcome | Severity | Safeguards |
|---|----------|----------|------------------|----------|------------|
| F1 | Token received, confirmed, backed up | Either | Perfect | None | — |
| F2 | Token received, no backup ran (old bug) | Wait | **Token PERMANENTLY LOST** — same as E4 | **CRITICAL** | **None** |
| F3 | Token received unproven, backup includes, FAILS | Include | Ghost token — dead weight until cleanup | **Medium** | TaskCheckForProofs timeout |

### Risk Summary

| Severity | Scenarios | Root Cause |
|----------|-----------|------------|
| **CRITICAL** | E4, F2 | Flag consumed without backup → token/PushDrop never backed up → unrecoverable |
| **HIGH** | D2 | Flag consumed → BRC-42 PeerPay output never backed up → lost on mnemonic-only recovery |
| **MEDIUM-HIGH** | A5, E3 | Extended no-backup window (3hr) OR ghost token persists until 6hr timeout |
| **MEDIUM** | A3, A4, C3, C4 | Ghost outputs or stale backup — safeguards catch within minutes-hours |
| **LOW** | A2, B2, B3, C2, D3, D4, E2, F3 | Temporary inconsistency, auto-resolved |

### Post-Recovery Validation

After on-chain backup recovery, the imported data may contain unproven transactions or ghost outputs. The Monitor now runs TaskCheckForProofs and TaskValidateUtxos **immediately** on the next tick (~30 seconds) via the `recovery_just_completed` flag, rather than waiting for their normal intervals (60s / 30min).

### Decisions Made

1. **Removed pending-tx guard** from `task_backup.rs` — backup captures current DB state regardless of in-flight transactions. Ghost outputs are self-healing (MEDIUM); missing tokens are not (CRITICAL).
2. **Flag only cleared on successful backup** — `BackupOutcome` enum distinguishes Broadcast/Skipped (clear flag) from Deferred/Failed (keep flag, retry next tick).
3. **Timer resets on new events** with 10-minute hard cap — `backup_check_needed` stores `(first_event_ts, latest_event_ts)`. 3-minute delay from latest event, never more than 10 minutes from first event.
4. **Post-recovery immediate validation** — `recovery_just_completed` AtomicBool triggers TaskCheckForProofs + TaskValidateUtxos on next Monitor tick after on-chain recovery.

### Files Changed

| File | Change |
|------|--------|
| `rust-wallet/src/monitor/task_backup.rs` | `BackupOutcome` enum, removed pending-tx guard |
| `rust-wallet/src/monitor/mod.rs` | Conditional flag clearing, post-recovery immediate validation |
| `rust-wallet/src/main.rs` | Timer reset with cap, `recovery_just_completed` flag |
| `rust-wallet/src/handlers.rs` | Set recovery flag after on-chain restore |

---

## Security Considerations

1. **Encryption key** is derived from master private key — only someone with the mnemonic can decrypt
2. **PushDrop output** is spendable only by the backup private key (BRC-42 derived from master)
3. **Mnemonic is excluded** from backup payload — user must re-enter it during recovery
4. **On-chain data** is encrypted — chain observers see opaque bytes, not wallet contents
5. **No new attack surface** — backup uses existing crypto primitives (AES-256-GCM, BRC-42, P2PK signing)
