//! TaskCheckForProofs — Acquire merkle proofs for unproven transactions
//!
//! Replaces arc_status_poller.rs and cache_sync.rs.
//! Queries ARC and WhatsOnChain for merkle proofs, creates proven_txs records,
//! and updates transaction status to 'completed'.
//!
//! Interval: 60 seconds

use actix_web::web;
use log::{info, warn, error};
use std::time::Duration;

use crate::AppState;
use crate::database::{TransactionRepository, OutputRepository, ProvenTxRepository, ProvenTxReqRepository, ParentTransactionRepository, BlockHeaderRepository};

/// Maximum number of transactions to check per cycle (rate limit protection)
const MAX_BATCH: usize = 20;

/// Timeout for transactions WE broadcast (unproven/sending) - 6 hours
const UNPROVEN_TIMEOUT_SECS: i64 = 6 * 60 * 60;

/// Timeout for transactions the APP broadcasts (nosend) - 48 hours
const NOSEND_TIMEOUT_SECS: i64 = 48 * 60 * 60;

/// After this many seconds in mempool, cross-verify with WhatsOnChain
const MEMPOOL_VERIFY_THRESHOLD_SECS: i64 = 30 * 60;

/// Timeout for transactions stuck in orphan mempool - 2 hours
/// Even though we fail immediately on orphan (like wallet-toolbox),
/// TaskUnFail recovers false failures. This timeout is used as a
/// secondary check for non-orphan cases.
const ORPHAN_TIMEOUT_SECS: i64 = 2 * 60 * 60;

struct PendingTxInfo {
    txid: String,
    status: String,
    age_secs: i64,
}

