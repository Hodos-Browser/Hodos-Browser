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

/// Run the TaskFailAbandoned task
pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    let results = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let conn = db.connection();
        let tx_repo = TransactionRepository::new(conn);
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

            // 2. Delete ghost change outputs for this transaction
            match output_repo.delete_by_txid(txid) {
                Ok(count) if count > 0 => {
                    info!("   🗑️ Deleted {} ghost output(s) from tx {}", count, short_txid);
                    total_outputs_deleted += count;
                }
                Err(e) => {
                    warn!("   ⚠️ Failed to delete ghost outputs for {}: {}", short_txid, e);
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

        // 4. Invalidate balance cache
        state.balance_cache.invalidate();

        (abandoned.len(), total_outputs_deleted, total_inputs_restored)
    };

    let (count, outputs_deleted, inputs_restored) = results;
    info!("✅ TaskFailAbandoned: failed {} tx(s), deleted {} ghost output(s), restored {} input(s)",
          count, outputs_deleted, inputs_restored);
    super::log_monitor_event(state, "TaskFailAbandoned:completed",
        Some(&format!("{} failed, {} outputs deleted, {} inputs restored", count, outputs_deleted, inputs_restored)));

    Ok(())
}
