# BRC-103 vs Sigma Identity — Comparison & Implementation Plan

> **OBSOLETE STATUS NOTE (2026-05-05):** The "intercepting Sigma Identity auth flows and handling them natively via the wallet" plan documented below is **cancelled**. Sigma Identity uses an iframe-signer architecture (`auth.sigmaidentity.com/signer` with `postMessage` IPC); private keys live on Sigma's domain and external BSV wallet keys cannot substitute. Quoting Sigma's own docs: *"External BAP keys cannot be used directly... External BSV wallet keys cannot substitute because BAP identity derivation is tied to the registered member key."* See `../../phase-0-research/FACT_CHECK_RESULTS.md` Q3 for the full evidence.
>
> **Still useful in this doc:**
> - Part 1's protocol comparison tables (BRC-103 vs BRC-77 vs Sigma protocol vs Sigma Identity) — accurate technical reference
> - Part 2's "Already Implemented" inventory — accurate snapshot of Hodos's BRC-103 capabilities
> - Part 3 Phase 2's BRC-77 / BSM signing primitives spec — preserved as reference material if we later want a content-signing/tipping demo (currently demoted; not on the active sprint plan)
>
> **Obsolete in this doc:**
> - Part 3 Phase 3 (Sigma interception in C++) — both Strategy A (V8 monkey-patch) and Strategy B (HTTP interception) cannot work; Sigma has no `window.sigma` global to override and the iframe signer rejects external keys
> - Part 4 open questions OQ#1–#5 — answered in `../../OPEN_QUESTIONS.md` and `../../phase-0-research/FACT_CHECK_RESULTS.md`
>
> Doc preserved as historical research. For current sprint plan, start at `../../README.md`.

> Research, comparison, and implementation plan for intercepting Sigma Identity auth flows
> in Hodos Browser and handling them natively via the wallet.

## Status: RESEARCH PHASE

| Phase | Status | Notes |
|-------|--------|-------|
| Research — BRC-103/104 (our implementation) | **COMPLETE** | Fully implemented in wallet |
| Research — BRC-77 specification | **COMPLETE** | Spec reviewed, see below |
| Research — Sigma protocol & Sigma Identity | **PARTIAL** | Library understood, auth product needs more investigation |
| Implementation plan | **DRAFT** | Outlined below, needs review |
| Implementation | NOT STARTED | |
| Testing against real sites | NOT STARTED | |
| Developer guide update | NOT STARTED | Update BRC103_SIGMA_AUTH_GUIDE.md |

---

## Part 1: Protocol Comparison

### BRC-103/104 (What We Implement Today)

**Purpose:** Mutual authentication between wallet and server.

**Signing scheme:** ECDSA via BRC-42 derived child keys.

**Flow:**
1. Server sends challenge: `POST /.well-known/auth` with server's pubkey + nonce
2. Wallet derives app-scoped identity key via BRC-42 (`invoice="2-identity"`)
3. Wallet derives signing key via BRC-42 (`invoice="2-auth message signature-{nonce1} {nonce2}"`)
4. Wallet signs concatenated nonces with child private key (ECDSA, DER format)
5. Server verifies signature, mutual auth established

**Key properties:**
- App-scoped identity (different key per app, prevents cross-app tracking)
- BRC-42 ECDH key derivation
- BRC-43 invoice format for key isolation
- Challenge-response with random nonces
- No on-chain component required

**Our implementation:** `rust-wallet/src/handlers.rs` lines 564-802, fully working.

---

### BRC-77 (Message Signature Creation and Verification)

**Purpose:** Sign arbitrary messages with derived keys, with private verification support.

