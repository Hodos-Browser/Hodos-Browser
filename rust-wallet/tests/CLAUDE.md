# Rust Wallet Integration Test Suite

> 780+ integration tests validating cryptography, transaction handling, BEEF format, certificates, and storage against known-answer vectors (NIST, RFC, BIP, BRC specs). 55 `#[test]` functions accumulate ~686 `check!` assertions across 12 files.

## Overview

This directory contains 12 integration test files organized into tiers of increasing coverage specificity. Tests use a diagnostic pattern with atomic pass/fail counters and a `check!` macro for structured reporting. All tests run against the `hodos_wallet` crate's public API — no mocking, no test doubles. Test vectors come from authoritative sources: NIST SP 800-38D (AES-GCM), RFC 4231 (HMAC), TREZOR (BIP-39), bitcoin-sv/bitcoin-sv (sighash), and the BSV TypeScript SDK (BRC-42/43/52).

## Running Tests

```bash
cd rust-wallet
cargo test               # Run all tests
cargo test tier3          # Run a specific tier
cargo test --test tier9_beef_struct_serde_test  # Run a specific file
```

## Files

| File | Tests | Coverage Area |
|------|-------|---------------|
| `diagnostic_test.rs` | ~77 | Foundation: BRC-42 key derivation, HMAC-SHA256, SHA-256, AES-256-GCM (NIST KATs), BIP-39 (24 TREZOR vectors), BIP-32 (11 paths), ECDSA sign/verify, BRC-3 signatures, BRC-2 HMAC |
| `beef_crypto_cert_test.rs` | ~50 | BEEF format parsing/serialization/roundtrip, BUMP↔TSC proof conversion, AES-GCM (12-byte & 32-byte IVs), BRC-2 symmetric encryption, certificate preimage serialization, BRC-52 sign+verify |
| `sighash_transaction_test.rs` | ~535 | Varint encode/decode, TX serialization roundtrip (20+ txns from sighash_vectors.json), TXID calculation (genesis tx), BIP-143 preimage (7 SIGHASH types), 500+ bitcoin-sv ForkID sighash vectors |
| `tier3_edge_recovery_test.rs` | ~71 | Key derivation boundary conditions, ECDSA determinism & cross-verification, RFC 4231 HMAC vectors, PushDrop encode/decode at PUSHDATA boundaries, script parser corruption handling, BIP-0032 official vectors, GHASH known-answer |
| `tier4_protocol_integration_test.rs` | ~57 | BRC-42 ECDH (spec vectors + cross-key), BRC-43 invoice formatting, BRC-2 end-to-end encrypt/decrypt, PIN-based mnemonic encryption (PBKDF2+AES-GCM), TX build→sign→verify workflow, cross-module BRC-42→BRC-2→certificate roundtrip |
| `tier5_coverage_hardening_test.rs` | ~102 | BEEF advanced ops (find_txid, sort_topologically, extract_raw_tx_hex), script conversions, certificate JSON parsing edge cases, AES-GCM with AAD, BalanceCache lifecycle & thread safety, crypto error paths, BEEF error paths, address/PriceCache misc |
| `tier6_protocol_vectors_test.rs` | ~60 | NIST SP 800-38D AES-256-GCM gold-standard vectors, BRC-42 symmetric key derivation symmetry, SIGHASH edge cases (SINGLE overflow, NONE, ANYONECANPAY), BEEF validation, certificate preimage serialization, Base58Check address validation, RFC 4231 HMAC, GHASH KATs |
| `tier7_boundary_coverage_test.rs` | ~74 | PIN encrypt/decrypt edge cases (corrupted ciphertext, empty input), BRC-2 error paths & symmetry, DPAPI platform stubs, extract_input_outpoints, BUMP parse error paths, script parser OP_PUSHDATA boundaries, ActionStatus/TransactionStatus/ProvenTxReqStatus roundtrips, uncompressed pubkey conversion, PriceCache/BalanceCache TTL & invalidation |
| `tier8_types_recovery_storage_test.rs` | ~65 | OutPoint/Script/TxOutput/TxInput/Transaction construction & serialization, BIP32 path derivation (hardened vs normal, deep paths), address↔P2PKH script conversion, sweep transaction building (batching, dust, fees), ActionStorage CRUD (file-based persistence), JsonStorage wallet loading & key derivation |
| `tier9_beef_struct_serde_test.rs` | ~58 | Beef builder API, V1↔V2 serialization roundtrip, Atomic BEEF (BRC-95) encoding, ParsedTransaction parsing (inputs/outputs/scripts), ActionStatus/TransactionStatus/ProvenTxReqStatus JSON serde, UTXO serde, CacheError & PushDropError Display impls |
| `tier10_cert_verify_beef_validate_test.rs` | ~56 | Certificate type construction & is_active, preimage serialization with field ordering, signature verification with BRC-42 keys, BEEF parse_bump_hex_to_tsc, validate_beef_v1_hex spec compliance, DB helper conversions (address_to_address_info, output_to_fetcher_utxo) |
| `tier11_final_coverage_test.rs` | ~48 | compute_invoice_hmac determinism, Brc42Error/SigningError/ScriptParseError Display variants, DomainPermission defaults, PriceCache Default, BRC-43 normalize_protocol_id, cross-module pipeline (ECDH→HMAC→derive→sign→verify), TX build/serialize, BRC-2 AES, script generation |

## Test Pattern

All files follow a consistent diagnostic pattern:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

static PASS: AtomicUsize = AtomicUsize::new(0);
static FAIL: AtomicUsize = AtomicUsize::new(0);

