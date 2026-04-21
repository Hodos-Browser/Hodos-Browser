/// Integration test: Compare Rust crypto output with known TypeScript SDK v2 values
/// Run with: cargo test --test sdk_interop_test -- --nocapture
///
/// These test vectors were generated from @bsv/sdk v2.0.7 (test-sdk-v2-vectors.mjs).
/// SDK v2 changed Utils.toUTF8 from a manual parser to TextDecoder, which replaces
/// invalid UTF-8 bytes with U+FFFD instead of skipping them. This is the industry
/// standard and matches Rust's String::from_utf8_lossy().

use hodos_wallet::crypto::brc42::derive_symmetric_key_for_hmac;
use hodos_wallet::crypto::brc43::{InvoiceNumber, SecurityLevel, normalize_protocol_id};
use hodos_wallet::crypto::signing::hmac_sha256;
use hodos_wallet::crypto::keys::derive_public_key;
use hodos_wallet::crypto::brc2;

/// Test private key (same as TypeScript test)
const TEST_PRIVKEY_HEX: &str = "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35";
/// Counterparty private key (same as TypeScript test)
const COUNTERPARTY_PRIVKEY_HEX: &str = "c9afa9d845ba75166b5c215767b1d6934e50c3db36e89b127b8a622b120f6721";

fn hex_decode(s: &str) -> Vec<u8> {
    hex::decode(s).unwrap()
}

/// Convert bytes to UTF-8 matching SDK v2's TextDecoder behavior.
/// SDK v2: new TextDecoder().decode(new Uint8Array(arr))
/// Rust:   String::from_utf8_lossy(bytes)
/// Both replace invalid UTF-8 bytes with U+FFFD.
fn js_to_utf8(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

// =========================================================================
// Test 1: js_to_utf8 matches SDK v2's TextDecoder (simple ASCII)
// =========================================================================
#[test]
fn test_utf8_simple_ascii() {
    // Simple ASCII bytes — same output in SDK v1 and v2
    let input: [u8; 16] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10];
    let expected_hex = "0102030405060708090a0b0c0d0e0f10";

    let result = js_to_utf8(&input);
    let result_hex = hex::encode(result.as_bytes());
    assert_eq!(result_hex, expected_hex, "Simple ASCII UTF-8 mismatch!");
    println!("✅ Simple ASCII bytes match SDK v2");
}

// =========================================================================
// Test 2: js_to_utf8 matches SDK v2 with INVALID UTF-8 bytes (CRITICAL)
// This is the test that would have caught the SocialCert v2 breakage.
// SDK v1 skipped invalid bytes; v2 replaces them with U+FFFD (ef bf bd).
// =========================================================================
#[test]
fn test_utf8_invalid_bytes_v2() {
    // Bytes with invalid UTF-8 sequences (0xFA, 0xA3, 0xD2, etc.)
    let input: [u8; 16] = [0x45, 0xFA, 0xA3, 0x0B, 0xE3, 0xCE, 0xBF, 0x14,
                            0x02, 0x6B, 0xE6, 0x91, 0xD2, 0x49, 0x63, 0xFA];
    // SDK v2 output: invalid bytes become U+FFFD (ef bf bd)
    let expected_hex = "45efbfbdefbfbd0befbfbdcebf14026befbfbdefbfbd4963efbfbd";

    let result = js_to_utf8(&input);
    let result_hex = hex::encode(result.as_bytes());
    println!("Input:    {}", hex::encode(&input));
    println!("Expected: {}", expected_hex);
    println!("Got:      {}", result_hex);
    assert_eq!(result_hex, expected_hex, "Invalid UTF-8 bytes mismatch with SDK v2!");
    println!("✅ Invalid UTF-8 bytes match SDK v2 (TextDecoder / from_utf8_lossy)");
}

// =========================================================================
// Test 3: js_to_utf8 with overlong 4-byte sequence (all invalid)
// =========================================================================
#[test]
fn test_utf8_overlong_4byte_v2() {
    let input = [0xF0u8, 0x80, 0x80, 0x80];
    // SDK v2: all 4 bytes are invalid UTF-8, each becomes U+FFFD
    let expected_hex = "efbfbdefbfbdefbfbdefbfbd";

    let result = js_to_utf8(&input);
    let result_hex = hex::encode(result.as_bytes());
    assert_eq!(result_hex, expected_hex, "Overlong 4-byte sequence mismatch!");
    println!("✅ Overlong 4-byte sequence matches SDK v2");
}

