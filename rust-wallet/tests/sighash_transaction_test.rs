//! SigHash & Transaction Diagnostic Test Suite
//!
//! Tests BSV ForkID SIGHASH calculation and transaction serialization
//! against canonical test vectors.
//!
//! Sources:
//!   - bitcoin-sv/bitcoin-sv sighash.json (1000 vectors, 514 with FORKID)
//!   - BIP-143 reference preimage vectors
//!
//! Run with: cargo test sighash_transaction -- --nocapture

use std::panic;

// Import our wallet crate
use hodos_wallet::transaction::{
    self, Transaction, TxInput, TxOutput, OutPoint,
    calculate_sighash,
};

/// Test result for diagnostic reporting
#[derive(Debug)]
enum DiagResult {
    Pass,
    WrongOutput { expected: String, got: String },
    Panic(String),
    Error(String),
    Skip(String),
}

impl DiagResult {
    fn symbol(&self) -> &'static str {
        match self {
            DiagResult::Pass => "PASS ",
            DiagResult::WrongOutput { .. } => "FAIL ",
            DiagResult::Panic(_) => "PANIC",
            DiagResult::Error(_) => "ERROR",
            DiagResult::Skip(_) => "SKIP ",
        }
    }

    fn is_pass(&self) -> bool {
        matches!(self, DiagResult::Pass)
    }

    fn is_skip(&self) -> bool {
        matches!(self, DiagResult::Skip(_))
    }
}

/// Run a test closure, catching any panics
fn run_test<F>(test_fn: F) -> DiagResult
where
    F: FnOnce() -> DiagResult + panic::UnwindSafe,
{
    match panic::catch_unwind(test_fn) {
        Ok(result) => result,
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic".to_string()
            };
            DiagResult::Panic(msg)
        }
    }
}

fn print_result(section: &str, num: usize, result: &DiagResult) {
    match result {
        DiagResult::Pass => {
            // Only print failures to keep output manageable for 500+ vectors
        }
        DiagResult::WrongOutput { expected, got } => {
            println!("    [{}] {}/{}", result.symbol(), section, num);
            println!("           expected: {}", &expected[..expected.len().min(64)]);
            println!("           got:      {}", &got[..got.len().min(64)]);
        }
        DiagResult::Panic(msg) => {
            println!("    [{}] {}/{} — {}", result.symbol(), section, num, &msg[..msg.len().min(120)]);
        }
        DiagResult::Error(msg) => {
            println!("    [{}] {}/{} — {}", result.symbol(), section, num, &msg[..msg.len().min(120)]);
        }
        DiagResult::Skip(msg) => {
            // Don't print skips to keep output clean
            let _ = msg;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Transaction parser (test-only, parses raw tx hex into our Transaction struct)
// ═══════════════════════════════════════════════════════════════════════════

/// Parse a raw transaction hex string into a Transaction struct.
/// This is a test-only utility; the main codebase builds transactions from fields.
fn parse_raw_tx(raw_hex: &str) -> Result<Transaction, String> {
    let bytes = hex::decode(raw_hex)
        .map_err(|e| format!("hex decode: {}", e))?;

    if bytes.len() < 10 {
        return Err("tx too short".into());
    }

    let mut pos = 0;

    // Version (4 bytes LE)
    let version = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
    pos += 4;

    // Input count (varint)
    let (num_inputs, vlen) = decode_varint_raw(&bytes[pos..])
        .map_err(|e| format!("input count varint: {}", e))?;
    pos += vlen;

    let mut inputs = Vec::with_capacity(num_inputs as usize);
    for _ in 0..num_inputs {
        if pos + 36 > bytes.len() {
            return Err("truncated input outpoint".into());
        }

        // txid (32 bytes, stored as little-endian in wire format)
        // We need to reverse for our hex-string representation (display format)
        let txid_bytes: Vec<u8> = bytes[pos..pos + 32].iter().rev().cloned().collect();
        let txid = hex::encode(&txid_bytes);
        pos += 32;

        // vout (4 bytes LE)
        let vout = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
        pos += 4;

        let outpoint = OutPoint::new(&txid, vout);

        // script_sig (varint length + data)
        let (script_len, vlen) = decode_varint_raw(&bytes[pos..])
            .map_err(|e| format!("script_sig varint: {}", e))?;
        pos += vlen;

        if pos + script_len as usize > bytes.len() {
            return Err("truncated script_sig".into());
        }
        let script_sig = bytes[pos..pos + script_len as usize].to_vec();
        pos += script_len as usize;

        // sequence (4 bytes LE)
        if pos + 4 > bytes.len() {
            return Err("truncated sequence".into());
        }
        let sequence = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
        pos += 4;

        let mut input = TxInput::new(outpoint);
        input.script_sig = script_sig;
        input.sequence = sequence;
        inputs.push(input);
    }

    // Output count (varint)
    let (num_outputs, vlen) = decode_varint_raw(&bytes[pos..])
        .map_err(|e| format!("output count varint: {}", e))?;
    pos += vlen;

    let mut outputs = Vec::with_capacity(num_outputs as usize);
    for _ in 0..num_outputs {
        if pos + 8 > bytes.len() {
            return Err("truncated output value".into());
        }

        // value (8 bytes LE, i64)
        let value = i64::from_le_bytes([
            bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3],
            bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7],
        ]);
        pos += 8;

        // script_pubkey (varint length + data)
        let (script_len, vlen) = decode_varint_raw(&bytes[pos..])
            .map_err(|e| format!("script_pubkey varint: {}", e))?;
        pos += vlen;

        if pos + script_len as usize > bytes.len() {
            return Err("truncated script_pubkey".into());
        }
        let script_pubkey = bytes[pos..pos + script_len as usize].to_vec();
        pos += script_len as usize;

        outputs.push(TxOutput::new(value, script_pubkey));
    }

    // Locktime (4 bytes LE)
    if pos + 4 > bytes.len() {
        return Err("truncated locktime".into());
    }
    let lock_time = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);

    let mut tx = Transaction::new();
    tx.version = version;
    tx.inputs = inputs;
    tx.outputs = outputs;
    tx.lock_time = lock_time;

    Ok(tx)
}

