# CEF Handler Layer
> CEF application lifecycle, browser-process IPC dispatch, render-process V8 injection, and off-screen overlay rendering.

## Overview

This module contains the five C++ files that implement the CEF handler interfaces — the "brain" of the browser shell. `SimpleApp` manages CEF initialization and overlay window creation. `SimpleHandler` implements 12 CEF client interfaces and dispatches 125+ IPC message types from React to C++/Rust. `SimpleRenderProcessHandler` injects the `window.hodosBrowser` JavaScript API and routes 70+ IPC responses back to React. `MyOverlayRenderHandler` provides platform-specific off-screen rendering for all overlay windows.

All files are cross-platform (Windows + macOS) with `#ifdef _WIN32` / `#elif defined(__APPLE__)` conditionals. Headers live in `cef-native/include/handlers/`.

## Files

| File | Lines | Purpose |
|------|-------|---------|
| `simple_handler.cpp` | ~6740 | **Largest file.** Browser-process CEF client implementing 12 interfaces (CefLifeSpanHandler, CefDisplayHandler, CefLoadHandler, CefRequestHandler, CefContextMenuHandler, CefDialogHandler, CefKeyboardHandler, CefPermissionHandler, CefDownloadHandler, CefFindHandler, CefJSDialogHandler, CefClient). Central IPC dispatcher for 125+ message types. Handles tab creation, navigation, overlay lifecycle, wallet operations, downloads, find-in-page, keyboard shortcuts, context menus, HTTP request interception (ad blocking, cookie filtering, wallet routing), certificate error handling, bookmarks, cookie blocking, profile management, browser import, and multi-window tab coordination. |
| `simple_app.cpp` | ~2425 | CEF application entry point (inherits CefApp + CefBrowserProcessHandler + CefRenderProcessHandler). Configures command-line switches, initializes browser context, creates header browser, restores multi-window sessions from `session.json` (v2 format), and contains all 11 overlay creation functions (Windows only — macOS equivalents in `cef_browser_shell_mac.mm`). |
| `simple_render_process_handler.cpp` | ~2060 | Render-process handler. Injects `window.hodosBrowser.*` and `window.cefMessage` V8 APIs in `OnContextCreated()`. Contains 5 V8 handler classes. Pre-caches and injects adblock scriptlets, cosmetic CSS, and fingerprint protection scripts. Routes 70+ IPC response messages from browser process back to JavaScript via `frame->ExecuteJavaScript()`. |
| `my_overlay_render_handler.cpp` | ~380 | Windows off-screen rendering for overlays. Uses GDI `CreateDIBSection` + `UpdateLayeredWindow` with per-pixel alpha blending. Dynamic bitmap reallocation on resize. Removes `WS_EX_TRANSPARENT` after first paint to enable mouse input. |
| `my_overlay_render_handler.mm` | ~364 | macOS off-screen rendering for overlays. Uses `CGImageCreate` + `CALayer.contents` with `dispatch_async` to main thread. Copies CEF buffer via `malloc` to prevent reuse ghosting. Disables Core Animation implicit transitions via `CATransaction`. Supports Retina via `NSScreen.backingScaleFactor`. |

## Classes

### SimpleHandler (simple_handler.cpp)

Central browser-process handler. One instance per CEF browser (tabs, header, overlays).

**Constructor**: `SimpleHandler(const std::string& role, int window_id = 0)` — role identifies browser purpose (`"header"`, `"tab_1"`, `"wallet"`, `"settings"`, `"omnibox"`, etc.). `window_id` supports multi-window tab routing.

**12 CEF interfaces implemented:**

