# Build Instructions - Bitcoin Browser

## 🎯 Overview

This document provides step-by-step instructions for building the Bitcoin Browser project. The build process involves multiple components: CEF binaries, C++ native shell, Rust wallet backend, and Vite React frontend (TypeScript).

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

**Note**: vcpkg is a C++ package manager that provides pre-built libraries. It needs to be installed separately and the toolchain path must be specified during CMake configuration. The vcpkg packages (OpenSSL, nlohmann/json) are automatically found and linked when using the vcpkg toolchain file.

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

**Note**: A reference copy of this CMakeLists.txt is available at `build-reference/CEF_WRAPPER_CMakeLists.txt` for documentation purposes. The actual file will be present in the CEF binaries when you download them.

**Important**: The wrapper must be built in-place within the `cef-binaries` directory structure. The build output contains paths specific to your system, so each developer must build their own wrapper. You cannot share the built wrapper library between systems.

```bash
# Navigate to the wrapper directory
cd cef-binaries/libcef_dll/wrapper

# Create build directory
mkdir build
cd build

# Configure CMake with vcpkg toolchain
# Replace [vcpkg_root] with your actual vcpkg installation path
cmake .. -DCMAKE_TOOLCHAIN_FILE=[vcpkg_root]/scripts/buildsystems/vcpkg.cmake

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

# Run the wallet server
cargo run --release
# Or run directly: ./target/release/hodos-wallet.exe

# Server starts on http://127.0.0.1:3301
```

**Features:**
- Custom Actix-web HTTP server
- BRC-103/104 mutual authentication
- Custom BSV ForkID SIGHASH implementation
- Transaction creation, signing, broadcasting
- Confirmed mainnet transactions

#### Wallet Storage

The wallet data is stored at: `%APPDATA%/HodosBrowser/wallet/wallet.json`

#### Test the Wallet API

**Rust Wallet Endpoints:**
- `GET http://localhost:3301/wallet/status` - Wallet status
- `POST http://localhost:3301/getVersion` - Get wallet version
- `POST http://localhost:3301/getPublicKey` - Get public key
- `POST http://localhost:3301/createHmac` - Create HMAC for authentication
- `POST http://localhost:3301/verifyHmac` - Verify HMAC
- `POST http://localhost:3301/createSignature` - Create message signature
- `POST http://localhost:3301/verifySignature` - Verify message signature
- `POST http://localhost:3301/.well-known/auth` - BRC-104 authentication
- `POST http://localhost:3301/createAction` - Create transaction
- `POST http://localhost:3301/signAction` - Sign transaction
- `POST http://localhost:3301/processAction` - Process and broadcast transaction

**Test with PowerShell:**
```powershell
# Test Rust wallet (make sure wallet is running)
Invoke-RestMethod -Uri "http://localhost:3301/wallet/status" -Method GET
```

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

**Important**: The CMakeLists.txt currently has some hardcoded paths that need to be addressed. The vcpkg toolchain path and installation prefix must be specified via command-line arguments.

**Working Configuration Command:**

```bash
cd cef-native

# Configure CMake with all required parameters
# Replace the paths with your actual vcpkg installation location
cmake -S . -B build -G "Visual Studio 17 2022" -A x64 ^
  -DCMAKE_TOOLCHAIN_FILE=C:/Users/archb/Dev/vcpkg/scripts/buildsystems/vcpkg.cmake ^
  -DCMAKE_BUILD_TYPE=Release ^
  -DCMAKE_INSTALL_PREFIX=C:/Users/archb/Dev/vcpkg/installed/x64-windows-static ^
  -DOPENSSL_USE_STATIC_LIBS=TRUE
```

**Path Customization:**
- Replace `C:/Users/archb/Dev/vcpkg` with your actual vcpkg installation path
- The `CMAKE_INSTALL_PREFIX` should point to your vcpkg installed packages directory
- Adjust the path based on your vcpkg installation (x64-windows vs x64-windows-static)

**Note**: The vcpkg toolchain file automatically locates packages (OpenSSL, nlohmann/json) from the vcpkg installation. However, the CMakeLists.txt may need updates to properly use vcpkg's package discovery instead of hardcoded paths.

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

### Step 5: Integration Testing

#### Start Rust Wallet Daemon

```bash
# In separate terminal
cd rust-wallet
cargo run --release
```

#### Run Native Shell

```bash
# From cef-native/build/bin/Release/
./HodosBrowserShell.exe
```

## 🚨 Known Issues & TODOs

### CEF Integration Issues
- [x] **CEF Version**: Current working version: `cef_binary_136.1.6+g1ac1b14+chromium-136.0.7103.114_windows64`
- [ ] **Hardcoded Paths**: CMakeLists.txt has hardcoded vcpkg paths that need to be made configurable
- [ ] **vcpkg Toolchain**: Users must specify their vcpkg toolchain path via command-line
- [ ] **OpenSSL Paths**: CMakeLists.txt has hardcoded OpenSSL include paths that should use vcpkg's package discovery
- [x] **Wrapper Build**: Wrapper builds correctly but must be built by each developer

### Build System Issues
- [ ] **vcpkg Path Configuration**: Make vcpkg paths configurable or auto-detectable
- [ ] **Cross-Platform**: Test build process on different platforms
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
- **`frontend/node_modules/`** - Node.js dependencies
- **`frontend/dist/`** - Frontend build output

**Important**: Before building, ensure you have:
1. Downloaded and extracted CEF binaries to `cef-binaries/`
2. Built the CEF wrapper library
3. Installed vcpkg and configured the toolchain path

## 🚀 Future Build Considerations

### Multi-Platform Support
- 🟡 **Windows**: Current CEF implementation
- 🟡 **macOS**: CEF with Cocoa integration
- 🟡 **Linux**: CEF with GTK integration
- 🟡 **Mobile**: React Native with native modules

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
# Example: -DCMAKE_TOOLCHAIN_FILE=C:/vcpkg/scripts/buildsystems/vcpkg.cmake
# Ensure vcpkg packages are installed: vcpkg install openssl:x64-windows-static nlohmann-json:x64-windows-static
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
