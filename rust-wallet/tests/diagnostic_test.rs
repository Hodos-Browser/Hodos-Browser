//! First Run Diagnostic Test Suite
//!
//! This isn't a normal test suite — it's a diagnostic tool to discover
//! what works, what's broken, and what's missing in our implementation
//! compared to the BSV TypeScript SDK.
//!
//! Run with: cargo test diagnostic -- --nocapture
//!
//! See FIRST_RUN_DIAGNOSTIC.md for full documentation.

use std::panic;

// Import our wallet crate
use hodos_wallet::crypto::{brc42, aesgcm_custom, signing};

/// Test result for diagnostic reporting
#[derive(Debug)]
enum DiagResult {
    Pass,
    WrongOutput { expected: String, got: String },
    Panic(String),
    Error(String),
}

impl DiagResult {
    fn symbol(&self) -> &'static str {
        match self {
            DiagResult::Pass => "✓",
            DiagResult::WrongOutput { .. } => "✗",
            DiagResult::Panic(_) => "✗",
            DiagResult::Error(_) => "✗",
        }
    }
    
    fn status(&self) -> &'static str {
        match self {
            DiagResult::Pass => "PASS",
            DiagResult::WrongOutput { .. } => "WRONG OUTPUT",
            DiagResult::Panic(_) => "PANIC",
            DiagResult::Error(_) => "ERROR",
        }
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

// ═══════════════════════════════════════════════════════════════════════════
// BRC-42 Private Key Derivation Tests
// ═══════════════════════════════════════════════════════════════════════════

