//! Identity Resolution via BSV Overlay Services
//!
//! Resolves identity keys (compressed public keys) to human-readable names and
//! avatars by querying the BSV Overlay Services lookup endpoint for BRC-52
//! identity certificates.
//!
//! **Flow:**
//! 1. POST to overlay lookup with `service: "ls_identity"`, `query: { identityKey, certifiers }`
//! 2. Parse BEEF outputs from response
//! 3. Decode PushDrop script to extract certificate JSON
//! 4. Decrypt publicly-revealed fields using "anyone" key (privkey = 0x01)
//! 5. Map certificate type to name/avatar fields
//!
//! Resolution is best-effort — never blocks sending. Returns None on any failure.

use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use crate::beef::{Beef, ParsedTransaction};
use crate::script::pushdrop;
use crate::crypto::brc2::decrypt_certificate_field;

/// Overlay lookup endpoints (US primary, EU fallback)
const OVERLAY_US: &str = "https://overlay-us-1.bsvb.tech/lookup";
const OVERLAY_EU: &str = "https://overlay-eu-1.bsvb.tech/lookup";

/// Cache TTL: 10 minutes
const CACHE_TTL_SECS: u64 = 600;

/// Trusted certifier public keys
const CERTIFIER_METANET: &str = "03daf815fe38f83da0ad83b5bedc520aa488aef5cbb93a93c67a7fe60406cbffe8";
const CERTIFIER_SOCIALCERT: &str = "02cf6cdf466951d8dfc9e7c9367511d0007ed6fba35ed42d425cc412fd6cfd4a17";

/// Certificate type IDs (base64-encoded)
const TYPE_TWITTER: &str = "vdDWvftf1H+5+ZprUw123kjHlywH+v20aPQTuXgMpNc=";
const TYPE_DISCORD: &str = "2TgqRC35B1zehGmB21xveZNc7i5iqHc0uxMb+1NMPW4=";
const TYPE_EMAIL: &str = "exOl3KM0dIJ04EW5pZgbZmPag6MdJXd3/a1enmUU/BA=";
const TYPE_GOV_ID: &str = "z40BOInXkI8m7f/wBrv4MJ09bZfzZbTj2fJqCtONqCY=";
const TYPE_REGISTRANT: &str = "YoPsbfR6YQczjzPdHCoGC7nJsOdPQR50+SYqcWpJ0y0=";

/// Resolved identity information
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResolvedIdentity {
    pub name: String,
    pub avatar_url: Option<String>,
    pub source: String,
    pub identity_key: String,
}

/// Cached identity entry with timestamp
struct CachedIdentity {
    resolved: Option<ResolvedIdentity>,
    fetched_at: Instant,
}

/// Identity resolver with in-memory cache
pub struct IdentityResolver {
    http_client: reqwest::Client,
    cache: Mutex<HashMap<String, CachedIdentity>>,
}

impl IdentityResolver {
    /// Create a new IdentityResolver with a 15-second HTTP timeout
    pub fn new() -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            http_client,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Resolve an identity key to a name/avatar.
    /// Returns None on any failure — resolution is best-effort.
    pub async fn resolve(&self, identity_key: &str) -> Option<ResolvedIdentity> {
        let key = identity_key.to_lowercase();

        // Check cache
        {
            let cache = self.cache.lock().ok()?;
            if let Some(cached) = cache.get(&key) {
                if cached.fetched_at.elapsed().as_secs() < CACHE_TTL_SECS {
                    debug!("IdentityResolver: cache hit for {}...", &key[..12]);
                    return cached.resolved.clone();
                }
            }
        }

        // Try US overlay, fall back to EU
        let result = match self.query_overlay(OVERLAY_US, &key).await {
            Some(r) => Some(r),
            None => {
                debug!("IdentityResolver: US overlay failed, trying EU fallback");
                self.query_overlay(OVERLAY_EU, &key).await
            }
        };

        // Cache result (even None — prevents re-querying failed lookups)
        {
            if let Ok(mut cache) = self.cache.lock() {
                cache.insert(key, CachedIdentity {
                    resolved: result.clone(),
                    fetched_at: Instant::now(),
                });
            }
        }

        result
    }

