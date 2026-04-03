//! TaskValidateUtxos — Periodic full UTXO validation against blockchain
//!
//! Validates ALL spendable outputs against WhatsOnChain, not just addresses
//! flagged with `pending_utxo_check`. This catches stale UTXOs that were spent
//! on-chain but our DB still thinks are spendable — e.g., when a broadcast
//! appeared to fail but actually mined, or when an external wallet spent from
//! a shared address.
//!
//! Runs on first tick (startup) then every 10 minutes.
//!
//! Strategy:
//! 1. Get all distinct addresses that have spendable outputs
//! 2. For each address, fetch current UTXOs from WhatsOnChain
//! 3. Any DB output NOT found in the API response → mark as `external-spend`
//!
//! Interval: 600 seconds (10 minutes)

use actix_web::web;
use log::{info, warn};
use std::collections::HashMap;

use crate::AppState;
use crate::database::{AddressRepository, OutputRepository, WalletRepository};

/// Grace period: don't invalidate outputs newer than 5 minutes.
/// Longer than before to avoid marking valid outputs as stale when
/// broadcast propagation is slow or API caches are stale.
const VALIDATE_GRACE_PERIOD_SECS: i64 = 300;

/// Maximum addresses to validate per cycle (rate limiting for WoC API)
const MAX_ADDRESSES_PER_CYCLE: usize = 50;

