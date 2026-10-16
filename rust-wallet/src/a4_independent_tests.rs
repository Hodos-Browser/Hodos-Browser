//! B5-T2-P1 `P1-A4` — independent REDs (mutation-driven): every token shape the wallet
//! recognises but does not file must reach the seam as **held with the token reason**, at
//! the value it really carries, locked by a key the wallet signs with.
//!
//! Why the exact `Class` and not "not Money": `P1-A4` has two overlapping guards (the
//! recognisers and rule ②'s exact-25-byte template). Either alone keeps these coins out of
//! the balance, so a test that only asserts "held" passes with a recogniser gone. The
//! owner closure answers `Ours` throughout, so the recogniser is the only thing standing
//! between each fixture and `Class::Money` once rule ② is loosened.
use super::*;
use crate::tokens::tests::{envelope, p2pkh, push};
use crate::tokens::{verdict, Verdict};

const ID: &str = "6a0a5f91ea03cdd4d7371b4ee0572dadb349588cb3b0bfc95fba0cd2a1987488_0";

/// The seam over an observed script, with an owner check that says "ours".
fn seam(script: &[u8], sats: i64) -> Class {
    let ev = Evidence { script_length: Some(script.len() as i64), script: Some(script), satoshis: sats };
    classify(&ev, |_| OwnerCheck::Ours)
}

fn bsv20(json: &str) -> Vec<u8> {
    let mut s = p2pkh([9; 20]);
    s.extend(envelope("application/bsv-20", json.as_bytes()));
    s
}

/// `<id push> <amount push> OP_2DROP <our P2PKH>`.
fn mandala(id: Option<&[u8]>) -> Vec<u8> {
    let mut s = match id { Some(d) => push(d), None => vec![0x00] };
    s.extend(push(&[0x88, 0x13]));
    s.push(0x6d);
    s.extend(p2pkh([9; 20]));
    s
}

fn held_as(script: &[u8], sats: i64, kind: &str) {
    // The money outcome first, so a mutation that makes a token money fails HERE, on the
    // assertion that matters, not on the recogniser kind below (review of the A4 run).
    let class = seam(script, sats);
    assert_ne!(class, Class::Money, "token became MONEY at {} sats", sats);
    assert_eq!(class, Class::Unknown(Reason::TokenShaped), "seam class at {} sats", sats);
    assert_eq!(verdict(script, sats), Verdict::Held(match kind {
        "mandala" => "mandala",
        "bsv21-non-value" => "bsv21-non-value",
        "bsv20-tick" => "bsv20-tick",
        "bsv21-malformed" => "bsv21-malformed",
        "envelope-on-multi-sat" => "envelope-on-multi-sat",
        k => panic!("unknown kind {}", k),
    }), "recogniser kind at {} sats", sats);
}

