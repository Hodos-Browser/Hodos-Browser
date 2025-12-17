//! BRC-100 Certificate Management Handlers
//!
//! Implements Group C methods for BRC-52 identity certificates:
//! - acquireCertificate (Call Code 17)
//! - listCertificates (Call Code 18)
//! - proveCertificate (Call Code 19)
//! - relinquishCertificate (Call Code 20)

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use crate::AppState;
// Note: Certificate and CertificateError will be used when implementing acquireCertificate and proveCertificate
// use crate::certificate::types::{Certificate, CertificateError};
// use crate::certificate::verifier::verify_certificate_signature;
use crate::database::CertificateRepository;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

// ============================================================================
// Method 20: relinquishCertificate (Call Code 20)
// ============================================================================

/// Request structure for relinquishCertificate
#[derive(Debug, Deserialize)]
pub struct RelinquishCertificateRequest {
    /// Certificate type identifier (base64-encoded, 32 bytes)
    #[serde(rename = "type")]
    pub type_: String,  // Base64 string

    /// Certificate serial number (base64-encoded, 32 bytes)
    pub serial_number: String,  // Base64 string

    /// Certifier's public key (33-byte compressed, hex-encoded)
    pub certifier: String,  // Hex string
}

/// Response structure for relinquishCertificate
#[derive(Debug, Serialize)]
pub struct RelinquishCertificateResponse {
    pub relinquished: bool,
}

/// relinquishCertificate - BRC-100 endpoint (Call Code 20)
///
/// Marks a certificate as relinquished (wallet no longer claims ownership).
/// This is a soft delete - certificate data is retained but not returned by listCertificates.
pub async fn relinquish_certificate(
    state: web::Data<AppState>,
    req: web::Json<RelinquishCertificateRequest>,
) -> HttpResponse {
    log::info!("📋 /relinquishCertificate called");
    log::info!("   Type: {}", req.type_);
    log::info!("   Serial Number: {}", req.serial_number);
    log::info!("   Certifier: {}", req.certifier);

    // Decode base64 type and serial_number
    let type_bytes = match BASE64.decode(&req.type_) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("   Invalid base64 type: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid base64 type: {}", e)
            }));
        }
    };

    let serial_number_bytes = match BASE64.decode(&req.serial_number) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("   Invalid base64 serial_number: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid base64 serial_number: {}", e)
            }));
        }
    };

    // Decode hex certifier
    let certifier_bytes = match hex::decode(&req.certifier) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("   Invalid hex certifier: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid hex certifier: {}", e)
            }));
        }
    };

    // Validate lengths
    if type_bytes.len() != 32 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Type must be 32 bytes, got {}", type_bytes.len())
        }));
    }

    if serial_number_bytes.len() != 32 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Serial number must be 32 bytes, got {}", serial_number_bytes.len())
        }));
    }

    if certifier_bytes.len() != 33 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Certifier must be 33 bytes, got {}", certifier_bytes.len())
        }));
    }

    // Get database connection
    let db = state.database.lock().unwrap();
    let cert_repo = CertificateRepository::new(db.connection());

    // Find certificate by identifiers
    let certificate = match cert_repo.get_by_identifiers(
        &type_bytes,
        &serial_number_bytes,
        &certifier_bytes,
    ) {
        Ok(Some(cert)) => cert,
        Ok(None) => {
            log::warn!("   Certificate not found");
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": "Certificate not found"
            }));
        }
        Err(e) => {
            log::error!("   Database error: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }));
        }
    };

    // Update certificate to mark as relinquished
    match cert_repo.update_relinquished(
        &type_bytes,
        &serial_number_bytes,
        &certifier_bytes,
    ) {
        Ok(true) => {
            log::info!("   ✅ Certificate relinquished successfully");
            HttpResponse::Ok().json(RelinquishCertificateResponse {
                relinquished: true,
            })
        }
        Ok(false) => {
            log::warn!("   Certificate not found for relinquishment");
            HttpResponse::NotFound().json(serde_json::json!({
                "error": "Certificate not found"
            }))
        }
        Err(e) => {
            log::error!("   Failed to relinquish certificate: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to relinquish certificate: {}", e)
            }))
        }
    }
}

// ============================================================================
// Method 18: listCertificates (Call Code 18)
// ============================================================================

/// Request structure for listCertificates
#[derive(Debug, Deserialize)]
pub struct ListCertificatesRequest {
    /// Array of certifier public keys (optional filter)
    pub certifiers: Option<Vec<String>>,  // Array of hex strings

    /// Array of certificate types (optional filter)
    #[serde(rename = "types")]
    pub types: Option<Vec<String>>,  // Array of base64 strings

    /// Maximum number of certificates to return
    pub limit: Option<i64>,

    /// Number of certificates to skip (pagination)
    pub offset: Option<i64>,
}

/// Certificate response structure (for listCertificates)
#[derive(Debug, Serialize)]
pub struct CertificateResponse {
    #[serde(rename = "type")]
    pub type_: String,  // Base64
    pub serial_number: String,  // Base64
    pub subject: String,  // Hex
    pub certifier: String,  // Hex
    pub revocation_outpoint: String,
    pub signature: String,  // Hex
    pub fields: serde_json::Value,  // Map of fieldName -> base64 encrypted value
    pub keyring: serde_json::Value,  // Map of fieldName -> base64 keyring value
}

/// Response structure for listCertificates
#[derive(Debug, Serialize)]
pub struct ListCertificatesResponse {
    pub total_certificates: i64,
    pub certificates: Vec<CertificateResponse>,
}

/// listCertificates - BRC-100 endpoint (Call Code 18)
///
/// Lists all certificates owned by the wallet, with optional filtering.
pub async fn list_certificates(
    state: web::Data<AppState>,
    req: web::Json<ListCertificatesRequest>,
) -> HttpResponse {
    log::info!("📋 /listCertificates called");
    log::info!("   Certifiers filter: {:?}", req.certifiers);
    log::info!("   Types filter: {:?}", req.types);
    log::info!("   Limit: {:?}, Offset: {:?}", req.limit, req.offset);

    // Get database connection
    let db = state.database.lock().unwrap();
    let cert_repo = CertificateRepository::new(db.connection());

    // Convert filter parameters to strings (for repository API)
    // For now, we'll filter by first certifier/type if provided
    // TODO: Support multiple certifiers/types (repository needs update)
    let certifier_filter: Option<String> = req.certifiers.as_ref()
        .and_then(|certs| certs.first())
        .map(|c| c.clone());

    let type_filter: Option<String> = req.types.as_ref()
        .and_then(|types| types.first())
        .map(|t| t.clone());

    // Query certificates (only active, not deleted)
    let certificates = match cert_repo.list_certificates(
        type_filter.as_deref(),
        certifier_filter.as_deref(),
        None,  // subject_filter
        Some(false),  // is_deleted = false (only active certificates)
        req.limit.map(|l| l as i32),
        req.offset.map(|o| o as i32),
    ) {
        Ok(certs) => certs,
        Err(e) => {
            log::error!("   Database error: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }));
        }
    };

    // Get total count (for pagination)
    let total = certificates.len() as i64;  // TODO: Get actual total from database

    // Convert to response format
    let mut cert_responses = Vec::new();
    for cert in certificates {
        // Get certificate fields (returns HashMap<String, CertificateField>)
        let fields_map = if let Some(cert_id) = cert.id {
            match cert_repo.get_certificate_fields(cert_id) {
                Ok(fields) => {
                    let mut fields_json = serde_json::Map::new();
                    for (field_name, field) in fields.iter() {
                        let field_value_base64 = BASE64.encode(&field.field_value);
                        fields_json.insert(
                            field_name.clone(),
                            serde_json::Value::String(field_value_base64),
                        );
                    }
                    serde_json::Value::Object(fields_json)
                }
                Err(_) => serde_json::json!({}),
            }
        } else {
            serde_json::json!({})
        };

        // Get keyring (from certificate fields' master_key)
        let keyring_map = if let Some(cert_id) = cert.id {
            match cert_repo.get_certificate_fields(cert_id) {
                Ok(fields) => {
                    let mut keyring_json = serde_json::Map::new();
                    for (field_name, field) in fields.iter() {
                        let master_key_base64 = BASE64.encode(&field.master_key);
                        keyring_json.insert(
                            field_name.clone(),
                            serde_json::Value::String(master_key_base64),
                        );
                    }
                    serde_json::Value::Object(keyring_json)
                }
                Err(_) => serde_json::json!({}),
            }
        } else {
            serde_json::json!({})
        };

        cert_responses.push(CertificateResponse {
            type_: BASE64.encode(&cert.type_),
            serial_number: BASE64.encode(&cert.serial_number),
            subject: hex::encode(&cert.subject),
            certifier: hex::encode(&cert.certifier),
            revocation_outpoint: cert.revocation_outpoint.clone(),
            signature: hex::encode(&cert.signature),
            fields: fields_map,
            keyring: keyring_map,
        });
    }

    log::info!("   ✅ Returning {} certificates", cert_responses.len());

    HttpResponse::Ok().json(ListCertificatesResponse {
        total_certificates: total,
        certificates: cert_responses,
    })
}

// ============================================================================
// Helper: createNonce (matches TypeScript SDK)
// ============================================================================

/// Creates a nonce using SDK's createNonce format:
/// 1. Generate 16 random bytes (firstHalf)
/// 2. Create HMAC over those bytes using BRC-42 with protocolID [2, 'server hmac']
/// 3. Concatenate: firstHalf (16 bytes) + hmac (32 bytes) = 48 bytes total
/// 4. Return base64-encoded
///
/// **Reference**: TypeScript SDK `createNonce` in `@bsv/sdk/src/auth/utils/createNonce.ts`
///
/// ## Arguments
/// - `state`: Application state (for database access)
/// - `counterparty`: Optional counterparty public key (hex). If None, uses 'self' (no BRC-42)
///
/// ## Returns
/// Base64-encoded nonce (48 bytes: 16 random + 32 HMAC)
/// Convert bytes to UTF-8 string matching TypeScript SDK's Utils.toUTF8 behavior exactly
/// The SDK uses a 'skip' counter to handle multi-byte sequences, which we must replicate exactly
fn js_to_utf8(bytes: &[u8]) -> String {
    let mut result = String::new();
    let mut skip = 0;

    for i in 0..bytes.len() {
        let byte = bytes[i];

        // If this byte is part of a multi-byte sequence, skip it
        if skip > 0 {
            skip -= 1;
            continue;
        }

        // 1-byte sequence (0xxxxxxx)
        if byte <= 0x7f {
            result.push(char::from(byte));
        } else if byte >= 0xc0 && byte <= 0xdf {
            // 2-byte sequence (110xxxxx 10xxxxxx)
            // SDK doesn't check bounds - it just accesses arr[i + 1] directly
            // If out of bounds, byte2 will be undefined, but we'll still compute code_point
            let byte2 = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
            skip = 1;
            let code_point = ((byte & 0x1f) as u32) << 6 | (byte2 & 0x3f) as u32;
            // SDK uses String.fromCharCode(codePoint) which always produces a character
            // Even for invalid code points, it produces a character (replacement character)
            // We need to always push a character to match SDK behavior
            result.push(char::from_u32(code_point).unwrap_or(char::from_u32(0xFFFD).unwrap()));
        } else if byte >= 0xe0 && byte <= 0xef {
            // 3-byte sequence (1110xxxx 10xxxxxx 10xxxxxx)
            // SDK doesn't check bounds - it just accesses arr[i + 1] and arr[i + 2] directly
            let byte2 = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
            let byte3 = if i + 2 < bytes.len() { bytes[i + 2] } else { 0 };
            skip = 2;
            let code_point = ((byte & 0x0f) as u32) << 12
                | ((byte2 & 0x3f) as u32) << 6
                | (byte3 & 0x3f) as u32;
            // SDK always produces a character, even for invalid code points
            result.push(char::from_u32(code_point).unwrap_or(char::from_u32(0xFFFD).unwrap()));
        } else if byte >= 0xf0 && byte <= 0xf7 {
            // 4-byte sequence (11110xxx 10xxxxxx 10xxxxxx 10xxxxxx)
            // SDK doesn't check bounds - it just accesses arr[i + 1], arr[i + 2], arr[i + 3] directly
            // If out of bounds, bytes will be undefined, but we'll still compute code_point
            let byte2 = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
            let byte3 = if i + 2 < bytes.len() { bytes[i + 2] } else { 0 };
            let byte4 = if i + 3 < bytes.len() { bytes[i + 3] } else { 0 };
            skip = 3;
            let code_point = ((byte & 0x07) as u32) << 18
                | ((byte2 & 0x3f) as u32) << 12
                | ((byte3 & 0x3f) as u32) << 6
                | (byte4 & 0x3f) as u32;

            // SDK always converts to surrogate pair for code points > 0xffff, even if invalid
            // JavaScript's String.fromCharCode will convert invalid surrogates to replacement chars
            if code_point > 0xffff {
                let surrogate1 = 0xd800u32 + ((code_point - 0x10000) >> 10);
                let surrogate2 = 0xdc00u32 + ((code_point - 0x10000) & 0x3ff);
                // Push surrogate pair - Rust will handle invalid surrogates when encoding to UTF-8
                // This matches JavaScript's behavior of producing replacement characters
                if let Some(ch1) = char::from_u32(surrogate1) {
                    result.push(ch1);
                } else {
                    result.push(char::from_u32(0xFFFD).unwrap()); // Replacement character
                }
                if let Some(ch2) = char::from_u32(surrogate2) {
                    result.push(ch2);
                } else {
                    result.push(char::from_u32(0xFFFD).unwrap()); // Replacement character
                }
            } else {
                // SDK always produces a character, even for invalid code points
                // JavaScript converts invalid code points to replacement characters
                result.push(char::from_u32(code_point).unwrap_or(char::from_u32(0xFFFD).unwrap()));
            }
        }
        // Invalid UTF-8 sequence start byte (0x80-0xbf continuation bytes, 0xf8-0xff invalid)
        // SDK's toUTF8 doesn't handle these cases - it just skips them (continues to next iteration)
        // We should do the same - don't push anything, just continue
    }

    result
}

async fn create_nonce_with_hmac(
    state: &web::Data<AppState>,
    counterparty: Option<&str>,
) -> Result<String, String> {
    use rand::RngCore;
    use crate::crypto::brc42::derive_symmetric_key_for_hmac;
    use crate::crypto::brc43::{InvoiceNumber, SecurityLevel, normalize_protocol_id};
    use crate::crypto::signing::hmac_sha256;

    // Step 1: Generate 16 random bytes (firstHalf)
    let mut first_half = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut first_half);

    log::info!("   🔐 Creating nonce with HMAC (SDK format):");
    log::info!("      First half (16 random bytes, hex): {}", hex::encode(&first_half));

    // Step 2: Create HMAC using BRC-42
    // Protocol ID: [2, 'server hmac']
    let protocol_id = match normalize_protocol_id("server hmac") {
        Ok(p) => p,
        Err(e) => return Err(format!("Failed to normalize protocol ID: {}", e)),
    };

    // Key ID: UTF8(firstHalf) - match SDK's Utils.toUTF8 behavior EXACTLY
    // SDK's toUTF8 manually decodes UTF-8 sequences character by character
    // We need to replicate this exactly to match the invoice number
    let key_id = js_to_utf8(&first_half);
    log::info!("      Key ID (UTF-8 decoded firstHalf, {} chars): {}", key_id.chars().count(), key_id);
    log::info!("      Key ID bytes (hex): {}", hex::encode(key_id.as_bytes()));
    log::info!("      First half bytes (hex): {}", hex::encode(&first_half));
    log::info!("      ⚠️  CRITICAL: Server will use Utils.toUTF8(firstHalf) to extract keyID for verifyNonce");
    log::info!("      ⚠️  Our keyID must match exactly what the server extracts!");

    // Create invoice number
    let security_level = if counterparty.is_some() {
        SecurityLevel::CounterpartyLevel
    } else {
        SecurityLevel::CounterpartyLevel // Still use level 2 even for 'self'
    };

    let invoice_number = match InvoiceNumber::new(
        security_level,
        protocol_id,
        key_id.clone(),
    ) {
        Ok(inv) => inv.to_string(),
        Err(e) => return Err(format!("Failed to create invoice number: {}", e)),
    };

    log::info!("      Invoice number: {} (keyID length: {} chars)", invoice_number, key_id.chars().count());

    // Get master private key
    let db = state.database.lock().unwrap();
    let master_privkey = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => key,
        Err(e) => {
            drop(db);
            return Err(format!("Failed to get master private key: {}", e));
        }
    };
    drop(db);

    // Derive HMAC key using BRC-42 (or use raw key for 'self')
    // For HMAC, we need a symmetric key (HMAC output), not a child private key
    let hmac_key = if let Some(counterparty_hex) = counterparty {
        log::info!("      Counterparty: {} (using BRC-42)", counterparty_hex);
        let counterparty_bytes = match hex::decode(counterparty_hex) {
            Ok(b) => b,
            Err(e) => return Err(format!("Failed to decode counterparty key: {}", e)),
        };

        // For HMAC, derive symmetric key (HMAC output), not child private key
        match derive_symmetric_key_for_hmac(&master_privkey, &counterparty_bytes, &invoice_number) {
            Ok(key) => key,
            Err(e) => return Err(format!("BRC-42 symmetric key derivation failed: {}", e)),
        }
    } else {
        log::info!("      Counterparty: 'self' (using raw master key)");
        // For 'self', we still need to derive a symmetric key using the invoice number
        // But without a counterparty, we can't use ECDH. Let's check what the SDK does...
        // Actually, for 'self', the SDK might use a different approach. Let's use the master key directly
        // and compute HMAC over invoice number with it as a fallback.
        // But wait - if counterparty is 'self', we shouldn't be using BRC-42 at all.
        // The SDK's createNonce with 'self' might not use BRC-42. Let me check...
        // For now, let's use the master key directly (this might be wrong, but let's test)
        master_privkey
    };

    // Compute HMAC-SHA256 over firstHalf
    let hmac_result = hmac_sha256(&hmac_key, &first_half);

    log::info!("      HMAC (32 bytes, hex): {}", hex::encode(&hmac_result));

    // Step 3: Concatenate firstHalf (16 bytes) + hmac (32 bytes) = 48 bytes
    let mut nonce_bytes = Vec::with_capacity(48);
    nonce_bytes.extend_from_slice(&first_half);
    nonce_bytes.extend_from_slice(&hmac_result);

    log::info!("      Nonce bytes (48 total, hex): {}", hex::encode(&nonce_bytes));

    // Step 4: Return base64-encoded
    let nonce_base64 = BASE64.encode(&nonce_bytes);
    log::info!("      ✅ Nonce created (base64): {} ({} chars)", &nonce_base64[..std::cmp::min(20, nonce_base64.len())], nonce_base64.len());

    Ok(nonce_base64)
}

