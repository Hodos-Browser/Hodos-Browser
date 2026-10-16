//! Helper functions for building BEEF structures
//!
//! Provides functions for fetching transactions and recursively building BEEF
//! for listOutputs when include='entire transactions' is requested.

use crate::beef::{Beef, ParsedTransaction};
use crate::database::{WalletDatabase, TransactionRepository, ParentTransactionRepository};
use crate::cache_helpers::{fetch_parent_transaction_from_api, fetch_tsc_proof_from_api};
use crate::proof_store::{ProofCheck, RootOracle};
use crate::services::WalletServices;
use std::sync::Mutex;
use sha2::{Sha256, Digest};

/// Result from fetch_transaction_for_beef indicating where the tx was found
pub struct FetchedTx {
    pub bytes: Vec<u8>,
    /// true if the tx was fetched from an external API (WoC), confirming it exists on-chain/mempool.
    /// false if it was found only in local cache (transactions table or parent_transactions).
    pub verified_on_network: bool,
}

/// Fetch a transaction by TXID from cache or API
///
/// Checks in order:
/// 1. `transactions` table (for wallet's own transactions)
/// 2. `parent_transactions` table (cached parent transactions)
/// 3. WhatsOnChain API (if not cached)
///
/// Returns raw transaction bytes and whether the tx was verified on-network
pub async fn fetch_transaction_for_beef(
    txid: &str,
    db: &Mutex<WalletDatabase>,
    services: &WalletServices,
) -> Result<FetchedTx, String> {
    // Step 1: Check transactions table (wallet's own transactions)
    // IMPORTANT: Lock must be scoped so it's dropped before Step 2
    let step1_hex: Option<String> = {
        let db_guard = db.lock().unwrap();
        let conn = db_guard.connection();
        let tx_repo = TransactionRepository::new(conn);
        match tx_repo.get_by_txid(txid) {
            Ok(Some(stored_action)) => {
                let raw_tx = stored_action.raw_tx;
                if !raw_tx.is_empty() { Some(raw_tx) } else { None }
            }
            _ => None,
        }
    }; // db_guard is ALWAYS dropped here

    if let Some(raw_tx_hex) = step1_hex {
        match hex::decode(&raw_tx_hex) {
            Ok(bytes) => {
                // Verify TXID matches
                let hash1 = Sha256::digest(&bytes);
                let hash2 = Sha256::digest(&hash1);
                let calculated_txid: Vec<u8> = hash2.into_iter().rev().collect();
                let calculated_txid_hex = hex::encode(calculated_txid);

                if calculated_txid_hex == txid {
                    log::info!("   ✅ Found transaction {} in transactions table", txid);
                    return Ok(FetchedTx { bytes, verified_on_network: false });
                } else {
                    log::warn!("   ⚠️  TXID mismatch for {} in transactions table, trying cache...", txid);
                }
            },
            Err(e) => {
                log::warn!("   ⚠️  Failed to decode transaction {} hex: {}, trying cache...", txid, e);
            },
        }
    }

    // Step 2: Check parent_transactions table (cached parent transactions)
    // IMPORTANT: Lock must be scoped so it's dropped before Step 3
    let step2_hex: Option<String> = {
        let db_guard = db.lock().unwrap();
        let conn = db_guard.connection();
        let parent_tx_repo = ParentTransactionRepository::new(conn);

        match parent_tx_repo.get_by_txid(txid) {
            Ok(Some(cached)) => {
                // Verify cached data
                match parent_tx_repo.verify_txid(txid, &cached.raw_hex) {
                    Ok(true) => {
                        log::info!("   ✅ Found transaction {} in parent_transactions cache", txid);
                        Some(cached.raw_hex)
                    },
                    Ok(false) => {
                        log::warn!("   ⚠️  Cached transaction {} failed TXID verification, fetching from API", txid);
                        None
                    },
                    Err(e) => {
                        log::warn!("   ⚠️  Error verifying cached transaction {}: {}, fetching from API", txid, e);
                        None
                    },
                }
            },
            Ok(None) => {
                log::info!("   🌐 Cache miss - fetching transaction {} from API...", txid);
                None
            },
            Err(e) => {
                log::warn!("   ⚠️  Database error checking cache: {}, fetching from API", e);
                None
            },
        }
    }; // db_guard is ALWAYS dropped here

    if let Some(cached_hex) = step2_hex {
        match hex::decode(&cached_hex) {
            Ok(bytes) => return Ok(FetchedTx { bytes, verified_on_network: false }),
            Err(e) => {
                log::warn!("   ⚠️  Failed to decode cached transaction {}: {}, fetching from API", txid, e);
            },
        }
    }

    // Step 3: Fetch from API
    match fetch_parent_transaction_from_api(services, txid).await {
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
                        Ok(FetchedTx { bytes, verified_on_network: true })
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

/// One transaction's merkle proof for a BEEF we are building (B5-T1-P7a-2 `C9a`/`C9b`):
/// - a stored proof, unless our own headers refute it;
/// - otherwise one fetched now — **stored** only if our headers verify it, **embedded** unless they
///   refute it (owner 2026-10-13, K-o: a block we do not hold — older than our checkpoint, or just
///   mined — is unchecked, not wrong; whoever receives the BEEF checks it against their own headers).
///
/// A fetched proof names its block by hash. When our chain does not hold that block, the height to
/// embed it at is an indexer's header lookup: a hint, used only for embedding, never stored.
pub async fn proof_for_beef(
    txid: &str,
    raw_tx: &[u8],
    db: &Mutex<WalletDatabase>,
    services: &WalletServices,
    chain: &dyn RootOracle,
) -> Option<serde_json::Value> {
    let stored = {
        let db_guard = db.lock().unwrap();
        crate::proof_store::stored_proof_for_beef(db_guard.connection(), chain, txid).unwrap_or(None)
    };
    if let Some(tsc) = stored {
        log::info!("   ✅ Using stored Merkle proof for {}", txid);
        return Some(tsc);
    }

    log::info!("   🌐 No usable stored proof - fetching TSC proof from API...");
    let tsc = match fetch_tsc_proof_from_api(services, txid).await {
        Ok(Some(tsc)) => tsc,
        Ok(None) => {
            log::warn!("   ⚠️  TSC proof not available (tx not confirmed)");
            return None;
        }
        Err(e) => {
            log::warn!("   ⚠️  Failed to fetch TSC proof: {}", e);
            return None;
        }
    };

    match crate::proof_store::check_tsc(chain, txid, &tsc) {
        ProofCheck::Verified(proof) => {
            let db_guard = db.lock().unwrap();
            let conn = db_guard.connection();
            let block_hash = tsc["target"].as_str().unwrap_or("");
            match crate::proof_store::store(conn, chain, &proof, raw_tx, block_hash) {
                Ok(row) => {
                    let _ = crate::proof_store::link(conn, &row, txid);
                    log::info!("   💾 Stored verified proof for {}", txid);
                }
                Err(e) => log::warn!("   ⚠️  Failed to store proof for {}: {}", txid, e),
            }
            Some(proof.tsc().clone())
        }
        ProofCheck::Wrong { height } => {
            log::warn!("   ❌ fetched proof for {} does not match our header at {} — not used", txid, height);
            None
        }
        ProofCheck::Malformed(e) => {
            log::warn!("   ⚠️  fetched proof for {} unreadable: {}", txid, e);
            None
        }
        ProofCheck::NotHeld => with_height_hint(txid, tsc, services, chain).await,
    }
}

/// A proof for a block our chain does not hold, made ready to embed unchecked (K-o): at its own
/// height, or at the height an indexer gives for its block hash — a hint, used only for embedding.
async fn with_height_hint(txid: &str, mut tsc: serde_json::Value, services: &WalletServices, chain: &dyn RootOracle) -> Option<serde_json::Value> {
    if tsc["height"].as_u64().unwrap_or(0) == 0 {
        let target = tsc["target"].as_str().filter(|t| !t.is_empty())?.to_string();
        let hint = match services.get_block_header(crate::services::BlockKey::Hash(target)).await {
            Ok(h) if h.height > 0 => h.height,
            _ => {
                log::warn!("   ⚠️  no height for the block of {}'s proof — not embedded", txid);
                return None;
            }
        };
        tsc["height"] = serde_json::json!(hint);
        // A hint is still checked wherever our chain can check it: a block that left our main chain
        // (or a made-up one) named at a height we hold is refuted there (adversarial review).
        match crate::proof_store::check_tsc(chain, txid, &tsc) {
            ProofCheck::Verified(_) | ProofCheck::NotHeld => {}
            ProofCheck::Wrong { height } => {
                log::warn!("   ❌ {}'s proof does not match our header at its hinted height {} — not embedded", txid, height);
                return None;
            }
            // xhigh 168 #5: a path that cannot produce a root is not embedded either.
            ProofCheck::Malformed(e) => {
                log::warn!("   ⚠️  {}'s proof is unreadable at its hinted height: {} — not embedded", txid, e);
                return None;
            }
        }
    }
    log::info!("   ⏳ embedding {}'s proof unchecked: our headers do not hold its block", txid);
    Some(tsc)
}

/// A fetched proof (TSC, possibly WhatsOnChain's array form or `blockHeight` field) made ready to
/// embed in a BEEF, never stored (B5-T1-P7a-2 `C15`): checked against our own headers — `None` when
/// they refute it or it cannot be read; at our chain's height when we hold its block; otherwise
/// unchecked at a hinted height (K-o).
pub async fn embeddable_proof(
    txid: &str,
    proof_json: &serde_json::Value,
    services: &WalletServices,
    chain: &dyn RootOracle,
) -> Option<serde_json::Value> {
    let mut tsc = match proof_json.as_array() {
        Some(a) => a.first()?.clone(),
        None => proof_json.clone(),
    };
    if tsc["height"].as_u64().unwrap_or(0) == 0 {
        if let Some(h) = tsc.get("blockHeight").and_then(|v| v.as_u64()).filter(|h| *h > 0) {
            tsc["height"] = serde_json::json!(h);
        }
    }
    match crate::proof_store::check_tsc(chain, txid, &tsc) {
        ProofCheck::Verified(p) => Some(p.tsc().clone()),
        ProofCheck::Wrong { height } => {
            log::warn!("   ❌ proof for {} does not match our header at {} — not embedded", txid, height);
            None
        }
        ProofCheck::Malformed(e) => {
            log::warn!("   ⚠️  proof for {} unreadable: {}", txid, e);
            None
        }
        ProofCheck::NotHeld => with_height_hint(txid, tsc, services, chain).await,
    }
}

/// Maximum number of ancestor transactions to include in a single BEEF build.
/// Prevents runaway ancestry walks from freezing the app.
const MAX_BEEF_ANCESTORS: usize = 50;

/// Recursively build BEEF for a transaction and its parent transactions
///
/// This function uses a queue-based approach to avoid async recursion issues.
/// It:
/// 1. Checks if transaction is already in BEEF (deduplication)
/// 2. Fetches the transaction
/// 3. Parses it to get inputs
/// 4. Adds it to BEEF
/// 5. Fetches Merkle proof if available
/// 6. Only queues parent transactions if current tx has NO merkle proof
///    (confirmed transactions with BUMPs don't need their parents in BEEF)
pub async fn build_beef_for_txid(
    txid: &str,
    beef: &mut Beef,
    db: &Mutex<WalletDatabase>,
    services: &WalletServices,
    chain: &dyn RootOracle,
) -> Result<(), String> {
    use std::collections::HashSet;

    // Queue of transactions to process
    let mut queue = vec![txid.to_string()];
    let mut processed = HashSet::new();
    let mut missing_ancestors: Vec<String> = Vec::new();

    while let Some(current_txid) = queue.pop() {
        // Skip if already processed
        if processed.contains(&current_txid) {
            continue;
        }

        // Safety limit: prevent runaway ancestry walks
        if processed.len() >= MAX_BEEF_ANCESTORS {
            log::warn!("   ⚠️  Reached maximum ancestor limit ({}) for BEEF building, stopping", MAX_BEEF_ANCESTORS);
            break;
        }

        // Skip if already in BEEF
        if beef.find_txid(&current_txid).is_some() {
            processed.insert(current_txid);
            continue;
        }

        log::info!("   📥 Building BEEF for transaction {}", current_txid);

        // Fetch transaction (from cache or API)
        // If fetch fails, this ancestor is unfetchable (ghost/phantom) — track it
        // but continue walking to discover ALL missing ancestors for a complete error message.
        let fetched = match fetch_transaction_for_beef(&current_txid, db, services).await {
            Ok(f) => f,
            Err(e) => {
                log::error!("   ❌ Failed to fetch ancestor {}: {} — ancestry chain broken", current_txid, e);
                missing_ancestors.push(current_txid.clone());
                processed.insert(current_txid);
                continue;
            }
        };
        let tx_bytes = fetched.bytes;
        let verified_on_network = fetched.verified_on_network;

        // Parse transaction to get inputs
        let parsed = match ParsedTransaction::from_bytes(&tx_bytes) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("   ⚠️  Failed to parse transaction {}: {}, skipping", current_txid, e);
                processed.insert(current_txid);
                continue;
            }
        };

        // Add transaction to BEEF (as parent, not main)
        let tx_index = beef.add_parent_transaction(tx_bytes.clone());

        // Check if this is a local unbroadcast transaction (no proof possible)
        // Transactions with status unsigned/failed/sending/unproven haven't been mined,
        // so there's no merkle proof to fetch. Skip expensive API calls.
        let skip_proof_fetch = {
            let db_guard = db.lock().unwrap();
            let conn = db_guard.connection();
            let tx_repo = TransactionRepository::new(conn);
            // Reported mined in a block our chain holds (or at a height not recorded): only then can a
            // fetched proof verify — an older block waits for B5-T1-P7a-3 (xhigh 168 #10).
            let reported_mined = conn.query_row(
                "SELECT confirmations > 0, block_height FROM transactions WHERE txid = ?1", [&current_txid],
                |r| Ok((r.get::<_, bool>(0)?, r.get::<_, Option<i64>>(1)?)),
            ).map(|(mined, h)| mined && h.map_or(true, |h| chain.holds_height(h as u32))).unwrap_or(false);
            match tx_repo.get_broadcast_status(&current_txid) {
                // "completed" → has proof, include as proven leaf. "unproven" an indexer reported mined
                // may still lack a verified proof (decision 6) — try for it (B5-T1-P7a-2 review); a
                // fresh unmined one is skipped, keeping proof fetches off the payment hot path.
                Ok(Some(ref status)) if status == "completed" || (status == "unproven" && reported_mined) => false,
                Ok(Some(ref status)) => {
                    log::info!("   ⏭️  Transaction {} has status '{}', skipping proof fetch", current_txid, status);
                    true
                }
                Ok(None) => false, // Not in our transactions table → might need proof from API
                Err(_) => false,   // Error checking → try anyway
            }
        };

        // Fetch Merkle proof from proven_txs or API
        let has_bump;
        let enhanced_tsc = if skip_proof_fetch {
            serde_json::Value::Null
        } else {
            let raw_tx_bytes = {
                let db_guard = db.lock().unwrap();
                match ParentTransactionRepository::new(db_guard.connection()).get_by_txid(&current_txid) {
                    Ok(Some(cached)) => hex::decode(&cached.raw_hex).unwrap_or_default(),
                    _ => tx_bytes.clone(),
                }
            };
            proof_for_beef(&current_txid, &raw_tx_bytes, db, services, chain).await
                .unwrap_or(serde_json::Value::Null)
        };

        // Add Merkle proof to BEEF if available
        if !enhanced_tsc.is_null() {
            if let Err(e) = beef.add_tsc_merkle_proof(&current_txid, tx_index, &enhanced_tsc) {
                log::warn!("   ⚠️  Failed to add TSC Merkle proof for {}: {}", current_txid, e);
                has_bump = false;
            } else {
                log::info!("   ✅ Added TSC Merkle proof (BUMP) to BEEF for {}", current_txid);
                has_bump = true;
            }
        } else {
            has_bump = false;
        }

        // Ghost detection: if this tx has no merkle proof AND was found only in local cache
        // (not verified on the network), check if it actually exists on WoC.
        // A tx in our local cache but unknown to the network is a ghost/phantom —
        // it was never successfully broadcast or was dropped from all mempools.
        // Including it in BEEF will cause SEEN_IN_ORPHAN_MEMPOOL from ARC.
        if !has_bump && !verified_on_network {
            // Ghost detection via Services chain (Phase 1.6d.C — was inline WoC fetch).
            // NotFound from the chain short-circuits → tx doesn't exist anywhere we can
            // reach → ghost. Other errors are inconclusive and we proceed cautiously
            // (include the parent and let ARC decide on broadcast).
            let status_result = services.tx_status(&current_txid).await;
            match status_result {
                Err(crate::services::IndexerError::NotFound) => {
                    log::error!(
                        "   ❌ Unconfirmed parent {} NOT FOUND via Services — ghost transaction, excluding from BEEF",
                        &current_txid[..std::cmp::min(16, current_txid.len())]
                    );
                    // Clean up: mark any outputs from this ghost tx as not spendable
                    {
                        let db_guard = db.lock().unwrap();
                        let conn = db_guard.connection();
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;
                        let _ = conn.execute(
                            "UPDATE outputs SET spendable = 0, spending_description = 'phantom: parent tx not on chain', updated_at = ?1
                             WHERE txid = ?2 AND spendable = 1",
                            rusqlite::params![now, &current_txid],
                        );
                        log::info!(
                            "   🗑️  Marked outputs from ghost tx {} as phantom",
                            &current_txid[..std::cmp::min(16, current_txid.len())]
                        );
                    }
                    missing_ancestors.push(current_txid.clone());
                    processed.insert(current_txid);
                    continue;
                }
                Ok(_) => {
                    log::info!(
                        "   ✅ Unconfirmed parent {} verified via Services (exists in mempool/chain)",
                        &current_txid[..std::cmp::min(16, current_txid.len())]
                    );
                }
                Err(e) => {
                    log::warn!(
                        "   ⚠️  Services tx_status check failed for {}: {} — proceeding without verification",
                        &current_txid[..std::cmp::min(16, current_txid.len())], e
                    );
                }
            }
        }

        // Only queue parent transactions if this tx has NO merkle proof.
        // A transaction with a BUMP is proven by its block inclusion -
        // its parents are not needed in the BEEF.
        if !has_bump {
            for input in parsed.inputs {
                let parent_txid = input.prev_txid;
                if !processed.contains(&parent_txid) && beef.find_txid(&parent_txid).is_none() {
                    log::info!("   🔄 Queuing parent transaction {}", parent_txid);
                    queue.push(parent_txid);
                }
            }
        }

        processed.insert(current_txid);
    }

    // If any ancestors couldn't be fetched, the BEEF chain is broken.
    // Return error with full list so caller can decide how to handle.
    if !missing_ancestors.is_empty() {
        let short_ids: Vec<String> = missing_ancestors.iter()
            .map(|t| t[..16.min(t.len())].to_string())
            .collect();
        return Err(format!(
            "BEEF ancestry broken: {} unfetchable ancestor(s): {}",
            missing_ancestors.len(),
            short_ids.join(", ")
        ));
    }

    Ok(())
}
