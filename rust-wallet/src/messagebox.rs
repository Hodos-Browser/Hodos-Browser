//! MessageBox API Client with BRC-2 Encryption
//!
//! Wraps the MessageBox API (https://messagebox.babbage.systems) with:
//! - BRC-103 authenticated requests (via AuthFetchClient)
//! - BRC-2 encrypted message bodies (via derive_symmetric_key + encrypt/decrypt)
//! - Deterministic message ID generation (HMAC-SHA256)
//!
//! MessageBox API (all POST, all require AuthFetch):
//! - POST /sendMessage   — body: { message: { recipient, messageBox, messageId, body } }
//! - POST /listMessages  — body: { messageBox: "payment_inbox" }
//! - POST /acknowledgeMessage — body: { messageIds: ["id1", "id2"] }

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::authfetch::AuthFetchClient;
use crate::crypto::brc2;
use crate::crypto::brc42;
use crate::crypto::signing::hmac_sha256;

/// MessageBox API base URL
const MESSAGEBOX_URL: &str = "https://messagebox.babbage.systems";

/// BRC-2 encryption parameters for MessageBox messages
const MESSAGEBOX_INVOICE: &str = "1-messagebox-1";

/// MessageBox client with automatic BRC-2 encryption
pub struct MessageBoxClient {
    auth_client: AuthFetchClient,
    our_private_key: Vec<u8>,
    our_public_key: Vec<u8>,
}

/// An incoming message from the MessageBox
#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub message_id: String,
    pub body: Vec<u8>,  // Decrypted plaintext bytes
    pub sender: String, // Sender's identity key (hex)
}

/// MessageBox error types
#[derive(Debug, thiserror::Error)]
pub enum MessageBoxError {
    #[error("AuthFetch error: {0}")]
    AuthFetch(#[from] crate::authfetch::AuthFetchError),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("BRC-2 encryption error: {0}")]
    Encryption(String),

    #[error("BRC-2 decryption error: {0}")]
    Decryption(String),

    #[error("API error: {0}")]
    Api(String),

    /// beta.3 Phase 10d — the relay refused the message with a 4xx. Unlike a
    /// timeout or a 5xx this will not succeed on retry with the same bytes
    /// (413 = body over the cap is the case that was measured live).
    #[error("MessageBox rejected the message ({status}): {body}")]
    Rejected { status: u16, body: String },
}

impl MessageBoxError {
    /// A failure that retrying the same message cannot fix. Drives the outbox's
    /// `undeliverable` status (`P10d-A4`).
    pub fn is_permanent(&self) -> bool {
        matches!(self, MessageBoxError::Rejected { .. })
    }
}

/// The public MessageBox host's `maxMessageBodyBytes` (`ts-stack/infra/message-box-server`,
/// `resourceConfig.maxMessageBodyBytes`; measured 2026-09-15 as the 413 text
/// "Message bodies must not exceed 1048576 bytes"). Under `HODOS_DEV=1` only,
/// `HODOS_MESSAGEBOX_MAX_BODY_BYTES` overrides it so the refuse path can be driven
/// with a small real payment (`P10d-A2` T2). A production binary scrubs `HODOS_DEV`
/// (`main.rs :: enforce_dev_prod_isolation`), so the override cannot reach users.
pub const MESSAGEBOX_MAX_BODY_BYTES: usize = 1_048_576;

pub fn messagebox_max_body_bytes() -> usize {
    if std::env::var("HODOS_DEV").as_deref() == Ok("1") {
        if let Some(n) = std::env::var("HODOS_MESSAGEBOX_MAX_BODY_BYTES").ok().and_then(|s| s.parse::<usize>().ok()) {
            return n;
        }
    }
    MESSAGEBOX_MAX_BODY_BYTES
}

#[cfg(test)]
mod permanence_tests {
    use super::MessageBoxError;

