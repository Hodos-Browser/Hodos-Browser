// B5-T5-P7a — the poller reads every MessageBox host, and one payment is credited once however many
// copies of it arrive (contract `T5P7a-A7`…`A9`). Database read through SELECTs; each host is a
// `MockInbox` that keeps a message listed until it is acknowledged ON THAT HOST.

use super::p7b_poller_tests::*;
use super::*;
use crate::handlers::p2_test_support::{status, ScriptedProvider};
use crate::services::{IndexerError, TxState};
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn mined() -> Arc<ScriptedProvider> {
    let mut p = ScriptedProvider::new("arc_gorillapool");
    p.status_for = Box::new(|txid| Ok(status(txid, TxState::Mined, None)));
    Arc::new(p)
}

/// Every status lookup fails as an outage would.
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

/// A valid payment from `SENDER_SK` (700 sats at vout 1) and its subject transaction.
fn payment_body(db: &crate::database::WalletDatabase) -> (Vec<u8>, Vec<u8>) {
    let (parent, subject) = payment(our_brc29_script(db, &SENDER_SK, "pfx", "sfx"));
    (token("pfx", "sfx", &atomic(&[parent], &subject), 700), subject)
}

fn received(path: &std::path::Path) -> i64 {
    count(path, "SELECT COUNT(*) FROM peerpay_received WHERE notification_type='receive'")
}

fn rejected(path: &std::path::Path) -> i64 {
    count(path, "SELECT COUNT(*) FROM peerpay_received WHERE notification_type='rejected'")
}

fn outputs_of(path: &std::path::Path, subject: &[u8]) -> i64 {
    count(path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}'", txid_hex(subject)))
}

/// A host whose inbox cannot be read.
struct DownInbox;

#[async_trait::async_trait]
impl PeerPayInbox for DownInbox {
    async fn list(&self) -> Result<Vec<IncomingMessage>, String> { Err("host down".into()) }
    async fn acknowledge(&self, _: &[String]) -> Result<(), String> { panic!("nothing was listed here") }
}

/// `T5P7a-A7` ⭐ (mandatory, G5 order note 2026-10-14) — the same payment arriving on two servers in
/// two messages with DIFFERENT ids (a sender that encrypted per server) is credited once and
/// notified once, and each copy is acknowledged on its own server. The message-id check cannot
/// catch it: the ids differ.
#[tokio::test]
async fn t5p7a_a7_one_payment_on_two_servers_is_credited_once() {
    let (db, path) = wallet();
    let (body, subject) = payment_body(&db);
    let state = state_with(db, mined());
    let babbage = MockInbox::new(&path, vec![msg("m-on-babbage", &sender_hex(&SENDER_SK), body.clone())]);
    let bsvb = MockInbox::new(&path, vec![msg("m-on-bsvb", &sender_hex(&SENDER_SK), body)]);

    run_with_inboxes(&state, &[&babbage, &bsvb], 1_000, 0).await.unwrap();

    assert_eq!(outputs_of(&path, &subject), 1, "one coin");
    assert_eq!(received(&path), 1, "credited once, notified once");
    assert_eq!(rejected(&path), 0, "a duplicate is not a rejected payment");
    assert_eq!(babbage.acked(), vec!["m-on-babbage".to_string()]);
    assert_eq!(bsvb.acked(), vec!["m-on-bsvb".to_string()]);
    assert!(babbage.listed().is_empty() && bsvb.listed().is_empty(), "neither copy is listed again");
}

/// `T5P7a-A7` — the second copy arriving on a LATER poll (the first already credited and gone) is
/// still a duplicate.
#[tokio::test]
async fn t5p7a_a7_a_late_second_copy_is_still_a_duplicate() {
    let (db, path) = wallet();
    let (body, subject) = payment_body(&db);
    let state = state_with(db, mined());
    let babbage = MockInbox::new(&path, vec![msg("m-first", &sender_hex(&SENDER_SK), body.clone())]);
    run_with_inboxes(&state, &[&babbage], 1_000, 0).await.unwrap();
    assert_eq!(received(&path), 1);

    let bsvb = MockInbox::new(&path, vec![msg("m-later", &sender_hex(&SENDER_SK), body)]);
    run_with_inboxes(&state, &[&babbage, &bsvb], 1_060, 0).await.unwrap();

    assert_eq!((outputs_of(&path, &subject), received(&path), rejected(&path)), (1, 1, 0));
    assert_eq!(bsvb.acked(), vec!["m-later".to_string()]);
}

