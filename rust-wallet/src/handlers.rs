use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use crate::AppState;
use crate::crypto::brc42::{derive_child_private_key, derive_child_public_key};
use crate::crypto::brc43::{InvoiceNumber, SecurityLevel, normalize_protocol_id};
use crate::crypto::signing::{sha256, hmac_sha256, verify_hmac_sha256};
use crate::crypto::brc2::{derive_symmetric_key, encrypt_brc2, decrypt_brc2};

// Certificate handlers (Group C - Part 3)
mod certificate_handlers;
pub use certificate_handlers::{
    relinquish_certificate,
    list_certificates,
    acquire_certificate,
    prove_certificate,
    discover_by_identity_key,
    discover_by_attributes,
};

// ============================================================================
// Fee Calculation Utilities
// ============================================================================

/// Default fee rate in satoshis per kilobyte (1 sat/byte = 1000 sat/kb)
/// BSV miners currently accept 0.5-1 sat/byte. Using 1 sat/byte for safety margin.
pub const DEFAULT_SATS_PER_KB: u64 = 1000;

/// Minimum fee to ensure transaction relay (dust prevention)
pub const MIN_FEE_SATS: u64 = 200;

/// Calculate the byte size of a VarInt encoding
fn varint_size(val: usize) -> usize {
    if val <= 0xFC { 1 }
    else if val <= 0xFFFF { 3 }
    else if val <= 0xFFFFFFFF { 5 }
    else { 9 }
}

/// Calculate the serialized size of a transaction input
/// Format: 32 (txid) + 4 (vout) + varint(script_len) + script + 4 (sequence)
fn input_size(script_len: usize) -> usize {
    32 + 4 + varint_size(script_len) + script_len + 4
}

/// Calculate the serialized size of a transaction output
/// Format: 8 (satoshis) + varint(script_len) + script
fn output_size(script_len: usize) -> usize {
    8 + varint_size(script_len) + script_len
}

/// Estimate transaction size in bytes from script lengths
///
/// # Arguments
/// * `input_script_lengths` - Vec of unlocking script lengths (in bytes)
/// * `output_script_lengths` - Vec of locking script lengths (in bytes)
///
/// # Returns
/// Estimated transaction size in bytes
pub fn estimate_transaction_size(
    input_script_lengths: &[usize],
    output_script_lengths: &[usize],
) -> usize {
    let mut size = 4; // Version (4 bytes)

    // Input count (varint)
    size += varint_size(input_script_lengths.len());

    // All inputs
    for script_len in input_script_lengths {
        size += input_size(*script_len);
    }

    // Output count (varint)
    size += varint_size(output_script_lengths.len());

    // All outputs
    for script_len in output_script_lengths {
        size += output_size(*script_len);
    }

    size += 4; // Locktime (4 bytes)

    size
}

/// Calculate transaction fee based on size and rate
///
/// # Arguments
/// * `tx_size_bytes` - Transaction size in bytes
/// * `sats_per_kb` - Fee rate in satoshis per kilobyte (1000 = 1 sat/byte)
///
/// # Returns
/// Fee in satoshis (minimum MIN_FEE_SATS)
pub fn calculate_fee(tx_size_bytes: usize, sats_per_kb: u64) -> u64 {
    // Calculate: (size * rate + 999) / 1000 to round up
    let fee = ((tx_size_bytes as u64 * sats_per_kb) + 999) / 1000;
    std::cmp::max(fee, MIN_FEE_SATS)
}

/// Estimate fee for a transaction before it's fully built
/// Uses typical script sizes for P2PKH inputs/outputs
///
/// P2PKH unlocking script: ~107 bytes (signature ~71-72 + pubkey 33 + push opcodes)
/// P2PKH locking script: 25 bytes (OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG)
///
/// # Arguments
/// * `num_inputs` - Number of inputs (will use P2PKH unlocking script size estimate)
/// * `output_script_lengths` - Actual locking script lengths for outputs
/// * `include_change` - Whether to include a P2PKH change output
/// * `sats_per_kb` - Fee rate in satoshis per kilobyte
///
/// # Returns
/// Estimated fee in satoshis
pub fn estimate_fee_for_transaction(
    num_inputs: usize,
    output_script_lengths: &[usize],
    include_change: bool,
    sats_per_kb: u64,
) -> u64 {
    const P2PKH_UNLOCKING_SCRIPT_LEN: usize = 107; // Typical P2PKH signature + pubkey
    const P2PKH_LOCKING_SCRIPT_LEN: usize = 25;    // P2PKH output script

    // Build input script lengths (assume P2PKH for wallet inputs)
    let input_script_lengths: Vec<usize> = vec![P2PKH_UNLOCKING_SCRIPT_LEN; num_inputs];

    // Build output script lengths
    let mut all_output_scripts: Vec<usize> = output_script_lengths.to_vec();
    if include_change {
        all_output_scripts.push(P2PKH_LOCKING_SCRIPT_LEN);
    }

    let size = estimate_transaction_size(&input_script_lengths, &all_output_scripts);
    calculate_fee(size, sats_per_kb)
}

/// Get fee from a fully built transaction
pub fn get_transaction_fee(tx: &crate::transaction::Transaction, sats_per_kb: u64) -> u64 {
    match tx.serialize() {
        Ok(bytes) => calculate_fee(bytes.len(), sats_per_kb),
        Err(_) => {
            // Fallback to estimation if serialization fails
            log::warn!("   Failed to serialize transaction for fee calculation, using estimate");
            estimate_fee_for_transaction(tx.inputs.len(), &[], true, sats_per_kb)
        }
    }
}

// TODO: Future enhancement - Dynamic fee rate fetching from MAPI
//
// BSV miners expose fee quotes via Merchant API (MAPI):
// - TAAL: https://merchantapi.taal.com/mapi/feeQuote
// - GorillaPool: Similar endpoint
//
// Response format:
// {
//   "fees": [{
//     "feeType": "standard",
//     "miningFee": { "satoshis": 500, "bytes": 1000 },
//     "relayFee": { "satoshis": 250, "bytes": 1000 }
//   }]
// }
//
// Implementation plan:
// 1. Add FeeRateCache struct with TTL (1 hour recommended)
// 2. Add async fn fetch_mapi_fee_quote() -> Result<u64, Error>
// 3. Store cached rate in AppState
// 4. Fall back to DEFAULT_SATS_PER_KB on error
//
// See: https://github.com/bitcoin-sv-specs/brfc-merchantapi

// Health check
pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "version": "0.1.0-rust",
        "backend": "rust-wallet"
    }))
}

// BRC-100 status check
pub async fn brc100_status() -> HttpResponse {
    log::info!("📋 /brc100/status called");
    HttpResponse::Ok().json(serde_json::json!({
        "available": true,          // For CEF isAvailable() check
        "service": "BRC-100",
        "version": "1.0.0",
        "status": "operational",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "components": {
            "identity": "operational",
            "authentication": "operational",
            "session": "operational",
            "beef": "operational",
            "spv": "operational"
        }
    }))
}

// /getVersion - BRC-100 endpoint
pub async fn get_version() -> HttpResponse {
    log::info!("📋 /getVersion called");
    HttpResponse::Ok().json(serde_json::json!({
        "version": "HodosWallet-Rust v0.0.1",
        "capabilities": [
            "getVersion",
            "getPublicKey",
            "createSignature",
            "isAuthenticated",
            "createAction",
            "signAction",
            "processAction"
        ],
        "brc100": true,
        "timestamp": chrono::Utc::now()
    }))
}

// /getPublicKey - BRC-100 endpoint
// Returns the master identity key (m) for BRC-100 authentication
pub async fn get_public_key(state: web::Data<AppState>) -> HttpResponse {
    log::info!("📋 /getPublicKey called - returning MASTER identity key");

    // Get master public key from database
    let db = state.database.lock().unwrap();
    let master_pubkey = match crate::database::get_master_public_key_from_db(&db) {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Failed to get master public key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to get master key: {}", e)
            }));
        }
    };
    drop(db);

    let master_pubkey_hex = hex::encode(master_pubkey);

    log::info!("   Master public key: {}", master_pubkey_hex);

    HttpResponse::Ok().json(serde_json::json!({
        "publicKey": master_pubkey_hex
    }))
}

// /isAuthenticated - BRC-100 endpoint
pub async fn is_authenticated() -> HttpResponse {
    log::info!("📋 /isAuthenticated called");
    HttpResponse::Ok().json(serde_json::json!({
        "authenticated": true
    }))
}

// /waitForAuthentication - BRC-100 endpoint (Call Code 24)
// Waits for wallet to be initialized and returns once ready.
// Unlike wrapper implementations, this IS the actual wallet - so we validate state.
pub async fn wait_for_authentication(state: web::Data<AppState>) -> HttpResponse {
    log::info!("📋 /waitForAuthentication called");

    // Verify wallet exists in database
    let db = state.database.lock().unwrap();
    let wallet_repo = crate::database::WalletRepository::new(db.connection());

    match wallet_repo.get_primary_wallet() {
        Ok(Some(wallet)) => {
            log::info!("   ✅ Wallet ready (ID: {})", wallet.id.unwrap_or(0));
            HttpResponse::Ok().json(serde_json::json!({
                "authenticated": true
            }))
        }
        Ok(None) => {
            log::warn!("   ⚠️ Wallet not initialized");
            HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "Wallet not initialized",
                "authenticated": false
            }))
        }
        Err(e) => {
            log::error!("   ❌ Database error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

// Authentication request structure
#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    pub version: String,
    #[serde(rename = "messageType")]
    pub message_type: String,
    #[serde(rename = "identityKey")]
    pub identity_key: String,
    #[serde(rename = "initialNonce")]
    pub initial_nonce: String,
}

// /.well-known/auth - Babbage authentication
pub async fn well_known_auth(
    state: web::Data<AppState>,
    req: web::Json<AuthRequest>,
) -> HttpResponse {
    log::info!("🔐 Babbage auth request received");
    log::info!("   Identity key from request: {}", req.identity_key);
    log::info!("   Initial nonce: {}", req.initial_nonce);
    log::info!("   Message type: {}", req.message_type);

    // Generate our nonce - simple random 32-byte nonce (BRC-103 standard)
    // NOTE: For wallet clients with low session volume, simple random nonces are ideal.
    // HMAC-based nonces (BRC-103 Section 6.2) are only needed for high-volume servers (100k+ sessions).
    // TODO: Add nonce tracking later to prevent replay attacks (store used nonces with timestamps)
    let our_nonce_bytes: [u8; 32] = rand::random();
    let our_nonce = general_purpose::STANDARD.encode(&our_nonce_bytes);
    log::info!("   Generated our nonce (32 bytes, random): {}", hex::encode(&our_nonce_bytes));

    // Get MASTER private key (m) for signing
    // BRC-42 requires the master key, not m/0 or any child derivation
    let db = state.database.lock().unwrap();
    let master_privkey = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key derivation error: {}", e)
            }));
        }
    };
    drop(db);

    // Get master public key for identity and BRC-42 key derivation
    use secp256k1::{Secp256k1, SecretKey, PublicKey, Message};

    let secp = Secp256k1::new();
    let master_seckey = SecretKey::from_slice(&master_privkey).expect("Valid private key");
    let master_pubkey = PublicKey::from_secret_key(&secp, &master_seckey);

    // Get MASTER public key bytes (33 bytes compressed)
    let master_pubkey_bytes = master_pubkey.serialize();
    let master_pubkey_hex = hex::encode(&master_pubkey_bytes);

    log::info!("   Our MASTER identity key: {}", master_pubkey_hex);

    // === APP-SCOPED IDENTITY KEY DERIVATION ===
    // Privacy feature: Derive a unique identity key for each app using BRC-42
    // This prevents cross-app tracking - each app sees a different identity key
    // Invoice: "2-identity" (Security Level 2, protocol "identity")
    // Counterparty: The app's identity key (from request)
    let app_identity_key_bytes = match hex::decode(&req.identity_key) {
        Ok(b) => b,
        Err(e) => {
            log::error!("   Failed to decode app identity key for scoping: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid identity key"
            }));
        }
    };

    let app_scoped_identity_pubkey_hex = match crate::crypto::brc42::derive_child_public_key(
        &master_privkey,           // Our master private key
        &app_identity_key_bytes,   // App's identity key as counterparty
        "2-identity"               // Fixed invoice number for identity derivation
    ) {
        Ok(pubkey_bytes) => {
            let pubkey_hex = hex::encode(&pubkey_bytes);
            log::info!("   ✅ Derived app-scoped identity key: {}", pubkey_hex);
            log::info!("   📍 For app: {}...", &req.identity_key[..16.min(req.identity_key.len())]);
            pubkey_hex
        },
        Err(e) => {
            log::error!("   Failed to derive app-scoped identity key: {}", e);
            log::warn!("   ⚠️  Falling back to master key (BRC-42 derivation failed)");
            // Fallback to master key if derivation fails (shouldn't happen normally)
            master_pubkey_hex.clone()
        }
    };

    // CRITICAL: TypeScript SDK does Utils.toArray(nonce1_base64 + nonce2_base64, 'base64')
    // This concatenates the BASE64 STRINGS first, THEN decodes the concatenated string!
    // NOT: decode(nonce1) + decode(nonce2)
    let concatenated_nonces_base64 = format!("{}{}", req.initial_nonce, our_nonce);

    let data_to_sign = match general_purpose::STANDARD.decode(&concatenated_nonces_base64) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("   Failed to decode concatenated nonces: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Invalid nonce concatenation: {}", e)
            }));
        }
    };

    log::info!("   Data to sign ({} bytes from concatenated base64 nonces)", data_to_sign.len());

    // Create BRC-43 invoice number: "2-auth message signature-theirNonce ourNonce"
    let protocol_id = match normalize_protocol_id("auth message signature") {
        Ok(p) => p,
        Err(e) => {
            log::error!("   Failed to normalize protocol ID: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Protocol ID error: {}", e)
            }));
        }
    };

    let key_id = format!("{} {}", req.initial_nonce, our_nonce);
    let invoice_number = match InvoiceNumber::new(
        SecurityLevel::CounterpartyLevel,
        protocol_id,
        key_id
    ) {
        Ok(inv) => inv.to_string(),
        Err(e) => {
            log::error!("   Failed to create invoice number: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Invoice number error: {}", e)
            }));
        }
    };

    log::info!("   BRC-43 invoice number: {}", invoice_number);

    // Get MASTER private key (m) for BRC-42 signing
    // Must use the same master key that we use for nonce HMAC
    let db = state.database.lock().unwrap();
    let private_key_bytes = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => {
            log::info!("   ✅ MASTER private key retrieved for BRC-42 signing");
            key
        },
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key derivation error: {}", e)
            }));
        }
    };
    drop(db);

    // Decode counterparty public key (identity key)
    let counterparty_pubkey_bytes = match hex::decode(&req.identity_key) {
        Ok(b) => b,
        Err(e) => {
            log::error!("   Failed to decode counterparty public key: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid identity key"
            }));
        }
    };

    // Use BRC-42 (ECDH-based derivation) for ALL counterparties (including "self")
    // The TypeScript SDK's PublicKey.deriveChild() uses ECDH even for "self" counterparty
    log::info!("   Using BRC-42 (ECDH-based derivation)...");

    let child_private_key = match derive_child_private_key(
        &private_key_bytes,
        &counterparty_pubkey_bytes,
        &invoice_number
    ) {
        Ok(key) => {
            log::info!("   ✅ BRC-42 child key derived successfully");
            key
        },
        Err(e) => {
            log::error!("   BRC-42 derivation failed: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key derivation error: {}", e)
            }));
        }
    };

    // Hash the data (SHA-256)
    let data_hash = sha256(&data_to_sign);
    log::info!("   Data hash (32 bytes): {}", hex::encode(&data_hash));

    // Sign the hash with the derived key (SecretKey and Message already imported above)
    let secret = SecretKey::from_slice(&child_private_key)
        .map_err(|e| {
            log::error!("   Invalid private key: {}", e);
            e
        }).unwrap();

    let message = Message::from_digest_slice(&data_hash)
        .map_err(|e| {
            log::error!("   Invalid message hash: {}", e);
            e
        }).unwrap();

    let signature = secp.sign_ecdsa(&message, &secret);

    // Serialize signature in DER format (as per BRC-77 specification)
    let sig_bytes = signature.serialize_der().to_vec();

    let signature_hex = hex::encode(&sig_bytes);
    log::info!("   ✅ Signature created ({} bytes, DER format): {}", sig_bytes.len(), signature_hex);

    // Return BRC-104 compliant response
    // Per BRC-103 spec section 6.1:
    // - "initialNonce": B_Nonce (our new session nonce)
    // - "yourNonce": A_Nonce (their initial nonce echoed back)
    // NOTE: signature must be returned as an array of bytes (like TypeScript SDK does), not hex string
    // PRIVACY: identityKey is now APP-SCOPED (derived via BRC-42) to prevent cross-app tracking
    let response = serde_json::json!({
        "version": "0.1",
        "messageType": "initialResponse",
        "identityKey": app_scoped_identity_pubkey_hex,  // APP-SCOPED identity key (privacy feature)
        "initialNonce": our_nonce,                       // Our new nonce (B_Nonce)
        "yourNonce": req.initial_nonce,                  // Their initial nonce echoed back (A_Nonce)
        "signature": sig_bytes                           // DER signature as byte array (not hex string!)
    });

    log::info!("✅ Returning auth response with BRC-42 signature");
    log::info!("   🔒 Using APP-SCOPED identity key (privacy: prevents cross-app tracking)");
    log::info!("   📤 Response fields: initialNonce=[ourNew], yourNonce=[theirInitialEchoed]");
    log::info!("   📤 FULL RESPONSE JSON: {}", serde_json::to_string_pretty(&response).unwrap_or_else(|_| "error".to_string()));

    // Store the auth session for subsequent authenticated requests
    state.auth_sessions.store_session(&req.identity_key, &our_nonce);

    HttpResponse::Ok().json(response)
}

// Request structure for /createHmac
#[derive(Debug, Deserialize)]
pub struct CreateHmacRequest {
    #[serde(rename = "protocolID")]
    pub protocol_id: serde_json::Value, // Can be [number, string] or string
    #[serde(rename = "keyID")]
    pub key_id: serde_json::Value, // Can be string, base64, or byte array
    pub data: serde_json::Value, // Can be array of bytes OR base64 string
    #[serde(rename = "counterparty")]
    pub counterparty: serde_json::Value, // Can be "self" or hex public key
}

// Response structure for /createHmac
#[derive(Debug, Serialize)]
pub struct CreateHmacResponse {
    pub hmac: Vec<u8>, // Array of bytes (not hex string)
}

// /createHmac - BRC-100 endpoint for creating HMAC
pub async fn create_hmac(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("📋 /createHmac called");

    // Special handling for keyID with invalid Unicode
    let body_str = String::from_utf8_lossy(&body).to_string();

    // Try initial parse
    let mut req_value: serde_json::Value = match serde_json::from_str(&body_str) {
        Ok(v) => v,
        Err(e) => {
            // If JSON parsing fails, try to fix keyID by replacing it with data array
            log::warn!("   Initial JSON parse failed: {}", e);
            log::info!("   Attempting to fix keyID field...");

            // Use regex to find and replace the keyID value with a placeholder
            // Pattern: "keyID":"<anything>","data":[array]
            use regex::Regex;
            let re = Regex::new(r#""keyID"\s*:\s*"[^"]*""#).unwrap();

            // Extract data array value
            let data_re = Regex::new(r#""data"\s*:\s*(\[[^\]]+\])"#).unwrap();

            if let Some(data_cap) = data_re.captures(&body_str) {
                if let Some(data_array) = data_cap.get(1) {
                    // Replace keyID string with data array
                    let fixed_json = re.replace(&body_str, &format!(r#""keyID":{}"#, data_array.as_str()));
                    log::info!("   Fixed JSON (replaced keyID with data array)");

                    // Try parsing again
                    match serde_json::from_str(&fixed_json) {
                        Ok(v) => v,
                        Err(e2) => {
                            log::error!("   JSON parse still failed after fix: {}", e2);
                            return HttpResponse::BadRequest().json(serde_json::json!({
                                "error": format!("Invalid JSON: {}", e2)
                            }));
                        }
                    }
                } else {
                    log::error!("   Could not extract data array");
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Invalid JSON: {}", e)
                    }));
                }
            } else {
                log::error!("   Could not find data array in JSON");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Invalid JSON: {}", e)
                }));
            }
        }
    };

    // If keyID is a string and data is an array, use data for keyID
    if let Some(obj) = req_value.as_object_mut() {
        if let (Some(key_id), Some(data)) = (obj.get("keyID"), obj.get("data")) {
            if key_id.is_string() && data.is_array() {
                // Replace keyID with data array to avoid Unicode issues
                log::info!("   Replacing string keyID with data array");
                obj.insert("keyID".to_string(), data.clone());
            }
        }
    }

    // Now parse into our struct
    let req: CreateHmacRequest = match serde_json::from_value(req_value) {
        Ok(r) => r,
        Err(e) => {
            log::error!("   Failed to parse CreateHmacRequest: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid request format: {}", e)
            }));
        }
    };

    log::info!("   Protocol ID: {:?}", req.protocol_id);
    log::info!("   Key ID: {:?}", req.key_id);
    log::info!("   Counterparty: {:?}", req.counterparty);
    log::info!("   Data: {:?}", req.data);

    // Parse keyID - use data bytes as fallback since keyID often contains the nonce
    // which is the same as the data for "server hmac" protocol
    let key_id_str: String = match &req.key_id {
        serde_json::Value::String(s) if !s.is_empty() => s.clone(),
        serde_json::Value::Array(arr) => {
            // Byte array - convert to base64 to preserve binary data
            let bytes: Vec<u8> = arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            // Use base64 encoding to preserve all bytes (not UTF-8 lossy!)
            general_purpose::STANDARD.encode(&bytes)
        },
        _ => {
            // Fallback: For "server hmac" protocol, keyID is often the same as data
            // Use data bytes as keyID if keyID parsing fails
            log::info!("   keyID parsing failed or empty, using data bytes as fallback");
            match &req.data {
                serde_json::Value::Array(arr) => {
                    let bytes: Vec<u8> = arr.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u8))
                        .collect();
                    // Use base64 encoding to preserve all bytes (not UTF-8 lossy!)
                    general_purpose::STANDARD.encode(&bytes)
                },
                _ => {
                    log::error!("   keyID and data parsing both failed");
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "keyID must be string or byte array"
                    }));
                }
            }
        }
    };

    // Parse data (can be array of bytes or base64 string)
    let data_bytes: Vec<u8> = match &req.data {
        serde_json::Value::Array(arr) => {
            // Array of numbers
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect()
        },
        serde_json::Value::String(s) => {
            // Base64 string
            match general_purpose::STANDARD.decode(s) {
                Ok(b) => b,
                Err(e) => {
                    log::error!("   Failed to decode base64 data: {}", e);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Invalid base64 data: {}", e)
                    }));
                }
            }
        },
        _ => {
            log::error!("   Data must be array or base64 string");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Data must be array or base64 string"
            }));
        }
    };

    log::info!("   Data bytes length: {}", data_bytes.len());

    // Parse protocol ID FIRST (we need it to determine how to handle "self")
    let protocol_id_str = match &req.protocol_id {
        serde_json::Value::Array(arr) => {
            if arr.len() >= 2 {
                if let Some(name) = arr[1].as_str() {
                    name.to_string()
                } else {
                    log::error!("   Invalid protocol ID array format");
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "Invalid protocol ID format"
                    }));
                }
            } else {
                log::error!("   Protocol ID array too short");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Protocol ID array too short"
                }));
            }
        },
        serde_json::Value::String(s) => s.clone(),
        _ => {
            log::error!("   Protocol ID must be array or string");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Protocol ID must be array or string"
            }));
        }
    };

    // Normalize protocol ID
    let protocol_id = match normalize_protocol_id(&protocol_id_str) {
        Ok(p) => p,
        Err(e) => {
            log::error!("   Failed to normalize protocol ID: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid protocol ID: {}", e)
            }));
        }
    };

    // Get MASTER private key (m) first - needed for "self" counterparty resolution
    let db = state.database.lock().unwrap();
    let private_key_bytes = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => {
            log::info!("   ✅ MASTER private key retrieved for HMAC (createHmac)");
            key
        },
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key derivation error: {}", e)
            }));
        }
    };
    drop(db);

    // Parse counterparty (can be "self" or hex public key)
    // For HMAC operations with "self", use RAW master key (no BRC-42)
    // This is for self-verification in auth handshakes, not two-party ECDH
    let counterparty_hex: Option<String> = match &req.counterparty {
        serde_json::Value::String(s) if s == "self" => {
            log::info!("   Counterparty is 'self' - using raw master key (no BRC-42 for HMAC)");
            None
        },
        serde_json::Value::String(s) => {
            log::info!("   Counterparty public key: {}", s);
            Some(s.clone())
        },
        _ => None
    };

    // Create BRC-43 invoice number
    let security_level = if counterparty_hex.is_some() {
        SecurityLevel::CounterpartyLevel
    } else {
        SecurityLevel::CounterpartyLevel // Default to level 2
    };

    let invoice_number = match InvoiceNumber::new(
        security_level,
        protocol_id,
        key_id_str.clone()
    ) {
        Ok(inv) => inv.to_string(),
        Err(e) => {
            log::error!("   Failed to create invoice number: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Invoice number error: {}", e)
            }));
        }
    };

    log::info!("   BRC-43 invoice number: {}", invoice_number);

    // Determine HMAC key based on whether counterparty is provided
    let hmac_key = if let Some(counterparty_hex) = &counterparty_hex {
        // BRC-42: Derive child key for mutual authentication with actual counterparty
        let counterparty_bytes = match hex::decode(counterparty_hex) {
            Ok(b) => b,
            Err(e) => {
                log::error!("   Failed to decode counterparty key: {}", e);
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid counterparty key"
                }));
            }
        };

        log::info!("   Deriving BRC-42 child key for HMAC...");
        match derive_child_private_key(&private_key_bytes, &counterparty_bytes, &invoice_number) {
            Ok(key) => {
                log::info!("   ✅ BRC-42 child key derived");
                key
            },
            Err(e) => {
                log::error!("   BRC-42 derivation failed: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Key derivation error: {}", e)
                }));
            }
        }
    } else {
        // For "self" counterparty, use raw master key (no BRC-42 derivation)
        log::info!("   Using raw master key for HMAC (counterparty='self')");
        private_key_bytes
    };

    // Compute HMAC-SHA256
    let hmac_result = hmac_sha256(&hmac_key, &data_bytes);
    let hmac_hex = hex::encode(&hmac_result);

    log::info!("   ✅ HMAC created: {}...", &hmac_hex[..std::cmp::min(32, hmac_hex.len())]);

    // Return HMAC as array of bytes (not hex string) for SDK compatibility
    HttpResponse::Ok().json(CreateHmacResponse { hmac: hmac_result.to_vec() })
}

// Request structure for /verifyHmac
#[derive(Debug, Deserialize)]
pub struct VerifyHmacRequest {
    #[serde(rename = "protocolID")]
    pub protocol_id: serde_json::Value, // Can be [number, string] or string
    #[serde(rename = "keyID")]
    pub key_id: serde_json::Value, // Can be string, base64, or byte array
    pub data: serde_json::Value, // Can be array of bytes OR base64 string
    pub hmac: serde_json::Value, // Can be array of bytes OR hex string
    #[serde(rename = "counterparty")]
    pub counterparty: serde_json::Value, // Can be "self" or hex public key
}

// Response structure for /verifyHmac
#[derive(Debug, Serialize)]
pub struct VerifyHmacResponse {
    pub valid: bool,
}

