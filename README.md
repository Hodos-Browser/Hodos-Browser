# HodosBrowser

A Web3 browser built on CEF (Chromium Embedded Framework) with a native Rust wallet backend for Bitcoin SV authentication, micropayments, and smart contracts.

**Status** (updated 2026-08-03): Active development. BRC-100 authentication (BRC-103/104) and transactions (`createAction`/`signAction`, BRC-29 payments, BEEF/SPV) are shipping, alongside the domain permission system, the permission/auto-approve engine, OS-keystore auto-unlock and mnemonic recovery. For the current sprint state, see the Active sprint status table in `CLAUDE.md`.

---

## Architecture

```
React Frontend (Vite dev server, port 5137)
    | window.hodosBrowser.*  ->  "wallet_call" CefProcessMessage (IPC bridge)
    v
C++ CEF Shell
    | HTTP interception -> 127.0.0.1:31301   (31401 when HODOS_DEV=1)
    v
Rust Wallet Backend (Actix-web, SQLite, BRC-100)
    v
Bitcoin SV Blockchain (WhatsOnChain, GorillaPool)
```

| Layer | Tech | Responsibility |
|-------|------|----------------|
| Frontend | React, Vite, TypeScript, MUI | UI and user interaction. Performs no signing and never holds a derived private key; the BIP39 recovery phrase is handled only inside the isolated wallet overlay. Layer doc: `frontend/src/CLAUDE.md` |
| CEF Shell | C++17 | Browser engine, V8 injection, HTTP interception, permission-row caching, prompt overlays. Builds partial permission context and forwards it to Rust. Layer doc: `cef-native/CLAUDE.md` |
| Wallet | Rust, Actix-web, SQLite | Crypto, signing, key material, BRC-100 protocol, domain permissions, and the permission decision engine. Layer doc: `rust-wallet/src/CLAUDE.md` |

**Ports** — wallet `31301` release / `31401` under `HODOS_DEV=1`; adblock engine `31302` / `31402`. `cef-native/include/core/PortConfig.h` is the single source of truth (`WalletPort()`, `AdblockPort()`, `WalletUrl()`); never hardcode a port literal.

**Permission engine** — the auto-approve decision cascade lives in Rust: `rust-wallet/crates/hodos_permission_engine` (`decide()` in `src/lib.rs`, the Matrix C cap/rate cascade in `src/matrix_c.rs`), wrapped by `rust-wallet/src/permission_service/` and wired as Actix middleware in `rust-wallet/src/main.rs`. The C++ shell renders whichever modal Rust asks for; it does not decide. Default spending limits are $1.00 per transaction and $10.00 per session.

**Process-per-overlay** — every panel and prompt overlay runs as a separate CEF subprocess with an isolated V8 context. Overlay roster: `cef-native/CLAUDE.md`.

---

## Quick Start (Windows)

**Prerequisites**: VS 2022 (MSVC), vcpkg, Rust, Node.js 18+, CEF binaries

The CEF shell launches the Rust wallet and the adblock engine as child processes if they are not already running, so only the frontend dev server has to be started by hand. Run a backend standalone when you are iterating on it — the launcher scripts set `HODOS_DEV=1`, which is what keeps dev runs out of the installed app's database.

```powershell
# 1. Frontend dev server (required)
cd frontend && npm install && npm run dev

# 2. CEF browser — builds, then spawns wallet + adblock
cd cef-native
.\win_build_run.ps1        # Windows
./mac_build_run.sh         # Mac

# Optional: run a backend standalone first (from project root)
.\dev-wallet.ps1   / ./dev-wallet.sh
.\dev-adblock.ps1  / ./dev-adblock.sh
```

**Build from source**: `build-instructions/BUILD_INSTRUCTIONS.md` is the entry point; `WINDOWS_BUILD_INSTRUCTIONS.md` and `MACOS_BUILD_INSTRUCTIONS.md` in the same folder cover first-time platform setup (CEF binaries, CMake, vcpkg).

---

## What's Working

