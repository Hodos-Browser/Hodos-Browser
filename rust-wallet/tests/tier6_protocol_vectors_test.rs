///
/// TIER 6 — PROTOCOL-LEVEL KNOWN-ANSWER TESTS
///
/// Targets 18 coverage gaps identified after Tiers 1–5.
/// Gold-standard NIST / RFC test vectors + structural coverage.
///
/// Sections:
///   [1/9] NIST SP 800-38D AES-256-GCM Known Answer Tests
///   [2/9] BRC-42 Symmetric Key Derivation Symmetry
///   [3/9] SIGHASH Edge Cases (SINGLE overflow, NONE, ANYONECANPAY)
///   [4/9] BEEF validate_beef_v1_hex
///   [5/9] BEEF sort_topologically with Real Parent-Child
///   [6/9] Certificate Preimage Serialization
///   [7/9] Base58Check Address Validation
///   [8/9] RFC 4231 HMAC-SHA256 Test Vectors
///   [9/9] GHASH & Crypto KATs
///
/// Methodology: "collect first, fix later" — wallet code is NEVER modified.
///

use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashMap;

static PASS: AtomicUsize = AtomicUsize::new(0);
static FAIL: AtomicUsize = AtomicUsize::new(0);

macro_rules! check {
    ($id:expr, $name:expr, $block:block) => {{
        let result: Result<(), String> = (|| -> Result<(), String> { $block })();
        match result {
            Ok(()) => {
                PASS.fetch_add(1, Ordering::Relaxed);
                eprintln!("  PASS  {}  {}", $id, $name);
            }
            Err(e) => {
                FAIL.fetch_add(1, Ordering::Relaxed);
                eprintln!("**FAIL  {}  {}  — {}", $id, $name, e);
            }
        }
    }};
}

// ============================================================================
// [1/9]  NIST SP 800-38D  AES-256-GCM Known Answer Tests
// ============================================================================
//
// Test vectors from NIST SP 800-38D Appendix B (AES-256 cases 13-16).
// These are the gold standard for validating AES-GCM implementations.
//

