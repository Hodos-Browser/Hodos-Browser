# Permission UX Design — Hodos Browser

**Date:** 2026-05-06.
**Status:** Design — gates Phase 1.5 implementation. Trimmed 2026-05-06 to align with existing Hodos infrastructure.
**Audience:** product owner + future implementers.

---

## TL;DR

Hodos's permission UX goal: **one well-explained first-visit prompt per site, then mostly silence until something genuinely high-stakes happens.** The privacy perimeter — identity-key reveal, key-linkage proofs, sensitive cert fields, large spends — always interrupts. Most other permissions are pre-bundled at site connect via a `wallet-manifest.json` the site declares.

This is built on **existing Hodos infrastructure**, not parallel structures:

- `domain_permissions` table (V3 migration) with `per_tx_limit_cents`, `per_session_limit_cents`, `rate_limit_per_min`, `max_tx_per_session` defaults of $1/$10/30/100 stays as-is.
- `cert_field_permissions` child table stays; gets a new `sensitivity` column.
- `ApprovedSitesTab` Default Limits + `DomainPermissionForm` "Always notify" toggle + new "Allow without limits" button cover the tier-of-trust UX without a separate preset table.
- Right-click "Manage Site Permissions" context menu (`MENU_ID_MANAGE_PERMISSIONS`) preserved.
- Three new child tables of `domain_permissions` mirror the `cert_field_permissions` shape — for protocol, basket, and counterparty grants.

This document covers the rationale and design positions. The Phase 1.5 README is the implementation scope.

---

## Table of contents

