# Sprint 13 Multi-Window — Lessons Learned for macOS Port

**Created**: 2026-03-03
**Context**: Sprint 13 Phases 1-2 (BrowserWindow refactor + Ctrl+N multi-window) completed on Windows. These notes capture architectural decisions, bugs encountered, and gotchas that will directly impact the macOS implementation.

---

## 1. Architecture Overview (What We Built)

### BrowserWindow Class
A per-window state container holding all HWNDs, CEF browser refs, mouse hooks, and icon offsets that were previously globals. Located in `include/core/BrowserWindow.h` and `src/core/BrowserWindow.cpp`.

**macOS equivalent needs**:
- Replace `HWND hwnd` → `NSWindow* ns_window`
- Replace `HWND header_hwnd` → `NSView* header_view`
- Replace all overlay HWNDs → `NSWindow*` overlay windows (settings, wallet, backup, brc100_auth, settings_menu, omnibox, cookie_panel, download_panel, profile_panel, menu)
- Mouse hooks (`HHOOK`) don't exist on macOS — use `NSEvent addLocalMonitorForEventsMatchingMask:` instead for click-outside detection
- Icon right-offsets are the same concept (positioning overlays relative to toolbar buttons)

```
// Windows BrowserWindow (current)         // macOS BrowserWindow (needed)
HWND hwnd                                  NSWindow* ns_window
HWND header_hwnd                           NSView* header_view
HWND settings_overlay_hwnd                 NSWindow* settings_overlay_window
HHOOK omnibox_mouse_hook                   id<NSObject> omnibox_event_monitor
int settings_icon_right_offset             int settings_icon_right_offset (same)
CefRefPtr<CefBrowser> header_browser       CefRefPtr<CefBrowser> header_browser (same)
```

### WindowManager Singleton
Platform-agnostic singleton at `include/core/WindowManager.h`. Already has `#ifdef _WIN32` for platform-specific methods. Key methods:

| Method | Windows | macOS Needed |
|--------|---------|--------------|
| `CreateFullWindow()` | Creates HWND + header child + CEF browser | Create NSWindow + header NSView + CEF browser |
| `GetWindowByHwnd(HWND)` | Lookup by HWND | Need `GetWindowByNSWindow(NSWindow*)` |
| `GetWindowForBrowser(int)` | Checks all browser refs | Same logic, no platform change |

### Tab.window_id
Added `int window_id = 0` to the `Tab` struct. This is already cross-platform — `Tab.h` has no platform conditionals. Works on macOS as-is.

---

## 2. Critical Bugs & Fixes (Will Repeat on macOS)

### Bug 1: Static Browser Getters Always Return Window 0

**Problem**: `SimpleHandler::GetWalletBrowser()`, `GetCookiePanelBrowser()`, etc. are static methods that always look up `WindowManager::GetWindow(0)->xxx_browser`. When an overlay is opened for window 2, its browser ref is stored in `BrowserWindow(2)`, but the static getter returns null.

**Windows fix**: Store `BrowserWindow*` in HWND via `SetWindowLongPtr(GWLP_USERDATA, bw)`, then read it in WndProc via `GetWindowLongPtr(GWLP_USERDATA)`.

**macOS equivalent**: Use `objc_setAssociatedObject` / `objc_getAssociatedObject` on NSWindow/NSView to store the `BrowserWindow*`:
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

Alternatively, use a `std::map<NSWindow*, BrowserWindow*>` in WindowManager (simpler, no ObjC runtime).

### Bug 2: Shared/Pre-Created Overlays Need Handler Retargeting

**Problem**: Pre-created overlays (cookie panel, download panel, profile panel, menu) are created once at startup in window 0. When shown for window 2, their `SimpleHandler::window_id_` must be updated so IPC routes to the correct window.

**Pattern (same on macOS)**: Before showing a shared overlay for a different window, update the handler:
```cpp
handler->SetWindowId(target_window->window_id);
```