    /// `P10d-A4` — a refusal is permanent; a server error or transport failure is not.
    /// Panel `F2-10d` — the modelled request size must equal what serde actually
    /// emits, for the real envelope `send_message` builds.
    #[test]
    fn wire_request_len_equals_the_serialized_request() {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        for n in [0usize, 1, 2, 3, 100, 4_999] {
            for boxname in ["payment_inbox", "x", "a_much_longer_box_name"] {
                let fake_cipher = vec![0u8; n + 48];
                let body_string = serde_json::to_string(
                    &serde_json::json!({ "encryptedMessage": BASE64.encode(&fake_cipher) })).unwrap();
                let request = serde_json::json!({ "message": {
                    "recipient": "0".repeat(66),
                    "messageBox": boxname,
                    "messageId": "a".repeat(64),
                    "body": body_string,
                }});
                let actual = serde_json::to_vec(&request).unwrap().len();
                assert_eq!(super::wire_request_len(n, boxname), actual,
                           "plaintext {} box {}", n, boxname);
            }
        }
    }

    #[test]
    fn a4_rejection_is_permanent_api_error_is_not() {
        assert!(MessageBoxError::Rejected { status: 413, body: "too large".into() }.is_permanent());
        assert!(!MessageBoxError::Api("sendMessage failed (503): busy".into()).is_permanent());
        assert!(!MessageBoxError::Encryption("x".into()).is_permanent());
    }
}

/// Exact size of the `message.body` string the server measures, for a plaintext
/// of `plaintext_len` bytes: BRC-2 adds a 32-byte IV and a 16-byte tag, the
/// result is base64'd, and wrapped as `{"encryptedMessage":"…"}`. Pure — used to
/// refuse a PeerPay BEFORE broadcasting (`P10d-A2`).
pub fn wire_body_len(plaintext_len: usize) -> usize {
    const BRC2_OVERHEAD: usize = 32 + 16;
    const WRAPPER: usize = r#"{"encryptedMessage":""}"#.len();
    let cipher_len = plaintext_len + BRC2_OVERHEAD;
    let b64_len = 4 * ((cipher_len + 2) / 3);
    WRAPPER + b64_len
}

/// Exact size of the WHOLE request body `send_message` posts, for a plaintext of
/// `plaintext_len` bytes going to `message_box`.
///
/// ⛔ Phase 10e (panel `F2-10d`). `wire_body_len` above is exact for `message.body`,
/// which is what the relay's route rule measures — but express parses the request
/// first, and `app.ts` mounts `bodyParser.json({ limit: … })` at the SAME 1 MiB. Our
/// body sits inside `{"message":{"recipient":…,"messageBox":…,"messageId":…,"body":…}}`,
/// so there was a window where we said "fits", broadcast, and then took a 413 from
/// the body parser — after the money had moved.
///
/// ⭐ Modelled rather than given a guessed margin, and checked against a real serde
/// render in `wire_request_len_equals_the_serialized_request`. The outer object is
/// fixed except for the box name: a 66-hex recipient, a 64-hex message id, and the
/// body embedded as a JSON string — which costs 4 extra bytes, one per `"` in
/// `{"encryptedMessage":"…"}` being escaped.
pub fn wire_request_len(plaintext_len: usize, message_box: &str) -> usize {
    const RECIPIENT_HEX: usize = 66;   // 33-byte compressed pubkey
    const MESSAGE_ID_HEX: usize = 64;  // sha256 hex
    const ESCAPED_QUOTES_IN_BODY: usize = 4;  // the 4 `"` of {"encryptedMessage":"…"}
    // {"message":{"recipient":"…","messageBox":"…","messageId":"…","body":"…"}}
    const FIXED: usize = r#"{"message":{"recipient":"","messageBox":"","messageId":"","body":""}}"#.len();
    FIXED + RECIPIENT_HEX + message_box.len() + MESSAGE_ID_HEX
        + wire_body_len(plaintext_len) + ESCAPED_QUOTES_IN_BODY
}

impl MessageBoxClient {
    /// Create a new MessageBox client
    ///
    /// # Arguments
    /// * `our_private_key` - 32-byte master private key
    /// * `our_public_key` - 33-byte compressed master public key
    pub fn new(our_private_key: Vec<u8>, our_public_key: Vec<u8>) -> Self {
        let auth_client = AuthFetchClient::new(our_public_key.clone(), our_private_key.clone());

        Self {
            auth_client,
            our_private_key,
            our_public_key,
        }
    }

