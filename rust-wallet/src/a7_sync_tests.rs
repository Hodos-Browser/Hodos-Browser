//! B5-T2-P1 `P1-A7`, group **sync** — an ordinal arriving by a sync route ends FILED
//! (`1sat` / `bsv21`), or HELD where the route cannot carry a real script — never money.
//!
//! Subjects: the **real** envelope bytes of mainnet outputs (`tokens_tests` fixtures,
//! txid-verified) with their 25-byte P2PKH lock swapped for one of this wallet's receive
//! keys — a real token's script locked to us, labelled as such.
//!
//! Route coverage in this file:
//! - **P4 `ingest::complete_unchecked`** (via `complete_unchecked_from_chain`, the exact
//!   call `TaskSyncPending :: sweep_unchecked_scripts` makes): driven.
//! - I1 `check_addresses_individually` / `check_addresses_bulk`, I1 create-action refresh,
//!   I7 `wallet_sync`: NOT driven — each lists coins through `utxo_fetcher`, which calls
//!   WhatsOnChain / GorillaPool over a hard-coded `reqwest` URL with no injection point,
//!   and this file never touches the network. Whatever those routes write unchecked is
//!   completed and filed by the P4 route below.
//! - I7 `reconcile_spent_inputs`: NOT driven (its spent / unspent probes are hard-coded
//!   network calls). Supporting evidence only: its ownership gate
//!   (`reconcile::find_receive_index_upto`, was `recover_change_index`) admits an output only when its script is
//!   byte-for-byte a receive-key P2PKH, so an inscription is never written by it.
use std::sync::{Arc, Mutex};

use crate::classify::p5_tests::{change_of, our_p2pkh, selected, wallet};
use crate::classify::{list_held, Reason};
use crate::database::{OutputRepository, ParentTransactionRepository, WalletDatabase};
use crate::services::WalletServices;
use crate::tokens::tests::{real, BSV21_TRANSFER, ORDINAL};

fn varint(n: usize) -> Vec<u8> {
    match n {
        0..=0xfc => vec![n as u8],
        0xfd..=0xffff => [vec![0xfd], (n as u16).to_le_bytes().to_vec()].concat(),
        _ => [vec![0xfe], (n as u32).to_le_bytes().to_vec()].concat(),
    }
}

