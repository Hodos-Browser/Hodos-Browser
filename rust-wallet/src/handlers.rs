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

/// Fallback fee rate in satoshis per kilobyte (1 sat/byte = 1000 sat/kb)
/// Used when ARC policy endpoint is unreachable. Dynamic fee rate is fetched
/// from ARC /v1/policy and cached in FeeRateCache (see fee_rate_cache.rs).
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
// Returns the master identity key when identityKey=true or no protocol params,
// otherwise derives a child public key using BRC-42 key derivation.
pub async fn get_public_key(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    log::info!("📋 /getPublicKey called");

    // Parse optional JSON body (empty body = return identity key)
    let req: serde_json::Value = if body.is_empty() {
        serde_json::json!({})
    } else {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                log::error!("   JSON parse error: {}", e);
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Invalid JSON: {}", e)
                }));
            }
        }
    };

    let identity_key = req.get("identityKey").and_then(|v| v.as_bool()).unwrap_or(false);
    let protocol_id = req.get("protocolID");
    let key_id = req.get("keyID").and_then(|v| v.as_str());
    let for_self = req.get("forSelf").and_then(|v| v.as_bool()).unwrap_or(false);

    // Get master public key (always needed)
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

    // If identityKey=true or no protocol params, return master identity key
    if identity_key || protocol_id.is_none() || key_id.is_none() {
        let master_pubkey_hex = hex::encode(&master_pubkey);
        log::info!("   Returning MASTER identity key: {}", master_pubkey_hex);
        return HttpResponse::Ok().json(serde_json::json!({
            "publicKey": master_pubkey_hex
        }));
    }

    // Parse protocolID for BRC-42 derivation
    let protocol_id = protocol_id.unwrap();
    let key_id = key_id.unwrap();

    let protocol_id_str = if let serde_json::Value::Array(arr) = protocol_id {
        if arr.len() == 2 {
            if let (Some(level), Some(name)) = (arr[0].as_u64(), arr[1].as_str()) {
                format!("{}-{}", level, name)
            } else {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid protocolID format: expected [number, string]"
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

    let invoice = format!("{}-{}", protocol_id_str, key_id);
    log::info!("   BRC-43 invoice: {}, forSelf: {}", invoice, for_self);

    // Resolve counterparty public key for BRC-42 derivation
    // "anyone" → PrivateKey(1).toPublicKey(), "self" → our master pubkey, hex → parse
    let counterparty_val = req.get("counterparty").cloned();
    let counterparty_pubkey = match resolve_counterparty_pubkey(&counterparty_val, &master_pubkey) {
        Ok(pubkey) => pubkey,
        Err(e) => {
            log::error!("   Failed to resolve counterparty: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid counterparty: {}", e)
            }));
        }
    };

    // Need master private key for BRC-42 ECDH derivation
    let db = state.database.lock().unwrap();
    let master_privkey = match crate::database::get_master_private_key_from_db(&db) {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Failed to get master private key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get master private key"
            }));
        }
    };
    drop(db);

    let derived_pubkey_hex = if for_self {
        // forSelf=true: derive OUR child public key
        // SDK equivalent: rootKey.deriveChild(counterparty, invoiceNumber).toPublicKey()
        match derive_child_private_key(&master_privkey, &counterparty_pubkey, &invoice) {
            Ok(child_privkey) => {
                use secp256k1::{Secp256k1, SecretKey, PublicKey};
                let secp = Secp256k1::new();
                match SecretKey::from_slice(&child_privkey) {
                    Ok(secret) => {
                        hex::encode(PublicKey::from_secret_key(&secp, &secret).serialize())
                    }
                    Err(e) => {
                        log::error!("   Invalid derived private key: {}", e);
                        return HttpResponse::InternalServerError().json(serde_json::json!({
                            "error": "Key derivation produced invalid key"
                        }));
                    }
                }
            }
            Err(e) => {
                log::error!("   BRC-42 derivation failed: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Key derivation failed: {}", e)
                }));
            }
        }
    } else {
        // forSelf=false: derive counterparty's child public key
        // SDK equivalent: counterparty.deriveChild(rootKey, invoiceNumber)
        match derive_child_public_key(&master_privkey, &counterparty_pubkey, &invoice) {
            Ok(child_pubkey) => hex::encode(&child_pubkey),
            Err(e) => {
                log::error!("   BRC-42 derivation failed: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Key derivation failed: {}", e)
                }));
            }
        }
    };

    log::info!("   ✅ Derived child public key (forSelf={}): {}", for_self, derived_pubkey_hex);

    // Cache forSelf=true derivations so signAction can sign PushDrop inputs later.
    // Persisted to database so it survives wallet restarts.
    if for_self {
        log::info!("   📋 Caching derived key (forSelf=true) for PushDrop signing...");
        log::info!("      pubkey: {}", derived_pubkey_hex);
        log::info!("      invoice: {}", invoice);
        log::info!("      counterparty: {}", hex::encode(&counterparty_pubkey));

        // In-memory cache (fast path)
        {
            let mut cache = state.derived_key_cache.lock().unwrap();
            cache.insert(derived_pubkey_hex.clone(), crate::DerivedKeyInfo {
                invoice: invoice.clone(),
                counterparty_pubkey: counterparty_pubkey.clone(),
            });
            log::info!("      ✅ In-memory cache: written (total entries: {})", cache.len());
        }

        // Persistent database cache (survives restarts)
        {
            let db = state.database.lock().unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let counterparty_hex = hex::encode(&counterparty_pubkey);
            match db.connection().execute(
                "INSERT OR REPLACE INTO derived_key_cache (derived_pubkey, invoice, counterparty_pubkey, created_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![derived_pubkey_hex, invoice, counterparty_hex, now],
            ) {
                Ok(rows) => log::info!("      ✅ DB cache: written ({} row(s) affected)", rows),
                Err(e) => log::error!("      ❌ DB cache WRITE FAILED: {} — signAction will not find this key after restart!", e),
            }
        }
    }

    HttpResponse::Ok().json(serde_json::json!({
        "publicKey": derived_pubkey_hex
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
    log::info!("   Counterparty: {:?}", req.counterparty);

    // Parse keyID - should be a string or small byte array (max 800 bytes per BRC-43 spec)
    // For "server hmac" protocol, keyID is typically a nonce (16-32 bytes)
    let key_id_str: String = match &req.key_id {
        serde_json::Value::String(s) if !s.is_empty() => {
            log::info!("   Key ID (string): {} bytes", s.len());
            s.clone()
        },
        serde_json::Value::Array(arr) => {
            // Byte array - convert to base64 to preserve binary data
            let bytes: Vec<u8> = arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            log::info!("   Key ID (array): {} bytes", bytes.len());
            // Use base64 encoding to preserve all bytes
            general_purpose::STANDARD.encode(&bytes)
        },
        _ => {
            log::error!("   keyID is missing or invalid type");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "keyID must be string or byte array"
            }));
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
                        .filter(|u| u.address_index == addr.index)
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
                    .filter(|u| u.address_index == addr.index)
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

    // Resolve counterparty public key for BRC-42 derivation
    // BRC-42 always requires a counterparty public key:
    //   "anyone" → PrivateKey(1).toPublicKey() (well-known generator point G)
    //   "self" → our own master public key
    //   hex string → parse as 33-byte compressed public key
    let counterparty_pubkey: Vec<u8> = match &req.counterparty {
        serde_json::Value::String(s) if s == "anyone" => {
            log::info!("   Counterparty: anyone (PrivateKey(1).toPublicKey())");
            use secp256k1::{Secp256k1, SecretKey, PublicKey};
            let secp = Secp256k1::new();
            let mut anyone_privkey = [0u8; 32];
            anyone_privkey[31] = 1; // Private key = 1
            let secret = SecretKey::from_slice(&anyone_privkey).unwrap();
            PublicKey::from_secret_key(&secp, &secret).serialize().to_vec()
        }
        serde_json::Value::String(s) if s == "self" => {
            log::info!("   Counterparty: self (our master public key)");
            match hex::decode(&our_identity_key) {
                Ok(bytes) if bytes.len() == 33 => bytes,
                _ => {
                    log::error!("   Failed to decode master public key for 'self' counterparty");
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "Failed to resolve 'self' counterparty"
                    }));
                }
            }
        }
        serde_json::Value::String(hex_pubkey) => {
            match hex::decode(hex_pubkey) {
                Ok(bytes) if bytes.len() == 33 => {
                    log::info!("   Counterparty pubkey: {}", hex_pubkey);
                    bytes
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

    // BRC-42 child private key derivation (always derived, even for "anyone" and "self")
    let child_privkey = match derive_child_private_key(&private_key_bytes, &counterparty_pubkey, &invoice) {
        Ok(key) => key,
        Err(e) => {
            log::error!("   Failed to derive BRC-42 child key: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to derive child key: {}", e)
            }));
        }
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
    reservation_placeholder: Option<String>, // pending-{timestamp} for optimistic lock rollback
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

    // BRC-100 basket and tag support
    #[serde(rename = "basket")]
    pub basket: Option<String>, // Optional basket name for UTXO tracking (<300 chars)

    #[serde(rename = "tags")]
    pub tags: Option<Vec<String>>, // Optional tags for filtering (<300 chars each)
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

    /// BRC-100: TXIDs of previously-created noSend transactions to broadcast
    /// alongside this transaction. Used for transaction chaining where the first
    /// tx is created with noSend=true, then the second tx uses sendWith to
    /// broadcast both together.
    #[serde(rename = "sendWith", default)]
    pub send_with: Option<Vec<String>>,
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
    #[serde(rename = "sendWithResults", skip_serializing_if = "Option::is_none")]
    pub send_with_results: Option<Vec<SendWithResult>>, // BRC-100: broadcast results array
    /// BRC-100: Change outpoints from noSend transactions, formatted as "txid.vout".
    /// The SDK uses these to chain transactions by passing them back as inputs in subsequent createAction calls.
    #[serde(rename = "noSendChange", skip_serializing_if = "Option::is_none")]
    pub no_send_change: Option<Vec<String>>,
    /// BRC-100: Signable transaction for two-phase flow.
    /// Present when inputs need SDK-side signing (PushDrop tokens, etc.).
    /// Contains AtomicBEEF bytes (for sighash computation) + reference (for signAction).
    #[serde(rename = "signableTransaction", skip_serializing_if = "Option::is_none")]
    pub signable_transaction: Option<SignableTransactionResponse>,
}

/// Two-phase signing response: the SDK uses `tx` to compute sighashes,
/// then calls signAction with the `reference` and computed unlock scripts.
#[derive(Debug, Serialize, Deserialize)]
pub struct SignableTransactionResponse {
    /// AtomicBEEF bytes containing the unsigned transaction + parent chain.
    /// SDK parses this to compute sighash preimages for each unsigned input.
    pub tx: Vec<u8>,
    /// Reference to pass to signAction along with the computed spends.
    pub reference: String,
}

/// BRC-100 broadcast result per transaction
/// Status values: 'unproven' (accepted, not mined), 'sending' (propagating), 'failed'
#[derive(Debug, Serialize, Deserialize)]
pub struct SendWithResult {
    pub txid: String,
    pub status: String, // "unproven" | "sending" | "failed"
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
    log::info!("📋 Raw request body ({} bytes):", body.len());
    // Log full request in chunks of 2000 chars for complete visibility
    let body_str = String::from_utf8_lossy(&body);
    let body_len = body_str.len();
    if body_len <= 4000 {
        log::info!("📋 FULL REQUEST: {}", body_str);
    } else {
        log::info!("📋 REQUEST (first 4000 of {} chars): {}", body_len, &body_str[..4000]);
    }

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
    log::info!("   Labels: {:?}", req.labels);
    log::info!("   Outputs: {}", req.outputs.len());
    for (i, output) in req.outputs.iter().enumerate() {
        log::info!("   Output[{}]: satoshis={:?}, basket={:?}, tags={:?}, description={:?}",
            i, output.satoshis, output.basket, output.tags, output.output_description);
        if let Some(ref script) = output.script {
            log::info!("   Output[{}]: lockingScript ({} hex chars): {}...",
                i, script.len(), &script[..std::cmp::min(120, script.len())]);
        }
        if let Some(ref addr) = output.address {
            log::info!("   Output[{}]: address={}", i, addr);
        }
        if let Some(ref ci) = output.custom_instructions {
            log::info!("   Output[{}]: customInstructions={}", i, ci);
        }
    }
    log::info!("   Inputs provided: {}", req.inputs.as_ref().map(|i| i.len()).unwrap_or(0));
    log::info!("   InputBEEF provided: {}", req.input_beef.is_some());
    if let Some(ref options) = req.options {
        log::info!("   Options: acceptDelayedBroadcast={:?}, signAndProcess={:?}, noSend={:?}, randomizeOutputs={:?}",
            options.accept_delayed_broadcast,
            options.sign_and_process,
            options.no_send,
            options.randomize_outputs);
    }

    // ============================================================
    // SERIALIZATION LOCK: Only one createAction can run at a time.
    // This prevents race conditions where:
    // - Two calls select UTXOs from the same parent transaction
    // - Second call's BEEF builder can't find first call's parent (not yet broadcast)
    // - Concurrent signing creates conflicting transaction chains
    // The lock is held for the entire handler (select → sign → BEEF → broadcast).
    // ============================================================
    let _create_action_guard = state.create_action_lock.lock().await;
    log::info!("   🔒 createAction serialization lock acquired");

    // ============================================================
    // BRC-100 Basket/Tag Validation (Phase 2 from implementation plan)
    // Validate and normalize all basket names and tags BEFORE processing
    // ============================================================
    use crate::database::basket_repo::validate_and_normalize_basket_name;
    use crate::database::tag_repo::validate_and_normalize_tag;
    use std::collections::HashMap;

    let mut normalized_baskets: HashMap<usize, String> = HashMap::new();
    let mut normalized_tags: HashMap<usize, Vec<String>> = HashMap::new();

