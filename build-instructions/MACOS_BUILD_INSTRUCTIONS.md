# macOS Build Instructions - HodosBrowser

## 🎯 Overview

Step-by-step instructions for building HodosBrowser on macOS. This guide covers all three components: CEF native shell (C++), Rust wallet backend, and React frontend (TypeScript).

**Status**: ⚠️ **PARTIAL IMPLEMENTATION** - CMake configuration ready, C++ window code needs macOS porting.

**Estimated Setup Time**: 2-3 hours (first time)

---

## 📋 Prerequisites

### Required Software

| Software | Version | Installation |
|----------|---------|--------------|
| **Xcode Command Line Tools** | Latest | `xcode-select --install` |
| **Homebrew** | Latest | See below |
| **CMake** | 3.20+ | `brew install cmake` |
| **Rust** | Latest stable | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Node.js** | 18+ | `brew install node` |
| **Git** | Latest | Included with Xcode |

### Install Homebrew

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

### Install Required Packages

```bash
# Install build tools
brew install cmake

# Install dependencies
brew install openssl nlohmann-json sqlite3

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install Node.js
brew install node
```

### Verify Installations

```bash
cmake --version        # Should be 3.20+
rustc --version        # Should be latest stable
node --version         # Should be 18+
brew --version         # Should show Homebrew version
```

---

## 🌐 Step 1: Download CEF Binaries

CEF (Chromium Embedded Framework) binaries must be downloaded separately for macOS.

### Download

