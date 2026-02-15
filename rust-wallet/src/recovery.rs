//! Wallet recovery and legacy BIP32 key derivation
//!
//! This module provides:
//! - Wallet recovery from mnemonic (re-derive addresses, re-discover UTXOs)
//! - BIP32 HD key derivation for spending legacy outputs (pre-BRC-42 addresses)
//! - BIP32 address derivation for recovery scanning
//!
//! BIP32 derivation lives here because it's only needed for:
//! 1. Spending legacy outputs tagged with `derivation_prefix = "bip32"` (via derive_key_for_output)
//! 2. Recovery scanning (check both BIP32 and BRC-42 addresses for UTXOs)
//! 3. Future: importing external BIP32 wallets
//!
//! New outputs use BRC-42 self-derivation exclusively. See `derive_key_for_output` in
//! `database/helpers.rs` for the active signing path.
//!
//! TODO (Recovery Sprint): Full recovery endpoint, database import, gap limit scanning

use crate::database::WalletDatabase;
use rusqlite::Result;
use log::info;
use bip39::{Mnemonic, Language};
use bip32::XPrv;
use secp256k1::{Secp256k1, SecretKey, PublicKey};
use sha2::{Sha256, Digest};
use ripemd::Ripemd160;
use bs58;
use crate::crypto::brc42::derive_child_public_key;
use crate::utxo_fetcher::fetch_utxos_for_address;

// ============================================================================
// BIP32 Key Derivation (Legacy)
// ============================================================================

/// Derive a BIP32 private key at `m/{index}` from the wallet's mnemonic.
///
/// Used for spending legacy outputs tagged with `derivation_prefix = "bip32"`.
/// Called by `derive_key_for_output()` in the signing path.
///
/// New outputs use BRC-42 self-derivation instead. This function is preserved
/// for backward compatibility with pre-BRC-42 addresses.
pub fn derive_private_key_bip32(db: &WalletDatabase, index: u32) -> Result<Vec<u8>> {
    // Use cached (decrypted) mnemonic — returns error if wallet is locked
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

    let child_key = master_key
        .derive_child(bip32::ChildNumber::new(index, false).unwrap())
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Failed to derive child key: {}", e))
        ))?;

    Ok(child_key.private_key().to_bytes().to_vec())
}

// ============================================================================
// Recovery
// ============================================================================

/// Options for wallet recovery
#[derive(Debug, Clone)]
pub struct RecoveryOptions {
    pub mnemonic: String,
    pub gap_limit: u32,  // Default: 20
    pub start_index: u32,  // Default: 0
    pub max_index: Option<u32>,  // Optional limit
}

impl Default for RecoveryOptions {
    fn default() -> Self {
        RecoveryOptions {
            mnemonic: String::new(),
            gap_limit: 20,
            start_index: 0,
            max_index: None,
        }
    }
}

/// Result of wallet recovery
#[derive(Debug)]
pub struct RecoveryResult {
    pub addresses_found: u32,
    pub utxos_found: u32,
    pub total_balance: i64,
    pub addresses: Vec<RecoveredAddress>,
}

/// Information about a recovered address
#[derive(Debug, Clone)]
pub struct RecoveredAddress {
    pub index: u32,
    pub address: String,
    pub public_key: String,
    pub derivation_method: String, // "BIP32" or "BRC-42"
    pub has_utxos: bool,
    pub balance: i64,
    pub utxos: Vec<crate::utxo_fetcher::UTXO>,
}

