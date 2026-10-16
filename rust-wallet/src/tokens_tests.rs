//! B5-T2-P1 `P1-A1` / `P1-X1` — the recognisers over **real chain scripts**.
//!
//! Fixtures are whole raw transactions (`tests/fixtures/tokens/<txid>.hex`, fetched from
//! WhatsOnChain 2026-10-06). Each test first proves the bytes hash to the txid
//! (`reconcile::verify_raw_txid`), then reads the output with the production parser
//! (`reconcile::parse_tx_outputs`) — the subject is the chain's script, not hand-typed hex.
//! Shapes with no mainnet example (deploy+auth, Mandala, malformed JSON) are hand-built
//! and named `hand_built_*`.

use super::*;

/// `(satoshis, script)` of a real output, after checking the bytes are that transaction.
pub(crate) fn real(txid: &str, vout: usize) -> (i64, Vec<u8>) {
    let path = format!("{}/tests/fixtures/tokens/{}.hex", env!("CARGO_MANIFEST_DIR"), txid);
    let hex_text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {}: {}", path, e));
    let raw = hex::decode(hex_text.trim()).expect("fixture hex");
    assert!(crate::reconcile::verify_raw_txid(&raw, txid), "fixture {} does not hash to its txid", txid);
    let (sats, script) = crate::reconcile::parse_tx_outputs(&raw).expect("parse").swap_remove(vout);
    // Right reason (`P1-A1`): say what was measured; a real token script is never 25 B.
    eprintln!("fixture {}:{} — {} sats, script {} B, first 16: {}", &txid[..8], vout, sats,
              script.len(), hex::encode(&script[..16.min(script.len())]));
    assert_ne!(script.len(), 25, "fixture {}:{} is a bare P2PKH, not a token", txid, vout);
    (sats, script)
}

pub(crate) const ORDINAL: (&str, usize) = ("964eaf33fd1d6dad03b561d5997ccf376a80d58a5cc1832ab611b65b5cedc83c", 0);
pub(crate) const BSV21_TRANSFER: (&str, usize) = ("17d32ab1932f7c542f76837fedb1b5caf3080707e27d449e01b1c92db51f0cc8", 1);
pub(crate) const BSV21_DEPLOY_MINT: (&str, usize) = ("62b594eb92036ba9b4193961746c1e3af19de7b694c1ba614a251953fc445e65", 0);
pub(crate) const BSV20_TICK_TRANSFER: (&str, usize) = ("4cc3423b6405ee72123de9a4bc8619615b4c2df9effebf0b9f6b8f6d842c9ab3", 0);
pub(crate) const BSV20_TICK_DEPLOY: (&str, usize) = ("0b92e58fd221b38e63156eda68ae28e0f5d41c28102689f062b5dc3152589197", 0);

pub(crate) fn push(d: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    match d.len() {
        0 => v.push(0x00),
        n if n < 0x4c => v.push(n as u8),
        n if n <= 0xff => { v.push(0x4c); v.push(n as u8); }
        n => { v.push(0x4d); v.extend_from_slice(&(n as u16).to_le_bytes()); }
    }
    v.extend_from_slice(d);
    v
}

pub(crate) fn p2pkh(hash: [u8; 20]) -> Vec<u8> {
    let mut v = vec![0x76, 0xa9, 0x14];
    v.extend_from_slice(&hash);
    v.extend_from_slice(&[0x88, 0xac]);
    v
}

/// BRC-160 envelope, `OP_FALSE OP_IF "ord" OP_1 <ct> OP_0 <body> OP_ENDIF`.
pub(crate) fn envelope(ct: &str, body: &[u8]) -> Vec<u8> {
    let mut v = vec![0x00, 0x63];
    v.extend(push(b"ord"));
    v.push(0x51);
    v.extend(push(ct.as_bytes()));
    v.push(0x00);
    v.extend(push(body));
    v.push(0x68);
    v
}

/// Hand-built: a BSV-21 JSON inscription after a P2PKH (the real deploy+mint's layout).
pub(crate) fn hand_built_bsv20(json: &str) -> Vec<u8> {
    let mut s = p2pkh([7; 20]);
    s.extend(envelope("application/bsv-20", json.as_bytes()));
    s
}

/// Hand-built: BRC-162 Mandala transfer, `<32-byte id> <amount> OP_2DROP <P2PKH>`.
pub(crate) fn hand_built_mandala() -> Vec<u8> {
    let mut s = push(&[0xab; 32]);
    s.extend(push(&[0x88, 0x13])); // 5000, minimal script number
    s.push(0x6d);
    s.extend(p2pkh([7; 20]));
    s
}

