///
/// TIER 7 — BOUNDARY CONDITIONS & UNTESTED MODULE COVERAGE
///
/// Targets remaining gaps after Tiers 1–6 (702 tests).
/// Focuses on error paths, boundary conditions, and previously untested modules.
///
/// Sections:
///   [1/9] PIN Encryption Edge Cases
///   [2/9] BRC-2 Encryption Error Paths & Symmetry
///   [3/9] DPAPI Platform Stubs
///   [4/9] extract_input_outpoints
///   [5/9] parse_bump_hex_to_tsc Error Paths
///   [6/9] Script Parser Edge Cases
///   [7/9] Status Type Roundtrips (ActionStorage)
///   [8/9] Keys & Uncompressed Pubkey Coverage
///   [9/9] PriceCache & BalanceCache Additional
///
/// Methodology: "collect first, fix later" — wallet code is NEVER modified.
///

use std::sync::atomic::{AtomicUsize, Ordering};

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
// [1/9]  PIN Encryption Edge Cases
// ============================================================================

#[test]
fn t7_01_pin_encryption() {
    use hodos_wallet::crypto::pin::{derive_key_from_pin, encrypt_mnemonic, decrypt_mnemonic};

    eprintln!("\n=== TIER 7 [1/9] PIN Encryption Edge Cases ===\n");

    // derive_key_from_pin is deterministic with fixed salt
    check!("pin/01", "derive-key-deterministic", {
        let salt = [0xAA; 16];
        let k1 = derive_key_from_pin("1234", &salt);
        let k2 = derive_key_from_pin("1234", &salt);
        if k1 != k2 {
            return Err("same PIN + salt should give same key".into());
        }
        if k1.len() != 32 {
            return Err(format!("key should be 32 bytes, got {}", k1.len()));
        }
        Ok(())
    });

    // Different PINs → different keys
    check!("pin/02", "different-pins-different-keys", {
        let salt = [0xBB; 16];
        let k1 = derive_key_from_pin("1234", &salt);
        let k2 = derive_key_from_pin("5678", &salt);
        if k1 == k2 {
            return Err("different PINs should give different keys".into());
        }
        Ok(())
    });

    // Different salts → different keys
    check!("pin/03", "different-salts-different-keys", {
        let s1 = [0x01; 16];
        let s2 = [0x02; 16];
        let k1 = derive_key_from_pin("1234", &s1);
        let k2 = derive_key_from_pin("1234", &s2);
        if k1 == k2 {
            return Err("different salts should give different keys".into());
        }
        Ok(())
    });

    // Encrypt/decrypt roundtrip
    check!("pin/04", "encrypt-decrypt-roundtrip", {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let pin = "9999";
        let (salt, encrypted) = encrypt_mnemonic(mnemonic, pin)
            .map_err(|e| format!("encrypt: {}", e))?;
        let decrypted = decrypt_mnemonic(&encrypted, pin, &salt)
            .map_err(|e| format!("decrypt: {}", e))?;
        if decrypted != mnemonic {
            return Err("decrypted mnemonic doesn't match".into());
        }
        Ok(())
    });

    // Wrong PIN → "Invalid PIN"
    check!("pin/05", "wrong-pin-rejected", {
        let (salt, encrypted) = encrypt_mnemonic("test mnemonic words here", "1234")
            .map_err(|e| format!("encrypt: {}", e))?;
        let result = decrypt_mnemonic(&encrypted, "0000", &salt);
        if result.is_ok() {
            return Err("wrong PIN should fail".into());
        }
        let err = result.unwrap_err();
        if err != "Invalid PIN" {
            return Err(format!("expected 'Invalid PIN', got '{}'", err));
        }
        Ok(())
    });

    // Corrupted ciphertext → error
    check!("pin/06", "corrupted-ciphertext-rejected", {
        let (salt, encrypted) = encrypt_mnemonic("some words here", "1234")
            .map_err(|e| format!("encrypt: {}", e))?;

        // Flip a byte in the middle of the encrypted data
        let mut enc_bytes = hex::decode(&encrypted)
            .map_err(|e| format!("hex: {}", e))?;
        if enc_bytes.len() > 20 {
            enc_bytes[20] ^= 0xFF;
        }
        let corrupted_hex = hex::encode(&enc_bytes);

        let result = decrypt_mnemonic(&corrupted_hex, "1234", &salt);
        if result.is_ok() {
            return Err("corrupted data should fail decrypt".into());
        }
        Ok(())
    });

    // Too-short encrypted data → error
    check!("pin/07", "too-short-encrypted-rejected", {
        let result = decrypt_mnemonic("aabbccdd", "1234", "0011223344556677");
        if result.is_ok() {
            return Err("too-short ciphertext should fail".into());
        }
        Ok(())
    });

    // Empty PIN works (PBKDF2 accepts any length)
    check!("pin/08", "empty-pin-roundtrip", {
        let (salt, encrypted) = encrypt_mnemonic("test data", "")
            .map_err(|e| format!("encrypt: {}", e))?;
        let decrypted = decrypt_mnemonic(&encrypted, "", &salt)
            .map_err(|e| format!("decrypt: {}", e))?;
        if decrypted != "test data" {
            return Err("empty PIN roundtrip failed".into());
        }
        Ok(())
    });

    // Unicode PIN roundtrip
    check!("pin/09", "unicode-pin-roundtrip", {
        let (salt, encrypted) = encrypt_mnemonic("mnemonic here", "🔒1234")
            .map_err(|e| format!("encrypt: {}", e))?;
        let decrypted = decrypt_mnemonic(&encrypted, "🔒1234", &salt)
            .map_err(|e| format!("decrypt: {}", e))?;
        if decrypted != "mnemonic here" {
            return Err("unicode PIN roundtrip failed".into());
        }
        Ok(())
    });

    // Invalid hex in salt → error
    check!("pin/10", "invalid-salt-hex-rejected", {
        let result = decrypt_mnemonic("aabb", "1234", "not-hex!!!");
        if result.is_ok() {
            return Err("invalid salt hex should fail".into());
        }
        Ok(())
    });

    // Invalid hex in encrypted data → error
    check!("pin/11", "invalid-encrypted-hex-rejected", {
        let result = decrypt_mnemonic("not-hex!!!", "1234", "aabb");
        if result.is_ok() {
            return Err("invalid encrypted hex should fail".into());
        }
        Ok(())
    });

    let p = PASS.load(Ordering::Relaxed);
    let f = FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 1/9: {} pass, {} fail\n", p, f);
}

