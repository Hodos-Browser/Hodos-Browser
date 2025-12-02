//! Background UTXO synchronization service
//!
//! Periodically syncs UTXOs from the blockchain for all used addresses
//! and addresses within the gap limit.

use crate::database::{WalletDatabase, WalletRepository, AddressRepository, UtxoRepository};
use crate::utxo_fetcher;
use crate::json_storage::AddressInfo;
use log::{info, warn, error};
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

    // Cache UTXOs to database
    let db_guard = db.lock().map_err(|e| format!("Failed to lock database: {}", e))?;
    let address_repo = AddressRepository::new(db_guard.connection());
    let utxo_repo = UtxoRepository::new(db_guard.connection());

    let mut updated_count = 0;
    let mut error_count = 0;

    for addr_info in &addresses_to_check {
        if let Ok(Some(db_addr)) = address_repo.get_by_address(&addr_info.address) {
            if let Some(addr_id) = db_addr.id {
                // Get UTXOs for this address
                let addr_utxos: Vec<_> = api_utxos.iter()
                    .filter(|u| u.address_index == addr_info.index as u32)
                    .cloned()
                    .collect();

                if !addr_utxos.is_empty() {
                    match utxo_repo.upsert_utxos(addr_id, &addr_utxos) {
                        Ok(_new_count) => {
                            updated_count += 1;
                            // Mark address as used if it has UTXOs
                            let _ = address_repo.mark_used(addr_id);
                        }
                        Err(e) => {
                            warn!("   ⚠️  Failed to cache UTXOs for {}: {}", addr_info.address, e);
                            error_count += 1;
                        }
                    }
                }
            }
        }
    }

    drop(db_guard);

    info!("   ✅ Sync complete: {} addresses updated, {} errors", updated_count, error_count);
    Ok(())
}
