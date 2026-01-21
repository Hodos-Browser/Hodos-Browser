# Architecture

**Analysis Date:** 2026-01-20

## Pattern Overview

**Overall:** Layered Architecture with Process Isolation (Three-Tier Web3 Browser)

**Key Characteristics:**
- Strict security boundaries between UI, browser engine, and cryptography
- Private keys never leave the Rust wallet process
- Multi-process CEF leverages Chromium's natural security boundaries
- Overlay model for settings, wallet panel, backup modal, and BRC-100 auth

```
React Frontend (Port 5137)
    | window.hodosBrowser.*
    v
C++ CEF Shell
    | HTTP interception & forwarding -> localhost:3301
    v
Rust Wallet Backend (Port 3301)
    |
    v
Bitcoin SV Blockchain (WhatsOnChain, GorillaPool)
```

## Layers

**Frontend Layer (React/TypeScript):**
- Purpose: User interface and interactions
- Contains: React components, hooks, overlay pages, bridge code
- Depends on: CEF V8 injection (`window.hodosBrowser`, `window.cefMessage`)
- Used by: End users via CEF browser window
- Location: `frontend/src/`
- Security: Never handles keys or signing; all crypto delegated

**Browser Shell Layer (C++/CEF):**
- Purpose: Window management, HTTP interception, browser data, V8 injection
- Contains: CEF handlers, overlay management, history/bookmarks, HTTP routing
- Depends on: CEF 136 framework, Rust wallet API on localhost:3301
- Used by: Frontend React layer via injected APIs
- Location: `cef-native/src/`
- Security: Bridges requests but never accesses private keys

**Wallet Backend Layer (Rust/Actix):**
- Purpose: Cryptographic operations, key management, BRC-100 protocol
- Contains: HTTP handlers, crypto modules, database repositories, UTXO management
- Depends on: SQLite for persistence, WhatsOnChain for blockchain data
- Used by: CEF shell via HTTP requests
- Location: `rust-wallet/src/`
- Security: Compile-time memory safety; private keys only here; key clearing

## Data Flow

**Address Generation Flow:**

1. Frontend: `useHodosBrowser().generateAddress()` called
2. Frontend: `window.cefMessage.send('address_generate', [])` sent
3. CEF Render: `CefMessageSendHandler` in V8 captures call
4. CEF Browser: `simple_handler.cpp` receives IPC message
5. CEF Browser: `AddressHandler.cpp` processes request
6. CEF Browser: WinHTTP/curl request to `localhost:3301/wallet/address/generate`
7. Rust: `handlers.rs::generate_address()` handler invoked
8. Rust: `AddressRepository.create_new_address()` with BRC-42 derivation
9. Rust: SQLite write via `database/mod.rs`
10. Response: JSON flows back through HTTP -> IPC -> V8 -> React callback

**BRC-100 Authentication Flow:**

1. Web page calls `window.hodosBrowser.brc100.getPublicKey()`
2. CEF intercepts HTTP request to `.well-known/auth`
3. HTTP routed to Rust `/getPublicKey` endpoint
4. Rust performs BRC-42 key derivation
5. Response includes derived public key for protocol
6. (Optional) Overlay opens for user approval via `overlay_show_brc100_auth`

**State Management:**
- File-based: Wallet state in SQLite (`wallet.db`)
- In-memory: Balance cache, pending transactions, auth sessions
- CEF-level: Browser history, bookmarks, cookies in `Default/` directory
- No shared state between CEF overlays (process isolation)

## Key Abstractions

**Frontend Abstractions:**
- Purpose: Encapsulate CEF bridge communication
- Examples: `useHodosBrowser`, `useWallet`, `useBalance`, `useTransaction`, `useHistory`, `useTabManager`, `useAddress` hooks
- Location: `frontend/src/hooks/*.ts`
- Pattern: React custom hooks returning API methods

**Rust Repository Pattern:**
- Purpose: Data access abstraction over SQLite
- Examples: `WalletRepository`, `AddressRepository`, `UtxoRepository`, `CertificateRepository`, `TransactionRepository`
- Location: `rust-wallet/src/database/*.rs`
- Pattern: Singleton-like modules with database connection pool

**Rust Crypto Modules:**
- Purpose: Cryptographic operations isolated from business logic
- Examples: `brc42` (key derivation), `brc43` (invoice numbers), `brc2` (encryption), `signing` (SHA256, HMAC)
- Location: `rust-wallet/src/crypto/*.rs`
- Pattern: Pure functions, no database access

**CEF Handler Classes:**
- Purpose: CEF lifecycle and IPC management
- Examples: `SimpleApp`, `SimpleHandler`, `CefMessageSendHandler`, `AsyncWalletResourceHandler`
- Location: `cef-native/src/handlers/*.cpp`
- Pattern: CEF callback interfaces (CefClient, CefRenderProcessHandler)

## Entry Points

**Frontend Entry:**
- Location: `frontend/src/main.tsx`
- Triggers: Vite dev server or CEF loading `index.html`
- Responsibilities: Mount React app, initialize `BrowserRouter`, call `initWindowBridge.ts`

**CEF Entry (Windows):**
- Location: `cef-native/cef_browser_shell.cpp`
- Triggers: User launches `HodosBrowserShell.exe`
- Responsibilities: Create main window, initialize CEF, spawn browser/render processes
- Globals: `g_hwnd`, `g_header_hwnd`, `g_webview_hwnd`, overlay HWNDs

**CEF Entry (macOS):**
- Location: `cef-native/cef_browser_shell_mac.mm`
- Triggers: User launches app bundle
- Responsibilities: macOS-specific window creation, CEF initialization

**Rust Wallet Entry:**
- Location: `rust-wallet/src/main.rs`
- Triggers: `cargo run --release` or process spawn
- Responsibilities: Initialize `AppState`, database connections, start Actix server on port 3301

## Error Handling

**Strategy:** Layer-specific handling with user-facing errors at boundaries

**Patterns:**
- Rust: Custom error types with `thiserror`, `.map_err()` chains, `HttpResponse::BadRequest` for invalid input
- C++: Logging via `Logger` class, error codes in IPC responses
- Frontend: try/catch in hooks, console.error for debugging, error state in React components

**Error Boundaries:**
- HTTP layer returns JSON error responses with `{"error": "message"}`
- IPC layer sends error messages via `cefMessageResponse` events
- Frontend displays errors via MUI components or console

## Cross-Cutting Concerns

**Logging:**
- Rust: `env_logger` with info/debug/error levels (`rust-wallet/src/main.rs`)
- C++: Custom `Logger` class with timestamps, emoji prefixes (`cef-native/`)
- Frontend: `console.log` with emoji prefixes (304+ instances)

**Validation:**
- Rust: Manual validation in handlers, `serde` for JSON deserialization
- Frontend: TypeScript compile-time checks (but many `any` types)
- No formal schema validation (Zod, JSON Schema)

**Authentication:**
- BRC-100 protocol suite (custom implementation)
- No traditional session cookies or JWT
- Domain whitelist for auto-approval (`rust-wallet/src/domain_whitelist.rs`)

**Security:**
- Private keys isolated in Rust process
- V8 injection exposes only safe, high-level APIs
- Multi-process CEF provides render/browser isolation
- CORS enabled for development (allow-any-origin)

---

*Architecture analysis: 2026-01-20*
*Update when major patterns change*
