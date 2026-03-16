# Test Scripts (Archived)
> Archived debugging and interoperability test scripts used during Rust wallet BRC-100 protocol implementation.

## Overview

This directory contains 46 ad-hoc test scripts created during development of the Rust wallet's BRC-100 protocol suite. They validate byte-level compatibility between the Rust wallet implementation and the TypeScript SDK reference (`@bsv/sdk`). These scripts are **archived** — they were critical during initial protocol implementation but are not part of the active test suite.

The primary goal was ensuring the Rust wallet produces identical outputs (CSR bodies, serialized requests, auth headers, encrypted fields, signatures) to the TypeScript SDK, which serves as the reference implementation for BRC-100/BRC-31/BRC-2 protocols.

**Note**: JavaScript scripts expect the TS SDK at `reference/ts-brc100/node_modules/@bsv/sdk` relative to `rust-wallet/`. PowerShell scripts target `localhost:31301` (Rust wallet HTTP API).

## Files

### CSR (Certificate Signing Request) Validation

| File | Purpose |
|------|---------|
| `analyze_csr_body.js` | Parses hex-encoded CSR JSON; verifies field order (clientNonce → type → fields → masterKeyring) |
| `compare_csr_bytes.js` | Parses VarInt-encoded serialized request binary format; displays structure breakdown |
| `compare_csr_with_ts_sdk.js` | Generates TS SDK CSR and compares field order, JSON length, and values against Rust output |
| `find_csr_difference.js` | Side-by-side CSR JSON comparison; identifies field-level differences between Rust and TS SDK |
| `generate_working_csr.js` | Generates reference CSR using `MasterCertificate.createCertificateFields` API |
| `test_csr_comparison_ts.js` | Generates CSR with hex/base64 output for manual comparison |
| `test_csr_format_comparison.js` | Validates CSR JSON field order programmatically |
| `test_csr_serialization.js` | Demonstrates exact VarInt serialization format with section-by-section breakdown |
| `test_ts_sdk_csr_comparison.js` | Full CSR generation + request serialization matching AuthFetch internals |
| `test_ts_sdk_exact_comparison.js` | Outputs exact bytes for byte-for-byte comparison with Rust implementation |
| `csr_byte_comparison_test.rs` | Rust test outputting serialized request bytes for cross-language comparison |
| `csr_serialization_ts_sdk.json` | Reference serialization output (371 bytes: nonce + method + path + search + headers + body) |

### Authentication & Headers (BRC-31)

| File | Purpose |
|------|---------|
| `capture_header_values.js` | Captures exact BRC-31 auth headers from TS SDK (version, identity-key, nonce, signature) |
| `capture_metanet_requests.js` | HTTP proxy intercepting metanet-client requests; logs all headers and bodies |
| `parse_and_compare_headers.js` | Parses Rust logs and validates header formats; detects signing-vs-request nonce mix-ups |
| `test_authfetch_headers.js` | Spins up test server to capture AuthFetch headers from TS SDK during 401 challenge flow |

### Encryption & Decryption (BRC-2)

| File | Purpose |
|------|---------|
| `test_decrypt_our_encryption.js` | Verifies Rust-encrypted values can be decrypted by TS SDK (server perspective) |
| `test_decrypt_rust_encrypted.js` | Simulates server-side CSR decryption; parses IV+ciphertext+tag (32+32+16 byte format) |
| `test_encryption_roundtrip.js` | Full encrypt→decrypt cycle: `createCertificateFields` → `decryptFields` |
| `test_server_decryption.js` | Server-side decryption of client CSR with detailed step logging |
| `test_side_by_side.js` | Step-by-step SDK encryption workflow with intermediate key material output |

### Certificate & Signature Verification

| File | Purpose |
|------|---------|
| `test_certificate_verification.js` | Creates test certificate, verifies with `Certificate.toBinary()` + ProtoWallet |
| `test_sdk_sign_verify.js` | Signs certificate with certifier wallet, verifies with "anyone" wallet; tests `forSelf` parameter |
| `test_derive_public_key.js` | Tests `KeyDeriver.derivePublicKey` with various counterparty/forSelf combinations |

### Encoding & Format Utilities

| File | Purpose |
|------|---------|
| `decode_hex.js` | Simple hex→UTF-8 decoder for debugging serialized data |
| `extract_and_compare.js` | Extracts hex from Rust log files and parses serialized request structure |
| `test_base64_decode.js` | Tests base64 concatenation edge cases in `base64ToArray` |
| `test_varint_encoding.js` | Validates VarInt encoding for positive integers, negatives (-1, -2), and edge cases (252, 253, 65535) |

