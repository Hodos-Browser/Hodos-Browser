// B5-T1-P7b — broadcast rows (contract §5b): BEEF first, the Atomic header stripped, TAAL gone,
// Bitails as the second operator. Contract:
// development-docs/0.4.0-beta.7/track-1-money-path/phase-P7b-payments-kept-broadcasts-reach-operators/
//
// `arc_model` answers as GorillaPool ARC did when probed live on 2026-10-10 (KICKOFF_DESIGN.md):
// the Atomic header ⇒ 400; a BEEF whose last transaction spends a parent the bundle does not
// carry ⇒ 463; otherwise accepted, naming the LAST transaction. Its default is a refusal, so a
// row cannot pass by reaching it with the wrong bytes. Known limit: one level of parents, BUMPs
// not checked (row `P7b-L1` calibrates it against the real service).

use super::p2_test_support::{app_state_from, app_state_with, status, ScriptedProvider};
use super::p7a_route_tests::{funding, internalize_with_parents, outputs_for, tx, txid_hex};
use super::*;
use crate::services::{BroadcastResult, IndexerError, IndexerProvider, TxState, WalletServices};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex as StdMutex};

type Sent = Arc<StdMutex<Vec<Vec<u8>>>>;

fn arc_answer(bytes: &[u8]) -> Result<BroadcastResult, IndexerError> {
    let refuse = |status: u16, body: &str| Err(IndexerError::ProviderStatus { provider: "arc_gorillapool", status, body: body.into() });
    if bytes.starts_with(&crate::beef::ATOMIC_BEEF_MARKER) {
        return refuse(400, "script(53): got 25 bytes: unexpected EOF");
    }
    let Ok(beef) = crate::beef::Beef::from_bytes(bytes) else { return refuse(400, "failed to parse beef") };
    let Some(last) = beef.transactions.last() else { return refuse(400, "empty beef") };
    let Ok(parsed) = crate::beef::ParsedTransaction::from_bytes(last) else { return refuse(400, "bad tx") };
    for input in &parsed.inputs {
        if beef.find_txid(&input.prev_txid).is_none() {
            return refuse(463, &format!("error while decoding BEEF: missing previous transaction for {}", input.prev_txid));
        }
    }
    Ok(BroadcastResult { provider: "arc_gorillapool", txid: txid_hex(last), tx_status: "SEEN_ON_NETWORK".into(), merkle_path_bump: None, block_height: None })
}

fn arc_model() -> (Arc<ScriptedProvider>, Sent) {
    let sent: Sent = Arc::new(StdMutex::new(vec![]));
    let mut p = ScriptedProvider::new("arc_gorillapool");
    p.status_for = Box::new(|_| Err(IndexerError::NotFound));
    let s = sent.clone();
    p.broadcast = Box::new(move |bytes| {
        s.lock().unwrap().push(bytes.to_vec());
        arc_answer(bytes)
    });
    (Arc::new(p), sent)
}

/// A raw-only provider (as MAPI / WhatsOnChain): extracts the subject, accepts naming it.
fn raw_only(name: &'static str) -> (Arc<ScriptedProvider>, Sent) {
    let sent: Sent = Arc::new(StdMutex::new(vec![]));
    let mut p = ScriptedProvider::new(name);
    let s = sent.clone();
    p.broadcast = Box::new(move |bytes| {
        s.lock().unwrap().push(bytes.to_vec());
        let beef = crate::beef::Beef::from_bytes(bytes).map_err(|e| IndexerError::InvalidResponse { provider: name, reason: e })?;
        let main = beef.main_transaction().cloned().unwrap_or_default();
        Ok(BroadcastResult { provider: name, txid: txid_hex(&main), tx_status: "SEEN_ON_NETWORK".into(), merkle_path_bump: None, block_height: None })
    });
    (Arc::new(p), sent)
}

fn status_provider(state: TxState) -> Arc<ScriptedProvider> {
    let mut p = ScriptedProvider::new("status");
    p.status_for = Box::new(move |txid| match state {
        TxState::Unknown => Err(IndexerError::NotFound),
        ref s => Ok(status(txid, s.clone(), None)),
    });
    Arc::new(p)
}

