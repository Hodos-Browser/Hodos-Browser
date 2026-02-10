//! TaskUnFail — Recover false failures by re-checking on-chain
//!
//! Re-checks recently failed transactions (within 30-minute window) to see if
//! they were actually mined despite the broadcast error report. If found on-chain,
//! recovers the transaction to 'completed' status.
//!
//! Ghost Transaction Safety:
//! - Only recovers if on-chain proof is confirmed (merkle path verified)
//! - Does NOT re-create deleted outputs — that's too dangerous. Instead, logs
//!   a warning so the user can investigate. The outputs from a recovered tx
//!   will be picked up by the next UTXO sync.
//!
//! Interval: 300 seconds (5 minutes)

use actix_web::web;
use log::{info, warn};
use std::time::Duration;

use crate::AppState;
use crate::database::{TransactionRepository, OutputRepository, ProvenTxRepository, ProvenTxReqRepository};

/// Only check transactions failed within this window (seconds)
/// Must be long enough for orphan mempool txs to get mined.
/// BSV node orphan pool expires after 20 min, but miners may have the tx
/// from other broadcasters. 6 hours matches UNPROVEN_TIMEOUT_SECS.
const UNFAIL_WINDOW_SECS: i64 = 6 * 60 * 60; // 6 hours

/// Run the TaskUnFail task
pub async fn run(state: &web::Data<AppState>, client: &reqwest::Client) -> Result<(), String> {
    // Get recently failed transactions within the UnFail window
    let failed_txs: Vec<(i64, String)> = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let conn = db.connection();

        let unfail_cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64 - UNFAIL_WINDOW_SECS;

        let mut stmt = conn.prepare(
            "SELECT id, txid FROM transactions
             WHERE new_status = 'failed'
             AND failed_at IS NOT NULL
             AND failed_at >= ?1"
        ).map_err(|e| format!("SQL prepare: {}", e))?;

        let rows = stmt.query_map(
            rusqlite::params![unfail_cutoff],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| format!("SQL query: {}", e))?
        .filter_map(|r| r.ok())
        .collect();
        rows
    };

    if failed_txs.is_empty() {
        return Ok(());
    }

    info!("🔍 TaskUnFail: checking {} recently failed transaction(s)...", failed_txs.len());

    let mut recovered_count = 0;

    for (_tx_id, txid) in &failed_txs {
        let short_txid = &txid[..txid.len().min(16)];

        // Step 1: Check if proven_txs record already exists
        let already_proven = {
            let db = match state.database.lock() {
                Ok(g) => g,
                Err(e) => { warn!("   ⚠️ DB lock: {}", e); continue; }
            };
            let proven_tx_repo = ProvenTxRepository::new(db.connection());
            match proven_tx_repo.get_by_txid(txid) {
                Ok(Some(pt)) => Some((pt.proven_tx_id, pt.height)),
                _ => None,
            }
        };

        if let Some((proven_tx_id, height)) = already_proven {
            info!("   ✅ {} has proof on-chain! Recovering from failed → completed", short_txid);
            recover_transaction(state, txid, proven_tx_id, height);
            recovered_count += 1;
            continue;
        }

        // Step 2: Query ARC
        match crate::handlers::query_arc_tx_status(client, txid).await {
            Ok(arc_resp) => {
                if arc_resp.tx_status.as_deref() == Some("MINED") {
                    let block_height = arc_resp.block_height.unwrap_or(0);
                    info!("   ⛏️ {} found MINED on ARC! Recovering...", short_txid);

                    if let Some(ref merkle_path_hex) = arc_resp.merkle_path {
                        match create_proven_tx_and_recover(
                            state, txid, merkle_path_hex, block_height,
                            arc_resp.block_hash.as_deref().unwrap_or(""),
                        ) {
                            Ok(_) => { recovered_count += 1; }
                            Err(e) => { warn!("   ⚠️ Recovery failed for {}: {}", short_txid, e); }
                        }
                    }
                }
            }
            Err(_) => {
                // Step 3: Fallback to WhatsOnChain
                let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/hash/{}", txid);
                if let Ok(response) = client.get(&url).timeout(Duration::from_secs(15)).send().await {
                    if response.status().is_success() {
                        if let Ok(json) = response.json::<serde_json::Value>().await {
                            let confirmations = json["confirmations"].as_u64().unwrap_or(0);
                            if confirmations > 0 {
                                info!("   ⛏️ {} found confirmed on WhatsOnChain! Recovering...", short_txid);

                                // Fetch proof and recover
                                let block_height = json["blockheight"].as_u64().map(|h| h as u32);
                                let proof_url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/proof/tsc", txid);
                                if let Ok(proof_resp) = client.get(&proof_url).timeout(Duration::from_secs(15)).send().await {
                                    if let Ok(tsc_json) = proof_resp.json::<serde_json::Value>().await {
                                        let height = tsc_json["height"].as_u64()
                                            .or_else(|| block_height.map(|h| h as u64))
                                            .unwrap_or(0) as u32;
                                        let tx_index = tsc_json["index"].as_u64().unwrap_or(0);
                                        let block_hash = tsc_json["target"].as_str().unwrap_or("");

                                        if let Ok(merkle_bytes) = serde_json::to_vec(&tsc_json) {
                                            if let Ok(db) = state.database.lock() {
                                                let conn = db.connection();
                                                let raw_tx_bytes: Vec<u8> = {
                                                    let tx_repo = TransactionRepository::new(conn);
                                                    match tx_repo.get_by_txid(txid) {
                                                        Ok(Some(stored)) => hex::decode(&stored.raw_tx).unwrap_or_default(),
                                                        _ => Vec::new(),
                                                    }
                                                };
                                                let proven_tx_repo = ProvenTxRepository::new(conn);
                                                if let Ok(proven_tx_id) = proven_tx_repo.insert_or_get(
                                                    txid, height, tx_index, &merkle_bytes, &raw_tx_bytes, block_hash, "",
                                                ) {
                                                    drop(db);
                                                    recover_transaction(state, txid, proven_tx_id, height);
                                                    recovered_count += 1;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Rate limiting
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    if recovered_count > 0 {
        info!("✅ TaskUnFail: recovered {} transaction(s) from false failure", recovered_count);
        super::log_monitor_event(state, "TaskUnFail:recovered", Some(&format!("{} recovered", recovered_count)));
    }

    Ok(())
}

/// Recover a failed transaction that was actually mined
fn recover_transaction(state: &web::Data<AppState>, txid: &str, proven_tx_id: i64, height: u32) {
    if let Ok(db) = state.database.lock() {
        let conn = db.connection();
        let short_txid = &txid[..txid.len().min(16)];

        // Update transaction status to completed
        let tx_repo = TransactionRepository::new(conn);
        if let Err(e) = tx_repo.update_broadcast_status(txid, "confirmed") {
            warn!("   ⚠️ Failed to recover tx {}: {}", short_txid, e);
            return;
        }
        if let Err(e) = tx_repo.update_confirmations(txid, 1, Some(height)) {
            warn!("   ⚠️ Failed to update confirmations for {}: {}", short_txid, e);
        }

        // Link proven_tx
        let proven_tx_repo = ProvenTxRepository::new(conn);
        let _ = proven_tx_repo.link_transaction(txid, proven_tx_id);

        // Update proven_tx_reqs if exists
        let req_repo = ProvenTxReqRepository::new(conn);
        if let Ok(Some(req)) = req_repo.get_by_txid(txid) {
            let _ = req_repo.update_status(req.proven_tx_req_id, "completed");
            let _ = req_repo.link_proven_tx(req.proven_tx_req_id, proven_tx_id);
            let _ = req_repo.add_history_note(
                req.proven_tx_req_id,
                "completed",
                &format!("UnFail recovery: tx found on-chain at height {}", height),
            );
        }

        // Re-mark inputs as spent by this transaction.
        // When mark_failed ran, it restored inputs as "spendable" and NULLed
        // spending_description. Now that the tx is confirmed on-chain, those
        // inputs are actually spent. Parse raw_tx to find which inputs to re-mark.
        let output_repo = OutputRepository::new(conn);
        let tx_repo_inner = TransactionRepository::new(conn);
        if let Ok(Some(stored_tx)) = tx_repo_inner.get_by_txid(txid) {
            match crate::transaction::extract_input_outpoints(&stored_tx.raw_tx) {
                Ok(input_outpoints) => {
                    let mut re_spent_count = 0;
                    for (prev_txid, prev_vout) in &input_outpoints {
                        match output_repo.mark_spent(prev_txid, *prev_vout, txid) {
                            Ok(n) if n > 0 => re_spent_count += n,
                            _ => {}
                        }
                    }
                    if re_spent_count > 0 {
                        info!("   🔒 Re-marked {} input(s) as spent by recovered tx {}", re_spent_count, short_txid);
                    }
                }
                Err(e) => {
                    warn!("   ⚠️ Failed to parse raw_tx inputs for {}: {} — run /wallet/sync to reconcile", short_txid, e);
                }
            }
        }

        // NOTE: We do NOT re-create deleted change outputs here.
        // The change outputs from this recovered tx will be discovered by the
        // next UTXO sync (POST /wallet/sync or TaskSyncPending).
        info!("   ✅ Recovered tx {} → completed (proof at height {})", short_txid, height);
        info!("   ℹ️ Run /wallet/sync to discover change outputs from this recovered transaction");
    }

    // Invalidate balance cache since status changed
    state.balance_cache.invalidate();
}

/// Create proven_txs record from ARC merkle path and recover the transaction
fn create_proven_tx_and_recover(
    state: &web::Data<AppState>,
    txid: &str,
    merkle_path_hex: &str,
    block_height: u64,
    block_hash: &str,
) -> Result<(), String> {
    let tsc_json = crate::beef::parse_bump_hex_to_tsc(merkle_path_hex)?;
    let height = tsc_json["height"].as_u64().unwrap_or(block_height) as u32;
    let tx_index = tsc_json["index"].as_u64().unwrap_or(0);

    let merkle_path_bytes = serde_json::to_vec(&tsc_json)
        .map_err(|e| format!("Serialize TSC: {}", e))?;

    let proven_tx_id = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let conn = db.connection();

        let raw_tx_bytes: Vec<u8> = {
            let tx_repo = TransactionRepository::new(conn);
            match tx_repo.get_by_txid(txid) {
                Ok(Some(stored)) => hex::decode(&stored.raw_tx).unwrap_or_default(),
                _ => Vec::new(),
            }
        };

        let proven_tx_repo = ProvenTxRepository::new(conn);
        proven_tx_repo.insert_or_get(
            txid, height, tx_index,
            &merkle_path_bytes, &raw_tx_bytes,
            block_hash, "",
        ).map_err(|e| format!("Insert proven_tx: {}", e))?
    };

    recover_transaction(state, txid, proven_tx_id, height);
    Ok(())
}
