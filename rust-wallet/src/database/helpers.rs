//! Helper functions for database operations
//!
//! Provides convenience functions for common database operations
//! that are used across multiple handlers.

use rusqlite::Result;
use bip39::{Mnemonic, Language};
use bip32::XPrv;
use super::{WalletDatabase, WalletRepository};

/// Get the master private key from the database wallet
///
/// This derives the master private key (m) from the mnemonic stored in the database.
/// Used for BRC-42/BRC-84 key derivation and authentication.
pub fn get_master_private_key_from_db(db: &WalletDatabase) -> Result<Vec<u8>> {
    let wallet_repo = WalletRepository::new(db.connection());
    let wallet = wallet_repo.get_primary_wallet()?
        .ok_or_else(|| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
            Some("No wallet found in database".to_string())
        ))?;

    // Parse mnemonic
    let mnemonic = Mnemonic::parse_in(Language::English, &wallet.mnemonic)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Invalid mnemonic: {}", e))
        ))?;

    // Generate seed from mnemonic (no password)
    let seed = mnemonic.to_seed("");

    // Create BIP32 master key from seed
    let master_key = XPrv::new(&seed)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Failed to create master key: {}", e))
        ))?;

    // Extract 32-byte master private key
    Ok(master_key.private_key().to_bytes().to_vec())
}

/// Get the master public key from the database wallet
pub fn get_master_public_key_from_db(db: &WalletDatabase) -> Result<Vec<u8>> {
    use secp256k1::{Secp256k1, SecretKey, PublicKey};

    let private_key_bytes = get_master_private_key_from_db(db)?;

    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&private_key_bytes)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Invalid private key: {}", e))
        ))?;

    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // Return compressed format (33 bytes with prefix)
    Ok(public_key.serialize().to_vec())
}

/// Derive private key for a specific address index
///
/// Automatically detects whether to use BRC-42 or BIP32 based on the address index.
/// 
/// - Addresses with index < 15: Uses BIP32 (migrated from old JSON storage)
/// - Addresses with index >= 15: Uses BRC-42 (created after database migration)
/// 
/// This threshold (15) corresponds to the addresses that were migrated from wallet.json
/// during the initial database migration. All addresses created after migration use BRC-42.
pub fn derive_private_key_from_db(db: &WalletDatabase, index: u32) -> Result<Vec<u8>> {
    // Addresses 0-14 were migrated from JSON (created with BIP32)
    // Addresses 15+ are new addresses created with BRC-42
    if index < 15 {
        // Old addresses - use BIP32
        derive_private_key_bip32(db, index)
    } else {
        // New addresses - use BRC-42
        derive_private_key_brc42(db, index)
    }
}

/// Derive private key using BRC-42 (for addresses created with BRC-42)
fn derive_private_key_brc42(db: &WalletDatabase, index: u32) -> Result<Vec<u8>> {
    use crate::crypto::brc42::derive_child_private_key;
    
    let master_privkey = get_master_private_key_from_db(db)?;
    let master_pubkey = get_master_public_key_from_db(db)?;
    
    // Create BRC-43 invoice number: "2-receive address-{index}"
    let invoice_number = format!("2-receive address-{}", index);
    
    // Derive child private key using BRC-42
    derive_child_private_key(&master_privkey, &master_pubkey, &invoice_number)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("BRC-42 derivation failed for index {}: {}", index, e))
        ))
}

/// Derive private key using BIP32 (for old addresses migrated from JSON)
fn derive_private_key_bip32(db: &WalletDatabase, index: u32) -> Result<Vec<u8>> {
    let wallet_repo = WalletRepository::new(db.connection());
    let wallet = wallet_repo.get_primary_wallet()?
        .ok_or_else(|| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
            Some("No wallet found in database".to_string())
        ))?;

    // Parse mnemonic
    let mnemonic = Mnemonic::parse_in(Language::English, &wallet.mnemonic)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Invalid mnemonic: {}", e))
        ))?;

    // Generate seed from mnemonic (no password)
    let seed = mnemonic.to_seed("");

    // Create BIP32 master key from seed
    let master_key = XPrv::new(&seed)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Failed to create master key: {}", e))
        ))?;

    // Derive child key at index
    let child_key = master_key
        .derive_child(bip32::ChildNumber::new(index, false).unwrap())
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Failed to derive child key: {}", e))
        ))?;

    // Extract 32-byte private key
    Ok(child_key.private_key().to_bytes().to_vec())
}

/// Convert database Address to AddressInfo (for compatibility with existing code)
pub fn address_to_address_info(addr: &super::Address) -> crate::json_storage::AddressInfo {
    crate::json_storage::AddressInfo {
        index: addr.index,
        address: addr.address.clone(),
        public_key: addr.public_key.clone(),
        used: addr.used,
        balance: addr.balance,
    }
}

/// Convert database Utxo to utxo_fetcher::UTXO (for compatibility with existing code)
pub fn utxo_to_fetcher_utxo(utxo: &super::Utxo, address_index: u32) -> crate::utxo_fetcher::UTXO {
    crate::utxo_fetcher::UTXO {
        txid: utxo.txid.clone(),
        vout: utxo.vout as u32,
        satoshis: utxo.satoshis,
        script: utxo.script.clone(),
        address_index,
    }
}
