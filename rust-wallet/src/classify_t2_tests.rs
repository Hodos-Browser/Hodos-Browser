//! B5-T2-P1 rows at the seam (`classify.rs`): a recognised token is FILED out of the
//! default basket (so out of the balance and the held line); other token shapes are
//! HELD with `token_shaped`; plain money still passes. REDs are scratch mutations,
//! recorded in the phase contract's evidence folder.
use super::*;
use super::p5_tests::{change_of, our_p2pkh, raw_row, selected, wallet};
use crate::database::{OutputRepository, TagRepository};
use crate::tokens::tests::{envelope, hand_built_bsv20, hand_built_mandala, real, BSV20_TICK_TRANSFER,
                           BSV21_TRANSFER, ORDINAL};

const RECV: Option<&str> = Some("2-receive address");

fn basket_of(db: &WalletDatabase, id: i64) -> Option<String> {
    db.connection().query_row(
        "SELECT b.name FROM outputs o LEFT JOIN output_baskets b ON o.basket_id = b.basketId WHERE o.outputId = ?1",
        [id], |r| r.get(0)).unwrap()
}

fn tags_of(db: &WalletDatabase, id: i64) -> Vec<String> {
    let mut t = TagRepository::new(db.connection()).get_tags_for_output(id).unwrap();
    t.sort();
    t
}

fn ci_of(db: &WalletDatabase, id: i64) -> Option<String> {
    db.connection().query_row("SELECT custom_instructions FROM outputs WHERE outputId = ?1", [id], |r| r.get(0)).unwrap()
}

fn held_reason(db: &WalletDatabase, txid: &str) -> Option<Reason> {
    list_held(db, 1).unwrap().into_iter().find(|h| h.txid.as_deref() == Some(txid)).map(|h| h.reason)
}

fn balance(db: &WalletDatabase) -> i64 {
    OutputRepository::new(db.connection()).calculate_balance(1).unwrap()
}

/// `P1-A2` (seam half) — the real ordinal, written to the default basket by a route that
/// asked for nothing, is filed in `1sat` with `ordinal` + `type:` and leaves the balance,
/// the held line and both selectors.
#[test]
fn p1_a2_real_ordinal_is_filed_in_1sat() {
    let db = wallet("t2a2");
    let (sats, script) = real(ORDINAL.0, ORDINAL.1);
    let id = raw_row(&db, ORDINAL.0, 0, sats, Some(&script), true, RECV, Some("0"), false);
    let before = balance(&db);
    let rep = stamp_outputs(&db, &[id], "test").unwrap();
    assert_eq!(rep.filed, 1, "{:?}", rep);
    assert_eq!(basket_of(&db, id).as_deref(), Some("1sat"));
    let tags = tags_of(&db, id);
    assert!(tags.contains(&"ordinal".to_string()) && tags.iter().any(|t| t.starts_with("type:")), "{:?}", tags);
    assert!(!tags.iter().any(|t| t.starts_with("origin")), "origin is P3's to verify, not claimed here: {:?}", tags);
    assert!(!change_of(&db, id) && !selected(&db, id));
    assert_eq!(held_reason(&db, ORDINAL.0), None, "a filed token is not in the held line");
    assert_eq!(balance(&db), before);
}

