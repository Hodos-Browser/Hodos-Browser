///! Cryptographic operations for Bitcoin wallet
///
///! Self-contained crypto module with no external wallet-core dependencies.

pub mod brc42;
pub mod brc43;
pub mod brc2;
pub mod ghash;
pub mod aesgcm_custom;
pub mod pin;
pub mod dpapi;

#[cfg(test)]
mod aesgcm_custom_test;
pub mod keys;
pub mod signing;

// Note: Items are imported directly from sub-modules in handlers.rs
// No re-exports needed since we're a binary application, not a library
