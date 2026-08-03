# CEF Core Services
> Singleton managers, V8 handlers, and HTTP clients that power the browser shell's non-UI logic.

**Last Updated:** 2026-08-03

## Overview

This module contains the core C++ business logic for the CEF browser shell. It provides data persistence (history, bookmarks, cookies, settings, profiles, site permissions, paid content), wallet/BRC-100 communication with the Rust backend, tab/window lifecycle management, the Windows auto-update stack, and V8 JavaScript handler classes that expose `window.hodosBrowser.*` APIs to the render process. Most classes are singletons initialized at startup and accessed from the browser process UI thread.

All files are cross-platform (Windows + macOS) unless noted. Windows uses WinHTTP; macOS uses libcurl. Platform-specific code is gated with `#ifdef _WIN32` / `#elif defined(__APPLE__)`.

**Ports:** never hardcode. `include/core/PortConfig.h` is the single source of truth — wallet `127.0.0.1:31301` in release / `31401` under `HODOS_DEV=1`, adblock `31302` / `31402`. Use `hodos::WalletUrl(path)`, `hodos::WalletPort()`, `hodos::IsWalletHostPort(url)`.

**The permission DECISION engine is NOT here.** It lives in Rust (`rust-wallet/crates/hodos_permission_engine`, wrapped by `rust-wallet/src/permission_service/`, wired as Actix middleware in `rust-wallet/src/main.rs`). The former C++ `PermissionEngine.cpp` and `SessionManager.cpp` were **deleted in Phase 2.6-H** — neither file exists in this directory. C++ is now a thin proxy: it forwards every external wallet call to Rust and reacts to `200` (silent) / `202` (prompt) / `403` (deny). See `runIpcEngineCascade` in `HttpRequestInterceptor.cpp`.

## Files

Directory contains **32 `.cpp` files, 3 `.mm` files, and 1 local header (`DefaultTrackerList.h`)**. Windows-only sources are marked; the rest build on both platforms (see `cef-native/CMakeLists.txt` `SOURCES` lists).

### Wallet / BRC-100 / permission proxy

| File | Purpose |
|------|---------|
| `HttpRequestInterceptor.cpp` | **By far the largest file (~5,150 lines / ~239 KB).** HTTP routing + Rust-engine proxy for wallet API calls. In-TU singletons: `DomainPermissionCache`, `WalletStatusCache`, `BSVPriceCache`, `NoWalletNotificationTracker`. Resource handlers: `AsyncWalletResourceHandler` (+ its `AsyncHTTPClient`), `Async402ResourceHandler` (+ `Async402HTTPClient`) for BRC-121. Entry points declared in `include/core/HttpRequestInterceptor.h`: `TryHandleBrc121_402`, `InstallAsync402HandlerIfPending`, `HandleIpcWalletCall`, `OnWalletCallSuccess`, `ResumeDrainedApprovedRequest`, `ForwardPendingWalletRequest`, `IsInternalOrigin`, plus **11 free-function modal openers** (`openDomainApprovalModal`, `openBRC100AuthApprovalModal`, `openManifestConnectBundleModal`, `openIdentityKeyRevealModal`, `openKeyLinkageRevealModal`, `openPaymentConfirmationModal`, `openRateLimitExceededModal`, `openProtocolPermissionPromptModal`, `openBasketPermissionPromptModal`, `openCounterpartyPermissionPromptModal`, `openCertificateDisclosureModal`) fronted by `OpenPromptModal`. Modal enrollment/resolution goes through `PendingRequestManager` (`include/core/PendingAuthRequest.h`). Session teardown calls into Rust via `ClearRustPaymentSessionForBrowser` → `fireSessionCloseToRust`. |
| `ManifestFetcher.cpp` | Fetches + parses `.well-known/wallet-manifest.json` from dApp origins. `hodos::ManifestFetcher::Fetch(origin)` uses `SyncHttpClient` (3 s timeout, 64 KB cap); `hodos::ManifestFetcher::ParseFromJson(json)` is pure and lenient (unknown fields ignored, malformed entries dropped, never throws). Structs in `ManifestFetcher.h`: `ManifestProtocol`, `ManifestBasket`, `ManifestCertificate`, `ManifestSpending`, `ManifestCounterparty`, `Manifest`. Consumed by `openManifestConnectBundleModal`. |
| `PaidContentCache.cpp` | BRC-121 paid response cache. Singleton. SQLite `paid_content_cache.db`, table `paid_content` keyed by URL (status/headers/body/byte_size/paid_at/last_access/expires_at). Methods: `Initialize`, `Shutdown`, `IsInitialized`, `SetEnabled`/`IsEnabled`, `Get`, `Put`, `Clear`, `GetTotalSize`, `EvictIfOverCap`; static `ParseCacheControl` extracts `max-age=N`. 500 MB LRU cap on `last_access`. `Put` is best-effort and swallows exceptions so a cache failure can never break the **gold-pill** payment indicator. Read-side playback handler is header-only in `include/core/CachedContentResourceHandler.h`. |
| `WalletService.cpp` | **Windows.** Wallet HTTP client over WinHTTP, base URL from `hodos::WalletBaseUrl()`. Methods per `include/core/WalletService.h`: `ensureInitialized`, `isHealthy`, `getWalletStatus`, `getWalletInfo`, `createWallet`, `loadWallet`, `markWalletBackedUp`, `getAllAddresses`, `generateAddress`, `getCurrentAddress`, `createTransaction`, `signTransaction`, `broadcastTransaction`, `sendTransaction`, `getBalance`, `getTransactionHistory`, `isConnected`, `setBaseUrl`, `makeHttpRequestPublic`, plus daemon lifecycle (`startDaemon`, `stopDaemon`, `isDaemonRunning`, `setDaemonPath`, `monitorDaemon`). |
| `WalletService_mac.cpp` | macOS `WalletService` using libcurl. Same header/API, same `hodos::WalletBaseUrl()`. No daemon management (developer runs the Rust wallet manually). |

