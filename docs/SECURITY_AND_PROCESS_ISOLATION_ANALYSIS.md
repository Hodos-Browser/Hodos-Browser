# Security & Process Isolation Analysis

**Date**: October 9, 2025 (Last Updated: 2026-09-30)
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
│  - Mediates all renderer IPC (local processes can reach the  │
│    wallet port directly; see Known issues)                   │
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
│ Roles (14): "settings", "settings_menu", "wallet",           │
│        "brc100auth", "notification", "omnibox", "menu",      │
│        "cookiepanel", "downloadpanel", "profilepanel", …     │
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
│ - Spawned by C++. On Windows it is held in a Job Object so   │
│   it dies with us                                            │
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
| Windows | PIN-encrypted (PBKDF2 + AES-GCM, `rust-wallet/src/crypto/pin.rs`) **if a PIN was set** — otherwise plaintext | DPAPI blob in a separate `mnemonic_dpapi` column (`database/wallet_repo.rs`). Mandatory: the write is tried up to 5 times (`dpapi_encrypt_retrying`), and if it still fails the wallet is not created |
| macOS | Same PIN rule | OS Keychain (service `HodosBrowser`/`HodosBrowserDev`, account `wallet-mnemonic`, `crypto/dpapi.rs`); the DB column holds only an 8-byte sentinel |
| Linux / other | Same PIN rule | **None** — `crypto/dpapi.rs` returns `Err` on these targets. Because the credential-store write is mandatory, wallet creation and import fail on Linux today |

> ⚠️ At least one wallet-creation path passes `pin = None`
> (`rust-wallet/src/handlers.rs :: create_wallet_from_existing_mnemonic` call site), which stores the
> phrase in plaintext in the `mnemonic` column. That is a real at-rest exposure on a machine where
> the user never set a PIN.

### Process Isolation Benefits

**Header Browser (React UI):**
- ✅ Isolated from web content
- ✅ Cannot be compromised by malicious websites
- ⚠️ Trusted UI, but it shares an origin and storage with any Hodos page a tab can navigate to
  (`cef-native/include/core/IpcAuth.h` documents the self-navigation case)
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
  pages (`simple_render_process_handler.cpp :: OnContextCreated`). `OnProcessMessageReceived` now
  resolves the calling frame's origin and refuses every message name from a non-internal origin
  except four (`wallet_call`, `cosmetic_class_id_query`, `find_result_js`, `qr_found`), then limits
  the grant/approve/reveal messages to the approval-overlay roles (`simple_handler.cpp ::
  IpcMessageAllowedFromWebPage`, `include/core/IpcAuth.h`). Internal origins, which include any page
  on `localhost` or `127.0.0.1`, pass the first gate. See "Current Security Gaps" below.

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
  Only the settings and BRC-100 auth (`brc100auth`) overlays destroy and recreate. **A page that
  needs a clean slate must reset
  itself explicitly** — the notification overlay does exactly this via `window.showNotification()`.

**Rust Wallet:**
- ✅ Separate process = can't be directly memory-exploited from web
- ✅ Only reachable over the localhost HTTP API — bound to `127.0.0.1`, never a routable interface —
  and browser-originated calls are further narrowed by the CORS allowlist below
- ✅ Domain trust check enforced in Rust on every request that carries a requesting domain (universal
  Actix middleware). It depends on C++ classifying the caller: a request C++ treats as internal arrives
  without the header and skips the gate (see Known issues and scheduled fixes)
- ✅ **Signing keys never leave the Rust process. No EC private key is ever returned to JavaScript,
  and no signing happens there. The BIP39 recovery phrase is the one deliberate exception: it is
  shown once at wallet creation so the user can record it, and thereafter only through PIN
  re-verification in the wallet overlay (for wallets that have a PIN; see "Known issues"). It is never
  reachable from web content — the wallet's CORS
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
   for wallet host:port URLs when `role_` is `wallet`, `wallet_panel` or `settings` (no browser is
   created with the `wallet_panel` role today) —
   "trusted overlay direct wallet request, bypassing all handlers" — so CEF's native network stack
   carries them. This exists to avoid `CefURLRequest` forwarding problems on macOS.
2. **Internal-origin direct dispatch.** `HttpRequestInterceptor.cpp :: HandleIpcWalletCall`
   short-circuits internal origins to `runIpcCallDirect`, which deliberately sends the request
   **without** the `X-Requesting-Domain` header, so Rust's `check_domain_approved` classifies the
   caller as internal and allows it.

