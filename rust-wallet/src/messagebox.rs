//! MessageBox API Client with BRC-2 Encryption
//!
//! Wraps the MessageBox API (every host in `MESSAGEBOX_HOSTS`) with:
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

/// The MessageBox servers Hodos knows (B5-T5-P7a; owner's list, 2026-10-14). A PeerPay goes to the
/// host the recipient advertises, else to every host here; the poller reads every host here.
/// - `messagebox.babbage.systems` — ours until now; the advertisements on existing Hodos keys name it;
///   MetaNet Client and BSV Desktop before 3.1.1 read it.
/// - `messagebox.bsvblockchain.tech` — BSV Desktop ≥ 3.1.1 and BSV Browser 1.7.
/// - `message-box-us-1.bsvb.tech` — `@bsv/message-box-client`'s default (2.5.4, 2.6.0): apps on it.
/// - `gmb.bsvblockchain.tech` — BSV Browser before 1.7. A SEPARATE mailbox from
///   `messagebox.bsvblockchain.tech` despite the shared address (measured 2026-10-14,
///   `live_gmb_and_messagebox_bsvblockchain_share_a_store`).
pub const MESSAGEBOX_HOSTS: &[&str] = &[
    "https://messagebox.babbage.systems",
    "https://messagebox.bsvblockchain.tech",
    "https://message-box-us-1.bsvb.tech",
    "https://gmb.bsvblockchain.tech",
];

/// BRC-2 encryption parameters for MessageBox messages
const MESSAGEBOX_INVOICE: &str = "1-messagebox-1";

/// MessageBox client with automatic BRC-2 encryption
pub struct MessageBoxClient {
    auth_client: AuthFetchClient,
    our_private_key: Vec<u8>,
    our_public_key: Vec<u8>,
}

/// A message encrypted once, ready to post to one or more hosts (`MessageBoxClient::prepare_message`).
pub struct PreparedMessage {
    pub message_id: String,
    request_body: Vec<u8>,
}

impl PreparedMessage {
    /// The exact request, as stored in the outbox (V31) for a retry.
    pub fn request_body(&self) -> &[u8] { &self.request_body }

    /// A stored request, re-posted as is: the same bytes and the same message id.
    pub fn from_request(request_body: Vec<u8>) -> Result<Self, MessageBoxError> {
        let v: serde_json::Value = serde_json::from_slice(&request_body)?;
        let message_id = v["message"]["messageId"].as_str()
            // Review 178-5 #12: our ids are 64 hex characters; anything else would also break the
            // `[..16]` slices in the logs.
            .filter(|id| id.len() >= 16 && id.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| MessageBoxError::Api("stored request has no messageId".into()))?
            .to_string();
        Ok(PreparedMessage { message_id, request_body })
    }
}

/// A refusal that means "this server already holds that message id" (`go-message-box-server`
/// `send_message.go`: `400 ERR_DUPLICATE_MESSAGE`).
fn is_duplicate_message(status: u16, body: &str) -> bool {
    status == 400 && body.contains("ERR_DUPLICATE_MESSAGE")
}

/// What a non-2xx `sendMessage` answer means.
fn classify_send_failure(status: u16, body: String) -> Result<(), MessageBoxError> {
    // B5-T5-P7a: the server already holds this message id. Delivered, as BSV Browser treats it
    // (`isDuplicateMessageError`). Our outbox retries re-post the stored request (V31), so a host that
    // stored it but answered too late lands here on the next attempt.
    if is_duplicate_message(status, &body) {
        return Ok(());
    }
    // The relay refused THIS message; retrying the same bytes cannot help
    // (P10d-A4): 413 body over the cap (measured), 400/422 malformed,
    // 404 unknown box. 401/403/429 and 5xx stay transient — auth, rate
    // limit and server trouble do recover on retry.
    if matches!(status, 400 | 404 | 413 | 422) {
        return Err(MessageBoxError::Rejected { status, body });
    }
    Err(MessageBoxError::Api(format!("sendMessage failed ({}): {}", status, body)))
}

/// B5-T5-P7a — where a message goes: the hosts the recipient advertises, else every known host.
/// A lookup that failed is not "no advertisement", but it leaves us knowing nothing either, so it
/// gets the same safe superset (the owner's 2026-10-06 rule for an unknown host: every known one).
pub fn delivery_targets(lookup: &crate::messagebox_lookup::AdvertLookup) -> Vec<String> {
    use crate::messagebox_lookup::AdvertLookup;
    match lookup {
        // Review 178-4 #7: at most a few advertised hosts (sorted, so the choice is stable) — a recipient
        // cannot make every payer post to hundreds of servers. The reference client uses one.
        AdvertLookup::Advertised(hosts) if !hosts.is_empty() => hosts.iter().take(MAX_ADVERTISED_TARGETS).cloned().collect(),
        _ => MESSAGEBOX_HOSTS.iter().map(|h| h.to_string()).collect(),
    }
}

/// Most advertised hosts one message is posted to (review 178-4 #7).
const MAX_ADVERTISED_TARGETS: usize = 3;

/// When a message whose advertised hosts all failed goes to the known hosts as well.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallback {
    /// Not here — the send path runs inside the panel's 30 s; the outbox does it (review 178-3 #5).
    Defer,
    /// When every advertised host refused it for good (owner, 2026-10-15).
    OnRefusal,
    /// When every advertised host failed in any way — a later outbox attempt: an advertised host that
    /// is gone (DNS, connection, handshake) must not strand the payment either (review 178-4 #1).
    OnAnyFailure,
}

// ⭐ DEVELOPER NOTE (owner, 2026-10-15). Hodos posts one PeerPay message to SEVERAL MessageBox servers
// because finding out which server a wallet reads is not reliable yet: few wallets publish an
// `ls_messagebox` advertisement (BSV Browser cannot; BSV Desktop only on request), the lookup hosts
// disagree (2026-10-14: Babbage's had none of the owner's three), and the servers are separate
// mailboxes. So the outbox remembers which servers took each message and re-posts the SAME message
// (same id) only to the ones that did not. Once discovery matures — most recipients advertised,
// our own advertisement published (B5-T5-P7 Part B) — most sends go to one host and this per-host
// record can be simplified.

