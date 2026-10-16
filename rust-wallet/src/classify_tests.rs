//! B5-T1-P5 T1 rows for `classify.rs`. Every row's RED is a scratch mutation of the
//! code it guards, run and reverted (evidence in the phase contract §4b), implementing
//! the controls designed by controls-B (Fable), 2026-09-28.
use super::*;
use crate::database::{OutputRepository, WalletDatabase};
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};

/// A real temp wallet (keys cached, schema at the current version).
pub(crate) fn wallet(tag: &str) -> WalletDatabase {
    let path = std::env::temp_dir().join(format!("hodos_p5_{}_{}.db", tag, uuid::Uuid::new_v4()));
    let mut db = WalletDatabase::new(path).unwrap();
    db.create_wallet_with_first_address(None).unwrap();
    db
}

pub(crate) fn p2pkh_for_key(privkey: &[u8]) -> Vec<u8> {
    let secp = secp256k1::Secp256k1::new();
    let pk = secp256k1::PublicKey::from_secret_key(&secp, &secp256k1::SecretKey::from_slice(privkey).unwrap()).serialize();
    let mut s = vec![0x76, 0xa9, 0x14];
    s.extend_from_slice(&Ripemd160::digest(Sha256::digest(pk)));
    s.extend_from_slice(&[0x88, 0xac]);
    s
}

/// The exact P2PKH template paying receive address `index` of this wallet.
pub(crate) fn our_p2pkh(db: &WalletDatabase, index: i32) -> Vec<u8> {
    let k = crate::database::helpers::derive_key_for_output(db, Some("2-receive address"), Some(&index.to_string()), None).unwrap();
    p2pkh_for_key(&k)
}

pub(crate) fn master_p2pkh(db: &WalletDatabase) -> Vec<u8> {
    p2pkh_for_key(&crate::database::helpers::get_master_private_key_from_db(db).unwrap())
}

/// `ord` envelope appended to a P2PKH — the ticket's measured inscription shape
/// (`76a914…88ac 0063036f7264 5109 image/png …`), 1Sat Ordinals layout.
pub(crate) fn p2pkh_plus_envelope(p2pkh: &[u8]) -> Vec<u8> {
    let mut s = p2pkh.to_vec();
    s.extend(hex::decode("0063036f72645109").unwrap());
    s.extend(b"image/png");
    s.extend([0x00, 0x04, 0xde, 0xad, 0xbe, 0xef, 0x68]);
    s
}

/// A token prefix followed by an ordinary P2PKH — the BRC-162 layout (prefix first,
/// P2PKH last, any satoshi value). Shape, not a mainnet script.
pub(crate) fn prefix_plus_p2pkh(p2pkh: &[u8]) -> Vec<u8> {
    // Push length 0x12 = len("application/bsv-20") (B5-T2-P1: it was 0x09, so the script
    // did not parse at all and only rule 2 ever saw it).
    let mut s = hex::decode("0063036f72645112").unwrap();
    s.extend(b"application/bsv-20");
    s.extend([0x00, 0x02, 0x7b, 0x7d, 0x68]);
    s.extend_from_slice(p2pkh);
    s
}

/// Insert one default-pool row by raw SQL (a route that never calls the seam).
#[allow(clippy::too_many_arguments)]
pub(crate) fn raw_row(db: &WalletDatabase, txid: &str, vout: i32, sats: i64, script: Option<&[u8]>,
                      observed: bool, prefix: Option<&str>, suffix: Option<&str>, change: bool) -> i64 {
    let len = if observed { script.map(|s| s.len() as i64) } else { None };
    db.connection().execute(
        "INSERT INTO outputs (user_id, spendable, change, vout, satoshis, txid, derivation_prefix, derivation_suffix,
                              locking_script, script_length, confirmed, provided_by, purpose, type, created_at, updated_at)
         VALUES (1, 1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, 'you', 'receive', 'P2PKH', 0, 0)",
        rusqlite::params![change as i64, vout, sats, txid, prefix, suffix, script, len],
    ).unwrap();
    db.connection().last_insert_rowid()
}

fn row(db: &WalletDatabase, id: i64) -> Row {
    read_rows(db.connection(), &format!("{CANDIDATES} AND o.outputId = ?1"), &[&id]).unwrap().remove(0)
}

pub(crate) fn change_of(db: &WalletDatabase, id: i64) -> bool {
    db.connection().query_row("SELECT change FROM outputs WHERE outputId = ?1", [id], |r| r.get::<_, i64>(0)).unwrap() != 0
}

