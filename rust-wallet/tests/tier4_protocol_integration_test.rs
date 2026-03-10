//! Tier 4: Protocol Integration & Cross-Module Diagnostic Tests
//!
//! Validates cross-module interactions and protocol-level correctness:
//! - BRC-42 ECDH key derivation (spec vectors + cross-key consistency)
//! - BRC-43 invoice number formatting and normalization
//! - BRC-2 end-to-end encryption (symmetric key derivation → encrypt → decrypt)
//! - PIN-based mnemonic encryption (PBKDF2 + AES-256-GCM)
//! - Transaction build → sign → verify workflow
//! - Cross-module: BRC-42 → BRC-2 → certificate field roundtrip
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
fn tier4_diagnostic_suite() {
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║   Tier 4: Protocol Integration & Cross-Module Tests     ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    section_1_brc42_spec_vectors();
    section_2_brc42_cross_key_consistency();
    section_3_brc43_invoice_numbers();
    section_4_brc2_end_to_end();
    section_5_pin_encryption();
    section_6_transaction_workflow();
    section_7_cross_module_integration();

    let p = PASS_COUNT.load(Ordering::Relaxed);
    let f = FAIL_COUNT.load(Ordering::Relaxed);
    println!("\n══════════════════════════════════════════════════════════");
    println!("  TOTAL: {} passed, {} failed, {} total", p, f, p + f);
    println!("══════════════════════════════════════════════════════════\n");

    assert_eq!(f, 0, "{} test(s) failed — see [FAIL] lines above", f);
}

// ============================================================================
// [1/7] BRC-42 Spec Test Vectors
// ============================================================================