/// `T5P7a-A7` — copies that are both HELD (an outage) resolve to one credit when the outage ends, and
/// neither held row is left behind.
#[tokio::test]
async fn t5p7a_a7_two_held_copies_resolve_to_one_credit() {
    let (db, path) = wallet();
    let (body, subject) = payment_body(&db);
    let up = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let state = state_with(db, switchable(up.clone()));
    let babbage = MockInbox::new(&path, vec![msg("m-a", &sender_hex(&SENDER_SK), body.clone())]);
    let bsvb = MockInbox::new(&path, vec![msg("m-b", &sender_hex(&SENDER_SK), body)]);
    run_with_inboxes(&state, &[&babbage, &bsvb], 1_000, 0).await.unwrap();
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages"), 2, "both held in the outage");

    up.store(true, Ordering::SeqCst);
    run_with_inboxes(&state, &[&babbage, &bsvb], 1_060, 0).await.unwrap();

    assert_eq!((outputs_of(&path, &subject), received(&path), rejected(&path)), (1, 1, 0));
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages"), 0, "no held copy is left");
}

/// `T5P7a-A8` — the same message id on two servers (our own fallback posts one encrypted message to
/// every host) is credited once and acknowledged on both.
#[tokio::test]
async fn t5p7a_a8_one_message_id_on_two_servers_is_acknowledged_on_both() {
    let (db, path) = wallet();
    let (body, subject) = payment_body(&db);
    let state = state_with(db, mined());
    let babbage = MockInbox::new(&path, vec![msg("m-same", &sender_hex(&SENDER_SK), body.clone())]);
    let bsvb = MockInbox::new(&path, vec![msg("m-same", &sender_hex(&SENDER_SK), body)]);

    run_with_inboxes(&state, &[&babbage, &bsvb], 1_000, 0).await.unwrap();

    assert_eq!((outputs_of(&path, &subject), received(&path)), (1, 1));
    assert_eq!(babbage.acked(), vec!["m-same".to_string()]);
    assert_eq!(bsvb.acked(), vec!["m-same".to_string()]);
}

/// `T5P7a-A9` — a server that cannot be read does not stop the others: a payment waiting on the
/// second host is credited when the first is down.
#[tokio::test]
async fn t5p7a_a9_one_host_down_does_not_stop_the_others() {
    let (db, path) = wallet();
    let (body, subject) = payment_body(&db);
    let state = state_with(db, mined());
    let bsvb = MockInbox::new(&path, vec![msg("m-bsvb", &sender_hex(&SENDER_SK), body)]);

    run_with_inboxes(&state, &[&DownInbox, &bsvb], 1_000, 0).await.unwrap();

    assert_eq!((outputs_of(&path, &subject), received(&path)), (1, 1));
    assert_eq!(bsvb.acked(), vec!["m-bsvb".to_string()]);
}

/// `T5P7a-A9` — the production poll reads every known host: one inbox per `MESSAGEBOX_HOSTS` entry.
#[test]
fn t5p7a_a9_every_known_host_is_read() {
    let client = MessageBoxClient::new(vec![1u8; 32], vec![2u8; 33]);
    let hosts: Vec<String> = host_inboxes(&client).iter().map(|i| i.host().to_string()).collect();
    let known: Vec<String> = crate::messagebox::MESSAGEBOX_HOSTS.iter().map(|h| h.to_string()).collect();
    assert_eq!(hosts, known);
    assert!(known.contains(&"https://messagebox.babbage.systems".to_string()));
    assert!(known.contains(&"https://messagebox.bsvblockchain.tech".to_string()));
}

/// `T5P7a-A8` — a message id already acknowledged as a duplicate on one host this tick is
/// acknowledged on the next host too, in the same tick (not left listed for a tick).
#[tokio::test]
async fn t5p7a_a8_a_duplicate_id_is_acknowledged_on_every_host_in_one_tick() {
    let (db, path) = wallet();
    let (body, subject) = payment_body(&db);
    let state = state_with(db, mined());
    let first = MockInbox::new(&path, vec![msg("m-first", &sender_hex(&SENDER_SK), body.clone())]);
    run_with_inboxes(&state, &[&first], 1_000, 0).await.unwrap();

    // The same payment re-sent under a new id, posted to both servers.
    let babbage = MockInbox::new(&path, vec![msg("m-again", &sender_hex(&SENDER_SK), body.clone())]);
    let bsvb = MockInbox::new(&path, vec![msg("m-again", &sender_hex(&SENDER_SK), body)]);
    run_with_inboxes(&state, &[&babbage, &bsvb], 1_060, 0).await.unwrap();

    assert_eq!((outputs_of(&path, &subject), received(&path), rejected(&path)), (1, 1, 0));
    assert_eq!(babbage.acked(), vec!["m-again".to_string()]);
    assert_eq!(bsvb.acked(), vec!["m-again".to_string()], "the second host's copy must go in the same tick");
}

