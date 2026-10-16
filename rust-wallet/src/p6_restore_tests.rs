//! B5-T1-P6 T1 rows on the restore side: `P6-A1` (the March phantom), `P6-A3` (stripped
//! change address), `P6-A10` (counter restored, both restore paths), `P6-A11` lives in
//! `p2_create_action_tests` (it needs that fixture), `P6-A14` (reconcile retried; scan
//! skipped on persistent failure), `P6-A16` (the rig's containment).
//!
//! Subject: `finish_onchain_restore` — the function `wallet_recover_onchain` calls with
//! the fetched backup — and `wallet_import_with` (what `wallet_import` calls), over real
//! temp wallets with the chain scripted. The REDs are the second agent's designs
//! (`…/phase-P6-…/evidence/REDS_2026-10-07.md`), run as scratch mutations.
use super::*;
use super::p2_test_support::{app_state_from, body_json};
use crate::classify::p5_tests::{change_of, our_p2pkh, p2pkh_for_key, raw_row, selected, wallet};
use crate::scan::tests::{keys, receive_address, tx, txid_of, FakeChain};

const RECV: Option<&str> = Some("2-receive address");

fn varint(n: usize) -> Vec<u8> {
    match n {
        0..=0xfc => vec![n as u8],
        _ => [vec![0xfd], (n as u16).to_le_bytes().to_vec()].concat(),
    }
}

/// A transaction spending `prev_txid`:`prev_vout` (display hex), paying `outputs`.
fn spend(prev_txid: &str, prev_vout: u32, outputs: &[(i64, Vec<u8>)]) -> Vec<u8> {
    let mut prev: Vec<u8> = hex::decode(prev_txid).unwrap();
    prev.reverse();
    let mut t = 1u32.to_le_bytes().to_vec();
    t.extend(varint(1));
    t.extend(prev);
    t.extend(prev_vout.to_le_bytes());
    t.extend(varint(0));
    t.extend(0xffff_ffffu32.to_le_bytes());
    t.extend(varint(outputs.len()));
    for (v, s) in outputs {
        t.extend((*v as u64).to_le_bytes());
        t.extend(varint(s.len()));
        t.extend(s);
    }
    t.extend(0u32.to_le_bytes());
    t
}

/// The backup transaction shape: vout 0 = data, vout 1 = marker (not ours), vout 2 = change.
fn backup_spending(prev_txid: &str, change: (i64, Vec<u8>)) -> Vec<u8> {
    spend(prev_txid, 0, &[(1, vec![0x00, 0x6a, 0x01, 0x42]), (1, p2pkh_for_key(&[0x42u8; 32])), change])
}

/// A source wallet with funding coin X at receive `x_index`, and its payload.
struct Source {
    db: crate::database::WalletDatabase,
    phrase: String,
    x_txid: String,
}

fn source(tag: &str, x_index: i32, x_sats: i64) -> Source {
    let db = wallet(tag);
    let phrase = db.get_cached_mnemonic().unwrap().to_string();
    let x_raw = tx(0x11, &[(x_sats, our_p2pkh(&db, x_index))]);
    let x_txid = txid_of(&x_raw);
    raw_row(&db, &x_txid, 0, x_sats, Some(&our_p2pkh(&db, x_index)), true, RECV, Some(&x_index.to_string()), true);
    Source { db, phrase, x_txid }
}

fn payload_of(src: &Source) -> crate::backup::BackupPayload {
    let ik: String = src.db.connection().query_row("SELECT identity_key FROM users ORDER BY userId LIMIT 1", [], |r| r.get(0)).unwrap();
    crate::backup::collect_payload(src.db.connection(), &ik, &src.phrase).unwrap()
}

