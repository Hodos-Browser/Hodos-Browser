//! BRC-42: BSV Key Derivation Scheme Implementation
//!
//! Implements the BRC-42 specification for deriving child keys between two parties
//! using ECDH shared secrets and HMAC-based key derivation.
//!
//! **Reference**: BRC-42 specification
//! https://github.com/bitcoin-sv/BRCs/blob/master/key-derivation/0042.md

use secp256k1::{Secp256k1, SecretKey, PublicKey};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// BRC-42 key derivation errors
#[derive(Debug, thiserror::Error)]
pub enum Brc42Error {
    #[error("invalid private key: {0}")]
    InvalidPrivateKey(String),

    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("derivation failed: {0}")]
    DerivationFailed(String),

    #[error("secp256k1 error: {0}")]
    Secp256k1Error(String),
}

/// Compute ECDH shared secret between sender's private key and recipient's public key
///
/// **BRC-42 Spec**: Step 1 for both sender and recipient
///
/// The shared secret is computed by multiplying the private key with the public key
/// using elliptic curve point multiplication, then serializing as compressed public key.
///
/// **TypeScript Reference**: sharedSecret.encode(true) returns 33-byte compressed point
///
/// ## Arguments
/// - `private_key`: 32-byte private key (sender or recipient)
/// - `public_key`: 33-byte compressed public key (counterparty)
///
/// ## Returns
/// 33-byte compressed shared secret (ECDH point as compressed public key)
pub fn compute_shared_secret(
    private_key: &[u8],
    public_key: &[u8],
) -> Result<Vec<u8>, Brc42Error> {
    if private_key.len() != 32 {
        return Err(Brc42Error::InvalidPrivateKey(
            format!("Private key must be 32 bytes, got {}", private_key.len())
        ));
    }

    let secp = Secp256k1::new();

    // Parse keys
    let secret = SecretKey::from_slice(private_key)
        .map_err(|e| Brc42Error::InvalidPrivateKey(e.to_string()))?;

    let pubkey = PublicKey::from_slice(public_key)
        .map_err(|e| Brc42Error::InvalidPublicKey(e.to_string()))?;

    // Compute ECDH: privkey * pubkey
    // mul_tweak multiplies the public key by the scalar (private key)
    let shared_point = pubkey.mul_tweak(&secp, &secret.into())
        .map_err(|e| Brc42Error::Secp256k1Error(e.to_string()))?;

    // Serialize as compressed (33 bytes) - THIS IS THE KEY DIFFERENCE
    // TypeScript: sharedSecret.encode(true) returns compressed format
    // Format: [0x02 or 0x03] + [32-byte x-coordinate]
    Ok(shared_point.serialize().to_vec())
}

/// Compute HMAC-SHA256 over invoice number using shared secret as key
///
/// **BRC-42 Spec**: Step 2 for both sender and recipient
///
/// **TypeScript Reference**: sha256hmac(sharedSecret.encode(true), invoiceNumberBin)
///
/// ## Arguments
/// - `shared_secret`: 33-byte compressed shared secret from ECDH
/// - `invoice_number`: UTF-8 encoded invoice number string
///
/// ## Returns
/// 32-byte HMAC output
pub fn compute_invoice_hmac(shared_secret: &[u8], invoice_number: &str) -> Result<Vec<u8>, Brc42Error> {
    let mut mac = HmacSha256::new_from_slice(shared_secret)
        .map_err(|e| Brc42Error::DerivationFailed(format!("HMAC init failed: {}", e)))?;

    mac.update(invoice_number.as_bytes());

    Ok(mac.finalize().into_bytes().to_vec())
}

