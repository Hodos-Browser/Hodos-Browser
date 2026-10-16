//! B5-T2-P1 `P1-A7`, group **restore** — an ordinal arriving by each restore / repair
//! ingest route (B5-T1-P5 §2(c): I3, I9) ends FILED (`1sat` / `bsv21`), never money.
//!
//! Each test drives the route's own production function (not a mirror of it) over a
//! scratch temp DB, with the chain scripted (`WalletServices::with_test_chains`) — no
//! network, no dev wallet.
//!
//! Subjects: the REAL envelope bytes of mainnet outputs (`tokens_tests` fixtures,
//! txid-verified by `real()`) with their 25-byte P2PKH lock swapped for a key of this
//! wallet — a real token script locked to us, labelled as such. The "never spendable"
//! subject is the real BSV-21 transfer (envelope first, P2PKH last — the shape an
//! `ends_with(P2PKH)` rule would wrongly accept) on 1,000 sats.
//!
//! Routes not driven here (named in the P1-A7 result): `wallet_recover_onchain` (its
//! `fetch_onchain_backup` reads WhatsOnChain over raw `reqwest`, so the handler cannot run
//! offline) and `admin_prepare_unpublish` (its first step is `services.get_raw_tx`, a chain
//! `with_test_chains` leaves empty).
use super::*;
use super::p2_test_support::{app_state_from, body_json, status, ScriptedProvider};
use crate::classify::p5_tests::{change_of, master_p2pkh, our_p2pkh, raw_row, selected, wallet};
use crate::classify::Reason;
use crate::services::{BroadcastResult, IndexerProvider, TxState};
use crate::tokens::tests::{real, BSV21_TRANSFER, ORDINAL};
use rusqlite::OptionalExtension;

const RECV: Option<&str> = Some("2-receive address");
/// The multi-sat inscription: above the 1-sat filing rule and above `reconcile_backup_tx`'s
/// `> 546` gate, so only a script rule can keep it from money.
const MULTI: i64 = 1_000;

/// The real ordinal script (P2PKH first, then the envelope) with `lock` in front.
fn ordinal_to(lock: &[u8]) -> Vec<u8> {
    let (_, s) = real(ORDINAL.0, ORDINAL.1);
    [lock, &s[25..]].concat()
}

/// The real BSV-21 transfer (envelope first, then P2PKH) with `lock` at the end.
fn bsv21_to(lock: &[u8]) -> Vec<u8> {
    let (_, s) = real(BSV21_TRANSFER.0, BSV21_TRANSFER.1);
    [&s[..s.len() - 25], lock].concat()
}

fn varint(n: usize) -> Vec<u8> {
    match n {
        0..=0xfc => vec![n as u8],
        0xfd..=0xffff => [vec![0xfd], (n as u16).to_le_bytes().to_vec()].concat(),
        _ => [vec![0xfe], (n as u32).to_le_bytes().to_vec()].concat(),
    }
}

/// A one-input transaction paying `outputs` (value, script).
fn raw_tx(outputs: &[(i64, Vec<u8>)]) -> Vec<u8> {
    spending(&[0x5a; 32], outputs)
}

/// A one-input transaction spending `prev` (internal byte order):0, paying `outputs`.
fn spending(prev: &[u8; 32], outputs: &[(i64, Vec<u8>)]) -> Vec<u8> {
    let mut t = 1u32.to_le_bytes().to_vec();
    t.extend(varint(1));
    t.extend(prev);
    t.extend(0u32.to_le_bytes());
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

fn txid_of(raw: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(Sha256::digest(raw)).iter().rev().copied().collect::<Vec<u8>>())
}

/// What one stored outpoint looks like after the route ran.
#[derive(Debug)]
struct Stored { id: i64, basket: Option<String>, change: bool, selected: bool, held: Option<Reason> }

fn stored(db: &crate::database::WalletDatabase, txid: &str, vout: u32) -> Option<Stored> {
    let c = db.connection();
    let row: Option<(i64, Option<String>)> = c.query_row(
        "SELECT o.outputId, b.name FROM outputs o LEFT JOIN output_baskets b ON o.basket_id = b.basketId
          WHERE o.txid = ?1 AND o.vout = ?2", rusqlite::params![txid, vout],
        |r| Ok((r.get(0)?, r.get(1)?))).optional().unwrap();
    let (id, basket) = row?;
    let held = crate::classify::list_held(db, 1).unwrap().into_iter()
        .find(|h| h.txid.as_deref() == Some(txid) && h.vout == vout as i32).map(|h| h.reason);
    Some(Stored { id, basket, change: change_of(db, id), selected: selected(db, id), held })
}

