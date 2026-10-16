//! B5-T5-P7a — advertisement lookup and verification.
//!
//! Fixtures are live `ls_messagebox` answers for the owner's Hodos identity key, recorded
//! 2026-10-14 from overlay-us-1 and overlay-ap-1 (three advertisements, all naming
//! `messagebox.babbage.systems`; Babbage's lookup host returned none for the same key).

use super::*;

const OWNER_KEY: &str = "029dce5a3602abd74ddf0a9dd1a9e80dff2eb997bc0b01af70b5cde0e7d5166dba";
const US1: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/messagebox/ls_messagebox_us1_2026-10-14.json"));
const AP1: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/messagebox/ls_messagebox_ap1_2026-10-14.json"));

fn outputs(answer: &str) -> Vec<serde_json::Value> {
    output_list(&serde_json::from_str(answer).unwrap()).unwrap()
}

/// `T5P7a-A1` GREEN — every live advertisement verifies, and names the host it was published for.
#[test]
fn t5p7a_a1_live_advertisements_verify_and_name_babbage() {
    let outs = outputs(US1);
    assert_eq!(outs.len(), 3);
    for o in &outs {
        let host = verify_advertisement(o, OWNER_KEY).expect("a live advertisement must verify");
        assert_eq!(host, "https://messagebox.babbage.systems");
    }
    assert_eq!(advertised_hosts(&outs, OWNER_KEY), (vec!["https://messagebox.babbage.systems".to_string()], 0));
}

/// `T5P7a-A1` RED — the same advertisement with one byte of its host changed must NOT verify: the
/// signature is what makes it count, not the lookup host's say-so.
#[test]
fn t5p7a_a1_a_changed_host_byte_fails_the_signature() {
    let mut o = outputs(US1).remove(0);
    let beef: Vec<u64> = o["beef"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    let needle: Vec<u64> = b"babbage".iter().map(|b| *b as u64).collect();
    let at = beef.windows(needle.len()).position(|w| w == needle.as_slice()).expect("host bytes in the BEEF");
    let mut tampered = beef.clone();
    tampered[at] = b'c' as u64; // "cabbage": same length, the script still parses
    o["beef"] = serde_json::json!(tampered);
    let err = verify_advertisement(&o, OWNER_KEY).unwrap_err().to_string();
    assert!(err.contains("does not verify"), "must fail on the signature, got: {}", err);
    assert!(advertised_hosts(&[o], OWNER_KEY).0.is_empty());
}

/// `T5P7a-A1` RED — an advertisement is only for the key it carries: asking for another key finds none.
#[test]
fn t5p7a_a1_another_keys_advertisement_does_not_count() {
    let other = "03".to_string() + &"11".repeat(32);
    assert!(advertised_hosts(&outputs(US1), &other).0.is_empty());
    assert!(verify_advertisement(&outputs(US1)[0], &other).unwrap_err().to_string().contains("another identity key"));
}

/// `T5P7a-A3` — a lookup no host answered is an error, never "no advertisement" (rule 7, trip-wire 2).
#[tokio::test]
async fn t5p7a_a3_no_host_answering_is_failed_not_none() {
    let r = lookup_on(&["https://127.0.0.1:9", "https://127.0.0.1:7"], OWNER_KEY).await;
    assert!(matches!(r, AdvertLookup::Failed(_)), "got {:?}", r);
}

/// `T5P7a-A3` — an answer that is not an output-list is an error, not an empty list.
#[test]
fn t5p7a_a3_an_error_answer_is_not_an_empty_list() {
    assert!(output_list(&serde_json::json!({"status":"error","message":"Request could not be processed"})).is_err());
    assert!(output_list(&serde_json::json!({"type":"output-list"})).is_err());
    assert_eq!(output_list(&serde_json::json!({"type":"output-list","outputs":[]})).unwrap().len(), 0);
}

/// `T5P7a-A4` — an advertised host must be a public HTTPS origin.
#[test]
fn t5p7a_a4_host_normalisation() {
    assert_eq!(normalize_host("https://MessageBox.Babbage.Systems/").unwrap(), "https://messagebox.babbage.systems");
    assert_eq!(normalize_host("https://mb.example.org:8443").unwrap(), "https://mb.example.org:8443");
    for bad in [
        "http://messagebox.babbage.systems",
        "https://user:pw@messagebox.babbage.systems",
        "https://messagebox.babbage.systems/inbox",
        "https://messagebox.babbage.systems?x=1",
        "https://localhost",
        "https://127.0.0.1",
        "https://10.0.0.5",
        "https://192.168.1.2",
        "https://[::1]",
        "https://box.local",
        "https://box.internal",
        "https://example.com",
        "not a url",
    ] {
        assert!(normalize_host(bad).is_err(), "{} must be refused", bad);
    }
}

/// Live, read-only: the owner's key resolves to Babbage's server. Network; run by hand.
#[tokio::test]
#[ignore]
async fn live_owner_key_resolves_to_babbage() {
    assert_eq!(lookup(OWNER_KEY).await, AdvertLookup::Advertised(vec!["https://messagebox.babbage.systems".into()]));
}

/// A local lookup host that answers every request with `body`.
async fn lookup_host(body: &'static str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let _ = s.read(&mut buf).await;
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                let _ = s.write_all(resp.as_bytes()).await;
            });
        }
    });
    format!("http://{}", addr)
}