### Browser
- Navigation, tabs, cookies, history, bookmarks; process-per-tab isolation
- HTTP request interception for BRC-100 endpoints
- Domain permission system with per-site USD spending limits, plus prompt overlays for payment confirmation, certificate disclosure and rate limiting
- Ad & tracker blocking — a standalone Rust HTTP service (`adblock-engine`, crate `hodos-adblock`, wrapping Brave's `adblock` crate) that the shell queries over localhost. Not an in-process FFI binding
- Third-party cookie blocking and fingerprint farbling
- Camera / microphone / geolocation permission prompts
- SSL certificate handling + secure connection indicator (padlock)
- Downloads with progress and pause/resume/cancel; find-in-page; context menus; JS dialog handling (beforeunload trap suppression); keyboard shortcuts
- QR code scanning via screen capture
- Settings persistence and Chrome/Brave/Edge profile import
- Feature-level detail: `cef-native/CLAUDE.md` and the `src/core/` + `include/core/` layer docs

### Wallet
- HD wallet (BIP39 mnemonic, BRC-42 self-derivation, legacy BIP32 recovery)
- BRC-103/104 mutual authentication
- BRC-29 payment protocol
- BRC-33 message relay
- BEEF/SPV transaction format with merkle proofs
- PIN encryption (AES-256-GCM, PBKDF2) + OS-keystore auto-unlock
- Mnemonic recovery with blockchain UTXO scanning; file-based backup and restore
- Background monitor — a task scheduler covering merkle-proof acquisition, crash recovery and UTXO sync. Task roster: `rust-wallet/src/monitor/CLAUDE.md`
- BSV/USD price cache — WhatsOnChain -> CoinGecko -> MEXC fallback chain, in-memory TTL, stale-value fallback, and a persisted SQLite cold-start cache
- Endpoint and module rosters: `rust-wallet/src/CLAUDE.md`

### What's Next

Current and upcoming work is tracked in the Active sprint status table in `CLAUDE.md` and under `development-docs/` (`Sigma-BRC121-Sprint/`, `Final-MVP-Sprint/`, `DevOps-CICD/`).

---

## Storage

| Context | Windows | macOS |
|---------|---------|-------|
| Installed app | `%APPDATA%/HodosBrowser/` | `~/Library/Application Support/HodosBrowser/` |
| Dev builds (`HODOS_DEV=1`) | `%APPDATA%/HodosBrowserDev/` | `~/Library/Application Support/HodosBrowserDev/` |

Wallet DB: `<storage>/wallet/wallet.db` (SQLite). Browser data (history, bookmarks, cookies): `<storage>/Default/`.

Dev binaries refuse to start unless `HODOS_DEV=1` is set, so a dev run can never write to the installed app's database. See the Dev Runbook in `CLAUDE.md`.

---

## Project Structure

Top-level layout only. Every directory that carries a `CLAUDE.md` owns its own file-level inventory — start there rather than here.

```
HodosBrowser/
|-- cef-native/          C++ CEF browser shell: entry point, overlays, HTTP interception
|-- rust-wallet/         Rust wallet backend: crypto, signing, BRC-100, SQLite, permission engine
|-- adblock-engine/      Standalone Rust ad & tracker blocking HTTP service
|-- frontend/            React + Vite + TypeScript UI: main window and overlay roots
|-- installer/           Windows installer script (Inno Setup)
|-- scripts/             Build, release and maintenance scripts
|-- external/            Vendored third-party dependencies (WinSparkle)
|-- cef-binaries/        Downloaded CEF distribution (not in git)
|-- demos/               Demo pages and sample dApps
|-- test-fixtures/       Test data and fixture dApps
|-- reference/           External specs and reference material
|-- build-instructions/  Platform build guides
|-- development-docs/    Active sprint plans, research, architecture docs
|-- archived-docs/       Completed and superseded sprints
|-- dist/                Packaged build output (not in git)
```

---

## Documentation

| Document | Purpose |
|----------|---------|
| `CLAUDE.md` | Project context for AI assistants (invariants, key files, dev runbook, sprint status) |
| `PROJECT_OVERVIEW.md` | Comprehensive architecture reference |
| `THE_WHY.md` | Rationale for Rust, CEF, and native wallet choices |
| `SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md` | Process isolation security model |
| `development-docs/` | Active sprints, architecture reference, DevOps/CI-CD process docs |
| `archived-docs/` | Completed and superseded sprints, incl. `IMPLEMENTATION_STATUS.md` (historical implementation log) and `browser-core/` (MVP sprint plan + gap analysis) |

---

## Security

- **Signing keys never leave the Rust process.** No EC private key is ever returned to JavaScript, and no signing happens there. The BIP39 recovery phrase is the one deliberate exception: it is shown once at wallet creation so the user can record it, and thereafter only through PIN re-verification in the wallet overlay. It is never reachable from web content — the wallet's CORS allowlist admits only Hodos's own local UI origins.
- **Process isolation** — wallet runs as a separate process from the browser
- **At-rest encryption** — the mnemonic is sealed by the OS keystore: Windows DPAPI, macOS Keychain via `security-framework` (`rust-wallet/src/crypto/dpapi.rs`). Linux and other platforms fall back to a stub with no auto-unlock
- **PIN encryption** — AES-256-GCM with PBKDF2 (600K iterations, `rust-wallet/src/crypto/pin.rs`)
- **Domain permissions** — per-site approval with USD spending limits; defaults $1.00 per transaction, $10.00 per session
- **Defense in depth** — Rust owns the permission decision; the C++ shell caches rows and renders prompts but cannot approve on its own
- **Memory safety** — Rust ownership model; no `unsafe` in the Rust wallet except the two Windows DPAPI FFI blocks in `crypto/dpapi.rs` (`CryptProtectData` / `CryptUnprotectData`)

---

## License

MIT — see [`LICENSE`](./LICENSE). The code is open source. Three notices sit alongside the grant rather than inside it, so that `LICENSE` stays the unmodified MIT text and is detected as such:

- [`TRADEMARK.md`](./TRADEMARK.md) — "Hodos", "Hodos Browser", and the logo are trademarks of Marston Enterprises. MIT grants rights to the code, not to the name or branding. Forks must rename and rebrand. Same approach as Firefox and Brave.
- [`NOTICE.md`](./NOTICE.md) — this build includes a **1000-satoshi service fee** to a Marston Enterprises address on every outgoing transaction (backup-token transactions excepted), which funds development. Forks must either redirect it or disclose it.
- [`COPYRIGHT`](./COPYRIGHT) — third-party components are not relicensed by the MIT grant: Chromium and the Chromium Embedded Framework carry their own terms, as do the bundled adblock filter lists and the disconnect.me entity list.
