# CEF Native Shell Layer

> Last Updated: 2026-08-03

## Responsibility

C++ browser shell using Chromium Embedded Framework. Provides process isolation security boundaries between web content and wallet operations. Manages windows, V8 JavaScript injection, HTTP request interception, and IPC between render/browser processes. Routes wallet API calls to Rust backend without accessing private keys.

## CEF / Chromium Version Pin

This directory is the owning doc for the engine pin. Read from `cef-binaries/include/cef_version.h`:

| Define | Value |
|--------|-------|
| `CEF_VERSION` | `136.1.7+g15882fe+chromium-136.0.7103.114` |
| `CEF_VERSION_MAJOR` / `MINOR` / `PATCH` | 136 / 1 / 7 |
| `CEF_COMMIT_NUMBER` | 3177 |
| `CEF_COMMIT_HASH` | `15882fefc4339cb456a2654a2055d75e6467ccf1` |
| `CHROME_VERSION` | 136.0.7103.114 |

CMake points at `CEF_ROOT = ../cef-binaries`. On Windows it links `${CEF_ROOT}/Release` (libcef, cef_sandbox); on macOS it links the `Chromium Embedded Framework.framework` out of the same `Release/` dir. A CEF bump means changing the contents of `cef-binaries/`, not this CMakeLists — but re-verify the wrapper build (below) after any bump.

## Build (Windows)

Requires: VS 2022, vcpkg (triplet `x64-windows-static`), CEF binaries in `../cef-binaries/`.

`CMakeLists.txt` resolves **three** dependencies on Windows:

| Package | CMake call | Linked as |
|---------|-----------|-----------|
| OpenSSL | `find_package(OpenSSL REQUIRED)` (static: `OPENSSL_USE_STATIC_LIBS TRUE`) | `OpenSSL::SSL`, `OpenSSL::Crypto` |
| nlohmann-json | `find_package(nlohmann_json CONFIG REQUIRED)` | `nlohmann_json::nlohmann_json` |
| sqlite3 | `find_package(unofficial-sqlite3 CONFIG REQUIRED)` | `unofficial::sqlite3::sqlite3` |

On macOS the same three come from Homebrew instead (`find_path` for `nlohmann/json.hpp`, `find_library` for `sqlite3`, `OPENSSL_ROOT_DIR` probed under `/opt/homebrew` then `/usr/local`).

Also linked on Windows: `WinSparkle` (from `../external/winsparkle/WinSparkle-0.8.1`), the vendored `quirc` QR decoder (`third_party/quirc/`, built as its own static lib), and a long list of Win32 system libs (`winhttp`, `dwmapi`, `windowscodecs`, `dbghelp`, …).

### Step 0 — build the CEF wrapper first (required, easy to forget)

The shell links `libcef_dll_wrapper.lib` from a path CMake does **not** build for you: `${CEF_ROOT}/libcef_dll/wrapper/build/Release`. If that .lib is missing, configure succeeds and the link fails.

```powershell
cd cef-binaries/libcef_dll/wrapper
mkdir build; cd build
cmake ..
cmake --build . --config Release
# produces: cef-binaries/libcef_dll/wrapper/build/Release/libcef_dll_wrapper.lib
```

On macOS the wrapper is built via the top-level CEF CMakeLists instead, and is expected at `cef-binaries/build/libcef_dll_wrapper/`.

### Step 1 — build the shell

```powershell
cd cef-native

# Configure (first time or after CMakeLists.txt changes)
cmake -S . -B build -G "Visual Studio 17 2022" -A x64 `
  -DCMAKE_TOOLCHAIN_FILE=[vcpkg_root]/scripts/buildsystems/vcpkg.cmake

