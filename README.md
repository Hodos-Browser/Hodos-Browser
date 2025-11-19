# HodosBrowser

A custom Web3 browser built on the Chromium Embedded Framework (CEF) with native BitcoinSV wallet for secure authentication, micropayments, and Electronic Data Interchange (EDI- smart contracts).

## ⚡ Current Status - Ready for Real-World Testing (Oct 27, 2025)

**🎉 MAJOR MILESTONE:** BRC-100 Groups A & B complete! Authentication + Full transaction management ready for production testing!

### Two Implementations (Both Port 3301 - Only One Runs At A Time):

1. **Go Wallet** - ✅ Production Ready
   - BSV Go SDK (`v1.2.9`)
   - HD wallet (BIP44)
   - CEF browser integration
   - Location: `go-wallet/`

2. **Rust Wallet** - ✅ **Production Ready**
   - Custom BSV ForkID SIGHASH
   - **Complete BRC-100 Groups A & B**
   - **Transaction history tracking**
   - **BEEF Phase 2 parsing**
   - **Confirmed mainnet transactions!**
   - Location: `rust-wallet/`

**Latest Additions (Oct 27-30):**
- ✅ Action storage system (transaction history)
- ✅ `abortAction`, `listActions`, `internalizeAction`
- ✅ BEEF parser with output ownership detection
- ✅ Confirmation tracking via WhatsOnChain
- ✅ Labels, addresses, and metadata support
- ✅ **BRC-29 payment protocol support**
- ✅ **TSC Merkle proof generation with block height resolution**
- ✅ **Atomic BEEF (BRC-95) format implementation**
- ✅ **Real-world testing: ToolBSV payments working!**

**Why Two Implementations?**
- Testing different languages (Go vs Rust)
- Comparing BSV SDK vs custom implementation
- Will choose one for production

**Shared Storage:** Both use `%APPDATA%/HodosBrowser/wallet/wallet.json`

**See:** `SESSION_SUMMARY_2025-10-27.md` for latest session details

---

## 🔧 Project Structure

> Note: `cef-binaries/` and `**/target/` are excluded from Git using `.gitignore`.

## 🚀 Goals

- ✅ CEF shell with secure wallet backend
- ✅ Process-per-overlay architecture (settings, wallet, backup modals)
- ✅ Complete identity system with Go daemon integration
- ✅ **BRC-100 Groups A & B Complete** - Auth + Transaction management
- ✅ **Transaction History System** - Full action tracking with labels
- ✅ **BEEF Phase 2 Parser** - Transaction parsing with output ownership
- ✅ **BEEF/SPV Integration** - Real blockchain transactions
- ✅ **Production-Ready Rust Wallet** - 45% of BRC-100 complete (14/31 methods)
- ✅ Enforce native, secure signing (not in JavaScript)
- 🧱 Build the UI from scratch using React + Vite
- 🎯 **Next: Real-world testing** with ToolBSV and Thryll.online
- ⚙️ Smart contract integration with sCrypt (or custom) and BRC-100/Authrite
- 🎯 Support micropayments, token gating, and identity-bound access

## 📦 Tech Stack

| Layer | Technology | Notes |
|-------|------------|-------|
| Browser Shell | C++ / Chromium Embedded Framework | ✅ Process-per-overlay architecture implemented |
| UI | React + Vite (TypeScript) | ✅ Multiple overlay routes (/settings, /wallet, /backup) |
| **Wallet Backend** | **Go + Rust** | ✅ **Two implementations (testing both)** |
| **Go Wallet** | **Go** (bitcoin-sv/go-sdk) | ✅ **Port 3301 - BSV SDK** |
| **Rust Wallet** | **Rust** (Actix-web) | ✅ **Port 3301 - Custom crypto** |
| Overlay System | **Process-Per-Overlay** | ✅ Each overlay runs in isolated CEF subprocess |
| Identity Management | **Complete System** | ✅ File-based identity with backup modal workflow |
| **BRC-100 Authentication** | **Complete Implementation** | ✅ **Rust: Full BRC-103/104 handshake working** |
| **Transaction System** | **Complete Implementation** | ✅ **Rust: BSV ForkID SIGHASH signing working** |
| **Broadcasting** | **Multi-Miner** | ✅ **WhatsOnChain + GorillaPool** |
| Key Derivation | **HD Wallet (BIP44)** | ✅ **Production-ready HD wallet** |
| Identity / Auth | BRC-100 (Authrite Protocol (Babbage)) | ✅ **Complete BRC-100 protocol implementation** |
| Smart Contracts | sCrypt (BSV) | |
| Blockchain Integration | Bitcoin SV (WhatsOnChain, GorillaPool) | ✅ **Real blockchain integration** |