/// Run the TaskCheckForProofs task
pub async fn run(state: &web::Data<AppState>, client: &reqwest::Client) -> Result<(), String> {
    // Step 1: Get transactions needing proof
    let pending_txs: Vec<PendingTxInfo> = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let conn = db.connection();

        let mut stmt = conn.prepare(
            "SELECT txid, status, (strftime('%s', 'now') - created_at) as age_secs
             FROM transactions
             WHERE status IN ('sending', 'unproven', 'nosend')
             ORDER BY created_at DESC LIMIT ?1"
        ).map_err(|e| format!("SQL prepare: {}", e))?;

        let rows = stmt.query_map(
            rusqlite::params![MAX_BATCH as i64],
            |row| Ok(PendingTxInfo {
                txid: row.get(0)?,
                status: row.get(1)?,
                age_secs: row.get(2)?,
            }),
        ).map_err(|e| format!("SQL query: {}", e))?;

        rows.filter_map(|r| r.ok()).collect()
    };

    if pending_txs.is_empty() {
        return Ok(());
    }

    info!("🔍 TaskCheckForProofs: checking {} pending transactions...", pending_txs.len());

    let mut confirmed_count = 0;

    for tx_info in &pending_txs {
        let txid = &tx_info.txid;
        let is_nosend = tx_info.status == "nosend";
        let timeout_secs = if is_nosend { NOSEND_TIMEOUT_SECS } else { UNPROVEN_TIMEOUT_SECS };
        let is_timed_out = tx_info.age_secs > timeout_secs;

        // Check if a proven_txs record already exists (created by another path)
        let already_proven = {
            let db = match state.database.lock() {
                Ok(g) => g,
                Err(e) => { warn!("   ⚠️ DB lock failed: {}", e); continue; }
            };
            let proven_tx_repo = ProvenTxRepository::new(db.connection());
            match proven_tx_repo.get_by_txid(txid) {
                Ok(Some(pt)) => Some(pt),
                _ => None,
            }
        };

        if let Some(proven_tx) = already_proven {
            // Proof exists — reconcile statuses
            info!("   ✅ {} already proven (ID {}), reconciling", txid, proven_tx.proven_tx_id);
            reconcile_proven_tx(state, txid, proven_tx.proven_tx_id, proven_tx.height);
            confirmed_count += 1;
            continue;
        }

        // Query ARC for status
        match crate::handlers::query_arc_tx_status(client, txid).await {
            Ok(arc_resp) => {
                let status = arc_resp.tx_status.as_deref().unwrap_or("UNKNOWN");

                match status {
                    "MINED" => {
                        let block_height = arc_resp.block_height.unwrap_or(0);
                        info!("   ⛏️ {} MINED (block {})", txid, block_height);

                        if let Some(ref merkle_path_hex) = arc_resp.merkle_path {
                            match create_proven_tx_from_arc(
                                state, txid, merkle_path_hex, block_height,
                                arc_resp.block_hash.as_deref().unwrap_or(""),
                            ) {
                                Ok(proven_tx_id) => {
                                    info!("   ✅ Created proven_txs {} for {}", proven_tx_id, txid);
                                }
                                Err(e) => {
                                    warn!("   ⚠️ Failed to create proven_txs for {}: {}", txid, e);
                                }
                            }
                        }

                        // Update transaction status
                        mark_confirmed(state, txid, block_height as u32);
                        confirmed_count += 1;
                    }
                    "SEEN_ON_NETWORK" | "ANNOUNCED_TO_NETWORK"
                    | "REQUESTED_BY_NETWORK" | "SENT_TO_NETWORK" | "ACCEPTED_BY_NETWORK"
                    | "STORED" => {
                        // In mempool — cross-verify with WhatsOnChain if old enough
                        if tx_info.age_secs > MEMPOOL_VERIFY_THRESHOLD_SECS {
                            if let Some(count) = try_whatsonchain_confirmation(state, client, txid).await {
                                confirmed_count += count;
                            }
                        } else {
                            info!("   ⏳ {} in mempool ({})", &txid[..txid.len().min(16)], status);
                        }
                    }
                    "SEEN_IN_ORPHAN_MEMPOOL" => {
                        // Orphan mempool = ARC's node doesn't have the parent tx.
                        // The tx may still be valid on other nodes/miners.
                        //
                        // Strategy (matches wallet-toolbox): fail immediately with cleanup,
                        // then let TaskUnFail recover if the tx actually gets mined.
                        // This is safe because TaskUnFail re-marks inputs as spent
                        // and /wallet/sync reconciles the UTXO set.
                        warn!("   ⚠️ {} in orphan mempool ({}m old)", &txid[..txid.len().min(16)], tx_info.age_secs / 60);

                        // Quick check: is it already confirmed despite ARC's orphan status?
                        match try_whatsonchain_confirmation(state, client, txid).await {
                            Some(count) => {
                                // Already mined — no need to fail
                                confirmed_count += count;
                            }
                            None => {
                                // Not confirmed yet — fail with cleanup.
                                // TaskUnFail will recover if it gets mined later.
                                warn!("   ⏰ {} SEEN_IN_ORPHAN_MEMPOOL, not confirmed — marking FAILED with cleanup",
                                      &txid[..txid.len().min(16)]);
                                mark_failed(state, txid);
                            }
                        }
                    }
                    "DOUBLE_SPEND_ATTEMPTED" | "REJECTED" => {
                        warn!("   ⚠️ {} status: {} — marking failed", txid, status);
                        mark_failed(state, txid);
                    }
                    other => {
                        info!("   ℹ️ {} status: {}", txid, other);
                    }
                }
            }
            Err(e) => {
                if e.contains("404") {
                    // Not on ARC — fallback to WhatsOnChain
                    info!("   ℹ️ {} not on ARC, checking WhatsOnChain...", &txid[..txid.len().min(16)]);
                    match try_whatsonchain_confirmation(state, client, txid).await {
                        Some(count) => { confirmed_count += count; }
                        None => {
                            // Not found anywhere
                            if is_timed_out {
                                let hours = tx_info.age_secs / 3600;
                                warn!("   ⏰ {} not found after {}h — marking FAILED", &txid[..txid.len().min(16)], hours);
                                mark_failed(state, txid);
                            }
                        }
                    }
                } else {
                    warn!("   ⚠️ ARC query failed for {}: {}", txid, e);
                }
            }
        }

        // Rate limiting
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    if confirmed_count > 0 {
        info!("✅ TaskCheckForProofs: {} transactions confirmed", confirmed_count);
        super::log_monitor_event(state, "TaskCheckForProofs:completed", Some(&format!("{} confirmed", confirmed_count)));
    }

    Ok(())
}

