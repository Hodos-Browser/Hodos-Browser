# macOS Port — Developer Handover

**Last updated**: 2026-03-09
**Status**: Foundation complete, feature parity sprint ready
**Goal**: Full feature parity with Windows build

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current State — What's Built](#2-current-state)
3. [What Needs to Be Done](#3-what-needs-to-be-done)
4. [Architecture: macOS vs Windows](#4-architecture-macos-vs-windows)
5. [macOS File System Conventions](#5-macos-file-system-conventions)
6. [Sprint Plan](#6-sprint-plan)
7. [Technical Patterns & Gotchas](#7-technical-patterns-and-gotchas)
8. [Multi-Window Architecture (Sprint 13)](#8-multi-window-architecture)
9. [App Bundle, Code Signing & Distribution](#9-app-bundle-code-signing-and-distribution)
10. [Reference Files](#10-reference-files)

---

## 1. Executive Summary

HodosBrowser is a three-layer application:

```
React Frontend (localhost:5137)  ← Cross-platform, no changes needed
       ↓
C++ CEF Shell (CEF 136)          ← THIS IS WHERE THE macOS WORK LIVES
       ↓
Rust Wallet (localhost:31301)    ← Cross-platform, mac-ready (see Section 3.1)
Rust Adblock (localhost:31302)   ← Cross-platform, mac-ready
```

**The React frontend and Rust backends are already cross-platform.** The macOS port is entirely a C++ task. The December 2025 port got the foundation working — main window, header, webview, 5 overlays, event forwarding, CALayer rendering. But Sprints 8-13 (Jan-Mar 2026) added 15+ major features to the Windows C++ layer that don't exist on macOS yet.

**Estimated effort for full feature parity**: 2-3 weeks for a developer familiar with Cocoa/CEF.

---

## 2. Current State

### What's Working on macOS

| Component | Status | Details |
|-----------|--------|---------|
| **CMakeLists.txt** | Done | Platform detection, ARM64/x64 auto-detect, Homebrew linking |
| **Main window** | Done | NSWindow with header NSView (99px) + webview NSView |
| **5 overlay types** | Done | Settings, Wallet, Backup, BRC-100 Auth, Settings Menu |
| **OSR rendering** | Done | CALayer with transparency, retina support |
| **Event forwarding** | Done | Mouse + keyboard to CEF browsers in all overlays |
| **Tab basics** | Partial | `TabManager_mac.mm` exists, basic lifecycle only |
| **Wallet HTTP client** | Partial | `WalletService_mac.cpp` uses libcurl, basic connection |
| **Profile system** | Done | ProfileManager + ProfileLock initialize before CefInitialize |
| **History** | Partial | HistoryManager initialized but path may be wrong |
| **Handler wrapping** | Done | `simple_handler.cpp`, `simple_app.cpp`, `simple_render_process_handler.cpp` all have `#ifdef _WIN32` / `#elif defined(__APPLE__)` |
| **React frontend** | Done | `npm run dev` — no platform-specific code |
| **Rust wallet** | Done | `cargo run --release` — mac-ready after 2 small fixes (done, see 3.1) |
| **Rust adblock engine** | Done | `cargo run --release` — production code is cross-platform |
| **CEF helper bundles** | Done | 5 `.app` helpers (GPU, Renderer, Plugin, Alerts, base) |
| **Info.plist** | Partial | Missing usage descriptions (camera/mic/location) |

### What's NOT Working on macOS (6 missing overlay types)

| Overlay | Windows HWND | macOS Status |
|---------|-------------|-------------|
| Cookie Panel (Privacy Shield) | `g_cookie_panel_overlay_hwnd` | Missing |
| Download Panel | `g_download_panel_overlay_hwnd` | Missing |
| Omnibox | `g_omnibox_overlay_hwnd` | Missing |
| Profile Panel | `g_profile_panel_overlay_hwnd` | Missing |
| Menu Overlay | `g_menu_overlay_hwnd` | Stubbed (logs "not implemented") |
| Notification | `g_notification_overlay_hwnd` | Missing |

### What's NOT Working on macOS (singletons & features)

| Feature | Why It's Missing |
|---------|-----------------|
| **AdblockCache** | Not initialized — requires SyncHttpClient (libcurl) |
| **FingerprintProtection** | Not initialized — singleton header not included |
| **CookieBlockManager** | Not initialized — needs profile path on macOS |
| **BookmarkManager** | Not initialized — needs profile path on macOS |
| **EphemeralCookieManager** | Partially present in TabManager_mac.mm only |
| **HTTP request interception** | 5 WinHTTP singletons not ported (see Section 6, Day 2) |
| **Process auto-launch** | Wallet & adblock servers must be started manually |
| **Graceful shutdown** | No HTTP `POST /shutdown` → `kill` fallback |
| **Multi-window** | BrowserWindow/WindowManager not used (single-window only) |
| **Session save/restore** | `SaveSession()` is a TODO stub |
| **Keyboard shortcuts** | Ctrl-based, not Cmd-based |
| **Overlay close-prevention** | No `g_wallet_overlay_prevent_close` or `g_file_dialog_active` guards |

---

## 3. What Needs to Be Done

### 3.1 Rust Wallet — Mac-Ready (2 fixes, already coded)

The Rust wallet compiles and runs on macOS. Two changes were needed:

**Fix 1: Data directory path** (`src/main.rs`)
- Old: Used `APPDATA` env var (Windows-only), fell back to current directory
- New: Uses `dirs::data_dir()` which resolves to `~/Library/Application Support/` on macOS
- The `dirs` crate was already in Cargo.toml but unused

**Fix 2: DPAPI → macOS Keychain** (`src/crypto/dpapi.rs`)
- Old: `#[cfg(not(windows))]` stub always returned `Err()`
- New: `#[cfg(target_os = "macos")]` implementation using `security-framework` crate
- Stores mnemonic in macOS Keychain via `SecKeychainAddGenericPassword`
- DB column `mnemonic_dpapi` stores a sentinel value (`b"KEYCHAIN"`)
- Retrieves via `SecKeychainFindGenericPassword` for auto-unlock

**Result**: After these fixes, the Rust wallet works identically on macOS — correct DB path, Keychain auto-unlock, no PIN required on restart. **New devs do not need to touch the wallet.**

### 3.2 Adblock Engine — Mac-Ready (trivial test fixes)

Production code already has `#[cfg(target_os = "macos")]` path handling in `resolve_adblock_dir()`. Two test functions have hardcoded `APPDATA` paths that need `#[cfg]` conditionals — trivial, non-blocking.

### 3.3 C++ CEF Shell — Where ALL the Work Lives

This is organized into the sprint plan in Section 6. Summary:

| Category | Tasks | Effort |
|----------|-------|--------|
| **SyncHttpClient abstraction** | Replace 5 WinHTTP singletons with libcurl on macOS | 2-3 days |
| **6 missing overlays** | Cookie, Download, Omnibox, Profile, Menu, Notification | 2-3 days |
| **Singleton initialization** | AdblockCache, FingerprintProtection, CookieBlockManager, BookmarkManager | 1 day |
| **Process lifecycle** | Auto-launch wallet/adblock, graceful shutdown | 1 day |
| **Keyboard shortcuts** | Cmd instead of Ctrl (12+ shortcuts) | 0.5 day |
| **Multi-window** | BrowserWindow macOS members, WindowManager, tab tear-off | 3-5 days |
| **App signing & distribution** | Entitlements, code signing, notarization | 1-2 days |
| **Integration testing & bugs** | Full test basket on macOS | 2-3 days |

---

## 4. Architecture: macOS vs Windows

### Window/View Hierarchy

**Windows:**
```
g_hwnd (main window, WS_OVERLAPPEDWINDOW)
  ├── g_header_hwnd (child, WS_CHILD, 99px)  →  CEF browser (windowed)
  ├── tab HWNDs (child, WS_CHILD)            →  CEF browsers (windowed)
  └── g_webview_hwnd (child, WS_CHILD)        →  CEF browser (windowed)

Overlay HWNDs (WS_POPUP, separate top-level windows):
  ├── Settings, Cookie, Download, Profile, Menu, Omnibox  →  Pre-created, show/hide
  ├── Wallet, Backup, BRC-100 Auth                        →  Destroy-on-close, recreated
  └── Notification                                         →  Keep-alive, show/hide
```

**macOS (current):**
```
g_main_window (NSWindow, titled+closable+resizable)
  contentView:
    ├── g_header_view (NSView, 99px at top)   →  CEF browser (SetAsChild, windowed)
    └── g_webview_view (NSView, fills rest)    →  CEF browser (SetAsChild, windowed)

Overlay NSWindows (borderless, separate windows):
  ├── g_settings_overlay_window    →  OSR via CALayer, child of main
  ├── g_wallet_overlay_window      →  OSR via CALayer, floating (NOT child — for keyboard input)
  ├── g_backup_overlay_window      →  OSR via CALayer, child of main
  ├── g_brc100_auth_overlay_window →  OSR via CALayer, child of main
  └── g_settings_menu_overlay_window → OSR via CALayer, NSPopUpMenuWindowLevel
```

### Key Architecture Difference: Wallet Overlay

The wallet overlay is **intentionally NOT a child window** on macOS. Child windows (via `addChildWindow:`) cannot become the key window and therefore cannot receive keyboard input. The wallet overlay needs text input for PIN entry and mnemonic recovery.

**Pattern**: Wallet overlay is a floating `NSWindow` at `NSFloatingWindowLevel`. Position is synchronized manually in `MainWindowDelegate::windowDidMove/windowDidResize`.

### Rendering Modes

| Component | Windows | macOS |
|-----------|---------|-------|
| Header browser | Windowed (SetAsChild) | Windowed (SetAsChild) |
| Tab browsers | Windowed (SetAsChild) | Windowed (SetAsChild) |
| Overlay browsers | OSR (WM_PAINT + BitBlt) | OSR (CALayer + CGImage) |

### Click-Outside Detection

| Mechanism | Windows | macOS Equivalent |
|-----------|---------|-----------------|
| Full-page overlays (wallet, settings) | React `handleBackgroundClick()` | Same (React, cross-platform) |
| Dropdown overlays (cookie, download, menu) | `WH_MOUSE_LL` global hook | `[NSEvent addLocalMonitorForEventsMatchingMask:]` |
| App focus loss | `WM_ACTIVATEAPP` in main WndProc | `windowDidResignKey:` or `NSWorkspaceDidDeactivateApplicationNotification` |
| Overlay focus loss | `WM_ACTIVATE(WA_INACTIVE)` in overlay WndProc | `windowDidResignKey:` on overlay NSWindow delegate |

---

## 5. macOS File System Conventions

### Storage Paths

```
~/Library/Application Support/HodosBrowser/           # Root (= %APPDATA%/HodosBrowser/)
├── wallet/
│   └── wallet.db                                      # Rust wallet database
├── Default/                                           # CEF profile
│   ├── HodosHistory                                   # History DB
│   ├── bookmarks.db                                   # Bookmarks DB
│   ├── cookie_blocks.db                               # Cookie blocking rules
│   ├── Cookies                                        # CEF cookie jar
│   └── ...                                            # CEF auto-created files
├── adblock/                                           # Adblock engine
│   ├── lists/                                         # Downloaded filter lists
│   └── engine.dat                                     # Compiled engine binary
└── settings.json                                      # Browser settings

~/Library/Caches/HodosBrowser/                         # HTTP cache (not backed up by Time Machine)
~/Library/Logs/HodosBrowser/                           # Application logs
```

### Path Resolution Pattern

**Rust** (already fixed): Uses `dirs::data_dir()` → `~/Library/Application Support/`

**C++** (needs implementation in each singleton/manager):
```cpp
#ifdef _WIN32
    std::string appdata = std::getenv("APPDATA");
    std::string dataDir = appdata + "\\HodosBrowser";
#elif defined(__APPLE__)
    NSArray* paths = NSSearchPathForDirectoriesInDomains(
        NSApplicationSupportDirectory, NSUserDomainMask, YES);
    NSString* appSupport = [paths firstObject];
    std::string dataDir = [appSupport UTF8String] + std::string("/HodosBrowser");
#endif
```

---

## 6. Sprint Plan

### Prerequisites

- macOS machine with Xcode command line tools, Homebrew
- CEF binaries for macOS (from https://cef-builds.spotifycdn.com/index.html)
- Apple Developer Program membership ($99/year) for code signing
- Read `CLAUDE.md` (root) and `cef-native/CLAUDE.md` first

### Phase 1: SyncHttpClient + Singleton Porting (Days 1-2)

**Goal**: Get HTTP communication working between C++ and Rust backends on macOS.

The core problem: 5 C++ singletons use synchronous WinHTTP to call `localhost:31301` (wallet) and `localhost:31302` (adblock). On macOS, WinHTTP doesn't exist.

**Task 1.1: Create `SyncHttpClient.h/cpp`** (Day 1)

Create a shared HTTP helper that wraps WinHTTP on Windows and libcurl on macOS:

```cpp
// include/core/SyncHttpClient.h
class SyncHttpClient {
public:
    static std::string Get(const std::string& url, int timeoutMs = 5000);
    static std::string Post(const std::string& url, const std::string& body,
                           const std::string& contentType = "application/json",
                           int timeoutMs = 5000);
};
```

- Windows impl: Wrap existing WinHTTP pattern from `HttpRequestInterceptor.cpp`
- macOS impl: Use libcurl (already linked via `WalletService_mac.cpp`)
- libcurl is already proven working in the codebase

**Task 1.2: Port 5 singletons** (Day 2)

Replace inline WinHTTP calls with `SyncHttpClient::Get()` / `SyncHttpClient::Post()`:

| Singleton | File | What It Does |
|-----------|------|-------------|
| `DomainPermissionCache` | `HttpRequestInterceptor.cpp` | Checks domain trust level |
| `BSVPriceCache` | `HttpRequestInterceptor.cpp` | BSV/USD price for auto-approve |
| `WalletStatusCache` | `HttpRequestInterceptor.cpp` | Checks if wallet exists/locked |
| `fetchCertFieldsFromBackend()` | `HttpRequestInterceptor.cpp` | Cert field permissions |
| Health checks | `cef_browser_shell.cpp` | `QuickHealthCheck()`, `QuickAdblockHealthCheck()` |

Also ensure `X-Requesting-Domain` header is injected on macOS path in `startAsyncHTTPRequest`.

### Phase 2: Missing Overlays + Singleton Init (Days 3-4)

**Task 2.1: Create 6 missing overlay types** (Day 3)

Follow the existing pattern in `cef_browser_shell_mac.mm` (e.g., `CreateSettingsOverlayWithSeparateProcess`). Each overlay needs:

1. `CreateXxxOverlay()` function — creates NSWindow + OSR CEF browser
2. `ShowXxxOverlay(int anchorRightOffset)` — positions and shows
3. `HideXxxOverlay()` — hides (don't destroy for pre-created overlays)
4. NSView subclass with mouse/keyboard event forwarding
5. Global `NSWindow*` variable

| Overlay | Size | Positioning | Lifecycle |
|---------|------|-------------|-----------|
| Cookie Panel | 400x500 | Anchored to shield icon | Pre-create, show/hide |
| Download Panel | 400x500 | Anchored to download icon | Pre-create, show/hide |
| Omnibox | Full width, 400px tall | Below address bar | Pre-create, show/hide |
| Profile Panel | 380x500 | Anchored to profile icon | Pre-create, show/hide |
| Menu | 300x400 | Anchored to three-dot icon | Pre-create, show/hide |
| Notification | 400x200 | Top-right corner | Keep-alive, show/hide |

**For dropdown-style overlays** (Cookie, Download, Profile, Menu), add `NSEvent` monitors for click-outside detection:
```objc
id monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
    handler:^NSEvent*(NSEvent* event) {
        NSPoint loc = [event locationInWindow];
        // If click is outside overlay, hide it
        if (![overlayWindow isEqual:[event window]]) {
            HideXxxOverlay();
        }
        return event;
    }];
```

**Task 2.2: Initialize singletons in `main()`** (Day 4)

Add missing `#include` and initialization calls in `cef_browser_shell_mac.mm`:

```objc
#include "core/AdblockCache.h"
#include "core/FingerprintProtection.h"
#include "core/CookieBlockManager.h"
#include "core/BookmarkManager.h"
#include "core/WindowManager.h"

// In main(), after profile initialization:
AdblockCache::getInstance();  // Triggers initial engine load
FingerprintProtection::getInstance().initialize();  // Generates session token
CookieBlockManager::getInstance().initialize(profilePath);
BookmarkManager::getInstance().initialize(profilePath);
```

**Task 2.3: Pre-create overlays at startup** (Day 4)

In `OnAfterCreated` for the header browser (same pattern as Windows `cef_browser_shell.cpp` ~line 2870):
```objc
// After header browser is created:
CreateCookiePanelOverlay();
CreateDownloadPanelOverlay();
CreateProfilePanelOverlay();
CreateMenuOverlay();
```

### Phase 3: Process Lifecycle + Keyboard Shortcuts (Days 5-6)

**Task 3.1: Auto-launch wallet and adblock servers**

```objc
// Pattern for posix_spawn:
pid_t StartServer(const char* path, const char* const argv[]) {
    pid_t pid;
    posix_spawn(&pid, path, NULL, NULL, (char* const*)argv, environ);
    return pid;
}

// Graceful shutdown:
void StopServer(pid_t pid, const char* shutdownUrl) {
    // 1. HTTP POST /shutdown (cross-platform endpoint already exists)
    SyncHttpClient::Post(shutdownUrl, "");
    // 2. Wait up to 3 seconds
    int status;
    for (int i = 0; i < 30; i++) {
        if (waitpid(pid, &status, WNOHANG) > 0) return;
        usleep(100000); // 100ms
    }
    // 3. Force kill
    kill(pid, SIGTERM);
    waitpid(pid, &status, 0);
}
```

Register cleanup in `applicationWillTerminate:` or the `ShutdownApplication()` function.

**Task 3.2: Keyboard shortcuts — Cmd instead of Ctrl**

In `simple_handler.cpp` keyboard handler, the modifier check needs platform conditioning:

```cpp
#ifdef _WIN32
    bool isModifier = (event.modifiers & EVENTFLAG_CONTROL_DOWN);
#elif defined(__APPLE__)
    bool isModifier = (event.modifiers & EVENTFLAG_COMMAND_DOWN);
#endif
```

| Action | Windows | macOS |
|--------|---------|-------|
| New tab | Ctrl+T | Cmd+T |
| Close tab | Ctrl+W | Cmd+W |
| Find | Ctrl+F | Cmd+F |
| Reload | Ctrl+R | Cmd+R |
| DevTools | Ctrl+Shift+I | Cmd+Option+I |
| Address bar | Ctrl+L | Cmd+L |
| Bookmark | Ctrl+D | Cmd+D |
| Print | Ctrl+P | Cmd+P |
| Quit | Alt+F4 | Cmd+Q |
| Preferences | — | Cmd+, |

### Phase 4: Multi-Window Support (Days 7-10)

This phase ports Sprint 13 (BrowserWindow + WindowManager + tab tear-off). See Section 8 for detailed architecture.

**Task 4.1: BrowserWindow macOS members**

Add `#elif defined(__APPLE__)` block to `BrowserWindow.h`:
```cpp
#elif defined(__APPLE__)
    void* ns_window = nullptr;           // NSWindow*
    void* header_view = nullptr;         // NSView*
    // One NSWindow* per overlay type (11 total)
    void* settings_overlay_window = nullptr;
    void* wallet_overlay_window = nullptr;
    // ... etc for all 11 overlay types
    // Event monitors (replace HHOOK)
    id omnibox_event_monitor = nil;
    id cookie_panel_event_monitor = nil;
    // ... etc
#endif
```

**Task 4.2: WindowManager::CreateFullWindow (macOS)**

Follow the 10-step checklist in Section 8.

**Task 4.3: Tab reparenting**

`MoveTabToWindow()` needs a macOS `#elif`:
```objc
NSView* tabView = (__bridge NSView*)tab->view_ptr;
[tabView removeFromSuperview];
NSView* contentView = [(__bridge NSWindow*)target_bw->ns_window contentView];
[tabView setFrame:...];  // Size to fit below header
[contentView addSubview:tabView];
```

**Task 4.4: Ghost tab window**

Replace GDI-painted `HodosGhostTab` WNDCLASS with NSWindow equivalent. See Section 8.8.

### Phase 5: Integration Testing + Polish (Days 11-14)

**Task 5.1: Test full site basket**

Run through the standard test sites (see `development-docs/browser-core/test-site-basket.md`):
- Auth: x.com, google.com, github.com
- Video: youtube.com, twitch.tv
- News: nytimes.com, reddit.com
- E-commerce: amazon.com

**Task 5.2: Data path verification**

Confirm all storage goes to correct macOS locations:
- Wallet DB: `~/Library/Application Support/HodosBrowser/wallet/wallet.db`
- History: `~/Library/Application Support/HodosBrowser/Default/HodosHistory`
- Bookmarks: `~/Library/Application Support/HodosBrowser/Default/bookmarks.db`
- Adblock: `~/Library/Application Support/HodosBrowser/adblock/`
- Settings: `~/Library/Application Support/HodosBrowser/settings.json`

**Task 5.3: Code signing and distribution**

See Section 9.

---

## 7. Technical Patterns and Gotchas

### 7.1 Coordinate System

macOS uses **bottom-left origin** (Y increases upward). Windows uses **top-left origin** (Y increases downward). This affects:

- Window positioning: `[NSWindow setFrameOrigin:]` takes bottom-left point
- Screen coordinates from JavaScript (`e.screenY`) are top-left — must flip
- `[NSEvent mouseLocation]` returns bottom-left screen coords

**Conversion**:
```objc
CGFloat screenHeight = [[NSScreen mainScreen] frame].size.height;
CGFloat macY = screenHeight - windowsY;
```

### 7.2 Overlay Close-Prevention Race Condition

**Critical lesson from Windows development**: React IPC is async. `WM_ACTIVATE` / `windowDidResignKey:` fires synchronously when focus changes. If you set a "prevent close" flag via React IPC, the flag may not arrive before the focus-loss handler fires.

**Safe pattern**: Set close-prevention flags synchronously from C++ (in `CreateXxxOverlay()`), then let React opt-in to allow close via IPC once the user reaches a safe state.

Example: Wallet overlay sets `g_wallet_overlay_prevent_close = true` at creation. React sends `wallet_allow_close` once the user sees the dashboard (safe state). React sends `wallet_prevent_close` when entering mnemonic display or PIN entry (unsafe state).

### 7.3 Mutex Deadlock

**Never call platform APIs (Cocoa, SendMessage, etc.) while holding the WindowManager mutex.** Cocoa's main thread is the UI thread. If a callback from `setNeedsDisplay:` or `setFrame:` tries to lock WindowManager, you deadlock.

**Pattern**: Collect data under lock, then operate outside the lock:
```cpp
BrowserWindow* bw = nullptr;
{
    std::lock_guard<std::mutex> lock(mutex_);
    bw = windows_[id].get();
}
// Now safe to call platform APIs with bw
[(__bridge NSWindow*)bw->ns_window setFrame:...];
```

### 7.4 CEF Framework Loading (macOS-specific)

Must call `cef_load_library()` before any CEF API usage:
```objc
NSString* frameworkPath = [[[NSBundle mainBundle] privateFrameworksPath]
    stringByAppendingPathComponent:@"Chromium Embedded Framework.framework/Chromium Embedded Framework"];
if (!cef_load_library([frameworkPath UTF8String])) {
    NSLog(@"Failed to load CEF framework");
    return 1;
}
```

### 7.5 Initialization Order

```
1. cef_load_library()
2. CreateMainWindow()  ← Before CefInitialize
3. ProfileManager + ProfileLock + SettingsManager init
4. CefInitialize()
5. OnContextInitialized → create header browser, pre-create overlays
```

Windows and macOS use the same order. Changing this order will break the app.

### 7.6 GPU Process

macOS CEF currently uses in-process GPU:
```cpp
command_line->AppendSwitch("in-process-gpu");
command_line->AppendSwitch("disable-gpu-sandbox");
```
This avoids GPU process crashes on unsigned apps. Once code-signed with proper entitlements, out-of-process GPU may work (test carefully).

### 7.7 Storing BrowserWindow* in NSWindow

Windows uses `SetWindowLongPtr(GWLP_USERDATA, bw)` to store a `BrowserWindow*` on each HWND. On macOS, use Objective-C associated objects:

```objc
#import <objc/runtime.h>
static const char kBrowserWindowKey = 0;

// Store
objc_setAssociatedObject(nsWindow, &kBrowserWindowKey,
    [NSValue valueWithPointer:bw], OBJC_ASSOCIATION_RETAIN_NONATOMIC);

// Retrieve
NSValue* val = objc_getAssociatedObject(nsWindow, &kBrowserWindowKey);
BrowserWindow* bw = (BrowserWindow*)[val pointerValue];
```

### 7.8 Handler Retargeting for Shared Overlays

Pre-created overlays (cookie, download, profile, menu) are created once at startup. When shown for a different window, their `SimpleHandler::window_id_` must be updated:

```cpp
handler->SetWindowId(target_window->window_id);
```

Also reposition relative to the target window's toolbar icons.

---

## 8. Multi-Window Architecture

> Based on Sprint 13 Windows implementation. These patterns will repeat on macOS.

### 8.1 BrowserWindow Class

Per-window state container. See `include/core/BrowserWindow.h`. Holds all HWNDs/NSWindows, browser refs, mouse hooks/event monitors, and icon offsets.

### 8.2 WindowManager Singleton

Platform-agnostic at `include/core/WindowManager.h`. Already has `#ifdef _WIN32`. Key methods needed for macOS:

| Method | macOS Implementation |
|--------|---------------------|
| `CreateFullWindow()` | Create NSWindow + header NSView + CEF browser + 11 overlays |
| `GetWindowByNSWindow(void*)` | Lookup BrowserWindow by NSWindow pointer |
| `GetWindowAtScreenPoint(x, y)` | Iterate windows, check `NSPointInRect` (with Y flip) |

### 8.3 CreateFullWindow macOS Checklist

1. Create `NSWindow` (titled, closable, resizable, miniaturizable)
2. Store `BrowserWindow*` via `objc_setAssociatedObject`
3. Create header `NSView` (99px or 12% of height, positioned at top)
4. Create header CEF browser (`SetAsChild`, role `"header"`, pass `window_id`)
5. Set window delegate (resize/move/close/focus handlers)
6. Create initial NTP tab (if `createInitialTab == true`)
7. Force `WasResized()` on all other windows (prevents stale render)
8. Register `NSEvent` monitors for click-outside detection
9. Show: `[nsWindow makeKeyAndOrderFront:nil]`
10. `SetActiveWindowId(wid)`

### 8.4 Tab Reparenting (Tear-Off / Merge)

```objc
// MoveTabToWindow — macOS block:
NSView* tabView = (__bridge NSView*)tab->view_ptr;
[tabView removeFromSuperview];
NSView* contentView = [(__bridge NSWindow*)target_bw->ns_window contentView];
NSRect bounds = [contentView bounds];
CGFloat headerH = MAX(100, bounds.size.height * 0.12);
[tabView setFrame:NSMakeRect(0, 0, bounds.size.width, bounds.size.height - headerH)];
[contentView addSubview:tabView];
tab->browser->GetHost()->WasResized();
```

### 8.5 Ghost Tab Window

Replace Windows GDI ghost with Cocoa equivalent:
```objc
NSWindow* ghost = [[NSWindow alloc]
    initWithContentRect:NSMakeRect(0, 0, width, height)
    styleMask:NSWindowStyleMaskBorderless
    backing:NSBackingStoreBuffered defer:NO];
[ghost setLevel:NSFloatingWindowLevel];
[ghost setAlphaValue:0.85];
[ghost setOpaque:NO];
[ghost setHasShadow:YES];
// Track cursor with NSTimer (16ms interval) + [NSEvent mouseLocation]
// Flip Y: screenY = [[NSScreen mainScreen] frame].size.height - mouseLocation.y
```

### 8.6 Merge Detection

No direct `WindowFromPoint()` on macOS. Iterate and hit-test:
```cpp
BrowserWindow* WindowManager::GetWindowAtScreenPoint(int screenX, int screenY) {
    CGFloat screenH = [[NSScreen mainScreen] frame].size.height;
    NSPoint pt = NSMakePoint(screenX, screenH - screenY);  // Flip Y
    std::lock_guard<std::mutex> lock(mutex_);
    for (auto& [id, win] : windows_) {
        NSWindow* nsw = (__bridge NSWindow*)win->ns_window;
        if (nsw && NSPointInRect(pt, [nsw frame])) return win.get();
    }
    return nullptr;
}
```

### 8.7 Window Close Behavior

| Event | Windows | macOS |
|-------|---------|-------|
| Close last tab | `PostMessage(WM_CLOSE)` | `[nsWindow close]` |
| Close last window | `ShutdownApplication()` | `[NSApp terminate:nil]` |
| Window cleanup | `WM_CLOSE` handler | `windowWillClose:` delegate |
| Check if last | `GetWindowCount() == 0` | Same check |

---

## 9. App Bundle, Code Signing and Distribution

### App Bundle Structure

```
HodosBrowserShell.app/
  Contents/
    MacOS/HodosBrowserShell
    Frameworks/
      Chromium Embedded Framework.framework/
      HodosBrowserShell Helper.app/
      HodosBrowserShell Helper (GPU).app/
      HodosBrowserShell Helper (Renderer).app/
      HodosBrowserShell Helper (Plugin).app/
      HodosBrowserShell Helper (Alerts).app/
    Resources/
      app.icns                    ← TODO: app icon
      *.pak files, locales/
    Info.plist
```

### Info.plist — Missing Usage Descriptions

```xml
<key>NSCameraUsageDescription</key>
<string>Websites may request access to your camera for video calls.</string>
<key>NSMicrophoneUsageDescription</key>
<string>Websites may request access to your microphone for audio calls.</string>
<key>NSLocationUsageDescription</key>
<string>Websites may request access to your location.</string>
```

### Entitlements

Required for CEF + Keychain + WebRTC:

```xml
<dict>
    <key>com.apple.security.cs.allow-jit</key><true/>
    <key>com.apple.security.cs.disable-library-validation</key><true/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key><true/>
    <key>com.apple.security.device.camera</key><true/>
    <key>com.apple.security.device.microphone</key><true/>
    <key>com.apple.security.personal-information.location</key><true/>
    <key>com.apple.security.network.client</key><true/>
    <key>com.apple.security.network.server</key><true/>
    <key>com.apple.security.files.user-selected.read-write</key><true/>
    <key>com.apple.security.files.downloads.read-write</key><true/>
</dict>
```

### Code Signing Process

1. Get Apple Developer ID ($99/year)
2. Sign inside-out: helpers → framework → main app
3. Enable Hardened Runtime (required for notarization)
4. `xcrun notarytool submit HodosBrowser.zip --apple-id X --team-id Y --password Z`
5. `xcrun stapler staple HodosBrowser.app`

Without signing, Gatekeeper blocks the app. For development, use `--no-sandbox` flag and accept Keychain prompts.

---

## 10. Reference Files

### macOS-Specific Files

| File | Lines | Purpose |
|------|-------|---------|
| `cef-native/cef_browser_shell_mac.mm` | 1825 | macOS entry point, NSWindow/NSView, 5 overlays |
| `cef-native/src/core/TabManager_mac.mm` | 446 | Tab lifecycle with NSView |
| `cef-native/src/core/WalletService_mac.cpp` | 267 | libcurl-based wallet HTTP client |
| `cef-native/src/handlers/my_overlay_render_handler.mm` | 358 | OSR rendering via CALayer |
| `cef-native/mac/process_helper_mac.mm` | 65 | CEF helper subprocess entry |
| `cef-native/Info.plist` | — | App bundle config |

### Key Cross-Platform Files (will need `#elif defined(__APPLE__)` additions)

| File | What Needs macOS Work |
|------|----------------------|
| `cef_browser_shell.cpp` | Reference for globals, overlay creation, process lifecycle |
| `include/core/BrowserWindow.h` | Add macOS member fields |
| `include/core/WindowManager.h` | Add `CreateFullWindow()` macOS impl |
| `src/core/WindowManager.cpp` | Add macOS implementations |
| `src/core/HttpRequestInterceptor.cpp` | Singleton calls → SyncHttpClient |
| `simple_handler.cpp` | Keyboard shortcuts modifier check |

### Documentation

| File | Purpose |
|------|---------|
| `CLAUDE.md` (root) | Master project context, architecture, invariants |
| `cef-native/CLAUDE.md` | C++ layer details, IPC patterns, window hierarchy |
| `development-docs/browser-core/CLAUDE.md` | Sprint details, cross-platform rules |
| `development-docs/browser-core/auth-cookies-profiles-guide.md` | Auth bypass layers guide |
| `development-docs/browser-core/test-site-basket.md` | Standard verification sites |

---

## Archived Documents

The following documents were consolidated into this handover. They contain historical detail from the Dec 2025 port but are no longer the source of truth:

- `MACOS_PORT_SUCCESS.md` — Dec 2025 success summary
- `MACOS_IMPLEMENTATION_COMPLETE.md` — Dec 2025 technical details
- `PHASE2_MACOS_SUPPORT_SUMMARY.md` — CMake build system details
- `MAC_PLATFORM_SUPPORT_PLAN.md` — Original planning doc (superseded by this document)
- `SPRINT13_MULTIWINDOW_MACOS_NOTES.md` — Multi-window lessons (folded into Section 8)
