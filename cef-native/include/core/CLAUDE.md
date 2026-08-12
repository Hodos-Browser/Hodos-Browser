# CEF Core Headers
> Header-only and declaration files for all C++ singletons, managers, and services in the browser shell.

**Last Updated:** 2026-08-03

## Overview

This directory contains the header files for the CEF native layer's core subsystems. These define the singletons, data structures, and class interfaces that implement browser data management (history, bookmarks, cookies), privacy features (ad blocking, fingerprint protection, cookie blocking, per-site camera/mic/location permissions), wallet integration (BRC-100 request interception, injected JS surfaces), window/tab management, the Windows auto-updater, and cross-cutting concerns (logging, settings, profiles, paths, ports). Most classes follow the singleton pattern with `GetInstance()` and are thread-safe via `std::mutex` or `std::shared_mutex`.

**48 headers** (recounted 2026-08-09 after the farbling teardown deleted `FingerprintScript.h`; it was 49 with that file, and 47 before `ChildProcessLogSink.h`). Several are pure-logic / header-only by design (`JsStringEscape.h`, `SensitiveCertFields.h`, `PortConfig.h`, `AppPaths.h`, `LayoutHelpers.h`, `CachedContentResourceHandler.h`, `LocalFileResourceHandler.h`, `Logger.h`, `PendingAuthRequest.h`, `PendingPermissionRequest.h`) so the GoogleTest target in `cef-native/tests/` can compile them with zero CEF dependencies.

## Files

