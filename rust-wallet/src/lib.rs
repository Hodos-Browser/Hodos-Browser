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
// Wallet-Hardening WS1 — spent-input reconcile primitives (c1 check_outpoint_spent; c2/c3 dormant).
pub mod spv_checks;  // B5-T1-P7a-1
pub mod header_chain;  // B5-T1-P7a-1
pub mod chain_tracker;  // B5-T1-P7a-1
pub mod proof_store;  // B5-T1-P7a-2
pub mod reconcile;
pub mod backup;  // classify/connection tests drive the production backup encode/decode
pub mod tokens;  // B5-T2-P1 (classify.rs calls it)
pub mod classify;  // B5-T1-P5 (ingest.rs calls it)
pub mod ingest;
pub mod scan;  // B5-T1-P6

// Re-export commonly used modules for convenience
pub use crypto::brc2;
pub use crypto::aesgcm_custom;
