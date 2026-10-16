//! Helper functions for BEEF/SPV caching operations
//!
//! Provides reusable functions for fetching data from APIs and managing cache operations.

use crate::cache_errors::{CacheError, CacheResult};
use crate::services::{IndexerError, WalletServices};
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
/// display-order). If both fail, the proof is an error (B5-T1-P7a-2 `C13`: a proof that does not
/// put the txid where it claims is not a proof).
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
    Err(CacheError::InvalidData(format!("merkle proof for {} does not contain the transaction at its index", txid)))
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