/// Try to confirm a transaction via WhatsOnChain, returning Some(1) if confirmed
async fn try_whatsonchain_confirmation(
    state: &web::Data<AppState>,
    client: &reqwest::Client,
    txid: &str,
) -> Option<usize> {
    match check_whatsonchain_confirmation(client, txid).await {
        Ok(Some((confirmations, block_height))) if confirmations > 0 => {
            info!("   ⛏️ {} confirmed on WhatsOnChain ({} confirmations)", &txid[..txid.len().min(16)], confirmations);

            // Fetch and store proof
            if let Err(e) = fetch_and_store_woc_proof(state, client, txid, block_height).await {
                warn!("   ⚠️ Failed to store WoC proof for {}: {}", txid, e);
            }

            mark_confirmed(state, txid, block_height.unwrap_or(0));
            Some(1)
        }
        Ok(Some(_)) => {
            // 0 confirmations — still in mempool
            None
        }
        Ok(None) => {
            // Not found on WhatsOnChain
            None
        }
        Err(e) => {
            warn!("   ⚠️ WhatsOnChain check failed for {}: {}", txid, e);
            None
        }
    }
}

/// Mark a transaction as confirmed (completed)
fn mark_confirmed(state: &web::Data<AppState>, txid: &str, block_height: u32) {
    if let Ok(db) = state.database.lock() {
        let conn = db.connection();
        let tx_repo = TransactionRepository::new(conn);
        if let Err(e) = tx_repo.update_broadcast_status(txid, "confirmed") {
            warn!("   ⚠️ Failed to update status for {}: {}", txid, e);
        }
        if let Err(e) = tx_repo.update_confirmations(txid, 1, Some(block_height)) {
            warn!("   ⚠️ Failed to update confirmations for {}: {}", txid, e);
        }
    }
}

/// Mark a transaction as failed with full ghost output cleanup.
///
/// Ghost Transaction Safety:
/// Order: mark failed → delete ghost outputs → restore inputs → invalidate cache
fn mark_failed(state: &web::Data<AppState>, txid: &str) {
    let short_txid = &txid[..txid.len().min(16)];

    if let Ok(db) = state.database.lock() {
        let conn = db.connection();

        // 1. Mark as failed with timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        if let Err(e) = conn.execute(
            "UPDATE transactions SET status = 'failed', failed_at = ?1 WHERE txid = ?2 AND status != 'failed'",
            rusqlite::params![now, txid],
        ) {
            warn!("   ⚠️ Failed to mark {} as failed: {}", short_txid, e);
            return;
        }

        // 2. Delete ghost change outputs created by this transaction
        let output_repo = OutputRepository::new(conn);
        match output_repo.delete_by_txid(txid) {
            Ok(count) if count > 0 => {
                info!("   🗑️ Deleted {} ghost output(s) from tx {}", count, short_txid);
            }
            Err(e) => {
                warn!("   ⚠️ Failed to delete ghost outputs for {}: {}", short_txid, e);
            }
            _ => {}
        }

        // 3. Restore input outputs that were spent by this transaction
        match output_repo.restore_spent_by_txid(txid) {
            Ok(count) if count > 0 => {
                info!("   ♻️ Restored {} input(s) from tx {}", count, short_txid);
            }
            Err(e) => {
                warn!("   ⚠️ Failed to restore inputs for {}: {}", short_txid, e);
            }
            _ => {
                // Fallback for placeholder reservations
                if let Ok(count) = output_repo.restore_by_spending_description(txid) {
                    if count > 0 {
                        info!("   ♻️ Restored {} input(s) via spending_description from tx {}", count, short_txid);
                    }
                }
            }
        }

        info!("   ✅ Failed tx {} cleaned up (ghost outputs deleted, inputs restored)", short_txid);
    }

    // 4. Invalidate balance cache (outside DB lock)
    state.balance_cache.invalidate();
}

/// Reconcile a transaction that already has a proven_txs record
fn reconcile_proven_tx(state: &web::Data<AppState>, txid: &str, proven_tx_id: i64, height: u32) {
    if let Ok(db) = state.database.lock() {
        let conn = db.connection();

        let tx_repo = TransactionRepository::new(conn);
        if let Err(e) = tx_repo.update_broadcast_status(txid, "confirmed") {
            warn!("   ⚠️ Failed to update status for {}: {}", txid, e);
        }
        if let Err(e) = tx_repo.update_confirmations(txid, 1, Some(height)) {
            warn!("   ⚠️ Failed to update confirmations for {}: {}", txid, e);
        }

        let proven_tx_repo = ProvenTxRepository::new(conn);
        let _ = proven_tx_repo.link_transaction(txid, proven_tx_id);

        let req_repo = ProvenTxReqRepository::new(conn);
        if let Ok(Some(req)) = req_repo.get_by_txid(txid) {
            let _ = req_repo.update_status(req.proven_tx_req_id, "completed");
            let _ = req_repo.link_proven_tx(req.proven_tx_req_id, proven_tx_id);
            let _ = req_repo.add_history_note(
                req.proven_tx_req_id,
                "completed",
                "Reconciled from existing proven_txs record",
            );
        }
    }
}

