//! UTXO Fetcher
//!
//! Fetches unspent transaction outputs from blockchain APIs.
//! Based on go-wallet/utxo_manager.go

use serde::{Deserialize, Serialize};

/// UTXO structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UTXO {
    pub txid: String,
    pub vout: u32,
    pub satoshis: i64,
    pub script: String, // Hex-encoded locking script
    pub address_index: i32, // Which address owns this UTXO (negative = derived, -1 = master)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_instructions: Option<String>, // BRC-29 derivation info for spending derived UTXOs
}

/// WhatsOnChain API response format (single-address endpoint)
#[derive(Debug, Deserialize)]
struct WhatsOnChainUTXO {
    tx_hash: String,
    tx_pos: u32,
    value: i64,
}

/// WhatsOnChain /unspent/all wrapper response
#[derive(Debug, Deserialize)]
struct WhatsOnChainUnspentAllResponse {
    #[serde(default)]
    result: Vec<WhatsOnChainUTXO>,
    #[serde(default)]
    error: String,
}

/// WhatsOnChain bulk endpoint response item
/// New API uses `result` field; old API used `unspent`. We accept both.
#[derive(Debug, Deserialize)]
struct WhatsOnChainBulkItem {
    address: String,
    #[serde(default)]
    result: Vec<WhatsOnChainUTXO>,
    #[serde(default)]
    unspent: Vec<WhatsOnChainUTXO>,
    #[serde(default)]
    error: String,
}

impl WhatsOnChainBulkItem {
    /// Get UTXOs from whichever field has data (new API uses `result`, old used `unspent`)
    fn utxos(&self) -> &Vec<WhatsOnChainUTXO> {
        if !self.result.is_empty() { &self.result } else { &self.unspent }
    }
}

/// GorillaPool ordinals API response format
#[derive(Debug, Deserialize)]
struct GorillaPoolUTXO {
    txid: String,
    vout: u32,
    satoshis: i64,
    #[serde(default)]
    owner: String,
}

/// Max addresses per bulk API request (WhatsOnChain limit)
const BULK_BATCH_SIZE: usize = 20;

/// Fetch UTXOs for a Bitcoin address from blockchain APIs.
///
/// Tries WhatsOnChain first, falls back to GorillaPool ordinals API.
/// Returns Ok(vec) on success (empty vec = genuinely no UTXOs).
/// Returns Err only if ALL providers failed (API down).
pub async fn fetch_utxos_for_address(address: &str, address_index: i32) -> Result<Vec<UTXO>, String> {
    log::info!("   Fetching UTXOs for address: {}", address);

    let client = reqwest::Client::new();

    // Try WhatsOnChain first
    match fetch_utxos_woc(&client, address, address_index).await {
        Ok(utxos) => return Ok(utxos),
        Err(e) => {
            log::debug!("   WoC failed for {}: {}, trying GorillaPool...", address, e);
        }
    }

    // Fallback: GorillaPool ordinals API (different provider)
    match fetch_utxos_gorillapool(&client, address, address_index).await {
        Ok(utxos) => return Ok(utxos),
        Err(e) => {
            log::warn!("   ⚠️  All UTXO APIs failed for {}: {}", address, e);
        }
    }

    Err(format!("All UTXO API providers failed for address {}", address))
}

/// Fetch UTXOs from WhatsOnChain
async fn fetch_utxos_woc(client: &reqwest::Client, address: &str, address_index: i32) -> Result<Vec<UTXO>, String> {
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/address/{}/unspent/all", address);

    let response = client.get(&url).send().await
        .map_err(|e| format!("WoC request failed: {}", e))?;

    let status = response.status();
    if status.is_success() {
        // New /unspent/all endpoint returns {result: [...], error: ""}
        // Try wrapped format first, fall back to flat array
        let api_utxos: Vec<WhatsOnChainUTXO> = {
            let body = response.text().await
                .map_err(|e| format!("Failed to read WoC response: {}", e))?;
            if let Ok(wrapped) = serde_json::from_str::<WhatsOnChainUnspentAllResponse>(&body) {
                if !wrapped.error.is_empty() {
                    return Err(format!("WoC error: {}", wrapped.error));
                }
                wrapped.result
            } else if let Ok(flat) = serde_json::from_str::<Vec<WhatsOnChainUTXO>>(&body) {
                flat
            } else {
                return Err(format!("Failed to parse WoC response: {}", &body[..body.len().min(200)]));
            }
        };

        let p2pkh_script = generate_p2pkh_script_from_address(address)?;
        let utxos: Vec<UTXO> = api_utxos.into_iter().map(|u| UTXO {
            txid: u.tx_hash,
            vout: u.tx_pos,
            satoshis: u.value,
            script: p2pkh_script.clone(),
            address_index,
            custom_instructions: None,
        }).collect();

        log::info!("   ✅ WoC: {} UTXOs ({} sats)", utxos.len(), utxos.iter().map(|u| u.satoshis).sum::<i64>());
        Ok(utxos)
    } else {
        Err(format!("WoC returned status {}", status))
    }
}

