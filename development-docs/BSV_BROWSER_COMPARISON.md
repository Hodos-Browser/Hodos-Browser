# BSV Browser Comparison Analysis

> **Date**: 2026-04-07  
> **Repo**: https://github.com/bsv-blockchain/bsv-browser (v1.3.4)  
> **Compared against**: Hodos Browser

---

## Architecture Overview

**BSV Browser** is a React Native + Expo mobile app using `react-native-webview` (v13.15.0):
- **iOS**: WKWebView (WebKit)
- **Android**: Android System WebView (Chromium-based)
- **No CEF, no Electron, no Electrum** — it's a mobile WebView wrapper
- **Mobile only** — no desktop support (Windows, macOS, Linux)
- All wallet logic runs in JavaScript via `@bsv/wallet-toolbox-mobile`
- Keys live in the same JS process as the app (mnemonic stored in `expo-secure-store`)

**YouTube/complex sites**: Partially works. They wrote a ~230-line MediaSource polyfill for iOS WKWebView, spoof user agents, and patch `Function.prototype.toString` to fool anti-bot scripts. Video playback is degraded. Would fail our site compatibility tests.

---

## What's Good (Worth Learning From)

### 1. HTTP 402 Payment Handler — HIGH VALUE
Automatic 402 payment flow: detect `x-bsv-sats` and `x-bsv-server` response headers → auto-pay → cache paid HTML for 30 minutes.

**Hodos should adopt this.** We already have the wallet backend and signing infrastructure. Adding 402 detection in `HttpRequestInterceptor.cpp` and auto-payment via the wallet would be a differentiating feature. Their implementation is a good reference for the flow, though ours would be more robust (C++ network-level interception vs JS-level fetch patching).

### 2. Comprehensive BRC-100 CWI Provider
Their `window.CWI` injection covers the full BRC-100 interface (28+ methods): `createAction`, `signAction`, `abortAction`, `listActions`, `internalizeAction`, `listOutputs`, `relinquishOutput`, `getPublicKey`, `encrypt/decrypt`, `createHmac/verifyHmac`, `createSignature/verifySignature`, `acquireCertificate/listCertificates/proveCertificate/relinquishCertificate`, `discoverByIdentityKey/discoverByAttributes`, `isAuthenticated`, `getHeight/getHeaderForHeight/getNetwork/getVersion`.

**Hodos should audit** our `window.hodosBrowser` API surface against their CWI provider to identify missing methods we should expose.

### 3. Web2/Web3 Mode Toggle
Clean separation: browser works as a plain browser without wallet features. Toggle enables `window.CWI` injection into pages. Good UX — users who just want to browse aren't forced into wallet prompts.

**Hodos partially does this** (wallet panel is optional), but a more explicit mode toggle could improve UX.

### 4. Wallet Pairing / Remote Wallet
Support for connecting to a remote wallet via relay (WalletClient pattern). A phone could act as a signer for another device.

**Worth considering** for Hodos — a mobile companion app as a hardware-wallet-like signer.

### 5. Shamir Secret Sharing for Backup
QR-code-based Shamir share scanning for mnemonic recovery. Split seed across multiple QR codes with a threshold scheme.

**Worth considering** as an alternative backup method alongside our current encrypted backup.

### 6. BLE Local Payments
Bluetooth Low Energy peer-to-peer payments for in-person transfers. Scan nearby, select receiver, send.

**Not directly applicable** to desktop, but worth noting for future mobile versions.

### 7. i18n (10 Languages)
Full internationalization with react-i18next — EN, ES, FR, DE, IT, PT, RU, ZH, JA, KO.

**Hodos has none.** Not urgent for MVP, but good to plan for.

---

## What's Bad (Where Hodos is Far Ahead)

### No Security Isolation — CRITICAL WEAKNESS
All wallet logic in the same JS process. Derived keys in JS memory at runtime. One XSS or malicious package compromises everything.
**Hodos**: Separate Rust process, keys never in JavaScript, DPAPI/Keychain encryption.

### No Ad Blocking
Zero ad blocking, tracker blocking, fingerprint protection, or cookie management.
**Hodos**: 3-layer ad blocking (network + cosmetic + scriptlets), CefResponseFilter for YouTube, ephemeral cookie manager, entity-aware blocking, fingerprint farbling.

### No Privacy Features
No DNT/GPC headers, no cookie controls, no tracker awareness. Whatever the platform WebView does is what you get.

### WebView Limitations
- No service workers on iOS WKWebView
- No proper extension APIs
- Limited cache control
- Fragile JS-level download interception (blob URLs, anchor clicks)
- MediaSource polyfill is a hack
- User agent spoofing is brittle

**Hodos**: Full Chromium rendering, real download manager, H.264/AAC codec support built from source.

### No Tests
Zero automated tests. No test files, no test framework.

### Monolithic Code
`app/index.tsx` is 63KB (entire browser screen in one file). `context/WalletContext.tsx` is 55KB.

---

## Feature Adoption Priority

| Priority | Feature | Effort | Notes |
|----------|---------|--------|-------|
| **1** | HTTP 402 auto-payment | 3-5 days | Killer BSV differentiator. Detect in `HttpRequestInterceptor.cpp`, pay via wallet, cache response. |
| **2** | BRC-100 CWI API surface audit | 1 day | Compare `window.CWI` (28+ methods) vs `window.hodosBrowser` for gaps |
| **3** | Web2/Web3 mode toggle | 1 day | UX improvement for non-crypto browsing |
| **4** | Shamir backup shares | 3-5 days | Alternative backup method |
| **Later** | i18n framework | 1 week | When targeting international users |
| **Later** | Wallet pairing/remote signer | 2+ weeks | When building mobile companion |

---

## Summary

BSV Browser is a **mobile wallet-first app that happens to have a browser**. Hodos is a **real browser that happens to have a wallet**. Different products for different platforms.

Their wallet API surface (full BRC-100 CWI) and 402 payment handling are worth studying and adopting. Their architecture (WebView wrapper, no security isolation, no privacy features) is fundamentally weaker than Hodos for desktop use.