| File | Purpose |
|------|---------|
| `AdblockCache.h` | `AdblockCache` singleton: URL block result cache, per-browser blocked counts, per-site adblock + scriptlet toggle persistence (`adblock_settings.json`), cosmetic filter fetching, session-total blocked counter. Also defines `AdblockBlockHandler` (cancels blocked requests), `DeferredAdblockHandler`, `AdblockFetchTask` (CefTask), nested `CosmeticResult`, `CefResourceTypeToAdblock()` mapping, and `shouldSkipAdblockCheck()`. `/check` goes through `SyncHttpClient::Post(hodos::AdblockUrl("/check"))`. `fetchCosmeticFromBackend` / `fetchHiddenIdsFromBackend` are inline WinHTTP on Windows and `SyncHttpClient` (libcurl) on macOS — the macOS arm was a `return {}` **stub until 2026-08-12**, so that platform had network blocking only (no element hiding and, more importantly, **no scriptlet injection**). Response parsing and body-building are now the shared, platform-free `ParseCosmeticResponse` / `ParseHiddenIdsResponse` / `BuildHiddenIdsBody` statics, so only the TRANSPORT differs and the two platforms cannot drift |
| `AddressHandler.h` | `AddressHandler` — CefV8Handler for `window.hodosBrowser.generateAddress()` in the render process |
| `AppPaths.h` | `namespace AppPaths` — header-only path resolution. `GetAppDirName()` (`HodosBrowser` / `HodosBrowserDev` under `HODOS_DEV=1`), `GetAppInstallDir`, `GetUpdateDir`, `GetPendingUpdateDir`, `GetRollbackDir`, `GetHelperStageDir`, `GetUpdateLockPath`, `GetUpdateStatePath`, `GetWalletDir`, `GetLogDir`, `GetInstanceMutexNameW`, `GetUpdateStagingMutexNameW`, `EnforceDevSafeguard()` (the runtime dev/prod guard), `EnvUtf8_()` (wide→UTF-8 env read; fixes mojibake for non-ASCII usernames) |
| `AutoUpdater.h` | `AutoUpdater` singleton + `enum class UpdateMode { Off, Notify, Silent }`. Wraps WinSparkle (Windows) / Sparkle 2 (macOS) over the shared appcast XML format. `Initialize`, `CheckForUpdatesInteractively`, `CheckForUpdatesInBackground`, `SetAutoCheckEnabled`, `SetUpdateMode`, `SetCheckInterval`, `Cleanup` |
| `BookmarkManager.h` | `BookmarkManager` singleton: SQLite-backed bookmark/folder CRUD with tags, search, and folder tree (max depth 3). Structs: `BookmarkData`, `FolderData` |
| `BrowserWindow.h` | `BrowserWindow` — per-window state container. Windows: 3 core HWNDs (`hwnd`, `header_hwnd`, legacy `webview_hwnd`) + **14 overlay HWNDs**, **10 `WH_MOUSE_LL` click-outside hooks**, **9 icon offsets** (6 right-anchored, 3 left-anchored: bookmarks / siteinfo / tablist). macOS mirrors with `void*` NSWindow/NSView: 14 overlay windows, **9 NSEvent local monitors**, the same 9 icon offsets. **18 `CefRefPtr<CefBrowser>` role refs** shared by both platforms. Role-based accessors: `SetBrowserForRole()`, `GetBrowserForRole()`, `ClearBrowserForRole()` |
| `CWIShimScript.h` | **Injected JavaScript surfaces** (two embedded `R"JS(...)"` string constants, ~70 KB). `WALLET_CALL_BRIDGE_SCRIPT` — Phase 2.5 promise-correlated IPC bridge (`window.__hodos_walletCall` / `window.__hodos_walletResponse`, 50 MB payload ceiling, chunked-response reassembly); idempotent IIFE, injected on first-party Hodos UI **and** external dApp pages, replacing the renderer `fetch` path so CSP/CORS are out of the call chain. `CWI_SHIM_SCRIPT` — Phase 2 `window.CWI` (canonical 28-method BRC-100 `WalletInterface`, non-writable/non-configurable) plus `window.yours` (writable legacy layer) and `window.panda` (alias); V8 `Proxy` apply-traps so detached method refs still bind; `bsv:announceProvider` multi-provider discovery. Both injected from `simple_render_process_handler.cpp :: OnContextCreated`; the bridge MUST be injected first |
| `CachedContentResourceHandler.h` | `CachedContentPlaybackHandler` (CefResourceHandler) + `CachedContentRequestHandler` (CefResourceRequestHandler), both header-only. Returned by `SimpleHandler::GetResourceRequestHandler` on a paid-content cache hit; replays stored bytes and headers, short-circuiting the entire 402 / Async402 chain |
| `ChildProcessLogSink.h` | `hodos::InstallChildProcessLogSink()` — routes a **child** process's `Logger` output into Chromium's log (`cef_debug.log`). Call once, early, in every renderer/GPU/utility process. Exists because `Logger::Initialize` is browser-process-only, so `LOG_*_RENDER` was a silent no-op everywhere else (`[RENDER]`: 0 occurrences before 2026-08-09), and because a sandboxed renderer at UNTRUSTED integrity cannot open the log file itself. DEBUG tier gated on the `--hodos-render-verbose` **switch** (never an env var — sandboxed children do not inherit the environment) |
| `CookieBlockManager.h` | `CookieBlockManager` singleton: SQLite-backed domain blocklist + third-party allowlist, in-memory `O(1)` IO-thread lookups via `shared_mutex`, per-browser blocked counts, async block logging. Structs: `BlockedDomainEntry`, `BlockLogEntry`. Also defines `CookieAccessFilterWrapper` (refcounted CEF adapter) |
| `CookieManager.h` | `CookieManager` — static-only class for CEF cookie/cache operations: enumerate, delete single/domain/all cookies, clear cache, get cache size. Called from the browser-process UI thread |
| `EphemeralCookieManager.h` | `EphemeralCookieManager` singleton: Brave-style ephemeral third-party cookies. Tracks open tabs per eTLD+1 site; 30-second grace period on last tab close before deleting third-party cookies. Struct: `SiteSession` |
| `FingerprintProtection.h` | `FingerprintProtection` singleton — **policy inputs only since the 2026-08-09 teardown; it computes and delivers no seed.** Supplies the three things `simple_handler.cpp :: OnBeforeBrowse` collapses into C2's single `enabled` bit: global toggle (`IsEnabled`/`SetEnabled`), the **host-precise** auth-domain exemption list (`IsAuthDomain()`), and the **per-site toggle persisted to `fingerprint_settings.json`** (`IsSiteEnabled`/`SetSiteEnabled`/`LoadSiteSettings`/`SaveSiteSettings`). `Initialize()` now only sets `initialized_` — do NOT delete it as empty: `IsEnabled()` is `initialized_ && enabled_` and `enabled_` defaults to **true**, so it is the gate that stops farbling being reported "on" before the user's stored settings load. ⚠️ `IsSiteEnabled`/`SetSiteEnabled` + the `fingerprint_get/set_site_enabled` IPC are **shipped user-facing control** (Privacy Shield); never delete them. Removed: `GetDomainSeed`, the per-session CSPRNG token, the seed cache — their only consumer was the deleted JS script |
| `GoogleSuggestService.h` | `GoogleSuggestService` singleton: fetches omnibox search suggestions from Google or DuckDuckGo (default). WinHTTP on Windows, libcurl on macOS (both in `GoogleSuggestService.cpp`) |
| `HistoryManager.h` | `HistoryManager` singleton: SQLite-backed browsing history with visit counting, frecency-scored search, top sites, time-range deletion, and 2-second URL debouncing. Structs: `HistoryEntry`, `HistorySearchParams`, `HistoryEntryWithScore` |
| `HttpRequestInterceptor.h` | `HttpRequestInterceptor` (CefResourceRequestHandler) — wallet endpoint routing, cookie access filtering, and the BRC-100 auth modal flow. Also declares the BRC-121 free functions (`TryHandleBrc121_402`, `TriggerPendingBrc121Reloads`, `CancelPendingBrc121Reloads`, `HasPendingBrc121ReloadForDomain`, `MarkBrc121PaymentApproved`, `RegisterBrc121FailedUrl`, `ConsumeBrc121FailedUrl`, `InstallAsync402HandlerIfPending`), the auth plumbing (`sendAuthRequestDataToOverlay`, `handleAuthResponse` × 2 overloads, `MarkIdentityKeyRevealApproved`, `MarkKeyLinkageRevealApproved`, `ForwardPendingWalletRequest`, `OnWalletCallSuccess`, `ResumeDrainedApprovedRequest`, `postIpcAuthTimeout`, `HandleIpcWalletCall`, `IsInternalOrigin`), structs `ModalContext` / `ResumeContext`, and the **11 `open*Modal()` prompt openers** + generic `OpenPromptModal()`. Concrete caches/handlers (`DomainPermissionCache`, `BSVPriceCache`, `WalletStatusCache`, `AsyncWalletResourceHandler`, `Async402ResourceHandler`) live in the `.cpp` |
| `IdentityHandler.h` | `IdentityHandler` — CefV8Handler for identity operations in the render process. Free function: `jsonToV8()` |
| `JsStringEscape.h` | `escapeJsonForJs()` — the **canonical** encoder for interpolating untrusted data into JS string literals passed to `CefFrame::ExecuteJavaScript`. Escapes `\ ' "`, `\n \r \t`, other C0 controls as `\u00XX`, and U+2028/U+2029 (3-byte UTF-8 lookahead). Deliberately does **not** HTML-entity-escape — no caller feeds an HTML parser. Header-only; unit-tested by `tests/js_string_escape_test.cpp` |
| `LayoutHelpers.h` | Windows-only DPI helpers: `HEADER_CSS_HEIGHT` (96 = 42px tab bar + 53px toolbar + 1px, matching macOS), `GetHeaderHeightPx(hwnd)`, `GetHeaderHeightPxSystem()`, `ScalePx(cssPx, hwnd)`. Use these for every overlay/header size — they are the fix path for the cross-DPI clipping bugs |
| `LocalFileResourceHandler.h` | `LocalFileResourceRequestHandler` — serves frontend files from a local directory in production builds, replacing the Vite dev server (port 5137) when `frontend/` sits next to the exe. Blocks `..`, drive-absolute, and rooted paths. Two-layer pattern mirroring `HttpRequestInterceptor` → `AsyncWalletResourceHandler` |
| `Logger.h` | `Logger` — centralized file logger with `enum class LogLevel { DEBUG, INFO, WARNING, ERROR_LEVEL }` and `enum class ProcessType { MAIN, RENDER, BROWSER }`. Declaration-only for the four public statics (`Initialize`, `Log`, `Shutdown`, `IsInitialized`); implementation in `src/core/Logger.cpp`. Default output `debug_output.log` |
| `ManifestFetcher.h` | `namespace hodos` — `ManifestFetcher::Fetch(origin)` pulls `.well-known/wallet-manifest.json` via `SyncHttpClient` (3 s timeout, 64 KB cap); `ParseFromJson()` is pure and lenient (unknown fields ignored, malformed entries dropped, never throws). Structs: `Manifest`, `ManifestProtocol`, `ManifestBasket`, `ManifestCertificate`, `ManifestSpending`, `ManifestCounterparty`. Unit-tested by `tests/manifest_fetcher_test.cpp` |
| `NavigationHandler.h` | `NavigationHandler` — CefV8Handler for navigation commands (back, forward, reload, navigate) in the render process |
| `PaidContentCache.h` | `PaidContentCache` singleton: SQLite-backed cache of BRC-121 paid HTTP responses at `<profile>/paid_content_cache.db`, keyed by URL, TTL from the server `Cache-Control: max-age`, `TOTAL_SIZE_LIMIT_BYTES = 500 MB` LRU cap on `last_access`. Struct: `PaidContentEntry`. Read-side handler: `CachedContentResourceHandler.h` |
| `PendingAuthRequest.h` | `PendingRequestManager` singleton (inline `GetInstance()`): thread-safe map of pending auth/domain/payment/certificate approvals. Struct `PendingAuthRequest` + `enum class ResumeKind { kHttpCallback, kIpcResponse, kInternal }`. Two `addRequest` overloads (HTTP-path parameter list; IPC-path by-value struct), `popRequest`, `getRequest`, `updateRequestBody`, `setApproveHeader`, per-domain queuing (`hasPendingForDomain`, `popAllForDomain`, `getRequestIdForDomain`). ⚠️ `originalIpcRequestId` MUST be set to the page-supplied id on the IPC path or the page's promise never resolves |
| `PendingPermissionRequest.h` | **Web-content permission prompts (camera / mic / location / notifications / clipboard).** `PendingPermissionManager` singleton (inline `GetInstance()`) parks a CEF permission callback while the Hodos-branded prompt is shown, resolved by the `permission_response` IPC. Struct `PendingPermissionRequest` holds exactly one of `CefMediaAccessCallback` / `CefPermissionPromptCallback`. One prompt at a time (`hasPending`), plus `add`, `pop`, `popByPromptId`, `popForBrowser` (tab-close/nav cleanup), `popExpired` (stale-entry watchdog). Also holds the **ephemeral "allow once" session grants** (`grantSession`, `isSessionGranted`, `clearSessionForBrowser`, `clearSessionForBrowserExceptHost`) keyed by `(browserId, host, type)` — never persisted |
| `PortConfig.h` | **Single source of truth for backend ports.** `namespace hodos`: `IsDevEnv()` (cached `HODOS_DEV==1`), `WalletPort()` → **31401 dev / 31301 release**, `AdblockPort()` → **31402 dev / 31302 release**, the `*Str()` / `*BaseUrl()` / `WalletUrl(path)` / `AdblockUrl(path)` builders, and `IsWalletHostPort(url)` (checks BOTH `localhost:<port>` and `127.0.0.1:<port>`). **Never hardcode a port anywhere else** — the dev port must never leak into a release build. Mirrors `rust-wallet/src/main.rs` / `adblock-engine/src/main.rs` |
| `ProfileImporter.h` | `ProfileImporter` — static utility for detecting and importing bookmarks/history from Chrome, Brave, Edge, and Firefox. Structs: `DetectedProfile`, `ImportResult`. Progress callback support; JSON serialization for IPC (`ResultToJson`, `ProfilesToJson`) |
| `ProfileLock.h` | `AcquireProfileLock()` / `ReleaseProfileLock()` — exclusive file lock on the profile directory to prevent SQLite corruption from concurrent instances |
| `ProfileManager.h` | `ProfileManager` singleton: multi-profile CRUD, color/avatar customization (including base64 `avatarImage`), default + current profile (`SetCurrentProfileId(id, persist)` — persist only on an explicit `--profile=` or picker choice), path helpers, startup-picker toggle, cross-instance launch (`LaunchWithProfile()`, with the Windows `linkParentExitHandle` picker-exit handle), and command-line profile parsing. Struct: `ProfileInfo`. `IsValidProfileId` is inline here and unit-tested by `tests/profile_id_test.cpp` |
| `QRScannerScript.h` | **AUTO-GENERATED** by `cef-native/build_tools/generate-qr-header.js` — do not hand-edit. `QR_SCANNER_SCRIPT[]`: minified jsQR + DOM scanner + BSV pattern filter, split across 10 string literals to dodge the MSVC C2026 16380-char limit. Injected on demand from `simple_handler.cpp` |
| `QRScreenCapture.h` | Windows-only. `StartQRScreenCapture()` / `FinishQRScreenCapture()` — full-screen drag-select overlay, `BitBlt` capture, quirc decode. UI-thread only (GDI). Fallback when the DOM scan returns 0 results |
| `SensitiveCertFields.h` | Header-only pure-logic classifier for always-prompt certificate fields (SSN, passport, DOB, legal name, address, biometrics, financial ids). `NormalizeFieldName`, `KnownSensitiveCertFieldPairs`, `FieldNameMatchesSensitiveHeuristic`, `IsSensitiveCertField`, `AnyRequestedCertFieldSensitive`. Struct: `SensitiveCertFieldPair`. Over-inclusive by design. **Kept in sync BY CODE REVIEW ONLY** with `rust-wallet/src/permission_service/context_builder.rs :: sensitive_cert_fields` — no compile-time linkage. Unit-tested by `tests/sensitive_cert_fields_test.cpp` |
| `SettingsManager.h` | `SettingsManager` singleton: JSON-persisted settings in three structs (`BrowserSettings`, `PrivacySettings`, `WalletSettings`), per-profile `Initialize(profile_path)`, thread-safe getters/setters with auto-save, `ToJson()`/`UpdateFromJson()` for IPC, custom nlohmann `to_json`/`from_json` for `BrowserSettings` (legacy `autoUpdateEnabled` bool → `autoUpdateMode` string; legacy `true` maps to **"notify"**, not "silent"). `autoUpdateMode` is machine-GLOBAL, not per-profile: `SetUpdateModeChangeCallback`, `GlobalUpdateModeWasAbsentAtLoad`, `SetGlobalUpdateModeAuthoritative`, static `ReadModeFromProfileSettings` |
| `SilentStateWriter.h` | `namespace hodos` — mirrors the user's global `autoUpdateMode` into the silent apply-eligibility gate. Pure cross-platform helpers `UpdateModeRank`, `MoreConservativeMode`, `ComputeSilentEligibility` (fail-safe: anything not `"silent"` → `silent=false`); Windows-only `MirrorSilentEligibility()` does the read-modify-write of the GLOBAL `update-state.json`. Unit-tested by `tests/silent_state_writer_test.cpp` |
| `SingleInstance.h` | `namespace SingleInstance` — named-pipe single-instance manager (Windows). `TryAcquireInstance(profileId)`, `SendToRunningInstance(profileId, url)`, `StartListenerThread`, `StopListenerThread`, and the `WM_SINGLE_INSTANCE_NEW_WINDOW` (`WM_APP + 1`) message whose `lParam` is a heap `std::string*` the WndProc **must delete**. Handles the shutdown-relaunch handoff (`"shutting_down"` retry) |
| `SitePermissionStore.h` | **Persisted per-site web-content permission decisions.** `SitePermissionStore` singleton: SQLite, per-profile, `Initialize(user_data_path)` + idempotent `Shutdown()`. `enum class SitePermissionType { Camera=1, Microphone=2, Location=3, Notifications=4, Clipboard=5 }` — **deliberately decoupled** from CEF's bitflag enums so a Chromium bump renumbering `cef_permission_request_types_t` can't corrupt stored rows; mapped only at the callback boundary in `simple_handler.cpp`. `enum class SitePermissionState { Ask=0, Allow=1, Block=2 }` (Ask = absence of a row). API: `GetState`, `SetState`, `ResetDomain`, `GetAllForHost` (JSON for the management UI), static `NormalizeHost`. Browser-process UI thread only → plain `std::mutex` |
| `SyncHttpClient.h` | `SyncHttpClient` — cross-platform synchronous HTTP client (WinHTTP on Windows, libcurl on macOS). Statics: `Get` (×2, plain + custom headers), `Post` (×2), `Download(url, destPath)` (streams large bodies to `<dest>.partial` and renames only on a complete 2xx — used for the ~95 MB installer), and a generic verb dispatch (DELETE/PUT/…) for the wallet IPC bridge. Struct: `HttpResponse`. Takes full URLs — build localhost ones with `hodos::WalletUrl()` / `hodos::AdblockUrl()` |
| `Tab.h` | `Tab` struct: per-tab state including `id`, `window_id` (owning BrowserWindow), `title`, `url`, `favicon_url`, `HWND hwnd` (Windows) / `void* view_ptr` (macOS), browser + handler refs, `is_visible`/`is_loading`/`is_closing`/`can_go_back`/`can_go_forward`/`has_cert_error`, and `created_at`/`last_accessed` timestamps. Two constructors (default, and `(id, url)` which starts `is_loading = true`) |
| `TabManager.h` | `TabManager` singleton: tab lifecycle (create/close/switch), browser registration, state updates from SimpleHandler callbacks, tab reordering, cross-window tab moves (`MoveTabToWindow()`), per-window active tab tracking (`GetActiveTabIdForWindow()`, `GetActiveTabForWindow()`). ⚠️ `Tab::id` ≠ `CefBrowser::GetIdentifier()` — translate via `GetTabIdForBrowserIdentifier` |
| `TaskbarProfile.h` | Windows-only `SetupTaskbarProfile(hwnd, hInstance)` — per-profile AUMID, taskbar overlay icon badge, and badged window icon. Call after `ShowWindow()`; requires COM. No-ops when only one profile exists |
| `UpdateApply.h` | `namespace hodos` — the PURE, cross-process data contract for the Windows apply transaction. `enum class ApplyPhase` + `ApplyPhaseFromString`, `ApplyRecord` (`apply.json`) with `SerializeApplyRecord`/`ParseApplyRecord`, `UpdateState` (GLOBAL `update-state.json`: silent/paused eligibility, anti-rollback high-water, installed-signer thumbprint cache, last failure, rescan) with `SerializeUpdateState`/`ParseUpdateState`, `PausedBlocksStagedBuild`, `FileManifest` (`{relpath → sha256}`) with `SerializeManifest`/`ParseManifest`/`NormalizeManifestKey`. Lenient parses — never throw. Unit-tested by `tests/update_apply_test.cpp` |
| `UpdateFs.h` | `namespace hodos::updatefs` — the filesystem primitives behind backup/verify/restore. `Sha256FileW`, `EnsureDirExists`, `BuildManifestForTree`, `CopyTreeRecursive`, `SnapshotWalletDbSet`/`RestoreWalletDbSet`, `SwapFileReplace`, `WriteFileAtomic`, `ReadFileAll`, `RemoveTree`, `VerifyEd25519`, `VerifyManifestSignature`. Struct: `VerifyResult`. Unit-tested by `tests/update_fs_test.cpp` |
| `UpdateLock.h` | `namespace hodos` — `UpdateLockOwner` (RAII exclusive owner of `update.lock`) + `UpdateLockIsHeld(path)`, an inline non-mutating PROBE used by the honor-gate so a launch can defer while an apply owner holds the lock |
| `UpdateStager.h` | `namespace hodos` — `UpdateStager`: appcast parse + installer download/verify/stage. Structs `AppcastEntry`, `StagedUpdateMarker`; `enum class StageResult`. Unit-tested by `tests/update_stager_test.cpp` |
| `WalletService.h` | `WalletService` — HTTP client to the Rust wallet backend: health check, wallet status/info/create/load/mark-backed-up, address management, transaction lifecycle (create/sign/broadcast/send/balance/history), `makeHttpRequestPublic()` for interceptors, and daemon process management (start/stop/monitor). WinHTTP on Windows (`WalletService.cpp`), libcurl on macOS (`WalletService_mac.cpp`); both resolve the port via `hodos::WalletPort()` |
| `WindowManager.h` | `WindowManager` singleton: manages `BrowserWindow` instances. Window 0 is the main window; multi-window via `CreateWindowRecord()` / `CreateFullWindow()` (both platforms). Lookups by window ID, HWND (`GetWindowByHwnd`), NSWindow (`GetWindowByNSWindow`), or browser ID. Active window tracking via `SetActiveWindowId()`/`GetActiveWindowId()` |