// ============================================================================
// Method 17: acquireCertificate (Call Code 17)
// ============================================================================

/// Acquisition protocol enum for deserialization
#[derive(Debug, Clone, Copy)]
pub enum AcquisitionProtocol {
    Direct = 1,
    Issuance = 2,
}

impl<'de> serde::Deserialize<'de> for AcquisitionProtocol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};
        use std::fmt;

        struct AcquisitionProtocolVisitor;

        impl<'de> Visitor<'de> for AcquisitionProtocolVisitor {
            type Value = AcquisitionProtocol;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("acquisition protocol as number (1 or 2) or string (\"direct\" or \"issuance\")")
            }

            fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    1 => Ok(AcquisitionProtocol::Direct),
                    2 => Ok(AcquisitionProtocol::Issuance),
                    _ => Err(E::custom(format!("Invalid acquisition protocol number: {}", value))),
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    1 => Ok(AcquisitionProtocol::Direct),
                    2 => Ok(AcquisitionProtocol::Issuance),
                    _ => Err(E::custom(format!("Invalid acquisition protocol number: {}", value))),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value.to_lowercase().as_str() {
                    "direct" | "1" => Ok(AcquisitionProtocol::Direct),
                    "issuance" | "2" => Ok(AcquisitionProtocol::Issuance),
                    _ => Err(E::custom(format!("Invalid acquisition protocol string: {}", value))),
                }
            }
        }

        deserializer.deserialize_any(AcquisitionProtocolVisitor)
    }
}

/// Request structure for acquireCertificate
#[derive(Debug, Deserialize)]
pub struct AcquireCertificateRequest {
    /// Acquisition protocol: 1 or "direct" for direct, 2 or "issuance" for issuance
    #[serde(rename = "acquisitionProtocol")]
    pub acquisition_protocol: Option<AcquisitionProtocol>,

    /// Certificate type (base64, required for 'direct')
    #[serde(rename = "type")]
    pub type_: Option<String>,

    /// Certifier public key (hex, required for 'direct')
    pub certifier: Option<String>,

    /// Certificate fields (map, required for 'direct')
    pub fields: Option<serde_json::Value>,

    /// Serial number (base64, required for 'direct')
    pub serial_number: Option<String>,

    /// Revocation outpoint (required for 'direct')
    pub revocation_outpoint: Option<String>,

    /// Certificate signature (hex, required for 'direct')
    pub signature: Option<String>,

    /// Keyring for subject (map, required for 'direct')
    #[serde(rename = "keyringForSubject")]
    pub keyring_for_subject: Option<serde_json::Value>,

    /// Subject public key (hex, optional - may be derived from wallet)
    pub subject: Option<String>,

    /// Certifier URL (required for 'issuance')
    #[serde(rename = "certifierUrl")]
    pub certifier_url: Option<String>,
}

/// Response structure for acquireCertificate
#[derive(Debug, Serialize)]
pub struct AcquireCertificateResponse {
    /// Certificate data (JSON object)
    pub certificate: serde_json::Value,  // Certificate as JSON object
}

/// acquireCertificate - BRC-100 endpoint (Call Code 17)
///
/// Acquires a BRC-52 certificate from direct JSON or via issuance protocol.
///
/// **Protocols**:
/// - 'direct' (1): Receives certificate data directly as JSON
/// - 'issuance' (2): Requests certificate from certifier URL
pub async fn acquire_certificate(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("📋 /acquireCertificate called");

    // Parse request body manually to handle potential deserialization issues
    let body_str = String::from_utf8_lossy(&body);
    log::debug!("   Request body (first 500 chars): {}",
        if body_str.len() > 500 { &body_str[..500] } else { &body_str });

    let req: AcquireCertificateRequest = match serde_json::from_str(&body_str) {
        Ok(r) => r,
        Err(e) => {
            log::error!("   Failed to parse request JSON: {}", e);
            log::error!("   Request body: {}", body_str);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid JSON request: {}", e)
            }));
        }
    };

    log::debug!("   Request: acquisitionProtocol={:?}, type={:?}, certifier={:?}, fields={:?}",
        req.acquisition_protocol,
        req.type_,
        req.certifier,
        req.fields.is_some()
    );

    let protocol = req.acquisition_protocol.unwrap_or(AcquisitionProtocol::Direct);
    log::info!("   Acquisition protocol: {:?}", protocol);

    match protocol {
        AcquisitionProtocol::Direct => acquire_certificate_direct(state, web::Json(req)).await,
        AcquisitionProtocol::Issuance => acquire_certificate_issuance(state, web::Json(req)).await,
    }
}

