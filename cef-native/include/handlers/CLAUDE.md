# CEF Handler Headers

> Declaration files for the four core CEF handler classes that drive the browser shell's process architecture.

**Last Updated:** 2026-08-03

## Overview

This directory contains the header files for CEF's handler hierarchy: application lifecycle (`SimpleApp`), browser-process client (`SimpleHandler`), render-process V8 injection (`SimpleRenderProcessHandler`), and off-screen overlay rendering (`MyOverlayRenderHandler`). Together they define the interfaces for IPC dispatch, overlay management, keyboard shortcuts, context menus, download tracking, V8 API injection, and platform-specific off-screen rendering. Implementations live in `cef-native/src/handlers/`.

## Files

| File | Purpose |
|------|---------|
| `simple_app.h` | `SimpleApp` — CefApp + CefBrowserProcessHandler + CefRenderProcessHandler. Application entry point for CEF initialization, command-line processing, a *subset* of the overlay creation declarations (see note below), and platform-specific global window/view externs |
| `simple_handler.h` | `SimpleHandler` — CefClient implementing 12 CEF handler interfaces. Browser-process hub for IPC dispatch (171 distinct message names), keyboard shortcuts, context menus, download tracking, SSL cert exceptions, site-permission prompts, overlay browser accessors, tab list notifications, and per-window ownership |
| `simple_render_process_handler.h` | `SimpleRenderProcessHandler` — CefRenderProcessHandler for V8 context setup. Injects `window.hodosBrowser` and `window.cefMessage` APIs, the wallet-call IPC bridge + `window.CWI` shim, scriptlet pre-cache injection, fingerprint seed injection, and cosmetic CSS/script IPC handling |
| `my_overlay_render_handler.h` | `MyOverlayRenderHandler` — CefRenderHandler for off-screen rendered overlays. GDI + `UpdateLayeredWindow` on Windows, Core Animation `CALayer` on macOS. Handles per-pixel alpha blending for transparent overlay popups |

## Key Exports

### SimpleApp (`simple_app.h`)

**Class**: `SimpleApp` — CefApp that bootstraps the browser.

| Method | Purpose |
|--------|---------|
| `GetBrowserProcessHandler()` / `GetRenderProcessHandler()` | CefApp plumbing — returns `this` / the owned `SimpleRenderProcessHandler` |
| `OnBeforeCommandLineProcessing()` | Appends CEF command-line switches (locale, remote-allow-origins, etc.) |
| `OnBeforeChildProcessLaunch()` | Propagates the active profile id to renderer/GPU/utility processes so per-process singletons (`HistoryManager`) bind to the right profile instead of a hardcoded `Default` |
| `OnContextInitialized()` | Creates initial tab/NTP on Windows; macOS defers to manual browser setup |
| `SetWindowHandles()` (Windows) | Stores main HWND, header HWND, webview HWND |
| `SetMacOSWindow()` (macOS) | Stores NSWindow*, header NSView*, webview NSView* |

**Platform globals** declared as `extern` in `simple_app.h`, defined in `cef_browser_shell.cpp` (Windows) / `cef_browser_shell_mac.mm` (macOS). Both platforms define **3 primary handles + 14 overlay handles**:

| Windows (`HWND`) | macOS (`NSWindow*` / `NSView*`) | Purpose |
|---------|-------|---------|
| `g_hwnd` | `g_main_window` | Main application window |
| `g_header_hwnd` | `g_header_view` | Toolbar/tab bar browser |
| `g_webview_hwnd` | `g_webview_view` | Legacy webview (unused, kept for compat) |
| `g_settings_overlay_hwnd` | `g_settings_overlay_window` | Settings overlay (`/settings`) |
| `g_wallet_overlay_hwnd` | `g_wallet_overlay_window` | Wallet overlay (`/wallet-panel`) |
| `g_backup_overlay_hwnd` | `g_backup_overlay_window` | Backup modal overlay (`/backup`) |
| `g_brc100_auth_overlay_hwnd` | `g_brc100_auth_overlay_window` | BRC-100 auth dialog (`/brc100-auth`) |
| `g_notification_overlay_hwnd` | `g_notification_overlay_window` | Notification/prompt toast (`/brc100-auth?type=…` — multiplexed) |
| `g_settings_menu_overlay_hwnd` | `g_settings_menu_overlay_window` | Settings menu dropdown (`/settings-menu`) |
| `g_omnibox_overlay_hwnd` | `g_omnibox_overlay_window` | Omnibox search overlay (`/omnibox`) |
| `g_cookie_panel_overlay_hwnd` | `g_cookie_panel_overlay_window` | Privacy Shield panel (`/privacy-shield`) |
| `g_download_panel_overlay_hwnd` | `g_download_panel_overlay_window` | Downloads panel (`/downloads`) |
| `g_profile_panel_overlay_hwnd` | `g_profile_panel_overlay_window` | Profile picker (`/profile-picker`) |
| `g_menu_overlay_hwnd` | `g_menu_overlay_window` | Three-dot menu (`/menu`) |
| `g_bookmarks_panel_overlay_hwnd` | `g_bookmarks_panel_overlay_window` | Bookmarks panel (`/bookmarks`) |
| `g_siteinfo_panel_overlay_hwnd` | `g_siteinfo_panel_overlay_window` | Site-info hub (`/site-info`) |
| `g_tablist_panel_overlay_hwnd` | `g_tablist_panel_overlay_window` | Tab-list panel (`/tab-list`) |

Windows additionally declares `g_hInstance` and `g_wallet_overlay_prevent_close` here (the file-dialog guard `g_file_dialog_active` lives in `cef_browser_shell.cpp` and is pulled in via local `extern`).

**Overlay creation functions — 14 on Windows, 14 on macOS.**

> ⚠️ `simple_app.h` only *declares* 5 of the 14 Windows creation functions (`CreateSettingsOverlayWithSeparateProcess`, `CreateBRC100AuthOverlayWithSeparateProcess`, `CreateNotificationOverlay`, `CreateSettingsMenuOverlay`, `CreateOmniboxOverlay`, plus `ShowOmniboxOverlay`/`HideOmniboxOverlay`) and 6 of the 14 macOS ones. The remainder are reached through function-local `extern` declarations at their call sites in `simple_handler.cpp` and `cef_browser_shell.cpp`. The authoritative roster is the definitions themselves — `src/handlers/simple_app.cpp` for Windows, `cef_browser_shell_mac.mm` for macOS.

Windows — defined in `src/handlers/simple_app.cpp`. Sizes are logical px passed through `ScalePx()` (per-monitor DPI aware):

| Function | Overlay | Size | Close Mechanism |
|----------|---------|------|-----------------|
| `CreateSettingsOverlayWithSeparateProcess()` | Settings panel | 450×450 | `SettingsPanelMouseHookProc` (WH_MOUSE_LL) |
| `CreateWalletOverlay()` | Wallet panel | 400 × height below header | `WM_ACTIVATE` in `WalletOverlayWndProc` + sync `g_wallet_overlay_prevent_close` guard |
| `CreateBackupOverlayWithSeparateProcess()` | Backup modal | Full main window | `WM_ACTIVATE` + IPC `overlay_close` |
| `CreateBRC100AuthOverlayWithSeparateProcess()` | Auth dialog | Full main window | `WM_ACTIVATE` + IPC `overlay_close` |
| `CreateNotificationOverlay()` | Prompt/toast (keep-alive) | Full main window | JS injection, auto-dismiss |
| `CreateSettingsMenuOverlay()` | Small dropdown | 200×120 | IPC `overlay_close` / `WM_CLOSE` only — **no mouse hook, no `WM_ACTIVATE`** |
| `CreateOmniboxOverlay()` | Omnibox (keep-alive) | (header width − 2×152) × 350 | `OmniboxMouseHookProc` (lazy) + `WM_ACTIVATEAPP` in main WndProc |
| `CreateCookiePanelOverlay()` | Privacy Shield panel (keep-alive) | 450×370 | `CookiePanelMouseHookProc` |
| `CreateDownloadPanelOverlay()` | Downloads dropdown (keep-alive) | 380×400 | `DownloadPanelMouseHookProc` |
| `CreateSiteInfoPanelOverlay()` | Site-info hub (keep-alive) | 360×480 | `SiteInfoPanelMouseHookProc` |
| `CreateTabListPanelOverlay()` | Tab-list dropdown (keep-alive) | 340×480 | `TabListPanelMouseHookProc` + `WM_ACTIVATE` |
| `CreateBookmarksPanelOverlay()` | Bookmarks dropdown (keep-alive) | 380×480 | `BookmarksPanelMouseHookProc` + `WM_ACTIVATE` |
| `CreateMenuOverlay()` | Three-dot menu (keep-alive) | 280×450 | `MenuMouseHookProc` |
| `CreateProfilePanelOverlay()` | Profile picker (keep-alive) | 380×520 | `ProfilePanelMouseHookProc` + `WM_ACTIVATE` |

