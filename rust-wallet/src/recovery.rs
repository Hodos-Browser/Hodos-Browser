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
    /// Inclusive index ranges the scan could **not** read (B5-T1-P4). Non-empty ⇒ the
    /// scan is incomplete: "nothing found" there means "could not look".
    pub unchecked_ranges: Vec<(u32, u32)>,
}

impl RecoveryResult {
    pub fn complete(&self) -> bool {
        self.unchecked_ranges.is_empty()
    }

    /// "incomplete — N ranges could not be checked (a–b, …)", or `None` when complete.
    pub fn incomplete_message(&self) -> Option<String> {
        if self.complete() {
            return None;
        }
        let list: Vec<String> = self.unchecked_ranges.iter()
            .map(|(a, b)| if a == b { a.to_string() } else { format!("{}–{}", a, b) })
            .collect();
        Some(format!("incomplete — {} range(s) of addresses could not be checked (indices {})",
                     self.unchecked_ranges.len(), list.join(", ")))
    }
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
///
/// ⛔ B5-T1-P6 (owner, 2026-10-07, K1): no scan calls this any more — the wallet is
/// BRC-42 and scans only its own receive addresses ([`scan_brc42_self`]). Kept for a
/// future old-wallet import.
#[allow(dead_code)]
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

    // Derive addresses and check blockchain in batches of 20 (BIP32 only — BRC-42 disabled)
    // BRC-42 scanning is disabled because it creates duplicate/phantom UTXOs
    // when backup change outputs exist at derived addresses. The on-chain backup
    // system handles BRC-42 address recovery. BRC-42 scanning will be revisited
    // as a separate sprint after MVP.
    let result = scan_indices(
        &options,
        "BIP32",
        None,
        |idx| derive_bip32_address(&master_key, idx).ok().map(|a| (a.address, a.public_key)),
        |batch| async move { crate::utxo_fetcher::fetch_all_utxos_detailed(&batch).await },
        |address| async move { crate::utxo_fetcher::address_has_history(&address).await },
    ).await;

    info!("   ✅ Recovery scan finished{}", if result.complete() { "" } else { " — INCOMPLETE" });
    info!("   📊 Found {} addresses with {} UTXOs, total balance: {} satoshis; unchecked ranges: {:?}",
          result.addresses_found, result.utxos_found, result.total_balance, result.unchecked_ranges);

    Ok(result)
}

/// Indices read past the high-water mark before the gap limit starts counting (B5-T1-P6).
pub const SCAN_MARGIN: u32 = 20;

/// Above `high-water + SCAN_MARGIN`, the scan stops after this many indices in a row
/// with no coin and no history. 👤 Owner, 2026-10-07 (K9): *"at least 20"* — a user can
/// press "generate address" many times without being paid.
pub const SCAN_GAP_LIMIT: u32 = 50;

/// Hard stop: never read more than this many indices past `high-water + SCAN_MARGIN`.
/// An indexer that reports history at every address would otherwise keep the scan going
/// for ever; stopping here is reported as unchecked, never as "complete".
pub const SCAN_CEILING_ABOVE: u32 = 5_000;

/// B5-T1-P6: the scan of the wallet's **own** receive addresses — BRC-42,
/// counterparty = self, invoice `2-receive address-{i}`. Reads every index
/// `0..=high_water + SCAN_MARGIN`, then continues until [`SCAN_GAP_LIMIT`] unused
/// indices in a row. Pure: it derives and asks `fetch` / `history`, and writes nothing.
///
/// ⛔ Only self-derived addresses can be found this way. A PeerPay / BRC-29 output is
/// locked to a key derived from the **sender's** key and a random prefix/suffix; no scan
/// can enumerate it.
pub(crate) async fn scan_brc42_self<F, FF, H, HF>(
    master_privkey: &[u8],
    master_pubkey: &[u8],
    high_water: u32,
    fetch: F,
    history: H,
) -> RecoveryResult
where
    F: Fn(Vec<FetchAddressInfo>) -> FF,
    FF: std::future::Future<Output = std::result::Result<(Vec<crate::utxo_fetcher::UTXO>, Vec<String>), String>>,
    H: Fn(String) -> HF,
    HF: std::future::Future<Output = std::result::Result<bool, String>>,
{
    let through = high_water.saturating_add(SCAN_MARGIN);
    let ceiling = through.saturating_add(SCAN_CEILING_ABOVE);
    let options = RecoveryOptions {
        mnemonic: String::new(),
        gap_limit: SCAN_GAP_LIMIT,
        start_index: 0,
        max_index: Some(ceiling),
    };
    info!("🔍 Scanning own receive addresses 0..={} (high-water {}), then until {} unused in a row (ceiling {})",
          through, high_water, SCAN_GAP_LIMIT, ceiling);
    let highest = std::cell::Cell::new(0u32);
    let mut result = scan_indices(
        &options,
        "BRC-42",
        Some(through),
        |idx| derive_brc42_address(master_privkey, master_pubkey, idx).ok().map(|a| (a.address, a.public_key)),
        |batch: Vec<FetchAddressInfo>| {
            if let Some(top) = batch.iter().map(|a| a.index.max(0) as u32).max() {
                highest.set(highest.get().max(top));
            }
            fetch(batch)
        },
        history,
    ).await;
    if highest.get() >= ceiling {
        log::warn!("   ⚠️  Scan stopped at its ceiling ({}) — indices above it were not checked", ceiling);
        result.unchecked_ranges.push((ceiling.saturating_add(1), u32::MAX));
    }
    result
}

