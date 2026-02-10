//! Hodos Wallet Library
//!
//! This library provides the core wallet functionality for the Hodos Browser.
//! The main binary is in main.rs, but this lib.rs exposes modules for testing.

pub mod crypto;
pub mod certificate;
pub mod database;
pub mod transaction;
pub mod recovery;
pub mod action_storage;
pub mod json_storage;
pub mod cache_errors;
pub mod utxo_fetcher;

// Re-export commonly used modules for convenience
pub use crypto::brc2;
pub use crypto::aesgcm_custom;
