//! TAAL ARC provider — fallback broadcast endpoint with hardcoded API key.
//!
//! Per memory `project-taal-arc-key-hardcoded`: the API key is rotated monthly between
//! builds and must NOT be env-var-ified. Mirror the literal at `handlers.rs:8791`.
//!
//! Per memory `project-taal-arc-unreliable-for-primary`: TAAL stays fallback, never
//! primary. The `WalletServices` broadcast chain orders this AFTER ArcGorillaPool.
//! Promoting TAAL would create a regression window every build cycle.

use async_trait::async_trait;

use crate::services::provider::{BroadcastResult, IndexerError, IndexerProvider, ProviderOp};

const NAME: &str = "arc_taal";
const URL: &str = "https://arc.taal.com/v1/tx";
const API_KEY: &str = "mainnet_fa871d12caa95b39076ac0b6b532a410";

pub struct ArcTaalProvider {
    client: reqwest::Client,
}

impl ArcTaalProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl IndexerProvider for ArcTaalProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn supports(&self, op: ProviderOp) -> bool {
        // TAAL fallback role is broadcast only. Status/proof lookups go to ARC GP.
        matches!(op, ProviderOp::BroadcastBeef)
    }

    async fn broadcast_beef(&self, beef: &[u8]) -> Result<BroadcastResult, IndexerError> {
        let beef_hex = hex::encode(beef);
        let body = serde_json::json!({ "rawTx": beef_hex });
        let resp = self
            .client
            .post(URL)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", API_KEY))
            .json(&body)
            .send()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        let http_status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let arc: super::arc_gorillapool::ArcResponse = serde_json::from_str(&text).map_err(|e| {
            IndexerError::InvalidResponse {
                provider: NAME,
                reason: format!("TAAL ARC parse error: {}", e),
            }
        })?;

        // TAAL has one extra failure mode vs GorillaPool ARC: HTTP 401 means the
        // hardcoded API key is expired or invalid. Surface that distinctly so callers
        // can tell "fallback is broken" from "tx was rejected".
        if http_status.as_u16() == 401 {
            return Err(IndexerError::ProviderStatus {
                provider: NAME,
                status: 401,
                body: format!(
                    "TAAL ARC authentication failed — hardcoded API key expired or invalid: {}",
                    truncate(&text, 200)
                ),
            });
        }

        // Otherwise reuse the GP interpretation — same ARC response shape, same 200/409
        // semantics. We rename the `provider` field on success.
        super::arc_gorillapool::interpret_broadcast_response(http_status.as_u16(), &arc, &text)
            .map(|mut r| {
                r.provider = NAME;
                r
            })
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

    #[test]
    fn supports_only_broadcast_beef() {
        let p = ArcTaalProvider::new(reqwest::Client::new());
        assert!(p.supports(ProviderOp::BroadcastBeef));
        for op in [
            ProviderOp::RawTx,
            ProviderOp::MerkleProof,
            ProviderOp::BlockHeader,
            ProviderOp::TxStatus,
            ProviderOp::Outspend,
            ProviderOp::FetchUtxos,
        ] {
            assert!(!p.supports(op), "TAAL must not claim {:?}", op);
        }
    }

    #[test]
    fn name_matches_constant() {
        let p = ArcTaalProvider::new(reqwest::Client::new());
        assert_eq!(p.name(), "arc_taal");
    }

    #[test]
    fn api_key_constant_is_present() {
        // Smoke check the hardcoded key matches the handlers.rs:8791 literal.
        // Memory `project-taal-arc-key-hardcoded` requires this be a string literal —
        // assertions are an audit trail for the next monthly rotation.
        assert_eq!(API_KEY, "mainnet_fa871d12caa95b39076ac0b6b532a410");
        assert_eq!(URL, "https://arc.taal.com/v1/tx");
    }
}