/// The restore target: a fresh wallet created from the same phrase (step 4 of the handler).
fn target(tag: &str, phrase: &str) -> (web::Data<AppState>, i64, i64) {
    let path = std::env::temp_dir().join(format!("hodos_p6_{}_{}.db", tag, uuid::Uuid::new_v4()));
    let mut db = crate::database::WalletDatabase::new(path).unwrap();
    let (wid, uid, _, _) = db.create_wallet_from_existing_mnemonic(phrase, None).unwrap();
    let state = app_state_from(db, crate::services::WalletServices::with_test_chains(vec![], vec![]));
    state.shutdown.cancel();
    (state, wid, uid)
}

async fn finish(state: &web::Data<AppState>, wid: i64, uid: i64, payload: &crate::backup::BackupPayload,
                backup_txid: &str, raw: Vec<u8>, chain: &FakeChain) -> RestoreFinish {
    finish_onchain_restore(state, wid, uid, payload, backup_txid, raw, chain, |_| None)
        .await.expect("import")
}

/// The background scan the restore handler starts after `finish_onchain_restore` — the same
/// function the spawned task runs, awaited here.
async fn restore_scan(state: &web::Data<AppState>, wid: i64, uid: i64, chain: &FakeChain) -> crate::scan::ScanOutcome {
    let (k, p) = { let d = state.database.lock().unwrap(); keys(&d) };
    run_background_scan(state, uid, wid, &k, &p, "I3:restore-scan", chain, false).await.expect("scan")
}

/// Wait for a scan started in the background by a handler to finish.
async fn wait_for_scan(state: &web::Data<AppState>) {
    for _ in 0..1000 {
        if !state.sync_status.read().unwrap().active {
            return;
        }
        actix_web::rt::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("the background scan did not finish");
}

/// `(outputId, spendable, spent_by)` by outpoint.
fn coin(state: &web::Data<AppState>, txid: &str, vout: u32) -> Option<(i64, bool, Option<i64>)> {
    use rusqlite::OptionalExtension;
    state.database.lock().unwrap().connection().query_row(
        "SELECT outputId, spendable, spent_by FROM outputs WHERE lower(txid) = lower(?1) AND vout = ?2",
        rusqlite::params![txid, vout], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0, r.get(2)?))).optional().unwrap()
}

fn tx_id(state: &web::Data<AppState>, txid: &str) -> i64 {
    state.database.lock().unwrap().connection().query_row(
        "SELECT id FROM transactions WHERE txid = ?1", [txid], |r| r.get(0)).unwrap()
}

fn balance(state: &web::Data<AppState>) -> i64 {
    crate::database::OutputRepository::new(state.database.lock().unwrap().connection()).calculate_balance(1).unwrap()
}

// ---------------------------------------------------------------------------------------
// P6-A1 — the March phantom: after restore + scan + one TaskReviewStatus pass, the coin
// the backup spent (X) is spent, its change (Y) is the one spendable coin.
// ---------------------------------------------------------------------------------------