/// What became of one message posted to its targets. `accepted` includes targets that took it on an
/// earlier attempt.
#[derive(Debug)]
pub struct Delivery {
    /// The lookup failed: the known hosts were posted to, and the outbox looks up again later
    /// (review 178-2 #4) — the recipient may advertise a host outside our list.
    pub lookup_failed: bool,
    /// Every advertised host refused it for good and the fallback to the known hosts was not run here
    /// (the send path's 30 s budget, review 178-3 #5): the outbox runs it.
    pub fallback_due: bool,
    pub accepted: Vec<String>,
    pub failed: Vec<(String, MessageBoxError)>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DeliveryVerdict {
    /// Every target took it — or some did and the rest refused it for good.
    Delivered,
    /// No target took it, and every one refused it for good.
    Undeliverable,
    /// A target failed in a way a retry can fix.
    Retry,
}

impl Delivery {
    pub fn verdict(&self) -> DeliveryVerdict {
        // Review 178-4 #4: a failed lookup is retried even when every known host refused — the recipient
        // may advertise a host outside our list that would take it.
        if self.fallback_due || self.lookup_failed || self.failed.iter().any(|(_, e)| !e.is_permanent()) {
            DeliveryVerdict::Retry
        } else if self.accepted.is_empty() {
            DeliveryVerdict::Undeliverable
        } else {
            DeliveryVerdict::Delivered
        }
    }

