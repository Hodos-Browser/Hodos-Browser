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

/// WhatsOnChain API response format
#[derive(Debug, Deserialize)]
struct WhatsOnChainUTXO {
    tx_hash: String,
    tx_pos: u32,
    value: i64,
    #[serde(default)]
    script: String,
}

/// Fetch UTXOs for a Bitcoin address from WhatsOnChain
///
/// API: https://api.whatsonchain.com/v1/bsv/main/address/{address}/unspent
///
/// Retries on 500 errors (server errors) with exponential backoff.
pub async fn fetch_utxos_for_address(address: &str, address_index: i32) -> Result<Vec<UTXO>, String> {
    const MAX_RETRIES: u32 = 3;
    const INITIAL_DELAY_MS: u64 = 1000; // 1 second

    let url = format!("https://api.whatsonchain.com/v1/bsv/main/address/{}/unspent", address);

    log::info!("   Fetching UTXOs for address: {}", address);
    log::debug!("   WhatsOnChain URL: {}", url);

    let client = reqwest::Client::new();

    // Retry loop for server errors (500, 502, 503, 504)
    for attempt in 0..=MAX_RETRIES {
        let response = match client.get(&url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                if attempt < MAX_RETRIES {
                    let delay_ms = INITIAL_DELAY_MS * (1 << attempt); // Exponential backoff
                    log::warn!("   Request failed (attempt {}/{}): {}. Retrying in {}ms...",
                              attempt + 1, MAX_RETRIES + 1, e, delay_ms);
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    continue;
                }
                return Err(format!("WhatsOnChain API request failed after {} attempts: {}", MAX_RETRIES + 1, e));
            }
        };

        let status = response.status();

        // Check status
        if status.is_success() {
            // Success - parse and return
            // Parse response
            let api_utxos: Vec<WhatsOnChainUTXO> = match response.json().await {
                Ok(utxos) => utxos,
                Err(e) => {
                    return Err(format!("Failed to parse WhatsOnChain response: {}", e));
                }
            };

            // Convert to our UTXO format
            // Generate P2PKH locking script from address (WhatsOnChain doesn't return it)
            let p2pkh_script = generate_p2pkh_script_from_address(address)?;

            let utxos: Vec<UTXO> = api_utxos.into_iter().map(|u| UTXO {
                txid: u.tx_hash,
                vout: u.tx_pos,
                satoshis: u.value,
                script: p2pkh_script.clone(), // Use generated P2PKH script
                address_index, // Track which address owns this UTXO
                custom_instructions: None, // HD wallet addresses don't need custom instructions
            }).collect();

            log::info!("   ✅ Fetched {} UTXOs ({} satoshis total)",
                utxos.len(),
                utxos.iter().map(|u| u.satoshis).sum::<i64>()
            );

            return Ok(utxos);
        } else if status.as_u16() == 404 {
            // Address never used or no UTXOs - not an error
            log::info!("   No UTXOs found (address unused or spent)");
            return Ok(Vec::new());
        } else if status.is_server_error() && attempt < MAX_RETRIES {
            // Server error (500, 502, 503, 504) - retry with exponential backoff
            let delay_ms = INITIAL_DELAY_MS * (1 << attempt);
            log::warn!("   Server error {} (attempt {}/{}). Retrying in {}ms...",
                      status, attempt + 1, MAX_RETRIES + 1, delay_ms);
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            continue;
        } else {
            // Client error (4xx) or max retries reached - don't retry
            return Err(format!("WhatsOnChain API returned status {}", status));
        }
    }

    // Should never reach here, but just in case
    Err(format!("WhatsOnChain API request failed after {} attempts", MAX_RETRIES + 1))
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

/// Fetch UTXOs for all addresses in the wallet
pub async fn fetch_all_utxos(addresses: &[crate::json_storage::AddressInfo]) -> Result<Vec<UTXO>, String> {
    let mut all_utxos = Vec::new();

    for (idx, addr) in addresses.iter().enumerate() {
        // Always check all addresses - balance cache may be stale
        match fetch_utxos_for_address(&addr.address, addr.index).await {
            Ok(mut utxos) => {
                all_utxos.append(&mut utxos);
            }
            Err(e) => {
                log::warn!("   Failed to fetch UTXOs for {}: {}", addr.address, e);
                // Continue with other addresses
            }
        }

        // Add small delay between requests to avoid rate limiting
        // (only if not the last address)
        if idx < addresses.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    log::info!("📊 Total UTXOs across all addresses: {} ({} satoshis)",
        all_utxos.len(),
        all_utxos.iter().map(|u| u.satoshis).sum::<i64>()
    );

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