fn dynv(ps: &[Arc<ScriptedProvider>]) -> Vec<Arc<dyn IndexerProvider>> {
    ps.iter().map(|p| p.clone() as Arc<dyn IndexerProvider>).collect()
}

/// A parent and a subject spending it, paying `to` 5,000 sats (fee 95,000). (parent, subject)
fn pair(to: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let parent = funding(100_000);
    let subject = tx(&[(&parent, 0, 0xFFFF_FFFF)], &[(5_000, to)], 0);
    (parent, subject)
}

fn two_tx_beef(parent: &[u8], subject: &[u8]) -> crate::beef::Beef {
    let mut beef = crate::beef::Beef::new();
    beef.add_parent_transaction(parent.to_vec());
    beef.set_main_transaction(subject.to_vec());
    beef
}

/// `P7b-B1` ⭐ — `internalizeAction` broadcasts the BEEF the sender gave us: ARC receives a V1 BEEF
/// whose last transaction is the subject and which carries the parent, accepts it on the first
/// try, and the payment is credited. Today's code sent the parentless wrapper (ARC 463).
#[actix_web::test]
async fn p7b_b1_internalize_broadcasts_the_senders_beef_with_parents() {
    let (arc, arc_sent) = arc_model();
    let (mapi, mapi_sent) = raw_only("gorillapool_mapi");
    let st = status_provider(TxState::Unknown);
    let state = app_state_from(crate::classify::p5_tests::wallet("p7b_b1"),
        WalletServices::with_test_chains(dynv(&[st.clone()]), dynv(&[arc.clone(), mapi.clone()])));
    let lock = { let db = state.database.lock().unwrap(); super::p7a_route_tests::our_lock(&db) };
    let (parent, subject) = pair(lock);

    let (code, body) = internalize_with_parents(&state, &[parent.clone()], &subject).await;

    assert_eq!(code, 200, "{}", body);
    let sent = arc_sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1, "one broadcast, to ARC");
    assert_eq!(&sent[0][..4], &crate::beef::BEEF_V1_MARKER, "ARC must get a V1 BEEF");
    let beef = crate::beef::Beef::from_bytes(&sent[0]).unwrap();
    assert_eq!(beef.transactions.last().map(|t| txid_hex(t)), Some(txid_hex(&subject)));
    assert!(beef.transactions.iter().any(|t| txid_hex(t) == txid_hex(&parent)), "the parent was not sent");
    assert!(mapi_sent.lock().unwrap().is_empty(), "ARC accepted on the first try; nothing should fall through");
    assert_eq!(st.status_calls.load(Ordering::SeqCst), 1, "only the existence check");
    assert_eq!(outputs_for(&state, &txid_hex(&subject)), 1);
}

/// `P7b-B1` arm 2 (two-sided): a subject already on chain is never broadcast.
#[actix_web::test]
async fn p7b_b1_a_subject_already_on_chain_is_not_broadcast() {
    let (arc, arc_sent) = arc_model();
    let (mapi, mapi_sent) = raw_only("gorillapool_mapi");
    let state = app_state_from(crate::classify::p5_tests::wallet("p7b_b1b"),
        WalletServices::with_test_chains(dynv(&[status_provider(TxState::Mined)]), dynv(&[arc, mapi])));
    let lock = { let db = state.database.lock().unwrap(); super::p7a_route_tests::our_lock(&db) };
    let (parent, subject) = pair(lock);

    let (code, body) = internalize_with_parents(&state, &[parent], &subject).await;

    assert_eq!(code, 200, "{}", body);
    assert!(arc_sent.lock().unwrap().is_empty() && mapi_sent.lock().unwrap().is_empty());
}