    for (i, output) in req.outputs.iter().enumerate() {
        // Validate and normalize basket name if provided
        if let Some(ref basket) = output.basket {
            match validate_and_normalize_basket_name(basket) {
                Ok(normalized) => {
                    log::info!("   Output {}: basket='{}' (normalized from '{}')", i, normalized, basket);
                    normalized_baskets.insert(i, normalized);
                }
                Err(e) => {
                    log::error!("   ❌ Output {}: invalid basket name: {}", i, e);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Output {}: {}", i, e),
                        "code": "ERR_INVALID_BASKET_NAME"
                    }));
                }
            }
        }

        // Validate and normalize tag names if provided
        if let Some(ref tags) = output.tags {
            let mut output_tags = Vec::new();
            for (j, tag) in tags.iter().enumerate() {
                match validate_and_normalize_tag(tag) {
                    Ok(normalized) => {
                        // Deduplicate during normalization
                        if !output_tags.contains(&normalized) {
                            output_tags.push(normalized);
                        }
                    }
                    Err(e) => {
                        log::error!("   ❌ Output {} tag {}: {}", i, j, e);
                        return HttpResponse::BadRequest().json(serde_json::json!({
                            "error": format!("Output {} tag {}: {}", i, j, e),
                            "code": "ERR_INVALID_TAG"
                        }));
                    }
                }
            }
            if !output_tags.is_empty() {
                log::info!("   Output {}: tags={:?}", i, output_tags);
                normalized_tags.insert(i, output_tags);
            }
        }

        // Validate output description length if provided (SDK: 5-50 bytes UTF-8)
        if let Some(ref desc) = output.output_description {
            let byte_len = desc.len();
            if byte_len > 0 && byte_len < 5 {
                log::warn!("   ⚠️  Output {}: outputDescription too short ({} bytes, min 5) - accepting anyway", i, byte_len);
            } else if byte_len > 50 {
                log::error!("   ❌ Output {}: outputDescription too long ({} bytes, max 50)", i, byte_len);
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Output {}: outputDescription exceeds 50 bytes ({} bytes)", i, byte_len),
                    "code": "ERR_INVALID_OUTPUT_DESCRIPTION"
                }));
            }
        }
    }

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
        unlocking_script_length: Option<usize>,
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
                unlocking_script_length: input.unlocking_script_length,
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
            .or(user_input.unlocking_script_length)
            .unwrap_or(107);  // Default to P2PKH unlocking script size
        user_input_script_lengths.push(script_len);
    }

    // Estimate fee based on size:
    // - User-provided inputs + estimated wallet inputs (assume 1-2 for simple tx)
    // - All outputs + potential change output
    let estimated_wallet_inputs = if user_inputs.is_empty() { 2 } else { 1 };
    let total_estimated_inputs = user_inputs.len() + estimated_wallet_inputs;

    // Get dynamic fee rate from ARC policy (cached, 1-hour TTL)
    let fee_rate_sats_per_kb = state.fee_rate_cache.get_rate().await;

    let mut estimated_fee = estimate_fee_for_transaction(
        total_estimated_inputs,
        &output_script_lengths,
        true,  // Include change output
        fee_rate_sats_per_kb
    ) as i64;

    log::info!("   📊 Fee estimation:");
    log::info!("      Estimated inputs: {} (user: {}, wallet: ~{})",
        total_estimated_inputs, user_inputs.len(), estimated_wallet_inputs);
    log::info!("      Output count: {} + 1 change", output_script_lengths.len());
    log::info!("      Estimated fee: {} satoshis ({} sat/KB, {:.1} sat/byte)",
        estimated_fee, fee_rate_sats_per_kb, fee_rate_sats_per_kb as f64 / 1000.0);

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
    let mut reservation_placeholder: Option<String> = None; // Tracks placeholder spent_txid for rollback
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
        let mut address_id_map: std::collections::HashMap<i64, i32> = std::collections::HashMap::new();
        let mut address_ids = Vec::new();

        for addr in &addresses {
            if let Ok(Some(db_addr)) = address_repo.get_by_address(&addr.address) {
                if let Some(addr_id) = db_addr.id {
                    address_ids.push(addr_id);
                    address_id_map.insert(addr_id, addr.index);
                }
            }
        }

        // Get UTXOs from database
        let mut all_utxos = match utxo_repo.get_unspent_by_addresses(&address_ids) {
            Ok(db_utxos) => {
                // Convert database UTXOs to fetcher format
                db_utxos.iter()
                    .filter_map(|db_utxo| {
                        db_utxo.address_id.and_then(|aid| address_id_map.get(&aid))
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
                            .filter(|u| u.address_index == addr.index)
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

            // Note: We don't use api_utxos directly because they don't reflect our local is_spent status.
            // The API UTXOs are now cached in the database - we'll re-read with is_spent filter below.
            log::info!("   ✅ API UTXOs cached to database");
        }

        // NOTE: The outer create_action_lock already serializes concurrent createAction calls.
        // This inner lock is retained as defense-in-depth for UTXO selection specifically,
        // in case other endpoints also select UTXOs in the future.
        let _utxo_lock = state.utxo_selection_lock.lock().await;

        // Re-read UTXOs from database (with correct is_spent status) under the lock
        {
            let db = state.database.lock().unwrap();
            let utxo_repo = crate::database::UtxoRepository::new(db.connection());

            // Re-query to get fresh unspent UTXOs (respects is_spent flag)
            all_utxos = match utxo_repo.get_unspent_by_addresses(&address_ids) {
                Ok(db_utxos) => {
                    db_utxos.iter()
                        .filter_map(|db_utxo| {
                            db_utxo.address_id.and_then(|aid| address_id_map.get(&aid))
                                .map(|&idx| utxo_to_fetcher_utxo(db_utxo, idx))
                        })
                        .collect::<Vec<_>>()
                }
                Err(e) => {
                    log::warn!("   Failed to re-read UTXOs from database: {}", e);
                    Vec::new()
                }
            };
            drop(db);
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

            // Mark selected UTXOs as "in use" immediately (still under the lock)
            if !selected_utxos.is_empty() {
                let db = state.database.lock().unwrap();
                let utxo_repo = crate::database::UtxoRepository::new(db.connection());

                // Mark as spent with a placeholder txid (will be updated with real txid after signing)
                // Using "pending-{timestamp}" to indicate these are reserved but not yet broadcast
                let placeholder_txid = format!("pending-{}", chrono::Utc::now().timestamp_millis());
                let utxos_to_reserve: Vec<(String, u32)> = selected_utxos.iter()
                    .map(|u| (u.txid.clone(), u.vout))
                    .collect();

                match utxo_repo.mark_multiple_spent(&utxos_to_reserve, &placeholder_txid) {
                    Ok(count) => {
                        log::info!("   🔒 Reserved {} UTXOs (preventing concurrent selection)", count);
                        reservation_placeholder = Some(placeholder_txid);
                    }
                    Err(e) => {
                        log::warn!("   ⚠️  Failed to reserve UTXOs: {} (continuing anyway)", e);
                    }
                }
                drop(db);
            }
        }
        // Lock is released here when _utxo_lock goes out of scope
    } else {
        // No wallet UTXOs needed - user inputs cover everything
        addresses = Vec::new();
        log::info!("   ✅ Skipping wallet UTXO fetch - user inputs cover all requirements");
    }

    // Reserve user-provided inputs in local utxos table (optimistic locking).
    // This prevents listOutputs from returning tokens that are already committed
    // to being spent in this transaction. Only affects UTXOs tracked locally
    // (basket outputs); external UTXOs not in our table are silently skipped.
    if !user_inputs.is_empty() {
        // Ensure we have a placeholder — create one if wallet UTXOs weren't needed
        if reservation_placeholder.is_none() {
            reservation_placeholder = Some(format!("pending-{}", chrono::Utc::now().timestamp_millis()));
        }

        let placeholder = reservation_placeholder.as_ref().unwrap();
        let user_outpoints: Vec<(String, u32)> = user_inputs.iter()
            .map(|ui| (ui.txid.clone(), ui.vout))
            .collect();

        let db = state.database.lock().unwrap();
        let utxo_repo = crate::database::UtxoRepository::new(db.connection());

        match utxo_repo.mark_multiple_spent(&user_outpoints, placeholder) {
            Ok(count) => {
                if count > 0 {
                    log::info!("   🔒 Reserved {} user-provided input(s) (basket outputs)", count);
                    state.balance_cache.invalidate();
                }
            }
            Err(e) => {
                log::warn!("   ⚠️  Failed to reserve user-provided inputs: {} (continuing anyway)", e);
            }
        }
        drop(db);
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

    // User inputs - use actual unlocking script size, or SDK-provided length hint, or default P2PKH
    for user_input in &user_inputs {
        let script_len = user_input.unlocking_script.as_ref()
            .map(|s| s.len())
            .or(user_input.unlocking_script_length)
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

    estimated_fee = calculate_fee(estimated_tx_size, fee_rate_sats_per_kb) as i64;

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

    // Track basket outputs for database insertion after signing (BRC-100 basket tracking)
    struct PendingBasketOutput {
        vout: u32,
        satoshis: i64,
        script_hex: String,
        basket_name: String,
        tags: Vec<String>,
        custom_instructions: Option<String>,
        output_description: Option<String>,
    }
    let mut pending_basket_outputs: Vec<PendingBasketOutput> = Vec::new();

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

        // Collect basket output metadata for database insertion (BRC-100)
        if let Some(basket_name) = normalized_baskets.get(&i) {
            pending_basket_outputs.push(PendingBasketOutput {
                vout: i as u32,
                satoshis,
                script_hex: hex::encode(&script_bytes),
                basket_name: basket_name.clone(),
                tags: normalized_tags.get(&i).cloned().unwrap_or_default(),
                custom_instructions: output.custom_instructions.clone(),
                output_description: output.output_description.clone(),
            });
            log::info!("   Output {}: tracked for basket '{}'", i, basket_name);
        }

        tx.add_output(TxOutput::new(satoshis, script_bytes));
    }

    // Calculate change
    let change = total_input - total_output - estimated_fee;
    log::info!("   Change: {} satoshis", change);

    // Track change output info for immediate UTXO insertion (balance accuracy fix)
    let mut pending_change_utxo: Option<(i64, i32, i64, String)> = None; // (address_id, address_index, satoshis, script_hex)

    if change > 546 { // Dust limit
        // Get first address for change
        // Generate NEW change address (privacy: don't reuse addresses)
        use crate::database::{WalletRepository, AddressRepository, get_master_private_key_from_db, get_master_public_key_from_db};
        // derive_child_public_key is already imported at top of file from crate::crypto::brc42
        use std::time::{SystemTime, UNIX_EPOCH};

        let db = state.database.lock().unwrap();
        let wallet_repo = WalletRepository::new(db.connection());
        let address_repo = AddressRepository::new(db.connection());

        let wallet = match wallet_repo.get_primary_wallet() {
            Ok(Some(w)) => w,
            Ok(None) | Err(_) => {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "No wallet found"
                }));
            }
        };

        let wallet_id = wallet.id.unwrap();

        // Use MAX(index) from database instead of wallet.current_index
        // This is more reliable - current_index can get out of sync
        let current_index = match address_repo.get_max_index(wallet_id) {
            Ok(Some(max_idx)) => max_idx + 1,  // Next index is max + 1
            Ok(None) => 0,  // No addresses yet, start at 0
            Err(e) => {
                log::warn!("   Failed to get max address index: {}, falling back to wallet.current_index", e);
                wallet.current_index
            }
        };
        log::info!("   Next change address index: {} (from MAX query)", current_index);

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
        // (address_repo already created above for MAX query)
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

        let change_address_id = match address_repo.create(&address_model) {
            Ok(addr_id) => {
                // Update wallet's current_index
                if let Err(e) = wallet_repo.update_current_index(wallet_id, current_index + 1) {
                    log::warn!("   Failed to update wallet index: {}", e);
                }
                log::info!("   ✅ Generated new change address: {} (index {}, id {})", change_address, current_index, addr_id);
                Some(addr_id)
            }
            Err(e) => {
                // Address creation failed - this should be rare since we use MAX(index) query
                // But keep fallback logic for edge cases (concurrent requests, etc.)
                log::warn!("   Failed to create change address: {} - checking if it already exists", e);

                // Try to look up the existing address
                match address_repo.get_by_address(&change_address) {
                    Ok(Some(existing_addr)) => {
                        if let Some(addr_id) = existing_addr.id {
                            log::info!("   ✅ Using existing address: {} (index {}, id {})", change_address, existing_addr.index, addr_id);

                            // Fix the current_index to prevent this from happening again
                            // Set it to one past this address's index
                            let correct_index = existing_addr.index + 1;
                            if correct_index > current_index {
                                if let Err(fix_err) = wallet_repo.update_current_index(wallet_id, correct_index) {
                                    log::warn!("   Failed to fix wallet current_index: {}", fix_err);
                                } else {
                                    log::info!("   🔧 Fixed wallet current_index: {} → {}", current_index, correct_index);
                                }
                            }

                            Some(addr_id)
                        } else {
                            log::warn!("   Existing address has no ID, cannot track change UTXO");
                            None
                        }
                    }
                    Ok(None) => {
                        // Address string not found, but index might exist with different address
                        // This could happen if derivation changed - try to find highest index and fix
                        log::warn!("   Address not found by string, index {} may have different address", current_index);

                        // Try to get the address at the conflicting index
                        match address_repo.get_by_wallet_and_index(wallet_id, current_index) {
                            Ok(Some(addr_at_index)) => {
                                log::warn!("   Index {} has address: {} (expected: {})",
                                          current_index, addr_at_index.address, change_address);
                                // Fix current_index to move past this
                                let correct_index = current_index + 1;
                                if let Err(fix_err) = wallet_repo.update_current_index(wallet_id, correct_index) {
                                    log::warn!("   Failed to fix wallet current_index: {}", fix_err);
                                } else {
                                    log::info!("   🔧 Fixed wallet current_index: {} → {}", current_index, correct_index);
                                }
                            }
                            _ => {}
                        }
                        None
                    }
                    Err(lookup_err) => {
                        log::warn!("   Failed to look up existing address: {}", lookup_err);
                        None
                    }
                }
            }
        };

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

        tx.add_output(TxOutput::new(change, change_script.bytes.clone()));
        log::info!("   Added change output: {} satoshis", change);

        // Store change UTXO info for later insertion (after txid is calculated)
        if let Some(addr_id) = change_address_id {
            pending_change_utxo = Some((addr_id, current_index, change, hex::encode(&change_script.bytes)));
            log::info!("   📝 Pending change UTXO tracked for address_id={}", addr_id);
        }
    } else if change > 0 {
        log::info!("   Change below dust limit ({}), adding to fee", change);
    }

    // ═══════════════════════════════════════════════════════════════
    // TRANSACTION FEE SUMMARY - Detailed breakdown for fee analysis
    // ═══════════════════════════════════════════════════════════════
    {
        let actual_fee = if change > 546 { estimated_fee } else { total_input - total_output };
        let change_returned = if change > 546 { change } else { 0 };
        let num_inputs = user_inputs.len() + selected_utxos.len();
        let num_requested_outputs = req.outputs.len();
        let num_total_outputs = num_requested_outputs + if change > 546 { 1 } else { 0 };

        log::info!("");
        log::info!("   ╔═══════════════════════════════════════════════════════════╗");
        log::info!("   ║              TRANSACTION FEE SUMMARY                     ║");
        log::info!("   ╠═══════════════════════════════════════════════════════════╣");
        log::info!("   ║  ARC Fee Rate:    {} sat/KB ({:.2} sat/byte)", fee_rate_sats_per_kb, fee_rate_sats_per_kb as f64 / 1000.0);
        log::info!("   ║  Min Fee Floor:   {} sats", MIN_FEE_SATS);
        log::info!("   ║  Est. Tx Size:    {} bytes", estimated_tx_size);
        log::info!("   ║");
        log::info!("   ║  INPUTS ({}):", num_inputs);
        if !user_inputs.is_empty() {
            log::info!("   ║    User inputs:   {} ({} sats)", user_inputs.len(), user_input_total);
        }
        if !selected_utxos.is_empty() {
            log::info!("   ║    Wallet inputs: {} ({} sats)", selected_utxos.len(), wallet_input_total);
            for utxo in &selected_utxos {
                log::info!("   ║      {}:{} = {} sats", &utxo.txid[..16], utxo.vout, utxo.satoshis);
            }
        }
        log::info!("   ║    Total In:      {} sats", total_input);
        log::info!("   ║");
        log::info!("   ║  OUTPUTS ({}):", num_total_outputs);
        for (i, output) in req.outputs.iter().enumerate() {
            let sats = output.satoshis.unwrap_or(0);
            let label = if output.output_description.is_some() {
                format!(" ({})", output.output_description.as_ref().unwrap())
            } else {
                String::new()
            };
            log::info!("   ║    Output {}:      {} sats{}", i, sats, label);
        }
        if change > 546 {
            log::info!("   ║    Change:        {} sats (returned to wallet)", change_returned);
        }
        log::info!("   ║    Total Out:     {} sats (excl. fee)", total_output + change_returned);
        log::info!("   ║");
        log::info!("   ║  >>> FEE PAID TO MINERS: {} sats <<<", actual_fee);
        if change > 0 && change <= 546 {
            log::info!("   ║    (includes {} sats dust absorbed into fee)", change);
        }
        log::info!("   ║  Net cost to wallet: {} sats (outputs to others) + {} sats (fee) = {} sats",
            total_output, actual_fee, total_output + actual_fee);
        log::info!("   ╚═══════════════════════════════════════════════════════════╝");
        log::info!("");
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
            reservation_placeholder: reservation_placeholder.clone(),
        });
    }

    // Log if this is a BRC-29 payment
    if brc29_info.is_some() {
        log::info!("   💰 BRC-29 payment metadata stored for later envelope conversion");
    }

    log::info!("   ✅ Transaction created: {}", txid);
    log::info!("   Reference: {}", reference);

    // Insert pending change UTXO into database (balance accuracy fix)
    // This ensures the change output is immediately reflected in balance calculations
    // BRC-100: Change outputs go to the "default" basket
    if let Some((addr_id, _addr_index, satoshis, ref script_hex)) = pending_change_utxo {
        let change_vout = req.outputs.len() as u32; // Change is always the last output
        log::info!("   💾 Inserting pending change UTXO: txid={}, vout={}, satoshis={}", txid, change_vout, satoshis);

        let db = state.database.lock().unwrap();
        let utxo_repo = crate::database::UtxoRepository::new(db.connection());
        let basket_repo = crate::database::BasketRepository::new(db.connection());

        // Get or create the "default" basket for change outputs (BRC-99)
        let default_basket_id = match basket_repo.find_or_insert("default") {
            Ok(id) => Some(id),
            Err(e) => {
                log::warn!("   ⚠️  Failed to get 'default' basket: {}", e);
                None
            }
        };

        // Insert change UTXO with default basket and 'completed' status
        // (change outputs are part of our own transaction, so they're immediately valid)
        match utxo_repo.insert_output_with_basket(
            Some(addr_id),
            &txid,
            change_vout,
            satoshis,
            &script_hex,
            default_basket_id,
            None,  // No custom instructions for change
            "completed",  // Status - change outputs are immediately valid
            None,  // No output description for change
        ) {
            Ok(_utxo_id) => {
                log::info!("   ✅ Change UTXO inserted with 'default' basket - balance will be accurate immediately");
            }
            Err(e) => {
                log::warn!("   ⚠️  Failed to insert change UTXO: {} (balance may be temporarily inaccurate)", e);
            }
        }
        drop(db);
    }

    // ═══════════════════════════════════════════════════════════════
    // BRC-100 BASKET OUTPUT TRACKING
    // Insert outputs that have a basket property into the database.
    // These are tracked as wallet-owned UTXOs queryable via listOutputs.
    // Uses pre-signing txid (reconciled after signing in Step 7).
    // ═══════════════════════════════════════════════════════════════
    if !pending_basket_outputs.is_empty() {
        log::info!("   💾 Inserting {} basket output(s)...", pending_basket_outputs.len());
        let db = state.database.lock().unwrap();
        let utxo_repo = crate::database::UtxoRepository::new(db.connection());
        let basket_repo = crate::database::BasketRepository::new(db.connection());
        let tag_repo = crate::database::TagRepository::new(db.connection());

        for bo in &pending_basket_outputs {
            // Resolve basket ID (find existing or create new)
            let basket_id = match basket_repo.find_or_insert(&bo.basket_name) {
                Ok(id) => id,
                Err(e) => {
                    log::warn!("   ⚠️  Failed to resolve basket '{}': {}", bo.basket_name, e);
                    continue;
                }
            };

            // Insert UTXO with basket (address_id = None for basket outputs)
            match utxo_repo.insert_output_with_basket(
                None,       // No address_id — basket outputs may not map to an HD wallet address
                &txid,      // Pre-signing txid (will be reconciled after signing)
                bo.vout,
                bo.satoshis,
                &bo.script_hex,
                Some(basket_id),
                bo.custom_instructions.as_deref(),
                "completed",
                bo.output_description.as_deref(),
            ) {
                Ok(utxo_id) => {
                    log::info!("   ✅ Basket output vout={} → basket='{}' (utxo_id={})",
                        bo.vout, bo.basket_name, utxo_id);

                    // Assign tags to the output
                    for tag in &bo.tags {
                        match tag_repo.assign_tag_to_output(utxo_id, tag) {
                            Ok(_) => {
                                log::info!("      Tagged with '{}'", tag);
                            }
                            Err(e) => {
                                log::warn!("      ⚠️  Failed to assign tag '{}': {}", tag, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("   ⚠️  Failed to insert basket output vout={}: {}", bo.vout, e);
                }
            }
        }
        drop(db);
    }

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
    let send_with_txids: Vec<String> = options
        .and_then(|o| o.send_with.clone())
        .unwrap_or_default();
    let is_send_with = !send_with_txids.is_empty();

    log::info!("   Options: signAndProcess={}, acceptDelayedBroadcast={}, noSend={}, sendWith={:?}",
               sign_and_process, accept_delayed_broadcast, no_send, send_with_txids);

    // Determine if we should sign and/or broadcast
    // Per BRC-100 spec: sendWith trumps noSend (isSendWith forces broadcast of batched txids)
    let should_sign = sign_and_process;
    let should_broadcast = (!accept_delayed_broadcast && !no_send) || is_send_with;

    // Save the pre-signing txid for post-signing reconciliation
    // (BSV txids include unlocking scripts, so signing changes the txid)
    let pre_signing_txid = txid.clone();

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
                                // Check if signAction reports unsigned inputs → two-phase flow
                                if let Some(unsigned) = json_resp["unsignedInputs"].as_array() {
                                    if !unsigned.is_empty() {
                                        log::info!("   ℹ️  signAction reports {} unsigned input(s) — two-phase flow",
                                            unsigned.len());

                                        // Extract the AtomicBEEF bytes from signAction's response.
                                        // The BEEF contains the partially-signed tx (wallet inputs signed,
                                        // PushDrop inputs empty) plus all parent transactions.
                                        // The SDK uses this to compute sighash preimages for unsigned inputs.
                                        let beef_bytes = if let Some(raw_tx_hex) = json_resp["rawTx"].as_str() {
                                            match hex::decode(raw_tx_hex) {
                                                Ok(bytes) => bytes,
                                                Err(e) => {
                                                    log::error!("   Failed to decode BEEF hex: {}", e);
                                                    return HttpResponse::InternalServerError().json(serde_json::json!({
                                                        "error": format!("Failed to decode BEEF for signableTransaction: {}", e)
                                                    }));
                                                }
                                            }
                                        } else {
                                            log::error!("   signAction returned unsigned inputs but no rawTx");
                                            return HttpResponse::InternalServerError().json(serde_json::json!({
                                                "error": "signAction returned unsigned inputs but no BEEF data"
                                            }));
                                        };

                                        log::info!("   ➡️  Returning signableTransaction ({} BEEF bytes, reference: {})",
                                            beef_bytes.len(), reference);

                                        // Return two-phase response per SDK spec:
                                        // signableTransaction: { tx: AtomicBEEF bytes, reference: string }
                                        return HttpResponse::Ok().json(serde_json::json!({
                                            "signableTransaction": {
                                                "tx": beef_bytes,
                                                "reference": reference,
                                            }
                                        }));
                                    }
                                }

                                let txid = json_resp["txid"].as_str().unwrap_or("").to_string();
                                log::info!("   ✅ Transaction signed successfully");
                                log::info!("   📝 Signed TXID: {}", txid);

                                // Extract rawTx (Atomic BEEF hex string) and convert to bytes
                                let tx_data = if let Some(raw_tx_hex) = json_resp["rawTx"].as_str() {
                                    log::info!("   📦 Extracting Atomic BEEF response");

                                    // Extract the signed raw transaction from the BEEF and store it.
                                    // This is critical for BEEF ancestry: if a subsequent createAction
                                    // spends outputs from THIS transaction, the BEEF builder needs the
                                    // SIGNED raw tx (not the unsigned version stored at creation time).
                                    match crate::beef::Beef::extract_raw_tx_hex(raw_tx_hex) {
                                        Ok(signed_raw_tx_hex) => {
                                            log::info!("   💾 Storing signed raw tx for BEEF ancestry ({} hex chars)", signed_raw_tx_hex.len());

                                            // Update transactions table with signed raw_tx
                                            {
                                                let db = state.database.lock().unwrap();
                                                let tx_repo = crate::database::TransactionRepository::new(db.connection());
                                                if let Err(e) = tx_repo.update_raw_tx(&txid, &signed_raw_tx_hex) {
                                                    log::warn!("   ⚠️  Failed to update signed raw_tx in transactions: {}", e);
                                                }

                                                // Also store in parent_transactions cache for fast BEEF lookup
                                                let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());
                                                if let Err(e) = parent_tx_repo.upsert(None, &txid, &signed_raw_tx_hex) {
                                                    log::warn!("   ⚠️  Failed to cache signed tx in parent_transactions: {}", e);
                                                } else {
                                                    log::info!("   ✅ Cached signed tx {} in parent_transactions for BEEF ancestry", &txid[..16]);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::warn!("   ⚠️  Failed to extract signed raw tx from BEEF: {}", e);
                                        }
                                    }

                                    hex::decode(raw_tx_hex).ok()
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

    // ═══════════════════════════════════════════════════════════════
    // POST-SIGNING TXID RECONCILIATION
    // BSV txids include unlocking scripts, so signing changes the txid.
    // Update all DB records from pre-signing txid to signed txid.
    // ═══════════════════════════════════════════════════════════════
    if final_txid != pre_signing_txid && !final_txid.is_empty() {
        log::info!("   🔄 Txid changed after signing: {} → {}", &pre_signing_txid[..16], &final_txid[..16]);

        let db = state.database.lock().unwrap();

        // 1. Update transaction record txid
        {
            use crate::database::TransactionRepository;
            let tx_repo = TransactionRepository::new(db.connection());
            if let Err(e) = tx_repo.rename_txid(&pre_signing_txid, &final_txid) {
                log::warn!("   ⚠️  Failed to update transaction txid: {}", e);
            }
        }

        // 2. Update change UTXO txid (change is always the last output)
        if pending_change_utxo.is_some() {
            let change_vout = req.outputs.len() as u32;
            let utxo_repo = crate::database::UtxoRepository::new(db.connection());
            if let Err(e) = utxo_repo.update_utxo_txid(&pre_signing_txid, change_vout, &final_txid) {
                log::warn!("   ⚠️  Failed to update change UTXO txid: {}", e);
            }
        }

        // 2b. Update basket output txids (pre-signing → signed)
        if !pending_basket_outputs.is_empty() {
            let utxo_repo = crate::database::UtxoRepository::new(db.connection());
            for bo in &pending_basket_outputs {
                if let Err(e) = utxo_repo.update_utxo_txid(&pre_signing_txid, bo.vout, &final_txid) {
                    log::warn!("   ⚠️  Failed to update basket output vout={} txid: {}", bo.vout, e);
                }
            }
            log::info!("   ✅ Updated {} basket output txid(s)", pending_basket_outputs.len());
        }

        // 3. Update spent_txid on reserved inputs (placeholder → signed txid)
        if let Some(ref placeholder) = reservation_placeholder {
            let utxo_repo = crate::database::UtxoRepository::new(db.connection());
            if let Err(e) = utxo_repo.update_spent_txid_batch(placeholder, &final_txid) {
                log::warn!("   ⚠️  Failed to update spent_txid on inputs: {}", e);
            }
        }

        drop(db);
    } else if !final_txid.is_empty() {
        // Txid didn't change (unsigned or same) — still update spent_txid from placeholder
        if let Some(ref placeholder) = reservation_placeholder {
            let db = state.database.lock().unwrap();
            let utxo_repo = crate::database::UtxoRepository::new(db.connection());
            if let Err(e) = utxo_repo.update_spent_txid_batch(placeholder, &final_txid) {
                log::warn!("   ⚠️  Failed to update spent_txid on inputs: {}", e);
            }
            drop(db);
        }
    }

    // Track broadcast result for response
    let mut send_with_results: Option<Vec<SendWithResult>> = None;

    if should_broadcast {
        if let Some(ref tx_bytes) = raw_tx {
            let beef_hex = hex::encode(tx_bytes);
            log::info!("   📡 Broadcasting transaction to network...");

            match broadcast_transaction(&beef_hex, Some(&state.database), Some(&final_txid)).await {
                Ok(_broadcast_msg) => {
                    log::info!("   ✅ Broadcast successful: {}", _broadcast_msg);

                    // Update broadcast_status to 'broadcast' in database
                    {
                        use crate::database::TransactionRepository;
                        let db = state.database.lock().unwrap();
                        let tx_repo = TransactionRepository::new(db.connection());
                        if let Err(e) = tx_repo.update_broadcast_status(&final_txid, "broadcast") {
                            log::warn!("   ⚠️  Failed to update broadcast_status: {}", e);
                        }
                    }

                    // Note: Do NOT populate send_with_results here.
                    // sendWithResults is only for transactions listed in sendWith[],
                    // not for the current transaction's broadcast result.
                }
                Err(e) => {
                    log::error!("   ❌ Broadcast failed: {}", e);

                    // Update broadcast_status to 'failed' in database
                    {
                        use crate::database::TransactionRepository;
                        use crate::database::UtxoRepository;
                        let db = state.database.lock().unwrap();
                        let tx_repo = TransactionRepository::new(db.connection());
                        if let Err(e) = tx_repo.update_broadcast_status(&final_txid, "failed") {
                            log::warn!("   ⚠️  Failed to update broadcast_status: {}", e);
                        }

                        // CRITICAL: Remove ghost change UTXO since broadcast failed.
                        // Try signed txid first, then pre-signing txid as fallback
                        // (signAction may not have updated the UTXO txid if it failed)
                        let utxo_repo = UtxoRepository::new(db.connection());
                        let mut deleted_count = 0usize;
                        match utxo_repo.delete_by_txid(&final_txid) {
                            Ok(count) => { deleted_count += count; }
                            Err(e) => {
                                log::warn!("   ⚠️  Failed to remove ghost UTXO by signed txid: {}", e);
                            }
                        }
                        // Fallback: also try pre-signing txid in case signAction didn't update it
                        if final_txid != pre_signing_txid {
                            match utxo_repo.delete_by_txid(&pre_signing_txid) {
                                Ok(count) => { deleted_count += count; }
                                Err(e) => {
                                    log::warn!("   ⚠️  Failed to remove ghost UTXO by pre-signing txid: {}", e);
                                }
                            }
                        }
                        if deleted_count > 0 {
                            log::info!("   🗑️  Removed {} ghost change UTXO(s) from failed broadcast", deleted_count);
                        }

                        // CRITICAL: Restore input UTXOs that were reserved for this transaction.
                        // Since broadcast failed, these coins were never spent on-chain.
                        match utxo_repo.restore_spent_by_txid(&final_txid) {
                            Ok(count) if count > 0 => {
                                log::info!("   ♻️  Restored {} input UTXO(s) from failed broadcast", count);
                            }
                            Ok(_) => {
                                // spent_txid might still be placeholder if signing failed to update
                                if let Some(ref placeholder) = reservation_placeholder {
                                    let _ = utxo_repo.restore_spent_by_txid(placeholder);
                                }
                            }
                            Err(e) => {
                                log::warn!("   ⚠️  Failed to restore input UTXOs: {}", e);
                            }
                        }

                        // Invalidate balance cache since we restored UTXOs
                        state.balance_cache.invalidate();
                    }

                    // Note: Do NOT populate send_with_results here.
                    // sendWithResults is only for transactions listed in sendWith[].
                    // Don't return error - still return the signed tx
                    // The app might want to retry broadcasting
                }
            }
        } else {
            log::warn!("   ⚠️  No transaction bytes available for broadcast");
        }
    } else {
        log::info!("   ℹ️  Skipping broadcast (acceptDelayedBroadcast={}, noSend={})",
                   accept_delayed_broadcast, no_send);
    }

    // ═══════════════════════════════════════════════════════════════
    // SEND_WITH: Broadcast previously-created noSend transactions
    // Per BRC-100 spec, sendWith contains TXIDs of transactions that were
    // created with noSend=true and should now be broadcast alongside this tx.
    // ═══════════════════════════════════════════════════════════════
    if is_send_with {
        log::info!("   📡 Processing sendWith: {} transaction(s) to broadcast", send_with_txids.len());

        // Initialize sendWithResults if not already set by the main broadcast
        let results = send_with_results.get_or_insert_with(Vec::new);

        for sw_txid in &send_with_txids {
            log::info!("   📡 Broadcasting sendWith txid: {}", sw_txid);

            // Look up the signed raw tx from parent_transactions or transactions table
            let sw_beef_hex = {
                let db = state.database.lock().unwrap();

                // First check parent_transactions (has signed raw tx if we stored it)
                let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());
                match parent_tx_repo.get_by_txid(sw_txid) {
                    Ok(Some(cached)) if !cached.raw_hex.is_empty() => {
                        log::info!("   ✅ Found sendWith tx {} in parent_transactions cache", &sw_txid[..16]);
                        // Build a minimal BEEF V1 for broadcasting: just the raw tx
                        // ARC accepts raw tx hex directly, so we can broadcast as-is
                        Some(cached.raw_hex.clone())
                    }
                    _ => {
                        // Fallback: check transactions table
                        let tx_repo = crate::database::TransactionRepository::new(db.connection());
                        match tx_repo.get_local_parent_tx(sw_txid) {
                            Ok(Some(raw_tx_hex)) => {
                                log::info!("   📋 Found sendWith tx {} in transactions table", &sw_txid[..16]);
                                Some(raw_tx_hex)
                            }
                            _ => {
                                log::warn!("   ⚠️  sendWith txid {} not found in local storage", sw_txid);
                                None
                            }
                        }
                    }
                }
            };

            match sw_beef_hex {
                Some(raw_tx_hex) => {
                    // Broadcast the raw transaction
                    match broadcast_transaction(&raw_tx_hex, Some(&state.database), Some(sw_txid)).await {
                        Ok(msg) => {
                            log::info!("   ✅ sendWith broadcast success for {}: {}", &sw_txid[..16], msg);
                            // Update broadcast status
                            {
                                let db = state.database.lock().unwrap();
                                let tx_repo = crate::database::TransactionRepository::new(db.connection());
                                let _ = tx_repo.update_broadcast_status(sw_txid, "broadcast");
                            }
                            results.push(SendWithResult {
                                txid: sw_txid.clone(),
                                status: "unproven".to_string(),
                            });
                        }
                        Err(e) => {
                            log::warn!("   ⚠️  sendWith broadcast failed for {}: {}", &sw_txid[..16], e);
                            results.push(SendWithResult {
                                txid: sw_txid.clone(),
                                status: "failed".to_string(),
                            });
                        }
                    }
                }
                None => {
                    results.push(SendWithResult {
                        txid: sw_txid.clone(),
                        status: "failed".to_string(),
                    });
                }
            }
        }
        log::info!("   📡 sendWith processing complete: {} result(s)", results.len());
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

    // Log complete response details before returning
    log::info!("═══════════════════════════════════════════════════════");
    log::info!("📤 createAction RESPONSE:");
    log::info!("   txid: {:?}", Some(&final_txid));
    log::info!("   tx (Atomic BEEF): {}", match &raw_tx {
        Some(bytes) => format!("present ({} bytes)", bytes.len()),
        None => "ABSENT (None)".to_string(),
    });
    log::info!("   sendWithResults: {:?}", send_with_results);
    log::info!("   outputs: {} entries", response_outputs.len());
    log::info!("   inputs: {} entries", response_inputs.len());
    // Build noSendChange outpoints for noSend transactions.
    // The SDK uses these to chain transactions: the change outpoints from this noSend tx
    // become inputs (via options.noSendChange) in the next createAction call.
    let no_send_change = if no_send && pending_change_utxo.is_some() && !final_txid.is_empty() {
        let change_vout = req.outputs.len(); // Change is appended after user outputs
        let outpoint = format!("{}.{}", final_txid, change_vout);
        log::info!("   📋 noSendChange: {}", outpoint);
        Some(vec![outpoint])
    } else {
        None
    };

    // Log the actual JSON that will be sent to the caller
    let response = CreateActionResponse {
        reference,
        version: tx.version,
        lock_time: tx.lock_time,
        inputs: response_inputs,
        outputs: response_outputs,
        derivation_prefix: None,
        input_beef: None,
        txid: Some(final_txid),
        tx: raw_tx,
        send_with_results,
        no_send_change,
        signable_transaction: None,
    };
    match serde_json::to_string(&response) {
        Ok(json_str) => {
            if json_str.len() <= 4000 {
                log::info!("📤 FULL RESPONSE JSON: {}", json_str);
            } else {
                log::info!("📤 RESPONSE JSON (first 4000 of {} chars): {}", json_str.len(), &json_str[..4000]);
            }
        }
        Err(e) => log::warn!("   Failed to serialize response for logging: {}", e),
    }
    log::info!("═══════════════════════════════════════════════════════");
    HttpResponse::Ok().json(response)
}

// Query confirmation status - tries ARC first, falls back to WhatsOnChain
async fn get_confirmation_status(txid: &str) -> Result<(u32, Option<u32>), String> {
    let client = reqwest::Client::new();

    // Try ARC first
    match query_arc_tx_status(&client, txid).await {
        Ok(arc_resp) => {
            let status = arc_resp.tx_status.as_deref().unwrap_or("UNKNOWN");
            if status == "MINED" {
                // ARC reports MINED - return at least 1 confirmation and block height
                let block_height = arc_resp.block_height.map(|h| h as u32);
                // ARC doesn't report exact confirmation count, but MINED means >= 1
                return Ok((1, block_height));
            }
            // Not yet mined - ARC knows about it but no confirmations
            return Ok((0, None));
        }
        Err(e) => {
            log::warn!("   ⚠️  ARC confirmation check failed for {}: {}, falling back to WhatsOnChain", txid, e);
        }
    }

    // Fall back to WhatsOnChain
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/hash/{}", txid);

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

/// Check if a transaction exists on-chain - tries ARC first, falls back to WhatsOnChain
///
/// Returns Ok(true) if tx exists (even with 0 confirmations - in mempool)
/// Returns Ok(false) if tx doesn't exist (404)
/// Returns Err if both APIs fail
async fn check_tx_exists_on_chain(txid: &str) -> Result<bool, String> {
    log::info!("   🔍 Checking if transaction exists on-chain: {}", txid);

    let client = reqwest::Client::new();

    // Try ARC first
    match query_arc_tx_status(&client, txid).await {
        Ok(arc_resp) => {
            let status = arc_resp.tx_status.as_deref().unwrap_or("UNKNOWN");
            match status {
                "MINED" | "SEEN_ON_NETWORK" | "SEEN_IN_ORPHAN_MEMPOOL"
                | "ANNOUNCED_TO_NETWORK" | "REQUESTED_BY_NETWORK"
                | "SENT_TO_NETWORK" | "ACCEPTED_BY_NETWORK" | "STORED" => {
                    log::info!("   ✅ Transaction exists (ARC status: {})", status);
                    return Ok(true);
                }
                _ => {
                    log::info!("   ⚠️  ARC returned status: {} - checking WhatsOnChain", status);
                }
            }
        }
        Err(e) => {
            // ARC 404 means tx not known to ARC - fall through to WhatsOnChain
            if e.contains("404") {
                log::info!("   ℹ️  Transaction not known to ARC, checking WhatsOnChain...");
            } else {
                log::warn!("   ⚠️  ARC check failed: {}, falling back to WhatsOnChain", e);
            }
        }
    }

    // Fall back to WhatsOnChain
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/hash/{}", txid);

    let response = client.get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP error checking tx: {}", e))?;

    let status = response.status();

    if status.is_success() {
        log::info!("   ✅ Transaction exists on-chain");
        Ok(true)
    } else if status.as_u16() == 404 {
        log::info!("   ⚠️  Transaction NOT found on-chain (404)");
        Ok(false)
    } else {
        Err(format!("API returned status: {}", status))
    }
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

/// Derive address from BRC-29 payment remittance using BRC-42
///
/// This derives the child private key that corresponds to where the sender
/// sent the payment. Uses BRC-42 with:
/// - Our master private key as recipient
/// - Sender's identity key as counterparty
/// - BRC-29 invoice number format: "2-3241645161d8-{derivationPrefix} {derivationSuffix}"
///   where "3241645161d8" is the BRC-29 payment protocol ID and 2 is security level
///
/// Reference: BRC-29 spec https://brc.dev/29
/// Reference: TypeScript SDK ScriptTemplateBRC29.ts
///
/// Returns (pubkey_hash, pubkey_bytes) for verification and spending
fn derive_address_from_payment_remittance(
    our_master_privkey: &[u8],
    remittance: &PaymentRemittance,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    use crate::crypto::brc42::derive_child_private_key;
    use secp256k1::{Secp256k1, SecretKey, PublicKey};
    use sha2::{Sha256, Digest};
    use ripemd::Ripemd160;

    // Parse sender's public key from hex
    let sender_pubkey_bytes = hex::decode(&remittance.sender_identity_key)
        .map_err(|e| format!("Invalid sender identity key hex: {}", e))?;

    if sender_pubkey_bytes.len() != 33 {
        return Err(format!(
            "Invalid sender identity key length: expected 33, got {}",
            sender_pubkey_bytes.len()
        ));
    }

    // BRC-29 invoice number format: "2-3241645161d8-{derivationPrefix} {derivationSuffix}"
    // - Security level 2 (counterparty-specific permission)
    // - Protocol ID "3241645161d8" (BRC-29 payment protocol magic number)
    // - Key ID is "{derivationPrefix} {derivationSuffix}" (space-separated)
    let key_id = format!("{} {}", remittance.derivation_prefix, remittance.derivation_suffix);
    let invoice_number = format!("2-3241645161d8-{}", key_id);
    log::info!("      BRC-29 invoice number: {}", invoice_number);

    // Derive our child private key using BRC-42
    // Recipient derives: derive_child_private_key(our_privkey, sender_pubkey, invoice)
    let child_privkey = derive_child_private_key(our_master_privkey, &sender_pubkey_bytes, &invoice_number)
        .map_err(|e| format!("BRC-42 derivation failed: {}", e))?;

    // Convert child private key to public key
    let secp = Secp256k1::new();
    let secret = SecretKey::from_slice(&child_privkey)
        .map_err(|e| format!("Invalid derived private key: {}", e))?;
    let pubkey = PublicKey::from_secret_key(&secp, &secret);
    let pubkey_bytes = pubkey.serialize().to_vec();

    // Calculate pubkey hash: RIPEMD160(SHA256(pubkey))
    let sha_hash = Sha256::digest(&pubkey_bytes);
    let pubkey_hash = Ripemd160::digest(&sha_hash).to_vec();

    Ok((pubkey_hash, pubkey_bytes))
}

/// Verify that a P2PKH output script matches a derived pubkey hash
fn verify_output_matches_derived_address(script_bytes: &[u8], expected_pubkey_hash: &[u8]) -> bool {
    // P2PKH script: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
    if script_bytes.len() == 25 &&
       script_bytes[0] == 0x76 &&  // OP_DUP
       script_bytes[1] == 0xa9 &&  // OP_HASH160
       script_bytes[2] == 0x14 &&  // Push 20 bytes
       script_bytes[23] == 0x88 && // OP_EQUALVERIFY
       script_bytes[24] == 0xac {  // OP_CHECKSIG
        let script_pubkey_hash = &script_bytes[3..23];
        return script_pubkey_hash == expected_pubkey_hash;
    }

    false
}

/// Store a BRC-42 derived UTXO in the database
///
/// For BRC-29 payment UTXOs, we store:
/// - The actual Bitcoin address (derived from the child public key)
/// - The actual derived public key (hex)
/// - A negative index to distinguish from HD wallet addresses
/// - Derivation info in custom_instructions for spending later
///
/// Index scheme:
/// - Positive indices (0, 1, 2, ...): HD wallet addresses (m/{index})
/// - Index -1: Master public key address
/// - Negative indices (-2, -3, ...): BRC-42 derived addresses (payments, etc.)
fn store_derived_utxo(
    db: &crate::database::WalletDatabase,
    txid: &str,
    vout: u32,
    satoshis: i64,
    script_hex: &str,
    derived_pubkey: &[u8],
    custom_instructions: &serde_json::Value,
) -> Result<(), String> {
    use crate::database::WalletRepository;
    use std::time::{SystemTime, UNIX_EPOCH};
    use sha2::{Sha256, Digest};
    use ripemd::Ripemd160;

    let conn = db.connection();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Get wallet ID
    let wallet_repo = WalletRepository::new(conn);
    let wallet = wallet_repo.get_primary_wallet()
        .map_err(|e| format!("Failed to get wallet: {}", e))?
        .ok_or("No wallet found")?;
    let wallet_id = wallet.id.ok_or("Wallet has no ID")?;

    // Calculate pubkey hash: RIPEMD160(SHA256(pubkey))
    let sha_hash = Sha256::digest(derived_pubkey);
    let pubkey_hash = Ripemd160::digest(&sha_hash);

    // Convert pubkey hash to Bitcoin address (mainnet P2PKH: version byte 0x00)
    let mut address_bytes = vec![0x00]; // Mainnet version byte
    address_bytes.extend_from_slice(&pubkey_hash);

    // Double SHA256 for checksum
    let checksum_hash1 = Sha256::digest(&address_bytes);
    let checksum_hash2 = Sha256::digest(&checksum_hash1);
    address_bytes.extend_from_slice(&checksum_hash2[0..4]);

    // Base58 encode
    let bitcoin_address = bs58::encode(&address_bytes).into_string();
    let pubkey_hex = hex::encode(derived_pubkey);

    log::info!("      Derived Bitcoin address: {}", bitcoin_address);
    log::info!("      Derived public key: {}", pubkey_hex);

    // Check if this address already exists (by address string)
    let existing_address_id: Option<i64> = conn.query_row(
        "SELECT id FROM addresses WHERE wallet_id = ?1 AND address = ?2",
        rusqlite::params![wallet_id, bitcoin_address],
        |row| row.get(0),
    ).ok();

    let address_id = if let Some(id) = existing_address_id {
        log::info!("      Address already exists (ID: {}), updating balance", id);
        // Update balance
        conn.execute(
            "UPDATE addresses SET balance = balance + ?1, used = 1 WHERE id = ?2",
            rusqlite::params![satoshis, id],
        ).map_err(|e| format!("Failed to update address balance: {}", e))?;
        id
    } else {
        // Find next available negative index (starting from -2, since -1 is master)
        let next_derived_index: i32 = conn.query_row(
            "SELECT COALESCE(MIN(\"index\"), 0) - 1 FROM addresses WHERE wallet_id = ?1 AND \"index\" < 0",
            rusqlite::params![wallet_id],
            |row| row.get(0),
        ).unwrap_or(-2);

        // Ensure we start at -2 or lower (not -1 which is master)
        let derived_index = if next_derived_index > -2 { -2 } else { next_derived_index };

        log::info!("      Using derived index: {}", derived_index);

        // Insert the derived address with real Bitcoin address and pubkey
        conn.execute(
            "INSERT INTO addresses (wallet_id, \"index\", address, public_key, used, balance, created_at, pending_utxo_check)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, 0)",
            rusqlite::params![
                wallet_id,
                derived_index,
                bitcoin_address,
                pubkey_hex,
                satoshis,
                now
            ],
        ).map_err(|e| format!("Failed to insert derived address: {}", e))?;

        // Get the inserted address ID
        conn.query_row(
            "SELECT id FROM addresses WHERE wallet_id = ?1 AND address = ?2",
            rusqlite::params![wallet_id, bitcoin_address],
            |row| row.get(0),
        ).map_err(|e| format!("Failed to get derived address ID: {}", e))?
    };

    // Check if UTXO already exists
    let utxo_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM utxos WHERE txid = ?1 AND vout = ?2)",
        rusqlite::params![txid, vout as i32],
        |row| row.get(0),
    ).unwrap_or(false);

    if utxo_exists {
        log::info!("      UTXO {}:{} already exists, updating custom_instructions", txid, vout);
        conn.execute(
            "UPDATE utxos SET custom_instructions = ?1, last_updated = ?2, address_id = ?3 WHERE txid = ?4 AND vout = ?5",
            rusqlite::params![
                custom_instructions.to_string(),
                now,
                address_id,
                txid,
                vout as i32
            ],
        ).map_err(|e| format!("Failed to update UTXO: {}", e))?;
    } else {
        // Insert the UTXO with derivation info
        conn.execute(
            "INSERT INTO utxos (address_id, txid, vout, satoshis, script, first_seen, last_updated, is_spent, custom_instructions)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8)",
            rusqlite::params![
                address_id,
                txid,
                vout as i32,
                satoshis,
                script_hex,
                now,
                now,
                custom_instructions.to_string()
            ],
        ).map_err(|e| format!("Failed to insert UTXO: {}", e))?;
    }

    log::info!("      ✅ Stored derived UTXO with real address and custom_instructions");
    Ok(())
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
    pub spends: Option<serde_json::Value>, // SDK-provided unlock scripts: { "inputIndex": { "unlockingScript": hex } }
}

// Response structure for /signAction
#[derive(Debug, Serialize, Deserialize)]
pub struct SignActionResponse {
    pub txid: String,
    #[serde(rename = "rawTx")]
    pub raw_tx: String,
    /// Input indices that weren't covered by pre-signing or spends (diagnostic).
    /// Per BSV SDK model, all custom inputs should be signed via two-phase flow
    /// (createSignature + signAction spends) before this point.
    #[serde(rename = "unsignedInputs", skip_serializing_if = "Option::is_none")]
    pub unsigned_inputs: Option<Vec<usize>>,
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
    let reservation_placeholder = pending_tx.reservation_placeholder;

    let num_user_inputs = user_input_infos.len();
    let num_wallet_inputs = input_utxos.len();
    log::info!("   Signing {} inputs ({} user, {} wallet)...",
        tx.inputs.len(), num_user_inputs, num_wallet_inputs);

    // Process spends parameter: apply SDK-provided unlocking scripts.
    // In two-phase flow, the SDK computes unlock scripts via createSignature
    // and passes them here. Each entry maps input index → { unlockingScript, sequenceNumber }.
    let mut spends_applied: std::collections::HashSet<usize> = std::collections::HashSet::new();
    if let Some(spends) = &req.spends {
        if let Some(spends_map) = spends.as_object() {
            log::info!("   Processing {} spend(s) from SDK...", spends_map.len());
            for (idx_str, spend_info) in spends_map {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if idx < tx.inputs.len() {
                        if let Some(script_hex) = spend_info.get("unlockingScript").and_then(|v| v.as_str()) {
                            match hex::decode(script_hex) {
                                Ok(script_bytes) => {
                                    log::info!("   Input {} (spends): applying SDK-provided unlocking script ({} bytes)",
                                        idx, script_bytes.len());
                                    tx.inputs[idx].set_script(script_bytes);
                                    spends_applied.insert(idx);

                                    // Apply optional sequence number
                                    if let Some(seq) = spend_info.get("sequenceNumber").and_then(|v| v.as_u64()) {
                                        tx.inputs[idx].sequence = seq as u32;
                                    }
                                }
                                Err(e) => {
                                    log::warn!("   Input {} (spends): invalid unlocking script hex: {}", idx, e);
                                }
                            }
                        }
                    } else {
                        log::warn!("   Input {} (spends): index out of range (tx has {} inputs)", idx, tx.inputs.len());
                    }
                }
            }
        }
    }

    // Track inputs that remain unsigned (for two-phase flow detection)
    let mut unsigned_inputs: Vec<usize> = Vec::new();

    // Process USER inputs: apply pre-signed scripts and SDK-provided spends.
    // Inputs without unlocking scripts are left unsigned — the SDK handles them
    // via two-phase flow (createSignature + signAction with spends).
    for (i, user_input) in user_input_infos.iter().enumerate() {
        if user_input.is_pre_signed {
            log::info!("   Input {} (user): {}:{} - already pre-signed, skipping",
                i, &user_input.txid[..16], user_input.vout);
            continue;
        }

        if spends_applied.contains(&i) {
            log::info!("   Input {} (user): {}:{} - signed via spends parameter",
                i, &user_input.txid[..16], user_input.vout);
            continue;
        }

        // Input is neither pre-signed nor covered by spends — left unsigned.
        // In two-phase flow (phase 1), this is expected for PushDrop inputs.
        // The SDK will compute unlock scripts via createSignature and call
        // signAction with spends in phase 2.
        log::info!("   Input {} (user): {}:{} - left unsigned (SDK signs via two-phase)",
            i, &user_input.txid[..16], user_input.vout);
        unsigned_inputs.push(i);
    }

    // Sign WALLET inputs
    for (wallet_idx, input_utxo) in input_utxos.iter().enumerate() {
        let i = num_user_inputs + wallet_idx;  // Actual input index in transaction
        log::info!("   Signing input {} (wallet): {}:{} (address index {})",
            i, input_utxo.txid, input_utxo.vout, input_utxo.address_index);

        // Get the private key for THIS specific address (not always index 0!)
        let db = state.database.lock().unwrap();
        let private_key_bytes = match crate::database::derive_private_key_for_utxo(&db, input_utxo.address_index, input_utxo.custom_instructions.as_deref()) {
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

        // Deduplication: skip if this parent tx is already in the BEEF (e.g., from inputBEEF).
        // This happens when a user input and a wallet input share the same parent transaction
        // (e.g., spending a PushDrop token output AND change from the same tx).
        if let Some(existing_idx) = beef.find_txid(&utxo.txid) {
            log::info!("   ⏭️  Parent tx {} already in BEEF (index {}), skipping duplicate fetch", &utxo.txid[..16], existing_idx);
            // Still try to attach a Merkle proof if not already present
            if existing_idx < beef.tx_to_bump.len() && beef.tx_to_bump[existing_idx].is_none() {
                // No BUMP yet — try to fetch one
                let cached_proof_result = {
                    let db = state.database.lock().unwrap();
                    let merkle_proof_repo = crate::database::MerkleProofRepository::new(db.connection());
                    merkle_proof_repo.get_by_parent_txid(&utxo.txid)
                };
                if let Ok(Some(cached_proof)) = cached_proof_result {
                    log::info!("   ✅ Attaching cached Merkle proof to existing BEEF entry (height: {})", cached_proof.block_height);
                    let enhanced_tsc = {
                        let db = state.database.lock().unwrap();
                        let merkle_proof_repo = crate::database::MerkleProofRepository::new(db.connection());
                        merkle_proof_repo.to_tsc_json(&cached_proof)
                    };
                    if !enhanced_tsc.is_null() {
                        if let Err(e) = beef.add_tsc_merkle_proof(&utxo.txid, existing_idx, &enhanced_tsc) {
                            log::warn!("   ⚠️  Failed to attach Merkle proof: {}", e);
                        }
                    }
                }
            }
            continue;
        }

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
                    // Step 2: Check if this is a local unbroadcast transaction
                    // This handles transaction chaining where child tx spends outputs from
                    // a parent tx that hasn't been confirmed/broadcast yet
                    let tx_repo = crate::database::TransactionRepository::new(db.connection());
                    if let Ok(Some(local_raw_tx)) = tx_repo.get_local_parent_tx(&utxo.txid) {
                        log::info!("   📋 Using local unbroadcast parent tx {} from transactions table", utxo.txid);
                        drop(db); // Release lock before hex decode
                        match hex::decode(&local_raw_tx) {
                            Ok(bytes) => {
                                // Mark this as a local (unconfirmed) parent for BEEF building
                                // It won't have a merkle proof, so BEEF will include it as raw tx
                                bytes
                            }
                            Err(e) => {
                                log::error!("   ❌ Failed to decode local parent tx {}: {}", utxo.txid, e);
                                continue; // Skip this parent transaction
                            }
                        }
                    } else {
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
                    } // Close else block for local tx check
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

    // Phase 2: Include ancestry chain for unconfirmed parent transactions
    // When a parent is unconfirmed (no BUMP), ARC needs its confirmed ancestors
    // with BUMPs in the BEEF to validate the transaction chain.
    //
    // Wrapped in a timeout to prevent freezing if API calls hang.
    {
        let ancestry_result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            async {
                // Identify unconfirmed parents (those with no BUMP)
                let mut unconfirmed_indices: Vec<usize> = Vec::new();
                for i in 0..beef.transactions.len() {
                    if i < beef.tx_to_bump.len() && beef.tx_to_bump[i].is_none() {
                        unconfirmed_indices.push(i);
                    }
                }

                if unconfirmed_indices.is_empty() {
                    return;
                }

                log::info!("   🔗 Found {} unconfirmed parent(s), building ancestry chains for ARC...", unconfirmed_indices.len());

                // Collect input TXIDs from unconfirmed parents that need ancestry
                let mut ancestor_txids_to_add: Vec<String> = Vec::new();

                for &idx in &unconfirmed_indices {
                    let tx_bytes = &beef.transactions[idx];
                    if let Ok(parsed) = crate::beef::ParsedTransaction::from_bytes(tx_bytes) {
                        for input in &parsed.inputs {
                            // Only queue ancestors that aren't already in BEEF
                            if beef.find_txid(&input.prev_txid).is_none() {
                                log::info!("   📥 Queuing ancestor {} (parent of unconfirmed tx)", &input.prev_txid[..std::cmp::min(16, input.prev_txid.len())]);
                                ancestor_txids_to_add.push(input.prev_txid.clone());
                            }
                        }
                    }
                }

                // Use build_beef_for_txid to add each ancestor chain.
                // build_beef_for_txid stops at confirmed transactions (with BUMPs)
                // and has a MAX_BEEF_ANCESTORS safety limit.
                for ancestor_txid in &ancestor_txids_to_add {
                    if beef.find_txid(ancestor_txid).is_some() {
                        continue; // Already added by a previous iteration
                    }
                    log::info!("   🔗 Building ancestry chain for {}...", &ancestor_txid[..std::cmp::min(16, ancestor_txid.len())]);
                    match crate::beef_helpers::build_beef_for_txid(
                        ancestor_txid,
                        &mut beef,
                        &state.database,
                        &client,
                    ).await {
                        Ok(_) => {
                            log::info!("   ✅ Added ancestry chain for {}", &ancestor_txid[..std::cmp::min(16, ancestor_txid.len())]);
                        }
                        Err(e) => {
                            log::warn!("   ⚠️  Failed to build ancestry for {}: {}", &ancestor_txid[..std::cmp::min(16, ancestor_txid.len())], e);
                            // Continue - partial ancestry is better than none
                        }
                    }
                }

                // Sort transactions topologically (parents before children) as required by BRC-62
                beef.sort_topologically();
            }
        ).await;

        if ancestry_result.is_err() {
            log::warn!("   ⚠️  Ancestry chain building timed out (30s) - proceeding with partial BEEF");
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
        use crate::database::TransactionRepository;
        use crate::database::UtxoRepository;
        use crate::action_storage::ActionStatus;
        let db = state.database.lock().unwrap();
        let tx_repo = TransactionRepository::new(db.connection());
        let utxo_repo = UtxoRepository::new(db.connection());

        // First, get the old (unsigned) txid before updating
        // We need this to update any UTXOs that were created with the unsigned txid
        let old_txid = match tx_repo.get_by_reference(&req.reference) {
            Ok(Some(action)) => {
                log::info!("   📝 Old (unsigned) txid: {}", action.txid);
                Some(action.txid)
            }
            Ok(None) => {
                log::warn!("   ⚠️  Transaction not found for reference: {}", req.reference);
                None
            }
            Err(e) => {
                log::warn!("   ⚠️  Failed to get old transaction: {}", e);
                None
            }
        };

        // Update TXID in transactions table (signing changes the transaction, so TXID changes)
        if let Err(e) = tx_repo.update_txid(&req.reference, txid.clone(), signed_tx_hex.clone()) {
            log::warn!("   ⚠️  Failed to update TXID: {}", e);
        } else {
            log::info!("   💾 Transaction TXID updated: unsigned → signed");
        }

        // Update any UTXOs that were created with the unsigned txid (e.g., change outputs)
        // This is critical for transaction chaining!
        if let Some(ref old_tx) = old_txid {
            if old_tx != &txid {
                match utxo_repo.update_txid(old_tx, &txid) {
                    Ok(count) => {
                        if count > 0 {
                            log::info!("   💾 Updated {} change UTXO(s) to use signed txid", count);
                        }
                    }
                    Err(e) => {
                        log::warn!("   ⚠️  Failed to update UTXO txids: {}", e);
                    }
                }
            }
        }

        // Update status to "signed"
        if let Err(e) = tx_repo.update_status(&txid, ActionStatus::Signed) {
            log::warn!("   ⚠️  Failed to update action status: {}", e);
        } else {
            log::info!("   💾 Action status updated: created → signed");
        }

        // Update broadcast_status to "broadcast" — signAction returns a fully-signed
        // Atomic BEEF to the SDK which will submit it to the overlay network.
        // Without this, the status stays "pending" and startup cleanup deletes the UTXOs.
        if let Err(e) = tx_repo.update_broadcast_status(&txid, "broadcast") {
            log::warn!("   ⚠️  Failed to update broadcast_status: {}", e);
        } else {
            log::info!("   💾 Broadcast status updated: pending → broadcast");
        }

        // Update reserved UTXOs: placeholder spent_txid → final signed txid.
        // Covers BOTH wallet UTXOs and user-provided basket outputs that were
        // reserved during createAction with the same placeholder.
        if let Some(ref placeholder) = reservation_placeholder {
            match utxo_repo.update_spent_txid_batch(placeholder, &txid) {
                Ok(count) => {
                    log::info!("   ✅ Updated spent_txid on {} reserved UTXO(s): {} → {}",
                        count,
                        &placeholder[..std::cmp::min(20, placeholder.len())],
                        &txid[..std::cmp::min(16, txid.len())]);
                    state.balance_cache.invalidate();
                }
                Err(e) => {
                    log::warn!("   ⚠️  Failed to update spent_txid from placeholder: {}", e);
                }
            }
        } else {
            // Fallback: no placeholder (shouldn't happen in normal flow).
            // Mark wallet UTXOs as spent directly with the final txid.
            let utxos_to_mark: Vec<_> = input_utxos.iter()
                .map(|u| (u.txid.clone(), u.vout))
                .collect();
            match utxo_repo.mark_multiple_spent(&utxos_to_mark, &txid) {
                Ok(count) => {
                    log::info!("   ✅ Marked {} wallet UTXOs as spent (no placeholder fallback)", count);
                    state.balance_cache.invalidate();
                }
                Err(e) => {
                    log::warn!("   ⚠️  Failed to mark wallet UTXOs as spent: {}", e);
                }
            }
        }
    }

    // Cache signed raw tx in parent_transactions for BEEF ancestry.
    // Only when all inputs are signed (phase 3 complete or single-phase).
    // Future transactions spending outputs from this tx need the signed raw tx.
    if unsigned_inputs.is_empty() {
        let db = state.database.lock().unwrap();
        let parent_tx_repo = crate::database::ParentTransactionRepository::new(db.connection());
        match parent_tx_repo.upsert(None, &txid, &signed_tx_hex) {
            Ok(_) => {
                log::info!("   💾 Cached signed tx {} in parent_transactions for BEEF ancestry", &txid[..std::cmp::min(16, txid.len())]);
            }
            Err(e) => {
                log::warn!("   ⚠️  Failed to cache signed tx in parent_transactions: {}", e);
            }
        }
    }

    // Clean up pending transaction from memory when fully signed
    if unsigned_inputs.is_empty() {
        let mut pending = PENDING_TRANSACTIONS.lock().unwrap();
        if pending.remove(&req.reference).is_some() {
            log::info!("   🗑️  Cleaned up pending transaction for reference {}", req.reference);
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
    let unsigned_for_response = if unsigned_inputs.is_empty() {
        None
    } else {
        log::warn!("   ⚠️  {} input(s) remain unsigned: {:?}", unsigned_inputs.len(), unsigned_inputs);
        Some(unsigned_inputs)
    };

    HttpResponse::Ok().json(SignActionResponse {
        txid,
        raw_tx: beef_hex,
        unsigned_inputs: unsigned_for_response,
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
            send_with: None,
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

        let broadcast_result = broadcast_transaction(&raw_tx, Some(&state.database), Some(&txid)).await;

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

                // Update action status to "unconfirmed" and broadcast_status to "broadcast"
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
                    if let Err(e) = tx_repo.update_broadcast_status(&txid, "broadcast") {
                        log::warn!("   ⚠️  Failed to update broadcast_status: {}", e);
                    }
                }

                "completed"
            }
            Err(e) => {
                log::error!("   ❌ Broadcast failed: {}", e);

                // Update action status to "failed" and broadcast_status to "failed"
                {
                    use crate::database::TransactionRepository;
                    use crate::database::UtxoRepository;
                    use crate::action_storage::ActionStatus;
                    let db = state.database.lock().unwrap();
                    let tx_repo = TransactionRepository::new(db.connection());
                    if let Err(e) = tx_repo.update_status(&txid, ActionStatus::Failed) {
                        log::warn!("   ⚠️  Failed to update action status: {}", e);
                    } else {
                        log::info!("   💾 Action status updated: signed → failed");
                    }
                    if let Err(e) = tx_repo.update_broadcast_status(&txid, "failed") {
                        log::warn!("   ⚠️  Failed to update broadcast_status: {}", e);
                    }

                    // CRITICAL: Remove ghost change UTXO since broadcast failed
                    let utxo_repo = UtxoRepository::new(db.connection());
                    match utxo_repo.delete_by_txid(&txid) {
                        Ok(count) if count > 0 => {
                            log::info!("   🗑️  Removed {} ghost change UTXO(s) from failed broadcast", count);
                        }
                        Ok(_) => {}
                        Err(e) => {
                            log::warn!("   ⚠️  Failed to remove ghost UTXO: {}", e);
                        }
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

// (check_tx_exists_on_chain is defined earlier in the file)

// Broadcast transaction to BSV network
//
// Strategy:
// 1. If input is BEEF format → convert to V1, send BEEF directly to ARC (enables tx chaining)
// 2. If ARC fails or input is raw tx → fall back to extracting raw tx and using WhatsOnChain
//
// ARC is preferred because it accepts BEEF with unconfirmed parent transactions,
// solving the transaction chaining race condition where the second tx's parent
// hasn't propagated yet.
async fn broadcast_transaction(
    beef_or_raw_hex: &str,
    db_for_cache: Option<&std::sync::Arc<std::sync::Mutex<crate::database::WalletDatabase>>>,
    txid_for_cache: Option<&str>,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let is_beef = beef_or_raw_hex.starts_with("0100beef")
        || beef_or_raw_hex.starts_with("0200beef")
        || beef_or_raw_hex.starts_with("01010101");

    // Strategy 1: If BEEF, try ARC first (supports tx chaining via BEEF)
    if is_beef {
        log::info!("   📦 Input is BEEF format, attempting ARC broadcast with BEEF V1...");

        // Parse BEEF and convert to V1 format for ARC
        match crate::beef::Beef::from_hex(beef_or_raw_hex) {
            Ok(beef) => {
                match beef.to_v1_hex() {
                    Ok(v1_hex) => {
                        log::info!("   ✅ Converted BEEF to V1 format ({} hex chars)", v1_hex.len());

                        // Try ARC with BEEF V1
                        log::info!("   📡 Broadcasting BEEF V1 to ARC (GorillaPool)...");
                        match broadcast_to_arc(&client, &v1_hex).await {
                            Ok(arc_resp) => {
                                let arc_txid = arc_resp.txid.as_deref().unwrap_or("unknown");
                                let status_str = arc_resp.tx_status.as_deref().unwrap_or("ACCEPTED");
                                let msg = format!("ARC accepted: {} ({})", arc_txid, status_str);
                                log::info!("   🎉 ARC broadcast successful: {}", msg);

                                // Detect ARC txid mismatch (expected for multi-tx BEEF submissions).
                                // When submitting BEEF containing parent + child transactions, ARC may
                                // return the parent's txid instead of the child's. We always use our
                                // locally-computed txid for database operations, not ARC's response.
                                // See: wallet-toolbox ARC provider does the same ("txid altered from...")
                                if let Some(our_txid) = txid_for_cache {
                                    if arc_txid != "unknown" && arc_txid != our_txid {
                                        log::warn!("   ⚠️  ARC txid mismatch: ARC returned '{}', our computed txid is '{}'",
                                            &arc_txid[..arc_txid.len().min(16)],
                                            &our_txid[..our_txid.len().min(16)]);
                                        log::info!("   ℹ️  Using locally-computed txid (expected for multi-tx BEEF)");
                                    }
                                }

                                // Cache merkle proof from ARC if available
                                if let (Some(ref merkle_path), Some(db), Some(cache_txid)) =
                                    (&arc_resp.merkle_path, db_for_cache, txid_for_cache)
                                {
                                    if !merkle_path.is_empty() {
                                        log::info!("   📋 Caching ARC merklePath for {}", cache_txid);
                                        cache_arc_merkle_proof(db, cache_txid, merkle_path);
                                    }
                                }

                                // Update broadcast_status and block_height if MINED
                                if let (Some(db), Some(cache_txid)) = (db_for_cache, txid_for_cache) {
                                    if status_str == "MINED" {
                                        if let Some(height) = arc_resp.block_height {
                                            log::info!("   📦 ARC reports MINED at height {}", height);
                                            if let Ok(db_guard) = db.lock() {
                                                let tx_repo = crate::database::TransactionRepository::new(db_guard.connection());
                                                let _ = tx_repo.update_broadcast_status(cache_txid, "confirmed");
                                            }
                                        }
                                    }
                                }

                                return Ok(msg);
                            }
                            Err(e) => {
                                log::warn!("   ⚠️ ARC broadcast failed: {}", e);
                                log::info!("   🔄 Falling back to raw tx broadcasters...");
                                // Fall through to raw tx broadcast
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("   ⚠️ Failed to convert BEEF to V1: {}", e);
                        log::info!("   🔄 Falling back to raw tx broadcasters...");
                    }
                }
            }
            Err(e) => {
                log::warn!("   ⚠️ Failed to parse BEEF: {}", e);
                log::info!("   🔄 Falling back to raw tx broadcasters...");
            }
        }
    }

    // Strategy 2: Extract raw tx and use traditional broadcasters as fallback
    let raw_tx_hex = if is_beef {
        log::info!("   📦 Extracting raw transaction from BEEF for fallback broadcast...");
        match crate::beef::Beef::extract_raw_tx_hex(beef_or_raw_hex) {
            Ok(raw_hex) => {
                log::info!("   ✅ Extracted raw tx ({} bytes)", raw_hex.len() / 2);
                raw_hex
            }
            Err(e) => {
                log::error!("   ❌ Failed to extract raw tx from BEEF: {}", e);
                return Err(format!("Failed to extract raw tx from BEEF: {}", e));
            }
        }
    } else {
        log::info!("   📦 Input appears to be raw transaction hex");
        beef_or_raw_hex.to_string()
    };

    let mut last_error = String::new();

    // Fallback 1: GorillaPool mAPI (legacy)
    log::info!("   📡 Broadcasting raw tx to GorillaPool (mAPI)...");
    match broadcast_to_gorillapool(&client, &raw_tx_hex).await {
        Ok(response) => {
            log::info!("   🎉 GorillaPool mAPI: {}", response);
            return Ok(response);
        }
        Err(e) => {
            log::warn!("   ⚠️ GorillaPool mAPI failed: {}", e);
            last_error = format!("GorillaPool: {}", e);
        }
    }

    // Fallback 2: WhatsOnChain
    log::info!("   📡 Broadcasting raw tx to WhatsOnChain...");
    match broadcast_to_whatsonchain(&client, &raw_tx_hex).await {
        Ok(response) => {
            log::info!("   🎉 WhatsOnChain: {}", response);
            return Ok(response);
        }
        Err(e) => {
            log::warn!("   ⚠️ WhatsOnChain failed: {}", e);
            last_error = format!("{}; WhatsOnChain: {}", last_error, e);
        }
    }

    log::error!("   ❌ All broadcasters failed!");
    let clean_error = extract_core_error(&last_error);
    Err(clean_error)
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
                                    // Transaction was rejected - check if it's actually already in mempool
                                    let error_desc = payload["resultDescription"].as_str()
                                        .unwrap_or("Unknown error");
                                    let error_lower = error_desc.to_lowercase();

                                    // Check if the "rejection" is actually because tx is already known
                                    if error_lower.contains("already in")
                                        || error_lower.contains("already known")
                                        || error_lower.contains("duplicate")
                                        || error_lower.contains("txn-already-in-mempool")
                                        || error_lower.contains("txn-already-known") {
                                        log::info!("   ℹ️  GorillaPool: Transaction already in mempool (treating as success)");
                                        Ok(format!("Transaction already in mempool: {}", error_desc))
                                    } else {
                                        let error_msg = format!("GorillaPool rejected: {} - {}", return_result, error_desc);
                                        log::warn!("   ⚠️ {}", error_msg);
                                        Err(error_msg)
                                    }
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

/// ARC API response structure
/// See: https://bitcoin-sv.github.io/arc/api.html
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct ArcResponse {
    /// Block hash (when mined)
    #[serde(rename = "blockHash", default)]
    pub block_hash: Option<String>,
    /// Block height (when mined)
    #[serde(rename = "blockHeight", default)]
    pub block_height: Option<u64>,
    /// Extra info from miner
    #[serde(rename = "extraInfo", default)]
    pub extra_info: Option<String>,
    /// Competing transaction IDs (double spend)
    #[serde(rename = "competingTxs", default)]
    pub competing_txs: Option<Vec<String>>,
    /// Merkle path (BUMP hex when mined)
    #[serde(rename = "merklePath", default)]
    pub merkle_path: Option<String>,
    /// Timestamp
    #[serde(default)]
    pub timestamp: Option<String>,
    /// Transaction ID
    #[serde(default)]
    pub txid: Option<String>,
    /// Transaction status string (e.g., "SEEN_ON_NETWORK", "MINED")
    #[serde(rename = "txStatus", default)]
    pub tx_status: Option<String>,
    /// HTTP status code mirrored in body
    #[serde(default)]
    pub status: Option<u16>,
    /// Error title (for error responses)
    #[serde(default)]
    pub title: Option<String>,
    /// Error detail (for error responses)
    #[serde(default)]
    pub detail: Option<String>,
    /// Error type (for error responses)
    #[serde(rename = "type", default)]
    pub error_type: Option<String>,
}

/// Broadcast transaction to GorillaPool ARC endpoint
///
/// ARC (A Record of Commitments) is BSV's modern transaction processor.
/// It accepts BEEF V1 hex in JSON body: { "rawTx": "<beef_v1_hex>" }
/// This enables transaction chaining since ARC validates parent transactions
/// included in the BEEF structure without needing them to be already on-chain.
///
/// ARC endpoint: https://arc.gorillapool.io/v1/tx
/// No API key required for GorillaPool.
///
/// HTTP status codes:
/// - 200: Success (SEEN_ON_NETWORK, MINED, etc.)
/// - 409: Already known (treat as success)
/// - 460-469: BEEF-specific validation errors
/// - 400/422/500: Other errors
async fn broadcast_to_arc(client: &reqwest::Client, beef_v1_hex: &str) -> Result<ArcResponse, String> {
    let url = "https://arc.gorillapool.io/v1/tx";

    let body = serde_json::json!({
        "rawTx": beef_v1_hex
    });

    log::info!("   📡 ARC: Sending BEEF V1 ({} hex chars) to {}", beef_v1_hex.len(), url);

    let response = client.post(url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("ARC HTTP error: {}", e))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    log::info!("   📡 ARC response: HTTP {} - {}", status.as_u16(), &text[..text.len().min(500)]);

    // Parse ARC response
    let arc_response: ArcResponse = serde_json::from_str(&text)
        .map_err(|e| format!("ARC: Failed to parse response: {} - Body: {}", e, &text[..text.len().min(200)]))?;

    match status.as_u16() {
        200 | 201 => {
            // Success - transaction accepted
            let txid = arc_response.txid.as_deref().unwrap_or("unknown");
            let tx_status = arc_response.tx_status.as_deref().unwrap_or("ACCEPTED");
            log::info!("   ✅ ARC accepted: txid={}, status={}", txid, tx_status);
            if let Some(ref mp) = arc_response.merkle_path {
                log::info!("   📋 ARC returned merklePath ({} hex chars)", mp.len());
            }
            Ok(arc_response)
        }
        409 => {
            // Already known - treat as success
            let txid = arc_response.txid.as_deref().unwrap_or("unknown");
            let tx_status = arc_response.tx_status.as_deref().unwrap_or("ALREADY_KNOWN");
            log::info!("   ℹ️  ARC: Transaction already known: txid={}, status={}", txid, tx_status);
            Ok(arc_response)
        }
        460..=469 => {
            // BEEF-specific validation errors
            let detail = arc_response.detail.unwrap_or_else(|| text.clone());
            let title = arc_response.title.unwrap_or_else(|| format!("BEEF Error {}", status.as_u16()));
            log::error!("   ❌ ARC BEEF validation error {}: {} - {}", status.as_u16(), title, detail);
            Err(format!("ARC BEEF error {}: {} - {}", status.as_u16(), title, detail))
        }
        _ => {
            // Other error
            let detail = arc_response.detail
                .or(arc_response.title)
                .unwrap_or_else(|| text.clone());
            log::error!("   ❌ ARC error {}: {}", status.as_u16(), detail);
            Err(format!("ARC error {}: {}", status.as_u16(), detail))
        }
    }
}

/// Query transaction status from ARC
///
/// GET /v1/tx/{txid} - Returns status, merkle path, block info
pub async fn query_arc_tx_status(client: &reqwest::Client, txid: &str) -> Result<ArcResponse, String> {
    let url = format!("https://arc.gorillapool.io/v1/tx/{}", txid);

    let response = client.get(&url)
        .send()
        .await
        .map_err(|e| format!("ARC status query HTTP error: {}", e))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if status.is_success() {
        serde_json::from_str(&text)
            .map_err(|e| format!("ARC: Failed to parse status response: {} - Body: {}", e, &text[..text.len().min(200)]))
    } else {
        Err(format!("ARC status query failed: HTTP {} - {}", status.as_u16(), &text[..text.len().min(200)]))
    }
}

/// Cache a merkle proof from ARC response into the database
///
/// Parses the BUMP hex from ARC's merklePath, converts to TSC format,
/// and stores in the merkle_proofs table for future BEEF building.
pub fn cache_arc_merkle_proof(
    db: &std::sync::Mutex<crate::database::WalletDatabase>,
    txid: &str,
    merkle_path_hex: &str,
) {
    // Parse BUMP hex to TSC format
    let tsc = match crate::beef::parse_bump_hex_to_tsc(merkle_path_hex) {
        Ok(tsc) => tsc,
        Err(e) => {
            log::warn!("   ⚠️  Failed to parse ARC merklePath for {}: {}", txid, e);
            return;
        }
    };

    let block_height = tsc["height"].as_u64().unwrap_or(0) as u32;
    let tx_index = tsc["index"].as_u64().unwrap_or(0);
    let nodes_json = match serde_json::to_string(&tsc["nodes"]) {
        Ok(j) => j,
        Err(e) => {
            log::warn!("   ⚠️  Failed to serialize nodes for {}: {}", txid, e);
            return;
        }
    };

    let db_guard = match db.lock() {
        Ok(g) => g,
        Err(e) => {
            log::warn!("   ⚠️  Failed to lock DB for merkle proof caching: {}", e);
            return;
        }
    };

    let conn = db_guard.connection();

    // Ensure the transaction exists in parent_transactions table
    let parent_tx_repo = crate::database::ParentTransactionRepository::new(conn);
    let parent_txn_id = match parent_tx_repo.get_id_by_txid(txid) {
        Ok(Some(id)) => id,
        Ok(None) => {
            // Transaction not in parent_transactions - try to insert a minimal record
            // This can happen if ARC returns merklePath for a tx we haven't cached as a parent yet
            log::info!("   📋 Transaction {} not in parent_transactions, creating placeholder", txid);
            match parent_tx_repo.upsert(None, txid, "") {
                Ok(id) => id,
                Err(e) => {
                    log::warn!("   ⚠️  Failed to create parent_transactions entry for {}: {}", txid, e);
                    return;
                }
            }
        }
        Err(e) => {
            log::warn!("   ⚠️  DB error looking up parent_transactions for {}: {}", txid, e);
            return;
        }
    };

    // Store the merkle proof
    let merkle_proof_repo = crate::database::MerkleProofRepository::new(conn);
    match merkle_proof_repo.upsert(parent_txn_id, block_height, tx_index, "", &nodes_json) {
        Ok(_) => {
            log::info!("   💾 Cached ARC merkle proof for {} (height: {}, index: {})", txid, block_height, tx_index);
        }
        Err(e) => {
            log::warn!("   ⚠️  Failed to cache ARC merkle proof for {}: {}", txid, e);
        }
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
        // Check if the error is actually a success case (transaction already exists)
        let error_lower = text.to_lowercase();
        if error_lower.contains("already in")
            || error_lower.contains("already known")
            || error_lower.contains("duplicate")
            || error_lower.contains("txn-already-in-mempool")
            || error_lower.contains("txn-already-known") {
            log::info!("   ℹ️  WhatsOnChain: Transaction already in mempool (treating as success)");
            Ok(format!("Transaction already in mempool: {}", text))
        } else {
            Err(format!("{} - {}", status, text))
        }
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
    let (wallet_id, next_index, master_privkey, master_pubkey) = {
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
        let current_index = wallet.current_index;

        // Increment index for new address
        let next_index = current_index + 1;

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

        // Update wallet's current_index BEFORE deriving address
        // This ensures we don't retry with same index if derivation fails
        if let Err(e) = wallet_repo.update_current_index(wallet_id, next_index) {
            log::error!("   Failed to update wallet index: {}", e);
            drop(db);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to update wallet index: {}", e)
            }));
        }

        drop(db);
        (wallet_id, next_index, privkey, pubkey)
    };

    // Create BRC-43 invoice number: "2-receive address-{index}"
    let invoice_number = format!("2-receive address-{}", next_index);
    log::info!("   Invoice number: {}", invoice_number);
    log::info!("   Using index: {}", next_index);

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
            index: next_index,
            address: address.clone(),
            public_key: hex::encode(&derived_pubkey),
            used: false,
            balance: 0,
            pending_utxo_check: true,  // Mark as pending - needs UTXO check
            created_at,
        };

        // Index already updated above before derivation
        match address_repo.create(&address_model) {
            Ok(addr_id) => {
                log::info!("   ✅ Address saved to database (ID: {}, index: {}, address: {})", addr_id, next_index, address);
            }
            Err(e) => {
                log::error!("   ❌ Failed to save address to database: {}", e);
                log::error!("   Address: {}, Index: {}", address, next_index);
                // Index already incremented above, so next attempt will use new index
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to save address: {}", e)
                }));
            }
        }
    }

    // Return response in Go wallet format
    HttpResponse::Ok().json(serde_json::json!({
        "address": address,
        "index": next_index,
        "publicKey": hex::encode(&derived_pubkey)
    }))
}

/// GET /wallet/addresses - Get all addresses in wallet
pub async fn get_all_addresses(state: web::Data<AppState>) -> HttpResponse {
    log::info!("📋 GET /wallet/addresses called");

    use crate::database::{WalletRepository, AddressRepository};

    let db = state.database.lock().unwrap();
    let wallet_repo = WalletRepository::new(db.connection());
    let address_repo = AddressRepository::new(db.connection());

    // Get primary wallet
    let wallet = match wallet_repo.get_primary_wallet() {
        Ok(Some(w)) => w,
        Ok(None) => {
            log::error!("   No wallet found");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "No wallet found"
            }));
        }
        Err(e) => {
            log::error!("   Failed to get wallet: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Database error: {}", e)
            }));
        }
    };

    let wallet_id = wallet.id.unwrap();

    // Get all addresses
    match address_repo.get_all_by_wallet(wallet_id) {
        Ok(addresses) => {
            log::info!("   ✅ Retrieved {} addresses", addresses.len());

            // Convert to JSON array
            let addresses_json: Vec<serde_json::Value> = addresses.iter().map(|addr| {
                serde_json::json!({
                    "index": addr.index,
                    "address": addr.address,
                    "publicKey": addr.public_key,
                    "used": addr.used,
                    "balance": addr.balance,
                    "createdAt": addr.created_at
                })
            }).collect();

            HttpResponse::Ok().json(addresses_json)
        }
        Err(e) => {
            log::error!("   ❌ Failed to get addresses: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to get addresses: {}", e)
            }))
        }
    }
}

