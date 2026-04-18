# macOS Build Instructions - HodosBrowser

## 🎯 Overview

Complete instructions for building and developing HodosBrowser on macOS (Apple Silicon and Intel). This guide covers all three components: CEF native shell (C++/Objective-C++), Rust wallet backend, and React frontend (TypeScript).

**Status**: ✅ **FULLY FUNCTIONAL** - Complete macOS port with all core features working

**Estimated Setup Time**: 2-3 hours (first time)
**Last Updated**: January 2, 2026

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

# Install C++ dependencies
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
clang --version        # Xcode command line tools
```

---

## 🌐 Step 1: Download and Build CEF

### Download CEF Binaries

1. Visit [CEF Automated Builds](https://cef-builds.spotifycdn.com/index.html)
2. Download **macOS ARM64** (for M1/M2/M3) or **macOS x64** (Intel) - Standard Distribution
3. **Version**: 136.1.6 (tested) or latest stable

### Extract and Build CEF Wrapper

```bash
# Extract CEF archive to project root
cd /path/to/Hodos-Browser
tar -xjf cef_binary_*.tar.bz2

# Rename to cef-binaries
mv cef_binary_* cef-binaries

# Build CEF wrapper library
cd cef-binaries
mkdir build
cd build

cmake .. -DCMAKE_BUILD_TYPE=Release
cmake --build . --target libcef_dll_wrapper --config Release
```

**Verify wrapper built:**
```bash
ls -lh libcef_dll_wrapper/libcef_dll_wrapper.a
# Should show ~5MB file
```

---

## 🦀 Step 2: Build Rust Wallet

```bash
cd rust-wallet

# Build release version
cargo build --release

# Test (use launcher script — sets HODOS_DEV=1 for dev isolation)
# From project root: ./dev-wallet.sh
```

**Expected output:**
```
DEV MODE: Launching wallet (data -> HodosBrowserDev)
🦀 Bitcoin Browser Wallet (Rust)
🔧 DEV MODE: Using HodosBrowserDev data directory
📁 Wallet directory: ~/Library/Application Support/HodosBrowserDev/wallet
✅ Database initialized
...
Listening on: http://127.0.0.1:3301
```

Press `Ctrl+C` to stop. You'll run this in a separate terminal later.

**Storage Location:** `~/Library/Application Support/HodosBrowser/wallet/wallet.db`

---

## ⚛️ Step 3: Build React Frontend

```bash
cd frontend

# Install dependencies
npm install

# Start dev server (for development)
npm run dev
# Frontend will be available at http://127.0.0.1:5137
```

Press `Ctrl+C` to stop. You'll run this in a separate terminal later.

---

## 🏗️ Step 4: Build CEF Native Shell (C++ Browser)

### Configure CMake

```bash
cd cef-native

# Clean build recommended for first time
rm -rf build

# Configure
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
```

**Expected output:**
```
-- vcpkg triplet: arm64-osx (or x64-osx for Intel)
-- Found OpenSSL: /opt/homebrew/...
-- nlohmann_json include: /opt/homebrew/include
-- sqlite3 library: ...
-- Adding macOS-specific sources
-- CEF framework found: .../Chromium Embedded Framework.framework
-- Configured helper: HodosBrowser Helper
-- Configured helper: HodosBrowser Helper (Alerts)
-- Configured helper: HodosBrowser Helper (GPU)
-- Configured helper: HodosBrowser Helper (Plugin)
-- Configured helper: HodosBrowser Helper (Renderer)
-- Configuring done
-- Generating done
```

### Build

```bash
cd build