1. Visit [CEF Automated Builds](https://cef-builds.spotifycdn.com/index.html)
2. Download **macOS 64-bit** or **macOS ARM64** (for M1/M2/M3) - Standard Distribution
3. **Recommended**: Match version to Windows build (136.1.6) or use latest stable

### Extract

```bash
# Extract to project root
# Should create: ./cef-binaries/ directory
# macOS CEF structure:
# cef-binaries/
# ├── Release/
# │   └── Chromium Embedded Framework.framework/
# ├── Resources/
# ├── include/
# └── libcef_dll/
```

### Build CEF Wrapper

The CEF wrapper library must be built from source on macOS.

```bash
# Navigate to wrapper directory
cd cef-binaries/libcef_dll/wrapper

# Create build directory
mkdir build
cd build

# Configure (Homebrew packages found automatically)
cmake .. -DCMAKE_BUILD_TYPE=Release

# Build
cmake --build . --config Release
```

**Verify Wrapper Built:**
```bash
# Check that library exists
ls -la Release/libcef_dll_wrapper.a
```

---

## 🦀 Step 2: Build Rust Wallet

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

📁 Wallet directory: ~/Library/Application Support/HodosBrowser/wallet
✅ Database initialized
✅ Domain whitelist manager initialized
✅ BRC-33 message relay initialized
✅ Auth session manager initialized
...
Listening on: http://127.0.0.1:3301
```

Press `Ctrl+C` to stop. You'll run this in a separate terminal later.

---

## ⚛️ Step 3: Build React Frontend

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

## 🏗️ Step 4: Build CEF Native Shell

⚠️ **IMPORTANT**: Current CMake configuration is ready, but C++ window code requires macOS porting.

### Configure CMake

```bash
cd cef-native

# Configure (Homebrew packages found automatically)
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
```

**Expected CMake Output:**
```
-- vcpkg triplet: arm64-osx (or x64-osx for Intel Macs)
-- macOS deployment target: 10.15
-- macOS architecture: arm64 (or x86_64 for Intel Macs)
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

**⚠️ Current Status**: Build will **FAIL** due to missing macOS-specific window code.

**Error Expected:**
```
cef_browser_shell.cpp: No such file or directory (macOS version)
```

---

## ⚠️ Missing Implementation: macOS Window Code

The current C++ code uses Windows-specific APIs (HWND, Win32). To complete macOS support, the following files need to be created:

### Required Files

1. **`cef_browser_shell_mac.mm`** (Objective-C++)
   - macOS window creation using NSWindow/NSView
   - Replace Win32 WndProc with Objective-C message handlers
   - CEF integration with Cocoa

2. **Platform Abstraction Headers** (Optional but recommended)
   - `include/platform/window_mac.h`
   - `include/platform/window_win.h`
   - Common interface for window operations

### What Needs to Be Ported

From `cef_browser_shell.cpp` (Windows):
- **Window Creation**: `CreateWindow()` → `NSWindow` + `NSView`
- **Window Handles**: `HWND` → `NSWindow*`
- **Message Loop**: `WndProc` → Objective-C selectors
- **Window Positioning**: `SetWindowPos()` → `setFrame:`
- **Event Handling**: Windows messages → Cocoa events

### Estimated Effort

- **Minimal Port**: 200-300 lines of Objective-C++ code (1-2 days)
- **Full Feature Parity**: 500-800 lines with overlays (3-5 days)

---

## 📦 What Works on macOS (Without Window Code)

Even without the macOS window implementation, these components work perfectly:

✅ **Rust Wallet Backend**
```bash
cd rust-wallet
cargo run --release
# Works on macOS - 100% functional
```

✅ **React Frontend**
```bash
cd frontend
npm run dev
# Works on macOS - 100% functional
```

✅ **CMake Configuration**
```bash
cd cef-native
cmake -S . -B build
# Configures successfully with proper macOS settings
```

---

## 🚀 Development Workflow (Current State)

### Option 1: Develop Rust + Frontend on macOS

```bash
# Terminal 1: Rust Wallet
cd rust-wallet
cargo run --release

# Terminal 2: Frontend
cd frontend
npm run dev

# Test wallet API directly
curl http://localhost:3301/health
```

**Benefits:**
- Full wallet backend development
- Full frontend development
- API testing via curl/Postman
- No CEF needed for most features

### Option 2: Implement macOS Window Code

If you want to implement the macOS window code:

1. Create `cef_browser_shell_mac.mm`
2. Use Cocoa/AppKit for window management
3. Follow CEF macOS sample applications as reference
4. Test incrementally

**Reference**: CEF has official macOS sample code at `cef-binaries/tests/`

---

## 🔄 Next Steps to Complete macOS Support

### Priority 1: Basic Window (Minimal Viable Product)

```objective-c
// cef_browser_shell_mac.mm
// Create single NSWindow with CEF browser view
// ~200 lines of code
```

**Delivers:**
- Browser window opens on macOS
- CEF renders content
- Basic functionality works

### Priority 2: Header UI Integration

```objective-c
// Add header NSView for React UI
// Load from http://localhost:5137
```

**Delivers:**
- Header with wallet buttons
- Settings integration
- Full UI functional

### Priority 3: Overlay System

```objective-c
// Port overlay HWND system to NSWindow
// Process-per-overlay architecture
```

**Delivers:**
- Wallet panel overlays
- Settings panel overlays
- Complete feature parity with Windows

---

## 📁 Project Structure (macOS)

```
Hodos-Browser/
├── cef-binaries/                    # CEF binaries (macOS version)
│   ├── Release/
│   │   └── Chromium Embedded Framework.framework/
│   ├── Resources/
│   └── libcef_dll/wrapper/
├── cef-native/
│   ├── CMakeLists.txt               # ✅ Ready for both platforms
│   ├── Info.plist                   # ✅ macOS app bundle config
│   ├── cef_browser_shell.cpp        # ⚠️ Windows-only currently
│   ├── cef_browser_shell_mac.mm     # ❌ TODO: Create this file
│   └── build/
│       └── HodosBrowser.app/        # macOS app bundle (when built)
├── rust-wallet/                     # ✅ Works on macOS
├── frontend/                        # ✅ Works on macOS
└── MACOS_BUILD_INSTRUCTIONS.md      # This file
```

---

## 🚨 Troubleshooting

### CMake: "Could NOT find OpenSSL"

**Problem**: Homebrew packages not found.

**Solution**:
```bash
# Install via Homebrew
brew install openssl nlohmann-json sqlite3

# Link OpenSSL (if needed)
brew link openssl --force

# Reconfigure
cd cef-native
rm -rf build
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
```

### CMake: "Triplet not set"

**Check output:**
```bash
cmake -S . -B build | grep "triplet"
# Should show: arm64-osx or x64-osx
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

## 🎯 Quick Reference

| Component | Port | Works on macOS? | Command |
|-----------|------|-----------------|---------|
| **Rust Wallet** | 3301 | ✅ Yes | `cd rust-wallet && cargo run --release` |
| **Frontend** | 5137 | ✅ Yes | `cd frontend && npm run dev` |
| **CEF Browser** | - | ⚠️ Partial | Needs macOS window code |

---

## 📝 Implementation Checklist

### CMake & Build System
- [x] Platform detection (`APPLE` vs `WIN32`)
- [x] macOS deployment target (10.15+)
- [x] Architecture detection (arm64 vs x86_64)
- [x] macOS frameworks linking (Cocoa, AppKit, etc.)
- [x] Info.plist for app bundle
- [x] Build configuration

### C++ Code (TODO)
- [ ] Create `cef_browser_shell_mac.mm`
- [ ] NSWindow creation and management
- [ ] CEF browser view integration
- [ ] Event handling (Cocoa events)
- [ ] Window positioning and resizing
- [ ] Header UI integration
- [ ] Overlay system (NSWindow-based)

### Testing
- [x] Rust wallet on macOS
- [x] Frontend on macOS
- [x] CMake configuration
- [ ] CEF window creation
- [ ] Full integration test

---

## 💡 Contributing macOS Support

Want to implement the macOS window code? Here's how to start:

1. **Study CEF macOS samples** in `cef-binaries/tests/cefsimple/`
2. **Create minimal window** in `cef_browser_shell_mac.mm`
3. **Test incrementally** - window first, then CEF, then features
4. **Follow Cocoa patterns** - NSApplicationDelegate, NSWindowDelegate
5. **Reference existing Windows code** for business logic

**Estimated effort**: 1-2 weeks for full feature parity with Windows build.

---

**Questions?** Check TECH_STACK_INTEGRATION.md for cross-platform implementation guidance.

*Last Updated: 2025-12-31 (Phase 2: CMake Configuration Complete)*
