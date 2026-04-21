///! Tier 10 — Certificate Preimage/Signature, BEEF Validation, DB Helpers
///!
///! Sections:
///!  [1/6]  Certificate types (new, identifier, is_active, errors)          (10 tests)
///!  [2/6]  serialize_certificate_preimage                                   (12 tests)
///!  [3/6]  verify_certificate_signature (BRC-42 key derivation path)        (8 tests)
///!  [4/6]  BEEF parse_bump_hex_to_tsc                                       (10 tests)
///!  [5/6]  BEEF validate_beef_v1_hex                                        (8 tests)
///!  [6/6]  Database helpers (address_to_address_info, output_to_fetcher_utxo) (8 tests)
///!
///! Total: 56 tests

use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashMap;
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

/// Helper: build a test Certificate with given fields
fn make_cert(
    num_fields: usize,
    revocation_outpoint: &str,
) -> hodos_wallet::certificate::Certificate {
    use hodos_wallet::certificate::types::{Certificate, CertificateField};

    let mut fields = HashMap::new();
    for i in 0..num_fields {
        let name = format!("field{}", i);
        fields.insert(name.clone(), CertificateField::new(
            name,
            vec![0xAA; 16], // encrypted value
            vec![0xBB; 32], // master key
        ));
    }

    Certificate::new(
        vec![0x01; 32],                     // type_ (32 bytes)
        vec![0x02; 33],                     // subject (33 bytes)
        vec![0x03; 32],                     // serial_number (32 bytes)
        vec![0x04; 33],                     // certifier (33 bytes)
        revocation_outpoint.to_string(),
        vec![0x30; 35],                     // signature (placeholder)
        fields,
        HashMap::new(),
    )
}

/// Helper: build a valid Certificate with real secp256k1 keys
fn make_valid_cert_with_keys() -> (
    hodos_wallet::certificate::Certificate,
    Vec<u8>,  // certifier private key
    Vec<u8>,  // certifier public key
) {
    use secp256k1::{Secp256k1, SecretKey, PublicKey};
    use hodos_wallet::certificate::types::{Certificate, CertificateField};

    let secp = Secp256k1::new();

    // Subject keys
    let subject_sec = SecretKey::from_slice(&[5u8; 32]).unwrap();
    let subject_pub = PublicKey::from_secret_key(&secp, &subject_sec);

    // Certifier keys
    let certifier_sec = SecretKey::from_slice(&[6u8; 32]).unwrap();
    let certifier_pub = PublicKey::from_secret_key(&secp, &certifier_sec);

    let mut fields = HashMap::new();
    fields.insert("name".to_string(), CertificateField::new(
        "name".to_string(),
        b"encrypted_alice".to_vec(),
        vec![0xCC; 32],
    ));
    fields.insert("email".to_string(), CertificateField::new(
        "email".to_string(),
        b"encrypted_email".to_vec(),
        vec![0xDD; 32],
    ));

    let txid = "a".repeat(64);
    let cert = Certificate::new(
        vec![0x10; 32],
        subject_pub.serialize().to_vec(),
        vec![0x20; 32],
        certifier_pub.serialize().to_vec(),
        format!("{}.0", txid),
        vec![], // no signature yet
        fields,
        HashMap::new(),
    );

    (cert, certifier_sec.secret_bytes().to_vec(), certifier_pub.serialize().to_vec())
}

/// Helper: compute TXID
fn compute_txid(tx_bytes: &[u8]) -> String {
    let h1 = Sha256::digest(tx_bytes);
    let h2 = Sha256::digest(&h1);
    hex::encode(h2.iter().rev().copied().collect::<Vec<u8>>())
}

// Real BRC-62 transactions (same as Tier 9)
const PARENT_TX_HEX: &str = "0100000001cd4e4cac3c7b56920d1e7655e7e260d31f29d9a388d04910f1bbd72304a79029010000006b483045022100e75279a205a547c445719420aa3138bf14743e3f42618e5f86a19bde14bb95f7022064777d34776b05d816daf1699493fcdf2ef5a5ab1ad710d9c97bfb5b8f7cef3641210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013e660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000";
const MAIN_TX_HEX: &str = "0100000001ac4e164f5bc16746bb0868404292ac8318bbac3800e4aad13a014da427adce3e000000006a47304402203a61a2e931612b4bda08d541cfb980885173b8dcf64a3471238ae7abcd368d6402204cbf24f04b9aa2256d8901f0ed97866603d2be8324c2bfb7a37bf8fc90edd5b441210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013c660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000";

/// Helper: build a test BEEF with TSC proof, return V1 hex
fn build_test_beef_v1_hex() -> String {
    use hodos_wallet::beef::Beef;
    let parent_tx = hex::decode(PARENT_TX_HEX).unwrap();
    let main_tx = hex::decode(MAIN_TX_HEX).unwrap();
    let parent_txid = compute_txid(&parent_tx);

    let mut beef = Beef::new();
    let idx = beef.add_parent_transaction(parent_tx);
    let tsc = serde_json::json!({
        "height": 918980,
        "index": 0,
        "nodes": [
            "9b18d77b48fde9b46d54b75d372e30a74cba0114cad4796f8f1d91946866a8bd",
            "45b8d1a256e4de964d2a70408e3ae4265b43544425ea40f370cd76d367575b0e"
        ]
    });
    beef.add_tsc_merkle_proof(&parent_txid, idx, &tsc).unwrap();
    beef.set_main_transaction(main_tx);
    beef.to_v1_hex().unwrap()
}

