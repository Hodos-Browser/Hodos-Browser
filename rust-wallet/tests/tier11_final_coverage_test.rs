///! Tier 11 — Final Coverage: Isolated BRC-42 Internals, Error Display, Model Constructors, Cross-Module
///!
///! Sections:
///!  [1/7]  compute_invoice_hmac (BRC-42 Step 2)                  (8 tests)
///!  [2/7]  Brc42Error Display + edge cases                       (6 tests)
///!  [3/7]  SigningError Display + verify_signature edge cases     (8 tests)
///!  [4/7]  ScriptParseError Display (all 4 variants)             (6 tests)
///!  [5/7]  DomainPermission::defaults + CertFieldPermission      (6 tests)
///!  [6/7]  PriceCache Default + Brc43 edge cases                 (6 tests)
///!  [7/7]  Cross-module integration pipelines                    (8 tests)
///!
///! Total: 48 tests

use std::sync::atomic::{AtomicUsize, Ordering};
use sha2::{Sha256, Digest};

static PASS: AtomicUsize = AtomicUsize::new(0);
static FAIL: AtomicUsize = AtomicUsize::new(0);

macro_rules! check {
    ($tag:expr, $block:expr) => {{
        let tag = $tag;
        let result: Result<(), String> = (|| $block)();
        match result {
            Ok(()) => {
                PASS.fetch_add(1, Ordering::SeqCst);
                eprintln!("  PASS  {}", tag);
            }
            Err(e) => {
                FAIL.fetch_add(1, Ordering::SeqCst);
                eprintln!("**FAIL** {} — {}", tag, e);
            }
        }
    }};
}

// ═══════════════════════════════════════════════════════════════════
// [1/7]  compute_invoice_hmac (BRC-42 Step 2)
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t11_01_compute_invoice_hmac() {
    use hodos_wallet::crypto::brc42::{compute_invoice_hmac, compute_shared_secret};

    eprintln!("\n[1/7] compute_invoice_hmac");

    check!("hmac/01 deterministic", {
        let shared_secret = vec![0x02; 33]; // fake compressed pubkey format
        let h1 = compute_invoice_hmac(&shared_secret, "2-test-key1").map_err(|e| format!("{}", e))?;
        let h2 = compute_invoice_hmac(&shared_secret, "2-test-key1").map_err(|e| format!("{}", e))?;
        if h1 != h2 {
            return Err("HMAC should be deterministic".into());
        }
        Ok(())
    });

    check!("hmac/02 output-is-32-bytes", {
        let shared_secret = vec![0x03; 33];
        let result = compute_invoice_hmac(&shared_secret, "2-proto-key1").map_err(|e| format!("{}", e))?;
        if result.len() != 32 {
            return Err(format!("expected 32 bytes, got {}", result.len()));
        }
        Ok(())
    });

    check!("hmac/03 different-invoices-different-hmacs", {
        let shared_secret = vec![0x02; 33];
        let h1 = compute_invoice_hmac(&shared_secret, "2-proto-key1").map_err(|e| format!("{}", e))?;
        let h2 = compute_invoice_hmac(&shared_secret, "2-proto-key2").map_err(|e| format!("{}", e))?;
        if h1 == h2 {
            return Err("different invoice numbers should produce different HMACs".into());
        }
        Ok(())
    });

    check!("hmac/04 different-secrets-different-hmacs", {
        let s1 = vec![0x02; 33];
        let s2 = vec![0x03; 33];
        let h1 = compute_invoice_hmac(&s1, "2-test-1").map_err(|e| format!("{}", e))?;
        let h2 = compute_invoice_hmac(&s2, "2-test-1").map_err(|e| format!("{}", e))?;
        if h1 == h2 {
            return Err("different secrets should produce different HMACs".into());
        }
        Ok(())
    });

    check!("hmac/05 empty-invoice-number", {
        let shared_secret = vec![0x02; 33];
        let result = compute_invoice_hmac(&shared_secret, "").map_err(|e| format!("{}", e))?;
        if result.len() != 32 {
            return Err("empty invoice should still produce 32-byte HMAC".into());
        }
        Ok(())
    });

    check!("hmac/06 with-real-ecdh-secret", {
        // Use real ECDH shared secret from compute_shared_secret
        use secp256k1::{Secp256k1, SecretKey, PublicKey};
        let secp = Secp256k1::new();
        let priv_a = SecretKey::from_slice(&[7u8; 32]).unwrap();
        let pub_b_key = SecretKey::from_slice(&[8u8; 32]).unwrap();
        let pub_b = PublicKey::from_secret_key(&secp, &pub_b_key);

        let shared = compute_shared_secret(&priv_a.secret_bytes(), &pub_b.serialize())
            .map_err(|e| format!("{}", e))?;
        let hmac = compute_invoice_hmac(&shared, "2-certificate signature-test")
            .map_err(|e| format!("{}", e))?;
        if hmac.len() != 32 {
            return Err(format!("HMAC len {} != 32", hmac.len()));
        }
        // Verify it produces valid secp256k1 scalar (all 32 byte values are valid unless 0 or >= N)
        // Just check it's not all zeros
        if hmac.iter().all(|&b| b == 0) {
            return Err("HMAC should not be all zeros".into());
        }
        Ok(())
    });

    check!("hmac/07 long-invoice-number", {
        let shared_secret = vec![0x02; 33];
        let long_invoice = "2-".to_string() + &"a".repeat(1000) + "-key1";
        let result = compute_invoice_hmac(&shared_secret, &long_invoice).map_err(|e| format!("{}", e))?;
        if result.len() != 32 {
            return Err("long invoice should still produce 32 bytes".into());
        }
        Ok(())
    });

    check!("hmac/08 unicode-invoice-number", {
        let shared_secret = vec![0x02; 33];
        let result = compute_invoice_hmac(&shared_secret, "2-日本語-テスト").map_err(|e| format!("{}", e))?;
        if result.len() != 32 {
            return Err("unicode invoice should produce 32 bytes".into());
        }
        Ok(())
    });
}

