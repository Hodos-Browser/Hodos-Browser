//! Expanded Diagnostic Test Suite
//!
//! Tests our Rust crypto implementations against BSV TypeScript SDK vectors.
//! Two-stage validation: vectors validated by TS validator first, then tested here.
//!
//! Run with: cargo test diagnostic -- --nocapture
//!
//! See FIRST_RUN_DIAGNOSTIC.md for full documentation.

use std::panic;

// Import our wallet crate
use hodos_wallet::crypto::{brc42, aesgcm_custom, signing, keys};
use hodos_wallet::recovery;

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
            DiagResult::Pass => "PASS",
            DiagResult::WrongOutput { .. } => "FAIL",
            DiagResult::Panic(_) => "PANIC",
            DiagResult::Error(_) => "ERROR",
        }
    }

    fn is_pass(&self) -> bool {
        matches!(self, DiagResult::Pass)
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
            println!("    [{}] {}/{}", result.symbol(), section, num);
        }
        DiagResult::WrongOutput { expected, got } => {
            println!("    [{}] {}/{}", result.symbol(), section, num);
            println!("           expected: {}...", &expected[..expected.len().min(64)]);
            println!("           got:      {}...", &got[..got.len().min(64)]);
        }
        DiagResult::Panic(msg) => {
            println!("    [{}] {}/{} — {}", result.symbol(), section, num, msg);
        }
        DiagResult::Error(msg) => {
            println!("    [{}] {}/{} — {}", result.symbol(), section, num, msg);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Test helper functions
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
            Err(e) => return DiagResult::Error(format!("decode sender_pubkey: {}", e)),
        };
        let recip_priv = match hex::decode(recipient_privkey) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode recipient_privkey: {}", e)),
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