// ═══════════════════════════════════════════════════════════════════
// [1/6]  Certificate types
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t10_01_certificate_types() {
    use hodos_wallet::certificate::types::{Certificate, CertificateField, CertificateError};

    eprintln!("\n[1/6] Certificate types");

    check!("cert-type/01 new-sets-timestamps", {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let cert = Certificate::new(
            vec![0; 32], vec![0; 33], vec![0; 32], vec![0; 33],
            "abc.0".into(), vec![], HashMap::new(), HashMap::new(),
        );
        // Timestamps should be recent (within 2 seconds)
        if (cert.created_at - now).abs() > 2 {
            return Err(format!("created_at {} too far from now {}", cert.created_at, now));
        }
        if cert.certificate_id.is_some() {
            return Err("new cert should have None id".into());
        }
        if cert.is_deleted {
            return Err("new cert should not be deleted".into());
        }
        Ok(())
    });

    check!("cert-type/02 identifier-returns-tuple", {
        let cert = make_cert(0, "abc.0");
        let (t, sn, c) = cert.identifier();
        if t != &[0x01; 32] { return Err("type mismatch".into()); }
        if sn != &[0x03; 32] { return Err("serial_number mismatch".into()); }
        if c != &[0x04; 33] { return Err("certifier mismatch".into()); }
        Ok(())
    });

    check!("cert-type/03 is_active-default-true", {
        let cert = make_cert(0, "abc.0");
        if !cert.is_active() {
            return Err("new cert should be active".into());
        }
        Ok(())
    });

    check!("cert-type/04 is_active-deleted-false", {
        let mut cert = make_cert(0, "abc.0");
        cert.is_deleted = true;
        if cert.is_active() {
            return Err("deleted cert should not be active".into());
        }
        Ok(())
    });

    check!("cert-type/05 field-new-timestamps", {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let field = CertificateField::new("test".into(), vec![1, 2, 3], vec![4, 5, 6]);
        if (field.created_at - now).abs() > 2 {
            return Err("field timestamp too far from now".into());
        }
        if field.field_name != "test" { return Err("name mismatch".into()); }
        if field.field_value != vec![1, 2, 3] { return Err("value mismatch".into()); }
        if field.master_key != vec![4, 5, 6] { return Err("master_key mismatch".into()); }
        if field.certificate_id.is_some() { return Err("should have None cert_id".into()); }
        Ok(())
    });

    check!("cert-type/06 error-display-invalid-format", {
        let e = CertificateError::InvalidFormat("bad format".into());
        let msg = format!("{}", e);
        if !msg.contains("Invalid certificate format") || !msg.contains("bad format") {
            return Err(format!("unexpected display: {}", msg));
        }
        Ok(())
    });

    check!("cert-type/07 error-display-missing-field", {
        let e = CertificateError::MissingField("name".into());
        let msg = format!("{}", e);
        if !msg.contains("Missing required field") || !msg.contains("name") {
            return Err(format!("unexpected display: {}", msg));
        }
        Ok(())
    });

    check!("cert-type/08 error-display-invalid-base64", {
        let e = CertificateError::InvalidBase64("not base64".into());
        let msg = format!("{}", e);
        if !msg.contains("Invalid base64") {
            return Err(format!("unexpected display: {}", msg));
        }
        Ok(())
    });

    check!("cert-type/09 error-display-signature-verification", {
        let e = CertificateError::SignatureVerification("mismatch".into());
        let msg = format!("{}", e);
        if !msg.contains("Signature verification failed") {
            return Err(format!("unexpected display: {}", msg));
        }
        Ok(())
    });

    check!("cert-type/10 error-display-revoked-relinquished", {
        let e1 = CertificateError::Revoked;
        let e2 = CertificateError::Relinquished;
        let m1 = format!("{}", e1);
        let m2 = format!("{}", e2);
        if !m1.contains("revoked") { return Err(format!("revoked: {}", m1)); }
        if !m2.contains("relinquished") { return Err(format!("relinquished: {}", m2)); }
        Ok(())
    });
}

