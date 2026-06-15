# Manifest connect-bundle test fixture (Phase 2.6-G G.4)

Exercises the `manifest_connect_bundle` prompt that Rust's `domain_trust_gate`
emits for an **unknown** origin that publishes a wallet manifest.

## Why it can't be tested purely locally

`rust-wallet/src/manifest.rs::manifest_url` fetches
`https://<dApp-origin>/.well-known/wallet-manifest.json` — **HTTPS only** for bare
hosts (manifests must not be served over plaintext). And the Rust bundle path only
fires on the **window.CWI shim (IPC) transport**, which G.4 made Rust-authoritative
(the direct-fetch `Open()` path still resolves trust in C++).

So a live test needs:
1. An **HTTPS** origin you control (GitHub Pages / Vercel / Netlify / Cloudflare
   Pages / your own HTTPS dev host).
2. That origin serving **both** `index.html` and `/.well-known/wallet-manifest.json`
   (copy this folder's contents to the site root).
3. The page calling the wallet through `window.CWI` (the shim) — `index.html` does this.
4. The origin being **unknown** to the wallet (not previously approved). Revoke it via
   the wallet's "Manage Site Permissions" if it was approved before.

## Steps

1. Deploy this folder to an HTTPS static host so that:
   - `https://<host>/` serves `index.html`
   - `https://<host>/.well-known/wallet-manifest.json` serves the manifest
2. Launch the Hodos dev stack (`.\dev-wallet.ps1`, frontend, browser). Tail
   `%APPDATA%\HodosBrowserDev\logs\wallet_rCURRENT.log`.
3. Open `https://<host>/` in the Hodos browser and click **Connect**.
4. Expect the **manifest connect bundle** modal (app name, protocols, baskets,
   certificates, spending caps), not the plain domain-approval modal.
5. Wallet log should show `engine Prompt (domain-trust) … type=ManifestConnectBundle`.
   Approve → the original `getPublicKey` resolves (200) on the re-issue, no re-prompt loop.

## Note

The plain `domain_approval` path (unknown origin **without** a manifest) is the common
case and is co-tested separately by visiting any unknown BRC-100 shim dApp — it does
not require this fixture.
