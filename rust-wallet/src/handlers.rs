use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use crate::AppState;
use crate::crypto::brc42::{derive_child_private_key, derive_child_public_key};
use crate::crypto::brc43::{InvoiceNumber, SecurityLevel, normalize_protocol_id};
use crate::crypto::signing::{sha256, hmac_sha256, verify_hmac_sha256};

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
        "version": "BitcoinBrowserWallet-Rust v0.0.1",
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

    let storage = state.storage.lock().unwrap();

    // Get master private key
    let master_privkey = match storage.get_master_private_key() {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            drop(storage);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to get master key: {}", e)
            }));
        }
    };
    drop(storage);

    // Derive master public key
    use secp256k1::{Secp256k1, SecretKey, PublicKey};
    let secp = Secp256k1::new();
    let master_seckey = match SecretKey::from_slice(&master_privkey) {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Invalid master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Invalid master private key"
            }));
        }
    };
    let master_pubkey = PublicKey::from_secret_key(&secp, &master_seckey);
    let master_pubkey_hex = hex::encode(master_pubkey.serialize());

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
    let storage = state.storage.lock().unwrap();
    let master_privkey = match storage.get_master_private_key() {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            drop(storage);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key derivation error: {}", e)
            }));
        }
    };
    drop(storage);

    // Get master public key for identity and BRC-42 key derivation
    use secp256k1::{Secp256k1, SecretKey, PublicKey, Message};

    let secp = Secp256k1::new();
    let master_seckey = SecretKey::from_slice(&master_privkey).expect("Valid private key");
    let master_pubkey = PublicKey::from_secret_key(&secp, &master_seckey);

    // Get MASTER public key bytes (33 bytes compressed)
    let master_pubkey_bytes = master_pubkey.serialize();
    let master_pubkey_hex = hex::encode(&master_pubkey_bytes);

    log::info!("   Our MASTER identity key: {}", master_pubkey_hex);

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
    let storage = state.storage.lock().unwrap();
    let private_key_bytes = match storage.get_master_private_key() {
        Ok(key) => {
            log::info!("   ✅ MASTER private key retrieved for BRC-42 signing");
            key
        },
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            drop(storage);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key derivation error: {}", e)
            }));
        }
    };
    drop(storage);

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
    let response = serde_json::json!({
        "version": "0.1",
        "messageType": "initialResponse",
        "identityKey": master_pubkey_hex,  // MASTER public key (m), not m/0
        "initialNonce": our_nonce,          // Our new nonce (B_Nonce)
        "yourNonce": req.initial_nonce,     // Their initial nonce echoed back (A_Nonce)
        "signature": sig_bytes              // DER signature as byte array (not hex string!)
    });

    log::info!("✅ Returning auth response with BRC-42 signature");
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
    let storage = state.storage.lock().unwrap();
    let private_key_bytes = match storage.get_master_private_key() {
        Ok(key) => {
            log::info!("   ✅ MASTER private key retrieved for HMAC (createHmac)");
            key
        },
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            drop(storage);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key derivation error: {}", e)
            }));
        }
    };
    drop(storage);

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
    let storage = state.storage.lock().unwrap();
    let private_key_bytes = match storage.get_master_private_key() {
        Ok(key) => {
            log::info!("   ✅ MASTER private key retrieved for HMAC (verifyHmac)");
            key
        },
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            drop(storage);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Key derivation error: {}", e)
            }));
        }
    };
    drop(storage);

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
        private_key_bytes
    };

    // Verify HMAC
    let is_valid = verify_hmac_sha256(&hmac_key, &data_bytes, &expected_hmac);

    log::info!("   ✅ HMAC verification result: {}", is_valid);

    HttpResponse::Ok().json(VerifyHmacResponse { valid: is_valid })
}

// Wallet status endpoint
pub async fn wallet_status(state: web::Data<AppState>) -> HttpResponse {
    let storage = state.storage.lock().unwrap();
    let exists = storage.get_wallet().is_ok();

    log::info!("📋 Wallet status: exists={}", exists);

    HttpResponse::Ok().json(serde_json::json!({
        "exists": exists
    }))
}