/// `P7b-B3` — an Atomic BEEF is sent to ARC as the BEEF inside it (ARC answers the header with
/// 400). ARC receives exactly `to_v1_bytes()` of the bundle and accepts on the first try.
#[tokio::test]
async fn p7b_b3_atomic_header_is_stripped_before_arc() {
    let (arc, arc_sent) = arc_model();
    let (mapi, mapi_sent) = raw_only("gorillapool_mapi");
    let state = app_state_with(WalletServices::with_test_chains(vec![], dynv(&[arc, mapi])));
    let (parent, subject) = pair(vec![0x51]);
    let beef = two_tx_beef(&parent, &subject);
    let txid = txid_hex(&subject);

    let r = broadcast_transaction_classified(&beef.to_atomic_beef_hex(&txid).unwrap(), &state.services, Some(crate::handlers::ProofSink::of(&state)), Some(&txid)).await;

    assert!(r.is_ok(), "{:?}", r.err().map(|f| f.reason));
    assert_eq!(arc_sent.lock().unwrap().clone(), vec![beef.to_v1_bytes().unwrap()], "ARC got something other than the inner BEEF");
    assert!(mapi_sent.lock().unwrap().is_empty());
}

/// `P7b-B4` — two-sided with B3: only the Atomic header is rewritten. V1, V2 and a raw tx reach the
/// first provider exactly as before (a raw tx still as the parentless wrapper).
#[tokio::test]
async fn p7b_b4_v1_v2_and_raw_inputs_pass_unchanged() {
    let (parent, subject) = pair(vec![0x51]);
    let beef = two_tx_beef(&parent, &subject);
    let txid = txid_hex(&subject);
    let mut wrapper = crate::beef::BEEF_V1_MARKER.to_vec();
    wrapper.extend([0x00, 0x01]);
    wrapper.extend(&subject);
    wrapper.push(0x00);
    for (input, expected) in [
        (beef.to_v1_hex().unwrap(), beef.to_v1_bytes().unwrap()),
        (beef.to_hex().unwrap(), beef.to_bytes().unwrap()),
        (hex::encode(&subject), wrapper),
    ] {
        let (first, sent) = raw_only("first");
        let state = app_state_with(WalletServices::with_test_chains(vec![], dynv(&[first])));
        let _ = broadcast_transaction_classified(&input, &state.services, Some(crate::handlers::ProofSink::of(&state)), Some(&txid)).await;
        assert_eq!(sent.lock().unwrap().clone(), vec![expected], "input {}… changed on the way", &input[..8]);
    }
}

/// `P7b-B5` — a truncated Atomic BEEF is a temporary error and reaches no provider; one with a
/// garbage inner bundle is forwarded, refused, and still temporary (never Ok, never a panic).
#[tokio::test]
async fn p7b_b5_malformed_atomic_input_is_temporary_and_never_ok() {
    for input in ["0101010100000000".to_string(), format!("01010101{}", "00".repeat(32))] {
        let (arc, arc_sent) = arc_model();
        let (mapi, mapi_sent) = raw_only("gorillapool_mapi");
        let state = app_state_with(WalletServices::with_test_chains(vec![], dynv(&[arc, mapi])));
        let r = broadcast_transaction_classified(&input, &state.services, Some(crate::handlers::ProofSink::of(&state)), Some("ab")).await;
        let f = r.expect_err("a truncated Atomic BEEF was accepted");
        assert!(!f.permanent && !f.double_spend, "our own bad bytes must never free coins: {}", f.reason);
        assert!(arc_sent.lock().unwrap().is_empty() && mapi_sent.lock().unwrap().is_empty(), "a truncated input reached a provider");
    }
    let (arc, arc_sent) = arc_model();
    let (mapi, _) = raw_only("gorillapool_mapi");
    let state = app_state_with(WalletServices::with_test_chains(vec![], dynv(&[arc, mapi])));
    let garbage = format!("01010101{}ff", "00".repeat(32));
    let f = broadcast_transaction_classified(&garbage, &state.services, Some(crate::handlers::ProofSink::of(&state)), Some("ab")).await.expect_err("garbage accepted");
    assert!(!f.permanent, "{}", f.reason);
    assert_eq!(arc_sent.lock().unwrap().clone(), vec![vec![0xffu8]], "the inner bytes are what is forwarded");
}