/// Recover wallet from mnemonic
///
/// This function:
/// 1. Re-derives addresses from mnemonic (trying both BIP32 and BRC-42)
/// 2. Checks blockchain for UTXOs on each address
/// 3. Uses gap limit to determine when to stop
/// 4. Returns discovered addresses and UTXOs (does not save to database yet)
///
/// No DB parameter — this function only does network calls (safe across .await).
///
/// # Arguments
/// * `options` - Recovery options (mnemonic, gap limit, etc.)
///
/// # Returns
/// * `Ok(RecoveryResult)` if recovery succeeded
/// * `Err` if recovery failed
pub async fn recover_wallet_from_mnemonic(
    options: RecoveryOptions,
) -> Result<RecoveryResult> {
    info!("🔍 Starting wallet recovery from mnemonic...");
    info!("   Gap limit: {}", options.gap_limit);
    info!("   Start index: {}", options.start_index);

    // Parse mnemonic
    let mnemonic = Mnemonic::parse_in(Language::English, &options.mnemonic)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Invalid mnemonic: {}", e))
        ))?;

    // Generate seed from mnemonic
    let seed = mnemonic.to_seed("");

    // Create BIP32 master key
    let master_key = XPrv::new(&seed)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Failed to create master key: {}", e))
        ))?;

    // Get master private and public keys for BRC-42
    let master_privkey = master_key.private_key().to_bytes();
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&master_privkey)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Invalid master private key: {}", e))
        ))?;
    let master_pubkey = PublicKey::from_secret_key(&secp, &secret_key);
    let master_pubkey_bytes = master_pubkey.serialize();

    // Derive addresses and check blockchain
    let mut recovered_addresses = Vec::new();
    let mut current_index = options.start_index;
    let mut unused_count = 0;
    let mut total_utxos = 0;
    let mut total_balance = 0i64;

    loop {
        // Check max_index limit
        if let Some(max) = options.max_index {
            if current_index > max {
                info!("   📊 Reached max_index limit: {}", max);
                break;
            }
        }

        info!("   🔍 Checking index {}...", current_index);

        // Try BIP32 derivation first
        let bip32_address = match derive_bip32_address(&master_key, current_index) {
            Ok(addr) => Some(addr),
            Err(e) => {
                log::warn!("   ⚠️  BIP32 derivation failed for index {}: {}", current_index, e);
                None
            }
        };

        // Try BRC-42 derivation
        let brc42_address = match derive_brc42_address(&master_privkey, &master_pubkey_bytes, current_index) {
            Ok(addr) => Some(addr),
            Err(e) => {
                log::warn!("   ⚠️  BRC-42 derivation failed for index {}: {}", current_index, e);
                None
            }
        };

        // Check both addresses for UTXOs
        let mut found_utxos = false;
        let mut recovered_address: Option<RecoveredAddress> = None;

        // Check BIP32 address
        if let Some(ref addr) = bip32_address {
            match fetch_utxos_for_address(&addr.address, current_index as i32).await {
                Ok(utxos) if !utxos.is_empty() => {
                    let address_balance = utxos.iter().map(|u| u.satoshis).sum();
                    let utxo_count = utxos.len() as u32;
                    info!("   ✅ Found {} UTXO(s) on BIP32 address {} ({} satoshis)",
                          utxo_count, addr.address, address_balance);
                    found_utxos = true;
                    recovered_address = Some(RecoveredAddress {
                        index: current_index,
                        address: addr.address.clone(),
                        public_key: addr.public_key.clone(),
                        derivation_method: "BIP32".to_string(),
                        has_utxos: true,
                        balance: address_balance,
                        utxos,
                    });
                    total_utxos += utxo_count;
                    total_balance += address_balance;
                }
                Ok(_) => {
                    // No UTXOs, continue
                }
                Err(e) => {
                    log::warn!("   ⚠️  Failed to fetch UTXOs for BIP32 address {}: {}", addr.address, e);
                }
            }
        }

        // Check BRC-42 address (only if BIP32 didn't find anything)
        if !found_utxos {
            if let Some(ref addr) = brc42_address {
                match fetch_utxos_for_address(&addr.address, current_index as i32).await {
                    Ok(utxos) if !utxos.is_empty() => {
                        let address_balance = utxos.iter().map(|u| u.satoshis).sum();
                        let utxo_count = utxos.len() as u32;
                        info!("   ✅ Found {} UTXO(s) on BRC-42 address {} ({} satoshis)",
                              utxo_count, addr.address, address_balance);
                        found_utxos = true;
                        recovered_address = Some(RecoveredAddress {
                            index: current_index,
                            address: addr.address.clone(),
                            public_key: addr.public_key.clone(),
                            derivation_method: "BRC-42".to_string(),
                            has_utxos: true,
                            balance: address_balance,
                            utxos,
                        });
                        total_utxos += utxo_count;
                        total_balance += address_balance;
                    }
                    Ok(_) => {
                        // No UTXOs, continue
                    }
                    Err(e) => {
                        log::warn!("   ⚠️  Failed to fetch UTXOs for BRC-42 address {}: {}", addr.address, e);
                    }
                }
            }
        }

        // If we found UTXOs, save the address
        if let Some(addr) = recovered_address {
            recovered_addresses.push(addr);
            unused_count = 0; // Reset gap limit
        } else {
            unused_count += 1;
        }

        // Check gap limit
        if unused_count >= options.gap_limit {
            info!("   📊 Gap limit reached ({} unused addresses), stopping recovery", options.gap_limit);
            break;
        }

        current_index += 1;

        // Rate limiting: small delay to avoid API limits
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    info!("   ✅ Recovery complete!");
    info!("   📊 Found {} addresses with {} UTXOs, total balance: {} satoshis",
          recovered_addresses.len(), total_utxos, total_balance);

    Ok(RecoveryResult {
        addresses_found: recovered_addresses.len() as u32,
        utxos_found: total_utxos,
        total_balance,
        addresses: recovered_addresses,
    })
}

