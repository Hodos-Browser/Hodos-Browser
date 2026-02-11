//! Certificate parser
//!
//! Parses BRC-52 certificates from JSON format (for `acquireCertificate` 'direct' protocol).

use crate::certificate::types::{Certificate, CertificateField, CertificateError};
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Parse certificate from JSON structure
///
/// **BRC-52**: Parses certificate JSON received in `acquireCertificate` 'direct' protocol
///
/// ## JSON Structure (from BRC-100 spec):
/// ```json
/// {
///   "type": "base64_string",           // 32 bytes base64-encoded
///   "certifier": "hex_string",         // 33 bytes compressed public key
///   "fields": {                         // Map of fieldName -> fieldValue (base64 encrypted)
///     "fieldName": "base64_encrypted_value"
///   },
///   "serialNumber": "base64_string",   // 32 bytes base64-encoded
///   "revocationOutpoint": "txid.vout",  // Revocation outpoint
///   "signature": "hex_string",          // DER-encoded ECDSA signature
///   "keyringForSubject": {              // Map of fieldName -> keyring value (base64)
///     "fieldName": "base64_keyring_value"
///   },
///   "subject": "hex_string"             // 33 bytes compressed public key (optional, may be derived)
/// }
/// ```
///
/// ## Arguments
/// - `json_data`: Certificate JSON data (from BRC-100 request)
///
/// ## Returns
/// Parsed `Certificate` struct
pub fn parse_certificate_from_json(
    json_data: &serde_json::Value,
) -> Result<Certificate, CertificateError> {
    // Parse type (base64, 32 bytes)
    let type_base64 = json_data.get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CertificateError::MissingField("type".to_string()))?;

    let type_bytes = BASE64.decode(type_base64)
        .map_err(|e| CertificateError::InvalidBase64(format!("type: {}", e)))?;

    if type_bytes.len() != 32 {
        return Err(CertificateError::InvalidFormat(
            format!("type must be 32 bytes, got {}", type_bytes.len())
        ));
    }

    // Parse serialNumber (base64, 32 bytes)
    let serial_base64 = json_data.get("serialNumber")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CertificateError::MissingField("serialNumber".to_string()))?;

    let serial_bytes = BASE64.decode(serial_base64)
        .map_err(|e| CertificateError::InvalidBase64(format!("serialNumber: {}", e)))?;

    if serial_bytes.len() != 32 {
        return Err(CertificateError::InvalidFormat(
            format!("serialNumber must be 32 bytes, got {}", serial_bytes.len())
        ));
    }

    // Parse certifier (hex, 33 bytes)
    let certifier_hex = json_data.get("certifier")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CertificateError::MissingField("certifier".to_string()))?;

    let certifier_bytes = hex::decode(certifier_hex)
        .map_err(|e| CertificateError::InvalidHex(format!("certifier: {}", e)))?;

    if certifier_bytes.len() != 33 {
        return Err(CertificateError::InvalidFormat(
            format!("certifier must be 33 bytes, got {}", certifier_bytes.len())
        ));
    }

    // Parse subject (hex, 33 bytes) - optional, may be derived from wallet
    let subject_bytes = if let Some(subject_hex) = json_data.get("subject")
        .and_then(|v| v.as_str()) {
        let bytes = hex::decode(subject_hex)
            .map_err(|e| CertificateError::InvalidHex(format!("subject: {}", e)))?;
        if bytes.len() != 33 {
            return Err(CertificateError::InvalidFormat(
                format!("subject must be 33 bytes, got {}", bytes.len())
            ));
        }
        bytes
    } else {
        // Subject not provided - will need to be set from wallet's identity key
        // For now, return error (caller should provide subject)
        return Err(CertificateError::MissingField("subject".to_string()));
    };

    // Parse revocationOutpoint (format: "txid.vout")
    let revocation_outpoint = json_data.get("revocationOutpoint")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CertificateError::MissingField("revocationOutpoint".to_string()))?
        .to_string();

    // Validate format
    let parts: Vec<&str> = revocation_outpoint.split('.').collect();
    if parts.len() != 2 {
        return Err(CertificateError::InvalidFormat(
            "revocationOutpoint must be 'txid.vout'".to_string()
        ));
    }

    // Validate txid is 32 bytes hex
    let txid_bytes = hex::decode(parts[0])
        .map_err(|e| CertificateError::InvalidHex(format!("revocationOutpoint txid: {}", e)))?;
    if txid_bytes.len() != 32 {
        return Err(CertificateError::InvalidFormat(
            format!("revocationOutpoint txid must be 32 bytes, got {}", txid_bytes.len())
        ));
    }

    // Parse signature (hex, DER-encoded ECDSA)
    let signature_hex = json_data.get("signature")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CertificateError::MissingField("signature".to_string()))?;

    let signature_bytes = hex::decode(signature_hex)
        .map_err(|e| CertificateError::InvalidHex(format!("signature: {}", e)))?;

    if signature_bytes.is_empty() {
        return Err(CertificateError::InvalidFormat("signature cannot be empty".to_string()));
    }

    // Parse fields (map of fieldName -> base64 encrypted value)
    let fields_json = json_data.get("fields")
        .and_then(|v| v.as_object())
        .ok_or_else(|| CertificateError::MissingField("fields".to_string()))?;

    let mut fields = HashMap::new();
    for (field_name, field_value_json) in fields_json {
        // Validate field name length (max 50 bytes per BRC-52)
        if field_name.len() > 50 {
            return Err(CertificateError::InvalidField(
                format!("Field name '{}' exceeds 50 bytes", field_name)
            ));
        }

        // Parse field value (base64 encrypted)
        let field_value_base64 = field_value_json.as_str()
            .ok_or_else(|| CertificateError::InvalidField(
                format!("Field '{}' value must be a string", field_name)
            ))?;

        let field_value_bytes = BASE64.decode(field_value_base64)
            .map_err(|e| CertificateError::InvalidBase64(
                format!("Field '{}' value: {}", field_name, e)
            ))?;

        // Create CertificateField (master_key will be set from keyring)
        let field = CertificateField::new(
            field_name.clone(),
            field_value_bytes,
            vec![], // master_key will be set from keyring
        );

        fields.insert(field_name.clone(), field);
    }

    // Parse keyringForSubject (map of fieldName -> base64 keyring value)
    // NOTE: keyringForSubject is OPTIONAL when receiving a certificate from a certifier
    // It's only present when proving a certificate to a verifier (proveCertificate)
    // When acquiring a certificate, the master keyring is stored separately in the database
    let mut keyring = HashMap::new();

    if let Some(keyring_json) = json_data.get("keyringForSubject")
        .and_then(|v| v.as_object()) {
        // keyringForSubject is present - parse it
        for (field_name, keyring_value_json) in keyring_json {
            // Parse keyring value (base64)
            let keyring_value_base64 = keyring_value_json.as_str()
                .ok_or_else(|| CertificateError::InvalidField(
                    format!("Keyring '{}' value must be a string", field_name)
                ))?;

            let keyring_value_bytes = BASE64.decode(keyring_value_base64)
                .map_err(|e| CertificateError::InvalidBase64(
                    format!("Keyring '{}' value: {}", field_name, e)
                ))?;

            keyring.insert(field_name.clone(), keyring_value_bytes);

            // Update field's master_key
            if let Some(field) = fields.get_mut(field_name) {
                field.master_key = keyring[field_name].clone();
            }
        }

        // Validate that all fields in keyringForSubject have corresponding fields
        for field_name in keyring.keys() {
            if !fields.contains_key(field_name) {
                return Err(CertificateError::InvalidField(
                    format!("Keyring field '{}' has no corresponding certificate field", field_name)
                ));
            }
        }
    } else {
        // keyringForSubject is missing - this is OK for certificates from certifiers
        // The master keyring will be stored separately in the database during acquisition
        // Empty keyring is acceptable - fields will have empty master_key initially
    }

    // Create Certificate struct
    let mut certificate = Certificate::new(
        type_bytes,
        subject_bytes,
        serial_bytes,
        certifier_bytes,
        revocation_outpoint,
        signature_bytes,
        fields,
        keyring,
    );

    // Set verifier if provided (optional validationKey)
    if let Some(verifier_hex) = json_data.get("verifier")
        .or_else(|| json_data.get("validationKey"))
        .and_then(|v| v.as_str()) {
        let verifier_bytes = hex::decode(verifier_hex)
            .map_err(|e| CertificateError::InvalidHex(format!("verifier: {}", e)))?;
        if verifier_bytes.len() == 33 {
            certificate.verifier = Some(verifier_bytes);
        }
    }

    Ok(certificate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_certificate_minimal() {
        let certifier_hex = format!("02{}", "0".repeat(64));  // 33 bytes hex
        let subject_hex = format!("02{}", "0".repeat(64));  // 33 bytes hex

        let json = serde_json::json!({
            "type": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",  // 32 bytes base64
            "serialNumber": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",  // 32 bytes base64
            "certifier": certifier_hex,
            "subject": subject_hex,
            "revocationOutpoint": "0000000000000000000000000000000000000000000000000000000000000000.0",
            "signature": "3006020101020101",  // Minimal DER signature
            "fields": {
                "name": "AQIDBA=="  // base64 encrypted value
            },
            "keyringForSubject": {
                "name": "AQIDBAUGBw=="  // base64 keyring value
            }
        });

        let result = parse_certificate_from_json(&json);
        assert!(result.is_ok());

        let cert = result.unwrap();
        assert_eq!(cert.type_.len(), 32);
        assert_eq!(cert.serial_number.len(), 32);
        assert_eq!(cert.certifier.len(), 33);
        assert_eq!(cert.subject.len(), 33);
        assert_eq!(cert.fields.len(), 1);
        assert!(cert.fields.contains_key("name"));
    }

    #[test]
    fn test_parse_certificate_missing_field() {
        let certifier_hex = format!("02{}", "0".repeat(64));
        let subject_hex = format!("02{}", "0".repeat(64));

        let json = serde_json::json!({
            "type": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            // Missing serialNumber
            "certifier": certifier_hex,
            "subject": subject_hex,
            "revocationOutpoint": "0000000000000000000000000000000000000000000000000000000000000000.0",
            "signature": "3006020101020101",
            "fields": {},
            "keyringForSubject": {}
        });

        let result = parse_certificate_from_json(&json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("serialNumber"));
    }
}
