//! ARC (GorillaPool) provider — primary broadcast endpoint + tx status + merkle proofs.
//!
//! NOTE on DESIGN §3 vs reality: DESIGN's per-op chain matrix lists ARC GP first for
//! `RawTx` (raw tx hex). ARC's public surface does not include a raw-tx-body endpoint —
//! `/v1/tx/{txid}` returns status + BUMP only. This impl therefore declares
//! `supports(RawTx) = false`; `ProviderCollection` then skips ARC GP for raw_tx and
//! falls through to the WoC second-tier, matching effective behavior.
//!
//! ARC's primary value here is broadcast_beef (BEEF accepted natively) and merkle proof
//! (BUMP, converted to TSC via `beef::parse_bump_hex_to_tsc`).

use async_trait::async_trait;
use serde_json::Value;

use crate::services::provider::{
    BroadcastResult, IndexerError, IndexerProvider, ProviderOp, TxState, TxStatus,
};

const NAME: &str = "arc_gorillapool";
const BASE: &str = "https://arc.gorillapool.io/v1";

/// ARC API response shape. Post-1.6d.D-3 this is the canonical wallet-side
/// definition of an ARC `/v1/tx` response — the duplicate in `crate::handlers`
/// was deleted when `query_arc_tx_status` migrated to `services.tx_status`.
/// Shared with `arc_taal.rs` via `pub(crate)`.
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct ArcResponse {
    #[serde(rename = "blockHash", default)]
    pub block_hash: Option<String>,
    #[serde(rename = "blockHeight", default)]
    pub block_height: Option<u64>,
    #[serde(rename = "extraInfo", default)]
    pub extra_info: Option<String>,
    #[serde(rename = "competingTxs", default)]
    pub competing_txs: Option<Vec<String>>,
    #[serde(rename = "merklePath", default)]
    pub merkle_path: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub txid: Option<String>,
    #[serde(rename = "txStatus", default)]
    pub tx_status: Option<String>,
    #[serde(default)]
    pub status: Option<u16>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(rename = "type", default)]
    pub error_type: Option<String>,
}

pub struct ArcGorillaPoolProvider {
    client: reqwest::Client,
}

impl ArcGorillaPoolProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl IndexerProvider for ArcGorillaPoolProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn supports(&self, op: ProviderOp) -> bool {
        matches!(
            op,
            ProviderOp::TxStatus | ProviderOp::MerkleProof | ProviderOp::BroadcastBeef
        )
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
        let text = resp
            .text()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        let arc: ArcResponse = serde_json::from_str(&text).map_err(|e| {
            IndexerError::InvalidResponse {
                provider: NAME,
                reason: format!("ARC parse error: {}", e),
            }
        })?;
        Ok(arc_response_to_tx_status(txid, &arc))
    }

    async fn get_merkle_proof_tsc(&self, txid: &str) -> Result<Value, IndexerError> {
        // ARC returns the merkle path as BUMP hex inside the tx-status response.
        // Convert BUMP→TSC and inject blockHeight (TSC needs it; BUMP carries it inline).
        let status = self.tx_status(txid).await?;
        let bump_hex = status.merkle_path_bump.ok_or(IndexerError::NotFound)?;
        let mut tsc = crate::beef::parse_bump_hex_to_tsc(&bump_hex).map_err(|e| {
            IndexerError::InvalidResponse {
                provider: NAME,
                reason: format!("BUMP parse error: {}", e),
            }
        })?;
        if let Some(h) = status.block_height {
            tsc["height"] = serde_json::json!(h);
        }
        Ok(tsc)
    }

    async fn broadcast_beef(&self, beef: &[u8]) -> Result<BroadcastResult, IndexerError> {
        let beef_hex = hex::encode(beef);
        let url = format!("{}/tx", BASE);
        let body = serde_json::json!({ "rawTx": beef_hex });
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        let http_status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let arc: ArcResponse = serde_json::from_str(&text).map_err(|e| {
            IndexerError::InvalidResponse {
                provider: NAME,
                reason: format!("ARC parse error: {} — body: {}", e, truncate(&text, 200)),
            }
        })?;
        interpret_broadcast_response(http_status.as_u16(), &arc, &text)
    }
}

// --- Pure helpers (unit-tested below) ---

