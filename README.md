# HodosBrowser

A Web3 browser built on CEF (Chromium Embedded Framework) with a native Rust wallet backend for Bitcoin SV authentication and micropayments. Apps can build transactions with custom locking scripts through `createAction`.

**Status** (updated 2026-10-16): Active development. Hodos implements BRC-100 (Unified, Vendor-Neutral, Unchanging, and Open BSV Blockchain Standard Wallet-to-Application Interface). Authentication (BRC-103, Peer-to-Peer Mutual Authentication and Certificate Exchange Protocol, carried over BRC-104, HTTP Transport for BRC-103 Mutual Authentication) and transactions (`createAction`/`signAction`, BRC-29 payments, BEEF/SPV) are shipping, alongside the domain permission system, the permission/auto-approve engine, OS-keystore auto-unlock and mnemonic recovery. One gap in that list: the client does not check a BRC-103 peer's signatures outside the certificate handshake (see [Known issues](#known-issues)).

---

## Architecture

```
React Frontend (Vite dev server on port 5137 in dev; release builds serve the same origin from disk)
    | window.hodosBrowser.*  ->  "wallet_call" CefProcessMessage (IPC bridge)
    v
C++ CEF Shell
    | HTTP interception -> 127.0.0.1:31301   (31401 when HODOS_DEV=1)
    v
Rust Wallet Backend (Actix-web, SQLite, BRC-100)
    v
Bitcoin SV Blockchain (WhatsOnChain, GorillaPool, Bitails)
```

| Layer | Tech | Responsibility |
|-------|------|----------------|
| Frontend | React, Vite, TypeScript, MUI | UI and user interaction. Performs no signing and never holds a derived private key; the BIP39 recovery phrase is displayed in two overlays (the wallet panel and the wallet dashboard) and passes through the C++ process on its way there. |
| CEF Shell | C++20 on Windows and macOS (C++17 on other targets) | Browser engine, V8 injection, HTTP interception, permission-row caching, prompt overlays. Builds partial permission context and forwards it to Rust. |
| Wallet | Rust, Actix-web, SQLite | Crypto, signing, key material, BRC-100 protocol, domain permissions, and the permission decision engine. |

**Ports** — wallet `31301` release / `31401` under `HODOS_DEV=1`; adblock engine `31302` / `31402`. `cef-native/include/core/PortConfig.h` is the single source of truth (`WalletPort()`, `AdblockPort()`, `WalletUrl()`); never hardcode a port literal. Two exceptions exist today: an unused literal at `CWIShimScript.h:258`, and the Rust CORS list, which names both wallet ports on purpose.

**Other services the browser contacts** — MessageBox (`messagebox.babbage.systems`, `messagebox.bsvblockchain.tech`, `message-box-us-1.bsvb.tech`, `gmb.bsvblockchain.tech`), BSVB overlay hosts, transaction broadcasters and indexers (GorillaPool, Bitails, WhatsOnChain), Banana Blocks, CoinGecko and MEXC for prices, and the adblock filter-list hosts (`easylist.to`, `raw.githubusercontent.com`).

