# Phase 2 Step 3b + 3c — Implementation Plan

> **Status:** Plan locked 2026-05-29. Phase 2 Steps 1-4 landed and smoke-verified
> (commits `d1e0a7f`, `6cddfde`, `884cf6c`, `63d3098`). This document covers the
> remaining legacy translations and the optional ECIES Electrum (BIE1) compat handler.

## Goal

Finish the legacy `window.yours` translations so Yours-era BSV dApps (Treechat,
RelayX-era apps) work in Hodos without modification, while keeping all BSV
crypto in Rust and the shim small.

## Why this matters

Phase 2's smoke test confirmed:
- The shim injects on every external https main frame
- `window.CWI`, `window.yours`, `window.panda` exist with correct method counts
- `bsv:announceProvider` fires with Hodos branding + icon
- Canonical CWI is non-writable/non-configurable

What it DOESN'T do yet: handle the legacy methods that need real BSV address
math. Those methods currently return `null` (`getAddresses`) or throw typed
`NOT_IMPL` errors (`sendBsv`, `getSignatures`, `encrypt`, `decrypt`). Step 3b
fixes the address-dependent and translation-only ones; Step 3c handles the
ECIES Electrum (BIE1) encryption compat gap.

---

## Reuse table — what Hodos already has

| Need | Function | Location | Status |
|---|---|---|---|
| Hex pubkey → BSV mainnet P2PKH address (Base58Check) | `pubkey_to_address(pubkey: &[u8]) -> Result<String, String>` | `rust-wallet/src/handlers.rs:8387` | **Reuse.** Already used 6+ times (certificate handlers, recovery, generate_address, backup ops). SHA256 + RIPEMD160 + Base58Check. |
| BSV/USD exchange rate | `GET /wallet/bsv-price` (uses `PriceCache`) | `rust-wallet/src/handlers.rs` + `price_cache.rs` | **Reuse.** Existing endpoint with CryptoCompare primary + CoinGecko fallback + 5-min TTL. The shim's `getBalance()` already calls this. |
| Identity public key derivation | `get_public_key` with `identityKey: true` | `rust-wallet/src/handlers.rs` | **Reuse.** Shim already calls it via `CWI.getPublicKey({identityKey: true})`. |
| BRC-42 child key derivation (bsv-receive, ord-receive protocols) | `derive_child_private_key` / `derive_child_public_key` | `rust-wallet/src/crypto/brc42.rs` | **Reuse.** Shim's `getPubKeys()` already uses these. |
| BRC-2 encrypt/decrypt (AES-256-GCM) | Canonical `CWI.encrypt` / `CWI.decrypt` | `rust-wallet/src/crypto/brc2.rs` | **Reuse for canonical path.** Step 3c adds BIE1 alongside, does NOT replace. |
| Action building (createAction / signAction) | `create_action`, `sign_action` | `rust-wallet/src/handlers.rs` | **Reuse via canonical CWI.** Shim translates legacy `sendBsv` / `getSignatures` to these. |
| Domain permission engine | `check_domain_approved` + `PermissionEngine` | `rust-wallet/src/handlers.rs` + `cef-native/src/core/PermissionEngine.cpp` | **Reuse.** New endpoints route through the same gates. |

**Bottom line:** ~95% of the math we need is already written. Step 3b adds two
small new endpoints; Step 3c adds one new crypto module.

---

## Out of scope — explicitly Phase 3 or later

- 1Sat envelope construction (inscription format)
- Ordinal UTXO classification (basket-based or on-chain)
- Ordinal Lock script handling
- `yours.inscribe` / `yours.transferOrdinal` / `yours.purchaseOrdinal` implementations
- Address derivation BEYOND the three Yours uses (bsv/ord/identity)
- Multi-recipient ECIES Electrum (Step 3c handles single-recipient only;
  multi-recipient was real in Yours but rare — defer to demand)

---

# Step 3b — Address derivation, sendBsv, getSignatures, getExchangeRate