fn test_brc42_private_key(
    sender_pubkey: &str,
    recipient_privkey: &str,
    invoice: &str,
    expected: &str,
) -> DiagResult {
    run_test(|| {
        let sender_pub = match hex::decode(sender_pubkey) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("Failed to decode sender_pubkey: {}", e)),
        };
        
        let recip_priv = match hex::decode(recipient_privkey) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("Failed to decode recipient_privkey: {}", e)),
        };
        
        match brc42::derive_child_private_key(&recip_priv, &sender_pub, invoice) {
            Ok(derived) => {
                let derived_hex = hex::encode(&derived);
                if derived_hex == expected {
                    DiagResult::Pass
                } else {
                    DiagResult::WrongOutput {
                        expected: expected.to_string(),
                        got: derived_hex,
                    }
                }
            }
            Err(e) => DiagResult::Error(format!("{:?}", e)),
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// BRC-42 Public Key Derivation Tests  
// ═══════════════════════════════════════════════════════════════════════════

fn test_brc42_public_key(
    sender_privkey: &str,
    recipient_pubkey: &str,
    invoice: &str,
    expected: &str,
) -> DiagResult {
    run_test(|| {
        let sender_priv = match hex::decode(sender_privkey) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("Failed to decode sender_privkey: {}", e)),
        };
        
        let recip_pub = match hex::decode(recipient_pubkey) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("Failed to decode recipient_pubkey: {}", e)),
        };
        
        match brc42::derive_child_public_key(&sender_priv, &recip_pub, invoice) {
            Ok(derived) => {
                let derived_hex = hex::encode(&derived);
                if derived_hex == expected {
                    DiagResult::Pass
                } else {
                    DiagResult::WrongOutput {
                        expected: expected.to_string(),
                        got: derived_hex,
                    }
                }
            }
            Err(e) => DiagResult::Error(format!("{:?}", e)),
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// HMAC-SHA256 Tests
// ═══════════════════════════════════════════════════════════════════════════

fn test_hmac_sha256(
    key_hex: &str,
    message: &[u8],
    expected: &str,
) -> DiagResult {
    run_test(|| {
        let key = match hex::decode(key_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("Failed to decode key: {}", e)),
        };
        
        let result = signing::hmac_sha256(&key, message);
        let result_hex = hex::encode(&result);
        
        if result_hex == expected {
            DiagResult::Pass
        } else {
            DiagResult::WrongOutput {
                expected: expected.to_string(),
                got: result_hex,
            }
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// AES-256-GCM Tests
// ═══════════════════════════════════════════════════════════════════════════

fn test_aesgcm_roundtrip(plaintext: &[u8], key: &[u8; 32], iv: &[u8]) -> DiagResult {
    run_test(|| {
        // Encrypt
        let (ciphertext, auth_tag) = match aesgcm_custom::aesgcm_custom(plaintext, &[], iv, key) {
            Ok(result) => result,
            Err(e) => return DiagResult::Error(format!("Encryption failed: {}", e)),
        };
        
        // Decrypt
        match aesgcm_custom::aesgcm_decrypt_custom(&ciphertext, &[], iv, &auth_tag, key) {
            Ok(decrypted) => {
                if decrypted == plaintext {
                    DiagResult::Pass
                } else {
                    DiagResult::WrongOutput {
                        expected: hex::encode(plaintext),
                        got: hex::encode(&decrypted),
                    }
                }
            }
            Err(e) => DiagResult::Error(format!("Decryption failed: {}", e)),
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// Main Diagnostic Test
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn diagnostic_full_suite() {
    println!("\n");
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("  HODOS WALLET — FIRST RUN DIAGNOSTIC");
    println!("  Testing against BSV TypeScript SDK vectors");
    println!("═══════════════════════════════════════════════════════════════════════════");
    
    let mut total_pass = 0;
    let mut total_fail = 0;
    
    // ─── BRC-42 Private Key Derivation ───
    println!("\n▶ BRC-42 PRIVATE KEY DERIVATION");
    println!("  Source: ts-sdk src/primitives/__tests/BRC42.private.vectors.ts\n");
    
    let brc42_priv_vectors = [
        ("033f9160df035156f1c48e75eae99914fa1a1546bec19781e8eddb900200bff9d1",
         "6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede",
         "f3WCaUmnN9U=",
         "761656715bbfa172f8f9f58f5af95d9d0dfd69014cfdcacc9a245a10ff8893ef"),
        ("027775fa43959548497eb510541ac34b01d5ee9ea768de74244a4a25f7b60fae8d",
         "cab2500e206f31bc18a8af9d6f44f0b9a208c32d5cca2b22acfe9d1a213b2f36",
         "2Ska++APzEc=",
         "09f2b48bd75f4da6429ac70b5dce863d5ed2b350b6f2119af5626914bdb7c276"),
        ("0338d2e0d12ba645578b0955026ee7554889ae4c530bd7a3b6f688233d763e169f",
         "7a66d0896f2c4c2c9ac55670c71a9bc1bdbdfb4e8786ee5137cea1d0a05b6f20",
         "cN/yQ7+k7pg=",
         "7114cd9afd1eade02f76703cc976c241246a2f26f5c4b7a3a0150ecc745da9f0"),
        ("02830212a32a47e68b98d477000bde08cb916f4d44ef49d47ccd4918d9aaabe9c8",
         "6e8c3da5f2fb0306a88d6bcd427cbfba0b9c7f4c930c43122a973d620ffa3036",
         "m2/QAsmwaA4=",
         "f1d6fb05da1225feeddd1cf4100128afe09c3c1aadbffbd5c8bd10d329ef8f40"),
        ("03f20a7e71c4b276753969e8b7e8b67e2dbafc3958d66ecba98dedc60a6615336d",
         "e9d174eff5708a0a41b32624f9b9cc97ef08f8931ed188ee58d5390cad2bf68e",
         "jgpUIjWFlVQ=",
         "c5677c533f17c30f79a40744b18085632b262c0c13d87f3848c385f1389f79a6"),
    ];
    
    for (i, (sender_pub, recip_priv, invoice, expected)) in brc42_priv_vectors.iter().enumerate() {
        let result = test_brc42_private_key(sender_pub, recip_priv, invoice, expected);
        print_result(i + 1, &result);
        if matches!(result, DiagResult::Pass) { total_pass += 1; } else { total_fail += 1; }
    }
    
    // ─── BRC-42 Public Key Derivation ───
    println!("\n▶ BRC-42 PUBLIC KEY DERIVATION");
    println!("  Source: ts-sdk src/primitives/__tests/BRC42.public.vectors.ts\n");
    
    let brc42_pub_vectors = [
        ("583755110a8c059de5cd81b8a04e1be884c46083ade3f779c1e022f6f89da94c",
         "02c0c1e1a1f7d247827d1bcf399f0ef2deef7695c322fd91a01a91378f101b6ffc",
         "IBioA4D/OaE=",
         "03c1bf5baadee39721ae8c9882b3cf324f0bf3b9eb3fc1b8af8089ca7a7c2e669f"),
        ("2c378b43d887d72200639890c11d79e8f22728d032a5733ba3d7be623d1bb118",
         "039a9da906ecb8ced5c87971e9c2e7c921e66ad450fd4fc0a7d569fdb5bede8e0f",
         "PWYuo9PDKvI=",
         "0398cdf4b56a3b2e106224ff3be5253afd5b72de735d647831be51c713c9077848"),
        ("d5a5f70b373ce164998dff7ecd93260d7e80356d3d10abf928fb267f0a6c7be6",
         "02745623f4e5de046b6ab59ce837efa1a959a8f28286ce9154a4781ec033b85029",
         "X9pnS+bByrM=",
         "0273eec9380c1a11c5a905e86c2d036e70cbefd8991d9a0cfca671f5e0bbea4a3c"),
        ("46cd68165fd5d12d2d6519b02feb3f4d9c083109de1bfaa2b5c4836ba717523c",
         "031e18bb0bbd3162b886007c55214c3c952bb2ae6c33dd06f57d891a60976003b1",
         "+ktmYRHv3uQ=",
         "034c5c6bf2e52e8de8b2eb75883090ed7d1db234270907f1b0d1c2de1ddee5005d"),
        ("7c98b8abd7967485cfb7437f9c56dd1e48ceb21a4085b8cdeb2a647f62012db4",
         "03c8885f1e1ab4facd0f3272bb7a48b003d2e608e1619fb38b8be69336ab828f37",
         "PPfDTTcl1ao=",
         "03304b41cfa726096ffd9d8907fe0835f888869eda9653bca34eb7bcab870d3779"),
    ];
    
    for (i, (sender_priv, recip_pub, invoice, expected)) in brc42_pub_vectors.iter().enumerate() {
        let result = test_brc42_public_key(sender_priv, recip_pub, invoice, expected);
        print_result(i + 1, &result);
        if matches!(result, DiagResult::Pass) { total_pass += 1; } else { total_fail += 1; }
    }
    
    // ─── HMAC-SHA256 ───
    println!("\n▶ HMAC-SHA256");
    println!("  Source: ts-sdk src/primitives/__tests/HMAC.test.ts\n");
    
    // Vector 1: 64-byte key (blocklen)
    let result = test_hmac_sha256(
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
        b"Sample message for keylen=blocklen",
        "8bb9a1db9806f20df7f77b82138c7914d174d59e13dc4d0169c9057b133e1d62"
    );
    print_result(1, &result);
    if matches!(result, DiagResult::Pass) { total_pass += 1; } else { total_fail += 1; }
    
    // Vector 2: 32-byte key (< blocklen)
    let result = test_hmac_sha256(
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        b"Sample message for keylen<blocklen",
        "a28cf43130ee696a98f14a37678b56bcfcbdd9e5cf69717fecf5480f0ebdf790"
    );
    print_result(2, &result);
    if matches!(result, DiagResult::Pass) { total_pass += 1; } else { total_fail += 1; }
    
    // Vector 3: 100-byte key (> blocklen)
    let result = test_hmac_sha256(
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f6061626364",
        b"Sample message for keylen=blocklen",
        "bdccb6c72ddeadb500ae768386cb38cc41c63dbb0878ddb9c7a38a431b78378d"
    );
    print_result(3, &result);
    if matches!(result, DiagResult::Pass) { total_pass += 1; } else { total_fail += 1; }
    
    // Vector 4: Raw hex key and message (BRC-42 internal usage pattern)
    let msg_bytes = hex::decode("1d495eef7761b65dccd0a983d2d7204fea28b5c81f1758046e062eb043755ea1").unwrap();
    let result = test_hmac_sha256(
        "48f38d0c6a344959cc94502b7b5e8dffb6a5f41795d9066fc9a649557167ee2f",
        &msg_bytes,
        "cf5ad5984f9e43917aa9087380dac46e410ddc8a7731859c84e9d0f31bd43655"
    );
    print_result(4, &result);
    if matches!(result, DiagResult::Pass) { total_pass += 1; } else { total_fail += 1; }
    
    // ─── AES-256-GCM ───
    println!("\n▶ AES-256-GCM (Roundtrip)");
    println!("  Testing encrypt → decrypt roundtrip\n");
    
    // Test 1: Simple 4-byte plaintext
    let key = [0u8; 32];
    let iv = [0u8; 32];
    let result = test_aesgcm_roundtrip(b"true", &key, &iv);
    print_result(1, &result);
    if matches!(result, DiagResult::Pass) { total_pass += 1; } else { total_fail += 1; }
    
    // Test 2: 32-byte plaintext (like a symmetric key)
    let plaintext = [0x42u8; 32];
    let result = test_aesgcm_roundtrip(&plaintext, &key, &iv);
    print_result(2, &result);
    if matches!(result, DiagResult::Pass) { total_pass += 1; } else { total_fail += 1; }
    
    // ─── Summary ───
    println!("\n═══════════════════════════════════════════════════════════════════════════");
    println!("  DIAGNOSTIC SUMMARY");
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!();
    println!("  Total:   {} tests", total_pass + total_fail);
    println!("  Passing: {} ✓", total_pass);
    println!("  Failing: {} ✗", total_fail);
    println!();
    
    if total_fail > 0 {
        println!("  ⚠️  Some tests failed. See details above.");
        println!("  Review FIRST_RUN_DIAGNOSTIC.md for next steps.");
    } else {
        println!("  ✅ All diagnostic tests passed!");
        println!("  Your implementation matches the BSV TypeScript SDK.");
    }
    println!();
    
    // Note: We don't assert!(total_fail == 0) because this is diagnostic
    // The test "passes" even if there are failures — the output IS the result
}

fn print_result(num: usize, result: &DiagResult) {
    match result {
        DiagResult::Pass => {
            println!("  {} Vector {}: {}", result.symbol(), num, result.status());
        }
        DiagResult::WrongOutput { expected, got } => {
            println!("  {} Vector {}: {}", result.symbol(), num, result.status());
            println!("      Expected: {}...", &expected[..expected.len().min(40)]);
            println!("      Got:      {}...", &got[..got.len().min(40)]);
        }
        DiagResult::Panic(msg) => {
            println!("  {} Vector {}: {} — {}", result.symbol(), num, result.status(), msg);
        }
        DiagResult::Error(msg) => {
            println!("  {} Vector {}: {} — {}", result.symbol(), num, result.status(), msg);
        }
    }
}
