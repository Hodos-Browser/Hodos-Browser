# HodosBrowser — Architecture & Project Overview

**Last Updated**: 2026-09-30 (accuracy pass against the code at `4d699ab`)

> This document consolidates the former PROJECT_OVERVIEW.md, ARCHITECTURE.md, and WALLET_ARCHITECTURE.md into a single reference.
>
> **Scope rule**: this document carries **shape, contracts and pointers**. Layer inventory — file
> rosters, handler catalogues, component tables, schema table lists — lives in the per-directory
> `CLAUDE.md` files and is not duplicated here. Dated sprint/release status lives in root
> `CLAUDE.md` ("Active sprint status").

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [C++ CEF Shell](#2-c-cef-shell)
3. [Rust Wallet Backend](#3-rust-wallet-backend)
4. [React Frontend](#4-react-frontend)
5. [Communication Patterns](#5-communication-patterns)
6. [Security Architecture](#6-security-architecture)
7. [Data Storage](#7-data-storage)
8. [Background Services](#8-background-services)
9. [BRC-100 Protocol](#9-brc-100-protocol)
10. [Status & Roadmap](#10-status--roadmap)

---

## 1. Architecture Overview

Three layers with strict separation:

```
React Frontend (Port 5137)
    | window.hodosBrowser.*  (IPC bridge, not direct fetch)
    v
C++ CEF Shell (CEF 150)
    | HTTP interception -> 127.0.0.1:31301 for wallet functions
    v
Rust Wallet Backend (127.0.0.1:31301)
    | Actix-web, SQLite (wallet.db)
    v
Bitcoin SV Blockchain (GorillaPool, Bitails, WhatsOnChain)
```

**Ports.** The wallet backend binds `127.0.0.1:31301` in a release build and `127.0.0.1:31401`
when `HODOS_DEV=1` (`rust-wallet/src/main.rs :: wallet_port`). The adblock engine is the same
split at 31302 / 31402. `cef-native/include/core/PortConfig.h` is the single source of truth on
the C++ side (`hodos::WalletPort()` / `hodos::WalletBaseUrl()`) — never hardcode either literal.

| Layer | Tech | Responsibility |
|-------|------|----------------|
| Frontend | React, Vite, TypeScript, MUI | UI, user interactions; performs no signing and never receives a derived EC private key |
| CEF Shell | C++17, CEF 150 (Chromium 150.0.7871.187) | Browser engine, V8 injection, HTTP interception; browser data (history, bookmarks) |
| Wallet | Rust, Actix-web, SQLite | Crypto, signing, keys, BRC-100 protocol |

The exact key-material boundary — including the one deliberate exception — is stated in
[§6.4 Key Security Properties](#64-key-security-properties).

**Process-per-overlay**: each overlay panel runs as its own CEF subprocess with an isolated V8
context, rather than as a panel inside the header browser. Full overlay roster:
`cef-native/CLAUDE.md` and `frontend/src/pages/CLAUDE.md`.

---

## 2. C++ CEF Shell

### 2.1 Process Architecture

The shell is a multi-process tree, not a fixed process count: a main browser process hosts the
header browser (React UI at port 5137), **one windowed CEF browser per open tab** for external web
content, and one browser per overlay HWND — so the renderer-process population scales with open
tabs and live overlays. The Rust wallet and the adblock engine are separate OS processes started
and supervised by the shell.

Process boundaries and what each one is trusted with are analysed in
[`SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md`](SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md) (this
folder), which owns the process map.

### 2.2 C++ Singletons

The shell keeps a small set of process-wide singletons for caches (domain trust, BSV price, wallet
status), cross-thread pending-request tracking, and window/tab/profile management. Full roster:
`cef-native/include/core/CLAUDE.md`.

Two facts that matter above the layer:

- **There is no C++ `SessionManager`.** It was deleted in Phase 2.6-H. Per-browser session
  spending, payment-rate and max-tx counters now live in Rust
  (`rust-wallet/src/permission_service/state.rs :: SessionCounters`, `record_spending`,
  `increment_payment_rate_counter`). Every remaining C++ mention is a past-tense comment.
- **There is no C++ auto-approve/permission decision engine.** The C++ `PermissionEngine` was
  deleted in the same phase. See [§6.3](#63-defense-in-depth).

### 2.3 SimpleHandler (CEF Client)

`simple_handler.cpp` is the CEF client object: it implements the browser-side handler interfaces
(lifecycle, display, load, request, context menu, dialog, keyboard, permission, download, find,
JS dialog) and dispatches IPC from the React overlays and the header. Exact interface list and IPC
message roster: `cef-native/include/handlers/CLAUDE.md` and `cef-native/src/handlers/CLAUDE.md`.

**It does not implement `CefResourceRequestHandler`** — it *vends* one from
`CefRequestHandler::GetResourceRequestHandler`. The implementations live in
`HttpRequestInterceptor` (`cef-native/include/core/HttpRequestInterceptor.h`) and
`CachedContentRequestHandler` (`cef-native/include/core/CachedContentResourceHandler.h`).

Context menus: all custom command IDs are in the `MENU_ID_USER_FIRST` range — CEF built-in IDs
auto-disable after `model->Clear()`.

### 2.4 HTTP Interception Flow

Interception starts at the handler-selection point, not at a resource-load callback:

```
Web request
  -> SimpleHandler::GetResourceRequestHandler        (selects a handler per request)
       - local frontend file serving
       - trusted-overlay direct bypass
       - PaidContentCache playback (CachedContentRequestHandler)
       - adblock
       - HttpRequestInterceptor
  -> HttpRequestInterceptor::GetResourceHandler
       - isWalletEndpoint match -> AsyncWalletResourceHandler
  -> AsyncWalletResourceHandler::Open()              (THIN PROXY, Phase 2.6-G)
       - convert sats -> USD cents via BSVPriceCache, inject X-Payment-* headers
       - forward every external origin to http://127.0.0.1:31301 (:31401 under HODOS_DEV=1)
         via CefURLRequest on the IO thread
       - except: no wallet -> NO_WALLET, wallet service unreachable -> WALLET_UNAVAILABLE
         (answered in C++, not forwarded)
       (the IPC bridge path, HandleIpcWalletCall, forwards with SyncHttpClient on
        TID_FILE_USER_BLOCKING instead — see §5.1)
  -> Rust answers, and Rust is authoritative
       200 -> pass through
       202 -> C++ opens the modal Rust named (domain_approval, manifest_connect_bundle,
              payment_confirmation, rate_limit_exceeded, cert disclosure, ...)
       403 -> blocked
  -> on an auto-approved payment: firePaymentSuccessIpc() -> gold-pill tab indicator
```

`OnBeforeResourceLoad` is **not** on this path — that callback belongs to the adblock handlers in
`AdblockCache.h`. BRC-121 `402 Payment Required` detection hangs off
`HttpRequestInterceptor::OnResourceResponse`.

`DomainPermissionCache` is still consulted inside `Open()`, but it no longer gates the forward to
Rust on this path. It survives for ancillary uses: the legacy BRC-100 auth-handshake modal branch
(matches `/brc100/auth/`, a path with no Rust route, so it has no practical effect), the BRC-121
(Simple 402 Payments) `402` pre-check, and the identity-key-disclosure lookup
(`GetDomainIdentityKeyDisclosureAllowed`) used by the cap modal. `IsInternalOrigin` is a plain
host match and does not use the cache. The cases where C++ withholds the forward are listed in
[§6.3](#63-defense-in-depth).

### 2.5 Notification Overlay

Keep-alive HWND pattern:
- HWND created once (pre-warmed during startup), reused via JS injection
- `window.showNotification(queryString)` for instant React state update
- `window.hideNotification()` + `SW_HIDE` to dismiss
- Prompt kinds are multiplexed through a single route, `/brc100-auth`, hosted in two windows (the
  `brc100auth` overlay and the keep-alive `notification` overlay): the URL carries `?type=<type>`
  and `BRC100AuthOverlayRoot.tsx` dispatches on it. Adding a prompt kind means adding a case, not a
  new HWND. Current type roster: `frontend/src/pages/CLAUDE.md`.
- Atomic `compare_exchange_strong` on timeout vs response to prevent double-fire crashes

### 2.6 Window Hierarchy (Windows)

```
Main Shell (g_hwnd)
    |-- Header (g_header_hwnd)          - WS_CHILD, React UI
    |-- Tab windows (one per tab)       - WS_CHILD, windowed CEF browser, only the active one visible
    |-- Overlay windows (one per panel) - WS_POPUP, layered, own V8 context
```

Tab windows are created by `TabManager::CreateTab` (HWND, then `CefBrowserHost::CreateBrowser`) and
parented to `g_hwnd`.

`g_webview_hwnd` is **legacy**: `WS_CHILD`, never given `WS_VISIBLE`, hosts no browser, and is
nulled on primary-window transfer. It is kept only for API compatibility — external content has not
lived there since the tab system landed.

Overlay HWND globals and their close/destroy semantics: `cef-native/CLAUDE.md` and root
`CLAUDE.md` ("Overlay Lifecycle & Close Prevention").

### 2.7 macOS Port

`cef_browser_shell_mac.mm` is the macOS entry point: NSWindow/NSView hierarchy, `NSWindow`-based
overlays with `NSWindowDelegate` close handling, and event forwarding, alongside `TabManager_mac.mm`,
`WindowManager_mac.mm` and `my_overlay_render_handler.mm`. Build system supports macOS via CMake.
Per-file Windows/macOS parity tables: `cef-native/src/handlers/CLAUDE.md` and
`cef-native/src/core/CLAUDE.md`. Port history: `archived-docs/Final-MVP-Sprint/macos-port/`.

---

## 3. Rust Wallet Backend

### 3.1 AppState (`src/main.rs`)

Shared state accessible to all HTTP handlers. Architecturally load-bearing fields:

| Field | Type | Purpose |
|-------|------|---------|
| `database` | `Arc<Mutex<WalletDatabase>>` | SQLite connection (single writer) |
| `balance_cache` | `Arc<BalanceCache>` | In-memory balance with instant invalidation |
| `price_cache` | `Arc<PriceCache>` | BSV/USD price — WhatsOnChain -> CoinGecko -> MEXC, 300s TTL, SQLite-persisted last-known price |
| `fee_rate_cache` | `Arc<FeeRateCache>` | Mining fee rate from the ARC policy endpoint, 1-hour TTL, 1000 sat/KB fallback |
| `sync_status` | `Arc<RwLock<SyncStatus>>` | Wallet recovery/sync progress |
| `current_user_id` | `i64` | Active user ID (default: 1) |
| `shutdown` | `CancellationToken` | Graceful shutdown signal |
| `auth_sessions` | `Arc<AuthSessionManager>` | BRC-103/104 auth sessions — the manager privately holds a `Mutex<HashMap>` keyed `"identity_key:our_nonce"`, 24 h expiry |

`AppState` also carries service handles, secondary caches and the permission-service state that
backs the Rust permission engine. Full field list: `rust-wallet/src/main.rs :: AppState` and
`rust-wallet/src/CLAUDE.md`.

Two things are commonly assumed to be in `AppState` and are not:

- **BRC-33 message relay.** There is no `message_store` field. The in-memory `MessageStore` type
  still sits in `rust-wallet/src/message_relay.rs`, but the module is not declared in `main.rs` or
  `lib.rs` — it is not compiled, and was superseded by the SQLite-backed
  `database/message_relay_repo.rs` (`MessageRelayRepository`).
- **Two-phase signing state.** See [§3.5](#35-transaction-lifecycle).

### 3.2 Database Layer (`src/database/`)

SQLite with WAL mode, foreign keys enabled. Consolidated V1 schema for fresh databases; incremental
migrations for existing ones.

**Repository pattern** — each table group has its own `*Repository` over the shared SQLite
connection; `WalletDatabase` owns that connection (single writer, `Arc<Mutex<…>>` in `AppState`)
and runs migrations. New per-entity data extends an existing table group via a child table joined
by FK + `CASCADE` (the `cert_field_permissions` pattern), rather than a parallel top-level table.
The pattern is not exclusive: `handlers.rs` and `handlers/certificate_handlers.rs` also run SQL
directly on the connection. Full repository roster and model map: `rust-wallet/src/database/CLAUDE.md`.

**Money is proven, not assumed (beta.7, B5-T1-P3 + P5).** Every spend route chooses coins from one
index, `money_utxos`, kept by SQLite triggers, which admits only outputs the classification seam
(`src/classify.rs`) has stamped `change = 1`: the observed script is exactly the 25-byte P2PKH
template, worth more than 1 satoshi, paying a key this wallet signs with. Every ingest route calls
the seam after its write; anything else (a token, an inscription, a 1-sat coin, a coin whose script
has not been read from the chain) is **held**: never spent automatically, shown under the balance
(`/wallet/balance` `held`, `/wallet/held-outputs`), retried every sync tick, and promotable only by
the user ("Treat as money", first-party only) — and only a coin of more than 1 satoshi whose script
has not been read yet; a 1-sat coin may be a transferred NFT and is never promoted (B5-T2-P1b). Static gates `G13`/`G14` keep coin selection and
the stamp inside those two modules.

### 3.3 Cryptography (`src/crypto/`)

The crypto layer covers four concerns:

- **Derivation** — BRC-42 ECDH child key derivation (Type-42) with BRC-43 invoice numbers
  (`{securityLevel}-{protocolID}-{keyID}`), plus legacy BIP32 for recovery.
- **Signing** — SHA-256, HMAC-SHA256, ECDSA with BSV ForkID SIGHASH.
- **Encryption** — BRC-2 (AES-256-GCM over a BRC-42-derived key), used for certificate fields and
  for MessageBox/PeerPay payloads.
- **At-rest protection** — `dpapi.rs` has three platform arms: Windows DPAPI
  (`CryptProtectData` / `CryptUnprotectData`) and macOS Keychain Services are **both full
  implementations** (macOS is dev/prod-namespaced via `keychain_service()`); Linux/other is the only
  stub. `pin.rs` layers PIN-based AES-256-GCM + PBKDF2 (600K iterations) on top.

Note that `crypto/mod.rs` is module declarations only — key-derivation routing lives in
`database/helpers.rs`, and public-key computation in `crypto/keys.rs`. Full module roster:
`rust-wallet/src/crypto/CLAUDE.md`.

### 3.4 Key Derivation

`derive_key_for_output()` in `database/helpers.rs` is the main entry point for deriving the key
that signs a wallet **UTXO input**. On-chain backup outputs (`backup.rs ::
derive_onchain_backup_key`) and overlay PushDrop spends (`script/pushdrop.rs`) derive their keys
separately. It routes on the output's stored derivation metadata:

| `derivation_prefix` | `derivation_suffix` | `sender_identity_key` | Path |
|---------------------|---------------------|----------------------|------|
| `"2-receive address"` | `"{index}"` | `None` | BRC-42 self-derivation (standard) |
| `"bip32"` | `"{index}"` | `None` | Legacy BIP32 HD (`m/{index}`) |
| `NULL` | `NULL` | `None` | Master private key directly |
| any | any | `Some(pubkey)` | BRC-42 counterparty derivation |

Other signing and derivation paths do **not** go through it — `create_signature`, BRC-103 AuthFetch
and certificate CSR signing each derive independently from `get_master_private_key_from_db` +
`brc42::derive_child_private_key`. BRC-2 (Data Encryption and Decryption) goes through
`crypto/brc2.rs :: derive_symmetric_key`.

### 3.5 Transaction Lifecycle

```
createAction (build + select UTXOs)
    -> status: 'unsigned'
    -> inputs reserved: spendable = 0, spending_description = 'pending-{ts}'
       (spent_by is NULL at reservation time — no transactions row exists for the
        placeholder txid yet; rollback keys off spending_description)
    -> change/basket outputs created with spendable = 1
       (change is spendable and the balance is accurate immediately)

signAction (sign + broadcast)
    -> status: 'sending' -> 'unproven'
    -> proven_tx_req created
    -> Monitor acquires proof -> 'completed' (only once the proof verifies against the wallet's own headers)

Broadcast refused — the caller gets HTTP 400, BRC-100 code 5 (WERR_REVIEW_ACTIONS), never a 200:
  temporary (a provider timed out, was unreachable, answered 5xx or only weakly accepted):
    -> the SAME signed bytes retried, up to 3 attempts inside a 20 s budget
    -> status stays 'sending', inputs stay reserved; TaskSendWaiting keeps re-broadcasting
    -> TaskCheckForProofs rolls back (as below) once WoC, JungleBus and Bitails have all not
       seen it for 5 min; TaskUnFail restores it if it lands later
  permanent (some provider refused the tx itself and none may hold it):
    -> status: 'failed'
    -> ghost outputs DISABLED, not deleted
       (spendable = 0, spending_description = 'failed-tx-output', via disable_by_txid —
        so TaskUnFail can reverse a false failure through reenable_failed_outputs)
    -> reserved inputs restored (spendable = 1, spent_by = NULL, spending_description = NULL),
       then reconcile_missing_inputs re-marks any the chain shows as spent
    -> balance cache invalidated
```

Ghost outputs are only really `DELETE`d by startup stale-pending recovery.

The two-phase `createAction` -> `signAction` map is **not** an `AppState` field: it is the
file-scoped process-global `handlers.rs :: PENDING_TRANSACTIONS`
(`Lazy<StdMutex<HashMap<String, PendingTransaction>>>`), inserted during `createAction`, read by
`sign_action`, and removed once the transaction is fully signed. Anything that changes signing
inputs (e.g. BRC-100 top-level `lockTime` / `version`) must be set on the in-memory transaction
before signing, because both phases share that one object.

### 3.6 Wallet Security

- **DPAPI/Keychain auto-unlock**: Mnemonic stored twice — PIN-encrypted + OS-encrypted. Startup:
  try the OS blob first, auto-cache mnemonic on success — only if it is a valid phrase whose
  master public key equals `users.identity_key`; otherwise the wallet stays locked and the PIN
  unlock rewrites the OS entry.
- **PIN encryption**: AES-256-GCM with PBKDF2 (600K iterations). PIN used during create/recover.
- **DPAPI backfill**: On PIN unlock, the OS blob is stored for future auto-unlock.
- **Legacy wallets**: `pin_salt=NULL` -> plaintext auto-cached. PIN-protected without an OS blob ->
  locked until PIN.
- **Known issue — no-PIN wallets:** the Centbee import (`recover_external_wallet`) creates a wallet
  with no PIN, so its recovery phrase is stored in plaintext, and no API sets a PIN later.
  Since beta.7 (B5-T1-P6) the wallet panel no longer offers the Centbee import; the endpoint
  remains, unreachable from the UI, and the issue applies to wallets already created that way.
  `development-docs/0.4.0-beta.8/tickets/TICKET_centbee_import_creates_a_wallet_with_no_pin.md`.
- **Known issue — no zeroization:** the cached mnemonic (`cached_mnemonic`) and derived keys are never
  wiped from memory; they stay until process exit or wallet delete.
  `development-docs/0.4.0-beta.8/tickets/TICKET_wallet_secrets_in_memory_are_never_wiped.md`.

### 3.7 API Endpoints

`handlers.rs` is the HTTP surface of the wallet: wallet CRUD and status, the BRC-100 protocol
surface (auth, actions, signatures, HMACs, outputs, certificates, encryption, messages), domain and
sub-permission CRUD, BRC-121 `402` payment, PeerPay, price and sync. New wallet endpoints are added
to the C++ `isWalletEndpoint` route table so they go **through** interception rather than around it.

Full endpoint roster with handler names: `rust-wallet/src/CLAUDE.md`. Handler names in this repo
are not always the obvious ones — check the layer doc before assuming a name (`get_sync_status`,
`get_bsv_price`, `set_domain_permission`, `list_domain_permissions`, `check_cert_permissions`).

`send_transaction` is the **internal wallet-panel send path**. It wraps the BRC-100 `create_action`
path that dApps use: it builds a `createAction` body and calls `create_action` — worth knowing when
tracing a spend.

---

## 4. React Frontend

### 4.1 Application Structure

One React codebase, multiple CEF instances; the route determines context. `/` is the header
browser (navigation toolbar and toolbar icon buttons); every panel is its own route rendered in its
own overlay browser.

The BRC-100 prompt surface is a single route, `/brc100-auth`, multiplexed by a `type` query param —
domain approval, auth approval, payment confirmation, certificate disclosure and the keep-alive
notification overlay (`/brc100-auth?type=idle` when pre-warmed) all render
`BRC100AuthOverlayRoot.tsx`. (`brc100auth` and `notification`, unhyphenated, are C++ SimpleHandler
*role strings*, not URLs.)

Full route table: `frontend/src/CLAUDE.md`.

### 4.2 Key Components

Component roster and per-component responsibilities: `frontend/src/components/CLAUDE.md`,
`frontend/src/components/wallet/CLAUDE.md` and `frontend/src/pages/CLAUDE.md`.

The architectural constraint that governs this layer: **no new panels/menus/dropdowns go into
`MainBrowserView.tsx`** — every panel is an overlay in its own CEF subprocess. See root `CLAUDE.md`
("UI Architecture Rules").

### 4.3 Hooks

Hooks wrap the bridge and the wallet calls — `useHodosBrowser()` for browser-shell actions,
`useBalance()` for balance + BSV price, `useBackgroundBalancePoller()` for the 30s auto-refresh.
Full roster: `frontend/src/hooks/CLAUDE.md`.

### 4.4 Bridge (`initWindowBridge.ts`)

`window.hodosBrowser.navigation` and `window.hodosBrowser.overlay` are defined by native V8
handlers in the C++ render process (`simple_render_process_handler.cpp`, internal pages only).
`initWindowBridge.ts` supplies `cefMessage.send()` fallbacks for any of them that are missing.
Wallet calls take the separate IPC path described in [§5.1](#51-three-communication-paths).

---

## 5. Communication Patterns

### 5.1 Three Communication Paths

| Pattern | Direction | Mechanism | Used For |
|---------|-----------|-----------|----------|
| **CefURLRequest** (async) | C++ -> Rust | HTTP on IO thread | BRC-100 wallet endpoints (payment, auth, signing) |
| **SyncHttpClient / WinHTTP** (sync) | C++ -> Rust | Synchronous HTTP | Domain permission lookups, price cache, wallet status |
| **IPC bridge** | React -> C++ -> Rust | `window.hodosBrowser.wallet.*` / `window.__hodos_walletCall` -> `"wallet_call"` `CefProcessMessage` -> C++ HTTP | All first-party wallet UI operations (balance, send, backup) |

The first-party React wallet UI does **not** fetch Rust directly. Routing wallet calls through the
C++ bridge means C++ owns the wallet port (dev 31401 / prod 31301) — the frontend never knows it —
and each call is gated by an un-forgeable frame origin rather than by anything the page can claim.

### 5.2 IPC (C++ <-> React)

```
React -> cefMessage.send("command", data)
  -> CefProcessMessage to browser process
    -> simple_handler.cpp OnProcessMessageReceived
      -> dispatch by message name
```

Message-name roster and payload shapes: `cef-native/src/handlers/CLAUDE.md` and
`frontend/src/bridge/CLAUDE.md`.

---

## 6. Security Architecture

### 6.1 Process Isolation

- **Header browser**: Trusted React UI, isolated from web content
- **Tab browsers**: Untrusted web content, HTTP interception active, one browser per tab
- **Overlays**: Each in its own process with its own V8 context
- **Rust wallet**: Separate process, only reachable over loopback HTTP on the wallet port
- **Tab isolation**: Process-per-tab via CEF (Chromium's security model)

### 6.2 Domain Permission System

Three trust levels: **unknown** (show approval overlay), **approved** (evaluate the permission
cascade) and **blocked** (refused without a prompt).

Per-domain controls:

| Control | Unit |
|---------|------|
| Per-transaction spending limit | USD cents |
| Per-session spending limit | USD cents |
| Rate limit | requests/min |
| Max transactions per session | count |
| Certificate field disclosure | tracked per field |

The user sets the defaults applied to new sites (wallet settings, `settings.default_*` columns) and
adjusts each site's limits afterwards. This document states no default values, because the code
does not yet agree on them: the V1 schema and V12 backfill in
`rust-wallet/src/database/migrations.rs`, the V10 column defaults in the same file, the
`settings_repo` fallbacks, `wallet_settings_get` in `handlers.rs`, the C++ fallbacks in
`cef-native/src/core/HttpRequestInterceptor.cpp`, and the form defaults in
`frontend/src/components/DomainPermissionForm.tsx` carry different values.

"Always notify" in `DomainPermissionForm` zeros the per-transaction, per-session and
max-transactions limits, and leaves the rate limit — the cautious-user opt-in path.
**Known issue:** payments under one US cent truncate to 0 cents and pass a 0 limit without a
prompt, even with Always notify on.
`development-docs/0.4.0-beta.8/tickets/TICKET_always_notify_lets_sub_cent_payments_through_silently.md`.
Per-session counters reset on tab close, by design.

### 6.3 Defense in Depth

1. **C++ shell (thin proxy)** — forwards every external origin's wallet call to Rust regardless of
   cached trust level; converts satoshis to USD cents via `BSVPriceCache` and injects the
   `X-Payment-*` headers; opens whatever modal Rust names on a `202`; fires the gold-pill
   `payment_success_indicator` IPC on every auto-approved payment. C++ withholds the forward in
   these cases: no wallet (`NO_WALLET`), wallet service unreachable (`WALLET_UNAVAILABLE`), the
   BRC-121 `402` pre-check for a non-approved domain, and a legacy `/brc100/auth/` branch for an
   unknown domain (no Rust route has that path, so it has no practical effect).
2. **Rust domain-trust middleware** — authoritative on domain trust for every wallet endpoint:
   `200` allow / `202` prompt (`domain_approval` or `manifest_connect_bundle`) / `403` blocked.
   `check_domain_approved()` validates the `X-Requesting-Domain` header.
3. **Rust permission engine** — `rust-wallet/crates/hodos_permission_engine` (`decide()` in
   `src/lib.rs`, the Matrix C cascade in `src/matrix_c.rs`), wrapped by
   `rust-wallet/src/permission_service/` and wired as Actix middleware in `rust-wallet/src/main.rs`.
   It runs domain trust -> privacy perimeter -> scoped grants -> payment caps -> cert disclosure ->
   generic, and owns the per-tx / per-session / rate / max-tx-per-session counters.
4. **Rust spend check at build time** — `create_action` re-verifies the per-tx limit against the
   price cache before building the transaction.

The C++ `PermissionEngine` and `SessionManager` were deleted in Phase 2.6-H. C++ now builds partial
context, forwards, and renders the modal Rust asks for.

### 6.4 Key Security Properties

1. Signing keys never leave the Rust process. No EC private key is ever returned to JavaScript, and
   no signing happens there. The BIP39 recovery phrase is the one deliberate exception: it is shown
   once at wallet creation so the user can record it, and thereafter only through PIN
   re-verification in the wallet overlay. It is never reachable from web content — the wallet's CORS
   allowlist admits only Hodos's own local UI origins.
   **Known issue — loopback trust:** pages on `localhost:*` / `127.0.0.1:*` (`IsInternalOrigin`) and
   any local process that sends no `X-Requesting-Domain` header are treated as Hodos's own UI and
   skip the Rust gates. A fix is planned for a later release.
2. DPAPI/Keychain encryption for the mnemonic at rest
3. PIN encryption (AES-256-GCM + PBKDF2) as second layer. Wallets created by the Centbee import
   have no PIN (see [§3.6](#36-wallet-security)).
4. Parameterized SQL — every value is bound. `format!` inserts only placeholder lists (`IN ({})`),
   counts and internal table names, never caller-supplied values.
5. App-scoped identity keys on `/.well-known/auth` (BRC-103/104). They do not cover
   `getPublicKey({identityKey: true})`, which returns the master identity key once the site's
   identity-key disclosure is approved — and that box is pre-ticked in the connect prompt.
6. Atomic timeout handling — `compare_exchange_strong` prevents double-fire crashes
7. Privacy-perimeter calls (branch 2 of the Rust cascade) take precedence over scoped grants and
   payment caps. Only sensitive certificate fields prompt every time. Identity-key reveal goes
   silent once the site's persistent disclosure flag is set (pre-ticked at connect) or a session
   opt-in exists; key linkage can be opted into per session. There is no large-spend perimeter —
   large spends are governed by the payment caps.

---

## 7. Data Storage

### 7.1 File System Layout

| Platform | Root | Wallet DB | Browser Data |
|----------|------|-----------|--------------|
| Windows | `%APPDATA%/HodosBrowser/` | `wallet/wallet.db` | one directory per profile (`Default/` for the default profile): `HodosHistory`, `bookmarks.db`, cookies |
| macOS | `~/Library/Application Support/HodosBrowser/` | `wallet/wallet.db` | one directory per profile (`Default/` for the default profile) |

Dev builds use a separate root (`HodosBrowserDev/`) selected by `HODOS_DEV=1`, so a dev session can
never touch the installed app's database. See root `CLAUDE.md` ("Dev/Production Data Isolation").

### 7.2 Database Schema

Fresh databases are created from a single consolidated `create_schema_v1()`; existing databases
migrate incrementally up the migration ladder in `rust-wallet/src/database/migrations.rs`.

The schema is grouped roughly as: wallet/user identity, HD addresses, transactions and outputs
(with the basket/tag side tables), SPV proof records, certificates and their fields, permissions
(domain-level plus the protocol/basket/counterparty/cert-field child tables), and operational
tables (settings, sync state, monitor events, caches). A `permission_audit_log` table exists in the
schema (V20), but nothing writes to it, so it records no decisions.

Current table roster and migration ladder: `rust-wallet/src/database/CLAUDE.md`. **Do not change
the wallet DB schema without asking first** (root `CLAUDE.md`, invariant #2).

### 7.3 Browser Data (C++ Layer)

History and bookmarks managed by C++ singletons (`HistoryManager`, `BookmarkManager`) with their own
SQLite databases in each profile's own directory (`HodosHistory`, `bookmarks.db`; `Default/` for the
default profile). Cookies managed by CEF's built-in cookie manager.

---

## 8. Background Services

### 8.1 Monitor Pattern

The Monitor (`src/monitor/mod.rs`) runs as a single tokio task with a 30-second tick loop. Each
named task carries its own interval in `TaskSchedule` and covers one recovery or upkeep concern:
merkle-proof acquisition, crash recovery for stuck sends, failing abandoned transactions, reversing
false failures, cross-table status consistency, purging old records, pending-address UTXO sync,
PeerPay polling, and periodic maintenance jobs.

Uses `CancellationToken` for graceful shutdown. Before each task it probes the database with
`try_lock()` and skips the tick if a user request holds the lock; the probe drops the guard at once,
and the task then takes the normal blocking lock.

Task roster and intervals: `rust-wallet/src/monitor/CLAUDE.md`; the authoritative list is
`monitor/mod.rs :: TaskSchedule`.

### 8.2 UTXO Synchronization

Two mechanisms:
1. **Periodic (TaskSyncPending)**: Checks addresses with `pending_utxo_check=1` every 30s
2. **On-demand (`POST /wallet/sync`)**: Frontend trigger, supports `?full=true` for all addresses

### 8.3 Price & Fee Caching

- **PriceCache** (Rust): WhatsOnChain primary -> CoinGecko (`bitcoin-cash-sv`) -> MEXC (BSVUSDT)
  fallback chain, 300s in-memory TTL, thread-safe via `RwLock`, with a sanity filter that accepts
  prices above $0 and up to $10,000 (the error text in `price_cache.rs :: validate_price` still says
  $0.01). The last good price is persisted to the `bsv_price_cache` table (source label
  `persisted(<source>)` on reload) and
  reloaded on startup, so a cold start with all three sources down still yields a Silent decision
  instead of `Prompt(price_unavailable)`. CryptoCompare was removed after it began returning HTTP 401.
- **BSVPriceCache** (C++): WinHTTP to `/wallet/bsv-price`, 5-min TTL, used to convert satoshis to
  USD cents before forwarding a payment.
- **FeeRateCache** (Rust): mining fee rate from GorillaPool ARC's policy endpoint
  (`https://arc.gorillapool.io/v1/policy`), 1-hour TTL, `RwLock`-guarded, falling back to
  1000 sat/KB (1 sat/byte) when ARC is unreachable. Not mAPI.

---

## 9. BRC-100 Protocol

### 9.1 Implementation Status

| Surface | Status | Description |
|---------|--------|-------------|
| **Authentication (BRC-103/104)** | Complete | Mutual auth, app-scoped identity keys, BRC-42/43 key derivation |
| **Transactions (createAction/signAction, BRC-29, BEEF/SPV)** | Complete | Two-phase build/sign, BRC-29 PeerPay, BEEF broadcast with SPV ancestry |
| **BRC-33 (PeerServ Message Relay Interface) Messages** | Local relay | sendMessage, listMessages, acknowledgeMessage are served from a local SQLite store (`database/message_relay_repo.rs`), keyed on an identity header the wallet does not verify. PeerPay separately uses the federated MessageBox (`messagebox.rs`: BRC-103 AuthFetch transport, BRC-2 encrypted payloads). Both are intended to stay |

Which handlers exist for each surface: `rust-wallet/src/CLAUDE.md`.

### 9.2 Authentication Flow (BRC-104)

```
1. Client POST /.well-known/auth {initialNonce, identityKey}
2. Server: BRC-42 key derivation (ECDH shared secret -> HMAC -> child key)
3. Server: Sign concatenated nonces with derived key
4. Response: {version, messageType, identityKey, initialNonce, yourNonce, signature}
```

`initialNonce` in the response carries our new B_Nonce (it is not named `nonce`); `yourNonce` echoes
the client's A_Nonce; `identityKey` is BRC-42 app-scoped; `signature` is a DER **byte array**, not a
hex string.

### 9.3 BEEF/SPV

Transactions broadcast in BEEF (Background Evaluation Extended Format):
- `beef.rs`: Parser, TSC proof <-> BUMP conversion
- `beef_helpers.rs`: Recursive ancestry chain building
- `parent_transactions` table: Raw tx cache
- `proven_txs` table: Immutable merkle proof records; only proofs that verify against the wallet's own
  block headers are stored (`proof_store.rs`)
- Merkle proofs inside incoming BEEF are checked against those headers (`spv_checks.rs`); a proof for a
  block the wallet holds no header for is neither trusted nor refused

---

## 10. Status & Roadmap

This document carries no roadmap. Dated status and plans live in the per-release folders:

- `development-docs/0.4.0-beta.6/` — engine security refresh and the BRC-103 nonce fix
- `development-docs/0.4.0-beta.7/` — the planned release with tracks T0–T6, and its tickets
- `development-docs/0.4.0-beta.8/` — intake, and the tickets not yet scheduled

Known issues named in this document link to their tickets in those folders.

---

*This document is maintained alongside the codebase. See `CLAUDE.md` for AI assistant context and invariants.*