fn test_brc42_public_key(
    sender_privkey: &str,
    recipient_pubkey: &str,
    invoice: &str,
    expected: &str,
) -> DiagResult {
    run_test(|| {
        let sender_priv = match hex::decode(sender_privkey) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode sender_privkey: {}", e)),
        };
        let recip_pub = match hex::decode(recipient_pubkey) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode recipient_pubkey: {}", e)),
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

fn test_hmac(key_hex: &str, message: &[u8], expected: &str) -> DiagResult {
    run_test(|| {
        let key = match hex::decode(key_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode key: {}", e)),
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

fn test_sha256(input: &[u8], expected: &str) -> DiagResult {
    run_test(|| {
        let result = signing::sha256(input);
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

fn test_aesgcm_encrypt(
    key_hex: &str,
    iv_hex: &str,
    plaintext_hex: &str,
    _aad_hex: &str,
    expected_ct_hex: &str,
    expected_tag_hex: &str,
) -> DiagResult {
    run_test(|| {
        let key_bytes = match hex::decode(key_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode key: {}", e)),
        };
        if key_bytes.len() != 32 {
            return DiagResult::Error(format!("key must be 32 bytes, got {}", key_bytes.len()));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);

        let iv = match hex::decode(iv_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode iv: {}", e)),
        };
        let plaintext = match hex::decode(plaintext_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode plaintext: {}", e)),
        };
        let aad = match hex::decode(_aad_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode aad: {}", e)),
        };

        match aesgcm_custom::aesgcm_custom(&plaintext, &aad, &iv, &key) {
            Ok((ciphertext, auth_tag)) => {
                let ct_hex = hex::encode(&ciphertext);
                let tag_hex = hex::encode(&auth_tag);
                if ct_hex == expected_ct_hex && tag_hex == expected_tag_hex {
                    DiagResult::Pass
                } else {
                    DiagResult::WrongOutput {
                        expected: format!("ct={} tag={}", expected_ct_hex, expected_tag_hex),
                        got: format!("ct={} tag={}", ct_hex, tag_hex),
                    }
                }
            }
            Err(e) => DiagResult::Error(format!("encrypt failed: {}", e)),
        }
    })
}

fn test_aesgcm_decrypt(
    key_hex: &str,
    iv_hex: &str,
    ciphertext_hex: &str,
    _aad_hex: &str,
    tag_hex: &str,
    expected_pt_hex: &str,
) -> DiagResult {
    run_test(|| {
        let key_bytes = match hex::decode(key_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode key: {}", e)),
        };
        if key_bytes.len() != 32 {
            return DiagResult::Error(format!("key must be 32 bytes, got {}", key_bytes.len()));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);

        let iv = match hex::decode(iv_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode iv: {}", e)),
        };
        let ciphertext = match hex::decode(ciphertext_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode ciphertext: {}", e)),
        };
        let aad = match hex::decode(_aad_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode aad: {}", e)),
        };
        let tag = match hex::decode(tag_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode tag: {}", e)),
        };

        match aesgcm_custom::aesgcm_decrypt_custom(&ciphertext, &aad, &iv, &tag, &key) {
            Ok(plaintext) => {
                let pt_hex = hex::encode(&plaintext);
                if pt_hex == expected_pt_hex {
                    DiagResult::Pass
                } else {
                    DiagResult::WrongOutput {
                        expected: expected_pt_hex.to_string(),
                        got: pt_hex,
                    }
                }
            }
            Err(e) => DiagResult::Error(format!("decrypt failed: {}", e)),
        }
    })
}

fn test_aesgcm_roundtrip(plaintext: &[u8], key: &[u8; 32], iv: &[u8]) -> DiagResult {
    run_test(|| {
        let (ciphertext, auth_tag) = match aesgcm_custom::aesgcm_custom(plaintext, &[], iv, key) {
            Ok(result) => result,
            Err(e) => return DiagResult::Error(format!("encrypt: {}", e)),
        };
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
            Err(e) => DiagResult::Error(format!("decrypt: {}", e)),
        }
    })
}

/// Parse BIP-32 path string like "m/0'/1/2'" into segments
fn parse_bip32_path(path: &str) -> Vec<(u32, bool)> {
    if path == "m" {
        return vec![];
    }
    let stripped = path.strip_prefix("m/").unwrap_or(path);
    stripped
        .split('/')
        .map(|seg| {
            if let Some(idx_str) = seg.strip_suffix('\'') {
                (idx_str.parse::<u32>().unwrap(), true)
            } else {
                (seg.parse::<u32>().unwrap(), false)
            }
        })
        .collect()
}

/// Extract 32-byte private key from base58-encoded xprv string
fn extract_privkey_from_xprv(xprv: &str) -> Result<Vec<u8>, String> {
    let decoded = bs58::decode(xprv)
        .with_check(None)
        .into_vec()
        .map_err(|e| format!("bs58 decode: {}", e))?;
    // xprv payload: 4 version + 1 depth + 4 fingerprint + 4 child + 32 chaincode + 1 (0x00) + 32 privkey = 78
    if decoded.len() != 78 {
        return Err(format!("xprv payload should be 78 bytes, got {}", decoded.len()));
    }
    if decoded[45] != 0x00 {
        return Err(format!("expected 0x00 at byte 45, got 0x{:02x}", decoded[45]));
    }
    Ok(decoded[46..78].to_vec())
}

fn test_bip32_derivation(seed_hex: &str, path: &str, expected_xprv: &str) -> DiagResult {
    run_test(|| {
        let seed = match hex::decode(seed_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode seed: {}", e)),
        };
        let segments = parse_bip32_path(path);
        let expected_privkey = match extract_privkey_from_xprv(expected_xprv) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("extract xprv privkey: {}", e)),
        };

        match recovery::derive_key_at_path(&seed, &segments) {
            Ok(derived) => {
                if derived == expected_privkey {
                    DiagResult::Pass
                } else {
                    DiagResult::WrongOutput {
                        expected: hex::encode(&expected_privkey),
                        got: hex::encode(&derived),
                    }
                }
            }
            Err(e) => DiagResult::Error(format!("derive_key_at_path: {}", e)),
        }
    })
}

fn test_bip39_seed(mnemonic: &str, passphrase: &str, expected_seed_hex: &str) -> DiagResult {
    run_test(|| {
        let m = match bip39::Mnemonic::parse(mnemonic) {
            Ok(m) => m,
            Err(e) => return DiagResult::Error(format!("parse mnemonic: {}", e)),
        };
        let seed = m.to_seed(passphrase);
        let seed_hex = hex::encode(&seed);
        if seed_hex == expected_seed_hex {
            DiagResult::Pass
        } else {
            DiagResult::WrongOutput {
                expected: expected_seed_hex.to_string(),
                got: seed_hex,
            }
        }
    })
}

fn test_ecdsa_sign_verify(privkey_hex: &str, message: &[u8]) -> DiagResult {
    run_test(|| {
        let privkey = match hex::decode(privkey_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode privkey: {}", e)),
        };
        // Hash the message
        let hash = signing::sha256(message);

        // Sign (appends sighash type byte 0x41)
        let sig_with_type = match signing::sign_ecdsa(&hash, &privkey, 0x41) {
            Ok(s) => s,
            Err(e) => return DiagResult::Error(format!("sign_ecdsa: {}", e)),
        };

        // Derive public key
        let pubkey = match keys::derive_public_key(&privkey) {
            Ok(p) => p,
            Err(e) => return DiagResult::Error(format!("derive_public_key: {}", e)),
        };

        // Verify
        match signing::verify_signature(&hash, &sig_with_type, &pubkey) {
            Ok(true) => DiagResult::Pass,
            Ok(false) => DiagResult::WrongOutput {
                expected: "verification=true".to_string(),
                got: "verification=false".to_string(),
            },
            Err(e) => DiagResult::Error(format!("verify_signature: {}", e)),
        }
    })
}

/// BRC-3 signature verification test
/// The ts-sdk test verifies a pre-existing signature using ProtoWallet('anyone').
/// We replicate: derive the signer's child public key, then verify the DER signature.
fn test_brc3_verify(
    verifier_privkey_hex: &str,
    signer_counterparty_hex: &str,
    invoice: &str,
    data: &[u8],
    expected_der: &[u8],
) -> DiagResult {
    run_test(|| {
        let verifier_priv = match hex::decode(verifier_privkey_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode verifier_priv: {}", e)),
        };
        let signer_pub = match hex::decode(signer_counterparty_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode signer_pub: {}", e)),
        };

        // Step 1: Derive the signer's child public key (as the verifier sees it)
        // ts-sdk: counterpartyPub.deriveChild(verifierPrivKey, invoice)
        // Our API: derive_child_public_key(sender_private_key, recipient_public_key, invoice)
        let child_pub = match brc42::derive_child_public_key(&verifier_priv, &signer_pub, invoice) {
            Ok(k) => k,
            Err(e) => return DiagResult::Error(format!("derive_child_public_key: {:?}", e)),
        };

        // Step 2: SHA-256 hash the data
        let hash = signing::sha256(data);

        // Step 3: Verify the DER signature against the derived public key
        let secp = secp256k1::Secp256k1::new();
        let pubkey = match secp256k1::PublicKey::from_slice(&child_pub) {
            Ok(p) => p,
            Err(e) => return DiagResult::Error(format!("PublicKey: {}", e)),
        };
        let sig = match secp256k1::ecdsa::Signature::from_der(expected_der) {
            Ok(s) => s,
            Err(e) => return DiagResult::Error(format!("Signature::from_der: {}", e)),
        };
        let message = match secp256k1::Message::from_digest_slice(&hash) {
            Ok(m) => m,
            Err(e) => return DiagResult::Error(format!("Message: {}", e)),
        };

        match secp.verify_ecdsa(&message, &sig, &pubkey) {
            Ok(()) => DiagResult::Pass,
            Err(e) => DiagResult::WrongOutput {
                expected: "signature valid".to_string(),
                got: format!("verification failed: {}", e),
            },
        }
    })
}

fn test_brc2_hmac(
    privkey_hex: &str,
    counterparty_hex: &str,
    invoice: &str,
    data: &[u8],
    expected_hmac: &[u8],
) -> DiagResult {
    run_test(|| {
        let privkey = match hex::decode(privkey_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode privkey: {}", e)),
        };
        let counterparty = match hex::decode(counterparty_hex) {
            Ok(v) => v,
            Err(e) => return DiagResult::Error(format!("decode counterparty: {}", e)),
        };

        // Step 1: Derive symmetric key via BRC-42 (ECDH on derived child keys)
        // This matches ts-sdk's KeyDeriver.deriveSymmetricKey()
        let symmetric_key = match brc42::derive_symmetric_key_for_hmac(&privkey, &counterparty, invoice) {
            Ok(k) => k,
            Err(e) => return DiagResult::Error(format!("derive_symmetric_key_for_hmac: {:?}", e)),
        };

        // Step 2: HMAC-SHA256 with symmetric key
        let hmac = signing::hmac_sha256(&symmetric_key, data);

        if hmac == expected_hmac {
            DiagResult::Pass
        } else {
            DiagResult::WrongOutput {
                expected: hex::encode(expected_hmac),
                got: hex::encode(&hmac),
            }
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// Main Diagnostic Test
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn diagnostic_full_suite() {
    println!("\n");
    println!("=========================================================================");
    println!("  HODOS WALLET — DIAGNOSTIC TEST SUITE");
    println!("  Testing against BSV TypeScript SDK vectors");
    println!("=========================================================================");

    let mut total_pass = 0u32;
    let mut total_fail = 0u32;

    // Helper macro to reduce boilerplate
    macro_rules! check {
        ($section:expr, $num:expr, $result:expr) => {{
            let r = $result;
            print_result($section, $num, &r);
            if r.is_pass() { total_pass += 1; } else { total_fail += 1; }
        }};
    }

    // ─── 1. BRC-42 Private Key Derivation (5 vectors) ───────────────────
    println!("\n  [1/11] BRC-42 Private Key Derivation");
    println!("         Source: ts-sdk BRC42.private.vectors.ts");

    let brc42_priv = [
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

    for (i, (spub, rpriv, inv, exp)) in brc42_priv.iter().enumerate() {
        check!("brc42-priv", i + 1, test_brc42_private_key(spub, rpriv, inv, exp));
    }

    // ─── 2. BRC-42 Public Key Derivation (5 vectors) ────────────────────
    println!("\n  [2/11] BRC-42 Public Key Derivation");
    println!("         Source: ts-sdk BRC42.public.vectors.ts");

    let brc42_pub = [
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

    for (i, (spriv, rpub, inv, exp)) in brc42_pub.iter().enumerate() {
        check!("brc42-pub", i + 1, test_brc42_public_key(spriv, rpub, inv, exp));
    }

    // ─── 3. HMAC-SHA256 (5 vectors) ─────────────────────────────────────
    println!("\n  [3/11] HMAC-SHA256");
    println!("         Source: ts-sdk HMAC.test.ts (NIST vectors + regression)");

    // NIST 1: 64-byte key (blocklen)
    check!("hmac", 1, test_hmac(
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
        b"Sample message for keylen=blocklen",
        "8bb9a1db9806f20df7f77b82138c7914d174d59e13dc4d0169c9057b133e1d62",
    ));

    // NIST 2: 32-byte key (< blocklen)
    check!("hmac", 2, test_hmac(
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        b"Sample message for keylen<blocklen",
        "a28cf43130ee696a98f14a37678b56bcfcbdd9e5cf69717fecf5480f0ebdf790",
    ));

    // NIST 3: 100-byte key (> blocklen)
    check!("hmac", 3, test_hmac(
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f60616263",
        b"Sample message for keylen=blocklen",
        "bdccb6c72ddeadb500ae768386cb38cc41c63dbb0878ddb9c7a38a431b78378d",
    ));

    // NIST 4: 49-byte key, truncated tag message
    check!("hmac", 4, test_hmac(
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f30",
        b"Sample message for keylen<blocklen, with truncated tag",
        "27a8b157839efeac98df070b331d593618ddb985d403c0c786d23b5d132e57c7",
    ));

    // Regression: Raw hex key and message (BRC-42 internal usage)
    let msg5 = hex::decode("1d495eef7761b65dccd0a983d2d7204fea28b5c81f1758046e062eb043755ea1").unwrap();
    check!("hmac", 5, test_hmac(
        "48f38d0c6a344959cc94502b7b5e8dffb6a5f41795d9066fc9a649557167ee2f",
        &msg5,
        "cf5ad5984f9e43917aa9087380dac46e410ddc8a7731859c84e9d0f31bd43655",
    ));

    // ─── 4. SHA-256 (3 vectors) ─────────────────────────────────────────
    println!("\n  [4/11] SHA-256");
    println!("         Source: ts-sdk Hash.test.ts");

    check!("sha256", 1, test_sha256(
        b"abc",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    ));

    check!("sha256", 2, test_sha256(
        b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
    ));

    let sha_input3 = hex::decode("deadbeef").unwrap();
    check!("sha256", 3, test_sha256(
        &sha_input3,
        "5f78c33274e43fa9de5659265c1d917e25c03722dcb0b8d27db8d5feaa813953",
    ));

    // ─── 5. AES-256-GCM NIST Known-Answer (6 tests: 3 encrypt + 3 decrypt) ─
    println!("\n  [5/11] AES-256-GCM NIST Known-Answer");
    println!("         Source: ts-sdk AESGCM.test.ts (cases 13, 14, 15)");

    // Case 13: empty plaintext
    check!("aesgcm-enc", 1, test_aesgcm_encrypt(
        "0000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000",
        "", "", "",
        "530f8afbc74536b9a963b4f1c4cb738b",
    ));
    check!("aesgcm-dec", 1, test_aesgcm_decrypt(
        "0000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000",
        "", "", "530f8afbc74536b9a963b4f1c4cb738b", "",
    ));

    // Case 14: 16-byte zero plaintext
    check!("aesgcm-enc", 2, test_aesgcm_encrypt(
        "0000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000",
        "00000000000000000000000000000000",
        "",
        "cea7403d4d606b6e074ec5d3baf39d18",
        "d0d1c8a799996bf0265b98b5d48ab919",
    ));
    check!("aesgcm-dec", 2, test_aesgcm_decrypt(
        "0000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000",
        "cea7403d4d606b6e074ec5d3baf39d18",
        "",
        "d0d1c8a799996bf0265b98b5d48ab919",
        "00000000000000000000000000000000",
    ));

    // Case 15: 64-byte plaintext with non-trivial key/IV
    check!("aesgcm-enc", 3, test_aesgcm_encrypt(
        "feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308",
        "cafebabefacedbaddecaf888",
        "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255",
        "",
        "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662898015ad",
        "b094dac5d93471bdec1a502270e3cc6c",
    ));
    check!("aesgcm-dec", 3, test_aesgcm_decrypt(
        "feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308",
        "cafebabefacedbaddecaf888",
        "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662898015ad",
        "",
        "b094dac5d93471bdec1a502270e3cc6c",
        "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255",
    ));

    // ─── 6. AES-256-GCM Roundtrip (2 tests) ─────────────────────────────
    println!("\n  [6/11] AES-256-GCM Roundtrip");

    let zero_key = [0u8; 32];
    let zero_iv = [0u8; 32];
    check!("aesgcm-rt", 1, test_aesgcm_roundtrip(b"true", &zero_key, &zero_iv));

    let fill_pt = [0x42u8; 32];
    check!("aesgcm-rt", 2, test_aesgcm_roundtrip(&fill_pt, &zero_key, &zero_iv));

    // ─── 7. BIP-39 Mnemonic to Seed (24 TREZOR vectors) ─────────────────
    println!("\n  [7/11] BIP-39 Mnemonic-to-Seed");
    println!("         Source: ts-sdk Mnemonic.vectors.ts (TREZOR vectors)");

    let bip39_vectors: &[(&str, &str)] = &[
        ("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
         "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04"),
        ("legal winner thank year wave sausage worth useful legal winner thank yellow",
         "2e8905819b8723fe2c1d161860e5ee1830318dbf49a83bd451cfb8440c28bd6fa457fe1296106559a3c80937a1c1069be3a3a5bd381ee6260e8d9739fce1f607"),
        ("letter advice cage absurd amount doctor acoustic avoid letter advice cage above",
         "d71de856f81a8acc65e6fc851a38d4d7ec216fd0796d0a6827a3ad6ed5511a30fa280f12eb2e47ed2ac03b5c462a0358d18d69fe4f985ec81778c1b370b652a8"),
        ("zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
         "ac27495480225222079d7be181583751e86f571027b0497b5b5d11218e0a8a13332572917f0f8e5a589620c6f15b11c61dee327651a14c34e18231052e48c069"),
        ("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon agent",
         "035895f2f481b1b0f01fcf8c289c794660b289981a78f8106447707fdd9666ca06da5a9a565181599b79f53b844d8a71dd9f439c52a3d7b3e8a79c906ac845fa"),
        ("legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal will",
         "f2b94508732bcbacbcc020faefecfc89feafa6649a5491b8c952cede496c214a0c7b3c392d168748f2d4a612bada0753b52a1c7ac53c1e93abd5c6320b9e95dd"),
        ("letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter always",
         "107d7c02a5aa6f38c58083ff74f04c607c2d2c0ecc55501dadd72d025b751bc27fe913ffb796f841c49b1d33b610cf0e91d3aa239027f5e99fe4ce9e5088cd65"),
        ("zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo when",
         "0cd6e5d827bb62eb8fc1e262254223817fd068a74b5b449cc2f667c3f1f985a76379b43348d952e2265b4cd129090758b3e3c2c49103b5051aac2eaeb890a528"),
        ("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art",
         "bda85446c68413707090a52022edd26a1c9462295029f2e60cd7c4f2bbd3097170af7a4d73245cafa9c3cca8d561a7c3de6f5d4a10be8ed2a5e608d68f92fcc8"),
        ("legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title",
         "bc09fca1804f7e69da93c2f2028eb238c227f2e9dda30cd63699232578480a4021b146ad717fbb7e451ce9eb835f43620bf5c514db0f8add49f5d121449d3e87"),
        ("letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic bless",
         "c0c519bd0e91a2ed54357d9d1ebef6f5af218a153624cf4f2da911a0ed8f7a09e2ef61af0aca007096df430022f7a2b6fb91661a9589097069720d015e4e982f"),
        ("zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote",
         "dd48c104698c30cfe2b6142103248622fb7bb0ff692eebb00089b32d22484e1613912f0a5b694407be899ffd31ed3992c456cdf60f5d4564b8ba3f05a69890ad"),
        ("jelly better achieve collect unaware mountain thought cargo oxygen act hood bridge",
         "b5b6d0127db1a9d2226af0c3346031d77af31e918dba64287a1b44b8ebf63cdd52676f672a290aae502472cf2d602c051f3e6f18055e84e4c43897fc4e51a6ff"),
        ("renew stay biology evidence goat welcome casual join adapt armor shuffle fault little machine walk stumble urge swap",
         "9248d83e06f4cd98debf5b6f010542760df925ce46cf38a1bdb4e4de7d21f5c39366941c69e1bdbf2966e0f6e6dbece898a0e2f0a4c2b3e640953dfe8b7bbdc5"),
        ("dignity pass list indicate nasty swamp pool script soccer toe leaf photo multiply desk host tomato cradle drill spread actor shine dismiss champion exotic",
         "ff7f3184df8696d8bef94b6c03114dbee0ef89ff938712301d27ed8336ca89ef9635da20af07d4175f2bf5f3de130f39c9d9e8dd0472489c19b1a020a940da67"),
        ("afford alter spike radar gate glance object seek swamp infant panel yellow",
         "65f93a9f36b6c85cbe634ffc1f99f2b82cbb10b31edc7f087b4f6cb9e976e9faf76ff41f8f27c99afdf38f7a303ba1136ee48a4c1e7fcd3dba7aa876113a36e4"),
        ("indicate race push merry suffer human cruise dwarf pole review arch keep canvas theme poem divorce alter left",
         "3bbf9daa0dfad8229786ace5ddb4e00fa98a044ae4c4975ffd5e094dba9e0bb289349dbe2091761f30f382d4e35c4a670ee8ab50758d2c55881be69e327117ba"),
        ("clutch control vehicle tonight unusual clog visa ice plunge glimpse recipe series open hour vintage deposit universe tip job dress radar refuse motion taste",
         "fe908f96f46668b2d5b37d82f558c77ed0d69dd0e7e043a5b0511c48c2f1064694a956f86360c93dd04052a8899497ce9e985ebe0c8c52b955e6ae86d4ff4449"),
        ("turtle front uncle idea crush write shrug there lottery flower risk shell",
         "bdfb76a0759f301b0b899a1e3985227e53b3f51e67e3f2a65363caedf3e32fde42a66c404f18d7b05818c95ef3ca1e5146646856c461c073169467511680876c"),
        ("kiss carry display unusual confirm curtain upgrade antique rotate hello void custom frequent obey nut hole price segment",
         "ed56ff6c833c07982eb7119a8f48fd363c4a9b1601cd2de736b01045c5eb8ab4f57b079403485d1c4924f0790dc10a971763337cb9f9c62226f64fff26397c79"),
        ("exile ask congress lamp submit jacket era scheme attend cousin alcohol catch course end lucky hurt sentence oven short ball bird grab wing top",
         "095ee6f817b4c2cb30a5a797360a81a40ab0f9a4e25ecd672a3f58a0b5ba0687c096a6b14d2c0deb3bdefce4f61d01ae07417d502429352e27695163f7447a8c"),
        ("board flee heavy tunnel powder denial science ski answer betray cargo cat",
         "6eff1bb21562918509c73cb990260db07c0ce34ff0e3cc4a8cb3276129fbcb300bddfe005831350efd633909f476c45c88253276d9fd0df6ef48609e8bb7dca8"),
        ("board blade invite damage undo sun mimic interest slam gaze truly inherit resist great inject rocket museum chief",
         "f84521c777a13b61564234bf8f8b62b3afce27fc4062b51bb5e62bdfecb23864ee6ecf07c1d5a97c0834307c5c852d8ceb88e7c97923c0a3b496bedd4e5f88a9"),
        ("beyond stage sleep clip because twist token leaf atom beauty genius food business side grid unable middle armed observe pair crouch tonight away coconut",
         "b15509eaa2d09d3efd3e006ef42151b30367dc6e3aa5e44caba3fe4d3e352e65101fbdb86a96776b91946ff06f8eac594dc6ee1d3e82a42dfe1b40fef6bcc3fd"),
    ];

    for (i, (mnemonic, expected_seed)) in bip39_vectors.iter().enumerate() {
        check!("bip39", i + 1, test_bip39_seed(mnemonic, "TREZOR", expected_seed));
    }

    // ─── 8. BIP-32 HD Key Derivation (11 paths across 2 seed sets) ──────
    println!("\n  [8/11] BIP-32 HD Key Derivation");
    println!("         Source: ts-sdk HD.test.ts");

    // Vector set 1: seed = 000102030405060708090a0b0c0d0e0f
    let seed1 = "000102030405060708090a0b0c0d0e0f";
    let set1_paths: &[(&str, &str)] = &[
        ("m",                          "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi"),
        ("m/0'",                       "xprv9uHRZZhk6KAJC1avXpDAp4MDc3sQKNxDiPvvkX8Br5ngLNv1TxvUxt4cV1rGL5hj6KCesnDYUhd7oWgT11eZG7XnxHrnYeSvkzY7d2bhkJ7"),
        ("m/0'/1",                     "xprv9wTYmMFdV23N2TdNG573QoEsfRrWKQgWeibmLntzniatZvR9BmLnvSxqu53Kw1UmYPxLgboyZQaXwTCg8MSY3H2EU4pWcQDnRnrVA1xe8fs"),
        ("m/0'/1/2'",                  "xprv9z4pot5VBttmtdRTWfWQmoH1taj2axGVzFqSb8C9xaxKymcFzXBDptWmT7FwuEzG3ryjH4ktypQSAewRiNMjANTtpgP4mLTj34bhnZX7UiM"),
        ("m/0'/1/2'/2",                "xprvA2JDeKCSNNZky6uBCviVfJSKyQ1mDYahRjijr5idH2WwLsEd4Hsb2Tyh8RfQMuPh7f7RtyzTtdrbdqqsunu5Mm3wDvUAKRHSC34sJ7in334"),
        ("m/0'/1/2'/2/1000000000",     "xprvA41z7zogVVwxVSgdKUHDy1SKmdb533PjDz7J6N6mV6uS3ze1ai8FHa8kmHScGpWmj4WggLyQjgPie1rFSruoUihUZREPSL39UNdE3BBDu76"),
    ];

    for (i, (path, xprv)) in set1_paths.iter().enumerate() {
        check!("bip32-s1", i + 1, test_bip32_derivation(seed1, path, xprv));
    }

    // Vector set 2: longer seed
    let seed2 = "fffcf9f6f3f0edeae7e4e1dedbd8d5d2cfccc9c6c3c0bdbab7b4b1aeaba8a5a29f9c999693908d8a8784817e7b7875726f6c696663605d5a5754514e4b484542";
    let set2_paths: &[(&str, &str)] = &[
        ("m",                                  "xprv9s21ZrQH143K31xYSDQpPDxsXRTUcvj2iNHm5NUtrGiGG5e2DtALGdso3pGz6ssrdK4PFmM8NSpSBHNqPqm55Qn3LqFtT2emdEXVYsCzC2U"),
        ("m/0",                                "xprv9vHkqa6EV4sPZHYqZznhT2NPtPCjKuDKGY38FBWLvgaDx45zo9WQRUT3dKYnjwih2yJD9mkrocEZXo1ex8G81dwSM1fwqWpWkeS3v86pgKt"),
        ("m/0/2147483647'",                    "xprv9wSp6B7kry3Vj9m1zSnLvN3xH8RdsPP1Mh7fAaR7aRLcQMKTR2vidYEeEg2mUCTAwCd6vnxVrcjfy2kRgVsFawNzmjuHc2YmYRmagcEPdU9"),
        ("m/0/2147483647'/1",                  "xprv9zFnWC6h2cLgpmSA46vutJzBcfJ8yaJGg8cX1e5StJh45BBciYTRXSd25UEPVuesF9yog62tGAQtHjXajPPdbRCHuWS6T8XA2ECKADdw4Ef"),
        ("m/0/2147483647'/1/2147483646'",      "xprvA1RpRA33e1JQ7ifknakTFpgNXPmW2YvmhqLQYMmrj4xJXXWYpDPS3xz7iAxn8L39njGVyuoseXzU6rcxFLJ8HFsTjSyQbLYnMpCqE2VbFWc"),
    ];

    for (i, (path, xprv)) in set2_paths.iter().enumerate() {
        check!("bip32-s2", i + 1, test_bip32_derivation(seed2, path, xprv));
    }

    // ─── 9. ECDSA Sign/Verify Roundtrip (3 tests) ──────────────────────
    println!("\n  [9/11] ECDSA Sign/Verify Roundtrip");
    println!("         Using known private keys from BRC-42 vectors");

    check!("ecdsa", 1, test_ecdsa_sign_verify(
        "6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede",
        b"Hello BSV world",
    ));
    check!("ecdsa", 2, test_ecdsa_sign_verify(
        "cab2500e206f31bc18a8af9d6f44f0b9a208c32d5cca2b22acfe9d1a213b2f36",
        b"ECDSA deterministic signature test",
    ));
    check!("ecdsa", 3, test_ecdsa_sign_verify(
        "7a66d0896f2c4c2c9ac55670c71a9bc1bdbdfb4e8786ee5137cea1d0a05b6f20",
        b"",  // empty message
    ));

    // ─── 10. BRC-3 Signature Compliance (1 test) ────────────────────────
    println!("\n  [10/11] BRC-3 Signature Compliance");
    println!("          Source: ts-sdk ProtoWallet.test.ts");

    let brc3_expected_der: Vec<u8> = vec![
        48, 68, 2, 32, 43, 34, 58, 156, 219, 32, 50, 70, 29, 240, 155, 137,
        88, 60, 200, 95, 243, 198, 201, 21, 56, 82, 141, 112, 69, 196, 170, 73,
        156, 6, 44, 48, 2, 32, 118, 125, 254, 201, 44, 87, 177, 170, 93, 11,
        193, 134, 18, 70, 9, 31, 234, 27, 170, 177, 54, 96, 181, 140, 166, 196,
        144, 14, 230, 118, 106, 105,
    ];

    // The ts-sdk test: ProtoWallet('anyone').verifySignature({counterparty: '0294c479...'})
    // Verifier = anyone (privkey=1), signer's identity = 0294c479...
    check!("brc3-verify", 1, test_brc3_verify(
        "0000000000000000000000000000000000000000000000000000000000000001",
        "0294c479f762f6baa97fbcd4393564c1d7bd8336ebd15928135bbcf575cd1a71a1",
        "2-brc3 test-42",
        b"BRC-3 Compliance Validated!",
        &brc3_expected_der,
    ));

    // ─── 11. BRC-2 HMAC Compliance (1 test) ─────────────────────────────
    println!("\n  [11/11] BRC-2 HMAC Compliance");
    println!("          Source: ts-sdk ProtoWallet.test.ts");

    let brc2_expected_hmac: Vec<u8> = vec![
        81, 240, 18, 153, 163, 45, 174, 85, 9, 246, 142, 125, 209, 133, 82, 76,
        254, 103, 46, 182, 86, 59, 219, 61, 126, 30, 176, 232, 233, 100, 234, 14,
    ];

    check!("brc2-hmac", 1, test_brc2_hmac(
        "6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8",
        "0294c479f762f6baa97fbcd4393564c1d7bd8336ebd15928135bbcf575cd1a71a1",
        "2-brc2 test-42",
        b"BRC-2 HMAC Compliance Validated!",
        &brc2_expected_hmac,
    ));

    // ─── Summary ─────────────────────────────────────────────────────────
    let total = total_pass + total_fail;
    println!("\n=========================================================================");
    println!("  DIAGNOSTIC SUMMARY: {}/{} passed", total_pass, total);
    println!("=========================================================================");
    println!();

    if total_fail > 0 {
        println!("  WARNING: {} tests failed. See details above.", total_fail);
        println!("  Review FIRST_RUN_DIAGNOSTIC.md for next steps.");
    } else {
        println!("  All {} diagnostic tests passed.", total);
        println!("  Implementation matches BSV TypeScript SDK vectors.");
    }

    println!();
    println!("  Coverage:");
    println!("    BRC-42 key derivation .... 10 vectors");
    println!("    HMAC-SHA256 .............. 5 vectors (NIST + regression)");
    println!("    SHA-256 .................. 3 vectors");
    println!("    AES-256-GCM NIST ......... 6 tests (3 encrypt + 3 decrypt)");
    println!("    AES-256-GCM roundtrip .... 2 tests");
    println!("    BIP-39 mnemonic->seed .... 24 vectors (TREZOR)");
    println!("    BIP-32 HD derivation ..... 11 paths");
    println!("    ECDSA sign/verify ........ 3 roundtrips");
    println!("    BRC-3 signature .......... 1 compliance vector");
    println!("    BRC-2 HMAC ............... 1 compliance vector");
    println!("    ─────────────────────────────");
    println!("    Total .................... {} tests", total);
    println!();
}
