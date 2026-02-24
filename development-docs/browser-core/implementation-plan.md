# MVP Implementation Plan

**Created**: 2026-02-19
**Status**: Complete (Phase D)
**Purpose**: Break the prioritized gap list (Phase C) into sprint-sized implementation chunks with dependencies, ordering, and architecture decisions.

---

## Sprint Structure

Each sprint is 1-3 days. Sprints are numbered by tier and sequence. Sprints within a tier can often be parallelized.

---

## Sprint 0: Safety & Quick Wins (1 day) ✅ COMPLETE

**Goal**: Fix the safety bug and apply all trivial one-line improvements.

| Task | Ref | Effort | Files |
|------|-----|--------|-------|
| Fix BSVPriceCache to cache last successful price | C.2.7 | 2 hrs | `HttpRequestInterceptor.cpp` (BSVPriceCache class) |
| Fix Rust price_cache to cache last successful price | C.2.7 | 1 hr | `rust-wallet/src/price_cache.rs` |
| Add WebRTC leak prevention flag | C.2.2 | 5 min | `cef_browser_shell.cpp` |
| Remove `--allow-running-insecure-content` flag | C.2.6 | 5 min | `cef_browser_shell.cpp` (verify internal pages first) |

### BSVPriceCache Fix — Design

**C++ (BSVPriceCache)**:
```
Current:  if (fetch fails) → return 0.0
Proposed: if (fetch fails) → return lastSuccessfulPrice_
          if (never fetched) → return -1.0 (sentinel)
```
Auto-approve engine: if price <= 0, treat as "price unavailable" → require user approval for all payment endpoints (never auto-approve).

**Rust (PriceCache)**:
Same pattern — `get_cached()` returns `Option<f64>` instead of `f64`. Callers that get `None` should deny or require approval.

### macOS Notes
- BSVPriceCache uses WinHTTP — existing singleton, will be refactored in macOS sprint. No new WinHTTP code here, just logic changes.
- WebRTC flag and mixed content flag: CEF command-line switches are cross-platform. Same code works on macOS.

### Verification
- [x] Build Rust (`cargo check`) — compiles clean
- [ ] Build C++ (`cmake --build`) — user to build on Windows
- [ ] Test: disconnect internet → make payment → should NOT auto-approve (should show notification)

---

## Sprint 1: SSL Certificate Handling + Secure Connection Indicator (1-2 days)

**Goal**: Handle SSL cert errors properly and show HTTPS status in the header.

### 1a: SSL Certificate Error Handler

**Add to `SimpleHandler`**:
- Inherit `CefRequestHandler::OnCertificateError` (already inherits `CefRequestHandler`)
- On cert error: show interstitial warning page with "Go Back" (default) and "Proceed Anyway" (advanced)
- Store `CefRefPtr<CefCallback>` to call `Continue()` if user proceeds

**Interstitial approach**: Load a data URL with inline HTML/CSS (simplest — no new React route needed). Include cert error type, domain name, and "Advanced" expandable section with cert details.

**Alternative**: React route at `/cert-error?domain=X&error=Y` — more polished but requires IPC roundtrip.

**Recommendation**: Inline data URL for MVP. Can upgrade to React route later.

### 1b: Secure Connection Indicator

**Header bar changes**:
- Add padlock icon to the left of the URL in the header
- States: 🔒 HTTPS (green/default), ⚠️ HTTP (yellow warning), 🔓 Cert error (red)
- URL scheme information from `OnAddressChange` callback (already fires on every navigation)
- Click on padlock → show connection info popup (domain, issuer, cert validity)

**IPC needed**: Forward SSL status from C++ to React header via existing `cefMessage.send()` pattern.

### macOS Notes
- `OnCertificateError` is a CEF API — fully cross-platform. Implementation goes in `simple_handler.cpp` which is shared.
- Padlock indicator: SSL status IPC to React header is cross-platform. No platform-specific code needed.
- Interstitial data URL: cross-platform.
- SSL status forwarding in `cef_browser_shell.cpp`: if adding to the header HWND, use `#ifdef _WIN32` / `#elif __APPLE__` with corresponding `cef_browser_shell_mac.mm` equivalent. If the padlock is purely React-based (recommended), no platform code needed.

### Files
- `simple_handler.h` — Add `OnCertificateError` override declaration
- `simple_handler.cpp` — Implement `OnCertificateError`
- `cef_browser_shell.cpp` — Forward SSL status to header
- `frontend/src/components/MainBrowserView.tsx` — Add padlock icon
- Inline HTML template for cert error interstitial

### Verification
- [ ] Visit a site with expired cert → interstitial appears
- [ ] Visit HTTPS site → padlock shows locked
- [ ] Visit HTTP site → padlock shows warning
- [ ] "Proceed Anyway" → site loads

---

## Sprint 2: Permission Handler (0.5 day)

**Goal**: Camera, mic, geolocation, and notification permission prompts work.

### Implementation

**Add `CefPermissionHandler` to `SimpleHandler`**:

```cpp
// simple_handler.h
class SimpleHandler : public CefClient,
                      // ... existing handlers ...
                      public CefPermissionHandler {
public:
    CefRefPtr<CefPermissionHandler> GetPermissionHandler() override { return this; }

    bool OnRequestMediaAccessPermission(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefFrame> frame,
        const CefString& requesting_origin,
        uint32_t requested_permissions,
        CefRefPtr<CefMediaAccessCallback> callback) override {
        // Return false → Chrome bootstrap shows native permission UI
        return false;
    }

    bool OnShowPermissionPrompt(
        CefRefPtr<CefBrowser> browser,
        uint64_t prompt_id,
        const CefString& requesting_origin,
        uint32_t requested_permissions,
        CefRefPtr<CefPermissionPromptCallback> callback) override {
        // Return false → Chrome bootstrap shows native permission bubble
        return false;
    }
};
```

That's the entire implementation for MVP. Chrome's native UI handles the rest.

### macOS Notes
- `CefPermissionHandler` is a CEF API — fully cross-platform. Returning `false` works identically on macOS.
- No platform-specific code needed for this sprint.

