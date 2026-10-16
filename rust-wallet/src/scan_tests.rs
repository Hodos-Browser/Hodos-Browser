//! B5-T1-P6 T1 rows for `scan.rs` + `recovery.rs :: scan_brc42_self`. The subject of
//! every row is `find_late_payments` (or the scan/writer it is made of) — the function
//! all three production callers call — over a real temp wallet with the chain scripted.
//! The REDs are the second agent's designs in
//! `development-docs/0.4.0-beta.7/track-1-money-path/phase-P6-…/evidence/REDS_2026-10-07.md`;
//! each is a scratch mutation of the code, run and reverted (evidence in the contract).
use super::*;
use crate::classify::p5_tests::{change_of, our_p2pkh, p2pkh_for_key, p2pkh_plus_envelope, selected, wallet};
use crate::database::WalletDatabase;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

// ---------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------

fn varint(n: usize) -> Vec<u8> {
    match n {
        0..=0xfc => vec![n as u8],
        _ => [vec![0xfd], (n as u16).to_le_bytes().to_vec()].concat(),
    }
}

/// A one-input transaction paying `outputs`; `salt` makes the txid unique.
pub(crate) fn tx(salt: u8, outputs: &[(i64, Vec<u8>)]) -> Vec<u8> {
    let mut t = 1u32.to_le_bytes().to_vec();
    t.extend(varint(1));
    t.extend([salt; 32]);
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

pub(crate) fn txid_of(raw: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(Sha256::digest(raw)).iter().rev().copied().collect::<Vec<u8>>())
}

pub(crate) fn keys(db: &WalletDatabase) -> (Vec<u8>, Vec<u8>) {
    (crate::database::get_master_private_key_from_db(db).unwrap(),
     crate::database::get_master_public_key_from_db(db).unwrap())
}

pub(crate) fn receive_address(db: &WalletDatabase, index: i32) -> String {
    let (k, p) = keys(db);
    crate::reconcile::derive_receive_address(&k, &p, index).unwrap()
}

/// A scripted chain. Coins are listed by **address**; like production's fetcher, each
/// listed coin carries the script ASSUMED from the address it was asked about — the real
/// script is only in the raw transaction.
#[derive(Default)]
pub(crate) struct FakeChain {
    /// address → (txid as listed, vout, sats)
    pub listed: HashMap<String, Vec<(String, u32, i64)>>,
    /// lowercase txid → raw bytes
    pub raws: HashMap<String, Vec<u8>>,
    /// (lowercase txid, vout) → answer; absent ⇒ `Unknown` — what the real spent check
    /// answers for every unspent plain P2PKH (WoC 404 + GorillaPool 404, measured
    /// 2026-10-07). A fake that defaulted to `Unspent` hid that the writer never wrote.
    pub spent: HashMap<(String, u32), SpentStatus>,
    /// outpoints the confirming listing no longer contains (indexer lag the other way)
    pub delisted: HashSet<(String, u32)>,
    /// outpoints whose confirming listing errors
    pub listing_errors: HashSet<(String, u32)>,
    /// addresses with history
    pub history: HashSet<String>,
    /// batches whose first index fails outright
    pub fail_batch_from: HashSet<i32>,
    /// addresses the fetcher reports as unreadable
    pub failed_addresses: HashSet<String>,
    pub highest_index_asked: AtomicI64,
    /// every address has history (an indexer that lies, or a very busy wallet)
    pub history_everywhere: bool,
    /// address → how many more history calls answer 429 (WhatsOnChain's rate limit)
    pub history_429: Mutex<HashMap<String, u32>>,
    /// lowercase txid → how many more raw-tx fetches fail
    pub raw_failures: Mutex<HashMap<String, u32>>,
    /// while set and closed, every batch fetch waits — holds a scan open for a test
    pub gate: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub asked: Mutex<Vec<String>>,
    pub raw_asked: Mutex<Vec<String>>,
}

impl FakeChain {
    pub fn new() -> Self { Self { highest_index_asked: AtomicI64::new(-1), ..Default::default() } }

    /// Pay `script` with `sats` at `address` (vout 0 of a fresh tx), listed there.
    pub fn pay(&mut self, salt: u8, address: &str, sats: i64, script: Vec<u8>) -> String {
        let raw = tx(salt, &[(sats, script)]);
        let txid = txid_of(&raw);
        self.raws.insert(txid.clone(), raw);
        self.listed.entry(address.to_string()).or_default().push((txid.clone(), 0, sats));
        txid
    }
}