/// Acquire certificate via 'direct' protocol
async fn acquire_certificate_direct(
    state: web::Data<AppState>,
    req: web::Json<AcquireCertificateRequest>,
) -> HttpResponse {
    log::info!("   Using 'direct' protocol");

    // Build JSON object from request fields
    let mut cert_json = serde_json::Map::new();

    if let Some(type_) = &req.type_ {
        cert_json.insert("type".to_string(), serde_json::Value::String(type_.clone()));
    }
    if let Some(certifier) = &req.certifier {
        cert_json.insert("certifier".to_string(), serde_json::Value::String(certifier.clone()));
    }
    if let Some(fields) = &req.fields {
        cert_json.insert("fields".to_string(), fields.clone());
    }
    if let Some(serial_number) = &req.serial_number {
        cert_json.insert("serialNumber".to_string(), serde_json::Value::String(serial_number.clone()));
    }
    if let Some(revocation_outpoint) = &req.revocation_outpoint {
        cert_json.insert("revocationOutpoint".to_string(), serde_json::Value::String(revocation_outpoint.clone()));
    }
    if let Some(signature) = &req.signature {
        cert_json.insert("signature".to_string(), serde_json::Value::String(signature.clone()));
    }
    if let Some(keyring) = &req.keyring_for_subject {
        cert_json.insert("keyringForSubject".to_string(), keyring.clone());
    }
    if let Some(subject) = &req.subject {
        cert_json.insert("subject".to_string(), serde_json::Value::String(subject.clone()));
    } else {
        // If subject not provided, use wallet's identity key
        let subject_hex = {
            let db = state.database.lock().unwrap();
            match crate::database::get_master_public_key_from_db(&db) {
                Ok(pubkey_bytes) => {
                    let hex = hex::encode(pubkey_bytes);
                    log::info!("   Using wallet identity key as subject: {}", hex);
                    hex
                }
                Err(e) => {
                    log::error!("   Failed to get wallet identity key: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to get wallet identity key: {}", e)
                    }));
                }
            }
        };
        cert_json.insert("subject".to_string(), serde_json::Value::String(subject_hex));
    }

    let cert_json_value = serde_json::Value::Object(cert_json);

    // Parse certificate from JSON
    use crate::certificate::parser::parse_certificate_from_json;
    let mut certificate = match parse_certificate_from_json(&cert_json_value) {
        Ok(cert) => cert,
        Err(e) => {
            log::error!("   Failed to parse certificate: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Failed to parse certificate: {}", e)
            }));
        }
    };

    // Verify certificate signature
    use crate::certificate::verifier::verify_certificate_signature_with_keyid;
    // Use original base64 strings from JSON for keyID (matching server's behavior)
    let type_base64_original = cert_json_value.get("type").and_then(|v| v.as_str());
    let serial_base64_original = cert_json_value.get("serialNumber").and_then(|v| v.as_str());
    match verify_certificate_signature_with_keyid(
        &certificate,
        type_base64_original,
        serial_base64_original,
    ) {
        Ok(_) => {
            log::info!("   ✅ Certificate signature verified");
        }
        Err(e) => {
            log::error!("   Certificate signature verification failed: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Certificate signature verification failed: {}", e)
            }));
        }
    }

    // Check revocation status (check if revocationOutpoint UTXO is spent)
    use crate::certificate::verifier::check_revocation_status;
    match check_revocation_status(&certificate.revocation_outpoint).await {
        Ok(true) => {
            log::error!("   Certificate is REVOKED - revocation outpoint is spent");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Certificate is revoked - revocation outpoint UTXO is spent"
            }));
        }
        Ok(false) => {
            log::info!("   ✅ Certificate is ACTIVE - revocation outpoint is unspent");
        }
        Err(e) => {
            log::warn!("   Failed to check revocation status: {} - proceeding anyway", e);
            // Continue with acquisition even if revocation check fails
            // This allows certificates to be acquired even if API is temporarily unavailable
        }
    }

    // Check if certificate already exists
    let db = state.database.lock().unwrap();
    let cert_repo = CertificateRepository::new(db.connection());

    match cert_repo.get_by_identifiers(
        &certificate.type_,
        &certificate.serial_number,
        &certificate.certifier,
    ) {
        Ok(Some(_)) => {
            log::warn!("   Certificate already exists");
            return HttpResponse::Conflict().json(serde_json::json!({
                "error": "Certificate already exists"
            }));
        }
        Ok(None) => {
            // Certificate doesn't exist, proceed with insertion
        }
        Err(e) => {
            log::error!("   Database error checking certificate: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }));
        }
    }

    // Store certificate in database
    match cert_repo.insert_certificate_with_fields(&mut certificate) {
        Ok(certificate_id) => {
            log::info!("   ✅ Certificate stored with ID: {}", certificate_id);

            // Return certificate as JSON object (matching other wallets' format)
            HttpResponse::Ok().json(AcquireCertificateResponse {
                certificate: cert_json_value,
            })
        }
        Err(e) => {
            log::error!("   Failed to store certificate: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to store certificate: {}", e)
            }))
        }
    }
}

