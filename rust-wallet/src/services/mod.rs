//! WalletServices — unified indexer facade for the wallet backend.
//!
//! Phase 1.6d.B introduces this module as dormant scaffolding. Subsequent commits
//! (1.6d.C through 1.6d.F) migrate existing call sites onto it. See
//! `development-docs/Sigma-BRC121-Sprint/phase-1.6-indexer-resilience/DESIGN.md`.

pub mod call_class;
pub mod collection;
pub mod provider;
pub mod providers;

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

pub use call_class::CallClass;
pub use collection::{BoxFut, FailureKind, ProviderCollection, ProviderFailure, ProviderStats};
pub use provider::{
    BlockHeader, BlockKey, BroadcastResult, IndexerError, IndexerProvider, OutspendStatus,
    ProviderOp, TxState, TxStatus,
};

use providers::{
    ArcGorillaPoolProvider, BitailsProvider, GorillaPoolMapiProvider,
    GorillaPoolOrdinalsProvider, JungleBusProvider, WhatsOnChainProvider,
};

/// Per-operation soft-timeout defaults from DESIGN §2.3.
mod soft_timeouts {
    use std::time::Duration;
    pub const RAW_TX: Duration = Duration::from_secs(8);
    pub const MERKLE_PROOF: Duration = Duration::from_secs(10);
    pub const BLOCK_HEADER: Duration = Duration::from_secs(8);
    pub const TX_STATUS: Duration = Duration::from_secs(8);
    pub const OUTSPEND: Duration = Duration::from_secs(8);
    pub const FETCH_UTXOS: Duration = Duration::from_secs(15);
    // `broadcast_beef` uses ProviderCollection::adaptive_soft_timeout_for_payload.
}

/// Unified indexer facade. Owns one shared `reqwest::Client` and one
/// `ProviderCollection` per operation (per-op chain orders per DESIGN §3).
///
/// **Dormant in 1.6d.B.** No existing call site routes through this yet; the field
/// on `AppState` exists so 1.6d.C can wire the facade live with a one-commit change.
pub struct WalletServices {
    /// Shared HTTP client across all providers — connection-pool reuse + a hard
    /// 30s ceiling per request. Per-call soft timeouts are layered on top.
    pub client: reqwest::Client,

    raw_tx: ProviderCollection<dyn IndexerProvider>,
    proof: ProviderCollection<dyn IndexerProvider>,
    header: ProviderCollection<dyn IndexerProvider>,
    tx_status_chain: ProviderCollection<dyn IndexerProvider>,
    outspend_chain: ProviderCollection<dyn IndexerProvider>,
    utxo: ProviderCollection<dyn IndexerProvider>,
    broadcast: ProviderCollection<dyn IndexerProvider>,
}

impl WalletServices {
    /// Construct with the canonical mainnet chains per DESIGN §3. Mainnet-only by
    /// design (Q1 in the 1.6d.B kickoff — no `Chain` enum today).
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(8)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        // One Arc per provider instance, shared across the chains that include it.
        let arc_gp: Arc<dyn IndexerProvider> =
            Arc::new(ArcGorillaPoolProvider::new(client.clone()));
        let gp_mapi: Arc<dyn IndexerProvider> =
            Arc::new(GorillaPoolMapiProvider::new(client.clone()));
        let gp_ords: Arc<dyn IndexerProvider> =
            Arc::new(GorillaPoolOrdinalsProvider::new(client.clone()));
        let woc: Arc<dyn IndexerProvider> = Arc::new(WhatsOnChainProvider::new(client.clone()));
        let jb: Arc<dyn IndexerProvider> = Arc::new(JungleBusProvider::new(client.clone()));
        let bt: Arc<dyn IndexerProvider> = Arc::new(BitailsProvider::new(client.clone()));