/// Fetch UTXOs from GorillaPool ordinals API (fallback)
///
/// API: https://ordinals.gorillapool.io/api/txos/address/{address}/unspent
/// Returns: [{txid, vout, satoshis, owner, spend, ...}]
/// Only includes UTXOs where spend is empty (unspent).
async fn fetch_utxos_gorillapool(client: &reqwest::Client, address: &str, address_index: i32) -> Result<Vec<UTXO>, String> {
    let url = format!("https://ordinals.gorillapool.io/api/txos/address/{}/unspent", address);

    let response = client.get(&url).send().await
        .map_err(|e| format!("GorillaPool request failed: {}", e))?;

    let status = response.status();
    if status.is_success() {
        let api_utxos: Vec<GorillaPoolUTXO> = response.json().await
            .map_err(|e| format!("Failed to parse GorillaPool response: {}", e))?;

        let p2pkh_script = generate_p2pkh_script_from_address(address)?;
        let utxos: Vec<UTXO> = api_utxos.into_iter().map(|u| UTXO {
            txid: u.txid,
            vout: u.vout,
            satoshis: u.satoshis,
            script: p2pkh_script.clone(),
            address_index,
            custom_instructions: None,
        }).collect();

        log::info!("   ✅ GorillaPool: {} UTXOs ({} sats)", utxos.len(), utxos.iter().map(|u| u.satoshis).sum::<i64>());
        Ok(utxos)
    } else {
        Err(format!("GorillaPool returned status {}", status))
    }
}

/// Check if an address has any transaction history on-chain.
///
/// Uses WhatsOnChain's confirmed history endpoint which returns transactions
/// even if the address currently has zero balance. This distinguishes "used
/// and spent" from "never used" — critical for gap limit logic during recovery.
///
/// API (new): https://api.whatsonchain.com/v1/bsv/main/address/{address}/confirmed/history
/// API (old, deprecated): https://api.whatsonchain.com/v1/bsv/main/address/{address}/history
pub async fn address_has_history(address: &str) -> Result<bool, String> {
    let client = reqwest::Client::new();

    // Try new WoC endpoint first: /address/{addr}/confirmed/history
    // Returns: {"address":"...","result":[{"tx_hash":"...","height":...}, ...],"error":""}
    let new_url = format!("https://api.whatsonchain.com/v1/bsv/main/address/{}/confirmed/history", address);
    match client.get(&new_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            // Parse the wrapped response — history is in the "result" array
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                let result = json.get("result").and_then(|v| v.as_array());
                return Ok(result.map(|a| !a.is_empty()).unwrap_or(false));
            }
            // Fallback: check if body has any tx_hash references
            return Ok(body.contains("tx_hash"));
        }
        Ok(resp) if resp.status().as_u16() == 404 => {
            // New endpoint also 404 — try legacy endpoint
            log::debug!("   WoC confirmed/history returned 404 for {}, trying legacy", address);
        }
        Ok(resp) => {
            log::warn!("   ⚠️  WoC confirmed/history returned {} for {}", resp.status(), address);
        }
        Err(e) => {
            log::warn!("   ⚠️  WoC confirmed/history request failed for {}: {}", address, e);
        }
    }

    // Fallback: try legacy endpoint /address/{addr}/history
    let legacy_url = format!("https://api.whatsonchain.com/v1/bsv/main/address/{}/history", address);
    match client.get(&legacy_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            let has_history = body.trim() != "[]" && !body.trim().is_empty();
            Ok(has_history)
        }
        Ok(resp) if resp.status().as_u16() == 404 => {
            // Both endpoints 404 — address genuinely has no history
            Ok(false)
        }
        Ok(resp) => {
            Err(format!("History API returned status {}", resp.status()))
        }
        Err(e) => {
            Err(format!("History API request failed: {}", e))
        }
    }
}

