//! Helper functions for BEEF/SPV caching operations
//!
//! Provides reusable functions for fetching data from APIs and managing cache operations.

use crate::cache_errors::{CacheError, CacheResult};
use crate::database::{BlockHeaderRepository, WalletDatabase};
use reqwest::Client;
use serde_json::Value;

/// Fetch parent transaction from WhatsOnChain API
pub async fn fetch_parent_transaction_from_api(
    client: &Client,
    txid: &str,
) -> CacheResult<String> {
    let tx_url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/hex", txid);
    let response = client.get(&tx_url).send().await
        .map_err(|e| CacheError::Api(format!("Failed to fetch parent tx {}: {}", txid, e)))?;

    if !response.status().is_success() {
        return Err(CacheError::Api(format!(
            "API returned status {} for tx {}", response.status(), txid
        )));
    }

    response.text().await
        .map_err(|e| CacheError::Api(format!("Failed to read parent tx response: {}", e)))
}

/// Fetch TSC Merkle proof - tries ARC first, falls back to WhatsOnChain
///
/// ARC returns merkle proofs in BUMP format (BRC-74) which we convert to TSC.
/// WhatsOnChain returns TSC format directly.
/// ARC is preferred because it's the same service we broadcast to, so it's
/// more likely to have proofs for recently-mined transactions.
pub async fn fetch_tsc_proof_from_api(
    client: &Client,
    txid: &str,
) -> CacheResult<Option<Value>> {
    // Try ARC first
    match fetch_tsc_proof_from_arc(client, txid).await {
        Ok(Some(tsc)) => {
            log::info!("   ✅ Got merkle proof from ARC for {}", txid);
            // Verify roundtrip: TSC → BUMP → check txid matches
            if verify_tsc_proof_roundtrip(txid, &tsc) {
                return Ok(Some(tsc));
            }
            log::warn!("   ⚠️  ARC proof failed roundtrip verification for {}, trying WhatsOnChain...", txid);
        }
        Ok(None) => {
            log::info!("   ℹ️  ARC has no merkle proof for {} (not yet mined), trying WhatsOnChain...", txid);
        }
        Err(e) => {
            log::warn!("   ⚠️  ARC merkle proof fetch failed for {}: {}, trying WhatsOnChain...", txid, e);
        }
    }

    // Fall back to WhatsOnChain
    match fetch_tsc_proof_from_whatsonchain(client, txid).await {
        Ok(Some(tsc)) => {
            // Verify roundtrip for WoC proof too
            if verify_tsc_proof_roundtrip(txid, &tsc) {
                return Ok(Some(tsc));
            }
            // If WoC proof fails with normal byte order, try WITHOUT reversing
            // (WoC may return natural byte order instead of display)
            log::warn!("   ⚠️  WoC proof failed roundtrip - attempting byte order fix for {}", txid);
            if let Some(fixed_tsc) = fix_tsc_byte_order(txid, &tsc) {
                log::info!("   ✅ Fixed WoC proof byte order for {}", txid);
                return Ok(Some(fixed_tsc));
            }
            log::error!("   ❌ Could not fix WoC proof byte order for {}", txid);
            Ok(Some(tsc)) // Return as-is, let caller handle
        }
        other => other,
    }
}

/// Fetch merkle proof from ARC and convert BUMP to TSC format
async fn fetch_tsc_proof_from_arc(
    client: &Client,
    txid: &str,
) -> CacheResult<Option<Value>> {
    let arc_response = crate::handlers::query_arc_tx_status(client, txid).await
        .map_err(|e| CacheError::Api(format!("ARC query failed: {}", e)))?;

    let status = arc_response.tx_status.as_deref().unwrap_or("UNKNOWN");

    if status != "MINED" {
        // Transaction not yet mined - no merkle proof available
        return Ok(None);
    }

    // Extract merklePath and convert from BUMP to TSC
    let merkle_path = match arc_response.merkle_path {
        Some(ref path) if !path.is_empty() => path,
        _ => return Ok(None),
    };

    let tsc = crate::beef::parse_bump_hex_to_tsc(merkle_path)
        .map_err(|e| CacheError::InvalidData(format!("Failed to parse ARC BUMP: {}", e)))?;

    // ARC's BUMP includes block_height but not the target hash.
    // The TSC from parse_bump_hex_to_tsc has height, index, nodes, and target="".
    // We need to add block height from the ARC response if not already present.
    let mut tsc = tsc;
    if let Some(height) = arc_response.block_height {
        tsc["height"] = serde_json::json!(height);
    }

    Ok(Some(tsc))
}

