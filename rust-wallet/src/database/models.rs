//! Database models for wallet data structures
//!
//! These structs represent the data stored in the database tables.

// Note: serde traits may be needed in the future for JSON serialization

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

/// Basket model matching the `output_baskets` table
#[derive(Debug, Clone)]
pub struct Basket {
    pub id: Option<i64>,
    pub user_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub token_type: Option<String>,
    pub protocol_id: Option<String>,
    pub is_deleted: bool,
    pub created_at: i64,
    pub updated_at: i64,
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

// =============================================================================
// Phase 5: Labels, Commissions, Supporting Tables (V19)
// =============================================================================

/// Transaction label entity model matching the `tx_labels` table.
/// Labels are deduplicated per user - each unique label string exists once.
/// Junction table `tx_labels_map` links labels to transactions.
#[derive(Debug, Clone)]
pub struct TxLabel {
    pub tx_label_id: Option<i64>,  // None for new labels, Some(id) for existing
    pub user_id: i64,              // References users(userId)
    pub label: String,             // Normalized label text (trimmed, lowercase)
    pub is_deleted: bool,          // Soft delete flag
    pub created_at: i64,           // Unix timestamp
    pub updated_at: i64,           // Unix timestamp
}

/// Transaction label mapping model matching the `tx_labels_map` table.
/// Many-to-many junction between tx_labels and transactions.
#[derive(Debug, Clone)]
pub struct TxLabelMap {
    pub tx_label_id: i64,          // References tx_labels(txLabelId)
    pub transaction_id: i64,       // References transactions(id)
    pub is_deleted: bool,          // Soft delete flag
    pub created_at: i64,           // Unix timestamp
    pub updated_at: i64,           // Unix timestamp
}

/// Commission model matching the `commissions` table.
/// Tracks fee commissions per transaction for wallet-toolbox compatibility.
#[derive(Debug, Clone)]
pub struct Commission {
    pub commission_id: Option<i64>,  // None for new, Some(id) for existing
    pub user_id: i64,                // References users(userId)
    pub transaction_id: i64,         // References transactions(id), unique
    pub satoshis: i64,               // Commission amount in satoshis
    pub key_offset: String,          // Key derivation offset for commission output
    pub is_redeemed: bool,           // Whether commission has been claimed
    pub locking_script: Vec<u8>,     // Locking script for commission output
    pub created_at: i64,             // Unix timestamp
    pub updated_at: i64,             // Unix timestamp
}

/// Settings model matching the `settings` table.
/// Persistent configuration for wallet operation.
#[derive(Debug, Clone)]
pub struct Setting {
    pub storage_identity_key: String,  // Identity key for cloud storage
    pub storage_name: String,          // Storage provider name
    pub chain: String,                 // Network: "main" or "test"
    pub db_type: String,               // Database type: "sqlite"
    pub max_output_script: i32,        // Max script size in bytes (default 500000)
    pub created_at: i64,               // Unix timestamp
    pub updated_at: i64,               // Unix timestamp
}

/// Sync state model matching the `sync_states` table.
/// Tracks multi-device synchronization state per user.
#[derive(Debug, Clone)]
pub struct SyncState {
    pub sync_state_id: Option<i64>,    // None for new, Some(id) for existing
    pub user_id: i64,                  // References users(userId)
    pub storage_identity_key: String,  // Remote storage identity
    pub storage_name: String,          // Remote storage name
    pub status: String,                // Sync status: "unknown", "syncing", "synced", "error"
    pub init: bool,                    // Whether initial sync completed
    pub ref_num: String,               // Unique reference number
    pub sync_map: String,              // JSON sync state mapping
    pub sync_when: Option<i64>,        // Last sync timestamp
    pub satoshis: Option<i64>,         // Balance at sync point
    pub error_local: Option<String>,   // Local error message
    pub error_other: Option<String>,   // Remote error message
    pub created_at: i64,               // Unix timestamp
    pub updated_at: i64,               // Unix timestamp
}