/// `P7b-B6` arm B — an own `createAction`-family broadcast (`broadcast_with_retries` with the Atomic
/// hex `create_action_internal_with` / `sign_action` pass) now reaches ARC as a V1 BEEF carrying
/// the parent and is accepted on the first attempt. Deliberate change (contract §3 decision 4):
/// before this phase ARC answered 400 and the send landed only through MAPI.
#[tokio::test]
async fn p7b_b6_own_send_reaches_arc_as_beef_with_its_parent() {
    let (arc, arc_sent) = arc_model();
    let (mapi, mapi_sent) = raw_only("gorillapool_mapi");
    let state = app_state_with(WalletServices::with_test_chains(vec![], dynv(&[arc, mapi])));
    let (parent, subject) = pair(vec![0x51]);
    let txid = txid_hex(&subject);
    let hex = two_tx_beef(&parent, &subject).to_atomic_beef_hex(&txid).unwrap();

    let r = broadcast_with_retries(&state, &hex, &txid, &BROADCAST_RETRY_POLICY).await;

    assert!(r.is_ok(), "{:?}", r.err().map(|f| f.reason));
    let sent = arc_sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(&sent[0][..4], &crate::beef::BEEF_V1_MARKER);
    assert!(crate::beef::Beef::from_bytes(&sent[0]).unwrap().transactions.contains(&parent));
    assert!(mapi_sent.lock().unwrap().is_empty(), "the winner must be ARC, not MAPI");
}

/// `P7b-B7` — the chains as `main.rs` builds them (`WalletServices::new()`, its only non-test
/// caller). Broadcast: ARC GorillaPool → Bitails → MAPI → WhatsOnChain, every one eligible, no TAAL.
/// Every other chain is the order this phase started from (`services/mod.rs` at `cb2c063`).
#[test]
fn p7b_b7_chain_order_from_the_production_constructor() {
    let orders = WalletServices::new().chain_orders();
    let full = |name: &str| orders.iter().find(|(n, _, _)| *n == name).unwrap().1.clone();
    let eligible = |name: &str| orders.iter().find(|(n, _, _)| *n == name).unwrap().2.clone();
    assert_eq!(full("broadcast"), vec!["arc_gorillapool", "bitails", "gorillapool_mapi", "whatsonchain"]);
    assert_eq!(eligible("broadcast"), full("broadcast"), "a provider in the broadcast chain is skipped");
    assert_eq!(full("raw_tx"), vec!["arc_gorillapool", "whatsonchain", "junglebus"]);
    assert_eq!(full("proof"), vec!["arc_gorillapool", "whatsonchain", "junglebus"]);
    assert_eq!(full("header"), vec!["whatsonchain", "junglebus"]);
    assert_eq!(full("tx_status"), vec!["arc_gorillapool", "whatsonchain", "junglebus", "bitails"]);
    assert_eq!(full("outspend"), vec!["whatsonchain", "junglebus"]);
    assert_eq!(full("utxo"), vec!["whatsonchain", "gorillapool_ordinals"]);
}

/// A local HTTP server standing in for Bitails: answers `answer(request text)` and records every
/// request. Stops accepting 2 s after the last connection.
async fn bitails_stand_in(answer: fn(&str) -> (u16, String)) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let handle = tokio::spawn(async move {
        let mut seen = Vec::new();
        while let Ok(Ok((mut sock, _))) = tokio::time::timeout(std::time::Duration::from_secs(2), listener.accept()).await {
            let mut buf = vec![0u8; 1 << 20];
            let mut n = 0;
            loop {
                let got = sock.read(&mut buf[n..]).await.unwrap();
                n += got;
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                if let Some(h) = text.find("\r\n\r\n") {
                    let len = text[..h].lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|x| x.trim().parse::<usize>().unwrap())).unwrap_or(0);
                    if n >= h + 4 + len { break; }
                }
                if got == 0 { break; }
            }
            let text = String::from_utf8_lossy(&buf[..n]).to_string();
            let (code, body) = answer(&text);
            let resp = format!("HTTP/1.1 {} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", code, body.len(), body);
            sock.write_all(resp.as_bytes()).await.unwrap();
            seen.push(text);
        }
        seen
    });
    (base, handle)
}

