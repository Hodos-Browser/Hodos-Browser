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

/// Largest handshake answer read (B5-T5-P7a, review 178-6 #1). A real one is well under 2 KB.
const HANDSHAKE_MAX_BYTES: usize = 64 * 1024;

/// Read a response body up to `cap` bytes; more is an error, never a larger allocation (B5-T5-P7a:
/// several of the hosts we talk to are named by a recipient).
pub(crate) async fn read_capped(mut response: reqwest::Response, cap: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > cap {
            return Err(format!("answer over {} bytes", cap));
        }
    }
    Ok(bytes)
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
        // B5-T5-P7a (review 178-5 #1): never follow a redirect. Since P7a this client talks to hosts a
        // RECIPIENT names (their signed advertisement); a redirect to http://127.0.0.1:<wallet port>
        // would re-send our request into our own wallet API, where a call without
        // `X-Requesting-Domain` is trusted as internal. The fallback is redirect-free too.
        let http_client = reqwest::Client::builder()
            .timeout(crate::services::CallClass::ThirdPartyNoFallback.timeout())
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|_| reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("an HTTP client without redirects"));

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
        // Keep an explicit port: without it the handshake of a server on a non-default port
        // went to the default port (found by B6-P1's wire test; MessageBox is on 443, unaffected).
        let base_url = match parsed.port() {
            Some(port) => format!("{}://{}:{}", parsed.scheme(), parsed.host_str().unwrap_or("localhost"), port),
            None => format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or("localhost")),
        };

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
        let client_nonce = create_handshake_nonce(&self.private_key)?;

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
        // B5-T5-P7a (review 178-6 #1): the host may be one a RECIPIENT named — never read its answer
        // without a bound.
        let body = read_capped(response, HANDSHAKE_MAX_BYTES).await
            .map_err(|e| AuthFetchError::Handshake(format!("server returned {}: answer unreadable: {}", status, e)))?;
        if status != 200 {
            return Err(AuthFetchError::Handshake(format!(
                "server returned {} (expected 200): {}",
                status, String::from_utf8_lossy(&body)
            )));
        }

        let response_json: serde_json::Value = serde_json::from_slice(&body)?;

        let (server_identity_key, server_nonce) =
            verify_initial_response(&self.private_key, &response_json, &client_nonce)?;

        let server_key_hex = hex::encode(&server_identity_key);
        info!(
            "AuthFetch: handshake OK, reply signature verified — server key: {}...",
            &server_key_hex[..16]
        );

        Ok(AuthSession {
            server_identity_key,
            server_initial_nonce: server_nonce,
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

/// Decode bytes to a string exactly as `@bsv/sdk` `Utils.toUTF8` does: WHATWG
/// `new TextDecoder().decode(bytes)` — invalid sequences become U+FFFD (maximal-subpart
/// rule, same as `String::from_utf8_lossy`) and one leading UTF-8 BOM is dropped
/// (`TextDecoder`'s default `ignoreBOM: false`), which `from_utf8_lossy` does not do.
fn sdk_to_utf8(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

/// HMAC-SHA256 under `[2,'server hmac']`, keyID = `sdk_to_utf8(first_half)`, counterparty self —
/// the second half of an `@bsv/sdk` `createNonce` nonce.
fn server_hmac(private_key: &[u8], first_half: &[u8]) -> Result<Vec<u8>, AuthFetchError> {
    use crate::crypto::brc42::derive_symmetric_key_for_hmac;
    use crate::crypto::brc43::{InvoiceNumber, SecurityLevel};
    use crate::crypto::signing::hmac_sha256;

    let own_public_key = crate::crypto::keys::derive_public_key(private_key)
        .map_err(|e| AuthFetchError::Signing(format!("nonce: public key: {}", e)))?;
    let invoice = InvoiceNumber::new(SecurityLevel::CounterpartyLevel, "server hmac", sdk_to_utf8(first_half))
        .map_err(|e| AuthFetchError::Signing(format!("nonce: invoice: {}", e)))?
        .to_string();
    let key = derive_symmetric_key_for_hmac(private_key, &own_public_key, &invoice)
        .map_err(|e| AuthFetchError::Signing(format!("nonce: key derivation: {}", e)))?;
    // The SDK keys the HMAC with `sharedSecret.x.toArray()` — minimal big-endian bytes, so a
    // leading 0x00 is dropped (~1 key in 256). HMAC treats a 31-byte key and the same key with
    // a zero in front as different keys, so strip to match (measured against SDK 2.8.11).
    let first_nonzero = key.iter().position(|b| *b != 0).unwrap_or(key.len());
    Ok(hmac_sha256(&key[first_nonzero..], first_half))
}

/// The BRC-103 handshake `initialNonce`, in `@bsv/sdk` `createNonce` format:
/// base64(16 random bytes ‖ `server_hmac` over them) = 48 bytes.
///
/// MessageBox's hardened auth middleware (ts-stack b3155fa2, GHSA-qp3j-h5xf-p2p7) refuses any
/// other length with `ERR_AUTH_MALFORMED`. Only the handshake nonce uses this format; the
/// per-request nonce and request id stay 32 random bytes (`generate_nonce_base64`).
fn create_handshake_nonce(private_key: &[u8]) -> Result<String, AuthFetchError> {
    let first_half: [u8; 16] = rand::random();
    create_handshake_nonce_from(private_key, &first_half)
}

fn create_handshake_nonce_from(private_key: &[u8], first_half: &[u8; 16]) -> Result<String, AuthFetchError> {
    let mut nonce = first_half.to_vec();
    nonce.extend(server_hmac(private_key, first_half)?);
    Ok(BASE64.encode(&nonce))
}

/// Verify the server's `initialResponse` (mirrors `@bsv/sdk` `Peer.authenticateInitialResponse`):
/// it must echo our nonce, name a valid identity key, and carry that key's signature over
/// `ourNonce ‖ serverNonce` under `[2,'auth message signature']`, keyID `"<ourNonce> <serverNonce>"`.
///
/// This proves the reply came from the key it names. Which key MessageBox *should* be is
/// beta.7 B5-T5-P1 (identity pinning), not this check.
///
/// Returns (server identity key, server nonce).
fn verify_initial_response(
    private_key: &[u8],
    reply: &serde_json::Value,
    client_nonce: &str,
) -> Result<(Vec<u8>, String), AuthFetchError> {
    let fail = |m: &str| AuthFetchError::Handshake(m.to_string());

    if reply["messageType"].as_str() != Some("initialResponse") {
        return Err(fail("reply is not an initialResponse"));
    }
    if reply["yourNonce"].as_str() != Some(client_nonce) {
        return Err(fail("reply does not echo our nonce"));
    }
    let server_key_hex = reply["identityKey"].as_str()
        .ok_or_else(|| fail("missing identityKey in response"))?;
    let server_identity_key = hex::decode(server_key_hex)
        .ok()
        .filter(|k| k.len() == 33 && secp256k1::PublicKey::from_slice(k).is_ok())
        .ok_or_else(|| fail("identityKey is not a compressed public key"))?;
    let server_nonce = reply["initialNonce"].as_str()
        .filter(|n| !n.is_empty())
        .ok_or_else(|| fail("missing initialNonce in response"))?;

    let signature: Vec<u8> = reply["signature"].as_array()
        .ok_or_else(|| fail("missing signature in response"))?
        .iter()
        .map(|v| v.as_u64().filter(|b| *b <= 255).map(|b| b as u8))
        .collect::<Option<Vec<u8>>>()
        .ok_or_else(|| fail("signature is not a byte array"))?;
    let mut signature = secp256k1::ecdsa::Signature::from_der(&signature)
        .map_err(|_| fail("signature is not DER"))?;
    // The SDK accepts any s in (0, n); libsecp256k1 only low-S. Normalising changes which
    // encodings we accept, not which (r, s) pairs verify.
    signature.normalize_s();

    let mut data = BASE64.decode(client_nonce).map_err(|_| fail("our nonce is not base64"))?;
    data.extend(BASE64.decode(server_nonce).map_err(|_| fail("server nonce is not base64"))?);
    let invoice = format!("2-auth message signature-{} {}", client_nonce, server_nonce);
    let signer = brc42::derive_child_public_key(private_key, &server_identity_key, &invoice)
        .map_err(|e| AuthFetchError::Handshake(format!("server key derivation failed: {}", e)))?;
    let signer = secp256k1::PublicKey::from_slice(&signer)
        .map_err(|_| fail("derived server key is invalid"))?;
    let message = Message::from_digest_slice(&Sha256::digest(&data))
        .map_err(|_| fail("invalid digest"))?;
    Secp256k1::verification_only()
        .verify_ecdsa(&message, &signature, &signer)
        .map_err(|_| fail("server signature does not verify"))?;

    Ok((server_identity_key, server_nonce.to_string()))
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

    // ── B6-P1 (beta.6): handshake nonce + initialResponse verification ──────────────────
    // Vectors come from @bsv/sdk (version in the file), generated by
    // development-docs/0.4.0-beta.6/g2-validator/gen_authfetch_vectors.mjs — never by our code.
    const VECTORS: &str = include_str!("../tests/fixtures/authfetch_vectors.json");

    fn vectors() -> serde_json::Value {
        serde_json::from_str(VECTORS).unwrap()
    }
    fn vector_client_priv() -> Vec<u8> {
        hex::decode(vectors()["clientPrivHex"].as_str().unwrap()).unwrap()
    }

    /// P1-A1: byte-identical to SDK createNonce for fixed prefixes, incl. invalid UTF-8,
    /// a leading BOM and a derived key with a leading zero byte.
    #[test]
    fn p1_a1_handshake_nonce_matches_sdk_vectors() {
        let v = vectors();
        let priv_key = vector_client_priv();
        let cases = v["nonces"].as_array().unwrap();
        assert!(cases.len() >= 12, "vector file lost cases");
        // Check every vector before failing, so a control can see exactly WHICH vectors diverge.
        let mut mismatches = Vec::new();
        for case in cases {
            let name = case["name"].as_str().unwrap();
            let first_half: [u8; 16] = hex::decode(case["firstHalfHex"].as_str().unwrap()).unwrap().try_into().unwrap();
            // The keyID decoding on its own, so a failure names the layer that diverged.
            if hex::encode(sdk_to_utf8(&first_half).as_bytes()) != case["keyIdUtf8Hex"].as_str().unwrap() {
                mismatches.push(format!("keyID:{}", name));
            }
            let ours = create_handshake_nonce_from(&priv_key, &first_half).unwrap();
            assert_eq!(BASE64.decode(&ours).unwrap().len(), 48, "vector {}", name);
            if ours != case["nonce"].as_str().unwrap() {
                mismatches.push(format!("nonce:{}", name));
            }
        }
        assert!(mismatches.is_empty(), "vector mismatches: [{}] of {}", mismatches.join(","), cases.len());
    }

    /// P1-A1: nonces made by the SDK's own createNonce (its own randomness) carry an HMAC we reproduce.
    #[test]
    fn p1_a1_sdk_generated_nonces_verify() {
        let v = vectors();
        let priv_key = vector_client_priv();
        for n in v["sdkNonces"].as_array().unwrap() {
            let bytes = BASE64.decode(n.as_str().unwrap()).unwrap();
            assert_eq!(bytes.len(), 48);
            assert_eq!(server_hmac(&priv_key, &bytes[..16]).unwrap(), bytes[16..].to_vec(),
                "SDK nonce {} not reproduced", n);
        }
    }

    #[test]
    fn p1_a1_random_handshake_nonce_is_48_bytes_and_self_consistent() {
        let priv_key = vector_client_priv();
        let nonce = create_handshake_nonce(&priv_key).unwrap();
        let bytes = BASE64.decode(&nonce).unwrap();
        assert_eq!(bytes.len(), 48);
        assert_eq!(server_hmac(&priv_key, &bytes[..16]).unwrap(), bytes[16..].to_vec());
    }

    fn sdk_reply(i: usize) -> (String, serde_json::Value) {
        let r = &vectors()["replies"][i];
        (r["clientNonce"].as_str().unwrap().to_string(), r["reply"].clone())
    }

    /// P1-A3: a reply produced by a real @bsv/sdk Peer (not our signer) verifies.
    #[test]
    fn p1_a3_sdk_peer_reply_verifies() {
        let v = vectors();
        let priv_key = vector_client_priv();
        for i in 0..v["replies"].as_array().unwrap().len() {
            let (client_nonce, reply) = sdk_reply(i);
            let (key, server_nonce) = verify_initial_response(&priv_key, &reply, &client_nonce)
                .unwrap_or_else(|e| panic!("SDK reply {} refused: {}", i, e));
            assert_eq!(hex::encode(key), v["serverPubHex"].as_str().unwrap());
            assert_eq!(server_nonce, reply["initialNonce"].as_str().unwrap());
        }
    }

    /// DER-encode (r, s) — used only to build the high-S form of a genuine signature.
    fn der(r: &[u8], s: &[u8]) -> Vec<u8> {
        fn int(x: &[u8]) -> Vec<u8> {
            let mut x: Vec<u8> = x.iter().copied().skip_while(|b| *b == 0).collect();
            if x.is_empty() || x[0] & 0x80 != 0 { x.insert(0, 0); }
            let mut out = vec![0x02, x.len() as u8];
            out.extend(x);
            out
        }
        let body: Vec<u8> = [int(r), int(s)].concat();
        let mut out = vec![0x30, body.len() as u8];
        out.extend(body);
        out
    }

    /// P1-A3: the SDK accepts a high-S signature, so we must too.
    #[test]
    fn p1_a3_high_s_form_of_genuine_signature_verifies() {
        const N: [u8; 32] = hex_literal_n();
        let priv_key = vector_client_priv();
        let (client_nonce, mut reply) = sdk_reply(0);
        let sig_bytes: Vec<u8> = reply["signature"].as_array().unwrap().iter().map(|b| b.as_u64().unwrap() as u8).collect();
        let compact = secp256k1::ecdsa::Signature::from_der(&sig_bytes).unwrap().serialize_compact();
        let (r, s) = compact.split_at(32);
        // high_s = n - s (big-endian subtraction)
        let mut high_s = [0u8; 32];
        let mut borrow = 0i16;
        for i in (0..32).rev() {
            let d = N[i] as i16 - s[i] as i16 - borrow;
            borrow = if d < 0 { 1 } else { 0 };
            high_s[i] = (d + 256 * borrow) as u8;
        }
        let high = der(r, &high_s);
        assert_ne!(high, sig_bytes, "fixture: high-S form must differ from the original");
        reply["signature"] = serde_json::json!(high);
        verify_initial_response(&priv_key, &reply, &client_nonce).expect("high-S form of a valid signature refused");
    }

    const fn hex_literal_n() -> [u8; 32] {
        // secp256k1 group order n
        [0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFE,
         0xBA,0xAE,0xDC,0xE6,0xAF,0x48,0xA0,0x3B,0xBF,0xD2,0x5E,0x8C,0xD0,0x36,0x41,0x41]
    }

    /// P1-A4: each forged / malformed reply is refused with its own reason; the valid reply
    /// passes in the same test, so a blanket failure cannot satisfy it.
    #[test]
    fn p1_a4_bad_replies_are_refused_each_for_its_own_reason() {
        let priv_key = vector_client_priv();
        let (client_nonce, good) = sdk_reply(0);
        verify_initial_response(&priv_key, &good, &client_nonce).expect("valid reply must pass");

        let other_key = hex::encode(PublicKey::from_secret_key(&Secp256k1::new(), &SecretKey::from_slice(&[7u8; 32]).unwrap()).serialize());
        let (_, other_reply) = sdk_reply(1); // a genuine SDK signature, for a different nonce pair
        let refusal = |name: &str, reply: serde_json::Value, nonce: &str| match verify_initial_response(&priv_key, &reply, nonce) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("case '{}': forged reply ACCEPTED", name),
        };
        let with = |f: &dyn Fn(&mut serde_json::Value)| { let mut r = good.clone(); f(&mut r); r };

        let cases: Vec<(&str, serde_json::Value, &str)> = vec![
            ("wrong messageType", with(&|r| r["messageType"] = "initialRequest".into()), "not an initialResponse"),
            ("yourNonce not ours", with(&|r| r["yourNonce"] = other_reply["yourNonce"].clone()), "does not echo our nonce"),
            ("identityKey swapped after signing", with(&|r| r["identityKey"] = other_key.clone().into()), "does not verify"),
            ("identityKey not a key", with(&|r| r["identityKey"] = "02deadbeef".into()), "not a compressed public key"),
            ("signature missing", with(&|r| { r.as_object_mut().unwrap().remove("signature"); }), "missing signature"),
            ("signature garbage", with(&|r| r["signature"] = serde_json::json!([1, 2, 3])), "not DER"),
            ("signature as hex string", with(&|r| r["signature"] = "3044".into()), "missing signature"),
            ("signature from another nonce pair", with(&|r| r["signature"] = other_reply["signature"].clone()), "does not verify"),
            ("server nonce empty", with(&|r| r["initialNonce"] = "".into()), "missing initialNonce"),
            ("server nonce changed after signing", with(&|r| r["initialNonce"] = other_reply["initialNonce"].clone()), "does not verify"),
        ];
        for (name, reply, want) in cases {
            let err = refusal(name, reply, &client_nonce);
            assert!(err.contains(want), "case '{}': expected '{}', got '{}'", name, want, err);
        }

        // Nonce order swapped: sign-check data built as serverNonce ‖ ourNonce must not verify —
        // simulated by presenting the server's nonce as ours and ours as the server's.
        let mut swapped = good.clone();
        swapped["initialNonce"] = serde_json::json!(client_nonce);
        swapped["yourNonce"] = good["initialNonce"].clone();
        let err = refusal("nonce order swapped", swapped, good["initialNonce"].as_str().unwrap());
        assert!(err.contains("does not verify"), "swapped order: {}", err);
    }

    const STAND_IN_SERVER_PRIV: &str = "c9afa9d845ba75166b5c215767b1d6934e50c3db36e89b127b8a622b120f6721";

    /// A local stand-in BRC-103 server. It answers `/.well-known/auth` claiming the identity of
    /// `claimed_priv` but signing with `signer_priv` (equal ⇒ an honest server), and `{}` to
    /// anything else. Returns every raw request it received; it stops accepting after 2 s idle.
    async fn stand_in_server(claimed_priv: Vec<u8>, signer_priv: Vec<u8>) -> (u16, tokio::task::JoinHandle<Vec<String>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let client_pub = hex::decode(vectors()["clientPubHex"].as_str().unwrap()).unwrap();
        let handle = tokio::spawn(async move {
            let mut seen = Vec::new();
            while let Ok(Ok((mut sock, _))) = tokio::time::timeout(std::time::Duration::from_secs(2), listener.accept()).await {
                let mut buf = vec![0u8; 65536];
                let mut n = 0;
                loop {
                    n += sock.read(&mut buf[n..]).await.unwrap();
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    if let Some(h) = text.find("\r\n\r\n") {
                        let len = text[..h].lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|x| x.trim().parse::<usize>().unwrap())).unwrap_or(0);
                        if n >= h + 4 + len { break; }
                    }
                }
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let body = if text.contains("/.well-known/auth") {
                    // Answer as an SDK Peer would, signing with the server key over our nonce.
                    let req: serde_json::Value = serde_json::from_str(&text[text.find("\r\n\r\n").unwrap() + 4..]).unwrap();
                    let client_nonce = req["initialNonce"].as_str().unwrap().to_string();
                    let server_nonce = BASE64.encode([9u8; 48]);
                    let mut data = BASE64.decode(&client_nonce).unwrap();
                    data.extend(BASE64.decode(&server_nonce).unwrap());
                    let invoice = format!("2-auth message signature-{} {}", client_nonce, server_nonce);
                    let k = brc42::derive_child_private_key(&signer_priv, &client_pub, &invoice).unwrap();
                    let sig = Secp256k1::new().sign_ecdsa(&Message::from_digest_slice(&Sha256::digest(&data)).unwrap(), &SecretKey::from_slice(&k).unwrap());
                    let claimed_pub = PublicKey::from_secret_key(&Secp256k1::new(), &SecretKey::from_slice(&claimed_priv).unwrap());
                    serde_json::json!({"version":"0.1","messageType":"initialResponse","identityKey":hex::encode(claimed_pub.serialize()),
                        "initialNonce":server_nonce,"yourNonce":client_nonce,"signature":sig.serialize_der().to_vec()}).to_string()
                } else { "{}".to_string() };
                seen.push(text);
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                sock.write_all(resp.as_bytes()).await.unwrap();
            }
            seen
        });
        (port, handle)
    }

    fn vector_client() -> AuthFetchClient {
        AuthFetchClient::new(hex::decode(vectors()["clientPubHex"].as_str().unwrap()).unwrap(), vector_client_priv())
    }

    /// P1-A4 through the PRODUCTION path: `fetch()` refuses a handshake reply that claims one
    /// server key but is signed by another, and never sends the authenticated request.
    #[tokio::test]
    async fn p1_a4_production_handshake_refuses_forged_reply() {
        let honest = hex::decode(STAND_IN_SERVER_PRIV).unwrap();
        let (port, server) = stand_in_server(honest, vec![5u8; 32]).await;
        let result = vector_client().fetch("POST", &format!("http://127.0.0.1:{}/listMessages", port), Some(b"{}")).await;
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("fetch() ACCEPTED a forged handshake reply"),
        };
        assert!(err.contains("does not verify"), "wrong refusal: {}", err);
        let seen = server.await.unwrap();
        assert_eq!(seen.len(), 1, "the authenticated request must not be sent after a forged handshake");
    }

    /// P1-A2: the per-request nonce and request id actually SENT stay 32 bytes, and the
    /// handshake nonce actually SENT is 48 — measured on the wire against a local stand-in
    /// server that answers the handshake honestly.
    #[tokio::test]
    async fn p1_a2_wire_nonce_lengths() {
        let honest = hex::decode(STAND_IN_SERVER_PRIV).unwrap();
        let (port, server) = stand_in_server(honest.clone(), honest).await;
        let resp = vector_client().fetch("POST", &format!("http://127.0.0.1:{}/listMessages", port), Some(b"{}")).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let seen = server.await.unwrap();

        let handshake: serde_json::Value = serde_json::from_str(&seen[0][seen[0].find("\r\n\r\n").unwrap() + 4..]).unwrap();
        assert_eq!(BASE64.decode(handshake["initialNonce"].as_str().unwrap()).unwrap().len(), 48, "handshake initialNonce");
        let header = |name: &str| seen[1].lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim().eq_ignore_ascii_case(name)).then(|| v.trim().to_string())
        }).unwrap_or_else(|| panic!("header {} not sent", name));
        assert_eq!(BASE64.decode(header("x-bsv-auth-nonce")).unwrap().len(), 32, "request nonce decoded length");
        assert_eq!(BASE64.decode(header("x-bsv-auth-request-id")).unwrap().len(), 32, "request id decoded length");
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