**Consequence to hold in mind:** no permission-engine gate and no domain gate applies to a call
originating from Hodos's own UI. Endpoint-level checks still apply where they exist — e.g.
`/wallet/reveal-mnemonic` re-verifies the PIN inside Rust regardless of caller, for wallets that have
a PIN. A wallet without one returns the phrase with no check, and there is no limit on PIN attempts
(see "Known issues"). **The security of
this boundary rests entirely on nothing untrusted being able to execute at the internal origin.**

### The wallet CORS allowlist

`rust-wallet/src/main.rs` wraps the Actix app in a CORS layer admitting six origins:

```
http://127.0.0.1:5137     http://localhost:5137
http://127.0.0.1          http://localhost
http://127.0.0.1:31301    http://127.0.0.1:31401
```

with `allow_any_method()`, `allow_any_header()`, `block_on_origin_mismatch(true)` and a 3600 s
preflight max-age. The first four are Hodos's own local UI origins — the Vite dev/UI server and bare
localhost. The last two are the wallet's own base URLs (release and dev), which Chromium stamps as the
`Origin` on requests the C++ interceptor re-issues on a dApp's behalf. `block_on_origin_mismatch(true)`
makes a mismatched origin end the request with a 400 before any handler runs, so a blocked request
also cannot write. No internet origin is on this list, so a web page's own `fetch()` to the wallet
port is refused. The portless `http://localhost` and `http://127.0.0.1` entries would also match a
local site served on port 80. CORS is a browser-enforced control, so it constrains web content
specifically — it is not a
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
- ✅ A malicious page cannot draw into the overlay windows. Overlays are separate WS_POPUP / layered /
  topmost windows living outside the page's paint area, so page pixels can never occupy them.
- ⚠️ A page can still draw a look-alike modal inside its own page area.
- ⚠️ `overlay_show_brc100_auth` (`simple_handler.cpp`) reads `domain`, `type`, `method`, `endpoint`
  and `body` straight off the IPC argument list, stores them as the pending auth request, and opens
  the **real** BRC-100 auth overlay. Internet pages are refused: neither it nor `overlay_show_wallet`
  is on the four-name web-page allowlist. Internal origins are not refused, and they include any page
  on `localhost` or `127.0.0.1`.

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
  - Trusted internal overlay roles (`wallet`, `wallet_panel`, `settings`) hitting the
    wallet host:port return `nullptr` and use CEF's native network stack.
  - Internal origins on the IPC bridge short-circuit to `runIpcCallDirect` with no engine gate.
  Both are still subject to the Rust-side trust check, which C++ deliberately leaves unstamped for
  genuinely internal callers.
- ✅ **The decision gate is in Rust, not C++.** The permission / auto-approve decision engine is
  `rust-wallet/crates/hodos_permission_engine/` (`decide()` in `src/lib.rs`, cascade in
  `src/matrix_c.rs`), wrapped by `rust-wallet/src/permission_service/` and wired as universal Actix
  middleware in `rust-wallet/src/main.rs`. The C++ `PermissionEngine` and `SessionManager` were
  **deleted** in Phase 2.6-H; C++ now builds partial context, forwards it, and renders whichever
  modal Rust asks for. C++ still decides whether a call is internal or external: it omits
  `X-Requesting-Domain` for internal origins, and Rust applies no gate when that header is absent
  (`rust-wallet/src/main.rs :: domain_trust_mw`). A misclassification in C++ therefore skips the Rust
  gates.
- ✅ Domain trust enforced before the route handler runs
- ✅ User approval required for sensitive operations
- ✅ Spending limits: the user sets per-transaction and per-session limits for new sites, and adjusts
  them per site

**Security:**
- ✅ **Signing keys never leave the Rust process. No EC private key is ever returned to JavaScript,
  and no signing happens there. The BIP39 recovery phrase is the one deliberate exception: it is
  shown once at wallet creation so the user can record it, and thereafter only through PIN
  re-verification in the wallet overlay (for wallets that have a PIN; see "Known issues"). It is never
  reachable from web content — the wallet's CORS
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
- Hodos adds no tab-to-tab channel. Same-site tabs can still use BroadcastChannel and localStorage,
  as in any Chromium browser
- ✅ No shared memory between tabs

**Security:**
- ✅ Tab 1 can't read Tab 2's cookies/localStorage when the two are on different sites. Same-site
  tabs share cookies and storage
