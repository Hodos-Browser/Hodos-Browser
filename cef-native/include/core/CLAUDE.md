# CEF Core Headers
> Header-only and declaration files for all C++ singletons, managers, and services in the browser shell.

## Overview

This directory contains the header files for the CEF native layer's core subsystems. These define the singletons, data structures, and class interfaces that implement browser data management (history, bookmarks, cookies), privacy features (ad blocking, fingerprint protection, cookie blocking), wallet integration (BRC-100, identity, HTTP interception), window/tab management, and cross-cutting concerns (logging, settings, profiles). Most classes follow the singleton pattern with `GetInstance()` and are thread-safe via `std::mutex` or `std::shared_mutex`.

## Files

| File | Purpose |
|------|---------|
| `AdblockCache.h` | `AdblockCache` singleton: URL block result cache, per-browser blocked counts, per-site/domain toggle persistence (`adblock_settings.json`), cosmetic filter fetching. Also defines `AdblockBlockHandler` (cancels blocked requests) and `CefResourceTypeToAdblock()` mapping. Windows uses WinHTTP; macOS uses `SyncHttpClient`. |
| `AddressHandler.h` | `AddressHandler` — CefV8Handler for `window.hodosBrowser.generateAddress()` in render process |
| `BookmarkManager.h` | `BookmarkManager` singleton: SQLite-backed bookmark/folder CRUD with tags, search, and folder tree (max depth 3). Structs: `BookmarkData`, `FolderData` |
| `BRC100Bridge.h` | `BRC100Bridge` — HTTP client to Rust wallet (localhost:31301) for BRC-100 protocol operations: identity, authentication, sessions, BEEF transactions, SPV proofs, and WebSocket support |
| `BRC100Handler.h` | `BRC100Handler` — CefV8Handler that exposes BRC-100 API to JavaScript via V8 injection. Delegates to `BRC100Bridge`. Static `RegisterBRC100API()` for context setup |
| `BrowserWindow.h` | `BrowserWindow` — per-window state container: 11 overlay HWNDs (Windows) or NSView pointers (macOS), 6 mouse hooks, 6 icon offsets, 15 CefBrowser refs. Role-based accessors via `SetBrowserForRole()`/`GetBrowserForRole()` |
| `CookieBlockManager.h` | `CookieBlockManager` singleton: SQLite-backed domain blocklist + third-party allowlist, in-memory `O(1)` IO-thread lookups via `shared_mutex`, per-browser blocked counts, async block logging. Also defines `CookieAccessFilterWrapper` (refcounted CEF adapter) |
| `CookieManager.h` | `CookieManager` — static-only class for CEF cookie/cache operations: enumerate, delete single/domain/all cookies, clear cache, get cache size. Called from browser process UI thread |
| `EphemeralCookieManager.h` | `EphemeralCookieManager` singleton: Brave-style ephemeral third-party cookies. Tracks open tabs per eTLD+1 site; 30-second grace period on last tab close before deleting third-party cookies. Struct: `SiteSession` |
| `FingerprintProtection.h` | `FingerprintProtection` singleton: per-session 32-byte CSPRNG token (Windows CryptGenRandom / macOS SecRandomCopyBytes), per-domain seed generation via hash mixing, auth domain exemption list (`IsAuthDomain()`) |
| `FingerprintScript.h` | `FINGERPRINT_PROTECTION_SCRIPT` — embedded JS constant injected via `OnContextCreated`. Mulberry32 PRNG seeded per-domain; farbles Canvas (`getImageData`, `toDataURL`, `toBlob`), WebGL (`getParameter`, `readPixels`), Navigator (`hardwareConcurrency`, `deviceMemory`, `plugins`), AudioContext (`getChannelData`, `getFloatFrequencyData`) |
| `GoogleSuggestService.h` | `GoogleSuggestService` singleton: fetches search suggestions from Google or DuckDuckGo for the omnibox |
| `HistoryManager.h` | `HistoryManager` singleton: SQLite-backed browsing history with visit counting, frecency-scored search, top sites, time-range deletion, and 2-second URL debouncing. Structs: `HistoryEntry`, `HistorySearchParams`, `HistoryEntryWithScore` |
| `HttpRequestInterceptor.h` | `HttpRequestInterceptor` — `CefResourceRequestHandler` that intercepts HTTP requests for wallet endpoint routing, cookie access filtering, and BRC-100 auth modal flow. Free functions: `sendAuthRequestDataToOverlay()`, `handleAuthResponse()` |
| `IdentityHandler.h` | `IdentityHandler` — CefV8Handler for identity operations in render process. Free function: `jsonToV8()` for JSON-to-V8 conversion |
| `Logger.h` | `Logger` — centralized file logger with `LogLevel` (DEBUG/INFO/WARNING/ERROR) and `ProcessType` (MAIN/RENDER/BROWSER). Header-only for cross-compilation. Output to `debug_output.log` |
| `NavigationHandler.h` | `NavigationHandler` — CefV8Handler for navigation commands (back, forward, reload, navigate) in render process |
| `PendingAuthRequest.h` | `PendingRequestManager` singleton: thread-safe map of pending auth/domain/payment/certificate approval requests. Struct: `PendingAuthRequest` with 6 request types. Supports per-domain queuing and batch resolution |
| `ProfileImporter.h` | `ProfileImporter` — static utility for detecting and importing bookmarks/history from Chrome, Brave, Edge, and Firefox. Structs: `DetectedProfile`, `ImportResult`. Progress callback support |
| `ProfileLock.h` | `AcquireProfileLock()` / `ReleaseProfileLock()` — exclusive file lock on profile directory to prevent SQLite corruption from concurrent instances |
| `ProfileManager.h` | `ProfileManager` singleton: multi-profile support with CRUD, color/avatar customization, startup picker toggle, and cross-instance launch (`LaunchWithProfile()`). Struct: `ProfileInfo` |
| `SessionManager.h` | `SessionManager` singleton: per-browser spending/rate tracking for BRC-100 auto-approve engine. Tracks USD cents spent per session and payment requests per minute. Struct: `BrowserSession` |
| `SettingsManager.h` | `SettingsManager` singleton: JSON-persisted settings with three categories (`BrowserSettings`, `PrivacySettings`, `WalletSettings`). Thread-safe getters/setters with auto-save. nlohmann JSON serialization macros |
| `SyncHttpClient.h` | `SyncHttpClient` — cross-platform synchronous HTTP client (WinHTTP on Windows, libcurl on macOS). Static `Get()` and `Post()` methods. Struct: `HttpResponse`. Used by `AdblockCache` and other singletons for localhost backend calls |
| `Tab.h` | `Tab` struct: per-tab state including browser ref, HWND/NSView, URL, title, favicon, loading/navigation/SSL state, timestamps. Cross-platform with `#ifdef _WIN32` / `void*` |
| `TabManager.h` | `TabManager` singleton: tab lifecycle (create/close/switch), browser registration, state updates from SimpleHandler callbacks, tab reordering, cross-window tab moves (`MoveTabToWindow()`), per-window active tab tracking |
| `WalletService.h` | `WalletService` — HTTP client to Rust wallet backend (localhost:31301): health check, wallet CRUD, address management, transaction lifecycle, daemon process management (start/stop/monitor) |
| `WindowManager.h` | `WindowManager` singleton: manages `BrowserWindow` instances. Window 0 is main window; supports multi-window via `CreateWindowRecord()` / `CreateFullWindow()`. Lookups by window ID, HWND, or browser ID |