| Interface | Key Methods |
|-----------|-------------|
| `CefLifeSpanHandler` | `OnAfterCreated` (register with TabManager), `OnBeforeClose` (cleanup), `OnBeforePopup` (open links in new tab) |
| `CefDisplayHandler` | `OnTitleChange`, `OnAddressChange`, `OnFaviconURLChange`, `OnFullscreenModeChange` |
| `CefLoadHandler` | `OnLoadingStateChange`, `OnLoadError` (SSL/DNS error pages) |
| `CefRequestHandler` | `OnBeforeBrowse` (scriptlet pre-cache, fingerprint seed IPC), `GetResourceRequestHandler` (ad blocking, cookie filtering, wallet routing, DNT/GPC injection) |
| `CefContextMenuHandler` | Custom right-click menus with 20+ items (link/image/edit/page contexts, all `MENU_ID_USER_FIRST` IDs) |
| `CefDialogHandler` | `OnFileDialog` (sets `g_file_dialog_active` guard) |
| `CefKeyboardHandler` | `OnPreKeyEvent` — Ctrl/Cmd+F/H/J/L/N/T/W/D/P, F12, Ctrl+Shift+Delete/I |
| `CefPermissionHandler` | `GetPermissionHandler` |
| `CefDownloadHandler` | `CanDownload`, `OnBeforeDownload` (configured folder or native Save As), `OnDownloadUpdated` (progress tracking) |
| `CefFindHandler` | `OnFindResult` — sends match count/ordinal to React find bar |
| `CefJSDialogHandler` | `OnBeforeUnloadDialog` — suppresses beforeunload traps |

**Static browser references** for targeting IPC to specific overlays:
`header_browser_`, `wallet_panel_browser_`, `settings_browser_`, `backup_browser_`, `brc100_auth_browser_`, `omnibox_browser_`, `cookie_panel_browser_`, `download_panel_browser_`, `profile_panel_browser_`, `menu_browser_`, `notification_browser_`

**Multi-window helpers:**
- `ExtractTabIdFromRole()` — parses tab ID from role string (format `"tab_N"`)
- `GetOwnerWindow()` — returns `BrowserWindow*` owning this handler
- `NotifyWindowTabListChanged()` — sends tab list to specific window's header browser

**Ghost tab window** (Windows only): `ShowGhostTab()` / `HideGhostTab()` — lightweight GDI-painted popup following cursor during tab tear-off. macOS equivalents in `WindowManager_mac.mm`.

### SimpleApp (simple_app.cpp)

CEF application object. Singleton created in `main()`.

**Inherits**: `CefApp`, `CefBrowserProcessHandler`, `CefRenderProcessHandler`

**Key methods:**
- `OnBeforeCommandLineProcessing()` — GPU flags, WebRTC IP leak prevention, macOS dev flags (`in-process-gpu`, `use-mock-keychain`, `disable-web-security`)
- `OnContextInitialized()` — creates header browser, restores multi-window session (v2 format: `windows[]` array with per-window tabs and positions) or creates NTP tab
- `SetWindowHandles()` (Windows) / `SetMacOSWindow()` (macOS) — stores platform window references

**11 Overlay creation functions** (Windows, in this file):

| Function | Overlay | Size | Pattern |
|----------|---------|------|---------|
| `CreateSettingsOverlayWithSeparateProcess()` | Settings dropdown | Right-side panel | Mouse hook close |
| `CreateWalletOverlayWithSeparateProcess()` | Wallet panel | Full-screen | Prevent-close flag on creation |
| `CreateBackupOverlayWithSeparateProcess()` | Backup modal | Full-screen | Native file inputs |
| `CreateBRC100AuthOverlayWithSeparateProcess()` | Auth dialog | Full-screen | BRC-100 challenge |
| `CreateNotificationOverlay()` | Notifications | Full-screen | Keep-alive with JS injection |
| `CreateSettingsMenuOverlay()` | Settings menu | Small dropdown | Toggle on repeat click |
| `CreateOmniboxOverlay()` / `Show` / `Hide` | Address bar dropdown | Below toolbar | Keep-alive, lazy mouse hook |
| `CreateCookiePanelOverlay()` / `Show` / `Hide` | Privacy shield | Right-side panel | Keep-alive, handler retarget |
| `CreateDownloadPanelOverlay()` / `Show` / `Hide` | Downloads | 380×400 dropdown | Keep-alive, handler retarget |
| `CreateMenuOverlay()` / `Show` / `Hide` | Hamburger menu | 280×450 dropdown | Keep-alive, handler retarget |
| `CreateProfilePanelOverlay()` / `Show` / `Hide` | Profile picker | 380×380 dropdown | Keep-alive, enables focus |

macOS overlay creation is in `cef_browser_shell_mac.mm`, not in this file.

### SimpleRenderProcessHandler (simple_render_process_handler.cpp)

