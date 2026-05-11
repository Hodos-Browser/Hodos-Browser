//! BRC-72 style key linkage revelation primitives
//!
//! Mirrors the canonical `@bsv/sdk` `KeyDeriver.revealCounterpartySecret` and
//! `revealSpecificSecret` algorithms used by `revealCounterpartyKeyLinkage` and
//! `revealSpecificKeyLinkage`.
//!
//! Reference: `reference/ts-brc100/node_modules/@bsv/sdk/src/wallet/KeyDeriver.ts`
//!   - `revealCounterpartySecret` returns `rootKey.deriveSharedSecret(counterparty).encode(true)`
//!     -- the 33-byte compressed ECDH shared secret between rootKey and counterparty.
//!   - `revealSpecificSecret` returns `sha256hmac(sharedSecret.encode(true), invoiceNumberBin)` --
//!     the 32-byte HMAC keyed by the shared secret over the BRC-43 invoice number.
//!
//! Phase 1.5 Step 1 ships the linkage **values** plus their BRC-2 encryption to the
//! verifier. The Schnorr DLEQ proof that the SDK packages into
//! `encryptedLinkageProof` for the counterparty variant is deferred -- we emit the
//! same `[0]` no-proof marker the SDK uses for the specific variant.
//!
//! Phase 1.5 Step 1: do not extend or change `brc42.rs`. This module
//! composes existing primitives only.
//!
//! Reuse anchors (kept narrow on purpose):
//!   - `crypto::brc42::compute_shared_secret`
//!   - `crypto::brc42::compute_invoice_hmac`

use crate::crypto::brc42::{compute_shared_secret, compute_invoice_hmac, Brc42Error};

/// `revealCounterpartySecret` value: the 33-byte compressed ECDH shared
/// secret between our master private key and the counterparty's public key.
///
/// The SDK forbids `counterparty === 'self'` for this call; callers must
/// resolve and validate the counterparty before invoking this function.
pub fn compute_counterparty_linkage(
    master_private_key: &[u8],
    counterparty_public_key: &[u8],
) -> Result<Vec<u8>, Brc42Error> {
    compute_shared_secret(master_private_key, counterparty_public_key)
}

/// `revealSpecificSecret` value: HMAC-SHA256 keyed by the master shared secret
/// over the UTF-8 invoice number `"{securityLevel}-{protocolName}-{keyID}"`.
///
/// Returns 32 bytes.
pub fn compute_specific_linkage(
    master_private_key: &[u8],
    counterparty_public_key: &[u8],
    invoice_number: &str,
) -> Result<Vec<u8>, Brc42Error> {
    let shared = compute_shared_secret(master_private_key, counterparty_public_key)?;
    compute_invoice_hmac(&shared, invoice_number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::{Secp256k1, SecretKey, PublicKey};

    fn pubkey_of(privkey: &[u8; 32]) -> Vec<u8> {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(privkey).unwrap();
        PublicKey::from_secret_key(&secp, &secret).serialize().to_vec()
    }

    #[test]
    fn counterparty_linkage_is_33_byte_compressed_point() {
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let bob_pub = pubkey_of(&bob);

        let linkage = compute_counterparty_linkage(&alice, &bob_pub).unwrap();
        assert_eq!(linkage.len(), 33, "shared secret must be 33-byte compressed");
        assert!(linkage[0] == 0x02 || linkage[0] == 0x03, "must start with 02/03 compression prefix");
    }

    #[test]
    fn counterparty_linkage_is_symmetric_in_ecdh() {
        // The shared secret should match from both perspectives -- this is the
        // property a verifier relies on when they receive the linkage value.
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let alice_pub = pubkey_of(&alice);
        let bob_pub = pubkey_of(&bob);

        let from_alice = compute_counterparty_linkage(&alice, &bob_pub).unwrap();
        let from_bob = compute_counterparty_linkage(&bob, &alice_pub).unwrap();
        assert_eq!(from_alice, from_bob);
    }

    #[test]
    fn specific_linkage_is_32_bytes() {
        let alice = [1u8; 32];
        let bob_pub = pubkey_of(&[2u8; 32]);
        let invoice = "2-test protocol-key1";

        let linkage = compute_specific_linkage(&alice, &bob_pub, invoice).unwrap();
        assert_eq!(linkage.len(), 32);
    }

    #[test]
    fn specific_linkage_differs_by_invoice_number() {
        let alice = [1u8; 32];
        let bob_pub = pubkey_of(&[2u8; 32]);

        let l1 = compute_specific_linkage(&alice, &bob_pub, "2-test protocol-key1").unwrap();
        let l2 = compute_specific_linkage(&alice, &bob_pub, "2-test protocol-key2").unwrap();
        let l3 = compute_specific_linkage(&alice, &bob_pub, "2-other protocol-key1").unwrap();

        assert_ne!(l1, l2, "different keyID must produce different linkage");
        assert_ne!(l1, l3, "different protocolID must produce different linkage");
        assert_ne!(l2, l3);
    }

    #[test]
    fn specific_linkage_differs_by_counterparty() {
        let alice = [1u8; 32];
        let invoice = "2-test protocol-key1";

        let l_to_bob = compute_specific_linkage(&alice, &pubkey_of(&[2u8; 32]), invoice).unwrap();
        let l_to_carol = compute_specific_linkage(&alice, &pubkey_of(&[3u8; 32]), invoice).unwrap();

        assert_ne!(l_to_bob, l_to_carol);
    }

    #[test]
    fn specific_linkage_is_symmetric_under_ecdh() {
        // If two parties share the same shared secret and compute HMAC over the same
        // invoice number, they get the same linkage value.
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let alice_pub = pubkey_of(&alice);
        let bob_pub = pubkey_of(&bob);
        let invoice = "2-shared protocol-shared key";

        let from_alice = compute_specific_linkage(&alice, &bob_pub, invoice).unwrap();
        let from_bob = compute_specific_linkage(&bob, &alice_pub, invoice).unwrap();
        assert_eq!(from_alice, from_bob);
    }
}
