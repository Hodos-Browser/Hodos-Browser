//! B5-T1-P7a-2 rows through the real proof writers, with the wallet's own header chain holding the
//! real mainnet headers 970,001 – 970,300 (`tests/fixtures/headers/`) and no network: every task row
//! gets a client whose every request fails (a dead proxy), so no old WhatsOnChain check can answer.
//!
//! Fixture FX2: a real mainnet transaction at index 3 of block 970,100 (`spv_checks::proof_tests`).
//! The three proofs every row uses:
//! - **real** — FX2's own nodes at 970,100 → Valid against our header;
//! - **wrong** — one sibling byte flipped → Invalid (we hold 970,100 and its root differs);
//! - **unheld** — the real nodes claimed at 970,400 → Unknown (we hold up to 970,300).
use super::*;
use super::p2_test_support::{app_state_with, status, ScriptedProvider};
use crate::header_chain::RootVerdict;
use crate::services::{BroadcastResult, TxState};
use crate::spv_checks::proof_tests::{FX2_HEIGHT, FX2_INDEX, FX2_NODES, FX2_RAW, FX2_TXID};
use std::sync::atomic::Ordering;
use std::sync::Arc;

const ARCADE_970001_970300: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/headers/arcade_970001_970300.bin"));
const UNHELD: u32 = 970_400;

/// A wallet whose header chain holds the checkpoint + the real headers up to 970,000 + `n`, with
/// `provider` answering every service chain.
fn state_with(n: usize, provider: Arc<ScriptedProvider>) -> web::Data<AppState> {
    let state = app_state_with(crate::services::WalletServices::with_one_test_provider(provider));
    let headers = crate::header_chain::parse_headers(&ARCADE_970001_970300[..n * 80]).unwrap();
    assert_eq!(state.chain_tracker.ingest(&state.database, &headers, u32::MAX).added, n);
    state.chain_tracker.mark_synced_for_test();
    // ⛔ F2-a precondition: the real root really is Valid here, so an Invalid or Unknown result in a
    // row is the proof's doing, not an empty chain's.
    let root = fx2_root(&real());
    assert_eq!(state.chain_tracker.is_valid_root_for_height(&root, FX2_HEIGHT), RootVerdict::Valid);
    state
}

fn real() -> Vec<String> {
    FX2_NODES.iter().map(|s| s.to_string()).collect()
}

/// One sibling byte flipped: still a well-formed proof (it parses and round-trips), wrong root.
fn wrong() -> Vec<String> {
    let mut n = real();
    n[4].replace_range(0..2, "68");
    n
}

/// FX2's root from `nodes`, computed here with our own sha256d — never by the code under test.
fn fx2_root(nodes: &[String]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let d = |b: &[u8]| -> [u8; 32] { Sha256::digest(Sha256::digest(b)).into() };
    let mut h: [u8; 32] = { let mut t = hex::decode(FX2_TXID).unwrap(); t.reverse(); t.try_into().unwrap() };
    let mut i = FX2_INDEX;
    for n in nodes {
        let mut s: [u8; 32] = { let mut t = hex::decode(n).unwrap(); t.reverse(); t.try_into().unwrap() };
        if n == "*" { s = h; }
        let mut cat = Vec::with_capacity(64);
        if i & 1 == 0 { cat.extend_from_slice(&h); cat.extend_from_slice(&s); } else { cat.extend_from_slice(&s); cat.extend_from_slice(&h); }
        h = d(&cat);
        i >>= 1;
    }
    h
}

fn tsc(height: u32, nodes: &[String]) -> serde_json::Value {
    serde_json::json!({ "index": FX2_INDEX, "nodes": nodes, "height": height })
}

fn bump_hex(height: u32, nodes: &[String]) -> String {
    let json: Vec<serde_json::Value> = nodes.iter().map(|n| serde_json::json!(n)).collect();
    crate::beef::bump_to_hex(&crate::beef::tsc_proof_to_bump(FX2_TXID, height, FX2_INDEX, &json).unwrap()).unwrap()
}

/// FX2 as one of our transactions, `age_secs` old, with an open `proven_tx_reqs` row.
fn seed_tx(state: &AppState, txid: &str, tx_status: &str, age_secs: i64) {
    let db = state.database.lock().unwrap();
    let c = db.connection();
    c.execute(
        "INSERT INTO transactions (txid, reference_number, raw_tx, status, is_outgoing, satoshis, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 1, -1000, strftime('%s','now') - ?5, 0)",
        rusqlite::params![txid, uuid::Uuid::new_v4().to_string(), FX2_RAW, tx_status, age_secs],
    ).unwrap();
    let tid = c.last_insert_rowid();
    c.execute(
        "INSERT INTO outputs (user_id, transaction_id, spendable, change, vout, satoshis, txid, created_at, updated_at)
         VALUES (1, ?1, 1, 1, 0, 599, ?2, 0, 0)",
        rusqlite::params![tid, txid],
    ).unwrap();
    crate::database::ProvenTxReqRepository::new(c).create(txid, &hex::decode(FX2_RAW).unwrap(), None, "unproven").unwrap();
}