/// Run the TaskValidateUtxos task
pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    // Step 1: Get all addresses that have spendable outputs (DB lock held briefly)
    let addresses_with_spendable: Vec<AddressWithDerivation> = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let conn = db.connection();

        // Step A: Get distinct (derivation_prefix, derivation_suffix) pairs from spendable P2PKH outputs.
        // Skip PushDrop/identity outputs (derivation_prefix like '%-identity%') — those are custom
        // scripts that can't be validated by address UTXO lookup.
        let mut stmt = conn.prepare(
            "SELECT DISTINCT derivation_prefix, derivation_suffix
             FROM outputs
             WHERE spendable = 1 AND user_id = ?1
               AND txid IS NOT NULL
               AND derivation_prefix IS NOT NULL
               AND derivation_suffix IS NOT NULL
               AND derivation_prefix IN ('2-receive address', 'bip32', 'master')"
        ).map_err(|e| format!("Query prepare error: {}", e))?;

        let derivation_pairs: Vec<(String, String)> = stmt.query_map(
            rusqlite::params![state.current_user_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ).map_err(|e| format!("Query error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

        if derivation_pairs.is_empty() {
            info!("🔍 TaskValidateUtxos: no spendable outputs with derivation info — skipping");
            return Ok(());
        }

        // Step B: Look up addresses for each derivation suffix (index)
        let wallet_repo = WalletRepository::new(conn);
        let wallet = wallet_repo.get_primary_wallet()
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or_else(|| "No wallet found".to_string())?;
        let wallet_id = wallet.id.unwrap();

        let address_repo = AddressRepository::new(conn);
        let mut results = Vec::new();

        for (prefix, suffix) in &derivation_pairs {
            let index: i32 = match suffix.parse() {
                Ok(i) => i,
                Err(_) => continue, // Skip non-numeric suffixes
            };

            if let Ok(Some(addr)) = address_repo.get_by_wallet_and_index(wallet_id, index) {
                results.push(AddressWithDerivation {
                    address: addr.address,
                    index,
                    derivation_prefix: prefix.clone(),
                });
            }
        }

        results
    }; // DB lock dropped

    if addresses_with_spendable.is_empty() {
        info!("🔍 TaskValidateUtxos: no addresses found for spendable outputs — skipping");
        return Ok(());
    }

    // Limit to prevent overwhelming the API
    let addresses_to_check: Vec<_> = addresses_with_spendable.into_iter()
        .take(MAX_ADDRESSES_PER_CYCLE)
        .collect();

    info!("🔍 TaskValidateUtxos: validating {} address(es) with spendable outputs",
          addresses_to_check.len());

    // Step 2: Group addresses for batch API fetch
    let address_infos: Vec<crate::json_storage::AddressInfo> = addresses_to_check.iter()
        .map(|a| crate::json_storage::AddressInfo {
            address: a.address.clone(),
            index: a.index,
            public_key: String::new(), // Not needed for UTXO fetch
            used: true,
            balance: 0,
        })
        .collect();

    // Step 3: Fetch UTXOs from WhatsOnChain (NO DB lock held)
    let api_utxos = match crate::utxo_fetcher::fetch_all_utxos(&address_infos).await {
        Ok(utxos) => utxos,
        Err(e) => {
            warn!("   TaskValidateUtxos: API fetch failed: {}", e);
            return Err(format!("API fetch failed: {}", e));
        }
    };

    // Build lookup: address_index → Vec<(txid, vout)>
    let mut api_utxo_map: HashMap<i32, Vec<crate::utxo_fetcher::UTXO>> = HashMap::new();
    for utxo in &api_utxos {
        api_utxo_map.entry(utxo.address_index)
            .or_default()
            .push(utxo.clone());
    }

    // Step 4: Reconcile (re-acquire DB lock)
    let mut total_stale = 0u32;
    {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let output_repo = OutputRepository::new(db.connection());

        // Build set of addresses that have in-flight (sending/unsigned) transactions.
        // Don't reconcile these — the outputs may not have propagated to the API yet.
        let inflight_addresses: std::collections::HashSet<i32> = {
            let mut stmt = db.connection().prepare(
                "SELECT DISTINCT o.derivation_suffix
                 FROM outputs o
                 INNER JOIN transactions t ON o.transaction_id = t.id
                 WHERE t.status IN ('sending', 'unsigned', 'unprocessed')
                   AND o.derivation_suffix IS NOT NULL"
            ).unwrap_or_else(|_| db.connection().prepare("SELECT 1 WHERE 0").unwrap());
            stmt.query_map([], |row| {
                let suffix: String = row.get(0)?;
                Ok(suffix.parse::<i32>().unwrap_or(-999))
            }).ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        };

        for addr in &addresses_to_check {
            // Skip addresses with in-flight transactions
            if inflight_addresses.contains(&addr.index) {
                continue;
            }

            let addr_utxos = api_utxo_map.get(&addr.index)
                .cloned()
                .unwrap_or_default();

            // Safety: never reconcile against an empty API response.
            // An empty response for an address we KNOW has spendable outputs
            // likely means the API returned an error or the address format is wrong.
            if addr_utxos.is_empty() {
                continue;
            }

            let derivation_suffix = addr.index.to_string();

            match output_repo.reconcile_for_derivation(
                state.current_user_id,
                Some(&addr.derivation_prefix),
                Some(&derivation_suffix),
                &addr_utxos,
                VALIDATE_GRACE_PERIOD_SECS,
            ) {
                Ok(stale) if stale > 0 => {
                    info!("   🔍 TaskValidateUtxos: {} stale output(s) at {} (index {})",
                          stale, addr.address, addr.index);
                    total_stale += stale as u32;
                }
                Ok(_) => {}
                Err(e) => warn!("   ⚠️  TaskValidateUtxos: reconcile error for {}: {}", addr.address, e),
            }

            // Also reconcile BIP32 outputs at the same index
            // (same address can have both BRC-42 and BIP32 derivation paths)
            if addr.derivation_prefix == "2-receive address" {
                match output_repo.reconcile_for_derivation(
                    state.current_user_id,
                    Some("bip32"),
                    Some(&derivation_suffix),
                    &addr_utxos,
                    VALIDATE_GRACE_PERIOD_SECS,
                ) {
                    Ok(stale) if stale > 0 => {
                        info!("   🔍 TaskValidateUtxos: {} stale BIP32 output(s) at index {}",
                              stale, addr.index);
                        total_stale += stale as u32;
                    }
                    Ok(_) => {}
                    Err(e) => warn!("   ⚠️  TaskValidateUtxos: BIP32 reconcile error for index {}: {}",
                                    addr.index, e),
                }
            }
        }
    } // DB lock dropped

    // Step 5: Invalidate balance cache if anything changed
    if total_stale > 0 {
        state.balance_cache.invalidate();
        info!("   🔍 TaskValidateUtxos: invalidated {} stale output(s) total", total_stale);
    }

    Ok(())
}

/// Internal struct for tracking addresses with their derivation info
struct AddressWithDerivation {
    address: String,
    index: i32,
    derivation_prefix: String,
}
