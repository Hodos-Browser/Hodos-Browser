# Codebase Structure

**Analysis Date:** 2026-01-20

## Directory Layout

```
Hodos-Browser/
├── frontend/               # React TypeScript UI (port 5137)
├── cef-native/             # C++ CEF browser shell
├── rust-wallet/            # Rust wallet backend (port 3301)
├── cef-binaries/           # CEF binaries (gitignored)
├── .planning/              # Project planning documents
├── CLAUDE.md               # AI assistant context
├── README.md               # User documentation
├── BUILD_INSTRUCTIONS.md   # Build steps
├── ARCHITECTURE.md         # System architecture
├── API_REFERENCES.md       # API documentation
└── .gitignore              # Git exclusions
```

## Directory Purposes

**frontend/**
- Purpose: React application for browser UI and overlays
- Contains: TypeScript/TSX source files, Vite config, npm packages
- Key files: `src/main.tsx` (entry), `src/App.tsx` (router), `vite.config.ts`
- Subdirectories:
  - `src/components/` - React UI components
  - `src/hooks/` - Custom React hooks for CEF bridge
  - `src/pages/` - Overlay root components
  - `src/bridge/` - V8 bridge initialization
  - `src/types/` - TypeScript type definitions

**cef-native/**
- Purpose: C++ browser shell using Chromium Embedded Framework
- Contains: C++ source, headers, CMake config, platform-specific code
- Key files: `cef_browser_shell.cpp` (Windows entry), `cef_browser_shell_mac.mm` (macOS entry)
- Subdirectories:
  - `src/handlers/` - CEF lifecycle handlers
  - `src/core/` - Business logic and services
  - `include/` - C++ headers
  - `build/` - CMake output (gitignored)
  - `mac/` - macOS build artifacts

**rust-wallet/**
- Purpose: Cryptographic wallet backend with BRC-100 protocol
- Contains: Rust source files, Cargo manifest
- Key files: `src/main.rs` (entry), `src/handlers.rs` (HTTP endpoints), `Cargo.toml`
- Subdirectories:
  - `src/crypto/` - Cryptographic implementations
  - `src/database/` - SQLite repositories
  - `src/certificate/` - BRC-52 certificate management
  - `src/transaction/` - Bitcoin transaction handling
  - `src/handlers/` - Additional HTTP handlers
  - `target/` - Build output (gitignored)

## Key File Locations

**Entry Points:**
- `frontend/src/main.tsx` - React bootstrap
- `cef-native/cef_browser_shell.cpp` - Windows CEF entry
- `cef-native/cef_browser_shell_mac.mm` - macOS CEF entry
- `rust-wallet/src/main.rs` - Rust HTTP server

**Configuration:**
- `frontend/vite.config.ts` - Vite dev server (port 5137)
- `frontend/tsconfig.app.json` - TypeScript strict mode
- `frontend/eslint.config.js` - ESLint v9+ flat config
- `frontend/package.json` - npm dependencies
- `rust-wallet/Cargo.toml` - Rust dependencies
- `cef-native/CMakeLists.txt` - CMake build config

**Core Logic:**
- `frontend/src/bridge/initWindowBridge.ts` - V8 API definition
- `frontend/src/hooks/useHodosBrowser.ts` - Main CEF bridge hook
- `cef-native/src/handlers/simple_handler.cpp` - Browser client, message routing
- `cef-native/src/handlers/simple_render_process_handler.cpp` - V8 injection
- `cef-native/src/core/HttpRequestInterceptor.cpp` - HTTP routing to wallet
- `rust-wallet/src/handlers.rs` - HTTP endpoint handlers (7500+ lines)
- `rust-wallet/src/handlers/certificate_handlers.rs` - BRC-52 endpoints

**Cryptography:**
- `rust-wallet/src/crypto/brc42.rs` - ECDH key derivation
- `rust-wallet/src/crypto/brc43.rs` - Invoice number format
- `rust-wallet/src/crypto/brc2.rs` - AES-GCM encryption
- `rust-wallet/src/crypto/signing.rs` - SHA256, HMAC
- `rust-wallet/src/transaction/sighash.rs` - BSV ForkID signing

**Database:**
- `rust-wallet/src/database/mod.rs` - Connection pool, schema init
- `rust-wallet/src/database/wallet_repo.rs` - Wallet repository
- `rust-wallet/src/database/address_repo.rs` - Address repository
- `rust-wallet/src/database/utxo_repo.rs` - UTXO repository
- `rust-wallet/src/database/certificate_repo.rs` - Certificate repository
- `rust-wallet/src/database/migrations.rs` - Schema definitions

**Testing:**
- `rust-wallet/src/crypto/brc42.rs` (inline tests)
- `rust-wallet/src/crypto/keys.rs` (inline tests)
- `rust-wallet/src/transaction/sighash.rs` (inline tests)
- `rust-wallet/src/certificate/selective_disclosure.rs` (inline tests)
- No frontend test directory

**Documentation:**
- `README.md` - Project overview
- `CLAUDE.md` - AI context
- `BUILD_INSTRUCTIONS.md` - Build steps
- `ARCHITECTURE.md` - System design
- `API_REFERENCES.md` - API docs

## Naming Conventions

**Files:**
- TypeScript components: `PascalCase.tsx` (e.g., `WalletPanel.tsx`)
- TypeScript hooks: `camelCase.ts` with `use` prefix (e.g., `useHodosBrowser.ts`)
- TypeScript types: `camelCase.d.ts` for ambient, `PascalCase.ts` for exports
- Rust modules: `snake_case.rs` (e.g., `wallet_repo.rs`)
- C++ files: `PascalCase.cpp/.h` (e.g., `HttpRequestInterceptor.cpp`)
- C++ macOS: `*_mac.mm` suffix (e.g., `TabManager_mac.mm`)

**Directories:**
- Lowercase with hyphens (frontend style) or underscores (Rust style)
- Plural for collections: `components/`, `hooks/`, `handlers/`

**Special Patterns:**
- `*OverlayRoot.tsx` - Overlay entry components
- `*_repo.rs` - Database repository modules
- `*Handler.cpp` - CEF/domain service classes
- `index.ts` - Barrel exports (not heavily used)

## Where to Add New Code

**New React Component:**
- Implementation: `frontend/src/components/{ComponentName}.tsx`
- Types: `frontend/src/types/{name}.d.ts` or inline
- Hook (if needed): `frontend/src/hooks/use{Feature}.ts`

**New Overlay/Page:**
- Implementation: `frontend/src/pages/{Name}OverlayRoot.tsx`
- Route: Add to `frontend/src/App.tsx`
- CEF handler: Update `cef-native/src/handlers/simple_handler.cpp`

**New Rust Endpoint:**
- Handler: Add to `rust-wallet/src/handlers.rs` or create new module in `rust-wallet/src/handlers/`
- Route: Register in `rust-wallet/src/main.rs`
- Types: Add structs in handler file or `rust-wallet/src/database/models.rs`

**New Crypto Module:**
- Implementation: `rust-wallet/src/crypto/{module}.rs`
- Export: Add `pub mod {module};` to `rust-wallet/src/crypto/mod.rs`
- Tests: Inline `#[cfg(test)]` module

**New Database Repository:**
- Implementation: `rust-wallet/src/database/{entity}_repo.rs`
- Export: Add to `rust-wallet/src/database/mod.rs`
- Schema: Update `rust-wallet/src/database/migrations.rs`

**New CEF Handler:**
- Header: `cef-native/include/handlers/{Name}.h` or `include/core/{Name}.h`
- Implementation: `cef-native/src/handlers/{Name}.cpp` or `src/core/{Name}.cpp`
- Integration: Include in `simple_handler.cpp` or appropriate handler

## Special Directories

**cef-binaries/**
- Purpose: CEF framework binaries (downloaded separately)
- Source: https://cef-builds.spotifycdn.com/index.html
- Committed: No (gitignored)

**frontend/node_modules/**
- Purpose: npm dependencies
- Source: Generated by `npm install`
- Committed: No (gitignored)

**rust-wallet/target/**
- Purpose: Rust build artifacts
- Source: Generated by `cargo build`
- Committed: No (gitignored)

**cef-native/build/**
- Purpose: CMake build output
- Source: Generated by CMake
- Committed: No (gitignored)

**.planning/**
- Purpose: Project planning and codebase documentation
- Source: GSD workflow outputs
- Committed: Yes

---

*Structure analysis: 2026-01-20*
*Update when directory structure changes*