/// Fetch TSC Merkle proof from WhatsOnChain API (with retry logic for null proofs)
async fn fetch_tsc_proof_from_whatsonchain(
    client: &Client,
    txid: &str,
) -> CacheResult<Option<Value>> {
    let proof_url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/proof/tsc", txid);

    // First attempt
    let response = client.get(&proof_url).send().await
        .map_err(|e| CacheError::Api(format!("Failed to fetch TSC proof for {}: {}", txid, e)))?;

    if !response.status().is_success() {
        return Err(CacheError::Api(format!(
            "TSC proof API returned status {}", response.status()
        )));
    }

    let proof_text = response.text().await
        .map_err(|e| CacheError::Api(format!("Failed to read TSC proof response: {}", e)))?;

    let tsc_json: Value = serde_json::from_str(&proof_text)?;

    // If null, retry once after brief delay (transaction might be confirming)
    if tsc_json.is_null() {
        log::warn!("   ⚠️  TSC proof is null - retrying after 500ms...");
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let retry_response = client.get(&proof_url).send().await
            .map_err(|e| CacheError::Api(format!("Retry failed: {}", e)))?;

        if retry_response.status().is_success() {
            let retry_text = retry_response.text().await
                .map_err(|e| CacheError::Api(format!("Failed to read retry response: {}", e)))?;
            let retry_json: Value = serde_json::from_str(&retry_text)?;

            if retry_json.is_null() {
                return Ok(None); // Still null after retry
            }
            return Ok(Some(retry_json));
        }
        return Ok(None);
    }

    // Normalize array response to single object
    let tsc_obj = if tsc_json.is_array() {
        tsc_json.get(0).cloned().unwrap_or(tsc_json)
    } else {
        tsc_json
    };

    Ok(Some(tsc_obj))
}

/// Verify a TSC proof by doing a roundtrip: TSC → BUMP → check that the txid
/// appears at the expected position in level 0 of the BUMP.
///
/// This catches byte ordering mismatches between different proof sources
/// (ARC BUMP vs WhatsOnChain TSC) before they're stored in proven_txs.
fn verify_tsc_proof_roundtrip(txid: &str, tsc: &Value) -> bool {
    use crate::beef::{tsc_proof_to_bump, read_node_offset};

    let block_height = tsc["height"].as_u64().unwrap_or(0) as u32;
    let tx_index = tsc["index"].as_u64().unwrap_or(0);
    let nodes = match tsc["nodes"].as_array() {
        Some(n) => n,
        None => return false,
    };

    // Convert TSC to BUMP
    let bump = match tsc_proof_to_bump(txid, block_height, tx_index, nodes) {
        Ok(b) => b,
        Err(_) => return false,
    };

    // Check that the txid appears at level 0 with flag 0x02
    if bump.levels.is_empty() {
        return false;
    }

    let txid_bytes = match hex::decode(txid) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let mut txid_natural = txid_bytes;
    txid_natural.reverse(); // display → natural

    for node in &bump.levels[0] {
        if let Ok((_, vl)) = read_node_offset(node) {
            let flag = if node.len() > vl { node[vl] } else { 0 };
            if flag & 0x02 != 0 && node.len() >= vl + 33 {
                let hash = &node[vl + 1..vl + 33];
                if hash == txid_natural.as_slice() {
                    return true; // TXID found at correct position with correct hash
                }
            }
        }
    }

    false
}

/// Attempt to fix TSC proof byte ordering by reversing all node hashes.
///
/// If a proof source returns hashes in natural byte order instead of display,
/// we reverse them so that tsc_proof_to_bump (which expects display format)
/// produces the correct BUMP.
fn fix_tsc_byte_order(txid: &str, tsc: &Value) -> Option<Value> {
    let nodes = tsc["nodes"].as_array()?;
    let mut fixed_nodes = Vec::with_capacity(nodes.len());

    for node in nodes {
        let node_str = node.as_str()?;
        if node_str == "*" {
            fixed_nodes.push(serde_json::json!("*"));
            continue;
        }
        // Reverse the hex bytes
        let bytes = hex::decode(node_str).ok()?;
        let mut reversed = bytes;
        reversed.reverse();
        fixed_nodes.push(serde_json::json!(hex::encode(reversed)));
    }

    let mut fixed_tsc = tsc.clone();
    fixed_tsc["nodes"] = serde_json::json!(fixed_nodes);

    // Verify the fixed version works
    if verify_tsc_proof_roundtrip(txid, &fixed_tsc) {
        Some(fixed_tsc)
    } else {
        None
    }
}

