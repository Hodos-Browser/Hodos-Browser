//! Database backup and restore utilities
//!
//! This module provides functions for backing up and restoring the wallet database.
//! Supports file-based backup (SQLite database copy) and JSON export (non-sensitive data).
//!
//! **Note**: For periodic/automatic backups, consider:
//! - Storing backups in a different location than the database (different disk/directory)
//! - Implementing retention policies (keep last N backups)
//! - Adding encryption for backup files
//! - Scheduling automatic backups (daily/weekly)
//! These features are deferred to the app refinement stage.

use crate::database::WalletDatabase;
use rusqlite::Result;
use std::fs;
use std::path::Path;
use log::{info, warn};

/// Backup the database using file copy
///
/// This method copies the SQLite database file and WAL file (if present).
/// The database should be in WAL mode for safe copying while in use.
///
/// # Arguments
/// * `source_path` - Path to the source database file
/// * `dest_path` - Path where the backup should be saved
///
/// # Returns
/// * `Ok(())` if backup succeeded
/// * `Err` if backup failed
pub fn backup_database_file(source_path: &Path, dest_path: &Path) -> Result<()> {
    info!("💾 Starting database backup...");
    info!("   Source: {}", source_path.display());
    info!("   Destination: {}", dest_path.display());

    // Ensure destination directory exists
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to create backup directory: {}", e))
            ))?;
    }

    // Copy main database file
    fs::copy(source_path, dest_path)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
            Some(format!("Failed to copy database file: {}", e))
        ))?;

    info!("   ✅ Copied database file");

    // Copy WAL file if it exists (Write-Ahead Logging mode)
    let wal_path = source_path.with_extension("db-wal");
    if wal_path.exists() {
        let dest_wal = dest_path.with_extension("db-wal");
        fs::copy(&wal_path, &dest_wal)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to copy WAL file: {}", e))
            ))?;
        info!("   ✅ Copied WAL file");
    }

    // Copy SHM file if it exists (Shared Memory file for WAL mode)
    let shm_path = source_path.with_extension("db-shm");
    if shm_path.exists() {
        let dest_shm = dest_path.with_extension("db-shm");
        fs::copy(&shm_path, &dest_shm)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to copy SHM file: {}", e))
            ))?;
        info!("   ✅ Copied SHM file");
    }

    // Get backup file size
    let metadata = fs::metadata(dest_path)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
            Some(format!("Failed to get backup file metadata: {}", e))
        ))?;
    let size_bytes = metadata.len();

    info!("   ✅ Backup complete! Size: {} bytes", size_bytes);
    Ok(())
}

/// Restore database from backup
///
/// This method replaces the current database with a backup file.
/// **WARNING**: This will overwrite the existing database!
///
/// # Arguments
/// * `backup_path` - Path to the backup database file
/// * `dest_path` - Path where the database should be restored
///
/// # Returns
/// * `Ok(())` if restore succeeded
/// * `Err` if restore failed
pub fn restore_database(backup_path: &Path, dest_path: &Path) -> Result<()> {
    info!("🔄 Starting database restore...");
    info!("   Backup: {}", backup_path.display());
    info!("   Destination: {}", dest_path.display());

    // Verify backup file exists
    if !backup_path.exists() {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
            Some(format!("Backup file not found: {}", backup_path.display()))
        ));
    }

    // Ensure destination directory exists
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to create restore directory: {}", e))
            ))?;
    }

    // Copy backup to destination
    fs::copy(backup_path, dest_path)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
            Some(format!("Failed to restore database file: {}", e))
        ))?;

    info!("   ✅ Restored database file");

    // Restore WAL file if it exists
    let backup_wal = backup_path.with_extension("db-wal");
    if backup_wal.exists() {
        let dest_wal = dest_path.with_extension("db-wal");
        fs::copy(&backup_wal, &dest_wal)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to restore WAL file: {}", e))
            ))?;
        info!("   ✅ Restored WAL file");
    }

    // Restore SHM file if it exists
    let backup_shm = backup_path.with_extension("db-shm");
    if backup_shm.exists() {
        let dest_shm = dest_path.with_extension("db-shm");
        fs::copy(&backup_shm, &dest_shm)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                Some(format!("Failed to restore SHM file: {}", e))
            ))?;
        info!("   ✅ Restored SHM file");
    }

    info!("   ✅ Restore complete!");
    Ok(())
}