/// Generate P2PKH locking script from a Bitcoin address
///
/// Decodes the address and creates: OP_DUP OP_HASH160 <pubkeyhash> OP_EQUALVERIFY OP_CHECKSIG
fn generate_p2pkh_script_from_address(address: &str) -> Result<String, String> {
    // Decode base58check address (with checksum verification)
    let decoded = bs58::decode(address)
        .with_check(None) // Verify checksum
        .into_vec()
        .map_err(|e| format!("Invalid base58 address: {}", e))?;

    // After checksum removal, we should have: 1 byte version + 20 bytes hash = 21 bytes
    if decoded.len() != 21 {
        return Err(format!("Invalid decoded address length: {} (expected 21)", decoded.len()));
    }

    // Verify it's a mainnet P2PKH address (version byte 0x00)
    if decoded[0] != 0x00 {
        return Err(format!("Not a mainnet P2PKH address (version: 0x{:02x})", decoded[0]));
    }

    // Extract pubkey hash (skip version byte, take next 20 bytes)
    let pubkey_hash = &decoded[1..21];

    // Build P2PKH script: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
    let mut script = Vec::new();
    script.push(0x76); // OP_DUP
    script.push(0xa9); // OP_HASH160
    script.push(0x14); // Push 20 bytes
    script.extend_from_slice(pubkey_hash);
    script.push(0x88); // OP_EQUALVERIFY
    script.push(0xac); // OP_CHECKSIG

    let script_hex = hex::encode(&script);
    log::debug!("   Generated P2PKH script for {}: {}", address, script_hex);
    log::debug!("   Pubkey hash: {}", hex::encode(pubkey_hash));

    Ok(script_hex)
}

