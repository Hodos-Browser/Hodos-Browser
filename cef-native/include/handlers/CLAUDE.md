# CEF Handler Headers
> Declaration files for the four core CEF handler classes that drive the browser shell's process architecture.

## Overview

This directory contains the header files for CEF's handler hierarchy: application lifecycle (`SimpleApp`), browser-process client (`SimpleHandler`), render-process V8 injection (`SimpleRenderProcessHandler`), and off-screen overlay rendering (`MyOverlayRenderHandler`). Together they define the interfaces for IPC dispatch, overlay management, keyboard shortcuts, context menus, download tracking, V8 API injection, and platform-specific off-screen rendering. Implementations live in `cef-native/src/handlers/`.

## Files

| File | Purpose |
|------|---------|
| `simple_app.h` | `SimpleApp` — CefApp + CefBrowserProcessHandler + CefRenderProcessHandler. Application entry point for CEF initialization, command-line processing, overlay creation functions (11 on Windows, 6 on macOS), and platform-specific global window/view externs |
| `simple_handler.h` | `SimpleHandler` — CefClient implementing 12 CEF handler interfaces. Browser-process hub for IPC dispatch (70+ message types), keyboard shortcuts, context menus, download tracking, SSL cert exceptions, overlay browser accessors, tab list notifications, and per-window ownership |
| `simple_render_process_handler.h` | `SimpleRenderProcessHandler` — CefRenderProcessHandler for V8 context setup. Injects `window.hodosBrowser` and `window.cefMessage` APIs, scriptlet pre-cache injection, fingerprint seed injection, and cosmetic CSS/script IPC handling |
| `my_overlay_render_handler.h` | `MyOverlayRenderHandler` — CefRenderHandler for off-screen rendered overlays. GDI + `UpdateLayeredWindow` on Windows, Core Animation `CALayer` on macOS. Handles per-pixel alpha blending for transparent overlay popups |

## Key Exports

### SimpleApp (`simple_app.h`)

**Class**: `SimpleApp` — singleton CefApp that bootstraps the browser.

| Method | Purpose |
|--------|---------|
| `OnBeforeCommandLineProcessing()` | Appends `--lang=en-US` and `--remote-allow-origins=*` |
| `OnContextInitialized()` | Creates initial tab/NTP on Windows; macOS defers to manual browser setup |
| `SetWindowHandles()` (Windows) | Stores main HWND, header HWND, webview HWND |
| `SetMacOSWindow()` (macOS) | Stores NSWindow*, header NSView*, webview NSView* |

**Platform globals** declared as `extern`:

| Windows | macOS | Purpose |
|---------|-------|---------|
| `g_hwnd` | `g_main_window` | Main application window |
| `g_header_hwnd` | `g_header_view` | Toolbar/tab bar browser |
| `g_webview_hwnd` | `g_webview_view` | Legacy webview (unused, kept for compat) |
| `g_settings_overlay_hwnd` | `g_settings_overlay_window` | Settings overlay |
| `g_wallet_overlay_hwnd` | `g_wallet_overlay_window` | Wallet overlay |
| `g_backup_overlay_hwnd` | `g_backup_overlay_window` | Backup modal overlay |
| `g_brc100_auth_overlay_hwnd` | `g_brc100_auth_overlay_window` | BRC-100 auth dialog |
| `g_notification_overlay_hwnd` | `g_notification_overlay_window` | Notification toast |
| `g_settings_menu_overlay_hwnd` | `g_settings_menu_overlay_window` | Settings menu dropdown |
| `g_omnibox_overlay_hwnd` | *(not yet ported)* | Omnibox search overlay |

**Overlay creation functions** (Windows — 11 total, macOS — 6):

