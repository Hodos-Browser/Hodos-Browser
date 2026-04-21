//! Tier 5: Coverage Hardening & Error Path Tests
//!
//! Targets gaps identified after Tiers 1-4:
//!   [1/8] BEEF Advanced Operations (find_txid, parent_transactions, sort_topologically,
//!          extract_raw_tx_hex, ParsedTransaction, atomic beef base64)
//!   [2/8] Transaction Types Coverage (Script conversions, OutPoint methods,
//!          TxOutput::from_hex_script, encode_varint_signed)
//!   [3/8] Certificate JSON Parsing (parse_certificate_from_json edge cases)
//!   [4/8] AES-GCM Edge Cases (AAD path, plaintext sizes, IV sizes)
//!   [5/8] BalanceCache Integration (lifecycle, stale fallback, thread safety)
//!   [6/8] Crypto Error Paths (invalid keys, empty inputs, boundary conditions)
//!   [7/8] BEEF Error Paths (invalid markers, truncated data, format edge cases)
//!   [8/8] Recovery & Misc Coverage (address_to_p2pkh_script edges, deep paths,
//!          PriceCache, certificate types)

use std::collections::HashMap;

// ============================================================================
// Test infrastructure — collect-first, report-after pattern
// ============================================================================

struct TestResults {
    passed: u32,
    failed: u32,
    results: Vec<(String, bool, Option<String>)>,
}

impl TestResults {
    fn new() -> Self { Self { passed: 0, failed: 0, results: Vec::new() } }

    fn pass(&mut self, name: &str) {
        self.passed += 1;
        self.results.push((name.to_string(), true, None));
    }

    fn fail(&mut self, name: &str, reason: &str) {
        self.failed += 1;
        self.results.push((name.to_string(), false, Some(reason.to_string())));
    }

    fn print_section(&self, section: &str) {
        println!("  {}", section);
        for (name, ok, reason) in &self.results {
            if *ok {
                println!("    [PASS] {}", name);
            } else {
                println!("    [FAIL] {} — {}", name, reason.as_deref().unwrap_or("unknown"));
            }
        }
    }
}

macro_rules! check {
    ($results:expr, $name:expr, $block:expr) => {{
        let outcome: Result<(), String> = (|| -> Result<(), String> { $block })();
        match outcome {
            Ok(()) => $results.pass($name),
            Err(e) => $results.fail($name, &e),
        }
    }};
}

// ============================================================================
// Main test harness
// ============================================================================

#[test]
fn tier5_diagnostic_suite() {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   Tier 5: Coverage Hardening & Error Path Tests         ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let mut total_pass = 0u32;
    let mut total_fail = 0u32;

    // [1/8] BEEF Advanced Operations
    {
        let mut r = TestResults::new();
        test_beef_advanced(&mut r);
        r.print_section("[1/8] BEEF Advanced Operations");
        total_pass += r.passed;
        total_fail += r.failed;
    }

    // [2/8] Transaction Types Coverage
    {
        let mut r = TestResults::new();
        test_transaction_types(&mut r);
        r.print_section("[2/8] Transaction Types Coverage");
        total_pass += r.passed;
        total_fail += r.failed;
    }

    // [3/8] Certificate JSON Parsing
    {
        let mut r = TestResults::new();
        test_certificate_parsing(&mut r);
        r.print_section("[3/8] Certificate JSON Parsing");
        total_pass += r.passed;
        total_fail += r.failed;
    }

    // [4/8] AES-GCM Edge Cases
    {
        let mut r = TestResults::new();
        test_aesgcm_edges(&mut r);
        r.print_section("[4/8] AES-GCM Edge Cases");
        total_pass += r.passed;
        total_fail += r.failed;
    }

    // [5/8] BalanceCache Integration
    {
        let mut r = TestResults::new();
        test_balance_cache(&mut r);
        r.print_section("[5/8] BalanceCache Integration");
        total_pass += r.passed;
        total_fail += r.failed;
    }

    // [6/8] Crypto Error Paths
    {
        let mut r = TestResults::new();
        test_crypto_error_paths(&mut r);
        r.print_section("[6/8] Crypto Error Paths");
        total_pass += r.passed;
        total_fail += r.failed;
    }

    // [7/8] BEEF Error Paths
    {
        let mut r = TestResults::new();
        test_beef_error_paths(&mut r);
        r.print_section("[7/8] BEEF Error Paths");
        total_pass += r.passed;
        total_fail += r.failed;
    }

    // [8/8] Recovery & Misc Coverage
    {
        let mut r = TestResults::new();
        test_recovery_misc(&mut r);
        r.print_section("[8/8] Recovery & Misc Coverage");
        total_pass += r.passed;
        total_fail += r.failed;
    }

    // Summary
    let total = total_pass + total_fail;
    println!();
    println!("══════════════════════════════════════════════════════════");
    println!("  TOTAL: {} passed, {} failed, {} total", total_pass, total_fail, total);
    println!("══════════════════════════════════════════════════════════");
    println!();

    assert_eq!(total_fail, 0, "{} tests failed", total_fail);
}

// ============================================================================
// [1/8] BEEF Advanced Operations
// ============================================================================

