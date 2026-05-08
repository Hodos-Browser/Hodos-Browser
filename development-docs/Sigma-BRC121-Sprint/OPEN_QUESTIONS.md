# Open Questions

## Sigma open questions (gate Phase 2B interception design)

### OQ#1 — Sigma server key acceptance
Will sigmaidentity.com accept a BRC-77 or BSM signature from a key it didn't generate? Or does it require a key Sigma issued in the browser?

**Status: ANSWERED — NO. Hodos's keys are structurally locked out.** (per fact-check 2026-05-05, see `phase-0-research/FACT_CHECK_RESULTS.md` Q3)

The earlier reading of "Sigma Auth acts as a consumer of BAP data rather than a gatekeeper" was about reading BAP profile data to enrich `/userinfo` responses — **not** about which keys can sign auth challenges. Those are different.

**Reality:** Sigma uses an **iframe signer** at `auth.sigmaidentity.com/signer`. Private keys live on Sigma's domain, accessed only via `postMessage` IPC, never accessible to client JS. The verifier requires `SIGMA_MEMBER_PRIVATE_KEY` (WIF, server-side), and per Sigma's own docs:

> "External BAP keys cannot be used directly... External BSV wallet keys cannot substitute because BAP identity derivation is tied to the registered member key."

This is the same architecture as Magic Link / Privy / Web3Auth — **custodial-style key-in-iframe**, not BYO-key federation. **No mitmproxy capture will reveal a path around this.**

**Implication: Phase 2B (Sigma OAuth interception) is cancelled.** Not deferred — structurally impossible.

---

### OQ#2 — Sigma client lib location
Where does the Sigma client lib live — stable `window.*` global, or inline-bundled per-app?

**Status: ANSWERED.** No `window.sigma` global exists. Sigma is a server-side OAuth provider — apps redirect to `auth.sigmaidentity.com` where the signing happens in-page using the iframe-signer model. There is no app-injected Sigma object to override and no path to substitute keys.

---

### OQ#3 — BRC-121 production servers exist?
Are any production servers actually returning HTTP 402 with `x-bsv-sats` / `x-bsv-server` headers today?

**Status: ANSWERED — YES, at least one.** Confirmed 2026-05-07.

- **Site:** `now.bsvblockchain.tech` ("The NOW™ Times" — paid micro-parody news).
- **Free landing page:** `/` (HTML, 200, with `access-control-expose-headers: x-bsv-sats,x-bsv-server` declaring the BRC-121 surface).
- **402-protected paths:** `/articles/<slug>` (e.g. `/articles/runar-playground` → 75 sats, `/articles/chronicle-activates` → 100 sats, `/articles/agentpay-hackathon` → 150 sats).
- **Verified response shape on `/articles/runar-playground`:**
  ```
  HTTP/1.1 402 Payment Required
  x-bsv-sats: 75
  x-bsv-server: 0373ce63481ace3634e235af7a73742444b2d6abd8742b182ad595a84028d00c00
  Content-Length: 0
  ```
- 33-byte compressed pubkey shape matches our `pay_402` validation. Our demo server (`demos/brc121-402/`) shape is identical.

This is the **canonical real-world Phase 1 acceptance target** alongside the localhost demo. Phase 4 LLM-ready dev guides should reference this site as a reproducible example.

---

## Phase 0.5 — Yours/ordinal ecosystem fact-checks (COMPLETED 2026-05-05)

Full report: `phase-0-research/FACT_CHECK_RESULTS.md`. Summary below.

### Q4 — Yours Wallet's BRC-100 migration plan
**Status: ANSWERED — IMMINENT.**

- Active branch `brc100-remote` with commits as recent as 2026-05-02 (bug-fix bumps, no longer feature work)
- Babbage publicly confirmed compatibility partnership Oct 2025
- New API surface: **`window.CWI`** (BRC-100 WalletInterface) with methods `listOutputs`, `listActions`, `getPublicKey`, `createSignature`, `encrypt`, `decrypt`, `createAction`
- **Breaking deprecation:** `getOrdinals` removed (commit `3d3e498`, 2025-12-28); replaced by `CWI.listOutputs({ basket: '1sat' })`
- Net: `window.yours` is being **replaced**, not aliased. Useful lifetime measured in months.
- Chrome Web Store still on v4.5.6 (pre-BRC-100); BRC-100 build not yet shipped publicly.

**Strategic implication:** Don't invest heavily in `window.yours` shim. The bigger lever is making sure Hodos's BRC-100 implementation matches Yours's `window.CWI` surface (basket conventions, method signatures).

### Q5 — Treechat's wallet integration
**Status: ANSWERED — TINY SURFACE.**

