# CEF Handler Layer
> CEF application lifecycle, browser-process IPC dispatch, render-process V8 injection, and off-screen overlay rendering.

> Last Updated: 2026-08-09

## Overview

This module contains the six C++/Objective-C++ source files that implement the CEF handler interfaces — the "brain" of the browser shell. `SimpleApp` manages CEF initialization and overlay window creation. `SimpleHandler` implements 12 CEF client interfaces and dispatches **169 IPC message types** from React to C++/Rust. `SimpleRenderProcessHandler` injects the `window.hodosBrowser` / `window.cefMessage` JavaScript APIs and routes **95 IPC response messages** back to React. `MyOverlayRenderHandler` provides platform-specific off-screen rendering for all overlay windows.

All files are cross-platform (Windows + macOS) with `#ifdef _WIN32` / `#elif defined(__APPLE__)` conditionals, except the two `.mm` files which are macOS-only translation units. Headers live in `cef-native/include/handlers/`.

> **Permission decisions are not made in this layer.** The decision engine is Rust (`rust-wallet/crates/hodos_permission_engine`, wrapped by `rust-wallet/src/permission_service/`, wired as Actix middleware in `rust-wallet/src/main.rs`). The C++ `PermissionEngine` and `SessionManager` were deleted in Phase 2.6-H. These handlers collect context, show prompts, and relay answers — nothing more.

## Files

| File | Lines | Purpose |
|------|-------|---------|
| `simple_handler.cpp` | 9306 | **Largest file in the project.** Browser-process CEF client implementing 12 interfaces (CefClient, CefLifeSpanHandler, CefDisplayHandler, CefLoadHandler, CefRequestHandler, CefContextMenuHandler, CefDialogHandler, CefKeyboardHandler, CefPermissionHandler, CefDownloadHandler, CefFindHandler, CefJSDialogHandler). Central IPC dispatcher for 169 message types (`OnProcessMessageReceived`, lines 1897–7416). Handles tab creation, navigation, window chrome, overlay lifecycle, wallet operations, downloads, find-in-page, keyboard shortcuts, context menus, HTTP request interception (paid-content cache, ad blocking, cookie filtering, wallet routing), certificate error handling, bookmarks, cookie blocking, site permissions, profile management, browser import, QR scanning, and multi-window tab coordination. |
| `simple_app.cpp` | 3356 | CEF application entry point (inherits `CefApp` + `CefBrowserProcessHandler` + `CefRenderProcessHandler`). Configures command-line switches, propagates the active profile to child processes, creates the header browser, restores multi-window sessions from `session.json` (v1 flat + v2 `windows[]` formats), and contains all **14 Windows overlay creation functions**. Everything from line 571 to EOF is inside a single `#ifdef _WIN32` block — macOS equivalents live in `cef_browser_shell_mac.mm`. |
| `simple_render_process_handler.cpp` | 2340 | Render-process handler. Injects `window.hodosBrowser.*` and `window.cefMessage` V8 APIs in `OnContextCreated()`. Contains 5 V8 handler classes. Pre-caches and injects adblock scriptlets, a `window.chrome` bot-detection stub, the wallet IPC bridge, and the `window.CWI`/`yours`/`panda` shim. Routes 95 IPC response messages from browser process back to JavaScript via `frame->ExecuteJavaScript()`. |
| `my_overlay_render_handler.cpp` | 393 | Windows off-screen rendering for overlays. Uses GDI `CreateDIBSection` + `UpdateLayeredWindow` with per-pixel alpha blending. Dynamic bitmap reallocation on resize. Reports true per-monitor DPI via `GetDpiForWindow()`. Removes `WS_EX_TRANSPARENT` after first paint to enable mouse input. |
| `my_overlay_render_handler.mm` | 384 | macOS off-screen rendering for overlays. Uses `CGImageCreate` + `CALayer.contents` with `dispatch_async` to main thread. Copies CEF buffer via `malloc` to prevent reuse ghosting. Disables Core Animation implicit transitions via `CATransaction`. Supports Retina via `NSScreen.backingScaleFactor`. Adds `DetachView()` (no Windows counterpart). |
| `simple_handler_mac.mm` | 158 | macOS-only helper for `SimpleHandler::RunContextMenu`. CEF on macOS windowed rendering does not auto-present the `CefMenuModel` built in `OnBeforeContextMenu`, so this converts `CefMenuModel` → `NSMenu` and pops it up via AppKit. Without it, right-clicking a link navigates instead of opening the menu. Defines `HodosContextMenuTarget` (retains the `CefRunContextMenuCallback` until an item is picked or the menu is dismissed). |

## Classes

### SimpleHandler (simple_handler.cpp)

Central browser-process handler. One instance per CEF browser (tabs, header, overlays).

**Constructor**: `SimpleHandler(const std::string& role, int window_id = 0)` — role identifies browser purpose (`"header"`, `"tab_N"`, `"wallet"`, `"settings"`, `"backup"`, `"omnibox"`, `"downloadpanel"`, etc.). `window_id` supports multi-window tab routing.

**12 CEF interfaces implemented:**

