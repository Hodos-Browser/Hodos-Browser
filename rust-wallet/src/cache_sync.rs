//! Background cache synchronization service
//!
//! Periodically pre-fetches and caches parent transactions, Merkle proofs, and block headers
//! for confirmed UTXOs to improve BEEF building performance.

use tokio::time::{sleep, Duration};
use crate::database::*;
use crate::cache_helpers;
use crate::cache_errors::{CacheResult, CacheError};
use actix_web::web;
use crate::AppState;
use reqwest::Client;
use serde_json::Value;

/// Background service to populate BEEF cache
pub async fn start_cache_sync_service(state: web::Data<AppState>) {
    let client = Client::new();

    loop {
        // Run every 10 minutes (configurable)
        sleep(Duration::from_secs(600)).await;

        log::info!("🔄 Starting BEEF cache sync...");

        match sync_cache_for_confirmed_utxos(&state, &client).await {
            Ok(count) => {
                log::info!("✅ Cache sync complete: {} proofs cached", count);
            }
            Err(e) => {
                log::error!("❌ Cache sync failed: {}", e);
            }
        }
    }
}

/// Sync cache for confirmed UTXOs that don't have proofs yet
async fn sync_cache_for_confirmed_utxos(
    state: &web::Data<AppState>,
    client: &Client,
) -> CacheResult<usize> {
    let mut cached_count = 0;
    const BATCH_SIZE: usize = 50;  // Limit to avoid rate limits

    // Get UTXOs without cached proofs
    let utxos_to_sync: Vec<(String, u32, Option<i64>)> = {
        let db = state.database.lock().unwrap();
        let mut stmt = db.connection().prepare(
            "SELECT DISTINCT u.txid, u.vout, u.id
             FROM utxos u
             LEFT JOIN parent_transactions pt ON pt.txid = u.txid
             LEFT JOIN merkle_proofs mp ON mp.parent_txn_id = pt.id
             WHERE u.is_spent = 0 AND mp.id IS NULL
             LIMIT ?"
        )?;

        let rows = stmt.query_map([BATCH_SIZE], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        result
    };

    log::info!("   Found {} UTXOs to sync", utxos_to_sync.len());

    // For each UTXO:
    for (txid, vout, utxo_id) in utxos_to_sync {
        // 1. Fetch parent transaction (if not cached)
        let needs_parent_tx = {
            let db = state.database.lock().unwrap();
            let parent_tx_repo = ParentTransactionRepository::new(db.connection());
            parent_tx_repo.get_by_txid(&txid)?.is_none()
        }; // db and parent_tx_repo dropped here

        if needs_parent_tx {
            match cache_helpers::fetch_parent_transaction_from_api(client, &txid).await {
                Ok(parent_tx_hex) => {
                    let parent_tx_bytes = hex::decode(&parent_tx_hex)?;
                    cache_helpers::verify_txid(&parent_tx_bytes, &txid)?;

                    // Lock again to cache
                    {
                        let db = state.database.lock().unwrap();
                        let parent_tx_repo = ParentTransactionRepository::new(db.connection());
                        parent_tx_repo.upsert(utxo_id, &txid, &parent_tx_hex)?;
                        log::debug!("   💾 Cached parent tx {}", txid);
                    }
                }
                Err(e) => {
                    log::warn!("   ⚠️  Failed to fetch parent tx {}: {}", txid, e);
                    continue;
                }
            }
        }

        // 2. Fetch TSC proof (if transaction is confirmed)
        {
            let has_proof = {
                let db = state.database.lock().unwrap();
                let merkle_proof_repo = MerkleProofRepository::new(db.connection());
                merkle_proof_repo.get_by_parent_txid(&txid)?.is_some()
            }; // db is dropped here when merkle_proof_repo goes out of scope

            if !has_proof {
                // Fetch TSC proof from API (without holding database lock)
                match cache_helpers::fetch_tsc_proof_from_api(client, &txid).await {
                    Ok(Some(tsc_json)) => {
                        // Check cache for block header first (synchronous, drop lock before await)
                        let target_hash = tsc_json["target"].as_str().unwrap_or("");
                        let (cached_height, need_api_fetch) = {
                            let db = state.database.lock().unwrap();
                            let block_header_repo = BlockHeaderRepository::new(db.connection());
                            match block_header_repo.get_by_hash(target_hash) {
                                Ok(Some(header)) => (Some(header.height), false),
                                Ok(None) => (None, true),
                                Err(_) => (None, true),
                            }
                        }; // db lock dropped here

                        // Enhance TSC proof with block height
                        let enhanced_tsc = if let Some(height) = cached_height {
                            // Use cached height
                            let mut enhanced = tsc_json.clone();
                            enhanced["height"] = serde_json::json!(height);
                            enhanced
                        } else if need_api_fetch {
                            // Fetch from API (async, no lock held)
                            let block_header_url = format!("https://api.whatsonchain.com/v1/bsv/main/block/hash/{}", target_hash);
                            let response = client.get(&block_header_url).send().await
                                .map_err(|e| CacheError::Api(format!("Failed to fetch block header: {}", e)))?;

                            if !response.status().is_success() {
                                log::warn!("   ⚠️  Block header API returned status {} for {}", response.status(), target_hash);
                                continue;
                            }

                            let header_json: Value = response.json().await
                                .map_err(|e| CacheError::Api(format!("Failed to parse block header JSON: {}", e)))?;

                            let height = header_json["height"].as_u64()
                                .ok_or_else(|| CacheError::InvalidData("Missing height in block header".to_string()))? as u32;

                            // Cache the header (lock again for writing)
                            {
                                let db = state.database.lock().unwrap();
                                let block_header_repo = BlockHeaderRepository::new(db.connection());
                                let header_hex = header_json["header"].as_str().unwrap_or("");
                                if let Err(e) = block_header_repo.upsert(target_hash, height, header_hex) {
                                    log::warn!("   ⚠️  Failed to cache block header: {}", e);
                                }
                            } // db lock dropped

                            // Enhance TSC proof
                            let mut enhanced = tsc_json.clone();
                            enhanced["height"] = serde_json::json!(height);
                            enhanced
                        } else {
                            // Should not reach here
                            continue;
                        };

                        // Cache the proof (lock again for writing)
                        {
                            let db = state.database.lock().unwrap();
                            let parent_tx_repo = ParentTransactionRepository::new(db.connection());
                            if let Some(parent_txn_id) = parent_tx_repo.get_id_by_txid(&txid)? {
                                let target_hash = enhanced_tsc["target"].as_str().unwrap_or("");
                                let nodes_json = serde_json::to_string(&enhanced_tsc["nodes"])?;
                                let block_height = enhanced_tsc["height"].as_u64().unwrap_or(0) as u32;
                                let tx_index = enhanced_tsc["index"].as_u64().unwrap_or(0);

                                let merkle_proof_repo = MerkleProofRepository::new(db.connection());
                                merkle_proof_repo.upsert(parent_txn_id, block_height, tx_index, target_hash, &nodes_json)?;
                                cached_count += 1;
                                log::debug!("   💾 Cached Merkle proof for {}", txid);
                            }
                        }
                    }
                    Ok(None) => {
                        // Transaction not confirmed yet, skip
                    }
                    Err(e) => {
                        log::warn!("   ⚠️  Failed to fetch TSC proof for {}: {}", txid, e);
                    }
                }
            }
        }

        // Rate limiting: small delay between requests
        sleep(Duration::from_millis(100)).await;
    }

    Ok(cached_count)
}
