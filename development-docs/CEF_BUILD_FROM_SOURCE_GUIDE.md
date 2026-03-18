# CEF Build from Source Guide — Proprietary Codecs Enabled

> **Purpose**: Build CEF (Chromium Embedded Framework) from source with H.264, AAC, and MP3 codec support for Hodos Browser.
>
> **Created**: 2026-03-01
> **Target**: CEF 136 (Chromium 136, branch 7103)
> **Platform**: Windows 10/11 x64

---

## Table of Contents

1. [Overview](#1-overview)
2. [Why Build from Source](#2-why-build-from-source)
3. [System Requirements](#3-system-requirements)
4. [Pre-Build Checklist](#4-pre-build-checklist)
5. [Step-by-Step Build Instructions](#5-step-by-step-build-instructions)
6. [Integrating with Hodos Browser](#6-integrating-with-hodos-browser)
7. [Verification & Testing](#7-verification--testing)
8. [Lessons Learned](#8-lessons-learned-real-world-build-notes)
9. [Troubleshooting](#9-troubleshooting)
10. [Maintenance & Updates](#10-maintenance--updates)
11. [macOS Build](#11-macos-build)
12. [Appendix](#12-appendix)

---

## 1. Overview

### What We're Building

- **CEF Version**: 136.x (matching our current prebuilt version)
- **Chromium Version**: 136.0.7103.x
- **CEF Branch**: 7103
- **Target Architecture**: Windows x64
- **Key Feature**: Proprietary codec support (H.264, AAC, MP3)

### Timeline Estimate

| Phase | Duration | Notes |
|-------|----------|-------|
| Environment setup | 1-2 hours | One-time |
| Source download | 1-3 hours | ~30GB download, depends on connection |
| Build (compile) | 6-10 hours | First build; depends on CPU/RAM. ~96K object files total |
| Packaging (make_distrib.py) | ~7 minutes | ~404 seconds |
| Integration | 1-2 hours | Replace binaries, rebuild wrapper |
| Testing | 1-2 hours | Verify codecs work |
| **Total** | **~10-15 hours** | Expect overnight; can be split across days |

### Actual Build Experience (2026-03-12)

The full build (download + compile + package) took roughly overnight (~10-12 hours total). The compile phase produced 78,821 object files before an interruption (Windows auto-restart — see Lessons Learned), then ~17,336 more on resume. Packaging via `make_distrib.py` took ~404 seconds (~7 minutes).

**The build IS resumable.** Ninja tracks completed work in `.ninja_log`. If the build is interrupted, just re-run the script — ninja skips already-compiled objects and picks up where it left off.

### Can We Do This Now?

**YES — and it has been done successfully (2026-03-12).** The built binaries work identically in dev and production. Building now gives you:
- Time to troubleshoot any build issues
- Ability to test codec support immediately
- Confidence the production build will work

---

## 2. Why Build from Source

### The Problem

Spotify's prebuilt CEF binaries exclude proprietary codecs due to patent licensing:

```javascript
// This returns empty string ("") with prebuilt CEF
video.canPlayType('video/mp4; codecs="avc1.42E01E"')  // H.264
audio.canPlayType('audio/mp4; codecs="mp4a.40.2"')   // AAC
```

### Sites That Don't Work Without Codecs

| Site | Issue |
|------|-------|
| x.com (Twitter) | Videos won't play, animated GIFs broken |
| Reddit | Video spinner forever |
| Twitch | Many streams fail |
| Instagram | Videos broken |
| TikTok | Nothing plays |
| Most news sites | Embedded videos fail |

### The Solution

Build CEF with these compile-time flags:
```
proprietary_codecs=true
ffmpeg_branding=Chrome
```

This enables H.264, AAC, MP3, and other codecs that Chrome supports.

---

## 3. System Requirements

### Hardware Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| **CPU** | 4 cores | 8+ cores |
| **RAM** | 16 GB | 32 GB |
| **Disk Space** | 100 GB free | 150 GB free (SSD strongly recommended) |
| **Network** | Stable connection | Fast connection (30GB+ download) |

### Why So Much Disk Space?

| Component | Size |
|-----------|------|
| depot_tools | ~500 MB |
| Chromium source | ~25 GB |
| Build output | ~30-50 GB |
| CEF patches/output | ~5 GB |
| **Total** | **~60-80 GB** |

### Software Requirements

| Software | Version | Notes |
|----------|---------|-------|
| **Windows** | 10 or 11 | 64-bit required |
| **Visual Studio 2022** | Latest | Community edition works |
| **Windows SDK** | 10.0.22621.0+ | With Debugging Tools |
| **Python** | 3.9 - 3.11 | NOT 3.12+ (compatibility issues) |
| **Git** | Latest | Included in depot_tools |

---

## 4. Pre-Build Checklist

### 4.1 Install Visual Studio 2022

1. Download [Visual Studio 2022 Community](https://visualstudio.microsoft.com/downloads/)
2. During installation, select these workloads:
   - ✅ **Desktop development with C++**
   - ✅ **Game development with C++** (for additional SDKs)
3. In "Individual components", ensure these are selected:
   - ✅ Windows 10/11 SDK (latest)
   - ✅ C++ CMake tools for Windows
   - ✅ C++ Clang Compiler for Windows

### 4.2 Install Windows SDK Debugging Tools

**Critical**: This is NOT installed by default!

1. Open **Settings** → **Apps** → **Installed apps**
2. Find "Windows Software Development Kit" 
3. Click **Modify**
4. Check ✅ **Debugging Tools for Windows**
5. Click **Change**

### 4.3 Verify Python Version

```powershell
python --version
# Should show Python 3.9.x, 3.10.x, or 3.11.x
# If you have Python 3.12+, install 3.11 and adjust PATH
```

If you need to install Python 3.11:
1. Download from [python.org](https://www.python.org/downloads/release/python-3119/)
2. During install, check "Add Python to PATH"
3. Verify: `python --version`

### 4.4 Disable Windows Defender Real-Time Scanning for Build Directory

**This is critical for build performance.** Windows Defender scans every file read/write. Chromium builds touch millions of files — Defender can 2-5x your build time.

1. Open **Windows Security** → **Virus & threat protection** → **Manage settings**
2. Scroll to **Exclusions** → **Add or remove exclusions**
3. Add folder exclusion: `C:\cef\` (or wherever your build dir is)
4. Also exclude `C:\cef\depot_tools\`

**Re-enable scanning after the build is complete** if you want.

### 4.5 Create Directory Structure

**Important**: Use short paths with ASCII-only characters!

```powershell
# Create the build directories (short paths required!)
mkdir C:\cef
mkdir C:\cef\automate
mkdir C:\cef\depot_tools
mkdir C:\cef\chromium_git
```

**Why short paths?** Windows has a 260-character path limit. Chromium's deep directory structure can exceed this with longer base paths.

---

## 5. Step-by-Step Build Instructions

### Step 1: Install depot_tools

depot_tools is Google's collection of build tools for Chromium projects.

```powershell
# Download depot_tools
cd C:\cef
Invoke-WebRequest -Uri "https://storage.googleapis.com/chrome-infra/depot_tools.zip" -OutFile "depot_tools.zip"

# Extract (use 7-Zip or Windows extraction - be careful with hidden .git folder)
Expand-Archive -Path "depot_tools.zip" -DestinationPath "C:\cef\depot_tools" -Force

# Or use 7-Zip (recommended - preserves hidden folders):
# 7z x depot_tools.zip -oC:\cef\depot_tools

# Run the update script
cd C:\cef\depot_tools
.\update_depot_tools.bat
```

### Step 2: Add depot_tools to PATH

**Option A: Temporary (current session only)**
```powershell
$env:PATH = "C:\cef\depot_tools;$env:PATH"
```

**Option B: Permanent (recommended)**
1. Press `Win + R`, type `SystemPropertiesAdvanced`, press Enter
2. Click **Environment Variables**
3. Under "System variables", find **Path**
4. Click **Edit** → **New**
5. Add: `C:\cef\depot_tools`
6. Click **OK** on all dialogs
7. Restart PowerShell/terminal

**Verify it works:**
```powershell
gclient --version
# Should output version info, not "command not found"
```

### Step 3: Download automate-git.py

```powershell
cd C:\cef\automate

# Download the build automation script
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/chromiumembedded/cef/master/tools/automate/automate-git.py" -OutFile "automate-git.py"
```

### Step 4: Create the Build Script

Create `C:\cef\chromium_git\build_cef.bat` (also saved as `development-docs/build_hodos_cef.bat`):

```batch
@echo off
REM ============================================
REM CEF Build Script for Hodos Browser
REM Builds CEF 136 with proprietary codecs (H.264, AAC, MP3)
REM
REM USAGE: Copy this file to C:\cef\chromium_git\ and run from there
REM        Run in a normal cmd or PowerShell (NOT Developer Command Prompt)
REM ============================================

REM Set Visual Studio version and path
set GYP_MSVS_VERSION=2022

REM CRITICAL: Use local VS install, not Google's internal toolchain
set DEPOT_TOOLS_WIN_TOOLCHAIN=0

REM Tell Chromium where BuildTools edition is installed
set GYP_MSVS_OVERRIDE_PATH=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools
set vs2022_install=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools

REM GN build defines for proprietary codecs
REM NOTE: chrome_pgo_phase=0 disables PGO profiling (avoids needing PGO profile data)
set GN_DEFINES=is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0

REM Archive format
set CEF_ARCHIVE_FORMAT=tar.bz2

REM Ensure depot_tools is on PATH
set PATH=C:\cef\depot_tools;%PATH%

REM Step 1: Run gclient sync to download all dependencies (ninja, node, etc.)
echo.
echo === Step 1: Running gclient sync to download dependencies ===
echo.
cd /d C:\cef\chromium_git\chromium
call C:\cef\depot_tools\gclient.bat sync --nohooks --no-history
if %ERRORLEVEL% NEQ 0 (
  echo.
  echo WARNING: gclient sync had errors, continuing...
  echo.
)

REM Step 1b: Run hooks
echo.
echo === Step 1b: Running gclient runhooks ===
echo.
call C:\cef\depot_tools\gclient.bat runhooks
if %ERRORLEVEL% NEQ 0 (
  echo.
  echo WARNING: gclient runhooks had errors, attempting build anyway...
  echo.
)

REM Step 2: Run the automated build
echo.
echo === Step 2: Running automate-git.py build ===
echo.
cd /d C:\cef\chromium_git
python C:\cef\automate\automate-git.py ^
  --download-dir=C:\cef\chromium_git ^
  --depot-tools-dir=C:\cef\depot_tools ^
  --branch=7103 ^
  --x64-build ^
  --minimal-distrib ^
  --client-distrib ^
  --no-debug-build ^
  --force-build

REM Exit code check
if %ERRORLEVEL% NEQ 0 (
  echo.
  echo BUILD FAILED with error code %ERRORLEVEL%
  echo Check output above for errors.
) else (
  echo.
  echo BUILD SUCCEEDED
  echo Output: C:\cef\chromium_git\chromium\src\cef\binary_distrib\
)

pause
```

**Key differences from the original guide (lessons from actual build):**

| Change | Why |
|--------|-----|
| Added `chrome_pgo_phase=0` to GN_DEFINES | Disables PGO profiling; avoids needing profile data files |
| Added `GYP_MSVS_OVERRIDE_PATH` and `vs2022_install` | Explicitly points to BuildTools edition of VS 2022 |
| Added Step 1 (`gclient sync --nohooks --no-history`) | Downloads dependencies (ninja, node, etc.) before automate-git.py |
| Added Step 1b (`gclient runhooks`) | Runs hooks separately for cleaner error handling |
| Uses `--force-build` instead of `--force-clean` | Forces rebuild but does NOT wipe the build dir (allows resume) |
| Removed `--proprietary-codecs` flag | Codecs are enabled via `GN_DEFINES` instead (more reliable) |
| Removed `--no-chromium-history` flag | Not needed with `--no-history` in gclient sync |
| Added `PATH` setup for depot_tools | Ensures depot_tools is available regardless of system PATH |

**Key flags explained:**

| Flag | Purpose |
|------|---------|
| `--branch=7103` | CEF 136 / Chromium 136 |
| `--x64-build` | 64-bit Windows build |
| `--minimal-distrib` | Smaller output (no debug symbols) |
| `--client-distrib` | Build cefclient for testing |
| `--no-debug-build` | Skip Debug build (faster, Release only) |
| `--force-build` | Force rebuild but keep existing objects (resumable) |

### Step 5: Run the Build

**Important**: This will take several hours!

```powershell
cd C:\cef\chromium_git

# Run as Administrator (recommended)
.\build_cef.bat
```

**What happens during the build:**

1. **Download phase** (1-3 hours):
   - Downloads Chromium source (~25 GB)
   - Downloads CEF source
   - Downloads dependencies

2. **Sync phase** (30-60 minutes):
   - Applies CEF patches to Chromium
   - Generates build files

3. **Compile phase** (2-4 hours):
   - Compiles Chromium
   - Compiles CEF
   - Links everything

4. **Package phase** (15-30 minutes):
   - Creates distribution packages

### Step 6: Find the Output

After successful build, find your binaries at:

```
C:\cef\chromium_git\chromium\src\cef\binary_distrib\
```

Look for a folder named something like:
```
cef_binary_136.1.7+g15882fe+chromium-136.0.7103.114_windows64_minimal/
```

Inside you'll find:
```
├── Debug/           (if built)
├── Release/
│   ├── libcef.dll          # Main CEF library (~239 MB with codecs)
│   ├── chrome_elf.dll      # Chrome helper
│   ├── d3dcompiler_47.dll  # DirectX shader compiler
│   ├── icudtl.dat          # Unicode data
│   ├── libEGL.dll          # OpenGL ES
│   ├── libGLESv2.dll       # OpenGL ES
│   ├── snapshot_blob.bin   # V8 snapshot
│   ├── v8_context_snapshot.bin
│   ├── resources/          # CEF resources
│   └── locales/            # Translations
├── Resources/
├── include/                # CEF headers
└── libcef_dll_wrapper/     # Wrapper source
```

---

## 6. Integrating with Hodos Browser

### Step 1: Backup Current Binaries

```powershell
cd C:\Users\archb\Hodos-Browser\cef-binaries

# Create backup
mkdir backup_prebuilt
Copy-Item -Path "Release\*" -Destination "backup_prebuilt\" -Recurse
Copy-Item -Path "Resources\*" -Destination "backup_prebuilt\" -Recurse
```

### Step 2: Copy New Binaries

```powershell
# Set paths
$CEF_BUILD = "C:\cef\chromium_git\chromium\src\cef\binary_distrib\cef_binary_136*_windows64"
$HODOS_CEF = "C:\Users\archb\Hodos-Browser\cef-binaries"

# Copy Release binaries
Copy-Item -Path "$CEF_BUILD\Release\*" -Destination "$HODOS_CEF\Release\" -Recurse -Force

# Copy Resources
Copy-Item -Path "$CEF_BUILD\Resources\*" -Destination "$HODOS_CEF\Resources\" -Recurse -Force

# Copy include headers
Copy-Item -Path "$CEF_BUILD\include\*" -Destination "$HODOS_CEF\include\" -Recurse -Force

# Copy wrapper source (for rebuilding)
Copy-Item -Path "$CEF_BUILD\libcef_dll_wrapper\*" -Destination "$HODOS_CEF\libcef_dll\wrapper\" -Recurse -Force
```

### Step 3: Rebuild libcef_dll_wrapper

The wrapper must be rebuilt when CEF headers change:

```powershell
cd C:\Users\archb\Hodos-Browser\cef-binaries\libcef_dll\wrapper

# Delete old CMakeCache (critical!)
Remove-Item -Path "build\CMakeCache.txt" -Force -ErrorAction SilentlyContinue
Remove-Item -Path "build" -Recurse -Force -ErrorAction SilentlyContinue

# Create fresh build directory
mkdir build
cd build

# Configure with CMake
cmake -G "Visual Studio 17 2022" -A x64 ..

# Build Release
cmake --build . --config Release

# Verify output exists
dir Release\libcef_dll_wrapper.lib
```

### Step 4: Rebuild Hodos Browser

```powershell
cd C:\Users\archb\Hodos-Browser\cef-native

# Clean old build
Remove-Item -Path "build" -Recurse -Force -ErrorAction SilentlyContinue

# Configure
mkdir build
cd build
cmake -G "Visual Studio 17 2022" -A x64 ..

# Build
cmake --build . --config Release

# Or open in Visual Studio
start cef.sln
```

### Step 5: Copy Built Binaries to Output

```powershell
# The build should automatically copy CEF binaries to output
# Verify they're present:
dir C:\Users\archb\Hodos-Browser\cef-native\build\bin\Release\libcef.dll
```

### Note: Widevine DRM Support

With proprietary codecs enabled, Widevine DRM is also available:

- **`enable_widevine=true`** is set automatically by CEF's build system — you do NOT need to add it manually.
- The actual Widevine CDM (`widevinecdm.dll`) is **NOT included** in the build output. It auto-downloads via Chromium's component updater at runtime (~5 minutes after first launch).
- Once the CDM downloads, DRM-protected content (Netflix, Disney+, Spotify) should work.
- **No license is needed** for the auto-download approach. The CDM is distributed by Google under their own license terms.

---

## 7. Verification & Testing

### Test 1: Codec Support Check

1. Launch Hodos Browser
2. Navigate to any page
3. Open DevTools (F12)
4. Run in Console:

```javascript
const v = document.createElement('video');
console.log('=== Codec Support ===');
console.log('H.264:', v.canPlayType('video/mp4; codecs="avc1.42E01E"'));
console.log('H.264 High:', v.canPlayType('video/mp4; codecs="avc1.64001E"'));
console.log('AAC:', v.canPlayType('audio/mp4; codecs="mp4a.40.2"'));
console.log('MP3:', v.canPlayType('audio/mpeg'));
console.log('VP9:', v.canPlayType('video/webm; codecs="vp9"'));
console.log('AV1:', v.canPlayType('video/webm; codecs="av01.0.01M.08"'));
```

**Expected output with codecs enabled:**
```
=== Codec Support ===
H.264: probably
H.264 High: probably
AAC: probably
MP3: probably
VP9: probably
AV1: probably
```

### Test 2: Real-World Video Playback

| Site | Test | Expected Result |
|------|------|-----------------|
| **x.com** | Open a tweet with video | Video plays |
| **x.com** | Open a tweet with GIF | Animated GIF plays (it's actually MP4) |
| **Reddit** | Open a post with video | Video plays with controls |
| **YouTube** | Any video | Should work (has VP9 fallback) |
| **Twitch** | Live stream | Stream plays |

### Test 3: Audio Playback

1. Go to a music streaming site or audio test page
2. Verify AAC audio plays
3. Verify MP3 audio plays

### Actual Test Results (2026-03-12)

| Site | Before (prebuilt, no codecs) | After (custom build) |
|------|------------------------------|----------------------|
| **x.com** | Videos broken, animated GIFs broken | Videos and animated GIFs now play |
| **Reddit** | Embedded videos broken | Embedded videos now play |
| **YouTube** | Worked (VP9/AV1 fallback) | Still works; now also has H.264 |
| **Audio** | AAC/MP3 not supported | AAC/MP3 working on music sites |

---

## 8. Lessons Learned (Real-World Build Notes)

These are hard-won lessons from the actual build on 2026-03-12.

### CRITICAL: Disable Windows Automatic Restarts

The build takes overnight. Windows Update **WILL** auto-restart your machine in the middle of the compile, killing the build process. Before starting:

1. **Pause Windows Update**: Settings > Windows Update > Pause updates for 1 week
2. **Set Active Hours**: Settings > Windows Update > Advanced options > Active hours > set to cover your build window (e.g., 6 PM to 12 PM next day)
3. **Disable automatic restart**: Group Policy (`gpedit.msc`) > Computer Configuration > Administrative Templates > Windows Components > Windows Update > "No auto-restart with logged on users"

Our build was interrupted at 78,821 objects by a Windows auto-restart. Fortunately, the build is resumable.

### The Build IS Resumable

If interrupted (power loss, crash, Windows restart), just re-run the build script. Ninja tracks completed work in `.ninja_log` and skips already-compiled objects. After our interruption, the resumed build only needed to compile ~17,336 additional objects instead of starting from scratch.

### Build Output Size

- **Build output name**: `cef_binary_136.1.7+g15882fe+chromium-136.0.7103.114_windows64_minimal`
- **libcef.dll**: 239 MB (vs 224 MB prebuilt — the 15 MB increase is the codec code)
- **Total disk usage for Release_GN_x64 build dir**: ~18 GB at interruption point
- **Total disk usage overall**: ~80-100 GB for the full source + build tree

### chrome_pgo_phase=0

Profile-Guided Optimization (PGO) requires pre-existing profiling data. Setting `chrome_pgo_phase=0` disables PGO, which avoids build failures from missing profile data. The performance difference is minimal for CEF usage.

---

## 9. Troubleshooting

### External Drive Considerations

Building on an external drive is viable but has tradeoffs:

| Drive Type | Impact | Recommended? |
|-----------|--------|-------------|
| **USB 3.0+ SSD** | ~1.5-2x slower than internal. Compile is CPU-bound so main slowdown is `gclient sync` (millions of small files). Workable. | Yes |
| **USB 3.0 HDD** | 2-3x slower. Random I/O on millions of small files is brutal on spinning disk. | Avoid if possible |
| **USB 2.0 anything** | Unworkable. | No |

**Tips for external drive builds:**
- Use NTFS (not exFAT) — Chromium uses symlinks and case-sensitive paths
- Connect to USB 3.0+ port (blue ports)
- Don't disconnect during build (corruption risk)
- Windows Defender real-time scanning will destroy performance on external drives — add an exclusion for `C:\cef\` (or `E:\cef\` etc.)

### Build Errors

#### "Python not found" or wrong version
```powershell
# Check Python version
python --version

# If wrong version, set path explicitly
set PATH=C:\Python311;%PATH%
```

#### "Visual Studio not found"
```powershell
# Verify VS 2022 installation
"C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\IDE\devenv.exe" /?

# Set version explicitly
set GYP_MSVS_VERSION=2022
```

#### "Failed to download VS toolchain" or "hash check failed"
This happens when `DEPOT_TOOLS_WIN_TOOLCHAIN` is not set to `0`. Without it, depot_tools tries to download Google's internal Windows toolchain (not public).
```powershell
set DEPOT_TOOLS_WIN_TOOLCHAIN=0
```
This tells the build system to use your locally installed Visual Studio 2022 instead.

#### "Debugging Tools for Windows not found"
- Reinstall Windows SDK with "Debugging Tools for Windows" checked
- See Section 4.2

#### Path too long errors
- Use shorter base path (e.g., `C:\cef\` not `C:\Users\username\Documents\CEF-Build\`)
- Enable long paths in Windows:
  ```powershell
  # Run as Administrator
  New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" -Name "LongPathsEnabled" -Value 1 -PropertyType DWORD -Force
  ```

#### Out of disk space
- Need ~100GB free minimum
- Clean previous build: `--force-clean` flag
- Remove `--no-chromium-history` only on subsequent builds

#### Build hangs or crashes
- Check RAM usage (need 16GB+, 32GB recommended)
- Close other applications during build
- Try reducing parallel jobs: add `--build-args="--jobs=4"` to limit CPU cores

### Integration Errors

#### "Unsupported CEF version"
- Wrapper must be rebuilt with matching headers
- Delete CMakeCache.txt and rebuild wrapper

#### libcef.dll missing
- Copy all files from CEF Release/ to output directory
- Check CMakeLists.txt copies DLLs correctly

#### Browser crashes on startup
- Check all CEF DLLs are present (libcef.dll, chrome_elf.dll, etc.)
- Check resources are copied (icudtl.dat, v8_context_snapshot.bin, locales/)
- Verify wrapper was built with same CEF version

---

## 10. Maintenance & Updates

### When to Rebuild

| Trigger | Action |
|---------|--------|
| CEF security update | Rebuild with new branch |
| Chromium security patch | Rebuild with new branch |
| New CEF release | Evaluate, rebuild if needed |
| Quarterly | Check for updates |

### Incremental Builds

After first successful build, remove `--force-clean` for faster rebuilds:

```batch
REM Incremental build (much faster)
python C:\cef\automate\automate-git.py ^
  --download-dir=C:\cef\chromium_git ^
  --depot-tools-dir=C:\cef\depot_tools ^
  --branch=7103 ^
  --x64-build ^
  --proprietary-codecs ^
  --minimal-distrib ^
  --client-distrib ^
  --no-debug-build
```

### Updating to New CEF Version

1. Check [CEF releases](https://cef-builds.spotifycdn.com/) for new versions
2. Find the branch number (e.g., 7103 for CEF 136)
3. Update `--branch=XXXX` in build script
4. Run build
5. Update Hodos Browser, rebuild wrapper

### CI/CD Setup (Future)

For automated builds, consider:
- GitHub Actions self-hosted runner (needs beefy machine)
- Dedicated build server
- Store artifacts in GitHub Releases or S3

---

## 11. macOS Build

macOS requires a **separate build** — Windows DLLs cannot be used on macOS. The macOS build produces `Chromium Embedded Framework.framework` instead of `libcef.dll`.

### macOS Build Script

A macOS build script is provided at `development-docs/build_hodos_cef_mac.sh`.

### Key Differences from Windows

| Aspect | Windows | macOS |
|--------|---------|-------|
| Output | `libcef.dll` (239 MB) | `Chromium Embedded Framework.framework` |
| Architecture | `--x64-build` | `--arm64-build` (Apple Silicon M1+) or `--x64-build` (Intel) |
| Toolchain | Visual Studio 2022 | Xcode + Command Line Tools |
| Build system | Same (ninja) | Same (ninja) |

### Architecture Selection

- **Apple Silicon (M1, M2, M3, M4)**: Use `--arm64-build`
- **Intel Mac**: Use `--x64-build`

### Requirements

Same disk space (~100 GB) and RAM (16 GB min, 32 GB recommended) requirements as Windows. macOS needs Xcode and its Command Line Tools installed.

---

## 12. Appendix

### A. All automate-git.py Flags

```
--download-dir      Where to download source
--depot-tools-dir   Path to depot_tools
--branch            CEF branch number (e.g., 7103)
--x64-build         Build 64-bit (Windows)
--arm64-build       Build ARM64 (Windows/Mac)
--proprietary-codecs Enable H.264/AAC/MP3
--minimal-distrib   Smaller distribution package
--client-distrib    Include cefclient test app
--no-debug-build    Skip Debug configuration
--force-clean       Clean rebuild (slow but thorough)
--no-chromium-history Remove git history (saves disk)
--with-pgo-profiles  Use profile-guided optimization
--build-args        Additional GN arguments
```

### B. CEF Version/Branch Mapping

| CEF Version | Chromium Version | Branch |
|-------------|------------------|--------|
| CEF 136 | Chromium 136 | 7103 |
| CEF 127 | Chromium 127 | 6533 |
| CEF 120 | Chromium 120 | 6167 |

Check current versions: https://cef-builds.spotifycdn.com/

### C. File Checklist

Files that MUST be present in output directory:

```
□ libcef.dll
□ chrome_elf.dll
□ d3dcompiler_47.dll
□ icudtl.dat
□ libEGL.dll
□ libGLESv2.dll
□ snapshot_blob.bin
□ v8_context_snapshot.bin
□ vk_swiftshader.dll
□ vk_swiftshader_icd.json
□ vulkan-1.dll
□ resources/
□   cef.pak
□   cef_100_percent.pak
□   cef_200_percent.pak
□   cef_extensions.pak
□   devtools_resources.pak
□ locales/
□   en-US.pak
□   (other locales...)
```

### D. Licensing Note

Building with proprietary codecs means distributing software that uses patented technology. 

**Under 100,000 installations**: Typically free under MPEG-LA/Via Licensing terms.

**Over 100,000 installations**: Royalties may apply (~$0.10-0.20 per unit with caps).

**Recommendation**: 
- Add MPEG-LA attribution to About page
- If Hodos grows significantly, budget for licensing
- Consult legal counsel if concerned

### E. Disk Space Cleanup

After successful build, you can recover space:

```powershell
# Remove Chromium git history (saves ~10GB, can't do incremental builds after)
# Only do this if you're sure the build works!

# Remove intermediate build files (keeps binaries)
# Be careful - this breaks incremental builds
Remove-Item -Path "C:\cef\chromium_git\chromium\src\out" -Recurse -Force
```

---

## Quick Reference Card

```
┌─────────────────────────────────────────────────────────────┐
│                 CEF BUILD QUICK REFERENCE                   │
├─────────────────────────────────────────────────────────────┤
│ Base path:        C:\cef\                                   │
│ depot_tools:      C:\cef\depot_tools\                       │
│ Source:           C:\cef\chromium_git\                      │
│ Output:           chromium_git\chromium\src\cef\binary_distrib\ │
├─────────────────────────────────────────────────────────────┤
│ CEF 136 branch:   7103                                      │
│ Build time:       ~10-12 hours (first), 30-60 min (incremental)│
│ Disk needed:      100GB minimum                             │
├─────────────────────────────────────────────────────────────┤
│ Key GN_DEFINES:   proprietary_codecs=true ffmpeg_branding=Chrome│
│ Key flags:        --branch=7103 --x64-build --no-debug-build│
│                   --force-build (resumable)                 │
├─────────────────────────────────────────────────────────────┤
│ Verify codecs:    video.canPlayType('video/mp4; ...')       │
│                   Should return "probably" not ""           │
└─────────────────────────────────────────────────────────────┘
```

---

*Document created 2026-03-01. Updated 2026-03-12 with actual build results and lessons learned. Update this guide when CEF versions change or build process evolves.*
