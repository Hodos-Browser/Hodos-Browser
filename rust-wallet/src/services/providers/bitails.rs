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
//! - `broadcast_beef` (B5-T1-P7b, owner 2026-10-10) — `POST /tx/broadcast/multi {"raws":[hex]}`, the
//!   request `@bsv/wallet-toolbox` `Bitails.ts` sends: the subject's raw transaction only (Bitails
//!   takes no BEEF), no key. A second operator after GorillaPool in the broadcast chain.
//!
//! Skipped (revisit when facade goes live in 1.6d.C, or in 1.6e if telemetry supports):
//! - `get_merkle_proof_tsc` — Bitails returns merkle info but the TSC compatibility
//!   isn't verified
//! - `get_block_header` — endpoint shape uncertain
//! - `outspend`, `fetch_utxos` — not in Bitails scope

use async_trait::async_trait;
use serde_json::Value;

use crate::services::provider::{
    BroadcastResult, IndexerError, IndexerProvider, ProviderOp, TxState, TxStatus,
};

const NAME: &str = "bitails";
const BASE: &str = "https://api.bitails.io";

pub struct BitailsProvider {
    client: reqwest::Client,
    base: String,
}

impl BitailsProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client, base: BASE.to_string() }
    }

    /// Test-only: the same provider against a local stand-in server.
    #[cfg(test)]
    pub fn with_base(client: reqwest::Client, base: String) -> Self {
        Self { client, base }
    }
}

#[async_trait]
impl IndexerProvider for BitailsProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn supports(&self, op: ProviderOp) -> bool {
        matches!(op, ProviderOp::TxStatus | ProviderOp::RawTx | ProviderOp::BroadcastBeef)
    }

    async fn tx_status(&self, txid: &str) -> Result<TxStatus, IndexerError> {
        let url = format!("{}/tx/{}", self.base, txid);
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
        let url = format!("{}/tx/{}/hex", self.base, txid);
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

    async fn broadcast_beef(&self, beef: &[u8]) -> Result<BroadcastResult, IndexerError> {
        let (body, expected_txid) = bitails_broadcast_body(beef)?;
        let resp = self
            .client
            .post(format!("{}/tx/broadcast/multi", self.base))
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        interpret_bitails_broadcast(status, &text, &expected_txid)
    }
}

/// The request body for one broadcast — `{"raws":[<subject raw tx hex>]}` — and the subject's
/// display txid. The subject is the BEEF's last transaction (the one ARC answers for too).
pub(crate) fn bitails_broadcast_body(beef: &[u8]) -> Result<(Value, String), IndexerError> {
    let parsed = crate::beef::Beef::from_bytes(beef).map_err(|e| IndexerError::InvalidResponse {
        provider: NAME,
        reason: format!("BEEF parse error: {}", e),
    })?;
    let raw = parsed.main_transaction().ok_or(IndexerError::InvalidResponse {
        provider: NAME,
        reason: "BEEF has no main transaction".to_string(),
    })?;
    use sha2::{Digest, Sha256};
    let txid: Vec<u8> = Sha256::digest(Sha256::digest(raw)).iter().rev().copied().collect();
    Ok((serde_json::json!({ "raws": [hex::encode(raw)] }), hex::encode(txid)))
}

