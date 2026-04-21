//! BRC-103/104 AuthFetch Client
//!
//! HTTP client that authenticates to external BRC-103 servers (e.g., MessageBox).
//!
//! Flow:
//! 1. Initial handshake: POST to `{base}/.well-known/auth` — exchange nonces and identity keys
//! 2. Authenticated requests: sign binary payload (BRC-104) with BRC-42-derived key
//!
//! Reference: https://bsv.brc.dev/peer-to-peer/0103

use log::{debug, info, warn};
use sha2::{Sha256, Digest};
use secp256k1::{Secp256k1, Message, SecretKey};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::crypto::brc42;

/// AuthFetch client for BRC-103/104 authenticated HTTP requests
pub struct AuthFetchClient {
    /// 33-byte compressed public key (identity key)
    identity_key: Vec<u8>,
    /// 32-byte private key
    private_key: Vec<u8>,
    /// HTTP client
    http_client: reqwest::Client,
}

/// Session state from initial handshake
struct AuthSession {
    server_identity_key: Vec<u8>,    // 33-byte compressed pubkey
    server_initial_nonce: String,    // base64
    client_initial_nonce: String,    // base64
}

/// AuthFetch error types
#[derive(Debug, thiserror::Error)]
pub enum AuthFetchError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Handshake failed: {0}")]
    Handshake(String),

    #[error("Auth failed: server rejected signed request with status {0}")]
    Rejected(u16),

    #[error("Signing error: {0}")]
    Signing(String),

    #[error("URL parse error: {0}")]
    UrlParse(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AuthFetchClient {
    /// Create a new AuthFetch client
    ///
    /// # Arguments
    /// * `identity_key` - 33-byte compressed public key
    /// * `private_key` - 32-byte private key
    pub fn new(identity_key: Vec<u8>, private_key: Vec<u8>) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            identity_key,
            private_key,
            http_client,
        }
    }

    /// Make an authenticated HTTP request using BRC-103/104
    ///
    /// Performs a fresh handshake, then sends the authenticated request.
    ///
    /// # Arguments
    /// * `method` - HTTP method (GET, POST, etc.)
    /// * `url` - Full URL to request
    /// * `body` - Optional request body bytes
    pub async fn fetch(
        &self,
        method: &str,
        url: &str,
        body: Option<&[u8]>,
    ) -> Result<reqwest::Response, AuthFetchError> {
        // Extract base URL for handshake
        let parsed = reqwest::Url::parse(url)
            .map_err(|e| AuthFetchError::UrlParse(e.to_string()))?;
        let base_url = format!(
            "{}://{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or("localhost")
        );

        // Step 1: Initial handshake to exchange nonces
        let session = self.handshake(&base_url).await?;

        // Step 2: Send authenticated request
        self.authenticated_request(method, url, body, &session).await
    }

    /// Perform initial handshake with the server
    ///
    /// POST to `{base_url}/.well-known/auth` with our identity key and nonce.
    /// Server responds with its identity key, nonce, and signature.
    async fn handshake(&self, base_url: &str) -> Result<AuthSession, AuthFetchError> {
        let client_nonce = generate_nonce_base64();

        let handshake_body = serde_json::json!({
            "version": "0.1",
            "messageType": "initialRequest",
            "identityKey": hex::encode(&self.identity_key),
            "initialNonce": client_nonce
        });

        let url = format!("{}/.well-known/auth", base_url);
        let body_bytes = serde_json::to_vec(&handshake_body)?;

        debug!("AuthFetch: handshake to {}", url);

        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body_bytes)
            .send()
            .await?;

        let status = response.status().as_u16();
        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(AuthFetchError::Handshake(format!(
                "server returned {} (expected 200): {}",
                status, body
            )));
        }

        let response_json: serde_json::Value = response.json().await?;

        let server_key_hex = response_json["identityKey"]
            .as_str()
            .ok_or_else(|| AuthFetchError::Handshake("missing identityKey in response".into()))?;

        let server_nonce = response_json["initialNonce"]
            .as_str()
            .ok_or_else(|| AuthFetchError::Handshake("missing initialNonce in response".into()))?;

        let server_identity_key = hex::decode(server_key_hex)
            .map_err(|e| AuthFetchError::Handshake(format!("invalid server identity key: {}", e)))?;

        info!(
            "AuthFetch: handshake OK — server key: {}...",
            &server_key_hex[..std::cmp::min(16, server_key_hex.len())]
        );

        Ok(AuthSession {
            server_identity_key,
            server_initial_nonce: server_nonce.to_string(),
            client_initial_nonce: client_nonce,
        })
    }

    /// Send an authenticated request with BRC-103/104 headers
    async fn authenticated_request(
        &self,
        method: &str,
        url: &str,
        body: Option<&[u8]>,
        session: &AuthSession,
    ) -> Result<reqwest::Response, AuthFetchError> {
        let request_nonce = generate_nonce_base64();
        let request_id = generate_nonce_base64();
        let body_bytes = body.unwrap_or(&[]);

        let parsed = reqwest::Url::parse(url)
            .map_err(|e| AuthFetchError::UrlParse(e.to_string()))?;
        let path = parsed.path();
        let query = parsed.query();

        // Build BRC-104 binary payload for signing
        let payload = build_request_payload(
            &request_id,
            &method.to_uppercase(),
            path,
            query,
            if body_bytes.is_empty() { None } else { Some(body_bytes) },
        );

        // Derive signing key via BRC-42
        // Invoice: "2-auth message signature-{request_nonce} {server_initial_nonce}"
        let invoice = format!(
            "2-auth message signature-{} {}",
            request_nonce, session.server_initial_nonce
        );

        let signature = self.sign_with_derived_key(
            &session.server_identity_key,
            &payload,
            &invoice,
        )?;

        debug!(
            "AuthFetch: signed request {} {} (payload {} bytes, sig {} bytes)",
            method.to_uppercase(),
            path,
            payload.len(),
            signature.len()
        );

        // Build HTTP request with auth headers
        let mut builder = match method.to_uppercase().as_str() {
            "GET" => self.http_client.get(url),
            "POST" => self.http_client.post(url),
            "PUT" => self.http_client.put(url),
            "DELETE" => self.http_client.delete(url),
            _ => self.http_client.post(url),
        };

        // Auth headers
        builder = builder
            .header("x-bsv-auth-version", "0.1")
            .header("x-bsv-auth-identity-key", hex::encode(&self.identity_key))
            .header("x-bsv-auth-nonce", &request_nonce)
            .header("x-bsv-auth-your-nonce", &session.server_initial_nonce)
            .header("x-bsv-auth-signature", hex::encode(&signature))
            .header("x-bsv-auth-request-id", &request_id);

        // Body
        if !body_bytes.is_empty() {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body_bytes.to_vec());
        }

        let response = builder.send().await?;

        let status = response.status().as_u16();
        if status == 401 || status == 403 {
            warn!("AuthFetch: server rejected authenticated request with status {}", status);
            return Err(AuthFetchError::Rejected(status));
        }

        Ok(response)
    }

    /// Sign payload with BRC-42-derived private key
    ///
    /// Derives a child private key from (our_priv, server_pub, invoice)
    /// and signs SHA-256(payload) with it.
    fn sign_with_derived_key(
        &self,
        server_pubkey: &[u8],
        payload: &[u8],
        invoice: &str,
    ) -> Result<Vec<u8>, AuthFetchError> {
        // Derive child private key via BRC-42
        let derived_key = brc42::derive_child_private_key(
            &self.private_key,
            server_pubkey,
            invoice,
        ).map_err(|e| AuthFetchError::Signing(format!("BRC-42 derivation failed: {}", e)))?;

        // SHA-256 hash the payload
        let hash = Sha256::digest(payload);

        // Sign with derived key (DER-encoded, no sighash type byte)
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&derived_key)
            .map_err(|e| AuthFetchError::Signing(format!("Invalid derived key: {}", e)))?;
        let message = Message::from_digest_slice(&hash)
            .map_err(|e| AuthFetchError::Signing(format!("Invalid message hash: {}", e)))?;
        let sig = secp.sign_ecdsa(&message, &secret);

        Ok(sig.serialize_der().to_vec())
    }
}