# Build
cmake --build build --config Release
```

### Build outputs

| Artifact | Path | Notes |
|----------|------|-------|
| Browser shell | `build/bin/Release/HodosBrowser.exe` | CMake **target** is `HodosBrowserShell`; `OUTPUT_NAME` is `HodosBrowser`. The shipped image name is `HodosBrowser.exe` — there is no `HodosBrowserShell.exe`. |
| Solution file | `build/HodosBrowserShell.sln` | Named after `project()`, not the exe |
| Update helper | `build/bin/Release/hodos-update-helper.exe` | Separate Win32 target, not linked against CEF (`update-helper/main.cpp` + `transaction.cpp` + shared `UpdateApply.cpp`/`UpdateFs.cpp`) |
| Unit tests | `build/tests/Release/hodos_tests.exe` | Only with `-DHODOS_BUILD_TESTS=ON` (default OFF) |

macOS produces an app bundle also named `HodosBrowser` (`OUTPUT_NAME "HodosBrowser"`, `MACOSX_BUNDLE`), plus **5** CEF helper bundles generated from `CEF_HELPER_APP_SUFFIXES` (base, Alerts, GPU, Plugin, Renderer).

### Build options

| Option | Default | Effect |
|--------|---------|--------|
| `APP_VERSION` | `0.2.0-dev` | Compiled in as `-DAPP_VERSION=` |
| `APP_BUILD_NUMBER` | derived from `APP_VERSION` (`MAJOR*1000000 + MINOR*10000 + PATCH*100 + betaN\|99`) | Monotonic integer used by Sparkle + the Windows anti-rollback gate. Configure fails if not a positive integer. |
| `HODOS_SILENT_AUTOUPDATE` | OFF | Compiles the Windows silent download-while-running staging thread |
| `HODOS_UPDATE_TEST_SEAM` | OFF | **Rig only — never ship.** Relaxes update signature/installer verification |
| `HODOS_BUILD_TESTS` | OFF | Adds `tests/` (googletest via FetchContent) |

## Run

Rust wallet and frontend dev server must be running first.

**To run the dev build**, use a launcher script (sets `HODOS_DEV=1` automatically):
```powershell
cd cef-native
.\win_build_run.ps1      # Windows (PowerShell) — builds + launches
./win_build_run.sh       # Windows (Git Bash / MSYS2) — same flow
./mac_build_run.sh       # Mac — builds + launches
```

Both Windows launchers kill only the DEV instance, matched by **executable path** under `build/bin/Release`, never by bare image name — dev and installed-prod ship the same image name `HodosBrowser.exe`, so a name-based kill would take down the user's installed browser.

**⚠️ NEVER launch the exe directly from the build directory** — the dev safeguard will block it. Dev builds detect they are running from `build/bin/Release/` and refuse to start without `HODOS_DEV=1` to prevent hitting the production database (`cef_browser_shell.cpp :: WinMain`, `cef_browser_shell_mac.mm :: main`, backed by `include/core/AppPaths.h`). The reverse gate also exists: a stray `HODOS_DEV=1` on an installed/portable binary is scrubbed from the environment rather than honored.

### Backend ports

`include/core/PortConfig.h` is the single source of truth. Never hardcode a port.

| Backend | Release | `HODOS_DEV=1` | Helper |
|---------|---------|---------------|--------|
| Rust wallet | `127.0.0.1:31301` | `127.0.0.1:31401` | `hodos::WalletPort()`, `hodos::WalletUrl(path)`, `hodos::IsWalletHostPort(url)` |
| Adblock engine | `127.0.0.1:31302` | `127.0.0.1:31402` | `hodos::AdblockPort()`, `hodos::AdblockUrl(path)` |

`IsWalletHostPort()` checks **both** `localhost:<port>` and `127.0.0.1:<port>` — the codebase uses both host forms and they must move in lockstep.

## Invariants

1. **This layer is a security boundary** — it forwards requests to Rust but never accesses or stores private keys
2. **Do not change CEF lifecycle/threading** — message loop, browser creation timing, render-process handlers are fragile
3. **Do not modify `CefMessageSendHandler`** without understanding IPC flow
4. **Do not change HTTP interception routing** without asking — affects all wallet API calls
5. **Process-per-overlay architecture is intentional** — each overlay subprocess provides V8 context isolation for defense in depth
6. **Browser data is separate from wallet data** — history, bookmarks, and cookies live in CEF layer (`%APPDATA%/HodosBrowser/Default/`, `HodosBrowserDev/` under `HODOS_DEV=1`), not in the Rust wallet
7. **Permission decisions are NOT made here.** The decision engine lives in Rust (`rust-wallet/crates/hodos_permission_engine`, wrapped by `rust-wallet/src/permission_service/`, wired as Actix middleware). The old C++ `PermissionEngine` and `SessionManager` were deleted in Phase 2.6-H — do not reintroduce a C++ decision path. This layer only *collects context, shows prompts, and relays answers*.
8. **Never hardcode a backend port** — route through `PortConfig.h` (see above)

## Logging

`Logger` is declared in `include/core/Logger.h` and implemented in `src/core/Logger.cpp` (static members + `Initialize`/`Log`/`Shutdown`/`IsInitialized`).

**Log file location:** `%APPDATA%\HodosBrowser\logs\debug_output.log` (`HodosBrowserDev\logs\` under `HODOS_DEV=1`), resolved via `AppPaths::GetLogDir()`. It is deliberately **outside** the install root: the browser holds the log open for writing, and a log inside `{app}` broke the silent-update backup hash of the `{app}` tree. It falls back to a relative `debug_output.log` only if `APPDATA` is unavailable. `stdout`/`stderr` are `freopen`'d to the same file.

Use the `Logger` macros — **never `std::cout` or `printf` directly** (stdout is redirected to the same file anyway).

```cpp
// Levels map to LogLevel: 0=DEBUG, 1=INFO, 2=WARNING, 3=ERROR_LEVEL
// Process codes map to ProcessType: 0=MAIN, 1=RENDER, 2=BROWSER
#define LOG_INFO(msg)          Logger::Log(msg, 1, 0)   // MAIN
#define LOG_INFO_RENDER(msg)   Logger::Log(msg, 1, 1)   // RENDER
#define LOG_INFO_BROWSER(msg)  Logger::Log(msg, 1, 2)   // BROWSER
```

Output format: `[timestamp] [SOURCE] [LEVEL] message`

There is no central macro header. **Each `.cpp` that logs `#define`s its own macro family locally**, using a subsystem suffix — the suffix is purely a naming convention for the author's benefit; the only thing that reaches `Logger` is the (level, process) pair. Current suffixes in use across the tree: `_APP`, `_BLOCK`, `_BM`, `_BROWSER`, `_CM`, `_COOKIE`, `_EPHEMERAL`, `_GOOGLE`, `_HISTORY`, `_HTTP`, `_PCC`, `_PI`, `_QR`, `_RENDER`, `_SM`, `_SP`, `_SSW`, `_SYNC`, `_TP`, `_UPD`, `_WALLET`, `_WM`, plus unsuffixed `LOG_INFO`/`LOG_DEBUG`/`LOG_WARNING`/`LOG_ERROR`.