impl ScanChain for FakeChain {
    fn fetch_batch(&self, batch: Vec<AddressInfo>) -> BoxFut<Result<(Vec<UTXO>, Vec<String>), String>> {
        let first = batch.first().map(|a| a.index).unwrap_or(-1);
        let top = batch.iter().map(|a| a.index as i64).max().unwrap_or(-1);
        assert!(top < 6_000, "harness ceiling: the scan asked for index {} — it does not stop", top);
        self.highest_index_asked.fetch_max(top, Ordering::SeqCst);
        self.asked.lock().unwrap().extend(batch.iter().map(|a| a.address.clone()));
        let result = if self.fail_batch_from.contains(&first) {
            Err("indexer down".to_string())
        } else {
            let mut coins = Vec::new();
            let mut failed = Vec::new();
            for a in &batch {
                if self.failed_addresses.contains(&a.address) { failed.push(a.address.clone()); continue; }
                for (txid, vout, sats) in self.listed.get(&a.address).cloned().unwrap_or_default() {
                    coins.push(UTXO {
                        txid, vout, satoshis: sats,
                        script: hex::encode(crate::recovery::address_to_p2pkh_script(&a.address).unwrap()),
                        address_index: a.index, custom_instructions: None, confirmed: true,
                    });
                }
            }
            Ok((coins, failed))
        };
        let gate = self.gate.clone();
        Box::pin(async move {
            if let Some(open) = gate {
                while !open.load(Ordering::SeqCst) {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            }
            result
        })
    }
    fn has_history(&self, address: String) -> BoxFut<Result<bool, String>> {
        if let Some(n) = self.history_429.lock().unwrap().get_mut(&address).filter(|n| **n > 0) {
            *n -= 1;
            return Box::pin(async { Err("History API returned status 429 Too Many Requests".to_string()) });
        }
        let h = self.history_everywhere || self.history.contains(&address);
        Box::pin(async move { Ok(h) })
    }
    fn raw_tx(&self, txid: String) -> BoxFut<Result<Vec<u8>, String>> {
        self.raw_asked.lock().unwrap().push(txid.to_ascii_lowercase());
        if let Some(n) = self.raw_failures.lock().unwrap().get_mut(&txid.to_ascii_lowercase()).filter(|n| **n > 0) {
            *n -= 1;
            return Box::pin(async { Err("invalid response from junglebus".to_string()) });
        }
        let r = self.raws.get(&txid.to_ascii_lowercase()).cloned().ok_or_else(|| "not found".to_string());
        Box::pin(async move { r })
    }
    fn spent(&self, txid: String, vout: u32) -> BoxFut<SpentStatus> {
        let s = self.spent.get(&(txid.to_ascii_lowercase(), vout)).cloned().unwrap_or(SpentStatus::Unknown);
        Box::pin(async move { s })
    }
    fn unspent_listed(&self, address: String, txid: String, vout: u32) -> BoxFut<UnspentProbe> {
        let key = (txid.to_ascii_lowercase(), vout);
        let listed = self.listed.get(&address).map_or(false, |coins|
            coins.iter().any(|(t, v, _)| t.eq_ignore_ascii_case(&txid) && *v == vout));
        let p = if self.listing_errors.contains(&key) { UnspentProbe::Error }
                else if listed && !self.delisted.contains(&key) { UnspentProbe::Present }
                else { UnspentProbe::Absent };
        Box::pin(async move { p })
    }
}

fn wallet_id(db: &Mutex<WalletDatabase>) -> i64 {
    let d = db.lock().unwrap();
    crate::database::WalletRepository::new(d.connection()).get_primary_wallet().unwrap().unwrap().id.unwrap()
}

async fn run(db: &Mutex<WalletDatabase>, chain: &FakeChain) -> ScanOutcome {
    let (k, p) = { let d = db.lock().unwrap(); keys(&d) };
    let wid = wallet_id(db);
    find_late_payments(db, 1, wid, &k, &p, "I2:rescan", chain).await.unwrap()
}

/// `(outputId, derivation_prefix, derivation_suffix, purpose, spendable)` by outpoint,
/// case-insensitive; `None` if absent.
fn row(db: &Mutex<WalletDatabase>, txid: &str, vout: u32) -> Option<(i64, Option<String>, Option<String>, Option<String>, bool)> {
    use rusqlite::OptionalExtension;
    db.lock().unwrap().connection().query_row(
        "SELECT outputId, derivation_prefix, derivation_suffix, purpose, spendable FROM outputs
          WHERE lower(txid) = lower(?1) AND vout = ?2",
        rusqlite::params![txid, vout], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, i64>(4)? != 0)),
    ).optional().unwrap()
}

fn count(db: &Mutex<WalletDatabase>, txid: &str, vout: u32) -> i64 {
    db.lock().unwrap().connection().query_row(
        "SELECT COUNT(*) FROM outputs WHERE lower(txid) = lower(?1) AND vout = ?2",
        rusqlite::params![txid, vout], |r| r.get(0)).unwrap()
}

fn mdb(tag: &str) -> Mutex<WalletDatabase> { Mutex::new(wallet(tag)) }

fn indices(o: &ScanOutcome) -> Vec<u32> { o.result.addresses.iter().map(|a| a.index).collect() }

