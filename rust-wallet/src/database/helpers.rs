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
/// Uses BIP39 to convert mnemonic → seed, then BIP32 to derive child key at index.
/// This matches the behavior of JsonStorage::derive_private_key.
pub fn derive_private_key_from_db(db: &WalletDatabase, index: u32) -> Result<Vec<u8>> {
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