fn test_beef_advanced(r: &mut TestResults) {
    use hodos_wallet::beef::Beef;

    // Real Bitcoin transactions from BRC-62 spec
    let parent_tx_hex = "0100000001cd4e4cac3c7b56920d1e7655e7e260d31f29d9a388d04910f1bbd72304a79029010000006b483045022100e75279a205a547c445719420aa3138bf14743e3f42618e5f86a19bde14bb95f7022064777d34776b05d816daf1699493fcdf2ef5a5ab1ad710d9c97bfb5b8f7cef3641210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013e660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000";
    let main_tx_hex = "0100000001ac4e164f5bc16746bb0868404292ac8318bbac3800e4aad13a014da427adce3e000000006a47304402203a61a2e931612b4bda08d541cfb980885173b8dcf64a3471238ae7abcd368d6402204cbf24f04b9aa2256d8901f0ed97866603d2be8324c2bfb7a37bf8fc90edd5b441210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013c660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000";
    let parent_tx = hex::decode(parent_tx_hex).unwrap();
    let main_tx = hex::decode(main_tx_hex).unwrap();

    // Compute parent TXID
    use sha2::{Sha256, Digest};
    let h1 = Sha256::digest(&parent_tx);
    let h2 = Sha256::digest(&h1);
    let parent_txid = hex::encode(h2.iter().rev().copied().collect::<Vec<u8>>());

    // Build a BEEF for multiple tests
    let build_test_beef = || -> Beef {
        let mut beef = Beef::new();
        let idx = beef.add_parent_transaction(parent_tx.clone());
        let tsc = serde_json::json!({
            "height": 918980,
            "index": 0,
            "nodes": [
                "9b18d77b48fde9b46d54b75d372e30a74cba0114cad4796f8f1d91946866a8bd",
                "45b8d1a256e4de964d2a70408e3ae4265b43544425ea40f370cd76d367575b0e"
            ]
        });
        beef.add_tsc_merkle_proof(&parent_txid, idx, &tsc).unwrap();
        beef.set_main_transaction(main_tx.clone());
        beef
    };

    // beef/1 — find_txid finds existing transaction
    check!(r, "beef/1 find-txid-found", {
        let beef = build_test_beef();
        let found = beef.find_txid(&parent_txid);
        if found != Some(0) {
            return Err(format!("Expected Some(0), got {:?}", found));
        }
        Ok(())
    });

    // beef/2 — find_txid returns None for missing txid
    check!(r, "beef/2 find-txid-missing", {
        let beef = build_test_beef();
        let fake_txid = "00".repeat(32);
        let found = beef.find_txid(&fake_txid);
        if found.is_some() {
            return Err(format!("Expected None, got {:?}", found));
        }
        Ok(())
    });

    // beef/3 — find_txid with invalid hex returns None
    check!(r, "beef/3 find-txid-invalid-hex", {
        let beef = build_test_beef();
        let found = beef.find_txid("not-valid-hex");
        if found.is_some() {
            return Err(format!("Expected None for invalid hex, got {:?}", found));
        }
        Ok(())
    });

    // beef/4 — find_txid with wrong length returns None
    check!(r, "beef/4 find-txid-wrong-length", {
        let beef = build_test_beef();
        let found = beef.find_txid("aabb");  // Only 2 bytes, not 32
        if found.is_some() {
            return Err(format!("Expected None for short txid, got {:?}", found));
        }
        Ok(())
    });

    // beef/5 — parent_transactions returns all except last
    check!(r, "beef/5 parent-transactions", {
        let beef = build_test_beef();
        let parents = beef.parent_transactions();
        if parents.len() != 1 {
            return Err(format!("Expected 1 parent, got {}", parents.len()));
        }
        if parents[0] != parent_tx {
            return Err("Parent tx bytes mismatch".into());
        }
        Ok(())
    });

    // beef/6 — parent_transactions with single tx returns empty
    check!(r, "beef/6 parent-transactions-single-tx", {
        let mut beef = Beef::new();
        beef.set_main_transaction(main_tx.clone());
        let parents = beef.parent_transactions();
        if !parents.is_empty() {
            return Err(format!("Expected empty parents, got {}", parents.len()));
        }
        Ok(())
    });

    // beef/7 — parent_transactions with no txs returns empty
    check!(r, "beef/7 parent-transactions-empty", {
        let beef = Beef::new();
        let parents = beef.parent_transactions();
        if !parents.is_empty() {
            return Err(format!("Expected empty parents, got {}", parents.len()));
        }
        Ok(())
    });

    // beef/8 — sort_topologically with single tx is no-op
    check!(r, "beef/8 topo-sort-single", {
        let mut beef = Beef::new();
        beef.set_main_transaction(main_tx.clone());
        let original = beef.transactions.clone();
        beef.sort_topologically();
        if beef.transactions != original {
            return Err("Single-tx sort should be no-op".into());
        }
        Ok(())
    });

    // beef/9 — sort_topologically with already-sorted transactions
    check!(r, "beef/9 topo-sort-already-sorted", {
        let mut beef = build_test_beef();
        let before = beef.transactions.clone();
        beef.sort_topologically();
        // Main tx references a different parent, so these are independent
        // Sort should preserve the order for independent txs
        if beef.transactions.len() != before.len() {
            return Err(format!("Transaction count changed: {} -> {}",
                before.len(), beef.transactions.len()));
        }
        Ok(())
    });

    // beef/10 — extract_raw_tx_hex
    check!(r, "beef/10 extract-raw-tx-hex", {
        let beef = build_test_beef();
        let beef_hex = beef.to_hex().map_err(|e| format!("to_hex: {}", e))?;
        let extracted = Beef::extract_raw_tx_hex(&beef_hex)
            .map_err(|e| format!("extract: {}", e))?;
        if extracted != main_tx_hex {
            return Err(format!("Extracted tx doesn't match main tx"));
        }
        Ok(())
    });

    // beef/11 — ParsedTransaction::from_hex
    check!(r, "beef/11 parsed-tx-from-hex", {
        let parsed = hodos_wallet::beef::ParsedTransaction::from_hex(parent_tx_hex)
            .map_err(|e| format!("from_hex: {}", e))?;
        if parsed.version != 1 {
            return Err(format!("Expected version 1, got {}", parsed.version));
        }
        if parsed.inputs.len() != 1 {
            return Err(format!("Expected 1 input, got {}", parsed.inputs.len()));
        }
        if parsed.outputs.len() != 1 {
            return Err(format!("Expected 1 output, got {}", parsed.outputs.len()));
        }
        if parsed.lock_time != 0 {
            return Err(format!("Expected locktime 0, got {}", parsed.lock_time));
        }
        Ok(())
    });

    // beef/12 — ParsedTransaction::from_hex extracts correct outpoint
    check!(r, "beef/12 parsed-tx-outpoint", {
        let parsed = hodos_wallet::beef::ParsedTransaction::from_hex(parent_tx_hex)
            .map_err(|e| format!("from_hex: {}", e))?;
        let input = &parsed.inputs[0];
        // prev_txid should be reversed hex of the raw bytes
        if input.prev_txid.len() != 64 {
            return Err(format!("prev_txid wrong length: {}", input.prev_txid.len()));
        }
        if input.prev_vout != 1 {
            return Err(format!("Expected prev_vout=1, got {}", input.prev_vout));
        }
        if input.sequence != 0xFFFFFFFF {
            return Err(format!("Expected sequence=0xFFFFFFFF, got {:#x}", input.sequence));
        }
        Ok(())
    });

    // beef/13 — ParsedTransaction output value
    check!(r, "beef/13 parsed-tx-output-value", {
        let parsed = hodos_wallet::beef::ParsedTransaction::from_hex(parent_tx_hex)
            .map_err(|e| format!("from_hex: {}", e))?;
        let output = &parsed.outputs[0];
        if output.value != 0x663e {  // 26174 satoshis
            return Err(format!("Expected value 0x663e, got {:#x}", output.value));
        }
        // P2PKH script should be 25 bytes
        if output.script.len() != 25 {
            return Err(format!("Expected 25-byte P2PKH, got {} bytes", output.script.len()));
        }
        Ok(())
    });

    // beef/14 — Atomic BEEF base64 roundtrip
    check!(r, "beef/14 atomic-beef-base64-roundtrip", {
        let beef = build_test_beef();
        // Compute main tx TXID
        let mh1 = Sha256::digest(&main_tx);
        let mh2 = Sha256::digest(&mh1);
        let main_txid = hex::encode(mh2.iter().rev().copied().collect::<Vec<u8>>());

        let atomic_hex = beef.to_atomic_beef_hex(&main_txid)
            .map_err(|e| format!("to_atomic: {}", e))?;

        // Convert hex to base64
        let atomic_bytes = hex::decode(&atomic_hex).map_err(|e| format!("hex decode: {}", e))?;
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let b64 = STANDARD.encode(&atomic_bytes);

        // Parse back
        let (parsed_txid, parsed_beef) = Beef::from_atomic_beef_base64(&b64)
            .map_err(|e| format!("from_base64: {}", e))?;

        if parsed_txid != main_txid {
            return Err(format!("TXID mismatch: {} != {}", parsed_txid, main_txid));
        }
        if parsed_beef.transactions.len() != 2 {
            return Err(format!("Expected 2 txs, got {}", parsed_beef.transactions.len()));
        }
        Ok(())
    });

    // beef/15 — to_v1_hex produces valid V1 format
    check!(r, "beef/15 v1-hex-format", {
        let beef = build_test_beef();
        let v1_hex = beef.to_v1_hex().map_err(|e| format!("to_v1_hex: {}", e))?;
        // V1 marker is 0100beef = "0100beef"
        if !v1_hex.starts_with("0100beef") {
            return Err(format!("V1 hex should start with 0100beef, got: {}", &v1_hex[..16]));
        }
        // Parse it back
        let parsed = Beef::from_hex(&v1_hex).map_err(|e| format!("parse V1: {}", e))?;
        if parsed.transactions.len() != 2 {
            return Err(format!("Expected 2 txs in parsed V1, got {}", parsed.transactions.len()));
        }
        Ok(())
    });

    // beef/16 — find_txid locates main tx (last)
    check!(r, "beef/16 find-txid-main-tx", {
        let beef = build_test_beef();
        let mh1 = Sha256::digest(&main_tx);
        let mh2 = Sha256::digest(&mh1);
        let main_txid = hex::encode(mh2.iter().rev().copied().collect::<Vec<u8>>());
        let found = beef.find_txid(&main_txid);
        if found != Some(1) {
            return Err(format!("Expected main tx at index 1, got {:?}", found));
        }
        Ok(())
    });
}

// ============================================================================
// [2/8] Transaction Types Coverage
// ============================================================================