### Files
- `simple_handler.h` — Add handler declarations
- `simple_handler.cpp` — Implement (return false)

### Verification
- [ ] Visit a site requesting camera (e.g., appear.in, meet.google.com) → Chrome permission prompt appears
- [ ] Allow → camera works
- [ ] Block → camera denied
- [ ] Visit a site requesting geolocation → permission prompt appears

---

## Sprint 3: Download Handler — COMPLETE (2026-02-21)

**Goal**: Users can download files with progress tracking, cancel/pause/resume.

### Implementation Summary

**C++ (`simple_handler.h/cpp`)**:
- `CefDownloadHandler` added as 9th interface on `SimpleHandler`
- `CanDownload` → returns `true`; `OnBeforeDownload` → `callback->Continue("", true)` (Save As dialog)
- `OnDownloadUpdated` → tracks state in `std::map<uint32_t, DownloadInfo>`. Skips updates until user saves (empty `full_path` = Save As still open). Manual pause tracking via `paused_downloads_` set (CEF 136 has no `IsPaused()`)
- `NotifyDownloadStateChanged()` sends JSON state to both header browser and download panel browser via `CefProcessMessage`
- 10 IPC handlers: `download_panel_show/hide`, `download_cancel/pause/resume/open/show_folder/clear_completed/get_state`
- `download_open` uses `ShellExecuteW` (Windows), `download_show_folder` opens parent directory

**C++ overlay (`cef_browser_shell.cpp` + `simple_app.cpp`)**:
- `g_download_panel_overlay_hwnd` + `g_download_panel_mouse_hook` globals
- `DownloadPanelOverlayWndProc` (mouse forwarding, `MA_NOACTIVATE`, scroll)
- `DownloadPanelMouseHookProc` (click-outside dismiss)
- `CreateDownloadPanelOverlay` / `ShowDownloadPanelOverlay` / `HideDownloadPanelOverlay` in `simple_app.cpp`
- 380x400 panel, positioned under download icon, keep-alive pattern
- WM_SIZE and WM_MOVE repositioning, shutdown cleanup

**Frontend**:
- `useDownloads.ts` hook — listens for `download_state_update`, exposes control functions
- `DownloadsOverlayRoot.tsx` — overlay page at `/downloads` route with progress bars, pause/resume/cancel, open/show-in-folder, clear completed (auto-closes overlay when list empty)
- `MainBrowserView.tsx` — download icon with `CircularProgress` ring (determinate/indeterminate), green when all complete, toast notifications on start/complete

**Render process** (`simple_render_process_handler.cpp`):
- `download_state_update` forwarded to React via `window.dispatchEvent`

### macOS Notes
- `CefDownloadHandler` is fully cross-platform. Save As dialog is native on both platforms.
- Download overlay HWND uses `#ifdef _WIN32` — needs macOS `NSWindow` stub in `cef_browser_shell_mac.mm`
- `download_open`/`download_show_folder` use `ShellExecuteW` — need macOS `NSWorkspace` implementation

### Files Changed
- `simple_handler.h` — Added `CefDownloadHandler` interface, `DownloadInfo` struct, download map, paused set
- `simple_handler.cpp` — Handler methods + 10 IPC handlers + `NotifyDownloadStateChanged`
- `simple_app.cpp` — Create/Show/Hide overlay functions
- `cef_browser_shell.cpp` — WndProc, mouse hook, window class, shutdown, resize/move
- `simple_render_process_handler.cpp` — `download_state_update` forwarding
- `frontend/src/hooks/useDownloads.ts` — NEW
- `frontend/src/pages/DownloadsOverlayRoot.tsx` — NEW
- `frontend/src/pages/MainBrowserView.tsx` — Download icon + progress + toasts
- `frontend/src/App.tsx` — `/downloads` route

### Verification
- [x] Click a download link → Save As dialog appears
- [x] File downloads to selected location
- [x] Download icon appears with circular progress ring after Save
- [x] Progress accurately reflects download progress
- [x] Click icon → overlay opens with download details
- [x] Pause/Resume/Cancel work correctly
- [x] Completed download → icon turns green, "Open"/"Show in folder" work
- [x] "Clear completed" removes items and closes overlay
- [x] Toast notifications on download start and completion

---

## Sprint 4: Find-in-Page (1-2 days) ✅ COMPLETE

**Goal**: Ctrl+F opens a find bar with match count, prev/next navigation.

### Implementation (Actual)

