//! PIN-based mnemonic encryption
//!
//! Encrypts/decrypts the wallet mnemonic using a 4-digit PIN.
//! Key derivation: PBKDF2-HMAC-SHA256 (600K iterations) + random salt.
//! Encryption: AES-256-GCM with random 12-byte nonce.
//!
//! Storage format:
//!   pin_salt column:  hex(salt_16)
//!   mnemonic column:  hex(nonce_12 || ciphertext || tag_16)

use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
use aes_gcm::aead::generic_array::GenericArray;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use rand::RngCore;

const PBKDF2_ITERATIONS: u32 = 600_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// Derive a 32-byte AES key from a PIN and salt using PBKDF2-HMAC-SHA256.
pub fn derive_key_from_pin(pin: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(pin.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

/// Encrypt a mnemonic string with a PIN.
/// Returns (salt_hex, encrypted_mnemonic_hex) on success.
pub fn encrypt_mnemonic(mnemonic: &str, pin: &str) -> Result<(String, String), String> {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);

    let key = derive_key_from_pin(pin, &salt);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&key));

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, mnemonic.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Combined format: nonce(12) || ciphertext+tag (aes-gcm appends 16-byte tag)
    let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok((hex::encode(salt), hex::encode(combined)))
}

/// Decrypt a mnemonic using PIN and salt.
/// encrypted_hex = hex(nonce_12 || ciphertext || tag_16)
/// Returns plaintext mnemonic on success, error on wrong PIN or corrupt data.
pub fn decrypt_mnemonic(encrypted_hex: &str, pin: &str, salt_hex: &str) -> Result<String, String> {
    let salt = hex::decode(salt_hex)
        .map_err(|e| format!("Invalid salt hex: {}", e))?;
    let combined = hex::decode(encrypted_hex)
        .map_err(|e| format!("Invalid encrypted hex: {}", e))?;

    // Minimum: 12 nonce + 16 tag + at least 1 byte ciphertext
    if combined.len() < NONCE_LEN + 17 {
        return Err("Encrypted data too short".to_string());
    }

    let key = derive_key_from_pin(pin, &salt);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&key));

    let nonce = GenericArray::from_slice(&combined[..NONCE_LEN]);
    let ciphertext_with_tag = &combined[NONCE_LEN..];

    let plaintext = cipher.decrypt(nonce, ciphertext_with_tag)
        .map_err(|_| "Invalid PIN".to_string())?;

    String::from_utf8(plaintext)
        .map_err(|e| format!("Invalid UTF-8 in decrypted mnemonic: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let pin = "1234";
        let (salt, encrypted) = encrypt_mnemonic(mnemonic, pin).unwrap();
        let decrypted = decrypt_mnemonic(&encrypted, pin, &salt).unwrap();
        assert_eq!(decrypted, mnemonic);
    }

    #[test]
    fn test_wrong_pin() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let pin = "1234";
        let (salt, encrypted) = encrypt_mnemonic(mnemonic, pin).unwrap();
        let result = decrypt_mnemonic(&encrypted, "5678", &salt);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid PIN");
    }
}