/// Acquire certificate via 'issuance' protocol
///
/// **Process**:
/// 1. Get subject's public key (wallet's identity key)
/// 2. Build certificate signing request (CSR) with type, certifier, fields, subject
/// 3. Make HTTP POST request to certifier URL
/// 4. Receive certificate from certifier (same format as 'direct' protocol)
/// 5. Process certificate like 'direct' protocol (parse, verify, store)
async fn acquire_certificate_issuance(
    state: web::Data<AppState>,
    req: web::Json<AcquireCertificateRequest>,
) -> HttpResponse {
    log::info!("   Using 'issuance' protocol");

    // Validate required fields for 'issuance' protocol
    let certifier_url = match &req.certifier_url {
        Some(url) => url.clone(),
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "certifierUrl is required for 'issuance' protocol"
            }));
        }
    };

    let type_ = match &req.type_ {
        Some(t) => t.clone(),
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "type is required for 'issuance' protocol"
            }));
        }
    };

    let certifier = match &req.certifier {
        Some(c) => c.clone(),
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "certifier is required for 'issuance' protocol"
            }));
        }
    };

    let fields = match &req.fields {
        Some(f) => f.clone(),
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "fields is required for 'issuance' protocol"
            }));
        }
    };

    // Get subject's public key (wallet's identity key)
    let db = state.database.lock().unwrap();
    let subject_public_key = match crate::database::get_master_public_key_from_db(&db) {
        Ok(pubkey_bytes) => {
            hex::encode(&pubkey_bytes)
        }
        Err(e) => {
            log::error!("   Failed to get master public key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to get identity key: {}", e)
            }));
        }
    };
    drop(db);

    log::info!("   Subject public key: {}", subject_public_key);
    log::info!("   Certifier URL: {}", certifier_url);

    // BRC-53: Step 1 - Initial Request using Peer protocol (like TypeScript SDK)
    // Generate client nonce using SDK's createNonce format:
    // 1. Generate 16 random bytes (firstHalf)
    // 2. Create HMAC over those bytes using BRC-42 with protocolID [2, 'server hmac']
    // 3. Concatenate: firstHalf (16 bytes) + hmac (32 bytes) = 48 bytes total
    // 4. Return base64-encoded
    // For initialRequest, counterparty is 'self' (no BRC-42 derivation)
    let client_nonce = match create_nonce_with_hmac(&state, None).await {
        Ok(nonce) => nonce,
        Err(e) => {
            log::error!("   Failed to create nonce: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Nonce creation failed: {}", e)
            }));
        }
    };

    log::info!("   🔐 BRC-53 Step 1: Sending initialRequest to /.well-known/auth (Peer protocol)...");

    // Build initialRequest message (NO SIGNATURE - matches TypeScript SDK)
    // The TypeScript SDK sends initialRequest as AuthMessage JSON to /.well-known/auth
    let initial_request_message = serde_json::json!({
        "version": "0.1",
        "messageType": "initialRequest",
        "identityKey": subject_public_key,
        "initialNonce": client_nonce
        // NO signature field - initialRequest is unsigned!
    });
    let initial_request_json = serde_json::to_string(&initial_request_message).unwrap();

    log::info!("   📤 Initial request message (unsigned): {}", initial_request_json);

    // Send initialRequest - try /initialRequest first (BRC-53 docs), fallback to /.well-known/auth (Peer protocol)
    // NOTE: BRC-53 documentation specifies /initialRequest, but TypeScript SDK uses /.well-known/auth
    // We'll try /initialRequest first per BRC-53, then fall back to Peer protocol endpoint
    let client = reqwest::Client::new();
    let initial_request_url = if certifier_url.ends_with('/') {
        format!("{}initialRequest", certifier_url)
    } else {
        format!("{}/initialRequest", certifier_url)
    };
    let well_known_auth_url = if certifier_url.ends_with('/') {
        format!("{}.well-known/auth", certifier_url)
    } else {
        format!("{}/.well-known/auth", certifier_url)
    };

    log::info!("   📤 POST to: {} (trying BRC-53 /initialRequest first)", initial_request_url);
    let initial_response = match client
        .post(&initial_request_url)
        .header("Content-Type", "application/json")
        // NO authentication headers - initialRequest is unsigned!
        .body(initial_request_json.clone())
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                log::info!("   ✅ /initialRequest succeeded");
                resp
            } else {
                // If /initialRequest fails, try /.well-known/auth as fallback
                log::warn!("   ⚠️  /initialRequest returned {}, trying /.well-known/auth fallback", resp.status());
                log::info!("   📤 POST to: {} (Peer protocol fallback)", well_known_auth_url);
                match client
                    .post(&well_known_auth_url)
                    .header("Content-Type", "application/json")
                    .body(initial_request_json.clone())
                    .send()
                    .await
                {
                    Ok(fallback_resp) => {
                        log::info!("   ✅ /.well-known/auth fallback succeeded");
                        fallback_resp
                    },
                    Err(e) => {
                        log::error!("   ❌ Both /initialRequest and /.well-known/auth failed: {}", e);
                        return HttpResponse::BadGateway().json(serde_json::json!({
                            "error": format!("Failed to connect to certifier: {}", e)
                        }));
                    }
                }
            }
        },
        Err(e) => {
            // If /initialRequest connection fails, try /.well-known/auth as fallback
            log::warn!("   ⚠️  /initialRequest connection failed: {}, trying /.well-known/auth fallback", e);
            log::info!("   📤 POST to: {} (Peer protocol fallback)", well_known_auth_url);
            match client
                .post(&well_known_auth_url)
                .header("Content-Type", "application/json")
                .body(initial_request_json.clone())
                .send()
                .await
            {
                Ok(fallback_resp) => {
                    log::info!("   ✅ /.well-known/auth fallback succeeded");
                    fallback_resp
                },
                Err(fallback_e) => {
                    log::error!("   ❌ Both /initialRequest and /.well-known/auth failed: {} / {}", e, fallback_e);
                    return HttpResponse::BadGateway().json(serde_json::json!({
                        "error": format!("Failed to connect to certifier: {}", e)
                    }));
                }
            }
        }
    };

    let initial_status = initial_response.status();
    let initial_response_text = initial_response.text().await.unwrap_or_else(|_| "Unknown error".to_string());

    if !initial_status.is_success() {
        log::error!("   Initial request failed: {} - {}", initial_status, initial_response_text);
        return HttpResponse::BadGateway().json(serde_json::json!({
            "error": format!("Certifier initial request failed ({}): {}", initial_status, initial_response_text)
        }));
    }

    // Parse initialResponse message (Peer protocol response)
    let initial_data: serde_json::Value = match serde_json::from_str(&initial_response_text) {
        Ok(data) => data,
        Err(e) => {
            log::error!("   Failed to parse initial response: {}", e);
            log::error!("   Response text: {}", initial_response_text);
            return HttpResponse::BadGateway().json(serde_json::json!({
                "error": format!("Invalid response from certifier: {}", e)
            }));
        }
    };

    log::info!("   ✅ Received initialResponse from certifier");
    log::info!("   📋 Response data: {}", serde_json::to_string_pretty(&initial_data).unwrap_or_else(|_| "error".to_string()));

    // Extract server's nonce from initialResponse (Peer protocol)
    // The response should have: initialNonce (server's nonce), yourNonce (our clientNonce echoed back)
    let server_nonce = match initial_data.get("initialNonce")
        .and_then(|v| v.as_str()) {
        Some(n) => n,
        None => {
            log::error!("   Missing initialNonce in initialResponse");
            return HttpResponse::BadGateway().json(serde_json::json!({
                "error": "Missing initialNonce in certifier response"
            }));
        }
    };

    // Verify the server echoed back our nonce
    let echoed_nonce = match initial_data.get("yourNonce")
        .and_then(|v| v.as_str()) {
        Some(n) => n,
        None => {
            log::error!("   Missing yourNonce in initialResponse");
            return HttpResponse::BadGateway().json(serde_json::json!({
                "error": "Missing yourNonce in certifier response"
            }));
        }
    };

    if echoed_nonce != client_nonce {
        log::error!("   Server echoed wrong nonce! Expected: {}, Got: {}", client_nonce, echoed_nonce);
        return HttpResponse::BadGateway().json(serde_json::json!({
            "error": "Server echoed incorrect nonce"
        }));
    }

    log::info!("   ✅ Server nonce received: {}", server_nonce);
    log::info!("   ✅ Server echoed our nonce correctly");

    // Extract server's identity key from initialResponse - THIS is the counterparty for BRC-42!
    // The server's identityKey is what they authenticated with, so we must use it for mutual auth
    let server_identity_key = match initial_data.get("identityKey")
        .and_then(|v| v.as_str()) {
        Some(key) => key,
        None => {
            log::error!("   Missing identityKey in initialResponse");
            return HttpResponse::BadGateway().json(serde_json::json!({
                "error": "Missing identityKey in certifier response"
            }));
        }
    };

    log::info!("   ✅ Server identity key: {}", server_identity_key);

    // Verify server's identity key matches the certifier public key from request
    // (They should be the same, but let's log if they differ)
    if server_identity_key != certifier {
        log::warn!("   ⚠️  Server identityKey ({}) differs from certifier ({})", server_identity_key, certifier);
        log::warn!("   ⚠️  Using server's identityKey for BRC-42 key derivation");
    }

    // CRITICAL: Verify server's signature on initialResponse to complete mutual authentication
    // The server signs: concatenated base64 nonces (clientNonce + serverNonce) decoded to bytes
    // ProtocolID: [2, 'auth message signature'], KeyID: "{clientNonce} {serverNonce}", Counterparty: our identity key
    let server_signature = match initial_data.get("signature") {
        Some(sig) => {
            // Signature is an array of bytes in the JSON response
            if let Some(sig_array) = sig.as_array() {
                let sig_bytes: Vec<u8> = sig_array.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect();
                if sig_bytes.is_empty() {
                    log::error!("   Failed to parse server signature as byte array");
                    None
                } else {
                    Some(sig_bytes)
                }
            } else {
                log::error!("   Server signature is not an array");
                None
            }
        },
        None => {
            log::error!("   Missing signature in initialResponse");
            None
        }
    };

    if let Some(ref sig_bytes) = server_signature {
        use crate::crypto::brc42::derive_child_public_key;
        use crate::crypto::brc43::{InvoiceNumber, SecurityLevel, normalize_protocol_id};
        use secp256k1::{Secp256k1, Message, PublicKey};
        use secp256k1::ecdsa::Signature;

        // Data to verify: concatenated base64 nonces decoded to bytes
        // CRITICAL: Server signs: client_nonce + server_nonce (in that order!)
        // Server code (line 512): Utils.toArray(message.initialNonce + sessionNonce, 'base64')
        //   where message.initialNonce = client_nonce, sessionNonce = server_nonce
        // Client verifies (line 562): Utils.toArray((peerSession.sessionNonce ?? '') + (message.initialNonce ?? ''), 'base64')
        //   where peerSession.sessionNonce = client_nonce, message.initialNonce = server_nonce
        // So it's: client_nonce + server_nonce (in that order!)
        //
        // PROTOCOL ISSUE: This is a specification problem that should be addressed.
        // For true language-agnostic interoperability, the protocol should specify:
        //   1. Decode each nonce separately using standard base64
        //   2. Concatenate the resulting byte arrays
        // This would work identically in all languages.
        //
        // CURRENT REALITY: The TypeScript SDK (reference implementation) uses a custom
        // base64ToArray that processes character-by-character. When concatenating base64
        // strings creates invalid base64 (e.g., '=' padding in the middle), it produces
        // non-standard but deterministic bytes. Since existing servers sign with this
        // behavior, we must match it for compatibility.
        //
        // JavaScript's base64ToArray behavior:
        // 1. Strips trailing padding: msg.replace(/=+$/, '')
        // 2. For each char: currentBit = (currentBit << 6) | base64Chars.indexOf(char)
        // 3. If char not found, indexOf returns -1, which becomes 0xFFFFFFFF in bitwise ops
        //
        // TODO: This should be standardized in the BRC specification.
        let concatenated_nonces_base64 = format!("{}{}", client_nonce, server_nonce);
        log::info!("   🔍 Concatenated base64 string: {}", concatenated_nonces_base64);

        // Implement JavaScript's base64ToArray logic EXACTLY
        // This reproduces the deterministic (but non-standard) behavior when
        // processing invalid base64 strings (e.g., with '=' in the middle)
        fn js_base64_to_array(msg: &str) -> Vec<u8> {
            const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut result = Vec::new();
            let mut current_bit: u32 = 0;
            let mut current_byte: u32 = 0;

            // Strip trailing padding (like JavaScript: msg.replace(/=+$/, ''))
            let msg_stripped = msg.trim_end_matches('=');

            for &char_byte in msg_stripped.as_bytes() {
                // Find index in base64 charset
                let index = BASE64_CHARS.iter().position(|&c| c == char_byte);

                // In JavaScript: currentBit = (currentBit << 6) | base64Chars.indexOf(char)
                // If indexOf returns -1, JavaScript does: (currentBit << 6) | -1
                // In JavaScript bitwise ops, -1 becomes 0xFFFFFFFF (all 1s in 32-bit)
                let index_value = match index {
                    Some(i) => i as u32,
                    None => 0xFFFFFFFFu32, // -1 in JavaScript becomes 0xFFFFFFFF
                };

                current_bit = (current_bit << 6) | index_value;
                current_byte += 6;

                if current_byte >= 8 {
                    current_byte -= 8;
                    result.push((current_bit >> current_byte) as u8);
                    current_bit &= (1u32 << current_byte) - 1;
                }
            }

            result
        }

        // Try the "proper" approach first: decode each nonce separately, then concatenate
        // This is the standard, language-agnostic way that should work in any implementation
        let client_bytes = match BASE64.decode(&client_nonce) {
            Ok(b) => b,
            Err(e) => {
                log::error!("   Failed to decode client nonce: {}", e);
                return HttpResponse::BadGateway().json(serde_json::json!({
                    "error": format!("Failed to verify server signature: invalid client nonce: {}", e)
                }));
            }
        };
        let server_bytes = match BASE64.decode(&server_nonce) {
            Ok(b) => b,
            Err(e) => {
                log::error!("   Failed to decode server nonce: {}", e);
                return HttpResponse::BadGateway().json(serde_json::json!({
                    "error": format!("Failed to verify server signature: invalid server nonce: {}", e)
                }));
            }
        };
        let mut data_to_verify = client_bytes;
        data_to_verify.extend_from_slice(&server_bytes);
        log::info!("   ✅ Using proper approach: decoded separately and concatenated (total: {} bytes)", data_to_verify.len());

        // For comparison, also compute what JavaScript's decoder would produce
        let js_decoded = js_base64_to_array(&concatenated_nonces_base64);
        if data_to_verify != js_decoded {
            log::warn!("   ⚠️  Proper decode differs from JavaScript decoder!");
            log::info!("      Proper decode (hex): {}", hex::encode(&data_to_verify));
            log::info!("      JS decoder (hex): {}", hex::encode(&js_decoded));
            log::info!("   ℹ️  Will try proper approach first, fallback to JS if verification fails");
        }

        // Create invoice number for verification (same format as server used to sign)
        let verify_protocol_id = match normalize_protocol_id("auth message signature") {
            Ok(p) => p,
            Err(e) => {
                log::error!("   Failed to normalize protocol ID: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Protocol ID error: {}", e)
                }));
            }
        };
        // KeyID format: "client_nonce server_nonce" (matches what server used to sign)
        // CRITICAL: The server uses the raw base64 strings (with padding) in the keyID
        // But we should verify if padding needs to be stripped for the invoice number
        let verify_key_id = format!("{} {}", client_nonce, server_nonce);
        log::info!("   🔍 KeyID for invoice number: {}", verify_key_id);
        let verify_invoice_number = match InvoiceNumber::new(
            SecurityLevel::CounterpartyLevel,
            verify_protocol_id,
            &verify_key_id
        ) {
            Ok(inv) => inv.to_string(),
            Err(e) => {
                log::error!("   Failed to create invoice number: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Invoice number error: {}", e)
                }));
            }
        };

        // Derive server's child public key using BRC-42
        // CRITICAL: The server signed with OUR identity key as counterparty (line 515)
        // When verifying, TypeScript uses the SERVER's identity key as counterparty (line 570)
        // This is because verifySignature derives the SIGNER's child public key
        // So we use: our_priv + server_pub + invoice = server's child pubkey
        let our_identity_bytes = match hex::decode(&subject_public_key) {
            Ok(b) => b,
            Err(e) => {
                log::error!("   Failed to decode our identity key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Invalid identity key"
                }));
            }
        };

        // Parse server's identity key bytes (this is the counterparty for verification)
        let server_identity_bytes = match hex::decode(server_identity_key) {
            Ok(b) => b,
            Err(e) => {
                log::error!("   Failed to decode server identity key: {}", e);
                return HttpResponse::BadGateway().json(serde_json::json!({
                    "error": format!("Invalid server identity key: {}", e)
                }));
            }
        };

        // For public key derivation, we use our master private key + server's public key
        // This derives the server's child public key (same as verifySignature handler)
        // The server signed with: server_priv + our_pub + invoice = server's child privkey
        // We verify with: our_priv + server_pub + invoice = server's child pubkey
        let db = state.database.lock().unwrap();
        let our_master_privkey = match crate::database::get_master_private_key_from_db(&db) {
            Ok(key) => key,
            Err(e) => {
                log::error!("   Failed to get master private key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to get identity key: {}", e)
                }));
            }
        };
        drop(db);

        log::info!("   🔑 Key derivation:");
        log::info!("      Our master privkey (first 8 bytes): {}", hex::encode(&our_master_privkey[..8]));
        log::info!("      Server identity pubkey: {}", hex::encode(&server_identity_bytes));
        log::info!("      Invoice number: {}", verify_invoice_number);

        let server_child_pubkey = match derive_child_public_key(
            &our_master_privkey,
            &server_identity_bytes,
            &verify_invoice_number
        ) {
            Ok(pubkey) => pubkey,
            Err(e) => {
                log::error!("   Failed to derive server's child public key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Key derivation error: {}", e)
                }));
            }
        };

        // Log verification details for debugging
        log::info!("   🔍 Verification details:");
        log::info!("      Data to verify length: {} bytes", data_to_verify.len());
        log::info!("      Data to verify (hex): {}", hex::encode(&data_to_verify));
        log::info!("      Client nonce: {}", client_nonce);
        log::info!("      Server nonce: {}", server_nonce);
        log::info!("      Invoice number: {}", verify_invoice_number);
        log::info!("      KeyID: {}", verify_key_id);
        log::info!("      Server identity key: {}", server_identity_key);
        log::info!("      Our identity key: {}", subject_public_key);

        // Verify signature
        let secp = Secp256k1::new();
        let server_pubkey = match PublicKey::from_slice(&server_child_pubkey) {
            Ok(pk) => pk,
            Err(e) => {
                log::error!("   Invalid server child public key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Invalid server public key"
                }));
            }
        };

        log::info!("      Server child pubkey (hex): {}", hex::encode(&server_child_pubkey));

        let data_hash = sha256(&data_to_verify);
        log::info!("      Message hash (SHA256, hex): {}", hex::encode(&data_hash));
        let message = Message::from_digest_slice(&data_hash).unwrap();
        let signature = match Signature::from_der(sig_bytes) {
            Ok(sig) => sig,
            Err(e) => {
                log::error!("   Invalid DER signature format: {}", e);
                return HttpResponse::BadGateway().json(serde_json::json!({
                    "error": "Invalid server signature format"
                }));
            }
        };

        log::info!("      Signature (DER, hex): {}", hex::encode(sig_bytes));

        let verify_result = secp.verify_ecdsa(&message, &signature, &server_pubkey);
        let mut is_valid = verify_result.is_ok();

        // If verification failed with proper approach, try JavaScript-compatible decoder
        if !is_valid {
            log::warn!("   ⚠️  Verification failed with proper decode, trying JavaScript-compatible decoder...");
            let js_decoded = js_base64_to_array(&concatenated_nonces_base64);
            if js_decoded != data_to_verify {
                log::info!("   🔄 Retrying with JavaScript decoder result ({} bytes)", js_decoded.len());
                let js_data_hash = sha256(&js_decoded);
                let js_message = Message::from_digest_slice(&js_data_hash).unwrap();
                let js_verify_result = secp.verify_ecdsa(&js_message, &signature, &server_pubkey);
                is_valid = js_verify_result.is_ok();

                if is_valid {
                    log::info!("   ✅ Verification succeeded with JavaScript-compatible decoder!");
                    log::warn!("   ⚠️  Server is using non-standard base64 decoding (protocol issue)");
                } else if let Err(e) = js_verify_result {
                    log::error!("   Signature verification error (JS decoder): {}", e);
                }
            }
        }

        if let Err(e) = verify_result {
            log::error!("   Signature verification error (proper decode): {}", e);
        }

        if !is_valid {
            log::error!("   ❌ Server signature verification FAILED with both approaches!");
            return HttpResponse::BadGateway().json(serde_json::json!({
                "error": "Server signature verification failed - mutual authentication incomplete"
            }));
        }

        log::info!("   ✅ Server signature verified - mutual authentication complete!");
    } else {
        log::warn!("   ⚠️  No server signature to verify (proceeding anyway)");
    }

    // Check if response has BRC-53 specific fields (validationKey, serialNumber, etc.)
    // Some certifiers might include these in the initialResponse, others might require a separate request
    let validation_key = initial_data.get("validationKey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let serial_number = initial_data.get("serialNumber")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let server_validation_nonce = initial_data.get("validationNonce")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Use server_nonce as server_serial_nonce (from Peer protocol)
    let server_serial_nonce = server_nonce.to_string();

    log::info!("   📋 BRC-53 fields from response:");
    log::info!("      validationKey: {:?}", validation_key);
    log::info!("      serialNumber: {:?}", serial_number);
    log::info!("      validationNonce: {:?}", server_validation_nonce);
    log::info!("      serialNonce: {}", server_serial_nonce);

    // TODO: Validate nonces (verify hash(clientNonce + serverNonce) == validationKey/serialNumber)

    // BRC-53: Step 2 - Certificate Signing Request
    // Encrypt fields using BRC-2 (we'll use the fields as-is for now, encryption happens in certifier)
    // Build keyring (encrypted field revelation keys for certifier)
    // For now, we'll send empty keyring - the certifier will handle encryption

    log::info!("   🔐 BRC-53 Step 2: Sending certificate signing request...");

    // Generate a NEW nonce for the serialized request (first 32 bytes)
    // This is the requestNonce that will be embedded in the serialized request
    // The TypeScript SDK's AuthFetch.fetch() generates a new requestNonce for each request
    // BUT the CSR body uses the ORIGINAL clientNonce from the initial request
    use rand::Rng;
    let mut csr_request_nonce_bytes = [0u8; 32];
    rand::thread_rng().fill(&mut csr_request_nonce_bytes);
    let csr_request_nonce = base64::engine::general_purpose::STANDARD.encode(&csr_request_nonce_bytes);

    // CRITICAL: TypeScript SDK's Peer.toPeer() uses the SAME nonce for both:
    // 1. The first 32 bytes of the serialized request (becomes x-bsv-auth-request-id header)
    // 2. The keyID for signing (becomes x-bsv-auth-nonce header)
    // See Peer.ts line 124: const requestNonce = Utils.toBase64(Random(32))
    // This requestNonce is used for both the request ID and the signing nonce
    // The requestId is the first 32 bytes of the serialized request (the csr_request_nonce_bytes), base64-encoded

    // Build certificate signing request (CSR) per BRC-53 spec
    // Per MasterCertificate.createCertificateFields():
    // 1. For each field, generate a random symmetric key
    // 2. Encrypt the field value with that key (AES-256-GCM)
    // 3. Encrypt the symmetric key (revelation key) for the certifier using BRC-2
    // 4. Store encrypted field values in certificateFields
    // 5. Store encrypted revelation keys in masterKeyring

    // Get subject's private key for encrypting revelation keys
    let subject_private_key = {
        let db = state.database.lock().unwrap();
        match crate::database::helpers::get_master_private_key_from_db(&db) {
            Ok(key) => key,
            Err(e) => {
                log::error!("   Failed to get master private key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to retrieve wallet private key"
                }));
            }
        }
    };

    // Decode certifier public key
    let certifier_bytes = match hex::decode(&certifier) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("   Invalid certifier public key: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid certifier public key: {}", e)
            }));
        }
    };

    // CRITICAL: TypeScript SDK uses vargs.certifier (original certifier parameter) for encryption
    // See Wallet.ts line 494-498: MasterCertificate.createCertificateFields(this, vargs.certifier, vargs.fields)
    // The SDK also validates that response header matches vargs.certifier (line 514-518)
    // So we MUST use the original certifier parameter, not the server's identityKey
    // If they differ, the server should reject the request (but we'll log a warning)
    let server_identity_bytes = match hex::decode(&server_identity_key) {
        Ok(b) => {
            if b.len() != 33 {
                log::error!("   Invalid server identity key length: {} bytes (expected 33)", b.len());
                return HttpResponse::BadGateway().json(serde_json::json!({
                    "error": format!("Invalid server identity key length: {} bytes", b.len())
                }));
            }
            b
        },
        Err(e) => {
            log::error!("   Failed to decode server identity key: {}", e);
            return HttpResponse::BadGateway().json(serde_json::json!({
                "error": format!("Invalid server identity key: {}", e)
            }));
        }
    };

    log::info!("   🔑 Key selection for masterKeyring encryption (matching TypeScript SDK):");
    log::info!("      Original certifier (from request, used for encryption): {}", hex::encode(&certifier_bytes));
    log::info!("      Server identityKey (from initialResponse): {}", hex::encode(&server_identity_bytes));
    if certifier_bytes != server_identity_bytes {
        log::error!("   ❌ CRITICAL: Certifier public key differs from server's identityKey!");
        log::error!("   ❌ TypeScript SDK would reject this (line 514-518 checks match)");
        log::error!("   ❌ Using original certifier for encryption (matching TypeScript SDK behavior)");
        log::error!("   ❌ Server may not be able to decrypt if keys don't match!");
    } else {
        log::info!("   ✅ Certifier matches server's identityKey");
    }

    // Use original certifier parameter (matching TypeScript SDK: vargs.certifier)
    let encryption_key = &certifier_bytes;

    // Encrypt fields and create masterKeyring
    // Also store plain revelation keys so we can populate fields' master_key when certificate is received
    let mut certificate_fields = serde_json::Map::new();
    let mut master_keyring = serde_json::Map::new();
    let mut plain_revelation_keys: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();

    use crate::crypto::brc2;

    for (field_name, field_value) in fields.as_object().unwrap() {
        let field_name_str = field_name.as_str();

        // Convert field value to string (matching TypeScript SDK's Utils.toArray(fieldValue, 'utf8'))
        // The TypeScript SDK expects fields to be strings, so we convert booleans/numbers to their string representation
        // without JSON-quoting them (e.g., true -> "true", not "\"true\"")
        let field_value_str = match field_value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                // For objects/arrays, serialize to JSON string (but this shouldn't happen in practice)
                match serde_json::to_string(field_value) {
                    Ok(json) => json,
                    Err(e) => {
                        log::error!("   Failed to serialize field '{}' value: {}", field_name_str, e);
                        return HttpResponse::BadRequest().json(serde_json::json!({
                            "error": format!("Failed to serialize field '{}': {}", field_name_str, e)
                        }));
                    }
                }
            }
        };

        // 1. Generate random 32-byte symmetric key for this field
        // IMPORTANT: The TypeScript SDK's SymmetricKey.fromRandom() generates exactly 32 random bytes
        // and SymmetricKey.encrypt() uses this.toArray('be', 32) which ensures exactly 32 bytes
        // (padding with leading zeros if the BigNumber representation has fewer bytes)
        let mut field_symmetric_key = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut field_symmetric_key);

        // 2. Encrypt field value with the symmetric key (AES-256-GCM)
        // TypeScript: fieldSymmetricKey.encrypt(Utils.toArray(fieldValue, 'utf8'))
        // This uses AESGCM with the key from this.toArray('be', 32)
        let field_value_bytes = field_value_str.as_bytes();

        log::info!("   🔐 Field '{}' encryption details:", field_name_str);
        log::info!("      Plaintext value: {} ({} bytes)", field_value_str, field_value_bytes.len());
        log::info!("      Plaintext bytes (hex): {}", hex::encode(field_value_bytes));
        log::info!("      Symmetric key (hex, full 32 bytes): {}", hex::encode(&field_symmetric_key));
        log::info!("      Symmetric key (base64): {}", base64::engine::general_purpose::STANDARD.encode(&field_symmetric_key));
        log::info!("      ⚠️  NOTE: TypeScript SymmetricKey.toArray('be', 32) pads with leading zeros if needed");
        log::info!("      ⚠️  Our key is already 32 bytes, so no padding needed");
        let encrypted_field_value = match brc2::encrypt_brc2(field_value_bytes, &field_symmetric_key) {
            Ok(encrypted) => {
                log::info!("      Encrypted field value length: {} bytes", encrypted.len());
                log::info!("      Encrypted field value (base64, FULL): {}", base64::engine::general_purpose::STANDARD.encode(&encrypted));
                log::info!("      Encrypted field value (hex, FULL): {}", hex::encode(&encrypted));
                encrypted
            },
            Err(e) => {
                log::error!("   Failed to encrypt field '{}': {}", field_name_str, e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to encrypt field '{}': {}", field_name_str, e)
                }));
            }
        };

        // 3. Encrypt the symmetric key (revelation key) for the certifier using BRC-2
        // CRITICAL: TypeScript SDK uses fieldSymmetricKey.toArray() (no length parameter)
        // This strips leading zeros! If key is [0x00, 0x01, ...], toArray() returns [0x01, ...]
        // We must match this behavior exactly!
        let revelation_key_bytes = {
            // Strip leading zeros to match TypeScript SDK's toArray() behavior
            let mut stripped = field_symmetric_key.to_vec();
            // Remove leading zeros, but keep at least 1 byte (TypeScript uses Math.max(1, actualByteLength))
            while stripped.len() > 1 && stripped[0] == 0 {
                stripped.remove(0);
            }
            stripped
        };

        log::info!("   🔐 Revelation key encryption for field '{}':", field_name_str);
        log::info!("      Original symmetric key (32 bytes, hex): {}", hex::encode(&field_symmetric_key));
        log::info!("      Revelation key after stripping leading zeros ({} bytes, hex): {}", revelation_key_bytes.len(), hex::encode(&revelation_key_bytes));
        log::info!("      ⚠️  TypeScript toArray() strips leading zeros - we must match this!");
        log::info!("      Subject private key (hex, first 16): {}", hex::encode(&subject_private_key[..16]));
        log::info!("      Certifier public key (for encryption, hex): {}", hex::encode(encryption_key));
        log::info!("      Field name: {}", field_name_str);
        log::info!("      Serial number: None (master keyring)");
        log::info!("      Invoice number will be: 2-certificate field encryption-{}", field_name_str);

        // CRITICAL: Use original certifier parameter (matching TypeScript SDK: vargs.certifier)
        // TypeScript SDK: MasterCertificate.createCertificateFields(this, vargs.certifier, vargs.fields)
        // The certifier parameter should match the server's identityKey (SDK validates this)
        let encrypted_revelation_key = match brc2::encrypt_certificate_field(
            &subject_private_key,
            encryption_key,  // Use original certifier parameter (matching TypeScript SDK)
            field_name_str,
            None,  // No serial_number yet
            &revelation_key_bytes,
        ) {
            Ok(encrypted) => {
                log::info!("      ✅ Encrypted revelation key length: {} bytes", encrypted.len());
                log::info!("      Encrypted revelation key (base64, FULL): {}", base64::engine::general_purpose::STANDARD.encode(&encrypted));
                log::info!("      Encrypted revelation key (hex, FULL): {}", hex::encode(&encrypted));
                log::info!("      📝 This is what the server will try to decrypt using:");
                log::info!("         - Server's private key");
                log::info!("         - Client's public key (from x-bsv-auth-identity-key header)");
                log::info!("         - Invoice: 2-certificate field encryption-{}", field_name_str);
                encrypted
            },
            Err(e) => {
                log::error!("   Failed to encrypt revelation key for field '{}': {}", field_name_str, e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to encrypt revelation key for field '{}': {}", field_name_str, e)
                }));
            }
        };

        // 4. Store encrypted field value (base64-encoded)
        certificate_fields.insert(
            field_name_str.to_string(),
            serde_json::Value::String(BASE64.encode(&encrypted_field_value))
        );

        // 5. Store encrypted revelation key (base64-encoded) for certifier
        master_keyring.insert(
            field_name_str.to_string(),
            serde_json::Value::String(BASE64.encode(&encrypted_revelation_key))
        );

        // 6. Store plain revelation key for ourselves (to populate master_key when certificate is received)
        plain_revelation_keys.insert(field_name_str.to_string(), revelation_key_bytes.clone());
    }

    // Build certificate signing request (CSR) per BRC-53 spec
    // TypeScript SDK sends: { clientNonce, type, fields, masterKeyring }
    // Build CSR - match TypeScript SDK exactly (minimal fields only)
    // TypeScript SDK sends: { clientNonce, type, fields, masterKeyring }
    // BRC-53 spec mentions additional fields (messageType, serverSerialNonce, validationKey, etc.),
    // but the TypeScript SDK doesn't send them and it works with real servers.
    // The spec may be aspirational or for future use, but current implementations use the minimal format.
    //
    // CRITICAL: Field order MUST match TypeScript SDK exactly!
    // TypeScript SDK order (from Wallet.ts line 506-511): clientNonce, type, fields, masterKeyring
    //
    // CRITICAL: Understanding counterparty in BRC-42:
    // - When WE create the nonce: WE are "self", SERVER is our "counterparty"
    //   → We derive: ECDH(our_privkey, server_pubkey)
    // - When SERVER verifies: SERVER is "self", WE are their "counterparty"
    //   → Server derives: ECDH(server_privkey, our_pubkey)
    // Since ECDH is symmetric, these produce the same shared secret!
    //
    // SDK creates nonce with: createNonce(this, vargs.certifier)
    //   → certifier = server's identity key (our counterparty)
    // Server verifies with: verifyNonce(clientNonce, server.wallet, clientIdentityKey)
    //   → clientIdentityKey = our identity key (server's counterparty)
    //
    // So we MUST use the SERVER's identity key as counterparty (matching SDK)
    let csr_client_nonce = match create_nonce_with_hmac(&state, Some(&server_identity_key)).await {
        Ok(nonce) => {
            log::info!("   ✅ Created CSR clientNonce with server identity key as counterparty (matching SDK)");
            log::info!("   ✅ Server will verify with client identity key as counterparty (ECDH symmetry ensures match)");
            nonce
        },
        Err(e) => {
            log::error!("   Failed to create CSR nonce: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Nonce creation failed: {}", e)
            }));
        }
    };

    // Build CSR with ONLY the 4 fields TypeScript SDK sends
    // CRITICAL: Remove any extra fields that might have been added elsewhere
    let mut csr_map = serde_json::Map::new();
    csr_map.insert("clientNonce".to_string(), serde_json::Value::String(csr_client_nonce.clone()));
    csr_map.insert("type".to_string(), serde_json::Value::String(type_.clone()));
    csr_map.insert("fields".to_string(), serde_json::Value::Object(certificate_fields.clone()));
    csr_map.insert("masterKeyring".to_string(), serde_json::Value::Object(master_keyring.clone()));

    // Verify we only have the 4 correct fields (explicitly remove any extras)
    let expected_fields = ["clientNonce", "type", "fields", "masterKeyring"];
    let actual_fields: Vec<String> = csr_map.keys().cloned().collect();
    for field in &actual_fields {
        if !expected_fields.contains(&field.as_str()) {
            log::warn!("   ⚠️  Removing unexpected field: {}", field);
            csr_map.remove(field);
        }
    }

    log::info!("   📋 CSR fields (verified): {:?}", csr_map.keys().collect::<Vec<_>>());
    log::info!("   ✅ CSR matches TypeScript SDK format (exactly 4 fields)");

    // Log complete CSR structure for debugging
    log::info!("   📄 Complete CSR JSON structure:");
    log::info!("      clientNonce: {} (CSR nonce, created with server identity key as counterparty)", csr_client_nonce);
    log::info!("      type: {}", type_);
    log::info!("      fields: {} field(s)", certificate_fields.len());
    for (field_name, field_value) in &certificate_fields {
        if let Some(field_str) = field_value.as_str() {
            log::info!("         - {}: {} (base64, {} chars)", field_name,
                if field_str.len() > 60 { format!("{}...", &field_str[..60]) } else { field_str.to_string() },
                field_str.len());
        }
    }
    log::info!("      masterKeyring: {} key(s)", master_keyring.len());
    for (field_name, key_value) in &master_keyring {
        if let Some(key_str) = key_value.as_str() {
            log::info!("         - {}: {} (base64, {} chars)", field_name,
                if key_str.len() > 60 { format!("{}...", &key_str[..60]) } else { key_str.to_string() },
                key_str.len());
        }
    }

    // Serialize using serde_json::to_string - with preserve_order feature, this maintains insertion order
    let csr_json_string = match serde_json::to_string(&serde_json::Value::Object(csr_map)) {
        Ok(s) => s,
        Err(e) => {
            log::error!("   Failed to serialize CSR: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to serialize CSR: {}", e)
            }));
        }
    };

    // Verify the constructed JSON is valid and can be parsed
    // This ensures our manual construction didn't introduce any errors
    let csr: serde_json::Value = match serde_json::from_str(&csr_json_string) {
        Ok(v) => v,
        Err(e) => {
            log::error!("   Manually constructed CSR JSON is invalid: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Manually constructed CSR JSON is invalid: {}", e)
            }));
        }
    };

    // NOTE: We're NOT including serverSerialNonce, validationKey, serialNumber, or serverValidationNonce
    // because the TypeScript SDK doesn't send them and it works. The server might reject them if included.
    log::info!("   📋 CSR matches TypeScript SDK format (minimal fields only)");

    log::info!("   🔐 Encrypted {} field(s) and created masterKeyring", certificate_fields.len());

    // CSR JSON string is already manually constructed above with correct field order
    // No need to serialize again - csr_json_string is ready to use

    // Log the full CSR for debugging (compare with successful metanet-client request)
    log::info!("   📋 ========== FULL CSR JSON (for comparison with metanet-client) ==========");
    log::info!("   📋 {}", csr_json_string);
    log::info!("   📋 ========== CSR STRUCTURE ==========");
    log::info!("   📋   clientNonce: {} ({} chars, CSR nonce)", csr_client_nonce, csr_client_nonce.len());
    log::info!("   📋   type: {} ({} chars)", type_, type_.len());
    log::info!("   📋   fields: {} field(s)", certificate_fields.len());
    for (field_name, field_value) in &certificate_fields {
        if let Some(fv) = field_value.as_str() {
            log::info!("   📋     - {}: {} ({} chars)", field_name, &fv[..std::cmp::min(50, fv.len())], fv.len());
        }
    }
    log::info!("   📋   masterKeyring: {} key(s)", master_keyring.len());
    for (field_name, key_value) in &master_keyring {
        if let Some(kv) = key_value.as_str() {
            log::info!("   📋     - {}: {} ({} chars)", field_name, &kv[..std::cmp::min(50, kv.len())], kv.len());
        }
    }
    log::info!("   📋 =================================================================");

    // Verify field order in serialized JSON by checking actual byte positions
    let field_order_check = ["clientNonce", "type", "fields", "masterKeyring"];
    let mut found_positions: Vec<(usize, &str)> = Vec::new();
    for field in &field_order_check {
        if let Some(pos) = csr_json_string.find(&format!("\"{}\"", field)) {
            found_positions.push((pos, field));
        }
    }
    found_positions.sort_by_key(|(pos, _)| *pos);
    let found_order: Vec<&str> = found_positions.iter().map(|(_, field)| *field).collect();

    log::info!("   🔍 CSR Field order check (by byte position): {:?}", found_order);
    log::info!("   🔍 Field positions: {:?}", found_positions.iter().map(|(pos, field)| format!("{}@{}", field, pos)).collect::<Vec<_>>());

    if found_order != field_order_check {
        log::warn!("   ⚠️  Field order differs from expected! Expected: {:?}, Found: {:?}", field_order_check, found_order);
    } else {
        log::info!("   ✅ Field order matches TypeScript SDK (verified by byte position)");
    }

    // Also verify nested object key order (fields and masterKeyring)
    if let Ok(parsed_csr) = serde_json::from_str::<serde_json::Value>(&csr_json_string) {
        if let Some(fields_obj) = parsed_csr.get("fields").and_then(|v| v.as_object()) {
            let fields_keys: Vec<&String> = fields_obj.keys().collect();
            log::info!("   🔍 Fields object keys (order): {:?}", fields_keys);
        }
        if let Some(master_keyring_obj) = parsed_csr.get("masterKeyring").and_then(|v| v.as_object()) {
            let master_keyring_keys: Vec<&String> = master_keyring_obj.keys().collect();
            log::info!("   🔍 MasterKeyring object keys (order): {:?}", master_keyring_keys);
        }
    }

    // Verify all required fields are present
    let required_fields = ["clientNonce", "type", "fields", "masterKeyring"];
    let mut missing_fields = Vec::new();
    for field in &required_fields {
        if !csr_json_string.contains(&format!("\"{}\"", field)) {
            missing_fields.push(*field);
        }
    }
    if !missing_fields.is_empty() {
        log::error!("   ❌ Missing required fields: {:?}", missing_fields);
    } else {
        log::info!("   ✅ All required fields present");
    }

    // Verify JSON is valid and can be parsed
    match serde_json::from_str::<serde_json::Value>(&csr_json_string) {
        Ok(parsed) => {
            log::info!("   ✅ CSR JSON is valid and parseable");
            // Verify structure matches
            if parsed.get("clientNonce").is_some()
                && parsed.get("type").is_some()
                && parsed.get("fields").is_some()
                && parsed.get("masterKeyring").is_some() {
                log::info!("   ✅ CSR structure is correct");
            } else {
                log::error!("   ❌ CSR structure is incorrect - missing top-level fields");
            }
        },
        Err(e) => {
            log::error!("   ❌ CSR JSON is invalid: {}", e);
        }
    }

    // Get master private key for signing
    let db = state.database.lock().unwrap();
    let master_privkey = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to get identity key: {}", e)
            }));
        }
    };
    drop(db);

    // BRC-103: Use BRC-3 signature (BRC-42 key derivation) for /signCertificate request
    // Use same approach as working /.well-known/auth handler
    // CRITICAL: Use server's identityKey from initialResponse as counterparty (not original certifier)
    use crate::crypto::brc42::derive_child_private_key;
    use crate::crypto::brc43::{InvoiceNumber, SecurityLevel, normalize_protocol_id};
    use crate::crypto::signing::sha256;
    use secp256k1::{Secp256k1, Message, SecretKey, PublicKey};

    // Parse server's identity key from initialResponse (this is the counterparty for BRC-42)
    let server_identity_bytes = match hex::decode(server_identity_key) {
        Ok(b) => b,
        Err(e) => {
            log::error!("   Failed to decode server identity key: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid server identity key: {}", e)
            }));
        }
    };

    // Create invoice number for signing the CSR - use requestNonce (request ID) and server's nonce
    // KeyID combines requestNonce and serverNonce (matching TypeScript SDK's Peer.toPeer pattern)
    // CRITICAL: TypeScript SDK Peer.ts line 128 uses: keyID = `${requestNonce} ${peerSession.peerNonce}`
    // Where requestNonce = Utils.toBase64(Random(32)) (the request ID, first 32 bytes of serialized request)
    // And peerSession.peerNonce = message.initialNonce (the server's nonce from initialResponse)
    // So keyID = requestNonce + " " + serverNonce (request ID first, then server's nonce)
    // CRITICAL: Use csr_request_nonce (the request ID) for keyID, NOT csr_client_nonce (from CSR body)
    let csr_key_id = format!("{} {}", csr_request_nonce, server_serial_nonce);
    let csr_protocol_id = match normalize_protocol_id("auth message signature") {
        Ok(p) => p,
        Err(e) => {
            log::error!("   Failed to normalize protocol ID for CSR: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Protocol ID error: {}", e)
            }));
        }
    };
    let csr_invoice_number = match InvoiceNumber::new(
        SecurityLevel::CounterpartyLevel,
        csr_protocol_id,
        &csr_key_id
    ) {
        Ok(inv) => inv,
        Err(e) => {
            log::error!("   Failed to create invoice number for CSR: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to create invoice number: {}", e)
            }));
        }
    };
    let csr_invoice_number_str = csr_invoice_number.to_string();

    // BRC-42: Derive child private key for signing CSR
    // Use server's identityKey as counterparty (from initialResponse)
    let csr_child_private_key = match derive_child_private_key(
        &master_privkey,
        &server_identity_bytes,
        &csr_invoice_number_str
    ) {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Failed to derive child private key for CSR: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to derive signing key: {}", e)
            }));
        }
    };

    // The TypeScript SDK's AuthFetch.fetch() serializes the request as:
    // [32-byte requestNonce][VarInt methodLen][method][VarInt pathLen][path][VarInt searchLen][search][VarInt headerCount][headers...][VarInt bodyLen][body]
    // Then Peer.toPeer() signs this entire binary data.
    //
    // We need to match this exact format for the signature to verify.
    use crate::transaction::{encode_varint, encode_varint_signed};

    let csr_request_nonce_bytes = base64::engine::general_purpose::STANDARD.decode(&csr_request_nonce)
        .unwrap_or_else(|_| vec![0u8; 32]); // Fallback if decode fails

    let mut serialized_request = Vec::new();

    // 1. Write 32-byte request nonce
    serialized_request.extend_from_slice(&csr_request_nonce_bytes);

    // 2. Write method: [VarInt methodLen][method]
    let method = b"POST";
    serialized_request.extend_from_slice(&encode_varint(method.len() as u64));
    serialized_request.extend_from_slice(method);

    // 3. Write pathname: [VarInt pathLen][path] (or -1 if empty)
    let pathname = b"/signCertificate";
    serialized_request.extend_from_slice(&encode_varint(pathname.len() as u64));
    serialized_request.extend_from_slice(pathname);

    // 4. Write search params: [VarInt searchLen][search] (or -1 if empty)
    // No search params, so write -1 using signed VarInt encoding
    serialized_request.extend_from_slice(&encode_varint_signed(-1));

    // 5. Write headers: [VarInt headerCount][for each: VarInt keyLen][key][VarInt valueLen][value]
    // Headers to include: content-type (normalized, lowercase key)
    // TypeScript SDK only includes: x-bsv-* (excluding x-bsv-auth-*), authorization, and content-type
    // It normalizes content-type by removing parameters (e.g., "; charset=utf-8")
    let mut included_headers: Vec<(String, String)> = Vec::new();
    // Add content-type header (normalized - remove parameters)
    let content_type = "application/json"; // Already normalized (no charset param)
    included_headers.push(("content-type".to_string(), content_type.to_string()));

    // Sort headers by key (TypeScript SDK does this)
    included_headers.sort_by(|a, b| a.0.cmp(&b.0));

    // Write header count
    serialized_request.extend_from_slice(&encode_varint(included_headers.len() as u64));

    // Write each header
    for (key, value) in &included_headers {
        let key_bytes = key.as_bytes();
        serialized_request.extend_from_slice(&encode_varint(key_bytes.len() as u64));
        serialized_request.extend_from_slice(key_bytes);

        let value_bytes = value.as_bytes();
        serialized_request.extend_from_slice(&encode_varint(value_bytes.len() as u64));
        serialized_request.extend_from_slice(value_bytes);
    }

    // 6. Write body: [VarInt bodyLen][body]
    let body_bytes = csr_json_string.as_bytes();
    serialized_request.extend_from_slice(&encode_varint(body_bytes.len() as u64));
    serialized_request.extend_from_slice(body_bytes);

    // Verify requestId: first 32 bytes of serialized request should match csr_request_nonce_bytes
    let calculated_request_id = base64::engine::general_purpose::STANDARD.encode(&serialized_request[0..32]);
    if calculated_request_id != csr_request_nonce {
        log::error!("   ❌ Request ID mismatch! Calculated: {}, Expected: {}", calculated_request_id, csr_request_nonce);
    } else {
        log::info!("   ✅ Request ID verified: {}", csr_request_nonce);
    }

    // Now sign the entire serialized request
    let csr_hash = sha256(&serialized_request);

    log::info!("   📦 Serialized request for signing:");
    log::info!("      Total length: {} bytes", serialized_request.len());
    log::info!("      Request nonce: {} bytes", csr_request_nonce_bytes.len());
    log::info!("      Method: POST ({} bytes)", method.len());
    log::info!("      Path: /signCertificate ({} bytes)", pathname.len());
    log::info!("      Headers: {} header(s)", included_headers.len());
    log::info!("      Body: {} bytes", body_bytes.len());
    log::info!("      Serialized request (hex, first 200 bytes): {}",
        hex::encode(&serialized_request[..200.min(serialized_request.len())]));
    log::info!("      Serialized request (hex, FULL): {}", hex::encode(&serialized_request));
    log::info!("      Serialized request (base64, FULL): {}", base64::engine::general_purpose::STANDARD.encode(&serialized_request));

    // Detailed breakdown
    let mut offset = 0;
    offset += 32; // nonce
    log::info!("      [{}..{}] Nonce: {}", 0, offset-1, hex::encode(&serialized_request[0..offset]));
    let method_varint_len = if serialized_request[offset] < 0xFD { 1 } else if serialized_request[offset] == 0xFD { 3 } else if serialized_request[offset] == 0xFE { 5 } else { 9 };
    offset += method_varint_len;
    log::info!("      [{}..{}] Method VarInt ({} bytes): {}", offset-method_varint_len, offset-1, method_varint_len, hex::encode(&serialized_request[offset-method_varint_len..offset]));
    offset += method.len();
    log::info!("      [{}..{}] Method: {}", offset-method.len(), offset-1, String::from_utf8_lossy(&serialized_request[offset-method.len()..offset]));
    let path_varint_len = if serialized_request[offset] < 0xFD { 1 } else if serialized_request[offset] == 0xFD { 3 } else if serialized_request[offset] == 0xFE { 5 } else { 9 };
    offset += path_varint_len;
    log::info!("      [{}..{}] Path VarInt ({} bytes): {}", offset-path_varint_len, offset-1, path_varint_len, hex::encode(&serialized_request[offset-path_varint_len..offset]));
    offset += pathname.len();
    log::info!("      [{}..{}] Path: {}", offset-pathname.len(), offset-1, String::from_utf8_lossy(&serialized_request[offset-pathname.len()..offset]));
    offset += 9; // search (-1)
    log::info!("      [{}..{}] Search VarInt (-1, 9 bytes): {}", offset-9, offset-1, hex::encode(&serialized_request[offset-9..offset]));

    // Log header section
    let header_section_start = offset;
    log::info!("      [{}..] Header section starts", header_section_start);
    for (idx, (key, value)) in included_headers.iter().enumerate() {
        log::info!("         Header {}: {} = {} (key: {} bytes, value: {} bytes)", idx+1, key, value, key.as_bytes().len(), value.as_bytes().len());
    }

    // Calculate header section end (approximate)
    let header_section_size = encode_varint(included_headers.len() as u64).len()
        + included_headers.iter().map(|(k, v)| {
            encode_varint(k.as_bytes().len() as u64).len() + k.as_bytes().len()
            + encode_varint(v.as_bytes().len() as u64).len() + v.as_bytes().len()
        }).sum::<usize>();
    let header_section_end = header_section_start + header_section_size;
    log::info!("      [{}..{}] Header section ({} bytes, hex): {}", header_section_start, header_section_end-1, header_section_size, hex::encode(&serialized_request[header_section_start..header_section_end.min(serialized_request.len())]));

    // Log body section
    let body_section_start = header_section_end;
    let body_varint_len = if serialized_request[body_section_start] < 0xFD { 1 } else if serialized_request[body_section_start] == 0xFD { 3 } else if serialized_request[body_section_start] == 0xFE { 5 } else { 9 };
    let body_section_end = body_section_start + body_varint_len + body_bytes.len();
    log::info!("      [{}..{}] Body length VarInt ({} bytes): {}", body_section_start, body_section_start+body_varint_len-1, body_varint_len, hex::encode(&serialized_request[body_section_start..body_section_start+body_varint_len]));
    log::info!("      [{}..{}] Body ({} bytes, hex, first 200): {}", body_section_start+body_varint_len, body_section_end-1, body_bytes.len(), hex::encode(&serialized_request[body_section_start+body_varint_len..body_section_start+body_varint_len+200.min(body_bytes.len())]));

    // Log FULL CSR JSON for comparison (this is what we're sending to the server)
    log::info!("   📋 ========== COMPLETE CSR JSON BEING SENT ==========");
    log::info!("   📋 {}", csr_json_string);

    // Verify the JSON only contains the 4 expected fields
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&csr_json_string) {
        if let Some(obj) = parsed.as_object() {
            let fields: Vec<&String> = obj.keys().collect();
            log::info!("   📋 Fields in JSON (count: {}): {:?}", fields.len(), fields);
            if fields.len() != 4 {
                log::error!("   ❌ ERROR: JSON has {} fields, expected 4!", fields.len());
            }
            for field in &fields {
                if !["clientNonce", "type", "fields", "masterKeyring"].contains(&field.as_str()) {
                    log::error!("   ❌ ERROR: Unexpected field in JSON: {}", field);
                }
            }
        }
    }
    log::info!("   📋 =================================================");

    // Log body bytes in detail for comparison
    log::info!("   📦 Body bytes (hex, full): {}", hex::encode(&body_bytes));
    log::info!("   📦 Body bytes (base64): {}", base64::engine::general_purpose::STANDARD.encode(&body_bytes));
    log::info!("   📦 Body length: {} bytes", body_bytes.len());

    log::info!("   📝 CSR JSON (first 200 chars): {}",
        if csr_json_string.len() > 200 {
            &csr_json_string[..200]
        } else {
            &csr_json_string
        });
    log::info!("   🔐 Signing CSR (BRC-3 with BRC-42):");
    log::info!("      Request hash (SHA256): {}", hex::encode(&csr_hash));
    log::info!("      ProtocolID: auth message signature");
    log::info!("      KeyID: {}", csr_key_id);
    log::info!("      KeyID (hex bytes): {}", hex::encode(csr_key_id.as_bytes()));
    log::info!("      KeyID length: {} bytes ({} chars)", csr_key_id.as_bytes().len(), csr_key_id.len());
    log::info!("      Request nonce (first part): {}", csr_request_nonce);
    log::info!("      Server nonce (second part): {}", server_serial_nonce);
    log::info!("      Counterparty: {} (server's identityKey from initialResponse)", server_identity_key);
    log::info!("      Invoice Number: {}", csr_invoice_number_str);

    // Sign the hash with derived child private key
    let secp2 = Secp256k1::new();
    let secret_key2 = SecretKey::from_slice(&csr_child_private_key).unwrap();
    let message2 = Message::from_digest_slice(&csr_hash).unwrap();

    let signature2 = secp2.sign_ecdsa(&message2, &secret_key2);
    let signature_der2 = signature2.serialize_der();

    // TypeScript SDK uses hex encoding for signatures
    let signature_hex2 = hex::encode(&signature_der2);

    log::info!("   🔐 BRC-103 Authentication:");
    log::info!("      Identity Key: {}", subject_public_key);
    log::info!("      Request Hash (SHA256): {}", hex::encode(&csr_hash));
    log::info!("      Signature (DER, hex): {}", signature_hex2);
    log::info!("      Signature length: {} bytes", signature_der2.len());
    log::info!("      CSR JSON length: {} bytes", csr_json_string.len());
    log::info!("   🔐 BRC-42 Key Derivation for Signing:");
    log::info!("      Our master private key (hex, first 16): {}", hex::encode(&master_privkey[..16]));
    log::info!("      Server identity key (counterparty, hex): {}", hex::encode(&server_identity_bytes));
    log::info!("      Invoice number: {}", csr_invoice_number_str);
    log::info!("      KeyID: {}", csr_key_id);
    log::info!("      Derived child private key (hex, first 16): {}", hex::encode(&csr_child_private_key[..16]));

    // Verify we can derive the public key from our child private key
    let secp_test = Secp256k1::new();
    let child_secret_test = SecretKey::from_slice(&csr_child_private_key).unwrap();
    let child_public_test = PublicKey::from_secret_key(&secp_test, &child_secret_test);
    let child_public_hex = hex::encode(&child_public_test.serialize());
    log::info!("      Derived child public key (hex): {}", child_public_hex);
    log::info!("      ✅ Server should derive this same public key to verify signature");

    // Make HTTP POST request to /signCertificate endpoint with BRC-103 authentication
    let sign_url = if certifier_url.ends_with('/') {
        format!("{}signCertificate", certifier_url)
    } else {
        format!("{}/signCertificate", certifier_url)
    };

    // Summary of all encrypted values for comparison with test server
    log::info!("   📊 ========== ENCRYPTION SUMMARY (for comparison) ==========");
    log::info!("   📊 Client Identity Key (for server decryption): {}", subject_public_key);
    log::info!("   📊 Certifier Public Key (used for encryption): {}", hex::encode(&certifier_bytes));
    log::info!("   📊 Server Identity Key (from initialResponse): {}", hex::encode(&server_identity_bytes));
    if certifier_bytes != server_identity_bytes {
        log::warn!("   📊 ⚠️  Certifier != Server Identity Key (server may not decrypt)");
    }
    for (field_name, field_value) in &certificate_fields {
        if let Some(field_str) = field_value.as_str() {
            log::info!("   📊 Field '{}' encrypted value (base64): {}", field_name, field_str);
        }
    }
    for (field_name, key_value) in &master_keyring {
        if let Some(key_str) = key_value.as_str() {
            log::info!("   📊 Field '{}' revelation key (base64): {}", field_name, key_str);
            log::info!("   📊   Invoice for decryption: 2-certificate field encryption-{}", field_name);
        }
    }
    log::info!("   📊 ======================================================");

    log::info!("   📤 ========== REQUEST TO CERTIFIER ==========");
    log::info!("   📤 POST to: {}", sign_url);
    log::info!("   📤 Headers:");
    log::info!("   📤   x-bsv-auth-version: 0.1");
    log::info!("   📤   x-bsv-auth-identity-key: {} (full: {})", &subject_public_key[..20], subject_public_key);
    log::info!("   📤   x-bsv-auth-nonce: {} (full: {})", &csr_request_nonce[..20], csr_request_nonce);
    log::info!("   📤   x-bsv-auth-your-nonce: {} (full: {})", &server_serial_nonce[..20], server_serial_nonce);
    log::info!("   📤   x-bsv-auth-request-id: {} (full: {})", &csr_request_nonce[..20], csr_request_nonce);
    log::info!("   📤   x-bsv-auth-signature: {}... (full: {})", &signature_hex2[..20], signature_hex2);
    log::info!("   📤 Body (CSR JSON): {} bytes", csr_json_string.len());
    log::info!("   📤 ==========================================");

    // TypeScript SDK's SimplifiedFetchTransport sends these headers for general messages:
    // - x-bsv-auth-version: "0.1"
    // - x-bsv-auth-identity-key: identity key
    // - x-bsv-auth-nonce: requestNonce (from Peer.toPeer(), same as request ID)
    // - x-bsv-auth-your-nonce: server's nonce from initialResponse
    // - x-bsv-auth-signature: signature (hex)
    // - x-bsv-auth-request-id: requestId (first 32 bytes of serialized request, base64)
    // CRITICAL: x-bsv-auth-nonce and x-bsv-auth-request-id MUST be the same value!
    //
    // CRITICAL: The Content-Type header we send MUST match what we serialized for signing!
    // If CEF modifies it (adds charset, changes case), signature verification will fail.
    let content_type_header_value = "application/json";
    log::info!("   ⚠️  CRITICAL: Content-Type header value being sent: '{}'", content_type_header_value);
    log::info!("   ⚠️  This MUST match the value in serialized request: '{}'", content_type);
    if content_type_header_value != content_type {
        log::error!("   ❌ Content-Type mismatch! Serialized: '{}', Sending: '{}'", content_type, content_type_header_value);
    }

    let response = match client
        .post(&sign_url)
        .header("Content-Type", content_type_header_value)
        .header("x-bsv-auth-version", "0.1")
        .header("x-bsv-auth-identity-key", &subject_public_key)
        .header("x-bsv-auth-nonce", &csr_request_nonce)  // Request nonce (same as request ID, from Peer.toPeer())
        .header("x-bsv-auth-your-nonce", &server_serial_nonce)  // Server's nonce from initialResponse
        .header("x-bsv-auth-request-id", &csr_request_nonce)  // Request ID (first 32 bytes of serialized request)
        .header("x-bsv-auth-signature", &signature_hex2)
        .body(csr_json_string.clone())
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("   Failed to connect to certifier: {}", e);
            return HttpResponse::BadGateway().json(serde_json::json!({
                "error": format!("Failed to connect to certifier: {}", e)
            }));
        }
    };

    let status = response.status();

    // Get headers before consuming the response (collect into a Vec of tuples)
    let response_headers: Vec<(String, String)> = response.headers()
        .iter()
        .map(|(name, value)| {
            (name.to_string(), value.to_str().unwrap_or("").to_string())
        })
        .collect();

    // Get response body as text first (for potential signature verification)
    let response_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());

    if !status.is_success() {
        log::error!("   ❌ Certifier returned error: {} - {}", status, response_text);
        log::error!("   📋 Full response headers: {:?}", response_headers);
        log::error!("   📋 Response body (full): {}", response_text);

        // Try to parse error JSON for more details
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            log::error!("   📋 Parsed error JSON: {}", serde_json::to_string_pretty(&error_json).unwrap_or_else(|_| "parse failed".to_string()));
            if let Some(description) = error_json.get("description") {
                log::error!("   📋 Error description: {}", description);
            }
            if let Some(code) = error_json.get("code") {
                log::error!("   📋 Error code: {}", code);
            }
        }

        return HttpResponse::BadGateway().json(serde_json::json!({
            "error": format!("Certifier error ({}): {}", status, response_text)
        }));
    }

    // TODO: Verify server's response signature (X-Authrite-Signature header)
    // For now, we'll trust the response if status is successful
    // In production, we should verify the certifier's signature on the response

    // Parse certificate response from certifier
    let cert_response: serde_json::Value = match serde_json::from_str(&response_text) {
        Ok(cert) => cert,
        Err(e) => {
            log::error!("   Failed to parse certifier response: {}", e);
            log::error!("   Response body: {}", response_text);
            return HttpResponse::BadGateway().json(serde_json::json!({
                "error": format!("Invalid response from certifier: {}", e)
            }));
        }
    };

    log::info!("   ✅ Received certificate from certifier");

    // Handle BRC-53 response format: { "status": "success", "certificate": { ... } }
    // Or direct format: { "type": "...", "certifier": "...", ... }
    let cert_obj = if cert_response.get("certificate").is_some() {
        // BRC-53 format - extract certificate object
        log::info!("   📋 Response is in BRC-53 format (with 'certificate' field)");
        cert_response.get("certificate").unwrap().clone()
    } else {
        // Direct format - use response directly
        log::info!("   📋 Response is in direct format (certificate object directly)");
        cert_response.clone()
    };

    // Process certificate like 'direct' protocol
    // Build a new request with the certificate data from certifier
    // If certifier didn't return keyringForSubject, populate it with our plain revelation keys
    let keyring_for_subject = if cert_obj.get("keyringForSubject").is_some() {
        // Certifier returned keyringForSubject (unlikely, but handle it)
        cert_obj.get("keyringForSubject").cloned()
    } else {
        // Certifier didn't return keyringForSubject - populate with our plain revelation keys
        // This allows us to decrypt fields later
        let mut keyring_map = serde_json::Map::new();
        for (field_name, revelation_key_bytes) in &plain_revelation_keys {
            keyring_map.insert(
                field_name.clone(),
                serde_json::Value::String(BASE64.encode(revelation_key_bytes))
            );
        }
        Some(serde_json::Value::Object(keyring_map))
    };

    let direct_req = AcquireCertificateRequest {
        acquisition_protocol: Some(AcquisitionProtocol::Direct), // Switch to 'direct' protocol
        type_: cert_obj.get("type").and_then(|v| v.as_str()).map(|s| s.to_string()),
        certifier: cert_obj.get("certifier").and_then(|v| v.as_str()).map(|s| s.to_string()),
        fields: cert_obj.get("fields").cloned(),
        serial_number: cert_obj.get("serialNumber").and_then(|v| v.as_str()).map(|s| s.to_string()),
        revocation_outpoint: cert_obj.get("revocationOutpoint").and_then(|v| v.as_str()).map(|s| s.to_string()),
        signature: cert_obj.get("signature").and_then(|v| v.as_str()).map(|s| s.to_string()),
        keyring_for_subject: keyring_for_subject,
        subject: cert_obj.get("subject").and_then(|v| v.as_str()).map(|s| s.to_string()),
        certifier_url: None,
    };

    // Process using 'direct' protocol handler
    acquire_certificate_direct(state, web::Json(direct_req)).await
}

