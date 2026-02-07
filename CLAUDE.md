# HodosBrowser - Project Context for Claude

# Guidelines
Build with a production-focused mindset. Do not take shortcuts. If you get stuck do research on proper implementation plans/debugging steps
## Overview

A Web3 browser built on CEF (Chromium Embedded Framework) with a native Rust wallet backend. Implements BRC-100 protocol suite for Bitcoin SV authentication and micropayments. This is production software handling real money; security and correctness take priority over development speed.

---

## Architecture

Three layers with strict separation:

```
React Frontend (Port 5137)
    │ window.hodosBrowser.*
    ▼
C++ CEF Shell
    │ HTTP interception & forwarding → localhost:3301 for wallet functions
    ▼
Rust Wallet Backend (Port 3301)
    │
    ▼
Bitcoin SV Blockchain (WhatsOnChain, GorillaPool)
```

| Layer | Tech | Responsibility |
|-------|------|----------------|
| Frontend | React, Vite, TypeScript, MUI | UI, user interactions; never handles keys or signing |
| CEF Shell | C++17, CEF 136 | Browser engine, V8 injection, HTTP interception; browser data (history, bookmarks) |
| Wallet | Rust, Actix-web, SQLite | Crypto, signing, keys, BRC-100 protocol; private keys never leave this process |

**Overlay Model**: Settings, Wallet Panel, Backup Modal, and BRC-100 Auth each run as separate CEF subprocesses with isolated V8 contexts.

---

## Dev Runbook (Windows)

**Prerequisites**: PowerShell, VS 2022 (MSVC), vcpkg, Rust, Node.js 18+

**Run order** (all three must be running):

1. **Rust wallet**:
   ```powershell
   cd rust-wallet
   cargo run --release
   # Runs on localhost:3301
   ```

2. **Frontend dev server**:
   ```powershell
   cd frontend
   npm run dev
   # Runs on localhost:5137
   ```

3. **CEF browser**:
   ```powershell
   cd cef-native/build/bin/Release
   ./HodosBrowserShell.exe
   ```

**Storage**: `%APPDATA%/HodosBrowser/` (root), `%APPDATA%/HodosBrowser/wallet/wallet.db` (SQLite)

---

## Build (Windows)

First-time setup (requires CEF binaries already downloaded):

1. **CEF binaries**: Download from https://cef-builds.spotifycdn.com/index.html
   - Extract to `./cef-binaries/`

2. **CEF wrapper**:
   ```powershell
   cd cef-binaries/libcef_dll/wrapper
   mkdir build; cd build
   cmake .. -DCMAKE_TOOLCHAIN_FILE=[vcpkg_root]/scripts/buildsystems/vcpkg.cmake
   cmake --build . --config Release
   ```

3. **Rust wallet**:
   ```powershell
   cd rust-wallet
   cargo build --release
   ```

4. **Frontend**:
   ```powershell
   cd frontend
   npm install
   npm run build
   ```

5. **CEF shell**:
   ```powershell
   cd cef-native
   cmake -S . -B build -G "Visual Studio 17 2022" -A x64 -DCMAKE_TOOLCHAIN_FILE=[vcpkg_root]/scripts/buildsystems/vcpkg.cmake
   cmake --build build --config Release
   ```

---

## Invariants / Safety Rules

1. **Private keys never in JavaScript** - all signing happens in Rust
2. **Do not change wallet DB schema** without asking first
3. **Do not change crypto/signing/derivation logic** without asking first
4. **Plan first** for cross-cutting refactors; implement in small steps
5. **Prefer minimal, reversible changes** - avoid "big bang" rewrites
6. **Read files before editing** - always use Read tool before Edit tool
7. **Build after changes**:
   - Rust: `cargo build`
   - TypeScript: `npm run build`
   - C++: `cmake --build . --config Release`
8. User runs the browser to test - do not attempt to run it
9. CEF lifecycle & threading rules are fragile — do not change message loop, browser creation timing, or render-process handlers without asking first.


---

## Key Files

| File | Purpose |
|------|---------|
| `rust-wallet/src/handlers.rs` | HTTP endpoints: `health`, `get_public_key`, `well_known_auth`, `create_action`, `sign_action`, `list_certificates`, `acquire_certificate`, `wallet_sync` |
| `rust-wallet/src/crypto/` | Modules: `brc42` (`derive_child_private_key`), `brc43` (`InvoiceNumber`), `signing` (`sha256`, `hmac_sha256`), `aesgcm_custom` |
| `rust-wallet/src/database/` | Repos: `WalletRepository`, `AddressRepository`, `OutputRepository`, `CertificateRepository`, `ProvenTxRepository`; helpers: `get_master_private_key_from_db` |
| `rust-wallet/src/monitor/` | Background task scheduler: `Monitor`, `TaskCheckForProofs`, `TaskSendWaiting`, `TaskFailAbandoned`, `TaskUnFail`, `TaskReviewStatus`, `TaskPurge` |
| `cef-native/cef_browser_shell.cpp` | Entry point; globals: `g_hwnd`, `g_header_hwnd`, `g_webview_hwnd`, overlay HWNDs; class: `Logger` |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | V8 injection; class: `CefMessageSendHandler`; helper: `escapeJsonForJs` |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | HTTP routing; classes: `DomainVerifier`, `AsyncWalletResourceHandler`; global: `g_pendingAuthRequest` |
| `frontend/src/hooks/useHodosBrowser.ts` | React hook: `useHodosBrowser()` with `getIdentity`, `generateAddress`, `navigate`, `markBackedUp` |
| `frontend/src/bridge/initWindowBridge.ts` | Defines `window.hodosBrowser.navigation`, `window.hodosBrowser.overlay` via `cefMessage.send()` |

---

## Glossary

| Term | Meaning |
|------|---------|
| BRC-100 | BSV authentication/identity protocol suite |
| BRC-42 | ECDH-based child key derivation (master key + counterparty public key → child key) |
| BRC-43 | Invoice number format: `{securityLevel}-{protocolID}-{keyID}` |
| BRC-52 | Identity certificate format with selective disclosure |
| BRC-103/104 | Mutual authentication protocol |
| BEEF | Background Evaluation Extended Format - atomic transaction format with SPV proofs |
| BUMP | BRC-74 Binary Merkle Proof format. Used inside BEEF for SPV verification |
| CEF | Chromium Embedded Framework |
| ForkID SIGHASH | BSV-specific transaction signing (differs from BTC since 2017 fork) |
| HD Wallet | Hierarchical Deterministic wallet using BIP39 (mnemonic→seed) and BIP32 (seed→keys). Derivation path: `m/{index}` |
| UTXO | Unspent Transaction Output |
| V8 Injection | Adding `window.hodosBrowser` API to JavaScript from C++ |
| `window.hodosBrowser` | JavaScript API exposed to React for wallet operations |
| Monitor Pattern | Background task scheduler (`src/monitor/`) with 6 named tasks on configurable intervals. Replaced ad-hoc background services in Phase 6 |
| Browser Data | History, bookmarks, cookies — stored in C++ layer (`%APPDATA%/HodosBrowser/Default/`), separate from wallet |
