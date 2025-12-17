//! BRC-2: Data Encryption and Decryption Implementation
//!
//! Implements the BRC-2 specification for encrypting data using keys derived from
//! the BRC-42 key derivation scheme and BRC-43 invoice numbers.
//!
//! **Reference**: BRC-2 specification
//! https://bsv.brc.dev/wallet/0002
//!
//! **Process**:
//! 1. Create BRC-43 invoice number from protocolID and keyID
//! 2. Use BRC-42 to derive child keys (public/private) using invoice number
//! 3. Compute ECDH shared secret from child keys
//! 4. Extract x-coordinate from shared secret point → 32-byte symmetric key
//! 5. Encrypt data with AES-256-GCM using symmetric key
//! 6. Format: [32-byte IV][ciphertext][16-byte auth tag]

use secp256k1::{Secp256k1, PublicKey};
use hex;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::crypto::brc42;
use crate::crypto::brc43::{InvoiceNumber, SecurityLevel};
use crate::crypto::aesgcm_custom;
use thiserror::Error;

/// BRC-2 encryption errors
#[derive(Debug, Error)]
pub enum Brc2Error {
    #[error("Invalid private key: {0}")]
    InvalidPrivateKey(String),

    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("Invalid invoice number: {0}")]
    InvalidInvoiceNumber(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivationFailed(String),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid ciphertext: {0}")]
    InvalidCiphertext(String),

    #[error("AES-GCM error: {0}")]
    AesGcmError(String),
}

/// Derive symmetric key from sender and recipient keys using BRC-42
///
/// **BRC-2 Process**:
/// 1. Derive child public key for recipient using BRC-42
/// 2. Derive child private key for sender using BRC-42
/// 3. Compute ECDH shared secret: childPrivateKey * childPublicKey
/// 4. Extract x-coordinate from shared secret point → 32-byte symmetric key
///
/// ## Arguments
/// - `sender_private_key`: Sender's 32-byte master private key
/// - `recipient_public_key`: Recipient's 33-byte compressed public key
/// - `invoice_number`: BRC-43 invoice number string
///
/// ## Returns
/// 32-byte symmetric key for AES-256-GCM encryption
pub fn derive_symmetric_key(
    sender_private_key: &[u8],
    recipient_public_key: &[u8],
    invoice_number: &str,
) -> Result<Vec<u8>, Brc2Error> {
    log::info!("   🔐 BRC-2 derive_symmetric_key:");
    log::info!("      Invoice number: {}", invoice_number);
    log::info!("      Sender private key (hex, first 16): {}", hex::encode(&sender_private_key[..16]));
    log::info!("      Recipient public key (hex): {}", hex::encode(recipient_public_key));
    // 1. Derive child public key for recipient
    let child_pubkey = brc42::derive_child_public_key(
        sender_private_key,
        recipient_public_key,
        invoice_number,
    ).map_err(|e| Brc2Error::KeyDerivationFailed(format!("Failed to derive child public key: {}", e)))?;

    // 2. Derive child private key for sender
    let child_privkey = brc42::derive_child_private_key(
        sender_private_key,
        recipient_public_key,
        invoice_number,
    ).map_err(|e| Brc2Error::KeyDerivationFailed(format!("Failed to derive child private key: {}", e)))?;

    // 3. Compute ECDH shared secret
    let shared_secret = brc42::compute_shared_secret(
        &child_privkey,
        &child_pubkey,
    ).map_err(|e| Brc2Error::KeyDerivationFailed(format!("Failed to compute shared secret: {}", e)))?;

    // 4. Extract x-coordinate from shared secret point
    // shared_secret is 33-byte compressed point: [prefix][x-coord]
    // The x-coordinate is in bytes 1..33 (last 32 bytes)
    // TypeScript: sharedSecret.x.toArray() returns 32-byte x-coordinate
    let secp = Secp256k1::new();
    let shared_point = PublicKey::from_slice(&shared_secret)
        .map_err(|e| Brc2Error::KeyDerivationFailed(format!("Failed to parse shared secret point: {}", e)))?;

    // Decompress to get full point, then extract x-coordinate
    // Uncompressed format: [0x04][x-coord (32 bytes)][y-coord (32 bytes)]
    // Extract x-coordinate (bytes 1..33)
    let uncompressed = shared_point.serialize_uncompressed();
    let symmetric_key = uncompressed[1..33].to_vec();

    log::info!("      Shared secret (hex, first 16): {}", hex::encode(&shared_secret[..16]));
    log::info!("      Symmetric key (hex, first 16): {}", hex::encode(&symmetric_key[..16]));

    Ok(symmetric_key)
}