/// Fetch UTXOs for multiple addresses using WhatsOnChain bulk endpoint.
///
/// POST https://api.whatsonchain.com/v1/bsv/main/addresses/confirmed/unspent
/// Body: { "addresses": ["addr1", "addr2", ...] }  (max 20)
///
/// Chunks addresses into groups of 20, retries on failure, falls back to
/// single-address fetching if the bulk endpoint returns an error.
/// Returns (utxos, success_count) where success_count is the number of addresses
/// that were successfully checked (got a 200 response).
async fn fetch_utxos_bulk(addresses: &[crate::json_storage::AddressInfo]) -> Result<(Vec<UTXO>, usize), String> {
    const MAX_RETRIES: u32 = 3;
    const INITIAL_DELAY_MS: u64 = 1000;

    let client = reqwest::Client::new();
    let mut all_utxos = Vec::new();
    let mut total_success_count: usize = 0;

    // Build address→index lookup
    let addr_to_index: std::collections::HashMap<&str, i32> = addresses.iter()
        .map(|a| (a.address.as_str(), a.index))
        .collect();

    // Pre-generate P2PKH scripts for each address
    let mut addr_to_script: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    for addr in addresses {
        match generate_p2pkh_script_from_address(&addr.address) {
            Ok(script) => { addr_to_script.insert(&addr.address, script); }
            Err(e) => {
                log::warn!("   Failed to generate script for {}: {}", addr.address, e);
            }
        }
    }

    let chunks: Vec<&[crate::json_storage::AddressInfo]> = addresses.chunks(BULK_BATCH_SIZE).collect();

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        let addr_list: Vec<&str> = chunk.iter().map(|a| a.address.as_str()).collect();

        log::info!("   Bulk UTXO fetch: chunk {}/{} ({} addresses)",
                  chunk_idx + 1, chunks.len(), addr_list.len());

        let body = serde_json::json!({ "addresses": addr_list });
        let mut bulk_ok = false;

        // Retry loop for bulk endpoint
        for attempt in 0..=MAX_RETRIES {
            let response = match client.post("https://api.whatsonchain.com/v1/bsv/main/addresses/confirmed/unspent")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        let delay_ms = INITIAL_DELAY_MS * (1 << attempt);
                        log::warn!("   Bulk request failed (attempt {}/{}): {}. Retrying in {}ms...",
                                  attempt + 1, MAX_RETRIES + 1, e, delay_ms);
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                        continue;
                    }
                    log::warn!("   Bulk request failed after {} attempts: {}", MAX_RETRIES + 1, e);
                    break;
                }
            };

            let status = response.status();
            if status.is_success() {
                match response.json::<Vec<WhatsOnChainBulkItem>>().await {
                    Ok(items) => {
                        for item in items {
                            if !item.error.is_empty() {
                                log::warn!("   Bulk API error for {}: {}", item.address, item.error);
                                continue;
                            }
                            total_success_count += 1; // This address was successfully checked
                            let address_index = addr_to_index.get(item.address.as_str()).copied().unwrap_or(0);
                            let script = match addr_to_script.get(item.address.as_str()) {
                                Some(s) => s.clone(),
                                None => continue,
                            };
                            for u in item.utxos() {
                                all_utxos.push(UTXO {
                                    txid: u.tx_hash.clone(),
                                    vout: u.tx_pos,
                                    satoshis: u.value,
                                    script: script.clone(),
                                    address_index,
                                    custom_instructions: None,
                                });
                            }
                        }
                        bulk_ok = true;
                        break;
                    }
                    Err(e) => {
                        log::warn!("   Failed to parse bulk response: {}", e);
                        break; // Fall through to single-address fallback
                    }
                }
            } else if status.is_server_error() && attempt < MAX_RETRIES {
                let delay_ms = INITIAL_DELAY_MS * (1 << attempt);
                log::warn!("   Bulk server error {} (attempt {}/{}). Retrying in {}ms...",
                          status, attempt + 1, MAX_RETRIES + 1, delay_ms);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                continue;
            } else {
                log::warn!("   Bulk endpoint returned status {}", status);
                break; // Fall through to single-address fallback
            }
        }
        // Fallback: single-address fetch for this chunk if bulk failed
        if !bulk_ok {
            log::info!("   Falling back to single-address fetch for {} addresses", chunk.len());
            for (i, addr) in chunk.iter().enumerate() {
                match fetch_utxos_for_address(&addr.address, addr.index).await {
                    Ok(mut utxos) => {
                        total_success_count += 1; // This address was successfully checked
                        all_utxos.append(&mut utxos);
                    }
                    Err(e) => log::warn!("   Failed to fetch UTXOs for {}: {}", addr.address, e),
                }
                if i < chunk.len() - 1 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }

        // Small delay between chunks to avoid rate limiting
        if chunk_idx < chunks.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
    }

    Ok((all_utxos, total_success_count))
}

/// Fetch UTXOs for all addresses in the wallet.
///
/// Uses bulk API (POST /addresses/unspent, 20 addresses/request) for speed.
/// Falls back to single-address fetch if bulk endpoint fails.
///
/// Returns Err if NO addresses could be successfully checked (API is down).
/// Returns Ok with empty vec only if addresses were checked and genuinely have no UTXOs.
pub async fn fetch_all_utxos(addresses: &[crate::json_storage::AddressInfo]) -> Result<Vec<UTXO>, String> {
    let (all_utxos, success_count) = fetch_utxos_bulk(addresses).await?;

    log::info!("📊 Total UTXOs across all addresses: {} ({} satoshis), {}/{} addresses checked successfully",
        all_utxos.len(),
        all_utxos.iter().map(|u| u.satoshis).sum::<i64>(),
        success_count,
        addresses.len(),
    );

    // If no addresses were successfully checked, the API is likely down.
    // Return Err so callers don't reconcile against empty results.
    if success_count == 0 && !addresses.is_empty() {
        return Err("UTXO API unavailable — no addresses could be checked".to_string());
    }

    Ok(all_utxos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_utxos_nonexistent_address() {
        // Test address with no UTXOs (Satoshi's genesis address)
        let result = fetch_utxos_for_address("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", 0).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }
}