const TID: &str = "6a0a5f91ea03cdd4d7371b4ee0572dadb349588cb3b0bfc95fba0cd2a1987488_0";

#[test]
fn p1_a1_real_ordinal_is_an_ordinal() {
    let (sats, s) = real(ORDINAL.0, ORDINAL.1);
    assert_eq!(sats, 1);
    assert!(matches!(recognise(&s), Recognised::Ordinal { ref content_type } if !content_type.is_empty()),
            "got {:?}", recognise(&s));
    assert!(matches!(verdict(&s, sats), Verdict::File(Filing::Ordinal { .. })));
}

#[test]
fn p1_a1_real_bsv21_transfer_is_value_with_the_inscriptions_id() {
    let (sats, s) = real(BSV21_TRANSFER.0, BSV21_TRANSFER.1);
    assert_eq!(sats, 1);
    match recognise(&s) {
        Recognised::Bsv21Value(b) => {
            assert_eq!(b.op, "transfer");
            assert_eq!(b.id.as_deref(), Some(TID));
            assert_eq!(b.amt, "100000000");
        }
        other => panic!("expected BSV-21 value, got {:?}", other),
    }
}

#[test]
fn p1_a1_real_bsv21_deploy_mint_is_value_with_its_own_outpoint_as_id() {
    let (sats, s) = real(BSV21_DEPLOY_MINT.0, BSV21_DEPLOY_MINT.1);
    let Verdict::File(f @ Filing::Bsv21(_)) = verdict(&s, sats) else { panic!("{:?}", recognise(&s)) };
    let id = format!("{}_0", BSV21_DEPLOY_MINT.0);
    assert_eq!(f.tags(BSV21_DEPLOY_MINT.0, 0), vec![format!("bsv21:{}", id), "bsv21:deploy".to_string()]);
    let ci: serde_json::Value = serde_json::from_str(&f.custom_instructions(BSV21_DEPLOY_MINT.0, 0).unwrap()).unwrap();
    assert_eq!(ci["id"], id.as_str());
    assert_eq!(ci["op"], "deploy+mint");
    // A relative icon (`_1`) names an output of the deploy's own transaction.
    assert_eq!(ci["icon"], format!("{}_1", BSV21_DEPLOY_MINT.0).as_str());
}

#[test]
fn p1_a1_real_legacy_bsv20_is_held() {
    for (t, v) in [BSV20_TICK_TRANSFER, BSV20_TICK_DEPLOY] {
        let (sats, s) = real(t, v);
        assert_eq!(recognise(&s), Recognised::Held(HeldKind::Bsv20Tick), "{}:{}", t, v);
        assert_eq!(verdict(&s, sats), Verdict::Held("bsv20-tick"));
    }
}

/// `P1-A1` switch ⑤ — the BSV-21 transfer cut to 64 B (T1-P4's rejected prefix option)
/// must not read as value: its JSON is gone.
#[test]
fn p1_a1_truncated_transfer_is_not_value() {
    let (_, s) = real(BSV21_TRANSFER.0, BSV21_TRANSFER.1);
    let r = recognise(&s[..64]);
    assert!(!matches!(r, Recognised::Bsv21Value(_)), "64-byte prefix read as value: {:?}", r);
}