/// Generate 32 random bytes encoded as base64
fn generate_nonce_base64() -> String {
    let bytes: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();
    BASE64.encode(&bytes)
}

/// Build BRC-104 binary payload for request signing
///
/// Format:
/// ```text
/// [request_id: 32 raw bytes]
/// [method_length: varint][method: UTF-8]
/// [path_length: varint][path: UTF-8]
/// [query_length: varint][query: UTF-8]  // -1 varint if no query
/// [header_count: varint]
///   [key_length: varint][key][value_length: varint][value]  // sorted
/// [body_length: varint][body bytes]  // -1 varint if no body
/// ```
fn build_request_payload(
    request_id_b64: &str,
    method: &str,
    path: &str,
    query: Option<&str>,
    body: Option<&[u8]>,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(256);

    // 1. Request ID (32 raw bytes, base64-decoded)
    if let Ok(id_bytes) = BASE64.decode(request_id_b64) {
        buf.extend_from_slice(&id_bytes);
    } else {
        buf.extend_from_slice(&[0u8; 32]);
    }

    // 2. Method (length-prefixed string)
    write_string(&mut buf, method);

    // 3. Path (length-prefixed string)
    write_string(&mut buf, path);

    // 4. Query (optional string: -1 varint if absent, otherwise "?query" prefixed)
    match query {
        Some(q) if !q.is_empty() => {
            let q_with_prefix = format!("?{}", q);
            write_string(&mut buf, &q_with_prefix);
        }
        _ => write_varint_negative_one(&mut buf),
    }

    // 5. Headers (only include content-type for requests with body)
    match body {
        Some(b) if !b.is_empty() => {
            write_varint(&mut buf, 1); // 1 header
            write_string(&mut buf, "content-type");
            write_string(&mut buf, "application/json");
        }
        _ => {
            write_varint(&mut buf, 0); // no headers
        }
    }

    // 6. Body (optional: -1 varint if absent, otherwise length-prefixed)
    match body {
        Some(b) if !b.is_empty() => {
            write_varint(&mut buf, b.len() as u64);
            buf.extend_from_slice(b);
        }
        _ => write_varint_negative_one(&mut buf),
    }

    buf
}

