//! B5-T5-P7a — where a recipient reads their MessageBox mail.
//!
//! A wallet publishes a MessageBox *advertisement*: a 1-sat PushDrop on the `tm_messagebox` overlay
//! topic whose fields are `[identityKey, host, signature]`, the signature over `identityKey ‖ host`
//! made with the identity key's child for `[1, "messagebox advertisement"]`, key `1`, counterparty
//! `anyone`. A sender looks the recipient's key up on the `ls_messagebox` lookup service and posts
//! to the host it names.
//!
//! Ported from `@bsv/message-box-client` 2.6.0 `MessageBoxClient :: queryAdvertisements` and
//! `host.js :: normalizeOverlayMessageBoxHost` (prior-art ledger, 2026-10-14), with two deliberate
//! differences:
//! - ⛔ A lookup that no host answered is an **error**, never "no advertisement" (the library
//!   swallows it and falls back to its own host — rule 7, trip-wire 2). The caller decides what an
//!   error means; it is never read as a verdict here.
//! - Every lookup host is asked and the answers are merged: on 2026-10-14 the owner's key had three
//!   advertisements on two BSVA hosts and none on Babbage's.
//!
//! ⛔ We never trust the lookup host: an advertisement counts only when its signature verifies for
//! the queried key. A lookup host cannot forge one; it can hide one.

use std::collections::BTreeSet;

use log::{debug, info, warn};
use sha2::{Digest, Sha256};

/// The hosts that answer `ls_messagebox` (measured 2026-10-14: the `ls_slap` answers of the four
/// SLAP trackers name these three; `overlay-eu-1` is not among them).
pub const LS_MESSAGEBOX_HOSTS: &[&str] = &[
    "https://overlay-us-1.bsvb.tech",
    "https://overlay-ap-1.bsvb.tech",
    "https://backend.59d6f2d7e6314d0b188e11df0f516478.projects.babbage.systems",
];

/// One client for every lookup (review 178-6 #14: connections reused). Review 178-5 #1: no redirects
/// (the SDK's lookup uses `redirect: 'error'`).
static LOOKUP_CLIENT: once_cell::sync::Lazy<Result<reqwest::Client, String>> = once_cell::sync::Lazy::new(|| {
    reqwest::Client::builder().timeout(LOOKUP_TIMEOUT).redirect(reqwest::redirect::Policy::none()).build().map_err(|e| e.to_string())
});

/// Per-host lookup timeout. Measured answers took 0.2–0.9 s. A send runs this after its broadcast,
/// inside the wallet panel's 30 s call (review 178 #2), so it stays short.
const LOOKUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);

/// Review 178-2 #9: the work done on one lookup answer is bounded — it runs on the send path after
/// the broadcast. The server returns at most 100 advertisements (`findAdvertisements` limit 100);
/// measured advertisement BEEFs are 0.7–1.7 KB.
const MAX_OUTPUTS_PER_HOST: usize = 100;
const MAX_ADVERT_BEEF_BYTES: usize = 64 * 1024;
/// A whole lookup answer. JSON number arrays cost up to 4 characters per byte, so this holds ~1 MB
/// of BEEF — hundreds of measured advertisements (0.7–1.7 KB each). A larger answer is an error, and
/// the lookup reads as failed, never as "none" (review 178-6 #8 corrected this comment).
const MAX_ANSWER_BYTES: usize = 4 * 1024 * 1024;

/// BRC-43 invoice of the advertisement signature: `[1, "messagebox advertisement"]`, key `1`.
const ADVERTISEMENT_INVOICE: &str = "1-messagebox advertisement-1";

/// The `anyone` private key (the number 1): the advertisement is signed for counterparty `anyone`,
/// so anyone can derive the key that verifies it (CLAUDE.md, "The well-known anyone key").
const ANYONE_PRIVATE_KEY: [u8; 32] = {
    let mut k = [0u8; 32];
    k[31] = 1;
    k
};

/// What the lookup found for one recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdvertLookup {
    /// The distinct hosts named by advertisements that verified, sorted.
    Advertised(Vec<String>),
    /// At least one lookup host answered, and no advertisement verified.
    NoAdvertisement,
    /// No lookup host answered. Not a verdict.
    Failed(String),
}

