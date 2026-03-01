///! Tier 9 — BEEF Struct Methods, ParsedTransaction, Serde Roundtrips, Error Types
///!
///! Sections:
///!  [1/6]  Beef::new + builder API              (10 tests)
///!  [2/6]  Beef V1 ↔ V2 serialization           (10 tests)
///!  [3/6]  Atomic BEEF (BRC-95)                  (8 tests)
///!  [4/6]  ParsedTransaction parsing             (10 tests)
///!  [5/6]  Serde roundtrips (ActionStatus, TransactionStatus, ProvenTxReqStatus, UTXO)  (12 tests)
///!  [6/6]  CacheError + PushDropError Display/From (8 tests)
///!
///! Total: 58 tests

use std::sync::atomic::{AtomicUsize, Ordering};
use sha2::{Sha256, Digest};

static PASS: AtomicUsize = AtomicUsize::new(0);
static FAIL: AtomicUsize = AtomicUsize::new(0);

macro_rules! check {
    ($tag:expr, $block:expr) => {{
        let tag = $tag;
        let result: Result<(), String> = (|| $block)();
        match result {
            Ok(()) => {
                PASS.fetch_add(1, Ordering::SeqCst);
                eprintln!("  PASS  {}", tag);
            }
            Err(e) => {
                FAIL.fetch_add(1, Ordering::SeqCst);
                eprintln!("**FAIL** {} — {}", tag, e);
            }
        }
    }};
}

// ── Real BRC-62 spec transactions (from beef.rs test_beef_roundtrip) ──
const PARENT_TX_HEX: &str = "0100000001cd4e4cac3c7b56920d1e7655e7e260d31f29d9a388d04910f1bbd72304a79029010000006b483045022100e75279a205a547c445719420aa3138bf14743e3f42618e5f86a19bde14bb95f7022064777d34776b05d816daf1699493fcdf2ef5a5ab1ad710d9c97bfb5b8f7cef3641210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013e660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000";
const MAIN_TX_HEX: &str = "0100000001ac4e164f5bc16746bb0868404292ac8318bbac3800e4aad13a014da427adce3e000000006a47304402203a61a2e931612b4bda08d541cfb980885173b8dcf64a3471238ae7abcd368d6402204cbf24f04b9aa2256d8901f0ed97866603d2be8324c2bfb7a37bf8fc90edd5b441210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013c660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000";

/// Helper: compute display-format TXID (double-SHA256, reversed)
fn compute_txid(tx_bytes: &[u8]) -> String {
    let h1 = Sha256::digest(tx_bytes);
    let h2 = Sha256::digest(&h1);
    hex::encode(h2.iter().rev().copied().collect::<Vec<u8>>())
}

/// Helper: build a BEEF with parent + main + TSC proof
fn build_test_beef() -> hodos_wallet::beef::Beef {
    use hodos_wallet::beef::Beef;
    let parent_tx = hex::decode(PARENT_TX_HEX).unwrap();
    let main_tx = hex::decode(MAIN_TX_HEX).unwrap();
    let parent_txid = compute_txid(&parent_tx);

    let mut beef = Beef::new();
    let idx = beef.add_parent_transaction(parent_tx);
    let tsc = serde_json::json!({
        "height": 918980,
        "index": 0,
        "nodes": [
            "9b18d77b48fde9b46d54b75d372e30a74cba0114cad4796f8f1d91946866a8bd",
            "45b8d1a256e4de964d2a70408e3ae4265b43544425ea40f370cd76d367575b0e"
        ]
    });
    beef.add_tsc_merkle_proof(&parent_txid, idx, &tsc).unwrap();
    beef.set_main_transaction(main_tx);
    beef
}

