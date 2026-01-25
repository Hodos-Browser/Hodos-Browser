# Architecture

**Analysis Date:** 2026-01-24

**Feature-Specific Documentation:**
- For detailed address bar and history database architecture, see `../phases/1-foundation-investigation/INVESTIGATION.md` (React components, HistoryManager C++ API, SQLite schema, IPC protocol, data flow diagrams)

## Pattern Overview

**Overall:** Three-Layer Distributed Architecture with Process Isolation

**Key Characteristics:**
- Strict separation of concerns: UI → Browser → Wallet → Blockchain
- Process isolation for security (each overlay runs as separate CEF subprocess)
- Layered communication via V8 injection and HTTP interception
- Production-focused: Security and correctness over development speed
- Private keys never leave Rust process (security invariant)

## Layers

**Frontend Layer (React/TypeScript):**
- Purpose: User interface and interactions
- Contains: React components, hooks, routing, UI state management
- Location: `frontend/src/`
- Depends on: window.hodosBrowser API (provided by C++ V8 injection)
- Used by: End users via CEF browser windows
- Never handles: Private keys, signing operations, cryptography

**Native Shell Layer (C++/CEF):**
- Purpose: Browser engine, V8 injection, HTTP interception, IPC routing
- Contains: CEF handlers, message routing, HTTP interceptor, overlay management
- Location: `cef-native/src/`
- Depends on: CEF framework, Rust wallet HTTP endpoints (localhost:3301)
- Used by: Frontend via window.cefMessage.send() and window.hodosBrowser.*
- Entry point: `cef-native/cef_browser_shell.cpp`

**Wallet Backend Layer (Rust):**
- Purpose: Cryptography, signing, key management, database persistence
- Contains: HTTP handlers, crypto modules, repositories, background services
- Location: `rust-wallet/src/`
- Depends on: SQLite database, blockchain APIs (WhatsOnChain, GorillaPool)
- Used by: C++ layer via HTTP requests to localhost:3301
- Entry point: `rust-wallet/src/main.rs`

## Data Flow

**HTTP Request Lifecycle (Example: Address Generation):**

1. **User initiates**: React component calls `window.hodosBrowser.address.generate()`
   - Location: `frontend/src/components/WalletPanel.tsx`

2. **V8 bridge**: JavaScript call → C++ handler
   - Handler: `CefMessageSendHandler::Execute()` in `cef-native/src/handlers/simple_render_process_handler.cpp`
   - Converts: JS function call → CefProcessMessage('address_generate')

3. **IPC routing**: Message routed to AddressHandler
   - Router: `SimpleHandler::OnProcessMessageReceived()` in `cef-native/src/handlers/simple_handler.cpp`
   - Routes to: `AddressHandler` in `cef-native/src/core/AddressHandler.cpp`

4. **HTTP interception**: C++ makes HTTP POST to localhost:3301
   - Handler: `AsyncWalletResourceHandler` in `cef-native/src/core/HttpRequestInterceptor.cpp`
   - Request: POST /wallet/address/generate with JSON body

5. **Rust processing**: HTTP endpoint executes wallet logic
   - Endpoint: `generate_address()` in `rust-wallet/src/handlers.rs`
   - Actions: Query database → Derive next address → Return JSON response

6. **Response propagation**: Flows back through layers
   - C++ receives HTTP response → Executes JS callback: `window.onAddressGenerated(data)`
   - React state updates → UI re-renders with new address

**State Management:**
- **Frontend**: React useState hooks, component-local state
- **C++**: Globals (g_hwnd, g_pendingAuthRequest), message queues
- **Rust**: AppState with Arc<Mutex<T>> shared state (database, whitelist, auth sessions, balance cache)
- **Database**: SQLite for persistent state (wallet.db)

## Key Abstractions

**Frontend Patterns:**
- **Custom Hook**: React hooks wrapping window.hodosBrowser API
  - Examples: `useHodosBrowser()`, `useBalance()` in `frontend/src/hooks/`
  - Pattern: Encapsulates C++ communication, returns typed data
- **Bridge Module**: Isolates C++ integration
  - Example: `initWindowBridge.ts` in `frontend/src/bridge/`
  - Pattern: Provides window.hodosBrowser.*, sets up callbacks
- **Route-Based Overlays**: Each overlay has dedicated route
  - Examples: /settings, /wallet, /backup, /brc100-auth
  - Pattern: `frontend/src/pages/*Root.tsx` files