- App URL: `app.treechat.com` (marketing at `treechat.com` / `treechat.ai`)
- Calls **`window.panda`** (legacy name, not `window.yours` — Yours's Chrome extension still injects both)
- Methods actually used: `isReady`, `connect`, `isConnected`, `signMessage`, `getBalance`, `getAddresses` — that's it
- `sendBsv` and `getPaymail` are stubbed out by Treechat itself (tipping uses BAP/server-side signing)
- Wallet-pluralist: also supports HandCash, Twetch, Sigma, BAP, Paymail

**Strategic implication:** Tiny shim of 6 read-mostly `window.panda`/`window.yours` methods unlocks Treechat. Cheap, but short useful life.

### Q6 — BSVradar's Sigma flow
**Status: ANSWERED — STRUCTURALLY BLOCKED.** See OQ#1 above.

- Confirmed using `@sigma-auth/better-auth-plugin` (Better Auth framework plugin by b-open-io)
- Standard OAuth flow: `auth.sigmaidentity.com/oauth2/authorize` + PKCE + iframe signer
- Cannot substitute Hodos's BAP key (iframe signer architecture)

### Q7 — Zoide
**Status: ANSWERED — ALIVE BUT IRRELEVANT.**

- Alive at `zoide.io` (not `zoide.com` — that DNS no longer resolves)
- BSV NFT marketplace + 1Sat Ordinals minting platform
- Self-contained key-import (seed phrase / WIF / Aym / RelayX), AES-in-localStorage
- **Does NOT use `window.yours` or any JS provider API** — irrelevant to provider-shimming work

---

## Scope questions (open)

- **Q1** — Does this sprint include 1Sat Ordinals payment support (full ordinal stack), or is Phase 3 truly deferred?
- **Q2** — Does this sprint include MNEE stablecoin support?
- **Q3** — Demo repo location: `Hodos-Browser/demos/` subdirectory vs separate `hodos-demos/` repo?
- **Q8** — Build a `window.panda`/`window.yours` shim for transitional compat? **ANSWERED — yes, but with translation logic, not aliases.** Yours has *removed* `window.yours` entirely on `brc100-remote` (verified — see `YOURS_CWI_MIGRATION.md`). Sites using legacy methods will break on Yours v5+. Hodos's `window.yours` shim becomes a real translation layer (legacy method → BRC-100 backend), not a thin alias. Per-method design decisions required: `signMessage` needs invented protocolID/keyID convention; `getAddresses` has no clean equivalent (only identity-key fallback); `encrypt`/`decrypt` semantics differ. Bounded value (~months until apps migrate).
- **Q9** — Verify Hodos's BRC-100 surface matches `window.CWI`. **ANSWERED — exact 28 methods identified.** See `YOURS_CWI_MIGRATION.md` §2. The canonical `@bsv/sdk@2.0.13` `WalletInterface`: identity (3) + crypto (6) + transactions (5) + outputs (2) + certificates (6) + auth (2) + chain info (4) = 28. Hodos audit needed against this list (Q16 below).
- **Q10** — Future M2M / agent-payments sprint scoped after this one ships, or treat as out-of-scope until partner demand?
- **Q11 (NEW)** — Confirm Hodos's existing 3-layer domain permission model (per-request approval overlay → auto-approve below threshold → hard whitelist) is wired through to **all** `window.CWI` / `window.yours` / `window.panda` calls. Each method routes through the V8 shim → IPC to Rust → Rust handler must invoke `check_domain_approved()` and (where spending) `SessionManager::Get/UpdateSpending()`. Same gate as our existing BRC-100 path; no new approval architecture, just routing the new IPCs through it. **Verify implementation lands all paths through this gate** — don't accidentally skip it for ordinal calls.
- **Q12 (NEW)** — Sigma OAuth (as a normal OAuth provider, like Google login) works in Hodos with **zero special code** — page redirects to `auth.sigmaidentity.com`, user signs in there, gets redirected back. Confirm this in the Phase 4 demo: alongside "Sign in with BSV Wallet (BRC-100)" and "Sign in with HandCash," show "Sign in with Sigma" working as a normal OAuth flow. The cancelled Phase 2B was about substituting Hodos's wallet identity *for* Sigma's iframe key — that's blocked. Normal Sigma OAuth use is fine.
- **Q13** — Brave Browser security/permission patterns for `window.ethereum`. **ANSWERED 2026-05-05** — see `BRAVE_WALLET_REFERENCE.md` at sprint root. Top transferable patterns: V8 Proxy with apply traps, non-writable property descriptors, origin+favicon on every prompt, no-injection-in-private-mode, iframe Permissions Policy gating, secure-context-only, EIP-6963-style multi-provider announce (propose BSV equivalent), "default wallet" setting. **Real tension surfaced:** Brave deliberately does NOT auto-approve any signing — closed feature request #27592 as not-planned. Hodos's 3-layer auto-approve model is more aggressive than industry norm; needs defensive defaults (low caps, narrow whitelist, prominent notification on auto-approve fire) and explicit documentation of why we differ.
- **Q14 (NEW — surfaced by Brave research)** — Should we propose a BSV-ecosystem equivalent of EIP-6963 (multi-injected-provider discovery)? Currently no formal BRC for wallet-provider conflict resolution. Brave's experience shows the `window.ethereum` race was painful 2018-2023 until EIP-6963; BSV could skip the painful era by adopting the pattern preemptively.
- **Q15 (NEW)** — Auto-approve UX hardening: low default spend cap (concrete number?), narrow default whitelist (empty? user must add explicitly?), prominent notification on each auto-approve fire (toast? log entry? both?). Currently the 3-layer model exists but defaults haven't been audited against industry standards.
- **Q16 (NEW — surfaced by Yours CWI deep-dive)** — Audit Hodos's existing BRC-100 implementation against the canonical 28-method `WalletInterface` (`@bsv/sdk@2.0.13`). Method-by-method coverage: `getPublicKey`, `revealCounterpartyKeyLinkage`, `revealSpecificKeyLinkage`, `encrypt`, `decrypt`, `createHmac`, `verifyHmac`, `createSignature`, `verifySignature`, `createAction`, `signAction`, `abortAction`, `listActions`, `internalizeAction`, `listOutputs`, `relinquishOutput`, `acquireCertificate`, `listCertificates`, `proveCertificate`, `relinquishCertificate`, `discoverByIdentityKey`, `discoverByAttributes`, `isAuthenticated`, `waitForAuthentication`, `getHeight`, `getHeaderForHeight`, `getNetwork`, `getVersion`. Where do we have gaps? Argument shapes match `@bsv/sdk@2.0.13`?
- **Q17 (NEW — surfaced by Yours CWI deep-dive)** — Extend Hodos's permission model to BRC-100's three additional tiers beyond connect/disconnect: per-protocol (`PermissionRequest`), grouped (`GroupedPermissionRequest`), per-counterparty (`CounterpartyPermissionRequest`). Each opens a separate approval flow in Yours. We'd add overlay subprocesses for each. Reference: `WalletPermissionsManager` in `@bsv/wallet-toolbox-mobile`.
- **Q18 (NEW — surfaced by Yours CWI deep-dive)** — `window.yours` shim semantic translations:
  - `signMessage({ message, encoding })` → `createSignature` requires inventing a protocolID/keyID convention. Proposal: protocolID `[2, "yours-legacy-message"]`, keyID `"1"`, counterparty `"anyone"`. Document this so other shims can interop.
  - `getAddresses()` — no clean BRC-100 equivalent. Options: (a) fall back to identity-key-derived P2PKH (semantically wrong but functional for Treechat), (b) return error, (c) extend our shim with a fresh-receive-address generator that doesn't exist on canonical BRC-100. Prefer (a) for Treechat-compat narrow case.
  - `encrypt`/`decrypt` — translate `{ message, encoding, pubKeys[] }` → BRC-100 `{ plaintext, protocolID, keyID, counterparty }`. Need protocolID/keyID convention here too.
- **Q19 (NEW)** — Should Hodos handle 1Sat ordinal `createAction` requests specially (recognize basket=`'1sat'`, do the right UTXO classification + locking script handling), or just pass through generic `createAction` and trust dApp templates? Research agent noted Yours does basket-aware handling inside `processCWICreateAction`; matching this behavior is a Phase 3 ordinals decision, not Phase 2.5 shim decision.

---

## Decision matrix for Sigma after Phase 0

**OBSOLETE.** All three rows of the original matrix are superseded by OQ#1's answer: Sigma OAuth interception is structurally blocked regardless of OQ#2's answer. The only remaining options are:

| Path | Verdict |
|------|---------|
| Strategy A — V8 monkey-patch | Not applicable (no `window.sigma` exists) |
| Strategy B — HTTP interception | **Not applicable** (iframe signer keeps keys on Sigma's domain; no key to substitute) |
| **Strategy C — Sigma as upstream OAuth provider** | Cancelled. Hodos integrates with Sigma the same way any other web app does — full redirect to `auth.sigmaidentity.com`, user enters Sigma credentials, no Hodos identity coupling. This adds nothing over a normal browser. |
| **Outcome** | **Cancel Phase 2B.** Ship Phase 2A (`/signMessage` primitives) for content-signing/tipping use cases that don't depend on Sigma. |