- ✅ Tab 1 can't intercept Tab 2's HTTP requests
- ⚠️ BRC-100 authentication and domain trust are per domain, so same-site tabs share them. Only the
  session spend counters are per tab
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
- ✅ For internet origins, wallet endpoints are reachable only from approved domains. Pages on
  `localhost` or `127.0.0.1` and local processes count as internal (see "Known issues")

**Contract:** on the HTTP path, `HttpRequestInterceptor :: isWalletEndpoint` is the single route
table that decides whether a URL is wallet-bound. The `wallet_call` IPC bridge does not use it: the
page supplies the endpoint, and it can name any wallet path. **New wallet endpoints go through that
table, never around it.** The
match set includes non-obvious entries — Socket.IO connections and `.well-known/auth` are routed to
the wallet, not just the wallet ports — so adding a match has security consequences.

Owner: `cef-native/src/core/CLAUDE.md` (interception flow), `cef-native/CLAUDE.md`
(`HttpRequestInterceptor.cpp` responsibilities).

### 3. API Injection Control ✅

**What You Have:**
- ✅ The `hodosBrowser` API is injected per browser, branched on the page URL
- ✅ Each browser gets its own injection into its own V8 context
- ✅ No global shared API object across browsers
- ⚠️ **`window.cefMessage.send` is the exception** — it is injected into *every* context, external
  pages included. The receiving dispatch refuses every message name from an internet origin except
  four (see "Process Isolation Benefits")

**Contract:** injection is driven per browser from the load-complete path, so each browser's API
surface is a function of the URL it loads — that URL branching is the mechanism that keeps the
privileged API off untrusted pages. Owner: `cef-native/src/handlers/CLAUDE.md`.

### 4. Domain Permission System ✅

**What You Have:**
- ✅ Persistent domain permissions (`domain_permissions` table, SQLite)
- ✅ Checked before processing wallet requests
- ✅ User approval modal for new domains
- ✅ Domain extracted from the calling frame's URL, falling back to ancestor frames and then the main
  frame
- ✅ Decision logic lives in Rust (`hodos_permission_engine`), enforced as universal middleware;
  C++ holds only a cache (`DomainPermissionCache`) and the modal rendering

> ⚠️ Because C++ caches permission state, **any change to domain permissions must fire the
> cache-invalidate IPC** or the C++ cache serves stale grants. Rust re-checks trust on every request,
> so a stale cache affects only the branches C++ still gates itself, such as the BRC-100 auth-handshake
> modal on the HTTP path and the BRC-121 (Simple 402 Payments) path.

Owner: `rust-wallet/src/database/CLAUDE.md` (repo + schema),
`cef-native/src/core/CLAUDE.md` (cache).

## ⚠️ Current Security Gaps

### 1. IPC surface open to internal origins ⚠️

**In place:**
- `OnProcessMessageReceived` resolves the calling frame's origin and refuses every message name from
  an internet origin except `wallet_call`, `cosmetic_class_id_query`, `find_result_js` and `qr_found`
- The grant/approve/reveal messages are accepted only from the approval-overlay roles
  (`include/core/IpcAuth.h`), so a tab that navigates itself to a Hodos page cannot send them

**Missing:**
- `window.cefMessage.send` is still injected into every V8 context, external pages included
- Internal origins, which include any page on `localhost` or `127.0.0.1`, pass the origin gate and
  can send any other message name, including `overlay_show_brc100_auth` and `overlay_show_wallet`

**Recommendation:**
- Narrow "internal" to the first-party UI origin (scheduled; see "Known issues")

### 2. No Content Security Policy (CSP) ⚠️

**Missing:**
- Hodos's own pages send `Content-Security-Policy: frame-ancestors 'self'` and
  `X-Frame-Options: SAMEORIGIN` (`include/core/LocalFileResourceHandler.h`), but no `script-src`
- Websites can include any external scripts
- No XSS protection beyond browser defaults

**Recommendation:**
- Consider adding CSP headers in HTTP interceptor
- Block inline scripts on approved domains
- Restrict external script sources

### 3. No Request Size Limits in C++ ⚠️

**Missing:**
- Request **size** limits in the HTTP interceptor — a memory-exhaustion surface

**Already covered — do not re-file as a gap:**
- ✅ Rust caps request bodies at 10 MB for JSON (`web::JsonConfig`) and 100 MB for raw bytes
  (`web::PayloadConfig`) in `rust-wallet/src/main.rs`.