/// Derive address using BIP32
fn derive_bip32_address(master_key: &XPrv, index: u32) -> Result<AddressInfo, rusqlite::Error> {
    use bip32::ChildNumber;

    // Derive child key
    let child_key = master_key
        .derive_child(ChildNumber::new(index, false).unwrap())
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("BIP32 derivation failed: {}", e))
        ))?;

    // Get private key
    let privkey_bytes = child_key.private_key().to_bytes();

    // Derive public key
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&privkey_bytes)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("Invalid private key: {}", e))
        ))?;
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    let pubkey_bytes = public_key.serialize();

    // Convert to address
    let address = pubkey_to_address(&pubkey_bytes)?;
    let pubkey_hex = hex::encode(&pubkey_bytes);

    Ok(AddressInfo {
        address,
        public_key: pubkey_hex,
    })
}

/// Derive address using BRC-42
fn derive_brc42_address(
    master_privkey: &[u8],
    master_pubkey: &[u8],
    index: u32,
) -> Result<AddressInfo, rusqlite::Error> {
    // Create invoice number for BRC-42
    let invoice_number = format!("2-receive address-{}", index);

    // Derive child public key
    let derived_pubkey = derive_child_public_key(master_privkey, master_pubkey, &invoice_number)
        .map_err(|e| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some(format!("BRC-42 derivation failed: {}", e))
        ))?;

    // Convert to address
    let address = pubkey_to_address(&derived_pubkey)?;
    let pubkey_hex = hex::encode(&derived_pubkey);

    Ok(AddressInfo {
        address,
        public_key: pubkey_hex,
    })
}

/// Convert public key to Bitcoin address
fn pubkey_to_address(pubkey_bytes: &[u8]) -> Result<String, rusqlite::Error> {
    // Hash public key: SHA256 then RIPEMD160
    let sha256_hash = Sha256::digest(pubkey_bytes);
    let ripemd160_hash = Ripemd160::digest(&sha256_hash);

    // Add version byte (0x00 for mainnet)
    let mut address_bytes = vec![0x00];
    address_bytes.extend_from_slice(&ripemd160_hash);

    // Double SHA256 for checksum
    let checksum = Sha256::digest(&Sha256::digest(&address_bytes)[..]);
    address_bytes.extend_from_slice(&checksum[..4]);

    // Base58 encode
    let address = bs58::encode(&address_bytes).into_string();

    Ok(address)
}

/// Address information (internal)
struct AddressInfo {
    address: String,
    public_key: String,
}