pub(crate) fn arc_response_to_tx_status(
    txid: &str,
    arc: &ArcResponse,
) -> TxStatus {
    let status_str = arc.tx_status.as_deref().unwrap_or("");
    let state = match status_str {
        "MINED" => TxState::Mined,
        "SEEN_ON_NETWORK"
        | "ANNOUNCED_TO_NETWORK"
        | "REQUESTED_BY_NETWORK"
        | "SENT_TO_NETWORK"
        | "ACCEPTED_BY_NETWORK"
        | "STORED"
        | "QUEUED"
        | "RECEIVED" => TxState::InMempool,
        "DOUBLE_SPEND_ATTEMPTED" => TxState::DoubleSpendAttempted,
        "REJECTED" | "SEEN_IN_ORPHAN_MEMPOOL" | "MINED_IN_STALE_BLOCK" => TxState::Rejected,
        _ => TxState::Unknown,
    };
    TxStatus {
        txid: txid.to_string(),
        state,
        block_height: arc.block_height.map(|h| h as u32),
        block_hash: arc.block_hash.clone(),
        merkle_path_bump: arc.merkle_path.clone(),
        // Preserve ARC's rich status vocabulary for callers that need to
        // distinguish ANNOUNCED_TO_NETWORK vs SEEN_ON_NETWORK,
        // SEEN_IN_ORPHAN_MEMPOOL vs REJECTED, etc.
        raw_provider_status: arc.tx_status.clone(),
    }
}

