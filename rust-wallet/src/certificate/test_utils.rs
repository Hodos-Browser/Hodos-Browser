//! Certificate test utilities
//!
//! Helper functions for generating test certificates and related data structures.

use crate::certificate::types::{Certificate, CertificateField};
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Create a test certificate with all required fields
///
/// ## Arguments
/// - `type_bytes`: Certificate type (32 bytes)
/// - `subject_bytes`: Subject public key (33 bytes)
/// - `serial_bytes`: Serial number (32 bytes)
/// - `certifier_bytes`: Certifier public key (33 bytes)
/// - `revocation_outpoint`: Revocation outpoint string (format: "txid.vout")
/// - `signature_bytes`: Certificate signature (DER-encoded ECDSA)
/// - `fields`: Map of field names to encrypted field values
/// - `keyring`: Map of field names to master keyring values
///
/// ## Returns
/// A `Certificate` struct ready for testing
pub fn create_test_certificate(
    type_bytes: Vec<u8>,
    subject_bytes: Vec<u8>,
    serial_bytes: Vec<u8>,
    certifier_bytes: Vec<u8>,
    revocation_outpoint: String,
    signature_bytes: Vec<u8>,
    fields: HashMap<String, CertificateField>,
    keyring: HashMap<String, Vec<u8>>,
) -> Certificate {
    Certificate::new(
        type_bytes,
        subject_bytes,
        serial_bytes,
        certifier_bytes,
        revocation_outpoint,
        signature_bytes,
        fields,
        keyring,
    )
}

/// Create a minimal test certificate with default values
///
/// Uses all-zero bytes for most fields. Useful for basic testing.
pub fn create_minimal_test_certificate() -> Certificate {
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), CertificateField::new(
        "name".to_string(),
        vec![1, 2, 3, 4], // Encrypted value
        vec![5, 6, 7, 8],  // Master key
    ));

    create_test_certificate(
        vec![0u8; 32], // type
        vec![0u8; 33], // subject
        vec![0u8; 32], // serial_number
        vec![0u8; 33], // certifier
        "0000000000000000000000000000000000000000000000000000000000000000.0".to_string(), // revocation_outpoint
        vec![], // signature (empty)
        fields,
        HashMap::new(), // keyring
    )
}

/// Create test certificate JSON (for HTTP requests)
///
/// ## Arguments
/// - `type_b64`: Certificate type (base64-encoded)
/// - `subject_hex`: Subject public key (hex-encoded)
/// - `serial_b64`: Serial number (base64-encoded)
/// - `certifier_hex`: Certifier public key (hex-encoded)
/// - `revocation_outpoint`: Revocation outpoint string
/// - `signature_hex`: Certificate signature (hex-encoded)
/// - `fields`: Map of field names to base64-encoded encrypted values
/// - `keyring`: Map of field names to base64-encoded keyring values
///
/// ## Returns
/// JSON `Value` representing the certificate
pub fn create_test_certificate_json(
    type_b64: &str,
    subject_hex: &str,
    serial_b64: &str,
    certifier_hex: &str,
    revocation_outpoint: &str,
    signature_hex: &str,
    fields: HashMap<String, String>, // fieldName -> base64 encrypted value
    keyring: HashMap<String, String>, // fieldName -> base64 keyring value
) -> serde_json::Value {
    let mut cert_json = serde_json::Map::new();

    cert_json.insert("type".to_string(), serde_json::Value::String(type_b64.to_string()));
    cert_json.insert("subject".to_string(), serde_json::Value::String(subject_hex.to_string()));
    cert_json.insert("serialNumber".to_string(), serde_json::Value::String(serial_b64.to_string()));
    cert_json.insert("certifier".to_string(), serde_json::Value::String(certifier_hex.to_string()));
    cert_json.insert("revocationOutpoint".to_string(), serde_json::Value::String(revocation_outpoint.to_string()));
    cert_json.insert("signature".to_string(), serde_json::Value::String(signature_hex.to_string()));

    // Fields
    let mut fields_json = serde_json::Map::new();
    for (name, value) in fields {
        fields_json.insert(name, serde_json::Value::String(value));
    }
    cert_json.insert("fields".to_string(), serde_json::Value::Object(fields_json));

    // Keyring
    let mut keyring_json = serde_json::Map::new();
    for (name, value) in keyring {
        keyring_json.insert(name, serde_json::Value::String(value));
    }
    cert_json.insert("keyringForSubject".to_string(), serde_json::Value::Object(keyring_json));

    serde_json::Value::Object(cert_json)
}

/// Create test certificate fields with encrypted values
///
/// ## Arguments
/// - `field_names`: Vector of field names
/// - `encrypted_values`: Map of field names to encrypted values (Vec<u8>)
/// - `master_keys`: Map of field names to master keys (Vec<u8>)
///
/// ## Returns
/// HashMap of field names to CertificateField structs
pub fn create_test_certificate_fields(
    field_names: Vec<String>,
    encrypted_values: HashMap<String, Vec<u8>>,
    master_keys: HashMap<String, Vec<u8>>,
) -> HashMap<String, CertificateField> {
    let mut fields = HashMap::new();

    for name in field_names {
        let encrypted_value = encrypted_values.get(&name).cloned().unwrap_or_default();
        let master_key = master_keys.get(&name).cloned().unwrap_or_default();

        fields.insert(name.clone(), CertificateField::new(
            name,
            encrypted_value,
            master_key,
        ));
    }

    fields
}

/// Generate random bytes for testing
pub fn random_bytes(length: usize) -> Vec<u8> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..length).map(|_| rng.gen()).collect()
}

/// Generate random hex string for testing
pub fn random_hex_string(length: usize) -> String {
    hex::encode(&random_bytes(length / 2))
}

/// Generate random base64 string for testing
pub fn random_base64_string(length: usize) -> String {
    BASE64.encode(&random_bytes(length))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_minimal_certificate() {
        let cert = create_minimal_test_certificate();
        assert_eq!(cert.type_.len(), 32);
        assert_eq!(cert.subject.len(), 33);
        assert_eq!(cert.serial_number.len(), 32);
        assert_eq!(cert.certifier.len(), 33);
        assert!(cert.fields.contains_key("name"));
    }

    #[test]
    fn test_create_test_certificate_json() {
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), BASE64.encode(&[1, 2, 3, 4]));

        let mut keyring = HashMap::new();
        keyring.insert("name".to_string(), BASE64.encode(&[5, 6, 7, 8]));

        let json = create_test_certificate_json(
            &BASE64.encode(&vec![0u8; 32]),
            &hex::encode(&vec![0u8; 33]),
            &BASE64.encode(&vec![0u8; 32]),
            &hex::encode(&vec![0u8; 33]),
            "0000000000000000000000000000000000000000000000000000000000000000.0",
            &hex::encode(&[]),
            fields,
            keyring,
        );

        assert!(json.get("type").is_some());
        assert!(json.get("fields").is_some());
        assert!(json.get("keyringForSubject").is_some());
    }
}
