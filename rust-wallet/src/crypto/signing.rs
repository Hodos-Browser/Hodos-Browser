//! ECDSA Signing Operations
//!
//! Bitcoin ECDSA signature generation using secp256k1.
//! Produces DER-encoded signatures with sighash type byte appended.
//!
//! **Reference**: TypeScript bsv-sdk ECDSA signing

use secp256k1::{Secp256k1, Message, SecretKey, PublicKey, ecdsa::Signature};
use sha2::{Sha256, Digest};
use hmac::{Hmac, Mac};

/// Signing errors
#[derive(Debug, thiserror::Error)]
pub enum SigningError {
    #[error("invalid private key: {0}")]
    InvalidPrivateKey(String),

    #[error("invalid message hash: {0}")]
    InvalidMessage(String),

    #[error("invalid signature: {0}")]
    InvalidSignature(String),
}

/// Sign a hash with a private key using ECDSA
///
/// **Reference**: TypeScript `PrivateKey.sign(hash)`
///
/// ## Arguments
/// - `sighash`: 32-byte hash to sign (from SigHash::calculate)
/// - `private_key_bytes`: 32-byte private key
/// - `sighash_type_byte`: Sighash type to append (typically 0x01 for SIGHASH_ALL)
///
/// ## Returns
/// DER-encoded signature with sighash type byte appended
pub fn sign_ecdsa(
    sighash: &[u8],
    private_key_bytes: &[u8],
    sighash_type_byte: u8,
) -> Result<Vec<u8>, SigningError> {
    // Validate input lengths
    if sighash.len() != 32 {
        return Err(SigningError::InvalidMessage(
            format!("Sighash must be 32 bytes, got {}", sighash.len())
        ));
    }

    if private_key_bytes.len() != 32 {
        return Err(SigningError::InvalidPrivateKey(
            format!("Private key must be 32 bytes, got {}", private_key_bytes.len())
        ));
    }

    // Create secp256k1 context
    let secp = Secp256k1::new();

    // Parse private key
    let secret_key = SecretKey::from_slice(private_key_bytes)
        .map_err(|e| SigningError::InvalidPrivateKey(e.to_string()))?;

    // Parse message (sighash)
    let message = Message::from_digest_slice(sighash)
        .map_err(|e| SigningError::InvalidMessage(e.to_string()))?;

    // Sign
    let signature = secp.sign_ecdsa(&message, &secret_key);

    // Serialize to DER format
    let der_bytes = signature.serialize_der();

    // Append sighash type byte
    let mut result = der_bytes.to_vec();
    result.push(sighash_type_byte);

    Ok(result)
}

/// Verify an ECDSA signature
///
/// **Reference**: TypeScript `PublicKey.verify(hash, signature)`
///
/// ## Arguments
/// - `sighash`: 32-byte hash that was signed
/// - `signature_with_type`: DER-encoded signature with sighash type byte
/// - `public_key_bytes`: 33-byte compressed public key
///
/// ## Returns
/// true if signature is valid
pub fn verify_signature(
    sighash: &[u8],
    signature_with_type: &[u8],
    public_key_bytes: &[u8],
) -> Result<bool, SigningError> {
    if sighash.len() != 32 {
        return Err(SigningError::InvalidMessage(
            format!("Sighash must be 32 bytes, got {}", sighash.len())
        ));
    }

    if signature_with_type.is_empty() {
        return Err(SigningError::InvalidSignature("Empty signature".to_string()));
    }

    // Remove sighash type byte (last byte)
    let der_bytes = &signature_with_type[..signature_with_type.len() - 1];

    // Create secp256k1 context
    let secp = Secp256k1::new();

    // Parse public key
    let public_key = PublicKey::from_slice(public_key_bytes)
        .map_err(|e| SigningError::InvalidSignature(e.to_string()))?;

    // Parse signature
    let signature = Signature::from_der(der_bytes)
        .map_err(|e| SigningError::InvalidSignature(e.to_string()))?;

    // Parse message
    let message = Message::from_digest_slice(sighash)
        .map_err(|e| SigningError::InvalidMessage(e.to_string()))?;

    // Verify
    Ok(secp.verify_ecdsa(&message, &signature, &public_key).is_ok())
}

/// Hash data with SHA-256
///
/// **Reference**: TypeScript `Hash.sha256(data)`
pub fn sha256(data: &[u8]) -> Vec<u8> {
    Sha256::digest(data).to_vec()
}

/// Double SHA-256 hash (SHA-256 twice)
///
/// **Reference**: TypeScript `Hash.sha256sha256(data)`
pub fn double_sha256(data: &[u8]) -> Vec<u8> {
    let hash1 = Sha256::digest(data);
    Sha256::digest(&hash1).to_vec()
}

