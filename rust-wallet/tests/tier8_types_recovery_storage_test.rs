//! ============================================================================
//! TIER 8: Transaction Types, Recovery Helpers, Storage CRUD
//! ============================================================================
//!
//! Tests transaction serialization, BIP32 path derivation, address conversion,
//! sweep transaction building, and filesystem-based storage operations.
//!
//! Zero modifications to wallet code.

use std::sync::atomic::{AtomicU32, Ordering};

// Wallet crate imports
use hodos_wallet::transaction::{
    Transaction, TxInput, TxOutput, OutPoint, Script,
    extract_input_outpoints,
};
use hodos_wallet::recovery::{
    derive_key_at_path, derive_address_at_path, address_to_p2pkh_script,
    ExternalWalletConfig, ExternalUTXO, build_sweep_transactions,
};
use hodos_wallet::action_storage::{
    ActionStorage, StoredAction, ActionStatus,
};
use hodos_wallet::json_storage::JsonStorage;

// ── check! macro ──────────────────────────────────────────────────────

static PASS: AtomicU32 = AtomicU32::new(0);
static FAIL: AtomicU32 = AtomicU32::new(0);

macro_rules! check {
    ($id:expr, $label:expr, $block:expr) => {{
        let result: Result<(), String> = (|| $block)();
        match result {
            Ok(()) => {
                PASS.fetch_add(1, Ordering::Relaxed);
                eprintln!("  PASS  {}  {}", $id, $label);
            }
            Err(e) => {
                FAIL.fetch_add(1, Ordering::Relaxed);
                eprintln!("**FAIL  {}  {}  — {}", $id, $label, e);
            }
        }
    }};
}

// ── Well-known BIP39 test seed ────────────────────────────────────────
// Mnemonic: "abandon abandon abandon abandon abandon abandon abandon
//            abandon abandon abandon abandon about"
// Passphrase: "" (empty)
// Seed = PBKDF2-HMAC-SHA512(mnemonic, "mnemonic")
const TEST_SEED: [u8; 64] = [
    0x5e, 0xb0, 0x0b, 0xbd, 0xdc, 0xf0, 0x69, 0x08,
    0x48, 0x89, 0xa8, 0xab, 0x91, 0x55, 0x56, 0x81,
    0x65, 0xf5, 0xc4, 0x53, 0xcc, 0xb8, 0x5e, 0x70,
    0x81, 0x1a, 0xae, 0xd6, 0xf6, 0xda, 0x5f, 0xc1,
    0x9a, 0x5a, 0xc4, 0x0b, 0x38, 0x9c, 0xd3, 0x70,
    0xd0, 0x86, 0x20, 0x6d, 0xec, 0x8a, 0xa6, 0xc4,
    0x3d, 0xae, 0xa6, 0x69, 0x0f, 0x20, 0xad, 0x3d,
    0x8d, 0x48, 0xb2, 0xd2, 0xce, 0x9e, 0x38, 0xe4,
];

// ── Helpers ───────────────────────────────────────────────────────────

/// Create a valid ExternalUTXO with proper key-script pairing
fn make_test_utxo(txid: &str, vout: u32, satoshis: i64) -> ExternalUTXO {
    let path = &[(42, false)];
    let (address, _pubkey_hex, privkey_bytes) =
        derive_address_at_path(&TEST_SEED, path).unwrap();
    let script_bytes = address_to_p2pkh_script(&address).unwrap();
    let script_hex = hex::encode(&script_bytes);
    ExternalUTXO {
        txid: txid.to_string(),
        vout,
        satoshis,
        script_hex,
        private_key: privkey_bytes,
        address,
        chain_index: 0,
        address_index: 42,
    }
}

/// Create a StoredAction for testing
fn make_test_action(
    txid: &str, ref_num: &str, satoshis: i64,
    labels: Vec<String>, timestamp: i64,
) -> StoredAction {
    StoredAction {
        txid: txid.to_string(),
        reference_number: ref_num.to_string(),
        raw_tx: "01000000000000000000".to_string(),
        description: Some("test".to_string()),
        labels,
        status: ActionStatus::Created,
        is_outgoing: true,
        satoshis,
        timestamp,
        block_height: None,
        confirmations: 0,
        version: 1,
        lock_time: 0,
        inputs: vec![],
        outputs: vec![],
        price_usd_cents: None,
    }
}

// ============================================================================
// [1/6]  Transaction Type Serialization
// ============================================================================

