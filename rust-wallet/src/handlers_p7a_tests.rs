//! B5-T1-P7a-1 route rows through the real handlers, with the wallet's own header chain synced from
//! the real mainnet fixture (970,001 – 970,300, `tests/fixtures/headers/`) and no network.
use super::*;
use super::p2_test_support::{app_state_with, body_json};

const ARCADE_970001_970300: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/headers/arcade_970001_970300.bin"));

/// A wallet whose header chain holds the checkpoint + the fixture's first `n` headers.
/// `synced = false` leaves it as a fresh process before its first sync.
pub(crate) fn state_with_headers(n: usize, synced: bool) -> web::Data<AppState> {
    let state = app_state_with(crate::services::WalletServices::with_test_chains(vec![], vec![]));
    let headers = crate::header_chain::parse_headers(&ARCADE_970001_970300[..n * 80]).unwrap();
    let r = state.chain_tracker.ingest(&state.database, &headers, u32::MAX);
    assert_eq!((r.added, r.refused.clone()), (n, None));
    if synced {
        state.chain_tracker.mark_synced_for_test();
    }
    state
}

fn fixture_header(height: u32) -> String {
    let i = (height - 970_001) as usize;
    hex::encode(&ARCADE_970001_970300[i * 80..(i + 1) * 80])
}

// ---------- `P7a-C16`: getHeight ----------

#[actix_web::test]
async fn c16_get_height_is_the_verified_tip() {
    let state = state_with_headers(120, true);
    let (code, body) = body_json(get_height(state.clone(), web::Bytes::new()).await).await;
    assert_eq!((code, body["height"].as_u64()), (200, Some(970_120)), "{}", body);
}

#[actix_web::test]
async fn c16_get_height_before_any_sync_is_an_error_never_a_number() {
    let state = state_with_headers(0, false);
    let (code, body) = body_json(get_height(state.clone(), web::Bytes::new()).await).await;
    assert_ne!(code, 200, "{}", body);
    assert!(body.get("height").is_none(), "never a height (the checkpoint's would be months stale): {}", body);
}

// ---------- `P7a-C17`: getHeaderForHeight ----------

#[actix_web::test]
async fn c17_get_header_for_height_is_the_verified_80_bytes_not_a_cached_row() {
    let state = state_with_headers(120, true);
    // A legacy cache row with an empty header — the JungleBus shape — must not be what answers.
    {
        let db = state.database.lock().unwrap();
        crate::database::BlockHeaderRepository::new(db.connection()).upsert("ab".repeat(32).as_str(), 970_005, "").unwrap();
    }
    let req = web::Json(GetHeaderForHeightRequest { height: 970_005 });
    let (code, body) = body_json(get_header_for_height(state.clone(), req).await).await;
    assert_eq!(code, 200, "{}", body);
    assert_eq!(body["header"].as_str(), Some(fixture_header(970_005).as_str()));
}

#[actix_web::test]
async fn c17_a_height_we_do_not_hold_is_an_error_never_empty() {
    let state = state_with_headers(120, true);
    for h in [970_121u32, 900_000] {
        let req = web::Json(GetHeaderForHeightRequest { height: h });
        let (code, body) = body_json(get_header_for_height(state.clone(), req).await).await;
        assert_ne!(code, 200, "height {}: {}", h, body);
        assert!(body.get("header").is_none(), "height {}: {}", h, body);
    }
}

// ---------- internalizeAction: BRC-67 checks 2 + 4 and the BUMP check, before any chain call ----------
//
// Rig (RED design §0 F-a/F-b): the chain says "not found" and the broadcaster records what reaches it,
// so with a check removed the subject IS sent to the network — the RED is a broadcast, not a refusal
// for the wrong reason. Every refusal asserts the exact code AND that no status call and no broadcast
// happened AND that nothing was stored.

use super::p2_test_support::ScriptedProvider;
use crate::services::{IndexerError, IndexerProvider};

pub(super) fn our_lock(db: &crate::database::WalletDatabase) -> Vec<u8> {
    let addr: String = db.connection().query_row("SELECT address FROM addresses ORDER BY \"index\" LIMIT 1", [], |r| r.get(0)).unwrap();
    let decoded = bs58::decode(addr).into_vec().unwrap();
    [&[0x76, 0xa9, 0x14][..], &decoded[1..21], &[0x88, 0xac]].concat()
}

pub(super) fn sha256d(b: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(Sha256::digest(b)).to_vec()
}

