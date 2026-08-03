# Security & Process Isolation Analysis

**Date**: October 9, 2025 (Last Updated: 2026-08-03)
**Focus**: Current security model and process isolation architecture

## 🔐 Current Process Architecture

### Process Map

The browser runs a **variable** number of OS processes. The count is not fixed — it grows with
open tabs and with which overlays have been opened — so this map describes **classes** of process
and the trust level of each. Per-overlay and per-handler rosters live in the layer docs
(`cef-native/CLAUDE.md`, `cef-native/src/handlers/CLAUDE.md`, `cef-native/src/core/CLAUDE.md`).

```
┌──────────────────────────────────────────────────────────────┐
│  CLASS A (×1): Main Browser Process (cef_browser_shell.cpp)  │
│  - Window management (WM_SIZE, WM_MOVE, WM_CLOSE)            │
│  - HWND creation and coordination                            │
│  - Logger initialization                                     │
│  - Graceful shutdown orchestration                           │
│  - Mediates ALL cross-process messages                       │
│  - NO web content rendering                                  │
│  - NO JavaScript execution                                   │
└──────────────────────────────────────────────────────────────┘
                              │
                              ├─────────────────────────────────┐
                              ▼                                 ▼
┌────────────────────────────────────┐    ┌─────────────────────────────────┐
│ CLASS B (×1 per window):           │    │ CLASS C (×N, one per tab):      │
│ Header Browser                     │    │ Tab Browsers                    │
│ Role: "header"                     │    │ Role: "tab_{id}" (tab_0, tab_1) │
│ Origin: http://127.0.0.1:5137      │    │ Origin: whatever the user loads │
│                                    │    │                                 │
│ - React UI rendering               │    │ - External website rendering    │
│ - Navigation controls              │    │ - Web content from internet     │
│ - Wallet/Settings buttons          │    │ - HTTP interception active      │
│ - Own V8 context                   │    │ - Domain permission gate applies│
│ - TRUSTED internal origin          │    │ - UNTRUSTED                     │
│ - WS_CHILD window                  │    │ - WS_CHILD window               │
└────────────────────────────────────┘    └─────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│ CLASS D (×0..M, one per opened overlay): Overlay Browsers    │
│ Roles: "settings", "wallet"/"wallet_panel", "backup",        │
│        "brc100auth", "notification", omnibox, cookie panel,  │
│        downloads, menu, profile picker, privacy shield, …    │
│ Origin: http://127.0.0.1:5137/<route> — ALL the same origin  │
│                                                              │
│ - Each is its own CefBrowser with its own V8 context         │
│ - WS_POPUP + layered + topmost (outside page paint area)     │
│ - TRUSTED internal origin — full wallet API by design        │
│ - Most are keep-alive: hidden on close, not destroyed        │
│ Full overlay roster: cef-native/CLAUDE.md                    │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│ CLASS E: Chromium infrastructure (GPU, utility, network)     │
│ - Count and composition managed by Chromium, not by us       │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│ CLASS F (×1): Rust Wallet Backend (separate OS process)      │
│ - HD wallet management                                       │
│ - Transaction creation/signing/broadcasting                  │
│ - UTXO management                                            │
│ - BRC-100 authentication                                     │
│ - Permission / auto-approve DECISION engine (Actix middleware)│
│ - HTTP API server: 127.0.0.1:31301 (31401 when HODOS_DEV=1)  │
│ - CORS allowlist: Hodos's own local UI origins only          │
│ - Started by C++ (WalletService.cpp), runs independently     │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│ CLASS G (×1): Adblock Engine (separate OS process)           │
│ - HTTP API server: 127.0.0.1:31302 (31402 when HODOS_DEV=1)  │
│ - Spawned by C++ and held in a Job Object so it dies with us  │
└──────────────────────────────────────────────────────────────┘
```

**Ports are not literals to be copied around.** The single source of truth is
`cef-native/include/core/PortConfig.h` on the C++ side and `rust-wallet/src/main.rs :: wallet_port`
on the Rust side. Both key off `HODOS_DEV=1` so a dev build and an installed build can run
side by side without fighting over a port.

