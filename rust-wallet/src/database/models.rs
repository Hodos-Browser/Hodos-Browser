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
    pub pending_utxo_check: bool,  // True if address needs UTXO check (newly created)
    pub created_at: i64,  // Unix timestamp
}

/// UTXO model matching the `utxos` table
#[derive(Debug, Clone)]
pub struct Utxo {
    pub id: Option<i64>,  // None for new UTXOs, Some(id) for existing
    pub address_id: i64,  // References addresses(id)
    pub basket_id: Option<i64>,  // References baskets(id), nullable
    pub txid: String,
    pub vout: i32,
    pub satoshis: i64,
    pub script: String,  // Hex-encoded locking script
    pub first_seen: i64,  // Unix timestamp
    pub last_updated: i64,  // Unix timestamp
    pub is_spent: bool,
    pub spent_txid: Option<String>,
    pub spent_at: Option<i64>,  // Unix timestamp
}
