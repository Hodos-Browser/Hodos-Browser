//! Database module for HodosBrowser wallet
//!
//! This module provides SQLite database functionality for wallet data storage,
//! replacing JSON file storage with a proper database solution.

pub mod connection;
pub mod migrations;

pub use connection::WalletDatabase;
