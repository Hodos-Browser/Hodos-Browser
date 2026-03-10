# Old Tests (Archived)
> Archived Rust integration tests for BRC-2 encryption and CSR serialization interoperability with the TypeScript SDK.

## Overview

This directory contains 3 archived Rust test files created during development of the wallet's BRC-2 encryption and certificate signing request (CSR) serialization. They verify that the Rust wallet's encryption output is compatible with the TypeScript SDK reference implementation (`@bsv/sdk`), focusing on AES-GCM encryption format, certificate field encryption/decryption roundtrips, and JSON serialization byte-level matching. These tests are **archived** — they served as debugging aids during initial protocol implementation and are not part of the active test suite.

## Files

| File | Purpose |
|------|---------|
| `certificate_decryption_test.rs` | Tests BRC-2 certificate field encryption roundtrip using `brc2::encrypt_certificate_field` and `brc2::decrypt_certificate_field`; verifies 80-byte encrypted output format (32-byte IV + 32-byte ciphertext + 16-byte auth tag) |
| `csr_json_serialization_test.rs` | Validates CSR JSON serialization: field ordering (`clientNonce` → `type` → `fields` → `masterKeyring`), compact output (no whitespace), and exact byte count (345 bytes for reference CSR) matching TypeScript `JSON.stringify()` |
| `interoperability_test.rs` | Cross-language encryption interoperability: decrypts TypeScript-encrypted data in Rust (`aesgcm_custom::aesgcm_decrypt_custom`), encrypts in Rust for TypeScript decryption (`aesgcm_custom::aesgcm_custom`), full BRC-2 field encryption flow, and revelation key leading-zero stripping to match TS SDK `toArray()` behavior |

## Key Functions Tested

| Function | Module | What It Does |
|----------|--------|--------------|
| `brc2::encrypt_certificate_field` | `hodos_wallet::brc2` | Encrypts a certificate field value using BRC-42 ECDH-derived AES-256-GCM key |
| `brc2::decrypt_certificate_field` | `hodos_wallet::brc2` | Decrypts a certificate field value using the reverse key pair |
| `aesgcm_custom::aesgcm_custom` | `hodos_wallet::aesgcm_custom` | Low-level AES-GCM encryption with 32-byte IV (non-standard IV size for BRC-2 compatibility) |
| `aesgcm_custom::aesgcm_decrypt_custom` | `hodos_wallet::aesgcm_custom` | Low-level AES-GCM decryption matching the custom IV format |

## Test Details

### certificate_decryption_test.rs (1 test)

`test_certificate_field_encryption_roundtrip` — Simulates the certifier server's CSR decryption flow:
1. Encrypts a revelation key using the subject's private key and certifier's public key
2. Verifies the encrypted output is exactly 80 bytes: `[32-byte IV][32-byte ciphertext][16-byte tag]`
3. Attempts roundtrip decryption (expected to fail since real decryption requires the certifier's private key)

### csr_json_serialization_test.rs (3 tests)

- `test_csr_json_field_ordering` — Verifies `serde_json` preserves insertion order (matching TS SDK's `JSON.stringify()` field order)
- `test_csr_json_byte_comparison` — Asserts exact 345-byte JSON output for a reference CSR with known base64 values
- `test_json_stringify_comparison` — Confirms compact serialization (no newlines, no extra whitespace)

### interoperability_test.rs (4 tests + 1 helper)

- `test_decrypt_typescript_encrypted_field` — Decrypts a hardcoded base64 value encrypted by the TS SDK using a known symmetric key; asserts plaintext is `"true"`
- `test_encrypt_for_typescript_decryption` — Encrypts `"true"` in Rust, outputs base64 for manual TS SDK verification, roundtrip-decrypts to confirm
- `test_brc2_field_encryption_interoperability` — Full BRC-2 flow: `encrypt_certificate_field` → format verification → `decrypt_certificate_field` with test key pairs
- `test_revelation_key_stripping` — Validates `strip_leading_zeros()` helper matches TS SDK `toArray()` behavior: strips leading `0x00` bytes but preserves at least one byte

Helper: `strip_leading_zeros(bytes: &[u8]) -> Vec<u8>` — Removes leading zero bytes from revelation keys, keeping at least 1 byte (all-zeros returns `[0x00]`).

## BRC-2 Encryption Format

All tests validate this wire format for encrypted certificate fields:

```
[32-byte IV] [N-byte ciphertext] [16-byte AES-GCM auth tag]
```

For a 32-byte revelation key, the total encrypted output is 80 bytes (32 + 32 + 16).

Key derivation uses BRC-42 ECDH: sender private key + recipient public key + invoice number derived from field name → symmetric AES-256-GCM key.

## Dependencies

- `hodos_wallet::brc2` — BRC-2 certificate field encryption/decryption
- `hodos_wallet::aesgcm_custom` — Custom AES-GCM with 32-byte IV
- `serde_json` — JSON serialization
- `base64` — Base64 encoding/decoding (STANDARD engine)
- `hex` — Hex encoding/decoding
- `rand` — Random IV generation

## Related

- `rust-wallet/archive/test-scripts/CLAUDE.md` — Sibling archive: 46 JavaScript/PowerShell test scripts testing the same protocols from the TS SDK side
- `rust-wallet/src/crypto/brc2.rs` — Production BRC-2 encryption module these tests validate
- `rust-wallet/src/crypto/aesgcm_custom.rs` — Custom AES-GCM implementation tested here
- `rust-wallet/src/crypto/brc42.rs` — BRC-42 ECDH key derivation used by BRC-2
- Root `CLAUDE.md` — Project architecture and conventions
