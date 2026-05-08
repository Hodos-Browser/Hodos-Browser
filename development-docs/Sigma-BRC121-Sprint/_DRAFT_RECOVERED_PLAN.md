# DRAFT — Plan Recovered from Crashed Planning Session

**Status:** raw recovery, to be split out into the proper sprint files (`README.md`, `CHECKLIST.md`, `OPEN_QUESTIONS.md`, `ARCHITECTURE.md`, and per-phase folders) and then deleted.

This file exists as a crash safety net so the planning work isn't lost if Bun segfaults again. The plan below is what the previous session produced before crashing during a major-expansion step (4 parallel research agents that completed but whose results were never folded in).

---

## Plan: Sigma Auth + BRC-121 Payments + Ecosystem Demo Sprint

### Context

Hodos Browser today is a Cluster A wallet — it covers BRC-100/103/104 mutual auth, BRC-29 PeerPay, Paymail, BEEF/SPV, and the createAction/signAction stack. It does not cover the two largest gaps to "real apps people use today":

1. **Sigma Identity OAuth** (the auth layer for the 1Sat Ordinals / b-open-io cluster — 1sat.market, 3DOrdi, Zoide, FireSat, MetaLens, BitChat Nitro, etc.).
2. **BRC-121 Simple 402 Payments** (the HTTP-monetization payment profile that pairs with BRC-100 / Babbage stack).

This plan does three things, in order of independence (most independent first):

- **Phase 1:** Ship BRC-121. It's pure additive, reuses existing PeerPay derivation (same protocol ID `3241645161d8`), and has zero protocol unknowns.
- **Phase 0 / 2:** Unblock Sigma open questions, then ship Sigma signing primitives + interception. The primitives (`bsm.rs`, `brc77.rs`, `/signMessage`) are independent of the open questions; the interception design is gated.
- **Phase 4:** Build localhost demo HTML pages + LLM-ready developer integration `.md` guides. Distribute to BSV devs to feed to Claude/Replit assistants.

The user wants the demos to be the path to "Hodos works with everything." 1Sat Ordinals support (Cluster B payments — distinct from Sigma auth) and MNEE (Cluster C tokens) are deferred and surfaced as scope questions.

The two seed docs (`development-docs/BRC103_SIGMA_AUTH_GUIDE.md` and `BRC103_SIGMA_COMPARISON_AND_IMPLEMENTATION.md`) already capture most of the BRC-77/BSM design; this plan extends them with BRC-121 and the demo/dev-marketing track.

---

### Ecosystem snapshot (the "why" for phasing)

| Cluster | Anchor | Auth | Payment | Hodos status |
|---------|--------|------|---------|--------------|
| A — Babbage / BRC-100 / MetaNet | Project Babbage (Ty Everett) | BRC-103/104 | createAction + BEEF; BRC-121 fits naturally here | ~95% covered. BRC-121 missing. |
| B — 1Sat Ordinals + Sigma | b-open-io / Yours Wallet (Satchmo / rohenaz) | Sigma OAuth (PKCE + BSM/BRC-77 + BAP) | 1Sat Ordinals UTXO transfers, BSV20/21, raw P2PKH | 0% covered. Sigma auth is the unlock; ordinal payments are a separate sprint. |
| C — HandCash / Paymail / MNEE | HandCash | HandCash Connect OAuth (proprietary) | Paymail (BRC-28/70), MNEE stablecoin (BSV21) | Paymail covered; MNEE not. HandCash Connect not on roadmap (apps route through HandCash itself). |

**Notable flags from research:**
- `bsv-blockchain/metanet-desktop` repo was archived Oct 2025 and renamed BSV Desktop. Confirm the rename didn't break our reference / interop checks.
- BitChat Nitro listed as Sigma "trusted by" but a BSV forum post calls it broken — pick a different Sigma test app.
- BRC-121 may have no production servers yet. We may need to ship our own demo server as Phase 1's integration partner.

---

### Verified code-path facts (from reading the source, not the docs)

These are the load-bearing references the implementation hangs on. All verified during planning:

- BRC-29 protocol ID `3241645161d8` is used at `rust-wallet/src/handlers.rs:4348, 5991, 6077, 15299` and in `monitor/task_check_peerpay.rs:201`. BRC-121 uses the same protocol ID — we can reuse the derivation primitive directly.
- Key handler entry points:
  - `well_known_auth` → `handlers.rs:564`
  - `create_action` → `handlers.rs:3381`
  - `create_action_internal` → `handlers.rs:3577`
  - `peerpay_send` → `handlers.rs:15224`
  - `paymail_send` → `handlers.rs:15637`