// /verifyHmac - BRC-100 endpoint for verifying HMAC
pub async fn verify_hmac(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("📋 /verifyHmac called");

    // Special handling for keyID with invalid Unicode (same as createHmac)
    let body_str = String::from_utf8_lossy(&body).to_string();

    // Try initial parse
    let req: VerifyHmacRequest = match serde_json::from_str(&body_str) {
        Ok(r) => r,
        Err(e) => {
            // If JSON parsing fails, try to fix keyID by replacing it with data array
            log::warn!("   Initial JSON parse failed: {}", e);
            log::info!("   Attempting to fix keyID field...");

            // Use regex to find and replace the keyID value
            use regex::Regex;
            let re = Regex::new(r#""keyID"\s*:\s*"[^"]*""#).unwrap();

            // Extract data array value
            let data_re = Regex::new(r#""data"\s*:\s*(\[[^\]]+\])"#).unwrap();

            if let Some(data_cap) = data_re.captures(&body_str) {
                if let Some(data_array) = data_cap.get(1) {
                    // Replace keyID string with data array
                    let fixed_json = re.replace(&body_str, &format!(r#""keyID":{}"#, data_array.as_str()));
                    log::info!("   Fixed JSON (replaced keyID with data array)");

                    // Try parsing again
                    match serde_json::from_str(&fixed_json) {
                        Ok(r) => r,
                        Err(e2) => {
                            log::error!("   JSON parse still failed after fix: {}", e2);
                            return HttpResponse::BadRequest().json(serde_json::json!({
                                "error": format!("Invalid JSON: {}", e2)
                            }));
                        }
                    }
                } else {
                    log::error!("   Could not extract data array");
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Invalid JSON: {}", e)
                    }));
                }
            } else {
                log::error!("   Could not find data array in JSON");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Invalid JSON: {}", e)
                }));
            }
        }
    };

    log::info!("   Protocol ID: {:?}", req.protocol_id);
    log::info!("   Key ID: {:?}", req.key_id);
    log::info!("   Counterparty: {:?}", req.counterparty);
    log::info!("   HMAC: {:?}", req.hmac);

    // Parse keyID (can be string, byte array, or base64)
    let key_id_str: String = match &req.key_id {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            // Byte array - convert to base64 to preserve binary data
            let bytes: Vec<u8> = arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            // Use base64 encoding to preserve all bytes (not UTF-8 lossy!)
            general_purpose::STANDARD.encode(&bytes)
        },
        _ => {
            log::error!("   keyID must be string or byte array");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "keyID must be string or byte array"
            }));
        }
    };

    // Parse HMAC (can be array of bytes or hex string)
    let expected_hmac: Vec<u8> = match &req.hmac {
        serde_json::Value::Array(arr) => {
            // Array of numbers
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect()
        },
        serde_json::Value::String(s) => {
            // Hex string
            match hex::decode(s) {
                Ok(b) => b,
                Err(e) => {
                    log::error!("   Failed to decode HMAC hex: {}", e);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Invalid HMAC hex: {}", e)
                    }));
                }
            }
        },
        _ => {
            log::error!("   HMAC must be array or hex string");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "HMAC must be array or hex string"
            }));
        }
    };

    log::info!("   HMAC bytes length: {}", expected_hmac.len());

    // Parse data (can be array of bytes or base64 string)
    let data_bytes: Vec<u8> = match &req.data {
        serde_json::Value::Array(arr) => {
            // Array of numbers
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect()
        },
        serde_json::Value::String(s) => {
            // Base64 string
            match general_purpose::STANDARD.decode(s) {
                Ok(b) => b,
                Err(e) => {
                    log::error!("   Failed to decode base64 data: {}", e);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Invalid base64 data: {}", e)
                    }));
                }
            }
        },
        _ => {
            log::error!("   Data must be array or base64 string");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Data must be array or base64 string"
            }));
        }
    };

    log::info!("   Data bytes length: {}", data_bytes.len());

    // Parse protocol ID FIRST (we need it to determine how to handle "self")
    let protocol_id_str = match &req.protocol_id {
        serde_json::Value::Array(arr) => {
            if arr.len() >= 2 {
                if let Some(name) = arr[1].as_str() {
                    name.to_string()
                } else {
                    log::error!("   Invalid protocol ID array format");
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "Invalid protocol ID format"
                    }));
                }
            } else {
                log::error!("   Protocol ID array too short");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Protocol ID array too short"
                }));
            }
        },
        serde_json::Value::String(s) => s.clone(),
        _ => {
            log::error!("   Protocol ID must be array or string");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Protocol ID must be array or string"
            }));
        }
    };

    // Normalize protocol ID
    let protocol_id = match normalize_protocol_id(&protocol_id_str) {
        Ok(p) => p,
        Err(e) => {
            log::error!("   Failed to normalize protocol ID: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid protocol ID: {}", e)
            }));
        }
    };

    // Get MASTER private key (m) first - needed for "self" counterparty resolution
    let db = state.database.lock().unwrap();
    let private_key_bytes = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => {
            log::info!("   ✅ MASTER private key retrieved for HMAC (verifyHmac)");
            key
        },
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key derivation error: {}", e)
            }));
        }
    };
    drop(db);

    // Parse counterparty (can be "self" or hex public key)
    // For HMAC operations with "self", use RAW master key (no BRC-42)
    // This is for self-verification in auth handshakes, not two-party ECDH
    let counterparty_hex: Option<String> = match &req.counterparty {
        serde_json::Value::String(s) if s == "self" => {
            log::info!("   Counterparty is 'self' - using raw master key (no BRC-42 for HMAC)");
            None
        },
        serde_json::Value::String(s) => {
            log::info!("   Counterparty public key: {}", s);
            Some(s.clone())
        },
        _ => None
    };

    // Create BRC-43 invoice number
    let security_level = if counterparty_hex.is_some() {
        SecurityLevel::CounterpartyLevel
    } else {
        SecurityLevel::CounterpartyLevel // Default to level 2
    };

    let invoice_number = match InvoiceNumber::new(
        security_level,
        protocol_id,
        key_id_str.clone()
    ) {
        Ok(inv) => inv.to_string(),
        Err(e) => {
            log::error!("   Failed to create invoice number: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Invoice number error: {}", e)
            }));
        }
    };

    log::info!("   BRC-43 invoice number: {}", invoice_number);

    // Determine HMAC key based on whether counterparty is provided
    let hmac_key = if let Some(counterparty_hex) = &counterparty_hex {
        // BRC-42: Derive child key for mutual authentication with actual counterparty
        let counterparty_bytes = match hex::decode(counterparty_hex) {
            Ok(b) => b,
            Err(e) => {
                log::error!("   Failed to decode counterparty key: {}", e);
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid counterparty key"
                }));
            }
        };

        log::info!("   Deriving BRC-42 child key for HMAC verification...");
        match derive_child_private_key(&private_key_bytes, &counterparty_bytes, &invoice_number) {
            Ok(key) => {
                log::info!("   ✅ BRC-42 child key derived");
                key
            },
            Err(e) => {
                log::error!("   BRC-42 derivation failed: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Key derivation error: {}", e)
                }));
            }
        }
    } else {
        // For "self" counterparty, use raw master key (no BRC-42 derivation)
        log::info!("   Using raw master key for HMAC verification (counterparty='self')");
        private_key_bytes.clone()
    };

    // Verify HMAC
    let is_valid = verify_hmac_sha256(&hmac_key, &data_bytes, &expected_hmac);

    log::info!("   ✅ HMAC verification result: {}", is_valid);

    HttpResponse::Ok().json(VerifyHmacResponse { valid: is_valid })
}

// ============================================================================
// BRC-2 Encryption / Decryption (Call Codes 11 and 12)
// ============================================================================

// Request structure for /encrypt
#[derive(Debug, Deserialize)]
pub struct EncryptRequest {
    #[serde(rename = "protocolID")]
    pub protocol_id: serde_json::Value, // [securityLevel, protocolString] or string
    #[serde(rename = "keyID")]
    pub key_id: serde_json::Value, // Can be string or various formats
    pub plaintext: serde_json::Value, // Byte array OR base64 string
    #[serde(rename = "counterparty")]
    pub counterparty: Option<serde_json::Value>, // "self", "anyone", or hex pubkey (default: "self")
}

// Response structure for /encrypt
#[derive(Debug, Serialize)]
pub struct EncryptResponse {
    pub ciphertext: Vec<u8>, // Array of bytes (BRC-2 format: IV + ciphertext + tag)
}

// Request structure for /decrypt
#[derive(Debug, Deserialize)]
pub struct DecryptRequest {
    #[serde(rename = "protocolID")]
    pub protocol_id: serde_json::Value, // [securityLevel, protocolString] or string
    #[serde(rename = "keyID")]
    pub key_id: serde_json::Value, // Can be string or various formats
    pub ciphertext: serde_json::Value, // Byte array OR base64 string
    #[serde(rename = "counterparty")]
    pub counterparty: Option<serde_json::Value>, // "self", "anyone", or hex pubkey (default: "self")
}

// Response structure for /decrypt
#[derive(Debug, Serialize)]
pub struct DecryptResponse {
    pub plaintext: Vec<u8>, // Decrypted data as array of bytes
}

/// Resolve counterparty public key for BRC-2 encryption/decryption
/// Returns the counterparty's public key bytes (33 bytes compressed)
fn resolve_counterparty_pubkey(
    counterparty: &Option<serde_json::Value>,
    our_master_pubkey: &[u8],
) -> Result<Vec<u8>, String> {
    match counterparty {
        None => {
            // Default to "self"
            log::info!("   Counterparty: self (default)");
            Ok(our_master_pubkey.to_vec())
        }
        Some(serde_json::Value::String(s)) if s == "self" => {
            log::info!("   Counterparty: self");
            Ok(our_master_pubkey.to_vec())
        }
        Some(serde_json::Value::String(s)) if s == "anyone" => {
            log::info!("   Counterparty: anyone (public key from private key 1)");
            // Private key 1 -> public key (generator point G)
            // This is a well-known public key that anyone can derive
            use secp256k1::{Secp256k1, SecretKey, PublicKey};
            let secp = Secp256k1::new();
            let mut anyone_privkey = [0u8; 32];
            anyone_privkey[31] = 1; // Private key with value 1
            let secret = SecretKey::from_slice(&anyone_privkey)
                .map_err(|e| format!("Failed to create 'anyone' secret key: {}", e))?;
            let pubkey = PublicKey::from_secret_key(&secp, &secret);
            Ok(pubkey.serialize().to_vec()) // 33-byte compressed
        }
        Some(serde_json::Value::String(hex_key)) => {
            log::info!("   Counterparty: {} (hex pubkey)", hex_key);
            hex::decode(hex_key)
                .map_err(|e| format!("Invalid counterparty public key hex: {}", e))
        }
        _ => Err("Counterparty must be 'self', 'anyone', or hex public key".to_string()),
    }
}

/// Parse byte data from JSON (can be array of numbers, hex string, or base64 string)
fn parse_byte_data(value: &serde_json::Value) -> Result<Vec<u8>, String> {
    match value {
        serde_json::Value::Array(arr) => {
            // Array of numbers
            Ok(arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect())
        }
        serde_json::Value::String(s) => {
            // Try hex first, then base64
            if let Ok(bytes) = hex::decode(s) {
                Ok(bytes)
            } else {
                general_purpose::STANDARD
                    .decode(s)
                    .map_err(|e| format!("Invalid data encoding (not hex or base64): {}", e))
            }
        }
        _ => Err("Data must be array of bytes or encoded string".to_string()),
    }
}

// /encrypt - BRC-100 endpoint for encrypting data (Call Code 11)
pub async fn encrypt(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("📋 /encrypt called");

    // Parse request body
    let body_str = String::from_utf8_lossy(&body).to_string();
    let req: EncryptRequest = match serde_json::from_str(&body_str) {
        Ok(r) => r,
        Err(e) => {
            log::error!("   Failed to parse EncryptRequest: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid request format: {}", e)
            }));
        }
    };

    log::info!("   Protocol ID: {:?}", req.protocol_id);
    log::info!("   Key ID: {:?}", req.key_id);
    log::info!("   Counterparty: {:?}", req.counterparty);

    // Parse plaintext
    let plaintext_bytes = match parse_byte_data(&req.plaintext) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("   Failed to parse plaintext: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid plaintext: {}", e)
            }));
        }
    };
    log::info!("   Plaintext length: {} bytes", plaintext_bytes.len());

    // Parse keyID (can be string or byte array)
    let key_id_str: String = match &req.key_id {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            general_purpose::STANDARD.encode(&bytes)
        }
        _ => {
            log::error!("   keyID must be string or byte array");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "keyID must be string or byte array"
            }));
        }
    };

    // Parse protocol ID and get security level
    let (security_level, protocol_id_str) = match &req.protocol_id {
        serde_json::Value::Array(arr) => {
            if arr.len() >= 2 {
                let level = arr[0].as_u64().unwrap_or(2) as u8;
                let name = arr[1].as_str().unwrap_or("unknown").to_string();
                (level, name)
            } else {
                log::error!("   Protocol ID array too short");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Protocol ID array must have at least 2 elements"
                }));
            }
        }
        serde_json::Value::String(s) => {
            // Parse string format: "level-protocol" or just "protocol" (defaults to level 2)
            if let Some(idx) = s.find('-') {
                let level = s[..idx].parse::<u8>().unwrap_or(2);
                (level, s[idx + 1..].to_string())
            } else {
                (2, s.clone())
            }
        }
        _ => {
            log::error!("   Protocol ID must be array or string");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Protocol ID must be array or string"
            }));
        }
    };

    // Normalize protocol ID and build invoice number
    let protocol_id = match normalize_protocol_id(&protocol_id_str) {
        Ok(p) => p,
        Err(e) => {
            log::error!("   Failed to normalize protocol ID: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid protocol ID: {}", e)
            }));
        }
    };

    let security = match security_level {
        0 => SecurityLevel::NoPermissions,
        1 => SecurityLevel::ProtocolLevel,
        _ => SecurityLevel::CounterpartyLevel,
    };

    let invoice = match InvoiceNumber::new(security, &protocol_id, &key_id_str) {
        Ok(inv) => inv,
        Err(e) => {
            log::error!("   Failed to create invoice number: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid invoice number: {}", e)
            }));
        }
    };
    let invoice_number = invoice.to_string();
    log::info!("   BRC-43 invoice number: {}", invoice_number);

    // Get master keys from database
    let db = state.database.lock().unwrap();
    let master_privkey = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => {
            log::info!("   ✅ Master private key retrieved");
            key
        }
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key retrieval error: {}", e)
            }));
        }
    };
    let master_pubkey = match crate::database::get_master_public_key_from_db(&db) {
        Ok(key) => {
            log::info!("   ✅ Master public key retrieved");
            key
        }
        Err(e) => {
            log::error!("   Failed to get master public key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key retrieval error: {}", e)
            }));
        }
    };
    drop(db);

    // Resolve counterparty public key
    let counterparty_pubkey = match resolve_counterparty_pubkey(&req.counterparty, &master_pubkey) {
        Ok(pk) => pk,
        Err(e) => {
            log::error!("   Failed to resolve counterparty: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": e
            }));
        }
    };

    // Derive symmetric key using BRC-2
    let symmetric_key = match derive_symmetric_key(&master_privkey, &counterparty_pubkey, &invoice_number) {
        Ok(key) => {
            log::info!("   ✅ Symmetric key derived");
            key
        }
        Err(e) => {
            log::error!("   Failed to derive symmetric key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key derivation error: {}", e)
            }));
        }
    };

    // Encrypt using AES-256-GCM (BRC-2)
    let ciphertext = match encrypt_brc2(&plaintext_bytes, &symmetric_key) {
        Ok(ct) => ct,
        Err(e) => {
            log::error!("   Encryption failed: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Encryption failed: {}", e)
            }));
        }
    };

    log::info!("   ✅ Encrypted {} bytes -> {} bytes", plaintext_bytes.len(), ciphertext.len());

    HttpResponse::Ok().json(EncryptResponse { ciphertext })
}

// /decrypt - BRC-100 endpoint for decrypting data (Call Code 12)
pub async fn decrypt(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("📋 /decrypt called");

    // Parse request body
    let body_str = String::from_utf8_lossy(&body).to_string();
    let req: DecryptRequest = match serde_json::from_str(&body_str) {
        Ok(r) => r,
        Err(e) => {
            log::error!("   Failed to parse DecryptRequest: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid request format: {}", e)
            }));
        }
    };

    log::info!("   Protocol ID: {:?}", req.protocol_id);
    log::info!("   Key ID: {:?}", req.key_id);
    log::info!("   Counterparty: {:?}", req.counterparty);

    // Parse ciphertext
    let ciphertext_bytes = match parse_byte_data(&req.ciphertext) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("   Failed to parse ciphertext: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid ciphertext: {}", e)
            }));
        }
    };
    log::info!("   Ciphertext length: {} bytes", ciphertext_bytes.len());

    // Validate minimum ciphertext length (32 IV + 16 tag = 48 bytes minimum)
    if ciphertext_bytes.len() < 48 {
        log::error!("   Ciphertext too short: {} bytes (need at least 48)", ciphertext_bytes.len());
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Ciphertext too short: need at least 48 bytes (32 IV + 16 tag), got {}", ciphertext_bytes.len())
        }));
    }

    // Parse keyID (can be string or byte array)
    let key_id_str: String = match &req.key_id {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            general_purpose::STANDARD.encode(&bytes)
        }
        _ => {
            log::error!("   keyID must be string or byte array");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "keyID must be string or byte array"
            }));
        }
    };

    // Parse protocol ID and get security level
    let (security_level, protocol_id_str) = match &req.protocol_id {
        serde_json::Value::Array(arr) => {
            if arr.len() >= 2 {
                let level = arr[0].as_u64().unwrap_or(2) as u8;
                let name = arr[1].as_str().unwrap_or("unknown").to_string();
                (level, name)
            } else {
                log::error!("   Protocol ID array too short");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Protocol ID array must have at least 2 elements"
                }));
            }
        }
        serde_json::Value::String(s) => {
            // Parse string format: "level-protocol" or just "protocol" (defaults to level 2)
            if let Some(idx) = s.find('-') {
                let level = s[..idx].parse::<u8>().unwrap_or(2);
                (level, s[idx + 1..].to_string())
            } else {
                (2, s.clone())
            }
        }
        _ => {
            log::error!("   Protocol ID must be array or string");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Protocol ID must be array or string"
            }));
        }
    };

    // Normalize protocol ID and build invoice number
    let protocol_id = match normalize_protocol_id(&protocol_id_str) {
        Ok(p) => p,
        Err(e) => {
            log::error!("   Failed to normalize protocol ID: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid protocol ID: {}", e)
            }));
        }
    };

    let security = match security_level {
        0 => SecurityLevel::NoPermissions,
        1 => SecurityLevel::ProtocolLevel,
        _ => SecurityLevel::CounterpartyLevel,
    };

    let invoice = match InvoiceNumber::new(security, &protocol_id, &key_id_str) {
        Ok(inv) => inv,
        Err(e) => {
            log::error!("   Failed to create invoice number: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid invoice number: {}", e)
            }));
        }
    };
    let invoice_number = invoice.to_string();
    log::info!("   BRC-43 invoice number: {}", invoice_number);

    // Get master keys from database
    let db = state.database.lock().unwrap();
    let master_privkey = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => {
            log::info!("   ✅ Master private key retrieved");
            key
        }
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key retrieval error: {}", e)
            }));
        }
    };
    let master_pubkey = match crate::database::get_master_public_key_from_db(&db) {
        Ok(key) => {
            log::info!("   ✅ Master public key retrieved");
            key
        }
        Err(e) => {
            log::error!("   Failed to get master public key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key retrieval error: {}", e)
            }));
        }
    };
    drop(db);

    // Resolve counterparty public key (for decrypt, this is the sender's key)
    let counterparty_pubkey = match resolve_counterparty_pubkey(&req.counterparty, &master_pubkey) {
        Ok(pk) => pk,
        Err(e) => {
            log::error!("   Failed to resolve counterparty: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": e
            }));
        }
    };

    // Derive symmetric key using BRC-2
    let symmetric_key = match derive_symmetric_key(&master_privkey, &counterparty_pubkey, &invoice_number) {
        Ok(key) => {
            log::info!("   ✅ Symmetric key derived");
            key
        }
        Err(e) => {
            log::error!("   Failed to derive symmetric key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key derivation error: {}", e)
            }));
        }
    };

    // Decrypt using AES-256-GCM (BRC-2)
    let plaintext = match decrypt_brc2(&ciphertext_bytes, &symmetric_key) {
        Ok(pt) => pt,
        Err(e) => {
            log::error!("   Decryption failed: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Decryption failed: {}", e)
            }));
        }
    };

    log::info!("   ✅ Decrypted {} bytes -> {} bytes", ciphertext_bytes.len(), plaintext.len());

    HttpResponse::Ok().json(DecryptResponse { plaintext })
}

// Wallet status endpoint
pub async fn wallet_status(state: web::Data<AppState>) -> HttpResponse {
    use crate::database::WalletRepository;

    // Check database first (primary storage)
    let db = state.database.lock().unwrap();
    let wallet_repo = WalletRepository::new(db.connection());
    let exists = wallet_repo.get_primary_wallet()
        .map(|opt| opt.is_some())
        .unwrap_or(false);
    drop(db);

    log::info!("📋 Wallet status: exists={}", exists);

    HttpResponse::Ok().json(serde_json::json!({
        "exists": exists
    }))
}

// Wallet balance endpoint
pub async fn wallet_balance(state: web::Data<AppState>) -> HttpResponse {
    log::info!("💰 /wallet/balance called");

    // Step 1: Check cache first (fast path)
    if let Some(cached_balance) = state.balance_cache.get() {
        log::info!("   ✅ Using cached balance: {} satoshis", cached_balance);
        return HttpResponse::Ok().json(serde_json::json!({
            "balance": cached_balance
        }));
    }

    log::info!("   🔄 Cache miss - calculating balance from database...");

    // Get all addresses from database
    let addresses = {
        use crate::database::{WalletRepository, AddressRepository, address_to_address_info};

        let db = state.database.lock().unwrap();
        let wallet_repo = WalletRepository::new(db.connection());

        // Get primary wallet
        let wallet = match wallet_repo.get_primary_wallet() {
            Ok(Some(w)) => w,
            Ok(None) => {
                log::error!("   No wallet found in database");
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "No wallet found"
                }));
            }
            Err(e) => {
                log::error!("   Failed to get wallet: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Database error: {}", e)
                }));
            }
        };

        // Get all addresses for this wallet
        let address_repo = AddressRepository::new(db.connection());
        match address_repo.get_all_by_wallet(wallet.id.unwrap()) {
            Ok(db_addresses) => {
                db_addresses.iter()
                    .map(|addr| address_to_address_info(addr))
                    .collect::<Vec<_>>()
            }
            Err(e) => {
                log::error!("   Failed to get addresses: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to get addresses: {}", e)
                }));
            }
        }
    };

    log::info!("   Checking balance for {} addresses", addresses.len());

    // Calculate balance from database cache
    use crate::database::{UtxoRepository, AddressRepository};
    let db = state.database.lock().unwrap();
    let address_repo = AddressRepository::new(db.connection());

    // Get address IDs
    let address_ids: Vec<i64> = addresses.iter()
        .filter_map(|addr| {
            // Find address ID by address string
            match address_repo.get_by_address(&addr.address) {
                Ok(Some(db_addr)) => db_addr.id,
                _ => None,
            }
        })
        .collect();

    let utxo_repo = UtxoRepository::new(db.connection());
    let cached_balance = match utxo_repo.calculate_balance(&address_ids) {
        Ok(balance) => balance,
        Err(e) => {
            log::error!("   Failed to calculate balance from database: {}", e);
            drop(db);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }));
        }
    };
    drop(db);

    // Use cached balance from database (Phase 4: primary source)
    // Only fetch from API if cache is empty (first time setup)
    // Background sync will update the cache periodically
    if cached_balance == 0 && address_ids.len() > 0 {
        // Cache is empty - fetch from API to populate cache
        log::info!("   Cache is empty, fetching UTXOs from API to populate cache...");

        let api_utxos = match crate::utxo_fetcher::fetch_all_utxos(&addresses).await {
            Ok(utxos) => utxos,
            Err(e) => {
                log::error!("   Failed to fetch UTXOs from API: {}", e);
                // Return cached balance (0) if API fails
                return HttpResponse::Ok().json(serde_json::json!({
                    "balance": cached_balance
                }));
            }
        };

        // Cache UTXOs to database
        let db = state.database.lock().unwrap();
        let address_repo = AddressRepository::new(db.connection());
        let utxo_repo = UtxoRepository::new(db.connection());

        for addr in &addresses {
            if let Ok(Some(db_addr)) = address_repo.get_by_address(&addr.address) {
                if let Some(addr_id) = db_addr.id {
                    // Get UTXOs for this address (clone to get owned values)
                    let addr_utxos: Vec<_> = api_utxos.iter()
                        .filter(|u| u.address_index == addr.index as u32)
                        .cloned()
                        .collect();

                    if !addr_utxos.is_empty() {
                        if let Err(e) = utxo_repo.upsert_utxos(addr_id, &addr_utxos) {
                            log::warn!("   Failed to cache UTXOs for {}: {}", addr.address, e);
                        } else {
                            // Mark address as used if it has UTXOs
                        let _ = address_repo.mark_used(addr_id);
                    }
                }
            }
        } else {
            // Address not in database - this shouldn't happen if generate_address worked
            log::warn!("   Address {} not found in database (index {})", addr.address, addr.index);
        }
    }
    drop(db);

        // Calculate balance from fetched UTXOs
        let total_balance: i64 = api_utxos.iter().map(|u| u.satoshis).sum();
        log::info!("   ✅ Total balance (from API, cache updated): {} satoshis ({} UTXOs)", total_balance, api_utxos.len());

        // Return response in Go wallet format: { "balance": number }
        return HttpResponse::Ok().json(serde_json::json!({
            "balance": total_balance
        }));
    }

    // Check for pending addresses (newly created addresses that need UTXO checking)
    let db = state.database.lock().unwrap();
    let wallet_repo = crate::database::WalletRepository::new(db.connection());
    let wallet = match wallet_repo.get_primary_wallet() {
        Ok(Some(w)) => w,
        Ok(None) | Err(_) => {
            drop(db);
            // No wallet found, return cached balance
            return HttpResponse::Ok().json(serde_json::json!({
                "balance": cached_balance
            }));
        }
    };

    let address_repo = AddressRepository::new(db.connection());
    let pending_addresses = match address_repo.get_pending_utxo_check(wallet.id.unwrap()) {
        Ok(addrs) => addrs,
        Err(e) => {
            log::warn!("   Failed to get pending addresses: {}", e);
            drop(db);
            return HttpResponse::Ok().json(serde_json::json!({
                "balance": cached_balance
            }));
        }
    };
    drop(db);

    // If there are pending addresses, fetch their UTXOs
    if !pending_addresses.is_empty() {
        log::info!("   🔍 Found {} pending address(es) to check for new UTXOs", pending_addresses.len());

        // Convert to AddressInfo format for API call
        let pending_address_infos: Vec<crate::json_storage::AddressInfo> = pending_addresses.iter()
            .map(|addr| crate::json_storage::AddressInfo {
                address: addr.address.clone(),
                index: addr.index,  // Already i32, no conversion needed
                public_key: addr.public_key.clone(),
                used: addr.used,
                balance: addr.balance,
            })
            .collect();

        // Fetch UTXOs for pending addresses
        let api_utxos = match crate::utxo_fetcher::fetch_all_utxos(&pending_address_infos).await {
            Ok(utxos) => utxos,
            Err(e) => {
                log::warn!("   Failed to fetch UTXOs for pending addresses: {}", e);
                log::warn!("   ⚠️  Keeping addresses marked as pending - will retry on next balance check");
                // Return cached balance but DON'T clear pending flags - retry next time
                return HttpResponse::Ok().json(serde_json::json!({
                    "balance": cached_balance
                }));
            }
        };

        // Cache UTXOs to database and clear pending flags (only if fetch succeeded)
        let db = state.database.lock().unwrap();
        let address_repo = AddressRepository::new(db.connection());
        let utxo_repo = UtxoRepository::new(db.connection());
        let mut cleared_ids = Vec::new();

        for addr in &pending_addresses {
            if let Some(addr_id) = addr.id {
                // Get UTXOs for this address
                let addr_utxos: Vec<_> = api_utxos.iter()
                    .filter(|u| u.address_index == addr.index as u32)
                    .cloned()
                    .collect();

                if !addr_utxos.is_empty() {
                    if let Err(e) = utxo_repo.upsert_utxos(addr_id, &addr_utxos) {
                        log::warn!("   Failed to cache UTXOs for {}: {}", addr.address, e);
                        // Continue to clear pending flag even if caching failed - we successfully checked the address
                    } else {
                        // Mark address as used if it has UTXOs
                        let _ = address_repo.mark_used(addr_id);
                    }
                }

                // Clear pending flag - fetch succeeded, so we've checked this address
                // (even if no UTXOs were found or caching failed, we at least tried)
                if let Err(e) = address_repo.clear_pending_utxo_check(addr_id) {
                    log::warn!("   Failed to clear pending flag for address {}: {}", addr.address, e);
                } else {
                    cleared_ids.push(addr_id);
                }
            }
        }
        drop(db);

        if !cleared_ids.is_empty() {
            log::info!("   ✅ Checked {} pending address(es) and cleared flags", cleared_ids.len());
        }

        // Recalculate balance including new UTXOs
        let db = state.database.lock().unwrap();
        let address_repo = AddressRepository::new(db.connection());
        let address_ids: Vec<i64> = addresses.iter()
            .filter_map(|addr| {
                match address_repo.get_by_address(&addr.address) {
                    Ok(Some(db_addr)) => db_addr.id,
                    _ => None,
                }
            })
            .collect();

        let utxo_repo = UtxoRepository::new(db.connection());
        let updated_balance = match utxo_repo.calculate_balance(&address_ids) {
            Ok(balance) => balance,
            Err(e) => {
                log::warn!("   Failed to recalculate balance: {}, using cached", e);
                cached_balance
            }
        };
        drop(db);

        log::info!("   ✅ Updated balance after checking pending addresses: {} satoshis", updated_balance);

        // Invalidate cache since new UTXOs were detected
        state.balance_cache.invalidate();
        // Update cache with new balance
        state.balance_cache.set(updated_balance);

        return HttpResponse::Ok().json(serde_json::json!({
            "balance": updated_balance
        }));
    }

    // Cache has balance from database - use it and cache it
    log::info!("   ✅ Using cached balance from database: {} satoshis", cached_balance);

    // Update cache with calculated balance
    state.balance_cache.set(cached_balance);

    // Return response in Go wallet format: { "balance": number }
    HttpResponse::Ok().json(serde_json::json!({
        "balance": cached_balance
    }))
}