| Interface | Key Methods |
|-----------|-------------|
| `CefClient` | The `Get*Handler()` accessors — all return `this` |
| `CefLifeSpanHandler` | `OnAfterCreated` (register with TabManager), `OnBeforeClose` (cleanup), `OnBeforePopup` (open links in new tab) |
| `CefDisplayHandler` | `OnTitleChange`, `OnAddressChange`, `OnFaviconURLChange`, `OnFullscreenModeChange` |
| `CefLoadHandler` | `OnLoadingStateChange`, `OnLoadError` (SSL/DNS error pages) |
| `CefRequestHandler` | `OnBeforeBrowse` (scriptlet pre-cache, `hodos_farble_key` enabled-bit send), `GetResourceRequestHandler` (paid-content cache, ad blocking, cookie filtering, wallet routing, DNT/GPC injection) |
| `CefContextMenuHandler` | `OnBeforeContextMenu`, `RunContextMenu`, `OnContextMenuCommand` — 20 custom `MENU_ID_USER_FIRST` items |
| `CefDialogHandler` | `OnFileDialog` (sets `g_file_dialog_active` guard) |
| `CefKeyboardHandler` | `OnPreKeyEvent` — see Keyboard Shortcuts below |
| `CefPermissionHandler` | `OnRequestMediaAccessPermission`, `OnShowPermissionPrompt`, `OnDismissPermissionPrompt` — honors stored Allow/Block from `SitePermissionStore`, returns false on "Ask" so Chromium's stock prompt shows |
| `CefDownloadHandler` | `CanDownload`, `OnBeforeDownload` (configured folder or native Save As), `OnDownloadUpdated` (progress tracking) |
| `CefFindHandler` | `OnFindResult` — sends match count/ordinal to React find bar |
| `CefJSDialogHandler` | `OnBeforeUnloadDialog` — suppresses beforeunload traps |

**Static browser references** (declared in `simple_handler.h`, defined at the top of `simple_handler.cpp`) — 15 total:
`webview_browser_`, `header_browser_`, `wallet_panel_browser_`, `overlay_browser_`, `settings_browser_`, `wallet_browser_`, `backup_browser_`, `brc100_auth_browser_`, `notification_browser_`, `settings_menu_browser_`, `omnibox_browser_`, `cookie_panel_browser_`, `download_panel_browser_`, `profile_panel_browser_`, `menu_browser_`

> ⚠️ Three of those are **vestigial**: `download_panel_browser_`, `profile_panel_browser_`, and `menu_browser_` are declared and initialized to `nullptr` but never assigned or read. Their accessors were migrated to per-window storage (below) and the statics were left behind.

**18 static accessors** (`Get*Browser()`) split across two storage models:

| Storage | Accessors |
|---------|-----------|
| Process-global static | `GetOverlayBrowser`, `GetHeaderBrowser`, `GetWebviewBrowser`, `GetWalletPanelBrowser`, `GetSettingsBrowser`, `GetWalletBrowser`, `GetBackupBrowser`, `GetBRC100AuthBrowser`, `GetNotificationBrowser`, `GetSettingsMenuBrowser`, `GetOmniboxBrowser`, `GetCookiePanelBrowser` |
| Per-window (`WindowManager::GetPrimaryWindow()` → `BrowserWindow` field) | `GetDownloadPanelBrowser`, `GetBookmarksPanelBrowser`, `GetSiteInfoPanelBrowser`, `GetTabListPanelBrowser`, `GetProfilePanelBrowser`, `GetMenuBrowser` |

**Multi-window helpers:**
- `SimpleHandler::ExtractTabIdFromRole()` — parses tab ID from role string (format `"tab_N"`)
- `SimpleHandler::GetOwnerWindow()` — returns `BrowserWindow*` owning this handler
- `SimpleHandler::NotifyWindowTabListChanged(int window_id)` — sends tab list to ONE window's header browser
- `SimpleHandler::NotifyTabListChanged()` — broadcasts to all windows
- `SimpleHandler::browser_handler_map_` — static `map<int, SimpleHandler*>` for overlay retargeting

**Ghost tab window** (tab tear-off drag affordance): file-static `ShowGhostTab()` / `HideGhostTab()` in `simple_handler.cpp` are cross-platform wrappers. Windows implements a GDI-painted `WS_POPUP` with `GhostTabWndProc`; macOS delegates to `ShowGhostTabMacOS` / `HideGhostTabMacOS` in `src/core/WindowManager_mac.mm`.

**Other file-scope helpers:** `CreateNewTabWithUrl()`, `CopyTextToClipboard()`, `SendSitePermissionsToBrowser()`, `ParseSitePermState()`, `SitePermStateStr()`, `g_qr_scan_requester` (non-static — `QRScreenCapture.cpp` reaches it via `extern`).

### SimpleApp (simple_app.cpp)

CEF application object. Singleton created in `main()`.

**Inherits**: `CefApp`, `CefBrowserProcessHandler`, `CefRenderProcessHandler`

