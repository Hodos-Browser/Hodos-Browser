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
use secp256k1::ecdsa::Signature;
use sha2::{Sha256, Digest};
use ripemd::Ripemd160;
use bs58;
use crate::crypto::brc42::derive_child_public_key;
use crate::utxo_fetcher::fetch_utxos_for_address;
use crate::json_storage::AddressInfo as FetchAddressInfo;
use crate::transaction::{
    Transaction, TxInput, TxOutput, OutPoint, Script,
    calculate_sighash, SIGHASH_ALL_FORKID,
};

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

    // Derive addresses and check blockchain in batches of 20
    let mut recovered_addresses = Vec::new();
    let mut current_index = options.start_index;
    let mut unused_count: u32 = 0;
    let mut total_utxos: u32 = 0;
    let mut total_balance = 0i64;

    const BATCH_SIZE: u32 = 20;

    loop {
        // Determine batch range
        let batch_start = current_index;
        let mut batch_end = batch_start + BATCH_SIZE;
        if let Some(max) = options.max_index {
            if batch_start > max {
                info!("   📊 Reached max_index limit: {}", max);
                break;
            }
            batch_end = batch_end.min(max + 1);
        }
        let batch_count = batch_end - batch_start;

        info!("   🔍 Checking indices {}..{} ({} addresses)...", batch_start, batch_end - 1, batch_count);

        // 1. Derive all addresses in this batch
        let mut batch_bip32: Vec<(u32, Option<AddressInfo>)> = Vec::new();
        let mut batch_brc42: Vec<(u32, Option<AddressInfo>)> = Vec::new();

        for idx in batch_start..batch_end {
            let bip32 = derive_bip32_address(&master_key, idx).ok();
            let brc42 = derive_brc42_address(&master_privkey, &master_pubkey_bytes, idx).ok();
            batch_bip32.push((idx, bip32));
            batch_brc42.push((idx, brc42));
        }

        // 2. Build address list for bulk UTXO fetch
        let mut fetch_addresses: Vec<FetchAddressInfo> = Vec::new();
        // Track which entries are BIP32 vs BRC-42 (for result mapping)
        // Use negative index convention: -(index+1) for BRC-42 to distinguish from BIP32
        for &(idx, ref addr) in &batch_bip32 {
            if let Some(ref a) = addr {
                fetch_addresses.push(FetchAddressInfo {
                    index: idx as i32,
                    address: a.address.clone(),
                    public_key: a.public_key.clone(),
                    used: false,
                    balance: 0,
                });
            }
        }
        for &(idx, ref addr) in &batch_brc42 {
            if let Some(ref a) = addr {
                // Use -(idx+1) so we can distinguish BRC-42 results from BIP32
                fetch_addresses.push(FetchAddressInfo {
                    index: -(idx as i32 + 1),
                    address: a.address.clone(),
                    public_key: a.public_key.clone(),
                    used: false,
                    balance: 0,
                });
            }
        }

        // 3. Bulk fetch UTXOs for all addresses in this batch
        let api_utxos = crate::utxo_fetcher::fetch_all_utxos(&fetch_addresses).await
            .unwrap_or_default();

        // 4. Process results per-index
        for idx in batch_start..batch_end {
            let bip32_addr = batch_bip32.iter().find(|(i, _)| *i == idx).and_then(|(_, a)| a.as_ref());
            let brc42_addr = batch_brc42.iter().find(|(i, _)| *i == idx).and_then(|(_, a)| a.as_ref());

            // Check BIP32 UTXOs (positive index match)
            let bip32_utxos: Vec<_> = api_utxos.iter()
                .filter(|u| u.address_index == idx as i32)
                .cloned()
                .collect();

            // Check BRC-42 UTXOs (negative index match)
            let brc42_utxos: Vec<_> = api_utxos.iter()
                .filter(|u| u.address_index == -(idx as i32 + 1))
                .cloned()
                .collect();

            let mut found_utxos = false;

            // BIP32 UTXOs found
            if !bip32_utxos.is_empty() {
                if let Some(addr) = bip32_addr {
                    let address_balance: i64 = bip32_utxos.iter().map(|u| u.satoshis).sum();
                    let utxo_count = bip32_utxos.len() as u32;
                    info!("   ✅ Found {} UTXO(s) on BIP32 address {} ({} satoshis)",
                          utxo_count, addr.address, address_balance);
                    // Fix address_index back to positive for storage
                    let mut fixed_utxos = bip32_utxos;
                    for u in &mut fixed_utxos { u.address_index = idx as i32; }
                    recovered_addresses.push(RecoveredAddress {
                        index: idx,
                        address: addr.address.clone(),
                        public_key: addr.public_key.clone(),
                        derivation_method: "BIP32".to_string(),
                        has_utxos: true,
                        balance: address_balance,
                        utxos: fixed_utxos,
                    });
                    total_utxos += utxo_count;
                    total_balance += address_balance;
                    found_utxos = true;
                }
            }

            // BRC-42 UTXOs found (only if BIP32 didn't find any)
            if !found_utxos && !brc42_utxos.is_empty() {
                if let Some(addr) = brc42_addr {
                    let address_balance: i64 = brc42_utxos.iter().map(|u| u.satoshis).sum();
                    let utxo_count = brc42_utxos.len() as u32;
                    info!("   ✅ Found {} UTXO(s) on BRC-42 address {} ({} satoshis)",
                          utxo_count, addr.address, address_balance);
                    // Fix address_index back to positive for storage
                    let mut fixed_utxos = brc42_utxos;
                    for u in &mut fixed_utxos { u.address_index = idx as i32; }
                    recovered_addresses.push(RecoveredAddress {
                        index: idx,
                        address: addr.address.clone(),
                        public_key: addr.public_key.clone(),
                        derivation_method: "BRC-42".to_string(),
                        has_utxos: true,
                        balance: address_balance,
                        utxos: fixed_utxos,
                    });
                    total_utxos += utxo_count;
                    total_balance += address_balance;
                    found_utxos = true;
                }
            }

            if found_utxos {
                unused_count = 0;
            } else {
                // No UTXOs — check tx history to determine if address was used and spent.
                // An address with history but 0 balance resets the gap counter.
                let mut has_history = false;
                if let Some(addr) = bip32_addr {
                    if let Ok(true) = crate::utxo_fetcher::address_has_history(&addr.address).await {
                        info!("   📜 BIP32 address {} has tx history (spent), resetting gap counter", addr.address);
                        has_history = true;
                    }
                }
                if !has_history {
                    if let Some(addr) = brc42_addr {
                        if let Ok(true) = crate::utxo_fetcher::address_has_history(&addr.address).await {
                            info!("   📜 BRC-42 address {} has tx history (spent), resetting gap counter", addr.address);
                            has_history = true;
                        }
                    }
                }
                if has_history {
                    unused_count = 0;
                } else {
                    unused_count += 1;
                }
            }

            // Check gap limit after each index
            if unused_count >= options.gap_limit {
                info!("   📊 Gap limit reached ({} unused addresses), stopping recovery", options.gap_limit);
                break;
            }
        }

        // If gap limit was met during the batch, stop
        if unused_count >= options.gap_limit {
            break;
        }

        current_index = batch_end;
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

// ============================================================================
// External Wallet Recovery (Centbee, etc.)
// ============================================================================

/// Derive a private key at an arbitrary BIP32 path from a seed.
///
/// `segments` is a list of `(index, is_hardened)` pairs, e.g.:
/// - `[(44, true), (0, false), (0, false)]` for Centbee receive chain prefix
///
/// Returns 32-byte private key.
pub fn derive_key_at_path(seed: &[u8], segments: &[(u32, bool)]) -> std::result::Result<Vec<u8>, String> {
    let master = XPrv::new(seed)
        .map_err(|e| format!("Failed to create master key: {}", e))?;

    let mut current = master;
    for &(index, hardened) in segments {
        let child_num = bip32::ChildNumber::new(index, hardened)
            .map_err(|e| format!("Invalid child number {}h={}: {}", index, hardened, e))?;
        current = current.derive_child(child_num)
            .map_err(|e| format!("Derivation failed at {}h={}: {}", index, hardened, e))?;
    }

    Ok(current.private_key().to_bytes().to_vec())
}

/// Derive address + keys at an arbitrary BIP32 path from a seed.
///
/// Returns `(address, pubkey_hex, privkey_bytes)`.
pub fn derive_address_at_path(
    seed: &[u8],
    segments: &[(u32, bool)],
) -> std::result::Result<(String, String, Vec<u8>), String> {
    let privkey_bytes = derive_key_at_path(seed, segments)?;

    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&privkey_bytes)
        .map_err(|e| format!("Invalid private key: {}", e))?;
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    let pubkey_bytes = public_key.serialize();

    let address = pubkey_to_address(&pubkey_bytes)
        .map_err(|e| format!("Address derivation failed: {}", e))?;
    let pubkey_hex = hex::encode(&pubkey_bytes);

    Ok((address, pubkey_hex, privkey_bytes))
}

