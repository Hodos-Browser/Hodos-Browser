//! Test to verify AESGCM encryption/decryption roundtrip
//! This helps verify our implementation is internally consistent

#[cfg(test)]
mod tests {
    use crate::crypto::aesgcm_custom;
    use hex;

    #[test]
    fn test_aesgcm_roundtrip_4_bytes() {
        // Test with 4-byte plaintext (like "true")
        let plaintext = b"true";
        let key = [0u8; 32]; // Zero key for testing
        let iv = [0u8; 32]; // Zero IV for testing

        // Encrypt
        let (ciphertext, auth_tag) = aesgcm_custom::aesgcm_custom(
            plaintext,
            &[],
            &iv,
            &key,
        ).unwrap();

        // Decrypt
        let decrypted = aesgcm_custom::aesgcm_decrypt_custom(
            &ciphertext,
            &[],
            &iv,
            &auth_tag,
            &key,
        ).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_aesgcm_roundtrip_32_bytes() {
        // Test with 32-byte plaintext (like a symmetric key)
        let plaintext = [0x42u8; 32];
        let key = [0u8; 32]; // Zero key for testing
        let iv = [0u8; 32]; // Zero IV for testing

        // Encrypt
        let (ciphertext, auth_tag) = aesgcm_custom::aesgcm_custom(
            &plaintext,
            &[],
            &iv,
            &key,
        ).unwrap();

        // Decrypt
        let decrypted = aesgcm_custom::aesgcm_decrypt_custom(
            &ciphertext,
            &[],
            &iv,
            &auth_tag,
            &key,
        ).unwrap();

        assert_eq!(&plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_aesgcm_with_real_values() {
        // Test with values from the logs
        let plaintext = hex::decode("74727565").unwrap(); // "true"
        let key = hex::decode("42b79dacfdca814a26a29522c53a50923574bf98c13cbaa5709053b71492e52b").unwrap();
        let iv = hex::decode("41113d6599ece0d23e9ec3e1e80b168019087a1d2e4e27061de54b4b79f5cb6c").unwrap();

        let key_array: [u8; 32] = key.try_into().unwrap();

        // Encrypt
        let (ciphertext, auth_tag) = aesgcm_custom::aesgcm_custom(
            &plaintext,
            &[],
            &iv,
            &key_array,
        ).unwrap();

        println!("Ciphertext: {}", hex::encode(&ciphertext));
        println!("Auth tag: {}", hex::encode(&auth_tag));

        // Decrypt
        let decrypted = aesgcm_custom::aesgcm_decrypt_custom(
            &ciphertext,
            &[],
            &iv,
            &auth_tag,
            &key_array,
        ).unwrap();

        assert_eq!(plaintext, decrypted);

        // Verify it matches expected from logs
        let expected_ciphertext = hex::decode("0b69b220").unwrap();
        let expected_tag = hex::decode("427c72dce8cdacf6418b9294153ffdfa").unwrap();

        assert_eq!(ciphertext, expected_ciphertext, "Ciphertext mismatch!");
        assert_eq!(auth_tag, expected_tag, "Auth tag mismatch!");
    }
}