**5 methods defined in this file** (`GetBrowserProcessHandler` / `GetRenderProcessHandler` are declared in `simple_app.h`; `SetWindowHandles` and `SetMacOSWindow` are platform-exclusive, so only 4 compile per platform):
- `SimpleApp::OnBeforeChildProcessLaunch()` — propagates the active profile id to renderer/GPU/utility processes via `--profile=` so per-process singletons bind to the right profile
- `SimpleApp::OnBeforeCommandLineProcessing()` — `--lang=en-US`, `--remote-allow-origins=*`, `--disable-gpu-compositing` (first-render black screen fix), `--force-webrtc-ip-handling-policy=default_public_interface_only` (IP leak prevention), `--disable-features=Autofill,AutofillServerCommunication`, `--disable-spell-checking`. macOS additionally appends `--allow-loopback-in-sandbox` and `--use-mock-keychain` unconditionally; the *detectable* dev flags (`in-process-gpu`, `disable-gpu-sandbox`, `disable-web-security`, `allow-running-insecure-content`) are **opt-in behind `HODOS_MAC_DEV_FLAGS=1`** because Cloudflare Turnstile rejects them.
- `SimpleApp::SetWindowHandles()` (Windows) / `SimpleApp::SetMacOSWindow()` (macOS) — stores platform window references
- `SimpleApp::OnContextInitialized()` — clears session-only SSL cert exceptions, creates the header browser, then either (a) shows the profile picker only, when `g_picker_mode` is set, or (b) restores the session from `session.json` — v2 multi-window (`windows[]` with per-window `tabs`, `activeTabIndex`, `x`/`y`/`width`/`height`) or v1 flat tab list — falling back to a fresh NTP tab.

**14 Overlay creation functions** (Windows only — the whole block is inside `#ifdef _WIN32`). Sizes are **logical px** passed through `ScalePx(n, g_hwnd)` for per-monitor DPI; full-window overlays take `GetWindowRect(g_hwnd)` verbatim:

| Function | Overlay | Size (logical) | Pattern |
|----------|---------|----------------|---------|
| `CreateSettingsOverlayWithSeparateProcess()` | Settings dropdown | 450 × 450 | Mouse hook close; clipboard + DOM paste enabled |
| `CreateWalletOverlay()` / `ShowWalletOverlay()` / `HideWalletOverlay()` | Wallet panel | 400 × (client height below header) | Keep-alive; prevent-close flag set on creation; clipboard + DOM paste |
| `CreateBackupOverlayWithSeparateProcess()` | Backup modal | full main window | Native file inputs; clipboard + DOM paste |
| `CreateBRC100AuthOverlayWithSeparateProcess()` | Auth dialog | full main window | BRC-100 challenge; clipboard + DOM paste |
| `CreateNotificationOverlay()` | Notifications / prompts | full main window | Keep-alive with JS injection (`window.showNotification()`); `?type=preload` warms the bundle |
| `CreateSettingsMenuOverlay()` | Settings menu | 200 × 120 | Toggle on repeat click |
| `CreateOmniboxOverlay()` / `Show` / `Hide` | Address bar dropdown | (header width − 2×152) × 350 | Keep-alive, lazy mouse hook |
| `CreateCookiePanelOverlay()` / `Show` / `Hide` | Privacy shield | 450 × 370 | Keep-alive, handler retarget |
| `CreateDownloadPanelOverlay()` / `Show` / `Hide` | Downloads | 380 × 400 | Keep-alive, handler retarget |
| `CreateSiteInfoPanelOverlay()` / `Show` / `Hide` | Site info hub | 360 × 480 | Keep-alive, anchored by **left** offset |
| `CreateTabListPanelOverlay()` / `Show` / `Hide` | Tab list / tab search | 340 × 480 | Keep-alive, anchored by **left** offset; clipboard + DOM paste (search input) |
| `CreateBookmarksPanelOverlay()` / `Show` / `Hide` | Bookmarks | 380 × 480 | Keep-alive, anchored by **left** offset; clipboard + DOM paste (search input) |
| `CreateMenuOverlay()` / `Show` / `Hide` | Hamburger menu | 280 × 450 | Keep-alive, handler retarget |
| `CreateProfilePanelOverlay()` / `Show` / `Hide` | Profile picker | 380 × 520 | Keep-alive, enables focus; clipboard + DOM paste (name edit) |

macOS overlay creation is in `cef_browser_shell_mac.mm` (14 `Create*Overlay*` functions there, plus a `CreateMenuOverlay` compat shim with the Windows signature). Names differ — the macOS side uses the `…MacOS` / `…WithSeparateProcess` suffixes, not the bare Windows names.

> Windows uses `CreateWalletOverlay()`; there is **no** `CreateWalletOverlayWithSeparateProcess()` on Windows — that name is macOS-only. Both call sites in `simple_handler.cpp` are `#ifdef`-split accordingly.

### SimpleRenderProcessHandler (simple_render_process_handler.cpp)

Runs in each renderer subprocess. Injects JavaScript APIs and routes IPC responses.

**5 V8 handler classes defined in this file:**

