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

/// User model matching the `users` table.
/// Represents an identity (master public key) that owns wallet data.
/// For single-user wallets, there is one default user linked to the wallet's master key.
#[derive(Debug, Clone)]
pub struct User {
    pub user_id: Option<i64>,  // None for new users, Some(id) for existing
    pub identity_key: String,  // Master public key (hex-encoded, 33 bytes compressed)
    pub active_storage: String,  // Storage mode: "local" (default)
    pub created_at: i64,  // Unix timestamp
    pub updated_at: i64,  // Unix timestamp
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
    pub address_id: Option<i64>,  // References addresses(id), nullable for basket outputs
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
    pub custom_instructions: Option<String>,  // BRC-29 custom instructions (added in v5)
    pub output_description: Option<String>,  // BRC-100 output description (added in v14)
}

/// Parent transaction model matching the `parent_transactions` table
#[derive(Debug, Clone)]
pub struct ParentTransaction {
    pub id: i64,
    pub utxo_id: Option<i64>,  // Nullable - allows external parent transactions
    pub txid: String,
    pub raw_hex: String,
    pub cached_at: i64,  // Unix timestamp
}

/// Merkle proof model matching the `merkle_proofs` table
#[derive(Debug, Clone)]
pub struct MerkleProof {
    pub id: i64,
    pub parent_txn_id: i64,
    pub block_height: u32,
    pub tx_index: u64,
    pub target_hash: String,
    pub nodes: Vec<String>, // Parsed from JSON
    pub cached_at: i64,  // Unix timestamp
}

/// Block header model matching the `block_headers` table
#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub id: i64,
    pub block_hash: String,
    pub height: u32,
    pub header_hex: String,
    pub cached_at: i64,  // Unix timestamp
}

/// Proven transaction model matching the `proven_txs` table.
/// Records are IMMUTABLE — once created, never updated.
/// Stores a confirmed transaction along with its merkle proof.
#[derive(Debug, Clone)]
pub struct ProvenTx {
    pub proven_tx_id: i64,
    pub txid: String,
    pub height: u32,
    pub tx_index: u64,
    pub merkle_path: Vec<u8>,    // TSC JSON serialized as bytes
    pub raw_tx: Vec<u8>,         // Raw transaction bytes
    pub block_hash: String,
    pub merkle_root: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Proven transaction request model matching the `proven_tx_reqs` table.
/// Tracks the lifecycle of proof acquisition for a broadcast transaction.
#[derive(Debug, Clone)]
pub struct ProvenTxReq {
    pub proven_tx_req_id: i64,
    pub proven_tx_id: Option<i64>,  // Links to proven_txs once proof acquired
    pub status: String,              // ProvenTxReqStatus value
    pub attempts: i32,
    pub notified: bool,
    pub txid: String,
    pub batch: Option<String>,
    pub history: String,             // JSON timestamped state transition log
    pub notify: String,              // JSON list of transaction IDs to notify
    pub raw_tx: Vec<u8>,
    pub input_beef: Option<Vec<u8>>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Basket model matching the `baskets` table
#[derive(Debug, Clone)]
pub struct Basket {
    pub id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub token_type: Option<String>,
    pub protocol_id: Option<String>,
    pub created_at: i64,
    pub last_used: Option<i64>,
}

/// Output tag model matching the `output_tags` table
#[derive(Debug, Clone)]
pub struct OutputTag {
    pub id: Option<i64>,
    pub tag: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_deleted: bool,
}

/// Output tag map model matching the `output_tag_map` table
#[derive(Debug, Clone)]
pub struct OutputTagMap {
    pub id: Option<i64>,
    pub output_id: i64,
    pub output_tag_id: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_deleted: bool,
}
