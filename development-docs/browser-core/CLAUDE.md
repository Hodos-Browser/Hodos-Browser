# Browser Core Sprint — Context for Claude

**Scope**: Sprints 0-10 from [implementation-plan.md](./implementation-plan.md). Browser features, security/privacy, and wallet polish.

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| [implementation-plan.md](./implementation-plan.md) | Sprint-by-sprint implementation details |
| [mvp-gap-analysis.md](./mvp-gap-analysis.md) | Prioritized gap list with effort estimates |
| [browser-capabilities.md](./browser-capabilities.md) | What works/doesn't/is missing |
| [architecture-assessment.md](./architecture-assessment.md) | Communication patterns, thread safety, debt |
| [01-chrome-brave-research.md](./01-chrome-brave-research.md) | Chrome/Brave research findings |
| [doc-discrepancies.md](./doc-discrepancies.md) | Stale documentation tracker |
| [file-inventory.md](./file-inventory.md) | Complete runtime file/DB/singleton catalog |
| [auth-cookies-profiles-guide.md](./auth-cookies-profiles-guide.md) | **AUTH BIBLE** — Google OAuth, Brave comparison, cookie architecture, sign-in debugging |

---

## Sprint Status Tracker

| Sprint | Name | Status |
|--------|------|--------|
| 0 | Safety & Quick Wins | **Complete** |
| 1 | SSL Certificate Handling + Secure Indicator | **Complete** |
| 2 | Permission Handler | Pending |
| 3 | Download Handler | **Complete** |
| 4 | Find-in-Page | **Complete** |
| 5 | Context Menu Enhancement | **Complete** |
| 6 | JS Dialog Handler + Keyboard Shortcuts | **Complete** |
| 7 | Light Wallet Polish | Pending |
| 8 | Ad & Tracker Blocking | **Complete** (8a-8f) |
| 9 | Settings Persistence + Profile Import | **Complete** |
| 10 | Scriptlet Compatibility System | **Complete** (10a-10c) |
| 11 | Menu Button UX + Full-Page Settings | **Complete** (11a-11b) |
| 12 | Fingerprint Protection | **Complete** (12c-12e) |
| 13 | Tab Tear-Off (Multi-Window) | **Planning** |

---

## Cross-Platform Rules (CRITICAL)

Every sprint MUST follow these rules to avoid accumulating macOS debt:

1. **Platform conditionals on all new C++ code**:
   ```cpp
   #ifdef _WIN32
       // Windows implementation
   #elif defined(__APPLE__)
       // macOS implementation (stub or TODO is OK)
   #endif
   ```

2. **No raw WinHTTP in new singletons** — use a `SyncHttpClient` abstraction (or at minimum, wrap with `#ifdef` and a macOS `#elif` using libcurl/NSURLSession). Existing WinHTTP singletons (`DomainPermissionCache`, `BSVPriceCache`, `WalletStatusCache`) will be refactored during the macOS sprint.

3. **File paths**: Use `SHGetFolderPathW(CSIDL_APPDATA)` on Windows (already in use), `NSSearchPathForDirectoriesInDomains` on macOS. Never hardcode `%APPDATA%` in new code — use the existing path resolution helper.

4. **New overlays**: Any new HWND-based overlay (e.g., downloads panel) needs a corresponding macOS creation stub in `cef_browser_shell_mac.mm`.

5. **Keyboard shortcuts**: Define with both `Ctrl+X` (Windows) and `Cmd+X` (macOS) variants. Use the platform key macro.

6. **CEF helper bundles**: No action needed per-sprint, but be aware: macOS requires 5 helper `.app` bundles. Any new subprocess types need entries.

---

## Windows Build Pitfalls

Common issues encountered during builds:

