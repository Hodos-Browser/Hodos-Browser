# Codebase Structure

**Analysis Date:** 2026-01-28

## Directory Layout

```
Hodos-Browser/
├── frontend/                      # React/TypeScript UI layer
│   ├── src/
│   │   ├── main.tsx               # React app entry
│   │   ├── App.tsx                # Router setup; overlay routes
│   │   ├── bridge/                # C++ ↔ JS communication
│   │   │   ├── initWindowBridge.ts # Defines window.hodosBrowser.*
│   │   │   └── brc100.ts          # BRC-100 protocol methods
│   │   ├── pages/                 # React router pages (overlays)
│   │   │   ├── MainBrowserView.tsx # Main browser (/), webview + header
│   │   │   ├── WalletOverlayRoot.tsx # Wallet overlay (/wallet)
│   │   │   ├── SettingsOverlayRoot.tsx # Settings overlay (/settings)
│   │   │   ├── BackupOverlayRoot.tsx # Backup modal (/backup)
│   │   │   ├── BRC100AuthOverlayRoot.tsx # BRC-100 auth (/brc100-auth)
│   │   │   ├── HistoryPage.tsx    # History viewer (/history)
│   │   │   └── WalletPanelPage.tsx # Wallet panel (/wallet-panel)
│   │   ├── components/            # Reusable React components
│   │   │   ├── WalletPanel.tsx    # Wallet UI (balance, addresses)
│   │   │   ├── TransactionForm.tsx # Send/receive forms
│   │   │   ├── BRC100AuthModal.tsx # Auth approval modal
│   │   │   ├── AddressManager.tsx  # Address display/generation
│   │   │   ├── TabBar.tsx         # Browser tab bar
│   │   │   ├── HistoryPanel.tsx   # History UI
│   │   │   ├── SettingsMenu.tsx   # Settings menu
│   │   │   └── panels/            # Panel-specific components
│   │   ├── hooks/                 # React hooks
│   │   │   ├── useHodosBrowser.ts # Hook to call window.hodosBrowser
│   │   │   ├── useWallet.ts       # Wallet operations state
│   │   │   ├── useBalance.ts      # Balance polling
│   │   │   ├── useAddress.ts      # Address generation
│   │   │   ├── useHistory.ts      # Browser history
│   │   │   ├── useTransaction.ts  # Transaction handling
│   │   │   ├── useTabManager.ts   # Tab state
│   │   │   └── useKeyboardShortcuts.ts # Keyboard handling
│   │   ├── types/                 # TypeScript declarations
│   │   │   └── hodosBrowser.d.ts  # Global window.hodosBrowser interface
│   │   └── index.css
│   ├── vite.config.ts             # Vite bundler config
│   ├── package.json               # npm dependencies
│   └── tsconfig.json              # TypeScript config
│
├── cef-native/                    # C++ CEF browser shell
│   ├── cef_browser_shell.cpp      # Entry point; window creation, message loop
│   ├── CMakeLists.txt             # C++ build configuration
│   ├── include/                   # Header files
│   │   ├── handlers/
│   │   │   ├── simple_handler.h   # Browser process message routing
│   │   │   ├── simple_app.h       # CEF app initialization
│   │   │   └── simple_render_process_handler.h # V8 injection
│   │   └── core/
│   │       ├── HttpRequestInterceptor.h # HTTP routing to Rust
│   │       ├── WalletService.h    # Rust wallet process management
│   │       ├── TabManager.h       # Tab state tracking
│   │       ├── HistoryManager.h   # Browser history (SQLite)
│   │       ├── Logger.h           # Logging system
│   │       ├── NavigationHandler.h # URL bar navigation
│   │       └── BRC100Handler.h    # BRC-100 protocol handling
│   ├── src/
│   │   ├── handlers/
│   │   │   ├── simple_handler.cpp # Browser process implementation
│   │   │   ├── simple_app.cpp     # CEF app + overlay spawn
│   │   │   └── simple_render_process_handler.cpp # V8 implementation
│   │   └── core/
│   │       ├── HttpRequestInterceptor.cpp # HTTP intercept routing
│   │       ├── WalletService.cpp  # Wallet process spawning
│   │       ├── TabManager.cpp     # Tab management
│   │       ├── HistoryManager.cpp # History DB + API
│   │       ├── Logger.cpp         # Logging implementation
│   │       ├── BRC100Bridge.cpp   # HTTP client to Rust
│   │       ├── IdentityHandler.cpp # Identity operations
│   │       ├── AddressHandler.cpp # Address operations
│   │       ├── BRC100Handler.cpp  # BRC-100 operations
│   │       └── NavigationHandler.cpp # Navigation operations
│   └── build/                     # CMake output (generated)
│       └── bin/Release/HodosBrowserShell.exe
│
├── rust-wallet/                   # Rust wallet backend (Actix-web)
│   ├── src/
│   │   ├── main.rs                # Actix-web server entry; AppState setup
│   │   ├── handlers.rs            # HTTP endpoint handlers
│   │   ├── handlers/              # Grouped handler modules
│   │   │   └── certificate_handlers.rs # BRC-52 certificate ops
│   │   ├── crypto/                # Cryptographic modules
│   │   │   ├── mod.rs             # Crypto module interface
│   │   │   ├── brc42.rs           # Child key derivation (ECDH)
│   │   │   ├── brc43.rs           # Invoice number format
│   │   │   ├── signing.rs         # SHA-256, HMAC, BSV SIGHASH
│   │   │   ├── brc2.rs            # Symmetric encryption (AES-GCM)
│   │   │   ├── aesgcm_custom.rs   # Custom AES-GCM implementation
│   │   │   ├── keys.rs            # Key derivation utilities
│   │   │   └── ghash.rs           # GHASH primitive
│   │   ├── database/              # SQLite storage layer
│   │   │   ├── mod.rs             # Database module exports
│   │   │   ├── connection.rs      # SQLite connection + WalletDatabase
│   │   │   ├── migrations.rs      # Schema migration logic
│   │   │   ├── models.rs          # Data models (Wallet, Address, Utxo, etc.)
│   │   │   ├── wallet_repo.rs     # WalletRepository (master key storage)
│   │   │   ├── address_repo.rs    # AddressRepository (address derivation)
│   │   │   ├── utxo_repo.rs       # UtxoRepository (spendable outputs)
│   │   │   ├── certificate_repo.rs # CertificateRepository (BRC-52 certs)
│   │   │   ├── transaction_repo.rs # TransactionRepository (tx history)
│   │   │   ├── message_relay_repo.rs # MessageRelayRepository (BRC-33 relay)
│   │   │   ├── helpers.rs         # Database helpers (key lookup, derivation)
│   │   │   ├── parent_transaction_repo.rs # Parent tx tracking
│   │   │   ├── block_header_repo.rs # Block header proofs
│   │   │   ├── basket_repo.rs     # Output basket storage
│   │   │   ├── tag_repo.rs        # Output tag (metadata)
│   │   │   └── merkle_proof_repo.rs # Merkle proof tracking
│   │   ├── certificate/           # BRC-52 certificate management
│   │   │   ├── mod.rs             # Certificate module interface
│   │   │   ├── types.rs           # Certificate data types
│   │   │   ├── parser.rs          # Parse certificates
│   │   │   ├── verifier.rs        # Verify signatures
│   │   │   ├── selective_disclosure.rs # Selective disclosure logic
│   │   │   └── test_utils.rs      # Test fixtures
│   │   ├── transaction/           # Transaction building/signing
│   │   │   ├── mod.rs             # Transaction module interface
│   │   │   └── sighash.rs         # BSV ForkID SIGHASH
│   │   ├── script/                # Bitcoin script utilities
│   │   │   ├── mod.rs             # Script module interface
│   │   │   └── (script parsing/PushDrop BRC-48)
│   │   ├── action_storage.rs      # BRC-100 action persistence
│   │   ├── auth_session.rs        # BRC-100 session management
│   │   ├── message_relay.rs       # BRC-33 message relay
│   │   ├── domain_whitelist.rs    # Domain whitelist manager
│   │   ├── balance_cache.rs       # In-memory balance cache
│   │   ├── cache_sync.rs          # Background balance sync
│   │   ├── beef.rs                # BEEF transaction format
│   │   ├── beef_helpers.rs        # BEEF building utilities
│   │   ├── backup.rs              # Wallet backup/restore
│   │   ├── recovery.rs            # Wallet recovery from mnemonic
│   │   ├── utxo_sync.rs           # Background UTXO sync
│   │   ├── cache_errors.rs        # Error types for caching
│   │   ├── cache_helpers.rs       # Cache utility functions
│   │   └── bin/
│   │       └── extract_master_key.rs # CLI tool to extract key
│   ├── Cargo.toml                 # Rust dependencies
│   └── Cargo.lock                 # Dependency lock file
│
├── cef-binaries/                  # CEF 136 binaries (downloaded)
│   ├── include/                   # CEF header files
│   ├── libcef_dll/                # CEF wrapper library source
│   ├── Release/                   # CEF runtime binaries
│   └── Resources/                 # CEF resources (locales, etc.)
│
├── frontend/package.json
├── rust-wallet/Cargo.toml
├── CLAUDE.md                      # This project's Claude guidelines
└── .planning/codebase/            # GSD codebase analysis (this directory)
    ├── ARCHITECTURE.md            # Layer structure, data flow, abstractions
    └── STRUCTURE.md               # This file

```

