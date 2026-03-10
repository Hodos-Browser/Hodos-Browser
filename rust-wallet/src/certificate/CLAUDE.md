# Certificate — BRC-52 Identity Certificates

> Parsing, verification, selective disclosure, and data structures for BRC-52 identity certificates.

## Overview

This module implements the BRC-52 identity certificate protocol for the HodosBrowser wallet. Certificates bind identity fields (name, email, etc.) to a subject's public key, signed by a certifier. Fields are encrypted with BRC-2 (AES-256-GCM) and can be selectively disclosed to verifiers via BRC-53 keyrings. Revocation is on-chain — spending a designated UTXO revokes the certificate.

**Architecture**: Pure domain logic — no HTTP handlers or database connections (except `selective_disclosure.rs` which reads certificate fields from the DB). The HTTP layer lives in `src/handlers/certificate_handlers.rs`; persistence lives in `src/database/certificate_repo.rs`.

**Security**: Certificate field values are always encrypted. Master keyring entries (used for selective disclosure) are encrypted with BRC-2 using the subject/certifier key pair. Private keys are never stored in this module.

## Files

| File | Purpose |
|------|---------|
| `mod.rs` | Module exports: re-exports `Certificate`, `CertificateField`, `CertificateError`, `parse_certificate_from_json`, `serialize_certificate_preimage`, `verify_certificate_signature`, `check_revocation_status` |
| `types.rs` | Core data structures: `Certificate`, `CertificateField`, `CertificateError` enum (10 variants) |
| `parser.rs` | `parse_certificate_from_json()` — parses BRC-52 JSON from `acquireCertificate` 'direct' protocol |
| `verifier.rs` | `serialize_certificate_preimage()`, `verify_certificate_signature()`, `verify_certificate_signature_with_keyid()`, `check_revocation_status()` |
| `selective_disclosure.rs` | `create_keyring_for_verifier()` — BRC-53 selective field disclosure via verifier-specific keyrings |
| `test_utils.rs` | Test helpers: `create_test_certificate()`, `create_minimal_test_certificate()`, `create_test_certificate_json()`, `create_test_certificate_fields()`, random generators |

## Key Exports

### Types (`types.rs`)