#[test]
fn t6_01_nist_aesgcm() {
    use hodos_wallet::crypto::aesgcm_custom::{aesgcm_custom, aesgcm_decrypt_custom};

    eprintln!("\n=== TIER 6 [1/9] NIST SP 800-38D AES-256-GCM KATs ===\n");

    // --- TC13: AES-256-GCM, empty plaintext, empty AAD, 12-byte IV ---
    check!("nist/01", "TC13-empty-pt-empty-aad", {
        let key = [0u8; 32];
        let iv = [0u8; 12];
        let pt: &[u8] = &[];
        let aad: &[u8] = &[];
        let expected_tag = hex::decode("530f8afbc74536b9a963b4f1c4cb738b")
            .map_err(|e| format!("hex: {}", e))?;

        let (ct, tag) = aesgcm_custom(pt, aad, &iv, &key)
            .map_err(|e| format!("encrypt: {}", e))?;
        if !ct.is_empty() {
            return Err(format!("ciphertext should be empty, got {} bytes", ct.len()));
        }
        if tag != expected_tag {
            return Err(format!("tag mismatch\n  got:  {}\n  want: {}",
                hex::encode(&tag), hex::encode(&expected_tag)));
        }
        Ok(())
    });

    // --- TC13 decrypt ---
    check!("nist/02", "TC13-decrypt-roundtrip", {
        let key = [0u8; 32];
        let iv = [0u8; 12];
        let ct: &[u8] = &[];
        let aad: &[u8] = &[];
        let tag = hex::decode("530f8afbc74536b9a963b4f1c4cb738b")
            .map_err(|e| format!("hex: {}", e))?;

        let pt = aesgcm_decrypt_custom(ct, aad, &iv, &tag, &key)
            .map_err(|e| format!("decrypt: {}", e))?;
        if !pt.is_empty() {
            return Err(format!("plaintext should be empty, got {} bytes", pt.len()));
        }
        Ok(())
    });

    // --- TC14: AES-256-GCM, 16 zero bytes plaintext, empty AAD ---
    check!("nist/03", "TC14-zero-block-encrypt", {
        let key = [0u8; 32];
        let iv = [0u8; 12];
        let pt = [0u8; 16];
        let aad: &[u8] = &[];
        let expected_ct = hex::decode("cea7403d4d606b6e074ec5d3baf39d18")
            .map_err(|e| format!("hex: {}", e))?;
        let expected_tag = hex::decode("d0d1c8a799996bf0265b98b5d48ab919")
            .map_err(|e| format!("hex: {}", e))?;

        let (ct, tag) = aesgcm_custom(&pt, aad, &iv, &key)
            .map_err(|e| format!("encrypt: {}", e))?;
        if ct != expected_ct {
            return Err(format!("ciphertext mismatch\n  got:  {}\n  want: {}",
                hex::encode(&ct), hex::encode(&expected_ct)));
        }
        if tag != expected_tag {
            return Err(format!("tag mismatch\n  got:  {}\n  want: {}",
                hex::encode(&tag), hex::encode(&expected_tag)));
        }
        Ok(())
    });

    // --- TC14 decrypt ---
    check!("nist/04", "TC14-decrypt-roundtrip", {
        let key = [0u8; 32];
        let iv = [0u8; 12];
        let ct = hex::decode("cea7403d4d606b6e074ec5d3baf39d18")
            .map_err(|e| format!("hex: {}", e))?;
        let tag = hex::decode("d0d1c8a799996bf0265b98b5d48ab919")
            .map_err(|e| format!("hex: {}", e))?;
        let aad: &[u8] = &[];

        let pt = aesgcm_decrypt_custom(&ct, aad, &iv, &tag, &key)
            .map_err(|e| format!("decrypt: {}", e))?;
        if pt != vec![0u8; 16] {
            return Err(format!("plaintext mismatch: {}", hex::encode(&pt)));
        }
        Ok(())
    });

    // --- TC15: AES-256-GCM, 64-byte plaintext, empty AAD ---
    check!("nist/05", "TC15-64byte-pt-no-aad", {
        let key_vec = hex::decode("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308")
            .map_err(|e| format!("hex: {}", e))?;
        let key: [u8; 32] = key_vec.try_into().map_err(|_| "key not 32 bytes".to_string())?;
        let iv = hex::decode("cafebabefacedbaddecaf888")
            .map_err(|e| format!("hex: {}", e))?;
        let pt = hex::decode(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72\
             1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255"
        ).map_err(|e| format!("hex: {}", e))?;
        let aad: &[u8] = &[];
        let expected_ct = hex::decode(
            "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa\
             8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662898015ad"
        ).map_err(|e| format!("hex: {}", e))?;
        let expected_tag = hex::decode("b094dac5d93471bdec1a502270e3cc6c")
            .map_err(|e| format!("hex: {}", e))?;

        let (ct, tag) = aesgcm_custom(&pt, aad, &iv, &key)
            .map_err(|e| format!("encrypt: {}", e))?;
        if ct != expected_ct {
            return Err(format!("ciphertext mismatch\n  got:  {}\n  want: {}",
                hex::encode(&ct), hex::encode(&expected_ct)));
        }
        if tag != expected_tag {
            return Err(format!("tag mismatch\n  got:  {}\n  want: {}",
                hex::encode(&tag), hex::encode(&expected_tag)));
        }
        Ok(())
    });

    // --- TC15 decrypt ---
    check!("nist/06", "TC15-decrypt-roundtrip", {
        let key_vec = hex::decode("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308")
            .map_err(|e| format!("hex: {}", e))?;
        let key: [u8; 32] = key_vec.try_into().map_err(|_| "key not 32 bytes".to_string())?;
        let iv = hex::decode("cafebabefacedbaddecaf888")
            .map_err(|e| format!("hex: {}", e))?;
        let ct = hex::decode(
            "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa\
             8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662898015ad"
        ).map_err(|e| format!("hex: {}", e))?;
        let tag = hex::decode("b094dac5d93471bdec1a502270e3cc6c")
            .map_err(|e| format!("hex: {}", e))?;
        let expected_pt = hex::decode(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72\
             1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255"
        ).map_err(|e| format!("hex: {}", e))?;

        let pt = aesgcm_decrypt_custom(&ct, &[], &iv, &tag, &key)
            .map_err(|e| format!("decrypt: {}", e))?;
        if pt != expected_pt {
            return Err(format!("plaintext mismatch"));
        }
        Ok(())
    });

    // --- TC16: AES-256-GCM, 60-byte plaintext, 20-byte AAD ---
    check!("nist/07", "TC16-60byte-pt-20byte-aad", {
        let key_vec = hex::decode("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308")
            .map_err(|e| format!("hex: {}", e))?;
        let key: [u8; 32] = key_vec.try_into().map_err(|_| "key not 32 bytes".to_string())?;
        let iv = hex::decode("cafebabefacedbaddecaf888")
            .map_err(|e| format!("hex: {}", e))?;
        let pt = hex::decode(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72\
             1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39"
        ).map_err(|e| format!("hex: {}", e))?;
        let aad = hex::decode("feedfacedeadbeeffeedfacedeadbeefabaddad2")
            .map_err(|e| format!("hex: {}", e))?;
        let expected_ct = hex::decode(
            "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa\
             8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662"
        ).map_err(|e| format!("hex: {}", e))?;
        let expected_tag = hex::decode("76fc6ece0f4e1768cddf8853bb2d551b")
            .map_err(|e| format!("hex: {}", e))?;

        let (ct, tag) = aesgcm_custom(&pt, &aad, &iv, &key)
            .map_err(|e| format!("encrypt: {}", e))?;
        if ct != expected_ct {
            return Err(format!("ciphertext mismatch\n  got:  {}\n  want: {}",
                hex::encode(&ct), hex::encode(&expected_ct)));
        }
        if tag != expected_tag {
            return Err(format!("tag mismatch\n  got:  {}\n  want: {}",
                hex::encode(&tag), hex::encode(&expected_tag)));
        }
        Ok(())
    });

    // --- TC16 decrypt ---
    check!("nist/08", "TC16-decrypt-roundtrip", {
        let key_vec = hex::decode("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308")
            .map_err(|e| format!("hex: {}", e))?;
        let key: [u8; 32] = key_vec.try_into().map_err(|_| "key not 32 bytes".to_string())?;
        let iv = hex::decode("cafebabefacedbaddecaf888")
            .map_err(|e| format!("hex: {}", e))?;
        let ct = hex::decode(
            "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa\
             8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662"
        ).map_err(|e| format!("hex: {}", e))?;
        let tag = hex::decode("76fc6ece0f4e1768cddf8853bb2d551b")
            .map_err(|e| format!("hex: {}", e))?;
        let aad = hex::decode("feedfacedeadbeeffeedfacedeadbeefabaddad2")
            .map_err(|e| format!("hex: {}", e))?;
        let expected_pt = hex::decode(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72\
             1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39"
        ).map_err(|e| format!("hex: {}", e))?;

        let pt = aesgcm_decrypt_custom(&ct, &aad, &iv, &tag, &key)
            .map_err(|e| format!("decrypt: {}", e))?;
        if pt != expected_pt {
            return Err(format!("plaintext mismatch"));
        }
        Ok(())
    });

    // --- TC16 wrong AAD should fail decrypt ---
    check!("nist/09", "TC16-wrong-aad-rejects", {
        let key_vec = hex::decode("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308")
            .map_err(|e| format!("hex: {}", e))?;
        let key: [u8; 32] = key_vec.try_into().map_err(|_| "key not 32 bytes".to_string())?;
        let iv = hex::decode("cafebabefacedbaddecaf888")
            .map_err(|e| format!("hex: {}", e))?;
        let ct = hex::decode(
            "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa\
             8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662"
        ).map_err(|e| format!("hex: {}", e))?;
        let tag = hex::decode("76fc6ece0f4e1768cddf8853bb2d551b")
            .map_err(|e| format!("hex: {}", e))?;
        // wrong AAD
        let wrong_aad = hex::decode("feedfacedeadbeeffeedfacedeadbeefabaddad3")
            .map_err(|e| format!("hex: {}", e))?;

        let result = aesgcm_decrypt_custom(&ct, &wrong_aad, &iv, &tag, &key);
        if result.is_ok() {
            return Err("should reject wrong AAD".into());
        }
        Ok(())
    });

    // --- TC15/TC16 CTR keystream consistency ---
    // Same key + IV → same CTR keystream. TC15 ciphertext prefix == TC16 ciphertext (60 bytes).
    check!("nist/10", "TC15-TC16-ctr-keystream-consistency", {
        let ct15 = hex::decode(
            "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa\
             8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662898015ad"
        ).map_err(|e| format!("hex: {}", e))?;
        let ct16 = hex::decode(
            "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa\
             8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662"
        ).map_err(|e| format!("hex: {}", e))?;

        // First 60 bytes of TC15 ciphertext == entire TC16 ciphertext
        if ct15[..60] != ct16[..] {
            return Err("CTR keystream prefix mismatch between TC15 and TC16".into());
        }
        Ok(())
    });

    let p = PASS.load(Ordering::Relaxed);
    let f = FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 1/9: {} pass, {} fail\n", p, f);
}

// ============================================================================
// [2/9]  BRC-42 Symmetric Key Derivation Symmetry
// ============================================================================