/// Bitails answers `/tx/broadcast/multi` with one result per raw tx — HTTP 201 even for a refusal
/// (measured 2026-10-10). Read as `@bsv/wallet-toolbox` `Bitails.ts` does: no `error` ⇒ accepted;
/// `-27` ⇒ already in the mempool (accepted); `-25` ⇒ missing inputs, worded as WhatsOnChain's
/// identical answer so `provider_verdict` classifies both alike; any other code ⇒ that code and
/// message. A non-2xx status, a result count other than one, or a txid that is not ours ⇒ error.
pub(crate) fn interpret_bitails_broadcast(
    http_status: u16,
    body: &str,
    expected_txid: &str,
) -> Result<BroadcastResult, IndexerError> {
    if !(200..300).contains(&http_status) {
        return Err(IndexerError::ProviderStatus { provider: NAME, status: http_status, body: body.chars().take(500).collect() });
    }
    let invalid = |reason: String| IndexerError::InvalidResponse { provider: NAME, reason };
    let json: Value = serde_json::from_str(body).map_err(|e| invalid(format!("JSON parse error: {}", e)))?;
    let entries = json.as_array().ok_or_else(|| invalid("broadcast answer is not an array".into()))?;
    if entries.len() != 1 {
        return Err(invalid(format!("{} broadcast results for 1 transaction", entries.len())));
    }
    let entry = &entries[0];
    // #5 (review 162): a result that is not an object is not an answer — never an acceptance.
    if !entry.is_object() {
        return Err(invalid(format!("broadcast result is not an object: {}", entry)));
    }
    if let Some(txid) = entry.get("txid").and_then(|t| t.as_str()) {
        if !txid.eq_ignore_ascii_case(expected_txid) {
            return Err(invalid(format!("result names txid {} — we sent {}", txid, expected_txid)));
        }
    }
    let accepted = |tx_status: &str| Ok(BroadcastResult {
        provider: NAME,
        txid: expected_txid.to_string(),
        tx_status: tx_status.to_string(),
        merkle_path_bump: None,
        block_height: None,
    });
    let Some(err) = entry.get("error").filter(|e| !e.is_null()) else {
        return accepted("SEEN_ON_NETWORK");
    };
    // Review 164 #9: an error with no code (or a bare string) is read by its text, as a refusal.
    let code_value = err.get("code").filter(|c| !c.is_null());
    if code_value.is_none() {
        let text = err.as_str().map(|s| s.to_string())
            .or_else(|| err.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| err.to_string());
        let lower = text.to_lowercase();
        let text = if lower.contains("missing-inputs") { format!("Missing inputs ({})", text) } else { text };
        // Review 164-5 #9: a service failure in words (no code) may still hold the transaction.
        let service = ["econnreset", "unavailable", "timeout", "timed out", "rate limit", "too many requests", "internal"]
            .iter().any(|w| lower.contains(w));
        return Err(IndexerError::ProviderStatus { provider: NAME, status: if service { 503 } else { 400 }, body: format!("bitails error: {}", text) });
    }
    // #4 (review 162): the code may come as a number or a string (go-wallet-toolbox reads both).
    let code = err.get("code").and_then(|c| c.as_i64().or_else(|| c.as_str().and_then(|s| s.trim().parse().ok())));
    let message = err.get("message").and_then(|m| m.as_str()).unwrap_or("");
    match code {
        Some(-27) => accepted("ALREADY_KNOWN"),
        // #2 (review 162): a refusal is reported as 400, not the 201 Bitails sent — a 2xx status reads
        // as "the provider may hold the transaction" and would keep a refused chain from ever settling.
        Some(-25) => Err(IndexerError::ProviderStatus {
            provider: NAME,
            status: 400,
            body: format!("Missing inputs (bitails -25 {})", message),
        }),
        // Review 164 #9: a code that is not a number (`ECONNRESET`, …) is Bitails' own trouble, not a
        // verdict on the transaction — the provider may hold it.
        None => Err(IndexerError::ProviderStatus {
            provider: NAME,
            status: 503,
            body: format!("bitails service error: {}", err),
        }),
        _ => Err(IndexerError::ProviderStatus {
            provider: NAME,
            status: 400,
            body: format!("bitails error {}: {}", code.map(|c| c.to_string()).unwrap_or_else(|| "?".into()), message),
        }),
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
        raw_provider_status: None, // Bitails doesn't expose a richer status string.
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
    fn supports_tx_status_raw_tx_and_broadcast_only() {
        let p = BitailsProvider::new(reqwest::Client::new());
        assert!(p.supports(ProviderOp::TxStatus));
        assert!(p.supports(ProviderOp::RawTx));
        assert!(p.supports(ProviderOp::BroadcastBeef), "B5-T1-P7b: Bitails broadcasts");
        for op in [
            ProviderOp::MerkleProof,
            ProviderOp::BlockHeader,
            ProviderOp::Outspend,
            ProviderOp::FetchUtxos,
        ] {
            assert!(!p.supports(op), "Bitails must not claim {:?}", op);
        }
    }
    /// `P7b-B10` — `/tx/broadcast/multi` answers, read as `@bsv/wallet-toolbox` `Bitails.ts` does.
    /// R1 is the body recorded live on 2026-10-10 (a mined tx: its inputs are spent).
    #[test]
    fn p7b_b10_broadcast_answers() {
        let ours = "ab".repeat(32);
        let ok = |s: u16, body: &str| interpret_bitails_broadcast(s, body, &ours);
        // accepted
        let r = ok(201, &format!(r#"[{{"txid":"{}"}}]"#, ours)).unwrap();
        assert_eq!((r.provider, r.txid.as_str(), r.tx_status.as_str()), ("bitails", ours.as_str(), "SEEN_ON_NETWORK"));
        assert_eq!(ok(201, "[{}]").unwrap().txid, ours, "no txid named: ours (computed) is bound");
        // -27: already in the mempool = accepted
        assert_eq!(ok(201, r#"[{"error":{"code":-27,"message":"txn-already-known"}}]"#).unwrap().tx_status, "ALREADY_KNOWN");
        // R1 (recorded): -25 = missing inputs, worded as WhatsOnChain's answer
        let e = ok(201, r#"[{"error":{"code":-25,"message":"missing-inputs"}}]"#).unwrap_err().to_string();
        assert!(e.contains("Missing inputs") && crate::arc_status::is_fatal_broadcast_error(&e), "{}", e);
        // another code: its code and message, not accepted
        let e = ok(201, r#"[{"error":{"code":-26,"message":"258: txn-mempool-conflict"}}]"#).unwrap_err().to_string();
        assert!(e.contains("-26") && e.contains("txn-mempool-conflict"), "{}", e);
        // #4: a string code is read as its number
        assert_eq!(ok(201, r#"[{"error":{"code":"-27","message":"txn-already-known"}}]"#).unwrap().tx_status, "ALREADY_KNOWN");
        let e = ok(201, r#"[{"error":{"code":"-25","message":"missing-inputs"}}]"#).unwrap_err().to_string();
        assert!(crate::arc_status::is_fatal_broadcast_error(&e), "{}", e);
        // #9 (review 164): a code that is not a number is Bitails' own trouble — 503, "may hold"
        assert!(matches!(ok(201, r#"[{"error":{"code":"ECONNRESET"}}]"#), Err(IndexerError::ProviderStatus { status: 503, .. })));
        // review 164 #9: no code, or a bare string error, reads by its text — a refusal, not "may hold"
        let e = ok(201, r#"[{"error":{"message":"missing-inputs"}}]"#).unwrap_err().to_string();
        assert!(e.contains("400") && crate::arc_status::is_fatal_broadcast_error(&e), "{}", e);
        assert!(matches!(ok(201, r#"[{"error":"Missing inputs"}]"#), Err(IndexerError::ProviderStatus { status: 400, .. })));
        // review 164-5 #9: a service failure in words is "may hold"
        assert!(matches!(ok(201, r#"[{"error":"ECONNRESET"}]"#), Err(IndexerError::ProviderStatus { status: 503, .. })));
        assert!(matches!(ok(201, r#"[{"error":{"message":"Service Unavailable"}}]"#), Err(IndexerError::ProviderStatus { status: 503, .. })));
        // #2: a refusal is a 400, so an unrecognised one reads as "not accepted", never "may hold"
        assert!(matches!(ok(201, r#"[{"error":{"code":-26,"message":"66: mempool min fee not met"}}]"#),
            Err(IndexerError::ProviderStatus { status: 400, .. })));
        // #5: an entry that is not an object is never an acceptance
        for body in ["[null]", r#"["queued"]"#, "[7]"] {
            assert!(matches!(ok(201, body), Err(IndexerError::InvalidResponse { .. })), "{}", body);
        }
        // not 2xx; not one result; not our txid; not JSON
        assert!(matches!(ok(500, "boom"), Err(IndexerError::ProviderStatus { status: 500, .. })));
        assert!(matches!(ok(201, "[]"), Err(IndexerError::InvalidResponse { .. })));
        assert!(matches!(ok(201, "[{},{}]"), Err(IndexerError::InvalidResponse { .. })));
        assert!(matches!(ok(201, &format!(r#"[{{"txid":"{}"}}]"#, "cd".repeat(32))), Err(IndexerError::InvalidResponse { .. })));
        assert!(matches!(ok(201, "not json"), Err(IndexerError::InvalidResponse { .. })));
    }

    /// `P7b-B9` (pure half) — the request body is the subject's raw tx only, and the txid is its own.
    #[test]
    fn p7b_b9_request_body_is_the_subject_raw_tx() {
        // version 1, one input (prev, vout 0, empty script, final), one output (1,000 sats, `script`), lock 0
        let tx = |prev: u8, script: u8| -> Vec<u8> {
            let mut t = vec![1, 0, 0, 0, 1];
            t.extend([prev; 32]);
            t.extend([0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 1]);
            t.extend(1_000u64.to_le_bytes());
            t.extend([1, script, 0, 0, 0, 0]);
            t
        };
        let (parent, subject) = (tx(0xaa, 0x51), tx(0xbb, 0x52));
        let beef = crate::beef::Beef { version: crate::beef::BEEF_V1_MARKER, bumps: vec![], transactions: vec![parent, subject.clone()], tx_to_bump: vec![None, None] };
        let (body, txid) = bitails_broadcast_body(&beef.to_v1_bytes().unwrap()).unwrap();
        assert_eq!(body, serde_json::json!({ "raws": [hex::encode(&subject)] }));
        use sha2::{Digest, Sha256};
        assert_eq!(txid, hex::encode(Sha256::digest(Sha256::digest(&subject)).iter().rev().copied().collect::<Vec<u8>>()));
    }
}