pub(crate) fn interpret_broadcast_response(
    http_status: u16,
    arc: &ArcResponse,
    raw_body: &str,
) -> Result<BroadcastResult, IndexerError> {
    // Inspect txStatus first — ARC can return error statuses with any HTTP code
    // (mirrors handlers.rs:8708).
    let tx_status_str = arc.tx_status.as_deref().unwrap_or("");
    if matches!(
        tx_status_str,
        "DOUBLE_SPEND_ATTEMPTED"
            | "REJECTED"
            | "SEEN_IN_ORPHAN_MEMPOOL"
            | "MINED_IN_STALE_BLOCK"
    ) {
        return Err(IndexerError::ProviderStatus {
            provider: NAME,
            status: http_status,
            body: format!("txStatus={}: {}", tx_status_str, truncate(raw_body, 200)),
        });
    }

    // ANNOUNCED_TO_NETWORK is step 5 of 10 in ARC's lifecycle — ARC sent an INV
    // message but no peer has yet requested the tx, let alone accepted it. We
    // deliberately diverge from canonical wallet-toolbox (which treats ANNOUNCED
    // as primary-broadcast success) and advance the ProviderCollection chain to
    // the next broadcaster. Mirrors today's handlers.rs:8294-8297 fall-through.
    // See memory `reference_arc_tx_status_ladder` for the full ladder + rationale.
    if tx_status_str == "ANNOUNCED_TO_NETWORK" {
        return Err(IndexerError::ProviderStatus {
            provider: NAME,
            status: http_status,
            body: "ANNOUNCED_TO_NETWORK — weak signal, advancing chain to next broadcaster"
                .to_string(),
        });
    }

    match http_status {
        200 | 201 => Ok(BroadcastResult {
            provider: NAME,
            txid: arc.txid.clone().unwrap_or_default(),
            tx_status: if tx_status_str.is_empty() {
                "ACCEPTED".to_string()
            } else {
                tx_status_str.to_string()
            },
            merkle_path_bump: arc.merkle_path.clone(),
            block_height: arc.block_height.map(|h| h as u32),
        }),
        409 => {
            // 409 has two meanings: genuine "already known" success OR timeout/generic
            // error masquerading as 409. Mirror handlers.rs:8731 disambiguation.
            let extra_info = arc.extra_info.as_deref().unwrap_or("");
            let detail = arc.detail.as_deref().unwrap_or("");
            let title = arc.title.as_deref().unwrap_or("");
            let is_timeout = extra_info.contains("DeadlineExceeded")
                || extra_info.contains("context deadline exceeded");
            let is_generic_error = detail.contains("could not be processed")
                || title == "Generic error";
            let txid = arc.txid.as_deref().unwrap_or("");
            let has_real_txid = !txid.is_empty() && txid != "unknown" && txid != "null";

            if is_timeout || (is_generic_error && !has_real_txid) {
                Err(IndexerError::ProviderStatus {
                    provider: NAME,
                    status: 409,
                    body: format!("ARC timeout/error 409: {} — {}", detail, extra_info),
                })
            } else {
                Ok(BroadcastResult {
                    provider: NAME,
                    txid: txid.to_string(),
                    tx_status: if tx_status_str.is_empty() {
                        "ALREADY_KNOWN".to_string()
                    } else {
                        tx_status_str.to_string()
                    },
                    merkle_path_bump: arc.merkle_path.clone(),
                    block_height: arc.block_height.map(|h| h as u32),
                })
            }
        }
        s => Err(IndexerError::ProviderStatus {
            provider: NAME,
            status: s,
            body: truncate(raw_body, 500).to_string(),
        }),
    }
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
    // ArcResponse is brought in via `use super::*;` above.
    use serde_json::json;

    fn arc_from_json(j: Value) -> ArcResponse {
        serde_json::from_value(j).expect("test ArcResponse parse")
    }

    #[test]
    fn arc_response_mined_maps_to_mined_state() {
        let arc = arc_from_json(json!({
            "txid": "deadbeef",
            "txStatus": "MINED",
            "blockHeight": 800123,
            "blockHash": "abc",
            "merklePath": "fe..."
        }));
        let s = arc_response_to_tx_status("deadbeef", &arc);
        assert_eq!(s.state, TxState::Mined);
        assert_eq!(s.block_height, Some(800123));
        assert_eq!(s.block_hash.as_deref(), Some("abc"));
        assert_eq!(s.merkle_path_bump.as_deref(), Some("fe..."));
    }

    #[test]
    fn arc_response_seen_on_network_maps_to_mempool() {
        let arc = arc_from_json(json!({"txStatus": "SEEN_ON_NETWORK"}));
        assert_eq!(arc_response_to_tx_status("t", &arc).state, TxState::InMempool);
    }

    #[test]
    fn arc_response_double_spend_maps_to_double_spend_state() {
        let arc = arc_from_json(json!({"txStatus": "DOUBLE_SPEND_ATTEMPTED"}));
        assert_eq!(
            arc_response_to_tx_status("t", &arc).state,
            TxState::DoubleSpendAttempted
        );
    }

    #[test]
    fn arc_response_unknown_status_maps_to_unknown_state() {
        let arc = arc_from_json(json!({"txStatus": "SOMETHING_NEW"}));
        assert_eq!(arc_response_to_tx_status("t", &arc).state, TxState::Unknown);
    }

    #[test]
    fn broadcast_200_with_seen_on_network_is_success() {
        let arc = arc_from_json(json!({
            "txid": "deadbeef",
            "txStatus": "SEEN_ON_NETWORK"
        }));
        let r = interpret_broadcast_response(200, &arc, "").expect("should succeed");
        assert_eq!(r.txid, "deadbeef");
        assert_eq!(r.tx_status, "SEEN_ON_NETWORK");
        assert_eq!(r.provider, NAME);
    }

    #[test]
    fn broadcast_409_already_known_is_success() {
        let arc = arc_from_json(json!({
            "txid": "deadbeef",
            "txStatus": "SEEN_ON_NETWORK"
        }));
        let r = interpret_broadcast_response(409, &arc, "").expect("should be success");
        assert_eq!(r.txid, "deadbeef");
        assert_eq!(r.tx_status, "SEEN_ON_NETWORK");
    }

    #[test]
    fn broadcast_409_timeout_is_error() {
        let arc = arc_from_json(json!({
            "txid": "deadbeef",
            "extraInfo": "context deadline exceeded",
            "detail": "transaction could not be processed"
        }));
        assert!(matches!(
            interpret_broadcast_response(409, &arc, ""),
            Err(IndexerError::ProviderStatus { status: 409, .. })
        ));
    }

    #[test]
    fn broadcast_double_spend_status_is_error_regardless_of_http() {
        let arc = arc_from_json(json!({
            "txid": "deadbeef",
            "txStatus": "DOUBLE_SPEND_ATTEMPTED"
        }));
        assert!(matches!(
            interpret_broadcast_response(200, &arc, ""),
            Err(IndexerError::ProviderStatus { .. })
        ));
    }

    #[test]
    fn broadcast_announced_to_network_is_err_so_chain_advances() {
        // Hodos policy (Phase 1.6d.D): ANNOUNCED_TO_NETWORK is step 5/10 in ARC's
        // lifecycle ladder. ARC sent INV; no peer has acknowledged. We deliberately
        // diverge from canonical wallet-toolbox (which treats this as primary-broadcast
        // success) and advance the ProviderCollection chain to the next broadcaster.
        // See memory `reference_arc_tx_status_ladder` and handlers.rs:8294-8297
        // for the existing fall-through that this preserves.
        let arc = arc_from_json(json!({
            "txid": "deadbeef",
            "txStatus": "ANNOUNCED_TO_NETWORK"
        }));
        match interpret_broadcast_response(200, &arc, "") {
            Err(IndexerError::ProviderStatus { provider, body, .. }) => {
                assert_eq!(provider, NAME);
                assert!(body.contains("ANNOUNCED_TO_NETWORK"));
            }
            other => panic!("expected ProviderStatus Err for ANNOUNCED, got {:?}", other),
        }
    }

    #[test]
    fn broadcast_other_http_status_is_error() {
        let arc = arc_from_json(json!({"detail": "validation failed"}));
        match interpret_broadcast_response(460, &arc, "{}") {
            Err(IndexerError::ProviderStatus { status, .. }) => assert_eq!(status, 460),
            other => panic!("expected ProviderStatus 460, got {:?}", other),
        }
    }
}