Runs in each renderer subprocess. Injects JavaScript APIs and routes IPC responses.

**V8 handler classes defined in this file:**

| Class | V8 Path | Methods |
|-------|---------|---------|
| `CefMessageSendHandler` | `window.cefMessage.send()` | Generic IPC dispatch — converts JS args to `CefProcessMessage` |
| `OverlayCloseHandler` | `window.hodosBrowser.overlay.close()` | Sends `overlay_close` IPC |
| `OmniboxCloseHandler` | `window.hodosBrowser.overlay.close()` (omnibox) | Sends `omnibox_hide` IPC |
| `HistoryV8Handler` | `window.hodosBrowser.history.*` | `get`, `search`, `searchWithFrecency`, `delete`, `clearAll`, `clearRange`, `test` |
| `GoogleSuggestV8Handler` | `window.hodosBrowser.googleSuggest.fetch()` | Sends `google_suggest_request` IPC, returns request ID |

**Static caches (thread-safe with mutex):**
- `s_scriptCache` — URL → adblock scriptlet JS (one-shot, erased after injection)
- `s_domainSeeds` — URL → fingerprint PRNG seed (one-shot for main frame)

**Overlay readiness signal:** After V8 injection completes for overlay browsers, dispatches `window.allSystemsReady` custom event and sets `window.allSystemsReady = true` flag.

### MyOverlayRenderHandler (my_overlay_render_handler.cpp/.mm)

Off-screen rendering for all overlay windows. One instance per overlay.

**CefRenderHandler methods:** `GetViewRect`, `OnPaint`, `GetScreenPoint`, `GetScreenInfo`, `OnPopupShow` (stub), `OnPopupSize` (stub)

**Windows (`OnPaint`):** Dynamic bitmap reallocation when overlay resizes. Removes `WS_EX_TRANSPARENT` after first paint to enable mouse input.

**macOS (`OnPaint`):** `malloc` copies CEF buffer to prevent reuse ghosting. `CATransaction.setDisableActions:YES` disables Core Animation implicit transitions. `GetScreenPoint` converts from CEF top-left origin to macOS bottom-left screen coordinates.

## IPC Message Categories

Messages dispatched in `SimpleHandler::OnProcessMessageReceived()`:

| Category | Example Messages | Count |
|----------|-----------------|-------|
| Tab management | `tab_create`, `tab_close`, `tab_switch`, `tab_reorder`, `tab_ghost_show`, `tab_ghost_hide`, `tab_tearoff`, `get_tab_list` | ~8 |
| Navigation | `navigate`, `navigate_back`, `navigate_forward`, `navigate_reload`, `cert_error_proceed`, `cert_error_go_back` | ~6 |
| Omnibox | `omnibox_create`, `omnibox_create_or_show`, `omnibox_show`, `omnibox_hide`, `omnibox_update_query`, `omnibox_select`, `omnibox_autocomplete` | ~7 |
| Overlay lifecycle | `overlay_show_wallet`, `overlay_show_settings`, `overlay_show_settings_menu`, `overlay_show_brc100_auth`, `overlay_close`, `overlay_hide`, `overlay_input`, `toggle_wallet_panel` | ~8 |
| Overlay panels | `cookie_panel_show/hide`, `profile_panel_show/hide`, `menu_show/hide/action`, `download_panel_show/hide` | ~9 |
| Wallet operations | `wallet_status_check`, `create_wallet`, `get_wallet_info`, `load_wallet`, `get_balance`, `send_transaction`, `address_generate`, `get_all_addresses`, `get_current_address`, `mark_wallet_backed_up`, `wallet_prevent_close`, `wallet_allow_close`, `wallet_delete_cancel`, `wallet_payment_dismissed` | ~15 |
| Transactions | `create_transaction`, `sign_transaction`, `broadcast_transaction`, `get_transaction_history` | ~4 |
| Settings & profiles | `settings_get_all`, `settings_set`, `settings_update_all`, `settings_close`, `profiles_get_all`, `profiles_create`, `profiles_rename`, `profiles_delete`, `profiles_switch` | ~9 |
| Browser import | `import_detect_profiles`, `import_bookmarks`, `import_history`, `import_all` | ~4 |
| Bookmarks | `bookmark_add`, `bookmark_get`, `bookmark_update`, `bookmark_remove`, `bookmark_search`, `bookmark_get_all`, `bookmark_is_bookmarked`, `bookmark_get_all_tags`, `bookmark_update_last_accessed`, `bookmark_folder_create/list/update/remove/get_tree` | ~14 |
| Cookie management | `cookie_get_all`, `cookie_delete`, `cookie_delete_domain`, `cookie_delete_all`, `cache_clear`, `cache_get_size` | ~6 |
| Cookie blocking | `cookie_block_domain`, `cookie_unblock_domain`, `cookie_get_blocklist`, `cookie_allow_third_party`, `cookie_remove_third_party_allow`, `cookie_get_block_log`, `cookie_clear_block_log`, `cookie_get_blocked_count`, `cookie_reset_blocked_count`, `cookie_check_site_allowed` | ~10 |
| Ad blocking | `adblock_get_blocked_count`, `adblock_reset_blocked_count`, `adblock_site_toggle`, `adblock_check_site_enabled`, `adblock_scriptlet_toggle`, `adblock_check_scriptlets_enabled`, `cosmetic_class_id_query` | ~7 |
| Downloads | `download_cancel`, `download_pause`, `download_resume`, `download_open`, `download_show_folder`, `download_clear_completed`, `download_get_state`, `download_browse_folder` | ~8 |
| Find in page | `find_text`, `find_result_js`, `find_stop` | ~3 |
| Browser UI | `print`, `devtools`, `zoom_in`, `zoom_out`, `zoom_reset`, `exit`, `force_repaint` | ~7 |
| BRC-100 auth | `brc100_auth_response`, `add_domain_permission`, `add_domain_permission_advanced`, `approve_cert_fields` | ~4 |
| Search & analytics | `google_suggest_request`, `get_most_visited`, `get_session_blocked_total` | ~3 |

