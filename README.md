# HodosBrowser

A Web3 browser built on CEF (Chromium Embedded Framework) with a native Rust wallet backend for Bitcoin SV authentication, micropayments, and smart contracts.

**Status**: Active development (February 2026). BRC-100 Groups A & B complete. Domain permission system, auto-approve engine, DPAPI auto-unlock, mnemonic recovery all shipping. Browser core MVP sprints in progress.

---

## Architecture

```
React Frontend (Port 5137)
    | window.hodosBrowser.*
    v
C++ CEF Shell (CEF 136)
    | HTTP interception -> localhost:3301
    v
Rust Wallet Backend (Port 3301)
    | Actix-web, SQLite, BRC-100
    v
Bitcoin SV Blockchain (WhatsOnChain, GorillaPool)
```

| Layer | Tech | Responsibility |
|-------|------|----------------|
| Frontend | React, Vite, TypeScript, MUI | UI, user interactions; never handles keys or signing |
| CEF Shell | C++17, CEF 136 | Browser engine, V8 injection, HTTP interception, domain permissions, auto-approve engine |
| Wallet | Rust, Actix-web, SQLite | Crypto, signing, keys, BRC-100 protocol; private keys never leave this process |

**Process-per-overlay**: Settings, Wallet Panel, Backup Modal, BRC-100 Auth, and Notification overlays each run as separate CEF subprocesses with isolated V8 contexts.

---

## Quick Start (Windows)

**Prerequisites**: VS 2022 (MSVC), vcpkg, Rust, Node.js 18+, CEF binaries

All three must be running. Dev launcher scripts set `HODOS_DEV=1` to isolate dev data from the installed app:

```powershell
# 1. Rust wallet (from project root)
.\dev-wallet.ps1           # Windows
./dev-wallet.sh            # Mac

# 2. Frontend
cd frontend && npm install && npm run dev

# 3. CEF browser
cd cef-native
.\win_build_run.ps1        # Windows
./mac_build_run.sh         # Mac
```

**Build from source**: See `build-instructions/BUILD_INSTRUCTIONS.md` for first-time setup (CEF binaries, CMake, vcpkg).

---

## What's Working

### Browser
- Navigation, tabs, cookies, history, bookmarks
- Process-per-tab isolation
- HTTP request interception for BRC-100 endpoints
- Domain permission system with per-site spending limits
- Auto-approve engine (rate limiting, session tracking, USD conversion)
- Notification overlay (payment confirmation, certificate disclosure, rate limiting)
- SSL certificate handling + secure connection indicator (padlock)
- Download handler with progress tracking, pause/resume/cancel
- Find-in-page (Ctrl+F) with match count and yellow highlight
- Context menus (Back/Forward/Reload, Copy/Cut/Paste, Save Image, Open in New Tab, View Source)
- JS dialog handling (beforeunload trap suppression)
- Keyboard shortcuts (Ctrl+H/J/D, Alt+Left/Right back/forward)

### Wallet
- HD wallet (BIP39 mnemonic, BRC-42 self-derivation, legacy BIP32 recovery)
- BRC-100 Groups A & B (authentication + transactions)
- BRC-103/104 mutual authentication
- BRC-29 payment protocol
- BRC-33 message relay
- BEEF/SPV transaction format with merkle proofs
- PIN encryption (AES-256-GCM, PBKDF2) + DPAPI auto-unlock
- Mnemonic recovery with blockchain UTXO scanning
- File-based backup and restore
- Background monitor (7 tasks: proof acquisition, crash recovery, UTXO sync)
- BSV/USD price cache (CryptoCompare + CoinGecko fallback)

### What's Next (MVP Roadmap)
- Camera/mic/geolocation permission prompts
- Ad & tracker blocking (adblock-rust FFI)
- Light wallet polish (QR codes, button states, transaction progress)
- Settings persistence + profile import
- Third-party cookie blocking + fingerprinting protection

See `development-docs/browser-core/implementation-plan.md` for the full sprint plan.

---

## Storage

| Platform | Location |
|----------|----------|
| Windows | `%APPDATA%/HodosBrowser/` |
| macOS | `~/Library/Application Support/HodosBrowser/` |

Wallet DB: `<storage>/wallet/wallet.db` (SQLite). Browser data (history, bookmarks, cookies): `<storage>/Default/`.

---

## Project Structure

```
HodosBrowser/
|-- cef-native/              C++ CEF browser shell
|   |-- src/core/            HTTP interception, domain permissions, session manager
|   |-- src/handlers/        CEF event handlers (SimpleHandler, render process)
|   |-- include/core/        PendingRequestManager, SessionManager, BSVPriceCache
|
|-- rust-wallet/             Rust wallet backend (Port 3301)
|   |-- src/handlers.rs      68+ HTTP endpoint handlers
|   |-- src/crypto/          11 modules: BRC-42/43, DPAPI, PIN, AES-GCM, signing
|   |-- src/database/        23 files, 18+ repositories (SQLite)
|   |-- src/monitor/         Background task scheduler (7 tasks)
|   |-- src/transaction/     BSV ForkID SIGHASH, transaction types
|   |-- src/price_cache.rs   BSV/USD price with dual-provider fallback
|   |-- src/recovery.rs      Mnemonic recovery + BIP32 legacy derivation
|
|-- frontend/                React + Vite + TypeScript UI
|   |-- src/components/      Wallet panel, transaction form, domain permissions
|   |-- src/hooks/           useHodosBrowser, useBalance, useBackgroundBalancePoller
|   |-- src/pages/           Overlay roots (wallet, settings, BRC100 auth, notification)
|   |-- src/bridge/          window.hodosBrowser API bridge
|
|-- development-docs/        Sprint plans, research, architecture docs
|   |-- browser-core/        MVP gap analysis + implementation plan
|   |-- Final-MVP-Sprint/    Active sprint: testing, optimization, security, macOS port
|   |-- UX_UI/               Wallet UX phase tracker
|
|-- build-instructions/      Platform-specific build guides
```

---

## Documentation

| Document | Purpose |
|----------|---------|
| `CLAUDE.md` | Project context for AI assistants (invariants, key files, architecture) |
| `PROJECT_OVERVIEW.md` | Comprehensive architecture reference |
| `THE_WHY.md` | Rationale for Rust, CEF, and native wallet choices |
| `SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md` | Process isolation security model |
| `IMPLEMENTATION_STATUS.md` | Detailed implementation log (all phases) |
| `development-docs/browser-core/` | MVP sprint plan and gap analysis |
| `development-docs/Final-MVP-Sprint/` | Active sprint (testing, optimization, security, macOS port) |

---

## Security

- **Private keys never in JavaScript** — all signing happens in Rust
- **Process isolation** — wallet runs as separate process from browser
- **DPAPI encryption** — mnemonic encrypted at OS level (Windows); macOS Keychain planned
- **PIN encryption** — AES-256-GCM with PBKDF2 (600K iterations)
- **Domain permissions** — per-site approval with spending limits (USD)
- **Defense in depth** — C++ auto-approve engine + Rust-side permission checks
- **Memory safety** — Rust ownership model, no `unsafe` in key-handling code

---

## License

Proprietary. All rights reserved.
