# CEF Core Services
> Singleton managers, V8 handlers, and HTTP clients that power the browser shell's non-UI logic.

## Overview

This module contains the core C++ business logic for the CEF browser shell. It provides data persistence (history, bookmarks, cookies, settings, profiles), wallet/BRC-100 communication with the Rust backend, tab/window lifecycle management, and V8 JavaScript handler classes that expose `window.hodosBrowser.*` APIs to the render process. Most classes are singletons initialized at startup and accessed from the browser process UI thread.

All files are cross-platform (Windows + macOS) unless noted. Windows uses WinHTTP; macOS uses libcurl. Platform-specific code is gated with `#ifdef _WIN32` / `#elif defined(__APPLE__)`.

## Files

| File | Purpose |
|------|---------|
| `HttpRequestInterceptor.cpp` | **Largest file (~91KB).** HTTP request routing and auto-approve engine for wallet API calls. Contains `DomainPermissionCache`, `BSVPriceCache`, `WalletStatusCache` (all singletons), and `AsyncWalletResourceHandler`. Intercepts `localhost:31301` requests, checks domain permissions, enforces per-tx/per-session/rate limits, and routes to Rust wallet. Also handles BRC-100 auth, domain permission, payment, and certificate approval flows via `PendingRequestManager`. |
| `BookmarkManager.cpp` | Bookmark CRUD with SQLite. Singleton (`GetInstance()`). Tables: `bookmarks`, `bookmark_folders`, `bookmark_tags`. Methods: `AddBookmark`, `CreateFolder`, `GetAllBookmarks`, `SearchBookmarks`, `DeleteBookmark`, `MoveBookmark`, etc. Initialized with profile-specific `user_data_path`. |
| `CookieBlockManager.cpp` | Third-party cookie blocking engine. Singleton. SQLite database (`cookie_blocks.db`) with tracker domain list, custom rules, and block log. Pre-populated from `DefaultTrackerList.h` on first run. Integrates with `EphemeralCookieManager` and `SettingsManager` for privacy settings. |
| `CookieManager.cpp` | CEF cookie CRUD via `CefCookieManager`. Static methods: `HandleGetAllCookies`, `HandleDeleteCookie`, `HandleDeleteDomainCookies`, `HandleDeleteAllCookies`, `HandleClearCache`, `HandleGetCacheSize`. Uses `CookieCollector` (CefCookieVisitor) on IO thread, posts results to UI thread via `SendResponseTask`. |
| `EphemeralCookieManager.cpp` | Ephemeral (session-only) cookie cleanup. Singleton with `shared_mutex` for thread safety. Tracks per-site tab reference counts; when last tab navigates away, starts 30-second grace period (`GraceExpiredTask` via `CefPostDelayedTask`), then deletes third-party cookies for that site. |
| `HistoryManager.cpp` | Browser history SQLite database. Singleton. Chromium-compatible schema (`urls` + `visits` tables). Methods: `AddVisit` (with debounce), `GetHistory`, `SearchHistory`, `GetTopSites`, `SearchHistoryWithFrecency` (SQL frecency scoring + post-query domain boost), `DeleteHistoryEntry`, `DeleteAllHistory`, `DeleteHistoryRange`. Uses Chromium epoch timestamps (microseconds since 1601). |
| `TabManager.cpp` | Tab lifecycle for Windows. Singleton. Creates HWND-per-tab with windowed CEF browsers parented to shell window. Methods: `CreateTab`, `CloseTab`, `SwitchToTab`, `ReorderTabs`, `MoveTabToWindow` (tab tear-off/merge). Tracks per-window active tabs via `active_tab_per_window_`. Auto-closes windows when last tab closes. |
| `TabManager_mac.mm` | macOS tab lifecycle using NSView instead of HWND. Same API as Windows `TabManager.cpp`. Uses `[view setHidden:]` for show/hide, `[view removeFromSuperview]` for cleanup. Synchronous view removal on close (differs from Windows async pattern). |
| `WindowManager.cpp` | Multi-window management (cross-platform core + Windows `CreateFullWindow`). Singleton. `CreateWindowRecord()` assigns integer IDs. `CreateFullWindow()` creates shell window + header CEF browser + initial NTP tab. `GetWindowByHwnd()` (Windows) / `GetWindowByNSWindow()` (macOS), `GetWindowForBrowser()`, `GetAllWindows()`, `GetWindowCount()` for lookups. Forces layout refresh on existing windows when new window created. |
| `WindowManager_mac.mm` | macOS `CreateFullWindow()` implementation. Creates NSWindow with header NSView (99px) + webview NSView, `BrowserWindowDelegate` for resize/move/close, per-window delegate lifecycle via `objc_setAssociatedObject`. Also provides `GetWindowAtScreenPointMacOS()` (hit-test for tab merge), `PositionWindowAtScreenPoint()` (tab tear-off), and ghost tab preview window (`ShowGhostTabMacOS`/`HideGhostTabMacOS`) with 60fps cursor-following timer. |
| `BrowserWindow.cpp` | Struct mapping string roles to `CefRefPtr<CefBrowser>` references. 15 browser role slots (header, webview, wallet_panel, overlay, settings, wallet, backup, brc100auth, notification, settings_menu, omnibox, cookiepanel, downloadpanel, profilepanel, menu). Methods: `SetBrowserForRole()`, `GetBrowserForRole()`, `ClearBrowserForRole()`. macOS fields: `ns_window`, `header_view`, `webview_view` (`void*` pointers to NSWindow/NSView). Used by `WindowManager`. |
| `WalletService.cpp` | Windows wallet HTTP client. Connects to Rust wallet at `localhost:31301` via WinHTTP. Methods: `getWalletStatus`, `createWallet`, `loadWallet`, `getBalance`, `sendTransaction`, `generateAddress`, etc. Also manages daemon process lifecycle (`startDaemon`, `stopDaemon`, `monitorDaemon`). |
| `WalletService_mac.cpp` | macOS wallet HTTP client using libcurl. Same API as Windows `WalletService`. No daemon management (developer runs Rust wallet manually). |
| `BRC100Bridge.cpp` | BRC-100 protocol HTTP bridge to Rust wallet. Windows uses WinHTTP, macOS uses libcurl. 15 API methods: `getStatus`, `isAvailable`, `generateIdentity`, `validateIdentity`, `createSelectiveDisclosure`, `generateChallenge`, `authenticate`, `deriveType42Keys`, `createSession`, `validateSession`, `revokeSession`, `createBEEF`, `verifyBEEF`, `broadcastBEEF`, `verifySPV`, `createSPVProof`. WebSocket stubs (placeholder). |
| `BRC100Handler.cpp` | V8 handler for `window.hodosBrowser.brc100.*`. Implements `CefV8Handler::Execute()` dispatching 16 methods. `RegisterBRC100API()` wires up the V8 object tree. Contains `V8ValueToJSON()` and `JSONToV8Value()` conversion helpers. Delegates all HTTP calls to `BRC100Bridge`. |
| `AddressHandler.cpp` | V8 handler for address generation. `Execute()` handles `"generate"`, `"getAll"`, `"getCurrent"`. Overlay browsers use direct V8 return; main browser uses IPC process messages with promise-like objects. |
| `IdentityHandler.cpp` | V8 handler for `identity.get()` and `identity.markBackedUp()`. Checks local `identity.json` file first, falls back to Rust wallet daemon. Platform-aware file paths. |
| `NavigationHandler.cpp` | V8 handler for `window.hodosBrowser.navigation.navigate()`. Handles `hodos://` custom protocol by rewriting to `http://127.0.0.1:5137/`. Sends `"navigate"` IPC message to browser process. |
| `SessionManager.cpp` | Per-browser-tab session spending/rate tracking for auto-approve. Tracks `spentCents`, `paymentRequestsThisMinute` per browser ID + domain. Rate limit uses 60-second sliding window. Used by `HttpRequestInterceptor` for payment auto-approve decisions. |
| `SettingsManager.cpp` | Persistent settings with profile support. Singleton. Three setting groups: `BrowserSettings` (homepage, search engine, zoom, downloads path), `PrivacySettings` (adblock, cookie blocking, DNT, fingerprint protection), `WalletSettings` (auto-approve limits, PeerPay). JSON file at `<profile>/settings.json`. Migrates from global to per-profile on first use. |
| `ProfileManager.cpp` | Multi-profile management. Singleton. Stores profiles in `profiles.json` at app data root. Methods: `CreateProfile`, `DeleteProfile`, `RenameProfile`, `SetProfileColor`, `LaunchWithProfile` (Windows: spawns new process with `--profile=` argument). Profile IDs: `"Default"` or `"Profile_N"`. |
| `ProfileImporter.cpp` | Import bookmarks and history from Chrome, Brave, and Edge. `DetectProfiles()` scans standard browser profile paths. `ImportBookmarks()` parses Chromium JSON bookmark format recursively. `ImportHistory()` copies and reads Chromium History SQLite database (handles WAL files). Writes to `BookmarkManager` and `HistoryManager`. |
| `ProfileLock.cpp` | Single-instance profile lock. Windows: `CreateFileA` with `FILE_FLAG_DELETE_ON_CLOSE` for exclusive access. macOS/Linux: `flock()` with `LOCK_EX | LOCK_NB`. Prevents multiple browser instances from using same profile. |
| `GoogleSuggestService.cpp` | Omnibox search suggestions. Singleton. Fetches from Google Suggest API or DuckDuckGo `/ac/` endpoint via HTTPS (WinHTTP on Windows). Parses JSON response arrays. URL-encodes query. 5-second timeout. Windows-only currently (macOS returns empty). |
| `SyncHttpClient.cpp` | Cross-platform synchronous HTTP client. Static `Get()` and `Post()` methods. Windows: WinHTTP with configurable timeout. macOS: libcurl with `CURLOPT_TIMEOUT_MS`. Returns `HttpResponse{body, statusCode, success}`. Used by `DomainPermissionCache` and other singletons that need blocking HTTP on non-UI threads. |
| `Logger.cpp` | File + stdout logger. Static class with `Initialize()`, `Log()`, `Shutdown()`. Supports `ProcessType` (MAIN, RENDER, etc.) and `LogLevel` (DEBUG=0, INFO=1, WARNING=2, ERROR=3). Output format: `[timestamp] [process] [level] message`. |
| `DefaultTrackerList.h` | Static list of 24 default tracker domains (Google Analytics, Facebook, Criteo, etc.) used by `CookieBlockManager` on first-run initialization. |

