//! UTXO Ancestry Validation and Chain Cleanup
//!
//! This module provides functions to:
//! 1. Validate that a UTXO has a complete, valid ancestry chain
//! 2. Mark broken chains as unusable when detected
//!
//! This prevents selecting UTXOs that will fail to broadcast due to missing ancestors.

use log::{info, warn, error};
use std::collections::HashSet;

/// Result of ancestry validation
#[derive(Debug)]
pub struct AncestryValidationResult {
    pub is_valid: bool,
    pub missing_txid: Option<String>,
    pub depth_checked: usize,
    pub error_message: Option<String>,
}

/// Validate that a UTXO has valid ancestry (all parent transactions exist)
///
/// This checks recursively until we find:
/// - A confirmed transaction (on blockchain with merkle proof)
/// - Or a broken link (transaction that doesn't exist anywhere)
///
/// Returns true if the ancestry is valid, false if there's a broken link.
pub async fn validate_utxo_ancestry(
    txid: &str,
    state: &crate::AppState,
    client: &reqwest::Client,
    max_depth: usize,
) -> AncestryValidationResult {
    let mut visited: HashSet<String> = HashSet::new();

    info!("   🔍 Validating ancestry for UTXO txid: {}...", &txid[..std::cmp::min(16, txid.len())]);

    validate_single_ancestor(
        txid,
        0,
        max_depth,
        &mut visited,
        state,
        client,
    ).await
}

/// Recursively validate a single ancestor
async fn validate_single_ancestor(
    txid: &str,
    depth: usize,
    max_depth: usize,
    visited: &mut HashSet<String>,
    state: &crate::AppState,
    client: &reqwest::Client,
) -> AncestryValidationResult {
    // Avoid cycles
    if visited.contains(txid) {
        return AncestryValidationResult {
            is_valid: true,
            missing_txid: None,
            depth_checked: depth,
            error_message: None,
        };
    }
    visited.insert(txid.to_string());

    // Check depth limit
    if depth > max_depth {
        warn!("   ⚠️  Max ancestry depth ({}) reached for {}", max_depth, &txid[..std::cmp::min(16, txid.len())]);
        // If we've gone this deep without finding a problem, assume it's okay
        return AncestryValidationResult {
            is_valid: true,
            missing_txid: None,
            depth_checked: depth,
            error_message: Some(format!("Max depth {} reached", max_depth)),
        };
    }

    // Step 1: Check if transaction is confirmed on blockchain (has merkle proof)
    if check_tx_confirmed(txid, client).await {
        info!("   ✅ [Depth {}] Transaction {} is confirmed on-chain", depth, &txid[..std::cmp::min(16, txid.len())]);
        return AncestryValidationResult {
            is_valid: true,
            missing_txid: None,
            depth_checked: depth,
            error_message: None,
        };
    }

    // Step 2: Try to get the transaction from local DB
    let raw_tx_result = get_local_transaction(txid, state);

    let raw_tx = match raw_tx_result {
        Some(tx) => {
            info!("   📋 [Depth {}] Found local transaction: {}", depth, &txid[..std::cmp::min(16, txid.len())]);
            tx
        }
        None => {
            // Step 3: Try to get from blockchain API
            match get_tx_from_api(txid, client).await {
                Some(tx) => {
                    info!("   🌐 [Depth {}] Found transaction on API: {}", depth, &txid[..std::cmp::min(16, txid.len())]);
                    tx
                }
                None => {
                    // Transaction doesn't exist anywhere - broken chain!
                    error!("   ❌ [Depth {}] Transaction {} not found - BROKEN CHAIN", depth, &txid[..std::cmp::min(16, txid.len())]);
                    return AncestryValidationResult {
                        is_valid: false,
                        missing_txid: Some(txid.to_string()),
                        depth_checked: depth,
                        error_message: Some(format!("Transaction {} not found locally or on blockchain", txid)),
                    };
                }
            }
        }
    };

    // Step 4: Parse the transaction to get its inputs
    let parsed = match crate::beef::ParsedTransaction::from_bytes(&raw_tx) {
        Ok(p) => p,
        Err(e) => {
            error!("   ❌ [Depth {}] Failed to parse transaction {}: {}", depth, &txid[..std::cmp::min(16, txid.len())], e);
            return AncestryValidationResult {
                is_valid: false,
                missing_txid: Some(txid.to_string()),
                depth_checked: depth,
                error_message: Some(format!("Failed to parse transaction: {}", e)),
            };
        }
    };

    // Step 5: Recursively validate each input's parent
    for input in &parsed.inputs {
        // Skip coinbase inputs
        if input.prev_txid == "0000000000000000000000000000000000000000000000000000000000000000" {
            continue;
        }

        let result = Box::pin(validate_single_ancestor(
            &input.prev_txid,
            depth + 1,
            max_depth,
            visited,
            state,
            client,
        )).await;

        if !result.is_valid {
            return result;
        }
    }

    // All ancestors validated successfully
    AncestryValidationResult {
        is_valid: true,
        missing_txid: None,
        depth_checked: depth,
        error_message: None,
    }
}

