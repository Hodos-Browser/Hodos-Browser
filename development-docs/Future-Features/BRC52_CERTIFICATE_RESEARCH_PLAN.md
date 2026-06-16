# BRC-52 Certificate Research & Implementation Plan

**Ticket:** [#41 — BRC-52 Certificates Research Spike](https://github.com/BSVArchie/Hodos-Browser/issues/41)
**Date:** 2025-03-25
**Status:** DRAFT — Awaiting review before work begins

---

## Executive Summary

BRC-52 identity certificates in Hodos are currently **read-only**. Users can view and delete certificates acquired externally, but cannot acquire new ones or prove fields to requesting apps. Two external Rust codebases (`bsv-rs` and `rust-wallet-toolbox`) contain canonical, production-tested implementations of everything we're missing. This plan uses them as **reference only** (no new dependencies) to fill the gaps.

---

## Current State Assessment

### What Works

| Component | File | Status |
|-----------|------|--------|
| Certificate storage (DB) | `rust-wallet/src/database/certificate_repo.rs` | Done |
| List certs + field decryption | `rust-wallet/src/handlers/certificate_handlers.rs` (`list_certificates`) | Done |
| Soft delete with overlay check | `rust-wallet/src/handlers/certificate_handlers.rs` (`relinquish_certificate`) | Done |
| Identity resolution (pubkey -> name) | `rust-wallet/src/identity_resolver.rs` | Done |
| Signature verification | `rust-wallet/src/certificate/verifier.rs` | Done |
| Frontend certificate viewer | `frontend/src/components/wallet/CertificatesTab.tsx` | Done |

### What's Missing

| Component | File | Status |
|-----------|------|--------|
| `acquire_certificate()` handler | `rust-wallet/src/handlers/certificate_handlers.rs` | **Empty skeleton** |
| `prove_certificate()` handler | `rust-wallet/src/handlers/certificate_handlers.rs` | **Empty skeleton** |
| Certificate disclosure auto-approve | `cef-native/src/core/HttpRequestInterceptor.cpp` | **Type defined, no logic** |
| Selective disclosure UI | (does not exist) | **Missing** |
| Field-level permission storage | (does not exist) | **Missing** |

---

## Reference Codebases

Using as **reference only** (no new crate dependencies):

### `~/bsv/bsv-rs` — Canonical BRC-52/53 Types & Crypto

The official BSV Rust SDK. 2,578 tests, byte-for-byte cross-SDK compatibility with TypeScript and Go.

| Asset | Path | Relevance |
|-------|------|-----------|
| `Certificate` struct (BRC-52) | `src/auth/certificates/certificate.rs` | Canonical binary format: `to_binary()`, `from_binary()`, `signing_hash()`, `sign()`, `verify()` |
| `MasterCertificate` | `src/auth/certificates/master.rs` | Master keyring management — how certifiers encrypt fields to the subject |
| `VerifiableCertificate` | `src/auth/certificates/verifiable.rs` | Verifier-specific keyring for selective disclosure — the spec for `prove_certificate()` |
| Field encryption key IDs | `certificate.rs` methods | `get_field_encryption_key_id_master()` / `get_field_encryption_key_id_verifiable()` — exact key ID string formats |
| BRC-31 peer auth | `src/auth/peer.rs` | Complete authentication flow with certificate exchange |
| Cross-SDK test vectors | `tests/vectors/auth_certificate.json` | 4 test cases for certificate serialization validation |

### `~/bsv/rust-wallet-toolbox` — Complete Wallet-Level Operations

Full wallet infrastructure (84k lines). Has the implementations Hodos is missing.

| Asset | Path | Relevance |
|-------|------|-----------|
| Certificate acquisition (BRC-104) | `src/wallet/certificate_issuance.rs` (1,090 lines) | **Complete acquire flow** — nonce gen, field encryption, certifier HTTP POST, response validation, storage |
| Certificate proof | `src/wallet/wallet.rs` (`prove_certificate()`) | **Complete selective disclosure** — verifier-specific keyring construction |
| DCAP permissions | `src/managers/permissions_manager.rs` (1,978 lines) | **Domain Certificate Access Control** — per-field, per-domain permission enforcement |
| Overlay lookup | `src/wallet/lookup.rs` (774 lines) | SHIP/SLAP overlay queries, certificate discovery, endpoint failover |
| Settings/certifier defaults | `src/managers/settings_manager.rs` | Default certifier lists (MetaNet Trust, SocialCert), trust configuration |

---

## Phased Plan

### Phase 1: Protocol Audit (Research — answers Q1-5)

**Goal:** Cross-reference Hodos certificate code against canonical implementations. Find protocol divergences. Answer the ticket's protocol questions.

**Tasks:**

1. **Binary format audit** — Compare `rust-wallet/src/certificate/parser.rs` (`serialize_certificate_preimage()` / `parse_certificate_from_json()`) against `bsv-rs` `Certificate::to_binary()` / `from_binary()`. Any divergence in field ordering, length encoding, or hash computation = bug.

2. **Key derivation audit** — Compare field encryption key ID generation in our code against `bsv-rs` `get_field_encryption_key_id_master()` and `get_field_encryption_key_id_verifiable()`. The exact string format matters — a mismatch means we can't decrypt fields from certs issued by standard certifiers.

3. **Overlay endpoint audit** — Compare `identity_resolver.rs` overlay URLs and query format against `rust-wallet-toolbox` `lookup.rs` (`OverlayLookupResolver`). Identify stale or missing endpoints.

4. **Signature verification audit** — Cross-reference `verifier.rs` BRC-42 derivation (protocol/counterparty/keyID) against `bsv-rs` `Certificate::verify()`.

**Protocol questions answered:**

| Question | Where to find the answer |
|----------|------------------------|
| Q1: Multi-key binding | `bsv-rs` `Certificate.subject` is a single `PublicKey`. One cert = one identity key. Re-certifying same social account with new key = new cert (old one still valid unless revoked). |
| Q2: Certificate lifecycle | `bsv-rs` `revocation_outpoint` — spending the UTXO = revocation. Overlay sees the spend. Our `verifier.rs` `check_revocation_status()` queries WhatsOnChain for this. |
| Q3: Certificate types | `rust-wallet-toolbox` `settings_manager.rs` — default certifier lists. Our `identity_resolver.rs` already maps 5 types (Twitter, Discord, Email, Gov ID, Registrant). |
| Q4: Is our code correct? | The audit above determines this. |
| Q5: Overlay interaction | `rust-wallet-toolbox` `lookup.rs` — compare against our `identity_resolver.rs`. |

**Output:** Protocol findings doc with code references. Bug diagnosis (deliverable 4 from ticket).

**Note:** Acquisition "failures" are almost certainly because the handler is empty — not a subtle protocol bug. But the audit may reveal secondary issues in parsing/verification that would bite us when we implement acquire/prove.

---

### Phase 2: Implement `acquire_certificate()` (Code)

**Goal:** Fill the empty handler so users can acquire certificates from SocialCert/MetaNet Trust.

**BRC-104 protocol flow (from `rust-wallet-toolbox/src/wallet/certificate_issuance.rs`):**

```
1. User triggers acquisition (e.g., "Link my Twitter" via socialcert.net in browser)
2. Wallet generates nonce
3. Wallet creates MasterCertificate with encrypted fields
4. HTTP POST to certifier with:
   - x-bsv-auth-version: 0.1
   - x-bsv-identity-key: <identity pub key hex>
   - Body: { type, fields, nonce }
5. Certifier verifies social account (OAuth on their side)
6. Certifier returns signed certificate + keyring
7. Wallet verifies signature + HMAC
8. Wallet stores certificate + encrypted fields in DB
```

**Adaptation to Hodos patterns:**

| Toolbox uses | Hodos equivalent |
|-------------|-----------------|
| `ProtoWallet` for crypto | `rust-wallet/src/crypto/` (BRC-42, BRC-2, signing) |
| `WalletStorage` trait | `certificate_repo.rs` |
| `reqwest` HTTP client | `reqwest` (same) |
| `MasterCertificate` type | Port the keyring logic into our `certificate/` module |

**Open question:** Does SocialCert handle the OAuth redirect flow externally (user visits socialcert.net in browser, completes OAuth, certifier calls wallet API)? Or does the wallet initiate? The toolbox suggests the wallet POSTs to the certifier — need to confirm against SocialCert's actual API.

**Depends on:** Phase 1 (protocol audit confirms our parsing/verification is correct).

---

### Phase 3: Implement `prove_certificate()` (Code)

**Goal:** Fill the empty handler so apps can request certificate fields and get cryptographic proofs.

**BRC-53 selective disclosure flow (from `bsv-rs` `VerifiableCertificate`):**

```
1. App requests: { certType, certifier, fieldsToReveal: ["userName", "profilePhoto"] }
2. Wallet finds matching certificate in DB
3. For each requested field:
   a. Decrypt master keyring entry -> revelation key
   b. Re-encrypt revelation key for the verifier's public key (BRC-42 derived key)
   c. Add to verifier-specific keyring
4. Return: { certificate, verifierKeyring, decryptedFields }
```

**Building blocks already in Hodos:**
- BRC-2 decrypt (used in `list_certificates`)
- BRC-42 key derivation (`crypto/brc42.rs`)
- Certificate field storage (`certificate_repo.rs`)

**The new piece** is constructing the verifier keyring — re-encrypting revelation keys for a specific verifier's public key. This is a straightforward BRC-42 derivation + BRC-2 encrypt operation.

**Depends on:** Phase 1 (key derivation audit), Phase 2 (acquire gives us certs to prove).

---

### Phase 4: Auto-Approve Model for Identity/PII (Design Doc Only)

**Goal:** Design the permission system for certificate field disclosure. **Implementation is a follow-up ticket** — this touches all three layers (Rust, C++, React).

**Proposed model (inspired by `rust-wallet-toolbox` DCAP):**

#### Permission Levels

```
Per (domain, certType, fieldName) -> permission:
  ALWAYS_ALLOW  -> auto-approve, no prompt
  ASK_ONCE      -> prompt once, remember for this domain
  ALWAYS_ASK    -> prompt every time
  ALWAYS_DENY   -> silently block
  UNSET         -> use default based on field sensitivity
```

#### Field Sensitivity Defaults

| Risk | Fields | Default | Rationale |
|------|--------|---------|-----------|
| Low | displayName, userName, profilePhoto, avatar | ASK_ONCE | Low PII, high utility |
| Medium | email, phone, social handles | ASK_ONCE | Moderate PII, common requests |
| High | governmentId, fullLegalName, address | ALWAYS_ASK | High PII, prompt every time |
| Critical | anything in cert types we don't recognize | ALWAYS_ASK | Safe fallback |

#### Architecture

| Layer | Component | What it does |
|-------|-----------|-------------|
| Rust | New `domain_certificate_permissions` table | Stores per-(domain, certType, field) permissions |
| Rust | New permission check endpoint | C++ queries before showing prompt |
| C++ | `HttpRequestInterceptor.cpp` extension | Checks certificate permissions, auto-approves or queues notification |
| React | New certificate approval overlay | Shows which fields, which domain, Allow once / Always / Deny |

#### Notification Fatigue Strategy (Q9)

- First request from a domain: always prompt with field details
- User picks "Always allow for [field]" -> auto-approve future requests for that field from that domain
- Low-risk fields: single prompt with "Allow all basic info for this site" shortcut
- High-risk fields: no batch approval, must approve individually
- Settings page: global view of all certificate permissions by domain (like cookie settings)

---

### Phase 5: Display Improvements (Quick Wins — can be done independently)

**Goal:** Polish `CertificatesTab.tsx`. Low-risk, high-visibility.

1. **Status badges** — Valid / Expired / Revoked indicators (call `check_revocation_status()` from a new endpoint or piggyback on `listCertificates`)
2. **Human-readable field names** — Lookup table: `userName` -> "Username", `profilePhoto` -> "Profile Photo", `discordUsername` -> "Discord", etc.
3. **User labeling** — New `label` column in `certificates` table, editable in the UI
4. **Tooltip info icons** — `(i)` next to serial number, revocation outpoint, certifier hex with explanatory text
5. **Relative timestamps** — "Issued 3 months ago" instead of raw epoch

---

## Sequencing & Dependencies

```
Phase 1 (Protocol Audit) -----> Phase 2 (acquire) -----> Phase 3 (prove)
         |
         +-----> Phase 4 (Auto-approve design doc)

Phase 5 (Display polish) <--- independent, can run anytime
```

**Suggested order:** 1 -> 5 -> 2 -> 3 -> 4

Phase 5 is a good warm-up (low risk, immediate visual payoff). Phase 4 is design-only and can be written after understanding the protocol (Phase 1).

---

## Risk / Open Questions

1. **Certifier interaction model** — How does SocialCert's OAuth flow work from the user's perspective? Does the user visit socialcert.net in the Hodos browser, or does the wallet API initiate it? Affects Phase 2 architecture significantly.

2. **Scope for this ticket** — The ticket says "research spike, implement if straightforward." Phases 1, 4, and 5 clearly fit. Phases 2 and 3 are medium-effort implementation. Should they be in this ticket or a follow-up?

3. **Cross-platform** — All new Rust code is cross-platform. But if Phase 4 implementation happens later, the C++ auto-approve changes need `#ifdef _WIN32` / `#elif __APPLE__` guards per CLAUDE.md rules.

4. **Testing** — No integration tests exist for acquire/prove flows. The `bsv-rs` test vectors (`auth_certificate.json`) should be ported to validate our parser.

5. **Coordination** — If Archie is already working on any of these phases, we need to sync to avoid conflicts. The phases are modular enough that different people could own different phases.

---

## Files That Will Be Modified

| Phase | Files |
|-------|-------|
| 1 | Read-only audit — output to `development-docs/` |
| 2 | `rust-wallet/src/handlers/certificate_handlers.rs`, possibly new module in `rust-wallet/src/certificate/` |
| 3 | `rust-wallet/src/handlers/certificate_handlers.rs`, `rust-wallet/src/certificate/selective_disclosure.rs` |
| 4 | Design doc only — `development-docs/` |
| 5 | `frontend/src/components/wallet/CertificatesTab.tsx`, `rust-wallet/src/database/certificate_repo.rs` (label column), `rust-wallet/src/handlers/certificate_handlers.rs` (revocation status in list response) |