fn test_transaction_types(r: &mut TestResults) {
    use hodos_wallet::transaction::{Script, OutPoint, TxOutput, TxInput, Transaction, encode_varint};
    use hodos_wallet::transaction::encode_varint_signed;

    // txtype/1 — Script from_hex -> to_hex roundtrip
    check!(r, "txtype/1 script-hex-roundtrip", {
        let hex_str = "76a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac";
        let script = Script::from_hex(hex_str).map_err(|e| format!("{}", e))?;
        let back = script.to_hex();
        if back != hex_str {
            return Err(format!("Roundtrip mismatch: {} != {}", back, hex_str));
        }
        Ok(())
    });

    // txtype/2 — Script from_bytes -> to_bytes roundtrip
    check!(r, "txtype/2 script-bytes-roundtrip", {
        let bytes = vec![0x76, 0xa9, 0x14, 0x00, 0x01, 0x02];
        let script = Script::from_bytes(bytes.clone());
        let back = script.to_bytes();
        if back != bytes.as_slice() {
            return Err(format!("Bytes mismatch: {:?} != {:?}", back, bytes));
        }
        Ok(())
    });

    // txtype/3 — Script from_hex with invalid hex
    check!(r, "txtype/3 script-invalid-hex", {
        let result = Script::from_hex("zzzz");
        if result.is_ok() {
            return Err("Should reject invalid hex".into());
        }
        Ok(())
    });

    // txtype/4 — Script from_hex with odd-length hex
    check!(r, "txtype/4 script-odd-hex", {
        let result = Script::from_hex("abc");
        if result.is_ok() {
            return Err("Should reject odd-length hex".into());
        }
        Ok(())
    });

    // txtype/5 — Script empty
    check!(r, "txtype/5 script-empty", {
        let script = Script::new();
        if !script.to_bytes().is_empty() {
            return Err("New script should be empty".into());
        }
        if !script.to_hex().is_empty() {
            return Err("New script hex should be empty".into());
        }
        Ok(())
    });

    // txtype/6 — OutPoint txid_bytes reverses for wire format
    check!(r, "txtype/6 outpoint-txid-bytes", {
        let txid = "0102030405060708091011121314151617181920212223242526272829303132";
        let op = OutPoint::new(txid, 0);
        let wire = op.txid_bytes().map_err(|e| format!("{}", e))?;
        if wire.len() != 32 {
            return Err(format!("Expected 32 bytes, got {}", wire.len()));
        }
        // Wire format should be reversed
        let first_byte = hex::decode(&txid[0..2]).unwrap()[0];
        let last_wire_byte = wire[31];
        if first_byte != last_wire_byte {
            return Err(format!("Wire format not reversed: first hex byte {:02x} != last wire byte {:02x}",
                first_byte, last_wire_byte));
        }
        Ok(())
    });

    // txtype/7 — OutPoint serialize produces 36 bytes
    check!(r, "txtype/7 outpoint-serialize", {
        let txid = "00".repeat(32);
        let op = OutPoint::new(&txid, 42);
        let serialized = op.serialize().map_err(|e| format!("{}", e))?;
        if serialized.len() != 36 {
            return Err(format!("Expected 36 bytes, got {}", serialized.len()));
        }
        // Last 4 bytes should be vout in LE
        let vout_bytes = &serialized[32..36];
        let vout = u32::from_le_bytes([vout_bytes[0], vout_bytes[1], vout_bytes[2], vout_bytes[3]]);
        if vout != 42 {
            return Err(format!("Expected vout=42, got {}", vout));
        }
        Ok(())
    });

    // txtype/8 — OutPoint Display format
    check!(r, "txtype/8 outpoint-display", {
        let txid = "abcd".to_string() + &"00".repeat(30);
        let op = OutPoint::new(&txid, 7);
        let display = format!("{}", op);
        let expected = format!("{}:7", txid);
        if display != expected {
            return Err(format!("Display mismatch: {} != {}", display, expected));
        }
        Ok(())
    });

    // txtype/9 — TxOutput::from_hex_script
    check!(r, "txtype/9 txoutput-from-hex-script", {
        let script_hex = "76a914000102030405060708090a0b0c0d0e0f1011121388ac";
        let output = TxOutput::from_hex_script(50000, script_hex)
            .map_err(|e| format!("{}", e))?;
        if output.value != 50000 {
            return Err(format!("Expected value 50000, got {}", output.value));
        }
        if output.script_pubkey.len() != 25 {
            return Err(format!("Expected 25-byte script, got {}", output.script_pubkey.len()));
        }
        Ok(())
    });

    // txtype/10 — TxOutput::from_hex_script with invalid hex
    check!(r, "txtype/10 txoutput-hex-invalid", {
        let result = TxOutput::from_hex_script(100, "not-hex");
        if result.is_ok() {
            return Err("Should reject invalid hex".into());
        }
        Ok(())
    });

    // txtype/11 — encode_varint_signed positive values match unsigned
    check!(r, "txtype/11 varint-signed-positive", {
        for val in [0i64, 1, 100, 252, 253, 65535, 65536, 0xFFFFFFFF] {
            let signed = encode_varint_signed(val);
            let unsigned = encode_varint(val as u64);
            if signed != unsigned {
                return Err(format!("Mismatch at {}: {:?} != {:?}", val, signed, unsigned));
            }
        }
        Ok(())
    });

    // txtype/12 — encode_varint_signed negative values use two's complement
    check!(r, "txtype/12 varint-signed-negative", {
        // -1 as u64 is u64::MAX = 0xFFFFFFFFFFFFFFFF
        let encoded = encode_varint_signed(-1);
        let expected = encode_varint(u64::MAX);
        if encoded != expected {
            return Err(format!("-1 encoding mismatch: {:?} != {:?}", encoded, expected));
        }
        // Should be 9 bytes: 0xFF prefix + 8 bytes
        if encoded.len() != 9 {
            return Err(format!("Expected 9 bytes for -1, got {}", encoded.len()));
        }
        Ok(())
    });

    // txtype/13 — TxInput::set_script
    check!(r, "txtype/13 txinput-set-script", {
        let txid = "00".repeat(32);
        let op = OutPoint::new(&txid, 0);
        let mut input = TxInput::new(op);
        if !input.script_sig.is_empty() {
            return Err("New input should have empty script".into());
        }
        input.set_script(vec![0x01, 0x02, 0x03]);
        if input.script_sig != vec![0x01, 0x02, 0x03] {
            return Err("set_script didn't update script_sig".into());
        }
        Ok(())
    });

    // txtype/14 — Transaction default values
    check!(r, "txtype/14 tx-defaults", {
        let tx = Transaction::new();
        if tx.version != 1 {
            return Err(format!("Expected version 1, got {}", tx.version));
        }
        if tx.lock_time != 0 {
            return Err(format!("Expected locktime 0, got {}", tx.lock_time));
        }
        if !tx.inputs.is_empty() || !tx.outputs.is_empty() {
            return Err("New tx should have empty inputs/outputs".into());
        }
        Ok(())
    });

    // txtype/15 — TxOutput serialize format
    check!(r, "txtype/15 txoutput-serialize", {
        let output = TxOutput::new(100_000, vec![0x76, 0xa9]);
        let bytes = output.serialize();
        // 8 bytes value + varint(2) + 2 bytes script = 11 bytes
        if bytes.len() != 11 {
            return Err(format!("Expected 11 bytes, got {}", bytes.len()));
        }
        // First 8 bytes: 100000 in LE
        let value = i64::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3],
                                         bytes[4], bytes[5], bytes[6], bytes[7]]);
        if value != 100_000 {
            return Err(format!("Value mismatch: {} != 100000", value));
        }
        Ok(())
    });
}

// ============================================================================
// [3/8] Certificate JSON Parsing
// ============================================================================

