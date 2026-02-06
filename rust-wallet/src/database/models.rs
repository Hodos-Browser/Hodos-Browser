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

/// Output model matching the `outputs` table (V18).
/// Wallet-toolbox compatible schema for tracking unspent transaction outputs.
///
/// Key fields:
/// - `spendable` - True if output is available to spend
/// - `spent_by` - FK to transactions.id when spent
/// - `derivation_prefix`/`derivation_suffix` - BRC-43 key derivation path
/// - `transaction_id` - FK to creating transaction
/// - `user_id` - FK for multi-user support
#[derive(Debug, Clone)]
pub struct Output {
    pub output_id: Option<i64>,  // None for new outputs, Some(id) for existing
    pub user_id: i64,  // References users(userId), required
    pub transaction_id: Option<i64>,  // References transactions(id), the tx that created this output
    pub basket_id: Option<i64>,  // References baskets(id), nullable
    pub spendable: bool,  // True if available to spend (inverse of is_spent)
    pub change: bool,  // True if this is a change output
    pub vout: i32,  // Output index in transaction
    pub satoshis: i64,  // Output value
    pub provided_by: String,  // Who provided: "you", "them", "dojo", etc.
    pub purpose: String,  // Purpose description
    pub output_type: String,  // Type of output (renamed from 'type' which is reserved)
    pub output_description: Option<String>,  // BRC-100 output description
    pub txid: Option<String>,  // Transaction ID (denormalized for queries)
    pub sender_identity_key: Option<String>,  // For received outputs, sender's identity key
    pub derivation_prefix: Option<String>,  // BRC-43 invoice prefix (e.g., "2-receive address")
    pub derivation_suffix: Option<String>,  // BRC-43 invoice suffix (e.g., "0", "1")
    pub custom_instructions: Option<String>,  // BRC-29 custom instructions JSON
    pub spent_by: Option<i64>,  // References transactions(id), the tx that spent this output
    pub sequence_number: Option<i64>,  // For ordering
    pub spending_description: Option<String>,  // Description of spending transaction
    pub script_length: Option<i32>,  // Length of locking script
    pub script_offset: Option<i32>,  // Offset in raw transaction (for BEEF)
    pub locking_script: Option<Vec<u8>>,  // Raw locking script bytes
    pub created_at: i64,  // Unix timestamp
    pub updated_at: i64,  // Unix timestamp
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