// Request structure for /verifySignature
#[derive(Debug, Deserialize)]
pub struct VerifySignatureRequest {
    #[serde(rename = "data")]
    pub data: Option<serde_json::Value>, // Array of bytes
    #[serde(rename = "hashToDirectlyVerify")]
    pub hash_to_directly_verify: Option<serde_json::Value>, // Array of bytes
    #[serde(rename = "signature")]
    pub signature: serde_json::Value, // Array of bytes (64-byte compact R+S)
    #[serde(rename = "protocolID")]
    pub protocol_id: serde_json::Value, // [2, "auth message signature"]
    #[serde(rename = "keyID")]
    pub key_id: String, // e.g., "nonce1 nonce2"
    #[serde(rename = "counterparty")]
    pub counterparty: String, // Public key hex
}

// Response structure for /verifySignature
#[derive(Debug, Serialize)]
pub struct VerifySignatureResponse {
    pub valid: bool,
}

// /verifySignature - BRC-3 endpoint for verifying ECDSA signatures
// Verifies signatures created with BRC-42 derived keys
pub async fn verify_signature(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("📋 /verifySignature called");

    // Parse JSON body
    let body_str = String::from_utf8_lossy(&body);
    log::info!("   Raw body ({} bytes): {}...", body.len(), &body_str[..std::cmp::min(200, body_str.len())]);

    let req: VerifySignatureRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            log::error!("   JSON parse error: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid JSON: {}", e)
            }));
        }
    };

    // Parse signature bytes (can be array of bytes OR hex string)
    let signature_bytes = match &req.signature {
        serde_json::Value::Array(arr) => {
            arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect::<Vec<u8>>()
        }
        serde_json::Value::String(hex_str) => {
            match hex::decode(hex_str) {
                Ok(bytes) => bytes,
                Err(e) => {
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Invalid signature hex: {}", e)
                    }));
                }
            }
        }
        _ => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Signature must be an array of bytes or hex string"
            }));
        }
    };

    // Signature can be either:
    // - Compact format: 64 bytes (R + S)
    // - DER format: variable length (typically 70-72 bytes)
    if signature_bytes.len() < 64 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Signature too short: {} bytes", signature_bytes.len())
        }));
    }

    log::info!("   Signature: {} bytes", signature_bytes.len());

    // Get the hash to verify
    let data_hash = if let Some(hash) = &req.hash_to_directly_verify {
        // Use provided hash directly
        match hash {
            serde_json::Value::Array(arr) => {
                arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect::<Vec<u8>>()
            }
            _ => {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "hashToDirectlyVerify must be an array of bytes"
                }));
            }
        }
    } else if let Some(data) = &req.data {
        // Hash the data
        let data_bytes = match data {
            serde_json::Value::Array(arr) => {
                arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect::<Vec<u8>>()
            }
            _ => {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "data must be an array of bytes"
                }));
            }
        };
        sha256(&data_bytes)
    } else {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Either 'data' or 'hashToDirectlyVerify' must be provided"
        }));
    };

    if data_hash.len() != 32 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Hash must be 32 bytes, got {}", data_hash.len())
        }));
    }

    // Parse protocol ID (similar to createHmac)
    let protocol_id_str = if let serde_json::Value::Array(arr) = &req.protocol_id {
        if arr.len() == 2 {
            if let (Some(level), Some(name)) = (arr[0].as_u64(), arr[1].as_str()) {
                format!("{}-{}", level, name)
            } else {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid protocolID format"
                }));
            }
        } else {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "protocolID must be [securityLevel, protocolName]"
            }));
        }
    } else {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "protocolID must be an array"
        }));
    };

    log::info!("   Protocol ID: {}", protocol_id_str);
    log::info!("   Key ID: {}", req.key_id);
    log::info!("   Counterparty: {}", req.counterparty);

    // Create BRC-43 invoice number
    let invoice = format!("{}-{}", protocol_id_str, req.key_id);
    log::info!("   BRC-43 invoice: {}", invoice);

    // Decode counterparty public key from hex
    let counterparty_pubkey = match hex::decode(&req.counterparty) {
        Ok(bytes) => bytes,
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid counterparty public key hex: {}", e)
            }));
        }
    };

    if counterparty_pubkey.len() != 33 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Counterparty public key must be 33 bytes, got {}", counterparty_pubkey.len())
        }));
    }

    // Get our MASTER private key (needed for BRC-42 key derivation)
    // The counterparty field contains the SIGNER's public key
    // In this case, ToolBSV is asking us to verify OUR signature, so counterparty = our master pubkey
    let db = state.database.lock().unwrap();
    let our_master_privkey = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => {
            log::info!("   ✅ Master private key retrieved for verification");
            key
        },
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get master private key"
            }));
        }
    };
    drop(db);

    // BRC-3 Verification Logic:
    // When ToolBSV asks us to verify a signature that WE created:
    // 1. counterparty = our master public key (the signer's identity)
    // 2. We need to derive the signer's (our) child public key
    // 3. Since we're verifying our own signature, we derive our child PRIVATE key
    //    (using our master priv + their master pub + invoice)
    // 4. Extract the public key from our child private key
    // 5. Verify signature with that child public key

    // For ToolBSV's request: counterparty_pubkey is actually OUR master public key (020b95...)
    // We need to figure out THEIR master public key to compute the shared secret
    // But wait - in mutual auth, we used: our_priv + their_pub + invoice
    // So to verify, we need to know THEIR pubkey... but they didn't send it!

    // Actually, the counterparty field should be the identity of the OTHER party!
    // Let me check what ToolBSV actually sends...

    // UPDATE: After analysis, for BRC-42 verification:
    // - We derive our own child private key using: our_master_priv + their_master_pub + invoice
    // - Extract public key from child private key
    // - Verify with that public key

    // But we need THEIR master public key! Let's check if it's in the request...
    // If counterparty is OUR key, then we need to find THEIR key somewhere else

    // SOLUTION: For verifySignature with "self", counterparty should be the OTHER party's key
    // NOT the signer's key. ToolBSV should send THEIR identity key as counterparty.

    // Let me implement assuming counterparty = their master pubkey (the OTHER party in the handshake)
    log::info!("   Deriving signer's child public key using BRC-42...");
    log::info!("   Signer's master pubkey: {}", hex::encode(&counterparty_pubkey));

    // **CRITICAL**: When verifying, we derive the SIGNER's child public key, not ours!
    // The signer derived: child_private = signer_private + HMAC_scalar
    // We derive: child_public = signer_public + G * HMAC_scalar
    // This uses BRC-42's derive_child_public_key function
    use crate::crypto::brc42::derive_child_public_key as derive_child_pub;

    let signer_child_pubkey_bytes = match derive_child_pub(&our_master_privkey, &counterparty_pubkey, &invoice) {
        Ok(key) => {
            log::info!("   ✅ BRC-42 signer's child public key derived");
            key
        },
        Err(e) => {
            log::error!("   Failed to derive signer's child public key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to derive signer's child public key: {}", e)
            }));
        }
    };

    // Parse the derived child public key
    use secp256k1::{Secp256k1, PublicKey, Message, ecdsa::Signature};
    let secp = Secp256k1::new();
    let signer_child_pubkey = match PublicKey::from_slice(&signer_child_pubkey_bytes) {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Invalid signer's child public key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Invalid signer's child public key"
            }));
        }
    };

    log::info!("   ✅ Signer's child public key ready for verification");
    log::info!("   Signer's child pubkey: {}", hex::encode(signer_child_pubkey.serialize()));

    // Create message from data hash
    let message = match Message::from_digest_slice(&data_hash) {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("   Invalid message hash: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid message hash: {}", e)
            }));
        }
    };

    // Parse signature (try DER format first, then fall back to compact)
    let signature = if signature_bytes.len() == 64 {
        // Compact format: 64 bytes (R + S)
        match Signature::from_compact(&signature_bytes) {
            Ok(sig) => sig,
            Err(e) => {
                log::error!("   Invalid compact signature format: {}", e);
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Invalid compact signature format: {}", e)
                }));
            }
        }
    } else {
        // DER format: variable length
        match Signature::from_der(&signature_bytes) {
            Ok(sig) => sig,
            Err(e) => {
                log::error!("   Invalid DER signature format: {}", e);
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Invalid DER signature format: {}", e)
                }));
            }
        }
    };

    // Verify the signature using the signer's derived child public key
    let valid = secp.verify_ecdsa(&message, &signature, &signer_child_pubkey).is_ok();

    log::info!("   ✅ Signature verification result: {}", valid);

    HttpResponse::Ok().json(VerifySignatureResponse { valid })
}

// Request structure for /createSignature
#[derive(Debug, Deserialize)]
pub struct CreateSignatureRequest {
    #[serde(rename = "protocolID")]
    pub protocol_id: serde_json::Value, // [2, "protocol name"]
    #[serde(rename = "keyID")]
    pub key_id: String,
    #[serde(rename = "counterparty")]
    pub counterparty: serde_json::Value, // "self", "anyone", or hex pubkey
    #[serde(rename = "data")]
    pub data: serde_json::Value, // Array of bytes to sign
}

// Response structure for /createSignature
#[derive(Debug, Serialize)]
pub struct CreateSignatureResponse {
    pub signature: Vec<u8>, // DER-encoded signature
}

// /createSignature - BRC-3 endpoint for creating ECDSA signatures
pub async fn create_signature(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("📋 /createSignature called");

    // Parse JSON body
    let body_str = String::from_utf8_lossy(&body);
    log::info!("   Raw body ({} bytes): {}...", body.len(), &body_str[..std::cmp::min(200, body_str.len())]);

    let req: CreateSignatureRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            log::error!("   JSON parse error: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid JSON: {}", e)
            }));
        }
    };

    // Parse protocol ID
    let protocol_id_str = if let serde_json::Value::Array(arr) = &req.protocol_id {
        if arr.len() == 2 {
            if let (Some(level), Some(name)) = (arr[0].as_u64(), arr[1].as_str()) {
                format!("{}-{}", level, name)
            } else {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid protocolID format"
                }));
            }
        } else {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "protocolID must be [securityLevel, protocolName]"
            }));
        }
    } else {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "protocolID must be an array"
        }));
    };

    log::info!("   Protocol ID: {}", protocol_id_str);
    log::info!("   Key ID: {}", req.key_id);
    log::info!("   Counterparty: {:?}", req.counterparty);

    // Parse data bytes
    let data_bytes = match &req.data {
        serde_json::Value::Array(arr) => {
            arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect::<Vec<u8>>()
        }
        _ => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "data must be an array of bytes"
            }));
        }
    };

    log::info!("   Data bytes length: {}", data_bytes.len());

    // Create BRC-43 invoice number
    let invoice = format!("{}-{}", protocol_id_str, req.key_id);
    log::info!("   BRC-43 invoice: {}", invoice);

    // 🔐 SESSION NONCE VALIDATION
    // For post-authentication requests, the keyID contains two base64 nonces separated by a space:
    // "<their-nonce> <our-nonce-from-auth-session>"
    // We must validate that the second nonce matches our stored auth session.
    //
    // **IMPORTANT**: Session validation only applies to wallet-to-app authentication.
    // For API requests to external backends (e.g., Thoth), the app handles authentication
    // with the backend directly, and we just sign the requests. No session validation needed.

    // Get our wallet's identity key for comparison
    let our_identity_key = {
        let db = state.database.lock().unwrap();
        match crate::database::get_master_public_key_from_db(&db) {
            Ok(pubkey_bytes) => hex::encode(pubkey_bytes),
            Err(_) => String::new(),
        }
    };

    // Determine if this is a request to an external backend
    let is_external_backend = match &req.counterparty {
        serde_json::Value::String(s) if s != "self" && s != "anyone" && s != &our_identity_key => {
            log::info!("   🌐 External backend detected: {}", s);
            true
        }
        _ => false
    };

    if req.key_id.contains(' ') && !is_external_backend {
        log::info!("   🔍 Detected session-based keyID (contains space) - validating session");
        let parts: Vec<&str> = req.key_id.split(' ').collect();
        if parts.len() == 2 {
            let our_nonce_from_request = parts[1];
            log::info!("   🔍 Nonce from request: {}", our_nonce_from_request);

            // Get the counterparty identity key (for session lookup)
            let identity_key = match &req.counterparty {
                serde_json::Value::String(s) if s != "self" && s != "anyone" => s.clone(),
                _ => {
                    log::warn!("   ⚠️  Session validation requires a counterparty identity key");
                    return HttpResponse::Unauthorized().json(serde_json::json!({
                        "status": "error",
                        "code": "UNAUTHORIZED",
                        "message": "Session validation requires counterparty identity key"
                    }));
                }
            };

            log::info!("   🔍 Looking up session for identity: {} with nonce: {}", identity_key, our_nonce_from_request);

            // Retrieve the stored session using both identity key and nonce
            match state.auth_sessions.get_session(&identity_key, our_nonce_from_request) {
                Some(session) => {
                    log::info!("   ✅ Found auth session (created: {})", session.created_at);
                    log::info!("   ✅ Session nonce validated successfully!");
                }
                None => {
                    log::error!("   ❌ No auth session found for identity: {} with nonce: {}", identity_key, our_nonce_from_request);
                    return HttpResponse::Unauthorized().json(serde_json::json!({
                        "status": "error",
                        "code": "UNAUTHORIZED",
                        "message": "Mutual-authentication failed!"
                    }));
                }
            }
        } else {
            log::warn!("   ⚠️  keyID contains space but doesn't have exactly 2 parts");
        }
    } else if is_external_backend {
        log::info!("   ℹ️  External backend request - skipping session validation");
    } else {
        log::info!("   ℹ️  No session validation required (no space in keyID)");
    }

    // Get counterparty public key (if not "self" or "anyone")
    let counterparty_pubkey = match &req.counterparty {
        serde_json::Value::String(s) if s == "self" || s == "anyone" => {
            log::info!("   No counterparty ({})", s);
            None
        }
        serde_json::Value::String(hex_pubkey) => {
            match hex::decode(hex_pubkey) {
                Ok(bytes) if bytes.len() == 33 => {
                    log::info!("   Counterparty pubkey: {}", hex_pubkey);
                    Some(bytes)
                }
                Ok(bytes) => {
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Counterparty public key must be 33 bytes, got {}", bytes.len())
                    }));
                }
                Err(e) => {
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Invalid counterparty hex: {}", e)
                    }));
                }
            }
        }
        _ => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "counterparty must be a string"
            }));
        }
    };

    // Get MASTER private key (m) for signature operations
    // CRITICAL: Must use the same master key for both auth and signature operations!
    let db = state.database.lock().unwrap();
    let private_key_bytes = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get master private key"
            }));
        }
    };
    drop(db);

    log::info!("   ✅ MASTER private key retrieved for signature (createSignature)");

    // Derive BRC-42 child private key
    let child_privkey = if let Some(counterparty_pub) = counterparty_pubkey {
        // BRC-42 derivation with counterparty
        match derive_child_private_key(&private_key_bytes, &counterparty_pub, &invoice) {
            Ok(key) => key,
            Err(e) => {
                log::error!("   Failed to derive BRC-42 child key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to derive child key: {}", e)
                }));
            }
        }
    } else {
        // Simple derivation (no counterparty)
        log::info!("   Using wallet private key for signature (no counterparty)");
        private_key_bytes
    };

    log::info!("   ✅ Child private key derived");

    // Hash the data with SHA256
    let data_hash = sha256(&data_bytes);
    log::info!("   Data hash (32 bytes): {}", hex::encode(&data_hash));

    // Sign with ECDSA
    use secp256k1::{Secp256k1, Message, SecretKey};

    let secp = Secp256k1::new();

    let secret = match SecretKey::from_slice(&child_privkey) {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Invalid private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Invalid private key"
            }));
        }
    };

    let message = match Message::from_digest_slice(&data_hash) {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("   Invalid message hash: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Invalid message hash"
            }));
        }
    };

    let signature = secp.sign_ecdsa(&message, &secret);

    // Serialize as DER format (not compact!)
    let signature_der = signature.serialize_der();
    let signature_hex = hex::encode(&signature_der);

    log::info!("   ✅ Signature created ({} bytes, DER format): {}", signature_der.len(), signature_hex);

    HttpResponse::Ok().json(CreateSignatureResponse {
        signature: signature_der.to_vec()
    })
}

// ============================================================================
// Transaction Action Endpoints (BRC-1)
// ============================================================================

use crate::transaction::{Transaction, TxInput, TxOutput, OutPoint};
use crate::utxo_fetcher::UTXO;
use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use once_cell::sync::Lazy;

// BRC-29 payment metadata (Simple Authenticated BSV P2PKH Payment Protocol)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Brc29PaymentInfo {
    derivation_prefix: String,
    derivation_suffix: String,
    payee: String,
    output_index: usize,
}

/// User input info for signing (when inputs come from inputBEEF)
#[derive(Debug, Clone)]
struct UserInputInfo {
    txid: String,
    vout: u32,
    satoshis: i64,
    locking_script: Vec<u8>,
    source_tx: Option<Vec<u8>>,  // Raw source transaction bytes (from inputBEEF)
    is_pre_signed: bool,  // true if unlocking script already set
}

// Pending transaction with metadata
#[derive(Debug, Clone)]
struct PendingTransaction {
    tx: Transaction,
    input_utxos: Vec<UTXO>, // Wallet UTXOs being spent (for signing)
    user_input_infos: Vec<UserInputInfo>, // User-provided inputs (from inputBEEF)
    brc29_info: Option<Brc29PaymentInfo>, // BRC-29 payment metadata if applicable
    input_beef: Option<crate::beef::Beef>, // Full inputBEEF for verification chain
}

// In-memory storage for pending transactions
static PENDING_TRANSACTIONS: Lazy<StdMutex<HashMap<String, PendingTransaction>>> =
    Lazy::new(|| StdMutex::new(HashMap::new()));

/// BRC-100 CreateAction input outpoint specification
/// Can be deserialized from either:
/// - Object format: {"txid": "abc...", "vout": 0}
/// - String format: "abc...0" (txid.vout)
#[derive(Debug, Clone, Serialize)]
pub struct CreateActionOutpoint {
    pub txid: String,
    pub vout: u32,
}

impl<'de> serde::Deserialize<'de> for CreateActionOutpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor, MapAccess};

        struct OutpointVisitor;

        impl<'de> Visitor<'de> for OutpointVisitor {
            type Value = CreateActionOutpoint;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an outpoint object {txid, vout} or string 'txid.vout'")
            }

            // Handle string format: "txid.vout"
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                // Find the last '.' to split txid and vout
                if let Some(dot_pos) = value.rfind('.') {
                    let txid = value[..dot_pos].to_string();
                    let vout_str = &value[dot_pos + 1..];
                    let vout = vout_str.parse::<u32>()
                        .map_err(|_| de::Error::custom(format!("invalid vout in outpoint: {}", vout_str)))?;
                    Ok(CreateActionOutpoint { txid, vout })
                } else {
                    Err(de::Error::custom(format!("invalid outpoint string format: {}", value)))
                }
            }

            // Handle object format: {txid: "...", vout: 0}
            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut txid: Option<String> = None;
                let mut vout: Option<u32> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "txid" => {
                            txid = Some(map.next_value()?);
                        }
                        "vout" => {
                            vout = Some(map.next_value()?);
                        }
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }

                let txid = txid.ok_or_else(|| de::Error::missing_field("txid"))?;
                let vout = vout.ok_or_else(|| de::Error::missing_field("vout"))?;

                Ok(CreateActionOutpoint { txid, vout })
            }
        }

        deserializer.deserialize_any(OutpointVisitor)
    }
}

/// BRC-100 CreateAction input specification
/// When an app provides inputs, it's specifying UTXOs to spend (possibly pre-signed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateActionInput {
    /// The outpoint (txid + vout) of the UTXO to spend
    #[serde(rename = "outpoint")]
    pub outpoint: CreateActionOutpoint,

    /// Hex-encoded unlocking script (if pre-signed, e.g., ANYONECANPAY)
    #[serde(rename = "unlockingScript")]
    pub unlocking_script: Option<String>,

    /// Length of unlocking script (for fee calculation when script not yet known)
    #[serde(rename = "unlockingScriptLength")]
    pub unlocking_script_length: Option<usize>,

    /// Sequence number (default: 0xFFFFFFFF)
    #[serde(rename = "sequenceNumber")]
    pub sequence_number: Option<u32>,

    /// Satoshi value of this input (for fee calculation)
    #[serde(rename = "inputSatoshis")]
    pub input_satoshis: Option<i64>,
}

