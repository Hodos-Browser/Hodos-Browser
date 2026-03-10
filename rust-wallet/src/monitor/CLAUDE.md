# Monitor — Background Task Scheduler

> Single tokio task that runs 8 named background tasks on configurable intervals, handling transaction lifecycle, proof acquisition, UTXO sync, and PeerPay polling.

## Overview

The Monitor replaces the ad-hoc background services (`arc_status_poller`, `cache_sync`, `utxo_sync`) with a structured scheduler. It runs as a single `tokio::spawn` task with a 30-second tick loop. Each tick, it checks which tasks are due based on elapsed time and runs them sequentially. The Monitor uses `try_lock()` on the database mutex before each tick — if a user HTTP request holds the lock, the entire tick is skipped to avoid blocking the user.

**Key design decisions:**
- `AtomicBool` (`MONITOR_STARTED`) prevents duplicate loops (safe to call `Monitor::start()` from both `wallet_create` and `wallet_recover`)
- Graceful shutdown via `CancellationToken` (Phase 8D) — `tokio::select!` checks `state.shutdown.cancelled()` each tick
- All tasks receive `&web::Data<AppState>` and optionally `&reqwest::Client` — no task owns persistent state
- Error logging to `monitor_events` table via `log_monitor_event()` helper

## Files

| File | Purpose | Interval |
|------|---------|----------|
| `mod.rs` | `Monitor` struct, `TaskSchedule`, tick loop, `MONITOR_STARTED` guard, `log_monitor_event()` | 30s tick |
| `task_check_for_proofs.rs` | Acquire merkle proofs for `sending`/`unproven`/`nosend` transactions via ARC + WhatsOnChain fallback | 60s |
| `task_send_waiting.rs` | Crash recovery: re-broadcast or clean up transactions stuck in `sending` status | 120s |
| `task_fail_abandoned.rs` | Fail `unprocessed`/`unsigned` transactions older than 5 minutes, restore reserved outputs | 300s |
| `task_unfail.rs` | Recover falsely-failed transactions by checking on-chain status (6-hour window) | 300s |
| `task_review_status.rs` | Consistency: propagate proof completion to transactions, fix output spendable flags, clean stale reservations | 60s |
| `task_purge.rs` | Delete old `monitor_events` (7 days) and completed `proven_tx_reqs` (30 days) | 3600s |
| `task_sync_pending.rs` | UTXO sync for addresses with `pending_utxo_check=1` via WhatsOnChain API | 30s |
| `task_check_peerpay.rs` | Poll MessageBox for incoming BRC-29 PeerPay payments, auto-accept via BRC-42 key derivation | 60s |

## Task Details

### TaskCheckForProofs (`task_check_for_proofs.rs`)

Queries transactions in `sending`/`unproven`/`nosend` status (max 20 per cycle). For each:

1. Check if `proven_txs` record already exists → reconcile statuses
2. Query ARC for tx status → handle `MINED`, mempool states, `SEEN_IN_ORPHAN_MEMPOOL`, rejections
3. For mempool txs older than 30 minutes, cross-verify with WhatsOnChain
4. On confirmation: create `proven_txs` record, update transaction to `confirmed`
5. On failure/rejection: `mark_failed()` with full ghost output cleanup
6. 200ms rate limiting between transactions

**Timeouts:** 6 hours for broadcast txs (`UNPROVEN_TIMEOUT_SECS`), 48 hours for nosend txs (`NOSEND_TIMEOUT_SECS`).

**Key functions:**
- `run()` — main task entry point
- `mark_confirmed()` — update tx status + confirmations
- `mark_failed()` — mark failed + delete ghost outputs + restore inputs + invalidate cache
- `reconcile_proven_tx()` — link existing proof to transaction and proof request
- `create_proven_tx_from_arc()` — parse ARC BUMP hex to TSC, store in `proven_txs`
- `fetch_and_store_woc_proof()` — fetch TSC proof from WhatsOnChain API
- `check_whatsonchain_confirmation()` — check confirmations via WoC `/tx/hash/{txid}`
- `try_whatsonchain_confirmation()` — wrapper that also stores proof on confirmation

### TaskSendWaiting (`task_send_waiting.rs`)

Recovers transactions stuck in `sending` for >120 seconds:

1. Query ARC status — if already `MINED`/in mempool, promote to `unproven`
2. If rejected/double-spent, clean up with `cleanup_failed_sending()`
3. If stuck >30 minutes (`GIVE_UP_THRESHOLD_SECS`), mark failed
4. Otherwise re-broadcast via `crate::handlers::broadcast_transaction()`
5. Distinguishes permanent vs transient errors via `is_permanent_error()`

**Key functions:**
- `promote_to_unproven()` — update status, ensure `proven_tx_req` exists for proof tracking
- `cleanup_failed_sending()` — full failure cleanup (same sequence as broadcast failure handler)
- `is_permanent_error()` — classifies errors: script failures, double-spend, missing inputs = permanent; timeouts, HTTP 5xx = transient

### TaskFailAbandoned (`task_fail_abandoned.rs`)

