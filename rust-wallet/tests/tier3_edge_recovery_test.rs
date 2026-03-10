//! Tier 3: Edge Cases, Recovery & GHASH Diagnostic Tests
//!
//! Validates boundary conditions and edge cases across:
//! - Key derivation (generator point, curve order boundaries)
//! - ECDSA signing (determinism, cross-verification, wrong-key rejection)
//! - Hash functions (SHA-256, double SHA-256, HMAC-SHA256 with RFC 4231 vectors)
//! - PushDrop encode/decode roundtrip with PUSHDATA boundary sizes
//! - Script parser truncation and corruption handling
//! - BIP32 derivation against official BIP-0032 test vectors
//! - GHASH hash subkey against NIST AES-256 known value
//!
//! Methodology: Collect-first with structured PASS/FAIL reporting.
//! Zero changes to wallet code — tests only.

use std::sync::atomic::{AtomicU32, Ordering};

static PASS_COUNT: AtomicU32 = AtomicU32::new(0);
static FAIL_COUNT: AtomicU32 = AtomicU32::new(0);

/// check! macro — wraps block in closure so `?` returns Err to the macro, not the outer fn
macro_rules! check {
    ($section:expr, $id:expr, $result:expr) => {
        match (|| -> Result<(), String> { $result })() {
            Ok(()) => {
                PASS_COUNT.fetch_add(1, Ordering::Relaxed);
                println!("    [PASS] {}/{}", $section, $id);
            }
            Err(e) => {
                FAIL_COUNT.fetch_add(1, Ordering::Relaxed);
                println!("    [FAIL] {}/{}: {}", $section, $id, e);
            }
        }
    };
}

#[test]
fn tier3_diagnostic_suite() {
    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║   Tier 3: Edge Cases, Recovery & GHASH Diagnostics  ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    section_1_key_derivation_edges();
    section_2_ecdsa_signing_edges();
    section_3_hash_known_vectors();
    section_4_pushdrop_roundtrip();
    section_5_script_parser_edges();
    section_6_bip32_recovery();
    section_7_ghash();

    let p = PASS_COUNT.load(Ordering::Relaxed);
    let f = FAIL_COUNT.load(Ordering::Relaxed);
    println!("\n══════════════════════════════════════════════════════");
    println!("  TOTAL: {} passed, {} failed, {} total", p, f, p + f);
    println!("══════════════════════════════════════════════════════\n");

    assert_eq!(f, 0, "{} test(s) failed — see [FAIL] lines above", f);
}

// ============================================================================
// [1/7] Key Derivation Edge Cases
// ============================================================================

fn section_1_key_derivation_edges() {
    use hodos_wallet::crypto::keys::{derive_public_key, derive_public_key_uncompressed};

    println!("  [1/7] Key Derivation Edge Cases");

    // 1. Private key = 1 → secp256k1 generator point G (compressed)
    check!("keys", "1 generator-point", {
        let privkey_1 = {
            let mut k = [0u8; 32];
            k[31] = 1;
            k
        };
        let pubkey = derive_public_key(&privkey_1).map_err(|e| e.to_string())?;
        let expected = hex::decode(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        ).unwrap();
        if pubkey != expected {
            return Err(format!(
                "generator point mismatch:\n  got:      {}\n  expected: {}",
                hex::encode(&pubkey), hex::encode(&expected)
            ));
        }
        Ok(())
    });

    // 2. Private key = N-1 (largest valid key on secp256k1)
    check!("keys", "2 curve-order-minus-1", {
        let n_minus_1 = hex::decode(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364140"
        ).unwrap();
        let pubkey = derive_public_key(&n_minus_1).map_err(|e| e.to_string())?;
        if pubkey.len() != 33 {
            return Err(format!("expected 33 bytes, got {}", pubkey.len()));
        }
        // N-1 is valid; pubkey should be -G (the negation of the generator)
        // Negation of G has same x-coordinate but different y parity
        // G compressed starts with 02, -G starts with 03
        if pubkey[0] != 0x03 {
            return Err(format!("expected 0x03 prefix for -G, got 0x{:02x}", pubkey[0]));
        }
        // x-coordinate should match G's x-coordinate
        let g_x = hex::decode(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        ).unwrap();
        if pubkey[1..33] != g_x[..] {
            return Err("x-coordinate of -G doesn't match G".to_string());
        }
        Ok(())
    });

    // 3. Private key = 0 (all zeros) — invalid on secp256k1
    check!("keys", "3 zero-key-rejected", {
        let zero_key = [0u8; 32];
        match derive_public_key(&zero_key) {
            Err(_) => Ok(()),
            Ok(pk) => Err(format!("zero key should be rejected, got pubkey {}", hex::encode(&pk))),
        }
    });

    // 4. Private key = N (curve order) — invalid, equivalent to 0
    check!("keys", "4 curve-order-rejected", {
        let n = hex::decode(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141"
        ).unwrap();
        match derive_public_key(&n) {
            Err(_) => Ok(()),
            Ok(pk) => Err(format!("curve order N should be rejected, got {}", hex::encode(&pk))),
        }
    });

    // 5. Invalid lengths (0, 31, 33, 64 bytes)
    check!("keys", "5 invalid-lengths", {
        for len in &[0, 31, 33, 64] {
            let key = vec![0x42u8; *len];
            if derive_public_key(&key).is_ok() {
                return Err(format!("{}-byte key should be rejected", len));
            }
        }
        Ok(())
    });

    // 6. Compressed vs uncompressed share same x-coordinate
    check!("keys", "6 compressed-uncompressed-x-match", {
        let privkey = {
            let mut k = [0u8; 32];
            k[31] = 42;
            k
        };
        let compressed = derive_public_key(&privkey).map_err(|e| e.to_string())?;
        let uncompressed = derive_public_key_uncompressed(&privkey).map_err(|e| e.to_string())?;
        if compressed.len() != 33 {
            return Err(format!("compressed len {}", compressed.len()));
        }
        if uncompressed.len() != 65 {
            return Err(format!("uncompressed len {}", uncompressed.len()));
        }
        if uncompressed[0] != 0x04 {
            return Err(format!("uncompressed prefix 0x{:02x}", uncompressed[0]));
        }
        // x-coordinate: compressed[1..33] == uncompressed[1..33]
        if compressed[1..33] != uncompressed[1..33] {
            return Err("x-coordinate mismatch between compressed and uncompressed".to_string());
        }
        Ok(())
    });

    // 7. Determinism: same key always produces same pubkey
    check!("keys", "7 determinism", {
        let privkey = [0xABu8; 32];
        let pk1 = derive_public_key(&privkey).map_err(|e| e.to_string())?;
        let pk2 = derive_public_key(&privkey).map_err(|e| e.to_string())?;
        if pk1 != pk2 {
            return Err("non-deterministic key derivation".to_string());
        }
        Ok(())
    });
}