// ---------------------------------------------------------------------------------------
// P6-A5 — an error is not an answer (re-run of P4-A9's fixture through the new scan)
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn p6_a5_failed_batch_is_unchecked_and_the_coin_after_it_is_found() {
    let db = mdb("a5");
    let mut chain = FakeChain::new();
    let (a19, a45) = { let d = db.lock().unwrap(); (receive_address(&d, 19), receive_address(&d, 45)) };
    let (s19, s45) = { let d = db.lock().unwrap(); (our_p2pkh(&d, 19), our_p2pkh(&d, 45)) };
    chain.pay(19, &a19, 1_000, s19);
    chain.pay(45, &a45, 1_000, s45);
    chain.fail_batch_from.insert(20);
    let o = run(&db, &chain).await;
    assert_eq!(o.result.unchecked_ranges, vec![(20, 39)], "the failed batch is reported");
    assert!(!o.complete());
    assert_eq!(indices(&o), vec![19, 45], "the coin after the failed batch is found");
}

#[tokio::test]
async fn p6_a5_partial_outage_marks_only_the_failed_address() {
    let db = mdb("a5p");
    let mut chain = FakeChain::new();
    let a25 = { let d = db.lock().unwrap(); receive_address(&d, 25) };
    chain.failed_addresses.insert(a25);
    let o = run(&db, &chain).await;
    assert_eq!(o.result.unchecked_ranges, vec![(25, 25)]);
    assert!(!o.complete());
}

// ---------------------------------------------------------------------------------------
// P6-A6 — the bound is the high-water mark, from both sources
// ---------------------------------------------------------------------------------------

fn seed_counter_and_rows(db: &Mutex<WalletDatabase>, current_index: i32, max_row: i32) {
    let d = db.lock().unwrap();
    let wid = crate::database::WalletRepository::new(d.connection()).get_primary_wallet().unwrap().unwrap().id.unwrap();
    d.connection().execute("UPDATE wallets SET current_index = ?1 WHERE id = ?2", rusqlite::params![current_index, wid]).unwrap();
    if max_row > 0 {
        let address = receive_address(&d, max_row);
        crate::database::AddressRepository::new(d.connection()).create(&crate::database::Address {
            id: None, wallet_id: wid, index: max_row, address, public_key: String::new(),
            used: true, balance: 0, pending_utxo_check: false, created_at: 0,
        }).unwrap();
    }
}

/// F1 — rows ahead of the counter (today's on-chain restore leaves the counter at 0).
#[tokio::test]
async fn p6_a6_f1_address_rows_set_the_mark() {
    // No gap is counted through 132 (mark 112 + 20); 133..=182 are then the 50 unused
    // indices the gap allows. Two-sided: a coin at 182 is found, one at 183 is not.
    for (tag, beyond, found) in [("a6f1a", 182, vec![128, 182]), ("a6f1b", 183, vec![128])] {
        let db = mdb(tag);
        seed_counter_and_rows(&db, 5, 112);
        let mut chain = FakeChain::new();
        for (salt, i) in [(1u8, 128), (2, beyond)] {
            let (a, s) = { let d = db.lock().unwrap(); (receive_address(&d, i), our_p2pkh(&d, i)) };
            chain.pay(salt, &a, 1_000, s);
        }
        let o = run(&db, &chain).await;
        assert_eq!(o.high_water, 112);
        assert_eq!(indices(&o), found, "coins at 128 and {}", beyond);
    }
}

/// F2 — the counter ahead of the rows (restored counter, stripped rows).
#[tokio::test]
async fn p6_a6_f2_the_counter_sets_the_mark() {
    let db = mdb("a6f2");
    seed_counter_and_rows(&db, 130, 12);
    let mut chain = FakeChain::new();
    let (a, s) = { let d = db.lock().unwrap(); (receive_address(&d, 148), our_p2pkh(&d, 148)) };
    chain.pay(1, &a, 1_000, s);
    let o = run(&db, &chain).await;
    assert_eq!(o.high_water, 130);
    assert_eq!(indices(&o), vec![148]);
}

// ---------------------------------------------------------------------------------------
// P6-A7 — every scanned coin goes through the seam on its OBSERVED script (K8)
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn p6_a7_scan_classifies_on_the_observed_script() {
    let db = mdb("a7");
    let mut chain = FakeChain::new();
    let (a3, a4, a5, l3, l4, l5) = { let d = db.lock().unwrap();
        (receive_address(&d, 3), receive_address(&d, 4), receive_address(&d, 5),
         our_p2pkh(&d, 3), our_p2pkh(&d, 4), our_p2pkh(&d, 5)) };
    let ord = chain.pay(1, &a3, 1, p2pkh_plus_envelope(&l3));        // (a) 1-sat inscription
    let plain = chain.pay(2, &a4, 1_000, l4.clone());               // (b) plain coin
    let multi = chain.pay(3, &a5, 1_000, p2pkh_plus_envelope(&l5));  // (c) envelope on 1,000 sats
    let o = run(&db, &chain).await;
    assert_eq!(o.write.written.len(), 3, "pre-condition: all three rows exist ({:?})", o.write);

    let (id_a, ..) = row(&db, &ord, 0).unwrap();
    let (id_b, prefix_b, ..) = row(&db, &plain, 0).unwrap();
    let (id_c, ..) = row(&db, &multi, 0).unwrap();
    let d = db.lock().unwrap();
    let basket = |id: i64| -> Option<String> { d.connection().query_row(
        "SELECT b.name FROM outputs o LEFT JOIN output_baskets b ON o.basket_id = b.basketId WHERE o.outputId = ?1",
        [id], |r| r.get(0)).unwrap() };
    assert_eq!(basket(id_a).as_deref(), Some("1sat"), "(a) the 1-sat inscription is filed as a token");
    assert!(!change_of(&d, id_a) && !selected(&d, id_a));
    assert_eq!(prefix_b.as_deref(), Some("2-receive address"), "(b) recorded under the verified BRC-42 method");
    assert!(change_of(&d, id_b) && selected(&d, id_b), "(b) the plain coin is money — the seam ran");
    assert!(!change_of(&d, id_c) && !selected(&d, id_c), "(c) an inscription on 1,000 sats is never money");
    let held = crate::classify::list_held(&d, 1).unwrap();
    assert!(held.iter().any(|h| h.txid.as_deref() == Some(multi.as_str()) && h.reason == crate::classify::Reason::TokenShaped),
            "(c) held as token-shaped");
}

