// B5-T1-P7b — the PeerPay poller end to end: `run_with` over a temp wallet, a fake inbox that
// records every acknowledge, and scripted chain providers. Rows: the phase contract §5
// (`development-docs/0.4.0-beta.7/track-1-money-path/phase-P7b-payments-kept-broadcasts-reach-operators/`).

use super::*;
use crate::handlers::p2_test_support::{app_state_from_with_tracker, status, ScriptedProvider};
use crate::services::{BroadcastResult, IndexerError, IndexerProvider, TxState, WalletServices};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

pub(crate) const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
/// The paying sender's identity private key.
pub(crate) const SENDER_SK: [u8; 32] = [0x22; 32];

pub(crate) fn sender_hex(sk: &[u8; 32]) -> String {
    let secp = secp256k1::Secp256k1::new();
    hex::encode(secp256k1::PublicKey::from_secret_key(&secp, &secp256k1::SecretKey::from_slice(sk).unwrap()).serialize())
}

/// A temp wallet with a user row and the test mnemonic unlocked. Returns it and its file path.
pub(crate) fn wallet() -> (crate::database::WalletDatabase, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("hodos_p7b_{}.db", uuid::Uuid::new_v4()));
    let mut db = crate::database::WalletDatabase::new(path.clone()).unwrap();
    db.connection().execute(
        "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at)
         VALUES (1, 'test_identity_key', 'local', 0, 0)",
        [],
    ).unwrap();
    db.cache_mnemonic(MNEMONIC.to_string()).unwrap();
    (db, path)
}

/// The P2PKH script a BRC-29 sender with `sk` pays us for `prefix suffix` — derived sender-side,
/// independently of the receive code.
pub(crate) fn our_brc29_script(db: &crate::database::WalletDatabase, sk: &[u8; 32], prefix: &str, suffix: &str) -> Vec<u8> {
    let our_pub = crate::database::get_master_public_key_from_db(db).unwrap();
    let child = crate::crypto::brc42::derive_child_public_key(sk, &our_pub, &format!("2-3241645161d8-{} {}", prefix, suffix)).unwrap();
    let h = ripemd::Ripemd160::digest(Sha256::digest(&child));
    let mut s = vec![0x76, 0xa9, 0x14];
    s.extend(h.as_slice());
    s.extend([0x88, 0xac]);
    s
}

pub(crate) fn push_varint(v: &mut Vec<u8>, n: u64) {
    if n < 0xfd { v.push(n as u8); } else { v.push(0xfd); v.extend(&(n as u16).to_le_bytes()); }
}

/// Version 1, one input spending `prev` (natural-order hash) : `vout`, the outputs, locktime 0.
pub(crate) fn spend(prev: [u8; 32], vout: u32, outputs: &[(i64, Vec<u8>)]) -> Vec<u8> {
    let mut v = 1u32.to_le_bytes().to_vec();
    push_varint(&mut v, 1);
    v.extend(prev);
    v.extend(vout.to_le_bytes());
    push_varint(&mut v, 0);
    v.extend(0xffff_ffffu32.to_le_bytes());
    push_varint(&mut v, outputs.len() as u64);
    for (value, script) in outputs {
        v.extend(value.to_le_bytes());
        push_varint(&mut v, script.len() as u64);
        v.extend(script);
    }
    v.extend(0u32.to_le_bytes());
    v
}

pub(crate) fn hash(tx: &[u8]) -> [u8; 32] { Sha256::digest(Sha256::digest(tx)).into() }
pub(crate) fn txid_hex(tx: &[u8]) -> String { hex::encode(hash(tx).iter().rev().copied().collect::<Vec<u8>>()) }

/// A 1-BSV parent and a subject spending it that pays `ours` 700 sats at vout 1. (parent, subject)
pub(crate) fn payment(ours: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let parent = spend([0xf0; 32], 0, &[(100_000_000, vec![0x51])]);
    let subject = spend(hash(&parent), 0, &[(5, vec![0x51]), (700, ours)]);
    (parent, subject)
}