1. [Where Hodos is today](#1-where-hodos-is-today)
2. [BRC-100 model — capabilities and limitations](#2-brc-100-model--capabilities-and-limitations)
3. [Cost/benefit and risk matrices](#3-costbenefit-and-risk-matrices)
4. [Three settings, not three tiers](#4-three-settings-not-three-tiers)
5. [First-visit bundle strategy](#5-first-visit-bundle-strategy)
6. [Architecture](#6-architecture)
7. [Best practices](#7-best-practices)
8. [Notes for future work](#8-notes-for-future-work)
9. [References](#9-references)

---

## 1. Where Hodos is today

Hodos's existing permission system is a **3-layer auto-approve model** — domain whitelist → spend cap → rate limit — with six popup types and several silent decisions.

### Existing popups

| Popup | Triggers when | What it asks | Storage |
|---|---|---|---|
| **Domain connect** | First-time site requests any BRC-100 capability | Allow site to verify identity, request payments, store/access data | `domain_permissions` row |
| **Payment confirmation** | Spend exceeds per-tx cap, or BSV/USD price unavailable | Approve once / Deny / Modify limits | Session memory + `monitor_events` |
| **Rate limit exceeded** | Site exceeds per-minute request rate (default 30/min) | Approve / Deny / Modify | Session-only (60s sliding window) |
| **Session-tx-count exceeded** | Site exceeds session payment count (default 100) | Approve / Deny / Modify | Session memory |
| **Certificate disclosure** | `proveCertificate` requests fields not pre-approved | Pick fields / All / Deny + remember-for-this-site | `cert_field_permissions` |
| **No wallet alert** | Site requests BRC-100 without wallet existing | Set up wallet / Not now | N/A |

### Existing UI surface

- **`ApprovedSitesTab.tsx`** in the wallet — **Default Limits** section editing global defaults via `/wallet/settings`, plus embedded **`DomainPermissionsTab.tsx`** with sort/paginate/edit/revoke per-site.
- **`DomainPermissionForm.tsx`** with **"Always notify" toggle** (zeros all limits) plus warning banner if limits exceed $5/tx or $50/session.
- **`BRC100AuthOverlayRoot.tsx`** for connect/payment/cert-disclosure prompts.
- **Right-click "Manage Site Permissions"** at `simple_handler.cpp:6696` opens the form quickly for revocation.
- **`TokensTab.tsx`** groups outputs by basket name in title-case display.

### Silent decisions today

| Silent decision | Notes |
|---|---|
| Per-call signing after domain approval | Wallet signs every `createAction`/`signAction` from approved sites within limits |
| Non-payment BRC-100 endpoints | `getPublicKey`, `listCertificates`, `listOutputs` etc. forwarded silently for any approved domain |
| Cert field auto-disclosure | After first approval per (domain, cert-type, field), future calls silent |
| Session counters reset on tab close | Per user direction, kept as-is |
| Rate-limit increments | Counter ticks silently; user only learns on hit |

### Gaps Phase 1.5 closes

1. **No per-protocol gate.** Approved domain → any protocolID is allowed.
2. **No per-counterparty gate.** Level-2 protocols don't ask separately about each peer.
3. **No identity-key reveal prompt.** `getPublicKey({ identityKey: true })` is silent on approved domains today; should always prompt.
4. **Two missing handlers.** `revealCounterpartyKeyLinkage` and `revealSpecificKeyLinkage` don't exist; once they do, they should always prompt.
5. **No first-visit manifest bundle.** Sites can't declare upfront what they need.
6. **No sensitivity classification on cert fields.** All field disclosures get the same prompt today, regardless of stakes.

---

## 2. BRC-100 model — capabilities and limitations

BRC-100 (`@bsv/wallet-toolbox`'s `WalletPermissionsManager`) gives us four protocol-level permission categories, plus a UX wrapper.

### The four categories

| Spec name | Acronym | What it gates | Hodos column home |
|---|---|---|---|
| Domain Protocol Access | DPACP | (origin, protocolID, keyID, counterparty) | new `domain_protocol_permissions` |
| Domain Basket Access | DBAP | (origin, basket) — UTXO basket access read/write | new `domain_basket_permissions` |
| Domain Certificate Access | DCAP | (origin, type, certifier, fields[]) — selective field disclosure | existing `cert_field_permissions` (+ `sensitivity` col) |
| Domain Spending Authorization | DSAP | (origin, monthly satoshi cap) — total satoshi flow | existing `domain_permissions` columns ($/tx, $/session, rate, max-tx) |

Plus a coarse pact-style gate for level-2 protocols:

| Type | What it gates | Hodos column home |
|---|---|---|
| `CounterpartyPermissionRequest` | (origin, counterparty) — peer-bound crypto | new `domain_counterparty_permissions` |

Plus a UX-layer wrapper:

| Type | Purpose |
|---|---|
| `GroupedPermissionRequest` | Bundle multiple of the above into one approval prompt; intended primary path per BRC-73 |

### Capabilities BRC-100 gives us

- **Granular consent.** Four orthogonal axes of trust.
- **Per-(origin, protocol) isolation.** Strong privacy default.
- **Manifest pre-bundling.** BRC-73 lets a dApp declare every permission it will ever need in one `manifest.json`; one prompt covers all.
- **Selective disclosure for certs.** Reveal "name" without revealing "DOB."
- **Cross-wallet interop.** Apps targeting BRC-100 work in any conforming wallet.
- **Per-counterparty cryptographic privacy by default.** Two sites cannot correlate the same user without explicit identity-key reveal.

### Limitations / unmet promises

- **On-chain grants are aspirational.** Babbage's reference stores grants as PushDrop UTXOs in admin baskets, but no shipped wallet syncs them across devices. Hodos goes **local-first**.
- **No example app ships a manifest.** BRC-73 is the stated intent; we'd be the first.
- **No graceful denial pattern.** Babbage example apps assume permissions are granted; deny throws unhandled errors. We need to define a typed-error shape.
- **`metanet-desktop` was archived October 2025.** Babbage's reference UX is in transition; we shouldn't anchor on it.
- **No expiry default.** BRC-100 says "0 = forever" — we go with explicit expiry on new tables (1-year default, "never" with warning).
- **No spec for certificate sensitivity.** No BRC standardizes which fields are sensitive. Hodos defines its own classifier; ecosystem standardization is future work.

### Where this leaves us

The BRC-100 protocol gives the right shape. The ecosystem hasn't built the UX layer on top yet. **Hodos can lead by being the first wallet to ship a manifest-first permission flow with plain-language descriptions and a clear privacy perimeter** — without pretending we're tier-presetted. We use existing Default Limits as the recommended path; "Always notify" toggle as the cautious path; "Allow without limits" button as the trusted-site escape hatch.

---

## 3. Cost/benefit and risk matrices

These matrices answer "what's at stake when a user grants X to site Y?" Inputs to the manifest-prompt's plain-language descriptions and to the engine's decision logic.

### Matrix A — Per-permission cost/benefit

| Permission category | User benefit | User cost | Auto-approve viability |
|---|---|---|---|
| **Connect** (DPACP at session start) | Site loads and works | Site learns wallet is installed and origin is approved | Always prompt (first visit) |
| **Read-only crypto** (`verifySignature`, `verifyHmac`) | Site verifies its claims to user | Nothing exposed | Silent always |
| **Derived signing** (`createSignature`, `createHmac` levels 0–1) | Site signs messages on user's behalf with site-specific derived key | A site-specific signature exists; verifiable as user but does NOT link to identity key | Auto-approve below per-tx cap; prompt above |
| **Encryption** (level 2 with counterparty) | Encrypted DMs / files | Counterparty knows user has interacted with them through this site | Prompt first time per (site, counterparty); silent thereafter |
| **Basket read** (`listOutputs({ basket })`) | Site shows user their assets | Site sees full basket contents | Bundle into connect prompt; silent thereafter |
| **Basket write** (insert/remove via `createAction`) | Site can manage assets | Site can move assets out (within spend cap) | Bundle into connect prompt; silent within cap |
| **Cert read** (`listCertificates`) | Site sees user has verified identity | Site learns user has cert from issuer X | Bundle into connect prompt |
| **Cert disclose — low** (display name, avatar) | Site personalizes UX | Site has display name | Bundle into connect prompt |
| **Cert disclose — medium** (email, country) | Site can email / regionalize | Site has contact info | Bundle with opt-out required |
| **Cert disclose — high** (phone, address) | Site can SMS / ship physical goods | Site has more contact info | Always prompt individually |
| **Cert disclose — highest** (DOB, government ID, SSN) | Site can KYC | Site has KYC data forever; breach risk | Always prompt + explicit confirm |
| **Spending — small** (≤ per-tx cap) | One-click micropayment / tipping | Money leaves wallet | Auto-approve within cap + rate limits |
| **Spending — large** (> per-tx cap) | Big purchase | More money leaves | Always prompt |
| **Identity key reveal** (`getPublicKey({ identityKey: true })`) | Cross-site identity (single login many apps) | Every site that sees this can correlate all your activity forever | **Always prompt + extra prominence** |
| **Counterparty key linkage** (`revealCounterpartyKeyLinkage`) | Compliance, dispute resolution, identity proof to verifier | Verifier learns ALL keys used with that counterparty share a root | **Always prompt + named verifier required** |
| **Specific key linkage** (`revealSpecificKeyLinkage`) | Prove one specific key is yours | Verifier learns that key is yours | **Always prompt + named verifier required** |

### Matrix B — Pre-approval risk by permission category

| Pre-approve scope | Worst case if site goes bad | Mitigations |
|---|---|---|
| **Connect** | Site keeps coming back; has whatever permissions it accumulated | Per-domain Revoke (right-click Manage Site Permissions; revoke in form) |
| **Protocol use** at level 0–1 within tier | Bad signatures get produced under user's derived key; do not link to identity | Derived keys: damage bounded to per-(origin, protocolID) blast radius |
| **Basket write** within cap | Site can drain up to per-tx × rate × max-tx-per-session | Spending caps + rate limits |
| **Cert low fields** | Display name and avatar — usually public anyway | Low risk; mostly annoyance |
| **Cert medium fields** | Email/contact info — phishing target | Per-call prompt regardless |
| **Cert high/highest fields** | KYC data; breach victim | Always-prompt; never bundled |
| **Spending in cap** | Site drains up to cap | Cap + rate limit; user can revoke |
| **Identity key — never auto-approve** | Site correlates user across all of Web3 | Always prompt |
| **Key linkage — never auto-approve** | Verifier gets cryptographic proof of linkage | Always prompt |

### Matrix C — Decision flowchart

```
                              user's per-domain
                              settings + global
                              defaults
                                    │
                                    ▼
   permission request comes in
            │
            ├── is this the privacy perimeter?
            │     (identityKey, revealKeyLinkage, sensitive cert fields,
            │      spend > per-tx-cap)
            │           │
            │           └─ YES → always prompt regardless of grants
            │
            ├── was this pre-bundled in the connect manifest?
            │           │
            │           └─ YES + in-bound → silent
            │           └─ YES + out-of-bound → prompt with delta
            │
            ├── is this a new (origin, protocolID, counterparty, basket)?
            │           │
            │           └─ YES → prompt with bundle option
            │
            ├── does it pass per-tx cap + rate limit + max-tx-per-session?
            │           │
            │           └─ YES → silent
            │
            └── otherwise → prompt
```

---

## 4. Three settings, not three tiers

The earlier draft proposed Cautious / Balanced / Power tier presets. Dropped — Hodos already has the right primitives.

The user picks one of three setups for any given domain:

### 1. **Recommended defaults** — $1/$10/30/100

Per-tx ≤ $1, per-session ≤ $10, ≤ 30 requests/min, ≤ 100 transactions per session. Defined as:

- Globally in `settings.default_per_tx_limit_cents` etc., editable via `ApprovedSitesTab` Default Limits.
- Per-domain in `domain_permissions` row, defaulting to the global values.

This is what most users will use without thinking about it. Auto-approves micropayments and routine BRC-100 calls within the cap.

### 2. **"Always notify me"** toggle (existing)

Zeroes all limits via `DomainPermissionForm` toggle (line 51-71). Every payment, every request triggers a prompt. For users who explicitly want to be asked about everything.

### 3. **"Allow without limits (advanced)"** — NEW button (Phase 1.5)

Sets very high caps with one-time warning + don't-show-again checkbox. For trusted sites where the user knows what they're doing. Lives in both `DomainPermissionForm` (per-site) and `ApprovedSitesTab` Default Limits (global if user really wants).

### Why this works

- No new tables.
- Matches user mental model — "default / always ask / always allow."
- Per-site override always available; global default the user can move once.
- Right-click "Manage Site Permissions" still revokes any of these instantly.

The new sub-permission tables (protocol/basket/counterparty) are **scope** axes, not preset axes. They live in the same domain row regardless of which of the three setups above the user picked.

---

## 5. First-visit bundle strategy

Biggest UX win is **front-loading permission asks into one connect prompt**. Requires the dApp to ship a manifest, but we support both manifest-driven AND inferred-by-call paths so adoption isn't gated on dev cooperation.

### How the bundle works

1. **Hodos checks `https://<origin>/.well-known/wallet-manifest.json`** on first BRC-100 call.
2. **If a manifest exists**, single bundled prompt covering everything in it, with plain-language descriptions and the user's existing default limits applied.
3. **If no manifest**, lightweight per-call prompts with a one-time toast suggesting the dev should ship a manifest.

### Manifest format (proposed)

```json
{
  "version": "1.0",
  "name": "1Sat Market",
  "description": "BSV NFT marketplace and trading platform",
  "iconUrl": "https://1sat.market/icon.png",
  "expiresAt": 1773427200,
  "permissions": {
    "protocols": [
      {
        "protocolID": [1, "ordinal-listing"],
        "keyID": "*",
        "purpose": "Sign NFT listings and transfers"
      }
    ],
    "baskets": [
      { "name": "1sat", "access": "read_write", "purpose": "Manage your NFT collection" }
    ],
    "certificates": [
      {
        "type": "https://socialcert.io/v1",
        "fields": ["displayName", "avatar"],
        "purpose": "Show your name and avatar on listings"
      }
    ],
    "spending": {
      "perTransactionUsd": 10,
      "perSessionUsd": 100,
      "purpose": "Marketplace fees and bid placement"
    },
    "counterparties": [
      { "type": "list-1sat-marketplace", "purpose": "Encrypted bid messages" }
    ]
  }
}
```

Hodos shows this as plain-language permissions, **not protocolID strings**.

### Connect-prompt UX

```
┌─ Connect to 1Sat Market ─────────────────────────────────┐
│   [icon]  1Sat Market                                    │
│           https://1sat.market                            │
│           BSV NFT marketplace and trading platform       │
│                                                          │
│   This site is asking permission to:                     │
│     ✓ Sign NFT listings and transfers                    │
│     ✓ Manage your NFT collection                         │
│     ✓ Show your name and avatar on listings              │
│     ✓ Send micropayments up to $10 each, max $100/sess.  │
│     ✓ Send encrypted bid messages                        │
│                                                          │
│   It will not be able to:                                │
│     ✗ See your identity key (cross-site identity)        │
│     ✗ See assets outside the 1sat basket                 │
│                                                          │
│   Limits: $1/tx, $10/session  [Edit]                     │
│                                                          │
│   [ ⓘ What does each permission mean? ]                   │
│                                                          │
│   [ Customize ]    [ Decline ]    [ Connect ]  ← primary │
└──────────────────────────────────────────────────────────┘
```

Three buttons:
- **Connect** (primary): grants everything in the manifest with default limits.
- **Customize**: opens a per-permission checkbox view; user can turn individual permissions off or override limits.
- **Decline**: site doesn't get access; receives typed denial error.

After connect: bundle permissions are silent within limits. New permissions trigger a smaller "1Sat Market is now also asking for X — allow?" prompt. Privacy perimeter calls (identity-key, key-linkage, sensitive cert fields) ALWAYS prompt regardless of bundle.

### When manifest is missing

- First call shows lightweight connect prompt for just that capability.
- Subsequent calls for new capabilities show smaller delta-prompts.
- After 3 in one session, surface a one-time toast: "1Sat Market is asking for permissions piecemeal — you can ask the developer to ship a wallet-manifest.json for a smoother experience."

Console log on every BRC-100 call from a manifest-less site: `[Hodos] No wallet-manifest.json at https://<origin>/.well-known/. Permission UX is degraded; please ship a manifest.`

---

## 6. Architecture

> **Legend:** ╔═NEW═╗ Phase 1.5 components. Existing components in plain boxes. (W/M) = present on both Windows and macOS.

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ React Frontend                                                                │
│ ─────────────────────────────────────────                                     │
│ EXISTING:                                                                     │
│   ApprovedSitesTab (Default Limits + DomainPermissionsTab)                    │
│   DomainPermissionForm (per-site limits + Always notify toggle)               │
│   BRC100AuthOverlayRoot (connect / payment / cert disclosure)                 │
│                                                                               │
│ EXTENSIONS — Phase 1.5:                                                       │
│   ApprovedSitesTab → ╔══ + Allow without limits button ══╗                    │
│                       ╚══ + Sensitivity classifier editor ══╝                 │
│   DomainPermissionForm → ╔══ + Allow without limits ══╗                       │
│                           ╚══ + Specific permissions section ══╝              │
│                           ╔══ + Cert fields with sensitivity ══╗              │
│   BRC100AuthOverlayRoot → ╔══ + Manifest-bundle path ══╗                      │
│                           ╚══ + Sensitivity-aware disclosure ══╝              │
│                                                                               │
│ NEW overlays — Phase 1.5:                                                     │
│   ╔═ IdentityKeyRevealOverlayRoot (always-prompt) ═╗                          │
│   ╔═ KeyLinkageRevealOverlayRoot (always-prompt) ═╗                           │
│   ╔═ ProtocolPermissionPromptOverlayRoot (manifest-less new scopes) ═╗        │
│   ╔═ CounterpartyPermissionPromptOverlayRoot (level-2 new peers) ═╗           │
└───────────────────────────────────────┬───────────────────────────────────────┘
                                        ↓
┌───────────────────────────────────────────────────────────────────────────────┐
│ CEF C++ Shell                                              (W/M)              │
│ ─────────────────────────────────────────                                     │
│  EXISTING:                                                                    │
│    HttpRequestInterceptor (route table + AsyncWalletResourceHandler)          │
│    SessionManager (per-tab spend cap + rate limit)                            │
│    Right-click Manage Site Permissions (MENU_ID_MANAGE_PERMISSIONS) — kept    │
│                                                                               │
│  ╔═══ Phase 1.5: Permission Engine ════════════════════════════════════╗      │
│  ║ For each BRC-100 call from origin:                                  ║      │
│  ║   1. Fetch domain row + sub-permission rows (cache → SQLite)        ║      │
│  ║   2. Classify (privacy perimeter? bundle-resolved? new scope?)      ║      │
│  ║   3. Check counters in SessionManager                               ║      │
│  ║   4. Decide SILENT / PROMPT(kind) / DENY                            ║      │
│  ║                                                                     ║      │
│  ║ On first connect:                                                   ║      │
│  ║   1. Fetch <origin>/.well-known/wallet-manifest.json                ║      │
│  ║   2. Render bundled connect prompt                                  ║      │
│  ║   3. On accept: write all bundle perms via /wallet/permissions/save ║      │
│  ╚═════════════════════════════════════════════════════════════════════╝      │
│                                                                               │
│  Platform parity: every NEW prompt overlay needs both Win (WS_POPUP) and      │
│  macOS (NSPanel) creation paths.                                              │
└───────────────────────────────────────┬───────────────────────────────────────┘
                                        ↓ localhost:31301
┌───────────────────────────────────────────────────────────────────────────────┐
│ Rust Wallet                                                                   │
│ ─────────────────────────────────────────                                     │
│  Existing 26 BRC-100 handlers — bodies unchanged                              │
│                                                                               │
│  ╔═══ Phase 1.5 NEW handlers ═══════════════════════════════════════════╗     │
│  ║   reveal_counterparty_key_linkage                                    ║     │
│  ║   reveal_specific_key_linkage                                        ║     │
│  ║   + crypto/key_linkage.rs                                            ║     │
│  ╚══════════════════════════════════════════════════════════════════════╝     │
│                                                                               │
│  ╔═══ Phase 1.5 NEW permission gates (additive at top of all 28 methods) ═╗   │
│  ║   check_protocol_approved(origin, protocolID, keyID, counterparty)    ║   │
│  ║   check_basket_approved(origin, basket, access)                       ║   │
│  ║   check_counterparty_approved(origin, counterparty)                   ║   │
│  ║   check_cert_field_approved(origin, certType, field, sensitivity)     ║   │
│  ║   (existing check_domain_approved retained — defense in depth)        ║   │
│  ╚═══════════════════════════════════════════════════════════════════════╝   │
│                                                                               │
│  ┌─ Phase 1.5 NEW DB tables ⚠️ awaits user walkthrough per invariant 2 ──┐    │
│  │ domain_protocol_permissions (FK → domain_permissions, CASCADE)       │    │
│  │ domain_basket_permissions    (FK → domain_permissions, CASCADE)      │    │
│  │ domain_counterparty_permissions (FK → domain_permissions, CASCADE)   │    │
│  │ Each with expires_at column (1y default, "never" with warning)       │    │
│  │                                                                      │    │
│  │ + ALTER cert_field_permissions ADD COLUMN sensitivity TEXT           │    │
│  │   ('low' | 'medium' | 'high' | 'highest' | 'unknown')                │    │
│  └──────────────────────────────────────────────────────────────────────┘    │
│                                                                               │
│  EXISTING tables UNTOUCHED:                                                   │
│    wallets, users, addresses, outputs, transactions, certificates,            │
│    domain_permissions (we add child tables, do not modify)                    │
│                                                                               │
│  Decision: store grants LOCALLY (SQLite). On-chain mirror deferred —          │
│  research found Babbage's on-chain grants are infrastructure debt.            │
└───────────────────────────────────────────────────────────────────────────────┘
```

### Key architectural decisions

1. **Local-first grants.** No on-chain mirror in Phase 1.5. Phase 4+ can revisit when UTXO sync is more mature.
2. **Permission engine in C++.** Hot path (cache hit) sub-millisecond.
3. **Defense in depth.** Rust handlers also check gates — same pattern as existing `check_domain_approved`.
4. **Cross-platform parity** for every new overlay before merge.
5. **Manifest fetch is sync** at first-visit (acceptable; user is already waiting on connect). Refresh on subsequent visits is async.
6. **Right-click "Manage Site Permissions" preserved exactly** as-is.

---

## 7. Best practices

### For Hodos (wallet-side)

1. **Privacy perimeter is non-negotiable.** Identity-key reveal, `revealCounterpartyKeyLinkage`, `revealSpecificKeyLinkage`, sensitive cert fields → always prompt. No setting overrides this.
2. **Three settings, not three tiers.** Default Limits + Always notify + Allow without limits cover the user-tier UX.
3. **Manifest-driven is the happy path.** Fall back gracefully when missing, but make missing worse UX than present (console-warn, slower flow) so devs adopt.
4. **One bundled prompt per site, then silence.** If a permission can't fit the bundle, owe the user a great reason to interrupt.
5. **Plain language always.** Never show `protocolID [2, "message-box"]`. Show "Send encrypted messages."
6. **Compelling rationale.** Per NNGroup research, every prompt has 1 sentence explaining why. App devs supply the language; we present it.
7. **Spend preview.** For payment prompts, show balance before/after (Rabby pattern).
8. **Audit trail in UI.** "Manage Site Permissions" page (existing) lists every grant; one-click revoke. Sub-permissions show with scope + expiry.
9. **Visual indicator on every auto-approved payment.** Hodos's existing **tab payment badge animation** (green-dot on the tab triggered by `payment_success_indicator` IPC; chain at `HttpRequestInterceptor.cpp` — `AsyncHTTPClient::OnRequestComplete` + `firePaymentSuccessIpc` → `simple_render_process_handler.cpp:1051` → `useTabManager.ts:141`) fires for every successful auto-approved payment. **This must be preserved end-to-end as handlers are rewired through the permission engine**, and **must also fire for V8 shim payments** (`window.CWI` / `window.yours` / `window.panda`) so users never miss a payment going out without some signal. This is the load-bearing safeguard against a malicious site spamming auto-approved payments under the user's nose — even when no popup appears, the green dot does.
10. **Local first.** Don't depend on broken UTXO sync.

### For app developers (recommendations we publish)

1. **Ship `wallet-manifest.json`** at `<origin>/.well-known/wallet-manifest.json`. Use plain-language `purpose` strings.
2. **Don't request `identityKey: true`** unless cross-site identity is the actual feature. Derived keys are usually sufficient.
3. **Use `protocolID`** strings that describe purpose, not implementation. `[2, "private-chat"]` beats `[2, "encrypt-1"]`.
4. **Use `counterparty`** only when peer-bound is essential.
5. **Test the deny path.** Most apps fail badly when permission is denied. Show graceful "we need X to do Y" UX.
6. **Don't ask for permissions you might want later.** Drift-prompts are acceptable; oversized initial bundles are not.
7. **Provide rationale in the manifest.** Hodos shows your `purpose` string verbatim.
8. **Default to least-sensitive cert fields.** If you only need `displayName`, ask for it and not `legalName`.
9. **Respect user limits.** If they have $5/tx and you need $20, expect to be prompted. Build for it.
10. **Test on Win and Mac.** Connect flow looks different on each platform.

### For the Phase 4 demo

1. **"How sites see you"** — interactive page showing master pubkey + N site-specific derived pubkeys side by side.
2. **"Prove it without revealing it"** — `revealCounterpartyKeyLinkage` interactive flow.
3. **"Identity-key reveal — when and why"** — explicit prompt mock-up.
4. **"Auto-approve trade-offs"** — Hodos vs Brave vs MetaMask side-by-side.
5. **"Manifest demo"** — live demo dApp with `wallet-manifest.json`.
6. **"Dev best practices"** — embedded in `LLM_DEV_GUIDE.md` (Phase 4 deliverable).

---

## 8. Notes for future work

Captured during this design phase, deferred to later sprints:

| Item | Why deferred | When |
|---|---|---|
| **BRC submission for `wallet-manifest.json`** | Wait for ecosystem appetite; ship Hodos's version first to validate | After Phase 4 demos |
| **BRC submission for cert-field `sensitivityHints`** | Same — Hodos defines its own classifier first | After Phase 4 demos |
| **Action registry** (translate protocolIDs → plain verbs in connect prompt) | User wants research first: BSVA standard? de-facto? long-term adoption likely? | Right after Phase 1.5 |
| **Audit log of permission decisions** | Fits a future browser-history-and-permissions sprint | TBD — user has a candidate sprint in mind |
| **On-chain permission token mirroring** | Babbage's reference is infrastructure debt; UTXO sync isn't robust | Phase 4+ |
| **`bsv:announceProvider` BRC submission** | Phase 2 ships the implementation; standardize after | Phase 2+ |

These are tracked in the sprint README's "Notes for future work" section so they survive sprint-doc archival.

---

## 9. References

### Research synthesized in this doc

- **Hodos current-state inventory** (2026-05-06 Explore agent run)
- **Web3 wallet UX research** (2026-05-06 researcher agent run) — MetaMask, Phantom, Brave, Coinbase, Rabby, Rainbow, WalletConnect, ERC-7715, permission fatigue, demographics
- **BRC-100 design intent** (2026-05-06 researcher agent run) — Babbage rationale, Yours `brc100-remote`, MetaNet Client (archived), example apps
- **ERC-7715 + cert sensitivity** (2026-05-06 researcher agent run) — confirmed not adoptable directly; cert sensitivity not standardized

### Primary sources

- [BRC-73 Group Permissions](https://bsv.brc.dev/wallet/0073)
- [BRC-100 Wallet Interface](https://github.com/bitcoin-sv/BRCs/blob/master/wallet/0100.md)
- [BRC-52 Identity Certificates](https://bsv.brc.dev/peer-to-peer/0052)
- [BRC-53 Certificate Creation and Revelation](https://github.com/bitcoin-sv/BRCs/blob/master/wallet/0053.md)
- [BRC-72 Key Linkage Encryption](https://github.com/bitcoin-sv/BRCs/blob/master/key-derivation/0072.md)
- [`@bsv/wallet-toolbox` `WalletPermissionsManager`](https://github.com/bsv-blockchain/wallet-toolbox/blob/master/src/WalletPermissionsManager.ts)
- [ERC-7715 `wallet_grantPermissions`](https://eips.ethereum.org/EIPS/eip-7715)
- [Coinbase Spend Permissions](https://docs.cdp.coinbase.com/server-wallets/v2/evm-features/spend-permissions)
- [WalletConnect v2 Namespaces](https://specs.walletconnect.com/2.0/specs/clients/sign/namespaces)
- [NNGroup permission UX research](https://www.nngroup.com/articles/permission-requests/)

### Internal references

- `../README.md` — sprint overview
- `../YOURS_CWI_MIGRATION.md` — BRC-100 method list
- `../AUTO_APPROVE_RATIONALE.md` — Hodos's existing 3-layer model
- `../BRAVE_WALLET_REFERENCE.md` — V8 / property descriptor patterns for Phase 2
- `../phase-0.1-brc100-audit/AUDIT_RESULTS.md` — what's implemented vs missing
- `../phase-0.2-window-yours-shim-design/SHIM_TRANSLATION_SPEC.md` — legacy translation rules
- `../ARCHITECTURE.md` — current and post-sprint diagrams
- Existing Hodos infrastructure docs:
  - `frontend/src/components/wallet/CLAUDE.md` — wallet UI tabs
  - `rust-wallet/src/database/CLAUDE.md` — DB schema
  - `cef-native/src/handlers/CLAUDE.md` — IPC + overlay creation