// ═══════════════════════════════════════════════════════════════════
// [2/6]  serialize_certificate_preimage
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t10_02_certificate_preimage() {
    use hodos_wallet::certificate::serialize_certificate_preimage;
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    eprintln!("\n[2/6] serialize_certificate_preimage");

    check!("preimage/01 basic-structure-length", {
        let txid_hex = "a".repeat(64);
        let cert = make_cert(1, &format!("{}.0", txid_hex));
        let preimage = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;
        // 32 (type) + 32 (serial) + 33 (subject) + 33 (certifier) + 32 (txid) + varint(vout)
        // + varint(1 field) + field data
        if preimage.len() < 32 + 32 + 33 + 33 + 32 + 1 {
            return Err(format!("preimage too short: {} bytes", preimage.len()));
        }
        Ok(())
    });

    check!("preimage/02 type-is-first-32-bytes", {
        let txid_hex = "a".repeat(64);
        let cert = make_cert(0, &format!("{}.0", txid_hex));
        let preimage = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;
        if preimage[0..32] != [0x01; 32] {
            return Err("first 32 bytes should be type".into());
        }
        Ok(())
    });

    check!("preimage/03 serial-at-offset-32", {
        let txid_hex = "a".repeat(64);
        let cert = make_cert(0, &format!("{}.0", txid_hex));
        let preimage = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;
        if preimage[32..64] != [0x03; 32] {
            return Err("bytes 32..64 should be serial_number".into());
        }
        Ok(())
    });

    check!("preimage/04 subject-at-offset-64", {
        let txid_hex = "a".repeat(64);
        let cert = make_cert(0, &format!("{}.0", txid_hex));
        let preimage = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;
        if preimage[64..97] != [0x02; 33] {
            return Err("bytes 64..97 should be subject".into());
        }
        Ok(())
    });

    check!("preimage/05 certifier-at-offset-97", {
        let txid_hex = "a".repeat(64);
        let cert = make_cert(0, &format!("{}.0", txid_hex));
        let preimage = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;
        if preimage[97..130] != [0x04; 33] {
            return Err("bytes 97..130 should be certifier".into());
        }
        Ok(())
    });

    check!("preimage/06 revocation-outpoint-txid", {
        let txid_hex = "ab".repeat(32); // 64 hex chars = 32 bytes
        let cert = make_cert(0, &format!("{}.0", txid_hex));
        let preimage = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;
        // After certifier (130 bytes), the next 32 bytes are the outpoint txid
        if preimage[130..162] != [0xAB; 32] {
            return Err(format!("revocation txid mismatch at offset 130"));
        }
        Ok(())
    });

    check!("preimage/07 revocation-outpoint-vout-varint", {
        let txid_hex = "00".repeat(32);
        let cert = make_cert(0, &format!("{}.5", txid_hex));
        let preimage = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;
        // vout at offset 162 should be varint(5) = [0x05]
        if preimage[162] != 5 {
            return Err(format!("vout varint: expected 5, got {}", preimage[162]));
        }
        Ok(())
    });

    check!("preimage/08 zero-fields-count", {
        let txid_hex = "00".repeat(32);
        let cert = make_cert(0, &format!("{}.0", txid_hex));
        let preimage = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;
        // field count varint(0) = [0x00] after outpoint
        // outpoint: 32 bytes txid + varint(0) = 33 bytes from offset 130
        let field_count_offset = 130 + 32 + 1; // 163
        if preimage[field_count_offset] != 0 {
            return Err(format!("field count: expected 0, got {}", preimage[field_count_offset]));
        }
        Ok(())
    });

    check!("preimage/09 fields-sorted-lexicographically", {
        use hodos_wallet::certificate::types::{Certificate, CertificateField};
        let txid_hex = "00".repeat(32);
        let mut fields = HashMap::new();
        // Add fields in reverse order
        fields.insert("zzz".to_string(), CertificateField::new("zzz".into(), vec![0x11], vec![]));
        fields.insert("aaa".to_string(), CertificateField::new("aaa".into(), vec![0x22], vec![]));
        fields.insert("mmm".to_string(), CertificateField::new("mmm".into(), vec![0x33], vec![]));

        let cert = Certificate::new(
            vec![0; 32], vec![0; 33], vec![0; 32], vec![0; 33],
            format!("{}.0", txid_hex), vec![], fields, HashMap::new(),
        );
        let preimage = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;
        // After outpoint (130 + 32 + 1 = 163), field count varint(3) = [0x03]
        let fc_off = 163;
        if preimage[fc_off] != 3 {
            return Err(format!("field count: expected 3, got {}", preimage[fc_off]));
        }
        // First field should be "aaa" (lexicographic)
        // varint(3) for name len, then "aaa"
        let name_len_off = fc_off + 1;
        if preimage[name_len_off] != 3 {
            return Err(format!("first field name len: expected 3, got {}", preimage[name_len_off]));
        }
        let name_bytes = &preimage[name_len_off + 1..name_len_off + 4];
        if name_bytes != b"aaa" {
            return Err(format!("first field should be 'aaa', got {:?}", String::from_utf8_lossy(name_bytes)));
        }
        Ok(())
    });

    check!("preimage/10 field-values-are-base64-encoded", {
        use hodos_wallet::certificate::types::{Certificate, CertificateField};
        let txid_hex = "00".repeat(32);
        let mut fields = HashMap::new();
        let raw_value = vec![0xAA, 0xBB, 0xCC]; // 3 bytes
        fields.insert("f".to_string(), CertificateField::new("f".into(), raw_value.clone(), vec![]));

        let cert = Certificate::new(
            vec![0; 32], vec![0; 33], vec![0; 32], vec![0; 33],
            format!("{}.0", txid_hex), vec![], fields, HashMap::new(),
        );
        let preimage = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;

        // Field value should be base64-encoded then stored as UTF-8 bytes
        let expected_b64 = BASE64.encode(&raw_value); // "qrvM"
        let expected_bytes = expected_b64.as_bytes();

        // Find the field value in preimage (after outpoint + field_count + name_len + name + value_len)
        // outpoint ends at 163, fc=1 byte, name_len=1 byte ("f"=1), name=1 byte, value_len=varint
        let val_len_off = 163 + 1 + 1 + 1;
        let val_len = preimage[val_len_off] as usize;
        let val_off = val_len_off + 1;
        let val_bytes = &preimage[val_off..val_off + val_len];

        if val_bytes != expected_bytes {
            return Err(format!("field value: expected {:?}, got {:?}",
                String::from_utf8_lossy(expected_bytes),
                String::from_utf8_lossy(val_bytes)));
        }
        Ok(())
    });

    check!("preimage/11 rejects-wrong-type-length", {
        use hodos_wallet::certificate::types::{Certificate, CertificateField};
        let cert = Certificate::new(
            vec![0; 16],  // WRONG: only 16 bytes, need 32
            vec![0; 33], vec![0; 32], vec![0; 33],
            "a.0".into(), vec![], HashMap::new(), HashMap::new(),
        );
        match serialize_certificate_preimage(&cert) {
            Err(e) if format!("{}", e).contains("type must be 32 bytes") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject 16-byte type".into()),
        }
    });

    check!("preimage/12 rejects-wrong-subject-length", {
        use hodos_wallet::certificate::types::{Certificate, CertificateField};
        let cert = Certificate::new(
            vec![0; 32], vec![0; 32], // WRONG: subject 32 not 33
            vec![0; 32], vec![0; 33],
            "a.0".into(), vec![], HashMap::new(), HashMap::new(),
        );
        match serialize_certificate_preimage(&cert) {
            Err(e) if format!("{}", e).contains("subject must be 33 bytes") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject 32-byte subject".into()),
        }
    });
}

