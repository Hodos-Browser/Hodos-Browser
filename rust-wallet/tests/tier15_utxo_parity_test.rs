///! Tier 15 — UTXO-set parity across a transport change (B5-T1-P1, P1-A2)
///!
///! Network test, `#[ignore]`d. Runs the production `fetch_all_utxos` over a
///! list of addresses and writes the sorted outpoint set, one
///! `txid:vout confirmed=<bool> sats=<n>` line each, so two builds can be diffed:
///!   set HODOS_PARITY_ADDRS=<json [{"index":0,"address":"1..."}...]>
///!   set HODOS_PARITY_OUT=<output file>
///!   cargo test --release --test tier15_utxo_parity_test -- --ignored --nocapture

use hodos_wallet::json_storage::AddressInfo;
use hodos_wallet::utxo_fetcher::fetch_all_utxos;

#[derive(serde::Deserialize)]
struct Addr {
    index: i32,
    address: String,
}

#[tokio::test]
#[ignore]
async fn p1_a2_write_outpoint_set() {
    // The fetcher falls back to single-address reads when a bulk call fails, which
    // would rebuild the same set; run with RUST_LOG=hodos_wallet::utxo_fetcher=info
    // and check no "Bulk request failed" / "Failed to parse bulk" / "Bulk endpoint"
    // line appears, or the parity says nothing about the bulk path.
    let _ = env_logger::builder().is_test(true).try_init();
    let addrs_path = std::env::var("HODOS_PARITY_ADDRS").expect("HODOS_PARITY_ADDRS");
    let out_path = std::env::var("HODOS_PARITY_OUT").expect("HODOS_PARITY_OUT");
    let addrs: Vec<Addr> =
        serde_json::from_str(&std::fs::read_to_string(addrs_path).unwrap()).unwrap();
    let infos: Vec<AddressInfo> = addrs
        .into_iter()
        .map(|a| AddressInfo {
            index: a.index,
            address: a.address,
            public_key: String::new(),
            used: false,
            balance: 0,
        })
        .collect();

    let utxos = fetch_all_utxos(&infos).await.expect("fetch_all_utxos");
    let mut lines: Vec<String> = utxos
        .iter()
        .map(|u| format!("{}:{} confirmed={} sats={}", u.txid, u.vout, u.confirmed, u.satoshis))
        .collect();
    lines.sort();
    eprintln!("  {} addresses -> {} outpoints, {} sats",
        infos.len(), lines.len(), utxos.iter().map(|u| u.satoshis).sum::<i64>());
    std::fs::write(out_path, lines.join("\n") + "\n").unwrap();
}