Use `LOG_INFO_*` for things you want to see during normal testing. Use `LOG_DEBUG_*` for high-frequency noise (e.g. every IPC message).

## Window & Process Architecture

Every CEF browser instance runs in its **own renderer process**. The browser process (UI thread) orchestrates them via IPC.

### HWND Hierarchy (Windows)

All HWND globals are declared at the top of `cef_browser_shell.cpp` (globals block, immediately after the `Logger` macros).

```
g_hwnd  (main shell — WS_OVERLAPPEDWINDOW, WndProc = ShellWindowProc)
  ├── g_header_hwnd       WINDOWED CEF browser, role "header"
  │     React UI: tab bar, toolbar, address bar, find bar
  │     Native keyboard input (SetAsChild rendering)
  │
  ├── Tab HWNDs           WINDOWED CEF browsers, role "tab_<id>"
  │     One HWND per tab, only active tab is WS_VISIBLE
  │     Created by TabManager::CreateTab(), parented to g_hwnd
  │     Role string built in TabManager.cpp ("tab_" << tab_id)
  │
  └── g_webview_hwnd      LEGACY — set to nullptr with a "legacy, unused"
                          comment. Kept for API compat + teardown.
```

**14 overlay HWNDs** (WS_POPUP, owned by `g_hwnd`, NOT children; all OSR browsers):