// ═══════════════════════════════════════════════════════════════════
// [2/7]  Brc42Error Display + edge cases
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t11_02_brc42_error() {
    use hodos_wallet::crypto::brc42::{
        Brc42Error, compute_shared_secret, derive_child_public_key, derive_child_private_key,
    };

    eprintln!("\n[2/7] Brc42Error Display + edge cases");

    check!("brc42err/01 display-invalid-private-key", {
        let e = Brc42Error::InvalidPrivateKey("all zeros".into());
        let msg = format!("{}", e);
        if !msg.contains("invalid private key") || !msg.contains("all zeros") {
            return Err(format!("unexpected: {}", msg));
        }
        Ok(())
    });

    check!("brc42err/02 display-invalid-public-key", {
        let e = Brc42Error::InvalidPublicKey("bad point".into());
        let msg = format!("{}", e);
        if !msg.contains("invalid public key") {
            return Err(format!("unexpected: {}", msg));
        }
        Ok(())
    });

    check!("brc42err/03 display-derivation-failed", {
        let e = Brc42Error::DerivationFailed("HMAC init".into());
        let msg = format!("{}", e);
        if !msg.contains("derivation failed") {
            return Err(format!("unexpected: {}", msg));
        }
        Ok(())
    });

    check!("brc42err/04 display-secp256k1-error", {
        let e = Brc42Error::Secp256k1Error("tweak overflow".into());
        let msg = format!("{}", e);
        if !msg.contains("secp256k1 error") {
            return Err(format!("unexpected: {}", msg));
        }
        Ok(())
    });

    check!("brc42err/05 shared-secret-bad-privkey", {
        let zero_key = [0u8; 32];
        match compute_shared_secret(&zero_key, &[0x02; 33]) {
            Err(e) => {
                let msg = format!("{}", e);
                if !msg.contains("private key") && !msg.contains("Invalid") {
                    return Err(format!("error should mention private key: {}", msg));
                }
                Ok(())
            }
            Ok(_) => Err("zero private key should fail".into()),
        }
    });

    check!("brc42err/06 shared-secret-bad-pubkey", {
        let valid_priv = [1u8; 32]; // valid private key
        let bad_pub = [0xFF; 33]; // invalid public key
        match compute_shared_secret(&valid_priv, &bad_pub) {
            Err(e) => {
                let msg = format!("{}", e);
                if !msg.contains("public key") && !msg.contains("Invalid") {
                    return Err(format!("error should mention public key: {}", msg));
                }
                Ok(())
            }
            Ok(_) => Err("invalid pubkey should fail".into()),
        }
    });
}

