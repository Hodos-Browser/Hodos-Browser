//! Selective disclosure
//!
//! Generates verifier-specific keyrings for selective disclosure of certificate fields.
//!
//! **BRC-53**: Selective disclosure allows revealing specific certificate fields to verifiers
//! without exposing all data. This is achieved by generating verifier-specific revelation keys.

use crate::certificate::types::{Certificate, CertificateError};
use crate::crypto::brc2::{decrypt_certificate_field, encrypt_certificate_field};
use crate::database::certificate_repo::CertificateRepository;
use rusqlite::Connection;
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Generate keyring for verifier (selective disclosure)
///
/// **BRC-53**: Generates verifier-specific revelation keys for selected fields
///
/// ## Process
/// 1. For each field in `fields_to_reveal`:
///    - Decrypt master keyring entry (encrypted for subject/certifier) using BRC-2
///    - Re-encrypt the field revelation key for verifier using BRC-2
///    - Add to verifier keyring
/// 2. Return keyring with only revealed fields
///
/// ## Arguments
/// - `db_conn`: Database connection
/// - `certificate`: Certificate to generate keyring for
/// - `subject_private_key`: Subject's 32-byte master private key (for decrypting master keyring)
/// - `certifier_public_key`: Certifier's 33-byte compressed public key (used in decryption)
/// - `verifier_public_key`: Verifier's 33-byte compressed public key (for re-encryption)
/// - `fields_to_reveal`: List of field names to include in keyring
/// - `serial_number_base64`: Certificate serial number (base64-encoded, for invoice number)
///
/// ## Returns
/// Map of fieldName → verifier-specific keyring value (base64-encoded encrypted field revelation key)
pub fn create_keyring_for_verifier(
    db_conn: &Connection,
    certificate: &Certificate,
    subject_private_key: &[u8],
    certifier_public_key: &[u8],
    verifier_public_key: &[u8],
    fields_to_reveal: &[String],
    serial_number_base64: &str,
) -> Result<HashMap<String, String>, CertificateError> {
    // Validate inputs
    if fields_to_reveal.is_empty() {
        return Err(CertificateError::InvalidFormat(
            "fieldsToReveal must not be empty".to_string()
        ));
    }

    // Get master keyring from database
    let cert_repo = CertificateRepository::new(db_conn);
    let certificate_fields = cert_repo.get_certificate_fields(certificate.certificate_id.unwrap())
        .map_err(|e| CertificateError::Database(format!("Failed to get certificate fields: {}", e)))?;

    // Build master keyring map (fieldName → encrypted master key)
    let mut master_keyring: HashMap<String, Vec<u8>> = HashMap::new();
    for (field_name, field) in certificate_fields {
        master_keyring.insert(field_name.clone(), field.master_key.clone());
    }

    // Generate verifier keyring
    let mut verifier_keyring: HashMap<String, String> = HashMap::new();

    for field_name in fields_to_reveal {
        // Validate field exists
        if !certificate.fields.contains_key(field_name) {
            return Err(CertificateError::InvalidFormat(
                format!("Field '{}' does not exist in certificate", field_name)
            ));
        }

        // Get encrypted master key for this field
        let encrypted_master_key = master_keyring.get(field_name)
            .ok_or_else(|| CertificateError::InvalidFormat(
                format!("Master keyring missing for field '{}'", field_name)
            ))?;

        // Step 1: Get the field revelation key from the master keyring.
        //
        // The master keyring entry is EITHER:
        // a) A raw symmetric key (32 bytes) — used by some certifiers (e.g., SocialCert)
        // b) A BRC-2 encrypted symmetric key (≥48 bytes: 32 IV + ciphertext + 16 tag)
        //
        // We try BRC-2 decryption first; if the data is too short, use it as a raw key.
        let field_revelation_key = if encrypted_master_key.len() < 48 {
            // Raw symmetric key — use directly (no decryption needed)
            log::info!("   ℹ️  Master key for field '{}' is {} bytes (raw key, not BRC-2 encrypted)",
                field_name, encrypted_master_key.len());
            encrypted_master_key.clone()
        } else {
            // BRC-2 encrypted — decrypt it
            // Invoice number: "2-certificate field encryption-{fieldName}" (no serialNumber for master keyring)
            decrypt_certificate_field(
                subject_private_key,
                certifier_public_key,
                field_name,
                None, // Master keyring uses fieldName only (no serialNumber)
                encrypted_master_key,
            ).map_err(|e| CertificateError::Database(
                format!("Failed to decrypt master key for field '{}': {}", field_name, e)
            ))?
        };

        // Step 2: Re-encrypt field revelation key for verifier
        // Invoice number: "2-certificate field encryption-{serialNumber} {fieldName}"
        let encrypted_for_verifier = encrypt_certificate_field(
            subject_private_key,
            verifier_public_key,
            field_name,
            Some(serial_number_base64),
            &field_revelation_key,
        ).map_err(|e| CertificateError::Database(
            format!("Failed to encrypt key for verifier (field '{}'): {}", field_name, e)
        ))?;

        // Step 3: Add to verifier keyring (base64-encoded)
        verifier_keyring.insert(field_name.clone(), BASE64.encode(&encrypted_for_verifier));
    }

    Ok(verifier_keyring)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::certificate::test_utils;
    use crate::crypto::brc2::encrypt_certificate_field;
    use rusqlite::Connection;
    use crate::database::migrations;

    /// Setup test database with certificate
    fn setup_test_db_with_certificate() -> (Connection, Certificate, Vec<u8>, Vec<u8>, Vec<u8>) {
        use secp256k1::{Secp256k1, SecretKey, PublicKey};

        // Create in-memory database
        let conn = Connection::open_in_memory().unwrap();

        // Run consolidated schema migration
        migrations::create_schema_v1(&conn).unwrap();

        // Generate valid test keys using secp256k1
        let secp = Secp256k1::new();

        // Subject private key (for decrypting master keyring)
        let subject_seckey = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let subject_private_key = subject_seckey.secret_bytes().to_vec();
        let subject_pubkey = PublicKey::from_secret_key(&secp, &subject_seckey);
        let subject_public_key_bytes = subject_pubkey.serialize();

        // Certifier public key
        let certifier_seckey = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let certifier_pubkey = PublicKey::from_secret_key(&secp, &certifier_seckey);
        let certifier_public_key = certifier_pubkey.serialize().to_vec();

        // Verifier public key
        let verifier_seckey = SecretKey::from_slice(&[3u8; 32]).unwrap();
        let verifier_pubkey = PublicKey::from_secret_key(&secp, &verifier_seckey);
        let verifier_public_key = verifier_pubkey.serialize().to_vec();

        // Create test certificate with encrypted fields
        let mut fields = std::collections::HashMap::new();
        let field_name = "name";
        let plaintext_value = b"Alice".to_vec();

        // Encrypt field value for subject/certifier (master keyring)
        let encrypted_value = encrypt_certificate_field(
            &subject_private_key,
            &certifier_public_key,
            field_name,
            None, // Master keyring uses fieldName only
            &plaintext_value,
        ).unwrap();

        // Create master keyring entry (encrypted field revelation key)
        // For testing, we'll use the plaintext value as the "revelation key"
        // In reality, this would be a symmetric key used to encrypt the field
        let field_revelation_key = vec![0xAA; 32]; // Test revelation key
        let encrypted_master_key = encrypt_certificate_field(
            &subject_private_key,
            &certifier_public_key,
            field_name,
            None, // Master keyring
            &field_revelation_key,
        ).unwrap();

        fields.insert(field_name.to_string(), crate::certificate::types::CertificateField::new(
            field_name.to_string(),
            encrypted_value,
            encrypted_master_key,
        ));

        // Create certificate with required fields
        let test_txid = "0000000000000000000000000000000000000000000000000000000000000000";
        let mut certificate = Certificate::new(
            vec![0u8; 32], // type
            subject_public_key_bytes.to_vec(), // subject (matches subject_private_key)
            vec![0u8; 32], // serial_number
            certifier_public_key.clone(),
            format!("{}.0", test_txid), // revocation_outpoint
            vec![], // signature
            fields,
            std::collections::HashMap::new(),
        );

        // Insert certificate into database
        use crate::database::certificate_repo::CertificateRepository;
        let cert_repo = CertificateRepository::new(&conn);
        let certificate_id = cert_repo.insert_certificate_with_fields(&mut certificate).unwrap();
        certificate.certificate_id = Some(certificate_id);

        (conn, certificate, subject_private_key, certifier_public_key, verifier_public_key)
    }

    #[test]
    fn test_create_keyring_single_field() {
        let (conn, certificate, subject_privkey, certifier_pubkey, verifier_pubkey) =
            setup_test_db_with_certificate();

        let serial_b64 = base64::engine::general_purpose::STANDARD.encode(&certificate.serial_number);
        let fields_to_reveal = vec!["name".to_string()];

        let result = create_keyring_for_verifier(
            &conn,
            &certificate,
            &subject_privkey,
            &certifier_pubkey,
            &verifier_pubkey,
            &fields_to_reveal,
            &serial_b64,
        );

        match &result {
            Ok(_) => {},
            Err(e) => {
                panic!("Keyring generation should succeed, but got error: {}", e);
            }
        }
        let keyring = result.unwrap();

        // Verify keyring contains only the revealed field
        assert_eq!(keyring.len(), 1, "Keyring should contain exactly 1 field");
        assert!(keyring.contains_key("name"), "Keyring should contain 'name' field");

        // Verify keyring value is base64-encoded
        let keyring_value = keyring.get("name").unwrap();
        assert!(!keyring_value.is_empty(), "Keyring value should not be empty");

        // Verify it's valid base64
        let decoded = base64::engine::general_purpose::STANDARD.decode(keyring_value);
        assert!(decoded.is_ok(), "Keyring value should be valid base64");
    }

    #[test]
    fn test_create_keyring_invalid_field() {
        let (conn, certificate, subject_privkey, certifier_pubkey, verifier_pubkey) =
            setup_test_db_with_certificate();

        let serial_b64 = base64::engine::general_purpose::STANDARD.encode(&certificate.serial_number);
        let fields_to_reveal = vec!["nonexistent".to_string()];

        let result = create_keyring_for_verifier(
            &conn,
            &certificate,
            &subject_privkey,
            &certifier_pubkey,
            &verifier_pubkey,
            &fields_to_reveal,
            &serial_b64,
        );

        assert!(result.is_err(), "Should fail for non-existent field");
        let error = result.unwrap_err();
        assert!(error.to_string().contains("nonexistent"),
            "Error should mention the field name");
    }

    #[test]
    fn test_create_keyring_empty_fields() {
        let (conn, certificate, subject_privkey, certifier_pubkey, verifier_pubkey) =
            setup_test_db_with_certificate();

        let serial_b64 = base64::engine::general_purpose::STANDARD.encode(&certificate.serial_number);
        let fields_to_reveal = vec![];

        let result = create_keyring_for_verifier(
            &conn,
            &certificate,
            &subject_privkey,
            &certifier_pubkey,
            &verifier_pubkey,
            &fields_to_reveal,
            &serial_b64,
        );

        assert!(result.is_err(), "Should fail for empty fields list");
        let error = result.unwrap_err();
        assert!(error.to_string().contains("fieldsToReveal"),
            "Error should mention fieldsToReveal");
    }
}