## 🛠️ Setup

**⚠️ NOTE:** Both wallets listen on port 3301. Only run ONE at a time.

### Option 1: Rust Wallet (Custom Implementation)

```bash
cd rust-wallet
cargo build
cargo run
# Server starts on http://127.0.0.1:3301
```

**Features:**
- Custom BSV ForkID SIGHASH implementation
- BRC-103/104 authentication
- Transaction signing working
- Confirmed mainnet transactions

### Option 2: Go Wallet (BSV SDK)

```bash
cd go-wallet
go build -o bitcoin-wallet.exe
./bitcoin-wallet.exe

# Or use the batch file
./start-wallet.bat
# Server starts on http://127.0.0.1:3301
```

**Features:**
- Official BSV Go SDK
- Full BRC-100 support
- CEF browser integration
- Production-ready

### Frontend Development

```bash
cd frontend
npm install
npm run dev
# Frontend will be available at http://127.0.0.1:5137
```

### CEF Native Shell

```bash
cd cef-native/build
cmake --build . --config Release
./bin/Release/HodosBrowserShell.exe
```

See `BUILD_INSTRUCTIONS.md` for detailed build steps.

## 📁 Repository Notes

- CEF binaries are local-only and not tracked by Git.
- The cef-native and cef-binaries/libcef_dll/wrapper layers are independently compiled but logically connected:
    - The wrapper is built as a standalone static library (libcef_dll_wrapper.lib)
    - Your native shell links to that static lib manually

    BABBAGE-BROWSER (HodosBrowser)/
    ├── .vscode/                     → VSCode workspace configs
    │
    ├── cef-binaries/               → CEF binaries and libcef_dll wrapper source (not tracked by Git)
    │   └── libcef_dll/
    │       └── wrapper/            → Custom-built wrapper compiled to static lib (needs the CMakeList.txt)
    │
    ├── cef-native/                 → Native C++ shell for browser logic
    │   ├── build/                  → Local CMake/MSVC build artifacts
    │   ├── include/
    │   │   ├── core/               → Wallet, identity, and navigation headers
    │   │   └── handlers/           → CEF event hook headers (client, render, etc.)
    │   ├── src/
    │   │   ├── core/               → Backend implementations for wallet and identity
    │   │   └── handlers/           → CEF app/client/render lifecycle implementations
    │   └── tests/                  → Native shell test harness and main entrypoint
    │
    ├── go-wallet/                  → Go wallet backend (Port 8080) ✅ PRODUCTION READY
    │   ├── main.go                 → HTTP server and endpoint handlers
    │   ├── hd_wallet.go            → HD wallet with BIP44 derivation
    │   ├── transaction_builder.go  → Transaction creation using BSV Go SDK
    │   ├── transaction_broadcaster.go → Multi-miner broadcasting
    │   ├── utxo_manager.go         → UTXO fetching and management
    │   ├── brc100_api.go           → BRC-100 authentication endpoints
    │   └── go.mod                  → Go dependencies (BSV SDK v1.2.9)
    │
    ├── rust-wallet/                → Rust wallet backend (Port 3301) ✅ WORKING
    │   ├── src/
    │   │   ├── main.rs             → Actix-web HTTP server
    │   │   ├── handlers.rs         → BRC-100 endpoint handlers (1900+ lines)
    │   │   ├── json_storage.rs     → Wallet.json management
    │   │   ├── crypto/             → BRC-42/43 crypto implementations
    │   │   ├── transaction/        → Transaction types and SIGHASH
    │   │   │   ├── mod.rs          → Module exports
    │   │   │   ├── types.rs        → Transaction structures
    │   │   │   └── sighash.rs      → BSV ForkID SIGHASH implementation
    │   │   └── utxo_fetcher.rs     → WhatsOnChain UTXO fetching
    │   ├── Cargo.toml              → Rust dependencies
    │   └── target/                 → Build artifacts (gitignored)
    │
    ├── frontend/                   → React + Vite UI
    │   ├── public/                 → Static assets served by Vite
    │   ├── src/
    │   │   ├── components/panels/  → Wallet UI, tabs, settings panels
    │   │   ├── hooks/              → Shared logic (e.g. `useHodosBrowser`)
    │   │   ├── pages/              → Page-level views like Browser and Welcome screens
    │   │   └── types/              → TypeScript types (identity, API contracts)
    │   ├── index.html              → App entrypoint (served by Vite)
    │   └── main.tsx                → React bootstrap
    │
    ├── .gitignore
    ├── README.md
    ├── BUILD_INSTRUCTIONS.md       → Build instructions for all components
    ├── DEVELOPER_NOTES.md          → Session notes and implementation details
    ├── ARCHITECTURE.md             → System architecture documentation
    ├── API_REFERENCES.md           → API endpoint documentation
    ├── RUST_TRANSACTION_IMPLEMENTATION_PLAN.md → Rust transaction implementation details
    ├── RUST_WALLET_SESSION_SUMMARY.md → Latest session summary (Oct 16, 2025)
    └── vite.config.ts             → Vite config (frontend build + dev server)