fn test_certificate_parsing(r: &mut TestResults) {
    use hodos_wallet::certificate::parser::parse_certificate_from_json;

    // Helper to create a valid certificate JSON
    let valid_cert_json = || -> serde_json::Value {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let type_b64 = STANDARD.encode(vec![0xAAu8; 32]);
        let serial_b64 = STANDARD.encode(vec![0xBBu8; 32]);
        let certifier = format!("02{}", "ab".repeat(32)); // 33 bytes compressed pubkey
        let subject = format!("03{}", "cd".repeat(32));
        let txid = "00".repeat(32);
        let field_b64 = STANDARD.encode(b"encrypted_data");
        let keyring_b64 = STANDARD.encode(b"keyring_value");

        serde_json::json!({
            "type": type_b64,
            "serialNumber": serial_b64,
            "certifier": certifier,
            "subject": subject,
            "revocationOutpoint": format!("{}.0", txid),
            "signature": "3006020101020101",
            "fields": {
                "name": field_b64
            },
            "keyringForSubject": {
                "name": keyring_b64
            }
        })
    };

    // cert/1 — valid certificate parses correctly
    check!(r, "cert/1 valid-parse", {
        let json = valid_cert_json();
        let cert = parse_certificate_from_json(&json)
            .map_err(|e| format!("{}", e))?;
        if cert.type_.len() != 32 { return Err("type not 32 bytes".into()); }
        if cert.serial_number.len() != 32 { return Err("serial not 32 bytes".into()); }
        if cert.certifier.len() != 33 { return Err("certifier not 33 bytes".into()); }
        if cert.subject.len() != 33 { return Err("subject not 33 bytes".into()); }
        if cert.fields.len() != 1 { return Err("expected 1 field".into()); }
        Ok(())
    });

    // cert/2 — missing type field
    check!(r, "cert/2 missing-type", {
        let mut json = valid_cert_json();
        json.as_object_mut().unwrap().remove("type");
        let result = parse_certificate_from_json(&json);
        if result.is_ok() {
            return Err("Should reject missing type".into());
        }
        let err = format!("{}", result.unwrap_err());
        if !err.contains("type") {
            return Err(format!("Error should mention 'type': {}", err));
        }
        Ok(())
    });

    // cert/3 — missing serialNumber
    check!(r, "cert/3 missing-serial", {
        let mut json = valid_cert_json();
        json.as_object_mut().unwrap().remove("serialNumber");
        let result = parse_certificate_from_json(&json);
        if result.is_ok() {
            return Err("Should reject missing serialNumber".into());
        }
        Ok(())
    });

    // cert/4 — missing subject
    check!(r, "cert/4 missing-subject", {
        let mut json = valid_cert_json();
        json.as_object_mut().unwrap().remove("subject");
        let result = parse_certificate_from_json(&json);
        if result.is_ok() {
            return Err("Should reject missing subject".into());
        }
        Ok(())
    });

    // cert/5 — invalid base64 in type
    check!(r, "cert/5 invalid-base64-type", {
        let mut json = valid_cert_json();
        json["type"] = serde_json::json!("!!!not-base64!!!");
        let result = parse_certificate_from_json(&json);
        if result.is_ok() {
            return Err("Should reject invalid base64".into());
        }
        Ok(())
    });

    // cert/6 — type wrong length (not 32 bytes)
    check!(r, "cert/6 type-wrong-length", {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let mut json = valid_cert_json();
        json["type"] = serde_json::json!(STANDARD.encode(vec![0xAAu8; 16])); // 16 bytes, not 32
        let result = parse_certificate_from_json(&json);
        if result.is_ok() {
            return Err("Should reject type that's not 32 bytes".into());
        }
        Ok(())
    });

    // cert/7 — certifier wrong length
    check!(r, "cert/7 certifier-wrong-length", {
        let mut json = valid_cert_json();
        json["certifier"] = serde_json::json!("02aabb"); // Only 3 bytes, not 33
        let result = parse_certificate_from_json(&json);
        if result.is_ok() {
            return Err("Should reject certifier that's not 33 bytes".into());
        }
        Ok(())
    });

    // cert/8 — invalid certifier hex
    check!(r, "cert/8 certifier-invalid-hex", {
        let mut json = valid_cert_json();
        json["certifier"] = serde_json::json!("zzz");
        let result = parse_certificate_from_json(&json);
        if result.is_ok() {
            return Err("Should reject invalid certifier hex".into());
        }
        Ok(())
    });

    // cert/9 — empty signature rejected
    check!(r, "cert/9 empty-signature", {
        let mut json = valid_cert_json();
        json["signature"] = serde_json::json!("");
        let result = parse_certificate_from_json(&json);
        if result.is_ok() {
            return Err("Should reject empty signature".into());
        }
        Ok(())
    });

    // cert/10 — field name exceeding 50 bytes
    check!(r, "cert/10 long-field-name", {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let mut json = valid_cert_json();
        let long_name = "a".repeat(51);
        let field_b64 = STANDARD.encode(b"value");
        json["fields"] = serde_json::json!({ long_name: field_b64 });
        json.as_object_mut().unwrap().remove("keyringForSubject"); // Remove to avoid mismatch
        let result = parse_certificate_from_json(&json);
        if result.is_ok() {
            return Err("Should reject field name > 50 bytes".into());
        }
        Ok(())
    });

    // cert/11 — revocation outpoint invalid format
    check!(r, "cert/11 invalid-revocation-outpoint", {
        let mut json = valid_cert_json();
        json["revocationOutpoint"] = serde_json::json!("not-a-valid-outpoint");
        let result = parse_certificate_from_json(&json);
        if result.is_ok() {
            return Err("Should reject invalid revocationOutpoint format".into());
        }
        Ok(())
    });

    // cert/12 — keyringForSubject with non-existent field
    check!(r, "cert/12 keyring-missing-field", {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let mut json = valid_cert_json();
        let kr_b64 = STANDARD.encode(b"keyring");
        json["keyringForSubject"] = serde_json::json!({
            "name": kr_b64.clone(),
            "nonexistent_field": kr_b64
        });
        let result = parse_certificate_from_json(&json);
        if result.is_ok() {
            return Err("Should reject keyring referencing non-existent field".into());
        }
        Ok(())
    });

    // cert/13 — certificate without keyringForSubject succeeds
    check!(r, "cert/13 no-keyring-ok", {
        let mut json = valid_cert_json();
        json.as_object_mut().unwrap().remove("keyringForSubject");
        let result = parse_certificate_from_json(&json);
        if result.is_err() {
            return Err(format!("Should allow missing keyring: {}", result.unwrap_err()));
        }
        Ok(())
    });

    // cert/14 — verifier/validationKey is parsed when present
    check!(r, "cert/14 verifier-parsed", {
        let mut json = valid_cert_json();
        let verifier = format!("02{}", "ff".repeat(32)); // 33 bytes
        json["verifier"] = serde_json::json!(verifier);
        let cert = parse_certificate_from_json(&json)
            .map_err(|e| format!("{}", e))?;
        if cert.verifier.is_none() {
            return Err("Verifier should be parsed".into());
        }
        if cert.verifier.as_ref().unwrap().len() != 33 {
            return Err("Verifier should be 33 bytes".into());
        }
        Ok(())
    });

    // cert/15 — multiple fields parsed correctly
    check!(r, "cert/15 multiple-fields", {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let mut json = valid_cert_json();
        let f1 = STANDARD.encode(b"field1_encrypted");
        let f2 = STANDARD.encode(b"field2_encrypted");
        let f3 = STANDARD.encode(b"field3_encrypted");
        json["fields"] = serde_json::json!({
            "name": f1,
            "email": f2,
            "age": f3
        });
        json.as_object_mut().unwrap().remove("keyringForSubject");
        let cert = parse_certificate_from_json(&json)
            .map_err(|e| format!("{}", e))?;
        if cert.fields.len() != 3 {
            return Err(format!("Expected 3 fields, got {}", cert.fields.len()));
        }
        if !cert.fields.contains_key("name") ||
           !cert.fields.contains_key("email") ||
           !cert.fields.contains_key("age") {
            return Err("Missing expected field keys".into());
        }
        Ok(())
    });
}

// ============================================================================
// [4/8] AES-GCM Edge Cases
// ============================================================================