#[actix_web::test]
async fn p6_a1_restore_never_shows_the_backups_spent_coin_beside_its_change() {
    let src = source("a1src", 2, 50_000);
    let payload = payload_of(&src);
    let y_script = our_p2pkh(&src.db, 4);
    let t_raw = backup_spending(&src.x_txid, (40_000, y_script));
    let t = txid_of(&t_raw);
    let (state, wid, uid) = target("a1", &src.phrase);

    // The chain lists Y at receive address 4 (the scan walks it) and knows T.
    let mut chain = FakeChain::new();
    let y_addr = { let d = state.database.lock().unwrap(); receive_address(&d, 4) };
    chain.raws.insert(t.clone(), t_raw.clone());
    chain.listed.insert(y_addr, vec![(t.clone(), 2, 40_000)]);

    let f = finish(&state, wid, uid, &payload, &t, t_raw, &chain).await;
    assert_eq!(f.reconcile, Ok((1, true)), "reconcile result");
    assert!(f.scan_allowed, "a reconciled backup allows the scan");
    restore_scan(&state, wid, uid, &chain).await;

    let (y_id, y_spendable, _) = coin(&state, &t, 2).expect("Y (the backup's change) is present");
    assert!(y_spendable && change_of(&state.database.lock().unwrap(), y_id), "Y is money");
    let (x_id, x_spendable, x_spent_by) = coin(&state, &src.x_txid, 0).unwrap();
    assert!(!x_spendable, "X — spent by the backup — is spendable (the March phantom)");
    assert_eq!(x_spent_by, Some(tx_id(&state, &t)), "X's spent_by is the backup transaction");
    assert!(!selected(&state.database.lock().unwrap(), x_id), "X is selectable money");
    assert_eq!(balance(&state), 40_000, "balance = Y only");

    // The second phantom route: one TaskReviewStatus pass. Join precondition (declared
    // fixture): X's creating transaction exists and is 'completed'.
    {
        let d = state.database.lock().unwrap();
        let c = d.connection();
        c.execute("INSERT OR IGNORE INTO transactions (user_id, txid, status, reference_number, description, is_outgoing, satoshis, created_at, updated_at)
                   VALUES (1, ?1, 'completed', 'a1-x-parent', 'a1', 0, 0, 0, 0)", [&src.x_txid]).unwrap();
        c.execute("UPDATE outputs SET transaction_id = (SELECT id FROM transactions WHERE txid = ?1) WHERE outputId = ?2",
                  rusqlite::params![src.x_txid, x_id]).unwrap();
    }
    crate::monitor::task_review_status::run(&state).await.unwrap();
    assert!(!coin(&state, &src.x_txid, 0).unwrap().1, "TaskReviewStatus revived X");
    assert_eq!(balance(&state), 40_000);
}

// ---------------------------------------------------------------------------------------
// P6-A3 — the backup stripped Y's address row; Y's index is more than 20 above the
// remaining rows and inside the counter + 20. Y is still recovered — by reconcile.
// ---------------------------------------------------------------------------------------

#[actix_web::test]
async fn p6_a3_backup_change_at_a_stripped_address_is_not_lost() {
    let src = source("a3src", 2, 50_000);
    src.db.connection().execute("UPDATE wallets SET current_index = 40", []).unwrap();
    let mut payload = payload_of(&src);
    payload.addresses.retain(|a| a.index <= 2);
    let max_row = payload.addresses.iter().map(|a| a.index).max().unwrap();
    assert!(35 > max_row + 20 && payload.wallet.current_index == 40, "pre-condition: Y (35) sits beyond MAX(rows) + 20, inside counter + 20");
    let t_raw = backup_spending(&src.x_txid, (40_000, our_p2pkh(&src.db, 35)));
    let t = txid_of(&t_raw);
    let (state, wid, uid) = target("a3", &src.phrase);
    let mut chain = FakeChain::new(); // Y NOT listed: only reconcile can insert it
    chain.raws.insert(t.clone(), t_raw.clone());

    let f = finish(&state, wid, uid, &payload, &t, t_raw, &chain).await;
    assert_eq!(f.reconcile, Ok((1, true)), "reconcile inserted the change");
    let (y_id, y_spendable, _) = coin(&state, &t, 2).expect("Y (change at a stripped address) was lost");
    assert!(y_spendable && change_of(&state.database.lock().unwrap(), y_id));
}

/// The other arm of the bound: Y sits above the counter + 20 but inside the address
/// rows + 20 — a bound on `current_index` alone would miss it.
#[actix_web::test]
async fn p6_a3_backup_change_above_the_counter_inside_the_rows_is_not_lost() {
    let src = source("a3bsrc", 2, 50_000);
    {
        let (k, p) = keys(&src.db);
        let repo = crate::database::AddressRepository::new(src.db.connection());
        for i in 1..=30 {
            repo.create(&crate::database::Address { id: None, wallet_id: 1, index: i,
                address: crate::reconcile::derive_receive_address(&k, &p, i).unwrap(), public_key: String::new(),
                used: true, balance: 0, pending_utxo_check: false, created_at: 0 }).unwrap();
        }
        src.db.connection().execute("UPDATE wallets SET current_index = 5", []).unwrap();
    }
    let payload = payload_of(&src);
    assert!(45 > payload.wallet.current_index + 20 && payload.addresses.iter().any(|a| a.index == 30),
            "pre-condition: Y (45) above counter + 20, inside MAX(rows) + 20");
    let t_raw = backup_spending(&src.x_txid, (40_000, our_p2pkh(&src.db, 45)));
    let t = txid_of(&t_raw);
    let (state, wid, uid) = target("a3b", &src.phrase);
    let mut chain = FakeChain::new();
    chain.raws.insert(t.clone(), t_raw.clone());
    let f = finish(&state, wid, uid, &payload, &t, t_raw, &chain).await;
    assert_eq!(f.reconcile, Ok((1, true)), "reconcile inserted the change");
    assert!(coin(&state, &t, 2).is_some(), "Y (above the counter, inside the rows) was lost");
}

// ---------------------------------------------------------------------------------------
// The handlers themselves: `wallet_recover` (no-backup recovery, P6-A17) and
// `wallet_rescan` (the "Find late payments" button, P6-A9 note) through their `_with`.
// ---------------------------------------------------------------------------------------

fn empty_state(tag: &str) -> web::Data<AppState> {
    let path = std::env::temp_dir().join(format!("hodos_p6_{}_{}.db", tag, uuid::Uuid::new_v4()));
    let state = app_state_from(crate::database::WalletDatabase::new(path).unwrap(),
                               crate::services::WalletServices::with_test_chains(vec![], vec![]));
    state.shutdown.cancel();
    state
}

#[actix_web::test]
async fn p6_a17_wallet_recover_reports_rows_written() {
    let src = wallet("a17h");
    let phrase = src.get_cached_mnemonic().unwrap().to_string();
    let mut chain = FakeChain::new();
    let c7 = chain.pay(7, &receive_address(&src, 7), 5_000, our_p2pkh(&src, 7));
    let c45 = chain.pay(45, &receive_address(&src, 45), 7_000, our_p2pkh(&src, 45));
    let c9 = chain.pay(9, &receive_address(&src, 9), 6_000, our_p2pkh(&src, 9));
    chain.spent.insert((c9.clone(), 0), crate::reconcile::SpentStatus::Spent { spending_txid: format!("{:064x}", 9) });

    let state = empty_state("a17h");
    let req = RecoveryRequest { mnemonic: phrase, pin: None, gap_limit: None, start_index: None, max_index: None, confirm: Some(true) };
    let (code, body) = body_json(wallet_recover_with(state.clone(), web::Json(req), std::sync::Arc::new(chain)).await).await;
    assert_eq!(code, 200, "{}", body);
    assert_eq!(body["scan_running"], true, "the scan runs in the background (/code-review ultra 2, #1): {}", body);
    wait_for_scan(&state).await;
    let status = state.sync_status.read().unwrap().clone();
    assert_eq!(status.utxos_found, 2, "rows written (7 and 45), not scan hits (3)");
    assert_eq!(status.total_satoshis, 12_000);
    assert_eq!(status.error, None, "the scan was complete");
    assert!(coin(&state, &c7, 0).is_some() && coin(&state, &c45, 0).is_some() && coin(&state, &c9, 0).is_none());
}

#[actix_web::test]
async fn p6_a9_wallet_rescan_response_carries_the_self_only_note() {
    let state = app_state_from(wallet("a9h"), crate::services::WalletServices::with_test_chains(vec![], vec![]));
    state.shutdown.cancel();
    let chain = std::sync::Arc::new(FakeChain::new());
    let (code, body) = body_json(wallet_rescan_with(state.clone(), chain).await).await;
    assert_eq!(code, 200, "{}", body);
    assert_eq!(body["started"], true, "the button's request returns at once: {}", body);
    wait_for_scan(&state).await;
    let report = state.sync_status.read().unwrap().report.clone().expect("the scan's report");
    let note = report["note"].as_str().unwrap_or_default();
    assert!(note.contains("PeerPay") && note.contains("Claim a payment"), "{}", report);
    assert_eq!(report["complete"], true);
    assert_eq!(report["new_utxos_found"], 0);
}

// ---------------------------------------------------------------------------------------
// P6-A10 — the restored wallet's counter is the backup's; the next address is new
// ---------------------------------------------------------------------------------------

fn source_addresses(src: &Source) -> Vec<String> {
    (0..=10).map(|i| receive_address(&src.db, i)).collect()
}

#[actix_web::test]
async fn p6_a10_onchain_restore_restores_the_counter() {
    let src = source("a10src", 2, 50_000);
    src.db.connection().execute("UPDATE wallets SET current_index = 10", []).unwrap();
    let mut payload = payload_of(&src);
    payload.addresses.retain(|a| a.index <= 0); // the backup's strip
    assert!(payload.addresses.iter().map(|a| a.index).max().unwrap() < payload.wallet.current_index,
            "pre-condition: the rows stop below the counter");
    let t_raw = backup_spending(&src.x_txid, (40_000, our_p2pkh(&src.db, 0)));
    let t = txid_of(&t_raw);
    let (state, wid, uid) = target("a10", &src.phrase);
    let mut chain = FakeChain::new();
    chain.raws.insert(t.clone(), t_raw.clone());

    let f = finish(&state, wid, uid, &payload, &t, t_raw, &chain).await;
    assert!(f.scan_allowed);
    // Read the counter BEFORE the scan (a scan can raise it).
    let counter: i32 = state.database.lock().unwrap().connection().query_row(
        "SELECT current_index FROM wallets WHERE id = ?1", [wid], |r| r.get(0)).unwrap();
    assert_eq!(counter, 10, "the restored row's counter");
    let (code, body) = body_json(generate_address(state.clone(), web::Bytes::new()).await).await;
    assert_eq!(code, 200, "{}", body);
    assert_eq!(body["index"], 11);
    assert!(!source_addresses(&src).contains(&body["address"].as_str().unwrap().to_string()),
            "the next address is one the old wallet already handed out");
}

#[actix_web::test]
async fn p6_a10_file_import_restores_the_counter_on_the_restored_row() {
    let src = source("a10fsrc", 2, 50_000);
    src.db.connection().execute("UPDATE wallets SET current_index = 10", []).unwrap();
    let payload = payload_of(&src);
    assert_eq!(payload.wallet.id, 1);
    let backup = crate::backup::encrypt_backup(&payload, "p6-password").unwrap();

    let path = std::env::temp_dir().join(format!("hodos_p6_a10f_{}.db", uuid::Uuid::new_v4()));
    let db = crate::database::WalletDatabase::new(path).unwrap();
    // A failed earlier import leaves the id sequence advanced (its own `DELETE FROM wallets`).
    db.connection().execute("INSERT INTO wallets (mnemonic, current_index, backed_up, created_at, updated_at) VALUES ('x', 0, 0, 0, 0)", []).unwrap();
    db.connection().execute("DELETE FROM wallets", []).unwrap();
    let state = app_state_from(db, crate::services::WalletServices::with_test_chains(vec![], vec![]));
    state.shutdown.cancel();
    let chain = std::sync::Arc::new(FakeChain::new());
    let http = actix_web::test::TestRequest::default().to_http_request();
    let req = WalletImportRequest { pin: None, password: "p6-password".into(), backup };
    let (code, body) = body_json(wallet_import_with(state.clone(), http, web::Json(req), chain).await).await;
    assert_eq!(code, 200, "{}", body);
    let (id, counter): (i64, i32) = state.database.lock().unwrap().connection().query_row(
        "SELECT id, current_index FROM wallets", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(id, 2, "pre-condition: the restored row's id differs from the backup's (1)");
    assert_eq!(counter, 10, "the restored row's counter");
    assert_eq!(body["scan"]["ran"], "background", "the import started the scan in the background");
    wait_for_scan(&state).await;
    assert!(state.sync_status.read().unwrap().report.is_some(), "the import's scan left its report");
}

// ---------------------------------------------------------------------------------------
// P6-A14 — a failed reconcile is retried (re-fetching); a persistent failure is reported
// and the scan does not run
// ---------------------------------------------------------------------------------------

/// Truncated right after vout 1's value: the script-length varint is the empty slice.
fn truncated_after_vout1_value(raw: &[u8]) -> Vec<u8> {
    let (_, outs) = crate::reconcile::parse_tx_spans(raw).unwrap();
    let (_, off1, _) = outs[1];
    let value1_start = off1 - 1 - 8; // script offset − varint(1 byte) − value(8)
    raw[..value1_start + 8].to_vec()
}

#[actix_web::test]
async fn p6_a14_persistent_reconcile_failure_is_reported_and_the_scan_skipped() {
    let src = source("a14src", 2, 50_000);
    let payload = payload_of(&src);
    let intact = backup_spending(&src.x_txid, (40_000, our_p2pkh(&src.db, 4)));
    let f1 = truncated_after_vout1_value(&intact);
    let f1_txid = txid_of(&f1);
    let (state, wid, uid) = target("a14", &src.phrase);
    let mut chain = FakeChain::new();
    chain.raws.insert(f1_txid.clone(), f1.clone()); // every re-fetch returns the same bad bytes

    let f = finish(&state, wid, uid, &payload, &f1_txid, f1, &chain).await;
    assert!(f.reconcile.is_err(), "reconcile: {:?}", f.reconcile);
    assert_eq!(f.reconcile_attempts, BACKUP_RECONCILE_ATTEMPTS);
    assert_eq!(chain.raw_asked.lock().unwrap().iter().filter(|t| **t == f1_txid).count() as u32,
               BACKUP_RECONCILE_ATTEMPTS - 1, "each retry re-fetched");
    assert!(!f.scan_allowed, "an unreconciled backup must not be scanned");
    assert!(chain.asked.lock().unwrap().is_empty(), "the scan asked the chain for addresses");
    assert_eq!(f.reconcile_json()["reconciled"], false);
    assert!(coin(&state, &f1_txid, 2).is_none());
}

#[actix_web::test]
async fn p6_a14_a_retry_with_good_bytes_reconciles_and_scans() {
    let src = source("a14bsrc", 2, 50_000);
    let payload = payload_of(&src);
    let intact = backup_spending(&src.x_txid, (40_000, our_p2pkh(&src.db, 4)));
    let t = txid_of(&intact);
    let bad = truncated_after_vout1_value(&intact);
    let (state, wid, uid) = target("a14b", &src.phrase);
    let mut chain = FakeChain::new();
    chain.raws.insert(t.clone(), intact);

    let f = finish(&state, wid, uid, &payload, &t, bad, &chain).await;
    assert_eq!(f.reconcile, Ok((1, true)));
    assert_eq!(f.reconcile_attempts, 2);
    assert!(f.scan_allowed, "the scan is allowed after the retry succeeded");
    restore_scan(&state, wid, uid, &chain).await;
    assert!(!chain.asked.lock().unwrap().is_empty(), "the scan ran after the retry succeeded");
}

/// F2: the LAST output's script length is larger than the bytes left — today's hand
/// parser panicked here. (On the last output no later read can catch it: only the
/// output's own bounds check stands between these bytes and a slice panic.)
#[actix_web::test]
async fn p6_a14_corrupt_length_is_an_error_not_a_panic() {
    let src = source("a14csrc", 2, 50_000);
    let intact = backup_spending(&src.x_txid, (40_000, our_p2pkh(&src.db, 4)));
    let (_, outs) = crate::reconcile::parse_tx_spans(&intact).unwrap();
    let len_at = outs[2].1 - 1;
    let f2 = [&intact[..len_at], &[0xfd, 0xff, 0xff], &intact[len_at + 1..]].concat();
    let (state, _, _) = target("a14c", &src.phrase);
    let e = reconcile_backup_tx(&state, &txid_of(&f2), &f2, 0).expect_err("corrupt bytes reconciled");
    assert!(e.contains("truncated"), "{}", e);
}

// ---------------------------------------------------------------------------------------
// P6-A16 — the NC-11 rig is reachable only with HODOS_DEV=1 AND its own variable
// ---------------------------------------------------------------------------------------

#[test]
fn p6_a16_rig_needs_both_keys() {
    let env = |dev: Option<&'static str>, rig: bool| move |k: &str| match k {
        "HODOS_DEV" => dev.map(str::to_string),
        "HODOS_RIG_SKIP_BACKUP_RECONCILE" => rig.then(|| "1".to_string()),
        _ => None,
    };
    assert!(!skip_backup_reconcile_rig(env(None, true)), "rig on without HODOS_DEV");
    assert!(!skip_backup_reconcile_rig(env(Some("0"), true)));
    assert!(!skip_backup_reconcile_rig(env(Some("true"), true)));
    assert!(!skip_backup_reconcile_rig(env(Some("1"), false)));
    assert!(skip_backup_reconcile_rig(env(Some("1"), true)));
}

// ---------------------------------------------------------------------------------------
// restore_false_external_spends (P5 route W2): an explicit Spent on chain keeps a
// falsely-marked token row spent; Unknown restores it as before.
// ---------------------------------------------------------------------------------------

#[actix_web::test]
async fn p6_external_spend_rows_restored_unless_the_chain_says_spent() {
    let state = app_state_from(wallet("xsp"), crate::services::WalletServices::with_test_chains(vec![], vec![]));
    let (spent_txid, unknown_txid) = (format!("{:064x}", 0x5a), format!("{:064x}", 0x5b));
    {
        let d = state.database.lock().unwrap();
        let basket = crate::database::BasketRepository::new(d.connection()).find_or_insert("my-tokens", 1).unwrap();
        for t in [&spent_txid, &unknown_txid] {
            let id = raw_row(&d, t, 0, 1, Some(&[0x00, 0x6a]), true, None, None, false);
            d.connection().execute("UPDATE outputs SET spendable = 0, spending_description = 'external-spend', basket_id = ?1 WHERE outputId = ?2",
                                   rusqlite::params![basket, id]).unwrap();
        }
    }
    let mut chain = FakeChain::new();
    chain.spent.insert((spent_txid.clone(), 0), crate::reconcile::SpentStatus::Spent { spending_txid: format!("{:064x}", 0x5c) });
    let report = restore_false_external_spends(&state, &chain).await;
    assert_eq!(report["restored"], 1, "{}", report);
    assert_eq!(report["spent_on_chain"], 1, "{}", report);
    assert!(!coin(&state, &spent_txid, 0).unwrap().1, "a token row the chain says is spent was made spendable");
    assert!(coin(&state, &unknown_txid, 0).unwrap().1, "a falsely-marked token row was not restored");
}

// ---------------------------------------------------------------------------------------
// /code-review ultra 2, #1 — the request answers BEFORE the scan finishes (the wallet UI's
// bridge gives up after 30 s); the scan runs on in the background and reports in the
// sync status.
// ---------------------------------------------------------------------------------------

#[actix_web::test]
async fn p6_the_button_answers_before_the_scan_finishes() {
    let state = app_state_from(wallet("bg"), crate::services::WalletServices::with_test_chains(vec![], vec![]));
    state.shutdown.cancel();
    let open = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut chain = FakeChain::new();
    chain.gate = Some(open.clone());
    let answered = actix_web::rt::time::timeout(std::time::Duration::from_secs(3),
        wallet_rescan_with(state.clone(), std::sync::Arc::new(chain))).await;
    let resp = answered.expect("the request waited for the scan — it must answer at once");
    assert_eq!(resp.status().as_u16(), 200);
    assert!(state.sync_status.read().unwrap().active, "the scan is still running when the request answers");
    open.store(true, std::sync::atomic::Ordering::SeqCst);
    wait_for_scan(&state).await;
    assert!(state.sync_status.read().unwrap().report.is_some());
}
