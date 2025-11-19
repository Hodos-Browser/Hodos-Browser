//! Key Derivation Operations
//!
//! Public key derivation from private keys using secp256k1.
//!
//! **Reference**: TypeScript bsv-sdk PrivateKey/PublicKey classes

use secp256k1::{Secp256k1, SecretKey, PublicKey};

/// Key derivation errors
#[derive(Debug, thiserror::Error)]
pub enum KeyDerivationError {
    #[error("invalid private key: {0}")]
    InvalidPrivateKey(String),
}

/// Derive compressed public key from private key
///
/// **Reference**: TypeScript `PrivateKey.toPublicKey()`
///
/// ## Arguments
/// - `private_key_bytes`: 32-byte private key
///
/// ## Returns
/// 33-byte compressed public key (02/03 prefix + 32-byte x coordinate)
pub fn derive_public_key(private_key_bytes: &[u8]) -> Result<Vec<u8>, KeyDerivationError> {
    if private_key_bytes.len() != 32 {
        return Err(KeyDerivationError::InvalidPrivateKey(
            format!("Private key must be 32 bytes, got {}", private_key_bytes.len())
        ));
    }

    // Create secp256k1 context
    let secp = Secp256k1::new();

    // Parse private key
    let secret_key = SecretKey::from_slice(private_key_bytes)
        .map_err(|e| KeyDerivationError::InvalidPrivateKey(e.to_string()))?;

    // Derive public key
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // Serialize as compressed (33 bytes)
    Ok(public_key.serialize().to_vec())
}

/// Derive uncompressed public key from private key
///
/// **Reference**: TypeScript `PrivateKey.toPublicKey().toUncompressed()`
///
/// ## Returns
/// 65-byte uncompressed public key (04 prefix + 32-byte x + 32-byte y)
pub fn derive_public_key_uncompressed(private_key_bytes: &[u8]) -> Result<Vec<u8>, KeyDerivationError> {
    if private_key_bytes.len() != 32 {
        return Err(KeyDerivationError::InvalidPrivateKey(
            format!("Private key must be 32 bytes, got {}", private_key_bytes.len())
        ));
    }

    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(private_key_bytes)
        .map_err(|e| KeyDerivationError::InvalidPrivateKey(e.to_string()))?;

    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // Serialize as uncompressed (65 bytes)
    Ok(public_key.serialize_uncompressed().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_public_key_compressed() {
        // TS Reference: Compressed public key derivation
        let private_key = [1u8; 32];
        let public_key = derive_public_key(&private_key).unwrap();

        // Compressed public key should be 33 bytes
        assert_eq!(public_key.len(), 33);

        // First byte should be 0x02 or 0x03
        assert!(public_key[0] == 0x02 || public_key[0] == 0x03);
    }

    #[test]
    fn test_derive_public_key_uncompressed() {
        // TS Reference: Uncompressed public key derivation
        let private_key = [1u8; 32];
        let public_key = derive_public_key_uncompressed(&private_key).unwrap();

        // Uncompressed public key should be 65 bytes
        assert_eq!(public_key.len(), 65);

        // First byte should be 0x04
        assert_eq!(public_key[0], 0x04);
    }

    #[test]
    fn test_derive_invalid_key_length() {
        // TS Reference: Validation of private key length
        let invalid_key = [1u8; 31];
        let result = derive_public_key(&invalid_key);

        assert!(result.is_err());
    }

    #[test]
    fn test_derive_deterministic() {
        // TS Reference: Public key derivation should be deterministic
        let private_key = [1u8; 32];

        let pubkey1 = derive_public_key(&private_key).unwrap();
        let pubkey2 = derive_public_key(&private_key).unwrap();

        assert_eq!(pubkey1, pubkey2);
    }

    #[test]
    fn test_different_private_keys_different_public_keys() {
        // TS Reference: Different private keys produce different public keys
        let private_key1 = [1u8; 32];
        let private_key2 = [2u8; 32];

        let pubkey1 = derive_public_key(&private_key1).unwrap();
        let pubkey2 = derive_public_key(&private_key2).unwrap();

        assert_ne!(pubkey1, pubkey2);
    }
}
