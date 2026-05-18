//! Bitails provider — 4th-tier fallback for tx-data ops per DESIGN §3.
//!
//! Independent BSV indexer (Meysam Rezaei et al.), keyless free tier at 10 TPS /
//! 1000 daily requests. Already used as 3rd-of-3 oracle quorum at
//! `task_check_for_proofs.rs:853`. Phase 1.6 adds it here as a true fallback so
//! `snapshot_stats()` can surface real-world reliability data; if 1.6e telemetry
//! shows it underperforms, the entry drops in a future phase (DESIGN §7 #8).
//!
//! Conservative scope for 1.6d.B (dormant facade):
//! - `tx_status` — proven pattern from `query_bitails_txid` at `task_check_for_proofs.rs:853`
//! - `get_raw_tx` — `/tx/{txid}/hex` per public docs (no live verification yet — chain
//!   will advance if endpoint shape differs)
//!
//! Skipped (revisit when facade goes live in 1.6d.C, or in 1.6e if telemetry supports):
//! - `get_merkle_proof_tsc` — Bitails returns merkle info but the TSC compatibility
//!   isn't verified
//! - `get_block_header` — endpoint shape uncertain
//! - `outspend`, `fetch_utxos`, `broadcast_beef` — not in Bitails scope

use async_trait::async_trait;
use serde_json::Value;

use crate::services::provider::{
    IndexerError, IndexerProvider, ProviderOp, TxState, TxStatus,
};

const NAME: &str = "bitails";
const BASE: &str = "https://api.bitails.io";

pub struct BitailsProvider {
    client: reqwest::Client,
}

impl BitailsProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl IndexerProvider for BitailsProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn supports(&self, op: ProviderOp) -> bool {
        matches!(op, ProviderOp::TxStatus | ProviderOp::RawTx)
    }

    async fn tx_status(&self, txid: &str) -> Result<TxStatus, IndexerError> {
        let url = format!("{}/tx/{}", BASE, txid);
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
        let json: Value = resp
            .json()
            .await
            .map_err(|e| IndexerError::InvalidResponse {
                provider: NAME,
                reason: format!("JSON parse error: {}", e),
            })?;
        Ok(parse_bitails_tx_status(txid, &json))
    }

    async fn get_raw_tx(&self, txid: &str) -> Result<Vec<u8>, IndexerError> {
        let url = format!("{}/tx/{}/hex", BASE, txid);
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
        let hex_str = resp
            .text()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        let hex_str = hex_str.trim();
        hex::decode(hex_str).map_err(|e| IndexerError::InvalidResponse {
            provider: NAME,
            reason: format!("hex decode error: {}", e),
        })
    }
}

// --- Pure parse helper (unit-tested below) ---

pub(crate) fn parse_bitails_tx_status(txid: &str, json: &Value) -> TxStatus {
    // Bitails has been observed to put height under either `blockHeight` or
    // `block.height` (per `task_check_for_proofs.rs:865`). Honor both.
    let block_height = json
        .get("blockHeight")
        .and_then(|v| v.as_u64())
        .or_else(|| json.get("block").and_then(|b| b.get("height")).and_then(|v| v.as_u64()))
        .map(|h| h as u32);
    let block_hash = json
        .get("blockHash")
        .and_then(|v| v.as_str())
        .or_else(|| json.get("block").and_then(|b| b.get("hash")).and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    let state = match block_height {
        Some(h) if h > 0 => TxState::Mined,
        Some(_) | None => TxState::InMempool,
    };

    TxStatus {
        txid: txid.to_string(),
        state,
        block_height,
        block_hash,
        merkle_path_bump: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_tx_status_mined_with_top_level_block_height() {
        let j = json!({"blockHeight": 800123, "blockHash": "abc"});
        let s = parse_bitails_tx_status("deadbeef", &j);
        assert_eq!(s.state, TxState::Mined);
        assert_eq!(s.block_height, Some(800123));
        assert_eq!(s.block_hash.as_deref(), Some("abc"));
    }

    #[test]
    fn parse_tx_status_mined_with_nested_block_height() {
        let j = json!({"block": {"height": 800123, "hash": "abc"}});
        let s = parse_bitails_tx_status("deadbeef", &j);
        assert_eq!(s.state, TxState::Mined);
        assert_eq!(s.block_height, Some(800123));
        assert_eq!(s.block_hash.as_deref(), Some("abc"));
    }

    #[test]
    fn parse_tx_status_mempool_when_no_block_height() {
        let j = json!({"txid": "deadbeef"});
        let s = parse_bitails_tx_status("deadbeef", &j);
        assert_eq!(s.state, TxState::InMempool);
        assert!(s.block_height.is_none());
    }

    #[test]
    fn parse_tx_status_zero_height_is_mempool() {
        let j = json!({"blockHeight": 0});
        let s = parse_bitails_tx_status("deadbeef", &j);
        assert_eq!(s.state, TxState::InMempool);
    }

    #[test]
    fn supports_tx_status_and_raw_tx_only() {
        let p = BitailsProvider::new(reqwest::Client::new());
        assert!(p.supports(ProviderOp::TxStatus));
        assert!(p.supports(ProviderOp::RawTx));
        for op in [
            ProviderOp::MerkleProof,
            ProviderOp::BlockHeader,
            ProviderOp::Outspend,
            ProviderOp::FetchUtxos,
            ProviderOp::BroadcastBeef,
        ] {
            assert!(!p.supports(op), "Bitails must not claim {:?}", op);
        }
    }
}