fn must(db: &crate::database::WalletDatabase, txid: &str, vout: u32) -> Stored {
    stored(db, txid, vout).unwrap_or_else(|| panic!("route wrote no row for {}:{}", txid, vout))
}

/// A real wallet behind the real handlers; the chain says every txid is mined and
/// accepts any broadcast, binding it to `bind` when given.
fn rig(db: crate::database::WalletDatabase, bind: Option<String>) -> web::Data<AppState> {
    let mut p = ScriptedProvider::new("fake");
    p.status_for = Box::new(|t| Ok(status(t, TxState::Mined, None)));
    let bound = bind.unwrap_or_default();
    p.broadcast = Box::new(move |_| Ok(BroadcastResult { provider: "fake", txid: bound.clone(),
        tx_status: "SEEN_ON_NETWORK".into(), merkle_path_bump: None, block_height: None }));
    let p: std::sync::Arc<dyn IndexerProvider> = std::sync::Arc::new(p);
    app_state_from(db, crate::services::WalletServices::with_test_chains(vec![p.clone()], vec![p]))
}

fn assert_filed(s: &Stored, basket: &str, what: &str) {
    assert_eq!(s.basket.as_deref(), Some(basket), "{}: not filed by the seam on this route ({:?})", what, s);
    assert!(!s.change && !s.selected, "{}: a filed token is money ({:?})", what, s);
}

fn assert_never_money(s: &Stored, what: &str) {
    assert!(!s.change && !s.selected, "{}: a {}-sat inscription became MONEY ({:?})", what, MULTI, s);
    assert_eq!(s.held, Some(Reason::TokenShaped), "{}: held with the token reason ({:?})", what, s);
}

fn assert_money(s: &Stored, what: &str) {
    assert!(s.change && s.selected, "{}: control — plain coin not stamped money, so the seam did not run ({:?})", what, s);
}

// ---------------------------------------------------------------------------------------
// I3 — `wallet_import` (+ `backup::import_entities` beneath it), seam call
// `classify::stamp_all(&db, "I3:wallet-import")`.
// ---------------------------------------------------------------------------------------

/// Drive `POST /wallet/import` with an encrypted backup of a source wallet holding:
/// a 1-sat ordinal (txid `a7..01`), a 1-sat BSV-21 transfer (`a7..02`), the 1,000-sat
/// BSV-21 transfer (`a7..03`) and a plain 5,000-sat control (`a7..04`), all locked to
/// receive key 0, all written stamped in the source so the payload carries `change=1`.
async fn imported() -> web::Data<AppState> {
    let src = wallet("a7imp-src");
    let phrase = src.get_cached_mnemonic().unwrap().to_string();
    let lock = our_p2pkh(&src, 0);
    let t = |n: u8| format!("{:062x}{:02x}", 0xa7, n);
    raw_row(&src, &t(1), 0, 1, Some(&ordinal_to(&lock)), true, RECV, Some("0"), true);
    raw_row(&src, &t(2), 0, 1, Some(&bsv21_to(&lock)), true, RECV, Some("0"), true);
    raw_row(&src, &t(3), 0, MULTI, Some(&bsv21_to(&lock)), true, RECV, Some("0"), true);
    raw_row(&src, &t(4), 0, 5_000, Some(&lock), true, RECV, Some("0"), true);
    let ik: String = src.connection().query_row("SELECT identity_key FROM users ORDER BY userId LIMIT 1", [], |r| r.get(0)).unwrap();
    let payload = crate::backup::collect_payload(src.connection(), &ik, &phrase).unwrap();
    assert_eq!(payload.outputs.len(), 4, "fixture: the backup carries the four outputs");
    let backup = crate::backup::encrypt_backup(&payload, "a7-password").unwrap();

    let path = std::env::temp_dir().join(format!("hodos_a7imp_dst_{}.db", uuid::Uuid::new_v4()));
    let state = rig(crate::database::WalletDatabase::new(path).unwrap(), None);
    // The handler starts the Monitor; stop it before its first tick (no network).
    state.shutdown.cancel();
    let http = actix_web::test::TestRequest::default().to_http_request();
    let req = WalletImportRequest { pin: None, password: "a7-password".into(), backup };
    // B5-T1-P6: the import now runs the scan — an empty scripted chain keeps it offline.
    let chain = std::sync::Arc::new(crate::scan::tests::FakeChain::new());
    let (code, body) = body_json(wallet_import_with(state.clone(), http, web::Json(req), chain).await).await;
    assert_eq!(code, 200, "{}", body);
    state
}

