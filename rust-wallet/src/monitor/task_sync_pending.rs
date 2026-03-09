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
use log::{info, warn, debug};

use crate::AppState;
use crate::database::{AddressRepository, OutputRepository, PeerPayRepository, WalletRepository,
    ParentTransactionRepository};

/// Grace period: don't mark outputs as externally spent if created < 10 min ago
const RECONCILE_GRACE_PERIOD_SECS: i64 = 600;

/// Stale pending addresses older than this are cleared without checking
const PENDING_TIMEOUT_HOURS: i64 = 2160; // 90 days

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

    // Snapshot current BSV/USD price for historical display
    let price_usd_cents = state.price_cache.get_cached()
        .or_else(|| state.price_cache.get_stale())
        .map(|p| (p * 100.0) as i64);

    // Step 4: Process results (re-acquire DB lock)
    let mut new_utxo_count = 0u32;
    let mut reconciled_count = 0u32;
    let mut new_txids: Vec<String> = Vec::new(); // Collect txids that need parent tx caching
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
                            Ok(1) => {
                                new_utxo_count += 1;
                                // Record notification for the new UTXO
                                if let Err(e) = PeerPayRepository::insert_address_sync_notification(
                                    db.connection(),
                                    &utxo.txid,
                                    utxo.vout as i64,
                                    utxo.satoshis,
                                    price_usd_cents,
                                ) {
                                    warn!("   Failed to record address sync notification: {}", e);
                                }
                                // Track txid for parent tx caching (done after DB lock drop)
                                if !new_txids.contains(&utxo.txid) {
                                    // Only cache if not already in parent_transactions
                                    let parent_tx_repo = ParentTransactionRepository::new(db.connection());
                                    if parent_tx_repo.get_by_txid(&utxo.txid).ok().flatten().is_none() {
                                        new_txids.push(utxo.txid.clone());
                                    }
                                }
                            }
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

                // Never clear pending flag on UTXO discovery — keep checking
                // for the full 90-day window. Users may reuse addresses or
                // senders may hold an address before paying. The stale timeout
                // in clear_stale_pending_addresses() handles expiry.
            }
        }
    } // DB lock dropped

    // Step 5: Cache parent transaction data for newly discovered UTXOs
    // This pre-populates the parent_transactions table so BEEF building
    // doesn't need to fetch from WhatsOnChain API during send.
    // Network calls are done outside DB lock (lock discipline).
    if !new_txids.is_empty() {
        info!("   📦 Caching parent tx data for {} new transaction(s)...", new_txids.len());
        let client = reqwest::Client::new();
        for txid in &new_txids {
            // Fetch raw transaction hex from WhatsOnChain
            let tx_url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/hex", txid);
            match client.get(&tx_url).send().await {
                Ok(response) if response.status().is_success() => {
                    match response.text().await {
                        Ok(raw_hex) => {
                            // Cache in parent_transactions table
                            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
                            let parent_tx_repo = ParentTransactionRepository::new(db.connection());
                            match parent_tx_repo.upsert(None, txid, &raw_hex) {
                                Ok(_) => debug!("   💾 Cached parent tx {}", txid),
                                Err(e) => warn!("   ⚠️  Failed to cache parent tx {}: {}", txid, e),
                            }
                        }
                        Err(e) => warn!("   ⚠️  Failed to read parent tx response for {}: {}", txid, e),
                    }
                }
                Ok(response) => warn!("   ⚠️  WoC returned {} for tx {}", response.status(), txid),
                Err(e) => warn!("   ⚠️  Failed to fetch parent tx {}: {}", txid, e),
            }
        }
    }

    // Step 6: Invalidate balance cache if anything changed
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
