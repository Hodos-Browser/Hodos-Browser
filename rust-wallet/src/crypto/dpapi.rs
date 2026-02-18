//! Windows DPAPI integration for mnemonic auto-unlock
//!
//! Uses CryptProtectData/CryptUnprotectData to encrypt/decrypt the mnemonic
//! tied to the current Windows user account. Decryption succeeds if and only if
//! the same Windows user is logged in — no password or PIN needed.
//!
//! This is the same mechanism Chrome, Firefox, and Edge use for saved passwords.
//!
//! Storage: `wallets.mnemonic_dpapi` = raw DPAPI blob (BLOB column)

/// Encrypt data using Windows DPAPI (tied to current user account).
/// Returns the DPAPI-encrypted blob on success.
#[cfg(windows)]
pub fn dpapi_encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    use windows::Win32::Foundation::{LocalFree, HLOCAL};

    let mut data_in = CRYPT_INTEGER_BLOB {
        cbData: plaintext.len() as u32,
        pbData: plaintext.as_ptr() as *mut u8,
    };

    let mut data_out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    unsafe {
        // CRYPTPROTECT_UI_FORBIDDEN: never show a UI prompt
        let ok = CryptProtectData(
            &mut data_in,
            None,                          // szDataDescr (optional description)
            None,                          // pOptionalEntropy (no extra entropy)
            None,                          // pvReserved
            None,                          // pPromptStruct
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        );

        if ok.is_err() {
            return Err(format!("CryptProtectData failed: {:?}", ok));
        }

        if data_out.pbData.is_null() || data_out.cbData == 0 {
            return Err("CryptProtectData returned empty output".to_string());
        }

        // Copy the encrypted data before freeing the system-allocated buffer
        let encrypted = std::slice::from_raw_parts(
            data_out.pbData,
            data_out.cbData as usize,
        ).to_vec();

        // Free the system-allocated buffer
        let _ = LocalFree(HLOCAL(data_out.pbData as *mut core::ffi::c_void));

        Ok(encrypted)
    }
}

/// Decrypt data using Windows DPAPI (requires same Windows user account).
/// Returns the plaintext on success, error if different user or data is corrupt.
#[cfg(windows)]
pub fn dpapi_decrypt(encrypted: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    use windows::Win32::Foundation::{LocalFree, HLOCAL};

    let mut data_in = CRYPT_INTEGER_BLOB {
        cbData: encrypted.len() as u32,
        pbData: encrypted.as_ptr() as *mut u8,
    };

    let mut data_out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    unsafe {
        let ok = CryptUnprotectData(
            &mut data_in,
            None,                          // ppszDataDescr (don't need description back)
            None,                          // pOptionalEntropy
            None,                          // pvReserved
            None,                          // pPromptStruct
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        );

        if ok.is_err() {
            return Err("CryptUnprotectData failed (different user or corrupted data)".to_string());
        }

        if data_out.pbData.is_null() || data_out.cbData == 0 {
            return Err("CryptUnprotectData returned empty output".to_string());
        }

        // Copy the decrypted data before freeing the system-allocated buffer
        let decrypted = std::slice::from_raw_parts(
            data_out.pbData,
            data_out.cbData as usize,
        ).to_vec();

        // Free the system-allocated buffer
        let _ = LocalFree(HLOCAL(data_out.pbData as *mut core::ffi::c_void));

        Ok(decrypted)
    }
}

/// Stub for non-Windows platforms. Always returns an error.
#[cfg(not(windows))]
pub fn dpapi_encrypt(_plaintext: &[u8]) -> Result<Vec<u8>, String> {
    Err("DPAPI is only available on Windows. Use macOS Keychain on macOS.".to_string())
}

/// Stub for non-Windows platforms. Always returns an error.
#[cfg(not(windows))]
pub fn dpapi_decrypt(_encrypted: &[u8]) -> Result<Vec<u8>, String> {
    Err("DPAPI is only available on Windows. Use macOS Keychain on macOS.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test_dpapi_round_trip() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let encrypted = dpapi_encrypt(mnemonic.as_bytes()).expect("encrypt should succeed");
        assert!(!encrypted.is_empty());
        assert_ne!(encrypted, mnemonic.as_bytes()); // Should be different from plaintext

        let decrypted = dpapi_decrypt(&encrypted).expect("decrypt should succeed");
        assert_eq!(decrypted, mnemonic.as_bytes());
    }

    #[test]
    #[cfg(not(windows))]
    fn test_dpapi_not_available() {
        let result = dpapi_encrypt(b"test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("only available on Windows"));
    }
}
