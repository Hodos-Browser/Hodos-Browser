///! Tier 13 — the wallet's outbound transport (B5-T1-P1, reqwest 0.12)
///!
///! Network tests, `#[ignore]`d: run by hand with
///!   cargo test --release --test tier13_transport_tls_timeouts_test -- --ignored --nocapture
///!
///! Every client here is built the way production builds one:
///! `reqwest::Client::builder().timeout(CallClass::X.timeout()).build()`.
///!
///!  P1-A5  bad-certificate hosts are refused BY THE VALIDATOR (its own reason
///!         text, not a DNS or timeout error), a good host succeeds, and a
///!         client that skips validation reaches the bad hosts (the RED).
///!  P1-A7  each CallClass fails at its own total timeout against a local
///!         listener that accepts the connection and never answers. (An
///!         unroutable address cannot measure budgets above ~21 s on Windows:
///!         the OS abandons the TCP connect first — measured 2026-10-02.)
///!         The unroutable address is still used for the connect stage, with
///!         the one class whose budget is below the OS ceiling.

use hodos_wallet::services::CallClass;
use std::time::{Duration, Instant};

/// (host, the validator's reason expected in the error chain)
const BAD_HOSTS: &[(&str, &str)] = &[
    ("https://expired.badssl.com/", "Expired"),
    ("https://wrong.host.badssl.com/", "NotValidForName"),
    ("https://self-signed.badssl.com/", "UnknownIssuer"),
    ("https://untrusted-root.badssl.com/", "UnknownIssuer"),
];

const GOOD_HOST: &str = "https://api.whatsonchain.com/v1/bsv/main/chain/info";

/// An address that drops packets: exercises the connect stage.
const UNROUTABLE: &str = "http://10.255.255.1/";

/// A local server that accepts every connection and never writes a byte, so
/// only the client's total timeout can end the call. Returns its URL.
async fn silent_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((sock, _)) = listener.accept().await {
            held.push(sock);
        }
    });
    format!("http://{}/", addr)
}

fn production_client(class: CallClass) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(class.timeout())
        .build()
        .expect("client builds")
}

fn error_chain(e: &reqwest::Error) -> String {
    let mut s = format!("{}", e);
    let mut src = std::error::Error::source(e);
    while let Some(inner) = src {
        s.push_str(" <- ");
        s.push_str(&format!("{:?}", inner));
        src = inner.source();
    }
    s
}

#[tokio::test]
#[ignore]
async fn p1_a5_bad_certificates_refused_by_validator() {
    let client = production_client(CallClass::IndexerSync);
    for (url, reason) in BAD_HOSTS {
        match client.get(*url).send().await {
            Ok(r) => panic!("{} was ACCEPTED (status {})", url, r.status()),
            Err(e) => {
                let chain = error_chain(&e);
                eprintln!("  refused {} :: {}", url, chain);
                assert!(e.is_connect(), "{}: not a connect-stage error: {}", url, chain);
                assert!(!e.is_timeout(), "{}: timed out, validator never ran: {}", url, chain);
                assert!(
                    chain.contains(reason),
                    "{}: refusal does not carry the validator's reason {:?}: {}",
                    url, reason, chain
                );
            }
        }
    }
}

#[tokio::test]
#[ignore]
async fn p1_a5_good_host_accepted() {
    let r = production_client(CallClass::IndexerSync)
        .get(GOOD_HOST)
        .send()
        .await
        .expect("good host reachable through the validator");
    eprintln!("  {} -> {} ({:?})", GOOD_HOST, r.status(), r.version());
    assert!(r.status().is_success());
    // Owner decision 8 kept 0.11's HTTP/2: without the `http2` feature this
    // would silently be HTTP/1.1.
    assert_eq!(r.version(), reqwest::Version::HTTP_2, "not negotiated over HTTP/2");
}

/// RED for P1-A5: the same hosts with validation switched off are reached,
/// which proves the refusals above came from the validator, not the network.
#[tokio::test]
#[ignore]
async fn p1_a5_red_unvalidated_client_reaches_bad_hosts() {
    let client = reqwest::Client::builder()
        .timeout(CallClass::IndexerSync.timeout())
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    for (url, _) in BAD_HOSTS {
        let r = client.get(*url).send().await;
        eprintln!("  unvalidated {} -> {:?}", url, r.as_ref().map(|r| r.status()));
        assert!(r.is_ok(), "{} unreachable even without validation", url);
    }
}