/// `T5P7a-A8` — a message decided on one host this tick and left in its inbox (a transient hold
/// over the sender's limit) is not decided again from the next host in the same tick.
#[tokio::test]
async fn t5p7a_a8_left_in_the_inbox_is_not_decided_twice_in_one_tick() {
    let (db, path) = wallet();
    let (body, _) = payment_body(&db);
    let p = outage();
    let state = state_with(db, p.clone());
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        for i in 0..HELD_PER_SENDER_MAX {
            crate::database::PeerPayRepository::save_held(&conn, &crate::database::HeldMessage {
                message_id: format!("seed-{}", i), sender: sender_hex(&SENDER_SK), body: Some(b"x".to_vec()),
                derivation_prefix: None, derivation_suffix: None, txid: None, reason: "unreadable".into(),
                held_at: 1, lock_time: None, next_check_at: Some(i64::MAX), attempts: 1,
            }).unwrap();
        }
    }
    let babbage = MockInbox::new(&path, vec![msg("m-x", &sender_hex(&SENDER_SK), body.clone())]);
    let bsvb = MockInbox::new(&path, vec![msg("m-x", &sender_hex(&SENDER_SK), body)]);

    run_with_inboxes(&state, &[&babbage, &bsvb], 1_000, 0).await.unwrap();

    assert_eq!(p.status_calls.load(Ordering::SeqCst), 1, "decided once, not once per host");
    assert_eq!((babbage.listed().len(), bsvb.listed().len()), (1, 1), "left in both inboxes for the next tick");
}

/// `T5P7a-A7` — "already stored" means the same coin: same txid:vout, value AND script. A stored
/// row that differs is not a duplicate (the store refuses it, as before).
#[test]
fn t5p7a_a7_already_stored_means_same_value_and_script() {
    let (db, _path) = wallet();
    let script = our_brc29_script(&db, &SENDER_SK, "pfx", "sfx");
    let txid = "ab".repeat(32);
    let ci = serde_json::json!({"type":"brc29_payment","senderIdentityKey":sender_hex(&SENDER_SK),"derivationPrefix":"pfx","derivationSuffix":"sfx"});
    let conn = db.connection();
    assert!(!crate::handlers::derived_output_already_stored(conn, &txid, 1, 700, &script).unwrap(), "absent");
    crate::handlers::store_derived_utxo(&db, &txid, 1, 700, &hex::encode(&script), &[], &ci, &[]).unwrap();
    assert!(crate::handlers::derived_output_already_stored(conn, &txid, 1, 700, &script).unwrap(), "same coin");
    assert!(!crate::handlers::derived_output_already_stored(conn, &txid, 1, 701, &script).unwrap(), "other value");
    let mut other = script.clone();
    other[5] ^= 1;
    assert!(!crate::handlers::derived_output_already_stored(conn, &txid, 1, 700, &other).unwrap(), "other script");
    assert!(!crate::handlers::derived_output_already_stored(conn, &txid, 0, 700, &script).unwrap(), "other vout");
}

/// `T5P7a-A12` (review 178 #3) — one id, two servers, two bodies: the server we read first served
/// junk (held), the other serves the real payment under the same id. The real copy is decided and
/// credited, not acknowledged away as "already held".
#[tokio::test]
async fn t5p7a_a12_a_real_copy_under_a_held_junk_id_is_credited() {
    let (db, path) = wallet();
    let (body, subject) = payment_body(&db);
    let state = state_with(db, mined());
    let junk_server = MockInbox::new(&path, vec![msg("m-x", &sender_hex(&SENDER_SK), b"\xffjunk".to_vec())]);
    run_with_inboxes(&state, &[&junk_server], 1_000, 0).await.unwrap();
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages WHERE message_id='m-x'"), 1, "junk held");

    let real_server = MockInbox::new(&path, vec![msg("m-x", &sender_hex(&SENDER_SK), body)]);
    run_with_inboxes(&state, &[&real_server], 1_060, 0).await.unwrap();

    assert_eq!((outputs_of(&path, &subject), received(&path)), (1, 1), "the real payment must be credited");
    assert_eq!(real_server.acked(), vec!["m-x".to_string()]);
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages WHERE message_id='m-x'"), 0, "the junk row goes with the credit");
}

