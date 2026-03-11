# Crypto — Wallet Cryptographic Operations

> Self-contained cryptographic module implementing BRC protocol key derivation, signing, encryption, and platform-native credential storage. Security-critical: all private key operations happen here.

## Overview

This module provides the complete cryptographic foundation for the HodosBrowser wallet. It implements the BSV BRC protocol suite (BRC-2, BRC-42, BRC-43) for key derivation and encryption, ECDSA signing for transactions and authentication, PIN-based mnemonic encryption, and platform-native auto-unlock (Windows DPAPI / macOS Keychain). The custom AES-GCM implementation matches the TypeScript BSV SDK byte-for-byte to ensure cross-platform interoperability.

**Security invariant**: Private keys are only handled as `&[u8]` slices and `SecretKey` structs — never serialized to strings or logged. All signing and key derivation stays within this module.

## Key Files

| File | Purpose |
|------|---------|
| `mod.rs` | Module exports — publishes all submodules, no re-exports (binary application) |
| `keys.rs` | secp256k1 public key derivation (compressed 33-byte and uncompressed 65-byte) |
| `signing.rs` | ECDSA signing/verification, SHA-256, double-SHA-256, HMAC-SHA256 with constant-time comparison |
| `brc42.rs` | BRC-42 ECDH key derivation: shared secrets, child key derivation (public and private), symmetric key derivation |
| `brc43.rs` | BRC-43 invoice number formatting: `SecurityLevel` enum, `InvoiceNumber` struct, protocol ID normalization |
| `brc2.rs` | BRC-2 encryption/decryption using BRC-42 derived AES-256-GCM keys; certificate field encryption helpers |
| `aesgcm_custom.rs` | Custom AES-GCM implementation matching TypeScript SDK exactly, including 32-byte IV support via GHASH |
| `aesgcm_custom_test.rs` | Roundtrip and known-value tests for the custom AES-GCM implementation |
| `ghash.rs` | GHASH (Galois Hash) for AES-GCM: GF(2^128) multiplication, hash subkey generation |
| `pin.rs` | PIN-based mnemonic encryption: PBKDF2-HMAC-SHA256 (600K iterations) + AES-256-GCM |
| `dpapi.rs` | Platform-native auto-unlock: Windows DPAPI, macOS Keychain, Linux stub |

## Key Exports

### keys.rs

| Function | Signature | Purpose |
|----------|-----------|---------|
| `derive_public_key` | `(&[u8]) → Vec<u8>` | 32-byte private key → 33-byte compressed public key |
| `derive_public_key_uncompressed` | `(&[u8]) → Vec<u8>` | 32-byte private key → 65-byte uncompressed public key (0x04 prefix) |

### signing.rs

| Function | Signature | Purpose |
|----------|-----------|---------|
| `sign_ecdsa` | `(sighash, privkey, sighash_type) → Vec<u8>` | DER-encoded ECDSA signature + sighash type byte |
| `verify_signature` | `(sighash, sig_with_type, pubkey) → bool` | Verify DER signature against compressed public key |
| `sha256` | `(&[u8]) → Vec<u8>` | Single SHA-256 hash |
| `double_sha256` | `(&[u8]) → Vec<u8>` | SHA-256(SHA-256(data)) — used for txid computation |
| `hmac_sha256` | `(key, data) → Vec<u8>` | HMAC-SHA256 (32-byte output) |
| `verify_hmac_sha256` | `(key, data, expected) → bool` | Constant-time HMAC verification |

### brc42.rs

| Function | Signature | Purpose |
|----------|-----------|---------|
| `compute_shared_secret` | `(privkey, pubkey) → Vec<u8>` | ECDH point multiplication → 33-byte compressed shared secret |
| `compute_invoice_hmac` | `(shared_secret, invoice_number) → Vec<u8>` | HMAC-SHA256 of invoice number keyed by shared secret |
| `derive_child_public_key` | `(sender_privkey, recipient_pubkey, invoice) → Vec<u8>` | Sender derives recipient's child public key (BRC-42 Steps 1-6) |
| `derive_child_private_key` | `(recipient_privkey, sender_pubkey, invoice) → Vec<u8>` | Recipient derives corresponding child private key (BRC-42 Steps 1-4) |
| `derive_symmetric_key_for_hmac` | `(our_privkey, their_pubkey, invoice) → Vec<u8>` | Full BRC-42 symmetric key: child ECDH → x-coordinate extraction (32 bytes) |

### brc43.rs

| Type/Function | Purpose |
|---------------|---------|
| `SecurityLevel` | Enum: `NoPermissions(0)`, `ProtocolLevel(1)`, `CounterpartyLevel(2)` |
| `InvoiceNumber` | Struct with `security_level`, `protocol_id`, `key_id`; formats as `"{level}-{protocol}-{keyID}"` |
| `InvoiceNumber::new()` | Validated construction with protocol ID normalization |
| `InvoiceNumber::from_string()` | Parse `"0-hello world-1"` format (uses `splitn(3, '-')` so key IDs may contain dashes) |
| `normalize_protocol_id()` | Lowercase, trim, collapse spaces, validate charset/length (5-280 chars), reject trailing " protocol" |