pub(super) fn txid_hex(raw: &[u8]) -> String {
    let mut h = sha256d(raw);
    h.reverse();
    hex::encode(h)
}

/// A transaction with explicit inputs `(parent raw, vout, sequence)`, outputs `(value, script)` and lock.
pub(super) fn tx(inputs: &[(&[u8], u32, u32)], outputs: &[(u64, Vec<u8>)], lock_time: u32) -> Vec<u8> {
    let mut t = 1u32.to_le_bytes().to_vec();
    t.push(inputs.len() as u8);
    for (parent, vout, seq) in inputs {
        t.extend(sha256d(parent));
        t.extend(vout.to_le_bytes());
        t.push(0);
        t.extend(seq.to_le_bytes());
    }
    t.push(outputs.len() as u8);
    for (v, s) in outputs {
        t.extend(v.to_le_bytes());
        t.push(s.len() as u8);
        t.extend(s);
    }
    t.extend(lock_time.to_le_bytes());
    t
}

pub(super) fn funding(value: u64) -> Vec<u8> {
    tx(&[(&[0x42u8; 4][..], 0, 0xFFFF_FFFF)], &[(value, vec![0x51])], 0)
}

pub(super) struct Rig {
    pub(super) state: web::Data<AppState>,
    pub(super) p: std::sync::Arc<ScriptedProvider>,
    pub(super) lock: Vec<u8>,
}

pub(super) fn rig(tag: &str, synced: bool) -> Rig {
    let mut p = ScriptedProvider::new("fake");
    p.status_for = Box::new(|_| Err(IndexerError::NotFound));
    p.broadcast = Box::new(|_| Err(IndexerError::Transport("test: broadcast reached".into())));
    let p = std::sync::Arc::new(p);
    let dynp: std::sync::Arc<dyn IndexerProvider> = p.clone();
    let state = super::p2_test_support::app_state_from(crate::classify::p5_tests::wallet(tag),
        crate::services::WalletServices::with_test_chains(vec![dynp.clone()], vec![dynp]));
    let headers = crate::header_chain::parse_headers(&ARCADE_970001_970300[..120 * 80]).unwrap();
    assert_eq!(state.chain_tracker.ingest(&state.database, &headers, u32::MAX).added, 120);
    if synced {
        state.chain_tracker.mark_synced_for_test();
    }
    let lock = { let db = state.database.lock().unwrap(); our_lock(&db) };
    Rig { state, p, lock }
}

pub(super) async fn internalize_beef(state: &web::Data<AppState>, beef: crate::beef::Beef, subject: &[u8]) -> (u16, serde_json::Value) {
    let atomic = hex::decode(beef.to_atomic_beef_hex(&txid_hex(subject)).unwrap()).unwrap();
    let req: InternalizeActionRequest = serde_json::from_value(serde_json::json!({
        "tx": atomic, "description": "B5-T1-P7a-1 route test",
        "outputs": [{ "outputIndex": 0, "protocol": "basket insertion", "insertionRemittance": { "basket": "todo" } }],
    })).unwrap();
    body_json(internalize_action(state.clone(), web::Json(req)).await).await
}

pub(super) async fn internalize_with_parents(state: &web::Data<AppState>, parents: &[Vec<u8>], subject: &[u8]) -> (u16, serde_json::Value) {
    let mut beef = crate::beef::Beef::new();
    for p in parents {
        beef.add_parent_transaction(p.clone());
    }
    beef.set_main_transaction(subject.to_vec());
    internalize_beef(state, beef, subject).await
}

pub(super) fn outputs_for(state: &web::Data<AppState>, txid: &str) -> i64 {
    state.database.lock().unwrap().connection().query_row("SELECT COUNT(*) FROM outputs WHERE txid = ?1", [txid], |r| r.get(0)).unwrap()
}

/// The common GREEN for a refusal: exact code, nothing asked of the chain, nothing sent, nothing stored.
fn assert_refused_before_the_chain(r: &Rig, got: (u16, serde_json::Value), code: &str, subject: &[u8]) {
    assert_eq!(got.1["code"].as_str(), Some(code), "{}", got.1);
    assert_eq!(r.p.status_calls.load(std::sync::atomic::Ordering::SeqCst), 0, "no chain call before the refusal");
    assert!(r.p.broadcast_hashes().is_empty(), "nothing broadcast");
    assert_eq!(outputs_for(&r.state, &txid_hex(subject)), 0, "nothing stored");
}