/// GREEN (filed): through `wallet_import`, the ordinal lands `1sat` and the BSV-21
/// transfer `bsv21` — filed, not money; the plain control is money (the seam ran).
#[actix_web::test]
async fn a7_i3_wallet_import_files_the_ordinal() {
    let state = imported().await;
    let db = state.database.lock().unwrap();
    let t = |n: u8| format!("{:062x}{:02x}", 0xa7, n);
    assert_filed(&must(&db, &t(1), 0), "1sat", "I3:wallet-import ordinal");
    assert_filed(&must(&db, &t(2), 0), "bsv21", "I3:wallet-import BSV-21 transfer");
    assert_money(&must(&db, &t(4), 0), "I3:wallet-import");
}

/// GREEN (never spendable): through `wallet_import`, the 1,000-sat BSV-21 transfer
/// (payload said `change=1`) is held `token_shaped`, never money.
#[actix_web::test]
async fn a7_i3_wallet_import_never_makes_an_inscription_money() {
    let state = imported().await;
    let db = state.database.lock().unwrap();
    let t = |n: u8| format!("{:062x}{:02x}", 0xa7, n);
    assert_never_money(&must(&db, &t(3), 0), "I3:wallet-import");
}

// ---------------------------------------------------------------------------------------
// I3 — `reconcile_backup_tx` (called by `wallet_recover_onchain` after the import), seam
// call `classify::after_write(&db, backup_txid, 2, "I3:reconcile-backup-change")`.
// ---------------------------------------------------------------------------------------

/// A backup-shaped tx: vout 0 = a data output, vout 1 = a marker P2PKH (not ours),
/// vout 2 = the subject.
fn backup_tx(subject: (i64, Vec<u8>)) -> Vec<u8> {
    raw_tx(&[(1, vec![0x00, 0x6a, 0x01, 0x42]), (1, crate::classify::p5_tests::p2pkh_for_key(&[0x42u8; 32])), subject])
}

/// HONEST RESULT (route cannot carry a token): `reconcile_backup_tx` writes vout 2 only
/// when its script EQUALS the exact P2PKH of one of our addresses (and `> 546` sats). The
/// real ordinal (1 sat) and the BSV-21 transfer (1,000 sats) locked to address 0 are
/// therefore never written — not filed, not held, not money. The plain control at vout 2
/// IS written and stamped money by this route's seam call (so the route was reached and
/// its seam call is live).
#[actix_web::test]
async fn a7_i3_reconcile_backup_change_writes_no_token_row() {
    let state = rig(wallet("a7rbt"), None);
    let lock = {
        let db = state.database.lock().unwrap();
        let addr: String = db.connection().query_row(
            "SELECT address FROM addresses WHERE \"index\" = 0", [], |r| r.get(0)).unwrap();
        crate::recovery::address_to_p2pkh_script(&addr).unwrap()
    };
    let ord = backup_tx((1, ordinal_to(&lock)));
    let multi = backup_tx((MULTI, bsv21_to(&lock)));
    let plain = backup_tx((5_000, lock.clone()));
    for raw in [&ord, &multi, &plain] {
        reconcile_backup_tx(&state, &txid_of(raw), raw, 0).expect("the route parses the tx");
    }
    let db = state.database.lock().unwrap();
    assert!(stored(&db, &txid_of(&ord), 2).is_none(), "ordinal at vout 2 was written");
    assert!(stored(&db, &txid_of(&multi), 2).is_none(), "{}-sat inscription at vout 2 was written", MULTI);
    // vouts 0/1 go to `wallet-backup` under '1-wallet-backup' — outside the seam's
    // candidates, never money.
    let m = must(&db, &txid_of(&ord), 1);
    assert_eq!(m.basket.as_deref(), Some("wallet-backup"));
    assert!(!m.change && !m.selected);
    assert_money(&must(&db, &txid_of(&plain), 2), "I3:reconcile-backup-change");
}

// ---------------------------------------------------------------------------------------
// I9 — `debug_repair_nosend`, seam call `after_write_id(.., "I9:debug-repair-nosend")`.
// The route re-creates the change row with `insert_output` (script from the stored raw
// tx, `script_length` set — observed), derivation NULL ⇒ the master key.
// ---------------------------------------------------------------------------------------

fn nosend_row(db: &crate::database::WalletDatabase, raw: &[u8]) -> String {
    tx_row(db, raw, "nosend")
}

fn tx_row(db: &crate::database::WalletDatabase, raw: &[u8], status: &str) -> String {
    let txid = txid_of(raw);
    db.connection().execute(
        "INSERT INTO transactions (user_id, txid, status, reference_number, description, raw_tx,
                                   is_outgoing, satoshis, created_at, updated_at)
         VALUES (1, ?1, ?4, ?2, 'P1-A7 fixture', ?3, 1, 0, 0, 0)",
        rusqlite::params![txid, format!("a7-{}", &txid[..12]), hex::encode(raw), status]).unwrap();
    txid
}