**How it differs from BSM (Bitcoin Signed Message):**
- BSM uses the master private key directly + magic prefix (`"\x18Bitcoin Signed Message:\n"`)
- BRC-77 derives a **new child key for every message** via BRC-42
- This prevents accidental transaction signing (different key = can't be replayed as a tx signature)
- Supports **private verification** (only the intended verifier can verify, not the public)

**Signing algorithm:**
1. Generate random 256-bit key ID (base64)
2. Compute BRC-43 invoice: `"2-message signing-{keyID}"`
3. Derive child private key via BRC-42 (ECDH with verifier's pubkey as counterparty)
4. Sign message with ECDSA using derived child key
5. Serialize: version (4 bytes: `0x42423301`) + signer pubkey (33 bytes) + verifier pubkey (1-33 bytes) + key ID (32 bytes) + DER signature

**Verification:**
- Deserialize to extract signer pubkey, key ID, verifier pubkey
- Verifier uses their own private key + signer's pubkey to derive the same child key via BRC-42
- Verify ECDSA signature with derived child public key

**Key insight: BRC-77 uses BRC-42/43 for key derivation — the same system we already implement.** The gap is that we don't have BRC-77's specific message formatting and serialization, not the underlying crypto.

---

### Sigma Protocol (Transaction-Level Signing)

**Purpose:** Sign custom output scripts in Bitcoin transactions to prove authorship.

**Format:** Appended to OP_RETURN outputs:
```
<locking script>
OP_RETURN
  [data]
  SIGMA
  [algorithm]      ← "BSM" or "BRC-77"
  [signing address] ← P2PKH address from signer's pubkey
  [signature]       ← base64-encoded
  [VIN]             ← input index (txid incorporated into signature for replay protection)
```

**Two signing modes:**
- **BSM:** Standard Bitcoin Signed Message (ECDSA + SHA256 + magic prefix)
- **BRC-77:** Derived child keys with signer pubkey embedded in signature

**Key point:** Sigma protocol itself is for on-chain data attestation — "I signed this OP_RETURN."

---

### Sigma Identity (Auth Product Built on Sigma)

**Purpose:** Passwordless authentication for web apps using Bitcoin keys.

**How it uses Sigma/BRC-77 for login:**
1. App redirects user to `auth.sigmaidentity.com`
2. Sigma issues a challenge message
3. User signs challenge with their Bitcoin key (BSM or BRC-77)
4. Sigma verifies signature
5. User gets OAuth 2.0 token, redirected back to app

**Current problem for Hodos users:**
- Sigma generates a **new key pair in the browser** (stored in browser storage)
- This key has NO connection to the user's Hodos wallet
- User ends up with two separate identities
- Key is ephemeral — lose the browser session, lose the key

**What we want:** Intercept Sigma's challenge and sign it with the Hodos wallet key instead.

---

### Side-by-Side Comparison

| Aspect | BRC-103/104 | BRC-77 | Sigma Identity |
|--------|-------------|--------|----------------|
| **Purpose** | Mutual auth (wallet ↔ server) | Message signing | Web app login (OAuth wrapper) |
| **Key derivation** | BRC-42/43 | BRC-42/43 | BSM or BRC-77 |
| **Uses ECDH?** | Yes | Yes | Depends on mode |
| **App-scoped identity?** | Yes (built in) | Yes (per-verifier) | No (single key) |
| **On-chain?** | No | Optional | Yes (Sigma protocol layer) |
| **Challenge format** | JSON with nonces | Arbitrary message bytes | Unknown (needs investigation) |
| **Signature format** | DER byte array | Custom serialization (version+keys+sig) | Base64 |
| **We implement it?** | Yes, fully | **NO — GAP** | **NO — GAP** |
| **Crypto gap?** | None | Message formatting only | BSM magic prefix OR BRC-77 formatting |

---

## Part 2: What We Already Have vs What We Need

### Already Implemented (No Changes Needed)

| Capability | File | Status |
|-----------|------|--------|
| ECDSA signing (secp256k1, DER) | `crypto/signing.rs` | Full |
| ECDSA verification | `crypto/signing.rs` | Full |
| BRC-42 ECDH key derivation | `crypto/brc42.rs` | Full, passes spec test vectors |
| BRC-43 invoice numbers | `crypto/brc43.rs` | Full |
| BRC-2 encryption (AES-256-GCM) | `crypto/brc2.rs` | Full |
| BRC-103/104 auth handler | `handlers.rs` | Full |
| App-scoped identity derivation | `handlers.rs` | Full |
| Domain permission enforcement | `HttpRequestInterceptor.cpp` | Full |
| Auth approval overlay | `HttpRequestInterceptor.cpp` | Full |

### Gaps to Fill

| Gap | Effort | Why Needed |
|-----|--------|------------|
| **BRC-77 message formatting** | ~50 LOC | Serialize: version + pubkeys + keyID + DER sig |
| **BRC-77 message signing endpoint** | ~100 LOC | `POST /signMessage` handler in Rust |
| **BSM signing** (Bitcoin Signed Message) | ~50 LOC | Magic prefix + SHA256d + ECDSA |
| **BSM verification** | ~50 LOC | Verify BSM-format signatures |
| **Sigma challenge interception** | ~100 LOC in C++ | Detect Sigma auth flow in interceptor |
| **Sigma response injection** | ~100 LOC in C++ | Sign challenge with wallet key, return to page |

**Total estimated: ~450 LOC, ~2-3 days**

The key insight: **BRC-77 uses BRC-42/43 which we already fully implement.** We just need the message formatting wrapper and a new endpoint. The hard crypto is done.

---

## Part 3: Implementation Plan

### Phase 1: Research & Explain (THIS DOCUMENT)

- [x] Document BRC-103/104 flow (our implementation)
- [x] Document BRC-77 specification
- [x] Document Sigma protocol and Sigma Identity
- [x] Identify gaps in our implementation
- [ ] **TODO:** Deep-dive Sigma Identity's actual auth endpoints (need to register a test app)
- [ ] **TODO:** Capture the exact HTTP flow when signing into a Sigma-integrated site
- [ ] **TODO:** Identify what JavaScript APIs Sigma's client library injects into pages

### Phase 2: Implement BRC-77 Signing in Rust Wallet

**New file: `rust-wallet/src/crypto/brc77.rs`**

```rust
/// BRC-77 message signature creation
/// Uses BRC-42 derived child keys (we already have this)
pub fn sign_message_brc77(
    signer_private_key: &[u8],      // Master private key
    verifier_public_key: &[u8],     // Counterparty (or "anyone")
    message: &[u8],                  // Message bytes to sign
) -> Result<Vec<u8>, SigningError> {
    // 1. Generate random 256-bit key ID
    // 2. Build BRC-43 invoice: "2-message signing-{keyID_base64}"
    // 3. Derive child private key via BRC-42 (already implemented)
    // 4. ECDSA sign message hash
    // 5. Serialize: version(4) + signer_pubkey(33) + verifier_pubkey(1-33) + keyID(32) + DER_sig
}

/// BRC-77 message signature verification
pub fn verify_message_brc77(
    serialized_signature: &[u8],
    message: &[u8],
    verifier_private_key: &[u8],    // Verifier's private key (for ECDH)
) -> Result<bool, SigningError> {
    // 1. Deserialize: extract signer pubkey, verifier pubkey, keyID, DER sig
    // 2. Derive child public key via BRC-42
    // 3. Verify ECDSA signature
}
```

**New file: `rust-wallet/src/crypto/bsm.rs`**

```rust
/// Bitcoin Signed Message (legacy BSM format)
pub fn sign_message_bsm(
    private_key: &[u8],
    message: &[u8],
) -> Result<Vec<u8>, SigningError> {
    // 1. Prepend: "\x18Bitcoin Signed Message:\n" + varint(message.len())
    // 2. SHA256d(prefixed_message)
    // 3. ECDSA sign with recovery flag
}

/// Verify BSM signature
pub fn verify_message_bsm(
    address: &str,                  // P2PKH address
    signature: &[u8],              // 65-byte compact signature with recovery
    message: &[u8],
) -> Result<bool, SigningError> {
    // 1. Recover public key from signature
    // 2. Derive P2PKH address from recovered pubkey
    // 3. Compare to claimed address
}
```

**New endpoint in `handlers.rs`:**

```rust
// POST /signMessage
// Body: { algorithm: "brc77" | "bsm", message: "<base64>", 
//         counterparty?: "<hex pubkey>", protocolID?: "...", keyID?: "..." }
// Returns: { signature: "<base64>", publicKey: "<hex>" }
```

### Phase 3: Implement Sigma Interception in C++

**File: `HttpRequestInterceptor.cpp`**

Two interception strategies (decide after Phase 1 research):

**Strategy A: JavaScript injection**
- Detect when page loads Sigma's client library (script src matching `sigmaidentity.com`)
- Inject our own script that overrides Sigma's signing function
- When Sigma calls its sign function, our override calls `window.hodosBrowser` wallet API instead
- Wallet signs with BRC-77 or BSM, returns signature to Sigma's flow

**Strategy B: HTTP interception**
- Detect HTTP requests to `auth.sigmaidentity.com` endpoints
- Intercept the challenge payload
- Sign with wallet key via Rust endpoint
- Inject signed response back into the page's fetch/XHR response

Strategy A is cleaner — it works at the JavaScript API level rather than trying to MITM HTTP requests.

### Phase 4: Testing Against Real Sites

1. **Build test site** using Sigma Identity SDK — register at sigmaidentity.com
2. **Test BRC-103 flow** against our existing test infrastructure
3. **Test Sigma interception** on the test site
4. **Test on real Sigma-integrated BSV sites** (if any exist in production)
5. **Regression test** — ensure BRC-103 native sites still work

### Phase 5: Update Developer Guide

Update `development-docs/BRC103_SIGMA_AUTH_GUIDE.md` with:
- Lessons learned from implementation
- Working code examples (tested, not theoretical)
- Correct Sigma Identity API documentation (from actual integration)
- Complete AI prompt for developers, validated against real flows

### Phase 6: Build Authentication Test Site

Create `frontend/public/auth-test.html` (or separate repo) with:
- BRC-103 "Sign in with BSV Wallet" button (working against our wallet)
- Sigma Identity "Sign in with Sigma" button (working against sigmaidentity.com)
- Account creation flow with optional MFA
- Identity key permission prompt
- Display authenticated user info

This becomes the demo site for the video and the reference implementation for developers.

---

## Part 4: Open Questions (Resolve in Phase 1 Research)

1. **What exact HTTP endpoints does auth.sigmaidentity.com expose?** We need to register a test app and capture the full OAuth flow.

2. **Does Sigma Identity use BSM or BRC-77 mode?** The library supports both. Which does the auth product use by default?

3. **Can Sigma verify a BRC-77 signature from an unknown key?** If Sigma expects the key it generated in the browser, will it accept a different key (our wallet key)?

4. **Is the Sigma challenge format documented?** We need to know exactly what bytes we're signing.

5. **Do any production BSV sites use Sigma Identity today?** If not, this becomes a future-proofing feature rather than an immediate compatibility fix.

6. **Should we propose a BRC standard for QR code payment URIs?** While researching, we extended BIP21 to support paymail/identity keys in the address position. This could become a formal BRC.

---

## Part 5: Key References

### Specifications
- BRC-42 (key derivation): https://bsv.brc.dev/key-derivation/0042
- BRC-43 (invoice format): https://bsv.brc.dev/key-derivation/0043
- BRC-77 (message signing): https://bsv.brc.dev/peer-to-peer/0077
- BRC-103 (mutual auth): https://bsv.brc.dev/peer-to-peer/0103
- BRC-104 (HTTP transport): https://bsv.brc.dev/peer-to-peer/0104
- BRC-52 (identity certificates): https://bsv.brc.dev/peer-to-peer/0052
- BRC-3 (digital signatures): https://bsv.brc.dev/wallet/0003
- BRC-85 (PIKE identity exchange): https://bsv.brc.dev/peer-to-peer/0085
- All BRCs: https://github.com/bitcoin-sv/BRCs

### Sigma
- Sigma protocol library: https://github.com/bitcoinschema/sigma
- Sigma Identity docs: https://docs.sigmaidentity.com
- Sigma Identity auth: https://auth.sigmaidentity.com

### Our Implementation
- BRC-42 derivation: `rust-wallet/src/crypto/brc42.rs`
- BRC-43 invoices: `rust-wallet/src/crypto/brc43.rs`
- BRC-103 handler: `rust-wallet/src/handlers.rs` (lines 564-802)
- Auth interception: `cef-native/src/core/HttpRequestInterceptor.cpp`
- Signing primitives: `rust-wallet/src/crypto/signing.rs`

### Related Hodos Docs
- Developer auth guide: `development-docs/BRC103_SIGMA_AUTH_GUIDE.md`
- QR scan overview: `development-docs/QR_SCAN_OVERVIEW.md` (BIP21 extension for paymail/identity key)