**At-rest secret storage is per-platform.** The BIP39 recovery phrase is NOT uniformly
"DPAPI-encrypted":

| Platform | Primary `mnemonic` column | Auto-unlock copy |
|----------|---------------------------|------------------|
| Windows | PIN-encrypted (PBKDF2 + AES-GCM, `rust-wallet/src/crypto/pin.rs`) **if a PIN was set** — otherwise plaintext | Best-effort DPAPI blob in a separate `mnemonic_dpapi` column (`database/wallet_repo.rs`); non-fatal if unavailable |
| macOS | Same PIN rule | OS Keychain (service `HodosBrowser`/`HodosBrowserDev`, account `wallet-mnemonic`, `crypto/dpapi.rs`); the DB column holds only an 8-byte sentinel |
| Linux / other | Same PIN rule | **None** — `crypto/dpapi.rs` returns `Err` on these targets. Only the PIN path protects the phrase |

> ⚠️ At least one wallet-creation path passes `pin = None`
> (`rust-wallet/src/handlers.rs :: create_wallet_from_existing_mnemonic` call site), which stores the
> phrase in plaintext in the `mnemonic` column. That is a real at-rest exposure on a machine where
> the user never set a PIN.

### Process Isolation Benefits

**Header Browser (React UI):**
- ✅ Isolated from web content
- ✅ Cannot be compromised by malicious websites
- ✅ Always trustworthy UI
- ✅ Controls navigation/wallet access

**Tab Browsers (Web Content):**
- ✅ Isolated from UI controls
- ✅ Can't modify navigation bar
- ✅ Can't intercept wallet button clicks
- ⚠️ Web content reaches the browser process through **two** channels, not one:
  1. **The HTTP interceptor** — `HttpRequestInterceptor :: isWalletEndpoint` routes wallet-bound
     requests into the permission cascade.
  2. **The Phase 2.5 `wallet_call` process-message bridge** — `simple_handler.cpp ::
     OnProcessMessageReceived` → `HandleIpcWalletCall`. This is the *primary* wallet path for
     external dApps, because it sidesteps CSP/CORS. It never enters the
     `CefResourceRequestHandler` pipeline.
- ⚠️ Separately, `window.cefMessage.send` is injected into **every** V8 context, including external
  pages (`simple_render_process_handler.cpp :: OnContextCreated`), and `OnProcessMessageReceived`
  dispatches its message names with **no origin, role, or frame check**. That IPC surface is not
  gated. See "Current Security Gaps" below.

**Overlay Browsers:**
- ✅ Each overlay is its own CefBrowser with its own V8 context — isolated at the **browser/V8
  level** from web content and from every other overlay
- ✅ Independent lifecycle (can close without affecting others)
- ⚠️ **Not** isolated at the storage or privilege level. Every overlay is created with
  `CefRequestContext::GetGlobalContext()` and loads same-origin from `http://127.0.0.1:5137/<route>`,
  so all overlays share one cookie jar, localStorage, sessionStorage, IndexedDB and BroadcastChannel
  namespace, backed by a single profile `cache_path`.
- ⚠️ **"Settings can't access wallet state" is false.** Every overlay receives the wallet transport
  bridge and is an explicitly trusted internal origin — see "Trust Boundary: Hodos's own UI" below.
- ⚠️ **"Fresh V8 context prevents state pollution" is false.** Most overlays are keep-alive: closing
  hides the HWND rather than destroying the browser (`simple_handler.cpp`, `overlay_close` →
  `HideWalletOverlay()`), so the same V8 context and React state persist across open/close cycles.
  Wallet, omnibox, cookie/privacy-shield, downloads, menu, profile picker and notification all reuse.
  Only the settings overlay destroys and recreates. **A page that needs a clean slate must reset
  itself explicitly** — the notification overlay does exactly this via `window.showNotification()`.

**Rust Wallet:**
- ✅ Separate process = can't be directly memory-exploited from web
- ✅ Only reachable over the localhost HTTP API — bound to `127.0.0.1`, never a routable interface —
  and browser-originated calls are further narrowed by the CORS allowlist below
- ✅ Domain trust check enforced in Rust on every request (universal Actix middleware), independent
  of whatever C++ did or didn't do