fn proven(state: &AppState, txid: &str) -> Vec<(u32, Vec<u8>)> {
    let db = state.database.lock().unwrap();
    let mut st = db.connection().prepare("SELECT height, merkle_path FROM proven_txs WHERE txid = ?1").unwrap();
    st.query_map([txid], |r| Ok((r.get::<_, i64>(0)? as u32, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect()
}

fn status_of(state: &AppState, txid: &str) -> String {
    state.database.lock().unwrap().connection()
        .query_row("SELECT status FROM transactions WHERE txid = ?1", [txid], |r| r.get(0)).unwrap()
}

fn req_of(state: &AppState, txid: &str) -> (String, String) {
    state.database.lock().unwrap().connection()
        .query_row("SELECT status, COALESCE(history, '') FROM proven_tx_reqs WHERE txid = ?1", [txid], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
}

fn spendable_outputs(state: &AppState, txid: &str) -> i64 {
    state.database.lock().unwrap().connection()
        .query_row("SELECT COALESCE(SUM(spendable), 0) FROM outputs WHERE txid = ?1", [txid], |r| r.get(0)).unwrap()
}

/// Every request fails: no row may be settled by an old network check.
fn dead_client() -> reqwest::Client {
    reqwest::Client::builder().proxy(reqwest::Proxy::all("http://127.0.0.1:1").unwrap()).build().unwrap()
}

/// A provider that says MINED at `height` with `nodes` as its BUMP (`None`: MINED, no BUMP).
fn mined(height: u32, nodes: Option<Vec<String>>) -> Arc<ScriptedProvider> {
    let mut p = ScriptedProvider::new("TEST");
    p.status_for = Box::new(move |t| {
        let mut s = status(t, TxState::Mined, Some("MINED"));
        s.block_height = Some(height);
        s.merkle_path_bump = nodes.as_ref().map(|n| bump_hex(height, n));
        Ok(s)
    });
    Arc::new(p)
}

// ───────────── P7a-C3: the broadcaster's proof (accept_broadcast_result → cache_arc_merkle_proof) ─────────────

async fn accept_mined(state: &AppState, height: u32, nodes: &[String]) -> Result<String, String> {
    let br = BroadcastResult { provider: "TEST", txid: FX2_TXID.to_string(), tx_status: "MINED".to_string(),
                               merkle_path_bump: Some(bump_hex(height, nodes)), block_height: Some(height) };
    accept_broadcast_result(br, Some(ProofSink::of(&state)), Some(FX2_TXID), |_| async {
        panic!("a bound acknowledgement must not be verified on the network")
    }).await
}

#[actix_web::test]
async fn c3_a_wrong_broadcaster_proof_is_not_stored_and_does_not_complete() {
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    seed_tx(&state, FX2_TXID, "sending", 0);
    assert!(accept_mined(&state, FX2_HEIGHT, &wrong()).await.is_ok());
    assert_eq!(proven(&state, FX2_TXID).len(), 0, "a proof our header refutes was stored");
    assert_ne!(status_of(&state, FX2_TXID), "completed");
    assert_ne!(req_of(&state, FX2_TXID).0, "completed");
}

#[actix_web::test]
async fn c3_an_unheld_broadcaster_proof_is_not_stored_and_does_not_complete() {
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    seed_tx(&state, FX2_TXID, "sending", 0);
    assert!(accept_mined(&state, UNHELD, &real()).await.is_ok());
    assert_eq!(proven(&state, FX2_TXID).len(), 0, "a proof for a block we do not hold was stored as verified");
    assert_eq!(status_of(&state, FX2_TXID), "unproven", "MINED without a verified proof is unproven");
    assert_ne!(req_of(&state, FX2_TXID).0, "completed");
}

#[actix_web::test]
async fn c3_a_real_broadcaster_proof_is_stored_once_and_completes() {
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    seed_tx(&state, FX2_TXID, "sending", 0);
    assert!(accept_mined(&state, FX2_HEIGHT, &real()).await.is_ok());
    let rows = proven(&state, FX2_TXID);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, FX2_HEIGHT);
    let stored: serde_json::Value = serde_json::from_slice(&rows[0].1).unwrap();
    assert_eq!(stored["nodes"], tsc(FX2_HEIGHT, &real())["nodes"], "the stored proof is the broadcaster's");
    assert_eq!(status_of(&state, FX2_TXID), "completed");
    assert_eq!(req_of(&state, FX2_TXID).0, "completed");
}

// ───────────── P7a-C4 / C4b / C14-a / C21: the proof task (task_check_for_proofs::run) ─────────────

async fn run_proof_task(state: &web::Data<AppState>) {
    crate::monitor::task_check_for_proofs::run(state, &dead_client()).await.unwrap();
}

#[actix_web::test]
async fn c4_a_wrong_proof_from_the_provider_is_not_stored_and_does_not_complete() {
    let state = state_with(120, mined(FX2_HEIGHT, Some(wrong())));
    seed_tx(&state, FX2_TXID, "unproven", 0);
    run_proof_task(&state).await;
    assert_eq!(proven(&state, FX2_TXID).len(), 0, "stored a proof our header refutes");
    assert_ne!(status_of(&state, FX2_TXID), "completed", "completed without a verified proof");
    let (req, history) = req_of(&state, FX2_TXID);
    assert_ne!(req, "completed");
    assert!(!history.contains("verified"), "the history claims a verification that did not happen: {}", history);
}

#[actix_web::test]
async fn c4_a_real_proof_from_the_provider_is_stored_and_completes() {
    let state = state_with(120, mined(FX2_HEIGHT, Some(real())));
    seed_tx(&state, FX2_TXID, "unproven", 0);
    run_proof_task(&state).await;
    assert_eq!(proven(&state, FX2_TXID).iter().map(|r| r.0).collect::<Vec<_>>(), vec![FX2_HEIGHT]);
    assert_eq!(status_of(&state, FX2_TXID), "completed");
    assert!(req_of(&state, FX2_TXID).1.contains("verified"));
}

#[actix_web::test]
async fn c4b_an_unheld_proof_is_kept_pending_and_asked_again() {
    let p = mined(UNHELD, Some(real()));
    let state = state_with(120, p.clone());
    seed_tx(&state, FX2_TXID, "unproven", 0);
    run_proof_task(&state).await;
    assert_eq!(proven(&state, FX2_TXID).len(), 0, "stored before our chain holds the block");
    assert_eq!(status_of(&state, FX2_TXID), "unproven");
    let req = req_of(&state, FX2_TXID).0;
    assert!(req != "completed" && req != "failed" && req != "invalid", "the pending proof was settled: {}", req);
    run_proof_task(&state).await;
    assert_eq!(p.status_calls.load(Ordering::SeqCst), 2, "the next pass must ask again");
}

/// MINED with no BUMP (every provider but ARC), and the proof chain answering `proof`.
fn mined_no_bump(proof: Result<serde_json::Value, ()>) -> Arc<ScriptedProvider> {
    let mut p = ScriptedProvider::new("TEST");
    p.status_for = Box::new(|t| {
        let mut s = status(t, TxState::Mined, Some("MINED"));
        s.block_height = Some(FX2_HEIGHT);
        Ok(s)
    });
    p.proof_for = Box::new(move |_| proof.clone().map_err(|_| crate::services::IndexerError::NotFound));
    Arc::new(p)
}

#[actix_web::test]
async fn c14a_mined_with_no_proof_is_unproven_not_completed() {
    let state = state_with(120, mined_no_bump(Err(())));
    seed_tx(&state, FX2_TXID, "sending", 0);
    run_proof_task(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "unproven");
}

/// Review #1: only ARC sends a BUMP with its status; for anything else the proof chain is asked, or
/// a mined transaction would stay unproven forever.
#[actix_web::test]
async fn c14a_mined_without_a_bump_asks_the_proof_chain_and_completes() {
    let p = mined_no_bump(Ok(woc_tsc(&fx2_block_hash(), &real())));
    let state = state_with(120, p.clone());
    seed_tx(&state, FX2_TXID, "sending", 0);
    run_proof_task(&state).await;
    assert_eq!(p.proof_calls.load(Ordering::SeqCst), 1, "the proof chain was not asked");
    assert_eq!(status_of(&state, FX2_TXID), "completed");
    assert_eq!(proven(&state, FX2_TXID).iter().map(|r| r.0).collect::<Vec<_>>(), vec![FX2_HEIGHT]);
}

#[actix_web::test]
async fn c21_a_mined_transaction_past_its_timeout_is_never_failed_while_its_proof_is_unheld() {
    let state = state_with(120, mined(UNHELD, Some(real())));
    seed_tx(&state, FX2_TXID, "unproven", 7 * 3600);
    run_proof_task(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "unproven", "a mined payment was failed by the timeout");
    assert_eq!(spendable_outputs(&state, FX2_TXID), 1, "its outputs were taken out of the balance");
}

#[actix_web::test]
async fn c21_a_nosend_transaction_reported_mined_moves_to_unproven_and_stays() {
    let state = state_with(120, mined(UNHELD, Some(real())));
    seed_tx(&state, FX2_TXID, "nosend", 11 * 60);
    run_proof_task(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "unproven");
    assert_eq!(spendable_outputs(&state, FX2_TXID), 1);
}

/// Two-sided with the rows above: a timed-out transaction no provider reports mined still fails
/// (the timeout keeps its teeth).
#[actix_web::test]
async fn c21_control_a_timed_out_transaction_nobody_reports_mined_still_fails() {
    let mut p = ScriptedProvider::new("TEST");
    p.status_for = Box::new(|t| Ok(status(t, TxState::Unknown, None)));
    let state = state_with(120, Arc::new(p));
    seed_tx(&state, FX2_TXID, "unproven", 7 * 3600);
    run_proof_task(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "failed");
}

// ───────────── shared for the batch below ─────────────

/// FX2's block hash (display hex), computed here from the fixture header bytes.
fn fx2_block_hash() -> String {
    use sha2::{Digest, Sha256};
    let i = (FX2_HEIGHT - 970_001) as usize;
    let mut h: [u8; 32] = Sha256::digest(Sha256::digest(&ARCADE_970001_970300[i * 80..(i + 1) * 80])).into();
    h.reverse();
    hex::encode(h)
}

/// A WhatsOnChain-shaped proof: the block named by hash, no height.
fn woc_tsc(target: &str, nodes: &[String]) -> serde_json::Value {
    serde_json::json!({ "index": FX2_INDEX, "nodes": nodes, "target": target })
}

/// A provider that answers proof requests with `proof` and raw-tx requests for FX2 (others: not found).
fn proof_provider(proof: serde_json::Value) -> ScriptedProvider {
    let mut p = ScriptedProvider::new("TEST");
    p.proof_for = Box::new(move |_| Ok(proof.clone()));
    p.raw_tx_for = Box::new(|t| if t == FX2_TXID { Ok(hex::decode(FX2_RAW).unwrap()) } else { Err(crate::services::IndexerError::NotFound) });
    p.status_for = Box::new(|t| Ok(status(t, TxState::Unknown, None)));
    p
}

fn plant_proof(state: &AppState, height: u32, tsc: &serde_json::Value) {
    let db = state.database.lock().unwrap();
    crate::database::ProvenTxRepository::new(db.connection())
        .insert_or_get(FX2_TXID, height, FX2_INDEX, &serde_json::to_vec(tsc).unwrap(), &hex::decode(FX2_RAW).unwrap(), "", "").unwrap();
}

fn set_failed(state: &AppState) {
    state.database.lock().unwrap().connection().execute(
        "UPDATE transactions SET status = 'failed', failed_at = strftime('%s','now') WHERE txid = ?1", [FX2_TXID]).unwrap();
}

// ───────────── P7a-C12: the check itself ─────────────

#[actix_web::test]
async fn c12_check_is_three_way_against_our_own_headers_with_no_network() {
    use crate::proof_store::{check_tsc, ProofCheck};
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    let chain = &*state.chain_tracker;
    assert!(matches!(check_tsc(chain, FX2_TXID, &tsc(FX2_HEIGHT, &real())), ProofCheck::Verified(ref p) if p.height() == FX2_HEIGHT));
    assert!(matches!(check_tsc(chain, FX2_TXID, &tsc(FX2_HEIGHT, &wrong())), ProofCheck::Wrong { height: FX2_HEIGHT }));
    assert!(matches!(check_tsc(chain, FX2_TXID, &tsc(UNHELD, &real())), ProofCheck::NotHeld));
    // A block named by hash: its height is our chain's, not anyone's claim.
    assert!(matches!(check_tsc(chain, FX2_TXID, &woc_tsc(&fx2_block_hash(), &real())), ProofCheck::Verified(ref p) if p.height() == FX2_HEIGHT));
    assert!(matches!(check_tsc(chain, FX2_TXID, &woc_tsc(&"ab".repeat(32), &real())), ProofCheck::NotHeld));
    // The right proof for another transaction does not match.
    let other = "11".repeat(32);
    assert!(matches!(check_tsc(chain, &other, &tsc(FX2_HEIGHT, &real())), ProofCheck::Wrong { .. }));
}

// ───────────── P7a-C13: a proof that does not contain the transaction is an error ─────────────

#[actix_web::test]
async fn c13_a_proof_that_fails_its_round_trip_is_an_error_not_a_proof() {
    let garbage = serde_json::json!({ "index": FX2_INDEX, "nodes": ["zz"], "height": FX2_HEIGHT });
    let state = state_with(120, Arc::new(proof_provider(garbage)));
    let r = crate::cache_helpers::fetch_tsc_proof_from_api(&state.services, FX2_TXID).await;
    assert!(r.is_err(), "a malformed proof came back as a proof: {:?}", r);
}

#[actix_web::test]
async fn c13_control_not_found_is_still_no_proof_yet() {
    let mut p = ScriptedProvider::new("TEST");
    p.proof_for = Box::new(|_| Err(crate::services::IndexerError::NotFound));
    let state = state_with(120, Arc::new(p));
    assert!(matches!(crate::cache_helpers::fetch_tsc_proof_from_api(&state.services, FX2_TXID).await, Ok(None)));
}

// ───────────── P7a-C6 / C7: recovering a failed transaction (task_unfail) ─────────────

async fn run_unfail(state: &web::Data<AppState>) {
    crate::monitor::task_unfail::run(state, &dead_client()).await.unwrap();
}

#[actix_web::test]
async fn c6_a_wrong_or_unheld_proof_never_recovers_a_failed_transaction() {
    for (height, nodes) in [(FX2_HEIGHT, wrong()), (UNHELD, real())] {
        let state = state_with(120, mined(height, Some(nodes)));
        seed_tx(&state, FX2_TXID, "unproven", 0);
        set_failed(&state);
        run_unfail(&state).await;
        assert_eq!(status_of(&state, FX2_TXID), "failed", "recovered on a proof our headers do not verify (h {})", height);
        assert_eq!(proven(&state, FX2_TXID).len(), 0);
    }
}

#[actix_web::test]
async fn c6_control_a_real_proof_recovers_it() {
    let state = state_with(120, mined(FX2_HEIGHT, Some(real())));
    seed_tx(&state, FX2_TXID, "unproven", 0);
    set_failed(&state);
    run_unfail(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "completed");
    assert_eq!(proven(&state, FX2_TXID).len(), 1);
}

fn in_mempool() -> Arc<ScriptedProvider> {
    let mut p = ScriptedProvider::new("TEST");
    p.status_for = Box::new(|t| Ok(status(t, TxState::InMempool, None)));
    Arc::new(p)
}

#[actix_web::test]
async fn c7_a_stored_wrong_proof_does_not_recover_and_is_not_deleted() {
    let state = state_with(120, in_mempool());
    seed_tx(&state, FX2_TXID, "unproven", 0);
    set_failed(&state);
    plant_proof(&state, FX2_HEIGHT, &tsc(FX2_HEIGHT, &wrong()));
    run_unfail(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "failed", "a stored row's existence recovered it");
    assert_eq!(proven(&state, FX2_TXID).len(), 1, "Invalid never deletes (C22)");
}

#[actix_web::test]
async fn c7_control_a_stored_real_proof_recovers_it() {
    let state = state_with(120, in_mempool());
    seed_tx(&state, FX2_TXID, "unproven", 0);
    set_failed(&state);
    plant_proof(&state, FX2_HEIGHT, &tsc(FX2_HEIGHT, &real()));
    run_unfail(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "completed");
}

// ───────────── P7a-C8: reconcile's promotion (promote_proven_local_tx) ─────────────

#[actix_web::test]
async fn c8_promotion_needs_a_verified_proof_and_stores_our_chains_height() {
    use crate::monitor::task_unfail::{promote_proven_local_tx, PromoteOutcome};
    let raw = hex::decode(FX2_RAW).unwrap();
    for (label, proof, want) in [
        ("wrong", woc_tsc(&fx2_block_hash(), &wrong()), PromoteOutcome::TransientFailed),
        ("unheld block", woc_tsc(&"ab".repeat(32), &real()), PromoteOutcome::TransientFailed),
        ("real", woc_tsc(&fx2_block_hash(), &real()), PromoteOutcome::Promoted),
    ] {
        let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
        seed_tx(&state, FX2_TXID, "unproven", 0);
        set_failed(&state);
        assert_eq!(promote_proven_local_tx(&state, FX2_TXID, &proof, &raw), want, "{}", label);
        let heights: Vec<u32> = proven(&state, FX2_TXID).iter().map(|r| r.0).collect();
        if want == PromoteOutcome::Promoted {
            assert_eq!(heights, vec![FX2_HEIGHT], "stored at our chain's height, never 0");
        } else {
            assert!(heights.is_empty(), "{}: stored a proof our headers do not verify", label);
        }
    }
}

// ───────────── P7a-C9a / C18 / C10: building a BEEF (the shared proof_for_beef) ─────────────

async fn beef_for_fx2(state: &web::Data<AppState>) -> crate::beef::Beef {
    let mut beef = crate::beef::Beef::new();
    let _ = crate::beef_helpers::build_beef_for_txid(FX2_TXID, &mut beef, &state.database, &state.services, &*state.chain_tracker).await;
    beef
}

fn has_bump(beef: &crate::beef::Beef) -> bool {
    let i = beef.find_txid(FX2_TXID).expect("FX2 is in the BEEF");
    beef.tx_to_bump.get(i).copied().flatten().is_some()
}

fn block_header_rows(state: &AppState) -> i64 {
    state.database.lock().unwrap().connection().query_row("SELECT COUNT(*) FROM block_headers", [], |r| r.get(0)).unwrap()
}

#[actix_web::test]
async fn c9a_a_wrong_fetched_proof_is_neither_stored_nor_embedded() {
    // ⛔ The wrong proof carries its own height: without one, embedding fails for lack of a height,
    // and this row would pass with the check removed (seen in the negative control, mutation M6).
    let p = Arc::new(proof_provider(tsc(FX2_HEIGHT, &wrong())));
    let state = state_with(120, p.clone());
    let beef = beef_for_fx2(&state).await;
    assert_eq!(p.proof_calls.load(Ordering::SeqCst), 1, "the proof was really fetched");
    assert!(!has_bump(&beef), "a refuted proof was embedded");
    assert_eq!(proven(&state, FX2_TXID).len(), 0);
}

#[actix_web::test]
async fn c9a_a_real_fetched_proof_is_stored_and_embedded_with_no_header_lookup() {
    let p = Arc::new(proof_provider(woc_tsc(&fx2_block_hash(), &real())));
    let state = state_with(120, p.clone());
    let beef = beef_for_fx2(&state).await;
    assert!(has_bump(&beef));
    assert_eq!(proven(&state, FX2_TXID).iter().map(|r| r.0).collect::<Vec<_>>(), vec![FX2_HEIGHT]);
    assert_eq!(p.header_calls.load(Ordering::SeqCst), 0, "C18: the height came from our chain, not an indexer");
    assert_eq!(block_header_rows(&state), 0, "C18: nothing cached in block_headers");
}

#[actix_web::test]
async fn c9a_an_unheld_proof_is_embedded_unchecked_but_never_stored() {
    let mut p = proof_provider(woc_tsc(&"ab".repeat(32), &real()));
    p.header_for = Box::new(|_| Ok(crate::services::BlockHeader { block_hash: "ab".repeat(32), height: 900_000, header_hex: String::new() }));
    let p = Arc::new(p);
    let state = state_with(120, p.clone());
    let beef = beef_for_fx2(&state).await;
    assert!(has_bump(&beef), "K-o: a proof for a block we do not hold is still embedded");
    assert_eq!(proven(&state, FX2_TXID).len(), 0, "an unchecked proof was stored as verified");
    assert_eq!(p.header_calls.load(Ordering::SeqCst), 1, "the indexer height is asked only as an embedding hint");
    assert_eq!(block_header_rows(&state), 0, "C18: the hint is never cached");
}

/// `C10`: a stored proof our headers refute is never embedded, whoever wrote the row.
#[actix_web::test]
async fn c10_a_stored_wrong_proof_is_not_embedded_and_the_real_one_is() {
    let provider = || {
        let mut p = ScriptedProvider::new("TEST");
        p.proof_for = Box::new(|_| Err(crate::services::IndexerError::NotFound));
        p.raw_tx_for = Box::new(|t| if t == FX2_TXID { Ok(hex::decode(FX2_RAW).unwrap()) } else { Err(crate::services::IndexerError::NotFound) });
        p.status_for = Box::new(|t| Ok(status(t, TxState::Unknown, None)));
        Arc::new(p)
    };
    let state = state_with(120, provider());
    plant_proof(&state, FX2_HEIGHT, &tsc(FX2_HEIGHT, &wrong()));
    assert!(!has_bump(&beef_for_fx2(&state).await), "a stored proof our header refutes was embedded");

    let state = state_with(120, provider());
    plant_proof(&state, FX2_HEIGHT, &tsc(FX2_HEIGHT, &real()));
    assert!(has_bump(&beef_for_fx2(&state).await));
}

/// `text` with every `#[cfg(test)]` item removed (the item runs to its `;`, or to the brace that closes
/// its first `{`). ⛔ Not "everything after the first `#[cfg(test)]`": `handlers.rs` has test items
/// long before `sign_action`, and that cut made this guard blind to it (negative control, M18).
fn without_test_items(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("#[cfg(test)]") {
        out.push_str(&rest[..i]);
        let item = &rest[i..];
        let semi = item.find(';');
        let open = item.find('{');
        let end = match (semi, open) {
            (Some(s), Some(o)) if s < o => s + 1,
            (_, Some(o)) => {
                let mut depth = 0usize;
                let mut end = item.len();
                for (k, c) in item[o..].char_indices() {
                    match c {
                        '{' => depth += 1,
                        '}' => { depth -= 1; if depth == 0 { end = o + k + 1; break; } }
                        _ => {}
                    }
                }
                end
            }
            (Some(s), None) => s + 1,
            (None, None) => item.len(),
        };
        rest = &item[end..];
    }
    out.push_str(rest);
    out
}

/// `C9b` / `D2b`: no production code reads a stored proof except through the checked readers, and
/// none writes one except through the seam (the unchecked insert is `#[cfg(test)]`, so a production
/// writer would not compile; this test holds the read side and the seam's own entry points).
#[test]
fn c9b_stored_proofs_are_touched_only_through_the_seam() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() { walk(&p, out) } else if p.extension().map_or(false, |x| x == "rs") { out.push(p) }
        }
    }
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&src, &mut files);
    let mut offenders = Vec::new();
    for f in files {
        let rel = f.strip_prefix(&src).unwrap().to_string_lossy().replace('\\', "/");
        if rel.starts_with("database/") || rel == "proof_store.rs" || rel.contains("tests") {
            continue;
        }
        let prod = without_test_items(&std::fs::read_to_string(&f).unwrap());
        for needle in ["get_merkle_proof_as_tsc(", "insert_or_get(", "replace_with_verified(", "insert_verified(", "fill_empty_merkle_path("] {
            if prod.contains(needle) {
                offenders.push(format!("{}: {}", rel, needle));
            }
        }
    }
    assert!(offenders.is_empty(), "stored proofs touched outside the seam: {:?}", offenders);
}

