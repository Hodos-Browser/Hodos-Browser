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

## Sprint 3: Download Handler (2-3 days)

**Goal**: Users can download files with progress tracking, cancel/pause/resume.

### 3a: CEF Download Handler (Day 1)

**Add `CefDownloadHandler` to `SimpleHandler`**:
- `CanDownload` → return `true` (allow all downloads)
- `OnBeforeDownload` → call `callback->Continue("", true)` for system Save As dialog
- `OnDownloadUpdated` → track progress, forward state to React via IPC

**Download state tracking**: C++ maintains `std::map<int32, DownloadState>` keyed by download ID.

```cpp
struct DownloadState {
    int32 id;
    std::string url;
    std::string fullPath;
    int64 receivedBytes;
    int64 totalBytes;
    int percentComplete;
    bool isComplete;
    bool isCanceled;
    CefRefPtr<CefDownloadItemCallback> callback; // for cancel/pause/resume
};
```

### 3b: Downloads Panel UI (Day 2-3)

**New React overlay**: `DownloadsPanelOverlayRoot.tsx`
- List of active and recent downloads
- Each item shows: filename, progress bar, speed, size, cancel/pause/resume buttons
- Completed items show: filename, size, "Open" and "Show in Folder" buttons
- Ctrl+J keyboard shortcut to toggle

**IPC messages**:
- `download_get_all` → returns current download list
- `download_cancel(id)` → cancel a download
- `download_pause(id)` → pause
- `download_resume(id)` → resume
- `download_open(id)` → open file
- `download_show_folder(id)` → open containing folder

**Alternative (simpler MVP)**: Skip the overlay entirely. Just use `OnBeforeDownload` with Save As dialog + no progress UI. Users see the system save dialog and the file appears in their Downloads folder. This is what basic browsers do.

**Recommendation**: Implement the simpler MVP first (just Save As). Add the overlay panel as a Tier 1 enhancement.

### macOS Notes
- `CefDownloadHandler` is a CEF API — fully cross-platform. Save As dialog is native on both platforms.
- If building a downloads overlay HWND: needs `#ifdef _WIN32` for `CreateWindowExW` and `#elif __APPLE__` stub (or macOS NSWindow in `cef_browser_shell_mac.mm`). **Recommendation**: Skip overlay for MVP (Save As only) — avoids platform-specific code entirely.
- `Ctrl+J` shortcut: define `Cmd+J` variant for macOS in the shortcut handler.

### Files
- `simple_handler.h` — Add `CefDownloadHandler`
- `simple_handler.cpp` — Implement 3 methods
- (Optional) New download overlay React component + IPC

### Verification
- [ ] Click a download link → Save As dialog appears
- [ ] File downloads to selected location
- [ ] Large file download → progress visible (if overlay implemented)
- [ ] Ctrl+J → downloads panel opens (if overlay implemented)

---

## Sprint 4: Find-in-Page (1-2 days)

**Goal**: Ctrl+F opens a find bar with match count, prev/next navigation.

### Implementation

**Find bar approach**: Lightweight HWND or React component rendered at the top of the browser content area.

**Recommended**: React component in `MainBrowserView.tsx` — positioned absolutely above the webview. Simpler than a new HWND, shares existing IPC infrastructure.

**State flow**:
1. Ctrl+F → `MainBrowserView` shows find bar (text input + "X of Y" + prev/next/close)
2. User types → debounced IPC `find_text(query, forward, matchCase)` → C++ calls `browser->GetHost()->Find()`
3. C++ `OnFindResult` callback → IPC `find_result(count, activeMatch)` → React updates "X of Y"
4. Prev/Next buttons → IPC `find_text` with `findNext=true`
5. Close/Escape → IPC `find_stop` → C++ calls `browser->GetHost()->StopFinding(true)`

### macOS Notes
- `CefFindHandler` and `CefBrowserHost::Find()` are cross-platform CEF APIs.
- Find bar is a React component — fully cross-platform, no platform code needed.
- `Ctrl+F` shortcut: define `Cmd+F` variant for macOS.

### Files
- `simple_handler.h` — Add `CefFindHandler`
- `simple_handler.cpp` — Implement `OnFindResult`, add IPC handlers for find_text/find_stop
- `frontend/src/components/MainBrowserView.tsx` — Add find bar component
- `frontend/src/components/FindBar.tsx` — New component

### Verification
- [ ] Ctrl+F → find bar appears
- [ ] Type text → matches highlighted, count shown
- [ ] Enter/Next → cycles through matches
- [ ] Shift+Enter/Prev → cycles backward
- [ ] Escape → find bar closes, highlights cleared

---

## Sprint 5: Context Menu Enhancement (1 day)

**Goal**: Right-click menu has all standard browser actions.

### Implementation

Extend existing `OnBeforeContextMenu` and `OnContextMenuCommand` in `simple_handler.cpp`.

