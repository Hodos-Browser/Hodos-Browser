//! Monitor Pattern — Background Task Scheduler (Phase 6, updated Phase 8D)
//!
//! Replaces the ad-hoc background services (arc_status_poller, cache_sync, utxo_sync)
//! with a structured set of named tasks running on configurable intervals.
//!
//! Phase 8D additions:
//! - Graceful shutdown via CancellationToken (Ctrl+C stops the loop cleanly)
//! - DB lock contention avoidance: try_lock() before each task — if the DB is
//!   currently held by a user HTTP request, the task is skipped for this tick
//!
//! Tasks:
//! - TaskCheckForProofs: Acquire merkle proofs for unproven transactions
//! - TaskSendWaiting: Crash recovery for orphaned 'sending' transactions
//! - TaskFailAbandoned: Fail stuck unprocessed/unsigned transactions
//! - TaskUnFail: Recover false failures by re-checking on-chain
//! - TaskReviewStatus: Ensure consistency across proven_tx_reqs → transactions → outputs
//! - TaskPurge: Cleanup old monitor_events and completed proof requests
//! - TaskSyncPending: Periodic UTXO sync for pending addresses

pub mod task_check_for_proofs;
pub mod task_send_waiting;
pub mod task_fail_abandoned;
pub mod task_unfail;
pub mod task_review_status;
pub mod task_purge;
pub mod task_sync_pending;
pub mod task_check_peerpay;
pub mod task_backup;
pub mod task_replay_overlay;
pub mod task_consolidate_dust;
pub mod task_verify_double_spend;

use actix_web::web;
use log::{info, warn, error, debug};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::AppState;

/// Prevents duplicate Monitor loops (e.g., wallet_create + wallet_recover race)
static MONITOR_STARTED: AtomicBool = AtomicBool::new(false);

/// Task interval configuration (in seconds)
struct TaskSchedule {
    check_for_proofs: u64,
    send_waiting: u64,
    fail_abandoned: u64,
    unfail: u64,
    review_status: u64,
    purge: u64,
    sync_pending: u64,
    check_peerpay: u64,
    backup: u64,
    replay_overlay: u64,
    consolidate_dust: u64,
    verify_double_spend: u64,
}

impl Default for TaskSchedule {
    fn default() -> Self {
        Self {
            check_for_proofs: 60,     // 1 minute
            send_waiting: 120,        // 2 minutes
            fail_abandoned: 300,      // 5 minutes
            unfail: 300,              // 5 minutes
            review_status: 60,        // 1 minute
            purge: 3600,              // 1 hour
            sync_pending: 30,         // 30 seconds
            check_peerpay: 60,        // 1 minute
            backup: 10800,            // 3 hours (180 minutes) — significant events trigger sooner via backup_check_needed flag
            replay_overlay: 300,      // 5 minutes — retry overlay notification for unpublished certs
            consolidate_dust: 86400,  // 24 hours — daily dust consolidation check
            verify_double_spend: 60,  // 1 minute — fast verification for suspected double-spends
        }
    }
}

/// Monitor — the main background task scheduler
pub struct Monitor {
    state: web::Data<AppState>,
    client: reqwest::Client,
    schedule: TaskSchedule,
}

