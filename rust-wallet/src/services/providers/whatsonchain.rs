//! WhatsOnChain provider — broadest API coverage (all 7 ops). Mainnet-only.
//!
//! Lifted from existing call sites in `cache_helpers.rs`, `utxo_fetcher.rs`,
//! `handlers.rs`. Phase 1.6d.B step 4.

use async_trait::async_trait;
use serde_json::Value;

use crate::services::collection::ProviderCollection;
use crate::services::provider::{
    BlockHeader, BlockKey, BroadcastResult, IndexerError, IndexerProvider, OutspendStatus,
    ProviderOp, TxState, TxStatus,
};
use crate::utxo_fetcher::UTXO;

const NAME: &str = "whatsonchain";
const BASE: &str = "https://api.whatsonchain.com/v1/bsv/main";

pub struct WhatsOnChainProvider {
    client: reqwest::Client,
}

impl WhatsOnChainProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    fn url(path: &str) -> String {
        format!("{}{}", BASE, path)
    }

    async fn get_text(&self, path: &str) -> Result<String, IndexerError> {
        let resp = self
            .client
            .get(Self::url(path))
            .send()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 404 {
            return Err(IndexerError::NotFound);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(IndexerError::ProviderStatus {
                provider: NAME,
                status: status.as_u16(),
                body,
            });
        }
        resp.text()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))
    }

    async fn get_json(&self, path: &str) -> Result<Value, IndexerError> {
        let text = self.get_text(path).await?;
        serde_json::from_str(&text).map_err(|e| IndexerError::InvalidResponse {
            provider: NAME,
            reason: format!("JSON parse error: {}", e),
        })
    }
}

