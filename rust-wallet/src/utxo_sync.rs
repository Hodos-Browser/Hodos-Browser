//! Background UTXO synchronization service
//!
//! Periodically syncs UTXOs from the blockchain for all used addresses
//! and addresses within the gap limit.

use crate::database::{WalletDatabase, WalletRepository, AddressRepository, OutputRepository};
use crate::utxo_fetcher;
use crate::json_storage::AddressInfo;
use log::{info, warn, error};

/// Default user ID for Phase 4C dual-writes (single-user wallet)
const DEFAULT_USER_ID: i64 = 1;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

pub const SYNC_INTERVAL_SECONDS: u64 = 300; // 5 minutes
const GAP_LIMIT: i32 = 20; // Check 20 unused addresses ahead of highest used address

/// Start background UTXO sync task
///
/// This runs in a separate tokio task and periodically:
/// 1. Gets all used addresses from the database
/// 2. Calculates gap limit (highest used index + gap limit)
/// 3. Fetches UTXOs for all addresses up to gap limit
/// 4. Updates the database cache
pub fn start_background_sync(db: Arc<Mutex<WalletDatabase>>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(SYNC_INTERVAL_SECONDS));

        // Wait a bit before first sync (let server start up)
        tokio::time::sleep(Duration::from_secs(30)).await;

        loop {
            interval.tick().await;

            info!("🔄 Starting periodic UTXO sync...");

            match sync_utxos(&db).await {
                Ok(()) => {
                    info!("✅ Periodic UTXO sync completed");
                }
                Err(e) => {
                    error!("❌ Periodic UTXO sync failed: {}", e);
                }
            }

            // BRC-100: Clean up failed/stale UTXOs
            cleanup_failed_utxos(&db);
        }
    });
}