// ---------------------------------------------------------------------------------------
// P6-A8 — 22 coins at one address ⇒ 22 rows (layer 1)
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn p6_a8_twenty_two_coins_in_one_tx_are_twenty_two_rows() {
    let db = mdb("a8");
    let mut chain = FakeChain::new();
    let (a, s) = { let d = db.lock().unwrap(); (receive_address(&d, 4), our_p2pkh(&d, 4)) };
    let raw = tx(8, &(0..22).map(|_| (1_000i64, s.clone())).collect::<Vec<_>>());
    let txid = txid_of(&raw);
    chain.raws.insert(txid.clone(), raw);
    chain.listed.insert(a, (0..22).map(|v| (txid.clone(), v, 1_000)).collect());
    let o = run(&db, &chain).await;
    let n: i64 = db.lock().unwrap().connection().query_row(
        "SELECT COUNT(DISTINCT vout) FROM outputs WHERE lower(txid) = ?1", [&txid], |r| r.get(0)).unwrap();
    assert_eq!(n, 22, "missing vouts: {:?}", (0..22u32).filter(|v| row(&db, &txid, *v).is_none()).collect::<Vec<_>>());
    assert_eq!(o.write.written.len(), 22);
    let again = run(&db, &chain).await;
    assert_eq!(again.write.written.len(), 0, "a second run adds nothing");
    assert_eq!(again.write.already_known, 22);
}

// ---------------------------------------------------------------------------------------
// P6-A2 / A2b — never resurrect; never insert what the chain calls spent or cannot say
// ---------------------------------------------------------------------------------------