| Function | Overlay Type | Close Mechanism |
|----------|-------------|-----------------|
| `CreateSettingsOverlayWithSeparateProcess()` | Dropdown | Mouse hook |
| `CreateWalletOverlayWithSeparateProcess()` | Full-screen | `WM_ACTIVATE` + sync prevent-close guard |
| `CreateBackupOverlayWithSeparateProcess()` | Full-screen modal | Manual close only |
| `CreateBRC100AuthOverlayWithSeparateProcess()` | Full-screen | IPC close |
| `CreateNotificationOverlay()` | Toast (keep-alive) | JS injection, auto-dismiss |
| `CreateSettingsMenuOverlay()` | Small dropdown | Mouse hook |
| `CreateOmniboxOverlay()` | Keep-alive | Lazy mouse hook |
| `CreateCookiePanelOverlay()` | Right-side panel (keep-alive) | Mouse hook |
| `CreateDownloadPanelOverlay()` | 380x400 dropdown (keep-alive) | Mouse hook |
| `CreateMenuOverlay()` | 280x450 menu (keep-alive) | Mouse hook |
| `CreateProfilePanelOverlay()` | 380x380 dropdown (keep-alive) | Mouse hook |

**Helper struct** (macOS only):
- `ViewDimensions` — `{ int width, int height }` returned by `GetViewDimensions(void* nsview)`

---

### SimpleHandler (`simple_handler.h`)

**Class**: `SimpleHandler` — the browser-process client handler, implementing 12 CEF interfaces.

**CEF Interfaces Implemented:**

| Interface | Key Overrides |
|-----------|--------------|
| `CefClient` | `OnProcessMessageReceived()` — 70+ IPC message dispatch |
| `CefLifeSpanHandler` | `OnAfterCreated()`, `OnBeforeClose()`, `OnBeforePopup()` |
| `CefDisplayHandler` | `OnTitleChange()`, `OnAddressChange()`, `OnFaviconURLChange()`, `OnFullscreenModeChange()`, `OnCursorChange()` |
| `CefLoadHandler` | `OnLoadError()`, `OnLoadingStateChange()` |
| `CefRequestHandler` | `GetResourceRequestHandler()`, `OnBeforeBrowse()`, `OnCertificateError()` |
| `CefContextMenuHandler` | `OnBeforeContextMenu()`, `OnContextMenuCommand()` |
| `CefDialogHandler` | `OnFileDialog()` — sets `g_file_dialog_active` guard |
| `CefKeyboardHandler` | `OnPreKeyEvent()` — keyboard shortcuts |
| `CefPermissionHandler` | *(permission grant/deny)* |
| `CefDownloadHandler` | `CanDownload()`, `OnBeforeDownload()`, `OnDownloadUpdated()` |
| `CefFindHandler` | `OnFindResult()` |
| `CefJSDialogHandler` | `OnBeforeUnloadDialog()` — beforeunload trap suppression |

**Constructor**: `SimpleHandler(role, window_id)` — `role` identifies the browser type (`"header"`, `"tab_1"`, `"wallet"`, `"omnibox"`, etc.), `window_id` associates with a `BrowserWindow`.

**Static browser accessors** (16 total):

| Accessor | Browser Role |
|----------|-------------|
| `GetHeaderBrowser()` | Toolbar/tab bar |
| `GetWebviewBrowser()` | Legacy webview |
| `GetWalletPanelBrowser()` | Wallet panel |
| `GetOverlayBrowser()` | Generic overlay |
| `GetSettingsBrowser()` | Settings |
| `GetWalletBrowser()` | Wallet |
| `GetBackupBrowser()` | Backup modal |
| `GetBRC100AuthBrowser()` | Auth dialog |
| `GetNotificationBrowser()` | Notification toast |
| `GetSettingsMenuBrowser()` | Settings menu |
| `GetOmniboxBrowser()` | Omnibox |
| `GetCookiePanelBrowser()` | Cookie panel |
| `GetDownloadPanelBrowser()` | Download panel |
| `GetProfilePanelBrowser()` | Profile picker |
| `GetMenuBrowser()` | Three-dot menu |

**Download tracking struct**:
```cpp
struct DownloadInfo {
    uint32_t id;
    std::string url, filename, full_path;
    int64_t received_bytes, total_bytes;
    int percent_complete;
    int64_t current_speed;
    bool is_in_progress, is_complete, is_canceled, is_paused;
    CefRefPtr<CefDownloadItemCallback> item_callback;
};
```
- `active_downloads_` — `std::map<uint32_t, DownloadInfo>` tracking all downloads
- `paused_downloads_` — `std::set<uint32_t>` of paused download IDs
- `NotifyDownloadStateChanged()` — serializes state to JSON, sends `download_state_update` IPC to frontend