// ═══════════════════════════════════════════════════════════════════
// [1/6]  Beef::new + builder API
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t9_01_beef_new_builder() {
    use hodos_wallet::beef::{Beef, BEEF_VERSION_MARKER, BEEF_V2_MARKER};

    eprintln!("\n[1/6] Beef::new + builder API");

    check!("beef/01 new-empty-defaults", {
        let b = Beef::new();
        if b.version != BEEF_VERSION_MARKER {
            return Err(format!("version {:02x?} != V2 marker", b.version));
        }
        if !b.transactions.is_empty() {
            return Err("transactions not empty".into());
        }
        if !b.bumps.is_empty() {
            return Err("bumps not empty".into());
        }
        if !b.tx_to_bump.is_empty() {
            return Err("tx_to_bump not empty".into());
        }
        Ok(())
    });

    check!("beef/02 new-is-v2", {
        let b = Beef::new();
        if b.version != BEEF_V2_MARKER {
            return Err(format!("default version should be V2"));
        }
        Ok(())
    });

    check!("beef/03 has_proofs-empty", {
        let b = Beef::new();
        if b.has_proofs() {
            return Err("empty BEEF should not have proofs".into());
        }
        Ok(())
    });

    check!("beef/04 main_transaction-empty", {
        let b = Beef::new();
        if b.main_transaction().is_some() {
            return Err("empty BEEF should have no main tx".into());
        }
        Ok(())
    });

    check!("beef/05 parent_transactions-empty", {
        let b = Beef::new();
        if !b.parent_transactions().is_empty() {
            return Err("empty BEEF should have no parents".into());
        }
        Ok(())
    });

    check!("beef/06 set_main_transaction", {
        let mut b = Beef::new();
        let tx = hex::decode(MAIN_TX_HEX).unwrap();
        b.set_main_transaction(tx.clone());
        let main = b.main_transaction().ok_or("no main tx")?;
        if *main != tx {
            return Err("main tx bytes mismatch".into());
        }
        if b.transactions.len() != 1 {
            return Err(format!("expected 1 tx, got {}", b.transactions.len()));
        }
        Ok(())
    });

    check!("beef/07 add_parent-before-main", {
        let mut b = Beef::new();
        let parent = hex::decode(PARENT_TX_HEX).unwrap();
        let main = hex::decode(MAIN_TX_HEX).unwrap();
        b.set_main_transaction(main.clone());
        let idx = b.add_parent_transaction(parent.clone());
        // Parent should be before main
        if idx != 0 {
            return Err(format!("parent index should be 0, got {}", idx));
        }
        if b.transactions.len() != 2 {
            return Err(format!("expected 2 txs, got {}", b.transactions.len()));
        }
        // Last tx should be main
        if *b.main_transaction().unwrap() != main {
            return Err("last tx is not main".into());
        }
        // First tx should be parent
        if b.transactions[0] != parent {
            return Err("first tx is not parent".into());
        }
        Ok(())
    });

    check!("beef/08 find_txid-present", {
        let parent = hex::decode(PARENT_TX_HEX).unwrap();
        let main = hex::decode(MAIN_TX_HEX).unwrap();
        let parent_txid = compute_txid(&parent);
        let main_txid = compute_txid(&main);

        let mut b = Beef::new();
        b.add_parent_transaction(parent);
        b.set_main_transaction(main);

        let pi = b.find_txid(&parent_txid).ok_or("parent not found")?;
        if pi != 0 { return Err(format!("parent idx {} != 0", pi)); }
        let mi = b.find_txid(&main_txid).ok_or("main not found")?;
        if mi != 1 { return Err(format!("main idx {} != 1", mi)); }
        Ok(())
    });

    check!("beef/09 find_txid-absent", {
        let b = build_test_beef();
        let fake = "0000000000000000000000000000000000000000000000000000000000000000";
        if b.find_txid(fake).is_some() {
            return Err("found nonexistent txid".into());
        }
        Ok(())
    });

    check!("beef/10 find_txid-invalid-hex", {
        let b = build_test_beef();
        if b.find_txid("not-hex").is_some() {
            return Err("should return None for bad hex".into());
        }
        if b.find_txid("abcd").is_some() {
            return Err("should return None for short hex".into());
        }
        Ok(())
    });
}

