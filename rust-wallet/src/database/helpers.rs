//! Helper functions for database operations
//!
//! Provides convenience functions for common database operations
//! that are used across multiple handlers.

use rusqlite::Result;
use bip39::{Mnemonic, Language};
use bip32::XPrv;
use super::WalletDatabase;

/// Get the master private key from the database wallet
///
/// Uses the cached (decrypted) mnemonic. Returns SQLITE_AUTH error if locked.
pub fn get_master_private_key_from_db(db: &WalletDatabase) -> Result<Vec<u8>> {
    let mnemonic_str = db.get_cached_mnemonic()?;

    let mnemonic = Mnemonic::parse_in(Language::English, mnemonic_str)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Invalid mnemonic: {}", e))
        ))?;

    let seed = mnemonic.to_seed("");

    let master_key = XPrv::new(&seed)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Failed to create master key: {}", e))
        ))?;

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

/// Derive private key for spending an output — Phase 7B/7C direct derivation
///
/// Derives the private key directly from the output's derivation_prefix/suffix
/// and sender_identity_key fields. No address table lookup needed.
///
/// Derivation categories:
/// - prefix="2-receive address", suffix="{N}", sender=NULL → BRC-42 self-derivation
/// - prefix="bip32", suffix="{N}", sender=NULL → BIP32 HD derivation m/{N}
/// - prefix=NULL, suffix=NULL → master private key directly
/// - prefix=any, suffix=any, sender=Some(pubkey) → BRC-42 counterparty derivation
/// - prefix=any, suffix=any, sender=NULL (other) → BRC-42 self-derivation with custom invoice
pub fn derive_key_for_output(
    db: &WalletDatabase,
    derivation_prefix: Option<&str>,
    derivation_suffix: Option<&str>,
    sender_identity_key: Option<&str>,
) -> Result<Vec<u8>> {
    use crate::crypto::brc42::derive_child_private_key;

    match (derivation_prefix, derivation_suffix) {
        // Case 1: NULL/NULL → master private key
        (None, None) => {
            log::info!("   🔑 derive_key_for_output: NULL/NULL → master key");
            get_master_private_key_from_db(db)
        }

        // Case 2: Has prefix and suffix
        (Some(prefix), Some(suffix)) => {
            // Case 2a: BIP32 legacy
            if prefix == "bip32" {
                let index: u32 = suffix.parse().map_err(|e| rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                    Some(format!("Invalid BIP32 index '{}': {}", suffix, e))
                ))?;
                log::info!("   🔑 derive_key_for_output: bip32 index {}", index);
                crate::recovery::derive_private_key_bip32(db, index)
            }
            // Case 2b: Has counterparty (non-self BRC-42)
            else if let Some(sender_key_hex) = sender_identity_key {
                let sender_pubkey = hex::decode(sender_key_hex).map_err(|e| rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                    Some(format!("Invalid senderIdentityKey hex: {}", e))
                ))?;
                let invoice_number = format!("{}-{}", prefix, suffix);
                log::info!("   🔑 derive_key_for_output: counterparty BRC-42 invoice={}", invoice_number);

                let master_privkey = get_master_private_key_from_db(db)?;
                derive_child_private_key(&master_privkey, &sender_pubkey, &invoice_number)
                    .map_err(|e| rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                        Some(format!("BRC-42 counterparty derivation failed: {}", e))
                    ))
            }
            // Case 2c: Self-derivation (BRC-42 with own master pubkey)
            else {
                let invoice_number = format!("{}-{}", prefix, suffix);
                log::info!("   🔑 derive_key_for_output: self BRC-42 invoice={}", invoice_number);

                let master_privkey = get_master_private_key_from_db(db)?;
                let master_pubkey = get_master_public_key_from_db(db)?;
                derive_child_private_key(&master_privkey, &master_pubkey, &invoice_number)
                    .map_err(|e| rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                        Some(format!("BRC-42 self-derivation failed: {}", e))
                    ))
            }
        }

        // Case 3: Partial (one NULL, one not) — shouldn't happen, treat as master key with warning
        _ => {
            log::warn!("   ⚠️  derive_key_for_output: partial derivation fields (prefix={:?}, suffix={:?}), falling back to master key",
                derivation_prefix, derivation_suffix);
            get_master_private_key_from_db(db)
        }
    }
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

/// Convert database Output to utxo_fetcher::UTXO
///
/// This adapter allows the signing code to work with Output structs from the
/// new wallet-toolbox compatible outputs table.
///
/// Key mappings:
/// - `derivation_prefix="2-receive address"` + `derivation_suffix="{n}"` → `address_index = n`
/// - `derivation_prefix=NULL` → `address_index = -1` (master pubkey or unknown)
/// - `locking_script` (BLOB) → `script` (hex string)
pub fn output_to_fetcher_utxo(output: &super::Output) -> crate::utxo_fetcher::UTXO {
    // Derive address_index from derivation_prefix/suffix
    let address_index: i32 = match (&output.derivation_prefix, &output.derivation_suffix) {
        (Some(prefix), Some(suffix)) if prefix == "2-receive address" => {
            // Standard HD wallet address: suffix is the index
            suffix.parse::<i32>().unwrap_or(-1)
        }
        (None, None) => {
            // Master pubkey or unknown derivation
            -1
        }
        _ => {
            // BRC-29 or other custom derivation - use -2 to signal custom
            // The signing code will use custom_instructions for derivation
            -2
        }
    };

    // Convert locking_script BLOB to hex string
    let script = output.locking_script
        .as_ref()
        .map(|bytes| hex::encode(bytes))
        .unwrap_or_default();

    crate::utxo_fetcher::UTXO {
        txid: output.txid.clone().unwrap_or_default(),
        vout: output.vout as u32,
        satoshis: output.satoshis,
        script,
        address_index,
        custom_instructions: output.custom_instructions.clone(),
    }
}