impl Monitor {
    pub fn new(state: web::Data<AppState>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            state,
            client,
            schedule: TaskSchedule::default(),
        }
    }

    /// Start the monitor as a background tokio task.
    ///
    /// Uses an AtomicBool to prevent duplicate Monitor loops.
    /// Safe to call multiple times — second call is a no-op.
    pub fn start(state: web::Data<AppState>) {
        if MONITOR_STARTED.swap(true, Ordering::SeqCst) {
            warn!("Monitor::start() called but Monitor is already running — skipping");
            return;
        }
        let monitor = Self::new(state);
        tokio::spawn(async move {
            monitor.run().await;
        });
    }

    /// Check if the database is currently available (not held by a user request).
    /// Returns true if we can proceed with background tasks.
    fn db_available(&self) -> bool {
        match self.state.database.try_lock() {
            Ok(_guard) => {
                // Lock acquired and immediately dropped — DB is free
                true
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                // DB is held by a user request — skip this tick
                debug!("Monitor: DB locked by user request, skipping tick");
                false
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                error!("Monitor: DB mutex poisoned!");
                false
            }
        }
    }

    /// Main run loop — ticks every 30 seconds, runs tasks that are due
    async fn run(&self) {
        info!("🔄 Monitor started with 12 tasks (graceful shutdown enabled)");
        info!("   TaskCheckForProofs: every {}s", self.schedule.check_for_proofs);
        info!("   TaskSendWaiting: every {}s", self.schedule.send_waiting);
        info!("   TaskFailAbandoned: every {}s", self.schedule.fail_abandoned);
        info!("   TaskUnFail: every {}s", self.schedule.unfail);
        info!("   TaskReviewStatus: every {}s", self.schedule.review_status);
        info!("   TaskPurge: every {}s", self.schedule.purge);
        info!("   TaskSyncPending: every {}s (tiered: 30s fresh, 3m recent, 30m old)", self.schedule.sync_pending);
        info!("   TaskCheckPeerPay: every {}s", self.schedule.check_peerpay);
        info!("   TaskBackup: every {}s (if dirty)", self.schedule.backup);
        info!("   TaskReplayOverlay: every {}s (pending overlay certs)", self.schedule.replay_overlay);
        info!("   TaskConsolidateDust: every {}s (dust UTXO sweep)", self.schedule.consolidate_dust);
        info!("   TaskVerifyDoubleSpend: every {}s (independent DS verification)", self.schedule.verify_double_spend);

        let tick_interval = Duration::from_secs(30);
        let mut last_check_for_proofs: u64 = 0;
        let mut last_send_waiting: u64 = 0;
        let mut last_fail_abandoned: u64 = 0;
        let mut last_unfail: u64 = 0;
        let mut last_review_status: u64 = 0;
        let mut last_purge: u64 = 0;
        let mut last_sync_pending: u64 = 0;
        let mut last_check_peerpay: u64 = 0;
        let mut last_backup: u64 = Self::now_secs(); // Don't run on first tick — wait for interval
        let mut last_replay_overlay: u64 = 0; // 0 = check on first tick
        let mut last_consolidate_dust: u64 = Self::now_secs(); // Don't run on first tick — daily task
        let mut last_verify_double_spend: u64 = 0; // Check on first tick

        // Small initial delay to let the server finish starting up
        tokio::time::sleep(Duration::from_secs(5)).await;

        loop {
            // Check for shutdown signal (Phase 8D)
            tokio::select! {
                _ = self.state.shutdown.cancelled() => {
                    info!("🛑 Monitor shutting down gracefully");
                    break;
                }
                _ = tokio::time::sleep(tick_interval) => {
                    // Continue to task scheduling
                }
            }

            // Check if DB is available before running any tasks.
            // If a user HTTP request currently holds the lock, skip this entire tick
            // to avoid blocking the user. Tasks will run on the next tick instead.
            if !self.db_available() {
                continue;
            }

            let now = Self::now_secs();

            // Post-recovery: force immediate validation of restored data
            let recovery_mode = self.state.recovery_just_completed.swap(false, std::sync::atomic::Ordering::SeqCst);
            if recovery_mode {
                info!("🔄 Post-recovery: running immediate TaskCheckForProofs");
                if let Err(e) = task_check_for_proofs::run(&self.state, &self.client).await {
                    error!("   ❌ Post-recovery TaskCheckForProofs failed: {}", e);
                }
                last_check_for_proofs = now;
            }

            // TaskCheckForProofs
            if now - last_check_for_proofs >= self.schedule.check_for_proofs {
                last_check_for_proofs = now;
                if let Err(e) = task_check_for_proofs::run(&self.state, &self.client).await {
                    error!("   ❌ TaskCheckForProofs failed: {}", e);
                    self.log_event("TaskCheckForProofs:error", Some(&e));
                }
            }

            // TaskSendWaiting
            if now - last_send_waiting >= self.schedule.send_waiting {
                last_send_waiting = now;
                if let Err(e) = task_send_waiting::run(&self.state, &self.client).await {
                    error!("   ❌ TaskSendWaiting failed: {}", e);
                    self.log_event("TaskSendWaiting:error", Some(&e));
                }
            }

            // TaskFailAbandoned
            if now - last_fail_abandoned >= self.schedule.fail_abandoned {
                last_fail_abandoned = now;
                if let Err(e) = task_fail_abandoned::run(&self.state).await {
                    error!("   ❌ TaskFailAbandoned failed: {}", e);
                    self.log_event("TaskFailAbandoned:error", Some(&e));
                }
            }

            // TaskUnFail
            if now - last_unfail >= self.schedule.unfail {
                last_unfail = now;
                if let Err(e) = task_unfail::run(&self.state, &self.client).await {
                    error!("   ❌ TaskUnFail failed: {}", e);
                    self.log_event("TaskUnFail:error", Some(&e));
                }
            }

            // TaskReviewStatus
            if now - last_review_status >= self.schedule.review_status {
                last_review_status = now;
                if let Err(e) = task_review_status::run(&self.state).await {
                    error!("   ❌ TaskReviewStatus failed: {}", e);
                    self.log_event("TaskReviewStatus:error", Some(&e));
                }
            }

            // TaskPurge
            if now - last_purge >= self.schedule.purge {
                last_purge = now;
                if let Err(e) = task_purge::run(&self.state).await {
                    error!("   ❌ TaskPurge failed: {}", e);
                    self.log_event("TaskPurge:error", Some(&e));
                }
            }

            // TaskSyncPending
            if now - last_sync_pending >= self.schedule.sync_pending {
                last_sync_pending = now;
                if let Err(e) = task_sync_pending::run(&self.state).await {
                    error!("   ❌ TaskSyncPending failed: {}", e);
                    self.log_event("TaskSyncPending:error", Some(&e));
                }
            }

            // TaskCheckPeerPay
            if now - last_check_peerpay >= self.schedule.check_peerpay {
                last_check_peerpay = now;
                if let Err(e) = task_check_peerpay::run(&self.state, &self.client).await {
                    error!("   ❌ TaskCheckPeerPay failed: {}", e);
                    self.log_event("TaskCheckPeerPay:error", Some(&e));
                }
            }

            // TaskBackup — runs on periodic schedule (3 hours) OR when significant event flag is set (after 3-min delay)
            let backup_triggered_by_event = {
                let guard = self.state.backup_check_needed.lock().ok();
                guard.and_then(|g| *g).map(|(_first_ts, latest_ts)| now as i64 - latest_ts >= 180).unwrap_or(false)  // 3-min delay from latest event
            };
            if now - last_backup >= self.schedule.backup || backup_triggered_by_event {
                let outcome = task_backup::run(&self.state).await;
                // Only clear the "soon" flag if backup state is current (Broadcast or Skipped).
                // Deferred/Failed keeps the flag so we retry on next tick.
                if outcome.is_current() {
                    last_backup = now;
                    if let Ok(mut guard) = self.state.backup_check_needed.lock() {
                        *guard = None;
                    }
                }
                match &outcome {
                    task_backup::BackupOutcome::Failed(e) => {
                        error!("   ❌ TaskBackup failed: {}", e);
                        self.log_event("TaskBackup:error", Some(e));
                    }
                    task_backup::BackupOutcome::Deferred(reason) => {
                        info!("   ⏳ TaskBackup deferred: {}", reason);
                    }
                    _ => {}
                }
            }

            // TaskReplayOverlay — retry overlay notification for certs stuck in 'unpublished_pending_overlay'
            if now - last_replay_overlay >= self.schedule.replay_overlay {
                last_replay_overlay = now;
                if let Err(e) = task_replay_overlay::run(&self.state, &self.client).await {
                    warn!("   ⚠️ TaskReplayOverlay failed: {}", e);
                    self.log_event("TaskReplayOverlay:error", Some(&e));
                }
            }

            // TaskConsolidateDust — daily sweep of dust UTXOs
            if now - last_consolidate_dust >= self.schedule.consolidate_dust {
                last_consolidate_dust = now;
                if let Err(e) = task_consolidate_dust::run(&self.state).await {
                    warn!("   ⚠️ TaskConsolidateDust failed: {}", e);
                    self.log_event("TaskConsolidateDust:error", Some(&e));
                }
            }

            // TaskVerifyDoubleSpend — independent verification of suspected double-spends
            if now - last_verify_double_spend >= self.schedule.verify_double_spend {
                last_verify_double_spend = now;
                if let Err(e) = task_verify_double_spend::run(&self.state, &self.client).await {
                    error!("   ❌ TaskVerifyDoubleSpend failed: {}", e);
                    self.log_event("TaskVerifyDoubleSpend:error", Some(&e));
                }
            }

        }

        info!("🛑 Monitor stopped");
    }

    /// Log an event to the monitor_events table
    fn log_event(&self, event: &str, details: Option<&str>) {
        let now = Self::now_secs() as i64;
        if let Ok(db) = self.state.database.try_lock() {
            let _ = db.connection().execute(
                "INSERT INTO monitor_events (event, details, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![event, details, now, now],
            );
        }
        // If lock is busy, silently skip logging — not critical
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Helper to log a monitor event from any task
pub fn log_monitor_event(state: &web::Data<AppState>, event: &str, details: Option<&str>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    if let Ok(db) = state.database.try_lock() {
        let _ = db.connection().execute(
            "INSERT INTO monitor_events (event, details, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![event, details, now, now],
        );
    }
    // If lock is busy, silently skip logging — not critical
}
