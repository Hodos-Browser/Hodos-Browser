# On-Chain Full Wallet Backup — Feasibility Research

**Created**: 2026-03-16
**Status**: Research complete — feasibility confirmed
**Author**: Edwin (deep dive research for Matt)
**Related**: `WALLET_BACKUP_AND_RECOVERY_PLAN.md`

---

## Executive Summary

**Matt's proposed approach is feasible and superior to the current plan.**

Key findings:
1. **Compression is exceptional** — JSON wallet data compresses 96-97% (50 KB → 2 KB)
2. **Cost is negligible** — Even heavy users pay < $7/year for daily on-chain backups
3. **PushDrop is better than OP_RETURN** — Recovers sats on each update
4. **No scanning needed** — Single deterministic address contains everything
5. **Built-in multi-device sync** — Works out of the box

---

## 1. The Proposed Approach

### Matt's Vision
Instead of the current plan (backup only counterparty outputs + scanning for self-derived), put the **entire encrypted wallet DB** into a PushDrop UTXO at a deterministic BRC-42 self-counterparty address.

```
Address derivation:
  protocolID: [1, "backup"]
  keyID: "1" (or "wallet-sync")
  counterparty: own identity key (self)
  
  → BRC-42(master_privkey, own_pubkey, "1-backup-1")
```

### Benefits
- **Instant recovery**: No scanning, just check one address
- **Full backup**: Everything is there, not just counterparty outputs
- **Cross-device sync**: All devices check same address, merge state
- **Simpler**: One mechanism for backup, recovery, AND sync

---

## 2. Compression Analysis

### Key Finding: JSON Compresses Extremely Well

| Wallet Size | Raw JSON | Compressed (gzip -9) | Ratio |
|-------------|----------|---------------------|-------|
| Light (50 tx, 100 outputs) | 47 KB | 2.1 KB | 4.5% |
| Moderate (500 tx, 1000 outputs) | 462 KB | 18.5 KB | 4.0% |
| Heavy (2000 tx, 5000 outputs) | 2.1 MB | 82 KB | 3.9% |
| Very Heavy (5000 tx, 10000 outputs) | 4.6 MB | 170 KB | 3.7% |

### Why So Good?
- JSON has repetitive structure (field names repeat thousands of times)
- Hex strings (txids, scripts) have patterns
- gzip's LZ77 + Huffman coding exploits this perfectly

### Critical Implementation Note
**Must compress BEFORE encrypting!**

```
CORRECT:  JSON → gzip → AES-256-GCM → on-chain
WRONG:    JSON → AES-256-GCM → gzip → on-chain (encrypted data doesn't compress)
```

Test results:
- Compress then encrypt: 12,259 bytes
- Encrypt then compress: 19,147 bytes (36% larger!)

---

## 3. Cost Analysis

### Fee Assumptions
- BSV relay fee: ~0.25 sat/byte (conservative; often lower)
- BSV price: ~$40 USD
- Transaction overhead: ~200 bytes (inputs, outputs, signatures)

### Per-Backup Cost

| Scenario | Data Size | Total Tx | Fee (sats) | Fee (USD) |
|----------|-----------|----------|------------|-----------|
| Light | 2.1 KB | 2.3 KB | 575 | $0.0002 |
| Moderate | 18.5 KB | 18.7 KB | 4,675 | $0.002 |
| Heavy | 82 KB | 82.5 KB | 20,625 | $0.008 |
| Very Heavy | 170 KB | 170.7 KB | 42,675 | $0.017 |

### Annual Cost (Daily Backups)

| Scenario | Per Backup | Daily Cost | Annual Cost |
|----------|------------|------------|-------------|
| Light | $0.0002 | $0.0002 | **$0.08** |
| Moderate | $0.002 | $0.002 | **$0.68** |
| Heavy | $0.008 | $0.008 | **$3.01** |
| Very Heavy | $0.017 | $0.017 | **$6.23** |

**Conclusion**: Cost is effectively invisible to users.

---

## 4. PushDrop vs OP_RETURN