/// `P1-A3` — the real BSV-21 transfer is filed in `bsv21` (never `1sat`), tagged with the
/// id decoded from the inscription, with the wallet's CI.
#[test]
fn p1_a3_real_bsv21_transfer_is_filed_in_bsv21_never_1sat() {
    let db = wallet("t2a3");
    let (sats, script) = real(BSV21_TRANSFER.0, BSV21_TRANSFER.1);
    let id = raw_row(&db, BSV21_TRANSFER.0, 1, sats, Some(&script), true, RECV, Some("0"), false);
    // Subject check: the same query can return a 1sat row (an ordinal in the same DB).
    let (os, oscript) = real(ORDINAL.0, ORDINAL.1);
    let ord = raw_row(&db, ORDINAL.0, 0, os, Some(&oscript), true, RECV, Some("1"), false);
    stamp_outputs(&db, &[id, ord], "test").unwrap();
    assert_eq!(basket_of(&db, id).as_deref(), Some("bsv21"));
    assert_eq!(basket_of(&db, ord).as_deref(), Some("1sat"), "the 1sat query can return a row at all");
    let tid = "6a0a5f91ea03cdd4d7371b4ee0572dadb349588cb3b0bfc95fba0cd2a1987488_0";
    assert_eq!(tags_of(&db, id), vec![format!("bsv21:{}", tid)]);
    let ci: serde_json::Value = serde_json::from_str(&ci_of(&db, id).unwrap()).unwrap();
    assert_eq!((ci["id"].as_str(), ci["amt"].as_str(), ci["op"].as_str()), (Some(tid), Some("100000000"), Some("transfer")));
    assert!(!selected(&db, id));
}

/// A sender's `customInstructions` is kept verbatim when the wallet files the row (BRC-37);
/// the wallet's own tags are still added.
#[test]
fn p1_a8_seam_keeps_a_senders_ci_verbatim() {
    let db = wallet("t2ci");
    let (sats, script) = real(BSV21_TRANSFER.0, BSV21_TRANSFER.1);
    let id = raw_row(&db, BSV21_TRANSFER.0, 1, sats, Some(&script), true, RECV, Some("0"), false);
    let sender = r#"{"amt":"1000000","note":"from a sender"}"#;
    db.connection().execute("UPDATE outputs SET custom_instructions = ?1 WHERE outputId = ?2",
                            rusqlite::params![sender, id]).unwrap();
    stamp_outputs(&db, &[id], "test").unwrap();
    assert_eq!(basket_of(&db, id).as_deref(), Some("bsv21"));
    assert_eq!(ci_of(&db, id).as_deref(), Some(sender));
}