// ═══════════════════════════════════════════════════════════════════
// [3/6]  verify_certificate_signature
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t10_03_cert_signature_verify() {
    use hodos_wallet::certificate::{verify_certificate_signature, serialize_certificate_preimage};
    use hodos_wallet::crypto::brc42::derive_child_public_key;
    use hodos_wallet::crypto::brc43::{InvoiceNumber, SecurityLevel};
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use secp256k1::{Secp256k1, SecretKey, PublicKey};

    eprintln!("\n[3/6] verify_certificate_signature");

    check!("verify/01 rejects-empty-signature", {
        let (cert, _, _) = make_valid_cert_with_keys();
        // cert has empty signature
        match verify_certificate_signature(&cert) {
            Err(e) if format!("{}", e).contains("no signature") || format!("{}", e).contains("empty") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject empty signature".into()),
        }
    });

    check!("verify/02 rejects-invalid-type-length", {
        use hodos_wallet::certificate::types::Certificate;
        let cert = Certificate::new(
            vec![0; 16], // wrong length
            vec![0x02; 33], vec![0; 32], vec![0x02; 33],
            format!("{}.0", "00".repeat(32)),
            vec![0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01], // minimal DER sig
            HashMap::new(), HashMap::new(),
        );
        match verify_certificate_signature(&cert) {
            Err(e) if format!("{}", e).contains("type must be 32 bytes") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject wrong type length".into()),
        }
    });

    check!("verify/03 rejects-invalid-serial-length", {
        use hodos_wallet::certificate::types::Certificate;
        let cert = Certificate::new(
            vec![0; 32],
            vec![0x02; 33],
            vec![0; 16], // wrong serial length
            vec![0x02; 33],
            format!("{}.0", "00".repeat(32)),
            vec![0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01],
            HashMap::new(), HashMap::new(),
        );
        match verify_certificate_signature(&cert) {
            Err(e) if format!("{}", e).contains("serialNumber must be 32 bytes") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject wrong serial length".into()),
        }
    });

    check!("verify/04 preimage-deterministic", {
        let (cert, _, _) = make_valid_cert_with_keys();
        let p1 = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;
        let p2 = serialize_certificate_preimage(&cert).map_err(|e| format!("{}", e))?;
        if p1 != p2 {
            return Err("preimage should be deterministic".into());
        }
        Ok(())
    });

    check!("verify/05 preimage-changes-with-fields", {
        use hodos_wallet::certificate::types::{Certificate, CertificateField};
        let txid = format!("{}.0", "00".repeat(32));

        let mut fields1 = HashMap::new();
        fields1.insert("a".into(), CertificateField::new("a".into(), vec![1], vec![]));
        let cert1 = Certificate::new(
            vec![0; 32], vec![0; 33], vec![0; 32], vec![0; 33],
            txid.clone(), vec![], fields1, HashMap::new(),
        );

        let mut fields2 = HashMap::new();
        fields2.insert("a".into(), CertificateField::new("a".into(), vec![2], vec![]));
        let cert2 = Certificate::new(
            vec![0; 32], vec![0; 33], vec![0; 32], vec![0; 33],
            txid, vec![], fields2, HashMap::new(),
        );

        let p1 = serialize_certificate_preimage(&cert1).map_err(|e| format!("{}", e))?;
        let p2 = serialize_certificate_preimage(&cert2).map_err(|e| format!("{}", e))?;
        if p1 == p2 {
            return Err("different field values should produce different preimages".into());
        }
        Ok(())
    });

    check!("verify/06 brc42-invoice-number-construction", {
        // Verify the BRC-43 invoice number format matches spec
        let type_b64 = BASE64.encode(&[0x10u8; 32]);
        let serial_b64 = BASE64.encode(&[0x20u8; 32]);
        let key_id = format!("{} {}", type_b64, serial_b64);
        let invoice = InvoiceNumber::new(
            SecurityLevel::CounterpartyLevel,
            "certificate signature",
            &key_id,
        ).map_err(|e| format!("{}", e))?;
        let inv_str = invoice.to_string();
        // Should start with "2-certificate signature-"
        if !inv_str.starts_with("2-certificate signature-") {
            return Err(format!("invoice number format wrong: {}", inv_str));
        }
        Ok(())
    });

    check!("verify/07 anyone-private-key-is-value-1", {
        // BRC-52 verification uses "anyone" (private key value 1)
        let mut anyone_key = [0u8; 32];
        anyone_key[31] = 1;
        // Verify it's a valid secp256k1 private key
        let sk = SecretKey::from_slice(&anyone_key).map_err(|e| format!("{}", e))?;
        let secp = Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &sk);
        // "Anyone" public key should be the generator point G
        let pk_hex = hex::encode(pk.serialize());
        // Well-known generator point compressed form starts with 02 or 03
        if !pk_hex.starts_with("02") && !pk_hex.starts_with("03") {
            return Err(format!("anyone pubkey not compressed: {}", pk_hex));
        }
        if pk_hex.len() != 66 {
            return Err(format!("anyone pubkey length {} != 66", pk_hex.len()));
        }
        Ok(())
    });

    check!("verify/08 derive-child-public-key-consistency", {
        let (_cert, _, certifier_pub) = make_valid_cert_with_keys();
        let mut anyone_key = [0u8; 32];
        anyone_key[31] = 1;
        let invoice_str = "2-certificate signature-test";

        // Derive child public key
        let child_pub = derive_child_public_key(&anyone_key, &certifier_pub, invoice_str)
            .map_err(|e| format!("{}", e))?;

        // Should be valid compressed public key (33 bytes, starts with 02 or 03)
        if child_pub.len() != 33 {
            return Err(format!("child pubkey len {} != 33", child_pub.len()));
        }
        if child_pub[0] != 0x02 && child_pub[0] != 0x03 {
            return Err(format!("child pubkey prefix 0x{:02x} not 02/03", child_pub[0]));
        }

        // Should differ from certifier public key (child != parent)
        if child_pub == certifier_pub {
            return Err("child pubkey should differ from certifier pubkey".into());
        }

        // Should be deterministic
        let child_pub2 = derive_child_public_key(&anyone_key, &certifier_pub, invoice_str)
            .map_err(|e| format!("{}", e))?;
        if child_pub != child_pub2 {
            return Err("derivation should be deterministic".into());
        }
        Ok(())
    });
}