### OP_RETURN (Current Plan)
```
Output 0: P2PKH marker (546 sats, for discovery)
Output 1: OP_FALSE OP_RETURN <encrypted_data>
```
- ❌ Unspendable (sats burned)
- ❌ Two outputs needed
- ❌ Marker sats not recoverable

### PushDrop + P2PKH Marker (Implemented)
```
Output 0: PushDrop to self-derived key (1000 sats)
          <pubkey> OP_CHECKSIG [compressed_encrypted_data]
Output 1: P2PKH marker at backup address (546 sats)
          Standard P2PKH — indexed by block explorers for discovery
```
- ✅ Spendable! Recover sats on next update
- ✅ PushDrop holds encrypted data, marker enables discovery
- ✅ Natural update model: spend old PushDrop + old marker → create new
- ✅ Only two UTXOs ever exist (the latest PushDrop + marker)

**Why the marker is required**: PushDrop outputs use nonstandard scripts. Block explorers
(WhatsOnChain, etc.) only index standard script types (P2PKH, P2SH) by address. Without the
marker, there is no way to discover the backup UTXO from the mnemonic alone — the PushDrop is
invisible to address-based UTXO queries. The P2PKH marker at a deterministic BRC-42 address
solves this: query the address → find marker → same txid → PushDrop at vout 0.

### Cost Comparison Over Time

With PushDrop + marker, each update spends both previous outputs as inputs:
- First backup: pay for PushDrop (1000) + marker (546) + mining fee
- Subsequent: recover previous PushDrop + marker, pay only fee difference

**Effectively free updates** after the first backup!

---

## 5. Update Frequency Strategy

### Recommended: Event-Triggered + Daily Minimum

**Triggers:**
1. New certificate acquired
2. New counterparty-derived output confirmed
3. Every N transactions (e.g., 10)
4. High-value transaction (> 1M sats)
5. User explicitly requests backup

**Debounce:**
- Wait 5-10 minutes after trigger before backing up
- Batch multiple triggers into single backup

**Minimum interval:**
- Backup at least once per 24 hours if ANY change occurred
- No backup needed if wallet is idle

### Why This Works
- Most wallets: 1-3 backups per week
- Active wallets: maybe daily
- Cost: $0.50-3.00/year even for heavy users

---

## 6. Cross-Device Sync Flow

### Device A Creates Transaction
```
1. Update local SQLite DB
2. Serialize wallet state to JSON
3. Compress with gzip
4. Encrypt with AES-256-GCM (key from master privkey via HKDF)
5. Build PushDrop tx (spend old backup UTXO if exists)
6. Broadcast to network
7. Update local "last_backup_txid"
```

### Device B Syncs
```
1. Derive backup address from seed
2. Query for UTXO at backup address (WhatsOnChain, etc.)
3. If found and txid != local last_sync_txid:
   a. Fetch transaction
   b. Extract PushDrop data
   c. Decrypt (key from master privkey)
   d. Decompress
   e. Merge into local DB (newer updated_at wins)
   f. Update local last_sync_txid
```

### Conflict Resolution
- **On-chain state is source of truth**
- Each device: pull remote → merge with local → push if local has newer changes
- Per-record `updated_at` timestamp determines winner
- IDs may differ between devices; use `txid+vout` or `serialNumber` as natural keys

---

## 7. Implementation Considerations

### First Backup (No Previous UTXO)
- Use any wallet UTXO as input
- Create PushDrop output at backup address
- Track txid for future updates

### Recovery from Seed Only
```
1. Derive master key from mnemonic
2. Derive backup address: BRC-42(master, self, "1-backup-1")
3. Query blockchain for UTXO at this address
4. If found:
   a. Fetch full transaction
   b. Extract and decrypt PushDrop data
   c. Decompress JSON
   d. Initialize new wallet DB with this data
   e. Done! Full wallet restored.
5. If not found:
   a. This is a brand new wallet, or
   b. No backups were ever made (shouldn't happen after MVP)
```

