//! Certificate verifier
//!
//! Verifies BRC-52 certificate signatures and revocation status.

use crate::certificate::types::{Certificate, CertificateError};
use crate::crypto::signing::sha256;
use crate::crypto::brc42::derive_child_public_key;
use crate::crypto::brc43::{InvoiceNumber, SecurityLevel};
use crate::transaction::encode_varint;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::Deserialize;

/// Serialize certificate preimage for signature verification
///
/// **BRC-52**: Creates binary preimage from certificate fields (excluding signature)
///
/// ## Binary Format (exact order):
/// 1. type (32 bytes) - Base64-decoded
/// 2. serialNumber (32 bytes) - Base64-decoded
/// 3. subject (33 bytes) - Hex-decoded compressed public key
/// 4. certifier (33 bytes) - Hex-decoded compressed public key
/// 5. revocationOutpoint:
///    - txid (32 bytes) - Hex-decoded
///    - outputIndex (VarInt) - Number
/// 6. fields:
///    - VarInt(count) - Number of fields
///    - For each field (sorted lexicographically by name):
///      - VarInt(nameLength) - Field name length
///      - fieldName (UTF-8 bytes) - Field name
///      - VarInt(valueLength) - Field value length
///      - fieldValue (UTF-8 bytes) - Field value (base64 string as UTF-8 bytes)
///
/// ## Arguments
/// - `certificate`: Certificate to serialize
///
/// ## Returns
/// Preimage bytes (ready for SHA-256 hashing)
pub fn serialize_certificate_preimage(
    certificate: &Certificate,
) -> Result<Vec<u8>, CertificateError> {
    let mut writer = Vec::new();

    // 1. type (32 bytes)
    // Note: In Certificate struct, type_ is stored as Vec<u8> (already decoded)
    // But we need to ensure it's exactly 32 bytes
    if certificate.type_.len() != 32 {
        return Err(CertificateError::InvalidFormat(
            format!("type must be 32 bytes, got {}", certificate.type_.len())
        ));
    }
    writer.extend_from_slice(&certificate.type_);

    // 2. serialNumber (32 bytes)
    if certificate.serial_number.len() != 32 {
        return Err(CertificateError::InvalidFormat(
            format!("serialNumber must be 32 bytes, got {}", certificate.serial_number.len())
        ));
    }
    writer.extend_from_slice(&certificate.serial_number);

    // 3. subject (33 bytes, compressed public key)
    if certificate.subject.len() != 33 {
        return Err(CertificateError::InvalidFormat(
            format!("subject must be 33 bytes, got {}", certificate.subject.len())
        ));
    }
    writer.extend_from_slice(&certificate.subject);

    // 4. certifier (33 bytes, compressed public key)
    if certificate.certifier.len() != 33 {
        return Err(CertificateError::InvalidFormat(
            format!("certifier must be 33 bytes, got {}", certificate.certifier.len())
        ));
    }
    writer.extend_from_slice(&certificate.certifier);

    // 5. revocationOutpoint
    let parts: Vec<&str> = certificate.revocation_outpoint.split('.').collect();
    if parts.len() != 2 {
        return Err(CertificateError::InvalidFormat(
            "revocationOutpoint must be 'txid.vout'".to_string()
        ));
    }

    // Parse txid (32 bytes, hex-decoded)
    let txid_bytes = hex::decode(parts[0])
        .map_err(|e| CertificateError::InvalidHex(format!("revocationOutpoint txid: {}", e)))?;
    if txid_bytes.len() != 32 {
        return Err(CertificateError::InvalidFormat(
            format!("revocationOutpoint txid must be 32 bytes, got {}", txid_bytes.len())
        ));
    }
    writer.extend_from_slice(&txid_bytes);

    // Parse vout (VarInt)
    let vout: u64 = parts[1].parse()
        .map_err(|e| CertificateError::InvalidFormat(format!("revocationOutpoint vout: {}", e)))?;
    writer.extend_from_slice(&encode_varint(vout));

    // 6. fields (sorted lexicographically)
    let mut field_names: Vec<String> = certificate.fields.keys().cloned().collect();
    field_names.sort(); // Lexicographic sort

    // Write field count
    writer.extend_from_slice(&encode_varint(field_names.len() as u64));

    for field_name in field_names {
        let field = &certificate.fields[&field_name];

        // Field name (UTF-8)
        let name_bytes = field_name.as_bytes();
        writer.extend_from_slice(&encode_varint(name_bytes.len() as u64));
        writer.extend_from_slice(name_bytes);

        // Field value (base64 string as UTF-8 bytes)
        // Note: field.field_value is the encrypted bytes (Vec<u8>)
        // We need to base64-encode it to get the base64 string, then convert to UTF-8 bytes
        let value_base64_str = BASE64.encode(&field.field_value);
        let value_bytes = value_base64_str.as_bytes();
        writer.extend_from_slice(&encode_varint(value_bytes.len() as u64));
        writer.extend_from_slice(value_bytes);
    }

    Ok(writer)
}