Each also has paired `Show*Overlay(...)` / `Hide*Overlay()` functions (except the modal/full-screen four, which are created-on-demand and destroyed on close).

macOS — defined in `cef_browser_shell_mac.mm`. Same 14 overlays; naming diverges (`…WithSeparateProcess` for the early ports, `…MacOS` / `…Mac` for the later ones):

| Function | Corresponding Windows function |
|----------|-------------------------------|
| `CreateSettingsOverlayWithSeparateProcess()` | `CreateSettingsOverlayWithSeparateProcess()` |
| `CreateWalletOverlayWithSeparateProcess()` | `CreateWalletOverlay()` |
| `CreateBackupOverlayWithSeparateProcess()` | `CreateBackupOverlayWithSeparateProcess()` |
| `CreateBRC100AuthOverlayWithSeparateProcess()` | `CreateBRC100AuthOverlayWithSeparateProcess()` |
| `CreateNotificationOverlay()` | `CreateNotificationOverlay()` |
| `CreateSettingsMenuOverlay()` | `CreateSettingsMenuOverlay()` |
| `CreateCookiePanelOverlayWithSeparateProcess()` | `CreateCookiePanelOverlay()` |
| `CreateOmniboxOverlayMacOS()` | `CreateOmniboxOverlay()` |
| `CreateDownloadPanelOverlayMacOS()` | `CreateDownloadPanelOverlay()` |
| `CreateProfilePanelOverlayMacOS()` | `CreateProfilePanelOverlay()` |
| `CreateBookmarksPanelOverlayMacOS()` | `CreateBookmarksPanelOverlay()` |
| `CreateSiteInfoPanelOverlayMacOS()` | `CreateSiteInfoPanelOverlay()` |
| `CreateTabListPanelOverlayMacOS()` | `CreateTabListPanelOverlay()` |
| `CreateMenuOverlayMac()` | `CreateMenuOverlay()` |

`cef_browser_shell_mac.mm` also defines a Windows-signature shim `CreateMenuOverlay(void* hInstance, bool, int)` that forwards to `CreateMenuOverlayMac()`, so shared call sites compile on both platforms. Every macOS overlay has matching `Show*` / `Hide*` functions.

**Helper struct** (macOS only):
- `ViewDimensions` — `{ int width, int height }` returned by `GetViewDimensions(void* nsview)`

---

### SimpleHandler (`simple_handler.h`)

