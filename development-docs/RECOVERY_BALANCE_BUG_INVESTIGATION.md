# Recovery Balance Inflation Bug — Investigation Guide

**Date:** 2026-04-17
**Status:** ACTIVE — not yet fixed
**Priority:** CRITICAL — real money discrepancy

## The Problem

After on-chain wallet recovery, the balance is inflated by ~15M sats ($2.47).

| Wallet | Balance (sats) |
|--------|---------------|
| Old (pre-recovery) | 51,111,017 |
| Recovered | 66,107,784 |
| **Difference** | **14,996,767** |

The backup tx `2a681aaa70c9a091984b8976f8244ff2571d08179c8b4e6e8d7ecd5b9295b771` is the source. Its change output (vout 2, 14,989,811 sats) is being double-counted.

## Evidence

### Old wallet activity for backup tx:
```
2a681aaa — direction: sent — 7,955 sats — source: wallet — "On-chain wallet backup"
2a681aaa — direction: received — 1,546 sats — source: address_sync — "Received BSV"
```

### Recovered wallet activity for backup tx:
```
2a681aaa — direction: received — 14,991,357 sats — source: address_sync — "Received BSV"
```
(14,989,811 change + 1,000 PushDrop + 546 marker = 14,991,357)

The backup tx doesn't show as "sent" at all in the recovered wallet — only as a massive "received."

## Recovery Flow (trace these steps)

### Step 1: Backup creation
- `POST /wallet/backup/onchain` creates the backup tx
- Immediately after, it collects a new backup payload and stores the hash
- The backup payload contains 271 outputs (80 stripped from 351 total)
- **KEY QUESTION:** Does the payload include `2a681aaa:2` (the change output)?

### Step 2: Recovery import
- `POST /wallet/recover/onchain` finds the backup on-chain
- Calls `import_to_db()` → `import_entities()` — inserts all 271 outputs via plain `INSERT`
- File: `rust-wallet/src/backup.rs` line 1329-1349

### Step 3: Reconciliation
- `reconcile_backup_tx()` parses the backup tx and:
  - Marks inputs as spent (funding UTXOs)
  - Inserts vout 0 (PushDrop) via `insert_output` — plain INSERT
  - Inserts vout 1 (marker) via `insert_output` — plain INSERT  
  - Inserts vout 2 (change) via `upsert_received_utxo` — INSERT OR IGNORE
- File: `rust-wallet/src/handlers.rs` line 12248-12372

### Step 4: Monitor starts
- TaskSyncPending discovers outputs at wallet addresses
- Finds `2a681aaa:2` at address index 233 → inserts as "received"
- Also finds PushDrop + marker at backup address → inserts as "received"
- File: `rust-wallet/src/monitor/task_sync_pending.rs`

## Balance Calculation
```sql
SELECT COALESCE(SUM(o.satoshis), 0)
FROM outputs o
LEFT JOIN transactions t ON o.transaction_id = t.id
WHERE o.user_id = ?1 AND o.spendable = 1
  AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nonfinal'))
  AND COALESCE(o.derivation_prefix, '') != '1-wallet-backup'
```
File: `rust-wallet/src/database/output_repo.rs` line 269-286

Note: Excludes `1-wallet-backup` prefix outputs. The change output has `2-receive address` prefix so it IS included.

## UNIQUE constraint
`outputs` table has `UNIQUE(txid, vout)` — so `INSERT OR IGNORE` should prevent true duplicates. But there may be a scenario where:
- The backup import inserts the output with one set of attributes
- TaskSyncPending tries to insert it again → OR IGNORE skips
- BUT if the backup STRIPPED the output, then TaskSyncPending is the FIRST insert and it creates it with wrong attributes (e.g., as "received" instead of internal change)

## Investigation Steps

1. **Determine if `2a681aaa:2` is in the backup payload:**
   - Add logging to the stripping logic in `backup.rs`
   - Or: decrypt and inspect the backup JSON directly
   - Or: check the 271 count vs what outputs the backup tx has

2. **Check the outputs table in both DBs:**
   - Old wallet: how many rows for txid `2a681aaa`, what are their attributes?
   - Recovered wallet: same query — are there duplicates or different attributes?
   - Need sqlite3 CLI or add a debug endpoint

3. **Trace the exact insert sequence:**
   - Add logging to `reconcile_backup_tx` to show whether INSERT OR IGNORE skipped
   - Add logging to TaskSyncPending when it finds outputs from known txids

4. **Fix options (after diagnosis):**
   - If stripping removes the change: don't strip backup tx change outputs
   - If TaskSyncPending duplicates: skip txids already in the DB
   - If reconciliation duplicates: check existence before inserting
   - Nuclear option: reconciliation should ONLY mark inputs as spent, never insert outputs (let the backup import handle all output creation)

## Related Issues (same session)

### TaskSyncPending phantom "received" entries
Every outgoing tx to self (service fees) shows as both "sent" and "received." Cosmetic on old wallet, potentially harmful on recovered wallet. This is partly because the test wallet sends service fees to its own master address.

### Identity key not showing post-recovery
Dashboard doesn't show identity key. x.com cert shows as "private" even though it's resolving on the overlay. May be a DB state issue from import.

### Certificate public/private mismatch
Cert is on the overlay but DB says private. See `project_overlay_unpublish_issue.md`.

## Current State
- Old wallet.db saved as backup file
- Recovered wallet.db also available
- Both can be swapped for testing
- Broadcast resilience changes committed at `63cda01`
- Do NOT make on-chain actions from recovered wallet until balance is fixed
