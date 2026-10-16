// B5-T1-P7b — held messages (contract §5a): a PeerPay message is acknowledged only after its payment
// is credited or stored. Every row reads the database through SELECTs and the fake inbox's
// acknowledge log; the held table is read at acknowledge time through a separate connection.

use super::p7b_poller_tests::*;
use super::*;
use crate::handlers::p2_test_support::{app_state_from_with_tracker, status, ScriptedProvider};
use crate::services::{IndexerError, IndexerProvider, TxState, WalletServices};
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn sk(n: u8) -> [u8; 32] { [n; 32] }

fn state_tracked(db: crate::database::WalletDatabase, p: Arc<ScriptedProvider>, tracker: crate::chain_tracker::ChainTracker) -> actix_web::web::Data<AppState> {
    let chain: Vec<Arc<dyn IndexerProvider>> = vec![p];
    app_state_from_with_tracker(db, WalletServices::with_test_chains_and_outspend(chain.clone(), chain.clone(), chain), tracker)
}

/// A provider whose every status lookup fails as an outage would (never with "not found" text:
/// `tx_exists_verdict` reads that as "absent" and broadcasts).
fn outage() -> Arc<ScriptedProvider> {
    let mut p = ScriptedProvider::new("arc_gorillapool");
    p.status_for = Box::new(|_| Err(IndexerError::Transport("outage".into())));
    Arc::new(p)
}

/// An outage until `up` is set, then every lookup answers Mined.
fn switchable(up: Arc<std::sync::atomic::AtomicBool>) -> Arc<ScriptedProvider> {
    let mut p = ScriptedProvider::new("arc_gorillapool");
    p.status_for = Box::new(move |txid| if up.load(Ordering::SeqCst) {
        Ok(status(txid, TxState::Mined, None))
    } else {
        Err(IndexerError::Transport("outage".into()))
    });
    Arc::new(p)
}

fn mined() -> Arc<ScriptedProvider> {
    let mut p = ScriptedProvider::new("arc_gorillapool");
    p.status_for = Box::new(|txid| Ok(status(txid, TxState::Mined, None)));
    Arc::new(p)
}

fn held_row(path: &std::path::Path, id: &str) -> Option<crate::database::HeldMessage> {
    let conn = rusqlite::Connection::open(path).unwrap();
    crate::database::PeerPayRepository::all_held(&conn).unwrap().into_iter().find(|m| m.message_id == id)
}

fn notices(path: &std::path::Path) -> i64 {
    count(path, "SELECT COUNT(*) FROM peerpay_received WHERE notification_type='rejected'")
}

/// A valid Atomic payment from `sender_sk` to our key for `pfx sfx` (parent 1 BSV, 700 sats at vout 1).
fn atomic_payment(db: &crate::database::WalletDatabase, sender_sk: &[u8; 32]) -> (Vec<u8>, Vec<u8>) {
    let (parent, subject) = payment(our_brc29_script(db, sender_sk, "pfx", "sfx"));
    (token("pfx", "sfx", &atomic(&[parent], &subject), 700), subject)
}

/// `subject` with a height lock and sequence 0 on its one input.
fn locked(subject: &[u8], lock: u32) -> Vec<u8> {
    let mut s = subject.to_vec();
    let n = s.len();
    s[n - 4..].copy_from_slice(&lock.to_le_bytes());
    s[42..46].copy_from_slice(&0u32.to_le_bytes());
    s
}

/// `P7b-A7` ⭐ — a garbage body is stored byte for byte, and only THEN acknowledged; shown as held,
/// not rejected.
#[tokio::test]
async fn p7b_a7_garbage_is_stored_before_it_is_acknowledged() {
    let (db, path) = wallet();
    let mut body = b"\xff\xfe{not json".to_vec();
    body.extend((0..300u32).map(|i| i as u8));
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![msg("m-garbage", &sender_hex(&sk(0x31)), body.clone())]);

    run_with(&state, &inbox, 1_000, 0).await.unwrap();

    let row = held_row(&path, "m-garbage").expect("not stored");
    assert_eq!((row.sender, row.body, row.reason.as_str(), row.attempts, row.lock_time, row.next_check_at),
        (sender_hex(&sk(0x31)), Some(body), "unreadable", 1, None, None));
    assert_eq!(inbox.acked(), vec!["m-garbage".to_string()]);
    assert!(inbox.held_at_ack.lock().unwrap()[0].contains(&"m-garbage".to_string()), "acknowledged before the row was committed");
    assert_eq!(notices(&path), 0);
}

/// `P7b-A8` — every former acknowledge-without-copy site, and every unreadable one, now holds.
#[tokio::test]
async fn p7b_a8_every_former_loss_site_now_holds() {
    let (db, path) = wallet();
    let mut msgs = vec![];
    let mut expect = vec![];
    let mut add = |n: u8, body: Vec<u8>, reason: &'static str| {
        let id = format!("m-a8-{}", n);
        msgs.push(msg(&id, &sender_hex(&sk(0x40 + n)), body));
        expect.push((id, reason));
    };
    let (parent, subject) = payment(our_brc29_script(&db, &sk(0x47), "pfx", "sfx"));
    let (orphan_parent, orphan) = payment(our_brc29_script(&db, &sk(0x48), "pfx", "sfx"));
    let _ = orphan_parent;
    use base64::Engine;
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    add(1, serde_json::to_vec(&serde_json::json!({"transaction": b64(&[1, 2, 3]), "amount": 1})).unwrap(), "missing_fields");
    add(2, serde_json::to_vec(&serde_json::json!({"customInstructions": {"derivationSuffix": "s"}, "transaction": b64(&[1])})).unwrap(), "missing_fields");
    add(3, serde_json::to_vec(&serde_json::json!({"customInstructions": {"derivationPrefix": "p"}, "transaction": b64(&[1])})).unwrap(), "missing_fields");
    add(4, serde_json::to_vec(&serde_json::json!({"customInstructions": {"derivationPrefix": "p", "derivationSuffix": "s"}})).unwrap(), "missing_fields");
    add(5, token("pfx", "sfx", &[1, 2, 3], 0), "not_beef");
    add(6, token("pfx", "sfx", &plain_v1(&[]), 0), "not_beef");
    add(7, token("pfx", "sfx", &atomic(&[parent], &subject), 701), "amount_mismatch");
    add(8, token("pfx", "sfx", &atomic(&[], &orphan), 700), "missing_parent");
    add(9, b"[1,2,3]".to_vec(), "unreadable");
    add(10, br#"{"customInstructions":{"derivationPrefix":"p","derivationSuffix":"s"},"transaction":"!!not-base64"}"#.to_vec(), "unreadable");
    add(11, br#"{"customInstructions":{"derivationPrefix":"p","derivationSuffix":"s"},"transaction":[1,"x",3]}"#.to_vec(), "unreadable");
    let bodies: Vec<(String, Vec<u8>)> = msgs.iter().map(|m| (m.message_id.clone(), m.body.clone())).collect();
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, msgs);

    run_with(&state, &inbox, 1_000, 0).await.unwrap();

    for (id, reason) in &expect {
        let row = held_row(&path, id).unwrap_or_else(|| panic!("{} not stored", id));
        assert_eq!((row.reason.as_str(), row.lock_time, row.next_check_at), (*reason, None, None), "{}", id);
        assert_eq!(row.body.as_deref(), bodies.iter().find(|(i, _)| i == id).map(|(_, b)| b.as_slice()), "{}", id);
        assert!(inbox.acked().contains(id), "{} not acknowledged", id);
    }
    assert_eq!(notices(&path), 0);
    assert_eq!(count(&path, "SELECT COUNT(*) FROM outputs"), 0);
}

/// `P7b-A9` — two-sided with A8: not our money, invalid by consensus, or no key can ever be derived
/// ⇒ rejected, acknowledged, no copy. One sender per fixture (the notice is one row per sender).
#[tokio::test]
async fn p7b_a9_what_can_never_be_ours_is_still_rejected() {
    let (db, path) = wallet();
    let mut msgs = vec![];
    let stranger = vec![0x76, 0xa9, 0x14, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 0x88, 0xac];
    let (p1, s1) = payment(stranger);
    msgs.push(msg("m-a9-1", &sender_hex(&sk(0x51)), token("pfx", "sfx", &atomic(&[p1], &s1), 0)));
    let parent = spend([0xf1; 32], 0, &[(1_000, vec![0x51])]);
    let over = spend(hash(&parent), 0, &[(1_001, our_brc29_script(&db, &sk(0x52), "pfx", "sfx"))]);
    msgs.push(msg("m-a9-2", &sender_hex(&sk(0x52)), token("pfx", "sfx", &atomic(&[parent], &over), 0)));
    msgs.push(msg("m-a9-6", "zz", token("pfx", "sfx", &[1], 0)));
    let mut bad_key = vec![0x05u8];
    bad_key.extend([0x11; 32]);
    msgs.push(msg("m-a9-7", &hex::encode(&bad_key), token("pfx", "sfx", &[1], 0)));
    let senders: Vec<String> = msgs.iter().map(|m| m.sender.clone()).collect();
    let ids: Vec<String> = msgs.iter().map(|m| m.message_id.clone()).collect();
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, msgs);

    run_with(&state, &inbox, 1_000, 0).await.unwrap();

    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages"), 0, "a payment that can never be ours was held");
    for (id, sender) in ids.iter().zip(&senders) {
        assert!(inbox.acked().contains(id), "{}", id);
        assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM peerpay_received WHERE message_id = 'reject:{}'", sender)), 1, "{}", id);
    }
    assert_eq!(count(&path, "SELECT COUNT(*) FROM outputs"), 0);
}

/// `P7b-A10` ⭐ — a hold that cannot be written is NOT acknowledged, and does not stop the rest of
/// the tick; once the write works the message is stored, then acknowledged.
#[tokio::test]
async fn p7b_a10_a_failed_hold_write_is_not_acknowledged() {
    let (db, path) = wallet();
    let (ok_body, ok_subject) = atomic_payment(&db, &SENDER_SK);
    db.connection().execute_batch(
        "CREATE TRIGGER inj BEFORE INSERT ON peerpay_held_messages BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![
        msg("m-fail", &sender_hex(&sk(0x61)), b"garbage".to_vec()),
        msg("m-ok", &sender_hex(&SENDER_SK), ok_body),
    ]);

    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    assert_eq!(inbox.acked(), vec!["m-ok".to_string()], "only what was credited or stored may be acknowledged");
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages"), 0);
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}'", txid_hex(&ok_subject))), 1);

    state.database.lock().unwrap().connection().execute_batch("DROP TRIGGER inj;").unwrap();
    run_with(&state, &inbox, 1_060, 0).await.unwrap();
    assert!(held_row(&path, "m-fail").is_some());
    assert_eq!(inbox.acks.lock().unwrap().last().unwrap(), &vec!["m-fail".to_string()]);
    assert!(inbox.held_at_ack.lock().unwrap().last().unwrap().contains(&"m-fail".to_string()));
}