    /// One line for logs and the outbox: who took it, who did not and why.
    pub fn summary(&self) -> String {
        let failed: Vec<String> = self.failed.iter().map(|(h, e)| format!("{}: {}", h, e)).collect();
        format!("accepted by [{}]; failed [{}]{}", self.accepted.join(", "), failed.join("; "),
            if self.lookup_failed { "; lookup failed — will look up again" } else { "" })
    }
}

/// How long one host gets to take a message. Delivery runs after the broadcast, inside the wallet
/// panel's 30 s call (`HttpRequestInterceptor.cpp`, SyncHttpClient 30 000 ms): a call cut off there
/// reads as a failed send although the money moved, and invites a second payment (review 178 #2).
/// Hosts are posted to at once, so lookup (4 s) + this stays well inside it. A host that has not
/// answered is a transient failure: the outbox retries it.
const POST_BUDGET: std::time::Duration = std::time::Duration::from_secs(12);
/// On the send path itself (after the broadcast, inside the panel's 30 s), a host gets less: one that
/// has not answered by then is left to the outbox (review 178-5 #4). Measured answers: 0.4–1.2 s.
const SEND_PATH_POST_BUDGET: std::time::Duration = std::time::Duration::from_secs(8);

/// `deliver` after the lookup, with the network seam (`post`) exposed for tests: targets the hosts
/// already holding the message are skipped and counted as accepted.
#[cfg(test)]
pub(crate) async fn deliver_prepared<F, Fut>(
    prepared: std::sync::Arc<PreparedMessage>,
    lookup: &crate::messagebox_lookup::AdvertLookup,
    already: &[String],
    post: F,
) -> Delivery
where
    F: Fn(String, std::sync::Arc<PreparedMessage>) -> Fut,
    Fut: std::future::Future<Output = Result<(), MessageBoxError>>,
{
    deliver_prepared_with(prepared, lookup, already, Fallback::OnRefusal, post).await
}

/// `deliver_prepared`, with the fallback to the known hosts run here or left to the outbox.
pub(crate) async fn deliver_prepared_with<F, Fut>(
    prepared: std::sync::Arc<PreparedMessage>,
    lookup: &crate::messagebox_lookup::AdvertLookup,
    already: &[String],
    fallback: Fallback,
    post: F,
) -> Delivery
where
    F: Fn(String, std::sync::Arc<PreparedMessage>) -> Fut,
    Fut: std::future::Future<Output = Result<(), MessageBoxError>>,
{
    // Review 178-3 #2: every host that took it on an earlier attempt counts, whether or not this
    // attempt's lookup names it.
    let todo: Vec<String> = delivery_targets(lookup).into_iter().filter(|h| !already.contains(h)).collect();
    info!("MessageBox: delivering {} — lookup {:?} ⇒ post to {:?} (already taken by {:?})",
        &prepared.message_id[..16], lookup, todo, already);
    let budget = if fallback == Fallback::Defer { SEND_PATH_POST_BUDGET } else { POST_BUDGET };
    let mut d = deliver_within(todo, prepared.clone(), &post, budget).await;
    d.accepted.splice(0..0, already.iter().cloned());
    d.lookup_failed = matches!(lookup, crate::messagebox_lookup::AdvertLookup::Failed(_));
    // Owner, 2026-10-15 (review 178-2 #5): when every host the recipient advertises refuses the message
    // for good, it goes to the known hosts too — a stale advertisement must not strand the payment.
    // A host that is only down is not refused for good: it is retried, with no fallback.
    let advertised = matches!(lookup, crate::messagebox_lookup::AdvertLookup::Advertised(_));
    let all_failed = d.accepted.is_empty() && !d.failed.is_empty();
    let for_good = d.failed.iter().all(|(_, e)| e.is_permanent());
    if advertised && all_failed && (for_good || fallback == Fallback::OnAnyFailure) {
        let known: Vec<String> = MESSAGEBOX_HOSTS.iter().map(|h| h.to_string())
            .filter(|h| !already.contains(h) && !d.failed.iter().any(|(f, _)| f == h))
            .collect();
        if fallback != Fallback::Defer {
            warn!("MessageBox: every advertised host refused {} for good — falling back to the known hosts {:?}", &prepared.message_id[..16], known);
            let mut k = deliver_to(known, prepared, &post).await;
            d.accepted.append(&mut k.accepted);
            d.failed.append(&mut k.failed);
        } else {
            warn!("MessageBox: every advertised host refused {} for good — the outbox falls back to the known hosts", &prepared.message_id[..16]);
            d.fallback_due = true;
        }
    }
    d
}

/// Post one prepared message to every target, all at once. `post` is the network seam (tests record
/// each host and the message id it was given).
pub(crate) async fn deliver_to<F, Fut>(targets: Vec<String>, prepared: std::sync::Arc<PreparedMessage>, post: &F) -> Delivery
where
    F: Fn(String, std::sync::Arc<PreparedMessage>) -> Fut,
    Fut: std::future::Future<Output = Result<(), MessageBoxError>>,
{
    deliver_within(targets, prepared, post, POST_BUDGET).await
}

/// `deliver_to` with an explicit per-host budget.
async fn deliver_within<F, Fut>(targets: Vec<String>, prepared: std::sync::Arc<PreparedMessage>, post: &F, budget: std::time::Duration) -> Delivery
where
    F: Fn(String, std::sync::Arc<PreparedMessage>) -> Fut,
    Fut: std::future::Future<Output = Result<(), MessageBoxError>>,
{
    let attempts = targets.iter().map(|host| {
        let attempt = post(host.clone(), prepared.clone());
        async move {
            match tokio::time::timeout(budget, attempt).await {
                Ok(r) => r,
                Err(_) => Err(MessageBoxError::Api(format!("no answer within {} s", budget.as_secs()))),
            }
        }
    });
    let results = futures_util::future::join_all(attempts).await;
    let mut accepted = Vec::new();
    let mut failed = Vec::new();
    for (host, result) in targets.into_iter().zip(results) {
        match result {
            Ok(()) => accepted.push(host),
            Err(e) => {
                warn!("MessageBox: delivery to {} failed: {}", host, e);
                failed.push((host, e));
            }
        }
    }
    Delivery { lookup_failed: false, fallback_due: false, accepted, failed }
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
/// (`main.rs :: enforce_dev_safeguard`), so the override cannot reach users.
pub const MESSAGEBOX_MAX_BODY_BYTES: usize = 1_048_576;

/// How long one `listMessages` page may take (B5-T5-P7a, review 178-2 #6/#7).
const PAGE_BUDGET: std::time::Duration = std::time::Duration::from_secs(20);

/// Upper bound on `listMessages` pages read in one poll (1000 messages per page).
const LIST_MESSAGES_MAX_PAGES: usize = 20;

/// Read every `listMessages` page. MessageBox pages its inbox (limit 1000, `hasMore` /
/// `nextOffset`; live shape recorded 2026-09-30, B6-P1-A6). Follows the pages as
/// `@bsv/message-box-client` `fetchMessagePages` does, bounded to `LIST_MESSAGES_MAX_PAGES`
/// so a runaway server cannot loop us forever. `fetch_page(offset)` returns one page's JSON.
async fn collect_message_pages<F, Fut>(mut fetch_page: F) -> Result<Vec<serde_json::Value>, MessageBoxError>
where
    F: FnMut(u64) -> Fut,
    Fut: std::future::Future<Output = Result<serde_json::Value, MessageBoxError>>,
{
    let mut messages = Vec::new();
    let mut offset: u64 = 0;
    for page in 0..LIST_MESSAGES_MAX_PAGES {
        let json = fetch_page(offset).await?;
        let (page_messages, next) = parse_list_messages_page(&json, offset)?;
        messages.extend(page_messages);
        match next {
            Some(n) => offset = n,
            None => return Ok(messages),
        }
        if page + 1 == LIST_MESSAGES_MAX_PAGES {
            // Unlike the official client (which throws), return what we have: every message
            // we process is acknowledged and deleted, so the next poll starts further on.
            // Messages we cannot process stay, and could hold later ones back — hence the warn.
            warn!("MessageBox: listMessages stopped after {} pages with more pending", LIST_MESSAGES_MAX_PAGES);
        }
    }
    Ok(messages)
}

/// Parse one `listMessages` page. Returns the page's messages and, when the server says more
/// remain, the offset of the next page (`nextOffset` if it advances; else offset + count;
/// else offset + the server's `limit` — the rule in `@bsv/message-box-client`
/// `nextMessagePageOffset`).
///
/// ⛔ An error body or a shape without a `messages` array is an **error**, never "no messages":
/// reading it as an empty inbox is what would silently hide incoming payments.
/// A bare top-level array (the pre-paging shape) is still accepted as a single page.
fn parse_list_messages_page(
    json: &serde_json::Value,
    offset: u64,
) -> Result<(Vec<serde_json::Value>, Option<u64>), MessageBoxError> {
    if let Some(arr) = json.as_array() {
        return Ok((arr.clone(), None));
    }
    if json.get("status").and_then(|s| s.as_str()) == Some("error") {
        return Err(MessageBoxError::Api(format!("listMessages error: {}", json)));
    }
    let messages = json.get("messages").and_then(|v| v.as_array()).cloned().ok_or_else(|| {
        MessageBoxError::Api(format!("listMessages returned an unexpected shape: {:.200}", json.to_string()))
    })?;
    if json.get("hasMore").and_then(|v| v.as_bool()) != Some(true) {
        return Ok((messages, None));
    }
    let next = match json.get("nextOffset").and_then(|v| v.as_u64()) {
        Some(n) if n > offset => n,
        _ if !messages.is_empty() => offset + messages.len() as u64,
        _ => match json.get("limit").and_then(|v| v.as_u64()) {
            Some(l) if l > 0 => offset + l,
            _ => offset + 1000,
        },
    };
    Ok((messages, Some(next)))
}

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
    /// emits, for the real envelope `prepare_message` builds.
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

/// Exact size of the WHOLE request body `prepare_message` builds, for a plaintext of
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

/// B5-T1-P7b: a listed message whose `body` is not a JSON string is not skipped — a skipped
/// message is never acknowledged and parks in the inbox for good. Its JSON text goes to the poller,
/// which holds what it cannot read.
pub(crate) fn non_string_body(msg: &serde_json::Value) -> Vec<u8> {
    msg["body"].to_string().into_bytes()
}

/// Decrypt a listed message body with our private key (BRC-2, recipient side). Shared by
/// `list_messages` and the PeerPay re-check of a stored body (B5-T1-P7b review 164 #11: no HTTP client).
pub(crate) fn decrypt_body(our_private_key: &[u8], sender_hex: &str, body_str: &str) -> Result<Vec<u8>, MessageBoxError> {
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
        our_private_key,
        &sender_pubkey,
        MESSAGEBOX_INVOICE,
    ).map_err(|e| MessageBoxError::Decryption(format!("{}", e)))?;

    brc2::decrypt_brc2(&encrypted_bytes, &symmetric_key)
        .map_err(|e| MessageBoxError::Decryption(format!("{}", e)))
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

    /// Encrypt a message for a recipient once, so the same bytes — and the same message id — go to
    /// every host it is posted to (B5-T5-P7a: a server refuses a repeated id, and a reader on two
    /// hosts sees one message id, not two).
    ///
    /// # Arguments
    /// * `recipient_pubkey` - Recipient's 33-byte compressed public key
    /// * `message_box` - Name of the message box (e.g., "payment_inbox")
    /// * `plaintext` - Message body bytes (will be BRC-2 encrypted)
    pub fn prepare_message(
        &self,
        recipient_pubkey: &[u8],
        message_box: &str,
        plaintext: &[u8],
    ) -> Result<PreparedMessage, MessageBoxError> {
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

        Ok(PreparedMessage { message_id, request_body: serde_json::to_vec(&request_body)? })
    }

    /// Post a prepared message to one MessageBox host.
    pub async fn post_message(&self, host: &str, prepared: &PreparedMessage) -> Result<(), MessageBoxError> {
        let url = format!("{}/sendMessage", host);
        debug!("MessageBox: sending {} to {}", &prepared.message_id[..16], host);

        let response = self.auth_client.fetch("POST", &url, Some(&prepared.request_body)).await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            // Review 178-5 #10: an answer whose body cannot be read is not a refusal — it may have
            // said ERR_DUPLICATE_MESSAGE. Retried.
            // Review 178-6 #1: bounded — the host may be one a recipient named.
            let body = crate::authfetch::read_capped(response, 64 * 1024).await
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .map_err(|e| MessageBoxError::Api(format!("sendMessage failed ({}), answer unreadable: {}", status, e)))?;
            classify_send_failure(status, body)?;
            info!("MessageBox: {} already holds message {} — delivered", host, &prepared.message_id[..16]);
            return Ok(());
        }

        info!("MessageBox: message sent to {} (id={})", host, &prepared.message_id[..16]);
        Ok(())
    }