/// Decode a Bitcoin varint from raw bytes. Returns (value, bytes_consumed).
fn decode_varint_raw(data: &[u8]) -> Result<(u64, usize), String> {
    if data.is_empty() {
        return Err("empty varint".into());
    }
    match data[0] {
        0..=0xFC => Ok((data[0] as u64, 1)),
        0xFD => {
            if data.len() < 3 { return Err("truncated varint fd".into()); }
            Ok((u16::from_le_bytes([data[1], data[2]]) as u64, 3))
        }
        0xFE => {
            if data.len() < 5 { return Err("truncated varint fe".into()); }
            Ok((u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as u64, 5))
        }
        0xFF => {
            if data.len() < 9 { return Err("truncated varint ff".into()); }
            Ok((u64::from_le_bytes([
                data[1], data[2], data[3], data[4],
                data[5], data[6], data[7], data[8],
            ]), 9))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SigHash test helper
// ═══════════════════════════════════════════════════════════════════════════

/// Test a single sighash vector from bitcoin-sv sighash.json
fn test_sighash_vector(
    raw_tx_hex: &str,
    script_hex: &str,
    input_index: usize,
    hash_type: i64,
    expected_hash: &str,
) -> DiagResult {
    run_test(move || {
        // Step 1: Parse the transaction
        let tx = match parse_raw_tx(raw_tx_hex) {
            Ok(tx) => tx,
            Err(e) => return DiagResult::Error(format!("tx parse: {}", e)),
        };

        // Step 2: Decode the script
        let script = if script_hex.is_empty() {
            vec![]
        } else {
            match hex::decode(script_hex) {
                Ok(s) => s,
                Err(e) => return DiagResult::Error(format!("script hex: {}", e)),
            }
        };

        // Step 3: Convert hash_type to u32
        // The bitcoin-sv vectors use signed integers that can be negative
        // We need to treat them as u32 (taking lower 32 bits)
        let sighash_type = hash_type as u32;

        // Step 4: Check if input_index is valid
        if input_index >= tx.inputs.len() {
            return DiagResult::Skip(format!("input_index {} >= inputs {}", input_index, tx.inputs.len()));
        }

        // Step 5: Calculate sighash (prev_value=0, matching bitcoin-sv test harness)
        let result = match calculate_sighash(&tx, input_index, &script, 0, sighash_type) {
            Ok(hash) => hash,
            Err(e) => return DiagResult::Error(format!("sighash calc: {}", e)),
        };

        // Step 6: Compare
        // bitcoin-sv sighash.json uses uint256::GetHex() which reverses byte order.
        // Our hash is in raw SHA256 output order, so we reverse for comparison.
        let result_reversed: Vec<u8> = result.iter().rev().cloned().collect();
        let got_hex = hex::encode(&result_reversed);
        if got_hex == expected_hash {
            DiagResult::Pass
        } else {
            // Also check non-reversed in case some vectors use raw order
            let got_raw = hex::encode(&result);
            if got_raw == expected_hash {
                DiagResult::Pass
            } else {
                DiagResult::WrongOutput {
                    expected: expected_hash.to_string(),
                    got: format!("{} (reversed: {})", got_hex, got_raw),
                }
            }
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// Transaction serialization test helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Test that parse → serialize roundtrips correctly
fn test_tx_roundtrip(raw_hex: &str) -> DiagResult {
    run_test(move || {
        // Parse
        let tx = match parse_raw_tx(raw_hex) {
            Ok(tx) => tx,
            Err(e) => return DiagResult::Error(format!("tx parse: {}", e)),
        };

        // Serialize back
        let serialized = match tx.to_hex() {
            Ok(h) => h,
            Err(e) => return DiagResult::Error(format!("tx serialize: {}", e)),
        };

        // Compare
        if serialized == raw_hex {
            DiagResult::Pass
        } else {
            // Find first difference
            let diff_pos = serialized.chars().zip(raw_hex.chars())
                .position(|(a, b)| a != b)
                .unwrap_or(serialized.len().min(raw_hex.len()));
            DiagResult::WrongOutput {
                expected: format!("len={} ...{}...", raw_hex.len(), &raw_hex[diff_pos.saturating_sub(8)..raw_hex.len().min(diff_pos + 16)]),
                got: format!("len={} ...{}...", serialized.len(), &serialized[diff_pos.saturating_sub(8)..serialized.len().min(diff_pos + 16)]),
            }
        }
    })
}

/// Test transaction ID calculation
fn test_txid(raw_hex: &str, expected_txid: &str) -> DiagResult {
    run_test(move || {
        let tx = match parse_raw_tx(raw_hex) {
            Ok(tx) => tx,
            Err(e) => return DiagResult::Error(format!("tx parse: {}", e)),
        };

        let txid = match tx.txid() {
            Ok(t) => t,
            Err(e) => return DiagResult::Error(format!("txid calc: {}", e)),
        };

        if txid == expected_txid {
            DiagResult::Pass
        } else {
            DiagResult::WrongOutput {
                expected: expected_txid.to_string(),
                got: txid,
            }
        }
    })
}

/// Test varint encoding
fn test_varint_encode(value: u64, expected_hex: &str) -> DiagResult {
    run_test(move || {
        let encoded = transaction::encode_varint(value);
        let got_hex = hex::encode(&encoded);
        if got_hex == expected_hex {
            DiagResult::Pass
        } else {
            DiagResult::WrongOutput {
                expected: expected_hex.to_string(),
                got: got_hex,
            }
        }
    })
}

/// Test varint roundtrip
fn test_varint_roundtrip(value: u64) -> DiagResult {
    run_test(move || {
        let encoded = transaction::encode_varint(value);
        match transaction::decode_varint(&encoded) {
            Ok((decoded, consumed)) => {
                if decoded != value {
                    return DiagResult::WrongOutput {
                        expected: format!("value={}", value),
                        got: format!("value={}", decoded),
                    };
                }
                if consumed != encoded.len() {
                    return DiagResult::WrongOutput {
                        expected: format!("consumed={}", encoded.len()),
                        got: format!("consumed={}", consumed),
                    };
                }
                DiagResult::Pass
            }
            Err(e) => DiagResult::Error(format!("decode: {}", e)),
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// BIP-143 preimage verification helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Convert a wire-format txid (as shown in BIP-143 preimages and serialized txs)
/// to display format (as used by OutPoint::new and block explorers).
/// This is a simple byte reversal of the hex string.
fn wire_to_display_txid(wire_hex: &str) -> String {
    let bytes = hex::decode(wire_hex).expect("invalid txid hex");
    let reversed: Vec<u8> = bytes.iter().rev().cloned().collect();
    hex::encode(&reversed)
}

/// Test sighash against a BIP-143 reference vector with known preimage.
/// These are the gold-standard vectors from the BIP-143 specification.
/// We adapt them for BSV by using the same algorithm (BSV ForkID sighash IS BIP143).
fn test_bip143_sighash(
    description: &str,
    version: u32,
    inputs: Vec<(&str, u32, u32)>,  // (txid_hex in WIRE format, vout, sequence)
    outputs: Vec<(i64, &str)>,      // (value, script_pubkey_hex)
    lock_time: u32,
    input_index: usize,
    script_code_hex: &str,
    prev_value: i64,
    sighash_type: u32,
    expected_hash: &str,
) -> DiagResult {
    run_test(move || {
        let mut tx = Transaction::new();
        tx.version = version;
        tx.lock_time = lock_time;

        for (txid_wire, vout, sequence) in &inputs {
            // BIP-143 shows txids in wire format; OutPoint::new expects display format
            let txid_display = wire_to_display_txid(txid_wire);
            let outpoint = OutPoint::new(&txid_display, *vout);
            let mut input = TxInput::new(outpoint);
            input.sequence = *sequence;
            tx.add_input(input);
        }

        for (value, script_hex) in &outputs {
            let script = match hex::decode(script_hex) {
                Ok(s) => s,
                Err(e) => return DiagResult::Error(format!("{}: script hex: {}", description, e)),
            };
            tx.add_output(TxOutput::new(*value, script));
        }

        let script_code = match hex::decode(script_code_hex) {
            Ok(s) => s,
            Err(e) => return DiagResult::Error(format!("{}: script_code hex: {}", description, e)),
        };

        match calculate_sighash(&tx, input_index, &script_code, prev_value, sighash_type) {
            Ok(hash) => {
                let got = hex::encode(&hash);
                if got == expected_hash {
                    DiagResult::Pass
                } else {
                    DiagResult::WrongOutput {
                        expected: expected_hash.to_string(),
                        got,
                    }
                }
            }
            Err(e) => DiagResult::Error(format!("{}: {}", description, e)),
        }
    })
}


// ═══════════════════════════════════════════════════════════════════════════
// Main Diagnostic Test
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sighash_transaction_diagnostic() {
    println!("\n");
    println!("=========================================================================");
    println!("  HODOS WALLET — SIGHASH & TRANSACTION DIAGNOSTIC");
    println!("  Testing BSV ForkID SIGHASH against canonical vectors");
    println!("=========================================================================");

    let mut total_pass = 0u32;
    let mut total_fail = 0u32;
    let mut total_skip = 0u32;

    // Helper macro
    macro_rules! check {
        ($section:expr, $num:expr, $result:expr) => {{
            let r = $result;
            print_result($section, $num, &r);
            if r.is_pass() { total_pass += 1; }
            else if r.is_skip() { total_skip += 1; }
            else { total_fail += 1; }
        }};
    }

    // ─── 1. Varint Encoding/Decoding ─────────────────────────────────────
    println!("\n  [1/5] Varint Encoding");

    check!("varint-enc", 1, test_varint_encode(0, "00"));
    check!("varint-enc", 2, test_varint_encode(1, "01"));
    check!("varint-enc", 3, test_varint_encode(252, "fc"));
    check!("varint-enc", 4, test_varint_encode(253, "fdfd00"));
    check!("varint-enc", 5, test_varint_encode(254, "fdfe00"));
    check!("varint-enc", 6, test_varint_encode(255, "fdff00"));
    check!("varint-enc", 7, test_varint_encode(0xffff, "fdffff"));
    check!("varint-enc", 8, test_varint_encode(0x10000, "fe00000100"));
    check!("varint-enc", 9, test_varint_encode(0xffffffff, "feffffffff"));
    check!("varint-enc", 10, test_varint_encode(0x100000000, "ff0000000001000000"));

    // Varint roundtrip
    check!("varint-rt", 1, test_varint_roundtrip(0));
    check!("varint-rt", 2, test_varint_roundtrip(100));
    check!("varint-rt", 3, test_varint_roundtrip(252));
    check!("varint-rt", 4, test_varint_roundtrip(253));
    check!("varint-rt", 5, test_varint_roundtrip(65535));
    check!("varint-rt", 6, test_varint_roundtrip(65536));
    check!("varint-rt", 7, test_varint_roundtrip(0xFFFFFFFF));
    check!("varint-rt", 8, test_varint_roundtrip(0x100000000));

    // ─── 2. Transaction Serialization Roundtrip ──────────────────────────
    println!("\n  [2/5] Transaction Serialization Roundtrip");
    println!("         parse(hex) → serialize() should reproduce original hex");

    // Bitcoin genesis coinbase transaction (parse/serialize roundtrip)
    check!("tx-rt", 1, test_tx_roundtrip(
        "01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff4d04ffff001d0104455468652054696d65732030332f4a616e2f32303039204368616e63656c6c6f72206f6e206272696e6b206f66207365636f6e64206261696c6f757420666f722062616e6b73ffffffff0100f2052a0100000043410496b538e853519c726a2c91e61ec11600ae1390813a627c66fb8be7947be63c52da7589379515d4e0a604f8141781e62294721166bf621e73a82cbf2342c858eeac00000000"
    ));

    // Use first sighash vector as second roundtrip test
    // (it will be loaded below but test roundtrip before sighash)

    // Load sighash.json vectors and test a few roundtrips
    let sighash_json_str = include_str!("fixtures/sighash_vectors.json");
    let sighash_vectors: serde_json::Value = serde_json::from_str(sighash_json_str)
        .expect("Failed to parse sighash_vectors.json");

    let vectors: Vec<&serde_json::Value> = sighash_vectors.as_array()
        .expect("sighash.json should be an array")
        .iter()
        .filter(|v| v.is_array() && v.as_array().map(|a| a.len() >= 5).unwrap_or(false))
        .collect();

    println!("         Loaded {} sighash vectors, testing roundtrip on first 20...", vectors.len());

    for (i, v) in vectors.iter().take(20).enumerate() {
        let arr = v.as_array().unwrap();
        let raw_tx = arr[0].as_str().unwrap();
        check!("tx-rt-v", i + 1, test_tx_roundtrip(raw_tx));
    }

    // ─── 3. Transaction ID Calculation ───────────────────────────────────
    println!("\n  [3/5] Transaction ID Calculation");

    // Known transaction with verified txid
    // Bitcoin genesis coinbase tx (Satoshi's "The Times" message)
    check!("txid", 1, test_txid(
        "01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff4d04ffff001d0104455468652054696d65732030332f4a616e2f32303039204368616e63656c6c6f72206f6e206272696e6b206f66207365636f6e64206261696c6f757420666f722062616e6b73ffffffff0100f2052a01000000434104678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5fac00000000",
        "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
    ));

    // ─── 4. BIP-143 Preimage Verification ────────────────────────────────
    println!("\n  [4/5] BIP-143 Preimage Verification");
    println!("         Gold-standard vectors with known preimage data");
    println!("         Algorithm is identical to BSV ForkID SIGHASH");

    // BIP-143 Example 1: Native P2WPKH (second input)
    // We construct the tx from known field data and verify sighash
    check!("bip143", 1, test_bip143_sighash(
        "P2WPKH ALL",
        1, // version
        vec![
            // Input 0
            ("fff7f7881a8099afa6940d42d1e7f6362bec38171ea3edf433541db4e4ad969f", 0, 0xFFFFFFEEu32),
            // Input 1 (the one being signed)
            ("ef51e1b804cc89d182d279655c3aa89e815b1b309fe287d9b2b55d57b90ec68a", 1, 0xffffffff),
        ],
        vec![
            (112340000i64, "76a9148280b37df378db99f66f85c95a783a76ac7a6d5988ac"),
            (223450000i64, "76a9143bde42dbee7e4dbe6a21b2d50ce2f0167faa815988ac"),
        ],
        17, // locktime = 0x11
        1,  // signing input 1
        "76a9141d0f172a0ecb48aee1be1f2687d2963ae33f71a188ac", // scriptCode (raw, without length prefix)
        600000000, // 6 BTC
        0x01, // SIGHASH_ALL (BIP143 uses raw type, no FORKID flag needed for the algorithm)
        "c37af31116d1b27caf68aae9e3ac82f1477929014d5b917657d0eb49478cb670",
    ));

    // BIP-143 Example 5: P2SH-P2WSH 6-of-6 multisig — SIGHASH_ALL
    // BIP143 scriptCode: raw script bytes WITHOUT the varint length prefix.
    // Our calculate_sighash() adds the length prefix automatically.
    let multisig_script_code = "56210307b8ae49ac90a048e9b53357a2354b3334e9c8bee813ecb98e99a7e07e8c3ba32103b28f0c28bfab54554ae8c658ac5c3e0ce6e79ad336331f78c428dd43eea8449b21034b8113d703413d57761b8b9781957b8c0ac1dfe69f492580ca4195f50376ba4a21033400f6afecb833092a9a21cfdf1ed1376e58c5d1f47de74683123987e967a8f42103a6d48b1131e94ba04d9737d61acdaa1322008af9602b3b14862c07a1789aac162102d8b661b0b3302ee2f162b09e07a55ad5dfbe673a9f01d9f0c19617681024306b56ae";

    check!("bip143", 2, test_bip143_sighash(
        "6-of-6 ALL",
        1,
        vec![
            ("36641869ca081e70f394c6948e8af409e18b619df2ed74aa106c1ca29787b96e", 1, 0xffffffff),
        ],
        vec![
            (900000000i64, "76a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe2688ac"),
            (87000000i64, "76a9147480a33f950689af511e6e84c138dbbd3c3ee41588ac"),
        ],
        0, // locktime
        0, // input index
        multisig_script_code,
        987654321, // 9.87654321 BTC
        0x01, // SIGHASH_ALL
        "185c0be5263dce5b4bb50a047973c1b6272bfbd0103a89444597dc40b248ee7c",
    ));

    // Same tx, SIGHASH_NONE (0x02)
    check!("bip143", 3, test_bip143_sighash(
        "6-of-6 NONE",
        1,
        vec![
            ("36641869ca081e70f394c6948e8af409e18b619df2ed74aa106c1ca29787b96e", 1, 0xffffffff),
        ],
        vec![
            (900000000i64, "76a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe2688ac"),
            (87000000i64, "76a9147480a33f950689af511e6e84c138dbbd3c3ee41588ac"),
        ],
        0,
        0,
        multisig_script_code,
        987654321,
        0x02, // SIGHASH_NONE
        "e9733bc60ea13c95c6527066bb975a2ff29a925e80aa14c213f686cbae5d2f36",
    ));

    // SIGHASH_SINGLE (0x03)
    check!("bip143", 4, test_bip143_sighash(
        "6-of-6 SINGLE",
        1,
        vec![
            ("36641869ca081e70f394c6948e8af409e18b619df2ed74aa106c1ca29787b96e", 1, 0xffffffff),
        ],
        vec![
            (900000000i64, "76a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe2688ac"),
            (87000000i64, "76a9147480a33f950689af511e6e84c138dbbd3c3ee41588ac"),
        ],
        0,
        0,
        multisig_script_code,
        987654321,
        0x03, // SIGHASH_SINGLE
        "1e1f1c303dc025bd664acb72e583e933fae4cff9148bf78c157d1e8f78530aea",
    ));

    // SIGHASH_ALL | ANYONECANPAY (0x81)
    check!("bip143", 5, test_bip143_sighash(
        "6-of-6 ALL|ACP",
        1,
        vec![
            ("36641869ca081e70f394c6948e8af409e18b619df2ed74aa106c1ca29787b96e", 1, 0xffffffff),
        ],
        vec![
            (900000000i64, "76a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe2688ac"),
            (87000000i64, "76a9147480a33f950689af511e6e84c138dbbd3c3ee41588ac"),
        ],
        0,
        0,
        multisig_script_code,
        987654321,
        0x81, // SIGHASH_ALL | ANYONECANPAY
        "2a67f03e63a6a422125878b40b82da593be8d4efaafe88ee528af6e5a9955c6e",
    ));

    // SIGHASH_NONE | ANYONECANPAY (0x82)
    check!("bip143", 6, test_bip143_sighash(
        "6-of-6 NONE|ACP",
        1,
        vec![
            ("36641869ca081e70f394c6948e8af409e18b619df2ed74aa106c1ca29787b96e", 1, 0xffffffff),
        ],
        vec![
            (900000000i64, "76a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe2688ac"),
            (87000000i64, "76a9147480a33f950689af511e6e84c138dbbd3c3ee41588ac"),
        ],
        0,
        0,
        multisig_script_code,
        987654321,
        0x82, // SIGHASH_NONE | ANYONECANPAY
        "781ba15f3779d5542ce8ecb5c18716733a5ee42a6f51488ec96154934e2c890a",
    ));

    // SIGHASH_SINGLE | ANYONECANPAY (0x83)
    check!("bip143", 7, test_bip143_sighash(
        "6-of-6 SINGLE|ACP",
        1,
        vec![
            ("36641869ca081e70f394c6948e8af409e18b619df2ed74aa106c1ca29787b96e", 1, 0xffffffff),
        ],
        vec![
            (900000000i64, "76a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe2688ac"),
            (87000000i64, "76a9147480a33f950689af511e6e84c138dbbd3c3ee41588ac"),
        ],
        0,
        0,
        multisig_script_code,
        987654321,
        0x83, // SIGHASH_SINGLE | ANYONECANPAY
        "511e8e52ed574121fc1b654970395502128263f62662e076dc6baf05c2e6a99b",
    ));

    // ─── 5. Bitcoin-SV sighash.json Vectors (ForkID) ─────────────────────
    println!("\n  [5/5] Bitcoin-SV sighash.json ForkID Vectors");

    let mut forkid_tested = 0u32;
    let mut non_forkid_skipped = 0u32;

    for (i, v) in vectors.iter().enumerate() {
        let arr = v.as_array().unwrap();
        let raw_tx = arr[0].as_str().unwrap();
        let script = arr[1].as_str().unwrap();
        let input_idx = arr[2].as_i64().unwrap() as usize;
        let hash_type = arr[3].as_i64().unwrap();
        // Position 4 is the hash computed with SCRIPT_ENABLE_SIGHASH_FORKID flag
        // Position 5 is the hash computed with the legacy algorithm (no FORKID)
        // We implement the BIP143/ForkID algorithm, so compare against position 4
        let expected_hash = arr[4].as_str().unwrap();

        // Only test vectors with FORKID flag set AND Chronicle NOT set
        // - 0x40 = SIGHASH_FORKID (uses BIP143 algorithm)
        // - 0x20 = SIGHASH_CHRONICLE (modifies behavior, not implemented in our wallet)
        // When Chronicle is set with FORKID, the node falls back to legacy behavior.
        // Our wallet only uses standard FORKID (0x41 = ALL|FORKID), so skip Chronicle.
        let hash_type_u32 = hash_type as u32;
        if hash_type_u32 & 0x40 == 0 {
            non_forkid_skipped += 1;
            continue;
        }
        if hash_type_u32 & 0x20 != 0 {
            // SIGHASH_CHRONICLE set — skip (our wallet doesn't implement Chronicle)
            non_forkid_skipped += 1;
            continue;
        }

        forkid_tested += 1;
        check!("sighash-forkid", i + 1, test_sighash_vector(
            raw_tx, script, input_idx, hash_type, expected_hash
        ));
    }

    println!("         Tested {} ForkID vectors, skipped {} non-ForkID", forkid_tested, non_forkid_skipped);

    // ─── Summary ─────────────────────────────────────────────────────────
    let total = total_pass + total_fail;
    println!("\n=========================================================================");
    println!("  SIGHASH/TX DIAGNOSTIC SUMMARY: {}/{} passed ({} skipped)", total_pass, total, total_skip);
    println!("=========================================================================");
    println!();

    if total_fail > 0 {
        println!("  {} tests FAILED. See details above.", total_fail);
        println!();
        println!("  Failure categories to investigate:");
        println!("    - tx-rt: Transaction parse/serialize roundtrip failure");
        println!("    - sighash-forkid: BSV ForkID SIGHASH calculation mismatch");
        println!("    - bip143: BIP-143 preimage construction mismatch");
    } else {
        println!("  All {} tests passed.", total);
    }

    println!();
    println!("  Coverage:");
    println!("    Varint encode ............ 10 tests");
    println!("    Varint roundtrip ......... 8 tests");
    println!("    TX serialization RT ...... 20+ tests");
    println!("    TX ID calculation ........ 1 test");
    println!("    BIP-143 preimage ......... 7 tests (all SIGHASH types)");
    println!("    BSV ForkID sighash ....... {} vectors", forkid_tested);
    println!("    ─────────────────────────────");
    println!("    Total .................... {} tests", total);
    println!();
}
