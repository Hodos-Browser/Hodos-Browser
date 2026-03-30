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

/// Unconfirmed UTXOs older than this are considered failed (never mined)
const UNCONFIRMED_TIMEOUT_SECS: i64 = 30 * 60; // 30 minutes

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

    // Step 3: Fetch confirmed UTXOs from WhatsOnChain bulk API (NO DB lock held)
    let api_utxos = match crate::utxo_fetcher::fetch_all_utxos(&address_infos).await {
        Ok(utxos) => utxos,
        Err(e) => {
            warn!("   TaskSyncPending: API fetch failed: {}", e);
            return Err(format!("API fetch failed: {}", e));
        }
    };

    // Step 3b: Supplementary single-address fetch to catch unconfirmed (mempool) UTXOs.
    // The bulk API only returns confirmed UTXOs. The single-address API includes
    // unconfirmed UTXOs with height=0, letting us show receives immediately.
    let confirmed_set: std::collections::HashSet<(String, u32)> = api_utxos.iter()
        .map(|u| (u.txid.clone(), u.vout))
        .collect();

    let mut unconfirmed_utxos: Vec<crate::utxo_fetcher::UTXO> = Vec::new();
    for addr in &addresses_to_sync {
        match crate::utxo_fetcher::fetch_utxos_single_address_with_unconfirmed(&addr.address, addr.index).await {
            Ok(all_utxos) => {
                for utxo in all_utxos {
                    if !utxo.confirmed && !confirmed_set.contains(&(utxo.txid.clone(), utxo.vout)) {
                        unconfirmed_utxos.push(utxo);
                    }
                }
            }
            Err(e) => {
                debug!("   Single-address fetch failed for {}: {}", addr.address, e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    if !unconfirmed_utxos.is_empty() {
        info!("   🆕 Found {} unconfirmed UTXO(s) in mempool", unconfirmed_utxos.len());
    }

    // Snapshot current BSV/USD price for historical display
    let price_usd_cents = state.price_cache.get_cached()
        .or_else(|| state.price_cache.get_stale())
        .map(|p| (p * 100.0) as i64);

    // Step 4: Process results (re-acquire DB lock)
    let mut new_utxo_count = 0u32;
    let mut new_sats_total: i64 = 0;
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
                                new_sats_total += utxo.satoshis;
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
                            Ok(_) => {
                                // Output already existed — if it was unconfirmed, upgrade to confirmed
                                let _ = output_repo.mark_output_confirmed(&utxo.txid, utxo.vout as i32);
                            }
                            Err(e) => warn!("   Failed to insert output {}:{}: {}", utxo.txid, utxo.vout, e),
                        }
                    }
                    let _ = address_repo.mark_used(addr_id);
                }

                // Insert unconfirmed UTXOs for this address
                let addr_unconfirmed: Vec<_> = unconfirmed_utxos.iter()
                    .filter(|u| u.address_index == addr.index)
                    .collect();
                for utxo in &addr_unconfirmed {
                    match output_repo.upsert_received_utxo_with_confirmed(
                        state.current_user_id,
                        &utxo.txid,
                        utxo.vout,
                        utxo.satoshis,
                        &utxo.script,
                        addr.index,
                        false, // unconfirmed
                    ) {
                        Ok(1) => {
                            new_utxo_count += 1;
                            new_sats_total += utxo.satoshis;
                            // Record notification for unconfirmed receive
                            if let Err(e) = PeerPayRepository::insert_address_sync_notification(
                                db.connection(),
                                &utxo.txid,
                                utxo.vout as i64,
                                utxo.satoshis,
                                price_usd_cents,
                            ) {
                                warn!("   Failed to record unconfirmed notification: {}", e);
                            }
                            if !new_txids.contains(&utxo.txid) {
                                let parent_tx_repo = ParentTransactionRepository::new(db.connection());
                                if parent_tx_repo.get_by_txid(&utxo.txid).ok().flatten().is_none() {
                                    new_txids.push(utxo.txid.clone());
                                }
                            }
                        }
                        Ok(_) => {} // Already exists
                        Err(e) => warn!("   Failed to insert unconfirmed output {}:{}: {}", utxo.txid, utxo.vout, e),
                    }
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

    // Step 6: Check for stale unconfirmed outputs (> 30 min without confirmation)
    // If a received UTXO was only seen in mempool and never confirmed, create a
    // failure notification and remove the output.
    let mut failed_unconfirmed = 0u32;
    {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let output_repo = OutputRepository::new(db.connection());

        let stale = output_repo.get_stale_unconfirmed(state.current_user_id, UNCONFIRMED_TIMEOUT_SECS)
            .map_err(|e| format!("Failed to get stale unconfirmed: {}", e))?;

        for (txid, vout, satoshis) in &stale {
            // Insert red failure notification
            if let Err(e) = PeerPayRepository::insert_failure_notification(
                db.connection(), txid, *vout as i64, *satoshis, price_usd_cents,
            ) {
                warn!("   Failed to insert failure notification for {}:{}: {}", txid, vout, e);
            }

            // Auto-dismiss the corresponding green receive notification
            let _ = PeerPayRepository::dismiss_by_txid_prefix(db.connection(), txid);

            // Remove the unconfirmed output
            if let Err(e) = output_repo.delete_unconfirmed_output(txid, *vout) {
                warn!("   Failed to delete unconfirmed output {}:{}: {}", txid, vout, e);
            }
            failed_unconfirmed += 1;
            warn!("   🔴 Unconfirmed output {}:{} ({} sats) failed after {}min timeout",
                  &txid[..std::cmp::min(16, txid.len())], vout, satoshis, UNCONFIRMED_TIMEOUT_SECS / 60);
        }
    } // DB lock dropped

    // Step 7: Invalidate balance cache if anything changed
    if new_utxo_count > 0 || reconciled_count > 0 || failed_unconfirmed > 0 {
        state.balance_cache.invalidate();
        if new_utxo_count > 0 {
            info!("   💰 TaskSyncPending: inserted {} new output(s)", new_utxo_count);
            // Request backup check if received amount is significant (> $3 USD)
            state.request_backup_check_if_significant(new_sats_total);
        }
        if reconciled_count > 0 {
            info!("   🔄 TaskSyncPending: reconciled {} stale output(s)", reconciled_count);
        }
        if failed_unconfirmed > 0 {
            warn!("   🔴 TaskSyncPending: {} unconfirmed output(s) failed to confirm", failed_unconfirmed);
        }
    }

    Ok(())
}