**CEF 136 Find API non-functional**: `CefBrowserHost::Find()` calls succeed but `GetFindHandler()` is never queried by CEF internals, and `OnFindResult` never fires. This was verified after a full wrapper rebuild (stale CMakeCache was pointing to old machine path for 5 months — see working-notes.md #7). CefFindHandler interface left in SimpleHandler but unused. Possibly a CEF 136 regression or windowed-mode limitation — needs investigation with cefclient sample app.

**JavaScript `window.find()` fallback**: Used Chromium's built-in (non-standard) `window.find()` API instead.

**Find bar**: React `FindBar.tsx` component rendered as inline flex item inside `<Toolbar>` in MainBrowserView.

**State flow** (actual):
1. Ctrl+F in tab → `OnPreKeyEvent` sends `find_show` to header browser → React shows FindBar
2. Ctrl+F in header → `useKeyboardShortcuts` shows FindBar directly
3. User types → IPC `find_text` → C++ injects JavaScript into active tab:
   - Injects `::selection { background: #FFFF00 !important; }` CSS (selection renders grey when tab unfocused)
   - Counts matches via `window.find()` loop with `wrapAround=false` (prevents infinite loop)
   - Navigates with `window.find()` using `wrapAround=true` for cycling
   - Tracks ordinal with simple counter (increment/decrement with wrap)
   - Sends `find_result_js` via cefMessage back to C++
4. C++ forwards `find_result_js` as `find_result` to header browser → render process → React updates "X of Y"
5. Close/Escape → IPC `find_stop` → C++ clears selection, removes injected CSS, deletes state variables

### macOS Notes
- `window.find()` is a Chromium built-in — works cross-platform.
- Find bar is a React component — fully cross-platform, no platform code needed.
- `Ctrl+F` shortcut: `#ifdef __APPLE__` → `EVENTFLAG_COMMAND_DOWN` / `#else` → `EVENTFLAG_CONTROL_DOWN`.

### Files Changed
- `simple_handler.h` — Added `CefFindHandler` (10th interface, unused by CEF but kept)
- `simple_handler.cpp` — `OnPreKeyEvent` Ctrl+F, `find_text` JS injection, `find_result_js` forwarding, `find_stop` cleanup
- `simple_render_process_handler.cpp` — Forward `find_show` and `find_result` to React
- `frontend/src/components/FindBar.tsx` — New component (inline in Toolbar)
- `frontend/src/pages/MainBrowserView.tsx` — FindBar state + event listeners
- `frontend/src/hooks/useKeyboardShortcuts.ts` — Added `onFindInPage` handler

### Verification
- [x] Ctrl+F → find bar appears in toolbar
- [x] Type text → matches highlighted yellow on page, "X of Y" shown
- [x] Enter/Next → cycles forward through matches
- [x] Shift+Enter/Prev → cycles backward
- [x] Escape → find bar closes, highlights cleared

---

## Sprint 5: Context Menu Enhancement (1 day) ✅ COMPLETE

**Goal**: Right-click menu has all standard browser actions.

### Implementation (Actual)

Rebuilt `OnBeforeContextMenu` to detect context via `CefContextMenuParams::GetTypeFlags()` flags (`CM_TYPEFLAG_LINK`, `CM_TYPEFLAG_SELECTION`, `CM_TYPEFLAG_EDITABLE`, `CM_TYPEFLAG_MEDIA` + `CM_MEDIATYPE_IMAGE`) and construct context-appropriate menus. Cleared default Chromium menu (`model->Clear()`) for full control. Non-tab browsers (header, overlays) get only "Inspect Element".

**Menu items by context (actual)**:

| Context | Items |
|---------|-------|
| Page (no selection) | Back, Forward, Reload, Separator, Select All, View Page Source, Separator, Inspect |
| Text selected | Copy, Separator, Select All, Separator, Inspect |
| Link | Open Link in New Tab, Copy Link Address, Separator, Inspect |
| Image | Save Image As, Copy Image Address, Open Image in New Tab, Separator, Inspect |
| Editable field | Undo, Redo, Separator, Cut, Copy, Paste, Delete, Separator, Select All, Separator, Inspect |
| Link + Image (combined) | Link items, Image items, Separator, Inspect |

**Custom command IDs**: ALL menu items use `MENU_ID_USER_FIRST` range (26500+). 11 custom IDs: `+1` Inspect, `+2` Open Link New Tab, `+3` Copy Link Address, `+4` Save Image As, `+5` Copy Image URL, `+6` Open Image New Tab, `+10` Back, `+11` Forward, `+12` Reload, `+13`-`+19` Undo/Redo/Cut/Copy/Paste/Delete/Select All, `+20` View Source. **CEF built-in IDs intentionally avoided** — see working-notes.md #8 for the quirk where `model->Clear()` + built-in IDs causes CEF's internal command updater to auto-disable all items.

**All commands handled manually in `OnContextMenuCommand`**: Navigation via `browser->GoBack()`/`GoForward()`/`Reload()`. Editing via `frame->ExecuteJavaScript("document.execCommand('copy')")` etc. This gives full control and avoids CEF's internal state management.

**Helpers added**:
- `CreateNewTabWithUrl(url)` — cross-platform tab creation (extracts shared logic from duplicated `tab_create` / command-50100 patterns)
- `CopyTextToClipboard(text)` — Windows: `OpenClipboard`/`SetClipboardData(CF_TEXT)`. macOS: pipe to `pbcopy` (safe from injection — uses `popen`/`fwrite`, not shell escaping)

**Back/Forward enablement**: `model->SetEnabled(MENU_ID_CUSTOM_BACK, browser->CanGoBack())` — greyed out when no history.

**Save Image As**: `browser->GetHost()->StartDownload(sourceUrl)` triggers existing `CefDownloadHandler` (Sprint 3) — Save As dialog and progress tracking come for free.

**View Page Source**: Opens `view-source:` + current URL in a new tab via `CreateNewTabWithUrl()`.

**Chromium command 50100**: Still intercepted (merged with `MENU_ID_OPEN_LINK_NEW_TAB`) for compatibility if CEF ever injects its own "Open in new tab" item.

### macOS Notes
- Context menu APIs (`OnBeforeContextMenu`, `OnContextMenuCommand`) are cross-platform CEF.
- `CreateNewTabWithUrl` uses `#ifdef _WIN32` / `#else` with existing `g_webview_view` + `GetViewDimensions()` pattern.
- `CopyTextToClipboard` uses `popen("pbcopy", "w")` on macOS — safe, no shell escaping.
- All commands handled manually — fully cross-platform (no CEF built-in ID dependency).

### Files Changed
- `simple_handler.cpp` — Rebuilt `OnBeforeContextMenu` (context-aware menu building), rebuilt `OnContextMenuCommand` (17 custom handlers), added `CreateNewTabWithUrl()` and `CopyTextToClipboard()` helpers, added 11 named command ID constants

### Verification
- [x] Right-click on text → Copy works
- [x] Right-click on link → "Open in New Tab" and "Copy Link Address" work
- [x] Right-click on image → "Save Image As" triggers download
- [x] Right-click in text field → Cut/Copy/Paste work
- [x] "View Page Source" opens `view-source:` URL in new tab
- [x] Right-click on empty page → Back/Forward greyed when no history
- [x] Right-click in header/overlay → only "Inspect Element" shown

---

## Sprint 6: JS Dialog Handler + Keyboard Shortcuts (0.5 day) — COMPLETE

**Goal**: JavaScript alert/confirm/prompt work properly. Additional keyboard shortcuts.

### 6a: JS Dialog Handler — Complete

**Tested**: Chrome bootstrap already handles `alert()`, `confirm()`, `prompt()` natively — no custom `OnJSDialog` needed.

Added `CefJSDialogHandler` interface to `SimpleHandler` for `OnBeforeUnloadDialog` only:
- Auto-allows navigation away (`callback->Continue(true, "")`) to suppress malicious beforeunload traps
- `GetJSDialogHandler()` returns `this`; no `OnJSDialog` override (Chrome bootstrap handles it)

### 6b: Keyboard Shortcuts — Complete

**Already working natively** (Chrome bootstrap, no custom code needed):
- Ctrl+P (print), Ctrl+±/0 (zoom), Ctrl+scroll (zoom), DevTools (F12, Ctrl+Shift+I)

**Intercepted to prevent chrome:// pages opening in separate windows**:

| Shortcut | Action | Implementation |
|----------|--------|----------------|
| Ctrl+H / Cmd+H | Open history in new tab | `CreateNewTabWithUrl("http://127.0.0.1:5137/history")` |
| Ctrl+J / Cmd+J | Show download panel | Extern `ShowDownloadPanelOverlay()` (reuses existing overlay) |
| Ctrl+D / Cmd+D | Bookmark current page | `BookmarkManager::GetInstance().AddBookmark()` via `TabManager::GetActiveTab()` |
| Alt+Left | Navigate back | `activeTab->browser->GoBack()` |
| Alt+Right | Navigate forward | `activeTab->browser->GoForward()` |

All shortcuts use `#ifdef __APPLE__` / `EVENTFLAG_COMMAND_DOWN` vs `EVENTFLAG_CONTROL_DOWN` for cross-platform. Arrow key codes use hex literals (0x25/0x27) instead of `VK_LEFT`/`VK_RIGHT` for macOS compatibility.

### Files Changed
- `simple_handler.h` — Added `CefJSDialogHandler` interface, `GetJSDialogHandler()`, `OnBeforeUnloadDialog` declaration
- `simple_handler.cpp` — `GetJSDialogHandler()`, `OnBeforeUnloadDialog` implementation, 5 new shortcuts in `OnPreKeyEvent`

### Verification
- [x] `alert("test")` → dialog appears (Chrome bootstrap)
- [x] `confirm("ok?")` → dialog with OK/Cancel (Chrome bootstrap)
- [x] `prompt("name?")` → input dialog (Chrome bootstrap)
- [x] beforeunload traps suppressed (auto-allow navigation)
- [x] Ctrl+H opens history in new tab (not chrome://history in separate window)
- [x] Ctrl+J opens download panel overlay (not chrome://downloads in separate window)
- [x] Ctrl+D bookmarks current page via BookmarkManager
- [x] Alt+Left/Right navigate back/forward
- [x] Existing shortcuts still work (F12, Ctrl+F, Ctrl+Shift+I, Ctrl+P, zoom)

---

## Sprint 7: Light Wallet Polish (2-3 days)

**Goal**: Wallet overlay looks and feels production-quality.

### 7a: Button States & Feedback (Day 1)

- All buttons: hover (lighten), pressed (darken), disabled (grey + cursor), loading (spinner)
- "Copy" button → "Copied!" toast (2s, fade out)
- Send button → loading spinner while broadcasting

### 7b: Transaction Progress (Day 1)

- Send form: show progress steps ("Signing...", "Broadcasting...", "Confirmed ✓")
- Error state: red banner with error message and retry option
- Success state: green banner with txid (truncated, clickable to explorer)

### 7c: QR Code & Validation (Day 2)

- Add `qrcode.react` dependency (or `qr-code-styling`)
- Receive section: QR code below address
- Send form: inline validation (address format, amount > 0, amount ≤ balance)
- Empty state messages ("No transactions yet", etc.)

### Files
- `frontend/src/components/WalletPanel.tsx` — Button states, copy feedback
- `frontend/src/components/WalletPanelContent.tsx` — Progress indicators
- `frontend/src/components/TransactionForm.tsx` — Validation, progress steps
- `frontend/src/components/WalletPanel.css` — Button state styles
- `frontend/package.json` — Add QR code dependency

### Verification
- [ ] All buttons have visible hover/pressed/disabled states
- [ ] Copy address → "Copied!" appears and fades
- [ ] Send transaction → progress steps visible
- [ ] QR code renders in receive section
- [ ] Invalid address → inline error message

---

## Sprint 8: Ad & Tracker Blocking (3-5 days)

**Goal**: Block ads and trackers using the `adblock` crate via a separate HTTP microservice.

### Architecture Decision: Separate Microservice (NOT FFI)

**Chosen approach**: Standalone Rust HTTP service (`adblock-engine/` at repo root), running on **port 3302**. C++ starts it alongside the wallet server and calls it via sync WinHTTP with a C++-side URL cache.

**Why microservice over FFI**: FFI requires a separate crate, cbindgen, CMake linking, system library deps (`ws2_32 userenv bcrypt ntdll`), panic-catching on every call, and `#[repr(C)]` structs. HTTP microservice uses proven patterns already in the codebase (same as wallet on port 3301).

**Why separate from `rust-wallet`**: (1) Separation of concerns — wallet handles money, adblock handles content filtering. (2) Attack surface — large dependency tree shouldn't be in the security-critical wallet process. (3) Independence — ad blocking works even if wallet is locked. (4) Startup — downloading filter lists shouldn't delay wallet. (5) Crash isolation.

**Latency trade-off**: Engine check is ~5.7us in-process, but ~1-2ms via localhost HTTP. Mitigated by aggressive C++-side URL cache — after initial checks, cache hit rate is very high.

### Key Technical Decisions

- **`adblock` crate** (NOT `adblock-rust`): `=0.10.3` pinned — v0.10.4+ requires unstable `unsigned_is_multiple_of` (needs Rust 1.87+; we're on stable 1.85.1)
- **`rmp` pinned to `=0.8.14`**: Required for rmp-serde 0.15 compat (used by adblock 0.10.x)
- **`default-features = false`**: Disables `unsync-regex-caching` feature, which is what removes `Send+Sync` in v0.10.3. Required for `RwLock<Engine>`.
- **Serialization**: v0.10.3 uses `.serialize()` (NOT `.serialize_raw()` which exists in newer versions)
- **Standard mode** (default): Only blocks third-party requests. First-party passes through. Critical for avoiding site breakage.
- **Filter lists**: EasyList + EasyPrivacy on first run. Stored in `%APPDATA%/HodosBrowser/adblock/lists/`. Serialized engine at `engine.dat` for fast reload.

### 8a: Standalone Rust Engine — COMPLETE (2026-02-23)

**Built**: `adblock-engine/` at repo root with Actix-web server on port 3302, 2 workers.

**Project structure**:
```
adblock-engine/
  Cargo.toml
  src/
    main.rs       # Actix-web server, two-phase startup
    engine.rs     # AdblockEngine: RwLock<Engine>, init, serialize, check
    handlers.rs   # HTTP endpoint handlers
```

**Cargo.toml configuration**:
```toml
adblock = { version = "=0.10.3", default-features = false, features = [
    "embedded-domain-resolver",
    "full-regex-handling",
] }
rmp = "=0.8.14"
```

**Two-phase startup**: Server starts immediately (health returns `{"status":"loading"}`), engine loads async (download lists or deserialize `engine.dat`), then health returns `{"status":"ready"}`.

**Endpoints**: `GET /health`, `POST /check` (url/sourceUrl/resourceType -> blocked/filter/redirect), `GET /status` (enabled/listCount/totalRules/lastUpdate/lists), `POST /toggle` (enabled bool).

### 8b: C++ Integration — COMPLETE (2026-02-23)

**Built**:
- `StartAdblockServer()` / `StopAdblockServer()` in `cef_browser_shell.cpp` (mirrors wallet pattern: `CreateProcessA` + Job Object for auto-kill)
- `AdblockCache` singleton (`AdblockCache.h`): sync WinHTTP to `POST localhost:3302/check`, in-memory URL cache (`hash(url+sourceDomain) -> Result`), per-browser blocked count tracking
- Hook in `GetResourceRequestHandler()` in `simple_handler.cpp`, BEFORE wallet interception
- `BlockedResourceHandler`: returns 0-byte response for blocked URLs
- `CefResourceTypeToAdblock()`: maps all 19 CEF resource types to adblock's 17 string types
- Cache invalidation: `clearForBrowser(browserId)` on main frame navigation, `clearAll()` on filter list update or toggle change
- Health poll: 6 attempts x 500ms = 3s max (shorter than wallet's 5s — non-critical)

### 8c: Per-Site Toggle + UI — COMPLETE (2026-02-23)

**Built**:
- `adblock_enabled` column in `domain_permissions` table (migration V5, default true)
- Rust wallet endpoints: `GET/POST /adblock/site-toggle?domain=X` (queries/updates domain_permissions)
- C++ `DomainPermissionCache`: adblock status included in cached data
- Frontend: `SecurityIcon` shield in header bar with blocked count badge
- Click shield -> dropdown popup: domain name, ON/OFF toggle, blocked count, "Blocking may break this site" hint
- IPC: `adblock_get_blocked_count`, `adblock_reset_blocked_count`, `adblock_site_toggle` -> renderer response handlers
- `useAdblock.ts` React hook for adblock state management

### 8d: Filter List Auto-Update — COMPLETE (2026-02-23)

**Built**: Background auto-update of filter lists in the adblock engine.

- Background tokio task in `adblock-engine/` (every 6 hours)
- Checks `meta.json` timestamps, respects `Expires` headers from list servers
- Downloads updated lists, recompiles engine under write lock, re-serializes to `engine.dat`
- Atomic swap: build new engine, swap under `RwLock` write lock, then serialize
- C++ cache invalidation: version counter in `/check` + `/status` responses — C++ `AdblockCache` detects version change on cache-miss calls and invalidates URL cache
- EasyList/EasyPrivacy expire every 4 days; 6-hour check interval ensures timely updates

### 8e: Cosmetic Filtering + Scriptlet Injection + YouTube Ad Blocking — COMPLETE (2026-02-24)

**Three-layer approach**: CSS cosmetic filtering, JavaScript scriptlet injection, and network-level response filtering work together to block ads including YouTube's deeply-integrated ad system.

#### Component A: Scriptlet Resources + uBlock Filters (Rust) — COMPLETE

- uBlock Origin `scriptlets.js` pinned to v1.48.4 tag (last version using old `///`-delimited format compatible with `assemble_scriptlet_resources()` in adblock-rust 0.10.3). Post-1.48.x uses ES module format which is incompatible.
- Added `resource-assembler` feature to `adblock` crate in `Cargo.toml`
- Added 2 trusted filter lists: `ublock-filters.txt` + `ublock-privacy.txt` with `PermissionMask::from_bits(1)` for trusted scriptlet access
- 6 extra scriptlets bundled as embedded JS templates (missing from v1.48.4 scriptlets.js, needed for YouTube):
  - `trusted-replace-fetch-response.js` — Proxy-wraps `window.fetch`, applies regex/string replacement
  - `trusted-replace-xhr-response.js` — Overrides XHR prototype, replaces response text
  - `json-prune-fetch-response.js` — Proxy-wraps `window.fetch`, prunes JSON properties
  - `json-prune-xhr-response.js` — Overrides XHR prototype, prunes JSON properties
  - `trusted-replace-node-text.js` — MutationObserver on script tags, replaces matching text
  - `remove-node-text.js` — MutationObserver that clears matching script text
- `load_extra_scriptlets()` function registers these via `engine.add_resource()` with base64-encoded content
- CONFIG_VERSION bumped to 3 (forces engine.dat rebuild)

#### Component B: `/cosmetic-resources` + `/cosmetic-hidden-ids` Endpoints (Rust) — COMPLETE

- `POST /cosmetic-resources` — returns `hideSelectors`, `injectedScript`, `generichide` for a URL
- `POST /cosmetic-hidden-ids` — Phase 2 generic cosmetic selectors matching DOM class names and IDs
- C++ `AdblockCache` calls these via sync WinHTTP with JSON parsing

#### Component C: Scriptlet Pre-Caching + V8 Injection (C++) — COMPLETE

Two-stage injection with timing optimization:

1. **Pre-cache** (`OnBeforeBrowse` + `OnLoadingStateChange`): Browser process fetches scriptlets for navigation target URL, sends `preload_cosmetic_script` IPC to renderer. `OnBeforeBrowse` provides correct URL before navigation (fixes empty-URL bug in `OnLoadingStateChange`).
2. **Inject** (`OnContextCreated` in renderer): Checks `s_scriptCache` for pre-cached scripts, executes via `frame->ExecuteJavaScript()` synchronously before any page JS runs.
3. **Fallback** (`OnLoadingStateChange(!isLoading)`): If pre-cache missed, injects scriptlets after page load (covers SPA navigations).

#### Component D: CSS Cosmetic Filtering (C++) — COMPLETE

- Phase 1: Hostname-specific selectors injected via `inject_cosmetic_css` IPC
- Phase 2: JS collects DOM classes/IDs, sends `cosmetic_class_id_query` IPC, C++ fetches generic selectors from `/cosmetic-hidden-ids`, returns additional CSS
- Dedup via `last_cosmetic_url_` member variable

#### Component E: CefResponseFilter for YouTube API/HTML (C++) — COMPLETE (2026-02-24)

**The primary YouTube ad blocking mechanism.** Operates at the network level (browser process IO thread), before any JavaScript sees the response data. Solves the scriptlet injection timing problem.

- `AdblockResponseFilter` class (CefResponseFilter): Buffers complete response, renames 5 ad-configuration JSON keys by appending `_` (`"adPlacements":` → `"adPlacements_":`, etc.). YouTube's player JS can't find the renamed keys → ads don't load.
- `CookieFilterResourceHandler::GetResourceResponseFilter()`: Returns filter for YouTube API JSON (`/youtubei/` + `application/json`) and main-frame HTML (`RT_MAIN_FRAME` + `text/html`).
- Host matching uses `://www.youtube.com/` prefix (not substring) to avoid false positives.
- Verified: 35 ad key renames across a browsing session, processing responses 35KB-1.8MB. Zero YouTube ads observed.

**Known trade-off**: Buffering adds page load latency (response can't render until fully buffered). Acceptable for YouTube; streaming replacement is a potential optimization tracked in `ux-ui-cleanup.md`.

#### Key Technical Decisions
- **Scriptlets.js URL**: Pinned to `gorhill/uBlock` tag 1.48.4 (NOT uAssets main branch which returns 404)
- **Trusted scriptlets**: `trusted-replace-xhr-response`, `trusted-replace-fetch-response` require permission bit 0 on the filter list. uBlock filters loaded with `PermissionMask::from_bits(1)`.
- **Response filter over scriptlet timing**: CefResponseFilter is more reliable than fixing IPC race conditions between `OnBeforeBrowse` and `OnContextCreated`. Both approaches are implemented for defense in depth.
- **Arms race**: YouTube changes ad delivery frequently. Auto-updating filter lists (8d) + response filter key renaming provide complementary protection.

### 8f: Unified Privacy Shield Panel — COMPLETE (2026-02-23)

**Built**: Merged adblock controls and cookie blocking controls into a single "Privacy Shield" panel behind the shield icon in the header bar.

- Unified overlay panel replaces separate adblock dropdown and cookie blocking UI
- Shows combined privacy metrics: ads blocked count + cookies blocked count
- Per-site toggle for ad blocking (existing from 8c)
- Per-site toggle for cookie blocking (existing `CookieBlockManager` integration)
- Consistent shield icon state reflects overall protection status
- Known UI polish issues deferred to `ux-ui-cleanup.md`

### Files
- `adblock-engine/` — Standalone Rust project (Cargo.toml, src/main.rs, src/engine.rs, src/handlers.rs)
- `adblock-engine/src/scriptlets/` — 6 bundled extra scriptlet JS templates (trusted-replace-fetch/xhr, json-prune-fetch/xhr, trusted-replace-node-text, remove-node-text)
- `cef-native/include/core/AdblockCache.h` — C++ singleton (URL cache, WinHTTP client, cosmetic resource fetching, `CookieFilterResourceHandler` with `GetResourceResponseFilter`)
- `cef-native/cef_browser_shell.cpp` — `StartAdblockServer()` / `StopAdblockServer()`
- `simple_handler.cpp` — `AdblockResponseFilter` (CefResponseFilter), `OnBeforeBrowse` scriptlet pre-cache, adblock check in `GetResourceRequestHandler()`, cosmetic filtering in `OnLoadingStateChange`, IPC handlers
- `simple_render_process_handler.cpp` — Scriptlet pre-cache storage (`s_scriptCache`), `OnContextCreated` early injection, `preload_cosmetic_script`/`inject_cosmetic_script`/`inject_cosmetic_css` IPC handlers
- `HttpRequestInterceptor.cpp` — Adblock check before wallet interception
- `rust-wallet/src/database/domain_permission_repo.rs` — `adblock_enabled` column
- `rust-wallet/src/database/migrations.rs` — V5 migration
- `rust-wallet/src/handlers.rs` — `/adblock/site-toggle` endpoints
- `frontend/src/hooks/useAdblock.ts` — React hook for adblock state
- `frontend/src/hooks/useCookieBlocking.ts` — React hook for cookie blocking state
- `frontend/src/hooks/usePrivacyShield.ts` — Composed hook (adblock + cookie blocking)
- `frontend/src/components/PrivacyShieldPanel.tsx` — Unified privacy panel UI
- `frontend/src/pages/PrivacyShieldOverlayRoot.tsx` — Overlay root for privacy shield panel

### macOS Notes
- **adblock-engine**: Pure Rust, builds natively on macOS (`cargo build --release`). No platform-specific code.
- **C++ AdblockCache**: Uses WinHTTP — needs `#elif defined(__APPLE__)` with libcurl/NSURLSession in macOS sprint. Same pattern as `DomainPermissionCache`, `BSVPriceCache`, `WalletStatusCache`.
- **`StartAdblockServer()`**: Uses `CreateProcessA` — needs macOS `posix_spawn` equivalent in `cef_browser_shell_mac.mm`.
- **Filter list storage**: Use `~/Library/Application Support/HodosBrowser/adblock/` on macOS (cross-platform path helper).
- **Shield icon / privacy panel**: React components, fully cross-platform.

### Verification
- [x] Visit ad-heavy site → ads blocked
- [x] Blocked count shown in shield icon badge
- [x] Disable for specific site → ads appear on that site
- [x] Re-enable → ads blocked again
- [x] Startup with cached `engine.dat` → instant blocking (no compile delay)
- [x] Privacy shield panel shows combined adblock + cookie blocking controls
- [x] Filter lists auto-update (8d) — 6-hour background check, version-based cache invalidation
- [x] YouTube ads blocked (8e) — CefResponseFilter strips ad keys from API/HTML responses; scriptlet injection provides defense in depth
- [x] Cosmetic CSS selectors hide ad-related DOM elements
- [x] Scriptlets inject before page JS via OnContextCreated pre-cache

---

## Sprint 9: Settings Persistence + Profile Import (2-3 days)

**Goal**: Browser settings persist across restarts. Users can import bookmarks and history from Chrome/Brave.

### 9a: Settings Persistence (Day 1)

**Storage**: JSON file at `%APPDATA%/HodosBrowser/settings.json` (simplest, human-readable).

**Key settings**:
```json
{
  "homepage": "about:blank",
  "searchEngine": "google",
  "adBlockEnabled": true,
  "thirdPartyCookieBlocking": true,
  "zoomLevel": 0.0,
  "downloadsPath": "",
  "showBookmarkBar": false
}
```

**C++ SettingsManager singleton**: Load on startup, save on change, provide getters.

### 9b: Profile Import (Day 2-3)

**Import wizard in Settings overlay**:
1. Auto-detect Chrome/Brave profile locations
2. Let user choose what to import (bookmarks, history, or both)
3. Import bookmarks: parse Chrome's `Bookmarks` JSON → insert into `BookmarkManager`
4. Import history: copy + read Chrome's `History` SQLite → insert into `HistoryManager`
5. Show progress and summary

**Chrome profile detection**:
- Chrome: `%LOCALAPPDATA%/Google/Chrome/User Data/Default/`
- Brave: `%LOCALAPPDATA%/BraveSoftware/Brave-Browser/User Data/Default/`
- Edge: `%LOCALAPPDATA%/Microsoft/Edge/User Data/Default/`

**Note**: Chrome locks its database files while running. The import handler should copy the file first, then read the copy.

### macOS Notes
- **Settings file path**: `~/Library/Application Support/HodosBrowser/settings.json` on macOS. Use cross-platform path resolution in `SettingsManager`.
- **Profile import detection**: Chrome/Brave profile paths differ on macOS:
  ```
  Chrome: ~/Library/Application Support/Google/Chrome/Default/
  Brave:  ~/Library/Application Support/BraveSoftware/Brave-Browser/Default/
  Safari: ~/Library/Safari/ (different format — post-MVP if at all)
  ```
  Use `#ifdef _WIN32` / `#elif __APPLE__` for profile path detection.
- **Chrome cookie import on macOS**: Chrome encrypts cookies differently on macOS (Keychain-derived key, AES-128-CBC). More complex than Windows DPAPI — defer to macOS sprint if cookie import is desired.
- **SettingsManager singleton**: File I/O is standard C++ `fstream` — cross-platform. Just need platform path.

### Files
- New `SettingsManager.h/cpp` singleton
- `cef_browser_shell.cpp` — Initialize SettingsManager
- `simple_handler.cpp` — IPC for import triggers
- New import logic in C++ (SQLite + JSON parsing)
- `SettingsOverlayRoot.tsx` — Import UI section

### Verification
- [ ] Change a setting → close browser → reopen → setting persisted
- [ ] Import bookmarks from Chrome → appear in bookmark manager
- [ ] Import history from Chrome → appear in history

---

## Sprint 10: Third-Party Cookie Blocking + Basic Fingerprinting Protection (2 days)

**Goal**: Block third-party tracking cookies and basic fingerprinting APIs.

### 10a: Third-Party Cookie Blocking (Day 1)

Implement `CefCookieAccessFilter` on `AsyncWalletResourceHandler` or create a new `CefResourceRequestHandler`:
- `CanSendCookie` / `CanSaveCookie`: check if cookie domain matches page domain
- If different → block (third-party cookie)
- Exception list for sites known to break (loaded from settings)

**Alternative**: Use `CefRequestContext` settings to block third-party cookies globally. This is simpler but less configurable.

### 10b: Basic Fingerprinting Protection (Day 2)

V8 injection in `simple_render_process_handler.cpp`:
- Detect third-party iframes (check `window.top !== window.self` + cross-origin)
- In third-party contexts: override `HTMLCanvasElement.toDataURL`, `HTMLCanvasElement.toBlob`, `CanvasRenderingContext2D.getImageData`, `WebGLRenderingContext.readPixels`, `AudioContext.createAnalyser`
- Return empty/zeroed data instead of actual fingerprint data

This blocks the most common fingerprinting vectors without affecting first-party site functionality.

### macOS Notes
- `CefCookieAccessFilter` is cross-platform CEF API. No platform code needed.
- V8 injection for fingerprinting protection: `simple_render_process_handler.cpp` is cross-platform (renderer process). No platform code needed.
- `CefRequestContext` cookie settings are cross-platform.

### Files
- `HttpRequestInterceptor.cpp` or new cookie filter
- `simple_render_process_handler.cpp` — V8 fingerprint blocking

### Verification
- [ ] Third-party cookies blocked (check with browser cookie panel)
- [ ] First-party cookies still work
- [ ] Visit fingerprinting test site (e.g., coveryourtracks.eff.org) → reduced fingerprint surface

---

## Sprint Schedule Summary

### Recommended Order (Sequential with Parallelization)

```
Week 1:
  Sprint 0: Safety + Quick Wins .............. 1 day   [PARALLEL START]
  Sprint 2: Permission Handler ............... 0.5 day  [PARALLEL START]
  Sprint 1: SSL + Secure Indicator ........... 1-2 days [PARALLEL START]
  Sprint 3: Download Handler ................. 2-3 days [PARALLEL START]

Week 2:
  Sprint 4: Find-in-Page .................... 1-2 days [after Sprint 0] ✅ COMPLETE
  Sprint 5: Context Menus ................... 1 day    [after Sprint 0]
  Sprint 6: JS Dialogs + Shortcuts .......... 0.5 day  [after Sprint 3,4]
  Sprint 7: Light Wallet Polish .............. 2-3 days [PARALLEL]

Week 3:
  Sprint 8: Ad & Tracker Blocking ........... 3-5 days [PARALLEL]

Week 4 (if time):
  Sprint 9: Settings + Import ............... 2-3 days [after Sprint 8]
  Sprint 10: Cookie + Fingerprinting ........ 2 days   [after Sprint 8]
```

### What Can Run in Parallel

- Sprint 0 + Sprint 1 + Sprint 2 + Sprint 3 (all independent)
- Sprint 4 + Sprint 5 + Sprint 7 (all independent, after Sprint 0)
- Sprint 8 is standalone (largest sprint, can start anytime)
- Sprint 9 + Sprint 10 are independent of each other

---

## Architecture Decisions Log

| Decision | Choice | Rationale |
|----------|--------|-----------|
| SSL interstitial | Inline data URL (not React route) | Simpler, no IPC needed, sufficient for MVP |
| Permission handler | Return `false` for Chrome native UI | Zero effort, Chrome handles everything |
| Download handler MVP | Save As dialog only (no progress overlay) | Ship faster, add overlay later |
| Find bar | React component in MainBrowserView | Shares existing IPC, simpler than HWND |
| Ad blocking architecture | HTTP microservice on port 3302 (not FFI) | Avoids cbindgen/CMake linking complexity, reuses proven HTTP patterns, crash isolation from wallet, adblock crate =0.10.3 pinned for stable Rust compat |
| Settings persistence | JSON file (not SQLite) | Human-readable, simple, no migration needed |
| Profile import | C++ reads Chrome files directly | No CEF API for import, direct read is straightforward |

---

## Documentation Fixes — COMPLETED (Pre-Sprint)

All documentation fixes completed 2026-02-19, before implementation sprints began:

1. ~~**Consolidate** PROJECT_OVERVIEW.md + ARCHITECTURE.md + WALLET_ARCHITECTURE.md~~ -> Done. Single PROJECT_OVERVIEW.md with archived pointers in the others.
2. ~~**Rewrite** README.md~~ -> Done. Concise landing page with current status and links.
3. ~~**Update** CLAUDE.md key files section~~ -> Done. All 8 discrepancies fixed, 2 new invariants added.
4. ~~**Update** CEF_REFINEMENT_TRACKER.md~~ -> Done. All CR-2/CR-3 items checked off.
5. ~~**Archive** FEATURES.md~~ -> Done. Pointer to browser-capabilities.md.
6. ~~**Minor edit** THE_WHY.md~~ -> Done. Date fixed, API names updated, concrete examples added.
7. **Update** UX_UI/00-IMPLEMENTATION_INDEX.md -> Still TODO (mark CR-2 complete, Phase 2 complete).
8. ~~**Move** macOS root docs~~ -> Done. 3 docs moved to development-docs/macos-port/.
9. ~~**Update** SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md~~ -> Done. Go->Rust, added notification process.
10. ~~**Create** browser-core/CLAUDE.md~~ -> Done. Sprint-specific context with cross-platform rules.
11. ~~**Archive** TECH_STACK_INTEGRATION.md, UX_FEATURE_COMPARISON_AND_ROADMAP.md~~ -> Done. Archive headers added.

---

## Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| `adblock` crate incompatible with future Rust versions | Sprint 8 maintenance | Pinned to =0.10.3; monitor for stable Rust gaining `unsigned_is_multiple_of`; can upgrade when Rust 1.87+ is stable |
| Chrome bootstrap doesn't show permission UI | Sprint 2 broken | Implement custom permission overlay (more work) |
| x.com login still broken after SSL handler | User frustration | Investigate FedCM / redirect OAuth fallback |
| YouTube ad blocking arms race | 8e effectiveness degrades | Auto-updating filter lists (8d) essential; uBlock Origin community maintains rules |
| Profile import reads corrupt Chrome DB | Data loss | Copy DB first, validate before importing |

---

## Post-Implementation Testing Checklist

### Tier 0 Verification
- [ ] BSV price cache: disconnect internet → payment request → requires user approval (not auto-approved)
- [ ] SSL: visit site with bad cert → interstitial → "Proceed" works
- [ ] Downloads: click download link → Save As → file appears
- [ ] Camera: visit Google Meet → permission prompt → camera works after allow

### Tier 1 Verification
- [ ] Ctrl+F → find bar → type → matches highlighted
- [ ] Right-click → full context menu with Copy/Paste/etc.
- [ ] Padlock shows green for HTTPS, warning for HTTP
- [ ] Wallet: send transaction → progress indicator → success/error feedback
- [ ] Wallet: receive tab shows QR code
- [ ] All keyboard shortcuts work

### Tier 2 Verification
- [ ] Visit ad-heavy site → ads blocked → count displayed
- [ ] Toggle ad blocking off for site → ads appear
- [ ] Settings persist across restart
- [ ] Import Chrome bookmarks → appear in Hodos
- [ ] Third-party cookies blocked by default

---

**End of Document**
