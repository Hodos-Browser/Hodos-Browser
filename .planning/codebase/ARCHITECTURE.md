# Architecture

**Analysis Date:** 2026-01-28

## Pattern Overview

**Overall:** Three-layer security-boundary architecture with isolated process-per-overlay model

**Key Characteristics:**
- Strict separation of concerns: Frontend (React) → CEF Shell (C++) → Wallet Backend (Rust)
- Private keys never accessible from JavaScript layer
- HTTP interception at CEF layer routes wallet requests to Rust on localhost:3301
- Each overlay (Settings, Wallet, Backup, BRC-100 Auth) runs as isolated CEF subprocess with separate V8 context
- V8 injection provides `window.hodosBrowser.*` API to JavaScript; all sensitive operations delegate to C++ then Rust

## Layers

**Presentation Layer (React):**
- Purpose: User interface for wallet, overlays, address management, transaction history, BRC-100 auth modal
- Location: `frontend/src/`
- Contains: React components (`src/components/`, `src/pages/`), hooks (`src/hooks/`), routing
- Depends on: `window.hodosBrowser.*` API injected by C++ via V8; `window.cefMessage.send()` for IPC
- Used by: CEF browser process which loads React dev server on localhost:5137
- Never: handles private keys, signs transactions, or calls Rust directly

**Browser Shell Layer (C++):**
- Purpose: Browser engine, V8 JavaScript injection, HTTP request interception, IPC routing, window/overlay management
- Location: `cef-native/`
- Contains: CEF handlers (`src/handlers/`), core routing (`src/core/`), HTTP interceptor, TabManager, HistoryManager
- Depends on: CEF 136, Rust wallet on localhost:3301 for /health, /getPublicKey, /.well-known/auth, etc.
- Used by: Acts as bridge between React frontend and Rust wallet
- Invariant: Never accesses or stores private keys; forwards requests only

**Wallet Backend Layer (Rust):**
- Purpose: Cryptographic signing, key derivation (BRC-42/BRC-43), BRC-100 authentication, SQLite storage, transaction building
- Location: `rust-wallet/src/`
- Contains: HTTP handlers (`handlers.rs`), crypto modules (`crypto/`), database (`database/`), certificate management
- Depends on: Actix-web, Bitcoin SV blockchain (WhatsOnChain, GorillaPool APIs)
- Used by: Receives HTTP requests from C++ on port 3301
- Invariant: Private keys never leave this process; all signing happens here in Rust for memory safety

## Data Flow

**User initiates wallet action (e.g., generate address):**

1. React component calls `window.hodosBrowser.wallet.generateAddress()` from `frontend/src/components/`
2. V8 injection handler (`cef-native/src/handlers/simple_render_process_handler.cpp`) catches call
3. JavaScript message sent via `cefMessage.send('address_generate', [])`
4. CEF browser process receives message in `OnProcessMessageReceived()` (`simple_handler.cpp`)
5. HTTP request forwarded to Rust wallet endpoint (e.g., POST `/generateAddress`)
6. Rust handler in `rust-wallet/src/handlers.rs` derives child private key using BRC-42 from master key in database
7. Response JSON returned to C++ layer
8. C++ injects response back into JavaScript via V8 callback
9. React component receives promise resolution and re-renders

**User submits BRC-100 authentication request:**

1. Website makes HTTP request to authenticated endpoint
2. CEF HTTP interceptor (`HttpRequestInterceptor.cpp`) detects wallet domain in request
3. Request blocked; user redirected to BRC-100 auth overlay subprocess
4. Overlay subprocess loads `frontend/src/pages/BRC100AuthOverlayRoot.tsx`
5. User approves/rejects in modal (`BRC100AuthModal.tsx`)
6. Response callback in `App.tsx` via `window.showBRC100AuthApprovalModal()`
7. C++ handles whitelist logic and domain verification (`DomainVerifier` class)
8. Request relayed to Rust wallet for signing action via `/signAction` endpoint
9. Signed action returned to website

**State Management:**

- Frontend component state: React hooks (`useWallet()`, `useBalance()`, `useAddress()`)
- C++ state: Global HWNDs (`g_hwnd`, `g_settings_overlay_hwnd`, etc.), backup modal flag `g_backupModalShown`
- Rust state: SQLite database (`%APPDATA%/HodosBrowser/wallet/wallet.db`) for addresses, UTXOs, certificates, transactions
- Session state: In-memory `AuthSessionManager` for BRC-100 sessions; `BalanceCache` for balance updates

## Key Abstractions