    /// Send an encrypted message to a recipient's message box
    ///
    /// # Arguments
    /// * `recipient_pubkey` - Recipient's 33-byte compressed public key
    /// * `message_box` - Name of the message box (e.g., "payment_inbox")
    /// * `plaintext` - Message body bytes (will be BRC-2 encrypted)
    pub async fn send_message(
        &self,
        recipient_pubkey: &[u8],
        message_box: &str,
        plaintext: &[u8],
    ) -> Result<(), MessageBoxError> {
        // 1. BRC-2 encrypt the message
        let encrypted_bytes = self.encrypt_for_recipient(recipient_pubkey, plaintext)?;
        let encrypted_body = serde_json::json!({
            "encryptedMessage": BASE64.encode(&encrypted_bytes)
        });
        let body_string = serde_json::to_string(&encrypted_body)?;

        // 2. Generate deterministic message ID
        let message_id = self.generate_message_id(recipient_pubkey, body_string.as_bytes())?;

        // 3. Build request body (messageId must be inside "message" object)
        let request_body = serde_json::json!({
            "message": {
                "recipient": hex::encode(recipient_pubkey),
                "messageBox": message_box,
                "messageId": message_id,
                "body": body_string
            }
        });

        let body_bytes = serde_json::to_vec(&request_body)?;
        let url = format!("{}/sendMessage", MESSAGEBOX_URL);

        debug!("MessageBox: sending to {} box={}", &hex::encode(recipient_pubkey)[..16], message_box);

        // 4. Send via AuthFetch
        let response = self.auth_client.fetch("POST", &url, Some(&body_bytes)).await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            // The relay refused THIS message; retrying the same bytes cannot help
            // (P10d-A4): 413 body over the cap (measured), 400/422 malformed,
            // 404 unknown box. 401/403/429 and 5xx stay transient — auth, rate
            // limit and server trouble do recover on retry.
            if matches!(status, 400 | 404 | 413 | 422) {
                return Err(MessageBoxError::Rejected { status, body });
            }
            return Err(MessageBoxError::Api(format!("sendMessage failed ({}): {}", status, body)));
        }

        info!("MessageBox: message sent successfully (id={})", &message_id[..16]);
        Ok(())
    }

    /// List messages from a message box (decrypts BRC-2 encrypted messages)
    ///
    /// # Arguments
    /// * `message_box` - Name of the message box (e.g., "payment_inbox")
    ///
    /// # Returns
    /// Vec of decrypted incoming messages
    pub async fn list_messages(&self, message_box: &str) -> Result<Vec<IncomingMessage>, MessageBoxError> {
        let request_body = serde_json::json!({
            "messageBox": message_box
        });

        let body_bytes = serde_json::to_vec(&request_body)?;
        let url = format!("{}/listMessages", MESSAGEBOX_URL);

        let response = self.auth_client.fetch("POST", &url, Some(&body_bytes)).await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(MessageBoxError::Api(format!("listMessages failed ({}): {}", status, body)));
        }

        let response_text = response.text().await.unwrap_or_default();
        info!("MessageBox: listMessages raw response ({} chars): {}",
            response_text.len(),
            if response_text.len() > 500 { &response_text[..500] } else { &response_text }
        );

        let response_json: serde_json::Value = serde_json::from_str(&response_text)?;

        // Handle both top-level array and wrapped { "messages": [...] } formats
        let messages = if let Some(arr) = response_json.as_array() {
            arr.clone()
        } else if let Some(arr) = response_json.get("messages").and_then(|v| v.as_array()) {
            info!("MessageBox: found messages inside 'messages' wrapper");
            arr.clone()
        } else {
            warn!("MessageBox: listMessages returned unexpected format: {}",
                if response_text.len() > 200 { &response_text[..200] } else { &response_text });
            return Ok(Vec::new());
        };

