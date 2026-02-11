//! TaskSendWaiting — Crash recovery for orphaned 'sending' transactions
//!
//! Picks up transactions stuck in 'sending' status (due to app crash or network drop)
//! and attempts recovery:
//! 1. Check ARC if tx was actually accepted/mined (crash after successful broadcast)
//! 2. If not found, re-broadcast the raw transaction
//! 3. On success → update to 'unproven' (proof will be tracked by TaskCheckForProofs)
//! 4. On permanent failure → full cleanup (delete ghost outputs, restore inputs)
//!
//! Ghost Transaction Safety:
//! - Verifies raw_tx exists before re-broadcasting (no blind retries)
//! - On failure cleanup, uses same sequence as broadcast failure handler:
//!   mark failed → delete ghost outputs → restore inputs → invalidate cache
//! - Does NOT create new outputs; only cleans up or promotes existing state
//!
//! Interval: 120 seconds (2 minutes)

use actix_web::web;
use log::{info, warn};
use std::time::Duration;

use crate::AppState;
use crate::database::{TransactionRepository, OutputRepository, ProvenTxReqRepository};

/// Transactions must be stuck in 'sending' for at least this long before recovery (seconds)
const STUCK_THRESHOLD_SECS: i64 = 120;

/// If stuck in 'sending' for longer than this, give up and mark as failed (seconds)
/// TaskUnFail will check if it was actually mined and recover if needed.
const GIVE_UP_THRESHOLD_SECS: i64 = 1800; // 30 minutes

/// Run the TaskSendWaiting task
pub async fn run(state: &web::Data<AppState>, client: &reqwest::Client) -> Result<(), String> {
    // 1. Collect stuck 'sending' transactions (release DB lock before network calls)
    let stuck_txs: Vec<(i64, String, String, i64)> = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let conn = db.connection();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let stuck_cutoff = now - STUCK_THRESHOLD_SECS;

        let mut stmt = conn.prepare(
            "SELECT id, txid, raw_tx, created_at FROM transactions
             WHERE status = 'sending'
             AND created_at < ?1"
        ).map_err(|e| format!("SQL prepare: {}", e))?;

        let rows = stmt.query_map(
            rusqlite::params![stuck_cutoff],
            |row| Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            )),
        ).map_err(|e| format!("SQL query: {}", e))?
        .filter_map(|r| r.ok())
        .collect();
        rows
    };

    if stuck_txs.is_empty() {
        return Ok(());
    }

    info!("🔄 TaskSendWaiting: found {} transaction(s) stuck in 'sending'", stuck_txs.len());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut recovered = 0u32;
    let mut failed = 0u32;

    for (tx_id, txid, raw_tx, timestamp) in &stuck_txs {
        let short_txid = &txid[..txid.len().min(16)];
        let age_secs = now - timestamp;

        // Skip if raw_tx is empty — nothing to broadcast
        if raw_tx.is_empty() {
            warn!("   ⚠️ {} has no raw_tx — cannot recover, marking failed", short_txid);
            cleanup_failed_sending(state, &txid, *tx_id);
            failed += 1;
            continue;
        }

        // Step 1: Check ARC if tx was already accepted (crash after successful broadcast)
        match crate::handlers::query_arc_tx_status(client, &txid).await {
            Ok(arc_resp) => {
                let status = arc_resp.tx_status.as_deref().unwrap_or("");
                match status {
                    "MINED" => {
                        info!("   ✅ {} already MINED on ARC — promoting to completed", short_txid);
                        promote_to_unproven(state, &txid);
                        // TaskCheckForProofs will handle proof acquisition
                        recovered += 1;
                        continue;
                    }
                    "SEEN_ON_NETWORK" | "SEEN_IN_ORPHAN_MEMPOOL" | "STORED" | "ANNOUNCED_TO_NETWORK" => {
                        info!("   ✅ {} already in mempool ({}) — promoting to unproven", short_txid, status);
                        promote_to_unproven(state, &txid);
                        recovered += 1;
                        continue;
                    }
                    "REJECTED" | "DOUBLE_SPEND_ATTEMPTED" => {
                        info!("   ❌ {} rejected by ARC ({}) — marking failed", short_txid, status);
                        cleanup_failed_sending(state, &txid, *tx_id);
                        failed += 1;
                        continue;
                    }
                    _ => {
                        // Unknown or empty status — ARC doesn't know about this tx
                        info!("   🔍 {} not found on ARC (status: '{}') — will try re-broadcast", short_txid, status);
                    }
                }
            }
            Err(_) => {
                // ARC query failed — proceed to re-broadcast attempt
                info!("   🔍 ARC query failed for {} — will try re-broadcast", short_txid);
            }
        }

        // Step 2: Check if too old — give up instead of endlessly retrying
        if age_secs > GIVE_UP_THRESHOLD_SECS {
            warn!("   ⏰ {} stuck in 'sending' for {}s (> {}s) — giving up, marking failed",
                  short_txid, age_secs, GIVE_UP_THRESHOLD_SECS);
            cleanup_failed_sending(state, &txid, *tx_id);
            failed += 1;
            continue;
        }

        // Step 3: Re-broadcast
        info!("   📡 Re-broadcasting {} (stuck for {}s)...", short_txid, age_secs);
        match crate::handlers::broadcast_transaction(
            &raw_tx,
            Some(&state.database),
            Some(&txid),
        ).await {
            Ok(msg) => {
                info!("   ✅ Re-broadcast succeeded for {}: {}", short_txid, msg);
                promote_to_unproven(state, &txid);
                recovered += 1;
            }
            Err(e) => {
                if is_permanent_error(&e) {
                    warn!("   ❌ Permanent broadcast failure for {}: {}", short_txid, e);
                    cleanup_failed_sending(state, &txid, *tx_id);
                    failed += 1;
                } else {
                    // Transient error — leave as 'sending', will retry on next tick
                    info!("   ⏳ Transient broadcast failure for {} — will retry: {}", short_txid, e);
                }
            }
        }

        // Rate limiting between broadcasts
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    if recovered > 0 || failed > 0 {
        info!("✅ TaskSendWaiting: {} recovered, {} failed", recovered, failed);
        super::log_monitor_event(state, "TaskSendWaiting:completed",
            Some(&format!("{} recovered, {} failed", recovered, failed)));
    }

    Ok(())
}