/// Look up where `identity_key_hex` reads its mail, asking every `ls_messagebox` host.
pub async fn lookup(identity_key_hex: &str) -> AdvertLookup {
    lookup_on(LS_MESSAGEBOX_HOSTS, identity_key_hex).await
}

pub(crate) async fn lookup_on(hosts: &[&str], identity_key_hex: &str) -> AdvertLookup {
    let key = identity_key_hex.to_lowercase();
    let client = match LOOKUP_CLIENT.as_ref() {
        Ok(c) => c.clone(),
        Err(e) => return AdvertLookup::Failed(format!("HTTP client: {}", e)),
    };
    let body = serde_json::json!({ "service": "ls_messagebox", "query": { "identityKey": key } });

    let mut set = tokio::task::JoinSet::new();
    for host in hosts {
        let (host, client, body) = (host.to_string(), client.clone(), body.clone());
        set.spawn(async move {
            let url = format!("{}/lookup", host);
            let answer = async {
                let response = client.post(&url).json(&body).send().await.map_err(|e| e.to_string())?;
                if !response.status().is_success() {
                    return Err(format!("HTTP {}", response.status()));
                }
                // Review 178-3 #10: the answer is read up to a cap, never parsed whole first.
                let bytes = crate::authfetch::read_capped(response, MAX_ANSWER_BYTES).await?;
                serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| e.to_string())
            }.await;
            (host, answer)
        });
    }
    let mut answers = Vec::new();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(a) => answers.push(a),
            Err(e) => answers.push(("(task)".to_string(), Err(e.to_string()))),
        }
    }

    let mut answered = 0usize;
    let mut over_cap = 0usize;
    let mut errors = Vec::new();
    let mut outputs = Vec::new();
    for (host, answer) in answers {
        match answer.and_then(|json| output_list(&json)) {
            Ok(list) => {
                debug!("MessageBox lookup: {} returned {} advertisement(s)", host, list.len());
                answered += 1;
                // Review 178-6 #8: outputs past the cap were not looked at — not "none".
                if list.len() > MAX_OUTPUTS_PER_HOST {
                    over_cap += 1;
                }
                outputs.extend(list.into_iter().take(MAX_OUTPUTS_PER_HOST));
            }
            Err(e) => {
                warn!("MessageBox lookup: {} failed: {}", host, e);
                errors.push(format!("{}: {}", host, e));
            }
        }
    }
    if answered == 0 {
        return AdvertLookup::Failed(errors.join("; "));
    }
    let (found, unreadable) = advertised_hosts(&outputs, &key);
    let unreadable = unreadable + over_cap;
    info!("MessageBox lookup for {}…: {} host(s) answered, advertised: {:?}", &key[..16.min(key.len())], answered, found);
    if !found.is_empty() {
        AdvertLookup::Advertised(found)
    } else if unreadable > 0 {
        // Review 178-5 #5: advertisements we could not READ (a format our parser does not know) are not
        // "none" — they may name the host the recipient reads.
        warn!("MessageBox lookup for {}…: {} advertisement(s) could not be read — lookup failed", &key[..16.min(key.len())], unreadable);
        AdvertLookup::Failed(format!("{} advertisement(s) could not be read", unreadable))
    } else if !errors.is_empty() && answered < 2 {
        // Review 178-3 #6: "none" from two of the three lookup hosts is an answer; from one alone,
        // while the others are silent, it is not (2026-10-14: Babbage's host lacked adverts BSVA's had).
        // Review 178 #4: the hosts disagree (2026-10-14: Babbage's had none of the owner's three), so
        // "none" from the ones that answered says nothing about the ones that did not.
        AdvertLookup::Failed(format!("no advertisement from the hosts that answered; no answer from: {}", errors.join("; ")))
    } else {
        AdvertLookup::NoAdvertisement
    }
}

/// The `outputs` of an `output-list` answer. Any other shape is an error, never "none".
fn output_list(json: &serde_json::Value) -> Result<Vec<serde_json::Value>, String> {
    if json.get("type").and_then(|t| t.as_str()) != Some("output-list") {
        return Err(format!("unexpected answer: {:.200}", json.to_string()));
    }
    json.get("outputs").and_then(|o| o.as_array()).cloned().ok_or_else(|| "output-list without outputs".to_string())
}