// ═══════════════════════════════════════════════════════════════════
// [2/6]  Beef V1 ↔ V2 serialization
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t9_02_beef_v1_v2_serialization() {
    use hodos_wallet::beef::{Beef, BEEF_V1_MARKER, BEEF_V2_MARKER};

    eprintln!("\n[2/6] Beef V1 ↔ V2 serialization");

    check!("v1v2/01 to_bytes-starts-with-v2-marker", {
        let b = build_test_beef();
        let bytes = b.to_bytes().map_err(|e| e)?;
        if bytes[0..4] != BEEF_V2_MARKER {
            return Err(format!("expected V2 marker, got {:02x?}", &bytes[0..4]));
        }
        Ok(())
    });

    check!("v1v2/02 to_v1_bytes-starts-with-v1-marker", {
        let b = build_test_beef();
        let bytes = b.to_v1_bytes().map_err(|e| e)?;
        if bytes[0..4] != BEEF_V1_MARKER {
            return Err(format!("expected V1 marker, got {:02x?}", &bytes[0..4]));
        }
        Ok(())
    });

    check!("v1v2/03 to_hex-roundtrip-v2", {
        let b = build_test_beef();
        let hex_str = b.to_hex().map_err(|e| e)?;
        let b2 = Beef::from_hex(&hex_str).map_err(|e| e)?;
        if b2.transactions.len() != b.transactions.len() {
            return Err(format!("tx count {} != {}", b2.transactions.len(), b.transactions.len()));
        }
        if b2.bumps.len() != b.bumps.len() {
            return Err(format!("bump count {} != {}", b2.bumps.len(), b.bumps.len()));
        }
        for (i, (a, c)) in b.transactions.iter().zip(b2.transactions.iter()).enumerate() {
            if a != c { return Err(format!("tx {} mismatch", i)); }
        }
        Ok(())
    });

    check!("v1v2/04 to_v1_hex-roundtrip", {
        let b = build_test_beef();
        let hex_str = b.to_v1_hex().map_err(|e| e)?;
        let b2 = Beef::from_hex(&hex_str).map_err(|e| e)?;
        if b2.version != BEEF_V1_MARKER {
            return Err(format!("parsed version should be V1"));
        }
        if b2.transactions.len() != 2 {
            return Err(format!("expected 2 txs, got {}", b2.transactions.len()));
        }
        Ok(())
    });

    check!("v1v2/05 v2-roundtrip-preserves-bump-associations", {
        let b = build_test_beef();
        let bytes = b.to_bytes().map_err(|e| e)?;
        let b2 = Beef::from_bytes(&bytes).map_err(|e| e)?;
        // Parent (idx 0) should have bump, main (idx 1) should not
        if b2.tx_to_bump.len() < 2 {
            return Err(format!("tx_to_bump len {}", b2.tx_to_bump.len()));
        }
        if b2.tx_to_bump[0].is_none() {
            return Err("parent should have bump association".into());
        }
        if b2.tx_to_bump[1].is_some() {
            return Err("main tx should NOT have bump association".into());
        }
        Ok(())
    });

    check!("v1v2/06 v1-roundtrip-preserves-bump-associations", {
        let b = build_test_beef();
        let bytes = b.to_v1_bytes().map_err(|e| e)?;
        let b2 = Beef::from_bytes(&bytes).map_err(|e| e)?;
        if b2.tx_to_bump[0].is_none() {
            return Err("parent should have bump (V1)".into());
        }
        if b2.tx_to_bump[1].is_some() {
            return Err("main should NOT have bump (V1)".into());
        }
        Ok(())
    });

    check!("v1v2/07 has_proofs-after-tsc", {
        let b = build_test_beef();
        if !b.has_proofs() {
            return Err("BEEF with TSC proof should have proofs".into());
        }
        Ok(())
    });

    check!("v1v2/08 main_transaction-is-last", {
        let b = build_test_beef();
        let main = b.main_transaction().ok_or("no main tx")?;
        let expected = hex::decode(MAIN_TX_HEX).unwrap();
        if *main != expected {
            return Err("main transaction bytes mismatch".into());
        }
        Ok(())
    });

    check!("v1v2/09 parent_transactions-is-all-but-last", {
        let b = build_test_beef();
        let parents = b.parent_transactions();
        if parents.len() != 1 {
            return Err(format!("expected 1 parent, got {}", parents.len()));
        }
        let expected_parent = hex::decode(PARENT_TX_HEX).unwrap();
        if parents[0] != expected_parent {
            return Err("parent tx bytes mismatch".into());
        }
        Ok(())
    });

    check!("v1v2/10 extract_raw_tx_hex", {
        let b = build_test_beef();
        let beef_hex = b.to_hex().map_err(|e| e)?;
        let raw = Beef::extract_raw_tx_hex(&beef_hex).map_err(|e| e)?;
        if raw != MAIN_TX_HEX {
            return Err(format!("extracted raw tx != expected main tx"));
        }
        Ok(())
    });
}