# Build all targets (main app + 5 helper bundles)
cmake --build . --config Release
```

**Build time:** ~2-3 minutes for full build

**Output:** `bin/HodosBrowserShell.app/`

### Copy Helper Bundles (IMPORTANT!)

After each build, copy helpers into the main app bundle:

```bash
cd bin
cp -r "HodosBrowser Helper"*.app HodosBrowserShell.app/Contents/Frameworks/
```

**Why needed:** Helpers are built separately and must be nested inside the main app bundle for CEF to find them.

**Verification:**
```bash
ls HodosBrowserShell.app/Contents/Frameworks/
# Should show:
# - Chromium Embedded Framework.framework/
# - HodosBrowser Helper.app
# - HodosBrowser Helper (Alerts).app
# - HodosBrowser Helper (GPU).app
# - HodosBrowser Helper (Plugin).app
# - HodosBrowser Helper (Renderer).app
```

---

## ✅ Step 5: Run HodosBrowser

> **Note:** On Windows, the C++ shell auto-launches the Rust wallet and adblock engine. On macOS, auto-launch is not yet implemented (it's part of the macOS feature parity sprint — see `development-docs/Final-MVP-Sprint/macos-port/MACOS-PORT-HANDOVER.md`). For now, you need to start them manually. If you don't need to see their logs, you can still run them in the background.

You need **three terminals** running simultaneously (four if you also want adblock):

### Terminal 1: Rust Wallet Backend

```bash
# From project root (sets HODOS_DEV=1 for dev/production isolation)
./dev-wallet.sh

# Wait for:
# "Listening on: http://127.0.0.1:3301"
```

**Leave running** - Provides wallet/crypto backend

### Terminal 2: Adblock Engine (optional but recommended)

```bash
# From project root (sets HODOS_DEV=1 for dev/production isolation)
./dev-adblock.sh

# Wait for:
# "Listening on: http://127.0.0.1:3302"
```

**Leave running** - Provides ad/tracker blocking

> **⚠️ Dev Isolation:** All dev launcher scripts set `HODOS_DEV=1` so dev data goes to `~/Library/Application Support/HodosBrowserDev/` (separate from the installed app). Dev builds refuse to start without it.

### Terminal 3: React Frontend Dev Server

```bash
cd frontend
npm run dev

# Wait for:
# "Local: http://127.0.0.1:5137"
```

**Leave running** - Serves React UI with hot reload

### Terminal 4: macOS Browser

```bash
cd cef-native/build/bin
open -a HodosBrowserShell.app

# Or run directly for console output:
./HodosBrowserShell.app/Contents/MacOS/HodosBrowserShell
```

> **Tip:** Once the macOS auto-launch feature is implemented, you'll only need the frontend dev server + browser (same as Windows). Running wallet/adblock manually will still be useful for debugging with console logs.

### Expected Behavior

✅ **Browser window appears** with header and content area
✅ **Header loads React UI** from http://localhost:5137
✅ **Webview displays** default page or navigation target
✅ **Tab management** - Create, switch, close tabs
✅ **Navigation works** - Address bar, back/forward, reload
✅ **No crashes** - Stable operation

---

## 🔄 Development Workflow

### After Code Changes

**Rust changes:**
```bash
cd rust-wallet
cargo build --release
# Restart: cargo run --release
```

**Frontend changes:**
```bash
cd frontend
npm run build  # Or keep dev server running for hot reload
```

**C++/Objective-C++ changes:**
```bash
cd cef-native
cmake --build build --config Release

# IMPORTANT: Copy helpers after rebuild
cd build/bin
cp -r "HodosBrowser Helper"*.app HodosBrowserShell.app/Contents/Frameworks/
```

**Incremental builds:** Only changed files recompile (~30 seconds)

---

## 🚨 Troubleshooting

### Build Issues

**"CEF framework not found"**
```bash
# Verify CEF was extracted correctly
ls cef-binaries/Release/Chromium\ Embedded\ Framework.framework/

# Should exist with ~200MB framework
```

**"libcef_dll_wrapper.a not found"**
```bash
# Rebuild wrapper from cef-binaries root
cd cef-binaries/build
cmake --build . --target libcef_dll_wrapper
```

**"nlohmann/json.hpp not found"**
```bash
# Install via Homebrew
brew install nlohmann-json

# Verify
ls /opt/homebrew/include/nlohmann/json.hpp
```

### Runtime Issues

**App quits immediately with "Opening in existing browser session"**
- Another instance is running
- Kill all instances: `pkill -9 HodosBrowserShell`
- Try again: `open -a HodosBrowserShell.app`

**App crashes on launch**
- Check helper bundles are copied: `ls HodosBrowserShell.app/Contents/Frameworks/HodosBrowser\ Helper*.app`
- If missing, copy manually (see Step 4)
- Check crash logs: `ls -lt ~/Library/Logs/DiagnosticReports/HodosBrowser*`

**Window appears but blank/white**
- Check frontend is running: `lsof -i :5137 | grep LISTEN`
- Check Rust wallet is running: `lsof -i :3301 | grep LISTEN`
- Grant network access if prompted by macOS
- Check DevTools: `curl -s http://127.0.0.1:9222/json`

