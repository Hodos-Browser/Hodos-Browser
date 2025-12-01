//! Database models for wallet data structures
//!
//! These structs represent the data stored in the database tables.

use serde::{Deserialize, Serialize};

/// Wallet model matching the `wallets` table
#[derive(Debug, Clone)]
pub struct Wallet {
    pub id: Option<i64>,  // None for new wallets, Some(id) for existing
    pub mnemonic: String,
    pub current_index: i32,
    pub backed_up: bool,
    pub created_at: i64,  // Unix timestamp
}

/// Address model matching the `addresses` table
#[derive(Debug, Clone)]
pub struct Address {
    pub id: Option<i64>,  // None for new addresses, Some(id) for existing
    pub wallet_id: i64,
    pub index: i32,
    pub address: String,
    pub public_key: String,
    pub used: bool,
    pub balance: i64,
    pub created_at: i64,  // Unix timestamp
}