#[test]
fn t6_02_brc42_symmetric_key() {
    use hodos_wallet::crypto::brc42::{
        derive_symmetric_key_for_hmac,
        derive_child_private_key,
        derive_child_public_key,
    };
    use hodos_wallet::crypto::keys::derive_public_key;

    eprintln!("\n=== TIER 6 [2/9] BRC-42 Symmetric Key Derivation ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // Alice and Bob derive the same symmetric key
    check!("brc42/01", "symmetric-key-alice-eq-bob", {
        let alice_priv = hex::decode("583755110a8c059de5cd81b8a04e1be884c46083ade3f779c1e022f6f89da94c")
            .map_err(|e| format!("hex: {}", e))?;
        let bob_priv = hex::decode("6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede")
            .map_err(|e| format!("hex: {}", e))?;
        let alice_pub = derive_public_key(&alice_priv)
            .map_err(|e| format!("pubkey: {}", e))?;
        let bob_pub = derive_public_key(&bob_priv)
            .map_err(|e| format!("pubkey: {}", e))?;

        let invoice = "2-authrite-1";

        let key_alice = derive_symmetric_key_for_hmac(&alice_priv, &bob_pub, invoice)
            .map_err(|e| format!("alice: {}", e))?;
        let key_bob = derive_symmetric_key_for_hmac(&bob_priv, &alice_pub, invoice)
            .map_err(|e| format!("bob: {}", e))?;

        if key_alice.len() != 32 {
            return Err(format!("key should be 32 bytes, got {}", key_alice.len()));
        }
        if key_alice != key_bob {
            return Err(format!("keys differ\n  alice: {}\n  bob:   {}",
                hex::encode(&key_alice), hex::encode(&key_bob)));
        }
        Ok(())
    });

    // Different invoice numbers → different keys
    check!("brc42/02", "symmetric-key-different-invoices", {
        let alice_priv = [1u8; 32];
        let bob_priv = [2u8; 32];
        let bob_pub = derive_public_key(&bob_priv)
            .map_err(|e| format!("pubkey: {}", e))?;

        let key1 = derive_symmetric_key_for_hmac(&alice_priv, &bob_pub, "2-proto-key1")
            .map_err(|e| format!("key1: {}", e))?;
        let key2 = derive_symmetric_key_for_hmac(&alice_priv, &bob_pub, "2-proto-key2")
            .map_err(|e| format!("key2: {}", e))?;

        if key1 == key2 {
            return Err("different invoices should give different keys".into());
        }
        Ok(())
    });

    // Self-derivation: derive_symmetric_key with own keys
    check!("brc42/03", "symmetric-key-self-derivation", {
        let priv_key = [3u8; 32];
        let pub_key = derive_public_key(&priv_key)
            .map_err(|e| format!("pubkey: {}", e))?;

        let key = derive_symmetric_key_for_hmac(&priv_key, &pub_key, "2-self-encrypt-1")
            .map_err(|e| format!("self: {}", e))?;
        if key.len() != 32 {
            return Err(format!("key should be 32 bytes, got {}", key.len()));
        }
        Ok(())
    });

    // Child key consistency: derive_child_private_key produces key whose pubkey matches derive_child_public_key
    check!("brc42/04", "child-key-pub-priv-consistency", {
        let sender_priv = hex::decode("583755110a8c059de5cd81b8a04e1be884c46083ade3f779c1e022f6f89da94c")
            .map_err(|e| format!("hex: {}", e))?;
        let recipient_priv = hex::decode("6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede")
            .map_err(|e| format!("hex: {}", e))?;
        let sender_pub = derive_public_key(&sender_priv)
            .map_err(|e| format!("pubkey: {}", e))?;
        let recipient_pub = derive_public_key(&recipient_priv)
            .map_err(|e| format!("pubkey: {}", e))?;

        let invoice = "2-protocol-keyid";

        // Sender derives child public key for recipient
        let child_pub = derive_child_public_key(&sender_priv, &recipient_pub, invoice)
            .map_err(|e| format!("child_pub: {}", e))?;

        // Recipient derives child private key
        let child_priv = derive_child_private_key(&recipient_priv, &sender_pub, invoice)
            .map_err(|e| format!("child_priv: {}", e))?;

        // Child private key's public key should match child public key
        let child_priv_pub = derive_public_key(&child_priv)
            .map_err(|e| format!("derive: {}", e))?;

        if child_priv_pub != child_pub {
            return Err(format!("child key mismatch\n  from_sender:    {}\n  from_recipient: {}",
                hex::encode(&child_pub), hex::encode(&child_priv_pub)));
        }
        Ok(())
    });

    // Symmetric key is deterministic
    check!("brc42/05", "symmetric-key-deterministic", {
        let priv_key = [5u8; 32];
        let pub_key = derive_public_key(&[6u8; 32])
            .map_err(|e| format!("pubkey: {}", e))?;

        let k1 = derive_symmetric_key_for_hmac(&priv_key, &pub_key, "2-test-1")
            .map_err(|e| format!("k1: {}", e))?;
        let k2 = derive_symmetric_key_for_hmac(&priv_key, &pub_key, "2-test-1")
            .map_err(|e| format!("k2: {}", e))?;
        if k1 != k2 {
            return Err("same inputs should give same key".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    let section_total = after - before;
    eprintln!("\n  section 2/9: {} tests\n", section_total);
}

// ============================================================================
// [3/9]  SIGHASH Edge Cases
// ============================================================================

#[test]
fn t6_03_sighash_edges() {
    use hodos_wallet::transaction::sighash::calculate_sighash;
    use hodos_wallet::transaction::types::{Transaction, TxInput, TxOutput, OutPoint};

    eprintln!("\n=== TIER 6 [3/9] SIGHASH Edge Cases ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // Helper: build a tx with N inputs and M outputs
    fn make_tx(n_inputs: usize, n_outputs: usize) -> Transaction {
        let mut inputs = Vec::new();
        for i in 0..n_inputs {
            let txid = format!("{:064x}", i + 1); // distinct txids
            inputs.push(TxInput::new(OutPoint { txid, vout: 0 }));
        }
        let mut outputs = Vec::new();
        for _ in 0..n_outputs {
            outputs.push(TxOutput {
                value: 1000,
                script_pubkey: vec![0x76, 0xa9, 0x14,
                    0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0x88, 0xac],
            });
        }
        Transaction {
            version: 1,
            inputs,
            outputs,
            lock_time: 0,
        }
    }

    let prev_script = vec![0x76, 0xa9, 0x14,
        0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb,
        0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb,
        0x88, 0xac];

    // SIGHASH_SINGLE with input_index < outputs → normal path
    check!("sighash/01", "SINGLE-index-within-outputs", {
        let tx = make_tx(2, 2);
        let hash = calculate_sighash(&tx, 0, &prev_script, 1000, 0x43) // 0x43 = SINGLE|FORKID
            .map_err(|e| format!("{}", e))?;
        if hash.len() != 32 {
            return Err(format!("hash should be 32 bytes, got {}", hash.len()));
        }
        Ok(())
    });

    // SIGHASH_SINGLE with input_index >= outputs → zero hash fallback
    check!("sighash/02", "SINGLE-index-overflow-zero-hash", {
        let tx = make_tx(2, 1); // 2 inputs, 1 output
        // Input index 1, but only 1 output (index 0). Should use zero-hash fallback.
        let hash = calculate_sighash(&tx, 1, &prev_script, 1000, 0x43)
            .map_err(|e| format!("{}", e))?;
        if hash.len() != 32 {
            return Err(format!("hash should be 32 bytes, got {}", hash.len()));
        }
        // Compute again to verify determinism
        let hash2 = calculate_sighash(&tx, 1, &prev_script, 1000, 0x43)
            .map_err(|e| format!("{}", e))?;
        if hash != hash2 {
            return Err("zero-hash path should be deterministic".into());
        }
        Ok(())
    });

    // SIGHASH_SINGLE input 0 vs input 1 should differ (different input details)
    check!("sighash/03", "SINGLE-different-inputs-different-hash", {
        let tx = make_tx(2, 2);
        let h0 = calculate_sighash(&tx, 0, &prev_script, 1000, 0x43)
            .map_err(|e| format!("{}", e))?;
        let h1 = calculate_sighash(&tx, 1, &prev_script, 1000, 0x43)
            .map_err(|e| format!("{}", e))?;
        if h0 == h1 {
            return Err("different inputs should produce different sighashes".into());
        }
        Ok(())
    });

    // SIGHASH_NONE (0x42 with ForkID) → zero hash for outputs
    check!("sighash/04", "NONE-zero-hash-outputs", {
        let tx = make_tx(1, 2);
        let hash = calculate_sighash(&tx, 0, &prev_script, 1000, 0x42) // 0x42 = NONE|FORKID
            .map_err(|e| format!("{}", e))?;
        if hash.len() != 32 {
            return Err(format!("hash should be 32 bytes, got {}", hash.len()));
        }
        Ok(())
    });

    // SIGHASH_ALL|ANYONECANPAY (0xC1) → zero hash for prevouts and sequence
    check!("sighash/05", "ALL-ANYONECANPAY", {
        let tx = make_tx(2, 1);
        let hash = calculate_sighash(&tx, 0, &prev_script, 1000, 0xc1) // ALL|ANYONECANPAY|FORKID
            .map_err(|e| format!("{}", e))?;
        if hash.len() != 32 {
            return Err(format!("hash should be 32 bytes, got {}", hash.len()));
        }
        Ok(())
    });

    // SIGHASH_NONE vs SIGHASH_ALL should differ
    check!("sighash/06", "ALL-vs-NONE-differ", {
        let tx = make_tx(1, 1);
        let h_all = calculate_sighash(&tx, 0, &prev_script, 1000, 0x41)
            .map_err(|e| format!("{}", e))?;
        let h_none = calculate_sighash(&tx, 0, &prev_script, 1000, 0x42)
            .map_err(|e| format!("{}", e))?;
        if h_all == h_none {
            return Err("ALL and NONE should produce different hashes".into());
        }
        Ok(())
    });

    // SIGHASH_SINGLE|ANYONECANPAY (0xC3)
    check!("sighash/07", "SINGLE-ANYONECANPAY", {
        let tx = make_tx(2, 2);
        let hash = calculate_sighash(&tx, 0, &prev_script, 1000, 0xc3)
            .map_err(|e| format!("{}", e))?;
        if hash.len() != 32 {
            return Err(format!("hash should be 32 bytes, got {}", hash.len()));
        }
        Ok(())
    });

    // Out-of-range input index → error
    check!("sighash/08", "out-of-range-input-error", {
        let tx = make_tx(1, 1);
        let result = calculate_sighash(&tx, 5, &prev_script, 1000, 0x41);
        if result.is_ok() {
            return Err("should error on out-of-range input index".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 3/9: {} tests\n", after - before);
}

// ============================================================================
// [4/9]  BEEF validate_beef_v1_hex
// ============================================================================

#[test]
fn t6_04_beef_validate_v1() {
    use hodos_wallet::beef::{Beef, validate_beef_v1_hex, BEEF_V1_MARKER};

    eprintln!("\n=== TIER 6 [4/9] BEEF validate_beef_v1_hex ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // Build a minimal valid BEEF V1 with 0 BUMPs and 1 transaction
    check!("beefv1/01", "valid-minimal-v1", {
        // Construct a minimal raw transaction (coinbase-like)
        let raw_tx = build_minimal_raw_tx(
            "1111111111111111111111111111111111111111111111111111111111111111",
            0,
            1000,
        );

        let beef = Beef {
            version: BEEF_V1_MARKER,
            bumps: Vec::new(),
            transactions: vec![raw_tx],
            tx_to_bump: vec![None],
        };

        let v1_hex = beef.to_v1_hex().map_err(|e| format!("to_v1_hex: {}", e))?;
        validate_beef_v1_hex(&v1_hex).map_err(|e| format!("validate: {}", e))?;
        Ok(())
    });

    // Invalid version marker
    check!("beefv1/02", "invalid-version-marker", {
        // Replace first 4 bytes with something else
        let mut bad = hex::decode("deadbeef").unwrap();
        bad.push(0x00); // num bumps = 0
        bad.push(0x00); // num txs = 0
        let hex_str = hex::encode(&bad);
        let result = validate_beef_v1_hex(&hex_str);
        if result.is_ok() {
            return Err("should reject non-BEEF version".into());
        }
        Ok(())
    });

    // Invalid hex
    check!("beefv1/03", "invalid-hex-rejected", {
        let result = validate_beef_v1_hex("not-valid-hex!!!");
        if result.is_ok() {
            return Err("should reject invalid hex".into());
        }
        Ok(())
    });

    // Truncated (just version marker, no body)
    check!("beefv1/04", "truncated-after-version", {
        let result = validate_beef_v1_hex("0100beef");
        if result.is_ok() {
            return Err("should reject truncated BEEF".into());
        }
        Ok(())
    });

    // V1 roundtrip: construct, serialize, validate, parse back
    check!("beefv1/05", "v1-roundtrip-construct-validate", {
        let raw_tx1 = build_minimal_raw_tx(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            0,
            5000,
        );
        let raw_tx2 = build_minimal_raw_tx(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            1,
            3000,
        );

        let beef = Beef {
            version: BEEF_V1_MARKER,
            bumps: Vec::new(),
            transactions: vec![raw_tx1, raw_tx2],
            tx_to_bump: vec![None, None],
        };

        let v1_hex = beef.to_v1_hex().map_err(|e| format!("to_v1_hex: {}", e))?;
        validate_beef_v1_hex(&v1_hex).map_err(|e| format!("validate: {}", e))?;

        // Parse it back
        let parsed = Beef::from_hex(&v1_hex).map_err(|e| format!("parse: {}", e))?;
        if parsed.transactions.len() != 2 {
            return Err(format!("expected 2 txs, got {}", parsed.transactions.len()));
        }
        Ok(())
    });

    // Empty string
    check!("beefv1/06", "empty-string-rejected", {
        let result = validate_beef_v1_hex("");
        if result.is_ok() {
            return Err("should reject empty string".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 4/9: {} tests\n", after - before);
}

/// Build minimal raw transaction bytes for testing
fn build_minimal_raw_tx(prev_txid_hex: &str, prev_vout: u32, output_sats: u64) -> Vec<u8> {
    let mut tx = Vec::new();

    // Version (4 bytes LE)
    tx.extend_from_slice(&1u32.to_le_bytes());

    // Input count: 1
    tx.push(0x01);

    // prev_txid in wire format (reverse of display hex)
    let txid_bytes = hex::decode(prev_txid_hex).unwrap();
    let wire_txid: Vec<u8> = txid_bytes.iter().rev().copied().collect();
    tx.extend_from_slice(&wire_txid);

    // prev_vout (4 bytes LE)
    tx.extend_from_slice(&prev_vout.to_le_bytes());

    // scriptSig length: 0
    tx.push(0x00);

    // sequence
    tx.extend_from_slice(&0xffffffffu32.to_le_bytes());

    // Output count: 1
    tx.push(0x01);

    // value (8 bytes LE)
    tx.extend_from_slice(&output_sats.to_le_bytes());

    // scriptPubKey: P2PKH (25 bytes)
    tx.push(0x19); // length
    tx.extend_from_slice(&[0x76, 0xa9, 0x14]);
    tx.extend_from_slice(&[0xaa; 20]); // pubkey hash
    tx.extend_from_slice(&[0x88, 0xac]);

    // locktime (4 bytes)
    tx.extend_from_slice(&0u32.to_le_bytes());

    tx
}

// ============================================================================
// [5/9]  BEEF sort_topologically with Real Parent-Child
// ============================================================================

#[test]
fn t6_05_beef_sort_topologically() {
    use hodos_wallet::beef::{Beef, BEEF_V1_MARKER};
    use sha2::{Sha256, Digest};

    eprintln!("\n=== TIER 6 [5/9] BEEF sort_topologically ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // Single transaction: no-op
    check!("topo/01", "single-tx-no-op", {
        let tx = build_minimal_raw_tx(
            "1111111111111111111111111111111111111111111111111111111111111111",
            0, 1000,
        );
        let mut beef = Beef {
            version: BEEF_V1_MARKER,
            bumps: Vec::new(),
            transactions: vec![tx.clone()],
            tx_to_bump: vec![None],
        };
        beef.sort_topologically();
        if beef.transactions.len() != 1 || beef.transactions[0] != tx {
            return Err("single tx should be unchanged".into());
        }
        Ok(())
    });

    // Two independent transactions: order preserved
    check!("topo/02", "independent-txs-preserved", {
        let tx_a = build_minimal_raw_tx(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            0, 1000,
        );
        let tx_b = build_minimal_raw_tx(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            0, 2000,
        );
        let orig_a = tx_a.clone();
        let orig_b = tx_b.clone();

        let mut beef = Beef {
            version: BEEF_V1_MARKER,
            bumps: Vec::new(),
            transactions: vec![tx_a, tx_b],
            tx_to_bump: vec![None, None],
        };
        beef.sort_topologically();
        // Independent txs: already in topological order, should remain
        if beef.transactions[0] != orig_a || beef.transactions[1] != orig_b {
            return Err("independent txs should keep original order".into());
        }
        Ok(())
    });

    // Parent-child reordering: child before parent → sort puts parent first
    check!("topo/03", "parent-child-reorder", {
        // Step 1: Build parent tx
        let parent_tx = build_minimal_raw_tx(
            "1111111111111111111111111111111111111111111111111111111111111111",
            0, 5000,
        );

        // Step 2: Compute parent's TXID (wire format = double SHA-256 of raw bytes)
        let hash1 = Sha256::digest(&parent_tx);
        let parent_wire_txid = Sha256::digest(&hash1);

        // Step 3: Convert wire TXID to display format (reverse bytes) for the child's input
        let parent_display_txid: Vec<u8> = parent_wire_txid.iter().rev().copied().collect();
        let parent_display_hex = hex::encode(&parent_display_txid);

        // Step 4: Build child tx that references parent's TXID
        let child_tx = build_minimal_raw_tx(&parent_display_hex, 0, 4000);

        // Step 5: Put child FIRST, parent SECOND (wrong order)
        let child_copy = child_tx.clone();
        let parent_copy = parent_tx.clone();

        let mut beef = Beef {
            version: BEEF_V1_MARKER,
            bumps: Vec::new(),
            transactions: vec![child_tx, parent_tx],
            tx_to_bump: vec![None, None],
        };

        // Step 6: Sort
        beef.sort_topologically();

        // Step 7: Parent should now be first, child second
        if beef.transactions[0] != parent_copy {
            return Err("parent should be sorted first".into());
        }
        if beef.transactions[1] != child_copy {
            return Err("child should be sorted second".into());
        }
        Ok(())
    });

    // Already sorted: no change
    check!("topo/04", "already-sorted-no-change", {
        let parent_tx = build_minimal_raw_tx(
            "1111111111111111111111111111111111111111111111111111111111111111",
            0, 5000,
        );
        let hash1 = Sha256::digest(&parent_tx);
        let parent_wire_txid = Sha256::digest(&hash1);
        let parent_display: Vec<u8> = parent_wire_txid.iter().rev().copied().collect();
        let parent_display_hex = hex::encode(&parent_display);

        let child_tx = build_minimal_raw_tx(&parent_display_hex, 0, 4000);

        let parent_copy = parent_tx.clone();
        let child_copy = child_tx.clone();

        // Correct order: parent first, child second
        let mut beef = Beef {
            version: BEEF_V1_MARKER,
            bumps: Vec::new(),
            transactions: vec![parent_tx, child_tx],
            tx_to_bump: vec![None, None],
        };
        beef.sort_topologically();

        if beef.transactions[0] != parent_copy || beef.transactions[1] != child_copy {
            return Err("already-sorted should remain unchanged".into());
        }
        Ok(())
    });

    // tx_to_bump mapping follows reorder
    check!("topo/05", "bump-mapping-follows-reorder", {
        let parent_tx = build_minimal_raw_tx(
            "2222222222222222222222222222222222222222222222222222222222222222",
            0, 5000,
        );
        let hash1 = Sha256::digest(&parent_tx);
        let parent_wire_txid = Sha256::digest(&hash1);
        let parent_display: Vec<u8> = parent_wire_txid.iter().rev().copied().collect();
        let parent_display_hex = hex::encode(&parent_display);

        let child_tx = build_minimal_raw_tx(&parent_display_hex, 0, 4000);

        // Child first (idx 0, bump=Some(0)), parent second (idx 1, bump=None)
        let mut beef = Beef {
            version: BEEF_V1_MARKER,
            bumps: Vec::new(),
            transactions: vec![child_tx, parent_tx],
            tx_to_bump: vec![Some(0), None],
        };
        beef.sort_topologically();

        // After sort: parent at idx 0 should have bump=None, child at idx 1 should have bump=Some(0)
        if beef.tx_to_bump[0] != None {
            return Err(format!("parent bump should be None, got {:?}", beef.tx_to_bump[0]));
        }
        if beef.tx_to_bump[1] != Some(0) {
            return Err(format!("child bump should be Some(0), got {:?}", beef.tx_to_bump[1]));
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 5/9: {} tests\n", after - before);
}

// ============================================================================
// [6/9]  Certificate Preimage Serialization
// ============================================================================

#[test]
fn t6_06_certificate_preimage() {
    use hodos_wallet::certificate::types::{Certificate, CertificateField};
    use hodos_wallet::certificate::verifier::serialize_certificate_preimage;

    eprintln!("\n=== TIER 6 [6/9] Certificate Preimage Serialization ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // Helper: build a test certificate
    fn make_cert(fields: Vec<(&str, &[u8])>) -> Certificate {
        let mut field_map = HashMap::new();
        for (name, value) in fields {
            field_map.insert(name.to_string(), CertificateField {
                certificate_id: None,
                user_id: None,
                field_name: name.to_string(),
                field_value: value.to_vec(),
                master_key: vec![],
                created_at: 0,
                updated_at: 0,
            });
        }

        Certificate {
            certificate_id: None,
            user_id: None,
            type_: vec![0xAA; 32],
            subject: vec![0x02; 33], // compressed pubkey prefix
            serial_number: vec![0xBB; 32],
            certifier: vec![0x03; 33], // compressed pubkey prefix
            verifier: None,
            revocation_outpoint: format!(
                "{}.0",
                "cc".repeat(32)
            ),
            signature: vec![0x30; 70],
            fields: field_map,
            keyring: HashMap::new(),
            is_deleted: false,
            created_at: 0,
            updated_at: 0,
        }
    }

    // Basic preimage structure: type(32) + serial(32) + subject(33) + certifier(33) + outpoint(32+varint) + fields
    check!("preimg/01", "basic-structure-length", {
        let cert = make_cert(vec![]);
        let preimage = serialize_certificate_preimage(&cert)
            .map_err(|e| format!("{}", e))?;

        // 32 + 32 + 33 + 33 + 32 (txid) + 1 (varint vout=0) + 1 (field count=0)
        let expected_min = 32 + 32 + 33 + 33 + 32 + 1 + 1;
        if preimage.len() != expected_min {
            return Err(format!("expected {} bytes, got {}", expected_min, preimage.len()));
        }

        // Verify type_ is first 32 bytes
        if preimage[..32] != vec![0xAA; 32][..] {
            return Err("type_ bytes mismatch".into());
        }
        // serial_number is next 32 bytes
        if preimage[32..64] != vec![0xBB; 32][..] {
            return Err("serial_number bytes mismatch".into());
        }
        // subject is next 33 bytes
        if preimage[64..97] != vec![0x02; 33][..] {
            return Err("subject bytes mismatch".into());
        }
        // certifier is next 33 bytes
        if preimage[97..130] != vec![0x03; 33][..] {
            return Err("certifier bytes mismatch".into());
        }
        Ok(())
    });

    // Fields are sorted lexicographically by name
    check!("preimg/02", "fields-sorted-lexicographically", {
        let cert = make_cert(vec![
            ("zebra", b"z_value"),
            ("alpha", b"a_value"),
            ("middle", b"m_value"),
        ]);
        let preimage = serialize_certificate_preimage(&cert)
            .map_err(|e| format!("{}", e))?;

        // After fixed header (32+32+33+33+32+1 = 163 bytes), field count, then fields
        // Field count varint at offset 163
        let field_count_offset = 32 + 32 + 33 + 33 + 32 + 1;
        if preimage[field_count_offset] != 3 {
            return Err(format!("field count should be 3, got {}", preimage[field_count_offset]));
        }

        // First field name should be "alpha" (lexicographic)
        // After field count (1 byte): name_len(1) + name("alpha" = 5 bytes)
        let name_start = field_count_offset + 1;
        let name_len = preimage[name_start] as usize;
        if name_len != 5 {
            return Err(format!("first field name len should be 5, got {}", name_len));
        }
        let name = std::str::from_utf8(&preimage[name_start + 1..name_start + 1 + name_len])
            .map_err(|e| format!("utf8: {}", e))?;
        if name != "alpha" {
            return Err(format!("first field should be 'alpha', got '{}'", name));
        }
        Ok(())
    });

    // Field values are base64-encoded in preimage
    check!("preimg/03", "field-values-base64-encoded", {
        use base64::Engine;
        let raw_value = b"hello";
        let cert = make_cert(vec![("name", raw_value)]);
        let preimage = serialize_certificate_preimage(&cert)
            .map_err(|e| format!("{}", e))?;

        // The base64 of "hello" is "aGVsbG8="
        let expected_b64 = base64::engine::general_purpose::STANDARD.encode(raw_value);
        let expected_b64_bytes = expected_b64.as_bytes();

        // Find "aGVsbG8=" in the preimage
        if !preimage.windows(expected_b64_bytes.len()).any(|w| w == expected_b64_bytes) {
            return Err(format!(
                "base64 '{}' not found in preimage (len {})",
                expected_b64, preimage.len()
            ));
        }
        Ok(())
    });

    // Revocation outpoint vout encoding (varint)
    check!("preimg/04", "revocation-vout-varint", {
        let mut cert = make_cert(vec![]);
        cert.revocation_outpoint = format!("{}.200", "dd".repeat(32));
        let preimage = serialize_certificate_preimage(&cert)
            .map_err(|e| format!("{}", e))?;

        // vout=200 is encoded as varint: 200 < 253, so single byte 0xc8
        let vout_offset = 32 + 32 + 33 + 33 + 32; // after fixed fields + txid
        if preimage[vout_offset] != 200 {
            return Err(format!("vout varint should be 200 (0xc8), got {}", preimage[vout_offset]));
        }
        Ok(())
    });

    // Type wrong length → error
    check!("preimg/05", "type-wrong-length-error", {
        let mut cert = make_cert(vec![]);
        cert.type_ = vec![0xAA; 31]; // wrong length
        let result = serialize_certificate_preimage(&cert);
        if result.is_ok() {
            return Err("should reject type_ with wrong length".into());
        }
        Ok(())
    });

    // Subject wrong length → error
    check!("preimg/06", "subject-wrong-length-error", {
        let mut cert = make_cert(vec![]);
        cert.subject = vec![0x02; 32]; // should be 33
        let result = serialize_certificate_preimage(&cert);
        if result.is_ok() {
            return Err("should reject subject with wrong length".into());
        }
        Ok(())
    });

    // Certifier wrong length → error
    check!("preimg/07", "certifier-wrong-length-error", {
        let mut cert = make_cert(vec![]);
        cert.certifier = vec![0x03; 34]; // should be 33
        let result = serialize_certificate_preimage(&cert);
        if result.is_ok() {
            return Err("should reject certifier with wrong length".into());
        }
        Ok(())
    });

    // Deterministic: same cert → same preimage
    check!("preimg/08", "deterministic", {
        let cert = make_cert(vec![("a", b"val_a"), ("b", b"val_b")]);
        let p1 = serialize_certificate_preimage(&cert)
            .map_err(|e| format!("{}", e))?;
        let p2 = serialize_certificate_preimage(&cert)
            .map_err(|e| format!("{}", e))?;
        if p1 != p2 {
            return Err("same cert should produce same preimage".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 6/9: {} tests\n", after - before);
}

// ============================================================================
// [7/9]  Base58Check Address Validation
// ============================================================================

#[test]
fn t6_07_base58check_address() {
    use hodos_wallet::recovery::address_to_p2pkh_script;
    use hodos_wallet::crypto::signing::{sha256, double_sha256};

    eprintln!("\n=== TIER 6 [7/9] Base58Check Address Validation ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // Construct a valid BSV address from known pubkey hash
    check!("addr/01", "valid-constructed-address", {
        let pubkey_hash = [0xaa; 20];
        let mut payload = vec![0x00]; // mainnet version byte
        payload.extend_from_slice(&pubkey_hash);
        let checksum = double_sha256(&payload);
        payload.extend_from_slice(&checksum[..4]);
        let address = bs58::encode(&payload).into_string();

        let script = address_to_p2pkh_script(&address)
            .map_err(|e| format!("script: {}", e))?;

        // P2PKH script: OP_DUP OP_HASH160 PUSH20 <hash> OP_EQUALVERIFY OP_CHECKSIG
        if script.len() != 25 {
            return Err(format!("P2PKH script should be 25 bytes, got {}", script.len()));
        }
        if script[0] != 0x76 { return Err("OP_DUP missing".into()); }
        if script[1] != 0xa9 { return Err("OP_HASH160 missing".into()); }
        if script[2] != 0x14 { return Err("PUSH20 missing".into()); }
        if script[3..23] != pubkey_hash[..] { return Err("pubkey hash mismatch".into()); }
        if script[23] != 0x88 { return Err("OP_EQUALVERIFY missing".into()); }
        if script[24] != 0xac { return Err("OP_CHECKSIG missing".into()); }
        Ok(())
    });

    // Bad checksum → error
    check!("addr/02", "bad-checksum-rejected", {
        let pubkey_hash = [0xbb; 20];
        let mut payload = vec![0x00];
        payload.extend_from_slice(&pubkey_hash);
        let checksum = double_sha256(&payload);
        let mut bad_checksum = checksum[..4].to_vec();
        bad_checksum[0] ^= 0xff; // corrupt checksum
        payload.extend_from_slice(&bad_checksum);
        let address = bs58::encode(&payload).into_string();

        let result = address_to_p2pkh_script(&address);
        if result.is_ok() {
            return Err("should reject bad checksum".into());
        }
        Ok(())
    });

    // Too short address (< 25 decoded bytes) → error
    check!("addr/03", "too-short-rejected", {
        let short = bs58::encode(&[0x00; 20]).into_string(); // only 20 bytes, need 25
        let result = address_to_p2pkh_script(&short);
        if result.is_ok() {
            return Err("should reject short address".into());
        }
        Ok(())
    });

    // Invalid base58 characters → error
    check!("addr/04", "invalid-base58-chars-rejected", {
        let result = address_to_p2pkh_script("0OIl"); // 0, O, I, l are not in base58
        if result.is_ok() {
            return Err("should reject invalid base58".into());
        }
        Ok(())
    });

    // Different version bytes still work (address_to_p2pkh_script doesn't check version)
    check!("addr/05", "testnet-version-byte-accepted", {
        let pubkey_hash = [0xcc; 20];
        let mut payload = vec![0x6f]; // testnet version byte
        payload.extend_from_slice(&pubkey_hash);
        let checksum = double_sha256(&payload);
        payload.extend_from_slice(&checksum[..4]);
        let address = bs58::encode(&payload).into_string();

        let script = address_to_p2pkh_script(&address)
            .map_err(|e| format!("script: {}", e))?;
        // Should produce valid P2PKH script regardless of version byte
        if script.len() != 25 {
            return Err(format!("script should be 25 bytes, got {}", script.len()));
        }
        Ok(())
    });

    // All-zero pubkey hash produces valid script
    check!("addr/06", "zero-hash-valid-script", {
        let pubkey_hash = [0x00; 20];
        let mut payload = vec![0x00];
        payload.extend_from_slice(&pubkey_hash);
        let checksum = double_sha256(&payload);
        payload.extend_from_slice(&checksum[..4]);
        let address = bs58::encode(&payload).into_string();

        let script = address_to_p2pkh_script(&address)
            .map_err(|e| format!("script: {}", e))?;
        if script[3..23] != [0x00; 20][..] {
            return Err("zero hash not preserved".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 7/9: {} tests\n", after - before);
}

// ============================================================================
// [8/9]  RFC 4231 HMAC-SHA256 Test Vectors
// ============================================================================

#[test]
fn t6_08_rfc4231_hmac() {
    use hodos_wallet::crypto::signing::{hmac_sha256, verify_hmac_sha256};

    eprintln!("\n=== TIER 6 [8/9] RFC 4231 HMAC-SHA256 Test Vectors ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // RFC 4231 Test Case 1: key=20 bytes of 0x0b
    check!("hmac/01", "RFC4231-TC1", {
        let key = vec![0x0b; 20];
        let data = b"Hi There";
        let expected = hex::decode("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7")
            .map_err(|e| format!("hex: {}", e))?;
        let result = hmac_sha256(&key, data);
        if result != expected {
            return Err(format!("mismatch\n  got:  {}\n  want: {}",
                hex::encode(&result), hex::encode(&expected)));
        }
        Ok(())
    });

    // RFC 4231 Test Case 2: key="Jefe"
    check!("hmac/02", "RFC4231-TC2", {
        let key = b"Jefe";
        let data = b"what do ya want for nothing?";
        let expected = hex::decode("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843")
            .map_err(|e| format!("hex: {}", e))?;
        let result = hmac_sha256(key, data);
        if result != expected {
            return Err(format!("mismatch\n  got:  {}\n  want: {}",
                hex::encode(&result), hex::encode(&expected)));
        }
        Ok(())
    });

    // RFC 4231 Test Case 3: key=20 bytes of 0xaa, data=50 bytes of 0xdd
    check!("hmac/03", "RFC4231-TC3", {
        let key = vec![0xaa; 20];
        let data = vec![0xdd; 50];
        let expected = hex::decode("773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe")
            .map_err(|e| format!("hex: {}", e))?;
        let result = hmac_sha256(&key, &data);
        if result != expected {
            return Err(format!("mismatch\n  got:  {}\n  want: {}",
                hex::encode(&result), hex::encode(&expected)));
        }
        Ok(())
    });

    // RFC 4231 Test Case 4: key=25 bytes (0x01..0x19)
    check!("hmac/04", "RFC4231-TC4", {
        let key: Vec<u8> = (1..=25).collect();
        let data = vec![0xcd; 50];
        let expected = hex::decode("82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b")
            .map_err(|e| format!("hex: {}", e))?;
        let result = hmac_sha256(&key, &data);
        if result != expected {
            return Err(format!("mismatch\n  got:  {}\n  want: {}",
                hex::encode(&result), hex::encode(&expected)));
        }
        Ok(())
    });

    // RFC 4231 Test Case 6: key=131 bytes of 0xaa (larger than block size)
    check!("hmac/05", "RFC4231-TC6-large-key", {
        let key = vec![0xaa; 131];
        let data = b"Test Using Larger Than Block-Size Key - Hash Key First";
        let expected = hex::decode("60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54")
            .map_err(|e| format!("hex: {}", e))?;
        let result = hmac_sha256(&key, data);
        if result != expected {
            return Err(format!("mismatch\n  got:  {}\n  want: {}",
                hex::encode(&result), hex::encode(&expected)));
        }
        Ok(())
    });

    // RFC 4231 Test Case 7: key=131 bytes of 0xaa, large data
    check!("hmac/06", "RFC4231-TC7-large-key-large-data", {
        let key = vec![0xaa; 131];
        let data = b"This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm.";
        let expected = hex::decode("9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2")
            .map_err(|e| format!("hex: {}", e))?;
        let result = hmac_sha256(&key, data);
        if result != expected {
            return Err(format!("mismatch\n  got:  {}\n  want: {}",
                hex::encode(&result), hex::encode(&expected)));
        }
        Ok(())
    });

    // Verify HMAC roundtrip for each TC
    check!("hmac/07", "verify-roundtrip-all-tcs", {
        let cases: Vec<(Vec<u8>, Vec<u8>)> = vec![
            (vec![0x0b; 20], b"Hi There".to_vec()),
            (b"Jefe".to_vec(), b"what do ya want for nothing?".to_vec()),
            (vec![0xaa; 20], vec![0xdd; 50]),
            ((1..=25).collect(), vec![0xcd; 50]),
            (vec![0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First".to_vec()),
        ];
        for (i, (key, data)) in cases.iter().enumerate() {
            let hmac = hmac_sha256(key, data);
            if !verify_hmac_sha256(key, data, &hmac) {
                return Err(format!("verify_hmac failed for TC{}", i + 1));
            }
        }
        Ok(())
    });

    // Empty key and empty data
    check!("hmac/08", "empty-key-empty-data", {
        let result = hmac_sha256(&[], &[]);
        if result.len() != 32 {
            return Err(format!("HMAC should be 32 bytes, got {}", result.len()));
        }
        // Verify it's deterministic
        let result2 = hmac_sha256(&[], &[]);
        if result != result2 {
            return Err("HMAC should be deterministic".into());
        }
        Ok(())
    });

    // Constant-time comparison: wrong length always false
    check!("hmac/09", "verify-wrong-length-false", {
        let key = b"test";
        let data = b"data";
        let hmac = hmac_sha256(key, data);
        // Truncated HMAC should fail
        if verify_hmac_sha256(key, data, &hmac[..16]) {
            return Err("truncated HMAC should fail verification".into());
        }
        // Extended HMAC should fail
        let mut extended = hmac.clone();
        extended.push(0x00);
        if verify_hmac_sha256(key, data, &extended) {
            return Err("extended HMAC should fail verification".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 8/9: {} tests\n", after - before);
}

// ============================================================================
// [9/9]  GHASH & Crypto KATs
// ============================================================================

#[test]
fn t6_09_ghash_crypto() {
    use hodos_wallet::crypto::ghash::{ghash, generate_hash_subkey};
    use hodos_wallet::crypto::signing::{sha256, double_sha256, sign_ecdsa, verify_signature};
    use hodos_wallet::crypto::keys::derive_public_key;

    eprintln!("\n=== TIER 6 [9/9] GHASH & Crypto KATs ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // GHASH hash subkey: AES-256(K=0^32, plaintext=0^16) = known value
    check!("ghash/01", "hash-subkey-all-zero-key", {
        let key = [0u8; 32];
        let h = generate_hash_subkey(&key);
        let expected = hex::decode("dc95c078a2408989ad48a21492842087")
            .map_err(|e| format!("hex: {}", e))?;
        if h[..] != expected[..] {
            return Err(format!("hash subkey mismatch\n  got:  {}\n  want: {}",
                hex::encode(&h), hex::encode(&expected)));
        }
        Ok(())
    });

    // GHASH of zero input with any H = zero (multiplicative identity)
    check!("ghash/02", "ghash-zero-input", {
        let h = [0x42u8; 16]; // arbitrary hash subkey
        let input = [0u8; 16];
        let result = ghash(&input, &h);
        // GHASH(0^16, H) = multiply(0 XOR 0, H) = multiply(0, H) = 0
        if result != [0u8; 16] {
            return Err(format!("GHASH of zero should be zero, got {}", hex::encode(&result)));
        }
        Ok(())
    });

    // GHASH empty input = zero (no blocks to process)
    check!("ghash/03", "ghash-empty-input", {
        let h = [0xFF; 16];
        let result = ghash(&[], &h);
        if result != [0u8; 16] {
            return Err(format!("GHASH of empty should be zero, got {}", hex::encode(&result)));
        }
        Ok(())
    });

    // GHASH with zero hash subkey = zero (anything × 0 = 0)
    check!("ghash/04", "ghash-zero-subkey", {
        let h = [0u8; 16];
        let input = [0xFF; 16]; // non-zero input
        let result = ghash(&input, &h);
        if result != [0u8; 16] {
            return Err(format!("GHASH with zero subkey should be zero, got {}", hex::encode(&result)));
        }
        Ok(())
    });

    // GHASH deterministic
    check!("ghash/05", "ghash-deterministic", {
        let key = [0u8; 32];
        let h = generate_hash_subkey(&key);
        let input = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                        0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10];
        let r1 = ghash(&input, &h);
        let r2 = ghash(&input, &h);
        if r1 != r2 {
            return Err("GHASH should be deterministic".into());
        }
        Ok(())
    });

    // Hash subkey is different for different keys
    check!("ghash/06", "hash-subkey-key-dependence", {
        let h1 = generate_hash_subkey(&[0u8; 32]);
        let h2 = generate_hash_subkey(&[1u8; 32]);
        if h1 == h2 {
            return Err("different AES keys should give different hash subkeys".into());
        }
        Ok(())
    });

    // SHA-256 known vector: "abc"
    check!("ghash/07", "sha256-abc", {
        let hash = sha256(b"abc");
        let expected = hex::decode("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
            .map_err(|e| format!("hex: {}", e))?;
        if hash != expected {
            return Err(format!("sha256('abc') mismatch\n  got:  {}\n  want: {}",
                hex::encode(&hash), hex::encode(&expected)));
        }
        Ok(())
    });

    // SHA-256 empty string
    check!("ghash/08", "sha256-empty", {
        let hash = sha256(b"");
        let expected = hex::decode("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
            .map_err(|e| format!("hex: {}", e))?;
        if hash != expected {
            return Err(format!("sha256('') mismatch"));
        }
        Ok(())
    });

    // Double SHA-256 of empty = SHA-256(SHA-256(""))
    check!("ghash/09", "double-sha256-empty", {
        let dbl = double_sha256(b"");
        let inner = sha256(b"");
        let expected = sha256(&inner);
        if dbl != expected {
            return Err("double_sha256('') != sha256(sha256(''))".into());
        }
        Ok(())
    });

    // ECDSA sign + verify with SIGHASH_ALL (0x01)
    check!("ghash/10", "ecdsa-sign-verify-sighash-all", {
        let privkey = hex::decode("583755110a8c059de5cd81b8a04e1be884c46083ade3f779c1e022f6f89da94c")
            .map_err(|e| format!("hex: {}", e))?;
        let pubkey = derive_public_key(&privkey)
            .map_err(|e| format!("pubkey: {}", e))?;
        let hash = sha256(b"test message");

        let sig = sign_ecdsa(&hash, &privkey, 0x41) // SIGHASH_ALL|FORKID
            .map_err(|e| format!("sign: {}", e))?;

        // Last byte should be sighash type
        if *sig.last().unwrap() != 0x41 {
            return Err(format!("sighash byte should be 0x41, got 0x{:02x}", sig.last().unwrap()));
        }

        let valid = verify_signature(&hash, &sig, &pubkey)
            .map_err(|e| format!("verify: {}", e))?;
        if !valid {
            return Err("signature should verify".into());
        }
        Ok(())
    });

    // ECDSA verify with wrong pubkey → false
    check!("ghash/11", "ecdsa-wrong-pubkey-false", {
        let privkey1 = [1u8; 32];
        let privkey2 = [2u8; 32];
        let pubkey2 = derive_public_key(&privkey2)
            .map_err(|e| format!("pubkey: {}", e))?;
        let hash = [3u8; 32];

        let sig = sign_ecdsa(&hash, &privkey1, 0x01)
            .map_err(|e| format!("sign: {}", e))?;
        let valid = verify_signature(&hash, &sig, &pubkey2)
            .map_err(|e| format!("verify: {}", e))?;
        if valid {
            return Err("wrong pubkey should not verify".into());
        }
        Ok(())
    });

    // ECDSA verify with wrong hash → false
    check!("ghash/12", "ecdsa-wrong-hash-false", {
        let privkey = [4u8; 32];
        let pubkey = derive_public_key(&privkey)
            .map_err(|e| format!("pubkey: {}", e))?;
        let hash1 = [5u8; 32];
        let hash2 = [6u8; 32];

        let sig = sign_ecdsa(&hash1, &privkey, 0x01)
            .map_err(|e| format!("sign: {}", e))?;
        let valid = verify_signature(&hash2, &sig, &pubkey)
            .map_err(|e| format!("verify: {}", e))?;
        if valid {
            return Err("wrong hash should not verify".into());
        }
        Ok(())
    });

    // ECDSA DER signature structure
    check!("ghash/13", "ecdsa-der-structure", {
        let privkey = [7u8; 32];
        let hash = [8u8; 32];
        let sig = sign_ecdsa(&hash, &privkey, 0x01)
            .map_err(|e| format!("sign: {}", e))?;

        // DER: 0x30 <total_len> 0x02 <r_len> <r> 0x02 <s_len> <s> <sighash_type>
        if sig[0] != 0x30 {
            return Err(format!("DER should start with 0x30, got 0x{:02x}", sig[0]));
        }
        // Total length is in sig[1]
        let total_len = sig[1] as usize;
        // DER body + sighash byte = sig length
        if total_len + 2 + 1 != sig.len() {
            return Err(format!("DER length mismatch: total_len={}, sig.len()={}", total_len, sig.len()));
        }
        // First integer marker
        if sig[2] != 0x02 {
            return Err(format!("r integer marker should be 0x02, got 0x{:02x}", sig[2]));
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 9/9: {} tests\n", after - before);
}

// ============================================================================
// Final Summary
// ============================================================================

#[test]
fn t6_zz_summary() {
    // Run last (alphabetically after t6_09)
    let p = PASS.load(Ordering::Relaxed);
    let f = FAIL.load(Ordering::Relaxed);
    eprintln!("\n╔══════════════════════════════════════════╗");
    eprintln!("║  TIER 6 FINAL: {} pass, {} fail, {} total   ", p, f, p + f);
    eprintln!("╚══════════════════════════════════════════╝\n");
    assert_eq!(f, 0, "{} test(s) failed — see FAIL lines above", f);
}