Also reposition the overlay relative to the target window's toolbar icons (same on both platforms — just different positioning APIs: `SetWindowPos` vs `[NSWindow setFrame:]`).

### Bug 3: Wallet Overlay is Destroy-on-Close (Not Pre-Created)

Unlike other overlays, the wallet overlay is destroyed and recreated each time it's shown (via `CreateWalletOverlayWithSeparateProcess`). On macOS, this is `CreateWalletOverlay()` in `cef_browser_shell_mac.mm`. The recreated overlay's browser ref goes to the correct BrowserWindow, but the overlay's event handlers need access to the correct `BrowserWindow*` for mouse/keyboard forwarding.

**Windows fix**: `SetWindowLongPtr(GWLP_USERDATA, bw)` on wallet HWND at creation.
**macOS fix**: Same pattern — `objc_setAssociatedObject` on the wallet NSWindow at creation time so event handler methods can find the right browser ref.

### Bug 4: WM_SIZE Refresh After New Window Creation

**Problem**: Creating a new overlapping window left stale CEF render buffers in existing windows (vertical strip artifact on right side). The first window's header/tab area didn't repaint until manually resized.

**Windows fix**: After `CreateFullWindow()`, send `WM_SIZE` with current dimensions to all other windows to force `WasResized()` on their CEF browsers.

**macOS equivalent**: After creating a new window, call `WasResized()` on existing windows' header and tab browsers:
```objc
for (BrowserWindow* bw : otherWindows) {
    if (bw->header_browser)
        bw->header_browser->GetHost()->WasResized();
    // Also for active tab browser
}
```
Or trigger a relayout via `[nsView setNeedsDisplay:YES]`.

### Bug 5: Mutex Deadlock in WindowManager

**Problem**: Calling `SendMessage(WM_SIZE)` while holding `WindowManager::mutex_` caused deadlock because the WM_SIZE handler calls back into `WindowManager::GetWindow()` which locks the same mutex.

**Lesson**: NEVER call into platform window APIs (SendMessage, performSelector, etc.) while holding the WindowManager mutex. Always collect data under lock, then operate outside the lock.

**macOS risk**: Same pattern — if you call `[NSView setNeedsDisplay:]` or `[NSWindow setFrame:]` while holding the mutex, and the resulting callback tries to lock WindowManager, you'll deadlock. Cocoa's main thread is the UI thread, same as Windows.

---

## 3. Graceful Process Shutdown

### What We Changed
- Added `POST /shutdown` endpoint to wallet (cancels `CancellationToken` → Actix-web graceful shutdown → SQLite WAL flush)
- Added `POST /shutdown` endpoint to adblock engine (delayed `process::exit(0)`)
- C++ `StopWalletServer()` / `StopAdblockServer()` now: HTTP shutdown first → wait → `TerminateProcess` fallback

### macOS Implications
- The Rust endpoints are cross-platform — already work on macOS
- macOS process management uses `NSTask` or `posix_spawn` instead of `CreateProcessA`
- Kill fallback: `kill(pid, SIGTERM)` then `waitpid()` instead of `TerminateProcess`
- Job objects don't exist on macOS — use `PR_SET_PDEATHSIG` (Linux) or track child PIDs manually and kill in `applicationWillTerminate:`
- **Critical**: Must implement wallet/adblock process startup on macOS (currently missing — marked as TODO in `cef_browser_shell_mac.mm`)

```objc
// macOS process lifecycle pattern:
// Start: NSTask or posix_spawn
// Graceful stop: HTTP POST /shutdown, then waitpid with timeout
// Forceful stop: kill(pid, SIGKILL) as fallback
// Cleanup: applicationWillTerminate: handler
```

---

## 4. Per-Window Tab List IPC

### What We Changed
- Added `NotifyWindowTabListChanged(int window_id)` — sends tab list to ONE window only
- `TabManager::CreateTab()` uses per-window notify (was broadcasting to all windows)
- `CreateFullWindow()` uses per-window notify (was broadcasting to all windows)
- `NotifyTabListChanged()` (global) still exists for cases that genuinely affect all windows

