# Crypto — Wallet Cryptographic Operations

> Self-contained cryptographic module implementing BRC protocol key derivation, signing, encryption, and platform-native credential storage. Security-critical: all private key operations happen here.
>
> **Last updated:** 2026-08-03

## Overview

This module provides the complete cryptographic foundation for the HodosBrowser wallet. It implements the BSV BRC protocol suite (BRC-2, BRC-42, BRC-43) for key derivation and encryption, BRC-72 key-linkage revelation primitives, BIE1 (ECIES Electrum) legacy-compat encryption, ECDSA signing for transactions and authentication, PIN-based mnemonic encryption, and platform-native auto-unlock (Windows DPAPI / macOS Keychain). The custom AES-GCM implementation matches the TypeScript BSV SDK byte-for-byte to ensure cross-platform interoperability.

`mod.rs` declares **11 public modules** plus one test-only module (`aesgcm_custom_test`, gated `#[cfg(test)]` and declared private). There are 13 `.rs` files in the directory (11 public modules + `mod.rs` + the test module).

**Security invariant**: Private keys are only handled as `&[u8]` slices and `SecretKey` structs — never serialized to strings or logged. All signing and key derivation stays within this module.

## Key Files

| File | Purpose |
|------|---------|
| `mod.rs` | Module declarations — publishes all 11 submodules, no re-exports (binary application). `aesgcm_custom_test` is declared `#[cfg(test)] mod` (private) |
| `keys.rs` | secp256k1 public key derivation (compressed 33-byte and uncompressed 65-byte) |
| `signing.rs` | ECDSA signing/verification, SHA-256, double-SHA-256, HMAC-SHA256 with constant-time comparison |
| `brc42.rs` | BRC-42 ECDH key derivation: shared secrets, child key derivation (public and private), symmetric key derivation |
| `brc43.rs` | BRC-43 invoice number formatting: `SecurityLevel` enum, `InvoiceNumber` struct, protocol ID normalization |
| `brc2.rs` | BRC-2 encryption/decryption using BRC-42 derived AES-256-GCM keys; certificate field encryption helpers |
| `bie1.rs` | **BIE1 / ECIES-Electrum legacy compat** (Phase 2 Step 3c.1). Wire-compatible with `@bsv/sdk`'s `ECIES.electrumEncrypt/Decrypt`. AES-128-CBC + PKCS#7 under an SHA-512-split ephemeral ECDH secret, HMAC-SHA256 MAC-then-decrypt. Exists so the `window.yours.encrypt/decrypt` shim can read Yours/RelayX-era ciphertexts — **not** a Hodos primary cipher |
| `key_linkage.rs` | **BRC-72 key-linkage revelation primitives.** Mirrors `@bsv/sdk` `KeyDeriver.revealCounterpartySecret` / `revealSpecificSecret`. Composes `brc42` primitives only — deliberately does not extend `brc42.rs`. The Schnorr DLEQ proof is deferred; handlers emit the SDK's `[0]` no-proof marker |
| `aesgcm_custom.rs` | Custom AES-GCM implementation matching TypeScript SDK exactly, including 32-byte IV support via GHASH |
| `aesgcm_custom_test.rs` | Roundtrip, known-value, and cross-implementation vector tests for the custom AES-GCM implementation (test-only module) |
| `ghash.rs` | GHASH (Galois Hash) for AES-GCM: GF(2^128) multiplication, hash subkey generation |
| `pin.rs` | PIN-based mnemonic encryption: PBKDF2-HMAC-SHA256 (600K iterations) + AES-256-GCM |
| `dpapi.rs` | Platform-native auto-unlock: Windows DPAPI (full impl), macOS Keychain (full impl), Linux/other stub |

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
| `SecurityLevel` | Enum: `NoPermissions(0)`, `ProtocolLevel(1)`, `CounterpartyLevel(2)`. Implements `Display`, `as_u8()`, `from_u8()` |
| `InvoiceNumber` | Struct with `security_level`, `protocol_id`, `key_id`; formats as `"{level}-{protocol}-{keyID}"` via `Display` |
| `InvoiceNumber::new()` | Validated construction with protocol ID normalization and key ID length check (1-800 bytes) |
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