// =========================================================================
// Test 4: js_to_utf8 with valid supplementary character U+10000
// =========================================================================
#[test]
fn test_utf8_valid_supplementary_v2() {
    let input = [0xF0u8, 0x90, 0x80, 0x80]; // U+10000
    let expected_hex = "f0908080";

    let result = js_to_utf8(&input);
    let result_hex = hex::encode(result.as_bytes());
    assert_eq!(result_hex, expected_hex, "Valid supplementary character mismatch!");
    println!("✅ Valid supplementary character matches SDK v2");
}

// =========================================================================
// Test 5: js_to_utf8 with all-invalid continuation bytes
// =========================================================================
#[test]
fn test_utf8_all_invalid_v2() {
    let input = [0x80u8, 0xBF, 0xC0];
    // SDK v2: each invalid byte becomes U+FFFD
    let expected_hex = "efbfbdefbfbdefbfbd";

    let result = js_to_utf8(&input);
    let result_hex = hex::encode(result.as_bytes());
    assert_eq!(result_hex, expected_hex, "All-invalid bytes mismatch!");
    println!("✅ All-invalid bytes match SDK v2");
}

// =========================================================================
// Test 6: Nonce HMAC with counterparty='self' (ASCII keyID)
// Same values in v1 and v2 because ASCII is valid UTF-8
// =========================================================================
#[test]
fn test_nonce_hmac_self_counterparty() {
    let first_half: [u8; 16] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                                  0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10];
    let master_privkey = hex_decode(TEST_PRIVKEY_HEX);

    // From SDK v2.0.7 (same as v1 for ASCII input)
    let expected_pubkey = "0339a36013301597daef41fbe593a02cc513d0b55527ec2df1050e2e8ff49c85c2";
    let expected_hmac = "838a21bd0f670c6a26e18692df1c13d8135f55c7ae66dda7450f023b7a7bca0f";

    let pubkey = derive_public_key(&master_privkey).unwrap();
    assert_eq!(hex::encode(&pubkey), expected_pubkey, "Public key mismatch!");

    let key_id = js_to_utf8(&first_half);
    let protocol_id = normalize_protocol_id("server hmac").unwrap();
    let invoice = InvoiceNumber::new(SecurityLevel::CounterpartyLevel, protocol_id, &key_id).unwrap();
    let invoice_str = invoice.to_string();

    let symmetric_key = derive_symmetric_key_for_hmac(&master_privkey, &pubkey, &invoice_str).unwrap();

    let mut stripped_key: &[u8] = &symmetric_key;
    while stripped_key.len() > 1 && stripped_key[0] == 0 {
        stripped_key = &stripped_key[1..];
    }

    let hmac = hmac_sha256(stripped_key, &first_half);
    assert_eq!(hex::encode(&hmac), expected_hmac, "HMAC mismatch!");
    println!("✅ Nonce HMAC (self, ASCII) matches SDK v2");
}

// =========================================================================
// Test 7: Nonce HMAC with actual counterparty (ASCII keyID)
// =========================================================================
#[test]
fn test_nonce_hmac_with_counterparty() {
    let first_half: [u8; 16] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                                  0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10];
    let master_privkey = hex_decode(TEST_PRIVKEY_HEX);
    let counterparty_privkey = hex_decode(COUNTERPARTY_PRIVKEY_HEX);

    // From SDK v2.0.7
    let expected_hmac = "9e5a48881dffda0da84a50159b91c447099916bd6b826d428ecd8d48770bc322";

    let counterparty_pubkey = derive_public_key(&counterparty_privkey).unwrap();
    let client_pubkey = derive_public_key(&master_privkey).unwrap();

    let key_id = js_to_utf8(&first_half);
    let protocol_id = normalize_protocol_id("server hmac").unwrap();
    let invoice = InvoiceNumber::new(SecurityLevel::CounterpartyLevel, protocol_id, &key_id).unwrap();
    let invoice_str = invoice.to_string();

    // Client side
    let client_sym_key = derive_symmetric_key_for_hmac(&master_privkey, &counterparty_pubkey, &invoice_str).unwrap();
    let mut client_stripped: &[u8] = &client_sym_key;
    while client_stripped.len() > 1 && client_stripped[0] == 0 { client_stripped = &client_stripped[1..]; }
    let client_hmac = hmac_sha256(client_stripped, &first_half);
    assert_eq!(hex::encode(&client_hmac), expected_hmac, "Client HMAC mismatch!");

    // Server side (ECDH symmetry)
    let server_sym_key = derive_symmetric_key_for_hmac(&counterparty_privkey, &client_pubkey, &invoice_str).unwrap();
    let mut server_stripped: &[u8] = &server_sym_key;
    while server_stripped.len() > 1 && server_stripped[0] == 0 { server_stripped = &server_stripped[1..]; }
    let server_hmac = hmac_sha256(server_stripped, &first_half);
    assert_eq!(hex::encode(&server_hmac), expected_hmac, "Server HMAC mismatch!");

    println!("✅ Nonce HMAC (counterparty, ASCII) matches SDK v2");
    println!("✅ ECDH symmetry verified");
}

