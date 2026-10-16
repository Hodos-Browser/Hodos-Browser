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
//!   lives in the OS Keychain under service returned by `keychain_service()`,
//!   account "wallet-mnemonic"). Dev and production use different service names
//!   to prevent cross-contamination.

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
        let _ = LocalFree(Some(HLOCAL(data_out.pbData as *mut core::ffi::c_void)));

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
        let _ = LocalFree(Some(HLOCAL(data_out.pbData as *mut core::ffi::c_void)));

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

/// Keychain service name — MUST differ between dev and production to prevent
/// cross-contamination (dev wallet overwriting production mnemonic in Keychain).
#[cfg(target_os = "macos")]
fn keychain_service() -> &'static str {
    if std::env::var("HODOS_DEV").unwrap_or_default() == "1" {
        "HodosBrowserDev"
    } else {
        "HodosBrowser"
    }
}

#[cfg(target_os = "macos")]
const KEYCHAIN_ACCOUNT: &str = "wallet-mnemonic";

// ⛔ M-06 §0 (2026-10-10): plain `cargo test` on macOS replaced the installed wallet's
// Keychain phrase with a random test phrase — wallet-creating tests reach `dpapi_encrypt`.
// So in a unit-test build the macOS store never touches the Keychain: the phrase travels
// in the returned blob, as a Windows DPAPI blob carries its data. No test can overwrite a
// real item, parallel tests cannot race on one slot, no ACL dialog. The real Keychain is
// reached only with HODOS_KEYCHAIN_TEST=1, and never the production item.
// ⚠️ `tests/*.rs` build the library WITHOUT cfg(test): see
// `no_integration_test_reaches_the_credential_store`.

/// Prefix of the blob a unit-test build returns instead of writing the Keychain.
#[cfg(all(test, target_os = "macos"))]
const TEST_STORE_PREFIX: &[u8] = b"HODOS-TEST-STORE:";

/// Which store a credential call may use in a unit-test build.
#[cfg(test)]
#[derive(Debug, PartialEq)]
enum TestStore {
    /// The phrase stays in the blob; the Keychain is not touched.
    InBlob,
    /// Opted in (HODOS_KEYCHAIN_TEST=1) and a non-production service.
    Keychain,
    /// Opted in but the call would reach the production item.
    Refused,
}

#[cfg(test)]
fn test_store_for(opted_in: bool, service: &str) -> TestStore {
    match (opted_in, service == "HodosBrowser") {
        (false, _) => TestStore::InBlob,
        (true, true) => TestStore::Refused,
        (true, false) => TestStore::Keychain,
    }
}

#[cfg(all(test, target_os = "macos"))]
fn test_store() -> TestStore {
    test_store_for(std::env::var("HODOS_KEYCHAIN_TEST").unwrap_or_default() == "1", keychain_service())
}

#[cfg(all(test, target_os = "macos"))]
const TEST_REFUSED: &str =
    "refusing the PRODUCTION Keychain item in a test build (HODOS_KEYCHAIN_TEST=1 needs HODOS_DEV=1)";

/// Store mnemonic in macOS Keychain (tied to current user account).
/// Returns a sentinel value for the DB column (the real secret is in Keychain).
#[cfg(target_os = "macos")]
pub fn dpapi_encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(test)]
    match test_store() {
        TestStore::InBlob => return Ok([TEST_STORE_PREFIX, plaintext].concat()),
        TestStore::Refused => return Err(TEST_REFUSED.to_string()),
        TestStore::Keychain => {}
    }
    keychain_set(keychain_service(), plaintext)?;
    // Return sentinel — the DB column stores this, not the actual secret
    Ok(KEYCHAIN_SENTINEL.to_vec())
}

/// Retrieve mnemonic from macOS Keychain (requires same macOS user account).
/// The `_encrypted` parameter is the sentinel value from the DB — ignored.
#[cfg(target_os = "macos")]
pub fn dpapi_decrypt(_encrypted: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(test)]
    match test_store() {
        TestStore::InBlob => {
            return _encrypted.strip_prefix(TEST_STORE_PREFIX).map(|p| p.to_vec()).ok_or_else(||
                "not a test-store blob: a test build does not read the Keychain (opt in: HODOS_KEYCHAIN_TEST=1 HODOS_DEV=1)".to_string())
        }
        TestStore::Refused => return Err(TEST_REFUSED.to_string()),
        TestStore::Keychain => {}
    }
    keychain_get(keychain_service())
}

