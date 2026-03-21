# Hodos Browser - Windows Build Guide

## Prerequisites

| Software | Version | Link |
|----------|---------|------|
| Visual Studio 2022 | Community+ | [Download](https://visualstudio.microsoft.com/downloads/) |
| CMake | 3.20+ | [Download](https://cmake.org/download/) |
| Rust | Latest stable | [Download](https://rustup.rs/) |
| Node.js | 18+ | [Download](https://nodejs.org/) |
| vcpkg | Latest | [GitHub](https://github.com/Microsoft/vcpkg) |
| Inno Setup 6 | (for installer) | [Download](https://jrsoftware.org/isinfo.php) |


## Building

### Automated Release Build

The build script compiles all components, assembles a staging directory, creates a portable zip, and builds the installer.

### Powershell
```powershell
# Full build (all components + installer)
.\scripts\build-release.ps1

# Skip compilation (use existing build artifacts)
.\scripts\build-release.ps1 -SkipBuild

# Skip installer (portable zip only)
.\scripts\build-release.ps1 -NoInstaller

# Custom version
.\scripts\build-release.ps1 -Version "0.2.0-alpha.1"
```

### Git Bash
```bash
# Full build (all components + installer)
powershell.exe -ExecutionPolicy Bypass -File scripts/build-release.ps1

# Skip compilation (use existing build artifacts)
powershell.exe -ExecutionPolicy Bypass -File scripts/build-release.ps1 -SkipBuild

# Skip installer (portable zip only)
powershell.exe -ExecutionPolicy Bypass -File scripts/build-release.ps1 -NoInstaller

# Custom version
powershell.exe -ExecutionPolicy Bypass -File scripts/build-release.ps1 -Version "0.2.0-alpha.1"
```

**Output** (in `dist/`):
- `HodosBrowser-<version>-portable.zip` - standalone zip
- `HodosBrowser-<version>-setup.exe` - Windows installer

### Manual Build (individual components)

```bash
# Rust wallet
cd rust-wallet && cargo build --release

# Adblock engine
cd adblock-engine && cargo build --release

# Frontend
cd frontend && npm install && npm run build

# CEF shell
cd cef-native && cmake --build build --config Release
```

## Running (Development)

```bash
# Terminal 1: Frontend dev server (required)
cd frontend && npm run dev

# Terminal 2: Launch browser (wallet + adblock auto-start)
cd cef-native/build/bin/Release && ./HodosBrowserShell.exe
```

| Component | Port | Auto-launched by browser? |
|-----------|------|--------------------------|
| Rust Wallet | 31301 | Yes |
| Adblock Engine | 31302 | Yes |
| Frontend | 5137 | No (run manually) |

## Troubleshooting

**CMake can't find OpenSSL/sqlite/json:** Ensure vcpkg packages use the `x64-windows-static` triplet and the `-DCMAKE_TOOLCHAIN_FILE` flag points to your vcpkg installation.

**CEF wrapper not found:** Build it per step 3 above. Verify `cef-binaries/libcef_dll/wrapper/build/Release/libcef_dll_wrapper.lib` exists.

**Installer step skipped:** Install [Inno Setup 6](https://jrsoftware.org/isinfo.php). The build script auto-detects it from Program Files.