fn request_body(text: &str) -> serde_json::Value {
    serde_json::from_str(&text[text.find("\r\n\r\n").unwrap() + 4..]).unwrap_or(serde_json::Value::Null)
}

/// The Bitails fixture subject (shared by the stand-in's answers, which are plain `fn`s).
fn bitails_subject() -> (Vec<u8>, Vec<u8>) { pair(vec![0x51]) }

/// `P7b-B8` ⭐ + `P7b-B9` — GorillaPool unreachable ⇒ the REAL Bitails provider (against a stand-in
/// server) accepts. B9: the request is `POST /tx/broadcast/multi`, JSON `{"raws":[<subject hex>]}`,
/// no Authorization header. The stand-in accepts only that exact request.
#[tokio::test]
async fn p7b_b8_b9_gorillapool_down_bitails_accepts_the_raw_subject() {
    fn answer(req: &str) -> (u16, String) {
        let (_, s) = bitails_subject();
        let ok = req.starts_with("POST /tx/broadcast/multi ")
            && request_body(req) == serde_json::json!({ "raws": [hex::encode(&s)] });
        if ok { (201, format!("[{{\"txid\":\"{}\"}}]", txid_hex(&s))) } else { (500, "unexpected request".into()) }
    }
    let (base, seen) = bitails_stand_in(answer).await;
    let mut gp = ScriptedProvider::new("arc_gorillapool");
    gp.broadcast = Box::new(|_| Err(IndexerError::Transport("connection refused".into())));
    let bitails: Arc<dyn IndexerProvider> = Arc::new(crate::services::providers::BitailsProvider::with_base(reqwest::Client::new(), base));
    let (mapi, mapi_sent) = raw_only("gorillapool_mapi");
    let state = app_state_with(WalletServices::with_test_chains(vec![], vec![Arc::new(gp), bitails, mapi]));
    let (parent, subject) = bitails_subject();
    let txid = txid_hex(&subject);

    let r = broadcast_transaction_classified(&two_tx_beef(&parent, &subject).to_v1_hex().unwrap(), &state.services, Some(crate::handlers::ProofSink::of(&state)), Some(&txid)).await;

    assert!(r.as_ref().map(|s| s.starts_with("bitails")).unwrap_or(false), "{:?}", r.map_err(|f| f.reason));
    assert!(mapi_sent.lock().unwrap().is_empty());
    let seen = seen.await.unwrap();
    assert_eq!(seen.len(), 1);
    assert!(!seen[0].to_lowercase().contains("\r\nauthorization:"), "no key is sent to Bitails");
}