/// Stop after this many consecutive indices that could not be read: the indexer is
/// down, and carrying on would only lengthen an answer that is already "incomplete".
const MAX_CONSECUTIVE_UNCHECKED: u32 = 40;

/// The gap-limit scan, with its three dependencies injected so it can be tested
/// without the network (B5-T1-P4 `P4-A9`/`A10`).
///
/// ⛔ An index that could not be read — its address's UTXO fetch failed, or its
/// history check returned `Err` — is **unchecked**, never "unused": it is recorded in
/// `unchecked_ranges` and the gap counter does not move over it. It used to count as
/// unused (`fetch_all_utxos(..).unwrap_or_default()`, `if let Ok(true) = ..`), so an
/// indexer outage ended the scan with "gap limit reached" and "N found".
///
/// `method` labels every hit (`RecoveredAddress::derivation_method`). With
/// `no_gap_through = Some(n)` (B5-T1-P6), indices `0..=n` are all read whatever they
/// hold — no history call, no gap counting — and the gap limit applies only above `n`.
pub(crate) async fn scan_indices<D, F, FF, H, HF>(
    options: &RecoveryOptions,
    method: &str,
    no_gap_through: Option<u32>,
    derive: D,
    fetch: F,
    history: H,
) -> RecoveryResult
where
    D: Fn(u32) -> Option<(String, String)>,
    F: Fn(Vec<FetchAddressInfo>) -> FF,
    FF: std::future::Future<Output = std::result::Result<(Vec<crate::utxo_fetcher::UTXO>, Vec<String>), String>>,
    H: Fn(String) -> HF,
    HF: std::future::Future<Output = std::result::Result<bool, String>>,
{
    let mut recovered_addresses = Vec::new();
    let mut current_index = options.start_index;
    let mut unused_count: u32 = 0;
    let mut consecutive_unchecked: u32 = 0;
    let mut unchecked: Vec<u32> = Vec::new();
    let mut total_utxos: u32 = 0;
    let mut total_balance = 0i64;

    const BATCH_SIZE: u32 = 20;

    'scan: loop {
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

        info!("   🔍 Checking indices {}..{} ({} addresses)...", batch_start, batch_end - 1, batch_end - batch_start);

        // 1. Derive all addresses in this batch
        let batch: Vec<(u32, Option<(String, String)>)> =
            (batch_start..batch_end).map(|idx| (idx, derive(idx))).collect();

        // 2. Bulk fetch UTXOs for all addresses in this batch
        let fetch_addresses: Vec<FetchAddressInfo> = batch.iter()
            .filter_map(|(idx, a)| a.as_ref().map(|(address, public_key)| FetchAddressInfo {
                index: *idx as i32,
                address: address.clone(),
                public_key: public_key.clone(),
                used: false,
                balance: 0,
            }))
            .collect();
        let (api_utxos, failed) = match fetch(fetch_addresses.clone()).await {
            Ok(r) => r,
            Err(e) => {
                log::warn!("   ⚠️  Indices {}..{} could not be read: {}", batch_start, batch_end - 1, e);
                (Vec::new(), fetch_addresses.iter().map(|a| a.address.clone()).collect())
            }
        };

        // 3. Process results per-index
        for (idx, addr) in &batch {
            let idx = *idx;
            let Some((address, public_key)) = addr else {
                // Underivable index: nothing can be held there.
                if no_gap_through.map_or(false, |n| idx <= n) { continue; }
                unused_count += 1;
                if unused_count >= options.gap_limit { break 'scan; }
                continue;
            };

            if failed.contains(address) {
                unchecked.push(idx);
                consecutive_unchecked += 1;
            } else {
                let mut utxos: Vec<_> = api_utxos.iter()
                    .filter(|u| u.address_index == idx as i32)
                    .cloned()
                    .collect();
                if !utxos.is_empty() {
                    let address_balance: i64 = utxos.iter().map(|u| u.satoshis).sum();
                    let utxo_count = utxos.len() as u32;
                    info!("   ✅ Found {} UTXO(s) on {} address {} (index {}, {} satoshis)",
                          utxo_count, method, address, idx, address_balance);
                    for u in &mut utxos { u.address_index = idx as i32; }
                    recovered_addresses.push(RecoveredAddress {
                        index: idx,
                        address: address.clone(),
                        public_key: public_key.clone(),
                        derivation_method: method.to_string(),
                        has_utxos: true,
                        balance: address_balance,
                        utxos,
                    });
                    total_utxos += utxo_count;
                    total_balance += address_balance;
                    unused_count = 0;
                    consecutive_unchecked = 0;
                } else if no_gap_through.map_or(false, |n| idx <= n) {
                    // Below the mark: read, never counted toward the gap.
                    consecutive_unchecked = 0;
                } else {
                    // No UTXOs — check tx history (spent addresses reset the gap counter)
                    match history(address.clone()).await {
                        Ok(true) => {
                            info!("   📜 {} address {} has tx history (spent), resetting gap counter", method, address);
                            unused_count = 0;
                            consecutive_unchecked = 0;
                        }
                        Ok(false) => {
                            unused_count += 1;
                            consecutive_unchecked = 0;
                        }
                        Err(e) => {
                            log::warn!("   ⚠️  History of index {} could not be read: {}", idx, e);
                            unchecked.push(idx);
                            consecutive_unchecked += 1;
                        }
                    }
                }
            }

            if unused_count >= options.gap_limit {
                info!("   📊 Gap limit reached ({} unused addresses), stopping recovery", options.gap_limit);
                break 'scan;
            }
            if consecutive_unchecked >= MAX_CONSECUTIVE_UNCHECKED {
                log::warn!("   ⚠️  {} consecutive indices could not be read — stopping; the scan is INCOMPLETE",
                           consecutive_unchecked);
                break 'scan;
            }
        }

        current_index = batch_end;
    }

    RecoveryResult {
        addresses_found: recovered_addresses.len() as u32,
        utxos_found: total_utxos,
        total_balance,
        addresses: recovered_addresses,
        unchecked_ranges: to_ranges(&unchecked),
    }
}