/// Check if a transaction is confirmed on the blockchain
async fn check_tx_confirmed(txid: &str, client: &reqwest::Client) -> bool {
    let url = format!(
        "https://api.whatsonchain.com/v1/bsv/main/tx/{}/proof/tsc",
        txid
    );

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(text) = response.text().await {
                    // A valid proof has actual data, not just "null"
                    !text.trim().eq_ignore_ascii_case("null") && text.contains("target")
                } else {
                    false
                }
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// Get transaction from local database
fn get_local_transaction(txid: &str, state: &crate::AppState) -> Option<Vec<u8>> {
    let db = state.database.lock().unwrap();

    // Try transactions table first
    let tx_repo = crate::database::TransactionRepository::new(db.connection());
    if let Ok(Some(raw_tx_hex)) = tx_repo.get_raw_tx(txid) {
        if !raw_tx_hex.is_empty() {
            if let Ok(raw_bytes) = hex::decode(&raw_tx_hex) {
                // Verify the stored tx produces this txid
                if verify_txid(&raw_bytes, txid) {
                    return Some(raw_bytes);
                }
            }
        }
    }

    // Try parent_transactions cache
    let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());
    if let Ok(Some(cached)) = parent_tx_repo.get_by_txid(txid) {
        if let Ok(raw_bytes) = hex::decode(&cached.raw_hex) {
            if verify_txid(&raw_bytes, txid) {
                return Some(raw_bytes);
            }
        }
    }

    None
}