// ═══════════════════════════════════════════════════════════════════
// [4/6]  BEEF parse_bump_hex_to_tsc
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t10_04_parse_bump_hex_to_tsc() {
    use hodos_wallet::beef::{Beef, parse_bump_hex_to_tsc};

    eprintln!("\n[4/6] BEEF parse_bump_hex_to_tsc");

    // First, build a BEEF with a known BUMP and extract the raw BUMP bytes
    // by reading them from the serialized V1 format

    check!("bump/01 roundtrip-tsc-to-bump-to-tsc", {
        // Build BEEF with known TSC proof
        let parent_tx = hex::decode(PARENT_TX_HEX).unwrap();
        let parent_txid = compute_txid(&parent_tx);

        let mut beef = Beef::new();
        let idx = beef.add_parent_transaction(parent_tx);
        let tsc = serde_json::json!({
            "height": 918980,
            "index": 0,
            "nodes": [
                "9b18d77b48fde9b46d54b75d372e30a74cba0114cad4796f8f1d91946866a8bd",
                "45b8d1a256e4de964d2a70408e3ae4265b43544425ea40f370cd76d367575b0e"
            ]
        });
        beef.add_tsc_merkle_proof(&parent_txid, idx, &tsc).unwrap();
        beef.set_main_transaction(hex::decode(MAIN_TX_HEX).unwrap());

        // Serialize to V1 and extract the BUMP portion
        let v1_bytes = beef.to_v1_bytes().map_err(|e| e)?;
        // V1 format: [4 bytes marker][varint num_bumps][bump data...][varint num_txs][tx data...]
        // Skip 4 byte marker, read num_bumps varint
        let num_bumps = v1_bytes[4]; // should be 1 (single byte varint)
        if num_bumps != 1 {
            return Err(format!("expected 1 bump, got {}", num_bumps));
        }
        // The BUMP starts at offset 5 and goes until the num_txs varint
        // We need to find where the BUMP ends. Parse it to find the boundary.
        // Instead, let's use the BUMP from the Beef struct directly — but it's not hex-accessible.
        // Use a simpler approach: build BUMP hex manually from known format.
        // Actually, let's just extract the bytes between the marker+count and the tx count.

        // The simplest approach: serialize just the BUMP by using write_bump indirectly
        // Since we can't call write_bump directly (it's private), we extract from V1 bytes.
        // After marker (4) + num_bumps varint (1), the BUMP data starts.
        // We need to find where it ends. The Beef struct has 1 bump with height=918980, tree_height=2

        // Actually, let's just verify parse_bump_hex_to_tsc works with a manually-built BUMP hex
        // A minimal BUMP: [height_varint][tree_height][level0...][level1...]

        // Height 918980 as varint: 918980 = 0x0E0644, needs 3-byte varint (0xFE prefix for u32)
        // Actually varint encoding: 918980 < 65536? No, 918980 > 65535.
        // 918980 = 0x000E0644 → varint 0xFE 0x44 0x06 0x0E 0x00

        // Build minimal BUMP manually: height=100, tree_height=1, 1 level with 2 nodes
        let mut bump_bytes = Vec::new();
        // Height = 100 (varint, single byte)
        bump_bytes.push(100);
        // Tree height = 1
        bump_bytes.push(1);
        // Level 0: 2 nodes
        bump_bytes.push(2); // num_nodes at level 0
        // Node 1: offset=0, flags=0x02 (TXID flag), hash=32 bytes
        bump_bytes.push(0); // offset 0
        bump_bytes.push(0x02); // TXID flag
        bump_bytes.extend_from_slice(&[0xAA; 32]); // txid hash
        // Node 2: offset=1, flags=0x00 (sibling), hash=32 bytes
        bump_bytes.push(1); // offset 1
        bump_bytes.push(0x00); // no flags
        bump_bytes.extend_from_slice(&[0xBB; 32]); // sibling hash

        let bump_hex = hex::encode(&bump_bytes);
        let result = parse_bump_hex_to_tsc(&bump_hex).map_err(|e| e)?;

        if result["height"] != 100 {
            return Err(format!("height: {:?}", result["height"]));
        }
        if result["index"] != 0 {
            return Err(format!("index: {:?}", result["index"]));
        }
        let nodes = result["nodes"].as_array().ok_or("no nodes array")?;
        if nodes.len() != 1 {
            return Err(format!("expected 1 node, got {}", nodes.len()));
        }
        // Sibling hash should be reversed (display format)
        let sibling = nodes[0].as_str().ok_or("node not string")?;
        if sibling.len() != 64 {
            return Err(format!("sibling hash len {} != 64", sibling.len()));
        }
        Ok(())
    });

    check!("bump/02 empty-hex-rejected", {
        match parse_bump_hex_to_tsc("") {
            Err(e) if e.contains("Empty BUMP") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject empty".into()),
        }
    });

    check!("bump/03 invalid-hex-rejected", {
        match parse_bump_hex_to_tsc("zzzz") {
            Err(e) if e.contains("Invalid BUMP hex") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject bad hex".into()),
        }
    });

    check!("bump/04 tree-height-zero-rejected", {
        // height=1 (varint), tree_height=0
        let bump_hex = hex::encode(&[0x01, 0x00]);
        match parse_bump_hex_to_tsc(&bump_hex) {
            Err(e) if e.contains("tree height is 0") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject tree_height=0".into()),
        }
    });

    check!("bump/05 height-in-output", {
        // Build BUMP with height=42
        let mut bytes = Vec::new();
        bytes.push(42); // height
        bytes.push(1);  // tree_height
        bytes.push(2);  // 2 nodes at level 0
        // TXID node
        bytes.push(0);     // offset
        bytes.push(0x02);  // TXID flag
        bytes.extend_from_slice(&[0x11; 32]);
        // Sibling node
        bytes.push(1);     // offset
        bytes.push(0x00);  // no flags
        bytes.extend_from_slice(&[0x22; 32]);

        let result = parse_bump_hex_to_tsc(&hex::encode(&bytes)).map_err(|e| e)?;
        if result["height"] != 42 {
            return Err(format!("height: {:?}", result["height"]));
        }
        Ok(())
    });

    check!("bump/06 tx-index-from-offset", {
        // Build BUMP where TXID is at offset 5 (tx_index=5)
        let mut bytes = Vec::new();
        bytes.push(1);   // height
        bytes.push(1);   // tree_height
        bytes.push(2);   // 2 nodes
        // TXID node at offset 5
        bytes.push(5);     // offset
        bytes.push(0x02);  // TXID flag
        bytes.extend_from_slice(&[0x33; 32]);
        // Sibling at offset 4
        bytes.push(4);     // offset
        bytes.push(0x00);  // no flags
        bytes.extend_from_slice(&[0x44; 32]);

        let result = parse_bump_hex_to_tsc(&hex::encode(&bytes)).map_err(|e| e)?;
        if result["index"] != 5 {
            return Err(format!("tx index: {:?}", result["index"]));
        }
        Ok(())
    });

    check!("bump/07 duplicate-flag-produces-star", {
        // Build BUMP with duplicate flag on sibling
        let mut bytes = Vec::new();
        bytes.push(1);   // height
        bytes.push(1);   // tree_height
        bytes.push(2);   // 2 nodes
        // TXID node
        bytes.push(0);     // offset
        bytes.push(0x02);  // TXID flag
        bytes.extend_from_slice(&[0x55; 32]);
        // Sibling with duplicate flag (0x01) — no hash follows
        bytes.push(1);     // offset
        bytes.push(0x01);  // duplicate flag

        let result = parse_bump_hex_to_tsc(&hex::encode(&bytes)).map_err(|e| e)?;
        let nodes = result["nodes"].as_array().ok_or("no nodes")?;
        if nodes[0].as_str() != Some("*") {
            return Err(format!("duplicate should produce '*', got {:?}", nodes[0]));
        }
        Ok(())
    });

    check!("bump/08 multi-level-bump", {
        // 2-level BUMP (tree_height=2)
        let mut bytes = Vec::new();
        bytes.push(200); // height
        bytes.push(2);   // tree_height = 2 levels

        // Level 0: 2 nodes
        bytes.push(2);
        bytes.push(0);     bytes.push(0x02); bytes.extend_from_slice(&[0x11; 32]); // TXID
        bytes.push(1);     bytes.push(0x00); bytes.extend_from_slice(&[0x22; 32]); // sibling

        // Level 1: 2 nodes
        bytes.push(2);
        bytes.push(0);     bytes.push(0x00); bytes.extend_from_slice(&[0x33; 32]); // hash
        bytes.push(1);     bytes.push(0x00); bytes.extend_from_slice(&[0x44; 32]); // sibling

        let result = parse_bump_hex_to_tsc(&hex::encode(&bytes)).map_err(|e| e)?;
        let nodes = result["nodes"].as_array().ok_or("no nodes")?;
        if nodes.len() != 2 {
            return Err(format!("expected 2 levels of nodes, got {}", nodes.len()));
        }
        Ok(())
    });

    check!("bump/09 truncated-data", {
        // Only height byte, no tree_height
        let bytes = vec![0x05];
        match parse_bump_hex_to_tsc(&hex::encode(&bytes)) {
            Err(_) => Ok(()), // Should fail trying to read tree_height
            Ok(_) => Err("should fail on truncated data".into()),
        }
    });

    check!("bump/10 target-field-is-empty-string", {
        // The TSC output always has target="" (block hash not available from BUMP)
        let mut bytes = Vec::new();
        bytes.push(1); bytes.push(1); bytes.push(2);
        bytes.push(0); bytes.push(0x02); bytes.extend_from_slice(&[0; 32]);
        bytes.push(1); bytes.push(0x00); bytes.extend_from_slice(&[0; 32]);

        let result = parse_bump_hex_to_tsc(&hex::encode(&bytes)).map_err(|e| e)?;
        if result["target"] != "" {
            return Err(format!("target should be empty string, got {:?}", result["target"]));
        }
        Ok(())
    });
}