### bie1.rs

| Item | Signature | Purpose |
|------|-----------|---------|
| `encrypt_bie1` | `(plaintext, recipient_pubkey, sender_privkey: Option<&[u8]>) → Vec<u8>` | Build a BIE1 envelope. `None` sender key → fresh ephemeral key from OS CSPRNG (standard Electrum behavior); `Some(bytes)` locks the ephemeral scalar for deterministic tests |
| `decrypt_bie1` | `(envelope, recipient_privkey) → Vec<u8>` | Parse + verify + decrypt. Order is length check → magic → ephemeral pubkey curve check → ECDH/SHA-512 subkeys → **HMAC verify** → AES decrypt + PKCS#7 unpad |
| `Bie1Error` | enum | See Error Types below |
| `derive_subkeys` | `(shared_compressed) → ([u8;16], [u8;16], [u8;32])` | **Private.** SHA-512 split: `iv = hash[0..16]`, `aeKey = hash[16..32]`, `macKey = hash[32..64]` — the exact `@bsv/sdk` convention |

Envelope layout constants (all private): `MAGIC = b"BIE1"`, `MAGIC_LEN = 4`, `PUBKEY_LEN = 33`, `MAC_LEN = 32`, `MIN_ENVELOPE_LEN = 4 + 33 + 16 + 32 = 85`.

```text
[ "BIE1" 4B ][ ephemeral_pub 33B compressed ][ AES-128-CBC ciphertext NB ][ HMAC-SHA256 32B ]
```

> **MAC-then-decrypt is load-bearing.** The HMAC is verified *before* AES decryption via `Hmac::verify_slice` (constant-time). Corrupting bytes inside the ciphertext therefore surfaces as `MacMismatch`, never `InvalidPadding` — no padding oracle. `decrypt_rejects_corrupted_ciphertext_via_mac_check` in `bie1.rs` locks this behavior; do not reorder the steps.

HTTP surface: `POST /wallet/encrypt-bie1` → `handlers.rs :: encrypt_bie1_handler`, `POST /wallet/decrypt-bie1` → `handlers.rs :: decrypt_bie1_handler` (routes registered in `main.rs`; both are gated by `permission_service/request_gate.rs`).

### key_linkage.rs

| Function | Signature | Purpose |
|----------|-----------|---------|
| `compute_counterparty_linkage` | `(master_privkey, counterparty_pubkey) → Vec<u8>` | `revealCounterpartySecret` value: 33-byte compressed ECDH shared secret. Thin wrapper over `brc42::compute_shared_secret`. The SDK forbids `counterparty === 'self'` here — callers must resolve/validate first |
| `compute_specific_linkage` | `(master_privkey, counterparty_pubkey, invoice_number) → Vec<u8>` | `revealSpecificSecret` value: 32-byte `HMAC-SHA256(shared_secret, invoice_number)`. Composes `compute_shared_secret` + `compute_invoice_hmac` |

Both return `Result<Vec<u8>, Brc42Error>` — this module defines **no error type of its own**.

Consumers: `handlers.rs :: reveal_counterparty_key_linkage` and `handlers.rs :: reveal_specific_key_linkage`, routed as `POST /revealCounterpartyKeyLinkage` / `POST /revealSpecificKeyLinkage` in `main.rs`. Both are privacy-perimeter gated by the Rust permission engine (`PromptType::KeyLinkageReveal`); per-domain session opt-in lives in `permission_service/state.rs :: approve_key_linkage_session` / `is_key_linkage_session_approved`.

### aesgcm_custom.rs

| Function | Purpose |
|----------|---------|
| `aesgcm_custom` | Encrypt: plaintext + AAD + IV + key → (ciphertext, 16-byte auth tag) |
| `aesgcm_decrypt_custom` | Decrypt: ciphertext + AAD + IV + tag + key → plaintext (verifies tag) |

### ghash.rs

