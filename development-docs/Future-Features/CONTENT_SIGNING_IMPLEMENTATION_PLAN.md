# Content Signing & Tipping — Implementation Plan

**Issue:** #42
**Date:** 2026-03-25
**Status:** Plan — awaiting approval
**Estimated effort:** ~2.5 weeks (reduced from 4 thanks to existing infrastructure)

---

## Executive Summary

The recipient autocomplete (#58) and PeerPay infrastructure already solved ~60-70% of the backend needed for content signing & tipping. Identity resolution, BRC-52 certificate decryption, PeerPay payment sending, MessageBox relay, and the scriptlet injection pipeline are all production-ready.

**New work is concentrated in 4 areas:**
1. X.com content script (JS DOM injection)
2. OP_RETURN signing endpoint (Rust)
3. Signature lookup mechanism
4. Minimal overlay UI

**Tipping requires ZERO changes to PeerPay** — it's just `peerpay_send` with the creator's identity key.

---

## What We Already Have (Reusable)

| Capability | Proven By | File |
|-----------|-----------|------|
| Identity key → name/avatar | Recipient autocomplete | `identity_resolver.rs` |
| BRC-52 two-stage cert decryption | Fixed in #58 | `identity_resolver.rs:350-450` |
| SocialCert X.com handle → key | Trusted certifier configured | `identity_resolver.rs:89` |
| PeerPay send by identity key | Send form | `handlers.rs:12290+` |
| Encrypted MessageBox relay | PeerPay | `messagebox.rs` + `authfetch.rs` |
| Scriptlet injection pipeline | Adblock + fingerprint | `simple_render_process_handler.cpp` |
| Per-domain feature gating | Fingerprint protection | `FingerprintProtection.h` |
| BRC-42 key derivation | Multiple features | `crypto/brc42.rs` |
| ECDSA signing | `create_signature` endpoint | `handlers.rs:2710+` |
| Transaction creation | `create_action` | `handlers.rs` |
| Certificate verification | Cert sprint | `certificate/verifier.rs` |

---

## Phase 0: Sign (Week 1)

**Goal:** Users can sign their own tweets on-chain via a button injected into X.com.

### 0.1 — Rust: Content signing endpoint

**New file:** `rust-wallet/src/content_normalize.rs`
**Modified:** `rust-wallet/src/handlers.rs`, `rust-wallet/src/main.rs`

**Endpoint:** `POST /wallet/content/sign`

```json
// Request
{
  "content": "Tweet text here",
  "platform": "x.com",
  "contentId": "1519480761749016577",
  "timestamp": 1711350000,
  "includeCertificate": true
}

// Response
{
  "success": true,
  "txid": "abcdef...",
  "contentHash": "e3b0c44...",
  "identityKey": "02abc...",
  "signature": "3045022100...",
  "certificateTxid": "fedcba...",
  "fee": 434
}
```

**OP_RETURN format** (`hodos.sign` protocol prefix):

```
OP_FALSE OP_RETURN
  "hodos.sign"           // Protocol prefix (10 bytes)
  0x01                   // Version byte
  <SHA-256 hash>         // 32 bytes — normalized content hash
  <timestamp>            // 8 bytes — big-endian uint64
  "x.com"                // Platform identifier (UTF-8)
  "1519480761749016577"  // Content ID (UTF-8)
  <compressed pubkey>    // 33 bytes — signer's identity key
  <DER signature>        // 70-72 bytes — ECDSA over fields 0-6
  <cert TXID or OP_0>   // 32 bytes or empty — SocialCert TXID
```

**TX size:** ~434 bytes → ~434 sats fee (~$0.0002). Auto-deducted from wallet.

**Signing key derivation** (BRC-42/43):
- `protocolID`: `[2, "hodos content signing"]`
- `keyID`: `"{platform}-{contentId}"`
- `counterparty`: `"anyone"` (publicly verifiable)

**Content normalization rules:**
1. HTML entity decode
2. Unicode NFC normalization
3. Strip trailing t.co media URLs
4. Collapse whitespace → single space
5. Trim

Reuses: `create_action()`, `create_minimally_encoded_chunk()` from pushdrop, `broadcast_transaction()`.

### 0.2 — C++: Content signing singleton + injection gate

**New files:**
- `cef-native/include/core/ContentSigningScript.h` — Embedded JS constant (pattern: `FingerprintScript.h`)
- `cef-native/include/core/ContentSigning.h` — Singleton with enable/disable, domain check

**Modified:**
- `simple_render_process_handler.cpp` — Inject script in `OnContextCreated` for x.com/twitter.com domains
- `simple_handler.cpp` — IPC handlers: `content_sign_request`, `content_verify_request`, `content_verify_batch`

**Domain gate:** Only inject on `x.com`, `twitter.com`, `mobile.twitter.com`.

### 0.3 — JS: X.com content script

**Embedded in `ContentSigningScript.h` as C++ raw string literal.**

Architecture:
- **MutationObserver** on `[data-testid="primaryColumn"]` with `subtree: true`
- Two-phase init: watch `document.documentElement` until `primaryColumn` appears, then narrow scope
- **Tweet detection** via `[data-testid="tweet"]` selector
- **Debounced processing** (150ms batch) with `data-hodos-processed` attribute

Content extraction:
- **Tweet ID:** `tweet.querySelector('time')?.closest('a')` → parse `/status/{id}` from href
- **Handle:** `[data-testid="User-Name"]` → find `<span>` starting with `@`
- **Text:** Tree-walk `[data-testid="tweetText"]`, replace emoji `<img>` with `alt` text
- **Timestamp:** `<time datetime="...">` attribute

UI injection:
- **Sign button** appended to action bar `div[role="group"]` (after Share button)
- Shield icon with checkmark, matches X's action button styling
- Dark mode support via `getComputedStyle(document.body).backgroundColor` detection

IPC flow:
```
Sign click → cefMessage.send('content_sign_request', {tweetId, handle, text, timestamp, url})
  → C++ handler → POST localhost:31301/wallet/content/sign
  → Response → frame->ExecuteJavaScript → window.postMessage({type: 'hodos_content_sign_result', ...})
  → Content script updates button to "Signed" state
```

---

## Phase 1: Verify (Week 2)

**Goal:** Verification badges appear on tweets that have on-chain signatures.

### 1.1 — Signature lookup service (MVP approach)

**Option chosen for MVP:** Monitor task pattern (C1) — 2-3 days.

Since WhatsOnChain has no OP_RETURN content search API, and BSV Overlay Services doesn't have `ls_content_signature` yet, the MVP approach is:

1. When content script detects tweets, send batch of tweet IDs to C++ via `content_verify_batch`
2. C++ forwards to new Rust endpoint `POST /wallet/content/verify`
3. Rust checks local signature cache first (SQLite table of known signatures)
4. For cache misses, query a known announcement endpoint (future: overlay services)

**New DB table:** `content_signatures`
```sql
CREATE TABLE content_signatures (
    id INTEGER PRIMARY KEY,
    platform TEXT NOT NULL,
    content_id TEXT NOT NULL,
    content_hash BLOB NOT NULL,
    signer_key TEXT NOT NULL,
    signature BLOB NOT NULL,
    txid TEXT NOT NULL,
    certificate_txid TEXT,
    signed_at INTEGER NOT NULL,
    verified_at INTEGER NOT NULL,
    UNIQUE(platform, content_id, signer_key)
);
```

**Why this works for MVP:** Users who sign tweets populate their own local DB. When they view their own tweets or tweets by people who signed on the same device, badges appear immediately. Cross-device verification comes in Phase 1.5 when overlay services adds `ls_content_signature`.

**Production path (Phase 1.5):** Request BSV Overlay Services add `ls_content_signature` topic type. Query pattern identical to existing `ls_identity` in `identity_resolver.rs`.

### 1.2 — Verification badge UI

**Inject into:** `[data-testid="User-Name"]` area, after display name link.
**Appearance:** Green shield icon (verified) or red shield (failed).
**Click action:** Tooltip showing signer name (from `IdentityResolver::resolve()`), timestamp, txid link to WhatsOnChain.

### 1.3 — Identity resolution for signers

Directly reuse `IdentityResolver::resolve(identity_key)` → `ResolvedIdentity { name, avatar_url, source }`.

This is the exact same code path the recipient autocomplete uses. When a badge is clicked, we resolve the signer's identity key to show "Signed by Alice Chen (X/Twitter via SocialCert)".

---

## Phase 2: Tip (Week 2.5)

**Goal:** Users can tip verified content creators directly from tweets.

### 2.1 — Tip button UI

**Inject alongside** the Sign button in the action bar (or as a sub-action of the verify badge).
**Amount input:** Small inline dropdown or overlay popup (e.g., $0.01, $0.05, $0.25, custom).

### 2.2 — Wire to PeerPay (ZERO backend changes)

```
Tip click → amount selection
  → cefMessage.send('content_tip', {identity_key, amount_satoshis})
  → C++ handler → POST localhost:31301/wallet/peerpay/send
    { "recipient_identity_key": "02abc...", "amount_satoshis": 1000 }
  → Existing PeerPay flow (BRC-42 derive → TX → MessageBox relay)
  → Success response → update UI to "Tipped!"
```

The `peerpay_send` endpoint is fully generic — takes identity key + amount, does everything else. Zero new payment code needed.

### 2.3 — Tip metadata (optional enhancement)

Add optional fields to the PaymentToken JSON (backward-compatible — existing parsers ignore unknown fields):

```json
{
  "customInstructions": { "derivationPrefix": "...", "derivationSuffix": "..." },
  "transaction": [...],
  "amount": 1000,
  "tipReason": "Great tweet",
  "contentUrl": "https://x.com/user/status/123",
  "contentHash": "sha256:abcdef..."
}
```

Requires minor change to `peerpay_send` handler to accept and forward optional tip fields. Receiver's `parse_payment_token()` would extract and display them.

---

## File Changes Summary

### New Files (5)

| File | Purpose | Lines (est.) |
|------|---------|-------------|
| `rust-wallet/src/content_normalize.rs` | Content normalization + unit tests | ~100 |
| `cef-native/include/core/ContentSigningScript.h` | Embedded JS content script | ~250 |
| `cef-native/include/core/ContentSigning.h` | Singleton: enable/disable, domain check | ~60 |
| `cef-native/src/core/ContentSigning.cpp` | Singleton implementation | ~80 |

### Modified Files (5)

| File | Change | Lines (est.) |
|------|--------|-------------|
| `rust-wallet/src/handlers.rs` | `content_sign` + `content_verify` handlers | ~250 |
| `rust-wallet/src/main.rs` | Route registration + module import | ~5 |
| `rust-wallet/src/database/migrations.rs` | V13 migration: `content_signatures` table | ~20 |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | Inject content script in `OnContextCreated` | ~15 |
| `cef-native/src/handlers/simple_handler.cpp` | IPC handlers for sign/verify/tip | ~80 |

### Optional Modifications (Phase 2 tip metadata)

| File | Change |
|------|--------|
| `rust-wallet/src/handlers.rs` | Add optional tip fields to `peerpay_send` |
| `rust-wallet/src/monitor/task_check_peerpay.rs` | Parse optional tip fields |

**Total new code:** ~860 lines across 9 files.

---

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| X.com changes `data-testid` selectors | Low | High | Versioned selector config in scriptlet (update without browser release) |
| Content normalization mismatch (JS vs Rust) | Medium | High | Rust is canonical — always re-normalizes server-side |
| Signature lookup has no discovery (MVP) | Expected | Medium | Local-only for MVP; overlay services for production |
| SocialCert acquisition UX missing | Known gap | Medium | Users acquire cert via SocialCert website; in-browser flow deferred |
| Adblock response filter interferes | Low | Low | X.com tweets not filtered by our adblock (only YouTube) |
| Fee economics if BSV price rises | Low | Low | ~434 sats, sub-cent even at $500/BSV |

---

## Testing Plan

| Phase | Test | Method |
|-------|------|--------|
| 0 | Sign endpoint creates valid OP_RETURN TX | Rust unit test + curl |
| 0 | Content normalization is deterministic | Rust unit tests with known test vectors |
| 0 | Content script detects tweets on X.com | Manual in HodosBrowser (requires X.com login) |
| 0 | Sign button appears and sends IPC | Manual in HodosBrowser |
| 0 | Signed TX appears on WhatsOnChain | Manual verification |
| 1 | Verify badge appears for locally-signed tweets | Manual in HodosBrowser |
| 1 | Identity resolution shows signer name | Manual (requires SocialCert) |
| 2 | Tip sends PeerPay to creator | Manual (two wallets needed) |
| 2 | Creator receives tip notification | Manual (check receiver wallet) |

---

## Dependency Chain

```
Phase 0.1 (Rust endpoint)  ──────────────────────┐
Phase 0.2 (C++ singleton + injection gate) ───────┤
Phase 0.3 (JS content script) ───────────────────┤
                                                   ▼
                                          Phase 1 (Verify)
                                                   │
                                                   ▼
                                          Phase 2 (Tip)
```

Phase 0 sub-tasks (0.1, 0.2, 0.3) can be built **in parallel** — they converge at integration testing.

---

## Relationship to Existing Work

| Prior Work | How It Applies |
|-----------|---------------|
| **Recipient autocomplete (#58)** | Identity resolution, BRC-52 decryption, overlay services query pattern — all directly reused |
| **PeerPay** | Tipping is literally `peerpay_send` with an identity key. Zero new payment code. |
| **Fingerprint protection** | `FingerprintScript.h` + `FingerprintProtection.h` are the exact template for `ContentSigningScript.h` + `ContentSigning.h` |
| **Adblock scriptlets** | Scriptlet injection pipeline (`OnBeforeBrowse` pre-cache → `OnContextCreated` inject) is the delivery mechanism |
| **Certificate sprint** | `verifier.rs` handles BRC-52 cert verification. Content signing adds a new use case for the same infrastructure. |
| **MessageBox** | Tip receipt notifications use the same `payment_inbox` polling that PeerPay already exercises |
