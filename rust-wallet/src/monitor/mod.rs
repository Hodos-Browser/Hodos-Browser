//! Monitor Pattern — Background Task Scheduler (Phase 6)
//!
//! Replaces the ad-hoc background services (arc_status_poller, cache_sync, utxo_sync)
//! with a structured set of named tasks running on configurable intervals.
//!
//! Tasks:
//! - TaskCheckForProofs: Acquire merkle proofs for unproven transactions
//! - TaskSendWaiting: Crash recovery for orphaned 'sending' transactions
//! - TaskFailAbandoned: Fail stuck unprocessed/unsigned transactions
//! - TaskUnFail: Recover false failures by re-checking on-chain
//! - TaskReviewStatus: Ensure consistency across proven_tx_reqs → transactions → outputs
//! - TaskPurge: Cleanup old monitor_events and completed proof requests

pub mod task_check_for_proofs;
pub mod task_send_waiting;
pub mod task_fail_abandoned;
pub mod task_unfail;
pub mod task_review_status;
pub mod task_purge;

use actix_web::web;
use log::{info, warn, error};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::AppState;

/// Task interval configuration (in seconds)
struct TaskSchedule {
    check_for_proofs: u64,
    send_waiting: u64,
    fail_abandoned: u64,
    unfail: u64,
    review_status: u64,
    purge: u64,
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

    /// Start the monitor as a background tokio task
    pub fn start(state: web::Data<AppState>) {
        let monitor = Self::new(state);
        tokio::spawn(async move {
            monitor.run().await;
        });
    }

    /// Main run loop — ticks every 30 seconds, runs tasks that are due
    async fn run(&self) {
        info!("🔄 Monitor started with 6 tasks");
        info!("   TaskCheckForProofs: every {}s", self.schedule.check_for_proofs);
        info!("   TaskSendWaiting: every {}s", self.schedule.send_waiting);
        info!("   TaskFailAbandoned: every {}s", self.schedule.fail_abandoned);
        info!("   TaskUnFail: every {}s", self.schedule.unfail);
        info!("   TaskReviewStatus: every {}s", self.schedule.review_status);
        info!("   TaskPurge: every {}s", self.schedule.purge);

        let tick_interval = Duration::from_secs(30);
        let mut last_check_for_proofs: u64 = 0;
        let mut last_send_waiting: u64 = 0;
        let mut last_fail_abandoned: u64 = 0;
        let mut last_unfail: u64 = 0;
        let mut last_review_status: u64 = 0;
        let mut last_purge: u64 = 0;

        // Small initial delay to let the server finish starting up
        tokio::time::sleep(Duration::from_secs(5)).await;

        loop {
            let now = Self::now_secs();

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

            tokio::time::sleep(tick_interval).await;
        }
    }

    /// Log an event to the monitor_events table
    fn log_event(&self, event: &str, details: Option<&str>) {
        let now = Self::now_secs() as i64;
        if let Ok(db) = self.state.database.lock() {
            let _ = db.connection().execute(
                "INSERT INTO monitor_events (event, details, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![event, details, now, now],
            );
        }
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
    if let Ok(db) = state.database.lock() {
        let _ = db.connection().execute(
            "INSERT INTO monitor_events (event, details, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![event, details, now, now],
        );
    }
}