pub(crate) fn selected(db: &WalletDatabase, id: i64) -> bool {
    let repo = OutputRepository::new(db.connection());
    repo.get_spendable_by_user(1).unwrap().iter().chain(repo.get_spendable_confirmed_by_user(1).unwrap().iter())
        .any(|o| o.output_id == Some(id))
}

fn label(db: &WalletDatabase, id: i64) -> Option<String> {
    db.connection().query_row("SELECT derivation_prefix FROM outputs WHERE outputId = ?1", [id], |r| r.get(0)).unwrap()
}

const RECV: Option<&str> = Some("2-receive address");

/// `P5-A1` — the seam table, one fixture per case, through the real ownership check.
#[test]
fn p5_a1_seam_table() {
    let db = wallet("a1");
    let ours = our_p2pkh(&db, 3);
    let theirs = p2pkh_for_key(&[0x42u8; 32]);
    let cases: Vec<(&str, Vec<u8>, i64, bool, Class)> = vec![
        ("exact P2PKH to our key, 5000 sats", ours.clone(), 5000, true, Class::Money),
        ("exact P2PKH to our key, 1 sat", ours.clone(), 1, true, Class::Unknown(Reason::OneSat)),
        // B5-T2-P1 landed: an envelope on more than 1 sat is a held token, not rule 2's.
        ("P2PKH + ord envelope, multi-sat", p2pkh_plus_envelope(&ours), 5000, true,
         Class::Unknown(Reason::TokenShaped)),
        ("token prefix + P2PKH, multi-sat", prefix_plus_p2pkh(&ours), 5000, true,
         Class::Unknown(Reason::TokenShaped)),
        // A prefix no recogniser claims, then our P2PKH: only rule 2's EXACT 25-byte match
        // keeps it from money (the recognisers above would mask an ends_with rule).
        ("non-token prefix (OP_1 OP_DROP) + P2PKH, multi-sat", [vec![0x51, 0x75], ours.clone()].concat(), 5000, true,
         Class::Unknown(Reason::NotPlainPayment)),
        ("script_length NULL (stored script = the fabricated P2PKH)", ours.clone(), 5000, false,
         Class::Unknown(Reason::NotObserved)),
        ("exact P2PKH to a key that is not ours", theirs, 5000, true, Class::Unknown(Reason::NotOurs)),
    ];
    for (i, (name, script, sats, observed, want)) in cases.into_iter().enumerate() {
        let id = raw_row(&db, &format!("{:064x}", i + 1), 0, sats, Some(&script), observed, RECV, Some("3"), false);
        let got = class_of(&db, &row(&db, id));
        assert_eq!(got, want, "{}: script {} B", name, script.len());
    }
}

/// `P5-A3` — the vacuous-pass trap: a pre-P4-shaped row (`script_length NULL`, the
/// fabricated 25-byte P2PKH to one of our keys, 5 sats) is not money; a row with no
/// script at all is Unknown too, so the test is reading the field it claims to.
#[test]
fn p5_a3_fabricated_script_without_observation_is_not_money() {
    let db = wallet("a3");
    let fabricated = our_p2pkh(&db, 0);
    let a = raw_row(&db, &"a3".repeat(32), 0, 5, Some(&fabricated), false, RECV, Some("0"), false);
    let b = raw_row(&db, &"b3".repeat(32), 0, 5, None, false, RECV, Some("0"), false);
    assert_eq!(class_of(&db, &row(&db, a)), Class::Unknown(Reason::NotObserved));
    assert_eq!(class_of(&db, &row(&db, b)), Class::Unknown(Reason::NotObserved));
    stamp_all(&db, "test");
    assert!(!change_of(&db, a) && !selected(&db, a), "the fabricated row was stamped / selected");
}

/// `P5-A2` — the default is refusal: a row a route writes without the seam
/// (`change = 0`) is offered by neither selector; the same row forged `change = 1`
/// is — which is why `P5-A14` gates writes of the stamp.
#[test]
fn p5_a2_a_row_that_skipped_the_seam_is_selected_by_no_route() {
    let db = wallet("a2");
    let ours = our_p2pkh(&db, 0);
    let control = raw_row(&db, &"c2".repeat(32), 0, 9_000, Some(&ours), true, RECV, Some("0"), true);
    let skipped = raw_row(&db, &"a2".repeat(32), 0, 50_000, Some(&ours), true, RECV, Some("0"), false);
    assert!(selected(&db, control), "control: a stamped coin is selected (the selectors are live)");
    assert!(!selected(&db, skipped), "an unstamped row was offered to a spend path");
    let forged = raw_row(&db, &"f2".repeat(32), 0, 60_000, Some(&ours), true, RECV, Some("0"), true);
    assert!(selected(&db, forged), "the stamp is a plain column: a forged one is honoured (gate P5-A14)");
}