    /// B5-T5-P7a — deliver a message to the recipient: look up where they read, then post `prepared`
    /// to every target that has not taken it yet (`already`, from the outbox on a retry).
    pub async fn deliver(
        &self,
        recipient_pubkey: &[u8],
        prepared: std::sync::Arc<PreparedMessage>,
        already: &[String],
        fallback: Fallback,
    ) -> Delivery {
        let lookup = crate::messagebox_lookup::lookup(&hex::encode(recipient_pubkey)).await;
        deliver_prepared_with(prepared, &lookup, already, fallback, |host, prepared| async move {
            self.post_message(&host, &prepared).await
        }).await
    }

    /// List messages from a message box (decrypts BRC-2 encrypted messages)
    ///
    /// # Arguments
    /// * `host` - The MessageBox host to read
    /// * `message_box` - Name of the message box (e.g., "payment_inbox")
    ///
    /// # Returns
    /// Vec of decrypted incoming messages
    pub async fn list_messages(&self, host: &str, message_box: &str) -> Result<Vec<IncomingMessage>, MessageBoxError> {
        let url = format!("{}/listMessages", host);

        let messages = collect_message_pages(|offset| {
            let url = url.clone();
            async move {
                let body_bytes = serde_json::to_vec(&serde_json::json!({
                    "messageBox": message_box,
                    "offset": offset
                }))?;
                // B5-T5-P7a: each page — request AND body — within its own budget (several hosts are
                // read every poll; review 178-3 #8: a host that drips the body is cut here too).
                let page = async {
                    let response = self.auth_client.fetch("POST", &url, Some(&body_bytes)).await?;
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    Ok::<_, MessageBoxError>((status, text))
                };
                let (status, response_text) = tokio::time::timeout(PAGE_BUDGET, page).await
                    .map_err(|_| MessageBoxError::Api(format!("listMessages: no answer within {} s", PAGE_BUDGET.as_secs())))??;

                if !status.is_success() {
                    return Err(MessageBoxError::Api(format!("listMessages failed ({}): {}", status.as_u16(), response_text)));
                }

                // Char-safe preview: a byte slice at 500 panics when a multibyte character
                // straddles it, and message bodies are sender-controlled.
                let preview: String = response_text.chars().take(500).collect();
                info!("MessageBox: listMessages raw response ({} chars): {}", response_text.len(), preview);
                Ok(serde_json::from_str(&response_text)?)
            }
        }).await?;

        let mut result = Vec::new();

        for msg in messages {
            let message_id = msg["messageId"].as_str().unwrap_or("").to_string();
            let sender = msg["sender"].as_str().unwrap_or("").to_string();
            // B5-T1-P7b (V10): a body that is not a JSON string is tried as its JSON text — the same
            // decryption, then the plaintext fallback — rather than handed on undecrypted.
            let object_body = msg["body"].as_str().is_none().then(|| String::from_utf8_lossy(&non_string_body(&msg)).into_owned());
            let body_str = msg["body"].as_str().or(object_body.as_deref()).unwrap_or("");

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

    /// Acknowledge (delete) messages from one server — the host they were listed from.
    ///
    /// # Arguments
    /// * `host` - The MessageBox host the messages were listed from
    /// * `message_ids` - List of message IDs to acknowledge
    pub async fn acknowledge(&self, host: &str, message_ids: &[String]) -> Result<(), MessageBoxError> {
        if message_ids.is_empty() {
            return Ok(());
        }

        let request_body = serde_json::json!({
            "messageIds": message_ids
        });

        let body_bytes = serde_json::to_vec(&request_body)?;
        let url = format!("{}/acknowledgeMessage", host);

        let response = self.auth_client.fetch("POST", &url, Some(&body_bytes)).await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(MessageBoxError::Api(format!("acknowledgeMessage failed ({}): {}", status, body)));
        }

        info!("MessageBox: acknowledged {} message(s) on {}", message_ids.len(), host);
        Ok(())
    }

    /// BRC-2 encrypt plaintext for a recipient
    ///
    /// Protocol: [1, "messagebox"], keyID: "1"
    /// Invoice number: "1-messagebox-1"
    pub(crate) fn encrypt_for_recipient(&self, recipient_pubkey: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, MessageBoxError> {
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
    pub(crate) fn decrypt_from_sender(&self, sender_hex: &str, body_str: &str) -> Result<Vec<u8>, MessageBoxError> {
        decrypt_body(&self.our_private_key, sender_hex, body_str)
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

    // ── B6-P1-A6: listMessages paging ──────────────────────────────────────────────────
    /// Verbatim from the dev wallet's first post-fix poll, 2026-09-30 14:44:36 -06:00.
    const LIVE_EMPTY_PAGE: &str = r#"{"status":"success","messages":[],"limit":1000,"offset":0,"nextOffset":0,"hasMore":false}"#;

    fn page(messages: usize, offset: u64, next: Option<u64>, has_more: bool) -> serde_json::Value {
        let msgs: Vec<_> = (0..messages).map(|i| serde_json::json!({"messageId": format!("m{}", offset as usize + i), "sender": "02", "body": "x"})).collect();
        let mut v = serde_json::json!({"status":"success","messages":msgs,"limit":1000,"offset":offset,"hasMore":has_more});
        if let Some(n) = next { v["nextOffset"] = n.into(); }
        v
    }

    #[test]
    fn p1_a6_live_empty_page_is_empty_and_final() {
        let (msgs, next) = parse_list_messages_page(&serde_json::from_str(LIVE_EMPTY_PAGE).unwrap(), 0).unwrap();
        assert!(msgs.is_empty());
        assert_eq!(next, None);
    }

    #[test]
    fn p1_a6_has_more_yields_every_message_and_the_next_offset() {
        let (msgs, next) = parse_list_messages_page(&page(3, 0, Some(1000), true), 0).unwrap();
        assert_eq!(msgs.len(), 3, "page messages must all be returned");
        assert_eq!(next, Some(1000), "nextOffset must be followed");
        // nextOffset missing or not advancing ⇒ offset + count, then offset + limit
        assert_eq!(parse_list_messages_page(&page(3, 1000, None, true), 1000).unwrap().1, Some(1003));
        assert_eq!(parse_list_messages_page(&page(3, 1000, Some(1000), true), 1000).unwrap().1, Some(1003));
        assert_eq!(parse_list_messages_page(&page(0, 1000, None, true), 1000).unwrap().1, Some(2000));
        // last page
        assert_eq!(parse_list_messages_page(&page(2, 1000, Some(1002), false), 1000).unwrap(), (page(2, 1000, None, false)["messages"].as_array().unwrap().clone(), None));
    }

    /// The loop, not just the parser: three pages, each message exactly once, offsets advance.
    #[tokio::test]
    async fn p1_a6_collects_every_page_once_with_advancing_offsets() {
        let requested = std::sync::Mutex::new(Vec::new());
        let msgs = collect_message_pages(|offset| {
            requested.lock().unwrap().push(offset);
            let json = match offset {
                0 => page(1000, 0, Some(1000), true),
                1000 => page(1000, 1000, Some(2000), true),
                2000 => page(7, 2000, Some(2007), false),
                other => panic!("unexpected offset {}", other),
            };
            async move { Ok(json) }
        }).await.unwrap();
        assert_eq!(*requested.lock().unwrap(), vec![0, 1000, 2000], "offsets must advance page by page");
        let ids: std::collections::HashSet<_> = msgs.iter().map(|m| m["messageId"].as_str().unwrap().to_string()).collect();
        assert_eq!((msgs.len(), ids.len()), (2007, 2007), "every message exactly once");
    }

    /// A server that always says `hasMore` cannot loop us forever.
    #[tokio::test]
    async fn p1_a6_endless_has_more_stops_at_the_page_cap() {
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let msgs = collect_message_pages(|offset| {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async move { Ok(page(1, offset, None, true)) }
        }).await.unwrap();
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), LIST_MESSAGES_MAX_PAGES);
        assert_eq!(msgs.len(), LIST_MESSAGES_MAX_PAGES);
    }

    #[test]
    fn p1_a6_error_or_unknown_shape_is_an_error_not_an_empty_inbox() {
        let err = parse_list_messages_page(&serde_json::json!({"status":"error","code":"ERR_X","description":"nope"}), 0);
        assert!(err.is_err(), "an error body must not read as 'no messages'");
        let odd = parse_list_messages_page(&serde_json::json!({"status":"success","items":[{"messageId":"a"}]}), 0);
        assert!(odd.is_err(), "an unknown shape must not read as 'no messages'");
        // pre-paging shape still accepted
        let (msgs, next) = parse_list_messages_page(&serde_json::json!([{"messageId":"a"}]), 0).unwrap();
        assert_eq!((msgs.len(), next), (1, None));
    }

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

    /// B5-T5-P7a, live and read-only: every known host accepts our handshake and lists an inbox, for
    /// a throwaway key (nothing is sent). Network; run by hand.
    #[tokio::test]
    #[ignore]
    async fn live_every_known_host_lists_an_inbox() {
        let secp = Secp256k1::new();
        let (sk, pk) = secp.generate_keypair(&mut secp256k1::rand::thread_rng());
        let client = MessageBoxClient::new(sk.secret_bytes().to_vec(), pk.serialize().to_vec());
        for host in MESSAGEBOX_HOSTS {
            let listed = client.list_messages(host, "payment_inbox").await;
            assert!(listed.is_ok(), "{}: {:?}", host, listed.err());
            assert!(listed.unwrap().is_empty(), "{}: a fresh key has no mail", host);
        }
        // Control: a host that is not a MessageBox server must fail, or the loop above proves nothing.
        assert!(client.list_messages("https://overlay-us-1.bsvb.tech", "payment_inbox").await.is_err());
    }

    // ── B5-T5-P7a: where a PeerPay message goes ─────────────────────────────────────────────
    use crate::messagebox_lookup::AdvertLookup;

    fn all_known() -> Vec<String> { MESSAGEBOX_HOSTS.iter().map(|h| h.to_string()).collect() }

    /// `T5P7a-A5` — an advertised host gets the message, and only it; no advertisement, or a lookup
    /// that failed, sends to every known host.
    #[test]
    fn t5p7a_a5_targets_are_the_advertised_host_else_every_known_host() {
        let x = "https://mb.elsewhere.org".to_string();
        assert_eq!(delivery_targets(&AdvertLookup::Advertised(vec![x.clone()])), vec![x]);
        assert_eq!(delivery_targets(&AdvertLookup::NoAdvertisement), all_known());
        assert_eq!(delivery_targets(&AdvertLookup::Failed("down".into())), all_known());
        assert!(all_known().len() >= 2, "the fallback must reach more than our own server");
    }

    /// `T5P7a-A6` — `deliver` posts ONE encrypted message to every target, once each, in order: the
    /// same message id on every host. No advertisement ⇒ every known host.
    #[tokio::test]
    async fn t5p7a_a6_deliver_posts_one_message_id_to_every_target() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let client = MessageBoxClient::new(priv1, pub1);
        let posted = std::sync::Mutex::new(Vec::new());
        let prepared = std::sync::Arc::new(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap());
        let d = deliver_prepared(prepared, &AdvertLookup::NoAdvertisement, &[], |host, p| {
            posted.lock().unwrap().push((host, p.message_id.clone()));
            async { Ok(()) }
        }).await;
        let posted = posted.into_inner().unwrap();
        assert_eq!(posted.iter().map(|(h, _)| h.clone()).collect::<Vec<_>>(), all_known());
        assert!(posted.windows(2).all(|w| w[0].1 == w[1].1), "every host must get the same message id: {:?}", posted);
        assert_eq!((d.accepted.len(), d.failed.len(), d.verdict()), (all_known().len(), 0, DeliveryVerdict::Delivered));
    }

    /// `T5P7a-A11` (review 178 #2) — hosts are posted to at once: the first host's post finishes only
    /// after the second host's has started. Posted one after another, the first would wait out its
    /// whole budget and fail.
    #[tokio::test]
    async fn t5p7a_a11_hosts_are_posted_to_at_once() {
        let started = std::sync::Arc::new(tokio::sync::Notify::new());
        let prepared = std::sync::Arc::new(PreparedMessage { message_id: "a".repeat(64), request_body: vec![] });
        let t0 = std::time::Instant::now();
        let d = deliver_to(vec!["first".into(), "second".into()], prepared, &|host: String, _| {
            let started = started.clone();
            async move {
                if host == "first" { started.notified().await } else { started.notify_one() }
                Ok(())
            }
        }).await;
        assert_eq!((d.accepted.len(), d.failed.len()), (2, 0), "{}", d.summary());
        assert!(t0.elapsed() < std::time::Duration::from_secs(5));
    }

    /// `T5P7a-A11` — a host that does not answer within the budget is a transient failure (retried),
    /// never a hang and never a refusal.
    #[tokio::test]
    async fn t5p7a_a11_a_silent_host_is_transient_after_the_budget() {
        let prepared = std::sync::Arc::new(PreparedMessage { message_id: "a".repeat(64), request_body: vec![] });
        let d = deliver_to(vec!["silent".into(), "ok".into()], prepared, &|host: String, _| async move {
            if host == "silent" { std::future::pending::<()>().await }
            Ok(())
        }).await;
        assert_eq!(d.accepted, vec!["ok".to_string()]);
        assert_eq!(d.failed.len(), 1);
        assert!(!d.failed[0].1.is_permanent());
        assert_eq!(d.verdict(), DeliveryVerdict::Retry);
    }

    /// `T5P7a-A14` (review 178 #1) — a retry posts only to the hosts that have not taken the message,
    /// and counts the others as delivered: one copy per server, and no false "not told".
    #[tokio::test]
    async fn t5p7a_a14_a_retry_posts_only_to_hosts_that_missed_it() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let client = MessageBoxClient::new(priv1, pub1);
        let prepared = std::sync::Arc::new(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap());
        let already: Vec<String> = all_known()[..2].to_vec();
        let posted = std::sync::Mutex::new(Vec::new());
        let d = deliver_prepared(prepared, &AdvertLookup::NoAdvertisement, &already, |host, _| {
            posted.lock().unwrap().push(host);
            async { Ok(()) }
        }).await;
        assert_eq!(posted.into_inner().unwrap(), all_known()[2..].to_vec(), "only the hosts that missed it");
        assert_eq!(d.accepted.len(), all_known().len());
        assert_eq!(d.verdict(), DeliveryVerdict::Delivered);

        // Every target already holds it: nothing is posted, and it is delivered.
        let prepared = std::sync::Arc::new(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap());
        let d = deliver_prepared(prepared, &AdvertLookup::NoAdvertisement, &all_known(), |_, _| async { panic!("nothing to post") }).await;
        assert_eq!(d.verdict(), DeliveryVerdict::Delivered);
    }