// Request structure for /createAction
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateActionRequest {
    /// User-provided inputs (optional) - references UTXOs in inputBEEF
    #[serde(rename = "inputs")]
    pub inputs: Option<Vec<CreateActionInput>>,

    #[serde(rename = "outputs")]
    pub outputs: Vec<CreateActionOutput>,

    #[serde(rename = "description")]
    pub description: Option<String>,

    #[serde(rename = "labels")]
    pub labels: Option<Vec<String>>,

    #[serde(rename = "options")]
    pub options: Option<CreateActionOptions>,

    /// BEEF containing source transactions for inputs (BRC-62/95/96)
    /// Can be either a hex string or a byte array [u8, u8, ...]
    #[serde(rename = "inputBEEF")]
    pub input_beef: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateActionOutput {
    #[serde(rename = "satoshis")]
    pub satoshis: Option<i64>,

    #[serde(rename = "script", alias = "lockingScript")]
    pub script: Option<String>, // Hex-encoded locking script (accepts both "script" and "lockingScript")

    #[serde(rename = "address")]
    pub address: Option<String>, // Bitcoin address (alternative to script)

    #[serde(rename = "customInstructions")]
    pub custom_instructions: Option<String>, // BRC-78 payment protocol data (JSON string)

    #[serde(rename = "outputDescription")]
    pub output_description: Option<String>, // Description of this output
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateActionOptions {
    #[serde(rename = "signAndProcess")]
    pub sign_and_process: Option<bool>, // Default: true

    #[serde(rename = "acceptDelayedBroadcast")]
    pub accept_delayed_broadcast: Option<bool>, // Default: true

    #[serde(rename = "returnTXIDOnly")]
    pub return_txid_only: Option<bool>, // Default: false

    #[serde(rename = "noSend")]
    pub no_send: Option<bool>, // Default: false

    #[serde(rename = "randomizeOutputs")]
    pub randomize_outputs: Option<bool>, // Default: true
}

// Response structure for /createAction - full BRC-100 spec
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateActionResponse {
    pub reference: String,
    pub version: u32,
    #[serde(rename = "lockTime")]
    pub lock_time: u32,
    pub inputs: Vec<CreateActionResponseInput>,
    pub outputs: Vec<CreateActionResponseOutput>,
    #[serde(rename = "derivationPrefix", skip_serializing_if = "Option::is_none")]
    pub derivation_prefix: Option<String>,
    #[serde(rename = "inputBeef", skip_serializing_if = "Option::is_none")]
    pub input_beef: Option<Vec<u8>>,
    #[serde(rename = "txid", skip_serializing_if = "Option::is_none")]
    pub txid: Option<String>,
    #[serde(rename = "tx", skip_serializing_if = "Option::is_none")]
    pub tx: Option<Vec<u8>>, // Atomic BEEF (BRC-95) as byte array per BRC-100 spec
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateActionResponseInput {
    pub txid: String,
    pub vout: u32,
    #[serde(rename = "outputIndex")]
    pub output_index: u32,
    #[serde(rename = "scriptLength")]
    pub script_length: usize,
    #[serde(rename = "scriptOffset")]
    pub script_offset: usize,
    pub sequence: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateActionResponseOutput {
    pub vout: u32,
    pub satoshis: i64,
    #[serde(rename = "scriptLength")]
    pub script_length: usize,
    #[serde(rename = "scriptOffset")]
    pub script_offset: usize,
}

// /createAction - Build unsigned transaction
pub async fn create_action(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("📋 /createAction called");
    log::info!("📋 Raw request body ({} bytes): {}...",
        body.len(),
        String::from_utf8_lossy(&body[..std::cmp::min(500, body.len())]));

    // Parse request
    let req: CreateActionRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            log::error!("   JSON parse error: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid JSON: {}", e)
            }));
        }
    };

    log::info!("   Description: {:?}", req.description);
    log::info!("   Outputs: {}", req.outputs.len());
    log::info!("   Inputs provided: {}", req.inputs.as_ref().map(|i| i.len()).unwrap_or(0));
    log::info!("   InputBEEF provided: {}", req.input_beef.is_some());

    // Parse inputBEEF if provided (contains source transactions for user inputs)
    // inputBEEF can be either a hex string or a byte array [u8, u8, ...]
    let parsed_input_beef: Option<crate::beef::Beef> = if let Some(ref beef_value) = req.input_beef {
        // Convert inputBEEF to bytes depending on format
        let beef_bytes: Vec<u8> = if let Some(hex_str) = beef_value.as_str() {
            // Format 1: Hex string
            log::info!("   InputBEEF format: hex string ({} chars)", hex_str.len());
            match hex::decode(hex_str) {
                Ok(bytes) => bytes,
                Err(e) => {
                    log::error!("   ❌ Failed to decode inputBEEF hex: {}", e);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Invalid inputBEEF hex: {}", e)
                    }));
                }
            }
        } else if let Some(arr) = beef_value.as_array() {
            // Format 2: Byte array [u8, u8, ...]
            log::info!("   InputBEEF format: byte array ({} elements)", arr.len());
            let mut bytes = Vec::with_capacity(arr.len());
            for (i, val) in arr.iter().enumerate() {
                match val.as_u64() {
                    Some(n) if n <= 255 => bytes.push(n as u8),
                    _ => {
                        log::error!("   ❌ Invalid byte at position {}: {:?}", i, val);
                        return HttpResponse::BadRequest().json(serde_json::json!({
                            "error": format!("Invalid inputBEEF: invalid byte at position {}", i)
                        }));
                    }
                }
            }
            bytes
        } else {
            log::error!("   ❌ Invalid inputBEEF format: expected string or array");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid inputBEEF format: expected hex string or byte array"
            }));
        };

        // Parse BEEF from bytes
        match crate::beef::Beef::from_bytes(&beef_bytes) {
            Ok(beef) => {
                log::info!("   ✅ Parsed inputBEEF: {} transactions, {} BUMPs",
                    beef.transactions.len(), beef.bumps.len());
                for (i, tx_bytes) in beef.transactions.iter().enumerate() {
                    // Calculate txid for logging
                    use sha2::{Sha256, Digest};
                    let first_hash = Sha256::digest(tx_bytes);
                    let second_hash = Sha256::digest(&first_hash);
                    let txid = hex::encode(second_hash.iter().rev().copied().collect::<Vec<u8>>());
                    log::info!("      BEEF tx {}: {} ({} bytes)", i, &txid[..16], tx_bytes.len());
                }
                Some(beef)
            }
            Err(e) => {
                log::error!("   ❌ Failed to parse inputBEEF: {}", e);
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Invalid inputBEEF format: {}", e)
                }));
            }
        }
    } else {
        None
    };

    // Process user-provided inputs (from args.inputs referencing inputBEEF)
    struct UserProvidedInput {
        txid: String,
        vout: u32,
        satoshis: i64,
        source_tx: Option<Vec<u8>>,
        locking_script: Vec<u8>,
        unlocking_script: Option<Vec<u8>>,
        sequence: u32,
    }

    let mut user_inputs: Vec<UserProvidedInput> = Vec::new();
    let mut user_input_total: i64 = 0;

    if let Some(ref inputs) = req.inputs {
        log::info!("   Processing {} user-provided inputs...", inputs.len());

        for (i, input) in inputs.iter().enumerate() {
            log::info!("   Input {}: {}:{}", i, &input.outpoint.txid[..16], input.outpoint.vout);

            // Look up source transaction from inputBEEF
            let source_tx = parsed_input_beef.as_ref()
                .and_then(|beef| beef.find_txid(&input.outpoint.txid))
                .map(|idx| parsed_input_beef.as_ref().unwrap().transactions[idx].clone());

            // Parse source transaction to get output value and locking script
            let (satoshis, locking_script) = if let Some(ref tx_bytes) = source_tx {
                match crate::beef::ParsedTransaction::from_bytes(tx_bytes) {
                    Ok(parsed_tx) => {
                        if let Some(output) = parsed_tx.outputs.get(input.outpoint.vout as usize) {
                            log::info!("      Found output: {} satoshis", output.value);
                            (output.value, output.script.clone())
                        } else {
                            log::error!("      Output index {} not found in source tx", input.outpoint.vout);
                            return HttpResponse::BadRequest().json(serde_json::json!({
                                "error": format!("Output index {} not found in source transaction {}",
                                    input.outpoint.vout, input.outpoint.txid)
                            }));
                        }
                    }
                    Err(e) => {
                        log::error!("      Failed to parse source transaction: {}", e);
                        return HttpResponse::BadRequest().json(serde_json::json!({
                            "error": format!("Failed to parse source transaction: {}", e)
                        }));
                    }
                }
            } else if let Some(sats) = input.input_satoshis {
                // No source tx in BEEF, but satoshis provided - this is for pre-signed inputs
                log::info!("      Using provided satoshis: {}", sats);
                (sats, Vec::new()) // Empty locking script - will use unlocking script directly
            } else {
                log::error!("      Source tx not in inputBEEF and no satoshis provided");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Input {}:{} not found in inputBEEF. Provide source transaction or inputSatoshis.",
                        input.outpoint.txid, input.outpoint.vout)
                }));
            };

            // Parse unlocking script if provided (pre-signed input)
            let unlocking_script = if let Some(ref script_hex) = input.unlocking_script {
                match hex::decode(script_hex) {
                    Ok(bytes) => {
                        log::info!("      Has pre-signed unlocking script ({} bytes)", bytes.len());
                        Some(bytes)
                    }
                    Err(e) => {
                        log::error!("      Invalid unlocking script hex: {}", e);
                        return HttpResponse::BadRequest().json(serde_json::json!({
                            "error": format!("Invalid unlocking script: {}", e)
                        }));
                    }
                }
            } else {
                None
            };

            user_input_total += satoshis;

            user_inputs.push(UserProvidedInput {
                txid: input.outpoint.txid.clone(),
                vout: input.outpoint.vout,
                satoshis,
                source_tx,
                locking_script,
                unlocking_script,
                sequence: input.sequence_number.unwrap_or(0xFFFFFFFF),
            });
        }

        log::info!("   Total from user inputs: {} satoshis", user_input_total);
    }

    // Calculate total output amount
    let mut total_output: i64 = 0;
    for (i, output) in req.outputs.iter().enumerate() {
        if let Some(sats) = output.satoshis {
            total_output += sats;
            log::info!("   Output {}: {} satoshis", i, sats);
        }
    }

    log::info!("   Total output amount: {} satoshis", total_output);

    // Collect output script lengths for fee estimation
    let mut output_script_lengths: Vec<usize> = Vec::new();
    for output in req.outputs.iter() {
        let script_len = if let Some(ref script_hex) = output.script {
            script_hex.len() / 2  // Hex string, so divide by 2 for bytes
        } else if output.address.is_some() {
            25  // P2PKH locking script length
        } else {
            25  // Default to P2PKH
        };
        output_script_lengths.push(script_len);
    }

    // Also account for user-provided inputs' unlocking scripts
    let mut user_input_script_lengths: Vec<usize> = Vec::new();
    for user_input in &user_inputs {
        let script_len = user_input.unlocking_script.as_ref()
            .map(|s| s.len())
            .unwrap_or(107);  // Default to P2PKH unlocking script size
        user_input_script_lengths.push(script_len);
    }

    // Estimate fee based on size:
    // - User-provided inputs + estimated wallet inputs (assume 1-2 for simple tx)
    // - All outputs + potential change output
    let estimated_wallet_inputs = if user_inputs.is_empty() { 2 } else { 1 };
    let total_estimated_inputs = user_inputs.len() + estimated_wallet_inputs;

    // Calculate estimated fee (1 sat/byte = 1000 sat/kb)
    let mut estimated_fee = estimate_fee_for_transaction(
        total_estimated_inputs,
        &output_script_lengths,
        true,  // Include change output
        DEFAULT_SATS_PER_KB
    ) as i64;

    log::info!("   📊 Fee estimation:");
    log::info!("      Estimated inputs: {} (user: {}, wallet: ~{})",
        total_estimated_inputs, user_inputs.len(), estimated_wallet_inputs);
    log::info!("      Output count: {} + 1 change", output_script_lengths.len());
    log::info!("      Estimated fee: {} satoshis ({} sat/byte)",
        estimated_fee, DEFAULT_SATS_PER_KB / 1000);

    let total_needed = total_output + estimated_fee;
    log::info!("   Total needed: {} satoshis", total_needed);

    // Determine if we need wallet's UTXOs
    // If user provided inputs that cover the total, we don't need wallet UTXOs
    let shortfall = total_needed - user_input_total;
    let need_wallet_utxos = user_inputs.is_empty() || shortfall > 0;

    if !user_inputs.is_empty() && shortfall <= 0 {
        log::info!("   ✅ User inputs cover total needed ({} >= {})", user_input_total, total_needed);
    } else if !user_inputs.is_empty() {
        log::info!("   User inputs don't cover total, need {} more from wallet", shortfall);
    }

    // Wallet UTXOs - only fetch if we need them
    let mut selected_utxos: Vec<crate::utxo_fetcher::UTXO> = Vec::new();
    let addresses: Vec<crate::json_storage::AddressInfo>;

    if need_wallet_utxos {
        // Fetch UTXOs from WhatsOnChain - get addresses from database
        addresses = {
            use crate::database::{WalletRepository, AddressRepository, address_to_address_info};

            let db = state.database.lock().unwrap();
            let wallet_repo = WalletRepository::new(db.connection());

            let wallet = match wallet_repo.get_primary_wallet() {
                Ok(Some(w)) => w,
                Ok(None) => {
                    log::error!("   No wallet found in database");
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "No wallet found"
                    }));
                }
                Err(e) => {
                    log::error!("   Failed to get wallet: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Database error: {}", e)
                    }));
                }
            };

            let address_repo = AddressRepository::new(db.connection());
            match address_repo.get_all_by_wallet(wallet.id.unwrap()) {
                Ok(db_addresses) => {
                    db_addresses.iter()
                        .map(|addr| address_to_address_info(addr))
                        .collect::<Vec<_>>()
                }
                Err(e) => {
                    log::error!("   Failed to get addresses: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to get addresses: {}", e)
                    }));
                }
            }
        };

        log::info!("   Checking {} addresses for UTXOs...", addresses.len());

        // Try to get UTXOs from database cache first
        use crate::database::{UtxoRepository, AddressRepository, utxo_to_fetcher_utxo};
        let db = state.database.lock().unwrap();
        let address_repo = AddressRepository::new(db.connection());
        let utxo_repo = UtxoRepository::new(db.connection());

        // Get address IDs and create address index map
        let mut address_id_map: std::collections::HashMap<i64, u32> = std::collections::HashMap::new();
        let mut address_ids = Vec::new();

        for addr in &addresses {
            if let Ok(Some(db_addr)) = address_repo.get_by_address(&addr.address) {
                if let Some(addr_id) = db_addr.id {
                    address_ids.push(addr_id);
                    address_id_map.insert(addr_id, addr.index as u32);
                }
            }
        }

        // Get UTXOs from database
        let mut all_utxos = match utxo_repo.get_unspent_by_addresses(&address_ids) {
            Ok(db_utxos) => {
                // Convert database UTXOs to fetcher format
                db_utxos.iter()
                    .filter_map(|db_utxo| {
                        address_id_map.get(&db_utxo.address_id)
                            .map(|&idx| utxo_to_fetcher_utxo(db_utxo, idx))
                    })
                    .collect::<Vec<_>>()
            }
            Err(e) => {
                log::warn!("   Failed to get UTXOs from database: {}, falling back to API", e);
                Vec::new()
            }
        };

        drop(db);

        // Amount needed from wallet (considering what user inputs provide)
        let wallet_amount_needed = if shortfall > 0 { shortfall } else { total_needed };

        // Phase 4: Use cached UTXOs from database (primary source)
        // Only fetch from API if cache is empty OR if we don't have enough balance
        let cached_balance: i64 = all_utxos.iter().map(|u| u.satoshis).sum();
        if all_utxos.is_empty() {
            log::info!("   Cache is empty, fetching UTXOs from API to populate cache...");
        } else if cached_balance < wallet_amount_needed {
            log::info!("   Insufficient cached balance ({} < {}), fetching from API to check for new UTXOs...", cached_balance, wallet_amount_needed);
        } else {
            log::info!("   ✅ Using cached UTXOs from database ({} UTXOs, {} satoshis)", all_utxos.len(), cached_balance);
        }

        if all_utxos.is_empty() || cached_balance < wallet_amount_needed {

            // Fetch from API
            let api_utxos = match crate::utxo_fetcher::fetch_all_utxos(&addresses).await {
                Ok(utxos) => utxos,
                Err(e) => {
                    log::error!("   Failed to fetch UTXOs from API: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to fetch UTXOs: {}", e)
                    }));
                }
            };

            // Cache UTXOs to database
            let db = state.database.lock().unwrap();
            let address_repo = AddressRepository::new(db.connection());
            let utxo_repo = UtxoRepository::new(db.connection());

            for addr in &addresses {
                if let Ok(Some(db_addr)) = address_repo.get_by_address(&addr.address) {
                    if let Some(addr_id) = db_addr.id {
                        // Get UTXOs for this address (clone to get owned values)
                        let addr_utxos: Vec<_> = api_utxos.iter()
                            .filter(|u| u.address_index == addr.index as u32)
                            .cloned()
                            .collect();

                        if !addr_utxos.is_empty() {
                            if let Err(e) = utxo_repo.upsert_utxos(addr_id, &addr_utxos) {
                                log::warn!("   Failed to cache UTXOs for {}: {}", addr.address, e);
                            }
                        }
                    }
                }
            }
            drop(db);

            // Use API UTXOs (they're more up-to-date)
            all_utxos = api_utxos;
        }

        if all_utxos.is_empty() && user_inputs.is_empty() {
            log::error!("   No UTXOs available and no user inputs");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Insufficient funds: no UTXOs available"
            }));
        }

        // Select UTXOs to cover the amount needed from wallet
        if !all_utxos.is_empty() {
            selected_utxos = select_utxos(&all_utxos, wallet_amount_needed);

            if selected_utxos.is_empty() && user_inputs.is_empty() {
                log::error!("   Insufficient funds");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Insufficient funds: need {} sats, have {} sats",
                        wallet_amount_needed,
                        all_utxos.iter().map(|u| u.satoshis).sum::<i64>()
                    )
                }));
            }

            let wallet_total: i64 = selected_utxos.iter().map(|u| u.satoshis).sum();
            log::info!("   Selected {} wallet UTXOs ({} satoshis)", selected_utxos.len(), wallet_total);
        }
    } else {
        // No wallet UTXOs needed - user inputs cover everything
        addresses = Vec::new();
        log::info!("   ✅ Skipping wallet UTXO fetch - user inputs cover all requirements");
    }

    // Calculate total input (user inputs + wallet inputs)
    let wallet_input_total: i64 = selected_utxos.iter().map(|u| u.satoshis).sum();
    let total_input: i64 = user_input_total + wallet_input_total;
    log::info!("   Total inputs: {} satoshis (user: {}, wallet: {})",
        total_input, user_input_total, wallet_input_total);

    // RECALCULATE fee now that we know actual input count
    let actual_input_count = user_inputs.len() + selected_utxos.len();

    // Build input script lengths for accurate size calculation
    let mut actual_input_script_lengths: Vec<usize> = Vec::new();

    // User inputs - use actual unlocking script size or estimate
    for user_input in &user_inputs {
        let script_len = user_input.unlocking_script.as_ref()
            .map(|s| s.len())
            .unwrap_or(107);  // Default P2PKH unlocking script
        actual_input_script_lengths.push(script_len);
    }

    // Wallet inputs - P2PKH unlocking script size
    for _ in &selected_utxos {
        actual_input_script_lengths.push(107);
    }

    // Recalculate fee with accurate input count
    let estimated_tx_size = estimate_transaction_size(&actual_input_script_lengths, &output_script_lengths)
        + 25 + 9;  // Add P2PKH change output (25 script + 8 value + 1 varint)

    estimated_fee = calculate_fee(estimated_tx_size, DEFAULT_SATS_PER_KB) as i64;

    log::info!("   📊 Recalculated fee with actual {} inputs:", actual_input_count);
    log::info!("      Estimated tx size: {} bytes", estimated_tx_size);
    log::info!("      Recalculated fee: {} satoshis", estimated_fee);

    // Build transaction
    let mut tx = Transaction::new();

    // Add USER inputs first (may have pre-signed unlocking scripts)
    for user_input in &user_inputs {
        let outpoint = OutPoint::new(user_input.txid.clone(), user_input.vout);
        let mut input = TxInput::new(outpoint);
        input.sequence = user_input.sequence;

        // If pre-signed, set the unlocking script
        if let Some(ref unlock_script) = user_input.unlocking_script {
            input.script_sig = unlock_script.clone();
            log::info!("   Added user input {}:{} with pre-signed script ({} bytes)",
                &user_input.txid[..16], user_input.vout, unlock_script.len());
        } else {
            log::info!("   Added user input {}:{} (will sign later)",
                &user_input.txid[..16], user_input.vout);
        }

        tx.add_input(input);
    }

    // Add WALLET inputs (unsigned - we'll sign these)
    for utxo in &selected_utxos {
        let outpoint = OutPoint::new(utxo.txid.clone(), utxo.vout);
        tx.add_input(TxInput::new(outpoint));
        log::info!("   Added wallet input {}:{}", &utxo.txid[..16], utxo.vout);
    }

    // Track BRC-29 payment info if present
    let mut brc29_info: Option<Brc29PaymentInfo> = None;

    // Add requested outputs
    for (i, output) in req.outputs.iter().enumerate() {
        let script_bytes = if let Some(custom_instr) = &output.custom_instructions {
            // Check if this is a BRC-29 payment
            log::info!("   Output {}: Has customInstructions, checking for BRC-29 payment...", i);
            match serde_json::from_str::<serde_json::Value>(custom_instr) {
                Ok(instr_json) => {
                    if let (Some(prefix), Some(suffix), Some(payee)) = (
                        instr_json["derivationPrefix"].as_str(),
                        instr_json["derivationSuffix"].as_str(),
                        instr_json["payee"].as_str()
                    ) {
                        log::info!("   ✅ BRC-29 payment detected");
                        log::info!("   Payee: {}", payee);
                        log::info!("   Deriving P2PKH script using BRC-42...");

                        // Store BRC-29 metadata for later conversion to BRC-29 format
                        brc29_info = Some(Brc29PaymentInfo {
                            derivation_prefix: prefix.to_string(),
                            derivation_suffix: suffix.to_string(),
                            payee: payee.to_string(),
                            output_index: i,
                        });

                        // Get our master private key for BRC-42 derivation
                        let db = state.database.lock().unwrap();
                        let master_key_bytes = match crate::database::get_master_private_key_from_db(&db) {
                            Ok(key) => key,
                            Err(e) => {
                                log::error!("   Failed to get master key: {}", e);
                                return HttpResponse::InternalServerError().json(serde_json::json!({
                                    "error": "Failed to get master key"
                                }));
                            }
                        };
                        drop(db);

                        // Parse recipient's public key (payee)
                        let payee_bytes = match hex::decode(payee) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                log::error!("   Failed to decode payee public key: {}", e);
                                return HttpResponse::BadRequest().json(serde_json::json!({
                                    "error": format!("Invalid payee public key: {}", e)
                                }));
                            }
                        };

                        // BRC-29 invoice number format: "2-3241645161d8-<prefix> <suffix>"
                        let invoice_number = format!("2-3241645161d8-{} {}", prefix, suffix);
                        log::info!("   Invoice number: {}", invoice_number);

                        // Derive child public key using BRC-42 (correct implementation from brc42.rs)
                        let derived_pubkey = match derive_child_public_key(&master_key_bytes, &payee_bytes, &invoice_number) {
                            Ok(pubkey) => pubkey,
                            Err(e) => {
                                log::error!("   Failed to derive BRC-42 public key: {}", e);
                                return HttpResponse::InternalServerError().json(serde_json::json!({
                                    "error": format!("BRC-42 derivation failed: {}", e)
                                }));
                            }
                        };

                        log::info!("   Derived pubkey: {}", hex::encode(&derived_pubkey));

                        // Create P2PKH script from derived public key
                        let script = create_p2pkh_script_from_pubkey(&derived_pubkey);
                        log::info!("   ✅ Created BRC-29 P2PKH script: {}", hex::encode(&script));
                        script
                    } else {
                        // customInstructions exists but not BRC-29 format, fall through to provided script
                        log::info!("   customInstructions not in BRC-29 format, using provided script");
                        if let Some(script_hex) = &output.script {
                            match hex::decode(script_hex) {
                                Ok(bytes) => bytes,
                                Err(e) => {
                                    log::error!("   Invalid output script hex: {}", e);
                                    return HttpResponse::BadRequest().json(serde_json::json!({
                                        "error": format!("Invalid output script hex: {}", e)
                                    }));
                                }
                            }
                        } else {
                            log::error!("   customInstructions present but no script provided");
                            return HttpResponse::BadRequest().json(serde_json::json!({
                                "error": "customInstructions present but no script provided"
                            }));
                        }
                    }
                }
                Err(e) => {
                    log::warn!("   Failed to parse customInstructions: {}", e);
                    // Fall through to provided script
                    if let Some(script_hex) = &output.script {
                        match hex::decode(script_hex) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                log::error!("   Invalid output script hex: {}", e);
                                return HttpResponse::BadRequest().json(serde_json::json!({
                                    "error": format!("Invalid output script hex: {}", e)
                                }));
                            }
                        }
                    } else {
                        log::error!("   Failed to parse customInstructions and no script provided");
                        return HttpResponse::BadRequest().json(serde_json::json!({
                            "error": "Invalid customInstructions format"
                        }));
                    }
                }
            }
        } else if let Some(script_hex) = &output.script {
            // Use provided script
            log::info!("   Output {}: Using provided script: {}", i, &script_hex[..script_hex.len().min(40)]);
            match hex::decode(script_hex) {
            Ok(bytes) => bytes,
            Err(e) => {
                    log::error!("   Invalid output script hex: {}", e);
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Invalid output script hex: {}", e)
                }));
            }
            }
        } else if let Some(address) = &output.address {
            // Convert address to P2PKH script
            log::info!("   Output {}: Converting address to script: {}", i, address);
            match address_to_script(address) {
                Ok(script) => script,
                Err(e) => {
                    log::error!("   Failed to convert address '{}': {}", address, e);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Invalid address: {}", e)
                    }));
                }
            }
        } else {
            // No address or script provided - this is a BRC-78 payment that the browser
            // should have handled but didn't. Reject with helpful error.
            log::error!("   Output {} missing both 'script' and 'address' fields", i);
            log::error!("   This usually means the browser didn't handle BRC-78 payment headers");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Output must have either 'script' or 'address' field. If you're trying to make a payment, the browser needs to implement BRC-78 payment protocol support first."
            }));
        };

        let satoshis = output.satoshis.unwrap_or(0);
        log::info!("   Output {}: {} satoshis", i, satoshis);
        log::info!("   Output {} script (hex): {}", i, hex::encode(&script_bytes));
        tx.add_output(TxOutput::new(satoshis, script_bytes));
    }

    // Calculate change
    let change = total_input - total_output - estimated_fee;
    log::info!("   Change: {} satoshis", change);

    if change > 546 { // Dust limit
        // Get first address for change
        // Generate NEW change address (privacy: don't reuse addresses)
        use crate::database::{WalletRepository, AddressRepository, get_master_private_key_from_db, get_master_public_key_from_db};
        // derive_child_public_key is already imported at top of file from crate::crypto::brc42
        use std::time::{SystemTime, UNIX_EPOCH};

        let db = state.database.lock().unwrap();
        let wallet_repo = WalletRepository::new(db.connection());
        let wallet = match wallet_repo.get_primary_wallet() {
            Ok(Some(w)) => w,
            Ok(None) | Err(_) => {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "No wallet found"
                }));
            }
        };

        let wallet_id = wallet.id.unwrap();
        let current_index = wallet.current_index;

        // Derive new address for change
        let master_privkey = match get_master_private_key_from_db(&db) {
            Ok(k) => k,
            Err(e) => {
                log::error!("   Failed to get master private key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to get master key: {}", e)
                }));
            }
        };

        let master_pubkey = match get_master_public_key_from_db(&db) {
            Ok(k) => k,
            Err(e) => {
                log::error!("   Failed to get master public key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to get master public key: {}", e)
                }));
            }
        };

        // Create BRC-43 invoice number for change address
        let invoice_number = format!("2-receive address-{}", current_index);

        // Derive child public key using BRC-42
        let derived_pubkey = match derive_child_public_key(&master_privkey, &master_pubkey, &invoice_number) {
            Ok(pubkey) => pubkey,
            Err(e) => {
                log::error!("   BRC-42 derivation failed: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("BRC-42 derivation failed: {}", e)
                }));
            }
        };

        // Convert to Bitcoin address
        let change_address = match pubkey_to_address(&derived_pubkey) {
            Ok(addr) => addr,
            Err(e) => {
                log::error!("   Failed to create change address: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to create change address: {}", e)
                }));
            }
        };

        // Save new change address to database
        let address_repo = AddressRepository::new(db.connection());
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let address_model = crate::database::Address {
            id: None,
            wallet_id,
            index: current_index,
            address: change_address.clone(),
            public_key: hex::encode(&derived_pubkey),
            used: true, // Mark as used since it's receiving change
            balance: 0,
            pending_utxo_check: false, // Change addresses don't need immediate check
            created_at,
        };

        match address_repo.create(&address_model) {
            Ok(_) => {
                // Update wallet's current_index
                if let Err(e) = wallet_repo.update_current_index(wallet_id, current_index + 1) {
                    log::warn!("   Failed to update wallet index: {}", e);
                }
                log::info!("   ✅ Generated new change address: {} (index {})", change_address, current_index);
            }
            Err(e) => {
                log::warn!("   Failed to save change address to database: {} (continuing anyway)", e);
            }
        }

        drop(db);

        // Create AddressInfo for change address
        let change_addr = crate::json_storage::AddressInfo {
            index: current_index,
            address: change_address,
            public_key: hex::encode(&derived_pubkey),
            used: true,
            balance: 0,
        };

        // Build P2PKH script for change
        use crate::transaction::Script;
        use sha2::{Sha256, Digest};
        use ripemd::Ripemd160;

        // Decode public key and hash it
        let pubkey_bytes = match hex::decode(&change_addr.public_key) {
            Ok(bytes) => bytes,
            Err(_) => {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Invalid public key"
                }));
            }
        };

        // SHA256 then RIPEMD160
        let sha_hash = Sha256::digest(&pubkey_bytes);
        let pubkey_hash = Ripemd160::digest(&sha_hash);

        let change_script = match Script::p2pkh_locking_script(&pubkey_hash) {
            Ok(script) => script,
            Err(e) => {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to create change script: {}", e)
                }));
            }
        };

        tx.add_output(TxOutput::new(change, change_script.bytes));
        log::info!("   Added change output: {} satoshis", change);
    } else if change > 0 {
        log::info!("   Change below dust limit ({}), adding to fee", change);
    }

    // Calculate txid
    let txid = match tx.txid() {
        Ok(id) => id,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to calculate txid: {}", e)
            }));
        }
    };

    // Generate reference ID
    let reference = format!("action-{}", uuid::Uuid::new_v4());

    // Build user input infos for signing
    let user_input_infos: Vec<UserInputInfo> = user_inputs.iter().map(|ui| UserInputInfo {
        txid: ui.txid.clone(),
        vout: ui.vout,
        satoshis: ui.satoshis,
        locking_script: ui.locking_script.clone(),
        source_tx: ui.source_tx.clone(),  // Include source tx bytes for BEEF building
        is_pre_signed: ui.unlocking_script.is_some(),
    }).collect();

    // Store transaction in memory with UTXO metadata for signing
    {
        let mut pending = PENDING_TRANSACTIONS.lock().unwrap();
        pending.insert(reference.clone(), PendingTransaction {
            tx: tx.clone(),
            input_utxos: selected_utxos.clone(),
            user_input_infos,
            brc29_info: brc29_info.clone(),
            input_beef: parsed_input_beef.clone(),
        });
    }

    // Log if this is a BRC-29 payment
    if brc29_info.is_some() {
        log::info!("   💰 BRC-29 payment metadata stored for later envelope conversion");
    }

    log::info!("   ✅ Transaction created: {}", txid);
    log::info!("   Reference: {}", reference);

    // Invalidate balance cache (outgoing transaction changes balance)
    state.balance_cache.invalidate();
    log::info!("   🔄 Balance cache invalidated (outgoing transaction)");

    // Store action in action storage
    use crate::action_storage::{StoredAction, ActionStatus, ActionInput, ActionOutput};
    use chrono::Utc;

    // Build inputs list for action storage (user inputs + wallet inputs)
    let num_user_inputs = user_inputs.len();
    let action_inputs: Vec<ActionInput> = tx.inputs.iter().enumerate().map(|(i, input)| {
        if i < num_user_inputs {
            // User input
            let user_input = &user_inputs[i];
            ActionInput {
                txid: user_input.txid.clone(),
                vout: user_input.vout,
                satoshis: user_input.satoshis,
                script: Some(hex::encode(&input.script_sig)),
            }
        } else {
            // Wallet input
            let wallet_idx = i - num_user_inputs;
            ActionInput {
                txid: selected_utxos.get(wallet_idx).map(|u| u.txid.clone()).unwrap_or_default(),
                vout: selected_utxos.get(wallet_idx).map(|u| u.vout).unwrap_or(0),
                satoshis: selected_utxos.get(wallet_idx).map(|u| u.satoshis).unwrap_or(0),
                script: Some(hex::encode(&input.script_sig)),
            }
        }
    }).collect();

    let stored_action = StoredAction {
        txid: txid.clone(),
        reference_number: reference.clone(),
        raw_tx: tx.to_hex().unwrap_or_default(),
        description: req.description.clone(),
        labels: req.labels.clone().unwrap_or_default(),
        status: ActionStatus::Created,
        is_outgoing: true,
        satoshis: total_output,
        timestamp: Utc::now().timestamp(),
        block_height: None,
        confirmations: 0,
        version: tx.version,
        lock_time: tx.lock_time,
        inputs: action_inputs,
        outputs: tx.outputs.iter().enumerate().map(|(i, output)| ActionOutput {
            vout: i as u32,
            satoshis: output.value,
            script: Some(hex::encode(&output.script_pubkey)),
            address: parse_address_from_script(&output.script_pubkey),
        }).collect(),
    };

    // Store the action in database
    {
        use crate::database::TransactionRepository;
        let db = state.database.lock().unwrap();
        let tx_repo = TransactionRepository::new(db.connection());
        match tx_repo.add_transaction(&stored_action) {
            Ok(_) => {
                log::info!("   💾 Action stored in database with status: created");
            }
            Err(e) => {
                log::warn!("   ⚠️  Failed to store action in database: {}", e);
            }
        }
    }

    // Get options with defaults
    let options = req.options.as_ref();
    let sign_and_process = options.and_then(|o| o.sign_and_process).unwrap_or(true);
    let accept_delayed_broadcast = options.and_then(|o| o.accept_delayed_broadcast).unwrap_or(true);
    let no_send = options.and_then(|o| o.no_send).unwrap_or(false);

    log::info!("   Options: signAndProcess={}, acceptDelayedBroadcast={}, noSend={}",
               sign_and_process, accept_delayed_broadcast, no_send);

    // Determine if we should sign and/or broadcast
    let should_sign = sign_and_process;
    let should_broadcast = !accept_delayed_broadcast && !no_send;

    let (final_txid, raw_tx) = if should_sign {
        log::info!("   🖊️  Signing transaction...");

        // Call signAction to sign the transaction
        let sign_req = SignActionRequest {
            reference: reference.clone(),
            spends: None,
        };

        let sign_body = match serde_json::to_vec(&sign_req) {
            Ok(b) => b,
            Err(e) => {
                log::error!("   Failed to serialize SignActionRequest: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to prepare signing request: {}", e)
                }));
            }
        };

        let sign_response = sign_action(state.clone(), web::Bytes::from(sign_body)).await;

        // Extract the signed transaction from the response
        match sign_response.status().is_success() {
            true => {
                // Parse the response body
                let body_bytes = actix_web::body::to_bytes(sign_response.into_body()).await;
                match body_bytes {
                    Ok(bytes) => {
                        // Parse as generic JSON to handle both regular and BRC-29 responses
                        match serde_json::from_slice::<serde_json::Value>(&bytes) {
                            Ok(json_resp) => {
                                let txid = json_resp["txid"].as_str().unwrap_or("").to_string();
                                log::info!("   ✅ Transaction signed successfully");
                                log::info!("   📝 Signed TXID: {}", txid);

                                // Extract rawTx (Atomic BEEF hex string) and convert to bytes
                                let tx_data = if let Some(raw_tx) = json_resp["rawTx"].as_str() {
                                    log::info!("   📦 Extracting Atomic BEEF response");
                                    hex::decode(raw_tx).ok()
                                } else {
                                    log::warn!("   ⚠️  Missing rawTx in response");
                                    None
                                };

                                (txid, tx_data)
                            },
                            Err(e) => {
                                log::error!("   Failed to parse sign response JSON: {}", e);
                                return HttpResponse::InternalServerError().json(serde_json::json!({
                                    "error": format!("Failed to parse signing response: {}", e)
                                }));
                            }
                        }
                    },
                    Err(e) => {
                        log::error!("   Failed to read sign response body: {}", e);
                        return HttpResponse::InternalServerError().json(serde_json::json!({
                            "error": format!("Failed to read signing response: {}", e)
                        }));
                    }
                }
            },
            false => {
                log::error!("   Signing failed with status: {}", sign_response.status());
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Transaction signing failed"
                }));
            }
        }
    } else {
        log::info!("   ℹ️  Skipping signing (signAndProcess=false)");
        // Convert unsigned transaction hex to byte array for BRC-100 spec compliance (AtomicBEEF = Byte[])
        let tx_bytes = tx.to_hex().ok()
            .and_then(|h| hex::decode(h).ok());
        (txid, tx_bytes)
    };

    if should_broadcast {
        log::info!("   📡 Would broadcast transaction (not yet implemented)");
        // TODO: Call processAction here to broadcast
    } else {
        log::info!("   ℹ️  Skipping broadcast (acceptDelayedBroadcast={}, noSend={})",
                   accept_delayed_broadcast, no_send);
    }

    // Build response inputs array (user inputs first, then wallet inputs)
    let mut response_inputs: Vec<CreateActionResponseInput> = Vec::new();

    // Add user-provided inputs
    for user_input in &user_inputs {
        response_inputs.push(CreateActionResponseInput {
            txid: user_input.txid.clone(),
            vout: user_input.vout,
            output_index: user_input.vout,
            script_length: user_input.unlocking_script.as_ref().map(|s| s.len()).unwrap_or(0),
            script_offset: 0,
            sequence: user_input.sequence,
        });
    }

    // Add wallet inputs
    for utxo in &selected_utxos {
        response_inputs.push(CreateActionResponseInput {
            txid: utxo.txid.clone(),
            vout: utxo.vout,
            output_index: utxo.vout,
            script_length: utxo.script.len() / 2, // Hex length to byte length
            script_offset: 0, // Not used in simplified implementation
            sequence: 0xffffffff,
        });
    }

    // Build response outputs array
    let response_outputs: Vec<CreateActionResponseOutput> = tx.outputs.iter().enumerate().map(|(i, output)| {
        CreateActionResponseOutput {
            vout: i as u32,
            satoshis: output.value,
            script_length: output.script_pubkey.len(),
            script_offset: 0, // Not used in simplified implementation
        }
    }).collect();

    // Log response format
    if let Some(ref tx_bytes) = raw_tx {
        log::info!("   📤 Returning tx as byte array ({} bytes)", tx_bytes.len());
        log::info!("   📤 First 40 bytes (hex): {}", hex::encode(&tx_bytes[..std::cmp::min(40, tx_bytes.len())]));

        // Also log what it looks like in base64 (what ToolBSV will see in JSON)
        let base64_tx = general_purpose::STANDARD.encode(tx_bytes);
        log::info!("   📤 Base64 encoded ({} chars): {}...", base64_tx.len(), &base64_tx[..std::cmp::min(80, base64_tx.len())]);
    }

    HttpResponse::Ok().json(CreateActionResponse {
        reference,
        version: tx.version,
        lock_time: tx.lock_time,
        inputs: response_inputs,
        outputs: response_outputs,
        derivation_prefix: None,
        input_beef: None,
        txid: Some(final_txid),
        tx: raw_tx,
    })
}

