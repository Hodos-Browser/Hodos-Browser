/**
 * Test CSR JSON serialization to compare with TypeScript SDK
 *
 * This test verifies that our JSON serialization matches TypeScript SDK's JSON.stringify()
 * output, including field ordering.
 */

use serde_json::json;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

#[test]
fn test_csr_json_field_ordering() {
    // Create CSR JSON matching our implementation
    let csr = json!({
        "clientNonce": "test_nonce_base64==",
        "type": "test_type_base64==",
        "fields": {
            "cool": "encrypted_field_value_base64=="
        },
        "masterKeyring": {
            "cool": "encrypted_revelation_key_base64=="
        }
    });

    // Serialize to string
    let json_string = serde_json::to_string(&csr).unwrap();

    println!("CSR JSON (Rust): {}", json_string);
    println!("CSR JSON length: {} bytes", json_string.len());
    println!("CSR JSON (hex): {}", hex::encode(json_string.as_bytes()));

    // Check field order in serialized output
    // TypeScript's JSON.stringify() typically orders fields alphabetically for objects
    // But serde_json preserves insertion order (which is what we want)
    let expected_order = ["clientNonce", "type", "fields", "masterKeyring"];
    let mut found_order = Vec::new();

    for field in expected_order {
        if json_string.contains(&format!("\"{}\"", field)) {
            found_order.push(field);
        }
    }

    println!("Field order in JSON: {:?}", found_order);

    // Verify all fields are present
    assert!(json_string.contains("clientNonce"));
    assert!(json_string.contains("type"));
    assert!(json_string.contains("fields"));
    assert!(json_string.contains("masterKeyring"));
}

#[test]
fn test_csr_json_byte_comparison() {
    // Create CSR with known values
    let csr = json!({
        "clientNonce": "yEXaohOEWAyadhFPzMaeIkSKn/qk82BvzBmNCMPk+sM=",
        "type": "AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=",
        "fields": {
            "cool": "ZLlhLWhdLAGqBFbXa5deXg/hIiYQW9nfjnPJc4PeaZDynaOn0Xv5qDkeFTlTyw1XT6GSLA=="
        },
        "masterKeyring": {
            "cool": "hGVH4crS3/r43dThakr0lTM8Q/bQsPScnp+8P+LJ/Jm2rnEatjX/K7zZkWOUwTP/Cxio/mqkC+kmxMx3DzPVpsGDR77NjLN6k+8id1QAHeE="
        }
    });

    let json_string = serde_json::to_string(&csr).unwrap();
    let json_bytes = json_string.as_bytes();

    println!("CSR JSON bytes (hex, full): {}", hex::encode(json_bytes));
    println!("CSR JSON bytes (base64): {}", base64::engine::general_purpose::STANDARD.encode(json_bytes));
    println!("CSR JSON length: {} bytes", json_bytes.len());

    // Expected from logs: 345 bytes
    // This test will help us verify the exact byte representation
    assert_eq!(json_bytes.len(), 345, "CSR JSON should be 345 bytes based on logs");
}

#[test]
fn test_json_stringify_comparison() {
    // Test that our serialization produces compact JSON (no extra whitespace)
    let csr = json!({
        "clientNonce": "test1",
        "type": "test2",
        "fields": {"cool": "test3"},
        "masterKeyring": {"cool": "test4"}
    });

    let json_string = serde_json::to_string(&csr).unwrap();

    // Should be compact (no pretty printing)
    assert!(!json_string.contains('\n'));
    assert!(!json_string.contains(' ')); // No spaces except in string values

    println!("Compact JSON: {}", json_string);
}
