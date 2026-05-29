//! BIE1 — ECIES Electrum legacy compat (Phase 2 Step 3c.1)
//!
//! Wire-compatible implementation of the `BIE1` ECIES format used by
//! Electrum-BSV, pre-BRC-2 Yours, RelayX-era apps, and the canonical
//! `@bsv/sdk`'s `ECIES.electrumEncrypt`. Hodos's primary cipher is BRC-2
//! (AES-256-GCM via BRC-42 key derivation); BIE1 lives here strictly as a
//! legacy compat layer so the `window.yours.encrypt` / `window.yours.decrypt`
//! shim translators (Step 3c.3) can interoperate with stored ciphertexts and
//! in-flight messages from Yours-era dApps that never migrated to BRC-2.
//!
//! ## Wire format
//!
//! ```text
//! [ "BIE1"           4  bytes ASCII magic ]
//! [ ephemeral_pub   33  bytes compressed secp256k1 ]
//! [ ciphertext      N   bytes AES-128-CBC, PKCS#7 padded ]
//! [ mac             32  bytes HMAC-SHA256 over MAGIC || ephemeral_pub || ciphertext ]
//! ```
//!
//! ## Key derivation
//!
//! 1. Sender generates ephemeral keypair `(r, R = r·G)`.
//! 2. Shared point `S = recipient_pubkey · r`, serialized **compressed (33 bytes)**.
//! 3. `H = SHA-512(S)` → 64 bytes.
//! 4. Split:
//!    - `iv      = H[0..16]` (16 bytes — note: doubles as both IV and key prefix in the
//!      original Electrum spec; matches `@bsv/sdk` exactly).
//!    - `kE      = H[16..32]` (16 bytes — AES-128 key).
//!    - `kM      = H[32..64]` (32 bytes — HMAC-SHA256 key).
//! 5. `ciphertext = AES-128-CBC(plaintext, kE, iv)` with PKCS#7 padding.
//! 6. `mac = HMAC-SHA256(kM, "BIE1" || R || ciphertext)`.
//!
//! Decryption inverts:
//! - Parse `magic`, `R`, `ciphertext`, `mac` (length-checked).
//! - Validate `R` is on the curve.
//! - `S = R · recipient_priv`, hash, derive the same `iv/kE/kM`.
//! - **Verify HMAC BEFORE AES decryption** (constant-time, prevents padding-oracle leak).
//! - PKCS#7-unpad after decrypt.
//!
//! ## Security properties
//!
//! - **MAC-then-decrypt**: HMAC is verified before the AES decryption step. Any
//!   tampering — including bit-flips inside the ciphertext that would otherwise
//!   produce a padding-oracle signal — is caught by the MAC first.
//! - **Constant-time MAC compare** via `Hmac::verify_slice`.
//! - **Ephemeral pubkey curve check**: `PublicKey::from_slice` rejects points
//!   that aren't on secp256k1, so an attacker can't smuggle in a junk byte string
//!   as the ephemeral pubkey.
//! - **No determinism in encrypt unless explicitly requested**: the default
//!   sender_privkey path generates a fresh ephemeral key from the OS CSPRNG.
//!   Test vectors pass an explicit sender_privkey to lock the wire bytes.
//!
//! ## Why this lives in `crypto/` even though it's not a Hodos primary cipher
//!
//! The Yours-era ecosystem has a long tail of on-chain and DM ciphertexts
//! encrypted under BIE1. Silently swapping to BRC-2 in the shim would break
//! continuity for any dApp that stored ciphertext locally and re-decrypts later.
//! Honest backward compat needs an honest implementation — and the module
//! belongs alongside the other secp256k1 + AES + HMAC primitives, not stuffed
//! into a handler file.
//!
//! ## Reference
//!
//! `@bsv/sdk` (`primitives/ECIES.ts` → `ECIES.electrumEncrypt` /
//! `ECIES.electrumDecrypt`). The Rust implementation mirrors the canonical
//! TypeScript byte-for-byte; round-trip + corruption tests live at the bottom
//! of this file, and cross-impl vectors against `@bsv/sdk` should be locked in
//! at 3c.2 integration smoke once a Node helper is wired.

use aes::Aes128;
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha2::{Digest, Sha256, Sha512};

type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes128CbcDec = cbc::Decryptor<Aes128>;
type HmacSha256 = Hmac<Sha256>;