| HWND global | Handler role | WndProc | Click-outside mouse hook |
|-------------|--------------|---------|--------------------------|
| `g_settings_overlay_hwnd` | `settings` | `SettingsOverlayWndProc` | `SettingsPanelMouseHookProc` |
| `g_wallet_overlay_hwnd` | `wallet` | `WalletOverlayWndProc` | — (WM_ACTIVATE path) |
| `g_backup_overlay_hwnd` | `backup` | `BackupOverlayWndProc` | — |
| `g_brc100_auth_overlay_hwnd` | `brc100auth` | `BRC100AuthOverlayWndProc` | — |
| `g_notification_overlay_hwnd` | `notification` | `NotificationOverlayWndProc` | — |
| `g_settings_menu_overlay_hwnd` | `settings_menu` | `SettingsMenuOverlayWndProc` | — |
| `g_omnibox_overlay_hwnd` | `omnibox` | `OmniboxOverlayWndProc` | `OmniboxMouseHookProc` |
| `g_cookie_panel_overlay_hwnd` | `cookiepanel` | `CookiePanelOverlayWndProc` | `CookiePanelMouseHookProc` |
| `g_download_panel_overlay_hwnd` | `downloadpanel` | `DownloadPanelOverlayWndProc` | `DownloadPanelMouseHookProc` |
| `g_bookmarks_panel_overlay_hwnd` | `bookmarkspanel` | `BookmarksPanelOverlayWndProc` | `BookmarksPanelMouseHookProc` |
| `g_tablist_panel_overlay_hwnd` | `tablistpanel` | `TabListPanelOverlayWndProc` | `TabListPanelMouseHookProc` |
| `g_siteinfo_panel_overlay_hwnd` | `siteinfopanel` | `SiteInfoPanelOverlayWndProc` | `SiteInfoPanelMouseHookProc` |
| `g_profile_panel_overlay_hwnd` | `profilepanel` | `ProfilePanelOverlayWndProc` | `ProfilePanelMouseHookProc` |
| `g_menu_overlay_hwnd` | `menu` | `MenuOverlayWndProc` | `MenuMouseHookProc` |

That is 14 overlay HWNDs, 14 overlay WndProcs (+ `ShellWindowProc` for the main window) and **9** `WH_MOUSE_LL` click-outside hook procs, all in `cef_browser_shell.cpp`. Note the asymmetry: `BrowserWindow.h` declares **10** `HHOOK` slots — `wallet_mouse_hook` has a slot but no corresponding hook proc, because the wallet overlay closes via its `WM_ACTIVATE` path instead.

### Overlay creation functions

**Windows — 14, all in `src/handlers/simple_app.cpp`:**
`CreateSettingsOverlayWithSeparateProcess`, `CreateWalletOverlay`, `CreateBackupOverlayWithSeparateProcess`, `CreateBRC100AuthOverlayWithSeparateProcess`, `CreateNotificationOverlay`, `CreateSettingsMenuOverlay`, `CreateOmniboxOverlay`, `CreateCookiePanelOverlay`, `CreateDownloadPanelOverlay`, `CreateSiteInfoPanelOverlay`, `CreateTabListPanelOverlay`, `CreateBookmarksPanelOverlay`, `CreateMenuOverlay`, `CreateProfilePanelOverlay`.

Most also have a `Show…Overlay(offset, targetWin)` / `Hide…Overlay()` pair in the same file (wallet, omnibox, cookie, download, siteinfo, tablist, bookmarks, menu, profile). Settings / backup / brc100auth / notification / settings_menu are create-and-show only.

**macOS — 14, all in `cef_browser_shell_mac.mm`** (NSPanel-based, not `WS_POPUP`):
`CreateSettingsOverlayWithSeparateProcess`, `CreateWalletOverlayWithSeparateProcess`, `CreateBackupOverlayWithSeparateProcess`, `CreateBRC100AuthOverlayWithSeparateProcess`, `CreateNotificationOverlay`, `CreateSettingsMenuOverlay`, `CreateCookiePanelOverlayWithSeparateProcess`, `CreateOmniboxOverlayMacOS`, `CreateDownloadPanelOverlayMacOS`, `CreateProfilePanelOverlayMacOS`, `CreateBookmarksPanelOverlayMacOS`, `CreateSiteInfoPanelOverlayMacOS`, `CreateTabListPanelOverlayMacOS`, `CreateMenuOverlayMac` (plus a `CreateMenuOverlay(void*, bool, int)` shim matching the Windows signature).