**Menu items by context**:

| Context | Items |
|---------|-------|
| Page (no selection) | Back, Forward, Reload, Separator, Select All, View Page Source, Inspect |
| Text selected | Copy, Cut (if editable), Separator, Select All, Inspect |
| Link | Open Link in New Tab, Copy Link Address, Separator, Inspect |
| Image | Save Image As, Copy Image, Open Image in New Tab, Separator, Inspect |
| Editable field | Cut, Copy, Paste, Separator, Select All, Inspect |

**Custom command IDs**: Extend existing range (50100+). Use `CefBrowserHost::GetNavigationEntryCount()` to enable/disable Back/Forward.

**Copy/Cut/Paste**: Call `frame->ExecuteCommand("Copy")`, `frame->ExecuteCommand("Cut")`, `frame->ExecuteCommand("Paste")`.

### macOS Notes
- Context menu APIs (`OnBeforeContextMenu`, `OnContextMenuCommand`) are cross-platform CEF.
- `frame->ExecuteCommand("Copy")` etc. are cross-platform.
- No platform-specific code needed.

### Files
- `simple_handler.cpp` — Extend `OnBeforeContextMenu` + `OnContextMenuCommand`

### Verification
- [ ] Right-click on text → Copy works
- [ ] Right-click on link → "Open in New Tab" and "Copy Link Address" work
- [ ] Right-click on image → "Save Image As" triggers download
- [ ] Right-click in text field → Cut/Copy/Paste work
- [ ] "View Page Source" opens `view-source:` URL in new tab

---

## Sprint 6: JS Dialog Handler + Keyboard Shortcuts (0.5 day)

**Goal**: JavaScript alert/confirm/prompt work properly. Additional keyboard shortcuts.

### 6a: JS Dialog Handler

**Test first**: Chrome bootstrap may already handle `alert()`, `confirm()`, `prompt()` natively. If so, no implementation needed.

If needed: Add `CefJsDialogHandler` to `SimpleHandler`, return `false` from `OnJSDialog` for default handling.

Add `OnBeforeUnloadDialog` to suppress malicious "are you sure you want to leave?" traps — return `true` with `callback->Continue(true, "")` to always allow navigation.

### 6b: Keyboard Shortcuts

Add to existing `OnPreKeyEvent` / `OnKeyEvent` in `simple_handler.cpp`:

| Shortcut | Action | IPC/Call |
|----------|--------|----------|
| Ctrl+F | Show find bar | IPC `find_show` |
| Ctrl+J | Show downloads | IPC `downloads_show` |
| Ctrl+H | Open history tab | IPC `tab_create` with `/history` URL |
| Ctrl+D | Bookmark current page | IPC `bookmark_add` with current URL/title |
| Ctrl+P | Print | `browser->GetHost()->Print()` |
| Ctrl++ | Zoom in | `browser->GetHost()->SetZoomLevel(current + 0.5)` |
| Ctrl+- | Zoom out | `browser->GetHost()->SetZoomLevel(current - 0.5)` |
| Ctrl+0 | Reset zoom | `browser->GetHost()->SetZoomLevel(0.0)` |
| Alt+Left | Back | `browser->GoBack()` |
| Alt+Right | Forward | `browser->GoForward()` |

### macOS Notes
- `CefJsDialogHandler` is cross-platform CEF.
- **Keyboard shortcuts are the main platform concern here**: Every `Ctrl+X` shortcut must also handle `Cmd+X` on macOS. Use platform detection in the key event handler:
  ```cpp
  #ifdef __APPLE__
  bool isModifier = event.modifiers & EVENTFLAG_COMMAND_DOWN;
  #else
  bool isModifier = event.modifiers & EVENTFLAG_CONTROL_DOWN;
  #endif
  ```
- `browser->GetHost()->Print()` and zoom methods are cross-platform.

### Files
- `simple_handler.h/cpp` — Add `CefJsDialogHandler` (if needed), extend key handling

### Verification
- [ ] `alert("test")` → dialog appears
- [ ] `confirm("ok?")` → dialog with OK/Cancel
- [ ] Each new keyboard shortcut works

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

**Goal**: Block ads and trackers using `adblock-rust` via FFI.

### Architecture Decision: FFI Static Library

Build `adblock-rust` as a static library with C FFI, linked into the C++ CEF process. This matches Brave's battle-tested architecture and provides ~5μs per request check.

### 8a: Rust FFI Library (Day 1-2)

**New crate**: `adblock-ffi/` in project root (NOT in the Rust wallet workspace).

```
adblock-ffi/
├── Cargo.toml          # depends on `adblock` crate
├── src/
│   └── lib.rs          # C FFI exports
├── cbindgen.toml       # generates C header
└── build.rs            # cargo build script
```

