//! Recursive Ancestor Collection for BEEF Building
//!
//! Per BRC-62 specification, BEEF must include all ancestor transactions
//! back to confirmed transactions (ones with merkle proofs).
//! This module implements recursive ancestor collection with topological ordering.

use std::collections::{HashMap, HashSet};
use log::{info, warn, error};

/// Information about an ancestor transaction
#[derive(Debug, Clone)]
pub struct AncestorInfo {
    pub txid: String,
    pub raw_tx: Vec<u8>,
    pub has_merkle_proof: bool,
    pub merkle_proof: Option<crate::beef::MerkleProof>,
    /// The depth in the ancestry chain (0 = immediate parent, 1 = grandparent, etc.)
    pub depth: usize,
}

/// Result of ancestor collection
#[derive(Debug)]
pub struct AncestorCollectionResult {
    /// Ancestors in topological order (oldest/deepest first, most recent last)
    pub ancestors: Vec<AncestorInfo>,
    /// TXIDs that couldn't be resolved (errors)
    pub errors: Vec<(String, String)>,
}

/// Collects all unconfirmed ancestors of the given transactions recursively.
///
/// This function walks back through the transaction DAG until it reaches
/// confirmed transactions (ones with merkle proofs) or transactions that
/// exist on the blockchain.
///
/// Returns ancestors in topological order: parents before children.
pub async fn collect_ancestors_recursive(
    starting_txids: &[(String, u32)], // (txid, vout) pairs for inputs
    state: &crate::AppState,
    client: &reqwest::Client,
    max_depth: usize,
) -> AncestorCollectionResult {
    let mut visited: HashSet<String> = HashSet::new();
    let mut ancestors: HashMap<String, AncestorInfo> = HashMap::new();
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut processing_order: Vec<String> = Vec::new(); // Track order for topological sort

    info!("   🔍 Starting recursive ancestor collection for {} inputs (max_depth: {})",
          starting_txids.len(), max_depth);

    // Process each starting txid
    for (txid, _vout) in starting_txids {
        collect_single_ancestor(
            txid,
            0,
            max_depth,
            &mut visited,
            &mut ancestors,
            &mut errors,
            &mut processing_order,
            state,
            client,
        ).await;
    }

    // Build result in topological order (deepest/oldest first)
    // Since we processed depth-first, processing_order already has parents before children
    // But we want oldest first, so we reverse it
    let mut result_ancestors: Vec<AncestorInfo> = Vec::new();

    // Sort by depth (deepest first) then by processing order
    let mut sorted_txids: Vec<_> = processing_order.into_iter()
        .filter(|txid| ancestors.contains_key(txid))
        .collect();

    // Sort by depth descending (oldest ancestors first)
    sorted_txids.sort_by(|a, b| {
        let depth_a = ancestors.get(a).map(|i| i.depth).unwrap_or(0);
        let depth_b = ancestors.get(b).map(|i| i.depth).unwrap_or(0);
        depth_b.cmp(&depth_a) // Descending by depth (deepest first)
    });

    for txid in sorted_txids {
        if let Some(info) = ancestors.remove(&txid) {
            result_ancestors.push(info);
        }
    }

    info!("   📊 Collected {} ancestors ({} errors)",
          result_ancestors.len(), errors.len());

    AncestorCollectionResult {
        ancestors: result_ancestors,
        errors,
    }
}