/// The distinct hosts named by the advertisements among `outputs` that verify for `identity_key_hex`,
/// sorted, and how many outputs could not be read at all. The same advertisement returned by several
/// lookup hosts names the same host once.
pub(crate) fn advertised_hosts(outputs: &[serde_json::Value], identity_key_hex: &str) -> (Vec<String>, usize) {
    let mut hosts = BTreeSet::new();
    let mut unreadable = 0;
    for output in outputs {
        match verify_advertisement(output, identity_key_hex) {
            Ok(host) => {
                hosts.insert(host);
            }
            Err(AdvertReject::Unreadable(e)) => {
                warn!("MessageBox lookup: an advertisement could not be read: {}", e);
                unreadable += 1;
            }
            Err(AdvertReject::NotValid(e)) => debug!("MessageBox lookup: advertisement ignored: {}", e),
        }
    }
    (hosts.into_iter().collect(), unreadable)
}

/// Why a lookup output is not counted (review 178-5 #5): one we could not read at all is not a
/// verdict; one we read and found not valid is.
#[derive(Debug)]
pub(crate) enum AdvertReject {
    Unreadable(String),
    NotValid(String),
}

impl std::fmt::Display for AdvertReject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { AdvertReject::Unreadable(s) | AdvertReject::NotValid(s) => f.write_str(s) }
    }
}

/// One lookup output ⇒ its host, when it is an advertisement for
/// `identity_key_hex` whose signature verifies and whose host is acceptable.
pub(crate) fn verify_advertisement(output: &serde_json::Value, identity_key_hex: &str) -> Result<String, AdvertReject> {
    // Review 178-2 #9: an oversized BEEF is refused before it is parsed.
    match output.get("beef") {
        // Review 178-6 #8: not read, so not a verdict — unreadable.
        Some(serde_json::Value::Array(a)) if a.len() > MAX_ADVERT_BEEF_BYTES => return Err(AdvertReject::Unreadable("beef too large".into())),
        Some(serde_json::Value::String(s)) if s.len() > MAX_ADVERT_BEEF_BYTES * 4 / 3 + 4 => return Err(AdvertReject::Unreadable("beef too large".into())),
        _ => {}
    }
    let (key_field, host_field, signature) = read_advertisement(output).map_err(AdvertReject::Unreadable)?;
    check_advertisement(identity_key_hex, key_field, host_field, signature).map_err(AdvertReject::NotValid)
}

/// The advertisement's three PushDrop fields, read from a lookup output.
fn read_advertisement(output: &serde_json::Value) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), String> {
    let output_index = output.get("outputIndex").and_then(|v| v.as_u64()).ok_or("no outputIndex")?;
    let beef_bytes: Vec<u8> = match output.get("beef") {
        Some(serde_json::Value::Array(a)) => a.iter()
            .map(|v| v.as_u64().filter(|n| *n <= 255).map(|n| n as u8))
            .collect::<Option<Vec<u8>>>().ok_or("beef is not a byte array")?,
        Some(serde_json::Value::String(s)) => {
            use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
            BASE64.decode(s).map_err(|e| format!("beef base64: {}", e))?
        }
        _ => return Err("no beef".into()),
    };
    let beef = crate::beef::Beef::from_bytes(&beef_bytes).map_err(|e| format!("beef: {}", e))?;
    let tx_bytes = beef.transactions.last().ok_or("empty beef")?;
    let tx = crate::beef::ParsedTransaction::from_bytes(tx_bytes).map_err(|e| format!("transaction: {}", e))?;
    let script = &tx.outputs.get(output_index as usize).ok_or("outputIndex out of range")?.script;
    let decoded = crate::script::pushdrop::decode(script).map_err(|e| format!("not a PushDrop: {:?}", e))?;
    let [key_field, host_field, signature]: [Vec<u8>; 3] = decoded.fields.try_into()
        .map_err(|f: Vec<Vec<u8>>| format!("{} fields, expected 3", f.len()))?;
    Ok((key_field, host_field, signature))
}

/// Is a readable advertisement valid for `identity_key_hex`: its key, its signature, its host.
fn check_advertisement(identity_key_hex: &str, key_field: Vec<u8>, host_field: Vec<u8>, signature: Vec<u8>) -> Result<String, String> {
    if hex::encode(&key_field) != identity_key_hex.to_lowercase() {
        return Err("advertisement is for another identity key".into());
    }
    if host_field.len() > 2048 || signature.len() > 80 {
        return Err("field too long".into());
    }
    let mut data = key_field.clone();
    data.extend_from_slice(&host_field);
    verify_advertisement_signature(&key_field, &data, &signature)?;

    let host = std::str::from_utf8(&host_field).map_err(|_| "host is not UTF-8")?;
    normalize_host(host)
}