/// Sync UTXOs for all addresses that need checking
async fn sync_utxos(db: &Arc<Mutex<WalletDatabase>>) -> Result<(), String> {
    // Get wallet and addresses
    let (wallet_id, addresses_to_check) = {
        let db_guard = db.lock().map_err(|e| format!("Failed to lock database: {}", e))?;
        let wallet_repo = WalletRepository::new(db_guard.connection());
        let wallet = wallet_repo.get_primary_wallet()
            .map_err(|e| format!("Failed to get wallet: {}", e))?
            .ok_or_else(|| "No wallet found".to_string())?;

        let wallet_id = wallet.id.ok_or_else(|| "Wallet has no ID".to_string())?;

        let address_repo = AddressRepository::new(db_guard.connection());
        let all_addresses = address_repo.get_all_by_wallet(wallet_id)
            .map_err(|e| format!("Failed to get addresses: {}", e))?;

        // Find highest used address index
        let highest_used = all_addresses.iter()
            .filter(|a| a.used)
            .map(|a| a.index)
            .max()
            .unwrap_or(-1);

        // Calculate scan limit: highest used + gap limit
        let scan_limit = if highest_used >= 0 {
            highest_used + GAP_LIMIT
        } else {
            GAP_LIMIT - 1 // If no addresses used, check first gap_limit addresses
        };

        info!("   📊 Sync stats: highest_used={}, scan_limit={}, total_addresses={}",
              highest_used, scan_limit, all_addresses.len());

        // Get addresses to check: all addresses up to scan_limit
        let addresses_to_check: Vec<AddressInfo> = all_addresses.iter()
            .filter(|a| a.index <= scan_limit)
            .map(|a| AddressInfo {
                index: a.index,
                address: a.address.clone(),
                public_key: a.public_key.clone(),
                used: a.used,
                balance: a.balance,
            })
            .collect();

        drop(db_guard);
        (wallet_id, addresses_to_check)
    };

    if addresses_to_check.is_empty() {
        info!("   ℹ️  No addresses to sync");
        return Ok(());
    }

    info!("   🔍 Fetching UTXOs for {} addresses...", addresses_to_check.len());

    // Fetch UTXOs from API
    let api_utxos = utxo_fetcher::fetch_all_utxos(&addresses_to_check).await
        .map_err(|e| format!("Failed to fetch UTXOs: {}", e))?;

    info!("   📦 Fetched {} UTXOs from API", api_utxos.len());

    // Cache outputs to database
    let db_guard = db.lock().map_err(|e| format!("Failed to lock database: {}", e))?;
    let address_repo = AddressRepository::new(db_guard.connection());
    let output_repo = OutputRepository::new(db_guard.connection());

    let mut updated_count = 0;
    let mut reconciled_count = 0;
    let mut error_count = 0;
    let mut outputs_synced = 0;

    // Grace period: don't mark outputs as externally spent if they were created
    // less than 10 minutes ago (they may be from a recent wallet transaction
    // that hasn't propagated to WhatsOnChain yet)
    const RECONCILE_GRACE_PERIOD_SECS: i64 = 600; // 10 minutes

    for addr_info in &addresses_to_check {
        if let Ok(Some(db_addr)) = address_repo.get_by_address(&addr_info.address) {
            if let Some(addr_id) = db_addr.id {
                // Get UTXOs for this address from API
                let addr_utxos: Vec<_> = api_utxos.iter()
                    .filter(|u| u.address_index == addr_info.index)
                    .cloned()
                    .collect();

                // Upsert API UTXOs into outputs table
                if !addr_utxos.is_empty() {
                    let mut upserted = 0;
                    for utxo in &addr_utxos {
                        // Upsert output (insert if not exists)
                        match output_repo.upsert_received_utxo(
                            DEFAULT_USER_ID,
                            &utxo.txid,
                            utxo.vout,
                            utxo.satoshis,
                            &utxo.script,
                            addr_info.index,
                        ) {
                            Ok(count) if count > 0 => {
                                upserted += 1;
                                outputs_synced += 1;
                            }
                            Ok(_) => {} // Already exists
                            Err(e) => {
                                warn!("   ⚠️  Failed to upsert output for {}:{}: {}",
                                      &utxo.txid[..std::cmp::min(16, utxo.txid.len())], utxo.vout, e);
                                error_count += 1;
                            }
                        }
                    }

                    if upserted > 0 {
                        updated_count += 1;
                        // Mark address as used if it has outputs
                        let _ = address_repo.mark_used(addr_id);
                    }
                }

                // Reconcile: mark database outputs as externally spent if they're
                // no longer in the WhatsOnChain API response. This catches outputs
                // spent on-chain by transactions the wallet doesn't track.
                let derivation_prefix = "2-receive address";
                let derivation_suffix = addr_info.index.to_string();

                match output_repo.reconcile_for_derivation(
                    DEFAULT_USER_ID,
                    Some(derivation_prefix),
                    Some(&derivation_suffix),
                    &addr_utxos,
                    RECONCILE_GRACE_PERIOD_SECS,
                ) {
                    Ok(stale) if stale > 0 => {
                        info!("   🔄 Reconciled {} stale output(s) for {} (marked as externally spent)",
                              stale, addr_info.address);
                        reconciled_count += stale;
                    }
                    Ok(_) => {} // No stale outputs
                    Err(e) => {
                        warn!("   ⚠️  Failed to reconcile outputs for {}: {}", addr_info.address, e);
                    }
                }
            }
        }
    }

    drop(db_guard);

    info!("   ✅ Sync complete: {} addresses updated, {} stale UTXOs reconciled, {} outputs synced, {} errors",
          updated_count, reconciled_count, outputs_synced, error_count);
    Ok(())
}

