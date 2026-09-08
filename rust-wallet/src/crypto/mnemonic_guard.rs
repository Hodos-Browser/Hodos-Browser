//! Validation gate for anything claiming to be a BIP39 recovery phrase.
//!
//! WHY THIS EXISTS
//! ---------------
//! ⛔ MEASURED 2026-09-08 on the owner's production wallet. The macOS Keychain slot for
//! service "HodosBrowser" held a **2-word** value. `try_dpapi_unlock` read it, cached it
//! without looking at it, and returned success. The consequences were all silent:
//!
//!   * `is_unlocked()` returned true, so the wallet believed it was open;
//!   * the wallet panel therefore rendered the balance view, never the PIN screen, so the
//!     user was never offered the one action that would have fixed it;
//!   * every key operation failed with
//!     `Invalid mnemonic: mnemonic has an invalid word count: 2`;
//!   * and the CORRECT PIN-encrypted phrase was in the database the whole time.
//!
//! The wallet was unrecoverable while holding everything needed to recover itself. That is
//! the same shape as the locked-wallet defect fixed on 2026-08-26
//! (`TICKET_locked_wallet_is_unrecoverable.md`): bad state accepted silently, recovery path
//! never offered.
//!
//! ⚠️ Validate EXACTLY as the consumer parses. `database::helpers::get_master_private_key_from_db`
//! calls `Mnemonic::parse_in(Language::English, ..)` on the cached string, so this predicate
//! makes the same call. That equivalence is the point: if this returns true, the downstream
//! parse cannot then fail. Loosening it (trimming, a different language, a word-count-only
//! check) would break that guarantee and let a bad value back through.

use bip39::{Language, Mnemonic};

/// True if `phrase` is a valid English BIP39 mnemonic — correct word count AND checksum.
///
/// Deliberately free of database, keychain and wallet state so it is unit-testable on its
/// own (the `IpcAuth.h` / `OverlayMouse.h` precedent).
pub fn is_valid_mnemonic(phrase: &str) -> bool {
    Mnemonic::parse_in(Language::English, phrase).is_ok()
}

/// A description of `phrase` that is safe to put in a log: word count only, never content.
/// ⛔ Never log the phrase itself, valid or not — a "bad" value may still be a real secret.
pub fn describe_shape(phrase: &str) -> String {
    format!("{} word(s), {} chars", phrase.split_whitespace().count(), phrase.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Canonical BIP39 English test vector.
    const VALID_12: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    const VALID_24: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

    #[test]
    fn accepts_real_phrases() {
        assert!(is_valid_mnemonic(VALID_12));
        assert!(is_valid_mnemonic(VALID_24));
    }

    // The exact value that bricked the owner's wallet: 2 words.
    #[test]
    fn rejects_the_two_word_value_that_caused_this() {
        assert!(!is_valid_mnemonic("abandon about"));
    }

    #[test]
    fn rejects_empty_and_whitespace() {
        assert!(!is_valid_mnemonic(""));
        assert!(!is_valid_mnemonic("   "));
    }

    // A word-count-only check would PASS this. The checksum is why we parse instead.
    #[test]
    fn rejects_right_length_wrong_checksum() {
        let twelve_abandons = "abandon ".repeat(12);
        let phrase = twelve_abandons.trim();
        assert_eq!(phrase.split_whitespace().count(), 12);
        assert!(!is_valid_mnemonic(phrase));
    }

    // Non-mnemonic values that could plausibly land in the credential store: the sentinel
    // the DB column stores, and a hex blob (e.g. a PIN-encrypted phrase written by mistake).
    #[test]
    fn rejects_non_mnemonic_payloads() {
        assert!(!is_valid_mnemonic("KEYCHAIN"));
        assert!(!is_valid_mnemonic("a3f19c02b7d4e8815506ff2a1b9c7d3e"));
    }

    #[test]
    fn describe_shape_never_contains_the_phrase() {
        let s = describe_shape(VALID_12);
        assert!(!s.contains("abandon"));
        assert!(s.contains("12 word(s)"));
    }
}