/// The signature over `data` verifies with `identity_key`'s child for the advertisement invoice,
/// derived as `anyone` (verifier private key 1, counterparty = the identity key).
fn verify_advertisement_signature(identity_key: &[u8], data: &[u8], der: &[u8]) -> Result<(), String> {
    let child = crate::crypto::brc42::derive_child_public_key(&ANYONE_PRIVATE_KEY, identity_key, ADVERTISEMENT_INVOICE)
        .map_err(|e| format!("derivation: {:?}", e))?;
    let secp = secp256k1::Secp256k1::verification_only();
    let pubkey = secp256k1::PublicKey::from_slice(&child).map_err(|e| e.to_string())?;
    let digest: [u8; 32] = Sha256::digest(data).into();
    let message = secp256k1::Message::from_digest(digest);
    let mut sig = secp256k1::ecdsa::Signature::from_der(der).map_err(|e| format!("signature DER: {}", e))?;
    // The SDK accepts a high-S signature; libsecp256k1 verifies only low-S.
    sig.normalize_s();
    secp.verify_ecdsa(&message, &sig, &pubkey).map_err(|_| "signature does not verify".to_string())
}

/// `normalizeOverlayMessageBoxHost`: an advertised host must be a public HTTPS origin — no
/// credentials, path, query or fragment, and not loopback, private, link-local or a reserved name.
/// Returns `https://host[:port]`.
pub(crate) fn normalize_host(raw: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(raw.trim()).map_err(|e| format!("host {:?}: {}", raw, e))?;
    if url.scheme() != "https" {
        return Err(format!("host {:?} is not https", raw));
    }
    if !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
        return Err(format!("host {:?} carries credentials, a query or a fragment", raw));
    }
    if !(url.path().is_empty() || url.path() == "/") {
        return Err(format!("host {:?} has a path", raw));
    }
    let host = url.host_str().unwrap_or_default().to_lowercase();
    // Review 178-2 #8: also 0.0.0.0/8, multicast and 240/4. ⚠️ A NAME that resolves to a private address
    // is not caught (nor by the library): that would need a resolver hook — noted in the contract.
    let blocked_v4 = |ip: std::net::Ipv4Addr| ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_unspecified()
        || ip.is_broadcast() || (ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 64)
        || ip.octets()[0] == 0 || ip.is_multicast() || ip.octets()[0] >= 240;
    let blocked = match host.trim_start_matches('[').trim_end_matches(']').parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => blocked_v4(ip),
        // Review 178 #6: an IPv4-mapped (`::ffff:a.b.c.d`) or NAT64 (`64:ff9b::/96`) literal reaches the
        // IPv4 address inside it — judged by the IPv4 rules.
        Ok(std::net::IpAddr::V6(ip)) => ip.is_loopback() || ip.is_unspecified()
            || (ip.segments()[0] & 0xfe00) == 0xfc00 || (ip.segments()[0] & 0xffc0) == 0xfe80
            || ip.is_multicast()
            || ip.to_ipv4_mapped().map_or(false, blocked_v4)
            // IPv4-compatible `::a.b.c.d`
            || (ip.segments()[..6] == [0, 0, 0, 0, 0, 0] && blocked_v4(std::net::Ipv4Addr::from(u128::from(ip) as u32)))
            || (ip.segments()[..6] == [0x64, 0xff9b, 0, 0, 0, 0]),
        Err(_) => {
            let d = host.trim_end_matches('.');
            d.is_empty() || d == "localhost" || d.ends_with(".localhost")
                || [".local", ".lan", ".internal", ".test"].iter().any(|s| d.ends_with(s))
                || d == "example.com" || d.ends_with(".example.com")
        }
    };
    if blocked {
        return Err(format!("host {:?} is not a public host", raw));
    }
    Ok(match url.port() {
        Some(p) => format!("https://{}:{}", host, p),
        None => format!("https://{}", host),
    })
}

#[cfg(test)]
#[path = "messagebox_lookup_tests.rs"]
mod tests;