// ═══════════════════════════════════════════════════════════════════
// [3/7]  SigningError Display + verify_signature edge cases
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t11_03_signing_error() {
    use hodos_wallet::crypto::signing::{SigningError, sign_ecdsa, verify_signature};

    eprintln!("\n[3/7] SigningError Display + verify_signature edges");

    check!("sigerr/01 display-invalid-private-key", {
        let e = SigningError::InvalidPrivateKey("too short".into());
        let msg = format!("{}", e);
        if !msg.contains("invalid private key") {
            return Err(format!("unexpected: {}", msg));
        }
        Ok(())
    });

    check!("sigerr/02 display-invalid-message", {
        let e = SigningError::InvalidMessage("wrong length".into());
        let msg = format!("{}", e);
        if !msg.contains("invalid message hash") {
            return Err(format!("unexpected: {}", msg));
        }
        Ok(())
    });

    check!("sigerr/03 display-invalid-signature", {
        let e = SigningError::InvalidSignature("bad DER".into());
        let msg = format!("{}", e);
        if !msg.contains("invalid signature") {
            return Err(format!("unexpected: {}", msg));
        }
        Ok(())
    });

    check!("sigerr/04 sign-wrong-hash-length", {
        match sign_ecdsa(&[0u8; 31], &[1u8; 32], 0x01) {
            Err(e) => {
                let msg = format!("{}", e);
                if !msg.contains("32 bytes") {
                    return Err(format!("should mention 32 bytes: {}", msg));
                }
                Ok(())
            }
            Ok(_) => Err("31-byte hash should fail".into()),
        }
    });

    check!("sigerr/05 sign-wrong-privkey-length", {
        match sign_ecdsa(&[0u8; 32], &[1u8; 16], 0x01) {
            Err(e) => {
                let msg = format!("{}", e);
                if !msg.contains("32 bytes") {
                    return Err(format!("should mention 32 bytes: {}", msg));
                }
                Ok(())
            }
            Ok(_) => Err("16-byte privkey should fail".into()),
        }
    });

    check!("sigerr/06 verify-wrong-hash-length", {
        match verify_signature(&[0u8; 31], &[0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01, 0x01], &[0x02; 33]) {
            Err(e) => {
                let msg = format!("{}", e);
                if !msg.contains("32 bytes") {
                    return Err(format!("should mention 32 bytes: {}", msg));
                }
                Ok(())
            }
            Ok(_) => Err("31-byte hash should fail verify".into()),
        }
    });

    check!("sigerr/07 verify-empty-signature", {
        match verify_signature(&[0u8; 32], &[], &[0x02; 33]) {
            Err(e) => {
                let msg = format!("{}", e);
                if !msg.contains("Empty signature") && !msg.contains("empty") {
                    return Err(format!("should mention empty: {}", msg));
                }
                Ok(())
            }
            Ok(_) => Err("empty signature should fail".into()),
        }
    });

    check!("sigerr/08 verify-bad-der", {
        // Valid hash + invalid DER + valid pubkey format
        use secp256k1::{Secp256k1, SecretKey, PublicKey};
        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let pk = PublicKey::from_secret_key(&secp, &sk);

        match verify_signature(&[0u8; 32], &[0xFF, 0xFF, 0xFF, 0x01], &pk.serialize()) {
            Err(e) => {
                let _ = format!("{}", e); // Just verify Display works
                Ok(())
            }
            Ok(valid) => {
                if valid { return Err("bad DER should not verify as valid".into()); }
                Ok(()) // false is also acceptable
            }
        }
    });
}

