# CEF Native Shell Layer

## Responsibility

C++ browser shell using Chromium Embedded Framework. Provides process isolation security boundaries between web content and wallet operations. Manages windows, V8 JavaScript injection, HTTP request interception, and IPC between render/browser processes. Routes wallet API calls to Rust backend without accessing private keys.

## Build (Windows)

Requires: VS 2022, vcpkg with OpenSSL + nlohmann-json, CEF binaries in `../cef-binaries/`

```powershell
cd cef-native

# Configure (first time or after CMakeLists.txt changes)
cmake -S . -B build -G "Visual Studio 17 2022" -A x64 `
  -DCMAKE_TOOLCHAIN_FILE=[vcpkg_root]/scripts/buildsystems/vcpkg.cmake

# Build
cmake --build build --config Release

# Output: build/bin/Release/HodosBrowserShell.exe
```

## Run

Rust wallet and frontend dev server must be running first.

```powershell
cd cef-native/build/bin/Release
./HodosBrowserShell.exe
```

## Invariants

1. **This layer is a security boundary** — it forwards requests to Rust but never accesses or stores private keys
2. **Do not change CEF lifecycle/threading** — message loop, browser creation timing, render-process handlers are fragile
3. **Do not modify `CefMessageSendHandler`** without understanding IPC flow
4. **Do not change HTTP interception routing** without asking — affects all wallet API calls
5. **Process-per-overlay architecture is intentional** — each overlay subprocess provides V8 context isolation for defense in depth

## Entry Points

| File | Purpose |
|------|---------|
| `cef_browser_shell.cpp` | `main()`, window creation, overlay HWNDs (`g_hwnd`, `g_settings_overlay_hwnd`, etc.) |
| `src/handlers/simple_app.cpp` | CEF app initialization, spawns render process handler |
| `src/handlers/simple_handler.cpp` | Browser process message routing, overlay management |
| `src/handlers/simple_render_process_handler.cpp` | V8 injection: `CefMessageSendHandler`, injects `window.hodosBrowser` |

## Extension Points

| To Add | Where |
|--------|-------|
| New V8 API method | `simple_render_process_handler.cpp` in `OnContextCreated()` |
| New IPC message handler | `simple_handler.cpp` in `OnProcessMessageReceived()` |
| New wallet endpoint interception | `HttpRequestInterceptor.cpp` in `isWalletEndpoint()` |
| New overlay window | `cef_browser_shell.cpp`, add HWND global, create in `WndProc` |

## Key Files

| File | Identifiers |
|------|-------------|
| `cef_browser_shell.cpp` | `g_hwnd`, `g_header_hwnd`, `g_webview_hwnd`, overlay HWNDs, `Logger`, `WndProc` |
| `src/handlers/simple_render_process_handler.cpp` | `CefMessageSendHandler`, `escapeJsonForJs`, `OnContextCreated` |
| `src/handlers/simple_handler.cpp` | `OnProcessMessageReceived`, `OnAfterCreated`, `OnBeforeClose` |
| `src/core/HttpRequestInterceptor.cpp` | `DomainVerifier`, `AsyncWalletResourceHandler`, `g_pendingAuthRequest`, `isWalletEndpoint` |
| `src/core/BRC100Bridge.cpp` | `makeHttpRequest` (WinHTTP to localhost:3301) |
