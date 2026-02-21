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
6. **Browser data is separate from wallet data** — history, bookmarks, and cookies live in CEF layer (`%APPDATA%/HodosBrowser/Default/`), not in the Rust wallet

## Window & Process Architecture

Every CEF browser instance runs in its **own renderer process**. The browser process (UI thread) orchestrates them via IPC.

### HWND Hierarchy (Windows)

```
g_hwnd  (main shell — WS_OVERLAPPEDWINDOW)
  ├── g_header_hwnd       WINDOWED CEF browser, role "header"
  │     React UI: tab bar, toolbar, address bar, find bar
  │     Native keyboard input (SetAsChild rendering)
  │
  ├── Tab HWNDs           WINDOWED CEF browsers, role "tab_N"
  │     One HWND per tab, only active tab is WS_VISIBLE
  │     Created by TabManager::CreateTab(), parented to g_hwnd
  │
  └── g_webview_hwnd      LEGACY — hidden, unused. Kept for API compat.

Overlay HWNDs  (WS_POPUP, owned by g_hwnd, NOT children)
  ├── g_settings_overlay_hwnd         OSR browser, role "settings"
  ├── g_wallet_overlay_hwnd           OSR browser, role "wallet"
  ├── g_download_panel_overlay_hwnd   OSR browser, role "downloadpanel"
  ├── g_omnibox_overlay_hwnd          OSR browser, role "omnibox"
  └── ... (6 more overlays)           all OSR browsers
```

### Rendering Modes

| Type | Rendering | Keyboard Input | Use For |
|------|-----------|---------------|---------|
| Header + Tabs | **Windowed** (`SetAsChild`) | Native (OS handles it) | Content that needs reliable text input |
| All Overlays | **OSR** (off-screen) | Manual WndProc forwarding (`WM_KEYDOWN`/`WM_CHAR` → `SendKeyEvent`) | Popups, panels, dropdowns |

**Key rule**: Windowed browsers get keyboard input for free. OSR browsers require manual keyboard forwarding in their WndProc — this is fragile and was a source of bugs (notification overlay keyboard fix). Prefer windowed rendering for anything with text input.

### Focus Management

- For **windowed** browsers: use `browser->GetHost()->SetFocus(true)` (NOT `SetFocus(hwnd)` — CEF creates internal child windows)
- For **OSR** browsers: use `browser->GetHost()->SetFocus(true)` + ensure WndProc forwards key events via `SendKeyEvent`

### IPC Flow

```
React (renderer process A) --cefMessage.send()--> SimpleHandler::OnProcessMessageReceived (browser process)
Browser process --SendProcessMessage(PID_RENDERER)--> SimpleRenderProcessHandler::OnProcessMessageReceived (renderer process B)
```

Cross-browser communication (e.g. header find bar → tab search) always routes through the browser process. There is no direct renderer-to-renderer IPC.

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
| `cef_browser_shell.cpp` | `g_hwnd`, `g_header_hwnd`, `g_webview_hwnd`, overlay HWNDs (incl. `g_download_panel_overlay_hwnd`), `Logger`, `WndProc`, `DownloadPanelOverlayWndProc`, `DownloadPanelMouseHookProc` |
| `src/handlers/simple_render_process_handler.cpp` | `CefMessageSendHandler`, `escapeJsonForJs`, `OnContextCreated` |
| `src/handlers/simple_handler.cpp` | `OnProcessMessageReceived`, `OnAfterCreated`, `OnBeforeClose`, `CefDownloadHandler` (`CanDownload`, `OnBeforeDownload`, `OnDownloadUpdated`), `DownloadInfo` struct, `active_downloads_` map, `NotifyDownloadStateChanged`, `CefFindHandler` (`OnFindResult`), find IPC (`find_text`, `find_stop`) |
| `src/handlers/simple_app.cpp` | `CreateDownloadPanelOverlay`, `ShowDownloadPanelOverlay`, `HideDownloadPanelOverlay` (overlay lifecycle) |
| `src/core/HttpRequestInterceptor.cpp` | `DomainVerifier`, `AsyncWalletResourceHandler`, `g_pendingAuthRequest`, `isWalletEndpoint` |
| `src/core/BRC100Bridge.cpp` | `makeHttpRequest` (WinHTTP to localhost:3301) |
| `src/core/HistoryManager.cpp` | Browser history SQLite database; singleton with `AddVisit`, `GetHistory`, `SearchHistory`, `DeleteHistoryEntry` |