/// `P5-A7` — the `main.rs` master fix-up is gone, and the seam compares scripts, not
/// the `master` label. Across a restart (`WalletDatabase::new` = migrate + repair, the
/// code that ran before the deleted UPDATE): a NULL-derivation row paying a key that is
/// not the master stays unlabelled and unselected; after the classifier, a
/// NULL-derivation row whose observed script IS the master key's P2PKH is money and
/// carries the `master` recipe.
#[test]
fn p5_a7_null_derivation_rows_are_judged_by_script_not_label() {
    let db = wallet("a7");
    let path = std::path::PathBuf::from(db.connection().path().unwrap());
    let mnemonic = db.get_cached_mnemonic().unwrap().to_string();
    let not_master = our_p2pkh(&db, 5); // ours, but not the master key
    let master = master_p2pkh(&db);
    let a = raw_row(&db, &"a7".repeat(32), 0, 7, Some(&not_master), true, None, None, false);
    let b = raw_row(&db, &"b7".repeat(32), 0, 175_000, Some(&master), true, None, None, false);
    drop(db);
    let mut db = WalletDatabase::new(path).unwrap();
    assert_eq!((label(&db, a), label(&db, b)), (None, None), "a restart re-labelled a NULL-derivation row");
    assert!(!selected(&db, a) && !selected(&db, b));
    db.cache_mnemonic(mnemonic).unwrap();
    stamp_all(&db, "test");
    assert_eq!(label(&db, a), None);
    assert!(!change_of(&db, a) && !selected(&db, a), "a not-master NULL-derivation row became money");
    assert_eq!(class_of(&db, &row(&db, a)), Class::Unknown(Reason::NotOurs));
    assert_eq!(label(&db, b).as_deref(), Some("master"));
    assert!(change_of(&db, b) && selected(&db, b), "a genuine master-key output was stranded");
}

/// `P5-A10` — "Treat as money": exactly one row, only after confirmation, refused on
/// a token-basket row and on a held token-shaped row, audit trace written. B5-T2-P1b:
/// the promoted coin is a not-yet-read multi-sat one — a 1-sat coin is never promoted
/// (`p1b_*` below).
#[test]
fn p5_a10_treat_as_money_stamps_exactly_one_row() {
    let db = wallet("a10");
    let ours = our_p2pkh(&db, 0);
    let txid = "aa".repeat(32);
    let v0 = raw_row(&db, &txid, 0, 5_000, Some(&ours), false, RECV, Some("0"), false); // not observed => held
    let v1 = raw_row(&db, &txid, 1, 5_000, Some(&ours), false, RECV, Some("0"), false); // same txid
    let shaped = raw_row(&db, &"bb".repeat(32), 0, 5000, Some(&prefix_plus_p2pkh(&ours)), true, RECV, Some("0"), false);
    let token = raw_row(&db, &"cc".repeat(32), 0, 1, Some(&ours), true, RECV, Some("0"), false);
    db.connection().execute("INSERT INTO output_baskets (user_id, name, created_at, updated_at) VALUES (1, '1sat', 0, 0)", []).unwrap();
    let bid = db.connection().last_insert_rowid();
    db.connection().execute("UPDATE outputs SET basket_id = ?1 WHERE outputId = ?2", [bid, token]).unwrap();

    assert_eq!(treat_as_money(&db, &txid, 0, false), Err(TreatRefusal::NotConfirmed));
    assert!(!change_of(&db, v0));
    assert_eq!(treat_as_money(&db, &"cc".repeat(32), 0, true), Err(TreatRefusal::NotDefaultBasket("1sat".into())));
    assert_eq!(treat_as_money(&db, &"bb".repeat(32), 0, true), Err(TreatRefusal::Refused(Reason::TokenShaped)));
    assert_eq!(treat_as_money(&db, &txid, 0, true), Ok(v0));
    assert!(change_of(&db, v0) && !change_of(&db, v1) && !change_of(&db, shaped) && !change_of(&db, token),
            "exactly that outpoint is stamped");
    let desc: String = db.connection().query_row(
        "SELECT output_description FROM outputs WHERE outputId = ?1", [v0], |r| r.get(0)).unwrap();
    assert!(desc.contains(TREATED_MARK) && desc.contains("not_observed"), "audit trace: {}", desc);
    // The user's choice survives the next pass and a re-classification of that row.
    stamp_all(&db, "test");
    stamp_outputs(&db, &[v0], "test").unwrap();
    assert!(change_of(&db, v0), "the next pass removed the user's stamp");
}