- `cef-native/src/core/HttpRequestInterceptor.cpp` already has:
  - `OnResourceResponse` hook at line `2056` (currently a no-op `return false` — this is the BRC-121 entry point and the work to land it is small).
  - `x-bsv-*` header allowlist at line `1748` (BRC-121 retry headers will pass through unchanged).
  - `isWalletEndpoint` route table at line `2075` (add BRC-121 + signMessage routes here).
- `crypto/mod.rs` already exports `brc42`, `brc43`, `signing`, `keys`, `brc2` — adding `bsm` and `brc77` is two `pub mod` lines.
- V8 handler pattern (`BRC100Handler.cpp` + `BRC100Bridge.cpp`) is the template for any new `window.hodosBrowser.*` methods we add.
- `SyncHttpClient` is the cross-platform localhost client to use (Windows WinHTTP / macOS libcurl); never call WinHTTP directly in new code (per top-level CLAUDE.md rule 9).

---

### Phase 0 — Unblock Sigma open questions (1–2 weeks calendar, no code)

These three questions gate Sigma interception design but do not block BRC-121 or the Sigma signing primitives. Run Phase 0 in parallel with Phase 1.

| OQ | Question | Evidence | Time |
|----|----------|----------|------|
| OQ#1 | Will sigmaidentity.com accept a BRC-77 or BSM signature from a key it didn't generate? Or does it require a key Sigma issued in the browser? | Register a developer test app at sigmaidentity.com; capture the OAuth round-trip with mitmproxy/Charles using a real Sigma-integrated app (1sat.market or 3DOrdi — not BitChat Nitro). Inspect challenge format, signature encoding (base64 length, recovery flag), what message is hashed, what's verified. | 3–5 days |
| OQ#2 | Where does the Sigma client lib live — stable `window.*` global, or inline-bundled per-app? | View-source 1sat.market + 3DOrdi. Look for `window.sigma`, BSM, or similar globals. Determines whether we hook a JS symbol or intercept HTTP. | 1–2 days |
| OQ#3 | Are any production servers actually returning HTTP 402 with `x-bsv-sats` / `x-bsv-server` headers today? | `curl -i` candidate URLs from BRC-121 spec / BSV community. If none, our Phase 4 demo server becomes the integration partner — not a blocker. | 0.5 day |

**Decision matrix for Sigma after Phase 0:**

| OQ#1 | OQ#2 | Strategy |
|------|------|----------|
| Server accepts any BAP-anchored key | Stable JS global | **Strategy A** — V8 monkey-patch in `simple_render_process_handler.cpp::OnContextCreated` (clean, fragile to lib version changes) |
| Server accepts any BAP-anchored key | No global, inline-bundled | **Strategy B** — HTTP-layer interception via `OnResourceResponse` (more general, requires intimate Sigma OAuth knowledge) |
| Server requires Sigma-issued key | any | **Hold** — ship `/signMessage` primitives only, document Sigma auth interception as upstream-blocked until b-open-io changes server policy |

---

### Phase 1 — BRC-121 Simple 402 Payments (1–2 weeks, parallel with Phase 0)

Why first: purely additive, no protocol unknowns, reuses BRC-29 derivation we already ship.

#### 1A — Rust handler `pay_402`

**File:** `rust-wallet/src/handlers.rs` — new function near `peerpay_send` (`:15224`).

**Request body:** `{ server_identity_key: hex, satoshis: u64, original_url: string, requesting_domain: string }`

**Logic** — copy `peerpay_send` and swap one line:

1. Validate `server_identity_key` is a 33-byte compressed pubkey (same check pattern as `:15241–15248`).
2. Generate random 16-byte derivation prefix (base64). For BRC-121 the suffix is `base64(unix_time_ms)` (this is the only divergence from PeerPay where suffix is also random).
3. Build invoice: `format!("2-3241645161d8-{} {}", prefix_b64, time_b64)` — same string format as `:15299`, same protocol ID.
4. Call `crypto::brc42::derive_child_public_key(&master_privkey, &server_pubkey, &invoice)`.
5. Build P2PKH locking script via `create_p2pkh_script_from_pubkey` (existing helper, used at `:15314`).
6. Construct a `CreateActionRequest` with `noSend=true`, `randomizeOutputs=false` (mirror PeerPay pattern at `:15332–15340`), invoke `create_action_internal` (`:3577`).
7. Extract atomic BEEF from the response, base64-encode it.
8. Domain-permission gate at top via `check_domain_approved()` — the existing 3-layer model handles approval-overlay invocation.