/// Derive symmetric key for HMAC using BRC-42
///
/// **TypeScript Reference**: keyDeriver.deriveSymmetricKey(protocolID, keyID, counterparty)
///
/// The SDK's deriveSymmetricKey:
/// 1. Derives our child private key (using BRC-42 with invoice number)
/// 2. Derives their child public key (using BRC-42 with invoice number)
/// 3. Computes ECDH between our child private key and their child public key
/// 4. Takes the x-coordinate of the result as the symmetric key
///
/// ## Algorithm
/// 1. Derive our child private key: our_master_privkey + HMAC_scalar (BRC-42)
/// 2. Derive their child public key: their_master_pubkey + HMAC_scalar*G (BRC-42)
/// 3. Compute ECDH: our_child_privkey * their_child_pubkey
/// 4. Extract x-coordinate (32 bytes) as symmetric key
///
/// ## Arguments
/// - `our_private_key`: Our 32-byte master private key
/// - `their_public_key`: Their 33-byte master public key (counterparty)
/// - `invoice_number`: UTF-8 invoice number string (format: "{level}-{protocolID}-{keyID}")
///
/// ## Returns
/// 32-byte symmetric key for HMAC operations (x-coordinate of ECDH result)
pub fn derive_symmetric_key_for_hmac(
    our_private_key: &[u8],
    their_public_key: &[u8],
    invoice_number: &str,
) -> Result<Vec<u8>, Brc42Error> {
    let secp = Secp256k1::new();

    // Step 1: Compute shared secret at master level (for HMAC scalar derivation)
    let master_shared_secret = compute_shared_secret(our_private_key, their_public_key)?;

    // Step 2: Compute HMAC over invoice number with master shared secret
    // This gives us the scalar to add to keys (BRC-42 step)
    let hmac_output = compute_invoice_hmac(&master_shared_secret, invoice_number)?;

    // Step 3: Convert HMAC to scalar (for BRC-42 child key derivation)
    let hmac_secret = SecretKey::from_slice(&hmac_output)
        .map_err(|e| Brc42Error::DerivationFailed(format!("Invalid HMAC for scalar: {}", e)))?;

    // Step 4: Derive our child private key (our_master_privkey + HMAC_scalar)
    let our_secret = SecretKey::from_slice(our_private_key)
        .map_err(|e| Brc42Error::InvalidPrivateKey(e.to_string()))?;
    let our_child_secret = our_secret.add_tweak(&hmac_secret.into())
        .map_err(|e| Brc42Error::Secp256k1Error(e.to_string()))?;

    // Step 5: Derive their child public key (their_master_pubkey + HMAC_scalar*G)
    let their_pubkey = PublicKey::from_slice(their_public_key)
        .map_err(|e| Brc42Error::InvalidPublicKey(e.to_string()))?;
    let their_child_pubkey = their_pubkey.add_exp_tweak(&secp, &hmac_secret.into())
        .map_err(|e| Brc42Error::Secp256k1Error(e.to_string()))?;

    // Step 6: Compute ECDH between our child private key and their child public key
    // This gives us the shared secret point
    let shared_point = their_child_pubkey.mul_tweak(&secp, &our_child_secret.into())
        .map_err(|e| Brc42Error::Secp256k1Error(e.to_string()))?;

    // Step 7: Extract x-coordinate (32 bytes) as symmetric key
    // The SDK uses: derivedPrivateKey.deriveSharedSecret(derivedPublicKey)?.x?.toArray()
    // This returns the x-coordinate in big-endian format, padded to 32 bytes
    let shared_point_bytes = shared_point.serialize();
    let x_coordinate = &shared_point_bytes[1..33]; // Skip compression byte, get x-coordinate

    Ok(x_coordinate.to_vec())
}

