///! Tier 14 — BSV/USD price fallback chain over the real transport (B5-T1-P1 P1-A6, B5-T1-P1a)
///!
///! Network test, `#[ignore]`d: run by hand with
///!   cargo test --release --test tier14_price_fallback_test -- --ignored --nocapture --test-threads=1
///!
///! Drives the production `PriceCache` unchanged. Providers are knocked out by
///! a local CONNECT proxy (picked up from `HTTPS_PROXY`, as reqwest does for
///! every wallet client) that refuses chosen hosts and tunnels the rest. The
///! proxy's own log of CONNECT targets is the record of which providers the
///! chain actually tried, in order.

use hodos_wallet::database::WalletDatabase;
use hodos_wallet::price_cache::PriceCache;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const WOC: &str = "api.whatsonchain.com";
const BANANA: &str = "bananablocks.com";
const COINGECKO: &str = "api.coingecko.com";
const MEXC: &str = "api.mexc.com";

/// Starts the proxy; returns its URL and the shared list of CONNECT hosts seen.
async fn refusing_proxy(refuse: &'static [&'static str]) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_bg = seen.clone();
    tokio::spawn(async move {
        while let Ok((mut client, _)) = listener.accept().await {
            let seen = seen_bg.clone();
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    if client.read(&mut byte).await.unwrap_or(0) == 0 { return; }
                    head.push(byte[0]);
                }
                let line = String::from_utf8_lossy(&head).lines().next().unwrap_or("").to_string();
                // "CONNECT host:443 HTTP/1.1"
                let target = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let host = target.split(':').next().unwrap_or("").to_string();
                seen.lock().unwrap().push(host.clone());
                if refuse.contains(&host.as_str()) {
                    let _ = client.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n").await;
                    return;
                }
                let Ok(mut upstream) = tokio::net::TcpStream::connect(&target).await else { return; };
                let _ = client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await;
                let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
            });
        }
    });
    (url, seen)
}

fn temp_db(tag: &str) -> Arc<Mutex<WalletDatabase>> {
    let path = std::env::temp_dir().join(format!(
        "hodos_t14_{}_{}_{}.db",
        tag,
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    Arc::new(Mutex::new(WalletDatabase::new(path).expect("temp wallet db")))
}

fn price_row(db: &Arc<Mutex<WalletDatabase>>) -> Option<(f64, i64, String)> {
    db.lock()
        .unwrap()
        .connection()
        .query_row(
            "SELECT price_usd, fetched_at, source FROM bsv_price_cache WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok()
}

/// Pushes the persisted price outside the 5-minute TTL, so the next cache
/// loaded from it must fetch live instead of answering from memory.
fn age_row(db: &Arc<Mutex<WalletDatabase>>) {
    db.lock()
        .unwrap()
        .connection()
        .execute("UPDATE bsv_price_cache SET fetched_at = fetched_at - 3600 WHERE id = 1", [])
        .unwrap();
}

/// Builds a fresh cache whose client routes through a proxy refusing `refuse`.
async fn run_chain(
    refuse: &'static [&'static str],
    db: &Arc<Mutex<WalletDatabase>>,
) -> (f64, Vec<String>) {
    let (proxy, seen) = refusing_proxy(refuse).await;
    std::env::set_var("HTTPS_PROXY", &proxy);
    let cache = PriceCache::new(Some(db.clone()));
    std::env::remove_var("HTTPS_PROXY");
    cache.load_persisted();
    let price = cache.get_price().await;
    let seen = seen.lock().unwrap().clone();
    (price, seen)
}

#[tokio::test]
#[ignore]
async fn p1_a6_price_fallback_chain() {
    // B5-T1-P1a: four live sources, WhatsOnChain → Banana Blocks → CoinGecko → MEXC.
    let db = temp_db("chain");
    let steps: [(&'static [&'static str], &str, &str); 3] = [
        (&[WOC], BANANA, "bananablocks"),                       // P1a-A1
        (&[WOC, BANANA], COINGECKO, "coingecko"),               // P1a-A2 (needs the User-Agent)
        (&[WOC, BANANA, COINGECKO], MEXC, "mexc"),              // P1a-A3
    ];
    for (i, (refuse, answers, source)) in steps.iter().enumerate() {
        if i > 0 { age_row(&db); }
        let (p, seen) = run_chain(refuse, &db).await;
        let row = price_row(&db);
        eprintln!("  [refused {:?}] price={} tried={:?} row={:?}", refuse, p, seen, row);
        let mut expected: Vec<String> = refuse.iter().map(|h| h.to_string()).collect();
        expected.push(answers.to_string());
        assert_eq!(seen, expected, "chain did not move on to {}", answers);
        assert!(p > 0.0);
        assert_eq!(row.as_ref().map(|r| r.2.as_str()), Some(*source));
        assert_eq!(row.as_ref().map(|r| r.0), Some(p));
    }

    // All four down → the last good (persisted) price, and the row is not written.
    age_row(&db);
    let before = price_row(&db).expect("row from the steps above");
    let (p, seen) = run_chain(&[WOC, BANANA, COINGECKO, MEXC], &db).await;
    let after = price_row(&db);
    eprintln!("  [all refused] price={} tried={:?} before={:?} after={:?}", p, seen, before, after);
    assert_eq!(seen, vec![WOC, BANANA, COINGECKO, MEXC]);
    assert_eq!(p, before.0, "did not return the last good price");
    assert_eq!(after, Some(before), "a failed fetch wrote the price row");
}
