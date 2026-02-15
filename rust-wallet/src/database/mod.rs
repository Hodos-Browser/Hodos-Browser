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
pub mod parent_transaction_repo;
pub mod block_header_repo;
pub mod proven_tx_repo;
pub mod proven_tx_req_repo;
pub mod basket_repo;
pub mod tag_repo;
pub mod certificate_repo;
pub mod message_relay_repo;
pub mod user_repo;
pub mod output_repo;
// Phase 5: Labels, Commissions, Supporting Tables (V19)
pub mod tx_label_repo;
pub mod commission_repo;
pub mod settings_repo;
pub mod sync_state_repo;

pub use connection::WalletDatabase;
pub use models::{Wallet, User, Address, Output, ParentTransaction, BlockHeader, ProvenTx, ProvenTxReq, Basket, OutputTag, OutputTagMap};
// Phase 5 models
pub use models::{TxLabel, TxLabelMap, Commission, Setting, SyncState};
pub use wallet_repo::WalletRepository;
pub use address_repo::AddressRepository;
pub use transaction_repo::TransactionRepository;
pub use migration::migrate_json_to_database;
pub use helpers::{get_master_private_key_from_db, get_master_public_key_from_db, derive_key_for_output, address_to_address_info, output_to_fetcher_utxo};
pub use parent_transaction_repo::ParentTransactionRepository;
pub use block_header_repo::BlockHeaderRepository;
pub use proven_tx_repo::ProvenTxRepository;
pub use proven_tx_req_repo::ProvenTxReqRepository;
pub use basket_repo::BasketRepository;
pub use tag_repo::TagRepository;
pub use certificate_repo::CertificateRepository;
pub use message_relay_repo::{MessageRelayRepository, RelayMessage, MessageRelayStats};
pub use user_repo::UserRepository;
pub use output_repo::OutputRepository;
// Phase 5 repositories
pub use tx_label_repo::TxLabelRepository;
pub use commission_repo::CommissionRepository;
pub use settings_repo::SettingsRepository;
pub use sync_state_repo::SyncStateRepository;
