# Brave Wallet Architecture — Reference for Hodos `window.CWI` Injection

**Date:** 2026-05-05. Source: research agent reading `brave/brave-core` source + Brave Wallet docs.

Faithful capture of the research report. Brave's `window.ethereum` implementation is the closest analog to what we're building, and several patterns are directly transferable.

---

## 1. Architecture

**Where Brave injects `window.ethereum`:** In the **renderer process**, via a `RenderFrameObserver` subclass called `JSEthereumProvider` at `components/brave_wallet/renderer/js_ethereum_provider.cc`. One instance per `RenderFrame`. Injection happens late in the frame lifecycle (around `DidFinishLoad` / `DidDispatchDOMContentLoadedEvent`), with companion bundled JS executed afterward (`AnnounceProvider` for EIP-6963). For Solana, the parallel class is `JSSolanaProvider`.

**Process model:** Strict split.
- **Renderer process:** holds only the V8-wrapped `JSEthereumProvider` shim. **Zero key material.** Thin marshaller that converts JS calls into Mojo IPC.
- **Browser (UI) process:** runs `KeyringService`, `EthTxController`, `BraveWalletService`, `EthJsonRpcController`. Private keys, the mnemonic, signing, and approval state all live here.

**IPC pattern:** Mojo (Chromium's typed IPC). Renderer obtains the interface via `BrowserInterfaceBroker::GetInterface()` and holds a `Remote<mojom::EthereumProvider>`. All calls (`request`, `send`, `sendAsync`, `enable`, `isLocked`, `getChainId`) go over this interface. Renderer also implements `mojom::EventsListener` for the browser process to push `accountsChanged` / `chainChanged` / `connect` / `disconnect` events back.

**Translation to Hodos:** Brave's renderer-only-shim model maps almost 1:1 to our design. CEF V8 binding plays the role of `JSEthereumProvider`; Rust HTTP API on `localhost:31301` plays the role of the Mojo `EthereumProvider` interface. The big difference: Brave gets typed Mojo for free; we're doing JSON-over-loopback-HTTP, so we need rigorous request schemas and an explicit "events" channel (Brave's `EventsListener` is push; HTTP is pull, so consider SSE / WebSocket / piggyback on existing CEF→Rust IPC).

---

## 2. Security

**Key storage:** Mnemonic seed encrypted with **AES-GCM** using a key derived from the user's password (`PasswordEncryptor`) and persisted in **Chromium `PrefService`** (JSON-on-disk Preferences). Derivation follows BIP-32 / BIP-39 / BIP-43 / BIP-44 over secp256k1 via libsecp256k1. No separate keychain integration on desktop by default.

**Hardware wallets:** `KeyringService` integrates Ledger and Trezor directly; signing requests routed to the device through vendor SDKs invoked from the browser process. Renderer never sees the interaction.

**JS ↔ keystore bridge specifics:** Brave wraps the injected provider in a **V8 Proxy with `apply` traps on each method** so that even if a page does `const r = ethereum.request; r({...})`, the call still binds correctly and avoids "Illegal invocation" errors. Properties marked **non-writable / non-configurable**, except `isMetaMask` (left writable to fix compatibility regressions — issue #21949).

**Translation to Hodos:** Our model is actually stronger here — keys live in a *separate OS process* with its own address space, not just a separate Chromium thread. **Worth copying from Brave: the Proxy-with-apply-trap defensive wrapper, and the non-writable property descriptors.**

---

## 3. Permission / approval model — IMPORTANT

**Per-site connection (EIP-2255-style):** First call to `eth_requestAccounts` triggers a prompt asking which accounts (if any) to expose. Grant persisted in **Chromium's content settings** at `brave://settings/content/ethereum` — same storage mechanism as camera/microphone/geolocation permissions.

**Per-call confirmation:** Every `eth_sign`, `personal_sign`, `eth_sendTransaction` triggers a separate confirmation in the wallet panel. **There is no auto-approve, no per-session threshold, no small-value pre-authorization.** Issue #27592 was explicitly closed as not-planned. Brave's stance: "every signature gets a prompt."

**Connected-sites UI:** Global view at `brave://settings/content/ethereum`, plus a per-site disconnect from the wallet panel popup while on the site.

**Translation to Hodos:** Hodos's **3-layer auto-approve** model (domain whitelist + session spend cap + per-call) is *more aggressive* than Brave's. Brave deliberately does not auto-approve signing. **This is a real product decision Hodos has to defend explicitly — Brave closed the auto-approve feature request after years of phishing post-mortems.** If we keep auto-approve, we should:
- Default-tighten the thresholds (low spend caps)
- Default-narrow the whitelist (no implicit additions)
- Show a prominent notification when auto-approve fires (so users know it happened)
- Document why our model differs

Worth copying: piggybacking on a content-settings-style storage rather than a parallel "domain permissions" table (we already have `domain_permission_repo` — could expose via a `hodos://settings/content/...` route to match user expectations).

---

## 4. Phishing / safety

- **Origin display:** Every signing prompt shows the **eTLD+1 + favicon** of the requesting origin. Cheapest, highest-impact anti-phishing measure.
- **Calldata decoding:** Brave built a **native ABI decoder in brave-core** (per the "safer-signing" blog) for 0x, Uniswap, Curve, PancakeSwap, Sushiswap. They explicitly avoided third-party services for the decode step. Goal: kill blind signing.
- **Transaction simulation / blocklists:** Brave integrates **Blowfish** (not Blockaid) for "is this domain/contract dangerous?" lookups and ML-based transaction risk analysis. Issue #45872 tracks expanding this.
- **No injection in private/Tor windows** at all.

**Translation to Hodos:** BSV transactions are simpler than EVM calldata, so we don't need the ABI-decoder layer. But we **should** display origin + favicon on every signing/spending prompt, and consider a domain-reputation lookup before first connect (could piggyback on existing adblock filter lists for known-malicious domains). Currently nothing equivalent to Blowfish.

---

## 5. Provider conflict (MetaMask coexistence)

Brave's **default-wallet setting** at `brave://settings/wallet` has four modes:
1. *Extensions (Brave Wallet fallback)* — **default**. Brave defines `window.ethereum` only if no extension claims it first.
2. *Brave Wallet* — Brave wins, sets `window.ethereum` non-writable.
3. *Extensions (no fallback)* — Brave does not inject at all.
4. *Crypto Wallets (Deprecated)* — legacy.

Brave **always** injects `window.braveEthereum` regardless of the setting, so dApps can target Brave specifically.

**EIP-6963:** Yes, Brave implements it. Both Brave and MetaMask `announceProvider` simultaneously, dApps pick via the EIP-6963 event flow. This is the recommended modern path and removes the `window.ethereum` race.

**Translation to Hodos:**
- Inject `window.CWI` as canonical, non-writable.
- Inject `window.yours` and `window.panda` as **aliases pointing to the same proxy object** for transitional compat — but mark them *writable* so pages or competing wallets can override.
- **Strongly consider a BSV-equivalent of EIP-6963.** Even just a `metanet:announceProvider` CustomEvent contract. If one doesn't exist yet in BSV, propose one. It's the cleanest long-term answer for multi-wallet coexistence.
- Add a setting equivalent to `brave://settings/wallet` so users with another BSV wallet extension installed can opt out.

---

## 6. Privacy / fingerprinting

- **No injection in Private or Tor windows.** Period.
- **Iframe rules:** `window.ethereum` is undefined in third-party iframes unless the iframe has `allow="ethereum"` (Permissions Policy). Same for `solana`. Sandboxed iframes need `allow-same-origin`. iOS blocks all iframe access.
- **Secure-context-only:** HTTPS or localhost only.
- **Fingerprinting acknowledgment:** Brave docs don't address whether the *presence* of `window.ethereum` itself is a fingerprinting bit. It clearly is — sites can detect "Brave with wallet on" vs "Brave with wallet off" — but Brave appears to consider this acceptable given user opt-in.

**Translation to Hodos:** Adopt all of these defaults verbatim. Given Hodos's privacy posture, also consider:
- **Hide the provider until first user gesture** (don't define `window.CWI` until user clicks the wallet icon for that site, or until the page calls a non-injected discovery method). Stricter than Brave; matches our fingerprint-farbling brand.
- Use Permissions Policy (`allow="bsv-wallet"`) for iframes.
- Per-tab provider state is the natural CEF default since each tab is its own browser host; ensure we're not sharing connect-state across tabs of the same eTLD+1 without intent.

---

## 7. Aliasing patterns

Brave injects (current state):
- `window.ethereum` (conditional, EIP-1193)
- `window.braveEthereum` (always when wallet is on, the canonical Brave handle)
- `window.solana` (alias)
- `window.braveSolana` (canonical Solana)
- EIP-6963 announcements for both

Each chain gets its **own provider class** (`JSEthereumProvider`, `JSSolanaProvider`) and its own Mojo interface. They share `KeyringService` in the browser process but are otherwise independent. **No multiplexing.**

**Translation to Hodos:** Mirror the pattern. `window.CWI` is our `braveEthereum` (canonical), `window.yours`/`window.panda` are our aliases. If we ever add a second protocol (e.g. Lightning surface), make it its own provider object — don't multiplex.

---

## Files in brave-core worth reading directly

| File | Why |
|---|---|
| `components/brave_wallet/renderer/js_ethereum_provider.{h,cc}` | Injection itself, V8 Proxy wrapping, EIP-6963 announce |
| `components/brave_wallet/renderer/js_solana_provider.{h,cc}` | Second-chain pattern |
| `components/brave_wallet/browser/keyring_service.{h,cc}` | Key management, password derivation, hardware wallet hooks |
| `components/brave_wallet/browser/eth_tx_manager.{h,cc}` | Transaction lifecycle |
| `components/brave_wallet/browser/brave_wallet_service.{h,cc}` | Permissions, defaults, orchestration |
| `components/brave_wallet/common/brave_wallet.mojom` | The IPC contract — most useful single file for shaping our Rust HTTP API |
| `components/permissions/contexts/` (Brave's additions) | Ethereum/Solana permissions plugged into Chromium content-settings |

---

## What does NOT translate to Hodos

- **Mojo typed IPC** — we're on JSON/HTTP. Equivalent investment: rigorous request schemas + push-events channel.
- **Chromium `PrefService` for storage** — we have SQLite-via-Rust, strictly better; just keep AES-GCM with user-derived key.
- **`brave://settings/content/...` reuse of Chromium content settings** — CEF doesn't expose this. We roll our own settings UI (`SettingsManager`).
- **Auto-approve absence** — Brave deliberately doesn't have it; our 3-layer model is a Hodos-specific differentiator. Treat as UX risk surface, not feature to copy from Brave.

---

## Patterns to adopt (priority list)

1. **V8 Proxy wrapper with `apply` traps** — defends against `const r = CWI.request; r(...)` patterns.
2. **Non-writable, non-configurable property descriptors** for the canonical handle (`window.CWI`).
3. **Origin + favicon on every prompt** — cheapest anti-phishing win.
4. **Iframe Permissions Policy gating** (`allow="bsv-wallet"`) and secure-context-only injection.
5. **No injection in private/incognito tabs.**
6. **EIP-6963-equivalent announce protocol for BSV** — propose one if it doesn't exist.
7. **Per-site permission storage** mirroring browser content-settings semantics.
8. **A "Default wallet" setting** for users running other BSV wallet extensions.
9. **Hide-until-gesture** for the strongest privacy posture (stricter than Brave).
10. **Defensive auto-approve defaults** — low spend cap, narrow whitelist, prominent notification when fired.

---

## Sources

- [JSEthereumProvider source](https://github.com/brave/brave-core/blob/master/components/brave_wallet/renderer/js_ethereum_provider.cc)
- [Brave Wallet developer information wiki](https://github.com/brave/brave-browser/wiki/Brave-Wallet-developer-information)
- [Ethereum Provider API wiki](https://github.com/brave/brave-browser/wiki/Ethereum-Provider-API)
- [KeyringService header](https://github.com/brave/brave-core/blob/master/components/brave_wallet/browser/keyring_service.h)
- [Brave Wallet docs — Connecting your site](https://wallet-docs.brave.com/ethereum/use-cases/connecting-your-site/)
- [Brave Wallet docs — Default Wallet setting](https://wallet-docs.brave.com/default-wallet/)
- [Brave Wallet docs — Provider availability / iframe rules](https://wallet-docs.brave.com/provider-availability/)
- [Brave Wallet docs — Solana provider](https://wallet-docs.brave.com/solana/)
- [Introducing EIP-6963 support in Brave Wallet (blog)](https://brave.com/blog/eip-6963/)
- [Towards a safer signing experience on Brave Wallet (blog)](https://brave.com/blog/safer-signing/)
- [Anti-phishing & transaction simulation issue #45872](https://github.com/brave/brave-browser/issues/45872)
- [Auto-approve transactions feature request — closed not-planned, issue #27592](https://github.com/brave/brave-browser/issues/27592)
- [`isBraveWallet` property — issue #21949](https://github.com/brave/brave-browser/issues/21949)
- [EIP-6963 specification](https://eips.ethereum.org/EIPS/eip-6963)
- [EIP-2255: Wallet Permissions System](https://eips.ethereum.org/EIPS/eip-2255)