### Concurrent Updates (Two Devices at Once)
```
Problem: Device A and B both try to spend the same backup UTXO

Solution:
1. Before building backup tx, check current UTXO at backup address
2. Build tx spending that UTXO
3. Broadcast
4. If broadcast fails (UTXO already spent):
   a. Wait 10-30 seconds
   b. Pull new backup from chain
   c. Merge local changes
   d. Retry backup with new input
```

### What to Include in Backup

**Include:**
- All transactions (metadata: txid, status, satoshis, description, labels — but NOT raw_tx for confirmed txs)
- All outputs (with full derivation data — but NOT locking_script for spent outputs)
- All proven_txs (merkle proofs only — NOT raw_tx, which is re-fetchable)
- All certificates + fields
- Baskets, tags, labels
- Settings

**Exclude:**
- Mnemonic (user re-enters)
- Balance cache (recomputed)
- Derived key cache (recomputed)
- Browser-specific data (domain whitelist)
- Backup transactions and their associated records (prevents recursive backup-of-backup growth)
- `raw_tx` bytes for confirmed transactions (re-fetchable from WhatsOnChain for free)
- `raw_tx` bytes in proven_txs (same — on-chain permanently)
- `locking_script` for spent outputs (never needed again)

### rawTx Storage Decision (Updated 2026-03-25)

**Decision: Strip raw_tx from confirmed transactions**

Original research recommended Option A (include rawTx). Real-world testing revealed this is the dominant cost driver — a wallet with 27 transactions produced an 85 KB backup, mostly raw_tx bytes. Stripping confirmed raw_tx reduces this to ~5-15 KB.

- Raw bytes for confirmed txs are permanently on-chain and free to re-fetch via WhatsOnChain `/tx/{txid}/hex`
- Unconfirmed tx raw_tx IS included (not yet on-chain, could be lost)
- Recovery re-fetches missing raw_tx as a background task (non-blocking)
- Trade-off: first few minutes after recovery, tx details may show "loading" until re-fetch completes

---

## 8. Comparison: Matt's Approach vs Current Plan

| Aspect | Current Plan | Matt's Approach |
|--------|--------------|-----------------|
| Data backed up | Counterparty outputs + certs only | Entire wallet DB |
| Recovery model | Scan self-derived + check backup | Check one address, done |
| Address type | BIP32 m/2147483647 | BRC-42 self-counterparty |
| Storage method | OP_RETURN + P2PKH marker | PushDrop (single output) |
| Sats recoverable? | No (OP_RETURN unspendable) | Yes (spend on update) |
| Multi-device sync | Manual / cloud scaffolding | Built-in via chain |
| Size per backup | 2-15 KB | 2-170 KB |
| Annual cost (heavy) | ~$1.50 | ~$3-7 |
| Complexity | Higher (scanning + backup) | Lower (one mechanism) |

**Verdict**: Matt's approach is superior. The marginal cost increase (~$5/year) buys dramatically simpler recovery and built-in multi-device sync.

---

## 9. Recommended Protocol Spec

```
PROTOCOL: hodos-wallet-backup-v1

ADDRESS DERIVATION:
  protocolID: [1, "wallet-backup"]
  keyID: "1"
  counterparty: self (wallet's identity key)
  invoice: "1-wallet-backup-1"

OUTPUT FORMAT:
  PushDrop with LockPosition::Before
  Fields: [compressed_encrypted_payload]
  Token amount: 1000 sats (above dust, recoverable)

PAYLOAD FORMAT (before compression):
  {
    "version": 1,
    "chain": "main",
    "created_at": "ISO8601",
    "wallet_identity_key": "02...",
    "transactions": [...],
    "outputs": [...],
    "proven_txs": [...],
    "certificates": [...],
    "baskets": [...],
    "tags": [...],
    "labels": [...],
    "settings": {...}
  }

ENCRYPTION:
  Algorithm: AES-256-GCM
  Key derivation: HKDF-SHA256(master_private_key, salt="hodos-wallet-backup-v1")
  Nonce: 12 random bytes (prepended to ciphertext)
  Auth tag: 16 bytes (appended to ciphertext)

COMPRESSION:
  Algorithm: gzip level 9
  Order: JSON → gzip → AES-GCM → on-chain

BACKUP TRIGGERS:
  - New certificate acquired
  - New counterparty output confirmed
  - Every 10 transactions
  - High-value transaction (> 1M sats)
  - 24 hours since last backup (if any changes)

UPDATE FLOW:
  1. Check current backup UTXO at derived address
  2. Build new backup payload
  3. Create tx: input=old backup UTXO, output=new PushDrop
  4. Broadcast
  5. Track new txid locally
```