**Rust Patterns:**
- **HTTP Handler**: Actix-web async functions
  - Examples: `get_public_key()`, `create_action()`, `sign_action()` in `rust-wallet/src/handlers.rs`
  - Pattern: Single monolithic file (8107 lines)
- **Repository**: Data access layer for SQLite
  - Examples: `WalletRepository`, `AddressRepository`, `UtxoRepository` in `rust-wallet/src/database/`
  - Pattern: Struct with methods for CRUD operations
- **Crypto Module**: BRC protocol implementations
  - Examples: `brc42.rs`, `brc43.rs`, `signing.rs` in `rust-wallet/src/crypto/`
  - Pattern: Pure functions with Result<T, Error> return types

**C++ Patterns:**
- **V8 Handler**: CefV8Handler subclasses for JavaScript injection
  - Example: `CefMessageSendHandler` in `cef-native/src/handlers/simple_render_process_handler.cpp`
  - Pattern: Execute() method handles JS function calls
- **HTTP Interceptor**: CefResourceHandler subclasses
  - Example: `AsyncWalletResourceHandler` in `cef-native/src/core/HttpRequestInterceptor.cpp`
  - Pattern: Routes wallet requests to Rust, proxies other requests
- **Message Handler**: CEF process message routing
  - Example: `SimpleHandler::OnProcessMessageReceived()` in `cef-native/src/handlers/simple_handler.cpp`
  - Pattern: Switch statement routing to specialized handlers

## Entry Points

**Frontend:**
- Location: `frontend/src/main.tsx`
- Triggers: Loaded by CEF browser window
- Responsibilities: Initialize React, render router, set up bridge

**C++ CEF Shell:**
- Location: `cef-native/cef_browser_shell.cpp` (main function)
- Triggers: User launches application executable
- Responsibilities: Create main window, spawn overlay subprocesses, initialize CEF, start message loop

**Rust Wallet:**
- Location: `rust-wallet/src/main.rs`
- Triggers: Started manually (dev) or bundled with app (production)
- Responsibilities: Initialize AppState, register HTTP routes, spawn background services (UTXO sync, cache sync)

## Error Handling

**Strategy:** Layer-specific error handling with propagation

**Patterns:**
- **Frontend**: Try/catch on promises, error state in React components
  - Example: `frontend/src/hooks/useHodosBrowser.ts` catch blocks log errors and reject promises
- **Rust**: Result<T, Error> pattern with custom error types
  - Example: `rust-wallet/src/crypto/brc42.rs` returns Result<Vec<u8>, Brc42Error>
  - Critical issue: 61 instances of `.lock().unwrap()` on mutex (panics if poisoned)
- **C++**: Exception handling at handler boundaries
  - Example: `cef-native/src/core/AddressHandler.cpp` catches exceptions, logs errors

**Error Propagation:**
- Rust errors → HTTP 400/500 responses → C++ logs error → JS callback receives error object → React displays error to user

## Cross-Cutting Concerns

**Logging:**
- **Frontend**: console.log/error with emoji prefixes (`🔍`, `🔐`, etc.)
- **Rust**: env_logger with log::info!/error! macros
- **C++**: Custom Logger class with LOG_INFO, LOG_DEBUG, LOG_ERROR macros
  - Location: `cef-native/src/core/Logger.cpp`
  - Output: stdout and file (`browser_debug.log`)

**Validation:**
- **Frontend**: Basic input validation in forms
  - Location: `frontend/src/components/TransactionForm.tsx`
  - Issue: No comprehensive validation before sending to backend
- **Rust**: JSON schema validation via serde, custom validators in handlers
  - Issue: Missing input validation for field constraints (TODO: nonce validation)

**Authentication:**
- **BRC-100 Protocol**: Mutual authentication via cryptographic signatures
  - Implementation: `rust-wallet/src/auth_session.rs`, `rust-wallet/src/handlers/certificate_handlers.rs`
  - Session tracking: AuthSessionManager in AppState
  - Domain whitelist: DomainWhitelistManager restricts auth requests

**Security:**
- **Private key isolation**: Keys never leave Rust process
- **Process isolation**: Each overlay runs in separate CEF subprocess with isolated V8 context
- **HTTP interception**: DomainVerifier class validates requests before forwarding to Rust
  - Location: `cef-native/src/core/HttpRequestInterceptor.cpp`

---

*Architecture analysis: 2026-01-24*
*Update when major patterns change*