| Issue | Cause | Fix |
|-------|-------|-----|
| `'XXX' is not a member of 'ClassName'` when calling Windows API | Windows API names are macros (e.g., `CopyFile` → `CopyFileW`) that conflict with method names | Rename methods to avoid conflicts: `CopyFilePortable`, `CreateWindowSafe`, etc. |
| `LOG_INFO_MAIN` not found | Each module defines its own log macros | Define local macros: `#define LOG_INFO_XX(msg) Logger::Log(msg, 1, MODULE_ID)` |
| Include file not found | Wrong include path pattern | Use relative paths from source file: `#include "../../include/core/Foo.h"` |
| vcpkg package not found | VCPKG_ROOT not set | Set env: `$env:VCPKG_ROOT = "C:/Users/archb/Dev/vcpkg"` |

**Windows API Macro Conflicts** (common culprits):
- `CopyFile` → `CopyFileA`/`CopyFileW`
- `CreateWindow` → `CreateWindowA`/`CreateWindowW`
- `DeleteFile` → `DeleteFileA`/`DeleteFileW`
- `GetMessage` → `GetMessageA`/`GetMessageW`
- `SendMessage` → `SendMessageA`/`SendMessageW`

When calling Windows APIs from within a method that might conflict, use `::` scope: `::CopyFileA(...)`.

---

## Key Architecture Patterns for This Sprint

### SimpleHandler is the Hub
`simple_handler.cpp` implements 11 CEF handler interfaces (CefClient + 10 handlers). New handlers are added here by:
1. Adding `public CefXxxHandler` to the class declaration in `simple_handler.h`
2. Adding `GetXxxHandler() override { return this; }` method
3. Implementing the handler methods

### HTTP Interception Flow
```
Web request → OnBeforeResourceLoad (HttpRequestInterceptor.cpp)
  → Is it a wallet endpoint? → AsyncWalletResourceHandler
    → Is domain approved? → DomainPermissionCache check
      → Is it a payment endpoint? → Auto-approve engine (spending limits, rate limits)
        → Forward to Rust (localhost:3301) via CefURLRequest on IO thread
```

### IPC Pattern (C++ ↔ React)
```
React → cefMessage.send("command_name", data)
  → CefProcessMessage to browser process
    → simple_handler.cpp OnProcessMessageReceived
      → dispatch by message name
```

### CEF Threading Model
- **UI thread**: Browser creation, window management, IPC dispatch
- **IO thread**: HTTP interception, CefURLRequest, resource handlers
- **Renderer process**: V8 injection, JavaScript context (separate process!)
- **RULE**: Never block the UI thread. Use `CefPostTask(TID_IO, ...)` for async work.

---

## Sprint-Specific Notes

### Sprint 0 (Safety)
- BSVPriceCache at `HttpRequestInterceptor.cpp` ~line 50 — the `fetchPrice()` method returns 0.0 on error. Fix: cache `lastSuccessfulPrice_`, return sentinel -1.0 if never fetched.
- `price_cache.rs` — similar pattern. `get_cached()` should return `Option<f64>`.
- WebRTC flag goes in `cef_browser_shell.cpp` where other `--switches` are set.

### Sprint 1 (SSL)
- `OnCertificateError` uses `CefCallback` (not `CefRequestCallback`). Check CEF 136 API.
- Padlock indicator: header bar is React (port 5137), SSL status is in C++. Need IPC bridge.

### Sprint 2 (Permissions)
- CEF 136 Chrome bootstrap: returning `false` from `OnShowPermissionPrompt` shows native Chrome permission UI. Test this FIRST before building custom UI.

### Sprint 3 (Downloads) — COMPLETE
- Full implementation: `CefDownloadHandler` + overlay panel + progress icon + toast notifications. See `implementation-plan.md` Sprint 3 for details.

### Sprint 5 (Context Menus) — COMPLETE
- **All custom IDs**: CEF built-in menu IDs (`MENU_ID_BACK`, `MENU_ID_COPY`, etc.) auto-disable when used after `model->Clear()`. All 11 commands use `MENU_ID_USER_FIRST` range instead. See working-notes.md #8.
- Editing commands use `frame->ExecuteJavaScript("document.execCommand('copy')")` etc.
- `CreateNewTabWithUrl()` and `CopyTextToClipboard()` helpers added (cross-platform).