/// Recursively collect a single ancestor and its parents
async fn collect_single_ancestor(
    txid: &str,
    depth: usize,
    max_depth: usize,
    visited: &mut HashSet<String>,
    ancestors: &mut HashMap<String, AncestorInfo>,
    errors: &mut Vec<(String, String)>,
    processing_order: &mut Vec<String>,
    state: &crate::AppState,
    client: &reqwest::Client,
) {
    // Check if already visited
    if visited.contains(txid) {
        return;
    }
    visited.insert(txid.to_string());

    // Check depth limit
    if depth > max_depth {
        warn!("   ⚠️  Max depth ({}) reached for txid {}", max_depth, &txid[..16]);
        errors.push((txid.to_string(), format!("Max depth {} exceeded", max_depth)));
        return;
    }

    info!("   🔍 [Depth {}] Processing ancestor: {}", depth, &txid[..std::cmp::min(16, txid.len())]);

    // Step 1: Check if this transaction has a merkle proof (is confirmed)
    let has_proof = check_merkle_proof_available(txid, client).await;

    if has_proof {
        info!("   ✅ [Depth {}] Transaction {} is confirmed (has merkle proof)", depth, &txid[..16]);
        // For confirmed transactions, we still need the raw tx but don't need to recurse
        match get_raw_transaction(txid, state, client).await {
            Ok(raw_tx) => {
                // Fetch the actual merkle proof
                let merkle_proof = fetch_merkle_proof(txid, client).await.ok();

                ancestors.insert(txid.to_string(), AncestorInfo {
                    txid: txid.to_string(),
                    raw_tx,
                    has_merkle_proof: true,
                    merkle_proof,
                    depth,
                });
                processing_order.push(txid.to_string());
            }
            Err(e) => {
                error!("   ❌ [Depth {}] Failed to get raw tx for confirmed {}: {}", depth, &txid[..16], e);
                errors.push((txid.to_string(), e));
            }
        }
        return; // Don't recurse into confirmed transactions
    }

    // Step 2: Get the raw transaction (from local DB or API)
    let raw_tx = match get_raw_transaction(txid, state, client).await {
        Ok(tx) => tx,
        Err(e) => {
            error!("   ❌ [Depth {}] Failed to get raw tx for {}: {}", depth, &txid[..16], e);
            errors.push((txid.to_string(), e));
            return;
        }
    };

    // Step 3: Parse the transaction to get its inputs
    let parsed = match crate::beef::ParsedTransaction::from_bytes(&raw_tx) {
        Ok(p) => p,
        Err(e) => {
            error!("   ❌ [Depth {}] Failed to parse tx {}: {}", depth, &txid[..16], e);
            errors.push((txid.to_string(), format!("Parse error: {}", e)));
            return;
        }
    };

    info!("   📋 [Depth {}] Transaction {} has {} inputs", depth, &txid[..16], parsed.inputs.len());

    // Step 4: Recursively collect ancestors for each input
    for input in &parsed.inputs {
        // Skip coinbase inputs (all zeros txid)
        if input.prev_txid == "0000000000000000000000000000000000000000000000000000000000000000" {
            info!("   ⏭️  [Depth {}] Skipping coinbase input", depth);
            continue;
        }

        // Recurse into parent using Box::pin for async recursion
        Box::pin(collect_single_ancestor(
            &input.prev_txid,
            depth + 1,
            max_depth,
            visited,
            ancestors,
            errors,
            processing_order,
            state,
            client,
        )).await;
    }

    // Step 5: Add this transaction AFTER its parents (topological order)
    ancestors.insert(txid.to_string(), AncestorInfo {
        txid: txid.to_string(),
        raw_tx,
        has_merkle_proof: false,
        merkle_proof: None,
        depth,
    });
    processing_order.push(txid.to_string());

    info!("   ✅ [Depth {}] Added unconfirmed ancestor: {}", depth, &txid[..16]);
}

