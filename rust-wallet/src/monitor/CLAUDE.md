# Monitor — Background Task Scheduler

> Single tokio task that runs 14 named background tasks on configurable intervals, handling transaction lifecycle, proof acquisition, UTXO sync, PeerPay delivery, on-chain backup, dust consolidation, double-spend verification, and SHIP cache warming.

**Last Updated:** 2026-08-03

> This file is the authoritative roster of Monitor tasks. Intervals below are read from
> `monitor/mod.rs :: TaskSchedule::default()`; the task list is `monitor/mod.rs`'s `pub mod` declarations.

## Overview

The Monitor replaces the ad-hoc background services (`arc_status_poller`, `cache_sync`, `utxo_sync`) with a structured scheduler. It runs as a single `tokio::spawn` task with a 30-second tick loop. Each tick, it checks which tasks are due based on elapsed time and runs them sequentially. The Monitor uses `try_lock()` on the database mutex before each tick — if a user HTTP request holds the lock, the DB-touching portion of the tick is skipped to avoid blocking the user.

**Key design decisions:**
- `AtomicBool` (`MONITOR_STARTED`) prevents duplicate loops (safe to call `Monitor::start()` from both `wallet_create` and `wallet_recover`)
- `Monitor::start()` returns `Option<tokio::task::JoinHandle<()>>` so the shutdown coordinator can quiesce the Monitor with a bounded join before process exit (OD-2). `None` is returned on the duplicate-start no-op path.
- Graceful shutdown via `CancellationToken` (Phase 8D) — `tokio::select!` checks `state.shutdown.cancelled()` each tick, plus mid-tick `shutdown.is_cancelled()` breaks after in-flight tasks
- All tasks receive `&web::Data<AppState>` and optionally `&reqwest::Client` — no task owns persistent state (per-task rate/retry state that does exist lives in module-level atomics, e.g. `task_sync_pending`'s `FIRST_RUN` / `LAST_RECENT_CHECK` / `LAST_OLD_CHECK`)
- Error logging to `monitor_events` table via `log_monitor_event()` helper (or `Monitor::log_event()` inside the loop)
- **One task runs outside the DB gate:** `task_refresh_ship_cache` is scheduled *before* the `db_available()` check because it is pure network + memory. A busy DB must never starve SHIP discovery refresh.
- The shared `reqwest::Client` is built with `crate::services::CallClass::IndexerAsync.timeout()`

## Files

15 files: `mod.rs` plus 14 `task_*.rs` modules — one per entry in `TaskSchedule`.

| File | Purpose | Interval |
|------|---------|----------|
| `mod.rs` | `Monitor` struct, `TaskSchedule` (14 fields), tick loop, `MONITOR_STARTED` guard, `Monitor::log_event()`, `log_monitor_event()`, `Monitor::db_available()`, `Monitor::now_secs()` | 30s tick |
| `task_check_for_proofs.rs` | Acquire merkle proofs for `sending`/`unproven`/`nosend` transactions via the `services` tx-status chain + 3-oracle txid quorum | 60s |
| `task_send_waiting.rs` | Crash recovery: re-broadcast or clean up transactions stuck in `sending` status | 120s |
| `task_fail_abandoned.rs` | Fail `unprocessed`/`unsigned` transactions older than 5 minutes (and stuck backup broadcasts older than 10 minutes), restore reserved outputs | 300s |
| `task_unfail.rs` | Recover falsely-failed transactions by checking on-chain status (6-hour window) | 300s |
| `task_review_status.rs` | Consistency: propagate proof completion to transactions, fix output spendable flags, clean stale reservations | 60s |
| `task_purge.rs` | Retention cleanup across 5 tables (`monitor_events`, `proven_tx_reqs`, `parent_transactions`, `peerpay_outbox`, `peerpay_pending_verification`) | 3600s |
| `task_sync_pending.rs` | Tiered UTXO sync for addresses with `pending_utxo_check=1` via WhatsOnChain API (30s fresh / 3m recent / 30m old) | 30s |
| `task_check_peerpay.rs` | Poll MessageBox for incoming BRC-29 PeerPay payments, auto-accept via BRC-42 key derivation | 60s |
| `task_backup.rs` | Periodic on-chain wallet backup via self-call to `/wallet/backup/onchain`; returns `BackupOutcome` | 10800s (3h) |
| `task_replay_overlay.rs` | Retry overlay notification for certificates stuck in `unpublished_pending_overlay` (max 20 attempts) | 300s |
| `task_consolidate_dust.rs` | Daily sweep: consolidate ≤1000-sat UTXOs into one self-output when 20+ accumulate | 86400s (24h) |
| `task_verify_double_spend.rs` | Independent verification of suspected double-spends against WhatsOnChain (SDK-style, never trusts a single broadcaster) | 60s |
| `task_retry_peerpay_outbox.rs` | Retry MessageBox delivery for PeerPay sends that succeeded on-chain but failed to deliver | 30s (fast tick; actual retry gated by `next_retry_at`) |
| `task_refresh_ship_cache.rs` | Keep `AppState.ship_cache` warm for `tm_identity` — **runs outside the `db_available()` gate** | 300s |

**First-tick seeding** (`mod.rs :: Monitor::run`): most `last_*` markers start at `0` so the task fires on the first eligible tick. Exceptions: `last_backup` is seeded from `SettingsRepository::get_last_backup_at()` (so a backup triggers soon after a long shutdown rather than 3 hours later), and `last_consolidate_dust` is seeded to "now" so the daily dust sweep does not run at startup. The loop also sleeps 5 seconds before its first tick.

**Tasks called from outside the Monitor:**
- `task_consolidate_dust::run_inner()` — manual consolidation endpoint in `handlers.rs`
- `task_check_peerpay::run()` — `peerpay_check` handler in `handlers.rs`
- `task_unfail::promote_proven_local_tx()` / `PromoteOutcome` — reconcile path in `handlers.rs`
- `task_replay_overlay::run()`'s BEEF-build sequence is mirrored (not called) by `handlers/certificate_handlers.rs`

## Task Details

### TaskCheckForProofs (`task_check_for_proofs.rs`)

Queries transactions in `sending`/`unproven`/`nosend` status (`MAX_BATCH = 20` per cycle). For each:

1. Check if `proven_txs` record already exists → reconcile statuses
2. Query tx status via `state.services.tx_status()` (multi-tier `WalletServices` chain, mapped from `services::TxState` → `MINED` / `SEEN_ON_NETWORK` / `REJECTED` / `DOUBLE_SPEND_ATTEMPTED` / `UNKNOWN`)
3. Cross-verify aging or weakly-signalled txs with a three-oracle txid quorum (`oracle_quorum_check()` over WhatsOnChain, JungleBus/GorillaPool, Bitails)
4. On confirmation: create `proven_txs` record, update transaction to `confirmed`
5. On failure/rejection: `mark_failed()` with full ghost output cleanup
6. 200ms rate limiting between transactions

**Timeouts / thresholds (all in this file):**

| Constant | Value | Meaning |
|----------|-------|---------|
| `MAX_BATCH` | 20 | Transactions checked per cycle |
| `UNPROVEN_TIMEOUT_SECS` | 6 hours | Give-up window for txs **we** broadcast |
| `NOSEND_TIMEOUT_SECS` | 10 minutes | Give-up window for `nosend` txs the **app** broadcasts |
| `ALL_ORACLES_NOT_FOUND_TIMEOUT_SECS` | 5 minutes | All three oracles 404 for this long → treat as never broadcast |
| `MEMPOOL_VERIFY_THRESHOLD_SECS` | 30 minutes | Age at which a mempool tx is cross-verified |
| `ANNOUNCED_VERIFY_THRESHOLD_SECS` | 3 minutes | `ANNOUNCED_TO_NETWORK` is a weak signal — verify much sooner |

`ORPHAN_TIMEOUT_SECS` no longer exists — orphan/stale txs fail immediately.

**Key functions:**
- `run()` — main task entry point
- `mark_confirmed()` — update tx status + confirmations
- `mark_failed()` — mark failed + delete ghost outputs + restore inputs + invalidate cache
- `reconcile_proven_tx()` — link existing proof to transaction and proof request
- `create_proven_tx_from_arc()` — parse ARC BUMP hex to TSC, store in `proven_txs`
- `fetch_and_store_woc_proof()` — fetch TSC proof from WhatsOnChain API
- `check_whatsonchain_confirmation()` — check confirmations via WoC `/tx/hash/{txid}`
- `try_whatsonchain_confirmation()` — wrapper that also stores proof on confirmation
- `oracle_quorum_check()` + `query_woc_txid()` / `query_junglebus_txid()` / `query_bitails_txid()` — three-way independent existence check, returning `OracleVerdict` from per-oracle `OracleStatus`

### TaskSendWaiting (`task_send_waiting.rs`)

Recovers transactions stuck in `sending` for more than `STUCK_THRESHOLD_SECS` (120s):

1. Query status via `state.services` (`services::TxState`) — if already `MINED`/in mempool, promote to `unproven`
2. If rejected/double-spent, clean up with `cleanup_failed_sending()` / `cleanup_failed_sending_suspected()`
3. If stuck longer than `GIVE_UP_THRESHOLD_SECS` (1800s / 30 minutes), mark failed
4. Otherwise re-broadcast via `crate::handlers::broadcast_transaction()`
5. Distinguishes permanent vs transient errors via `is_permanent_error()`

**Key functions:**
- `promote_to_unproven()` — update status, ensure `proven_tx_req` exists for proof tracking
- `cleanup_failed_sending()` / `cleanup_failed_sending_suspected()` → shared `cleanup_failed_sending_impl(.., is_double_spend)` — full failure cleanup (same sequence as the broadcast failure handler). Suspected double-spends are tagged with `crate::arc_status::SUSPECTED_DOUBLE_SPEND_PREFIX` (`dss:{txid}`) rather than being marked permanently lost — `TaskVerifyDoubleSpend` adjudicates.
- `is_permanent_error()` — thin delegate to `crate::arc_status::is_fatal_broadcast_error()` (classification lives in `arc_status`, not here)
- `get_beef_for_rebroadcast()` — rebuild the BEEF needed for the retry broadcast

### TaskFailAbandoned (`task_fail_abandoned.rs`)

Finds transactions in `unprocessed`/`unsigned` status older than `ABANDON_THRESHOLD_SECS` (300s). These are transactions that were created but never completed signing or broadcasting. Also sweeps **backup** transactions stuck in `sending` longer than `BACKUP_SENDING_THRESHOLD_SECS` (600s) — normal sends legitimately sit in `sending` longer (TaskSendWaiting handles those at 30 min), but backup broadcasts should complete in under a minute.

Cleanup follows the ghost transaction safety sequence: mark failed → delete ghost outputs → restore inputs → invalidate balance cache. Repos are constructed inside each loop iteration so every transaction's cleanup lands in its own SQLite transaction (per-tx atomicity against an unclean kill — OD-2).

### TaskUnFail (`task_unfail.rs`)

Re-checks failed transactions within a 6-hour window (`UNFAIL_WINDOW_SECS = 6 * 60 * 60`, matched to `UNPROVEN_TIMEOUT_SECS`). Recovery path:

1. Check if `proven_txs` record exists → recover immediately
2. Query on-chain status for `MINED` → create proof record + recover (`create_proven_tx_and_recover()`)
3. Fallback to direct WhatsOnChain confirmation + TSC proof fetch → recover

**On recovery** (`recover_transaction()`):
- Updates status to `confirmed`, links proof
- Re-marks inputs as spent by parsing raw_tx outpoints (reverses `mark_failed()`'s input restoration)
- Does NOT re-create deleted change outputs — relies on `/wallet/sync` or `TaskSyncPending`

**Shared API:** `pub(crate) fn promote_proven_local_tx()` returning `pub(crate) enum PromoteOutcome { Promoted, NotApplicable, TransientFailed }` — reused by the reconcile path in `handlers.rs`, so this module is not Monitor-private.

### TaskReviewStatus (`task_review_status.rs`)

Three consistency checks in a single DB lock:

1. **Proof propagation**: Find `proven_tx_reqs` with `status='completed', notified=0` → mark parent transaction as `confirmed`, link `proven_tx`, set `notified=1`
2. **Output spendability**: Find outputs belonging to `completed` transactions with `spendable=0`, `spent_by IS NULL` **and `spending_description IS NULL`** → set `spendable=1`. The `spending_description IS NULL` guard is load-bearing: if anything already tagged the output (`spent-by:*`, `dss:*`, `double-spend-detected`, `external-spend`, `stale-backup`, `spent-by-backup-*`, `failed-tx-output`, or a txid reservation) this task must not override that judgment.
3. **Stale reservation cleanup**: Find failed transactions past the 30-minute cutoff that still have reserved outputs → `restore_spent_by_txid()`, falling back to `restore_by_spending_description()`

Balance cache is invalidated only when at least one of the three counters moved.

### TaskPurge (`task_purge.rs`)

Five retention sweeps:

| Target | Retention | Constant / call |
|--------|-----------|-----------------|
| `monitor_events` | 7 days | `EVENTS_RETENTION_SECS` |
| `proven_tx_reqs` (`status='completed' AND notified=1`) | 30 days | `PROOF_REQS_RETENTION_SECS` |
| `parent_transactions` that are confirmed (`txid IN proven_txs`) | 7 days | `PARENT_TX_RETENTION_SECS` — re-fetchable from WoC |
| delivered `peerpay_outbox` entries | 7 days | `PeerPayRepository::remove_delivered_outbox()` |
| expired `peerpay_pending_verification` | 24 hours | `PENDING_VERIFICATION_RETENTION_SECS` + `PeerPayRepository::cleanup_expired_pending()` |

Immutable `proven_txs` records are kept permanently.

### TaskSyncPending (`task_sync_pending.rs`)

Syncs addresses flagged with `pending_utxo_check=1`, tiered by address age so old addresses don't burn API calls every 30 seconds:

| Tier | Address age | Check cadence | Mode |
|------|-------------|---------------|------|
| Fresh | 0–3 h (`FRESH_THRESHOLD_SECS`) | every tick (30s) | individual (includes mempool/unconfirmed) |
| Recent | 3–18 h (`RECENT_THRESHOLD_SECS`) | every 3 min (`RECENT_CHECK_INTERVAL_SECS`) | individual |
| Old | 18 h+ | every 30 min (`OLD_CHECK_INTERVAL_SECS`) | bulk (`check_addresses_bulk`, confirmed only) |

On the first run after startup (`FIRST_RUN` atomic), **all** pending addresses are checked individually as a startup sweep, then the tier timestamps are reset.

Per-cycle behavior:

1. Clear stale pending flags older than `PENDING_TIMEOUT_HOURS` (2160 h = 90 days)
2. Fetch UTXOs from WhatsOnChain for each due address (DB lock released during network calls)
3. Insert new outputs via `upsert_received_utxo()`, record notifications via `PeerPayRepository::insert_address_sync_notification()` with a BSV/USD price snapshot from `price_cache`
4. Reconcile stale outputs: mark DB outputs not found in API as `external-spend`
5. Cache parent transaction raw hex (`cache_parent_transactions()`) for future BEEF building
6. `check_stale_unconfirmed()` re-checks outputs unconfirmed for more than `UNCONFIRMED_CHECK_SECS` (30 min)
7. Pending flag is NOT cleared on discovery — kept for the full 90-day window (addresses may be reused)

### TaskCheckPeerPay (`task_check_peerpay.rs`)

Polls the remote MessageBox API for incoming BRC-29 PeerPay payments:

1. Build `MessageBoxClient` with wallet's master private/public keys
2. List messages from `payment_inbox` (BRC-103 authenticated, BRC-2 decrypted)
3. Deduplicate via `PeerPayRepository::is_already_processed()`
4. Parse `PaymentToken` / `PaymentInstructions` flexibly via `parse_payment_token()` (base64 string OR byte array for the transaction field)
5. Derive child private key via BRC-42 with `invoice_number = "2-3241645161d8-{prefix} {suffix}"`
6. Parse Atomic BEEF, find matching P2PKH output by comparing `HASH160(child_pubkey)` to script
7. Store as spendable output via `crate::handlers::store_derived_utxo()`, record in `peerpay_received` with a BSV/USD price snapshot (`price_cache.get_cached()` → `get_stale()` fallback) for historical display
8. Payments that parse but can't yet be verified on-chain are parked via `PeerPayRepository::upsert_pending_verification()` and cleared with `remove_pending_verification()` once stored (TaskPurge expires these after 24 h)
9. Cache all BEEF transactions in `parent_transactions` for future BEEF building
10. Acknowledge processed messages on MessageBox server (idempotent — duplicates are safe)

**Error handling:** Parse failures on payment tokens skip without acknowledging (retry next tick). Storage failures also skip without acknowledging. MessageBox API errors return `Ok(())` to retry next tick.

### TaskBackup (`task_backup.rs`)

Periodic on-chain wallet backup. Runs on the 3-hour schedule **or** early when `AppState.backup_check_needed` holds a significant-event timestamp older than 180 seconds (3-minute debounce from the latest event).

Preconditions checked under one `try_lock()` (no network): wallet exists, DB unlocked, balance ≥ `MIN_BACKUP_BALANCE_SATS` (3000 — token + marker + fee buffer). Then self-calls `POST http://127.0.0.1:{crate::wallet_port()}/wallet/backup/onchain` (60s timeout); the handler does hash comparison and skips if nothing changed.

`pub enum BackupOutcome { Broadcast(String), Skipped, Deferred(String), Failed(String) }` with `is_current()` returning true for `Broadcast`/`Skipped`. The Monitor clears `backup_check_needed` and advances `last_backup` **only** when `is_current()` — `Deferred`/`Failed` keep the flag so the next tick retries.

> `crate::wallet_port()` is the source of the port here — 31301 release / 31401 under `HODOS_DEV=1`. Never hardcode.

### TaskReplayOverlay (`task_replay_overlay.rs`)

Retries overlay notification for certificates whose on-chain unpublish succeeded but whose overlay submission did not (`publish_status = 'unpublished_pending_overlay'`). Builds a minimal BEEF (publish tx + spending tx, both with merkle proofs) and submits it to the overlay; once the overlay confirms removal (or lookup shows the cert is gone), status becomes `unpublished`.

- First attempt roughly 10 minutes after unpublish (waits for block confirmation), then every 5 minutes (this task's interval)
- `MAX_OVERLAY_RETRIES = 20`, tracked in the `certificates.overlay_retry_count` column (added idempotently by an `ALTER TABLE ... ADD COLUMN` at the top of `run()`, which silently fails when the column already exists)
- After 20 failures the status stays `unpublished_pending_overlay` for manual attention
- Helpers: `increment_retry_count()`, `resolve_block_height()`

### TaskConsolidateDust (`task_consolidate_dust.rs`)

Daily sweep that consolidates dust UTXOs into a single self-output.

| Constant | Value | Meaning |
|----------|-------|---------|
| `DUST_THRESHOLD_SATS` | 1000 | Outputs at or below this are consolidation candidates |
| `MIN_DUST_COUNT` | 20 | Minimum dust UTXOs before consolidating |
| `DUST_LIMIT_SATS` | 546 | Bitcoin dust limit — abort if net output would land below it |

Opt-out via the `disable_dust_consolidation` row in `settings`. Reads confirmed dust from the default basket, derives a fresh self destination via `crypto::brc42::derive_child_public_key`, signs with `derive_key_for_output` + ForkID `calculate_sighash`, pays the standard `HODOS_SERVICE_FEE_SATS` output to `HODOS_FEE_ADDRESS` and records it in `commissions`, then broadcasts via `crate::handlers::broadcast_transaction`.

Public API: `run()` (Monitor path, collapses outcomes to `Ok(())`) and `run_inner()` → `pub enum ConsolidateResult { Consolidated { txid, input_count, net_sats }, Skipped(String) }` (manual endpoint path in `handlers.rs`). Internal: `is_p2pkh_script()`, `struct DustUtxo`.

### TaskVerifyDoubleSpend (`task_verify_double_spend.rs`)

Independent adjudication of double-spend suspicions. When ARC reports `DOUBLE_SPEND_ATTEMPTED`, inputs are tagged `spending_description = 'dss:{our_txid}'` (suspected, not confirmed — see `crate::arc_status::SUSPECTED_DOUBLE_SPEND_PREFIX`). This task verifies each suspicion against WhatsOnChain rather than trusting a single broadcaster, mirroring the BSV SDK's `TaskReviewDoubleSpends`:

1. Check our txid on WoC — known / mined / unknown (`check_our_txid_on_woc()` → `enum TxidStatus`)
2. If known → false alarm, recover (`handle_false_alarm_mined()` / `handle_false_alarm_mempool()`)
3. If unknown → check each input via `/tx/{txid}/{vout}/spent` (`check_output_spent()` → `struct SpentInfo`)
4. Input spent by another tx → confirmed double-spend
5. Input not spent → false alarm, restore as spendable

| Constant | Value | Meaning |
|----------|-------|---------|
| `MAX_GROUPS_PER_TICK` | 5 | Suspected-tx groups processed per tick |
| `WOC_CALL_DELAY_MS` | 500 | Delay between individual WoC calls |
| `ESCALATION_CONFIRM_SECS` | 6 hours | Promote to confirmed double-spend regardless after this |
| `TXID_CHECK_RETRIES` | 3 | Retries when checking our txid status on WoC |

### TaskRetryPeerPayOutbox (`task_retry_peerpay_outbox.rs`)

When a PeerPay send succeeds on-chain but MessageBox delivery fails, the payload is queued in `peerpay_outbox`. This task retries delivery on an escalating schedule:

- First 10 retries (~10 min): every 60s
- Next 10 retries (~20 min): every 120s
- After 20 retries (~30 min total): marked `exhausted` (user can manually retry)

The task itself ticks every 30 seconds; the actual retry timing is governed by `next_retry_at` on the row (`PeerPayRepository::get_due_outbox_entries()`). Successes call `mark_outbox_delivered()`, failures call `update_outbox_retry_failed()`. 500ms rate limit between entries when more than one is due. Delivered rows are purged by TaskPurge after 7 days.

### TaskRefreshShipCache (`task_refresh_ship_cache.rs`)

Keeps `AppState.ship_cache` warm for the `tm_identity` topic (`crate::overlay::TOPIC_IDENTITY`) so publish/unpublish never pays the ~75s SHIP discovery round-trip on the hot path — `discover_hosts_for_topic` becomes a synchronous cache hit.

- Interval 300s, chosen to match `ship_cache::FRESH_TTL` so the cache never enters the stale window during normal operation
- **Scheduled before the `db_available()` gate** in `mod.rs :: Monitor::run` — pure network + memory, never touches SQLite
- Failures are logged with `warn!` only; it deliberately does **not** call `log_event()`, because that would touch the DB and defeat the point of being DB-independent
- If more overlay topics enter the publish flow, add them to this task's `run()` so each stays warm

## Ghost Transaction Safety

All tasks that modify outputs follow a strict cleanup sequence:

```
1. Mark transaction as failed (with failed_at timestamp)
2. Delete ghost change outputs created by the failed transaction
3. Restore input outputs that were reserved (spent_by) for the failed transaction
4. Invalidate balance cache
```

Key invariants:
- Background tasks never create output records — the exceptions are `TaskSyncPending` and `TaskCheckPeerPay` (which sync from external sources) and `TaskConsolidateDust` (which creates a genuine self-spend transaction and its change/self output)
- `TaskUnFail` does NOT re-create deleted outputs — relies on UTXO sync
- `TaskReviewStatus` only updates `spendable` flags on existing outputs, never creates or deletes, and never overrides a non-NULL `spending_description`
- A double-spend report is a *suspicion* (`dss:` prefix) until `TaskVerifyDoubleSpend` confirms it independently
- Balance cache is always invalidated after any output change

## DB Lock Discipline

All tasks follow a pattern of holding the DB lock for minimal duration:

1. Acquire lock briefly to read data into local `Vec`
2. Drop lock before making network calls (services chain, WhatsOnChain, MessageBox, overlay)
3. Re-acquire lock to write results

The Monitor's `db_available()` check at the tick level provides a coarse-grained contention avoidance — if any user request holds the lock, every DB-touching task skips that tick. `TaskRefreshShipCache` is deliberately scheduled above that gate and runs anyway. `TaskBackup` additionally uses its own `try_lock()` and returns `Deferred("DB busy")` rather than blocking.

## Related

- [`../CLAUDE.md`](../CLAUDE.md) — Rust wallet source modules: `AppState`, handlers, caches
- [`../../CLAUDE.md`](../../CLAUDE.md) — Rust wallet backend layer overview, build, invariants
- [`../database/CLAUDE.md`](../database/CLAUDE.md) — Database schema, repository pattern, output model
- [`../../../CLAUDE.md`](../../../CLAUDE.md) — Project root: architecture and cross-layer contracts (this file is the roster of record for Monitor tasks)