### Sprint 6 (JS Dialog + Shortcuts) — COMPLETE
- Chrome bootstrap handles `alert()`, `confirm()`, `prompt()` natively — no custom `OnJSDialog` needed.
- `OnBeforeUnloadDialog` added: auto-allows navigation (suppresses malicious beforeunload traps).
- 5 new keyboard shortcuts: Ctrl+H (history tab), Ctrl+J (download panel), Ctrl+D (bookmark), Alt+Left/Right (back/forward). All cross-platform with `#ifdef __APPLE__` / `EVENTFLAG_COMMAND_DOWN`.
- Many shortcuts already work natively (Ctrl+P print, zoom, DevTools) — only intercepted the ones that opened `chrome://` pages in separate windows.

### Sprint 10 (Scriptlet Compatibility) — COMPLETE
- **10a**: `hodos-unbreak.txt` exception list with `#@#+js()` blanket scriptlet exceptions for auth sites (x.com, google.com, github.com, microsoft.com, apple.com, etc.). Loaded by adblock engine alongside filter lists.
- **10b**: Per-site scriptlet toggle — `scriptlets_enabled` column in `domain_permissions` (migration V6). Rust endpoints: `GET/POST /adblock/scriptlet-toggle` on port 3301. C++ IPC handler `adblock_scriptlet_toggle`. `AdblockCache::fetchCosmeticResources()` takes `skipScriptlets` parameter. Three call sites updated in `simple_handler.cpp` (OnBeforeBrowse, OnLoadingStateChange pre-cache, OnLoadingStateChange Phase 1).
- **10c**: Privacy Shield panel scriptlet toggle row. `useAdblock.ts` exposes `scriptletsEnabled`/`toggleScriptlets`/`checkScriptlets`. Toggle disabled when adblock is off.

### Sprint 11 (Menu Button + Settings) — COMPLETE
- **11a**: Three-dot menu (`MenuOverlay.tsx`) replaces History + Settings buttons. Sections: Tab, Content, Page Actions, Developer, Settings/Exit. Inline zoom controls with +/- and percentage display. Keyboard shortcut labels. C++ IPC handlers: `print`, `devtools`, `zoom_in`, `zoom_out`, `zoom_reset`, `exit`.
- **11a**: Full-page settings (`SettingsPage.tsx`) with 240px sidebar. 5 sections: General, Privacy, Downloads, Wallet, About. Shared `SettingsCard`/`SettingRow` components. Route: `/settings-page/:section`.
- **11b**: Wallet settings section (auto-approve, spending limits). Wired homepage setting to `tab_create` IPC. Wired DNT/GPC headers in `GetResourceRequestHandler`.
- **New files**: `MenuOverlay.tsx`, `SettingsPage.tsx`, `SettingsCard.tsx`, `GeneralSettings.tsx`, `PrivacySettings.tsx`, `DownloadSettings.tsx`, `AboutSettings.tsx`, `WalletSettings.tsx`

### Sprint 12 (Fingerprint Protection) — COMPLETE
- **12c**: `FingerprintProtection.h` singleton — session token from platform CSPRNG (Windows `CryptGenRandom`, macOS `SecRandomCopyBytes`, fallback `mt19937`). `GetDomainSeed()` hashes session token with domain for deterministic per-domain seeds. Seed IPC: `OnBeforeBrowse` sends seed → renderer caches in `s_domainSeeds` → `OnContextCreated` injects.
- **12d**: `FingerprintScript.h` — embedded JS with Mulberry32 PRNG. Canvas farbling (getImageData, toDataURL, toBlob for canvases <65536px), WebGL spoofing (generic UNMASKED_VENDOR/RENDERER, readPixels), Navigator overrides (hardwareConcurrency 2-8, deviceMemory 8, plugins empty), AudioContext farbling. **NO screen resolution spoofing** (intentional — Brave removed it, breakage > entropy).
- **12e**: `fingerprintProtection` added to `PrivacySettings` struct (SettingsManager), defaults to `true`. Toggle in `PrivacySettings.tsx` settings page. Read-only "Fingerprint shield" row in Privacy Shield panel. `settings_set` IPC handler wired.
- **Test checklist**: [sprint-10-11-12-test-checklist.md](./sprint-10-11-12-test-checklist.md)