### macOS Impact
- `SimpleHandler` IPC dispatch is already cross-platform — these changes work on macOS
- The `SendTabListToWindow()` helper uses `BrowserWindow*` which needs macOS members populated
- No additional macOS work needed for the IPC layer

---

## 5. Session Save/Restore

### Version 2 Format
```json
{
  "version": 2,
  "windows": [
    {
      "tabs": [{"url": "...", "title": "..."}],
      "activeTabIndex": 0,
      "x": 100, "y": 100, "width": 1200, "height": 800
    }
  ]
}
```

### macOS Considerations
- Window position uses `GetWindowRect` (Windows) → need `[NSWindow frame]` (macOS)
- macOS coordinate system is bottom-left origin — convert when saving/restoring
- Restore uses `SetWindowPos` (Windows) → `[NSWindow setFrame:display:]` (macOS)
- `SaveSession()` already has `#ifdef _WIN32` for the position capture — add `#elif defined(__APPLE__)` block
- The `session.json` format is the same on both platforms (just x/y/width/height numbers)

---

## 6. Window Close Behavior

### Current Logic
- Closing last tab in a window → auto-closes that window (via `PostMessage(WM_CLOSE)`)
- Closing last window → `ShutdownApplication()` → exits app
- WM_CLOSE on secondary window → closes tabs, header browser, overlays, removes from WindowManager

### macOS Equivalent
- `PostMessage(WM_CLOSE)` → `[nsWindow performClose:nil]` or `[nsWindow close]`
- `ShutdownApplication()` → `[NSApp terminate:nil]`
- Window delegate's `windowShouldClose:` handles cleanup (close tabs, header, overlays)
- `windowWillClose:` removes from WindowManager
- If `WindowManager::GetWindowCount() == 0` → `[NSApp terminate:nil]`

---

## 7. CreateFullWindow — macOS Implementation Checklist

When implementing `WindowManager::CreateFullWindow()` for macOS, these are the required steps (derived from the Windows implementation):

1. **Create NSWindow** — `NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable | NSWindowStyleMaskMiniaturizable`
2. **Store BrowserWindow* in NSWindow** — `objc_setAssociatedObject` or map lookup
3. **Create header NSView** — child of contentView, positioned at top (99px or 12% of height)
4. **Create header CEF browser** — `SetAsChild(header_view, ...)`, role `"header"`, pass `window_id`
5. **Set window delegate** — for resize, move, close, focus events
6. **Create initial NTP tab** (if `createInitialTab` is true)
7. **Force WasResized on other windows** (prevents stale render)
8. **Register event monitors** for click-outside detection (replaces mouse hooks)
9. **Show window** — `[nsWindow makeKeyAndOrderFront:nil]`
10. **Update active window** — `SetActiveWindowId(wid)`

### What to Skip (Not Needed on macOS)
- WS_CLIPCHILDREN — NSView handles clipping automatically
- GWLP_USERDATA — use associated objects instead
- Mouse hooks (HHOOK) — use `NSEvent addLocalMonitorForEventsMatchingMask:`
- `RegisterClass`/`CreateWindow` boilerplate — Cocoa handles window class registration

---

## 8. Phase 3-4 (Tear-Off/Merge) — Implemented on Windows, macOS Notes

### What Was Built (Windows)

**`TabManager::MoveTabToWindow(tab_id, target_window_id)`** — single method handles both tear-off and merge:
1. `SetParent(tab_hwnd, target_bw->hwnd)` — reparents HWND, preserves renderer/DOM/session
2. Resizes tab to fit target window (12% header, rest is tab area)
3. Updates `tab->window_id` and `tab->handler->SetWindowId()`
4. Manages active tab in source/target windows
5. Notifies both windows' frontends via `NotifyWindowTabListChanged()`
6. Auto-closes source window if empty (`PostMessage(WM_CLOSE)`)