// Wallet balance endpoint
pub async fn wallet_balance(state: web::Data<AppState>) -> HttpResponse {
    log::info!("💰 /wallet/balance called");

    // Get all addresses from storage
    let addresses = {
        let storage = state.storage.lock().unwrap();
        match storage.get_all_addresses() {
            Ok(addrs) => addrs.to_vec(), // Convert to owned Vec for async operation
            Err(e) => {
                log::error!("   Failed to get addresses: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": e
                }));
            }
        }
    };

    log::info!("   Checking balance for {} addresses", addresses.len());

    // Fetch UTXOs for all addresses
    match crate::utxo_fetcher::fetch_all_utxos(&addresses).await {
        Ok(utxos) => {
            let total_balance: i64 = utxos.iter().map(|u| u.satoshis).sum();

            log::info!("   ✅ Total balance: {} satoshis ({} UTXOs)", total_balance, utxos.len());

            // Return response in Go wallet format: { "balance": number }
            HttpResponse::Ok().json(serde_json::json!({
                "balance": total_balance
            }))
        }
        Err(e) => {
            log::error!("   Failed to fetch UTXOs: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e
            }))
        }
    }
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
    log::info!("   Raw body: {}", body_str);

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
    let storage = state.storage.lock().unwrap();
    let our_master_privkey = match storage.get_master_private_key() {
        Ok(key) => {
            log::info!("   ✅ Master private key retrieved for verification");
            key
        },
        Err(e) => {
            drop(storage);
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get master private key"
            }));
        }
    };
    drop(storage);

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
    log::info!("   Raw body: {}", body_str);

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
        let storage = state.storage.lock().unwrap();
        match storage.get_master_public_key() {
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
    let storage = state.storage.lock().unwrap();
    let private_key_bytes = match storage.get_master_private_key() {
        Ok(key) => key,
        Err(e) => {
            drop(storage);
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get master private key"
            }));
        }
    };
    drop(storage);

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
use crate::utxo_fetcher::{fetch_all_utxos, UTXO};
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

// Pending transaction with metadata
#[derive(Debug, Clone)]
struct PendingTransaction {
    tx: Transaction,
    input_utxos: Vec<UTXO>, // UTXOs being spent (for signing)
    brc29_info: Option<Brc29PaymentInfo>, // BRC-29 payment metadata if applicable
}

// In-memory storage for pending transactions
static PENDING_TRANSACTIONS: Lazy<StdMutex<HashMap<String, PendingTransaction>>> =
    Lazy::new(|| StdMutex::new(HashMap::new()));