#[actix_web::test]
async fn b1_route_1000_in_1001_out_is_refused_before_any_chain_call() {
    let r = rig("p7a-b1", true);
    let parent = funding(1_000);
    let subject = tx(&[(&parent, 0, 0xFFFF_FFFF)], &[(1_001, r.lock.clone())], 0);
    let got = internalize_with_parents(&r.state, &[parent], &subject).await;
    assert_eq!(got.0, 400);
    assert_refused_before_the_chain(&r, got, "ERR_FEE", &subject);
}

#[actix_web::test]
async fn b4_route_a_missing_parent_is_refused_before_any_chain_call() {
    let r = rig("p7a-b4", true);
    let parent = funding(100_000);
    let subject = tx(&[(&parent, 0, 0xFFFF_FFFF)], &[(5_000, r.lock.clone())], 0);
    let got = internalize_with_parents(&r.state, &[], &subject).await;
    assert_refused_before_the_chain(&r, got, "ERR_MISSING_PARENT", &subject);
}

#[actix_web::test]
async fn b11_route_a_far_future_lock_with_a_non_final_sequence_is_refused() {
    let r = rig("p7a-b11", true);
    let parent = funding(100_000);
    let subject = tx(&[(&parent, 0, 0)], &[(5_000, r.lock.clone())], 970_120 + 10_000);
    let got = internalize_with_parents(&r.state, &[parent], &subject).await;
    assert_eq!(got.0, 400);
    assert_refused_before_the_chain(&r, got, "ERR_NONFINAL", &subject);
}

#[actix_web::test]
async fn b15_route_unsynced_headers_cannot_judge_a_lock() {
    let r = rig("p7a-b15", false);
    let parent = funding(100_000);
    let subject = tx(&[(&parent, 0, 0)], &[(5_000, r.lock.clone())], 970_050);
    let got = internalize_with_parents(&r.state, &[parent], &subject).await;
    assert_eq!(got.0, 503, "retryable, not a verdict");
    assert_refused_before_the_chain(&r, got, "ERR_HEADERS_UNAVAILABLE", &subject);
}

#[actix_web::test]
async fn b20_route_control_a_normal_payment_passes_the_checks_and_reaches_the_chain() {
    let r = rig("p7a-b20", true);
    let parent = funding(100_000);
    let subject = tx(&[(&parent, 0, 0xFFFF_FFFF)], &[(5_000, r.lock.clone())], 0);
    let (_, body) = internalize_with_parents(&r.state, &[parent], &subject).await;
    for c in ["ERR_FEE", "ERR_MISSING_PARENT", "ERR_NONFINAL", "ERR_HEADERS_UNAVAILABLE", "ERR_BAD_PROOF"] {
        assert_ne!(body["code"].as_str(), Some(c), "{}", body);
    }
    assert_eq!(r.p.status_calls.load(std::sync::atomic::Ordering::SeqCst), 1, "it reached the chain check");
    assert_eq!(r.p.broadcast_hashes().len(), 1, "and the broadcast — so the refusals above can see one");
}

fn fx2_nodes() -> Vec<String> {
    crate::spv_checks::proof_tests::FX2_NODES.iter().map(|s| s.to_string()).collect()
}

#[actix_web::test]
async fn c1_route_a_bump_with_a_wrong_root_is_refused_before_any_chain_call() {
    use crate::spv_checks::proof_tests::{fx2_beef, FX2_HEIGHT, FX2_RAW};
    let r = rig("p7a-c1", true);
    let mut nodes = fx2_nodes();
    nodes[4].replace_range(0..2, "68");
    let subject = hex::decode(FX2_RAW).unwrap();
    let got = internalize_beef(&r.state, fx2_beef(FX2_HEIGHT, &nodes), &subject).await;
    assert_eq!(got.0, 400);
    assert_refused_before_the_chain(&r, got, "ERR_BAD_PROOF", &subject);
}

#[actix_web::test]
async fn b16_route_a_subject_proven_against_our_headers_needs_no_parents() {
    use crate::spv_checks::proof_tests::{fx2_beef, FX2_HEIGHT, FX2_RAW};
    let r = rig("p7a-b16", true);
    let subject = hex::decode(FX2_RAW).unwrap();
    let (_, body) = internalize_beef(&r.state, fx2_beef(FX2_HEIGHT, &fx2_nodes()), &subject).await;
    assert_ne!(body["code"].as_str(), Some("ERR_MISSING_PARENT"), "a mined subject's BEEF may omit parents: {}", body);
    assert_ne!(body["code"].as_str(), Some("ERR_BAD_PROOF"), "{}", body);
    assert_eq!(r.p.status_calls.load(std::sync::atomic::Ordering::SeqCst), 1, "it went on to the chain check");
}