/// "BIE1" magic prefix.
const MAGIC: [u8; 4] = *b"BIE1";
/// Length of the 4-byte magic prefix.
const MAGIC_LEN: usize = 4;
/// Length of a compressed secp256k1 public key.
const PUBKEY_LEN: usize = 33;
/// Length of the trailing HMAC-SHA256 tag.
const MAC_LEN: usize = 32;
/// Minimum BIE1 envelope: magic + ephemeral pubkey + 1 AES block + MAC.
const MIN_ENVELOPE_LEN: usize = MAGIC_LEN + PUBKEY_LEN + 16 + MAC_LEN;

/// BIE1 encrypt / decrypt errors.
#[derive(Debug, thiserror::Error)]
pub enum Bie1Error {
    #[error("recipient public key invalid: {0}")]
    InvalidRecipientPublicKey(String),

    #[error("sender private key invalid: {0}")]
    InvalidSenderPrivateKey(String),

    #[error("recipient private key invalid: {0}")]
    InvalidRecipientPrivateKey(String),

    #[error("ECDH shared point computation failed: {0}")]
    EcdhFailed(String),

    #[error("envelope too short: {len} < {min} bytes")]
    EnvelopeTooShort { len: usize, min: usize },

    #[error("envelope missing BIE1 magic prefix")]
    InvalidMagic,

    #[error("ephemeral public key in envelope is not a valid secp256k1 point")]
    InvalidEphemeralPublicKey,

    #[error("MAC verification failed (corrupted ciphertext or wrong recipient key)")]
    MacMismatch,

    #[error("PKCS#7 padding invalid after AES decryption")]
    InvalidPadding,

    #[error("AES cipher error: {0}")]
    AesError(String),

    #[error("HMAC init error: {0}")]
    HmacInitError(String),
}

/// Encrypt `plaintext` for `recipient_pubkey` using BIE1 (ECIES Electrum).
///
/// Returns the full BIE1 envelope:
/// `MAGIC || ephemeral_pub_compressed (33B) || AES-128-CBC ciphertext || HMAC-SHA256 (32B)`.
///
/// If `sender_privkey` is `None`, a fresh ephemeral key is generated from the
/// OS CSPRNG (the standard Electrum behavior — every encrypt produces a unique
/// envelope, no key reuse). When `Some(bytes)`, the caller supplies the
/// ephemeral private scalar — only used for deterministic tests and to allow
/// callers that want to bind sender identity at the byte layer.
pub fn encrypt_bie1(
    plaintext: &[u8],
    recipient_pubkey: &[u8],
    sender_privkey: Option<&[u8]>,
) -> Result<Vec<u8>, Bie1Error> {
    let secp = Secp256k1::new();

    let recipient_pubkey = PublicKey::from_slice(recipient_pubkey)
        .map_err(|e| Bie1Error::InvalidRecipientPublicKey(e.to_string()))?;

    let ephemeral_priv = match sender_privkey {
        Some(bytes) => SecretKey::from_slice(bytes)
            .map_err(|e| Bie1Error::InvalidSenderPrivateKey(e.to_string()))?,
        None => SecretKey::new(&mut rand::thread_rng()),
    };
    let ephemeral_pub = PublicKey::from_secret_key(&secp, &ephemeral_priv);

    // Shared point = recipient_pubkey · ephemeral_priv, serialized compressed (33 bytes).
    let shared_point = recipient_pubkey
        .mul_tweak(&secp, &ephemeral_priv.into())
        .map_err(|e| Bie1Error::EcdhFailed(e.to_string()))?;
    let shared_compressed = shared_point.serialize();

    let (iv, ae_key, mac_key) = derive_subkeys(&shared_compressed);

    // AES-128-CBC encrypt with PKCS#7 padding.
    let cipher = Aes128CbcEnc::new_from_slices(&ae_key, &iv)
        .map_err(|e| Bie1Error::AesError(e.to_string()))?;
    let ciphertext = cipher.encrypt_padded_vec_mut::<Pkcs7>(plaintext);

    // Assemble: MAGIC || R || ct, then HMAC over the same prefix.
    let mut envelope = Vec::with_capacity(MAGIC_LEN + PUBKEY_LEN + ciphertext.len() + MAC_LEN);
    envelope.extend_from_slice(&MAGIC);
    envelope.extend_from_slice(&ephemeral_pub.serialize());
    envelope.extend_from_slice(&ciphertext);

    let mut mac = HmacSha256::new_from_slice(&mac_key)
        .map_err(|e| Bie1Error::HmacInitError(e.to_string()))?;
    mac.update(&envelope);
    let mac_bytes = mac.finalize().into_bytes();

    envelope.extend_from_slice(&mac_bytes);
    Ok(envelope)
}