/// `P5-A6` I6 control — the seam is authoritative both ways: our own change built with
/// a script that is not the exact template is not kept as money.
#[test]
fn p5_a6_i6_non_template_change_is_refused() {
    let db = wallet("a6");
    let ours = our_p2pkh(&db, 0);
    let mut pushdrop = vec![0x21];
    pushdrop.extend([0x02u8; 33]);
    pushdrop.extend([0xac, 0x00, 0x75]);
    let repo = OutputRepository::new(db.connection());
    let good = repo.insert_output(1, &"d6".repeat(32), 0, 5000, &hex::encode(&ours), None,
                                  RECV, Some("0"), None, None, true).unwrap();
    let bad = repo.insert_output(1, &"e6".repeat(32), 0, 5000, &hex::encode(&pushdrop), None,
                                 RECV, Some("0"), None, None, true).unwrap();
    assert!(!change_of(&db, good) && !change_of(&db, bad), "insert_output writes no stamp");
    after_write_id(&db, good, "I6:test");
    after_write_id(&db, bad, "I6:test");
    assert!(change_of(&db, good), "our own exact-P2PKH change is money");
    assert!(!change_of(&db, bad) && !selected(&db, bad), "a PushDrop-shaped 'change' became money");
}

/// `P5-A4` (T1 half) — the V28 migration on a synthetic V27 wallet: every row ends
/// money or held, and money + held = the pre-migration balance, per class, by outpoint.
#[test]
fn p5_a4_migration_keeps_every_coin_visible() {
    let db = wallet("a4");
    let ours = our_p2pkh(&db, 0);
    let rows = [
        raw_row(&db, &"01".repeat(32), 0, 40_000, Some(&ours), true, RECV, Some("0"), false), // received => money
        raw_row(&db, &"02".repeat(32), 0, 7_000, Some(&ours), true, RECV, Some("0"), true),   // pre-P5 change => money
        raw_row(&db, &"03".repeat(32), 0, 1, Some(&ours), true, RECV, Some("0"), false),      // 1-sat => held
        raw_row(&db, &"04".repeat(32), 0, 3_000, Some(&ours), false, RECV, Some("0"), true),  // unobserved, was change=1 => held
        raw_row(&db, &"05".repeat(32), 0, 2_000, Some(&p2pkh_plus_envelope(&ours)), true, RECV, Some("0"), false), // => held
    ];
    let repo = OutputRepository::new(db.connection());
    let before = repo.calculate_balance(1).unwrap() + repo.calculate_held(1).unwrap().0;
    assert_eq!(before, 52_001);
    crate::database::migrations::migrate_v27_to_v28(db.connection()).unwrap();
    assert_eq!(repo.calculate_balance(1).unwrap(), 0, "V28 resets every stamp");
    assert_eq!(repo.calculate_held(1).unwrap().0, before, "...and shows every coin as held meanwhile");
    stamp_all(&db, "test");
    let money = repo.calculate_balance(1).unwrap();
    let (held, held_n) = repo.calculate_held(1).unwrap();
    assert_eq!((money, held, held_n), (47_000, 5_001, 3), "per-class totals");
    assert_eq!(money + held, before, "nothing vanished");
    let stamped: Vec<bool> = rows.iter().map(|id| change_of(&db, *id)).collect();
    assert_eq!(stamped, vec![true, true, false, false, false], "by outpoint");
}