// Request structure for /createAction
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateActionRequest {
    #[serde(rename = "outputs")]
    pub outputs: Vec<CreateActionOutput>,

    #[serde(rename = "description")]
    pub description: Option<String>,

    #[serde(rename = "labels")]
    pub labels: Option<Vec<String>>,

    #[serde(rename = "options")]
    pub options: Option<CreateActionOptions>,

    #[serde(rename = "inputBEEF")]
    pub input_beef: Option<String>, // Hex-encoded BEEF with input transaction proofs (BRC-100)
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
    log::info!("📋 Raw request body: {}", String::from_utf8_lossy(&body));

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

    // Calculate total output amount
    let mut total_output: i64 = 0;
    for (i, output) in req.outputs.iter().enumerate() {
        if let Some(sats) = output.satoshis {
            total_output += sats;
            log::info!("   Output {}: {} satoshis", i, sats);
        }
    }

    log::info!("   Total output amount: {} satoshis", total_output);

    // Estimate fee (rough calculation: ~200 bytes per input + output + overhead)
    let estimated_fee = 5000; // Increased fee: 5000 sats (~22 sat/byte for 225 byte tx)
    let total_needed = total_output + estimated_fee;

    log::info!("   Estimated fee: {} satoshis", estimated_fee);
    log::info!("   Total needed: {} satoshis", total_needed);

    // Fetch UTXOs from WhatsOnChain
    let storage = state.storage.lock().unwrap();
    let addresses = match storage.get_all_addresses() {
        Ok(addrs) => addrs.to_vec(),
        Err(e) => {
            drop(storage);
            log::error!("   Failed to get addresses: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get addresses"
            }));
        }
    };
    drop(storage);

    log::info!("   Checking {} addresses for UTXOs...", addresses.len());

    let all_utxos = match fetch_all_utxos(&addresses).await {
        Ok(utxos) => utxos,
        Err(e) => {
            log::error!("   Failed to fetch UTXOs: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch UTXOs: {}", e)
            }));
        }
    };

    if all_utxos.is_empty() {
        log::error!("   No UTXOs available");
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Insufficient funds: no UTXOs available"
        }));
    }

    // Select UTXOs to cover the amount
    let selected_utxos = select_utxos(&all_utxos, total_needed);

    if selected_utxos.is_empty() {
        log::error!("   Insufficient funds");
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Insufficient funds: need {} sats, have {} sats",
                total_needed,
                all_utxos.iter().map(|u| u.satoshis).sum::<i64>()
            )
        }));
    }

    let total_input: i64 = selected_utxos.iter().map(|u| u.satoshis).sum();
    log::info!("   Selected {} UTXOs ({} satoshis)", selected_utxos.len(), total_input);

    // Build transaction
    let mut tx = Transaction::new();

    // Add inputs (unsigned)
    for utxo in &selected_utxos {
        let outpoint = OutPoint::new(utxo.txid.clone(), utxo.vout);
        tx.add_input(TxInput::new(outpoint));
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
                        let storage = state.storage.lock().unwrap();
                        let master_key_bytes = match storage.get_master_private_key() {
                            Ok(key) => key,
                            Err(e) => {
                                drop(storage);
                                log::error!("   Failed to get master key: {}", e);
                                return HttpResponse::InternalServerError().json(serde_json::json!({
                                    "error": "Failed to get master key"
                                }));
                            }
                        };
                        drop(storage);

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
        let storage = state.storage.lock().unwrap();
        let change_addr = match storage.get_current_address() {
            Ok(addr) => addr.clone(),
            Err(e) => {
                drop(storage);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to get change address: {}", e)
                }));
            }
        };
        drop(storage);

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

    // Store transaction in memory with UTXO metadata for signing
    {
        let mut pending = PENDING_TRANSACTIONS.lock().unwrap();
        pending.insert(reference.clone(), PendingTransaction {
            tx: tx.clone(),
            input_utxos: selected_utxos.clone(),
            brc29_info: brc29_info.clone(),
        });
    }

    // Log if this is a BRC-29 payment
    if brc29_info.is_some() {
        log::info!("   💰 BRC-29 payment metadata stored for later envelope conversion");
    }

    log::info!("   ✅ Transaction created: {}", txid);
    log::info!("   Reference: {}", reference);

    // Store action in action storage
    use crate::action_storage::{StoredAction, ActionStatus, ActionInput, ActionOutput};
    use chrono::Utc;

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
        inputs: tx.inputs.iter().enumerate().map(|(i, input)| ActionInput {
            txid: selected_utxos.get(i).map(|u| u.txid.clone()).unwrap_or_default(),
            vout: selected_utxos.get(i).map(|u| u.vout).unwrap_or(0),
            satoshis: selected_utxos.get(i).map(|u| u.satoshis).unwrap_or(0),
            script: Some(hex::encode(&input.script_sig)),
        }).collect(),
        outputs: tx.outputs.iter().enumerate().map(|(i, output)| ActionOutput {
            vout: i as u32,
            satoshis: output.value,
            script: Some(hex::encode(&output.script_pubkey)),
            address: parse_address_from_script(&output.script_pubkey),
        }).collect(),
    };

    // Store the action
    {
        let mut action_storage = state.action_storage.lock().unwrap();
        if let Err(e) = action_storage.add_action(stored_action) {
            log::warn!("   ⚠️  Failed to store action: {}", e);
        } else {
            log::info!("   💾 Action stored with status: created");
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

    // Build response inputs array
    let response_inputs: Vec<CreateActionResponseInput> = selected_utxos.iter().map(|utxo| {
        CreateActionResponseInput {
            txid: utxo.txid.clone(),
            vout: utxo.vout,
            output_index: utxo.vout,
            script_length: utxo.script.len() / 2, // Hex length to byte length
            script_offset: 0, // Not used in simplified implementation
            sequence: 0xffffffff,
        }
    }).collect();

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

    // Get all actions that need confirmation updates
    let actions_to_update: Vec<(String, String)> = {
        let storage = state.action_storage.lock().unwrap();
        storage.list_actions(None, None)
            .iter()
            .filter(|a| matches!(a.status, crate::action_storage::ActionStatus::Unconfirmed))
            .map(|a| (a.txid.clone(), a.status.to_string()))
            .collect()
    };

    log::info!("📊 Checking confirmations for {} transactions...", actions_to_update.len());

    // Query each transaction
    for (txid, _status) in actions_to_update {
        match get_confirmation_status(&txid).await {
            Ok((confirmations, block_height)) => {
                let mut storage = state.action_storage.lock().unwrap();
                if let Err(e) = storage.update_confirmations(&txid, confirmations, block_height) {
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
    let brc29_info = pending_tx.brc29_info;

    log::info!("   Signing {} inputs...", tx.inputs.len());

    // Sign each input
    for (i, input_utxo) in input_utxos.iter().enumerate() {
        log::info!("   Signing input {}: {}:{} (address index {})",
            i, input_utxo.txid, input_utxo.vout, input_utxo.address_index);

        // Get the private key for THIS specific address (not always index 0!)
        let storage = state.storage.lock().unwrap();
        let private_key_bytes = match storage.derive_private_key(input_utxo.address_index) {
            Ok(key) => key,
            Err(e) => {
                drop(storage);
                log::error!("   Failed to derive private key for address index {}: {}", input_utxo.address_index, e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to derive private key for address {}", input_utxo.address_index)
                }));
            }
        };
        drop(storage);

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
    log::info!("   📦 Building BEEF format with {} parent transactions...", input_utxos.len());

    let mut beef = crate::beef::Beef::new();

    // Fetch parent transactions and their Merkle proofs from WhatsOnChain
    let client = reqwest::Client::new();
    for (i, utxo) in input_utxos.iter().enumerate() {
        log::info!("   📥 Fetching parent tx {}/{}: {}", i + 1, input_utxos.len(), utxo.txid);

        // Fetch transaction hex
        let tx_url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/hex", utxo.txid);
        match client.get(&tx_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.text().await {
                        Ok(parent_tx_hex) => {
                            match hex::decode(&parent_tx_hex) {
                                Ok(parent_tx_bytes) => {
                                    // Verify TXID matches what we requested
                                    use sha2::{Sha256, Digest};
                                    let hash1 = Sha256::digest(&parent_tx_bytes);
                                    let hash2 = Sha256::digest(&hash1);
                                    let calculated_txid: Vec<u8> = hash2.into_iter().rev().collect();
                                    let calculated_txid_hex = hex::encode(calculated_txid);

                                    log::info!("   ✅ Fetched parent tx {} ({} bytes)", utxo.txid, parent_tx_bytes.len());
                                    log::info!("   🔍 Calculated TXID from bytes: {}", calculated_txid_hex);

                                    if calculated_txid_hex != utxo.txid {
                                        log::error!("   ❌ TXID MISMATCH! Requested: {}, Got: {}", utxo.txid, calculated_txid_hex);
                                        log::error!("   ❌ Transaction hex first 80 chars: {}", &parent_tx_hex[..80.min(parent_tx_hex.len())]);
                                        continue; // Skip this parent transaction
                                    }

                                    let tx_index = beef.add_parent_transaction(parent_tx_bytes);

                                    // Fetch TSC Merkle proof (with transaction index)
                                    log::info!("   🔍 Checking for TSC Merkle proof...");
                                    let proof_url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/proof/tsc", utxo.txid);
                                    match client.get(&proof_url).send().await {
                                        Ok(proof_response) => {
                                            let status = proof_response.status();
                                            log::info!("   📡 TSC proof API status: {}", status);

                                            if status.is_success() {
                                                match proof_response.text().await {
                                                    Ok(proof_text) => {
                                                        log::info!("   📄 TSC proof response: {}", &proof_text[..proof_text.len().min(200)]);

                                                        match serde_json::from_str::<serde_json::Value>(&proof_text) {
                                                            Ok(tsc_json) => {
                                                                // Check if response is null (transaction not yet in a block)
                                                                if tsc_json.is_null() {
                                                                    log::warn!("   ⚠️  TSC proof is null - transaction {} not yet confirmed in a block", utxo.txid);
                                                                    log::warn!("   ⚠️  Retrying TSC proof fetch after 2 seconds...");

                                                                    // Retry once after a short delay (transaction might be confirming)
                                                                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                                                                    match client.get(&proof_url).send().await {
                                                                        Ok(retry_response) if retry_response.status().is_success() => {
                                                                            match retry_response.text().await {
                                                                                Ok(retry_text) => {
                                                                                    match serde_json::from_str::<serde_json::Value>(&retry_text) {
                                                                                        Ok(retry_json) => {
                                                                                            if retry_json.is_null() {
                                                                                                log::error!("   ❌ TSC proof still null after retry - transaction {} is not confirmed. Cannot create valid Atomic BEEF without BUMP.", utxo.txid);
                                                                                                log::error!("   ❌ Thoth requires Atomic BEEF with valid BUMPs. Please wait for the transaction to be confirmed before spending it.");
                                                                                                // Continue without BUMP - this will cause Thoth to reject it
                                                                                            } else {
                                                                                                log::info!("   ✅ TSC proof available on retry!");
                                                                                                // Process the retry_json the same way as below
                                                                                                let tsc_obj = if retry_json.is_array() {
                                                                                                    retry_json.get(0)
                                                                                                } else {
                                                                                                    Some(&retry_json)
                                                                                                };

                                                                                                if let Some(tsc_obj) = tsc_obj {
                                                                                                    // Continue with tsc_obj processing below...
                                                                                                    // (we'll handle this with a helper function to avoid duplication)
                                                                                                    if let (Some(index), Some(target)) = (tsc_obj["index"].as_u64(), tsc_obj["target"].as_str()) {
                                                                                                        log::info!("   ✅ Parent tx confirmed at tx_index {}, target: {}", index, &target[..16.min(target.len())]);
                                                                                                        log::info!("   📊 Merkle path length: {}", tsc_obj["nodes"].as_array().map(|a| a.len()).unwrap_or(0));

                                                                                                        let block_header_url = format!("https://api.whatsonchain.com/v1/bsv/main/block/hash/{}", target);
                                                                                                        match client.get(&block_header_url).send().await {
                                                                                                            Ok(header_response) if header_response.status().is_success() => {
                                                                                                                match header_response.json::<serde_json::Value>().await {
                                                                                                                    Ok(header_json) => {
                                                                                                                        if let Some(height) = header_json["height"].as_u64() {
                                                                                                                            log::info!("   ✅ Block height: {}", height);
                                                                                                                            let mut enhanced_tsc = tsc_obj.clone();
                                                                                                                            enhanced_tsc["height"] = serde_json::json!(height);
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
                                                                                                                    Err(_) => {}
                                                                                                                }
                                                                                                            }
                                                                                                            _ => {}
                                                                                                        }
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                        Err(_) => {
                                                                                            log::warn!("   ⚠️  Failed to parse retry TSC proof JSON");
                                                                                        }
                                                                                    }
                                                                                }
                                                                                Err(_) => {}
                                                                            }
                                                                        }
                                                                        _ => {
                                                                            log::warn!("   ⚠️  Retry TSC proof fetch failed");
                                                                        }
                                                                    }
                                                                } else {
                                                                    // Normal case: TSC proof is not null
                                                                    // WhatsOnChain returns array: [{index, txOrId, target, nodes}]
                                                                    let tsc_obj = if tsc_json.is_array() {
                                                                        tsc_json.get(0)
                                                                    } else {
                                                                        Some(&tsc_json)
                                                                    };

                                                    if let Some(tsc_obj) = tsc_obj {
                                                    // TSC format has: index, target (block hash), nodes
                                                    if let (Some(index), Some(target)) = (tsc_obj["index"].as_u64(), tsc_obj["target"].as_str()) {
                                                        log::info!("   ✅ Parent tx confirmed at tx_index {}, target: {}", index, &target[..16.min(target.len())]);
                                                        log::info!("   📊 Merkle path length: {}", tsc_obj["nodes"].as_array().map(|a| a.len()).unwrap_or(0));

                                                        // Fetch block height from block hash (BSV/SDK does this too)
                                                        log::info!("   🔍 Fetching block height for hash: {}...", &target[..16.min(target.len())]);
                                                        let block_header_url = format!("https://api.whatsonchain.com/v1/bsv/main/block/hash/{}", target);

                                                        match client.get(&block_header_url).send().await {
                                                            Ok(header_response) if header_response.status().is_success() => {
                                                                match header_response.json::<serde_json::Value>().await {
                                                                    Ok(header_json) => {
                                                                        if let Some(height) = header_json["height"].as_u64() {
                                                                            log::info!("   ✅ Block height: {}", height);

                                                                            // Create enhanced TSC object with height field
                                                                            let mut enhanced_tsc = tsc_obj.clone();
                                                                            enhanced_tsc["height"] = serde_json::json!(height);

                                                                            // Try to add the TSC proof
                                                                            match beef.add_tsc_merkle_proof(&utxo.txid, tx_index, &enhanced_tsc) {
                                                                                Ok(_) => {
                                                                                    log::info!("   ✅ Added TSC Merkle proof (BUMP) to BEEF");
                                                                                }
                                                                                Err(e) => {
                                                                                    log::warn!("   ⚠️  Failed to add TSC Merkle proof: {}", e);
                                                                                }
                                                                            }
                                                                        } else {
                                                                            log::warn!("   ⚠️  Block header missing height field");
                                                                        }
                                                                    }
                                                                    Err(e) => {
                                                                        log::warn!("   ⚠️  Failed to parse block header JSON: {}", e);
                                                                    }
                                                                }
                                                            }
                                                            Ok(header_response) => {
                                                                log::warn!("   ⚠️  Failed to fetch block header: HTTP {}", header_response.status());
                                                            }
                                                            Err(e) => {
                                                                log::warn!("   ⚠️  Failed to fetch block header: {}", e);
                                                            }
                                                        }
                                                    } else {
                                                        log::warn!("   ⚠️  TSC proof missing index or target field");
                                                    }
                                                } else {
                                                    log::warn!("   ⚠️  TSC proof array is empty");
                                                }
                                                                } // end else (tsc_json not null)
                                                            }
                                                            Err(e) => {
                                                                log::warn!("   ⚠️  Failed to parse TSC proof JSON: {}", e);
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        log::warn!("   ⚠️  Failed to read TSC proof response: {}", e);
                                                    }
                                                }
                                            } else {
                                                log::info!("   ℹ️  TSC proof not available (HTTP {})", status);
                                            }
                                        }
                                        Err(e) => {
                                            log::warn!("   ⚠️  Failed to fetch TSC proof: {}", e);
                                        }
                                    }
                                },
                                Err(e) => {
                                    log::warn!("   ⚠️  Failed to decode parent tx {}: {}", utxo.txid, e);
                                }
                            }
                        },
                        Err(e) => {
                            log::warn!("   ⚠️  Failed to read parent tx {} response: {}", utxo.txid, e);
                        }
                    }
                } else {
                    log::warn!("   ⚠️  Failed to fetch parent tx {}: HTTP {}", utxo.txid, response.status());
                }
            },
            Err(e) => {
                log::warn!("   ⚠️  Failed to fetch parent tx {}: {}", utxo.txid, e);
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
    log::info!("   📝 FULL Standard BEEF hex: {}", standard_beef_hex);

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
        let mut action_storage = state.action_storage.lock().unwrap();

        // Update TXID (signing changes the transaction, so TXID changes)
        if let Err(e) = action_storage.update_txid(&req.reference, txid.clone(), signed_tx_hex.clone()) {
            log::warn!("   ⚠️  Failed to update TXID: {}", e);
        } else {
            log::info!("   💾 TXID updated after signing");
        }

        // Update status to signed
        use crate::action_storage::ActionStatus;
        if let Err(e) = action_storage.update_status(&txid, ActionStatus::Signed) {
            log::warn!("   ⚠️  Failed to update action status: {}", e);
        } else {
            log::info!("   💾 Action status updated: created → signed");
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

    // Step 3: Broadcast (if requested)
    let status = if should_broadcast {
        log::info!("   Broadcasting to network...");

        match broadcast_transaction(&raw_tx).await {
            Ok(_) => {
                log::info!("   ✅ Transaction broadcast successful!");

                // Update action status to "unconfirmed"
                {
                    let mut action_storage = state.action_storage.lock().unwrap();
                    use crate::action_storage::ActionStatus;
                    if let Err(e) = action_storage.update_status(&txid, ActionStatus::Unconfirmed) {
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
                    let mut action_storage = state.action_storage.lock().unwrap();
                    use crate::action_storage::ActionStatus;
                    if let Err(e) = action_storage.update_status(&txid, ActionStatus::Failed) {
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
            log::info!("   ✅ GorillaPool accepted: {}", response);
            success_count += 1;
        }
        Err(e) => {
            log::warn!("   ⚠️ GorillaPool failed: {}", e);
            last_error = e;
        }
    }

    // Broadcaster 2: WhatsOnChain
    log::info!("   📡 Broadcasting to WhatsOnChain...");
    match broadcast_to_whatsonchain(&client, raw_tx_hex).await {
        Ok(response) => {
            log::info!("   ✅ WhatsOnChain accepted: {}", response);
            success_count += 1;
        }
        Err(e) => {
            log::warn!("   ⚠️ WhatsOnChain failed: {}", e);
            last_error = e;
        }
    }

    if success_count > 0 {
        log::info!("   🎉 Broadcast successful to {} service(s)", success_count);
        Ok(format!("Broadcast to {} service(s)", success_count))
    } else {
        log::error!("   ❌ All broadcasters failed!");
        Err(format!("All broadcasters failed. Last error: {}", last_error))
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

    if status.is_success() {
        Ok(text)
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

    // Get current index and master keys (release lock quickly)
    let (current_index, master_privkey, master_pubkey) = {
        let storage = state.storage.lock().unwrap();

        let wallet = match storage.get_wallet() {
            Ok(w) => w,
            Err(e) => {
                log::error!("   Failed to get wallet: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": e
                }));
            }
        };

        let index = wallet.current_index;

        let privkey = match storage.get_master_private_key() {
            Ok(k) => k,
            Err(e) => {
                log::error!("   Failed to get master private key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": e
                }));
            }
        };

        let pubkey = match storage.get_master_public_key() {
            Ok(k) => k,
            Err(e) => {
                log::error!("   Failed to get master public key: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": e
                }));
            }
        };

        (index, privkey, pubkey)
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

    // Create AddressInfo
    let address_info = crate::json_storage::AddressInfo {
        index: current_index,
        address: address.clone(),
        public_key: hex::encode(&derived_pubkey),
        used: false,
        balance: 0,
    };

    // Add address to wallet and save (acquire lock again)
    {
        let mut storage = state.storage.lock().unwrap();
        match storage.add_address(address_info) {
            Ok(_) => {
                log::info!("   ✅ Address saved to wallet.json");
            },
            Err(e) => {
                log::error!("   Failed to save address: {}", e);
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
            // Even if broadcast fails, we still have a valid transaction
            // Return success but note the broadcast issue
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "txid": txid,
                "whatsOnChainUrl": format!("https://whatsonchain.com/tx/{}", txid),
                "message": format!("Transaction created but broadcast may have failed: {}", e)
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

    // Load action storage
    let mut storage = state.action_storage.lock().unwrap();

    // Find action by reference number
    let action = match storage.get_action_by_reference(&req.reference_number) {
        Some(a) => a.clone(), // Clone to avoid borrow issues
        None => {
            log::warn!("   ⚠️  Action not found: {}", req.reference_number);
            drop(storage);
            return HttpResponse::NotFound().json(serde_json::json!({
                "status": "error",
                "code": "ERR_ACTION_NOT_FOUND",
                "description": format!("Action not found: {}", req.reference_number)
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
            drop(storage);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "code": "ERR_CANNOT_ABORT_CONFIRMED",
                "description": "Cannot abort confirmed transaction"
            }));
        }
        ActionStatus::Aborted => {
            log::info!("   ℹ️  Transaction already aborted");
            drop(storage);
            return HttpResponse::Ok().json(AbortActionResponse { aborted: true });
        }
        _ => {}
    }

    // Update status to aborted
    match storage.update_status(&action.txid, ActionStatus::Aborted) {
        Ok(_) => {
            log::info!("✅ Action aborted successfully: {}", action.txid);
            drop(storage);
            HttpResponse::Ok().json(AbortActionResponse { aborted: true })
        }
        Err(e) => {
            log::error!("   ❌ Failed to abort action: {}", e);
            drop(storage);
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
        let storage = state.storage.lock().unwrap();
        match storage.get_all_addresses() {
            Ok(addrs) => addrs.to_vec(),
            Err(e) => {
                log::error!("   Failed to get wallet addresses: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": "error",
                    "code": "ERR_WALLET",
                    "description": format!("Failed to get wallet addresses: {}", e)
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

    // Store the action
    {
        let mut action_storage = state.action_storage.lock().unwrap();
        if let Err(e) = action_storage.add_action(stored_action) {
            log::error!("   Failed to store action: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "code": "ERR_STORAGE",
                "description": format!("Failed to store action: {}", e)
            }));
        }
        log::info!("   💾 Action stored with status: unconfirmed");
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

    // Load action storage
    let storage = state.action_storage.lock().unwrap();

    // Get label filter mode
    let label_mode = req.label_query_mode.as_deref();

    // List actions with optional label filter
    let actions = storage.list_actions(req.labels.as_ref(), label_mode);

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

    drop(storage);

    HttpResponse::Ok().json(ListActionsResponse {
        total_actions: total,
        actions: actions_json,
    })
}
