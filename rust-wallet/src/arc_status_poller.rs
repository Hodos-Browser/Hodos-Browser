//! ARC Status Poller - Background service for checking transaction confirmation status
//!
//! Periodically queries ARC (GorillaPool) for the status of broadcast transactions.
//! When a transaction is confirmed (MINED), creates an immutable proven_txs record
//! and links it to the transaction. Updates proven_tx_reqs lifecycle tracking.

use crate::database::{WalletDatabase, TransactionRepository, ProvenTxRepository, ProvenTxReqRepository};
use crate::handlers;
use log::{info, warn, error};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

/// Poll interval: check every 60 seconds
pub const POLL_INTERVAL_SECONDS: u64 = 60;

/// Maximum number of transactions to poll per cycle (rate limit protection)
const MAX_POLL_BATCH: usize = 20;

/// Start the ARC status poller background task
///
/// Follows the same pattern as utxo_sync::start_background_sync.
/// Queries for transactions with broadcast_status='broadcast' and checks
/// their status on ARC. When MINED, caches the merkle proof.
pub fn start_arc_status_poller(db: Arc<Mutex<WalletDatabase>>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(POLL_INTERVAL_SECONDS));

        // Wait before first poll (let server start up and initial broadcasts complete)
        tokio::time::sleep(Duration::from_secs(45)).await;

        loop {
            interval.tick().await;

            match poll_pending_transactions(&db).await {
                Ok(confirmed_count) => {
                    if confirmed_count > 0 {
                        info!("✅ ARC poller: {} transactions confirmed", confirmed_count);
                    }
                }
                Err(e) => {
                    error!("❌ ARC poller error: {}", e);
                }
            }
        }
    });
}

/// Timeout for transactions WE broadcast (unproven/sending) - 6 hours
const UNPROVEN_TIMEOUT_SECS: i64 = 6 * 60 * 60;

/// Timeout for transactions the APP broadcasts (nosend) - 48 hours
/// Longer because we don't control when the app submits to overlay
const NOSEND_TIMEOUT_SECS: i64 = 48 * 60 * 60;

/// Transaction info for polling - includes status and age for timeout logic
struct PendingTxInfo {
    txid: String,
    new_status: String,
    age_secs: i64,
}

/// Poll ARC for pending broadcast transactions
///
/// Returns the number of transactions that were confirmed this cycle.
async fn poll_pending_transactions(db: &Arc<Mutex<WalletDatabase>>) -> Result<usize, String> {
    // Step 1: Get txids with new_status in ('sending', 'unproven', 'nosend') — these need confirmation checking
    // Note: All DB operations must complete before any .await to satisfy Send requirements
    let pending_txs: Vec<PendingTxInfo> = {
        let db_guard = db.lock().map_err(|e| format!("Failed to lock DB: {}", e))?;
        let conn = db_guard.connection();

        // Prefer new_status column (v15+)
        let has_new_status: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'new_status'",
            [],
            |row| Ok(row.get::<_, i64>(0).unwrap_or(0) > 0),
        ).unwrap_or(false);

        let query = if has_new_status {
            "SELECT txid, new_status, (strftime('%s', 'now') - timestamp) as age_secs
             FROM transactions
             WHERE new_status IN ('sending', 'unproven', 'nosend')
             ORDER BY timestamp DESC LIMIT ?1"
        } else {
            // Fallback to broadcast_status for pre-v15 databases
            let column_exists: bool = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'broadcast_status'",
                [],
                |row| Ok(row.get::<_, i64>(0).unwrap_or(0) > 0),
            ).unwrap_or(false);

            if !column_exists {
                return Ok(0);
            }
            "SELECT txid, 'unproven' as new_status, (strftime('%s', 'now') - timestamp) as age_secs
             FROM transactions
             WHERE broadcast_status = 'broadcast'
             ORDER BY timestamp DESC LIMIT ?1"
        };

        let mut stmt = conn.prepare(query)
            .map_err(|e| format!("SQL prepare error: {}", e))?;

        let rows = stmt.query_map(
            rusqlite::params![MAX_POLL_BATCH as i64],
            |row| Ok(PendingTxInfo {
                txid: row.get(0)?,
                new_status: row.get(1)?,
                age_secs: row.get(2)?,
            }),
        ).map_err(|e| format!("SQL query error: {}", e))?;

        rows.filter_map(|r| r.ok()).collect()
    }; // db_guard and stmt dropped here, before any .await

    poll_pending_transactions_inner(db, pending_txs).await
}

