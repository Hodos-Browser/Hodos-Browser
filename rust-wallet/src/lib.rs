//! Hodos Wallet Library
//!
//! This library provides the core wallet functionality for the Hodos Browser.
//! The main binary is in main.rs, but this lib.rs exposes modules for testing.

pub mod arc_status;
pub mod crypto;
pub mod certificate;
pub mod database;
pub mod transaction;
pub mod recovery;
pub mod action_storage;
pub mod json_storage;
pub mod cache_errors;
pub mod utxo_fetcher;
pub mod beef;
pub mod script;
pub mod balance_cache;
pub mod price_cache;
pub mod overlay;
pub mod services;
// Phase 2.6-A.5 — engine-to-Rust scaffolding module (dormant; wired into AppState in A.6).
pub mod permission_service;
// Phase 2.6-G — Rust ManifestFetcher port (dormant; wired into the unknown-domain dispatch in G.3).
pub mod manifest;

// Re-export commonly used modules for convenience
pub use crypto::brc2;
pub use crypto::aesgcm_custom;