/// `P1-A4` — token shapes the wallet does not file are HELD with the seam's token reason
/// (not rule ②'s), listed, not money, and "Treat as money" refuses them.
#[test]
fn p1_a4_held_token_shapes_carry_the_token_reason() {
    let db = wallet("t2a4");
    let (_, tick) = real(BSV20_TICK_TRANSFER.0, BSV20_TICK_TRANSFER.1);
    let (_, ordinal) = real(ORDINAL.0, ORDINAL.1);
    let deploy_auth = hand_built_bsv20(r#"{"p":"bsv-20","op":"deploy+auth","sym":"X"}"#);
    let mut ours_mandala = hand_built_mandala();
    ours_mandala.truncate(ours_mandala.len() - 25);
    ours_mandala.extend(our_p2pkh(&db, 4));
    let cases: Vec<(&str, Vec<u8>, i64)> = vec![
        ("deploy+auth (hand-built), 1 sat", deploy_auth, 1),
        ("legacy BSV-20 tick (real), 1 sat", tick, 1),
        ("Mandala to our key (hand-built), 1 sat", ours_mandala.clone(), 1),
        ("Mandala to our key (hand-built), 5000 sats", ours_mandala, 5_000),
        ("real ordinal envelope on 546 sats", ordinal, 546),
    ];
    for (i, (name, script, sats)) in cases.into_iter().enumerate() {
        let txid = format!("{:064x}", 0xa400 + i);
        let id = raw_row(&db, &txid, 0, sats, Some(&script), true, RECV, Some("4"), false);
        let rep = stamp_outputs(&db, &[id], "test").unwrap();
        assert_eq!((rep.filed, rep.stamped), (0, 0), "{}", name);
        assert_eq!(basket_of(&db, id), None, "{}: stays in the default pool", name);
        assert!(!change_of(&db, id) && !selected(&db, id), "{}: not money", name);
        assert_eq!(held_reason(&db, &txid), Some(Reason::TokenShaped), "{}: held with the token reason", name);
        assert_eq!(treat_as_money(&db, &txid, 0, true), Err(TreatRefusal::Refused(Reason::TokenShaped)), "{}", name);
    }
}

/// `P1-A5` — the recognisers do not over-claim: a 1-sat plain P2PKH to our key is held
/// (rule ③), NOT filed in `1sat` (no envelope = not an ordinal); a multi-sat plain P2PKH
/// to our key is money.
#[test]
fn p1_a5_plain_coins_are_not_tokens() {
    let db = wallet("t2a5");
    let ours = our_p2pkh(&db, 5);
    let one = raw_row(&db, &"a5".repeat(32), 0, 1, Some(&ours), true, RECV, Some("5"), false);
    let many = raw_row(&db, &"a6".repeat(32), 0, 5_000, Some(&ours), true, RECV, Some("5"), false);
    stamp_outputs(&db, &[one, many], "test").unwrap();
    assert_eq!(basket_of(&db, one), None);
    assert_eq!(held_reason(&db, &"a5".repeat(32)), Some(Reason::OneSat));
    assert!(change_of(&db, many) && selected(&db, many), "plain multi-sat money still passes");
}

/// `P1-A6` — the vacuous-pass trap. Pre-P4-shaped rows (`script_length NULL`, the stored
/// script fabricated from the address): (i) a 1-sat and (ii) a multi-sat row are held as
/// `not_observed` — the reason proves the seam did not read the fabricated script; (iii)
/// a row with no script at all is `not_observed` too (the test reads the field it claims).
#[test]
fn p1_a6_unobserved_rows_are_never_read() {
    let db = wallet("t2a6");
    let fabricated = our_p2pkh(&db, 6);
    let i = raw_row(&db, &"b1".repeat(32), 0, 1, Some(&fabricated), false, RECV, Some("6"), false);
    let ii = raw_row(&db, &"b2".repeat(32), 0, 546, Some(&fabricated), false, RECV, Some("6"), false);
    let iii = raw_row(&db, &"b3".repeat(32), 0, 546, None, false, RECV, Some("6"), false);
    stamp_outputs(&db, &[i, ii, iii], "test").unwrap();
    for (t, id) in [("b1", i), ("b2", ii), ("b3", iii)] {
        assert_eq!(held_reason(&db, &t.repeat(32)), Some(Reason::NotObserved), "{}", t);
        assert!(!change_of(&db, id) && !selected(&db, id));
    }
}

/// A script above the inline cap (`locking_script` NULL, `script_length` + `script_offset`
/// set — B5-T1-P4) is read from its pinned parent and filed; with the parent absent the
/// row is held `unreadable`, never money and never "not a token".
#[test]
fn p1_large_script_is_read_from_its_parent() {
    let db = wallet("t2big");
    let path = format!("{}/tests/fixtures/tokens/{}.hex", env!("CARGO_MANIFEST_DIR"), ORDINAL.0);
    let raw_hex = std::fs::read_to_string(path).unwrap().trim().to_string();
    let raw = hex::decode(&raw_hex).unwrap();
    let (_, off, len) = crate::reconcile::parse_tx_output_spans(&raw).unwrap()[0];
    let c = db.connection();
    let insert = |txid: &str| {
        c.execute("INSERT INTO outputs (user_id, spendable, change, vout, satoshis, txid, derivation_prefix,
                   derivation_suffix, locking_script, script_length, script_offset, confirmed, provided_by,
                   purpose, type, created_at, updated_at)
                   VALUES (1, 1, 0, 0, 1, ?1, '2-receive address', '0', NULL, ?2, ?3, 1, 'you', 'receive', 'P2PKH', 0, 0)",
                  rusqlite::params![txid, len as i64, off as i64]).unwrap();
        c.last_insert_rowid()
    };
    let missing = insert(&"cd".repeat(32));
    let held = insert(ORDINAL.0);
    stamp_outputs(&db, &[missing], "test").unwrap();
    assert_eq!(held_reason(&db, &"cd".repeat(32)), Some(Reason::Unreadable), "no parent ⇒ unreadable, held");
    c.execute("INSERT INTO parent_transactions (txid, raw_hex, cached_at) VALUES (?1, ?2, 0)",
              rusqlite::params![ORDINAL.0, raw_hex]).unwrap();
    stamp_outputs(&db, &[held], "test").unwrap();
    assert_eq!(basket_of(&db, held).as_deref(), Some("1sat"), "{} B script read from the parent", len);
}