/// The trigger refresh: a V27-shaped trigger (no stamp term) is replaced at open.
#[test]
fn p5_v28_replaces_old_triggers() {
    let db = wallet("trig");
    let c = db.connection();
    c.execute_batch("DROP TRIGGER money_utxos_on_output_insert;
        CREATE TRIGGER money_utxos_on_output_insert AFTER INSERT ON outputs
        WHEN NEW.spendable = 1 AND NEW.derivation_prefix IS NOT NULL
        BEGIN INSERT OR IGNORE INTO money_utxos (output_id, user_id, satoshis, reserved_by, created_at)
              VALUES (NEW.outputId, NEW.user_id, NEW.satoshis, NULL, 0); END;").unwrap();
    assert!(crate::database::migrations::ensure_money_index_current(c).unwrap(), "stale trigger detected");
    let ours = our_p2pkh(&db, 0);
    let id = raw_row(&db, &"ab".repeat(32), 0, 5000, Some(&ours), true, RECV, Some("0"), false);
    let n: i64 = c.query_row("SELECT COUNT(*) FROM money_utxos WHERE output_id = ?1", [id], |r| r.get(0)).unwrap();
    assert_eq!(n, 0, "an unstamped insert entered the index through an old trigger");
    assert!(!crate::database::migrations::ensure_money_index_current(c).unwrap(), "current triggers left alone");
}

/// `P5-A9` (`R-RESTORE`, T1's half) — restore through the production encode / decode /
/// import path (`wallet_import`'s preparation) into a fresh wallet from the same phrase:
/// the import completes; a 1-sat inscription whose payload says `change = 1` (an old or
/// forged backup) lands **held, listed, not spendable**; the plain coin is money again.
/// The classifier makes no network call, so "indexer down" cannot change this result
/// (the seam has no parent-fetch `Err` to map). RED (scratch): import copies the
/// payload's `change` ⇒ the inscription is stamped and selectable.
#[test]
fn p5_a9_restore_lands_unclassifiable_coins_held_and_shown() {
    let src = wallet("a9src");
    let phrase = src.get_cached_mnemonic().unwrap().to_string();
    let ours = our_p2pkh(&src, 0);
    let ins = p2pkh_plus_envelope(&ours);
    let plain = raw_row(&src, &"9a".repeat(32), 0, 5_000, Some(&ours), true, RECV, Some("0"), true);
    let forged = raw_row(&src, &"9b".repeat(32), 0, 1, Some(&ins), true, RECV, Some("0"), true);
    assert!(selected(&src, plain) && selected(&src, forged), "fixture: the payload carries both stamped");
    let c = src.connection();
    let ik: String = c.query_row("SELECT identity_key FROM users ORDER BY userId LIMIT 1", [], |r| r.get(0)).unwrap();
    let key = [7u8; 32];
    let compressed = crate::backup::compress_for_onchain(c, &ik, 1_700_000_000).unwrap();
    let encrypted = crate::backup::encrypt_compressed(&key, &compressed).unwrap();
    let payload = crate::backup::deserialize_from_onchain(&encrypted, &key).unwrap();

    let path = std::env::temp_dir().join(format!("hodos_p5_a9dst_{}.db", uuid::Uuid::new_v4()));
    let mut dst = WalletDatabase::new(path).unwrap();
    dst.create_wallet_from_existing_mnemonic(&phrase, None).unwrap();
    {
        let c = dst.connection();
        let wid: i64 = c.query_row("SELECT id FROM wallets ORDER BY id LIMIT 1", [], |r| r.get(0)).unwrap();
        c.execute("DELETE FROM addresses WHERE wallet_id = ?1", [wid]).unwrap();
        c.execute("DELETE FROM output_baskets", []).unwrap();
        c.execute("DELETE FROM users", []).unwrap();
        crate::backup::import_to_db_with_ids(c, &payload, wid, 1).expect("the restore completes");
    }
    stamp_all(&dst, "I3:test");
    let id = |t: &str| dst.connection().query_row("SELECT outputId FROM outputs WHERE txid = ?1", [t], |r| r.get::<_, i64>(0)).unwrap();
    let (p_id, f_id) = (id(&"9a".repeat(32)), id(&"9b".repeat(32)));
    assert!(change_of(&dst, p_id) && selected(&dst, p_id), "the plain coin is money after restore");
    assert!(!change_of(&dst, f_id) && !selected(&dst, f_id), "the restored inscription is spendable");
    // B5-T2-P1: the restored 1-sat inscription is recognised and FILED in `1sat` (shown in
    // the Tokens view), not held — and never in the money set.
    let basket: Option<String> = dst.connection().query_row(
        "SELECT b.name FROM outputs o LEFT JOIN output_baskets b ON o.basket_id = b.basketId WHERE o.outputId = ?1",
        [f_id], |r| r.get(0)).unwrap();
    assert_eq!(basket.as_deref(), Some("1sat"), "the restored inscription is filed");
    assert!(!list_held(&dst, 1).unwrap().iter().any(|h| h.txid.as_deref() == Some(&"9b".repeat(32))),
            "a filed token is not also in the held line");
}

/// Adversarial review #1 — the "treated as money" mark is plain text that travels in a
/// backup; alone it must not make a coin money. A NotObserved row carrying the mark but
/// paying a key that is not ours stays held. RED (scratch): drop
/// `stored_script_is_ours` from `class_of`'s override ⇒ stamped.
#[test]
fn p5_review1_a_copied_treated_mark_does_not_make_money_alone() {
    let db = wallet("r1");
    let theirs = p2pkh_for_key(&[0x42u8; 32]);
    let ours = our_p2pkh(&db, 0);
    let forged = raw_row(&db, &"f1".repeat(32), 0, 9_000, Some(&theirs), false, RECV, Some("0"), false);
    let genuine = raw_row(&db, &"f2".repeat(32), 0, 9_000, Some(&ours), false, RECV, Some("0"), false);
    for id in [forged, genuine] {
        db.connection().execute("UPDATE outputs SET output_description = ?1 WHERE outputId = ?2",
            rusqlite::params![format!("{} at 1; was not_observed]", TREATED_MARK), id]).unwrap();
    }
    stamp_all(&db, "test");
    assert!(!change_of(&db, forged) && !selected(&db, forged), "a copied mark made a not-ours coin money");
    assert!(change_of(&db, genuine), "control: the mark on our own plain coin is honoured");
}

/// Adversarial review #3 — "cannot check" is not "not money": with the wallet locked, a
/// re-classification of a stamped coin leaves its stamp. RED (scratch): drop
/// `reason.is_definitive()` from `stamp_rows` ⇒ unstamped.
#[test]
fn p5_review3_a_locked_wallet_never_unmarks_money() {
    let mut db = wallet("r3");
    let ours = our_p2pkh(&db, 0);
    let id = raw_row(&db, &"f3".repeat(32), 0, 9_000, Some(&ours), true, RECV, Some("0"), false);
    stamp_all(&db, "test");
    assert!(change_of(&db, id), "fixture: stamped while unlocked");
    db.clear_cached_mnemonic();
    stamp_outputs(&db, &[id], "test").unwrap();
    assert!(change_of(&db, id) && selected(&db, id), "a locked wallet removed a money stamp");
}

// ── B5-T2-P1b — no "Treat as money" for a coin of 1 satoshi or less ─────────────────
// A transferred 1Sat Ordinal is an exact 1-sat P2PKH to our key: every older check
// (basket, confirmation, stored script, ownership) passes it, so only the new floor
// can refuse it. Fixtures are built that way on purpose.

fn held_entry(db: &WalletDatabase, txid: &str) -> HeldOutput {
    list_held(db, 1).unwrap().into_iter().find(|h| h.txid.as_deref() == Some(txid))
        .unwrap_or_else(|| panic!("{} is not in the held list", txid))
}

/// `P1b-A1` + `P1b-A3` (two-sided) — the Pixel Fox shape (observed exact 1-sat P2PKH to
/// our key, default basket, confirmed) is listed without the button and refused by the
/// route, and stays out of every selector; a not-yet-read 5,000-sat coin to our key in
/// the same wallet is still offered and promoted.
#[test]
fn p1b_a1_a_one_sat_coin_is_never_offered_or_promoted() {
    let db = wallet("p1b_a1");
    let ours = our_p2pkh(&db, 0);
    let nft = "fa".repeat(32);
    let plain = "fb".repeat(32);
    let n = raw_row(&db, &nft, 0, 1, Some(&ours), true, RECV, Some("0"), false);
    let p = raw_row(&db, &plain, 0, 5_000, Some(&ours), false, RECV, Some("0"), false);

    let h = held_entry(&db, &nft);
    assert_eq!((h.reason, h.can_treat_as_money), (Reason::OneSat, false), "the 1-sat coin is offered");
    assert_eq!(treat_as_money(&db, &nft, 0, true), Err(TreatRefusal::Refused(Reason::OneSat)));
    assert!(!change_of(&db, n) && !selected(&db, n), "the 1-sat coin became money");

    let h = held_entry(&db, &plain);
    assert_eq!((h.reason, h.can_treat_as_money), (Reason::NotObserved, true), "control: the feature still works");
    assert_eq!(treat_as_money(&db, &plain, 0, true), Ok(p));
    assert!(change_of(&db, p) && selected(&db, p));
}

/// `P1b-A2` — a not-yet-read row of 1 sat (its stored script is our P2PKH, so treat_check's
/// script and ownership checks pass) is not offered and is refused as a 1-sat coin.
#[test]
fn p1b_a2_a_not_observed_one_sat_coin_is_not_offered() {
    let db = wallet("p1b_a2");
    let ours = our_p2pkh(&db, 0);
    let txid = "fc".repeat(32);
    let id = raw_row(&db, &txid, 0, 1, Some(&ours), false, RECV, Some("0"), false);
    let h = held_entry(&db, &txid);
    assert_eq!((h.reason, h.can_treat_as_money), (Reason::NotObserved, false));
    assert_eq!(treat_as_money(&db, &txid, 0, true), Err(TreatRefusal::Refused(Reason::OneSat)));
    assert!(!change_of(&db, id) && !selected(&db, id));
}

/// `P1b-A4` + `P1b-A5` — a 1-sat coin already promoted (the mark and `change = 1`, as an
/// earlier dev build or a copied backup leaves it) is unstamped when it is classified
/// again, observed or not; a promoted 1-sat row restored at `change = 0` is not
/// re-promoted by the background pass; the same mark on a not-yet-read 5,000-sat coin is
/// still honoured (control). ⚠️ The tick (`stamp_pending`) reads `change = 0` rows only,
/// so a promoted coin already stamped stays money until an ingest route re-classifies it.
/// No public build ever offered the button (`classify.rs` is in no `v0.4.0-beta.*` tag).
#[test]
fn p1b_a4_an_already_promoted_one_sat_coin_is_unstamped() {
    let db = wallet("p1b_a4");
    let ours = our_p2pkh(&db, 0);
    let observed = raw_row(&db, &"fd".repeat(32), 0, 1, Some(&ours), true, RECV, Some("0"), true);
    let unread = raw_row(&db, &"fe".repeat(32), 0, 1, Some(&ours), false, RECV, Some("0"), true);
    let control = raw_row(&db, &"ff".repeat(32), 0, 5_000, Some(&ours), false, RECV, Some("0"), false);
    for id in [observed, unread, control] {
        db.connection().execute("UPDATE outputs SET output_description = ?1 WHERE outputId = ?2",
            rusqlite::params![format!("{} at 1; was rule2:value]", TREATED_MARK), id]).unwrap();
    }
    assert!(selected(&db, observed) && selected(&db, unread), "fixture: both start as money");
    stamp_outputs(&db, &[observed, unread], "test").unwrap(); // an ingest route re-classifies
    stamp_all(&db, "test"); // the tick: now `change = 0`, the override must not re-promote
    assert!(!change_of(&db, observed) && !selected(&db, observed), "an observed promoted 1-sat coin kept its stamp");
    assert!(!change_of(&db, unread) && !selected(&db, unread), "a not-yet-read promoted 1-sat coin kept its stamp");
    assert!(change_of(&db, control), "control: the mark on a multi-sat coin is honoured");
}

/// `P1b-A6` — the floor's edges and the reasons it must not loosen: a 0-sat row is refused
/// as 1-sat; a not-yet-read 2-sat row is still promoted (the floor is "more than one", not
/// "more than two" — RED: `> 2`); a key that is not ours and a non-template script stay
/// refused for their own reasons. ⚠️ The 0-sat line is defence in depth, not a control
/// of the floor: with the floor loosened to the SDK's literal `!= 1`, `treat_check`'s
/// stored-script check (rule ③) still refuses it with the same reason — observed.
#[test]
fn p1b_a6_floor_edges_and_other_refusals() {
    let db = wallet("p1b_a6");
    let ours = our_p2pkh(&db, 0);
    let theirs = p2pkh_for_key(&[0x42u8; 32]);
    let zero = "e0".repeat(32);
    let two = "e2".repeat(32);
    raw_row(&db, &zero, 0, 0, Some(&ours), false, RECV, Some("0"), false);
    let t = raw_row(&db, &two, 0, 2, Some(&ours), false, RECV, Some("0"), false);
    raw_row(&db, &"e3".repeat(32), 0, 5_000, Some(&theirs), true, RECV, Some("0"), false);
    raw_row(&db, &"e4".repeat(32), 0, 5_000, Some(&[vec![0x51, 0x75], ours.clone()].concat()), true, RECV, Some("0"), false);
    assert_eq!(treat_as_money(&db, &zero, 0, true), Err(TreatRefusal::Refused(Reason::OneSat)));
    assert!(held_entry(&db, &two).can_treat_as_money, "a 2-sat not-yet-read coin is offered");
    assert_eq!(treat_as_money(&db, &two, 0, true), Ok(t));
    assert_eq!(treat_as_money(&db, &"e3".repeat(32), 0, true), Err(TreatRefusal::Refused(Reason::NotOurs)));
    assert_eq!(treat_as_money(&db, &"e4".repeat(32), 0, true), Err(TreatRefusal::Refused(Reason::NotPlainPayment)));
}

/// `P1b-A8` (/code-review 176 #2) — the one class the user can still promote (not yet
/// read, > 1 sat) is guarded by `treat_check`'s stored-script check alone: a stored script
/// paying a key that is not ours, or not the exact template, is not offered and is refused
/// for that reason. RED: `treat_check`'s final match returns `Ok(())` ⇒ both are promoted.
#[test]
fn p1b_a8_a_not_observed_coin_is_promoted_only_if_its_stored_script_is_ours() {
    let db = wallet("p1b_a8");
    let ours = our_p2pkh(&db, 0);
    let theirs = p2pkh_for_key(&[0x42u8; 32]);
    let a = "d1".repeat(32);
    let b = "d2".repeat(32);
    let ia = raw_row(&db, &a, 0, 5_000, Some(&theirs), false, RECV, Some("0"), false);
    let ib = raw_row(&db, &b, 0, 5_000, Some(&[vec![0x51, 0x75], ours].concat()), false, RECV, Some("0"), false);
    for (txid, id, want) in [(&a, ia, Reason::NotOurs), (&b, ib, Reason::NotPlainPayment)] {
        let h = held_entry(&db, txid);
        assert_eq!((h.reason, h.can_treat_as_money), (Reason::NotObserved, false), "{:?}: offered", want);
        assert_eq!(treat_as_money(&db, txid, 0, true), Err(TreatRefusal::Refused(want)));
        assert!(!change_of(&db, id) && !selected(&db, id), "{:?}: promoted", want);
    }
}

/// `P1b-A9` (/code-review 176 #5) — the route acts only on rows the list shows: a
/// not-yet-read 5,000-sat coin to our key from an unsigned, failed or not-yet-final
/// transaction is neither listed nor promotable (`NotFound`); the same coin from a
/// completed transaction is (control). RED: drop `SHOWN_HELD` from `treat_as_money`'s read
/// ⇒ the failed transaction's output is stamped.
#[test]
fn p1b_a9_the_route_acts_only_on_rows_the_list_shows() {
    let db = wallet("p1b_a9");
    let ours = our_p2pkh(&db, 0);
    let c = db.connection();
    let mut rows = Vec::new();
    for (i, status) in ["failed", "unsigned", "nonfinal", "completed"].iter().enumerate() {
        let txid = format!("{:064x}", 0xb900 + i);
        c.execute("INSERT INTO transactions (txid, reference_number, raw_tx, status, is_outgoing, satoshis, created_at, updated_at)
                   VALUES (?1, ?2, x'00', ?3, 0, 5000, 0, 0)",
                  rusqlite::params![txid, uuid::Uuid::new_v4().to_string(), status]).unwrap();
        let tid = c.last_insert_rowid();
        let id = raw_row(&db, &txid, 0, 5_000, Some(&ours), false, RECV, Some("0"), false);
        c.execute("UPDATE outputs SET transaction_id = ?1 WHERE outputId = ?2", [tid, id]).unwrap();
        rows.push((status.to_string(), txid, id));
    }
    let listed: Vec<String> = list_held(&db, 1).unwrap().into_iter().filter_map(|h| h.txid).collect();
    for (status, txid, id) in &rows {
        if status == "completed" {
            assert!(listed.contains(txid), "control: the completed transaction's coin is listed");
            assert_eq!(treat_as_money(&db, txid, 0, true), Ok(*id), "control");
            assert!(change_of(&db, *id));
        } else {
            assert!(!listed.contains(txid), "{}: listed", status);
            assert_eq!(treat_as_money(&db, txid, 0, true), Err(TreatRefusal::NotFound), "{}", status);
            assert!(!change_of(&db, *id), "{}: the route stamped a row the list never offers", status);
        }
    }
}