### brc2.rs

| Function | Signature | Purpose |
|----------|-----------|---------|
| `derive_symmetric_key` | `(sender_privkey, recipient_pubkey, invoice) → Vec<u8>` | BRC-42 child key derivation → ECDH → x-coordinate as 32-byte AES key |
| `encrypt_brc2` | `(plaintext, symmetric_key) → Vec<u8>` | AES-256-GCM encrypt; output: `[32-byte IV][ciphertext][16-byte tag]` |
| `decrypt_brc2` | `(ciphertext_with_iv, symmetric_key) → Vec<u8>` | AES-256-GCM decrypt; expects `[32-byte IV][ciphertext][16-byte tag]` format |
| `encrypt_certificate_field` | `(privkey, pubkey, field_name, serial?, plaintext) → Vec<u8>` | BRC-52 certificate field encryption (protocol: `"certificate field encryption"`, level 2) |
| `decrypt_certificate_field` | `(privkey, pubkey, field_name, serial?, ciphertext) → Vec<u8>` | Corresponding decryption |

### aesgcm_custom.rs

| Function | Purpose |
|----------|---------|
| `aesgcm_custom` | Encrypt: plaintext + AAD + IV + key → (ciphertext, 16-byte auth tag) |
| `aesgcm_decrypt_custom` | Decrypt: ciphertext + AAD + IV + tag + key → plaintext (verifies tag) |

### pin.rs

| Function | Purpose |
|----------|---------|
| `derive_key_from_pin` | PBKDF2-HMAC-SHA256 (600K iterations) → 32-byte AES key |
| `encrypt_mnemonic` | PIN + mnemonic → (salt_hex, encrypted_hex). Format: `hex(nonce_12 \|\| ciphertext \|\| tag_16)` |
| `decrypt_mnemonic` | PIN + salt_hex + encrypted_hex → plaintext mnemonic. Returns `"Invalid PIN"` on wrong PIN |

### dpapi.rs

| Function | Platform | Purpose |
|----------|----------|---------|
| `dpapi_encrypt` | Windows | `CryptProtectData` — ties encrypted blob to current Windows user |
| `dpapi_encrypt` | macOS | `set_generic_password` — stores in Keychain (service: `"HodosBrowser"`, account: `"wallet-mnemonic"`); returns sentinel `b"KEYCHAIN"` for DB |
| `dpapi_decrypt` | Windows | `CryptUnprotectData` — decrypts if same Windows user |
| `dpapi_decrypt` | macOS | `get_generic_password` — retrieves from Keychain (ignores sentinel input) |
| `dpapi_encrypt/decrypt` | Linux | Stubs returning `Err` — wallet still works, just requires PIN |

## Architecture: BRC-2 Encryption Pipeline

The full encryption path chains three BRC protocols:

```
BRC-43: Format invoice number
  "{level}-{protocolID}-{keyID}"
         │
         ▼
BRC-42: Derive child keys
  1. ECDH shared secret (privkey * pubkey)
  2. HMAC(shared_secret, invoice_number) → scalar
  3. child_pubkey  = recipient_pubkey + scalar*G
  4. child_privkey = recipient_privkey + scalar (mod N)
  5. ECDH(child_privkey, child_pubkey) → x-coordinate = symmetric key
         │
         ▼
BRC-2: AES-256-GCM encryption
  1. Random 32-byte IV
  2. Custom AESGCM (32-byte IV → GHASH pre-counter block)
  3. Output: [IV(32)][ciphertext][tag(16)]
```

## Custom AES-GCM: Why Not Use a Standard Library?

The `aesgcm_custom.rs` + `ghash.rs` modules exist because BRC-2 uses **32-byte IVs**, while standard AES-GCM libraries only accept 12-byte nonces. The TypeScript BSV SDK handles non-standard IVs by hashing them through GHASH to produce the initial counter block. This custom implementation replicates that exact behavior to ensure byte-for-byte compatibility with the TypeScript SDK.

Standard `aes-gcm` crate is still used in `pin.rs` (which uses standard 12-byte nonces for local PIN encryption).

## Mnemonic Protection: Two Layers

| Layer | Mechanism | When Used |
|-------|-----------|-----------|
| **PIN encryption** (`pin.rs`) | PBKDF2 (600K rounds) + AES-256-GCM | Always — stored in `wallets.mnemonic` as hex |
| **Platform auto-unlock** (`dpapi.rs`) | DPAPI / Keychain | Optional — stored in `wallets.mnemonic_dpapi` |