/// The background pass files too (decision 7 ④'s retry; the route every ingest falls
/// back to): a recognised token written with no seam call is filed by `stamp_all`.
#[test]
fn p1_background_pass_files_tokens() {
    let db = wallet("t2bg");
    let mut s = our_p2pkh(&db, 7);
    s.extend(envelope("text/plain", b"hello"));
    let id = raw_row(&db, &"e7".repeat(32), 0, 1, Some(&s), true, RECV, Some("7"), false);
    stamp_all(&db, "tick");
    assert_eq!(basket_of(&db, id).as_deref(), Some("1sat"));
    assert_eq!(tags_of(&db, id), vec!["ordinal".to_string(), "type:text/plain".to_string()]);
}

/// A token a site created (`createAction` basket output, already in `bsv21`) is given the
/// wallet's tags and CI under its FINAL txid — a `deploy+mint`'s id is its own outpoint.
#[test]
fn p1_site_created_token_is_tagged_under_its_final_txid() {
    let db = wallet("t2ca");
    let (_, script) = real(crate::tokens::tests::BSV21_DEPLOY_MINT.0, 0);
    let bid = crate::database::BasketRepository::new(db.connection()).find_or_insert("bsv21", 1).unwrap();
    let id = raw_row(&db, &"f1".repeat(32), 0, 1, Some(&script), true, None, None, false);
    db.connection().execute("UPDATE outputs SET basket_id = ?1 WHERE outputId = ?2", [bid, id]).unwrap();
    file_written_tokens(&db, &"f1".repeat(32), "test");
    let tid = format!("{}_0", "f1".repeat(32));
    assert_eq!(tags_of(&db, id), vec!["bsv21:deploy".to_string(), format!("bsv21:{}", tid)]);
    let ci: serde_json::Value = serde_json::from_str(&ci_of(&db, id).unwrap()).unwrap();
    assert_eq!(ci["id"], tid.as_str());
    assert_eq!(basket_of(&db, id).as_deref(), Some("bsv21"));
}

/// /code-review ultra #1: one row whose filing fails does not stop the others in the same
/// transaction. A trigger refuses the first row's update; the second row is still filed.
#[test]
fn p1_ultra1_one_failed_row_does_not_stop_the_batch() {
    let db = wallet("t2u1");
    let (_, s) = real(crate::tokens::tests::BSV21_DEPLOY_MINT.0, 0);
    let bid = crate::database::BasketRepository::new(db.connection()).find_or_insert("bsv21", 1).unwrap();
    let first = raw_row(&db, &"f2".repeat(32), 0, 1, Some(&s), true, None, None, false);
    let second = raw_row(&db, &"f2".repeat(32), 1, 1, Some(&s), true, None, None, false);
    db.connection().execute("UPDATE outputs SET basket_id = ?1 WHERE outputId IN (?2, ?3)", [bid, first, second]).unwrap();
    db.connection().execute_batch(&format!(
        "CREATE TRIGGER t2u1_refuse BEFORE UPDATE OF custom_instructions ON outputs WHEN NEW.outputId = {first}
         BEGIN SELECT RAISE(ABORT, 'test: refused'); END;")).unwrap();
    file_written_tokens(&db, &"f2".repeat(32), "test");
    assert!(tags_of(&db, first).is_empty(), "the refused row rolled back whole");
    assert!(!tags_of(&db, second).is_empty(), "the next row is still filed");
}