// ═══════════════════════════════════════════════════════════════════
// [3/6]  Atomic BEEF (BRC-95)
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t9_03_atomic_beef() {
    use hodos_wallet::beef::{Beef, BEEF_V1_MARKER, ATOMIC_BEEF_MARKER};

    eprintln!("\n[3/6] Atomic BEEF (BRC-95)");

    check!("atomic/01 to_atomic_beef_hex-roundtrip", {
        let b = build_test_beef();
        let main_tx = hex::decode(MAIN_TX_HEX).unwrap();
        let main_txid = compute_txid(&main_tx);

        let atomic_hex = b.to_atomic_beef_hex(&main_txid).map_err(|e| e)?;
        let atomic_bytes = hex::decode(&atomic_hex).map_err(|e| format!("{}", e))?;

        // Should start with atomic marker
        if atomic_bytes[0..4] != ATOMIC_BEEF_MARKER {
            return Err(format!("expected atomic marker, got {:02x?}", &atomic_bytes[0..4]));
        }
        Ok(())
    });

    check!("atomic/02 atomic-contains-txid-big-endian", {
        let b = build_test_beef();
        let main_tx = hex::decode(MAIN_TX_HEX).unwrap();
        let main_txid = compute_txid(&main_tx);

        let atomic_hex = b.to_atomic_beef_hex(&main_txid).map_err(|e| e)?;
        let atomic_bytes = hex::decode(&atomic_hex).map_err(|e| format!("{}", e))?;

        // Bytes 4..36 should be TXID in big-endian (reversed from display)
        let txid_le_bytes = hex::decode(&main_txid).map_err(|e| format!("{}", e))?;
        let txid_be: Vec<u8> = txid_le_bytes.iter().rev().copied().collect();
        if atomic_bytes[4..36] != txid_be[..] {
            return Err("TXID in atomic BEEF not in big-endian".into());
        }
        Ok(())
    });

    check!("atomic/03 atomic-uses-v1-internally", {
        let b = build_test_beef();
        let main_tx = hex::decode(MAIN_TX_HEX).unwrap();
        let main_txid = compute_txid(&main_tx);

        let atomic_hex = b.to_atomic_beef_hex(&main_txid).map_err(|e| e)?;
        let atomic_bytes = hex::decode(&atomic_hex).map_err(|e| format!("{}", e))?;

        // After 36-byte header, should be V1 BEEF
        if atomic_bytes[36..40] != BEEF_V1_MARKER {
            return Err(format!("inner BEEF should be V1, got {:02x?}", &atomic_bytes[36..40]));
        }
        Ok(())
    });

    check!("atomic/04 from_atomic_beef_bytes-parses-back", {
        let b = build_test_beef();
        let main_tx = hex::decode(MAIN_TX_HEX).unwrap();
        let main_txid = compute_txid(&main_tx);

        let atomic_hex = b.to_atomic_beef_hex(&main_txid).map_err(|e| e)?;
        let atomic_bytes = hex::decode(&atomic_hex).map_err(|e| format!("{}", e))?;

        let (parsed_txid, parsed_beef) = Beef::from_atomic_beef_bytes(&atomic_bytes).map_err(|e| e)?;

        if parsed_txid != main_txid {
            return Err(format!("txid {} != {}", parsed_txid, main_txid));
        }
        if parsed_beef.transactions.len() != 2 {
            return Err(format!("expected 2 txs, got {}", parsed_beef.transactions.len()));
        }
        Ok(())
    });

    check!("atomic/05 from_atomic_beef_base64-roundtrip", {
        let b = build_test_beef();
        let main_tx = hex::decode(MAIN_TX_HEX).unwrap();
        let main_txid = compute_txid(&main_tx);

        let atomic_hex = b.to_atomic_beef_hex(&main_txid).map_err(|e| e)?;
        let atomic_bytes = hex::decode(&atomic_hex).map_err(|e| format!("{}", e))?;

        // Encode to base64
        use base64::{Engine as _, engine::general_purpose};
        let b64 = general_purpose::STANDARD.encode(&atomic_bytes);

        let (parsed_txid, parsed_beef) = Beef::from_atomic_beef_base64(&b64).map_err(|e| e)?;
        if parsed_txid != main_txid {
            return Err(format!("base64 roundtrip txid mismatch"));
        }
        if parsed_beef.transactions.len() != 2 {
            return Err(format!("base64 roundtrip tx count wrong"));
        }
        Ok(())
    });

    check!("atomic/06 atomic-too-short-rejected", {
        let short_bytes = vec![0x01, 0x01, 0x01, 0x01, 0x00, 0x00]; // only 6 bytes
        match Beef::from_atomic_beef_bytes(&short_bytes) {
            Err(e) if e.contains("too short") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject short atomic beef".into()),
        }
    });

    check!("atomic/07 atomic-bad-magic-rejected", {
        let mut bad = vec![0x00; 40]; // 40 bytes, wrong magic
        bad[0..4].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        // Put valid V2 BEEF marker at offset 36
        bad[36..40].copy_from_slice(&[0x02, 0x00, 0xbe, 0xef]);
        match Beef::from_atomic_beef_bytes(&bad) {
            Err(e) if e.contains("magic prefix") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject bad magic".into()),
        }
    });

    check!("atomic/08 to_atomic-invalid-txid-rejected", {
        let b = build_test_beef();
        // 31 bytes (not 32)
        match b.to_atomic_beef_hex("abcdef") {
            Err(e) if e.contains("32 bytes") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject short txid".into()),
        }
    });
}