> **Deleted in Phase 2.6-H — do not re-add:** `SessionManager.h` (per-browser spend / rate counters) and `PermissionEngine.h`/`.cpp`. **The permission DECISION engine is now Rust:** `rust-wallet/crates/hodos_permission_engine` (`decide()` in `src/lib.rs`, cascade in `src/matrix_c.rs`), wrapped by `rust-wallet/src/permission_service/` and wired as Actix middleware in `rust-wallet/src/main.rs`. The session spend/rate counters live there too. C++ retains only modal dispatch + resume (`HttpRequestInterceptor.h`, `PendingAuthRequest.h`). `BRC100Bridge.h` / `BRC100Handler.h` are likewise gone — wallet calls go through the `WALLET_CALL_BRIDGE_SCRIPT` IPC bridge and `HttpRequestInterceptor`.

## Architecture Patterns

### Singleton Pattern

Most managers use Meyer's singleton (`static T instance` in `GetInstance()`):
- **Inline `GetInstance()` in the header** (4): `AdblockCache`, `FingerprintProtection`, `PendingRequestManager`, `PendingPermissionManager`
- **`GetInstance()` defined in the `.cpp`**: `BookmarkManager`, `CookieBlockManager`, `HistoryManager`, `SettingsManager`, `ProfileManager`, `WindowManager`, `TabManager`, `EphemeralCookieManager`, `GoogleSuggestService`, `SitePermissionStore`, `PaidContentCache`, `AutoUpdater`

