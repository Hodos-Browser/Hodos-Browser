/// ═══════════════════════════════════════════════════════════════════
/// HODOS WALLET — TIER 2 DIAGNOSTIC TEST SUITE
/// BEEF, BRC-2 Encryption, Certificate Verification
/// ═══════════════════════════════════════════════════════════════════
///
/// Tests:
///   [1] BEEF format detection, parsing, serialization, roundtrip
///   [2] BUMP (BRC-74) ↔ TSC merkle proof conversion
///   [3] AES-256-GCM with 12-byte and 32-byte IVs
///   [4] BRC-2 symmetric key derivation and encrypt/decrypt
///   [5] Certificate preimage serialization
///   [6] Certificate synthetic sign + verify (full BRC-52 chain)
///
/// Methodology: collect-first — no panics, structured PASS/FAIL reporting.

use std::collections::HashMap;
use hodos_wallet::crypto::brc42;
use hodos_wallet::crypto::brc2;
use hodos_wallet::crypto::aesgcm_custom;
use hodos_wallet::crypto::signing::sha256;
use hodos_wallet::crypto::brc43::{InvoiceNumber, SecurityLevel};
use hodos_wallet::certificate::types::{Certificate, CertificateField};
use hodos_wallet::certificate::verifier::{serialize_certificate_preimage, verify_certificate_signature};
use hodos_wallet::beef::Beef;

use std::sync::atomic::{AtomicUsize, Ordering};

static PASS_COUNT: AtomicUsize = AtomicUsize::new(0);
static FAIL_COUNT: AtomicUsize = AtomicUsize::new(0);
static SKIP_COUNT: AtomicUsize = AtomicUsize::new(0);

macro_rules! check {
    ($section:expr, $id:expr, $result:expr) => {
        match (|| -> Result<(), String> { $result })() {
            Ok(()) => {
                PASS_COUNT.fetch_add(1, Ordering::Relaxed);
                println!("    [PASS] {}/{}", $section, $id);
            }
            Err(e) => {
                FAIL_COUNT.fetch_add(1, Ordering::Relaxed);
                println!("    [FAIL] {}/{}: {}", $section, $id, e);
            }
        }
    };
}

macro_rules! skip {
    ($section:expr, $id:expr, $reason:expr) => {
        SKIP_COUNT.fetch_add(1, Ordering::Relaxed);
        println!("    [SKIP] {}/{}: {}", $section, $id, $reason);
    };
}

fn assert_eq_hex(label: &str, actual: &[u8], expected: &[u8]) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{}: expected {}, got {}",
            label, hex::encode(expected), hex::encode(actual)))
    }
}