const EMPTY: &str = r#"{"type":"output-list","outputs":[]}"#;

/// `T5P7a-A3` (review 178 #4) — "none" from the hosts that answered, while another host did not
/// answer, is `Failed`: the silent host may be the one that carries the advertisement (measured
/// 2026-10-14: Babbage's lookup host had none of the owner's three).
#[tokio::test]
async fn t5p7a_a3_none_from_some_hosts_while_another_is_silent_is_failed() {
    let empty = lookup_host(EMPTY).await;
    let r = lookup_on(&[empty.as_str(), "http://127.0.0.1:9"], OWNER_KEY).await;
    assert!(matches!(r, AdvertLookup::Failed(_)), "got {:?}", r);
    // Control: every host answered "none" ⇒ NoAdvertisement.
    assert_eq!(lookup_on(&[empty.as_str()], OWNER_KEY).await, AdvertLookup::NoAdvertisement);
    // An advertisement from any host is enough, whoever else is silent.
    let us1 = lookup_host(US1).await;
    assert_eq!(lookup_on(&[us1.as_str(), "http://127.0.0.1:9"], OWNER_KEY).await,
        AdvertLookup::Advertised(vec!["https://messagebox.babbage.systems".into()]));
}

/// `T5P7a-A3` (review 178-3 #6) — "none" from two lookup hosts while the third is silent is an answer
/// (NoAdvertisement), not a failure; and an answer over the size cap is an error.
#[tokio::test]
async fn t5p7a_a3_none_from_two_hosts_is_an_answer() {
    let a = lookup_host(EMPTY).await;
    let b = lookup_host(EMPTY).await;
    assert_eq!(lookup_on(&[a.as_str(), b.as_str(), "http://127.0.0.1:9"], OWNER_KEY).await, AdvertLookup::NoAdvertisement);
    let huge: &'static str = Box::leak(format!(r#"{{"type":"output-list","outputs":[],"pad":"{}"}}"#, "x".repeat(5 * 1024 * 1024)).into_boxed_str());
    let h = lookup_host(huge).await;
    assert!(matches!(lookup_on(&[h.as_str()], OWNER_KEY).await, AdvertLookup::Failed(e) if e.contains("over")));
}

/// `T5P7a-A4` (review 178 #6) — IPv6 literals that reach a private IPv4 address are refused.
#[test]
fn t5p7a_a4_mapped_and_nat64_literals_are_refused() {
    for bad in ["https://[::ffff:192.168.1.1]", "https://[::ffff:127.0.0.1]", "https://[::ffff:10.0.0.1]", "https://[64:ff9b::a00:1]"] {
        assert!(normalize_host(bad).is_err(), "{} must be refused", bad);
    }
    assert!(normalize_host("https://[2606:4700::6810:84e5]").is_ok(), "a public IPv6 literal is fine");
    // Review 178-2 #8
    for bad in ["https://0.1.2.3", "https://224.0.0.1", "https://250.1.1.1", "https://[::127.0.0.1]", "https://[::10.0.0.5]", "https://[ff02::1]"] {
        assert!(normalize_host(bad).is_err(), "{} must be refused", bad);
    }
}