/// `P7b-B11` — Bitails' `-25` (missing inputs) reaches the chain verdict the way WhatsOnChain's
/// "Missing inputs" does. Alone it is a refusal (permanent); with a provider that may hold the
/// transaction it is not (two-sided).
#[tokio::test]
async fn p7b_b11_bitails_missing_inputs_is_classified_like_whatsonchain() {
    fn answer(_: &str) -> (u16, String) { (201, r#"[{"error":{"code":-25,"message":"missing-inputs"}}]"#.into()) }
    let (parent, subject) = bitails_subject();
    let hex = two_tx_beef(&parent, &subject).to_v1_hex().unwrap();
    let txid = txid_hex(&subject);

    let (base, _) = bitails_stand_in(answer).await;
    let bitails: Arc<dyn IndexerProvider> = Arc::new(crate::services::providers::BitailsProvider::with_base(reqwest::Client::new(), base));
    let state = app_state_with(WalletServices::with_test_chains(vec![], vec![bitails]));
    let f = broadcast_transaction_classified(&hex, &state.services, Some(crate::handlers::ProofSink::of(&state)), Some(&txid)).await.expect_err("-25 accepted");
    assert!(f.permanent, "Bitails' refusal alone must read as a refusal: {}", f.reason);

    let woc = crate::services::ProviderFailure { provider: "whatsonchain", kind: crate::services::FailureKind::Status(400), message: "provider whatsonchain returned status 400: Missing inputs".into() };
    let bt = crate::services::ProviderFailure { provider: "bitails", kind: crate::services::FailureKind::Status(201), message: f.reason.clone() };
    assert_eq!(provider_verdict(&bt), provider_verdict(&woc));

    let (base, _) = bitails_stand_in(answer).await;
    let bitails: Arc<dyn IndexerProvider> = Arc::new(crate::services::providers::BitailsProvider::with_base(reqwest::Client::new(), base));
    let mut slow = ScriptedProvider::new("arc_gorillapool");
    slow.broadcast = Box::new(|_| Err(IndexerError::Transport("timed out".into())));
    let state = app_state_with(WalletServices::with_test_chains(vec![], vec![Arc::new(slow), bitails]));
    let f = broadcast_transaction_classified(&hex, &state.services, Some(crate::handlers::ProofSink::of(&state)), Some(&txid)).await.expect_err("accepted");
    assert!(!f.permanent, "a provider that may hold the tx keeps the verdict temporary");
}

/// `P7b-S1` — the TAAL key and endpoint are gone from the wallet: no file of any type under
/// `rust-wallet/` (build output excluded) holds the key, and no source file names TAAL ARC.
/// Needles are assembled at runtime so this file does not match itself.
#[test]
fn p7b_s1_no_taal_key_or_endpoint_remains() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // Review 164 #13: the repository's docs are read for the key too (by its hash).
    let repo = root.parent().unwrap();
    let mut doc_stack = vec![repo.join("development-docs"), repo.join("README.md")];
    while let Some(path) = doc_stack.pop() {
        if path.is_dir() {
            for e in std::fs::read_dir(&path).unwrap().flatten() { doc_stack.push(e.path()); }
        } else if let Ok(bytes) = std::fs::read(&path) {
            assert!(!holds_key_in(&String::from_utf8_lossy(&bytes)), "the TAAL key is written out in {}", path.display());
        }
    }
    // The key itself is never written down here: it is found by the SHA-256 of any 40-character
    // `mainnet_` + 32-hex window (review LOW: a split literal still committed the secret).
    const KEY_SHA256: &str = "7afa3bf8ce92504a0d2db72395b0fc63d0a5aa2c00a93f480b9d74c1400bfcdf";
    fn holds_key_in(text: &str) -> bool {
        text.match_indices("mainnet_").any(|(i, _)| {
            use sha2::{Digest, Sha256};
            text.get(i..i + 40).map(|w| hex::encode(Sha256::digest(w.as_bytes())) == KEY_SHA256).unwrap_or(false)
        })
    }
    let endpoint = ["arc.", "taal.com"].concat();
    let provider = ["ArcTaal", "Provider"].concat();
    let module = ["arc", "_taal"].concat();
    let (mut offenders, mut rs_files, mut saw_services) = (vec![], 0usize, false);
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !matches!(path.file_name().and_then(|n| n.to_str()), Some("target") | Some(".git")) {
                    stack.push(path);
                }
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let text = String::from_utf8_lossy(&bytes);
            let is_source = matches!(path.extension().and_then(|e| e.to_str()), Some("rs") | Some("md"));
            if path.extension().and_then(|e| e.to_str()) == Some("rs") { rs_files += 1; }
            if path.ends_with("services/mod.rs") || path.ends_with("services\\mod.rs") { saw_services = true; }
            if holds_key_in(&text) || (is_source && (text.contains(&endpoint) || text.contains(&provider) || text.contains(&module))) {
                offenders.push(path.display().to_string());
            }
        }
    }
    assert!(rs_files >= 100 && saw_services, "the walk did not see the source tree ({} .rs files)", rs_files);
    assert!(offenders.is_empty(), "TAAL key or endpoint still in: {:?}", offenders);
}