## V8 API Shape

Injected in `OnContextCreated()` for all browsers at `http://127.0.0.1:5137`:

```
window.hodosBrowser
├── identity.get()
├── identity.markBackedUp()
├── navigation.navigate(url)       // cross-platform
├── address.generate()
├── address.getAll()               // macOS only
├── address.getCurrent()           // macOS only
├── history.get()
├── history.search(query)
├── history.searchWithFrecency(query)
├── history.delete(id)
├── history.clearAll()
├── history.clearRange(start, end)
├── history.test()
├── overlay.close()                // overlay browsers only
├── googleSuggest.fetch(query)     // omnibox overlay only
└── brc100.*                       // registered by BRC100Handler

window.cefMessage.send(name, ...args)   // generic IPC
window.allSystemsReady                  // set true after V8 injection (overlays only)
```

## Injection Pipeline

Three types of content injected into page contexts before page JavaScript runs:

1. **Adblock scriptlets** — Pre-cached via `preload_cosmetic_script` IPC in `OnBeforeBrowse`, injected from `s_scriptCache` in `OnContextCreated`. One-shot per URL.
2. **Fingerprint protection** — Seed pre-cached via `fingerprint_seed` IPC, `FINGERPRINT_PROTECTION_SCRIPT` constant patched with seed and injected in `OnContextCreated`. Skips auth domains (Google, Microsoft).
3. **Cosmetic CSS/scripts** — Injected post-load via `inject_cosmetic_css` and `inject_cosmetic_script` IPC. CSS creates `<style id="hodos-cosmetic-css">` with `display: none !important` rules.

## Keyboard Shortcuts

Handled in `SimpleHandler::OnPreKeyEvent()`:

| Shortcut | macOS | Action |
|----------|-------|--------|
| Ctrl+D | Cmd+D | Bookmark current page |
| Ctrl+F | Cmd+F | Find in page |
| Ctrl+H | Cmd+H | Open History tab |
| Ctrl+J | Cmd+J | Show Downloads panel |
| Ctrl+L | Cmd+L | Focus address bar |
| Ctrl+N | Cmd+N | New window |
| Ctrl+P | Cmd+P | Print current page |
| Ctrl+T | Cmd+T | New tab |
| Ctrl+W | Cmd+W | Close active tab |
| Ctrl+Shift+Del | Cmd+Shift+Del | Clear browsing data |
| Ctrl+Shift+I | Cmd+Option+I | DevTools (alt) |
| F12 | F12 | DevTools |
| — | Cmd+, | Open Settings (macOS only) |