async fn poll_pending_transactions_inner(db: &Arc<Mutex<WalletDatabase>>, pending_txs: Vec<PendingTxInfo>) -> Result<usize, String> {
    if pending_txs.is_empty() {
        return Ok(0);
    }

    info!("🔍 ARC poller: checking {} pending transactions...", pending_txs.len());

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut confirmed_count = 0;

    for tx_info in &pending_txs {
        let txid = &tx_info.txid;
        let is_nosend = tx_info.new_status == "nosend";
        let timeout_secs = if is_nosend { NOSEND_TIMEOUT_SECS } else { UNPROVEN_TIMEOUT_SECS };
        let is_timed_out = tx_info.age_secs > timeout_secs;

        // Check if cache_sync (or another service) already created a proven_txs record
        // for this transaction. If so, update statuses without querying ARC.
        let already_proven = {
            let db_guard = match db.lock() {
                Ok(g) => g,
                Err(e) => {
                    warn!("   ⚠️  Failed to lock DB for proven check: {}", e);
                    continue;
                }
            };
            let conn = db_guard.connection();
            let proven_tx_repo = ProvenTxRepository::new(conn);
            match proven_tx_repo.get_by_txid(txid) {
                Ok(Some(proven_tx)) => Some(proven_tx),
                _ => None,
            }
        };

        if let Some(proven_tx) = already_proven {
            info!("   ✅ {} already has proven_txs record (ID {}), updating statuses", txid, proven_tx.proven_tx_id);

            let db_guard = match db.lock() {
                Ok(g) => g,
                Err(e) => {
                    warn!("   ⚠️  Failed to lock DB for status reconciliation: {}", e);
                    continue;
                }
            };
            let conn = db_guard.connection();

            // Update transaction status to completed
            let tx_repo = TransactionRepository::new(conn);
            if let Err(e) = tx_repo.update_broadcast_status(txid, "confirmed") {
                warn!("   ⚠️  Failed to update broadcast_status for {}: {}", txid, e);
            }
            if let Err(e) = tx_repo.update_confirmations(txid, 1, Some(proven_tx.height)) {
                warn!("   ⚠️  Failed to update confirmations for {}: {}", txid, e);
            }

            // Link proven_tx_id if not already linked
            let proven_tx_repo = ProvenTxRepository::new(conn);
            let _ = proven_tx_repo.link_transaction(txid, proven_tx.proven_tx_id);

            // Update proven_tx_reqs if one exists
            let req_repo = ProvenTxReqRepository::new(conn);
            if let Ok(Some(req)) = req_repo.get_by_txid(txid) {
                let _ = req_repo.update_status(req.proven_tx_req_id, "completed");
                let _ = req_repo.link_proven_tx(req.proven_tx_req_id, proven_tx.proven_tx_id);
                let _ = req_repo.add_history_note(
                    req.proven_tx_req_id,
                    "completed",
                    "Reconciled from existing proven_txs record",
                );
            }

            confirmed_count += 1;
            continue;
        }

        match handlers::query_arc_tx_status(&client, txid).await {
            Ok(arc_resp) => {
                let status = arc_resp.tx_status.as_deref().unwrap_or("UNKNOWN");

                match status {
                    "MINED" => {
                        let block_height = arc_resp.block_height.unwrap_or(0);
                        info!("   ⛏️  {} is MINED (block {})", txid, block_height);

                        // Create proven_txs record from ARC merkle proof
                        if let Some(ref merkle_path_hex) = arc_resp.merkle_path {
                            match create_proven_tx_from_arc(
                                db, txid, merkle_path_hex,
                                block_height,
                                arc_resp.block_hash.as_deref().unwrap_or(""),
                            ) {
                                Ok(proven_tx_id) => {
                                    info!("   ✅ Created proven_txs record {} for {}", proven_tx_id, txid);
                                }
                                Err(e) => {
                                    warn!("   ⚠️  Failed to create proven_txs record for {}: {}", txid, e);
                                }
                            }
                        }

                        // Update broadcast_status to 'confirmed' and block_height
                        {
                            let db_guard = match db.lock() {
                                Ok(g) => g,
                                Err(e) => {
                                    warn!("   ⚠️  Failed to lock DB for status update: {}", e);
                                    continue;
                                }
                            };
                            let conn = db_guard.connection();
                            let tx_repo = TransactionRepository::new(conn);
                            if let Err(e) = tx_repo.update_broadcast_status(txid, "confirmed") {
                                warn!("   ⚠️  Failed to update broadcast_status for {}: {}", txid, e);
                            }
                            if let Err(e) = tx_repo.update_confirmations(txid, 1, Some(block_height as u32)) {
                                warn!("   ⚠️  Failed to update block_height for {}: {}", txid, e);
                            }
                        }

                        confirmed_count += 1;
                    }
                    "SEEN_ON_NETWORK" | "SEEN_IN_ORPHAN_MEMPOOL" | "ANNOUNCED_TO_NETWORK"
                    | "REQUESTED_BY_NETWORK" | "SENT_TO_NETWORK" | "ACCEPTED_BY_NETWORK"
                    | "STORED" => {
                        // ARC says still in mempool, but if it's been more than 30 mins,
                        // verify with WhatsOnChain since ARC data may be stale
                        const MEMPOOL_VERIFY_THRESHOLD_SECS: i64 = 30 * 60; // 30 minutes

                        if tx_info.age_secs > MEMPOOL_VERIFY_THRESHOLD_SECS {
                            let mins = tx_info.age_secs / 60;
                            info!("   ⏳ {} ARC says: {} but {} mins old - verifying on WhatsOnChain...",
                                  &txid[..std::cmp::min(16, txid.len())], status, mins);

                            // Verify with WhatsOnChain - ARC might have stale data
                            match check_whatsonchain_confirmation(&client, txid).await {
                                Ok(Some((confirmations, block_height))) => {
                                    if confirmations > 0 {
                                        info!("   ⛏️  {} is actually CONFIRMED on WhatsOnChain ({} confirmations)!",
                                              &txid[..std::cmp::min(16, txid.len())], confirmations);

                                        // Fetch merkle proof and create proven_txs record
                                        match fetch_and_store_woc_proof(db, &client, txid, block_height).await {
                                            Ok(proven_tx_id) => {
                                                info!("   ✅ Created proven_txs record {} from WhatsOnChain", proven_tx_id);
                                            }
                                            Err(e) => {
                                                warn!("   ⚠️  Failed to fetch/store WoC proof for {}: {}", txid, e);
                                            }
                                        }

                                        // Update transaction status
                                        let db_guard = match db.lock() {
                                            Ok(g) => g,
                                            Err(e) => {
                                                warn!("   ⚠️  Failed to lock DB: {}", e);
                                                continue;
                                            }
                                        };
                                        let conn = db_guard.connection();
                                        let tx_repo = TransactionRepository::new(conn);
                                        if let Err(e) = tx_repo.update_broadcast_status(txid, "confirmed") {
                                            warn!("   ⚠️  Failed to update broadcast_status: {}", e);
                                        }
                                        if let Err(e) = tx_repo.update_confirmations(txid, confirmations, block_height) {
                                            warn!("   ⚠️  Failed to update confirmations: {}", e);
                                        }

                                        confirmed_count += 1;
                                    } else {
                                        // WhatsOnChain also says unconfirmed - ARC is correct
                                        info!("   ⏳ {} confirmed unconfirmed on both ARC and WhatsOnChain ({} mins)",
                                              &txid[..std::cmp::min(16, txid.len())], mins);
                                    }
                                }
                                Ok(None) => {
                                    // Not on WhatsOnChain - strange since ARC has it
                                    warn!("   ⚠️  {} on ARC but not WhatsOnChain - will retry later",
                                          &txid[..std::cmp::min(16, txid.len())]);
                                }
                                Err(e) => {
                                    warn!("   ⚠️  WhatsOnChain verification failed for {}: {}", txid, e);
                                }
                            }
                        } else {
                            // Recent transaction - trust ARC's mempool status
                            info!("   ⏳ {} status: {} (in mempool)", &txid[..std::cmp::min(16, txid.len())], status);
                        }
                    }
                    "DOUBLE_SPEND_ATTEMPTED" | "REJECTED" => {
                        warn!("   ⚠️  {} has status: {} - marking as failed", txid, status);
                        let db_guard = match db.lock() {
                            Ok(g) => g,
                            Err(e) => {
                                warn!("   ⚠️  Failed to lock DB: {}", e);
                                continue;
                            }
                        };
                        let tx_repo = TransactionRepository::new(db_guard.connection());
                        if let Err(e) = tx_repo.update_broadcast_status(txid, "failed") {
                            warn!("   ⚠️  Failed to update broadcast_status for {}: {}", txid, e);
                        }
                    }
                    other => {
                        info!("   ℹ️  {} has status: {}", txid, other);
                    }
                }
            }
            Err(e) => {
                // ARC doesn't know about this transaction - fall back to WhatsOnChain
                if e.contains("404") {
                    info!("   ℹ️  {} not found in ARC, checking WhatsOnChain...", &txid[..std::cmp::min(16, txid.len())]);

                    // Fall back to WhatsOnChain for confirmation status
                    match check_whatsonchain_confirmation(&client, txid).await {
                        Ok(Some((confirmations, block_height))) => {
                            if confirmations > 0 {
                                info!("   ⛏️  {} is CONFIRMED on WhatsOnChain ({} confirmations, block {})",
                                      &txid[..std::cmp::min(16, txid.len())], confirmations, block_height.unwrap_or(0));

                                // Fetch merkle proof from WhatsOnChain and create proven_txs record
                                match fetch_and_store_woc_proof(db, &client, txid, block_height).await {
                                    Ok(proven_tx_id) => {
                                        info!("   ✅ Created proven_txs record {} from WhatsOnChain", proven_tx_id);
                                    }
                                    Err(e) => {
                                        warn!("   ⚠️  Failed to fetch/store WoC proof for {}: {}", txid, e);
                                    }
                                }

                                // Update transaction status
                                let db_guard = match db.lock() {
                                    Ok(g) => g,
                                    Err(e) => {
                                        warn!("   ⚠️  Failed to lock DB: {}", e);
                                        continue;
                                    }
                                };
                                let conn = db_guard.connection();
                                let tx_repo = TransactionRepository::new(conn);
                                if let Err(e) = tx_repo.update_broadcast_status(txid, "confirmed") {
                                    warn!("   ⚠️  Failed to update broadcast_status for {}: {}", txid, e);
                                }
                                if let Err(e) = tx_repo.update_confirmations(txid, confirmations, block_height) {
                                    warn!("   ⚠️  Failed to update confirmations for {}: {}", txid, e);
                                }

                                confirmed_count += 1;
                            }
                            // else: 0 confirmations means still in mempool - continue waiting
                        }
                        Ok(None) => {
                            // Transaction not found on WhatsOnChain either
                            // This could mean it was never broadcast or was dropped from mempool
                            if is_timed_out {
                                let hours = tx_info.age_secs / 3600;
                                warn!("   ⏰ {} not found anywhere after {} hours - marking as FAILED",
                                      &txid[..std::cmp::min(16, txid.len())], hours);

                                // Mark as failed so UTXOs can be reclaimed
                                let db_guard = match db.lock() {
                                    Ok(g) => g,
                                    Err(e) => {
                                        warn!("   ⚠️  Failed to lock DB: {}", e);
                                        continue;
                                    }
                                };
                                let conn = db_guard.connection();
                                let tx_repo = TransactionRepository::new(conn);
                                if let Err(e) = tx_repo.update_broadcast_status(txid, "failed") {
                                    warn!("   ⚠️  Failed to update status to failed: {}", e);
                                } else {
                                    info!("   ❌ {} marked as failed (timeout: {} was {} hours old)",
                                          &txid[..std::cmp::min(16, txid.len())],
                                          if is_nosend { "nosend 48h" } else { "unproven 6h" },
                                          hours);
                                }
                            } else {
                                let hours = tx_info.age_secs / 3600;
                                let timeout_hours = timeout_secs / 3600;
                                info!("   ⚠️  {} not found on WhatsOnChain either ({} hours old, timeout at {}h)",
                                      &txid[..std::cmp::min(16, txid.len())], hours, timeout_hours);
                            }
                        }
                        Err(e) => {
                            warn!("   ⚠️  WhatsOnChain fallback failed for {}: {}", txid, e);
                        }
                    }
                } else {
                    warn!("   ⚠️  Failed to query ARC for {}: {}", txid, e);
                }
            }
        }

        // Small delay between requests to avoid rate limiting
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Ok(confirmed_count)
}

/// Create a proven_txs record from ARC's MINED response.
///
/// Parses the BUMP merkle path, gets the raw transaction, creates an immutable
/// proven_txs record, links it to the transaction, and updates any existing
/// proven_tx_reqs entry.
fn create_proven_tx_from_arc(
    db: &Arc<Mutex<WalletDatabase>>,
    txid: &str,
    merkle_path_hex: &str,
    block_height: u64,
    block_hash: &str,
) -> Result<i64, String> {
    // Parse BUMP hex to TSC JSON
    let tsc_json = crate::beef::parse_bump_hex_to_tsc(merkle_path_hex)?;

    let height = tsc_json["height"].as_u64().unwrap_or(block_height) as u32;
    let tx_index = tsc_json["index"].as_u64().unwrap_or(0);

    // Serialize TSC JSON to bytes for storage
    let merkle_path_bytes = serde_json::to_vec(&tsc_json)
        .map_err(|e| format!("Failed to serialize TSC JSON: {}", e))?;

    // Get raw_tx from transactions table
    let db_guard = db.lock().map_err(|e| format!("Failed to lock DB: {}", e))?;
    let conn = db_guard.connection();

    let raw_tx_bytes: Vec<u8> = {
        let tx_repo = TransactionRepository::new(conn);
        match tx_repo.get_by_txid(txid) {
            Ok(Some(stored)) => {
                hex::decode(&stored.raw_tx).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    };

    // Create immutable proven_txs record
    let proven_tx_repo = ProvenTxRepository::new(conn);
    let proven_tx_id = proven_tx_repo.insert_or_get(
        txid, height, tx_index,
        &merkle_path_bytes, &raw_tx_bytes,
        block_hash, "",
    ).map_err(|e| format!("Failed to insert proven_tx: {}", e))?;

    // Link transaction to proven_txs
    if let Err(e) = proven_tx_repo.link_transaction(txid, proven_tx_id) {
        warn!("   ⚠️  Failed to link transaction {} to proven_tx {}: {}", txid, proven_tx_id, e);
    }

    // Update proven_tx_reqs if one exists for this txid
    let req_repo = ProvenTxReqRepository::new(conn);
    if let Ok(Some(req)) = req_repo.get_by_txid(txid) {
        if let Err(e) = req_repo.update_status(req.proven_tx_req_id, "completed") {
            warn!("   ⚠️  Failed to update proven_tx_req status: {}", e);
        }
        if let Err(e) = req_repo.link_proven_tx(req.proven_tx_req_id, proven_tx_id) {
            warn!("   ⚠️  Failed to link proven_tx_req to proven_tx: {}", e);
        }
        let _ = req_repo.add_history_note(
            req.proven_tx_req_id,
            "completed",
            &format!("Proof acquired from ARC at height {}", height),
        );
    }

    Ok(proven_tx_id)
}

/// Check WhatsOnChain for transaction confirmation status
///
/// Returns Ok(Some((confirmations, block_height))) if found, Ok(None) if not found (404)
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

    let status = response.status();

    if status.as_u16() == 404 {
        // Transaction not found
        return Ok(None);
    }

    if !status.is_success() {
        return Err(format!("WhatsOnChain returned status: {}", status));
    }

    let json: serde_json::Value = response.json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let confirmations = json["confirmations"].as_u64().unwrap_or(0) as u32;
    let block_height = json["blockheight"].as_u64().map(|h| h as u32);

    Ok(Some((confirmations, block_height)))
}

/// Fetch merkle proof from WhatsOnChain and store as proven_txs record
async fn fetch_and_store_woc_proof(
    db: &Arc<Mutex<WalletDatabase>>,
    client: &reqwest::Client,
    txid: &str,
    block_height: Option<u32>,
) -> Result<i64, String> {
    // Fetch TSC merkle proof from WhatsOnChain
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/proof/tsc", txid);

    let response = client.get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("HTTP error fetching proof: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("WhatsOnChain proof API returned status: {}", response.status()));
    }

    let tsc_json: serde_json::Value = response.json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    // Extract proof data from TSC format
    let height = tsc_json["height"].as_u64()
        .or_else(|| block_height.map(|h| h as u64))
        .ok_or("Missing height in TSC proof")? as u32;
    let tx_index = tsc_json["index"].as_u64().unwrap_or(0);
    let block_hash = tsc_json["target"].as_str().unwrap_or("");

    // Serialize TSC JSON to bytes for storage
    let merkle_path_bytes = serde_json::to_vec(&tsc_json)
        .map_err(|e| format!("Failed to serialize TSC JSON: {}", e))?;

    // Get raw_tx from transactions table
    let db_guard = db.lock().map_err(|e| format!("Failed to lock DB: {}", e))?;
    let conn = db_guard.connection();

    let raw_tx_bytes: Vec<u8> = {
        let tx_repo = TransactionRepository::new(conn);
        match tx_repo.get_by_txid(txid) {
            Ok(Some(stored)) => {
                hex::decode(&stored.raw_tx).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    };

    // Create immutable proven_txs record
    let proven_tx_repo = ProvenTxRepository::new(conn);
    let proven_tx_id = proven_tx_repo.insert_or_get(
        txid, height, tx_index,
        &merkle_path_bytes, &raw_tx_bytes,
        block_hash, "",
    ).map_err(|e| format!("Failed to insert proven_tx: {}", e))?;

    // Link transaction to proven_txs
    if let Err(e) = proven_tx_repo.link_transaction(txid, proven_tx_id) {
        warn!("   ⚠️  Failed to link transaction {} to proven_tx {}: {}", txid, proven_tx_id, e);
    }

    // Update proven_tx_reqs if one exists for this txid
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