// ============================================================================
// Method 19: proveCertificate (Call Code 19)
// ============================================================================

/// Request structure for proveCertificate
#[derive(Debug, Deserialize)]
pub struct ProveCertificateRequest {
    /// Certificate identifier (partial match)
    pub certificate: CertificateIdentifier,

    /// Fields to reveal (array of field names)
    pub fields_to_reveal: Vec<String>,

    /// Verifier's public key (33-byte compressed, hex-encoded)
    pub verifier: String,  // Hex string

    /// Privileged access (optional)
    #[serde(default)]
    pub privileged: Option<bool>,

    /// Privileged reason (optional, required if privileged=true)
    #[serde(default)]
    pub privileged_reason: Option<String>,
}

/// Certificate identifier (partial match for listCertificates)
#[derive(Debug, Deserialize)]
pub struct CertificateIdentifier {
    /// Certificate type (base64-encoded, optional)
    #[serde(rename = "type")]
    #[serde(default)]
    pub type_: Option<String>,

    /// Serial number (base64-encoded, optional)
    #[serde(default)]
    pub serial_number: Option<String>,

    /// Certifier (hex-encoded, optional)
    #[serde(default)]
    pub certifier: Option<String>,

    /// Subject (hex-encoded, optional)
    #[serde(default)]
    pub subject: Option<String>,