/// Clean up failed and stale UTXOs
///
/// This function handles (matching SDK TaskFailAbandoned + TaskReviewStatus patterns):
/// 1. UTXOs with status='failed' - remove from tracking
/// 2. UTXOs with status='unproven' older than 1 hour - mark as failed
/// 3. Stale pending-* reservations older than 5 minutes - restore to spendable
/// 4. Outputs consumed by failed broadcasts - restore to spendable (with UnFail delay)
/// 5. Old spent UTXOs (>30 days) - clean up to save space
///
/// UnFail mechanism (Phase 1 addition):
/// When a transaction is marked as failed, we record `failed_at` timestamp.
/// We wait 30 minutes before permanently cleaning up ghost UTXOs and restoring
/// inputs. This prevents fund loss from prematurely failed transactions that
/// were actually mined (e.g., ARC returned error but miners accepted it).
/// During the 30-minute window, the ARC poller may discover the tx was mined
/// and update the status to 'completed', preventing unnecessary cleanup.
///
/// This is part of the optimistic UTXO creation flow (BRC-100):
/// - UTXOs are created with status='unproven' after signing
/// - Status changes to 'completed' after broadcast confirmation
/// - Status changes to 'failed' if broadcast fails
/// - This cleanup job removes failed UTXOs and times out stale ones
fn cleanup_failed_utxos(db: &Arc<Mutex<WalletDatabase>>) {
    info!("🧹 Running UTXO cleanup...");

    let db_guard = match db.lock() {
        Ok(guard) => guard,
        Err(e) => {
            error!("   ❌ Failed to lock database for cleanup: {}", e);
            return;
        }
    };

    let conn = db_guard.connection();

    // Check if status column exists on utxos (handles case where v9 migration hasn't run yet)
    let utxo_status_column_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('utxos') WHERE name = 'status'",
        [],
        |row| Ok(row.get::<_, i64>(0).unwrap_or(0) > 0),
    ).unwrap_or(false);

    // Check if new_status column exists on transactions (v15+)
    let has_new_status: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'new_status'",
        [],
        |row| Ok(row.get::<_, i64>(0).unwrap_or(0) > 0),
    ).unwrap_or(false);

    // Get current time
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    if utxo_status_column_exists {
        // 1. Delete UTXOs with status='failed'
        match conn.execute(
            "DELETE FROM utxos WHERE status = 'failed'",
            [],
        ) {
            Ok(count) if count > 0 => {
                info!("   🗑️  Deleted {} failed UTXOs", count);
            }
            Ok(_) => {} // No failed UTXOs to clean up
            Err(e) => {
                warn!("   ⚠️  Failed to delete failed UTXOs: {}", e);
            }
        }

        // 2. Mark 'unproven' UTXOs older than 1 hour as 'failed'
        let one_hour_ago = now - 3600;
        match conn.execute(
            "UPDATE utxos SET status = 'failed' WHERE status = 'unproven' AND first_seen < ?1",
            rusqlite::params![one_hour_ago],
        ) {
            Ok(count) if count > 0 => {
                info!("   ⏱️  Marked {} stale 'unproven' UTXOs as 'failed'", count);
            }
            Ok(_) => {} // No stale UTXOs
            Err(e) => {
                warn!("   ⚠️  Failed to update stale UTXOs: {}", e);
            }
        }

        // 3. Restore stale pending-* reservations older than 5 minutes
        let five_minutes_ago = now - 300;
        match conn.execute(
            "UPDATE utxos SET is_spent = 0, spent_txid = NULL, spent_at = NULL
             WHERE is_spent = 1 AND spent_txid LIKE 'pending-%'
             AND spent_at IS NOT NULL AND spent_at < ?1",
            rusqlite::params![five_minutes_ago],
        ) {
            Ok(count) if count > 0 => {
                info!("   🔓 Restored {} stale pending reservation(s) (older than 5 min)", count);
            }
            Ok(_) => {} // No stale reservations
            Err(e) => {
                warn!("   ⚠️  Failed to restore stale reservations: {}", e);
            }
        }
    } else {
        info!("   ℹ️  Skipping UTXO status-based cleanup (status column not found - run migration v9)");
    }

    // 4. Restore outputs consumed by failed broadcasts (with UnFail delay)
    // Uses new_status column (v15+) with failed_at timestamp for UnFail mechanism.
    // Only clean up transactions that have been failed for > 30 minutes (UNFAIL_DELAY).
    const UNFAIL_DELAY_SECS: i64 = 1800; // 30 minutes

    if has_new_status {
        let unfail_cutoff = now - UNFAIL_DELAY_SECS;

        // 4a. Restore outputs consumed by confirmed-failed transactions (past UnFail window)
        match conn.execute(
            "UPDATE utxos SET is_spent = 0, spent_txid = NULL, spent_at = NULL
             WHERE is_spent = 1 AND spent_txid IN (
                 SELECT txid FROM transactions
                 WHERE new_status = 'failed' AND failed_at IS NOT NULL AND failed_at < ?1
             )",
            rusqlite::params![unfail_cutoff],
        ) {
            Ok(count) if count > 0 => {
                info!("   🔓 Restored {} output(s) consumed by failed broadcast(s) (past UnFail window)", count);
            }
            Ok(_) => {} // No failed broadcast outputs past window
            Err(e) => {
                warn!("   ⚠️  Failed to restore failed broadcast outputs: {}", e);
            }
        }

        // 4b. Delete ghost UTXOs from confirmed-failed transactions (past UnFail window)
        match conn.execute(
            "DELETE FROM utxos WHERE txid IN (
                 SELECT txid FROM transactions
                 WHERE new_status = 'failed' AND failed_at IS NOT NULL AND failed_at < ?1
             ) AND is_spent = 0",
            rusqlite::params![unfail_cutoff],
        ) {
            Ok(count) if count > 0 => {
                info!("   🗑️  Deleted {} ghost UTXO(s) from failed transactions (past UnFail window)", count);
            }
            Ok(_) => {}
            Err(e) => {
                warn!("   ⚠️  Failed to delete ghost UTXOs: {}", e);
            }
        }

        // 4c. Log transactions in UnFail waiting period (failed < 30 min ago)
        match conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE new_status = 'failed' AND failed_at IS NOT NULL AND failed_at >= ?1",
            rusqlite::params![unfail_cutoff],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(count) if count > 0 => {
                info!("   ⏳ {} transaction(s) in UnFail waiting period (will re-check blockchain before cleanup)", count);
            }
            _ => {}
        }

        // 4d. Mark stale 'unsigned' transactions as failed
        // Transactions stuck in 'unsigned' status for >15 minutes
        let fifteen_minutes_ago = now - 900;
        match conn.execute(
            "UPDATE transactions SET new_status = 'failed', status = 'failed', failed_at = ?1
             WHERE new_status = 'unsigned' AND timestamp < ?2",
            rusqlite::params![now, fifteen_minutes_ago],
        ) {
            Ok(count) if count > 0 => {
                info!("   ⏱️  Marked {} stale unsigned transaction(s) as failed", count);
            }
            Ok(_) => {} // No stale unsigned transactions
            Err(e) => {
                warn!("   ⚠️  Failed to mark stale unsigned transactions: {}", e);
            }
        }
    } else {
        // Pre-v15 fallback: use broadcast_status
        let broadcast_status_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'broadcast_status'",
            [],
            |row| Ok(row.get::<_, i64>(0).unwrap_or(0) > 0),
        ).unwrap_or(false);

        if broadcast_status_exists {
            match conn.execute(
                "UPDATE utxos SET is_spent = 0, spent_txid = NULL, spent_at = NULL
                 WHERE is_spent = 1 AND spent_txid IN (
                     SELECT txid FROM transactions WHERE broadcast_status = 'failed'
                 )",
                [],
            ) {
                Ok(count) if count > 0 => {
                    info!("   🔓 Restored {} output(s) consumed by failed broadcast(s)", count);
                }
                Ok(_) => {}
                Err(e) => {
                    warn!("   ⚠️  Failed to restore failed broadcast outputs: {}", e);
                }
            }

            let fifteen_minutes_ago = now - 900;
            match conn.execute(
                "UPDATE transactions SET broadcast_status = 'failed'
                 WHERE broadcast_status = 'pending' AND timestamp < ?1",
                rusqlite::params![fifteen_minutes_ago],
            ) {
                Ok(count) if count > 0 => {
                    info!("   ⏱️  Marked {} stale pending transaction(s) as failed", count);
                }
                Ok(_) => {}
                Err(e) => {
                    warn!("   ⚠️  Failed to mark stale pending transactions: {}", e);
                }
            }
        }
    }

    // 5. Clean up old spent outputs (older than 30 days) to save space
    let output_repo = OutputRepository::new(conn);
    match output_repo.cleanup_old_spent(30) {
        Ok(count) if count > 0 => {
            info!("   🧹 Cleaned up {} old spent outputs", count);
        }
        Ok(_) => {} // No old outputs to clean
        Err(e) => {
            warn!("   ⚠️  Failed to cleanup old spent outputs: {}", e);
        }
    }

    info!("🧹 Output cleanup complete");
}