- ✅ **Signing keys never leave the Rust process. No EC private key is ever returned to JavaScript,
  and no signing happens there. The BIP39 recovery phrase is the one deliberate exception: it is
  shown once at wallet creation so the user can record it, and thereafter only through PIN
  re-verification in the wallet overlay. It is never reachable from web content — the wallet's CORS
  allowlist admits only Hodos's own local UI origins.**
- ℹ️ Mechanically: there is no signing code anywhere in `cef-native/` — no secp256k1, no ECDSA, no
  EC_KEY, no private-key material. The mnemonic, on the create-wallet and PIN-gated
  `/wallet/reveal-mnemonic` flows, does transit the C++ browser process as a response body on its way
  to the wallet-overlay renderer that displays it, because first-party wallet calls are relayed
  through the `wallet_call` IPC bridge (`frontend/src/services/walletApi.ts` →
  `simple_handler.cpp :: OnProcessMessageReceived` → `HttpRequestInterceptor.cpp :: runIpcCallDirect`).

### Trust Boundary: Hodos's own UI

Hodos's overlay and header UI run at an **internal origin** (`http://127.0.0.1:5137`) and are
**deliberately trusted with the full wallet API**. This is a designed trust boundary, not a
vulnerability — it is the same relationship a native app's own window has with its own backend.

Two mechanisms implement it:

1. **Resource-handler bypass.** `simple_handler.cpp :: GetResourceRequestHandler` returns `nullptr`
   for wallet host:port URLs when `role_` is `wallet`, `wallet_panel`, `settings` or `backup` —
   "trusted overlay direct wallet request, bypassing all handlers" — so CEF's native network stack
   carries them. This exists to avoid `CefURLRequest` forwarding problems on macOS.
2. **Internal-origin direct dispatch.** `HttpRequestInterceptor.cpp :: HandleIpcWalletCall`
   short-circuits internal origins to `runIpcCallDirect`, which deliberately sends the request
   **without** the `X-Requesting-Domain` header, so Rust's `check_domain_approved` classifies the
   caller as internal and allows it.

**Consequence to hold in mind:** no permission-engine gate and no domain gate applies to a call
originating from Hodos's own UI. Endpoint-level checks still apply where they exist — e.g.
`/wallet/reveal-mnemonic` re-verifies the PIN inside Rust regardless of caller. **The security of
this boundary rests entirely on nothing untrusted being able to execute at the internal origin.**

### The wallet CORS allowlist

`rust-wallet/src/main.rs` wraps the Actix app in a CORS layer admitting exactly four origins:

```
http://127.0.0.1:5137     http://localhost:5137
http://127.0.0.1          http://localhost
```

with `allow_any_method()`, `allow_any_header()` and a 3600 s preflight max-age. These are Hodos's own
local UI origins — the Vite dev/UI server and bare localhost. **No web origin is on this list**, so a
page's own `fetch()` to the wallet port is rejected by the browser before its response reaches page
JS. CORS is a browser-enforced control, so it constrains web content specifically — it is not a
substitute for a server-side gate against non-browser callers on the same machine.

This is defense-in-depth, not the primary gate: in production, C++ intercepts wallet-bound requests
before they reach Rust, and website JS is expected to use `window.hodosBrowser.*` / the `wallet_call`
IPC bridge rather than direct fetch. CORS is what remains standing if that layer is bypassed.

> Note the shape of the boundary: the allowlist is an **origin** allowlist, and the internal-origin
> trust described above is also **origin**-based. Both collapse together if untrusted content can
> ever run at `127.0.0.1:5137`.

## 🛡️ Security Boundaries

### Boundary 1: UI ↔ Web Content

**Separation:**
- Header browser: Trusted React UI
- Tab browsers: Untrusted web content

**Communication:**
- ❌ NO direct JavaScript access between them
- ✅ Communication only via CEF process messages
- ✅ Main process mediates all communication

**Security:**
- ✅ Malicious website can't modify UI
- ✅ Malicious website can't intercept wallet button
- ✅ A malicious page cannot **paint** a fake modal. Overlays are separate WS_POPUP / layered /
  topmost windows living outside the page's paint area, so page pixels can never occupy them.