// ═══════════════════════════════════════════════════════════════════
// [4/6]  ParsedTransaction parsing
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t9_04_parsed_transaction() {
    use hodos_wallet::beef::ParsedTransaction;

    eprintln!("\n[4/6] ParsedTransaction parsing");

    check!("parsed/01 from_hex-parent", {
        let ptx = ParsedTransaction::from_hex(PARENT_TX_HEX).map_err(|e| e)?;
        if ptx.version != 1 {
            return Err(format!("version {} != 1", ptx.version));
        }
        if ptx.inputs.len() != 1 {
            return Err(format!("inputs {} != 1", ptx.inputs.len()));
        }
        if ptx.outputs.len() != 1 {
            return Err(format!("outputs {} != 1", ptx.outputs.len()));
        }
        if ptx.lock_time != 0 {
            return Err(format!("locktime {} != 0", ptx.lock_time));
        }
        Ok(())
    });

    check!("parsed/02 from_hex-main", {
        let ptx = ParsedTransaction::from_hex(MAIN_TX_HEX).map_err(|e| e)?;
        if ptx.version != 1 {
            return Err(format!("version {} != 1", ptx.version));
        }
        if ptx.inputs.len() != 1 {
            return Err(format!("inputs {} != 1", ptx.inputs.len()));
        }
        if ptx.outputs.len() != 1 {
            return Err(format!("outputs {} != 1", ptx.outputs.len()));
        }
        Ok(())
    });

    check!("parsed/03 from_bytes-matches-from_hex", {
        let bytes = hex::decode(PARENT_TX_HEX).unwrap();
        let ptx1 = ParsedTransaction::from_hex(PARENT_TX_HEX).map_err(|e| e)?;
        let ptx2 = ParsedTransaction::from_bytes(&bytes).map_err(|e| e)?;
        if ptx1.version != ptx2.version { return Err("version mismatch".into()); }
        if ptx1.inputs.len() != ptx2.inputs.len() { return Err("input count mismatch".into()); }
        if ptx1.outputs.len() != ptx2.outputs.len() { return Err("output count mismatch".into()); }
        if ptx1.lock_time != ptx2.lock_time { return Err("locktime mismatch".into()); }
        Ok(())
    });

    check!("parsed/04 input-prev_txid-is-display-format", {
        let ptx = ParsedTransaction::from_hex(MAIN_TX_HEX).map_err(|e| e)?;
        let input = &ptx.inputs[0];
        // Main tx spends parent tx — prev_txid should match parent's TXID
        let parent_bytes = hex::decode(PARENT_TX_HEX).unwrap();
        let parent_txid = compute_txid(&parent_bytes);
        if input.prev_txid != parent_txid {
            return Err(format!("prev_txid {} != parent txid {}", input.prev_txid, parent_txid));
        }
        Ok(())
    });

    check!("parsed/05 input-prev_vout", {
        let ptx = ParsedTransaction::from_hex(MAIN_TX_HEX).map_err(|e| e)?;
        if ptx.inputs[0].prev_vout != 0 {
            return Err(format!("prev_vout {} != 0", ptx.inputs[0].prev_vout));
        }
        Ok(())
    });

    check!("parsed/06 input-sequence-is-max", {
        let ptx = ParsedTransaction::from_hex(MAIN_TX_HEX).map_err(|e| e)?;
        if ptx.inputs[0].sequence != 0xFFFFFFFF {
            return Err(format!("sequence 0x{:08x} != 0xFFFFFFFF", ptx.inputs[0].sequence));
        }
        Ok(())
    });

    check!("parsed/07 output-value", {
        let ptx = ParsedTransaction::from_hex(MAIN_TX_HEX).map_err(|e| e)?;
        // Main tx output is 0x663c = 26172 satoshis (little-endian in hex)
        if ptx.outputs[0].value != 26172 {
            return Err(format!("output value {} != 26172", ptx.outputs[0].value));
        }
        Ok(())
    });

    check!("parsed/08 output-script-is-p2pkh", {
        let ptx = ParsedTransaction::from_hex(MAIN_TX_HEX).map_err(|e| e)?;
        let script = &ptx.outputs[0].script;
        // P2PKH: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
        if script.len() != 25 {
            return Err(format!("P2PKH script len {} != 25", script.len()));
        }
        if script[0] != 0x76 || script[1] != 0xa9 || script[2] != 0x14 {
            return Err("not P2PKH prefix".into());
        }
        if script[23] != 0x88 || script[24] != 0xac {
            return Err("not P2PKH suffix".into());
        }
        Ok(())
    });

    check!("parsed/09 from_hex-invalid-hex", {
        match ParsedTransaction::from_hex("not-hex!!") {
            Err(e) if e.contains("Invalid hex") || e.contains("Odd number") => Ok(()),
            Err(e) => Err(format!("unexpected error: {}", e)),
            Ok(_) => Err("should reject bad hex".into()),
        }
    });

    check!("parsed/10 from_bytes-truncated", {
        // Just 4 bytes (version only, no inputs)
        let short = vec![0x01, 0x00, 0x00, 0x00];
        match ParsedTransaction::from_bytes(&short) {
            Err(_) => Ok(()),
            Ok(_) => Err("should reject truncated transaction".into()),
        }
    });
}

