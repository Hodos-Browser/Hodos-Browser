# Rust Wallet Full Test Suite Results — 2026-03-01

**992 tests, 0 failures, 0 wallet code modifications**

Run environment: WSL2 on Windows, `cargo test` with `--nocapture`, clean build after `cargo clean`.

---

## Summary

| Tier | Test File | Tests | Passed | Failed | Focus |
|------|-----------|-------|--------|--------|-------|
| 1 | `sighash_transaction_test.rs` | 367 | 367 | 0 | SigHash (FORKID), Transaction serialization, Varint encoding |
| 2 | `beef_crypto_cert_test.rs` | 37 | 37 | 0 | BEEF parsing, AES-256-GCM, BRC-2 encryption, Certificate preimage/verify |
| 3 | `tier3_edge_recovery_test.rs` | 70 | 70 | 0 | Key edge cases, ECDSA signing, Hashing, PushDrop, Script parser, BIP32, GHASH |
| 4 | `tier4_protocol_integration_test.rs` | 56 | 56 | 0 | BRC-42 spec vectors, BRC-43 invoice numbers, BRC-2 E2E, PIN encryption, Tx workflow, Cross-module |
| 5 | `tier5_coverage_hardening_test.rs` | 102 | 102 | 0 | BEEF advanced, Tx types, Cert parser, AES-GCM edges, BalanceCache, Error paths |
| 6 | `tier6_protocol_vectors_test.rs` | 70 | 70 | 0 | NIST SP 800-38D AES-256-GCM KATs, BRC-42 symmetry, SIGHASH edges, BEEF V1 validation, Topological sort, Certificate preimage, Base58Check, RFC 4231 HMAC, GHASH |
| 7 | `tier7_boundary_coverage_test.rs` | 71 | 71 | 0 | PIN edges, BRC-2 error paths, DPAPI stubs, extract_input_outpoints, BUMP parser, Script parser edges, Status types, Keys coverage, PriceCache/BalanceCache |
| 8 | `tier8_types_recovery_storage_test.rs` | 57 | 57 | 0 | Transaction type serialization, BIP32 path derivation, address_to_p2pkh_script, build_sweep_transactions, ActionStorage CRUD, JsonStorage operations |
| 9 | `tier9_beef_struct_serde_test.rs` | 58 | 58 | 0 | BEEF struct methods, Atomic BEEF, ParsedTransaction, Serde roundtrips, Error type coverage |
| 10 | `tier10_cert_verify_beef_validate_test.rs` | 56 | 56 | 0 | Certificate preimage/signature verification, BUMP-to-TSC conversion, BEEF V1 validation, Database helpers |
| 11 | `tier11_final_coverage_test.rs` | 48 | 48 | 0 | compute_invoice_hmac, Error Display traits (Brc42/Signing/ScriptParse), DomainPermission defaults, PriceCache, BRC-43 edges, Cross-module integration |
| **Total** | **11 files** | **992** | **992** | **0** | |

---

## Module Coverage

### Crypto (`src/crypto/`)

| Module | Functions Tested | Status |
|--------|-----------------|--------|
| `brc42` | `derive_child_private_key`, `derive_child_public_key`, `compute_shared_secret`, `compute_invoice_hmac`, `derive_symmetric_key_for_hmac` | PASS |
| `brc43` | `InvoiceNumber::new`, `from_string`, `to_string`, `SecurityLevel`, `normalize_protocol_id` | PASS |
| `signing` | `sign_ecdsa`, `verify_signature`, `sha256`, `double_sha256`, `hmac_sha256`, `verify_hmac_sha256` | PASS |
| `aesgcm_custom` | `aesgcm_custom` (encrypt), `aesgcm_decrypt_custom`, NIST KAT vectors (TC13-TC16) | PASS |
| `brc2` | `derive_key`, `encrypt`, `decrypt`, `encrypt_certificate_field`, `decrypt_certificate_field` | PASS |
| `pin` | `derive_key_from_pin`, `encrypt_with_pin`, `decrypt_with_pin` | PASS |
| `keys` | `derive_public_key`, `derive_public_key_uncompressed` | PASS |
| `ghash` | `generate_hash_subkey`, `ghash` | PASS |
| `dpapi` | `encrypt_data`, `decrypt_data` (Windows DPAPI roundtrip) | PASS |

### Transaction (`src/transaction/`)

| Module | Functions Tested | Status |
|--------|-----------------|--------|
| `types` | `OutPoint`, `TxInput`, `TxOutput`, `Transaction`, `Script` (new, from_hex, p2pkh_locking, p2pkh_unlocking) | PASS |
| `sighash` | `SigHash::calculate` (ALL, NONE, SINGLE, ANYONECANPAY combos, FORKID) | PASS |
| Serialization | `serialize`, `txid`, `to_hex`, `extract_input_outpoints`, `encode_varint`, `decode_varint`, `encode_varint_signed` | PASS |