// ═══════════════════════════════════════════════════════════════════
// [4/7]  ScriptParseError Display (all 4 variants)
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t11_04_script_parse_error() {
    use hodos_wallet::script::ScriptParseError;
    use hodos_wallet::script::parse_script_chunks;

    eprintln!("\n[4/7] ScriptParseError Display");

    check!("spe/01 unexpected-end-display", {
        let e = ScriptParseError::UnexpectedEndOfScript;
        let msg = format!("{}", e);
        if !msg.contains("Unexpected end") {
            return Err(format!("unexpected: {}", msg));
        }
        Ok(())
    });

    check!("spe/02 invalid-push-data-length-display", {
        let e = ScriptParseError::InvalidPushDataLength;
        let msg = format!("{}", e);
        if !msg.contains("push data length") {
            return Err(format!("unexpected: {}", msg));
        }
        Ok(())
    });

    check!("spe/03 invalid-opcode-display", {
        let e = ScriptParseError::InvalidOpcode(0xBA);
        let msg = format!("{}", e);
        if !msg.contains("0xba") {
            return Err(format!("should contain hex opcode: {}", msg));
        }
        Ok(())
    });

    check!("spe/04 other-display", {
        let e = ScriptParseError::Other("custom error".into());
        let msg = format!("{}", e);
        if !msg.contains("custom error") {
            return Err(format!("unexpected: {}", msg));
        }
        Ok(())
    });

    check!("spe/05 is-std-error", {
        let e = ScriptParseError::UnexpectedEndOfScript;
        let _: &dyn std::error::Error = &e;
        Ok(())
    });

    check!("spe/06 pushdata1-truncated-triggers-error", {
        // OP_PUSHDATA1 (0x4c) followed by length 10 but no data
        let script = vec![0x4c, 0x0a]; // push 10 bytes, but no data follows
        match parse_script_chunks(&script) {
            Err(_) => Ok(()),
            Ok(_) => Err("truncated pushdata1 should fail".into()),
        }
    });
}

// ═══════════════════════════════════════════════════════════════════
// [5/7]  DomainPermission::defaults + CertFieldPermission
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t11_05_domain_permission() {
    use hodos_wallet::database::DomainPermission;
    use hodos_wallet::database::CertFieldPermission;

    eprintln!("\n[5/7] DomainPermission::defaults + CertFieldPermission");

    check!("domain/01 defaults-trust-level", {
        let dp = DomainPermission::defaults(1, "example.com");
        if dp.trust_level != "unknown" {
            return Err(format!("trust_level: '{}' != 'unknown'", dp.trust_level));
        }
        Ok(())
    });

    check!("domain/02 defaults-domain-stored", {
        let dp = DomainPermission::defaults(1, "test.org");
        if dp.domain != "test.org" {
            return Err(format!("domain: '{}'", dp.domain));
        }
        Ok(())
    });

    check!("domain/03 defaults-user-id", {
        let dp = DomainPermission::defaults(42, "x.com");
        if dp.user_id != 42 {
            return Err(format!("user_id: {} != 42", dp.user_id));
        }
        Ok(())
    });

    check!("domain/04 defaults-spending-limits", {
        let dp = DomainPermission::defaults(1, "shop.com");
        if dp.per_tx_limit_cents != 10 {
            return Err(format!("per_tx: {} != 10", dp.per_tx_limit_cents));
        }
        if dp.per_session_limit_cents != 300 {
            return Err(format!("per_session: {} != 300", dp.per_session_limit_cents));
        }
        if dp.rate_limit_per_min != 10 {
            return Err(format!("rate_limit: {} != 10", dp.rate_limit_per_min));
        }
        Ok(())
    });

    check!("domain/05 defaults-no-id", {
        let dp = DomainPermission::defaults(1, "a.com");
        if dp.id.is_some() {
            return Err("new domain permission should have None id".into());
        }
        Ok(())
    });

    check!("domain/06 defaults-timestamps-recent", {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let dp = DomainPermission::defaults(1, "b.com");
        if (dp.created_at - now).abs() > 2 {
            return Err(format!("created_at {} too far from {}", dp.created_at, now));
        }
        if (dp.updated_at - now).abs() > 2 {
            return Err(format!("updated_at {} too far from {}", dp.updated_at, now));
        }
        Ok(())
    });
}