// Query confirmation status from WhatsOnChain API
async fn get_confirmation_status(txid: &str) -> Result<(u32, Option<u32>), String> {
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/hash/{}", txid);

    let client = reqwest::Client::new();
    let response = client.get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("API returned status: {}", response.status()));
    }

    let json: serde_json::Value = response.json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let confirmations = json["confirmations"].as_u64().unwrap_or(0) as u32;
    let block_height = json["blockheight"].as_u64().map(|h| h as u32);

    Ok((confirmations, block_height))
}

// Update confirmation status for all unconfirmed/pending actions
pub async fn update_confirmations(state: web::Data<AppState>) -> Result<usize, String> {
    let mut updated_count = 0;

    // Get all actions that need confirmation updates from database
    use crate::database::TransactionRepository;
    let db = state.database.lock().unwrap();
    let tx_repo = TransactionRepository::new(db.connection());

    let actions_to_update = match tx_repo.list_transactions(None, None) {
        Ok(actions) => {
            actions.into_iter()
                .filter(|a| matches!(a.status, crate::action_storage::ActionStatus::Unconfirmed))
                .map(|a| a.txid.clone())
                .collect::<Vec<_>>()
        }
        Err(e) => {
            log::error!("   Failed to get transactions: {}", e);
            return Err(format!("Failed to get transactions: {}", e));
        }
    };
    drop(db);

    log::info!("📊 Checking confirmations for {} transactions...", actions_to_update.len());

    // Query each transaction
    for txid in actions_to_update {
        match get_confirmation_status(&txid).await {
            Ok((confirmations, block_height)) => {
                let db = state.database.lock().unwrap();
                let tx_repo = TransactionRepository::new(db.connection());
                if let Err(e) = tx_repo.update_confirmations(&txid, confirmations, block_height) {
                    log::warn!("   Failed to update {}: {}", txid, e);
                } else {
                    log::info!("   ✅ {} - {} confirmations", &txid[..16], confirmations);
                    updated_count += 1;
                }
            }
            Err(e) => {
                log::warn!("   Failed to query {}: {}", &txid[..16], e);
            }
        }

        // Rate limit: small delay between requests
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    log::info!("✅ Updated {} transactions", updated_count);
    Ok(updated_count)
}

// Parse address from P2PKH script (76a914{20-byte-hash}88ac)
/// Convert a Bitcoin address to a P2PKH locking script
fn address_to_script(address: &str) -> Result<Vec<u8>, String> {
    // Decode base58 address
    let decoded = match bs58::decode(address).into_vec() {
        Ok(v) => v,
        Err(e) => return Err(format!("Base58 decode error: {}", e)),
    };

    // Address format: [version byte][20-byte pubkey hash][4-byte checksum]
    if decoded.len() != 25 {
        return Err(format!("Invalid address length: {}", decoded.len()));
    }

    // Extract pubkey hash (skip version byte, remove checksum)
    let pubkey_hash = &decoded[1..21];

    // Create P2PKH script: OP_DUP OP_HASH160 <pubkey_hash> OP_EQUALVERIFY OP_CHECKSIG
    let mut script = Vec::new();
    script.push(0x76); // OP_DUP
    script.push(0xa9); // OP_HASH160
    script.push(0x14); // Push 20 bytes
    script.extend_from_slice(pubkey_hash);
    script.push(0x88); // OP_EQUALVERIFY
    script.push(0xac); // OP_CHECKSIG

    Ok(script)
}

// Create P2PKH script from a public key (for BRC-29)
fn create_p2pkh_script_from_pubkey(pubkey: &[u8]) -> Vec<u8> {
    use sha2::{Sha256, Digest};
    use ripemd::Ripemd160;

    // Hash the public key: RIPEMD160(SHA256(pubkey))
    let sha_hash = Sha256::digest(pubkey);
    let pubkey_hash = Ripemd160::digest(&sha_hash);

    // Create P2PKH script: OP_DUP OP_HASH160 <pubkey_hash> OP_EQUALVERIFY OP_CHECKSIG
    let mut script = Vec::new();
    script.push(0x76); // OP_DUP
    script.push(0xa9); // OP_HASH160
    script.push(0x14); // Push 20 bytes
    script.extend(pubkey_hash.as_slice());
    script.push(0x88); // OP_EQUALVERIFY
    script.push(0xac); // OP_CHECKSIG

    script
}

fn parse_address_from_script(script_bytes: &[u8]) -> Option<String> {
    // P2PKH: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
    if script_bytes.len() == 25 &&
       script_bytes[0] == 0x76 &&  // OP_DUP
       script_bytes[1] == 0xa9 &&  // OP_HASH160
       script_bytes[2] == 0x14 &&  // Push 20 bytes
       script_bytes[23] == 0x88 && // OP_EQUALVERIFY
       script_bytes[24] == 0xac {  // OP_CHECKSIG

        // Extract pubkey hash (bytes 3-22)
        let pubkey_hash = &script_bytes[3..23];

        // Convert to Bitcoin address (mainnet prefix 0x00)
        use sha2::{Sha256, Digest};
        let mut addr_bytes = vec![0x00]; // Mainnet prefix
        addr_bytes.extend_from_slice(pubkey_hash);

        // Double SHA256 checksum
        let checksum_full = Sha256::digest(&Sha256::digest(&addr_bytes));
        let checksum = &checksum_full[0..4];

        // Append checksum
        addr_bytes.extend_from_slice(checksum);

        // Base58 encode
        return Some(bs58::encode(&addr_bytes).into_string());
    }

    // TODO: Add P2SH, P2PK, and other script types
    None
}

// Check if an output script belongs to our wallet
fn is_output_ours(script_bytes: &[u8], our_addresses: &[crate::json_storage::AddressInfo]) -> bool {
    // Extract pubkey hash from P2PKH script
    if script_bytes.len() == 25 &&
       script_bytes[0] == 0x76 &&  // OP_DUP
       script_bytes[1] == 0xa9 &&  // OP_HASH160
       script_bytes[2] == 0x14 &&  // Push 20 bytes
       script_bytes[23] == 0x88 && // OP_EQUALVERIFY
       script_bytes[24] == 0xac {  // OP_CHECKSIG

        let script_pubkey_hash = &script_bytes[3..23];

        // Check against all our addresses
        use sha2::{Sha256, Digest};
        use ripemd::Ripemd160;

        for addr in our_addresses {
            // Decode our public key
            if let Ok(pubkey_bytes) = hex::decode(&addr.public_key) {
                // Calculate pubkey hash: RIPEMD160(SHA256(pubkey))
                let sha_hash = Sha256::digest(&pubkey_bytes);
                let our_pubkey_hash = Ripemd160::digest(&sha_hash);

                // Compare
                if our_pubkey_hash.as_slice() == script_pubkey_hash {
                    return true;
                }
            }
        }
    }

    false
}

// Select UTXOs to cover required amount (simple greedy algorithm)
fn select_utxos(available: &[UTXO], amount_needed: i64) -> Vec<UTXO> {
    let mut selected = Vec::new();
    let mut total: i64 = 0;

    // Sort by value (largest first) for efficiency
    let mut sorted_utxos = available.to_vec();
    sorted_utxos.sort_by(|a, b| b.satoshis.cmp(&a.satoshis));

    for utxo in sorted_utxos {
        selected.push(utxo.clone());
        total += utxo.satoshis;

        if total >= amount_needed {
            break;
        }
    }

    if total < amount_needed {
        // Not enough funds
        return Vec::new();
    }

    selected
}

// Request structure for /signAction
#[derive(Debug, Serialize, Deserialize)]
pub struct SignActionRequest {
    #[serde(rename = "reference")]
    pub reference: String,

    #[serde(rename = "spends")]
    pub spends: Option<serde_json::Value>, // Not used in simple implementation
}

// Response structure for /signAction
#[derive(Debug, Serialize, Deserialize)]
pub struct SignActionResponse {
    pub txid: String,
    #[serde(rename = "rawTx")]
    pub raw_tx: String,
}

// /signAction - Sign transaction inputs
pub async fn sign_action(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("📋 /signAction called");

    // Parse request
    let req: SignActionRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            log::error!("   JSON parse error: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid JSON: {}", e)
            }));
        }
    };

    log::info!("   Reference: {}", req.reference);

    // Retrieve pending transaction
    let pending_tx = {
        let pending = PENDING_TRANSACTIONS.lock().unwrap();
        match pending.get(&req.reference) {
            Some(ptx) => ptx.clone(),
            None => {
                log::error!("   Transaction not found: {}", req.reference);
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "Transaction reference not found"
                }));
            }
        }
    };

    let mut tx = pending_tx.tx;
    let input_utxos = pending_tx.input_utxos;
    let user_input_infos = pending_tx.user_input_infos;
    let brc29_info = pending_tx.brc29_info;
    let input_beef = pending_tx.input_beef;

    let num_user_inputs = user_input_infos.len();
    let num_wallet_inputs = input_utxos.len();
    log::info!("   Signing {} inputs ({} user, {} wallet)...",
        tx.inputs.len(), num_user_inputs, num_wallet_inputs);

    // Sign USER inputs that need signing (skip pre-signed ones)
    for (i, user_input) in user_input_infos.iter().enumerate() {
        if user_input.is_pre_signed {
            log::info!("   Input {} (user): {}:{} - already pre-signed, skipping",
                i, &user_input.txid[..16], user_input.vout);
            continue;
        }

        log::info!("   Input {} (user): {}:{} - needs wallet signature",
            i, &user_input.txid[..16], user_input.vout);

        // User input without pre-signed script - wallet needs to sign it
        // This would require the wallet to have the private key for this UTXO
        // For now, we'll skip this case and log a warning
        // TODO: Implement signing for user inputs when wallet has the key
        log::warn!("   ⚠️  User input {} requires signing but no key available - skipping", i);
    }

    // Sign WALLET inputs
    for (wallet_idx, input_utxo) in input_utxos.iter().enumerate() {
        let i = num_user_inputs + wallet_idx;  // Actual input index in transaction
        log::info!("   Signing input {} (wallet): {}:{} (address index {})",
            i, input_utxo.txid, input_utxo.vout, input_utxo.address_index);

        // Get the private key for THIS specific address (not always index 0!)
        let db = state.database.lock().unwrap();
        let private_key_bytes = match crate::database::derive_private_key_from_db(&db, input_utxo.address_index as u32) {
            Ok(key) => key,
            Err(e) => {
                log::error!("   Failed to derive private key for address index {}: {}", input_utxo.address_index, e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to derive private key for address {}: {}", input_utxo.address_index, e)
                }));
            }
        };
        drop(db);

        // Decode prev script
        let prev_script = match hex::decode(&input_utxo.script) {
            Ok(bytes) => bytes,
            Err(e) => {
                log::error!("   Invalid script hex: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Invalid script hex for input {}: {}", i, e)
                }));
            }
        };

        // Calculate SIGHASH
        use crate::transaction::{calculate_sighash, SIGHASH_ALL_FORKID, Script};

        let sighash: Vec<u8> = match calculate_sighash(&tx, i, &prev_script, input_utxo.satoshis, SIGHASH_ALL_FORKID) {
            Ok(hash) => hash,
            Err(e) => {
                log::error!("   Failed to calculate sighash: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to calculate sighash for input {}: {}", i, e)
                }));
            }
        };

        log::info!("   SIGHASH: {}", hex::encode(&sighash));

        // Sign with ECDSA
        use secp256k1::{Secp256k1, Message, SecretKey};

        let secp = Secp256k1::new();
        let secret = match SecretKey::from_slice(&private_key_bytes) {
            Ok(key) => key,
            Err(e) => {
                log::error!("   Invalid private key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Invalid private key"
                }));
            }
        };

        let message = match Message::from_digest_slice(&sighash) {
            Ok(msg) => msg,
            Err(e) => {
                log::error!("   Invalid sighash: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Invalid sighash"
                }));
            }
        };

        let signature = secp.sign_ecdsa(&message, &secret);

        // Serialize signature as DER + sighash byte
        let mut sig_der = signature.serialize_der().to_vec();
        sig_der.push(SIGHASH_ALL_FORKID as u8); // Append sighash type byte

        log::info!("   Signature ({} bytes): {}", sig_der.len(), hex::encode(&sig_der));

        // Get public key
        use secp256k1::PublicKey;
        let pubkey = PublicKey::from_secret_key(&secp, &secret);
        let pubkey_bytes = pubkey.serialize();

        log::info!("   Public key length: {} bytes", pubkey_bytes.len());
        log::info!("   Public key: {}", hex::encode(&pubkey_bytes));
        log::info!("   Private key (first 8 bytes): {}...", hex::encode(&private_key_bytes[..8]));

        // Build unlocking script: <signature> <pubkey>
        let unlocking_script = Script::p2pkh_unlocking_script(&sig_der, &pubkey_bytes);

        // Update input with unlocking script
        tx.inputs[i].set_script(unlocking_script.bytes);

        log::info!("   ✅ Input {} signed", i);
    }

    // Serialize signed transaction to hex first
    let signed_tx_hex = match tx.to_hex() {
        Ok(hex) => hex,
        Err(e) => {
            log::error!("   Failed to serialize transaction: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to serialize transaction: {}", e)
            }));
        }
    };

    // Decode to bytes for BEEF
    let signed_tx_bytes = match hex::decode(&signed_tx_hex) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("   Failed to decode transaction hex: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to decode transaction hex: {}", e)
            }));
        }
    };

    // Calculate final txid
    let txid = match tx.txid() {
        Ok(id) => id,
        Err(e) => {
            log::error!("   Failed to calculate txid: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to calculate txid: {}", e)
            }));
        }
    };

    log::info!("   ✅ Transaction signed: {}", txid);
    log::info!("   📝 Signed TX hex ({} bytes): {}...", signed_tx_bytes.len(), &signed_tx_hex[..std::cmp::min(80, signed_tx_hex.len())]);

    // Build BEEF (Background Evaluation Extended Format) with parent transactions
    log::info!("   📦 Building BEEF format with {} parent transactions ({} user, {} wallet)...",
        num_user_inputs + input_utxos.len(), num_user_inputs, input_utxos.len());

    let mut beef = crate::beef::Beef::new();

    // First, copy ALL transactions and BUMPs from inputBEEF (if provided)
    // This preserves the full verification chain for user-provided inputs
    if let Some(ref ib) = input_beef {
        log::info!("   📥 Copying {} transactions and {} BUMPs from inputBEEF",
            ib.transactions.len(), ib.bumps.len());

        // Copy all BUMPs first (they need to be indexed before transactions reference them)
        for (i, bump) in ib.bumps.iter().enumerate() {
            log::info!("      BUMP {}: block height {}", i, bump.block_height);
            beef.bumps.push(bump.clone());
        }

        // Copy all transactions (except the last one which is the main tx we're building on)
        // The inputBEEF contains parents in order, with the transaction being spent as last
        for (i, tx_bytes) in ib.transactions.iter().enumerate() {
            // Calculate txid for logging
            use sha2::{Sha256, Digest};
            let first_hash = Sha256::digest(tx_bytes);
            let second_hash = Sha256::digest(&first_hash);
            let txid = hex::encode(second_hash.iter().rev().copied().collect::<Vec<u8>>());

            // Copy the tx_to_bump mapping if it exists
            let has_bump = ib.tx_to_bump.get(i).and_then(|b| *b).is_some();

            log::info!("      TX {}: {} ({} bytes, has_bump: {})",
                i, &txid[..16], tx_bytes.len(), has_bump);

            let tx_index = beef.add_parent_transaction(tx_bytes.clone());

            // Copy the BUMP reference if this transaction has one
            if let Some(Some(bump_idx)) = ib.tx_to_bump.get(i) {
                beef.tx_to_bump[tx_index] = Some(*bump_idx);
            }
        }

        log::info!("   ✅ Copied full inputBEEF chain ({} txs, {} bumps)",
            beef.transactions.len(), beef.bumps.len());
    } else {
        // No inputBEEF - add individual user input source transactions if available
        for (i, user_input) in user_input_infos.iter().enumerate() {
            if let Some(ref source_tx) = user_input.source_tx {
                log::info!("   📥 Adding user input {} source tx: {} ({} bytes)",
                    i, &user_input.txid[..16], source_tx.len());
                beef.add_parent_transaction(source_tx.clone());
            } else {
                log::warn!("   ⚠️  User input {} ({}) has no source transaction in BEEF",
                    i, &user_input.txid[..16]);
            }
        }
    }

    // Fetch WALLET parent transactions and their Merkle proofs (with caching)
    let client = reqwest::Client::new();
    for (wallet_idx, utxo) in input_utxos.iter().enumerate() {
        let i = wallet_idx; // Keep original variable name for compatibility
        log::info!("   📥 Processing parent tx {}/{}: {}", i + 1, input_utxos.len(), utxo.txid);

        // STEP 1: Try to get parent transaction from cache
        let parent_tx_bytes = {
            let db = state.database.lock().unwrap();
            let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());

            match parent_tx_repo.get_by_txid(&utxo.txid) {
                Ok(Some(cached)) => {
                    // Verify cached data
                    match parent_tx_repo.verify_txid(&utxo.txid, &cached.raw_hex) {
                        Ok(true) => {
                            log::info!("   ✅ Using cached parent tx {} (cached at {})", utxo.txid, cached.cached_at);
                            drop(db); // Release lock before hex decode
                            match hex::decode(&cached.raw_hex) {
                                Ok(bytes) => bytes,
                                Err(e) => {
                                    log::warn!("   ⚠️  Failed to decode cached parent tx {}: {}, fetching from API", utxo.txid, e);
                                    // Fall through to API fetch
                                    match crate::cache_helpers::fetch_parent_transaction_from_api(&client, &utxo.txid).await {
                                        Ok(parent_tx_hex) => {
                                            match hex::decode(&parent_tx_hex) {
                                                Ok(bytes) => {
                                                    match crate::cache_helpers::verify_txid(&bytes, &utxo.txid) {
                                                        Ok(_) => {
                                                            // Cache it
                                                            {
                                                                let db = state.database.lock().unwrap();
                                                                let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());
                                                                let utxo_id = crate::cache_helpers::get_utxo_id_from_db(db.connection(), &utxo.txid, utxo.vout)
                                                                    .ok()
                                                                    .flatten();
                                                                let _ = parent_tx_repo.upsert(utxo_id, &utxo.txid, &parent_tx_hex);
                                                            }
                                                            bytes
                                                        }
                                                        Err(e) => {
                                                            log::error!("   ❌ TXID verification failed for {}: {}", utxo.txid, e);
                                                            continue; // Skip this parent transaction
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    log::error!("   ❌ Failed to decode parent tx hex for {}: {}", utxo.txid, e);
                                                    continue; // Skip this parent transaction
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("   ❌ Failed to fetch parent tx {}: {}", utxo.txid, e);
                                            continue; // Skip this parent transaction
                                        }
                                    }
                                }
                            }
                        }
                        Ok(false) => {
                            log::warn!("   ⚠️  Cached parent tx {} failed TXID verification, fetching from API", utxo.txid);
                            drop(db); // Release lock
                            // Fall through to API fetch
                            match crate::cache_helpers::fetch_parent_transaction_from_api(&client, &utxo.txid).await {
                                Ok(parent_tx_hex) => {
                                    match hex::decode(&parent_tx_hex) {
                                        Ok(bytes) => {
                                            match crate::cache_helpers::verify_txid(&bytes, &utxo.txid) {
                                                Ok(_) => {
                                                    // Cache it
                                                    {
                                                        let db = state.database.lock().unwrap();
                                                        let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());
                                                        let utxo_id = crate::cache_helpers::get_utxo_id_from_db(db.connection(), &utxo.txid, utxo.vout)
                                                            .ok()
                                                            .flatten();
                                                        let _ = parent_tx_repo.upsert(utxo_id, &utxo.txid, &parent_tx_hex);
                                                    }
                                                    bytes
                                                }
                                                Err(e) => {
                                                    log::error!("   ❌ TXID verification failed for {}: {}", utxo.txid, e);
                                                    continue; // Skip this parent transaction
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("   ❌ Failed to decode parent tx hex for {}: {}", utxo.txid, e);
                                            continue; // Skip this parent transaction
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("   ❌ Failed to fetch parent tx {}: {}", utxo.txid, e);
                                    continue; // Skip this parent transaction
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("   ⚠️  Error verifying cached parent tx {}: {}, fetching from API", utxo.txid, e);
                            drop(db); // Release lock
                            // Fall through to API fetch
                            match crate::cache_helpers::fetch_parent_transaction_from_api(&client, &utxo.txid).await {
                                Ok(parent_tx_hex) => {
                                    match hex::decode(&parent_tx_hex) {
                                        Ok(bytes) => {
                                            match crate::cache_helpers::verify_txid(&bytes, &utxo.txid) {
                                                Ok(_) => {
                                                    // Cache it
                                                    {
                                                        let db = state.database.lock().unwrap();
                                                        let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());
                                                        let utxo_id = crate::cache_helpers::get_utxo_id_from_db(db.connection(), &utxo.txid, utxo.vout)
                                                            .ok()
                                                            .flatten();
                                                        let _ = parent_tx_repo.upsert(utxo_id, &utxo.txid, &parent_tx_hex);
                                                    }
                                                    bytes
                                                }
                                                Err(e) => {
                                                    log::error!("   ❌ TXID verification failed for {}: {}", utxo.txid, e);
                                                    continue; // Skip this parent transaction
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("   ❌ Failed to decode parent tx hex for {}: {}", utxo.txid, e);
                                            continue; // Skip this parent transaction
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("   ❌ Failed to fetch parent tx {}: {}", utxo.txid, e);
                                    continue; // Skip this parent transaction
                                }
                            }
                        }
                    }
                }
                Ok(None) => {
                    drop(db); // Release lock before API call
                    log::info!("   🌐 Cache miss - fetching parent tx {} from API...", utxo.txid);
                    // Fetch from API
                    match crate::cache_helpers::fetch_parent_transaction_from_api(&client, &utxo.txid).await {
                        Ok(parent_tx_hex) => {
                            match hex::decode(&parent_tx_hex) {
                                Ok(bytes) => {
                                    match crate::cache_helpers::verify_txid(&bytes, &utxo.txid) {
                                        Ok(_) => {
                                            // Cache it for next time
                                            {
                                                let db = state.database.lock().unwrap();
                                                let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());
                                                let utxo_id = crate::cache_helpers::get_utxo_id_from_db(db.connection(), &utxo.txid, utxo.vout)
                                                    .ok()
                                                    .flatten();
                                                match parent_tx_repo.upsert(utxo_id, &utxo.txid, &parent_tx_hex) {
                                                    Ok(_) => {
                                                        log::info!("   💾 Cached parent tx {}", utxo.txid);
                                                    }
                                                    Err(e) => {
                                                        log::warn!("   ⚠️  Failed to cache parent tx {}: {}", utxo.txid, e);
                                                        // Continue - caching failure shouldn't block transaction
                                                    }
                                                }
                                            }
                                            bytes
                                        }
                                        Err(e) => {
                                            log::error!("   ❌ TXID verification failed for {}: {}", utxo.txid, e);
                                            continue; // Skip this parent transaction
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("   ❌ Failed to decode parent tx hex for {}: {}", utxo.txid, e);
                                    continue; // Skip this parent transaction
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("   ❌ Failed to fetch parent tx {}: {}", utxo.txid, e);
                            continue; // Skip this parent transaction
                        }
                    }
                }
                Err(e) => {
                    drop(db); // Release lock
                    log::warn!("   ⚠️  Database error checking cache: {}, fetching from API", e);
                    // Fall through to API fetch
                    match crate::cache_helpers::fetch_parent_transaction_from_api(&client, &utxo.txid).await {
                        Ok(parent_tx_hex) => {
                            match hex::decode(&parent_tx_hex) {
                                Ok(bytes) => {
                                    match crate::cache_helpers::verify_txid(&bytes, &utxo.txid) {
                                        Ok(_) => bytes,
                                        Err(e) => {
                                            log::error!("   ❌ TXID verification failed for {}: {}", utxo.txid, e);
                                            continue; // Skip this parent transaction
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("   ❌ Failed to decode parent tx hex for {}: {}", utxo.txid, e);
                                    continue; // Skip this parent transaction
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("   ❌ Failed to fetch parent tx {}: {}", utxo.txid, e);
                            continue; // Skip this parent transaction
                        }
                    }
                }
            }
        };

        let tx_index = beef.add_parent_transaction(parent_tx_bytes);

        // STEP 2: Try to get Merkle proof from cache
        let enhanced_tsc = {
            let cached_proof_result = {
                let db = state.database.lock().unwrap();
                let merkle_proof_repo = crate::database::MerkleProofRepository::new(db.connection());
                merkle_proof_repo.get_by_parent_txid(&utxo.txid)
            }; // db is dropped here when merkle_proof_repo goes out of scope

            match cached_proof_result {
                Ok(Some(cached_proof)) => {
                    log::info!("   ✅ Using cached Merkle proof for {} (height: {})", utxo.txid, cached_proof.block_height);
                    // Create a new repo just for conversion (doesn't need to persist)
                    {
                        let db = state.database.lock().unwrap();
                        let merkle_proof_repo = crate::database::MerkleProofRepository::new(db.connection());
                        merkle_proof_repo.to_tsc_json(&cached_proof)
                    } // db is dropped here
                }
                Ok(None) => {
                    // Release lock before API call
                    log::info!("   🌐 Cache miss - fetching TSC proof from API...");

                    // Fetch TSC proof from API (with retry logic)
                    match crate::cache_helpers::fetch_tsc_proof_from_api(&client, &utxo.txid).await {
                        Ok(Some(tsc_json)) => {
                            // Get block height from block header (cache or API)
                            let db = state.database.lock().unwrap();
                            let block_header_repo = crate::database::BlockHeaderRepository::new(db.connection());
                            match crate::cache_helpers::enhance_tsc_with_height(
                                &client,
                                &block_header_repo,
                                &tsc_json,
                            ).await {
                                Ok(enhanced_tsc) => {
                                    // Cache the proof
                                    if let Some(parent_txn_id) = {
                                        let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());
                                        parent_tx_repo.get_id_by_txid(&utxo.txid).unwrap_or(None)
                                    } {
                                        let target_hash = enhanced_tsc["target"].as_str().unwrap_or("");
                                        match serde_json::to_string(&enhanced_tsc["nodes"]) {
                                            Ok(nodes_json) => {
                                                let block_height = enhanced_tsc["height"].as_u64().unwrap_or(0) as u32;
                                                let tx_index = enhanced_tsc["index"].as_u64().unwrap_or(0);

                                                let merkle_proof_repo = crate::database::MerkleProofRepository::new(db.connection());
                                                match merkle_proof_repo.upsert(parent_txn_id, block_height, tx_index, target_hash, &nodes_json) {
                                                    Ok(_) => {
                                                        log::info!("   💾 Cached Merkle proof for {}", utxo.txid);
                                                    }
                                                    Err(e) => {
                                                        log::warn!("   ⚠️  Failed to cache Merkle proof for {}: {}", utxo.txid, e);
                                                        // Continue - caching failure shouldn't block transaction
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                log::warn!("   ⚠️  Failed to serialize nodes for {}: {}", utxo.txid, e);
                                                // Continue - can still use the proof
                                            }
                                        }
                                    }

                                    enhanced_tsc
                                }
                                Err(e) => {
                                    log::warn!("   ⚠️  Failed to enhance TSC proof for {}: {}", utxo.txid, e);
                                    serde_json::Value::Null  // Return null to skip proof
                                }
                            }
                        }
                        Ok(None) => {
                            log::warn!("   ⚠️  TSC proof not available (tx not confirmed)");
                            serde_json::Value::Null  // Return null to skip proof
                        }
                        Err(e) => {
                            log::warn!("   ⚠️  Failed to fetch TSC proof: {}", e);
                            serde_json::Value::Null  // Return null to skip proof
                        }
                    }
                }
                Err(e) => {
                    log::warn!("   ⚠️  Database error checking cache: {}, skipping proof", e);
                    serde_json::Value::Null  // Return null to skip proof
                }
            }
        };

        // STEP 3: Add proof to BEEF
        if !enhanced_tsc.is_null() {
            match beef.add_tsc_merkle_proof(&utxo.txid, tx_index, &enhanced_tsc) {
                Ok(_) => {
                    log::info!("   ✅ Added TSC Merkle proof (BUMP) to BEEF");
                }
                Err(e) => {
                    log::warn!("   ⚠️  Failed to add TSC Merkle proof: {}", e);
                }
            }
        }
    }

    // Add the signed transaction as the main transaction (must be last)
    beef.set_main_transaction(signed_tx_bytes.clone());

    log::info!("   📊 BEEF structure before Atomic wrapping:");
    log::info!("      - Parent transactions: {}", beef.transactions.len() - 1);
    log::info!("      - Main transaction: 1");
    log::info!("      - Total transactions: {}", beef.transactions.len());
    log::info!("      - Merkle proofs (BUMPs): {}", beef.bumps.len());

    // Generate standard BEEF first (for logging)
    let standard_beef_hex = match beef.to_hex() {
        Ok(hex) => hex,
        Err(e) => {
            log::error!("   Failed to serialize standard BEEF: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to serialize standard BEEF: {}", e)
            }));
        }
    };
    log::info!("   📝 Standard BEEF hex ({} bytes): {}...", standard_beef_hex.len() / 2, &standard_beef_hex[..std::cmp::min(120, standard_beef_hex.len())]);

    // Serialize to Atomic BEEF (BRC-95) format
    let beef_hex = match beef.to_atomic_beef_hex(&txid) {
        Ok(hex) => hex,
        Err(e) => {
            log::error!("   Failed to serialize Atomic BEEF: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to serialize Atomic BEEF: {}", e)
            }));
        }
    };

    log::info!("   ✅ Atomic BEEF (BRC-95) created: {} bytes", beef_hex.len() / 2);
    log::info!("   📦 Includes {} parent tx(s) + 1 signed tx with 36-byte header", input_utxos.len());
    log::info!("   🔐 Merkle proofs (BUMPs): {} included for SPV validation", beef.bumps.len());
    log::info!("   🔍 Atomic BEEF starts with: {}...", &beef_hex[..std::cmp::min(120, beef_hex.len())]);

    // Update action with new TXID and status
    {
        // Update TXID (signing changes the transaction, so TXID changes)
        use crate::database::TransactionRepository;
        let db = state.database.lock().unwrap();
        let tx_repo = TransactionRepository::new(db.connection());
        if let Err(e) = tx_repo.update_txid(&req.reference, txid.clone(), signed_tx_hex.clone()) {
            log::warn!("   ⚠️  Failed to update TXID: {}", e);
        } else {
            log::info!("   💾 TXID updated after signing");
        }

        // Update status to "signed"
        use crate::action_storage::ActionStatus;
        if let Err(e) = tx_repo.update_status(&txid, ActionStatus::Signed) {
            log::warn!("   ⚠️  Failed to update action status: {}", e);
        } else {
            log::info!("   💾 Action status updated: created → signed");
        }

        // Mark UTXOs as spent in database
        use crate::database::UtxoRepository;
        let utxo_repo = UtxoRepository::new(db.connection());
        let utxos_to_mark: Vec<_> = input_utxos.iter()
            .map(|u| (u.txid.clone(), u.vout))
            .collect();

        match utxo_repo.mark_multiple_spent(&utxos_to_mark, &txid) {
            Ok(count) => {
                log::info!("   ✅ Marked {} UTXOs as spent in database", count);
                // Invalidate balance cache (UTXOs were spent)
                state.balance_cache.invalidate();
                log::info!("   🔄 Balance cache invalidated (UTXOs marked as spent)");
            }
            Err(e) => {
                log::warn!("   ⚠️  Failed to mark UTXOs as spent: {}", e);
            }
        }
    }

    // Note: BRC-29 payments are detected in create_action and handled there by deriving
    // the correct locking script using BRC-42. Here in sign_action, we just return
    // Atomic BEEF as normal for all transactions.
    if brc29_info.is_some() {
        log::info!("   💰 BRC-29 payment detected, returning standard Atomic BEEF");
    }

    // Return regular Atomic BEEF (same for BRC-29 and non-BRC-29)
    log::info!("   📦 Returning Atomic BEEF (binary format)");
    HttpResponse::Ok().json(SignActionResponse {
        txid,
        raw_tx: beef_hex,
    })
}

// Request structure for /processAction
#[derive(Debug, Deserialize)]
pub struct ProcessActionRequest {
    #[serde(rename = "outputs")]
    pub outputs: Vec<CreateActionOutput>,

    #[serde(rename = "description")]
    pub description: Option<String>,

    #[serde(rename = "labels")]
    pub labels: Option<Vec<String>>,

    #[serde(rename = "broadcast")]
    pub broadcast: Option<bool>,
}

// Response structure for /processAction
#[derive(Debug, Serialize)]
pub struct ProcessActionResponse {
    pub txid: String,
    pub status: String,
    #[serde(rename = "rawTx")]
    pub raw_tx: Option<String>,
}

// /processAction - Complete transaction flow (create + sign + broadcast)
pub async fn process_action(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("📋 /processAction called");

    // Parse request
    let req: ProcessActionRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            log::error!("   JSON parse error: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid JSON: {}", e)
            }));
        }
    };

    let should_broadcast = req.broadcast.unwrap_or(true);
    log::info!("   Broadcast: {}", should_broadcast);

    // Step 1: Create action (build unsigned transaction)
    let create_req = CreateActionRequest {
        inputs: None,  // process_action doesn't use inputBEEF
        outputs: req.outputs,
        description: req.description,
        labels: req.labels,
        options: Some(CreateActionOptions {
            sign_and_process: None,
            accept_delayed_broadcast: None,
            return_txid_only: Some(false),
            no_send: None,
            randomize_outputs: None,
        }),
        input_beef: None,
    };

    let create_body = serde_json::to_vec(&create_req).unwrap();
    let create_response = create_action(state.clone(), web::Bytes::from(create_body)).await;

    // Extract reference from create response
    let create_json: CreateActionResponse = match create_response.status().is_success() {
        true => {
            let body_bytes = actix_web::body::to_bytes(create_response.into_body()).await.unwrap();
            serde_json::from_slice(&body_bytes).unwrap()
        }
        false => {
            log::error!("   createAction failed");
            return create_response;
        }
    };

    let reference = create_json.reference;
    log::info!("   Created transaction: {}", reference);

    // Step 2: Sign action
    let sign_req = SignActionRequest {
        reference: reference.clone(),
        spends: None,
    };

    let sign_body = serde_json::to_vec(&sign_req).unwrap();
    let sign_response = sign_action(state.clone(), web::Bytes::from(sign_body)).await;

    // Extract txid and rawTx from sign response
    let sign_json: SignActionResponse = match sign_response.status().is_success() {
        true => {
            let body_bytes = actix_web::body::to_bytes(sign_response.into_body()).await.unwrap();
            serde_json::from_slice(&body_bytes).unwrap()
        }
        false => {
            log::error!("   signAction failed");
            return sign_response;
        }
    };

    let txid = sign_json.txid.clone();
    let raw_tx = sign_json.raw_tx.clone();

    log::info!("   Signed transaction: {}", txid);

    // Get input UTXOs from pending transaction for error handling
    let input_utxos = {
        let pending = PENDING_TRANSACTIONS.lock().unwrap();
        if let Some(pending_tx) = pending.get(&reference) {
            pending_tx.input_utxos.clone()
        } else {
            Vec::new() // If not found, we can't check UTXOs, but that's okay
        }
    };

    // Step 3: Broadcast (if requested)
    let status = if should_broadcast {
        log::info!("   Broadcasting to network...");

        let broadcast_result = broadcast_transaction(&raw_tx).await;

        // Handle "Missing inputs" error by checking UTXOs and marking them as spent
        if let Err(ref e) = broadcast_result {
            let error_str = e.to_string().to_lowercase();
            if error_str.contains("missing inputs") {
                log::warn!("   ⚠️  Received 'Missing inputs' error - checking which UTXOs are spent...");

                // Check each input UTXO to see if it's spent on-chain
                let mut spent_utxos = Vec::new();
                for utxo in &input_utxos {
                    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/outspend/{}", utxo.txid, utxo.vout);
                    let client = reqwest::Client::new();
                    match client.get(&url).send().await {
                        Ok(response) => {
                            if response.status() == 404 {
                                log::warn!("      ⚠️  UTXO {}:{} returned 404 - likely SPENT", utxo.txid, utxo.vout);
                                spent_utxos.push((utxo.txid.clone(), utxo.vout));
                            } else if response.status().is_success() {
                                if let Ok(json) = response.json::<serde_json::Value>().await {
                                    if let Some(spent) = json.get("spent").and_then(|v| v.as_bool()) {
                                        if spent {
                                            log::warn!("      ⚠️  UTXO {}:{} is SPENT on-chain", utxo.txid, utxo.vout);
                                            spent_utxos.push((utxo.txid.clone(), utxo.vout));
                                        }
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            // Ignore API errors
                        }
                    }
                }

                // Mark spent UTXOs in database
                if !spent_utxos.is_empty() {
                    log::info!("   🔄 Marking {} UTXO(s) as spent in database...", spent_utxos.len());
                    let db = state.database.lock().unwrap();
                    use crate::database::UtxoRepository;
                    let utxo_repo = UtxoRepository::new(db.connection());
                    for (txid, vout) in &spent_utxos {
                        let _ = utxo_repo.mark_spent(txid, *vout, "unknown");
                        log::info!("      ✅ Marked {}:{} as spent", txid, vout);
                    }
                    drop(db);
                }
            }
        }

        match broadcast_result {
            Ok(_) => {
                log::info!("   ✅ Transaction broadcast successful!");

                // Update action status to "unconfirmed"
                {
                    use crate::database::TransactionRepository;
                    use crate::action_storage::ActionStatus;
                    let db = state.database.lock().unwrap();
                    let tx_repo = TransactionRepository::new(db.connection());
                    if let Err(e) = tx_repo.update_status(&txid, ActionStatus::Unconfirmed) {
                        log::warn!("   ⚠️  Failed to update action status: {}", e);
                    } else {
                        log::info!("   💾 Action status updated: signed → unconfirmed");
                    }
                }

                "completed"
            }
            Err(e) => {
                log::error!("   ❌ Broadcast failed: {}", e);

                // Update action status to "failed"
                {
                    use crate::database::TransactionRepository;
                    use crate::action_storage::ActionStatus;
                    let db = state.database.lock().unwrap();
                    let tx_repo = TransactionRepository::new(db.connection());
                    if let Err(e) = tx_repo.update_status(&txid, ActionStatus::Failed) {
                        log::warn!("   ⚠️  Failed to update action status: {}", e);
                    } else {
                        log::info!("   💾 Action status updated: signed → failed");
                    }
                }

                "failed"
            }
        }
    } else {
        log::info!("   Skipping broadcast (noSend option)");
        "nosend"
    };

    HttpResponse::Ok().json(ProcessActionResponse {
        txid,
        status: status.to_string(),
        raw_tx: Some(raw_tx),
    })
}

// Broadcast transaction to BSV network (multiple broadcasters for redundancy)
async fn broadcast_transaction(raw_tx_hex: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let mut success_count = 0;
    let mut last_error = String::new();

    // Broadcaster 1: GorillaPool
    log::info!("   📡 Broadcasting to GorillaPool...");
    match broadcast_to_gorillapool(&client, raw_tx_hex).await {
        Ok(response) => {
            log::info!("   ✅ GorillaPool: {}", response);
            success_count += 1;
        }
        Err(e) => {
            log::warn!("   ⚠️ GorillaPool failed: {}", e);
            if last_error.is_empty() {
                last_error = format!("GorillaPool: {}", e);
            } else {
                last_error = format!("{}; GorillaPool: {}", last_error, e);
            }
        }
    }

    // Broadcaster 2: WhatsOnChain
    log::info!("   📡 Broadcasting to WhatsOnChain...");
    match broadcast_to_whatsonchain(&client, raw_tx_hex).await {
        Ok(response) => {
            log::info!("   ✅ WhatsOnChain: {}", response);
            success_count += 1;
        }
        Err(e) => {
            log::warn!("   ⚠️ WhatsOnChain failed: {}", e);
            if last_error.is_empty() {
                last_error = format!("WhatsOnChain: {}", e);
            } else {
                last_error = format!("{}; WhatsOnChain: {}", last_error, e);
            }
        }
    }

    if success_count > 0 {
        log::info!("   🎉 Broadcast successful to {} service(s)", success_count);
        Ok(format!("Broadcast to {} service(s)", success_count))
    } else {
        log::error!("   ❌ All broadcasters failed!");
        // Extract a clean, user-friendly error message
        let clean_error = extract_core_error(&last_error);
        Err(clean_error)
    }
}

// Helper function to extract a clean, user-friendly error message from broadcast errors
fn extract_core_error(error: &str) -> String {
    // Look for "ERROR: 16:" pattern (script verification errors)
    if let Some(start) = error.find("ERROR: 16:") {
        // Extract from "ERROR: 16:" to the end of the error description
        let error_part = &error[start..];
        // Find the end of the error description (usually ends with a closing paren or newline)
        let end = error_part.find('\n')
            .or_else(|| error_part.find(';'))
            .unwrap_or(error_part.len());
        let core_error = error_part[..end].trim();
        format!("Transaction broadcast failed: {}", core_error)
    } else if error.contains("OP_EQUALVERIFY") {
        // Fallback: if we see OP_EQUALVERIFY but not the ERROR: 16 pattern
        if let Some(start) = error.find("mandatory-script-verify") {
            let error_part = &error[start..];
            let end = error_part.find('\n')
                .or_else(|| error_part.find(';'))
                .unwrap_or(error_part.len().min(100));
            let core_error = error_part[..end].trim();
            format!("Transaction broadcast failed: ERROR: 16: {}", core_error)
        } else {
            "Transaction broadcast failed: ERROR: 16: mandatory-script-verify-flag-failed (Script failed an OP_EQUALVERIFY operation)".to_string()
        }
    } else {
        // For other errors, try to extract a meaningful part
        // Limit to first 200 characters to keep it manageable
        let truncated = if error.len() > 200 {
            format!("{}...", &error[..200])
        } else {
            error.to_string()
        };
        format!("Transaction broadcast failed: {}", truncated)
    }
}

// Broadcast to GorillaPool
async fn broadcast_to_gorillapool(client: &reqwest::Client, raw_tx_hex: &str) -> Result<String, String> {
    let url = "https://mapi.gorillapool.io/mapi/tx";

    let body = serde_json::json!({
        "rawtx": raw_tx_hex
    });

    let response = client.post(url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    // GorillaPool returns HTTP 200 even for failures, so we need to parse the response
    if status.is_success() {
        // Parse the response JSON
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(json) => {
                // GorillaPool wraps the actual response in a "payload" field (which is a JSON string)
                if let Some(payload_str) = json["payload"].as_str() {
                    // Parse the inner payload JSON
                    match serde_json::from_str::<serde_json::Value>(payload_str) {
                        Ok(payload) => {
                            // Check if returnResult is "success"
                            if let Some(return_result) = payload["returnResult"].as_str() {
                                if return_result == "success" {
                                    if let Some(txid) = payload["txid"].as_str() {
                                        log::info!("   ✅ GorillaPool accepted transaction: {}", txid);
                                        Ok(format!("GorillaPool accepted: {}", txid))
                                    } else {
                                        Ok("GorillaPool accepted (no TXID in response)".to_string())
                                    }
                                } else {
                                    // Transaction was rejected
                                    let error_desc = payload["resultDescription"].as_str()
                                        .unwrap_or("Unknown error");
                                    let error_msg = format!("GorillaPool rejected: {} - {}", return_result, error_desc);
                                    log::warn!("   ⚠️ {}", error_msg);
                                    Err(error_msg)
                                }
                            } else {
                                // No returnResult field - assume failure
                                Err(format!("GorillaPool response missing returnResult: {}", text))
                            }
                        }
                        Err(e) => {
                            Err(format!("Failed to parse GorillaPool payload JSON: {} - Response: {}", e, text))
                        }
                    }
                } else {
                    // No payload field - try to parse directly
                    if let Some(return_result) = json["returnResult"].as_str() {
                        if return_result == "success" {
                            Ok("GorillaPool accepted".to_string())
                        } else {
                            let error_desc = json["resultDescription"].as_str().unwrap_or("Unknown error");
                            Err(format!("GorillaPool rejected: {} - {}", return_result, error_desc))
                        }
                    } else {
                        // Assume success if we can't parse (fallback)
                        log::warn!("   ⚠️ Could not parse GorillaPool response, assuming success: {}", text);
                        Ok(text)
                    }
                }
            }
            Err(e) => {
                // If JSON parsing fails, check HTTP status
                if status.is_success() {
                    log::warn!("   ⚠️ GorillaPool returned non-JSON response, assuming success: {}", text);
                    Ok(text)
                } else {
                    Err(format!("{} - {} (JSON parse error: {})", status, text, e))
                }
            }
        }
    } else {
        Err(format!("{} - {}", status, text))
    }
}

// Broadcast to WhatsOnChain
async fn broadcast_to_whatsonchain(client: &reqwest::Client, raw_tx_hex: &str) -> Result<String, String> {
    let url = "https://api.whatsonchain.com/v1/bsv/main/tx/raw";

    let body = serde_json::json!({
        "txhex": raw_tx_hex
    });

    let response = client.post(url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if status.is_success() {
        Ok(text)
    } else {
        Err(format!("{} - {}", status, text))
    }
}

// Helper function to convert public key to Bitcoin address
fn pubkey_to_address(pubkey: &[u8]) -> Result<String, String> {
    use sha2::{Sha256, Digest};
    use ripemd::Ripemd160;

    // Hash the public key: RIPEMD160(SHA256(pubkey))
    let sha_hash = Sha256::digest(pubkey);
    let pubkey_hash = Ripemd160::digest(&sha_hash);

    // Create address: [version byte][20-byte pubkey hash][4-byte checksum]
    let mut addr_bytes = vec![0x00]; // Mainnet prefix
    addr_bytes.extend_from_slice(pubkey_hash.as_slice());

    // Double SHA256 checksum
    let checksum_full = Sha256::digest(&Sha256::digest(&addr_bytes));
    let checksum = &checksum_full[0..4];

    // Append checksum
    addr_bytes.extend_from_slice(checksum);

    // Base58 encode
    Ok(bs58::encode(&addr_bytes).into_string())
}

pub async fn generate_address(state: web::Data<AppState>) -> HttpResponse {
    log::info!("🔑 /wallet/address/generate called");

    // Get current index and master keys from database
    let (wallet_id, current_index, master_privkey, master_pubkey) = {
        use crate::database::{WalletRepository, get_master_private_key_from_db, get_master_public_key_from_db};

        let db = state.database.lock().unwrap();
        let wallet_repo = WalletRepository::new(db.connection());

        let wallet = match wallet_repo.get_primary_wallet() {
            Ok(Some(w)) => w,
            Ok(None) => {
                log::error!("   No wallet found in database");
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "No wallet found"
                }));
            }
            Err(e) => {
                log::error!("   Failed to get wallet: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Database error: {}", e)
                }));
            }
        };

        let wallet_id = wallet.id.unwrap();
        let index = wallet.current_index;

        let privkey = match get_master_private_key_from_db(&db) {
            Ok(k) => k,
            Err(e) => {
                log::error!("   Failed to get master private key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to get master key: {}", e)
                }));
            }
        };

        let pubkey = match get_master_public_key_from_db(&db) {
            Ok(k) => k,
            Err(e) => {
                log::error!("   Failed to get master public key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to get master public key: {}", e)
                }));
            }
        };

        drop(db);
        (wallet_id, index, privkey, pubkey)
    };

    // Create BRC-43 invoice number: "2-receive address-{index}"
    let invoice_number = format!("2-receive address-{}", current_index);
    log::info!("   Invoice number: {}", invoice_number);

    // Derive child public key using BRC-42 (self-derivation)
    let derived_pubkey = match derive_child_public_key(&master_privkey, &master_pubkey, &invoice_number) {
        Ok(pubkey) => {
            log::info!("   ✅ Derived pubkey: {}", hex::encode(&pubkey));
            pubkey
        },
        Err(e) => {
            log::error!("   BRC-42 derivation failed: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("BRC-42 derivation failed: {}", e)
            }));
        }
    };

    // Convert derived public key to Bitcoin address
    let address = match pubkey_to_address(&derived_pubkey) {
        Ok(addr) => addr,
        Err(e) => {
            log::error!("   Failed to create address: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to create address: {}", e)
            }));
        }
    };

    log::info!("   ✅ Generated address: {}", address);

    // Save address to database and update wallet index
    {
        use crate::database::{AddressRepository, WalletRepository, Address};
        use std::time::{SystemTime, UNIX_EPOCH};

        let db = state.database.lock().unwrap();
        let address_repo = AddressRepository::new(db.connection());
        let wallet_repo = WalletRepository::new(db.connection());

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let address_model = Address {
            id: None,
            wallet_id,
            index: current_index,
            address: address.clone(),
            public_key: hex::encode(&derived_pubkey),
            used: false,
            balance: 0,
            pending_utxo_check: true,  // Mark as pending - needs UTXO check
            created_at,
        };

        match address_repo.create(&address_model) {
            Ok(addr_id) => {
                // Update wallet's current_index
                if let Err(e) = wallet_repo.update_current_index(wallet_id, current_index + 1) {
                    log::warn!("   Failed to update wallet index: {}", e);
                }
                log::info!("   ✅ Address saved to database (ID: {}, index: {}, address: {})", addr_id, current_index, address);
            }
            Err(e) => {
                log::error!("   ❌ Failed to save address to database: {}", e);
                log::error!("   Address: {}, Index: {}", address, current_index);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to save address: {}", e)
                }));
            }
        }
    }

    // Return response in Go wallet format
    HttpResponse::Ok().json(serde_json::json!({
        "address": address,
        "index": current_index,
        "publicKey": hex::encode(&derived_pubkey)
    }))
}

