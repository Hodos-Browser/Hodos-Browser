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
| Storage format | PushDrop (spendable) | Recovers sats on each update; single UTXO = latest backup |
| Address derivation | BRC-42 self-counterparty, invoice `"1-wallet-backup-1"` | Deterministic from mnemonic, same across all devices |
| Address DB index | -3 (special index) | Follows pattern: -1 = master, -2 = external, -3 = backup |
| Token amount | 1000 sats | Above dust, recoverable on next update |
| Compression | gzip level 9 BEFORE encryption | 96-97% reduction; encrypted data doesn't compress |
| Encryption | AES-256-GCM, key from HKDF(master_privkey) | No password needed — mnemonic-derived key |
| Serialization | Reuse existing `BackupPayload` from `backup.rs` | Already has all entity types mapped |
| Update frequency | On shutdown + event-triggered + periodic safety net | Dirty flag prevents wasted backups when nothing changed |
| Recovery cost | Zero coins | Read-only chain query — no transaction needed |

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

## Transaction Output Order (Backup Tx)

```
Input 0:     Previous backup UTXO (P2PK — spend old backup) [if exists]
Input 1..N:  Wallet UTXOs (P2PKH — funding)

Output 0:    PushDrop backup (1000 sats → backup address)
Output 1:    Hodos service fee (1000 sats → company address)
Output 2:    Change (if above dust → new change address)
```

First backup (no previous UTXO): inputs are only wallet UTXOs.

---

## Cost Summary

| Operation | Cost | Frequency |
|-----------|------|-----------|
| First backup | ~1600 sats (1000 token + ~600 fee) | Once |
| Subsequent backup | ~600 sats (fee only — old token recovered) | Per trigger |
| Recovery | **0 sats** (read-only API query) | On demand |
| Service fee | 1000 sats | Per backup tx |

Annual cost for daily backups: ~$0.50-$7 depending on wallet size.

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

## Security Considerations

1. **Encryption key** is derived from master private key — only someone with the mnemonic can decrypt
2. **PushDrop output** is spendable only by the backup private key (BRC-42 derived from master)
3. **Mnemonic is excluded** from backup payload — user must re-enter it during recovery
4. **On-chain data** is encrypted — chain observers see opaque bytes, not wallet contents
5. **No new attack surface** — backup uses existing crypto primitives (AES-256-GCM, BRC-42, P2PK signing)