// ============================================================================
// [2/7] ECDSA Signing Edge Cases
// ============================================================================

fn section_2_ecdsa_signing_edges() {
    use hodos_wallet::crypto::signing::{sign_ecdsa, verify_signature};
    use hodos_wallet::crypto::keys::derive_public_key;

    println!("  [2/7] ECDSA Signing Edge Cases");

    // 1. RFC 6979 determinism: same key + hash → identical signature bytes
    check!("ecdsa", "1 rfc6979-determinism", {
        let privkey = {
            let mut k = [0u8; 32];
            k[31] = 1;
            k
        };
        let hash = [0x42u8; 32];
        let sig1 = sign_ecdsa(&hash, &privkey, 0x41).map_err(|e| e.to_string())?;
        let sig2 = sign_ecdsa(&hash, &privkey, 0x41).map_err(|e| e.to_string())?;
        if sig1 != sig2 {
            return Err("RFC 6979: same inputs produced different signatures".to_string());
        }
        Ok(())
    });

    // 2. Different hashes → different signatures
    check!("ecdsa", "2 different-hashes-differ", {
        let privkey = [0x11u8; 32];
        let hash_a = [0x01u8; 32];
        let hash_b = [0x02u8; 32];
        let sig_a = sign_ecdsa(&hash_a, &privkey, 0x01).map_err(|e| e.to_string())?;
        let sig_b = sign_ecdsa(&hash_b, &privkey, 0x01).map_err(|e| e.to_string())?;
        if sig_a == sig_b {
            return Err("different hashes produced identical signatures".to_string());
        }
        Ok(())
    });

    // 3. Sign+verify roundtrip with generator key (privkey=1)
    check!("ecdsa", "3 sign-verify-generator", {
        let privkey = {
            let mut k = [0u8; 32];
            k[31] = 1;
            k
        };
        let hash = [0xFFu8; 32];
        let sig = sign_ecdsa(&hash, &privkey, 0x41).map_err(|e| e.to_string())?;
        let pubkey = derive_public_key(&privkey).map_err(|e| e.to_string())?;
        let valid = verify_signature(&hash, &sig, &pubkey).map_err(|e| e.to_string())?;
        if !valid {
            return Err("valid signature failed verification".to_string());
        }
        Ok(())
    });

    // 4. Cross-key verification: sign with key A, verify with key B → false
    check!("ecdsa", "4 wrong-key-rejected", {
        let privkey_a = [0x11u8; 32];
        let privkey_b = [0x22u8; 32];
        let hash = [0x33u8; 32];
        let sig = sign_ecdsa(&hash, &privkey_a, 0x01).map_err(|e| e.to_string())?;
        let pubkey_b = derive_public_key(&privkey_b).map_err(|e| e.to_string())?;
        let valid = verify_signature(&hash, &sig, &pubkey_b).map_err(|e| e.to_string())?;
        if valid {
            return Err("signature verified with wrong key".to_string());
        }
        Ok(())
    });

    // 5. Wrong hash verification → false
    check!("ecdsa", "5 wrong-hash-rejected", {
        let privkey = [0x44u8; 32];
        let hash_sign = [0x55u8; 32];
        let hash_verify = [0x66u8; 32];
        let sig = sign_ecdsa(&hash_sign, &privkey, 0x01).map_err(|e| e.to_string())?;
        let pubkey = derive_public_key(&privkey).map_err(|e| e.to_string())?;
        let valid = verify_signature(&hash_verify, &sig, &pubkey).map_err(|e| e.to_string())?;
        if valid {
            return Err("signature verified with wrong hash".to_string());
        }
        Ok(())
    });

    // 6. DER structure: starts with 0x30, sighash byte appended
    check!("ecdsa", "6 der-structure", {
        let privkey = [0x77u8; 32];
        let hash = [0x88u8; 32];
        let sig = sign_ecdsa(&hash, &privkey, 0x41).map_err(|e| e.to_string())?;
        // DER: 0x30 <total_len> 0x02 <r_len> <r> 0x02 <s_len> <s> <sighash_type>
        if sig[0] != 0x30 {
            return Err(format!("DER should start with 0x30, got 0x{:02x}", sig[0]));
        }
        let last = sig[sig.len() - 1];
        if last != 0x41 {
            return Err(format!("sighash byte should be 0x41, got 0x{:02x}", last));
        }
        // DER total length = sig.len() - 3 (0x30, length byte, sighash type)
        let der_len = sig[1] as usize;
        if der_len != sig.len() - 3 {
            return Err(format!(
                "DER length {} doesn't match sig.len()-3={}",
                der_len, sig.len() - 3
            ));
        }
        Ok(())
    });

    // 7. Low-S normalization: S value must be <= N/2
    check!("ecdsa", "7 low-s-enforcement", {
        // secp256k1 curve order N
        let n_half = hex::decode(
            "7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0"
        ).unwrap();
        // Sign several messages and verify all produce low-S
        for i in 1u8..=10 {
            let privkey = [i; 32];
            let hash = [(i.wrapping_mul(37)); 32];
            let sig = sign_ecdsa(&hash, &privkey, 0x01).map_err(|e| e.to_string())?;
            // Parse DER to extract S
            // 0x30 <len> 0x02 <r_len> <r_bytes> 0x02 <s_len> <s_bytes> <sighash>
            let r_len = sig[3] as usize;
            let s_offset = 4 + r_len;
            if sig[s_offset] != 0x02 {
                return Err(format!("iter {}: expected 0x02 at S marker, got 0x{:02x}", i, sig[s_offset]));
            }
            let s_len = sig[s_offset + 1] as usize;
            let s_bytes = &sig[s_offset + 2..s_offset + 2 + s_len];
            // Pad S to 32 bytes for comparison
            let mut s_padded = vec![0u8; 32];
            let offset = 32usize.saturating_sub(s_bytes.len());
            // Skip leading zero if S has 33 bytes (DER sign padding)
            let s_data = if s_bytes.len() > 32 { &s_bytes[s_bytes.len()-32..] } else { s_bytes };
            let copy_offset = 32 - s_data.len();
            s_padded[copy_offset..].copy_from_slice(s_data);
            // S must be <= N/2
            if s_padded.as_slice() > n_half.as_slice() {
                return Err(format!(
                    "iter {}: S > N/2 (high-S signature)\n  S = {}",
                    i, hex::encode(&s_padded)
                ));
            }
        }
        Ok(())
    });

    // 8. Empty signature rejected
    check!("ecdsa", "8 empty-sig-rejected", {
        let hash = [0x99u8; 32];
        let pubkey = derive_public_key(&[0x11u8; 32]).map_err(|e| e.to_string())?;
        match verify_signature(&hash, &[], &pubkey) {
            Err(_) => Ok(()),
            Ok(_) => Err("empty signature should be rejected".to_string()),
        }
    });

    // 9. Invalid hash length rejected on sign
    check!("ecdsa", "9 invalid-hash-len-sign", {
        let privkey = [0x11u8; 32];
        for len in &[0, 16, 31, 33, 64] {
            let bad_hash = vec![0xAA; *len];
            if sign_ecdsa(&bad_hash, &privkey, 0x01).is_ok() {
                return Err(format!("{}-byte hash should be rejected", len));
            }
        }
        Ok(())
    });

    // 10. Invalid key length rejected on sign
    check!("ecdsa", "10 invalid-key-len-sign", {
        let hash = [0xBBu8; 32];
        for len in &[0, 16, 31, 33, 64] {
            let bad_key = vec![0xCC; *len];
            if sign_ecdsa(&hash, &bad_key, 0x01).is_ok() {
                return Err(format!("{}-byte key should be rejected", len));
            }
        }
        Ok(())
    });
}