/// Verify a TSC proof's merkle root against the actual block header.
///
/// Returns Ok(true) if the proof's computed root matches the block's merkle root,
/// Ok(false) if they don't match, or Err if the block header can't be fetched.
pub async fn verify_tsc_proof_against_block(
    client: &Client,
    txid: &str,
    tsc: &Value,
) -> CacheResult<bool> {
    let block_height = match tsc["height"].as_u64() {
        Some(h) if h > 0 => h as u32,
        _ => {
            // WoC TSC proofs often lack height — try target (block hash) to look it up
            if let Some(target) = tsc["target"].as_str().filter(|t| !t.is_empty()) {
                let header_url = format!("https://api.whatsonchain.com/v1/bsv/main/block/hash/{}", target);
                let resp = client.get(&header_url).send().await
                    .map_err(|e| CacheError::Api(format!("Failed to fetch block header for target {}: {}", target, e)))?;
                if !resp.status().is_success() {
                    return Err(CacheError::Api(format!("Block header API returned {} for target {}", resp.status(), target)));
                }
                let header_json: Value = resp.json().await
                    .map_err(|e| CacheError::Api(format!("Failed to parse block header JSON: {}", e)))?;
                match header_json["height"].as_u64() {
                    Some(h) if h > 0 => h as u32,
                    _ => return Err(CacheError::InvalidData("Could not resolve height from target block hash".to_string())),
                }
            } else {
                return Err(CacheError::InvalidData("No height or target in TSC proof".to_string()));
            }
        }
    };

    let tx_index = tsc["index"].as_u64().unwrap_or(0);
    let nodes = match tsc["nodes"].as_array() {
        Some(n) => n,
        None => return Ok(false),
    };

    // Compute merkle root from the proof
    let computed_root = match crate::beef::compute_merkle_root_from_tsc(txid, block_height, tx_index, nodes) {
        Ok(root) => root,
        Err(e) => {
            log::warn!("   Failed to compute merkle root for {}: {}", txid, e);
            return Ok(false);
        }
    };

    // Fetch the actual block header from WoC
    let block_url = format!("https://api.whatsonchain.com/v1/bsv/main/block/height/{}", block_height);
    let response = client.get(&block_url).send().await
        .map_err(|e| CacheError::Api(format!("Failed to fetch block {}: {}", block_height, e)))?;

    if !response.status().is_success() {
        return Err(CacheError::Api(format!(
            "Block API returned status {} for height {}", response.status(), block_height
        )));
    }

    let block_json: Value = response.json().await
        .map_err(|e| CacheError::Api(format!("Failed to parse block JSON: {}", e)))?;

    let actual_root = block_json["merkleroot"].as_str()
        .ok_or_else(|| CacheError::InvalidData("Missing merkleroot in block header".to_string()))?;

    if computed_root == actual_root {
        log::info!("   Merkle root verified for {} at height {}", txid, block_height);
        Ok(true)
    } else {
        log::error!("   Merkle root MISMATCH for {} at height {}!", txid, block_height);
        log::error!("      Computed: {}", computed_root);
        log::error!("      Actual:   {}", actual_root);
        Ok(false)
    }
}

/// Check block header cache for a known height (sync, no network)
pub fn get_cached_block_height(
    block_header_repo: &BlockHeaderRepository,
    target_hash: &str,
) -> CacheResult<Option<u32>> {
    match block_header_repo.get_by_hash(target_hash)? {
        Some(header) => Ok(Some(header.height)),
        None => Ok(None),
    }
}

/// Fetch block header from WhatsOnChain API and cache it (async, manages own lock)
pub async fn fetch_and_cache_block_header(
    client: &Client,
    db: &std::sync::Mutex<WalletDatabase>,
    target_hash: &str,
) -> CacheResult<u32> {
    let block_header_url = format!("https://api.whatsonchain.com/v1/bsv/main/block/hash/{}", target_hash);
    let response = client.get(&block_header_url).send().await
        .map_err(|e| CacheError::Api(format!("Failed to fetch block header: {}", e)))?;

    if !response.status().is_success() {
        return Err(CacheError::Api(format!(
            "Block header API returned status {}", response.status()
        )));
    }

    let header_json: serde_json::Value = response.json().await
        .map_err(|e| CacheError::Api(format!("Failed to parse block header JSON: {}", e)))?;

    let height = header_json["height"].as_u64()
        .ok_or_else(|| CacheError::InvalidData("Missing height in block header".to_string()))? as u32;

    // Brief lock to cache the header
    let header_hex = header_json["header"].as_str().unwrap_or("");
    {
        let db_guard = db.lock().unwrap();
        let block_header_repo = BlockHeaderRepository::new(db_guard.connection());
        block_header_repo.upsert(target_hash, height, header_hex)?;
    }

    Ok(height)
}

/// Verify that transaction bytes match expected TXID
pub fn verify_txid(tx_bytes: &[u8], expected_txid: &str) -> CacheResult<()> {
    use sha2::{Sha256, Digest};
    let hash1 = Sha256::digest(tx_bytes);
    let hash2 = Sha256::digest(&hash1);
    let calculated_txid: Vec<u8> = hash2.into_iter().rev().collect();
    let calculated_txid_hex = hex::encode(calculated_txid);

    if calculated_txid_hex != expected_txid {
        return Err(CacheError::InvalidData(format!(
            "TXID mismatch: expected {}, got {}", expected_txid, calculated_txid_hex
        )));
    }
    Ok(())
}

/// Get UTXO database ID for linking parent transactions
pub fn get_utxo_id_from_db(
    conn: &rusqlite::Connection,
    txid: &str,
    vout: u32,
) -> Result<Option<i64>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT outputId FROM outputs WHERE txid = ? AND vout = ? AND spendable = 1"
    )?;

    match stmt.query_row([txid, &vout.to_string()], |row| row.get::<_, i64>(0)) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}