/// Decrypt a BIE1 `envelope` using `recipient_privkey`.
///
/// Steps in order, fail-fast on each:
/// 1. Length check (envelope ≥ MAGIC + pubkey + 1 block + MAC).
/// 2. Magic validation.
/// 3. Ephemeral pubkey parse (rejects off-curve junk).
/// 4. ECDH shared point + SHA-512 key derivation.
/// 5. **HMAC verify before AES** (constant-time, no padding-oracle leak).
/// 6. AES-128-CBC decrypt + PKCS#7 unpad.
pub fn decrypt_bie1(envelope: &[u8], recipient_privkey: &[u8]) -> Result<Vec<u8>, Bie1Error> {
    if envelope.len() < MIN_ENVELOPE_LEN {
        return Err(Bie1Error::EnvelopeTooShort {
            len: envelope.len(),
            min: MIN_ENVELOPE_LEN,
        });
    }

    if &envelope[0..MAGIC_LEN] != MAGIC {
        return Err(Bie1Error::InvalidMagic);
    }

    let ephemeral_pub_bytes = &envelope[MAGIC_LEN..MAGIC_LEN + PUBKEY_LEN];
    let prefix_len = envelope.len() - MAC_LEN;
    let ciphertext = &envelope[MAGIC_LEN + PUBKEY_LEN..prefix_len];
    let received_mac = &envelope[prefix_len..];

    let ephemeral_pub = PublicKey::from_slice(ephemeral_pub_bytes)
        .map_err(|_| Bie1Error::InvalidEphemeralPublicKey)?;

    let secp = Secp256k1::new();
    let recipient_priv = SecretKey::from_slice(recipient_privkey)
        .map_err(|e| Bie1Error::InvalidRecipientPrivateKey(e.to_string()))?;

    let shared_point = ephemeral_pub
        .mul_tweak(&secp, &recipient_priv.into())
        .map_err(|e| Bie1Error::EcdhFailed(e.to_string()))?;
    let shared_compressed = shared_point.serialize();

    let (iv, ae_key, mac_key) = derive_subkeys(&shared_compressed);

    // VERIFY MAC BEFORE AES — constant-time (Hmac::verify_slice).
    let mut mac = HmacSha256::new_from_slice(&mac_key)
        .map_err(|e| Bie1Error::HmacInitError(e.to_string()))?;
    mac.update(&envelope[..prefix_len]);
    mac.verify_slice(received_mac)
        .map_err(|_| Bie1Error::MacMismatch)?;

    let cipher = Aes128CbcDec::new_from_slices(&ae_key, &iv)
        .map_err(|e| Bie1Error::AesError(e.to_string()))?;
    let plaintext = cipher
        .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
        .map_err(|_| Bie1Error::InvalidPadding)?;

    Ok(plaintext)
}