fn section_1_brc42_spec_vectors() {
    use hodos_wallet::crypto::brc42::{
        compute_shared_secret, derive_child_public_key, derive_child_private_key,
    };

    println!("  [1/7] BRC-42 Spec Test Vectors");

    // 1. Private key derivation — BRC-42 vector 1
    check!("brc42", "1 privkey-vector-1", {
        let sender_pubkey = hex::decode(
            "033f9160df035156f1c48e75eae99914fa1a1546bec19781e8eddb900200bff9d1"
        ).unwrap();
        let recipient_privkey = hex::decode(
            "6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede"
        ).unwrap();
        let invoice = "f3WCaUmnN9U=";
        let expected = hex::decode(
            "761656715bbfa172f8f9f58f5af95d9d0dfd69014cfdcacc9a245a10ff8893ef"
        ).unwrap();
        let derived = derive_child_private_key(&recipient_privkey, &sender_pubkey, invoice)
            .map_err(|e| e.to_string())?;
        if derived != expected {
            return Err(format!("mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&derived), hex::encode(&expected)));
        }
        Ok(())
    });

    // 2. Private key derivation — BRC-42 vector 2
    check!("brc42", "2 privkey-vector-2", {
        let sender_pubkey = hex::decode(
            "027775fa43959548497eb510541ac34b01d5ee9ea768de74244a4a25f7b60fae8d"
        ).unwrap();
        let recipient_privkey = hex::decode(
            "cab2500e206f31bc18a8af9d6f44f0b9a208c32d5cca2b22acfe9d1a213b2f36"
        ).unwrap();
        let invoice = "2Ska++APzEc=";
        let expected = hex::decode(
            "09f2b48bd75f4da6429ac70b5dce863d5ed2b350b6f2119af5626914bdb7c276"
        ).unwrap();
        let derived = derive_child_private_key(&recipient_privkey, &sender_pubkey, invoice)
            .map_err(|e| e.to_string())?;
        if derived != expected {
            return Err(format!("mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&derived), hex::encode(&expected)));
        }
        Ok(())
    });

    // 3. Public key derivation — BRC-42 vector 1
    check!("brc42", "3 pubkey-vector-1", {
        let sender_privkey = hex::decode(
            "583755110a8c059de5cd81b8a04e1be884c46083ade3f779c1e022f6f89da94c"
        ).unwrap();
        let recipient_pubkey = hex::decode(
            "02c0c1e1a1f7d247827d1bcf399f0ef2deef7695c322fd91a01a91378f101b6ffc"
        ).unwrap();
        let invoice = "IBioA4D/OaE=";
        let expected = hex::decode(
            "03c1bf5baadee39721ae8c9882b3cf324f0bf3b9eb3fc1b8af8089ca7a7c2e669f"
        ).unwrap();
        let derived = derive_child_public_key(&sender_privkey, &recipient_pubkey, invoice)
            .map_err(|e| e.to_string())?;
        if derived != expected {
            return Err(format!("mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&derived), hex::encode(&expected)));
        }
        Ok(())
    });

    // 4. Public key derivation — BRC-42 vector 2
    check!("brc42", "4 pubkey-vector-2", {
        let sender_privkey = hex::decode(
            "2c378b43d887d72200639890c11d79e8f22728d032a5733ba3d7be623d1bb118"
        ).unwrap();
        let recipient_pubkey = hex::decode(
            "039a9da906ecb8ced5c87971e9c2e7c921e66ad450fd4fc0a7d569fdb5bede8e0f"
        ).unwrap();
        let invoice = "PWYuo9PDKvI=";
        let expected = hex::decode(
            "0398cdf4b56a3b2e106224ff3be5253afd5b72de735d647831be51c713c9077848"
        ).unwrap();
        let derived = derive_child_public_key(&sender_privkey, &recipient_pubkey, invoice)
            .map_err(|e| e.to_string())?;
        if derived != expected {
            return Err(format!("mismatch:\n  got: {}\n  exp: {}",
                hex::encode(&derived), hex::encode(&expected)));
        }
        Ok(())
    });

    // 5. Shared secret symmetry (Alice privkey * Bob pubkey == Bob privkey * Alice pubkey)
    check!("brc42", "5 shared-secret-symmetry", {
        let alice_priv = [0x11u8; 32];
        let bob_priv = [0x22u8; 32];
        let alice_pub = hodos_wallet::crypto::keys::derive_public_key(&alice_priv)
            .map_err(|e| e.to_string())?;
        let bob_pub = hodos_wallet::crypto::keys::derive_public_key(&bob_priv)
            .map_err(|e| e.to_string())?;
        let ss_ab = compute_shared_secret(&alice_priv, &bob_pub).map_err(|e| e.to_string())?;
        let ss_ba = compute_shared_secret(&bob_priv, &alice_pub).map_err(|e| e.to_string())?;
        if ss_ab != ss_ba {
            return Err("ECDH shared secret is not symmetric".to_string());
        }
        Ok(())
    });

    // 6. "Anyone" key (privkey=1) shared secret
    check!("brc42", "6 anyone-key-shared-secret", {
        let anyone_priv = {
            let mut k = [0u8; 32];
            k[31] = 1;
            k
        };
        let test_priv = [0x42u8; 32];
        let test_pub = hodos_wallet::crypto::keys::derive_public_key(&test_priv)
            .map_err(|e| e.to_string())?;
        let anyone_pub = hodos_wallet::crypto::keys::derive_public_key(&anyone_priv)
            .map_err(|e| e.to_string())?;
        // Verify symmetry with "anyone"
        let ss1 = compute_shared_secret(&anyone_priv, &test_pub).map_err(|e| e.to_string())?;
        let ss2 = compute_shared_secret(&test_priv, &anyone_pub).map_err(|e| e.to_string())?;
        if ss1 != ss2 {
            return Err("anyone key shared secret not symmetric".to_string());
        }
        if ss1.len() != 33 {
            return Err(format!("shared secret should be 33 bytes, got {}", ss1.len()));
        }
        Ok(())
    });
}

// ============================================================================
// [2/7] BRC-42 Cross-Key Consistency
// ============================================================================

fn section_2_brc42_cross_key_consistency() {
    use hodos_wallet::crypto::brc42::{derive_child_public_key, derive_child_private_key};
    use hodos_wallet::crypto::keys::derive_public_key;

    println!("  [2/7] BRC-42 Cross-Key Consistency");

    // 1. Core BRC-42 property: derive_child_pubkey(sender) == pubkey(derive_child_privkey(recipient))
    check!("brc42-xkey", "1 pubkey-privkey-consistency", {
        let sender_priv = [0x11u8; 32];
        let recipient_priv = [0x22u8; 32];
        let sender_pub = derive_public_key(&sender_priv).map_err(|e| e.to_string())?;
        let recipient_pub = derive_public_key(&recipient_priv).map_err(|e| e.to_string())?;
        let invoice = "2-test derivation-key1";

        // Sender derives child PUBLIC key for recipient
        let child_pubkey = derive_child_public_key(&sender_priv, &recipient_pub, invoice)
            .map_err(|e| e.to_string())?;

        // Recipient derives child PRIVATE key
        let child_privkey = derive_child_private_key(&recipient_priv, &sender_pub, invoice)
            .map_err(|e| e.to_string())?;

        // Derive public key from child private key
        let derived_pubkey = derive_public_key(&child_privkey).map_err(|e| e.to_string())?;

        if child_pubkey != derived_pubkey {
            return Err(format!(
                "BRC-42 consistency violated:\n  child_pubkey:   {}\n  derived_pubkey: {}",
                hex::encode(&child_pubkey), hex::encode(&derived_pubkey)
            ));
        }
        Ok(())
    });

    // 2. Self-derivation consistency (sender privkey/pubkey as both sides)
    check!("brc42-xkey", "2 self-derivation-consistency", {
        let my_priv = [0x33u8; 32];
        let my_pub = derive_public_key(&my_priv).map_err(|e| e.to_string())?;
        let invoice = "2-receive address-0";

        let child_pubkey = derive_child_public_key(&my_priv, &my_pub, invoice)
            .map_err(|e| e.to_string())?;
        let child_privkey = derive_child_private_key(&my_priv, &my_pub, invoice)
            .map_err(|e| e.to_string())?;
        let derived_pubkey = derive_public_key(&child_privkey).map_err(|e| e.to_string())?;

        if child_pubkey != derived_pubkey {
            return Err("self-derivation: pubkey/privkey mismatch".to_string());
        }
        Ok(())
    });

    // 3. Different invoice numbers produce different child keys
    check!("brc42-xkey", "3 different-invoices-differ", {
        let sender_priv = [0x44u8; 32];
        let recipient_pub = derive_public_key(&[0x55u8; 32]).map_err(|e| e.to_string())?;
        let pk1 = derive_child_public_key(&sender_priv, &recipient_pub, "2-test derivation-key1")
            .map_err(|e| e.to_string())?;
        let pk2 = derive_child_public_key(&sender_priv, &recipient_pub, "2-test derivation-key2")
            .map_err(|e| e.to_string())?;
        if pk1 == pk2 {
            return Err("different invoices produced same child key".to_string());
        }
        Ok(())
    });

    // 4. Different counterparties produce different child keys
    check!("brc42-xkey", "4 different-counterparties-differ", {
        let sender_priv = [0x66u8; 32];
        let recipient1_pub = derive_public_key(&[0x77u8; 32]).map_err(|e| e.to_string())?;
        let recipient2_pub = derive_public_key(&[0x88u8; 32]).map_err(|e| e.to_string())?;
        let invoice = "2-test derivation-key1";
        let pk1 = derive_child_public_key(&sender_priv, &recipient1_pub, invoice)
            .map_err(|e| e.to_string())?;
        let pk2 = derive_child_public_key(&sender_priv, &recipient2_pub, invoice)
            .map_err(|e| e.to_string())?;
        if pk1 == pk2 {
            return Err("different counterparties produced same child key".to_string());
        }
        Ok(())
    });

    // 5. Determinism
    check!("brc42-xkey", "5 determinism", {
        let sender_priv = [0x99u8; 32];
        let recipient_pub = derive_public_key(&[0xAAu8; 32]).map_err(|e| e.to_string())?;
        let invoice = "2-test derivation-key1";
        let pk1 = derive_child_public_key(&sender_priv, &recipient_pub, invoice)
            .map_err(|e| e.to_string())?;
        let pk2 = derive_child_public_key(&sender_priv, &recipient_pub, invoice)
            .map_err(|e| e.to_string())?;
        if pk1 != pk2 {
            return Err("non-deterministic child key derivation".to_string());
        }
        Ok(())
    });

    // 6. Multi-index self-derivation (wallet address generation pattern)
    check!("brc42-xkey", "6 multi-index-self-derive", {
        let my_priv = [0xBBu8; 32];
        let my_pub = derive_public_key(&my_priv).map_err(|e| e.to_string())?;
        let mut keys = Vec::new();
        for i in 0..5 {
            let invoice = format!("2-receive address-{}", i);
            let child_pub = derive_child_public_key(&my_priv, &my_pub, &invoice)
                .map_err(|e| e.to_string())?;
            let child_priv = derive_child_private_key(&my_priv, &my_pub, &invoice)
                .map_err(|e| e.to_string())?;
            let derived_pub = derive_public_key(&child_priv).map_err(|e| e.to_string())?;
            if child_pub != derived_pub {
                return Err(format!("index {} pubkey/privkey mismatch", i));
            }
            if keys.contains(&child_pub) {
                return Err(format!("index {} produced duplicate key", i));
            }
            keys.push(child_pub);
        }
        Ok(())
    });
}

// ============================================================================
// [3/7] BRC-43 Invoice Number Protocol
// ============================================================================

fn section_3_brc43_invoice_numbers() {
    use hodos_wallet::crypto::brc43::{InvoiceNumber, SecurityLevel, normalize_protocol_id};

    println!("  [3/7] BRC-43 Invoice Number Protocol");

    // 1. Invoice number creation and formatting
    check!("brc43", "1 create-format", {
        let inv = InvoiceNumber::new(SecurityLevel::NoPermissions, "hello world", "1")
            .map_err(|e| e.to_string())?;
        let formatted = inv.to_string();
        if formatted != "0-hello world-1" {
            return Err(format!("expected '0-hello world-1', got '{}'", formatted));
        }
        Ok(())
    });

    // 2. Parse from string
    check!("brc43", "2 parse-from-string", {
        let inv = InvoiceNumber::from_string("2-certificate field encryption-name")
            .map_err(|e| e.to_string())?;
        if inv.security_level != SecurityLevel::CounterpartyLevel {
            return Err(format!("expected level 2, got {:?}", inv.security_level));
        }
        if inv.protocol_id != "certificate field encryption" {
            return Err(format!("protocol_id: '{}'", inv.protocol_id));
        }
        if inv.key_id != "name" {
            return Err(format!("key_id: '{}'", inv.key_id));
        }
        Ok(())
    });

    // 3. Roundtrip: create → to_string → from_string → compare
    check!("brc43", "3 roundtrip", {
        let original = InvoiceNumber::new(SecurityLevel::ProtocolLevel, "document signing", "doc42")
            .map_err(|e| e.to_string())?;
        let formatted = original.to_string();
        let parsed = InvoiceNumber::from_string(&formatted).map_err(|e| e.to_string())?;
        if original != parsed {
            return Err(format!("roundtrip failed: '{}' vs '{}'", original, parsed));
        }
        Ok(())
    });

    // 4. Key ID with dashes (splitn(3) should handle this)
    check!("brc43", "4 key-id-with-dashes", {
        let inv = InvoiceNumber::from_string("1-document signing-key-with-dashes")
            .map_err(|e| e.to_string())?;
        if inv.key_id != "key-with-dashes" {
            return Err(format!("expected 'key-with-dashes', got '{}'", inv.key_id));
        }
        Ok(())
    });

    // 5. Protocol ID normalization: case + spaces
    check!("brc43", "5 normalize-case-spaces", {
        let n = normalize_protocol_id("  Hello   World  ").map_err(|e| e.to_string())?;
        if n != "hello world" {
            return Err(format!("expected 'hello world', got '{}'", n));
        }
        Ok(())
    });

    // 6. Protocol ID too short (< 5 chars)
    check!("brc43", "6 too-short-rejected", {
        match normalize_protocol_id("test") {
            Err(_) => Ok(()),
            Ok(n) => Err(format!("4-char protocol ID should be rejected, got '{}'", n)),
        }
    });

    // 7. Protocol ID too long (> 280 chars)
    check!("brc43", "7 too-long-rejected", {
        let long = "a".repeat(281);
        match normalize_protocol_id(&long) {
            Err(_) => Ok(()),
            Ok(_) => Err("281-char protocol ID should be rejected".to_string()),
        }
    });

    // 8. Protocol ID with invalid chars
    check!("brc43", "8 invalid-chars-rejected", {
        for bad in &["hello-world", "hello@world", "hello_world", "hello!world"] {
            if normalize_protocol_id(bad).is_ok() {
                return Err(format!("'{}' should be rejected (special chars)", bad));
            }
        }
        Ok(())
    });

    // 9. Protocol ID ending with " protocol"
    check!("brc43", "9 ends-with-protocol-rejected", {
        match normalize_protocol_id("test protocol") {
            Err(_) => Ok(()),
            Ok(_) => Err("ending with ' protocol' should be rejected".to_string()),
        }
    });

    // 10. Key ID boundary lengths
    check!("brc43", "10 key-id-boundaries", {
        // Empty key ID → error
        if InvoiceNumber::new(SecurityLevel::NoPermissions, "hello world", "").is_ok() {
            return Err("empty key ID should be rejected".to_string());
        }
        // 1-char key ID → OK
        InvoiceNumber::new(SecurityLevel::NoPermissions, "hello world", "x")
            .map_err(|e| format!("1-char key ID: {}", e))?;
        // 800-char key ID → OK
        let long_key = "k".repeat(800);
        InvoiceNumber::new(SecurityLevel::NoPermissions, "hello world", &long_key)
            .map_err(|e| format!("800-char key ID: {}", e))?;
        // 801-char key ID → error
        let too_long = "k".repeat(801);
        if InvoiceNumber::new(SecurityLevel::NoPermissions, "hello world", &too_long).is_ok() {
            return Err("801-char key ID should be rejected".to_string());
        }
        Ok(())
    });

    // 11. Security levels round-trip
    check!("brc43", "11 security-levels", {
        for level in 0..=2 {
            let sl = SecurityLevel::from_u8(level)
                .ok_or_else(|| format!("level {} not recognized", level))?;
            if sl.as_u8() != level {
                return Err(format!("level {} roundtrip failed", level));
            }
        }
        if SecurityLevel::from_u8(3).is_some() {
            return Err("level 3 should not exist".to_string());
        }
        Ok(())
    });

    // 12. Invalid format parsing
    check!("brc43", "12 invalid-format-rejected", {
        for bad in &["", "hello", "0-ab-1", "3-hello world-1", "x-hello world-1"] {
            if InvoiceNumber::from_string(bad).is_ok() {
                return Err(format!("'{}' should be rejected", bad));
            }
        }
        Ok(())
    });
}

// ============================================================================
// [4/7] BRC-2 End-to-End Encryption
// ============================================================================

fn section_4_brc2_end_to_end() {
    use hodos_wallet::brc2::{
        derive_symmetric_key, encrypt_brc2, decrypt_brc2,
        encrypt_certificate_field, decrypt_certificate_field,
    };
    use hodos_wallet::crypto::keys::derive_public_key;

    println!("  [4/7] BRC-2 End-to-End Encryption");

    // 1. Symmetric key derivation is deterministic
    check!("brc2", "1 symkey-deterministic", {
        let sender_priv = [0x11u8; 32];
        let recipient_pub = derive_public_key(&[0x22u8; 32]).map_err(|e| e.to_string())?;
        let invoice = "2-test encryption-key1";
        let k1 = derive_symmetric_key(&sender_priv, &recipient_pub, invoice)
            .map_err(|e| e.to_string())?;
        let k2 = derive_symmetric_key(&sender_priv, &recipient_pub, invoice)
            .map_err(|e| e.to_string())?;
        if k1 != k2 {
            return Err("symmetric key derivation is not deterministic".to_string());
        }
        if k1.len() != 32 {
            return Err(format!("expected 32-byte key, got {}", k1.len()));
        }
        Ok(())
    });

    // 2. Encrypt → decrypt roundtrip with raw symmetric key
    check!("brc2", "2 encrypt-decrypt-roundtrip", {
        let key = [0x42u8; 32];
        let plaintext = b"Hello, BRC-2 encryption!";
        let ciphertext = encrypt_brc2(plaintext, &key).map_err(|e| e.to_string())?;
        // Ciphertext must be larger: 32 IV + plaintext.len() + 16 tag
        let expected_len = 32 + plaintext.len() + 16;
        if ciphertext.len() != expected_len {
            return Err(format!("ciphertext len {} != expected {}", ciphertext.len(), expected_len));
        }
        let decrypted = decrypt_brc2(&ciphertext, &key).map_err(|e| e.to_string())?;
        if decrypted != plaintext {
            return Err("decrypted doesn't match plaintext".to_string());
        }
        Ok(())
    });

    // 3. Wrong key fails decryption
    check!("brc2", "3 wrong-key-fails", {
        let key = [0x42u8; 32];
        let wrong_key = [0x43u8; 32];
        let plaintext = b"secret data";
        let ciphertext = encrypt_brc2(plaintext, &key).map_err(|e| e.to_string())?;
        match decrypt_brc2(&ciphertext, &wrong_key) {
            Err(_) => Ok(()),
            Ok(_) => Err("decryption with wrong key should fail".to_string()),
        }
    });

    // 4. Tampered ciphertext fails
    check!("brc2", "4 tampered-ciphertext-fails", {
        let key = [0x42u8; 32];
        let plaintext = b"authenticated data";
        let mut ciphertext = encrypt_brc2(plaintext, &key).map_err(|e| e.to_string())?;
        // Flip a bit in the ciphertext body (after IV)
        ciphertext[40] ^= 0xFF;
        match decrypt_brc2(&ciphertext, &key) {
            Err(_) => Ok(()),
            Ok(_) => Err("tampered ciphertext should fail auth".to_string()),
        }
    });

    // 5. Cross-party: Alice encrypts for Bob using derived symmetric key
    check!("brc2", "5 cross-party-encrypt-decrypt", {
        let alice_priv = [0x11u8; 32];
        let bob_priv = [0x22u8; 32];
        let alice_pub = derive_public_key(&alice_priv).map_err(|e| e.to_string())?;
        let bob_pub = derive_public_key(&bob_priv).map_err(|e| e.to_string())?;
        let invoice = "2-message encryption-msg1";

        // Alice derives symmetric key (sender=Alice, recipient=Bob)
        let alice_key = derive_symmetric_key(&alice_priv, &bob_pub, invoice)
            .map_err(|e| e.to_string())?;
        // Bob derives the same symmetric key (sender=Bob, recipient=Alice)
        // Wait — for BRC-2 the symmetric key derivation is sender→recipient,
        // so Bob needs to use Alice as sender: derive_symmetric_key(bob_priv, alice_pub)
        let bob_key = derive_symmetric_key(&bob_priv, &alice_pub, invoice)
            .map_err(|e| e.to_string())?;

        if alice_key != bob_key {
            return Err(format!(
                "cross-party symmetric keys differ:\n  alice: {}\n  bob:   {}",
                hex::encode(&alice_key), hex::encode(&bob_key)
            ));
        }

        // Alice encrypts
        let plaintext = b"Secret message from Alice to Bob";
        let ciphertext = encrypt_brc2(plaintext, &alice_key).map_err(|e| e.to_string())?;
        // Bob decrypts
        let decrypted = decrypt_brc2(&ciphertext, &bob_key).map_err(|e| e.to_string())?;
        if decrypted != plaintext {
            return Err("Bob couldn't decrypt Alice's message".to_string());
        }
        Ok(())
    });

    // 6. Self-encryption (sender == recipient)
    check!("brc2", "6 self-encryption", {
        let my_priv = [0x33u8; 32];
        let my_pub = derive_public_key(&my_priv).map_err(|e| e.to_string())?;
        let invoice = "2-personal notes-note1";
        let key = derive_symmetric_key(&my_priv, &my_pub, invoice)
            .map_err(|e| e.to_string())?;
        let plaintext = b"My personal encrypted note";
        let ciphertext = encrypt_brc2(plaintext, &key).map_err(|e| e.to_string())?;
        let decrypted = decrypt_brc2(&ciphertext, &key).map_err(|e| e.to_string())?;
        if decrypted != plaintext {
            return Err("self-encryption roundtrip failed".to_string());
        }
        Ok(())
    });

    // 7. Certificate field encrypt/decrypt roundtrip
    check!("brc2", "7 cert-field-roundtrip", {
        let certifier_priv = [0x44u8; 32];
        let subject_priv = [0x55u8; 32];
        let certifier_pub = derive_public_key(&certifier_priv).map_err(|e| e.to_string())?;
        let subject_pub = derive_public_key(&subject_priv).map_err(|e| e.to_string())?;
        let field_name = "userName";
        let plaintext = b"Alice Smith";

        // Certifier encrypts field for subject
        let ciphertext = encrypt_certificate_field(
            &certifier_priv, &subject_pub, field_name, None, plaintext,
        ).map_err(|e| e.to_string())?;

        // Subject decrypts
        let decrypted = decrypt_certificate_field(
            &subject_priv, &certifier_pub, field_name, None, &ciphertext,
        ).map_err(|e| e.to_string())?;

        if decrypted != plaintext {
            return Err("certificate field roundtrip failed".to_string());
        }
        Ok(())
    });

    // 8. Certificate field with serial number
    check!("brc2", "8 cert-field-with-serial", {
        let certifier_priv = [0x44u8; 32];
        let subject_priv = [0x55u8; 32];
        let certifier_pub = derive_public_key(&certifier_priv).map_err(|e| e.to_string())?;
        let subject_pub = derive_public_key(&subject_priv).map_err(|e| e.to_string())?;
        let serial = "abc123def456";
        let plaintext = b"alice@example.com";

        let ciphertext = encrypt_certificate_field(
            &certifier_priv, &subject_pub, "email", Some(serial), plaintext,
        ).map_err(|e| e.to_string())?;

        let decrypted = decrypt_certificate_field(
            &subject_priv, &certifier_pub, "email", Some(serial), &ciphertext,
        ).map_err(|e| e.to_string())?;

        if decrypted != plaintext {
            return Err("cert field with serial roundtrip failed".to_string());
        }
        Ok(())
    });

    // 9. Ciphertext too short rejected
    check!("brc2", "9 short-ciphertext-rejected", {
        let key = [0x42u8; 32];
        // Needs at least 48 bytes (32 IV + 16 tag)
        let short = vec![0u8; 47];
        match decrypt_brc2(&short, &key) {
            Err(_) => Ok(()),
            Ok(_) => Err("47-byte ciphertext should be rejected".to_string()),
        }
    });

    // 10. Different invoices produce different symmetric keys
    check!("brc2", "10 different-invoices-different-keys", {
        let priv_key = [0x11u8; 32];
        let pub_key = derive_public_key(&[0x22u8; 32]).map_err(|e| e.to_string())?;
        let k1 = derive_symmetric_key(&priv_key, &pub_key, "2-test encryption-key1")
            .map_err(|e| e.to_string())?;
        let k2 = derive_symmetric_key(&priv_key, &pub_key, "2-test encryption-key2")
            .map_err(|e| e.to_string())?;
        if k1 == k2 {
            return Err("different invoices produced same symmetric key".to_string());
        }
        Ok(())
    });
}

// ============================================================================
// [5/7] PIN Encryption
// ============================================================================

fn section_5_pin_encryption() {
    use hodos_wallet::crypto::pin::{derive_key_from_pin, encrypt_mnemonic, decrypt_mnemonic};

    println!("  [5/7] PIN Encryption");

    let test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    // 1. Encrypt/decrypt roundtrip
    check!("pin", "1 roundtrip", {
        let pin = "1234";
        let (salt, encrypted) = encrypt_mnemonic(test_mnemonic, pin)
            .map_err(|e| e.to_string())?;
        let decrypted = decrypt_mnemonic(&encrypted, pin, &salt)
            .map_err(|e| e.to_string())?;
        if decrypted != test_mnemonic {
            return Err("decrypt didn't match original".to_string());
        }
        Ok(())
    });

    // 2. Wrong PIN rejected
    check!("pin", "2 wrong-pin-rejected", {
        let (salt, encrypted) = encrypt_mnemonic(test_mnemonic, "1234")
            .map_err(|e| e.to_string())?;
        match decrypt_mnemonic(&encrypted, "5678", &salt) {
            Err(e) if e.contains("Invalid PIN") => Ok(()),
            Err(e) => Ok(()), // any error is acceptable
            Ok(_) => Err("wrong PIN should fail".to_string()),
        }
    });

    // 3. PBKDF2 key derivation determinism (same PIN + salt → same key)
    check!("pin", "3 pbkdf2-determinism", {
        let salt = hex::decode("0102030405060708090a0b0c0d0e0f10").unwrap();
        let k1 = derive_key_from_pin("1234", &salt);
        let k2 = derive_key_from_pin("1234", &salt);
        if k1 != k2 {
            return Err("PBKDF2 not deterministic with same inputs".to_string());
        }
        Ok(())
    });

    // 4. Different PINs produce different keys
    check!("pin", "4 different-pins-different-keys", {
        let salt = hex::decode("0102030405060708090a0b0c0d0e0f10").unwrap();
        let k1 = derive_key_from_pin("1234", &salt);
        let k2 = derive_key_from_pin("5678", &salt);
        if k1 == k2 {
            return Err("different PINs produced same key".to_string());
        }
        Ok(())
    });

    // 5. Different salts produce different keys
    check!("pin", "5 different-salts-different-keys", {
        let salt1 = [0x01u8; 16];
        let salt2 = [0x02u8; 16];
        let k1 = derive_key_from_pin("1234", &salt1);
        let k2 = derive_key_from_pin("1234", &salt2);
        if k1 == k2 {
            return Err("different salts produced same key".to_string());
        }
        Ok(())
    });

    // 6. Random salt means different ciphertexts each time
    check!("pin", "6 random-salt-different-ciphertexts", {
        let (salt1, enc1) = encrypt_mnemonic(test_mnemonic, "1234")
            .map_err(|e| e.to_string())?;
        let (salt2, enc2) = encrypt_mnemonic(test_mnemonic, "1234")
            .map_err(|e| e.to_string())?;
        if salt1 == salt2 {
            return Err("two encryptions produced same salt (extremely unlikely)".to_string());
        }
        if enc1 == enc2 {
            return Err("two encryptions produced same ciphertext".to_string());
        }
        // But both should decrypt correctly
        let d1 = decrypt_mnemonic(&enc1, "1234", &salt1).map_err(|e| e.to_string())?;
        let d2 = decrypt_mnemonic(&enc2, "1234", &salt2).map_err(|e| e.to_string())?;
        if d1 != test_mnemonic || d2 != test_mnemonic {
            return Err("both ciphertexts should decrypt to same plaintext".to_string());
        }
        Ok(())
    });

    // 7. Truncated ciphertext rejected
    check!("pin", "7 truncated-ciphertext-rejected", {
        let (salt, encrypted) = encrypt_mnemonic(test_mnemonic, "1234")
            .map_err(|e| e.to_string())?;
        // Truncate the encrypted hex to make it too short
        let short = &encrypted[..20]; // way too short
        match decrypt_mnemonic(short, "1234", &salt) {
            Err(_) => Ok(()),
            Ok(_) => Err("truncated ciphertext should fail".to_string()),
        }
    });

    // 8. Key output is always 32 bytes
    check!("pin", "8 key-always-32-bytes", {
        let salt = [0x42u8; 16];
        for pin in &["0", "1234", "000000", "a very long passphrase with spaces"] {
            let key = derive_key_from_pin(pin, &salt);
            if key.len() != 32 {
                return Err(format!("PIN '{}' produced {}-byte key", pin, key.len()));
            }
        }
        Ok(())
    });
}

// ============================================================================
// [6/7] Transaction Build → Sign → Verify Workflow
// ============================================================================

fn section_6_transaction_workflow() {
    use hodos_wallet::transaction::{
        Transaction, TxInput, TxOutput, OutPoint, Script,
        calculate_sighash, SIGHASH_ALL_FORKID,
        encode_varint, decode_varint, extract_input_outpoints,
    };
    use hodos_wallet::crypto::signing::{sign_ecdsa, verify_signature};
    use hodos_wallet::crypto::keys::derive_public_key;

    println!("  [6/7] Transaction Build → Sign → Verify Workflow");

    // 1. Build a P2PKH transaction, compute sighash, sign, verify
    check!("tx", "1 build-sign-verify", {
        let privkey = [0x11u8; 32];
        let pubkey = derive_public_key(&privkey).map_err(|e| e.to_string())?;

        // Create a fake previous txid
        let prev_txid = "a".repeat(64);
        let mut tx = Transaction::new();
        tx.add_input(TxInput::new(OutPoint::new(&prev_txid, 0)));

        // Create P2PKH locking script from pubkey hash
        let pubkey_hash = {
            use sha2::{Sha256, Digest};
            use ripemd::Ripemd160;
            let sha = Sha256::digest(&pubkey);
            Ripemd160::digest(&sha).to_vec()
        };
        let locking_script = Script::p2pkh_locking_script(&pubkey_hash)
            .map_err(|e| e.to_string())?;
        tx.add_output(TxOutput::new(50000, locking_script.bytes.clone()));

        // Calculate sighash for input 0
        let sighash = calculate_sighash(&tx, 0, &locking_script.bytes, 100000, SIGHASH_ALL_FORKID)
            .map_err(|e| e.to_string())?;
        if sighash.len() != 32 {
            return Err(format!("sighash should be 32 bytes, got {}", sighash.len()));
        }

        // Sign
        let sig = sign_ecdsa(&sighash, &privkey, 0x41).map_err(|e| e.to_string())?;

        // Verify
        let valid = verify_signature(&sighash, &sig, &pubkey).map_err(|e| e.to_string())?;
        if !valid {
            return Err("valid signature failed verification".to_string());
        }
        Ok(())
    });

    // 2. Unlocking script construction
    check!("tx", "2 unlocking-script-structure", {
        let privkey = [0x22u8; 32];
        let pubkey = derive_public_key(&privkey).map_err(|e| e.to_string())?;
        let fake_sig = vec![0x30, 0x44]; // doesn't matter for structure test
        let unlocking = Script::p2pkh_unlocking_script(&fake_sig, &pubkey);
        // Should be: <sig_len><sig_bytes><pubkey_len><pubkey_bytes>
        if unlocking.bytes[0] as usize != fake_sig.len() {
            return Err("sig length prefix wrong".to_string());
        }
        let pubkey_offset = 1 + fake_sig.len();
        if unlocking.bytes[pubkey_offset] as usize != pubkey.len() {
            return Err("pubkey length prefix wrong".to_string());
        }
        if &unlocking.bytes[pubkey_offset + 1..] != pubkey.as_slice() {
            return Err("pubkey bytes mismatch in unlocking script".to_string());
        }
        Ok(())
    });

    // 3. Varint encode → decode roundtrip
    check!("tx", "3 varint-roundtrip", {
        let test_values: Vec<u64> = vec![
            0, 1, 252,          // 1-byte
            253, 65535,          // 2-byte (FD prefix)
            65536, 4294967295,   // 4-byte (FE prefix)
        ];
        for val in &test_values {
            let encoded = encode_varint(*val);
            let (decoded, consumed) = decode_varint(&encoded).map_err(|e| e.to_string())?;
            if decoded != *val {
                return Err(format!("varint {} roundtrip: got {}", val, decoded));
            }
            if consumed != encoded.len() {
                return Err(format!("varint {} consumed {} != encoded len {}",
                    val, consumed, encoded.len()));
            }
        }
        Ok(())
    });

    // 4. Transaction serialization → extract outpoints
    check!("tx", "4 serialize-extract-outpoints", {
        let prev_txid1 = "aa".repeat(32);
        let prev_txid2 = "bb".repeat(32);
        let mut tx = Transaction::new();
        tx.add_input(TxInput::new(OutPoint::new(&prev_txid1, 0)));
        tx.add_input(TxInput::new(OutPoint::new(&prev_txid2, 3)));

        // Need at least one output for valid serialization
        tx.add_output(TxOutput::new(1000, vec![0x76, 0xa9]));

        let hex = tx.to_hex().map_err(|e| e.to_string())?;
        let outpoints = extract_input_outpoints(&hex).map_err(|e| e.to_string())?;

        if outpoints.len() != 2 {
            return Err(format!("expected 2 outpoints, got {}", outpoints.len()));
        }
        if outpoints[0].0 != prev_txid1 || outpoints[0].1 != 0 {
            return Err(format!("outpoint 0: {:?}", outpoints[0]));
        }
        if outpoints[1].0 != prev_txid2 || outpoints[1].1 != 3 {
            return Err(format!("outpoint 1: {:?}", outpoints[1]));
        }
        Ok(())
    });

    // 5. Different inputs produce different sighashes
    check!("tx", "5 different-inputs-different-sighash", {
        let prev_txid = "cc".repeat(32);
        let mut tx = Transaction::new();
        tx.add_input(TxInput::new(OutPoint::new(&prev_txid, 0)));
        tx.add_input(TxInput::new(OutPoint::new(&prev_txid, 1)));
        let script = vec![0x76, 0xa9, 0x14];
        tx.add_output(TxOutput::new(1000, script.clone()));

        let sh0 = calculate_sighash(&tx, 0, &script, 50000, SIGHASH_ALL_FORKID)
            .map_err(|e| e.to_string())?;
        let sh1 = calculate_sighash(&tx, 1, &script, 50000, SIGHASH_ALL_FORKID)
            .map_err(|e| e.to_string())?;
        if sh0 == sh1 {
            return Err("different input indices produced same sighash".to_string());
        }
        Ok(())
    });

    // 6. Locking script construction
    check!("tx", "6 p2pkh-locking-script", {
        let hash = [0xAAu8; 20];
        let script = Script::p2pkh_locking_script(&hash).map_err(|e| e.to_string())?;
        if script.bytes.len() != 25 {
            return Err(format!("P2PKH should be 25 bytes, got {}", script.bytes.len()));
        }
        // Verify opcodes: OP_DUP OP_HASH160 PUSH20 <hash> OP_EQUALVERIFY OP_CHECKSIG
        if script.bytes[0] != 0x76 || script.bytes[1] != 0xa9 || script.bytes[2] != 0x14 {
            return Err("wrong prefix opcodes".to_string());
        }
        if &script.bytes[3..23] != &hash {
            return Err("hash mismatch in script".to_string());
        }
        if script.bytes[23] != 0x88 || script.bytes[24] != 0xac {
            return Err("wrong suffix opcodes".to_string());
        }
        Ok(())
    });

    // 7. Invalid pubkey hash length rejected
    check!("tx", "7 bad-pubkey-hash-rejected", {
        for len in &[0, 19, 21, 32] {
            let bad_hash = vec![0xBB; *len];
            if Script::p2pkh_locking_script(&bad_hash).is_ok() {
                return Err(format!("{}-byte hash should be rejected", len));
            }
        }
        Ok(())
    });
}

// ============================================================================
// [7/7] Cross-Module Integration
// ============================================================================

fn section_7_cross_module_integration() {
    use hodos_wallet::crypto::keys::derive_public_key;
    use hodos_wallet::crypto::signing::{sign_ecdsa, verify_signature, sha256};
    use hodos_wallet::crypto::brc42::{derive_child_public_key, derive_child_private_key};
    use hodos_wallet::brc2::{encrypt_brc2, decrypt_brc2, derive_symmetric_key};
    use hodos_wallet::recovery::{derive_key_at_path, derive_address_at_path, address_to_p2pkh_script};
    use hodos_wallet::script::pushdrop::{encode, decode, LockPosition};

    println!("  [7/7] Cross-Module Integration");

    // 1. BIP32 derive key → sign → verify
    check!("xmod", "1 bip32-sign-verify", {
        let seed = hex::decode("000102030405060708090a0b0c0d0e0f").unwrap();
        let privkey = derive_key_at_path(&seed, &[(0, true)]).map_err(|e| e.to_string())?;
        let pubkey = derive_public_key(&privkey).map_err(|e| e.to_string())?;
        let hash = sha256(b"message to sign");
        let sig = sign_ecdsa(&hash, &privkey, 0x01).map_err(|e| e.to_string())?;
        let valid = verify_signature(&hash, &sig, &pubkey).map_err(|e| e.to_string())?;
        if !valid {
            return Err("BIP32-derived key: sign+verify failed".to_string());
        }
        Ok(())
    });

    // 2. BRC-42 derive key → sign → verify
    check!("xmod", "2 brc42-sign-verify", {
        let master_priv = [0x11u8; 32];
        let master_pub = derive_public_key(&master_priv).map_err(|e| e.to_string())?;
        let counterparty_priv = [0x22u8; 32];
        let counterparty_pub = derive_public_key(&counterparty_priv).map_err(|e| e.to_string())?;
        let invoice = "2-test signing-key1";

        // Recipient derives child private key
        let child_priv = derive_child_private_key(&master_priv, &counterparty_pub, invoice)
            .map_err(|e| e.to_string())?;
        // Sender derives child public key
        let child_pub = derive_child_public_key(&counterparty_priv, &master_pub, invoice)
            .map_err(|e| e.to_string())?;

        let hash = sha256(b"BRC-42 authenticated message");
        let sig = sign_ecdsa(&hash, &child_priv, 0x41).map_err(|e| e.to_string())?;
        let valid = verify_signature(&hash, &sig, &child_pub).map_err(|e| e.to_string())?;
        if !valid {
            return Err("BRC-42 child key: sign+verify failed".to_string());
        }
        Ok(())
    });

    // 3. BRC-42 → BRC-2 full pipeline (derive keys → encrypt → decrypt)
    check!("xmod", "3 brc42-brc2-full-pipeline", {
        let alice_priv = [0x33u8; 32];
        let bob_priv = [0x44u8; 32];
        let alice_pub = derive_public_key(&alice_priv).map_err(|e| e.to_string())?;
        let bob_pub = derive_public_key(&bob_priv).map_err(|e| e.to_string())?;
        let invoice = "2-secure messaging-chat1";

        // Alice encrypts for Bob
        let key_alice = derive_symmetric_key(&alice_priv, &bob_pub, invoice)
            .map_err(|e| e.to_string())?;
        let plaintext = b"Top secret message via BRC-42 + BRC-2";
        let ciphertext = encrypt_brc2(plaintext, &key_alice).map_err(|e| e.to_string())?;

        // Bob decrypts
        let key_bob = derive_symmetric_key(&bob_priv, &alice_pub, invoice)
            .map_err(|e| e.to_string())?;
        let decrypted = decrypt_brc2(&ciphertext, &key_bob).map_err(|e| e.to_string())?;

        if decrypted != plaintext {
            return Err("BRC-42 → BRC-2 pipeline failed".to_string());
        }
        Ok(())
    });

    // 4. PushDrop encode certificate JSON → decode → verify
    check!("xmod", "4 pushdrop-certificate-json", {
        let certifier_priv = [0x55u8; 32];
        let certifier_pub = derive_public_key(&certifier_priv).map_err(|e| e.to_string())?;
        let cert_json = br#"{"type":"identity","subject":"02abc...","certifier":"02def..."}"#;

        let script = encode(
            &[cert_json.to_vec()],
            &certifier_pub,
            LockPosition::Before,
        ).map_err(|e| e.to_string())?;

        let decoded = decode(&script).map_err(|e| e.to_string())?;
        if decoded.locking_public_key != certifier_pub {
            return Err("decoded pubkey mismatch".to_string());
        }
        if decoded.fields.len() != 1 {
            return Err(format!("expected 1 field, got {}", decoded.fields.len()));
        }
        if decoded.fields[0] != cert_json {
            return Err("decoded certificate JSON mismatch".to_string());
        }
        Ok(())
    });

    // 5. BIP32 derive address → address_to_p2pkh_script → verify consistency
    check!("xmod", "5 bip32-address-script-consistency", {
        let seed = hex::decode("000102030405060708090a0b0c0d0e0f").unwrap();
        for idx in 0..3 {
            let (address, pubkey_hex, _privkey) = derive_address_at_path(
                &seed, &[(44, true), (0, false), (0, false), (idx, false)]
            ).map_err(|e| e.to_string())?;

            let script = address_to_p2pkh_script(&address).map_err(|e| e.to_string())?;
            if script.len() != 25 {
                return Err(format!("index {}: script {} bytes", idx, script.len()));
            }

            // Extract pubkey hash from script and verify it matches the address
            let script_hash = &script[3..23];
            // Compute expected hash from pubkey
            let pubkey_bytes = hex::decode(&pubkey_hex).map_err(|e| e.to_string())?;
            let expected_hash = {
                use sha2::{Sha256, Digest};
                use ripemd::Ripemd160;
                let sha = Sha256::digest(&pubkey_bytes);
                Ripemd160::digest(&sha).to_vec()
            };
            if script_hash != expected_hash.as_slice() {
                return Err(format!("index {}: pubkey hash mismatch in script", idx));
            }
        }
        Ok(())
    });

    // 6. Self-derivation address → sign transaction (wallet spend flow)
    check!("xmod", "6 self-derive-spend-flow", {
        use hodos_wallet::transaction::{
            Transaction, TxInput, TxOutput, OutPoint, Script as TxScript,
            calculate_sighash, SIGHASH_ALL_FORKID,
        };

        let master_priv = [0x66u8; 32];
        let master_pub = derive_public_key(&master_priv).map_err(|e| e.to_string())?;
        let invoice = "2-receive address-0";

        // Derive child keys (self-derivation)
        let child_priv = derive_child_private_key(&master_priv, &master_pub, &invoice)
            .map_err(|e| e.to_string())?;
        let child_pub = derive_public_key(&child_priv).map_err(|e| e.to_string())?;

        // Build P2PKH locking script for the derived address
        let pubkey_hash = {
            use sha2::{Sha256, Digest};
            use ripemd::Ripemd160;
            let sha = Sha256::digest(&child_pub);
            Ripemd160::digest(&sha).to_vec()
        };
        let locking_script = TxScript::p2pkh_locking_script(&pubkey_hash)
            .map_err(|e| e.to_string())?;

        // Build spend transaction
        let mut tx = Transaction::new();
        tx.add_input(TxInput::new(OutPoint::new(&"dd".repeat(32), 0)));
        tx.add_output(TxOutput::new(49000, locking_script.bytes.clone()));

        // Calculate sighash and sign with derived key
        let sighash = calculate_sighash(&tx, 0, &locking_script.bytes, 50000, SIGHASH_ALL_FORKID)
            .map_err(|e| e.to_string())?;
        let sig = sign_ecdsa(&sighash, &child_priv, 0x41).map_err(|e| e.to_string())?;

        // Verify with derived public key
        let valid = verify_signature(&sighash, &sig, &child_pub).map_err(|e| e.to_string())?;
        if !valid {
            return Err("self-derive spend flow: sign+verify failed".to_string());
        }

        // Build unlocking script and set on input
        let unlocking = TxScript::p2pkh_unlocking_script(&sig, &child_pub);
        if unlocking.bytes.is_empty() {
            return Err("unlocking script is empty".to_string());
        }

        Ok(())
    });

    // 7. Multi-field PushDrop with BRC-42 derived key
    check!("xmod", "7 pushdrop-brc42-multi-field", {
        let master_priv = [0x77u8; 32];
        let master_pub = derive_public_key(&master_priv).map_err(|e| e.to_string())?;
        let invoice = "2-token issuance-token1";
        let child_pub = derive_child_public_key(&master_priv, &master_pub, invoice)
            .map_err(|e| e.to_string())?;

        let fields = vec![
            b"token_type:identity".to_vec(),
            b"issuer:hodos".to_vec(),
            b"subject:02abc123".to_vec(),
        ];

        let script = encode(&fields, &child_pub, LockPosition::After)
            .map_err(|e| e.to_string())?;
        let decoded = decode(&script).map_err(|e| e.to_string())?;

        if decoded.locking_public_key != child_pub {
            return Err("decoded key != BRC-42 derived key".to_string());
        }
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
}