/// Z: a coin our own unbroadcast transaction is spending (`spendable=0`, `spent_by` set);
/// the indexer and the spent check both still say unspent — the honest state.
#[tokio::test]
async fn p6_a2_a_known_spent_row_is_never_made_spendable() {
    let db = mdb("a2");
    let mut chain = FakeChain::new();
    let (a5, l5) = { let d = db.lock().unwrap(); (receive_address(&d, 5), our_p2pkh(&d, 5)) };
    let z = chain.pay(1, &a5, 2_000, l5.clone());
    let z_sibling = chain.pay(2, &a5, 3_000, l5.clone()); // positive control: unknown, unspent
    let spender = {
        let d = db.lock().unwrap();
        let c = d.connection();
        c.execute("INSERT INTO transactions (user_id, txid, status, reference_number, description, is_outgoing, satoshis, created_at, updated_at)
                   VALUES (1, ?1, 'sending', 'a2-spender', 'a2', 1, 0, 0, 0)", [format!("{:064x}", 0xa2)]).unwrap();
        let spender = c.last_insert_rowid();
        crate::classify::p5_tests::raw_row(&d, &z, 0, 2_000, Some(&l5), true, Some("2-receive address"), Some("5"), true);
        c.execute("UPDATE outputs SET spendable = 0, spent_by = ?1, spending_description = 'a2-spend' WHERE txid = ?2",
                  rusqlite::params![spender, z]).unwrap();
        spender
    };
    // Run twice: once as listed, once with the txid upper-cased by the indexer.
    run(&db, &chain).await;
    let upper = z.to_ascii_uppercase();
    chain.listed.get_mut(&a5).unwrap().push((upper, 0, 2_000));
    let o = run(&db, &chain).await;

    let (id_z, _, _, _, spendable) = row(&db, &z, 0).unwrap();
    assert!(!spendable, "Z was made spendable again");
    let spent_by: Option<i64> = db.lock().unwrap().connection().query_row(
        "SELECT spent_by FROM outputs WHERE outputId = ?1", [id_z], |r| r.get(0)).unwrap();
    assert_eq!(spent_by, Some(spender));
    assert_eq!(count(&db, &z, 0), 1, "a second row for Z (case)");
    let free: i64 = db.lock().unwrap().connection().query_row(
        "SELECT COUNT(*) FROM money_utxos WHERE output_id = ?1 AND reserved_by IS NULL", [id_z], |r| r.get(0)).unwrap();
    assert_eq!(free, 0, "Z is free in the money index");
    assert!(row(&db, &z_sibling, 0).is_some(), "control: the unknown sibling at the same address is written");
    assert!(o.write.already_known >= 1);
}

#[tokio::test]
async fn p6_a2b_the_chain_says_spent_or_cannot_say_nothing_is_written() {
    let db = mdb("a2b");
    let mut chain = FakeChain::new();
    let (a6, l6) = { let d = db.lock().unwrap(); (receive_address(&d, 6), our_p2pkh(&d, 6)) };
    let z2 = chain.pay(1, &a6, 2_000, l6.clone());
    let z4 = chain.pay(3, &a6, 2_500, l6.clone());
    let z5 = chain.pay(4, &a6, 2_600, l6.clone());
    let z3 = chain.pay(2, &a6, 3_000, l6);
    chain.spent.insert((z2.clone(), 0), SpentStatus::Spent { spending_txid: format!("{:064x}", 0xb2) });
    // Z4: spent status unknown (the real answer for any unspent P2PKH) AND the confirming
    // listing no longer names it. Z5: the confirming listing errors.
    chain.delisted.insert((z4.clone(), 0));
    chain.listing_errors.insert((z5.clone(), 0));
    let o = run(&db, &chain).await;
    assert!(row(&db, &z3, 0).is_some(), "control: Unknown + affirmatively listed is written — ownership is not what rejects Z2/Z4/Z5");
    assert!(row(&db, &z2, 0).is_none(), "Z2 (spent on chain) was written");
    assert!(row(&db, &z4, 0).is_none(), "Z4 (unknown, not listed) was written");
    assert!(row(&db, &z5, 0).is_none(), "Z5 (unknown, listing failed) was written");
    assert_eq!(o.write.spent, vec![format!("{}:0", z2)]);
    let mut unverified = o.write.unverified.clone();
    unverified.sort();
    let mut want = vec![format!("{}:0", z4), format!("{}:0", z5)];
    want.sort();
    assert_eq!(unverified, want);
    assert!(!o.complete(), "an unknown spent status is not a complete scan");
}

// ---------------------------------------------------------------------------------------
// P6-A4 — signable or not at all: the signer's key for each written row pays the
// chain's script
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn p6_a4_every_written_row_signs_for_the_chain_script() {
    let db = mdb("a4");
    let mut chain = FakeChain::new();
    let mut paid = Vec::new();
    for (salt, i) in [(3u8, 3), (7, 7), (12, 12)] {
        let (a, s) = { let d = db.lock().unwrap(); (receive_address(&d, i), our_p2pkh(&d, i)) };
        paid.push(chain.pay(salt, &a, 1_000, s));
    }
    // Mis-tagged: listed at index 7's address, but the chain's script is index 8's.
    let (a7, l8) = { let d = db.lock().unwrap(); (receive_address(&d, 7), our_p2pkh(&d, 8)) };
    let mis = chain.pay(78, &a7, 1_000, l8);
    let o = run(&db, &chain).await;

    assert!(row(&db, &mis, 0).is_none(), "the mis-tagged coin was written");
    assert_eq!(o.write.not_ours, vec![format!("{}:0", mis)]);
    for txid in &paid {
        let (_, prefix, suffix, ..) = row(&db, txid, 0).unwrap_or_else(|| panic!("{} not written", txid));
        let chain_script = {
            let raw = &chain.raws[txid];
            let spans = crate::reconcile::parse_tx_output_spans(raw).unwrap();
            raw[spans[0].1..spans[0].1 + spans[0].2].to_vec()
        };
        let d = db.lock().unwrap();
        let key = crate::database::helpers::derive_key_for_output(&d, prefix.as_deref(), suffix.as_deref(), None).unwrap();
        let derived = p2pkh_for_key(&key);
        assert_eq!(hex::encode(&derived), hex::encode(&chain_script),
                   "{}: prefix {:?} suffix {:?} does not sign for the chain's script", txid, prefix, suffix);
    }
}

// ---------------------------------------------------------------------------------------
// P6-A9 — self only; the response says how PeerPay is found
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn p6_a9_a_foreign_key_coin_is_never_written_and_the_note_says_so() {
    let db = mdb("a9");
    let mut chain = FakeChain::new();
    let (a3, l3, a5) = { let d = db.lock().unwrap(); (receive_address(&d, 3), our_p2pkh(&d, 3), receive_address(&d, 5)) };
    let s = chain.pay(3, &a3, 1_000, l3);
    // P: a BRC-29-style coin locked to a key this wallet's self-derivation never makes,
    // listed (by a lying/colliding indexer) under index 5 — with index 5's assumed script.
    let p = chain.pay(5, &a5, 1_000, p2pkh_for_key(&[0x29u8; 32]));
    let o = run(&db, &chain).await;
    assert!(row(&db, &s, 0).is_some(), "the self coin is written");
    assert!(row(&db, &p, 0).is_none(), "the foreign-key coin was written as ours");
    assert!(chain.raw_asked.lock().unwrap().contains(&p), "P was observed — rejected on its chain script, not skipped");
    assert_eq!(o.write.not_ours, vec![format!("{}:0", p)]);
    let note = o.to_json()["note"].as_str().unwrap_or_default().to_string();
    assert!(note.contains("PeerPay") && note.contains("Claim a payment"), "note: {:?}", note);
}

// ---------------------------------------------------------------------------------------
// P6-A12 — nothing derives or writes a BIP-32 address (K1)
// ---------------------------------------------------------------------------------------

fn non_brc42_address_rows(db: &Mutex<WalletDatabase>, after_id: i64) -> Vec<(i32, String)> {
    let d = db.lock().unwrap();
    let rows: Vec<(i32, String)> = {
        let mut st = d.connection().prepare("SELECT \"index\", address FROM addresses WHERE id > ?1 AND \"index\" >= 0").unwrap();
        st.query_map([after_id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect()
    };
    rows.into_iter().filter(|(i, a)| receive_address(&d, *i) != *a).collect()
}

fn max_address_id(db: &Mutex<WalletDatabase>) -> i64 {
    db.lock().unwrap().connection().query_row("SELECT COALESCE(MAX(id), 0) FROM addresses", [], |r| r.get(0)).unwrap()
}

#[tokio::test]
async fn p6_a12_no_scan_derives_or_writes_a_bip32_address() {
    let db = mdb("a12");
    let mut chain = FakeChain::new();
    let (a4, l4, seed) = { let d = db.lock().unwrap();
        let phrase = d.get_cached_mnemonic().unwrap().to_string();
        let seed = bip39::Mnemonic::parse_in(bip39::Language::English, &phrase).unwrap().to_seed("");
        (receive_address(&d, 4), our_p2pkh(&d, 4), seed) };
    let anchor = chain.pay(4, &a4, 1_000, l4);
    let mut bip32 = Vec::new();
    for i in [4u32, 9] {
        let (addr, _, _) = crate::recovery::derive_address_at_path(&seed, &[(i, false)]).unwrap();
        let script = crate::recovery::address_to_p2pkh_script(&addr).unwrap();
        bip32.push((addr.clone(), chain.pay(100 + i as u8, &addr, 1_000, script)));
    }
    let before = max_address_id(&db);

    // Instrument first: a planted BIP-32 row is seen by the checker.
    {
        let d = db.lock().unwrap();
        let wid = crate::database::WalletRepository::new(d.connection()).get_primary_wallet().unwrap().unwrap().id.unwrap();
        crate::database::AddressRepository::new(d.connection()).create(&crate::database::Address {
            id: None, wallet_id: wid, index: 50, address: bip32[1].0.clone(), public_key: String::new(),
            used: false, balance: 0, pending_utxo_check: false, created_at: 0,
        }).unwrap();
    }
    assert_eq!(non_brc42_address_rows(&db, before).len(), 1, "instrument: the planted BIP-32 row is not seen");
    db.lock().unwrap().connection().execute("DELETE FROM addresses WHERE \"index\" = 50", []).unwrap();

    let o = run(&db, &chain).await;
    assert!(row(&db, &anchor, 0).is_some(), "anchor: the BRC-42 coin at index 4 is written");
    assert_eq!(non_brc42_address_rows(&db, before), vec![], "a scan wrote a non-BRC-42 address row");
    let bip32_rows: i64 = db.lock().unwrap().connection().query_row(
        "SELECT COUNT(*) FROM outputs WHERE derivation_prefix = 'bip32'", [], |r| r.get(0)).unwrap();
    assert_eq!(bip32_rows, 0);
    let asked = chain.asked.lock().unwrap().clone();
    for (addr, txid) in &bip32 {
        assert!(!asked.contains(addr), "the scan asked the indexer for BIP-32 address {}", addr);
        assert!(row(&db, txid, 0).is_none(), "a BIP-32 coin was written");
    }
    assert_eq!(indices(&o), vec![4]);
}

// ---------------------------------------------------------------------------------------
// P6-A15 (the "once" half, T1) — the same coin listed in another case is one row
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn p6_a15_a_coin_is_credited_once_whatever_the_txid_case() {
    let db = mdb("a15");
    let mut chain = FakeChain::new();
    let (a8, l8) = { let d = db.lock().unwrap(); (receive_address(&d, 8), our_p2pkh(&d, 8)) };
    let t = chain.pay(8, &a8, 4_000, l8);
    let first = run(&db, &chain).await;
    assert_eq!(first.write.written.len(), 1);
    let bal = || crate::database::OutputRepository::new(db.lock().unwrap().connection()).calculate_balance(1).unwrap();
    let b1 = bal();
    chain.listed.insert(a8, vec![(t.to_ascii_uppercase(), 0, 4_000)]);
    let second = run(&db, &chain).await;
    assert_eq!(second.write.written.len(), 0);
    assert_eq!(count(&db, &t, 0), 1, "the coin counts twice");
    assert_eq!(bal(), b1);
    let stored: String = db.lock().unwrap().connection().query_row(
        "SELECT txid FROM outputs WHERE lower(txid) = ?1", [&t], |r| r.get(0)).unwrap();
    assert_eq!(stored, t, "txid stored lowercase");
}

// ---------------------------------------------------------------------------------------
// P6-A17 — a restore with no backup finds the wallet's own coins (K2, K9)
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn p6_a17_no_backup_recovery_finds_brc42_coins_past_index_20() {
    let db = mdb("a17");
    let mut chain = FakeChain::new();
    let (seed, a7, l7, a9, l9, a45, l45) = { let d = db.lock().unwrap();
        let phrase = d.get_cached_mnemonic().unwrap().to_string();
        (bip39::Mnemonic::parse_in(bip39::Language::English, &phrase).unwrap().to_seed(""),
         receive_address(&d, 7), our_p2pkh(&d, 7), receive_address(&d, 9), our_p2pkh(&d, 9),
         receive_address(&d, 45), our_p2pkh(&d, 45)) };
    let c7 = chain.pay(7, &a7, 5_000, l7);
    chain.listed.get_mut(&a7).unwrap().push((c7.to_ascii_uppercase(), 0, 5_000));
    let c9 = chain.pay(9, &a9, 6_000, l9);
    chain.spent.insert((c9.clone(), 0), SpentStatus::Spent { spending_txid: format!("{:064x}", 9) });
    let c45 = chain.pay(45, &a45, 7_000, l45);
    let (m3, _, _) = crate::recovery::derive_address_at_path(&seed, &[(3, false)]).unwrap();
    let b3 = chain.pay(103, &m3, 1_000, crate::recovery::address_to_p2pkh_script(&m3).unwrap());

    let o = run(&db, &chain).await;
    let (id7, prefix, suffix, purpose, spendable) = row(&db, &c7, 0).expect("the index-7 coin is recovered");
    assert_eq!(count(&db, &c7, 0), 1);
    assert_eq!((prefix.as_deref(), suffix.as_deref()), (Some("2-receive address"), Some("7")));
    assert_eq!(purpose.as_deref(), Some(SCAN_PURPOSE));
    assert!(spendable && change_of(&db.lock().unwrap(), id7), "classified money by the seam");
    assert!(row(&db, &c45, 0).is_some(), "the coin at 45 (past high-water + 20, inside the 50 gap) is recovered");
    assert!(row(&db, &c9, 0).is_none(), "a coin the chain calls spent was recovered");
    assert!(row(&db, &b3, 0).is_none(), "a BIP-32 coin was recovered");
    assert_eq!(o.write.written.len(), 2, "rows written, not scan hits");
    let again = run(&db, &chain).await;
    assert_eq!(again.write.written.len(), 0, "a second run changes nothing");
}

// ---------------------------------------------------------------------------------------
// P6-A11 (unit half) — the one next-index source
// ---------------------------------------------------------------------------------------

#[test]
fn p6_a11_claims_are_above_both_sources_and_never_repeat() {
    let db = mdb("a11u");
    seed_counter_and_rows(&db, 12, 5);
    let d = db.lock().unwrap();
    let repo = crate::database::WalletRepository::new(d.connection());
    let wid = repo.get_primary_wallet().unwrap().unwrap().id.unwrap();
    let a = repo.claim_next_receive_index(wid).unwrap();
    let b = repo.claim_next_receive_index(wid).unwrap();
    assert_eq!((a, b), (13, 14));
    repo.raise_current_index(wid, 3).unwrap();
    assert_eq!(repo.receive_high_water(wid).unwrap(), 14, "the counter never moves down");
    assert!(repo.raise_current_index(wid + 99, 1).is_err(), "a write that matched no row is an error");
}

// ---------------------------------------------------------------------------------------
// The scan's ceiling: an indexer reporting history at every address cannot keep the scan
// going for ever, and stopping there is "incomplete", never "complete"
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn p6_scan_stops_at_its_ceiling_and_says_so() {
    let db = mdb("ceil");
    let mut chain = FakeChain::new();
    chain.history_everywhere = true;
    let o = run(&db, &chain).await;
    let ceiling = crate::recovery::SCAN_MARGIN + crate::recovery::SCAN_CEILING_ABOVE; // high-water 0
    assert!(chain.highest_index_asked.load(Ordering::SeqCst) <= ceiling as i64, "read past the ceiling");
    assert_eq!(o.result.unchecked_ranges.last(), Some(&(ceiling + 1, u32::MAX)), "{:?}", o.result.unchecked_ranges);
    assert!(!o.complete(), "stopping at the ceiling was reported complete");
}

// ---------------------------------------------------------------------------------------
// Rate limiting (2026-10-09 sitting): WhatsOnChain answered 429 to the scan's history
// burst, and a parent fetch failed — both are retried with backoff; and an address the
// scan saw a coin at is watched again even when the coin could not be verified.
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn p6_rate_limited_history_is_retried_not_reported_unchecked() {
    let db = mdb("r429");
    let mut chain = FakeChain::new();
    let (a30, a45, l45) = { let d = db.lock().unwrap(); (receive_address(&d, 30), receive_address(&d, 45), our_p2pkh(&d, 45)) };
    chain.pay(45, &a45, 1_000, l45);
    chain.history_429.lock().unwrap().insert(a30, 2); // refused twice, then answers
    let o = run(&db, &chain).await;
    assert_eq!(o.result.unchecked_ranges, vec![], "a rate-limited address was reported unchecked");
    assert_eq!(indices(&o), vec![45]);
}

#[tokio::test]
async fn p6_a_failed_parent_fetch_is_retried_before_the_coin_is_left_unverified() {
    let db = mdb("rraw");
    let mut chain = FakeChain::new();
    let (a3, l3) = { let d = db.lock().unwrap(); (receive_address(&d, 3), our_p2pkh(&d, 3)) };
    let t = chain.pay(3, &a3, 5_000, l3);
    chain.raw_failures.lock().unwrap().insert(t.clone(), 2);
    let o = run(&db, &chain).await;
    assert_eq!(o.write.written, vec![format!("{}:0", t)], "{:?}", o.write);
}

#[tokio::test]
async fn p6_an_unverified_coins_address_is_watched_again() {
    let db = mdb("rwatch");
    let mut chain = FakeChain::new();
    let (a7, l7) = { let d = db.lock().unwrap(); (receive_address(&d, 7), our_p2pkh(&d, 7)) };
    let t = chain.pay(7, &a7, 5_000, l7);
    chain.raw_failures.lock().unwrap().insert(t.clone(), 99); // never readable this run
    let o = run(&db, &chain).await;
    assert_eq!(o.write.unverified, vec![format!("{}:0", t)]);
    let watched: Option<(String, i64)> = { use rusqlite::OptionalExtension;
        db.lock().unwrap().connection().query_row(
            "SELECT address, pending_utxo_check FROM addresses WHERE \"index\" = 7", [],
            |r| Ok((r.get(0)?, r.get(1)?))).optional().unwrap() };
    assert_eq!(watched, Some((a7, 1)), "the address of an unverified coin is not being watched");
}

#[tokio::test]
async fn p6_an_existing_unwatched_address_is_watched_again() {
    let db = mdb("rwatch2");
    let mut chain = FakeChain::new();
    let (a7, l7) = { let d = db.lock().unwrap(); (receive_address(&d, 7), our_p2pkh(&d, 7)) };
    {
        let d = db.lock().unwrap();
        let wid = crate::database::WalletRepository::new(d.connection()).get_primary_wallet().unwrap().unwrap().id.unwrap();
        crate::database::AddressRepository::new(d.connection()).create(&crate::database::Address {
            id: None, wallet_id: wid, index: 7, address: a7.clone(), public_key: String::new(),
            used: true, balance: 0, pending_utxo_check: false, created_at: 0 }).unwrap();
    }
    let t = chain.pay(7, &a7, 5_000, l7);
    chain.raw_failures.lock().unwrap().insert(t, 99);
    // An address row older than the 90-day watch window (ultra review 2, #3).
    db.lock().unwrap().connection().execute(
        "UPDATE addresses SET created_at = strftime('%s','now') - 200 * 86400 WHERE \"index\" = 7", []).unwrap();
    run(&db, &chain).await;
    // The background sync's own stale-clear must not undo the re-watch at once.
    crate::database::AddressRepository::new(db.lock().unwrap().connection())
        .clear_stale_pending_addresses(2160).unwrap();
    let pending: i64 = db.lock().unwrap().connection().query_row(
        "SELECT pending_utxo_check FROM addresses WHERE \"index\" = 7", [], |r| r.get(0)).unwrap();
    assert_eq!(pending, 1, "an existing, unwatched address with a coin on it is not watched again (or the 90-day clear undid it)");
}

// ---------------------------------------------------------------------------------------
// /code-review ultra 2, #2 — retries share one budget per scan: once spent, a refused
// read is not retried (the coin is reported unverified), never waited on without end
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn p6_retries_stop_when_the_scans_budget_is_spent() {
    let db = mdb("rbudget");
    let mut chain = FakeChain::new();
    let mut paid = Vec::new();
    for i in 3..=8u8 {
        let (a, l) = { let d = db.lock().unwrap(); (receive_address(&d, i as i32), our_p2pkh(&d, i as i32)) };
        paid.push(chain.pay(i, &a, 1_000, l));
    }
    // Each parent fetch fails twice first: 1 + 2 = 3 ms of waiting per coin; six coins
    // would need 18 ms, the test budget is 10 ms.
    for t in &paid { chain.raw_failures.lock().unwrap().insert(t.clone(), 2); }
    let o = run(&db, &chain).await;
    assert!(!o.write.unverified.is_empty(), "every coin was retried past the budget: {:?}", o.write);
    assert!(o.write.written.len() >= 2, "the budget allowed some retries: {:?}", o.write);
    assert_eq!(o.write.written.len() + o.write.unverified.len(), paid.len());
    assert!(!o.complete(), "unverified coins make the scan incomplete");
}