Windows and macOS are at **parity: 14 overlays each**.

### Rendering Modes

| Type | Rendering | Keyboard Input | Use For |
|------|-----------|---------------|---------|
| Header + Tabs | **Windowed** (`SetAsChild`) | Native (OS handles it) | Content that needs reliable text input |
| All Overlays | **OSR** (off-screen) | Manual WndProc forwarding (`WM_KEYDOWN`/`WM_CHAR` → `SendKeyEvent`) | Popups, panels, dropdowns |

**Key rule**: Windowed browsers get keyboard input for free. OSR browsers require manual keyboard forwarding in their WndProc — this is fragile and was a source of bugs (notification overlay keyboard fix). Prefer windowed rendering for anything with text input.

### Focus Management

- For **windowed** browsers: use `browser->GetHost()->SetFocus(true)` (NOT `SetFocus(hwnd)` — CEF creates internal child windows)
- For **OSR** browsers: use `browser->GetHost()->SetFocus(true)` + ensure WndProc forwards key events via `SendKeyEvent`

### Browser Role Slots

`BrowserWindow` (`src/core/BrowserWindow.cpp :: SetBrowserForRole` / `GetBrowserForRole`) maps **18** role strings to `CefRefPtr<CefBrowser>` slots:

`header`, `webview`, `wallet_panel`, `overlay`, `settings`, `wallet`, `backup`, `brc100auth`, `notification`, `settings_menu`, `omnibox`, `cookiepanel`, `downloadpanel`, `profilepanel`, `menu`, `bookmarkspanel`, `siteinfopanel`, `tablistpanel`.

Tab browsers use the dynamic role `tab_<id>` and are **not** stored in these slots — `SimpleHandler` matches them by the `"tab_"` prefix (`simple_handler.cpp`, `role_.rfind("tab_", 0) == 0`).

### IPC Flow

```
React (renderer process A) --cefMessage.send()--> SimpleHandler::OnProcessMessageReceived (browser process)
Browser process --SendProcessMessage(PID_RENDERER)--> SimpleRenderProcessHandler::OnProcessMessageReceived (renderer process B)
```

Cross-browser communication (e.g. header find bar → tab search) always routes through the browser process. There is no direct renderer-to-renderer IPC.

## Entry Points

| File | Purpose |
|------|---------|
| `cef_browser_shell.cpp` | Windows entry `WinMain`; `ShellWindowProc`; all HWND globals; 14 overlay WndProcs + 9 mouse hooks; `Logger::Initialize` + stdout/stderr redirection; dev safeguard |
| `cef_browser_shell_mac.mm` | macOS entry `main`; NSWindow/NSView hierarchy; 14 overlay creation functions; event forwarding; multi-window support |
| `src/handlers/simple_app.cpp` | `SimpleApp` (`OnContextInitialized`, `OnBeforeChildProcessLaunch`, `OnBeforeCommandLineProcessing`, `SetWindowHandles`, `SetMacOSWindow`); `InjectHodosBrowserAPI`; all 14 Windows overlay create/show/hide functions |
| `src/handlers/simple_handler.cpp` | Browser-process message routing, overlay management, context menus, downloads, find-in-page |
| `src/handlers/simple_render_process_handler.cpp` | V8 injection: `CefMessageSendHandler` + 4 more V8 handlers, injects `window.hodosBrowser` |
| `mac/process_helper_mac.mm` | macOS helper-process entry (5 helper bundles built from `HODOS_HELPER_SRCS`) |
| `update-helper/main.cpp` | `hodos-update-helper.exe` entry (`wmain`, GUI subsystem via `/ENTRY:wmainCRTStartup`) |

