//! Helper functions for building BEEF structures
//!
//! Provides functions for fetching transactions and recursively building BEEF
//! for listOutputs when include='entire transactions' is requested.

use crate::beef::{Beef, ParsedTransaction};
use crate::database::{WalletDatabase, TransactionRepository, ParentTransactionRepository, MerkleProofRepository, BlockHeaderRepository};
use crate::cache_helpers::{fetch_parent_transaction_from_api, fetch_tsc_proof_from_api, enhance_tsc_with_height};
use reqwest::Client;
use std::sync::Mutex;
use sha2::{Sha256, Digest};

/// Fetch a transaction by TXID from cache or API
///
/// Checks in order:
/// 1. `transactions` table (for wallet's own transactions)
/// 2. `parent_transactions` table (cached parent transactions)
/// 3. WhatsOnChain API (if not cached)
///
/// Returns raw transaction bytes
pub async fn fetch_transaction_for_beef(
    txid: &str,
    db: &Mutex<WalletDatabase>,
    client: &Client,
) -> Result<Vec<u8>, String> {
    let db_guard = db.lock().unwrap();
    let conn = db_guard.connection();

    // Step 1: Check transactions table (wallet's own transactions)
    let tx_repo = TransactionRepository::new(conn);
    if let Ok(Some(stored_action)) = tx_repo.get_by_txid(txid) {
        let raw_tx_hex = stored_action.raw_tx;
        if !raw_tx_hex.is_empty() {
            drop(db_guard); // Release lock before hex decode
            match hex::decode(&raw_tx_hex) {
                Ok(bytes) => {
                    // Verify TXID matches
                    let hash1 = Sha256::digest(&bytes);
                    let hash2 = Sha256::digest(&hash1);
                    let calculated_txid: Vec<u8> = hash2.into_iter().rev().collect();
                    let calculated_txid_hex = hex::encode(calculated_txid);

                    if calculated_txid_hex == txid {
                        log::info!("   ✅ Found transaction {} in transactions table", txid);
                        return Ok(bytes);
                    } else {
                        log::warn!("   ⚠️  TXID mismatch for {} in transactions table, trying cache...", txid);
                    }
                },
                Err(e) => {
                    log::warn!("   ⚠️  Failed to decode transaction {} hex: {}, trying cache...", txid, e);
                },
            }
        }
    }

    // Step 2: Check parent_transactions table (cached parent transactions)
    {
        let db_guard = db.lock().unwrap();
        let conn = db_guard.connection();
        let parent_tx_repo = ParentTransactionRepository::new(conn);

        match parent_tx_repo.get_by_txid(txid) {
            Ok(Some(cached)) => {
                // Verify cached data
                match parent_tx_repo.verify_txid(txid, &cached.raw_hex) {
                    Ok(true) => {
                        log::info!("   ✅ Found transaction {} in parent_transactions cache", txid);
                        drop(db_guard); // Release lock before hex decode
                        match hex::decode(&cached.raw_hex) {
                            Ok(bytes) => return Ok(bytes),
                            Err(e) => {
                                log::warn!("   ⚠️  Failed to decode cached transaction {}: {}, fetching from API", txid, e);
                            },
                        }
                    },
                    Ok(false) => {
                        log::warn!("   ⚠️  Cached transaction {} failed TXID verification, fetching from API", txid);
                    },
                    Err(e) => {
                        log::warn!("   ⚠️  Error verifying cached transaction {}: {}, fetching from API", txid, e);
                    },
                }
            },
            Ok(None) => {
                log::info!("   🌐 Cache miss - fetching transaction {} from API...", txid);
            },
            Err(e) => {
                log::warn!("   ⚠️  Database error checking cache: {}, fetching from API", e);
            },
        }
    } // db_guard is dropped here

    // Step 3: Fetch from API
    match fetch_parent_transaction_from_api(client, txid).await {
        Ok(parent_tx_hex) => {
            match hex::decode(&parent_tx_hex) {
                Ok(bytes) => {
                    // Verify TXID matches
                    let hash1 = Sha256::digest(&bytes);
                    let hash2 = Sha256::digest(&hash1);
                    let calculated_txid: Vec<u8> = hash2.into_iter().rev().collect();
                    let calculated_txid_hex = hex::encode(calculated_txid);

                    if calculated_txid_hex == txid {
                        // Cache it for future use
                        {
                            let db_guard = db.lock().unwrap();
                            let conn = db_guard.connection();
                            let parent_tx_repo = ParentTransactionRepository::new(conn);
                            // Try to get UTXO ID (optional, may not exist)
                            let utxo_id = None; // We don't have UTXO ID here, but that's okay
                            let _ = parent_tx_repo.upsert(utxo_id, txid, &parent_tx_hex);
                        }
                        log::info!("   💾 Cached transaction {} from API", txid);
                        Ok(bytes)
                    } else {
                        Err(format!("TXID verification failed for {}: calculated {} but expected {}", txid, calculated_txid_hex, txid))
                    }
                },
                Err(e) => {
                    Err(format!("Failed to decode transaction hex for {}: {}", txid, e))
                },
            }
        },
        Err(e) => {
            Err(format!("Failed to fetch transaction {} from API: {}", txid, e))
        },
    }
}