#[async_trait]
impl IndexerProvider for WhatsOnChainProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn supports(&self, op: ProviderOp) -> bool {
        // WoC supports everything — broadcast goes through tx/raw on the main tx.
        match op {
            ProviderOp::RawTx
            | ProviderOp::MerkleProof
            | ProviderOp::BlockHeader
            | ProviderOp::TxStatus
            | ProviderOp::Outspend
            | ProviderOp::FetchUtxos
            | ProviderOp::BroadcastBeef => true,
        }
    }

    async fn get_raw_tx(&self, txid: &str) -> Result<Vec<u8>, IndexerError> {
        let hex_str = self.get_text(&format!("/tx/{}/hex", txid)).await?;
        let hex_str = hex_str.trim();
        hex::decode(hex_str).map_err(|e| IndexerError::InvalidResponse {
            provider: NAME,
            reason: format!("hex decode error: {}", e),
        })
    }

    async fn get_merkle_proof_tsc(&self, txid: &str) -> Result<Value, IndexerError> {
        // WoC may return `null` (proof not yet available), an object, or an array.
        // Normalize array → first element; null → NotFound (treat as "no proof yet").
        let json = self.get_json(&format!("/tx/{}/proof/tsc", txid)).await?;
        parse_tsc_proof(json).ok_or(IndexerError::NotFound)
    }

    async fn get_block_header(&self, key: BlockKey) -> Result<BlockHeader, IndexerError> {
        // Two-hop:
        //   - If by Height: /block/height/{h} → hash, then /block/hash/{hash}/header → header hex
        //   - If by Hash:   /block/hash/{hash}/header → header hex, plus we still need height
        //                   which comes from /block/hash/{hash}
        let (block_hash, height) = match &key {
            BlockKey::Height(h) => {
                let info = self.get_json(&format!("/block/height/{}", h)).await?;
                let hash = info
                    .get("hash")
                    .and_then(|v| v.as_str())
                    .ok_or(IndexerError::InvalidResponse {
                        provider: NAME,
                        reason: "missing 'hash' in block/height response".into(),
                    })?
                    .to_string();
                (hash, *h)
            }
            BlockKey::Hash(hash) => {
                let info = self.get_json(&format!("/block/hash/{}", hash)).await?;
                let height = info
                    .get("height")
                    .and_then(|v| v.as_u64())
                    .ok_or(IndexerError::InvalidResponse {
                        provider: NAME,
                        reason: "missing 'height' in block/hash response".into(),
                    })? as u32;
                (hash.clone(), height)
            }
        };

        let header_hex = self
            .get_text(&format!("/block/hash/{}/header", block_hash))
            .await?
            .trim()
            .to_string();

        Ok(BlockHeader {
            block_hash,
            height,
            header_hex,
        })
    }

    async fn tx_status(&self, txid: &str) -> Result<TxStatus, IndexerError> {
        let json = self.get_json(&format!("/tx/hash/{}", txid)).await?;
        Ok(parse_tx_status(txid, &json))
    }

    async fn outspend(&self, txid: &str, vout: u32) -> Result<OutspendStatus, IndexerError> {
        // WoC: 404 = spent (or doesn't exist), 200 with {"spent": bool} → status.
        // Per the existing certificate_handlers.rs:2982 pattern, treat 404 as Spent.
        match self
            .get_json(&format!("/tx/{}/outspend/{}", txid, vout))
            .await
        {
            Ok(json) => Ok(parse_outspend(json)),
            Err(IndexerError::NotFound) => Ok(OutspendStatus::Spent {
                spending_txid: String::new(), // WoC 404 doesn't tell us who spent it.
                spending_vin: None,
            }),
            Err(e) => Err(e),
        }
    }

    async fn fetch_utxos(&self, address: &str) -> Result<Vec<UTXO>, IndexerError> {
        let json = self
            .get_json(&format!("/address/{}/unspent/all", address))
            .await?;
        parse_woc_utxos(&json, address).map_err(|reason| IndexerError::InvalidResponse {
            provider: NAME,
            reason,
        })
    }

    async fn broadcast_beef(&self, beef: &[u8]) -> Result<BroadcastResult, IndexerError> {
        // WoC's broadcast endpoint expects raw tx hex of the *main* tx, not BEEF.
        // Extract the main tx from the BEEF wrapper.
        let parsed = crate::beef::Beef::from_bytes(beef).map_err(|e| {
            IndexerError::InvalidResponse {
                provider: NAME,
                reason: format!("BEEF parse error: {}", e),
            }
        })?;
        let main_tx = parsed.main_transaction().ok_or(IndexerError::InvalidResponse {
            provider: NAME,
            reason: "BEEF has no main transaction".to_string(),
        })?;
        let raw_tx_hex = hex::encode(main_tx);

        let body = serde_json::json!({ "txhex": raw_tx_hex });
        let resp = self
            .client
            .post(Self::url("/tx/raw"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        if status.is_success() {
            let txid = text.trim().trim_matches('"').to_string();
            Ok(BroadcastResult {
                provider: NAME,
                txid,
                tx_status: "SEEN_ON_NETWORK".to_string(),
                merkle_path_bump: None,
                block_height: None,
            })
        } else {
            // Treat "already in mempool" / "already known" as success per the existing
            // handlers.rs:8992 logic.
            let lower = text.to_lowercase();
            if lower.contains("already in")
                || lower.contains("already known")
                || lower.contains("duplicate")
                || lower.contains("txn-already-in-mempool")
                || lower.contains("txn-already-known")
            {
                Ok(BroadcastResult {
                    provider: NAME,
                    txid: String::new(), // WoC error path doesn't return the txid.
                    tx_status: "ALREADY_KNOWN".to_string(),
                    merkle_path_bump: None,
                    block_height: None,
                })
            } else {
                Err(IndexerError::ProviderStatus {
                    provider: NAME,
                    status: status.as_u16(),
                    body: text,
                })
            }
        }
    }
}

// --- Pure parse helpers (unit-tested below) ---

/// Normalize a WoC TSC proof response: null → None; array → first element wrapped;
/// object → wrapped as-is.
pub(crate) fn parse_tsc_proof(json: Value) -> Option<Value> {
    if json.is_null() {
        return None;
    }
    if json.is_array() {
        return json.as_array().and_then(|a| a.get(0).cloned());
    }
    Some(json)
}

/// Parse WoC `/tx/hash/{txid}` response into a normalized `TxStatus`. WoC reports
/// `confirmations` and `blockheight`/`blockhash` — confirmations=0 means mempool;
/// missing `blockheight` also means mempool.
pub(crate) fn parse_tx_status(txid: &str, json: &Value) -> TxStatus {
    let confirmations = json.get("confirmations").and_then(|v| v.as_u64()).unwrap_or(0);
    let block_height = json.get("blockheight").and_then(|v| v.as_u64()).map(|h| h as u32);
    let block_hash = json
        .get("blockhash")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let state = if confirmations >= 1 || block_height.is_some() {
        TxState::Mined
    } else {
        TxState::InMempool
    };

    TxStatus {
        txid: txid.to_string(),
        state,
        block_height,
        block_hash,
        merkle_path_bump: None, // WoC doesn't return BUMP here.
    }
}

/// Parse WoC `/tx/{txid}/outspend/{vout}` response.
pub(crate) fn parse_outspend(json: Value) -> OutspendStatus {
    let spent = json.get("spent").and_then(|v| v.as_bool()).unwrap_or(false);
    if spent {
        let spending_txid = json
            .get("txid")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
        OutspendStatus::Spent {
            spending_txid,
            spending_vin: json
                .get("vin")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
        }
    } else {
        OutspendStatus::Unspent
    }
}

/// Parse WoC `/address/{addr}/unspent/all` response. Accepts both the wrapped
/// `{result: [...], error: ""}` shape and the legacy flat array.
pub(crate) fn parse_woc_utxos(json: &Value, address: &str) -> Result<Vec<UTXO>, String> {
    let script = generate_p2pkh_script_from_address(address)?;

    let entries: Vec<Value> = if let Some(obj) = json.as_object() {
        if let Some(err) = obj.get("error").and_then(|v| v.as_str()) {
            if !err.is_empty() {
                return Err(format!("WoC error: {}", err));
            }
        }
        obj.get("result")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    } else if let Some(arr) = json.as_array() {
        arr.clone()
    } else {
        return Err("response was neither object nor array".into());
    };

    let utxos = entries
        .into_iter()
        .filter_map(|e| {
            let txid = e.get("tx_hash").and_then(|v| v.as_str())?.to_string();
            let vout = e.get("tx_pos").and_then(|v| v.as_u64())? as u32;
            let satoshis = e.get("value").and_then(|v| v.as_i64())?;
            let height = e.get("height").and_then(|v| v.as_i64()).unwrap_or(0);
            Some(UTXO {
                txid,
                vout,
                satoshis,
                script: script.clone(),
                address_index: -1,
                custom_instructions: None,
                confirmed: height > 0,
            })
        })
        .collect();

    Ok(utxos)
}

/// P2PKH script generation lifted from `utxo_fetcher::generate_p2pkh_script_from_address`.
/// Duplicated here to avoid bringing a `pub` on the original; this trait impl is the
/// only out-of-module caller and the function is small.
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
    script.push(0x76); // OP_DUP
    script.push(0xa9); // OP_HASH160
    script.push(0x14); // Push 20 bytes
    script.extend_from_slice(pubkey_hash);
    script.push(0x88); // OP_EQUALVERIFY
    script.push(0xac); // OP_CHECKSIG
    Ok(hex::encode(&script))
}