/// Encrypt data using BRC-2 (AES-256-GCM)
///
/// **BRC-2 Format**: [32-byte IV][ciphertext][16-byte auth tag]
///
/// ## Arguments
/// - `plaintext`: Data to encrypt
/// - `symmetric_key`: 32-byte AES key (from `derive_symmetric_key`)
///
/// ## Returns
/// Ciphertext with IV prepended: [32-byte IV][ciphertext][16-byte tag]
pub fn encrypt_brc2(
    plaintext: &[u8],
    symmetric_key: &[u8],
) -> Result<Vec<u8>, Brc2Error> {
    if symmetric_key.len() != 32 {
        return Err(Brc2Error::InvalidCiphertext(
            format!("Symmetric key must be 32 bytes, got {}", symmetric_key.len())
        ));
    }

    // 1. Generate random 32-byte IV
    let mut iv_bytes = [0u8; 32];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut iv_bytes);

    // 2. Use custom AESGCM matching TypeScript SDK exactly
    let key_array: [u8; 32] = symmetric_key.try_into()
        .map_err(|_| Brc2Error::InvalidCiphertext("Invalid key length".to_string()))?;

    log::info!("   🔐 Custom AESGCM encryption:");
    log::info!("      Plaintext length: {} bytes", plaintext.len());
    log::info!("      Plaintext (hex, first 32): {}", hex::encode(&plaintext[..plaintext.len().min(32)]));
    log::info!("      IV (hex): {}", hex::encode(&iv_bytes));
    log::info!("      Key (hex, first 16): {}", hex::encode(&symmetric_key[..16]));

    let (ciphertext, auth_tag) = aesgcm_custom::aesgcm_custom(
        plaintext,
        &[],  // Additional authenticated data (empty for BRC-2)
        &iv_bytes,
        &key_array,
    ).map_err(|e| Brc2Error::EncryptionFailed(format!("Custom AESGCM encryption failed: {}", e)))?;

    log::info!("      Ciphertext length: {} bytes", ciphertext.len());
    log::info!("      Ciphertext (hex, first 32): {}", hex::encode(&ciphertext[..ciphertext.len().min(32)]));
    log::info!("      Auth tag length: {} bytes", auth_tag.len());
    log::info!("      Auth tag (hex): {}", hex::encode(&auth_tag));
    log::info!("      Total output length: {} bytes (32 IV + {} ciphertext + 16 tag)", 32 + ciphertext.len() + 16, ciphertext.len());

    // 3. Format: [32-byte IV][ciphertext][16-byte auth tag]
    let mut result = iv_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    result.extend_from_slice(&auth_tag);

    log::info!("      Final encrypted data length: {} bytes", result.len());
    log::info!("      Final encrypted data (hex, first 64): {}", hex::encode(&result[..result.len().min(64)]));

    Ok(result)
}