#[test]
fn a4_deploy_auth_is_held_even_with_amt_and_id() {
    for json in [
        r#"{"p":"bsv-20","op":"deploy+auth","sym":"X"}"#.to_string(),
        format!(r#"{{"p":"bsv-20","op":"deploy+auth","amt":"1000","sym":"X","id":"{}"}}"#, ID),
        r#"{"p":"bsv-20","op":"deploy+auth","amt":"1000"}"#.to_string(),
    ] {
        held_as(&bsv20(&json), 1, "bsv21-non-value");
    }
}

#[test]
fn a4_auth_and_burn_are_held_even_with_amt_and_id() {
    for op in ["auth", "burn"] {
        let json = format!(r#"{{"p":"bsv-20","op":"{}","amt":"1000","id":"{}"}}"#, op, ID);
        for sats in [1, 546] {
            held_as(&bsv20(&json), sats, "bsv21-non-value");
        }
    }
}

#[test]
fn a4_mandala_at_one_sat_every_id_form() {
    for id in [Some(&[0xab; 32][..]), Some(&[0xcd; 36][..]), None] {
        held_as(&mandala(id), 1, "mandala");
    }
}

#[test]
fn a4_mandala_multi_sat_on_our_key_is_token_shaped_not_money() {
    for id in [Some(&[0xab; 32][..]), Some(&[0xcd; 36][..]), None] {
        for sats in [2, 546, 5000] {
            held_as(&mandala(id), sats, "mandala");
        }
    }
    // Control: the same P2PKH alone, same value, same owner answer — is money.
    assert_eq!(seam(&p2pkh([9; 20]), 5000), Class::Money);
}

#[test]
fn a4_legacy_bsv20_tick_deploy_and_transfer() {
    for json in [
        r#"{"p":"bsv-20","op":"deploy","tick":"ordi","max":"21000000","lim":"1000"}"#.to_string(),
        r#"{"p":"bsv-20","op":"transfer","tick":"ordi","amt":"100"}"#.to_string(),
        r#"{"p":"bsv-20","op":"mint","tick":"ordi","amt":"1000"}"#.to_string(),
        format!(r#"{{"p":"bsv-20","op":"transfer","tick":"ordi","amt":"100","id":"{}"}}"#, ID),
    ] {
        held_as(&bsv20(&json), 1, "bsv20-tick");
    }
}

#[test]
fn a4_envelope_on_more_than_one_sat_is_held_at_the_seam() {
    let mut ord = p2pkh([9; 20]);
    ord.extend(envelope("text/plain", b"hi"));
    let value = bsv20(&format!(r#"{{"p":"bsv-20","op":"transfer","amt":"5","id":"{}"}}"#, ID));
    for s in [&ord, &value] {
        assert!(matches!(seam(s, 1), Class::Token(_)), "1-sat control must be filed");
        for sats in [2, 546, 5000] {
            held_as(s, sats, "envelope-on-multi-sat");
        }
    }
}

#[test]
fn a4_malformed_bsv21_json_is_held() {
    for json in [
        format!(r#"{{"p":"bsv-20","op":"transfer","amt":"5","amt":"9","id":"{}"}}"#, ID), // duplicate key
        format!(r#"{{"p":"bsv-20","op":"transfer","amt":"05","id":"{}"}}"#, ID),          // leading zero
        format!(r#"{{"p":"bsv-20","op":"transfer","amt":5,"id":"{}"}}"#, ID),             // numeric amt
        r#"{"p":"bsv-20","op":"transfer","amt":"5"}"#.to_string(),                          // no id
        format!(r#"{{"p":"bsv-20","op":"transfer","amt":"5","id":"{}"}}"#, ID.to_uppercase()), // upper hex
        format!(r#"{{"p":"bsv-20","op":"transfer","amt":"-5","id":"{}"}}"#, ID),
        format!(r#"{{"p":"bsv-20","op":"Transfer","amt":"5","id":"{}"}}"#, ID),
        r#"{"p":"bsv-20","op":"transfer","amt":"5","#.to_string(),                          // truncated
    ] {
        held_as(&bsv20(&json), 1, "bsv21-malformed");
    }
}

#[test]
fn a4_unparseable_script_is_unreadable_not_not_plain() {
    // Our P2PKH followed by a push that runs off the end; and a lone OP_PUSHDATA1.
    let mut overrun = p2pkh([9; 20]);
    overrun.extend([0x0a, 0x01, 0x02]);
    for s in [overrun, vec![0x4c]] {
        assert_eq!(verdict(&s, 5000), Verdict::Unreadable);
        assert_eq!(seam(&s, 5000), Class::Unknown(Reason::Unreadable));
        assert_eq!(seam(&s, 1), Class::Unknown(Reason::Unreadable));
    }
    // Observed length but the bytes are not held (above the inline cap, parent missing).
    let ev = Evidence { script_length: Some(2_000), script: None, satoshis: 5000 };
    assert_eq!(classify(&ev, |_| OwnerCheck::Ours), Class::Unknown(Reason::Unreadable));
}
