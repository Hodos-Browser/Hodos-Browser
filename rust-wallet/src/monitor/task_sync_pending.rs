//! TaskSyncPending — Periodic UTXO sync for pending addresses
//!
//! Scans addresses with `pending_utxo_check = 1` against WhatsOnChain,
//! inserts new outputs, reconciles stale ones, and clears the pending flag.
//!
//! This fills the gap left when Phase 6I removed the background utxo_sync
//! service: without this task, newly generated addresses are only checked
//! when the frontend explicitly calls POST /wallet/sync.
//!
//! Interval: 30 seconds

use actix_web::web;
use log::{info, warn};

use crate::AppState;
use crate::database::{AddressRepository, OutputRepository, WalletRepository};

/// Grace period: don't mark outputs as externally spent if created < 10 min ago
const RECONCILE_GRACE_PERIOD_SECS: i64 = 600;

/// Stale pending addresses older than this are cleared without checking
const PENDING_TIMEOUT_HOURS: i64 = 240; // 10 days

/// Run the TaskSyncPending task
pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    // Step 1: Get pending addresses (DB lock held briefly)
    let (wallet_id, addresses_to_sync) = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let wallet_repo = WalletRepository::new(db.connection());
        let wallet = wallet_repo.get_primary_wallet()
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or_else(|| "No wallet found".to_string())?;

        let wid = wallet.id.unwrap();
        let address_repo = AddressRepository::new(db.connection());

        // Clear stale pending addresses (older than 10 days)
        if let Err(e) = address_repo.clear_stale_pending_addresses(PENDING_TIMEOUT_HOURS) {
            warn!("   Failed to clear stale pending addresses: {}", e);
        }

        let pending = address_repo.get_pending_utxo_check(wid)
            .map_err(|e| format!("Failed to get pending addresses: {}", e))?;

        (wid, pending)
    }; // DB lock dropped

    if addresses_to_sync.is_empty() {
        return Ok(());
    }

    info!("🔄 TaskSyncPending: syncing {} pending address(es)", addresses_to_sync.len());

    // Step 2: Convert to AddressInfo format for API call
    let address_infos: Vec<crate::json_storage::AddressInfo> = addresses_to_sync.iter()
        .map(|addr| crate::json_storage::AddressInfo {
            address: addr.address.clone(),
            index: addr.index,
            public_key: addr.public_key.clone(),
            used: addr.used,
            balance: addr.balance,
        })
        .collect();

    // Step 3: Fetch UTXOs from WhatsOnChain (NO DB lock held)
    let api_utxos = match crate::utxo_fetcher::fetch_all_utxos(&address_infos).await {
        Ok(utxos) => utxos,
        Err(e) => {
            warn!("   TaskSyncPending: API fetch failed: {}", e);
            return Err(format!("API fetch failed: {}", e));
        }
    };

    // Step 4: Process results (re-acquire DB lock)
    let mut new_utxo_count = 0u32;
    let mut reconciled_count = 0u32;
    {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let address_repo = AddressRepository::new(db.connection());
        let output_repo = OutputRepository::new(db.connection());

        for addr in &addresses_to_sync {
            if let Some(addr_id) = addr.id {
                let addr_utxos: Vec<_> = api_utxos.iter()
                    .filter(|u| u.address_index == addr.index)
                    .collect();

                if !addr_utxos.is_empty() {
                    for utxo in &addr_utxos {
                        match output_repo.upsert_received_utxo(
                            state.current_user_id,
                            &utxo.txid,
                            utxo.vout,
                            utxo.satoshis,
                            &utxo.script,
                            addr.index,
                        ) {
                            Ok(1) => new_utxo_count += 1,
                            Ok(_) => {}
                            Err(e) => warn!("   Failed to insert output {}:{}: {}", utxo.txid, utxo.vout, e),
                        }
                    }
                    let _ = address_repo.mark_used(addr_id);
                }

                // Reconcile stale outputs
                let derivation_prefix = "2-receive address";
                let derivation_suffix = addr.index.to_string();
                let owned_utxos: Vec<crate::utxo_fetcher::UTXO> = addr_utxos.iter()
                    .map(|u| (*u).clone())
                    .collect();

                match output_repo.reconcile_for_derivation(
                    state.current_user_id,
                    Some(derivation_prefix),
                    Some(&derivation_suffix),
                    &owned_utxos,
                    RECONCILE_GRACE_PERIOD_SECS,
                ) {
                    Ok(stale) if stale > 0 => {
                        info!("   🔄 Reconciled {} stale output(s) for address {}", stale, addr.address);
                        reconciled_count += stale as u32;
                    }
                    Ok(_) => {}
                    Err(e) => warn!("   ⚠️  Failed to reconcile outputs for {}: {}", addr.address, e),
                }

                // Only clear pending flag if UTXOs were found for this address.
                // If no UTXOs found, keep checking until 10-day stale timeout.
                if addr.pending_utxo_check && !addr_utxos.is_empty() {
                    let _ = address_repo.clear_pending_utxo_check(addr_id);
                }
            }
        }
    } // DB lock dropped

    // Step 5: Invalidate balance cache if anything changed
    if new_utxo_count > 0 || reconciled_count > 0 {
        state.balance_cache.invalidate();
        if new_utxo_count > 0 {
            info!("   💰 TaskSyncPending: inserted {} new output(s)", new_utxo_count);
        }
        if reconciled_count > 0 {
            info!("   🔄 TaskSyncPending: reconciled {} stale output(s)", reconciled_count);
        }
    }

    Ok(())
}