## Directory Purposes

**frontend/src/:**
- Purpose: React UI code; never handles keys or signing
- Contains: Components, hooks, pages, routing, TypeScript types
- Key files: `App.tsx` (router), `main.tsx` (entry), `bridge/initWindowBridge.ts` (API defs)

**cef-native/:**
- Purpose: C++ browser shell; HTTP interception, V8 injection, window management
- Contains: CEF handlers, request interceptor, overlay spawning logic
- Key files: `cef_browser_shell.cpp` (entry), `src/handlers/simple_render_process_handler.cpp` (V8 injection)

**rust-wallet/src/:**
- Purpose: Wallet backend; crypto, signing, database, HTTP server
- Contains: Handlers, crypto modules, database repos, BRC protocol implementations
- Key files: `main.rs` (server entry), `handlers.rs` (endpoints), `crypto/brc42.rs` (key derivation)

**rust-wallet/src/database/:**
- Purpose: SQLite storage abstraction
- Contains: Repository patterns for wallet, address, UTXO, certificate, transaction storage
- Key files: `connection.rs` (WalletDatabase), `migrations.rs` (schema), `helpers.rs` (utilities)

**cef-binaries/:**
- Purpose: CEF 136 source and binaries (downloaded externally)
- Contains: CEF headers, wrapper library, runtime DLLs
- Note: Not generated; checked into repo for build consistency