// Suppress unused-warning on ProviderCollection import — it's used transitively by
// downstream WalletServices construction in step 11.
#[allow(dead_code)]
fn _ensure_collection_in_scope() -> Option<ProviderCollection<dyn IndexerProvider>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_tsc_proof_null_returns_none() {
        assert!(parse_tsc_proof(Value::Null).is_none());
    }

    #[test]
    fn parse_tsc_proof_object_returns_object() {
        let v = json!({"height": 800000, "index": 0, "nodes": []});
        assert_eq!(parse_tsc_proof(v.clone()), Some(v));
    }

    #[test]
    fn parse_tsc_proof_array_unwraps_first_element() {
        let inner = json!({"height": 800000, "index": 0, "nodes": []});
        let outer = json!([inner.clone()]);
        assert_eq!(parse_tsc_proof(outer), Some(inner));
    }

    #[test]
    fn parse_tsc_proof_empty_array_returns_none() {
        let outer = json!([]);
        assert!(parse_tsc_proof(outer).is_none());
    }

    #[test]
    fn parse_tx_status_mined_with_confirmations() {
        let j = json!({"confirmations": 6, "blockheight": 800123, "blockhash": "abc"});
        let s = parse_tx_status("txid1", &j);
        assert_eq!(s.state, TxState::Mined);
        assert_eq!(s.block_height, Some(800123));
        assert_eq!(s.block_hash.as_deref(), Some("abc"));
    }

    #[test]
    fn parse_tx_status_mempool_when_zero_confirmations_and_no_blockheight() {
        let j = json!({"confirmations": 0});
        let s = parse_tx_status("txid1", &j);
        assert_eq!(s.state, TxState::InMempool);
        assert!(s.block_height.is_none());
    }

    #[test]
    fn parse_outspend_spent() {
        let j = json!({"spent": true, "txid": "ffff", "vin": 2});
        match parse_outspend(j) {
            OutspendStatus::Spent {
                spending_txid,
                spending_vin,
            } => {
                assert_eq!(spending_txid, "ffff");
                assert_eq!(spending_vin, Some(2));
            }
            _ => panic!("expected Spent"),
        }
    }

    #[test]
    fn parse_outspend_unspent() {
        let j = json!({"spent": false});
        assert!(matches!(parse_outspend(j), OutspendStatus::Unspent));
    }

    #[test]
    fn parse_utxos_wrapped_with_result() {
        // Satoshi's genesis address — pick something with a valid P2PKH form.
        let address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let j = json!({
            "result": [
                {"tx_hash": "aa", "tx_pos": 0, "value": 5000, "height": 800000},
                {"tx_hash": "bb", "tx_pos": 1, "value": 1000, "height": 0}
            ],
            "error": ""
        });
        let utxos = parse_woc_utxos(&j, address).expect("should parse");
        assert_eq!(utxos.len(), 2);
        assert_eq!(utxos[0].txid, "aa");
        assert!(utxos[0].confirmed);
        assert_eq!(utxos[1].txid, "bb");
        assert!(!utxos[1].confirmed, "height=0 should be unconfirmed");
    }

    #[test]
    fn parse_utxos_flat_array_legacy_format() {
        let address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let j = json!([{"tx_hash": "cc", "tx_pos": 0, "value": 9999, "height": 800001}]);
        let utxos = parse_woc_utxos(&j, address).expect("should parse");
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].satoshis, 9999);
    }

    #[test]
    fn parse_utxos_returns_err_on_wrapped_error() {
        let address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let j = json!({"result": [], "error": "rate limited"});
        assert!(parse_woc_utxos(&j, address).is_err());
    }
}
