# Build Instructions - Hodos Browser

## ⚡ Quick Start (TL;DR)

**Prerequisites:** Visual Studio 2022, CMake 3.20+, Rust, Node.js 18+, vcpkg

**Build steps:**
1. Download CEF binaries → extract to `cef-binaries/`
2. Build CEF wrapper: `cd cef-binaries/libcef_dll/wrapper && mkdir build && cd build && cmake .. && cmake --build . --config Release`
3. Build Rust wallet: `cd rust-wallet && cargo build --release`
4. Build adblock engine: `cd adblock-engine && cargo build --release`
5. Install frontend deps: `cd frontend && npm install`
6. Build C++ shell: `cd cef-native && cmake -S . -B build -G "Visual Studio 17 2022" -A x64 && cmake --build build --config Release`

**Run the browser:**
```bash
cd frontend && npm run dev          # Terminal 1: Start frontend dev server
cd cef-native/build/bin/Release && ./HodosBrowserShell.exe   # Terminal 2: Launch browser
```

The browser **auto-launches** the Rust wallet and adblock-engine. You only need the frontend dev server running.

---

## 🎯 Overview

This document provides step-by-step instructions for building Hodos Browser. The build process involves multiple components:

| Component | Language | Port | Auto-launched? |
|-----------|----------|------|----------------|
| CEF Native Shell | C++ | — | — (main app) |
| Rust Wallet | Rust | 3301 | ✅ Yes |
| Adblock Engine | Rust | 3302 | ✅ Yes |
| Frontend | TypeScript/React | 5137 | ❌ No (run manually) |

**Note:** The C++ shell automatically starts `rust-wallet` and `adblock-engine` executables on launch. You only need to run them manually if you want to see their logs or are debugging those components.

## 📋 Prerequisites

### Required Software
- **Visual Studio 2022** (Community or Professional)
- **CMake** 3.20 or later
- **Rust** (latest stable version)
- **Node.js** 18 or later
- **Git** for version control

### Required Libraries
- **vcpkg** (C++ package manager)
  - **OpenSSL** (via vcpkg)
  - **nlohmann/json** (via vcpkg)
  - **sqlite3** (via vcpkg)

#### Installing vcpkg

**Recommended Installation Location**: `C:\Users\<YourUsername>\Dev\vcpkg` or `C:\Dev\vcpkg`

vcpkg is a C++ package manager that provides pre-built libraries. It needs to be installed separately and the toolchain path must be specified during CMake configuration.

**Installation Steps:**
```bash
# Clone vcpkg repository (recommended location: C:\Users\<YourUsername>\Dev\vcpkg)
git clone https://github.com/Microsoft/vcpkg.git C:\Users\<YourUsername>\Dev\vcpkg

# Navigate to vcpkg directory
cd C:\Users\<YourUsername>\Dev\vcpkg

# Bootstrap vcpkg (builds the vcpkg executable)
.\bootstrap-vcpkg.bat

# Add vcpkg to your PATH (optional, but recommended)
# Add C:\Users\<YourUsername>\Dev\vcpkg to your system PATH environment variable
```

#### Installing Required vcpkg Packages

**Note**: The vcpkg packages (OpenSSL, nlohmann/json, sqlite3) are automatically found and linked when using the vcpkg toolchain file during CMake configuration.

**Install all packages at once (recommended):**
```bash
# Navigate to vcpkg directory
cd C:\Users\<YourUsername>\Dev\vcpkg

# Install all required packages for x64-windows-static (static linking)
.\vcpkg install openssl:x64-windows-static sqlite3:x64-windows-static nlohmann-json:x64-windows-static
```

**Or install packages individually:**
```bash
# Install OpenSSL
.\vcpkg install openssl:x64-windows-static

# Install nlohmann/json
.\vcpkg install nlohmann-json:x64-windows-static

# Install sqlite3
.\vcpkg install sqlite3:x64-windows-static
```

**Important**: Use `x64-windows-static` triplet for static linking (all libraries bundled into the executable). This is recommended for distributing a standalone browser application.

## 🔧 Build Process