**`Certificate`** — Maps to `certificates` table. Core fields:
- `type_: Vec<u8>` — 32-byte certificate type (base64-decoded)
- `subject: Vec<u8>` — 33-byte compressed public key (subject's identity key)
- `serial_number: Vec<u8>` — 32-byte unique serial (base64-decoded)
- `certifier: Vec<u8>` — 33-byte compressed public key (certifier's key)
- `verifier: Option<Vec<u8>>` — Optional 33-byte validation key
- `revocation_outpoint: String` — Format `"txid.vout"`, spending this UTXO revokes the cert
- `signature: Vec<u8>` — DER-encoded ECDSA signature
- `fields: HashMap<String, CertificateField>` — Encrypted field values
- `keyring: HashMap<String, Vec<u8>>` — Master keyring (fieldName → encrypted revelation key)
- `identifier()` → `(&[u8], &[u8], &[u8])` — Returns `(type, serialNumber, certifier)` tuple for lookups
- `is_active()` → `bool` — `true` if not soft-deleted

**`CertificateField`** — Maps to `certificate_fields` table:
- `field_name: String` — e.g. "name", "email", "age"
- `field_value: Vec<u8>` — BRC-2 encrypted value (base64-decoded bytes)
- `master_key: Vec<u8>` — Master keyring entry for this field (base64-decoded)

**`CertificateError`** — 10 error variants: `InvalidFormat`, `InvalidField`, `MissingField`, `InvalidBase64`, `InvalidHex`, `InvalidPublicKey`, `Database`, `SignatureVerification`, `Revoked`, `Relinquished`

### Parser (`parser.rs`)

**`parse_certificate_from_json(json_data: &Value) -> Result<Certificate, CertificateError>`**

Parses BRC-52 certificate JSON received via `acquireCertificate` 'direct' protocol. Validates:
- `type` — base64, must decode to exactly 32 bytes
- `serialNumber` — base64, must decode to exactly 32 bytes
- `certifier` / `subject` — hex, must decode to exactly 33 bytes (compressed pubkey)
- `revocationOutpoint` — format `"txid.vout"`, txid must be 64 hex chars
- `signature` — hex, must not be empty
- `fields` — map of fieldName → base64 encrypted values (field names max 50 bytes)
- `keyringForSubject` — optional map of fieldName → base64 keyring values; if present, every key must have a corresponding entry in `fields`
- `verifier` / `validationKey` — optional 33-byte hex public key

### Verifier (`verifier.rs`)

**`serialize_certificate_preimage(certificate: &Certificate) -> Result<Vec<u8>, CertificateError>`**

Builds the binary preimage for signature verification per BRC-52 spec. Field order:
1. `type` (32 bytes)
2. `serialNumber` (32 bytes)
3. `subject` (33 bytes)
4. `certifier` (33 bytes)
5. Revocation outpoint: txid (32 bytes) + vout (VarInt)
6. Fields: VarInt(count) + for each field (sorted lexicographically): VarInt(nameLen) + name + VarInt(valueLen) + value (base64-encoded string as UTF-8 bytes)

**`verify_certificate_signature(certificate: &Certificate) -> Result<(), CertificateError>`**

Verifies BRC-52 ECDSA signature. Process:
1. Serialize preimage → SHA-256 hash
2. Build BRC-43 invoice: `"2-certificate signature-{type_b64} {serial_b64}"`
3. Derive child public key via BRC-42 using 'anyone' (private key 1) as sender, certifier's pubkey as counterparty
4. Verify ECDSA signature against derived key

**`verify_certificate_signature_with_keyid(..., type_base64_original, serial_base64_original)`**

Same as above but accepts original base64 strings from JSON to avoid re-encoding mismatches between client and server.

**`check_revocation_status(revocation_outpoint: &str) -> Result<bool, CertificateError>`** (async)

Checks WhatsOnChain API (`/v1/bsv/main/tx/{txid}/outspend/{vout}`) to determine if the revocation UTXO is spent. Returns `Ok(true)` if revoked (spent), `Ok(false)` if active (unspent). Treats HTTP 404 as active (outpoint may not exist yet).

### Selective Disclosure (`selective_disclosure.rs`)

**`create_keyring_for_verifier(db_conn, certificate, subject_private_key, certifier_public_key, verifier_public_key, fields_to_reveal, serial_number_base64) -> Result<HashMap<String, String>, CertificateError>`**

BRC-53 selective disclosure. For each field in `fields_to_reveal`:
1. Reads master keyring from DB via `CertificateRepository`
2. Decrypts master keyring entry using BRC-2 (`decrypt_certificate_field` with subject/certifier key pair, invoice: `"2-certificate field encryption-{fieldName}"`)
3. Re-encrypts field revelation key for verifier using BRC-2 (`encrypt_certificate_field` with subject/verifier key pair, invoice: `"2-certificate field encryption-{serialNumber} {fieldName}"`)
4. Returns map of fieldName → base64-encoded verifier-specific keyring values

Validates that `fields_to_reveal` is non-empty and that all requested fields exist in the certificate.

## Dependencies

| Crate / Module | Used For |
|----------------|----------|
| `crate::crypto::brc42` | `derive_child_public_key`, `compute_shared_secret`, `compute_invoice_hmac` — BRC-42 key derivation for signature verification |
| `crate::crypto::brc43` | `InvoiceNumber`, `SecurityLevel` — invoice number formatting |
| `crate::crypto::brc2` | `encrypt_certificate_field`, `decrypt_certificate_field` — BRC-2 encryption for selective disclosure |
| `crate::crypto::signing` | `sha256` — preimage hashing |
| `crate::transaction` | `encode_varint` — VarInt encoding for preimage serialization |
| `crate::database::certificate_repo` | `CertificateRepository` — DB access for master keyring (selective disclosure only) |
| `secp256k1` | ECDSA signature verification, public key parsing |
| `reqwest` | HTTP client for WhatsOnChain revocation check |
| `base64` / `hex` | Encoding/decoding |

## Usage

Called from `src/handlers/certificate_handlers.rs`:

```rust
// Parsing a certificate from acquireCertificate JSON
use crate::certificate::parse_certificate_from_json;
let cert = parse_certificate_from_json(&json_body)?;

// Verifying signature (with original base64 strings for exact match)
use crate::certificate::verifier::verify_certificate_signature_with_keyid;
verify_certificate_signature_with_keyid(&cert, Some(type_b64), Some(serial_b64))?;

// Checking revocation status
use crate::certificate::verifier::check_revocation_status;
let is_revoked = check_revocation_status(&cert.revocation_outpoint).await?;

// Selective disclosure for proveCertificate
use crate::certificate::selective_disclosure::create_keyring_for_verifier;
let verifier_keyring = create_keyring_for_verifier(
    &db_conn, &cert, &subject_privkey, &certifier_pubkey,
    &verifier_pubkey, &fields_to_reveal, &serial_b64,
)?;
```

## Related

- `../database/certificate_repo.rs` — `CertificateRepository`: DB CRUD for certificates and certificate fields
- `../crypto/brc2.rs` — BRC-2 symmetric encryption used for field values and keyring entries
- `../crypto/brc42.rs` — BRC-42 ECDH key derivation used in signature verification
- `../crypto/brc43.rs` — BRC-43 invoice number formatting
- `../handlers/certificate_handlers.rs` — HTTP endpoint handlers that call into this module
- `../database/CLAUDE.md` — Database layer documentation (includes `certificates` and `certificate_fields` table schemas)