        let mut result = Vec::new();

        for msg in messages {
            let message_id = msg["messageId"].as_str().unwrap_or("").to_string();
            let sender = msg["sender"].as_str().unwrap_or("").to_string();
            let body_str = match msg["body"].as_str() {
                Some(b) => b,
                None => continue,
            };

            // Try to decrypt BRC-2 encrypted message
            match self.decrypt_from_sender(&sender, body_str) {
                Ok(plaintext) => {
                    result.push(IncomingMessage {
                        message_id,
                        body: plaintext,
                        sender,
                    });
                }
                Err(e) => {
                    // Try as plaintext JSON (backwards compatibility)
                    warn!("MessageBox: failed to decrypt message {}: {}, trying as plaintext", &message_id, e);
                    result.push(IncomingMessage {
                        message_id,
                        body: body_str.as_bytes().to_vec(),
                        sender,
                    });
                }
            }
        }

        debug!("MessageBox: listed {} messages from {}", result.len(), message_box);
        Ok(result)
    }

    /// Acknowledge (delete) messages from the server
    ///
    /// # Arguments
    /// * `message_ids` - List of message IDs to acknowledge
    pub async fn acknowledge(&self, message_ids: &[String]) -> Result<(), MessageBoxError> {
        if message_ids.is_empty() {
            return Ok(());
        }

        let request_body = serde_json::json!({
            "messageIds": message_ids
        });

        let body_bytes = serde_json::to_vec(&request_body)?;
        let url = format!("{}/acknowledgeMessage", MESSAGEBOX_URL);

        let response = self.auth_client.fetch("POST", &url, Some(&body_bytes)).await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(MessageBoxError::Api(format!("acknowledgeMessage failed ({}): {}", status, body)));
        }

        info!("MessageBox: acknowledged {} message(s)", message_ids.len());
        Ok(())
    }

    /// BRC-2 encrypt plaintext for a recipient
    ///
    /// Protocol: [1, "messagebox"], keyID: "1"
    /// Invoice number: "1-messagebox-1"
    fn encrypt_for_recipient(&self, recipient_pubkey: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, MessageBoxError> {
        let symmetric_key = brc2::derive_symmetric_key(
            &self.our_private_key,
            recipient_pubkey,
            MESSAGEBOX_INVOICE,
        ).map_err(|e| MessageBoxError::Encryption(format!("{}", e)))?;

        brc2::encrypt_brc2(plaintext, &symmetric_key)
            .map_err(|e| MessageBoxError::Encryption(format!("{}", e)))
    }

    /// BRC-2 decrypt a message from a sender
    ///
    /// Handles MessageBox body wrapping:
    /// - Direct: `{ "encryptedMessage": "<base64>" }`
    /// - Wrapped: `{ "message": "{\"encryptedMessage\":\"<base64>\"}" }`
    fn decrypt_from_sender(&self, sender_hex: &str, body_str: &str) -> Result<Vec<u8>, MessageBoxError> {
        // Parse the body — MessageBox may wrap in {"message": "..."} envelope
        let wrapper: serde_json::Value = serde_json::from_str(body_str)?;

        // Try direct encryptedMessage first
        let encrypted_b64 = if let Some(b64) = wrapper["encryptedMessage"].as_str() {
            b64.to_string()
        } else if let Some(inner_str) = wrapper.get("message").and_then(|v| v.as_str()) {
            // MessageBox wraps body in {"message": "<original body>"} — unwrap and re-parse
            let inner: serde_json::Value = serde_json::from_str(inner_str)
                .map_err(|e| MessageBoxError::Decryption(format!("failed to parse inner message: {}", e)))?;
            inner["encryptedMessage"].as_str()
                .ok_or_else(|| MessageBoxError::Decryption("missing encryptedMessage in inner message".to_string()))?
                .to_string()
        } else {
            return Err(MessageBoxError::Decryption("missing encryptedMessage field".to_string()));
        };

        let encrypted_bytes = BASE64.decode(encrypted_b64)
            .map_err(|e| MessageBoxError::Decryption(format!("base64 decode failed: {}", e)))?;

        let sender_pubkey = hex::decode(sender_hex)
            .map_err(|e| MessageBoxError::Decryption(format!("invalid sender hex: {}", e)))?;

        // Derive symmetric key: recipient perspective (our priv + sender pub)
        let symmetric_key = brc2::derive_symmetric_key(
            &self.our_private_key,
            &sender_pubkey,
            MESSAGEBOX_INVOICE,
        ).map_err(|e| MessageBoxError::Decryption(format!("{}", e)))?;

        brc2::decrypt_brc2(&encrypted_bytes, &symmetric_key)
            .map_err(|e| MessageBoxError::Decryption(format!("{}", e)))
    }

    /// Generate deterministic message ID using HMAC-SHA256
    ///
    /// Key derived from BRC-42 (protocol [1, "messagebox"], keyID "1", counterparty = recipient)
    /// Data = message body bytes
    fn generate_message_id(&self, recipient_pubkey: &[u8], body_bytes: &[u8]) -> Result<String, MessageBoxError> {
        // Derive HMAC key from BRC-42
        let hmac_key = brc42::derive_symmetric_key_for_hmac(
            &self.our_private_key,
            recipient_pubkey,
            MESSAGEBOX_INVOICE,
        ).map_err(|e| MessageBoxError::Encryption(format!("HMAC key derivation failed: {}", e)))?;

        let hmac_result = hmac_sha256(&hmac_key, body_bytes);
        Ok(hex::encode(hmac_result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::{Secp256k1, SecretKey, PublicKey};

    fn test_keypair() -> (Vec<u8>, Vec<u8>) {
        let secp = Secp256k1::new();
        let privkey = vec![1u8; 32];
        let secret = SecretKey::from_slice(&privkey).unwrap();
        let pubkey = PublicKey::from_secret_key(&secp, &secret).serialize().to_vec();
        (privkey, pubkey)
    }

    fn test_keypair_2() -> (Vec<u8>, Vec<u8>) {
        let secp = Secp256k1::new();
        let privkey = vec![2u8; 32];
        let secret = SecretKey::from_slice(&privkey).unwrap();
        let pubkey = PublicKey::from_secret_key(&secp, &secret).serialize().to_vec();
        (privkey, pubkey)
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let (priv1, pub1) = test_keypair();
        let (priv2, pub2) = test_keypair_2();

        let plaintext = b"Hello PeerPay!";

        // Sender encrypts for recipient
        let client1 = MessageBoxClient::new(priv1.clone(), pub1.clone());
        let encrypted = client1.encrypt_for_recipient(&pub2, plaintext).unwrap();

        // Wrap in JSON format as it would be on the wire
        let wire_body = serde_json::json!({
            "encryptedMessage": BASE64.encode(&encrypted)
        });
        let wire_str = serde_json::to_string(&wire_body).unwrap();

        // Recipient decrypts from sender
        let client2 = MessageBoxClient::new(priv2, pub2);
        let decrypted = client2.decrypt_from_sender(&hex::encode(&pub1), &wire_str).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_message_id_deterministic() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();

        let client = MessageBoxClient::new(priv1, pub1);
        let body = b"test message body";

        let id1 = client.generate_message_id(&pub2, body).unwrap();
        let id2 = client.generate_message_id(&pub2, body).unwrap();

        assert_eq!(id1, id2, "Message IDs should be deterministic");
        assert_eq!(id1.len(), 64, "Message ID should be 64 hex chars (32 bytes)");
    }

    #[test]
    fn test_message_id_differs_by_content() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();

        let client = MessageBoxClient::new(priv1, pub1);

        let id1 = client.generate_message_id(&pub2, b"message 1").unwrap();
        let id2 = client.generate_message_id(&pub2, b"message 2").unwrap();

        assert_ne!(id1, id2, "Different messages should have different IDs");
    }
}