/// Derive child public key for recipient (sender's perspective)
///
/// **BRC-42 Spec**: Steps for the Sender (lines 1-6)
///
/// The sender derives a child public key for the recipient without knowing
/// the recipient's private key.
///
/// ## Algorithm
/// 1. Compute shared secret: sender_privkey * recipient_pubkey
/// 2. Compute HMAC over invoice number with shared secret
/// 3. Convert HMAC to scalar (big-endian)
/// 4. Compute: scalar * G
/// 5. Add result to recipient's public key
/// 6. Result is the child public key
///
/// ## Arguments
/// - `sender_private_key`: Sender's 32-byte master private key
/// - `recipient_public_key`: Recipient's 33-byte master public key
/// - `invoice_number`: UTF-8 invoice number string
///
/// ## Returns
/// 33-byte compressed child public key for recipient
pub fn derive_child_public_key(
    sender_private_key: &[u8],
    recipient_public_key: &[u8],
    invoice_number: &str,
) -> Result<Vec<u8>, Brc42Error> {
    let secp = Secp256k1::new();

    // Step 1: Compute shared secret
    let shared_secret = compute_shared_secret(sender_private_key, recipient_public_key)?;

    // Step 2: Compute HMAC over invoice number
    let hmac_output = compute_invoice_hmac(&shared_secret, invoice_number)?;

    // Step 3: Convert HMAC to scalar - treat as private key to get the scalar value
    let hmac_secret = SecretKey::from_slice(&hmac_output)
        .map_err(|e| Brc42Error::DerivationFailed(format!("Invalid HMAC for scalar: {}", e)))?;

    // Parse recipient's public key
    let recipient_pubkey = PublicKey::from_slice(recipient_public_key)
        .map_err(|e| Brc42Error::InvalidPublicKey(e.to_string()))?;

    // Step 4 & 5: hmac_scalar * G + recipient_pubkey
    let child_pubkey = recipient_pubkey.add_exp_tweak(&secp, &hmac_secret.into())
        .map_err(|e| Brc42Error::Secp256k1Error(e.to_string()))?;

    // Step 6: Return compressed child public key
    Ok(child_pubkey.serialize().to_vec())
}