`CookieManager` and `ProfileImporter` are static-only (no instance). `WalletService` is an ordinary constructible class, not a singleton.

All singletons delete the copy constructor/assignment. Most are thread-safe via `std::mutex`. `CookieBlockManager` and `EphemeralCookieManager` use `std::shared_mutex` for read-heavy IO-thread access.

### Data Storage

| Manager | Storage | Database / file |
|---------|---------|-----------------|
| `BookmarkManager` | SQLite | `bookmarks.db` |
| `CookieBlockManager` | SQLite + in-memory sets | `cookie_blocks.db` |
| `HistoryManager` | SQLite | `History` (own DB, not CEF's) |
| `SitePermissionStore` | SQLite | per-profile site-permission DB |
| `PaidContentCache` | SQLite | `<profile>/paid_content_cache.db` (500 MB LRU) |
| `CookieManager` | CEF internal cookie store | N/A (uses CEF APIs) |
| `AdblockCache` | In-memory cache + JSON file | `adblock_settings.json` |
| `FingerprintProtection` | In-memory + JSON file | `fingerprint_settings.json` |
| `SettingsManager` | JSON file | `settings.json` (per-profile + a GLOBAL one for `updateMode`) |
| `ProfileManager` | JSON file | `profiles.json` |
| Auto-updater (`UpdateApply`/`UpdateStager`) | JSON files under `AppPaths::GetUpdateDir()` | `apply.json`, `update-state.json`, `*-manifest.json`, `update.lock` |

### V8 Handlers (Render Process)

These run in the renderer process and communicate via IPC:

| Handler | Header | JavaScript API surface |
|---------|--------|------------------------|
| `IdentityHandler` | `IdentityHandler.h` | Identity certificate operations |
| `AddressHandler` | `AddressHandler.h` | BSV address generation |
| `NavigationHandler` | `NavigationHandler.h` | Back, forward, reload, navigate |

Additional `CefV8Handler` subclasses are defined **inline in `simple_render_process_handler.cpp`**, not in this directory: `CefMessageSendHandler`, `OverlayCloseHandler`, `OmniboxCloseHandler`, `HistoryV8Handler`, `GoogleSuggestV8Handler`.

### Injected JavaScript (not V8 bindings)

**Two** headers ship JS as embedded C string constants, executed via `CefFrame::ExecuteJavaScript` rather than bound as V8 functions. (It was three until 2026-08-09, when `FingerprintScript.h` was deleted — farbling moved into Blink itself, so there is no longer any injected-JS privacy surface.)

| Header | Constant(s) | Injection point |
|--------|-------------|-----------------|
| `CWIShimScript.h` | `WALLET_CALL_BRIDGE_SCRIPT`, `CWI_SHIM_SCRIPT` | `simple_render_process_handler.cpp :: OnContextCreated` (bridge first, then shim; shim only on external main frames) |
| `QRScannerScript.h` | `QR_SCANNER_SCRIPT` | `simple_handler.cpp`, on demand |

Anything interpolated into these must go through `escapeJsonForJs()` (`JsStringEscape.h`).

### HTTP Backend Communication

Two backend services on loopback. **Ports are resolved at runtime via `PortConfig.h` — never hardcoded:**

| Backend | Release port | Dev port (`HODOS_DEV=1`) | Client classes |
|---------|--------------|--------------------------|----------------|
| Rust wallet | 31301 | 31401 | `WalletService`, `HttpRequestInterceptor` (via `AsyncWalletResourceHandler` in the `.cpp`), `ManifestFetcher` |
| Adblock engine | 31302 | 31402 | `AdblockCache` |

`SyncHttpClient` provides the cross-platform abstraction (WinHTTP on Windows, libcurl on macOS) and also handles external `https://` hosts with redirect-following for the auto-updater. `AdblockCache` uses `SyncHttpClient` for `/check` and for both cosmetic endpoints on macOS; Windows keeps its inline WinHTTP transport for the two cosmetic endpoints. (Those two were **stubbed on macOS until 2026-08-12**.) `WalletService` has real Windows + macOS implementations in separate `.cpp` files. `GoogleSuggestService` has WinHTTP and libcurl branches in one `.cpp`.

### Cross-Platform Conditionals

All files use `#ifdef _WIN32` / `#elif defined(__APPLE__)` for platform differences:
- Window handles: `HWND` vs `void*` (NSWindow*/NSView*)
- Click-outside detection: `WH_MOUSE_LL` hooks vs NSEvent local monitors
- Crypto: `CryptGenRandom` vs `SecRandomCopyBytes`
- HTTP: WinHTTP vs libcurl (via `SyncHttpClient`)
- Path separators: `\\` vs `/`

**Windows-only headers** (guarded or with no macOS counterpart): `LayoutHelpers.h`, `QRScreenCapture.h`, `SingleInstance.h`, `TaskbarProfile.h`, `UpdateFs.h`, `UpdateLock.h`, `UpdateStager.h`, and the `MirrorSilentEligibility` half of `SilentStateWriter.h`. `AppPaths.h`'s update-path helpers are Windows-flavored; `UpdateApply.h`'s serializers are pure/cross-platform.

## Key Data Structures

```cpp
// Tab state (Tab.h)
struct Tab { int id; int window_id; std::string title, url, favicon_url;
    HWND hwnd /* win */ | void* view_ptr /* mac */;
    CefRefPtr<CefBrowser> browser; CefRefPtr<SimpleHandler> handler;
    bool is_visible, is_loading, is_closing, can_go_back, can_go_forward, has_cert_error;
    time_point created_at, last_accessed; };

// Per-window state (BrowserWindow.h)
class BrowserWindow { int window_id;
    /* win: hwnd + header_hwnd + legacy webview_hwnd, 14 overlay HWNDs,
       10 mouse hooks, 9 icon offsets
       mac: ns_window + header_view + webview_view, 14 overlay NSWindows,
       9 NSEvent monitors, 9 icon offsets
       both: 18 CefRefPtr<CefBrowser> role refs */
    void SetBrowserForRole(const std::string& role, CefRefPtr<CefBrowser>);
    CefRefPtr<CefBrowser> GetBrowserForRole(const std::string& role) const;
    void ClearBrowserForRole(const std::string& role); };

// Wallet auth/approval queue (PendingAuthRequest.h)
enum class ResumeKind { kHttpCallback, kIpcResponse, kInternal };
struct PendingAuthRequest { std::string requestId, domain, method, endpoint, body, type;
    CefRefPtr<CefResourceHandler> handler;      // iff kHttpCallback
    ResumeKind resumeKind = ResumeKind::kHttpCallback;
    CefRefPtr<CefFrame> frame; int browserId;   // iff kIpcResponse
    std::map<std::string, std::string> headersOnApprove;
    std::string httpMethod = "POST";
    std::string originalIpcRequestId; };        // MUST be set on the IPC path
// type values: "domain_approval", "brc100_auth", "no_wallet", "payment_confirmation",
//              "rate_limit_exceeded", "certificate_disclosure", + scoped-grant types

// Web-content permission prompts (PendingPermissionRequest.h / SitePermissionStore.h)
enum class SitePermissionType { Camera=1, Microphone=2, Location=3,
                                Notifications=4, Clipboard=5 };
enum class SitePermissionState { Ask=0, Allow=1, Block=2 };
struct PendingPermissionRequest { std::string requestId, host; bool isMedia;
    CefRefPtr<CefMediaAccessCallback> mediaCb;      // exactly one of these
    CefRefPtr<CefPermissionPromptCallback> promptCb;
    uint32_t requestedMask; uint64_t promptId; int browserId; int64_t createdAtMs;
    std::vector<SitePermissionType> types; };

// Settings (SettingsManager.h)
struct BrowserSettings { std::string homepage, searchEngine, downloadsPath;
    double zoomLevel; bool showBookmarkBar, restoreSessionOnStart, askWhereToSave;
    std::string autoUpdateMode; };           // "off" | "notify" | "silent" (default silent)
struct PrivacySettings { bool adBlockEnabled, thirdPartyCookieBlocking, doNotTrack,
    clearDataOnExit, fingerprintProtection, paidContentCacheEnabled; };
struct WalletSettings { bool autoApproveEnabled;
    int defaultPerTxLimitCents = 100;        // $1.00 per transaction
    int defaultPerSessionLimitCents = 1000;  // $10.00 per session
    int defaultRateLimitPerMin = 30;
    int defaultMaxTxPerSession = 100;
    bool peerpayAutoAccept = true; };

// Cookie blocking (CookieBlockManager.h)
struct BlockedDomainEntry { std::string domain, source; bool is_wildcard; int64_t created_at; };
struct BlockLogEntry { std::string cookie_domain, page_url, reason; int64_t blocked_at; };

// Ephemeral cookies (EphemeralCookieManager.h)
struct SiteSession { std::string site; int tab_ref_count;
    std::unordered_set<std::string> third_party_domains; bool grace_active; };

// Adblock cosmetic result (AdblockCache.h — nested in AdblockCache)
struct CosmeticResult { std::string cssSelectors, injectedScript; bool generichide; };

// Profile (ProfileManager.h)
struct ProfileInfo { std::string id, name, color, path, createdAt,
    avatarInitial, avatarImage; };

// Auto-update contract (UpdateApply.h)
enum class ApplyPhase { None, Preparing, /* … ordered by progression … */ };
struct ApplyRecord   { /* apply.json — the durable transaction state */ };
struct UpdateState   { bool silent, paused; long highWaterBuild; /* … */ };  // GLOBAL
struct FileManifest  { std::map<std::string, std::string> files; };          // relpath → sha256
```

## Thread Safety Notes

- **UI thread only**: `TabManager`, `CookieManager`, `SitePermissionStore`, `PendingPermissionManager` (CEF permission callbacks are UI-thread; the mutexes are belt-and-suspenders)
- **IO thread reads, UI thread writes**: `CookieBlockManager`, `EphemeralCookieManager` (via `shared_mutex`)
- **Any thread**: `AdblockCache`, `PendingRequestManager`, `FingerprintProtection`, `SettingsManager`, `HistoryManager`, `PaidContentCache` (via `mutex`)
- **Render process only**: `AddressHandler`, `IdentityHandler`, `NavigationHandler` (V8 handlers)
- **Pure / stateless, safe anywhere**: `JsStringEscape.h`, `SensitiveCertFields.h`, `PortConfig.h`, `AppPaths.h`, `LayoutHelpers.h`, `ManifestFetcher::ParseFromJson`, the `UpdateApply.h` serializers, the `SilentStateWriter.h` rank/collapse helpers

## Unit Test Coverage

`cef-native/tests/` builds a single `hodos_tests` GoogleTest target covering the pure-logic headers here:

| Test file | Header/module under test |
|-----------|--------------------------|
| `manifest_fetcher_test.cpp` | `ManifestFetcher.h` (+ `src/core/ManifestFetcher.cpp`) |
| `sensitive_cert_fields_test.cpp` | `SensitiveCertFields.h` (header-only) |
| `js_string_escape_test.cpp` | `JsStringEscape.h` (header-only) |
| `profile_id_test.cpp` | `ProfileManager.h :: IsValidProfileId` (inline) |
| `update_stager_test.cpp` | `UpdateStager.h` |
| `update_apply_test.cpp` | `UpdateApply.h` |
| `update_fs_test.cpp` | `UpdateFs.h` |
| `silent_state_writer_test.cpp` | `SilentStateWriter.h` pure helpers |

`permission_engine_test.cpp` / `permission_gate_test.cpp` were removed in Phase 2.6-H along with the C++ engine; the equivalent coverage lives in the Rust crate.

## Related

- [../../../CLAUDE.md](../../../CLAUDE.md) — root project context (architecture overview, invariants, overlay lifecycle)
- [../../CLAUDE.md](../../CLAUDE.md) — `cef-native/` build instructions, window/process architecture, IPC flow, entry points
- [../../tests/CLAUDE.md](../../tests/CLAUDE.md) — C++ test target details
- Implementations live in `cef-native/src/core/` (e.g., `HistoryManager.cpp`, `HttpRequestInterceptor.cpp`, `SitePermissionStore.cpp`, `UpdateApply.cpp`)
- V8 injection + JS-surface injection in `cef-native/src/handlers/simple_render_process_handler.cpp`
- IPC dispatch, permission callbacks, and context menus in `cef-native/src/handlers/simple_handler.cpp`
- Permission decisions are made in Rust: `rust-wallet/crates/hodos_permission_engine` + `rust-wallet/src/permission_service/`
