# HodosBrowser - Project Context for Claude

# Guidelines

Build with a production-focused mindset. Do not take shortcuts. If you get stuck do research on proper implementation plans/debugging steps.

## Testing Standards

**Every feature must be tested against real-world sites.** Standard verification sites are listed below.

| Level | When | Duration | Sites |
|-------|------|----------|-------|
| **Minimal** | After any browser-core change | 5 min | youtube.com, x.com, github.com |
| **Standard** | After sprint completion | 15 min | Auth category + 2-3 video/media + 1-2 news |
| **Thorough** | Before release/demo | 30-45 min | Full basket, all categories |

**Categories:** Authentication (x.com, google.com, github.com), Video/Media (youtube.com, twitch.tv), News/Content (nytimes.com, reddit.com), E-commerce (amazon.com), Productivity (docs.google.com), BSV (whatsonchain.com)
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
    │ HTTP interception & forwarding → localhost:31301 for wallet functions
    ▼
Rust Wallet Backend (Port 31301)
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

## ⚠️ CRITICAL: UI Architecture Rules

**NEVER add new panels/menus/dropdowns directly to MainBrowserView.tsx (header_hwnd).**

All UI panels MUST be implemented as **overlays** in their own CEF subprocess:

| Component | Implementation | Location |
|-----------|---------------|----------|
| Wallet Panel | ✅ Overlay | `WalletPanelPage.tsx` → `WalletOverlayRoot.tsx` |
| Settings | ✅ Overlay | `SettingsOverlayRoot.tsx` |
| Cookie Panel | ✅ Overlay | `CookiePanelOverlayRoot.tsx` |
| Downloads | ✅ Overlay | `DownloadsOverlayRoot.tsx` |
| Privacy Shield | ✅ Overlay | `PrivacyShieldOverlayRoot.tsx` |
| Omnibox | ✅ Overlay | `OmniboxOverlayRoot.tsx` |
| Menu | ✅ Overlay | `MenuOverlayRoot.tsx` |
| Profile Picker | ✅ Overlay | `ProfilePickerOverlayRoot.tsx` |

**Why overlays?**
- Each overlay is isolated V8 context (security)
- Doesn't block main browser thread
- Can be positioned relative to toolbar icons
- Consistent UX pattern across all panels

**Pattern for new panels:**
1. Create `<Name>OverlayRoot.tsx` in `frontend/src/pages/`
2. Add route in `frontend/src/App.tsx`
3. Add C++ handler to show/hide overlay in `simple_handler.cpp`
4. Trigger via `window.cefMessage.send('<name>_panel_show', [offset])`

**MainBrowserView.tsx should ONLY contain:**
- Tab bar
- Navigation buttons (back/forward/refresh)
- Address bar input
- Toolbar icon buttons (that TRIGGER overlays)
- Find bar (inline exception)

---

## ⚠️ Overlay Lifecycle & Close Prevention (IMPORTANT — Windows)

> **macOS note:** On macOS, overlays use `NSPanel` (not `WS_POPUP`). Close behavior is handled via `NSWindowDelegate` and `resignKey`/`resignMain` notifications in `cef_browser_shell_mac.mm`. The patterns below are Windows-specific.

Overlays are WS_POPUP windows (not children of `g_hwnd`). Each overlay has a different close/destroy pattern. Understanding these is critical for UX work.

### Overlay Close Mechanisms

| Mechanism | Where | Overlays Affected |
|-----------|-------|-------------------|
| **Click-outside (React)** | `handleBackgroundClick()` in overlay page | Wallet, Settings (full-page overlays with transparent backdrop) |
| **Click-outside (Mouse hook)** | `WH_MOUSE_LL` hook in C++ | Cookie, Download, Menu, Profile, Omnibox (dropdown-style overlays) |
| **IPC `overlay_close`** | React → `simple_handler.cpp` | All overlays (explicit close from React) |
| **Focus loss (`WM_ACTIVATEAPP`)** | `cef_browser_shell.cpp` WndProc | Omnibox only. **Wallet is exempt** (user may switch apps to paste mnemonic) |
| **Old overlay cleanup** | `CreateXxxOverlay()` functions | Destroys existing overlay before creating new one |