/// Seed `n` held rows for `sender` directly (the V30 columns).
fn seed_held(path: &std::path::Path, sender: &str, n: usize, tag: &str) {
    let conn = rusqlite::Connection::open(path).unwrap();
    for i in 0..n {
        crate::database::PeerPayRepository::save_held(&conn, &crate::database::HeldMessage {
            message_id: format!("seed-{}-{}", tag, i), sender: sender.into(), body: Some(b"x".to_vec()),
            derivation_prefix: None, derivation_suffix: None, txid: None, reason: "unreadable".into(),
            held_at: 1, lock_time: None, next_check_at: None, attempts: 1,
        }).unwrap();
    }
}

/// `P7b-A11` — the 256 KiB body cap, both sides of the boundary; it turns a HOLD into a reject and
/// never refuses a payment we can credit.
#[tokio::test]
async fn p7b_a11_body_cap_applies_to_holds_only() {
    let (db, path) = wallet();
    let at = vec![b'x'; HELD_BODY_MAX];
    let over = vec![b'x'; HELD_BODY_MAX + 1];
    let filler = (1i64, vec![0x6a; 60_000]);
    let big_parent = spend([0xf2; 32], 0, &[filler.clone(), filler.clone(), filler.clone(), filler, (100_000_000, vec![0x51])]);
    let big_subject = spend(hash(&big_parent), 4, &[(700, our_brc29_script(&db, &SENDER_SK, "big", "one"))]);
    let big_ok = token("big", "one", &atomic(&[big_parent], &big_subject), 700);
    assert!(big_ok.len() > HELD_BODY_MAX, "fixture: the creditable payment must be over the cap");
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![
        msg("m-at", &sender_hex(&sk(0x71)), at),
        msg("m-over", &sender_hex(&sk(0x72)), over),
        msg("m-big-ok", &sender_hex(&SENDER_SK), big_ok),
    ]);

    run_with(&state, &inbox, 1_000, 0).await.unwrap();

    assert!(held_row(&path, "m-at").is_some(), "exactly at the cap is held");
    assert!(held_row(&path, "m-over").is_none(), "one byte over is not held");
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM peerpay_received WHERE message_id='reject:{}'", sender_hex(&sk(0x72)))), 1);
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}'", txid_hex(&big_subject))), 1, "a creditable payment over the cap was refused");
    assert_eq!(inbox.acked().len(), 3);
}

/// `P7b-A12` — 20 held per sender, both sides; another sender is unaffected.
#[tokio::test]
async fn p7b_a12_per_sender_cap() {
    let (db, path) = wallet();
    let s1 = sender_hex(&sk(0x81));
    seed_held(&path, &s1, (HELD_PER_SENDER_MAX - 1) as usize, "a12");
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![msg("m-20th", &s1, b"junk".to_vec()), msg("m-other", &sender_hex(&sk(0x82)), b"junk".to_vec())]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    assert!(held_row(&path, "m-20th").is_some() && held_row(&path, "m-other").is_some());

    inbox.messages.lock().unwrap().push(msg("m-21st", &s1, b"junk".to_vec()));
    run_with(&state, &inbox, 1_060, 0).await.unwrap();
    assert!(held_row(&path, "m-21st").is_none());
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM peerpay_received WHERE message_id='reject:{}'", s1)), 1);
    assert!(inbox.acked().contains(&"m-21st".to_string()));
}

/// `P7b-A13` — 500 held in total, both sides.
#[tokio::test]
async fn p7b_a13_total_cap() {
    let (db, path) = wallet();
    for i in 0..50u8 {
        seed_held(&path, &format!("seed-sender-{}", i), 10 - if i == 49 { 1 } else { 0 }, &format!("a13-{}", i));
    }
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages"), HELD_TOTAL_MAX - 1);
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![msg("m-500th", &sender_hex(&sk(0x91)), b"junk".to_vec())]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    assert!(held_row(&path, "m-500th").is_some());
    inbox.messages.lock().unwrap().push(msg("m-501st", &sender_hex(&sk(0x92)), b"junk".to_vec()));
    run_with(&state, &inbox, 1_060, 0).await.unwrap();
    assert!(held_row(&path, "m-501st").is_none());
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages"), HELD_TOTAL_MAX);
}

/// `P7b-A14` — the caps never refuse a re-check of a held row, nor a re-listed held message.
#[tokio::test]
async fn p7b_a14_caps_never_refuse_a_held_row() {
    let (db, path) = wallet();
    let (body, _) = atomic_payment(&db, &SENDER_SK);
    let s = sender_hex(&SENDER_SK);
    seed_held(&path, &s, (HELD_PER_SENDER_MAX - 1) as usize, "a14");
    let state = state_with(db, outage());
    let inbox = MockInbox::new(&path, vec![msg("m-out", &s, body)]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap(); // the 20th row for this sender
    let before = held_row(&path, "m-out").unwrap();
    assert_eq!(before.reason, "chain_check_error");

    // (a) re-checked at its time, still failing: still held, attempts + 1
    run_with(&state, &inbox, before.next_check_at.unwrap(), 0).await.unwrap();
    let after = held_row(&path, "m-out").expect("a re-check under the cap dropped the row");
    assert_eq!(after.attempts, 2);
    assert_eq!(notices(&path), 0);
    // (b) re-listed (an earlier acknowledge failed): acknowledged, row unchanged
    inbox.messages.lock().unwrap().push(msg("m-out", &s, after.body.clone().unwrap()));
    run_with(&state, &inbox, after.next_check_at.unwrap() - 1, 0).await.unwrap();
    assert_eq!(held_row(&path, "m-out").unwrap(), after);
    assert_eq!(notices(&path), 0);
}

/// `P7b-A15` ⭐ — a 20-tick outage never acknowledges without a held row and never drops the payment
/// (the old code gave up after 10 and deleted it); the backoff doubles to an hour; once the chain
/// answers, it is credited and the row removed.
#[tokio::test]
async fn p7b_a15_an_outage_holds_and_never_drops() {
    let (db, path) = wallet();
    let (body, subject) = atomic_payment(&db, &SENDER_SK);
    let up = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let p = switchable(up.clone());
    let state = state_with(db, p.clone());
    let inbox = MockInbox::new(&path, vec![msg("m-out", &sender_hex(&SENDER_SK), body)]);
    let expected_wait = [60, 120, 240, 480, 960, 1920, 3600, 3600];
    for k in 1..=20i64 {
        let now = 1_000_000 + k * 3_600;
        run_with(&state, &inbox, now, 0).await.unwrap();
        let row = held_row(&path, "m-out").unwrap_or_else(|| panic!("dropped on tick {}", k));
        assert_eq!((row.reason.as_str(), row.attempts), ("chain_check_error", k), "tick {}", k);
        assert_eq!(row.next_check_at.unwrap() - now, expected_wait[(k as usize - 1).min(7)], "tick {}", k);
    }
    assert_eq!(inbox.acked(), vec!["m-out".to_string()], "acknowledged once, after the first store");
    assert_eq!((notices(&path), count(&path, "SELECT COUNT(*) FROM outputs")), (0, 0));
    assert_eq!(p.status_calls.load(Ordering::SeqCst), 20);

    up.store(true, Ordering::SeqCst);
    run_with(&state, &inbox, 1_000_000 + 21 * 3_600, 0).await.unwrap();
    assert!(held_row(&path, "m-out").is_none());
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}' AND vout=1 AND satoshis=700", txid_hex(&subject))), 1);
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_received WHERE message_id='m-out'"), 1);
}

// ---------- T0: plain BEEF through `resolve_brc29_credit` ----------

fn key() -> Vec<u8> {
    let secp = secp256k1::Secp256k1::new();
    secp256k1::PublicKey::from_secret_key(&secp, &secp256k1::SecretKey::from_slice(&[0x11; 32]).unwrap()).serialize().to_vec()
}
fn p2pkh_of(pubkey: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    let h = ripemd::Ripemd160::digest(sha2::Sha256::digest(pubkey));
    [&[0x76, 0xa9, 0x14][..], h.as_slice(), &[0x88, 0xac]].concat()
}
fn mainnet() -> crate::chain_tracker::ChainTracker {
    crate::chain_tracker::ChainTracker::from_chain(crate::header_chain::mainnet_header_chain().unwrap())
}
fn credit_of(r: &Result<Brc29Credit, CreditReject>) -> (String, u32, i64, Vec<u8>, Vec<u8>) {
    let c = r.as_ref().unwrap_or_else(|e| panic!("not credited: {:?}", e));
    (c.subject_txid.clone(), c.vout, c.satoshis, c.script.clone(), c.credited_tx_bytes.clone())
}

/// `P7b-A1` + `P7b-A2` — a plain BEEF V1 and V2 credit exactly what the Atomic envelope of the same
/// transaction credits. Today's red (seam commit): `NotAtomicBeef("…magic prefix: [01, 00, be, ef]")`.
#[test]
fn p7b_a1_a2_plain_v1_and_v2_credit_like_atomic() {
    let (parent, subject) = payment(p2pkh_of(&key()));
    let atomic_credit = credit_of(&resolve_brc29_credit(&atomic(&[parent.clone()], &subject), &key(), Some(700), &mainnet()));
    assert_eq!(atomic_credit, (txid_hex(&subject), 1, 700, p2pkh_of(&key()), subject.clone()));
    let v1 = plain_v1(&[parent.clone(), subject.clone()]);
    let mut b = crate::beef::Beef::new();
    b.add_parent_transaction(parent);
    b.set_main_transaction(subject);
    let v2 = b.to_bytes().unwrap();
    assert_eq!(&v2[..4], &crate::beef::BEEF_V2_MARKER);
    assert_eq!(credit_of(&resolve_brc29_credit(&v1, &key(), Some(700), &mainnet())), atomic_credit, "V1");
    assert_eq!(credit_of(&resolve_brc29_credit(&v2, &key(), Some(700), &mainnet())), atomic_credit, "V2");
}

/// `P7b-A4` — a plain BEEF's subject is its LAST transaction (`@bsv/sdk` `Transaction.fromBEEF`): an
/// earlier transaction paying us is not credited; two-sided with the last one paying us.
#[test]
fn p7b_a4_the_plain_subject_is_the_last_transaction() {
    let fpar = spend([0xf3; 32], 0, &[(100_000, vec![0x51])]);
    let e = spend(hash(&fpar), 0, &[(5_000, p2pkh_of(&key()))]);
    let l = spend(hash(&e), 0, &[(4_000, vec![0x51])]);
    let r = resolve_brc29_credit(&plain_v1(&[fpar.clone(), e.clone(), l]), &key(), None, &mainnet());
    // V15 (owner 2026-10-11): not credited (the subject is the last tx) — but held, not rejected.
    assert!(matches!(r, Err(CreditReject::PaysOutsideSubject)), "{:?}", r.map(|c| c.subject_txid));
    assert_eq!(r.err().unwrap().decision(1), Decision::Hold { reason: "not_subject", wait: Wait::Start });
    let l2 = spend(hash(&e), 0, &[(1, vec![0x51]), (700, p2pkh_of(&key()))]);
    let c = credit_of(&resolve_brc29_credit(&plain_v1(&[fpar, e, l2.clone()]), &key(), None, &mainnet()));
    assert_eq!((c.0, c.1, c.2), (txid_hex(&l2), 1, 700));
}