| Function | Purpose |
|----------|---------|
| `ghash` | GHASH over input using hash subkey → 16-byte result. Processes input in 16-byte chunks with GF(2^128) multiplication |
| `generate_hash_subkey` | AES-256 encrypt zero block → 16-byte hash subkey for GHASH |

### pin.rs

| Function | Purpose |
|----------|---------|
| `derive_key_from_pin` | PBKDF2-HMAC-SHA256 (600K iterations) → 32-byte AES key |
| `encrypt_mnemonic` | PIN + mnemonic → (salt_hex, encrypted_hex). Format: `hex(nonce_12 \|\| ciphertext \|\| tag_16)` |
| `decrypt_mnemonic` | PIN + salt_hex + encrypted_hex → plaintext mnemonic. Returns `"Invalid PIN"` on wrong PIN |

### dpapi.rs

Two public functions, `dpapi_encrypt` and `dpapi_decrypt`, each with three mutually exclusive `cfg` bodies. **Windows and macOS are both full implementations; only Linux/other is a stub.**

| Function | Platform (`cfg`) | Purpose |
|----------|------------------|---------|
| `dpapi_encrypt` | `windows` — **full impl** | `CryptProtectData` with `CRYPTPROTECT_UI_FORBIDDEN` — ties encrypted blob to current Windows user; copies then `LocalFree`s the system buffer |
| `dpapi_decrypt` | `windows` — **full impl** | `CryptUnprotectData` — decrypts only if the same Windows user is logged in |
| `dpapi_encrypt` | `target_os = "macos"` — **full impl** | `delete_generic_password` (clears any existing entry) then `set_generic_password`; returns sentinel `b"KEYCHAIN"` for the DB column |
| `dpapi_decrypt` | `target_os = "macos"` — **full impl** | `get_generic_password` — retrieves from Keychain; the `_encrypted` sentinel argument is ignored |
| `dpapi_encrypt` / `dpapi_decrypt` | `all(not(windows), not(macos))` — **stub** | Both return `Err("Platform auto-unlock not available. Use PIN to unlock wallet.")`. Wallet still works, just always requires the PIN |

macOS Keychain identifiers (all private to `dpapi.rs`):

| Item | Value |
|------|-------|
| `KEYCHAIN_SENTINEL` | `b"KEYCHAIN"` — must be non-empty so `mnemonic_dpapi.is_some()` reads as "auto-unlock available" |
| `KEYCHAIN_ACCOUNT` | `"wallet-mnemonic"` |
| `keychain_service()` | **Function, not a constant.** Returns `"HodosBrowserDev"` when `HODOS_DEV=1`, else `"HodosBrowser"` — the dev/prod deconfliction guard that stops a dev wallet overwriting the production mnemonic in the Keychain |

Storage on both real platforms is the `wallets.mnemonic_dpapi` BLOB column (added by migration V4 for pre-V4 DBs; present in the V1 consolidated schema). On Windows it holds the raw DPAPI blob; on macOS it holds only the sentinel.

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

`key_linkage.rs` taps the same chain at the top two stages: the counterparty linkage value **is** the stage-1 shared secret, and the specific linkage value **is** the stage-2 invoice HMAC. That is why it composes `brc42` primitives rather than duplicating them.

BIE1 (`bie1.rs`) is a **separate, non-BRC pipeline** — no invoice number, no BRC-42 child keys, AES-128-CBC instead of AES-256-GCM. Do not mix the two paths.

## ⚠️ CRITICAL: Rust ↔ TypeScript SDK Interop Rules

This module reimplements the TypeScript BSV SDK's cryptographic operations in Rust. The two languages have fundamentally different standard library behaviors that cause subtle, hard-to-diagnose interop failures. **Always verify Rust crypto behavior against the TypeScript SDK source (`reference/ts-brc100/node_modules/@bsv/sdk/`), not assumptions about what "should" happen.**

### Rule 1: `counterparty='self'` = BRC-42 ECDH with own public key

The TypeScript SDK's `KeyDeriver.normalizeCounterparty('self')` returns `rootKey.toPublicKey()`, then performs full BRC-42 ECDH. This is non-obvious — self-ECDH seems pointless, but it produces a different symmetric key per invoice number, which is the whole point of BRC-42 key isolation.