/// Promote a 'sending' transaction to 'unproven' status.
/// Also ensures a proven_tx_req exists for proof tracking.
fn promote_to_unproven(state: &web::Data<AppState>, txid: &str) {
    if let Ok(db) = state.database.lock() {
        let conn = db.connection();
        let short_txid = &txid[..txid.len().min(16)];

        // Update status to unproven (broadcast_status = "broadcast" maps to status = "unproven")
        let tx_repo = TransactionRepository::new(conn);
        if let Err(e) = tx_repo.update_broadcast_status(txid, "broadcast") {
            warn!("   ⚠️ Failed to promote {} to unproven: {}", short_txid, e);
            return;
        }

        // Ensure proven_tx_req exists for proof tracking
        let req_repo = ProvenTxReqRepository::new(conn);
        if let Ok(None) = req_repo.get_by_txid(txid) {
            let raw_tx_bytes: Vec<u8> = match tx_repo.get_by_txid(txid) {
                Ok(Some(stored)) => hex::decode(&stored.raw_tx).unwrap_or_default(),
                _ => Vec::new(),
            };
            let _ = req_repo.create(txid, &raw_tx_bytes, None, "unproven");
        }
    }
}

/// Full cleanup for a permanently failed 'sending' transaction.
/// Same sequence as the broadcast failure handler in handlers.rs:
/// mark failed → delete ghost outputs → restore inputs → invalidate cache
fn cleanup_failed_sending(state: &web::Data<AppState>, txid: &str, tx_id: i64) {
    if let Ok(db) = state.database.lock() {
        let conn = db.connection();
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
            return;
        }

        let output_repo = OutputRepository::new(conn);

        // 2. Delete ghost change outputs created by this transaction
        match output_repo.delete_by_txid(txid) {
            Ok(count) if count > 0 => {
                info!("   🗑️ Deleted {} ghost output(s) from failed tx {}", count, short_txid);
            }
            Err(e) => {
                warn!("   ⚠️ Failed to delete ghost outputs for {}: {}", short_txid, e);
            }
            _ => {}
        }

        // 3. Restore input outputs that were reserved for this tx
        match output_repo.restore_spent_by_txid(txid) {
            Ok(count) if count > 0 => {
                info!("   ♻️ Restored {} input(s) from failed tx {}", count, short_txid);
            }
            Err(e) => {
                warn!("   ⚠️ Failed to restore inputs for {}: {}", short_txid, e);
            }
            _ => {
                // Fallback: try restore_by_spending_description
                if let Ok(count) = output_repo.restore_by_spending_description(txid) {
                    if count > 0 {
                        info!("   ♻️ Restored {} input(s) via spending_description from tx {}", count, short_txid);
                    }
                }
            }
        }
    }

    // 4. Invalidate balance cache
    state.balance_cache.invalidate();
}

/// Determine if a broadcast error is permanent (never retry) vs transient (worth retrying).
///
/// Permanent errors indicate the transaction itself is invalid:
/// - Script verification failures (ERROR: 16)
/// - Double-spend attempts
/// - Missing inputs (UTXOs already spent)
/// - Already-known txids (actually success — handled separately by ARC check)
///
/// Transient errors indicate network/server issues worth retrying:
/// - Timeouts, connection refused, HTTP 500/502/503
fn is_permanent_error(error: &str) -> bool {
    let lower = error.to_lowercase();

    // Script verification failures
    if lower.contains("error: 16") || lower.contains("mandatory-script-verify") {
        return true;
    }
    // Double-spend / conflicting transaction
    if lower.contains("double spend") || lower.contains("double-spend")
        || lower.contains("txn-mempool-conflict") {
        return true;
    }
    // Missing inputs (UTXOs already consumed)
    if lower.contains("missing inputs") || lower.contains("missingorspent") {
        return true;
    }
    // Transaction too large or dust outputs
    if lower.contains("tx-size") || lower.contains("dust") {
        return true;
    }
    // Non-standard / policy rejection
    if lower.contains("non-mandatory-script-verify") || lower.contains("scriptpubkey") {
        return true;
    }

    false
}