**Response:**

```json
{
  "beef_base64": "...",
  "sender_pubkey_hex": "...",
  "nonce_base64": "...",
  "time_ms": 1714771200000,
  "vout": 0
}
```

These are exactly the values the C++ side needs to emit as `x-bsv-beef`, `x-bsv-sender`, `x-bsv-nonce`, `x-bsv-time`, `x-bsv-vout`.

**Route registration:** `rust-wallet/src/main.rs` — add `.route("/wallet/pay402", web::post().to(handlers::pay_402))` next to existing PeerPay routes.

**Reuse, do not reimplement:** `derive_child_public_key`, `create_p2pkh_script_from_pubkey`, `create_action_internal`, BEEF construction, fee logic, service-fee output, `check_domain_approved`. Phase 1 should add ~150 LOC of glue, no crypto.

#### 1B — CEF 402 response interception

**File:** `cef-native/src/core/HttpRequestInterceptor.cpp` — fill in the existing `OnResourceResponse` stub at line `2056`.

**Logic for the 402 case** (skip everything else; return `false` to let normal flow continue):

1. Read `x-bsv-sats` and `x-bsv-server` headers from the response.
2. Extract requesting domain via existing `extractDomain` helper (used at `:1899`).
3. Check `DomainPermissionCache::GetInstance().getPermission(domain)`. If auto-approve covers this amount → silent. Otherwise queue via `PendingRequestManager` + show approval overlay (existing pattern; reuse the payment-approval overlay).
4. Call `SyncHttpClient::Post("http://localhost:31301/wallet/pay402", body)` with the server pubkey + sats + URL + domain.
5. On success, re-issue the original request with the five `x-bsv-*` retry headers. The header allowlist at `:1748` already permits any `x-bsv-` prefix, so they pass through cleanly.
6. Return the retried response to the page.

**Cross-platform:** `SyncHttpClient` already abstracts WinHTTP/libcurl. No new `#ifdef` blocks. Per `cef-native/src/core/CLAUDE.md`, this is the right pattern.

**Auto-approve integration:** plug into `SessionManager::Get/UpdateSpending` exactly like `createAction` does — same per-tab spending limit, same rate-limit window.

#### 1C — Acceptance criteria for Phase 1

- Localhost demo server returns 402 + `x-bsv-sats` + `x-bsv-server` → Hodos completes payment → server validates BEEF → returns 200 with paid content. Round-trip <2s.
- Auto-approve respects per-tab session limits.
- Negative test: insufficient funds → wallet returns structured error → page sees the original 402.
- Regression test: createAction, PeerPay, and Paymail flows still work end-to-end (real-world test against the standard verification basket per top-level CLAUDE.md).
- BRC-121 path adds zero schema changes (reuses existing `outputs` and `transactions` tables).

---

### Phase 2 — Sigma Auth (3–4 weeks, partially gated by Phase 0)

#### 2A — Rust signing primitives (build immediately, ~300 LOC, independent of Phase 0)

These are useful regardless of how interception resolves; they also unlock content-signing/tipping use cases referenced in `development-docs/Possible-MVP-Features/`.

**`rust-wallet/src/crypto/bsm.rs`** (new, ~100 LOC):
- `sign_message_bsm(privkey, message)` — `sha256d("\x18Bitcoin Signed Message:\n" + varint(len) + msg)`, ECDSA with recovery flag, base64 65-byte output.
- `verify_message_bsm(address, sig, message)` — recover pubkey, derive P2PKH, compare.
- Reuse: `crypto/signing.rs::sign_ecdsa` is the foundation.

**`rust-wallet/src/crypto/brc77.rs`** (new, ~100 LOC):
- Implement BRC-77 message signing per `development-docs/BRC103_SIGMA_COMPARISON_AND_IMPLEMENTATION.md` Part 3 Phase 2. Serialization: `version(4) + signer_pubkey(33) + verifier_pubkey(1-33) + keyID(32) + DER_sig`.
- Reuse: `crypto/brc42.rs::derive_child_private_key`, `crypto/signing.rs::sign_ecdsa`.