/// Get transaction from blockchain API
async fn get_tx_from_api(txid: &str, client: &reqwest::Client) -> Option<Vec<u8>> {
    let url = format!(
        "https://api.whatsonchain.com/v1/bsv/main/tx/{}/hex",
        txid
    );

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(hex_str) = response.text().await {
                    let hex_str = hex_str.trim().trim_matches('"');
                    hex::decode(hex_str).ok()
                } else {
                    None
                }
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

/// Verify that raw transaction bytes produce the expected txid
fn verify_txid(raw_tx: &[u8], expected_txid: &str) -> bool {
    use sha2::{Sha256, Digest};
    let first_hash = Sha256::digest(raw_tx);
    let second_hash = Sha256::digest(&first_hash);
    let computed_txid = hex::encode(second_hash.iter().rev().copied().collect::<Vec<u8>>());
    computed_txid == expected_txid
}

/// Mark a UTXO chain as unusable
///
/// This marks the specified UTXO and any UTXOs that descend from it as spent,
/// preventing them from being selected in future transactions.
pub fn mark_chain_unusable(
    txid: &str,
    state: &crate::AppState,
    reason: &str,
) -> Result<usize, String> {
    let db = state.database.lock().unwrap();
    let conn = db.connection();

    info!("   🚫 Marking chain as unusable: {} (reason: {})", &txid[..std::cmp::min(16, txid.len())], reason);

    // Mark UTXOs with this txid as spent
    let result = conn.execute(
        "UPDATE utxos SET is_spent = 1, spent_txid = ?1 WHERE txid = ?2 AND is_spent = 0",
        rusqlite::params![reason, txid],
    );

    let direct_count = match result {
        Ok(count) => {
            if count > 0 {
                info!("   ✅ Marked {} direct UTXO(s) as unusable", count);
            }
            count
        }
        Err(e) => {
            error!("   ❌ Failed to mark UTXOs as unusable: {}", e);
            return Err(format!("Database error: {}", e));
        }
    };

    // Also find and mark any UTXOs that are outputs of transactions that spend from this txid
    // This handles the case where we have a chain: A -> B -> C, and A is broken
    // We need to mark UTXOs from B and C as well
    let descendant_count = mark_descendant_utxos(conn, txid, reason)?;

    Ok(direct_count + descendant_count)
}

/// Mark UTXOs that descend from a given transaction as unusable
fn mark_descendant_utxos(
    conn: &rusqlite::Connection,
    ancestor_txid: &str,
    reason: &str,
) -> Result<usize, String> {
    // Find transactions that have inputs referencing this txid
    let mut stmt = conn.prepare(
        "SELECT DISTINCT t.txid
         FROM transactions t
         JOIN transaction_inputs ti ON t.id = ti.transaction_id
         WHERE ti.txid = ?1"
    ).map_err(|e| format!("Prepare error: {}", e))?;

    let descendant_txids: Vec<String> = stmt.query_map(
        rusqlite::params![ancestor_txid],
        |row| row.get(0)
    )
    .map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    let mut total_marked = 0;

    for desc_txid in descendant_txids {
        // Mark UTXOs from this descendant transaction
        let result = conn.execute(
            "UPDATE utxos SET is_spent = 1, spent_txid = ?1 WHERE txid = ?2 AND is_spent = 0",
            rusqlite::params![reason, desc_txid],
        );

        if let Ok(count) = result {
            if count > 0 {
                info!("   ✅ Marked {} descendant UTXO(s) from {} as unusable", count, &desc_txid[..std::cmp::min(16, desc_txid.len())]);
                total_marked += count;
            }
        }

        // Recursively mark descendants of descendants
        total_marked += mark_descendant_utxos(conn, &desc_txid, reason)?;
    }

    Ok(total_marked)
}

/// Scan all unspent UTXOs and validate their ancestry
/// This is useful for cleanup on startup or after detecting issues
pub async fn scan_and_cleanup_broken_chains(
    state: &crate::AppState,
    client: &reqwest::Client,
) -> (usize, usize) {
    info!("🔍 Scanning for broken UTXO chains...");

    // Get all unspent UTXOs
    let unspent_utxos: Vec<(String, u32, i64)> = {
        let db = state.database.lock().unwrap();
        let conn = db.connection();

        let mut stmt = match conn.prepare(
            "SELECT DISTINCT txid, vout, satoshis FROM utxos WHERE is_spent = 0"
        ) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to query UTXOs: {}", e);
                return (0, 0);
            }
        };

        stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?, row.get::<_, i64>(2)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    };

    info!("   Found {} unspent UTXOs to validate", unspent_utxos.len());

    let mut valid_count = 0;
    let mut broken_count = 0;
    let mut checked_txids: HashSet<String> = HashSet::new();

    for (txid, _vout, satoshis) in unspent_utxos {
        // Skip if we've already checked this txid
        if checked_txids.contains(&txid) {
            continue;
        }
        checked_txids.insert(txid.clone());

        let result = validate_utxo_ancestry(&txid, state, client, 10).await;

        if result.is_valid {
            valid_count += 1;
        } else {
            broken_count += 1;
            warn!("   ⚠️  Broken chain detected for UTXO {} ({} sats): {:?}",
                  &txid[..std::cmp::min(16, txid.len())], satoshis, result.error_message);

            // Mark the chain as unusable
            if let Some(ref missing) = result.missing_txid {
                let _ = mark_chain_unusable(missing, state, "broken-ancestry-scan");
            }
            let _ = mark_chain_unusable(&txid, state, "broken-ancestry-scan");
        }
    }

    info!("✅ Scan complete: {} valid, {} broken chains cleaned up", valid_count, broken_count);

    (valid_count, broken_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_txid() {
        // A simple test - in practice this would use real transaction data
        let fake_tx = vec![0u8; 100];
        // This will compute some txid, we just verify the function works
        let result = verify_txid(&fake_tx, "not_the_right_txid");
        assert!(!result);
    }
}