**Tabs don't work or crash**
- Ensure you built the LATEST code (after 2026-01-02)
- Verify helper bundles are copied
- Check debug logs: `tail -50 cef-native/build/bin/debug_output.log`

### Frontend Connection

**React UI doesn't load**
```bash
# Verify Vite dev server responds
curl http://127.0.0.1:5137

# Should return HTML
```

**"Network access" permission prompt**
- **Always Allow** when macOS asks about local network access
- Or manually enable: System Settings → Privacy & Security → Local Network → HodosBrowserShell

---

## 🎯 What Works on macOS

### Core Functionality (100% Working)
- ✅ **Window System** - NSWindow with header and webview areas
- ✅ **React UI** - Full frontend rendering in 80px header
- ✅ **Webview** - Display any website
- ✅ **Navigation** - Address bar, back/forward, reload
- ✅ **Tab Management** - Create, switch, close tabs with UI updates
- ✅ **Multi-process** - Each tab gets own renderer process
- ✅ **CEF Rendering** - SetAsChild child window rendering
- ✅ **Network** - Localhost and external URLs

### Helper Processes (Required for CEF)
- ✅ 5 helper app bundles (Base, Alerts, GPU, Plugin, Renderer)
- ✅ Proper Info.plist with LSUIElement (no Dock icons)
- ✅ Framework paths configured correctly
- ✅ Process isolation working

### Not Yet at Windows Feature Parity

The macOS C++ layer needs additional work for full feature parity. The Rust wallet and adblock engine are fully cross-platform and work identically on macOS.

For the complete gap analysis and sprint plan, see: **`development-docs/Final-MVP-Sprint/macos-port/MACOS-PORT-HANDOVER.md`**

Key gaps: 6 missing overlay types, HTTP singleton porting (WinHTTP → libcurl), process auto-launch, multi-window support, keyboard shortcuts (Cmd vs Ctrl).

---

## 📁 Project Structure (macOS-specific)

```
Hodos-Browser/
├── cef-binaries/                    # CEF framework (download separately)
│   ├── Release/
│   │   └── Chromium Embedded Framework.framework/
│   ├── build/
│   │   └── libcef_dll_wrapper/
│   │       └── libcef_dll_wrapper.a
│   └── Resources/
├── cef-native/
│   ├── CMakeLists.txt               # Cross-platform build config
│   ├── Info.plist                   # macOS app bundle metadata
│   ├── cef_browser_shell_mac.mm     # macOS entry point + windows
│   ├── mac/
│   │   ├── process_helper_mac.mm    # Helper process entry point
│   │   └── helper-Info.plist.in     # Helper bundle template
│   ├── src/
│   │   ├── handlers/                # Cross-platform CEF handlers
│   │   ├── core/
│   │   │   ├── TabManager_mac.mm    # macOS tab management
│   │   │   ├── NavigationHandler.cpp # Cross-platform
│   │   │   └── Logger.cpp/.h        # Shared logging
│   └── build/
│       └── bin/
│           └── HodosBrowserShell.app/
│               ├── Contents/
│               │   ├── MacOS/HodosBrowserShell
│               │   ├── Frameworks/
│               │   │   ├── Chromium Embedded Framework.framework/
│               │   │   └── HodosBrowser Helper*.app (×5)
│               │   └── Resources/
│               └── Info.plist
├── rust-wallet/                     # Works on macOS unchanged
├── frontend/                        # Works on macOS unchanged
└── build-instructions/
    └── MACOS_BUILD_INSTRUCTIONS.md  # This file
```

---

## 🔧 Development Tips

### Debugging

**Enable verbose logging:**
- Logs written to: `cef-native/build/bin/debug_output.log`
- CEF logs: `cef-native/build/bin/debug.log`
- Check after running app

**Access DevTools:**
```bash
# Get DevTools URLs
curl -s http://127.0.0.1:9222/json | python3 -m json.tool

# Open in Chrome/Safari to inspect React UI or webpage
```

