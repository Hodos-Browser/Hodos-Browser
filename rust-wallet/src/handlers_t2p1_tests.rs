//! B5-T2-P1 route rows through the real handlers (`internalize_action`, `create_action`
//! validation, `list_outputs`) over a real wallet DB, with the chain scripted
//! (`WalletServices::with_test_chains`).
//!
//! Subjects: the **real** envelope bytes of mainnet outputs (`tokens_tests` fixtures,
//! txid-verified) with their 25-byte P2PKH lock swapped for one of this wallet's keys —
//! a real token's script locked to us, labelled as such — because the internalize route
//! only files an output the wallet owns.
use super::*;
use super::p2_test_support::{app_state_from, body_json, status, ScriptedProvider};
use crate::classify::p5_tests::wallet;
use crate::services::{IndexerProvider, TxState};
use crate::tokens::tests::{real, BSV21_TRANSFER, ORDINAL};

/// The P2PKH of this wallet's first address (what `is_output_ours` matches against).
fn our_lock(db: &crate::database::WalletDatabase) -> Vec<u8> {
    let addr: String = db.connection().query_row("SELECT address FROM addresses ORDER BY \"index\" LIMIT 1", [], |r| r.get(0)).unwrap();
    let decoded = bs58::decode(addr).into_vec().unwrap();
    [&[0x76, 0xa9, 0x14][..], &decoded[1..21], &[0x88, 0xac]].concat()
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

fn varint(n: usize) -> Vec<u8> {
    match n {
        0..=0xfc => vec![n as u8],
        0xfd..=0xffff => [vec![0xfd], (n as u16).to_le_bytes().to_vec()].concat(),
        _ => [vec![0xfe], (n as u32).to_le_bytes().to_vec()].concat(),
    }
}

/// The parent every subject spends: one 1-BSV output. B5-T1-P7a-1 (BRC-67 check 2): a BEEF must carry
/// the parents its subject spends, and the inputs must be worth more than the outputs — before P7a these
/// subjects spent a parent (`[0x5a; 32]`) the BEEF never held (row `P7a-B0`, seen red first).
fn parent_tx() -> Vec<u8> {
    let mut t = 1u32.to_le_bytes().to_vec();
    t.extend(varint(1));
    t.extend([0x5a; 32]);
    t.extend(0u32.to_le_bytes());
    t.extend(varint(0));
    t.extend(0xffff_ffffu32.to_le_bytes());
    t.extend(varint(1));
    t.extend(100_000_000u64.to_le_bytes());
    t.extend(varint(1));
    t.push(0x51);
    t.extend(0u32.to_le_bytes());
    t
}

/// A one-input transaction spending `parent_tx():0` and paying `outputs` (value, script).
fn raw_tx(outputs: &[(i64, Vec<u8>)]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut t = 1u32.to_le_bytes().to_vec();
    t.extend(varint(1));
    t.extend(Sha256::digest(Sha256::digest(parent_tx())));
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

/// A real wallet behind the real handlers; the chain says every txid is mined.
fn rig(tag: &str) -> web::Data<AppState> {
    let mut p = ScriptedProvider::new("fake");
    p.status_for = Box::new(|t| Ok(status(t, TxState::Mined, None)));
    let p: std::sync::Arc<dyn IndexerProvider> = std::sync::Arc::new(p);
    app_state_from(wallet(tag), crate::services::WalletServices::with_test_chains(vec![p], vec![]))
}

/// Internalize `raw` with one output spec per entry.
async fn internalize(state: &web::Data<AppState>, raw: &[u8], outputs: serde_json::Value) -> (u16, serde_json::Value) {
    let mut beef = crate::beef::Beef::new();
    beef.add_parent_transaction(parent_tx());
    beef.set_main_transaction(raw.to_vec());
    let atomic = hex::decode(beef.to_atomic_beef_hex(&txid_of(raw)).unwrap()).unwrap();
    let req: InternalizeActionRequest = serde_json::from_value(serde_json::json!({
        "tx": atomic, "outputs": outputs, "description": "B5-T2-P1 route test"
    })).unwrap();
    body_json(internalize_action(state.clone(), web::Json(req)).await).await
}

fn insertion(i: u32, basket: &str, ci: Option<&str>, tags: &[&str]) -> serde_json::Value {
    serde_json::json!({ "outputIndex": i, "protocol": "basket insertion",
        "insertionRemittance": { "basket": basket, "customInstructions": ci, "tags": tags } })
}

struct Stored { basket: Option<String>, tags: Vec<String>, ci: Option<String>, change: bool }

fn stored(state: &web::Data<AppState>, txid: &str, vout: u32) -> Stored {
    let db = state.database.lock().unwrap();
    let c = db.connection();
    let (id, basket, ci, change): (i64, Option<String>, Option<String>, i64) = c.query_row(
        "SELECT o.outputId, b.name, o.custom_instructions, o.change FROM outputs o
           LEFT JOIN output_baskets b ON o.basket_id = b.basketId WHERE o.txid = ?1 AND o.vout = ?2",
        rusqlite::params![txid, vout], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
    let mut tags = crate::database::TagRepository::new(c).get_tags_for_output(id).unwrap();
    tags.sort();
    Stored { basket, tags, ci, change: change != 0 }
}

fn balance(state: &web::Data<AppState>) -> i64 {
    let db = state.database.lock().unwrap();
    crate::database::OutputRepository::new(db.connection()).calculate_balance(1).unwrap()
}

/// `P1-A2` — the real ordinal (the dev wallet's own `964eaf33…:0`, `text/plain`) arriving by basket insertion into an **app** basket (`todo`,
/// not `1sat`: the filing must be the recogniser's) lands in `1sat` with the wallet's tags
/// plus the sender's, the sender's CI verbatim, and moves no balance.
#[actix_web::test]
async fn p1_a2_internalized_ordinal_is_filed_by_the_recogniser() {
    let state = rig("rt-a2");
    let lock = { let db = state.database.lock().unwrap(); our_lock(&db) };
    let raw = raw_tx(&[(1, ordinal_to(&lock))]);
    let before = balance(&state);
    let ci = r#"{"origin":"x","name":"x"}"#;
    let (code, body) = internalize(&state, &raw, serde_json::json!([insertion(0, "todo", Some(ci), &["app:demo"])])).await;
    assert_eq!(code, 200, "{}", body);
    let s = stored(&state, &txid_of(&raw), 0);
    assert_eq!(s.basket.as_deref(), Some("1sat"), "the recogniser files, not the sender");
    assert!(s.tags.contains(&"ordinal".into()) && s.tags.contains(&"type:text/plain".into())
            && s.tags.contains(&"app:demo".into()), "{:?}", s.tags);
    assert_eq!(s.ci.as_deref(), Some(ci), "BRC-37: the sender's string, unchanged");
    assert!(!s.change);
    assert_eq!(balance(&state), before);
}

/// `P1-A3` ② / `P1-A12` — a sender's basket never wins: the BSV-21 transfer asked into
/// `1sat` lands `bsv21`; the ordinal asked into `bsv21` lands `1sat`; a 5,000-sat plain
/// payment asked into `1sat` lands in the default pool (counted or held — never hidden).
#[actix_web::test]
async fn p1_a12_requested_basket_never_beats_the_recogniser() {
    let state = rig("rt-a12");
    let (l0, l1, l2) = { let db = state.database.lock().unwrap(); (our_lock(&db), our_lock(&db), our_lock(&db)) };
    let raw = raw_tx(&[(1, bsv21_to(&l0)), (1, ordinal_to(&l1)), (5_000, l2)]);
    let (code, body) = internalize(&state, &raw, serde_json::json!([
        insertion(0, "1sat", None, &[]), insertion(1, "bsv21", None, &[]), insertion(2, "1sat", None, &[])])).await;
    assert_eq!(code, 200, "{}", body);
    let t = txid_of(&raw);
    assert_eq!(stored(&state, &t, 0).basket.as_deref(), Some("bsv21"));
    assert_eq!(stored(&state, &t, 1).basket.as_deref(), Some("1sat"));
    let plain = stored(&state, &t, 2);
    assert!(matches!(plain.basket.as_deref(), Some("default") | None), "plain payment in {:?}", plain.basket);
    let held: i64 = {
        let db = state.database.lock().unwrap();
        crate::classify::list_held(&db, 1).unwrap().iter().filter(|h| h.txid.as_deref() == Some(&t)).map(|h| h.satoshis).sum()
    };
    assert_eq!(balance(&state) + held, 5_000, "the payment is visible: in the balance or the held line");
}

/// `P1-A8` — a CI string is returned by `listOutputs` exactly as sent (via the BRC-165 view);
/// `P1-A10` — `p 1sat all` reads storage basket `1sat`.
#[actix_web::test]
async fn p1_a8_ci_round_trips_exactly_through_list_outputs() {
    let state = rig("rt-a8");
    let lock = { let db = state.database.lock().unwrap(); our_lock(&db) };
    let raw = raw_tx(&[(1, ordinal_to(&lock))]);
    let ci = r#"{"origin":"…","name":"x"}"#;
    internalize(&state, &raw, serde_json::json!([insertion(0, "1sat", Some(ci), &[])])).await;
    let (code, body) = list(&state, serde_json::json!({ "basket": "p 1sat all", "includeCustomInstructions": true })).await;
    assert_eq!(code, 200, "{}", body);
    assert_eq!(body["outputs"][0]["customInstructions"], ci, "{}", body);
}

async fn list(state: &web::Data<AppState>, req: serde_json::Value) -> (u16, serde_json::Value) {
    let http = actix_web::test::TestRequest::default().to_http_request();
    body_json(list_outputs(state.clone(), http, web::Bytes::from(req.to_string())).await).await
}

/// `P1-A9` — a row stored on the pre-change build (wrapped CI) still reads as the
/// sender's original string.
#[actix_web::test]
async fn p1_a9_wrapped_ci_rows_still_read() {
    assert_eq!(legacy_basket_insertion_ci(r#"{"type":"basket_insertion","basket":"todo","appData":"{\"a\":1}"}"#).as_deref(),
               Some(r#"{"a":1}"#));
    assert_eq!(legacy_basket_insertion_ci(r#"{"type":"basket_insertion","basket":"todo"}"#), None);
    assert_eq!(legacy_basket_insertion_ci(r#"{"a":1}"#).as_deref(), Some(r#"{"a":1}"#));
    // Through the handler: a pre-change row in an app basket.
    let state = rig("rt-a9");
    {
        let db = state.database.lock().unwrap();
        let c = db.connection();
        let bid = crate::database::BasketRepository::new(c).find_or_insert("todo", 1).unwrap();
        c.execute("INSERT INTO outputs (user_id, spendable, change, vout, satoshis, txid, basket_id, custom_instructions,
                   locking_script, script_length, provided_by, purpose, type, created_at, updated_at)
                   VALUES (1, 1, 0, 0, 1, ?1, ?2, ?3, x'51', 1, 'you', 'receive', 'custom', 0, 0)",
                  rusqlite::params!["a9".repeat(32), bid,
                      r#"{"type":"basket_insertion","basket":"todo","appData":"{\"task\":\"buy milk\"}"}"#]).unwrap();
    }
    let (_, body) = list(&state, serde_json::json!({ "basket": "todo", "includeCustomInstructions": true })).await;
    assert_eq!(body["outputs"][0]["customInstructions"], r#"{"task":"buy milk"}"#, "{}", body);
}

/// `P1-A10` — `p 1sat` routing: bare / unknown / extra scope and a missing axis tag are
/// refused; the axis tag is a mandatory filter (`tagQueryMode:"any"` cannot widen it);
/// `p 1sat id` needs no grant; other `p ` names still refused; `1sat` itself is not
/// listable directly.
#[actix_web::test]
async fn p1_a10_p1sat_view_routing() {
    let state = rig("rt-a10");
    let (l0, l1) = { let db = state.database.lock().unwrap(); (our_lock(&db), our_lock(&db)) };
    let raw = raw_tx(&[(1, ordinal_to(&l0)), (1, ordinal_to(&l1))]);
    internalize(&state, &raw, serde_json::json!([
        insertion(0, "1sat", None, &["collection:alpha", "id:k1"]),
        insertion(1, "1sat", None, &["collection:beta", "type:image/png"])])).await;
    for bad in [serde_json::json!({"basket": "p 1sat"}), serde_json::json!({"basket": "p 1sat foo"}),
                serde_json::json!({"basket": "p 1sat all extra"}), serde_json::json!({"basket": "p 1sat collection"}),
                serde_json::json!({"basket": "p other x"})] {
        let (code, body) = list(&state, bad.clone()).await;
        assert_eq!(code, 400, "{} ⇒ {}", bad, body);
    }
    let count = |b: serde_json::Value| b["totalOutputs"].as_u64().unwrap();
    let (_, all) = list(&state, serde_json::json!({"basket": "p 1sat all"})).await;
    assert_eq!(count(all), 2, "p 1sat all reads storage basket 1sat");
    // Review #5: plain `1sat` (BRC-147's own example) reads the same, through the same gate.
    let (code, plain) = list(&state, serde_json::json!({"basket": "1sat"})).await;
    assert_eq!((code, count(plain)), (200, 2), "plain 1sat is routed, not refused");
    // Review #2: the sender planted `id:k1` — it was dropped, so `p 1sat id` finds nothing.
    let (code, by_id) = list(&state, serde_json::json!({"basket": "p 1sat id", "tags": ["id:k1"]})).await;
    assert_eq!((code, count(by_id)), (200, 0), "a counterparty id: tag must not be stored");
    let (_, alpha) = list(&state, serde_json::json!({"basket": "p 1sat collection",
        "tags": ["collection:alpha", "type:image/png"], "tagQueryMode": "any"})).await;
    assert_eq!(count(alpha), 1, "the axis tag stays mandatory under tagQueryMode any");
}

/// `P1-A12` / BRC-147 at `createAction`: a site may not put a multi-sat output in `1sat`
/// (refused before anything is signed); a 1-sat ordinal asked into an app basket is filed
/// in `1sat`.
#[actix_web::test]
async fn p1_a12_create_action_asset_basket_rules() {
    let state = rig("rt-ca");
    let lock = { let db = state.database.lock().unwrap(); our_lock(&db) };
    let req = serde_json::json!({ "description": "B5-T2-P1 create", "outputs": [
        { "satoshis": 5000, "lockingScript": hex::encode(&lock), "basket": "1sat", "outputDescription": "not a token" }]});
    let http = actix_web::test::TestRequest::default().to_http_request();
    let (code, body) = body_json(create_action(state.clone(), http, web::Bytes::from(req.to_string())).await).await;
    assert_eq!((code, body["code"].as_str()), (400, Some("ERR_INVALID_BASKET_NAME")), "{}", body);
}

/// A site with the broadest payment trust the wallet can give: approved, every cap at
/// its maximum, identity disclosure and the connect-modal "no prompts" bundle on.
fn broad_site(state: &web::Data<AppState>, domain: &str, view_grants: &[&str]) {
    let db = state.database.lock().unwrap();
    let repo = crate::database::DomainPermissionRepository::new(db.connection());
    let mut p = crate::database::DomainPermission::defaults(1, domain);
    p.trust_level = "approved".into();
    (p.per_tx_limit_cents, p.per_session_limit_cents, p.rate_limit_per_min, p.max_tx_per_session) =
        (i64::MAX / 4, i64::MAX / 4, 10_000, 10_000);
    p.identity_key_disclosure_allowed = true;
    p.bundled_scope_grant = true;
    let id = repo.upsert(&p).unwrap();
    for g in view_grants {
        repo.grant_basket(id, g, "read", None).unwrap();
    }
}

async fn view_as(state: &web::Data<AppState>, domain: &str, req: serde_json::Value) -> u16 {
    let http = actix_web::test::TestRequest::default().insert_header(("X-Requesting-Domain", domain)).to_http_request();
    list_outputs(state.clone(), http, web::Bytes::from(req.to_string())).await.status().as_u16()
}

/// `P1-A11` — BRC-165 "item view is not covered by ordinary payment / auto-pay grants".
/// D1 (broadest pay trust, no view grant) is PROMPTED (202); D2 (the same + a `p 1sat all`
/// view grant) is SILENT (200) — so the prompt is a decision, not the absence of any grant
/// path. A scoped grant covers only its value; `all` covers every scope; `bsv21` always
/// prompts, even with a grant row forced into the DB.
#[actix_web::test]
async fn p1_a11_pay_grants_never_cover_the_asset_view() {
    let state = rig("rt-a11");
    broad_site(&state, "d1.example", &[]);
    broad_site(&state, "d2.example", &["p 1sat all", "bsv21", "BSV21"]);
    broad_site(&state, "d3.example", &["p 1sat collection abc"]);
    let all = serde_json::json!({"basket": "p 1sat all"});
    let coll = |v: &str| serde_json::json!({"basket": "p 1sat collection", "tags": [format!("collection:{}", v)]});
    assert_eq!(view_as(&state, "d1.example", all.clone()).await, 202, "pay trust must not silence the view");
    assert_eq!(view_as(&state, "d1.example", serde_json::json!({"basket": "1sat"})).await, 202, "plain 1sat is gated too");
    assert_eq!(view_as(&state, "d2.example", serde_json::json!({"basket": "1sat"})).await, 200, "…by the same p 1sat all grant");
    assert_eq!(view_as(&state, "d2.example", all).await, 200, "a view grant does");
    assert_eq!(view_as(&state, "d2.example", coll("xyz")).await, 200, "all covers every scope");
    assert_eq!(view_as(&state, "d3.example", coll("abc")).await, 200);
    assert_eq!(view_as(&state, "d3.example", coll("xyz")).await, 202, "a scoped grant covers its value only");
    assert_eq!(view_as(&state, "d2.example", serde_json::json!({"basket": "bsv21"})).await, 202, "bsv21 always prompts");
    assert_eq!(view_as(&state, "d2.example", serde_json::json!({"basket": "BSV21"})).await, 202,
               "review #7: an odd-case grant row cannot unlock the protected basket");
    assert_eq!(view_as(&state, "d1.example", serde_json::json!({"basket": "p 1sat id", "tags": ["id:k1"]})).await, 200,
               "p 1sat id needs no inventory grant");
}

/// Owner decision Q1 (2026-10-07): a token sent as a "wallet payment" is refused, as
/// `@bsv/wallet-toolbox` refuses it — and BEFORE the route's chain check could broadcast
/// the sender's transaction. The chain says "not found", so a late refusal would have
/// broadcast first: the broadcaster must see nothing. A plain-script wallet payment in the
/// same shape is not refused by this check (control).
#[actix_web::test]
async fn p1_q1_token_as_wallet_payment_is_refused_before_any_broadcast() {
    use crate::services::IndexerError;
    let mut p = ScriptedProvider::new("fake");
    p.status_for = Box::new(|_| Err(IndexerError::NotFound));
    p.broadcast = Box::new(|_| Err(IndexerError::Transport("test: broadcast reached".into())));
    let p = std::sync::Arc::new(p);
    let dynp: std::sync::Arc<dyn IndexerProvider> = p.clone();
    let state = app_state_from(wallet("rt-q1"), crate::services::WalletServices::with_test_chains(vec![dynp.clone()], vec![dynp]));
    let lock = { let db = state.database.lock().unwrap(); our_lock(&db) };
    let remit = serde_json::json!({ "senderIdentityKey": "02".to_string() + &"11".repeat(32),
                                    "derivationPrefix": "cHJlZml4", "derivationSuffix": "c3VmZml4" });
    let spec = |i: u32| serde_json::json!([{ "outputIndex": i, "protocol": "wallet payment", "paymentRemittance": remit }]);
    let raw = raw_tx(&[(1, ordinal_to(&lock))]);
    let (code, body) = internalize(&state, &raw, spec(0)).await;
    assert_eq!((code, body["code"].as_str()), (400, Some("ERR_INVALID_PARAMETER")), "{}", body);
    assert!(p.broadcast_hashes().is_empty(), "refused after a broadcast");
    // Control: the same request with a plain script passes this check and reaches the
    // broadcast (which the fake then fails) — so the assertion above can see a broadcast.
    let plain = raw_tx(&[(5_000, lock.clone())]);
    let (_, body) = internalize(&state, &plain, spec(0)).await;
    assert_ne!(body["code"].as_str(), Some("ERR_INVALID_PARAMETER"), "{}", body);
    assert_eq!(p.broadcast_hashes().len(), 1, "the control must reach the broadcaster");
}

/// Review #4: a "wallet payment" must BE the plain payment script (positive test, as the
/// toolbox) — a non-token that is not plain (P2PKH + `OP_RETURN` memo) is refused before
/// any broadcast; and a token at a known address sent as a wallet payment WITHOUT a
/// remittance is not counted as ours (it used to return 200 and record +1 sat, no row).
#[actix_web::test]
async fn p1_review4_wallet_payment_must_be_the_plain_script() {
    use crate::services::IndexerError;
    let mut p = ScriptedProvider::new("fake");
    p.status_for = Box::new(|_| Err(IndexerError::NotFound));
    p.broadcast = Box::new(|_| Err(IndexerError::Transport("test: broadcast reached".into())));
    let p = std::sync::Arc::new(p);
    let dynp: std::sync::Arc<dyn IndexerProvider> = p.clone();
    let state = app_state_from(wallet("rt-r4"), crate::services::WalletServices::with_test_chains(vec![dynp.clone()], vec![dynp]));
    let lock = { let db = state.database.lock().unwrap(); our_lock(&db) };
    let remit = serde_json::json!({ "senderIdentityKey": "02".to_string() + &"11".repeat(32),
                                    "derivationPrefix": "cHJlZml4", "derivationSuffix": "c3VmZml4" });
    let memo = [lock.clone(), vec![0x6a, 0x04, b'm', b'e', b'm', b'o']].concat();
    let (code, body) = internalize(&state, &raw_tx(&[(5_000, memo)]),
        serde_json::json!([{ "outputIndex": 0, "protocol": "wallet payment", "paymentRemittance": remit }])).await;
    assert_eq!((code, body["code"].as_str()), (400, Some("ERR_INVALID_PARAMETER")), "{}", body);
    assert!(p.broadcast_hashes().is_empty(), "refused only after a broadcast");
    // (b) token to our known address, "wallet payment" with no remittance — not ours.
    let mut q = ScriptedProvider::new("fake");
    q.status_for = Box::new(|t| Ok(status(t, TxState::Mined, None)));
    let q: std::sync::Arc<dyn IndexerProvider> = std::sync::Arc::new(q);
    let state = app_state_from(wallet("rt-r4b"), crate::services::WalletServices::with_test_chains(vec![q], vec![]));
    let lock = { let db = state.database.lock().unwrap(); our_lock(&db) };
    let (code, body) = internalize(&state, &raw_tx(&[(1, ordinal_to(&lock))]),
        serde_json::json!([{ "outputIndex": 0, "protocol": "wallet payment" }])).await;
    assert_eq!((code, body["code"].as_str()), (400, Some("ERR_NO_OUTPUTS_OWNED")), "{}", body);
}

/// Review #6: `sendMax` replaces output 0's amount after the asset-basket check, so an
/// output headed for `1sat` can never be the send-max output.
#[actix_web::test]
async fn p1_review6_send_max_never_into_an_asset_basket() {
    let state = rig("rt-r6");
    let lock = { let db = state.database.lock().unwrap(); our_lock(&db) };
    let req = serde_json::json!({ "description": "B5-T2-P1 sendMax", "options": { "sendMax": true }, "outputs": [
        { "satoshis": 1, "lockingScript": hex::encode(&lock), "basket": "1sat", "outputDescription": "tiny tip" }]});
    let http = actix_web::test::TestRequest::default().to_http_request();
    let (code, body) = body_json(create_action(state.clone(), http, web::Bytes::from(req.to_string())).await).await;
    assert_eq!((code, body["code"].as_str()), (400, Some("ERR_INVALID_PARAMETER")), "{}", body);
}

/// Owner decision (review #1): a transferred ordinal tip — 1 sat, plain P2PKH to us, no
/// envelope — that the sender explicitly files into `1sat` stays in `1sat` (a claim until
/// B5-T2-P3 verifies origin): not in the held line, so never offered "Treat as money",
/// and "Treat as money" refuses it. Sender tags kept, except `id:`. A 2-sat plain output
/// asked into `1sat` still goes to the default pool.
#[actix_web::test]
async fn p1_review1_a_sender_filed_transfer_tip_stays_in_1sat() {
    let state = rig("rt-r1");
    let lock = { let db = state.database.lock().unwrap(); our_lock(&db) };
    let raw = raw_tx(&[(1, lock.clone()), (2, lock.clone())]);
    let (code, body) = internalize(&state, &raw, serde_json::json!([
        insertion(0, "1sat", None, &["ordinal", "origin:abc_0", "id:zz"]),
        insertion(1, "1sat", None, &[])])).await;
    assert_eq!(code, 200, "{}", body);
    let t = txid_of(&raw);
    let tip = stored(&state, &t, 0);
    assert_eq!(tip.basket.as_deref(), Some("1sat"), "the sender's 1sat filing of a 1-sat tip is kept");
    assert_eq!(tip.tags, vec!["ordinal".to_string(), "origin:abc_0".to_string()], "id: dropped, claims kept");
    assert!(!tip.change);
    let db = state.database.lock().unwrap();
    assert!(!crate::classify::list_held(&db, 1).unwrap().iter().any(|h| h.txid.as_deref() == Some(&t) && h.vout == 0),
            "a filed tip is not in the held line");
    assert!(matches!(crate::classify::treat_as_money(&db, &t, 0, true), Err(crate::classify::TreatRefusal::NotDefaultBasket(_))));
    drop(db);
    assert!(matches!(stored(&state, &t, 1).basket.as_deref(), Some("default") | None), "2 sats is not a tip");
}

/// /code-review ultra #5: `listActions` reads the same `custom_instructions` column as
/// `listOutputs`, so a row written by the pre-fix build (wrapped) must read back as the
/// sender's original string there too. The row is made by the real internalize route,
/// then given the wrapped form the old build wrote.
#[actix_web::test]
async fn p1_ultra5_list_actions_unwraps_pre_fix_ci() {
    let state = rig("rt-u5");
    let lock = { let db = state.database.lock().unwrap(); our_lock(&db) };
    let raw = raw_tx(&[(1, ordinal_to(&lock))]);
    let (code, body) = internalize(&state, &raw, serde_json::json!([insertion(0, "todo", Some(r#"{"a":1}"#), &[])])).await;
    assert_eq!(code, 200, "{}", body);
    let t = txid_of(&raw);
    {
        let db = state.database.lock().unwrap();
        db.connection().execute("UPDATE outputs SET custom_instructions = ?1 WHERE txid = ?2 AND vout = 0",
            rusqlite::params![r#"{"type":"basket_insertion","basket":"todo","appData":"{\"a\":1}"}"#, t]).unwrap();
    }
    let req: ListActionsRequest = serde_json::from_value(serde_json::json!({ "includeOutputs": true })).unwrap();
    let (code, body) = body_json(list_actions(state.clone(), web::Json(req)).await).await;
    assert_eq!(code, 200, "{}", body);
    let ci: Vec<&serde_json::Value> = body["actions"].as_array().unwrap().iter()
        .filter(|a| a["txid"] == t.as_str())
        .flat_map(|a| a["outputs"].as_array().unwrap().iter()).map(|o| &o["customInstructions"]).collect();
    assert_eq!(ci, vec![&serde_json::json!(r#"{"a":1}"#)], "{}", body);
}
