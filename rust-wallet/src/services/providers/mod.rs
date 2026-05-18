//! Per-network provider implementations of `IndexerProvider`.
//!
//! Phase 1.6d.B steps 4–10 populate each file in turn. `WalletServices::new()`
//! wires these into per-operation `ProviderCollection`s in step 11.

pub mod arc_gorillapool;
pub mod arc_taal;
pub mod bitails;
pub mod gorillapool_mapi;
pub mod gorillapool_ordinals;
pub mod junglebus;
pub mod whatsonchain;

pub use arc_gorillapool::ArcGorillaPoolProvider;
pub use arc_taal::ArcTaalProvider;
pub use bitails::BitailsProvider;
pub use gorillapool_mapi::GorillaPoolMapiProvider;
pub use gorillapool_ordinals::GorillaPoolOrdinalsProvider;
pub use junglebus::JungleBusProvider;
pub use whatsonchain::WhatsOnChainProvider;