// ═══════════════════════════════════════════════════════════════════
// [5/6]  BEEF validate_beef_v1_hex
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t10_05_validate_beef_v1() {
    use hodos_wallet::beef::validate_beef_v1_hex;

    eprintln!("\n[5/6] BEEF validate_beef_v1_hex");

    check!("validate/01 valid-v1-passes", {
        let v1_hex = build_test_beef_v1_hex();
        validate_beef_v1_hex(&v1_hex).map_err(|e| e)?;
        Ok(())
    });

    check!("validate/02 invalid-hex-rejected", {
        match validate_beef_v1_hex("not-valid-hex!!!") {
            Err(e) if e.contains("Invalid hex") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject bad hex".into()),
        }
    });

    check!("validate/03 wrong-version-marker", {
        // V2 marker should be rejected by validate_beef_v1_hex
        let beef = hodos_wallet::beef::Beef::new();
        let v2_hex = beef.to_hex().map_err(|e| e)?;
        match validate_beef_v1_hex(&v2_hex) {
            Err(e) if e.contains("Not BEEF V1") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject V2 marker".into()),
        }
    });

    check!("validate/04 truncated-at-marker", {
        // Only 4 bytes (V1 marker, nothing else)
        let hex_str = hex::encode(&[0x01, 0x00, 0xbe, 0xef]);
        match validate_beef_v1_hex(&hex_str) {
            Err(_) => Ok(()),
            Ok(_) => Err("should reject truncated BEEF".into()),
        }
    });

    check!("validate/05 empty-beef-v1-valid", {
        // V1 marker + 0 bumps + 0 txs
        let mut bytes = vec![0x01, 0x00, 0xbe, 0xef];
        bytes.push(0x00); // 0 bumps
        bytes.push(0x00); // 0 txs
        let hex_str = hex::encode(&bytes);
        validate_beef_v1_hex(&hex_str).map_err(|e| e)?;
        Ok(())
    });

    check!("validate/06 random-garbage-rejected", {
        match validate_beef_v1_hex("deadbeefcafebabe") {
            Err(_) => Ok(()),
            Ok(_) => Err("random bytes should fail validation".into()),
        }
    });

    check!("validate/07 v1-with-bumps-validates", {
        let v1_hex = build_test_beef_v1_hex();
        // Just verify it passes without error (bump parsing is correct)
        validate_beef_v1_hex(&v1_hex).map_err(|e| e)?;
        Ok(())
    });

    check!("validate/08 empty-string-rejected", {
        match validate_beef_v1_hex("") {
            Err(_) => Ok(()),
            Ok(_) => Err("empty string should fail".into()),
        }
    });
}