### Sprint 9 (Settings + Import + Profiles) — COMPLETE
- **Detailed plan**: [sprint-9-implementation-plan.md](./sprint-9-implementation-plan.md)
- **Research**: [sprint-9-profile-account-research.md](./sprint-9-profile-account-research.md) — Chrome/Firefox/Safari/Edge profile UX patterns
- **9a Settings**: New `SettingsManager` singleton, JSON file at `%APPDATA%/HodosBrowser/settings.json`, IPC for React UI
- **9b Import**: `ProfileImporter` class, auto-detect Chrome/Brave/Edge profiles, import bookmarks (JSON parse) and history (SQLite copy-then-read)
- **9c Clear Data**: `DataClearer` class, clear history/cache/cookies with time range options
- **9d Multi-Profile**: `ProfileManager` singleton, `profiles.json` metadata, profile picker UI, header profile indicator with dropdown
- **Key gotcha**: Chrome locks its DB while running — must copy file before reading
- **Chrome timestamp conversion**: `(chrome_timestamp / 1000000) - 11644473600LL` = Unix epoch
- **Decision**: Wallet is shared across all profiles (not per-profile) for MVP
- **New files**: `SettingsManager.h/cpp`, `ProfileImporter.h/cpp`, `DataClearer.h/cpp`, `ProfileManager.h/cpp`, `ProfilePickerOverlayRoot.tsx`

### Sprint 8 (Ad Blocking) — COMPLETE
- `adblock-engine/` is a **separate Rust project** at repo root (NOT in `rust-wallet`). Runs on **port 3302**.
- **Crate pinning**: `adblock = "=0.10.3"` (0.10.4+ needs unstable Rust feature), `rmp = "=0.8.14"`, `default-features = false` (keeps `Send+Sync` by disabling `unsync-regex-caching`). Serialization uses `.serialize()` not `.serialize_raw()`.
- C++ starts it via `StartAdblockServer()` (same `CreateProcessA` + Job Object pattern as the wallet).
- C++ calls `POST localhost:3302/check` via sync WinHTTP with `AdblockCache` singleton (per-URL result cache + per-browser blocked count tracking).
- Hook point: `GetResourceRequestHandler()` in `simple_handler.cpp`, BEFORE wallet interception check.
- **Per-site toggle (8c)**: `adblock_enabled` column in `domain_permissions` table (migration V5). C++ checks `DomainPermissionCache` before adblock check. Frontend shield icon (`SecurityIcon`) with blocked count badge + dropdown toggle.
- IPC: `adblock_get_blocked_count`, `adblock_reset_blocked_count`, `adblock_site_toggle` → renderer response handlers.
- Rust endpoints: `GET/POST /adblock/site-toggle` on port 3301 (wallet backend manages DB).
- **8d (COMPLETE)**: Filter list auto-update — background tokio task every 6 hours, parses `! Expires:` headers from filter lists, `needs_update()` check, `rebuild_engine()` hot-swap under `RwLock`. Engine version counter in `/check` + `/status` responses; C++ `AdblockCache` detects version change on cache-miss calls and invalidates URL cache.
- **8e (COMPLETE)**: Three-layer ad blocking:
  - **Cosmetic CSS filtering**: Hostname-specific + generic selectors injected via IPC. Two-phase: Phase 1 on page load, Phase 2 after DOM class/ID collection.
  - **Scriptlet injection**: uBlock scriptlets.js (pinned to v1.48.4) + 6 bundled extra scriptlets. Pre-cached via `OnBeforeBrowse` (timing fix), injected in `OnContextCreated` before page JS, fallback in `OnLoadingStateChange`.
  - **CefResponseFilter** (`AdblockResponseFilter`): Network-level YouTube ad blocking. Buffers YouTube API JSON and main-frame HTML responses, renames ad-configuration keys (`adPlacements` → `adPlacements_`, etc.). Primary YouTube ad blocking mechanism — no timing issues since it runs before JS sees the data.