/// Create a proven_txs record from ARC's MINED response
fn create_proven_tx_from_arc(
    state: &web::Data<AppState>,
    txid: &str,
    merkle_path_hex: &str,
    block_height: u64,
    block_hash: &str,
) -> Result<i64, String> {
    let tsc_json = crate::beef::parse_bump_hex_to_tsc(merkle_path_hex)?;

    let height = tsc_json["height"].as_u64().unwrap_or(block_height) as u32;
    let tx_index = tsc_json["index"].as_u64().unwrap_or(0);

    let merkle_path_bytes = serde_json::to_vec(&tsc_json)
        .map_err(|e| format!("Failed to serialize TSC: {}", e))?;

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
    let proven_tx_id = proven_tx_repo.insert_or_get(
        txid, height, tx_index,
        &merkle_path_bytes, &raw_tx_bytes,
        block_hash, "",
    ).map_err(|e| format!("Failed to insert proven_tx: {}", e))?;

    let _ = proven_tx_repo.link_transaction(txid, proven_tx_id);

    let req_repo = ProvenTxReqRepository::new(conn);
    if let Ok(Some(req)) = req_repo.get_by_txid(txid) {
        let _ = req_repo.update_status(req.proven_tx_req_id, "completed");
        let _ = req_repo.link_proven_tx(req.proven_tx_req_id, proven_tx_id);
        let _ = req_repo.add_history_note(
            req.proven_tx_req_id,
            "completed",
            &format!("Proof acquired from ARC at height {}", height),
        );
    }

    Ok(proven_tx_id)
}

/// Check WhatsOnChain for transaction confirmation status
async fn check_whatsonchain_confirmation(
    client: &reqwest::Client,
    txid: &str,
) -> Result<Option<(u32, Option<u32>)>, String> {
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/hash/{}", txid);

    let response = client.get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if response.status().as_u16() == 404 {
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(format!("WoC status: {}", response.status()));
    }

    let json: serde_json::Value = response.json()
        .await
        .map_err(|e| format!("JSON parse: {}", e))?;

    let confirmations = json["confirmations"].as_u64().unwrap_or(0) as u32;
    let block_height = json["blockheight"].as_u64().map(|h| h as u32);

    Ok(Some((confirmations, block_height)))
}

/// Fetch merkle proof from WhatsOnChain and store as proven_txs record
async fn fetch_and_store_woc_proof(
    state: &web::Data<AppState>,
    client: &reqwest::Client,
    txid: &str,
    block_height: Option<u32>,
) -> Result<i64, String> {
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/proof/tsc", txid);

    let response = client.get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("WoC proof API status: {}", response.status()));
    }

    let tsc_json: serde_json::Value = response.json()
        .await
        .map_err(|e| format!("JSON parse: {}", e))?;

    let height = tsc_json["height"].as_u64()
        .or_else(|| block_height.map(|h| h as u64))
        .ok_or("Missing height in TSC proof")? as u32;
    let tx_index = tsc_json["index"].as_u64().unwrap_or(0);
    let block_hash = tsc_json["target"].as_str().unwrap_or("");

    let merkle_path_bytes = serde_json::to_vec(&tsc_json)
        .map_err(|e| format!("Serialize TSC: {}", e))?;

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
    let proven_tx_id = proven_tx_repo.insert_or_get(
        txid, height, tx_index,
        &merkle_path_bytes, &raw_tx_bytes,
        block_hash, "",
    ).map_err(|e| format!("Insert proven_tx: {}", e))?;

    let _ = proven_tx_repo.link_transaction(txid, proven_tx_id);

    let req_repo = ProvenTxReqRepository::new(conn);
    if let Ok(Some(req)) = req_repo.get_by_txid(txid) {
        let _ = req_repo.update_status(req.proven_tx_req_id, "completed");
        let _ = req_repo.link_proven_tx(req.proven_tx_req_id, proven_tx_id);
        let _ = req_repo.add_history_note(
            req.proven_tx_req_id,
            "completed",
            &format!("Proof acquired from WhatsOnChain at height {}", height),
        );
    }

    Ok(proven_tx_id)
}