    /// Search for identities by name/attribute via BSV Overlay Services.
    /// Uses the `attributes: { any: query }` search which does fuzzy text matching
    /// across all publicly-revealed certificate fields (userName, email, name, etc.)
    ///
    /// Returns multiple results (up to `limit`). Best-effort — returns empty vec on failure.
    pub async fn search(&self, query: &str, limit: usize) -> Vec<ResolvedIdentity> {
        if query.len() < 2 {
            return Vec::new();
        }

        // Try US overlay, fall back to EU
        let results = match self.search_overlay(OVERLAY_US, query, limit).await {
            r if !r.is_empty() => r,
            _ => {
                debug!("IdentityResolver: US overlay search returned empty, trying EU fallback");
                self.search_overlay(OVERLAY_EU, query, limit).await
            }
        };

        results
    }

    /// Query a single overlay endpoint for identity certificates matching a search term
    async fn search_overlay(&self, endpoint: &str, query: &str, limit: usize) -> Vec<ResolvedIdentity> {
        let body = serde_json::json!({
            "service": "ls_identity",
            "query": {
                "attributes": { "any": query },
                "certifiers": [CERTIFIER_METANET, CERTIFIER_SOCIALCERT],
                "limit": limit
            }
        });

        debug!("IdentityResolver: search POST {} for query \"{}\"", endpoint, query);

        let response = match self.http_client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("IdentityResolver: overlay search request failed: {}", e);
                return Vec::new();
            }
        };

        let status = response.status().as_u16();
        if status != 200 {
            warn!("IdentityResolver: overlay search returned HTTP {}", status);
            return Vec::new();
        }

        let json: serde_json::Value = match response.json().await {
            Ok(j) => j,
            Err(_) => return Vec::new(),
        };

        let outputs = match json.get("outputs").and_then(|v| v.as_array()) {
            Some(o) => o,
            None => return Vec::new(),
        };

        if outputs.is_empty() {
            debug!("IdentityResolver: search returned no outputs for \"{}\"", query);
            return Vec::new();
        }

        info!("IdentityResolver: search returned {} output(s) for \"{}\"", outputs.len(), query);

        let mut results = Vec::new();
        for output_entry in outputs {
            if let Some(resolved) = self.process_search_output(output_entry) {
                // Deduplicate by identity key
                if !results.iter().any(|r: &ResolvedIdentity| r.identity_key == resolved.identity_key) {
                    results.push(resolved);
                }
                if results.len() >= limit {
                    break;
                }
            }
        }

        results
    }

    /// Process a search output — same as process_output but extracts identity key from certificate subject.
    /// Handles BEEF as either base64 string or JSON byte array (overlay returns byte arrays for attribute searches).
    fn process_search_output(&self, output_entry: &serde_json::Value) -> Option<ResolvedIdentity> {
        let output_index = output_entry.get("outputIndex").and_then(|v| v.as_u64())? as usize;

        // BEEF can be base64 string OR byte array depending on overlay response format
        let beef_bytes: Vec<u8> = if let Some(b64) = output_entry.get("beef").and_then(|v| v.as_str()) {
            use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
            BASE64.decode(b64).ok()?
        } else if let Some(arr) = output_entry.get("beef").and_then(|v| v.as_array()) {
            arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect()
        } else {
            return None;
        };

        let beef = Beef::from_bytes(&beef_bytes)
            .map_err(|e| {
                debug!("IdentityResolver: search BEEF parse failed: {}", e);
                e
            })
            .ok()?;
        let main_tx_bytes = beef.main_transaction()?;
        let parsed_tx = ParsedTransaction::from_bytes(main_tx_bytes).ok()?;

        if output_index >= parsed_tx.outputs.len() {
            return None;
        }

        let output_script = &parsed_tx.outputs[output_index].script;
        let decoded = pushdrop::decode(output_script).ok()?;

        if decoded.fields.is_empty() {
            return None;
        }

        let cert_json_str = String::from_utf8(decoded.fields[0].clone()).ok()?;
        let cert: serde_json::Value = serde_json::from_str(&cert_json_str).ok()?;

        // Extract the subject (identity key) from the certificate
        let subject = cert.get("subject").and_then(|v| v.as_str())?;
        self.extract_identity_from_certificate(&cert, subject)
    }

    /// Query a single overlay endpoint for identity certificates
    async fn query_overlay(&self, endpoint: &str, identity_key: &str) -> Option<ResolvedIdentity> {
        let body = serde_json::json!({
            "service": "ls_identity",
            "query": {
                "identityKey": identity_key,
                "certifiers": [CERTIFIER_METANET, CERTIFIER_SOCIALCERT]
            }
        });

        debug!("IdentityResolver: POST {} for key {}...", endpoint, &identity_key[..12]);

        let response = self.http_client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                warn!("IdentityResolver: overlay request failed: {}", e);
                e
            })
            .ok()?;

        let status = response.status().as_u16();
        if status != 200 {
            warn!("IdentityResolver: overlay returned HTTP {}", status);
            return None;
        }

        let json: serde_json::Value = response.json().await.ok()?;

        // Response format: { type: "output-list", outputs: [{ beef: "<base64>", outputIndex: N }] }
        let outputs = json.get("outputs").and_then(|v| v.as_array())?;

        if outputs.is_empty() {
            debug!("IdentityResolver: no outputs for key {}...", &identity_key[..12]);
            return None;
        }

        info!("IdentityResolver: {} output(s) for key {}...", outputs.len(), &identity_key[..12]);

        // Try each output until we find a valid certificate
        for output_entry in outputs {
            if let Some(resolved) = self.process_output(output_entry, identity_key) {
                return Some(resolved);
            }
        }

        debug!("IdentityResolver: no valid certificates decoded from {} outputs", outputs.len());
        None
    }

    /// Process a single overlay output: parse BEEF → extract script → decode PushDrop → decrypt fields
    fn process_output(&self, output_entry: &serde_json::Value, identity_key: &str) -> Option<ResolvedIdentity> {
        let beef_b64 = output_entry.get("beef").and_then(|v| v.as_str())?;
        let output_index = output_entry.get("outputIndex").and_then(|v| v.as_u64())? as usize;

        // Parse BEEF
        let (_txid, beef) = Beef::from_atomic_beef_base64(beef_b64)
            .map_err(|e| {
                debug!("IdentityResolver: BEEF parse failed: {}", e);
                e
            })
            .ok()?;

        // Get the main transaction (last in array)
        let main_tx_bytes = beef.main_transaction()?;

        // Parse the transaction to access outputs
        let parsed_tx = ParsedTransaction::from_bytes(main_tx_bytes)
            .map_err(|e| {
                debug!("IdentityResolver: tx parse failed: {}", e);
                e
            })
            .ok()?;

        if output_index >= parsed_tx.outputs.len() {
            debug!("IdentityResolver: outputIndex {} out of range (tx has {} outputs)",
                   output_index, parsed_tx.outputs.len());
            return None;
        }

        let output_script = &parsed_tx.outputs[output_index].script;

        // Decode PushDrop to extract certificate data
        let decoded = pushdrop::decode(output_script)
            .map_err(|e| {
                debug!("IdentityResolver: PushDrop decode failed: {:?}", e);
                e
            })
            .ok()?;

        if decoded.fields.is_empty() {
            debug!("IdentityResolver: PushDrop has no fields");
            return None;
        }

        // fields[0] should be the certificate JSON
        let cert_json_str = String::from_utf8(decoded.fields[0].clone())
            .map_err(|e| {
                debug!("IdentityResolver: certificate not valid UTF-8: {}", e);
                e
            })
            .ok()?;

        let cert: serde_json::Value = serde_json::from_str(&cert_json_str)
            .map_err(|e| {
                debug!("IdentityResolver: certificate JSON parse failed: {}", e);
                e
            })
            .ok()?;

        self.extract_identity_from_certificate(&cert, identity_key)
    }

    /// Extract name/avatar from a BRC-52 certificate by decrypting public fields
    fn extract_identity_from_certificate(
        &self,
        cert: &serde_json::Value,
        identity_key: &str,
    ) -> Option<ResolvedIdentity> {
        let cert_type = cert.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let subject = cert.get("subject").and_then(|v| v.as_str()).unwrap_or("");
        let certifier = cert.get("certifier").and_then(|v| v.as_str()).unwrap_or("");
        let serial_number = cert.get("serialNumber").and_then(|v| v.as_str());

        let fields = match cert.get("fields").and_then(|v| v.as_object()) {
            Some(f) => f,
            None => {
                debug!("IdentityResolver: certificate has no fields object");
                return None;
            }
        };

        // Determine which fields to try based on certificate type
        let (name_fields, avatar_fields, source_label) = match cert_type {
            t if t == TYPE_TWITTER => (
                vec!["userName"],
                vec!["profilePhoto"],
                format!("X/Twitter via {}", certifier_name(certifier)),
            ),
            t if t == TYPE_DISCORD => (
                vec!["userName"],
                vec!["profilePhoto"],
                format!("Discord via {}", certifier_name(certifier)),
            ),
            t if t == TYPE_EMAIL => (
                vec!["email"],
                vec![],
                format!("Email via {}", certifier_name(certifier)),
            ),
            t if t == TYPE_GOV_ID => (
                vec!["firstName", "lastName"],
                vec!["profilePhoto"],
                format!("Government ID via {}", certifier_name(certifier)),
            ),
            t if t == TYPE_REGISTRANT => (
                vec!["name"],
                vec!["icon"],
                format!("Registrant via {}", certifier_name(certifier)),
            ),
            _ => (
                vec!["firstName", "lastName", "name", "userName", "email"],
                vec!["profilePhoto", "avatar"],
                format!("Certificate via {}", certifier_name(certifier)),
            ),
        };

        // Anyone key for public field decryption: private key = 1
        let anyone_private_key = {
            let mut key = [0u8; 32];
            key[31] = 1;
            key
        };

        // Subject public key (the identity key owner)
        let subject_pubkey = hex::decode(subject)
            .map_err(|e| {
                debug!("IdentityResolver: subject hex decode failed: {}", e);
                e
            })
            .ok()?;

        if subject_pubkey.len() != 33 {
            debug!("IdentityResolver: subject pubkey wrong length: {}", subject_pubkey.len());
            return None;
        }

        // Get keyring — needed for two-stage decryption (BRC-52)
        let keyring = cert.get("keyring").and_then(|v| v.as_object());

        // Try to decrypt and read name fields
        let mut name_parts: Vec<String> = Vec::new();
        for field_name in &name_fields {
            if let Some(encrypted_field_b64) = fields.get(*field_name).and_then(|v| v.as_str()) {
                if let Some(decrypted) = self.decrypt_certificate_field_two_stage(
                    &anyone_private_key,
                    &subject_pubkey,
                    field_name,
                    serial_number,
                    encrypted_field_b64,
                    keyring,
                ) {
                    if !decrypted.is_empty() {
                        name_parts.push(decrypted);
                    }
                }
            }
        }

        if name_parts.is_empty() {
            debug!("IdentityResolver: no name fields decrypted for cert type {}", cert_type);
            return None;
        }

        let name = name_parts.join(" ");

        // Try to decrypt avatar field
        let mut avatar_url: Option<String> = None;
        for field_name in &avatar_fields {
            if let Some(encrypted_field_b64) = fields.get(*field_name).and_then(|v| v.as_str()) {
                if let Some(decrypted) = self.decrypt_certificate_field_two_stage(
                    &anyone_private_key,
                    &subject_pubkey,
                    field_name,
                    serial_number,
                    encrypted_field_b64,
                    keyring,
                ) {
                    if !decrypted.is_empty() && (decrypted.starts_with("http://") || decrypted.starts_with("https://")) {
                        avatar_url = Some(decrypted);
                        break;
                    }
                }
            }
        }

        info!("IdentityResolver: resolved {} → \"{}\" ({})", &identity_key[..12], name, source_label);

        Some(ResolvedIdentity {
            name,
            avatar_url,
            source: source_label,
            identity_key: identity_key.to_string(),
        })
    }

    /// Two-stage BRC-52 certificate field decryption:
    /// 1. BRC-2 decrypt keyring[fieldName] → revelation key (symmetric AES key)
    /// 2. AES-256-GCM decrypt fields[fieldName] using the revelation key
    ///
    /// This matches the ts-sdk VerifiableCertificate.decryptFields() behavior.
    /// The keyring is encrypted FROM the subject TO the verifier (anyone).
    fn decrypt_certificate_field_two_stage(
        &self,
        anyone_private_key: &[u8; 32],
        subject_pubkey: &[u8],
        field_name: &str,
        serial_number: Option<&str>,
        encrypted_field_b64: &str,
        keyring: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> Option<String> {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

        // Get keyring entry for this field
        let keyring_entry_b64 = keyring
            .and_then(|kr| kr.get(field_name))
            .and_then(|v| v.as_str());

        let keyring_entry_b64 = match keyring_entry_b64 {
            Some(v) => v,
            None => {
                debug!("IdentityResolver: no keyring entry for field {}, trying direct decrypt", field_name);
                // Fallback: try direct decrypt (old behavior, for certs without keyring)
                return self.decrypt_field(
                    anyone_private_key,
                    subject_pubkey,
                    field_name,
                    serial_number,
                    encrypted_field_b64,
                );
            }
        };

        // Stage 1: BRC-2 decrypt the keyring entry to get the revelation key (raw bytes)
        // The keyring was encrypted FROM subject TO anyone (verifier)
        // So to decrypt: anyone_privkey + subject_pubkey as counterparty
        let keyring_ciphertext = BASE64.decode(keyring_entry_b64)
            .map_err(|e| {
                debug!("IdentityResolver: keyring base64 decode failed for {}: {}", field_name, e);
                e
            })
            .ok()?;

        let revelation_key_bytes = decrypt_certificate_field(
            anyone_private_key,
            subject_pubkey,
            field_name,
            serial_number,
            &keyring_ciphertext,
        )
        .map_err(|e| {
            debug!("IdentityResolver: keyring BRC-2 decrypt failed for {}: {}", field_name, e);
            e
        })
        .ok()?;

        debug!("IdentityResolver: decrypted revelation key for {}: {} bytes", field_name, revelation_key_bytes.len());

        // Stage 2: Symmetric AES-256-GCM decrypt the field value using the revelation key
        let field_ciphertext = BASE64.decode(encrypted_field_b64)
            .map_err(|e| {
                debug!("IdentityResolver: field base64 decode failed for {}: {}", field_name, e);
                e
            })
            .ok()?;

        // Pad revelation key to 32 bytes with leading zeros (ts-sdk behavior)
        let mut padded_key = [0u8; 32];
        let key_len = revelation_key_bytes.len().min(32);
        let offset = 32 - key_len;
        padded_key[offset..].copy_from_slice(&revelation_key_bytes[..key_len]);

        // Decrypt using same AES-256-GCM format: [32-byte IV][ciphertext][16-byte tag]
        let plaintext = crate::crypto::brc2::decrypt_brc2(&field_ciphertext, &padded_key)
            .map_err(|e| {
                debug!("IdentityResolver: field symmetric decrypt failed for {}: {}", field_name, e);
                e
            })
            .ok()?;

        String::from_utf8(plaintext)
            .map_err(|e| {
                debug!("IdentityResolver: decrypted field {} not UTF-8: {}", field_name, e);
                e
            })
            .ok()
    }

    /// Decrypt a single certificate field using the anyone key (legacy direct method)
    fn decrypt_field(
        &self,
        anyone_private_key: &[u8; 32],
        subject_pubkey: &[u8],
        field_name: &str,
        serial_number: Option<&str>,
        encrypted_b64: &str,
    ) -> Option<String> {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

        // Decode base64 ciphertext
        let ciphertext = BASE64.decode(encrypted_b64)
            .map_err(|e| {
                debug!("IdentityResolver: base64 decode failed for {}: {}", field_name, e);
                e
            })
            .ok()?;

        // Decrypt using BRC-2 certificate field decryption
        let plaintext = decrypt_certificate_field(
            anyone_private_key,
            subject_pubkey,
            field_name,
            serial_number,
            &ciphertext,
        )
        .map_err(|e| {
            debug!("IdentityResolver: decrypt failed for {}: {}", field_name, e);
            e
        })
        .ok()?;

        String::from_utf8(plaintext)
            .map_err(|e| {
                debug!("IdentityResolver: decrypted field {} not UTF-8: {}", field_name, e);
                e
            })
            .ok()
    }
}

