# CEF Binaries — Custom Build with Proprietary Codecs

## Source
These are **custom-built** CEF binaries, NOT the standard prebuilt binaries from Spotify CDN.

Built from source on 2026-03-12 using `automate-git.py`.

## Version
- **CEF**: 136.1.7+g15882fe
- **Chromium**: 136.0.7103.114
- **Branch**: 7103
- **Platform**: Windows x64
- **Build type**: Release only (no Debug)

## Key Feature: Proprietary Codecs Enabled
Build flags:
```
proprietary_codecs=true
ffmpeg_branding=Chrome
is_official_build=true
```

This enables H.264, AAC, MP3 decoding that is missing from prebuilt CEF.

## Widevine DRM
- `enable_widevine=true` is set automatically (all CEF builds have this)
- The actual Widevine CDM (`widevinecdm.dll`) auto-downloads at runtime via Chromium component updater
- No extra DLLs or license needed

## Rebuilding the Wrapper
After replacing binaries, the wrapper MUST be rebuilt:
```powershell
cd cef-binaries\libcef_dll\wrapper\build
cmake -G "Visual Studio 17 2022" -A x64 ..\..\..
cmake --build . --config Release --target libcef_dll_wrapper
# Copy output to expected location:
copy libcef_dll_wrapper\Release\libcef_dll_wrapper.lib Release\
```

Then rebuild cef-native:
```powershell
cd cef-native
set VCPKG_ROOT=C:/Users/archb/Dev/vcpkg
cmake -S . -B build -G "Visual Studio 17 2022" -A x64 -DCMAKE_TOOLCHAIN_FILE=%VCPKG_ROOT%/scripts/buildsystems/vcpkg.cmake
cmake --build build --config Release
```

## File Sizes (Reference)
| File | Size | Notes |
|------|------|-------|
| libcef.dll | 239 MB | +15 MB vs prebuilt (codec code) |
| libcef_dll_wrapper.lib | 93 MB | Must match libcef.dll version |
| cef_sandbox.lib | 86 MB | |

## Build Source
Full build instructions: `development-docs/CEF_BUILD_FROM_SOURCE_GUIDE.md`
Build script (Windows): `development-docs/build_hodos_cef.bat`
Build script (macOS): `development-docs/build_hodos_cef_mac.sh`
Build output location: `C:\cef\chromium_git\chromium\src\cef\binary_distrib\`

## macOS
These binaries are Windows-only. macOS requires a separate build producing `Chromium Embedded Framework.framework`. See build guide for macOS instructions.