## Scope (six sub-tasks)

1. Add `POST /wallet/derive-address` (Rust)
2. Add `POST /wallet/address-to-script` (Rust)
3. Wire `yours.getAddresses()` (JS, uses #1)
4. Wire `yours.sendBsv([{address, amount}])` (JS, uses #2 + canonical createAction)
5. Wire `yours.getSignatures()` (JS only, no Rust work)
6. Wire `yours.getExchangeRate()` (JS only, wraps existing /wallet/bsv-price)
7. Update `yours.getSocialProfile()` typed error from REMOVED → DEFERRED with migration hint

## New Rust endpoints

### 1. `POST /wallet/derive-address`

**Purpose:** Convert an arbitrary hex public key to a BSV mainnet P2PKH address.

**Implementation:** Thin wrapper around existing `pubkey_to_address`.

```rust
// rust-wallet/src/handlers.rs — new handler (~20 LOC)

#[derive(Deserialize)]
pub struct DeriveAddressRequest {
    pub publicKey: String,  // hex
}

#[derive(Serialize)]
pub struct DeriveAddressResponse {
    pub address: String,
}

pub async fn derive_address(
    body: web::Json<DeriveAddressRequest>,
) -> HttpResponse {
    let pubkey_bytes = match hex::decode(&body.publicKey) {
        Ok(b) => b,
        Err(e) => return HttpResponse::BadRequest().json(json!({
            "error": format!("Invalid hex publicKey: {}", e)
        })),
    };
    match pubkey_to_address(&pubkey_bytes) {
        Ok(address) => HttpResponse::Ok().json(DeriveAddressResponse { address }),
        Err(e) => HttpResponse::BadRequest().json(json!({
            "error": format!("Address derivation failed: {}", e)
        })),
    }
}
```

**Route registration in `main.rs`:**
```rust
.route("/wallet/derive-address", web::post().to(handlers::derive_address))
```

**Permission gate:** None needed at the wallet level. This is pure math (hex
→ Base58Check) with no key material involved. The shim-side caller routes
through `check_domain_approved` if needed, but address derivation itself is
public.

**Test:** Unit test with known pubkey/address pairs. The recovery module's
tests already cover the underlying `pubkey_to_address` math.

---

### 2. `POST /wallet/address-to-script`

**Purpose:** Convert a BSV mainnet P2PKH address to its hex locking script,
so the shim can pass `lockingScript` to `CWI.createAction({outputs: [...]})`.

**Implementation:** ~25 LOC new Rust. Reuses `bs58` crate (already a dependency
per `pubkey_to_address`'s usage).

```rust
// rust-wallet/src/handlers.rs — new handler + helper (~25 LOC)

#[derive(Deserialize)]
pub struct AddressToScriptRequest {
    pub address: String,
}

#[derive(Serialize)]
pub struct AddressToScriptResponse {
    pub lockingScript: String,  // hex
}

fn address_to_p2pkh_script(address: &str) -> Result<Vec<u8>, String> {
    // Base58Check decode
    let decoded = bs58::decode(address)
        .into_vec()
        .map_err(|e| format!("Base58 decode failed: {}", e))?;
    if decoded.len() != 25 {
        return Err(format!("Invalid address length: {}", decoded.len()));
    }
    // Verify checksum (last 4 bytes = double-SHA256 of first 21 bytes, truncated)
    use sha2::{Sha256, Digest};
    let payload = &decoded[..21];
    let checksum = &decoded[21..];
    let expected = Sha256::digest(&Sha256::digest(payload));
    if &expected[..4] != checksum {
        return Err("Address checksum mismatch".into());
    }
    // Check version byte (0x00 = mainnet P2PKH)
    if decoded[0] != 0x00 {
        return Err(format!("Unsupported version byte: 0x{:02x}", decoded[0]));
    }
    let hash160 = &decoded[1..21];
    // Build P2PKH locking script: OP_DUP OP_HASH160 <push 20> <hash160> OP_EQUALVERIFY OP_CHECKSIG
    let mut script = Vec::with_capacity(25);
    script.push(0x76);  // OP_DUP
    script.push(0xa9);  // OP_HASH160
    script.push(0x14);  // push 20 bytes
    script.extend_from_slice(hash160);
    script.push(0x88);  // OP_EQUALVERIFY
    script.push(0xac);  // OP_CHECKSIG
    Ok(script)
}

pub async fn address_to_script(
    body: web::Json<AddressToScriptRequest>,
) -> HttpResponse {
    match address_to_p2pkh_script(&body.address) {
        Ok(script) => HttpResponse::Ok().json(AddressToScriptResponse {
            lockingScript: hex::encode(script),
        }),
        Err(e) => HttpResponse::BadRequest().json(json!({ "error": e })),
    }
}
```

**Route registration:**
```rust
.route("/wallet/address-to-script", web::post().to(handlers::address_to_script))
```

**Permission gate:** None. Pure math, no key material.

**Tests:** Unit tests with known address/script pairs. Add at least one
checksum-failure case and one wrong-version-byte case to confirm error paths.

**Phase 3 forward-think:** Wrap `address_to_p2pkh_script` behind a public
`pub fn address_to_locking_script(address: &str, script_type: ScriptType) -> Result<Vec<u8>, String>`
where `ScriptType::P2PKH` is the only variant today. Phase 3 adds
`ScriptType::OrdinalLock`, `ScriptType::OneSatEnvelope`, etc. Same endpoint,
new variant — no refactor of `sendBsv` needed.

---

## Shim wiring — JS changes in `CWIShimScript.h`

### 3. `yours.getAddresses()`

**Before (current):**
```js
return Promise.resolve({
    bsvAddress: null,
    ordAddress: null,
    identityAddress: null
});
```

**After:**
```js
return withMutex('getAddresses', function() {
    var originKey = 'yours-' + window.location.host;
    return Promise.all([
        // bsv-receive pubkey
        canonical.getPublicKey({
            protocolID: YOURS_LEGACY_V1.RECEIVE_PROTOCOL,
            keyID: originKey,
            counterparty: YOURS_LEGACY_V1.COUNTERPARTY_SELF
        }).then(function(r) { return r && r.publicKey; }),
        // ord-receive pubkey
        canonical.getPublicKey({
            protocolID: YOURS_LEGACY_V1.ORD_RECEIVE_PROTOCOL,
            keyID: originKey,
            counterparty: YOURS_LEGACY_V1.COUNTERPARTY_SELF
        }).then(function(r) { return r && r.publicKey; }),
        // identity pubkey
        canonical.getPublicKey({ identityKey: true })
            .then(function(r) { return r && (r.publicKey || r.identityKey); })
    ]).then(function(pubkeys) {
        // Convert each pubkey to address via the new Rust endpoint
        return Promise.all(pubkeys.map(function(pk) {
            if (!pk) return null;
            return fetch(ENDPOINT_BASE + '/wallet/derive-address', {
                method: 'POST',
                mode: 'cors',
                credentials: 'omit',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ publicKey: pk })
            }).then(function(r) {
                if (!r.ok) return null;
                return r.json().then(function(j) { return j.address || null; });
            }).catch(function() { return null; });
        }));
    }).then(function(addrs) {
        return {
            bsvAddress: addrs[0],
            ordAddress: addrs[1],
            identityAddress: addrs[2]
        };
    });
});
```

**Behavior:** Returns three real BSV addresses. If a derivation fails, that
slot is `null` (gracefully degrades — matches Yours's tolerance for missing
fields).

---

### 4. `yours.sendBsv([{address, amount}])`

**Before (current):**
```js
return Promise.reject(typedError(
    LEGACY_ERR.NOT_IMPL, 'sendBsv',
    'plain-address translation lands in Step 3b. ...'
));
```

**After:**
```js
defineLegacyProp(legacy, 'sendBsv', makeLegacyMethod(function(payments) {
    warnDeprecated('sendBsv');
    if (!Array.isArray(payments) || payments.length === 0) {
        return Promise.reject(typedError(
            LEGACY_ERR.INVALID_ENCODING, 'sendBsv',
            'sendBsv expects an array of {address, amount} objects'
        ));
    }
    return withMutex('sendBsv', function() {
        // Resolve each address to a locking script
        return Promise.all(payments.map(function(p) {
            if (!p || typeof p.address !== 'string' || typeof p.amount !== 'number') {
                return Promise.reject(typedError(
                    LEGACY_ERR.INVALID_ENCODING, 'sendBsv',
                    'each payment must be {address: string, amount: number}'
                ));
            }
            return fetch(ENDPOINT_BASE + '/wallet/address-to-script', {
                method: 'POST',
                mode: 'cors',
                credentials: 'omit',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ address: p.address })
            }).then(function(r) {
                return r.json().then(function(j) {
                    if (!r.ok) {
                        throw typedError(LEGACY_ERR.INVALID_ENCODING, 'sendBsv',
                            'address-to-script failed for ' + p.address + ': ' + (j && j.error));
                    }
                    return { satoshis: p.amount, lockingScript: j.lockingScript };
                });
            });
        })).then(function(outputs) {
            return canonical.createAction({
                description: 'window.yours.sendBsv',
                outputs: outputs
            });
        }).then(function(actionResult) {
            // Canonical returns full action; legacy expects just {txid}
            return { txid: actionResult && actionResult.txid };
        });
    });
}));
```

**Behavior:** Yours dApps pass `[{address: "1ABC...", amount: 1000}]`. Shim
resolves addresses to scripts (one Rust round-trip per output), builds a
canonical `createAction` with the resulting outputs, returns the txid in the
legacy shape `{txid}`.

**Permission flow:** Each `createAction` call hits the canonical
PermissionEngine — same auto-approve / prompt path as any other BRC-100
payment. The shim doesn't bypass any gate.

---

### 5. `yours.getSignatures({rawtx, inputs})`

**No Rust work needed.** Pure JS translation to canonical `createAction` +
`signAction`.

**Before (current):**
```js
return Promise.reject(typedError(
    LEGACY_ERR.NOT_IMPL, 'getSignatures',
    'translation not yet implemented. Use window.CWI.createAction(...) ...'
));
```

**After:** Investigate exact Yours API shape first — `getSignatures` is the
fuzziest method in the legacy spec. Yours took `{rawtx, sighashType, inputs}`
and returned `{signatures: [{sig, pubkey}]}` for each requested input.
Canonical CWI's `createAction` + `signAction` flow is reference-based and
doesn't take pre-built rawtx.

**Recommendation:** Defer to a follow-up `Step 3b.7` once Treechat tells us
which inputs/outputs it actually wants signed. Don't speculate. Keep the
NOT_IMPL with a clearer migration hint until we have a real call to translate.

---

### 6. `yours.getExchangeRate()`

**Before (current — wrong):**
```js
defineLegacyProp(legacy, 'getExchangeRate', removed('getExchangeRate',
    'Fetch BSV/USD from a public price source (CryptoCompare, CoinGecko).'));
```

**After:**
```js
defineLegacyProp(legacy, 'getExchangeRate', makeLegacyMethod(function() {
    warnDeprecated('getExchangeRate');
    return fetch(ENDPOINT_BASE + '/wallet/bsv-price', {
        method: 'GET',
        mode: 'cors',
        credentials: 'omit'
    }).then(function(r) {
        if (!r.ok) {
            throw typedError(LEGACY_ERR.NOT_IMPL, 'getExchangeRate',
                'price fetch failed: HTTP ' + r.status);
        }
        return r.json();
    }).then(function(price) {
        // Yours shape: { rate: number, currency: 'USD' }
        // Hodos price endpoint shape: { usd: number, ... } or { priceUsd: number, ... }
        var rate = (typeof price.usd === 'number') ? price.usd :
                   (typeof price.priceUsd === 'number') ? price.priceUsd :
                   (typeof price.price === 'number') ? price.price : null;
        if (rate == null) {
            throw typedError(LEGACY_ERR.NOT_IMPL, 'getExchangeRate',
                'price endpoint returned unrecognized shape');
        }
        return { rate: rate, currency: 'USD' };
    });
}));
```

**Behavior:** Calls existing `/wallet/bsv-price` (cached), reshapes the result
into the Yours `{rate, currency}` envelope.

---

### 7. `yours.getSocialProfile()` — update messaging

**Before (current):**
```js
defineLegacyProp(legacy, 'getSocialProfile', removed('getSocialProfile',
    'Use BRC-100 acquireCertificate + Sigma OAuth provider for identity profile.'));
```

**After:**
```js
defineLegacyProp(legacy, 'getSocialProfile', makeLegacyMethod(function() {
    warnDeprecated('getSocialProfile');
    return Promise.reject(typedError(
        LEGACY_ERR.NOT_IMPL, 'getSocialProfile',
        'social profile resolution is deferred. The legacy Yours/RelayX backend ' +
        'returned {username, avatar, paymail}; the BRC-100 path is window.CWI.' +
        'acquireCertificate + listCertificates with a SocialCert type. Hodos will ' +
        'add a unified resolver as ecosystem demand emerges. For now, dApps should ' +
        'fall back to identity-key-only flows.'
    ));
}));
```

Reason: it's not actually REMOVED (the conceptual capability still exists via
SocialCert), it's just not wired in Hodos yet. NOT_IMPL is honest.

---

## Step 3b commit-level plan

Atomic commits, each independently testable:

| Commit | What | Files |
|---|---|---|
| 3b.1 | Add `pub fn address_to_p2pkh_script` + tests | `handlers.rs` (rust-wallet) + unit tests |
| 3b.2 | Add `POST /wallet/derive-address` handler + route + integration test | `handlers.rs`, `main.rs` |
| 3b.3 | Add `POST /wallet/address-to-script` handler + route + integration test | `handlers.rs`, `main.rs` |
| 3b.4 | Shim: wire `yours.getAddresses()` | `CWIShimScript.h` |
| 3b.5 | Shim: wire `yours.sendBsv()` | `CWIShimScript.h` |
| 3b.6 | Shim: wire `yours.getExchangeRate()` + update `getSocialProfile` deferral message | `CWIShimScript.h` |
| 3b.7 | (Deferred — surface from Treechat first) `yours.getSignatures()` translation | TBD |

Each commit independently rebuildable and smoke-testable. Phase 2 Step 4
literal-split rules apply — verify MSVC literal sizes after each shim edit.

---

## Step 3b test plan

| Method | Test |
|---|---|
| `derive-address` | Unit: known pubkey/address pairs from BIP test vectors. Integration: POST hex pubkey, get address back, verify with `pubkey_to_address` directly. |
| `address-to-script` | Unit: known address/script pairs. Negative: invalid Base58, wrong checksum, wrong version byte. Integration: POST address, get hex script, verify it starts with `76a914` and ends with `88ac`. |
| `yours.getAddresses` | Live: open a yours dApp (Treechat or test page), call `window.yours.getAddresses()` in DevTools console, verify three valid BSV addresses returned, verify they match wallet's own addresses for the same protocols. |
| `yours.sendBsv` | Live: send 1000 sats to a known address via `window.yours.sendBsv([{address, amount: 1000}])`. Verify wallet prompts for approval, payment animation fires, broadcast succeeds, txid matches. |
| `yours.getExchangeRate` | Live: call in DevTools, verify `{rate, currency: 'USD'}` returned with rate matching `/wallet/bsv-price` directly. |
| `yours.getSocialProfile` | Live: call in DevTools, verify typed NOT_IMPL with deferral message. |

**Cross-platform parity:** All Step 3b work is pure Rust + pure JS. No
platform-specific code. Mac smoke is the same as Windows smoke once Mac
build catches up.

---

# Step 3c — ECIES Electrum (BIE1) encrypt/decrypt

## Why we're building it

Per user decision: BIE1 will be useful for a long transition window — many BSV
dApps and on-chain data rely on it. Building a single, well-tested
`crypto/bie1.rs` module gives Hodos interoperability with the entire pre-BRC-2
BSV ecosystem (Yours-era apps, Electrum-BSV-encrypted backups, etc.).

## The BIE1 byte layout

```
[ "BIE1" magic, 4 bytes ASCII ]
[ ephemeral pubkey, 33 bytes compressed secp256k1 ]
[ AES-128-CBC encrypted payload, variable, PKCS#7 padded ]
[ HMAC-SHA256 tag, 32 bytes, over magic + ephemeral_pubkey + ciphertext ]
```

Key derivation:
1. Sender generates ephemeral keypair `(r, R)` where `R = r·G`
2. Computes shared secret `S = r·P` (P = recipient's static pubkey)
3. Hashes `S.x` (32 bytes) with SHA-512 → 64 bytes
4. First 32 bytes → AES-128-CBC key (truncated to 16) + IV (truncated to 16)
5. Last 32 bytes → HMAC-SHA256 key

Decryption inverts the process using recipient's private key `p` and
ephemeral pubkey `R` from the ciphertext.

## Scope and out-of-scope

**In scope (Step 3c):**
- Single-recipient encrypt/decrypt
- Standard BIE1 byte layout (matches Electrum-BSV, Yours v4.5.6)
- Rust module `crypto/bie1.rs`
- Two endpoints: `POST /wallet/encrypt-bie1`, `POST /wallet/decrypt-bie1`
- Shim wiring for `yours.encrypt({message})` and `yours.decrypt({ciphertext})`
- Unit tests with known plaintexts/ciphertexts (lifted from Yours test
  vectors or generated via @bsv/sdk's `ECIES.electrumEncrypt`)

**Out of scope (later if demand surfaces):**
- Multi-recipient (`pubKeys[]` → `N independent ciphertexts`). Yours iterated
  over an array but it was rarely used in practice. If a dApp needs it,
  add then.
- Custom magic-byte variants. BIE1 is what we ship; if other BSV ECIES
  variants surface, add as additional modules.
- Key rotation / forward secrecy beyond what BIE1 already provides (ephemeral
  keypair per message).

## Reuse table for Step 3c

| Need | Reuse |
|---|---|
| secp256k1 ECDH | `rust-wallet/src/crypto/brc42.rs` (already uses secp256k1) or pull from existing `bsv` crate dependency. |
| SHA-512 / HMAC-SHA256 | `sha2`, `hmac` crates (already deps for BRC-2). |
| AES-128-CBC | Add the `aes` + `cbc` crate features — small. (BRC-2 uses AES-256-GCM, so we already have the `aes` crate; CBC mode is a feature flag.) |
| PKCS#7 padding | `block-padding` crate (small dep, ~50 LOC). |
| Master private key access | `get_master_private_key_from_db` in `database/helpers.rs`. |

Net new Rust LOC: ~250 (module + handler + tests). Net new dep crates: 1-2
small ones (`cbc`, `block-padding`).

## Step 3c new Rust endpoints

### `POST /wallet/encrypt-bie1`

```rust
#[derive(Deserialize)]
pub struct EncryptBie1Request {
    pub message: String,       // UTF-8 plaintext OR hex bytes (encoding field below)
    pub encoding: Option<String>,  // 'utf8' (default) | 'hex' | 'base64'
    pub recipientPublicKey: String,  // hex, compressed secp256k1
}

#[derive(Serialize)]
pub struct EncryptBie1Response {
    pub ciphertext: String,    // hex of full BIE1 envelope
}

pub async fn encrypt_bie1(
    state: web::Data<AppState>,
    body: web::Json<EncryptBie1Request>,
) -> HttpResponse { /* ... */ }
```

### `POST /wallet/decrypt-bie1`

```rust
#[derive(Deserialize)]
pub struct DecryptBie1Request {
    pub ciphertext: String,    // hex of full BIE1 envelope
    pub outputEncoding: Option<String>,  // 'utf8' (default) | 'hex' | 'base64'
}

#[derive(Serialize)]
pub struct DecryptBie1Response {
    pub plaintext: String,
}

pub async fn decrypt_bie1(
    state: web::Data<AppState>,
    body: web::Json<DecryptBie1Request>,
) -> HttpResponse { /* ... */ }
```

**Permission gates:** Both endpoints route through `check_domain_approved`
because they use the user's private key for ECDH. Same gate as
canonical `CWI.encrypt`/`CWI.decrypt`.

**Decryption key sourcing:** The user's master private key is the recipient
private key. (Yours used a single static identity-tied key; we match that
behavior. Future: optional key selection via `keyID` parameter.)

## Step 3c shim wiring

Replace the current `eciesElectrumNotImplemented` typed-error stubs with:

```js
defineLegacyProp(legacy, 'encrypt', makeLegacyMethod(function(opts) {
    warnDeprecated('encrypt');
    opts = opts || {};
    if (typeof opts.message !== 'string') {
        return Promise.reject(typedError(LEGACY_ERR.INVALID_ENCODING, 'encrypt',
            'message must be a string'));
    }
    var recipient = opts.pubKey || opts.pubKeys && opts.pubKeys[0];
    if (!recipient) {
        return Promise.reject(typedError(LEGACY_ERR.INVALID_ENCODING, 'encrypt',
            'recipient pubKey is required'));
    }
    if (opts.pubKeys && opts.pubKeys.length > 1) {
        return Promise.reject(typedError(LEGACY_ERR.MULTI_RECIPIENT, 'encrypt',
            'multi-recipient encrypt is deferred; pass a single pubKey for now'));
    }
    return withMutex('encrypt', function() {
        return fetch(ENDPOINT_BASE + '/wallet/encrypt-bie1', {
            method: 'POST',
            mode: 'cors',
            credentials: 'omit',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                message: opts.message,
                encoding: opts.encoding || 'utf8',
                recipientPublicKey: recipient
            })
        }).then(function(r) {
            return r.json().then(function(j) {
                if (!r.ok) throw typedError(LEGACY_ERR.NOT_IMPL, 'encrypt',
                    'BIE1 encrypt failed: ' + (j && j.error));
                return j.ciphertext;
            });
        });
    });
}));

defineLegacyProp(legacy, 'decrypt', makeLegacyMethod(function(opts) {
    warnDeprecated('decrypt');
    opts = opts || {};
    if (typeof opts.ciphertext !== 'string') {
        return Promise.reject(typedError(LEGACY_ERR.INVALID_ENCODING, 'decrypt',
            'ciphertext must be a hex string'));
    }
    return withMutex('decrypt', function() {
        return fetch(ENDPOINT_BASE + '/wallet/decrypt-bie1', {
            method: 'POST',
            mode: 'cors',
            credentials: 'omit',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                ciphertext: opts.ciphertext,
                outputEncoding: opts.outputEncoding || 'utf8'
            })
        }).then(function(r) {
            return r.json().then(function(j) {
                if (!r.ok) throw typedError(LEGACY_ERR.DECRYPT_FAILED, 'decrypt',
                    'BIE1 decrypt failed: ' + (j && j.error));
                return j.plaintext;
            });
        });
    });
}));
```

**Important:** The `LEGACY_OVERRIDES` table in `buildLegacyProvider` still
keeps encrypt/decrypt OUT of the canonical pass-through. The legacy versions
WIN on `window.yours`. The canonical BRC-2 versions are still accessible via
`window.CWI.encrypt`/`window.CWI.decrypt` for new dApps that want them.

## Step 3c commit-level plan

| Commit | What |
|---|---|
| 3c.1 | Add `crypto/bie1.rs` module with `encrypt_bie1` / `decrypt_bie1` pure functions + unit tests with vectors |
| 3c.2 | Add `POST /wallet/encrypt-bie1` + `POST /wallet/decrypt-bie1` handlers + routes + integration tests |
| 3c.3 | Shim: wire `yours.encrypt` / `yours.decrypt` to the new endpoints; remove the `eciesElectrumNotImplemented` stub |

## Step 3c test plan

| Test | What |
|---|---|
| Round-trip unit | Encrypt a known plaintext with a known pubkey, decrypt with the corresponding privkey, verify match. |
| Cross-impl vector | Use @bsv/sdk's `ECIES.electrumEncrypt` to produce a ciphertext, decrypt with our handler, verify match. (Confirms wire-format compat with the canonical BSV implementation.) |
| Encoding variants | utf8, hex, base64 inputs all round-trip cleanly. |
| Malformed ciphertext | Wrong magic, truncated, bad HMAC — each fails with clear error. |
| Wrong recipient | Decrypt with wrong privkey — HMAC verification fails before AES, no oracle leak. |
| Live yours dApp | Use Treechat (or a test page) to encrypt a message via `window.yours.encrypt({message, pubKey})`, store it, then decrypt via `window.yours.decrypt({ciphertext})`, verify round-trip. |

---

# Forward-thinking hooks for Phase 3

When implementing Step 3b, structure the code so Phase 3 (ordinals) doesn't
require refactoring:

1. **Wrap `address_to_p2pkh_script` in a public `address_to_locking_script(address, ScriptType)`** with `ScriptType::P2PKH` as the only variant today. Phase 3 adds `ScriptType::OrdinalLock`, `ScriptType::OneSatEnvelope`. Same endpoint, new variants.

2. **In the shim's `sendBsv`, factor out "translate legacy payment item to canonical output spec" as a helper** (e.g., `_translatePayment(item)`) instead of inlining the address-to-script call. Phase 3 adds `_translateOrdinalTransfer(item)`, `_translateInscription(item)`, all returning canonical output specs that `createAction` can consume.

3. **Add a `script_type` field to the JSON response of `address-to-script`** today (`{lockingScript, script_type: "p2pkh"}`). Phase 3 callers can dispatch on this.

4. **Do NOT** try to anticipate ordinal UTXO classification, ordinal-aware basket assignment, or inscribe handlers. Those belong in Phase 3 because they touch `list_outputs`, basket model, and require ordinal indexing. Premature abstraction will hurt.

---

# Out of scope summary (for both 3b and 3c)

- `yours.inscribe`, `yours.transferOrdinal`, `yours.purchaseOrdinal` — Phase 3
- 1Sat envelope construction and ordinal UTXO classification — Phase 3
- Multi-recipient ECIES Electrum — defer to demand
- Social profile resolution (unified BRC-100 + paymail + identity) — defer to ecosystem demand
- `yours.getSignatures()` — defer until Treechat surfaces a real call

---

# Resume instructions for next session (after clearing context)

1. Read this plan (`STEP_3B_PLAN.md`).
2. Read `MEMORY.md` index and the Phase 2 memory entries.
3. Check git log: latest Phase 2 commit is `63d3098` (encrypt/decrypt fix landed 2026-05-29).
4. Begin with **Commit 3b.1** (Rust `address_to_p2pkh_script` + unit tests). Smallest atomic step.
5. Each commit independently rebuildable and smoke-testable. Don't batch.
6. Verify MSVC literal sizes in `CWIShimScript.h` after every shim edit (Phase 2 Step 4 lesson — see `project_phase2_step4_landed.md`).
7. After Step 3b lands (all six sub-tasks), smoke against Treechat. That guides Step 3c priority.