## Architecture Patterns

### Singleton Pattern

Most managers use Meyer's singleton (`static T instance` in `GetInstance()`):
- `AdblockCache`, `FingerprintProtection`, `PendingRequestManager`, `SessionManager` — inline `GetInstance()` in header
- `BookmarkManager`, `CookieBlockManager`, `HistoryManager`, `SettingsManager`, `ProfileManager`, `WindowManager`, `TabManager`, `EphemeralCookieManager`, `GoogleSuggestService` — `GetInstance()` defined in `.cpp`

All singletons delete copy constructor/assignment. Most are thread-safe via `std::mutex`. `CookieBlockManager` and `EphemeralCookieManager` use `std::shared_mutex` for read-heavy IO-thread access.

### Data Storage

| Manager | Storage | Database |
|---------|---------|----------|
| `BookmarkManager` | SQLite | `bookmarks.db` |
| `CookieBlockManager` | SQLite + in-memory sets | `cookie_blocks.db` |
| `HistoryManager` | SQLite | `History` (own DB, not CEF's) |
| `CookieManager` | CEF internal cookie store | N/A (uses CEF APIs) |
| `AdblockCache` | In-memory cache + JSON file | `adblock_settings.json` |
| `SettingsManager` | JSON file | `settings.json` |
| `ProfileManager` | JSON file | `profiles.json` |

### V8 Handlers (Render Process)

These run in the renderer process and communicate via IPC:

| Handler | JavaScript API Surface |
|---------|----------------------|
| `BRC100Handler` | BRC-100 protocol: identity, auth, sessions, BEEF, SPV |
| `IdentityHandler` | Identity certificate operations |
| `AddressHandler` | BSV address generation |
| `NavigationHandler` | Back, forward, reload, navigate |

### HTTP Backend Communication

Two backend services on localhost:

| Backend | Port | Client Classes |
|---------|------|---------------|
| Rust wallet | 31301 | `WalletService`, `BRC100Bridge`, `HttpRequestInterceptor` (via `AsyncWalletResourceHandler` in `.cpp`) |
| Adblock engine | 31302 | `AdblockCache` |

`SyncHttpClient` provides the cross-platform HTTP abstraction (WinHTTP on Windows, libcurl on macOS). `AdblockCache` uses it on macOS but still has inline WinHTTP on Windows. `WalletService` and `BRC100Bridge` use direct WinHTTP (Windows-only; macOS port pending).

### Cross-Platform Conditionals

All files use `#ifdef _WIN32` / `#elif defined(__APPLE__)` for platform differences:
- Window handles: `HWND` vs `void*` (NSView/NSWindow)
- Crypto: `CryptGenRandom` vs `SecRandomCopyBytes`
- HTTP: WinHTTP vs libcurl (via `SyncHttpClient`)
- Path separators: `\\` vs `/`

## Key Data Structures

```cpp
// Tab state (Tab.h)
struct Tab { int id; std::string url, title, favicon_url; CefRefPtr<CefBrowser> browser; bool is_visible, is_loading, can_go_back, can_go_forward, has_cert_error; };

// Per-window state (BrowserWindow.h)
class BrowserWindow { int window_id; /* 11 overlay HWNDs, 6 mouse hooks, 6 icon offsets, 15 CefBrowser refs */ };

// Auth request queue (PendingAuthRequest.h)
struct PendingAuthRequest { std::string requestId, domain, method, endpoint, body, type; CefRefPtr<CefResourceHandler> handler; };
// Types: "domain_approval", "brc100_auth", "no_wallet", "payment_confirmation", "rate_limit_exceeded", "certificate_disclosure"

// Session tracking (SessionManager.h)
struct BrowserSession { int browserId; std::string domain; int64_t spentCents; int paymentRequestsThisMinute; };

// Settings (SettingsManager.h)
struct BrowserSettings { std::string homepage, searchEngine; double zoomLevel; bool showBookmarkBar, restoreSessionOnStart, askWhereToSave; };
struct PrivacySettings { bool adBlockEnabled, thirdPartyCookieBlocking, doNotTrack, clearDataOnExit, fingerprintProtection; };
struct WalletSettings { bool autoApproveEnabled; int defaultPerTxLimitCents, defaultPerSessionLimitCents, defaultRateLimitPerMin; bool peerpayAutoAccept; };

// Ephemeral cookies (EphemeralCookieManager.h)
struct SiteSession { std::string site; int tab_ref_count; std::unordered_set<std::string> third_party_domains; bool grace_active; };

// Adblock cosmetic result (AdblockCache.h)
struct CosmeticResult { std::string cssSelectors, injectedScript; bool generichide; };
```

## Thread Safety Notes

- **UI thread only**: `TabManager`, `CookieManager` (CEF requirement)
- **IO thread reads, UI thread writes**: `CookieBlockManager`, `EphemeralCookieManager` (via `shared_mutex`)
- **Any thread**: `AdblockCache`, `PendingRequestManager`, `SessionManager`, `FingerprintProtection`, `SettingsManager`, `HistoryManager` (via `mutex`)
- **Render process only**: `AddressHandler`, `BRC100Handler`, `IdentityHandler`, `NavigationHandler` (V8 handlers)

## Related

- [../../../CLAUDE.md](../../../CLAUDE.md) — root project context (architecture overview, invariants, overlay lifecycle)
- [../../CLAUDE.md](../../CLAUDE.md) — `cef-native/` build instructions, window/process architecture, IPC flow, entry points
- Implementations live in `cef-native/src/core/` (e.g., `HistoryManager.cpp`, `HttpRequestInterceptor.cpp`, `BRC100Bridge.cpp`)
- V8 injection in `cef-native/src/handlers/simple_render_process_handler.cpp`
- IPC dispatch in `cef-native/src/handlers/simple_handler.cpp`