## Key File Locations

**Entry Points:**

- Frontend React app: `frontend/src/main.tsx` - Creates React root, mounts App component
- Frontend router: `frontend/src/App.tsx` - Defines routes for main browser, overlays, settings, wallet, backup, BRC-100 auth
- C++ CEF shell: `cef-native/cef_browser_shell.cpp` - `main()` function, window creation, message loop
- Rust wallet server: `rust-wallet/src/main.rs` - Actix-web server initialization, database setup, route registration

**Configuration:**

- Frontend build: `frontend/vite.config.ts` - Vite bundler config; loads from localhost:5137
- Frontend TypeScript: `frontend/tsconfig.json` - Strict mode, JSX support
- C++ build: `cef-native/CMakeLists.txt` - Compiler flags, dependencies, link libraries
- Rust build: `rust-wallet/Cargo.toml` - Actix-web, serde, SQLite dependencies

**Core Logic:**

- Wallet API bridge: `frontend/src/bridge/initWindowBridge.ts` - Defines `window.hodosBrowser.wallet.*` methods
- HTTP interception: `cef-native/src/core/HttpRequestInterceptor.cpp` - Intercepts requests, routes to Rust or overlays
- V8 injection: `cef-native/src/handlers/simple_render_process_handler.cpp` - Injects `window.hodosBrowser` into render process
- Wallet handlers: `rust-wallet/src/handlers.rs` - `/getPublicKey`, `/signAction`, `/listCertificates` endpoints
- Crypto: `rust-wallet/src/crypto/brc42.rs` - BRC-42 child key derivation (master + counterparty → child)
- Database: `rust-wallet/src/database/connection.rs` - SQLite WalletDatabase connection management
- Messaging: `rust-wallet/src/auth_session.rs` - BRC-100 auth session state per domain

**Testing:**

- Not detected; project uses manual testing with user running browser

## Naming Conventions

**Files:**