/// `P7b-A5` — the plain fallback does not reopen CU-3: an Atomic header naming a txid not in the
/// bundle, and a trailing byte (Atomic and plain), are still refused — funded fixtures, so only the
/// parse can refuse them.
#[test]
fn p7b_a5_the_fallback_does_not_reopen_cu3() {
    let (parent, subject) = payment(p2pkh_of(&key()));
    let other = spend([0xcc; 32], 0, &[(5, vec![0x51])]);
    let mut beef = crate::beef::Beef::new();
    beef.add_parent_transaction(parent.clone());
    beef.set_main_transaction(subject.clone());
    let absent = hex::decode(beef.to_atomic_beef_hex(&txid_hex(&other)).unwrap()).unwrap();
    let mut trailing_atomic = atomic(&[parent.clone()], &subject);
    trailing_atomic.push(0);
    let mut trailing_plain = plain_v1(&[parent, subject]);
    trailing_plain.push(0);
    for (name, bytes, needle) in [("absent", absent, "not a transaction in the bundle"), ("atomic+1", trailing_atomic, "trailing byte"), ("plain+1", trailing_plain, "trailing byte")] {
        let r = resolve_brc29_credit(&bytes, &key(), None, &mainnet());
        assert!(matches!(&r, Err(CreditReject::NotAtomicBeef(e)) if e.contains(needle)), "{}: {:?}", name, r.map(|c| c.satoshis));
    }
}

/// `P7b-A6` — a plain BEEF goes through the same checks as Atomic: fee, missing parent, amount.
#[test]
fn p7b_a6_plain_beef_runs_the_same_checks() {
    let parent = spend([0xf4; 32], 0, &[(1_000, vec![0x51])]);
    let over = spend(hash(&parent), 0, &[(1_001, p2pkh_of(&key()))]);
    let r = resolve_brc29_credit(&plain_v1(&[parent, over]), &key(), None, &mainnet());
    assert!(matches!(r, Err(CreditReject::Spv(crate::spv_checks::Refusal::Fee { inputs: 1_000, outputs: 1_001 }))));
    let (_, orphan) = payment(p2pkh_of(&key()));
    let r = resolve_brc29_credit(&plain_v1(&[orphan]), &key(), None, &mainnet());
    assert!(matches!(r, Err(CreditReject::Spv(crate::spv_checks::Refusal::MissingParent { .. }))));
    let (parent, subject) = payment(p2pkh_of(&key()));
    let r = resolve_brc29_credit(&plain_v1(&[parent, subject]), &key(), Some(701), &mainnet());
    assert!(matches!(r, Err(CreditReject::AmountMismatch { declared: 701, found: 700 })));
}

/// `P7b-A32` (K-bp) and `A28`'s decision half — a BUMP that cannot be computed is rejected; a
/// root mismatch near our tip is held flat; a settled mismatch is rejected.
#[test]
fn p7b_a32_malformed_proof_rejected_recent_mismatch_held() {
    use crate::spv_checks::Refusal;
    let bad = |height, reason: &str| CreditReject::Spv(Refusal::BadProof { tx: 0, height, reason: reason.into() });
    assert_eq!(bad(u32::MAX, "BUMP has no levels").decision(103), Decision::Hold { reason: "bad_proof", wait: Wait::Start }, "V15");
    assert_eq!(bad(101, crate::spv_checks::ROOT_MISMATCH).decision(103), Decision::Hold { reason: "recent_bad_proof", wait: Wait::Flat });
    assert_eq!(bad(101, crate::spv_checks::ROOT_MISMATCH).decision(111), Decision::Reject);
    assert_eq!(CreditReject::Spv(Refusal::HeadersUnavailable { lock_time: 5 }).decision(1), Decision::Hold { reason: "headers_unavailable", wait: Wait::Flat });
}

// ---------- T1: schedules, restarts, dedupe, text, schema, summary, backup ----------

/// `P7b-A16` + `P7b-A28` — the schedules: outage backoff 60 s doubling to an hour; headers / recent
/// proof every 60 s; a row is not looked at one second before its time, and is at its time.
#[tokio::test]
async fn p7b_a16_a28_schedules_and_their_boundary() {
    assert_eq!((1..=9).map(|a| schedule(Wait::Backoff, a, 0).1.unwrap()).collect::<Vec<_>>(), vec![60, 120, 240, 480, 960, 1920, 3600, 3600, 3600]);
    assert_eq!(schedule(Wait::Flat, 7, 1_000), (None, Some(1_060)));
    assert_eq!(schedule(Wait::Lock(150), 1, 1_000), (Some(150), None));
    assert_eq!(schedule(Wait::Start, 3, 1_000), (None, None));

    let (db, path) = wallet();
    let (body, _) = atomic_payment(&db, &SENDER_SK);
    let p = outage();
    let state = state_with(db, p.clone());
    let inbox = MockInbox::new(&path, vec![msg("m-b", &sender_hex(&SENDER_SK), body)]);
    run_with(&state, &inbox, 5_000, 0).await.unwrap();
    let t = held_row(&path, "m-b").unwrap().next_check_at.unwrap();
    run_with(&state, &inbox, t - 1, 0).await.unwrap();
    assert_eq!((p.status_calls.load(Ordering::SeqCst), held_row(&path, "m-b").unwrap().attempts), (1, 1), "looked at before its time");
    run_with(&state, &inbox, t, 0).await.unwrap();
    assert_eq!((p.status_calls.load(Ordering::SeqCst), held_row(&path, "m-b").unwrap().attempts), (2, 2), "not looked at at its time");

    // headers unavailable (an unsynced tracker, a lock): held flat
    let (db, path) = wallet();
    let (parent, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx", "sfx"));
    let body = token("pfx", "sfx", &atomic(&[parent], &locked(&subject, 960_000)), 700);
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![msg("m-h", &sender_hex(&SENDER_SK), body)]);
    run_with(&state, &inbox, 7_000, 0).await.unwrap();
    let row = held_row(&path, "m-h").unwrap();
    assert_eq!((row.reason.as_str(), row.next_check_at, row.lock_time), ("headers_unavailable", Some(7_060), None));
    // the first wait of the backoff is also 60 s: the SECOND hold tells the schedules apart
    run_with(&state, &inbox, 7_060, 0).await.unwrap();
    let row = held_row(&path, "m-h").unwrap();
    assert_eq!((row.attempts, row.next_check_at), (2, Some(7_120)), "headers unavailable must wait 60 s flat, not back off");
}

/// A wallet over an existing database file (a restart), unlocked again (the mnemonic cache is in memory).
fn reopen(path: &std::path::Path) -> crate::database::WalletDatabase {
    let mut db = crate::database::WalletDatabase::new(path.to_path_buf()).unwrap();
    db.cache_mnemonic(MNEMONIC.to_string()).unwrap();
    db
}

