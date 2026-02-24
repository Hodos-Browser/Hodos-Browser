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

**Storage**: Windows: `%APPDATA%/HodosBrowser/`, macOS: `~/Library/Application Support/HodosBrowser/`. Wallet DB: `<storage>/wallet/wallet.db` (SQLite)

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
10. **macOS cross-platform readiness**: All new C++ code must use `#ifdef _WIN32` / `#elif defined(__APPLE__)` platform conditionals. Never use raw WinHTTP for new singletons — use `SyncHttpClient` (or add macOS `#elif` with libcurl). New overlays need a macOS creation function in `cef_browser_shell_mac.mm`. New file paths must use cross-platform resolution, not hardcoded Windows paths.
11. **Update docs with features**: When completing a sprint or feature that changes architecture, APIs, endpoints, or user-facing behavior, update CLAUDE.md Key Files table and any affected top-level docs. Don't let docs drift.


---

## Key Files

| File | Purpose |
|------|---------|
| `rust-wallet/src/handlers.rs` | 68+ HTTP endpoint handlers: wallet CRUD (`wallet_create`, `wallet_recover`, `wallet_balance`, `wallet_backup`), BRC-100 (`well_known_auth`, `create_action`, `create_hmac`, `create_signature`), domain permissions, price, sync status, and more |
| `rust-wallet/src/crypto/` | 11 modules: `brc42`, `brc43`, `signing`, `aesgcm_custom`, `dpapi` (Windows DPAPI / macOS Keychain stub), `pin` (PBKDF2+AES-GCM), `keys`, `brc2`, `ghash`, plus tests |
| `rust-wallet/src/database/` | 23 files, 18+ repos: `wallet_repo`, `address_repo`, `output_repo`, `certificate_repo`, `proven_tx_repo`, `domain_permission_repo`, `user_repo`, `settings_repo`, `backup`, `migrations`, `connection`, and more |
| `rust-wallet/src/recovery.rs` | BIP32 legacy key derivation (`derive_private_key_bip32`), wallet recovery from mnemonic |
| `rust-wallet/src/price_cache.rs` | BSV/USD price cache (CryptoCompare primary + CoinGecko fallback, 5-min TTL) |
| `rust-wallet/src/monitor/` | Background task scheduler: `Monitor`, `TaskCheckForProofs`, `TaskSendWaiting`, `TaskFailAbandoned`, `TaskUnFail`, `TaskReviewStatus`, `TaskPurge`, `TaskSyncPending` |
| `cef-native/cef_browser_shell.cpp` | Windows entry point; globals: `g_hwnd`, `g_header_hwnd`, `g_webview_hwnd`, overlay HWNDs (incl. `g_download_panel_overlay_hwnd`); class: `Logger`; overlay functions: `CreateDownloadPanelOverlay`, `ShowDownloadPanelOverlay`, `HideDownloadPanelOverlay` |
| `cef-native/cef_browser_shell_mac.mm` | macOS entry point (1754 lines); NSWindow/NSView hierarchy, 5 overlay types, event forwarding |
| `adblock-engine/src/engine.rs` | AdblockEngine wrapper: filter list downloading, engine compilation, serialization, `RwLock<Engine>` thread-safe checking. 4 filter lists (EasyList, EasyPrivacy, uBlock Filters, uBlock Privacy) + 6 bundled extra scriptlets. Auto-update every 6 hours. |
| `adblock-engine/src/handlers.rs` | HTTP endpoints on port 3302: `/health`, `/check`, `/status`, `/toggle`, `/cosmetic-resources`, `/cosmetic-hidden-ids` |
| `cef-native/include/core/AdblockCache.h` | `AdblockCache` singleton: sync WinHTTP to port 3302, URL result cache, per-browser blocked counts, cosmetic resource fetching. `AdblockBlockHandler` cancels blocked requests. `AdblockResponseFilter` (CefResponseFilter) buffers YouTube responses and renames ad-configuration JSON keys. `CookieFilterResourceHandler` returns cookie filter + response filter for YouTube. |
| `cef-native/src/handlers/simple_handler.cpp` | CEF client handler (12 interfaces incl. CefDownloadHandler, CefFindHandler, CefJSDialogHandler); IPC dispatch, keyboard shortcuts (Ctrl+F/H/J/D, Alt+Left/Right), context menus (5 context types, all custom `MENU_ID_USER_FIRST` IDs — see working-notes.md #8), download tracking, find-in-page (JS `window.find()` — CEF Find API non-functional in CEF 136), beforeunload trap suppression, `OnBeforeBrowse` scriptlet pre-cache, cosmetic CSS/scriptlet injection. Helpers: `CreateNewTabWithUrl()`, `CopyTextToClipboard()`. Cross-platform wrapped. |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | V8 injection; class: `CefMessageSendHandler`; helper: `escapeJsonForJs`; scriptlet pre-cache (`s_scriptCache` + `OnContextCreated` early injection); cosmetic CSS/script IPC handlers |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | HTTP routing + auto-approve engine; classes: `DomainPermissionCache`, `BSVPriceCache`, `WalletStatusCache`, `AsyncWalletResourceHandler`; singleton: `PendingRequestManager` (in PendingAuthRequest.h) |
| `cef-native/include/core/PendingAuthRequest.h` | `PendingRequestManager` singleton — thread-safe request tracking for auth/domain/payment/cert approvals |
| `cef-native/include/core/SessionManager.h` | `SessionManager` singleton + `BrowserSession` — per-browser session spending/rate tracking for auto-approve |
| `frontend/src/hooks/useHodosBrowser.ts` | React hook: `useHodosBrowser()` with `getIdentity`, `generateAddress`, `navigate`, `markBackedUp`, `goBack`, `goForward`, `reload` |
| `frontend/src/hooks/useDownloads.ts` | React hook for download state; listens for `download_state_update` IPC; exposes control functions (cancel, pause, resume, open, showInFolder, clearCompleted) |
| `frontend/src/pages/DownloadsOverlayRoot.tsx` | Download panel overlay page; lists active/completed downloads with progress bars, pause/resume/cancel, open/show-in-folder |
| `frontend/src/components/FindBar.tsx` | Find-in-page bar component; Ctrl+F triggered; sends `find_text`/`find_stop` IPC; displays "X of Y" match count |
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
| HD Wallet | Hierarchical Deterministic wallet using BIP39 (mnemonic→seed). New outputs use BRC-42 self-derivation; legacy BIP32 (`m/{index}`) preserved in recovery module |
| UTXO | Unspent Transaction Output |
| V8 Injection | Adding `window.hodosBrowser` API to JavaScript from C++ |
| `window.hodosBrowser` | JavaScript API exposed to React for wallet operations |
| Monitor Pattern | Background task scheduler (`src/monitor/`) with 7 named tasks on configurable intervals. Replaced ad-hoc background services in Phase 6 |
| Browser Data | History, bookmarks, cookies — stored in C++ layer (`%APPDATA%/HodosBrowser/Default/`), separate from wallet |
| CefResponseFilter | CEF API for streaming modification of HTTP response bodies. Used by `AdblockResponseFilter` to strip YouTube ad keys at the network level before JavaScript sees the data |
| Cosmetic Filtering | CSS selector injection to hide ad-related DOM elements + scriptlet injection to override JavaScript ad behavior. Two-phase: hostname-specific selectors on page load, generic selectors after DOM class/ID collection |
| Scriptlet Injection | JavaScript injected into page context via V8 to override browser APIs (fetch, XHR, JSON.parse) and strip ad data. Pre-cached via `OnBeforeBrowse` IPC, injected in `OnContextCreated` |
