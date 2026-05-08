# Phase 0 Fact-Check Results

**Date:** 2026-05-05. Source: research agent with live source-code/web verification.

Faithful capture of the fact-check report. **Three of four findings significantly shifted the sprint's strategic posture** — see `README.md` for the revised plan.

---

## Q1. Yours Wallet's BRC-100 migration — CONFIRMED, ACTIVELY UNDER WAY

**Headline:** Yours has not just announced BRC-100 — they have a near-production branch (`brc100-remote`) with active commits as recent as **2 May 2026**. The work is essentially done, awaiting cutover.

**Evidence:**
- **Public confirmation by Babbage (28 Oct 2025):** *"compatible with all wallets like Metanet Explorer, Metanet Client (Desktop), BSV Desktop, and now also Yours Wallet thanks to @kurtwuckertjr"* ([x.com/ProjectBabbage/status/1982842865236799884](https://x.com/ProjectBabbage/status/1982842865236799884))
- **PR #292 "BRC-100"** opened 2025-12-09, draft, branch `brc-100`, +4168/-112 lines, 32 files, last touched 2025-12-11. This is the original push but has stalled — superseded.
- **Active branch `brc100-remote`** — current target. Recent commits include:
  - `2026-05-02`: bumps `@1sat/wallet-browser 0.0.54`, `@bopen-io/wallet-toolbox-mobile@2.1.21-parity-fix.2` (the BRC-100 toolbox)
  - `2026-04-24`: "security: comprehensive wallet security hardening" — production-grade polish
  - `2026-04-23`: "feat: multi-account master backup, legacy import support" — handles "v1, v2, and **legacy (pre-BRC-100)** backup formats"
  - `2026-02-06`: "feat: upgrade to wallet-toolbox v2, add grouped and counterparty permission flows"
- **Branch `1sat-wallet`** — even further back, commit `2025-12-28`: *"implement BRC-100 CWI (Chrome Wallet Interface) — Add window.CWI with full BRC-100 WalletInterface methods: listOutputs, listActions, getPublicKey... createSignature, encrypt, decrypt, createAction"*.
- **`package.json` on `brc100-remote`** confirms: `@bsv/sdk ^2.0.13`, `@bsv/wallet-toolbox-mobile` (BRC-100), `@1sat/*` SDK suite, `bitcoin-backup`, `sigma-protocol 0.1.9`.
- **yours.org website** itself now markets the wallet as BRC-100: *"CWI → background service worker (BRC-100 wallet interface)"*, *"@bsv/wallet-toolbox-mobile"*, *"anyone can run a BRC-100 storage server"* ([yours.org](https://yours.org/)).

**Architecture decision (additive vs replacement):** The new architecture exposes **`window.CWI`** (BRC-100 WalletInterface) as the new surface. Commit `3d3e498` (2025-12-28) is *"remove getOrdinals API — Breaking change: clients should use CWI.listOutputs({ basket: '1sat' }) directly"* — that's a **breaking deprecation** of a `window.yours` method. Net read: **`window.yours` is being deprecated in favor of `window.CWI`, not aliased.** Legacy backup import is preserved, but the JS API is replaced.

**Timeline:** No public ETA, but May 2026 commit cadence (bug-fix bumps, no longer feature work) suggests imminent merge — **weeks to a few months, not years**. The published Chrome Web Store extension is still v4.5.6 (main branch, pre-BRC-100); the BRC-100 build has not shipped yet.

**Strategic implication for Hodos:** A `window.yours` shim still has near-term value (every shipped extension today, the entire `app.treechat.com` codebase, every BSV dapp targeting v4.x), but it has a **finite useful lifetime measured in months**. Investment should be sized accordingly. **The bigger lever is making sure Hodos's BRC-100 implementation matches the `window.CWI` surface Yours is converging on.**

---

## Q2. Treechat — uses the legacy "panda" name, narrow surface

**URLs:** Marketing `treechat.com` / `treechat.ai` (same content). App `app.treechat.com`.

**Wallet API:** Treechat **still calls `window.panda`** — Yours Wallet's pre-rename name (Panda → Yours; the Chrome extension still injects both). This is the old API surface, not even today's `window.yours`. Found in `app.treechat.com/assets/index-17f012af.js` (build hash `75c991bc`, 2026-04 build).

**Methods actually called on `window.panda`:**

| Method | Usage |
|---|---|
| `isReady` | extension presence check |
| `connect` | login flow |
| `isConnected` | auth check |
| `signMessage` | login signature |
| `getBalance` | wallet display |
| `getAddresses` | identity address |

Notably `sendBsv` and `getPaymail` are **explicitly stubbed out** with `Promise.reject("not implemented for PandaWallet")` — Treechat's tipping/microtransaction flow does **not** route through Yours; it uses BAP/server-side signing.

**Other wallets supported alongside:** HandCash, Twetch, Sigma, BAP, Paymail (login selector strings present). Treechat is wallet-pluralist; Yours is one of several.

**Scope implication:** A `window.yours` (and `window.panda` alias) shim covering just `{isReady, connect, isConnected, signMessage, getBalance, getAddresses}` is sufficient to log into Treechat. This is a very small surface.

---

## Q3. BSVradar — Sigma OAuth confirmed, **iframe signer, no external BAP keys**

**Product:** Public BSV app directory (~343+ apps, 16 categories), built by "Crumbs" (@shadilayvision). Also lists Google + GitHub login alongside Sigma ([coingeek.com/bsv-radar-maps-hundreds-of-apps...](https://coingeek.com/bsv-radar-maps-hundreds-of-apps-tackles-ecosystem-human-gap/)).

**Auth:** [Sigma Identity](https://sigmaidentity.com/) via the [`@sigma-auth/better-auth-plugin`](https://github.com/b-open-io/better-auth-plugin) (Better Auth framework plugin by b-open-io / Luke Rohenaz). The catalog data embedded in BSVradar's HTML even self-references the plugin: `"installCommand":"npm install @sigma-auth/better-auth-plugin"`, `"demoUrl":"https://sigmaidentity.com"`.

**OAuth flow (from sigma-auth-guide and plugin README):**

```
GET https://auth.sigmaidentity.com/oauth2/authorize
  ?client_id=YOUR_CLIENT_ID
  &redirect_uri=YOUR_CALLBACK_URL
  &response_type=code
  &code_challenge=...&code_challenge_method=S256   (PKCE mandatory)
  &state=...
```
Token endpoint: `POST /api/auth/sigma/callback` (relayed via `/api/auth/oauth2/token`). UserInfo: `/api/auth/oauth2/userinfo`. Discovery: `/.well-known/openid-configuration`.

**Client_id used by BSVradar:** Not extractable from static HTML — set per-deployment via `NEXT_PUBLIC_SIGMA_CLIENT_ID` env. Would need mitmproxy capture to confirm — but see CRITICAL section below: capture won't unlock a path around the architecture.

**Scopes:** Sigma docs list `scope` as a parameter but do not enumerate values. Likely `openid profile bap` based on returned user info.

**CRITICAL — Can Hodos bring its own BAP-anchored key? NO.**

Quoting [sigma-auth-guide](https://www.claudepluginhub.com/agents/b-open-io-sigma-auth/agents/sigma-auth-guide) directly:

> *"External BAP keys cannot be used directly. The system requires `SIGMA_MEMBER_PRIVATE_KEY` (WIF format, server-side only) — the stable member key at rootPath. This key defines the BAP identity and signs all authentication operations... External BSV wallet keys cannot substitute because BAP identity derivation is tied to the registered member key."*

The signing happens in a **hidden iframe loaded from `auth.sigmaidentity.com/signer`** with `postMessage` IPC: *"Private keys stay on the Sigma domain and are never accessible to your JavaScript context."* This is structurally the same model as Magic Link / Privy / Web3Auth — Sigma is a **custodial-style key-in-iframe service**, not a BYO-key federation.

**Strategic implication:** Hodos cannot transparently sign Sigma OAuth challenges using the user's existing Hodos identity. The user must either (a) sign in to Sigma in Hodos's webview (iframe loads, user enters Sigma credentials, key stays on `auth.sigmaidentity.com`), or (b) Hodos integrates as a Sigma client with its own client_id. There is no third path where Hodos's BAP key satisfies a third-party Sigma auth challenge. The mitmproxy capture will mostly confirm parameter values, not unlock any architectural option.

**This kills the original Phase 2B (Sigma OAuth interception) plan. It is structurally impossible, not just hard.**

---

## Q4. Zoide — alive at zoide.io, NOT on `window.yours`

**Status:** `zoide.io` — alive, returns HTTP 200 (Microsoft-IIS, PHP 8.3.4, Laravel session cookies). `zoide.com` and `zoide.app` did not respond.

**Product:** BSV NFT marketplace + minting platform for 1Sat Ordinals (showrooms, mint zone with countdowns, ranking system, secondary trading). Also active on X as [@ZoideNFT](https://x.com/zoidenft).

**Wallet integration:** **Self-contained, key-import model — does NOT use `window.yours`.** Wallet options on the connect modal:
- "Zoide (seed)" — generate native seed
- "1satordinals" — paste exported keys
- "Aym" — seed phrase or JSON file
- "RelayX" — seed phrase paste
- "WIF" — private-key import

Site declares: *"This website doesn't store your private keys. They will be encrypted and stored at your browser."* This is **AES-in-localStorage** style, not a JS provider API. Zoide does not appear in the `window.yours`-shim coverage list — a Hodos shim provides zero leverage here. (If Hodos wanted Zoide support, it would be a separate "wallet-import-from-Hodos" flow, not protocol shimming.)

---

## TL;DR Decision Inputs

| Q | Verdict |
|---|---|
| Yours BRC-100 migration | **Imminent** (weeks-to-months). New surface is `window.CWI`, not aliased. `window.yours` will be deprecated, not preserved. |
| Treechat surface | Tiny: 6 methods on `window.panda` (the legacy name). `sendBsv` already stubbed off. |
| BSVradar / Sigma | OAuth via `auth.sigmaidentity.com/oauth2/authorize` + PKCE + iframe signer. **Cannot accept Hodos's own BAP key** — key is bound to Sigma's server-side member WIF. |
| Zoide | Alive at zoide.io, but uses key-import (no JS provider API). Not relevant to a `window.yours` shim. |

**Sprint takeaway:** A `window.yours`/`window.panda` shim covering ~6 read-mostly methods unlocks Treechat for the next ~2-6 months until Yours ships `window.CWI`. Building it is cheap; building anything more elaborate has a short lifespan. **The bigger lever is shipping a `window.CWI` (BRC-100) implementation, since that's the surface Yours is converging on, and which Babbage-aligned apps (Metanet App Catalog) already require.** For BSVradar specifically, mitmproxy will confirm OAuth parameters but cannot reveal a path around the iframe-signer constraint — Hodos's BAP key is structurally locked out of Sigma auth.