/// Create HMAC-SHA256 from key and data
///
/// **Reference**: TypeScript `Hash.sha256hmac(key, data)`
///
/// ## Arguments
/// - `key`: HMAC key bytes
/// - `data`: Data to authenticate
///
/// ## Returns
/// 32-byte HMAC-SHA256
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Verify HMAC-SHA256
///
/// **Reference**: TypeScript HMAC verification
///
/// ## Arguments
/// - `key`: HMAC key bytes
/// - `data`: Data that was authenticated
/// - `expected_hmac`: HMAC to verify
///
/// ## Returns
/// true if HMAC is valid
pub fn verify_hmac_sha256(key: &[u8], data: &[u8], expected_hmac: &[u8]) -> bool {
    let computed = hmac_sha256(key, data);

    // Constant-time comparison
    if computed.len() != expected_hmac.len() {
        return false;
    }

    computed.iter()
        .zip(expected_hmac.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::keys::derive_public_key;

    #[test]
    fn test_sign_ecdsa_basic() {
        // TS Reference: Basic ECDSA signing
        let private_key = [1u8; 32]; // Simple test key
        let sighash = [2u8; 32];      // Simple test hash

        let signature = sign_ecdsa(&sighash, &private_key, 0x01).unwrap();

        // Signature should be DER-encoded + 1 byte sighash type
        // DER signatures are typically 70-72 bytes + 1 = 71-73 bytes
        assert!(signature.len() >= 71 && signature.len() <= 73);

        // Last byte should be sighash type
        assert_eq!(signature[signature.len() - 1], 0x01);
    }

    #[test]
    fn test_sign_and_verify() {
        // TS Reference: Sign and verify roundtrip
        let private_key = [1u8; 32];
        let sighash = [2u8; 32];

        // Sign
        let signature = sign_ecdsa(&sighash, &private_key, 0x01).unwrap();

        // Get public key
        let public_key = derive_public_key(&private_key).unwrap();

        // Verify
        let valid = verify_signature(&sighash, &signature, &public_key).unwrap();
        assert!(valid);
    }

    #[test]
    fn test_sign_invalid_hash_length() {
        // TS Reference: Validation of hash length
        let private_key = [1u8; 32];
        let invalid_hash = [2u8; 31]; // Wrong length

        let result = sign_ecdsa(&invalid_hash, &private_key, 0x01);
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_invalid_key_length() {
        // TS Reference: Validation of private key length
        let invalid_key = [1u8; 31]; // Wrong length
        let sighash = [2u8; 32];

        let result = sign_ecdsa(&sighash, &invalid_key, 0x01);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_invalid_signature() {
        // TS Reference: Invalid signature verification
        let sighash = [2u8; 32];
        let invalid_sig = vec![0u8; 73]; // Invalid DER signature
        let public_key = [3u8; 33];

        let result = verify_signature(&sighash, &invalid_sig, &public_key);
        // Should either return false or error
        assert!(result.is_err() || !result.unwrap());
    }

    #[test]
    fn test_sha256() {
        // TS Reference: SHA-256 hash
        let data = b"hello world";
        let hash = sha256(data);

        assert_eq!(hash.len(), 32);

        // Known SHA-256 hash of "hello world"
        let expected = hex::decode("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9").unwrap();
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_double_sha256() {
        // TS Reference: Double SHA-256 (used for txid)
        let data = b"hello world";
        let hash = double_sha256(data);

        assert_eq!(hash.len(), 32);

        // Should be different from single SHA-256
        let single_hash = sha256(data);
        assert_ne!(hash, single_hash);
    }

    #[test]
    fn test_signature_deterministic() {
        // TS Reference: Signatures should be deterministic (RFC 6979)
        let private_key = [1u8; 32];
        let sighash = [2u8; 32];

        let sig1 = sign_ecdsa(&sighash, &private_key, 0x01).unwrap();
        let sig2 = sign_ecdsa(&sighash, &private_key, 0x01).unwrap();

        // Should produce identical signatures
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_hmac_sha256_basic() {
        // TS Reference: HMAC-SHA256 creation
        let key = b"secret_key";
        let data = b"message to authenticate";

        let hmac = hmac_sha256(key, data);

        // HMAC-SHA256 should be 32 bytes
        assert_eq!(hmac.len(), 32);
    }

    #[test]
    fn test_hmac_verify_valid() {
        // TS Reference: HMAC verification success
        let key = b"secret_key";
        let data = b"message";

        let hmac = hmac_sha256(key, data);
        let valid = verify_hmac_sha256(key, data, &hmac);

        assert!(valid);
    }

    #[test]
    fn test_hmac_verify_invalid_wrong_key() {
        // TS Reference: HMAC verification failure (wrong key)
        let key1 = b"secret_key_1";
        let key2 = b"secret_key_2";
        let data = b"message";

        let hmac = hmac_sha256(key1, data);
        let valid = verify_hmac_sha256(key2, data, &hmac);

        assert!(!valid);
    }

    #[test]
    fn test_hmac_verify_invalid_wrong_data() {
        // TS Reference: HMAC verification failure (wrong data)
        let key = b"secret_key";
        let data1 = b"message1";
        let data2 = b"message2";

        let hmac = hmac_sha256(key, data1);
        let valid = verify_hmac_sha256(key, data2, &hmac);

        assert!(!valid);
    }

    #[test]
    fn test_hmac_deterministic() {
        // TS Reference: HMAC should be deterministic
        let key = b"key";
        let data = b"data";

        let hmac1 = hmac_sha256(key, data);
        let hmac2 = hmac_sha256(key, data);

        assert_eq!(hmac1, hmac2);
    }
}