### Close Prevention Patterns

**1. `g_file_dialog_active` (C++ synchronous guard)**
- Set to `true` in `OnFileDialog()` (C++ side, synchronous — before dialog opens)
- Cleared on `WM_ACTIVATEAPP(TRUE)` (app regains focus)
- Guards ALL overlays during native file dialog
- **Works because it's set synchronously in C++ before focus loss can fire**

**2. `g_wallet_overlay_prevent_close` (React → C++ IPC flag)**
- Set via `wallet_prevent_close` / `wallet_allow_close` IPC messages from React
- Guards React's `handleBackgroundClick()` (click-outside in React code)
- **⚠️ Cannot guard `WM_ACTIVATEAPP`** — IPC is async, flag may not be set before focus loss fires
- Auto-cleared on `overlay_close` IPC

**3. Wallet creation-time default (flag set in C++, cleared by React)**
- `g_wallet_overlay_prevent_close` is set to `true` in `CreateWalletOverlayWithSeparateProcess()` (synchronous, no race)
- React sends `wallet_allow_close` IPC once user reaches a safe state (live wallet, loading, locked)
- React sends `wallet_prevent_close` IPC when entering unsafe state (mnemonic display, PIN entry)
- Result: new overlay survives focus loss by default; React opts in to allow close once ready

### Key Rule: Synchronous vs Async Guards

> **If you need to prevent overlay close during a C++ event (like `WM_ACTIVATEAPP`), the guard flag MUST be set synchronously from C++.** React → IPC → C++ flags have a race condition because `WM_ACTIVATEAPP` fires immediately when the user clicks another window, before async IPC messages arrive.

**Safe pattern:** Set flag in `CreateXxxOverlay()` or in `OnFileDialog()` (C++ side)
**Unsafe pattern:** Set flag via React `useEffect` → `cefMessage.send()` → IPC handler (async, race condition)

### Destruction Paths for Wallet Overlay (5 total)

1. **`WM_ACTIVATE(WA_INACTIVE)` in `WalletOverlayWndProc`** — THE PRIMARY close path. Fires when wallet HWND loses activation (click outside, Alt+Tab, click another app). Guarded by `g_wallet_overlay_prevent_close`. This is the WndProc for the overlay HWND itself (`cef_browser_shell.cpp` ~line 1297).
2. **`WM_ACTIVATEAPP` in main WndProc** — App-level focus loss. Also guarded by `g_wallet_overlay_prevent_close`. (`cef_browser_shell.cpp` ~line 952).
3. **IPC `overlay_close`** from React → `simple_handler.cpp` destroys HWND
4. **Old overlay cleanup** in `CreateWalletOverlayWithSeparateProcess()` → destroys existing before creating new
5. **Application shutdown** → `ShutdownApplication()` cleanup

> **Key lesson:** Overlays have BOTH app-level (`WM_ACTIVATEAPP`) AND HWND-level (`WM_ACTIVATE`) close paths. Both must be guarded. The HWND-level `WM_ACTIVATE` in the overlay's own WndProc is typically the one that actually fires first.

### Code Locations

| What | File | Line Reference |
|------|------|----------------|
| Overlay globals & flags | `cef_browser_shell.cpp` | Lines 51-90 (globals section) |
| `WalletOverlayWndProc` (`WM_ACTIVATE`) | `cef_browser_shell.cpp` | ~line 1297 (**primary close path**) |
| `WM_ACTIVATEAPP` handler (main WndProc) | `cef_browser_shell.cpp` | ~line 952 |
| `overlay_close` IPC | `simple_handler.cpp` | ~line 3020 |
| Wallet overlay creation + flag init | `simple_app.cpp` | `CreateWalletOverlayWithSeparateProcess()` ~line 654 |
| Prevent-close IPC handlers | `simple_handler.cpp` | `wallet_prevent_close` / `wallet_allow_close` ~line 3005 |
| React preventClose logic | `WalletPanelPage.tsx` | `preventClose` derived state + `useEffect` |