## Singleton Pattern

Most managers use Meyer's singleton:
```cpp
SettingsManager& SettingsManager::GetInstance() {
    static SettingsManager instance;
    return instance;
}
```

**Meyer's singletons (static local):** `HistoryManager`, `BookmarkManager`, `CookieBlockManager`, `EphemeralCookieManager`, `SettingsManager`, `ProfileManager`, `WindowManager`, `SessionManager`, `GoogleSuggestService`, `DomainPermissionCache`, `BSVPriceCache`, `WalletStatusCache`.

**`unique_ptr` singleton:** `TabManager` — uses `std::unique_ptr<TabManager>` with lazy init in `GetInstance()` (not Meyer's pattern).

**Non-singletons:** `WalletService` (instantiated per-use), `BRC100Bridge` (owned by `BRC100Handler`), `BrowserWindow` (one per window, owned by `WindowManager`).

## Threading Model

| Thread | Components |
|--------|-----------|
| **UI thread** | `TabManager`, `WindowManager`, `CookieManager` handlers, IPC dispatch, `EphemeralCookieManager::OnTabNavigated/OnTabClosed` |
| **IO thread** | `CookieCollector::Visit()`, `EphemeralCookieManager::OnGraceExpired`, `CookieBlockManager` cookie checking |
| **FILE thread** | `CacheSizeTask`, `LogBlockedCookieTask` (SQLite writes that shouldn't block IO) |
| **Render thread** | V8 handlers (`AddressHandler`, `BRC100Handler`, `IdentityHandler`, `NavigationHandler`) |
| **Any thread** | `SyncHttpClient`, `Logger::Log()`, `DomainPermissionCache` (mutex-protected) |

Cross-thread communication uses `CefPostTask(TID_*)` and `SendResponseTask` to safely post results back to UI thread for IPC dispatch.

## Key Architectural Patterns

### V8 Handler Pattern (Render Process)
V8 handlers implement `CefV8Handler::Execute()` and are registered in `OnContextCreated()`:
```
React calls window.hodosBrowser.brc100.authenticate({...})
  → BRC100Handler::Execute("authenticate", args)
    → BRC100Bridge::authenticate(json)
      → HTTP POST to localhost:31301/brc100/auth/authenticate
    → JSONToV8Value(response) → retval
```

### HTTP Interception Pattern (Browser Process)
`HttpRequestInterceptor` sits in CEF's `OnBeforeResourceLoad` pipeline:
```
Page requests http://localhost:31301/createAction
  → isWalletEndpoint() matches
  → DomainPermissionCache.getPermission(domain)
  → Auto-approve check (limits, rate, session spending)
  → If approved: AsyncWalletResourceHandler forwards to Rust wallet
  → If needs approval: PendingRequestManager queues, shows overlay
```

### Database Initialization Pattern
Data managers follow a consistent pattern:
```cpp
manager.Initialize(user_data_path);  // Opens/creates SQLite DB, runs schema migrations
// ... use throughout app lifetime ...
// Destructor calls CloseDatabase()
```

Storage paths:
- Windows: `%APPDATA%/HodosBrowser/<Profile>/`
- macOS: `~/Library/Application Support/HodosBrowser/<Profile>/`

## Cross-Platform Notes

| Component | Windows | macOS |
|-----------|---------|-------|
| `WalletService` | `WalletService.cpp` (WinHTTP) | `WalletService_mac.cpp` (libcurl) |
| `BRC100Bridge` | WinHTTP in same file | libcurl in same file (`#elif __APPLE__`) |
| `SyncHttpClient` | WinHTTP | libcurl |
| `TabManager` | `TabManager.cpp` (HWND) | `TabManager_mac.mm` (NSView) |
| `WindowManager::CreateFullWindow` | `WindowManager.cpp` (HWND + WM_SIZE) | `WindowManager_mac.mm` (NSWindow + BrowserWindowDelegate) |
| `GoogleSuggestService` | WinHTTP | Not implemented (returns empty) |
| `ProfileLock` | `CreateFileA` exclusive lock | `flock()` |

When adding new HTTP-calling singletons, use `SyncHttpClient` (already cross-platform) rather than raw WinHTTP.

## Related

- **Headers:** `cef-native/include/core/` — all class declarations, struct definitions, and inline header-only classes (`FingerprintProtection.h`, `FingerprintScript.h`, `AdblockCache.h`, `PendingAuthRequest.h`)
- **Parent CLAUDE.md:** `cef-native/CLAUDE.md` — build instructions, HWND hierarchy, IPC flow, window architecture
- **Root CLAUDE.md:** `/CLAUDE.md` — full architecture overview, key files table, overlay lifecycle, CEF input patterns
- **Handler files:** `cef-native/src/handlers/` — `simple_handler.cpp` (IPC dispatch that calls these core managers), `simple_render_process_handler.cpp` (V8 injection that registers these handlers)
