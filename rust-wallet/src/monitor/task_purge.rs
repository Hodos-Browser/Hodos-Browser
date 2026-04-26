//! TaskPurge — Cleanup old monitor_events, completed proof requests, and confirmed parent_transactions
//!
//! Removes old data that's no longer needed:
//! - monitor_events older than 7 days
//! - completed+notified proven_tx_reqs older than 30 days
//! - confirmed parent_transactions older than 7 days (re-fetchable from WoC API)
//!
//! Interval: 3600 seconds (1 hour)

use actix_web::web;
use log::info;

use crate::AppState;

/// Keep monitor_events for this many seconds (7 days)
const EVENTS_RETENTION_SECS: i64 = 7 * 24 * 60 * 60;

/// Keep completed proven_tx_reqs for this many seconds (30 days)
const PROOF_REQS_RETENTION_SECS: i64 = 30 * 24 * 60 * 60;

/// Keep confirmed parent_transactions for this many seconds (7 days)
/// Once a tx is confirmed (has proven_txs record), its parents aren't needed for BEEF building.
/// If ever needed again, re-fetched from WoC API (tier 3 in beef_helpers cache hierarchy).
const PARENT_TX_RETENTION_SECS: i64 = 7 * 24 * 60 * 60;

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

    // 3. Delete old confirmed parent_transactions (older than 7 days)
    // Confirmed txs have merkle proofs — their parents aren't needed for BEEF.
    // Only deletes records that are both old AND confirmed on chain (proven_txs exists).
    let parent_cutoff = now - PARENT_TX_RETENTION_SECS;
    match conn.execute(
        "DELETE FROM parent_transactions WHERE cached_at < ?1 AND txid IN (SELECT txid FROM proven_txs)",
        rusqlite::params![parent_cutoff],
    ) {
        Ok(count) if count > 0 => {
            info!("🧹 TaskPurge: deleted {} old confirmed parent_transaction(s)", count);
            total_purged += count;
        }
        Err(e) => {
            info!("   ℹ️ parent_transactions purge skipped: {}", e);
        }
        _ => {}
    }

    // 4. Delete expired peerpay_pending_verification records (older than 24 hours)
    // These are PeerPay messages that were parsed but never verified on-chain.
    // After 24h they're stale — either the message was acknowledged by other means
    // or the payment is genuinely invalid.
    const PENDING_VERIFICATION_RETENTION_SECS: i64 = 24 * 60 * 60;
    match crate::database::PeerPayRepository::cleanup_expired_pending(conn, PENDING_VERIFICATION_RETENTION_SECS) {
        Ok(count) if count > 0 => {
            info!("🧹 TaskPurge: deleted {} expired peerpay_pending_verification(s)", count);
            total_purged += count;
        }
        Err(e) => {
            info!("   ℹ️ peerpay_pending_verification purge skipped: {}", e);
        }
        _ => {}
    }

    if total_purged > 0 {
        super::log_monitor_event(state, "TaskPurge:completed", Some(&format!("{} records purged", total_purged)));
    }

    Ok(())
}