/// GET /wallet/address/current - Get current active address
pub async fn get_current_address(state: web::Data<AppState>) -> HttpResponse {
    log::info!("📍 GET /wallet/address/current called");

    use crate::database::{WalletRepository, AddressRepository};

    let db = state.database.lock().unwrap();
    let wallet_repo = WalletRepository::new(db.connection());
    let address_repo = AddressRepository::new(db.connection());

    // Get primary wallet
    let wallet = match wallet_repo.get_primary_wallet() {
        Ok(Some(w)) => w,
        Ok(None) => {
            log::error!("   No wallet found");
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

    // Get all addresses and find current one (highest index)
    match address_repo.get_all_by_wallet(wallet_id) {
        Ok(mut addresses) => {
            if addresses.is_empty() {
                log::warn!("   No addresses in wallet");
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "No addresses found"
                }));
            }

            // Sort by index descending to get latest
            addresses.sort_by(|a, b| b.index.cmp(&a.index));
            let current = &addresses[0];

            log::info!("   ✅ Current address: {} (index: {})", current.address, current.index);

            HttpResponse::Ok().json(serde_json::json!({
                "index": current.index,
                "address": current.address,
                "publicKey": current.public_key,
                "used": current.used,
                "balance": current.balance,
                "createdAt": current.created_at
            }))
        }
        Err(e) => {
            log::error!("   ❌ Failed to get addresses: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to get addresses: {}", e)
            }))
        }
    }
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
            basket: None,  // Simple send doesn't use baskets
            tags: None,    // Simple send doesn't use tags
        }],
        description: Some(format!("Send {} satoshis to {}", req.amount, req.to_address)),
        labels: Some(vec!["send".to_string(), "wallet".to_string()]),
        options: Some(CreateActionOptions {
            sign_and_process: Some(true),
            accept_delayed_broadcast: Some(false), // Don't delay - we want to broadcast immediately
            return_txid_only: Some(false),
            no_send: Some(true), // Don't let createAction broadcast - we'll do it ourselves
            randomize_outputs: Some(true), // Default behavior
            send_with: None,
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
    match broadcast_transaction(&raw_tx_hex, Some(&state.database), Some(&txid)).await {
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

            // Update transaction status to "failed" in database and clean up ghost UTXOs
            {
                use crate::database::TransactionRepository;
                use crate::database::UtxoRepository;
                use crate::action_storage::ActionStatus;
                let db = state.database.lock().unwrap();
                let tx_repo = TransactionRepository::new(db.connection());
                if let Err(db_err) = tx_repo.update_status(&txid, ActionStatus::Failed) {
                    log::warn!("   ⚠️  Failed to update transaction status in database: {}", db_err);
                } else {
                    log::info!("   💾 Transaction status updated to 'failed' in database");
                }

                // CRITICAL: Remove ghost change UTXO since broadcast failed
                let utxo_repo = UtxoRepository::new(db.connection());
                match utxo_repo.delete_by_txid(&txid) {
                    Ok(count) if count > 0 => {
                        log::info!("   🗑️  Removed {} ghost change UTXO(s) from failed broadcast", count);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::warn!("   ⚠️  Failed to remove ghost UTXO: {}", e);
                    }
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

    // Store the message in database
    let db = state.database.lock().unwrap();
    let repo = crate::database::MessageRelayRepository::new(db.connection());

    match repo.send_message(&request.recipient, &request.message_box, &sender, &request.body) {
        Ok(message_id) => {
            log::info!("✅ Message sent successfully with ID: {}", message_id);
            HttpResponse::Ok().json(SendMessageResponse {
                status: "success".to_string(),
            })
        }
        Err(e) => {
            log::error!("❌ Failed to store message: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "description": format!("Failed to store message: {}", e)
            }))
        }
    }
}

/// Request structure for /listMessages endpoint
#[derive(Debug, Deserialize)]
struct ListMessagesRequest {
    #[serde(rename = "messageBox")]
    message_box: String,
}

/// Message format for API response (compatible with existing clients)
#[derive(Debug, Serialize)]
struct ApiMessage {
    #[serde(rename = "messageId")]
    message_id: i64,
    sender: String,
    body: String,
}

/// Response structure for /listMessages endpoint
#[derive(Debug, Serialize)]
struct ListMessagesResponse {
    status: String,
    messages: Vec<ApiMessage>,
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

    // Retrieve messages from database
    let db = state.database.lock().unwrap();
    let repo = crate::database::MessageRelayRepository::new(db.connection());

    match repo.list_messages(&recipient, &request.message_box) {
        Ok(db_messages) => {
            // Convert to API format
            let messages: Vec<ApiMessage> = db_messages
                .into_iter()
                .map(|m| ApiMessage {
                    message_id: m.id,
                    sender: m.sender,
                    body: m.body,
                })
                .collect();

            log::info!("✅ Found {} messages", messages.len());

            HttpResponse::Ok().json(ListMessagesResponse {
                status: "success".to_string(),
                messages,
            })
        }
        Err(e) => {
            log::error!("❌ Failed to list messages: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "description": format!("Failed to list messages: {}", e)
            }))
        }
    }
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

    // Acknowledge the messages in database
    let db = state.database.lock().unwrap();
    let repo = crate::database::MessageRelayRepository::new(db.connection());

    // Convert u64 to i64 for database (SQLite uses i64)
    let message_ids_i64: Vec<i64> = request.message_ids.iter().map(|&id| id as i64).collect();

    match repo.acknowledge_messages(&recipient, &message_ids_i64) {
        Ok(deleted) => {
            log::info!("✅ Acknowledged {} messages successfully", deleted);
            HttpResponse::Ok().json(AcknowledgeMessageResponse {
                status: "success".to_string(),
            })
        }
        Err(e) => {
            log::error!("❌ Failed to acknowledge messages: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "description": format!("Failed to acknowledge messages: {}", e)
            }))
        }
    }
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
    pub tx: serde_json::Value,  // BEEF as byte array OR hex/base64 string
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
    pub payment_remittance: Option<PaymentRemittance>,
    #[serde(rename = "insertionRemittance")]
    pub insertion_remittance: Option<InsertionRemittance>,
}