### Server Integration

| File | Purpose |
|------|---------|
| `test_ts_sdk_server.js` | Full BRC-53 certifier server simulation; handles initialRequest/initialResponse, CSR decryption, field validation. Most comprehensive test (537 lines) |
| `test_interoperability_ts.js` | Encrypts data with TS SDK and outputs test vectors for Rust interoperability tests |

### PowerShell Test Scripts (Windows)

| File | Purpose |
|------|---------|
| `test_acquire_certificate.ps1` | Tests certificate acquisition flow against Rust wallet HTTP API |
| `test_actions.ps1` | Tests BRC-100 action creation endpoints |
| `test_beef_phase2.ps1` | Tests BEEF (Background Evaluation Extended Format) structure phase 2 |
| `test_beef_structure.ps1` | Tests BEEF binary parsing |
| `test_endpoints.ps1` | Tests Rust wallet HTTP endpoint availability |
| `test_internalize.ps1` | Tests transaction internalization flow |
| `test_labels.ps1` | Tests label CRUD operations |
| `test_parse_generated_beef.ps1` | Tests BEEF generation output |
| `test_pushdrop_cross_validation.ps1` | Cross-validates PushDrop script encoding between Rust and TS SDK |
| `test_transaction_flow.ps1` | Tests end-to-end transaction creation flow |
| `test_server_connection.ps1` | Tests basic server connectivity |
| `test_interoperability.ps1` | PowerShell interoperability tests |
| `delete_db.ps1` | Utility: deletes wallet SQLite database for clean test runs |
| `extract_key.ps1` | Utility: extracts key material for debugging |

### Shell Scripts

| File | Purpose |
|------|---------|
| `test_acquire_certificate.sh` | Bash version of certificate acquisition test (curl-based) |

### Test Vector Data

| File | Purpose |
|------|---------|
| `csr_serialization_ts_sdk.json` | Reference CSR serialization output (371 bytes) with hex/base64/breakdown showing nonce + method + path + search + headers + body structure |
| `pushdrop_test_vectors.json` | 8 test cases for PushDrop script encoding/decoding: empty fields, single/multiple fields, special opcodes (OP_0, OP_1–OP_16), large fields |

## Key Testing Patterns

### 1. TypeScript SDK as Reference Implementation
All JavaScript test scripts use `@bsv/sdk` as the ground truth. The pattern is:
- Generate output using TS SDK
- Compare byte-for-byte with Rust wallet output
- Log differences for debugging

### 2. VarInt Serialization Format
The BRC-31 request serialization uses VarInt encoding throughout:
```
[32-byte nonce] [varint method] [varint path] [varint search (-1 if empty)]
[varint header_count] [varint key, varint value]... [varint body]
```

### 3. BRC-2 Encryption Structure
Encrypted field values follow the format:
```
[32-byte IV] [32-byte ciphertext] [16-byte AES-GCM tag]
```
Revelation keys in `masterKeyring` use BRC-42 ECDH-derived symmetric keys.

### 4. PowerShell Scripts Target Wallet HTTP API
PowerShell scripts test the Rust wallet's HTTP endpoints on `localhost:31301` directly, verifying request/response formats for BRC-100 operations.

## Running Scripts

These are archived and not intended for regular use. If needed for debugging:

```bash
# JavaScript (requires TS SDK at reference/ts-brc100/node_modules/@bsv/sdk)
cd rust-wallet && node archive/test-scripts/<script>.js

# PowerShell (requires Rust wallet running on localhost:31301)
cd rust-wallet/archive/test-scripts && pwsh ./<script>.ps1

# Shell
cd rust-wallet/archive/test-scripts && bash test_acquire_certificate.sh
```

## Dependencies

- **JavaScript scripts**: `@bsv/sdk` (TypeScript SDK), Node.js built-ins (`http`, `https`, `fs`, `crypto`, `readline`)
- **Rust test**: Standard Rust test harness
- **PowerShell scripts**: `Invoke-RestMethod`, `Invoke-WebRequest`
- **Shell scripts**: `curl`

## Related

- `rust-wallet/src/authfetch.rs` — BRC-103 AuthFetch client (what these scripts validate)
- `rust-wallet/src/crypto/` — Rust crypto modules tested against TS SDK
- `rust-wallet/src/handlers.rs` — HTTP endpoints tested by PowerShell scripts
- Root `CLAUDE.md` — Project architecture and conventions