---

## 10. Backup Timing Analysis (2026-04-03)

### Should backup wait for transaction confirmation?

**No.** Analysis of all scenarios (regular sends, PeerPay, PushDrop tokens) shows that waiting is more dangerous than including unproven transactions:

- **Missing token = permanent loss**: PushDrop outputs and BRC-42 PeerPay outputs cannot be rediscovered via address scanning. If the backup never runs (flag consumed by a deferred backup), these are permanently lost on recovery.
- **Ghost outputs = self-healing**: If backup includes an unproven transaction that later fails, TaskCheckForProofs marks it failed within 6 hours and cleans up ghost outputs. TaskValidateUtxos catches P2PKH ghosts within 30 minutes. BEEF construction fails gracefully for ghost token outputs.
- **Shutdown backup already does this**: The `do_onchain_backup()` called during shutdown has no pending-tx guard — it has always backed up unproven transactions.

### Full Risk Matrix

**Strategy key**: "Wait" = defer while unproven (old). "Include" = backup regardless (new).

#### Regular P2PKH Send/Receive

| # | Scenario | Strategy | Severity | What Happens on Recovery |
|---|----------|----------|----------|--------------------------|
| A1 | Send confirmed, backed up | Either | None | Perfect |
| A2 | Send unproven, included, confirms | Include | **Low** | Good — TaskCheckForProofs acquires proof, sync finds change |
| A3 | Send unproven, included, FAILS | Include | **Medium** | Ghost change output + spent inputs. TaskCheckForProofs cleans up ≤6hr, TaskValidateUtxos ≤30min |
| A4 | Send unproven, backup deferred, shutdown | Wait | **Medium** | Stale backup — inputs appear spendable but spent on-chain. TaskSyncPending reconciles |
| A5 | Send, flag cleared without backup (old bug) | Wait | **Medium-High** | Same as A4 but 3-hour exposure window |
| B1 | Receive confirmed, backed up | Either | None | Perfect |
| B2 | Receive unproven, included, confirms | Include | **Low** | Good — proof acquired later |
| B3 | Receive unproven, included, FAILS | Include | **Low** | TaskValidateUtxos marks external-spend ≤30min |
| B4 | Receive confirmed, no backup (old bug) | Wait | **Very Low** | BIP32 recovery rediscovers via address scan |

#### PeerPay Send/Receive (BRC-42 Derived)

| # | Scenario | Strategy | Severity | What Happens on Recovery |
|---|----------|----------|----------|--------------------------|
| C1 | PeerPay send confirmed, backed up | Either | None | Perfect |
| C2 | PeerPay send unproven, included, confirms | Include | **Low** | Good — same as A2 |
| C3 | PeerPay send unproven, included, FAILS | Include | **Medium** | Same as A3 — ghost cleanup |
| C4 | PeerPay send, no backup (old bug) | Wait | **Medium** | Inputs spendable in backup but spent on-chain |
| D1 | PeerPay receive confirmed, backed up | Either | None | Perfect |
| D2 | PeerPay receive, no backup (old bug) | Wait | **HIGH** | **BRC-42 output LOST** — mnemonic recovery won't find it (BRC-42 scanning disabled). No safeguard. |
| D3 | PeerPay receive unproven, included, confirms | Include | **Low** | Good — output on-chain, proof later |
| D4 | PeerPay receive unproven, included, FAILS | Include | **Low** | Ghost BRC-42 output — BEEF fails gracefully, dead weight, no corruption |

#### PushDrop/Token Create & Receive