/// Split SHA-512(shared_compressed) into `(iv, aes_key, hmac_key)` as owned arrays.
///
/// Exact layout used by `@bsv/sdk`:
/// - `iv     = hash[0..16]`  (16-byte AES IV)
/// - `aeKey  = hash[16..32]` (16-byte AES-128 key)
/// - `macKey = hash[32..64]` (32-byte HMAC-SHA256 key)
///
/// Returned by value as fixed-size arrays so the SHA-512 output buffer can
/// drop immediately — no lifetimes tied to a temporary.
fn derive_subkeys(shared_compressed: &[u8]) -> ([u8; 16], [u8; 16], [u8; 32]) {
    let hash = Sha512::digest(shared_compressed);
    let mut iv = [0u8; 16];
    let mut ae_key = [0u8; 16];
    let mut mac_key = [0u8; 32];
    iv.copy_from_slice(&hash[0..16]);
    ae_key.copy_from_slice(&hash[16..32]);
    mac_key.copy_from_slice(&hash[32..64]);
    (iv, ae_key, mac_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::Secp256k1;

    fn make_keypair(byte: u8) -> (Vec<u8>, Vec<u8>) {
        let secp = Secp256k1::new();
        let priv_bytes = vec![byte; 32];
        let secret = SecretKey::from_slice(&priv_bytes).expect("non-zero key");
        let pub_bytes = PublicKey::from_secret_key(&secp, &secret).serialize().to_vec();
        (priv_bytes, pub_bytes)
    }

    #[test]
    fn round_trip_small_message() {
        let (_, recipient_pub) = make_keypair(0x11);
        let recipient_priv = vec![0x11u8; 32];
        let plaintext = b"Hello, BIE1!";

        let envelope = encrypt_bie1(plaintext, &recipient_pub, None).expect("encrypt");
        let decrypted = decrypt_bie1(&envelope, &recipient_priv).expect("decrypt");
        assert_eq!(decrypted.as_slice(), plaintext);
    }

    #[test]
    fn round_trip_empty_plaintext() {
        let (_, recipient_pub) = make_keypair(0x22);
        let recipient_priv = vec![0x22u8; 32];
        let envelope = encrypt_bie1(&[], &recipient_pub, None).expect("encrypt");
        let decrypted = decrypt_bie1(&envelope, &recipient_priv).expect("decrypt");
        assert!(decrypted.is_empty());
    }

    #[test]
    fn round_trip_exactly_one_aes_block() {
        // 16 bytes — PKCS#7 will pad with a full block of 0x10. Common edge case.
        let (_, recipient_pub) = make_keypair(0x33);
        let recipient_priv = vec![0x33u8; 32];
        let plaintext = b"0123456789ABCDEF";
        assert_eq!(plaintext.len(), 16);

        let envelope = encrypt_bie1(plaintext, &recipient_pub, None).expect("encrypt");
        let decrypted = decrypt_bie1(&envelope, &recipient_priv).expect("decrypt");
        assert_eq!(decrypted.as_slice(), plaintext);
    }

    #[test]
    fn round_trip_multi_block_message() {
        let (_, recipient_pub) = make_keypair(0x44);
        let recipient_priv = vec![0x44u8; 32];
        let plaintext: Vec<u8> = (0..200u8).collect();
        let envelope = encrypt_bie1(&plaintext, &recipient_pub, None).expect("encrypt");
        let decrypted = decrypt_bie1(&envelope, &recipient_priv).expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn envelope_layout_is_correct() {
        let (_, recipient_pub) = make_keypair(0x55);
        let plaintext = b"x";
        let envelope = encrypt_bie1(plaintext, &recipient_pub, None).expect("encrypt");

        // BIE1 prefix
        assert_eq!(&envelope[0..4], b"BIE1");
        // 33-byte compressed ephemeral pubkey at offset 4
        assert!(envelope[4] == 0x02 || envelope[4] == 0x03,
                "ephemeral pubkey must be compressed (0x02|0x03 prefix), got 0x{:02x}", envelope[4]);
        // Total length = 4 + 33 + 16 (one padded block for 1-byte input) + 32 MAC
        assert_eq!(envelope.len(), 4 + 33 + 16 + 32);
    }

    #[test]
    fn deterministic_with_explicit_sender_priv() {
        // Same plaintext + same recipient + same sender_priv → identical envelope.
        // This locks the wire format against our own impl; @bsv/sdk vectors lock
        // it across implementations at smoke-test time.
        let (_, recipient_pub) = make_keypair(0x66);
        let sender_priv = vec![0x99u8; 32];
        let plaintext = b"deterministic";

        let envelope_a = encrypt_bie1(plaintext, &recipient_pub, Some(&sender_priv)).unwrap();
        let envelope_b = encrypt_bie1(plaintext, &recipient_pub, Some(&sender_priv)).unwrap();
        assert_eq!(envelope_a, envelope_b);
    }

    #[test]
    fn random_ephemeral_keys_produce_different_envelopes() {
        let (_, recipient_pub) = make_keypair(0x77);
        let plaintext = b"same message";
        let envelope_a = encrypt_bie1(plaintext, &recipient_pub, None).unwrap();
        let envelope_b = encrypt_bie1(plaintext, &recipient_pub, None).unwrap();
        assert_ne!(envelope_a, envelope_b,
                   "two random ephemeral keys must produce distinct envelopes");
    }

    #[test]
    fn decrypt_with_wrong_privkey_fails_with_mac_mismatch() {
        let (_, recipient_pub) = make_keypair(0x88);
        let plaintext = b"secret";
        let envelope = encrypt_bie1(plaintext, &recipient_pub, None).unwrap();

        // Different recipient privkey — must fail at MAC, not at AES padding.
        let wrong_priv = vec![0x99u8; 32];
        match decrypt_bie1(&envelope, &wrong_priv) {
            Err(Bie1Error::MacMismatch) => {}
            Err(other) => panic!("expected MacMismatch, got {:?}", other),
            Ok(_) => panic!("decrypt must fail with wrong recipient privkey"),
        }
    }

    #[test]
    fn decrypt_rejects_truncated_envelope() {
        let (_, recipient_pub) = make_keypair(0xaa);
        let recipient_priv = vec![0xaau8; 32];
        let envelope = encrypt_bie1(b"hi", &recipient_pub, None).unwrap();
        let truncated = &envelope[..envelope.len() / 2];
        match decrypt_bie1(truncated, &recipient_priv) {
            Err(Bie1Error::EnvelopeTooShort { .. }) => {}
            other => panic!("expected EnvelopeTooShort, got {:?}", other),
        }
    }

    #[test]
    fn decrypt_rejects_bad_magic() {
        let (_, recipient_pub) = make_keypair(0xbb);
        let recipient_priv = vec![0xbbu8; 32];
        let mut envelope = encrypt_bie1(b"hi", &recipient_pub, None).unwrap();
        envelope[0] ^= 0x01; // corrupt 'B' → something else
        match decrypt_bie1(&envelope, &recipient_priv) {
            Err(Bie1Error::InvalidMagic) => {}
            other => panic!("expected InvalidMagic, got {:?}", other),
        }
    }

    #[test]
    fn decrypt_rejects_corrupted_mac() {
        let (_, recipient_pub) = make_keypair(0xcc);
        let recipient_priv = vec![0xccu8; 32];
        let mut envelope = encrypt_bie1(b"hi", &recipient_pub, None).unwrap();
        let last = envelope.len() - 1;
        envelope[last] ^= 0xff; // flip trailing MAC byte
        match decrypt_bie1(&envelope, &recipient_priv) {
            Err(Bie1Error::MacMismatch) => {}
            other => panic!("expected MacMismatch, got {:?}", other),
        }
    }

    #[test]
    fn decrypt_rejects_corrupted_ciphertext_via_mac_check() {
        let (_, recipient_pub) = make_keypair(0xdd);
        let recipient_priv = vec![0xddu8; 32];
        let mut envelope = encrypt_bie1(b"longer-plaintext", &recipient_pub, None).unwrap();
        // Flip a bit inside the AES ciphertext region (after magic+pubkey, before MAC).
        let ct_byte = MAGIC_LEN + PUBKEY_LEN + 2;
        envelope[ct_byte] ^= 0x01;
        match decrypt_bie1(&envelope, &recipient_priv) {
            // MAC verification fires BEFORE AES, so corruption inside the ciphertext
            // surfaces as MacMismatch, never as InvalidPadding (no oracle leak).
            Err(Bie1Error::MacMismatch) => {}
            other => panic!("expected MacMismatch (not padding error!), got {:?}", other),
        }
    }

    #[test]
    fn decrypt_rejects_off_curve_ephemeral_pubkey() {
        let (_, recipient_pub) = make_keypair(0xee);
        let recipient_priv = vec![0xeeu8; 32];
        let mut envelope = encrypt_bie1(b"hi", &recipient_pub, None).unwrap();
        // Corrupt the ephemeral pubkey region (offset 4..37) to something not on curve.
        for i in 4..37 {
            envelope[i] = 0xff;
        }
        match decrypt_bie1(&envelope, &recipient_priv) {
            Err(Bie1Error::InvalidEphemeralPublicKey) => {}
            other => panic!("expected InvalidEphemeralPublicKey, got {:?}", other),
        }
    }

    #[test]
    fn encrypt_rejects_bad_recipient_pubkey() {
        // 32-byte garbage instead of 33-byte compressed pubkey.
        let result = encrypt_bie1(b"hi", &[0u8; 32], None);
        match result {
            Err(Bie1Error::InvalidRecipientPublicKey(_)) => {}
            other => panic!("expected InvalidRecipientPublicKey, got {:?}", other),
        }
    }

    #[test]
    fn decrypt_rejects_bad_recipient_privkey() {
        let (_, recipient_pub) = make_keypair(0x12);
        let envelope = encrypt_bie1(b"hi", &recipient_pub, None).unwrap();
        // 30-byte private key (must be 32).
        match decrypt_bie1(&envelope, &[0u8; 30]) {
            Err(Bie1Error::InvalidRecipientPrivateKey(_)) => {}
            other => panic!("expected InvalidRecipientPrivateKey, got {:?}", other),
        }
    }

    #[test]
    fn subkey_layout_matches_canonical_split() {
        // Lock the iv/aeKey/macKey split bytes against @bsv/sdk's exact convention.
        let shared = [0xab; 33];
        let (iv, ae_key, mac_key) = derive_subkeys(&shared);
        let full = Sha512::digest(shared);
        assert_eq!(&iv[..], &full[0..16]);
        assert_eq!(&ae_key[..], &full[16..32]);
        assert_eq!(&mac_key[..], &full[32..64]);
    }
}