/// `P7b-A17` — a payment held for its lock survives a restart, is not looked at while the lock
/// holds, and is credited from the stored row (the inbox is empty) once our chain passes it.
#[tokio::test]
async fn p7b_a17_a_lock_held_row_survives_restart_and_is_credited_from_the_row() {
    let (db, path) = wallet();
    let (parent, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx", "sfx"));
    let subject = locked(&subject, 150);
    let body = token("pfx", "sfx", &atomic(&[parent], &subject), 700);
    let chain = |more| { let t = super::tests::chain_with_block([0x99; 32], more); t.mark_synced_for_test(); t };
    let state = state_tracked(db, mined(), chain(10)); // tip 111
    let inbox = MockInbox::new(&path, vec![msg("m-lock", &sender_hex(&SENDER_SK), body)]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    let held = held_row(&path, "m-lock").expect("not held");
    assert_eq!((held.reason.as_str(), held.lock_time, held.next_check_at), ("non_final", Some(150), None));
    assert_eq!(inbox.acked(), vec!["m-lock".to_string()]);
    drop(state);

    let p = mined();
    let state = state_tracked(reopen(&path), p.clone(), chain(48)); // tip 149: 150 is not < 150
    let empty = MockInbox::new(&path, vec![]);
    run_with(&state, &empty, 2_000, i64::MAX).await.unwrap();
    assert_eq!(p.status_calls.load(Ordering::SeqCst), 0);
    assert_eq!(held_row(&path, "m-lock").unwrap(), held, "touched before its lock passed");
    drop(state);

    let state = state_tracked(reopen(&path), mined(), chain(49)); // tip 150: final
    run_with(&state, &empty, 3_000, 0).await.unwrap();
    assert!(held_row(&path, "m-lock").is_none());
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}' AND vout=1 AND satoshis=700", txid_hex(&subject))), 1);
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_received WHERE message_id='m-lock'"), 1);
}

/// `P7b-A18` — V4 closed: messages that used to park in the inbox (no transaction; our headers not
/// synced; a proof for a block next to our tip) are stored, acknowledged, and not listed again.
#[tokio::test]
async fn p7b_a18_parked_messages_are_stored_and_leave_the_inbox() {
    let (db, path) = wallet();
    let (parent, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx", "sfx"));
    let no_tx = serde_json::to_vec(&serde_json::json!({"customInstructions": {"derivationPrefix": "p", "derivationSuffix": "s"}})).unwrap();
    let not_synced = token("pfx", "sfx", &atomic(&[parent], &locked(&subject, 960_000)), 700);
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![msg("m-v4a", &sender_hex(&sk(0xa1)), no_tx), msg("m-v4b", &sender_hex(&SENDER_SK), not_synced)]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    run_with(&state, &inbox, 1_001, 0).await.unwrap();
    assert!(inbox.listed().is_empty(), "still parked: {:?}", inbox.listed());
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages"), 2);

    let (db, path) = wallet();
    let (_, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx", "sfx"));
    let recent = token("pfx", "sfx", &super::tests::bump_beef(&subject, 101), 700);
    let t = super::tests::chain_with_block([0x99; 32], 2); // tip 103: the mismatch at 101 is recent
    t.mark_synced_for_test();
    let state = state_tracked(db, mined(), t);
    let inbox = MockInbox::new(&path, vec![msg("m-v4c", &sender_hex(&SENDER_SK), recent)]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    assert!(inbox.listed().is_empty());
    assert_eq!(held_row(&path, "m-v4c").unwrap().reason, "recent_bad_proof");
}

/// `P7b-A19` — a payment credited from a held row is stored exactly as a direct credit is.
#[tokio::test]
async fn p7b_a19_a_credit_from_a_held_row_keeps_the_derivation() {
    let (db, path) = wallet();
    let (body, subject) = atomic_payment(&db, &SENDER_SK);
    let up = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let state = state_with(db, switchable(up.clone()));
    let inbox = MockInbox::new(&path, vec![msg("m-19", &sender_hex(&SENDER_SK), body)]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    up.store(true, Ordering::SeqCst);
    run_with(&state, &inbox, 1_060, 0).await.unwrap();
    let row: (i64, i64, String, String, String, i64) = rusqlite::Connection::open(&path).unwrap().query_row(
        "SELECT vout, satoshis, sender_identity_key, derivation_prefix, derivation_suffix, spendable FROM outputs WHERE txid = ?1",
        [txid_hex(&subject)], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).unwrap();
    assert_eq!(row, (1, 700, sender_hex(&SENDER_SK), "2-3241645161d8".into(), "pfx sfx".into(), 1));
}

/// `P7b-A20` — a message already credited, or already held, whose acknowledge failed is acknowledged
/// again when re-listed — not decided again.
#[tokio::test]
async fn p7b_a20_already_credited_or_held_is_acknowledged_not_redecided() {
    let (db, path) = wallet();
    let (body, subject) = atomic_payment(&db, &SENDER_SK);
    let p = mined();
    let state = state_with(db, p.clone());
    let inbox = MockInbox::new(&path, vec![msg("m-paid", &sender_hex(&SENDER_SK), body), msg("m-held", &sender_hex(&sk(0xb1)), b"junk".to_vec())]);
    inbox.fail_next_ack.store(true, Ordering::SeqCst);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    let held = held_row(&path, "m-held").unwrap();
    assert_eq!(inbox.listed().len(), 2, "the acknowledge failed: both still listed");

    run_with(&state, &inbox, 1_001, 0).await.unwrap();
    assert!(inbox.listed().is_empty());
    assert_eq!(p.status_calls.load(Ordering::SeqCst), 1, "the credited message was decided again");
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}'", txid_hex(&subject))), 1);
    assert_eq!(held_row(&path, "m-held").unwrap(), held, "the held row was decided again");
}

/// `P7b-A21` + `P7b-A22` — sender-controlled text with a multi-byte character where the old code cut
/// at a byte offset (16 for ids and senders, 500 for bodies) is handled without a panic.
#[tokio::test]
async fn p7b_a21_a22_multibyte_text_never_panics() {
    let (db, path) = wallet();
    let id = format!("{}€zz", "a".repeat(15)); // '€' spans bytes 15..18
    let sender = format!("{}€zz", "b".repeat(15));
    let body = format!("{{{}é{}", "a".repeat(498), "x".repeat(20)).into_bytes(); // 'é' spans 499..501
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![
        msg(&id, &sender_hex(&sk(0xc1)), body),
        msg("m-sender", &sender, token("p", "s", &[1], 0)),
    ]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    assert!(held_row(&path, &id).is_some());
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM peerpay_received WHERE message_id='reject:{}'", sender)), 1);
}

/// `P7b-A24` — V30: the latest schema, and the held table with exactly these columns.
#[test]
fn p7b_a24_v30_held_table() {
    let (_db, path) = wallet();
    assert_eq!(count(&path, "SELECT COUNT(*) FROM schema_version WHERE version = 30"), 1);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let cols: Vec<(String, String, i64, i64)> = conn.prepare("PRAGMA table_info(peerpay_held_messages)").unwrap()
        .query_map([], |r| Ok((r.get(1)?, r.get(2)?, r.get(3)?, r.get(5)?))).unwrap().map(|r| r.unwrap()).collect();
    let expected: Vec<(String, String, i64, i64)> = [
        ("message_id", "TEXT", 0, 1), ("sender", "TEXT", 1, 0), ("derivation_prefix", "TEXT", 0, 0),
        ("derivation_suffix", "TEXT", 0, 0), ("txid", "TEXT", 0, 0), ("reason", "TEXT", 1, 0), ("held_at", "INTEGER", 1, 0),
        ("lock_time", "INTEGER", 0, 0), ("next_check_at", "INTEGER", 0, 0), ("attempts", "INTEGER", 1, 0), ("checked_at", "INTEGER", 0, 0), ("body", "BLOB", 0, 0),
    ].iter().map(|(n, t, nn, pk)| (n.to_string(), t.to_string(), *nn, *pk)).collect();
    assert_eq!(cols, expected);
}

/// `P7b-A25` — housekeeping never drops a held row, however old.
#[tokio::test]
async fn p7b_a25_purge_never_drops_a_held_row() {
    let (db, path) = wallet();
    seed_held(&path, "old-sender", 1, "a25");
    rusqlite::Connection::open(&path).unwrap().execute("UPDATE peerpay_held_messages SET held_at = 0", []).unwrap();
    let state = state_with(db, mined());
    crate::monitor::task_purge::run(&state).await.unwrap();
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages"), 1);
}

/// `P7b-A26` — `GET /wallet/peerpay/status` reports `held_count` from the table, and the held rows do
/// not add to `rejected_count` or `unread_count`.
#[actix_web::test]
async fn p7b_a26_held_count_is_reported() {
    let (db, path) = wallet();
    seed_held(&path, "s", 3, "a26"); // garbage: no derivation — not counted (V12)
    let conn = rusqlite::Connection::open(&path).unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    for (id, held_at, attempts) in [("pay-new", now, 1), ("pay-old", now - 3_601, 7)] {
        crate::database::PeerPayRepository::save_held(&conn, &crate::database::HeldMessage {
            message_id: id.into(), sender: "s".into(), body: Some(b"b".to_vec()), derivation_prefix: Some("p".into()),
            derivation_suffix: Some("s".into()), txid: Some("ab".repeat(32)), reason: "chain_check_error".into(), held_at,
            lock_time: None, next_check_at: None, attempts,
        }).unwrap();
    }
    let state = state_with(db, mined());
    let (code, json) = crate::handlers::p2_test_support::body_json(crate::handlers::peerpay_status(state).await).await;
    assert_eq!(code, 200);
    assert_eq!((json["held_count"].as_i64(), json["held_overdue_count"].as_i64(), json["rejected_count"].as_i64(), json["unread_count"].as_i64()),
        (Some(2), Some(1), Some(0), Some(0)));
}

/// `P7b-A29` — a row that waits for "wallet start" is looked at on the Monitor's first tick only.
#[tokio::test]
async fn p7b_a29_start_rows_are_rechecked_at_start_only() {
    let (db, path) = wallet();
    let (body, subject) = atomic_payment(&db, &SENDER_SK);
    rusqlite::Connection::open(&path).unwrap().execute(
        "INSERT INTO peerpay_held_messages (message_id, sender, body, reason, held_at, attempts) VALUES ('m-start', ?1, ?2, 'unreadable', 1, 1)",
        rusqlite::params![sender_hex(&SENDER_SK), body]).unwrap();
    let state = state_with(db, mined());
    let empty = MockInbox::new(&path, vec![]);
    run_with(&state, &empty, 1_000, 0).await.unwrap();
    assert!(held_row(&path, "m-start").is_some(), "looked at outside a start");
    run_with(&state, &empty, 1_001, i64::MAX).await.unwrap();
    assert!(held_row(&path, "m-start").is_none());
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}'", txid_hex(&subject))), 1);
}

/// `P7b-A30` — a listed message whose body is not a JSON string reaches the poller (its JSON text)
/// and is held, instead of being skipped and parked for good.
#[tokio::test]
async fn p7b_a30_a_non_string_body_is_held_not_skipped() {
    let listed = serde_json::json!({"messageId": "m-obj", "sender": "x", "body": {"customInstructions": {"derivationPrefix": "p"}}});
    let body = crate::messagebox::non_string_body(&listed);
    assert_eq!(body, br#"{"customInstructions":{"derivationPrefix":"p"}}"#.to_vec());
    let (db, path) = wallet();
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![msg("m-obj", &sender_hex(&sk(0xd1)), body)]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    assert_eq!(held_row(&path, "m-obj").unwrap().reason, "missing_fields");
    assert_eq!(inbox.acked(), vec!["m-obj".to_string()]);
}

/// `P7b-A31` (amended, K-b + owner 2026-10-11) — the backup carries a held payment as its derivation
/// and txid; only a payment NOT on chain yet also carries its message (the one case a restore cannot
/// recover from the chain); garbage is not carried. After import both are due at once: the one with
/// its message is credited from it, the one without from the chain (here: the transaction cache the
/// fetch reads first).
#[tokio::test]
async fn p7b_a31_backup_carries_derivation_txid_and_only_unbroadcast_bodies() {
    use base64::Engine;
    let (db, path) = wallet();
    db.connection().execute("INSERT INTO wallets (mnemonic, created_at, updated_at) VALUES ('(encrypted)', 0, 0)", []).unwrap();
    let (parent, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx", "sfx"));
    let keep_body = token("pfx", "sfx", &atomic(&[parent.clone()], &subject), 700);
    let (parent2, subject2) = payment(our_brc29_script(&db, &SENDER_SK, "p2", "s2"));
    let subject2 = { let mut s = subject2; s[0] = 2; s }; // a different txid, same shape
    let state = state_with(db, outage());
    let inbox = MockInbox::new(&path, vec![
        msg("m-keep", &sender_hex(&SENDER_SK), keep_body.clone()),
        msg("m-junk", &sender_hex(&sk(0xe1)), b"junk".to_vec()),
    ]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap(); // m-keep: chain_check_error (not on chain)
    crate::database::PeerPayRepository::save_held(&rusqlite::Connection::open(&path).unwrap(), &crate::database::HeldMessage {
        message_id: "m-mined".into(), sender: sender_hex(&SENDER_SK), body: Some(b"the message".to_vec()),
        derivation_prefix: Some("p2".into()), derivation_suffix: Some("s2".into()), txid: Some(txid_hex(&subject2)),
        reason: "missing_parent".into(), held_at: 900, lock_time: None, next_check_at: None, attempts: 1,
    }).unwrap();
    let payload = {
        let db = state.database.lock().unwrap();
        crate::backup::collect_payload(db.connection(), "test_identity_key", MNEMONIC).unwrap()
    };
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    assert_eq!(payload.peerpay_held, vec![
        crate::backup::BackupHeldPayment { message_id: "m-mined".into(), sender: sender_hex(&SENDER_SK), derivation_prefix: "p2".into(),
            derivation_suffix: "s2".into(), txid: txid_hex(&subject2), held_at: 900, body: None },
        crate::backup::BackupHeldPayment { message_id: "m-keep".into(), sender: sender_hex(&SENDER_SK), derivation_prefix: "pfx".into(),
            derivation_suffix: "sfx".into(), txid: txid_hex(&subject), held_at: 1_000, body: Some(b64(&keep_body)) },
    ]);

    let fresh_path = std::env::temp_dir().join(format!("hodos_p7b_{}.db", uuid::Uuid::new_v4()));
    let mut fresh = crate::database::WalletDatabase::new(fresh_path.clone()).unwrap();
    crate::backup::import_to_db(fresh.connection(), &payload).unwrap();
    fresh.cache_mnemonic(MNEMONIC.to_string()).unwrap();
    let keep = held_row(&fresh_path, "m-keep").unwrap();
    assert_eq!((keep.body, keep.reason.as_str(), keep.next_check_at), (Some(keep_body), "restored", Some(0)));
    assert_eq!(held_row(&fresh_path, "m-mined").unwrap().body, None);
    {
        let repo = crate::database::ParentTransactionRepository::new(fresh.connection());
        repo.upsert(None, &txid_hex(&subject2), &hex::encode(&subject2)).unwrap();
        repo.upsert(None, &txid_hex(&parent2), &hex::encode(&parent2)).unwrap();
    }
    let state = state_with(fresh, mined());
    run_with(&state, &MockInbox::new(&fresh_path, vec![]), 2_000, 0).await.unwrap();
    for (id, s) in [("m-keep", &subject), ("m-mined", &subject2)] {
        assert!(held_row(&fresh_path, id).is_none(), "{} still held", id);
        assert_eq!(count(&fresh_path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}' AND vout=1 AND satoshis=700", txid_hex(s))), 1, "{}", id);
    }
}

/// `P7b-A33` — a held row whose re-check turns into a rejection (a near-tip proof mismatch that
/// settles as our chain grows) is removed and the user is told.
#[tokio::test]
async fn p7b_a33_a_recheck_that_rejects_removes_the_row_and_tells_the_user() {
    let (db, path) = wallet();
    let (_, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx", "sfx"));
    let chain = |more| { let t = super::tests::chain_with_block([0x99; 32], more); t.mark_synced_for_test(); t };
    let state = state_tracked(db, mined(), chain(2));
    let inbox = MockInbox::new(&path, vec![msg("m-33", &sender_hex(&SENDER_SK), token("pfx", "sfx", &super::tests::bump_beef(&subject, 101), 700))]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    let due = held_row(&path, "m-33").unwrap().next_check_at.unwrap();
    drop(state);
    let state = state_tracked(reopen(&path), mined(), chain(10));
    run_with(&state, &MockInbox::new(&path, vec![]), due, 0).await.unwrap();
    assert!(held_row(&path, "m-33").is_none());
    assert_eq!(notices(&path), 1);
}

// ---------- Adversarial review fixes (2026-10-10), contract §5d ----------

/// `P7b-R1` (review HIGH) — with the held table full of junk, an HONEST payment that is waiting on a
/// transient condition (here our outage) is NOT rejected and NOT acknowledged: it stays in the inbox
/// and is credited once the outage ends. Before the fix the caps rejected it and the acknowledge
/// deleted the only copy of its derivation.
#[tokio::test]
async fn p7b_r1_a_full_table_never_costs_an_honest_payment() {
    let (db, path) = wallet();
    for i in 0..50u8 {
        seed_held(&path, &format!("junk-{}", i), 10, &format!("r1-{}", i)); // 500 lasting rows
    }
    let (body, subject) = atomic_payment(&db, &SENDER_SK);
    let up = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let state = state_with(db, switchable(up.clone()));
    let inbox = MockInbox::new(&path, vec![msg("m-honest", &sender_hex(&SENDER_SK), body)]);

    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    assert!(inbox.acked().is_empty(), "acknowledged while it could not be stored");
    assert_eq!(notices(&path), 0, "an honest payment held by our outage was rejected");
    assert_eq!(inbox.listed(), vec!["m-honest".to_string()]);

    up.store(true, Ordering::SeqCst);
    run_with(&state, &inbox, 1_060, 0).await.unwrap();
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}' AND vout=1 AND satoshis=700", txid_hex(&subject))), 1);
    assert_eq!(inbox.acked(), vec!["m-honest".to_string()]);
}

/// `P7b-R2` (review MEDIUM) — a payment the network refuses for good (every provider that answered
/// refused it) is held until wallet start, not rebroadcast every hour; a may-still-land failure
/// keeps the backoff (two-sided).
#[tokio::test]
async fn p7b_r2_a_permanent_refusal_is_not_rebroadcast_hourly() {
    // Review 164 #7: a refusal is looked at again hourly (not only at the next start, nor every minute).
    for (permanent, reason, next) in [(true, "broadcast_refused", Some(4_600)), (false, "broadcast_failed", Some(1_060))] {
        let (db, path) = wallet();
        let (body, _) = atomic_payment(&db, &SENDER_SK);
        let mut p = ScriptedProvider::new("arc_gorillapool");
        p.status_for = Box::new(|txid| Ok(status(txid, TxState::Unknown, None)));
        p.broadcast = Box::new(move |_| Err(if permanent {
            IndexerError::ProviderStatus { provider: "arc_gorillapool", status: 400, body: "txn-mempool-conflict: double spend".into() }
        } else {
            IndexerError::Transport("connection reset".into())
        }));
        let p = Arc::new(p);
        let state = state_with(db, p.clone());
        let inbox = MockInbox::new(&path, vec![msg("m-r2", &sender_hex(&SENDER_SK), body)]);
        run_with(&state, &inbox, 1_000, 0).await.unwrap();
        let row = held_row(&path, "m-r2").unwrap();
        assert_eq!((row.reason.as_str(), row.next_check_at), (reason, next), "permanent = {}", permanent);
        for k in 1..=5 {
            run_with(&state, &inbox, 1_000 + k * 300, 0).await.unwrap(); // all inside the hour
        }
        let sent = p.broadcast_hashes().len();
        if permanent {
            assert_eq!(sent, 1, "a refused transaction was broadcast again");
        } else {
            assert!(sent > 1, "a may-still-land failure was not retried");
        }
    }
}

/// `P7b-R3` (review MEDIUM) — one tick re-checks at most `RECHECKS_PER_TICK` held rows; the rest
/// stay due for the next tick.
#[tokio::test]
async fn p7b_r3_rechecks_per_tick_are_bounded() {
    let (db, path) = wallet();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let (body, _) = atomic_payment(&db, &SENDER_SK);
    for i in 0..25 {
        crate::database::PeerPayRepository::save_held(&conn, &crate::database::HeldMessage {
            message_id: format!("due-{:02}", i), sender: sender_hex(&SENDER_SK), body: Some(body.clone()),
            derivation_prefix: None, derivation_suffix: None, txid: None, reason: "chain_check_error".into(),
            held_at: 1, lock_time: None, next_check_at: Some(10), attempts: 1,
        }).unwrap();
    }
    let p = outage();
    let state = state_with(db, p.clone());
    run_with(&state, &MockInbox::new(&path, vec![]), 1_000, 0).await.unwrap();
    assert_eq!(p.status_calls.load(Ordering::SeqCst), RECHECKS_PER_TICK);
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages WHERE next_check_at = 10"), (25 - RECHECKS_PER_TICK) as i64);
}

/// `P7b-R4` (review MEDIUM; reworked for `checked_at`, review 164) — a poll on a locked wallet does
/// not count as a look at the held rows: start rows stay unstamped for the first poll that can decide.
#[tokio::test]
async fn p7b_r4_a_locked_wallet_does_not_use_up_the_start_recheck() {
    let path = std::env::temp_dir().join(format!("hodos_p7b_{}.db", uuid::Uuid::new_v4()));
    let locked_db = crate::database::WalletDatabase::new(path.clone()).unwrap(); // no mnemonic cached
    seed_row(&path, "start-row", "x", "unreadable", None, None);
    let state = state_with(locked_db, mined());
    run_with(&state, &MockInbox::new(&path, vec![]), 1_000, i64::MAX).await.unwrap();
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages WHERE checked_at IS NULL"), 1);
}

/// `P7b-R5` (review LOW + MEDIUM) — the backup carries only held payments a restore may credit: not
/// an amount mismatch (owner decision 3), not one with a malformed sender; the import respects the
/// total cap.
#[tokio::test]
async fn p7b_r5_backup_carries_only_restorable_held_payments() {
    let (db, path) = wallet();
    db.connection().execute("INSERT INTO wallets (mnemonic, created_at, updated_at) VALUES ('(encrypted)', 0, 0)", []).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let row = |id: &str, sender: &str, reason: &str| crate::database::HeldMessage {
        message_id: id.into(), sender: sender.into(), body: Some(b"b".to_vec()), derivation_prefix: Some("p".into()),
        derivation_suffix: Some("s".into()), txid: Some("ab".repeat(32)), reason: reason.into(), held_at: 5,
        lock_time: None, next_check_at: None, attempts: 1,
    };
    for r in [row("ok", &sender_hex(&SENDER_SK), "chain_check_error"), row("mismatch", &sender_hex(&SENDER_SK), "amount_mismatch"),
              row("bad-sender", "zz", "chain_check_error"), row("refused", &sender_hex(&SENDER_SK), "broadcast_refused")] {
        crate::database::PeerPayRepository::save_held(&conn, &r).unwrap();
    }
    let payload = crate::backup::collect_payload(db.connection(), "test_identity_key", MNEMONIC).unwrap();
    assert_eq!(payload.peerpay_held.iter().map(|h| h.message_id.as_str()).collect::<Vec<_>>(), vec!["ok"]);

    let fresh_path = std::env::temp_dir().join(format!("hodos_p7b_{}.db", uuid::Uuid::new_v4()));
    let fresh = crate::database::WalletDatabase::new(fresh_path.clone()).unwrap();
    seed_held(&fresh_path, "full", HELD_TOTAL_MAX as usize, "r5");
    crate::backup::import_to_db(fresh.connection(), &payload).unwrap();
    assert!(held_row(&fresh_path, "ok").is_none(), "the import went past the total cap");
}


// ---------- /code-review max 163 candidates, verified in code by the session (2026-10-11), contract §5e ----------

/// `P7b-V6` — the due-row query reads no bodies; the one row decided loads its own.
#[test]
fn p7b_v6_due_rows_carry_no_body() {
    let (_db, path) = wallet();
    seed_held(&path, "s", 2, "v6");
    let conn = rusqlite::Connection::open(&path).unwrap();
    let due = crate::database::PeerPayRepository::held_due(&conn, 0, i64::MAX).unwrap();
    assert_eq!(due.len(), 2);
    assert!(due.iter().all(|m| m.body.is_none()), "held_due loaded bodies");
    assert_eq!(crate::database::PeerPayRepository::held_body(&conn, "seed-v6-0").unwrap(), Some(b"x".to_vec()));
}

/// `P7b-V8` — a held row restored without its message gets the message when it is listed again,
/// before it is acknowledged. (B5-T5-P7a, review 178-3 #3: the fixture is now a real restored row —
/// derivation + txid — and the listed message names that derivation; a listed body that does not is
/// a contesting copy, `t5p7a_a20_*`.)
#[tokio::test]
async fn p7b_v8_a_restored_row_gets_its_body_back() {
    let (db, path) = wallet();
    let (body, subject) = atomic_payment(&db, &SENDER_SK);
    rusqlite::Connection::open(&path).unwrap().execute(
        "INSERT INTO peerpay_held_messages (message_id, sender, derivation_prefix, derivation_suffix, txid, reason, held_at, attempts, next_check_at)
         VALUES ('m-v8', ?1, 'pfx', 'sfx', ?2, 'restored', 1, 0, 999999)",
        [sender_hex(&SENDER_SK), txid_hex(&subject)]).unwrap();
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![msg("m-v8", &sender_hex(&SENDER_SK), body.clone())]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    assert_eq!(inbox.acked(), vec!["m-v8".to_string()]);
    // Filled before the acknowledge (the ack-time snapshot still holds the row), then — due at once
    // (`P7b-W10`) — credited from that body in the same poll.
    assert_eq!(*inbox.held_at_ack.lock().unwrap(), vec![vec!["m-v8".to_string()]]);
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}'", txid_hex(&subject))), 1);
    let _ = body;
}

/// `P7b-V14` — one lock rule for `check_final` and the held-row gate, both arms, both strict.
#[test]
fn p7b_v14_lock_passed_height_and_time() {
    use crate::spv_checks::lock_passed;
    assert!(!lock_passed(150, 149, 0), "height lock = tip + 1 is not passed");
    assert!(lock_passed(150, 150, 0));
    assert!(!lock_passed(1_700_000_000, 999_999, 1_700_000_000), "time lock = MTP is not passed");
    assert!(lock_passed(1_700_000_000, 0, 1_700_000_001));
    assert!(!lock_passed(1_700_000_000, 2_000_000_000, 0), "a time lock is not judged by height");
}

/// `P7b-V4` — a held payment already among the restored outputs (credited before the backup) is not
/// held again on import.
#[test]
fn p7b_v4_import_skips_a_payment_already_in_the_outputs() {
    let fresh_path = std::env::temp_dir().join(format!("hodos_p7b_{}.db", uuid::Uuid::new_v4()));
    let fresh = crate::database::WalletDatabase::new(fresh_path.clone()).unwrap();
    let (src, _) = wallet();
    src.connection().execute("INSERT INTO wallets (mnemonic, created_at, updated_at) VALUES ('(encrypted)', 0, 0)", []).unwrap();
    let mut payload = crate::backup::collect_payload(src.connection(), "test_identity_key", MNEMONIC).unwrap();
    payload.peerpay_held = vec![crate::backup::BackupHeldPayment {
        message_id: "m-v4".into(), sender: sender_hex(&SENDER_SK), derivation_prefix: "p".into(), derivation_suffix: "s".into(),
        txid: "cd".repeat(32), held_at: 1, body: None }];
    crate::backup::import_to_db(fresh.connection(), &payload).unwrap();
    assert!(held_row(&fresh_path, "m-v4").is_some(), "control: an unknown payment is held");

    let fresh_path = std::env::temp_dir().join(format!("hodos_p7b_{}.db", uuid::Uuid::new_v4()));
    let fresh = crate::database::WalletDatabase::new(fresh_path.clone()).unwrap();
    fresh.connection().execute("CREATE TEMP TRIGGER noop AFTER INSERT ON peerpay_held_messages BEGIN SELECT 1; END;", []).unwrap();
    fresh.connection().execute("INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at) VALUES (99, 'x', 'local', 0, 0)", []).unwrap();
    fresh.connection().execute(
        "INSERT INTO outputs (user_id, txid, vout, satoshis, locking_script, spendable, change, derivation_suffix, created_at, updated_at) VALUES (99, ?1, 1, 700, x'00', 1, 0, 'p s', 0, 0)",
        ["cd".repeat(32)]).unwrap();
    crate::backup::import_to_db(fresh.connection(), &payload).unwrap();
    assert!(held_row(&fresh_path, "m-v4").is_none(), "a payment already in the outputs was held again");
}


// ---------- /code-review xhigh 164 findings, verified in code by the session (2026-10-12), contract §5g ----------

fn seed_row(path: &std::path::Path, id: &str, sender: &str, reason: &str, next: Option<i64>, checked: Option<i64>) {
    let conn = rusqlite::Connection::open(path).unwrap();
    crate::database::PeerPayRepository::save_held(&conn, &crate::database::HeldMessage {
        message_id: id.into(), sender: sender.into(), body: Some(b"junk".to_vec()), derivation_prefix: None,
        derivation_suffix: None, txid: None, reason: reason.into(), held_at: 1, lock_time: None, next_check_at: next, attempts: 1,
    }).unwrap();
    if let Some(c) = checked {
        crate::database::PeerPayRepository::mark_held_checked(&conn, id, c).unwrap();
    }
}

/// `P7b-W1` (#1) — the wallet-start pass covers EVERY start row, over as many ticks as it needs: 25
/// rows, 20 per tick ⇒ two ticks, each row looked at once since this start; a later tick looks at none.
#[tokio::test]
async fn p7b_w1_the_start_pass_reaches_every_row() {
    let (db, path) = wallet();
    for i in 0..25 { seed_row(&path, &format!("s-{:02}", i), "x", "unreadable", None, None); }
    let state = state_with(db, mined());
    let empty = MockInbox::new(&path, vec![]);
    let started = 5_000;
    run_with(&state, &empty, 5_001, started).await.unwrap();
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages WHERE checked_at = 5001"), 20);
    run_with(&state, &empty, 5_002, started).await.unwrap();
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages WHERE checked_at = 5002"), 5);
    run_with(&state, &empty, 5_003, started).await.unwrap();
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages WHERE checked_at = 5003"), 0, "a start row was looked at twice");
}

/// `P7b-W2` (#2) — twenty rows due every minute cannot starve a newer due row: the least recently
/// checked go first.
#[tokio::test]
async fn p7b_w2_due_rows_rotate() {
    let (db, path) = wallet();
    for i in 0..20 { seed_row(&path, &format!("flat-{:02}", i), "x", "headers_unavailable", Some(0), Some(4_000)); }
    seed_row(&path, "newer", "x", "chain_check_error", Some(0), None);
    let state = state_with(db, mined());
    run_with(&state, &MockInbox::new(&path, vec![]), 5_000, 0).await.unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let c: Option<i64> = conn.query_row("SELECT checked_at FROM peerpay_held_messages WHERE message_id='newer'", [], |r| r.get(0)).unwrap();
    assert_eq!(c, Some(5_000), "the never-checked row was starved");
}

/// `P7b-W4` (owner 2026-10-12, re-decided) — over a sender's own limit, as over the total, a
/// TRANSIENT hold (a real payment waiting on our outage) stays in the inbox: never deleted; a lasting
/// one (junk) is rejected (two-sided).
#[tokio::test]
async fn p7b_w4_over_a_limit_transient_stays_lasting_is_rejected() {
    let (db, path) = wallet();
    let s = sender_hex(&SENDER_SK);
    seed_held(&path, &s, HELD_PER_SENDER_MAX as usize, "w4");
    let (body, _) = atomic_payment(&db, &SENDER_SK);
    let state = state_with(db, outage());
    let inbox = MockInbox::new(&path, vec![msg("m-real", &s, body), msg("m-junk", &s, b"junk".to_vec())]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    assert_eq!(inbox.acked(), vec!["m-junk".to_string()], "only the junk is rejected and acknowledged");
    assert_eq!(inbox.listed(), vec!["m-real".to_string()], "the real payment stays in the inbox");
    assert_eq!(notices(&path), 1);

    let (db, path) = wallet();
    for i in 0..50u8 { seed_held(&path, &format!("junk-{}", i), 10, &format!("w4b-{}", i)); }
    let (body, _) = atomic_payment(&db, &SENDER_SK);
    let state = state_with(db, outage());
    let inbox = MockInbox::new(&path, vec![msg("m-total", &sender_hex(&SENDER_SK), body)]);
    run_with(&state, &inbox, 1_000, 0).await.unwrap();
    assert!(inbox.acked().is_empty() && notices(&path) == 0, "over the total: left in the inbox");
}

/// `P7b-J8` (owner 2026-10-12) — held junk with no derivation is deleted after 7 days; a row that
/// names a derivation is not, however old (two-sided); fresh junk is not.
#[tokio::test]
async fn p7b_j8_junk_without_a_derivation_expires_after_a_week() {
    let (db, path) = wallet();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let now = 10_000_000;
    let save = |id: &str, prefix: Option<&str>, held_at: i64| crate::database::PeerPayRepository::save_held(&conn, &crate::database::HeldMessage {
        message_id: id.into(), sender: "s".into(), body: Some(b"b".to_vec()), derivation_prefix: prefix.map(|s| s.to_string()),
        derivation_suffix: prefix.map(|_| "s".to_string()), txid: None, reason: "unreadable".into(), held_at,
        lock_time: None, next_check_at: Some(now + 999_999), attempts: 1,
    }).unwrap();
    save("old-junk", None, now - JUNK_TTL_SECS - 1);
    save("new-junk", None, now - JUNK_TTL_SECS + 1);
    save("old-derivation", Some("p"), now - JUNK_TTL_SECS * 10);
    let state = state_with(db, mined());
    run_with(&state, &MockInbox::new(&path, vec![]), now, 0).await.unwrap();
    assert!(held_row(&path, "old-junk").is_none(), "week-old junk kept");
    assert!(held_row(&path, "new-junk").is_some(), "junk deleted before a week");
    assert!(held_row(&path, "old-derivation").is_some(), "a row naming a derivation was deleted");
}

/// `P7b-W6` (#6 + V4; scoped by review 164-5 #4/#5) — a payment restored from a BACKUP (it may be
/// older than a spend of its own coin) asks whether the output is spent: spent ⇒ final reject; no
/// answer ⇒ held, never credited as unspent; unspent ⇒ credited. Two-sided: an ordinary held payment
/// (never credited, so never spent by us) does not ask, and is credited even if the lookup would say spent.
#[tokio::test]
async fn p7b_w6_a_restored_credit_checks_the_output_is_unspent() {
    for (answer, from_backup, credited, held) in [("spent", true, false, false), ("error", true, false, true),
                                                  ("unspent", true, true, false), ("spent", false, true, false)] {
        let (db, path) = wallet();
        let (body, subject) = atomic_payment(&db, &SENDER_SK);
        rusqlite::Connection::open(&path).unwrap().execute(
            "INSERT INTO peerpay_held_messages (message_id, sender, body, reason, held_at, attempts, next_check_at) VALUES ('m-w6', ?1, ?2, ?3, 1, 1, 0)",
            rusqlite::params![sender_hex(&SENDER_SK), body, if from_backup { "restored" } else { "chain_check_error" }]).unwrap();
        let mut p = ScriptedProvider::new("arc_gorillapool");
        p.status_for = Box::new(|txid| Ok(status(txid, TxState::Mined, None)));
        p.outspend_for = Box::new(move |_, _| match answer {
            "spent" => Ok(crate::services::OutspendStatus::Spent { spending_txid: "ee".repeat(32), spending_vin: Some(0) }),
            "error" => Err(IndexerError::Transport("down".into())),
            _ => Ok(crate::services::OutspendStatus::Unspent),
        });
        let state = state_with(db, Arc::new(p));
        run_with(&state, &MockInbox::new(&path, vec![]), 1_000, 0).await.unwrap();
        let tag = format!("{} / from backup {}", answer, from_backup);
        assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}'", txid_hex(&subject))) == 1, credited, "{}", tag);
        assert_eq!(held_row(&path, "m-w6").is_some(), held, "{}", tag);
    }
}

/// `P7b-W10` (#10) — a restored row takes a body only from its own sender, and is then due at once.
#[tokio::test]
async fn p7b_w10_fill_only_from_the_same_sender_and_due_at_once() {
    let (_db, path) = wallet();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("INSERT INTO peerpay_held_messages (message_id, sender, reason, held_at, attempts) VALUES ('m', 'alice', 'restored', 1, 0)", []).unwrap();
    crate::database::PeerPayRepository::fill_held_body(&conn, "m", "mallory", b"forged").unwrap();
    assert_eq!(crate::database::PeerPayRepository::held_body(&conn, "m").unwrap(), None, "another sender filled it");
    crate::database::PeerPayRepository::fill_held_body(&conn, "m", "alice", b"real").unwrap();
    let r: (Option<Vec<u8>>, Option<i64>) = conn.query_row("SELECT body, next_check_at FROM peerpay_held_messages WHERE message_id='m'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(r, (Some(b"real".to_vec()), Some(0)));
}

/// `P7b-W12` (#12) — the alarm counts only reasons that should be on chain by now: a lawful lock or
/// a deliberate hold (amount mismatch) never raises "something is wrong".
#[tokio::test]
async fn p7b_w12_the_alarm_ignores_locks_and_deliberate_holds() {
    let (_db, path) = wallet();
    let conn = rusqlite::Connection::open(&path).unwrap();
    for (id, reason, attempts) in [("lock", "non_final", 99), ("mismatch", "amount_mismatch", 99), ("outage", "chain_check_error", 7),
                                   ("outage-new", "chain_check_error", 6), ("refused", "broadcast_refused", 23)] {
        crate::database::PeerPayRepository::save_held(&conn, &crate::database::HeldMessage {
            message_id: id.into(), sender: "s".into(), body: Some(b"b".to_vec()), derivation_prefix: Some("p".into()),
            derivation_suffix: Some("s".into()), txid: Some("ab".repeat(32)), reason: reason.into(), held_at: 0,
            lock_time: None, next_check_at: None, attempts,
        }).unwrap();
    }
    // review 164 #5: an hour in the CURRENT reason (7 backoff tries), not since first held; a refusal
    // is the network's verdict, never the alarm (owner 2026-10-12)
    assert_eq!(crate::database::PeerPayRepository::held_summary(&conn).unwrap(), (5, 1));
}

/// `P7b-W5` (#5, #13) — bodies in the backup: each ≤ 32 KiB, newest first up to 256 KiB in all; a
/// "headers unavailable" payment (not on chain) carries its body too.
#[test]
fn p7b_w5_backup_bodies_are_bounded() {
    let (db, path) = wallet();
    db.connection().execute("INSERT INTO wallets (mnemonic, created_at, updated_at) VALUES ('(encrypted)', 0, 0)", []).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let mut save = |id: &str, reason: &str, len: usize, held_at: i64| crate::database::PeerPayRepository::save_held(&conn, &crate::database::HeldMessage {
        message_id: id.into(), sender: sender_hex(&SENDER_SK), body: Some(vec![b'x'; len]), derivation_prefix: Some("p".into()),
        derivation_suffix: Some("s".into()), txid: Some("ab".repeat(32)), reason: reason.into(), held_at,
        lock_time: None, next_check_at: None, attempts: 1,
    }).unwrap();
    save("big", "non_final", 40_000, 1);
    save("headers", "headers_unavailable", 1_000, 2);
    for i in 0..10 { save(&format!("fill-{}", i), "chain_check_error", 30_000, 10 + i); }
    let payload = crate::backup::collect_payload(db.connection(), "test_identity_key", MNEMONIC).unwrap();
    let body_of = |id: &str| payload.peerpay_held.iter().find(|h| h.message_id == id).unwrap().body.is_some();
    assert!(!body_of("big"), "a body over 32 KiB was carried");
    let total: usize = payload.peerpay_held.iter().filter_map(|h| h.body.as_ref()).map(|b| b.len() * 3 / 4).sum();
    assert!(total <= 256 * 1024, "{} bytes of bodies", total);
    assert!(body_of("fill-9") && !body_of("fill-0"), "newest first");
    assert!(body_of("headers"), "a not-on-chain (headers unavailable) payment lost its body");
}


// ---------- /code-review xhigh 163 findings still open at 4961aa01 (2026-10-12), contract §5h ----------

/// `P7b-Y10` (#10) — an input spending an output its PRESENT parent does not have is invalid by
/// consensus: rejected, not held as a missing parent; an absent parent is still held (two-sided).
#[test]
fn p7b_y10_a_missing_parent_output_is_rejected_a_missing_parent_held() {
    let parent = spend([0xf5; 32], 0, &[(100_000_000, vec![0x51])]);
    let bad = spend(hash(&parent), 5, &[(700, p2pkh_of(&key()))]); // the parent has 1 output
    let r = resolve_brc29_credit(&atomic(&[parent], &bad), &key(), None, &mainnet());
    assert!(matches!(&r, Err(CreditReject::NoSuchParentOutput(_))), "{:?}", r.as_ref().err());
    assert_eq!(r.err().unwrap().decision(1), Decision::Reject);
    let (_, orphan) = payment(p2pkh_of(&key()));
    let r = resolve_brc29_credit(&atomic(&[], &orphan), &key(), None, &mainnet());
    assert!(matches!(&r, Err(CreditReject::Spv(crate::spv_checks::Refusal::MissingParent { .. }))));
}

/// `P7b-Y11` (#11) — a first outage after a run of other holds waits 60 s, not an hour.
#[tokio::test]
async fn p7b_y11_the_backoff_counts_its_own_run() {
    let (db, path) = wallet();
    let (body, _) = atomic_payment(&db, &SENDER_SK);
    rusqlite::Connection::open(&path).unwrap().execute(
        "INSERT INTO peerpay_held_messages (message_id, sender, body, reason, held_at, attempts, next_check_at) VALUES ('m-y11', ?1, ?2, 'headers_unavailable', 1, 15, 0)",
        rusqlite::params![sender_hex(&SENDER_SK), body]).unwrap();
    let state = state_with(db, outage());
    run_with(&state, &MockInbox::new(&path, vec![]), 9_000, 0).await.unwrap();
    let row = held_row(&path, "m-y11").unwrap();
    assert_eq!((row.reason.as_str(), row.attempts, row.next_check_at), ("chain_check_error", 1, Some(9_060)));
}

/// `P7b-Y2` (#2) — a row restored without its message is never deleted on a refusal of the BEEF our
/// own code built: here the chain copy pays a different key than the restored derivation names.
#[tokio::test]
async fn p7b_y2_a_restored_row_is_held_not_deleted_on_our_own_evidence() {
    let (db, path) = wallet();
    let (parent, subject) = payment(our_brc29_script(&db, &SENDER_SK, "other", "key"));
    {
        let repo = crate::database::ParentTransactionRepository::new(db.connection());
        repo.upsert(None, &txid_hex(&subject), &hex::encode(&subject)).unwrap();
        repo.upsert(None, &txid_hex(&parent), &hex::encode(&parent)).unwrap();
    }
    rusqlite::Connection::open(&path).unwrap().execute(
        "INSERT INTO peerpay_held_messages (message_id, sender, derivation_prefix, derivation_suffix, txid, reason, held_at, attempts, next_check_at)
         VALUES ('m-y2', ?1, 'pfx', 'sfx', ?2, 'restored', 1, 0, 0)",
        rusqlite::params![sender_hex(&SENDER_SK), txid_hex(&subject)]).unwrap();
    let state = state_with(db, mined());
    run_with(&state, &MockInbox::new(&path, vec![]), 1_000, 0).await.unwrap();
    let row = held_row(&path, "m-y2").expect("the only copy of the derivation was deleted");
    assert_eq!((row.reason.as_str(), row.derivation_prefix.as_deref()), ("restored", Some("pfx")));
    assert_eq!(notices(&path), 0);
}

/// `P7b-Y9` (#9) — a stored body still in its BRC-2 envelope is decrypted on re-check.
#[tokio::test]
async fn p7b_y9_a_stored_envelope_is_decrypted_again() {
    let (db, path) = wallet();
    let (plain, subject) = atomic_payment(&db, &SENDER_SK);
    let our_pub = crate::database::get_master_public_key_from_db(&db).unwrap();
    let sender_pub = hex::decode(sender_hex(&SENDER_SK)).unwrap();
    // the sender encrypts to us as MessageBox senders do (BRC-2, sender priv + our pub)
    let sender_client = crate::messagebox::MessageBoxClient::new(SENDER_SK.to_vec(), sender_pub.clone());
    let cipher = sender_client.encrypt_for_recipient(&our_pub, &plain).unwrap();
    use base64::Engine;
    let envelope = serde_json::json!({ "encryptedMessage": base64::engine::general_purpose::STANDARD.encode(cipher) }).to_string();
    rusqlite::Connection::open(&path).unwrap().execute(
        "INSERT INTO peerpay_held_messages (message_id, sender, body, reason, held_at, attempts) VALUES ('m-y9', ?1, ?2, 'missing_fields', 1, 1)",
        rusqlite::params![sender_hex(&SENDER_SK), envelope.into_bytes()]).unwrap();
    let state = state_with(db, mined());
    run_with(&state, &MockInbox::new(&path, vec![]), 1_000, i64::MAX).await.unwrap();
    assert!(held_row(&path, "m-y9").is_none(), "the envelope was not decrypted");
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}'", txid_hex(&subject))), 1);
}

/// `P7b-Y13` (#13) — when the held table cannot be read, the status route says so (500); it never
/// reports "nothing held".
#[actix_web::test]
async fn p7b_y13_an_unreadable_held_table_is_an_error_not_zero() {
    let (db, _path) = wallet();
    db.connection().execute_batch("DROP TABLE peerpay_held_messages;").unwrap();
    let state = state_with(db, mined());
    let (code, json) = crate::handlers::p2_test_support::body_json(crate::handlers::peerpay_status(state).await).await;
    assert_eq!(code, 200, "{}", json);
    assert!(json["held_count"].is_null(), "an unreadable held table must not read as 0: {}", json);
    assert_eq!(json["rejected_count"].as_i64(), Some(0), "the other counts still answer");
}


// ---------- final /code-review xhigh 164 findings + owner decisions (2026-10-12), contract §5i ----------

/// `P7b-Z2` (#2) — a verdict that does not rest on our own evidence stays final for a restored row:
/// a restored payment whose output the chain says is SPENT is rejected and removed (not held hourly).
#[tokio::test]
async fn p7b_z2_a_spent_restored_payment_is_final() {
    let (db, path) = wallet();
    let (parent, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx", "sfx"));
    {
        let repo = crate::database::ParentTransactionRepository::new(db.connection());
        repo.upsert(None, &txid_hex(&subject), &hex::encode(&subject)).unwrap();
        repo.upsert(None, &txid_hex(&parent), &hex::encode(&parent)).unwrap();
    }
    rusqlite::Connection::open(&path).unwrap().execute(
        "INSERT INTO peerpay_held_messages (message_id, sender, derivation_prefix, derivation_suffix, txid, reason, held_at, attempts, next_check_at)
         VALUES ('m-z2', ?1, 'pfx', 'sfx', ?2, 'restored', 1, 0, 0)",
        rusqlite::params![sender_hex(&SENDER_SK), txid_hex(&subject)]).unwrap();
    let mut p = ScriptedProvider::new("arc_gorillapool");
    p.status_for = Box::new(|txid| Ok(status(txid, TxState::Mined, None)));
    p.outspend_for = Box::new(|_, _| Ok(crate::services::OutspendStatus::Spent { spending_txid: "ee".repeat(32), spending_vin: Some(0) }));
    let state = state_with(db, Arc::new(p));
    run_with(&state, &MockInbox::new(&path, vec![]), 1_000, 0).await.unwrap();
    assert!(held_row(&path, "m-z2").is_none(), "a spent restored payment was held again");
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}'", txid_hex(&subject))), 0);
}

/// `P7b-Z4` (owner 2026-10-12) — a payment every broadcaster refuses is rejected after 24 hourly
/// tries, not before (two-sided).
#[tokio::test]
async fn p7b_z4_a_refused_payment_ends_after_a_day() {
    let (db, path) = wallet();
    let (body, _) = atomic_payment(&db, &SENDER_SK);
    rusqlite::Connection::open(&path).unwrap().execute(
        "INSERT INTO peerpay_held_messages (message_id, sender, body, reason, held_at, attempts, next_check_at) VALUES ('m-z4', ?1, ?2, 'broadcast_refused', 1, 23, 0)",
        rusqlite::params![sender_hex(&SENDER_SK), body]).unwrap();
    let mut p = ScriptedProvider::new("arc_gorillapool");
    p.status_for = Box::new(|txid| Ok(status(txid, TxState::Unknown, None)));
    p.broadcast = Box::new(|_| Err(IndexerError::ProviderStatus { provider: "arc_gorillapool", status: 400, body: "txn-mempool-conflict: double spend".into() }));
    let state = state_with(db, Arc::new(p));
    run_with(&state, &MockInbox::new(&path, vec![]), 1_000, 0).await.unwrap();
    assert_eq!(held_row(&path, "m-z4").map(|r| r.attempts), Some(24), "the 24th try must still hold it");
    run_with(&state, &MockInbox::new(&path, vec![]), 1_000 + 3_600, 0).await.unwrap();
    assert!(held_row(&path, "m-z4").is_none(), "still held after 24 hourly tries");
    assert_eq!(notices(&path), 1);
}

/// `P7b-Z5` (owner 2026-10-12) — a lock more than a year out is a lasting hold; a nearer one is not.
#[test]
fn p7b_z5_a_lock_beyond_a_year_is_lasting() {
    use crate::spv_checks::Refusal;
    let tip = 970_000;
    assert_eq!(CreditReject::Spv(Refusal::NonFinal { lock_time: tip + LOCK_HORIZON_BLOCKS + 1 }).decision(tip),
        Decision::Hold { reason: "non_final_far", wait: Wait::Lock(tip + LOCK_HORIZON_BLOCKS + 1) });
    assert_eq!(CreditReject::Spv(Refusal::NonFinal { lock_time: tip + LOCK_HORIZON_BLOCKS }).decision(tip),
        Decision::Hold { reason: "non_final", wait: Wait::Lock(tip + LOCK_HORIZON_BLOCKS) });
    assert!(is_lasting("non_final_far") && !is_lasting("non_final"));
    assert_eq!(CreditReject::Spv(Refusal::NonFinal { lock_time: u32::MAX }).decision(tip),
        Decision::Hold { reason: "non_final_far", wait: Wait::Lock(u32::MAX) });
}

/// `P7b-Z7` (#7) — rows the backup does not carry no longer spend the body budget.
#[test]
fn p7b_z7_dropped_rows_spend_no_body_budget() {
    let (db, path) = wallet();
    db.connection().execute("INSERT INTO wallets (mnemonic, created_at, updated_at) VALUES ('(encrypted)', 0, 0)", []).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let save = |id: &str, prefix: String, held_at: i64| crate::database::PeerPayRepository::save_held(&conn, &crate::database::HeldMessage {
        message_id: id.into(), sender: sender_hex(&SENDER_SK), body: Some(vec![b'x'; 32_000]), derivation_prefix: Some(prefix),
        derivation_suffix: Some("s".into()), txid: Some("ab".repeat(32)), reason: "non_final".into(), held_at,
        lock_time: None, next_check_at: None, attempts: 1,
    }).unwrap();
    save("honest", "p".into(), 1);
    for i in 0..8 { save(&format!("junk-{}", i), "x".repeat(200), 10 + i); } // newer, dropped (prefix > 128)
    let payload = crate::backup::collect_payload(db.connection(), "test_identity_key", MNEMONIC).unwrap();
    let honest = payload.peerpay_held.iter().find(|h| h.message_id == "honest").expect("honest row not carried");
    assert!(honest.body.is_some(), "dropped junk spent the honest row's body budget");
}

/// `P7b-Z15` (#15) — on import, a held payment is skipped only when ITS OWN output (this derivation)
/// is already credited — another payment in the same transaction does not hide it.
#[test]
fn p7b_z15_import_matches_the_derivation_not_just_the_txid() {
    let (src, _) = wallet();
    src.connection().execute("INSERT INTO wallets (mnemonic, created_at, updated_at) VALUES ('(encrypted)', 0, 0)", []).unwrap();
    let mut payload = crate::backup::collect_payload(src.connection(), "test_identity_key", MNEMONIC).unwrap();
    payload.peerpay_held = vec![crate::backup::BackupHeldPayment {
        message_id: "second".into(), sender: sender_hex(&SENDER_SK), derivation_prefix: "p2".into(), derivation_suffix: "s2".into(),
        txid: "cd".repeat(32), held_at: 1, body: None }];
    let fresh_path = std::env::temp_dir().join(format!("hodos_p7b_{}.db", uuid::Uuid::new_v4()));
    let fresh = crate::database::WalletDatabase::new(fresh_path.clone()).unwrap();
    fresh.connection().execute("INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at) VALUES (99, 'x', 'local', 0, 0)", []).unwrap();
    fresh.connection().execute(
        "INSERT INTO outputs (user_id, txid, vout, satoshis, locking_script, spendable, change, derivation_suffix, created_at, updated_at)
         VALUES (99, ?1, 0, 700, x'00', 1, 0, 'p1 s1', 0, 0)", ["cd".repeat(32)]).unwrap();
    crate::backup::import_to_db(fresh.connection(), &payload).unwrap();
    assert!(held_row(&fresh_path, "second").is_some(), "the first payment's output hid the second payment");
}


// ---------- 5th xhigh review of #164 (2026-10-12), contract §5j ----------

/// `P7b-Q1` (#1) — a V30 table created without `checked_at` gets it at the next open.
#[test]
fn p7b_q1_a_held_table_without_checked_at_is_repaired() {
    let (db, path) = wallet();
    db.connection().execute_batch(
        "DROP TABLE peerpay_held_messages;
         CREATE TABLE peerpay_held_messages (message_id TEXT PRIMARY KEY, sender TEXT NOT NULL, body BLOB, derivation_prefix TEXT,
           derivation_suffix TEXT, txid TEXT, reason TEXT NOT NULL, held_at INTEGER NOT NULL, lock_time INTEGER, next_check_at INTEGER,
           attempts INTEGER NOT NULL DEFAULT 0);").unwrap();
    drop(db);
    let _reopened = crate::database::WalletDatabase::new(path.clone()).unwrap();
    assert_eq!(count(&path, "SELECT COUNT(*) FROM pragma_table_info('peerpay_held_messages') WHERE name = 'checked_at'"), 1);
}

/// `P7b-Q6` (#6) — flapping between reasons that share a schedule keeps one run: chain-check error
/// then broadcast failure is the 2nd try (120 s), not a fresh 1st.
#[tokio::test]
async fn p7b_q6_reasons_that_share_a_schedule_share_a_run() {
    let (db, path) = wallet();
    let (body, _) = atomic_payment(&db, &SENDER_SK);
    rusqlite::Connection::open(&path).unwrap().execute(
        "INSERT INTO peerpay_held_messages (message_id, sender, body, reason, held_at, attempts, next_check_at) VALUES ('m-q6', ?1, ?2, 'chain_check_error', 1, 1, 0)",
        rusqlite::params![sender_hex(&SENDER_SK), body]).unwrap();
    let mut p = ScriptedProvider::new("arc_gorillapool");
    p.status_for = Box::new(|txid| Ok(status(txid, TxState::Unknown, None)));
    p.broadcast = Box::new(|_| Err(IndexerError::Transport("reset".into())));
    let state = state_with(db, Arc::new(p));
    run_with(&state, &MockInbox::new(&path, vec![]), 2_000, 0).await.unwrap();
    let row = held_row(&path, "m-q6").unwrap();
    assert_eq!((row.reason.as_str(), row.attempts, row.next_check_at), ("broadcast_failed", 2, Some(2_120)));
}

/// `P7b-Q7` (#7) — a row restored from a backup (no body: its BEEF is ours) is never deleted by the
/// 24-hour refusal rule, even after 30 refused tries.
#[tokio::test]
async fn p7b_q7_a_restored_row_survives_the_refusal_rule() {
    let (db, path) = wallet();
    let (parent, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx", "sfx"));
    {
        let repo = crate::database::ParentTransactionRepository::new(db.connection());
        repo.upsert(None, &txid_hex(&subject), &hex::encode(&subject)).unwrap();
        repo.upsert(None, &txid_hex(&parent), &hex::encode(&parent)).unwrap();
    }
    rusqlite::Connection::open(&path).unwrap().execute(
        "INSERT INTO peerpay_held_messages (message_id, sender, derivation_prefix, derivation_suffix, txid, reason, held_at, attempts, next_check_at)
         VALUES ('m-q7', ?1, 'pfx', 'sfx', ?2, 'broadcast_refused', 1, 30, 0)",
        rusqlite::params![sender_hex(&SENDER_SK), txid_hex(&subject)]).unwrap();
    let mut p = ScriptedProvider::new("arc_gorillapool");
    p.status_for = Box::new(|txid| Ok(status(txid, TxState::Unknown, None)));
    p.broadcast = Box::new(|_| Err(IndexerError::ProviderStatus { provider: "arc_gorillapool", status: 400, body: "txn-mempool-conflict: double spend".into() }));
    let state = state_with(db, Arc::new(p));
    run_with(&state, &MockInbox::new(&path, vec![]), 1_000, 0).await.unwrap();
    let row = held_row(&path, "m-q7").expect("a restored row was deleted by the refusal rule");
    assert_eq!((row.reason.as_str(), row.attempts), ("broadcast_refused", 31), "the rule had to apply (31 > 24)");
}

/// `P7b-Q12` (#12) — a message just held from the inbox for a start-wait reason is not decided
/// again at the next tick.
#[tokio::test]
async fn p7b_q12_a_fresh_hold_is_not_redecided_next_tick() {
    let (db, path) = wallet();
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![msg("m-q12", &sender_hex(&sk(0x71)), b"junk".to_vec())]);
    run_with(&state, &inbox, 1_000, 999).await.unwrap(); // this process started at 999
    let first = held_row(&path, "m-q12").unwrap();
    run_with(&state, &inbox, 1_060, 999).await.unwrap();
    assert_eq!(held_row(&path, "m-q12").unwrap().attempts, first.attempts, "re-decided one tick after it was stored");
}

/// `P7b-Q3` (#3) — a far-locked payment (not on chain) is carried in the backup with its body.
#[test]
fn p7b_q3_a_far_lock_is_carried_with_its_body() {
    let (db, path) = wallet();
    db.connection().execute("INSERT INTO wallets (mnemonic, created_at, updated_at) VALUES ('(encrypted)', 0, 0)", []).unwrap();
    crate::database::PeerPayRepository::save_held(&rusqlite::Connection::open(&path).unwrap(), &crate::database::HeldMessage {
        message_id: "far".into(), sender: sender_hex(&SENDER_SK), body: Some(b"b".to_vec()), derivation_prefix: Some("p".into()),
        derivation_suffix: Some("s".into()), txid: Some("ab".repeat(32)), reason: "non_final_far".into(), held_at: 1,
        lock_time: Some(4_000_000_000), next_check_at: None, attempts: 1,
    }).unwrap();
    let payload = crate::backup::collect_payload(db.connection(), "test_identity_key", MNEMONIC).unwrap();
    assert!(payload.peerpay_held.iter().any(|h| h.message_id == "far" && h.body.is_some()));
}