macro_rules! check {
    ($label:expr, $cond:expr) => {
        if $cond {
            PASS.fetch_add(1, Ordering::Relaxed);
        } else {
            FAIL.fetch_add(1, Ordering::Relaxed);
            eprintln!("FAIL: {}", $label);
        }
    };
}

#[test]
fn test_section() {
    check!("descriptive label", some_condition);
    // ...
    assert_eq!(FAIL.load(Ordering::Relaxed), 0, "failures detected");
}
```

Tiers 6-11 split into multiple `#[test]` functions per section (e.g., `t6_01_nist_aesgcm`, `t6_02_brc42_symmetric_key`) with a dedicated summary test (e.g., `t6_zz_summary`) that sleeps 200ms then asserts zero failures. Tiers 1-5 use a single large `#[test]` function with internal sections; tier 5 delegates to helper functions but still has a single `#[test]` entry point.

## Modules Under Test

| Module | What's Tested |
|--------|---------------|
| `hodos_wallet::crypto::brc42` | `derive_child_private_key`, `derive_child_public_key`, `compute_invoice_hmac`, shared secret, error types |
| `hodos_wallet::crypto::brc43` | `InvoiceNumber`, `SecurityLevel`, `normalize_protocol_id` |
| `hodos_wallet::crypto::brc2` | `derive_symmetric_key`, `encrypt`, `decrypt` |
| `hodos_wallet::crypto::aesgcm_custom` | `encrypt`, `decrypt` with 12-byte and 32-byte IVs, AAD |
| `hodos_wallet::crypto::signing` | `sha256`, `sha256d`, `hmac_sha256`, `sign_message`, `verify_signature` |
| `hodos_wallet::crypto::keys` | `compress_public_key`, `decompress_public_key`, uncompressed conversion |
| `hodos_wallet::crypto::pin` | `encrypt_mnemonic`, `decrypt_mnemonic` |
| `hodos_wallet::crypto::dpapi` | `encrypt_data`, `decrypt_data` (platform stubs) |
| `hodos_wallet::crypto::ghash` | GHASH hash subkey computation |
| `hodos_wallet::transaction` | `Transaction`, `TxInput`, `TxOutput`, `OutPoint`, `Script`, `calculate_sighash`, `extract_input_outpoints`, varint encode/decode |
| `hodos_wallet::beef` | `Beef` builder/parser, V1/V2/Atomic serialization, `ParsedTransaction`, `validate_beef_v1_hex`, `parse_bump_hex_to_tsc`, `sort_topologically` |
| `hodos_wallet::certificate::types` | `Certificate`, `CertificateField` construction |
| `hodos_wallet::certificate::verifier` | `serialize_certificate_preimage`, `verify_certificate_signature` |
| `hodos_wallet::recovery` | `derive_key_at_path`, `derive_address_at_path`, `address_to_p2pkh_script`, `ExternalWalletConfig`, `build_sweep_transactions` |
| `hodos_wallet::script::pushdrop` | PushDrop encode/decode, `PushDropError` |
| `hodos_wallet::action_storage` | `ActionStorage`, `StoredAction`, `ActionStatus`, `TransactionStatus`, `ProvenTxReqStatus` |
| `hodos_wallet::json_storage` | `JsonStorage` wallet file operations |
| `hodos_wallet::price_cache` | `PriceCache` Default, TTL behavior |
| `hodos_wallet::balance_cache` | `BalanceCache` lifecycle, stale fallback, thread safety |
| `hodos_wallet::domain_permission` | `DomainPermission` defaults and field validation |
| `hodos_wallet::cache_errors` | `CacheError` Display and Error impls |

## Test Vector Sources

| Source | Used In | What |
|--------|---------|------|
| NIST SP 800-38D | diagnostic_test, tier6 | AES-256-GCM known-answer tests |
| RFC 4231 | diagnostic_test, tier3, tier6 | HMAC-SHA256 test vectors |
| TREZOR reference | diagnostic_test | 24 BIP-39 mnemonic→seed vectors |
| BIP-0032 spec | diagnostic_test, tier3 | HD key derivation paths |
| BIP-143 spec | sighash_transaction_test | Sighash preimage vectors (7 SIGHASH types) |
| bitcoin-sv/bitcoin-sv | sighash_transaction_test | 500+ ForkID sighash vectors (`sighash_vectors.json`) |
| BSV TypeScript SDK | diagnostic_test, tier4 | BRC-42, BRC-3, BRC-2 compliance vectors |
| NIST AES-256 | tier3 | GHASH hash subkey known-answer |

## Adding New Tests

Follow the established tier pattern:

1. Use `check!` macro with descriptive labels for each assertion
2. Use atomic `PASS`/`FAIL` counters for structured reporting
3. End each `#[test]` function with `assert_eq!(FAIL.load(...), 0)`
4. Import from `hodos_wallet::` — test the public API, not internals
5. Include authoritative test vectors where available (NIST, RFC, BIP)
6. Name files `tier{N}_{description}_test.rs` for coverage-hardening tests

## Related

- [`../CLAUDE.md`](../CLAUDE.md) — Rust wallet architecture, build instructions, invariants
- [`../src/crypto/CLAUDE.md`](../src/crypto/CLAUDE.md) — Cryptography module details
- [`../src/certificate/CLAUDE.md`](../src/certificate/CLAUDE.md) — Certificate types and verification
- [`../src/transaction/CLAUDE.md`](../src/transaction/CLAUDE.md) — Transaction module details
- [`../src/script/CLAUDE.md`](../src/script/CLAUDE.md) — Script parsing and PushDrop
- [`../../CLAUDE.md`](../../CLAUDE.md) — Project-wide context and architecture