**View helper processes:**
```bash
ps aux | grep "HodosBrowser Helper"
# Should show 4-5 helper processes when running
```

### Code Organization

**macOS-only files (never compiled on Windows):**
- `cef_browser_shell_mac.mm` - Main app entry point
- `src/core/TabManager_mac.mm` - Tab management
- `src/handlers/my_overlay_render_handler.mm` - Rendering
- `mac/process_helper_mac.mm` - Helper entry point

**Cross-platform files (work on both):**
- `src/handlers/simple_handler.cpp` - Browser callbacks
- `src/handlers/simple_app.cpp` - CEF app lifecycle
- `src/handlers/simple_render_process_handler.cpp` - V8 injection
- `src/core/NavigationHandler.cpp` - Navigation
- `src/core/Logger.cpp/.h` - Logging

**Platform selection:**
- Controlled by `#ifdef _WIN32` / `#elif defined(__APPLE__)` / `#endif`
- CMakeLists.txt: `if(APPLE)` / `elseif(WIN32)` / `endif()`

### Making Changes

**Adding new features:**
1. Check if Windows version exists
2. Study Windows implementation
3. Create macOS equivalent (use NSView instead of HWND)
4. Update CMakeLists.txt if adding new files
5. Wrap in platform conditionals
6. Test on macOS
7. Verify Windows still compiles (if possible)

**Common patterns:**
```cpp
#ifdef _WIN32
    HWND hwnd = CreateWindow(...);
    ShowWindow(hwnd, SW_SHOW);
#elif defined(__APPLE__)
    NSView* view = [[NSView alloc] initWithFrame:...];
    [parentView addSubview:view];
#endif
```

---

## 🎓 Platform Differences

### Window/View Management

| Windows | macOS |
|---------|-------|
| `HWND` (window handle) | `NSView*` / `NSWindow*` |
| `CreateWindow()` | `[[NSView alloc] initWithFrame:]` |
| `ShowWindow(SW_SHOW/HIDE)` | `[view setHidden:NO/YES]` |
| `DestroyWindow()` | `[view removeFromSuperview]` |
| `SetWindowPos()` | `[view setFrame:]` |
| `GetClientRect()` | `[view bounds]` |

### CEF Integration

| Aspect | Windows | macOS |
|--------|---------|-------|
| **NSApplication** | Not needed | Custom subclass implementing `CefAppProtocol` required |
| **Helper processes** | Single .exe | 5 separate .app bundles |
| **Framework loading** | Automatic DLL load | Explicit `cef_load_library()` call |
| **Subprocess path** | Same .exe | Path to Helper.app bundle |

### Build System

| Tool | Windows | macOS |
|------|---------|-------|
| **Compiler** | MSVC (Visual Studio) | Clang (Xcode) |
| **Package manager** | vcpkg | Homebrew |
| **Generator** | Visual Studio 2022 | Unix Makefiles |
| **App bundle** | Folder with .exe + DLLs | .app bundle structure |

---

## 🚀 Quick Reference

### One-command build (after setup):
```bash
cd cef-native && cmake --build build --config Release && \
cd build/bin && cp -r "HodosBrowser Helper"*.app HodosBrowserShell.app/Contents/Frameworks/
```

### Launch stack (3 terminals):
```bash
# Terminal 1 (from project root)
./dev-wallet.sh

# Terminal 2
cd frontend && npm run dev

# Terminal 3
cd cef-native && ./mac_build_run.sh
```

### Clean rebuild:
```bash
cd cef-native
rm -rf build
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release
cd build/bin && cp -r "HodosBrowser Helper"*.app HodosBrowserShell.app/Contents/Frameworks/
```

---

## 📝 Known Issues & Limitations

### Development Mode Settings

The current build has **development-only** settings:
- `no_sandbox = true` - Sandbox disabled (requires code signing to enable)
- `disable-web-security` - For localhost dev server access
- `in-process-gpu` - GPU in main process (not separate)

**For production:** Enable sandbox, remove web security bypass, use separate GPU process (requires code signing).

### macOS Feature Parity

The macOS C++ layer has foundational support (window, tabs, 5 overlays, rendering) but is behind Windows on features added in Sprints 8-13. The Rust wallet and React frontend work identically on both platforms.