| # | Scenario | Strategy | Severity | What Happens on Recovery |
|---|----------|----------|----------|--------------------------|
| E1 | Token created, confirmed, backed up | Either | None | Perfect |
| E2 | Token unproven, included, confirms | Include | **Low** | Good — token on-chain, proof later |
| E3 | Token unproven, included, FAILS | Include | **Medium-High** | Ghost token appears in wallet, can't spend (BEEF fails). TaskCheckForProofs cleans up ≤6hr. TaskValidateUtxos SKIPS tokens. |
| E4 | Token confirmed, no backup (old bug) | Wait | **CRITICAL** | **Token PERMANENTLY LOST** — can't rediscover via address scan, TaskValidateUtxos skips it. No safeguard. |
| F1 | Token received, confirmed, backed up | Either | None | Perfect |
| F2 | Token received, no backup (old bug) | Wait | **CRITICAL** | **Token PERMANENTLY LOST** — same as E4 |
| F3 | Token received unproven, included, FAILS | Include | **Medium** | Ghost token — dead weight until TaskCheckForProofs cleanup |

### Risk Summary

| Severity | Scenarios | Root Cause |
|----------|-----------|------------|
| **CRITICAL** | E4, F2 | Flag consumed without backup → token/PushDrop never backed up → unrecoverable on recovery |
| **HIGH** | D2 | Flag consumed → BRC-42 PeerPay output never backed up → lost on mnemonic-only recovery |
| **MEDIUM-HIGH** | A5, E3 | Extended no-backup window OR ghost token persists until 6hr timeout |
| **MEDIUM** | A3, A4, C3, C4 | Ghost outputs or stale backup — safeguards catch within minutes-hours |
| **LOW** | A2, B2, B3, C2, D3, D4, E2, F3 | Temporary inconsistency, auto-resolved |

### Key Insight

The worst outcomes (CRITICAL) come from the backup **never running** — not from backing up unproven transactions. Ghost outputs from failed txs are self-healing (MEDIUM at worst). Missing tokens are permanent (CRITICAL).

### Implemented Safeguards

1. **BackupOutcome enum**: `task_backup::run()` returns Broadcast/Skipped/Deferred/Failed. Flag only cleared for Broadcast/Skipped — Deferred/Failed retries on next 30s tick.
2. **Timer reset with cap**: New events extend the 3-minute delay window, hard-capped at 10 minutes from first event (prevents infinite deferral).
3. **Post-recovery validation**: After on-chain backup restore, `recovery_just_completed` AtomicBool triggers TaskCheckForProofs + TaskValidateUtxos immediately on next Monitor tick (~30s), not on normal intervals (60s / 30min). This cleans up any ghost outputs or validates unproven transactions from the restored backup.

---

## 11. Open Questions for Implementation

1. **Token amount**: 1000 sats vs 546 sats (dust limit)?
   - 1000 sats is safer against cleanup sweeps
   - Still negligible cost

2. **Backup history**: Keep only latest, or chain of backups?
   - Recommend: latest only (simpler, old is spent anyway)
   - Could add "backup-archive" address for historical if needed

3. **Maximum size**: At what point split across multiple outputs?
   - Current max OP_RETURN: 100 KB (policy, not consensus)
   - PushDrop: no explicit limit
   - Recommend: warn user if backup > 200 KB, suggest cleanup

4. **Merge conflict UX**: What if merge finds conflicts?
   - Recommend: auto-resolve with newer wins
   - Log conflicts for debugging
   - Could add manual resolution later

---

## 12. Conclusion

**Matt's approach is not only feasible but superior to the current plan.**

### Why It Works
1. Compression is exceptional (96-97% reduction)
2. BSV fees are negligible ($0.08-7/year)
3. PushDrop recovers sats on updates
4. Single address = instant recovery
5. Built-in multi-device sync

### Recommended Next Steps
1. Update `WALLET_BACKUP_AND_RECOVERY_PLAN.md` with this approach
2. Implement compression + encryption pipeline
3. Add PushDrop backup to certificate publish (Sprint 3) infrastructure
4. Test with real wallet data

---

*Research conducted by Edwin, 2026-03-16*