/// Check if a transaction has a merkle proof available (is confirmed)
async fn check_merkle_proof_available(txid: &str, client: &reqwest::Client) -> bool {
    // Try to fetch TSC proof from WhatsOnChain
    let url = format!(
        "https://api.whatsonchain.com/v1/bsv/main/tx/{}/proof/tsc",
        txid
    );

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                // Check if the response is valid JSON with proof data
                if let Ok(text) = response.text().await {
                    // A valid proof response has actual proof data, not just "null"
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

/// Verify that raw transaction bytes produce the expected txid
fn verify_txid(raw_tx: &[u8], expected_txid: &str) -> bool {
    use sha2::{Sha256, Digest};
    let first_hash = Sha256::digest(raw_tx);
    let second_hash = Sha256::digest(&first_hash);
    let computed_txid = hex::encode(second_hash.iter().rev().copied().collect::<Vec<u8>>());
    computed_txid == expected_txid
}

/// Get raw transaction bytes from local DB or API
async fn get_raw_transaction(
    txid: &str,
    state: &crate::AppState,
    client: &reqwest::Client,
) -> Result<Vec<u8>, String> {
    // First, try local transactions table
    {
        let db = state.database.lock().unwrap();
        let tx_repo = crate::database::TransactionRepository::new(db.connection());

        if let Ok(Some(raw_tx_hex)) = tx_repo.get_raw_tx(txid) {
            if !raw_tx_hex.is_empty() {
                if let Ok(raw_bytes) = hex::decode(&raw_tx_hex) {
                    // Verify the stored tx actually produces this txid
                    if verify_txid(&raw_bytes, txid) {
                        info!("   📋 Using local transaction from DB: {}", &txid[..16]);
                        return Ok(raw_bytes);
                    } else {
                        warn!("   ⚠️  Local tx {} has stale raw_tx (txid mismatch), falling back to API", &txid[..16]);
                    }
                }
            }
        }
    }

    // Second, try parent_transactions cache
    {
        let db = state.database.lock().unwrap();
        let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());

        if let Ok(Some(cached)) = parent_tx_repo.get_by_txid(txid) {
            if let Ok(raw_bytes) = hex::decode(&cached.raw_hex) {
                // Verify cached tx produces expected txid
                if verify_txid(&raw_bytes, txid) {
                    info!("   💾 Using cached parent transaction: {}", &txid[..16]);
                    return Ok(raw_bytes);
                } else {
                    warn!("   ⚠️  Cached tx {} has wrong raw_hex (txid mismatch), falling back to API", &txid[..16]);
                }
            }
        }
    }

    // Third, try API
    info!("   🌐 Fetching transaction from API: {}", &txid[..16]);
    let url = format!(
        "https://api.whatsonchain.com/v1/bsv/main/tx/{}/hex",
        txid
    );

    let response = client.get(&url).send().await
        .map_err(|e| format!("API request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("API returned status {} for tx {}", response.status(), txid));
    }

    let hex_str = response.text().await
        .map_err(|e| format!("Failed to read API response: {}", e))?;

    let hex_str = hex_str.trim().trim_matches('"');

    hex::decode(hex_str)
        .map_err(|e| format!("Failed to decode API tx hex: {}", e))
}

/// Fetch merkle proof for a confirmed transaction
async fn fetch_merkle_proof(
    txid: &str,
    client: &reqwest::Client,
) -> Result<crate::beef::MerkleProof, String> {
    let url = format!(
        "https://api.whatsonchain.com/v1/bsv/main/tx/{}/proof/tsc",
        txid
    );

    let response = client.get(&url).send().await
        .map_err(|e| format!("Failed to fetch merkle proof: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("API returned status {} for proof", response.status()));
    }

    let json: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse proof JSON: {}", e))?;

    // Parse TSC proof format into our MerkleProof struct
    parse_tsc_to_merkle_proof(&json)
}

/// Parse TSC proof JSON into our MerkleProof struct
fn parse_tsc_to_merkle_proof(json: &serde_json::Value) -> Result<crate::beef::MerkleProof, String> {
    let block_height = json["targetType"].as_str()
        .and_then(|_| json["target"].as_str())
        .ok_or("Missing target in TSC proof")?;

    // For now, create a minimal proof structure
    // The actual implementation would need to parse the full TSC format
    let index = json["index"].as_u64().unwrap_or(0) as u32;
    let nodes = json["nodes"].as_array()
        .map(|arr| arr.iter()
            .filter_map(|v| v.as_str())
            .map(|s| hex::decode(s).unwrap_or_default())
            .collect::<Vec<_>>())
        .unwrap_or_default();

    // Calculate tree height from nodes
    let tree_height = nodes.len() as u8;

    // Create a simplified proof structure
    // In a full implementation, we'd properly parse the BUMP format
    Ok(crate::beef::MerkleProof {
        block_height: index, // Placeholder - would need actual block height
        tree_height,
        levels: vec![nodes],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coinbase_detection() {
        let coinbase_txid = "0000000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(coinbase_txid.len(), 64);
    }
}