/// The already-mined transaction the kickoff probed live (block 970,510) and its parent.
const LIVE_SUBJECT: &str = "7d9d3ab1f46f8287ba37be3d5263f5f030c049ba47628e9fd469f1643d6db6dd";

/// WhatsOnChain only (test fixture, read-only): raw tx, TSC proof, and the proof's block height.
async fn woc(path: &str) -> String {
    reqwest::get(format!("https://api.whatsonchain.com/v1/bsv/main{}", path)).await.unwrap().text().await.unwrap()
}

async fn live_broadcast_bytes() -> Vec<u8> {
    let subject = hex::decode(woc(&format!("/tx/{}/hex", LIVE_SUBJECT)).await.trim()).expect("subject raw");
    let parsed = crate::beef::ParsedTransaction::from_bytes(&subject).unwrap();
    let mut beef = crate::beef::Beef::new();
    for input in &parsed.inputs {
        let raw = hex::decode(woc(&format!("/tx/{}/hex", input.prev_txid)).await.trim()).expect("parent raw");
        let i = beef.add_parent_transaction(raw);
        let mut tsc: serde_json::Value = serde_json::from_str(&woc(&format!("/tx/{}/proof/tsc", input.prev_txid)).await).unwrap();
        if tsc.is_array() { tsc = tsc[0].clone(); }
        let block: serde_json::Value = serde_json::from_str(&woc(&format!("/block/hash/{}", tsc["target"].as_str().unwrap())).await).unwrap();
        tsc["height"] = block["height"].clone();
        beef.add_tsc_merkle_proof(&input.prev_txid, i, &tsc).expect("parent BUMP");
    }
    beef.set_main_transaction(subject);
    beef.broadcast_v1_for(LIVE_SUBJECT).unwrap()
}

/// `P7b-L1` (T2, live, read-only: the transaction is already mined) — the bytes `broadcast_v1_for`
/// builds from a real transaction whose parents carry their proofs are accepted by the REAL
/// GorillaPool ARC provider (it answers MINED / already known), calibrating the `arc_model` fake.
#[tokio::test]
#[ignore = "live network; run by hand: cargo test --bin hodos-wallet p7b_l1 -- --ignored"]
async fn p7b_l1_live_gorillapool_arc_accepts_our_broadcast_beef() {
    let bytes = live_broadcast_bytes().await;
    let arc = crate::services::providers::ArcGorillaPoolProvider::new(reqwest::Client::new());
    let r = arc.broadcast_beef(&bytes).await;
    println!("L1 ARC answer: {:?}", r);
    assert_eq!(r.expect("refused").txid, LIVE_SUBJECT);
}

/// `P7b-L2` (T2, live, read-only) — the REAL Bitails provider on the same mined transaction: Bitails
/// answers `-25 missing-inputs` (its inputs are spent), which must read as "Missing inputs".
#[tokio::test]
#[ignore = "live network; run by hand: cargo test --bin hodos-wallet p7b_l2 -- --ignored"]
async fn p7b_l2_live_bitails_answers_through_our_parser() {
    let bytes = live_broadcast_bytes().await;
    let bt = crate::services::providers::BitailsProvider::new(reqwest::Client::new());
    let r = bt.broadcast_beef(&bytes).await;
    println!("L2 Bitails answer: {:?}", r);
    let e = r.expect_err("a mined transaction re-sent is not 'accepted' by Bitails").to_string();
    assert!(e.contains("Missing inputs"), "{}", e);
}

