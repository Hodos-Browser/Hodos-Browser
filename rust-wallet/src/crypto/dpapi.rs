//! Platform-native mnemonic auto-unlock
//!
//! Each platform uses its OS credential storage to encrypt/decrypt the mnemonic
//! tied to the current user account. Decryption succeeds if and only if the same
//! OS user is logged in — no password or PIN needed.
//!
//! - **Windows**: DPAPI (CryptProtectData/CryptUnprotectData)
//!   Same mechanism Chrome, Firefox, and Edge use for saved passwords.
//!   Storage: `wallets.mnemonic_dpapi` = raw DPAPI blob (BLOB column)
//!
//! - **macOS**: Keychain Services (SecKeychainAddGenericPassword/FindGenericPassword)
//!   Same mechanism Chrome ("Chrome Safe Storage") and Brave use.
//!   Storage: `wallets.mnemonic_dpapi` = sentinel value b"KEYCHAIN" (actual secret
//!   lives in the OS Keychain under service "HodosBrowser", account "wallet-mnemonic")

// =============================================================================
// Windows DPAPI implementation
// =============================================================================

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

// =============================================================================
// macOS Keychain implementation
// =============================================================================

/// Sentinel value stored in DB column `mnemonic_dpapi` when the actual secret
/// lives in the macOS Keychain. Must be non-empty so `mnemonic_dpapi.is_some()`
/// returns true (indicating auto-unlock is available).
#[cfg(target_os = "macos")]
const KEYCHAIN_SENTINEL: &[u8] = b"KEYCHAIN";

#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "HodosBrowser";
#[cfg(target_os = "macos")]
const KEYCHAIN_ACCOUNT: &str = "wallet-mnemonic";

/// Store mnemonic in macOS Keychain (tied to current user account).
/// Returns a sentinel value for the DB column (the real secret is in Keychain).
#[cfg(target_os = "macos")]
pub fn dpapi_encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use security_framework::passwords::{set_generic_password, delete_generic_password};

    // Delete any existing entry first (set_generic_password fails if entry exists)
    let _ = delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);

    set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, plaintext)
        .map_err(|e| format!("Keychain store failed: {}", e))?;

    log::info!("   Mnemonic stored in macOS Keychain (service={}, account={})",
        KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);

    // Return sentinel — the DB column stores this, not the actual secret
    Ok(KEYCHAIN_SENTINEL.to_vec())
}

/// Retrieve mnemonic from macOS Keychain (requires same macOS user account).
/// The `_encrypted` parameter is the sentinel value from the DB — ignored.
#[cfg(target_os = "macos")]
pub fn dpapi_decrypt(_encrypted: &[u8]) -> Result<Vec<u8>, String> {
    use security_framework::passwords::get_generic_password;

    let password = get_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| format!("Keychain retrieve failed: {}", e))?;

    Ok(password)
}

// =============================================================================
// Linux / other platforms — stub (wallet still works, just no auto-unlock)
// =============================================================================

#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn dpapi_encrypt(_plaintext: &[u8]) -> Result<Vec<u8>, String> {
    Err("Platform auto-unlock not available. Use PIN to unlock wallet.".to_string())
}

#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn dpapi_decrypt(_encrypted: &[u8]) -> Result<Vec<u8>, String> {
    Err("Platform auto-unlock not available. Use PIN to unlock wallet.".to_string())
}

// =============================================================================
// Tests
// =============================================================================

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
    #[cfg(target_os = "macos")]
    fn test_keychain_round_trip() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let sentinel = dpapi_encrypt(mnemonic.as_bytes()).expect("keychain store should succeed");
        assert_eq!(sentinel, KEYCHAIN_SENTINEL);

        let retrieved = dpapi_decrypt(&sentinel).expect("keychain retrieve should succeed");
        assert_eq!(retrieved, mnemonic.as_bytes());
    }

    #[test]
    #[cfg(all(not(windows), not(target_os = "macos")))]
    fn test_platform_unlock_not_available() {
        let result = dpapi_encrypt(b"test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not available"));
    }
}