| Class | V8 Path | Methods |
|-------|---------|---------|
| `CefMessageSendHandler` | `window.cefMessage.send()` | Generic IPC dispatch — converts JS args to `CefProcessMessage` |
| `OverlayCloseHandler` | `window.hodosBrowser.overlay.close()` | Sends `overlay_close` IPC |
| `OmniboxCloseHandler` | `window.hodosBrowser.overlay.close()` (omnibox) | Sends `omnibox_hide` IPC |
| `HistoryV8Handler` | `window.hodosBrowser.history.*` | `get`, `search`, `searchWithFrecency`, `delete`, `clearAll`, `clearRange`, `test` (7) |
| `GoogleSuggestV8Handler` | `window.hodosBrowser.googleSuggest.fetch()` | Sends `google_suggest_request` IPC, returns request ID |

(`IdentityHandler` and the navigation/address handlers are bound here but declared in `src/core/` — see the Core Services doc.)

**1 static cache (with its own mutex):**
- `s_scriptCache` / `s_scriptCacheMutex` — URL → adblock scriptlet JS (one-shot, erased after injection)

The two fingerprint caches (`s_domainSeeds`/`s_seedMutex`, `s_fingerprintDisabledUrls`/`s_fpDisabledMutex`) were **deleted 2026-08-09** with the JS farbling path. ⛔ Do not reintroduce a renderer-side farbling cache: a per-URL map in this process is exactly what hid the shipped constant-seed bug, because a cross-process navigation left it empty in the *incoming* renderer while the browser happily computed a correct seed for the outgoing one.

**Per-process HistoryManager init**: the constructor only initializes `HistoryManager` when it is a real renderer subprocess (`--type=renderer`), and binds to the profile from `--profile=`. Running it in the browser process used to pre-open `Default` and leak it into every profile.

**Overlay readiness signal:** After V8 injection completes for overlay browsers, sets `window.allSystemsReady = true` and dispatches the `allSystemsReady` custom event.

### MyOverlayRenderHandler (my_overlay_render_handler.cpp/.mm)

Off-screen rendering for all overlay windows. One instance per overlay.

**6 CefRenderHandler methods (both platforms):** `GetViewRect`, `OnPaint`, `GetScreenPoint`, `GetScreenInfo`, `OnPopupShow` (stub), `OnPopupSize` (stub). macOS adds `DetachView()`.

**Windows:** `GetViewRect` converts the HWND's *physical* client rect to *logical* CSS px by dividing by `GetDpiForWindow(hwnd_)/96`. `GetScreenInfo` reports the real `device_scale_factor` so CEF renders at native monitor resolution. `GetScreenPoint` scales logical → physical before adding the window origin. `OnPaint` reallocates the DIB section when the overlay resizes and removes `WS_EX_TRANSPARENT` after first paint to enable mouse input.

**macOS (`OnPaint`):** `malloc` copies CEF buffer to prevent reuse ghosting. `CATransaction.setDisableActions:YES` disables Core Animation implicit transitions. `GetScreenPoint` converts from CEF top-left origin to macOS bottom-left screen coordinates.

## IPC Message Categories — browser process

**169 live message names** dispatched in `SimpleHandler::OnProcessMessageReceived()` (lines 1897–7416). Two additional names — `overlay_hide_NEVER_CALLED_12345` and `overlay_hide_NEVER_CALLED_67890` — are guarded by `if (false && …)` and are dead code; they are **not** counted below.