#[actix_web::test]
async fn b16_route_an_unknown_block_falls_back_to_requiring_parents() {
    use crate::spv_checks::proof_tests::{fx2_beef, FX2_RAW};
    let r = rig("p7a-b16u", true);
    let subject = hex::decode(FX2_RAW).unwrap();
    let got = internalize_beef(&r.state, fx2_beef(970_400, &fx2_nodes()), &subject).await;
    assert_refused_before_the_chain(&r, got, "ERR_MISSING_PARENT", &subject);
}

// ---------- `P7a-C19`: createAction's inputBEEF proofs ----------

async fn create_with_input_beef(r: &Rig, beef: &crate::beef::Beef) -> (u16, serde_json::Value) {
    let req = serde_json::json!({ "description": "B5-T1-P7a-1 inputBEEF", "inputBEEF": beef.to_v1_hex().unwrap(),
        "outputs": [{ "satoshis": 5000, "lockingScript": hex::encode(&r.lock), "outputDescription": "x" }] });
    let http = actix_web::test::TestRequest::default().to_http_request();
    body_json(create_action(r.state.clone(), http, web::Bytes::from(req.to_string())).await).await
}

#[actix_web::test]
async fn c19_an_input_beef_with_a_wrong_root_is_refused_before_signing() {
    use crate::spv_checks::proof_tests::{fx2_beef, FX2_HEIGHT};
    let r = rig("p7a-c19", true);
    let mut nodes = fx2_nodes();
    nodes[4].replace_range(0..2, "68");
    let (code, body) = create_with_input_beef(&r, &fx2_beef(FX2_HEIGHT, &nodes)).await;
    assert_eq!((code, body["code"].as_str()), (400, Some("ERR_BAD_PROOF")), "{}", body);
    assert!(r.p.broadcast_hashes().is_empty());
}

#[actix_web::test]
async fn c19_control_a_real_input_beef_proof_is_not_refused() {
    use crate::spv_checks::proof_tests::{fx2_beef, FX2_HEIGHT};
    let r = rig("p7a-c19c", true);
    let (_, body) = create_with_input_beef(&r, &fx2_beef(FX2_HEIGHT, &fx2_nodes())).await;
    assert_ne!(body["code"].as_str(), Some("ERR_BAD_PROOF"), "{}", body);
    // SUBJECT: it went past the inputBEEF check to the next step (this empty wallet cannot fund it).
    assert!(body["error"].as_str().map_or(false, |e| e.contains("Insufficient funds")), "{}", body);
}

/// `P7a-B20` as a crediting control: with the chain saying "mined", a normal payment that passes the
/// new checks is credited with exactly the fixture's values.
#[actix_web::test]
async fn b20_route_control_a_normal_payment_is_credited_unchanged() {
    let mut p = ScriptedProvider::new("fake");
    p.status_for = Box::new(|t| Ok(super::p2_test_support::status(t, crate::services::TxState::Mined, None)));
    let dynp: std::sync::Arc<dyn IndexerProvider> = std::sync::Arc::new(p);
    let state = super::p2_test_support::app_state_from(crate::classify::p5_tests::wallet("p7a-b20c"),
        crate::services::WalletServices::with_test_chains(vec![dynp], vec![]));
    state.chain_tracker.mark_synced_for_test();
    let lock = { let db = state.database.lock().unwrap(); our_lock(&db) };
    let parent = funding(100_000);
    let subject = tx(&[(&parent, 0, 0xFFFF_FFFF)], &[(5_000, lock.clone())], 0);
    let (code, body) = internalize_with_parents(&state, &[parent], &subject).await;
    assert_eq!(code, 200, "{}", body);
    let (vout, sats, basket): (i64, i64, Option<String>) = state.database.lock().unwrap().connection().query_row(
        "SELECT o.vout, o.satoshis, b.name FROM outputs o LEFT JOIN output_baskets b ON o.basket_id = b.basketId WHERE o.txid = ?1",
        [txid_hex(&subject)], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!((vout, sats, basket.as_deref()), (0, 5_000, Some("todo")));
}