/// `T5P7a-A12` — the reverse: we hold the real payment (an outage), another server serves junk under
/// its id. The junk is not acknowledged, and the held real payment is untouched.
#[tokio::test]
async fn t5p7a_a12_junk_under_a_held_real_id_is_left_and_changes_nothing() {
    let (db, path) = wallet();
    let (body, _) = payment_body(&db);
    let state = state_with(db, outage());
    let real_server = MockInbox::new(&path, vec![msg("m-y", &sender_hex(&SENDER_SK), body.clone())]);
    run_with_inboxes(&state, &[&real_server], 1_000, 0).await.unwrap();
    let held_before = count(&path, "SELECT COUNT(*) FROM peerpay_held_messages WHERE message_id='m-y' AND derivation_prefix='pfx'");
    assert_eq!(held_before, 1);

    let junk_server = MockInbox::new(&path, vec![msg("m-y", &sender_hex(&SENDER_SK), b"\xffjunk".to_vec())]);
    run_with_inboxes(&state, &[&junk_server], 1_001, 0).await.unwrap();

    assert!(junk_server.acked().is_empty(), "a differing copy that does not credit stays on its server");
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages WHERE message_id='m-y' AND derivation_prefix='pfx'"), 1,
        "the held real payment is untouched");
}

/// A server whose list answers only once another server's list has started.
struct WaitsForOther(Arc<tokio::sync::Notify>, MockInbox);
struct SignalsOther(Arc<tokio::sync::Notify>);

#[async_trait::async_trait]
impl PeerPayInbox for WaitsForOther {
    async fn list(&self) -> Result<Vec<IncomingMessage>, String> {
        self.0.notified().await;
        self.1.list().await
    }
    async fn acknowledge(&self, ids: &[String]) -> Result<(), String> { self.1.acknowledge(ids).await }
}

#[async_trait::async_trait]
impl PeerPayInbox for SignalsOther {
    async fn list(&self) -> Result<Vec<IncomingMessage>, String> {
        self.0.notify_one();
        Ok(vec![])
    }
    async fn acknowledge(&self, _: &[String]) -> Result<(), String> { Ok(()) }
}

/// `T5P7a-A13` (review 178 #7) — hosts are listed at once: the first host's answer comes only after
/// the second host's list has started. Listed one after another, the first would wait out its whole
/// budget and its payment would not be credited this poll.
#[tokio::test]
async fn t5p7a_a13_hosts_are_listed_at_once() {
    let (db, path) = wallet();
    let (body, subject) = payment_body(&db);
    let state = state_with(db, mined());
    let n = Arc::new(tokio::sync::Notify::new());
    let first = WaitsForOther(n.clone(), MockInbox::new(&path, vec![msg("m-1", &sender_hex(&SENDER_SK), body)]));
    let second = SignalsOther(n);
    let t0 = std::time::Instant::now();

    run_with_inboxes(&state, &[&first, &second], 1_000, 0).await.unwrap();

    assert!(t0.elapsed() < std::time::Duration::from_secs(10), "took {:?}", t0.elapsed());
    assert_eq!((outputs_of(&path, &subject), received(&path)), (1, 1));
}