**`rust-wallet/src/handlers.rs`** — new handler `sign_message` (~100 LOC):
- Body: `{ algorithm: "bsm" | "brc77", message_base64: string, counterparty?: hex, protocol_id?: string, key_id?: string }`
- Returns: `{ signature_base64: string, public_key_hex: string }`
- Domain-permission gate via `check_domain_approved`.

**Module wiring:** `crypto/mod.rs` — add `pub mod bsm; pub mod brc77;`. Route in `main.rs`: `.route("/signMessage", web::post().to(handlers::sign_message))`.

#### 2B — Sigma interception (DECISION GATED ON PHASE 0)

Two strategies, picked by the Phase 0 decision matrix above:

- **Strategy A — V8 monkey-patch.** In `cef-native/src/handlers/simple_render_process_handler.cpp::OnContextCreated`, override `window.sigma.sign` (or whatever stable symbol Phase 0 finds) to route through `window.hodosBrowser.signMessage`. Pattern is already used by `BRC100Handler::RegisterBRC100API` for `window.hodosBrowser.brc100.*`.
- **Strategy B — HTTP interception.** In `OnResourceResponse` (same hook BRC-121 lands in), detect Sigma OAuth challenge endpoints, sign locally, inject signed payload into the redirect/response.

**Defer until Phase 0 returns:** any C++ Sigma-specific bridge code, any client-secret handling, any `auth.sigmaidentity.com` URL hardcoding. The only thing safe to ship pre-Phase-0 is the `/signMessage` endpoint in 2A.

#### 2C — Acceptance criteria for Phase 2

- Sigma sign-in on a real production app (chosen during Phase 0) succeeds end-to-end with a Hodos-derived key.
- Same key persists across sessions (no ephemeral browser key — that's the headline win over today's Sigma UX).
- BRC-103 mutual auth flows untouched (regression-tested against the standard basket).
- `/signMessage` is independently exercised by a unit test for both BSM and BRC-77 modes.

---

### Phase 3 — 1Sat Ordinals (DEFERRED — scope question)

Cluster B has the largest app count we don't address. But ordinal support is not Sigma's coattails — it's a separate, larger body of work:

