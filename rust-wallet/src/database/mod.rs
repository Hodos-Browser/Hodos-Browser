//! Database module for HodosBrowser wallet
//!
//! This module provides SQLite database functionality for wallet data storage,
//! replacing JSON file storage with a proper database solution.

pub mod connection;
pub mod migrations;
pub mod models;
pub mod wallet_repo;
pub mod address_repo;
pub mod transaction_repo;
pub mod migration;
pub mod helpers;

pub use connection::WalletDatabase;
pub use models::{Wallet, Address};
pub use wallet_repo::WalletRepository;
pub use address_repo::AddressRepository;
pub use transaction_repo::TransactionRepository;
pub use migration::migrate_json_to_database;
pub use helpers::{get_master_private_key_from_db, get_master_public_key_from_db, derive_private_key_from_db, address_to_address_info};
