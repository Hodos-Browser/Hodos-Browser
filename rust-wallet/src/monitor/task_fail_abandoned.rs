//! TaskFailAbandoned — Fail stuck unprocessed/unsigned transactions
//!
//! Transactions that were created but never completed signing/broadcasting
//! after 5 minutes are marked as failed, and their reserved outputs are restored.
//!
//! Ghost Transaction Safety:
//! - Uses same cleanup sequence as broadcast failure handler
//! - Order: mark failed → delete ghost outputs → restore inputs → invalidate cache
//!
//! Interval: 300 seconds (5 minutes)

use actix_web::web;
use log::{info, warn};

use crate::AppState;
use crate::database::{TransactionRepository, OutputRepository};

/// Transactions older than this (in seconds) are considered abandoned
const ABANDON_THRESHOLD_SECS: i64 = 300;

/// Backup transactions in 'sending' older than this are considered stuck.
/// Normal sends stay in 'sending' longer (TaskSendWaiting handles at 30min),
/// but backup broadcasts should complete in <60s.
const BACKUP_SENDING_THRESHOLD_SECS: i64 = 600;

/// Run the TaskFailAbandoned task
pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    let results = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let conn = db.connection();
        let _tx_repo = TransactionRepository::new(conn);
        let output_repo = OutputRepository::new(conn);

        // Find transactions stuck in unprocessed/unsigned for more than 5 minutes
        let mut stmt = conn.prepare(
            "SELECT id, txid FROM transactions
             WHERE status IN ('unprocessed', 'unsigned')
             AND (strftime('%s', 'now') - created_at) > ?1"
        ).map_err(|e| format!("SQL prepare: {}", e))?;

        let abandoned: Vec<(i64, String)> = stmt.query_map(
            rusqlite::params![ABANDON_THRESHOLD_SECS],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| format!("SQL query: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

        if abandoned.is_empty() {
            return Ok(());
        }

        info!("🧹 TaskFailAbandoned: found {} abandoned transaction(s)", abandoned.len());

        let mut total_outputs_deleted = 0;
        let mut total_inputs_restored = 0;

        for (tx_id, txid) in &abandoned {
            let short_txid = &txid[..txid.len().min(16)];

            // 1. Mark as failed
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            if let Err(e) = conn.execute(
                "UPDATE transactions SET status = 'failed', failed_at = ?1 WHERE id = ?2",
                rusqlite::params![now, tx_id],
            ) {
                warn!("   ⚠️ Failed to mark tx {} as failed: {}", short_txid, e);
                continue;
            }

            // 2. Disable (not delete) change outputs — TaskUnFail can re-enable if tx was actually mined
            match output_repo.disable_by_txid(txid) {
                Ok(count) if count > 0 => {
                    info!("   🚫 Disabled {} output(s) from tx {} (recoverable by TaskUnFail)", count, short_txid);
                    total_outputs_deleted += count;
                }
                Err(e) => {
                    warn!("   ⚠️ Failed to disable outputs for {}: {}", short_txid, e);
                }
                _ => {}
            }

            // 3. Restore input outputs that were reserved for this tx
            match output_repo.restore_spent_by_txid(txid) {
                Ok(count) if count > 0 => {
                    info!("   ♻️ Restored {} input(s) from tx {}", count, short_txid);
                    total_inputs_restored += count;
                }
                Err(e) => {
                    warn!("   ⚠️ Failed to restore inputs for {}: {}", short_txid, e);
                }
                _ => {
                    // Also try restore by spending_description (fallback for placeholder reservations)
                    if let Ok(count) = output_repo.restore_by_spending_description(txid) {
                        if count > 0 {
                            info!("   ♻️ Restored {} input(s) via spending_description from tx {}", count, short_txid);
                            total_inputs_restored += count;
                        }
                    }
                }
            }

            info!("   ✅ Abandoned tx {} marked as failed", short_txid);
        }

        // Second sweep: backup transactions stuck in 'sending' for >10 minutes.
        // These should complete in <60s — if still 'sending' after 10min, the
        // broadcast was lost (process killed, network timeout, etc.).
        let mut backup_stmt = conn.prepare(
            "SELECT id, txid FROM transactions
             WHERE status = 'sending'
             AND description = 'On-chain wallet backup'
             AND (strftime('%s', 'now') - created_at) > ?1"
        ).map_err(|e| format!("SQL prepare backup: {}", e))?;

        let stuck_backups: Vec<(i64, String)> = backup_stmt.query_map(
            rusqlite::params![BACKUP_SENDING_THRESHOLD_SECS],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| format!("SQL query backup: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

        for (tx_id, txid) in &stuck_backups {
            let short_txid = &txid[..txid.len().min(16)];
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            if let Err(e) = conn.execute(
                "UPDATE transactions SET status = 'failed', failed_at = ?1 WHERE id = ?2",
                rusqlite::params![now, tx_id],
            ) {
                warn!("   ⚠️ Failed to mark stuck backup {} as failed: {}", short_txid, e);
                continue;
            }

            match output_repo.disable_by_txid(txid) {
                Ok(count) if count > 0 => {
                    info!("   🚫 Disabled {} output(s) from stuck backup {} (recoverable by TaskUnFail)", count, short_txid);
                    total_outputs_deleted += count;
                }
                Err(e) => warn!("   ⚠️ Failed to disable outputs for backup {}: {}", short_txid, e),
                _ => {}
            }

            match output_repo.restore_spent_by_txid(txid) {
                Ok(count) if count > 0 => {
                    info!("   ♻️ Restored {} input(s) from stuck backup {}", count, short_txid);
                    total_inputs_restored += count;
                }
                Err(e) => warn!("   ⚠️ Failed to restore inputs for backup {}: {}", short_txid, e),
                _ => {
                    if let Ok(count) = output_repo.restore_by_spending_description(txid) {
                        if count > 0 {
                            info!("   ♻️ Restored {} input(s) via spending_description from backup {}", count, short_txid);
                            total_inputs_restored += count;
                        }
                    }
                }
            }

            info!("   ✅ Stuck backup {} marked as failed", short_txid);
        }

        let total_count = abandoned.len() + stuck_backups.len();
        if stuck_backups.len() > 0 {
            info!("🧹 TaskFailAbandoned: also cleaned up {} stuck backup tx(s)", stuck_backups.len());
        }

        // 4. Invalidate balance cache
        if total_count > 0 {
            state.balance_cache.invalidate();
        }

        (total_count, total_outputs_deleted, total_inputs_restored)
    };

    let (count, outputs_deleted, inputs_restored) = results;
    if count > 0 {
        info!("✅ TaskFailAbandoned: failed {} tx(s), deleted {} ghost output(s), restored {} input(s)",
              count, outputs_deleted, inputs_restored);
        super::log_monitor_event(state, "TaskFailAbandoned:completed",
            Some(&format!("{} failed, {} outputs deleted, {} inputs restored", count, outputs_deleted, inputs_restored)));
    }

    Ok(())
}