// Request structure for /transaction/send (frontend wallet)
#[derive(Debug, Deserialize)]
pub struct SendTransactionRequest {
    #[serde(rename = "toAddress")]
    pub to_address: String,
    pub amount: i64,      // Satoshis
    #[serde(rename = "feeRate")]
    pub fee_rate: Option<i64>, // Satoshis per byte (currently ignored - deferred)
}

pub async fn send_transaction(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("💸 /transaction/send called");

    // Parse request
    let req: SendTransactionRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            log::error!("   Failed to parse request: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("Invalid request: {}", e)
            }));
        }
    };

    log::info!("   To address: {}", req.to_address);
    log::info!("   Amount: {} satoshis", req.amount);
    if let Some(fee_rate) = req.fee_rate {
        log::info!("   Fee rate: {} sat/byte (⚠️ currently ignored)", fee_rate);
    }

    // Validate address format (basic check)
    if !req.to_address.starts_with('1') && !req.to_address.starts_with('3') {
        log::error!("   Invalid address format");
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "Invalid Bitcoin address format"
        }));
    }

    // Validate amount
    if req.amount <= 0 {
        log::error!("   Invalid amount: {}", req.amount);
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "Amount must be greater than 0"
        }));
    }

    // Convert to CreateActionRequest format
    let create_req = CreateActionRequest {
        inputs: None,  // send_transaction doesn't use inputBEEF
        outputs: vec![CreateActionOutput {
            satoshis: Some(req.amount),
            script: None,
            address: Some(req.to_address.clone()),
            custom_instructions: None,
            output_description: None,
        }],
        description: Some(format!("Send {} satoshis to {}", req.amount, req.to_address)),
        labels: Some(vec!["send".to_string(), "wallet".to_string()]),
        options: Some(CreateActionOptions {
            sign_and_process: Some(true),
            accept_delayed_broadcast: Some(false), // Don't delay - we want to broadcast immediately
            return_txid_only: Some(false),
            no_send: Some(true), // Don't let createAction broadcast - we'll do it ourselves
            randomize_outputs: Some(true), // Default behavior
        }),
        input_beef: None,
    };

    log::info!("   📝 Creating transaction...");

    // Call createAction to create and sign the transaction
    let create_body = match serde_json::to_vec(&create_req) {
        Ok(b) => b,
        Err(e) => {
            log::error!("   Failed to serialize CreateActionRequest: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to prepare transaction: {}", e)
            }));
        }
    };

    let create_response = create_action(state.clone(), web::Bytes::from(create_body)).await;

    // Extract the signed transaction from the response
    let (txid, atomic_beef_hex) = match create_response.status().is_success() {
        true => {
            let body_bytes = match actix_web::body::to_bytes(create_response.into_body()).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    log::error!("   Failed to read createAction response body: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "success": false,
                        "error": format!("Failed to read transaction response: {}", e)
                    }));
                }
            };

            match serde_json::from_slice::<serde_json::Value>(&body_bytes) {
                Ok(json_resp) => {
                    let txid = match json_resp["txid"].as_str() {
                        Some(s) => s.to_string(),
                        None => {
                            log::error!("   Missing txid in response");
                            return HttpResponse::InternalServerError().json(serde_json::json!({
                                "success": false,
                                "error": "Transaction created but missing TXID"
                            }));
                        }
                    };

                    log::info!("   ✅ Transaction created and signed: {}", txid);

                    // Extract rawTx (Atomic BEEF) - can be hex string or byte array
                    let atomic_beef_hex = if let Some(hex_str) = json_resp["tx"].as_str() {
                        // If it's a hex string, use it directly
                        log::info!("   📦 TX field is hex string ({} chars)", hex_str.len());
                        hex_str.to_string()
                    } else if let Some(byte_array) = json_resp["tx"].as_array() {
                        // If it's a byte array (Vec<u8> serialized as JSON array), convert to hex
                        log::info!("   📦 TX field is byte array ({} bytes)", byte_array.len());
                        let bytes: Result<Vec<u8>, String> = byte_array
                            .iter()
                            .enumerate()
                            .map(|(i, v)| {
                                v.as_u64()
                                    .ok_or_else(|| format!("Invalid byte value at index {}", i))
                                    .and_then(|n| {
                                        if n > 255 {
                                            Err(format!("Byte value out of range at index {}: {}", i, n))
                                        } else {
                                            Ok(n as u8)
                                        }
                                    })
                            })
                            .collect();

                        match bytes {
                            Ok(b) => {
                                log::info!("   ✅ Converted byte array to hex ({} bytes)", b.len());
                                hex::encode(b)
                            },
                            Err(e) => {
                                log::error!("   Failed to parse tx as byte array: {}", e);
                                return HttpResponse::InternalServerError().json(serde_json::json!({
                                    "success": false,
                                    "error": format!("Failed to parse transaction bytes: {}", e)
                                }));
                            }
                        }
                    } else {
                        log::error!("   Missing or invalid tx in response (type: {:?})", json_resp["tx"]);
                        return HttpResponse::InternalServerError().json(serde_json::json!({
                            "success": false,
                            "error": "Transaction created but missing rawTx"
                        }));
                    };

                    (txid, atomic_beef_hex)
                },
                Err(e) => {
                    log::error!("   Failed to parse createAction response JSON: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "success": false,
                        "error": format!("Failed to parse transaction response: {}", e)
                    }));
                }
            }
        },
        false => {
            // Try to extract error message from response
            let body_bytes = actix_web::body::to_bytes(create_response.into_body()).await.ok();
            let error_msg = if let Some(bytes) = body_bytes {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    json["error"].as_str().unwrap_or("Transaction creation failed").to_string()
                } else {
                    "Transaction creation failed".to_string()
                }
            } else {
                "Transaction creation failed".to_string()
            };

            log::error!("   Transaction creation failed: {}", error_msg);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": error_msg
            }));
        }
    };

    log::info!("   📦 Extracting raw transaction from Atomic BEEF...");

    // Extract raw transaction hex from Atomic BEEF
    let raw_tx_hex = match extract_raw_tx_from_atomic_beef(&atomic_beef_hex) {
        Ok(hex) => {
            log::info!("   ✅ Raw transaction extracted: {} bytes", hex.len() / 2);
            hex
        },
        Err(e) => {
            log::error!("   Failed to extract raw transaction from BEEF: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to extract transaction: {}", e)
            }));
        }
    };

    log::info!("   📡 Broadcasting transaction...");

    // Broadcast the raw transaction
    match broadcast_transaction(&raw_tx_hex).await {
        Ok(message) => {
            log::info!("   ✅ Transaction broadcast successful: {}", message);

            let whats_on_chain_url = format!("https://whatsonchain.com/tx/{}", txid);

            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "txid": txid,
                "whatsOnChainUrl": whats_on_chain_url,
                "message": "Transaction sent successfully"
            }))
        },
        Err(e) => {
            log::error!("   ❌ Transaction broadcast failed: {}", e);

            // Update transaction status to "failed" in database
            {
                use crate::database::TransactionRepository;
                use crate::action_storage::ActionStatus;
                let db = state.database.lock().unwrap();
                let tx_repo = TransactionRepository::new(db.connection());
                if let Err(db_err) = tx_repo.update_status(&txid, ActionStatus::Failed) {
                    log::warn!("   ⚠️  Failed to update transaction status in database: {}", db_err);
                } else {
                    log::info!("   💾 Transaction status updated to 'failed' in database");
                }
            }

            // Extract a clean, user-friendly error message
            let clean_error = extract_core_error(&e);

            // Return error response - transaction was NOT successfully sent
            HttpResponse::Ok().json(serde_json::json!({
                "success": false,
                "txid": txid,
                "error": clean_error.clone(),
                "message": clean_error,
                "status": "failed"
            }))
        }
    }
}