- ⚠️ **But a malicious page CAN summon the genuine modal.** `overlay_show_brc100_auth`
  (`simple_handler.cpp`) reads `domain`, `type`, `method`, `endpoint` and `body` straight off the IPC
  argument list, stores them as the pending auth request, and opens the **real** BRC-100 auth
  overlay. `role_` is logged, never checked. `overlay_show_wallet` is equally ungated, and external
  pages can reach both because `window.cefMessage.send` is injected on every context.
  **Treat modal-summoning phishing as an open vector, not a solved one.**

### Boundary 2: Browser ↔ Wallet Daemon

**Separation:**
- CEF browsers: JavaScript execution environment
- Rust wallet backend: Wallet operations

**Communication:**
- ✅ HTTP requests to `127.0.0.1:31301` (`31401` when `HODOS_DEV=1`) — see `PortConfig.h` /
  `main.rs :: wallet_port`. There is **no** `:3301`.
- ⚠️ **Two** browser→wallet paths, not one:
  1. The `CefResourceRequestHandler` pipeline via `HttpRequestInterceptor`.
  2. The Phase 2.5 `wallet_call` process-message bridge, which never enters that pipeline.
- ⚠️ Two documented interception exemptions:
  - Trusted internal overlay roles (`wallet`, `wallet_panel`, `settings`, `backup`) hitting the
    wallet host:port return `nullptr` and use CEF's native network stack.
  - Internal origins on the IPC bridge short-circuit to `runIpcCallDirect` with no engine gate.
  Both are still subject to the Rust-side trust check, which C++ deliberately leaves unstamped for
  genuinely internal callers.
- ✅ **The decision gate is in Rust, not C++.** The permission / auto-approve decision engine is
  `rust-wallet/crates/hodos_permission_engine/` (`decide()` in `src/lib.rs`, cascade in
  `src/matrix_c.rs`), wrapped by `rust-wallet/src/permission_service/` and wired as universal Actix
  middleware in `rust-wallet/src/main.rs`. The C++ `PermissionEngine` and `SessionManager` were
  **deleted** in Phase 2.6-H; C++ now builds partial context, forwards it, and renders whichever
  modal Rust asks for. A C++ bypass therefore cannot bypass the decision.
- ✅ Domain trust enforced before the route handler runs
- ✅ User approval required for sensitive operations
- ✅ Default spending limits: **$1.00 per transaction, $10.00 per session** (100 / 1000 USD cents)

**Security:**
- ✅ **Signing keys never leave the Rust process. No EC private key is ever returned to JavaScript,
  and no signing happens there. The BIP39 recovery phrase is the one deliberate exception: it is
  shown once at wallet creation so the user can record it, and thereafter only through PIN
  re-verification in the wallet overlay. It is never reachable from web content — the wallet's CORS
  allowlist admits only Hodos's own local UI origins.** (The mnemonic's transit through the C++
  browser process on those two flows is detailed once, under "Process Isolation Benefits / Rust
  Wallet" above.)
- ✅ Transaction signing in separate process
- ✅ Domain-based access control
- ✅ HTTP-only communication (no shared memory exploits)

### Boundary 3: Tab ↔ Tab

**Separation:**
- Each tab: Own render process
- Each tab: Own V8 context

**Communication:**
- ❌ Tabs CANNOT communicate directly
- ✅ Can only communicate via main process
- ✅ No shared memory between tabs

**Security:**
- ✅ Tab 1 can't read Tab 2's cookies/localStorage
- ✅ Tab 1 can't intercept Tab 2's HTTP requests
- ✅ Tab 1 can't steal Tab 2's BRC100 session
- ✅ Complete isolation between websites

## 🔒 Current Security Strengths

### 1. Process Isolation ✅

**What You Have:**
- ✅ UI in separate process from web content
- ✅ Each overlay in own process
- ✅ Wallet operations in Rust wallet
- ✅ No shared memory between security boundaries

**Attack Surface:**
- ❌ Malicious website can't access wallet directly
- ❌ Malicious website can't modify UI
- ❌ Compromised tab can't affect other tabs

### 2. HTTP Request Interception ✅