#[test]
fn p1_a4_hand_built_non_value_ops_are_held() {
    for op in ["deploy+auth", "auth", "burn"] {
        let json = match op {
            "deploy+auth" => r#"{"p":"bsv-20","op":"deploy+auth","sym":"X","dec":"2"}"#.to_string(),
            "auth" => format!(r#"{{"p":"bsv-20","op":"auth","id":"{}"}}"#, TID),
            _ => format!(r#"{{"p":"bsv-20","op":"burn","id":"{}","amt":"5"}}"#, TID),
        };
        assert_eq!(recognise(&hand_built_bsv20(&json)), Recognised::Held(HeldKind::Bsv21NonValue), "{}", op);
    }
}

#[test]
fn p1_a4_hand_built_mandala_is_held_at_any_value() {
    let s = hand_built_mandala();
    assert_eq!(recognise(&s), Recognised::Held(HeldKind::Mandala));
    for sats in [1, 2, 546, 5_000] {
        assert_eq!(verdict(&s, sats), Verdict::Held("mandala"), "{} sats", sats);
    }
    // A deploy: `OP_0 OP_0 OP_2DROP <lock>`.
    let mut deploy = vec![0x00, 0x00, 0x6d];
    deploy.extend(p2pkh([7; 20]));
    assert_eq!(recognise(&deploy), Recognised::Held(HeldKind::Mandala));
}

#[test]
fn p1_a4_envelope_on_multi_sat_is_held_not_filed() {
    let (_, s) = real(ORDINAL.0, ORDINAL.1);
    assert_eq!(verdict(&s, 546), Verdict::Held("envelope-on-multi-sat"));
    let (_, t) = real(BSV21_TRANSFER.0, BSV21_TRANSFER.1);
    assert_eq!(verdict(&t, 2), Verdict::Held("envelope-on-multi-sat"));
}

#[test]
fn p1_a5_plain_p2pkh_is_not_a_token() {
    assert_eq!(recognise(&p2pkh([9; 20])), Recognised::None);
    assert_eq!(verdict(&p2pkh([9; 20]), 1), Verdict::NotToken);
}

/// `P1-X1` — non-canonical BRC-161 JSON is held, never filed with an amount the token's
/// own ledger may not hold.
#[test]
fn p1_x1_non_canonical_json_is_held() {
    let cases = [
        ("duplicate amt", format!(r#"{{"p":"bsv-20","op":"transfer","id":"{TID}","amt":"1","amt":"1000000"}}"#)),
        ("numeric amt", format!(r#"{{"p":"bsv-20","op":"transfer","id":"{TID}","amt":5}}"#)),
        ("leading zero", format!(r#"{{"p":"bsv-20","op":"transfer","id":"{TID}","amt":"05"}}"#)),
        ("over u64", format!(r#"{{"p":"bsv-20","op":"transfer","id":"{TID}","amt":"18446744073709551616"}}"#)),
        ("negative", format!(r#"{{"p":"bsv-20","op":"transfer","id":"{TID}","amt":"-5"}}"#)),
        ("dot id", format!(r#"{{"p":"bsv-20","op":"transfer","id":"{}","amt":"5"}}"#, TID.replace('_', "."))),
        ("upper id", format!(r#"{{"p":"bsv-20","op":"transfer","id":"{}","amt":"5"}}"#, TID.to_uppercase())),
        ("missing id", r#"{"p":"bsv-20","op":"transfer","amt":"5"}"#.to_string()),
        ("unknown op", format!(r#"{{"p":"bsv-20","op":"mint2","id":"{TID}","amt":"5"}}"#)),
        ("wrong p", format!(r#"{{"p":"bsv-21","op":"transfer","id":"{TID}","amt":"5"}}"#)),
        ("not json", "transfer 5".to_string()),
    ];
    for (name, json) in cases {
        assert_eq!(recognise(&hand_built_bsv20(&json)), Recognised::Held(HeldKind::Bsv21Malformed), "{}", name);
    }
    // The canonical form of the same transfer is value — the cases above fail on their defect.
    let ok = format!(r#"{{"p":"bsv-20","op":"transfer","id":"{TID}","amt":"5"}}"#);
    assert!(matches!(recognise(&hand_built_bsv20(&ok)), Recognised::Bsv21Value(_)));
}

#[test]
fn p1_a1_unreadable_and_bad_envelope_are_not_none() {
    // A push that runs past the end of the script.
    assert_eq!(recognise(&[0x76, 0xa9, 0x14, 0x01, 0x02]), Recognised::Unreadable);
    assert_eq!(verdict(&[0x76, 0xa9, 0x14, 0x01, 0x02], 5_000), Verdict::Unreadable);
    // An envelope with no body before OP_ENDIF.
    let mut s = p2pkh([7; 20]);
    s.extend([0x00, 0x63]);
    s.extend(push(b"ord"));
    s.extend([0x51]);
    s.extend(push(b"text/plain"));
    s.push(0x68);
    assert_eq!(recognise(&s), Recognised::Held(HeldKind::BadEnvelope));
}

/// BRC-160: field tags as 1-byte pushes are aliases; the last occurrence wins.
#[test]
fn p1_a1_field_tag_aliases_and_last_occurrence() {
    let mut s = vec![0x00, 0x63];
    s.extend(push(b"ord"));
    s.extend(push(&[0x01]));
    s.extend(push(b"text/plain"));
    s.extend(push(&[0x01]));
    s.extend(push(b"image/png; charset=binary"));
    s.extend(push(&[0x00]));
    s.extend(push(b"x"));
    s.push(0x68);
    s.extend(p2pkh([7; 20]));
    assert_eq!(recognise(&s), Recognised::Ordinal { content_type: "image/png".into() });
}

/// The contract's named subject `7faac48b…:0` (2,596,810 B, `image/png`) — fetched live,
/// so ignored by default: `cargo test -- --ignored p1_a1_live_7faac48b`.
#[test]
#[ignore]
fn p1_a1_live_7faac48b() {
    let txid = "7faac48b03b04d5c84a7bf4002ca958e90e60a50d5e996eaf760fdcf70506473";
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/hex", txid);
    let hex_text = tokio::runtime::Runtime::new().unwrap()
        .block_on(async { reqwest::get(url).await?.text().await }).expect("fetch");
    let raw = hex::decode(hex_text.trim()).expect("hex");
    assert!(crate::reconcile::verify_raw_txid(&raw, txid));
    let (sats, s) = crate::reconcile::parse_tx_outputs(&raw).unwrap().swap_remove(0);
    eprintln!("7faac48b:0 — {} sats, script {} B", sats, s.len());
    assert_eq!(s.len(), 2_596_810);
    assert_eq!(recognise(&s), Recognised::Ordinal { content_type: "image/png".into() });
}

/// The ownership layout: exactly one P2PKH + one envelope, either order — the real
/// fixtures yield their own lock's hash; anything looser yields nothing.
#[test]
fn p1_lock_is_exactly_p2pkh_plus_one_envelope() {
    for (t, v, first) in [(ORDINAL.0, ORDINAL.1, true), (BSV21_TRANSFER.0, BSV21_TRANSFER.1, false),
                          (BSV21_DEPLOY_MINT.0, BSV21_DEPLOY_MINT.1, true), (BSV20_TICK_TRANSFER.0, BSV20_TICK_TRANSFER.1, true)] {
        let (_, s) = real(t, v);
        let lock = if first { &s[..25] } else { &s[s.len() - 25..] };
        assert_eq!(p2pkh_lock(&s), Some(&lock[3..23]), "{}", t);
    }
    let lock = p2pkh([7; 20]);
    let env = envelope("text/plain", b"hi");
    assert_eq!(p2pkh_lock(&lock), None, "a bare P2PKH is the caller's case, not this one");
    assert_eq!(p2pkh_lock(&[lock.clone(), env.clone(), vec![0x51]].concat()), None, "trailing opcode");
    assert_eq!(p2pkh_lock(&[lock.clone(), env.clone(), env.clone()].concat()), None, "two envelopes");
    assert_eq!(p2pkh_lock(&[env.clone(), lock.clone(), lock.clone()].concat()), None, "two locks");
    assert_eq!(p2pkh_lock(&[vec![0x51, 0x75], lock.clone()].concat()), None, "a non-envelope prefix");
    assert_eq!(p2pkh_lock(&hand_built_mandala()), None, "Mandala is not claimed by this layout");
    assert_eq!(p2pkh_lock(&[lock.clone(), env].concat()), Some(&lock[3..23]));
}

/// `P1-A10` (parser) — BRC-165 scope rules, one axis value per request (Hodos).
#[test]
fn p1_a10_p1sat_view_parse() {
    let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(p1sat_view("todo", &[]), P1satView::NotP1sat);
    assert_eq!(p1sat_view("p 1satx all", &[]), P1satView::NotP1sat);
    for bad in ["p 1sat", "P 1SAT ", "p 1sat foo", "p 1sat all extra"] {
        assert!(matches!(p1sat_view(bad, &[]), P1satView::Reject(_)), "{}", bad);
    }
    assert!(matches!(p1sat_view("p 1sat app", &t(&["collection:x"])), P1satView::Reject(_)), "axis tag missing");
    assert!(matches!(p1sat_view("p 1sat app", &t(&["app:x", "app:y"])), P1satView::Reject(_)), "two axis values");
    assert_eq!(p1sat_view(" P 1sat All ", &[]),
               P1satView::View { scope: "all", axis_tag: None, grant_key: Some("p 1sat all".into()) });
    assert_eq!(p1sat_view("p 1sat collection", &t(&["Collection:Abc", "type:image/png"])),
               P1satView::View { scope: "collection", axis_tag: Some("collection:abc".into()),
                                 grant_key: Some("p 1sat collection abc".into()) });
    assert_eq!(p1sat_view("p 1sat id", &t(&["id:k1"])),
               P1satView::View { scope: "id", axis_tag: Some("id:k1".into()), grant_key: None });
}
