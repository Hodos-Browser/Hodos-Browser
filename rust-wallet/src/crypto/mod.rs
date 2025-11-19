///! Cryptographic operations for Bitcoin wallet
///
///! Self-contained crypto module with no external wallet-core dependencies.

pub mod brc42;
pub mod brc43;
pub mod keys;
pub mod signing;

// Note: Items are imported directly from sub-modules in handlers.rs
// No re-exports needed since we're a binary application, not a library
