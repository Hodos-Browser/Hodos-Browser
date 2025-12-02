//! One-time migration script to migrate JSON files to database
//!
//! This module handles the migration of wallet.json and actions.json
//! to the SQLite database. This is a ONE-TIME migration.

use rusqlite::Result;
use std::path::Path;
use log::{info, error, warn};
use crate::json_storage::{JsonStorage, Wallet as JsonWallet, AddressInfo};
use crate::action_storage::{ActionStorage, StoredAction};
use super::{WalletDatabase, WalletRepository, AddressRepository};
use std::time::{SystemTime, UNIX_EPOCH};

/// Migrate wallet.json and actions.json to the database
///
/// This is a ONE-TIME migration. It will:
/// 1. Delete any existing wallets in the database
/// 2. Read wallet.json and insert into database
/// 3. Read actions.json and insert into database
///
/// # Arguments
/// * `db` - Database connection
/// * `wallet_json_path` - Path to wallet.json
/// * `actions_json_path` - Path to actions.json
pub fn migrate_json_to_database(
    db: &WalletDatabase,
    wallet_json_path: &Path,
    actions_json_path: &Path,
) -> Result<()> {
    info!("🔄 Starting JSON to database migration...");

    // Step 1: Delete any existing wallets (including test wallet)
    info!("   Step 1: Cleaning existing wallets...");
    db.connection().execute("DELETE FROM addresses", [])?;
    db.connection().execute("DELETE FROM wallets", [])?;
    info!("   ✅ Cleared existing wallets and addresses");

    // Step 2: Read and migrate wallet.json
    info!("   Step 2: Reading wallet.json...");
    let json_storage = JsonStorage::new(wallet_json_path.to_path_buf())
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Failed to read wallet.json: {}", e))
        ))?;

    let wallet = json_storage.get_wallet()
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Failed to get wallet: {}", e))
        ))?;

    info!("   Found wallet with {} addresses", wallet.addresses.len());

    // Step 3: Insert wallet into database
    info!("   Step 3: Inserting wallet into database...");
    let wallet_repo = WalletRepository::new(db.connection());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    db.connection().execute(
        "INSERT INTO wallets (mnemonic, current_index, backed_up, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            wallet.mnemonic,
            wallet.current_index,
            wallet.backed_up,
            now,
            now,
        ],
    )?;

    let wallet_id = db.connection().last_insert_rowid();
    info!("   ✅ Wallet inserted with ID: {}", wallet_id);

    // Step 4: Insert all addresses
    info!("   Step 4: Inserting {} addresses...", wallet.addresses.len());
    let address_repo = AddressRepository::new(db.connection());

    for addr_info in &wallet.addresses {
        let address_model = super::Address {
            id: None,
            wallet_id,
            index: addr_info.index,
            address: addr_info.address.clone(),
            public_key: addr_info.public_key.clone(),
            used: addr_info.used,
            balance: addr_info.balance,
            pending_utxo_check: false, // Migrated addresses don't need immediate check
            created_at: now, // Use current time since we don't have original timestamp
        };

        address_repo.create(&address_model)?;
    }
    info!("   ✅ All addresses inserted");

    // Step 5: Read and migrate actions.json
    info!("   Step 5: Reading actions.json...");
    let action_storage = ActionStorage::new(actions_json_path.to_path_buf())
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Failed to read actions.json: {}", e))
        ))?;

    let actions_count = action_storage.count();
    info!("   Found {} actions to migrate", actions_count);

    if actions_count > 0 {
        // Step 6: Insert all transactions
        info!("   Step 6: Inserting transactions...");
        migrate_actions_to_database(db, &action_storage, wallet_id)?;
        info!("   ✅ All transactions inserted");
    } else {
        info!("   ⚠️  No actions to migrate");
    }

    info!("✅ Migration complete!");
    info!("   Wallet ID: {}", wallet_id);
    info!("   Addresses: {}", wallet.addresses.len());
    info!("   Transactions: {}", actions_count);

    Ok(())
}

/// Migrate actions from ActionStorage to database
fn migrate_actions_to_database(
    db: &WalletDatabase,
    action_storage: &ActionStorage,
    _wallet_id: i64,
) -> Result<()> {
    // Get all actions using list_actions (no filter = all actions)
    let actions = action_storage.list_actions(None, None);
    let actions_count = actions.len();

    info!("   Migrating {} actions...", actions_count);

    for action in actions {
        // Insert into transactions table
        db.connection().execute(
            "INSERT INTO transactions (
                txid, reference_number, raw_tx, description, status, is_outgoing,
                satoshis, timestamp, block_height, confirmations, version, lock_time
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                action.txid,
                action.reference_number,
                action.raw_tx,
                action.description,
                action.status.to_string(),
                action.is_outgoing,
                action.satoshis,
                action.timestamp,
                action.block_height.map(|h| h as i32),
                action.confirmations as i32,
                action.version as i32,
                action.lock_time as i32,
            ],
        )?;

        let transaction_id = db.connection().last_insert_rowid();

        // Insert transaction labels
        for label in &action.labels {
            db.connection().execute(
                "INSERT INTO transaction_labels (transaction_id, label) VALUES (?1, ?2)",
                rusqlite::params![transaction_id, label],
            )?;
        }

        // Insert transaction inputs
        for input in &action.inputs {
            db.connection().execute(
                "INSERT INTO transaction_inputs (
                    transaction_id, txid, vout, satoshis, script
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    transaction_id,
                    input.txid,
                    input.vout as i32,
                    input.satoshis,
                    input.script,
                ],
            )?;
        }

        // Insert transaction outputs
        for output in &action.outputs {
            db.connection().execute(
                "INSERT INTO transaction_outputs (
                    transaction_id, vout, satoshis, script, address
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    transaction_id,
                    output.vout as i32,
                    output.satoshis,
                    output.script,
                    output.address,
                ],
            )?;
        }
    }

    info!("   ✅ Migrated {} actions", actions_count);
    Ok(())
}