/// Recursively build BEEF for a transaction and its parent transactions
///
/// This function uses a queue-based approach to avoid async recursion issues.
/// It:
/// 1. Checks if transaction is already in BEEF (deduplication)
/// 2. Fetches the transaction
/// 3. Parses it to get inputs
/// 4. Adds it to BEEF
/// 5. Fetches Merkle proof if available
/// 6. Processes parent transactions iteratively
pub async fn build_beef_for_txid(
    txid: &str,
    beef: &mut Beef,
    db: &Mutex<WalletDatabase>,
    client: &Client,
) -> Result<(), String> {
    use std::collections::HashSet;

    // Queue of transactions to process
    let mut queue = vec![txid.to_string()];
    let mut processed = HashSet::new();

    while let Some(current_txid) = queue.pop() {
        // Skip if already processed
        if processed.contains(&current_txid) {
            continue;
        }

        // Skip if already in BEEF
        if beef.find_txid(&current_txid).is_some() {
            processed.insert(current_txid);
            continue;
        }

        log::info!("   📥 Building BEEF for transaction {}", current_txid);

        // Fetch transaction (from cache or API)
        let tx_bytes = fetch_transaction_for_beef(&current_txid, db, client).await?;

        // Parse transaction to get inputs
        let parsed = ParsedTransaction::from_bytes(&tx_bytes)
            .map_err(|e| format!("Failed to parse transaction {}: {}", current_txid, e))?;

        // Add transaction to BEEF (as parent, not main)
        let tx_index = beef.add_parent_transaction(tx_bytes.clone());

        // Fetch Merkle proof if available
        let enhanced_tsc = {
            let db_guard = db.lock().unwrap();
            let conn = db_guard.connection();
            let merkle_proof_repo = MerkleProofRepository::new(conn);

            let cached_proof_result = merkle_proof_repo.get_by_parent_txid(&current_txid);
            drop(db_guard);

            match cached_proof_result {
                Ok(Some(cached_proof)) => {
                    log::info!("   ✅ Using cached Merkle proof for {} (height: {})", current_txid, cached_proof.block_height);
                    {
                        let db_guard = db.lock().unwrap();
                        let conn = db_guard.connection();
                        let merkle_proof_repo = MerkleProofRepository::new(conn);
                        merkle_proof_repo.to_tsc_json(&cached_proof)
                    }
                },
                Ok(None) => {
                    log::info!("   🌐 Cache miss - fetching TSC proof from API...");
                    match fetch_tsc_proof_from_api(client, &current_txid).await {
                        Ok(Some(tsc_json)) => {
                            let db_guard = db.lock().unwrap();
                            let conn = db_guard.connection();
                            let block_header_repo = BlockHeaderRepository::new(conn);
                            match enhance_tsc_with_height(client, &block_header_repo, &tsc_json).await {
                                Ok(enhanced_tsc) => {
                                    {
                                        let db_guard = db.lock().unwrap();
                                        let conn = db_guard.connection();
                                        if let Some(parent_txn_id) = {
                                            let parent_tx_repo = ParentTransactionRepository::new(conn);
                                            parent_tx_repo.get_id_by_txid(&current_txid).unwrap_or(None)
                                        } {
                                            let target_hash = enhanced_tsc["target"].as_str().unwrap_or("");
                                            if let Ok(nodes_json) = serde_json::to_string(&enhanced_tsc["nodes"]) {
                                                let block_height = enhanced_tsc["height"].as_u64().unwrap_or(0) as u32;
                                                let tx_index = enhanced_tsc["index"].as_u64().unwrap_or(0);
                                                let merkle_proof_repo = MerkleProofRepository::new(conn);
                                                let _ = merkle_proof_repo.upsert(parent_txn_id, block_height, tx_index, target_hash, &nodes_json);
                                                log::info!("   💾 Cached Merkle proof for {}", current_txid);
                                            }
                                        }
                                    }
                                    enhanced_tsc
                                },
                                Err(e) => {
                                    log::warn!("   ⚠️  Failed to enhance TSC proof for {}: {}", current_txid, e);
                                    serde_json::Value::Null
                                },
                            }
                        },
                        Ok(None) => {
                            log::warn!("   ⚠️  TSC proof not available (tx not confirmed)");
                            serde_json::Value::Null
                        },
                        Err(e) => {
                            log::warn!("   ⚠️  Failed to fetch TSC proof: {}", e);
                            serde_json::Value::Null
                        },
                    }
                },
                Err(e) => {
                    log::warn!("   ⚠️  Database error checking cache: {}, skipping proof", e);
                    serde_json::Value::Null
                },
            }
        };

        // Add Merkle proof to BEEF if available
        if !enhanced_tsc.is_null() {
            if let Err(e) = beef.add_tsc_merkle_proof(&current_txid, tx_index, &enhanced_tsc) {
                log::warn!("   ⚠️  Failed to add TSC Merkle proof for {}: {}", current_txid, e);
            } else {
                log::info!("   ✅ Added TSC Merkle proof (BUMP) to BEEF for {}", current_txid);
            }
        }

        // Add parent transactions to queue
        for input in parsed.inputs {
            let parent_txid = input.prev_txid;
            if !processed.contains(&parent_txid) && beef.find_txid(&parent_txid).is_none() {
                log::info!("   🔄 Queuing parent transaction {}", parent_txid);
                queue.push(parent_txid);
            }
        }

        processed.insert(current_txid);
    }

    Ok(())
}