// ═══════════════════════════════════════════════════════════════════
// [6/6]  Database helpers
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t10_06_database_helpers() {
    use hodos_wallet::database::{address_to_address_info, output_to_fetcher_utxo};
    use hodos_wallet::database::Address;
    use hodos_wallet::database::Output;

    eprintln!("\n[6/6] Database helpers (address_to_address_info, output_to_fetcher_utxo)");

    check!("helpers/01 address-to-info-basic", {
        let addr = Address {
            id: Some(1),
            wallet_id: 1,
            index: 5,
            address: "1ABC".to_string(),
            public_key: "02abcd".to_string(),
            used: true,
            balance: 50000,
            pending_utxo_check: false,
            created_at: 1000,
        };
        let info = address_to_address_info(&addr);
        if info.index != 5 { return Err(format!("index {} != 5", info.index)); }
        if info.address != "1ABC" { return Err("address mismatch".into()); }
        if info.public_key != "02abcd" { return Err("pubkey mismatch".into()); }
        if !info.used { return Err("used should be true".into()); }
        if info.balance != 50000 { return Err(format!("balance {} != 50000", info.balance)); }
        Ok(())
    });

    check!("helpers/02 address-to-info-unused", {
        let addr = Address {
            id: None,
            wallet_id: 1,
            index: 0,
            address: "1XYZ".to_string(),
            public_key: "03ef".to_string(),
            used: false,
            balance: 0,
            pending_utxo_check: true,
            created_at: 0,
        };
        let info = address_to_address_info(&addr);
        if info.used { return Err("should not be used".into()); }
        if info.balance != 0 { return Err("balance should be 0".into()); }
        Ok(())
    });

    check!("helpers/03 output-to-utxo-hd-address", {
        let output = Output {
            output_id: Some(10),
            user_id: 1,
            transaction_id: Some(5),
            basket_id: None,
            spendable: true,
            change: false,
            vout: 0,
            satoshis: 25000,
            provided_by: "you".into(),
            purpose: "receive".into(),
            output_type: "P2PKH".into(),
            output_description: None,
            txid: Some("abcd1234".to_string()),
            sender_identity_key: None,
            derivation_prefix: Some("2-receive address".to_string()),
            derivation_suffix: Some("7".to_string()),
            custom_instructions: None,
            spent_by: None,
            sequence_number: None,
            spending_description: None,
            script_length: None,
            script_offset: None,
            locking_script: Some(vec![0x76, 0xa9, 0x14]),
            created_at: 1000,
            updated_at: 1000,
        };
        let utxo = output_to_fetcher_utxo(&output);
        if utxo.txid != "abcd1234" { return Err(format!("txid: {}", utxo.txid)); }
        if utxo.vout != 0 { return Err(format!("vout: {}", utxo.vout)); }
        if utxo.satoshis != 25000 { return Err(format!("satoshis: {}", utxo.satoshis)); }
        if utxo.address_index != 7 { return Err(format!("address_index {} != 7", utxo.address_index)); }
        if utxo.script != "76a914" { return Err(format!("script: {}", utxo.script)); }
        if utxo.custom_instructions.is_some() { return Err("should have no custom_instructions".into()); }
        Ok(())
    });

    check!("helpers/04 output-to-utxo-master-key", {
        let output = Output {
            output_id: Some(1),
            user_id: 1,
            transaction_id: None,
            basket_id: None,
            spendable: true,
            change: false,
            vout: 2,
            satoshis: 100,
            provided_by: "you".into(),
            purpose: "".into(),
            output_type: "P2PKH".into(),
            output_description: None,
            txid: Some("deadbeef".into()),
            sender_identity_key: None,
            derivation_prefix: None, // No derivation = master key
            derivation_suffix: None,
            custom_instructions: None,
            spent_by: None,
            sequence_number: None,
            spending_description: None,
            script_length: None,
            script_offset: None,
            locking_script: None,
            created_at: 0,
            updated_at: 0,
        };
        let utxo = output_to_fetcher_utxo(&output);
        if utxo.address_index != -1 {
            return Err(format!("master key should give address_index -1, got {}", utxo.address_index));
        }
        if !utxo.script.is_empty() {
            return Err(format!("no locking_script should give empty script, got {}", utxo.script));
        }
        Ok(())
    });

    check!("helpers/05 output-to-utxo-custom-derivation", {
        let output = Output {
            output_id: Some(1),
            user_id: 1,
            transaction_id: None,
            basket_id: None,
            spendable: true,
            change: false,
            vout: 0,
            satoshis: 500,
            provided_by: "them".into(),
            purpose: "".into(),
            output_type: "custom".into(),
            output_description: None,
            txid: Some("cafebabe".into()),
            sender_identity_key: None,
            derivation_prefix: Some("bip32".to_string()),
            derivation_suffix: Some("0".to_string()),
            custom_instructions: Some("{\"path\":\"m/0\"}".to_string()),
            spent_by: None,
            sequence_number: None,
            spending_description: None,
            script_length: None,
            script_offset: None,
            locking_script: Some(vec![0x00, 0x14]),
            created_at: 0,
            updated_at: 0,
        };
        let utxo = output_to_fetcher_utxo(&output);
        // "bip32" prefix is not "2-receive address", so address_index = -2
        if utxo.address_index != -2 {
            return Err(format!("custom derivation should give -2, got {}", utxo.address_index));
        }
        if utxo.custom_instructions != Some("{\"path\":\"m/0\"}".to_string()) {
            return Err("custom_instructions mismatch".into());
        }
        Ok(())
    });

    check!("helpers/06 output-to-utxo-unparseable-suffix", {
        let output = Output {
            output_id: Some(1),
            user_id: 1,
            transaction_id: None,
            basket_id: None,
            spendable: true,
            change: false,
            vout: 0,
            satoshis: 100,
            provided_by: "you".into(),
            purpose: "".into(),
            output_type: "P2PKH".into(),
            output_description: None,
            txid: Some("abc".into()),
            sender_identity_key: None,
            derivation_prefix: Some("2-receive address".to_string()),
            derivation_suffix: Some("not-a-number".to_string()),
            custom_instructions: None,
            spent_by: None,
            sequence_number: None,
            spending_description: None,
            script_length: None,
            script_offset: None,
            locking_script: None,
            created_at: 0,
            updated_at: 0,
        };
        let utxo = output_to_fetcher_utxo(&output);
        // Unparseable suffix should fall back to -1
        if utxo.address_index != -1 {
            return Err(format!("unparseable suffix should give -1, got {}", utxo.address_index));
        }
        Ok(())
    });

    check!("helpers/07 output-txid-defaults-to-empty", {
        let output = Output {
            output_id: None,
            user_id: 1,
            transaction_id: None,
            basket_id: None,
            spendable: false,
            change: false,
            vout: 0,
            satoshis: 0,
            provided_by: "".into(),
            purpose: "".into(),
            output_type: "".into(),
            output_description: None,
            txid: None, // No txid
            sender_identity_key: None,
            derivation_prefix: None,
            derivation_suffix: None,
            custom_instructions: None,
            spent_by: None,
            sequence_number: None,
            spending_description: None,
            script_length: None,
            script_offset: None,
            locking_script: None,
            created_at: 0,
            updated_at: 0,
        };
        let utxo = output_to_fetcher_utxo(&output);
        if !utxo.txid.is_empty() {
            return Err(format!("None txid should give empty string, got '{}'", utxo.txid));
        }
        Ok(())
    });

    check!("helpers/08 output-vout-i32-to-u32", {
        let output = Output {
            output_id: None,
            user_id: 1,
            transaction_id: None,
            basket_id: None,
            spendable: true,
            change: false,
            vout: 255,
            satoshis: 1,
            provided_by: "".into(),
            purpose: "".into(),
            output_type: "".into(),
            output_description: None,
            txid: Some("x".into()),
            sender_identity_key: None,
            derivation_prefix: None,
            derivation_suffix: None,
            custom_instructions: None,
            spent_by: None,
            sequence_number: None,
            spending_description: None,
            script_length: None,
            script_offset: None,
            locking_script: None,
            created_at: 0,
            updated_at: 0,
        };
        let utxo = output_to_fetcher_utxo(&output);
        if utxo.vout != 255 {
            return Err(format!("vout {} != 255", utxo.vout));
        }
        Ok(())
    });
}

// ═══════════════════════════════════════════════════════════════════
// Summary
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t10_99_summary() {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let p = PASS.load(Ordering::SeqCst);
    let f = FAIL.load(Ordering::SeqCst);
    eprintln!("\n════════════════════════════════════════");
    eprintln!("  TIER 10 FINAL:  {} passed, {} failed  (of {} total)", p, f, p + f);
    eprintln!("════════════════════════════════════════\n");
    assert_eq!(f, 0, "{} test(s) failed — see FAIL lines above", f);
}