**FFI functions**:
```rust
#[no_mangle] pub extern "C" fn adblock_engine_create() -> *mut Engine;
#[no_mangle] pub extern "C" fn adblock_engine_destroy(engine: *mut Engine);
#[no_mangle] pub extern "C" fn adblock_engine_add_filter_list(engine: *mut Engine, data: *const c_char);
#[no_mangle] pub extern "C" fn adblock_engine_check(engine: *mut Engine, url: *const c_char, source: *const c_char, request_type: *const c_char) -> bool;
#[no_mangle] pub extern "C" fn adblock_engine_serialize(engine: *mut Engine, out_data: *mut *mut u8, out_len: *mut usize);
#[no_mangle] pub extern "C" fn adblock_engine_deserialize(data: *const u8, len: usize) -> *mut Engine;
```

### 8b: C++ Integration (Day 2-3)

**New singleton**: `AdBlockEngine` in `cef-native/include/core/AdBlockEngine.h`
- Wraps FFI calls
- Loads serialized engine from `%APPDATA%/HodosBrowser/adblock/engine.dat`
- Falls back to compiling from text lists if no cache
- Thread-safe (engine itself is read-only after creation; reload creates new + atomic swap)

**Hook point**: `GetResourceRequestHandler` or `OnBeforeResourceLoad` in `HttpRequestInterceptor.cpp`
- Before wallet interception check, call `AdBlockEngine::shouldBlock(url, sourceUrl, resourceType)`
- If blocked: set `disable_default_handling = true` and return appropriate handler (or nullptr)

### 8c: Filter List Management (Day 3)

- Download EasyList + EasyPrivacy on first run (store in `%APPDATA%/HodosBrowser/adblock/lists/`)
- Compile to engine, serialize to `engine.dat`
- Background update every 24 hours (check ETag / Last-Modified)
- Bundle pre-compiled engine with installer for instant blocking on fresh install

### 8d: Per-Site Toggle UI (Day 4-5)

- Shield icon in header bar (similar to Brave)
- Click → popup showing "Ad blocking ON/OFF for this site"
- Persist per-site exceptions (could use `domain_permissions` table or separate storage)
- Show blocked count badge on shield icon

### Files
- New `adblock-ffi/` crate directory
- `cef-native/include/core/AdBlockEngine.h` — Singleton wrapper
- `cef-native/src/core/AdBlockEngine.cpp` — Implementation
- `HttpRequestInterceptor.cpp` — Add block check before forwarding
- `CMakeLists.txt` — Link adblock FFI static lib
- Frontend shield icon component

### macOS Notes
- **adblock-rust builds natively on macOS** — Brave uses it on macOS. `cargo build --release` produces `.a` (static lib) on macOS instead of `.lib`.
- **CMakeLists.txt**: Needs platform conditional for the library path and system libs:
  ```cmake
  if(WIN32)
      target_link_libraries(${PROJECT_NAME} PRIVATE adblock_ffi.lib ws2_32 userenv bcrypt ntdll)
  elseif(APPLE)
      target_link_libraries(${PROJECT_NAME} PRIVATE libadblock_ffi.a "-framework Security" "-framework CoreFoundation")
  endif()
  ```
- **C++ AdBlockEngine singleton**: All FFI function calls (`adblock_engine_check`, etc.) are C ABI — fully cross-platform. No platform conditionals needed inside the singleton.
- **Filter list storage**: Use `~/Library/Application Support/HodosBrowser/adblock/` on macOS (resolved via the cross-platform path helper).
- **Shield icon / per-site toggle**: React component, fully cross-platform.

### Build Integration

CMakeLists.txt addition:
```cmake
# Link adblock-rust static library
target_link_libraries(${PROJECT_NAME} PRIVATE
    ${PROJECT_SOURCE_DIR}/../adblock-ffi/target/release/adblock_ffi.lib
    # Windows system libs needed by Rust
    ws2_32 userenv bcrypt ntdll
)
```

### Verification
- [ ] Visit ad-heavy site → ads blocked
- [ ] Blocked count shown in UI
- [ ] Disable for specific site → ads appear
- [ ] Re-enable → ads blocked again
- [ ] Startup with cached engine → instant blocking (no compile delay)

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
  Sprint 4: Find-in-Page .................... 1-2 days [after Sprint 0]
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
| Ad blocking architecture | FFI static lib (not HTTP to Rust) | 1000x faster, Brave's proven pattern |
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
| `adblock-rust` build fails on Windows | Sprint 8 blocked | Test build early; fallback to HTTP approach if FFI fails |
| Chrome bootstrap doesn't show permission UI | Sprint 2 broken | Implement custom permission overlay (more work) |
| x.com login still broken after SSL handler | User frustration | Investigate FedCM / redirect OAuth fallback |
| adblock FFI linking conflicts with CEF | Sprint 8 blocked | Isolate in DLL instead of static lib |
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
