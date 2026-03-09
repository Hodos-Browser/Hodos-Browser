# Windows Build Instructions - HodosBrowser

## 🎯 Overview

Step-by-step instructions for building HodosBrowser on Windows. This guide covers all three components: CEF native shell (C++), Rust wallet backend, and React frontend (TypeScript).

**Estimated Setup Time**: 1-2 hours (first time)

---

## 📋 Prerequisites

### Required Software

| Software | Version | Download |
|----------|---------|----------|
| **Visual Studio 2022** | Community or Professional | [Download](https://visualstudio.microsoft.com/downloads/) |
| **CMake** | 3.20+ | [Download](https://cmake.org/download/) |
| **Rust** | Latest stable | [Download](https://rustup.rs/) |
| **Node.js** | 18+ | [Download](https://nodejs.org/) |
| **Git** | Latest | [Download](https://git-scm.com/downloads) |
| **vcpkg** | Latest | See below |

### Verify Installations

```bash
# Check versions
cmake --version        # Should be 3.20+
rustc --version        # Should be latest stable
node --version         # Should be 18+
git --version          # Any recent version
```

---

## 🔧 Step 1: Install vcpkg

vcpkg is Microsoft's C++ package manager. We use it for OpenSSL, nlohmann-json, and sqlite3.

### Install vcpkg

```bash
# Clone to C:\Dev\vcpkg (recommended location)
git clone https://github.com/Microsoft/vcpkg.git C:\Dev\vcpkg

# Navigate to vcpkg directory
cd C:\Dev\vcpkg

# Bootstrap vcpkg
.\bootstrap-vcpkg.bat
```

### Install Required Packages

**IMPORTANT**: Must use `x64-windows-static` triplet for static linking.

```bash
cd C:\Dev\vcpkg

# Install all required packages
.\vcpkg install openssl:x64-windows-static sqlite3:x64-windows-static nlohmann-json:x64-windows-static
```

### Verify Installation

```bash
# Should show all three packages
.\vcpkg list | findstr "openssl nlohmann sqlite"
```

**Expected output:**
```
nlohmann-json:x64-windows-static    3.12.0#1    JSON for Modern C++
openssl:x64-windows-static          3.6.0#3     OpenSSL is an open source...
sqlite3:x64-windows-static          3.51.1      SQLite is a software library...
```

---

## 🌐 Step 2: Download CEF Binaries

CEF (Chromium Embedded Framework) binaries are too large for Git and must be downloaded separately.

### Download

1. Visit [CEF Automated Builds](https://cef-builds.spotifycdn.com/index.html)
2. Download **Windows 64-bit** - Standard Distribution
3. **Recommended Version**: `cef_binary_136.1.6+g1ac1b14+chromium-136.0.7103.114_windows64` (tested)
4. **Or**: Latest stable build (may require testing)

### Extract

```bash
# Extract to project root
# Should create: ./cef-binaries/ directory
# Example structure:
# cef-binaries/
# ├── Release/
# ├── Resources/
# ├── include/
# └── libcef_dll/
```

### Build CEF Wrapper

The CEF wrapper library must be built from source.

```bash
# Navigate to wrapper directory
cd cef-binaries/libcef_dll/wrapper

# Create build directory
mkdir build
cd build

# Configure (replace with YOUR vcpkg path if different)
cmake .. -DCMAKE_TOOLCHAIN_FILE=C:/Dev/vcpkg/scripts/buildsystems/vcpkg.cmake

# Build
cmake --build . --config Release
```

**Verify Wrapper Built:**
```bash
# Check that library exists
dir Release\libcef_dll_wrapper.lib
```

---

## 🦀 Step 3: Build Rust Wallet

The Rust wallet backend runs on port 3301 and provides all wallet/crypto operations.

```bash
# Navigate to rust-wallet directory
cd rust-wallet

# Build release version
cargo build --release

# Test build (optional)
cargo run --release
```

**Expected output:**
```
🦀 Bitcoin Browser Wallet (Rust)
=================================

📁 Wallet directory: C:\Users\<YourUsername>\AppData\Roaming\HodosBrowser\wallet
✅ Database initialized
✅ Domain whitelist manager initialized
✅ BRC-33 message relay initialized
✅ Auth session manager initialized
...
Listening on: http://127.0.0.1:3301
```

Press `Ctrl+C` to stop. The browser auto-launches this on startup.

---

## 🛡️ Step 3.5: Build Adblock Engine

The adblock engine provides ad and tracker blocking. It's auto-launched by the browser.

```bash
cd adblock-engine
cargo build --release
```

**Output:** `adblock-engine/target/release/hodos-adblock.exe`

**Version constraints:** Uses pinned versions (actix-web 4.11.0, adblock 0.10.3) for Rust 1.85 compatibility. See main BUILD_INSTRUCTIONS.md for details.

---

## ⚛️ Step 4: Build React Frontend

The React frontend serves the UI on port 5137.

```bash
# Navigate to frontend directory
cd frontend

# Install dependencies
npm install

# Test dev server (optional)
npm run dev
```

**Expected output:**
```
  VITE v... ready in ...ms

  ➜  Local:   http://127.0.0.1:5137/
```

Press `Ctrl+C` to stop. You'll run this in a separate terminal later.

---

## 🏗️ Step 5: Build CEF Native Shell

The C++ browser shell that ties everything together.

### Configure CMake

**Method 1: Using vcpkg toolchain path directly**

```bash
cd cef-native

# Configure (replace path if your vcpkg is elsewhere)
cmake -S . -B build -G "Visual Studio 17 2022" -A x64 \
  -DCMAKE_TOOLCHAIN_FILE=C:/Dev/vcpkg/scripts/buildsystems/vcpkg.cmake
```

**Method 2: Using VCPKG_ROOT environment variable**

```powershell
# Set environment variable (one-time)
$env:VCPKG_ROOT = "C:/Dev/vcpkg"

# Or make permanent:
[System.Environment]::SetEnvironmentVariable('VCPKG_ROOT', 'C:/Dev/vcpkg', 'User')

# Configure (auto-detects vcpkg)
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
```

**Expected CMake Output:**
```
-- Using vcpkg toolchain: C:/Dev/vcpkg/scripts/buildsystems/vcpkg.cmake
-- vcpkg triplet: x64-windows-static
-- Found OpenSSL: ...
-- Found nlohmann_json: ...
-- Found unofficial-sqlite3: ...
-- Configuring done
-- Generating done
```

### Build

```bash
cd build

# Build Release version
cmake --build . --config Release
```

**Output location:**
```
cef-native/build/bin/Release/HodosBrowserShell.exe
```

---

## ✅ Step 6: Run the Browser

The browser **auto-launches** the Rust wallet (port 3301) and adblock engine (port 3302). You only need to run the frontend dev server manually.

### Minimal Setup (2 terminals)

**Terminal 1: Frontend Dev Server**
```bash
cd frontend
npm run dev
```
**Leave this running** (port 5137)

**Terminal 2: CEF Browser**
```bash
cd cef-native/build/bin/Release
./HodosBrowserShell.exe
```

### Optional: Run Backend Services Manually (for debugging)

If you need to see wallet or adblock logs, run them before launching the browser:

```bash
# Terminal 1 (optional): Rust Wallet - for logs
cd rust-wallet && cargo run --release

# Terminal 2 (optional): Adblock Engine - for logs  
cd adblock-engine && cargo run --release

# Terminal 3: Frontend Dev Server (required)
cd frontend && npm run dev

# Terminal 4: CEF Browser
cd cef-native/build/bin/Release && ./HodosBrowserShell.exe
```

When services are already running, the browser detects and uses them instead of spawning new processes.

**Expected behavior:**
- Browser window opens
- Header loads React UI from http://127.0.0.1:5137
- Webview displays content
- Wallet operations work (backed by Rust on port 3301)
- Ad blocking works (backed by adblock engine on port 3302)

---

## 🚨 Troubleshooting

### CMake: "Could NOT find OpenSSL"

**Problem**: vcpkg triplet not set correctly.

**Solution**:
```bash
# Verify packages are installed
cd C:\Dev\vcpkg
.\vcpkg list | findstr openssl

# Should show:
openssl:x64-windows-static

# If not, install:
.\vcpkg install openssl:x64-windows-static

# Clean and reconfigure
cd cef-native
rm -rf build
cmake -S . -B build -G "Visual Studio 17 2022" -A x64 \
  -DCMAKE_TOOLCHAIN_FILE=C:/Dev/vcpkg/scripts/buildsystems/vcpkg.cmake
```

### CMake: "vcpkg not found"

**Problem**: Toolchain path incorrect.

**Solution**: Use absolute path to your vcpkg installation:
```bash
cmake ... -DCMAKE_TOOLCHAIN_FILE=C:/Dev/vcpkg/scripts/buildsystems/vcpkg.cmake
```

### CEF Wrapper Not Found

**Problem**: Wrapper library wasn't built.

**Solution**:
```bash
cd cef-binaries/libcef_dll/wrapper
mkdir build
cd build
cmake .. -DCMAKE_TOOLCHAIN_FILE=C:/Dev/vcpkg/scripts/buildsystems/vcpkg.cmake
cmake --build . --config Release

# Verify:
dir Release\libcef_dll_wrapper.lib
```

### Rust Build Fails

**Solution**:
```bash
rustup update
cargo clean
cargo build --release
```

### Frontend npm install Fails

**Solution**:
```bash
npm cache clean --force
rm -rf node_modules
npm install
```

---

## 📁 Project Structure

```
Hodos-Browser/
├── cef-binaries/              # CEF binaries (download separately)
│   ├── Release/               # CEF runtime DLLs
│   ├── Resources/             # CEF resources
│   └── libcef_dll/wrapper/    # Wrapper library (build separately)
├── cef-native/                # C++ browser shell
│   ├── src/                   # Source files
│   ├── include/               # Header files
│   ├── CMakeLists.txt         # Build configuration
│   └── build/                 # Build output (gitignored)
├── rust-wallet/               # Rust wallet backend
│   ├── src/                   # Rust source
│   ├── Cargo.toml             # Rust dependencies
│   └── target/                # Build artifacts (gitignored)
├── frontend/                  # React frontend
│   ├── src/                   # TypeScript source
│   ├── package.json           # Node dependencies
│   ├── node_modules/          # Dependencies (gitignored)
│   └── dist/                  # Build output (gitignored)
└── WINDOWS_BUILD_INSTRUCTIONS.md  # This file
```

---

## 🔄 Rebuilding After Changes

### Rust Changes
```bash
cd rust-wallet
cargo build --release
# Restart: cargo run --release
```

### Frontend Changes
```bash
cd frontend
npm run build  # Or just keep `npm run dev` running
```

### C++ Changes
```bash
cd cef-native/build
cmake --build . --config Release
# Restart: ./bin/Release/HodosBrowserShell.exe
```

---

## 🎯 Quick Reference

| Component | Port | Auto-launched? | Command |
|-----------|------|----------------|---------|
| **Rust Wallet** | 3301 | ✅ Yes | `cd rust-wallet && cargo run --release` |
| **Adblock Engine** | 3302 | ✅ Yes | `cd adblock-engine && cargo run --release` |
| **Frontend** | 5137 | ❌ No | `cd frontend && npm run dev` |
| **CEF Browser** | — | — | `cd cef-native/build/bin/Release && ./HodosBrowserShell.exe` |

**Minimal run:** Start frontend (`npm run dev`), then launch browser. Wallet and adblock auto-start.

---

## 📝 Notes

- **vcpkg location**: Can be anywhere, but `C:\Dev\vcpkg` is recommended
- **CEF version**: 136.1.6 is tested, newer versions may work
- **Triplet**: MUST use `x64-windows-static` for static linking
- **First build**: Expect 30-60 minutes for vcpkg + CEF wrapper + project
- **Incremental builds**: Much faster (< 5 minutes for C++ changes)

---

## 🚀 Next Steps

- For macOS build instructions, see `MACOS_BUILD_INSTRUCTIONS.md`
- For macOS porting work, see `development-docs/Final-MVP-Sprint/macos-port/MACOS-PORT-HANDOVER.md`
- For development workflow, see `PROJECT_OVERVIEW.md`
- For project architecture, see root `CLAUDE.md`

---

**Build Issues?** Check troubleshooting section above or open an issue on GitHub.

*Last Updated: 2026-03-09*