// ───────────── P7a-C11: restore fills a stripped proof only when it is verified and agrees ─────────────

async fn refetch_with(proof: serde_json::Value, row_height: u32) -> Vec<u8> {
    let p = Arc::new(proof_provider(proof));
    let state = super::p2_test_support::app_state_from(crate::classify::p5_tests::wallet("p7a2-c11"),
        crate::services::WalletServices::with_one_test_provider(p));
    let headers = crate::header_chain::parse_headers(&ARCADE_970001_970300[..120 * 80]).unwrap();
    state.chain_tracker.ingest(&state.database, &headers, u32::MAX);
    state.database.lock().unwrap().connection().execute(
        "INSERT INTO proven_txs (txid, height, tx_index, merkle_path, raw_tx, created_at, updated_at)
         VALUES (?1, ?2, ?3, x'', x'', 0, 0)", rusqlite::params![FX2_TXID, row_height, FX2_INDEX as i64]).unwrap();
    let payload = crate::backup::collect_payload(state.database.lock().unwrap().connection(), "test_identity_key", "").unwrap();
    refetch_stripped_data(&state, &payload).await;
    let path: Vec<u8> = state.database.lock().unwrap().connection()
        .query_row("SELECT merkle_path FROM proven_txs WHERE txid = ?1", [FX2_TXID], |r| r.get(0)).unwrap();
    path
}

#[actix_web::test]
async fn c11_a_wrong_disagreeing_or_unheld_proof_is_not_written_into_a_restored_row() {
    assert!(refetch_with(woc_tsc(&fx2_block_hash(), &wrong()), FX2_HEIGHT).await.is_empty(), "wrong root written");
    assert!(refetch_with(woc_tsc(&fx2_block_hash(), &real()), FX2_HEIGHT + 1).await.is_empty(), "a proof for another height written");
    assert!(refetch_with(woc_tsc(&"ab".repeat(32), &real()), FX2_HEIGHT).await.is_empty(), "an unchecked proof written");
}

#[actix_web::test]
async fn c11_control_a_real_agreeing_proof_is_written() {
    assert!(!refetch_with(woc_tsc(&fx2_block_hash(), &real()), FX2_HEIGHT).await.is_empty());
}

// ───────────── P7a-C15: proofs embedded by the certificate and overlay paths ─────────────

#[actix_web::test]
async fn c15_embeddable_proof_refuses_a_wrong_root_and_takes_heights_from_our_chain() {
    let p = Arc::new(ScriptedProvider::new("TEST")); // header_for panics: no hint may be asked for a held block
    let state = state_with(120, p.clone());
    async fn embed(state: &AppState, proof: serde_json::Value) -> Option<serde_json::Value> {
        crate::beef_helpers::embeddable_proof(FX2_TXID, &proof, &state.services, &*state.chain_tracker).await
    }
    let arc_shaped = |nodes: &[String]| serde_json::json!({ "index": FX2_INDEX, "nodes": nodes, "height": FX2_HEIGHT, "target": "" });
    assert!(embed(&state, arc_shaped(&wrong())).await.is_none(), "an ARC-shaped proof our header refutes was embedded");
    assert_eq!(embed(&state, arc_shaped(&real())).await.unwrap()["height"], FX2_HEIGHT);
    assert_eq!(embed(&state, woc_tsc(&fx2_block_hash(), &real())).await.unwrap()["height"], FX2_HEIGHT);
    assert_eq!(embed(&state, serde_json::json!([woc_tsc(&fx2_block_hash(), &real())])).await.unwrap()["height"], FX2_HEIGHT, "WoC's array form");
    assert_eq!(p.header_calls.load(Ordering::SeqCst), 0);
}

// ───────────── P7a2-C22: Invalid while our chain is not synced is never a failure ─────────────

#[actix_web::test]
async fn c22_a_wrong_proof_while_unsynced_fails_nothing_and_is_asked_again() {
    let p = mined(FX2_HEIGHT, Some(wrong()));
    let state = app_state_with(crate::services::WalletServices::with_one_test_provider(p.clone()));
    let headers = crate::header_chain::parse_headers(&ARCADE_970001_970300[..120 * 80]).unwrap();
    state.chain_tracker.ingest(&state.database, &headers, u32::MAX); // not marked synced
    seed_tx(&state, FX2_TXID, "unproven", 0);
    run_proof_task(&state).await;
    run_proof_task(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "unproven");
    assert_eq!(spendable_outputs(&state, FX2_TXID), 1);
    assert_eq!(proven(&state, FX2_TXID).len(), 0);
    assert_eq!(p.status_calls.load(Ordering::SeqCst), 2, "asked again on the next pass");
}

// ───────────── P7a-C14-g: a completed request is not evidence ─────────────

#[actix_web::test]
async fn c14g_a_completed_request_with_no_verified_proof_does_not_complete_the_transaction() {
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    seed_tx(&state, FX2_TXID, "unproven", 0);
    state.database.lock().unwrap().connection().execute(
        "UPDATE proven_tx_reqs SET status = 'completed', notified = 0 WHERE txid = ?1", [FX2_TXID]).unwrap();
    crate::monitor::task_review_status::run(&state).await.unwrap();
    assert_eq!(status_of(&state, FX2_TXID), "unproven");

    // A stored proof our header refutes is not evidence either (a row's existence is not a check).
    let state_wrong = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    seed_tx(&state_wrong, FX2_TXID, "unproven", 0);
    plant_proof(&state_wrong, FX2_HEIGHT, &tsc(FX2_HEIGHT, &wrong()));
    state_wrong.database.lock().unwrap().connection().execute(
        "UPDATE proven_tx_reqs SET status = 'completed', notified = 0 WHERE txid = ?1", [FX2_TXID]).unwrap();
    crate::monitor::task_review_status::run(&state_wrong).await.unwrap();
    assert_eq!(status_of(&state_wrong, FX2_TXID), "unproven", "a refuted stored proof completed the transaction");

    plant_proof(&state, FX2_HEIGHT, &tsc(FX2_HEIGHT, &real()));
    state.database.lock().unwrap().connection().execute(
        "UPDATE proven_tx_reqs SET notified = 0 WHERE txid = ?1", [FX2_TXID]).unwrap();
    crate::monitor::task_review_status::run(&state).await.unwrap();
    assert_eq!(status_of(&state, FX2_TXID), "completed", "control: with a verified stored proof it completes");
}

// ───────────── Decision 6: unproven money is shown at once (D6a / D6b) ─────────────

#[actix_web::test]
async fn d6a_unproven_coins_count_in_the_balance() {
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    seed_tx(&state, FX2_TXID, "sending", 0);
    // The production writer puts it in `unproven`: MINED with a proof for a block we do not hold.
    assert!(accept_mined(&state, UNHELD, &real()).await.is_ok());
    assert_eq!(status_of(&state, FX2_TXID), "unproven");
    let balance = crate::database::OutputRepository::new(state.database.lock().unwrap().connection())
        .calculate_balance(1).unwrap();
    assert_eq!(balance, 599, "the unproven transaction's change is not in the balance");
}

#[actix_web::test]
async fn d6b_unproven_transactions_are_listed_at_once_as_unproven() {
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    seed_tx(&state, FX2_TXID, "sending", 0);
    assert!(accept_mined(&state, UNHELD, &real()).await.is_ok());
    let q = web::Query::<std::collections::HashMap<String, String>>::from_query("").unwrap();
    let (code, body) = super::p2_test_support::body_json(wallet_activity(state.clone(), q).await).await;
    assert_eq!(code, 200);
    let row = body["items"].as_array().unwrap().iter().find(|i| i["txid"] == FX2_TXID).expect("listed");
    assert_eq!(row["status"], "unproven");
}

// ───────────── P7a-C1b: internalizeAction stores the payment's own verified proof ─────────────

/// A payment to this wallet, mined at index 0 of a two-transaction regtest block 101 (FX-R, with a
/// sibling in place of the coinbase), whose header carries `chain_root` — the real root when `None`,
/// anything else for a header that refutes the proof.
struct Payment {
    chain_root: Option<[u8; 32]>,
    /// The sibling leaf's flag: 0x00 a plain hash, 0x02 another transaction in the same BUMP.
    sibling_flag: u8,
    /// A `transactions` row for the payment already there, with this status.
    existing_status: Option<&'static str>,
    /// The height the BUMP claims (101 is held; above 102 is not).
    bump_height: u32,
    /// Plant the payment's input as one of our outputs, spendable again (what `mark_failed` leaves).
    restored_input: bool,
}

impl Default for Payment {
    fn default() -> Self {
        Payment { chain_root: None, sibling_flag: 0x00, existing_status: None, bump_height: 101, restored_input: false }
    }
}

async fn internalize_mined_payment(pay: Payment) -> (web::Data<AppState>, (u16, serde_json::Value), String) {
    use crate::monitor::task_check_peerpay::tests::{chain_with_block, txid_natural};
    use super::p7a_route_tests::{funding, our_lock, tx, txid_hex};
    use sha2::{Digest, Sha256};
    let db = crate::classify::p5_tests::wallet("p7a2-c1b");
    let lock = our_lock(&db);
    let parent = funding(5_000);
    let subject = tx(&[(&parent, 0, 0xFFFF_FFFF)], &[(4_000, lock)], 0);
    let mut p = ScriptedProvider::new("fake");
    p.status_for = Box::new(|t| Ok(status(t, TxState::Mined, None)));
    let p = Arc::new(p);
    let dynp: Arc<dyn crate::services::IndexerProvider> = p.clone();
    let sibling = [0x5c; 32];
    let mut cat = txid_natural(&subject).to_vec();
    cat.extend_from_slice(&sibling);
    let real_root: [u8; 32] = Sha256::digest(Sha256::digest(&cat)).into();
    let mut leaf = vec![0x00, 0x02];
    leaf.extend_from_slice(&txid_natural(&subject));
    let mut sib = vec![0x01, pay.sibling_flag];
    sib.extend_from_slice(&sibling);
    let bump = crate::beef::MerkleProof { block_height: pay.bump_height, tree_height: 1, levels: vec![vec![leaf, sib]] };
    // The parent rides along so the BRC-67 checks can run when the BUMP is for a block we do not hold.
    let beef = crate::beef::Beef { version: crate::beef::BEEF_V1_MARKER, bumps: vec![bump],
                                   transactions: vec![parent.clone(), subject.clone()], tx_to_bump: vec![None, Some(0)] };
    let atomic = hex::decode(beef.to_atomic_beef_hex(&txid_hex(&subject)).unwrap()).unwrap();
    let tracker = chain_with_block(pay.chain_root.unwrap_or(real_root), 1);
    let state = super::p2_test_support::app_state_from_with_tracker(db,
        crate::services::WalletServices::with_test_chains(vec![dynp.clone()], vec![dynp]), tracker);
    if let Some(st) = pay.existing_status {
        state.database.lock().unwrap().connection().execute(
            "INSERT INTO transactions (txid, reference_number, raw_tx, status, is_outgoing, satoshis, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, 4000, 0, 0)",
            rusqlite::params![txid_hex(&subject), uuid::Uuid::new_v4().to_string(), hex::encode(&subject), st]).unwrap();
    }
    if pay.restored_input {
        state.database.lock().unwrap().connection().execute(
            "INSERT INTO outputs (user_id, spendable, vout, satoshis, txid, created_at, updated_at)
             VALUES (1, 1, 0, 5000, ?1, 0, 0)", [txid_hex(&parent)]).unwrap();
    }
    let req: InternalizeActionRequest = serde_json::from_value(serde_json::json!({
        "tx": atomic, "description": "B5-T1-P7a-2 C1b",
        "outputs": [{ "outputIndex": 0, "protocol": "basket insertion", "insertionRemittance": { "basket": "todo" } }],
    })).unwrap();
    let got = super::p2_test_support::body_json(internalize_action(state.clone(), web::Json(req)).await).await;
    assert!(p.broadcast_hashes().is_empty(), "nothing was broadcast: the proof came from the BEEF");
    (state, got, txid_hex(&subject))
}

/// The payment's parent txid in `internalize_mined_payment` (its input's outpoint is vout 0).
fn payment_parent_txid() -> String {
    super::p7a_route_tests::txid_hex(&super::p7a_route_tests::funding(5_000))
}

#[actix_web::test]
async fn c1b_a_verified_subject_bump_is_stored_once_and_completes_the_payment() {
    let (state, (code, body), txid) = internalize_mined_payment(Payment::default()).await;
    assert_eq!(code, 200, "{}", body);
    assert_eq!(proven(&state, &txid).iter().map(|r| r.0).collect::<Vec<_>>(), vec![101]);
    let linked: Option<i64> = state.database.lock().unwrap().connection()
        .query_row("SELECT proven_tx_id FROM transactions WHERE txid = ?1", [&txid], |r| r.get(0)).unwrap();
    assert!(linked.is_some(), "the transaction is linked to its proof");
    assert_eq!(status_of(&state, &txid), "completed");
}

#[actix_web::test]
async fn c1b_control_a_refuted_subject_bump_stores_nothing() {
    let (state, (code, body), txid) = internalize_mined_payment(Payment { chain_root: Some([0x77; 32]), ..Default::default() }).await;
    assert_eq!((code, body["code"].as_str()), (400, Some("ERR_BAD_PROOF")), "{}", body);
    assert!(proven(&state, &txid).is_empty());
}

// ───────────── Adversarial review (2026-10-13): one test per real finding ─────────────

/// #12 / #19: a BUMP holding two transactions of the block is read along OUR leaf, not the last one.
#[actix_web::test]
async fn c1b_a_bump_shared_with_another_transaction_still_stores_our_proof() {
    let (state, (code, body), txid) = internalize_mined_payment(Payment { sibling_flag: 0x02, ..Default::default() }).await;
    assert_eq!(code, 200, "{}", body);
    assert_eq!(proven(&state, &txid).iter().map(|r| r.0).collect::<Vec<_>>(), vec![101]);
    assert_eq!(status_of(&state, &txid), "completed");
}

/// #6 (+ fix review #8): an existing `failed` row is recovered with its bookkeeping, not completed by a
/// bare status write — its disabled outputs come back.
#[actix_web::test]
async fn c1b_a_failed_row_is_recovered_not_completed_by_a_bare_status_write() {
    let (state, (code, body), txid) = internalize_mined_payment(
        Payment { existing_status: Some("failed"), restored_input: true, ..Default::default() }).await;
    assert_eq!(code, 200, "{}", body);
    assert_eq!(proven(&state, &txid).len(), 1);
    assert_eq!(status_of(&state, &txid), "completed");
    // xhigh 168 #9: the recovery's own bookkeeping — the input the chain spent is spent again.
    // A bare status write would leave it spendable.
    assert_eq!(spendable_outputs(&state, &payment_parent_txid()), 0, "the spent input is selectable again");
    let recovered_note: i64 = state.database.lock().unwrap().connection().query_row(
        "SELECT COUNT(*) FROM transactions WHERE txid = ?1 AND proven_tx_id IS NOT NULL AND confirmations = 1", [&txid], |r| r.get(0)).unwrap();
    assert_eq!(recovered_note, 1);
}

/// `A15` / #18: a subject BUMP for a block we do not hold yet stores nothing; the payment is credited
/// unproven and the proof task asks again later.
#[actix_web::test]
async fn a15_an_unheld_subject_bump_stores_nothing_and_leaves_the_payment_unproven() {
    let (state, (code, body), txid) = internalize_mined_payment(Payment { bump_height: 105, ..Default::default() }).await;
    assert_eq!(code, 200, "{}", body);
    assert!(proven(&state, &txid).is_empty(), "stored a proof for a block we do not hold");
    assert_ne!(status_of(&state, &txid), "completed");
}

/// #3 / #10 / #17: a stored wrong proof that a proof request still links to is replaced by the
/// verified one (foreign keys are on: the delete used to fail and leave the transaction unlinked).
#[actix_web::test]
async fn store_replaces_a_wrong_proof_a_request_links_to() {
    let state = state_with(120, mined(FX2_HEIGHT, Some(real())));
    seed_tx(&state, FX2_TXID, "unproven", 0);
    plant_proof(&state, FX2_HEIGHT, &tsc(FX2_HEIGHT, &wrong()));
    state.database.lock().unwrap().connection().execute(
        "UPDATE proven_tx_reqs SET proven_tx_id = (SELECT provenTxId FROM proven_txs WHERE txid = ?1) WHERE txid = ?1", [FX2_TXID]).unwrap();
    run_proof_task(&state).await;
    let rows = proven(&state, FX2_TXID);
    assert_eq!(rows.len(), 1);
    let stored: serde_json::Value = serde_json::from_slice(&rows[0].1).unwrap();
    assert_eq!(stored["nodes"], tsc(FX2_HEIGHT, &real())["nodes"], "the wrong proof is still stored");
    assert_eq!(status_of(&state, FX2_TXID), "completed");
}

/// #7 / #11 / #14: a transaction reported mined earlier is not failed by an undecided oracle vote past
/// its timeout. (The other arm — every oracle's authoritative "not found" fails it — needs the three
/// live indexers and is not reachable offline: the dead-proxy client makes every vote undecided.)
#[actix_web::test]
async fn c21_reported_mined_earlier_survives_an_undecided_vote_past_its_timeout() {
    let mut p = ScriptedProvider::new("TEST");
    p.status_for = Box::new(|t| Ok(status(t, TxState::Unknown, None)));
    let state = state_with(120, Arc::new(p));
    seed_tx(&state, FX2_TXID, "unproven", 7 * 3600);
    state.database.lock().unwrap().connection().execute(
        "UPDATE transactions SET confirmations = 1 WHERE txid = ?1", [FX2_TXID]).unwrap();
    run_proof_task(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "unproven", "a transaction reported mined was failed on an undecided vote");
    assert_eq!(spendable_outputs(&state, FX2_TXID), 1);
}

/// #9: an indexer's height hint is still checked wherever our chain holds that height.
#[actix_web::test]
async fn c15_a_hinted_height_we_hold_refutes_a_wrong_proof() {
    let mut p = ScriptedProvider::new("TEST");
    p.header_for = Box::new(|_| Ok(crate::services::BlockHeader { block_hash: "ab".repeat(32), height: FX2_HEIGHT, header_hex: String::new() }));
    let state = state_with(120, Arc::new(p));
    let embed = |nodes: Vec<String>| {
        let state = state.clone();
        async move { crate::beef_helpers::embeddable_proof(FX2_TXID, &woc_tsc(&"ab".repeat(32), &nodes), &state.services, &*state.chain_tracker).await }
    };
    assert!(embed(wrong()).await.is_none(), "a proof refuted at its hinted height was embedded");
    assert_eq!(embed(real()).await.unwrap()["height"], FX2_HEIGHT, "control: the right proof at that height is embedded");
}

/// #5 / #15: the BEEF builder also tries for a proof of our own `unproven` transaction an indexer
/// reported mined; a fresh unmined one is not asked about (the payment hot path).
#[actix_web::test]
async fn c9a_our_own_unproven_transaction_still_gets_its_proof() {
    let state = state_with(120, Arc::new(proof_provider(woc_tsc(&fx2_block_hash(), &real()))));
    seed_tx(&state, FX2_TXID, "unproven", 0);
    state.database.lock().unwrap().connection().execute(
        "UPDATE transactions SET confirmations = 1 WHERE txid = ?1", [FX2_TXID]).unwrap();
    assert!(has_bump(&beef_for_fx2(&state).await), "an unproven ancestor reported mined got no proof");
}

#[actix_web::test]
async fn c9a_a_fresh_unmined_ancestor_is_not_asked_for_a_proof() {
    let p = Arc::new(proof_provider(woc_tsc(&fx2_block_hash(), &real())));
    let state = state_with(120, p.clone());
    seed_tx(&state, FX2_TXID, "unproven", 0);
    let _ = beef_for_fx2(&state).await;
    assert_eq!(p.proof_calls.load(Ordering::SeqCst), 0);
}

/// #8: an immediate MINED with no height still records the indexer's report (C21's marker).
#[actix_web::test]
async fn c3_mined_with_no_height_still_records_the_report() {
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    seed_tx(&state, FX2_TXID, "sending", 0);
    let br = BroadcastResult { provider: "TEST", txid: FX2_TXID.to_string(), tx_status: "MINED".to_string(),
                               merkle_path_bump: None, block_height: None };
    assert!(accept_broadcast_result(br, Some(ProofSink::of(&state)), Some(FX2_TXID), |_| async {
        panic!("a bound acknowledgement must not be verified on the network")
    }).await.is_ok());
    let confirmations: i64 = state.database.lock().unwrap().connection()
        .query_row("SELECT confirmations FROM transactions WHERE txid = ?1", [FX2_TXID], |r| r.get(0)).unwrap();
    assert_eq!(confirmations, 1);
    assert_eq!(status_of(&state, FX2_TXID), "unproven");
}

/// #13: an index with bits beyond the proof's depth is not a position; it is not stored as one.
#[actix_web::test]
async fn c12_an_index_beyond_the_proof_depth_is_malformed() {
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    let mut t = tsc(FX2_HEIGHT, &real());
    t["index"] = serde_json::json!(FX2_INDEX + (1u64 << FX2_NODES.len()));
    assert!(matches!(crate::proof_store::check_tsc(&*state.chain_tracker, FX2_TXID, &t), crate::proof_store::ProofCheck::Malformed(_)));
}

/// #2 / #16: rows that stay unproven (reported mined, proof not verifiable yet) must not crowd a
/// genuinely pending older transaction out of the proof task's batch of 20.
#[actix_web::test]
async fn c21_pending_transactions_are_not_starved_by_unproven_mined_ones() {
    let mut p = ScriptedProvider::new("TEST");
    p.status_for = Box::new(|t| Ok(status(t, TxState::Unknown, None)));
    let state = state_with(120, Arc::new(p));
    seed_tx(&state, FX2_TXID, "sending", 7 * 3600); // older, never reported mined, past its timeout
    for i in 0..20u8 {
        let t = hex::encode([i + 1; 32]);
        seed_tx(&state, &t, "unproven", 60);
        state.database.lock().unwrap().connection().execute(
            "UPDATE transactions SET confirmations = 1 WHERE txid = ?1", [&t]).unwrap();
    }
    run_proof_task(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "failed", "the older pending transaction was never reached");
}

// ───────────── Review of the fix commit (2026-10-13) ─────────────

/// Fix review #5: a MINED report for a block our chain does not hold does not fetch the proof every tick.
#[actix_web::test]
async fn c14a_mined_in_a_block_we_do_not_hold_is_not_refetched_every_tick() {
    let mut p = ScriptedProvider::new("TEST");
    p.status_for = Box::new(|t| {
        let mut s = status(t, TxState::Mined, Some("MINED"));
        s.block_height = Some(UNHELD);
        Ok(s)
    });
    p.proof_for = Box::new(|t| panic!("a proof was fetched for {} in a block we do not hold", t));
    let p = Arc::new(p);
    let state = state_with(120, p.clone());
    seed_tx(&state, FX2_TXID, "unproven", 0);
    run_proof_task(&state).await;
    assert_eq!(p.proof_calls.load(Ordering::SeqCst), 0);
    assert_eq!(status_of(&state, FX2_TXID), "unproven");
}

/// Fix review #2: past its timeout, a transaction reported mined earlier is failed only by every
/// oracle's authoritative not-found — not by another arm of the same tick (here: a REJECTED answer).
#[actix_web::test]
async fn c21_reported_mined_earlier_is_not_failed_by_another_arm_of_the_same_tick() {
    let mut p = ScriptedProvider::new("TEST");
    p.status_for = Box::new(|t| Ok(status(t, TxState::Rejected, Some("REJECTED"))));
    let state = state_with(120, Arc::new(p));
    seed_tx(&state, FX2_TXID, "unproven", 7 * 3600);
    state.database.lock().unwrap().connection().execute(
        "UPDATE transactions SET confirmations = 1 WHERE txid = ?1", [FX2_TXID]).unwrap();
    run_proof_task(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "unproven");
}

// ───────────── `/code-review xhigh 167` (2026-10-13) ─────────────

/// #4: a stored proof saved with height 0 (older rows: a WhatsOnChain proof names its block by hash)
/// is embedded at the height our chain resolves, never at block 0.
#[actix_web::test]
async fn c10_a_stored_height_zero_proof_is_embedded_at_our_chains_height() {
    let mut p = ScriptedProvider::new("TEST");
    p.proof_for = Box::new(|t| panic!("the stored proof should have served {}", t));
    p.raw_tx_for = Box::new(|t| if t == FX2_TXID { Ok(hex::decode(FX2_RAW).unwrap()) } else { Err(crate::services::IndexerError::NotFound) });
    p.status_for = Box::new(|t| Ok(status(t, TxState::Unknown, None)));
    let state = state_with(120, Arc::new(p));
    plant_proof(&state, 0, &woc_tsc(&fx2_block_hash(), &real()));
    let beef = beef_for_fx2(&state).await;
    let i = beef.find_txid(FX2_TXID).unwrap();
    let b = beef.tx_to_bump[i].expect("embedded");
    assert_eq!(beef.bumps[b].block_height, FX2_HEIGHT, "embedded at the stored height 0");
}

/// #8: TaskUnFail recovers a falsely failed transaction an indexer reports mined with no BUMP — the
/// proof comes from the proof chain.
#[actix_web::test]
async fn c6_mined_with_no_bump_is_recovered_from_the_proof_chain() {
    let state = state_with(120, mined_no_bump(Ok(woc_tsc(&fx2_block_hash(), &real()))));
    seed_tx(&state, FX2_TXID, "unproven", 0);
    set_failed(&state);
    run_unfail(&state).await;
    assert_eq!(status_of(&state, FX2_TXID), "completed");
    assert_eq!(proven(&state, FX2_TXID).iter().map(|r| r.0).collect::<Vec<_>>(), vec![FX2_HEIGHT]);
}

// ───────────── `/code-review xhigh 168` (2026-10-13) ─────────────

/// #1: a failed row with a verified proof is left to TaskUnFail, never completed by a bare write.
#[actix_web::test]
async fn c14g_a_failed_row_is_not_completed_by_task_review_status() {
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    seed_tx(&state, FX2_TXID, "unproven", 0);
    set_failed(&state);
    plant_proof(&state, FX2_HEIGHT, &tsc(FX2_HEIGHT, &real()));
    state.database.lock().unwrap().connection().execute(
        "UPDATE proven_tx_reqs SET status = 'completed', notified = 0 WHERE txid = ?1", [FX2_TXID]).unwrap();
    crate::monitor::task_review_status::run(&state).await.unwrap();
    assert_eq!(status_of(&state, FX2_TXID), "failed");
}

/// #5: a proof that cannot produce a root at its hinted height is not embedded.
#[actix_web::test]
async fn c15_an_unreadable_proof_at_its_hinted_height_is_not_embedded() {
    let mut p = ScriptedProvider::new("TEST");
    p.header_for = Box::new(|_| Ok(crate::services::BlockHeader { block_hash: "ab".repeat(32), height: FX2_HEIGHT, header_hex: String::new() }));
    let state = state_with(120, Arc::new(p));
    let mut nodes = real();
    nodes[4] = "zz".to_string();
    let got = crate::beef_helpers::embeddable_proof(FX2_TXID, &woc_tsc(&"ab".repeat(32), &nodes), &state.services, &*state.chain_tracker).await;
    assert!(got.is_none(), "an unreadable proof was embedded");
}

/// #6: a restored row stored with height 0 is refilled with the verified proof, at its real height.
#[actix_web::test]
async fn c11_a_restored_height_zero_row_is_refilled_at_the_verified_height() {
    assert!(!refetch_with(woc_tsc(&fx2_block_hash(), &real()), 0).await.is_empty());
}

/// #7: a proof as deep as 64 levels is not a path.
#[actix_web::test]
async fn c12_a_64_level_proof_is_malformed() {
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    let nodes: Vec<String> = (0..64).map(|_| "11".repeat(32)).collect();
    assert!(matches!(crate::proof_store::check_tsc(&*state.chain_tracker, FX2_TXID, &tsc(FX2_HEIGHT, &nodes)),
                     crate::proof_store::ProofCheck::Malformed(_)));
}

/// #8: a MINED answer with no height keeps a height already recorded.
#[actix_web::test]
async fn c3_mined_with_no_height_keeps_a_recorded_height() {
    let state = state_with(120, Arc::new(ScriptedProvider::new("TEST")));
    seed_tx(&state, FX2_TXID, "unproven", 0);
    state.database.lock().unwrap().connection().execute(
        "UPDATE transactions SET confirmations = 1, block_height = ?2 WHERE txid = ?1", rusqlite::params![FX2_TXID, FX2_HEIGHT]).unwrap();
    let br = BroadcastResult { provider: "TEST", txid: FX2_TXID.to_string(), tx_status: "MINED".to_string(),
                               merkle_path_bump: None, block_height: None };
    assert!(accept_broadcast_result(br, Some(ProofSink::of(&state)), Some(FX2_TXID), |_| async {
        panic!("a bound acknowledgement must not be verified on the network")
    }).await.is_ok());
    let h: Option<i64> = state.database.lock().unwrap().connection()
        .query_row("SELECT block_height FROM transactions WHERE txid = ?1", [FX2_TXID], |r| r.get(0)).unwrap();
    assert_eq!(h, Some(FX2_HEIGHT as i64));
}

/// #10: no proof fetch on the payment path for an ancestor mined in a block our chain does not hold.
#[actix_web::test]
async fn c9a_an_ancestor_in_a_block_we_do_not_hold_is_not_asked_for_a_proof() {
    let p = Arc::new(proof_provider(woc_tsc(&fx2_block_hash(), &real())));
    let state = state_with(120, p.clone());
    seed_tx(&state, FX2_TXID, "unproven", 0);
    state.database.lock().unwrap().connection().execute(
        "UPDATE transactions SET confirmations = 1, block_height = ?2 WHERE txid = ?1", rusqlite::params![FX2_TXID, UNHELD]).unwrap();
    let _ = beef_for_fx2(&state).await;
    assert_eq!(p.proof_calls.load(Ordering::SeqCst), 0);
}