**For the complete feature gap analysis, sprint plan, and implementation guide, see:**
**`development-docs/Final-MVP-Sprint/macos-port/MACOS-PORT-HANDOVER.md`**

---

## 🔐 Security Notes

**Current configuration is for DEVELOPMENT only:**
- Sandbox disabled (requires code signing)
- Web security disabled (for localhost access)
- No code signing (helpers run unsigned)

**For distribution:**
1. Enable sandbox (`settings.no_sandbox = false`)
2. Remove `disable-web-security` flag
3. Code sign all bundles (main app + 5 helpers)
4. Notarize for macOS Catalina+
5. Create installer DMG

**Code signing command (when ready):**
```bash
codesign --deep --force --sign "Developer ID Application: Your Name" HodosBrowserShell.app
```

---

## 🎯 Success Criteria

After following these instructions, you should have:

- [x] CEF binaries downloaded and wrapper built
- [x] Rust wallet compiles and runs
- [x] React frontend serves on port 5137
- [x] C++ browser compiles with 5 helper bundles
- [x] Helpers copied into app bundle Frameworks
- [x] App launches and shows window
- [x] React UI renders in header (80px bar at top)
- [x] Webview displays web content
- [x] Can navigate to websites
- [x] Can create and switch between tabs
- [x] Can close tabs without app quitting
- [x] Tab bar updates when tabs close

**If all checked:** ✅ **macOS development environment ready!**

---

## 🆘 Getting Help

**Build errors:**
- Check all prerequisites installed: `cmake --version`, `brew --version`, etc.
- Verify CEF binaries match architecture (ARM64 vs x64)
- Try clean rebuild: `rm -rf build && cmake ...`

**Runtime crashes:**
- Check crash logs: `ls -lt ~/Library/Logs/DiagnosticReports/`
- Enable console output: `./HodosBrowserShell.app/Contents/MacOS/HodosBrowserShell`
- Verify helper bundles copied

**Blank window:**
- Ensure frontend running on :5137
- Grant network access permission
- Check DevTools for errors

---

## 📊 Comparison with Windows Build

| Feature | Windows | macOS | Notes |
|---------|---------|-------|-------|
| **Build time** | 5-10 min | 2-3 min | macOS faster (fewer sources) |
| **Dependencies** | vcpkg | Homebrew | Different package managers |
| **App bundle** | .exe + DLLs | .app bundle | macOS structured bundle |
| **Helpers** | 1 exe | 5 .app bundles | macOS requires separate bundles |
| **Code signing** | Optional | Required for sandbox | macOS stricter |
| **Navigation** | ✅ | ✅ | Fully working |
| **Tabs** | ✅ | ✅ | Fully working |
| **Wallet** | ✅ | ⏳ | Needs porting |
| **History** | ✅ | ⏳ | Needs porting |

---

## 🎓 Advanced Topics

### Using Production Build

```bash
# Build optimized release
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_DEPLOYMENT_TARGET=10.15

# Strip symbols for smaller size
strip HodosBrowserShell.app/Contents/MacOS/HodosBrowserShell
```

### Creating DMG Installer

```bash
# After code signing
hdiutil create -volname "HodosBrowser" -srcfolder HodosBrowserShell.app -ov -format UDZO HodosBrowser.dmg
```

### Enabling Sandbox

1. Code sign main app and all 5 helpers
2. Remove `settings.no_sandbox = true` from cef_browser_shell_mac.mm
3. Rebuild and test
4. Verify helpers launch without errors

---

## ✅ Verification Checklist

Before committing or distributing:

- [ ] Clean build from scratch succeeds
- [ ] All 5 helper bundles build
- [ ] Helpers copy into Frameworks/
- [ ] App launches without errors
- [ ] Window appears with UI
- [ ] React frontend loads
- [ ] Can navigate to websites
- [ ] Multiple tabs work
- [ ] Tabs close properly
- [ ] UI updates when tabs close
- [ ] No crashes during normal use
- [ ] Quit via Cmd+Q or File → Quit works

---

**Questions or issues?** Check the main project documentation or open a GitHub issue.

*Last updated: March 9, 2026 - Updated run instructions, feature parity references*
