//! Helper functions for BEEF/SPV caching operations
//!
//! Provides reusable functions for fetching data from APIs and managing cache operations.

use crate::cache_errors::{CacheError, CacheResult};
use crate::database::{BlockHeaderRepository, WalletDatabase};
use crate::services::{BlockKey, IndexerError, WalletServices};
use reqwest::Client;
use serde_json::Value;

/// Map a Services `IndexerError` into the cache-layer `CacheError` taxonomy.
fn indexer_to_cache_err(e: IndexerError) -> CacheError {
    match e {
        IndexerError::InvalidResponse { reason, .. } => CacheError::InvalidData(reason),
        other => CacheError::Api(other.to_string()),
    }
}

/// Fetch parent transaction raw hex via the Services facade.
///
/// Phase 1.6d.C: was WoC-only (`/v1/bsv/main/tx/{txid}/hex`), now routes through the
/// 4-tier `WalletServices::get_raw_tx` chain (ARC GP → WoC → JungleBus → Bitails).
/// Soft timeout 8s per provider; demote-on-SoftTimeout. `NotFound` short-circuits the
/// chain (positive "tx doesn't exist" signal).
///
/// Returns the raw tx as hex string (preserved from the pre-1.6d.C contract so existing
/// callers don't need return-type changes).
pub async fn fetch_parent_transaction_from_api(
    services: &WalletServices,
    txid: &str,
) -> CacheResult<String> {
    let bytes = services
        .get_raw_tx(txid)
        .await
        .map_err(indexer_to_cache_err)?;
    Ok(hex::encode(bytes))
}

/// Fetch TSC Merkle proof via the Services facade with post-processing.
///
/// Phase 1.6d.C: was an inline ARC-primary + WoC-fallback implementation, now routes
/// through `WalletServices::get_merkle_proof_tsc` (ARC GP → WoC → JungleBus → Bitails;
/// ARC's BUMP→TSC conversion happens inside the provider impl). Soft timeout 10s.
///
/// Post-processing: roundtrip verifies the returned TSC, and on failure attempts a
/// byte-order fix (WoC quirk — sometimes returns natural-order hashes rather than
/// display-order). If both fail, returns the proof as-is (matches pre-1.6d.C behavior
/// — caller may still find it usable for the height/index parts even if nodes are
/// malformed).
///
/// `NotFound` from the chain → `Ok(None)` (proof not yet available / tx still in mempool).
pub async fn fetch_tsc_proof_from_api(
    services: &WalletServices,
    txid: &str,
) -> CacheResult<Option<Value>> {
    let tsc = match services.get_merkle_proof_tsc(txid).await {
        Ok(v) => v,
        Err(IndexerError::NotFound) => {
            log::info!("   ℹ️  No merkle proof available yet for {}", txid);
            return Ok(None);
        }
        Err(e) => return Err(indexer_to_cache_err(e)),
    };

    log::info!("   ✅ Got merkle proof for {} via Services chain", txid);

    if verify_tsc_proof_roundtrip(txid, &tsc) {
        return Ok(Some(tsc));
    }

    // Roundtrip failed — most commonly a WoC byte-order quirk. Reverse all node hashes
    // and verify again. If still failing, return as-is and let the caller handle.
    log::warn!("   ⚠️  TSC proof failed roundtrip — attempting byte-order fix for {}", txid);
    if let Some(fixed_tsc) = fix_tsc_byte_order(txid, &tsc) {
        log::info!("   ✅ Fixed TSC proof byte order for {}", txid);
        return Ok(Some(fixed_tsc));
    }

    log::error!("   ❌ Could not fix TSC proof byte order for {}", txid);
    Ok(Some(tsc))
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

/// Fetch block header via the Services facade and cache it (async, manages own lock).
///
/// Phase 1.6d.C: was WoC-only (`/v1/bsv/main/block/hash/{hash}`), now routes through
/// `WalletServices::get_block_header(BlockKey::Hash(...))` chain (WoC → JungleBus →
/// Bitails per DESIGN §3). Soft timeout 8s.
///
/// JungleBus's provider impl returns an empty `header_hex` (the JungleBus block_header
/// endpoint doesn't expose prev_hash, so the full 80-byte header can't be reconstructed).
/// The cache write tolerates an empty header_hex — the height is what most callers need.
/// Cache-no-poison invariant preserved: a failed fetch returns `Err` and writes nothing.
pub async fn fetch_and_cache_block_header(
    services: &WalletServices,
    db: &std::sync::Mutex<WalletDatabase>,
    target_hash: &str,
) -> CacheResult<u32> {
    let header = services
        .get_block_header(BlockKey::Hash(target_hash.to_string()))
        .await
        .map_err(indexer_to_cache_err)?;

    // Brief lock to cache the header. Empty header_hex is acceptable — JungleBus
    // returns it that way and callers that only need height are unaffected.
    {
        let db_guard = db.lock().unwrap();
        let block_header_repo = BlockHeaderRepository::new(db_guard.connection());
        block_header_repo.upsert(target_hash, header.height, &header.header_hex)?;
    }

    Ok(header.height)
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