/// A one-input transaction paying `outputs` (value, script).
fn raw_tx(outputs: &[(i64, Vec<u8>)]) -> Vec<u8> {
    let mut t = 1u32.to_le_bytes().to_vec();
    t.extend(varint(1));
    t.extend([0x7a; 32]);
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

/// The real ordinal script (P2PKH first, then the envelope) with our lock in front.
fn ordinal_to(lock: &[u8]) -> Vec<u8> {
    let (_, s) = real(ORDINAL.0, ORDINAL.1);
    [lock, &s[25..]].concat()
}

/// The real BSV-21 transfer (envelope first, then P2PKH) with our lock at the end.
fn bsv21_to(lock: &[u8]) -> Vec<u8> {
    let (_, s) = real(BSV21_TRANSFER.0, BSV21_TRANSFER.1);
    [&s[..s.len() - 25], lock].concat()
}

struct RowState {
    id: i64,
    basket: Option<String>,
    script_length: Option<i64>,
}

fn row_state(db: &WalletDatabase, txid: &str, vout: u32) -> RowState {
    db.connection().query_row(
        "SELECT o.outputId, b.name, o.script_length FROM outputs o
           LEFT JOIN output_baskets b ON o.basket_id = b.basketId WHERE o.txid = ?1 AND o.vout = ?2",
        rusqlite::params![txid, vout],
        |r| Ok(RowState { id: r.get(0)?, basket: r.get(1)?, script_length: r.get(2)? })).unwrap()
}

fn held_reason(db: &WalletDatabase, txid: &str) -> Option<Reason> {
    list_held(db, 1).unwrap().into_iter().find(|h| h.txid.as_deref() == Some(txid)).map(|h| h.reason)
}

/// Write the coin exactly as a sync route writes one whose parent could not be read:
/// `upsert_received_utxo_with_confirmed` with the address-derived script and
/// `ScriptSource::Unchecked` (`script_length` NULL), then the route's own seam call.
fn write_unobserved(db: &WalletDatabase, txid: &str, vout: u32, sats: i64, index: i32) {
    let assumed = hex::encode(our_p2pkh(db, index));
    let n = OutputRepository::new(db.connection()).upsert_received_utxo_with_confirmed(
        1, txid, vout, sats, &assumed, index, true, crate::ingest::ScriptSource::Unchecked).unwrap();
    assert_eq!(n, 1);
    crate::classify::after_write(db, txid, vout, "I1:sync-individual");
}

/// The fake chain: every provider chain empty, so any fetch fails fast with no network.
/// The parent's bytes reach the route only through the local parent cache, which
/// `verified_raw_tx` reads first (and re-verifies against the txid).
fn offline_services() -> Arc<WalletServices> {
    Arc::new(WalletServices::with_test_chains(vec![], vec![]))
}

fn cache_parent(db: &Mutex<WalletDatabase>, txid: &str, raw: &[u8]) {
    let d = db.lock().unwrap();
    ParentTransactionRepository::new(d.connection()).upsert(None, txid, &hex::encode(raw)).unwrap();
}

/// `P1-A7` route **P4** — a 1-sat ordinal written unobserved by sync is held
/// `not_observed` (the seam did not read the assumed script); once the parent is
/// readable, `complete_unchecked` observes the real script and the row is FILED in
/// `1sat` — out of the balance, the held line and both selectors.
#[tokio::test]
async fn p1_a7_p4_unobserved_ordinal_is_filed_once_observed() {
    let db = wallet("a7p4");
    let script = ordinal_to(&our_p2pkh(&db, 0));
    let raw = raw_tx(&[(1, script.clone())]);
    let txid = txid_of(&raw);
    write_unobserved(&db, &txid, 0, 1, 0);
    assert_eq!(held_reason(&db, &txid), Some(Reason::NotObserved), "pre-state: held, not observed");

    let db = Mutex::new(db);
    let services = offline_services();
    // Parent unreadable: the row stays unchecked and held.
    let r0 = crate::ingest::complete_unchecked_from_chain(&db, &services, 0, 10).await;
    assert_eq!((r0.completed, r0.still_unchecked), (0, 1), "{:?}", r0);
    assert_eq!(held_reason(&db.lock().unwrap(), &txid), Some(Reason::NotObserved));

    cache_parent(&db, &txid, &raw);
    let r1 = crate::ingest::complete_unchecked_from_chain(&db, &services, 0, 10).await;
    assert_eq!(r1.completed, 1, "{:?}", r1);
    let d = db.lock().unwrap();
    let s = row_state(&d, &txid, 0);
    eprintln!("P4 ordinal row: basket={:?} script_length={:?} change={}", s.basket, s.script_length, change_of(&d, s.id));
    assert_eq!(s.script_length, Some(script.len() as i64), "the real script was observed");
    assert_eq!(s.basket.as_deref(), Some("1sat"), "observed ordinal must be FILED by the P4 route's seam call");
    assert!(!change_of(&d, s.id) && !selected(&d, s.id), "a filed ordinal is never money");
    assert_eq!(held_reason(&d, &txid), None, "filed, so not in the held line");
}

/// `P1-A7` route **P4** — the real BSV-21 transfer, same arrival, is filed in `bsv21`.
#[tokio::test]
async fn p1_a7_p4_unobserved_bsv21_is_filed_once_observed() {
    let db = wallet("a7p4b");
    let raw = raw_tx(&[(1, bsv21_to(&our_p2pkh(&db, 2)))]);
    let txid = txid_of(&raw);
    write_unobserved(&db, &txid, 0, 1, 2);
    let db = Mutex::new(db);
    cache_parent(&db, &txid, &raw);
    let r = crate::ingest::complete_unchecked_from_chain(&db, &offline_services(), 0, 10).await;
    assert_eq!(r.completed, 1, "{:?}", r);
    let d = db.lock().unwrap();
    let s = row_state(&d, &txid, 0);
    assert_eq!(s.basket.as_deref(), Some("bsv21"));
    assert!(!change_of(&d, s.id) && !selected(&d, s.id));
}

/// `P1-A7` route **P4**, the "never spendable" half — a 546-sat inscription (real BSV-21
/// transfer envelope, our lock last) observed by the P4 route is HELD `token_shaped`,
/// never money. RED-2 (recogniser → NotToken and rule ② → "ends with a P2PKH") makes
/// this row money — proving the guard is live on this route.
#[tokio::test]
async fn p1_a7_p4_multi_sat_inscription_is_never_money() {
    let db = wallet("a7p4m");
    let raw = raw_tx(&[(546, bsv21_to(&our_p2pkh(&db, 1)))]);
    let txid = txid_of(&raw);
    write_unobserved(&db, &txid, 0, 546, 1);
    let db = Mutex::new(db);
    cache_parent(&db, &txid, &raw);
    let r = crate::ingest::complete_unchecked_from_chain(&db, &offline_services(), 0, 10).await;
    assert_eq!(r.completed, 1, "{:?}", r);
    let d = db.lock().unwrap();
    let s = row_state(&d, &txid, 0);
    eprintln!("P4 546-sat row: basket={:?} change={} selected={} held={:?}",
              s.basket, change_of(&d, s.id), selected(&d, s.id), held_reason(&d, &txid));
    assert!(!change_of(&d, s.id), "546-sat inscription stamped money");
    assert!(!selected(&d, s.id), "546-sat inscription is selectable");
    assert_eq!(held_reason(&d, &txid), Some(Reason::TokenShaped));
}

/// Supporting evidence for **I7 `reconcile_spent_inputs`** (not a route drive): the
/// route writes a successor's output only when `find_receive_index_upto` finds a receive
/// index whose P2PKH byte-equals the output's script. Our bare receive P2PKH is found
/// (the gate can say yes); the same key under the real ordinal envelope is not — so the
/// route never ingests an inscription.
#[test]
fn p1_a7_i7_reconcile_gate_never_admits_an_inscription() {
    let db = wallet("a7i7");
    let wallet_id = crate::database::WalletRepository::new(db.connection())
        .get_primary_wallet().unwrap().unwrap().id.unwrap();
    let mp = crate::database::get_master_private_key_from_db(&db).unwrap();
    let mq = crate::database::get_master_public_key_from_db(&db).unwrap();
    let lock = our_p2pkh(&db, 0);
    assert_eq!(crate::reconcile::find_receive_index_upto(db.connection(), &mp, &mq, wallet_id, 0, &lock), Some(0),
               "subject check: the gate admits our bare receive P2PKH");
    assert_eq!(crate::reconcile::find_receive_index_upto(db.connection(), &mp, &mq, wallet_id, 0, &ordinal_to(&lock)), None);
    assert_eq!(crate::reconcile::find_receive_index_upto(db.connection(), &mp, &mq, wallet_id, 0, &bsv21_to(&lock)), None);
}