## Context Menu

Custom context menu replaces Chromium defaults. All IDs use `MENU_ID_USER_FIRST` base (26500):

- **Link context**: Open in New Tab, Copy Link Address
- **Image context**: Save Image As, Copy Image Address, Open Image in New Tab
- **Editable context**: Undo, Redo, Cut, Copy, Paste, Delete, Select All
- **Page context**: Back, Forward, Reload, View Page Source, Set as Home Page
- **All contexts**: Inspect Element (DevTools)

## HTTP Request Interception

`GetResourceRequestHandler()` runs on IO thread for every request:

1. **Wallet bypass** — Direct `127.0.0.1:31301` from wallet/settings/backup roles → `nullptr` (native CEF handling)
2. **DNT/GPC headers** — Injects `DNT: 1` and `Sec-GPC: 1` when privacy setting enabled
3. **Ad blocking** — `AdblockCache::check()` → returns `AdblockBlockHandler` to cancel blocked requests
4. **Wallet routing** — Requests to ports 31301/3321/2121/8080 or `.well-known/auth` → returns `HttpRequestInterceptor`
5. **Cookie/response filtering** — Cookie blocking or YouTube ad response filtering → returns `CookieFilterResourceHandler`

## Overlay Patterns

All overlays use off-screen rendering (OSR) with `MyOverlayRenderHandler`. Common creation pattern:

```cpp
CefWindowInfo window_info;
window_info.windowless_rendering_enabled = true;
window_info.SetAsPopup(hwnd, "RoleName");

CefBrowserSettings settings;
settings.windowless_frame_rate = 30;
settings.background_color = CefColorSetARGB(0, 0, 0, 0);  // transparent
settings.javascript_access_clipboard = STATE_ENABLED;
settings.javascript_dom_paste = STATE_ENABLED;
```

**Keep-alive overlays** (Omnibox, Cookie, Download, Menu, Profile): created once, shown/hidden via `ShowWindow(hwnd, SW_SHOW/SW_HIDE)`. Mouse hook installed lazily on show, removed on hide.

**Full-screen overlays** (Wallet, Backup, BRC-100 Auth): cover entire main window. Wallet sets `g_wallet_overlay_prevent_close = true` on creation (synchronous, no race condition).

**Notification overlay**: Unique keep-alive with JS injection — first call preloads browser hidden, subsequent calls invoke `window.showNotification()` for instant React state update without page reload.

## Platform Differences

| Aspect | Windows | macOS |
|--------|---------|-------|
| Overlay creation | In `simple_app.cpp` (HWND + GDI) | In `cef_browser_shell_mac.mm` (NSWindow + Core Animation) |
| OSR rendering | `UpdateLayeredWindow` + `BLENDFUNCTION` | `CALayer.contents` + `CGImageCreate` |
| Buffer handling | Direct `dib_data_` pointer | `malloc` copy (prevents CEF buffer reuse ghosting) |
| Resize handling | Dynamic `CreateDIBSection` realloc in `OnPaint` | Queries `NSView.bounds` in `GetViewRect` |
| Clipboard | `OpenClipboard` / `SetClipboardData` | `popen("pbcopy")` pipe |
| DPI scaling | Hardcoded 1.0f | `NSScreen.backingScaleFactor` (Retina) |
| Screen coords | Direct pixel mapping | Top-left → bottom-left origin conversion |
| Tab tearoff | Ghost tab window (GDI popup) | Ghost tab via `WindowManager_mac.mm` |
| `HistoryV8Handler` | Full implementation via `HistoryManager` | V8 handler registered but `HistoryManager` not initialized in render process |
| `GoogleSuggestService` | WinHTTP to Google/DuckDuckGo | libcurl to Google/DuckDuckGo |

## Related

- [Parent: CEF Native Shell](../../CLAUDE.md) — build instructions, HWND hierarchy, focus management
- [Sibling: Core Services](../core/CLAUDE.md) — singletons (TabManager, HistoryManager, SettingsManager, HttpRequestInterceptor, etc.) used by these handlers
- [Root: Project](../../../CLAUDE.md) — architecture overview, overlay lifecycle rules, CEF input patterns
