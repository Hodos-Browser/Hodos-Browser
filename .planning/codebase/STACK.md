# Technology Stack

**Analysis Date:** 2026-01-24

## Languages

**Primary:**
- TypeScript 5.8.3 - All frontend application code (`frontend/src/`)
- Rust 2021 - Wallet backend, cryptography, database layer (`rust-wallet/src/`)
- C++17 - CEF browser shell, V8 injection, HTTP interception (`cef-native/`)

**Secondary:**
- JavaScript - Build scripts, Vite configuration (`frontend/vite.config.ts`)

## Runtime

**Environment:**
- Node.js 18+ - Frontend development server
- Rust with Tokio async runtime - Wallet backend HTTP server
- CEF (Chromium Embedded Framework) 136 - Browser engine

**Package Manager:**
- npm - Frontend dependencies (`frontend/package.json`)
- Lockfile: `frontend/package-lock.json` present
- Cargo - Rust dependencies (`rust-wallet/Cargo.toml`)
- Lockfile: `rust-wallet/Cargo.lock` present
- vcpkg/Homebrew - C++ dependencies (platform-specific)

## Frameworks

**Core:**
- React 19.1.0 - UI framework (`frontend/package.json`)
- Actix-web 4.9 - Rust HTTP server framework (`rust-wallet/Cargo.toml`)
- CEF 136 - Chromium-based browser engine (`cef-native/CMakeLists.txt`)

**Testing:**
- Rust built-in `#[test]` - Unit and integration tests (`rust-wallet/tests/`)
- No frontend test framework configured

**Build/Dev:**
- Vite 6.3.5 - Frontend bundling and dev server (`frontend/vite.config.ts`)
- TypeScript Compiler 5.8.3 - Type checking and compilation
- CMake 3.15+ - C++ build system (`cef-native/CMakeLists.txt`)
- ESLint 9.25.0 - TypeScript/React linting (`frontend/eslint.config.js`)

## Key Dependencies

**Critical:**
- secp256k1 0.28 - Elliptic curve cryptography for Bitcoin signing (`rust-wallet/Cargo.toml`)
- bip39 2.0 / bip32 0.5 - HD wallet implementation (mnemonic → keys) (`rust-wallet/Cargo.toml`)
- rusqlite 0.30 - SQLite database driver for wallet persistence (`rust-wallet/Cargo.toml`)
- Material-UI 7.1.1 - React component library (`frontend/package.json`)
- React Router DOM 7.6.1 - Frontend routing for overlays (`frontend/package.json`)

**Infrastructure:**
- actix-cors 0.7 - CORS middleware for Rust HTTP server (`rust-wallet/Cargo.toml`)
- reqwest 0.11 - HTTP client for blockchain API calls (`rust-wallet/Cargo.toml`)
- tokio 1.0 (full features) - Async runtime for Rust (`rust-wallet/Cargo.toml`)
- nlohmann_json - C++ JSON parsing (`cef-native/CMakeLists.txt`)
- SQLite3 - C++ history database (`cef-native/src/core/HistoryManager.cpp`)

## Configuration

**Environment:**
- No .env files - Environment-independent configuration
- Wallet directory: `%APPDATA%/HodosBrowser/wallet/` (Windows), `~/Library/Application Support/HodosBrowser/wallet/` (macOS)
- Database location: `wallet.db` in wallet directory (`rust-wallet/src/main.rs`)

**Build:**
- `frontend/vite.config.ts` - Vite dev server (port 5137, CORS enabled)
- `frontend/tsconfig.app.json` - TypeScript strict mode, ES2020 target
- `rust-wallet/Cargo.toml` - Rust dependencies and features
- `cef-native/CMakeLists.txt` - C++ build configuration, vcpkg integration

## Platform Requirements

**Development:**
- Windows/macOS (primary platforms)
- PowerShell or Terminal
- Visual Studio 2022 (Windows) or Xcode (macOS) for C++ builds
- Rust toolchain (rustc, cargo)
- Node.js 18+
- vcpkg (Windows) or Homebrew (macOS) for C++ dependencies

**Production:**
- Native desktop application (Windows/macOS)
- Distributed as compiled executable (`HodosBrowserShell.exe` / `HodosBrowser.app`)
- CEF binaries bundled with application
- Rust wallet compiled to release binary (included in distribution)
- Frontend compiled to static assets served by CEF

---

*Stack analysis: 2026-01-24*
*Update after major dependency changes*