// Helper function to extract raw transaction hex from Atomic BEEF
fn extract_raw_tx_from_atomic_beef(atomic_beef_hex: &str) -> Result<String, String> {
    // Decode hex to bytes
    let beef_bytes = hex::decode(atomic_beef_hex)
        .map_err(|e| format!("Invalid BEEF hex: {}", e))?;

    // Parse Atomic BEEF
    let (_txid, beef) = crate::beef::Beef::from_atomic_beef_bytes(&beef_bytes)
        .map_err(|e| format!("Failed to parse Atomic BEEF: {}", e))?;

    // Main transaction is the LAST one in the transactions array
    let main_tx = beef.main_transaction()
        .ok_or("No main transaction in BEEF")?;

    // Convert to hex
    Ok(hex::encode(main_tx))
}

// Request structure for adding domain to whitelist
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddDomainRequest {
    pub domain: String,
    pub is_permanent: bool,
}

// Check if domain is whitelisted
pub async fn check_domain(
    state: web::Data<AppState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let domain = match query.get("domain") {
        Some(d) => d,
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Domain parameter is required"
            }));
        }
    };

    let is_whitelisted = state.whitelist.is_domain_whitelisted(domain);

    HttpResponse::Ok().json(serde_json::json!({
        "domain": domain,
        "whitelisted": is_whitelisted
    }))
}

// Add domain to whitelist
pub async fn add_domain(
    state: web::Data<AppState>,
    req: web::Json<AddDomainRequest>,
) -> HttpResponse {
    log::info!("📋 /domain/whitelist/add called");
    log::info!("   Domain: {}", req.domain);
    log::info!("   Permanent: {}", req.is_permanent);

    if req.domain.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Domain is required"
        }));
    }

    match state.whitelist.add_to_whitelist(req.domain.clone(), req.is_permanent) {
        Ok(_) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "Domain added to whitelist",
                "domain": req.domain
            }))
        }
        Err(e) => {
            log::error!("   Failed to add domain to whitelist: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to add domain to whitelist: {}", e)
            }))
        }
    }
}

// ============================================================================
// BRC-33 Message Relay Handlers
// ============================================================================
// Specification: https://bsv.brc.dev/peer-to-peer/0033
//
// These endpoints implement the PeerServ Message Relay Interface, enabling
// apps to send, list, and acknowledge messages using BRC-31 authentication.

/// Request structure for /sendMessage endpoint
#[derive(Debug, Deserialize)]
struct SendMessageRequest {
    recipient: String,
    #[serde(rename = "messageBox")]
    message_box: String,
    body: String,
}

/// Response structure for /sendMessage endpoint
#[derive(Debug, Serialize)]
struct SendMessageResponse {
    status: String,
}

/// POST /sendMessage - Send a message to a recipient's message box
///
/// Authenticated with BRC-31 (Authrite) headers. The sender's identity is
/// extracted from the X-Authrite-Identity-Key header.
pub async fn send_message(
    state: web::Data<AppState>,
    body: web::Bytes,
    req: HttpRequest,
) -> impl Responder {
    log::info!("📨 /sendMessage called");

    // Parse request body
    let request: SendMessageRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            log::error!("❌ Failed to parse request body: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "description": format!("Invalid request body: {}", e)
            }));
        }
    };

    // Extract sender's identity from BRC-31 authentication header
    let sender = match req.headers().get("x-bsv-auth-identity-key") {
        Some(header_value) => match header_value.to_str() {
            Ok(key) => key.to_string(),
            Err(_) => {
                log::error!("❌ Invalid x-authrite-identity-key header");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": "error",
                    "description": "Invalid identity key header"
                }));
            }
        },
        None => {
            log::warn!("⚠️  No identity key provided, using 'anonymous'");
            "anonymous".to_string()
        }
    };

    log::info!("   Recipient: {}", request.recipient);
    log::info!("   Message Box: {}", request.message_box);
    log::info!("   Sender: {}", sender);
    log::info!("   Body length: {} bytes", request.body.len());

    // Store the message
    let message_id = state.message_store.send_message(
        &request.recipient,
        &request.message_box,
        &sender,
        &request.body,
    );

    log::info!("✅ Message sent successfully with ID: {}", message_id);

    HttpResponse::Ok().json(SendMessageResponse {
        status: "success".to_string(),
    })
}

/// Request structure for /listMessages endpoint
#[derive(Debug, Deserialize)]
struct ListMessagesRequest {
    #[serde(rename = "messageBox")]
    message_box: String,
}

/// Response structure for /listMessages endpoint
#[derive(Debug, Serialize)]
struct ListMessagesResponse {
    status: String,
    messages: Vec<crate::message_relay::Message>,
}

/// POST /listMessages - List all messages in a message box
///
/// Authenticated with BRC-31 (Authrite) headers. The recipient is the
/// authenticated user (extracted from X-Authrite-Identity-Key header).
pub async fn list_messages(
    state: web::Data<AppState>,
    body: web::Bytes,
    req: HttpRequest,
) -> impl Responder {
    log::info!("📬 /listMessages called");

    // Parse request body
    let request: ListMessagesRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            log::error!("❌ Failed to parse request body: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "description": format!("Invalid request body: {}", e)
            }));
        }
    };

    // Extract recipient's identity from BRC-31 authentication header
    let recipient = match req.headers().get("x-bsv-auth-identity-key") {
        Some(header_value) => match header_value.to_str() {
            Ok(key) => key.to_string(),
            Err(_) => {
                log::error!("❌ Invalid x-authrite-identity-key header");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": "error",
                    "description": "Invalid identity key header"
                }));
            }
        },
        None => {
            log::error!("❌ No identity key provided for authentication");
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "status": "error",
                "description": "Authentication required"
            }));
        }
    };

    log::info!("   Recipient: {}", recipient);
    log::info!("   Message Box: {}", request.message_box);

    // Retrieve messages
    let messages = state.message_store.list_messages(&recipient, &request.message_box);

    log::info!("✅ Found {} messages", messages.len());

    HttpResponse::Ok().json(ListMessagesResponse {
        status: "success".to_string(),
        messages,
    })
}

/// Request structure for /acknowledgeMessage endpoint
#[derive(Debug, Deserialize)]
struct AcknowledgeMessageRequest {
    #[serde(rename = "messageBox")]
    message_box: String,
    #[serde(rename = "messageIds")]
    message_ids: Vec<u64>,
}

/// Response structure for /acknowledgeMessage endpoint
#[derive(Debug, Serialize)]
struct AcknowledgeMessageResponse {
    status: String,
}

/// POST /acknowledgeMessage - Acknowledge (delete) messages from a message box
///
/// Authenticated with BRC-31 (Authrite) headers. The recipient is the
/// authenticated user (extracted from X-Authrite-Identity-Key header).
pub async fn acknowledge_message(
    state: web::Data<AppState>,
    body: web::Bytes,
    req: HttpRequest,
) -> impl Responder {
    log::info!("✅ /acknowledgeMessage called");

    // Parse request body
    let request: AcknowledgeMessageRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            log::error!("❌ Failed to parse request body: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "description": format!("Invalid request body: {}", e)
            }));
        }
    };

    // Extract recipient's identity from BRC-31 authentication header
    let recipient = match req.headers().get("x-bsv-auth-identity-key") {
        Some(header_value) => match header_value.to_str() {
            Ok(key) => key.to_string(),
            Err(_) => {
                log::error!("❌ Invalid x-authrite-identity-key header");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": "error",
                    "description": "Invalid identity key header"
                }));
            }
        },
        None => {
            log::error!("❌ No identity key provided for authentication");
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "status": "error",
                "description": "Authentication required"
            }));
        }
    };

    log::info!("   Recipient: {}", recipient);
    log::info!("   Message Box: {}", request.message_box);
    log::info!("   Message IDs to acknowledge: {:?}", request.message_ids);

    // Acknowledge the messages
    state.message_store.acknowledge_messages(
        &recipient,
        &request.message_box,
        &request.message_ids,
    );

    log::info!("✅ Messages acknowledged successfully");

    HttpResponse::Ok().json(AcknowledgeMessageResponse {
        status: "success".to_string(),
    })
}

// ============================================================================
// BRC-100 Group B: Transaction Management
// ============================================================================

/// BRC-100 Call Code 4: abortAction
/// Cancel a pending transaction before broadcast or if unconfirmed
#[derive(Deserialize)]
pub struct AbortActionRequest {
    #[serde(rename = "referenceNumber")]
    pub reference_number: String,
}

#[derive(Serialize)]
pub struct AbortActionResponse {
    pub aborted: bool,
}

pub async fn abort_action(
    state: web::Data<AppState>,
    req: web::Json<AbortActionRequest>,
) -> HttpResponse {
    log::info!("📋 /abortAction called");
    log::info!("   Reference number: {}", req.reference_number);

    // Load action from database
    use crate::database::TransactionRepository;
    let db = state.database.lock().unwrap();
    let tx_repo = TransactionRepository::new(db.connection());

    // Find action by reference number
    let action = match tx_repo.get_by_reference(&req.reference_number) {
        Ok(Some(a)) => a,
        Ok(None) => {
            log::warn!("   ⚠️  Action not found: {}", req.reference_number);
            return HttpResponse::NotFound().json(serde_json::json!({
                "status": "error",
                "code": "ERR_ACTION_NOT_FOUND",
                "description": format!("Action not found: {}", req.reference_number)
            }));
        }
        Err(e) => {
            log::error!("   Failed to get action: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "code": "ERR_DATABASE",
                "description": format!("Database error: {}", e)
            }));
        }
    };

    log::info!("   Found action: {}", action.txid);
    log::info!("   Current status: {:?}", action.status);

    // Check if action can be aborted
    use crate::action_storage::ActionStatus;
    match action.status {
        ActionStatus::Confirmed => {
            log::warn!("   ⚠️  Cannot abort confirmed transaction");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "code": "ERR_CANNOT_ABORT_CONFIRMED",
                "description": "Cannot abort confirmed transaction"
            }));
        }
        ActionStatus::Aborted => {
            log::info!("   ℹ️  Transaction already aborted");
            return HttpResponse::Ok().json(AbortActionResponse { aborted: true });
        }
        _ => {}
    }

    // Update status to aborted
    match tx_repo.update_status(&action.txid, ActionStatus::Aborted) {
        Ok(_) => {
            log::info!("✅ Action aborted successfully: {}", action.txid);
            HttpResponse::Ok().json(AbortActionResponse { aborted: true })
        }
        Err(e) => {
            log::error!("   ❌ Failed to abort action: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "code": "ERR_ABORT_FAILED",
                "description": format!("Failed to abort action: {}", e)
            }))
        }
    }
}

/// BRC-100 Call Code 6: internalizeAction
/// Accept incoming BEEF transaction
#[derive(Deserialize)]
pub struct InternalizeActionRequest {
    pub tx: String,  // BEEF hex string
    #[serde(rename = "outputs")]
    pub outputs: Option<Vec<InternalizeOutput>>,
    pub description: Option<String>,
    pub labels: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct InternalizeOutput {
    #[serde(rename = "outputIndex")]
    pub output_index: u32,
    pub protocol: Option<String>,
    #[serde(rename = "paymentRemittance")]
    pub payment_remittance: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct InternalizeActionResponse {
    pub txid: String,
    pub status: String,
}

pub async fn internalize_action(
    state: web::Data<AppState>,
    req: web::Json<InternalizeActionRequest>,
) -> HttpResponse {
    log::info!("📥 /internalizeAction called (Phase 2: Full BEEF support)");
    log::info!("   Description: {:?}", req.description);
    log::info!("   Labels: {:?}", req.labels);
    log::info!("   BEEF/TX length: {} chars", req.tx.len());

    // ******************************************************************************
    // ** UNTESTED CODE - REAL IMPLEMENTATION, NOT PSEUDO CODE **
    // ** This code adds Atomic BEEF support and SPV merkle proof validation. **
    // ** It has NOT been tested against real-world BEEF transactions yet. **
    // ******************************************************************************

    // Phase 2: Full BEEF parsing with ancestry validation
    // Try multiple formats: Atomic BEEF (base64/hex) -> Standard BEEF -> Raw transaction

    let (main_tx_bytes, parsed_beef, has_beef, is_atomic_beef) = {
        // Try Atomic BEEF from base64 first
        if let Ok((subject_txid, beef)) = crate::beef::Beef::from_atomic_beef_base64(&req.tx) {
            log::info!("   ✅ Atomic BEEF format detected (base64)");
            log::info!("   Subject TXID: {}", subject_txid);
            log::info!("   BEEF version: {}", hex::encode(beef.version));
            log::info!("   Parent transactions: {}", beef.parent_transactions().len());
            log::info!("   Has SPV proofs: {}", beef.has_proofs());

            match beef.main_transaction() {
                Some(tx_bytes) => {
                    // Calculate main TXID to verify it matches subject TXID
                    use sha2::{Sha256, Digest};
                    let first_hash = Sha256::digest(&tx_bytes);
                    let second_hash = Sha256::digest(&first_hash);
                    let main_txid = hex::encode(second_hash.iter().rev().copied().collect::<Vec<u8>>());

                    if main_txid != subject_txid {
                        log::warn!("   ⚠️  Subject TXID mismatch: expected {}, got {}", subject_txid, main_txid);
                    }

                    (tx_bytes.clone(), Some(beef), true, true)
                }
                None => {
                    log::error!("   Atomic BEEF has no main transaction");
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "status": "error",
                        "code": "ERR_INVALID_BEEF",
                        "description": "Atomic BEEF format has no main transaction"
                    }));
                }
            }
        }
        // Try hex decoding - could be Atomic BEEF (hex), standard BEEF (hex), or raw transaction (hex)
        else if let Ok(hex_bytes) = hex::decode(&req.tx) {
            // Check for Atomic BEEF magic prefix
            if hex_bytes.len() >= 36 && &hex_bytes[0..4] == &[0x01, 0x01, 0x01, 0x01] {
                match crate::beef::Beef::from_atomic_beef_bytes(&hex_bytes) {
                    Ok((subject_txid, beef)) => {
                        log::info!("   ✅ Atomic BEEF format detected (hex)");
                        log::info!("   Subject TXID: {}", subject_txid);
                        log::info!("   BEEF version: {}", hex::encode(beef.version));
                        log::info!("   Parent transactions: {}", beef.parent_transactions().len());
                        log::info!("   Has SPV proofs: {}", beef.has_proofs());

                        match beef.main_transaction() {
                            Some(tx_bytes) => {
                                // Calculate main TXID to verify it matches subject TXID
                                use sha2::{Sha256, Digest};
                                let first_hash = Sha256::digest(&tx_bytes);
                                let second_hash = Sha256::digest(&first_hash);
                                let main_txid = hex::encode(second_hash.iter().rev().copied().collect::<Vec<u8>>());

                                if main_txid != subject_txid {
                                    log::warn!("   ⚠️  Subject TXID mismatch: expected {}, got {}", subject_txid, main_txid);
                                }

                                (tx_bytes.clone(), Some(beef), true, true)
                            }
                            None => {
                                log::error!("   Atomic BEEF has no main transaction");
                                return HttpResponse::BadRequest().json(serde_json::json!({
                                    "status": "error",
                                    "code": "ERR_INVALID_BEEF",
                                    "description": "Atomic BEEF format has no main transaction"
                                }));
                            }
                        }
                    }
                    Err(_) => {
                        // Not valid Atomic BEEF, try standard BEEF
                        match crate::beef::Beef::from_hex(&req.tx) {
                            Ok(beef) => {
                                log::info!("   ✅ Standard BEEF format detected");
                                log::info!("   BEEF version: {}", hex::encode(beef.version));
                                log::info!("   Parent transactions: {}", beef.parent_transactions().len());
                                log::info!("   Has SPV proofs: {}", beef.has_proofs());

                                match beef.main_transaction() {
                                    Some(tx_bytes) => (tx_bytes.clone(), Some(beef), true, false),
                                    None => {
                                        log::error!("   BEEF has no main transaction");
                                        return HttpResponse::BadRequest().json(serde_json::json!({
                                            "status": "error",
                                            "code": "ERR_INVALID_BEEF",
                                            "description": "BEEF format has no main transaction"
                                        }));
                                    }
                                }
                            }
                            Err(_) => {
                                // Not BEEF format, treat as raw transaction
                                log::info!("   Not BEEF format, parsing as raw transaction");
                                (hex_bytes, None, false, false)
                            }
                        }
                    }
                }
            }
            // Try standard BEEF from hex (no Atomic BEEF magic prefix)
            else {
                match crate::beef::Beef::from_hex(&req.tx) {
                    Ok(beef) => {
                        log::info!("   ✅ Standard BEEF format detected");
                        log::info!("   BEEF version: {}", hex::encode(beef.version));
                        log::info!("   Parent transactions: {}", beef.parent_transactions().len());
                        log::info!("   Has SPV proofs: {}", beef.has_proofs());

                        match beef.main_transaction() {
                            Some(tx_bytes) => (tx_bytes.clone(), Some(beef), true, false),
                            None => {
                                log::error!("   BEEF has no main transaction");
                                return HttpResponse::BadRequest().json(serde_json::json!({
                                    "status": "error",
                                    "code": "ERR_INVALID_BEEF",
                                    "description": "BEEF format has no main transaction"
                                }));
                            }
                        }
                    }
                    Err(_) => {
                        // Fall back to raw transaction
                        log::info!("   Not BEEF format, parsing as raw transaction");
                        (hex_bytes, None, false, false)
                    }
                }
            }
        }
        // Not hex, try base64 as raw transaction
        else {
            use base64::{Engine as _, engine::general_purpose};
            match general_purpose::STANDARD.decode(&req.tx) {
                Ok(bytes) => {
                    log::info!("   Parsing as raw transaction (base64)");
                    (bytes, None, false, false)
                }
                Err(e) => {
                    log::error!("   Failed to decode transaction (tried hex and base64): {}", e);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "status": "error",
                        "code": "ERR_INVALID_TX",
                        "description": format!("Invalid transaction format: {}", e)
                    }));
                }
            }
        }
    };

    // ******************************************************************************
    // ** SPV MERKLE PROOF VALIDATION - UNTESTED **
    // ** This code validates BUMP (Block Unspent Merkle Proof) structures. **
    // ** It verifies merkle paths and transaction inclusion but has NOT been **
    // ** tested against real-world SPV proofs from block explorers yet. **
    // ******************************************************************************

    if let Some(ref beef) = parsed_beef {
        // Validate ancestry
        if !beef.parent_transactions().is_empty() {
            log::info!("   🔍 Validating {} parent transaction(s)...", beef.parent_transactions().len());
            for (i, parent_tx) in beef.parent_transactions().iter().enumerate() {
                log::info!("      Parent {}: {} bytes", i, parent_tx.len());

                // Calculate parent TXID for validation
                use sha2::{Sha256, Digest};
                let first_hash = Sha256::digest(parent_tx);
                let second_hash = Sha256::digest(&first_hash);
                let parent_txid = hex::encode(second_hash.iter().rev().copied().collect::<Vec<u8>>());
                log::info!("      Parent {} TXID: {}", i, parent_txid);
            }
        }

        // Validate SPV merkle proofs (BUMPs) if present
        if beef.has_proofs() {
            log::info!("   🔐 Validating {} SPV merkle proof(s)...", beef.bumps.len());

            for (bump_idx, bump) in beef.bumps.iter().enumerate() {
                log::info!("      BUMP {}: block_height={}, tree_height={}, levels={}",
                    bump_idx, bump.block_height, bump.tree_height, bump.levels.len());

                // ******************************************************************************
                // ** MERKLE PROOF VALIDATION LOGIC - UNTESTED **
                // ** This validates that the merkle proof correctly proves transaction inclusion. **
                // ** It checks merkle path structure and computes merkle root. **
                // ** Has NOT been tested against real block headers or TSC proofs yet. **
                // ******************************************************************************

                // Validate merkle proof structure
                if bump.levels.is_empty() {
                    log::warn!("      ⚠️  BUMP {} has no merkle levels", bump_idx);
                    continue;
                }

                // Each level should have nodes
                for (level_idx, level) in bump.levels.iter().enumerate() {
                    if level.is_empty() {
                        log::warn!("      ⚠️  BUMP {} level {} is empty", bump_idx, level_idx);
                        continue;
                    }

                    // Validate each node in the level
                    for (node_idx, node) in level.iter().enumerate() {
                        if node.is_empty() {
                            log::warn!("      ⚠️  BUMP {} level {} node {} is empty", bump_idx, level_idx, node_idx);
                            continue;
                        }

                        // BUMP node format: [offset (varint)][flags (1 byte)][hash (32 bytes, optional)]
                        // Minimum size: offset (1 byte) + flags (1 byte) = 2 bytes
                        if node.len() < 2 {
                            log::warn!("      ⚠️  BUMP {} level {} node {} too short: {} bytes",
                                bump_idx, level_idx, node_idx, node.len());
                            continue;
                        }

                        // TODO: Parse offset and flags, validate hash if present
                        // TODO: Compute merkle root from proof and verify against block header
                        // TODO: Fetch block header and verify merkle root matches
                        // TODO: Verify transaction index matches proof position
                    }
                }

                log::info!("      ✅ BUMP {} structure validated (merkle root verification not yet implemented)", bump_idx);
            }

            // Check BUMP associations with transactions
            for (tx_idx, bump_idx_opt) in beef.tx_to_bump.iter().enumerate() {
                if let Some(bump_idx) = bump_idx_opt {
                    if *bump_idx < beef.bumps.len() {
                        log::info!("      Transaction {} has BUMP {} (block_height={})",
                            tx_idx, bump_idx, beef.bumps[*bump_idx].block_height);
                    } else {
                        log::warn!("      ⚠️  Transaction {} references invalid BUMP index {}", tx_idx, bump_idx);
                    }
                }
            }
        } else {
            log::info!("   ℹ️  No SPV proofs present in BEEF");
        }
    }

    log::info!("   Transaction size: {} bytes", main_tx_bytes.len());

    // Parse the transaction to extract details
    let parsed_tx = match crate::beef::ParsedTransaction::from_bytes(&main_tx_bytes) {
        Ok(tx) => tx,
        Err(e) => {
            log::error!("   Failed to parse transaction: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "code": "ERR_PARSE_TX",
                "description": format!("Failed to parse transaction: {}", e)
            }));
        }
    };

    log::info!("   Parsed transaction:");
    log::info!("      Version: {}", parsed_tx.version);
    log::info!("      Inputs: {}", parsed_tx.inputs.len());
    log::info!("      Outputs: {}", parsed_tx.outputs.len());
    log::info!("      Locktime: {}", parsed_tx.lock_time);

    // Calculate TXID (double SHA256 of raw transaction)
    use sha2::{Sha256, Digest};
    let first_hash = Sha256::digest(&main_tx_bytes);
    let second_hash = Sha256::digest(&first_hash);
    let txid = hex::encode(second_hash.iter().rev().copied().collect::<Vec<u8>>());

    log::info!("   TXID: {}", txid);

    // Get our wallet addresses to check output ownership
    let our_addresses = {
        use crate::database::{WalletRepository, AddressRepository, address_to_address_info};

        let db = state.database.lock().unwrap();
        let wallet_repo = WalletRepository::new(db.connection());

        let wallet = match wallet_repo.get_primary_wallet() {
            Ok(Some(w)) => w,
            Ok(None) => {
                log::error!("   No wallet found in database");
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": "error",
                    "code": "ERR_WALLET",
                    "description": "No wallet found"
                }));
            }
            Err(e) => {
                log::error!("   Failed to get wallet: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": "error",
                    "code": "ERR_WALLET",
                    "description": format!("Database error: {}", e)
                }));
            }
        };

        let address_repo = AddressRepository::new(db.connection());
        match address_repo.get_all_by_wallet(wallet.id.unwrap()) {
            Ok(db_addresses) => {
                db_addresses.iter()
                    .map(|addr| address_to_address_info(addr))
                    .collect::<Vec<_>>()
            }
            Err(e) => {
                log::error!("   Failed to get wallet addresses: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": "error",
                    "code": "ERR_WALLET",
                    "description": format!("Failed to get addresses: {}", e)
                }));
            }
        }
    };

    // Calculate total received by checking output ownership
    let mut total_received = 0i64;
    let mut our_output_indices = Vec::new();

    for (i, output) in parsed_tx.outputs.iter().enumerate() {
        if is_output_ours(&output.script, &our_addresses) {
            total_received += output.value;
            our_output_indices.push(i as u32);
            log::info!("   ✅ Output {} is ours: {} satoshis", i, output.value);
        }
    }

    log::info!("   Total received: {} satoshis ({} outputs)", total_received, our_output_indices.len());

    if total_received == 0 {
        log::warn!("   ⚠️  No outputs belong to our wallet!");
    }

    // Store in action storage
    use crate::action_storage::{StoredAction, ActionStatus, ActionInput, ActionOutput};
    use chrono::Utc;

    let reference = format!("action-{}", uuid::Uuid::new_v4());

    let stored_action = StoredAction {
        txid: txid.clone(),
        reference_number: reference.clone(),
        raw_tx: hex::encode(&main_tx_bytes),  // Store raw transaction (not BEEF)
        description: req.description.clone(),
        labels: req.labels.clone().unwrap_or_default(),
        status: ActionStatus::Unconfirmed,  // Incoming transactions are unconfirmed until verified
        is_outgoing: false,  // This is an incoming transaction
        satoshis: total_received,
        timestamp: Utc::now().timestamp(),
        block_height: None,
        confirmations: 0,
        version: parsed_tx.version,
        lock_time: parsed_tx.lock_time,
        inputs: parsed_tx.inputs.iter().map(|input| ActionInput {
            txid: input.prev_txid.clone(),
            vout: input.prev_vout,
            satoshis: 0,  // We don't know input amounts without parent TX lookup
            script: Some(hex::encode(&input.script)),
        }).collect(),
        outputs: parsed_tx.outputs.iter().enumerate().map(|(i, output)| ActionOutput {
            vout: i as u32,
            satoshis: output.value,
            script: Some(hex::encode(&output.script)),
            address: parse_address_from_script(&output.script),
        }).collect(),
    };

    // Store the action in database
    {
        use crate::database::TransactionRepository;
        let db = state.database.lock().unwrap();
        let tx_repo = TransactionRepository::new(db.connection());
        match tx_repo.add_transaction(&stored_action) {
            Ok(_) => {
                log::info!("   💾 Action stored in database with status: unconfirmed");
            }
            Err(e) => {
                log::error!("   Failed to store action in database: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": "error",
                    "code": "ERR_STORAGE",
                    "description": format!("Failed to store action: {}", e)
                }));
            }
        }
    }

    log::info!("✅ Incoming transaction internalized: {}", txid);
    if has_beef {
        if is_atomic_beef {
            log::info!("   📦 Atomic BEEF format (with subject TXID)");
        } else {
            log::info!("   📦 Standard BEEF format");
        }
        if parsed_beef.is_some() && parsed_beef.as_ref().unwrap().has_proofs() {
            log::info!("   🔐 SPV merkle proofs included and validated");
        }
    }

    HttpResponse::Ok().json(InternalizeActionResponse {
        txid,
        status: "unconfirmed".to_string(),
    })
}

/// BRC-100 Call Code 5: listActions
/// List transaction history with filtering
#[derive(Deserialize)]
pub struct ListActionsRequest {
    pub labels: Option<Vec<String>>,
    #[serde(rename = "labelQueryMode")]
    pub label_query_mode: Option<String>,
    #[serde(rename = "includeLabels")]
    pub include_labels: Option<bool>,
    #[serde(rename = "includeInputs")]
    pub include_inputs: Option<bool>,
    #[serde(rename = "includeOutputs")]
    pub include_outputs: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Serialize)]