/// Write a Bitcoin-style CompactSize/VarInt (unsigned)
fn write_varint(buf: &mut Vec<u8>, value: u64) {
    if value < 253 {
        buf.push(value as u8);
    } else if value < 0x10000 {
        buf.push(0xfd);
        buf.extend_from_slice(&(value as u16).to_le_bytes());
    } else if value < 0x100000000 {
        buf.push(0xfe);
        buf.extend_from_slice(&(value as u32).to_le_bytes());
    } else {
        buf.push(0xff);
        buf.extend_from_slice(&value.to_le_bytes());
    }
}

/// Write -1 as a VarInt (0xFF followed by 8 bytes of 0xFF)
/// Used for absent/null optional values in BRC-104 payloads
fn write_varint_negative_one(buf: &mut Vec<u8>) {
    buf.push(0xff);
    buf.extend_from_slice(&u64::MAX.to_le_bytes());
}

/// Write a length-prefixed UTF-8 string
fn write_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    write_varint(buf, bytes.len() as u64);
    buf.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::PublicKey;

    fn test_client() -> AuthFetchClient {
        let privkey = vec![1u8; 32];
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&privkey).unwrap();
        let pubkey = PublicKey::from_secret_key(&secp, &secret).serialize().to_vec();
        AuthFetchClient::new(pubkey, privkey)
    }

    #[test]
    fn test_nonce_generation_base64() {
        let nonce = generate_nonce_base64();
        let decoded = BASE64.decode(&nonce).unwrap();
        assert_eq!(decoded.len(), 32, "Nonce should be 32 bytes");
    }

    #[test]
    fn test_varint_encoding() {
        let mut buf = Vec::new();

        // Small value
        write_varint(&mut buf, 42);
        assert_eq!(buf, vec![42]);

        // Medium value
        buf.clear();
        write_varint(&mut buf, 300);
        assert_eq!(buf, vec![0xfd, 0x2c, 0x01]);

        // Negative one
        buf.clear();
        write_varint_negative_one(&mut buf);
        assert_eq!(buf, vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn test_write_string() {
        let mut buf = Vec::new();
        write_string(&mut buf, "POST");
        assert_eq!(buf, vec![4, b'P', b'O', b'S', b'T']);
    }

    #[test]
    fn test_build_payload_structure() {
        let request_id = BASE64.encode(&[0xABu8; 32]);
        let payload = build_request_payload(
            &request_id,
            "POST",
            "/sendMessage",
            None,
            Some(b"{\"test\":true}"),
        );

        // Should start with 32-byte request ID
        assert_eq!(&payload[0..32], &[0xAB; 32]);

        // Should have non-zero length (basic sanity)
        assert!(payload.len() > 64);
    }

    #[test]
    fn test_sign_with_derived_key() {
        let client = test_client();

        // Generate a "server" keypair
        let server_priv = vec![2u8; 32];
        let secp = Secp256k1::new();
        let server_secret = SecretKey::from_slice(&server_priv).unwrap();
        let server_pub = PublicKey::from_secret_key(&secp, &server_secret).serialize().to_vec();

        let payload = b"test payload data";
        let invoice = "2-auth message signature-nonce1 nonce2";

        let sig = client.sign_with_derived_key(&server_pub, payload, invoice).unwrap();

        // DER signature should be 70-72 bytes
        assert!(sig.len() >= 68 && sig.len() <= 72, "DER sig length: {}", sig.len());

        // Should be valid DER
        assert!(secp256k1::ecdsa::Signature::from_der(&sig).is_ok());
    }
}