---

## ⚠️ CEF Input Patterns (IMPORTANT)

CEF overlays have quirks with form inputs. Follow these patterns:

### Text Inputs
- **Use native `<input>` elements**, not MUI `TextField`
- MUI's extra layers break CEF focus handling
- Add delayed focus with `useEffect` + `setTimeout(50ms)`

```tsx
// ✅ Works in CEF
<input
  type="text"
  style={{ width: '100%', padding: '8px', border: '1px solid #ccc', borderRadius: '4px' }}
  onFocus={(e) => e.target.style.borderColor = '#1a73e8'}
  onBlur={(e) => e.target.style.borderColor = '#ccc'}
/>

// ❌ Broken in CEF overlays
<TextField variant="outlined" />
```

### File Inputs
- **Use VISIBLE file inputs**, not hidden ones triggered by click
- CEF handles visible `<input type="file">` correctly
- Hidden file inputs triggered via `.click()` often fail

```tsx
// ✅ Works in CEF (visible input)
<div style={{ background: '#f5f5f5', padding: '8px', borderRadius: '4px' }}>
  <input type="file" accept="image/*" onChange={handleFile} style={{ width: '100%' }} />
</div>

// ❌ Unreliable in CEF (hidden + click trigger)
<input type="file" style={{ display: 'none' }} ref={ref} />
<button onClick={() => ref.current?.click()}>Choose File</button>
```

### Reference Implementation
- `WalletPanelPage.tsx` — working file input for wallet recovery
- `BackupOverlayRoot.tsx` — working native text inputs

### Focus & Keyboard Handling (C++ side)
CEF windowless overlays need explicit focus AND keyboard event forwarding:

**1. HWND Creation:**
```cpp
// Use WS_VISIBLE flag for proper focus
WS_POPUP | WS_VISIBLE,  // NOT just WS_POPUP
```

**2. Browser Settings:**
```cpp
settings.javascript_access_clipboard = STATE_ENABLED;
settings.javascript_dom_paste = STATE_ENABLED;
```

**3. WndProc (CRITICAL):**
```cpp
case WM_MOUSEACTIVATE:
    return MA_ACTIVATE;  // NOT MA_NOACTIVATE!

case WM_LBUTTONDOWN:
    SetFocus(hwnd);  // Windows focus
    browser->GetHost()->SetFocus(true);  // CEF focus
    browser->GetHost()->SendMouseClickEvent(...);
    return 0;

case WM_KEYDOWN:
case WM_KEYUP:
case WM_CHAR:
    // Forward ALL keyboard events to CEF browser
    browser->GetHost()->SendKeyEvent(key_event);
    return 0;
```

**4. OnAfterCreated:**
```cpp
browser->GetHost()->SetFocus(true);
```

**Reference:** `WalletOverlayWndProc` in `cef_browser_shell.cpp` — working keyboard input

---

## Dev Runbook

**Run order** (all three must be running):

1. **Rust wallet**: `cd rust-wallet && cargo run --release` → localhost:31301
2. **Frontend dev server**: `cd frontend && npm run dev` → localhost:5137
3. **CEF browser**:
   - Windows: `cd cef-native/build/bin/Release && ./HodosBrowserShell.exe`
   - macOS: `cd cef-native/build/bin && ./HodosBrowserShell.app/Contents/MacOS/HodosBrowserShell`

**Storage**: Windows: `%APPDATA%/HodosBrowser/`, macOS: `~/Library/Application Support/HodosBrowser/`. Wallet DB: `<storage>/wallet/wallet.db` (SQLite)

---

## Build