/// Verify certificate signature
///
/// **BRC-52**: Verifies ECDSA signature over certificate data using BRC-42 key derivation
///
/// ## Process
/// 1. Serialize certificate preimage (exclude signature)
/// 2. Hash preimage with SHA-256
/// 3. Create BRC-43 invoice number: `"2-certificate signature-${type} ${serialNumber}"`
/// 4. Derive child public key using BRC-42 (with 'anyone' as sender)
/// 5. Verify ECDSA signature using derived public key
///
/// ## Arguments
/// - `certificate`: Certificate to verify
///
/// ## Returns
/// Ok(()) if signature is valid, Err if invalid
pub fn verify_certificate_signature(
    certificate: &Certificate,
) -> Result<(), CertificateError> {
    verify_certificate_signature_with_keyid(
        certificate,
        None, // Use re-encoded base64 (backward compatibility)
        None,
    )
}

pub fn verify_certificate_signature_with_keyid(
    certificate: &Certificate,
    type_base64_original: Option<&str>,
    serial_base64_original: Option<&str>,
) -> Result<(), CertificateError> {
    log::info!("   🔍 Verifying certificate signature (BRC-52):");
    log::info!("      Certificate type (hex, {} bytes): {}", certificate.type_.len(), hex::encode(&certificate.type_));
    log::info!("      Serial number (hex, {} bytes): {}", certificate.serial_number.len(), hex::encode(&certificate.serial_number));

    // Verify type and serialNumber are exactly 32 bytes
    if certificate.type_.len() != 32 {
        return Err(CertificateError::InvalidFormat(
            format!("type must be 32 bytes, got {} bytes", certificate.type_.len())
        ));
    }
    if certificate.serial_number.len() != 32 {
        return Err(CertificateError::InvalidFormat(
            format!("serialNumber must be 32 bytes, got {} bytes", certificate.serial_number.len())
        ));
    }
    log::info!("      Subject (hex): {}", hex::encode(&certificate.subject));
    log::info!("      Certifier (hex): {}", hex::encode(&certificate.certifier));
    log::info!("      Revocation outpoint: {}", certificate.revocation_outpoint);
    log::info!("      Signature length: {} bytes", certificate.signature.len());
    log::info!("      Signature (hex, first 32): {}", hex::encode(&certificate.signature[..std::cmp::min(32, certificate.signature.len())]));

    // Check if signature exists
    if certificate.signature.is_empty() {
        return Err(CertificateError::SignatureVerification(
            "Certificate has no signature".to_string()
        ));
    }

    // 1. Serialize certificate preimage (exclude signature)
    let preimage = serialize_certificate_preimage(certificate)?;
    log::info!("      Preimage length: {} bytes", preimage.len());
    log::info!("      Preimage (hex, first 64): {}", hex::encode(&preimage[..std::cmp::min(64, preimage.len())]));
    log::info!("      Preimage (hex, FULL): {}", hex::encode(&preimage));

    // Log each component for debugging
    log::info!("      Preimage breakdown:");
    log::info!("         Type (32 bytes, hex): {}", hex::encode(&preimage[0..32]));
    log::info!("         SerialNumber (32 bytes, hex): {}", hex::encode(&preimage[32..64]));
    log::info!("         Subject (33 bytes, hex): {}", hex::encode(&preimage[64..97]));
    log::info!("         Certifier (33 bytes, hex): {}", hex::encode(&preimage[97..130]));
    if preimage.len() > 130 {
        log::info!("         RevocationOutpoint (hex, first 32): {}", hex::encode(&preimage[130..std::cmp::min(162, preimage.len())]));
    }

    // 2. Hash preimage with SHA-256
    let hash = sha256(&preimage);
    log::info!("      Hash (SHA256, hex): {}", hex::encode(&hash));

    // 3. Create invoice number for BRC-42
    // Format: "2-certificate signature-${type} ${serialNumber}"
    // Note: type and serialNumber are base64-encoded in the invoice number
    // CRITICAL: Use original base64 strings from JSON if provided, otherwise re-encode
    // The server uses the original base64 strings directly, so we must match exactly
    let type_base64 = type_base64_original
        .map(|s| s.to_string())
        .unwrap_or_else(|| BASE64.encode(&certificate.type_));
    let serial_base64 = serial_base64_original
        .map(|s| s.to_string())
        .unwrap_or_else(|| BASE64.encode(&certificate.serial_number));
    let key_id = format!("{} {}", type_base64, serial_base64);
    log::info!("      KeyID: {}", key_id);
    if type_base64_original.is_some() || serial_base64_original.is_some() {
        log::info!("      ✅ Using original base64 strings from JSON (matching server)");
        log::info!("      Original type (base64): {}", type_base64_original.unwrap_or(""));
        log::info!("      Original serialNumber (base64): {}", serial_base64_original.unwrap_or(""));
    } else {
        log::info!("      ⚠️  Using re-encoded base64 (may not match server's original strings)");
        log::info!("      Re-encoded type (base64): {}", type_base64);
        log::info!("      Re-encoded serialNumber (base64): {}", serial_base64);
    }

    let invoice = InvoiceNumber::new(
        SecurityLevel::CounterpartyLevel, // Level 2
        "certificate signature",
        &key_id,
    ).map_err(|e| CertificateError::InvalidFormat(format!("Invoice number: {}", e)))?;
    log::info!("      Invoice number: {}", invoice.to_string());
    log::info!("      Invoice number bytes (UTF-8, hex): {}", hex::encode(invoice.to_string().as_bytes()));

    // 4. Derive public key using BRC-42
    // CRITICAL INSIGHT from SDK's Certificate.verify():
    // - When signing: createSignature() is called WITHOUT counterparty (undefined)
    //   - SDK sends 0 to wallet, which wallet interprets as 'self'
    //   - 'self' means: normalizeCounterparty('self') = certifier's own public key
    // - When verifying: verifySignature() uses counterparty: this.certifier (certifier's public key)
    //
    // So both signing and verifying use the certifier's public key as the counterparty!
    // For public verification, we use 'anyone' (private key 1) as the sender,
    // with certifier's public key as the counterparty (recipient).
    // This gives us: childPubkey = certifierPubkey + (HMAC(ECDH(1, certifierPubkey), invoiceNumber) * G)
    // which should match what the certifier computed when signing with 'self'.
    // CRITICAL INSIGHT: The SDK's Certificate.verify() uses:
    // - verifier = new ProtoWallet('anyone') (private key 1)
    // - counterparty: this.certifier (certifier's public key)
    // - forSelf: false (default)
    //
    // This calls: certifier_pubkey.deriveChild(anyone_privkey, invoiceNumber)
    // Which computes: ECDH(anyone_privkey, certifier_pubkey) = 1 * certifier_pubkey = certifier_pubkey
    //
    // When the server signs with 'self', it uses:
    // - certifier_privkey.deriveChild(certifier_pubkey, invoiceNumber)
    // Which computes: ECDH(certifier_privkey, certifier_pubkey) = certifier_privkey * certifier_pubkey
    //
    // These shared secrets ARE different, but the SDK's verify() works!
    // This means the SDK must be doing something that makes them match.
    //
    // SOLUTION: The SDK's verify() works because BRC-42 is symmetric!
    // When server signs: ECDH(server_privkey, server_pubkey) = server_privkey * server_pubkey
    // When we verify: ECDH(1, server_pubkey) = 1 * server_pubkey = server_pubkey
    //
    // But wait - these are still different! Unless... maybe the wallet handles 'self' specially?
    // Or maybe we need to check if the SDK actually works with real servers?
    //
    // Let's try the SDK's exact approach: use 'anyone' as sender with certifier's pubkey as recipient
    // CRITICAL: Private key 1 is 31 zeros followed by 1, NOT all bytes set to 1!
    let mut anyone_private_key = [0u8; 32];
    anyone_private_key[31] = 1; // Private key with value 1 (public key is 1*G = anyone)
    log::info!("      Using 'anyone' (private key 1) as sender for public derivation");
    log::info!("      Anyone private key (hex): {}", hex::encode(&anyone_private_key));
    log::info!("      Counterparty: certifier's public key ({})", hex::encode(&certificate.certifier));
    log::info!("      This matches SDK's verify() which uses: verifier = new ProtoWallet('anyone'), counterparty: this.certifier");

    // Log detailed derivation steps
    log::info!("      📊 Detailed BRC-42 Key Derivation Steps:");
    log::info!("         Step 1: Compute shared secret = ECDH(anyone_privkey, certifier_pubkey)");

    // CRITICAL: The SDK's verify() works, so our approach should work too.
    // The SDK's Certificate.verify() uses:
    // - verifier = new ProtoWallet('anyone') (private key 1)
    // - counterparty: this.certifier (certifier's public key)
    // - forSelf: false (default)
    //
    // This calls: certifier_pubkey.deriveChild(anyone_privkey, invoiceNumber)
    // Which computes: ECDH(anyone_privkey, certifier_pubkey) = 1 * certifier_pubkey = certifier_pubkey
    //
    // When the server signs with 'self' (counterparty = undefined), the wallet might:
    // - Use forSelf: true → certifier_privkey.deriveChild(certifier_pubkey, invoiceNumber)
    //   → Shared secret: ECDH(certifier_privkey, certifier_pubkey) = certifier_privkey * certifier_pubkey
    // - OR use forSelf: false with special handling
    //
    // Since the SDK's verify() works, the server must be using forSelf: false with certifier_pubkey as counterparty!
    // This means the shared secret should be: ECDH(certifier_privkey, certifier_pubkey) when signing
    // But when verifying, we use: ECDH(1, certifier_pubkey) = certifier_pubkey
    //
    // These are DIFFERENT! But the SDK works, so maybe the wallet uses a different computation for 'self'?
    // Or maybe we're missing something in our implementation?
    // Add detailed logging inside derive_child_public_key by calling helper functions directly
    use crate::crypto::brc42::{compute_shared_secret, compute_invoice_hmac};

    // Step 1: Compute shared secret
    let shared_secret = compute_shared_secret(&anyone_private_key, &certificate.certifier)
        .map_err(|e| CertificateError::SignatureVerification(format!("Shared secret computation failed: {}", e)))?;
    log::info!("         Shared secret (ECDH result, hex, first 16): {}", hex::encode(&shared_secret[..std::cmp::min(16, shared_secret.len())]));
    log::info!("         Shared secret length: {} bytes", shared_secret.len());

    // Step 2: Compute HMAC over invoice number
    log::info!("         Step 2: Compute HMAC-SHA256(shared_secret, invoice_number)");
    let hmac_output = compute_invoice_hmac(&shared_secret, &invoice.to_string())
        .map_err(|e| CertificateError::SignatureVerification(format!("HMAC computation failed: {}", e)))?;
    log::info!("         HMAC output (32 bytes, hex): {}", hex::encode(&hmac_output));

    // Step 3: Convert HMAC to scalar
    log::info!("         Step 3: Convert HMAC to scalar (for BRC-42 child key derivation)");
    use secp256k1::SecretKey;
    let hmac_secret = SecretKey::from_slice(&hmac_output)
        .map_err(|e| CertificateError::SignatureVerification(format!("Invalid HMAC for scalar: {}", e)))?;
    log::info!("         HMAC scalar (hex): {}", hex::encode(&hmac_secret.secret_bytes()));

    // Step 4 & 5: Compute child public key = certifier_pubkey + (HMAC_scalar * G)
    log::info!("         Step 4-5: Compute child_pubkey = certifier_pubkey + (HMAC_scalar * G)");
    let derived_pubkey = derive_child_public_key(
        &anyone_private_key,
        &certificate.certifier,
        &invoice.to_string(),
    ).map_err(|e| CertificateError::SignatureVerification(
        format!("Key derivation failed: {}", e)
    ))?;
    log::info!("         ✅ Derived child public key (33 bytes, hex): {}", hex::encode(&derived_pubkey));
    log::info!("      📊 Derivation complete!");
    log::info!("      ⚠️  Verification is failing - derived key doesn't match server's signing key");
    log::info!("      ⚠️  This suggests the shared secret computation differs between signing and verifying");

    // 5. Verify ECDSA signature
    // Note: BRC-52 certificate signatures are DER-encoded
    // They may or may not include a sighash type byte (depends on implementation)
    // Try both: first as DER-only, then with sighash type byte removed
    use secp256k1::{Secp256k1, Message, PublicKey, ecdsa::Signature};

    if certificate.signature.is_empty() {
        return Err(CertificateError::SignatureVerification(
            "Signature is empty".to_string()
        ));
    }

    // Create secp256k1 context
    let secp = Secp256k1::verification_only();

    // Parse public key
    let public_key = PublicKey::from_slice(&derived_pubkey)
        .map_err(|e| CertificateError::SignatureVerification(
            format!("Invalid public key: {}", e)
        ))?;

    // Parse message (hash)
    let message = Message::from_digest_slice(&hash)
        .map_err(|e| CertificateError::SignatureVerification(
            format!("Invalid message hash: {}", e)
        ))?;

    // Try parsing signature as DER-only first (BRC-52 standard format)
    let signature = match Signature::from_der(&certificate.signature) {
        Ok(sig) => {
            log::info!("      ✅ Signature parsed as DER-only (no sighash byte)");
            sig
        },
        Err(e) => {
            log::info!("      ⚠️  Failed to parse as DER-only: {}, trying with last byte removed", e);
            // If that fails, try removing last byte (sighash type byte)
            if certificate.signature.len() > 1 {
                let sig_without_sighash = Signature::from_der(&certificate.signature[..certificate.signature.len() - 1])
                    .map_err(|e| {
                        log::error!("      ❌ Failed to parse signature even with last byte removed: {}", e);
                        CertificateError::SignatureVerification(
                            format!("Invalid signature format: {}", e)
                        )
                    })?;
                log::info!("      ✅ Signature parsed with last byte removed (sighash byte)");
                sig_without_sighash
            } else {
                return Err(CertificateError::SignatureVerification(
                    "Signature too short".to_string()
                ));
            }
        }
    };

    log::info!("      Signature (DER, hex): {}", hex::encode(&certificate.signature));

    // Verify signature
    let is_valid = secp.verify_ecdsa(&message, &signature, &public_key).is_ok();
    log::info!("      Signature verification result: {}", if is_valid { "✅ VALID" } else { "❌ INVALID" });

    if !is_valid {
        log::error!("      ❌ Signature verification failed!");
        log::error!("      Expected public key (derived, hex): {}", hex::encode(&derived_pubkey));
        log::error!("      Message hash (hex): {}", hex::encode(&hash));
        log::error!("      Signature (DER, hex, full): {}", hex::encode(&certificate.signature));
        return Err(CertificateError::SignatureVerification(
            "Signature is invalid".to_string()
        ));
    }

    log::info!("      ✅ Certificate signature verified successfully!");
    Ok(())
}