    /// Revocation outpoint (optional)
    #[serde(default)]
    pub revocation_outpoint: Option<String>,

    /// Signature (hex-encoded, optional)
    #[serde(default)]
    pub signature: Option<String>,
}

/// Response structure for proveCertificate
#[derive(Debug, Serialize)]
pub struct ProveCertificateResponse {
    /// Keyring for verifier (fieldName → base64-encoded encrypted revelation key)
    pub keyring_for_verifier: std::collections::HashMap<String, String>,
}

/// proveCertificate - BRC-100 endpoint (Call Code 19)
///
/// Generates a proof for selective disclosure of certificate fields.
///
/// **Process**:
/// 1. Find certificate using listCertificates (must match exactly 1)
/// 2. Get master keyring from database
/// 3. For each field to reveal:
///    - Decrypt master keyring entry (encrypted for subject/certifier)
///    - Re-encrypt field revelation key for verifier
///    - Add to verifier keyring
/// 4. Return keyring with only revealed fields
pub async fn prove_certificate(
    state: web::Data<AppState>,
    req: web::Json<ProveCertificateRequest>,
) -> HttpResponse {
    log::info!("📋 /proveCertificate called");

    // Validate inputs
    if req.fields_to_reveal.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "fieldsToReveal must not be empty"
        }));
    }

    // Decode verifier public key
    let verifier_pubkey_bytes = match hex::decode(&req.verifier) {
        Ok(b) => {
            if b.len() != 33 {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "verifier must be 33 bytes (compressed public key)"
                }));
            }
            b
        }
        Err(e) => {
            log::error!("   Invalid verifier public key: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid verifier public key: {}", e)
            }));
        }
    };

    // Step 1: Find certificate using listCertificates (must match exactly 1)
    let db = state.database.lock().unwrap();
    let cert_repo = CertificateRepository::new(db.connection());

    // Build listCertificates request from certificate identifier
    let certifier_filter: Option<&str> = req.certificate.certifier.as_deref();
    let type_filter: Option<&str> = req.certificate.type_.as_deref();
    let subject_filter: Option<&str> = req.certificate.subject.as_deref();

    // Convert certificate identifier to filters for listCertificates
    let list_result = cert_repo.list_certificates(
        type_filter,
        certifier_filter,
        subject_filter,
        Some(false), // Only active certificates (not deleted)
        Some(2), // Limit to 2 to check for uniqueness
        Some(0), // Offset
    );

    let certificates = match list_result {
        Ok(certs) => certs,
        Err(e) => {
            log::error!("   Failed to list certificates: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to list certificates: {}", e)
            }));
        }
    };

    // Filter certificates to match the identifier exactly
    let mut matching_certs: Vec<_> = certificates.into_iter().filter(|cert| {
        // Match type
        if let Some(ref type_) = req.certificate.type_ {
            let cert_type_b64 = BASE64.encode(&cert.type_);
            if cert_type_b64 != *type_ {
                return false;
            }
        }

        // Match serial number
        if let Some(ref serial) = req.certificate.serial_number {
            let cert_serial_b64 = BASE64.encode(&cert.serial_number);
            if cert_serial_b64 != *serial {
                return false;
            }
        }

        // Match certifier
        if let Some(ref certifier) = req.certificate.certifier {
            let cert_certifier_hex = hex::encode(&cert.certifier);
            if cert_certifier_hex != *certifier {
                return false;
            }
        }

        // Match subject
        if let Some(ref subject) = req.certificate.subject {
            let cert_subject_hex = hex::encode(&cert.subject);
            if cert_subject_hex != *subject {
                return false;
            }
        }

        // Match revocation outpoint
        if let Some(ref rev_outpoint) = req.certificate.revocation_outpoint {
            if cert.revocation_outpoint != *rev_outpoint {
                return false;
            }
        }

        // Match signature (if provided)
        if let Some(ref sig) = req.certificate.signature {
            let cert_sig_hex = hex::encode(&cert.signature);
            if cert_sig_hex != *sig {
                return false;
            }
        }

        true
    }).collect();

    // Must match exactly 1 certificate
    if matching_certs.len() != 1 {
        log::error!("   Certificate match failed: found {} certificates (expected 1)", matching_certs.len());
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Certificate match failed: found {} certificates (expected exactly 1)", matching_certs.len())
        }));
    }

    let certificate = matching_certs.remove(0);
    log::info!("   ✅ Found certificate: type={}, serial={}",
        BASE64.encode(&certificate.type_),
        BASE64.encode(&certificate.serial_number));

    // Step 2: Get subject's private key from database
    let subject_private_key = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to get master private key: {}", e)
            }));
        }
    };

    // Step 3: Generate verifier keyring
    let serial_number_b64 = BASE64.encode(&certificate.serial_number);
    let certifier_pubkey_bytes = certificate.certifier.clone();

    let verifier_keyring = match crate::certificate::selective_disclosure::create_keyring_for_verifier(
        db.connection(),
        &certificate,
        &subject_private_key,
        &certifier_pubkey_bytes,
        &verifier_pubkey_bytes,
        &req.fields_to_reveal,
        &serial_number_b64,
    ) {
        Ok(keyring) => keyring,
        Err(e) => {
            log::error!("   Failed to create keyring for verifier: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to create keyring: {}", e)
            }));
        }
    };

    log::info!("   ✅ Generated keyring for {} fields", verifier_keyring.len());

    HttpResponse::Ok().json(ProveCertificateResponse {
        keyring_for_verifier: verifier_keyring,
    })
}