/// Configuration for an external BIP39 wallet's derivation paths.
pub struct ExternalWalletConfig {
    pub name: &'static str,
    /// Path prefixes for each chain (without the final address index).
    /// e.g. receive = `[(44,true),(0,false),(0,false)]`
    pub chains: Vec<Vec<(u32, bool)>>,
    pub chain_labels: Vec<&'static str>,
}

impl ExternalWalletConfig {
    /// Centbee wallet: BIP44 with only 44' hardened, path m/44'/0/0/{i} (receive) and m/44'/0/1/{i} (change).
    pub fn centbee() -> Self {
        Self {
            name: "centbee",
            chains: vec![
                vec![(44, true), (0, false), (0, false)], // receive
                vec![(44, true), (0, false), (1, false)], // change
            ],
            chain_labels: vec!["receive", "change"],
        }
    }
}

/// A UTXO found on an external wallet's address, with its private key for sweep signing.
pub struct ExternalUTXO {
    pub txid: String,
    pub vout: u32,
    pub satoshis: i64,
    pub script_hex: String,
    pub private_key: Vec<u8>,
    pub address: String,
    pub chain_index: usize,
    pub address_index: u32,
}

/// Result of scanning an external wallet for UTXOs.
pub struct ExternalScanResult {
    pub utxos: Vec<ExternalUTXO>,
    pub total_balance: i64,
    pub addresses_scanned: u32,
}

