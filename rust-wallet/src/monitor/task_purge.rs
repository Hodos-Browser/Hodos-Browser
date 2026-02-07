//! TaskPurge — Cleanup old monitor_events and completed proof requests
//!
//! Removes old data that's no longer needed:
//! - monitor_events older than 7 days
//! - completed+notified proven_tx_reqs older than 30 days
//!
//! Interval: 3600 seconds (1 hour)

use actix_web::web;
use log::info;

use crate::AppState;

/// Keep monitor_events for this many seconds (7 days)
const EVENTS_RETENTION_SECS: i64 = 7 * 24 * 60 * 60;

/// Keep completed proven_tx_reqs for this many seconds (30 days)
const PROOF_REQS_RETENTION_SECS: i64 = 30 * 24 * 60 * 60;

/// Run the TaskPurge task
pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
    let conn = db.connection();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut total_purged = 0;

    // 1. Delete old monitor_events (older than 7 days)
    let events_cutoff = now - EVENTS_RETENTION_SECS;
    match conn.execute(
        "DELETE FROM monitor_events WHERE created_at < ?1",
        rusqlite::params![events_cutoff],
    ) {
        Ok(count) if count > 0 => {
            info!("🧹 TaskPurge: deleted {} old monitor event(s)", count);
            total_purged += count;
        }
        Err(e) => {
            // Table might not exist yet if this is a fresh install
            info!("   ℹ️ monitor_events purge skipped: {}", e);
        }
        _ => {}
    }

    // 2. Delete old completed+notified proven_tx_reqs (older than 30 days)
    // The proven_txs record (immutable proof) is kept permanently —
    // we only delete the mutable request tracking record.
    let reqs_cutoff = now - PROOF_REQS_RETENTION_SECS;
    match conn.execute(
        "DELETE FROM proven_tx_reqs WHERE status = 'completed' AND notified = 1 AND updated_at < ?1",
        rusqlite::params![reqs_cutoff],
    ) {
        Ok(count) if count > 0 => {
            info!("🧹 TaskPurge: deleted {} old proven_tx_req(s)", count);
            total_purged += count;
        }
        Err(e) => {
            info!("   ℹ️ proven_tx_reqs purge skipped: {}", e);
        }
        _ => {}
    }

    if total_purged > 0 {
        super::log_monitor_event(state, "TaskPurge:completed", Some(&format!("{} records purged", total_purged)));
    }

    Ok(())
}