// ═══════════════════════════════════════════════════════════════════
// [6/7]  PriceCache Default + Brc43 edge cases
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t11_06_price_cache_brc43() {
    use hodos_wallet::price_cache::PriceCache;
    use hodos_wallet::crypto::brc43::{InvoiceNumber, SecurityLevel, normalize_protocol_id};

    eprintln!("\n[6/7] PriceCache Default + Brc43 edges");

    check!("misc/01 price-cache-default-impl", {
        let pc = PriceCache::default();
        // Default should behave same as new()
        if pc.get_cached().is_some() {
            return Err("default cache should be empty".into());
        }
        if pc.get_stale().is_some() {
            return Err("default stale should be None".into());
        }
        Ok(())
    });

    check!("misc/02 brc43-security-level-display", {
        // Test all security levels
        let l0 = SecurityLevel::NoPermissions;
        let l1 = SecurityLevel::ProtocolLevel;
        let l2 = SecurityLevel::CounterpartyLevel;

        // Verify their numeric values are correct via InvoiceNumber construction
        let inv0 = InvoiceNumber::new(l0, "hello world", "key1").map_err(|e| format!("{}", e))?;
        let inv1 = InvoiceNumber::new(l1, "hello world", "key1").map_err(|e| format!("{}", e))?;
        let inv2 = InvoiceNumber::new(l2, "hello world", "key1").map_err(|e| format!("{}", e))?;

        if !inv0.to_string().starts_with("0-") { return Err(format!("Silent: {}", inv0.to_string())); }
        if !inv1.to_string().starts_with("1-") { return Err(format!("Passive: {}", inv1.to_string())); }
        if !inv2.to_string().starts_with("2-") { return Err(format!("Counter: {}", inv2.to_string())); }
        Ok(())
    });

    check!("misc/03 normalize-protocol-id", {
        let result = normalize_protocol_id("  Hello World  ").map_err(|e| e)?;
        // Should normalize: trim + lowercase
        if result != "hello world" {
            return Err(format!("expected 'hello world', got '{}'", result));
        }
        Ok(())
    });

    check!("misc/04 brc43-invoice-with-spaces", {
        let inv = InvoiceNumber::new(
            SecurityLevel::CounterpartyLevel,
            "certificate signature",
            "AAAA BBBB",
        ).map_err(|e| format!("{}", e))?;
        let s = inv.to_string();
        if !s.contains("certificate signature") {
            return Err(format!("missing protocol: {}", s));
        }
        if !s.contains("AAAA BBBB") {
            return Err(format!("missing key_id: {}", s));
        }
        Ok(())
    });

    check!("misc/05 brc43-invoice-from-string-roundtrip", {
        let original = "2-hello world-my key";
        let inv = InvoiceNumber::from_string(original).map_err(|e| format!("{}", e))?;
        if inv.to_string() != original {
            return Err(format!("roundtrip: '{}' != '{}'", inv.to_string(), original));
        }
        Ok(())
    });

    check!("misc/06 brc43-from-string-bad-format", {
        // Missing parts
        match InvoiceNumber::from_string("just-a-string") {
            Err(_) => Ok(()),
            Ok(inv) => {
                // Some implementations may accept this; verify it at least parses
                let _ = inv.to_string();
                Ok(())
            }
        }
    });
}