/// BRC-100 Insertion Remittance information
/// Used for "basket insertion" protocol to assign received outputs to baskets
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InsertionRemittance {
    /// Basket name (required, 1-300 bytes, will be normalized)
    pub basket: String,
    /// Optional tags for this output (each 1-300 bytes)
    pub tags: Option<Vec<String>>,
    /// Optional custom instructions (app-specific metadata)
    #[serde(rename = "customInstructions")]
    pub custom_instructions: Option<String>,
}

/// BRC-29 Payment Remittance information
/// Contains derivation info for incoming payments
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaymentRemittance {
    /// Sender's identity public key (hex)
    #[serde(rename = "senderIdentityKey")]
    pub sender_identity_key: String,
    /// Derivation prefix (base64)
    #[serde(rename = "derivationPrefix")]
    pub derivation_prefix: String,
    /// Derivation suffix (base64)
    #[serde(rename = "derivationSuffix")]
    pub derivation_suffix: String,
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

    // Parse tx field - can be byte array or string (hex/base64)
    let tx_string: String = match &req.tx {
        serde_json::Value::Array(arr) => {
            // Byte array format - convert to hex string for existing parsing logic
            let bytes: Vec<u8> = arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            log::info!("   TX format: byte array ({} bytes)", bytes.len());
            // Check if it looks like Atomic BEEF (starts with 0x01010101)
            if bytes.len() >= 4 && bytes[0..4] == [0x01, 0x01, 0x01, 0x01] {
                log::info!("   Detected Atomic BEEF magic prefix");
            }
            hex::encode(&bytes)
        },
        serde_json::Value::String(s) => {
            log::info!("   TX format: string ({} chars)", s.len());
            s.clone()
        },
        _ => {
            log::error!("   TX field has invalid type: {:?}", req.tx);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "code": "ERR_INVALID_TX_FORMAT",
                "description": "tx must be a byte array or string"
            }));
        }
    };
    log::info!("   BEEF/TX length: {} chars (hex)", tx_string.len());

    // ******************************************************************************
    // ** UNTESTED CODE - REAL IMPLEMENTATION, NOT PSEUDO CODE **
    // ** This code adds Atomic BEEF support and SPV merkle proof validation. **
    // ** It has NOT been tested against real-world BEEF transactions yet. **
    // ******************************************************************************

    // Phase 2: Full BEEF parsing with ancestry validation
    // Try multiple formats: Atomic BEEF (base64/hex) -> Standard BEEF -> Raw transaction

    let (main_tx_bytes, parsed_beef, has_beef, is_atomic_beef) = {
        // Try Atomic BEEF from base64 first
        if let Ok((subject_txid, beef)) = crate::beef::Beef::from_atomic_beef_base64(&tx_string) {
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
        else if let Ok(hex_bytes) = hex::decode(&tx_string) {
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
                        match crate::beef::Beef::from_hex(&tx_string) {
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
                match crate::beef::Beef::from_hex(&tx_string) {
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
            match general_purpose::STANDARD.decode(&tx_string) {
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

    // ========================================================================
    // SECURITY: Verify transaction exists on-chain before storing
    // This prevents attackers from sending fake BEEF that was never broadcast
    // ========================================================================
    let tx_exists = match check_tx_exists_on_chain(&txid).await {
        Ok(exists) => exists,
        Err(e) => {
            log::error!("   ❌ Failed to verify transaction on-chain: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "code": "ERR_VERIFICATION_FAILED",
                "description": format!("Failed to verify transaction on-chain: {}", e)
            }));
        }
    };

    if !tx_exists {
        log::info!("   📡 Transaction not on-chain, broadcasting...");

        // Broadcast the transaction ourselves
        let raw_tx_hex = hex::encode(&main_tx_bytes);
        match broadcast_transaction(&raw_tx_hex, Some(&state.database), Some(&txid)).await {
            Ok(broadcast_msg) => {
                // broadcast_transaction returns a message string, not a txid.
                // Our locally-computed txid is authoritative (passed via txid_for_cache).
                log::info!("   ✅ Transaction broadcast successfully: {}", broadcast_msg);
            }
            Err(e) => {
                log::error!("   ❌ Failed to broadcast transaction: {}", e);
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": "error",
                    "code": "ERR_BROADCAST_FAILED",
                    "description": format!("Transaction not on-chain and broadcast failed: {}", e)
                }));
            }
        }
    } else {
        log::info!("   ✅ Transaction verified on-chain");
    }

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
    // This handles both:
    // 1. Outputs to known addresses (in our database)
    // 2. Outputs to BRC-42 derived addresses (from paymentRemittance)
    let mut total_received = 0i64;
    let mut our_output_indices = Vec::new();

    // Track derived UTXOs to store later (output_index, remittance, satoshis, script, derived_pubkey)
    let mut derived_utxos: Vec<(u32, PaymentRemittance, i64, Vec<u8>, Vec<u8>)> = Vec::new();

    // Build a map of output index -> paymentRemittance for quick lookup
    let mut remittance_map: std::collections::HashMap<u32, PaymentRemittance> = std::collections::HashMap::new();
    // BRC-100: Build a map of output index -> insertionRemittance for basket insertion
    let mut insertion_map: std::collections::HashMap<u32, InsertionRemittance> = std::collections::HashMap::new();

    if let Some(ref outputs) = req.outputs {
        for output_spec in outputs {
            // Check protocol type
            let protocol = output_spec.protocol.as_deref().unwrap_or("");

            if protocol == "wallet payment" {
                if let Some(ref remittance) = output_spec.payment_remittance {
                    remittance_map.insert(output_spec.output_index, remittance.clone());
                    log::info!("   📩 Output {} has paymentRemittance (wallet payment) from sender: {}",
                        output_spec.output_index,
                        &remittance.sender_identity_key[..std::cmp::min(16, remittance.sender_identity_key.len())]);
                }
            } else if protocol == "basket insertion" {
                if let Some(ref insertion) = output_spec.insertion_remittance {
                    // Validate and normalize basket name
                    use crate::database::basket_repo::validate_and_normalize_basket_name;
                    use crate::database::tag_repo::validate_and_normalize_tag;

                    match validate_and_normalize_basket_name(&insertion.basket) {
                        Ok(normalized_basket) => {
                            // Validate and normalize tags
                            let mut normalized_tags: Vec<String> = Vec::new();
                            if let Some(ref tags) = insertion.tags {
                                for tag in tags {
                                    match validate_and_normalize_tag(tag) {
                                        Ok(normalized) => {
                                            if !normalized_tags.contains(&normalized) {
                                                normalized_tags.push(normalized);
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("   ❌ Output {} has invalid tag: {}", output_spec.output_index, e);
                                            return HttpResponse::BadRequest().json(serde_json::json!({
                                                "status": "error",
                                                "code": "ERR_INVALID_TAG",
                                                "description": format!("Output {}: {}", output_spec.output_index, e)
                                            }));
                                        }
                                    }
                                }
                            }

                            // Create normalized insertion remittance
                            let normalized_insertion = InsertionRemittance {
                                basket: normalized_basket.clone(),
                                tags: if normalized_tags.is_empty() { None } else { Some(normalized_tags) },
                                custom_instructions: insertion.custom_instructions.clone(),
                            };
                            insertion_map.insert(output_spec.output_index, normalized_insertion);
                            log::info!("   🧺 Output {} has insertionRemittance (basket insertion) basket='{}'",
                                output_spec.output_index, normalized_basket);
                        }
                        Err(e) => {
                            log::error!("   ❌ Output {} has invalid basket name: {}", output_spec.output_index, e);
                            return HttpResponse::BadRequest().json(serde_json::json!({
                                "status": "error",
                                "code": "ERR_INVALID_BASKET_NAME",
                                "description": format!("Output {}: {}", output_spec.output_index, e)
                            }));
                        }
                    }
                }
            } else if protocol.is_empty() {
                // No protocol specified - check for paymentRemittance for backward compatibility
                if let Some(ref remittance) = output_spec.payment_remittance {
                    remittance_map.insert(output_spec.output_index, remittance.clone());
                    log::info!("   📩 Output {} has paymentRemittance from sender: {}",
                        output_spec.output_index,
                        &remittance.sender_identity_key[..std::cmp::min(16, remittance.sender_identity_key.len())]);
                }
            }
        }
    }

    // Get our master keys for BRC-42 derivation (only if we have remittances)
    let master_keys = if !remittance_map.is_empty() {
        let db = state.database.lock().unwrap();
        match (
            crate::database::get_master_private_key_from_db(&db),
            crate::database::get_master_public_key_from_db(&db)
        ) {
            (Ok(privkey), Ok(pubkey)) => Some((privkey, pubkey)),
            _ => {
                log::error!("   Failed to get master keys for BRC-42 derivation");
                None
            }
        }
    } else {
        None
    };

    for (i, output) in parsed_tx.outputs.iter().enumerate() {
        let output_index = i as u32;

        // First, check if output matches a known address in our database
        if is_output_ours(&output.script, &our_addresses) {
            total_received += output.value;
            our_output_indices.push(output_index);
            log::info!("   ✅ Output {} is ours (known address): {} satoshis", i, output.value);
            continue;
        }

        // If not a known address, check if we have paymentRemittance for this output
        if let Some(remittance) = remittance_map.get(&output_index) {
            if let Some((ref master_privkey, ref _master_pubkey)) = master_keys {
                // Derive the expected address using BRC-42
                match derive_address_from_payment_remittance(master_privkey, remittance) {
                    Ok((derived_pubkey_hash, derived_pubkey)) => {
                        // Check if output script matches the derived address
                        if verify_output_matches_derived_address(&output.script, &derived_pubkey_hash) {
                            total_received += output.value;
                            our_output_indices.push(output_index);
                            log::info!("   ✅ Output {} is ours (BRC-42 derived): {} satoshis", i, output.value);
                            log::info!("      Sender: {}...", &remittance.sender_identity_key[..std::cmp::min(16, remittance.sender_identity_key.len())]);
                            log::info!("      Derived pubkey: {}", hex::encode(&derived_pubkey));

                            // Track for UTXO storage (include derived pubkey for address calculation)
                            derived_utxos.push((output_index, remittance.clone(), output.value, output.script.clone(), derived_pubkey.clone()));
                        } else {
                            log::warn!("   ⚠️  Output {} paymentRemittance doesn't match output script!", i);
                            log::warn!("      Expected pubkey hash: {}", hex::encode(&derived_pubkey_hash));
                        }
                    }
                    Err(e) => {
                        log::error!("   ❌ Failed to derive address from paymentRemittance for output {}: {}", i, e);
                    }
                }
            }
        }
    }

    log::info!("   Total received: {} satoshis ({} outputs)", total_received, our_output_indices.len());
    if !derived_utxos.is_empty() {
        log::info!("   BRC-42 derived outputs: {}", derived_utxos.len());
    }

    if total_received == 0 {
        log::warn!("   ⚠️  No outputs belong to our wallet!");
    }

    // Store derived UTXOs with their derivation info
    if !derived_utxos.is_empty() {
        let db = state.database.lock().unwrap();

        for (vout, remittance, satoshis, script, pubkey) in &derived_utxos {
            // Store the UTXO with derivation info in custom_instructions
            let custom_instructions = serde_json::json!({
                "type": "brc29_payment",
                "senderIdentityKey": remittance.sender_identity_key,
                "derivationPrefix": remittance.derivation_prefix,
                "derivationSuffix": remittance.derivation_suffix
            });

            match store_derived_utxo(&db, &txid, *vout, *satoshis, &hex::encode(script), pubkey, &custom_instructions) {
                Ok(_) => {
                    log::info!("   💾 Stored BRC-42 derived UTXO {}:{} ({} sats)", txid, vout, satoshis);
                }
                Err(e) => {
                    log::error!("   ❌ Failed to store derived UTXO {}:{}: {}", txid, vout, e);
                }
            }
        }
    }

    // ============================================================
    // BRC-100 Basket Insertion: Store outputs with basket/tag assignments
    // ============================================================
    if !insertion_map.is_empty() {
        log::info!("   🧺 Processing {} basket insertion outputs...", insertion_map.len());

        let db = state.database.lock().unwrap();
        let utxo_repo = crate::database::UtxoRepository::new(db.connection());
        let basket_repo = crate::database::BasketRepository::new(db.connection());
        let tag_repo = crate::database::TagRepository::new(db.connection());
        let address_repo = crate::database::AddressRepository::new(db.connection());
        let wallet_repo = crate::database::WalletRepository::new(db.connection());

        // Get wallet ID for external address creation
        let wallet_id = match wallet_repo.get_primary_wallet() {
            Ok(Some(w)) => w.id.unwrap_or(1),
            _ => 1,
        };

        for (output_index, insertion) in &insertion_map {
            let output_index_usize = *output_index as usize;
            if output_index_usize >= parsed_tx.outputs.len() {
                log::warn!("   ⚠️  Basket insertion output index {} out of bounds", output_index);
                continue;
            }

            let output = &parsed_tx.outputs[output_index_usize];
            let satoshis = output.value;
            let script_hex = hex::encode(&output.script);

            // Get or create basket
            let basket_id = match basket_repo.find_or_insert(&insertion.basket) {
                Ok(id) => id,
                Err(e) => {
                    log::error!("   ❌ Failed to get basket '{}': {}", insertion.basket, e);
                    continue;
                }
            };

            // Get or create external address for this output
            // (basket insertion outputs typically have custom scripts we can't derive)
            let address_id = match address_repo.get_or_create_external_address(wallet_id) {
                Ok(id) => id,
                Err(e) => {
                    log::error!("   ❌ Failed to get external address: {}", e);
                    continue;
                }
            };

            // Build custom instructions JSON
            let custom_json = if let Some(ref ci) = insertion.custom_instructions {
                serde_json::json!({
                    "type": "basket_insertion",
                    "basket": insertion.basket,
                    "appData": ci
                }).to_string()
            } else {
                serde_json::json!({
                    "type": "basket_insertion",
                    "basket": insertion.basket
                }).to_string()
            };

            // Insert UTXO with basket assignment
            // Status is 'completed' because we've verified the BEEF/transaction
            match utxo_repo.insert_output_with_basket(
                Some(address_id),
                &txid,
                *output_index,
                satoshis,
                &script_hex,
                Some(basket_id),
                Some(&custom_json),
                "completed",
                None,  // No output description for internalized outputs
            ) {
                Ok(utxo_id) => {
                    log::info!("   💾 Stored basket insertion UTXO {}:{} in basket '{}' ({} sats)",
                              txid, output_index, insertion.basket, satoshis);

                    // Assign tags if provided
                    if let Some(ref tags) = insertion.tags {
                        for tag in tags {
                            if let Err(e) = tag_repo.assign_tag_to_output(utxo_id, tag) {
                                log::warn!("   ⚠️  Failed to assign tag '{}' to UTXO: {}", tag, e);
                            } else {
                                log::info!("      🏷️  Tagged with '{}'", tag);
                            }
                        }
                    }

                    // Track as received
                    total_received += satoshis;
                    our_output_indices.push(*output_index);
                }
                Err(e) => {
                    log::error!("   ❌ Failed to store basket insertion UTXO {}:{}: {}", txid, output_index, e);
                }
            }
        }

        // Update total received logging
        log::info!("   🧺 Total from basket insertions: {} outputs processed", insertion_map.len());
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

    // Store the action in database (idempotent - check if exists first)
    {
        use crate::database::TransactionRepository;
        let db = state.database.lock().unwrap();
        let tx_repo = TransactionRepository::new(db.connection());

        // Check if transaction already exists (idempotent handling)
        match tx_repo.get_by_txid(&txid) {
            Ok(Some(_)) => {
                log::info!("   ℹ️  Transaction {} already exists in database (idempotent success)", txid);
                // Continue - this is fine, the tx was already internalized
            }
            Ok(None) => {
                // Transaction doesn't exist, insert it
                match tx_repo.add_transaction(&stored_action) {
                    Ok(_) => {
                        log::info!("   💾 Action stored in database with status: unconfirmed");
                    }
                    Err(e) => {
                        // Check if it's a UNIQUE constraint error (race condition - another process inserted)
                        let error_str = e.to_string();
                        if error_str.contains("UNIQUE constraint failed") {
                            log::info!("   ℹ️  Transaction inserted by concurrent process (idempotent success)");
                            // Continue - this is fine
                        } else {
                            log::error!("   Failed to store action in database: {}", e);
                            return HttpResponse::InternalServerError().json(serde_json::json!({
                                "status": "error",
                                "code": "ERR_STORAGE",
                                "description": format!("Failed to store action: {}", e)
                            }));
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("   Failed to check if transaction exists: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": "error",
                    "code": "ERR_STORAGE",
                    "description": format!("Database error: {}", e)
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
    #[serde(rename = "includeOutputDescription")]
    pub include_output_description: Option<bool>,
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
    pub BEEF: Option<Vec<u8>>,  // BEEF as byte array (SDK expects number[])
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
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "outputDescription")]
    pub output_description: Option<String>,
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

    // Get database - scoped to drop lock before BEEF building (which re-locks internally)
    let (basket_id, filtered_utxos) = {
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

        // Query outputs with SQL-based tag filtering (efficient single query)
        let tag_query_mode = req.tag_query_mode.as_deref().unwrap_or("any");
        let require_all_tags = tag_query_mode == "all";

        let filtered_utxos = if !tag_ids.is_empty() {
            // Use efficient SQL-based tag filtering
            match utxo_repo.get_unspent_by_basket_with_tags(basket_id, Some(&tag_ids), require_all_tags) {
                Ok(utxos) => utxos,
                Err(e) => {
                    log::error!("   Failed to query outputs with tags: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to query outputs: {}", e)
                    }));
                }
            }
        } else {
            // No tags - just query by basket
            match utxo_repo.get_unspent_by_basket(basket_id) {
                Ok(utxos) => utxos,
                Err(e) => {
                    log::error!("   Failed to query outputs: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to query outputs: {}", e)
                    }));
                }
            }
        };

        (basket_id, filtered_utxos)
    }; // db lock dropped here - safe for BEEF building to re-lock

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
    let include_output_description = req.include_output_description.unwrap_or(false);

    let mut outputs = Vec::new();
    let mut beef = crate::beef::Beef::new();

    // Create HTTP client for BEEF building (if needed) with timeout to prevent hanging
    let client = if include_transactions {
        Some(reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new()))
    } else {
        None
    };

    for utxo in &paginated_utxos {
        let outpoint = format!("{}.{}", utxo.txid, utxo.vout);

        // Get tags and labels if requested (brief lock, released before BEEF building)
        let (tags, labels) = if include_tags || include_labels {
            let db = state.database.lock().unwrap();
            let tag_repo = crate::database::TagRepository::new(db.connection());

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

            let labels = if include_labels {
                tag_repo.get_labels_for_txid(&utxo.txid).ok()
                    .filter(|l| !l.is_empty())
            } else {
                None
            };

            (tags, labels)
        } else {
            (None, None)
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
            output_description: if include_output_description {
                utxo.output_description.clone()
            } else {
                None
            },
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

    // Serialize BEEF if built (V1 format for overlay compatibility)
    let beef_bytes = if include_transactions && !beef.transactions.is_empty() {
        match beef.to_v1_bytes() {
            Ok(bytes) => Some(bytes),
            Err(e) => {
                log::warn!("   Failed to serialize BEEF: {}", e);
                None
            }
        }
    } else {
        None
    };

    HttpResponse::Ok().json(ListOutputsResponse {
        total_outputs: total_outputs as u32,
        outputs,
        BEEF: beef_bytes,
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

/// POST /wallet/cleanup - Scan and remove ghost UTXOs
///
/// Checks all unspent UTXOs against the blockchain and removes any that
/// don't exist on-chain (ghost UTXOs from failed broadcasts).
pub async fn wallet_cleanup(state: web::Data<AppState>) -> HttpResponse {
    log::info!("🧹 /wallet/cleanup called - scanning for ghost UTXOs...");

    // Get all unspent UTXOs
    let unspent_utxos: Vec<(String, u32, i64, i64)> = {
        let db = state.database.lock().unwrap();
        let conn = db.connection();

        let mut stmt = match conn.prepare(
            "SELECT txid, vout, satoshis, address_id FROM utxos WHERE is_spent = 0"
        ) {
            Ok(s) => s,
            Err(e) => {
                log::error!("   Failed to query UTXOs: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Database error: {}", e)
                }));
            }
        };

        stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    };

    log::info!("   Found {} unspent UTXOs to check", unspent_utxos.len());

    let mut checked = 0;
    let mut ghosts_removed = 0;
    let mut ghost_sats = 0i64;
    let mut valid_count = 0;
    let mut valid_sats = 0i64;

    // Check unique txids (multiple UTXOs can share same txid with different vouts)
    let mut checked_txids: std::collections::HashMap<String, bool> = std::collections::HashMap::new();

    for (txid, vout, satoshis, _address_id) in &unspent_utxos {
        checked += 1;

        // Cache the on-chain check per txid
        let is_on_chain = if let Some(&cached) = checked_txids.get(txid) {
            cached
        } else {
            let result = check_tx_exists_on_chain(txid).await.unwrap_or(false);
            checked_txids.insert(txid.clone(), result);
            result
        };

        if is_on_chain {
            valid_count += 1;
            valid_sats += satoshis;
        } else {
            ghosts_removed += 1;
            ghost_sats += satoshis;
            log::warn!("   👻 Ghost UTXO: {}:{} ({} sats)", &txid[..std::cmp::min(16, txid.len())], vout, satoshis);

            // Mark as spent
            let db = state.database.lock().unwrap();
            let utxo_repo = crate::database::UtxoRepository::new(db.connection());
            let _ = utxo_repo.mark_spent(txid, *vout, "ghost-cleanup");
        }
    }

    log::info!("   🧹 Cleanup complete: {} checked, {} valid ({} sats), {} ghosts removed ({} sats)",
              checked, valid_count, valid_sats, ghosts_removed, ghost_sats);

    HttpResponse::Ok().json(serde_json::json!({
        "checked": checked,
        "valid": valid_count,
        "validSats": valid_sats,
        "ghostsRemoved": ghosts_removed,
        "ghostSats": ghost_sats,
    }))
}