| Category | Messages | Count |
|----------|----------|-------|
| Tab management | `tab_create`, `tab_close`, `tab_switch`, `tab_reorder`, `tab_ghost_show`, `tab_ghost_hide`, `tab_tearoff`, `get_tab_list`, `get_recently_closed`, `reopen_recently_closed` | 10 |
| Navigation | `navigate`, `navigate_back`, `navigate_forward`, `navigate_reload`, `cert_error_proceed`, `cert_error_go_back` | 6 |
| Window chrome | `window_close`, `window_maximize`, `window_minimize`, `window_start_drag` | 4 |
| Omnibox | `omnibox_create`, `omnibox_create_or_show`, `omnibox_show`, `omnibox_hide`, `omnibox_update_query`, `omnibox_select`, `omnibox_autocomplete` | 7 |
| Overlay lifecycle | `overlay_show_wallet`, `overlay_show_settings`, `overlay_show_settings_menu`, `overlay_show_brc100_auth`, `overlay_show_backup`, `overlay_close`, `overlay_hide`, `overlay_input`, `toggle_wallet_panel` | 9 |
| Overlay panels | `cookie_panel_show/hide`, `profile_panel_show/hide`, `menu_show/hide/action`, `download_panel_show/hide`, `bookmarks_panel_show/hide`, `siteinfo_panel_show/hide/resize`, `tablist_panel_show/hide` | 16 |
| Wallet operations | `wallet_call` (Phase 2.5 IPC bridge), `wallet_status_check`, `create_wallet`, `get_wallet_info`, `load_wallet`, `get_balance`, `send_transaction`, `address_generate`, `get_addresses`, `get_all_addresses`, `get_current_address`, `mark_wallet_backed_up`, `wallet_prevent_close`, `wallet_allow_close`, `wallet_delete_cancel`, `wallet_payment_dismissed`, `get_backup_modal_state`, `set_backup_modal_state`, `open_wallet_permissions` | 19 |
| Transactions | `create_transaction`, `sign_transaction`, `broadcast_transaction`, `get_transaction_history` | 4 |
| Settings & profiles | `settings_get_all`, `settings_set`, `settings_update_all`, `settings_close`, `test_settings_message`, `profiles_get_all`, `profiles_create`, `profiles_rename`, `profiles_delete`, `profiles_switch`, `profiles_set_avatar`, `profiles_set_color`, `profiles_set_default` | 13 |
| Browser import | `import_detect_profiles`, `import_bookmarks`, `import_history`, `import_all` | 4 |
| Bookmarks | `bookmark_add`, `bookmark_get`, `bookmark_update`, `bookmark_remove`, `bookmark_search`, `bookmark_get_all`, `bookmark_is_bookmarked`, `bookmark_get_all_tags`, `bookmark_update_last_accessed`, `bookmark_folder_create/list/update/remove/get_tree` | 14 |
| Cookie management | `cookie_get_all`, `cookie_delete`, `cookie_delete_domain`, `cookie_delete_all`, `cache_clear`, `cache_get_size` | 6 |
| Cookie blocking | `cookie_block_domain`, `cookie_unblock_domain`, `cookie_get_blocklist`, `cookie_allow_third_party`, `cookie_remove_third_party_allow`, `cookie_get_block_log`, `cookie_clear_block_log`, `cookie_get_blocked_count`, `cookie_reset_blocked_count`, `cookie_check_site_allowed` | 10 |
| Ad blocking / cosmetic | `adblock_get_blocked_count`, `adblock_reset_blocked_count`, `adblock_site_toggle`, `adblock_check_site_enabled`, `adblock_scriptlet_toggle`, `adblock_check_scriptlets_enabled`, `cosmetic_class_id_query` | 7 |
| Fingerprint farbling | `fingerprint_get_site_enabled`, `fingerprint_set_site_enabled` | 2 |
| Downloads | `download_cancel`, `download_pause`, `download_resume`, `download_open`, `download_show_folder`, `download_clear_completed`, `download_get_state`, `download_browse_folder` | 8 |
| Find in page | `find_text`, `find_result_js`, `find_stop` | 3 |
| Browser UI | `print`, `devtools`, `zoom_in`, `zoom_out`, `zoom_reset`, `exit`, `force_repaint`, `check_for_updates` | 8 |
| BRC-100 / wallet permissions | `brc100_auth_response`, `add_domain_permission`, `add_domain_permission_advanced`, `approve_cert_fields`, `approve_identity_key_reveal`, `approve_key_linkage_reveal`, `grant_scoped_permission`, `permission_response`, `domain_permission_invalidate` | 9 |
| Site (Chromium) permissions | `site_permissions_get`, `site_permissions_set`, `site_permissions_reset` | 3 |
| Paid content cache | `paid_cache_clear`, `paid_cache_get_size` | 2 |
| QR scanning | `qr_scan_request`, `qr_found` | 2 |
| Search & analytics | `google_suggest_request`, `get_most_visited`, `get_session_blocked_total` | 3 |
| **Total** | | **169** |

## IPC Message Categories — render process

**95 message names** handled in `SimpleRenderProcessHandler::OnProcessMessageReceived()` (line 924 → EOF). Most are `*_response` / `*_error` replies to the browser-process messages above; the rest are pushes.