**Keyboard shortcuts** (`OnPreKeyEvent()`):

| Shortcut | Action |
|----------|--------|
| F12 | Open DevTools |
| Ctrl/Cmd+F | Find in page (tab browsers only) |
| Ctrl/Cmd+I | DevTools (alternate) |
| Ctrl/Cmd+N | New window |
| Ctrl/Cmd+T | New tab |
| Ctrl/Cmd+W | Close tab |
| Ctrl/Cmd+H | History |
| Ctrl/Cmd+J | Downloads |
| Ctrl/Cmd+D | Bookmark current page |
| Alt+Left/Right | Navigate back/forward |

**Context menu command IDs** (base `MENU_ID_USER_FIRST` = 26500):

| ID | Command |
|----|---------|
| 26501 | DevTools Inspect |
| 26502 | Open Link in New Tab |
| 26503 | Copy Link Address |
| 26504 | Save Image As |
| 26505 | Copy Image URL |
| 26506 | Open Image in New Tab |
| 26510–26521 | Back, Forward, Reload, Edit operations |

**Multi-window support**:
- `GetWindowId()` / `SetWindowId()` — per-handler window association
- `GetOwnerWindow()` — returns the `BrowserWindow*` that owns this handler
- `GetHandlerForBrowser(browser_id)` — static lookup for overlay retargeting
- `NotifyTabListChanged()` — notifies ALL windows' frontends
- `NotifyWindowTabListChanged(window_id)` — notifies ONE window's frontend

**Other state**:
- `allowed_cert_exceptions_` — session-only set of domains where user proceeded past SSL errors
- `last_cosmetic_url_` — per-handler dedup for cosmetic filter injection
- `pending_panel_` / `pending_shield_domain_` / `needs_overlay_reload_` — deferred panel show state

---

### SimpleRenderProcessHandler (`simple_render_process_handler.h`)

**Class**: `SimpleRenderProcessHandler` — runs in each renderer subprocess.

| Method | Purpose |
|--------|---------|
| `OnContextCreated()` | Injects V8 API objects into JavaScript context |
| `OnProcessMessageReceived()` | Handles browser→renderer IPC responses |

**V8 API injected** (in implementation):
```
window.hodosBrowser (READONLY)
├── identity: { get(), markBackedUp() }
├── navigation: { navigate(url) }
├── address: { generate(), getAll(), getCurrent() }
├── history: { get(), search(), searchWithFrecency(), delete(), clearAll(), clearRange(), test() }
├── overlay: { close() }
├── googleSuggest: { fetch() }         [omnibox overlay only]
└── brc100.*                            [registered by BRC100Handler::RegisterBRC100API()]

window.cefMessage (READONLY)
└── send(name, ...args)                 [CefMessageSendHandler — generic IPC]
```

**V8 handler classes** (in implementation):
- `CefMessageSendHandler` — serializes JS args to `CefProcessMessage`, sends to browser process
- `OverlayCloseHandler` — sends `overlay_close` IPC
- `OmniboxCloseHandler` — sends `omnibox_hide` IPC
- `HistoryV8Handler` — wraps `HistoryManager` singleton for direct history access
- `GoogleSuggestV8Handler` — sends `google_suggest_request` IPC

**Static caches** (in implementation):
- `s_scriptCache` — URL → scriptlet JS, pre-cached via `OnBeforeBrowse` IPC, injected once per context
- `s_domainSeeds` — URL → `uint32_t` fingerprint PRNG seed, injected once per frame

**IPC responses handled** (`OnProcessMessageReceived()`):
`tab_list_response`, `find_show`, `find_result`, `download_state_update`, `most_visited_response`, `preload_cosmetic_script`, `fingerprint_seed`, `inject_cosmetic_css`, `inject_cosmetic_script`, `download_folder_selected`, `session_blocked_total_response`

---

### MyOverlayRenderHandler (`my_overlay_render_handler.h`)

**Class**: `MyOverlayRenderHandler` — CefRenderHandler for all OSR overlay browsers.

| Method | Purpose |
|--------|---------|
| `GetViewRect()` | Returns current overlay dimensions from HWND/NSView |
| `OnPaint()` | Composites CEF pixel buffer to native window surface |
| `GetScreenPoint()` | View-to-screen coordinate mapping |
| `GetScreenInfo()` | DPI and scale factor reporting |
| `OnPopupShow()` / `OnPopupSize()` | CEF select element popup handling |