> There is **no `BRC100Bridge.cpp` and no `BRC100Handler.cpp`** in this directory — they were removed. BRC-100 now reaches the page through the `window.CWI` shim (`include/core/CWIShimScript.h`) and the wallet IPC bridge (`HandleIpcWalletCall`), not through a dedicated V8 bridge class.

### Payment success indicator — the GOLD PILL safeguard

The **gold pill** is the tab-strip badge that flashes on the paying tab every time a payment is auto-approved without a modal. It is the user's primary visual safeguard against silent payment abuse and **must survive every refactor of this directory**. It is never a "green dot" — do not rename it in code, comments, or logs.

**Single emit point:** `HttpRequestInterceptor.cpp :: OnWalletCallSuccess(browserId, domain, cents, wasAutoApprovedPayment, endpoint)` (declared in `include/core/HttpRequestInterceptor.h`). Everything else funnels through it.

```
OnWalletCallSuccess
  ├─ early-return unless wasAutoApprovedPayment            (no cents floor — see below)
  ├─ SimpleHandler::GetHeaderBrowser()                     (bail if no header browser)
  ├─ TabManager::GetTabIdForBrowserIdentifier(browserId)   (CEF browser id → Tab::id)
  ├─ payload = { browserId: <Tab::id>, domain, cents }     (field name kept for React compat)
  └─ header frame SendProcessMessage(PID_RENDERER, "payment_success_indicator")
        → simple_render_process_handler.cpp  (handler for "payment_success_indicator")
            → frame->ExecuteJavaScript: window.dispatchEvent(MessageEvent 'message',
                                          { type: 'payment_success_indicator', data })
                → frontend/src/hooks/useTabManager.ts  (message listener)
                    → tab.paymentIndicator = { amount, timestamp }, 6000 ms badge
```

**Six call sites of `OnWalletCallSuccess`, all in `HttpRequestInterceptor.cpp`:**

| Enclosing symbol | Path it covers |
|---|---|
| `AsyncHTTPClient::OnRequestComplete` | HTTP interception path — `createAction` and friends silent-approved by Rust |
| `Async402HTTPClient::firePaymentSuccessIpc` | BRC-121 paid retry — fires on `2xx` from the upstream retry, alongside `broadcastNosendAsync()` and the `PaidContentCache::Put` |
| `runIpcEngineCascade` | IPC-bridge (`window.CWI`) path — silent approve |
| `resumeInternalResponse` | modal-resume, internal delivery |
| `resumeHttpCallbackResponse` | modal-resume, HTTP resource-handler delivery |
| `resumeIpcResponse` | modal-resume, IPC delivery |