/// `expected_secs` is written out, not read from `CallClass::timeout`, so a
/// changed budget fails here instead of moving the target with it. The outer
/// guard turns a call that ignores its budget (e.g. a timeout that only covers
/// the connect) into a failure rather than a hang.
async fn assert_class_times_out(class: CallClass, url: &str, expected_secs: u64) {
    let expected = Duration::from_secs(expected_secs);
    let started = Instant::now();
    let guarded = tokio::time::timeout(
        expected + Duration::from_secs(5),
        production_client(class).get(url).send(),
    )
    .await;
    let took = started.elapsed();
    let r = guarded.unwrap_or_else(|_| {
        panic!("{:?}: still running {:?} after its {:?} budget — the timeout did not end the call", class, took, expected)
    });
    eprintln!("  {:?} @ {}: expected {:?}, failed after {:.2?} :: {:?}", class, url, expected, took, r.as_ref().err());
    let e = r.expect_err("the target must not answer");
    assert!(e.is_timeout(), "{:?}: ended by something other than the timeout: {}", class, error_chain(&e));
    let slack = Duration::from_secs(1);
    assert!(
        took + slack >= expected && took <= expected + slack,
        "{:?}: failed after {:?}, expected {:?} ± 1 s",
        class, took, expected
    );
}

#[tokio::test]
#[ignore]
async fn p1_a7_indexer_sync_timeout() {
    assert_class_times_out(CallClass::IndexerSync, &silent_server().await, 8).await;
}

#[tokio::test]
#[ignore]
async fn p1_a7_indexer_async_timeout() {
    assert_class_times_out(CallClass::IndexerAsync, &silent_server().await, 15).await;
}

#[tokio::test]
#[ignore]
async fn p1_a7_indexer_bulk_timeout() {
    assert_class_times_out(CallClass::IndexerBulk, &silent_server().await, 30).await;
}

#[tokio::test]
#[ignore]
async fn p1_a7_third_party_no_fallback_timeout() {
    assert_class_times_out(CallClass::ThirdPartyNoFallback, &silent_server().await, 240).await;
}

/// Connect stage: an address that never completes the TCP handshake ends the
/// call at the class budget (IndexerSync's 8 s is below the OS connect ceiling).
#[tokio::test]
#[ignore]
async fn p1_a7_connect_stage_unroutable() {
    assert_class_times_out(CallClass::IndexerSync, UNROUTABLE, 8).await;
}

/// Every service host named in `rust-wallet/src` (grep `https://`, 2026-10-02;
/// docs and example hosts left out). The root store changed with the bump
/// (`webpki-roots` 0.25.4 → 1.0.9: 147 → 121 anchors, e.g. Baltimore CyberTrust
/// and DigiCert Global Root G1 removed), so each host's chain is checked through
/// the production client. Only a certificate refusal fails the test: any HTTP
/// status means the chain validated; DNS/timeout is reported, not judged.
const WALLET_HOSTS: &[&str] = &[
    "https://api.whatsonchain.com/",
    "https://api.bitails.io/",
    "https://api.coingecko.com/",
    "https://api.mexc.com/",
    "https://arc.gorillapool.io/",
    "https://junglebus.gorillapool.io/",
    "https://mapi.gorillapool.io/",
    "https://ordinals.gorillapool.io/",
    "https://messagebox.babbage.systems/",
    "https://overlay-us-1.bsvb.tech/",
    "https://overlay-eu-1.bsvb.tech/",
    "https://overlay-ap-1.bsvb.tech/",
    "https://users.bapp.dev/",
];

#[tokio::test]
#[ignore]
async fn p1_a5_every_wallet_host_validates() {
    let client = production_client(CallClass::IndexerSync);
    let mut refused = Vec::new();
    for url in WALLET_HOSTS {
        match client.get(*url).send().await {
            Ok(r) => eprintln!("  chain OK  {} -> {}", url, r.status()),
            Err(e) => {
                let chain = error_chain(&e);
                if chain.contains("InvalidCertificate") {
                    eprintln!("  REFUSED   {} :: {}", url, chain);
                    refused.push(*url);
                } else {
                    eprintln!("  no answer {} :: {}", url, chain);
                }
            }
        }
    }
    assert!(refused.is_empty(), "certificate refused by the validator: {:?}", refused);
}
