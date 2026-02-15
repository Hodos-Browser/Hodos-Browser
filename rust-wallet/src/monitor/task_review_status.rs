//! TaskReviewStatus — Ensure consistency across proven_tx_reqs → transactions → outputs
//!
//! Propagates proof completion status to transactions and ensures output spendable
//! flags are consistent with transaction status.
//!
//! Ghost Transaction Safety:
//! - This task ONLY updates spendable flags and FKs on existing outputs
//! - It NEVER creates or deletes output rows
//!
//! Interval: 60 seconds

use actix_web::web;
use log::{info, warn};

use crate::AppState;
use crate::database::{TransactionRepository, ProvenTxRepository, ProvenTxReqRepository, OutputRepository};

/// Run the TaskReviewStatus task
pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
    let conn = db.connection();

    let mut proofs_reconciled = 0u32;
    let mut outputs_fixed = 0u32;
    let mut cleanups_verified = 0u32;

    // 1. proven_tx_reqs → transactions: Find completed but un-notified proof requests
    {
        let req_repo = ProvenTxReqRepository::new(conn);
        let tx_repo = TransactionRepository::new(conn);
        let proven_tx_repo = ProvenTxRepository::new(conn);

        let mut stmt = conn.prepare(
            "SELECT provenTxReqId, txid, proven_tx_id FROM proven_tx_reqs
             WHERE status = 'completed' AND notified = 0"
        ).map_err(|e| format!("SQL prepare: {}", e))?;

        let unnotified: Vec<(i64, String, Option<i64>)> = stmt.query_map(
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).map_err(|e| format!("SQL query: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

        for (req_id, txid, proven_tx_id) in &unnotified {
            // Ensure transaction is marked completed
            if let Err(e) = tx_repo.update_broadcast_status(&txid, "confirmed") {
                warn!("   ⚠️ Failed to update tx {} status: {}", txid, e);
                continue;
            }

            // Link proven_tx if available
            if let Some(ptx_id) = proven_tx_id {
                let _ = proven_tx_repo.link_transaction(&txid, *ptx_id);
            }

            // Mark as notified
            let _ = conn.execute(
                "UPDATE proven_tx_reqs SET notified = 1, updated_at = strftime('%s', 'now') WHERE provenTxReqId = ?1",
                rusqlite::params![req_id],
            );

            proofs_reconciled += 1;
        }
    }

    // 2. transactions → outputs: Ensure completed tx outputs are spendable
    {
        // Find outputs that belong to completed transactions but aren't spendable
        // (and aren't already spent by another transaction).
        // IMPORTANT: Exclude outputs marked 'external-spend' by wallet sync reconciliation —
        // those were confirmed spent on-chain by a tx the wallet doesn't know about.
        let mut stmt = conn.prepare(
            "SELECT o.outputId, o.txid, t.id
             FROM outputs o
             INNER JOIN transactions t ON o.transaction_id = t.id
             WHERE t.status = 'completed'
             AND o.spendable = 0
             AND o.spent_by IS NULL
             AND (o.spending_description IS NULL OR o.spending_description != 'external-spend')"
        ).map_err(|e| format!("SQL prepare: {}", e))?;

        let needs_fix: Vec<(i64, String, i64)> = stmt.query_map(
            [],
            |row| Ok((row.get(0)?, row.get::<_, String>(1)?, row.get(2)?)),
        ).map_err(|e| format!("SQL query: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

        for (output_id, txid, _tx_id) in &needs_fix {
            if let Err(e) = conn.execute(
                "UPDATE outputs SET spendable = 1, updated_at = strftime('%s', 'now') WHERE outputId = ?1",
                rusqlite::params![output_id],
            ) {
                warn!("   ⚠️ Failed to fix output {} spendable flag: {}", output_id, e);
            } else {
                info!("   🔧 Fixed output {} (tx {}) → spendable", output_id, &txid[..txid.len().min(16)]);
                outputs_fixed += 1;
            }
        }
    }

    // 3. Failed tx cleanup verification: Ensure old failed txs have been cleaned up
    {
        let unfail_cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64 - 1800; // 30 minutes

        // Find failed txs past the UnFail window that still have reserved outputs
        let mut stmt = conn.prepare(
            "SELECT t.id, t.txid FROM transactions t
             WHERE t.status = 'failed'
             AND t.failed_at IS NOT NULL
             AND t.failed_at < ?1
             AND EXISTS (
                SELECT 1 FROM outputs o
                WHERE o.spent_by = t.id AND o.spendable = 0
             )"
        ).map_err(|e| format!("SQL prepare: {}", e))?;

        let needs_cleanup: Vec<(i64, String)> = stmt.query_map(
            rusqlite::params![unfail_cutoff],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| format!("SQL query: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

        let output_repo = OutputRepository::new(conn);

        for (_tx_id, txid) in &needs_cleanup {
            let short_txid = &txid[..txid.len().min(16)];

            // Restore inputs that are still reserved by this failed tx
            match output_repo.restore_spent_by_txid(txid) {
                Ok(count) if count > 0 => {
                    info!("   🔧 Restored {} stale reserved input(s) from failed tx {}", count, short_txid);
                    cleanups_verified += count as u32;
                }
                _ => {
                    // Try fallback
                    if let Ok(count) = output_repo.restore_by_spending_description(txid) {
                        if count > 0 {
                            info!("   🔧 Restored {} stale reserved input(s) via spending_description from {}", count, short_txid);
                            cleanups_verified += count as u32;
                        }
                    }
                }
            }
        }
    }

    // Only log if we did something
    if proofs_reconciled > 0 || outputs_fixed > 0 || cleanups_verified > 0 {
        info!("✅ TaskReviewStatus: {} proofs reconciled, {} outputs fixed, {} stale reservations cleaned",
              proofs_reconciled, outputs_fixed, cleanups_verified);
        super::log_monitor_event(state, "TaskReviewStatus:completed",
            Some(&format!("{} proofs, {} outputs, {} cleanups", proofs_reconciled, outputs_fixed, cleanups_verified)));

        // Invalidate balance cache if anything changed
        state.balance_cache.invalidate();
    }

    Ok(())
}