/// Verify backup file integrity
///
/// This method checks if a backup file is a valid SQLite database.
///
/// # Arguments
/// * `backup_path` - Path to the backup file
///
/// # Returns
/// * `Ok(true)` if backup is valid
/// * `Ok(false)` if backup is invalid
/// * `Err` if verification failed
pub fn verify_backup(backup_path: &Path) -> Result<bool> {
    info!("🔍 Verifying backup: {}", backup_path.display());

    // Check if file exists
    if !backup_path.exists() {
        return Ok(false);
    }

    // Try to open the database
    match rusqlite::Connection::open(backup_path) {
        Ok(conn) => {
            // Try a simple query to verify integrity
            match conn.query_row("SELECT 1", [], |_row| Ok(())) {
                Ok(_) => {
                    info!("   ✅ Backup is valid");
                    Ok(true)
                }
                Err(e) => {
                    log::warn!("   ⚠️  Backup file exists but is corrupted: {}", e);
                    Ok(false)
                }
            }
        }
        Err(e) => {
            log::warn!("   ⚠️  Backup file is not a valid SQLite database: {}", e);
            Ok(false)
        }
    }
}

/// Export non-sensitive wallet data to JSON
///
/// This exports addresses, transactions, and UTXOs (but NOT mnemonic or private keys).
/// Useful for debugging, migration, or accounting purposes.
///
/// # Arguments
/// * `db` - Database connection
/// * `dest_path` - Path where JSON export should be saved
///
/// # Returns
/// * `Ok(())` if export succeeded
/// * `Err` if export failed
pub fn export_to_json(db: &WalletDatabase, dest_path: &Path) -> Result<()> {
    use crate::database::{AddressRepository, TransactionRepository, UtxoRepository, Address};
    use serde_json;
    use std::time::{SystemTime, UNIX_EPOCH};

    info!("📄 Exporting wallet data to JSON...");
    info!("   Destination: {}", dest_path.display());

    // Ensure destination directory exists
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
    let utxo_repo = UtxoRepository::new(conn);

    // Get wallet ID first
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

    // Export structure
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

    // Get all addresses for the wallet
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

    // Get all transactions (no filters)
    let transactions = transaction_repo.list_transactions(None, None)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
            Some(format!("Failed to get transactions: {}", e))
        ))?;

    let transaction_exports: Vec<TransactionExport> = transactions.iter()
        .map(|tx| TransactionExport {
            txid: tx.txid.clone(),
            reference_number: Some(tx.reference_number.clone()),
            label: tx.labels.first().cloned(), // Get first label if any
            amount: tx.satoshis,
            created_at: tx.timestamp,
        })
        .collect();

    // Get all UTXOs for all addresses in the wallet
    let address_ids: Vec<i64> = addresses.iter()
        .filter_map(|addr| addr.id)
        .collect();

    let utxos = if address_ids.is_empty() {
        Vec::new()
    } else {
        utxo_repo.get_unspent_by_addresses(&address_ids)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
                Some(format!("Failed to get UTXOs: {}", e))
            ))?
    };

    // Create a map of address_id -> address for quick lookup
    let address_map: std::collections::HashMap<i64, &Address> = addresses.iter()
        .filter_map(|addr| addr.id.map(|id| (id, addr)))
        .collect();

    // Get addresses for UTXO export
    let utxo_exports: Vec<UtxoExport> = utxos.iter()
        .filter_map(|utxo| {
            // Get address for this UTXO
            utxo.address_id.and_then(|aid| address_map.get(&aid)).map(|addr| UtxoExport {
                txid: utxo.txid.clone(),
                vout: utxo.vout,
                address: addr.address.clone(),
                amount: utxo.satoshis,
                is_spent: utxo.is_spent,
            })
        })
        .collect();

    // Create export data
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

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&export_data)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Failed to serialize export data: {}", e))
        ))?;

    // Write to file
    fs::write(dest_path, json)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
            Some(format!("Failed to write export file: {}", e))
        ))?;

    info!("   ✅ Export complete!");
    info!("   📊 Exported: {} addresses, {} transactions, {} UTXOs",
          export_data.addresses.len(),
          export_data.transactions.len(),
          export_data.utxos.len());

    Ok(())
}
