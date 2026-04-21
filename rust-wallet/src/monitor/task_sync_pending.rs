//! TaskSyncPending — Periodic UTXO discovery for pending addresses
//!
//! Scans addresses with `pending_utxo_check = 1` against WhatsOnChain to
//! discover new incoming UTXOs. Discovery only — never marks existing
//! outputs as spent.
//!
//! Tiered checking by address age:
//! - Startup: check ALL pending addresses immediately (individual)
//! - Fresh (0-3 hours): every 30 seconds (individual, includes unconfirmed)
//! - Recent (3-18 hours): every 3 minutes (individual)
//! - Old (18+ hours): every 30 minutes (bulk, confirmed only)
//!
//! Interval: 30 seconds (task runs every tick, but only checks addresses
//! whose tier is due)

use actix_web::web;
use log::{info, warn, debug};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::AppState;
use crate::database::{AddressRepository, OutputRepository, PeerPayRepository, WalletRepository,
    ParentTransactionRepository};

/// Stale pending addresses older than this are cleared without checking
const PENDING_TIMEOUT_HOURS: i64 = 2160; // 90 days

/// Unconfirmed UTXOs older than this trigger a WoC verification check
const UNCONFIRMED_CHECK_SECS: i64 = 30 * 60; // 30 minutes

/// Tier thresholds (seconds since address creation)
const FRESH_THRESHOLD_SECS: i64 = 3 * 3600;   // 0-3 hours: check every tick (30s)
const RECENT_THRESHOLD_SECS: i64 = 18 * 3600;  // 3-18 hours: check every 3 minutes
const RECENT_CHECK_INTERVAL_SECS: u64 = 180;   // 3 minutes
const OLD_CHECK_INTERVAL_SECS: u64 = 1800;     // 30 minutes

/// First run flag — on startup, check all addresses immediately
static FIRST_RUN: AtomicBool = AtomicBool::new(true);

/// Track when we last checked recent/old tiers
static LAST_RECENT_CHECK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LAST_OLD_CHECK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Run the TaskSyncPending task
pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    let is_startup = FIRST_RUN.swap(false, Ordering::SeqCst);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Step 1: Get pending addresses (DB lock held briefly)
    let (wallet_id, all_pending) = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let wallet_repo = WalletRepository::new(db.connection());
        let wallet = wallet_repo.get_primary_wallet()
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or_else(|| "No wallet found".to_string())?;

        let wid = wallet.id.unwrap();
        let address_repo = AddressRepository::new(db.connection());

        // Clear stale pending addresses (older than 90 days)
        if let Err(e) = address_repo.clear_stale_pending_addresses(PENDING_TIMEOUT_HOURS) {
            warn!("   Failed to clear stale pending addresses: {}", e);
        }

        let pending = address_repo.get_pending_utxo_check(wid)
            .map_err(|e| format!("Failed to get pending addresses: {}", e))?;

        (wid, pending)
    }; // DB lock dropped

    if all_pending.is_empty() {
        return Ok(());
    }

    // Step 2: Determine which addresses to check this tick based on age tiers
    let now_i64 = now_secs as i64;
    let mut fresh_addresses = Vec::new();   // 0-3h: check every tick
    let mut recent_addresses = Vec::new();  // 3-18h: check every 3 min
    let mut old_addresses = Vec::new();     // 18h+: check every 30 min

    for addr in &all_pending {
        let age_secs = now_i64 - addr.created_at;
        if age_secs < FRESH_THRESHOLD_SECS {
            fresh_addresses.push(addr);
        } else if age_secs < RECENT_THRESHOLD_SECS {
            recent_addresses.push(addr);
        } else {
            old_addresses.push(addr);
        }
    }

    // On startup: check everything immediately (individual)
    if is_startup {
        info!("🔄 TaskSyncPending: startup sweep — checking all {} pending address(es) individually",
              all_pending.len());
        let addresses_to_check: Vec<_> = all_pending.iter().collect();
        check_addresses_individually(state, &addresses_to_check).await?;
        // Update tier timestamps so we don't re-check immediately
        LAST_RECENT_CHECK.store(now_secs, Ordering::Relaxed);
        LAST_OLD_CHECK.store(now_secs, Ordering::Relaxed);
        return Ok(());
    }

    // Fresh addresses: check every tick (30s), individually (includes unconfirmed)
    if !fresh_addresses.is_empty() {
        debug!("🔄 TaskSyncPending: checking {} fresh address(es) (0-3h)", fresh_addresses.len());
        check_addresses_individually(state, &fresh_addresses).await?;
    }

    // Recent addresses: check every 3 minutes, individually
    let last_recent = LAST_RECENT_CHECK.load(Ordering::Relaxed);
    if !recent_addresses.is_empty() && now_secs - last_recent >= RECENT_CHECK_INTERVAL_SECS {
        LAST_RECENT_CHECK.store(now_secs, Ordering::Relaxed);
        info!("🔄 TaskSyncPending: checking {} recent address(es) (3-18h)", recent_addresses.len());
        check_addresses_individually(state, &recent_addresses).await?;
    }

    // Old addresses: check every 30 minutes, bulk (confirmed only)
    let last_old = LAST_OLD_CHECK.load(Ordering::Relaxed);
    if !old_addresses.is_empty() && now_secs - last_old >= OLD_CHECK_INTERVAL_SECS {
        LAST_OLD_CHECK.store(now_secs, Ordering::Relaxed);
        info!("🔄 TaskSyncPending: checking {} old address(es) (18h+) via bulk", old_addresses.len());
        check_addresses_bulk(state, &old_addresses).await?;
    }

    // Check for stale unconfirmed outputs (> 30 min without confirmation)
    check_stale_unconfirmed(state).await?;

    Ok(())
}

