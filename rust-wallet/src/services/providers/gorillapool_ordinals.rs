//! GorillaPool Ordinals provider — UTXO listing fallback.
//!
//! Position: 2nd-tier UTXO fetch per DESIGN §3 (after WoC). Used today as the
//! fallback inside `utxo_fetcher::fetch_utxos_for_address` (`utxo_fetcher.rs:178`).
//!
//! Only confirmed UTXOs are returned — GorillaPool Ordinals does not surface
//! unconfirmed UTXOs. For mempool-aware fetches, callers must use WoC.

use async_trait::async_trait;
use serde_json::Value;

use crate::services::provider::{IndexerError, IndexerProvider, ProviderOp};
use crate::utxo_fetcher::UTXO;

const NAME: &str = "gorillapool_ordinals";
const BASE: &str = "https://ordinals.gorillapool.io/api";

pub struct GorillaPoolOrdinalsProvider {
    client: reqwest::Client,
}

impl GorillaPoolOrdinalsProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl IndexerProvider for GorillaPoolOrdinalsProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn supports(&self, op: ProviderOp) -> bool {
        matches!(op, ProviderOp::FetchUtxos)
    }

    async fn fetch_utxos(&self, address: &str) -> Result<Vec<UTXO>, IndexerError> {
        let url = format!("{}/txos/address/{}/unspent", BASE, address);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 404 {
            // Treat 404 as "no UTXOs found" rather than NotFound (which would
            // short-circuit the chain). Address with no on-chain history is a normal
            // result, not a failure.
            return Ok(Vec::new());
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(IndexerError::ProviderStatus {
                provider: NAME,
                status: status.as_u16(),
                body,
            });
        }
        let json: Value = resp
            .json()
            .await
            .map_err(|e| IndexerError::InvalidResponse {
                provider: NAME,
                reason: format!("JSON parse error: {}", e),
            })?;
        parse_gorillapool_utxos(&json, address).map_err(|reason| IndexerError::InvalidResponse {
            provider: NAME,
            reason,
        })
    }
}

// --- Pure parse helper (unit-tested below) ---

/// Parse GorillaPool Ordinals `/txos/address/{addr}/unspent` response.
/// Shape: `[{txid, vout, satoshis, owner, spend, ...}, ...]`
/// Only entries with empty `spend` are kept (unspent).
pub(crate) fn parse_gorillapool_utxos(json: &Value, address: &str) -> Result<Vec<UTXO>, String> {
    let arr = json
        .as_array()
        .ok_or_else(|| "expected JSON array".to_string())?;
    let script = generate_p2pkh_script_from_address(address)?;

    let utxos = arr
        .iter()
        .filter_map(|e| {
            // Drop entries with a non-empty `spend` field — those are spent UTXOs.
            let spend = e.get("spend").and_then(|v| v.as_str()).unwrap_or("");
            if !spend.is_empty() {
                return None;
            }
            let txid = e.get("txid").and_then(|v| v.as_str())?.to_string();
            let vout = e.get("vout").and_then(|v| v.as_u64())? as u32;
            let satoshis = e.get("satoshis").and_then(|v| v.as_i64())?;
            Some(UTXO {
                txid,
                vout,
                satoshis,
                script: script.clone(),
                address_index: -1,
                custom_instructions: None,
                confirmed: true, // GorillaPool only returns confirmed UTXOs.
            })
        })
        .collect();

    Ok(utxos)
}

/// P2PKH script generation — same as in `whatsonchain.rs`. Mainnet only.
fn generate_p2pkh_script_from_address(address: &str) -> Result<String, String> {
    let decoded = bs58::decode(address)
        .with_check(None)
        .into_vec()
        .map_err(|e| format!("Invalid base58 address: {}", e))?;
    if decoded.len() != 21 {
        return Err(format!("Invalid decoded address length: {}", decoded.len()));
    }
    if decoded[0] != 0x00 {
        return Err(format!("Not a mainnet P2PKH address (version: 0x{:02x})", decoded[0]));
    }
    let pubkey_hash = &decoded[1..21];
    let mut script = Vec::new();
    script.push(0x76);
    script.push(0xa9);
    script.push(0x14);
    script.extend_from_slice(pubkey_hash);
    script.push(0x88);
    script.push(0xac);
    Ok(hex::encode(&script))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TEST_ADDR: &str = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";

    #[test]
    fn parse_unspent_entries_only() {
        let j = json!([
            {"txid": "aa", "vout": 0, "satoshis": 5000, "spend": ""},
            {"txid": "bb", "vout": 1, "satoshis": 1000, "spend": "spent-by-txid-cc"},
            {"txid": "dd", "vout": 0, "satoshis": 2000} // no spend field — also unspent
        ]);
        let utxos = parse_gorillapool_utxos(&j, TEST_ADDR).expect("should parse");
        assert_eq!(utxos.len(), 2, "spent entry must be filtered out");
        assert_eq!(utxos[0].txid, "aa");
        assert_eq!(utxos[1].txid, "dd");
    }

    #[test]
    fn parse_all_unspent_returned_as_confirmed() {
        let j = json!([{"txid": "aa", "vout": 0, "satoshis": 100, "spend": ""}]);
        let utxos = parse_gorillapool_utxos(&j, TEST_ADDR).expect("should parse");
        assert!(utxos[0].confirmed, "GP Ordinals UTXOs are always confirmed");
    }

    #[test]
    fn parse_returns_err_on_non_array() {
        let j = json!({"some": "object"});
        assert!(parse_gorillapool_utxos(&j, TEST_ADDR).is_err());
    }

    #[test]
    fn parse_empty_array_returns_empty_vec() {
        let j = json!([]);
        let utxos = parse_gorillapool_utxos(&j, TEST_ADDR).expect("should parse");
        assert!(utxos.is_empty());
    }

    #[test]
    fn supports_only_fetch_utxos() {
        let p = GorillaPoolOrdinalsProvider::new(reqwest::Client::new());
        assert!(p.supports(ProviderOp::FetchUtxos));
        for op in [
            ProviderOp::RawTx,
            ProviderOp::MerkleProof,
            ProviderOp::BlockHeader,
            ProviderOp::TxStatus,
            ProviderOp::Outspend,
            ProviderOp::BroadcastBeef,
        ] {
            assert!(!p.supports(op), "GP Ordinals must not claim {:?}", op);
        }
    }
}