/// Sorted indices → inclusive `(from, to)` runs.
fn to_ranges(indices: &[u32]) -> Vec<(u32, u32)> {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for &i in indices {
        match ranges.last_mut() {
            Some((_, to)) if *to + 1 == i => *to = i,
            _ => ranges.push((i, i)),
        }
    }
    ranges
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

/// Derive address using BRC-42 (`2-receive address-{index}`, counterparty = self)
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
pub fn pubkey_to_address(pubkey_bytes: &[u8]) -> Result<String, rusqlite::Error> {
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
    /// Sweepable outputs only. Token-reserved outputs are **not** here — see `token_reserved`.
    pub utxos: Vec<ExternalUTXO>,
    /// Sum of `utxos` only. Deliberately excludes `token_reserved`.
    pub total_balance: i64,
    pub addresses_scanned: u32,
    /// Outputs found on the external wallet that must not be swept as value
    /// (1-satoshi token carriers). Reported so the caller can tell the user they
    /// exist and were left behind, rather than silently destroying or hiding them.
    pub token_reserved: Vec<ExternalUTXO>,
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
    let mut scanned: Vec<ExternalUTXO> = Vec::new();
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
                        let external = ExternalUTXO {
                            txid: utxo.txid.clone(),
                            vout: utxo.vout,
                            satoshis: utxo.satoshis,
                            script_hex: utxo.script.clone(),
                            private_key: privkey_bytes.clone(),
                            address: address.clone(),
                            chain_index: chain_idx,
                            address_index: addr_index,
                        };

                        scanned.push(external);
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

    let (sweepable, token_reserved, total_balance) = split_token_reserved(scanned);

    info!("   External scan complete: {} sweepable UTXOs, {} sats, {} token-reserved withheld, {} addresses scanned",
          sweepable.len(), total_balance, token_reserved.len(), total_scanned);
    for u in &token_reserved {
        info!("   🔒 Withholding token-reserved output {}:{} ({} sat) from sweep",
              &u.txid[..16.min(u.txid.len())], u.vout, u.satoshis);
    }

    Ok(ExternalScanResult {
        utxos: sweepable,
        total_balance,
        addresses_scanned: total_scanned,
        token_reserved,
    })
}

/// Split a scan's findings into (sweepable, token-reserved, sweepable total).
///
/// ⛔ `total_balance` counts the sweepable half **only**. Summing a token-reserved
/// output into the headline balance would present it as spendable money immediately
/// before a flow whose entire purpose is to spend everything it was shown.
///
/// Pure and separate from `scan_external_wallet` so this can be tested without a
/// network round-trip — the scan itself is `async` and hits WhatsOnChain.
fn split_token_reserved(scanned: Vec<ExternalUTXO>) -> (Vec<ExternalUTXO>, Vec<ExternalUTXO>, i64) {
    let mut sweepable = Vec::new();
    let mut token_reserved = Vec::new();
    let mut total_balance: i64 = 0;

    for utxo in scanned {
        if crate::utxo_fetcher::is_token_reserved_value(utxo.satoshis) {
            token_reserved.push(utxo);
        } else {
            total_balance += utxo.satoshis;
            sweepable.push(utxo);
        }
    }

    (sweepable, token_reserved, total_balance)
}

/// B5-T1-P5 (`R-NOSPEND` b): split scanned coins by their **observed** script.
/// `observed` maps `(txid, vout)` to the script read from the verified parent
/// (`ingest::observe_output`). A coin is swept only when that script is exactly the
/// 25-byte P2PKH template and it holds more than 1 satoshi; its `script_hex` is
/// replaced by the observed bytes (the scan's value was assumed from the address).
/// Everything else is withheld with a reason — a multi-sat inscription is not money
/// just because it is not 1 sat.
pub fn split_by_observed_script(
    utxos: Vec<ExternalUTXO>,
    observed: &std::collections::HashMap<(String, u32), Vec<u8>>,
) -> (Vec<ExternalUTXO>, Vec<(ExternalUTXO, &'static str)>) {
    let mut sweep = Vec::new();
    let mut withheld = Vec::new();
    for mut u in utxos {
        match observed.get(&(u.txid.clone(), u.vout)) {
            None => withheld.push((u, "not_observed")),
            Some(s) if !crate::utxo_fetcher::is_p2pkh_script(s) => withheld.push((u, "not_plain_payment")),
            Some(_) if crate::utxo_fetcher::is_token_reserved_value(u.satoshis) => withheld.push((u, "one_sat")),
            Some(s) => { u.script_hex = hex::encode(s); sweep.push(u); }
        }
    }
    (sweep, withheld)
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

    // ⛔ Second line of defence. `scan_external_wallet` already withholds these, but
    // this function is `pub` and takes an arbitrary slice, so it must not depend on
    // its caller having filtered. The batching below sums a batch into ONE output —
    // a 1-sat carrier swept in with ordinary coins is annihilated, and the existing
    // dust check further down only inspects the resulting OUTPUT, never the inputs.
    // Borrowed, not cloned — `ExternalUTXO` carries private key bytes and there is no
    // reason to make a second copy of them in memory to run a filter.
    // B5-T1-P5: and only a coin whose script is exactly the P2PKH template — the value
    // floor alone sweeps a multi-sat inscription (`R-NOSPEND` b).
    let sweepable: Vec<&ExternalUTXO> = utxos.iter()
        .filter(|u| !crate::utxo_fetcher::is_token_reserved_value(u.satoshis))
        .filter(|u| hex::decode(&u.script_hex).map_or(false, |s| crate::utxo_fetcher::is_p2pkh_script(&s)))
        .collect();
    let withheld = utxos.len() - sweepable.len();
    if withheld > 0 {
        log::info!("   🔒 Sweep: withheld {} token-reserved output(s) from batching", withheld);
    }
    if sweepable.is_empty() {
        return Err("No sweepable UTXOs (all outputs are token-reserved)".to_string());
    }

    // Decode destination address to get pubkey hash for P2PKH locking script
    let dest_script = address_to_p2pkh_script(destination_address)?;

    let mut results = Vec::new();

    // Process UTXOs in batches
    for batch in sweepable.chunks(max_inputs_per_tx) {
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

/// Convert a Base58Check BSV mainnet address to a P2PKH locking script (25 bytes).
///
/// Validates:
/// 1. Base58 decoding succeeds
/// 2. Payload length is exactly 25 bytes (1 version + 20 hash + 4 checksum)
/// 3. Double-SHA256 checksum matches the trailing 4 bytes
/// 4. Version byte is `0x00` (BSV mainnet P2PKH)
///
/// Rejects testnet addresses (`0x6f`), P2SH addresses (`0x05`), and any other
/// non-mainnet-P2PKH version byte to prevent silently locking funds to an
/// uncontrolled hash160. This is the single source of truth used by every
/// address-derived locking script construction in the wallet — both trusted
/// (e.g., `HODOS_FEE_ADDRESS`, change addresses) and untrusted (user-supplied
/// payment destinations from `create_action`, sweep destinations).
///
/// Phase 2 Step 3b.0 (2026-05-29): unified the previous unchecked
/// `handlers::address_to_script` and the partially-checked
/// `recovery::address_to_p2pkh_script` into this function.
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

    // BSV mainnet P2PKH only. Reject testnet (0x6f), P2SH (0x05), and any
    // other version byte to prevent silently building a P2PKH locking script
    // from a hash160 that the user never controlled.
    if decoded[0] != 0x00 {
        return Err(format!(
            "Unsupported address version byte: 0x{:02x} (only mainnet P2PKH 0x00 is supported)",
            decoded[0]
        ));
    }

    // Extract 20-byte pubkey hash (skip version byte)
    let pubkey_hash = &decoded[1..21];

    let script = Script::p2pkh_locking_script(pubkey_hash)
        .map_err(|e| format!("Failed to build P2PKH script: {}", e))?;

    Ok(script.bytes)
}

#[cfg(test)]
mod address_to_p2pkh_script_tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn build_address(version_byte: u8, hash160: &[u8; 20]) -> String {
        let mut payload = vec![version_byte];
        payload.extend_from_slice(hash160);
        let checksum = Sha256::digest(&Sha256::digest(&payload));
        payload.extend_from_slice(&checksum[..4]);
        bs58::encode(&payload).into_string()
    }

    #[test]
    fn known_mainnet_address_to_script() {
        // Hodos service fee address — a known-good mainnet P2PKH address.
        let address = "1Q1A2rq6trBdptd3t6n53vB79mRN6JHEFT";
        let script = address_to_p2pkh_script(address).expect("known address must decode");
        assert_eq!(script.len(), 25);
        assert_eq!(script[0], 0x76); // OP_DUP
        assert_eq!(script[1], 0xa9); // OP_HASH160
        assert_eq!(script[2], 0x14); // push 20
        assert_eq!(script[23], 0x88); // OP_EQUALVERIFY
        assert_eq!(script[24], 0xac); // OP_CHECKSIG
    }

    #[test]
    fn synthetic_mainnet_address_round_trip() {
        let hash160 = [0xaau8; 20];
        let address = build_address(0x00, &hash160);
        let script = address_to_p2pkh_script(&address).unwrap();
        // The 20-byte pubkey hash must round-trip into the script.
        assert_eq!(&script[3..23], &hash160[..]);
    }

    #[test]
    fn rejects_malformed_base58() {
        // '0', 'O', 'I', 'l' are not valid Base58 characters.
        let result = address_to_p2pkh_script("0OIl");
        assert!(result.is_err(), "malformed Base58 must error");
    }

    #[test]
    fn rejects_empty_string() {
        let result = address_to_p2pkh_script("");
        assert!(result.is_err(), "empty address must error");
    }

    #[test]
    fn rejects_bad_checksum() {
        let hash160 = [0xbbu8; 20];
        let mut payload = vec![0x00u8];
        payload.extend_from_slice(&hash160);
        let mut checksum = Sha256::digest(&Sha256::digest(&payload))[..4].to_vec();
        checksum[0] ^= 0xff; // corrupt
        payload.extend_from_slice(&checksum);
        let address = bs58::encode(&payload).into_string();
        let err = address_to_p2pkh_script(&address).unwrap_err();
        assert!(err.contains("checksum"), "expected checksum error, got: {}", err);
    }

    #[test]
    fn rejects_testnet_version_byte() {
        let hash160 = [0xccu8; 20];
        let address = build_address(0x6f, &hash160);
        let err = address_to_p2pkh_script(&address).unwrap_err();
        assert!(err.contains("0x6f"), "expected 0x6f-version error, got: {}", err);
    }

    #[test]
    fn rejects_p2sh_version_byte() {
        let hash160 = [0xddu8; 20];
        let address = build_address(0x05, &hash160);
        let err = address_to_p2pkh_script(&address).unwrap_err();
        assert!(err.contains("0x05"), "expected 0x05-version error, got: {}", err);
    }

    #[test]
    fn rejects_short_address() {
        // Encoding only 20 bytes — decode succeeds but length check fires.
        let short = bs58::encode(&[0u8; 20]).into_string();
        let result = address_to_p2pkh_script(&short);
        assert!(result.is_err(), "20-byte payload must error on length check");
    }
}

#[cfg(test)]
mod token_reserved_sweep_tests {
    use super::*;

    /// A well-formed mainnet P2PKH address — the sweep destination.
    const DEST: &str = "1Q1A2rq6trBdptd3t6n53vB79mRN6JHEFT";

    /// Distinctive, well-formed 32-byte txids so the carrier can be searched for
    /// in the serialised transaction.
    const CARRIER_TXID: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

    fn utxo(txid: &str, satoshis: i64) -> ExternalUTXO {
        // 25-byte P2PKH locking script, hex-encoded.
        let mut script = vec![0x76u8, 0xa9, 0x14];
        script.extend_from_slice(&[0xab; 20]);
        script.extend_from_slice(&[0x88, 0xac]);

        ExternalUTXO {
            txid: txid.to_string(),
            vout: 0,
            satoshis,
            script_hex: hex::encode(&script),
            private_key: vec![0x11; 32],
            address: DEST.to_string(),
            chain_index: 0,
            address_index: 0,
        }
    }

    /// Three ordinary outputs plus one 1-satoshi token carrier.
    fn staged() -> Vec<ExternalUTXO> {
        vec![
            utxo(&"aa".repeat(32), 50_000),
            utxo(CARRIER_TXID, 1),
            utxo(&"bb".repeat(32), 30_000),
            utxo(&"cc".repeat(32), 20_000),
        ]
    }

    /// `P8-A4` — the scan must not sum a token carrier into the headline balance,
    /// and must surface it separately rather than dropping it silently.
    #[test]
    fn scan_split_excludes_token_carrier_from_balance_and_reports_it() {
        let (sweepable, token_reserved, total) = split_token_reserved(staged());

        assert_eq!(sweepable.len(), 3, "only the ordinary outputs are sweepable");
        assert_eq!(total, 100_000, "balance sums the sweepable half ONLY");
        assert_eq!(token_reserved.len(), 1, "the carrier is reported, not discarded");
        assert_eq!(token_reserved[0].txid, CARRIER_TXID);
        assert!(!sweepable.iter().any(|u| u.txid == CARRIER_TXID));
    }

    /// `P8-A4` negative control. Unguarded, the same input is 4 outputs worth
    /// 100,001 sats — the carrier counted as money and queued to be spent. If these
    /// numbers ever match the guarded ones, the test above is vacuous.
    #[test]
    fn without_the_floor_the_carrier_would_be_swept_and_counted() {
        let scanned = staged();
        let unguarded_total: i64 = scanned.iter().map(|u| u.satoshis).sum();

        assert_eq!(scanned.len(), 4);
        assert_eq!(unguarded_total, 100_001,
            "unguarded the carrier is money; 100_001 must differ from the guarded 100_000");
    }

    /// `P8-A3` — the destructive step. Batching sums a batch into ONE output, so a
    /// carrier reaching this function is annihilated. Asserted on the SERIALISED
    /// transaction: its input count, and the absence of the carrier's outpoint.
    #[test]
    fn sweep_transaction_inputs_exclude_the_token_carrier() {
        let txs = build_sweep_transactions(&staged(), DEST, 1000, 50)
            .expect("sweep should build");
        assert_eq!(txs.len(), 1, "four inputs fit in one batch");

        let raw = hex::decode(&txs[0].0).expect("valid tx hex");

        // Wire format: 4-byte version, then the input-count varint (1 byte below 253).
        assert_eq!(raw[4], 3, "the transaction spends 3 inputs, not 4");

        // Outpoint txids are serialised little-endian — the reverse of the hex form.
        let mut carrier_le = hex::decode(CARRIER_TXID).unwrap();
        carrier_le.reverse();
        assert!(!raw.windows(32).any(|w| w == carrier_le.as_slice()),
            "the carrier's outpoint must not appear anywhere in the transaction");

        // Control on the assertion itself: an output that SHOULD be spent is present,
        // proving the search above can actually find an outpoint when one is there.
        let mut ordinary_le = hex::decode("aa".repeat(32)).unwrap();
        ordinary_le.reverse();
        assert!(raw.windows(32).any(|w| w == ordinary_le.as_slice()),
            "an ordinary input IS present — otherwise the search proves nothing");
    }

    /// `P8-A3` negative control: a slice of nothing but carriers must refuse to
    /// build, rather than quietly producing a sweep that consumes them.
    #[test]
    fn sweep_of_only_carriers_refuses_rather_than_spending_them() {
        let only_carriers = vec![utxo(&"ee".repeat(32), 1), utxo(&"ff".repeat(32), 1)];
        let result = build_sweep_transactions(&only_carriers, DEST, 1000, 50);
        assert!(result.is_err(), "must refuse, not sweep");
        assert!(result.unwrap_err().contains("token-reserved"));
    }

    /// The floor is a floor. 2 satoshis is ordinary value and must still sweep.
    #[test]
    fn two_sat_output_still_sweeps() {
        let (sweepable, reserved, total) = split_token_reserved(vec![
            utxo(&"11".repeat(32), 2),
            utxo(&"22".repeat(32), 1),
        ]);
        assert_eq!(sweepable.len(), 1);
        assert_eq!(sweepable[0].satoshis, 2);
        assert_eq!(total, 2);
        assert_eq!(reserved.len(), 1);
    }

    // ---- B5-T1-P5 `P5-A5b` (R-NOSPEND b) — the sweep classifies by script ----------

    const INSCRIPTION_TXID: &str = "9999999999999999999999999999999999999999999999999999999999999999";

    fn p2pkh() -> Vec<u8> {
        let mut s = vec![0x76u8, 0xa9, 0x14];
        s.extend_from_slice(&[0xab; 20]);
        s.extend_from_slice(&[0x88, 0xac]);
        s
    }

    /// P2PKH + `ord` envelope — a multi-sat inscription the 1-sat floor cannot see.
    fn inscription() -> Vec<u8> {
        let mut s = p2pkh();
        s.extend(hex::decode("0063036f72645109").unwrap());
        s.extend(b"image/png");
        s.extend([0x00, 0x02, 0xbe, 0xef, 0x68]);
        s
    }

    fn scanned_with_inscription() -> (Vec<ExternalUTXO>, std::collections::HashMap<(String, u32), Vec<u8>>) {
        let coins = vec![
            utxo(&"aa".repeat(32), 50_000),
            utxo(INSCRIPTION_TXID, 10), // >= 2 sats: the value floor passes it
            utxo(&"bb".repeat(32), 30_000),
            utxo(&"cc".repeat(32), 20_000), // not observed
        ];
        let mut observed = std::collections::HashMap::new();
        observed.insert(("aa".repeat(32), 0), p2pkh());
        observed.insert((INSCRIPTION_TXID.to_string(), 0), inscription());
        observed.insert(("bb".repeat(32), 0), p2pkh());
        (coins, observed)
    }

    /// GREEN: the scan's coins are split by their observed script — the inscription and
    /// the unobserved coin are withheld and named; the sweep's serialised inputs hold
    /// only the two plain payments.
    #[test]
    fn p5_a5b_sweep_excludes_and_reports_a_multi_sat_inscription() {
        let (coins, observed) = scanned_with_inscription();
        let (sweep, withheld) = split_by_observed_script(coins, &observed);
        let reasons: Vec<(String, &str)> = withheld.iter().map(|(u, r)| (u.txid.clone(), *r)).collect();
        assert_eq!(reasons, vec![(INSCRIPTION_TXID.to_string(), "not_plain_payment"), ("cc".repeat(32), "not_observed")]);
        assert_eq!(sweep.iter().map(|u| u.satoshis).sum::<i64>(), 80_000, "headline = plain coins only");

        let txs = build_sweep_transactions(&sweep, DEST, 1000, 50).expect("sweep builds");
        let raw = hex::decode(&txs[0].0).unwrap();
        assert_eq!(raw[4], 2, "two inputs");
        let mut ins_le = hex::decode(INSCRIPTION_TXID).unwrap();
        ins_le.reverse();
        assert!(!raw.windows(32).any(|w| w == ins_le.as_slice()), "the inscription is not an input");
    }

    /// Second layer: `build_sweep_transactions` is `pub` and must not trust its caller.
    /// Handed the unfiltered slice — with the inscription carrying its observed script —
    /// it still leaves the inscription out.
    #[test]
    fn p5_a5b_sweep_builder_refuses_a_non_template_script_itself() {
        let (mut coins, observed) = scanned_with_inscription();
        for c in coins.iter_mut() {
            if let Some(s) = observed.get(&(c.txid.clone(), c.vout)) { c.script_hex = hex::encode(s); }
        }
        let txs = build_sweep_transactions(&coins, DEST, 1000, 50).expect("sweep builds");
        let raw = hex::decode(&txs[0].0).unwrap();
        let mut ins_le = hex::decode(INSCRIPTION_TXID).unwrap();
        ins_le.reverse();
        assert!(!raw.windows(32).any(|w| w == ins_le.as_slice()), "the builder swept the inscription");
        assert_eq!(raw[4], 3, "the three P2PKH-scripted coins (one unobserved, assumed script) are inputs");
    }
}

/// B5-T1-P4 `P4-A9` / `P4-A10`: the gap-limit scan with fake fetchers.
#[cfg(test)]
mod scan_tests {
    use super::*;
    use crate::utxo_fetcher::UTXO;

    fn coin(idx: u32) -> UTXO {
        UTXO {
            txid: format!("{:064x}", idx),
            vout: 0,
            satoshis: 1000,
            script: String::new(),
            address_index: idx as i32,
            custom_instructions: None,
            confirmed: true,
        }
    }

    fn opts() -> RecoveryOptions {
        RecoveryOptions { mnemonic: String::new(), gap_limit: 20, start_index: 0, max_index: None }
    }

    fn derive(idx: u32) -> Option<(String, String)> {
        Some((format!("addr{}", idx), "pk".into()))
    }

    fn index_of(address: &str) -> u32 {
        address.trim_start_matches("addr").parse().unwrap()
    }

    /// `P4-A9`: the UTXO fetch for batch 1 (indices 20–39) fails. The scan reports
    /// that range as unchecked, does not count it toward the gap, and still finds the
    /// coin at index 45 (= 1·20 + 25), past the failed batch.
    #[tokio::test]
    async fn p4_a9_failed_batch_is_reported_and_not_counted_as_unused() {
        let holders = [19u32, 45];
        let result = scan_indices(
            &opts(),
            "BIP32",
            None,
            derive,
            |batch: Vec<FetchAddressInfo>| async move {
                if batch.first().map(|a| a.index) == Some(20) {
                    return Err("indexer down".to_string());
                }
                Ok((batch.iter().filter(|a| holders.contains(&(a.index as u32))).map(|a| coin(a.index as u32)).collect(),
                    Vec::new()))
            },
            |_| async { Ok(false) },
        ).await;

        assert_eq!(result.unchecked_ranges, vec![(20, 39)]);
        assert!(!result.complete());
        let found: Vec<u32> = result.addresses.iter().map(|a| a.index).collect();
        assert_eq!(found, vec![19, 45], "the coin after the failed batch is found");
        assert!(result.incomplete_message().unwrap().contains("20–39"));
    }

    /// `P4-A9`: everything fails ⇒ the scan stops (bounded) and says so; it never
    /// returns "complete, 0 found".
    #[tokio::test]
    async fn p4_a9_total_outage_is_incomplete_not_empty() {
        let result = scan_indices(
            &opts(),
            "BIP32",
            None,
            derive,
            |_batch: Vec<FetchAddressInfo>| async { Err::<(Vec<UTXO>, Vec<String>), _>("down".to_string()) },
            |_| async { Ok(false) },
        ).await;
        assert_eq!(result.unchecked_ranges, vec![(0, 39)]);
        assert!(!result.complete());
        assert_eq!(result.addresses_found, 0);
    }

    /// `P4-A10`: history checks fail (`Err`) for indices 5–24 and a coin sits at 30.
    /// The unknown indices do not reach the gap limit, so the scan reaches 30.
    #[tokio::test]
    async fn p4_a10_history_error_is_unknown_not_unused() {
        let result = scan_indices(
            &opts(),
            "BIP32",
            None,
            derive,
            |batch: Vec<FetchAddressInfo>| async move {
                Ok((batch.iter().filter(|a| a.index == 0 || a.index == 30).map(|a| coin(a.index as u32)).collect(),
                    Vec::new()))
            },
            |address: String| async move {
                let i = index_of(&address);
                if (5..=24).contains(&i) { Err("history API down".to_string()) } else { Ok(false) }
            },
        ).await;

        let found: Vec<u32> = result.addresses.iter().map(|a| a.index).collect();
        assert_eq!(found, vec![0, 30], "index 30 is reached");
        assert_eq!(result.unchecked_ranges, vec![(5, 24)]);
    }

    /// A partially failed batch: only the addresses the fetcher names as failed are
    /// unchecked; the rest of the batch is read normally.
    #[tokio::test]
    async fn partially_failed_batch_marks_only_the_failed_addresses() {
        let result = scan_indices(
            &opts(),
            "BIP32",
            None,
            derive,
            |batch: Vec<FetchAddressInfo>| async move {
                let failed = batch.iter().filter(|a| a.index == 3).map(|a| a.address.clone()).collect();
                Ok((Vec::new(), failed))
            },
            |_| async { Ok(false) },
        ).await;
        assert_eq!(result.unchecked_ranges, vec![(3, 3)]);
    }
}