pub(crate) fn plain_v1(txs: &[Vec<u8>]) -> Vec<u8> {
    crate::beef::Beef { version: crate::beef::BEEF_V1_MARKER, bumps: vec![], transactions: txs.to_vec(), tx_to_bump: vec![None; txs.len()] }
        .to_v1_bytes().unwrap()
}

pub(crate) fn atomic(parents: &[Vec<u8>], subject: &[u8]) -> Vec<u8> {
    let mut beef = crate::beef::Beef::new();
    for p in parents { beef.add_parent_transaction(p.clone()); }
    beef.set_main_transaction(subject.to_vec());
    hex::decode(beef.to_atomic_beef_hex(&txid_hex(subject)).unwrap()).unwrap()
}

/// A PaymentToken body as BRC-29 senders write it.
pub(crate) fn token(prefix: &str, suffix: &str, tx: &[u8], amount: i64) -> Vec<u8> {
    use base64::Engine;
    serde_json::to_vec(&serde_json::json!({
        "customInstructions": { "derivationPrefix": prefix, "derivationSuffix": suffix },
        "transaction": base64::engine::general_purpose::STANDARD.encode(tx),
        "amount": amount,
    })).unwrap()
}

pub(crate) fn msg(id: &str, sender: &str, body: Vec<u8>) -> IncomingMessage {
    IncomingMessage { message_id: id.to_string(), body, sender: sender.to_string() }
}

/// The MessageBox server, as far as the poller can see it: a message stays listed until it is
/// acknowledged. Every acknowledge is recorded with a snapshot of the held table, read through a
/// SEPARATE connection, so a row counts as stored only once it is committed.
pub(crate) struct MockInbox {
    pub messages: StdMutex<Vec<IncomingMessage>>,
    pub acks: StdMutex<Vec<Vec<String>>>,
    pub held_at_ack: StdMutex<Vec<Vec<String>>>,
    pub fail_next_ack: AtomicBool,
    pub list_calls: AtomicUsize,
    pub db_path: std::path::PathBuf,
}

impl MockInbox {
    pub(crate) fn new(db_path: &std::path::Path, messages: Vec<IncomingMessage>) -> Self {
        MockInbox {
            messages: StdMutex::new(messages),
            acks: StdMutex::new(vec![]),
            held_at_ack: StdMutex::new(vec![]),
            fail_next_ack: AtomicBool::new(false),
            list_calls: AtomicUsize::new(0),
            db_path: db_path.to_path_buf(),
        }
    }
    pub(crate) fn acked(&self) -> Vec<String> { self.acks.lock().unwrap().concat() }
    pub(crate) fn listed(&self) -> Vec<String> { self.messages.lock().unwrap().iter().map(|m| m.message_id.clone()).collect() }
}

#[async_trait::async_trait]
impl PeerPayInbox for MockInbox {
    async fn list(&self) -> Result<Vec<IncomingMessage>, String> {
        self.list_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.messages.lock().unwrap().clone())
    }
    async fn acknowledge(&self, ids: &[String]) -> Result<(), String> {
        let conn = rusqlite::Connection::open(&self.db_path).unwrap();
        let held: Vec<String> = conn
            .prepare("SELECT message_id FROM peerpay_held_messages ORDER BY message_id")
            .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
            .expect("the held table must exist (V30): a missing one is not 'nothing held'");
        self.held_at_ack.lock().unwrap().push(held);
        self.acks.lock().unwrap().push(ids.to_vec());
        if self.fail_next_ack.swap(false, Ordering::SeqCst) {
            return Err("injected acknowledge failure".into());
        }
        self.messages.lock().unwrap().retain(|m| !ids.contains(&m.message_id));
        Ok(())
    }
}