## 💡 Project Philosophy

- **Security-first**: Private keys and signing logic never exposed to JS
- **Native control**: Full backend control over cookie, adds, contract, InterPlanetary File System, and payment enforcement
- **Web3 reimagined**: Built for real micropayments, not fake dApps
- **Prioritize user experience**: Clean easy to use and understand

## 🔒 Security Architecture

### Why Native Wallet Backend?

**JavaScript Security Vulnerabilities:**
- **Process Isolation**: JavaScript runs in the browser's render process, which is inherently less secure than native processes
- **XSS Attack Surface**: Malicious websites could potentially access wallet functions through cross-site scripting attacks
- **Extension Interference**: Browser extensions or injected scripts could intercept wallet operations
- **Memory Exposure**: Private keys stored in JavaScript variables are accessible through console inspection, memory dumps, and developer tools

**Native Backend Benefits:**
- **Process Separation**: Wallet operations happen in isolated Go daemon processes, completely separate from web content
- **Memory Protection**: Go daemon provides stronger memory protection than JavaScript
- **Cryptographic Libraries**: Direct access to Bitcoin SV Go SDK (bitcoin-sv/go-sdk) with BEEF and SPV support
- **Attack Surface Reduction**: Even if a website compromises the render process, it cannot access the wallet backend
| 🟡 *PoC: Will migrate to Rust for production* |

**Architecture Security:**
- **Controlled Bridge API**: Only safe, high-level functions are exposed through `window.hodosBrowser`
- **Multi-Process CEF**: Leverages Chromium's natural security boundaries between processes
- **Go Daemon Isolation**: Private keys never leave the isolated Go wallet process
- **Real Financial Security**: Built for production use where real money is at stake, not just development/testing

## 🧬 BRC-100 Protocol Compatibility

This project is being built to support apps that follow the **BRC-100 authentication and identity standards**, enabling secure, privacy-preserving interaction between wallets and applications. The goal is to ensure seamless compatibility with:

- **Toolio-generated identities and WAB certificates**
- **MetanetDesktop-style storage and identity detection**: Identity and wallet information will be stored in AppData%/MetanetDesktop/identity.json
- **BRC-52/103 identity certificates** with selective disclosure
- **Type-42 key derivation** for encrypted P2P channels
- **BEEF-formatted atomic transactions** for identity-bound actions
- **SPV-based identity and transaction verification**
- **Browser-side API injection for identity access**, e.g.:
  ```js
  window.hodosBrowser.identity.get()
  window.hodosBrowser.brc100.getPublicKey()
  window.hodosBrowser.brc100.signMessage(...)
  window.hodosBrowser.brc100.getCertificate()

---

This is an early-stage rewrite.