**Window.hodosBrowser API:**
- Purpose: Provides safe interface to wallet operations without exposing keys
- Examples: `frontend/src/bridge/initWindowBridge.ts` defines and exposes methods
- Pattern: Promise-based callbacks using `window.onResponse` handlers; C++ injects via V8 into render process
- Methods: `wallet.create()`, `wallet.getStatus()`, `address.generate()`, `overlay.show()`, `navigation.navigate()`

**HTTP Request Interception:**
- Purpose: Intercepts requests to wallet domains, enforces whitelisting, routes to C++/Rust
- Location: `cef-native/src/core/HttpRequestInterceptor.cpp`
- Classes: `DomainVerifier` (checks whitelist JSON), `AsyncWalletResourceHandler` (handles async routing)
- Pattern: CEF resource handler that intercepts requests matching wallet domains; blocks or allows based on whitelist

**BRC-42 / BRC-43 Crypto:**
- Purpose: Derives child keys from master seed; generates invoice numbers with security levels
- Location: `rust-wallet/src/crypto/brc42.rs`, `rust-wallet/src/crypto/brc43.rs`
- Pattern: Functions take master private key + counterparty public key + protocol ID → derive child key
- Used by: `/signAction` handler to sign actions with derived keys

**Overlay Architecture:**
- Purpose: Process-per-overlay isolation provides defense in depth for sensitive UIs (settings, backup, BRC-100 auth)
- Location: CEF subprocesses created by `CreateWalletOverlayWithSeparateProcess()` in `simple_app.cpp`
- Pattern: Each overlay is separate CEF browser instance with own window HWND, own V8 context, own React router path
- Benefit: Compromised website JS cannot access overlay's V8 context or private keys

**Database Layer:**
- Purpose: Replace JSON file storage with SQLite for wallet state persistence
- Location: `rust-wallet/src/database/`
- Repos: `WalletRepository`, `AddressRepository`, `UtxoRepository`, `CertificateRepository`, `MessageRelayRepository`
- Pattern: Each repository provides typed access to a database table; `WalletDatabase` provides transaction support

## Entry Points

**C++ Shell:**
- Location: `cef-native/cef_browser_shell.cpp`
- Triggers: User runs `HodosBrowserShell.exe`
- Responsibilities: Window creation, CEF initialization, message loop, overlay HWND setup

**React Frontend:**
- Location: `frontend/src/main.tsx`
- Triggers: CEF loads localhost:5137
- Responsibilities: Routes requests to `App.tsx` which renders router with overlay pages

**Rust Wallet:**
- Location: `rust-wallet/src/main.rs`
- Triggers: User starts `cargo run --release` or C++ WalletService spawns it
- Responsibilities: Actix-web HTTP server on port 3301; initializes database, crypto, handlers

**Route Entry Points (Frontend React Router):**
- `/` → `MainBrowserView` - main browser window with tabs, address bar, wallet panel trigger
- `/settings` → `SettingsOverlayRoot` - settings overlay (separate CEF subprocess)
- `/wallet` → `WalletOverlayRoot` - wallet overlay with transaction form, history
- `/backup` → `BackupOverlayRoot` - backup modal with mnemonic display
- `/brc100-auth` → `BRC100AuthOverlayRoot` - BRC-100 authentication overlay

## Error Handling

**Strategy:** Layered error handling with try-catch at each boundary

**Patterns:**

- Frontend: React component `try`-`catch` in hooks; promise rejections handled with `onError` callbacks
- C++ to Rust: HTTP response status codes; JSON error fields in response body
- Rust: Handler functions return `Result<HttpResponse>` with custom error types
- Logging: Multi-tier logging system via `Logger` class in `cef-native/include/core/Logger.h`
  - Renderer process logs: `LOG_DEBUG_RENDER()`, `LOG_ERROR_RENDER()`
  - Browser process logs: `LOG_DEBUG_BROWSER()`, `LOG_ERROR_BROWSER()`
  - HTTP interceptor logs: `LOG_DEBUG_HTTP()`, `LOG_ERROR_HTTP()`

## Cross-Cutting Concerns

**Logging:**
- Centralized via `Logger::Log(message, level, context)` in C++
- Rust uses `env_logger` with environment variable configuration
- Frontend uses `console.log()` with emoji prefixes for easy searching

**Validation:**
- C++ HTTP interceptor validates domain whitelist before forwarding requests
- Rust handlers validate BRC-43 invoice numbers and protocol IDs
- Frontend React components validate user input (recipient address, amounts) before submitting

**Authentication:**
- BRC-100 protocol: Certificate-based mutual authentication via `/well-known/auth` endpoint
- WhiteListing: `DomainVerifier` manages domain whitelist JSON; checked on each request
- Session Management: `AuthSessionManager` in Rust tracks active BRC-100 auth sessions per domain

---

*Architecture analysis: 2026-01-28*