/// A provider that records the exact bytes of every broadcast, answers `state` for every status
/// lookup, and accepts a broadcast naming the last transaction's txid.
pub(crate) fn recording(name: &'static str, state: TxState) -> (Arc<ScriptedProvider>, Arc<StdMutex<Vec<Vec<u8>>>>) {
    let sent: Arc<StdMutex<Vec<Vec<u8>>>> = Arc::new(StdMutex::new(vec![]));
    let mut p = ScriptedProvider::new(name);
    p.status_for = Box::new(move |txid| Ok(status(txid, state.clone(), None)));
    let s = sent.clone();
    p.broadcast = Box::new(move |bytes| {
        s.lock().unwrap().push(bytes.to_vec());
        let beef = crate::beef::Beef::from_bytes(bytes).map_err(IndexerError::Transport)?;
        let last = beef.transactions.last().cloned().unwrap_or_default();
        Ok(BroadcastResult { provider: "arc_gorillapool", txid: txid_hex(&last), tx_status: "SEEN_ON_NETWORK".into(), merkle_path_bump: None, block_height: None })
    });
    (Arc::new(p), sent)
}

pub(crate) fn state_with(db: crate::database::WalletDatabase, p: Arc<ScriptedProvider>) -> actix_web::web::Data<AppState> {
    let chain: Vec<Arc<dyn IndexerProvider>> = vec![p];
    app_state_from_with_tracker(db, WalletServices::with_test_chains_and_outspend(chain.clone(), chain.clone(), chain),
        crate::chain_tracker::ChainTracker::from_chain(crate::header_chain::mainnet_header_chain().unwrap()))
}

pub(crate) fn count(path: &std::path::Path, sql: &str) -> i64 {
    rusqlite::Connection::open(path).unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
}

/// `P7b-A3` ⭐ — the ticket's GREEN through the poller: a payment packed as PLAIN BEEF is credited
/// once, with the subject's txid, vout and sats; no rejected notice; acknowledged.
#[tokio::test]
async fn p7b_a3_plain_beef_payment_is_credited_once() {
    let (db, path) = wallet();
    let (parent, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx", "sfx"));
    let body = token("pfx", "sfx", &plain_v1(&[parent, subject.clone()]), 700);
    let (p, _) = recording("arc_gorillapool", TxState::Mined);
    let state = state_with(db, p.clone());
    let inbox = MockInbox::new(&path, vec![msg("m-plain", &sender_hex(&SENDER_SK), body)]);

    run_with(&state, &inbox, 1_000, 0).await.unwrap();

    let txid = txid_hex(&subject);
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}' AND vout=1 AND satoshis=700", txid)), 1,
        "plain-BEEF payment not credited");
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_received WHERE message_id='m-plain'"), 1);
    assert_eq!(count(&path, "SELECT COUNT(*) FROM peerpay_received WHERE notification_type='rejected'"), 0);
    assert_eq!(inbox.acked(), vec!["m-plain".to_string()]);
    assert_eq!(p.status_calls.load(Ordering::SeqCst), 1, "the plain message reached the chain check once");
}

/// `P7b-B2` ⭐ — PeerPay broadcasts the sender's BEEF: the bytes that leave the wallet parse as a
/// BEEF (no Atomic header) whose LAST transaction is the subject AND which carries its parent.
#[tokio::test]
async fn p7b_b2_peerpay_broadcasts_the_senders_beef_with_parents() {
    let (db, path) = wallet();
    let (parent, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx", "sfx"));
    let body = token("pfx", "sfx", &atomic(&[parent.clone()], &subject), 700);
    let (p, sent) = recording("arc_gorillapool", TxState::Unknown); // not on chain ⇒ broadcast
    let state = state_with(db, p);
    let inbox = MockInbox::new(&path, vec![msg("m-b2", &sender_hex(&SENDER_SK), body)]);

    run_with(&state, &inbox, 1_000, 0).await.unwrap();

    let sent = sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1, "exactly one broadcast");
    assert_ne!(&sent[0][..4], &[1u8, 1, 1, 1], "Atomic header sent to ARC");
    let beef = crate::beef::Beef::from_bytes(&sent[0]).unwrap();
    assert_eq!(beef.transactions.last(), Some(&subject), "subject is not the last transaction");
    assert!(beef.transactions.contains(&parent), "the parent the sender supplied was not sent");
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}'", txid_hex(&subject))), 1);
}