Both can coexist. Auto-unlock bypasses the PIN prompt on startup if the same OS user is logged in. The PIN-encrypted version remains as fallback.

## Usage Patterns

### Transaction signing (handlers.rs)
```rust
use crate::crypto::signing::{sign_ecdsa, sha256};
use crate::crypto::keys::derive_public_key;

let sighash = sha256(&preimage);  // Actually computed by sighash module
let signature = sign_ecdsa(&sighash, &private_key_bytes, 0x41)?;  // 0x41 = SIGHASH_ALL|FORKID
let pubkey = derive_public_key(&private_key_bytes)?;
```

### BRC-42 key derivation for HMAC (handlers.rs create_hmac)
```rust
use crate::crypto::brc42::derive_symmetric_key_for_hmac;

let symmetric_key = derive_symmetric_key_for_hmac(
    &master_private_key,
    &counterparty_pubkey,
    &invoice_number,  // "2-protocol name-keyID"
)?;
let hmac = hmac_sha256(&symmetric_key, data);
```

### BRC-2 certificate field encryption (handlers.rs acquire_certificate)
```rust
use crate::crypto::brc2::{encrypt_certificate_field, decrypt_certificate_field};

let ciphertext = encrypt_certificate_field(
    &master_privkey, &verifier_pubkey,
    "name", Some(&serial_number), plaintext_bytes,
)?;
```

### PIN-based wallet unlock (database/connection.rs)
```rust
use crate::crypto::pin::{encrypt_mnemonic, decrypt_mnemonic};

let (salt_hex, encrypted_hex) = encrypt_mnemonic(&mnemonic, "1234")?;
let mnemonic = decrypt_mnemonic(&encrypted_hex, "1234", &salt_hex)?;
```

## Error Types

Each submodule defines its own error enum with `thiserror::Error`:

| Module | Error Type | Key Variants |
|--------|-----------|--------------|
| `keys` | `KeyDerivationError` | `InvalidPrivateKey` |
| `signing` | `SigningError` | `InvalidPrivateKey`, `InvalidMessage`, `InvalidSignature` |
| `brc42` | `Brc42Error` | `InvalidPrivateKey`, `InvalidPublicKey`, `DerivationFailed`, `Secp256k1Error` |
| `brc2` | `Brc2Error` | `InvalidPrivateKey`, `InvalidPublicKey`, `InvalidInvoiceNumber`, `KeyDerivationFailed`, `EncryptionFailed`, `DecryptionFailed`, `InvalidCiphertext`, `AesGcmError` |
| `aesgcm_custom` | `String` | Free-form error strings |
| `pin` | `String` | `"Invalid PIN"` on wrong PIN, format errors otherwise |
| `dpapi` | `String` | Platform-specific error messages |

## Dependencies

| Crate | Used By | Purpose |
|-------|---------|---------|
| `secp256k1` | keys, signing, brc42, brc2 | Elliptic curve operations (ECDSA, ECDH, point arithmetic) |
| `sha2` | signing, pin | SHA-256 hashing |
| `hmac` | signing, brc42 | HMAC-SHA256 |
| `aes` | aesgcm_custom, ghash | Raw AES-256 block encryption (for custom GCM) |
| `aes-gcm` | pin | Standard AES-256-GCM (12-byte nonce, for PIN encryption) |
| `pbkdf2` | pin | Key stretching (600K iterations) |
| `rand` | brc2, pin | Cryptographic random IV/nonce/salt generation |
| `hex` | brc2, pin | Hex encoding for storage format |
| `base64` | brc2 | Base64 encoding (imported but used in handlers) |
| `thiserror` | keys, signing, brc42, brc2 | Derive `Error` trait for error enums |
| `security-framework` | dpapi (macOS) | macOS Keychain access |
| `windows` | dpapi (Windows) | Windows DPAPI (CryptProtectData/CryptUnprotectData) |

## Testing

All modules have inline `#[cfg(test)]` tests. BRC-42 tests use **official spec test vectors**.

```bash
cd rust-wallet
cargo test crypto          # Run all crypto tests
cargo test brc42::tests    # BRC-42 test vectors only
cargo test pin::tests      # PIN encryption roundtrip
```

Platform-specific tests (`dpapi`) are gated with `#[cfg(windows)]` / `#[cfg(target_os = "macos")]`.

## Related

- `../database/CLAUDE.md` — Database layer that stores encrypted mnemonics and derived keys
- `../database/helpers.rs` — `derive_key_for_output()` calls into `brc42` for output signing
- `../handlers.rs` — HTTP handlers that invoke crypto functions for BRC-100 protocol endpoints
- `../authfetch.rs` — BRC-103 AuthFetch uses `signing.rs` for ECDSA request signing
- `../messagebox.rs` — MessageBox uses `brc2.rs` for BRC-2 message encryption
- `../../CLAUDE.md` — Root project context with full architecture overview
