//! GorillaPool MAPI provider — legacy broadcast fallback (raw-tx hex, JSON envelope).
//!
//! Position: 3rd-tier broadcast per DESIGN §3 (after ARC GP, after TAAL ARC, before WoC).
//! MAPI accepts raw tx hex (not BEEF), so this impl extracts the main tx from the
//! BEEF wrapper before sending.
//!
//! Lifted from `handlers.rs:8531`'s `broadcast_to_gorillapool`.

use async_trait::async_trait;
use serde_json::Value;

use crate::services::provider::{BroadcastResult, IndexerError, IndexerProvider, ProviderOp};

const NAME: &str = "gorillapool_mapi";
const URL: &str = "https://mapi.gorillapool.io/mapi/tx";

pub struct GorillaPoolMapiProvider {
    client: reqwest::Client,
}

impl GorillaPoolMapiProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl IndexerProvider for GorillaPoolMapiProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn supports(&self, op: ProviderOp) -> bool {
        matches!(op, ProviderOp::BroadcastBeef)
    }

    async fn broadcast_beef(&self, beef: &[u8]) -> Result<BroadcastResult, IndexerError> {
        // MAPI takes raw tx hex; extract main tx from the BEEF wrapper.
        let parsed =
            crate::beef::Beef::from_bytes(beef).map_err(|e| IndexerError::InvalidResponse {
                provider: NAME,
                reason: format!("BEEF parse error: {}", e),
            })?;
        let main_tx = parsed.main_transaction().ok_or(IndexerError::InvalidResponse {
            provider: NAME,
            reason: "BEEF has no main transaction".to_string(),
        })?;
        let raw_tx_hex = hex::encode(main_tx);

        let body = serde_json::json!({ "rawtx": raw_tx_hex });
        let resp = self
            .client
            .post(URL)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        let http_status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        // MAPI returns HTTP 200 even on broadcast failure; parse the payload.
        if !http_status.is_success() {
            return Err(IndexerError::ProviderStatus {
                provider: NAME,
                status: http_status.as_u16(),
                body: truncate(&text, 500).to_string(),
            });
        }

        let outer: Value = serde_json::from_str(&text).map_err(|e| IndexerError::InvalidResponse {
            provider: NAME,
            reason: format!("outer JSON parse: {}", e),
        })?;
        let payload_str = outer
            .get("payload")
            .and_then(|v| v.as_str())
            .ok_or(IndexerError::InvalidResponse {
                provider: NAME,
                reason: "missing 'payload' field".to_string(),
            })?;
        let payload: Value = serde_json::from_str(payload_str).map_err(|e| {
            IndexerError::InvalidResponse {
                provider: NAME,
                reason: format!("payload JSON parse: {}", e),
            }
        })?;

        interpret_mapi_payload(&payload)
    }
}

// --- Pure helper (unit-tested below) ---

pub(crate) fn interpret_mapi_payload(payload: &Value) -> Result<BroadcastResult, IndexerError> {
    let return_result = payload
        .get("returnResult")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let txid = payload
        .get("txid")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let desc = payload
        .get("resultDescription")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if return_result == "success" {
        return Ok(BroadcastResult {
            provider: NAME,
            txid,
            tx_status: "SEEN_ON_NETWORK".to_string(),
            merkle_path_bump: None,
            block_height: None,
        });
    }

    // "already in mempool" rejections are also success per handlers.rs:8575.
    let lower = desc.to_lowercase();
    if lower.contains("already in")
        || lower.contains("already known")
        || lower.contains("duplicate")
        || lower.contains("txn-already-in-mempool")
        || lower.contains("txn-already-known")
    {
        return Ok(BroadcastResult {
            provider: NAME,
            txid,
            tx_status: "ALREADY_KNOWN".to_string(),
            merkle_path_bump: None,
            block_height: None,
        });
    }

    Err(IndexerError::ProviderStatus {
        provider: NAME,
        status: 200, // MAPI uses HTTP 200 + payload signal for failures
        body: format!("returnResult={}, desc={}", return_result, desc),
    })
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn payload_success_returns_ok_with_txid() {
        let payload = json!({
            "returnResult": "success",
            "txid": "deadbeef"
        });
        let r = interpret_mapi_payload(&payload).expect("should succeed");
        assert_eq!(r.txid, "deadbeef");
        assert_eq!(r.tx_status, "SEEN_ON_NETWORK");
        assert_eq!(r.provider, NAME);
    }

    #[test]
    fn payload_already_in_mempool_is_success() {
        let payload = json!({
            "returnResult": "failure",
            "resultDescription": "Transaction already in the mempool",
            "txid": "deadbeef"
        });
        let r = interpret_mapi_payload(&payload).expect("should be success");
        assert_eq!(r.tx_status, "ALREADY_KNOWN");
        assert_eq!(r.txid, "deadbeef");
    }

    #[test]
    fn payload_already_known_is_success() {
        let payload = json!({
            "returnResult": "failure",
            "resultDescription": "txn-already-known"
        });
        let r = interpret_mapi_payload(&payload).expect("should be success");
        assert_eq!(r.tx_status, "ALREADY_KNOWN");
    }

    #[test]
    fn payload_real_failure_is_err() {
        let payload = json!({
            "returnResult": "failure",
            "resultDescription": "Missing inputs"
        });
        match interpret_mapi_payload(&payload) {
            Err(IndexerError::ProviderStatus { body, .. }) => {
                assert!(body.contains("Missing inputs"));
            }
            other => panic!("expected ProviderStatus, got {:?}", other),
        }
    }

    #[test]
    fn supports_only_broadcast() {
        let p = GorillaPoolMapiProvider::new(reqwest::Client::new());
        assert!(p.supports(ProviderOp::BroadcastBeef));
        for op in [
            ProviderOp::RawTx,
            ProviderOp::MerkleProof,
            ProviderOp::BlockHeader,
            ProviderOp::TxStatus,
            ProviderOp::Outspend,
            ProviderOp::FetchUtxos,
        ] {
            assert!(!p.supports(op), "MAPI must not claim {:?}", op);
        }
    }
}