**Invariants:**
- **No `cents > 0` guard.** Removed deliberately (6.d.BE+1). Sub-`~16,667`-sat payments round to 0 cents at current BSV price and would silently lose the badge. React renders `< $0.01`.
- **ID translation is mandatory.** `Tab::id` ≠ `CefBrowser::GetIdentifier()` — `Tab::id` is a `TabManager`-local counter, the CEF identifier is global (counts overlays, devtools, etc.). Always go through `TabManager::GetTabIdForBrowserIdentifier` (`TabManager.cpp` / `TabManager_mac.mm`). The JSON field stays named `browserId` for historical React compat but carries a `Tab::id`.
- **Ordering in the BRC-121 path:** `firePaymentSuccessIpc()` runs *before* `PaidContentCache::Put()` so a disk/SQLite failure cannot suppress the badge.
- **No spend accounting here.** Per-session spend, rate counters and payment counts are recorded in Rust at payment-decision time (`dispatch_payment`). `OnWalletCallSuccess` keeps **only** the gold-pill IPC.

### Browsing data & privacy

| File | Purpose |
|------|---------|
| `HistoryManager.cpp` | Browser history SQLite. Singleton. Chromium-compatible schema (`urls` + `visits`). `AddVisit` (debounced), `GetHistory`, `SearchHistory`, `GetTopSites`, `SearchHistoryWithFrecency` (SQL frecency + post-query domain boost), `DeleteHistoryEntry`, `DeleteAllHistory`, `DeleteHistoryRange`. Chromium epoch timestamps (µs since 1601). |
| `BookmarkManager.cpp` | Bookmark CRUD with SQLite (`bookmarks.db`). Singleton. Tables: `bookmark_folders`, `bookmarks`, `bookmark_tags`. `AddBookmark`, `CreateFolder`, `GetAllBookmarks`, `SearchBookmarks`, `DeleteBookmark`, `MoveBookmark`, etc. Initialized with the profile-specific `user_data_path`. |
| `CookieManager.cpp` | CEF cookie CRUD via `CefCookieManager`. Static methods: `HandleGetAllCookies`, `HandleDeleteCookie`, `HandleDeleteDomainCookies`, `HandleDeleteAllCookies`, `HandleClearCache`, `HandleGetCacheSize`. Uses `CookieCollector` (`CefCookieVisitor`), posts results back to the UI thread via `SendResponseTask`; `CacheSizeTask` runs on `TID_FILE_USER_BLOCKING`. |
| `CookieBlockManager.cpp` | Third-party cookie blocking engine. Singleton. SQLite `cookie_blocks.db`, tables `blocked_domains`, `allowed_third_party`, `block_log`, `meta`. Pre-populated on first run from `DefaultTrackerList.h`. Block logging posted to `TID_FILE_USER_BLOCKING`. Integrates with `EphemeralCookieManager` and `SettingsManager`. |
| `EphemeralCookieManager.cpp` | Ephemeral (session-only) cookie cleanup. Singleton with `shared_mutex`. Tracks per-site tab refcounts; when the last tab navigates away, a 30 s grace period (`GraceExpiredTask` via `CefPostDelayedTask(TID_IO, …)`) elapses before third-party cookies for that site are deleted. |
| `SitePermissionStore.cpp` | Per-host site permission store (camera/mic/location/etc.). Singleton. SQLite `site_permissions.db`, table `site_permissions`. `Initialize`, `Shutdown`, `IsInitialized`, `GetState`, `SetState`, `ResetDomain`, `GetAllForHost`, static `NormalizeHost`. Enums `SitePermissionType` / `SitePermissionState` in the header. Backs the right-click "Manage Site Permissions" flow. |
| `DefaultTrackerList.h` | Local header. `DEFAULT_TRACKERS` — **24** `pair<domain, is_wildcard>` entries (Google ×5, Meta ×2, other majors ×8, ad networks ×5, analytics ×4), all exact-match. Consumed by `CookieBlockManager` on first-run init. |

### Windows / tabs / profiles