// ═══════════════════════════════════════════════════════════════════
// [5/6]  Serde roundtrips
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t9_05_serde_roundtrips() {
    use hodos_wallet::action_storage::{
        ActionStatus, TransactionStatus, ProvenTxReqStatus,
        ActionInput, ActionOutput,
    };
    use hodos_wallet::utxo_fetcher::UTXO;

    eprintln!("\n[5/6] Serde roundtrips");

    check!("serde/01 action-status-all-variants-json", {
        let variants = vec![
            (ActionStatus::Created, "created"),
            (ActionStatus::Signed, "signed"),
            (ActionStatus::Unconfirmed, "unconfirmed"),
            (ActionStatus::Pending, "pending"),
            (ActionStatus::Confirmed, "confirmed"),
            (ActionStatus::Aborted, "aborted"),
            (ActionStatus::Failed, "failed"),
        ];
        for (v, expected_str) in &variants {
            let json = serde_json::to_string(v).map_err(|e| format!("{}", e))?;
            if json != format!("\"{}\"", expected_str) {
                return Err(format!("{:?} serialized to {} (expected \"{}\")", v, json, expected_str));
            }
            let parsed: ActionStatus = serde_json::from_str(&json).map_err(|e| format!("{}", e))?;
            if parsed != *v {
                return Err(format!("roundtrip failed for {:?}", v));
            }
        }
        Ok(())
    });

    check!("serde/02 transaction-status-all-variants-json", {
        let variants = vec![
            (TransactionStatus::Completed, "completed"),
            (TransactionStatus::Unprocessed, "unprocessed"),
            (TransactionStatus::Sending, "sending"),
            (TransactionStatus::Unproven, "unproven"),
            (TransactionStatus::Unsigned, "unsigned"),
            (TransactionStatus::Nosend, "nosend"),
            (TransactionStatus::Nonfinal, "nonfinal"),
            (TransactionStatus::Failed, "failed"),
        ];
        for (v, expected_str) in &variants {
            let json = serde_json::to_string(v).map_err(|e| format!("{}", e))?;
            if json != format!("\"{}\"", expected_str) {
                return Err(format!("{:?} serialized to {} (expected \"{}\")", v, json, expected_str));
            }
            let parsed: TransactionStatus = serde_json::from_str(&json).map_err(|e| format!("{}", e))?;
            if parsed != *v {
                return Err(format!("roundtrip failed for {:?}", v));
            }
        }
        Ok(())
    });

    check!("serde/03 proven-tx-req-status-all-variants-json", {
        let variants = vec![
            (ProvenTxReqStatus::Unknown, "unknown"),
            (ProvenTxReqStatus::Sending, "sending"),
            (ProvenTxReqStatus::Unsent, "unsent"),
            (ProvenTxReqStatus::Nosend, "nosend"),
            (ProvenTxReqStatus::Unproven, "unproven"),
            (ProvenTxReqStatus::Invalid, "invalid"),
            (ProvenTxReqStatus::Unmined, "unmined"),
            (ProvenTxReqStatus::Callback, "callback"),
            (ProvenTxReqStatus::Completed, "completed"),
        ];
        for (v, expected_str) in &variants {
            let json = serde_json::to_string(v).map_err(|e| format!("{}", e))?;
            if json != format!("\"{}\"", expected_str) {
                return Err(format!("{:?} serialized to {} (expected \"{}\")", v, json, expected_str));
            }
            let parsed: ProvenTxReqStatus = serde_json::from_str(&json).map_err(|e| format!("{}", e))?;
            if parsed != *v {
                return Err(format!("roundtrip failed for {:?}", v));
            }
        }
        Ok(())
    });

    check!("serde/04 transaction-status-as_str-roundtrip", {
        let all = vec![
            TransactionStatus::Completed, TransactionStatus::Unprocessed,
            TransactionStatus::Sending, TransactionStatus::Unproven,
            TransactionStatus::Unsigned, TransactionStatus::Nosend,
            TransactionStatus::Nonfinal, TransactionStatus::Failed,
        ];
        for v in &all {
            let s = v.as_str();
            let back = TransactionStatus::from_str(s);
            // Check they serialize the same way (PartialEq on deserialized)
            let j1 = serde_json::to_string(v).unwrap();
            let j2 = serde_json::to_string(&back).unwrap();
            if j1 != j2 {
                return Err(format!("as_str roundtrip: {} -> {} -> {}", j1, s, j2));
            }
        }
        Ok(())
    });

    check!("serde/05 transaction-status-unknown-defaults-unprocessed", {
        let v = TransactionStatus::from_str("garbage_status_xyz");
        if v.as_str() != "unprocessed" {
            return Err(format!("unknown status should default to unprocessed, got {}", v.as_str()));
        }
        Ok(())
    });

    check!("serde/06 proven-tx-req-as_str-roundtrip", {
        let all = vec![
            ProvenTxReqStatus::Unknown, ProvenTxReqStatus::Sending,
            ProvenTxReqStatus::Unsent, ProvenTxReqStatus::Nosend,
            ProvenTxReqStatus::Unproven, ProvenTxReqStatus::Invalid,
            ProvenTxReqStatus::Unmined, ProvenTxReqStatus::Callback,
            ProvenTxReqStatus::Completed,
        ];
        for v in &all {
            let s = v.as_str();
            let back = ProvenTxReqStatus::from_str(s);
            let j1 = serde_json::to_string(v).unwrap();
            let j2 = serde_json::to_string(&back).unwrap();
            if j1 != j2 {
                return Err(format!("roundtrip mismatch: {} vs {}", j1, j2));
            }
        }
        Ok(())
    });

    check!("serde/07 proven-tx-req-is_terminal", {
        if !ProvenTxReqStatus::Completed.is_terminal() {
            return Err("Completed should be terminal".into());
        }
        if !ProvenTxReqStatus::Invalid.is_terminal() {
            return Err("Invalid should be terminal".into());
        }
        if ProvenTxReqStatus::Sending.is_terminal() {
            return Err("Sending should NOT be terminal".into());
        }
        if ProvenTxReqStatus::Unproven.is_terminal() {
            return Err("Unproven should NOT be terminal".into());
        }
        Ok(())
    });

    check!("serde/08 from-legacy-conversion", {
        // Created → Unsigned
        let t = TransactionStatus::from_legacy(&ActionStatus::Created, None);
        if t.as_str() != "unsigned" { return Err(format!("Created→{}", t.as_str())); }
        // Signed+broadcast → Sending
        let t = TransactionStatus::from_legacy(&ActionStatus::Signed, Some("broadcast"));
        if t.as_str() != "sending" { return Err(format!("Signed+broadcast→{}", t.as_str())); }
        // Confirmed → Completed
        let t = TransactionStatus::from_legacy(&ActionStatus::Confirmed, None);
        if t.as_str() != "completed" { return Err(format!("Confirmed→{}", t.as_str())); }
        // Aborted → Nosend
        let t = TransactionStatus::from_legacy(&ActionStatus::Aborted, None);
        if t.as_str() != "nosend" { return Err(format!("Aborted→{}", t.as_str())); }
        // Pending+confirmed → Completed
        let t = TransactionStatus::from_legacy(&ActionStatus::Pending, Some("confirmed"));
        if t.as_str() != "completed" { return Err(format!("Pending+confirmed→{}", t.as_str())); }
        // Pending+other → Unproven
        let t = TransactionStatus::from_legacy(&ActionStatus::Pending, None);
        if t.as_str() != "unproven" { return Err(format!("Pending→{}", t.as_str())); }
        Ok(())
    });

    check!("serde/09 to-action-status-conversion", {
        if TransactionStatus::Completed.to_action_status() != ActionStatus::Confirmed {
            return Err("Completed→Confirmed".into());
        }
        if TransactionStatus::Unsigned.to_action_status() != ActionStatus::Created {
            return Err("Unsigned→Created".into());
        }
        if TransactionStatus::Nosend.to_action_status() != ActionStatus::Aborted {
            return Err("Nosend→Aborted".into());
        }
        if TransactionStatus::Failed.to_action_status() != ActionStatus::Failed {
            return Err("Failed→Failed".into());
        }
        Ok(())
    });

    check!("serde/10 utxo-serde-roundtrip", {
        let utxo = UTXO {
            txid: "abc123".to_string(),
            vout: 0,
            satoshis: 50000,
            script: "76a914...88ac".to_string(),
            address_index: 3,
            custom_instructions: None,
        };
        let json = serde_json::to_string(&utxo).map_err(|e| format!("{}", e))?;
        let back: UTXO = serde_json::from_str(&json).map_err(|e| format!("{}", e))?;
        if back.txid != utxo.txid || back.vout != utxo.vout || back.satoshis != utxo.satoshis {
            return Err("UTXO roundtrip mismatch".into());
        }
        // custom_instructions should be absent from JSON when None
        if json.contains("custom_instructions") {
            return Err("None field should be skipped".into());
        }
        Ok(())
    });

    check!("serde/11 utxo-with-custom-instructions", {
        let utxo = UTXO {
            txid: "def456".to_string(),
            vout: 1,
            satoshis: 1000,
            script: "76a914...88ac".to_string(),
            address_index: -1,
            custom_instructions: Some("brc29-derivation".to_string()),
        };
        let json = serde_json::to_string(&utxo).map_err(|e| format!("{}", e))?;
        if !json.contains("custom_instructions") {
            return Err("Some field should be present".into());
        }
        let back: UTXO = serde_json::from_str(&json).map_err(|e| format!("{}", e))?;
        if back.custom_instructions != Some("brc29-derivation".to_string()) {
            return Err("custom_instructions roundtrip mismatch".into());
        }
        Ok(())
    });

    check!("serde/12 action-input-output-serde", {
        let input = ActionInput {
            txid: "abcdef00".to_string(),
            vout: 0,
            satoshis: 5000,
            script: None,
        };
        let output = ActionOutput {
            vout: 0,
            satoshis: 4500,
            script: Some("76a914...88ac".to_string()),
            address: Some("1A1z...".to_string()),
        };
        let ij = serde_json::to_string(&input).map_err(|e| format!("{}", e))?;
        let oj = serde_json::to_string(&output).map_err(|e| format!("{}", e))?;
        // None script should be skipped
        if ij.contains("script") {
            return Err("input None script should be skipped".into());
        }
        // Some script should be present
        if !oj.contains("script") {
            return Err("output Some script should be present".into());
        }
        // Roundtrip
        let back_in: ActionInput = serde_json::from_str(&ij).map_err(|e| format!("{}", e))?;
        let back_out: ActionOutput = serde_json::from_str(&oj).map_err(|e| format!("{}", e))?;
        if back_in.txid != "abcdef00" || back_out.satoshis != 4500 {
            return Err("roundtrip mismatch".into());
        }
        Ok(())
    });
}

