# Hodos Browser - Windows Build Guide

*Last Updated: 2026-08-03*

## Prerequisites

| Software | Version | Link |
|----------|---------|------|
| Visual Studio 2022 | Community+ | [Download](https://visualstudio.microsoft.com/downloads/) |
| CMake | 3.21+ | [Download](https://cmake.org/download/) |
| Rust | Latest stable | [Download](https://rustup.rs/) |
| Node.js | 18+ | [Download](https://nodejs.org/) |
| vcpkg | Latest | [GitHub](https://github.com/Microsoft/vcpkg) |
| Inno Setup 6 | (for installer) | [Download](https://jrsoftware.org/isinfo.php) |

CMake 3.21 is the binding floor for a *full* build: `cef-native/CMakeLists.txt` itself only
declares 3.15, but the CEF wrapper the shell links against (`cef-binaries/CMakeLists.txt`)
requires 3.21.

This table is the toolchain floor only. The authoritative list of **per-layer build inputs** —
vendored SDKs, vcpkg packages, the CEF binary distribution — is owned by the layer doc,
`cef-native/CLAUDE.md`. Check it before a first build.


## Building

### Automated Release Build

`scripts/build-release.ps1` runs the full Windows release pipeline end to end. The canonical
phase-by-phase description of that pipeline lives in
`development-docs/DevOps-CICD/BUILD_AND_RELEASE.md`.

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

> The release script **does not configure** the CEF shell — it only runs `cmake --build` for it.
> Run the configure line from Manual Build below once before the first automated release build.
> (CI does this explicitly in `.github/workflows/release.yml`, adding `-DOPENSSL_ROOT_DIR`,
> `-DAPP_VERSION` and `-DAPP_BUILD_NUMBER` on top of the toolchain file.)

### Manual Build (individual components)

Per-layer build detail is owned by each layer's `CLAUDE.md` (`cef-native/CLAUDE.md` for the
shell). What this section carries is the **Windows ordering** — the CEF wrapper library must
exist before the shell can link against it.

```bash
# 0. CEF wrapper (first time only) - produces libcef_dll_wrapper.lib
cd cef-binaries/libcef_dll/wrapper && mkdir -p build && cd build
cmake .. && cmake --build . --config Release

# 1. Rust wallet
cd rust-wallet && cargo build --release

# 2. Adblock engine
cd adblock-engine && cargo build --release

# 3. Frontend
cd frontend && npm install && npm run build

# 4. CEF shell - configure once, then build
cd cef-native
cmake -S . -B build -G "Visual Studio 17 2022" -A x64 \
  -DCMAKE_TOOLCHAIN_FILE=<vcpkg_root>/scripts/buildsystems/vcpkg.cmake
cmake --build build --config Release
```

The configure line is needed on a first build and after any `cef-native/CMakeLists.txt` change;
subsequent builds can skip it. It is load-bearing, not optional boilerplate: `cef-native/CMakeLists.txt`
resolves the vcpkg toolchain **before** `project()`, and its `find_package(... REQUIRED)` calls fail
outright without it.

## Running (Development)

Dev builds use a **separate data directory** (`%APPDATA%/HodosBrowserDev`) and **separate backend
ports** from an installed build, so a dev browser and the installed browser can run at the same time.

```bash
# Terminal 1: Frontend dev server (required)
cd frontend && npm run dev

# Terminal 2: Build + launch the browser in dev mode (wallet + adblock auto-start)
cd cef-native && ./win_build_run.sh        # Git Bash
# or:                                      # PowerShell
cd cef-native; .\win_build_run.ps1
```

**Never launch `build/bin/Release/HodosBrowser.exe` directly.** The launcher scripts set
`HODOS_DEV=1` before launching; without it `AppPaths::EnforceDevSafeguard`
(`cef-native/include/core/AppPaths.h`, called from `cef-native/cef_browser_shell.cpp`) detects the
build-directory path and **refuses to start**. The guard exists because a dev build without the
flag resolves to the *production* data directory (`%APPDATA%/HodosBrowser`) and can corrupt real
wallet data.

> `HodosBrowserShell` is the CMake **target** name; the emitted binary is `HodosBrowser.exe`
> (`OUTPUT_NAME` in `cef-native/CMakeLists.txt`). Dev and installed builds ship the *same* image
> name — never match or kill the process by bare image name.

| Component | Dev port (`HODOS_DEV=1`) | Installed / production port | Auto-launched by browser? |
|-----------|--------------------------|-----------------------------|---------------------------|
| Rust Wallet | 31401 | 31301 | Yes |
| Adblock Engine | 31402 | 31302 | Yes |
| Frontend dev server | 5137 | — | No (run manually) |

Ports are **computed, never hardcoded**. Single source of truth on the C++ side is
`cef-native/include/core/PortConfig.h` (`hodos::WalletPort()` / `hodos::WalletUrl()` /
`hodos::AdblockPort()`); mirrored on the Rust side by `wallet_port()` in `rust-wallet/src/main.rs`
and `adblock_port()` in `adblock-engine/src/main.rs`. The two sides must move in lockstep, and the
dev port must apply **only** when `HODOS_DEV=1` so it can never leak into a release build.

## Troubleshooting

**Browser exits with "DEV SAFEGUARD: HODOS_DEV=1 is not set!":** you launched the exe directly out
of the build directory. Use `./win_build_run.sh` / `.\win_build_run.ps1` instead — see Running
(Development) above.

**CMake can't resolve a vcpkg dependency at configure time:** ensure the vcpkg packages are
installed with the `x64-windows-static` triplet and that `-DCMAKE_TOOLCHAIN_FILE` points at your
vcpkg installation. The set of vcpkg packages the shell requires is owned by `cef-native/CLAUDE.md`;
the enforcing source of truth is the `find_package(... REQUIRED)` block in `cef-native/CMakeLists.txt`.

**CEF wrapper not found:** build it first (step 0 in Manual Build above). Verify
`cef-binaries/libcef_dll/wrapper/build/Release/libcef_dll_wrapper.lib` exists — that path is what
`cef-native/CMakeLists.txt` resolves as `WRAPPER_LIB_PATH`.

**Installer step skipped:** install [Inno Setup 6](https://jrsoftware.org/isinfo.php). The build
script auto-detects `ISCC.exe` from Program Files and skips the installer step (portable zip only)
if it isn't found.
