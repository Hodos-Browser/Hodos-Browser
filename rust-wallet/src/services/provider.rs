//! IndexerProvider trait, error type, and shared value types for the Services facade.
//!
//! Phase 1.6d.B scaffolding. The trait + impls land dormant — no existing call site
//! routes through `WalletServices` until 1.6d.C. See
//! `development-docs/Sigma-BRC121-Sprint/phase-1.6-indexer-resilience/DESIGN.md` §2.

use async_trait::async_trait;
use std::time::Duration;

/// All provider call results funnel through this error type. `NotFound` short-circuits
/// the fallback chain (it's a positive "this tx genuinely doesn't exist" signal, not a
/// failure to retry on). Other variants advance the chain; `SoftTimeout` also demotes
/// the offending provider for the rest of the process lifetime.
#[derive(Debug, thiserror::Error)]
pub enum IndexerError {
    #[error("transport error: {0}")]
    Transport(String),

    #[error("soft timeout after {0:?}")]
    SoftTimeout(Duration),

    #[error("provider {provider} returned status {status}: {body}")]
    ProviderStatus {
        provider: &'static str,
        status: u16,
        body: String,
    },

    #[error("invalid response from {provider}: {reason}")]
    InvalidResponse {
        provider: &'static str,
        reason: String,
    },

    #[error("not found")]
    NotFound,
}

/// Key for looking up a block header. Providers may support one or both (WoC supports
/// both; some providers may only support one — they should return `InvalidResponse`
/// from the unsupported variant).
#[derive(Debug, Clone)]
pub enum BlockKey {
    Hash(String),
    Height(u32),
}

/// Lightweight block header shape returned by providers. Distinct from
/// `crate::database::models::BlockHeader` (which carries `id` and `cached_at` cache
/// metadata). cache_helpers is responsible for the network→DB shape conversion.
#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub block_hash: String,
    pub height: u32,
    pub header_hex: String,
}

/// Cross-provider normalized tx status. ARC's status vocabulary is the richest;
/// other providers compute their best-effort mapping into `TxState`.
#[derive(Debug, Clone)]
pub struct TxStatus {
    pub txid: String,
    pub state: TxState,
    pub block_height: Option<u32>,
    pub block_hash: Option<String>,
    /// BUMP merkle path (hex), when the provider returns one alongside MINED status.
    pub merkle_path_bump: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxState {
    Unknown,
    InMempool,
    Mined,
    Rejected,
    DoubleSpendAttempted,
}

#[derive(Debug, Clone)]
pub enum OutspendStatus {
    Unspent,
    Spent {
        spending_txid: String,
        spending_vin: Option<u32>,
    },
}

/// Returned by `broadcast_beef` on success. `provider` records which chain link
/// actually accepted the tx (for telemetry + activity-log debugging).
#[derive(Debug, Clone)]
pub struct BroadcastResult {
    pub provider: &'static str,
    pub txid: String,
    pub tx_status: String,
    pub merkle_path_bump: Option<String>,
    pub block_height: Option<u32>,
}

/// Enumerates the operations a provider may declare support for via `supports()`.
/// Used by `ProviderCollection::call` to skip providers that opt out of an op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderOp {
    RawTx,
    MerkleProof,
    BlockHeader,
    TxStatus,
    Outspend,
    FetchUtxos,
    BroadcastBeef,
}

/// Provider trait. Default impls return `InvalidResponse { reason: "unsupported" }` for
/// every operation — providers override only what they actually implement, and override
/// `supports()` to declare it. The default-Err pattern is belt-and-braces: even if a
/// provider forgets to override `supports()`, calling an unimplemented op returns a
/// graceful Err rather than panicking, and the ProviderCollection advances to the next
/// provider in the chain.
#[async_trait]
pub trait IndexerProvider: Send + Sync {
    fn name(&self) -> &'static str;

    /// Default `true`. Providers MUST override to return `false` for ops they can't
    /// serve. `ProviderCollection::call` skips providers whose `supports()` returns
    /// false for the requested op.
    fn supports(&self, _op: ProviderOp) -> bool {
        true
    }

    async fn get_raw_tx(&self, _txid: &str) -> Result<Vec<u8>, IndexerError> {
        Err(IndexerError::InvalidResponse {
            provider: self.name(),
            reason: "get_raw_tx unsupported".to_string(),
        })
    }

    async fn get_merkle_proof_tsc(&self, _txid: &str) -> Result<serde_json::Value, IndexerError> {
        Err(IndexerError::InvalidResponse {
            provider: self.name(),
            reason: "get_merkle_proof_tsc unsupported".to_string(),
        })
    }

    async fn get_block_header(&self, _key: BlockKey) -> Result<BlockHeader, IndexerError> {
        Err(IndexerError::InvalidResponse {
            provider: self.name(),
            reason: "get_block_header unsupported".to_string(),
        })
    }

    async fn tx_status(&self, _txid: &str) -> Result<TxStatus, IndexerError> {
        Err(IndexerError::InvalidResponse {
            provider: self.name(),
            reason: "tx_status unsupported".to_string(),
        })
    }

    async fn outspend(&self, _txid: &str, _vout: u32) -> Result<OutspendStatus, IndexerError> {
        Err(IndexerError::InvalidResponse {
            provider: self.name(),
            reason: "outspend unsupported".to_string(),
        })
    }

    async fn fetch_utxos(
        &self,
        _address: &str,
    ) -> Result<Vec<crate::utxo_fetcher::UTXO>, IndexerError> {
        Err(IndexerError::InvalidResponse {
            provider: self.name(),
            reason: "fetch_utxos unsupported".to_string(),
        })
    }

    async fn broadcast_beef(&self, _beef: &[u8]) -> Result<BroadcastResult, IndexerError> {
        Err(IndexerError::InvalidResponse {
            provider: self.name(),
            reason: "broadcast_beef unsupported".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoOpProvider;

    #[async_trait]
    impl IndexerProvider for NoOpProvider {
        fn name(&self) -> &'static str {
            "noop"
        }
    }

    #[tokio::test]
    async fn default_methods_return_unsupported_error() {
        let p = NoOpProvider;
        match p.get_raw_tx("abc").await {
            Err(IndexerError::InvalidResponse { provider, reason }) => {
                assert_eq!(provider, "noop");
                assert!(reason.contains("unsupported"));
            }
            other => panic!("expected InvalidResponse, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn default_supports_returns_true_for_every_op() {
        let p = NoOpProvider;
        for op in [
            ProviderOp::RawTx,
            ProviderOp::MerkleProof,
            ProviderOp::BlockHeader,
            ProviderOp::TxStatus,
            ProviderOp::Outspend,
            ProviderOp::FetchUtxos,
            ProviderOp::BroadcastBeef,
        ] {
            assert!(p.supports(op), "default supports should be true for {:?}", op);
        }
    }

    #[test]
    fn tx_state_equality() {
        assert_eq!(TxState::Mined, TxState::Mined);
        assert_ne!(TxState::Mined, TxState::InMempool);
    }

    #[test]
    fn block_key_variants_construct() {
        let _ = BlockKey::Hash("00000000000000000abc".to_string());
        let _ = BlockKey::Height(800000);
    }
}