/// Repair three nosend txs whose vout 0 is: the 1-sat ordinal, the 1,000-sat BSV-21
/// transfer, a plain 5,000-sat control — all locked to the master key.
async fn repaired() -> (web::Data<AppState>, [String; 3]) {
    let state = rig(wallet("a7rep"), None);
    let lock = { let db = state.database.lock().unwrap(); master_p2pkh(&db) };
    let subjects = [(1, ordinal_to(&lock)), (MULTI, bsv21_to(&lock)), (5_000, lock.clone())];
    let mut txids = Vec::new();
    for (sats, script) in subjects {
        let raw = raw_tx(&[(sats, script)]);
        let txid = { let db = state.database.lock().unwrap(); nosend_row(&db, &raw) };
        let body = serde_json::json!({ "txid": txid, "change_vout": 0, "change_sats": sats });
        let (code, json) = body_json(debug_repair_nosend(state.clone(), web::Json(body)).await).await;
        assert_eq!(code, 200, "{}", json);
        txids.push(txid);
    }
    (state, txids.try_into().unwrap())
}

#[actix_web::test]
async fn a7_i9_debug_repair_nosend_files_the_ordinal() {
    let (state, [ord, _, plain]) = repaired().await;
    let db = state.database.lock().unwrap();
    assert_filed(&must(&db, &ord, 0), "1sat", "I9:debug-repair-nosend ordinal");
    assert_money(&must(&db, &plain, 0), "I9:debug-repair-nosend");
}

#[actix_web::test]
async fn a7_i9_debug_repair_nosend_never_makes_an_inscription_money() {
    let (state, [_, multi, _]) = repaired().await;
    let db = state.database.lock().unwrap();
    assert_never_money(&must(&db, &multi, 0), "I9:debug-repair-nosend");
}

// ---------------------------------------------------------------------------------------
// I9 — `debug_broadcast_nosend`, seam call `after_write_id(.., "I9:debug-broadcast-nosend")`.
// After a (scripted) accepted broadcast the route re-creates the LAST output with
// `insert_output` (script from the stored raw tx), derivation NULL ⇒ the master key.
// ---------------------------------------------------------------------------------------

async fn broadcast_one(sats: i64, lock_to: impl Fn(&[u8]) -> Vec<u8>, tag: &str) -> (web::Data<AppState>, String) {
    let db = wallet(tag);
    let lock = master_p2pkh(&db);
    // The route refuses a BEEF whose main tx has a parent it cannot find; the parent is
    // our own unproven tx, held locally (`fetch_transaction_for_beef` step 1).
    // Zero inputs, so its own ancestry is complete (an unproven parent's inputs must be
    // in the BEEF too).
    let parent = [&1u32.to_le_bytes()[..], &[0x00, 0x01], &((sats + 500) as u64).to_le_bytes(),
                  &[lock.len() as u8], &lock, &0u32.to_le_bytes()].concat();
    tx_row(&db, &parent, "unproven");
    let prev: [u8; 32] = { let mut h = hex::decode(txid_of(&parent)).unwrap(); h.reverse(); h.try_into().unwrap() };
    let raw = spending(&prev, &[(sats, lock_to(&lock))]);
    let txid = nosend_row(&db, &raw);
    let state = rig(db, Some(txid.clone()));
    let (code, json) = body_json(debug_broadcast_nosend(state.clone(),
        web::Json(serde_json::json!({ "txid": txid }))).await).await;
    assert_eq!((code, json["success"].as_bool()), (200, Some(true)), "{}", json);
    (state, txid)
}

#[actix_web::test]
async fn a7_i9_debug_broadcast_nosend_files_the_ordinal() {
    let (state, ord) = broadcast_one(1, ordinal_to, "a7bc-ord").await;
    let (pstate, plain) = broadcast_one(5_000, |l| l.to_vec(), "a7bc-plain").await;
    assert_filed(&must(&state.database.lock().unwrap(), &ord, 0), "1sat", "I9:debug-broadcast-nosend ordinal");
    assert_money(&must(&pstate.database.lock().unwrap(), &plain, 0), "I9:debug-broadcast-nosend");
}

#[actix_web::test]
async fn a7_i9_debug_broadcast_nosend_never_makes_an_inscription_money() {
    let (state, multi) = broadcast_one(MULTI, bsv21_to, "a7bc-multi").await;
    assert_never_money(&must(&state.database.lock().unwrap(), &multi, 0), "I9:debug-broadcast-nosend");
}
