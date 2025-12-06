# HodosBrowser

A custom Web3 browser built on the Chromium Embedded Framework (CEF) with native BitcoinSV wallet for secure authentication, micropayments, and Electronic Data Interchange (EDI- smart contracts).

## ⚡ Current Status - Production Ready (Dec 2025)

**🎉 MAJOR MILESTONE:** BRC-100 Groups A & B complete! Authentication + Full transaction management with real-world testing successful!

### Rust Wallet - ✅ Production Ready

**Complete Features:**
- ✅ **BRC-100 Groups A & B** - Authentication + Transaction management
- ✅ **Custom BSV ForkID SIGHASH** - Production-ready signing
- ✅ **BRC-103/104 Mutual Authentication** - 7 critical breakthroughs
- ✅ **BRC-29 Payment Protocol** - Privacy-preserving micropayments
- ✅ **Transaction History** - Full action storage with labels and metadata
- ✅ **BEEF Phase 2 Parser** - Output ownership detection
- ✅ **BRC-33 Message Relay** - Peer-to-peer messaging support
- ✅ **Confirmed Mainnet Transactions** - Real-world validation

**Latest Achievements:**
- ✅ BRC-29 payments working with ToolBSV and other sites
- ✅ TSC Merkle proof generation with block height resolution
- ✅ Atomic BEEF (BRC-95) format implementation
- ✅ Complete transaction lifecycle (create → sign → broadcast → confirm)

**Storage:** `%APPDATA%/HodosBrowser/wallet/wallet.db` (SQLite database)

---

## 🔧 Project Structure

> Note: `cef-binaries/` and `**/target/` are excluded from Git using `.gitignore`.

## 🚀 Goals

- ✅ CEF shell with secure wallet backend
- ✅ Process-per-overlay architecture (settings, wallet, backup modals)
- ✅ Complete identity system with Rust wallet backend
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

| Layer | Technology | Status |
|-------|------------|--------|
| Browser Shell | C++ / Chromium Embedded Framework | ✅ Process-per-overlay architecture |
| UI | React + Vite (TypeScript) | ✅ Multiple overlay routes |
| **Wallet Backend** | **Rust** (Actix-web) | ✅ **Production ready** |
| Overlay System | **Process-Per-Overlay** | ✅ Isolated CEF subprocesses |
| **BRC-100 Authentication** | **Rust Implementation** | ✅ **Groups A & B complete** |
| **Transaction System** | **BSV ForkID SIGHASH** | ✅ **Mainnet confirmed** |
| **Broadcasting** | **Multi-Miner** | ✅ WhatsOnChain + GorillaPool |
| Key Derivation | **HD Wallet (BIP44)** | ✅ Production-ready |
| Identity / Auth | BRC-100 (Authrite Protocol) | ✅ Complete implementation |
| Blockchain Integration | Bitcoin SV APIs | ✅ Real mainnet integration |

## 🛠️ Setup

### Rust Wallet Backend

```bash
cd rust-wallet
cargo build
cargo run
# Server starts on http://127.0.0.1:3301
```



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

## 💾 Backup & Recovery

**Current Implementation:**
- Local file-based backups (SQLite database copy)
- JSON export for non-sensitive data
- Recovery from mnemonic (re-derive addresses, re-discover UTXOs from blockchain)

**Future: Online Wallet Backend** (Coordination Required):
- Cloud-based backup storage for wallet databases
- User authentication and access control (method TBD)
- Encrypted backups with user-controlled keys
- Storage location TBD (coordinated with protocol developers)
- **Note**: This requires coordination with open source BRC-100 protocol developers to ensure:
  - Standardized backup format for interoperability
  - Consistent authentication methods
  - Unified security practices
  - User privacy and control

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
    ├── rust-wallet/                → Rust wallet backend (Port 3301) ✅ PRODUCTION READY
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
- **Process Separation**: Wallet operations happen in isolated Rust daemon processes, completely separate from web content
- **Memory Safety**: Rust provides compile-time memory safety guarantees without runtime overhead
- **Cryptographic Operations**: Custom Rust implementation with BSV ForkID SIGHASH and BRC-100 protocol support
- **Attack Surface Reduction**: Even if a website compromises the render process, it cannot access the wallet backend

**Architecture Security:**
- **Controlled Bridge API**: Only safe, high-level functions are exposed through `window.hodosBrowser`
- **Multi-Process CEF**: Leverages Chromium's natural security boundaries between processes
- **Rust Process Isolation**: Private keys never leave the isolated Rust wallet process
- **Real Financial Security**: Built for production use where real money is at stake, with compile-time safety guarantees

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
