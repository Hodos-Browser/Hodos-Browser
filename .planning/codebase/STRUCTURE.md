# Codebase Structure

**Analysis Date:** 2026-01-24

## Directory Layout

```
Hodos-Browser/
├── frontend/               # React UI (TypeScript, Vite)
│   ├── src/               # Source code
│   ├── dist/              # Build output (gitignored)
│   ├── node_modules/      # Dependencies (gitignored)
│   ├── package.json       # Node dependencies
│   ├── vite.config.ts     # Vite configuration
│   └── tsconfig.json      # TypeScript configuration
├── rust-wallet/           # Wallet backend (Rust, Actix-web)
│   ├── src/               # Source code
│   ├── tests/             # Integration tests
│   ├── target/            # Build output (gitignored)
│   ├── Cargo.toml         # Rust dependencies
│   └── Cargo.lock         # Dependency lock
├── cef-native/            # CEF browser shell (C++17)
│   ├── src/               # Source code
│   ├── include/           # Header files
│   ├── build/             # CMake build output (gitignored)
│   ├── mac/               # macOS-specific code
│   └── CMakeLists.txt     # Build configuration
└── cef-binaries/          # CEF framework binaries (gitignored)
```

## Directory Purposes

**frontend/**
- Purpose: React-based user interface
- Contains: TypeScript components, hooks, pages, bridge code
- Key files:
  - `src/main.tsx` - React entry point
  - `src/App.tsx` - Router with 7 routes
  - `src/bridge/initWindowBridge.ts` - window.hodosBrowser API setup
  - `vite.config.ts` - Dev server config (port 5137)
- Subdirectories:
  - `src/components/` - Reusable UI components
  - `src/pages/` - Route-level views (overlays)
  - `src/hooks/` - Custom React hooks
  - `src/bridge/` - C++ integration layer
  - `src/types/` - TypeScript declarations

**rust-wallet/**
- Purpose: Wallet backend with cryptography and database
- Contains: HTTP handlers, crypto modules, repositories
- Key files:
  - `src/main.rs` - HTTP server entry point (316 lines)
  - `src/handlers.rs` - All API endpoints (8107 lines)
  - `Cargo.toml` - Dependencies (actix-web, secp256k1, rusqlite, bip39)
- Subdirectories:
  - `src/crypto/` - BRC protocols (42, 43, 52), signing, encryption
  - `src/database/` - Repositories, migrations, SQLite connection
  - `src/transaction/` - TX building, SIGHASH, serialization
  - `src/certificate/` - BRC-52 identity certificates
  - `src/handlers/` - Handler submodules
  - `tests/` - Integration tests

**cef-native/**
- Purpose: CEF browser shell with V8 injection
- Contains: C++ handlers, HTTP interceptor, overlay management
- Key files:
  - `cef_browser_shell.cpp` - Main entry point, window creation
  - `CMakeLists.txt` - Build configuration
  - `src/handlers/simple_render_process_handler.cpp` - V8 injection (CefMessageSendHandler)
  - `src/core/HttpRequestInterceptor.cpp` - HTTP routing to Rust
- Subdirectories:
  - `src/handlers/` - CEF lifecycle handlers
  - `src/core/` - Business logic (Identity, Address, BRC100, History, Logger, etc.)
  - `include/handlers/` - Handler header files
  - `include/core/` - Core header files
  - `mac/` - macOS-specific implementations

## Key File Locations

**Entry Points:**
- `frontend/src/main.tsx` - React application entry
- `rust-wallet/src/main.rs` - Rust HTTP server entry (port 3301)
- `cef-native/cef_browser_shell.cpp` - C++ main() function

**Configuration:**
- `frontend/vite.config.ts` - Vite dev server config (port 5137, CORS)
- `frontend/tsconfig.app.json` - TypeScript compiler options (strict mode, ES2020)
- `frontend/tsconfig.node.json` - TypeScript config for build tools
- `frontend/eslint.config.js` - ESLint configuration
- `rust-wallet/Cargo.toml` - Rust dependencies and features
- `cef-native/CMakeLists.txt` - C++ build system, vcpkg integration

**Core Logic:**
- `frontend/src/bridge/initWindowBridge.ts` - window.hodosBrowser API (413 lines)
- `rust-wallet/src/handlers.rs` - HTTP endpoints (8107 lines)
- `rust-wallet/src/crypto/brc42.rs` - ECDH key derivation
- `rust-wallet/src/database/wallet_repo.rs` - Wallet data access
- `cef-native/src/handlers/simple_render_process_handler.cpp` - V8 injection
- `cef-native/src/core/HttpRequestInterceptor.cpp` - HTTP interception

**Testing:**
- `rust-wallet/tests/interoperability_test.rs` - Protocol interop tests (209 lines)
- `rust-wallet/tests/certificate_decryption_test.rs` - BRC-52 cert tests (79 lines)
- `rust-wallet/src/crypto/aesgcm_custom_test.rs` - AES-GCM tests (101 lines)
- No frontend tests

**Documentation:**
- `CLAUDE.md` - Project context for Claude Code
- `README.md` - Installation and build instructions (assumed)

## Naming Conventions

**Files:**

Frontend:
- `PascalCase.tsx` for React components (e.g., `WalletPanel.tsx`, `BRC100AuthModal.tsx`)
- `camelCase.ts` for non-component files (e.g., `initWindowBridge.ts`, `useHodosBrowser.ts`)
- `kebab-case.d.ts` for type definitions (e.g., `hodosBrowser.d.ts`, `identity.d.ts`)

Rust:
- `snake_case.rs` for all modules (e.g., `handlers.rs`, `wallet_repo.rs`, `utxo_sync.rs`)
- `snake_case_test.rs` for test files (e.g., `interoperability_test.rs`)

C++:
- `snake_case.cpp/.h` for implementation/header files (e.g., `simple_handler.cpp`, `simple_handler.h`)
- `PascalCase` for class files sometimes (e.g., `AddressHandler.cpp`)

**Directories:**
- Frontend: `kebab-case` (e.g., `src/`, `components/`, `bridge/`)
- Rust: `snake_case` (e.g., `src/`, `crypto/`, `database/`)
- C++: `snake_case` (e.g., `src/`, `include/`, `handlers/`, `core/`)

**Special Patterns:**
- `index.tsx` - Not used (direct imports preferred)
- `*Root.tsx` - Page-level components for overlays (e.g., `WalletOverlayRoot.tsx`)
- `use*.ts` - Custom React hooks (e.g., `useBalance.ts`, `useAddress.ts`)
- `*_repo.rs` - Rust repository pattern (e.g., `wallet_repo.rs`, `address_repo.rs`)
- `*Handler.cpp` - C++ handler pattern (e.g., `AddressHandler.cpp`, `IdentityHandler.cpp`)

## Where to Add New Code

**New React Component:**
- Primary code: `frontend/src/components/[ComponentName].tsx`
- Types: `frontend/src/types/[domain].d.ts` (if new domain)
- Hook: `frontend/src/hooks/use[Domain].ts` (if needs API integration)
- Tests: None configured (no test framework)

**New Rust Endpoint:**
- Handler: `rust-wallet/src/handlers.rs` (add function to existing monolith)
- Or: `rust-wallet/src/handlers/[module]_handlers.rs` (if creating new module)
- Route registration: `rust-wallet/src/main.rs` (add to HttpServer::new())
- Types: `rust-wallet/src/database/models.rs` (if new data structure)
- Repository: `rust-wallet/src/database/[name]_repo.rs` (if new database table)
- Tests: `rust-wallet/tests/[feature]_test.rs`

**New Overlay:**
- Frontend page: `frontend/src/pages/[Name]OverlayRoot.tsx`
- Route: Add to `frontend/src/App.tsx` router
- C++ window creation: Add HWND to `cef-native/cef_browser_shell.cpp` globals
- Bridge message: Add to `frontend/src/bridge/initWindowBridge.ts`
- C++ handler: `cef-native/src/core/[Name]Handler.cpp`

**New BRC Protocol:**
- Crypto module: `rust-wallet/src/crypto/brc[number].rs`
- Handlers: `rust-wallet/src/handlers/[protocol]_handlers.rs`
- Tests: `rust-wallet/tests/[protocol]_test.rs`
- Frontend integration: `frontend/src/bridge/brc[number].ts`

**New Database Table:**
- Repository: `rust-wallet/src/database/[name]_repo.rs`
- Model: Add struct to `rust-wallet/src/database/models.rs`
- Migration: Update `rust-wallet/src/database/migrations.rs`
- Export: Add to `rust-wallet/src/database/mod.rs`

## Special Directories

**frontend/dist/**
- Purpose: Vite build output
- Source: Generated by `npm run build`
- Committed: No (in .gitignore)

**rust-wallet/target/**
- Purpose: Cargo build output
- Source: Generated by `cargo build`
- Committed: No (in .gitignore)

**cef-native/build/**
- Purpose: CMake build artifacts
- Source: Generated by `cmake --build`
- Committed: No (in .gitignore)

**cef-binaries/**
- Purpose: CEF framework binaries
- Source: Downloaded from https://cef-builds.spotifycdn.com/
- Committed: No (in .gitignore, user must download)

**%APPDATA%/HodosBrowser/** (Runtime)
- Purpose: Application data directory
- Source: Created at runtime
- Structure:
  - `wallet/wallet.db` - SQLite wallet database
  - `Default/` - CEF browser data (cookies, cache, history)
- Committed: No (user data, not in repo)

---

*Structure analysis: 2026-01-24*
*Update when directory structure changes*