// ═══════════════════════════════════════════════════════════════════
// [6/6]  CacheError + PushDropError Display/From
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t9_06_error_types() {
    use hodos_wallet::cache_errors::CacheError;
    use hodos_wallet::script::pushdrop::PushDropError;

    eprintln!("\n[6/6] CacheError + PushDropError Display/From");

    check!("errors/01 cache-error-from-hex", {
        let hex_err = hex::decode("zzzz").unwrap_err();
        let ce: CacheError = hex_err.into();
        let msg = format!("{}", ce);
        if !msg.contains("Hex decode error") {
            return Err(format!("unexpected display: {}", msg));
        }
        Ok(())
    });

    check!("errors/02 cache-error-from-json", {
        let json_err = serde_json::from_str::<serde_json::Value>("{{bad}}").unwrap_err();
        let ce: CacheError = json_err.into();
        let msg = format!("{}", ce);
        if !msg.contains("JSON error") {
            return Err(format!("unexpected display: {}", msg));
        }
        Ok(())
    });

    check!("errors/03 cache-error-api-display", {
        let ce = CacheError::Api("timeout".to_string());
        let msg = format!("{}", ce);
        if !msg.contains("API error") || !msg.contains("timeout") {
            return Err(format!("unexpected display: {}", msg));
        }
        Ok(())
    });

    check!("errors/04 cache-error-invalid-data-display", {
        let ce = CacheError::InvalidData("bad format".to_string());
        let msg = format!("{}", ce);
        if !msg.contains("Invalid data") || !msg.contains("bad format") {
            return Err(format!("unexpected display: {}", msg));
        }
        Ok(())
    });

    check!("errors/05 cache-error-is-std-error", {
        // Verify CacheError implements std::error::Error
        let ce = CacheError::Api("test".to_string());
        let _: &dyn std::error::Error = &ce;
        Ok(())
    });

    check!("errors/06 pushdrop-error-variants-display", {
        use hodos_wallet::script::ScriptParseError;
        let e1 = PushDropError::ParseError(ScriptParseError::Other("bad parse".to_string()));
        let e2 = PushDropError::InvalidScriptStructure("bad structure".to_string());
        let e3 = PushDropError::MissingPublicKey;
        let e4 = PushDropError::MissingChecksig;
        let e5 = PushDropError::Other("misc".to_string());

        // Just verify Display works without panicking
        let _ = format!("{}", e1);
        let _ = format!("{}", e2);
        let _ = format!("{}", e3);
        let _ = format!("{}", e4);
        let _ = format!("{}", e5);
        Ok(())
    });

    check!("errors/07 pushdrop-decode-empty-script", {
        use hodos_wallet::script::pushdrop::decode;
        match decode(&[]) {
            Err(_) => Ok(()),
            Ok(_) => Err("empty script should fail decode".into()),
        }
    });

    check!("errors/08 pushdrop-decode-garbage-script", {
        use hodos_wallet::script::pushdrop::decode;
        // Random bytes that are not a valid PushDrop script
        let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];
        match decode(&garbage) {
            Err(_) => Ok(()),
            Ok(_) => Err("garbage script should fail decode".into()),
        }
    });
}

// ═══════════════════════════════════════════════════════════════════
// Summary
// ═══════════════════════════════════════════════════════════════════
#[test]
fn t9_99_summary() {
    // Sleep briefly to let parallel tests finish printing
    std::thread::sleep(std::time::Duration::from_millis(200));
    let p = PASS.load(Ordering::SeqCst);
    let f = FAIL.load(Ordering::SeqCst);
    eprintln!("\n════════════════════════════════════════");
    eprintln!("  TIER 9 FINAL:  {} passed, {} failed  (of {} total)", p, f, p + f);
    eprintln!("════════════════════════════════════════\n");
    assert_eq!(f, 0, "{} test(s) failed — see FAIL lines above", f);
}
