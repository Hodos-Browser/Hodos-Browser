//! Database module for HodosBrowser wallet
//!
//! This module provides SQLite database functionality for wallet data storage,
//! replacing JSON file storage with a proper database solution.

pub mod connection;
pub mod migrations;
pub mod models;
pub mod wallet_repo;
pub mod address_repo;
pub mod migration;

pub use connection::WalletDatabase;
pub use models::{Wallet, Address};
pub use wallet_repo::WalletRepository;
pub use address_repo::AddressRepository;
pub use migration::migrate_json_to_database;