**Platform rendering**:

| Platform | Technique | Key Detail |
|----------|-----------|------------|
| Windows | GDI `UpdateLayeredWindow` with DIB section | Per-pixel alpha via `BLENDFUNCTION(AC_SRC_OVER, AC_SRC_ALPHA)`. Removes `WS_EX_TRANSPARENT` after first non-transparent paint to enable mouse input |
| macOS | Core Animation `CALayer.contents` via `CGImageRef` | Main-thread dispatch via `dispatch_async`. Uses `CATransaction.setDisableActions:YES` to prevent fade-in ghosting. Malloc-copies buffer to avoid CEF reuse artifacts |

**Private members** (platform-specific):
- Windows: `HWND hwnd_`, `HDC hdc_mem_`, `HBITMAP hbitmap_`, `void* dib_data_`
- macOS: `void* nsview_` (bridged NSView pointer)

## Architecture Patterns

### Role-Based Handler Dispatch

`SimpleHandler` instances are created with a `role` string that determines their behavior:
- Tab browsers: `"tab_1"`, `"tab_2"`, etc. — `ExtractTabIdFromRole()` parses the numeric ID
- Overlay browsers: `"wallet"`, `"settings"`, `"omnibox"`, `"downloadpanel"`, etc.
- Infrastructure: `"header"` (toolbar), `"webview"` (legacy)

The role affects which keyboard shortcuts fire, which context menu items appear, which V8 APIs get injected, and how `OnAfterCreated()` registers the browser.

### Browser Process ↔ Render Process IPC

All cross-browser communication routes through `SimpleHandler` in the browser process:

```
React (renderer A) → cefMessage.send("msg", args)
  → CefMessageSendHandler (V8)
    → SendProcessMessage(PID_BROWSER)
      → SimpleHandler::OnProcessMessageReceived()
        → [processes or forwards]
          → SendProcessMessage(PID_RENDERER)
            → SimpleRenderProcessHandler::OnProcessMessageReceived() (renderer B)
```

There is no direct renderer-to-renderer IPC.

### Static Browser Registry

`SimpleHandler` maintains a static map `browser_handler_map_` (browser ID → handler pointer) plus 14 static `CefRefPtr<CefBrowser>` members for each overlay type. These are set in `OnAfterCreated()` and cleared in `OnBeforeClose()`. The `Get*Browser()` static accessors provide type-safe access.

### OSR vs Windowed Rendering

Headers and tabs use **windowed** rendering (`SetAsChild`) — keyboard input works natively. All overlays use **OSR** via `MyOverlayRenderHandler` — keyboard events must be manually forwarded through the overlay's WndProc via `SendKeyEvent()`.

## Usage

**Creating a new overlay**: Add the extern HWND/NSWindow in `simple_app.h` with platform conditionals, add a `Create*Overlay()` function declaration, add a `Get*Browser()` static accessor in `simple_handler.h`, implement creation in `simple_app.cpp`, and register the browser in `SimpleHandler::OnAfterCreated()`.

**Adding a new V8 API**: Create a `CefV8Handler` subclass, register it in `SimpleRenderProcessHandler::OnContextCreated()` under `window.hodosBrowser`.

**Adding a new IPC message**: Handle the message name in `SimpleHandler::OnProcessMessageReceived()`. If the render process needs to receive a response, add handling in `SimpleRenderProcessHandler::OnProcessMessageReceived()`.

**Adding a keyboard shortcut**: Add the key check in `SimpleHandler::OnPreKeyEvent()` with appropriate role filtering (e.g., tab-only vs global).

## Related

- [`../core/CLAUDE.md`](../core/CLAUDE.md) — Core singleton headers (managers, services, caches)
- [`../../CLAUDE.md`](../../CLAUDE.md) — CEF native layer overview, build instructions, HWND hierarchy
- [`../../src/handlers/`](../../src/handlers/) — Implementation files for these headers
- [`../../../CLAUDE.md`](../../../CLAUDE.md) — Root project context, overlay lifecycle docs, CEF input patterns