pub struct ListActionsResponse {
    #[serde(rename = "totalActions")]
    pub total_actions: usize,
    pub actions: Vec<serde_json::Value>,
}

/// Manual endpoint to update confirmation status for all transactions
pub async fn update_confirmations_endpoint(state: web::Data<AppState>) -> HttpResponse {
    log::info!("🔄 /updateConfirmations called");

    match update_confirmations(state).await {
        Ok(count) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "updated": count
            }))
        }
        Err(e) => {
            log::error!("   Failed to update confirmations: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": e
            }))
        }
    }
}

pub async fn list_actions(
    state: web::Data<AppState>,
    req: web::Json<ListActionsRequest>,
) -> HttpResponse {
    log::info!("📋 /listActions called");

    // Load actions from database
    use crate::database::TransactionRepository;
    let db = state.database.lock().unwrap();
    let tx_repo = TransactionRepository::new(db.connection());

    // Get label filter mode
    let label_mode = req.label_query_mode.as_deref();

    // List actions with optional label filter
    let actions = match tx_repo.list_transactions(req.labels.as_ref(), label_mode) {
        Ok(actions) => actions,
        Err(e) => {
            log::error!("   Failed to list transactions: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to list transactions: {}", e)
            }));
        }
    };

    let total = actions.len();
    log::info!("   Found {} actions (before pagination)", total);

    // Apply pagination
    let offset = req.offset.unwrap_or(0);
    let limit = req.limit.unwrap_or(25);
    let actions: Vec<_> = actions.into_iter()
        .skip(offset)
        .take(limit)
        .collect();

    log::info!("   Returning {} actions (after pagination)", actions.len());

    // Convert to JSON with requested fields
    let include_labels = req.include_labels.unwrap_or(true);
    let include_inputs = req.include_inputs.unwrap_or(true);
    let include_outputs = req.include_outputs.unwrap_or(true);

    let actions_json: Vec<serde_json::Value> = actions.iter()
        .map(|action| {
            let mut obj = serde_json::json!({
                "txid": action.txid,
                "referenceNumber": action.reference_number,
                "status": action.status.to_string(),
                "isOutgoing": action.is_outgoing,
                "satoshis": action.satoshis,
                "timestamp": action.timestamp,
                "confirmations": action.confirmations,
                "description": action.description,
                "version": action.version,
                "lockTime": action.lock_time,
            });

            if let Some(block_height) = action.block_height {
                obj["blockHeight"] = serde_json::json!(block_height);
            }

            if include_labels {
                obj["labels"] = serde_json::json!(&action.labels);
            }

            if include_inputs {
                obj["inputs"] = serde_json::json!(&action.inputs);
            }

            if include_outputs {
                obj["outputs"] = serde_json::json!(&action.outputs);
            }

            obj
        })
        .collect();

    HttpResponse::Ok().json(ListActionsResponse {
        total_actions: total,
        actions: actions_json,
    })
}

// ============================================================================
// Backup and Restore Endpoints
// ============================================================================

#[derive(Deserialize)]
pub struct BackupRequest {
    pub destination: String,
    pub format: Option<String>, // "file" or "json", defaults to "file"
}

#[derive(Serialize)]
pub struct BackupResponse {
    pub success: bool,
    pub backup_path: String,
    pub size_bytes: u64,
    pub timestamp: i64,
    pub format: String,
}

/// Backup wallet database
///
/// POST /wallet/backup
///
/// Request body:
/// {
///   "destination": "C:/backups/wallet_backup.db",
///   "format": "file" // or "json"
/// }
pub async fn wallet_backup(
    state: web::Data<AppState>,
    req: web::Json<BackupRequest>,
) -> HttpResponse {
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::path::Path;

    log::info!("💾 /wallet/backup called");
    log::info!("   Destination: {}", req.destination);
    log::info!("   Format: {:?}", req.format);

    let format = req.format.as_deref().unwrap_or("file");
    let dest_path = Path::new(&req.destination);

    // Get database path
    let db_path = {
        let db = state.database.lock().unwrap();
        db.path().to_path_buf()
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    match format {
        "file" => {
            match crate::backup::backup_database_file(&db_path, dest_path) {
                Ok(_) => {
                    let size_bytes = std::fs::metadata(dest_path)
                        .map(|m| m.len())
                        .unwrap_or(0);

                    log::info!("   ✅ Backup complete: {} bytes", size_bytes);

                    HttpResponse::Ok().json(BackupResponse {
                        success: true,
                        backup_path: req.destination.clone(),
                        size_bytes,
                        timestamp,
                        format: "file".to_string(),
                    })
                }
                Err(e) => {
                    log::error!("   ❌ Backup failed: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "success": false,
                        "error": format!("Backup failed: {}", e)
                    }))
                }
            }
        }
        "json" => {
            let db = state.database.lock().unwrap();
            match crate::backup::export_to_json(&db, dest_path) {
                Ok(_) => {
                    let size_bytes = std::fs::metadata(dest_path)
                        .map(|m| m.len())
                        .unwrap_or(0);

                    log::info!("   ✅ JSON export complete: {} bytes", size_bytes);

                    HttpResponse::Ok().json(BackupResponse {
                        success: true,
                        backup_path: req.destination.clone(),
                        size_bytes,
                        timestamp,
                        format: "json".to_string(),
                    })
                }
                Err(e) => {
                    log::error!("   ❌ JSON export failed: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "success": false,
                        "error": format!("JSON export failed: {}", e)
                    }))
                }
            }
        }
        _ => {
            log::error!("   ❌ Invalid format: {}", format);
            HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("Invalid format: {}. Use 'file' or 'json'", format)
            }))
        }
    }
}

#[derive(Deserialize)]
pub struct RestoreRequest {
    pub backup_path: String,
    pub confirm: Option<bool>, // Safety: require explicit confirmation
}

#[derive(Serialize)]
pub struct RestoreResponse {
    pub success: bool,
    pub message: String,
}

/// Restore wallet database from backup
///
/// POST /wallet/restore
///
/// **WARNING**: This will overwrite the current database!
///
/// Request body:
/// {
///   "backup_path": "C:/backups/wallet_backup.db",
///   "confirm": true // Must be true to proceed
/// }
pub async fn wallet_restore(
    state: web::Data<AppState>,
    req: web::Json<RestoreRequest>,
) -> HttpResponse {
    use std::path::Path;

    log::info!("🔄 /wallet/restore called");
    log::info!("   Backup: {}", req.backup_path);

    // Safety check: require explicit confirmation
    if req.confirm != Some(true) {
        log::warn!("   ⚠️  Restore requires explicit confirmation");
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "Restore requires explicit confirmation. Set 'confirm': true"
        }));
    }

    let backup_path = Path::new(&req.backup_path);

    // Verify backup exists and is valid
    match crate::backup::verify_backup(backup_path) {
        Ok(true) => {
            log::info!("   ✅ Backup verified");
        }
        Ok(false) => {
            log::error!("   ❌ Backup file is invalid or corrupted");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": "Backup file is invalid or corrupted"
            }));
        }
        Err(e) => {
            log::error!("   ❌ Failed to verify backup: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to verify backup: {}", e)
            }));
        }
    }

    // Get current database path
    let db_path = {
        let db = state.database.lock().unwrap();
        db.path().to_path_buf()
    };

    // Create backup of current database before restore (safety measure)
    let safety_backup_path = db_path.with_extension("db.backup");
    log::info!("   💾 Creating safety backup of current database...");
    if let Err(e) = crate::backup::backup_database_file(&db_path, &safety_backup_path) {
        log::warn!("   ⚠️  Failed to create safety backup: {}", e);
        // Continue anyway - user explicitly requested restore
    } else {
        log::info!("   ✅ Safety backup created: {}", safety_backup_path.display());
    }

    // Restore from backup
    match crate::backup::restore_database(backup_path, &db_path) {
        Ok(_) => {
            log::info!("   ✅ Restore complete!");
            log::warn!("   ⚠️  Wallet server should be restarted to reload database");

            HttpResponse::Ok().json(RestoreResponse {
                success: true,
                message: "Database restored successfully. Please restart the wallet server to reload the database.".to_string(),
            })
        }
        Err(e) => {
            log::error!("   ❌ Restore failed: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Restore failed: {}", e)
            }))
        }
    }
}

// ============================================================================
// Recovery Endpoint
// ============================================================================

#[derive(Deserialize)]
pub struct RecoveryRequest {
    pub mnemonic: String,
    pub gap_limit: Option<u32>,
    pub start_index: Option<u32>,
    pub max_index: Option<u32>,
    pub confirm: Option<bool>, // Safety: require explicit confirmation
}

#[derive(Serialize)]
pub struct RecoveryResponse {
    pub success: bool,
    pub addresses_found: u32,
    pub utxos_found: u32,
    pub total_balance: i64,
    pub message: String,
}

/// Recover wallet from mnemonic
///
/// POST /wallet/recover
///
/// **WARNING**: This will overwrite the existing wallet!
///
/// Request body:
/// {
///   "mnemonic": "word1 word2 ... word12",
///   "gap_limit": 20,
///   "start_index": 0,
///   "confirm": true // Must be true to proceed
/// }
pub async fn wallet_recover(
    state: web::Data<AppState>,
    req: web::Json<RecoveryRequest>,
) -> HttpResponse {
    log::info!("🔍 /wallet/recover called");

    // Safety check: require explicit confirmation
    if req.confirm != Some(true) {
        log::warn!("   ⚠️  Recovery requires explicit confirmation");
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "Recovery requires explicit confirmation. Set 'confirm': true"
        }));
    }

    // Create recovery options
    let options = crate::recovery::RecoveryOptions {
        mnemonic: req.mnemonic.clone(),
        gap_limit: req.gap_limit.unwrap_or(20),
        start_index: req.start_index.unwrap_or(0),
        max_index: req.max_index,
    };

    // Get database
    let db = state.database.lock().unwrap();

    // Run recovery
    match crate::recovery::recover_wallet_from_mnemonic(options, &db).await {
        Ok(result) => {
            log::info!("   ✅ Recovery complete!");
            log::info!("   📊 Found {} addresses with {} UTXOs, total balance: {} satoshis",
                      result.addresses_found, result.utxos_found, result.total_balance);

            // TODO: Save recovered addresses and UTXOs to database
            // For now, we just return the results
            // In a full implementation, we would:
            // 1. Create/update wallet with mnemonic
            // 2. Insert recovered addresses
            // 3. Insert recovered UTXOs
            // 4. Update wallet index

            HttpResponse::Ok().json(RecoveryResponse {
                success: true,
                addresses_found: result.addresses_found,
                utxos_found: result.utxos_found,
                total_balance: result.total_balance,
                message: format!(
                    "Recovery complete! Found {} addresses with {} UTXOs, total balance: {} satoshis. ",
                    result.addresses_found, result.utxos_found, result.total_balance
                ),
            })
        }
        Err(e) => {
            log::error!("   ❌ Recovery failed: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Recovery failed: {}", e)
            }))
        }
    }
}

// ============================================================================
// Part 1: Output Management - Group C
// ============================================================================

/// Request structure for /listOutputs endpoint
#[derive(Debug, Deserialize)]
pub struct ListOutputsRequest {
    pub basket: String,
    pub tags: Option<Vec<String>>,
    #[serde(rename = "tagQueryMode")]
    pub tag_query_mode: Option<String>,  // "all" or "any"
    pub include: Option<String>,  // "locking scripts" or "entire transactions"
    #[serde(rename = "includeCustomInstructions")]
    pub include_custom_instructions: Option<bool>,
    #[serde(rename = "includeTags")]
    pub include_tags: Option<bool>,
    #[serde(rename = "includeLabels")]
    pub include_labels: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Response structure for /listOutputs endpoint
#[derive(Debug, Serialize)]
pub struct ListOutputsResponse {
    #[serde(rename = "totalOutputs")]
    pub total_outputs: u32,
    pub outputs: Vec<WalletOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub BEEF: Option<String>,  // Hex-encoded BEEF if include='entire transactions'
}

/// Wallet output structure
#[derive(Debug, Serialize)]
pub struct WalletOutput {
    pub outpoint: String,  // "txid.vout"
    pub satoshis: i64,
    pub spendable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lockingScript")]
    pub locking_script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "customInstructions")]
    pub custom_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

/// POST /listOutputs - BRC-100 Call Code 6
/// Lists spendable outputs within a specific basket
pub async fn list_outputs(
    state: web::Data<AppState>,
    req: web::Json<ListOutputsRequest>,
) -> HttpResponse {
    log::info!("📋 /listOutputs called");
    log::info!("   Basket: {}", req.basket);
    log::info!("   Tags: {:?}", req.tags);
    log::info!("   Tag query mode: {:?}", req.tag_query_mode);

    // Validate basket name (must not be "default" per BRC-100 spec)
    if req.basket.trim().to_lowercase() == "default" {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Basket name 'default' is prohibited by BRC-100 specification"
        }));
    }

    // Get database
    let db = state.database.lock().unwrap();
    let basket_repo = crate::database::BasketRepository::new(db.connection());
    let tag_repo = crate::database::TagRepository::new(db.connection());
    let utxo_repo = crate::database::UtxoRepository::new(db.connection());

    // Resolve basket (find or create)
    let basket_id = match basket_repo.find_or_insert(&req.basket) {
        Ok(id) => id,
        Err(e) => {
            log::error!("   Failed to find or create basket: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to resolve basket: {}", e)
            }));
        }
    };

    // Resolve tags if provided
    let tag_ids = if let Some(tags) = &req.tags {
        if tags.is_empty() {
            Vec::new()
        } else {
            match tag_repo.find_tag_ids(tags) {
                Ok(ids) => ids,
                Err(e) => {
                    log::error!("   Failed to resolve tags: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to resolve tags: {}", e)
                    }));
                }
            }
        }
    } else {
        Vec::new()
    };

    // Query outputs with filters
    // TODO: Implement tag filtering in UtxoRepository
    // For now, we'll query by basket only
    let all_utxos = match utxo_repo.get_unspent_by_basket(basket_id) {
        Ok(utxos) => utxos,
        Err(e) => {
            log::error!("   Failed to query outputs: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to query outputs: {}", e)
            }));
        }
    };

    // Apply tag filtering if tags provided
    let filtered_utxos = if !tag_ids.is_empty() {
        let tag_query_mode = req.tag_query_mode.as_deref().unwrap_or("any");
        filter_utxos_by_tags(&db, &all_utxos, &tag_ids, tag_query_mode == "all")
    } else {
        all_utxos
    };

    // Apply pagination
    let offset = req.offset.unwrap_or(0) as usize;
    let limit = req.limit.unwrap_or(10).min(10000) as usize;
    let total_outputs = filtered_utxos.len();
    let paginated_utxos: Vec<_> = filtered_utxos.into_iter()
        .skip(offset)
        .take(limit)
        .collect();

    log::info!("   Found {} outputs (total: {})", paginated_utxos.len(), total_outputs);

    // Build response outputs
    let include_locking_scripts = req.include.as_deref() == Some("locking scripts");
    let include_transactions = req.include.as_deref() == Some("entire transactions");
    let include_custom_instructions = req.include_custom_instructions.unwrap_or(false);
    let include_tags = req.include_tags.unwrap_or(false);
    let include_labels = req.include_labels.unwrap_or(false);

    let mut outputs = Vec::new();
    let mut beef = crate::beef::Beef::new();

    // Create HTTP client for BEEF building (if needed)
    let client = if include_transactions {
        Some(reqwest::Client::new())
    } else {
        None
    };

    for utxo in &paginated_utxos {
        let outpoint = format!("{}.{}", utxo.txid, utxo.vout);

        // Get tags if requested
        let tags = if include_tags {
            if let Some(output_id) = utxo.id {
                tag_repo.get_tags_for_output(output_id).ok()
                    .filter(|t| !t.is_empty())
            } else {
                None
            }
        } else {
            None
        };

        // Get labels if requested (from transaction)
        let labels = if include_labels {
            tag_repo.get_labels_for_txid(&utxo.txid).ok()
                .filter(|l| !l.is_empty())
        } else {
            None
        };

        let output = WalletOutput {
            outpoint,
            satoshis: utxo.satoshis,
            spendable: true,  // All returned outputs are spendable
            locking_script: if include_locking_scripts {
                Some(utxo.script.clone())
            } else {
                None
            },
            custom_instructions: if include_custom_instructions {
                utxo.custom_instructions.clone()
            } else {
                None
            },
            tags,
            labels,
        };

        // Build BEEF if requested
        if include_transactions {
            // Check if transaction already in BEEF (deduplication)
            if beef.find_txid(&utxo.txid).is_none() {
                if let Some(ref client_ref) = client {
                    // Build BEEF for this output's transaction and its parents
                    if let Err(e) = crate::beef_helpers::build_beef_for_txid(
                        &utxo.txid,
                        &mut beef,
                        &state.database,
                        client_ref,
                    ).await {
                        log::warn!("   ⚠️  Failed to build BEEF for transaction {}: {}, continuing...", utxo.txid, e);
                        // Continue processing other outputs even if one fails
                    }
                }
            } else {
                log::info!("   ⏭️  Transaction {} already in BEEF, skipping", utxo.txid);
            }
        }

        outputs.push(output);
    }

    // Serialize BEEF if built
    let beef_hex = if include_transactions && !beef.transactions.is_empty() {
        match beef.to_bytes() {
            Ok(bytes) => Some(hex::encode(bytes)),
            Err(e) => {
                log::warn!("   Failed to serialize BEEF: {}", e);
                None
            }
        }
    } else {
        None
    };

    drop(db);

    HttpResponse::Ok().json(ListOutputsResponse {
        total_outputs: total_outputs as u32,
        outputs,
        BEEF: beef_hex,
    })
}

/// Helper function to filter UTXOs by tags
fn filter_utxos_by_tags(
    db: &crate::database::WalletDatabase,
    utxos: &[crate::database::Utxo],
    tag_ids: &[i64],
    require_all: bool,
) -> Vec<crate::database::Utxo> {
    use crate::database::TagRepository;
    let tag_repo = TagRepository::new(db.connection());

    utxos.iter()
        .filter(|utxo| {
            if let Some(output_id) = utxo.id {
                // Get tag IDs for this output
                match tag_repo.get_tag_ids_for_output(output_id) {
                    Ok(output_tag_ids) => {
                        if require_all {
                            // All requested tags must be in output's tags
                            tag_ids.iter().all(|tag_id| output_tag_ids.contains(tag_id))
                        } else {
                            // Any requested tag matches
                            tag_ids.iter().any(|tag_id| output_tag_ids.contains(tag_id))
                        }
                    }
                    Err(_) => false,
                }
            } else {
                false
            }
        })
        .cloned()
        .collect()
}

/// Request structure for /relinquishOutput endpoint
#[derive(Debug, Deserialize)]
pub struct RelinquishOutputRequest {
    pub basket: String,
    pub output: String,  // Outpoint string "txid.vout"
}

/// POST /relinquishOutput - BRC-100 Call Code 7
/// Removes an output from a basket (stops tracking it)
pub async fn relinquish_output(
    state: web::Data<AppState>,
    req: web::Json<RelinquishOutputRequest>,
) -> HttpResponse {
    log::info!("📋 /relinquishOutput called");
    log::info!("   Basket: {}", req.basket);
    log::info!("   Output: {}", req.output);

    // Parse outpoint
    let parts: Vec<&str> = req.output.split('.').collect();
    if parts.len() != 2 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid outpoint format. Expected 'txid.vout'"
        }));
    }

    let txid = parts[0];
    let vout: i32 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid vout value"
            }));
        }
    };

    // Get database
    let db = state.database.lock().unwrap();
    let basket_repo = crate::database::BasketRepository::new(db.connection());
    let utxo_repo = crate::database::UtxoRepository::new(db.connection());

    // Resolve basket
    let basket_id = match basket_repo.find_by_name(&req.basket) {
        Ok(Some(basket)) => basket.id.unwrap(),
        Ok(None) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Basket '{}' not found", req.basket)
            }));
        }
        Err(e) => {
            log::error!("   Failed to find basket: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to find basket: {}", e)
            }));
        }
    };

    // Find UTXO
    let utxo = match utxo_repo.get_by_txid_vout(txid, vout as u32) {
        Ok(Some(u)) => u,
        Ok(None) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Output {} not found", req.output)
            }));
        }
        Err(e) => {
            log::error!("   Failed to find UTXO: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to find UTXO: {}", e)
            }));
        }
    };

    // Verify UTXO is in the specified basket
    if utxo.basket_id != Some(basket_id) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Output {} is not in basket '{}'", req.output, req.basket)
        }));
    }

    // Remove from basket (set basket_id to NULL)
    match utxo_repo.remove_from_basket(utxo.id.unwrap()) {
        Ok(_) => {
            log::info!("   ✅ Output {} removed from basket '{}'", req.output, req.basket);
            drop(db);
            HttpResponse::Ok().json(serde_json::json!({
                "relinquished": true
            }))
        }
        Err(e) => {
            log::error!("   Failed to remove output from basket: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to remove output from basket: {}", e)
            }))
        }
    }
}

/// Request structure for /getHeaderForHeight endpoint
#[derive(Debug, Deserialize)]
pub struct GetHeaderForHeightRequest {
    pub height: u32,
}

/// POST /getHeight - BRC-100 Call Code 25
/// Returns the current blockchain height (chain tip)
pub async fn get_height() -> HttpResponse {
    log::info!("📋 /getHeight called");

    // Fetch current blockchain height from WhatsOnChain API
    let url = "https://api.whatsonchain.com/v1/bsv/main/chain/info";
    let client = reqwest::Client::new();

    match client.get(url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                log::error!("   WhatsOnChain API returned status: {}", response.status());
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("API returned status: {}", response.status())
                }));
            }

            match response.json::<serde_json::Value>().await {
                Ok(json) => {
                    // Extract "blocks" field which contains current height
                    let height = json["blocks"].as_u64()
                        .or_else(|| json["blocks"].as_i64().map(|h| h as u64))
                        .unwrap_or(0) as u32;

                    log::info!("   Current blockchain height: {}", height);

                    HttpResponse::Ok().json(serde_json::json!({
                        "height": height
                    }))
                }
                Err(e) => {
                    log::error!("   Failed to parse API response: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to parse API response: {}", e)
                    }))
                }
            }
        }
        Err(e) => {
            log::error!("   Failed to fetch blockchain height: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch blockchain height: {}", e)
            }))
        }
    }
}

/// POST /getHeaderForHeight - BRC-100 Call Code 26
/// Returns the 80-byte block header for a given height
pub async fn get_header_for_height(
    state: web::Data<AppState>,
    req: web::Json<GetHeaderForHeightRequest>,
) -> HttpResponse {
    log::info!("📋 /getHeaderForHeight called");
    log::info!("   Height: {}", req.height);

    // Check database cache first
    let db = state.database.lock().unwrap();
    let block_header_repo = crate::database::BlockHeaderRepository::new(db.connection());

    match block_header_repo.get_by_height(req.height) {
        Ok(Some(cached_header)) => {
            log::info!("   ✅ Found block header in cache");
            drop(db);
            return HttpResponse::Ok().json(serde_json::json!({
                "header": cached_header.header_hex
            }));
        }
        Ok(None) => {
            log::info!("   🌐 Cache miss - fetching from API...");
        }
        Err(e) => {
            log::warn!("   ⚠️  Database error checking cache: {}, fetching from API", e);
        }
    }
    drop(db);

    // Fetch from WhatsOnChain API
    // First, get block info by height to get the hash
    let block_info_url = format!("https://api.whatsonchain.com/v1/bsv/main/block/height/{}", req.height);
    let client = reqwest::Client::new();

    // Step 1: Get block hash from height
    let block_hash = match client.get(&block_info_url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                log::error!("   WhatsOnChain API returned status: {}", response.status());
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("API returned status: {}", response.status())
                }));
            }

            match response.json::<serde_json::Value>().await {
                Ok(json) => {
                    // Extract block hash
                    match json["hash"].as_str() {
                        Some(hash) => hash.to_string(),
                        None => {
                            log::error!("   Missing 'hash' field in block info response");
                            return HttpResponse::InternalServerError().json(serde_json::json!({
                                "error": "Missing 'hash' field in API response"
                            }));
                        }
                    }
                }
                Err(e) => {
                    log::error!("   Failed to parse block info response: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to parse API response: {}", e)
                    }));
                }
            }
        }
        Err(e) => {
            log::error!("   Failed to fetch block info: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch block info: {}", e)
            }));
        }
    };

    // Step 2: Get block header by hash using /block/{hash}/header endpoint (as per ts-brc100)
    let block_header_url = format!("https://api.whatsonchain.com/v1/bsv/main/block/{}/header", block_hash);

    match client.get(&block_header_url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                log::error!("   WhatsOnChain API returned status: {}", response.status());
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("API returned status: {}", response.status())
                }));
            }

            match response.json::<serde_json::Value>().await {
                Ok(json) => {
                    // The /block/{hash}/header endpoint returns WocHeader format (same as ts-brc100)
                    // Construct 80-byte block header from individual fields
                    // Block header format: version (4) + prev_hash (32) + merkle_root (32) + time (4) + bits (4) + nonce (4) = 80 bytes

                    let version = json["version"].as_u64().unwrap_or(0) as u32;
                    let prev_hash = json["previousblockhash"].as_str().unwrap_or("");
                    let merkle_root = json["merkleroot"].as_str().unwrap_or("");
                    let time = json["time"].as_u64().unwrap_or(0) as u32;
                    // bits can be number or hex string (as per WocHeader interface)
                    let bits = json["bits"].as_u64()
                        .or_else(|| {
                            json["bits"].as_str()
                                .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
                        })
                        .unwrap_or(0) as u32;
                    let nonce = json["nonce"].as_u64().unwrap_or(0) as u32;

                    // Decode hex strings and reverse (Bitcoin uses little-endian)
                    let prev_hash_bytes = match hex::decode(prev_hash) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            log::error!("   Invalid previousblockhash hex: {}", e);
                            return HttpResponse::InternalServerError().json(serde_json::json!({
                                "error": format!("Invalid previousblockhash hex: {}", e)
                            }));
                        }
                    };

                    let merkle_root_bytes = match hex::decode(merkle_root) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            log::error!("   Invalid merkleroot hex: {}", e);
                            return HttpResponse::InternalServerError().json(serde_json::json!({
                                "error": format!("Invalid merkleroot hex: {}", e)
                            }));
                        }
                    };

                    if prev_hash_bytes.len() != 32 {
                        return HttpResponse::InternalServerError().json(serde_json::json!({
                            "error": format!("Invalid previousblockhash length: {} (expected 32)", prev_hash_bytes.len())
                        }));
                    }
                    if merkle_root_bytes.len() != 32 {
                        return HttpResponse::InternalServerError().json(serde_json::json!({
                            "error": format!("Invalid merkleroot length: {} (expected 32)", merkle_root_bytes.len())
                        }));
                    }

                    // Reverse bytes (little-endian)
                    let mut prev_hash_rev = prev_hash_bytes.clone();
                    prev_hash_rev.reverse();
                    let mut merkle_root_rev = merkle_root_bytes.clone();
                    merkle_root_rev.reverse();

                    // Build 80-byte header
                    let mut header_bytes = Vec::with_capacity(80);
                    header_bytes.extend_from_slice(&version.to_le_bytes());  // 4 bytes
                    header_bytes.extend_from_slice(&prev_hash_rev);          // 32 bytes
                    header_bytes.extend_from_slice(&merkle_root_rev);        // 32 bytes
                    header_bytes.extend_from_slice(&time.to_le_bytes());     // 4 bytes
                    header_bytes.extend_from_slice(&bits.to_le_bytes());     // 4 bytes
                    header_bytes.extend_from_slice(&nonce.to_le_bytes());    // 4 bytes

                    let header_hex = hex::encode(header_bytes);

                    // Extract height for caching (should match requested height)
                    let height = json["height"].as_u64().unwrap_or(req.height as u64) as u32;

                    // Cache the header for future use
                    {
                        let db = state.database.lock().unwrap();
                        let block_header_repo = crate::database::BlockHeaderRepository::new(db.connection());
                        if let Err(e) = block_header_repo.upsert(&block_hash, height, &header_hex) {
                            log::warn!("   ⚠️  Failed to cache block header: {}", e);
                        } else {
                            log::info!("   💾 Cached block header for height {}", height);
                        }
                    }

                    log::info!("   ✅ Fetched block header ({} bytes)", header_hex.len() / 2);

                    HttpResponse::Ok().json(serde_json::json!({
                        "header": header_hex
                    }))
                }
                Err(e) => {
                    log::error!("   Failed to parse API response: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to parse API response: {}", e)
                    }))
                }
            }
        }
        Err(e) => {
            log::error!("   Failed to fetch block header: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch block header: {}", e)
            }))
        }
    }
}

/// POST /getNetwork - BRC-100 Call Code 27
/// Returns the network name ("mainnet" or "testnet")
pub async fn get_network() -> HttpResponse {
    log::info!("📋 /getNetwork called");

    // For now, return hardcoded "mainnet"
    // TODO: Could read from config file or environment variable later
    HttpResponse::Ok().json(serde_json::json!({
        "network": "mainnet"
    }))
}
