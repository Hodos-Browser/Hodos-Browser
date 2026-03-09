//! Helper functions for BEEF/SPV caching operations
//!
//! Provides reusable functions for fetching data from APIs and managing cache operations.

use crate::cache_errors::{CacheError, CacheResult};
use crate::database::BlockHeaderRepository;
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
            return Ok(Some(tsc));
        }
        Ok(None) => {
            log::info!("   ℹ️  ARC has no merkle proof for {} (not yet mined), trying WhatsOnChain...", txid);
        }
        Err(e) => {
            log::warn!("   ⚠️  ARC merkle proof fetch failed for {}: {}, trying WhatsOnChain...", txid, e);
        }
    }

    // Fall back to WhatsOnChain
    fetch_tsc_proof_from_whatsonchain(client, txid).await
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

/// Enhance TSC proof with block height (fetch from cache or API)
pub async fn enhance_tsc_with_height<'a>(
    client: &Client,
    block_header_repo: &'a BlockHeaderRepository<'a>,
    tsc_json: &Value,
) -> CacheResult<Value> {
    let target_hash = tsc_json["target"].as_str()
        .ok_or_else(|| CacheError::InvalidData("Missing target hash in TSC proof".to_string()))?;

    // Try cache first
    if let Some(header) = block_header_repo.get_by_hash(target_hash)? {
        let mut enhanced = tsc_json.clone();
        enhanced["height"] = serde_json::json!(header.height);
        return Ok(enhanced);
    }

    // Fetch from API
    let block_header_url = format!("https://api.whatsonchain.com/v1/bsv/main/block/hash/{}", target_hash);
    let response = client.get(&block_header_url).send().await
        .map_err(|e| CacheError::Api(format!("Failed to fetch block header: {}", e)))?;

    if !response.status().is_success() {
        return Err(CacheError::Api(format!(
            "Block header API returned status {}", response.status()
        )));
    }

    let header_json: Value = response.json().await
        .map_err(|e| CacheError::Api(format!("Failed to parse block header JSON: {}", e)))?;

    let height = header_json["height"].as_u64()
        .ok_or_else(|| CacheError::InvalidData("Missing height in block header".to_string()))? as u32;

    // Cache the header
    let header_hex = header_json["header"].as_str().unwrap_or("");
    block_header_repo.upsert(target_hash, height, header_hex)?;

    // Enhance TSC proof
    let mut enhanced = tsc_json.clone();
    enhanced["height"] = serde_json::json!(height);
    Ok(enhanced)
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