#[test]
fn t8_01_transaction_types() {
    eprintln!("\n=== TIER 8 [1/6] Transaction Type Serialization ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    check!("txtype/01", "outpoint-new-and-display", {
        let txid = "a".repeat(64);
        let op = OutPoint::new(&txid, 7);
        let display = format!("{}", op);
        if display != format!("{}:7", txid) {
            return Err(format!("Display mismatch: {}", display));
        }
        Ok(())
    });

    check!("txtype/02", "outpoint-txid-bytes-reversed", {
        // Use a recognizable pattern so we can verify byte reversal
        let mut txid_hex = String::new();
        for i in 0u8..32 { txid_hex.push_str(&format!("{:02x}", i)); }
        let op = OutPoint::new(&txid_hex, 0);
        let bytes = op.txid_bytes().map_err(|e| format!("{}", e))?;
        if bytes.len() != 32 {
            return Err(format!("expected 32 bytes, got {}", bytes.len()));
        }
        // Original: [00, 01, 02, ..., 31]. Reversed: [31, 30, ..., 00]
        if bytes[0] != 31 || bytes[31] != 0 {
            return Err(format!("reversal wrong: first={}, last={}", bytes[0], bytes[31]));
        }
        Ok(())
    });

    check!("txtype/03", "outpoint-serialize-36-bytes", {
        let txid = "ff".repeat(32);
        let op = OutPoint::new(&txid, 42);
        let ser = op.serialize().map_err(|e| format!("{}", e))?;
        if ser.len() != 36 {
            return Err(format!("expected 36 bytes, got {}", ser.len()));
        }
        let vout = u32::from_le_bytes([ser[32], ser[33], ser[34], ser[35]]);
        if vout != 42 {
            return Err(format!("vout should be 42, got {}", vout));
        }
        Ok(())
    });

    check!("txtype/04", "outpoint-invalid-hex-txid", {
        let op = OutPoint::new("xyz_not_hex", 0);
        let result = op.txid_bytes();
        if result.is_ok() {
            return Err("invalid hex txid should error".into());
        }
        Ok(())
    });

    check!("txtype/05", "outpoint-max-vout", {
        let txid = "00".repeat(32);
        let op = OutPoint::new(&txid, u32::MAX);
        let ser = op.serialize().map_err(|e| format!("{}", e))?;
        let vout = u32::from_le_bytes([ser[32], ser[33], ser[34], ser[35]]);
        if vout != u32::MAX {
            return Err(format!("vout should be u32::MAX, got {}", vout));
        }
        Ok(())
    });

    check!("txtype/06", "script-from-hex-roundtrip", {
        let hex_str = "76a91489abcdefabbaabbaabbaabbaabbaabbaabbaabba88ac";
        let script = Script::from_hex(hex_str).map_err(|e| format!("{}", e))?;
        if script.to_hex() != hex_str {
            return Err("from_hex/to_hex roundtrip failed".into());
        }
        if script.to_bytes().len() != 25 {
            return Err(format!("expected 25 bytes, got {}", script.to_bytes().len()));
        }
        Ok(())
    });

    check!("txtype/07", "script-empty-and-default", {
        let s1 = Script::new();
        let s2 = Script::default();
        if !s1.bytes.is_empty() || !s2.bytes.is_empty() {
            return Err("new/default scripts should be empty".into());
        }
        Ok(())
    });

    check!("txtype/08", "script-p2pkh-locking-structure", {
        let hash = [0x42u8; 20];
        let script = Script::p2pkh_locking_script(&hash).map_err(|e| format!("{}", e))?;
        if script.bytes.len() != 25 { return Err(format!("expected 25, got {}", script.bytes.len())); }
        if script.bytes[0] != 0x76 { return Err("byte 0: OP_DUP".into()); }
        if script.bytes[1] != 0xa9 { return Err("byte 1: OP_HASH160".into()); }
        if script.bytes[2] != 0x14 { return Err("byte 2: PUSH 20".into()); }
        if &script.bytes[3..23] != &hash { return Err("hash not embedded".into()); }
        if script.bytes[23] != 0x88 { return Err("byte 23: OP_EQUALVERIFY".into()); }
        if script.bytes[24] != 0xac { return Err("byte 24: OP_CHECKSIG".into()); }
        Ok(())
    });

    check!("txtype/09", "script-p2pkh-locking-wrong-hash-len", {
        if Script::p2pkh_locking_script(&[0u8; 19]).is_ok() {
            return Err("19-byte hash should be rejected".into());
        }
        if Script::p2pkh_locking_script(&[0u8; 21]).is_ok() {
            return Err("21-byte hash should be rejected".into());
        }
        Ok(())
    });

    check!("txtype/10", "script-p2pkh-unlocking-structure", {
        let sig = vec![0x30, 0x44, 0x02, 0x20]; // 4-byte fake sig
        let pubkey = vec![0x02; 33]; // compressed pubkey
        let script = Script::p2pkh_unlocking_script(&sig, &pubkey);
        // Structure: <sig_len><sig><pubkey_len><pubkey>
        if script.bytes[0] != 4 { return Err(format!("sig push should be 4, got {}", script.bytes[0])); }
        if script.bytes[5] != 33 { return Err(format!("pubkey push should be 33, got {}", script.bytes[5])); }
        if script.bytes.len() != 1 + 4 + 1 + 33 {
            return Err(format!("total length wrong: {}", script.bytes.len()));
        }
        Ok(())
    });

    check!("txtype/11", "script-from-hex-invalid", {
        if Script::from_hex("xyz").is_ok() { return Err("invalid hex should error".into()); }
        if Script::from_hex("abc").is_ok() { return Err("odd-length hex should error".into()); }
        Ok(())
    });

    check!("txtype/12", "txoutput-serialize-value-script", {
        let out = TxOutput::new(50000, vec![0x76, 0xa9, 0x14]);
        let ser = out.serialize();
        // 8 (value) + 1 (varint 3) + 3 (script) = 12
        if ser.len() != 12 { return Err(format!("expected 12, got {}", ser.len())); }
        let val = i64::from_le_bytes(ser[0..8].try_into().unwrap());
        if val != 50000 { return Err(format!("value should be 50000, got {}", val)); }
        Ok(())
    });

    check!("txtype/13", "txoutput-from-hex-script", {
        let out = TxOutput::from_hex_script(100, "76a914").map_err(|e| format!("{}", e))?;
        if out.value != 100 { return Err("value mismatch".into()); }
        if out.script_pubkey != vec![0x76, 0xa9, 0x14] { return Err("script mismatch".into()); }
        Ok(())
    });

    check!("txtype/14", "txinput-defaults", {
        let inp = TxInput::new(OutPoint::new("a".repeat(64), 0));
        if inp.sequence != 0xFFFFFFFF { return Err("default sequence wrong".into()); }
        if !inp.script_sig.is_empty() { return Err("new input should have empty scriptSig".into()); }
        Ok(())
    });

    check!("txtype/15", "transaction-empty-serialize", {
        let tx = Transaction::new();
        let ser = tx.serialize().map_err(|e| format!("{}", e))?;
        // 4 (version) + 1 (0 inputs) + 1 (0 outputs) + 4 (locktime) = 10
        if ser.len() != 10 { return Err(format!("expected 10, got {}", ser.len())); }
        let ver = u32::from_le_bytes(ser[0..4].try_into().unwrap());
        if ver != 1 { return Err(format!("version should be 1, got {}", ver)); }
        let lock = u32::from_le_bytes(ser[6..10].try_into().unwrap());
        if lock != 0 { return Err(format!("locktime should be 0, got {}", lock)); }
        // txid is deterministic
        let txid = tx.txid().map_err(|e| format!("{}", e))?;
        if txid.len() != 64 { return Err(format!("txid should be 64 chars, got {}", txid.len())); }
        let txid2 = tx.txid().map_err(|e| format!("{}", e))?;
        if txid != txid2 { return Err("txid should be deterministic".into()); }
        Ok(())
    });

    check!("txtype/16", "serialize-extract-outpoints-roundtrip", {
        let txid_a = "aa".repeat(32);
        let txid_b = "bb".repeat(32);
        let mut tx = Transaction::new();
        tx.add_input(TxInput::new(OutPoint::new(&txid_a, 0)));
        tx.add_input(TxInput::new(OutPoint::new(&txid_b, 3)));
        tx.add_output(TxOutput::new(1000, vec![0x76]));

        let hex_str = tx.to_hex().map_err(|e| format!("{}", e))?;
        let outpoints = extract_input_outpoints(&hex_str).map_err(|e| format!("{}", e))?;

        if outpoints.len() != 2 { return Err(format!("expected 2, got {}", outpoints.len())); }
        if outpoints[0].0 != txid_a { return Err(format!("txid_a mismatch: {}", outpoints[0].0)); }
        if outpoints[0].1 != 0 { return Err(format!("vout[0] should be 0, got {}", outpoints[0].1)); }
        if outpoints[1].0 != txid_b { return Err(format!("txid_b mismatch: {}", outpoints[1].0)); }
        if outpoints[1].1 != 3 { return Err(format!("vout[1] should be 3, got {}", outpoints[1].1)); }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 1/6: {} tests\n", after - before);
}

// ============================================================================
// [2/6]  BIP32 Path Derivation
// ============================================================================

#[test]
fn t8_02_bip32_path_derivation() {
    eprintln!("\n=== TIER 8 [2/6] BIP32 Path Derivation ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    check!("bip32/01", "empty-path-returns-master-key", {
        let key = derive_key_at_path(&TEST_SEED, &[]).map_err(|e| e)?;
        if key.len() != 32 {
            return Err(format!("master key should be 32 bytes, got {}", key.len()));
        }
        Ok(())
    });

    check!("bip32/02", "deterministic-same-path", {
        let k1 = derive_key_at_path(&TEST_SEED, &[(0, false)]).map_err(|e| e)?;
        let k2 = derive_key_at_path(&TEST_SEED, &[(0, false)]).map_err(|e| e)?;
        if k1 != k2 { return Err("same path should give same key".into()); }
        Ok(())
    });

    check!("bip32/03", "different-paths-different-keys", {
        let k0 = derive_key_at_path(&TEST_SEED, &[(0, false)]).map_err(|e| e)?;
        let k1 = derive_key_at_path(&TEST_SEED, &[(1, false)]).map_err(|e| e)?;
        if k0 == k1 { return Err("different indices should differ".into()); }
        Ok(())
    });

    check!("bip32/04", "hardened-differs-from-normal", {
        let normal = derive_key_at_path(&TEST_SEED, &[(44, false)]).map_err(|e| e)?;
        let hardened = derive_key_at_path(&TEST_SEED, &[(44, true)]).map_err(|e| e)?;
        if normal == hardened { return Err("hardened should differ from normal".into()); }
        Ok(())
    });

    check!("bip32/05", "deep-centbee-path", {
        // m/44'/0/0/0
        let key = derive_key_at_path(&TEST_SEED, &[(44, true), (0, false), (0, false), (0, false)])
            .map_err(|e| e)?;
        if key.len() != 32 { return Err(format!("should be 32 bytes, got {}", key.len())); }
        Ok(())
    });

    check!("bip32/06", "derive-address-valid-format", {
        let (addr, pubkey_hex, privkey) =
            derive_address_at_path(&TEST_SEED, &[(0, false)]).map_err(|e| e)?;
        if addr.is_empty() { return Err("address empty".into()); }
        if !addr.starts_with('1') {
            return Err(format!("mainnet P2PKH should start with '1', got: {}", &addr[..1]));
        }
        if pubkey_hex.len() != 66 {
            return Err(format!("compressed pubkey hex should be 66 chars, got {}", pubkey_hex.len()));
        }
        if privkey.len() != 32 { return Err(format!("privkey should be 32, got {}", privkey.len())); }
        Ok(())
    });

    check!("bip32/07", "address-privkey-matches-key-fn", {
        let path = &[(5, false)];
        let key_only = derive_key_at_path(&TEST_SEED, path).map_err(|e| e)?;
        let (_, _, privkey) = derive_address_at_path(&TEST_SEED, path).map_err(|e| e)?;
        if key_only != privkey { return Err("privkeys should match".into()); }
        Ok(())
    });

    check!("bip32/08", "external-wallet-config-centbee", {
        let cfg = ExternalWalletConfig::centbee();
        if cfg.name != "centbee" { return Err("name mismatch".into()); }
        if cfg.chains.len() != 2 { return Err(format!("expected 2 chains, got {}", cfg.chains.len())); }
        if cfg.chain_labels != vec!["receive", "change"] {
            return Err("labels mismatch".into());
        }
        // Receive: m/44'/0/0
        if cfg.chains[0] != vec![(44, true), (0, false), (0, false)] {
            return Err("receive chain path wrong".into());
        }
        // Change: m/44'/0/1
        if cfg.chains[1] != vec![(44, true), (0, false), (1, false)] {
            return Err("change chain path wrong".into());
        }
        Ok(())
    });

    check!("bip32/09", "different-seeds-different-keys", {
        let alt_seed: [u8; 64] = [0xAB; 64];
        let k1 = derive_key_at_path(&TEST_SEED, &[(0, false)]).map_err(|e| e)?;
        let k2 = derive_key_at_path(&alt_seed, &[(0, false)]).map_err(|e| e)?;
        if k1 == k2 { return Err("different seeds should give different keys".into()); }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 2/6: {} tests\n", after - before);
}

// ============================================================================
// [3/6]  address_to_p2pkh_script
// ============================================================================

#[test]
fn t8_03_address_to_p2pkh() {
    eprintln!("\n=== TIER 8 [3/6] address_to_p2pkh_script ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    check!("addr/01", "derived-address-to-p2pkh-roundtrip", {
        let (address, _, _) = derive_address_at_path(&TEST_SEED, &[(0, false)]).map_err(|e| e)?;
        let script = address_to_p2pkh_script(&address).map_err(|e| e)?;
        if script.len() != 25 { return Err(format!("expected 25 bytes, got {}", script.len())); }
        // Verify P2PKH pattern
        if script[0] != 0x76 || script[1] != 0xa9 || script[2] != 0x14
            || script[23] != 0x88 || script[24] != 0xac {
            return Err("P2PKH opcode pattern mismatch".into());
        }
        Ok(())
    });

    check!("addr/02", "different-addresses-different-scripts", {
        let (a1, _, _) = derive_address_at_path(&TEST_SEED, &[(0, false)]).map_err(|e| e)?;
        let (a2, _, _) = derive_address_at_path(&TEST_SEED, &[(1, false)]).map_err(|e| e)?;
        let s1 = address_to_p2pkh_script(&a1).map_err(|e| e)?;
        let s2 = address_to_p2pkh_script(&a2).map_err(|e| e)?;
        if s1 == s2 { return Err("different addresses should give different scripts".into()); }
        Ok(())
    });

    check!("addr/03", "invalid-base58-rejected", {
        if address_to_p2pkh_script("!!!invalid!!!").is_ok() {
            return Err("invalid base58 should be rejected".into());
        }
        Ok(())
    });

    check!("addr/04", "too-short-address-rejected", {
        // "1A" is valid base58 but decodes to < 25 bytes
        if address_to_p2pkh_script("1A").is_ok() {
            return Err("too short address should be rejected".into());
        }
        Ok(())
    });

    check!("addr/05", "corrupted-checksum-rejected", {
        let (mut address, _, _) = derive_address_at_path(&TEST_SEED, &[(0, false)]).map_err(|e| e)?;
        // Corrupt last character
        let last = address.pop().unwrap();
        let replacement = if last == 'A' { 'B' } else { 'A' };
        address.push(replacement);
        if address_to_p2pkh_script(&address).is_ok() {
            return Err("corrupted checksum should be rejected".into());
        }
        Ok(())
    });

    check!("addr/06", "empty-address-rejected", {
        if address_to_p2pkh_script("").is_ok() {
            return Err("empty address should be rejected".into());
        }
        Ok(())
    });

    check!("addr/07", "script-hash-matches-address-decode", {
        let (address, _, _) = derive_address_at_path(&TEST_SEED, &[(0, false)]).map_err(|e| e)?;
        let script = address_to_p2pkh_script(&address).map_err(|e| e)?;
        // Decode address with bs58 to get the pubkey hash
        let decoded = bs58::decode(&address).into_vec().map_err(|e| format!("{}", e))?;
        let pubkey_hash = &decoded[1..21]; // skip version byte
        // Script bytes 3..23 should match the pubkey hash from the address
        if &script[3..23] != pubkey_hash {
            return Err("script pubkey hash doesn't match address decode".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 3/6: {} tests\n", after - before);
}

// ============================================================================
// [4/6]  build_sweep_transactions
// ============================================================================

#[test]
fn t8_04_sweep_transactions() {
    eprintln!("\n=== TIER 8 [4/6] build_sweep_transactions ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // Derive a destination address (different path from UTXOs)
    let (dest_addr, _, _) = derive_address_at_path(&TEST_SEED, &[(99, false)]).unwrap();

    check!("sweep/01", "single-utxo-success", {
        let txid = "aa".repeat(32);
        let utxos = vec![make_test_utxo(&txid, 0, 100_000)];
        let results = build_sweep_transactions(&utxos, &dest_addr, 1000, 100)
            .map_err(|e| e)?;
        if results.len() != 1 {
            return Err(format!("expected 1 result, got {}", results.len()));
        }
        let (raw_hex, fee, output_value) = &results[0];
        // fee + output = total input
        if *fee as i64 + *output_value != 100_000 {
            return Err(format!("fee({}) + output({}) != 100000", fee, output_value));
        }
        // Hex should decode
        let _bytes = hex::decode(raw_hex).map_err(|e| format!("bad hex: {}", e))?;
        Ok(())
    });

    check!("sweep/02", "empty-utxos-error", {
        let result = build_sweep_transactions(&[], &dest_addr, 1000, 100);
        match result {
            Err(e) if e.contains("No UTXOs") => Ok(()),
            Err(e) => Err(format!("wrong error: {}", e)),
            Ok(_) => Err("should error on empty UTXOs".into()),
        }
    });

    check!("sweep/03", "dust-utxo-error", {
        // 300 sats → fee ~200 → output ~100 < 546 dust limit
        let utxos = vec![make_test_utxo(&"cc".repeat(32), 0, 300)];
        let result = build_sweep_transactions(&utxos, &dest_addr, 1000, 100);
        match result {
            Err(e) if e.contains("dust") => Ok(()),
            Err(e) => Err(format!("wrong error: {}", e)),
            Ok(_) => Err("dust UTXO should be rejected".into()),
        }
    });

    check!("sweep/04", "multiple-utxos-batching", {
        let utxos = vec![
            make_test_utxo(&"d1".repeat(32), 0, 50_000),
            make_test_utxo(&"d2".repeat(32), 1, 50_000),
            make_test_utxo(&"d3".repeat(32), 2, 50_000),
        ];
        let results = build_sweep_transactions(&utxos, &dest_addr, 1000, 2)
            .map_err(|e| e)?;
        // max_inputs=2 → 3 UTXOs split into 2 batches (2+1)
        if results.len() != 2 {
            return Err(format!("expected 2 batches, got {}", results.len()));
        }
        // Batch 0: 2 inputs → fee+output = 100000
        if results[0].1 as i64 + results[0].2 != 100_000 {
            return Err(format!("batch 0: {}+{} != 100000", results[0].1, results[0].2));
        }
        // Batch 1: 1 input → fee+output = 50000
        if results[1].1 as i64 + results[1].2 != 50_000 {
            return Err(format!("batch 1: {}+{} != 50000", results[1].1, results[1].2));
        }
        Ok(())
    });

    check!("sweep/05", "fee-at-least-minimum", {
        let utxos = vec![make_test_utxo(&"ee".repeat(32), 0, 100_000)];
        let results = build_sweep_transactions(&utxos, &dest_addr, 1000, 100)
            .map_err(|e| e)?;
        // MIN_FEE_SATS = 200
        if results[0].1 < 200 {
            return Err(format!("fee {} below minimum 200", results[0].1));
        }
        Ok(())
    });

    check!("sweep/06", "output-hex-is-valid-transaction", {
        let utxos = vec![make_test_utxo(&"f0".repeat(32), 0, 100_000)];
        let results = build_sweep_transactions(&utxos, &dest_addr, 1000, 100)
            .map_err(|e| e)?;
        let raw_hex = &results[0].0;
        // Parse back with extract_input_outpoints to verify structure
        let outpoints = extract_input_outpoints(raw_hex).map_err(|e| format!("{}", e))?;
        if outpoints.len() != 1 {
            return Err(format!("expected 1 input in sweep tx, got {}", outpoints.len()));
        }
        if outpoints[0].0 != "f0".repeat(32) {
            return Err("input txid mismatch in sweep tx".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 4/6: {} tests\n", after - before);
}

// ============================================================================
// [5/6]  ActionStorage Filesystem CRUD
// ============================================================================

#[test]
fn t8_05_action_storage() {
    eprintln!("\n=== TIER 8 [5/6] ActionStorage CRUD ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    let temp = std::env::temp_dir();

    check!("action/01", "new-creates-empty-file", {
        let path = temp.join("tier8_action_01.json");
        let _ = std::fs::remove_file(&path);
        let storage = ActionStorage::new(path.clone()).map_err(|e| e)?;
        if storage.count() != 0 { return Err(format!("expected 0, got {}", storage.count())); }
        if !path.exists() { return Err("file should exist".into()); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("action/02", "add-and-get-by-txid", {
        let path = temp.join("tier8_action_02.json");
        let _ = std::fs::remove_file(&path);
        let mut storage = ActionStorage::new(path.clone()).map_err(|e| e)?;
        storage.add_action(make_test_action("tx_aaa", "ref_1", 5000, vec![], 100))
            .map_err(|e| e)?;
        if storage.count() != 1 { return Err(format!("count should be 1, got {}", storage.count())); }
        let a = storage.get_action("tx_aaa").ok_or("action not found")?;
        if a.satoshis != 5000 { return Err(format!("satoshis wrong: {}", a.satoshis)); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("action/03", "get-by-reference-number", {
        let path = temp.join("tier8_action_03.json");
        let _ = std::fs::remove_file(&path);
        let mut storage = ActionStorage::new(path.clone()).map_err(|e| e)?;
        storage.add_action(make_test_action("tx_bbb", "ref_magic", 7000, vec![], 200))
            .map_err(|e| e)?;
        let a = storage.get_action_by_reference("ref_magic").ok_or("not found by ref")?;
        if a.txid != "tx_bbb" { return Err(format!("txid mismatch: {}", a.txid)); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("action/04", "update-status", {
        let path = temp.join("tier8_action_04.json");
        let _ = std::fs::remove_file(&path);
        let mut storage = ActionStorage::new(path.clone()).map_err(|e| e)?;
        storage.add_action(make_test_action("tx_c", "ref_c", 1000, vec![], 300))
            .map_err(|e| e)?;
        storage.update_status("tx_c", ActionStatus::Signed).map_err(|e| e)?;
        let a = storage.get_action("tx_c").ok_or("not found")?;
        if a.status != ActionStatus::Signed { return Err("status should be Signed".into()); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("action/05", "update-confirmations-auto-status", {
        let path = temp.join("tier8_action_05.json");
        let _ = std::fs::remove_file(&path);
        let mut storage = ActionStorage::new(path.clone()).map_err(|e| e)?;
        storage.add_action(make_test_action("tx_d", "ref_d", 2000, vec![], 400))
            .map_err(|e| e)?;
        // 0 confirmations → Unconfirmed
        storage.update_confirmations("tx_d", 0, None).map_err(|e| e)?;
        let a = storage.get_action("tx_d").ok_or("not found")?;
        if a.status != ActionStatus::Unconfirmed { return Err(format!("0 conf → Unconfirmed, got {:?}", a.status)); }
        // 3 confirmations → Pending
        storage.update_confirmations("tx_d", 3, Some(800000)).map_err(|e| e)?;
        let a = storage.get_action("tx_d").ok_or("not found")?;
        if a.status != ActionStatus::Pending { return Err(format!("3 conf → Pending, got {:?}", a.status)); }
        if a.block_height != Some(800000) { return Err("block_height not set".into()); }
        // 10 confirmations → Confirmed
        storage.update_confirmations("tx_d", 10, Some(800000)).map_err(|e| e)?;
        let a = storage.get_action("tx_d").ok_or("not found")?;
        if a.status != ActionStatus::Confirmed { return Err(format!("10 conf → Confirmed, got {:?}", a.status)); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("action/06", "update-txid-rekeys", {
        let path = temp.join("tier8_action_06.json");
        let _ = std::fs::remove_file(&path);
        let mut storage = ActionStorage::new(path.clone()).map_err(|e| e)?;
        storage.add_action(make_test_action("old_tx", "ref_rekey", 3000, vec![], 500))
            .map_err(|e| e)?;
        storage.update_txid("ref_rekey", "new_tx".to_string(), "01000000...".to_string())
            .map_err(|e| e)?;
        // Old txid should be gone
        if storage.get_action("old_tx").is_some() { return Err("old txid should be removed".into()); }
        // New txid should exist
        let a = storage.get_action("new_tx").ok_or("new txid not found")?;
        if a.reference_number != "ref_rekey" { return Err("reference_number should persist".into()); }
        if a.satoshis != 3000 { return Err("satoshis should persist".into()); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("action/07", "delete-action", {
        let path = temp.join("tier8_action_07.json");
        let _ = std::fs::remove_file(&path);
        let mut storage = ActionStorage::new(path.clone()).map_err(|e| e)?;
        storage.add_action(make_test_action("tx_del", "ref_del", 1000, vec![], 600))
            .map_err(|e| e)?;
        storage.delete_action("tx_del").map_err(|e| e)?;
        if storage.count() != 0 { return Err("count should be 0 after delete".into()); }
        if storage.get_action("tx_del").is_some() { return Err("deleted action should be gone".into()); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("action/08", "duplicate-txid-rejected", {
        let path = temp.join("tier8_action_08.json");
        let _ = std::fs::remove_file(&path);
        let mut storage = ActionStorage::new(path.clone()).map_err(|e| e)?;
        storage.add_action(make_test_action("tx_dup", "ref_a", 1000, vec![], 700))
            .map_err(|e| e)?;
        let result = storage.add_action(make_test_action("tx_dup", "ref_b", 2000, vec![], 800));
        if result.is_ok() { return Err("duplicate txid should be rejected".into()); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("action/09", "list-with-label-filter", {
        let path = temp.join("tier8_action_09.json");
        let _ = std::fs::remove_file(&path);
        let mut storage = ActionStorage::new(path.clone()).map_err(|e| e)?;
        storage.add_action(make_test_action(
            "tx_shop", "ref_shop", 5000,
            vec!["shopping".into(), "online".into()], 900,
        )).map_err(|e| e)?;
        storage.add_action(make_test_action(
            "tx_pay", "ref_pay", 3000,
            vec!["payment".into()], 901,
        )).map_err(|e| e)?;
        storage.add_action(make_test_action(
            "tx_both", "ref_both", 7000,
            vec!["shopping".into(), "payment".into()], 902,
        )).map_err(|e| e)?;

        // "any" mode: actions with "shopping" label
        let filter = vec!["shopping".into()];
        let results = storage.list_actions(Some(&filter), Some("any"));
        if results.len() != 2 {
            return Err(format!("'any' shopping: expected 2, got {}", results.len()));
        }

        // "all" mode: actions with BOTH "shopping" AND "payment"
        let filter_both = vec!["shopping".into(), "payment".into()];
        let results = storage.list_actions(Some(&filter_both), Some("all"));
        if results.len() != 1 {
            return Err(format!("'all' shop+pay: expected 1, got {}", results.len()));
        }
        if results[0].txid != "tx_both" {
            return Err(format!("'all' mode should find tx_both, got {}", results[0].txid));
        }

        // No filter → all actions
        let all = storage.list_actions(None, None);
        if all.len() != 3 {
            return Err(format!("no filter: expected 3, got {}", all.len()));
        }

        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("action/10", "list-sorted-by-timestamp-desc", {
        let path = temp.join("tier8_action_10.json");
        let _ = std::fs::remove_file(&path);
        let mut storage = ActionStorage::new(path.clone()).map_err(|e| e)?;
        storage.add_action(make_test_action("tx_old", "r1", 1000, vec![], 100)).map_err(|e| e)?;
        storage.add_action(make_test_action("tx_new", "r2", 2000, vec![], 200)).map_err(|e| e)?;
        storage.add_action(make_test_action("tx_mid", "r3", 3000, vec![], 150)).map_err(|e| e)?;
        let list = storage.list_actions(None, None);
        // Should be newest first: tx_new(200), tx_mid(150), tx_old(100)
        if list[0].txid != "tx_new" { return Err(format!("first should be tx_new, got {}", list[0].txid)); }
        if list[1].txid != "tx_mid" { return Err(format!("second should be tx_mid, got {}", list[1].txid)); }
        if list[2].txid != "tx_old" { return Err(format!("third should be tx_old, got {}", list[2].txid)); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("action/11", "persistence-roundtrip", {
        let path = temp.join("tier8_action_persist.json");
        let _ = std::fs::remove_file(&path);
        // Phase 1: create, add, drop
        {
            let mut s = ActionStorage::new(path.clone()).map_err(|e| e)?;
            s.add_action(make_test_action("persist_tx", "persist_ref", 9999, vec![], 42))
                .map_err(|e| e)?;
        }
        // Phase 2: reload and verify
        {
            let s = ActionStorage::new(path.clone()).map_err(|e| e)?;
            let a = s.get_action("persist_tx").ok_or("action should survive reload")?;
            if a.satoshis != 9999 { return Err(format!("satoshis should be 9999, got {}", a.satoshis)); }
            if a.reference_number != "persist_ref" { return Err("ref should survive".into()); }
        }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 5/6: {} tests\n", after - before);
}

// ============================================================================
// [6/6]  JsonStorage Wallet Operations
// ============================================================================

#[test]
fn t8_06_json_storage() {
    eprintln!("\n=== TIER 8 [6/6] JsonStorage Wallet Operations ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    let temp = std::env::temp_dir();

    // Standard BIP39 test mnemonic (checksum-valid, 12 words)
    let test_wallet_json = r#"{
        "mnemonic": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        "addresses": [
            {
                "index": 0,
                "address": "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2",
                "publicKey": "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                "used": false,
                "balance": 0
            }
        ],
        "currentIndex": 0,
        "backedUp": false
    }"#;

    check!("json/01", "load-valid-wallet", {
        let path = temp.join("tier8_json_01.json");
        std::fs::write(&path, test_wallet_json).map_err(|e| format!("{}", e))?;
        let storage = JsonStorage::new(path.clone()).map_err(|e| e)?;
        let wallet = storage.get_wallet().map_err(|e| e)?;
        if wallet.mnemonic != "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about" {
            return Err("mnemonic mismatch".into());
        }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("json/02", "get-current-address", {
        let path = temp.join("tier8_json_02.json");
        std::fs::write(&path, test_wallet_json).map_err(|e| format!("{}", e))?;
        let storage = JsonStorage::new(path.clone()).map_err(|e| e)?;
        let addr = storage.get_current_address().map_err(|e| e)?;
        if addr.index != 0 { return Err(format!("index should be 0, got {}", addr.index)); }
        if addr.address.is_empty() { return Err("address empty".into()); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("json/03", "get-all-addresses", {
        let path = temp.join("tier8_json_03.json");
        std::fs::write(&path, test_wallet_json).map_err(|e| format!("{}", e))?;
        let storage = JsonStorage::new(path.clone()).map_err(|e| e)?;
        let addrs = storage.get_all_addresses().map_err(|e| e)?;
        if addrs.len() != 1 { return Err(format!("expected 1 address, got {}", addrs.len())); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("json/04", "get-master-private-key", {
        let path = temp.join("tier8_json_04.json");
        std::fs::write(&path, test_wallet_json).map_err(|e| format!("{}", e))?;
        let storage = JsonStorage::new(path.clone()).map_err(|e| e)?;
        let key = storage.get_master_private_key().map_err(|e| e)?;
        if key.len() != 32 { return Err(format!("expected 32 bytes, got {}", key.len())); }
        // Deterministic: same mnemonic → same key
        let key2 = storage.get_master_private_key().map_err(|e| e)?;
        if key != key2 { return Err("should be deterministic".into()); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("json/05", "get-master-public-key", {
        let path = temp.join("tier8_json_05.json");
        std::fs::write(&path, test_wallet_json).map_err(|e| format!("{}", e))?;
        let storage = JsonStorage::new(path.clone()).map_err(|e| e)?;
        let pubkey = storage.get_master_public_key().map_err(|e| e)?;
        if pubkey.len() != 33 { return Err(format!("expected 33 bytes, got {}", pubkey.len())); }
        // Must start with 0x02 or 0x03 (compressed pubkey prefix)
        if pubkey[0] != 0x02 && pubkey[0] != 0x03 {
            return Err(format!("bad prefix: 0x{:02x}", pubkey[0]));
        }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("json/06", "derive-private-key-different-indices", {
        let path = temp.join("tier8_json_06.json");
        std::fs::write(&path, test_wallet_json).map_err(|e| format!("{}", e))?;
        let storage = JsonStorage::new(path.clone()).map_err(|e| e)?;
        let k0 = storage.derive_private_key(0).map_err(|e| e)?;
        let k1 = storage.derive_private_key(1).map_err(|e| e)?;
        if k0.len() != 32 { return Err(format!("key 0 should be 32 bytes, got {}", k0.len())); }
        if k1.len() != 32 { return Err(format!("key 1 should be 32 bytes, got {}", k1.len())); }
        if k0 == k1 { return Err("different indices should give different keys".into()); }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("json/07", "master-key-matches-seed-derivation", {
        // The master private key from JsonStorage using the "abandon" mnemonic
        // should match derive_key_at_path with the same seed and empty path
        let path = temp.join("tier8_json_07.json");
        std::fs::write(&path, test_wallet_json).map_err(|e| format!("{}", e))?;
        let storage = JsonStorage::new(path.clone()).map_err(|e| e)?;
        let json_master = storage.get_master_private_key().map_err(|e| e)?;
        let seed_master = derive_key_at_path(&TEST_SEED, &[]).map_err(|e| e)?;
        // Both derive from the same mnemonic/seed → should match
        if json_master != seed_master {
            return Err("JsonStorage master key should match derive_key_at_path with empty path".into());
        }
        let _ = std::fs::remove_file(&path);
        Ok(())
    });

    check!("json/08", "missing-file-error", {
        let path = temp.join("tier8_json_nonexistent_99999.json");
        let _ = std::fs::remove_file(&path);
        let result = JsonStorage::new(path);
        if result.is_ok() {
            return Err("missing file should error".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 6/6: {} tests\n", after - before);
}

// ============================================================================
// Summary
// ============================================================================

#[test]
fn t8_zz_summary() {
    let p = PASS.load(Ordering::Relaxed);
    let f = FAIL.load(Ordering::Relaxed);
    eprintln!("\n╔══════════════════════════════════════════╗");
    eprintln!("║  TIER 8 FINAL: {} pass, {} fail, {} total   ", p, f, p + f);
    eprintln!("╚══════════════════════════════════════════╝\n");
    assert_eq!(f, 0, "There were {} test failures", f);
}