/// Derive child private key (recipient's perspective)
///
/// **BRC-42 Spec**: Steps for the Recipient (lines 1-4)
///
/// The recipient derives the child private key corresponding to the
/// child public key derived by the sender.
///
/// ## Algorithm
/// 1. Compute shared secret: recipient_privkey * sender_pubkey
/// 2. Compute HMAC over invoice number with shared secret
/// 3. Convert HMAC to scalar (big-endian)
/// 4. Add scalar to recipient's private key (mod N)
/// 5. Result is the child private key
///
/// ## Arguments
/// - `recipient_private_key`: Recipient's 32-byte master private key
/// - `sender_public_key`: Sender's 33-byte master public key
/// - `invoice_number`: UTF-8 invoice number string
///
/// ## Returns
/// 32-byte child private key
pub fn derive_child_private_key(
    recipient_private_key: &[u8],
    sender_public_key: &[u8],
    invoice_number: &str,
) -> Result<Vec<u8>, Brc42Error> {
    // Step 1: Compute shared secret
    let shared_secret = compute_shared_secret(recipient_private_key, sender_public_key)?;

    // Step 2: Compute HMAC over invoice number
    let hmac_output = compute_invoice_hmac(&shared_secret, invoice_number)?;

    // Step 3: Convert HMAC to scalar (big-endian) - treat as a private key for addition
    let hmac_secret = SecretKey::from_slice(&hmac_output)
        .map_err(|e| Brc42Error::DerivationFailed(format!("Invalid HMAC for secret key: {}", e)))?;

    // Parse recipient's private key
    let recipient_secret = SecretKey::from_slice(recipient_private_key)
        .map_err(|e| Brc42Error::InvalidPrivateKey(e.to_string()))?;

    // Step 4: Add HMAC (as scalar) to private key (mod N)
    let child_secret = recipient_secret.add_tweak(&hmac_secret.into())
        .map_err(|e| Brc42Error::Secp256k1Error(e.to_string()))?;

    // Return child private key
    Ok(child_secret.secret_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vectors from BRC-42 specification

    #[test]
    fn test_private_key_derivation_vector_1() {
        // BRC-42 Test Vector 1 for private key derivation
        let sender_pubkey = hex::decode("033f9160df035156f1c48e75eae99914fa1a1546bec19781e8eddb900200bff9d1").unwrap();
        let recipient_privkey = hex::decode("6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede").unwrap();
        let invoice_number = "f3WCaUmnN9U=";
        let expected_privkey = hex::decode("761656715bbfa172f8f9f58f5af95d9d0dfd69014cfdcacc9a245a10ff8893ef").unwrap();

        let derived = derive_child_private_key(&recipient_privkey, &sender_pubkey, invoice_number).unwrap();

        assert_eq!(derived, expected_privkey);
    }

    #[test]
    fn test_private_key_derivation_vector_2() {
        // BRC-42 Test Vector 2
        let sender_pubkey = hex::decode("027775fa43959548497eb510541ac34b01d5ee9ea768de74244a4a25f7b60fae8d").unwrap();
        let recipient_privkey = hex::decode("cab2500e206f31bc18a8af9d6f44f0b9a208c32d5cca2b22acfe9d1a213b2f36").unwrap();
        let invoice_number = "2Ska++APzEc=";
        let expected_privkey = hex::decode("09f2b48bd75f4da6429ac70b5dce863d5ed2b350b6f2119af5626914bdb7c276").unwrap();

        let derived = derive_child_private_key(&recipient_privkey, &sender_pubkey, invoice_number).unwrap();

        assert_eq!(derived, expected_privkey);
    }

    #[test]
    fn test_public_key_derivation_vector_1() {
        // BRC-42 Test Vector 1 for public key derivation
        let sender_privkey = hex::decode("583755110a8c059de5cd81b8a04e1be884c46083ade3f779c1e022f6f89da94c").unwrap();
        let recipient_pubkey = hex::decode("02c0c1e1a1f7d247827d1bcf399f0ef2deef7695c322fd91a01a91378f101b6ffc").unwrap();
        let invoice_number = "IBioA4D/OaE=";
        let expected_pubkey = hex::decode("03c1bf5baadee39721ae8c9882b3cf324f0bf3b9eb3fc1b8af8089ca7a7c2e669f").unwrap();

        let derived = derive_child_public_key(&sender_privkey, &recipient_pubkey, invoice_number).unwrap();

        assert_eq!(derived, expected_pubkey);
    }

    #[test]
    fn test_public_key_derivation_vector_2() {
        // BRC-42 Test Vector 2
        let sender_privkey = hex::decode("2c378b43d887d72200639890c11d79e8f22728d032a5733ba3d7be623d1bb118").unwrap();
        let recipient_pubkey = hex::decode("039a9da906ecb8ced5c87971e9c2e7c921e66ad450fd4fc0a7d569fdb5bede8e0f").unwrap();
        let invoice_number = "PWYuo9PDKvI=";
        let expected_pubkey = hex::decode("0398cdf4b56a3b2e106224ff3be5253afd5b72de735d647831be51c713c9077848").unwrap();

        let derived = derive_child_public_key(&sender_privkey, &recipient_pubkey, invoice_number).unwrap();

        assert_eq!(derived, expected_pubkey);
    }

    #[test]
    fn test_shared_secret_symmetry() {
        // Shared secret should be the same from both perspectives
        let alice_privkey = [1u8; 32];
        let bob_privkey = [2u8; 32];

        // Derive public keys
        let secp = Secp256k1::new();
        let alice_secret = SecretKey::from_slice(&alice_privkey).unwrap();
        let bob_secret = SecretKey::from_slice(&bob_privkey).unwrap();

        let alice_pubkey = PublicKey::from_secret_key(&secp, &alice_secret).serialize().to_vec();
        let bob_pubkey = PublicKey::from_secret_key(&secp, &bob_secret).serialize().to_vec();

        // Compute shared secrets
        let secret_ab = compute_shared_secret(&alice_privkey, &bob_pubkey).unwrap();
        let secret_ba = compute_shared_secret(&bob_privkey, &alice_pubkey).unwrap();

        assert_eq!(secret_ab, secret_ba);
    }
}