**Class**: `SimpleHandler` — the browser-process client handler, implementing **12 CEF interfaces** (verified against the class's base list in `simple_handler.h`).

**CEF Interfaces Implemented:**

| Interface | Key Overrides |
|-----------|--------------|
| `CefClient` | `OnProcessMessageReceived()` — 171 distinct IPC message names; plus the 8 `Get*Handler()` accessors and `GetRenderHandler()` |
| `CefLifeSpanHandler` | `OnAfterCreated()`, `DoClose()`, `OnBeforeClose()`, `OnBeforePopup()` |
| `CefDisplayHandler` | `OnTitleChange()`, `OnAddressChange()`, `OnFaviconURLChange()`, `OnFullscreenModeChange()`, `OnCursorChange()` |
| `CefLoadHandler` | `OnLoadError()`, `OnLoadingStateChange()` |
| `CefRequestHandler` | `GetResourceRequestHandler()`, `OnBeforeBrowse()`, `OnCertificateError()` |
| `CefContextMenuHandler` | `OnBeforeContextMenu()`, `OnContextMenuCommand()`, `RunContextMenu()` (macOS converts the rebuilt `CefMenuModel` to an `NSMenu` itself; Windows returns false so CEF shows its own) |
| `CefDialogHandler` | `OnFileDialog()` — sets `g_file_dialog_active` guard |
| `CefKeyboardHandler` | `OnPreKeyEvent()` — keyboard shortcuts |
| `CefPermissionHandler` | `OnRequestMediaAccessPermission()` (camera/mic), `OnShowPermissionPrompt()` (location/notifications/clipboard), `OnDismissPermissionPrompt()`. Honors stored Allow/Block silently; returns false on "Ask" so the stock prompt shows |
| `CefDownloadHandler` | `CanDownload()`, `OnBeforeDownload()`, `OnDownloadUpdated()` |
| `CefFindHandler` | `OnFindResult()` |
| `CefJSDialogHandler` | `OnBeforeUnloadDialog()` — beforeunload trap suppression |

**Constructor**: `SimpleHandler(role, window_id = 0)` — `role` identifies the browser type, `window_id` associates with a `BrowserWindow`.

**Static browser accessors** (18 total). All of them now resolve through `WindowManager::GetInstance().GetPrimaryWindow()` and read the corresponding field on that `BrowserWindow` — they are backwards-compat shims for window 0. Handler-local code should prefer `GetOwnerWindow()` instead:

| Accessor | `BrowserWindow` field | Browser Role |
|----------|----------------------|-------------|
| `GetHeaderBrowser()` | `header_browser` | Toolbar/tab bar |
| `GetWebviewBrowser()` | `webview_browser` | Legacy webview |
| `GetOverlayBrowser()` | `overlay_browser` | Generic overlay |
| `GetWalletPanelBrowser()` | `wallet_panel_browser` | Wallet panel |
| `GetSettingsBrowser()` | `settings_browser` | Settings |
| `GetWalletBrowser()` | `wallet_browser` | Wallet |
| `GetBackupBrowser()` | `backup_browser` | Backup modal |
| `GetBRC100AuthBrowser()` | `brc100_auth_browser` | Auth dialog |
| `GetNotificationBrowser()` | `notification_browser` | Notification/prompt toast |
| `GetSettingsMenuBrowser()` | `settings_menu_browser` | Settings menu |
| `GetOmniboxBrowser()` | `omnibox_browser` | Omnibox |
| `GetCookiePanelBrowser()` | `cookie_panel_browser` | Privacy Shield panel |
| `GetDownloadPanelBrowser()` | `download_panel_browser` | Download panel |
| `GetBookmarksPanelBrowser()` | `bookmarks_panel_browser` | Bookmarks panel |
| `GetSiteInfoPanelBrowser()` | `siteinfo_panel_browser` | Site-info hub |
| `GetTabListPanelBrowser()` | `tablist_panel_browser` | Tab-list panel |
| `GetProfilePanelBrowser()` | `profile_panel_browser` | Profile picker |
| `GetMenuBrowser()` | `menu_browser` | Three-dot menu |

> **Vestigial state:** `simple_handler.h` still declares 15 `static CefRefPtr<CefBrowser>` members (`webview_browser_`, `header_browser_`, `wallet_panel_browser_`, `overlay_browser_`, `settings_browser_`, `wallet_browser_`, `backup_browser_`, `brc100_auth_browser_`, `notification_browser_`, `settings_menu_browser_`, `omnibox_browser_`, `cookie_panel_browser_`, `download_panel_browser_`, `profile_panel_browser_`, `menu_browser_`). They are defined to `nullptr` in `simple_handler.cpp` and cleared in the overlay-close paths, but the accessors above no longer read them. The one remaining *read* is a `header_browser_` null-check in `OnProcessMessageReceived()`, which can never be true because nothing assigns it — a dead branch. Do not treat these as the source of truth for "which browser is which".

**Deferred-panel / deferred-context statics** (set before an overlay finishes loading, injected once it does):
`pending_panel_`, `pending_shield_domain_`, `pending_bookmark_url_`, `pending_bookmark_title_`, `pending_siteinfo_host_`, `pending_siteinfo_security_`, `needs_overlay_reload_`, plus `TriggerDeferredPanel(panel)`.

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
- `download_notify_pending_` — 500 ms debounce flag for the notify throttle
- `NotifyDownloadStateChanged()` — serializes state to JSON, sends `download_state_update` IPC to frontend

**Keyboard shortcuts** (`OnPreKeyEvent()`). On macOS the Ctrl checks become Cmd (`EVENTFLAG_COMMAND_DOWN`); CEF uses Windows key codes on all platforms:

| Shortcut | Action |
|----------|--------|
| Ctrl/Cmd + `+` / `-` / `0` / numpad ± | **Consumed** on non-tab browsers (header + overlays) so browser zoom never scales the chrome |
| F12 | Open/focus DevTools (all browsers) |
| Ctrl+Shift+I / Cmd+Opt+I | DevTools (alternate) |
| Ctrl/Cmd+F | Find in page (tab browsers only — `role_` starts with `tab_`) |
| Ctrl/Cmd+L | Focus address bar (`focus_address_bar` IPC to header) |
| Ctrl/Cmd+N | New window |
| Ctrl/Cmd+T | New tab |
| Ctrl/Cmd+W | Close active tab in this handler's window |
| Ctrl/Cmd+H | Browser data page (`/browser-data`) in a new tab |
| Ctrl/Cmd+J | Show downloads panel overlay |
| Ctrl/Cmd+Shift+A | Show tab-list overlay (Chrome-style tab search; Shift required so plain Ctrl+A still selects all) |
| Ctrl/Cmd+D | Toggle bookmark on current page (adds if absent, removes if present) |
| Ctrl/Cmd+P | Print current page |
| Cmd+`,` | Open settings (macOS only) |
| Alt+Left / Alt+Right | Navigate back/forward on the active tab |

**Context menu command IDs** — all custom, all offsets from `MENU_ID_USER_FIRST` (26500), defined as file-static constants in `simple_handler.cpp`:

| ID | Constant | Command |
|----|----------|---------|
| 26501 | `MENU_ID_DEV_TOOLS_INSPECT` | DevTools Inspect |
| 26502 | `MENU_ID_OPEN_LINK_NEW_TAB` | Open Link in New Tab |
| 26503 | `MENU_ID_COPY_LINK_ADDRESS` | Copy Link Address |
| 26504 | `MENU_ID_SAVE_IMAGE_AS` | Save Image As |
| 26505 | `MENU_ID_COPY_IMAGE_URL` | Copy Image URL |
| 26506 | `MENU_ID_OPEN_IMAGE_NEW_TAB` | Open Image in New Tab |
| 26510 | `MENU_ID_CUSTOM_BACK` | Back |
| 26511 | `MENU_ID_CUSTOM_FORWARD` | Forward |
| 26512 | `MENU_ID_CUSTOM_RELOAD` | Reload |
| 26513–26519 | `MENU_ID_CUSTOM_{UNDO,REDO,CUT,COPY,PASTE,DELETE,SELECT_ALL}` | Edit operations |
| 26520 | `MENU_ID_CUSTOM_VIEW_SOURCE` | View Source |
| 26521 | `MENU_ID_SET_HOMEPAGE` | Set as Homepage |
| 26522 | `MENU_ID_MANAGE_PERMISSIONS` | Manage Site Permissions (quick-revoke flow — load-bearing UX safeguard) |
| 26523 | `MENU_ID_OPEN_LINK_NEW_WINDOW` | Open Link in New Window |

**Multi-window support**:
- `GetWindowId()` / `SetWindowId()` — per-handler window association
- `GetOwnerWindow()` — returns the `BrowserWindow*` that owns this handler
- `GetHandlerForBrowser(browser_id)` — static lookup for overlay retargeting, backed by `browser_handler_map_` (`std::map<int, SimpleHandler*>`)
- `NotifyTabListChanged()` — notifies ALL windows' frontends
- `NotifyWindowTabListChanged(window_id)` — notifies ONE window's frontend
- `ForceCloseRemainingBrowsers()` — shutdown safety net; force-closes browsers still in the handler map (e.g. notification overlays leaked from torn-off windows)

**Other state**:
- `allowed_cert_exceptions_` — session-only set of domains where user proceeded past SSL errors
- `last_cosmetic_url_` — per-handler dedup for cosmetic filter injection
- `is_windowed_browser_` (Windows) — cached at construction to avoid per-cursor-event string ops
- `ExtractTabIdFromRole(role)` — parses `"tab_N"` → N, or −1

---

### SimpleRenderProcessHandler (`simple_render_process_handler.h`)

**Class**: `SimpleRenderProcessHandler` — runs in each renderer subprocess. The header is minimal (2 overrides); everything below lives in the implementation.

| Method | Purpose |
|--------|---------|
| `OnContextCreated()` | Injects V8 API objects, the wallet-call bridge, the `window.CWI` shim, pre-cached scriptlets and the fingerprint seed |
| `OnProcessMessageReceived()` | Handles browser→renderer IPC responses (97 distinct message names) |

**V8 API injected** (in implementation, `OnContextCreated()`):
```
window.hodosBrowser (READONLY)
├── platform: "windows" | "macos"     [ALL pages — bare string, no new fingerprint surface]
├── identity: { get(), markBackedUp() }              ┐
├── navigation: { navigate(url) }                    │
├── address: { generate(), getAll(), getCurrent() }  ├─ internal pages + overlays ONLY
├── history: { get(), search(), searchWithFrecency(),│
│              delete(), clearAll(), clearRange(),   │
│              test() }                              │
├── overlay: { close() }                             ┘
└── googleSuggest: { fetch() }        [omnibox overlay only]

window.cefMessage (READONLY)          [ALL pages]
└── send(name, ...args)               [CefMessageSendHandler — generic IPC]
```

External (dApp) pages get only `hodosBrowser.platform`, `window.cefMessage`, and — on https main frames — the wallet transport bridge plus the provider shim:

- `WALLET_CALL_BRIDGE_SCRIPT` → `window.__hodos_walletCall` / `window.__hodos_walletResponse`. Injected on internal pages, overlays, *and* qualifying external pages. Browser-process side is the `wallet_call` IPC handler in `SimpleHandler::OnProcessMessageReceived()`, which delegates to `HandleIpcWalletCall` (`HttpRequestInterceptor.h`). C++ owns the wallet port so CSP/CORS can't block dApps — the port itself comes from `include/core/PortConfig.h` (127.0.0.1:31301 release, 31401 under `HODOS_DEV=1`).
- `CWI_SHIM_SCRIPT` (`include/core/CWIShimScript.h`) → `window.CWI` / `window.yours` / `window.panda`. Main-frame + `https://` external pages only.

> Legacy `window.hodosBrowser.brc100.*` V8 bindings (`BRC100Handler` / `BRC100Bridge`) were **removed** — they did synchronous WinHTTP on the render thread and the only caller was a startup probe that just logged. Live BRC-100 surfaces are the `window.CWI` shim and the wallet IPC bridge.

**V8 handler classes** (5, in implementation):
- `CefMessageSendHandler` — serializes JS args to `CefProcessMessage`, sends to browser process
- `OverlayCloseHandler` — sends `overlay_close` IPC
- `OmniboxCloseHandler` — sends `omnibox_hide` IPC
- `HistoryV8Handler` — wraps `HistoryManager` singleton for direct history access
- `GoogleSuggestV8Handler` — sends `google_suggest_request` IPC

**Static caches** (3, in implementation, each with its own mutex):
- `s_scriptCache` — URL → scriptlet JS, pre-cached via `preload_cosmetic_script` IPC from `OnBeforeBrowse`; one-shot (erased after main-frame injection)
- `s_domainSeeds` — URL → `uint32_t` fingerprint PRNG seed; one-shot for the main frame
- `s_fingerprintDisabledUrls` — URLs where per-site fingerprint protection is off (`fingerprint_site_disabled` IPC)

**IPC responses handled** (`OnProcessMessageReceived()`) — **97 distinct message names**. They fall into families:

| Family | Examples |
|--------|----------|
| Wallet transport | `wallet_response`, `wallet_response_chunk`, `wallet_status_check_response`, `wallet_payment_dismissed` |
| Wallet CRUD / tx | `create_wallet_response`, `load_wallet_response`, `get_balance_response`/`_error`, `create_transaction_response`/`_error`, `sign_transaction_response`/`_error`, `send_transaction_response`/`_error`, `broadcast_transaction_response`/`_error`, `get_transaction_history_response`/`_error`, `get_wallet_info_response` |
| Identity / addresses | `create_identity_response`, `identity_status_check_response`, `mark_identity_backed_up_response`, `mark_wallet_backed_up_response`, `address_generate_response`/`_error`, `get_addresses_response`, `get_all_addresses_response`, `get_current_address_response` |
| Auth / payments | `brc100_auth_request`, **`payment_success_indicator`** (the GOLD PILL tab indicator — fires on every auto-approved payment; must survive every refactor) |
| Bookmarks | 13 `bookmark_*_response` / `bookmark_folder_*_response` messages |
| Cookies / adblock / fingerprint | 14 `cookie_*_response`, 6 `adblock_*_response`, `fingerprint_seed`, `fingerprint_get_site_enabled_response`, `fingerprint_site_disabled` |
| Cosmetic filtering | `preload_cosmetic_script`, `inject_cosmetic_css`, `inject_cosmetic_script` |
| Browser chrome | `tab_list_response`, `find_show`, `find_result`, `focus_address_bar`, `most_visited_response`, `recently_closed_response`, `session_blocked_total_response`, `omnibox_query_update`, `omnibox_autocomplete_update`, `omnibox_select` |
| Downloads / profiles / settings | `download_state_update`, `download_folder_selected`, `profiles_result`, `import_profiles_result`, `import_complete`, `settings_response`, `site_permissions_response`, `get_backup_modal_state_response`, `set_backup_modal_state_response` |
| Caches / QR | `cache_get_size_response`, `cache_clear_response`, `paid_cache_get_size_response`, `paid_cache_clear_response`, `qr_scan_result`, `qr_screen_capture_starting`, `qr_screen_capture_result`, `google_suggest_response` |

---

### MyOverlayRenderHandler (`my_overlay_render_handler.h`)

**Class**: `MyOverlayRenderHandler` — CefRenderHandler for all OSR overlay browsers. Constructed with `(HWND, width, height)` on Windows, `(void* nsview, width, height)` on macOS.

| Method | Purpose |
|--------|---------|
| `GetViewRect()` | Returns current overlay dimensions from HWND/NSView |
| `OnPaint()` | Composites CEF pixel buffer to native window surface |
| `GetScreenPoint()` | View-to-screen coordinate mapping |
| `GetScreenInfo()` | DPI and scale factor reporting |
| `OnPopupShow()` / `OnPopupSize()` | CEF select element popup handling |
| `DetachView()` | Nulls the view pointer so `OnPaint` can't touch a deallocated view. **Must be called before closing the overlay window.** |
| `~MyOverlayRenderHandler()` | Releases the GDI DC/bitmap (Windows) |

**Platform rendering**:

| Platform | Technique | Key Detail |
|----------|-----------|------------|
| Windows | GDI `UpdateLayeredWindow` with DIB section | Per-pixel alpha via `BLENDFUNCTION(AC_SRC_OVER, AC_SRC_ALPHA)`. Removes `WS_EX_TRANSPARENT` after first non-transparent paint to enable mouse input |
| macOS | Core Animation `CALayer.contents` via `CGImageRef` | Main-thread dispatch via `dispatch_async`. Uses `CATransaction.setDisableActions:YES` to prevent fade-in ghosting. Malloc-copies buffer to avoid CEF reuse artifacts |

**Private members**:
- Both platforms: `int width_`, `int height_`
- Windows: `HWND hwnd_`, `HDC hdc_mem_`, `HBITMAP hbitmap_`, `void* dib_data_`
- macOS: `void* nsview_` (bridged NSView pointer)

## Architecture Patterns

### Role-Based Handler Dispatch

`SimpleHandler` instances are created with a `role` string that determines their behavior. **16 fixed role literals are constructed in code** (15 logical roles — the BRC-100 auth overlay is spelled two different ways, see below), plus the `tab_N` family:

- Tab browsers: `"tab_1"`, `"tab_2"`, … — built in `TabManager.cpp :: CreateTab` / `TabManager_mac.mm`; `ExtractTabIdFromRole()` parses the numeric ID
- Infrastructure: `"header"` (toolbar) — built in `WindowManager.cpp :: CreateWindow` / `WindowManager_mac.mm`
- Overlay browsers: `"settings"`, `"wallet"`, `"backup"`, `"brc100auth"` (Windows) / `"brc100_auth"` (macOS), `"notification"`, `"settings_menu"`, `"omnibox"`, `"cookiepanel"`, `"downloadpanel"`, `"bookmarkspanel"`, `"siteinfopanel"`, `"tablistpanel"`, `"profilepanel"`, `"menu"`

`simple_handler.cpp` additionally compares against `"overlay"`, `"wallet_panel"` and `"webview"`, which nothing currently constructs — legacy branches.

> ⚠️ **Known platform divergence:** `simple_app.cpp` constructs the BRC-100 auth overlay handler with role `"brc100auth"`, but `cef_browser_shell_mac.mm` uses `"brc100_auth"`. `simple_handler.cpp` compares against `"brc100auth"` in 9 places, several of which are *not* `#ifdef _WIN32`-guarded — notably the `OnLoadingStateChange` branch that calls `InjectHodosBrowserAPI()` and `sendAuthRequestDataToOverlay()`. Those branches cannot fire on macOS. Treat as a code bug, not a doc gap.

The role affects which keyboard shortcuts fire, which context menu items appear, which V8 APIs get injected, and how `OnAfterCreated()` registers the browser.

### Browser Process ↔ Render Process IPC

All cross-browser communication routes through `SimpleHandler` in the browser process:

```
React (renderer A) → cefMessage.send("msg", args)
  → CefMessageSendHandler (V8)
    → SendProcessMessage(PID_BROWSER)
      → SimpleHandler::OnProcessMessageReceived()   [171 message names]
        → [processes or forwards]
          → SendProcessMessage(PID_RENDERER)
            → SimpleRenderProcessHandler::OnProcessMessageReceived()  [97 message names] (renderer B)
```

There is no direct renderer-to-renderer IPC.

### Per-Window Browser Registry

`SimpleHandler` keeps a static `browser_handler_map_` (browser ID → handler pointer) used by `GetHandlerForBrowser()` for overlay retargeting. The actual per-role browser references live on the `BrowserWindow` struct owned by `WindowManager` — set in `OnAfterCreated()`, cleared in `OnBeforeClose()`. The 18 static `Get*Browser()` accessors are window-0 convenience shims over those fields (see the vestigial-state note above).

### OSR vs Windowed Rendering

Headers and tabs use **windowed** rendering (`SetAsChild`) — keyboard input works natively. All 14 overlays use **OSR** via `MyOverlayRenderHandler` — keyboard events must be manually forwarded through the overlay's WndProc via `SendKeyEvent()`. On Windows there are 14 overlay WndProcs in `cef_browser_shell.cpp`; only 5 of them forward `WM_KEYDOWN`/`WM_CHAR` (wallet, notification, bookmarks panel, tab-list panel, profile panel) — those are the overlays with text inputs. Nine low-level mouse hooks (`WH_MOUSE_LL`) provide click-outside dismissal: omnibox, settings panel, cookie panel, download panel, bookmarks panel, tab-list panel, profile panel, site-info panel, menu.

## Usage

**Creating a new overlay**: Add the extern HWND/NSWindow in `simple_app.h` with platform conditionals, add a `Create*Overlay()` function declaration (or a local `extern` at the call site, matching the existing pattern), add a `Get*Browser()` static accessor in `simple_handler.h` backed by a new `BrowserWindow` field, implement creation in `simple_app.cpp` (Windows) and `cef_browser_shell_mac.mm` (macOS), add a WndProc + optional mouse hook in `cef_browser_shell.cpp`, and register the browser in `SimpleHandler::OnAfterCreated()`.

**Adding a new V8 API**: Create a `CefV8Handler` subclass, register it in `SimpleRenderProcessHandler::OnContextCreated()` under `window.hodosBrowser`. Decide explicitly whether it belongs inside the `isInternalPage || isOverlayBrowser` gate — anything outside that gate becomes fingerprint surface on every page on the web.

**Adding a new IPC message**: Handle the message name in `SimpleHandler::OnProcessMessageReceived()`. If the render process needs to receive a response, add handling in `SimpleRenderProcessHandler::OnProcessMessageReceived()`.

**Adding a keyboard shortcut**: Add the key check in `SimpleHandler::OnPreKeyEvent()` with appropriate role filtering (e.g., tab-only vs global) and `#ifdef __APPLE__` Cmd-vs-Ctrl handling.

## Related

- [`../core/CLAUDE.md`](../core/CLAUDE.md) — Core singleton headers (managers, services, caches, `PortConfig.h`)
- [`../../CLAUDE.md`](../../CLAUDE.md) — CEF native layer overview, build instructions, HWND hierarchy
- [`../../src/handlers/`](../../src/handlers/) — Implementation files for these headers
- [`../../../CLAUDE.md`](../../../CLAUDE.md) — Root project context, overlay lifecycle docs, CEF input patterns