Finds transactions in `unprocessed`/`unsigned` status older than 5 minutes (`ABANDON_THRESHOLD_SECS`). These are transactions that were created but never completed signing or broadcasting. Cleanup follows the ghost transaction safety sequence: mark failed → delete ghost outputs → restore inputs → invalidate balance cache.

### TaskUnFail (`task_unfail.rs`)

Re-checks failed transactions within a 6-hour window (`UNFAIL_WINDOW_SECS`). Recovery path:

1. Check if `proven_txs` record exists → recover immediately
2. Query ARC for `MINED` status → create proof record + recover
3. Fallback to WhatsOnChain confirmation check → fetch TSC proof + recover

**On recovery** (`recover_transaction()`):
- Updates status to `confirmed`, links proof
- Re-marks inputs as spent by parsing raw_tx outpoints (reverses `mark_failed()`'s input restoration)
- Does NOT re-create deleted change outputs — relies on `/wallet/sync` or `TaskSyncPending`

### TaskReviewStatus (`task_review_status.rs`)

Three consistency checks in a single DB lock:

1. **Proof propagation**: Find `proven_tx_reqs` with `status='completed', notified=0` → mark parent transaction as `confirmed`, set `notified=1`
2. **Output spendability**: Find outputs belonging to `completed` transactions that have `spendable=0` and no `spent_by` → set `spendable=1` (excludes `external-spend` outputs)
3. **Stale reservation cleanup**: Find failed transactions past the 30-minute UnFail window that still have reserved outputs → restore inputs

### TaskPurge (`task_purge.rs`)

Retention cleanup:
- `monitor_events` older than 7 days (`EVENTS_RETENTION_SECS`)
- `proven_tx_reqs` with `status='completed' AND notified=1` older than 30 days (`PROOF_REQS_RETENTION_SECS`)
- Immutable `proven_txs` records are kept permanently

### TaskSyncPending (`task_sync_pending.rs`)

Syncs addresses flagged with `pending_utxo_check=1`:

1. Clear stale pending flags older than 90 days (`PENDING_TIMEOUT_HOURS`)
2. Fetch UTXOs from WhatsOnChain for each pending address (DB lock released during network calls)
3. Insert new outputs via `upsert_received_utxo()`, record notifications via `PeerPayRepository::insert_address_sync_notification()`
4. Reconcile stale outputs: mark DB outputs not found in API as `external-spend` (10-minute grace period)
5. Cache parent transaction raw hex from WhatsOnChain for future BEEF building
6. Pending flag is NOT cleared on discovery — kept for full 90-day window (addresses may be reused)

### TaskCheckPeerPay (`task_check_peerpay.rs`)

Polls the remote MessageBox API for incoming BRC-29 PeerPay payments:

1. Build `MessageBoxClient` with wallet's master private/public keys
2. List messages from `payment_inbox` (BRC-103 authenticated, BRC-2 decrypted)
3. Deduplicate via `PeerPayRepository::is_already_processed()`
4. Parse `PaymentToken` flexibly (base64 string OR byte array for transaction field)
5. Derive child private key via BRC-42 with `invoice_number = "2-3241645161d8-{prefix} {suffix}"`
6. Parse Atomic BEEF, find matching P2PKH output by comparing `HASH160(child_pubkey)` to script
7. Store as spendable output via `store_derived_utxo()`, record in `peerpay_received`
8. Cache all BEEF transactions in `parent_transactions` for future BEEF building
9. Acknowledge processed messages on MessageBox server (idempotent — duplicates are safe)

**Error handling:** Parse failures on payment tokens skip without acknowledging (retry next tick). Storage failures also skip without acknowledging. MessageBox API errors return `Ok(())` to retry next tick.

## Ghost Transaction Safety

All tasks that modify outputs follow a strict cleanup sequence:

```
1. Mark transaction as failed (with failed_at timestamp)
2. Delete ghost change outputs created by the failed transaction
3. Restore input outputs that were reserved (spent_by) for the failed transaction
4. Invalidate balance cache
```

Key invariants:
- Background tasks never create output records (except `TaskSyncPending` and `TaskCheckPeerPay` which sync from external sources)
- `TaskUnFail` does NOT re-create deleted outputs — relies on UTXO sync
- `TaskReviewStatus` only updates `spendable` flags on existing outputs, never creates or deletes
- Balance cache is always invalidated after any output change

## DB Lock Discipline

All tasks follow a pattern of holding the DB lock for minimal duration:

1. Acquire lock briefly to read data into local `Vec`
2. Drop lock before making network calls (ARC, WhatsOnChain, MessageBox)
3. Re-acquire lock to write results

The Monitor's `db_available()` check at the tick level provides a coarse-grained contention avoidance — if any user request holds the lock, all tasks skip that tick.

## Related

- [`../CLAUDE.md`](../CLAUDE.md) — Rust wallet backend overview, `AppState`, endpoint handlers
- [`../database/CLAUDE.md`](../database/CLAUDE.md) — Database schema, repository pattern, output model
- [`../../CLAUDE.md`](../../CLAUDE.md) — Project root: architecture, Monitor task table, ghost transaction safety rules