// ============================================================================
// [2/9]  BRC-2 Encryption Error Paths & Symmetry
// ============================================================================

#[test]
fn t7_02_brc2_encryption() {
    use hodos_wallet::crypto::brc2::{
        derive_symmetric_key, encrypt_brc2, decrypt_brc2,
        encrypt_certificate_field, decrypt_certificate_field,
    };
    use hodos_wallet::crypto::keys::derive_public_key;

    eprintln!("\n=== TIER 7 [2/9] BRC-2 Encryption Error Paths ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // derive_symmetric_key symmetry: sender→recipient == recipient→sender
    check!("brc2/01", "derive-symmetric-key-symmetry", {
        let sender_priv = hex::decode("583755110a8c059de5cd81b8a04e1be884c46083ade3f779c1e022f6f89da94c")
            .map_err(|e| format!("hex: {}", e))?;
        let recipient_priv = hex::decode("6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede")
            .map_err(|e| format!("hex: {}", e))?;
        let sender_pub = derive_public_key(&sender_priv)
            .map_err(|e| format!("pubkey: {}", e))?;
        let recipient_pub = derive_public_key(&recipient_priv)
            .map_err(|e| format!("pubkey: {}", e))?;

        let invoice = "2-certificate field encryption-name";

        let k_send = derive_symmetric_key(&sender_priv, &recipient_pub, invoice)
            .map_err(|e| format!("sender: {}", e))?;
        let k_recv = derive_symmetric_key(&recipient_priv, &sender_pub, invoice)
            .map_err(|e| format!("recipient: {}", e))?;

        if k_send != k_recv {
            return Err(format!("keys differ\n  sender:    {}\n  recipient: {}",
                hex::encode(&k_send), hex::encode(&k_recv)));
        }
        Ok(())
    });

    // encrypt_brc2/decrypt_brc2 roundtrip
    check!("brc2/02", "encrypt-decrypt-roundtrip", {
        let key = [0x42u8; 32];
        let plaintext = b"Hello, BRC-2 protocol!";
        let ciphertext = encrypt_brc2(plaintext, &key)
            .map_err(|e| format!("encrypt: {}", e))?;
        let decrypted = decrypt_brc2(&ciphertext, &key)
            .map_err(|e| format!("decrypt: {}", e))?;
        if decrypted != plaintext {
            return Err("roundtrip mismatch".into());
        }
        Ok(())
    });

    // Ciphertext format: [32-byte IV][ciphertext][16-byte tag]
    check!("brc2/03", "ciphertext-format-structure", {
        let key = [0x55u8; 32];
        let plaintext = b"test";
        let ct = encrypt_brc2(plaintext, &key)
            .map_err(|e| format!("encrypt: {}", e))?;
        // Minimum: 32 (IV) + plaintext_len + 16 (tag)
        let expected_min = 32 + plaintext.len() + 16;
        if ct.len() < expected_min {
            return Err(format!("ciphertext too short: {} < {}", ct.len(), expected_min));
        }
        Ok(())
    });

    // Wrong key → decrypt error
    check!("brc2/04", "wrong-key-rejected", {
        let key1 = [0x01u8; 32];
        let key2 = [0x02u8; 32];
        let ct = encrypt_brc2(b"secret", &key1)
            .map_err(|e| format!("encrypt: {}", e))?;
        let result = decrypt_brc2(&ct, &key2);
        if result.is_ok() {
            return Err("wrong key should fail decrypt".into());
        }
        Ok(())
    });

    // Key too short → error
    check!("brc2/05", "key-too-short-rejected", {
        let short_key = [0x01u8; 16]; // only 16 bytes
        let result = encrypt_brc2(b"test", &short_key);
        if result.is_ok() {
            return Err("16-byte key should be rejected".into());
        }
        Ok(())
    });

    // Ciphertext too short → error
    check!("brc2/06", "ciphertext-too-short-rejected", {
        let key = [0x01u8; 32];
        let result = decrypt_brc2(&[0u8; 47], &key); // need >=48
        if result.is_ok() {
            return Err("47-byte ciphertext should be rejected".into());
        }
        Ok(())
    });

    // Empty plaintext roundtrip
    check!("brc2/07", "empty-plaintext-roundtrip", {
        let key = [0x77u8; 32];
        let ct = encrypt_brc2(b"", &key)
            .map_err(|e| format!("encrypt: {}", e))?;
        let pt = decrypt_brc2(&ct, &key)
            .map_err(|e| format!("decrypt: {}", e))?;
        if !pt.is_empty() {
            return Err(format!("expected empty, got {} bytes", pt.len()));
        }
        Ok(())
    });

    // Large plaintext roundtrip
    check!("brc2/08", "large-plaintext-roundtrip", {
        let key = [0x88u8; 32];
        let plaintext = vec![0xAB; 10_000];
        let ct = encrypt_brc2(&plaintext, &key)
            .map_err(|e| format!("encrypt: {}", e))?;
        let pt = decrypt_brc2(&ct, &key)
            .map_err(|e| format!("decrypt: {}", e))?;
        if pt != plaintext {
            return Err("large plaintext roundtrip mismatch".into());
        }
        Ok(())
    });

    // Certificate field encrypt/decrypt roundtrip
    check!("brc2/09", "cert-field-encrypt-decrypt-roundtrip", {
        let sender_priv = [1u8; 32];
        let sender_pub = derive_public_key(&sender_priv)
            .map_err(|e| format!("pubkey: {}", e))?;
        let recipient_priv = [2u8; 32];
        let recipient_pub = derive_public_key(&recipient_priv)
            .map_err(|e| format!("pubkey: {}", e))?;

        let plaintext = b"Alice Smith";

        let ct = encrypt_certificate_field(
            &sender_priv, &recipient_pub, "name", None, plaintext,
        ).map_err(|e| format!("encrypt: {}", e))?;

        let pt = decrypt_certificate_field(
            &recipient_priv, &sender_pub, "name", None, &ct,
        ).map_err(|e| format!("decrypt: {}", e))?;

        if pt != plaintext {
            return Err("cert field roundtrip mismatch".into());
        }
        Ok(())
    });

    // Certificate field with serial number
    check!("brc2/10", "cert-field-with-serial-number", {
        let sender_priv = [3u8; 32];
        let sender_pub = derive_public_key(&sender_priv)
            .map_err(|e| format!("pubkey: {}", e))?;
        let recipient_priv = [4u8; 32];
        let recipient_pub = derive_public_key(&recipient_priv)
            .map_err(|e| format!("pubkey: {}", e))?;

        let serial = "abc123serial";
        let ct = encrypt_certificate_field(
            &sender_priv, &recipient_pub, "email", Some(serial), b"alice@example.com",
        ).map_err(|e| format!("encrypt: {}", e))?;

        let pt = decrypt_certificate_field(
            &recipient_priv, &sender_pub, "email", Some(serial), &ct,
        ).map_err(|e| format!("decrypt: {}", e))?;

        if pt != b"alice@example.com" {
            return Err("cert field+serial roundtrip mismatch".into());
        }
        Ok(())
    });

    // Random IV: two encryptions of same plaintext differ
    check!("brc2/11", "random-iv-nondeterministic", {
        let key = [0x99u8; 32];
        let ct1 = encrypt_brc2(b"same", &key)
            .map_err(|e| format!("enc1: {}", e))?;
        let ct2 = encrypt_brc2(b"same", &key)
            .map_err(|e| format!("enc2: {}", e))?;
        if ct1 == ct2 {
            return Err("two encryptions should differ (random IV)".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 2/9: {} tests\n", after - before);
}

// ============================================================================
// [3/9]  DPAPI Platform Stubs
// ============================================================================

#[test]
fn t7_03_dpapi() {
    use hodos_wallet::crypto::dpapi::{dpapi_encrypt, dpapi_decrypt};

    eprintln!("\n=== TIER 7 [3/9] DPAPI Platform Stubs ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // On non-Windows (Linux/WSL test environment), stubs return error
    #[cfg(not(windows))]
    {
        check!("dpapi/01", "encrypt-stub-errors-on-linux", {
            let result = dpapi_encrypt(b"test data");
            if result.is_ok() {
                return Err("DPAPI encrypt should fail on non-Windows".into());
            }
            let err = result.unwrap_err();
            if !err.contains("Windows") {
                return Err(format!("error should mention Windows, got: {}", err));
            }
            Ok(())
        });

        check!("dpapi/02", "decrypt-stub-errors-on-linux", {
            let result = dpapi_decrypt(b"encrypted data");
            if result.is_ok() {
                return Err("DPAPI decrypt should fail on non-Windows".into());
            }
            Ok(())
        });

        check!("dpapi/03", "encrypt-empty-data-still-errors", {
            let result = dpapi_encrypt(b"");
            if result.is_ok() {
                return Err("DPAPI encrypt of empty should still error on non-Windows".into());
            }
            Ok(())
        });
    }

    // On Windows, test actual DPAPI roundtrip
    #[cfg(windows)]
    {
        check!("dpapi/01", "windows-encrypt-decrypt-roundtrip", {
            let data = b"test mnemonic phrase";
            let encrypted = dpapi_encrypt(data)
                .map_err(|e| format!("encrypt: {}", e))?;
            if encrypted.is_empty() {
                return Err("encrypted should not be empty".into());
            }
            if encrypted == data {
                return Err("encrypted should differ from plaintext".into());
            }
            let decrypted = dpapi_decrypt(&encrypted)
                .map_err(|e| format!("decrypt: {}", e))?;
            if decrypted != data {
                return Err("roundtrip mismatch".into());
            }
            Ok(())
        });

        check!("dpapi/02", "windows-different-data-different-output", {
            let e1 = dpapi_encrypt(b"data1")
                .map_err(|e| format!("enc1: {}", e))?;
            let e2 = dpapi_encrypt(b"data2")
                .map_err(|e| format!("enc2: {}", e))?;
            if e1 == e2 {
                return Err("different data should produce different ciphertext".into());
            }
            Ok(())
        });

        check!("dpapi/03", "windows-garbage-decrypt-fails", {
            // Totally invalid data (not a DPAPI blob) should fail decrypt
            let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03];
            let result = dpapi_decrypt(&garbage);
            if result.is_ok() {
                return Err("garbage data should fail decrypt".into());
            }
            Ok(())
        });
    }

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 3/9: {} tests\n", after - before);
}

// ============================================================================
// [4/9]  extract_input_outpoints
// ============================================================================

#[test]
fn t7_04_extract_input_outpoints() {
    use hodos_wallet::transaction::extract_input_outpoints;
    use hodos_wallet::transaction::types::{Transaction, TxInput, TxOutput, OutPoint};

    eprintln!("\n=== TIER 7 [4/9] extract_input_outpoints ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // Helper: build a transaction and get its hex
    fn tx_to_hex(tx: &Transaction) -> String {
        let bytes = tx.serialize().unwrap();
        hex::encode(bytes)
    }

    // Single input extraction
    check!("extract/01", "single-input", {
        let txid = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let tx = Transaction {
            version: 1,
            inputs: vec![TxInput::new(OutPoint { txid: txid.to_string(), vout: 0 })],
            outputs: vec![TxOutput { value: 1000, script_pubkey: vec![0x76, 0xa9, 0x14,
                0xaa,0xaa,0xaa,0xaa,0xaa,0xaa,0xaa,0xaa,0xaa,0xaa,
                0xaa,0xaa,0xaa,0xaa,0xaa,0xaa,0xaa,0xaa,0xaa,0xaa,
                0x88, 0xac] }],
            lock_time: 0,
        };
        let hex = tx_to_hex(&tx);
        let outpoints = extract_input_outpoints(&hex)
            .map_err(|e| format!("{}", e))?;

        if outpoints.len() != 1 {
            return Err(format!("expected 1 outpoint, got {}", outpoints.len()));
        }
        if outpoints[0].0 != txid {
            return Err(format!("txid mismatch: got {}", outpoints[0].0));
        }
        if outpoints[0].1 != 0 {
            return Err(format!("vout mismatch: got {}", outpoints[0].1));
        }
        Ok(())
    });

    // Multiple inputs
    check!("extract/02", "multiple-inputs", {
        let txid1 = "1111111111111111111111111111111111111111111111111111111111111111";
        let txid2 = "2222222222222222222222222222222222222222222222222222222222222222";
        let txid3 = "3333333333333333333333333333333333333333333333333333333333333333";
        let tx = Transaction {
            version: 1,
            inputs: vec![
                TxInput::new(OutPoint { txid: txid1.to_string(), vout: 0 }),
                TxInput::new(OutPoint { txid: txid2.to_string(), vout: 1 }),
                TxInput::new(OutPoint { txid: txid3.to_string(), vout: 5 }),
            ],
            outputs: vec![TxOutput { value: 1000, script_pubkey: vec![0x6a] }],
            lock_time: 0,
        };
        let hex = tx_to_hex(&tx);
        let outpoints = extract_input_outpoints(&hex)
            .map_err(|e| format!("{}", e))?;

        if outpoints.len() != 3 {
            return Err(format!("expected 3 outpoints, got {}", outpoints.len()));
        }
        if outpoints[0] != (txid1.to_string(), 0) {
            return Err(format!("outpoint 0 mismatch: {:?}", outpoints[0]));
        }
        if outpoints[1] != (txid2.to_string(), 1) {
            return Err(format!("outpoint 1 mismatch: {:?}", outpoints[1]));
        }
        if outpoints[2] != (txid3.to_string(), 5) {
            return Err(format!("outpoint 2 mismatch: {:?}", outpoints[2]));
        }
        Ok(())
    });

    // Invalid hex → error
    check!("extract/03", "invalid-hex-rejected", {
        let result = extract_input_outpoints("not-valid-hex!!!");
        if result.is_ok() {
            return Err("invalid hex should fail".into());
        }
        Ok(())
    });

    // Too short → error
    check!("extract/04", "too-short-rejected", {
        let result = extract_input_outpoints("01000000");  // only 4 bytes
        if result.is_ok() {
            return Err("4-byte tx should fail".into());
        }
        Ok(())
    });

    // Empty string → error
    check!("extract/05", "empty-string-rejected", {
        let result = extract_input_outpoints("");
        if result.is_ok() {
            return Err("empty string should fail".into());
        }
        Ok(())
    });

    // Tx with scriptSig (signed input)
    check!("extract/06", "input-with-scriptsig", {
        let txid = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let mut input = TxInput::new(OutPoint { txid: txid.to_string(), vout: 2 });
        input.script_sig = vec![0x48, 0x30, 0x45]; // fake 3-byte scriptSig
        let tx = Transaction {
            version: 1,
            inputs: vec![input],
            outputs: vec![TxOutput { value: 500, script_pubkey: vec![0x6a, 0x00] }],
            lock_time: 0,
        };
        let hex = tx_to_hex(&tx);
        let outpoints = extract_input_outpoints(&hex)
            .map_err(|e| format!("{}", e))?;

        if outpoints.len() != 1 {
            return Err(format!("expected 1, got {}", outpoints.len()));
        }
        if outpoints[0].0 != txid {
            return Err(format!("txid mismatch: {}", outpoints[0].0));
        }
        if outpoints[0].1 != 2 {
            return Err(format!("vout mismatch: {}", outpoints[0].1));
        }
        Ok(())
    });

    // Tx with high vout
    check!("extract/07", "high-vout-value", {
        let txid = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
        let tx = Transaction {
            version: 1,
            inputs: vec![TxInput::new(OutPoint { txid: txid.to_string(), vout: 4294967295 })],
            outputs: vec![TxOutput { value: 100, script_pubkey: vec![0x6a] }],
            lock_time: 0,
        };
        let hex = tx_to_hex(&tx);
        let outpoints = extract_input_outpoints(&hex)
            .map_err(|e| format!("{}", e))?;

        if outpoints[0].1 != 4294967295 {
            return Err(format!("max vout mismatch: got {}", outpoints[0].1));
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 4/9: {} tests\n", after - before);
}

// ============================================================================
// [5/9]  parse_bump_hex_to_tsc Error Paths
// ============================================================================

#[test]
fn t7_05_parse_bump() {
    use hodos_wallet::beef::parse_bump_hex_to_tsc;

    eprintln!("\n=== TIER 7 [5/9] parse_bump_hex_to_tsc ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // Invalid hex → error
    check!("bump/01", "invalid-hex-rejected", {
        let result = parse_bump_hex_to_tsc("not-valid-hex!!!");
        if result.is_ok() {
            return Err("invalid hex should fail".into());
        }
        Ok(())
    });

    // Empty data → error
    check!("bump/02", "empty-data-rejected", {
        let result = parse_bump_hex_to_tsc("");
        if result.is_ok() {
            return Err("empty data should fail".into());
        }
        Ok(())
    });

    // Single byte (too short for valid BUMP) → error
    check!("bump/03", "single-byte-rejected", {
        let result = parse_bump_hex_to_tsc("00");
        // block_height=0, then tree_height would need next byte
        // This should fail due to being too short or tree_height=0
        if result.is_ok() {
            return Err("single byte should fail parse".into());
        }
        Ok(())
    });

    // Tree height 0 → error
    check!("bump/04", "tree-height-zero-rejected", {
        // block_height=100 (varint 0x64), tree_height=0
        let result = parse_bump_hex_to_tsc("6400");
        if result.is_ok() {
            return Err("tree height 0 should fail".into());
        }
        let err = result.unwrap_err();
        if !err.contains("tree height") {
            return Err(format!("error should mention tree height, got: {}", err));
        }
        Ok(())
    });

    // Truncated BUMP (block height + tree height but no level data) → error
    check!("bump/05", "truncated-bump-rejected", {
        // block_height=100 (0x64), tree_height=5
        let result = parse_bump_hex_to_tsc("6405");
        if result.is_ok() {
            return Err("truncated BUMP should fail".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 5/9: {} tests\n", after - before);
}

// ============================================================================
// [6/9]  Script Parser Edge Cases
// ============================================================================

#[test]
fn t7_06_script_parser() {
    use hodos_wallet::script::parser::{parse_script_chunks, opcodes};

    eprintln!("\n=== TIER 7 [6/9] Script Parser Edge Cases ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // Empty script → empty chunks
    check!("script/01", "empty-script", {
        let chunks = parse_script_chunks(&[])
            .map_err(|e| format!("{}", e))?;
        if !chunks.is_empty() {
            return Err(format!("expected 0 chunks, got {}", chunks.len()));
        }
        Ok(())
    });

    // P2PKH script: OP_DUP OP_HASH160 PUSH20 <hash> OP_EQUALVERIFY OP_CHECKSIG
    check!("script/02", "p2pkh-script-5-chunks", {
        let mut script = vec![0x76, 0xa9, 0x14]; // OP_DUP, OP_HASH160, PUSH20
        script.extend_from_slice(&[0xaa; 20]); // 20-byte hash
        script.extend_from_slice(&[0x88, 0xac]); // OP_EQUALVERIFY, OP_CHECKSIG

        let chunks = parse_script_chunks(&script)
            .map_err(|e| format!("{}", e))?;
        if chunks.len() != 5 {
            return Err(format!("P2PKH should have 5 chunks, got {}", chunks.len()));
        }
        // chunk[0] = OP_DUP (0x76, no data)
        if chunks[0].op != 0x76 || chunks[0].data.is_some() {
            return Err("chunk 0 should be OP_DUP".into());
        }
        // chunk[1] = OP_HASH160 (0xa9, no data)
        if chunks[1].op != 0xa9 || chunks[1].data.is_some() {
            return Err("chunk 1 should be OP_HASH160".into());
        }
        // chunk[2] = PUSH20 + 20 bytes data
        if chunks[2].op != 0x14 {
            return Err(format!("chunk 2 op should be 0x14, got 0x{:02x}", chunks[2].op));
        }
        if chunks[2].data.as_ref().map(|d| d.len()) != Some(20) {
            return Err("chunk 2 should have 20 bytes data".into());
        }
        Ok(())
    });

    // OP_0 (0x00) is parsed as opcode with no data
    check!("script/03", "op-0-no-data", {
        let chunks = parse_script_chunks(&[0x00])
            .map_err(|e| format!("{}", e))?;
        if chunks.len() != 1 {
            return Err(format!("expected 1 chunk, got {}", chunks.len()));
        }
        if chunks[0].op != 0x00 || chunks[0].data.is_some() {
            return Err("OP_0 should have no data".into());
        }
        Ok(())
    });

    // OP_PUSHDATA1 with 100 bytes
    check!("script/04", "pushdata1-100-bytes", {
        let mut script = vec![opcodes::OP_PUSHDATA1, 100];
        script.extend(vec![0x42; 100]);
        let chunks = parse_script_chunks(&script)
            .map_err(|e| format!("{}", e))?;
        if chunks.len() != 1 {
            return Err(format!("expected 1 chunk, got {}", chunks.len()));
        }
        if chunks[0].op != opcodes::OP_PUSHDATA1 {
            return Err("op should be OP_PUSHDATA1".into());
        }
        if chunks[0].data.as_ref().map(|d| d.len()) != Some(100) {
            return Err("data should be 100 bytes".into());
        }
        Ok(())
    });

    // OP_PUSHDATA2 with 300 bytes
    check!("script/05", "pushdata2-300-bytes", {
        let mut script = vec![opcodes::OP_PUSHDATA2];
        script.extend_from_slice(&(300u16).to_le_bytes());
        script.extend(vec![0x55; 300]);
        let chunks = parse_script_chunks(&script)
            .map_err(|e| format!("{}", e))?;
        if chunks.len() != 1 || chunks[0].data.as_ref().map(|d| d.len()) != Some(300) {
            return Err("PUSHDATA2 300-byte parse failed".into());
        }
        Ok(())
    });

    // OP_PUSHDATA4 with 500 bytes
    check!("script/06", "pushdata4-500-bytes", {
        let mut script = vec![opcodes::OP_PUSHDATA4];
        script.extend_from_slice(&(500u32).to_le_bytes());
        script.extend(vec![0x66; 500]);
        let chunks = parse_script_chunks(&script)
            .map_err(|e| format!("{}", e))?;
        if chunks.len() != 1 || chunks[0].data.as_ref().map(|d| d.len()) != Some(500) {
            return Err("PUSHDATA4 500-byte parse failed".into());
        }
        Ok(())
    });

    // Truncated push data → UnexpectedEndOfScript
    check!("script/07", "truncated-push-error", {
        // Push 20 bytes but only provide 10
        let mut script = vec![0x14]; // push 20 bytes
        script.extend(vec![0xaa; 10]); // only 10
        let result = parse_script_chunks(&script);
        if result.is_ok() {
            return Err("truncated push should fail".into());
        }
        Ok(())
    });

    // Truncated OP_PUSHDATA1 → error
    check!("script/08", "truncated-pushdata1-error", {
        // OP_PUSHDATA1 + length=50, but no data follows
        let script = vec![opcodes::OP_PUSHDATA1, 50];
        let result = parse_script_chunks(&script);
        if result.is_ok() {
            return Err("truncated PUSHDATA1 should fail".into());
        }
        Ok(())
    });

    // Multiple opcodes in sequence
    check!("script/09", "multiple-opcodes-sequence", {
        let script = vec![
            opcodes::OP_1,      // OP_1 (0x51)
            opcodes::OP_2,      // OP_2 (0x52)
            opcodes::OP_DROP,   // OP_DROP (0x75)
            opcodes::OP_CHECKSIG, // OP_CHECKSIG (0xac)
        ];
        let chunks = parse_script_chunks(&script)
            .map_err(|e| format!("{}", e))?;
        if chunks.len() != 4 {
            return Err(format!("expected 4 chunks, got {}", chunks.len()));
        }
        // All should have no data (they're opcodes, not push ops)
        for (i, chunk) in chunks.iter().enumerate() {
            if chunk.data.is_some() {
                return Err(format!("chunk {} should have no data", i));
            }
        }
        Ok(())
    });

    // Direct push: 1-byte push
    check!("script/10", "direct-push-1-byte", {
        let script = vec![0x01, 0xFF]; // push 1 byte: 0xFF
        let chunks = parse_script_chunks(&script)
            .map_err(|e| format!("{}", e))?;
        if chunks.len() != 1 {
            return Err(format!("expected 1 chunk, got {}", chunks.len()));
        }
        if chunks[0].data != Some(vec![0xFF]) {
            return Err("data should be [0xFF]".into());
        }
        Ok(())
    });

    // Direct push: 75-byte push (max direct)
    check!("script/11", "direct-push-75-bytes", {
        let mut script = vec![75]; // push 75 bytes
        script.extend(vec![0x42; 75]);
        let chunks = parse_script_chunks(&script)
            .map_err(|e| format!("{}", e))?;
        if chunks.len() != 1 || chunks[0].data.as_ref().map(|d| d.len()) != Some(75) {
            return Err("75-byte direct push failed".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 6/9: {} tests\n", after - before);
}

// ============================================================================
// [7/9]  Status Type Roundtrips (ActionStorage)
// ============================================================================

#[test]
fn t7_07_status_types() {
    use hodos_wallet::action_storage::{
        TransactionStatus, ActionStatus, ProvenTxReqStatus,
    };

    eprintln!("\n=== TIER 7 [7/9] Status Type Roundtrips ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // TransactionStatus roundtrip: all variants
    check!("status/01", "tx-status-roundtrip-all-variants", {
        let variants = vec![
            ("completed", TransactionStatus::Completed),
            ("unprocessed", TransactionStatus::Unprocessed),
            ("sending", TransactionStatus::Sending),
            ("unproven", TransactionStatus::Unproven),
            ("unsigned", TransactionStatus::Unsigned),
            ("nosend", TransactionStatus::Nosend),
            ("nonfinal", TransactionStatus::Nonfinal),
            ("failed", TransactionStatus::Failed),
        ];
        for (name, variant) in &variants {
            let s = variant.as_str();
            if s != *name {
                return Err(format!("{:?}.as_str() = '{}', expected '{}'", variant, s, name));
            }
            let parsed = TransactionStatus::from_str(s);
            if parsed != *variant {
                return Err(format!("from_str('{}') != {:?}", s, variant));
            }
        }
        Ok(())
    });

    // Unknown string → Unprocessed (safe default)
    check!("status/02", "tx-status-unknown-default", {
        let status = TransactionStatus::from_str("garbage");
        if status != TransactionStatus::Unprocessed {
            return Err(format!("unknown string should default to Unprocessed, got {:?}", status));
        }
        Ok(())
    });

    // ActionStatus to_string roundtrip
    check!("status/03", "action-status-to-string", {
        let variants: Vec<(ActionStatus, &str)> = vec![
            (ActionStatus::Created, "created"),
            (ActionStatus::Signed, "signed"),
            (ActionStatus::Unconfirmed, "unconfirmed"),
            (ActionStatus::Pending, "pending"),
            (ActionStatus::Confirmed, "confirmed"),
            (ActionStatus::Aborted, "aborted"),
            (ActionStatus::Failed, "failed"),
        ];
        for (variant, expected) in &variants {
            let s = variant.to_string();
            if s != *expected {
                return Err(format!("{:?}.to_string() = '{}', expected '{}'", variant, s, expected));
            }
        }
        Ok(())
    });

    // TransactionStatus → ActionStatus conversion
    check!("status/04", "tx-to-action-status-conversion", {
        let mappings: Vec<(TransactionStatus, ActionStatus)> = vec![
            (TransactionStatus::Completed, ActionStatus::Confirmed),
            (TransactionStatus::Unprocessed, ActionStatus::Created),
            (TransactionStatus::Sending, ActionStatus::Signed),
            (TransactionStatus::Unproven, ActionStatus::Unconfirmed),
            (TransactionStatus::Unsigned, ActionStatus::Created),
            (TransactionStatus::Nosend, ActionStatus::Aborted),
            (TransactionStatus::Nonfinal, ActionStatus::Created),
            (TransactionStatus::Failed, ActionStatus::Failed),
        ];
        for (tx_status, expected_action) in &mappings {
            let action = tx_status.to_action_status();
            if action != *expected_action {
                return Err(format!("{:?}.to_action_status() = {:?}, expected {:?}",
                    tx_status, action, expected_action));
            }
        }
        Ok(())
    });

    // from_legacy conversion
    check!("status/05", "from-legacy-conversion", {
        let result = TransactionStatus::from_legacy(&ActionStatus::Created, None);
        if result != TransactionStatus::Unsigned {
            return Err(format!("Created/None → {:?}, expected Unsigned", result));
        }

        let result = TransactionStatus::from_legacy(&ActionStatus::Confirmed, Some("confirmed"));
        if result != TransactionStatus::Completed {
            return Err(format!("Confirmed/confirmed → {:?}, expected Completed", result));
        }

        let result = TransactionStatus::from_legacy(&ActionStatus::Failed, Some("failed"));
        if result != TransactionStatus::Failed {
            return Err(format!("Failed/failed → {:?}, expected Failed", result));
        }

        let result = TransactionStatus::from_legacy(&ActionStatus::Aborted, None);
        if result != TransactionStatus::Nosend {
            return Err(format!("Aborted/None → {:?}, expected Nosend", result));
        }

        let result = TransactionStatus::from_legacy(&ActionStatus::Pending, Some("confirmed"));
        if result != TransactionStatus::Completed {
            return Err(format!("Pending/confirmed → {:?}, expected Completed", result));
        }

        let result = TransactionStatus::from_legacy(&ActionStatus::Pending, Some("broadcast"));
        if result != TransactionStatus::Unproven {
            return Err(format!("Pending/broadcast → {:?}, expected Unproven", result));
        }
        Ok(())
    });

    // ProvenTxReqStatus roundtrip
    check!("status/06", "proven-tx-req-status-roundtrip", {
        let variants = vec![
            ("unknown", ProvenTxReqStatus::Unknown),
            ("sending", ProvenTxReqStatus::Sending),
            ("unsent", ProvenTxReqStatus::Unsent),
            ("nosend", ProvenTxReqStatus::Nosend),
            ("unproven", ProvenTxReqStatus::Unproven),
            ("invalid", ProvenTxReqStatus::Invalid),
            ("unmined", ProvenTxReqStatus::Unmined),
            ("callback", ProvenTxReqStatus::Callback),
            ("completed", ProvenTxReqStatus::Completed),
        ];
        for (name, variant) in &variants {
            let s = variant.as_str();
            if s != *name {
                return Err(format!("{:?}.as_str() = '{}', expected '{}'", variant, s, name));
            }
            let parsed = ProvenTxReqStatus::from_str(s);
            if parsed != *variant {
                return Err(format!("from_str('{}') != {:?}", s, variant));
            }
        }
        Ok(())
    });

    // ProvenTxReqStatus unknown default
    check!("status/07", "proven-tx-req-unknown-default", {
        let status = ProvenTxReqStatus::from_str("nonsense");
        if status != ProvenTxReqStatus::Unknown {
            return Err(format!("unknown string should default to Unknown, got {:?}", status));
        }
        Ok(())
    });

    // ProvenTxReqStatus is_terminal
    check!("status/08", "proven-tx-req-is-terminal", {
        let terminal = vec![ProvenTxReqStatus::Completed, ProvenTxReqStatus::Invalid];
        let non_terminal = vec![
            ProvenTxReqStatus::Unknown, ProvenTxReqStatus::Sending,
            ProvenTxReqStatus::Unsent, ProvenTxReqStatus::Nosend,
            ProvenTxReqStatus::Unproven, ProvenTxReqStatus::Unmined,
            ProvenTxReqStatus::Callback,
        ];
        for s in &terminal {
            if !s.is_terminal() {
                return Err(format!("{:?} should be terminal", s));
            }
        }
        for s in &non_terminal {
            if s.is_terminal() {
                return Err(format!("{:?} should NOT be terminal", s));
            }
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 7/9: {} tests\n", after - before);
}

// ============================================================================
// [8/9]  Keys & Uncompressed Pubkey Coverage
// ============================================================================

#[test]
fn t7_08_keys_coverage() {
    use hodos_wallet::crypto::keys::{derive_public_key, derive_public_key_uncompressed};

    eprintln!("\n=== TIER 7 [8/9] Keys & Uncompressed Pubkey ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // Uncompressed key is 65 bytes with 0x04 prefix
    check!("keys/01", "uncompressed-key-format", {
        let priv_key = [1u8; 32];
        let unc = derive_public_key_uncompressed(&priv_key)
            .map_err(|e| format!("{}", e))?;
        if unc.len() != 65 {
            return Err(format!("expected 65 bytes, got {}", unc.len()));
        }
        if unc[0] != 0x04 {
            return Err(format!("prefix should be 0x04, got 0x{:02x}", unc[0]));
        }
        Ok(())
    });

    // Compressed and uncompressed share the same x-coordinate
    check!("keys/02", "compressed-uncompressed-same-x", {
        let priv_key = [7u8; 32];
        let comp = derive_public_key(&priv_key)
            .map_err(|e| format!("{}", e))?;
        let unc = derive_public_key_uncompressed(&priv_key)
            .map_err(|e| format!("{}", e))?;

        // Compressed: [prefix][x(32)] ; Uncompressed: [0x04][x(32)][y(32)]
        let comp_x = &comp[1..33];
        let unc_x = &unc[1..33];
        if comp_x != unc_x {
            return Err("x-coordinates should match".into());
        }
        Ok(())
    });

    // Invalid key length → error
    check!("keys/03", "invalid-key-length-rejected", {
        let result = derive_public_key(&[1u8; 31]);
        if result.is_ok() {
            return Err("31-byte key should be rejected".into());
        }
        let result = derive_public_key_uncompressed(&[1u8; 33]);
        if result.is_ok() {
            return Err("33-byte key should be rejected".into());
        }
        Ok(())
    });

    // Zero private key is invalid for secp256k1
    check!("keys/04", "zero-private-key-rejected", {
        let result = derive_public_key(&[0u8; 32]);
        if result.is_ok() {
            return Err("zero private key should be rejected by secp256k1".into());
        }
        Ok(())
    });

    // Known key pair: private key 1 → known public key
    check!("keys/05", "known-key-pair", {
        // Private key = 1 (the generator point G)
        let mut priv_key = [0u8; 32];
        priv_key[31] = 1;
        let pub_key = derive_public_key(&priv_key)
            .map_err(|e| format!("{}", e))?;
        // G = 0279BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798
        let expected = hex::decode("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .map_err(|e| format!("hex: {}", e))?;
        if pub_key != expected {
            return Err(format!("G point mismatch\n  got:  {}\n  want: {}",
                hex::encode(&pub_key), hex::encode(&expected)));
        }
        Ok(())
    });

    // Uncompressed version of generator point
    check!("keys/06", "known-uncompressed-g-point", {
        let mut priv_key = [0u8; 32];
        priv_key[31] = 1;
        let unc = derive_public_key_uncompressed(&priv_key)
            .map_err(|e| format!("{}", e))?;
        // Uncompressed G starts with 04, then x, then y
        if unc[0] != 0x04 {
            return Err("prefix should be 0x04".into());
        }
        // x-coordinate should match compressed version
        let expected_x = hex::decode("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .map_err(|e| format!("hex: {}", e))?;
        if unc[1..33] != expected_x[..] {
            return Err("x-coordinate of G mismatch".into());
        }
        Ok(())
    });

    // Different keys → different public keys (regression)
    check!("keys/07", "different-keys-different-pubkeys", {
        let pk1 = derive_public_key(&[1u8; 32]).map_err(|e| format!("{}", e))?;
        let pk2 = derive_public_key(&[2u8; 32]).map_err(|e| format!("{}", e))?;
        let pk3 = derive_public_key(&[3u8; 32]).map_err(|e| format!("{}", e))?;
        if pk1 == pk2 || pk2 == pk3 || pk1 == pk3 {
            return Err("all three pubkeys should differ".into());
        }
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 8/9: {} tests\n", after - before);
}

// ============================================================================
// [9/9]  PriceCache & BalanceCache Additional
// ============================================================================

#[test]
fn t7_09_caches() {
    use hodos_wallet::price_cache::PriceCache;
    use hodos_wallet::balance_cache::BalanceCache;

    eprintln!("\n=== TIER 7 [9/9] PriceCache & BalanceCache ===\n");
    let before = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);

    // PriceCache: new → get_cached returns None
    check!("cache/01", "price-cache-new-is-empty", {
        let cache = PriceCache::new();
        let price = cache.get_cached();
        if price.is_some() {
            return Err(format!("new cache should be empty, got {:?}", price));
        }
        Ok(())
    });

    // PriceCache: get_stale also None on new
    check!("cache/02", "price-cache-stale-none-on-new", {
        let cache = PriceCache::new();
        let stale = cache.get_stale();
        if stale.is_some() {
            return Err(format!("new cache stale should be None, got {:?}", stale));
        }
        Ok(())
    });

    // BalanceCache: new → get returns None
    check!("cache/03", "balance-cache-new-is-none", {
        let cache = BalanceCache::new();
        let balance = cache.get();
        if balance.is_some() {
            return Err(format!("new balance cache should be None, got {:?}", balance));
        }
        Ok(())
    });

    // BalanceCache: set → get returns value
    check!("cache/04", "balance-cache-set-get", {
        let cache = BalanceCache::new();
        cache.set(12345);
        let balance = cache.get();
        if balance != Some(12345) {
            return Err(format!("expected Some(12345), got {:?}", balance));
        }
        Ok(())
    });

    // BalanceCache: invalidate → get returns None
    check!("cache/05", "balance-cache-invalidate", {
        let cache = BalanceCache::new();
        cache.set(99999);
        cache.invalidate();
        let balance = cache.get();
        if balance.is_some() {
            return Err(format!("invalidated cache should be None, got {:?}", balance));
        }
        Ok(())
    });

    // BalanceCache: get_or_stale returns stale value after invalidation within TTL
    check!("cache/06", "balance-cache-get-or-stale", {
        let cache = BalanceCache::new();
        cache.set(55555);
        // get_or_stale should return the value even if we just set it
        let val = cache.get_or_stale();
        if val != Some(55555) {
            return Err(format!("expected Some(55555), got {:?}", val));
        }
        Ok(())
    });

    // BalanceCache: update overwrites
    check!("cache/07", "balance-cache-update-overwrites", {
        let cache = BalanceCache::new();
        cache.set(100);
        cache.update(200);
        let balance = cache.get();
        if balance != Some(200) {
            return Err(format!("expected Some(200), got {:?}", balance));
        }
        Ok(())
    });

    // BalanceCache: thread-safe access
    check!("cache/08", "balance-cache-thread-safe", {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(BalanceCache::new());
        let mut handles = vec![];

        // 10 threads set different values
        for i in 0..10 {
            let c = Arc::clone(&cache);
            handles.push(thread::spawn(move || {
                c.set(i * 1000);
                c.get(); // read
                c.invalidate();
                c.set((i + 1) * 1000);
            }));
        }

        for h in handles {
            h.join().map_err(|_| "thread panicked".to_string())?;
        }

        // Cache should have some valid state (no panics or data races)
        // The exact value depends on thread scheduling
        let _ = cache.get(); // just verify no panic
        Ok(())
    });

    let after = PASS.load(Ordering::Relaxed) + FAIL.load(Ordering::Relaxed);
    eprintln!("\n  section 9/9: {} tests\n", after - before);
}

// ============================================================================
// Final Summary
// ============================================================================

#[test]
fn t7_zz_summary() {
    let p = PASS.load(Ordering::Relaxed);
    let f = FAIL.load(Ordering::Relaxed);
    eprintln!("\n╔══════════════════════════════════════════╗");
    eprintln!("║  TIER 7 FINAL: {} pass, {} fail, {} total   ", p, f, p + f);
    eprintln!("╚══════════════════════════════════════════╝\n");
    assert_eq!(f, 0, "{} test(s) failed — see FAIL lines above", f);
}