/// Check certificate revocation status
///
/// **BRC-52**: Checks if revocation outpoint UTXO is spent
///
/// ## Process
/// 1. Parse revocation outpoint (format: "txid.vout")
/// 2. Query WhatsOnChain API to check if UTXO is spent
/// 3. Return true if spent (revoked), false if unspent (active)
///
/// ## Arguments
/// - `revocation_outpoint`: Revocation outpoint (format: "txid.vout")
///
/// ## Returns
/// Ok(false) if active (unspent), Ok(true) if revoked (spent), Err on error
pub async fn check_revocation_status(
    revocation_outpoint: &str,
) -> Result<bool, CertificateError> {
    log::info!("   Checking revocation status for: {}", revocation_outpoint);

    // Parse outpoint (txid.vout)
    let parts: Vec<&str> = revocation_outpoint.split('.').collect();
    if parts.len() != 2 {
        return Err(CertificateError::InvalidFormat(
            "revocationOutpoint must be 'txid.vout'".to_string()
        ));
    }

    let txid = parts[0];
    let vout: u32 = parts[1].parse()
        .map_err(|e| CertificateError::InvalidFormat(format!("Invalid vout: {}", e)))?;

    // Validate txid format (64 hex characters = 32 bytes)
    if txid.len() != 64 || !txid.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(CertificateError::InvalidFormat(
            format!("Invalid txid format: {}", txid)
        ));
    }

    // Query WhatsOnChain API to check if UTXO is spent
    // API endpoint: GET /v1/bsv/main/tx/{txid}/outspend/{vout}
    // Returns: { "spent": true/false, "txid": "...", "vin": ... }
    let url = format!(
        "https://api.whatsonchain.com/v1/bsv/main/tx/{}/outspend/{}",
        txid, vout
    );

    log::debug!("   Querying WhatsOnChain: {}", url);

    let client = reqwest::Client::new();
    let response = match client.get(&url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("   Failed to query WhatsOnChain API: {}", e);
            return Err(CertificateError::Database(
                format!("Failed to query revocation status: {}", e)
            ));
        }
    };

    let status = response.status();

    if !status.is_success() {
        if status == 404 {
            // Transaction or output not found - treat as unspent (certificate is active)
            // This could mean the transaction doesn't exist yet, or the output index is invalid
            log::warn!("   Transaction/output not found (404) - treating as active");
            return Ok(false);
        }
        log::error!("   WhatsOnChain API returned status: {}", status);
        return Err(CertificateError::Database(
            format!("WhatsOnChain API error: {}", status)
        ));
    }

    // Parse response
    // WhatsOnChain API returns: { "spent": true/false, "txid": "...", "vin": ... }
    let response_text = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            log::error!("   Failed to read WhatsOnChain response: {}", e);
            return Err(CertificateError::Database(
                format!("Failed to read revocation status: {}", e)
            ));
        }
    };

    #[derive(Debug, Deserialize)]
    struct OutspendResponse {
        spent: bool,
        #[serde(default)]
        txid: Option<String>,  // Spending transaction ID (if spent)
        #[serde(default)]
        vin: Option<u32>,      // Input index in spending transaction (if spent)
    }

    let outspend: OutspendResponse = match serde_json::from_str(&response_text) {
        Ok(data) => data,
        Err(e) => {
            log::error!("   Failed to parse WhatsOnChain response: {}", e);
            return Err(CertificateError::Database(
                format!("Failed to parse revocation status: {}", e)
            ));
        }
    };

    if outspend.spent {
        log::warn!("   ⚠️  Certificate REVOKED - UTXO is spent");
        if let Some(spending_txid) = outspend.txid {
            log::info!("   Spending transaction: {}", spending_txid);
        }
        Ok(true) // Revoked
    } else {
        log::info!("   ✅ Certificate ACTIVE - UTXO is unspent");
        Ok(false) // Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::certificate::types::CertificateField;

    #[test]
    fn test_serialize_preimage_basic() {
        // Create a minimal certificate
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), CertificateField::new(
            "name".to_string(),
            vec![1, 2, 3, 4], // Encrypted value
            vec![5, 6, 7, 8],  // Master key
        ));

        let cert = Certificate::new(
            vec![0u8; 32], // type
            vec![0u8; 33],  // subject
            vec![0u8; 32],  // serial_number
            vec![0u8; 33],  // certifier
            "0000000000000000000000000000000000000000000000000000000000000000.0".to_string(), // revocation_outpoint
            vec![], // signature (empty for preimage)
            fields,
            HashMap::new(), // keyring
        );

        let preimage = serialize_certificate_preimage(&cert).unwrap();

        // Preimage should be:
        // - 32 bytes (type)
        // - 32 bytes (serialNumber)
        // - 33 bytes (subject)
        // - 33 bytes (certifier)
        // - 32 bytes (txid)
        // - 1 byte (VarInt for vout=0)
        // - 1 byte (VarInt for field count=1)
        // - 1 byte (VarInt for name length=4)
        // - 4 bytes ("name")
        // - 1 byte (VarInt for value length)
        // - value bytes (base64 of [1,2,3,4] = "AQIDBA==" = 8 bytes)
        let expected_min_size = 32 + 32 + 33 + 33 + 32 + 1 + 1 + 1 + 4 + 1 + 8;
        assert!(preimage.len() >= expected_min_size,
            "Preimage too short: {} bytes (expected at least {})",
            preimage.len(), expected_min_size);
    }

    #[test]
    fn test_serialize_preimage_field_ordering() {
        // Create certificate with fields in non-lexicographic order
        let mut fields = HashMap::new();
        fields.insert("zebra".to_string(), CertificateField::new(
            "zebra".to_string(),
            vec![1],
            vec![2],
        ));
        fields.insert("alpha".to_string(), CertificateField::new(
            "alpha".to_string(),
            vec![3],
            vec![4],
        ));

        let cert = Certificate::new(
            vec![0u8; 32],
            vec![0u8; 33],
            vec![0u8; 32],
            vec![0u8; 33],
            "0000000000000000000000000000000000000000000000000000000000000000.0".to_string(),
            vec![],
            fields,
            HashMap::new(),
        );

        let preimage = serialize_certificate_preimage(&cert).unwrap();

        // Fields should be sorted: "alpha" comes before "zebra"
        // Find "alpha" and "zebra" in the preimage
        let preimage_str = String::from_utf8_lossy(&preimage);
        let alpha_pos = preimage_str.find("alpha").unwrap_or(0);
        let zebra_pos = preimage_str.find("zebra").unwrap_or(0);

        assert!(alpha_pos < zebra_pos,
            "Fields should be sorted lexicographically: alpha at {}, zebra at {}",
            alpha_pos, zebra_pos);
    }

    #[test]
    fn test_serialize_preimage_invalid_type_length() {
        let cert = Certificate::new(
            vec![0u8; 31], // Invalid: should be 32 bytes
            vec![0u8; 33],
            vec![0u8; 32],
            vec![0u8; 33],
            "0000000000000000000000000000000000000000000000000000000000000000.0".to_string(),
            vec![],
            HashMap::new(),
            HashMap::new(),
        );

        let result = serialize_certificate_preimage(&cert);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("type must be 32 bytes"));
    }

    #[test]
    fn test_serialize_preimage_invalid_revocation_outpoint() {
        let cert = Certificate::new(
            vec![0u8; 32],
            vec![0u8; 33],
            vec![0u8; 32],
            vec![0u8; 33],
            "invalid".to_string(), // Invalid format
            vec![],
            HashMap::new(),
            HashMap::new(),
        );

        let result = serialize_certificate_preimage(&cert);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("revocationOutpoint"));
    }

    #[tokio::test]
    async fn test_check_revocation_status_invalid_format() {
        // Test with invalid outpoint format
        let result = check_revocation_status("invalid").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_revocation_status_invalid_txid() {
        // Test with invalid txid format
        let result = check_revocation_status("invalid.0").await;
        assert!(result.is_err());
    }
}
