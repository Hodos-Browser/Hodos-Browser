# Manifest connect-bundle test fixture

Exercises the `manifest_connect_bundle` prompt that Rust's `domain_trust_gate` emits for an
**unknown** origin that publishes a wallet manifest.

**Updated beta.3 Phase 0.8 (2026-08-22)** — now ships **two** manifests, one per shape, because
they demo different halves of the prompt. See "Which manifest demos what" below.

## ⛔ Why this cannot be tested from `http://localhost`

Three independent gates block it, and only the third is about the manifest:

1. **The dApp shim is never injected into a loopback page.**
   `simple_render_process_handler.cpp :: OnContextCreated` computes
   `isExternalPage = !IsLoopbackUrl(url) && !IsInternalFrontendUrl(url)`. A page on
   `localhost` / `127.0.0.1` / `[::1]` is loopback, so it is **not** external and gets **no
   `window.CWI`**. Deliberate — a local dev server is not a dApp.
2. **The shim is https-only even for external pages.**
   Same function: `else if (url.find("https://") != 0) → "shim skipped (insecure context)"`.
3. **The manifest fetch resolves a bare authority to `https://`.**
   `manifest.rs :: manifest_origin_base`. `X-Requesting-Domain` carries a bare `host[:port]`
   (no scheme, by `hodos::OriginFromUrl`), so `localhost:3000` is fetched as
   `https://localhost:3000/manifest.json`.

⚠️ Gate 3 alone is spec-adjustable — BRC-73 says *"For local development, wallets MAY use
`http://localhost/.../manifest.json`"* — but relaxing it achieves nothing while gates 1 and 2
stand, and those two are **security gates hardened in Phase 0.5** (P0.5-G3/C1). Do not loosen
them to make a demo work.

**So: you need a real HTTPS origin.** Options below, cheapest first.

## Getting an HTTPS origin

### Option A — a quick tunnel (fastest, no account for the first two)

From this folder, serve it and expose it:

```bash
# terminal 1 — serve this folder on 8788
cd test-fixtures/manifest-dapp && python -m http.server 8788

# terminal 2 — pick one; each prints a public https:// URL
npx --yes localtunnel --port 8788
npx --yes cloudflared tunnel --url http://localhost:8788
ngrok http 8788                      # needs a free account
```

Open the printed `https://…` URL **in the Hodos browser**.

⚠️ `python -m http.server` will not serve `/.well-known/` on some Python builds' directory
listings, but it *does* serve the file by path — which is all the wallet needs.

### Option B — a static host

Deploy this folder's contents to the site root of GitHub Pages / Vercel / Netlify /
Cloudflare Pages, so that `https://<host>/manifest.json` and
`https://<host>/.well-known/wallet-manifest.json` both resolve.

## Which manifest demos what

⛔ **`/manifest.json` wins when both are present** — BRC-73 is canonical and the parser stops
there (BRC-116 Backwards Compatibility: *"Check for `metanet` first. If present, use it."*). So
the two are an either/or, not a both.

| Serve | Shape | What the modal shows |
|---|---|---|
| `manifest.json` (default) | **BRC-73** `metanet.groupPermissions` | 3 protocols **with counterparties** (`self`, a named pubkey), 2 baskets, 1 certificate **with its verifier and field list**, and *"This site declares a monthly allowance of 250,000 satoshis / month. Hodos does not enforce monthly limits."* |
| rename `manifest.json` away, leaving `.well-known/wallet-manifest.json` | **our legacy shape** | The same categories **plus** a site-suggested **$1/tx, $5/session**. ⭐ This is the only way to demo `R-PROV` — the *"suggested by site"* marking, the *"These are not your defaults"* prose, and **"Use my defaults"** — because BRC-73 has no field in our unit and can never pre-fill a cap. |

⭐ **bitgenius.net cannot demo the provenance marking.** It declares protocols only, no spending,
so there is nothing for a site to suggest. Use the legacy shape above for that half.

## Steps

1. Get an HTTPS origin serving this folder (above).
2. Launch the dev stack — see `development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/PHASE_CONTRACT.md`
   or the root `CLAUDE.md` Dev Runbook. Tail `%APPDATA%\HodosBrowserDev\logs\wallet_rCURRENT.log`
   and `%APPDATA%\HodosBrowserDev\logs\debug_output.log`.
3. ⛔ **Make sure the origin is UNKNOWN to the wallet.** `request_gate.rs :: domain_trust_gate`
   short-circuits on `trust == "approved"` and never fetches, so a previously-approved origin
   tests **nothing**. Revoke via right-click → **Manage Site Permissions**.
4. Open the HTTPS URL in Hodos and click **Connect**.
5. Expect the **manifest connect bundle** modal, itemised. Wallet log:
   `📦 manifest for <host> parsed from https://<host>/manifest.json (namespace=metanet, …)`
   then `engine Prompt (domain-trust) … type=ManifestConnectBundle`.
   Browser log: `📦 Triggering manifest_connect_bundle for <host> (app=…, ns=metanet, N protocols, …)`.
6. Approve → the original `getPublicKey` resolves (200) on the re-issue, no re-prompt loop.

## Note

The plain `domain_approval` path (unknown origin **without** a usable manifest) no longer needs a
separate fixture — since Phase 0.8 any origin whose manifest declares no recognised permission
takes it. `projectbabbage.com` (SPA `200 text/html`) and `socialcert.net` (a `babbage` key with no
`groupPermissions`) are both live examples.