`SimpleHandler` implements **12** CEF interfaces (`include/handlers/simple_handler.h`): `CefClient`, `CefLifeSpanHandler`, `CefDisplayHandler`, `CefLoadHandler`, `CefRequestHandler`, `CefContextMenuHandler`, `CefDialogHandler`, `CefKeyboardHandler`, `CefPermissionHandler`, `CefDownloadHandler`, `CefFindHandler`, `CefJSDialogHandler`.

## Extension Points

| To Add | Where |
|--------|-------|
| New V8 API method | `simple_render_process_handler.cpp` in `SimpleRenderProcessHandler::OnContextCreated()` (V8 handler classes live in the same file) |
| New IPC message handler | `simple_handler.cpp` in `OnProcessMessageReceived()` |
| New wallet endpoint interception | `HttpRequestInterceptor.cpp :: HttpRequestInterceptor::isWalletEndpoint()` — the route table; new endpoints go through it, never around it |
| New overlay window (Windows) | 1) HWND global + WndProc (+ mouse hook if dropdown-style) in `cef_browser_shell.cpp`; 2) `Create…Overlay` / `Show…` / `Hide…` trio in `src/handlers/simple_app.cpp`; 3) role slot in `src/core/BrowserWindow.cpp` |
| New overlay window (macOS) | Matching `Create…OverlayMacOS` in `cef_browser_shell_mac.mm` (NSPanel), same role slot |
| New C++ unit test | Add the `.cpp` to the explicit source list in `tests/CMakeLists.txt` (`hodos_tests` target) |

## Key Files

| File | Identifiers |
|------|-------------|
| `cef_browser_shell.cpp` | `WinMain`, `ShellWindowProc`, `g_hwnd`, `g_header_hwnd`, `g_webview_hwnd`, the 14 overlay HWNDs, 14 overlay WndProcs, 9 `…MouseHookProc` click-outside hooks, `Logger::Initialize` + log-path resolution, dev safeguard |
| `cef_browser_shell_mac.mm` | `main`, 14 macOS overlay creation functions, NSWindow/NSView hierarchy, `Logger::Initialize` |
| `src/handlers/simple_render_process_handler.cpp` | `SimpleRenderProcessHandler::OnContextCreated`, and 5 V8 handler classes: `CefMessageSendHandler`, `OverlayCloseHandler`, `OmniboxCloseHandler`, `HistoryV8Handler`, `GoogleSuggestV8Handler` |
| `include/core/JsStringEscape.h` | `escapeJsonForJs` — the canonical JS-string-literal encoder (header-only; moved out of `simple_render_process_handler.cpp`, which now `#include`s it) |
| `src/handlers/simple_handler.cpp` | `OnProcessMessageReceived`, `OnAfterCreated`, `OnBeforeClose`, `GetResourceRequestHandler`, `CefDownloadHandler` (`CanDownload`, `OnBeforeDownload`, `OnDownloadUpdated`), `DownloadInfo` struct, `active_downloads_` map, `NotifyDownloadStateChanged`, `CefFindHandler::OnFindResult`, find IPC (`find_text`, `find_stop`), helpers `CreateNewTabWithUrl()` / `CopyTextToClipboard()` |
| `src/handlers/simple_app.cpp` | `SimpleApp::OnContextInitialized`, `InjectHodosBrowserAPI`, and the 14 `Create…Overlay` functions (+ their `Show…`/`Hide…` pairs) |
| `src/core/HttpRequestInterceptor.cpp` | `HttpRequestInterceptor::isWalletEndpoint`, `DomainPermissionCache`, `WalletStatusCache`, `BSVPriceCache`, `AsyncWalletResourceHandler`, `AsyncHTTPClient`, `Async402ResourceHandler` + `Async402HTTPClient`, free functions `TryHandleBrc121_402` / `InstallAsync402HandlerIfPending`, structs `PaidRetryContext`, `PendingEnvelope`, `PendingReload`, `Brc121FailedEntry`, `CertDisclosureInfo`, `ProtocolScope`, `BasketScope`. **`DomainVerifier` was removed** — replaced by the DB-backed `DomainPermissionCache`. |
| `include/core/PortConfig.h` | `hodos::IsDevEnv`, `WalletPort`, `AdblockPort`, `WalletUrl`, `AdblockUrl`, `IsWalletHostPort` — the only sanctioned source of backend ports |
| `include/core/AppPaths.h` | `GetAppDirName()` (dev/prod namespace), `GetLogDir()`, `GetInstanceMutexNameW()`, dev/prod safeguard logic |
| `include/core/Logger.h` + `src/core/Logger.cpp` | `Logger`, `LogLevel` (DEBUG/INFO/WARNING/ERROR_LEVEL), `ProcessType` (MAIN/RENDER/BROWSER) |
| `src/core/HistoryManager.cpp` | Browser history SQLite database; singleton with `Initialize`, `AddVisit`, `GetHistory`, `GetHistorySimple`, `SearchHistory`, `SearchHistoryWithFrecency`, `GetTopSites`, `DeleteHistoryEntry`, `DeleteAllHistory`, `DeleteHistoryRange`, Chromium-time converters |
| `src/core/BrowserWindow.cpp` | `SetBrowserForRole` / `GetBrowserForRole` / `ClearBrowserForRole` — 18 role slots |