// =========================================================================
// Test 8: Nonce HMAC with INVALID UTF-8 keyID (SDK v2 CRITICAL)
// This test ONLY passes with SDK v2 compatible js_to_utf8.
// It would FAIL with the old SDK v1 manual parser.
// =========================================================================
#[test]
fn test_nonce_hmac_invalid_utf8_keyid_v2() {
    // Bytes with invalid UTF-8 (same as real nonce random bytes)
    let first_half: [u8; 16] = [0x45, 0xFA, 0xA3, 0x0B, 0xE3, 0xCE, 0xBF, 0x14,
                                0x02, 0x6B, 0xE6, 0x91, 0xD2, 0x49, 0x63, 0xFA];
    let master_privkey = hex_decode(TEST_PRIVKEY_HEX);

    // From SDK v2.0.7 — the keyID contains U+FFFD replacements
    let expected_key_id_hex = "45efbfbdefbfbd0befbfbdcebf14026befbfbdefbfbd4963efbfbd";
    let expected_hmac = "63c74504c01fa34426ea3866ab25613f3d1ec4d9bbee02871cd24da234cec819";

    let pubkey = derive_public_key(&master_privkey).unwrap();
    let key_id = js_to_utf8(&first_half);

    // Verify keyID bytes match SDK v2
    let key_id_hex = hex::encode(key_id.as_bytes());
    println!("KeyID hex:     {}", key_id_hex);
    println!("Expected hex:  {}", expected_key_id_hex);
    assert_eq!(key_id_hex, expected_key_id_hex, "KeyID bytes mismatch with SDK v2!");

    // Build invoice and compute HMAC
    let protocol_id = normalize_protocol_id("server hmac").unwrap();
    let invoice = InvoiceNumber::new(SecurityLevel::CounterpartyLevel, protocol_id, &key_id).unwrap();
    let invoice_str = invoice.to_string();

    let symmetric_key = derive_symmetric_key_for_hmac(&master_privkey, &pubkey, &invoice_str).unwrap();
    let mut stripped_key: &[u8] = &symmetric_key;
    while stripped_key.len() > 1 && stripped_key[0] == 0 {
        stripped_key = &stripped_key[1..];
    }

    let hmac = hmac_sha256(stripped_key, &first_half);
    println!("HMAC:          {}", hex::encode(&hmac));
    println!("Expected HMAC: {}", expected_hmac);
    assert_eq!(hex::encode(&hmac), expected_hmac, "HMAC mismatch with SDK v2 for invalid UTF-8 keyID!");

    println!("✅ Nonce HMAC with invalid UTF-8 keyID matches SDK v2");
    println!("   (This test catches SDK v1→v2 toUTF8 breakage)");
}

// =========================================================================
// Test 9: BRC-2 symmetric key derivation
// =========================================================================
#[test]
fn test_brc2_derive_symmetric_key() {
    let client_privkey = hex_decode(TEST_PRIVKEY_HEX);
    let server_privkey = hex_decode(COUNTERPARTY_PRIVKEY_HEX);
    let server_pubkey = derive_public_key(&server_privkey).unwrap();
    let client_pubkey = derive_public_key(&client_privkey).unwrap();

    let expected_sym_key = "9c04965b43947395e68ef47b750b1f8f339d5429404954a1a365a332470be8ee";

    let client_sym_key = brc2::derive_symmetric_key(&client_privkey, &server_pubkey, "2-certificate field encryption-cool").unwrap();
    assert_eq!(hex::encode(&client_sym_key), expected_sym_key, "BRC-2 symmetric key mismatch!");

    let server_sym_key = brc2::derive_symmetric_key(&server_privkey, &client_pubkey, "2-certificate field encryption-cool").unwrap();
    assert_eq!(hex::encode(&server_sym_key), expected_sym_key, "Server BRC-2 symmetric key mismatch!");

    println!("✅ BRC-2 symmetric key derivation matches SDK v2");
}