#[cfg(target_os = "macos")]
fn keychain_set(service: &str, plaintext: &[u8]) -> Result<(), String> {
    use security_framework::passwords::{set_generic_password, delete_generic_password};

    // Delete any existing entry first (set_generic_password fails if entry exists)
    let _ = delete_generic_password(service, KEYCHAIN_ACCOUNT);

    set_generic_password(service, KEYCHAIN_ACCOUNT, plaintext)
        .map_err(|e| format!("Keychain store failed: {}", e))?;

    log::info!("   Mnemonic stored in macOS Keychain (service={}, account={})",
        service, KEYCHAIN_ACCOUNT);
    Ok(())
}

#[cfg(target_os = "macos")]
fn keychain_get(service: &str) -> Result<Vec<u8>, String> {
    use security_framework::passwords::get_generic_password;

    get_generic_password(service, KEYCHAIN_ACCOUNT)
        .map_err(|e| format!("Keychain retrieve failed (service={}): {}", service, e))
}

// =============================================================================
// Retrying store — the OS credential store is not optional
// =============================================================================

/// Attempts before a credential-store write is reported as failed.
pub const STORE_ATTEMPTS: u32 = 5;

/// Store the mnemonic in the OS credential store, retrying transient failures.
///
/// ⛔ WHY THIS EXISTS. `dpapi_encrypt` used to be called once, at wallet creation, with
/// its error swallowed and `None` written to `wallets.mnemonic_dpapi` — the wallet was
/// created anyway. That produces a wallet which can NEVER auto-unlock, because the only
/// repair path (`store_dpapi_blob`) needs the mnemonic, which needs an unlock, which
/// needs the blob. MEASURED 2026-08-26: the macOS dev wallet created 2026-06-25 has been
/// in exactly that state ever since — 61 consecutive `getPublicKey` failures with
/// "Wallet is locked", and no site could ever connect. Nothing surfaced it.
///
/// The failure that caused it is believed to be the pre-`deff765` shared Keychain service
/// name: dev and production both used "HodosBrowser", and the ad-hoc-signed dev binary was
/// refused access to an item owned by the Developer-ID-signed production app. That name
/// collision is fixed; this retry is the belt to its braces, and covers the genuinely
/// transient cases (credential store busy, locked, momentarily unavailable) that can hit a
/// real user on a properly signed install.
///
/// ⚠️ A failure here must be reported to the caller, never swallowed. Callers decide
/// whether to refuse the operation, but none of them may silently continue.
pub fn dpapi_encrypt_retrying(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let mut last_err = String::new();
    for attempt in 1..=STORE_ATTEMPTS {
        match dpapi_encrypt(plaintext) {
            Ok(blob) => {
                if attempt > 1 {
                    log::info!("   Credential-store write succeeded on attempt {}/{}", attempt, STORE_ATTEMPTS);
                }
                return Ok(blob);
            }
            Err(e) => {
                log::warn!("   Credential-store write failed (attempt {}/{}): {}", attempt, STORE_ATTEMPTS, e);
                last_err = e;
                if attempt < STORE_ATTEMPTS {
                    // 50ms, 100ms, 200ms, 400ms — bounded at well under a second total.
                    std::thread::sleep(std::time::Duration::from_millis(50u64 << (attempt - 1)));
                }
            }
        }
    }
    Err(format!(
        "credential-store write failed after {} attempts: {}",
        STORE_ATTEMPTS, last_err
    ))
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
        // ⛔ THIS TEST WRITES TO THE REAL KEYCHAIN. Read before removing a guard.
        //
        // As originally written it called dpapi_encrypt() with HODOS_DEV unset, so
        // keychain_service() resolved to "HodosBrowser" — the PRODUCTION service. It then
        // delete_generic_password()'d the user's real entry and replaced it with the test
        // mnemonic below, leaving no cleanup. A developer running `cargo test` on macOS
        // would have destroyed their production wallet's auto-unlock AND left it pointing
        // at a foreign mnemonic. Found 2026-08-26 while diagnosing a locked dev wallet.
        //
        // Guards now: opt-in, and a service that is never a wallet's (2026-10-12: it used
        // to set HODOS_DEV=1 for the whole process and delete the DEV wallet's item).
        if std::env::var("HODOS_KEYCHAIN_TEST").unwrap_or_default() != "1" {
            eprintln!("skipped: set HODOS_KEYCHAIN_TEST=1 to run (writes to the real Keychain)");
            return;
        }
        const SERVICE: &str = "HodosBrowserKeychainRoundTrip";

        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        keychain_set(SERVICE, mnemonic.as_bytes()).expect("keychain store should succeed");
        let retrieved = keychain_get(SERVICE).expect("keychain retrieve should succeed");
        let _ = security_framework::passwords::delete_generic_password(SERVICE, KEYCHAIN_ACCOUNT);
        assert_eq!(retrieved, mnemonic.as_bytes());
    }

    /// M-06 §0 — a unit-test build never reaches a real Keychain item unless opted in, and
    /// never the production item even then. Pure half, run on every platform.
    #[test]
    fn a_test_build_keeps_the_phrase_out_of_the_keychain() {
        assert_eq!(test_store_for(false, "HodosBrowser"), TestStore::InBlob);
        assert_eq!(test_store_for(false, "HodosBrowserDev"), TestStore::InBlob);
        assert_eq!(test_store_for(true, "HodosBrowser"), TestStore::Refused);
        assert_eq!(test_store_for(true, "HodosBrowserDev"), TestStore::Keychain);
    }

    /// M-06 §0 — the wired half: `dpapi_encrypt`, the function every wallet-creating test
    /// reaches, returns the test blob, never the Keychain sentinel. ⭐ macOS RED: delete
    /// the `#[cfg(test)] match test_store()` arm in `dpapi_encrypt` ⇒ this fails (and the
    /// Keychain item's `cdat` changes).
    #[test]
    #[cfg(target_os = "macos")]
    fn macos_dpapi_encrypt_in_a_test_build_never_writes_the_keychain() {
        if std::env::var("HODOS_KEYCHAIN_TEST").unwrap_or_default() == "1" {
            eprintln!("skipped: HODOS_KEYCHAIN_TEST=1 opts into the real Keychain");
            return;
        }
        let phrase = b"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let blob = dpapi_encrypt(phrase).expect("test store");
        assert_ne!(blob, KEYCHAIN_SENTINEL, "a test build wrote the real Keychain");
        assert!(blob.starts_with(TEST_STORE_PREFIX));
        assert_eq!(dpapi_decrypt(&blob).unwrap(), phrase.to_vec());
        assert!(dpapi_decrypt(KEYCHAIN_SENTINEL).is_err(), "a test build read the real Keychain");
    }

    /// M-06 §0 — `tests/*.rs` build the library WITHOUT `cfg(test)`, so the test store does
    /// not protect them. No integration test may reach the credential store, except
    /// tier7's DPAPI section, which must stay off macOS.
    #[test]
    fn no_integration_test_reaches_the_credential_store() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        const REACH: [&str; 5] = ["dpapi_encrypt", "dpapi_decrypt", "create_wallet", "store_dpapi_blob", "try_dpapi_unlock"];
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("tests/ exists") {
            let path = entry.unwrap().path();
            if path.extension().map_or(true, |x| x != "rs") { continue; }
            seen += 1;
            let text = std::fs::read_to_string(&path).unwrap();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if name == "tier7_boundary_coverage_test.rs" {
                assert!(text.contains("#[cfg(all(not(windows), not(target_os = \"macos\")))]"),
                    "tier7's DPAPI section must not run on macOS");
                continue;
            }
            for r in REACH {
                assert!(!text.contains(r), "{name} reaches the credential store via `{r}`");
            }
        }
        assert!(seen > 5, "read {seen} files from {}", dir.display());
    }

    #[test]
    #[cfg(all(not(windows), not(target_os = "macos")))]
    fn test_platform_unlock_not_available() {
        let result = dpapi_encrypt(b"test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not available"));
    }
}