    /// `T5P7a-A14` — a stored request is re-posted as is: same bytes, same message id.
    #[test]
    fn t5p7a_a14_a_stored_request_keeps_its_message_id() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let p = MessageBoxClient::new(priv1, pub1).prepare_message(&pub2, "payment_inbox", b"pay").unwrap();
        let back = PreparedMessage::from_request(p.request_body().to_vec()).unwrap();
        assert_eq!((back.message_id.as_str(), back.request_body()), (p.message_id.as_str(), p.request_body()));
        assert!(PreparedMessage::from_request(b"{}".to_vec()).is_err());
    }

    /// `T5P7a-A17` (review 178-2 #4) — a lookup that failed posts to every known host AND is retried, so
    /// the outbox looks up again (the recipient may advertise a host outside our list); the hosts that
    /// took it are not posted to again.
    #[tokio::test]
    async fn t5p7a_a17_a_failed_lookup_is_looked_up_again() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let client = MessageBoxClient::new(priv1, pub1);
        let prepared = std::sync::Arc::new(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap());
        let d = deliver_prepared(prepared.clone(), &AdvertLookup::Failed("down".into()), &[], |_, _| async { Ok(()) }).await;
        assert_eq!((d.accepted.len(), d.verdict()), (all_known().len(), DeliveryVerdict::Retry));
        // The next attempt's lookup answers: a host outside our list gets it, the known ones are not re-posted.
        let x = "https://mb.elsewhere.org".to_string();
        let posted = std::sync::Mutex::new(Vec::new());
        let d = deliver_prepared(prepared, &AdvertLookup::Advertised(vec![x.clone()]), &all_known(), |h, _| {
            posted.lock().unwrap().push(h);
            async { Ok(()) }
        }).await;
        assert_eq!(posted.into_inner().unwrap(), vec![x]);
        assert_eq!(d.verdict(), DeliveryVerdict::Delivered);
    }

    /// `T5P7a-A19` (owner 2026-10-15, review 178-2 #5) — every advertised host refusing the message for
    /// good ⇒ it goes to the known hosts too, and is delivered. An advertised host that is only down
    /// gets no fallback (retried). A known host that already took it is not posted to again.
    #[tokio::test]
    async fn t5p7a_a19_a_refusing_advertised_host_falls_back_to_the_known_hosts() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let client = MessageBoxClient::new(priv1, pub1);
        let x = "https://mb.stale.org".to_string();
        let lookup = AdvertLookup::Advertised(vec![x.clone()]);
        let posted = std::sync::Mutex::new(Vec::new());
        let prepared = std::sync::Arc::new(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap());
        let d = deliver_prepared(prepared.clone(), &lookup, &[], |h, _| {
            posted.lock().unwrap().push(h.clone());
            async move { if h == "https://mb.stale.org" { Err(MessageBoxError::Rejected { status: 404, body: "gone".into() }) } else { Ok(()) } }
        }).await;
        let mut expect = vec![x.clone()];
        expect.extend(all_known());
        assert_eq!(posted.into_inner().unwrap(), expect);
        assert_eq!(d.verdict(), DeliveryVerdict::Delivered);

        // Only down (transient): no fallback, retried.
        let posted = std::sync::Mutex::new(Vec::new());
        let d = deliver_prepared(prepared.clone(), &lookup, &[], |h, _| {
            posted.lock().unwrap().push(h);
            async { Err(MessageBoxError::Api("503".into())) }
        }).await;
        assert_eq!((posted.into_inner().unwrap(), d.verdict()), (vec![x.clone()], DeliveryVerdict::Retry));

        // A known host that already took it (an earlier fallback) is not posted to again.
        let posted = std::sync::Mutex::new(Vec::new());
        let taken = vec![all_known()[0].clone()];
        deliver_prepared(prepared, &lookup, &taken, |h, _| {
            posted.lock().unwrap().push(h.clone());
            async move { if h == "https://mb.stale.org" { Err(MessageBoxError::Rejected { status: 404, body: "gone".into() }) } else { Ok(()) } }
        }).await;
        assert!(!posted.into_inner().unwrap().contains(&all_known()[0]));
    }

    /// `T5P7a-A22` (review 178-3 #2) — hosts that took it on an earlier attempt count even when this
    /// attempt's lookup names another host that refuses: delivered, not "undeliverable".
    #[tokio::test]
    async fn t5p7a_a22_earlier_hosts_count_outside_this_attempts_targets() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let client = MessageBoxClient::new(priv1, pub1);
        let prepared = std::sync::Arc::new(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap());
        let lookup = AdvertLookup::Advertised(vec!["https://mb.new.org".into()]);
        let d = deliver_prepared(prepared, &lookup, &all_known(), |_, _| async {
            Err(MessageBoxError::Rejected { status: 404, body: "no".into() })
        }).await;
        assert_eq!(d.verdict(), DeliveryVerdict::Delivered, "{}", d.summary());
    }

    /// `T5P7a-A23` (review 178-3 #5) — on the send path (inside the panel's 30 s) the fallback is not
    /// run: it is left to the outbox (Retry), and no known host is posted to.
    #[tokio::test]
    async fn t5p7a_a23_the_send_path_leaves_the_fallback_to_the_outbox() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let client = MessageBoxClient::new(priv1, pub1);
        let prepared = std::sync::Arc::new(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap());
        let posted = std::sync::Mutex::new(Vec::new());
        let d = deliver_prepared_with(prepared, &AdvertLookup::Advertised(vec!["https://mb.stale.org".into()]), &[], Fallback::Defer, |h, _| {
            posted.lock().unwrap().push(h);
            async { Err(MessageBoxError::Rejected { status: 404, body: "gone".into() }) }
        }).await;
        assert_eq!(posted.into_inner().unwrap(), vec!["https://mb.stale.org".to_string()]);
        assert_eq!(d.verdict(), DeliveryVerdict::Retry);
    }

    /// `T5P7a-A26` (review 178-4 #1) — on a later outbox attempt, an advertised host that is GONE (any
    /// failure, not a refusal) falls back to the known hosts; on the first attempts it is retried.
    #[tokio::test]
    async fn t5p7a_a26_a_gone_advertised_host_falls_back_on_later_attempts() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let client = MessageBoxClient::new(priv1, pub1);
        let prepared = std::sync::Arc::new(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap());
        let lookup = AdvertLookup::Advertised(vec!["https://mb.gone.org".into()]);
        let gone = |h: String, _| async move {
            if h == "https://mb.gone.org" { Err(MessageBoxError::Api("dns error".into())) } else { Ok(()) }
        };
        let d = deliver_prepared_with(prepared.clone(), &lookup, &[], Fallback::OnRefusal, gone).await;
        assert_eq!((d.accepted.len(), d.verdict()), (0, DeliveryVerdict::Retry), "early attempt: retried, no fallback");
        let d = deliver_prepared_with(prepared, &lookup, &[], Fallback::OnAnyFailure, gone).await;
        assert_eq!(d.accepted, all_known(), "later attempt: the known hosts take it");
    }

    /// `T5P7a-A27` (review 178-4 #4, #7) — a failed lookup is retried even when every known host
    /// refused; at most 3 advertised hosts are posted to.
    #[tokio::test]
    async fn t5p7a_a27_failed_lookup_refused_everywhere_is_retried_and_targets_are_capped() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let client = MessageBoxClient::new(priv1, pub1);
        let prepared = std::sync::Arc::new(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap());
        let d = deliver_prepared(prepared, &AdvertLookup::Failed("down".into()), &[], |_, _| async {
            Err(MessageBoxError::Rejected { status: 400, body: "no".into() })
        }).await;
        assert_eq!(d.verdict(), DeliveryVerdict::Retry);
        let many: Vec<String> = (0..10).map(|i| format!("https://mb{}.example.org", i)).collect();
        assert_eq!(delivery_targets(&AdvertLookup::Advertised(many.clone())), many[..3].to_vec());
    }

    /// `T5P7a-A28` (review 178-5 #12) — a stored request whose id is not hex is refused, not sliced.
    #[test]
    fn t5p7a_a28_a_stored_request_id_must_be_hex() {
        let req = |id: &str| serde_json::to_vec(&serde_json::json!({"message": {"messageId": id}})).unwrap();
        assert!(PreparedMessage::from_request(req(&"ab".repeat(32))).is_ok());
        assert!(PreparedMessage::from_request(req("ééééééééééééééééé")).is_err());
    }

    /// `T5P7a-A29` (review 178-5 #1) — the MessageBox client never follows a redirect: a host that
    /// answers 307 to another address gets no second request there.
    #[tokio::test]
    async fn t5p7a_a29_messagebox_requests_never_follow_a_redirect() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = target.local_addr().unwrap();
        let reached = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let r = reached.clone();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = target.accept().await.unwrap();
                r.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").await;
            }
        });
        let evil = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let evil_addr = evil.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = evil.accept().await.unwrap();
                let mut buf = vec![0u8; 8192];
                let _ = s.read(&mut buf).await;
                let resp = format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{}/wallet/anything\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", target_addr);
                let _ = s.write_all(resp.as_bytes()).await;
            }
        });
        let (priv1, pub1) = test_keypair();
        let client = MessageBoxClient::new(priv1, pub1);
        assert!(client.list_messages(&format!("http://{}", evil_addr), "payment_inbox").await.is_err());
        let (_, pub2) = test_keypair_2();
        let p = client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap();
        assert!(client.post_message(&format!("http://{}", evil_addr), &p).await.is_err());
        assert_eq!(reached.load(std::sync::atomic::Ordering::SeqCst), 0, "a redirect must not be followed");
    }

    /// `T5P7a-A31` (review 178-5 #4) — on the send path a silent host is given up after 8 s (left to the
    /// outbox), so lookup + posting stays well inside the panel's 30 s after the broadcast.
    #[tokio::test]
    async fn t5p7a_a31_the_send_path_gives_a_silent_host_8_seconds() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let client = MessageBoxClient::new(priv1, pub1);
        let prepared = std::sync::Arc::new(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap());
        let t0 = std::time::Instant::now();
        let d = deliver_prepared_with(prepared, &AdvertLookup::NoAdvertisement, &[], Fallback::Defer, |h, _| async move {
            if h == "https://gmb.bsvblockchain.tech" { std::future::pending::<()>().await }
            Ok(())
        }).await;
        assert!(t0.elapsed() < std::time::Duration::from_secs(10), "took {:?}", t0.elapsed());
        assert_eq!(d.verdict(), DeliveryVerdict::Retry);
    }

    /// `T5P7a-A32` (review 178-6 #1) — an answer from a host a recipient named is read only up to a
    /// bound: a handshake answer over 64 KiB is an error, not a growing buffer.
    #[tokio::test]
    async fn t5p7a_a32_a_huge_handshake_answer_is_refused() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let evil = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = evil.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = evil.accept().await.unwrap();
                let mut buf = vec![0u8; 8192];
                let _ = s.read(&mut buf).await;
                let body = vec![b' '; 200 * 1024];
                let head = format!("HTTP/1.1 200 OK
Content-Length: {}
Connection: close

", body.len());
                let _ = s.write_all(head.as_bytes()).await;
                let _ = s.write_all(&body).await;
            }
        });
        let (priv1, pub1) = test_keypair();
        let client = MessageBoxClient::new(priv1, pub1);
        let err = client.list_messages(&format!("http://{}", addr), "payment_inbox").await.unwrap_err().to_string();
        assert!(err.contains("over"), "expected the size bound, got: {}", err);
    }

    /// `T5P7a-A5` — an advertised host is the only one posted to.
    #[tokio::test]
    async fn t5p7a_a5_deliver_posts_only_to_the_advertised_host() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let client = MessageBoxClient::new(priv1, pub1);
        let x = "https://mb.elsewhere.org".to_string();
        let posted = std::sync::Mutex::new(Vec::new());
        let prepared = std::sync::Arc::new(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap());
        deliver_prepared(prepared, &AdvertLookup::Advertised(vec![x.clone()]), &[], |host, _| {
            posted.lock().unwrap().push(host);
            async { Ok(()) }
        }).await;
        assert_eq!(posted.into_inner().unwrap(), vec![x]);
    }

    fn delivery(accepted: &[&str], failed: Vec<(&str, MessageBoxError)>) -> Delivery {
        Delivery {
            lookup_failed: false,
            fallback_due: false,
            accepted: accepted.iter().map(|s| s.to_string()).collect(),
            failed: failed.into_iter().map(|(h, e)| (h.to_string(), e)).collect(),
        }
    }
    fn refused() -> MessageBoxError { MessageBoxError::Rejected { status: 413, body: "too large".into() } }
    fn down() -> MessageBoxError { MessageBoxError::Api("sendMessage failed (503): busy".into()) }

    /// `T5P7a-A6` — a transient failure on any target is retried (that recipient may read only
    /// there); refused everywhere is undeliverable; taken somewhere and refused for good elsewhere
    /// is delivered.
    #[test]
    fn t5p7a_a6_verdicts() {
        assert_eq!(delivery(&["a"], vec![("b", down())]).verdict(), DeliveryVerdict::Retry);
        assert_eq!(delivery(&[], vec![("a", down()), ("b", refused())]).verdict(), DeliveryVerdict::Retry);
        assert_eq!(delivery(&[], vec![("a", refused()), ("b", refused())]).verdict(), DeliveryVerdict::Undeliverable);
        assert_eq!(delivery(&["a"], vec![("b", refused())]).verdict(), DeliveryVerdict::Delivered);
        assert_eq!(delivery(&["a", "b"], vec![]).verdict(), DeliveryVerdict::Delivered);
    }

    /// `T5P7a-A6` — a preparation carries its id in the request; a second preparation is a new id,
    /// which is why `deliver` prepares once (asserted end to end in `t5p7a_a6_deliver_posts_one_message_id_to_every_target`).
    #[test]
    fn t5p7a_a6_one_preparation_one_message_id() {
        let (priv1, pub1) = test_keypair();
        let (_, pub2) = test_keypair_2();
        let client = MessageBoxClient::new(priv1, pub1);
        let p = client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap();
        let body: serde_json::Value = serde_json::from_slice(&p.request_body).unwrap();
        assert_eq!(body["message"]["messageId"], p.message_id.as_str());
        // A second preparation is a different message (new IV ⇒ new id): sending per host would
        // put two ids on the wire; `deliver` prepares once.
        assert_ne!(client.prepare_message(&pub2, "payment_inbox", b"pay").unwrap().message_id, p.message_id);
    }

    /// B5-T5-P7a K1, live (one message between two throwaway keys, no money): is `gmb.bsvblockchain.tech`
    /// the same mailbox as `messagebox.bsvblockchain.tech`? Prints the answer; cleans up after itself.
    #[tokio::test]
    #[ignore]
    async fn live_gmb_and_messagebox_bsvblockchain_share_a_store() {
        let secp = Secp256k1::new();
        let mut rng = secp256k1::rand::thread_rng();
        let (sa, pa) = secp.generate_keypair(&mut rng);
        let (sb, pb) = secp.generate_keypair(&mut rng);
        let a = MessageBoxClient::new(sa.secret_bytes().to_vec(), pa.serialize().to_vec());
        let b = MessageBoxClient::new(sb.secret_bytes().to_vec(), pb.serialize().to_vec());
        let main = "https://messagebox.bsvblockchain.tech";
        let gmb = "https://gmb.bsvblockchain.tech";
        let prepared = a.prepare_message(&pb.serialize(), "payment_inbox", b"hodos-k1-probe").unwrap();
        a.post_message(main, &prepared).await.expect("post to the main name");
        let on_main = b.list_messages(main, "payment_inbox").await.expect("list main");
        let on_gmb = b.list_messages(gmb, "payment_inbox").await.expect("list gmb");
        println!("K1: listed on main = {}, on gmb = {}", on_main.len(), on_gmb.len());
        assert_eq!(on_main.len(), 1, "the probe must be on the server it was posted to (control)");
        b.acknowledge(main, &[prepared.message_id.clone()]).await.expect("clean up");
        println!("K1 RESULT: {}", if on_gmb.len() == 1 { "SAME store" } else { "SEPARATE stores" });
    }

    /// `T5P7a-A10` — what a failed `sendMessage` answer means: a repeated message id is delivered; a
    /// refusal of the bytes is permanent; anything else is retried.
    #[test]
    fn t5p7a_a10_send_failure_classification() {
        let dup = r#"{"status":"error","code":"ERR_DUPLICATE_MESSAGE","description":"x"}"#.to_string();
        assert!(classify_send_failure(400, dup).is_ok(), "a repeated id is delivered");
        assert!(classify_send_failure(400, r#"{"code":"ERR_INVALID"}"#.into()).unwrap_err().is_permanent());
        assert!(classify_send_failure(413, "ERR_DUPLICATE_MESSAGE".into()).unwrap_err().is_permanent());
        assert!(!classify_send_failure(503, "busy".into()).unwrap_err().is_permanent());
        assert!(!classify_send_failure(401, "".into()).unwrap_err().is_permanent());
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