/// Decrypt data using BRC-2 (AES-256-GCM)
///
/// **BRC-2 Format**: [32-byte IV][ciphertext][16-byte auth tag]
///
/// ## Arguments
/// - `ciphertext_with_iv`: Ciphertext with IV prepended
/// - `symmetric_key`: 32-byte AES key (from `derive_symmetric_key`)
///
/// ## Returns
/// Decrypted plaintext
pub fn decrypt_brc2(
    ciphertext_with_iv: &[u8],
    symmetric_key: &[u8],
) -> Result<Vec<u8>, Brc2Error> {
    if symmetric_key.len() != 32 {
        return Err(Brc2Error::InvalidCiphertext(
            format!("Symmetric key must be 32 bytes, got {}", symmetric_key.len())
        ));
    }

    if ciphertext_with_iv.len() < 48 {
        return Err(Brc2Error::InvalidCiphertext(
            format!("Ciphertext too short: need at least 48 bytes (32 IV + 16 tag), got {}", ciphertext_with_iv.len())
        ));
    }

    // 1. Extract IV (first 32 bytes)
    let iv_bytes = &ciphertext_with_iv[0..32];

    // 2. Extract ciphertext and tag
    // Format: [32-byte IV][ciphertext][16-byte tag]
    let ciphertext_len = ciphertext_with_iv.len() - 32 - 16;
    let ciphertext = &ciphertext_with_iv[32..32 + ciphertext_len];
    let auth_tag = &ciphertext_with_iv[32 + ciphertext_len..];

    // 3. Use custom AESGCM decryption matching TypeScript SDK exactly
    let key_array: [u8; 32] = symmetric_key.try_into()
        .map_err(|_| Brc2Error::InvalidCiphertext("Invalid key length".to_string()))?;

    let plaintext = aesgcm_custom::aesgcm_decrypt_custom(
        ciphertext,
        &[],  // Additional authenticated data (empty for BRC-2)
        iv_bytes,
        auth_tag,
        &key_array,
    ).map_err(|e| Brc2Error::DecryptionFailed(format!("Custom AESGCM decryption failed: {}", e)))?;

    Ok(plaintext)
}

/// Encrypt certificate field using BRC-2
///
/// **Certificate Field Encryption**:
/// - Protocol ID: `[2, "certificate field encryption"]`
/// - Key ID: `fieldName` (for master) or `"${serialNumber} ${fieldName}"` (for verifier)
/// - Invoice Number: `"2-certificate field encryption-${keyID}"`
///
/// ## Arguments
/// - `sender_private_key`: Sender's 32-byte master private key
/// - `recipient_public_key`: Recipient's 33-byte compressed public key
/// - `field_name`: Certificate field name
/// - `serial_number`: Optional certificate serial number (for verifier keyring)
/// - `plaintext`: Field value to encrypt
///
/// ## Returns
/// Encrypted ciphertext (with IV prepended)
pub fn encrypt_certificate_field(
    sender_private_key: &[u8],
    recipient_public_key: &[u8],
    field_name: &str,
    serial_number: Option<&str>,
    plaintext: &[u8],
) -> Result<Vec<u8>, Brc2Error> {
    // 1. Create invoice number
    let key_id = if let Some(serial) = serial_number {
        format!("{} {}", serial, field_name)
    } else {
        field_name.to_string()
    };

    let invoice = InvoiceNumber::new(
        SecurityLevel::CounterpartyLevel,  // Level 2
        "certificate field encryption",
        &key_id,
    ).map_err(|e| Brc2Error::InvalidInvoiceNumber(e))?;

    let invoice_string = invoice.to_string();
    log::info!("   🔐 BRC-2 encrypt_certificate_field:");
    log::info!("      Invoice number: {}", invoice_string);
    log::info!("      Invoice number (hex): {}", hex::encode(invoice_string.as_bytes()));
    log::info!("      Invoice number length: {} bytes", invoice_string.len());
    log::info!("      Key ID: {}", key_id);
    log::info!("      Protocol ID (normalized): {}", invoice.protocol_id);

    // 2. Derive symmetric key
    let symmetric_key = derive_symmetric_key(
        sender_private_key,
        recipient_public_key,
        &invoice_string,
    )?;

    log::debug!("   BRC-2 encrypt_certificate_field: derived symmetric key (hex, first 16) = {}", hex::encode(&symmetric_key[..16]));

    // 3. Encrypt with AES-256-GCM
    log::info!("      Plaintext length: {} bytes", plaintext.len());
    log::info!("      Derived symmetric key (hex, first 16): {}", hex::encode(&symmetric_key[..16]));
    let result = encrypt_brc2(plaintext, &symmetric_key)?;
    log::info!("      Ciphertext length: {} bytes", result.len());
    Ok(result)
}