/// Scan an external wallet's derivation paths for UTXOs.
///
/// For each chain in `config`, iterates address indices 0..N, appending `(index, false)`
/// to the chain prefix. Stops after `gap_limit` consecutive empty addresses per chain.
/// Rate-limits API calls at 100ms intervals.
pub async fn scan_external_wallet(
    seed: &[u8],
    config: &ExternalWalletConfig,
    gap_limit: u32,
) -> std::result::Result<ExternalScanResult, String> {
    let mut all_utxos = Vec::new();
    let mut total_balance: i64 = 0;
    let mut total_scanned: u32 = 0;

    for (chain_idx, chain_prefix) in config.chains.iter().enumerate() {
        let label = config.chain_labels.get(chain_idx).unwrap_or(&"unknown");
        info!("   Scanning {} chain ({})...", config.name, label);

        let mut gap_count: u32 = 0;
        let mut addr_index: u32 = 0;

        loop {
            if gap_count >= gap_limit {
                info!("   Gap limit reached on {} chain after {} addresses", label, addr_index);
                break;
            }

            // Build full path: chain_prefix + (addr_index, false)
            let mut path = chain_prefix.clone();
            path.push((addr_index, false));

            let (address, _pubkey_hex, privkey_bytes) = derive_address_at_path(seed, &path)?;

            // Fetch UTXOs from blockchain
            match fetch_utxos_for_address(&address, addr_index as i32).await {
                Ok(utxos) if !utxos.is_empty() => {
                    let addr_balance: i64 = utxos.iter().map(|u| u.satoshis).sum();
                    info!("   Found {} UTXO(s) on {} idx {} ({} sats)",
                          utxos.len(), label, addr_index, addr_balance);

                    for utxo in utxos {
                        all_utxos.push(ExternalUTXO {
                            txid: utxo.txid.clone(),
                            vout: utxo.vout,
                            satoshis: utxo.satoshis,
                            script_hex: utxo.script.clone(),
                            private_key: privkey_bytes.clone(),
                            address: address.clone(),
                            chain_index: chain_idx,
                            address_index: addr_index,
                        });
                        total_balance += utxo.satoshis;
                    }
                    gap_count = 0;
                }
                Ok(_) => {
                    gap_count += 1;
                }
                Err(e) => {
                    log::warn!("   Failed to fetch UTXOs for {} idx {}: {}", label, addr_index, e);
                    gap_count += 1;
                }
            }

            total_scanned += 1;
            addr_index += 1;

            // Rate limiting
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    info!("   External scan complete: {} UTXOs, {} sats, {} addresses scanned",
          all_utxos.len(), total_balance, total_scanned);

    Ok(ExternalScanResult {
        utxos: all_utxos,
        total_balance,
        addresses_scanned: total_scanned,
    })
}

/// Build sweep transactions that move all external UTXOs to a single destination address.
///
/// Batches UTXOs into groups of up to `max_inputs_per_tx` to avoid oversized transactions.
/// Returns `Vec<(raw_tx_hex, fee, output_value)>` for each sweep transaction.
pub fn build_sweep_transactions(
    utxos: &[ExternalUTXO],
    destination_address: &str,
    fee_rate_sats_per_kb: u64,
    max_inputs_per_tx: usize,
) -> std::result::Result<Vec<(String, u64, i64)>, String> {
    if utxos.is_empty() {
        return Err("No UTXOs to sweep".to_string());
    }

    // Decode destination address to get pubkey hash for P2PKH locking script
    let dest_script = address_to_p2pkh_script(destination_address)?;

    let mut results = Vec::new();

    // Process UTXOs in batches
    for batch in utxos.chunks(max_inputs_per_tx) {
        let total_input: i64 = batch.iter().map(|u| u.satoshis).sum();

        // Estimate fee: P2PKH unlocking = 107 bytes, input = 32+4+varint+107+4 = 148 bytes
        let est_size = 4 // version
            + 1 // input count varint
            + batch.len() * 148 // inputs (P2PKH unlocking ~107 bytes)
            + 1 // output count varint
            + 8 + 1 + dest_script.len() // single output
            + 4; // locktime
        let fee = std::cmp::max(
            ((est_size as u64 * fee_rate_sats_per_kb) + 999) / 1000,
            200, // MIN_FEE_SATS
        );

        let output_value = total_input - fee as i64;
        if output_value < 546 {
            // Dust — skip this batch (or it could be a single tiny UTXO)
            log::warn!("   Sweep batch skipped: output {} sats < dust limit (inputs: {} sats, fee: {} sats)",
                      output_value, total_input, fee);
            continue;
        }

        // Build transaction
        let mut tx = Transaction::new();

        // Add inputs (unsigned)
        for utxo in batch {
            let input = TxInput::new(OutPoint::new(&utxo.txid, utxo.vout));
            tx.add_input(input);
        }

        // Add single output
        tx.add_output(TxOutput::new(output_value, dest_script.clone()));

        // Sign each input
        let secp = Secp256k1::new();
        for (i, utxo) in batch.iter().enumerate() {
            let locking_script_bytes = hex::decode(&utxo.script_hex)
                .map_err(|e| format!("Bad script hex for UTXO {}:{}: {}", &utxo.txid[..16.min(utxo.txid.len())], utxo.vout, e))?;

            // Calculate sighash
            let sighash = calculate_sighash(&tx, i, &locking_script_bytes, utxo.satoshis, SIGHASH_ALL_FORKID)
                .map_err(|e| format!("Sighash failed for input {}: {}", i, e))?;

            // Sign with ECDSA
            let secret_key = SecretKey::from_slice(&utxo.private_key)
                .map_err(|e| format!("Invalid privkey for input {}: {}", i, e))?;
            let message = secp256k1::Message::from_digest_slice(&sighash)
                .map_err(|e| format!("Invalid sighash digest for input {}: {}", i, e))?;
            let sig: Signature = secp.sign_ecdsa(&message, &secret_key);

            // DER-encode signature + append hashtype byte (0x41 = SIGHASH_ALL_FORKID)
            let mut sig_bytes = sig.serialize_der().to_vec();
            sig_bytes.push(SIGHASH_ALL_FORKID as u8);

            // Get public key
            let public_key = PublicKey::from_secret_key(&secp, &secret_key);
            let pubkey_bytes = public_key.serialize();

            // Build unlocking script and set on input
            let unlocking = Script::p2pkh_unlocking_script(&sig_bytes, &pubkey_bytes);
            tx.inputs[i].set_script(unlocking.bytes);
        }

        // Serialize
        let raw_hex = tx.to_hex()
            .map_err(|e| format!("Failed to serialize sweep tx: {}", e))?;

        results.push((raw_hex, fee, output_value));
    }

    if results.is_empty() {
        return Err("All UTXO batches were below dust limit after fees".to_string());
    }

    Ok(results)
}

/// Convert a Base58Check Bitcoin address to a P2PKH locking script (25 bytes).
pub fn address_to_p2pkh_script(address: &str) -> std::result::Result<Vec<u8>, String> {
    let decoded = bs58::decode(address)
        .into_vec()
        .map_err(|e| format!("Invalid address base58: {}", e))?;

    if decoded.len() != 25 {
        return Err(format!("Address decoded to {} bytes, expected 25", decoded.len()));
    }

    // Verify checksum
    let payload = &decoded[..21];
    let checksum = &decoded[21..25];
    let hash = Sha256::digest(&Sha256::digest(payload));
    if &hash[..4] != checksum {
        return Err("Address checksum mismatch".to_string());
    }

    // Extract 20-byte pubkey hash (skip version byte)
    let pubkey_hash = &decoded[1..21];

    let script = Script::p2pkh_locking_script(pubkey_hash)
        .map_err(|e| format!("Failed to build P2PKH script: {}", e))?;

    Ok(script.bytes)
}
