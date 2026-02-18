//! Database backup and restore utilities
//!
//! This module provides functions for backing up and restoring the wallet database.
//! Supports:
//! - File-based backup (SQLite database copy)
//! - JSON export (non-sensitive data)
//! - Encrypted wallet backup (AES-256-GCM, all entities, excludes mnemonic)

use crate::database::WalletDatabase;
use rusqlite::{Connection, Result};
use std::fs;
use std::path::Path;
use log::info;
use serde::{Serialize, Deserialize};

// ============================================================================
// Encrypted Wallet Backup — Serde Structs
// ============================================================================

/// Outer file format for .hodos-wallet files
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EncryptedBackup {
    pub format: String,
    pub version: u32,
    pub created_at: i64,
    pub salt: String,   // hex(16 bytes)
    pub data: String,   // base64(nonce_12 || ciphertext || tag_16)
}

/// Decrypted payload containing all wallet entities including mnemonic
#[derive(Serialize, Deserialize, Debug)]
pub struct BackupPayload {
    pub version: u32,
    pub identity_key: String,
    pub mnemonic: String,
    pub wallet: BackupWallet,
    pub users: Vec<BackupUser>,
    pub addresses: Vec<BackupAddress>,
    pub output_baskets: Vec<BackupBasket>,
    pub transactions: Vec<BackupTransaction>,
    pub outputs: Vec<BackupOutput>,
    pub proven_txs: Vec<BackupProvenTx>,
    pub proven_tx_reqs: Vec<BackupProvenTxReq>,
    pub certificates: Vec<BackupCertificate>,
    pub certificate_fields: Vec<BackupCertificateField>,
    pub output_tags: Vec<BackupOutputTag>,
    pub output_tag_map: Vec<BackupOutputTagMap>,
    pub tx_labels: Vec<BackupTxLabel>,
    pub tx_labels_map: Vec<BackupTxLabelMap>,
    pub commissions: Vec<BackupCommission>,
    pub settings: Vec<BackupSetting>,
    pub sync_states: Vec<BackupSyncState>,
    pub parent_transactions: Vec<BackupParentTransaction>,
    pub block_headers: Vec<BackupBlockHeader>,
    #[serde(default)]
    pub domain_permissions: Vec<BackupDomainPermission>,
    #[serde(default)]
    pub cert_field_permissions: Vec<BackupCertFieldPermission>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupWallet {
    pub id: i64,
    pub current_index: i32,
    pub backed_up: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupUser {
    #[serde(rename = "userId")]
    pub user_id: i64,
    pub identity_key: String,
    pub active_storage: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupAddress {
    pub id: i64,
    pub wallet_id: i64,
    pub index: i32,
    pub address: String,
    pub public_key: String,
    pub used: bool,
    pub balance: i64,
    pub pending_utxo_check: bool,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupBasket {
    #[serde(rename = "basketId")]
    pub basket_id: i64,
    pub user_id: Option<i64>,
    pub name: String,
    pub number_of_desired_utxos: i32,
    pub minimum_desired_utxo_value: i64,
    pub is_deleted: bool,
    pub description: Option<String>,
    pub token_type: Option<String>,
    pub protocol_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupTransaction {
    pub id: i64,
    pub user_id: Option<i64>,
    pub proven_tx_id: Option<i64>,
    pub txid: Option<String>,
    pub reference_number: String,
    pub raw_tx: Option<String>,
    pub description: Option<String>,
    pub status: String,
    pub is_outgoing: bool,
    pub satoshis: i64,
    pub input_beef: Option<String>,  // base64
    pub version: i32,
    pub lock_time: i64,
    pub block_height: Option<i64>,
    pub confirmations: i32,
    pub failed_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupOutput {
    #[serde(rename = "outputId")]
    pub output_id: i64,
    pub user_id: i64,
    pub transaction_id: Option<i64>,
    pub basket_id: Option<i64>,
    pub spendable: bool,
    pub change: bool,
    pub vout: i32,
    pub satoshis: i64,
    pub provided_by: String,
    pub purpose: String,
    #[serde(rename = "type")]
    pub output_type: String,
    pub output_description: Option<String>,
    pub txid: Option<String>,
    pub sender_identity_key: Option<String>,
    pub derivation_prefix: Option<String>,
    pub derivation_suffix: Option<String>,
    pub custom_instructions: Option<String>,
    pub spent_by: Option<i64>,
    pub sequence_number: Option<i64>,
    pub spending_description: Option<String>,
    pub script_length: Option<i64>,
    pub script_offset: Option<i64>,
    pub locking_script: Option<String>,  // base64
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupProvenTx {
    #[serde(rename = "provenTxId")]
    pub proven_tx_id: i64,
    pub txid: String,
    pub height: i64,
    pub tx_index: i64,
    pub merkle_path: String,  // base64
    pub raw_tx: String,       // base64
    pub block_hash: String,
    pub merkle_root: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupProvenTxReq {
    #[serde(rename = "provenTxReqId")]
    pub proven_tx_req_id: i64,
    pub proven_tx_id: Option<i64>,
    pub txid: String,
    pub status: String,
    pub attempts: i32,
    pub notified: bool,
    pub batch: Option<String>,
    pub history: String,
    pub notify: String,
    pub raw_tx: String,        // base64
    pub input_beef: Option<String>,  // base64
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupCertificate {
    #[serde(rename = "certificateId")]
    pub certificate_id: i64,
    pub user_id: i64,
    #[serde(rename = "type")]
    pub cert_type: String,
    pub serial_number: String,
    pub certifier: String,
    pub subject: String,
    pub verifier: Option<String>,
    pub revocation_outpoint: String,
    pub signature: String,
    pub is_deleted: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupCertificateField {
    #[serde(rename = "certificateId")]
    pub certificate_id: i64,
    pub user_id: i64,
    pub field_name: String,
    pub field_value: String,
    pub master_key: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupOutputTag {
    pub id: i64,
    pub user_id: Option<i64>,
    pub tag: String,
    pub is_deleted: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupOutputTagMap {
    pub id: i64,
    pub output_id: i64,
    pub output_tag_id: i64,
    pub is_deleted: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupTxLabel {
    #[serde(rename = "txLabelId")]
    pub tx_label_id: i64,
    pub user_id: i64,
    pub label: String,
    pub is_deleted: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupTxLabelMap {
    #[serde(rename = "txLabelId")]
    pub tx_label_id: i64,
    pub transaction_id: i64,
    pub is_deleted: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupCommission {
    #[serde(rename = "commissionId")]
    pub commission_id: i64,
    pub user_id: i64,
    pub transaction_id: i64,
    pub satoshis: i64,
    pub key_offset: String,
    pub is_redeemed: bool,
    pub locking_script: String,  // base64
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupSetting {
    pub storage_identity_key: String,
    pub storage_name: String,
    pub chain: String,
    pub dbtype: String,
    pub max_output_script: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupSyncState {
    #[serde(rename = "syncStateId")]
    pub sync_state_id: i64,
    pub user_id: i64,
    pub storage_identity_key: String,
    pub storage_name: String,
    pub status: String,
    pub init: bool,
    pub ref_num: String,
    pub sync_map: String,
    pub sync_when: Option<i64>,
    pub satoshis: Option<i64>,
    pub error_local: Option<String>,
    pub error_other: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupParentTransaction {
    pub id: i64,
    pub utxo_id: Option<i64>,
    pub txid: String,
    pub raw_hex: String,
    pub cached_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupBlockHeader {
    pub id: i64,
    pub block_hash: String,
    pub height: i64,
    pub header_hex: String,
    pub cached_at: i64,
}

// ============================================================================
// Phase 2.1: Domain permission backup structs
// ============================================================================

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupDomainPermission {
    pub domain: String,
    #[serde(rename = "trustLevel")]
    pub trust_level: String,
    #[serde(rename = "perTxLimitCents")]
    pub per_tx_limit_cents: i64,
    #[serde(rename = "perSessionLimitCents")]
    pub per_session_limit_cents: i64,
    #[serde(rename = "rateLimitPerMin")]
    pub rate_limit_per_min: i64,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupCertFieldPermission {
    pub domain: String,
    #[serde(rename = "certType")]
    pub cert_type: String,
    #[serde(rename = "fieldName")]
    pub field_name: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
}

// ============================================================================
// collect_payload — read all entities from DB
// ============================================================================

/// Collect all wallet entities into a BackupPayload.
/// Requires the identity_key (hex-encoded master public key) and plaintext mnemonic.
pub fn collect_payload(conn: &Connection, identity_key: &str, mnemonic: &str) -> Result<BackupPayload> {
    info!("   Collecting backup payload...");

    // Wallet (no mnemonic, no pin_salt)
    let wallet = conn.query_row(
        "SELECT id, current_index, backed_up, created_at, updated_at FROM wallets LIMIT 1",
        [],
        |row| Ok(BackupWallet {
            id: row.get(0)?,
            current_index: row.get(1)?,
            backed_up: row.get::<_, i32>(2)? != 0,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        }),
    )?;

    // Users
    let users = {
        let mut stmt = conn.prepare("SELECT userId, identity_key, active_storage, created_at, updated_at FROM users")?;
        let rows = stmt.query_map([], |row| Ok(BackupUser {
            user_id: row.get(0)?,
            identity_key: row.get(1)?,
            active_storage: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Addresses
    let addresses = {
        let mut stmt = conn.prepare(
            "SELECT id, wallet_id, \"index\", address, public_key, used, balance, pending_utxo_check, created_at FROM addresses"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupAddress {
            id: row.get(0)?,
            wallet_id: row.get(1)?,
            index: row.get(2)?,
            address: row.get(3)?,
            public_key: row.get(4)?,
            used: row.get::<_, i32>(5)? != 0,
            balance: row.get(6)?,
            pending_utxo_check: row.get::<_, i32>(7)? != 0,
            created_at: row.get(8)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Output baskets
    let output_baskets = {
        let mut stmt = conn.prepare(
            "SELECT basketId, user_id, name, number_of_desired_utxos, minimum_desired_utxo_value, \
             is_deleted, description, token_type, protocol_id, created_at, updated_at FROM output_baskets"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupBasket {
            basket_id: row.get(0)?,
            user_id: row.get(1)?,
            name: row.get(2)?,
            number_of_desired_utxos: row.get(3)?,
            minimum_desired_utxo_value: row.get(4)?,
            is_deleted: row.get::<_, i32>(5)? != 0,
            description: row.get(6)?,
            token_type: row.get(7)?,
            protocol_id: row.get(8)?,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Transactions
    let transactions = {
        let mut stmt = conn.prepare(
            "SELECT id, user_id, proven_tx_id, txid, reference_number, raw_tx, description, \
             status, is_outgoing, satoshis, input_beef, version, lock_time, block_height, \
             confirmations, failed_at, created_at, updated_at FROM transactions"
        )?;
        let rows = stmt.query_map([], |row| {
            let input_beef: Option<Vec<u8>> = row.get(10)?;
            Ok(BackupTransaction {
                id: row.get(0)?,
                user_id: row.get(1)?,
                proven_tx_id: row.get(2)?,
                txid: row.get(3)?,
                reference_number: row.get(4)?,
                raw_tx: row.get(5)?,
                description: row.get(6)?,
                status: row.get(7)?,
                is_outgoing: row.get::<_, i32>(8)? != 0,
                satoshis: row.get(9)?,
                input_beef: input_beef.map(|b| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &b)),
                version: row.get(11)?,
                lock_time: row.get(12)?,
                block_height: row.get(13)?,
                confirmations: row.get(14)?,
                failed_at: row.get(15)?,
                created_at: row.get(16)?,
                updated_at: row.get(17)?,
            })
        })?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Outputs
    let outputs = {
        let mut stmt = conn.prepare(
            "SELECT outputId, user_id, transaction_id, basket_id, spendable, change, vout, satoshis, \
             provided_by, purpose, type, output_description, txid, sender_identity_key, \
             derivation_prefix, derivation_suffix, custom_instructions, spent_by, sequence_number, \
             spending_description, script_length, script_offset, locking_script, created_at, updated_at FROM outputs"
        )?;
        let rows = stmt.query_map([], |row| {
            let locking_script: Option<Vec<u8>> = row.get(22)?;
            Ok(BackupOutput {
                output_id: row.get(0)?,
                user_id: row.get(1)?,
                transaction_id: row.get(2)?,
                basket_id: row.get(3)?,
                spendable: row.get::<_, i32>(4)? != 0,
                change: row.get::<_, i32>(5)? != 0,
                vout: row.get(6)?,
                satoshis: row.get(7)?,
                provided_by: row.get(8)?,
                purpose: row.get(9)?,
                output_type: row.get(10)?,
                output_description: row.get(11)?,
                txid: row.get(12)?,
                sender_identity_key: row.get(13)?,
                derivation_prefix: row.get(14)?,
                derivation_suffix: row.get(15)?,
                custom_instructions: row.get(16)?,
                spent_by: row.get(17)?,
                sequence_number: row.get(18)?,
                spending_description: row.get(19)?,
                script_length: row.get(20)?,
                script_offset: row.get(21)?,
                locking_script: locking_script.map(|b| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &b)),
                created_at: row.get(23)?,
                updated_at: row.get(24)?,
            })
        })?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Proven txs
    let proven_txs = {
        let mut stmt = conn.prepare(
            "SELECT provenTxId, txid, height, tx_index, merkle_path, raw_tx, block_hash, merkle_root, \
             created_at, updated_at FROM proven_txs"
        )?;
        let rows = stmt.query_map([], |row| {
            let merkle_path: Vec<u8> = row.get(4)?;
            let raw_tx: Vec<u8> = row.get(5)?;
            Ok(BackupProvenTx {
                proven_tx_id: row.get(0)?,
                txid: row.get(1)?,
                height: row.get(2)?,
                tx_index: row.get(3)?,
                merkle_path: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &merkle_path),
                raw_tx: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &raw_tx),
                block_hash: row.get(6)?,
                merkle_root: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Proven tx reqs
    let proven_tx_reqs = {
        let mut stmt = conn.prepare(
            "SELECT provenTxReqId, proven_tx_id, txid, status, attempts, notified, batch, history, \
             notify, raw_tx, input_beef, created_at, updated_at FROM proven_tx_reqs"
        )?;
        let rows = stmt.query_map([], |row| {
            let raw_tx: Vec<u8> = row.get(9)?;
            let input_beef: Option<Vec<u8>> = row.get(10)?;
            Ok(BackupProvenTxReq {
                proven_tx_req_id: row.get(0)?,
                proven_tx_id: row.get(1)?,
                txid: row.get(2)?,
                status: row.get(3)?,
                attempts: row.get(4)?,
                notified: row.get::<_, i32>(5)? != 0,
                batch: row.get(6)?,
                history: row.get(7)?,
                notify: row.get(8)?,
                raw_tx: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &raw_tx),
                input_beef: input_beef.map(|b| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &b)),
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Certificates
    let certificates = {
        let mut stmt = conn.prepare(
            "SELECT certificateId, user_id, type, serial_number, certifier, subject, verifier, \
             revocation_outpoint, signature, is_deleted, created_at, updated_at FROM certificates"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupCertificate {
            certificate_id: row.get(0)?,
            user_id: row.get(1)?,
            cert_type: row.get(2)?,
            serial_number: row.get(3)?,
            certifier: row.get(4)?,
            subject: row.get(5)?,
            verifier: row.get(6)?,
            revocation_outpoint: row.get(7)?,
            signature: row.get(8)?,
            is_deleted: row.get::<_, i32>(9)? != 0,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Certificate fields
    let certificate_fields = {
        let mut stmt = conn.prepare(
            "SELECT certificateId, user_id, field_name, field_value, master_key, created_at, updated_at FROM certificate_fields"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupCertificateField {
            certificate_id: row.get(0)?,
            user_id: row.get(1)?,
            field_name: row.get(2)?,
            field_value: row.get(3)?,
            master_key: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Output tags
    let output_tags = {
        let mut stmt = conn.prepare(
            "SELECT id, user_id, tag, is_deleted, created_at, updated_at FROM output_tags"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupOutputTag {
            id: row.get(0)?,
            user_id: row.get(1)?,
            tag: row.get(2)?,
            is_deleted: row.get::<_, i32>(3)? != 0,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Output tag map
    let output_tag_map = {
        let mut stmt = conn.prepare(
            "SELECT id, output_id, output_tag_id, is_deleted, created_at, updated_at FROM output_tag_map"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupOutputTagMap {
            id: row.get(0)?,
            output_id: row.get(1)?,
            output_tag_id: row.get(2)?,
            is_deleted: row.get::<_, i32>(3)? != 0,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Tx labels
    let tx_labels = {
        let mut stmt = conn.prepare(
            "SELECT txLabelId, user_id, label, is_deleted, created_at, updated_at FROM tx_labels"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupTxLabel {
            tx_label_id: row.get(0)?,
            user_id: row.get(1)?,
            label: row.get(2)?,
            is_deleted: row.get::<_, i32>(3)? != 0,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Tx labels map
    let tx_labels_map = {
        let mut stmt = conn.prepare(
            "SELECT txLabelId, transaction_id, is_deleted, created_at, updated_at FROM tx_labels_map"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupTxLabelMap {
            tx_label_id: row.get(0)?,
            transaction_id: row.get(1)?,
            is_deleted: row.get::<_, i32>(2)? != 0,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Commissions
    let commissions = {
        let mut stmt = conn.prepare(
            "SELECT commissionId, user_id, transaction_id, satoshis, key_offset, is_redeemed, \
             locking_script, created_at, updated_at FROM commissions"
        )?;
        let rows = stmt.query_map([], |row| {
            let locking_script: Vec<u8> = row.get(6)?;
            Ok(BackupCommission {
                commission_id: row.get(0)?,
                user_id: row.get(1)?,
                transaction_id: row.get(2)?,
                satoshis: row.get(3)?,
                key_offset: row.get(4)?,
                is_redeemed: row.get::<_, i32>(5)? != 0,
                locking_script: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &locking_script),
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Settings
    let settings = {
        let mut stmt = conn.prepare(
            "SELECT storage_identity_key, storage_name, chain, dbtype, max_output_script, created_at, updated_at FROM settings"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupSetting {
            storage_identity_key: row.get(0)?,
            storage_name: row.get(1)?,
            chain: row.get(2)?,
            dbtype: row.get(3)?,
            max_output_script: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Sync states
    let sync_states = {
        let mut stmt = conn.prepare(
            "SELECT syncStateId, user_id, storage_identity_key, storage_name, status, init, ref_num, \
             sync_map, sync_when, satoshis, error_local, error_other, created_at, updated_at FROM sync_states"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupSyncState {
            sync_state_id: row.get(0)?,
            user_id: row.get(1)?,
            storage_identity_key: row.get(2)?,
            storage_name: row.get(3)?,
            status: row.get(4)?,
            init: row.get::<_, i32>(5)? != 0,
            ref_num: row.get(6)?,
            sync_map: row.get(7)?,
            sync_when: row.get(8)?,
            satoshis: row.get(9)?,
            error_local: row.get(10)?,
            error_other: row.get(11)?,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Parent transactions
    let parent_transactions = {
        let mut stmt = conn.prepare(
            "SELECT id, utxo_id, txid, raw_hex, cached_at FROM parent_transactions"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupParentTransaction {
            id: row.get(0)?,
            utxo_id: row.get(1)?,
            txid: row.get(2)?,
            raw_hex: row.get(3)?,
            cached_at: row.get(4)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Block headers
    let block_headers = {
        let mut stmt = conn.prepare(
            "SELECT id, block_hash, height, header_hex, cached_at FROM block_headers"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupBlockHeader {
            id: row.get(0)?,
            block_hash: row.get(1)?,
            height: row.get(2)?,
            header_hex: row.get(3)?,
            cached_at: row.get(4)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Domain permissions (Phase 2.1)
    let domain_permissions = {
        let mut stmt = conn.prepare(
            "SELECT domain, trust_level, per_tx_limit_cents, per_session_limit_cents, \
             rate_limit_per_min, created_at, updated_at FROM domain_permissions"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupDomainPermission {
            domain: row.get(0)?,
            trust_level: row.get(1)?,
            per_tx_limit_cents: row.get(2)?,
            per_session_limit_cents: row.get(3)?,
            rate_limit_per_min: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Cert field permissions (Phase 2.1) — join to get domain name
    let cert_field_permissions = {
        let mut stmt = conn.prepare(
            "SELECT dp.domain, cfp.cert_type, cfp.field_name, cfp.created_at \
             FROM cert_field_permissions cfp \
             JOIN domain_permissions dp ON dp.id = cfp.domain_permission_id"
        )?;
        let rows = stmt.query_map([], |row| Ok(BackupCertFieldPermission {
            domain: row.get(0)?,
            cert_type: row.get(1)?,
            field_name: row.get(2)?,
            created_at: row.get(3)?,
        }))?.collect::<Result<Vec<_>>>()?;
        rows
    };

    let payload = BackupPayload {
        version: 1,
        identity_key: identity_key.to_string(),
        mnemonic: mnemonic.to_string(),
        wallet,
        users,
        addresses,
        output_baskets,
        transactions,
        outputs,
        proven_txs,
        proven_tx_reqs,
        certificates,
        certificate_fields,
        output_tags,
        output_tag_map,
        tx_labels,
        tx_labels_map,
        commissions,
        settings,
        sync_states,
        parent_transactions,
        block_headers,
        domain_permissions,
        cert_field_permissions,
    };

    info!("   Collected: {} users, {} addresses, {} txs, {} outputs, {} certs",
          payload.users.len(), payload.addresses.len(),
          payload.transactions.len(), payload.outputs.len(),
          payload.certificates.len());

    Ok(payload)
}

// ============================================================================
// encrypt_backup / decrypt_backup — AES-256-GCM with PBKDF2 key derivation
// ============================================================================

/// Encrypt a BackupPayload into an EncryptedBackup file.
pub fn encrypt_backup(payload: &BackupPayload, password: &str) -> std::result::Result<EncryptedBackup, String> {
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::aead::generic_array::GenericArray;
    use rand::RngCore;
    use std::time::{SystemTime, UNIX_EPOCH};

    let json = serde_json::to_string(payload)
        .map_err(|e| format!("Failed to serialize payload: {}", e))?;

    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);

    let key = crate::crypto::pin::derive_key_from_pin(password, &salt);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&key));

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, json.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Combined: nonce(12) || ciphertext+tag
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    Ok(EncryptedBackup {
        format: "hodos-wallet-backup".to_string(),
        version: 1,
        created_at,
        salt: hex::encode(salt),
        data: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &combined),
    })
}

/// Decrypt an EncryptedBackup file into a BackupPayload.
pub fn decrypt_backup(backup: &EncryptedBackup, password: &str) -> std::result::Result<BackupPayload, String> {
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::aead::generic_array::GenericArray;

    if backup.format != "hodos-wallet-backup" {
        return Err("Invalid backup format".to_string());
    }

    let salt = hex::decode(&backup.salt)
        .map_err(|e| format!("Invalid salt hex: {}", e))?;
    let combined = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &backup.data)
        .map_err(|e| format!("Invalid base64 data: {}", e))?;

    if combined.len() < 12 + 17 {
        return Err("Encrypted data too short".to_string());
    }

    let key = crate::crypto::pin::derive_key_from_pin(password, &salt);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&key));

    let nonce = GenericArray::from_slice(&combined[..12]);
    let ciphertext_with_tag = &combined[12..];

    let plaintext = cipher.decrypt(nonce, ciphertext_with_tag)
        .map_err(|_| "Invalid password".to_string())?;

    let json_str = String::from_utf8(plaintext)
        .map_err(|e| format!("Invalid UTF-8 in decrypted data: {}", e))?;

    serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse backup JSON: {}", e))
}

// ============================================================================
// import_to_db — write all backup entities into a fresh database
// ============================================================================

/// Import all entities from a BackupPayload into the database.
/// Caller must have already created the wallet record (with mnemonic+PIN).
/// This function handles all other entities in FK dependency order.
pub fn import_to_db(conn: &Connection, payload: &BackupPayload) -> std::result::Result<(), String> {
    info!("   Importing backup entities into database...");

    conn.execute("BEGIN TRANSACTION", [])
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let result = import_entities(conn, payload);

    match result {
        Ok(()) => {
            conn.execute("COMMIT", [])
                .map_err(|e| format!("Failed to commit: {}", e))?;
            info!("   Import committed successfully");
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
}

fn import_entities(conn: &Connection, payload: &BackupPayload) -> std::result::Result<(), String> {
    // 1. proven_txs (no FKs)
    for pt in &payload.proven_txs {
        let merkle_path = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &pt.merkle_path)
            .map_err(|e| format!("proven_txs merkle_path base64: {}", e))?;
        let raw_tx = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &pt.raw_tx)
            .map_err(|e| format!("proven_txs raw_tx base64: {}", e))?;
        conn.execute(
            "INSERT INTO proven_txs (provenTxId, txid, height, tx_index, merkle_path, raw_tx, block_hash, merkle_root, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![pt.proven_tx_id, pt.txid, pt.height, pt.tx_index, merkle_path, raw_tx,
                             pt.block_hash, pt.merkle_root, pt.created_at, pt.updated_at],
        ).map_err(|e| format!("Insert proven_txs: {}", e))?;
    }

    // 2. users (no FKs)
    for u in &payload.users {
        conn.execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![u.user_id, u.identity_key, u.active_storage, u.created_at, u.updated_at],
        ).map_err(|e| format!("Insert users: {}", e))?;
    }

    // 3. output_baskets (FK → users)
    for b in &payload.output_baskets {
        conn.execute(
            "INSERT INTO output_baskets (basketId, user_id, name, number_of_desired_utxos, minimum_desired_utxo_value, \
             is_deleted, description, token_type, protocol_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![b.basket_id, b.user_id, b.name, b.number_of_desired_utxos,
                             b.minimum_desired_utxo_value, b.is_deleted as i32, b.description,
                             b.token_type, b.protocol_id, b.created_at, b.updated_at],
        ).map_err(|e| format!("Insert output_baskets: {}", e))?;
    }

    // 4. transactions (FK → users, proven_txs)
    for tx in &payload.transactions {
        let input_beef = match &tx.input_beef {
            Some(b64) => Some(base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
                .map_err(|e| format!("transactions input_beef base64: {}", e))?),
            None => None,
        };
        conn.execute(
            "INSERT INTO transactions (id, user_id, proven_tx_id, txid, reference_number, raw_tx, description, \
             status, is_outgoing, satoshis, input_beef, version, lock_time, block_height, confirmations, \
             failed_at, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            rusqlite::params![tx.id, tx.user_id, tx.proven_tx_id, tx.txid, tx.reference_number,
                             tx.raw_tx, tx.description, tx.status, tx.is_outgoing as i32, tx.satoshis,
                             input_beef, tx.version, tx.lock_time, tx.block_height, tx.confirmations,
                             tx.failed_at, tx.created_at, tx.updated_at],
        ).map_err(|e| format!("Insert transactions: {}", e))?;
    }

    // 5. proven_tx_reqs (FK → proven_txs)
    for pr in &payload.proven_tx_reqs {
        let raw_tx = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &pr.raw_tx)
            .map_err(|e| format!("proven_tx_reqs raw_tx base64: {}", e))?;
        let input_beef = match &pr.input_beef {
            Some(b64) => Some(base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
                .map_err(|e| format!("proven_tx_reqs input_beef base64: {}", e))?),
            None => None,
        };
        conn.execute(
            "INSERT INTO proven_tx_reqs (provenTxReqId, proven_tx_id, txid, status, attempts, notified, batch, \
             history, notify, raw_tx, input_beef, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![pr.proven_tx_req_id, pr.proven_tx_id, pr.txid, pr.status, pr.attempts,
                             pr.notified as i32, pr.batch, pr.history, pr.notify, raw_tx, input_beef,
                             pr.created_at, pr.updated_at],
        ).map_err(|e| format!("Insert proven_tx_reqs: {}", e))?;
    }

    // 6. outputs (FK → users, transactions, output_baskets, spent_by → transactions)
    for o in &payload.outputs {
        let locking_script = match &o.locking_script {
            Some(b64) => Some(base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
                .map_err(|e| format!("outputs locking_script base64: {}", e))?),
            None => None,
        };
        conn.execute(
            "INSERT INTO outputs (outputId, user_id, transaction_id, basket_id, spendable, change, vout, satoshis, \
             provided_by, purpose, type, output_description, txid, sender_identity_key, derivation_prefix, \
             derivation_suffix, custom_instructions, spent_by, sequence_number, spending_description, \
             script_length, script_offset, locking_script, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
            rusqlite::params![o.output_id, o.user_id, o.transaction_id, o.basket_id,
                             o.spendable as i32, o.change as i32, o.vout, o.satoshis,
                             o.provided_by, o.purpose, o.output_type, o.output_description,
                             o.txid, o.sender_identity_key, o.derivation_prefix, o.derivation_suffix,
                             o.custom_instructions, o.spent_by, o.sequence_number, o.spending_description,
                             o.script_length, o.script_offset, locking_script, o.created_at, o.updated_at],
        ).map_err(|e| format!("Insert outputs: {}", e))?;
    }

    // 7. certificates (FK → users)
    for c in &payload.certificates {
        conn.execute(
            "INSERT INTO certificates (certificateId, user_id, type, serial_number, certifier, subject, verifier, \
             revocation_outpoint, signature, is_deleted, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![c.certificate_id, c.user_id, c.cert_type, c.serial_number, c.certifier,
                             c.subject, c.verifier, c.revocation_outpoint, c.signature,
                             c.is_deleted as i32, c.created_at, c.updated_at],
        ).map_err(|e| format!("Insert certificates: {}", e))?;
    }

    // 8. certificate_fields (FK → certificates, users)
    for cf in &payload.certificate_fields {
        conn.execute(
            "INSERT INTO certificate_fields (certificateId, user_id, field_name, field_value, master_key, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![cf.certificate_id, cf.user_id, cf.field_name, cf.field_value,
                             cf.master_key, cf.created_at, cf.updated_at],
        ).map_err(|e| format!("Insert certificate_fields: {}", e))?;
    }

    // 9. output_tags (FK → users)
    for ot in &payload.output_tags {
        conn.execute(
            "INSERT INTO output_tags (id, user_id, tag, is_deleted, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![ot.id, ot.user_id, ot.tag, ot.is_deleted as i32, ot.created_at, ot.updated_at],
        ).map_err(|e| format!("Insert output_tags: {}", e))?;
    }

    // 10. output_tag_map (FK → outputs, output_tags)
    for otm in &payload.output_tag_map {
        conn.execute(
            "INSERT INTO output_tag_map (id, output_id, output_tag_id, is_deleted, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![otm.id, otm.output_id, otm.output_tag_id, otm.is_deleted as i32,
                             otm.created_at, otm.updated_at],
        ).map_err(|e| format!("Insert output_tag_map: {}", e))?;
    }

    // 11. tx_labels (FK → users)
    for tl in &payload.tx_labels {
        conn.execute(
            "INSERT INTO tx_labels (txLabelId, user_id, label, is_deleted, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![tl.tx_label_id, tl.user_id, tl.label, tl.is_deleted as i32,
                             tl.created_at, tl.updated_at],
        ).map_err(|e| format!("Insert tx_labels: {}", e))?;
    }

    // 12. tx_labels_map (FK → tx_labels, transactions)
    for tlm in &payload.tx_labels_map {
        conn.execute(
            "INSERT INTO tx_labels_map (txLabelId, transaction_id, is_deleted, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![tlm.tx_label_id, tlm.transaction_id, tlm.is_deleted as i32,
                             tlm.created_at, tlm.updated_at],
        ).map_err(|e| format!("Insert tx_labels_map: {}", e))?;
    }

    // 13. commissions (FK → users, transactions)
    for cm in &payload.commissions {
        let locking_script = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &cm.locking_script)
            .map_err(|e| format!("commissions locking_script base64: {}", e))?;
        conn.execute(
            "INSERT INTO commissions (commissionId, user_id, transaction_id, satoshis, key_offset, \
             is_redeemed, locking_script, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![cm.commission_id, cm.user_id, cm.transaction_id, cm.satoshis,
                             cm.key_offset, cm.is_redeemed as i32, locking_script,
                             cm.created_at, cm.updated_at],
        ).map_err(|e| format!("Insert commissions: {}", e))?;
    }

    // 14. sync_states (FK → users)
    for ss in &payload.sync_states {
        conn.execute(
            "INSERT INTO sync_states (syncStateId, user_id, storage_identity_key, storage_name, status, init, \
             ref_num, sync_map, sync_when, satoshis, error_local, error_other, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            rusqlite::params![ss.sync_state_id, ss.user_id, ss.storage_identity_key, ss.storage_name,
                             ss.status, ss.init as i32, ss.ref_num, ss.sync_map, ss.sync_when,
                             ss.satoshis, ss.error_local, ss.error_other, ss.created_at, ss.updated_at],
        ).map_err(|e| format!("Insert sync_states: {}", e))?;
    }

    // 15. domain_permissions (FK → users)
    for dp in &payload.domain_permissions {
        // Find user_id — default to 1 (single-user wallet)
        let user_id: i64 = payload.users.first().map(|u| u.user_id).unwrap_or(1);
        conn.execute(
            "INSERT OR IGNORE INTO domain_permissions
             (user_id, domain, trust_level, per_tx_limit_cents, per_session_limit_cents,
              rate_limit_per_min, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![user_id, dp.domain, dp.trust_level, dp.per_tx_limit_cents,
                             dp.per_session_limit_cents, dp.rate_limit_per_min,
                             dp.created_at, dp.updated_at],
        ).map_err(|e| format!("Insert domain_permissions: {}", e))?;
    }

    // 16. cert_field_permissions (FK → domain_permissions via domain lookup)
    for cfp in &payload.cert_field_permissions {
        // Look up domain_permission_id by domain name
        let dp_id: Option<i64> = conn.query_row(
            "SELECT id FROM domain_permissions WHERE domain = ?1 LIMIT 1",
            rusqlite::params![cfp.domain],
            |row| row.get(0),
        ).ok();
        if let Some(dp_id) = dp_id {
            conn.execute(
                "INSERT OR IGNORE INTO cert_field_permissions
                 (domain_permission_id, cert_type, field_name, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![dp_id, cfp.cert_type, cfp.field_name, cfp.created_at],
            ).map_err(|e| format!("Insert cert_field_permissions: {}", e))?;
        }
    }

    // 17. settings (no FKs)
    for s in &payload.settings {
        conn.execute(
            "INSERT INTO settings (storage_identity_key, storage_name, chain, dbtype, max_output_script, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![s.storage_identity_key, s.storage_name, s.chain, s.dbtype,
                             s.max_output_script, s.created_at, s.updated_at],
        ).map_err(|e| format!("Insert settings: {}", e))?;
    }

    // 18. addresses (FK → wallets — wallet was created by caller)
    for a in &payload.addresses {
        conn.execute(
            "INSERT OR IGNORE INTO addresses (id, wallet_id, \"index\", address, public_key, used, balance, pending_utxo_check, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![a.id, a.wallet_id, a.index, a.address, a.public_key,
                             a.used as i32, a.balance, a.pending_utxo_check as i32, a.created_at],
        ).map_err(|e| format!("Insert addresses: {}", e))?;
    }

    // 19. parent_transactions (no FKs)
    for pt in &payload.parent_transactions {
        conn.execute(
            "INSERT INTO parent_transactions (id, utxo_id, txid, raw_hex, cached_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![pt.id, pt.utxo_id, pt.txid, pt.raw_hex, pt.cached_at],
        ).map_err(|e| format!("Insert parent_transactions: {}", e))?;
    }

    // 20. block_headers (no FKs)
    for bh in &payload.block_headers {
        conn.execute(
            "INSERT INTO block_headers (id, block_hash, height, header_hex, cached_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![bh.id, bh.block_hash, bh.height, bh.header_hex, bh.cached_at],
        ).map_err(|e| format!("Insert block_headers: {}", e))?;
    }

    info!("   Imported: {} users, {} addresses, {} txs, {} outputs, {} certs, {} proven_txs",
          payload.users.len(), payload.addresses.len(), payload.transactions.len(),
          payload.outputs.len(), payload.certificates.len(), payload.proven_txs.len());

    Ok(())
}

// ============================================================================
// Legacy file-based backup/restore (unchanged)
// ============================================================================

/// Backup the database using file copy
pub fn backup_database_file(source_path: &Path, dest_path: &Path) -> Result<()> {
    info!("Starting database backup...");
    info!("   Source: {}", source_path.display());
    info!("   Destination: {}", dest_path.display());

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to create backup directory: {}", e))
            ))?;
    }

    fs::copy(source_path, dest_path)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
            Some(format!("Failed to copy database file: {}", e))
        ))?;

    info!("   Copied database file");

    let wal_path = source_path.with_extension("db-wal");
    if wal_path.exists() {
        let dest_wal = dest_path.with_extension("db-wal");
        fs::copy(&wal_path, &dest_wal)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to copy WAL file: {}", e))
            ))?;
        info!("   Copied WAL file");
    }

    let shm_path = source_path.with_extension("db-shm");
    if shm_path.exists() {
        let dest_shm = dest_path.with_extension("db-shm");
        fs::copy(&shm_path, &dest_shm)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to copy SHM file: {}", e))
            ))?;
        info!("   Copied SHM file");
    }

    let metadata = fs::metadata(dest_path)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
            Some(format!("Failed to get backup file metadata: {}", e))
        ))?;
    let size_bytes = metadata.len();

    info!("   Backup complete! Size: {} bytes", size_bytes);
    Ok(())
}

/// Restore database from backup
pub fn restore_database(backup_path: &Path, dest_path: &Path) -> Result<()> {
    info!("Starting database restore...");
    info!("   Backup: {}", backup_path.display());
    info!("   Destination: {}", dest_path.display());

    if !backup_path.exists() {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
            Some(format!("Backup file not found: {}", backup_path.display()))
        ));
    }

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to create restore directory: {}", e))
            ))?;
    }

    fs::copy(backup_path, dest_path)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
            Some(format!("Failed to restore database file: {}", e))
        ))?;

    info!("   Restored database file");

    let backup_wal = backup_path.with_extension("db-wal");
    if backup_wal.exists() {
        let dest_wal = dest_path.with_extension("db-wal");
        fs::copy(&backup_wal, &dest_wal)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to restore WAL file: {}", e))
            ))?;
        info!("   Restored WAL file");
    }

    let backup_shm = backup_path.with_extension("db-shm");
    if backup_shm.exists() {
        let dest_shm = dest_path.with_extension("db-shm");
        fs::copy(&backup_shm, &dest_shm)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to restore SHM file: {}", e))
            ))?;
        info!("   Restored SHM file");
    }

    info!("   Restore complete!");
    Ok(())
}

/// Verify backup file integrity
pub fn verify_backup(backup_path: &Path) -> Result<bool> {
    info!("Verifying backup: {}", backup_path.display());

    if !backup_path.exists() {
        return Ok(false);
    }

    match rusqlite::Connection::open(backup_path) {
        Ok(conn) => {
            match conn.query_row("SELECT 1", [], |_row| Ok(())) {
                Ok(_) => {
                    info!("   Backup is valid");
                    Ok(true)
                }
                Err(e) => {
                    log::warn!("   Backup file exists but is corrupted: {}", e);
                    Ok(false)
                }
            }
        }
        Err(e) => {
            log::warn!("   Backup file is not a valid SQLite database: {}", e);
            Ok(false)
        }
    }
}

/// Export non-sensitive wallet data to JSON (debugging/accounting)
pub fn export_to_json(db: &WalletDatabase, dest_path: &Path) -> Result<()> {
    use crate::database::{AddressRepository, TransactionRepository, OutputRepository, Address};
    use std::time::{SystemTime, UNIX_EPOCH};

    info!("Exporting wallet data to JSON...");
    info!("   Destination: {}", dest_path.display());

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to create export directory: {}", e))
            ))?;
    }

    use crate::database::WalletRepository;

    let conn = db.connection();
    let wallet_repo = WalletRepository::new(conn);
    let address_repo = AddressRepository::new(conn);
    let transaction_repo = TransactionRepository::new(conn);
    let output_repo = OutputRepository::new(conn);

    let wallet = wallet_repo.get_primary_wallet()
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
            Some(format!("Failed to get wallet: {}", e))
        ))?
        .ok_or_else(|| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
            Some("No wallet found in database".to_string())
        ))?;

    let wallet_id = wallet.id.ok_or_else(|| rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
        Some("Wallet has no ID".to_string())
    ))?;

    #[derive(serde::Serialize)]
    struct ExportData {
        export_timestamp: i64,
        addresses: Vec<AddressExport>,
        transactions: Vec<TransactionExport>,
        utxos: Vec<UtxoExport>,
    }

    #[derive(serde::Serialize)]
    struct AddressExport {
        index: i32,
        address: String,
        public_key: String,
    }

    #[derive(serde::Serialize)]
    struct TransactionExport {
        txid: String,
        reference_number: Option<String>,
        label: Option<String>,
        amount: i64,
        created_at: i64,
    }

    #[derive(serde::Serialize)]
    struct UtxoExport {
        txid: String,
        vout: i32,
        address: String,
        amount: i64,
        is_spent: bool,
    }

    let addresses = address_repo.get_all_by_wallet(wallet_id)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
            Some(format!("Failed to get addresses: {}", e))
        ))?;

    let address_exports: Vec<AddressExport> = addresses.iter()
        .map(|addr| AddressExport {
            index: addr.index,
            address: addr.address.clone(),
            public_key: addr.public_key.clone(),
        })
        .collect();

    let transactions = transaction_repo.list_transactions(None, None)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
            Some(format!("Failed to get transactions: {}", e))
        ))?;

    let transaction_exports: Vec<TransactionExport> = transactions.iter()
        .map(|tx| TransactionExport {
            txid: tx.txid.clone(),
            reference_number: Some(tx.reference_number.clone()),
            label: tx.labels.first().cloned(),
            amount: tx.satoshis,
            created_at: tx.timestamp,
        })
        .collect();

    const DEFAULT_USER_ID: i64 = 1;
    let outputs = output_repo.get_all_by_user(DEFAULT_USER_ID)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
            Some(format!("Failed to get outputs: {}", e))
        ))?;

    let address_map: std::collections::HashMap<String, &Address> = addresses.iter()
        .map(|addr| (addr.index.to_string(), addr))
        .collect();

    let utxo_exports: Vec<UtxoExport> = outputs.iter()
        .filter_map(|output| {
            let txid = output.txid.as_ref()?;
            let address_str = output.derivation_suffix.as_ref()
                .and_then(|suffix| address_map.get(suffix))
                .map(|addr| addr.address.clone())
                .unwrap_or_else(|| "unknown".to_string());
            Some(UtxoExport {
                txid: txid.clone(),
                vout: output.vout,
                address: address_str,
                amount: output.satoshis,
                is_spent: !output.spendable,
            })
        })
        .collect();

    let export_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let export_data = ExportData {
        export_timestamp,
        addresses: address_exports,
        transactions: transaction_exports,
        utxos: utxo_exports,
    };

    let json = serde_json::to_string_pretty(&export_data)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Failed to serialize export data: {}", e))
        ))?;

    fs::write(dest_path, json)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
            Some(format!("Failed to write export file: {}", e))
        ))?;

    info!("   Export complete!");
    info!("   Exported: {} addresses, {} transactions, {} UTXOs",
          export_data.addresses.len(),
          export_data.transactions.len(),
          export_data.utxos.len());

    Ok(())
}
