# Technology Stack

**Analysis Date:** 2026-01-28

## Languages

**Primary:**
- TypeScript 5.8.3 - React frontend UI layer (`frontend/`)
- Rust 2021 edition - Wallet backend and cryptographic operations (`rust-wallet/`)
- C++ 17 - CEF browser shell and process isolation (`cef-native/`)

**Secondary:**
- Objective-C++ (Objective-C++) - macOS-specific implementations in CEF shell (`cef-native/src/handlers/my_overlay_render_handler.mm`, `cef-native/cef_browser_shell_mac.mm`, `cef-native/mac/process_helper_mac.mm`)

## Runtime

**Environment:**
- Node.js 18+ - Frontend build and dev server
- Rust 1.70+ (inferred from Cargo.toml edition="2021") - Wallet backend runtime
- Chromium Embedded Framework (CEF) 136 - Browser engine runtime

**Package Manager:**
- npm 9+ - Frontend TypeScript/React dependencies
- Cargo 1.70+ - Rust wallet dependencies
- vcpkg - C++ dependency management (OpenSSL, nlohmann-json, sqlite3)

**Lockfiles:**
- `frontend/package-lock.json` - npm lockfile present
- `rust-wallet/Cargo.lock` - Cargo lockfile present

## Frameworks

**Core:**
- React 19.1.0 - UI component framework (`frontend/src/`)
- Vite 6.3.5 - TypeScript/React build tool and dev server
- React Router DOM 7.6.1 - Client-side routing (`frontend/src/pages/`, `frontend/src/App.tsx`)
- Material-UI (MUI) 7.1.1 - UI component library (@emotion/react 11.14.0, @emotion/styled 11.14.0)
- Actix-web 4.9 - Rust async HTTP framework for wallet backend (`rust-wallet/src/handlers.rs`, `rust-wallet/src/main.rs`)
- CEF (Chromium Embedded Framework) 136 - Embedded browser engine for Windows and macOS

**Testing:**
- Not detected - no testing framework configured (no Jest, Vitest, or Cargo test framework identified in dependencies)

**Build/Dev:**
- Vite 6.3.5 - Build bundler and dev server
- TypeScript 5.8.3 - TypeScript compiler for frontend
- @vitejs/plugin-react 4.4.1 - React fast refresh support
- CMake 3.15+ - C++ build system (`cef-native/CMakeLists.txt`)
- Visual Studio 2022 (MSVC compiler) - Windows C++ compilation
- Clang - macOS C++ compilation

## Key Dependencies

**Critical (Cryptography & Security):**
- secp256k1 0.28 - ECDSA signing library for Bitcoin transactions (`rust-wallet/src/crypto/`)
- sha2 0.10 - SHA256 hashing for transaction signing (`rust-wallet/src/crypto/signing.rs`)
- hmac 0.12 - HMAC verification for authentication
- aes-gcm 0.10 - AES-GCM encryption for private key storage (`rust-wallet/src/crypto/aesgcm_custom.rs`)
- bip39 2.0 - BIP39 mnemonic seed generation (`rust-wallet/src/recovery.rs`)
- bip32 0.5 - BIP32 HD wallet key derivation
- ripemd 0.1 - RIPEMD160 hashing for address generation

**Infrastructure (HTTP & Async):**
- actix-web 4.9 - REST API server (`rust-wallet/src/handlers.rs`)
- actix-cors 0.7 - CORS middleware for wallet endpoints
- reqwest 0.11 - HTTP client for blockchain API calls (`rust-wallet/src/utxo_fetcher.rs`, `rust-wallet/src/cache_helpers.rs`)
- tokio 1.x (full features) - Async runtime for Rust

**Data Storage & Serialization:**
- rusqlite 0.30 (bundled) - SQLite driver for wallet database (`rust-wallet/src/database/`)
- serde 1.0 - Serialization framework (with derive)
- serde_json 1.0 (preserve_order) - JSON serialization maintaining key order

**Utility:**
- chrono 0.4 - Date/time handling with serde support
- base64 0.22 - Base64 encoding/decoding
- hex 0.4 - Hex encoding/decoding for transaction data
- uuid 1.x - UUID generation for certificate tracking
- rand 0.8 - Random number generation for nonces
- dirs 5.0 - OS-specific directory paths (AppData on Windows)
- log 0.4 - Logging facade
- env_logger 0.11 - Logging implementation
- thiserror 1.0 - Error type derivation

## Configuration

**Environment:**
- `APPDATA` environment variable - Windows user application data directory
  - Wallet DB: `%APPDATA%/HodosBrowser/wallet/wallet.db`
  - Browser data: `%APPDATA%/HodosBrowser/Default/`
- `VCPKG_ROOT` environment variable - C++ dependency manager root (required for CMake)
- `RUST_LOG` - Log level control via env_logger (default "info")

**Build Configuration:**
- `frontend/vite.config.ts` - Vite dev server: `localhost:5137`, CORS enabled
- `frontend/eslint.config.js` - ESLint rules (React Hooks, React Refresh)
- `frontend/tsconfig.app.json` - TypeScript strict mode, ES2020 target, no unused warnings
- `cef-native/CMakeLists.txt` - Platform-specific (Windows x64, macOS arm64/x64)
  - Windows: MSVC runtime library, vcpkg OpenSSL/nlohmann-json/sqlite3
  - macOS: Homebrew packages (nlohmann-json, sqlite3), system frameworks (Cocoa, AppKit, Foundation, CoreGraphics, QuartzCore, curl)
- `rust-wallet/Cargo.toml` - Standard Rust edition 2021

## Platform Requirements

**Development:**
- Windows:
  - PowerShell (execution policy: RemoteSigned minimum)
  - Visual Studio 2022 Community+ (MSVC toolchain, C++ workload)
  - vcpkg - C++ package manager
  - Rust toolchain (via rustup)
  - Node.js 18+
  - CEF binaries (136) manually downloaded from https://cef-builds.spotifycdn.com/

- macOS:
  - Xcode with Command Line Tools
  - Homebrew (for OpenSSL, Node.js, curl)
  - Rust toolchain (via rustup, supports arm64 and x86_64)
  - Node.js 18+
  - CMake 3.15+
  - CEF binaries (136) manually downloaded from https://cef-builds.spotifycdn.com/

**Production:**
- Windows 7+ (CEF 136 target)
- macOS 10.15+ (deployment target in CMakeLists.txt)
- Requires: Rust wallet backend running on `localhost:3301` and frontend dev server on `localhost:5137` during development

## Build Commands

**Frontend (TypeScript/React/Vite):**
```bash
cd frontend
npm install              # Install dependencies
npm run dev              # Dev server on localhost:5137
npm run build            # TypeScript + Vite production build to dist/
npm run lint             # ESLint validation
```

**Rust Wallet:**
```bash
cd rust-wallet
cargo build --release    # Production binary
cargo run --release      # Run on localhost:3301
cargo test               # Unit/integration tests
cargo check              # Type check without building
```

**CEF Browser (C++/CMake):**
```bash
cd cef-native
cmake -S . -B build -G "Visual Studio 17 2022" -A x64 \
  -DCMAKE_TOOLCHAIN_FILE=[vcpkg_root]/scripts/buildsystems/vcpkg.cmake
cmake --build build --config Release
# Output: build/bin/Release/HodosBrowserShell.exe
```

---

*Stack analysis: 2026-01-28*
