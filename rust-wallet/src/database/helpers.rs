//! Helper functions for database operations
//!
//! Provides convenience functions for common database operations
//! that are used across multiple handlers.

use rusqlite::Result;
use bip39::{Mnemonic, Language};
use bip32::XPrv;
use log::info;
use super::{WalletDatabase, WalletRepository, AddressRepository};

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

/// Derive private key for spending a UTXO
///
/// This is the main entry point for deriving private keys for spending.
/// It handles all address types:
/// - index >= 0: HD wallet addresses (BIP32 or BRC-42 "receive address")
/// - index == -1: Master public key address (return master private key)
/// - index < -1: BRC-29 derived addresses (use custom_instructions for derivation)
///
/// For index < -1, custom_instructions MUST be provided (contains BRC-29 derivation info).
pub fn derive_private_key_for_utxo(
    db: &WalletDatabase,
    index: i32,
    custom_instructions: Option<&str>,
) -> Result<Vec<u8>> {
    use crate::crypto::brc42::derive_child_private_key;

    // Handle special cases for negative indices
    if index == -1 {
        // Master address - return master private key directly
        log::info!("   🔑 Index -1: Using master private key directly");
        return get_master_private_key_from_db(db);
    }

    if index < -1 {
        // BRC-29 derived address - parse custom_instructions for derivation
        log::info!("   🔑 Index {}: BRC-29 derived address, parsing custom_instructions", index);

        let instructions = custom_instructions.ok_or_else(|| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("BRC-29 derived address (index {}) requires custom_instructions for spending", index))
        ))?;

        // Parse the JSON custom_instructions
        let instr: serde_json::Value = serde_json::from_str(instructions)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Invalid custom_instructions JSON: {}", e))
            ))?;

        // Extract BRC-29 fields
        let sender_identity_key = instr["senderIdentityKey"].as_str()
            .ok_or_else(|| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some("custom_instructions missing senderIdentityKey".to_string())
            ))?;

        let derivation_prefix = instr["derivationPrefix"].as_str()
            .ok_or_else(|| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some("custom_instructions missing derivationPrefix".to_string())
            ))?;

        let derivation_suffix = instr["derivationSuffix"].as_str()
            .ok_or_else(|| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some("custom_instructions missing derivationSuffix".to_string())
            ))?;

        // Parse sender's public key
        let sender_pubkey_bytes = hex::decode(sender_identity_key)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Invalid senderIdentityKey hex: {}", e))
            ))?;

        // Build BRC-29 invoice number: "2-3241645161d8-{prefix} {suffix}"
        let key_id = format!("{} {}", derivation_prefix, derivation_suffix);
        let invoice_number = format!("2-3241645161d8-{}", key_id);
        log::info!("   BRC-29 invoice number: {}", invoice_number);

        // Get master private key and derive child private key using BRC-42
        let master_privkey = get_master_private_key_from_db(db)?;

        let child_privkey = derive_child_private_key(&master_privkey, &sender_pubkey_bytes, &invoice_number)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("BRC-42 derivation failed for BRC-29 address: {}", e))
            ))?;

        log::info!("   ✅ BRC-29 derived private key ready for signing");
        return Ok(child_privkey);
    }

    // Positive index - use original logic for HD wallet addresses
    derive_private_key_from_db_positive(db, index as u32)
}