- New UTXO classification (1-sat outputs with inscribed data — current `outputs` schema doesn't distinguish them).
- BSV20/21 token indexer integration (existing design work in `development-docs/BSV-Tokens/BSV21_PLAN_A_BACKEND.md` and `BSV21_PLAN_B_FRONTEND.md` — read these before scoping).
- New ordinal transfer transaction builder.
- New monitor task to sync ordinal UTXOs from a 1Sat indexer.
- UI for viewing inscriptions.

**Recommendation:** Sigma auth alone unlocks app discovery in Cluster B (users can sign in to 1sat.market with their Hodos identity). Ordinal transfer is a v2 sprint. Surface to user as scope question Q1.

---

### Phase 4 — Demo Pages + Developer Integration Guides (1–2 weeks, separate sprint)

The user explicitly framed this as "probably a different sprint or at least a different phase." Confirmed — depends on Phase 1 + 2A landing.

#### 4A — Localhost demo servers (Node.js + Express, one folder per cluster)

| Demo | Validates | Doubles as Phase-1/2 acceptance partner? |
|------|-----------|------------------------------------------|
| `demo-brc100-createaction` | Cluster A — createAction round-trip with domain-permission overlay | yes (already works; smoke test) |
| `demo-brc121-402` | Cluster A — 402 → payment → retry round-trip | yes — Phase 1 acceptance partner |
| `demo-sigma-oauth` | Cluster B — Sigma OAuth login, identity display | yes — Phase 2 acceptance partner |
| `demo-brc29-peerpay` | Cluster A — PeerPay send/receive | yes — regression smoke test |

Each demo is self-contained: `git clone && npm install && npm start`, served on a configurable localhost port.

#### 4B — LLM-ready developer `.md` guides

Each demo emits a sibling `.md` written specifically to be pasted into a developer's Claude / Replit / Cursor session. Required contents:

- "Here is your BRC-X wallet client setup" with copy-pasteable code blocks.
- Error handling and edge cases (insufficient funds, denied permission, expired session).
- Expected user flow narrated step-by-step (so the LLM understands what UI states the dev needs to handle).
- Reference Hodos as the test wallet, with the standard verification basket the user can run.
- Known caveats (e.g., "as of 2026-05, Sigma server requires X").

#### 4C — Repo location (SCOPE QUESTION Q3)

- **Option 1:** `Hodos-Browser/demos/` subdirectory.
- **Option 2 (recommended):** separate `hodos-demos` repo. The audience is external BSV devs, not Hodos contributors. Independently versionable, easier to share a single repo URL.

---

### Cross-cutting concerns

#### Testing matrix (real-world verification basket)

Per top-level CLAUDE.md "Testing Standards" — use this as the Sprint Standard tier addition:

| Cluster | Production app | Validates | Pre-existing? |
|---------|----------------|-----------|---------------|
| A — Babbage | Babbage App Catalog (todo / polls / proof-of-existence) | BRC-103/104 + createAction | yes |
| A — Babbage | Mars weather demo | createAction with micropayment | yes |
| A — Babbage | Our `demo-brc121-402` | BRC-121 round-trip | new (Phase 1) |
| B — Sigma | 1sat.market sign-in | Sigma OAuth interception | new (Phase 2) |
| B — Sigma | 3DOrdi sign-in | Sigma OAuth (different lib bundling) | new (Phase 2) |
| C — Paymail | Any Paymail send target | Paymail regression | yes |

Avoid BitChat Nitro (status contested per BSV forum).

#### Metanet Desktop archive flag

`bsv-blockchain/metanet-desktop` was archived Oct 2025 and renamed BSV Desktop. One-time check: spot-check any URL/repo references in `development-docs/` and `reference/ts-brc100/`. No code change unless an import actually breaks.

#### MNEE stablecoin

Design doc exists at `development-docs/BSV-Tokens/MNEE_STABLECOIN_IMPLEMENTATION.md`. Cluster C foothold but adds another protocol surface. Surfaced as scope question Q2.

#### handcash-mpp (out of scope for this sprint)

Investigated `https://github.com/GenericCPU/handcash-mpp` because its README mentions HTTP 402 — initially looked like a BRC-121 candidate. **It is not.**

| Aspect | BRC-121 | handcash-mpp |
|--------|---------|--------------|
| 402 signal | Headers `x-bsv-sats` + `x-bsv-server` | JSON body `{ type: "https://paymentauth.org/problems/payment-required", challengeId, handcash: { paymentRequestUrl, paymentRequestId } }` |
| Retry credential | Headers `x-bsv-beef` + `x-bsv-sender` + `x-bsv-nonce` + `x-bsv-time` + `x-bsv-vout` | HS256 JWT in `x-handcash-receipt` header (or `Authorization: Bearer …`) |
| Settlement | On-chain BEEF, BRC-29 derivation, ECDSA | Out-of-band: HandCash hosted checkout URL OR server-side `@handcash/sdk` `Connect.pay` with delegated authToken. No on-chain settlement client-side. |
| Identity | secp256k1 pubkeys (server + client) | HandCash app-id/app-secret + HMAC-shared `receiptSecret` |
| Trust model | Self-custody, signatures verifiable on-chain | Custodial; HandCash holds the keys |
| BRC alignment | Yes, BRC-121 | None — author's own "Machine Payments Protocol" coinage |

**Maturity:** unofficial individual project (Brandon Cryderman / GenericCPU), days old, 0 stars, prototype.

**Compatibility in Hodos today:** Works passively. A user navigating to a handcash-mpp-protected URL gets the 402 JSON, follows the HandCash hosted-pay URL in-browser, and completes payment like any Chrome user. Our CEF interceptor doesn't recognize the 402 (no `x-bsv-*` headers) so it stays out of the way. Nothing breaks. No work required for passive compatibility.

**Why a Hodos-native handcash-mpp integration is NOT this sprint:**
- Implementing it would require either (a) shallow UX work to auto-pop the HandCash payment window — a HandCash-coupling that doesn't generalize, or (b) embedding a HandCash Connect delegated authToken — a custodial credential, which conflicts with Hodos's "keys never leave the Rust process" invariant on its face (the credential is HandCash-issued, not wallet-derived, but it still represents money-spending authority that Hodos would have to manage securely).
- Implementing BRC-121 gives zero compatibility with handcash-mpp and vice versa. They are not different dialects of the same protocol; they are different protocols.

**Recommendation:** Out of scope for this sprint. If we later spin up a real machine-to-machine / agent-payments sprint, the right targets are BRC-105 (HTTP Service Monetization Framework — BRC-103 mutual auth + on-chain settle) and BRC-120 (x402 stateless settlement-gated HTTP). handcash-mpp itself is a study artifact, not an integration target — its protocol shape may inform a Hodos UX decision later (e.g., should we show a "pay via HandCash" affordance on 402 pages we don't recognize), but no code lands for it now.

**One small free win we could grab:** If we're already adding a generic 402 handler in `OnResourceResponse` for BRC-121, we could log unrecognized 402 responses with a "this site uses a non-BRC-121 payment protocol" debug note, so future telemetry helps us decide whether handcash-mpp / BRC-105 / BRC-120 ever clears the threshold for native support.

#### Security invariants (will not be relaxed by this sprint)

- Private keys never leave the Rust process. All signing in Phase 1 + 2A is Rust-side; CEF/JS layer only proxies. (THE_WHY.md invariant; top-level CLAUDE.md rule 1.)
- New CEF code uses cross-platform conditionals + `SyncHttpClient`. (Top-level CLAUDE.md rule 9.)
- 3-layer domain permission model preserved end-to-end. (THE_WHY.md; `cef-native/src/core/CLAUDE.md` HTTP Interception Pattern.)
- No DB schema changes in Phase 1 or 2A. (Top-level CLAUDE.md invariant 2; `rust-wallet` CLAUDE.md.) Phase 3 (Ordinals) would change this — gated by explicit user approval.

---

### Critical files to modify

| File | What changes |
|------|--------------|
| `rust-wallet/src/handlers.rs` | Add `pay_402` (Phase 1, near `:15224`); add `sign_message` (Phase 2A) |
| `rust-wallet/src/main.rs` | Register `/wallet/pay402` and `/signMessage` routes |
| `rust-wallet/src/crypto/mod.rs` | `pub mod bsm; pub mod brc77;` |
| `rust-wallet/src/crypto/bsm.rs` | NEW — Phase 2A |
| `rust-wallet/src/crypto/brc77.rs` | NEW — Phase 2A |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | Fill in `OnResourceResponse` at `:2056` (Phase 1); add `/wallet/pay402` and `/signMessage` to `isWalletEndpoint` at `:2075` |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | (Phase 2B Strategy A only) V8 monkey-patch in `OnContextCreated` |
| `development-docs/BRC103_SIGMA_COMPARISON_AND_IMPLEMENTATION.md` | Update status table after Phase 0 evidence comes in |
| `development-docs/BRC103_SIGMA_AUTH_GUIDE.md` | Update with verified flows post-Phase-2 |
| (new repo or subdir) `hodos-demos/` | Phase 4 demo servers + integration `.md`s |

---

### Verification (end-to-end, after each phase)

**After Phase 1 (BRC-121):**
1. Start demo server returning 402 with `x-bsv-sats=100`, `x-bsv-server=<our-test-pubkey>`.
2. Browse to it from Hodos. Approve when domain-permission overlay appears.
3. Verify the retry succeeds and the demo server validates BEEF (it knows the test pubkey + can re-derive the expected address).
4. Run the standard 5-min verification basket (youtube.com, x.com, github.com) — confirm no regression.

**After Phase 2A (signing primitives, no interception yet):**
1. Unit-test `bsm.rs` and `brc77.rs` against the spec test vectors.
2. `curl -X POST http://localhost:31301/signMessage -d '{"algorithm":"bsm","message_base64":"..."}'` — verify base64 signature returned and is valid.

**After Phase 2B (Sigma interception):**
1. Sign in to 1sat.market with Hodos. Verify identity persists across browser restart (no ephemeral key).
2. (Additional steps were truncated in the crashed session — to reconstruct.)

---

## Things lost in the crash (to redo)

The crashed session had just kicked off a major expansion when Bun segfaulted. None of the following made it into the plan:

1. **JungleBus + 1Sat + Sigma stack research** — agent finished, output lost
2. **b-open-io / bopen.ai Claude skill research** — agent finished, output lost
3. **"Real apps people use today" justification** — agent finished, output lost (was specifically responding to user pushback that the claim wasn't well-evidenced)
4. **A 4th research stream** — appears to have completed too, output lost
5. **Proposed dev-docs folder structure addition** (now superseded by this skeleton)
6. **Architecture diagram additions**
7. **Expanded open questions list** (Q4, Q5 placeholders only — Q6 about M2M follow-up sprint is preserved)
8. **The full prompts sent to each of the 4 research agents** — only the 3-5 word descriptions are visible in the transcript

The 4 research streams need to be re-dispatched with fresh prompts, then their findings folded into `ARCHITECTURE.md` / `OPEN_QUESTIONS.md` / `README.md`.
