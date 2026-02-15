//! Certificate data structures
//!
//! Defines the core data structures for BRC-52 identity certificates.

use std::collections::HashMap;
use thiserror::Error;

/// Certificate model matching the `certificates` table
///
/// **BRC-52 Structure**: Represents a BRC-52 identity certificate
/// **Database**: Maps to `certificates` table (toolbox-aligned V1 schema)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    /// Database ID (None for new certificates)
    pub certificate_id: Option<i64>,

    /// User ID (FK to users table)
    pub user_id: Option<i64>,

    /// Certificate type identifier (base64-encoded)
    /// **BRC-52**: `type` field
    pub type_: Vec<u8>,

    /// Subject's identity key (33-byte compressed public key, hex-encoded)
    /// **BRC-52**: `subject` field
    pub subject: Vec<u8>,

    /// Unique serial number (base64-encoded)
    /// **BRC-52**: `serialNumber` field
    pub serial_number: Vec<u8>,

    /// Certifier's public key (33-byte compressed public key, hex-encoded)
    /// **BRC-52**: `certifier` field
    pub certifier: Vec<u8>,

    /// Optional verifier's public key (33-byte compressed public key, hex-encoded)
    /// **BRC-52**: `validationKey` field (optional)
    /// **Database**: Stored as `verifier`
    pub verifier: Option<Vec<u8>>,

    /// Revocation outpoint (format: "txid.vout")
    /// **BRC-52**: `revocationOutpoint` field
    pub revocation_outpoint: String,

    /// Certificate signature (DER-encoded ECDSA signature, hex-encoded)
    /// **BRC-52**: `signature` field
    pub signature: Vec<u8>,

    /// Certificate fields (fieldName → encrypted fieldValue)
    /// **BRC-52**: `fields` map
    /// **Database**: Stored in separate `certificate_fields` table
    pub fields: HashMap<String, CertificateField>,

    /// Master keyring (fieldName → master keyring value)
    /// **BRC-52**: `keyring` map
    /// **Database**: Stored as `master_key` in `certificate_fields` table
    pub keyring: HashMap<String, Vec<u8>>,

    /// Soft delete flag (true if certificate is relinquished)
    /// **Database**: `is_deleted` column
    pub is_deleted: bool,

    /// Created timestamp (Unix timestamp)
    pub created_at: i64,

    /// Updated timestamp (Unix timestamp)
    pub updated_at: i64,
}

/// Certificate field model matching the `certificate_fields` table
///
/// **BRC-52 Structure**: Represents a single encrypted field in a certificate
/// **Database**: Maps to `certificate_fields` table (composite key: certificateId + field_name)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateField {
    /// Foreign key to `certificates` table
    pub certificate_id: Option<i64>,

    /// User ID (FK to users table)
    pub user_id: Option<i64>,

    /// Field name (e.g., "name", "email", "age")
    pub field_name: String,

    /// Encrypted field value (base64-encoded)
    /// **BRC-52**: Encrypted using BRC-2 (AES-256-GCM)
    pub field_value: Vec<u8>,  // Base64-decoded bytes

    /// Master keyring value for this field (base64-encoded)
    /// **BRC-52**: Used for selective disclosure
    pub master_key: Vec<u8>,  // Base64-decoded bytes

    /// Timestamp when field was created (Unix timestamp)
    pub created_at: i64,

    /// Timestamp when field was last updated (Unix timestamp)
    pub updated_at: i64,
}

/// Certificate-related errors
#[derive(Debug, Error)]
pub enum CertificateError {
    #[error("Invalid certificate format: {0}")]
    InvalidFormat(String),

    #[error("Invalid field: {0}")]
    InvalidField(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid base64 encoding: {0}")]
    InvalidBase64(String),

    #[error("Invalid hex encoding: {0}")]
    InvalidHex(String),

    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Signature verification failed: {0}")]
    SignatureVerification(String),

    #[error("Certificate is revoked")]
    Revoked,

    #[error("Certificate is relinquished")]
    Relinquished,
}

impl Certificate {
    /// Create a new certificate (for database insertion)
    pub fn new(
        type_: Vec<u8>,
        subject: Vec<u8>,
        serial_number: Vec<u8>,
        certifier: Vec<u8>,
        revocation_outpoint: String,
        signature: Vec<u8>,
        fields: HashMap<String, CertificateField>,
        keyring: HashMap<String, Vec<u8>>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        Self {
            certificate_id: None,
            user_id: None,
            type_,
            subject,
            serial_number,
            certifier,
            verifier: None,
            revocation_outpoint,
            signature,
            fields,
            keyring,
            is_deleted: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get certificate identifier tuple (type, serialNumber, certifier)
    /// Used for unique identification and database lookups
    pub fn identifier(&self) -> (&[u8], &[u8], &[u8]) {
        (&self.type_, &self.serial_number, &self.certifier)
    }

    /// Check if certificate is active (not deleted/relinquished)
    pub fn is_active(&self) -> bool {
        !self.is_deleted
    }
}

impl CertificateField {
    /// Create a new certificate field
    pub fn new(
        field_name: String,
        field_value: Vec<u8>,
        master_key: Vec<u8>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        Self {
            certificate_id: None,
            user_id: None,
            field_name,
            field_value,
            master_key,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certificate_new() {
        let cert = Certificate::new(
            b"test_type".to_vec(),
            vec![0x02; 33],  // Dummy 33-byte pubkey
            b"serial123".to_vec(),
            vec![0x03; 33],  // Dummy 33-byte pubkey
            "txid.0".to_string(),
            vec![0x01, 0x02, 0x03],  // Dummy signature
            HashMap::new(),
            HashMap::new(),
        );

        assert_eq!(cert.type_, b"test_type");
        assert_eq!(cert.subject.len(), 33);
        assert!(!cert.is_deleted);
        assert!(cert.is_active());
    }

    #[test]
    fn test_certificate_identifier() {
        let cert = Certificate::new(
            b"type1".to_vec(),
            vec![0x02; 33],
            b"serial1".to_vec(),
            vec![0x03; 33],
            "txid.0".to_string(),
            vec![0x01],
            HashMap::new(),
            HashMap::new(),
        );

        let (type_, serial, certifier) = cert.identifier();
        assert_eq!(type_, b"type1");
        assert_eq!(serial, b"serial1");
        assert_eq!(certifier.len(), 33);
    }

    #[test]
    fn test_certificate_field_new() {
        let field = CertificateField::new(
            "name".to_string(),
            b"encrypted_value".to_vec(),
            b"master_key".to_vec(),
        );

        assert_eq!(field.field_name, "name");
        assert_eq!(field.field_value, b"encrypted_value");
        assert_eq!(field.master_key, b"master_key");
        assert!(field.created_at > 0);
    }
}