/// `P7b-A27` control — the Atomic path credits exactly what it credited before this phase. Every
/// expected value is computed here (txid, script) or fixed by the fixture, never read back from the
/// code under test. Green on the seam commit (before the behaviour change) and after it.
#[tokio::test]
async fn p7b_a27_atomic_payment_is_credited_as_before() {
    let (db, path) = wallet();
    let script = our_brc29_script(&db, &SENDER_SK, "pfx", "sfx");
    let (parent, subject) = payment(script.clone());
    let body = token("pfx", "sfx", &atomic(&[parent], &subject), 700);
    let (p, _) = recording("arc_gorillapool", TxState::Mined);
    let state = state_with(db, p);
    let inbox = MockInbox::new(&path, vec![msg("m-atomic", &sender_hex(&SENDER_SK), body)]);

    run_with(&state, &inbox, 1_000, 0).await.unwrap();

    let conn = rusqlite::Connection::open(&path).unwrap();
    let row: (i64, i64, Vec<u8>, String, String, String, i64) = conn.query_row(
        "SELECT vout, satoshis, locking_script, sender_identity_key, derivation_prefix, derivation_suffix, spendable
         FROM outputs WHERE txid = ?1", [txid_hex(&subject)],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))).unwrap();
    // Stored as (protocol id, keyID) — the convention every receive path writes.
    assert_eq!(row, (1, 700, script, sender_hex(&SENDER_SK), "2-3241645161d8".into(), "pfx sfx".into(), 1));
    let received: (String, i64, String, String, Option<String>) = conn.query_row(
        "SELECT sender_identity_key, amount_satoshis, derivation_prefix, derivation_suffix, txid FROM peerpay_received WHERE message_id = 'm-atomic'",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).unwrap();
    assert_eq!(received, (sender_hex(&SENDER_SK), 700, "pfx".into(), "sfx".into(), Some(txid_hex(&subject))));
    assert_eq!(inbox.acked(), vec!["m-atomic".to_string()]);
}

/// `P7b-B12` (K-1: trim) — an Atomic envelope whose header names S, with a transaction X appended
/// AFTER S that spends S:0. ARC answers for the last transaction, so the bytes PeerPay broadcasts
/// must end with S and leave X out; the payment is then credited.
#[tokio::test]
async fn p7b_b12_the_broadcast_bytes_end_with_the_subject() {
    let (db, path) = wallet();
    let (parent, subject) = payment(our_brc29_script(&db, &SENDER_SK, "pfx", "sfx"));
    let x = spend(hash(&subject), 0, &[(1, vec![0x51])]);
    let bundle = crate::beef::Beef { version: crate::beef::BEEF_V1_MARKER, bumps: vec![], transactions: vec![parent.clone(), subject.clone(), x.clone()], tx_to_bump: vec![None; 3] };
    let env = hex::decode(bundle.to_atomic_beef_hex(&txid_hex(&subject)).unwrap()).unwrap();
    let (p, sent) = recording("arc_gorillapool", TxState::Unknown);
    let state = state_with(db, p);
    let inbox = MockInbox::new(&path, vec![msg("m-b12", &sender_hex(&SENDER_SK), token("pfx", "sfx", &env, 700))]);

    run_with(&state, &inbox, 1_000, 0).await.unwrap();

    let sent = sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    let beef = crate::beef::Beef::from_bytes(&sent[0]).unwrap();
    assert_eq!(beef.transactions, vec![parent, subject.clone()], "X must be left out and S be last");
    assert_eq!(count(&path, &format!("SELECT COUNT(*) FROM outputs WHERE txid='{}' AND vout=1 AND satoshis=700", txid_hex(&subject))), 1);
}
