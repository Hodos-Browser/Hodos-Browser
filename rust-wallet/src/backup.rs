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

    // Transactions (exclude backup txs and failed txs — both are dead weight in backup)
    let transactions = {
        let mut stmt = conn.prepare(
            "SELECT id, user_id, proven_tx_id, txid, reference_number, raw_tx, description, \
             status, is_outgoing, satoshis, input_beef, version, lock_time, block_height, \
             confirmations, failed_at, created_at, updated_at FROM transactions \
             WHERE reference_number NOT LIKE 'backup-%' AND status != 'failed'"
        )?;
        let rows = stmt.query_map([], |row| {
            let proven_tx_id: Option<i64> = row.get(2)?;
            // Strip raw_tx for confirmed transactions (has proven_tx_id) — re-fetchable from chain
            let raw_tx_blob: Option<Vec<u8>> = if proven_tx_id.is_some() {
                None // Confirmed — raw_tx on-chain, free to re-fetch
            } else {
                row.get::<_, Option<Vec<u8>>>(5)
                    .or_else(|_| {
                        row.get::<_, Option<String>>(5).map(|opt| {
                            opt.and_then(|s| hex::decode(&s).ok())
                        })
                    })?
            };
            let input_beef: Option<Vec<u8>> = row.get(10)?;
            Ok(BackupTransaction {
                id: row.get(0)?,
                user_id: row.get(1)?,
                proven_tx_id,
                txid: row.get(3)?,
                reference_number: row.get(4)?,
                raw_tx: raw_tx_blob.map(|b| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &b)),
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

    // Outputs: exclude backup PushDrop/marker outputs AND outputs spent by backup transactions.
    // Outputs spent by backup txs have spendable=0 and spent_by pointing to a backup tx.
    // Including them with spent_by=NULL (FK cleanup) causes TaskReviewStatus to incorrectly
    // restore them to spendable, inflating the balance.
    let outputs = {
        let mut stmt = conn.prepare(
            "SELECT outputId, user_id, transaction_id, basket_id, spendable, change, vout, satoshis, \
             provided_by, purpose, type, output_description, txid, sender_identity_key, \
             derivation_prefix, derivation_suffix, custom_instructions, spent_by, sequence_number, \
             spending_description, script_length, script_offset, locking_script, created_at, updated_at FROM outputs \
             WHERE COALESCE(derivation_prefix, '') != '1-wallet-backup' \
             AND (spent_by IS NULL OR NOT EXISTS (SELECT 1 FROM transactions t WHERE t.id = outputs.spent_by AND t.reference_number LIKE 'backup-%')) \
             AND (transaction_id IS NULL OR NOT EXISTS (SELECT 1 FROM transactions t WHERE t.id = outputs.transaction_id AND t.status = 'failed'))"
        )?;
        let rows = stmt.query_map([], |row| {
            let spendable = row.get::<_, i32>(4)? != 0;
            // Strip locking_script from spent outputs — never needed again
            let locking_script: Option<Vec<u8>> = if spendable {
                row.get(22)?
            } else {
                None
            };
            Ok(BackupOutput {
                output_id: row.get(0)?,
                user_id: row.get(1)?,
                transaction_id: row.get(2)?,
                basket_id: row.get(3)?,
                spendable,
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

    // Proven txs (exclude backup txs; raw_tx and merkle_path stripped in serialize_for_onchain)
    let proven_txs = {
        let mut stmt = conn.prepare(
            "SELECT provenTxId, txid, height, tx_index, merkle_path, raw_tx, block_hash, merkle_root, \
             created_at, updated_at FROM proven_txs \
             WHERE NOT EXISTS (SELECT 1 FROM transactions t WHERE t.txid = proven_txs.txid AND t.reference_number LIKE 'backup-%')"
        )?;
        let rows = stmt.query_map([], |row| {
            let merkle_path: Vec<u8> = row.get(4)?;
            // Strip raw_tx — confirmed tx bytes are on-chain permanently, free to re-fetch
            Ok(BackupProvenTx {
                proven_tx_id: row.get(0)?,
                txid: row.get(1)?,
                height: row.get(2)?,
                tx_index: row.get(3)?,
                merkle_path: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &merkle_path),
                raw_tx: String::new(), // Stripped — re-fetch from WhatsOnChain on recovery
                block_hash: row.get(6)?,
                merkle_root: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?.collect::<Result<Vec<_>>>()?;
        rows
    };

    // Proven tx reqs (exclude backup txs; raw_tx and input_beef stripped in serialize_for_onchain)
    let proven_tx_reqs = {
        let mut stmt = conn.prepare(
            "SELECT provenTxReqId, proven_tx_id, txid, status, attempts, notified, batch, history, \
             notify, raw_tx, input_beef, created_at, updated_at FROM proven_tx_reqs \
             WHERE NOT EXISTS (SELECT 1 FROM transactions t WHERE t.txid = proven_tx_reqs.txid AND t.reference_number LIKE 'backup-%')"
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

    // Parent transactions (entire table cleared in serialize_for_onchain — BEEF cache rebuilt on re-fetch)
    let parent_transactions = {
        let mut stmt = conn.prepare(
            "SELECT pt.id, pt.utxo_id, pt.txid, pt.raw_hex, pt.cached_at FROM parent_transactions pt \
             WHERE NOT EXISTS (SELECT 1 FROM transactions t WHERE t.txid = pt.txid AND t.reference_number LIKE 'backup-%')"
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
// On-chain backup — serialize + compress + encrypt / decrypt + decompress
// ============================================================================

/// Derive the 32-byte AES key for on-chain backup encryption.
/// Uses SHA-256(master_privkey || "hodos-wallet-backup-v1") — simple and secure
/// since the input is already a 256-bit secret.
fn derive_onchain_backup_key(master_privkey: &[u8]) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(master_privkey);
    hasher.update(b"hodos-wallet-backup-v1");
    hasher.finalize().into()
}

/// Serialize the current wallet state into encrypted, compressed bytes for on-chain storage.
///
/// Pipeline: BackupPayload → JSON → gzip(level 9) → AES-256-GCM encrypt
///
/// The mnemonic is excluded from the payload (set to empty string) because the
/// recovery user already has it — they need it to derive the backup address.
///
/// Output format: nonce(12) || ciphertext || tag(16)
pub fn serialize_for_onchain(
    conn: &Connection,
    identity_key: &str,
    master_privkey: &[u8],
) -> std::result::Result<Vec<u8>, String> {
    // Collect, strip, and compress
    let compressed = compress_for_onchain(conn, identity_key)?;

    // Encrypt
    let encrypted = encrypt_compressed(master_privkey, &compressed)?;

    info!("   On-chain backup: {} bytes encrypted (total with nonce)", encrypted.len());

    Ok(encrypted)
}

/// Collect, strip, and compress the wallet payload (without encrypting).
/// Returns the compressed bytes suitable for hashing to detect changes.
pub fn compress_for_onchain(
    conn: &Connection,
    identity_key: &str,
) -> std::result::Result<Vec<u8>, String> {
    use std::io::Write;

    // 1. Collect payload (mnemonic excluded)
    let mut payload = collect_payload(conn, identity_key, "")
        .map_err(|e| format!("Failed to collect backup payload: {}", e))?;
    payload.mnemonic = String::new();

    // 2. Strip re-fetchable data (same as serialize_for_onchain)
    payload.parent_transactions.clear();
    payload.block_headers.clear(); // fetched on demand via cache_helpers.rs
    for req in &mut payload.proven_tx_reqs {
        req.raw_tx = String::new();
        req.input_beef = None;
        // Cap history to last 5 entries. The history field is an unbounded JSON
        // audit log appended on every status transition — it grows monotonically
        // and dominates the proven_tx_reqs payload on active wallets.
        if !req.history.is_empty() && req.history != "{}" {
            if let Ok(history) = serde_json::from_str::<serde_json::Value>(&req.history) {
                if let Some(obj) = history.as_object() {
                    if obj.len() > 5 {
                        // Keys are unix timestamps — sort descending, keep last 5
                        let mut keys: Vec<&String> = obj.keys().collect();
                        keys.sort_unstable_by(|a, b| b.cmp(a));
                        let trimmed: serde_json::Map<String, serde_json::Value> = keys.into_iter()
                            .take(5)
                            .map(|k| (k.clone(), obj[k].clone()))
                            .collect();
                        if let Ok(s) = serde_json::to_string(&serde_json::Value::Object(trimmed)) {
                            req.history = s;
                        }
                    }
                }
            }
        }
    }
    for ptx in &mut payload.proven_txs {
        ptx.merkle_path = String::new();
    }

    // Spent-output time-tiered strip: drop spent HD-self outputs older than 7 days
    // whose owning address is no longer flagged for pending UTXO check.
    //
    // HARD RULE — non-standard token preservation: any output not recoverable from
    // the master key alone (BRC-42 counterparty, PushDrop, future BRC-X) MUST be
    // preserved regardless of age. Token loss is permanent and unrecoverable.
    //
    // PENDING-ADDRESS GUARD: even a spent HD-self output is preserved if its
    // owning address (matched by derivation_suffix → addresses.index) still has
    // pending_utxo_check=1, because the wallet considers that address live for
    // future receives. The "advanced wallet receive" UI may display such an
    // address as the user's current receive address.
    //
    // Six inclusion clauses; drop only if NONE match.
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        const SPENT_OUTPUT_BACKUP_RETENTION_SECS: i64 = 7 * 24 * 60 * 60;
        let now: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Build set of HD indices whose owning address is still pending UTXO check.
        // payload.addresses was loaded by collect_payload above and reflects the live
        // addresses table at the moment of backup.
        let pending_indices: std::collections::HashSet<i32> = payload.addresses.iter()
            .filter(|a| a.pending_utxo_check)
            .map(|a| a.index)
            .collect();

        let before = payload.outputs.len();
        payload.outputs.retain(|o| {
            // 1. Active UTXOs — always keep
            if o.spendable { return true; }
            // 2. BRC-42 counterparty (PeerPay etc.) — non-recoverable from master key
            if o.sender_identity_key.is_some() { return true; }
            // 3. Non-HD-self derivation prefixes (PushDrop, tokens, master-direct, future BRC-X)
            match o.derivation_prefix.as_deref() {
                Some("2-receive address") | Some("bip32") => {} // candidate for drop
                _ => return true, // preserve everything else (including None)
            }
            // 4. Recent records — keep for in-flight cert reclaim / replay / status reconciliation
            if now.saturating_sub(o.updated_at) < SPENT_OUTPUT_BACKUP_RETENTION_SECS {
                return true;
            }
            // 5. Pending-address guard — owning address still being watched for new UTXOs.
            //    Match output's derivation_suffix (HD index as string) against pending set.
            if let Some(suffix) = o.derivation_suffix.as_deref() {
                if let Ok(idx) = suffix.parse::<i32>() {
                    if pending_indices.contains(&idx) { return true; }
                }
            }
            // Spent + HD-self + older than 7 days + owning address not pending → drop
            false
        });
        let dropped = before - payload.outputs.len();
        if dropped > 0 {
            info!("   🧹 Backup spent-output strip: dropped {} of {} outputs (spent + HD-self + >7d + addr not pending)",
                dropped, before);
        }
    }

    // Address time-tiered strip: drop "operationally dead" addresses from the backup.
    // An address is operationally dead if ALL of the following are true:
    //   - used = true (it's been used at some point)
    //   - has zero spendable outputs (all UTXOs spent or never received)
    //   - pending_utxo_check = false (not flagged for active sync)
    //   - older than 30 days (by created_at)
    //   - index >= 0 (NOT master pubkey at -1, NOT external placeholder at -2)
    //
    // Recovery imports addresses from backup directly (handlers.rs:12443 deletes
    // auto-created then imports from payload). Kept addresses are imported normally.
    // Dropped addresses have no funds and no in-flight operations.
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        const ADDRESS_BACKUP_RETENTION_SECS: i64 = 30 * 24 * 60 * 60;
        let now: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Build set of address indices that have spendable outputs
        let active_indices: std::collections::HashSet<i32> = payload.outputs.iter()
            .filter(|o| o.spendable)
            .filter_map(|o| {
                if o.derivation_prefix.as_deref() == Some("2-receive address") {
                    o.derivation_suffix.as_deref().and_then(|s| s.parse::<i32>().ok())
                } else if o.derivation_prefix.as_deref() == Some("bip32") {
                    o.derivation_suffix.as_deref().and_then(|s| s.parse::<i32>().ok())
                } else { None }
            })
            .collect();

        let before = payload.addresses.len();
        payload.addresses.retain(|a| {
            // ALWAYS keep special indices (master pubkey, external placeholder)
            if a.index < 0 { return true; }
            // ALWAYS keep unused addresses (might receive future payments)
            if !a.used { return true; }
            // ALWAYS keep addresses pending UTXO check
            if a.pending_utxo_check { return true; }
            // ALWAYS keep addresses with active spendable UTXOs
            if active_indices.contains(&a.index) { return true; }
            // ALWAYS keep recent addresses (within retention window)
            if now.saturating_sub(a.created_at) < ADDRESS_BACKUP_RETENTION_SECS { return true; }
            // Operationally dead — drop from backup
            false
        });
        let dropped = before - payload.addresses.len();
        if dropped > 0 {
            info!("   🧹 Backup address strip: dropped {} of {} addresses (used + no spendable + not pending + >30d)",
                dropped, before);
        }
    }

    // Transaction sliding window: drop confirmed transactions older than 60 days.
    // Recovery gets current state + 60 days of history. Older history can be
    // re-fetched from chain in the background if the user wants it.
    //
    // Keep rules:
    //   - Non-completed transactions (sending, unproven, failed, etc.) — always keep
    //   - Transactions with spendable outputs still referencing them — always keep
    //   - Completed transactions within 60 days — keep
    //   - Completed transactions older than 60 days — drop
    //
    // The orphan FK cleanup below handles any output.transaction_id or output.spent_by
    // references that pointed to dropped transactions. proven_txs records are kept
    // as-is (unreferenced but harmless — they're immutable proof records).
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        const TX_BACKUP_RETENTION_SECS: i64 = 60 * 24 * 60 * 60; // 60 days
        let now: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Build set of transaction IDs that have spendable outputs
        let tx_ids_with_spendable: std::collections::HashSet<i64> = payload.outputs.iter()
            .filter(|o| o.spendable)
            .filter_map(|o| o.transaction_id)
            .collect();

        let before = payload.transactions.len();
        payload.transactions.retain(|tx| {
            // ALWAYS keep non-completed transactions (in-flight, failed, etc.)
            if tx.status != "completed" { return true; }
            // ALWAYS keep transactions with active spendable outputs
            if tx_ids_with_spendable.contains(&tx.id) { return true; }
            // ALWAYS keep recent completed transactions
            if now.saturating_sub(tx.updated_at) < TX_BACKUP_RETENTION_SECS { return true; }
            // Old completed transaction with no active outputs — drop
            false
        });
        let dropped = before - payload.transactions.len();
        if dropped > 0 {
            info!("   🧹 Backup transaction strip: dropped {} of {} transactions (completed + no spendable + >60d)",
                dropped, before);
        }

        // Also drop proven_tx_reqs whose txid no longer has a matching transaction
        let kept_txids: std::collections::HashSet<&str> = payload.transactions.iter()
            .filter_map(|t| t.txid.as_deref())
            .collect();
        let req_before = payload.proven_tx_reqs.len();
        payload.proven_tx_reqs.retain(|r| kept_txids.contains(r.txid.as_str()));
        let req_dropped = req_before - payload.proven_tx_reqs.len();
        if req_dropped > 0 {
            info!("   🧹 Backup proven_tx_reqs strip: dropped {} orphaned reqs", req_dropped);
        }
    }

    // Null out orphan FK references
    let valid_tx_ids: std::collections::HashSet<i64> = payload.transactions.iter()
        .map(|t| t.id)
        .collect();
    let valid_basket_ids: std::collections::HashSet<i64> = payload.output_baskets.iter()
        .filter_map(|b| Some(b.basket_id))
        .collect();
    for output in &mut payload.outputs {
        if let Some(tx_id) = output.transaction_id {
            if !valid_tx_ids.contains(&tx_id) { output.transaction_id = None; }
        }
        if let Some(spent_by) = output.spent_by {
            if !valid_tx_ids.contains(&spent_by) { output.spent_by = None; }
        }
        if let Some(basket_id) = output.basket_id {
            if !valid_basket_ids.contains(&basket_id) { output.basket_id = None; }
        }
    }
    let valid_proven_tx_ids: std::collections::HashSet<i64> = payload.proven_txs.iter()
        .map(|p| p.proven_tx_id)
        .collect();
    for tx in &mut payload.transactions {
        if let Some(ptx_id) = tx.proven_tx_id {
            if !valid_proven_tx_ids.contains(&ptx_id) { tx.proven_tx_id = None; }
        }
    }
    payload.commissions.retain(|c| valid_tx_ids.contains(&c.transaction_id));
    payload.tx_labels_map.retain(|m| valid_tx_ids.contains(&m.transaction_id));
    let valid_output_ids: std::collections::HashSet<i64> = payload.outputs.iter()
        .map(|o| o.output_id)
        .collect();
    payload.output_tag_map.retain(|m| valid_output_ids.contains(&m.output_id));

    compress_payload(&payload)
}

/// Compress a BackupPayload to gzip bytes.
fn compress_payload(payload: &BackupPayload) -> std::result::Result<Vec<u8>, String> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let json_bytes = serde_json::to_vec(payload)
        .map_err(|e| format!("Failed to serialize payload to JSON: {}", e))?;
    let json_size = json_bytes.len();

    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&json_bytes)
        .map_err(|e| format!("Gzip compression failed: {}", e))?;
    let compressed = encoder.finish()
        .map_err(|e| format!("Gzip finish failed: {}", e))?;

    info!("   On-chain backup: {} bytes JSON → {} bytes compressed ({:.1}% reduction)",
        json_size, compressed.len(),
        (1.0 - compressed.len() as f64 / json_size as f64) * 100.0);

    if compressed.len() > 200_000 {
        log::warn!("   ⚠️  On-chain backup is large ({} KB compressed). Consider wallet cleanup.", compressed.len() / 1024);
    }

    Ok(compressed)
}

/// Encrypt compressed bytes with AES-256-GCM. Returns nonce(12) || ciphertext || tag(16).
pub fn encrypt_compressed(master_privkey: &[u8], compressed: &[u8]) -> std::result::Result<Vec<u8>, String> {
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::aead::generic_array::GenericArray;
    use rand::RngCore;

    let key = derive_onchain_backup_key(master_privkey);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&key));

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, compressed)
        .map_err(|e| format!("AES-256-GCM encryption failed: {}", e))?;

    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypt and decompress on-chain backup bytes back into a BackupPayload.
///
/// Pipeline: AES-256-GCM decrypt → gzip decompress → JSON parse → BackupPayload
///
/// Input format: nonce(12) || ciphertext || tag(16)
pub fn deserialize_from_onchain(
    encrypted_bytes: &[u8],
    master_privkey: &[u8],
) -> std::result::Result<BackupPayload, String> {
    use flate2::read::GzDecoder;
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::aead::generic_array::GenericArray;
    use std::io::Read;

    // Minimum: 12 nonce + 16 tag + at least 1 byte ciphertext
    if encrypted_bytes.len() < 29 {
        return Err("Encrypted backup data too short".to_string());
    }

    // 1. Decrypt
    let key = derive_onchain_backup_key(master_privkey);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&key));

    let nonce = GenericArray::from_slice(&encrypted_bytes[..12]);
    let ciphertext_with_tag = &encrypted_bytes[12..];

    let compressed = cipher.decrypt(nonce, ciphertext_with_tag)
        .map_err(|_| "Failed to decrypt on-chain backup (wrong key or corrupt data)".to_string())?;

    // 2. Decompress gzip
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut json_bytes = Vec::new();
    decoder.read_to_end(&mut json_bytes)
        .map_err(|e| format!("Gzip decompression failed: {}", e))?;

    info!("   On-chain backup recovery: {} bytes encrypted → {} bytes compressed → {} bytes JSON",
        encrypted_bytes.len(), compressed.len(), json_bytes.len());

    // 3. Parse JSON
    serde_json::from_slice::<BackupPayload>(&json_bytes)
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that the on-chain encryption key derivation is deterministic
    #[test]
    fn test_onchain_key_derivation_deterministic() {
        let privkey = [0x42u8; 32];
        let key1 = derive_onchain_backup_key(&privkey);
        let key2 = derive_onchain_backup_key(&privkey);
        assert_eq!(key1, key2);

        // Different privkey → different key
        let other_privkey = [0x43u8; 32];
        let key3 = derive_onchain_backup_key(&other_privkey);
        assert_ne!(key1, key3);
    }

    /// Test round-trip: serialize → encrypt → decrypt → deserialize produces identical payload
    #[test]
    fn test_onchain_round_trip() {
        let privkey = [0x01u8; 32];

        // Build a minimal BackupPayload
        let payload = BackupPayload {
            version: 1,
            identity_key: "02abcdef".to_string(),
            mnemonic: String::new(),
            wallet: BackupWallet {
                id: 1, current_index: 5, backed_up: true,
                created_at: 1000, updated_at: 2000,
            },
            users: vec![BackupUser {
                user_id: 1, identity_key: "02abcdef".to_string(),
                active_storage: "local".to_string(), created_at: 1000, updated_at: 2000,
            }],
            addresses: vec![BackupAddress {
                id: 1, wallet_id: 1, index: 0, address: "1test".to_string(),
                public_key: "02ab".to_string(), used: false, balance: 5000,
                pending_utxo_check: false, created_at: 1000,
            }],
            output_baskets: vec![],
            transactions: vec![],
            outputs: vec![],
            proven_txs: vec![],
            proven_tx_reqs: vec![],
            certificates: vec![],
            certificate_fields: vec![],
            output_tags: vec![],
            output_tag_map: vec![],
            tx_labels: vec![],
            tx_labels_map: vec![],
            commissions: vec![],
            settings: vec![],
            sync_states: vec![],
            parent_transactions: vec![],
            block_headers: vec![],
            domain_permissions: vec![],
            cert_field_permissions: vec![],
        };

        // Serialize to JSON, compress, encrypt
        let json_bytes = serde_json::to_vec(&payload).unwrap();

        // Compress
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&json_bytes).unwrap();
        let compressed = encoder.finish().unwrap();

        // Verify compression ratio
        assert!(compressed.len() < json_bytes.len(),
            "compressed {} should be smaller than json {}", compressed.len(), json_bytes.len());

        // Encrypt
        use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
        use aes_gcm::aead::generic_array::GenericArray;
        use rand::RngCore;

        let key = derive_onchain_backup_key(&privkey);
        let cipher = Aes256Gcm::new(GenericArray::from_slice(&key));
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = GenericArray::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, compressed.as_ref()).unwrap();

        let mut encrypted = Vec::new();
        encrypted.extend_from_slice(&nonce_bytes);
        encrypted.extend_from_slice(&ciphertext);

        // Now decrypt and deserialize using the public API
        let recovered = deserialize_from_onchain(&encrypted, &privkey).unwrap();

        assert_eq!(recovered.version, payload.version);
        assert_eq!(recovered.identity_key, payload.identity_key);
        assert_eq!(recovered.mnemonic, "");
        assert_eq!(recovered.addresses.len(), 1);
        assert_eq!(recovered.addresses[0].address, "1test");
        assert_eq!(recovered.users.len(), 1);
    }

    /// Test that wrong key fails decryption
    #[test]
    fn test_onchain_wrong_key_fails() {
        let privkey = [0x01u8; 32];
        let wrong_privkey = [0x02u8; 32];

        let payload = BackupPayload {
            version: 1,
            identity_key: "02ab".to_string(),
            mnemonic: String::new(),
            wallet: BackupWallet {
                id: 1, current_index: 0, backed_up: true,
                created_at: 1000, updated_at: 2000,
            },
            users: vec![],
            addresses: vec![],
            output_baskets: vec![],
            transactions: vec![],
            outputs: vec![],
            proven_txs: vec![],
            proven_tx_reqs: vec![],
            certificates: vec![],
            certificate_fields: vec![],
            output_tags: vec![],
            output_tag_map: vec![],
            tx_labels: vec![],
            tx_labels_map: vec![],
            commissions: vec![],
            settings: vec![],
            sync_states: vec![],
            parent_transactions: vec![],
            block_headers: vec![],
            domain_permissions: vec![],
            cert_field_permissions: vec![],
        };

        // Encrypt with correct key
        let json_bytes = serde_json::to_vec(&payload).unwrap();
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&json_bytes).unwrap();
        let compressed = encoder.finish().unwrap();

        use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
        use aes_gcm::aead::generic_array::GenericArray;
        use rand::RngCore;

        let key = derive_onchain_backup_key(&privkey);
        let cipher = Aes256Gcm::new(GenericArray::from_slice(&key));
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = GenericArray::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, compressed.as_ref()).unwrap();

        let mut encrypted = Vec::new();
        encrypted.extend_from_slice(&nonce_bytes);
        encrypted.extend_from_slice(&ciphertext);

        // Decrypt with wrong key should fail
        let result = deserialize_from_onchain(&encrypted, &wrong_privkey);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("wrong key or corrupt data"));
    }

    /// Test that data too short is rejected
    #[test]
    fn test_onchain_short_data_rejected() {
        let privkey = [0x01u8; 32];
        let result = deserialize_from_onchain(&[0u8; 20], &privkey);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too short"));
    }
}
