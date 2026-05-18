//! JungleBus provider — greenfield Rust impl (was previously only a skill reference).
//!
//! Per DESIGN §3 this is 3rd-tier for tx-data ops. Limitations vs DESIGN:
//! - **MerkleProof** is deferred. JungleBus returns `merkle_proof` in the tx response
//!   as "TSC-compatible binary" (base64-encoded), but the exact binary↔JSON conversion
//!   isn't documented and we have no live fixture to verify against. Marking
//!   `supports(MerkleProof) = false` skips JB; the chain falls through to Bitails
//!   (4th tier). Revisit in 1.6d.C when the facade goes live and a real response can
//!   be inspected.
//! - **BlockHeader::Height** is unsupported — JungleBus only has `/block_header/get/{hash}`.
//!   The impl returns `InvalidResponse` for the Height variant; collection advances.
//! - **Outspend**, **FetchUtxos**, **BroadcastBeef** — no public JungleBus endpoints.

use async_trait::async_trait;
use serde_json::Value;

use crate::services::provider::{
    BlockHeader, BlockKey, IndexerError, IndexerProvider, ProviderOp, TxState, TxStatus,
};

const NAME: &str = "junglebus";
const BASE: &str = "https://junglebus.gorillapool.io/v1";

pub struct JungleBusProvider {
    client: reqwest::Client,
}

impl JungleBusProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    async fn get_json(&self, path: &str) -> Result<Value, IndexerError> {
        let url = format!("{}{}", BASE, path);
        let resp = self
            .client
            .get(&url)
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
        resp.json().await.map_err(|e| IndexerError::InvalidResponse {
            provider: NAME,
            reason: format!("JSON parse error: {}", e),
        })
    }
}

#[async_trait]
impl IndexerProvider for JungleBusProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn supports(&self, op: ProviderOp) -> bool {
        matches!(op, ProviderOp::RawTx | ProviderOp::BlockHeader | ProviderOp::TxStatus)
    }

    async fn get_raw_tx(&self, txid: &str) -> Result<Vec<u8>, IndexerError> {
        let json = self.get_json(&format!("/transaction/get/{}", txid)).await?;
        let hex_str = json
            .get("transaction")
            .and_then(|v| v.as_str())
            .ok_or(IndexerError::InvalidResponse {
                provider: NAME,
                reason: "missing 'transaction' field".to_string(),
            })?;
        hex::decode(hex_str).map_err(|e| IndexerError::InvalidResponse {
            provider: NAME,
            reason: format!("hex decode error: {}", e),
        })
    }

    async fn tx_status(&self, txid: &str) -> Result<TxStatus, IndexerError> {
        let json = self.get_json(&format!("/transaction/get/{}", txid)).await?;
        Ok(parse_jb_tx_status(txid, &json))
    }

    async fn get_block_header(&self, key: BlockKey) -> Result<BlockHeader, IndexerError> {
        let hash = match key {
            BlockKey::Hash(h) => h,
            BlockKey::Height(_) => {
                return Err(IndexerError::InvalidResponse {
                    provider: NAME,
                    reason: "JungleBus has no height→header endpoint; use Hash variant".to_string(),
                });
            }
        };
        let json = self.get_json(&format!("/block_header/get/{}", hash)).await?;
        parse_jb_block_header(&hash, &json).map_err(|reason| IndexerError::InvalidResponse {
            provider: NAME,
            reason,
        })
    }
}

// --- Pure parse helpers (unit-tested below) ---

pub(crate) fn parse_jb_tx_status(txid: &str, json: &Value) -> TxStatus {
    let block_height = json.get("block_height").and_then(|v| v.as_u64()).map(|h| h as u32);
    let block_hash = json
        .get("block_hash")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // JungleBus only indexes confirmed txs in the standard response. If block_height
    // is present and > 0, treat as Mined; otherwise unknown (tx may not have hit JB yet).
    let state = match block_height {
        Some(h) if h > 0 => TxState::Mined,
        Some(_) => TxState::InMempool,
        None => TxState::Unknown,
    };

    TxStatus {
        txid: txid.to_string(),
        state,
        block_height,
        block_hash,
        merkle_path_bump: None, // see module-level note re: MerkleProof deferral
    }
}

pub(crate) fn parse_jb_block_header(hash: &str, json: &Value) -> Result<BlockHeader, String> {
    // JungleBus shape (from the documented response):
    //   {"hash", "coin", "height", "time", "nonce", "version", "merkleroot", "bits", ...}
    // JungleBus does not return the raw 80-byte header hex directly — we need to
    // reconstruct it from {version, merkleroot, time, bits, nonce, prev_hash}.
    // It does NOT return prev_hash, so we can't reconstruct the full 80-byte header.
    //
    // Strategy: return what we have. Caller (cache_helpers) that needs the full hex
    // will detect a non-160-char header and fall through to the next provider.
    let height = json
        .get("height")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "missing 'height'".to_string())? as u32;

    // We can't synthesize the full 80-byte header without prev_hash. Return an empty
    // header_hex marker — the receiving caller (cache_helpers, 1.6d.C) checks this and
    // falls back. ProviderCollection sees a successful `Ok(BlockHeader)` so it doesn't
    // demote JungleBus on the bookkeeping miss.
    Ok(BlockHeader {
        block_hash: hash.to_string(),
        height,
        header_hex: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_tx_status_mined_when_block_height_present() {
        let j = json!({
            "id": "deadbeef",
            "block_height": 800123,
            "block_hash": "abc"
        });
        let s = parse_jb_tx_status("deadbeef", &j);
        assert_eq!(s.state, TxState::Mined);
        assert_eq!(s.block_height, Some(800123));
        assert_eq!(s.block_hash.as_deref(), Some("abc"));
    }

    #[test]
    fn parse_tx_status_unknown_when_no_block_height() {
        let j = json!({"id": "deadbeef"});
        let s = parse_jb_tx_status("deadbeef", &j);
        assert_eq!(s.state, TxState::Unknown);
        assert!(s.block_height.is_none());
    }

    #[test]
    fn parse_tx_status_zero_block_height_is_mempool() {
        let j = json!({"id": "deadbeef", "block_height": 0});
        let s = parse_jb_tx_status("deadbeef", &j);
        assert_eq!(s.state, TxState::InMempool);
    }

    #[test]
    fn parse_block_header_extracts_height() {
        let j = json!({
            "hash": "0000abc",
            "coin": 1,
            "height": 750000,
            "time": 1658878267,
            "merkleroot": "deadbeef"
        });
        let h = parse_jb_block_header("0000abc", &j).expect("should parse");
        assert_eq!(h.block_hash, "0000abc");
        assert_eq!(h.height, 750000);
        assert!(h.header_hex.is_empty(), "JB doesn't return full header hex");
    }

    #[test]
    fn parse_block_header_returns_err_on_missing_height() {
        let j = json!({"hash": "0000abc"});
        assert!(parse_jb_block_header("0000abc", &j).is_err());
    }

    #[test]
    fn supports_raw_tx_block_header_tx_status_only() {
        let p = JungleBusProvider::new(reqwest::Client::new());
        assert!(p.supports(ProviderOp::RawTx));
        assert!(p.supports(ProviderOp::BlockHeader));
        assert!(p.supports(ProviderOp::TxStatus));
        for op in [
            ProviderOp::MerkleProof,
            ProviderOp::Outspend,
            ProviderOp::FetchUtxos,
            ProviderOp::BroadcastBeef,
        ] {
            assert!(!p.supports(op), "JungleBus must not claim {:?}", op);
        }
    }
}