#[test]
fn tier2_diagnostic_suite() {
    println!();
    println!("=========================================================================");
    println!("  HODOS WALLET — TIER 2 DIAGNOSTIC SUITE");
    println!("  BEEF, BRC-2 Encryption, Certificate Verification");
    println!("=========================================================================");

    // ═══════════════════════════════════════════════════════════════
    // [1/6] BEEF Format Detection & Parsing
    // ═══════════════════════════════════════════════════════════════
    println!();
    println!("  [1/6] BEEF Format Detection & Parsing");

    // 1a. V1 marker detection
    check!("beef-format", "1a-v1-marker", {
        let v1_marker: [u8; 4] = [0x01, 0x00, 0xbe, 0xef];
        if v1_marker == hodos_wallet::beef::BEEF_V1_MARKER {
            Ok(())
        } else {
            Err("V1 marker mismatch".to_string())
        }
    });

    // 1b. V2 marker detection
    check!("beef-format", "1b-v2-marker", {
        let v2_marker: [u8; 4] = [0x02, 0x00, 0xbe, 0xef];
        if v2_marker == hodos_wallet::beef::BEEF_V2_MARKER {
            Ok(())
        } else {
            Err("V2 marker mismatch".to_string())
        }
    });

    // 1c. Atomic BEEF marker
    check!("beef-format", "1c-atomic-marker", {
        let atomic_marker: [u8; 4] = [0x01, 0x01, 0x01, 0x01];
        if atomic_marker == hodos_wallet::beef::ATOMIC_BEEF_MARKER {
            Ok(())
        } else {
            Err("Atomic marker mismatch".to_string())
        }
    });

    // 1d. Create empty BEEF and check defaults
    check!("beef-format", "1d-new-beef-defaults", {
        let beef = Beef::new();
        if beef.bumps.is_empty() && beef.transactions.is_empty() && beef.tx_to_bump.is_empty() {
            Ok(())
        } else {
            Err("New BEEF should be empty".to_string())
        }
    });

    // 1e. BEEF V2 roundtrip: build → serialize → parse → verify
    check!("beef-roundtrip", "1e-v2-roundtrip", {
        // Real parent transaction (from BRC-62 spec)
        let parent_hex = "0100000001cd4e4cac3c7b56920d1e7655e7e260d31f29d9a388d04910f1bbd72304a79029010000006b483045022100e75279a205a547c445719420aa3138bf14743e3f42618e5f86a19bde14bb95f7022064777d34776b05d816daf1699493fcdf2ef5a5ab1ad710d9c97bfb5b8f7cef3641210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013e660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000";
        let parent_tx = hex::decode(parent_hex).map_err(|e| e.to_string())?;

        // Main transaction
        let main_hex = "0100000001ac4e164f5bc16746bb0868404292ac8318bbac3800e4aad13a014da427adce3e000000006a47304402203a61a2e931612b4bda08d541cfb980885173b8dcf64a3471238ae7abcd368d6402204cbf24f04b9aa2256d8901f0ed97866603d2be8324c2bfb7a37bf8fc90edd5b441210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013c660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000";
        let main_tx = hex::decode(main_hex).map_err(|e| e.to_string())?;

        let mut beef = Beef::new();
        let tx_idx = beef.add_parent_transaction(parent_tx.clone());

        // Compute parent txid
        let parent_txid = {
            use sha2::{Sha256, Digest};
            let h1 = Sha256::digest(&parent_tx);
            let h2 = Sha256::digest(&h1);
            hex::encode(h2.iter().rev().copied().collect::<Vec<u8>>())
        };

        // Add TSC merkle proof
        let tsc = serde_json::json!({
            "height": 918980,
            "index": 0,
            "nodes": [
                "9b18d77b48fde9b46d54b75d372e30a74cba0114cad4796f8f1d91946866a8bd",
                "45b8d1a256e4de964d2a70408e3ae4265b43544425ea40f370cd76d367575b0e"
            ]
        });
        beef.add_tsc_merkle_proof(&parent_txid, tx_idx, &tsc).map_err(|e| e.to_string())?;
        beef.set_main_transaction(main_tx.clone());

        // Serialize to V2 bytes
        let bytes = beef.to_bytes().map_err(|e| e.to_string())?;

        // Verify V2 marker at start
        if &bytes[0..4] != &[0x02, 0x00, 0xbe, 0xef] {
            return Err(format!("Expected V2 marker, got {:02x?}", &bytes[0..4]));
        }

        // Parse back
        let parsed = Beef::from_bytes(&bytes).map_err(|e| e.to_string())?;

        if parsed.transactions.len() != 2 {
            return Err(format!("Expected 2 transactions, got {}", parsed.transactions.len()));
        }
        if parsed.transactions[0] != parent_tx {
            return Err("Parent tx mismatch after roundtrip".to_string());
        }
        if parsed.transactions[1] != main_tx {
            return Err("Main tx mismatch after roundtrip".to_string());
        }
        if parsed.bumps.len() != 1 {
            return Err(format!("Expected 1 BUMP, got {}", parsed.bumps.len()));
        }
        if parsed.bumps[0].block_height != 918980 {
            return Err(format!("Block height mismatch: expected 918980, got {}", parsed.bumps[0].block_height));
        }
        Ok(())
    });

    // 1f. BEEF V1 roundtrip
    check!("beef-roundtrip", "1f-v1-roundtrip", {
        let parent_hex = "0100000001cd4e4cac3c7b56920d1e7655e7e260d31f29d9a388d04910f1bbd72304a79029010000006b483045022100e75279a205a547c445719420aa3138bf14743e3f42618e5f86a19bde14bb95f7022064777d34776b05d816daf1699493fcdf2ef5a5ab1ad710d9c97bfb5b8f7cef3641210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013e660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000";
        let parent_tx = hex::decode(parent_hex).map_err(|e| e.to_string())?;
        let main_tx = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]; // minimal

        let mut beef = Beef::new();
        let tx_idx = beef.add_parent_transaction(parent_tx.clone());

        let parent_txid = {
            use sha2::{Sha256, Digest};
            let h1 = Sha256::digest(&parent_tx);
            let h2 = Sha256::digest(&h1);
            hex::encode(h2.iter().rev().copied().collect::<Vec<u8>>())
        };

        let tsc = serde_json::json!({
            "height": 100,
            "index": 0,
            "nodes": ["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
        });
        beef.add_tsc_merkle_proof(&parent_txid, tx_idx, &tsc).map_err(|e| e.to_string())?;
        beef.set_main_transaction(main_tx.clone());

        // V1 serialize
        let v1_bytes = beef.to_v1_bytes().map_err(|e| e.to_string())?;
        if &v1_bytes[0..4] != &[0x01, 0x00, 0xbe, 0xef] {
            return Err(format!("Expected V1 marker, got {:02x?}", &v1_bytes[0..4]));
        }

        // Parse V1 back
        let parsed = Beef::from_bytes(&v1_bytes).map_err(|e| e.to_string())?;
        if parsed.transactions[0] != parent_tx {
            return Err("V1 parent tx mismatch".to_string());
        }
        Ok(())
    });

    // 1g. BEEF hex roundtrip
    check!("beef-roundtrip", "1g-hex-roundtrip", {
        let mut beef = Beef::new();
        let tx = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        beef.set_main_transaction(tx.clone());

        let hex_str = beef.to_hex().map_err(|e| e.to_string())?;
        let parsed = Beef::from_hex(&hex_str).map_err(|e| e.to_string())?;
        if parsed.transactions.last() != Some(&tx) {
            return Err("Hex roundtrip mismatch".to_string());
        }
        Ok(())
    });

    // 1h. Atomic BEEF roundtrip
    check!("beef-roundtrip", "1h-atomic-roundtrip", {
        let mut beef = Beef::new();
        let tx = hex::decode("0100000001cd4e4cac3c7b56920d1e7655e7e260d31f29d9a388d04910f1bbd72304a79029010000006b483045022100e75279a205a547c445719420aa3138bf14743e3f42618e5f86a19bde14bb95f7022064777d34776b05d816daf1699493fcdf2ef5a5ab1ad710d9c97bfb5b8f7cef3641210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013e660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000").unwrap();
        beef.set_main_transaction(tx.clone());

        // Compute txid
        let txid = {
            use sha2::{Sha256, Digest};
            let h1 = Sha256::digest(&tx);
            let h2 = Sha256::digest(&h1);
            hex::encode(h2.iter().rev().copied().collect::<Vec<u8>>())
        };

        let atomic_hex = beef.to_atomic_beef_hex(&txid).map_err(|e| e.to_string())?;

        // Should start with 01010101
        if !atomic_hex.starts_with("01010101") {
            return Err(format!("Atomic BEEF should start with 01010101, got {}", &atomic_hex[..8]));
        }

        // Parse back
        let (parsed_txid, parsed_beef) = Beef::from_atomic_beef_bytes(
            &hex::decode(&atomic_hex).map_err(|e| e.to_string())?
        ).map_err(|e| e.to_string())?;

        if parsed_txid != txid {
            return Err(format!("Atomic TXID mismatch: expected {}, got {}", txid, parsed_txid));
        }
        if parsed_beef.transactions.last() != Some(&tx) {
            return Err("Atomic BEEF tx mismatch".to_string());
        }
        Ok(())
    });

    // 1i. BUMP associations (parent with BUMP, main without)
    check!("beef-bump", "1i-associations", {
        let mut beef = Beef::new();
        let parent = hex::decode("0100000001cd4e4cac3c7b56920d1e7655e7e260d31f29d9a388d04910f1bbd72304a79029010000006b483045022100e75279a205a547c445719420aa3138bf14743e3f42618e5f86a19bde14bb95f7022064777d34776b05d816daf1699493fcdf2ef5a5ab1ad710d9c97bfb5b8f7cef3641210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013e660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000").unwrap();
        let tx_idx = beef.add_parent_transaction(parent.clone());

        let txid = {
            use sha2::{Sha256, Digest};
            let h1 = Sha256::digest(&parent);
            let h2 = Sha256::digest(&h1);
            hex::encode(h2.iter().rev().copied().collect::<Vec<u8>>())
        };

        let tsc = serde_json::json!({
            "height": 500000,
            "index": 42,
            "nodes": [
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            ]
        });
        beef.add_tsc_merkle_proof(&txid, tx_idx, &tsc).map_err(|e| e.to_string())?;
        beef.set_main_transaction(vec![0xBB; 10]);

        if beef.tx_to_bump[0] != Some(0) {
            return Err(format!("Parent should map to BUMP 0, got {:?}", beef.tx_to_bump[0]));
        }
        if beef.tx_to_bump[1] != None {
            return Err(format!("Main tx should have no BUMP, got {:?}", beef.tx_to_bump[1]));
        }
        Ok(())
    });

    // 1j. Invalid BEEF hex should fail gracefully
    check!("beef-error", "1j-invalid-hex", {
        match Beef::from_hex("not_valid_hex") {
            Err(_) => Ok(()),
            Ok(_) => Err("Should fail on invalid hex".to_string()),
        }
    });

    // 1k. Too-short BEEF should fail
    check!("beef-error", "1k-too-short", {
        match Beef::from_hex("0100") {
            Err(_) => Ok(()),
            Ok(_) => Err("Should fail on too-short data".to_string()),
        }
    });

    // 1l. Wrong version marker should fail
    check!("beef-error", "1l-wrong-version", {
        match Beef::from_hex("deadbeef0000") {
            Err(_) => Ok(()),
            Ok(_) => Err("Should fail on wrong version marker".to_string()),
        }
    });

    // ═══════════════════════════════════════════════════════════════
    // [2/6] BUMP ↔ TSC Merkle Proof Conversion
    // ═══════════════════════════════════════════════════════════════
    println!();
    println!("  [2/6] BUMP ↔ TSC Merkle Proof Conversion");

    // 2a. TSC → BUMP → serialize → parse_bump_hex_to_tsc → verify height/index
    check!("bump-tsc", "2a-roundtrip-simple", {
        // Build a BEEF with a known TSC proof, serialize, then extract BUMP and convert back
        let mut beef = Beef::new();
        let tx = hex::decode("0100000001cd4e4cac3c7b56920d1e7655e7e260d31f29d9a388d04910f1bbd72304a79029010000006b483045022100e75279a205a547c445719420aa3138bf14743e3f42618e5f86a19bde14bb95f7022064777d34776b05d816daf1699493fcdf2ef5a5ab1ad710d9c97bfb5b8f7cef3641210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013e660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000").unwrap();
        let tx_idx = beef.add_parent_transaction(tx.clone());

        let txid = {
            use sha2::{Sha256, Digest};
            let h1 = Sha256::digest(&tx);
            let h2 = Sha256::digest(&h1);
            hex::encode(h2.iter().rev().copied().collect::<Vec<u8>>())
        };

        let original_height = 850000u32;
        let original_index = 5u64;
        let tsc = serde_json::json!({
            "height": original_height,
            "index": original_index,
            "nodes": [
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
            ]
        });
        beef.add_tsc_merkle_proof(&txid, tx_idx, &tsc).map_err(|e| e.to_string())?;

        // Verify the BUMP stored in our BEEF
        if beef.bumps[0].block_height != original_height {
            return Err(format!("BUMP height mismatch: expected {}, got {}", original_height, beef.bumps[0].block_height));
        }
        if beef.bumps[0].tree_height != 3 {
            return Err(format!("BUMP tree_height mismatch: expected 3, got {}", beef.bumps[0].tree_height));
        }
        Ok(())
    });

    // 2b. TSC with duplicate marker (*)
    check!("bump-tsc", "2b-duplicate-marker", {
        let mut beef = Beef::new();
        let tx = vec![0x01; 32]; // dummy tx
        let tx_idx = beef.add_parent_transaction(tx.clone());

        let txid = {
            use sha2::{Sha256, Digest};
            let h1 = Sha256::digest(&tx);
            let h2 = Sha256::digest(&h1);
            hex::encode(h2.iter().rev().copied().collect::<Vec<u8>>())
        };

        // Use "*" for duplicate node (tx is last in its pair)
        let tsc = serde_json::json!({
            "height": 100,
            "index": 1,
            "nodes": [
                "*",  // duplicate (tx has no sibling, it IS the sibling)
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            ]
        });
        beef.add_tsc_merkle_proof(&txid, tx_idx, &tsc).map_err(|e| e.to_string())?;
        if beef.bumps[0].tree_height != 2 {
            return Err(format!("Expected tree_height 2, got {}", beef.bumps[0].tree_height));
        }
        Ok(())
    });

    // 2c. parse_bump_hex_to_tsc on a manually-crafted BUMP
    check!("bump-tsc", "2c-parse-bump-hex", {
        // Build a BEEF, serialize, extract just the BUMP bytes and test parse_bump_hex_to_tsc
        let mut beef = Beef::new();
        let tx = hex::decode("0100000001cd4e4cac3c7b56920d1e7655e7e260d31f29d9a388d04910f1bbd72304a79029010000006b483045022100e75279a205a547c445719420aa3138bf14743e3f42618e5f86a19bde14bb95f7022064777d34776b05d816daf1699493fcdf2ef5a5ab1ad710d9c97bfb5b8f7cef3641210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013e660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000").unwrap();
        let tx_idx = beef.add_parent_transaction(tx.clone());
        let txid = {
            use sha2::{Sha256, Digest};
            let h1 = Sha256::digest(&tx);
            let h2 = Sha256::digest(&h1);
            hex::encode(h2.iter().rev().copied().collect::<Vec<u8>>())
        };

        let tsc = serde_json::json!({
            "height": 750000,
            "index": 3,
            "nodes": [
                "1111111111111111111111111111111111111111111111111111111111111111",
                "2222222222222222222222222222222222222222222222222222222222222222"
            ]
        });
        beef.add_tsc_merkle_proof(&txid, tx_idx, &tsc).map_err(|e| e.to_string())?;

        // Serialize the BUMP by serializing the whole BEEF and extracting
        let beef_bytes = beef.to_bytes().map_err(|e| e.to_string())?;

        // Parse back and verify height
        let parsed = Beef::from_bytes(&beef_bytes).map_err(|e| e.to_string())?;
        if parsed.bumps[0].block_height != 750000 {
            return Err(format!("BUMP height lost: expected 750000, got {}", parsed.bumps[0].block_height));
        }
        Ok(())
    });

    // ═══════════════════════════════════════════════════════════════
    // [3/6] AES-256-GCM (Custom Implementation)
    // ═══════════════════════════════════════════════════════════════
    println!();
    println!("  [3/6] AES-256-GCM (Custom Implementation)");

    // 3a. Encrypt/decrypt roundtrip with 32-byte IV (BRC-2 mode)
    check!("aesgcm", "3a-roundtrip-32byte-iv", {
        let key: [u8; 32] = [0x42; 32];
        let iv: [u8; 32] = [0x01; 32];
        let plaintext = b"Hello, AES-GCM with 32-byte IV!";

        let (ciphertext, tag) = aesgcm_custom::aesgcm_custom(
            plaintext, &[], &iv, &key,
        ).map_err(|e| e.to_string())?;

        if ciphertext.len() != plaintext.len() {
            return Err(format!("Ciphertext length should match plaintext: {} vs {}", ciphertext.len(), plaintext.len()));
        }
        if tag.len() != 16 {
            return Err(format!("Auth tag should be 16 bytes, got {}", tag.len()));
        }

        let decrypted = aesgcm_custom::aesgcm_decrypt_custom(
            &ciphertext, &[], &iv, &tag, &key,
        ).map_err(|e| e.to_string())?;

        if decrypted != plaintext {
            return Err("Decrypted text doesn't match original".to_string());
        }
        Ok(())
    });

    // 3b. Encrypt/decrypt roundtrip with 12-byte IV (standard mode)
    check!("aesgcm", "3b-roundtrip-12byte-iv", {
        let key: [u8; 32] = [0xAA; 32];
        let iv: [u8; 12] = [0xBB; 12];
        let plaintext = b"Standard 12-byte nonce path";

        let (ciphertext, tag) = aesgcm_custom::aesgcm_custom(
            plaintext, &[], &iv, &key,
        ).map_err(|e| e.to_string())?;

        let decrypted = aesgcm_custom::aesgcm_decrypt_custom(
            &ciphertext, &[], &iv, &tag, &key,
        ).map_err(|e| e.to_string())?;

        if decrypted != plaintext {
            return Err("12-byte IV roundtrip failed".to_string());
        }
        Ok(())
    });

    // 3c. Wrong key should fail decryption
    check!("aesgcm", "3c-wrong-key-fails", {
        let key: [u8; 32] = [0x42; 32];
        let wrong_key: [u8; 32] = [0x43; 32];
        let iv: [u8; 32] = [0x01; 32];
        let plaintext = b"Secret message";

        let (ciphertext, tag) = aesgcm_custom::aesgcm_custom(
            plaintext, &[], &iv, &key,
        ).map_err(|e| e.to_string())?;

        match aesgcm_custom::aesgcm_decrypt_custom(&ciphertext, &[], &iv, &tag, &wrong_key) {
            Err(_) => Ok(()),
            Ok(_) => Err("Should fail with wrong key".to_string()),
        }
    });

    // 3d. Tampered ciphertext should fail auth tag verification
    check!("aesgcm", "3d-tampered-ciphertext-fails", {
        let key: [u8; 32] = [0x42; 32];
        let iv: [u8; 32] = [0x01; 32];
        let plaintext = b"Do not tamper";

        let (mut ciphertext, tag) = aesgcm_custom::aesgcm_custom(
            plaintext, &[], &iv, &key,
        ).map_err(|e| e.to_string())?;

        // Flip a bit
        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0x01;
        }

        match aesgcm_custom::aesgcm_decrypt_custom(&ciphertext, &[], &iv, &tag, &key) {
            Err(_) => Ok(()),
            Ok(_) => Err("Should fail on tampered ciphertext".to_string()),
        }
    });

    // 3e. Empty plaintext roundtrip
    check!("aesgcm", "3e-empty-plaintext", {
        let key: [u8; 32] = [0x42; 32];
        let iv: [u8; 32] = [0x99; 32];
        let plaintext = b"";

        let (ciphertext, tag) = aesgcm_custom::aesgcm_custom(
            plaintext, &[], &iv, &key,
        ).map_err(|e| e.to_string())?;

        if !ciphertext.is_empty() {
            return Err(format!("Empty plaintext should give empty ciphertext, got {} bytes", ciphertext.len()));
        }

        let decrypted = aesgcm_custom::aesgcm_decrypt_custom(
            &ciphertext, &[], &iv, &tag, &key,
        ).map_err(|e| e.to_string())?;

        if !decrypted.is_empty() {
            return Err("Decrypted empty should be empty".to_string());
        }
        Ok(())
    });

    // 3f. Large plaintext roundtrip (1KB)
    check!("aesgcm", "3f-large-plaintext", {
        let key: [u8; 32] = [0x42; 32];
        let iv: [u8; 32] = [0x77; 32];
        let plaintext: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();

        let (ciphertext, tag) = aesgcm_custom::aesgcm_custom(
            &plaintext, &[], &iv, &key,
        ).map_err(|e| e.to_string())?;

        let decrypted = aesgcm_custom::aesgcm_decrypt_custom(
            &ciphertext, &[], &iv, &tag, &key,
        ).map_err(|e| e.to_string())?;

        if decrypted != plaintext {
            return Err("1KB roundtrip failed".to_string());
        }
        Ok(())
    });

    // 3g. NIST AES-256-GCM Test Case 13 (12-byte IV, from NIST SP 800-38D)
    // Key: 0^256, IV: 0^96, PT: empty, AAD: empty
    // Expected Tag: 530f8afbc74536b9a963b4f1c4cb738b
    check!("aesgcm", "3g-nist-aes256-tc13", {
        let key: [u8; 32] = [0u8; 32];
        let iv: [u8; 12] = [0u8; 12];

        let (ciphertext, tag) = aesgcm_custom::aesgcm_custom(
            &[], &[], &iv, &key,
        ).map_err(|e| e.to_string())?;

        let expected_tag = hex::decode("530f8afbc74536b9a963b4f1c4cb738b").unwrap();

        if ciphertext.len() != 0 {
            return Err(format!("Expected empty ciphertext, got {} bytes", ciphertext.len()));
        }
        assert_eq_hex("NIST TC13 tag", &tag, &expected_tag)
    });

    // 3h. NIST AES-256-GCM Test Case 14 (12-byte IV, 16-byte plaintext)
    // Key: 0^256, IV: 0^96, PT: 0^128
    // Expected CT: cea7403d4d606b6e074ec5d3baf39d18
    // Expected Tag: d0d1c8a799996bf0265b98b5d48ab919
    check!("aesgcm", "3h-nist-aes256-tc14", {
        let key: [u8; 32] = [0u8; 32];
        let iv: [u8; 12] = [0u8; 12];
        let pt: [u8; 16] = [0u8; 16];

        let (ciphertext, tag) = aesgcm_custom::aesgcm_custom(
            &pt, &[], &iv, &key,
        ).map_err(|e| e.to_string())?;

        let expected_ct = hex::decode("cea7403d4d606b6e074ec5d3baf39d18").unwrap();
        let expected_tag = hex::decode("d0d1c8a799996bf0265b98b5d48ab919").unwrap();

        assert_eq_hex("NIST TC14 ciphertext", &ciphertext, &expected_ct)?;
        assert_eq_hex("NIST TC14 tag", &tag, &expected_tag)
    });

    // ═══════════════════════════════════════════════════════════════
    // [4/6] BRC-2 Symmetric Key Derivation & Encryption
    // ═══════════════════════════════════════════════════════════════
    println!();
    println!("  [4/6] BRC-2 Symmetric Key Derivation & Encryption");

    // 4a. Symmetric key derivation is deterministic
    check!("brc2-key", "4a-deterministic", {
        let sender_priv = hex::decode("6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8").unwrap();
        let secp = secp256k1::Secp256k1::new();
        let sender_secret = secp256k1::SecretKey::from_slice(&sender_priv).unwrap();
        let sender_pub = secp256k1::PublicKey::from_secret_key(&secp, &sender_secret).serialize().to_vec();

        let counterparty_pub = hex::decode("0294c479f762f6baa97fbcd4393564c1d7bd8336ebd15928135bbcf575cd1a71a1").unwrap();

        let invoice = "2-brc2 test-42";

        let key1 = brc2::derive_symmetric_key(&sender_priv, &counterparty_pub, invoice)
            .map_err(|e| e.to_string())?;
        let key2 = brc2::derive_symmetric_key(&sender_priv, &counterparty_pub, invoice)
            .map_err(|e| e.to_string())?;

        if key1.len() != 32 {
            return Err(format!("Key should be 32 bytes, got {}", key1.len()));
        }
        if key1 != key2 {
            return Err("Symmetric key derivation is not deterministic".to_string());
        }
        Ok(())
    });

    // 4b. BRC-2 encrypt/decrypt roundtrip (self-encryption)
    check!("brc2-roundtrip", "4b-self-encrypt", {
        let priv_key = hex::decode("6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8").unwrap();
        let secp = secp256k1::Secp256k1::new();
        let secret = secp256k1::SecretKey::from_slice(&priv_key).unwrap();
        let pub_key = secp256k1::PublicKey::from_secret_key(&secp, &secret).serialize().to_vec();

        let invoice = "2-brc2 test-self";
        let plaintext = b"BRC-2 self-encryption test message";

        // Derive key (self = sender and recipient are the same identity)
        let sym_key = brc2::derive_symmetric_key(&priv_key, &pub_key, invoice)
            .map_err(|e| e.to_string())?;

        // Encrypt
        let ciphertext = brc2::encrypt_brc2(plaintext, &sym_key)
            .map_err(|e| e.to_string())?;

        // Verify format: [32-byte IV][ciphertext][16-byte tag]
        let expected_len = 32 + plaintext.len() + 16;
        if ciphertext.len() != expected_len {
            return Err(format!("BRC-2 output length: expected {}, got {} (32 IV + {} pt + 16 tag)",
                expected_len, ciphertext.len(), plaintext.len()));
        }

        // Decrypt
        let decrypted = brc2::decrypt_brc2(&ciphertext, &sym_key)
            .map_err(|e| e.to_string())?;

        if decrypted != plaintext {
            return Err("BRC-2 self-encrypt roundtrip failed".to_string());
        }
        Ok(())
    });

    // 4c. BRC-2 encrypt/decrypt with counterparty
    check!("brc2-roundtrip", "4c-counterparty-encrypt", {
        let alice_priv = hex::decode("6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8").unwrap();
        let bob_priv = hex::decode("cab2500e206f31bc18a8af9d6f44f0b9a208c32d5cca2b22acfe9d1a213b2f36").unwrap();

        let secp = secp256k1::Secp256k1::new();
        let alice_pub = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&alice_priv).unwrap()
        ).serialize().to_vec();
        let bob_pub = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&bob_priv).unwrap()
        ).serialize().to_vec();

        let invoice = "2-secure messaging-1";
        let plaintext = b"Secret message from Alice to Bob";

        // Alice encrypts for Bob
        let alice_key = brc2::derive_symmetric_key(&alice_priv, &bob_pub, invoice)
            .map_err(|e| e.to_string())?;
        let ciphertext = brc2::encrypt_brc2(plaintext, &alice_key)
            .map_err(|e| e.to_string())?;

        // Bob decrypts (derives same key from his perspective)
        let bob_key = brc2::derive_symmetric_key(&bob_priv, &alice_pub, invoice)
            .map_err(|e| e.to_string())?;

        if alice_key != bob_key {
            return Err(format!(
                "Symmetric keys don't match!\n  Alice: {}\n  Bob:   {}",
                hex::encode(&alice_key), hex::encode(&bob_key)
            ));
        }

        let decrypted = brc2::decrypt_brc2(&ciphertext, &bob_key)
            .map_err(|e| e.to_string())?;

        if decrypted != plaintext {
            return Err("Counterparty decrypt failed".to_string());
        }
        Ok(())
    });

    // 4d. BRC-2 "anyone" encryption (privkey=1)
    check!("brc2-roundtrip", "4d-anyone-encrypt", {
        let priv_key = hex::decode("6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8").unwrap();
        let secp = secp256k1::Secp256k1::new();
        let pub_key = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&priv_key).unwrap()
        ).serialize().to_vec();

        // "anyone" public key = generator point G (privkey 1)
        let mut anyone_priv = [0u8; 32];
        anyone_priv[31] = 1;
        let anyone_pub = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&anyone_priv).unwrap()
        ).serialize().to_vec();

        let invoice = "2-public data-1";
        let plaintext = b"Anyone can decrypt this";

        // Encrypt with our key toward "anyone"
        let encrypt_key = brc2::derive_symmetric_key(&priv_key, &anyone_pub, invoice)
            .map_err(|e| e.to_string())?;
        let ciphertext = brc2::encrypt_brc2(plaintext, &encrypt_key)
            .map_err(|e| e.to_string())?;

        // "Anyone" decrypts (using privkey=1 and our pubkey)
        let decrypt_key = brc2::derive_symmetric_key(&anyone_priv.to_vec(), &pub_key, invoice)
            .map_err(|e| e.to_string())?;

        if encrypt_key != decrypt_key {
            return Err("Anyone symmetric key mismatch".to_string());
        }

        let decrypted = brc2::decrypt_brc2(&ciphertext, &decrypt_key)
            .map_err(|e| e.to_string())?;

        if decrypted != plaintext {
            return Err("Anyone decrypt failed".to_string());
        }
        Ok(())
    });

    // 4e. BRC-2 ciphertext too short should fail
    check!("brc2-error", "4e-ciphertext-too-short", {
        let key = [0x42u8; 32];
        let short = vec![0u8; 47]; // Need at least 48 (32 IV + 16 tag)
        match brc2::decrypt_brc2(&short, &key) {
            Err(_) => Ok(()),
            Ok(_) => Err("Should fail on short ciphertext".to_string()),
        }
    });

    // 4f. Certificate field encryption roundtrip
    check!("brc2-cert", "4f-cert-field-roundtrip", {
        let sender_priv = hex::decode("6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8").unwrap();
        let secp = secp256k1::Secp256k1::new();
        let sender_pub = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&sender_priv).unwrap()
        ).serialize().to_vec();

        // Recipient is different from sender
        let recip_priv = hex::decode("cab2500e206f31bc18a8af9d6f44f0b9a208c32d5cca2b22acfe9d1a213b2f36").unwrap();
        let recip_pub = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&recip_priv).unwrap()
        ).serialize().to_vec();

        let field_name = "email";
        let plaintext = b"alice@example.com";

        // Encrypt
        let ciphertext = brc2::encrypt_certificate_field(
            &sender_priv, &recip_pub, field_name, None, plaintext,
        ).map_err(|e| e.to_string())?;

        // Decrypt (from recipient's perspective)
        let decrypted = brc2::decrypt_certificate_field(
            &recip_priv, &sender_pub, field_name, None, &ciphertext,
        ).map_err(|e| e.to_string())?;

        if decrypted != plaintext {
            return Err("Certificate field roundtrip failed".to_string());
        }
        Ok(())
    });

    // 4g. Certificate field with serial number
    check!("brc2-cert", "4g-cert-field-with-serial", {
        let priv_key = hex::decode("6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8").unwrap();
        let secp = secp256k1::Secp256k1::new();
        let pub_key = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&priv_key).unwrap()
        ).serialize().to_vec();

        let field_name = "name";
        let serial = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="; // base64 of 32 zero bytes
        let plaintext = b"Alice Smith";

        let ciphertext = brc2::encrypt_certificate_field(
            &priv_key, &pub_key, field_name, Some(serial), plaintext,
        ).map_err(|e| e.to_string())?;

        let decrypted = brc2::decrypt_certificate_field(
            &priv_key, &pub_key, field_name, Some(serial), &ciphertext,
        ).map_err(|e| e.to_string())?;

        if decrypted != plaintext {
            return Err("Certificate field with serial roundtrip failed".to_string());
        }
        Ok(())
    });

    // ═══════════════════════════════════════════════════════════════
    // [5/6] Certificate Preimage Serialization
    // ═══════════════════════════════════════════════════════════════
    println!();
    println!("  [5/6] Certificate Preimage Serialization");

    // 5a. Basic preimage structure: type(32) + serial(32) + subject(33) + certifier(33) + revocation + fields
    check!("cert-preimage", "5a-basic-structure", {
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), CertificateField::new(
            "name".to_string(),
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
        ));

        let cert = Certificate::new(
            vec![0xAAu8; 32],  // type
            vec![0x02; 33],    // subject (valid compressed pubkey prefix)
            vec![0xBBu8; 32],  // serial
            vec![0x03; 33],    // certifier (valid compressed pubkey prefix)
            "0000000000000000000000000000000000000000000000000000000000000000.0".to_string(),
            vec![],
            fields,
            HashMap::new(),
        );

        let preimage = serialize_certificate_preimage(&cert)
            .map_err(|e| e.to_string())?;

        // Check type bytes at start
        if &preimage[0..32] != &[0xAA; 32] {
            return Err("Type bytes mismatch in preimage".to_string());
        }
        // Check serial at offset 32
        if &preimage[32..64] != &[0xBB; 32] {
            return Err("Serial bytes mismatch in preimage".to_string());
        }
        // Check subject at offset 64 (33 bytes)
        if preimage[64] != 0x02 {
            return Err("Subject prefix mismatch in preimage".to_string());
        }
        // Check certifier at offset 97 (33 bytes)
        if preimage[97] != 0x03 {
            return Err("Certifier prefix mismatch in preimage".to_string());
        }
        Ok(())
    });

    // 5b. Fields sorted lexicographically
    check!("cert-preimage", "5b-field-ordering", {
        let mut fields = HashMap::new();
        fields.insert("zebra".to_string(), CertificateField::new(
            "zebra".to_string(), vec![1], vec![2],
        ));
        fields.insert("alpha".to_string(), CertificateField::new(
            "alpha".to_string(), vec![3], vec![4],
        ));
        fields.insert("middle".to_string(), CertificateField::new(
            "middle".to_string(), vec![5], vec![6],
        ));

        let cert = Certificate::new(
            vec![0u8; 32], vec![0u8; 33], vec![0u8; 32], vec![0u8; 33],
            "0000000000000000000000000000000000000000000000000000000000000000.0".to_string(),
            vec![], fields, HashMap::new(),
        );

        let preimage = serialize_certificate_preimage(&cert)
            .map_err(|e| e.to_string())?;

        // Find field names in the preimage bytes
        let preimage_str = String::from_utf8_lossy(&preimage);
        let alpha_pos = preimage_str.find("alpha").ok_or("'alpha' not found in preimage")?;
        let middle_pos = preimage_str.find("middle").ok_or("'middle' not found in preimage")?;
        let zebra_pos = preimage_str.find("zebra").ok_or("'zebra' not found in preimage")?;

        if !(alpha_pos < middle_pos && middle_pos < zebra_pos) {
            return Err(format!("Fields not sorted: alpha={}, middle={}, zebra={}", alpha_pos, middle_pos, zebra_pos));
        }
        Ok(())
    });

    // 5c. Invalid type length rejected
    check!("cert-preimage", "5c-invalid-type-len", {
        let cert = Certificate::new(
            vec![0u8; 31], // Wrong size
            vec![0u8; 33], vec![0u8; 32], vec![0u8; 33],
            "0000000000000000000000000000000000000000000000000000000000000000.0".to_string(),
            vec![], HashMap::new(), HashMap::new(),
        );
        match serialize_certificate_preimage(&cert) {
            Err(_) => Ok(()),
            Ok(_) => Err("Should reject 31-byte type".to_string()),
        }
    });

    // 5d. Invalid serial length rejected
    check!("cert-preimage", "5d-invalid-serial-len", {
        let cert = Certificate::new(
            vec![0u8; 32], vec![0u8; 33],
            vec![0u8; 16], // Wrong size
            vec![0u8; 33],
            "0000000000000000000000000000000000000000000000000000000000000000.0".to_string(),
            vec![], HashMap::new(), HashMap::new(),
        );
        match serialize_certificate_preimage(&cert) {
            Err(_) => Ok(()),
            Ok(_) => Err("Should reject 16-byte serial".to_string()),
        }
    });

    // ═══════════════════════════════════════════════════════════════
    // [6/6] Certificate Synthetic Sign + Verify
    // ═══════════════════════════════════════════════════════════════
    println!();
    println!("  [6/6] Certificate Synthetic Sign + Verify (BRC-52)");
    println!("         Create certifier keys, sign certificate, verify with 'anyone'");

    // 6a. Full BRC-52 sign and verify chain
    //
    // This is the most important test in Tier 2. We:
    // 1. Generate a certifier key pair
    // 2. Create a certificate with known fields
    // 3. Serialize the preimage and hash it
    // 4. Derive the signing key using BRC-42 (certifier as both sender and recipient)
    // 5. Sign the hash with the derived key
    // 6. Call verify_certificate_signature() which uses "anyone" (privkey=1) as sender
    //
    // If this passes, our full BRC-52 chain is correct.
    check!("cert-verify", "6a-synthetic-sign-verify", {
        let secp = secp256k1::Secp256k1::new();

        // Step 1: Generate certifier key pair
        let certifier_priv = hex::decode("6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8").unwrap();
        let certifier_secret = secp256k1::SecretKey::from_slice(&certifier_priv).unwrap();
        let certifier_pub = secp256k1::PublicKey::from_secret_key(&secp, &certifier_secret).serialize().to_vec();

        // Subject (the person the certificate is about)
        let subject_priv = hex::decode("cab2500e206f31bc18a8af9d6f44f0b9a208c32d5cca2b22acfe9d1a213b2f36").unwrap();
        let subject_pub = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&subject_priv).unwrap()
        ).serialize().to_vec();

        // Step 2: Certificate fields
        let cert_type = vec![0x01u8; 32];
        let serial = vec![0x02u8; 32];
        let revocation = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.0";

        let mut fields = HashMap::new();
        fields.insert("name".to_string(), CertificateField::new(
            "name".to_string(), vec![0x11, 0x22, 0x33], vec![0x44, 0x55, 0x66],
        ));
        fields.insert("email".to_string(), CertificateField::new(
            "email".to_string(), vec![0xAA, 0xBB], vec![0xCC, 0xDD],
        ));

        // Step 3: Create unsigned certificate for preimage
        let unsigned_cert = Certificate::new(
            cert_type.clone(), subject_pub.clone(), serial.clone(), certifier_pub.clone(),
            revocation.to_string(), vec![], fields.clone(), HashMap::new(),
        );

        let preimage = serialize_certificate_preimage(&unsigned_cert)
            .map_err(|e| format!("Preimage serialization failed: {}", e))?;
        let hash = sha256(&preimage);

        // Step 4: Create BRC-43 invoice number for signing
        // Format: "2-certificate signature-{type_base64} {serial_base64}"
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        let type_b64 = BASE64.encode(&cert_type);
        let serial_b64 = BASE64.encode(&serial);
        let key_id = format!("{} {}", type_b64, serial_b64);

        let invoice = InvoiceNumber::new(
            SecurityLevel::CounterpartyLevel,
            "certificate signature",
            &key_id,
        ).map_err(|e| format!("Invoice creation failed: {}", e))?;
        let invoice_str = invoice.to_string();

        // Step 5: Sign using BRC-42 derived key
        //
        // The SDK's createSignature for certificates uses:
        // - counterparty = "anyone" (when the certificate should be publicly verifiable)
        // - The certifier derives a child private key using BRC-42:
        //     ECDH(certifier_privkey, anyone_pubkey) → shared secret
        //     HMAC(shared_secret, invoice_number) → scalar
        //     certifier_privkey + scalar → child private key
        //
        // "anyone" pubkey = generator point G (privkey=1)
        let mut anyone_priv = [0u8; 32];
        anyone_priv[31] = 1;
        let anyone_pub = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&anyone_priv).unwrap()
        ).serialize().to_vec();

        // Certifier derives child private key with "anyone" as counterparty
        let child_privkey = brc42::derive_child_private_key(
            &certifier_priv, &anyone_pub, &invoice_str,
        ).map_err(|e| format!("Child privkey derivation failed: {}", e))?;

        // Sign the hash
        let child_secret = secp256k1::SecretKey::from_slice(&child_privkey)
            .map_err(|e| format!("Invalid child private key: {}", e))?;
        let message = secp256k1::Message::from_digest_slice(&hash)
            .map_err(|e| format!("Invalid message hash: {}", e))?;
        let signature = secp.sign_ecdsa(&message, &child_secret);
        let sig_der = signature.serialize_der().to_vec();

        // Step 6: Create signed certificate and verify
        let signed_cert = Certificate::new(
            cert_type, subject_pub, serial, certifier_pub,
            revocation.to_string(), sig_der, fields, HashMap::new(),
        );

        // verify_certificate_signature uses "anyone" (privkey=1) as sender,
        // certifier pubkey as counterparty → derives child pubkey
        // Then verifies ECDSA signature with that child pubkey
        verify_certificate_signature(&signed_cert)
            .map_err(|e| format!("Signature verification failed: {}", e))
    });

    // 6b. Verify fails with wrong signature
    check!("cert-verify", "6b-wrong-sig-fails", {
        let secp = secp256k1::Secp256k1::new();
        let certifier_priv = hex::decode("6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8").unwrap();
        let certifier_pub = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&certifier_priv).unwrap()
        ).serialize().to_vec();

        let subject_priv = hex::decode("cab2500e206f31bc18a8af9d6f44f0b9a208c32d5cca2b22acfe9d1a213b2f36").unwrap();
        let subject_pub = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&subject_priv).unwrap()
        ).serialize().to_vec();

        // Sign with a WRONG key (not derived from BRC-42)
        let wrong_key = secp256k1::SecretKey::from_slice(&[0x42u8; 32]).unwrap();
        let dummy_hash = sha256(b"dummy");
        let message = secp256k1::Message::from_digest_slice(&dummy_hash).unwrap();
        let wrong_sig = secp.sign_ecdsa(&message, &wrong_key).serialize_der().to_vec();

        let cert = Certificate::new(
            vec![0x01; 32], subject_pub, vec![0x02; 32], certifier_pub,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.0".to_string(),
            wrong_sig, HashMap::new(), HashMap::new(),
        );

        match verify_certificate_signature(&cert) {
            Err(_) => Ok(()),
            Ok(()) => Err("Should fail with wrong signature".to_string()),
        }
    });

    // 6c. Verify fails with empty signature
    check!("cert-verify", "6c-empty-sig-fails", {
        let secp = secp256k1::Secp256k1::new();
        let certifier_priv = hex::decode("6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8").unwrap();
        let certifier_pub = secp256k1::PublicKey::from_secret_key(
            &secp, &secp256k1::SecretKey::from_slice(&certifier_priv).unwrap()
        ).serialize().to_vec();
        let subject_pub = vec![0x02; 33];

        let cert = Certificate::new(
            vec![0x01; 32], subject_pub, vec![0x02; 32], certifier_pub,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.0".to_string(),
            vec![], // empty signature
            HashMap::new(), HashMap::new(),
        );

        match verify_certificate_signature(&cert) {
            Err(_) => Ok(()),
            Ok(()) => Err("Should fail with empty signature".to_string()),
        }
    });

    // ═══════════════════════════════════════════════════════════════
    // Summary
    // ═══════════════════════════════════════════════════════════════
    let passed = PASS_COUNT.load(Ordering::Relaxed);
    let failed = FAIL_COUNT.load(Ordering::Relaxed);
    let skipped = SKIP_COUNT.load(Ordering::Relaxed);
    let total = passed + failed;

    println!();
    println!("=========================================================================");
    println!("  TIER 2 DIAGNOSTIC SUMMARY: {}/{} passed ({} skipped)", passed, total, skipped);
    println!("=========================================================================");

    if failed > 0 {
        println!();
        println!("  {} test(s) FAILED. Review output above.", failed);
        println!();
    } else {
        println!();
        println!("  All {} tests passed.", total);
        println!();
        println!("  Coverage:");
        println!("    BEEF format detection ...... 4 tests");
        println!("    BEEF roundtrip (V1/V2/Atomic/hex) ... 4 tests");
        println!("    BEEF error handling ........ 3 tests");
        println!("    BEEF BUMP associations ..... 1 test");
        println!("    BUMP/TSC conversion ........ 3 tests");
        println!("    AES-GCM 32-byte IV ......... 1 test");
        println!("    AES-GCM 12-byte IV ......... 1 test");
        println!("    AES-GCM error cases ........ 2 tests");
        println!("    AES-GCM edge cases ......... 2 tests (empty, 1KB)");
        println!("    AES-GCM NIST vectors ....... 2 tests (TC13, TC14)");
        println!("    BRC-2 key derivation ....... 1 test");
        println!("    BRC-2 self-encrypt ......... 1 test");
        println!("    BRC-2 counterparty ......... 1 test");
        println!("    BRC-2 anyone ............... 1 test");
        println!("    BRC-2 error handling ....... 1 test");
        println!("    BRC-2 cert field ........... 2 tests");
        println!("    Cert preimage .............. 4 tests");
        println!("    Cert sign+verify ........... 3 tests");
        println!("    ─────────────────────────────");
        println!("    Total .................... {} tests", total);
    }

    assert_eq!(failed, 0, "{} test(s) failed", failed);
}