/// Review 162 #7 — an Atomic BEEF reaches ARC built around the subject its header NAMES: a
/// transaction appended after the subject (spending it) is left out and the subject is last.
#[tokio::test]
async fn p7b_x7_the_atomic_strip_sends_the_named_subject_last() {
    let (arc, arc_sent) = arc_model();
    let state = app_state_with(WalletServices::with_test_chains(vec![], dynv(&[arc])));
    let (parent, subject) = pair(vec![0x51]);
    let extra = tx(&[(&subject, 0, 0xFFFF_FFFF)], &[(1_000, vec![0x51])], 0);
    let beef = crate::beef::Beef { version: crate::beef::BEEF_V1_MARKER, bumps: vec![],
        transactions: vec![parent.clone(), subject.clone(), extra], tx_to_bump: vec![None; 3] };
    let txid = txid_hex(&subject);
    let r = broadcast_transaction_classified(&beef.to_atomic_beef_hex(&txid).unwrap(), &state.services, Some(crate::handlers::ProofSink::of(&state)), Some(&txid)).await;
    assert!(r.is_ok(), "{:?}", r.err().map(|f| f.reason));
    let sent = crate::beef::Beef::from_bytes(&arc_sent.lock().unwrap()[0]).unwrap();
    assert_eq!(sent.transactions, vec![parent, subject], "the named subject must be last, the extra left out");
}

/// Review 162 #14 — provider error text is cut on a character boundary, never inside one.
#[test]
fn p7b_x14_long_provider_text_is_cut_safely() {
    let text = format!("a{}", "\u{e9}".repeat(150)); // byte 200 falls inside a 2-byte character
    let out = extract_core_error(&text);
    assert!(out.starts_with("Transaction broadcast failed: a") && out.ends_with("..."), "{}", out);
}

/// `P7b-L4` (T2, live, read-only; review 164 #1) — the REAL WhatsOnChain provider names the spender of
/// an output that is spent: the live subject's parent, output 1, spent by `7d9d3ab1…`. The route used
/// before this fix (`/outspend/`) answered 404 "page not found" for it, read as Unspent.
#[tokio::test]
#[ignore = "live network; run by hand: cargo test --bin hodos-wallet p7b_l4 -- --ignored"]
async fn p7b_l4_live_whatsonchain_names_the_spender() {
    let woc = crate::services::providers::WhatsOnChainProvider::new(reqwest::Client::new());
    let r = woc.outspend("2e20231462547cd41f8b4ae21d9bab075a821d090ee28a0e3fddaaf0a55721ad", 1).await;
    println!("L4 WoC answer: {:?}", r);
    assert!(matches!(r, Ok(crate::services::OutspendStatus::Spent { ref spending_txid, .. }) if spending_txid == LIVE_SUBJECT));
}

/// `P7b-L3` (T2, live — SPENDS real coins; sitting only) — a fresh signed transaction (`HODOS_L3_RAW`, from the
/// sitting sender's `--mode raw`: a scratch self-send) broadcast through the REAL Bitails provider as the only
/// provider is accepted, bound to its own txid. Records Bitails' live success answer, never seen before.
#[tokio::test]
#[ignore = "live, spends real coins; run in the sitting: HODOS_L3_RAW=<hex> cargo test --bin hodos-wallet p7b_l3 -- --ignored"]
async fn p7b_l3_live_bitails_alone_accepts_a_fresh_transaction() {
    let raw = hex::decode(std::env::var("HODOS_L3_RAW").expect("HODOS_L3_RAW").trim()).unwrap();
    let mut wrapper = crate::beef::BEEF_V1_MARKER.to_vec();
    wrapper.extend([0x00, 0x01]);
    wrapper.extend(&raw);
    wrapper.push(0x00);
    let bitails: Arc<dyn IndexerProvider> = Arc::new(crate::services::providers::BitailsProvider::new(reqwest::Client::new()));
    let state = app_state_with(WalletServices::with_test_chains(vec![], vec![bitails]));
    let txid = txid_hex(&raw);
    let r = broadcast_transaction_classified(&hex::encode(&wrapper), &state.services, Some(crate::handlers::ProofSink::of(&state)), Some(&txid)).await;
    println!("L3 Bitails: {:?}", r.as_ref().map_err(|f| &f.reason));
    assert!(r.as_ref().map(|s| s.starts_with("bitails")).unwrap_or(false), "{:?}", r.map_err(|f| f.reason));
}
