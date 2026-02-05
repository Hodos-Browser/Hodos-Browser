//! ARC Status Poller - Background service for checking transaction confirmation status
//!
//! Periodically queries ARC (GorillaPool) for the status of broadcast transactions.
//! When a transaction is confirmed (MINED), caches the merkle proof and updates
//! the broadcast_status to 'confirmed'.
//!
//! This is Phase 3 of the ARC migration: instead of relying solely on WhatsOnChain
//! block-height checks, we poll ARC directly for merkle proofs, which are needed
//! to build valid BEEF for transaction chaining.

use crate::database::{WalletDatabase, TransactionRepository};
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

/// Poll ARC for pending broadcast transactions
///
/// Returns the number of transactions that were confirmed this cycle.
async fn poll_pending_transactions(db: &Arc<Mutex<WalletDatabase>>) -> Result<usize, String> {
    // Step 1: Get txids with broadcast_status = 'broadcast'
    let pending_txids = {
        let db_guard = db.lock().map_err(|e| format!("Failed to lock DB: {}", e))?;
        let conn = db_guard.connection();

        // Check if broadcast_status column exists
        let column_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'broadcast_status'",
            [],
            |row| Ok(row.get::<_, i64>(0).unwrap_or(0) > 0),
        ).unwrap_or(false);

        if !column_exists {
            return Ok(0);
        }

        let mut stmt = conn.prepare(
            "SELECT txid FROM transactions WHERE broadcast_status = 'broadcast' ORDER BY timestamp DESC LIMIT ?1"
        ).map_err(|e| format!("SQL prepare error: {}", e))?;

        let txids: Vec<String> = stmt.query_map(
            rusqlite::params![MAX_POLL_BATCH as i64],
            |row| row.get(0),
        )
        .map_err(|e| format!("SQL query error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

        txids
    };

    if pending_txids.is_empty() {
        return Ok(0);
    }

    info!("🔍 ARC poller: checking {} pending transactions...", pending_txids.len());

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut confirmed_count = 0;

    for txid in &pending_txids {
        match handlers::query_arc_tx_status(&client, txid).await {
            Ok(arc_resp) => {
                let status = arc_resp.tx_status.as_deref().unwrap_or("UNKNOWN");

                match status {
                    "MINED" => {
                        info!("   ⛏️  {} is MINED (block {})", txid,
                              arc_resp.block_height.unwrap_or(0));

                        // Cache merkle proof if available
                        if let Some(ref merkle_path) = arc_resp.merkle_path {
                            handlers::cache_arc_merkle_proof(
                                &*db,
                                txid,
                                merkle_path,
                            );
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
                            let tx_repo = TransactionRepository::new(db_guard.connection());
                            if let Err(e) = tx_repo.update_broadcast_status(txid, "confirmed") {
                                warn!("   ⚠️  Failed to update broadcast_status for {}: {}", txid, e);
                            }
                            if let Some(height) = arc_resp.block_height {
                                if let Err(e) = tx_repo.update_confirmations(txid, 1, Some(height as u32)) {
                                    warn!("   ⚠️  Failed to update block_height for {}: {}", txid, e);
                                }
                            }
                        }

                        confirmed_count += 1;
                    }
                    "SEEN_ON_NETWORK" | "SEEN_IN_ORPHAN_MEMPOOL" | "ANNOUNCED_TO_NETWORK"
                    | "REQUESTED_BY_NETWORK" | "SENT_TO_NETWORK" | "ACCEPTED_BY_NETWORK"
                    | "STORED" => {
                        // Still pending - normal, just continue
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
                // Don't spam logs for expected 404s (tx not yet propagated to ARC)
                if !e.contains("404") {
                    warn!("   ⚠️  Failed to query ARC for {}: {}", txid, e);
                }
            }
        }

        // Small delay between requests to avoid rate limiting
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Ok(confirmed_count)
}