**Unified `tab_tearoff` IPC handler** — receives `(tab_id, screen_x, screen_y)`, uses `WindowFromPoint()` + `GetAncestor(GA_ROOT)` + `GetWindowByHwnd()` to decide:
- Drop over another Hodos window → **merge** (call `MoveTabToWindow`)
- Drop elsewhere → **tear-off** (call `CreateFullWindow(false)` + position + `MoveTabToWindow`)

**`ReorderTabs()` fix** — no longer checks `order.size() == tabs_.size()`. Multi-window safe: preserves other windows' tab positions, only reorders the subset sent by frontend.

**Ghost Tab Window** — `HodosGhostTab` WNDCLASS, `WS_POPUP | WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW`. GDI-painted white rounded rect with Segoe UI title text, 85% opacity. Tracks cursor via `GetCursorPos()` on a 16ms `SetTimer`. Shown/hidden via `tab_ghost_show`/`tab_ghost_hide` IPC. React ghost is suppressed during tear-off.

**Frontend `setPointerCapture`** — CEF windowed header is a child HWND. Without capture, pointer events are lost when cursor enters sibling tab HWNDs. `setPointerCapture(pointerId)` on the tab element keeps events flowing. `.closest('.tab-close-btn')` guard prevents capture from swallowing close button clicks.

### Tab Reparenting
- **Windows**: `SetParent(tab_hwnd, new_parent_hwnd)` + `SetWindowPos` to resize — CEF preserves renderer process, DOM, JS context, cookies
- **macOS**: `[tabView removeFromSuperview]` + `[newContentView addSubview:tabView]` + `[tabView setFrame:]` — same CEF preservation confirmed. Call `browser->GetHost()->WasResized()` after.

### macOS `MoveTabToWindow` Implementation
The `MoveTabToWindow` logic is mostly cross-platform. Only the reparent + resize block needs `#elif defined(__APPLE__)`:
```objc
#elif defined(__APPLE__)
if (tab->view_ptr && target_bw->ns_window) {
    NSView* tabView = (__bridge NSView*)tab->view_ptr;
    NSView* contentView = [(__bridge NSWindow*)target_bw->ns_window contentView];
    [tabView removeFromSuperview];
    NSRect bounds = [contentView bounds];
    CGFloat headerH = MAX(100, bounds.size.height * 0.12);
    [tabView setFrame:NSMakeRect(0, 0, bounds.size.width, bounds.size.height - headerH)];
    [contentView addSubview:tabView];
}
#endif
```

### Drag Detection
- **Windows**: Frontend detects Y > 40px below tab bar, sends `tab_tearoff` IPC with `e.screenX`/`e.screenY`
- **macOS**: Same frontend logic works. `setPointerCapture` should work in CEF on macOS too (CEF calls the platform capture API internally). No native `NSDraggingSource` needed — the unified IPC approach is simpler and consistent.

### Ghost Tab Window (macOS)
The GDI ghost window needs a Cocoa equivalent:
```objc
// Create borderless transparent NSWindow
NSWindow* ghost = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, width, height)
    styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
[ghost setLevel:NSFloatingWindowLevel];       // Always on top
[ghost setAlphaValue:0.85];                    // 85% opaque
[ghost setBackgroundColor:[NSColor whiteColor]];
[ghost setOpaque:NO];
[ghost setHasShadow:YES];
// Add NSTextField subview for title text
// Use NSTimer or CVDisplayLink for cursor tracking via [NSEvent mouseLocation]
```

Key differences:
- `NSFloatingWindowLevel` instead of `WS_EX_TOPMOST`
- `[NSEvent mouseLocation]` instead of `GetCursorPos()` — returns screen coords in bottom-left origin
- `NSTimer` with 0.016s interval instead of `SetTimer(hwnd, 1, 16, nullptr)`
- Must flip Y coordinate: `screenY = [[NSScreen mainScreen] frame].size.height - mouseLocation.y`