- ✅ **Timeouts already exist on every interceptor-originated call.** WinHTTP send/receive/connect
  are capped at 3 s on most calls and 1 s on some, `FETCH_TIMEOUT_MS` is 3000, and
  `handleHttpTimeout()` returns a `{"error":"Wallet request timeout"}` body
  (`HttpRequestInterceptor.cpp`). The IPC path calls `SyncHttpClient` with 30 s; the HTTP path
  extends its ceiling to 120 s for `/wallet/recover` and 300 s for certificate acquire/prove.
  `SyncHttpClient.h` defaults to 5 s, with a 120 s variant for large transfers.
- ✅ Pending permission modals are capped at 600 s (`kPromptAuthTimeoutMs`).

**Recommendation:**
- Add request **body-size** caps in the HTTP interceptor
- Rate limiting per domain

*The open item here is body-size caps, not timeouts.*

## 🗓 Known issues and scheduled fixes

Status re-checked against the code for `v0.4.0-beta.7` (16 October 2026). Each item has a fix
planned for a later release. Described at the level of the mechanism, not the means of using it.
BRC-100 is the Unified, Vendor-Neutral, Unchanging, and Open BSV Blockchain Standard
Wallet-to-Application Interface.

### Identity and privacy

**Local callers without a requesting-domain header are trusted as the wallet UI.** A request to the
wallet from the local machine that does not name a requesting site is treated as Hodos's own UI and
skips the permission gates, and pages served from `localhost` count as Hodos's own UI. The fix adds a
per-launch secret and treats only the first-party UI as internal.

**The surface an approved site can reach is a denylist.** Once a site is approved, it can call any
wallet route that is not on the first-party-only list (permissions, session state, reveal, settings,
delete, backup, recover and a few others). The fix replaces the list with an allowlist.

### Wallet

**A Centbee import creates a wallet with no PIN**, so its recovery phrase has no PIN protection. The
wallet panel no longer offers the import; the issue applies to wallets already created that way.

**Payments under one cent bypass "Always notify".** Amounts are compared in whole cents, so a payment
worth less than one cent does not exceed a limit of zero and goes through without a prompt.

**`verifyLegacyMessage` ignores `identityKey`.** The legacy helper's answer does not depend on whose
signature the caller asked about.

**Secrets in memory are not zeroized.** The cached recovery phrase and keys derived from it are freed
without being overwritten.

### Peer authentication

**BRC-103 (Peer-to-Peer Mutual Authentication and Certificate Exchange Protocol) server signatures
are not verified.** The client the wallet uses for MessageBox takes the server's identity key as
asserted.

### Current behaviour

- **macOS runs without the Chromium sandbox.** The Windows build runs with it.
- **The PIN is 4 digits, with no attempt limit.** It is stretched with PBKDF2-HMAC-SHA256 at
  600,000 iterations.
- **The wallet auto-unlocks from the OS keystore and does not lock again** until the process exits.
- **Only the recovery phrase is encrypted at rest.** The rest of the wallet database (transactions,
  outputs, certificates, permissions, PeerPay history) is not.

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
- **Per-tab session spend counters** (authentication and domain trust are per domain, not per tab)
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
- ✅ Wallet reachable only over localhost, behind a six-entry CORS allowlist of Hodos's own UI
  origins and the wallet's own base URLs, with mismatched origins stopped before any handler runs
- ✅ **Signing keys never leave the Rust process. No EC private key is ever returned to JavaScript,
  and no signing happens there. The BIP39 recovery phrase is the one deliberate exception: it is
  shown once at wallet creation so the user can record it, and thereafter only through PIN
  re-verification in the wallet overlay (for wallets that have a PIN; see "Known issues"). It is never
  reachable from web content — the wallet's CORS
  allowlist admits only Hodos's own local UI origins.**
- ✅ Session spend counters are kept per tab (BRC-100 authentication and domain trust are per domain)

### What does not hold — read before citing this document as an all-clear

- ⚠️ The `cefMessage` IPC surface is gated for internet pages but open to internal origins: any page
  on `localhost` or `127.0.0.1` can send `overlay_show_brc100_auth` / `overlay_show_wallet` with
  parameters it chooses
- ⚠️ Overlays share one origin, one request context and one storage namespace; "Settings can't reach
  wallet state" is not true and was never true
- ⚠️ Overlays are keep-alive, so V8/React state persists across open/close cycles
- ⚠️ On a machine where the user never set a PIN, the BIP39 phrase is stored in plaintext in the
  `mnemonic` column on every platform; the OS-level auto-unlock copy is mandatory on Windows and
  macOS, and because Linux has no credential store, wallet creation fails there today
- ⚠️ No `script-src` CSP; no request body-size caps in C++
- ⚠️ The items under "Known issues and scheduled fixes"