- **8f (COMPLETE)**: Unified Privacy Shield Panel — single shield icon merging adblock + cookie blocking. OSR overlay panel with master toggle, individual toggles, blocked counts. Known UI polish issues deferred to `ux-ui-cleanup.md`.
- Filter lists stored in `%APPDATA%/HodosBrowser/adblock/`.
- Non-critical: if engine fails to start, browsing works without ad blocking.
- **Performance note**: CefResponseFilter buffering adds YouTube page load latency. Optimization opportunities tracked in `ux-ui-cleanup.md`.

### Sprint 13 (Tab Tear-Off / Multi-Window) — Planning
- **Goal**: Allow users to drag a tab out of the browser window to create a new independent window, and drag tabs between windows.
- **Key findings from research**: HWND reparenting works in CEF (confirmed). Must stay single-process (SingletonLock). All singletons shared automatically. `Tab.window_id` field is minimal approach.
- **Architecture needed**: `BrowserWindow` class to encapsulate per-window state + `WindowManager` singleton to track all windows. Current global HWNDs (`g_hwnd`, `g_header_hwnd`, `g_webview_hwnd`) must be refactored into per-window instances.
- **macOS**: Needs NSWindow management equivalent in `cef_browser_shell_mac.mm`.
- **Depends on**: No hard blockers, but benefits from stable overlay system (Sprints 10-12 complete).

---

## Files Most Likely to Change

| File | Sprints |
|------|---------|
| `simple_handler.h` | 1, 2, 3, 4, 6 |
| `simple_handler.cpp` | 1, 2, 3, 4, 5, 6 |
| `HttpRequestInterceptor.cpp` | 0, 8, 8c, 10 |
| `cef_browser_shell.cpp` | 0, 1, 6, 9 |
| `simple_render_process_handler.cpp` | 10 |
| `frontend/src/components/MainBrowserView.tsx` | 1, 4 |
| `frontend/src/components/WalletPanel.tsx` | 7 |
| `frontend/src/components/TransactionForm.tsx` | 7 |
| `rust-wallet/src/price_cache.rs` | 0 |

---

## Testing Approach

- User runs the browser to test — do not attempt to run it
- After C++ changes: `cmake --build build --config Release` in `cef-native/`
- After Rust changes: `cargo check` or `cargo build --release` in `rust-wallet/`
- After frontend changes: `npm run build` in `frontend/`
- Each sprint has a verification checklist in `implementation-plan.md`

### Test Site Basket

See [test-site-basket.md](./test-site-basket.md) for standard verification sites.

**Minimum after any change:** youtube.com, x.com, github.com (5 min)
**After sprint completion:** Full "Standard" basket (15 min)
**Before release/demo:** Full basket, all categories (30-45 min)

---

## Continuous Improvement Directive

**After each sprint, phase, or sub-phase:**
1. Review this CLAUDE.md — Is it still accurate? Update stale sections.
2. Update Sprint Status Tracker — Mark completed work.
3. Add new patterns/discoveries (key gotchas, timing issues, workarounds).
4. Update test-site-basket.md if new test cases identified.
5. Check if main CLAUDE.md (repo root) needs key file updates.

**Goal:** Context files should always reflect current reality. They're the institutional memory that lets any AI (or human) pick up where the last session left off.