- React components: `PascalCase.tsx` (e.g., `WalletPanel.tsx`, `BRC100AuthModal.tsx`)
- React hooks: `useKebabCase.ts` (e.g., `useHodosBrowser.ts`, `useBalance.ts`)
- React pages/overlays: `PascalCaseOverlayRoot.tsx` or `PascalCasePage.tsx` (e.g., `WalletOverlayRoot.tsx`, `HistoryPage.tsx`)
- C++ files: `snake_case.cpp` / `snake_case.h` (e.g., `simple_handler.cpp`, `http_request_interceptor.h`)
- C++ classes: `PascalCase` (e.g., `DomainVerifier`, `AsyncWalletResourceHandler`)
- Rust files: `snake_case.rs` (e.g., `brc42.rs`, `wallet_repo.rs`)
- Rust structs/traits: `PascalCase` (e.g., `WalletRepository`, `AuthSessionManager`)

**Directories:**

- Frontend: `src/components/`, `src/pages/`, `src/hooks/`, `src/types/`, `src/bridge/`
- C++: `include/handlers/`, `include/core/`, `src/handlers/`, `src/core/`
- Rust: `src/crypto/`, `src/database/`, `src/certificate/`, `src/transaction/`, `src/script/`

## Where to Add New Code

**New Feature:**

- Primary code: Implementation goes in corresponding layer
  - UI feature → `frontend/src/components/` or `frontend/src/pages/`
  - Backend logic → `rust-wallet/src/` (likely in `handlers.rs` or new module)
  - Cross-cutting (history, tabs) → `cef-native/src/core/` (e.g., `TabManager.cpp`)
- Tests: Project uses manual browser testing only
- Routes: Add to `frontend/src/App.tsx` if new overlay page; add HWND in `cef_browser_shell.cpp` if standalone overlay

**New Component/Module:**

- Implementation: `frontend/src/components/ComponentName.tsx` for UI; add import to page that uses it
- If reusable across pages: Place in `src/components/`; if page-specific: Place in page file or co-locate
- Types: Add to `src/types/componentName.ts` or `src/types/hodosBrowser.d.ts` if global

**Utilities:**

- Shared helpers: `frontend/src/bridge/` for window API helpers; `rust-wallet/src/` for backend utilities
- Hook utilities: `frontend/src/hooks/useUtility.ts` for React-specific helpers

**New Endpoint:**

- Location: Add handler function in `rust-wallet/src/handlers.rs`
- Registration: Register route in `rust-wallet/src/main.rs` using `.route()` builder
- V8 exposure: If callable from JavaScript, add method in `frontend/src/bridge/initWindowBridge.ts` and corresponding `cefMessage.send()` call in C++

**New Overlay/Modal:**

- React component: `frontend/src/pages/NewOverlayRoot.tsx` with router entry
- App.tsx update: Add `<Route path="/new-overlay" element={<NewOverlayRoot />} />`
- C++ updates:
  - Add HWND global in `cef_browser_shell.cpp` (e.g., `HWND g_new_overlay_hwnd = nullptr;`)
  - Add spawn function in `simple_app.cpp`
  - Register message handler in `simple_handler.cpp`

## Special Directories

**%APPDATA%/HodosBrowser/:**
- Purpose: Application data storage on Windows
- Contains: User wallet database, browser data, configuration
- Generated: Yes (created at runtime)
- Committed: No

**%APPDATA%/HodosBrowser/wallet/:**
- Purpose: Wallet-specific storage
- Contains: `wallet.db` (SQLite database with master key, addresses, UTXOs), domain whitelist JSON
- Generated: Yes
- Committed: No

**%APPDATA%/HodosBrowser/Default/:**
- Purpose: CEF browser data (history, cookies, bookmarks separate from wallet)
- Contents: Standard Chromium user data directory
- Generated: Yes (by CEF)
- Committed: No

**frontend/dist/:**
- Purpose: Vite build output
- Generated: Yes (`npm run build`)
- Committed: No (should be in .gitignore)

**cef-native/build/:**
- Purpose: CMake build output for C++
- Generated: Yes (`cmake --build .`)
- Committed: No

**rust-wallet/target/:**
- Purpose: Cargo build output
- Generated: Yes (`cargo build`)
- Committed: No

---

*Structure analysis: 2026-01-28*
