//! Test to output exact bytes for comparison with TypeScript SDK
//!
//! This test generates a CSR and serialized request exactly as our Rust
//! implementation does, and outputs all bytes in hex and base64 format
//! for byte-for-byte comparison with TypeScript SDK output.

use hex;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json;

#[test]
fn output_csr_bytes_for_comparison() {
    // Create a test CSR JSON matching our implementation
    let client_nonce = "test_client_nonce_base64_encoded_32_bytes==";
    let certificate_type = "AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=";

    // Create fields and masterKeyring (simplified for testing)
    let fields = serde_json::json!({
        "cool": "encrypted_field_value_base64"
    });
    let master_keyring = serde_json::json!({
        "cool": "encrypted_revelation_key_base64"
    });

    // Manually construct JSON string with exact field order (matching our implementation)
    let fields_json = serde_json::to_string(&fields).unwrap();
    let master_keyring_json = serde_json::to_string(&master_keyring).unwrap();
    let client_nonce_json = serde_json::to_string(&client_nonce).unwrap();
    let type_json = serde_json::to_string(&certificate_type).unwrap();

    let csr_json_string = format!(
        r#"{{"clientNonce":{},"type":{},"fields":{},"masterKeyring":{}}}"#,
        client_nonce_json,
        type_json,
        fields_json,
        master_keyring_json
    );

    let csr_json_bytes = csr_json_string.as_bytes();

    println!("\n📋 CSR JSON for Comparison:");
    println!("============================");
    println!("CSR JSON String: {}", csr_json_string);
    println!("CSR JSON Length: {} bytes", csr_json_bytes.len());
    println!("CSR JSON Hex: {}", hex::encode(csr_json_bytes));
    println!("CSR JSON Base64: {}", BASE64.encode(csr_json_bytes));
    println!("");

    // Now serialize the request (matching our implementation)
    // Note: encode_varint_signed is in transaction module but not re-exported
    // We'll use encode_varint directly for -1 (which is u64::MAX)
    use hodos_wallet::transaction::encode_varint;

    // Request nonce (32 bytes)
    let request_nonce = b"test_request_nonce_32_bytes_exactly";
    assert_eq!(request_nonce.len(), 32);

    let mut serialized_request = Vec::new();

    // 1. Request nonce (32 bytes)
    serialized_request.extend_from_slice(request_nonce);

    // 2. Method: POST
    let method = b"POST";
    serialized_request.extend_from_slice(&encode_varint(method.len() as u64));
    serialized_request.extend_from_slice(method);

    // 3. Pathname: /signCertificate
    let pathname = b"/signCertificate";
    serialized_request.extend_from_slice(&encode_varint(pathname.len() as u64));
    serialized_request.extend_from_slice(pathname);

    // 4. Search: -1 (empty)
    // -1 as VarInt is 0xFF followed by 8 bytes of 0xFF (u64::MAX)
    serialized_request.push(0xFF);
    serialized_request.extend_from_slice(&u64::MAX.to_le_bytes());

    // 5. Headers: content-type = application/json
    let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
    headers.sort_by(|a, b| a.0.cmp(&b.0));

    serialized_request.extend_from_slice(&encode_varint(headers.len() as u64));
    for (key, value) in &headers {
        let key_bytes = key.as_bytes();
        serialized_request.extend_from_slice(&encode_varint(key_bytes.len() as u64));
        serialized_request.extend_from_slice(key_bytes);

        let value_bytes = value.as_bytes();
        serialized_request.extend_from_slice(&encode_varint(value_bytes.len() as u64));
        serialized_request.extend_from_slice(value_bytes);
    }

    // 6. Body: CSR JSON
    serialized_request.extend_from_slice(&encode_varint(csr_json_bytes.len() as u64));
    serialized_request.extend_from_slice(csr_json_bytes);

    println!("📦 Serialized Request for Comparison:");
    println!("=====================================");
    println!("Total Length: {} bytes", serialized_request.len());
    println!("Serialized Request Hex (FULL): {}", hex::encode(&serialized_request));
    println!("Serialized Request Base64 (FULL): {}", BASE64.encode(&serialized_request));
    println!("");

    // Breakdown by section
    let mut offset = 0;
    println!("Breakdown by Section:");
    println!("---------------------");

    // Nonce
    offset += 32;
    println!("[0..31] Nonce (32 bytes): {}", hex::encode(&serialized_request[0..offset]));

    // Method
    let method_varint_start = offset;
    let method_varint_len = if serialized_request[offset] < 0xFD { 1 }
        else if serialized_request[offset] == 0xFD { 3 }
        else if serialized_request[offset] == 0xFE { 5 }
        else { 9 };
    offset += method_varint_len;
    let method_start = offset;
    offset += method.len();
    println!("[{}..{}] Method VarInt ({} bytes): {}",
        method_varint_start, method_varint_start + method_varint_len - 1,
        method_varint_len, hex::encode(&serialized_request[method_varint_start..method_varint_start + method_varint_len]));
    println!("[{}..{}] Method ({} bytes): {}",
        method_start, offset - 1, method.len(),
        String::from_utf8_lossy(&serialized_request[method_start..offset]));

    // Pathname
    let path_varint_start = offset;
    let path_varint_len = if serialized_request[offset] < 0xFD { 1 }
        else if serialized_request[offset] == 0xFD { 3 }
        else if serialized_request[offset] == 0xFE { 5 }
        else { 9 };
    offset += path_varint_len;
    let path_start = offset;
    offset += pathname.len();
    println!("[{}..{}] Path VarInt ({} bytes): {}",
        path_varint_start, path_varint_start + path_varint_len - 1,
        path_varint_len, hex::encode(&serialized_request[path_varint_start..path_varint_start + path_varint_len]));
    println!("[{}..{}] Path ({} bytes): {}",
        path_start, offset - 1, pathname.len(),
        String::from_utf8_lossy(&serialized_request[path_start..offset]));

    // Search (-1)
    let search_varint_start = offset;
    let search_varint_len = if serialized_request[offset] < 0xFD { 1 }
        else if serialized_request[offset] == 0xFD { 3 }
        else if serialized_request[offset] == 0xFE { 5 }
        else { 9 };
    offset += search_varint_len;
    println!("[{}..{}] Search VarInt (-1, {} bytes): {}",
        search_varint_start, offset - 1, search_varint_len,
        hex::encode(&serialized_request[search_varint_start..offset]));

    // Headers
    let header_count_varint_start = offset;
    let header_count_varint_len = if serialized_request[offset] < 0xFD { 1 }
        else if serialized_request[offset] == 0xFD { 3 }
        else if serialized_request[offset] == 0xFE { 5 }
        else { 9 };
    offset += header_count_varint_len;
    println!("[{}..{}] Header Count VarInt ({} bytes): {}",
        header_count_varint_start, offset - 1, header_count_varint_len,
        hex::encode(&serialized_request[header_count_varint_start..offset]));

    for (key, value) in &headers {
        // Key
        let key_varint_start = offset;
        let key_varint_len = if serialized_request[offset] < 0xFD { 1 }
            else if serialized_request[offset] == 0xFD { 3 }
            else if serialized_request[offset] == 0xFE { 5 }
            else { 9 };
        offset += key_varint_len;
        let key_start = offset;
        offset += key.as_bytes().len();
        println!("  [{}..{}] Header Key VarInt ({} bytes): {}",
            key_varint_start, key_varint_start + key_varint_len - 1,
            key_varint_len, hex::encode(&serialized_request[key_varint_start..key_varint_start + key_varint_len]));
        println!("  [{}..{}] Header Key ({} bytes): {}",
            key_start, offset - 1, key.as_bytes().len(),
            String::from_utf8_lossy(&serialized_request[key_start..offset]));

        // Value
        let value_varint_start = offset;
        let value_varint_len = if serialized_request[offset] < 0xFD { 1 }
            else if serialized_request[offset] == 0xFD { 3 }
            else if serialized_request[offset] == 0xFE { 5 }
            else { 9 };
        offset += value_varint_len;
        let value_start = offset;
        offset += value.as_bytes().len();
        println!("  [{}..{}] Header Value VarInt ({} bytes): {}",
            value_varint_start, value_varint_start + value_varint_len - 1,
            value_varint_len, hex::encode(&serialized_request[value_varint_start..value_varint_start + value_varint_len]));
        println!("  [{}..{}] Header Value ({} bytes): {}",
            value_start, offset - 1, value.as_bytes().len(),
            String::from_utf8_lossy(&serialized_request[value_start..offset]));
    }

    // Body
    let body_varint_start = offset;
    let body_varint_len = if serialized_request[offset] < 0xFD { 1 }
        else if serialized_request[offset] == 0xFD { 3 }
        else if serialized_request[offset] == 0xFE { 5 }
        else { 9 };
    offset += body_varint_len;
    let body_start = offset;
    offset += csr_json_bytes.len();
    println!("[{}..{}] Body Length VarInt ({} bytes): {}",
        body_varint_start, body_varint_start + body_varint_len - 1,
        body_varint_len, hex::encode(&serialized_request[body_varint_start..body_varint_start + body_varint_len]));
    println!("[{}..{}] Body ({} bytes): {}",
        body_start, offset - 1, csr_json_bytes.len(),
        String::from_utf8_lossy(&serialized_request[body_start..offset]));

    println!("\n✅ Test complete - compare the hex/base64 output above with TypeScript SDK output");
}