| File | Purpose |
|------|---------|
| `TabManager.cpp` | **Windows.** Tab lifecycle. Singleton (`unique_ptr`, not Meyer's). One windowed CEF browser per tab, HWND parented to the shell window; only the active tab is `WS_VISIBLE`. `CreateTab`, `CloseTab`, `SwitchToTab`, `GetTab`, `GetActiveTab`, `GetAllTabs`, `GetTabIdForBrowserIdentifier`, `ReorderTabs`, `MoveTabToWindow` (tear-off/merge), `GetActiveTabIdForWindow`, `GetActiveTabForWindow`, `GetTabCount`, `UpdateTabTitle/URL/LoadingState/Favicon`, `RegisterTabBrowser` (also re-runs layout — fixes the off-center webview race), `OnTabBrowserClosed`, `RecordClosedTab`/`GetRecentlyClosed`/`RemoveRecentlyClosed`. `CloseTab` calls `ClearRustPaymentSessionForBrowser(browserId)` so a reopened tab on the same domain starts with fresh caps. |
| `TabManager_mac.mm` | macOS tab lifecycle using NSView instead of HWND. Same `TabManager.h` API, including `GetTabIdForBrowserIdentifier` and the `ClearRustPaymentSessionForBrowser` call in `CloseTab`. `[view setHidden:]` for show/hide, `[view removeFromSuperview]` for cleanup; synchronous view removal on close (differs from the Windows async pattern). |
| `WindowManager.cpp` | Multi-window management (cross-platform core + Windows `CreateFullWindow`). Singleton, mutex-protected. `CreateWindowRecord`, `RemoveWindow`, `GetWindow`, `GetActiveWindow`, `GetWindowByHwnd` (Win) / `GetWindowByNSWindow` (mac), `GetWindowForBrowser`, `GetAllWindows`, `GetWindowCount`, `SetActiveWindowId`/`GetActiveWindowId`, and primary-window tracking (`SetPrimaryWindowId`, `GetPrimaryWindowId`, `GetPrimaryWindow`, `GetNextWindowId`) — the primary window owns the overlay handles and transfers when it closes. |
| `WindowManager_mac.mm` | macOS `CreateFullWindow()`. NSWindow with header NSView (99 px) + webview NSView, `BrowserWindowDelegate` for resize/move/close, per-window delegate lifetime via `objc_setAssociatedObject`. Also `GetWindowAtScreenPointMacOS()` (hit-test for tab merge), `PositionWindowAtScreenPoint()` (tear-off), and the ghost-tab preview window (`ShowGhostTabMacOS` / `HideGhostTabMacOS`) with a 60 fps cursor-following timer. |
| `BrowserWindow.cpp` | Per-window state record owned by `WindowManager`. Implements `SetBrowserForRole` / `GetBrowserForRole` / `ClearBrowserForRole` over **18 role strings**: `header`, `webview`, `wallet_panel`, `overlay`, `settings`, `wallet`, `backup`, `brc100auth`, `notification`, `settings_menu`, `omnibox`, `cookiepanel`, `downloadpanel`, `profilepanel`, `menu`, `bookmarkspanel`, `siteinfopanel`, `tablistpanel`. The header additionally carries **14 overlay HWNDs** (macOS: 14 overlay `NSWindow*`), **10 mouse hooks** (macOS: 9 NSEvent monitors), and **9 icon offsets** (6 right-anchored + 3 left-anchored: bookmarks, site-info, tab-list). |
| `ProfileManager.cpp` | Multi-profile management. Singleton. `profiles.json` at the app-data root, cross-process registry lock + atomic tmp+rename writes (`Save` / `SaveUnlocked`). `GetAllProfiles`, `GetCurrentProfile`, `GetProfileById`, `CreateProfile`, `DeleteProfile`, `RenameProfile`, `SetProfileColor`, `SetProfileAvatar`, `SetDefaultProfile`/`GetDefaultProfileId`, `SetCurrentProfileId`/`GetCurrentProfileId`, `GetProfileDataPath`/`GetCurrentProfileDataPath`, `ShouldShowPickerOnStartup`/`SetShowPickerOnStartup`, `LaunchWithProfile` (Windows: spawns a new process with `--profile=`), static `IsValidProfileId`, `ResolveStartup` (pure startup resolver), `ParseProfileArgument`. Profile IDs: `"Default"` or `"Profile_N"`. |
| `ProfileImporter.cpp` | Import bookmarks and history from Chrome, Brave, and Edge. `DetectProfiles()` scans standard browser profile paths. `ImportBookmarks()` parses the Chromium JSON bookmark format recursively. `ImportHistory()` copies and reads the Chromium History SQLite DB (handles WAL files). Writes into `BookmarkManager` and `HistoryManager`. |
| `ProfileLock.cpp` | Single-instance **profile** lock. Windows: `CreateFileA` with `FILE_FLAG_DELETE_ON_CLOSE`. macOS/Linux: `flock()` with `LOCK_EX \| LOCK_NB`. Prevents two browser instances from sharing one profile directory. |
| `SingleInstance.cpp` | **Windows.** Cross-process sole-instance coordination + URL hand-off, namespaced per profile **and** per dev/prod (`hodos::IsDevEnv()`) so a dev build and the installed build never collide. `TryAcquireInstance`, `SendToRunningInstance(profileId, url)`, `StartListenerThread`, `StopListenerThread`, `IsShuttingDown`, `SetShuttingDown`. Distinct from `ProfileLock.cpp` — that one guards the data directory, this one routes a second launch to the running window. |
| `TaskbarProfile.cpp` | **Windows.** `SetupTaskbarProfile(hwnd, hInstance)` — per-profile AppUserModelID + generated taskbar overlay icon (profile color/initial) so multiple profiles get separate taskbar entries. |

### Settings, updates, misc

| File | Purpose |
|------|---------|
| `SettingsManager.cpp` | Persistent settings with profile support. Singleton. JSON at `<profile>/settings.json`; migrates from global to per-profile on first use. **Three** setting structs in `SettingsManager.h`: `BrowserSettings` (homepage, searchEngine, zoomLevel, showBookmarkBar, downloadsPath, restoreSessionOnStart, askWhereToSave, `autoUpdateMode` = off/notify/**silent**), `PrivacySettings` (adBlockEnabled, thirdPartyCookieBlocking, doNotTrack, clearDataOnExit, fingerprintProtection, paidContentCacheEnabled), `WalletSettings` (autoApproveEnabled, `defaultPerTxLimitCents = 100` → **$1.00 per tx**, `defaultPerSessionLimitCents = 1000` → **$10.00 per session**, defaultRateLimitPerMin = 30, defaultMaxTxPerSession = 100, peerpayAutoAccept). |
| `AutoUpdater.cpp` | **Windows.** WinSparkle wrapper. Singleton. `Initialize(version, appcastUrl, autoCheck)`, `CheckForUpdatesInteractively`, `CheckForUpdatesInBackground`, `SetAutoCheckEnabled`/`IsAutoCheckEnabled`, `SetUpdateMode` (`UpdateMode` enum), `SetCheckInterval`, `SetShutdownCallback`, `Cleanup`. All WinSparkle config must happen before `win_sparkle_init()`. |
| `AutoUpdater_mac.mm` | macOS `AutoUpdater` via Sparkle. Same `AutoUpdater.h` singleton API. |
| `UpdateStager.cpp` | **Windows.** Downloads + verifies an update into a staging dir. `UpdateStager` class plus statics: `ParseWindowsAppcastItem`, `AppcastSignaturePrefix`, `IsNewerBuild`, `Sha256File`, `SerializeMarker`/`ParseMarker` (`StagedUpdateMarker`), kill-list handling (`KillList`, `ParseKillList`, `KillListSignaturePrefix`, `IsBuildRetracted`), Authenticode verification (`AuthenticodeResult`, `ExpectedSigner`), `PublicKeyBase64`. Structs/enums: `AppcastEntry`, `StagedUpdateMarker`, `StageResult`. **The signer gate compares Subject CN, not a rotating leaf thumbprint.** |
| `UpdateApply.cpp` | **Windows.** Apply-phase state machine + serialization. `ApplyPhase` enum with `ApplyPhaseToString`/`ApplyPhaseFromString`; `ApplyRecord`, `UpdateState`, `FileManifest` with `Serialize*`/`Parse*` pairs; `PausedBlocksStagedBuild`, `NormalizeManifestKey`. |
| `UpdateFs.cpp` | **Windows.** Filesystem primitives for the updater: `Sha256FileW`, `EnsureDirExists`, `VerifyTreeAgainstManifest` (`VerifyResult`), `SnapshotWalletDbSet`/`RestoreWalletDbSet` (wallet DB safety around an apply), `SwapFileReplace`, `FreeBytesOnVolume`, `DirSizeBytes`, `WriteFileAtomic`, `ReadFileAll`, `RemoveTree`, and manifest-signature verification (`ManifestSignaturePrefix`, `PublicKeyBase64`, `VerifyManifestSignature`). Lock helpers are header-only in `include/core/UpdateLock.h`. |
| `SilentStateWriter.cpp` | Writes/mirrors silent-update eligibility into `update-state.json`. Pure, unit-testable helpers in `namespace hodos`: `UpdateModeRank`, `MoreConservativeMode` (ties resolve to the *safer* mode), `ComputeSilentEligibility` (touches only `state.silent`, never clears `paused`), `MirrorSilentEligibility`. |
| `QRScreenCapture.cpp` | **Windows.** OS-level screen-region capture + QR decode via `quirc` (follows the ghost-tab overlay pattern). `StartQRScreenCapture()`, `FinishQRScreenCapture(cancelled, selection)`. Feeds the wallet overlay's QR scan flow; the in-page scanner script is `include/core/QRScannerScript.h`. |
| `SyncHttpClient.cpp` | Cross-platform synchronous HTTP client. Windows WinHTTP / macOS libcurl. Static methods: `Get(url, timeoutMs)`, `Get(url, headers, timeoutMs)`, `Post(url, body, contentType, timeoutMs)`, `Post(url, body, headers, timeoutMs)`, `Download(url, destPath, timeoutMs = 120000)` (streams to `<dest>.partial`, renames only on a complete 2xx — used for the ~95 MB installer), and `Request(method, url, body, headers, timeoutMs)` for verbs like `DELETE`. Returns `HttpResponse{statusCode, body, success}`. Supports external `https://` with redirect following. |
| `GoogleSuggestService.cpp` | Omnibox search suggestions. Singleton. Google Suggest or DuckDuckGo `/ac/`, JSON array parsing, URL-encoded query, 5 s timeout. **Cross-platform** — WinHTTP on Windows, libcurl on macOS. |
| `Logger.cpp` | Out-of-line storage + `Initialize()` / `Log()` / `Shutdown()` / `IsInitialized()` for the header-only `Logger` class (`include/core/Logger.h`). `LogLevel` = DEBUG 0 / INFO 1 / WARNING 2 / ERROR_LEVEL 3; `ProcessType` = MAIN 0 / RENDER 1 / BROWSER 2. Output format `[timestamp] [PROCESS] [LEVEL] message` to `debug_output.log`. |

### V8 handlers (render process)

| File | Purpose |
|------|---------|
| `AddressHandler.cpp` | `CefV8Handler` for address generation. `Execute()` dispatches `"generate"`, `"getAll"`, `"getCurrent"`. Overlay browsers get a direct V8 return; the main browser uses IPC process messages with promise-like objects. |
| `IdentityHandler.cpp` | `CefV8Handler` for `identity.get()` and `identity.markBackedUp()` (2 methods). Checks a local `identity.json` first, falls back to the wallet daemon via `WalletService`. Also exposes the `jsonToV8` helper. Platform-aware file paths. |
| `NavigationHandler.cpp` | `CefV8Handler` for `window.hodosBrowser.navigation.navigate()`. Rewrites the `hodos://` custom protocol to `http://127.0.0.1:5137/` and sends a `"navigate"` IPC message to the browser process. |

## Singleton Pattern

Most managers use Meyer's singleton:
```cpp
SettingsManager& SettingsManager::GetInstance() {
    static SettingsManager instance;
    return instance;
}
```

**Meyer's singletons defined in this directory (11):** `AutoUpdater` (`AutoUpdater.cpp` on Windows, `AutoUpdater_mac.mm` on macOS), `BookmarkManager`, `CookieBlockManager`, `EphemeralCookieManager`, `GoogleSuggestService`, `HistoryManager`, `PaidContentCache`, `ProfileManager`, `SettingsManager`, `SitePermissionStore`, `WindowManager`.

**Meyer's singletons local to `HttpRequestInterceptor.cpp` (4):** `DomainPermissionCache`, `WalletStatusCache`, `BSVPriceCache`, `NoWalletNotificationTracker`. These are file-scope classes with no header — reach them through the free functions in `HttpRequestInterceptor.h` (`warmDomainPermissionCache`, `warmWalletStatusCache`, `warmBSVPriceCache`, `invalidateDomainPermissionCache`, `clearDomainPermissionCache`, `GetDomainIdentityKeyDisclosureAllowed`).

**`unique_ptr` singleton (1):** `TabManager` — lazy `std::unique_ptr<TabManager>` init in `GetInstance()` (`TabManager.cpp` / `TabManager_mac.mm`), not Meyer's.

**Header-only singletons used from here but owned by `include/core/`:** `AdblockCache`, `FingerprintProtection`, `PendingRequestManager` (`PendingAuthRequest.h`), `PendingPermissionManager` (`PendingPermissionRequest.h`).

**Non-singletons:** `WalletService` (instantiated per use), `BrowserWindow` (one per window, owned by `WindowManager`), `ManifestFetcher` (static methods, no instance state), the V8 handlers (`AddressHandler`, `IdentityHandler`, `NavigationHandler` — refcounted per V8 context), `SyncHttpClient` and `Logger` (all-static), and the `hodos::` free functions in `SilentStateWriter` / `SingleInstance` / `UpdateApply` / `UpdateFs` / `TaskbarProfile` / `QRScreenCapture`.

> `SessionManager` and `PermissionEngine` used to appear in this list. **Both are gone** — session spend/rate state and every permission decision live in Rust.

## Threading Model

Only three CEF thread IDs are referenced in this directory: `TID_UI`, `TID_IO`, and `TID_FILE_USER_BLOCKING`.

| Thread | Components |
|--------|-----------|
| **UI thread (`TID_UI`)** | `TabManager`, `WindowManager`, `BrowserWindow`, `CookieManager` result delivery, all modal openers + `PendingRequestManager` enrollment, `OnWalletCallSuccess` (the gold-pill IPC is always posted to `TID_UI` before sending), IPC dispatch |
| **IO thread (`TID_IO`)** | `CookieCollector::Visit()`, `EphemeralCookieManager::GraceExpiredTask` (via `CefPostDelayedTask`), BRC-121 upstream-retry scheduling (`Async402HTTPClient::StartTask`), `CookieBlockManager` cookie checking |
| **`TID_FILE_USER_BLOCKING`** | `CookieManager::CacheSizeTask`, `CookieBlockManager`'s block-log write, `Async402HTTPClient::BroadcastTask` (`/wallet/broadcast-nosend` via `SyncHttpClient`), wallet worker posts in `HttpRequestInterceptor` — anything blocking that must not sit on IO or UI |
| **Render thread** | V8 handlers (`AddressHandler`, `IdentityHandler`, `NavigationHandler`) |
| **Any thread** | `SyncHttpClient`, `Logger::Log()`, `DomainPermissionCache` / `WalletStatusCache` / `BSVPriceCache` (mutex-protected), `EphemeralCookieManager` (`shared_mutex`), `WindowManager` / `ProfileManager` (mutex-protected) |

Cross-thread communication uses `CefPostTask(TID_*)` / `CefPostDelayedTask(TID_*)` and `SendResponseTask` to hop results back to the UI thread for IPC dispatch.

## Key Architectural Patterns

### V8 Handler Pattern (Render Process)
V8 handlers implement `CefV8Handler::Execute()` and are registered in `OnContextCreated()`:
```
React calls window.hodosBrowser.addresses.generate()
  → AddressHandler::Execute("generate", args)
    → overlay browser: direct V8 return
    → main browser: IPC process message + promise-like object
```

BRC-100 no longer has a dedicated V8 bridge — the page-facing surface is the `window.CWI` shim (`include/core/CWIShimScript.h`), whose calls arrive in the browser process and are serviced by `HandleIpcWalletCall` in `HttpRequestInterceptor.cpp`.

### HTTP Interception Pattern (Browser Process)
`HttpRequestInterceptor` sits in CEF's resource-request pipeline. C++ is a **thin proxy** — Rust decides:
```
Page requests http://127.0.0.1:<hodos::WalletPort()>/createAction
  → isWalletEndpoint() matches
  → C++ enriches: X-Requesting-Domain, X-Browser-Id,
                  X-Payment-Satoshis / X-Payment-Cents / X-Bsv-Price-Available
                  (satoshi + cents derivation stays in C++ via BSVPriceCache)
  → forwarded unconditionally to the Rust wallet
  → Rust permission middleware answers:
       200 → silent approve  → deliver + OnWalletCallSuccess (GOLD PILL)
       202 → prompt          → OpenPromptModal → PendingRequestManager
                               → user resolves → resume*Response → (payment? GOLD PILL)
       403 → deny            → error surfaced to the page
```

### Database Initialization Pattern
Data managers follow a consistent pattern:
```cpp
manager.Initialize(user_data_path);  // Opens/creates SQLite DB, runs schema migrations
// ... use throughout app lifetime ...
manager.Shutdown();                  // idempotent; destructor also closes
```

SQLite databases owned by this directory, all under the active profile dir:
`history` DB (`HistoryManager`), `bookmarks.db`, `cookie_blocks.db`, `site_permissions.db`, `paid_content_cache.db`.

Storage paths:
- Windows: `%APPDATA%/HodosBrowser/<Profile>/` (dev: `%APPDATA%/HodosBrowserDev/<Profile>/`)
- macOS: `~/Library/Application Support/HodosBrowser/<Profile>/` (dev: `…/HodosBrowserDev/…`)

## Cross-Platform Notes

| Component | Windows | macOS |
|-----------|---------|-------|
| `WalletService` | `WalletService.cpp` (WinHTTP) | `WalletService_mac.cpp` (libcurl) |
| `SyncHttpClient` | WinHTTP | libcurl |
| `TabManager` | `TabManager.cpp` (HWND) | `TabManager_mac.mm` (NSView) |
| `WindowManager::CreateFullWindow` | `WindowManager.cpp` (HWND + WM_SIZE) | `WindowManager_mac.mm` (NSWindow + `BrowserWindowDelegate`) |
| `AutoUpdater` | `AutoUpdater.cpp` (WinSparkle) | `AutoUpdater_mac.mm` (Sparkle) |
| `GoogleSuggestService` | WinHTTP | libcurl (implemented — no longer a stub) |
| `ProfileLock` | `CreateFileA` exclusive lock | `flock()` |
| `SingleInstance` | `SingleInstance.cpp` (named pipe + listener thread) | Same file, compiled but stubbed — every `SingleInstance::` function is a documented no-op under the macOS branch |
| `TaskbarProfile`, `QRScreenCapture` | Windows-only | Not present |
| `UpdateStager` / `UpdateApply` / `UpdateFs` / `SilentStateWriter` | Windows-only (silent-update stack) | `SilentStateWriter.cpp` also builds on macOS; the rest are Windows-only |

Not in the macOS `SOURCES` list in `CMakeLists.txt` at all: `TabManager.cpp`, `WalletService.cpp`, `TaskbarProfile.cpp`, `QRScreenCapture.cpp`, `AutoUpdater.cpp`, `UpdateStager.cpp`, `UpdateApply.cpp`, `UpdateFs.cpp`. (`SingleInstance.cpp` and `ProfileLock.cpp` are in the shared list but internally `#ifdef`-split.)

When adding new HTTP-calling singletons, use `SyncHttpClient` (already cross-platform) rather than raw WinHTTP, and route the port through `hodos::WalletUrl()` / `hodos::AdblockUrl()` — never a literal.

## Build output

The CMake target is `HodosBrowserShell` but `OUTPUT_NAME` is set to `HodosBrowser` on both platforms — the built Windows binary is **`HodosBrowser.exe`**, not `HodosBrowserShell.exe`.

## Related

- **Headers:** `cef-native/include/core/` — 47 `.h` files: all class declarations for the sources here, plus header-only classes and embedded scripts (`PortConfig.h`, `AdblockCache.h`, `FingerprintProtection.h`, `FingerprintScript.h`, `PendingAuthRequest.h`, `PendingPermissionRequest.h`, `CachedContentResourceHandler.h`, `LocalFileResourceHandler.h`, `SensitiveCertFields.h`, `CWIShimScript.h`, `QRScannerScript.h`, `UpdateLock.h`, `AppPaths.h`, `JsStringEscape.h`, `LayoutHelpers.h`, `Tab.h`, …). See `cef-native/include/core/CLAUDE.md`.
- **Parent CLAUDE.md:** `cef-native/CLAUDE.md` — build instructions, HWND hierarchy, IPC flow, window architecture
- **Root CLAUDE.md:** `/CLAUDE.md` — architecture overview, overlay lifecycle, CEF input patterns
- **Handler files:** `cef-native/src/handlers/` — `simple_handler.cpp` (IPC dispatch into these managers; also the `PaidContentCache` read-side dispatch in `GetResourceRequestHandler`), `simple_render_process_handler.cpp` (V8 injection + the `payment_success_indicator` renderer hop)
- **Permission engine (Rust):** `rust-wallet/crates/hodos_permission_engine` (`decide()` in `src/lib.rs`, cascade in `src/matrix_c.rs`), wrapper `rust-wallet/src/permission_service/`, middleware wiring in `rust-wallet/src/main.rs`