> **Removed:** `src/core/BRC100Bridge.cpp` no longer exists (only a stale `.obj` lingers in `build/`). Outbound HTTP to the Rust wallet now goes through `SyncHttpClient` (`include/core/SyncHttpClient.h`, WinHTTP / libcurl), `WalletService`, and the async handlers inside `HttpRequestInterceptor.cpp`.

## Directory Inventory

| Path | Contents |
|------|----------|
| `src/core/` | 35 implementation files: 32 `.cpp` + 3 `.mm` (`AutoUpdater_mac.mm`, `TabManager_mac.mm`, `WindowManager_mac.mm` — note `WalletService_mac.cpp` is a `.cpp`, not a `.mm`). Also `DefaultTrackerList.h` and `src/core/CLAUDE.md`. Not all 35 are in every build: `CMakeLists.txt` splits them into a cross-platform `SOURCES` list plus `if(APPLE)` / `elseif(WIN32)` appends. |
| `src/handlers/` | `simple_app.cpp`, `simple_handler.cpp`, `simple_handler_mac.mm`, `simple_render_process_handler.cpp`, `my_overlay_render_handler.cpp` / `.mm`, plus `src/handlers/CLAUDE.md` |
| `include/core/` | 47 headers (see `include/core/CLAUDE.md`) |
| `include/handlers/` | `simple_app.h`, `simple_handler.h`, `simple_render_process_handler.h`, `my_overlay_render_handler.h`, plus `include/handlers/CLAUDE.md` |
| `include/platform/` | `platform_window.h` |
| `update-helper/` | `main.cpp`, `transaction.cpp`, `transaction.h`, `splash.h` |
| `third_party/quirc/` | Vendored QR decoder (ISC), built as the `quirc` static lib on Windows only |
| `mac/` | macOS helper entry (`process_helper_mac.mm`), `helper-Info.plist.in`, icon assets + `generate-icon.sh` |
| `tests/` | 8 gtest files: `manifest_fetcher_test.cpp`, `sensitive_cert_fields_test.cpp`, `js_string_escape_test.cpp`, `profile_id_test.cpp`, `update_stager_test.cpp`, `update_apply_test.cpp`, `update_fs_test.cpp`, `silent_state_writer_test.cpp`. (Phase 2.6-H deleted `permission_engine_test.cpp` and `permission_gate_test.cpp` along with the C++ permission engine.) |

## Sub-directory Docs

| Doc | Scope |
|-----|-------|
| `src/core/CLAUDE.md` | Core `.cpp` implementations |
| `src/handlers/CLAUDE.md` | CEF handler implementations |
| `include/core/CLAUDE.md` | Core header roster |
| `include/handlers/CLAUDE.md` | Handler header roster |
| `tests/CLAUDE.md` | C++ test target + how to run |
| `CROSS_PLATFORM_GUIDE.md` | Windows/macOS conditional-compilation conventions |
