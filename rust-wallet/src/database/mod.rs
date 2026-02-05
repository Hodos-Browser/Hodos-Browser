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
pub mod utxo_repo;
pub mod migration;
pub mod helpers;
pub mod parent_transaction_repo;
pub mod merkle_proof_repo;
pub mod block_header_repo;
pub mod proven_tx_repo;
pub mod proven_tx_req_repo;
pub mod basket_repo;
pub mod tag_repo;
pub mod certificate_repo;
pub mod message_relay_repo;
pub mod user_repo;

pub use connection::WalletDatabase;
pub use models::{Wallet, User, Address, Utxo, ParentTransaction, MerkleProof, BlockHeader, ProvenTx, ProvenTxReq, Basket, OutputTag, OutputTagMap};
pub use wallet_repo::WalletRepository;
pub use address_repo::AddressRepository;
pub use transaction_repo::TransactionRepository;
pub use utxo_repo::UtxoRepository;
pub use migration::migrate_json_to_database;
pub use helpers::{get_master_private_key_from_db, get_master_public_key_from_db, derive_private_key_for_utxo, address_to_address_info, utxo_to_fetcher_utxo};
pub use parent_transaction_repo::ParentTransactionRepository;
pub use merkle_proof_repo::MerkleProofRepository;
pub use block_header_repo::BlockHeaderRepository;
pub use proven_tx_repo::ProvenTxRepository;
pub use proven_tx_req_repo::ProvenTxReqRepository;
pub use basket_repo::BasketRepository;
pub use tag_repo::TagRepository;
pub use certificate_repo::CertificateRepository;
pub use message_relay_repo::{MessageRelayRepository, RelayMessage, MessageRelayStats};
pub use user_repo::UserRepository;