/// Map certifier pubkey to human-readable name
fn certifier_name(certifier_hex: &str) -> &str {
    match certifier_hex {
        CERTIFIER_METANET => "Metanet Trust",
        CERTIFIER_SOCIALCERT => "SocialCert",
        _ => "Unknown Certifier",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certifier_name_metanet() {
        assert_eq!(certifier_name(CERTIFIER_METANET), "Metanet Trust");
    }

    #[test]
    fn test_certifier_name_socialcert() {
        assert_eq!(certifier_name(CERTIFIER_SOCIALCERT), "SocialCert");
    }

    #[test]
    fn test_certifier_name_unknown() {
        assert_eq!(certifier_name("deadbeef"), "Unknown Certifier");
    }

    #[test]
    fn test_cert_type_mapping_twitter() {
        let resolver = IdentityResolver::new();
        let cert = serde_json::json!({
            "type": TYPE_TWITTER,
            "subject": "02".to_string() + &"a1".repeat(32),
            "certifier": CERTIFIER_SOCIALCERT,
            "fields": {}
        });
        // Should return None because no fields can be decrypted (dummy key),
        // but should not panic
        let result = resolver.extract_identity_from_certificate(&cert, "02a1a1a1");
        assert!(result.is_none()); // No decryptable fields with dummy key
    }

    #[test]
    fn test_cert_type_mapping_email() {
        let resolver = IdentityResolver::new();
        let cert = serde_json::json!({
            "type": TYPE_EMAIL,
            "subject": "03".to_string() + &"b2".repeat(32),
            "certifier": CERTIFIER_METANET,
            "fields": {}
        });
        let result = resolver.extract_identity_from_certificate(&cert, "03b2b2b2");
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_miss_returns_none_without_network() {
        let resolver = IdentityResolver::new();
        // Cache should be empty
        let cache = resolver.cache.lock().unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_source_label_format() {
        // Twitter via SocialCert
        assert_eq!(
            format!("X/Twitter via {}", certifier_name(CERTIFIER_SOCIALCERT)),
            "X/Twitter via SocialCert"
        );
        // Discord via Metanet Trust
        assert_eq!(
            format!("Discord via {}", certifier_name(CERTIFIER_METANET)),
            "Discord via Metanet Trust"
        );
    }
}
