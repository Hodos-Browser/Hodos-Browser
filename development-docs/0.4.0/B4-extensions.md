# B4 — Extensions / Plugins (security-focused)

**Status:** 🛑 RE-SCOPE REQUIRED — A4 found CWS extensions infeasible on CEF.
**Type:** feature (large) · **0.4.0:** candidate (at risk)

## A4 finding (see `../DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md`)
**Real Chrome Web Store extensions — including MetaMask — are NOT feasible on our CEF stack.**
CEF's extension API is effectively dead (Alloy removed ~M127; Chrome-runtime-only; ~4 of 70+
`chrome.*` APIs; no MV3 service workers, which MetaMask requires). Vivaldi/Brave/Opera support
extensions only because they build the full Chromium **chrome layer** CEF lacks.

**Re-scope directions (need a dedicated session — decide JOINTLY with B2):**
1. Curated first-party wallet integration (we integrate specific providers, not host their extensions).
2. Adopt CEF **Chrome runtime** — partial extension support BUT forces Chrome's toolbar UI → collides
   with a custom native header (B2). This is why B4 and B2 are coupled.
3. Defer extensions / reconsider stack (full-Chromium shell = Brave-class cost, already rejected).

**Wallet deconfliction (if extensions ever exist):** EIP-6963 `rdns` + `isMetaMask` flag for
detection; CRX extension-ID allow/block list for enforcement. Today: defensively lock `window.yours`
against override in our V8 injection.

## Summary
Add support for browser extensions/plugins, focused on **security** and good UX. Likely
Chromium-approved (Chrome Web Store) extensions. Must **deconflict wallets**.

## Wallet deconfliction (product requirement)
- **Block** extensions that are conflicting **BSV wallets** (would collide with our native wallet).
- **Allow** non-BSV wallets — Ethereum wallets like **MetaMask**, etc.
- Open: how to detect/classify a "conflicting BSV wallet" extension reliably.

## ⚠️ Untrusted background docs
Prior research lives in `browser-extensions/` (relocated into this sprint). **It cannot be trusted —
verify every claim against primary source** before using any of it. Files: `README.md`,
`BROWSER_PLUGINS_DEEP_DIVE.md`, `IMPLEMENTATION_OUTLINE.md`, `EXTENSION_SECURITY.md`,
`CRYPTO_WALLET_EXTENSIONS.md`.

## Open questions / research needed
- **CEF reality:** what extension support does CEF 136 actually provide vs full Chromium? (This is
  partly why A4/Brave matters — Brave/Vivaldi ship real extension support.)
- How do other browsers sandbox/permission extensions (security model, manifest v3, permissions UI)?
- Distribution: Chrome Web Store vs curated allowlist vs both.
- The wallet-deconfliction detection mechanism.

## Dependencies
A4 (build surface affects what extension support is even feasible).

## To fill after research
Acceptance criteria · Reuse map · Risk table · Implementation order · security model · Test plan · What this does NOT do.