        Self {
            client,
            // Bitails demoted from raw_tx/proof/header chains: it returns HTTP 500
            // (instead of a proper 404) for unknown txids, poisoning error messages.
            // Kept on tx_status where its response shape is reliable, and on broadcast (B5-T1-P7b).
            raw_tx: ProviderCollection::new(vec![arc_gp.clone(), woc.clone(), jb.clone()]),
            proof: ProviderCollection::new(vec![arc_gp.clone(), woc.clone(), jb.clone()]),
            header: ProviderCollection::new(vec![woc.clone(), jb.clone()]),
            tx_status_chain: ProviderCollection::new(vec![
                arc_gp.clone(),
                woc.clone(),
                jb.clone(),
                bt.clone(),
            ]),
            outspend_chain: ProviderCollection::new(vec![woc.clone(), jb.clone()]),
            utxo: ProviderCollection::new(vec![woc.clone(), gp_ords.clone()]),
            // B5-T1-P7b (owner 2026-10-10): two independent operators before the raw-only GorillaPool
            // MAPI and WhatsOnChain. TAAL ARC was removed: it answered 401 with and without our key.
            broadcast: ProviderCollection::new(vec![arc_gp, bt, gp_mapi, woc]),
        }
    }

    /// Test-only (B5-T1-P7b): as `with_test_chains`, with an outspend chain too.
    #[cfg(test)]
    pub fn with_test_chains_and_outspend(
        tx_status: Vec<Arc<dyn IndexerProvider>>,
        broadcast: Vec<Arc<dyn IndexerProvider>>,
        outspend: Vec<Arc<dyn IndexerProvider>>,
    ) -> Self {
        let mut s = Self::with_test_chains(tx_status, broadcast);
        s.outspend_chain = ProviderCollection::new(outspend);
        s
    }

    /// Test-only (B5-T1-P7a-2): one provider answers every chain (status, broadcast, proof, raw tx,
    /// header), so a proof writer's fetch goes through the real facade to a scripted answer.
    #[cfg(test)]
    pub fn with_one_test_provider(p: Arc<dyn IndexerProvider>) -> Self {
        let mut s = Self::with_test_chains(vec![p.clone()], vec![p.clone()]);
        s.proof = ProviderCollection::new(vec![p.clone()]);
        s.raw_tx = ProviderCollection::new(vec![p.clone()]);
        s.header = ProviderCollection::new(vec![p]);
        s
    }

    /// Test-only: the real facade with chosen `tx_status` and `broadcast` chains, so a
    /// handler under test runs its real code against a scripted chain (B5-T1-P2). Every
    /// other chain is empty and fails fast.
    #[cfg(test)]
    pub fn with_test_chains(
        tx_status: Vec<Arc<dyn IndexerProvider>>,
        broadcast: Vec<Arc<dyn IndexerProvider>>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            raw_tx: ProviderCollection::new(vec![]),
            proof: ProviderCollection::new(vec![]),
            header: ProviderCollection::new(vec![]),
            tx_status_chain: ProviderCollection::new(tx_status),
            outspend_chain: ProviderCollection::new(vec![]),
            utxo: ProviderCollection::new(vec![]),
            broadcast: ProviderCollection::new(broadcast),
        }
    }

    /// Test-only (B5-T1-P7b `P7b-B7`): every chain's full provider order and the order `call`
    /// would actually try for that chain's operation.
    #[cfg(test)]
    pub(crate) fn chain_orders(&self) -> Vec<(&'static str, Vec<&'static str>, Vec<&'static str>)> {
        vec![
            ("raw_tx", self.raw_tx.current_order(), self.raw_tx.eligible_order(ProviderOp::RawTx)),
            ("proof", self.proof.current_order(), self.proof.eligible_order(ProviderOp::MerkleProof)),
            ("header", self.header.current_order(), self.header.eligible_order(ProviderOp::BlockHeader)),
            ("tx_status", self.tx_status_chain.current_order(), self.tx_status_chain.eligible_order(ProviderOp::TxStatus)),
            ("outspend", self.outspend_chain.current_order(), self.outspend_chain.eligible_order(ProviderOp::Outspend)),
            ("utxo", self.utxo.current_order(), self.utxo.eligible_order(ProviderOp::FetchUtxos)),
            ("broadcast", self.broadcast.current_order(), self.broadcast.eligible_order(ProviderOp::BroadcastBeef)),
        ]
    }

    // --- per-op wrappers (per DESIGN §2.3) ---

    pub async fn get_raw_tx(&self, txid: &str) -> Result<Vec<u8>, IndexerError> {
        let txid = txid.to_string();
        self.raw_tx
            .call(ProviderOp::RawTx, soft_timeouts::RAW_TX, move |p| {
                let txid = txid.clone();
                Box::pin(async move { p.get_raw_tx(&txid).await })
            })
            .await
    }

    pub async fn get_merkle_proof_tsc(&self, txid: &str) -> Result<Value, IndexerError> {
        let txid = txid.to_string();
        self.proof
            .call(ProviderOp::MerkleProof, soft_timeouts::MERKLE_PROOF, move |p| {
                let txid = txid.clone();
                Box::pin(async move { p.get_merkle_proof_tsc(&txid).await })
            })
            .await
    }

    pub async fn get_block_header(&self, key: BlockKey) -> Result<BlockHeader, IndexerError> {
        self.header
            .call(ProviderOp::BlockHeader, soft_timeouts::BLOCK_HEADER, move |p| {
                let key = key.clone();
                Box::pin(async move { p.get_block_header(key).await })
            })
            .await
    }

    pub async fn tx_status(&self, txid: &str) -> Result<TxStatus, IndexerError> {
        let txid = txid.to_string();
        self.tx_status_chain
            .call(ProviderOp::TxStatus, soft_timeouts::TX_STATUS, move |p| {
                let txid = txid.clone();
                Box::pin(async move { p.tx_status(&txid).await })
            })
            .await
    }

    /// `tx_status`, where `NotFound` means every provider in the chain said not found.
    /// For callers that release coins on "absent" (`abort_action`); see
    /// `ProviderCollection::call_unanimous_not_found`.
    pub async fn tx_status_unanimous(&self, txid: &str) -> Result<TxStatus, IndexerError> {
        let txid = txid.to_string();
        self.tx_status_chain
            .call_unanimous_not_found(ProviderOp::TxStatus, soft_timeouts::TX_STATUS, move |p| {
                let txid = txid.clone();
                Box::pin(async move { p.tx_status(&txid).await })
            })
            .await
    }

    pub async fn outspend(&self, txid: &str, vout: u32) -> Result<OutspendStatus, IndexerError> {
        let txid = txid.to_string();
        self.outspend_chain
            .call(ProviderOp::Outspend, soft_timeouts::OUTSPEND, move |p| {
                let txid = txid.clone();
                Box::pin(async move { p.outspend(&txid, vout).await })
            })
            .await
    }

    pub async fn fetch_utxos(
        &self,
        address: &str,
    ) -> Result<Vec<crate::utxo_fetcher::UTXO>, IndexerError> {
        let address = address.to_string();
        self.utxo
            .call(ProviderOp::FetchUtxos, soft_timeouts::FETCH_UTXOS, move |p| {
                let address = address.clone();
                Box::pin(async move { p.fetch_utxos(&address).await })
            })
            .await
    }

    pub async fn broadcast_beef(&self, beef: &[u8]) -> Result<BroadcastResult, IndexerError> {
        self.broadcast_beef_traced(beef).await.0
    }

    /// `broadcast_beef`, plus every provider that failed on the way (B5-T1-P2: the
    /// caller judges temporary vs permanent over the whole chain, and logs each
    /// fall-through with the txid).
    pub async fn broadcast_beef_traced(
        &self,
        beef: &[u8],
    ) -> (Result<BroadcastResult, IndexerError>, Vec<ProviderFailure>) {
        let soft =
            ProviderCollection::<dyn IndexerProvider>::adaptive_soft_timeout_for_payload(beef.len());
        let beef = beef.to_vec();
        self.broadcast
            .call_traced(ProviderOp::BroadcastBeef, soft, move |p| {
                let beef = beef.clone();
                Box::pin(async move { p.broadcast_beef(&beef).await })
            })
            .await
    }

    /// Telemetry snapshot covering every chain. Useful for the activity log and for
    /// 1.6e fault-injection tests confirming the fallback chain actually exercised
    /// non-primary providers.
    pub fn snapshot_all_stats(&self) -> WalletServicesStats {
        WalletServicesStats {
            raw_tx: self.raw_tx.snapshot_stats(),
            proof: self.proof.snapshot_stats(),
            header: self.header.snapshot_stats(),
            tx_status: self.tx_status_chain.snapshot_stats(),
            outspend: self.outspend_chain.snapshot_stats(),
            utxo: self.utxo.snapshot_stats(),
            broadcast: self.broadcast.snapshot_stats(),
        }
    }
}

impl Default for WalletServices {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct WalletServicesStats {
    pub raw_tx: Vec<(&'static str, ProviderStats)>,
    pub proof: Vec<(&'static str, ProviderStats)>,
    pub header: Vec<(&'static str, ProviderStats)>,
    pub tx_status: Vec<(&'static str, ProviderStats)>,
    pub outspend: Vec<(&'static str, ProviderStats)>,
    pub utxo: Vec<(&'static str, ProviderStats)>,
    pub broadcast: Vec<(&'static str, ProviderStats)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallet_services_constructs_without_panic() {
        let _ = WalletServices::new();
    }

    #[test]
    fn fresh_facade_has_empty_stats_everywhere() {
        let s = WalletServices::new();
        let stats = s.snapshot_all_stats();
        assert!(stats.raw_tx.is_empty(), "fresh facade should record zero raw_tx calls");
        assert!(stats.proof.is_empty());
        assert!(stats.header.is_empty());
        assert!(stats.tx_status.is_empty());
        assert!(stats.outspend.is_empty());
        assert!(stats.utxo.is_empty());
        assert!(stats.broadcast.is_empty());
    }
}