```rust
// ✅ CORRECT — matches TypeScript SDK
let own_pubkey = derive_public_key(&master_privkey)?;
let sym_key = derive_symmetric_key_for_hmac(&master_privkey, &own_pubkey, &invoice)?;

// ❌ WRONG — produces completely different key, breaks all auth handshakes
let sym_key = master_privkey.clone();
```

This applies to: `createHmac`, `verifyHmac`, `createNonce`, and any future BRC-42 symmetric key usage where counterparty is `'self'` or `None`.

### Rule 2: Leading-zero stripping on symmetric keys

TypeScript SDK's `SymmetricKey` extends `BigNumber`. Calling `.toArray()` (without length param) strips leading zero bytes. The SDK's `createHmac` does `sha256hmac(key.toArray(), data)`. Rust must strip before HMAC:

```rust
let mut k = hmac_key.as_slice();
while k.len() > 1 && k[0] == 0 { k = &k[1..]; }
```

### Rule 3: `Utils.toUTF8()` surrogate pair handling

TypeScript's `String.fromCharCode()` accepts surrogate values (0xD800-0xDFFF) and creates valid supplementary characters when paired. Rust's `char::from_u32()` rejects surrogates. Use `char::from_u32(code_point)` directly with the full code point, not computed surrogates. See `js_to_utf8()` in `certificate_handlers.rs`.

### Rule 4: Cross-implementation test vectors

`aesgcm_custom_test.rs` contains test vectors generated by the TypeScript SDK (`reference/ts-brc100/test-aesgcm-vectors.mjs`). These verify byte-for-byte compatibility for AESGCM encryption and BRC-42 key derivation. Run `cargo test aesgcm` after any crypto changes.

**Known gap:** `bie1.rs` has *no* cross-implementation vectors yet. Its 16 tests lock the wire format against our own implementation only (`deterministic_with_explicit_sender_priv` + `subkey_layout_matches_canonical_split`). The module doc-comment flags this: `@bsv/sdk` vectors are to be locked in at Phase 2 Step 3c.2 integration smoke once a Node helper is wired. Until then, BIE1 byte-compatibility with the SDK is asserted by code review, not by test.

## Custom AES-GCM: Why Not Use a Standard Library?

The `aesgcm_custom.rs` + `ghash.rs` modules exist because BRC-2 uses **32-byte IVs**, while standard AES-GCM libraries only accept 12-byte nonces. The TypeScript BSV SDK handles non-standard IVs by hashing them through GHASH to produce the initial counter block. This custom implementation replicates that exact behavior to ensure byte-for-byte compatibility with the TypeScript SDK.

Standard `aes-gcm` crate is still used in `pin.rs` (which uses standard 12-byte nonces for local PIN encryption). `bie1.rs` uses neither — it needs AES-128-**CBC** with PKCS#7, so it pulls the `cbc` crate.

## Mnemonic Protection: Two Layers

| Layer | Mechanism | When Used |
|-------|-----------|-----------|
| **PIN encryption** (`pin.rs`) | PBKDF2-HMAC-SHA256, `PBKDF2_ITERATIONS = 600_000`, `SALT_LEN = 16`, `NONCE_LEN = 12`, then AES-256-GCM | Always — stored in `wallets.mnemonic` as hex |
| **Platform auto-unlock** (`dpapi.rs`) | DPAPI (Windows) / Keychain (macOS); unavailable on Linux | Optional — stored in `wallets.mnemonic_dpapi` |

Both can coexist. Auto-unlock bypasses the PIN prompt on startup if the same OS user is logged in. The PIN-encrypted version remains as fallback.

Consumers of these two modules outside `crypto/`:

| Caller | Uses |
|--------|------|
| `database/wallet_repo.rs` | `pin::encrypt_mnemonic` + `dpapi::dpapi_encrypt` on wallet create and on mnemonic re-encrypt |
| `database/connection.rs` | `pin::decrypt_mnemonic` (PIN unlock) and `dpapi::dpapi_decrypt` / `dpapi_encrypt` (auto-unlock path) |
| `backup.rs` | `pin::derive_key_from_pin` — the encrypted wallet backup file reuses the PIN KDF (it does **not** reuse `encrypt_mnemonic`) |

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