| Category | Messages | Count |
|----------|----------|-------|
| Wallet / identity replies | `wallet_response`, `wallet_response_chunk` (Phase 2.5 bridge), `wallet_status_check_response`, `create_wallet_response`, `create_identity_response`, `load_wallet_response`, `get_wallet_info_response`, `get_balance_response/_error`, `identity_status_check_response`, `mark_wallet_backed_up_response`, `mark_identity_backed_up_response`, `get_backup_modal_state_response`, `set_backup_modal_state_response`, `wallet_payment_dismissed` | 15 |
| Addresses | `address_generate_response/_error`, `get_addresses_response`, `get_all_addresses_response`, `get_current_address_response` | 5 |
| Transactions | `create_transaction_response/_error`, `sign_transaction_response/_error`, `broadcast_transaction_response/_error`, `send_transaction_response/_error`, `get_transaction_history_response/_error` | 10 |
| Bookmarks | `bookmark_add_response`, `bookmark_get_response`, `bookmark_get_all_response`, `bookmark_get_all_tags_response`, `bookmark_is_bookmarked_response`, `bookmark_remove_response`, `bookmark_search_response`, `bookmark_update_response`, `bookmark_update_last_accessed_response`, `bookmark_folder_create/get_tree/list/remove/update_response` | 14 |
| Cookies | `cookie_get_all_response`, `cookie_delete_response`, `cookie_delete_all_response`, `cookie_delete_domain_response`, `cookie_block_domain_response`, `cookie_unblock_domain_response`, `cookie_blocklist_response`, `cookie_allow_third_party_response`, `cookie_remove_third_party_allow_response`, `cookie_block_log_response`, `cookie_clear_block_log_response`, `cookie_blocked_count_response`, `cookie_reset_blocked_count_response`, `cookie_check_site_allowed_response`, `cache_clear_response`, `cache_get_size_response` | 16 |
| Ad blocking | `adblock_blocked_count_response`, `adblock_reset_blocked_count_response`, `adblock_site_toggle_response`, `adblock_check_site_enabled_response`, `adblock_scriptlet_toggle_response`, `adblock_check_scriptlets_enabled_response`, `session_blocked_total_response` | 7 |
| Cosmetic injection / fingerprint UI | `preload_cosmetic_script`, `inject_cosmetic_css`, `inject_cosmetic_script`, `fingerprint_get_site_enabled_response` | 4 |
| Tabs / navigation / UI | `tab_list_response`, `recently_closed_response`, `most_visited_response`, `focus_address_bar`, `find_show`, `find_result`, `settings_response`, `profiles_result`, `site_permissions_response` | 9 |
| Omnibox | `omnibox_query_update`, `omnibox_autocomplete_update`, `omnibox_select`, `google_suggest_response` | 4 |
| Downloads | `download_state_update`, `download_folder_selected` | 2 |
| Import | `import_complete`, `import_profiles_result` | 2 |
| BRC-100 / payment | `brc100_auth_request`, **`payment_success_indicator`** | 2 |
| Paid content cache | `paid_cache_clear_response`, `paid_cache_get_size_response` | 2 |
| QR scanning | `qr_scan_result`, `qr_screen_capture_result`, `qr_screen_capture_starting` | 3 |
| **Total** | | **95** |

> ⚠️ **`payment_success_indicator` drives the GOLD PILL** on the tab — the user's primary visual safeguard against silent payment abuse. It fires on every auto-approved payment. Never call it a "green dot"; never let a refactor drop this route.

## V8 API Shape

Injected in `SimpleRenderProcessHandler::OnContextCreated()`. Injection is **gated by page class**, not uniform:

```
window.hodosBrowser                      // ALL pages (internal + external)
├── platform                             // "windows" | "macos" — always injected
│
├── ── internal pages (127.0.0.1:5137) and overlays only ──
├── identity.get()
├── identity.markBackedUp()
├── navigation.navigate(url)
├── address.generate()
├── address.getAll()                     // macOS branch only
├── address.getCurrent()                 // macOS branch only
├── history.get() / search() / searchWithFrecency() / delete()
│        / clearAll() / clearRange() / test()
├── overlay.close()                      // overlay browsers only
│                                        //   omnibox → OmniboxCloseHandler (omnibox_hide)
│                                        //   all others → OverlayCloseHandler (overlay_close)
└── googleSuggest.fetch(query)           // omnibox overlay only

window.cefMessage.send(name, ...args)    // ALL pages (internal + external)
window.allSystemsReady                   // set true after V8 injection (overlays only)

window.chrome                            // external pages only — bot-detection stub
                                         //   (runtime/loadTimes/csi), injected only if
                                         //   window.chrome is undefined
window.__hodos_walletCall(...)           // WALLET_CALL_BRIDGE_SCRIPT — internal pages,
                                         //   overlays, AND qualifying external pages
window.CWI / window.yours / window.panda // CWI_SHIM_SCRIPT — external pages only,
                                         //   main frame only, https:// only
```

> The legacy `window.hodosBrowser.brc100.*` bindings (`BRC100Handler` / `BRC100Bridge`) were **removed** — they did synchronous WinHTTP on the render thread and their only caller was a startup probe that just logged. The live BRC-100 surfaces are the `window.CWI` shim and the Phase 2.5 `wallet_call` IPC bridge.

**CWI shim gating cascade** (each rejection logged separately): external page → main frame only (no iframes) → `https://` only. A future private/incognito mode must also be gated here (Brave posture: no provider injection in private windows at all).

## Injection Pipeline

Content injected into page contexts by `OnContextCreated()`, in order:

1. **Adblock scriptlets** — pre-cached via `preload_cosmetic_script` IPC in `OnBeforeBrowse`, injected from `s_scriptCache`. One-shot per URL (erased after injection so subframe contexts don't re-inject). Skipped for `127.0.0.1` URLs.
2. ~~**Fingerprint protection**~~ — **REMOVED 2026-08-09.** Farbling is native in Blink (fork patches C1/C3/C4/C5/C6) and is applied at API-call time, so the renderer injects nothing and holds no farbling state. libcef pulls the per-origin key at `OnContextCreated` and installs it on `HodosSessionCache`; the shell's contribution is the single `enabled` bit it sends with `hodos_farble_key` from `OnBeforeBrowse`. ⛔ Do not re-add an injection step here — it cannot cover workers, it restores the `toString` tamper tell, and it would double-perturb what Blink already farbles.
3. **`window.chrome` stub** — external pages only, injected independently of the fingerprint script so it works even with farbling disabled.
4. **Wallet IPC bridge** (`WALLET_CALL_BRIDGE_SCRIPT`) — idempotent IIFE; internal pages + overlays always, external pages only when the CWI shim also qualifies.
5. **CWI/yours/panda provider shim** (`CWI_SHIM_SCRIPT`) — external https main frames, injected *after* the bridge (the provider's methods call `window.__hodos_walletCall`).
6. **Cosmetic CSS/scripts** — injected post-load via `inject_cosmetic_css` / `inject_cosmetic_script` IPC. CSS creates `<style id="hodos-cosmetic-css">` with `display: none !important` rules.

## Keyboard Shortcuts

Handled in `SimpleHandler::OnPreKeyEvent()`. CEF reports Windows virtual key codes on **both** platforms.

| Shortcut | macOS | Action |
|----------|-------|--------|
| Ctrl+D | Cmd+D | Toggle bookmark on current page (adds, or removes if already bookmarked) |
| Ctrl+F | Cmd+F | Find in page — **tab browsers only**; sends `find_show` to header and moves CEF focus there |
| Ctrl+H | Cmd+H | Open `/browser-data` in a new tab |
| Ctrl+J | Cmd+J | Show downloads panel (macOS toggles it) |
| Ctrl+L | Cmd+L | Focus address bar (`focus_address_bar` → header) |
| Ctrl+N | Cmd+N | New window (`WindowManager::CreateFullWindow()`) |
| Ctrl+P | Cmd+P | Print active tab |
| Ctrl+T | Cmd+T | New tab |
| Ctrl+W | Cmd+W | Close active tab; auto-creates an NTP if the window would be left empty |
| Ctrl+Shift+A | Cmd+Shift+A | Toggle tab-list / tab-search overlay (Shift required so plain Ctrl+A select-all passes through) |
| Ctrl+Shift+I | Cmd+Option+I | DevTools |
| F12 | F12 | DevTools |
| Alt+Left | Alt+Left | Navigate back (active tab, if `can_go_back`) |
| Alt+Right | Alt+Right | Navigate forward (active tab, if `can_go_forward`) |
| — | Cmd+, | Open `/settings-page/general` in a new tab (macOS only) |

**Zoom suppression:** on any browser whose role is not `tab_*` (header, all overlays), Ctrl/Cmd + `=` / `-` / `0` / numpad `+` / numpad `-` is consumed so browser chrome stays at a fixed size. Only web content zooms.

> There is **no** Ctrl+Shift+Delete handler — clear-browsing-data is reached through Settings, not a shortcut.

## Context Menu

Custom context menu replaces Chromium defaults. 20 command IDs, all offsets from `MENU_ID_USER_FIRST` (26500), defined near `SimpleHandler::OnBeforeContextMenu`:

- **Non-tab browsers** (header, overlays): only Cut/Copy/Paste/Select All when `CM_TYPEFLAG_EDITABLE`, plus Inspect Element.
- **Link context**: Open Link in New Tab, Open Link in New Window, Copy Link Address
- **Image context**: Save Image As…, Copy Image Address, Open Image in New Tab
- **Editable context**: Undo, Redo, Cut, Copy, Paste, Delete, Select All
- **Selection context** (non-editable): Copy, Select All
- **Plain page context**: Back, Forward, Reload, Select All, View Page Source, Set as Home Page — Back/Forward enabled per navigation state
- **All tab contexts**: **Manage Wallet Permissions** (`MENU_ID_MANAGE_PERMISSIONS` — the quick-revoke flow), then Inspect Element at the bottom

On macOS the model is popped up manually as an `NSMenu` by `simple_handler_mac.mm :: RunContextMenu`.

## HTTP Request Interception

`SimpleHandler::GetResourceRequestHandler()` runs on the IO thread for every request, in this order:

1. **Production frontend serving** — `127.0.0.1:5137` URLs when a `frontend/` dir sits next to the exe → `LocalFileResourceRequestHandler`. Must run first; later handlers would try to network-fetch port 5137, which has no server in production.
2. **Trusted overlay wallet bypass** — `hodos::IsWalletHostPort(url)` from roles `wallet` / `wallet_panel` / `settings` / `backup` → `nullptr` (native CEF handling; avoids `CefURLRequest` forwarding issues on macOS).
3. **Paid content cache read hook** (BRC-121) — GET, non-localhost, cache enabled, and not a hard reload (`Cache-Control`/`Pragma: no-cache` from Ctrl+Shift+R) → `CachedContentRequestHandler` serving bytes from SQLite. Short-circuits the entire 402 chain; no payment IPC, no session state change.
4. **DNT/GPC headers** — injects `DNT: 1` and `Sec-GPC: 1` when the privacy setting is on.
5. **Ad & tracker blocking** — global toggle + per-site toggle; `AdblockCache::checkCacheOnly()` → `AdblockBlockHandler` on a cached block, `DeferredAdblockHandler` on a cache miss (defers to a background thread so the IO thread stays free). A cached *allow* falls through.
6. **Wallet routing** — `hodos::IsWalletHostPort(url)`, or `localhost:3321` / `localhost:2121` / `localhost:8080`, or `messagebox.babbage.systems`, or `/.well-known/auth` → `HttpRequestInterceptor`.
7. **Everything else http/https** → `CookieFilterResourceHandler`. This is **not** optional: it applies cookie blocking + YouTube ad-response filtering *and* runs BRC-121 402 detection in `OnResourceResponse`. Returning `nullptr` here would mean no response callback fires and 402 challenges from arbitrary sites would never trigger the pay flow.

> Ports are never literals. `hodos::IsWalletHostPort()` from `include/core/PortConfig.h` resolves 31301 (release) vs 31401 (`HODOS_DEV=1`), and checks both `localhost:` and `127.0.0.1:` host forms.

## Overlay Patterns

All overlays use off-screen rendering (OSR) with `MyOverlayRenderHandler`. Common creation pattern:

```cpp
CefWindowInfo window_info;
window_info.windowless_rendering_enabled = true;
window_info.SetAsPopup(overlay_hwnd, "RoleName");

CefBrowserSettings settings;
settings.windowless_frame_rate = 30;
settings.background_color = CefColorSetARGB(0, 0, 0, 0);  // transparent
settings.javascript = STATE_ENABLED;
// Only on overlays with text input (wallet, settings, backup, brc100 auth,
// notification, bookmarks, tablist, profile):
settings.javascript_access_clipboard = STATE_ENABLED;
settings.javascript_dom_paste = STATE_ENABLED;

// All overlays share the global request context (shared cache/cookies).
CefBrowserHost::CreateBrowser(window_info, handler, url, settings, nullptr,
                              CefRequestContext::GetGlobalContext());
```

**Keep-alive overlays** (Wallet, Omnibox, Cookie, Download, SiteInfo, TabList, Bookmarks, Menu, Profile): created once, shown/hidden via `ShowWindow(hwnd, SW_SHOW/SW_HIDE)`. Mouse hook installed lazily on show, removed on hide; `WS_EX_TRANSPARENT` cleared on show so clicks land.

**Full-window overlays** (Backup, BRC-100 Auth, Notification): sized from `GetWindowRect(g_hwnd)`, cover the entire main window.

**Wallet overlay**: sets `g_wallet_overlay_prevent_close = true` at creation (synchronous C++, no IPC race). Show is suppressed if the overlay was hidden < 200 ms ago — that hide/show race happens because the toolbar click that closes it also steals focus.

**Notification overlay**: unique keep-alive with JS injection — a `?type=preload` call loads the React bundle in an idle state, and subsequent calls invoke `window.showNotification(query)` for instant state update with no page navigation (falls back to `window.location.search` if the hook isn't present).

## Platform Differences

| Aspect | Windows | macOS |
|--------|---------|-------|
| Overlay creation | 14 functions in `simple_app.cpp` (HWND + GDI) | 14 functions in `cef_browser_shell_mac.mm` (NSWindow + Core Animation) |
| Context menu presentation | CEF presents the `CefMenuModel` natively | Manual `CefMenuModel` → `NSMenu` popup in `simple_handler_mac.mm` |
| OSR rendering | `UpdateLayeredWindow` + `BLENDFUNCTION` | `CALayer.contents` + `CGImageCreate` |
| Buffer handling | Direct `dib_data_` pointer | `malloc` copy (prevents CEF buffer reuse ghosting) |
| Resize handling | Dynamic `CreateDIBSection` realloc in `OnPaint` | Queries `NSView.bounds` in `GetViewRect` |
| Clipboard | `OpenClipboard` / `SetClipboardData` | `popen("pbcopy")` pipe |
| DPI scaling | `GetDpiForWindow()/96` reported as `device_scale_factor`; overlay sizes via `ScalePx()` | `NSScreen.backingScaleFactor` (Retina) |
| Screen coords | Logical → physical scale, then window origin | Top-left → bottom-left origin conversion |
| Tab tearoff | Ghost tab window (GDI `WS_POPUP` + `GhostTabWndProc`) | `ShowGhostTabMacOS` in `src/core/WindowManager_mac.mm` |
| Dev Chromium flags | none | `--allow-loopback-in-sandbox` + `--use-mock-keychain` always; the detectable ones behind `HODOS_MAC_DEV_FLAGS=1` |
| `HistoryV8Handler` | Full implementation via `HistoryManager` | V8 handler registered but `HistoryManager` not initialized in render process |
| `GoogleSuggestService` | WinHTTP to Google/DuckDuckGo | libcurl to Google/DuckDuckGo |

## Related

- [Parent: CEF Native Shell](../../CLAUDE.md) — build instructions, HWND hierarchy, focus management, port config
- [Sibling: Handler headers](../../include/handlers/CLAUDE.md) — class declarations for the four handler types
- [Sibling: Core Services](../core/CLAUDE.md) — singletons (TabManager, HistoryManager, SettingsManager, HttpRequestInterceptor, PaidContentCache, etc.) used by these handlers
- [Root: Project](../../../CLAUDE.md) — architecture overview, overlay lifecycle rules, CEF input patterns