### Step 1: CEF Binaries Setup

#### Download CEF Binaries

CEF binaries are too large for Git and are gitignored. You must download them separately.

1. **Download Location**: [CEF Automated Builds](https://cef-builds.spotifycdn.com/index.html)
2. **Current Working Version**: `cef_binary_136.1.6+g1ac1b14+chromium-136.0.7103.114_windows64`
3. **Extract Location**: Extract the downloaded archive to `./cef-binaries/` directory in the project root

**Note**: We will likely try the latest stable version in the future. The current version (136.1.6) is confirmed working.

#### Build CEF Wrapper

The CEF wrapper library (`libcef_dll_wrapper`) must be built from source. The wrapper CMakeLists.txt is located in the CEF binaries directory and builds a static library that the native shell links against.

**CMakeLists.txt Location**: `cef-binaries/libcef_dll/wrapper/CMakeLists.txt`

**Important**: The wrapper must be built in-place within the `cef-binaries` directory structure. The build output contains paths specific to your system, so each developer must build their own wrapper. You cannot share the built wrapper library between systems.

```bash
# Navigate to the wrapper directory
cd cef-binaries/libcef_dll/wrapper

# Create build directory
mkdir build
cd build

# Configure CMake with vcpkg toolchain
# Replace with your actual vcpkg installation path (recommended: C:/Users/<YourUsername>/Dev/vcpkg)
cmake .. -DCMAKE_TOOLCHAIN_FILE=C:/Users/<YourUsername>/Dev/vcpkg/scripts/buildsystems/vcpkg.cmake

# Build the wrapper library
cmake --build . --config Release
```

**Output**: The wrapper library (`libcef_dll_wrapper.lib`) will be created at:
- `cef-binaries/libcef_dll/wrapper/build/Release/libcef_dll_wrapper.lib`

**Verification**: After building, verify the library exists before proceeding to the native shell build:
```bash
# Check that the library was created
dir cef-binaries\libcef_dll\wrapper\build\Release\libcef_dll_wrapper.lib
```

### Step 2: Rust Wallet Backend Setup

#### Build Rust Wallet

```bash
# Navigate to Rust wallet directory
cd rust-wallet

# Build the wallet executable (release mode)
cargo build --release

# Run the wallet server (use launcher script for dev/production isolation)
# From project root:
#   Windows: .\dev-wallet.ps1
#   Mac:     ./dev-wallet.sh
# ⚠️ Do NOT use bare 'cargo run' — dev safeguard requires HODOS_DEV=1

# Server starts on http://127.0.0.1:31301
```

**Features:**
- Custom Actix-web HTTP server
- BRC-103/104 mutual authentication
- Custom BSV ForkID SIGHASH implementation
- Transaction creation, signing, broadcasting
- Confirmed mainnet transactions
- SQLite database storage (replaces JSON files)
- UTXO caching and management
- Background UTXO sync service

#### Wallet Storage

The wallet data is stored in a SQLite database. The path is resolved automatically per platform:

| Platform | Path |
|----------|------|
| **Windows** | `%APPDATA%\HodosBrowser\wallet\wallet.db` |
| **macOS** | `~/Library/Application Support/HodosBrowser/wallet/wallet.db` |

The database is automatically created and initialized on first run.

#### Test the Wallet API

**Rust Wallet Endpoints:**
- `GET http://localhost:3301/wallet/status` - Wallet status
- `GET http://localhost:3301/wallet/balance` - Get wallet balance (from database cache)
- `POST http://localhost:3301/wallet/address/generate` - Generate new address
- `POST http://localhost:3301/getVersion` - Get wallet version
- `POST http://localhost:3301/getPublicKey` - Get public key
- `POST http://localhost:3301/createHmac` - Create HMAC for authentication
- `POST http://localhost:3301/verifyHmac` - Verify HMAC
- `POST http://localhost:3301/createSignature` - Create message signature
- `POST http://localhost:3301/verifySignature` - Verify message signature
- `POST http://localhost:3301/.well-known/auth` - BRC-104 authentication
- `POST http://localhost:3301/createAction` - Create transaction (uses database UTXOs)
- `POST http://localhost:3301/signAction` - Sign transaction (marks UTXOs as spent)
- `POST http://localhost:3301/processAction` - Process and broadcast transaction
- `POST http://localhost:3301/transaction/send` - Send transaction (with error handling)

**Test with PowerShell:**
```powershell
# Test Rust wallet (make sure wallet is running)
Invoke-RestMethod -Uri "http://localhost:3301/wallet/status" -Method GET
```

### Step 2.5: Adblock Engine Setup

The adblock engine provides ad and tracker blocking using Brave's `adblock-rust` library. It runs as a separate Rust microservice on port 3302.

**Note:** The browser auto-launches this on startup. You only need to build it — running manually is optional (for debugging).

#### Build Adblock Engine

```bash
cd adblock-engine
cargo build --release
```

**Output:** `adblock-engine/target/release/hodos-adblock.exe`

#### Version Constraints

The adblock engine has specific version pins due to Rust compatibility:

| Dependency | Pinned Version | Reason |
|------------|----------------|--------|
| `actix-web` | 4.11.0 | 4.13+ requires Rust 1.88 |
| `adblock` | 0.10.3 | 0.10.4+ requires nightly Rust |
| `rmp` | 0.8.14 | Required by adblock 0.10.x |

**Rust version:** Use stable Rust 1.85. Do not upgrade adblock crate without checking compatibility.

#### Run Manually (Optional)

```bash
# From project root (sets HODOS_DEV=1 for dev isolation):
#   Windows: .\dev-adblock.ps1
#   Mac:     ./dev-adblock.sh
# Server starts on http://127.0.0.1:31302
```

#### Adblock Engine Endpoints

- `POST http://localhost:3302/check` — Check if URL should be blocked
- `POST http://localhost:3302/cosmetic` — Get cosmetic filter rules for a page
- `GET http://localhost:3302/stats` — Blocking statistics

### Step 3: Vite React Frontend Setup

The frontend is built with **Vite** and **TypeScript**.

#### Install Node.js Dependencies

```bash
cd frontend
npm install
```

#### Start Development Server

```bash
npm run dev
# Frontend will be available at http://127.0.0.1:5137
```

#### Build for Production

```bash
npm run build
# Output will be in frontend/dist/
```

### Step 4: C++ Native Shell Build

#### Configure CMake

**Note**: The CMakeLists.txt now uses portable configuration - no hardcoded paths! You have two options:

**Option 1: Using Environment Variable (Recommended)**

```powershell
# Set VCPKG_ROOT environment variable (one-time setup)
# Replace with your actual vcpkg installation path
$env:VCPKG_ROOT = "C:/Users/<YourUsername>/Dev/vcpkg"

# Configure CMake (vcpkg will be found automatically)
cd cef-native
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
```

**To make VCPKG_ROOT permanent (recommended):**
```powershell
# Add to your PowerShell profile or set in Windows Environment Variables
[System.Environment]::SetEnvironmentVariable('VCPKG_ROOT', 'C:/Users/<YourUsername>/Dev/vcpkg', 'User')
```

**Option 2: Command-Line Parameter**

```powershell
cd cef-native

# Configure CMake with explicit toolchain path
# Replace with your actual vcpkg installation location
cmake -S . -B build -G "Visual Studio 17 2022" -A x64 ^
  -DCMAKE_TOOLCHAIN_FILE=C:/Users/<YourUsername>/Dev/vcpkg/scripts/buildsystems/vcpkg.cmake
```

**Benefits of New Configuration:**
- ✅ Works on any developer's machine (no hardcoded paths)
- ✅ Automatic package discovery (OpenSSL, nlohmann-json, sqlite3)
- ✅ Cleaner CMake output with helpful messages
- ✅ Foundation for cross-platform support (Windows + macOS)

#### Build Native Shell

```bash
cd build

# Build Release configuration
cmake --build . --config Release

# Build Debug configuration (for development)
cmake --build . --config Debug
```

**Output**: The executable will be at:
- `cef-native/build/bin/Release/HodosBrowserShell.exe` (or Debug for debug builds)

The build process automatically copies required CEF runtime files (DLLs, resources) to the output directory.

### Step 5: Run the Browser

The browser auto-launches the Rust wallet (port 3301) and adblock engine (port 3302). You only need to start the frontend dev server.

#### Start Frontend Dev Server

```bash
cd frontend
npm run dev
# Serves on http://127.0.0.1:5137
```

#### Launch Browser

```bash
cd cef-native/build/bin/Release
./HodosBrowserShell.exe
```

The browser will automatically start `rust-wallet` and `adblock-engine` from their `target/release/` directories.

#### Optional: Run Backend Services Manually (for debugging)

If you need to see wallet or adblock logs, run them in separate terminals before launching the browser:

```powershell
# Terminal 1: Wallet (from project root — sets HODOS_DEV=1)
.\dev-wallet.ps1           # Windows
./dev-wallet.sh            # Mac

# Terminal 2: Adblock (from project root — sets HODOS_DEV=1)
.\dev-adblock.ps1          # Windows
./dev-adblock.sh           # Mac

# Terminal 3: Frontend (required)
cd frontend && npm run dev

# Terminal 4: Browser (builds + launches with HODOS_DEV=1)
cd cef-native
.\win_build_run.ps1        # Windows
./mac_build_run.sh         # Mac
```

When running services manually, the browser detects they're already running and uses them instead of spawning new processes.

## 🚨 Known Issues & TODOs

### CEF Integration Issues
- [x] **CEF Version**: Current working version: `cef_binary_136.1.6+g1ac1b14+chromium-136.0.7103.114_windows64`
- [x] **Hardcoded Paths**: ✅ FIXED! CMakeLists.txt now uses environment variables and auto-detection
- [x] **vcpkg Toolchain**: ✅ FIXED! Automatically detected via VCPKG_ROOT environment variable
- [x] **OpenSSL Paths**: ✅ FIXED! Uses vcpkg's automatic package discovery
- [x] **Wrapper Build**: Wrapper builds correctly but must be built by each developer

### Build System Issues
- [x] **vcpkg Path Configuration**: ✅ FIXED! Uses VCPKG_ROOT environment variable or command-line parameter
- [ ] **Cross-Platform**: Test build process on different platforms (macOS support in progress)
- [ ] **CI/CD**: Set up automated build pipeline
- [ ] **Dependencies**: Automate dependency installation
- [ ] **Version Management**: Implement proper versioning for all components

## 🔧 Development Environment Setup

### Visual Studio Configuration

For IntelliSense support, configure `.vscode/c_cpp_properties.json`:

```json
{
    "configurations": [
        {
            "name": "Win32",
            "includePath": [
                "${workspaceFolder}/cef-binaries/include",
                "${workspaceFolder}/cef-native/include",
                "${vcpkgRoot}/installed/x64-windows/include"
            ],
            "defines": [
                "_DEBUG",
                "UNICODE",
                "_UNICODE"
            ],
            "windowsSdkVersion": "10.0.22000.0",
            "compilerPath": "C:/Program Files/Microsoft Visual Studio/2022/Community/VC/Tools/MSVC/14.37.32822/bin/Hostx64/x64/cl.exe",
            "cStandard": "c17",
            "cppStandard": "c++17",
            "intelliSenseMode": "windows-msvc-x64"
        }
    ]
}
```

### Rust Environment

The Rust wallet uses standard Cargo for dependency management. Dependencies are specified in `rust-wallet/Cargo.toml`.

### TypeScript/Node.js Environment

The frontend uses Vite with TypeScript. Dependencies are managed via `frontend/package.json`.

## 📁 Repository Structure & Gitignore

The following directories/files are gitignored and must be set up locally:

- **`/cef-binaries/`** - CEF binaries (too large for Git, must be downloaded separately)
- **`/cef-native/build/`** - CMake build output
- **`/cef-native/bin/`** - Compiled executables
- **`rust-wallet/target/`** - Rust build artifacts
- **`adblock-engine/target/`** - Adblock engine build artifacts
- **`frontend/node_modules/`** - Node.js dependencies
- **`frontend/dist/`** - Frontend build output

**Important**: Before building, ensure you have:
1. Downloaded and extracted CEF binaries to `cef-binaries/`
2. Built the CEF wrapper library
3. Installed vcpkg and configured the toolchain path

## 🚀 Future Build Considerations

### Multi-Platform Support
- ✅ **Windows**: Full feature parity (current primary platform)
- 🟡 **macOS**: Foundation complete (window, overlays, tabs). Feature parity sprint in progress. See `development-docs/Final-MVP-Sprint/macos-port/MACOS-PORT-HANDOVER.md`
- 🟡 **Linux**: CEF with GTK integration (future)
- 🟡 **Mobile**: React Native with native modules (future)

### Build Optimizations
- 🟡 **Incremental Builds**: Optimize CMake for faster rebuilds
- 🟡 **Parallel Compilation**: Use multiple cores for faster builds
- 🟡 **Dependency Management**: Automate vcpkg package installation
- 🟡 **Cross-Compilation**: Support building for different architectures

### CI/CD Pipeline
- 🟡 **GitHub Actions**: Automated builds on multiple platforms
- 🟡 **Docker**: Containerized build environment
- 🟡 **Artifact Management**: Automated release packaging
- 🟡 **Testing**: Automated integration testing

## 📝 Build Troubleshooting

### Common Issues

#### CEF Binary Issues
```bash
# Error: CEF binaries not found
# Solution: Download CEF binaries from https://cef-builds.spotifycdn.com/index.html
# Extract to ./cef-binaries/ directory
# Verify CEF version matches (currently: 136.1.6)
```

#### CEF Wrapper Build Issues
```bash
# Error: Wrapper library not found
# Solution: Build the wrapper library first (Step 1)
# Ensure you're building in the correct directory: cef-binaries/libcef_dll/wrapper/build/
# Check that the wrapper library exists at: cef-binaries/libcef_dll/wrapper/build/Release/libcef_dll_wrapper.lib
```

#### vcpkg Issues
```bash
# Error: vcpkg toolchain not found
# Solution: Install vcpkg and specify correct toolchain path
# Recommended location: C:\Users\<YourUsername>\Dev\vcpkg
# Example: -DCMAKE_TOOLCHAIN_FILE=C:/Users/<YourUsername>/Dev/vcpkg/scripts/buildsystems/vcpkg.cmake
# Ensure vcpkg packages are installed: vcpkg install openssl:x64-windows-static sqlite3:x64-windows-static nlohmann-json:x64-windows-static

# Error: Could not find package "unofficial-sqlite3" (or other packages)
# Solution:
# 1. Ensure all packages are installed: vcpkg install openssl:x64-windows-static sqlite3:x64-windows-static nlohmann-json:x64-windows-static
# 2. Delete the CMake cache and reconfigure: rm -rf build && cmake -S . -B build ...
# 3. The CMake configure step must be run AFTER installing packages, as it discovers dependencies
# 4. Verify packages are installed: vcpkg list (should show openssl, sqlite3, nlohmann-json)
```

#### CMake Configuration Issues
```bash
# Error: CMake configuration failed
# Solution:
# 1. Check all required libraries are installed via vcpkg
# 2. Verify vcpkg toolchain path is correct
# 3. Ensure CEF binaries are extracted to cef-binaries/
# 4. Verify wrapper library has been built
# 5. Check that paths in CMake command match your system
```

#### Rust Build Issues
```bash
# Error: Cargo build fails
# Solution:
# 1. Ensure Rust is installed: rustc --version
# 2. Update Rust: rustup update
# 3. Clean and rebuild: cargo clean && cargo build --release
```

#### Frontend Build Issues
```bash
# Error: npm install fails
# Solution:
# 1. Ensure Node.js 18+ is installed: node --version
# 2. Clear cache: npm cache clean --force
# 3. Delete node_modules and reinstall: rm -rf node_modules && npm install
```

---

*This build guide will be updated as the project evolves and build issues are resolved.*