// ============================================================================
// [3/7] Hash Function Known Vectors
// ============================================================================

fn section_3_hash_known_vectors() {
    use hodos_wallet::crypto::signing::{sha256, double_sha256, hmac_sha256, verify_hmac_sha256};

    println!("  [3/7] Hash Function Known Vectors");

    // 1. SHA-256("") — NIST known value
    check!("hash", "1 sha256-empty", {
        let hash = sha256(b"");
        let expected = hex::decode(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ).unwrap();
        if hash != expected {
            return Err(format!("SHA-256('') mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&hash), hex::encode(&expected)));
        }
        Ok(())
    });

    // 2. SHA-256("abc") — NIST known value
    check!("hash", "2 sha256-abc", {
        let hash = sha256(b"abc");
        let expected = hex::decode(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        ).unwrap();
        if hash != expected {
            return Err(format!("SHA-256('abc') mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&hash), hex::encode(&expected)));
        }
        Ok(())
    });

    // 3. SHA-256("hello world")
    check!("hash", "3 sha256-hello-world", {
        let hash = sha256(b"hello world");
        let expected = hex::decode(
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        ).unwrap();
        if hash != expected {
            return Err(format!("mismatch: got {}", hex::encode(&hash)));
        }
        Ok(())
    });

    // 4. Double SHA-256("") — known value
    check!("hash", "4 double-sha256-empty", {
        let hash = double_sha256(b"");
        let expected = hex::decode(
            "5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456"
        ).unwrap();
        if hash != expected {
            return Err(format!("double-SHA-256('') mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&hash), hex::encode(&expected)));
        }
        Ok(())
    });

    // 5. Double SHA-256 ≠ single SHA-256
    check!("hash", "5 double-differs-from-single", {
        let data = b"test data";
        let single = sha256(data);
        let double = double_sha256(data);
        if single == double {
            return Err("double SHA-256 should differ from single".to_string());
        }
        // Double = SHA-256(SHA-256(data))
        let manual_double = sha256(&single);
        if double != manual_double {
            return Err("double_sha256 doesn't match manual SHA-256(SHA-256())".to_string());
        }
        Ok(())
    });

    // 6. HMAC-SHA256 RFC 4231 Test Case 1
    check!("hash", "6 hmac-rfc4231-tc1", {
        let key = hex::decode("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b").unwrap();
        let data = b"Hi There";
        let hmac = hmac_sha256(&key, data);
        let expected = hex::decode(
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        ).unwrap();
        if hmac != expected {
            return Err(format!("HMAC TC1 mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&hmac), hex::encode(&expected)));
        }
        Ok(())
    });

    // 7. HMAC-SHA256 RFC 4231 Test Case 2
    check!("hash", "7 hmac-rfc4231-tc2", {
        let key = b"Jefe";
        let data = b"what do ya want for nothing?";
        let hmac = hmac_sha256(key, data);
        let expected = hex::decode(
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        ).unwrap();
        if hmac != expected {
            return Err(format!("HMAC TC2 mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&hmac), hex::encode(&expected)));
        }
        Ok(())
    });

    // 8. HMAC verify roundtrip
    check!("hash", "8 hmac-verify-roundtrip", {
        let key = b"my secret key";
        let data = b"authenticated message";
        let hmac = hmac_sha256(key, data);
        if !verify_hmac_sha256(key, data, &hmac) {
            return Err("HMAC verify failed for correct input".to_string());
        }
        Ok(())
    });

    // 9. HMAC verify rejects wrong key
    check!("hash", "9 hmac-wrong-key-rejected", {
        let key = b"correct key";
        let wrong_key = b"wrong key!!";
        let data = b"message";
        let hmac = hmac_sha256(key, data);
        if verify_hmac_sha256(wrong_key, data, &hmac) {
            return Err("HMAC should reject wrong key".to_string());
        }
        Ok(())
    });

    // 10. HMAC verify rejects tampered data
    check!("hash", "10 hmac-tampered-data-rejected", {
        let key = b"key";
        let data = b"original";
        let hmac = hmac_sha256(key, data);
        if verify_hmac_sha256(key, b"tampered", &hmac) {
            return Err("HMAC should reject tampered data".to_string());
        }
        Ok(())
    });

    // 11. HMAC verify rejects truncated mac
    check!("hash", "11 hmac-truncated-rejected", {
        let key = b"key";
        let data = b"data";
        let hmac = hmac_sha256(key, data);
        if verify_hmac_sha256(key, data, &hmac[..31]) {
            return Err("HMAC should reject truncated mac".to_string());
        }
        Ok(())
    });

    // 12. SHA-256 output is always 32 bytes
    check!("hash", "12 sha256-output-length", {
        for size in &[0, 1, 15, 16, 32, 100, 1000] {
            let data = vec![0x42u8; *size];
            let hash = sha256(&data);
            if hash.len() != 32 {
                return Err(format!("SHA-256 of {}-byte input produced {} bytes", size, hash.len()));
            }
        }
        Ok(())
    });
}

// ============================================================================
// [4/7] PushDrop Encode/Decode Roundtrip
// ============================================================================

fn section_4_pushdrop_roundtrip() {
    use hodos_wallet::script::pushdrop::{encode, decode, create_minimally_encoded_chunk, LockPosition};
    use hodos_wallet::script::parser::opcodes;

    println!("  [4/7] PushDrop Encode/Decode Roundtrip");

    let test_pubkey = vec![0x02u8; 33]; // valid compressed pubkey format

    // 1. Single field, lock Before
    check!("pushdrop", "1 single-field-before", {
        let fields = vec![b"hello pushdrop".to_vec()];
        let script = encode(&fields, &test_pubkey, LockPosition::Before)
            .map_err(|e| e.to_string())?;
        let decoded = decode(&script).map_err(|e| e.to_string())?;
        if decoded.locking_public_key != test_pubkey {
            return Err("pubkey mismatch".to_string());
        }
        if decoded.fields != fields {
            return Err("field mismatch".to_string());
        }
        Ok(())
    });

    // 2. Single field, lock After
    check!("pushdrop", "2 single-field-after", {
        let fields = vec![b"after position".to_vec()];
        let script = encode(&fields, &test_pubkey, LockPosition::After)
            .map_err(|e| e.to_string())?;
        let decoded = decode(&script).map_err(|e| e.to_string())?;
        if decoded.locking_public_key != test_pubkey {
            return Err("pubkey mismatch".to_string());
        }
        if decoded.fields != fields {
            return Err("field mismatch".to_string());
        }
        Ok(())
    });

    // 3. Multiple fields (3 fields — tests OP_2DROP + OP_DROP)
    check!("pushdrop", "3 three-fields", {
        let fields = vec![
            b"field-one".to_vec(),
            b"field-two".to_vec(),
            b"field-three".to_vec(),
        ];
        let script = encode(&fields, &test_pubkey, LockPosition::Before)
            .map_err(|e| e.to_string())?;
        let decoded = decode(&script).map_err(|e| e.to_string())?;
        if decoded.fields.len() != 3 {
            return Err(format!("expected 3 fields, got {}", decoded.fields.len()));
        }
        for (i, (got, expected)) in decoded.fields.iter().zip(fields.iter()).enumerate() {
            if got != expected {
                return Err(format!("field {} mismatch", i));
            }
        }
        Ok(())
    });

    // 4. Even field count (4 fields — tests two OP_2DROPs)
    check!("pushdrop", "4 four-fields-even", {
        let fields: Vec<Vec<u8>> = (0..4).map(|i| format!("field-{}", i).into_bytes()).collect();
        let script = encode(&fields, &test_pubkey, LockPosition::After)
            .map_err(|e| e.to_string())?;
        let decoded = decode(&script).map_err(|e| e.to_string())?;
        if decoded.fields.len() != 4 {
            return Err(format!("expected 4 fields, got {}", decoded.fields.len()));
        }
        Ok(())
    });

    // 5. Boundary: 75-byte field (direct push) vs 76-byte (PUSHDATA1)
    check!("pushdrop", "5 boundary-75-vs-76", {
        let data_75 = vec![0x42u8; 75];
        let data_76 = vec![0x42u8; 76];
        let enc_75 = create_minimally_encoded_chunk(&data_75);
        let enc_76 = create_minimally_encoded_chunk(&data_76);
        // 75 bytes → direct push: opcode=75, no PUSHDATA prefix
        if enc_75[0] != 75 {
            return Err(format!("75-byte: expected opcode 75, got {}", enc_75[0]));
        }
        if enc_75.len() != 76 { // 1 byte opcode + 75 bytes data
            return Err(format!("75-byte: expected 76 total, got {}", enc_75.len()));
        }
        // 76 bytes → PUSHDATA1
        if enc_76[0] != opcodes::OP_PUSHDATA1 {
            return Err(format!("76-byte: expected PUSHDATA1 (0x4c), got 0x{:02x}", enc_76[0]));
        }
        if enc_76[1] != 76 {
            return Err(format!("76-byte: expected length 76, got {}", enc_76[1]));
        }
        Ok(())
    });

    // 6. Boundary: 255-byte (PUSHDATA1) vs 256-byte (PUSHDATA2)
    check!("pushdrop", "6 boundary-255-vs-256", {
        let data_255 = vec![0x42u8; 255];
        let data_256 = vec![0x42u8; 256];
        let enc_255 = create_minimally_encoded_chunk(&data_255);
        let enc_256 = create_minimally_encoded_chunk(&data_256);
        if enc_255[0] != opcodes::OP_PUSHDATA1 {
            return Err(format!("255-byte: expected PUSHDATA1, got 0x{:02x}", enc_255[0]));
        }
        if enc_256[0] != opcodes::OP_PUSHDATA2 {
            return Err(format!("256-byte: expected PUSHDATA2 (0x4d), got 0x{:02x}", enc_256[0]));
        }
        // Verify PUSHDATA2 length encoding (2 bytes LE)
        let len = u16::from_le_bytes([enc_256[1], enc_256[2]]);
        if len != 256 {
            return Err(format!("256-byte: PUSHDATA2 length {}", len));
        }
        Ok(())
    });

    // 7. Roundtrip with 75-byte field (boundary for direct push)
    check!("pushdrop", "7 roundtrip-75-byte-field", {
        let field = vec![0xAB; 75];
        let script = encode(&[field.clone()], &test_pubkey, LockPosition::Before)
            .map_err(|e| e.to_string())?;
        let decoded = decode(&script).map_err(|e| e.to_string())?;
        if decoded.fields[0] != field {
            return Err("75-byte field roundtrip failed".to_string());
        }
        Ok(())
    });

    // 8. Roundtrip with 200-byte field (PUSHDATA1 range)
    check!("pushdrop", "8 roundtrip-200-byte-field", {
        let field = vec![0xCD; 200];
        let script = encode(&[field.clone()], &test_pubkey, LockPosition::After)
            .map_err(|e| e.to_string())?;
        let decoded = decode(&script).map_err(|e| e.to_string())?;
        if decoded.fields[0] != field {
            return Err("200-byte field roundtrip failed".to_string());
        }
        Ok(())
    });

    // 9. Invalid pubkey length rejected on encode
    check!("pushdrop", "9 invalid-pubkey-rejected", {
        for len in &[0, 32, 34, 65] {
            let bad_pk = vec![0x02; *len];
            if encode(&[b"data".to_vec()], &bad_pk, LockPosition::Before).is_ok() {
                return Err(format!("{}-byte pubkey should be rejected", len));
            }
        }
        Ok(())
    });

    // 10. Empty script decode fails
    check!("pushdrop", "10 empty-script-decode-fails", {
        match decode(&[]) {
            Err(_) => Ok(()),
            Ok(_) => Err("empty script decode should fail".to_string()),
        }
    });

    // 11. Minimal encoding: special values OP_0, OP_1..16, OP_1NEGATE
    check!("pushdrop", "11 special-value-encoding", {
        // OP_0 for empty and [0]
        if create_minimally_encoded_chunk(&[]) != vec![opcodes::OP_0] {
            return Err("empty → OP_0 failed".to_string());
        }
        if create_minimally_encoded_chunk(&[0]) != vec![opcodes::OP_0] {
            return Err("[0] → OP_0 failed".to_string());
        }
        // OP_1 through OP_16
        for i in 1u8..=16 {
            let encoded = create_minimally_encoded_chunk(&[i]);
            let expected_op = opcodes::OP_1 + (i - 1);
            if encoded != vec![expected_op] {
                return Err(format!("[{}] → OP_{} failed: got {:?}", i, i, encoded));
            }
        }
        // OP_1NEGATE for [0x81]
        if create_minimally_encoded_chunk(&[0x81]) != vec![opcodes::OP_1NEGATE] {
            return Err("[0x81] → OP_1NEGATE failed".to_string());
        }
        Ok(())
    });
}

// ============================================================================
// [5/7] Script Parser Edge Cases
// ============================================================================

fn section_5_script_parser_edges() {
    use hodos_wallet::script::parser::{parse_script_chunks, opcodes};

    println!("  [5/7] Script Parser Edge Cases");

    // 1. Empty script → empty chunks
    check!("parser", "1 empty-script", {
        let chunks = parse_script_chunks(&[]).map_err(|e| e.to_string())?;
        if !chunks.is_empty() {
            return Err(format!("expected 0 chunks, got {}", chunks.len()));
        }
        Ok(())
    });

    // 2. Truncated direct push (opcode says 5 bytes but only 3 follow)
    check!("parser", "2 truncated-direct-push", {
        let script = vec![0x05, 0x01, 0x02, 0x03]; // claims 5 bytes, only 3 present
        match parse_script_chunks(&script) {
            Err(_) => Ok(()),
            Ok(_) => Err("should fail on truncated direct push".to_string()),
        }
    });

    // 3. Truncated PUSHDATA1 — missing length byte
    check!("parser", "3 truncated-pushdata1-no-len", {
        let script = vec![opcodes::OP_PUSHDATA1]; // no length byte
        match parse_script_chunks(&script) {
            Err(_) => Ok(()),
            Ok(_) => Err("should fail on truncated PUSHDATA1".to_string()),
        }
    });

    // 4. Truncated PUSHDATA1 — length says 50 but only 10 bytes follow
    check!("parser", "4 truncated-pushdata1-short-data", {
        let mut script = vec![opcodes::OP_PUSHDATA1, 50];
        script.extend(vec![0x42; 10]);
        match parse_script_chunks(&script) {
            Err(_) => Ok(()),
            Ok(_) => Err("should fail on short PUSHDATA1 data".to_string()),
        }
    });

    // 5. Truncated PUSHDATA2 — missing length bytes
    check!("parser", "5 truncated-pushdata2-no-len", {
        let script = vec![opcodes::OP_PUSHDATA2, 0x00]; // only 1 of 2 length bytes
        match parse_script_chunks(&script) {
            Err(_) => Ok(()),
            Ok(_) => Err("should fail on truncated PUSHDATA2 length".to_string()),
        }
    });

    // 6. Truncated PUSHDATA4 — missing length bytes
    check!("parser", "6 truncated-pushdata4-no-len", {
        let script = vec![opcodes::OP_PUSHDATA4, 0x01, 0x00]; // only 2 of 4 length bytes
        match parse_script_chunks(&script) {
            Err(_) => Ok(()),
            Ok(_) => Err("should fail on truncated PUSHDATA4 length".to_string()),
        }
    });

    // 7. Multi-opcode script: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
    check!("parser", "7 p2pkh-script-parse", {
        let mut script = Vec::new();
        script.push(0x76); // OP_DUP
        script.push(0xa9); // OP_HASH160
        script.push(0x14); // push 20 bytes
        script.extend(vec![0xAA; 20]); // 20-byte hash
        script.push(0x88); // OP_EQUALVERIFY
        script.push(0xac); // OP_CHECKSIG
        let chunks = parse_script_chunks(&script).map_err(|e| e.to_string())?;
        if chunks.len() != 5 {
            return Err(format!("expected 5 chunks for P2PKH, got {}", chunks.len()));
        }
        if chunks[0].op != 0x76 { return Err("chunk 0 not OP_DUP".to_string()); }
        if chunks[1].op != 0xa9 { return Err("chunk 1 not OP_HASH160".to_string()); }
        if chunks[2].data.as_ref().map(|d| d.len()) != Some(20) {
            return Err("chunk 2 not 20-byte push".to_string());
        }
        if chunks[3].op != 0x88 { return Err("chunk 3 not OP_EQUALVERIFY".to_string()); }
        if chunks[4].op != opcodes::OP_CHECKSIG { return Err("chunk 4 not OP_CHECKSIG".to_string()); }
        Ok(())
    });

    // 8. Maximum direct push (75 bytes) parses correctly
    check!("parser", "8 max-direct-push-75", {
        let mut script = vec![75u8]; // opcode = 75 means push 75 bytes
        script.extend(vec![0xFF; 75]);
        let chunks = parse_script_chunks(&script).map_err(|e| e.to_string())?;
        if chunks.len() != 1 {
            return Err(format!("expected 1 chunk, got {}", chunks.len()));
        }
        if chunks[0].data.as_ref().map(|d| d.len()) != Some(75) {
            return Err("data length not 75".to_string());
        }
        Ok(())
    });

    // 9. Single-byte opcode with no data (OP_0, OP_1, etc.)
    check!("parser", "9 single-byte-opcodes", {
        for op in &[opcodes::OP_0, opcodes::OP_1, opcodes::OP_16, opcodes::OP_1NEGATE,
                    opcodes::OP_DROP, opcodes::OP_CHECKSIG] {
            let chunks = parse_script_chunks(&[*op]).map_err(|e| e.to_string())?;
            if chunks.len() != 1 {
                return Err(format!("opcode 0x{:02x}: expected 1 chunk, got {}", op, chunks.len()));
            }
            if chunks[0].op != *op {
                return Err(format!("opcode mismatch: expected 0x{:02x}, got 0x{:02x}", op, chunks[0].op));
            }
        }
        Ok(())
    });
}

// ============================================================================
// [6/7] BIP32 Key Derivation (recovery.rs)
// ============================================================================

fn section_6_bip32_recovery() {
    use hodos_wallet::recovery::{derive_key_at_path, derive_address_at_path, address_to_p2pkh_script};

    println!("  [6/7] BIP32 Key Derivation (recovery.rs)");

    // BIP-0032 Test Vector 1
    // Seed: 000102030405060708090a0b0c0d0e0f
    let seed = hex::decode("000102030405060708090a0b0c0d0e0f").unwrap();

    // 1. Master key (m) — privkey from BIP-32 test vector 1
    check!("bip32", "1 master-key-tv1", {
        let privkey = derive_key_at_path(&seed, &[]).map_err(|e| e.to_string())?;
        let expected = hex::decode(
            "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35"
        ).unwrap();
        if privkey != expected {
            return Err(format!(
                "master key mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&privkey), hex::encode(&expected)
            ));
        }
        Ok(())
    });

    // 2. m/0' (hardened) — privkey from BIP-32 test vector 1
    check!("bip32", "2 m-0h-tv1", {
        let privkey = derive_key_at_path(&seed, &[(0, true)]).map_err(|e| e.to_string())?;
        let expected = hex::decode(
            "edb2e14f9ee77d26dd93b4ecede8d16ed408ce149b6cd80b0715a2d911a0afea"
        ).unwrap();
        if privkey != expected {
            return Err(format!(
                "m/0' mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&privkey), hex::encode(&expected)
            ));
        }
        Ok(())
    });

    // 3. m/0'/1 — privkey from BIP-32 test vector 1
    check!("bip32", "3 m-0h-1-tv1", {
        let privkey = derive_key_at_path(&seed, &[(0, true), (1, false)])
            .map_err(|e| e.to_string())?;
        let expected = hex::decode(
            "3c6cb8d0f6a264c91ea8b5030fadaa8e538b020f0a387421a12de9319dc93368"
        ).unwrap();
        if privkey != expected {
            return Err(format!(
                "m/0'/1 mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&privkey), hex::encode(&expected)
            ));
        }
        Ok(())
    });

    // 4. derive_address_at_path returns valid address and matching privkey
    check!("bip32", "4 address-derivation-valid", {
        let (address, pubkey_hex, privkey) = derive_address_at_path(&seed, &[])
            .map_err(|e| e.to_string())?;
        // Address should start with '1' (mainnet P2PKH)
        if !address.starts_with('1') {
            return Err(format!("address should start with '1', got '{}'", &address[..1]));
        }
        // Address should be 25-34 characters
        if address.len() < 25 || address.len() > 34 {
            return Err(format!("address length {} out of range 25-34", address.len()));
        }
        // Pubkey should be 66 hex chars (33 bytes compressed)
        if pubkey_hex.len() != 66 {
            return Err(format!("pubkey hex length {}, expected 66", pubkey_hex.len()));
        }
        // Pubkey should start with 02 or 03
        if !pubkey_hex.starts_with("02") && !pubkey_hex.starts_with("03") {
            return Err(format!("pubkey doesn't start with 02/03: {}", &pubkey_hex[..4]));
        }
        // Privkey should match master key
        let expected_privkey = hex::decode(
            "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35"
        ).unwrap();
        if privkey != expected_privkey {
            return Err("privkey from derive_address_at_path doesn't match derive_key_at_path".to_string());
        }
        Ok(())
    });

    // 5. address_to_p2pkh_script produces valid P2PKH locking script
    check!("bip32", "5 address-to-p2pkh-roundtrip", {
        let (address, _pubkey_hex, _privkey) = derive_address_at_path(&seed, &[])
            .map_err(|e| e.to_string())?;
        let script = address_to_p2pkh_script(&address).map_err(|e| e.to_string())?;
        // P2PKH locking script: OP_DUP(76) OP_HASH160(a9) PUSH20(14) <20 bytes> OP_EQUALVERIFY(88) OP_CHECKSIG(ac)
        if script.len() != 25 {
            return Err(format!("P2PKH script should be 25 bytes, got {}", script.len()));
        }
        if script[0] != 0x76 {
            return Err(format!("byte 0: expected OP_DUP (0x76), got 0x{:02x}", script[0]));
        }
        if script[1] != 0xa9 {
            return Err(format!("byte 1: expected OP_HASH160 (0xa9), got 0x{:02x}", script[1]));
        }
        if script[2] != 0x14 {
            return Err(format!("byte 2: expected PUSH20 (0x14), got 0x{:02x}", script[2]));
        }
        if script[23] != 0x88 {
            return Err(format!("byte 23: expected OP_EQUALVERIFY (0x88), got 0x{:02x}", script[23]));
        }
        if script[24] != 0xac {
            return Err(format!("byte 24: expected OP_CHECKSIG (0xac), got 0x{:02x}", script[24]));
        }
        Ok(())
    });

    // 6. address_to_p2pkh_script rejects invalid base58
    check!("bip32", "6 invalid-address-rejected", {
        match address_to_p2pkh_script("not-a-valid-address!!!") {
            Err(_) => Ok(()),
            Ok(_) => Err("invalid address should be rejected".to_string()),
        }
    });

    // 7. address_to_p2pkh_script rejects bad checksum
    check!("bip32", "7 bad-checksum-rejected", {
        let (address, _, _) = derive_address_at_path(&seed, &[]).map_err(|e| e.to_string())?;
        // Mangle the last character to corrupt checksum
        let mut chars: Vec<char> = address.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == '1' { '2' } else { '1' };
        let bad_address: String = chars.into_iter().collect();
        match address_to_p2pkh_script(&bad_address) {
            Err(e) if e.contains("checksum") => Ok(()),
            Err(e) => Ok(()), // Any error is acceptable for bad address
            Ok(_) => Err("corrupted checksum should be rejected".to_string()),
        }
    });

    // 8. Derivation determinism
    check!("bip32", "8 determinism", {
        let key1 = derive_key_at_path(&seed, &[(44, true), (0, false)])
            .map_err(|e| e.to_string())?;
        let key2 = derive_key_at_path(&seed, &[(44, true), (0, false)])
            .map_err(|e| e.to_string())?;
        if key1 != key2 {
            return Err("non-deterministic BIP32 derivation".to_string());
        }
        Ok(())
    });

    // 9. Different paths produce different keys
    check!("bip32", "9 different-paths-differ", {
        let key_a = derive_key_at_path(&seed, &[(0, false)]).map_err(|e| e.to_string())?;
        let key_b = derive_key_at_path(&seed, &[(1, false)]).map_err(|e| e.to_string())?;
        if key_a == key_b {
            return Err("m/0 and m/1 produced the same key".to_string());
        }
        Ok(())
    });

    // 10. Hardened vs non-hardened produce different keys
    check!("bip32", "10 hardened-vs-normal-differ", {
        let key_normal = derive_key_at_path(&seed, &[(0, false)]).map_err(|e| e.to_string())?;
        let key_hardened = derive_key_at_path(&seed, &[(0, true)]).map_err(|e| e.to_string())?;
        if key_normal == key_hardened {
            return Err("m/0 and m/0' produced the same key".to_string());
        }
        Ok(())
    });

    // 11. Centbee config path structure
    check!("bip32", "11 centbee-config-paths", {
        let config = hodos_wallet::recovery::ExternalWalletConfig::centbee();
        if config.name != "centbee" {
            return Err(format!("name: expected 'centbee', got '{}'", config.name));
        }
        if config.chains.len() != 2 {
            return Err(format!("expected 2 chains, got {}", config.chains.len()));
        }
        // Receive chain: m/44'/0/0/{i}
        let receive = &config.chains[0];
        if receive != &vec![(44, true), (0, false), (0, false)] {
            return Err(format!("receive chain path wrong: {:?}", receive));
        }
        // Change chain: m/44'/0/1/{i}
        let change = &config.chains[1];
        if change != &vec![(44, true), (0, false), (1, false)] {
            return Err(format!("change chain path wrong: {:?}", change));
        }
        Ok(())
    });

    // 12. Derive key at Centbee receive path m/44'/0/0/0
    check!("bip32", "12 centbee-receive-derivation", {
        let key = derive_key_at_path(&seed, &[(44, true), (0, false), (0, false), (0, false)])
            .map_err(|e| e.to_string())?;
        if key.len() != 32 {
            return Err(format!("expected 32-byte key, got {}", key.len()));
        }
        // Verify it's deterministic
        let key2 = derive_key_at_path(&seed, &[(44, true), (0, false), (0, false), (0, false)])
            .map_err(|e| e.to_string())?;
        if key != key2 {
            return Err("non-deterministic Centbee derivation".to_string());
        }
        Ok(())
    });
}

// ============================================================================
// [7/7] GHASH Known Vectors
// ============================================================================

fn section_7_ghash() {
    use hodos_wallet::crypto::ghash::{ghash, generate_hash_subkey};

    println!("  [7/7] GHASH Known Vectors");

    // 1. Hash subkey for all-zero AES-256 key — NIST known value
    //    AES-256-ECB(key=0x00..00, plaintext=0x00..00) = dc95c078a2408989ad48a21492842087
    check!("ghash", "1 hash-subkey-zero-key", {
        let zero_key = [0u8; 32];
        let h = generate_hash_subkey(&zero_key);
        let expected = hex::decode("dc95c078a2408989ad48a21492842087").unwrap();
        if h[..] != expected[..] {
            return Err(format!(
                "H mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&h), hex::encode(&expected)
            ));
        }
        Ok(())
    });

    // 2. GHASH with zero input → zero output
    check!("ghash", "2 zero-input-zero-output", {
        let h = generate_hash_subkey(&[0u8; 32]);
        let result = ghash(&[0u8; 16], &h);
        // ghash(0^16, H) = multiply(0^16 XOR 0^16, H) = multiply(0, H) = 0
        if result != [0u8; 16] {
            return Err(format!("expected all zeros, got {}", hex::encode(&result)));
        }
        Ok(())
    });

    // 3. GHASH output is always 16 bytes
    check!("ghash", "3 output-always-16-bytes", {
        let h = generate_hash_subkey(&[0x42u8; 32]);
        for size in &[0, 1, 15, 16, 17, 32, 48, 100] {
            let input = vec![0xAB; *size];
            let result = ghash(&input, &h);
            if result.len() != 16 {
                return Err(format!(
                    "{}-byte input: expected 16-byte output, got {}",
                    size, result.len()
                ));
            }
        }
        Ok(())
    });

    // 4. GHASH determinism
    check!("ghash", "4 determinism", {
        let h = generate_hash_subkey(&[0x11u8; 32]);
        let input = vec![0x22u8; 32];
        let r1 = ghash(&input, &h);
        let r2 = ghash(&input, &h);
        if r1 != r2 {
            return Err("non-deterministic GHASH".to_string());
        }
        Ok(())
    });

    // 5. Different inputs produce different outputs
    check!("ghash", "5 different-inputs-differ", {
        let h = generate_hash_subkey(&[0x33u8; 32]);
        let r1 = ghash(&[0x01u8; 16], &h);
        let r2 = ghash(&[0x02u8; 16], &h);
        if r1 == r2 {
            return Err("different inputs produced same GHASH".to_string());
        }
        Ok(())
    });

    // 6. Different keys produce different hash subkeys
    check!("ghash", "6 different-keys-different-h", {
        let h1 = generate_hash_subkey(&[0x00u8; 32]);
        let h2 = generate_hash_subkey(&[0x01u8; 32]);
        if h1 == h2 {
            return Err("different AES keys produced same hash subkey".to_string());
        }
        Ok(())
    });

    // 7. Non-aligned input (not multiple of 16) — should pad with zeros
    check!("ghash", "7 non-aligned-input", {
        let h = generate_hash_subkey(&[0x44u8; 32]);
        // 17-byte input: processed as 2 blocks (16 + 1 with 15 zero-padded)
        let input_17 = vec![0xAB; 17];
        let result = ghash(&input_17, &h);
        // Compare with manually constructing the padded 2-block version
        let mut padded_32 = vec![0xAB; 17];
        padded_32.extend(vec![0u8; 15]); // pad to 32 bytes
        let result_padded = ghash(&padded_32, &h);
        if result != result_padded {
            return Err(format!(
                "17-byte != 32-byte padded:\n  17: {}\n  32: {}",
                hex::encode(&result), hex::encode(&result_padded)
            ));
        }
        Ok(())
    });

    // 8. Empty input → zero output (no blocks to process)
    check!("ghash", "8 empty-input", {
        let h = generate_hash_subkey(&[0x55u8; 32]);
        let result = ghash(&[], &h);
        // Empty input → no iterations → result stays as initial zero
        if result != [0u8; 16] {
            return Err(format!("empty input should give zeros, got {}", hex::encode(&result)));
        }
        Ok(())
    });

    // 9. Single non-zero byte input
    check!("ghash", "9 single-byte-input", {
        let h = generate_hash_subkey(&[0x66u8; 32]);
        let result = ghash(&[0xFF], &h);
        // Single byte → 1 block with byte[0]=0xFF, rest zero-padded
        // Should be non-zero (0xFF XOR 0 = 0xFF in first byte, then multiply by H)
        if result == [0u8; 16] {
            return Err("single non-zero byte should produce non-zero output".to_string());
        }
        Ok(())
    });
}