fn test_aesgcm_edges(r: &mut TestResults) {
    use hodos_wallet::crypto::aesgcm_custom::{aesgcm_custom, aesgcm_decrypt_custom};
    // Signature: aesgcm_custom(plaintext, additional_data, iv, key) -> Result<(ct, tag), String>
    // Signature: aesgcm_decrypt_custom(ciphertext, additional_data, iv, auth_tag, key) -> Result<Vec<u8>, String>

    let key = [0x42u8; 32];

    // aes/1 — Non-empty AAD (Additional Authenticated Data)
    check!(r, "aes/1 non-empty-aad", {
        let iv = vec![0x01u8; 12];
        let plaintext = b"hello world";
        let aad = b"additional data";
        let (ct, tag) = aesgcm_custom(plaintext, aad, &iv, &key)
            .map_err(|e| format!("Encrypt: {}", e))?;
        let decrypted = aesgcm_decrypt_custom(&ct, aad, &iv, &tag, &key)
            .map_err(|e| format!("Decrypt failed: {}", e))?;
        if decrypted != plaintext {
            return Err("Roundtrip with AAD failed".into());
        }
        Ok(())
    });

    // aes/2 — AAD mismatch causes authentication failure
    check!(r, "aes/2 aad-mismatch", {
        let iv = vec![0x02u8; 12];
        let plaintext = b"secret message";
        let aad = b"correct aad";
        let (ct, tag) = aesgcm_custom(plaintext, aad, &iv, &key)
            .map_err(|e| format!("Encrypt: {}", e))?;
        let wrong_aad = b"wrong aad";
        let result = aesgcm_decrypt_custom(&ct, wrong_aad, &iv, &tag, &key);
        if result.is_ok() {
            return Err("Should fail with wrong AAD".into());
        }
        Ok(())
    });

    // aes/3 — Single-byte plaintext
    check!(r, "aes/3 single-byte", {
        let iv = vec![0x03u8; 12];
        let plaintext: &[u8] = &[0x42u8];
        let (ct, tag) = aesgcm_custom(plaintext, &[], &iv, &key)
            .map_err(|e| format!("Encrypt: {}", e))?;
        let decrypted = aesgcm_decrypt_custom(&ct, &[], &iv, &tag, &key)
            .map_err(|e| format!("{}", e))?;
        if decrypted != plaintext {
            return Err("Single-byte roundtrip failed".into());
        }
        Ok(())
    });

    // aes/4 — Exactly 16 bytes (one AES block)
    check!(r, "aes/4 one-block", {
        let iv = vec![0x04u8; 12];
        let plaintext = &[0xAAu8; 16];
        let (ct, tag) = aesgcm_custom(plaintext, &[], &iv, &key)
            .map_err(|e| format!("Encrypt: {}", e))?;
        if ct.len() != 16 {
            return Err(format!("Expected 16-byte ciphertext, got {}", ct.len()));
        }
        let decrypted = aesgcm_decrypt_custom(&ct, &[], &iv, &tag, &key)
            .map_err(|e| format!("{}", e))?;
        if decrypted != plaintext {
            return Err("One-block roundtrip failed".into());
        }
        Ok(())
    });

    // aes/5 — 15-byte plaintext (not block-aligned)
    check!(r, "aes/5 non-aligned-15", {
        let iv = vec![0x05u8; 12];
        let plaintext = &[0xBBu8; 15];
        let (ct, tag) = aesgcm_custom(plaintext, &[], &iv, &key)
            .map_err(|e| format!("Encrypt: {}", e))?;
        if ct.len() != 15 {
            return Err(format!("CTR mode: ciphertext should be 15 bytes, got {}", ct.len()));
        }
        let decrypted = aesgcm_decrypt_custom(&ct, &[], &iv, &tag, &key)
            .map_err(|e| format!("{}", e))?;
        if decrypted != plaintext {
            return Err("15-byte roundtrip failed".into());
        }
        Ok(())
    });

    // aes/6 — 32-byte IV (non-standard, used by BRC-2)
    check!(r, "aes/6 32-byte-iv", {
        let iv = vec![0x06u8; 32];
        let plaintext = b"BRC-2 uses 32-byte IVs";
        let (ct, tag) = aesgcm_custom(plaintext, &[], &iv, &key)
            .map_err(|e| format!("Encrypt: {}", e))?;
        let decrypted = aesgcm_decrypt_custom(&ct, &[], &iv, &tag, &key)
            .map_err(|e| format!("{}", e))?;
        if decrypted != plaintext {
            return Err("32-byte IV roundtrip failed".into());
        }
        Ok(())
    });

    // aes/7 — 16-byte IV (non-standard, GHASH-based processing)
    check!(r, "aes/7 16-byte-iv", {
        let iv = vec![0x07u8; 16];
        let plaintext = b"non-standard IV length";
        let (ct, tag) = aesgcm_custom(plaintext, &[], &iv, &key)
            .map_err(|e| format!("Encrypt: {}", e))?;
        let decrypted = aesgcm_decrypt_custom(&ct, &[], &iv, &tag, &key)
            .map_err(|e| format!("{}", e))?;
        if decrypted != plaintext {
            return Err("16-byte IV roundtrip failed".into());
        }
        Ok(())
    });

    // aes/8 — Zero-length ciphertext with auth tag check
    check!(r, "aes/8 all-zeros-tag-rejected", {
        let iv = vec![0x08u8; 12];
        let plaintext = b"test";
        let (ct, _real_tag) = aesgcm_custom(plaintext, &[], &iv, &key)
            .map_err(|e| format!("Encrypt: {}", e))?;
        let zero_tag = vec![0u8; 16];
        let result = aesgcm_decrypt_custom(&ct, &[], &iv, &zero_tag, &key);
        if result.is_ok() {
            return Err("Should reject all-zeros auth tag".into());
        }
        Ok(())
    });

    // aes/9 — Multi-block plaintext (48 bytes = 3 blocks)
    check!(r, "aes/9 multi-block-48", {
        let iv = vec![0x09u8; 12];
        let plaintext = &[0xCCu8; 48];
        let (ct, tag) = aesgcm_custom(plaintext, &[], &iv, &key)
            .map_err(|e| format!("Encrypt: {}", e))?;
        if ct.len() != 48 {
            return Err(format!("Expected 48-byte ciphertext, got {}", ct.len()));
        }
        let decrypted = aesgcm_decrypt_custom(&ct, &[], &iv, &tag, &key)
            .map_err(|e| format!("{}", e))?;
        if decrypted != plaintext {
            return Err("48-byte roundtrip failed".into());
        }
        Ok(())
    });

    // aes/10 — Empty plaintext
    check!(r, "aes/10 empty-plaintext", {
        let iv = vec![0x0Au8; 12];
        let plaintext: &[u8] = &[];
        let (ct, tag) = aesgcm_custom(plaintext, &[], &iv, &key)
            .map_err(|e| format!("Encrypt: {}", e))?;
        if !ct.is_empty() {
            return Err(format!("Expected empty ciphertext, got {} bytes", ct.len()));
        }
        // Tag should still be 16 bytes
        if tag.len() != 16 {
            return Err(format!("Expected 16-byte tag, got {} bytes", tag.len()));
        }
        let decrypted = aesgcm_decrypt_custom(&ct, &[], &iv, &tag, &key)
            .map_err(|e| format!("{}", e))?;
        if !decrypted.is_empty() {
            return Err("Decrypted empty plaintext should be empty".into());
        }
        Ok(())
    });

    // aes/11 — AAD-only authentication (no plaintext)
    check!(r, "aes/11 aad-only-auth", {
        let iv = vec![0x0Bu8; 12];
        let plaintext: &[u8] = &[];
        let aad = b"authenticate this without encrypting anything";
        let (_ct, tag) = aesgcm_custom(plaintext, aad, &iv, &key)
            .map_err(|e| format!("Encrypt: {}", e))?;
        // Wrong AAD should fail
        let result = aesgcm_decrypt_custom(&[], b"wrong", &iv, &tag, &key);
        if result.is_ok() {
            return Err("Should fail with wrong AAD even with empty plaintext".into());
        }
        // Correct AAD should pass
        let decrypted = aesgcm_decrypt_custom(&[], aad, &iv, &tag, &key)
            .map_err(|e| format!("{}", e))?;
        if !decrypted.is_empty() {
            return Err("Should return empty plaintext".into());
        }
        Ok(())
    });
}

// ============================================================================
// [5/8] BalanceCache Integration
// ============================================================================