**What You Have:**
- ✅ Wallet-bound HTTP requests go through the CEF interceptor (exemptions documented under
  Boundary 2)
- ✅ Domain trust check before processing
- ✅ User approval for new domains
- ✅ Wallet endpoints only accessible from approved domains

**Contract:** `HttpRequestInterceptor :: isWalletEndpoint` is the single route table that decides
whether a URL is wallet-bound. **New wallet endpoints go through that table, never around it.** The
match set includes non-obvious entries — Socket.IO connections and `.well-known/auth` are routed to
the wallet, not just the wallet ports — so adding a match has security consequences.

Owner: `cef-native/src/core/CLAUDE.md` (interception flow), `cef-native/CLAUDE.md`
(`HttpRequestInterceptor.cpp` responsibilities).

### 3. API Injection Control ✅

**What You Have:**
- ✅ The `hodosBrowser` API is injected per browser, branched on that browser's role
- ✅ Each browser gets its own injection into its own V8 context
- ✅ No global shared API object across browsers
- ⚠️ **`window.cefMessage.send` is the exception** — it is injected into *every* context, external
  pages included, and the receiving dispatch performs no origin/role/frame check (see Boundary 1)

**Contract:** injection is driven per browser from the load-complete path, so each browser's API
surface is a function of its role — that role branching is the mechanism that keeps the privileged
API off untrusted pages. Owner: `cef-native/src/handlers/CLAUDE.md`.

### 4. Domain Permission System ✅

**What You Have:**
- ✅ Persistent domain permissions (`domain_permissions` table, SQLite)
- ✅ Checked before processing wallet requests
- ✅ User approval modal for new domains
- ✅ Domain extracted from main frame URL
- ✅ Decision logic lives in Rust (`hodos_permission_engine`), enforced as universal middleware;
  C++ holds only a cache (`DomainPermissionCache`) and the modal rendering

> ⚠️ Because C++ caches permission state, **any change to domain permissions must fire the
> cache-invalidate IPC** or the C++ cache serves stale grants.

Owner: `rust-wallet/src/database/CLAUDE.md` (repo + schema),
`cef-native/src/core/CLAUDE.md` (cache).

## ⚠️ Current Security Gaps

### 1. Ungated IPC surface ⚠️

**Missing:**
- `window.cefMessage.send` is injected into every V8 context, external pages included
- `OnProcessMessageReceived` dispatches every message name with no origin, role or frame check
- Consequences already identified: `overlay_show_brc100_auth` and `overlay_show_wallet` can be
  summoned by any page with attacker-controlled parameters (Boundary 1)

**Recommendation:**
- Gate the dispatch on caller role / origin, at minimum for the overlay-summoning and tab-lifecycle
  message names

### 2. No Content Security Policy (CSP) ⚠️

**Missing:**
- No CSP headers enforced
- Websites can include any external scripts
- No XSS protection beyond browser defaults

**Recommendation:**
- Consider adding CSP headers in HTTP interceptor
- Block inline scripts on approved domains
- Restrict external script sources

### 3. No Request Size Limits ⚠️

**Missing:**
- Request **size** limits in the HTTP interceptor — a memory-exhaustion surface

**Already covered — do not re-file as a gap:**
- ✅ **Timeouts already exist on every interceptor-originated call.** WinHTTP send/receive/connect
  are capped at 3 s, `FETCH_TIMEOUT_MS` is 3000, and `handleHttpTimeout()` returns a
  `{"error":"Wallet request timeout"}` body (`HttpRequestInterceptor.cpp`). `SyncHttpClient.h`
  defaults to 5 s, with a 120 s variant for large transfers.
- ✅ Pending permission modals are capped at 600 s (`kPromptAuthTimeoutMs`).

**Recommendation:**
- Add request **body-size** caps in the HTTP interceptor
- Rate limiting per domain

*The open item here is body-size caps, not timeouts.*

## 🎯 Why Process-Per-Tab

Tabs are implemented (one `CefBrowser` + `SimpleHandler` per tab, role `tab_{id}`). This section
records **why** the shipped design is the one it is, so the constraint survives future refactors.

### Shared-context tabs (NEVER DO THIS)

