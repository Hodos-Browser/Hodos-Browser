# Technology Stack

**Analysis Date:** 2026-01-20

## Languages

**Primary:**
- TypeScript 5.8.3 - All frontend application code (`frontend/package.json`)
- Rust (2021 edition) - Wallet backend (`rust-wallet/Cargo.toml`)
- C++17 - CEF browser shell (`cef-native/CMakeLists.txt`)

**Secondary:**
- JavaScript - Build configs, ESLint config (`frontend/eslint.config.js`)
- Objective-C++ - macOS-specific CEF handlers (`cef-native/src/core/TabManager_mac.mm`)

## Runtime

**Environment:**
- Node.js 18+ - Frontend build tooling (implied by ES2020 target)
- Rust stable (2021 edition) - Async runtime via Tokio
- CEF 136 - Chromium browser engine (Windows & macOS)

**Package Managers:**
- npm - Frontend (`frontend/package-lock.json`)
- Cargo - Rust backend (`rust-wallet/Cargo.lock`)
- CMake 3.15+ with vcpkg - C++ dependencies (`cef-native/CMakeLists.txt`)

## Frameworks

**Core:**
- React 19.1.0 - UI framework (`frontend/package.json`)
- Actix-web 4.9 - Rust HTTP server on port 3301 (`rust-wallet/Cargo.toml`)
- CEF 136 - Chromium Embedded Framework (`cef-binaries/`)

**Testing:**
- Rust built-in `#[test]` - Unit tests for crypto/transaction modules
- No frontend test framework configured

**Build/Dev:**
- Vite 6.3.5 - Frontend bundling and dev server (`frontend/vite.config.ts`)
- TypeScript 5.8.3 - Compilation with strict mode (`frontend/tsconfig.app.json`)
- CMake - C++ build system with MSVC (Windows) / Clang (macOS)

## Key Dependencies

**Critical (Cryptography):**
- `secp256k1` 0.28 with `rand-std` - ECDSA signing for Bitcoin (`rust-wallet/Cargo.toml`)
- `bip39` 2.0 - Mnemonic seed generation (`rust-wallet/Cargo.toml`)
- `bip32` 0.5 - HD wallet key derivation (`rust-wallet/Cargo.toml`)
- `sha2` 0.10 - SHA-256 hashing (`rust-wallet/Cargo.toml`)
- `ripemd` 0.1 - RIPEMD-160 for Bitcoin addresses (`rust-wallet/Cargo.toml`)
- `aes-gcm` 0.10 - AES encryption for BRC-2 (`rust-wallet/Cargo.toml`)
- `hmac` 0.12 - HMAC-SHA256 authentication (`rust-wallet/Cargo.toml`)

**Infrastructure:**
- `rusqlite` 0.30 (bundled) - SQLite database (`rust-wallet/Cargo.toml`)
- `tokio` 1 (full features) - Async runtime (`rust-wallet/Cargo.toml`)
- `reqwest` 0.11 - HTTP client for blockchain APIs (`rust-wallet/Cargo.toml`)
- `actix-cors` 0.7 - CORS middleware (`rust-wallet/Cargo.toml`)
- `serde_json` 1.0 with `preserve_order` - JSON parsing (`rust-wallet/Cargo.toml`)

**Frontend:**
- `@mui/material` 7.1.1 - UI component library (`frontend/package.json`)
- `@emotion/react` 11.14.0 - CSS-in-JS styling (`frontend/package.json`)
- `react-router-dom` 7.6.1 - Client-side routing (`frontend/package.json`)

**C++ (via vcpkg/Homebrew):**
- OpenSSL - TLS/HTTPS support
- nlohmann_json - JSON parsing
- SQLite3 - Browser history database

## Configuration

**Environment:**
- No .env files - Configuration via hardcoded endpoints and environment variables
- `APPDATA` env var for wallet directory on Windows
- `env_logger` for Rust logging (default "info" level)
- CORS: allow-any-origin for development (`rust-wallet/src/main.rs`)

**Build:**
- `frontend/vite.config.ts` - Dev server on port 5137, React plugin, CORS enabled
- `frontend/tsconfig.app.json` - ES2020 target, strict mode, JSX support
- `frontend/eslint.config.js` - ESLint v9+ flat config with typescript-eslint
- `cef-native/CMakeLists.txt` - Platform-specific triplets, vcpkg integration

## Platform Requirements

**Development:**
- Windows: MSVC (VS 2022), vcpkg, PowerShell
- macOS: Clang, Homebrew (sqlite3, nlohmann-json), deployment target 10.15+
- Both: Node.js 18+, Rust stable, CMake 3.15+

**Production:**
- Windows: `%APPDATA%/HodosBrowser/` for data storage
- macOS: `~/Library/Application Support/HodosBrowser/` for data storage
- Wallet database: `wallet/wallet.db` (SQLite)
- Browser data: `Default/` directory (history, bookmarks, cookies)

---

*Stack analysis: 2026-01-20*
*Update after major dependency changes*