/// Check addresses individually (includes unconfirmed UTXOs from mempool)
/// Used for fresh/recent addresses and startup sweep
async fn check_addresses_individually(
    state: &web::Data<AppState>,
    addresses: &[&crate::database::Address],
) -> Result<(), String> {
    let price_usd_cents = state.price_cache.get_cached()
        .or_else(|| state.price_cache.get_stale())
        .map(|p| (p * 100.0) as i64);

    let mut new_utxo_count = 0u32;
    let mut new_sats_total: i64 = 0;
    let mut new_txids: Vec<String> = Vec::new();

    for addr in addresses {
        // Fetch all UTXOs (confirmed + unconfirmed) for this address
        let utxos = match crate::utxo_fetcher::fetch_utxos_single_address_with_unconfirmed(
            &addr.address, addr.index
        ).await {
            Ok(u) => u,
            Err(e) => {
                debug!("   Single-address fetch failed for {}: {}", addr.address, e);
                continue;
            }
        };

        if utxos.is_empty() {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            continue;
        }

        // Insert new UTXOs into DB
        {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            let output_repo = OutputRepository::new(db.connection());
            let address_repo = AddressRepository::new(db.connection());

            for utxo in &utxos {
                let insert_result = if utxo.confirmed {
                    output_repo.upsert_received_utxo(
                        state.current_user_id, &utxo.txid, utxo.vout,
                        utxo.satoshis, &utxo.script, addr.index,
                    )
                } else {
                    output_repo.upsert_received_utxo_with_confirmed(
                        state.current_user_id, &utxo.txid, utxo.vout,
                        utxo.satoshis, &utxo.script, addr.index, false,
                    )
                };

                match insert_result {
                    Ok(1) => {
                        new_utxo_count += 1;
                        new_sats_total += utxo.satoshis;
                        if let Err(e) = PeerPayRepository::insert_address_sync_notification(
                            db.connection(), &utxo.txid, utxo.vout as i64,
                            utxo.satoshis, price_usd_cents,
                        ) {
                            warn!("   Failed to record notification: {}", e);
                        }
                        if !new_txids.contains(&utxo.txid) {
                            let parent_tx_repo = ParentTransactionRepository::new(db.connection());
                            if parent_tx_repo.get_by_txid(&utxo.txid).ok().flatten().is_none() {
                                new_txids.push(utxo.txid.clone());
                            }
                        }
                    }
                    Ok(_) => {
                        // Already existed — if confirmed now, upgrade
                        if utxo.confirmed {
                            let _ = output_repo.mark_output_confirmed(&utxo.txid, utxo.vout as i32);
                        }
                    }
                    Err(e) => warn!("   Failed to insert output {}:{}: {}", utxo.txid, utxo.vout, e),
                }
            }

            if let Some(addr_id) = addr.id {
                let _ = address_repo.mark_used(addr_id);
            }
        } // DB lock dropped

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Cache parent transactions for BEEF building
    cache_parent_transactions(state, &new_txids).await;

    if new_utxo_count > 0 {
        state.balance_cache.invalidate();
        info!("   💰 TaskSyncPending: inserted {} new output(s) ({} sats)",
              new_utxo_count, new_sats_total);
        state.request_backup_check_if_significant(new_sats_total);
    }

    Ok(())
}

/// Check addresses via bulk API (confirmed UTXOs only)
/// Used for old addresses where we don't need mempool visibility
async fn check_addresses_bulk(
    state: &web::Data<AppState>,
    addresses: &[&crate::database::Address],
) -> Result<(), String> {
    let address_infos: Vec<crate::json_storage::AddressInfo> = addresses.iter()
        .map(|addr| crate::json_storage::AddressInfo {
            address: addr.address.clone(),
            index: addr.index,
            public_key: addr.public_key.clone(),
            used: addr.used,
            balance: addr.balance,
        })
        .collect();

    let api_utxos = match crate::utxo_fetcher::fetch_all_utxos(&address_infos).await {
        Ok(utxos) => utxos,
        Err(e) => {
            warn!("   TaskSyncPending: bulk API fetch failed: {}", e);
            return Err(format!("Bulk API fetch failed: {}", e));
        }
    };

    let price_usd_cents = state.price_cache.get_cached()
        .or_else(|| state.price_cache.get_stale())
        .map(|p| (p * 100.0) as i64);

    let mut new_utxo_count = 0u32;
    let mut new_sats_total: i64 = 0;
    let mut new_txids: Vec<String> = Vec::new();

    {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let output_repo = OutputRepository::new(db.connection());
        let address_repo = AddressRepository::new(db.connection());

        for addr in addresses {
            let addr_utxos: Vec<_> = api_utxos.iter()
                .filter(|u| u.address_index == addr.index)
                .collect();

            if addr_utxos.is_empty() {
                continue;
            }

            for utxo in &addr_utxos {
                match output_repo.upsert_received_utxo(
                    state.current_user_id, &utxo.txid, utxo.vout,
                    utxo.satoshis, &utxo.script, addr.index,
                ) {
                    Ok(1) => {
                        new_utxo_count += 1;
                        new_sats_total += utxo.satoshis;
                        if let Err(e) = PeerPayRepository::insert_address_sync_notification(
                            db.connection(), &utxo.txid, utxo.vout as i64,
                            utxo.satoshis, price_usd_cents,
                        ) {
                            warn!("   Failed to record notification: {}", e);
                        }
                        if !new_txids.contains(&utxo.txid) {
                            let parent_tx_repo = ParentTransactionRepository::new(db.connection());
                            if parent_tx_repo.get_by_txid(&utxo.txid).ok().flatten().is_none() {
                                new_txids.push(utxo.txid.clone());
                            }
                        }
                    }
                    Ok(_) => {
                        let _ = output_repo.mark_output_confirmed(&utxo.txid, utxo.vout as i32);
                    }
                    Err(e) => warn!("   Failed to insert output {}:{}: {}", utxo.txid, utxo.vout, e),
                }
            }

            if let Some(addr_id) = addr.id {
                let _ = address_repo.mark_used(addr_id);
            }
        }
    } // DB lock dropped

    // No reconcile — discovery only. We never mark existing outputs as spent
    // based on absence from the bulk API response.

    cache_parent_transactions(state, &new_txids).await;

    if new_utxo_count > 0 {
        state.balance_cache.invalidate();
        info!("   ��� TaskSyncPending: inserted {} new output(s) ({} sats) via bulk",
              new_utxo_count, new_sats_total);
        state.request_backup_check_if_significant(new_sats_total);
    }

    Ok(())
}

/// Cache parent transaction raw hex for BEEF building
async fn cache_parent_transactions(state: &web::Data<AppState>, txids: &[String]) {
    if txids.is_empty() {
        return;
    }

    info!("   📦 Caching parent tx data for {} new transaction(s)...", txids.len());
    let client = reqwest::Client::new();

    for txid in txids {
        let tx_url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/hex", txid);
        match client.get(&tx_url).send().await {
            Ok(response) if response.status().is_success() => {
                match response.text().await {
                    Ok(raw_hex) => {
                        if let Ok(db) = state.database.lock() {
                            let parent_tx_repo = ParentTransactionRepository::new(db.connection());
                            match parent_tx_repo.upsert(None, txid, &raw_hex) {
                                Ok(_) => debug!("   💾 Cached parent tx {}", txid),
                                Err(e) => warn!("   ⚠️  Failed to cache parent tx {}: {}", txid, e),
                            }
                        }
                    }
                    Err(e) => warn!("   ⚠��  Failed to read parent tx response for {}: {}", txid, e),
                }
            }
            Ok(response) => warn!("   ⚠️  WoC returned {} for tx {}", response.status(), txid),
            Err(e) => warn!("   ⚠️  Failed to fetch parent tx {}: {}", txid, e),
        }
    }
}

/// Check for stale unconfirmed outputs (> 30 min without confirmation)
/// Before deleting, verify with WoC whether the tx is actually gone from mempool
/// or just slow to confirm (e.g., large backup txs with minimum fee rate).
async fn check_stale_unconfirmed(state: &web::Data<AppState>) -> Result<(), String> {
    let price_usd_cents = state.price_cache.get_cached()
        .or_else(|| state.price_cache.get_stale())
        .map(|p| (p * 100.0) as i64);

    // Read stale candidates from DB (brief lock)
    let stale_candidates: Vec<(String, u32, i64)> = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let output_repo = OutputRepository::new(db.connection());
        output_repo.get_stale_unconfirmed(state.current_user_id, UNCONFIRMED_CHECK_SECS)
            .unwrap_or_default()
    }; // DB lock dropped

    if stale_candidates.is_empty() {
        return Ok(());
    }

    // Deduplicate by txid
    let unique_txids: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        stale_candidates.iter()
            .filter(|(txid, _, _)| seen.insert(txid.clone()))
            .map(|(txid, _, _)| txid.clone())
            .collect()
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut confirmed_txids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut still_pending_txids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for txid in &unique_txids {
        let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/hash/{}", txid);
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(body) => {
                        let confirmations = body.get("confirmations")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        if confirmations > 0 {
                            info!("   ✅ Stale tx {}... is now confirmed ({} confirmations)",
                                &txid[..std::cmp::min(16, txid.len())], confirmations);
                            confirmed_txids.insert(txid.clone());
                        } else {
                            info!("   ⏳ Stale tx {}... still in mempool — not deleting",
                                &txid[..std::cmp::min(16, txid.len())]);
                            still_pending_txids.insert(txid.clone());
                        }
                    }
                    Err(_) => {
                        still_pending_txids.insert(txid.clone());
                    }
                }
            }
            Ok(resp) if resp.status().as_u16() == 404 => {
                warn!("   ❌ Stale tx {}... not found on WoC (404) — confirmed failed",
                    &txid[..std::cmp::min(16, txid.len())]);
            }
            Ok(resp) => {
                warn!("   ⚠️  WoC returned {} for tx {}... — skipping",
                    resp.status(), &txid[..std::cmp::min(16, txid.len())]);
                still_pending_txids.insert(txid.clone());
            }
            Err(e) => {
                warn!("   ⚠️  WoC check failed for tx {}...: {} — skipping",
                    &txid[..std::cmp::min(16, txid.len())], e);
                still_pending_txids.insert(txid.clone());
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    // Process results
    let mut failed_unconfirmed = 0u32;
    {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let output_repo = OutputRepository::new(db.connection());

        for (txid, vout, satoshis) in &stale_candidates {
            if confirmed_txids.contains(txid) {
                let _ = output_repo.mark_output_confirmed(txid, *vout as i32);
            } else if still_pending_txids.contains(txid) {
                // Still in mempool — leave it alone
            } else {
                // Truly failed (404 from WoC) — delete and notify
                if let Err(e) = PeerPayRepository::insert_failure_notification(
                    db.connection(), txid, *vout as i64, *satoshis, price_usd_cents,
                ) {
                    warn!("   Failed to insert failure notification for {}:{}: {}", txid, vout, e);
                }
                let _ = PeerPayRepository::dismiss_by_txid_prefix(db.connection(), txid);
                if let Err(e) = output_repo.delete_unconfirmed_output(txid, *vout) {
                    warn!("   Failed to delete unconfirmed output {}:{}: {}", txid, vout, e);
                }
                failed_unconfirmed += 1;
                warn!("   🔴 Unconfirmed output {}:{} ({} sats) confirmed failed — tx dropped from mempool",
                      &txid[..std::cmp::min(16, txid.len())], vout, satoshis);
            }
        }
    } // DB lock dropped

    if failed_unconfirmed > 0 {
        state.balance_cache.invalidate();
        warn!("   🔴 TaskSyncPending: {} unconfirmed output(s) failed to confirm", failed_unconfirmed);
    }

    Ok(())
}
