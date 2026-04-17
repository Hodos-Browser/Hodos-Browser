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
                    "SEEN_ON_NETWORK" | "STORED" | "ANNOUNCED_TO_NETWORK"
                    | "QUEUED" | "RECEIVED" => {
                        info!("   ✅ {} already in mempool ({}) — promoting to unproven", short_txid, status);
                        promote_to_unproven(state, &txid);
                        recovered += 1;
                        continue;
                    }
                    "SEEN_IN_ORPHAN_MEMPOOL" | "MINED_IN_STALE_BLOCK" => {
                        // Orphan = BEEF validation failure, stale = block orphaned.
                        // Inputs NOT spent on-chain — clean up for re-broadcast.
                        warn!("   ⚠️ {} status {} — cleaning up for re-broadcast", short_txid, status);
                        cleanup_failed_sending(state, &txid, *tx_id);
                        failed += 1;
                        continue;
                    }
                    "REJECTED" | "DOUBLE_SPEND_ATTEMPTED" => {
                        info!("   ❌ {} rejected by ARC ({}) — marking failed", short_txid, status);
                        if status == "DOUBLE_SPEND_ATTEMPTED" {
                            cleanup_failed_sending_double_spend(state, &txid, *tx_id);
                        } else {
                            cleanup_failed_sending(state, &txid, *tx_id);
                        }
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

        // Step 3: Re-broadcast — prefer BEEF if available (stored raw_tx may be BEEF from createAction,
        // or input_beef may contain the original BEEF from the transaction build).
        let broadcast_hex = get_beef_for_rebroadcast(state, &txid, &raw_tx);
        if broadcast_hex.starts_with("0100beef") || broadcast_hex.starts_with("0200beef") || broadcast_hex.starts_with("01010101") {
            info!("   📡 Re-broadcasting {} with BEEF ({} hex chars, stuck for {}s)...", short_txid, broadcast_hex.len(), age_secs);
        } else {
            info!("   📡 Re-broadcasting {} with raw tx (stuck for {}s)...", short_txid, age_secs);
        }
        match crate::handlers::broadcast_transaction(
            &broadcast_hex,
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
                    if crate::arc_status::is_double_spend_error(&e) {
                        cleanup_failed_sending_double_spend(state, &txid, *tx_id);
                    } else {
                        cleanup_failed_sending(state, &txid, *tx_id);
                    }
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
    cleanup_failed_sending_impl(state, txid, tx_id, false);
}

/// Cleanup variant that knows inputs are double-spent (spent on-chain).
/// When `is_double_spend` is true, marks inputs as externally spent instead
/// of restoring them as spendable — prevents the wallet from re-selecting
/// the same dead UTXOs.
fn cleanup_failed_sending_double_spend(state: &web::Data<AppState>, txid: &str, tx_id: i64) {
    cleanup_failed_sending_impl(state, txid, tx_id, true);
}

fn cleanup_failed_sending_impl(state: &web::Data<AppState>, txid: &str, tx_id: i64, is_double_spend: bool) {
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

        // 2. Disable (not delete) change outputs — TaskUnFail can re-enable if tx was actually mined
        match output_repo.disable_by_txid(txid) {
            Ok(count) if count > 0 => {
                info!("   🚫 Disabled {} output(s) from failed tx {} (recoverable by TaskUnFail)", count, short_txid);
            }
            Err(e) => {
                warn!("   ⚠️ Failed to disable outputs for {}: {}", short_txid, e);
            }
            _ => {}
        }

        // 3. Handle inputs based on whether this is a double-spend
        if is_double_spend {
            // Inputs ARE spent on-chain — mark as externally spent, do NOT restore.
            let marked = conn.execute(
                "UPDATE outputs SET spending_description = 'double-spend-detected'
                 WHERE spending_description = ?1 AND spendable = 0",
                rusqlite::params![txid],
            ).unwrap_or(0);
            if marked > 0 {
                warn!("   ⚠️ Double-spend: marked {} input(s) as externally spent for {}", marked, short_txid);
            }
        } else {
            // Normal failure — restore inputs as spendable
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
    }

    // 4. Invalidate balance cache
    state.balance_cache.invalidate();
}

/// Determine if a broadcast error is permanent (never retry) vs transient (worth retrying).
/// Delegates to the centralized arc_status module.
fn is_permanent_error(error: &str) -> bool {
    crate::arc_status::is_fatal_broadcast_error(error)
}

/// Get the best available hex for re-broadcasting a stuck transaction.
/// Checks (in order):
/// 1. If the stored raw_tx is already BEEF format (createAction stores BEEF)
/// 2. If there's an input_beef blob stored from the original transaction build
/// 3. Falls back to the raw tx hex
fn get_beef_for_rebroadcast(
    state: &web::Data<AppState>,
    txid: &str,
    raw_tx_hex: &str,
) -> String {
    // Check if raw_tx is already BEEF (createAction stores Atomic BEEF / BEEF V1/V2)
    if raw_tx_hex.starts_with("0100beef") || raw_tx_hex.starts_with("0200beef") || raw_tx_hex.starts_with("01010101") {
        return raw_tx_hex.to_string();
    }

    // Check if there's a stored input_beef from the original build
    if let Ok(db) = state.database.lock() {
        let conn = db.connection();
        if let Ok(blob) = conn.query_row(
            "SELECT input_beef FROM transactions WHERE txid = ?1 AND input_beef IS NOT NULL",
            rusqlite::params![txid],
            |row| row.get::<_, Vec<u8>>(0),
        ) {
            if !blob.is_empty() {
                let beef_hex = hex::encode(&blob);
                if beef_hex.starts_with("0100beef") || beef_hex.starts_with("0200beef") {
                    info!("   📦 Using stored input_beef for re-broadcast of {}", &txid[..txid.len().min(16)]);
                    return beef_hex;
                }
            }
        }
    }

    // Fallback to raw tx
    raw_tx_hex.to_string()
}