### BEEF (`src/beef.rs`)

| Functions Tested | Status |
|-----------------|--------|
| `Beef::new`, `from_hex`, `from_bytes`, `to_hex`, `to_bytes`, `to_v1_hex`, `to_v1_bytes` | PASS |
| `from_atomic_beef_base64`, `from_atomic_beef_bytes`, `to_atomic_beef_hex` | PASS |
| `main_transaction`, `parent_transactions`, `has_proofs`, `find_txid` | PASS |
| `add_parent_transaction`, `set_main_transaction`, `add_tsc_merkle_proof` | PASS |
| `sort_topologically`, `extract_raw_tx_hex` | PASS |
| `parse_bump_hex_to_tsc`, `validate_beef_v1_hex` | PASS |
| `ParsedTransaction::from_bytes`, `from_hex` | PASS |

### Certificate (`src/certificate/`)

| Module | Functions Tested | Status |
|--------|-----------------|--------|
| `types` | `Certificate::new`, `identifier`, `is_active`, `CertificateField::new`, `CertificateError` Display | PASS |
| `verifier` | `serialize_certificate_preimage`, `verify_certificate_signature`, `verify_certificate_signature_with_keyid` | PASS |
| Parser | JSON parsing, field validation, revocation outpoint, keyring | PASS |

### Storage & Database

| Module | Functions Tested | Status |
|--------|-----------------|--------|
| `action_storage` | `ActionStorage` CRUD, `ActionStatus`, `TransactionStatus`, `ProvenTxReqStatus`, `update_confirmations` | PASS |
| `json_storage` | `JsonStorage` wallet loading, address retrieval, key derivation | PASS |
| `database::helpers` | `address_to_address_info`, `output_to_fetcher_utxo` | PASS |
| `database::models` | `DomainPermission::defaults`, `CertFieldPermission` | PASS |

### Other Modules

| Module | Functions Tested | Status |
|--------|-----------------|--------|
| `recovery` | `derive_private_key_bip32` (BIP32 HD derivation, Centbee paths) | PASS |
| `balance_cache` | `BalanceCache` set/get/invalidate/get_or_stale, thread safety | PASS |
| `price_cache` | `PriceCache::new`, `default`, `get_cached`, `get_stale` | PASS |
| `cache_errors` | `CacheError` variants, From impls, Display | PASS |
| `utxo_fetcher` | `UTXO` struct serde roundtrip | PASS |
| `script` | `parse_script_chunks`, `PushDrop` encode/decode, `ScriptParseError` Display | PASS |

---

## Not Tested (by design)

These modules/functions require database connections, network access, or async runtimes that are not available in pure integration tests:

| Module | Reason |
|--------|--------|
| `handlers.rs` (HTTP endpoints) | Requires running Actix-web server + SQLite database |
| `monitor/` (background tasks) | Requires database + network (WhatsOnChain, ARC) |
| `certificate::selective_disclosure` | `create_keyring_for_verifier` requires DB Connection + CertificateRepository |
| `database/*_repo.rs` (repositories) | Require SQLite connections |
| `recovery::recover_wallet_from_mnemonic` | Async, requires network for UTXO scanning |
| `beef_helpers.rs` | Recursive BEEF building requires DB + network |
| `auth_session.rs` | Not exported from lib.rs |
| `message_relay.rs` | Not exported from lib.rs |
| `fee_rate_cache.rs` | Not exported from lib.rs |
| `backup.rs` | Not exported from lib.rs |

---

## Test Methodology

- **Collect first, fix later**: Tests use a `check!` macro that records PASS/FAIL counts and continues execution rather than aborting on first failure
- **Zero wallet modifications**: All 992 tests were written against the existing wallet API without any changes to production code
- **Real protocol vectors**: BRC-42/43 spec vectors, NIST SP 800-38D AES-256-GCM Known Answer Tests, RFC 4231 HMAC-SHA256 test vectors, BRC-62 spec transactions
- **Cross-module integration**: Tier 4, 6, and 11 include end-to-end pipelines (ECDH → HMAC → derive → sign → verify, tx build → serialize → parse → BEEF)

---

## How to Run

### All tiers:
```bash
cargo test --test sighash_transaction_test \
  --test beef_crypto_cert_test \
  --test tier3_edge_recovery_test \
  --test tier4_protocol_integration_test \
  --test tier5_coverage_hardening_test \
  --test tier6_protocol_vectors_test \
  --test tier7_boundary_coverage_test \
  --test tier8_types_recovery_storage_test \
  --test tier9_beef_struct_serde_test \
  --test tier10_cert_verify_beef_validate_test \
  --test tier11_final_coverage_test \
  -- --nocapture
```

### Single tier:
```bash
cargo test --test tier11_final_coverage_test -- --nocapture
```

### If linter shows false errors:
```bash
cargo clean
cargo check --tests
```