/// Decrypt certificate field using BRC-2
///
/// ## Arguments
/// - `recipient_private_key`: Recipient's 32-byte master private key
/// - `sender_public_key`: Sender's 33-byte compressed public key
/// - `field_name`: Certificate field name
/// - `serial_number`: Optional certificate serial number
/// - `ciphertext`: Encrypted field value (with IV prepended)
///
/// ## Returns
/// Decrypted plaintext
pub fn decrypt_certificate_field(
    recipient_private_key: &[u8],
    sender_public_key: &[u8],
    field_name: &str,
    serial_number: Option<&str>,
    ciphertext: &[u8],
) -> Result<Vec<u8>, Brc2Error> {
    // 1. Create invoice number (same as encryption)
    let key_id = if let Some(serial) = serial_number {
        format!("{} {}", serial, field_name)
    } else {
        field_name.to_string()
    };

    let invoice = InvoiceNumber::new(
        SecurityLevel::CounterpartyLevel,  // Level 2
        "certificate field encryption",
        &key_id,
    ).map_err(|e| Brc2Error::InvalidInvoiceNumber(e))?;

    // 2. Derive symmetric key
    let symmetric_key = derive_symmetric_key(
        recipient_private_key,
        sender_public_key,
        &invoice.to_string(),
    )?;

    // 3. Decrypt with AES-256-GCM
    decrypt_brc2(ciphertext, &symmetric_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_symmetric_key() {
        // Test symmetric key derivation
        let sender_priv = [1u8; 32];
        let recipient_pub = [0x02; 33];  // Dummy compressed pubkey
        let invoice = "2-certificate field encryption-name";

        let result = derive_symmetric_key(&sender_priv, &recipient_pub, invoice);
        // Should succeed (even if keys are dummy, structure is correct)
        assert!(result.is_ok());
        let key = result.unwrap();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = b"Hello, BRC-2!";
        let symmetric_key = [0x42u8; 32];  // Dummy key for testing

        // Encrypt
        let ciphertext = encrypt_brc2(plaintext, &symmetric_key).unwrap();
        assert!(ciphertext.len() > plaintext.len());  // Should have IV + tag

        // Decrypt
        let decrypted = decrypt_brc2(&ciphertext, &symmetric_key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_certificate_field_encryption() {
        let sender_priv = [1u8; 32];
        let recipient_pub = [0x02; 33];
        let field_name = "name";
        let plaintext = b"Alice";

        let result = encrypt_certificate_field(
            &sender_priv,
            &recipient_pub,
            field_name,
            None,
            plaintext,
        );
        // Should succeed (even if keys are dummy)
        assert!(result.is_ok());
    }

    // TODO: Add test vectors from BRC-2 spec when available
    // Test vector from BRC-2 spec:
    // - Identity private key: 6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8
    // - Identity public key: 025ad43a22ac38d0bc1f8bacaabb323b5d634703b7a774c4268f6a09e4ddf79097
    // - Counterparty: 0294c479f762f6baa97fbcd4393564c1d7bd8336ebd15928135bbcf575cd1a71a1
    // - Protocol: [2, "BRC2 Test"], KeyID: 42
    // - Plaintext: "BRC-2 Encryption Compliance Validated!"
    // - Expected ciphertext: [252, 203, 216, 184, ...] (96 bytes total)
}