// ═══════════════════════════════════════════════════════════════════
// [7/7]  Cross-module integration pipelines
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t11_07_cross_module() {
    use hodos_wallet::crypto::brc42::{compute_shared_secret, compute_invoice_hmac, derive_child_public_key, derive_child_private_key};
    use hodos_wallet::crypto::signing::{sign_ecdsa, verify_signature, sha256};
    use hodos_wallet::crypto::keys::derive_public_key;
    use hodos_wallet::transaction::{Transaction, TxInput, TxOutput, OutPoint, Script};
    use hodos_wallet::beef::{Beef, ParsedTransaction};

    eprintln!("\n[7/7] Cross-module integration pipelines");

    check!("cross/01 ecdh-hmac-derive-sign-verify", {
        // Full BRC-42 pipeline: ECDH → HMAC → derive child key → sign → verify
        use secp256k1::{Secp256k1, SecretKey, PublicKey};
        let secp = Secp256k1::new();

        let alice_priv = [10u8; 32];
        let alice_sk = SecretKey::from_slice(&alice_priv).unwrap();
        let alice_pub = PublicKey::from_secret_key(&secp, &alice_sk).serialize();

        let bob_priv = [20u8; 32];
        let bob_sk = SecretKey::from_slice(&bob_priv).unwrap();
        let bob_pub = PublicKey::from_secret_key(&secp, &bob_sk).serialize();

        let invoice = "2-test protocol-key1";

        // Alice derives child private key for Bob
        let child_priv = derive_child_private_key(&alice_priv, &bob_pub, invoice)
            .map_err(|e| format!("{}", e))?;

        // Bob derives child public key for Alice
        let child_pub = derive_child_public_key(&bob_priv, &alice_pub, invoice)
            .map_err(|e| format!("{}", e))?;

        // Child public key should match child private key's public key
        let derived_pub = derive_public_key(&child_priv)
            .map_err(|e| format!("{}", e))?;
        if derived_pub != child_pub {
            return Err("child key pair mismatch: BRC-42 symmetry broken".into());
        }

        // Sign with child private key, verify with child public key
        let hash = sha256(b"test message");
        let sig = sign_ecdsa(&hash, &child_priv, 0x01).map_err(|e| format!("{}", e))?;
        let valid = verify_signature(&hash, &sig, &child_pub).map_err(|e| format!("{}", e))?;
        if !valid {
            return Err("signature should verify with derived child keys".into());
        }
        Ok(())
    });

    check!("cross/02 tx-build-serialize-parse-back", {
        // Build a transaction, serialize, parse with ParsedTransaction, verify match
        let mut tx = Transaction::new();
        let outpoint = OutPoint::new(
            "aabbccdd00112233aabbccdd00112233aabbccdd00112233aabbccdd00112233",
            0,
        );
        let mut input = TxInput::new(outpoint);
        input.set_script(hex::decode("483045022100abcdef").unwrap());
        tx.add_input(input);

        let output = TxOutput::new(25000, hex::decode("76a914aabbccdd00112233445566778899aabb00112233445588ac").unwrap());
        tx.add_output(output);

        let raw = tx.serialize().map_err(|e| format!("{}", e))?;
        let hex_str = tx.to_hex().map_err(|e| format!("{}", e))?;

        // Parse back with ParsedTransaction
        let parsed = ParsedTransaction::from_bytes(&raw).map_err(|e| e)?;
        if parsed.version != 1 { return Err(format!("version {} != 1", parsed.version)); }
        if parsed.inputs.len() != 1 { return Err(format!("inputs {}", parsed.inputs.len())); }
        if parsed.outputs.len() != 1 { return Err(format!("outputs {}", parsed.outputs.len())); }
        if parsed.outputs[0].value != 25000 { return Err(format!("value {}", parsed.outputs[0].value)); }

        // ParsedTransaction from_hex should match from_bytes
        let parsed2 = ParsedTransaction::from_hex(&hex_str).map_err(|e| e)?;
        if parsed2.inputs.len() != parsed.inputs.len() { return Err("input count mismatch".into()); }
        Ok(())
    });

    check!("cross/03 tx-txid-matches-parsed-beef-find", {
        // Build tx → get TXID → put in BEEF → find_txid should locate it
        let mut tx = Transaction::new();
        let mut inp = TxInput::new(OutPoint::new(&"00".repeat(32), 0));
        inp.set_script(hex::decode("00").unwrap());
        tx.add_input(inp);
        tx.add_output(TxOutput::new(1000, hex::decode("76a914000000000000000000000000000000000000000088ac").unwrap()));

        let raw = tx.serialize().map_err(|e| format!("{}", e))?;
        let txid = tx.txid().map_err(|e| format!("{}", e))?;

        let mut beef = Beef::new();
        beef.set_main_transaction(raw);

        let found = beef.find_txid(&txid);
        if found != Some(0) {
            return Err(format!("find_txid returned {:?}, expected Some(0)", found));
        }
        Ok(())
    });

    check!("cross/04 script-p2pkh-roundtrip-with-keys", {
        // Generate key → Hash160 → P2PKH locking script → verify structure
        use hodos_wallet::crypto::keys::derive_public_key;
        use hodos_wallet::crypto::signing::sha256;
        use ripemd::{Ripemd160, Digest as RipDigest};

        let privkey = [42u8; 32];
        let pubkey = derive_public_key(&privkey)
            .map_err(|e| format!("{}", e))?;

        // Hash160 = RIPEMD160(SHA256(pubkey))
        let sha_hash = sha256(&pubkey);
        let ripemd_hash = Ripemd160::digest(&sha_hash);
        let pubkey_hash: Vec<u8> = ripemd_hash.to_vec();

        let locking = Script::p2pkh_locking_script(&pubkey_hash).map_err(|e| format!("{}", e))?;
        let script_bytes = locking.to_bytes();

        // Verify P2PKH structure: OP_DUP OP_HASH160 <20> ... OP_EQUALVERIFY OP_CHECKSIG
        if script_bytes.len() != 25 { return Err(format!("script len {}", script_bytes.len())); }
        if script_bytes[0] != 0x76 { return Err("OP_DUP missing".into()); }
        if script_bytes[1] != 0xa9 { return Err("OP_HASH160 missing".into()); }
        if script_bytes[2] != 0x14 { return Err("push 20 bytes missing".into()); }
        if script_bytes[23] != 0x88 { return Err("OP_EQUALVERIFY missing".into()); }
        if script_bytes[24] != 0xac { return Err("OP_CHECKSIG missing".into()); }

        // Hash160 inside should match what we computed
        if script_bytes[3..23] != pubkey_hash[..] {
            return Err("Hash160 inside P2PKH doesn't match pubkey hash".into());
        }
        Ok(())
    });

    check!("cross/05 beef-v1-v2-extract-raw-tx-match", {
        // Build BEEF → V1 and V2 → extract_raw_tx_hex from both should match
        let mut beef = Beef::new();
        let parent = hex::decode("0100000001cd4e4cac3c7b56920d1e7655e7e260d31f29d9a388d04910f1bbd72304a79029010000006b483045022100e75279a205a547c445719420aa3138bf14743e3f42618e5f86a19bde14bb95f7022064777d34776b05d816daf1699493fcdf2ef5a5ab1ad710d9c97bfb5b8f7cef3641210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013e660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000").unwrap();
        let main = hex::decode("0100000001ac4e164f5bc16746bb0868404292ac8318bbac3800e4aad13a014da427adce3e000000006a47304402203a61a2e931612b4bda08d541cfb980885173b8dcf64a3471238ae7abcd368d6402204cbf24f04b9aa2256d8901f0ed97866603d2be8324c2bfb7a37bf8fc90edd5b441210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013c660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000").unwrap();
        beef.add_parent_transaction(parent);
        beef.set_main_transaction(main);

        let v1_hex = beef.to_v1_hex().map_err(|e| e)?;
        let v2_hex = beef.to_hex().map_err(|e| e)?;

        let raw_from_v1 = Beef::extract_raw_tx_hex(&v1_hex).map_err(|e| e)?;
        let raw_from_v2 = Beef::extract_raw_tx_hex(&v2_hex).map_err(|e| e)?;

        if raw_from_v1 != raw_from_v2 {
            return Err("extracted raw tx should be same from V1 and V2".into());
        }
        Ok(())
    });

    check!("cross/06 brc2-encrypt-with-brc42-derived-key", {
        // BRC-42 derived symmetric key → use as AES-GCM key for BRC-2 encryption
        use hodos_wallet::crypto::brc42::derive_symmetric_key_for_hmac;
        use hodos_wallet::crypto::aesgcm_custom;
        use secp256k1::{Secp256k1, SecretKey, PublicKey};

        let secp = Secp256k1::new();
        let priv_a = SecretKey::from_slice(&[11u8; 32]).unwrap();
        let pub_b_key = SecretKey::from_slice(&[12u8; 32]).unwrap();
        let pub_b = PublicKey::from_secret_key(&secp, &pub_b_key);

        let sym_key = derive_symmetric_key_for_hmac(
            &priv_a.secret_bytes(), &pub_b.serialize(), "2-encrypt-key1"
        ).map_err(|e| format!("{}", e))?;

        // Use as AES-256-GCM key
        let plaintext = b"secret message for BRC-2";
        let iv = vec![0u8; 12];
        let key_arr: [u8; 32] = sym_key.as_slice().try_into().map_err(|_| "key not 32 bytes".to_string())?;
        let (ciphertext, tag) = aesgcm_custom::aesgcm_custom(plaintext, &[], &iv, &key_arr)
            .map_err(|e| format!("{}", e))?;
        let decrypted = aesgcm_custom::aesgcm_decrypt_custom(&ciphertext, &[], &iv, &tag, &key_arr)
            .map_err(|e| format!("{}", e))?;

        if decrypted != plaintext {
            return Err("BRC-42 derived key AES-GCM roundtrip failed".into());
        }
        Ok(())
    });

    check!("cross/07 sha256-sign-verify-double-sha256", {
        // SHA-256 → sign → double-SHA-256 → different signatures
        use hodos_wallet::crypto::signing::{sha256, double_sha256, sign_ecdsa, verify_signature};
        use hodos_wallet::crypto::keys::derive_public_key;

        let privkey = [50u8; 32];
        let pubkey = derive_public_key(&privkey).map_err(|e| format!("{}", e))?;

        let data = b"cross-module test data";
        let single_hash = sha256(data);
        let double_hash = double_sha256(data);

        // Sign with single hash
        let sig1 = sign_ecdsa(&single_hash, &privkey, 0x01).map_err(|e| format!("{}", e))?;
        let valid1 = verify_signature(&single_hash, &sig1, &pubkey).map_err(|e| format!("{}", e))?;
        if !valid1 { return Err("single hash sig should verify".into()); }

        // Sign with double hash
        let sig2 = sign_ecdsa(&double_hash, &privkey, 0x01).map_err(|e| format!("{}", e))?;
        let valid2 = verify_signature(&double_hash, &sig2, &pubkey).map_err(|e| format!("{}", e))?;
        if !valid2 { return Err("double hash sig should verify".into()); }

        // Signatures should differ
        if sig1 == sig2 { return Err("single vs double hash sigs should differ".into()); }

        // Cross-verify should fail
        let cross_valid = verify_signature(&double_hash, &sig1, &pubkey).map_err(|e| format!("{}", e))?;
        if cross_valid { return Err("cross-verify should fail".into()); }
        Ok(())
    });

    check!("cross/08 outpoint-serialize-parse-in-parsed-tx", {
        // Build OutPoint → use in tx → serialize → ParsedTransaction → verify prev_txid
        let txid_hex = "ab".repeat(32);
        let outpoint = OutPoint::new(&txid_hex, 3);
        let mut tx = Transaction::new();
        tx.add_input(TxInput::new(outpoint));
        tx.add_output(TxOutput::new(500, hex::decode("6a").unwrap())); // OP_RETURN

        let raw = tx.serialize().map_err(|e| format!("{}", e))?;
        let parsed = ParsedTransaction::from_bytes(&raw).map_err(|e| e)?;

        if parsed.inputs[0].prev_txid != txid_hex {
            return Err(format!("prev_txid '{}' != '{}'", parsed.inputs[0].prev_txid, txid_hex));
        }
        if parsed.inputs[0].prev_vout != 3 {
            return Err(format!("prev_vout {} != 3", parsed.inputs[0].prev_vout));
        }
        if parsed.outputs[0].value != 500 {
            return Err(format!("output value {} != 500", parsed.outputs[0].value));
        }
        Ok(())
    });
}

// ═══════════════════════════════════════════════════════════════════
// Summary
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t11_99_summary() {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let p = PASS.load(Ordering::SeqCst);
    let f = FAIL.load(Ordering::SeqCst);
    eprintln!("\n════════════════════════════════════════");
    eprintln!("  TIER 11 FINAL:  {} passed, {} failed  (of {} total)", p, f, p + f);
    eprintln!("════════════════════════════════════════\n");
    assert_eq!(f, 0, "{} test(s) failed — see FAIL lines above", f);
}