fn test_balance_cache(r: &mut TestResults) {
    use hodos_wallet::balance_cache::BalanceCache;
    use std::sync::Arc;

    // cache/1 — new cache returns None
    check!(r, "cache/1 new-returns-none", {
        let cache = BalanceCache::new();
        if cache.get().is_some() {
            return Err("New cache should return None".into());
        }
        if cache.get_or_stale().is_some() {
            return Err("New cache get_or_stale should return None".into());
        }
        Ok(())
    });

    // cache/2 — set then get
    check!(r, "cache/2 set-get", {
        let cache = BalanceCache::new();
        cache.set(500_000);
        match cache.get() {
            Some(v) if v == 500_000 => Ok(()),
            Some(v) => Err(format!("Expected 500000, got {}", v)),
            None => Err("Expected Some, got None".into()),
        }
    });

    // cache/3 — invalidate makes get return None
    check!(r, "cache/3 invalidate", {
        let cache = BalanceCache::new();
        cache.set(100);
        cache.invalidate();
        if cache.get().is_some() {
            return Err("get() should return None after invalidate".into());
        }
        Ok(())
    });

    // cache/4 — get_or_stale returns value after invalidate
    check!(r, "cache/4 get-or-stale-after-invalidate", {
        let cache = BalanceCache::new();
        cache.set(42_000);
        cache.invalidate();
        match cache.get_or_stale() {
            Some(v) if v == 42_000 => Ok(()),
            Some(v) => Err(format!("Expected 42000, got {}", v)),
            None => Err("get_or_stale should return stale value".into()),
        }
    });

    // cache/5 — update overwrites previous value
    check!(r, "cache/5 update-overwrites", {
        let cache = BalanceCache::new();
        cache.set(100);
        cache.update(200);
        match cache.get() {
            Some(v) if v == 200 => Ok(()),
            Some(v) => Err(format!("Expected 200 after update, got {}", v)),
            None => Err("get() should work after update".into()),
        }
    });

    // cache/6 — update after invalidate restores fresh state
    check!(r, "cache/6 update-after-invalidate", {
        let cache = BalanceCache::new();
        cache.set(100);
        cache.invalidate();
        cache.update(300);
        match cache.get() {
            Some(v) if v == 300 => Ok(()),
            other => Err(format!("Expected Some(300), got {:?}", other)),
        }
    });

    // cache/7 — thread safety: concurrent set
    check!(r, "cache/7 thread-safety", {
        let cache = Arc::new(BalanceCache::new());
        let mut handles = Vec::new();
        for i in 0..10 {
            let c = cache.clone();
            handles.push(std::thread::spawn(move || {
                c.set(i * 1000);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        // Should have some value (the last writer wins)
        if cache.get().is_none() {
            return Err("Should have a value after concurrent sets".into());
        }
        Ok(())
    });

    // cache/8 — negative balance allowed
    check!(r, "cache/8 negative-balance", {
        let cache = BalanceCache::new();
        cache.set(-500);
        match cache.get() {
            Some(v) if v == -500 => Ok(()),
            other => Err(format!("Expected Some(-500), got {:?}", other)),
        }
    });

    // cache/9 — zero balance
    check!(r, "cache/9 zero-balance", {
        let cache = BalanceCache::new();
        cache.set(0);
        match cache.get() {
            Some(v) if v == 0 => Ok(()),
            other => Err(format!("Expected Some(0), got {:?}", other)),
        }
    });
}

// ============================================================================
// [6/8] Crypto Error Paths
// ============================================================================

fn test_crypto_error_paths(r: &mut TestResults) {
    use hodos_wallet::crypto::keys::{derive_public_key, derive_public_key_uncompressed};
    use hodos_wallet::crypto::brc42;
    use hodos_wallet::crypto::signing;
    use hodos_wallet::crypto::brc2;

    // err/1 — derive_public_key with zero key (invalid)
    check!(r, "err/1 pubkey-zero-key", {
        let zero = [0u8; 32];
        let result = derive_public_key(&zero);
        if result.is_ok() {
            return Err("Zero private key should be invalid".into());
        }
        Ok(())
    });

    // err/2 — derive_public_key with curve order (invalid)
    check!(r, "err/2 pubkey-curve-order", {
        let n = hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141").unwrap();
        let result = derive_public_key(&n);
        if result.is_ok() {
            return Err("Curve order N should be invalid private key".into());
        }
        Ok(())
    });

    // err/3 — derive_public_key_uncompressed output is 65 bytes
    check!(r, "err/3 uncompressed-65-bytes", {
        let key = [0x01u8; 32]; // key = 0x0101...01
        let result = derive_public_key_uncompressed(&key)
            .map_err(|e| format!("{}", e))?;
        if result.len() != 65 {
            return Err(format!("Expected 65 bytes, got {}", result.len()));
        }
        if result[0] != 0x04 {
            return Err(format!("Expected 0x04 prefix, got 0x{:02x}", result[0]));
        }
        Ok(())
    });

    // err/4 — BRC-42 shared secret with invalid public key
    check!(r, "err/4 brc42-bad-pubkey", {
        let priv_key = [0x01u8; 32];
        let bad_pubkey = vec![0x02; 10]; // Wrong length
        let result = brc42::compute_shared_secret(&priv_key, &bad_pubkey);
        if result.is_ok() {
            return Err("Should reject invalid public key".into());
        }
        Ok(())
    });

    // err/5 — BRC-42 derive child private key with empty invoice
    check!(r, "err/5 brc42-empty-invoice", {
        let priv_key = [0x42u8; 32];
        let pub_key = derive_public_key(&priv_key).map_err(|e| format!("{}", e))?;
        // Empty invoice should still work (HMAC of empty string is defined)
        let result = brc42::derive_child_private_key(&priv_key, &pub_key, "");
        if result.is_err() {
            return Err(format!("Empty invoice should work: {}", result.unwrap_err()));
        }
        Ok(())
    });

    // err/6 — BRC-42 with very long invoice number
    check!(r, "err/6 brc42-long-invoice", {
        let priv_key = [0x42u8; 32];
        let pub_key = derive_public_key(&priv_key).map_err(|e| format!("{}", e))?;
        let long_invoice = "x".repeat(10000);
        let result = brc42::derive_child_private_key(&priv_key, &pub_key, &long_invoice);
        // Should succeed — HMAC handles any input length
        if result.is_err() {
            return Err(format!("Long invoice should work: {}", result.unwrap_err()));
        }
        Ok(())
    });

    // err/7 — sign with zero sighash (edge case for k-generation)
    check!(r, "err/7 sign-zero-sighash", {
        let priv_key = [0x42u8; 32];
        let zero_hash = [0u8; 32];
        let result = signing::sign_ecdsa(&zero_hash, &priv_key, 0x41);
        // RFC6979 with all-zeros hash should still produce a valid signature
        if result.is_err() {
            return Err(format!("Zero hash should still sign: {}", result.unwrap_err()));
        }
        let sig = result.unwrap();
        let pub_key = derive_public_key(&priv_key).map_err(|e| format!("{}", e))?;
        let valid = signing::verify_signature(&zero_hash, &sig, &pub_key)
            .map_err(|e| format!("{}", e))?;
        if !valid {
            return Err("Signature of zero hash should verify".into());
        }
        Ok(())
    });

    // err/8 — HMAC with key longer than 64 bytes
    check!(r, "err/8 hmac-long-key", {
        let long_key = vec![0xAA; 128]; // > 64 bytes, should be hashed first per HMAC spec
        let data = b"test data";
        let mac1 = signing::hmac_sha256(&long_key, data);
        let mac2 = signing::hmac_sha256(&long_key, data);
        if mac1 != mac2 {
            return Err("HMAC with long key should be deterministic".into());
        }
        if mac1.len() != 32 {
            return Err(format!("Expected 32-byte HMAC, got {}", mac1.len()));
        }
        Ok(())
    });

    // err/9 — double_sha256 of empty input
    check!(r, "err/9 double-sha256-empty", {
        let result = signing::double_sha256(&[]);
        if result.len() != 32 {
            return Err(format!("Expected 32 bytes, got {}", result.len()));
        }
        // SHA256(SHA256("")) is a known constant
        // SHA256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        // SHA256(above) = 5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456
        let expected = "5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456";
        if hex::encode(&result) != expected {
            return Err(format!("double_sha256('') mismatch: {}", hex::encode(&result)));
        }
        Ok(())
    });

    // err/10 — BRC-2 encrypt with empty plaintext
    check!(r, "err/10 brc2-empty-plaintext", {
        let sender = [0x42u8; 32];
        let recipient = [0x43u8; 32];
        let recipient_pub = derive_public_key(&recipient).map_err(|e| format!("{}", e))?;
        let sym_key = brc2::derive_symmetric_key(&sender, &recipient_pub, "test")
            .map_err(|e| format!("{}", e))?;
        let encrypted = brc2::encrypt_brc2(&[], &sym_key)
            .map_err(|e| format!("{}", e))?;
        // Should have at minimum IV (32 bytes) + tag (16 bytes) = 48 bytes
        if encrypted.len() < 48 {
            return Err(format!("Expected >=48 bytes for empty plaintext, got {}", encrypted.len()));
        }
        let decrypted = brc2::decrypt_brc2(&encrypted, &sym_key)
            .map_err(|e| format!("{}", e))?;
        if !decrypted.is_empty() {
            return Err("Decrypted empty plaintext should be empty".into());
        }
        Ok(())
    });

    // err/11 — BRC-2 certificate field with mismatched serial
    check!(r, "err/11 brc2-cert-field-serial-mismatch", {
        let sender = [0x42u8; 32];
        let recipient = [0x43u8; 32];
        let sender_pub = derive_public_key(&sender).map_err(|e| format!("{}", e))?;
        let recipient_pub = derive_public_key(&recipient).map_err(|e| format!("{}", e))?;
        let serial = hex::encode(vec![0xAA; 32]);
        // Encrypt with serial
        let encrypted = brc2::encrypt_certificate_field(
            &sender, &recipient_pub, "name", Some(&serial), b"Alice"
        ).map_err(|e| format!("{}", e))?;
        // Decrypt with different serial → should fail (different key derivation)
        let wrong_serial = hex::encode(vec![0xBB; 32]);
        let result = brc2::decrypt_certificate_field(
            &recipient, &sender_pub, "name", Some(&wrong_serial), &encrypted
        );
        if result.is_ok() {
            let decrypted = result.unwrap();
            // Even if it doesn't error, the plaintext should be wrong
            if decrypted == b"Alice" {
                return Err("Decryption with wrong serial should NOT produce correct plaintext".into());
            }
        }
        Ok(())
    });

    // err/12 — PIN derive_key_from_pin with empty PIN
    check!(r, "err/12 pin-empty-pin", {
        use hodos_wallet::crypto::pin;
        let salt = [0u8; 16];
        let key = pin::derive_key_from_pin("", &salt);
        if key.len() != 32 {
            return Err(format!("Expected 32-byte key, got {}", key.len()));
        }
        // Different salt should give different key
        let salt2 = [0x01u8; 16];
        let key2 = pin::derive_key_from_pin("", &salt2);
        if key == key2 {
            return Err("Empty PIN with different salts should give different keys".into());
        }
        Ok(())
    });
}

// ============================================================================
// [7/8] BEEF Error Paths
// ============================================================================

fn test_beef_error_paths(r: &mut TestResults) {
    use hodos_wallet::beef::Beef;

    // beefer/1 — from_bytes with invalid version marker
    check!(r, "beefer/1 invalid-marker", {
        let bytes = vec![0xFF, 0xFF, 0xFF, 0xFF, 0x00]; // Bad marker + 0 bumps
        let result = Beef::from_bytes(&bytes);
        if result.is_ok() {
            return Err("Should reject invalid version marker".into());
        }
        Ok(())
    });

    // beefer/2 — from_bytes with empty input
    check!(r, "beefer/2 empty-input", {
        let result = Beef::from_bytes(&[]);
        if result.is_ok() {
            return Err("Should reject empty input".into());
        }
        Ok(())
    });

    // beefer/3 — from_bytes with truncated data (only marker)
    check!(r, "beefer/3 truncated-marker-only", {
        let result = Beef::from_bytes(&[0x02, 0x00, 0xbe, 0xef]);
        if result.is_ok() {
            return Err("Should reject truncated BEEF (marker only)".into());
        }
        Ok(())
    });

    // beefer/4 — from_hex with invalid hex
    check!(r, "beefer/4 invalid-hex", {
        let result = Beef::from_hex("not-valid-hex");
        if result.is_ok() {
            return Err("Should reject invalid hex".into());
        }
        Ok(())
    });

    // beefer/5 — from_atomic_beef_bytes too short
    check!(r, "beefer/5 atomic-too-short", {
        let result = Beef::from_atomic_beef_bytes(&[0x01, 0x01, 0x01, 0x01, 0x00]);
        if result.is_ok() {
            return Err("Should reject atomic BEEF < 36 bytes".into());
        }
        Ok(())
    });

    // beefer/6 — from_atomic_beef_bytes wrong magic
    check!(r, "beefer/6 atomic-wrong-magic", {
        let mut bytes = vec![0xFF; 36]; // Wrong magic
        // Need at least marker + txid + beef header to attempt parse
        let result = Beef::from_atomic_beef_bytes(&bytes);
        if result.is_ok() {
            return Err("Should reject wrong atomic magic".into());
        }
        Ok(())
    });

    // beefer/7 — extract_raw_tx_hex with empty BEEF
    check!(r, "beefer/7 extract-empty", {
        // Create a valid BEEF V2 with 0 bumps and 0 transactions
        let bytes = vec![
            0x02, 0x00, 0xbe, 0xef, // V2 marker
            0x00,                     // 0 bumps
            0x00,                     // 0 transactions
        ];
        let beef_hex = hex::encode(&bytes);
        let result = Beef::extract_raw_tx_hex(&beef_hex);
        if result.is_ok() {
            return Err("Should fail with no transactions".into());
        }
        Ok(())
    });

    // beefer/8 — to_atomic_beef_hex with wrong-length TXID
    check!(r, "beefer/8 atomic-wrong-txid-length", {
        let beef = Beef::new();
        let result = beef.to_atomic_beef_hex("aabb"); // Only 2 bytes
        if result.is_ok() {
            return Err("Should reject TXID that's not 32 bytes".into());
        }
        Ok(())
    });

    // beefer/9 — from_atomic_beef_base64 with invalid base64
    check!(r, "beefer/9 atomic-invalid-base64", {
        let result = Beef::from_atomic_beef_base64("!!!not-base64!!!");
        if result.is_ok() {
            return Err("Should reject invalid base64".into());
        }
        Ok(())
    });

    // beefer/10 — ParsedTransaction::from_hex with invalid hex
    check!(r, "beefer/10 parsed-tx-invalid-hex", {
        let result = hodos_wallet::beef::ParsedTransaction::from_hex("zzz");
        if result.is_ok() {
            return Err("Should reject invalid hex".into());
        }
        Ok(())
    });

    // beefer/11 — ParsedTransaction::from_hex with truncated tx
    check!(r, "beefer/11 parsed-tx-truncated", {
        let result = hodos_wallet::beef::ParsedTransaction::from_hex("01000000");
        if result.is_ok() {
            return Err("Should reject truncated transaction".into());
        }
        Ok(())
    });
}

// ============================================================================
// [8/8] Recovery & Misc Coverage
// ============================================================================

fn test_recovery_misc(r: &mut TestResults) {
    use hodos_wallet::recovery;

    // rec/1 — address_to_p2pkh_script with valid mainnet address
    check!(r, "rec/1 p2pkh-valid-address", {
        // Use a known BIP-32 Test Vector 1 derived address
        let seed = hex::decode("000102030405060708090a0b0c0d0e0f").unwrap();
        let (address, _pubkey, _privkey) = recovery::derive_address_at_path(
            &seed, &[(0, true)]
        ).map_err(|e| format!("{}", e))?;
        let script = recovery::address_to_p2pkh_script(&address)
            .map_err(|e| format!("{}", e))?;
        // P2PKH script is 25 bytes: OP_DUP OP_HASH160 <20> <hash> OP_EQUALVERIFY OP_CHECKSIG
        if script.len() != 25 {
            return Err(format!("Expected 25-byte P2PKH, got {}", script.len()));
        }
        if script[0] != 0x76 || script[1] != 0xa9 || script[2] != 0x14 {
            return Err("Script prefix should be 76 a9 14".into());
        }
        if script[23] != 0x88 || script[24] != 0xac {
            return Err("Script suffix should be 88 ac".into());
        }
        Ok(())
    });

    // rec/2 — deep BIP-32 derivation path (5 levels)
    check!(r, "rec/2 deep-bip32-path", {
        let seed = hex::decode("000102030405060708090a0b0c0d0e0f").unwrap();
        let deep_path = vec![(0, true), (1, false), (2, true), (3, false), (4, true)];
        let key = recovery::derive_key_at_path(&seed, &deep_path)
            .map_err(|e| format!("{}", e))?;
        if key.len() != 32 {
            return Err(format!("Expected 32-byte key, got {}", key.len()));
        }
        // Should be deterministic
        let key2 = recovery::derive_key_at_path(&seed, &deep_path)
            .map_err(|e| format!("{}", e))?;
        if key != key2 {
            return Err("Deep derivation not deterministic".into());
        }
        Ok(())
    });

    // rec/3 — different paths give different keys
    check!(r, "rec/3 different-paths-differ", {
        let seed = hex::decode("000102030405060708090a0b0c0d0e0f").unwrap();
        let key1 = recovery::derive_key_at_path(&seed, &[(0, true)])
            .map_err(|e| format!("{}", e))?;
        let key2 = recovery::derive_key_at_path(&seed, &[(1, true)])
            .map_err(|e| format!("{}", e))?;
        if key1 == key2 {
            return Err("Different paths should give different keys".into());
        }
        Ok(())
    });

    // rec/4 — hardened vs non-hardened derivation differ
    check!(r, "rec/4 hardened-vs-normal", {
        let seed = hex::decode("000102030405060708090a0b0c0d0e0f").unwrap();
        let hardened = recovery::derive_key_at_path(&seed, &[(0, true)])
            .map_err(|e| format!("{}", e))?;
        let normal = recovery::derive_key_at_path(&seed, &[(0, false)])
            .map_err(|e| format!("{}", e))?;
        if hardened == normal {
            return Err("Hardened and normal derivation should differ".into());
        }
        Ok(())
    });

    // rec/5 — ExternalWalletConfig::centbee paths
    check!(r, "rec/5 centbee-config", {
        let config = recovery::ExternalWalletConfig::centbee();
        if config.name != "centbee" {
            return Err(format!("Expected 'centbee', got '{}'", config.name));
        }
        if config.chains.is_empty() {
            return Err("Should have chains".into());
        }
        if config.chains.len() < 2 {
            return Err(format!("Expected at least 2 chains (receive+change), got {}", config.chains.len()));
        }
        Ok(())
    });

    // rec/6 — Certificate types: is_active
    check!(r, "rec/6 cert-is-active", {
        use hodos_wallet::certificate::types::{Certificate, CertificateField};
        let cert = Certificate::new(
            vec![0; 32], vec![0x02; 33], vec![0; 32], vec![0x03; 33],
            "txid.0".into(), vec![0x30], HashMap::new(), HashMap::new(),
        );
        if !cert.is_active() {
            return Err("New cert should be active".into());
        }
        let mut deleted_cert = cert.clone();
        deleted_cert.is_deleted = true;
        if deleted_cert.is_active() {
            return Err("Deleted cert should not be active".into());
        }
        Ok(())
    });

    // rec/7 — Certificate identifier returns correct refs
    check!(r, "rec/7 cert-identifier", {
        use hodos_wallet::certificate::types::Certificate;
        let type_ = vec![0xAA; 32];
        let serial = vec![0xBB; 32];
        let certifier = vec![0x02; 33];
        let cert = Certificate::new(
            type_.clone(), vec![0x03; 33], serial.clone(), certifier.clone(),
            "txid.0".into(), vec![0x30], HashMap::new(), HashMap::new(),
        );
        let (t, s, c) = cert.identifier();
        if t != type_.as_slice() { return Err("type mismatch".into()); }
        if s != serial.as_slice() { return Err("serial mismatch".into()); }
        if c != certifier.as_slice() { return Err("certifier mismatch".into()); }
        Ok(())
    });

    // rec/8 — Certificate timestamps
    check!(r, "rec/8 cert-timestamps", {
        use hodos_wallet::certificate::types::Certificate;
        let cert = Certificate::new(
            vec![0; 32], vec![0x02; 33], vec![0; 32], vec![0x03; 33],
            "txid.0".into(), vec![0x30], HashMap::new(), HashMap::new(),
        );
        if cert.created_at <= 0 {
            return Err(format!("created_at should be positive, got {}", cert.created_at));
        }
        if cert.created_at != cert.updated_at {
            return Err("created_at should equal updated_at for new cert".into());
        }
        Ok(())
    });

    // rec/9 — CertificateField timestamps
    check!(r, "rec/9 cert-field-timestamps", {
        use hodos_wallet::certificate::types::CertificateField;
        let field = CertificateField::new(
            "email".into(), b"encrypted".to_vec(), b"key".to_vec()
        );
        if field.created_at <= 0 {
            return Err("created_at should be positive".into());
        }
        if field.certificate_id.is_some() {
            return Err("New field should have None certificate_id".into());
        }
        if field.user_id.is_some() {
            return Err("New field should have None user_id".into());
        }
        Ok(())
    });

    // rec/10 — PriceCache: new returns None
    check!(r, "rec/10 price-cache-empty", {
        use hodos_wallet::price_cache::PriceCache;
        let cache = PriceCache::new();
        if cache.get_cached().is_some() {
            return Err("New PriceCache should return None for get_cached".into());
        }
        if cache.get_stale().is_some() {
            return Err("New PriceCache should return None for get_stale".into());
        }
        Ok(())
    });

    // rec/11 — BRC-43 SecurityLevel from_u8 covers all levels
    check!(r, "rec/11 brc43-security-all-levels", {
        use hodos_wallet::crypto::brc43::SecurityLevel;
        // Level 0 = NoPermissions (Master)
        let l0 = SecurityLevel::from_u8(0);
        if l0.is_none() {
            return Err("Level 0 should be valid".into());
        }
        if l0.unwrap().as_u8() != 0 {
            return Err("Level 0 as_u8 should return 0".into());
        }
        // Level 1 = ProtocolLevel
        let l1 = SecurityLevel::from_u8(1);
        if l1.is_none() || l1.unwrap().as_u8() != 1 {
            return Err("Level 1 mismatch".into());
        }
        // Level 2 = CounterpartyLevel
        let l2 = SecurityLevel::from_u8(2);
        if l2.is_none() || l2.unwrap().as_u8() != 2 {
            return Err("Level 2 mismatch".into());
        }
        // Level 3+ = invalid
        let l3 = SecurityLevel::from_u8(3);
        if l3.is_some() {
            return Err("Level 3 should be invalid".into());
        }
        Ok(())
    });

    // rec/12 — BRC-43 normalize special chars rejected, unicode alphanumeric allowed
    check!(r, "rec/12 brc43-special-chars-rejected", {
        use hodos_wallet::crypto::brc43::normalize_protocol_id;
        // Special characters like @#$ should be rejected
        let result = normalize_protocol_id("hello@world");
        if result.is_ok() {
            return Err("Should reject special characters like @".into());
        }
        // Unicode alphanumeric (ö is alphanumeric) is accepted by is_alphanumeric()
        let result2 = normalize_protocol_id("hello wörld");
        if result2.is_err() {
            return Err(format!("Unicode alphanumeric should be accepted: {}", result2.unwrap_err()));
        }
        Ok(())
    });

    // rec/13 — varint boundary values
    check!(r, "rec/13 varint-boundaries", {
        use hodos_wallet::transaction::{encode_varint, decode_varint};
        let boundaries: Vec<(u64, usize)> = vec![
            (0, 1),           // smallest
            (0xFC, 1),        // last 1-byte
            (0xFD, 3),        // first 2-byte
            (0xFFFF, 3),      // last 2-byte
            (0x10000, 5),     // first 4-byte
            (0xFFFFFFFF, 5),  // last 4-byte
            (0x100000000, 9), // first 8-byte
        ];
        for (val, expected_len) in boundaries {
            let encoded = encode_varint(val);
            if encoded.len() != expected_len {
                return Err(format!("Varint {} encoded to {} bytes, expected {}",
                    val, encoded.len(), expected_len));
            }
            let (decoded, consumed) = decode_varint(&encoded)
                .map_err(|e| format!("Decode varint {}: {}", val, e))?;
            if decoded != val {
                return Err(format!("Varint roundtrip failed: {} -> {}", val, decoded));
            }
            if consumed != expected_len {
                return Err(format!("Consumed {} bytes, expected {}", consumed, expected_len));
            }
        }
        Ok(())
    });
}