/// `T5P7a-A16` (review 178-2 #1) — one id, two servers, two bodies, where the first copy is NOT held:
/// server 1 replays an already-credited payment under id X (a duplicate, acknowledged), server 2 lists
/// the real payment under the same id X. The real one is decided and credited — never acknowledged
/// away because "X was just acknowledged".
#[tokio::test]
async fn t5p7a_a16_a_real_copy_is_not_acked_away_by_another_bodys_shortcut() {
    let (db, path) = wallet();
    let (old_body, _) = payment_body(&db);
    let (parent, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx2", "sfx2"));
    let new_body = token("pfx2", "sfx2", &atomic(&[parent], &subject), 700);
    let state = state_with(db, mined());
    let first = MockInbox::new(&path, vec![msg("m-old", &sender_hex(&SENDER_SK), old_body.clone())]);
    run_with_inboxes(&state, &[&first], 1_000, 0).await.unwrap();

    let replay = MockInbox::new(&path, vec![msg("m-x", &sender_hex(&SENDER_SK), old_body)]);
    let real = MockInbox::new(&path, vec![msg("m-x", &sender_hex(&SENDER_SK), new_body)]);
    run_with_inboxes(&state, &[&replay, &real], 1_060, 0).await.unwrap();

    assert_eq!(outputs_of(&path, &subject), 1, "the real payment under the replayed id must be credited");
}

/// `T5P7a-A16` (review 178-2 #2) — a differing copy of a HELD id does not stop the held payment's own
/// re-check: once the outage ends and the row is due, the held real payment is credited although the
/// junk copy is still listed every tick.
#[tokio::test]
async fn t5p7a_a16_a_differing_copy_does_not_starve_the_held_payment() {
    let (db, path) = wallet();
    let (body, subject) = payment_body(&db);
    let up = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let state = state_with(db, switchable(up.clone()));
    let real_server = MockInbox::new(&path, vec![msg("m-y", &sender_hex(&SENDER_SK), body)]);
    run_with_inboxes(&state, &[&real_server], 1_000, 0).await.unwrap();
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages WHERE message_id='m-y'"), 1);

    up.store(true, Ordering::SeqCst);
    let junk_server = MockInbox::new(&path, vec![msg("m-y", &sender_hex(&SENDER_SK), b"\xffjunk".to_vec())]);
    run_with_inboxes(&state, &[&junk_server], 10_000, 0).await.unwrap();

    assert_eq!((outputs_of(&path, &subject), received(&path)), (1, 1), "the held payment must be re-checked and credited");
    // Review 178-4 #9: this copy reached the contested decide (the row was due) and did not credit —
    // it is left on its server, never acknowledged.
    assert!(junk_server.acked().is_empty(), "a differing copy that does not credit is not acknowledged");
}

/// `T5P7a-A20` (review 178-3 #3) — a held row restored without its body does not take a listed body
/// that names another derivation (any server could list junk under its id): not filled, not
/// acknowledged, and the row keeps its restored derivation + txid.
#[tokio::test]
async fn t5p7a_a20_a_restored_row_does_not_take_a_foreign_body() {
    let (db, path) = wallet();
    let (_, subject) = payment_body(&db);
    rusqlite::Connection::open(&path).unwrap().execute(
        "INSERT INTO peerpay_held_messages (message_id, sender, derivation_prefix, derivation_suffix, txid, reason, held_at, attempts, next_check_at)
         VALUES ('m-r', ?1, 'pfx', 'sfx', ?2, 'restored', 1, 0, 999999)",
        [sender_hex(&SENDER_SK), txid_hex(&subject)]).unwrap();
    let state = state_with(db, mined());
    let junk = MockInbox::new(&path, vec![msg("m-r", &sender_hex(&SENDER_SK), b"\xffjunk".to_vec())]);
    run_with_inboxes(&state, &[&junk], 1_000, 0).await.unwrap();
    assert!(junk.acked().is_empty(), "a foreign body is not acknowledged as the restored payment");
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(crate::database::PeerPayRepository::held_body(&conn, "m-r").unwrap(), None, "not filled");
}

/// `T5P7a-A21` (review 178-3 #1) — a DIFFERENT real payment listed under a held payment's id is
/// credited, and the held payment is NOT removed: it is credited when its own re-check comes.
#[tokio::test]
async fn t5p7a_a21_a_different_payment_under_a_held_id_does_not_remove_it() {
    let (db, path) = wallet();
    let (body_a, subject_a) = payment_body(&db);
    let (parent, subject_b) = payment(our_brc29_script(&db, &SENDER_SK, "pfx2", "sfx2"));
    let body_b = token("pfx2", "sfx2", &atomic(&[parent], &subject_b), 700);
    let up = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let state = state_with(db, switchable(up.clone()));
    let first = MockInbox::new(&path, vec![msg("m-x", &sender_hex(&SENDER_SK), body_a)]);
    run_with_inboxes(&state, &[&first], 1_000, 0).await.unwrap();
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_held_messages WHERE message_id='m-x'"), 1, "A held");

    up.store(true, Ordering::SeqCst);
    let other = MockInbox::new(&path, vec![msg("m-x", &sender_hex(&SENDER_SK), body_b)]);
    run_with_inboxes(&state, &[&other], 10_000, 0).await.unwrap();

    assert_eq!(outputs_of(&path, &subject_b), 1, "B credited");
    assert_eq!(outputs_of(&path, &subject_a), 1, "A credited from its held row, not deleted by B");
    assert_eq!(received(&path), 2);
}

/// `T5P7a-A24` (review 178-3 #9) — a coin stored but never recorded (the process stopped between the
/// two) is not a duplicate: the next copy records the receive.
#[tokio::test]
async fn t5p7a_a24_stored_but_unrecorded_is_recorded_not_skipped() {
    let (db, path) = wallet();
    let (body, subject) = payment_body(&db);
    let state = state_with(db, mined());
    let inbox = MockInbox::new(&path, vec![msg("m-1", &sender_hex(&SENDER_SK), body.clone())]);
    run_with_inboxes(&state, &[&inbox], 1_000, 0).await.unwrap();
    rusqlite::Connection::open(&path).unwrap().execute("DELETE FROM peerpay_received", []).unwrap();

    let again = MockInbox::new(&path, vec![msg("m-2", &sender_hex(&SENDER_SK), body)]);
    run_with_inboxes(&state, &[&again], 1_060, 0).await.unwrap();
    assert_eq!((outputs_of(&path, &subject), received(&path)), (1, 1), "recorded once");
}

/// `T5P7a-A25` (review 178-3 #7) — a copy that differs from a held row that names a derivation is
/// decided no more often than that row: not before the row is due.
#[tokio::test]
async fn t5p7a_a25_a_contesting_copy_waits_for_the_held_rows_turn() {
    let (db, path) = wallet();
    let (body_a, _) = payment_body(&db);
    let (parent, subject_b) = payment(our_brc29_script(&db, &SENDER_SK, "pfx2", "sfx2"));
    let body_b = token("pfx2", "sfx2", &atomic(&[parent], &subject_b), 700);
    let p = outage();
    let state = state_with(db, p.clone());
    let first = MockInbox::new(&path, vec![msg("m-x", &sender_hex(&SENDER_SK), body_a)]);
    run_with_inboxes(&state, &[&first], 1_000, 0).await.unwrap();
    let calls = p.status_calls.load(Ordering::SeqCst);

    let other = MockInbox::new(&path, vec![msg("m-x", &sender_hex(&SENDER_SK), body_b)]);
    run_with_inboxes(&state, &[&other], 1_001, 0).await.unwrap();
    assert_eq!(p.status_calls.load(Ordering::SeqCst), calls, "not decided before the held row is due");
    assert!(other.acked().is_empty());
}

/// `T5P7a-A34` (review 178-6 #2) — a host that lists one id with two bodies: the acknowledged one (a
/// replayed, already-credited payment) does not make the other body — left in the inbox — count as
/// acknowledged on the next host.
#[tokio::test]
async fn t5p7a_a34_two_bodies_under_one_id_on_one_host_do_not_ack_the_other_elsewhere() {
    let (db, path) = wallet();
    let (old_body, _) = payment_body(&db);
    let (parent, subject_b) = payment(our_brc29_script(&db, &SENDER_SK, "pfx2", "sfx2"));
    let new_body = token("pfx2", "sfx2", &atomic(&[parent], &subject_b), 700);
    let up = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let state = state_with(db, switchable(up.clone()));
    let first = MockInbox::new(&path, vec![msg("m-old", &sender_hex(&SENDER_SK), old_body.clone())]);
    run_with_inboxes(&state, &[&first], 1_000, 0).await.unwrap();
    // The sender is at its held limit, so a transient hold of the new body stays in the inbox.
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        for i in 0..HELD_PER_SENDER_MAX {
            crate::database::PeerPayRepository::save_held(&conn, &crate::database::HeldMessage {
                message_id: format!("seed-{}", i), sender: sender_hex(&SENDER_SK), body: Some(b"x".to_vec()),
                derivation_prefix: None, derivation_suffix: None, txid: None, reason: "unreadable".into(),
                held_at: 1, lock_time: None, next_check_at: Some(i64::MAX), attempts: 1,
            }).unwrap();
        }
    }
    up.store(false, Ordering::SeqCst);
    let host1 = MockInbox::new(&path, vec![
        msg("m-x", &sender_hex(&SENDER_SK), old_body),
        msg("m-x", &sender_hex(&SENDER_SK), new_body.clone()),
    ]);
    let host2 = MockInbox::new(&path, vec![msg("m-x", &sender_hex(&SENDER_SK), new_body)]);
    run_with_inboxes(&state, &[&host1, &host2], 1_060, 0).await.unwrap();
    assert!(host2.acked().is_empty(), "the real payment on host 2 must not be acknowledged unseen");
    assert_eq!(outputs_of(&path, &subject_b), 0, "not credited yet (outage) — and still listed for the next poll");
}