### Window Positioning at Drop Point
- **Windows**: `SetWindowPos(new_bw->hwnd, nullptr, screenX - 100, screenY - 50, 0, 0, SWP_NOSIZE)`
- **macOS**: Screen coords from frontend are top-left origin (web convention). Must flip for Cocoa's bottom-left:
  ```objc
  CGFloat screenHeight = [[NSScreen mainScreen] frame].size.height;
  NSPoint origin = NSMakePoint(screenX - 100, screenHeight - screenY - windowHeight + 50);
  [nsWindow setFrameOrigin:origin];
  ```

### Merge Detection
- **Windows**: `WindowFromPoint(POINT)` + `GetAncestor(GA_ROOT)` + `GetWindowByHwnd()`
- **macOS**: Need custom lookup — no direct `WindowFromPoint` equivalent. Options:
  1. Iterate `WindowManager::GetAllWindows()`, check if point is inside each window's frame: `NSPointInRect(point, [nsWindow frame])` — simplest and sufficient for a few windows
  2. `[NSWindow windowNumberAtPoint:belowWindowWithWindowNumber:]` + lookup — more correct but complex

Recommended: Option 1 (iterate + hit-test). Add `GetWindowAtScreenPoint(int x, int y)` to WindowManager:
```cpp
#elif defined(__APPLE__)
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
#endif
```

### Performance Note
Tear-off is slow on Windows due to `CreateFullWindow()` creating 11 overlay HWNDs synchronously. On macOS, overlay NSWindows are lighter than HWNDs (no separate OS-level window class registration needed), but still expect noticeable delay from creating header CEF browser + 11 overlay browsers. Future optimization: lazy overlay creation (only create overlays when first shown, not at window creation).

---

## 9. Priority Order for macOS Multi-Window

Based on Sprint 13 experience, recommended implementation order for macOS:

1. **BrowserWindow macOS members** — Add `NSWindow*`, `NSView*`, overlay `NSWindow*` fields with `#elif defined(__APPLE__)` conditionals
2. **WindowManager::CreateFullWindow (macOS)** — Follow checklist in Section 7
3. **Window delegate per-window** — Resize, close, focus handlers using BrowserWindow*
4. **Event monitor registration** — Click-outside for overlays, per-window
5. **Tab reparenting** — `removeFromSuperview` + `addSubview`
6. **Process lifecycle** — `posix_spawn` for wallet/adblock, HTTP shutdown, `kill` fallback
7. **Session save/restore** — `[NSWindow frame]` for position capture/restore

---

## 10. Files Modified in Sprint 13 (Reference for macOS Port)

| File | What Changed | macOS Action Needed |
|------|-------------|-------------------|
| `include/core/BrowserWindow.h` | NEW — per-window state | Add `#elif defined(__APPLE__)` members |
| `src/core/BrowserWindow.cpp` | NEW — role-based browser lookup | Cross-platform, works as-is |
| `include/core/WindowManager.h` | NEW — singleton | Add `GetWindowByNSWindow()`, macOS `CreateFullWindow()` |
| `src/core/WindowManager.cpp` | NEW — window lifecycle | Add `#elif defined(__APPLE__)` CreateFullWindow |
| `include/core/Tab.h` | Added `window_id` | Already cross-platform |
| `src/core/TabManager.cpp` | Per-window notify, auto-close | Needs macOS equivalent of `PostMessage(WM_CLOSE)` |
| `cef_browser_shell.cpp` | Migrated 17 globals → BrowserWindow | `cef_browser_shell_mac.mm` needs same migration |
| `simple_handler.cpp` | Per-window IPC routing | Already cross-platform (uses BrowserWindow*) |
| `simple_handler.h` | Added `window_id_`, `GetOwnerWindow()` | Already cross-platform |
| `simple_app.cpp` | Multi-window session restore | Needs macOS window position API |
| `rust-wallet/src/handlers.rs` | Added `/shutdown` endpoint | Cross-platform, works as-is |
| `adblock-engine/src/handlers.rs` | Added `/shutdown` endpoint | Cross-platform, works as-is |