/// Derive private key for a specific positive address index
///
/// Automatically detects whether to use BRC-42 or BIP32 by verifying which method
/// produces the address stored in the database. This is more robust than using
/// a hardcoded threshold.
fn derive_private_key_from_db_positive(db: &WalletDatabase, index: u32) -> Result<Vec<u8>> {
    use super::AddressRepository;
    use crate::crypto::brc42::derive_child_public_key;
    use sha2::{Sha256, Digest};
    use ripemd::Ripemd160;
    use bs58;

    // Get the address from database to verify derivation method
    let wallet_repo = WalletRepository::new(db.connection());
    let wallet = wallet_repo.get_primary_wallet()?
        .ok_or_else(|| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
            Some("No wallet found in database".to_string())
        ))?;
    let wallet_id = wallet.id.ok_or_else(|| rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
        Some("Wallet has no ID".to_string())
    ))?;

    let address_repo = AddressRepository::new(db.connection());
    let stored_address = match address_repo.get_by_wallet_and_index(wallet_id, index as i32) {
        Ok(Some(addr)) => addr.address,
        Ok(None) => {
            // Address doesn't exist yet - default to BRC-42 for new addresses
            log::info!("   Address index {} not found in database, using BRC-42 (default for new addresses)", index);
            return derive_private_key_brc42(db, index);
        }
        Err(e) => {
            // If we can't look up the address, try both methods
            log::warn!("   Failed to look up address index {}: {}, will try both derivation methods", index, e);
            return try_both_derivation_methods(db, index);
        }
    };

    // Try BIP32 first (for old addresses)
    let bip32_key = match derive_private_key_bip32(db, index) {
        Ok(key) => key,
        Err(_) => {
            // BIP32 derivation failed, try BRC-42
            log::info!("   BIP32 derivation failed for index {}, trying BRC-42", index);
            return derive_private_key_brc42(db, index);
        }
    };

    // Verify BIP32 produces the correct address
    let bip32_address = {
        use secp256k1::{Secp256k1, SecretKey, PublicKey};
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&bip32_key)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Invalid BIP32 key: {}", e))
            ))?;
        let pubkey = PublicKey::from_secret_key(&secp, &secret);
        let pubkey_bytes = pubkey.serialize();

        // Hash to address
        let sha_hash = Sha256::digest(&pubkey_bytes);
        let pubkey_hash = Ripemd160::digest(&sha_hash);
        let mut addr_bytes = vec![0x00];
        addr_bytes.extend_from_slice(pubkey_hash.as_slice());
        let checksum_full = Sha256::digest(&Sha256::digest(&addr_bytes));
        let checksum = &checksum_full[0..4];
        addr_bytes.extend_from_slice(checksum);
        bs58::encode(&addr_bytes).into_string()
    };

    if bip32_address == stored_address {
        log::info!("   ✅ Address index {} matches BIP32 derivation", index);
        return Ok(bip32_key);
    }

    // BIP32 doesn't match, try BRC-42
    log::info!("   Address index {} doesn't match BIP32, trying BRC-42", index);
    let brc42_key = derive_private_key_brc42(db, index)?;

    // Verify BRC-42 produces the correct address
    let brc42_address = {
        let master_privkey = get_master_private_key_from_db(db)?;
        let master_pubkey = get_master_public_key_from_db(db)?;
        let invoice_number = format!("2-receive address-{}", index);
        let derived_pubkey = derive_child_public_key(&master_privkey, &master_pubkey, &invoice_number)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("BRC-42 derivation failed: {}", e))
            ))?;

        // Hash to address
        let sha_hash = Sha256::digest(&derived_pubkey);
        let pubkey_hash = Ripemd160::digest(&sha_hash);
        let mut addr_bytes = vec![0x00];
        addr_bytes.extend_from_slice(pubkey_hash.as_slice());
        let checksum_full = Sha256::digest(&Sha256::digest(&addr_bytes));
        let checksum = &checksum_full[0..4];
        addr_bytes.extend_from_slice(checksum);
        bs58::encode(&addr_bytes).into_string()
    };

    if brc42_address == stored_address {
        log::info!("   ✅ Address index {} matches BRC-42 derivation", index);
        Ok(brc42_key)
    } else {
        // Neither method matches - this shouldn't happen
        Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!(
                "Address index {} doesn't match BIP32 ({}) or BRC-42 ({}) derivation. Stored: {}",
                index, bip32_address, brc42_address, stored_address
            ))
        ))
    }
}

/// Try both derivation methods when address lookup fails
fn try_both_derivation_methods(db: &WalletDatabase, index: u32) -> Result<Vec<u8>> {
    // Default to BRC-42 for new addresses if we can't verify
    log::warn!("   ⚠️  Cannot verify derivation method for index {}, defaulting to BRC-42", index);
    derive_private_key_brc42(db, index)
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
pub fn utxo_to_fetcher_utxo(utxo: &super::Utxo, address_index: i32) -> crate::utxo_fetcher::UTXO {
    crate::utxo_fetcher::UTXO {
        txid: utxo.txid.clone(),
        vout: utxo.vout as u32,
        satoshis: utxo.satoshis,
        script: utxo.script.clone(),
        address_index,
        custom_instructions: utxo.custom_instructions.clone(),
    }
}