/// `T5P7a-A18` (review 178-2 #9) — the work on one lookup answer is bounded: an oversized BEEF is
/// refused before it is parsed, and at most 100 outputs per lookup host are looked at.
#[tokio::test]
async fn t5p7a_a18_lookup_work_is_bounded() {
    let big = serde_json::json!({"outputIndex": 0, "beef": vec![0u8; 64 * 1024 + 1]});
    assert_eq!(verify_advertisement(&big, OWNER_KEY).unwrap_err().to_string(), "beef too large");
    // 100 junk outputs ahead of the owner's valid advertisement: the valid one is past the bound.
    let mut outs: Vec<serde_json::Value> = (0..100).map(|_| serde_json::json!({"outputIndex": 0, "beef": [1, 2, 3]})).collect();
    outs.push(outputs(US1).remove(0));
    let answer: &'static str = Box::leak(serde_json::json!({"type":"output-list","outputs":outs}).to_string().into_boxed_str());
    let host = lookup_host(answer).await;
    // The valid advertisement past the bound is never looked at (the 100 unreadable ones make it
    // Failed since review 178-5 #5 — either way, not Advertised).
    assert!(!matches!(lookup_on(&[host.as_str()], OWNER_KEY).await, AdvertLookup::Advertised(_)));
    // Control: within the bound, the same advertisement is found.
    let short: &'static str = Box::leak(serde_json::json!({"type":"output-list","outputs":[outputs(US1).remove(0)]}).to_string().into_boxed_str());
    let host = lookup_host(short).await;
    assert!(matches!(lookup_on(&[host.as_str()], OWNER_KEY).await, AdvertLookup::Advertised(_)));
}

/// `T5P7a-A30` (review 178-5 #5) — an advertisement we cannot READ makes the lookup Failed, not "none".
#[tokio::test]
async fn t5p7a_a30_an_unreadable_advertisement_is_failed_not_none() {
    let unreadable: &'static str = r#"{"type":"output-list","outputs":[{"outputIndex":0,"beef":[1,2,3]}]}"#;
    let host = lookup_host(unreadable).await;
    assert!(matches!(lookup_on(&[host.as_str()], OWNER_KEY).await, AdvertLookup::Failed(_)));
    // Control: a readable advertisement for ANOTHER key is a verdict — none for ours.
    let other_key = lookup_host(US1).await;
    let other = "03".to_string() + &"11".repeat(32);
    assert_eq!(lookup_on(&[other_key.as_str()], &other).await, AdvertLookup::NoAdvertisement);
}

/// `T5P7a-A29` (review 178-5 #1) — the lookup never follows a redirect.
#[tokio::test]
async fn t5p7a_a29_lookup_never_follows_a_redirect() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let target = lookup_host(EMPTY).await;
    let evil = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let evil_addr = evil.local_addr().unwrap();
    let t = target.clone();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = evil.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let _ = s.read(&mut buf).await;
            let resp = format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: {}/lookup\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", t);
            let _ = s.write_all(resp.as_bytes()).await;
        }
    });
    let r = lookup_on(&[&format!("http://{}", evil_addr)], OWNER_KEY).await;
    assert!(matches!(r, AdvertLookup::Failed(_)), "followed the redirect: {:?}", r);
}

/// `T5P7a-A33` (review 178-6 #8) — outputs past the per-host cap, and an oversized BEEF, were not
/// looked at: the lookup is Failed, never "none".
#[tokio::test]
async fn t5p7a_a33_unexamined_outputs_make_the_lookup_failed() {
    let other = "03".to_string() + &"11".repeat(32);
    // 101 readable advertisements, all for the owner's key: for `other` each is a verdict (not ours).
    let outs: Vec<serde_json::Value> = (0..101).map(|_| outputs(US1).remove(0)).collect();
    let answer: &'static str = Box::leak(serde_json::json!({"type":"output-list","outputs":outs}).to_string().into_boxed_str());
    let host = lookup_host(answer).await;
    assert!(matches!(lookup_on(&[host.as_str()], &other).await, AdvertLookup::Failed(_)), "past the cap is not none");
    // Control: within the cap, the same answer is a verdict — none for `other`.
    let outs: Vec<serde_json::Value> = (0..3).map(|_| outputs(US1).remove(0)).collect();
    let answer: &'static str = Box::leak(serde_json::json!({"type":"output-list","outputs":outs}).to_string().into_boxed_str());
    let host = lookup_host(answer).await;
    assert_eq!(lookup_on(&[host.as_str()], &other).await, AdvertLookup::NoAdvertisement);
    // An oversized BEEF is unreadable, not a verdict.
    let big: &'static str = Box::leak(serde_json::json!({"type":"output-list","outputs":[{"outputIndex":0,"beef":vec![0u8; 64 * 1024 + 1]}]}).to_string().into_boxed_str());
    let host = lookup_host(big).await;
    assert!(matches!(lookup_on(&[host.as_str()], OWNER_KEY).await, AdvertLookup::Failed(_)));
}