**Prerequisites**: Rust, Node.js 18+, CEF binaries (download from https://cef-builds.spotifycdn.com/index.html → `./cef-binaries/`)

**Platform-specific build guides**: See `build-instructions/WINDOWS_BUILD_INSTRUCTIONS.md` or `build-instructions/MACOS_BUILD_INSTRUCTIONS.md` for first-time setup.

**Quick build (all platforms):**
```bash
# 1. CEF wrapper (first time only)
cd cef-binaries/libcef_dll/wrapper && mkdir build && cd build
cmake .. && cmake --build . --config Release

# 2. Rust wallet
cd rust-wallet && cargo build --release

# 3. Frontend
cd frontend && npm install && npm run build

# 4. CEF shell
cd cef-native
# Windows: cmake -S . -B build -G "Visual Studio 17 2022" -A x64 -DCMAKE_TOOLCHAIN_FILE=[vcpkg_root]/scripts/buildsystems/vcpkg.cmake
# macOS:   cmake -S . -B build -G "Unix Makefiles"
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
| `rust-wallet/src/handlers.rs` | 76+ HTTP endpoint handlers: wallet CRUD (`wallet_create`, `wallet_recover`, `wallet_balance`, `wallet_backup`), BRC-100 (`well_known_auth`, `create_action`, `create_hmac`, `create_signature`), domain permissions, price, sync status, PeerPay (`peerpay_send`, `peerpay_check`, `peerpay_status`, `peerpay_dismiss`), and more |
| `rust-wallet/src/crypto/` | 11 modules: `brc42`, `brc43`, `signing`, `aesgcm_custom`, `dpapi` (Windows DPAPI / macOS Keychain stub), `pin` (PBKDF2+AES-GCM), `keys`, `brc2`, `ghash`, plus tests |
| `rust-wallet/src/authfetch.rs` | BRC-103 AuthFetch HTTP client: 401 challenge-response with ECDSA signing, server/client nonce exchange, authenticated requests to external BRC-103 servers (MessageBox) |
| `rust-wallet/src/messagebox.rs` | MessageBox API client: BRC-2 encrypted message send/receive/acknowledge via `messagebox.babbage.systems`, deterministic HMAC message IDs, uses AuthFetch for authentication |
| `rust-wallet/src/database/` | 24 files, 19+ repos: `wallet_repo`, `address_repo`, `output_repo`, `certificate_repo`, `proven_tx_repo`, `domain_permission_repo`, `peerpay_repo`, `user_repo`, `settings_repo`, `backup`, `migrations`, `connection`, and more |
| `rust-wallet/src/recovery.rs` | BIP32 legacy key derivation (`derive_private_key_bip32`), wallet recovery from mnemonic |
| `rust-wallet/src/price_cache.rs` | BSV/USD price cache (CryptoCompare primary + CoinGecko fallback, 5-min TTL) |
| `rust-wallet/src/monitor/` | Background task scheduler: `Monitor`, `TaskCheckForProofs`, `TaskSendWaiting`, `TaskFailAbandoned`, `TaskUnFail`, `TaskReviewStatus`, `TaskPurge`, `TaskSyncPending`, `TaskCheckPeerPay` (BRC-103 AuthFetch + BRC-2 encrypted MessageBox polling + auto-accept via `internalize_action`) |
| `cef-native/cef_browser_shell.cpp` | Windows entry point; globals: `g_hwnd`, `g_header_hwnd`, `g_webview_hwnd`, overlay HWNDs (incl. `g_download_panel_overlay_hwnd`); class: `Logger`; overlay functions: `CreateDownloadPanelOverlay`, `ShowDownloadPanelOverlay`, `HideDownloadPanelOverlay` |
| `cef-native/cef_browser_shell_mac.mm` | macOS entry point (~3900 lines); NSWindow/NSView hierarchy, 10 overlay types (settings, wallet, backup, BRC100 auth, notification, settings menu, cookie panel, omnibox, downloads, profile picker), event forwarding, multi-window support |
| `adblock-engine/src/engine.rs` | AdblockEngine wrapper: filter list downloading, engine compilation, serialization, `RwLock<Engine>` thread-safe checking. 4 filter lists (EasyList, EasyPrivacy, uBlock Filters, uBlock Privacy) + 6 bundled extra scriptlets. Auto-update every 6 hours. |
| `adblock-engine/src/handlers.rs` | HTTP endpoints on port 31302: `/health`, `/check`, `/status`, `/toggle`, `/cosmetic-resources`, `/cosmetic-hidden-ids` |
| `cef-native/include/core/AdblockCache.h` | `AdblockCache` singleton: sync WinHTTP to port 31302, URL result cache, per-browser blocked counts, cosmetic resource fetching. `AdblockBlockHandler` cancels blocked requests. `AdblockResponseFilter` (CefResponseFilter) buffers YouTube responses and renames ad-configuration JSON keys. `CookieFilterResourceHandler` returns cookie filter + response filter for YouTube. |
| `cef-native/src/handlers/simple_handler.cpp` | CEF client handler (12 interfaces incl. CefDownloadHandler, CefFindHandler, CefJSDialogHandler); IPC dispatch, keyboard shortcuts (Ctrl+F/H/J/D, Alt+Left/Right), context menus (5 context types, all custom `MENU_ID_USER_FIRST` IDs — see working-notes.md #8), download tracking, find-in-page (JS `window.find()` — CEF Find API non-functional in CEF 136), beforeunload trap suppression, `OnBeforeBrowse` scriptlet pre-cache + fingerprint seed IPC, cosmetic CSS/scriptlet injection, menu IPC (print/devtools/zoom/exit), DNT/GPC header injection, settings_set dispatch. Helpers: `CreateNewTabWithUrl()`, `CopyTextToClipboard()`. Cross-platform wrapped. |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | V8 injection; class: `CefMessageSendHandler`; helper: `escapeJsonForJs`; scriptlet pre-cache (`s_scriptCache` + `OnContextCreated` early injection); cosmetic CSS/script IPC handlers; fingerprint seed cache (`s_domainSeeds`) + fingerprint script injection in `OnContextCreated` |
| `cef-native/include/core/FingerprintProtection.h` | `FingerprintProtection` singleton: platform CSPRNG session token, per-domain seed generation via hash mixing, enable/disable toggle |
| `cef-native/include/core/FingerprintScript.h` | Embedded JS constant `FINGERPRINT_PROTECTION_SCRIPT`: Mulberry32 PRNG, Canvas/WebGL/Navigator/AudioContext farbling (no screen resolution spoofing) |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | HTTP routing + auto-approve engine; classes: `DomainPermissionCache`, `BSVPriceCache`, `WalletStatusCache`, `AsyncWalletResourceHandler`; singleton: `PendingRequestManager` (in PendingAuthRequest.h) |
| `cef-native/include/core/PendingAuthRequest.h` | `PendingRequestManager` singleton — thread-safe request tracking for auth/domain/payment/cert approvals |
| `cef-native/include/core/SessionManager.h` | `SessionManager` singleton + `BrowserSession` — per-browser session spending/rate tracking for auto-approve |
| `cef-native/include/core/ProfileManager.h` | `ProfileManager` singleton: multi-profile support, profile creation/switching, profile directory management |
| `cef-native/include/core/TabManager.h` | `TabManager` singleton: per-window tab tracking, tab creation/close/switch, multi-window tab coordination |
| `cef-native/include/core/WindowManager.h` | `WindowManager` singleton: multi-window lifecycle, window creation/destruction, window-to-tab mapping |
| `cef-native/include/core/SettingsManager.h` | `SettingsManager` singleton: persistent settings storage, cross-platform settings resolution |
| `cef-native/include/core/ProfileImporter.h` | Chrome/Edge profile importer: bookmarks, history, cookies import from other browsers |
| `cef-native/include/core/SyncHttpClient.h` | Cross-platform sync HTTP client (WinHTTP on Windows, libcurl on macOS). Use this for new singletons instead of raw WinHTTP |
| `frontend/src/hooks/useHodosBrowser.ts` | React hook: `useHodosBrowser()` with `getIdentity`, `generateAddress`, `navigate`, `markBackedUp`, `goBack`, `goForward`, `reload` |
| `frontend/src/hooks/useDownloads.ts` | React hook for download state; listens for `download_state_update` IPC; exposes control functions (cancel, pause, resume, open, showInFolder, clearCompleted) |
| `frontend/src/pages/DownloadsOverlayRoot.tsx` | Download panel overlay page; lists active/completed downloads with progress bars, pause/resume/cancel, open/show-in-folder |
| `frontend/src/components/FindBar.tsx` | Find-in-page bar component; Ctrl+F triggered; sends `find_text`/`find_stop` IPC; displays "X of Y" match count |
| `frontend/src/components/MenuOverlay.tsx` | Three-dot menu dropdown: New Tab, Find, Print, Zoom controls, Bookmark, Downloads, History, DevTools, Settings, Exit. Replaces old History+Settings buttons. |
| `frontend/src/pages/SettingsPage.tsx` | Full-page settings with sidebar navigation (General, Privacy, Downloads, Wallet, About). Route: `/settings-page/:section` |
| `frontend/src/hooks/usePrivacyShield.ts` | Composed privacy hook: adblock + cookie blocking + scriptlet toggle state. Used by `PrivacyShieldPanel` overlay |
| `frontend/src/bridge/initWindowBridge.ts` | Defines `window.hodosBrowser.navigation`, `window.hodosBrowser.overlay` via `cefMessage.send()` |

---

## Glossary

| Term | Meaning |
|------|---------|
| BRC-100 | BSV authentication/identity protocol suite |
| BRC-42 | ECDH-based child key derivation (master key + counterparty public key → child key) |
| BRC-43 | Invoice number format: `{securityLevel}-{protocolID}-{keyID}` |
| BRC-52 | Identity certificate format with selective disclosure |
| BRC-2 | Symmetric encryption using BRC-42-derived AES-256-GCM keys. Used for MessageBox message encryption |
| BRC-29 | PeerPay direct payment protocol: sender derives recipient key via BRC-42, creates P2PKH output, sends PaymentToken via encrypted MessageBox. Protocol ID: `3241645161d8` |
| BRC-103/104 | Mutual authentication protocol. Client side (`authfetch.rs`): 401 challenge → sign nonces+request → re-send with auth headers |
| MessageBox | Remote message relay service at `messagebox.babbage.systems`. BRC-103 authenticated, BRC-2 encrypted. Used for PeerPay payment delivery |
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
| Fingerprint Farbling | Brave-style fingerprint randomization: per-session token hashed with domain → deterministic PRNG seed → Canvas/WebGL/Audio/Navigator values slightly perturbed. Same values within session+domain, different across sessions |
| `#@#+js()` | adblock-rust exception syntax: blanket disable all scriptlet injection for a domain. Used in `hodos-unbreak.txt` for auth sites |

---

## Context File Maintenance

**After each sprint, phase, or sub-phase:**
1. Review this CLAUDE.md — Is it still accurate? Update Key Files table if architecture changed.
2. Check sprint-specific CLAUDE.md in `development-docs/Final-MVP-Sprint/` or `development-docs/UX_UI/`.
3. Add new patterns/gotchas to the relevant context file.

**Goal:** Context files should always reflect current reality. They're the institutional memory that lets any AI (or human) pick up where the last session left off.

### Sprint Documentation

| Folder | Purpose |
|--------|---------|
| `development-docs/Final-MVP-Sprint/` | Active sprint: testing, optimization, security, macOS port |
| `development-docs/Final-MVP-Sprint/macos-port/` | macOS port tracking: progress, handover docs, archived milestones |
| `development-docs/UX_UI/` | Wallet UI phases (setup, notifications, wallet panel polish) |
| `development-docs/Possible-MVP-Features/` | Roadmap items and feature research |
| `build-instructions/` | Platform-specific build guides (Windows, macOS) |
