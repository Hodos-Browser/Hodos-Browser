//! WalletServices — unified indexer facade for the wallet backend.
//!
//! Phase 1.6d.B introduces this module as dormant scaffolding. Subsequent commits
//! (1.6d.C through 1.6d.F) migrate existing call sites onto it. See
//! `development-docs/Sigma-BRC121-Sprint/phase-1.6-indexer-resilience/DESIGN.md`.

pub mod collection;
pub mod provider;
pub mod providers;

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

pub use collection::{BoxFut, ProviderCollection, ProviderStats};
pub use provider::{
    BlockHeader, BlockKey, BroadcastResult, IndexerError, IndexerProvider, OutspendStatus,
    ProviderOp, TxState, TxStatus,
};

use providers::{
    ArcGorillaPoolProvider, ArcTaalProvider, BitailsProvider, GorillaPoolMapiProvider,
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
        let arc_tl: Arc<dyn IndexerProvider> = Arc::new(ArcTaalProvider::new(client.clone()));
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
            // Kept on tx_status where its response shape is reliable.
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
            broadcast: ProviderCollection::new(vec![arc_gp, arc_tl, gp_mapi, woc]),
        }
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
        let soft =
            ProviderCollection::<dyn IndexerProvider>::adaptive_soft_timeout_for_payload(beef.len());
        let beef = beef.to_vec();
        self.broadcast
            .call(ProviderOp::BroadcastBeef, soft, move |p| {
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