**Permission engine** — the auto-approve decision cascade lives in Rust: `rust-wallet/crates/hodos_permission_engine` (`decide()` in `src/lib.rs`, the Matrix C cap/rate cascade in `src/matrix_c.rs`), wrapped by `rust-wallet/src/permission_service/` and wired as Actix middleware in `rust-wallet/src/main.rs`. The C++ shell renders whichever modal Rust asks for and makes no cap or grant decisions. It does decide whether a caller is internal (Hodos's own UI) or external, and internal callers skip the Rust gates. The user sets default spending limits for new sites and adjusts them per site.

**Overlays** — every panel and prompt overlay is a separate CEF browser with its own V8 context; Chromium assigns the renderer processes.

---

## Quick Start (Windows)

**Prerequisites**: VS 2022 (MSVC), vcpkg, Rust, Node.js 22+, CEF binaries

The CEF shell (`cef_browser_shell.cpp` on Windows) launches the Rust wallet and the adblock engine as child processes if they are not already running. If something is already listening on a backend's port, the shell uses it without checking what it is. Only the frontend dev server has to be started by hand. Run a backend standalone when you are iterating on it — the launcher scripts set `HODOS_DEV=1`, which is what keeps dev runs out of the installed app's database.

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
- Navigation, tabs, cookies, history, bookmarks; one CEF browser per tab, with renderer processes assigned by Chromium's site isolation
- HTTP request interception for BRC-100 endpoints
- Domain permission system with per-site USD spending limits, plus prompt overlays for payment confirmation, certificate disclosure and rate limiting
- Ad & tracker blocking — a standalone Rust HTTP service (`adblock-engine`, crate `hodos-adblock`, wrapping Brave's `adblock` crate) that the shell queries over localhost. Not an in-process FFI binding
- Cookie blocking for a 24-domain tracker list; other third-party cookies are kept while you are on the site and deleted about 30 seconds after its last tab closes
- Fingerprint farbling
- Camera / microphone / geolocation permission prompts
- SSL certificate handling + secure connection indicator (padlock)
- Downloads with progress and pause/resume/cancel; find-in-page; context menus; JS dialog handling (beforeunload trap suppression); keyboard shortcuts
- QR code scanning via screen capture
- Settings persistence and Chrome/Brave/Edge profile import

### Wallet
- HD wallet (BIP39 mnemonic, BRC-42 (BSV Key Derivation Scheme) self-derivation, legacy BIP32 recovery)
- BRC-103/104 mutual authentication. Hodos signs its side; it verifies the peer only in the certificate handshake, and the server side does not yet track nonces against replay
- BRC-29 (Simple Authenticated BSV P2PKH Payment Protocol) payments
- BRC-33 (PeerServ Message Relay Interface) endpoints, backed by a local SQLite store. Messages are stored unencrypted and keyed on an identity header the wallet does not verify. PeerPay also sends and receives through MessageBox: a payment goes to the server the recipient has advertised, or to every known server if they have advertised none; incoming payments are read from every known server and each is credited once
- BEEF/SPV transaction format with merkle proofs. Merkle proofs in incoming BEEF are checked against the wallet's own verified block headers, and only verified proofs are stored
- PIN encryption of the recovery phrase (AES-256-GCM, PBKDF2) for wallets that have a PIN + OS-keystore auto-unlock
- Spends only coins proven to be money: the observed script must be a plain P2PKH payment to this wallet's key, worth more than 1 satoshi. Anything else (tokens, inscriptions, 1-sat coins, coins whose script has not been read yet) is held, never spent automatically, and listed under the balance as "N items held". A held coin worth more than 1 satoshi can be added to the balance with a confirmed "Treat as money"; a 1-satoshi coin never can, because it may be an ordinal or an NFT. Ordinals and tokens are recognised and kept safe, but not yet shown as items
- Mnemonic recovery with blockchain UTXO scanning; encrypted on-chain wallet backup (every 3 hours) and restore. An encrypted file export exists in the API but has no UI yet
- Background monitor — a task scheduler covering merkle-proof acquisition, crash recovery and UTXO sync
- BSV/USD price cache — WhatsOnChain -> Banana Blocks -> CoinGecko -> MEXC fallback chain, in-memory TTL, stale-value fallback, and a persisted SQLite cold-start cache

### What's Next

Next: showing ordinals and tokens as items, and the permission to spend them; on-chain backup and sync across devices; verified server identity for BRC-103 peers; an allowlist for the wallet surface sites can reach.

---

## Storage

| Context | Windows | macOS |
|---------|---------|-------|
| Installed app | `%APPDATA%/HodosBrowser/` | `~/Library/Application Support/HodosBrowser/` |
| Dev builds (`HODOS_DEV=1`) | `%APPDATA%/HodosBrowserDev/` | `~/Library/Application Support/HodosBrowserDev/` |

Wallet DB: `<storage>/wallet/wallet.db` (SQLite). Browser data (history, bookmarks, cookies) lives in each profile's own directory; the default profile uses `<storage>/Default/`.

A wallet binary run from a `target/` build directory refuses to start unless `HODOS_DEV=1` is set, and a binary run from anywhere else ignores a stray `HODOS_DEV=1`, so a dev run does not write to the installed app's database. The guard keys on the executable's path. Build and run steps: `build-instructions/`.

---

## Project Structure

Top-level layout only.

```
HodosBrowser/
|-- cef-native/          C++ CEF browser shell: entry point, overlays, HTTP interception
|-- rust-wallet/         Rust wallet backend: crypto, signing, BRC-100, SQLite, permission engine
|-- adblock-engine/      Standalone Rust ad & tracker blocking HTTP service
|-- frontend/            React + Vite + TypeScript UI: main window and overlay roots
|-- installer/           Windows installer script (Inno Setup)
|-- scripts/             Build, release and maintenance scripts
|-- external/            Updater frameworks downloaded at build time: WinSparkle, and Sparkle on macOS (not in git)
|-- cef-binaries/        Downloaded CEF distribution (not in git)
|-- demos/               Demo pages and sample dApps
|-- test-fixtures/       Test data and fixture dApps
|-- reference/           Clones of third-party repos for reference (not in git)
|-- build-instructions/  Platform build guides
|-- docs/                Reference docs: architecture, design rationale, security model
|-- dist/                Packaged build output (not in git)
```

---

## Documentation

| Document | Purpose |
|----------|---------|
| [`docs/`](docs/README.md) | Index of the reference docs below |
| [`docs/PROJECT_OVERVIEW.md`](docs/PROJECT_OVERVIEW.md) | Architecture reference |
| [`docs/THE_WHY.md`](docs/THE_WHY.md) | Rationale for Rust, CEF, and native wallet choices |
| [`docs/SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md`](docs/SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md) | Process isolation security model |
| [`docs/architecture/`](docs/architecture/README.md) | Wallet API map, IPC bridge, auto-approve engine |
| [`build-instructions/Hodos_Build_Windows.md`](build-instructions/Hodos_Build_Windows.md) | Windows build quick-start |

---

## Security

- **Signing keys never leave the Rust process.** No EC private key is ever returned to JavaScript, and no signing happens there. The BIP39 recovery phrase is the deliberate exception: it is shown at wallet creation so the user can record it, and afterwards through the reveal screen in the wallet UI, which asks for the PIN only when the wallet has one. The phrase is also inside the encrypted backup file that the export API writes. Web pages reach the wallet by design through the IPC bridge, and Rust gates those calls by origin; a first-party-only route list refuses state-changing requests from web origins to the reveal, backup, recover, delete, settings and domain-permission routes (a site may still revoke its own permission). The wallet's CORS allowlist blocks only a direct fetch from a web page, and pages on `localhost` count as Hodos's own UI.
- **Process isolation** — wallet runs as a separate process from the browser
- **At-rest encryption** — the mnemonic is sealed by the OS keystore: Windows DPAPI, macOS Keychain via `security-framework` (`rust-wallet/src/crypto/dpapi.rs`). On Linux and other platforms the keystore call is a stub that returns an error, so wallet creation fails
- **PIN encryption** — AES-256-GCM with PBKDF2 (600K iterations, `rust-wallet/src/crypto/pin.rs`), for wallets that have a PIN
- **Domain permissions** — per-site approval with USD spending limits. The user sets defaults for new sites and adjusts them per site
- **Defense in depth** — Rust owns the permission decision; the C++ shell caches rows and renders prompts but cannot approve on its own. The shell does classify each caller as internal or external, and a caller classified as internal skips the Rust gates
- **Memory safety** — Rust ownership model; no `unsafe` in the Rust wallet except the two Windows DPAPI FFI blocks in `crypto/dpapi.rs` (`CryptProtectData` / `CryptUnprotectData`)

---

## Known issues

Each item is being worked on; fixes are planned for a later release.

- **Local callers without a requesting-domain header are trusted as the wallet UI**, so they skip the permission gates. This includes other local processes and pages served from `localhost`.
- **The wallet surface a site can reach is a denylist, not an allowlist.** An approved site can call any wallet path that is not on the first-party-only list.
- **A Centbee import creates a wallet with no PIN**, so its recovery phrase has no PIN encryption. The wallet panel no longer offers the Centbee import; the issue applies to wallets already created that way.
- **"Always notify" lets payments under one cent through without a prompt.**
- **`verifyLegacyMessage(message, signature, identityKey)` ignores `identityKey`.**
- **Wallet secrets in memory are not zeroized.** The cached recovery phrase stays in memory until exit or wallet deletion.
- **BRC-103 peer checks are incomplete.** The AuthFetch client does not verify the server's signatures, and the wallet's own `/.well-known/auth` does not track nonces against replay.
- **Some merkle proofs cannot be checked yet.** A proof for a block below the wallet's built-in header checkpoint, or one it has not synced yet, is neither trusted nor refused, and it is not stored as verified.
- **Ordinals and tokens are held safely but not yet listed as items.**
- **The permission audit log is not written.** The table exists, but no code path writes to it.
- **macOS: an automatic update asks before installing.** It downloads in the background, then asks to install and relaunch instead of installing quietly on quit.

---

## License

MIT — see [`LICENSE`](./LICENSE). The code is open source. Three notices sit alongside the grant rather than inside it, so that `LICENSE` stays the unmodified MIT text and is detected as such:

- [`TRADEMARK.md`](./TRADEMARK.md) — "Hodos", "Hodos Browser", and the logo are trademarks of Marston Enterprises. MIT grants rights to the code, not to the name or branding. Forks must rename and rebrand. Same approach as Firefox and Brave.
- [`NOTICE.md`](./NOTICE.md) — this build includes a **1000-satoshi service fee** to a Marston Enterprises address on every outgoing transaction (on-chain backup transactions and recovery sweeps excepted), which funds development. Forks must either redirect it or disclose it.
- [`COPYRIGHT`](./COPYRIGHT) — third-party components are not relicensed by the MIT grant: Chromium and the Chromium Embedded Framework carry their own terms, as do the adblock filter lists (downloaded on first run and refreshed every 6 hours) and the embedded disconnect.me entity list.