Five submodules define their own error enum with `thiserror::Error`; the rest use `String`. Two modules (`ghash`, `key_linkage`) define no error type at all — `ghash` is infallible and `key_linkage` returns `Brc42Error` from the primitives it composes.

| Module | Error Type | Variants (complete) |
|--------|-----------|---------------------|
| `keys` | `KeyDerivationError` | `InvalidPrivateKey(String)` — single variant |
| `signing` | `SigningError` | `InvalidPrivateKey`, `InvalidMessage`, `InvalidSignature` (all `(String)`) |
| `brc42` | `Brc42Error` | `InvalidPrivateKey`, `InvalidPublicKey`, `DerivationFailed`, `Secp256k1Error` (all `(String)`) |
| `brc2` | `Brc2Error` | `InvalidPrivateKey`, `InvalidPublicKey`, `InvalidInvoiceNumber`, `KeyDerivationFailed`, `EncryptionFailed`, `DecryptionFailed`, `InvalidCiphertext`, `AesGcmError` — 8 variants |
| `bie1` | `Bie1Error` | `InvalidRecipientPublicKey`, `InvalidSenderPrivateKey`, `InvalidRecipientPrivateKey`, `EcdhFailed`, `EnvelopeTooShort { len, min }`, `InvalidMagic`, `InvalidEphemeralPublicKey`, `MacMismatch`, `InvalidPadding`, `AesError`, `HmacInitError` — 11 variants |
| `key_linkage` | *(none — reuses `Brc42Error`)* | — |
| `ghash` | *(none — infallible)* | — |
| `aesgcm_custom` | `String` | Free-form error strings (e.g. `"Authentication tag verification failed"`) |
| `pin` | `String` | `"Invalid PIN"` on wrong PIN, format errors otherwise |
| `dpapi` | `String` | Platform-specific error messages |

> `bie1`'s error variants are deliberately fine-grained but the **handlers collapse them** — `handlers.rs :: decrypt_bie1_handler` maps every parse/MAC/padding variant to one opaque client-facing error so the distinction can't be used as an oracle.

## Dependencies

| Crate | Version | Used By | Purpose |
|-------|---------|---------|---------|
| `secp256k1` | 0.28 (`rand-std`) | keys, signing, brc42, brc2, bie1 | Elliptic curve operations (ECDSA, ECDH, point arithmetic) |
| `sha2` | 0.10 | signing, brc42, pin, bie1 | SHA-256 hashing; **SHA-512** for the BIE1 subkey split |
| `hmac` | 0.12 | signing, brc42, bie1 | HMAC-SHA256 |
| `subtle` | 2 | signing, aesgcm_custom | **Constant-time comparison** (`ConstantTimeEq`) for HMAC verify and GCM auth-tag verify. Was missing from this table — it is production code, not a test dep |
| `aes` | 0.8 | aesgcm_custom, ghash (AES-256), bie1 (AES-128) | Raw AES block encryption |
| `cbc` | 0.1 (`alloc`) | bie1 | AES-128-CBC mode + PKCS#7 padding for the BIE1 envelope |
| `aes-gcm` | 0.10 | pin | Standard AES-256-GCM (12-byte nonce, for PIN encryption) |
| `pbkdf2` | 0.12 | pin | Key stretching (600K iterations) |
| `rand` | 0.8 | brc2, pin, bie1 | Cryptographic random IV / nonce / salt / ephemeral-key generation |
| `hex` | 0.4 | brc2, pin | Hex encoding for storage format |
| `base64` | 0.22 | brc2 | Imported in `brc2.rs` but **not referenced there** — a dead import; actual base64 work happens in `handlers.rs` |
| `log` | 0.4 | brc2, aesgcm_custom, dpapi (macOS) | Debug/info logging for key derivation and encryption operations |
| `thiserror` | 1.0 | keys, signing, brc42, brc2, bie1 | Derive `Error` trait for error enums |
| `security-framework` | 2 | dpapi (macOS) | macOS Keychain access — declared under `[target.'cfg(target_os = "macos")'.dependencies]` |
| `windows` | 0.62 | dpapi (Windows) | Windows DPAPI (`CryptProtectData`/`CryptUnprotectData`) — declared under `[target.'cfg(windows)'.dependencies]`, features `Win32_Security_Cryptography` + `Win32_Foundation` |

