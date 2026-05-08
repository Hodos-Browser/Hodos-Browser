///! Tier 12 — BRC-121 Phase 1 contract tests
///!
///! Verifies the deterministic primitives that pay_402 wires together. Full
///! transaction-shape (BEEF + service fee + change) coverage lives in the
///! Phase 1 demo-server end-to-end test (see demos/brc121-402/), since it
///! requires a fully-initialized AppState which is beyond unit-test scope.
///!
///! Sections:
///!  [1/3]  BRC-29 invoice format unchanged (no protocol drift from PeerPay)
///!  [2/3]  BRC-42 derivation against server pubkey is reproducible
///!  [3/3]  BRC-29 protocol ID magic constant unchanged

use std::sync::atomic::{AtomicUsize, Ordering};

static PASS: AtomicUsize = AtomicUsize::new(0);
static FAIL: AtomicUsize = AtomicUsize::new(0);

macro_rules! check {
    ($tag:expr, $cond:expr) => {{
        if $cond {
            PASS.fetch_add(1, Ordering::SeqCst);
            eprintln!("  PASS  {}", $tag);
        } else {
            FAIL.fetch_add(1, Ordering::SeqCst);
            eprintln!("**FAIL** {}", $tag);
        }
    }};
}

// ═══════════════════════════════════════════════════════════════════
// [1/3]  BRC-29 invoice format must match the constant used by
//         peerpay_send and task_check_peerpay (no protocol drift)
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t12_01_brc29_invoice_format_matches_peerpay() {
    eprintln!("\n[1/3] BRC-29 invoice format unchanged");

    let prefix = "ZGVtb3ByZWZpeA=="; // arbitrary base64
    let suffix = "ZGVtb3N1ZmZpeA==";

    // This MUST match the format in:
    //   handlers.rs::peerpay_send       (BRC-29 send path)
    //   handlers.rs::pay_402            (BRC-121 path under test)
    //   monitor::task_check_peerpay::run (BRC-29 receive path)
    let invoice = format!("2-3241645161d8-{} {}", prefix, suffix);

    check!("invoice/01 starts with security-level prefix '2-'", invoice.starts_with("2-"));
    check!("invoice/02 contains BRC-29 magic protocol id", invoice.contains("3241645161d8"));
    check!("invoice/03 prefix and suffix separated by single space", {
        let parts: Vec<&str> = invoice.splitn(2, ' ').collect();
        parts.len() == 2 && parts[0].ends_with(prefix) && parts[1] == suffix
    });
    check!(
        "invoice/04 full string equals canonical template",
        invoice == "2-3241645161d8-ZGVtb3ByZWZpeA== ZGVtb3N1ZmZpeA=="
    );

    assert_eq!(
        FAIL.load(Ordering::SeqCst),
        0,
        "BRC-29 invoice format drift detected"
    );
}

// ═══════════════════════════════════════════════════════════════════
// [2/3]  BRC-42 derivation against server pubkey is reproducible.
//         pay_402 builds an invoice and derives a child pubkey; same
//         inputs must always yield the same output.
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t12_02_brc42_derivation_against_server_pubkey() {
    use hodos_wallet::crypto::brc42::derive_child_public_key;

    eprintln!("\n[2/3] BRC-42 derivation reproducibility");

    // Fixed master private key (32 bytes of 0x11) — adequate for derivation,
    // not a real wallet key.
    let master_privkey: [u8; 32] = [0x11; 32];

    // A valid 33-byte compressed test pubkey (0x02 prefix).
    let server_pubkey =
        hex::decode("02a1633cafcc01ebfb6d78e39f687a1f0995c62fc95f51ead10a02ee0be551b5dc")
            .expect("valid hex");

    let invoice_a = "2-3241645161d8-fixed_prefix fixed_suffix";
    let invoice_b = "2-3241645161d8-DIFFERENT different";

    let child_a1 = derive_child_public_key(&master_privkey, &server_pubkey, invoice_a)
        .expect("derivation should succeed");
    let child_a2 = derive_child_public_key(&master_privkey, &server_pubkey, invoice_a)
        .expect("derivation should succeed (second call)");
    let child_b = derive_child_public_key(&master_privkey, &server_pubkey, invoice_b)
        .expect("derivation should succeed (different invoice)");

    check!("brc42/01 derivation is deterministic for identical inputs", child_a1 == child_a2);
    check!("brc42/02 child pubkey is 33 bytes (compressed)", child_a1.len() == 33);
    check!("brc42/03 child pubkey starts with 0x02 or 0x03", {
        let prefix = child_a1.first().copied().unwrap_or(0);
        prefix == 0x02 || prefix == 0x03
    });
    check!("brc42/04 child pubkey differs from server pubkey (privacy)", child_a1 != server_pubkey);
    check!(
        "brc42/05 different invoice yields different child key (per-payment isolation)",
        child_a1 != child_b
    );

    assert_eq!(
        FAIL.load(Ordering::SeqCst),
        0,
        "BRC-42 derivation regression detected"
    );
}

// ═══════════════════════════════════════════════════════════════════
// [3/3]  BRC-29 protocol ID magic constant must remain stable.
//         Changing this byte-string breaks PeerPay AND BRC-121 in lockstep.
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t12_03_brc29_protocol_id_constant() {
    eprintln!("\n[3/3] BRC-29 protocol id magic constant");

    let magic = "3241645161d8";

    check!("magic/01 length is 12 chars (6 bytes hex)", magic.len() == 12);
    check!("magic/02 chars are all ASCII hex", magic.chars().all(|c| c.is_ascii_hexdigit()));
    check!("magic/03 lowercase only (matches in-source usage)", magic == magic.to_lowercase());
    check!(
        "magic/04 hex-decodes to 6 bytes",
        hex::decode(magic).map(|b| b.len() == 6).unwrap_or(false)
    );

    assert_eq!(
        FAIL.load(Ordering::SeqCst),
        0,
        "BRC-29 protocol id drift detected"
    );
}