```
❌ BAD APPROACH:
┌─────────────────────────────────────┐
│    Single Web-Content Process       │
│  - Tab 1: peerpay.com  ─┐           │
│  - Tab 2: malicious.com ├─ Same V8  │
│  - Tab 3: thryll.com   ─┘           │
│                                     │
│  All tabs share JavaScript context! │
│  Malicious site can access others!  │
└─────────────────────────────────────┘
```

**Risks:**
- ❌ Tab 2 can read Tab 1's cookies/localStorage
- ❌ Tab 2 can intercept Tab 1's wallet API calls
- ❌ Tab 2 can steal Tab 3's BRC100 session
- ❌ **CRITICAL SECURITY VULNERABILITY**

**Verdict**: ❌ **NEVER DO THIS FOR BITCOIN WALLET BROWSER**

### Process-per-tab (the shipped design)

```
✅ GOOD APPROACH:
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   Tab 1     │  │   Tab 2     │  │   Tab 3     │
│  Process    │  │  Process    │  │  Process    │
│             │  │             │  │             │
│ peerpay.com │  │malicious.com│  │ thryll.com  │
│ Own V8      │  │ Own V8      │  │ Own V8      │
│ Isolated    │  │ Isolated    │  │ Isolated    │
└─────────────┘  └─────────────┘  └─────────────┘
       │                │                │
       └────────────────┴────────────────┘
                        │
                        ▼
              ┌──────────────────┐
              │ HTTP Interceptor │
              │ Domain Permissions │
              └──────────────────┘
                        │
                        ▼
              ┌──────────────────┐
              │   Rust Wallet      │
              │   Wallet Ops     │
              └──────────────────┘
```

**Security:**
- ✅ Tab 2 can't access Tab 1's memory
- ✅ Tab 2 can't steal Tab 1's sessions
- ✅ Tab crash doesn't affect other tabs
- ✅ Complete isolation between websites

**Verdict**: ✅ **REQUIRED FOR SECURE BITCOIN WALLET BROWSER**

## 📋 Standing Tab Security Requirements

These are invariants for the tab system, not a build checklist. Any change to tab lifecycle,
session handling or auth routing must preserve all of them:

- **Process-per-tab architecture** (like Chrome/Brave) — one `CefBrowser` + `SimpleHandler` per tab
- **Tab-specific session management** (track which tab is authenticated)
- **UTXO locking** (prevent double-spend from concurrent tabs)
- **Auth request queuing** (handle multiple simultaneous auth requests)
- **Tab context in messages** (know which tab sent the request)
- **Tab-specific domain tracking** (each tab has its own domain context)
- **Proper cleanup on tab close** (release sessions, locks, resources)

## 🎓 Summary

### What holds

- ✅ Process isolation between UI and web content
- ✅ Process-per-tab; complete isolation between websites
- ✅ Each overlay in its own browser + V8 context
- ✅ Wallet in a separate Rust process, with the permission decision engine on the Rust side
- ✅ Wallet reachable only over localhost, behind a four-entry CORS allowlist of Hodos's own UI
  origins
- ✅ **Signing keys never leave the Rust process. No EC private key is ever returned to JavaScript,
  and no signing happens there. The BIP39 recovery phrase is the one deliberate exception: it is
  shown once at wallet creation so the user can record it, and thereafter only through PIN
  re-verification in the wallet overlay. It is never reachable from web content — the wallet's CORS
  allowlist admits only Hodos's own local UI origins.**
- ✅ Wallet / BRC-100 work independently per tab

### What does not hold — read before citing this document as an all-clear

- ⚠️ The `cefMessage` IPC surface is ungated: any page can send any message name, including
  `overlay_show_brc100_auth` / `overlay_show_wallet` with attacker-supplied parameters
- ⚠️ Overlays share one origin, one request context and one storage namespace; "Settings can't reach
  wallet state" is not true and was never true
- ⚠️ Overlays are keep-alive, so V8/React state persists across open/close cycles
- ⚠️ On a machine where the user never set a PIN, the BIP39 phrase is stored in plaintext in the
  `mnemonic` column on every platform; the OS-level auto-unlock copy is a separate, best-effort
  column and does not exist at all on Linux
- ⚠️ No CSP enforcement; no request body-size caps