## Testing

Every module has inline `#[cfg(test)]` tests. BRC-42 tests use **official spec test vectors** (`test_private_key_derivation_vector_1/2`, `test_public_key_derivation_vector_1/2`).

| Module | `#[test]` count | Notes |
|--------|-----------------|-------|
| `bie1.rs` | 16 | Round-trips (empty / 1-block / multi-block), envelope layout, determinism, and 7 negative/corruption cases |
| `brc43.rs` | 14 | Invoice number parse/format + `normalize_protocol_id` charset & length rules |
| `signing.rs` | 13 | ECDSA sign/verify, SHA-256, double-SHA-256, HMAC |
| `aesgcm_custom_test.rs` | 7 | Includes 2 cross-impl vectors (32-byte and 12-byte IV) generated by `@bsv/sdk` |
| `brc42.rs` | 6 | 4 spec vectors + self-derivation consistency + shared-secret symmetry |
| `key_linkage.rs` | 6 | Length/prefix assertions, ECDH symmetry, invoice- and counterparty-sensitivity |
| `keys.rs` | 5 | Compressed / uncompressed derivation |
| `brc2.rs` | 3 | Encrypt/decrypt roundtrip + certificate field helpers |
| `dpapi.rs` | 3 | **Mutually exclusive** — exactly one compiles per platform |
| `aesgcm_custom.rs` | 2 | Roundtrip + tag verification |
| `pin.rs` | 2 | PIN encryption roundtrip + wrong-PIN rejection |
| `ghash.rs` | 1 | GF(2^128) multiply / subkey generation |

That is **78 declared `#[test]` functions**; because `dpapi`'s three are mutually exclusive, **76 compile on any single platform**.

```bash
cd rust-wallet
cargo test crypto             # Run all crypto tests
cargo test brc42::tests       # BRC-42 spec vectors only
cargo test aesgcm             # Custom AES-GCM incl. cross-impl vectors
cargo test bie1               # BIE1 roundtrip + corruption cases
cargo test pin::tests         # PIN encryption roundtrip
```

Platform-specific tests (`dpapi`) are gated with `#[cfg(windows)]` / `#[cfg(target_os = "macos")]` / `#[cfg(all(not(windows), not(target_os = "macos")))]`.

## Related

- `../database/CLAUDE.md` — Database layer that stores encrypted mnemonics and derived keys
- `../database/helpers.rs` — `derive_key_for_output()` calls into `brc42` for output signing
- `../database/wallet_repo.rs`, `../database/connection.rs` — the only callers of `pin.rs` + `dpapi.rs`
- `../backup.rs` — reuses `pin::derive_key_from_pin` for encrypted wallet backups
- `../handlers.rs` — HTTP handlers that invoke crypto functions for BRC-100 protocol endpoints, plus `encrypt_bie1_handler` / `decrypt_bie1_handler` and `reveal_counterparty_key_linkage` / `reveal_specific_key_linkage`
- `../permission_service/` — Rust wrapper around the permission decision engine; gates the key-linkage and BIE1 endpoints before they reach this module
- `../../crates/hodos_permission_engine/` — the permission **decision** engine (`decide()` in `src/lib.rs`, cascade in `src/matrix_c.rs`). `PromptType::KeyLinkageReveal` is the privacy-perimeter gate in front of `key_linkage.rs`
- `../authfetch.rs` — BRC-103 AuthFetch uses `signing.rs` for ECDSA request signing
- `../messagebox.rs` — MessageBox uses `brc2.rs` for BRC-2 message encryption
- `../../CLAUDE.md` — Root project context with full architecture overview
