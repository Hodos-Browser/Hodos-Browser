# Sprint 13: Tab Tear-Off (Multi-Window)

**Created**: 2026-03-02
**Status**: Planning
**Complexity**: High
**Estimated Effort**: 3-5 days

---

## Goal

Enable dragging a tab out of the tab bar to create a new browser window. Support moving tabs between windows. Single-process architecture (all windows share singletons and CEF SingletonLock).

---

## Prerequisites

| Prerequisite | Status |
|---|---|
| G4 Phase 4 (Tab drag-reorder) | COMPLETE |
| Sprint 9d (Multi-profile / per-profile isolation) | COMPLETE |

---

## Architecture

### Key Decisions (from prior research in G4-new-tab-page.md Phase 5)

- **HWND reparenting works in CEF** (confirmed via CEF forum: `SetParent()` on browser HWND preserves renderer process, DOM state, JS context, cookies).
- **Must stay single-process.** CEF enforces `SingletonLock` per `root_cache_path`. All windows run in one `HodosBrowserShell.exe` instance.
- **All singletons shared automatically.** `SettingsManager`, `AdblockCache`, `FingerprintProtection`, `EphemeralCookieManager`, `SessionManager`, `HistoryManager` are all in-process singletons. A new window inherits everything with zero extra work.
- **External services unaffected.** Rust wallet (port 3301) and adblock engine (port 3302) are separate processes that serve any window.
- **`Tab.window_id` field is minimal approach.** Keep `TabManager` as a singleton, add `int window_id` to `Tab` struct, filter by window ID when building per-window tab lists. This avoids a large refactor of TabManager into per-window instances.

### Current State (What Needs to Change)

The C++ layer currently uses 17 global HWNDs in `cef_browser_shell.cpp`:

```
g_hwnd                          Main shell window (WS_OVERLAPPEDWINDOW)
g_header_hwnd                   Header browser (tab bar + toolbar)
g_webview_hwnd                  Legacy, unused
g_settings_overlay_hwnd         (10 more overlay HWNDs...)
g_wallet_overlay_hwnd
g_menu_overlay_hwnd
...etc
```

This "one window, many globals" pattern cannot support multiple windows. The globals must move into a `BrowserWindow` class, and a `WindowManager` singleton must track all instances.

### New Classes Needed

#### BrowserWindow

Owns one top-level HWND with header browser + webview area. Replaces the current global `g_hwnd` / `g_header_hwnd` pattern.

```cpp
class BrowserWindow {
public:
    int window_id;                          // Unique window identifier
    HWND hwnd;                              // Top-level shell window (macOS: NSWindow*)
    HWND header_hwnd;                       // Header CEF browser HWND
    CefRefPtr<CefBrowser> header_browser;   // Header browser instance

    // Overlay HWNDs — each window gets its own set
    HWND settings_overlay_hwnd;
    HWND wallet_overlay_hwnd;
    HWND menu_overlay_hwnd;
    HWND download_panel_overlay_hwnd;
    // ...etc (all current g_*_overlay_hwnd globals)

    // Window geometry
    void Resize(int width, int height);
    void Show();
    void Close();

    // Tab area management
    void ResizeActiveTab(int x, int y, int width, int height);
};
```

Each `BrowserWindow` has its own header CEF browser that loads `http://127.0.0.1:5137/` with a role like `"header_0"`, `"header_1"`, etc. The header's React app renders only tabs belonging to that window.

#### WindowManager (Singleton)

Tracks all `BrowserWindow` instances. Replaces direct access to global HWNDs.

```cpp
class WindowManager {
public:
    static WindowManager& GetInstance();

    // Lifecycle
    int CreateWindow(int x, int y, int width, int height);   // Returns window_id
    void CloseWindow(int window_id);
    void CloseAllWindows();                                   // App exit

    // Queries
    BrowserWindow* GetWindow(int window_id);
    BrowserWindow* GetActiveWindow();                         // Foreground window
    std::vector<BrowserWindow*> GetAllWindows();
    int GetWindowCount() const;

    // Tab-window association
    void MoveTabToWindow(int tab_id, int target_window_id);
    int GetWindowForTab(int tab_id);

private:
    std::map<int, std::unique_ptr<BrowserWindow>> windows_;
    int active_window_id_;
    int next_window_id_ = 0;
};
```

### Tab Struct Changes

Add `window_id` to the existing `Tab` struct in `include/core/Tab.h`:

```cpp
struct Tab {
    // ... existing fields ...
    int id;
    std::string title;
    std::string url;
    // ... etc ...

    // NEW: Which window this tab belongs to (0 = main window)
    int window_id = 0;
};
```

### TabManager Changes

`TabManager` stays as a global singleton but becomes window-aware:

- `GetAllTabs()` returns all tabs across all windows (for session save).
- New: `GetTabsForWindow(int window_id)` returns only tabs belonging to that window.
- `CreateTab()` gains an optional `window_id` parameter (defaults to active window).
- `CloseTab()` checks if closing the last tab in a window; if so, signals `WindowManager::CloseWindow()`.
- `SwitchToTab()` activates the tab's parent window if it differs from the current foreground window.

### Tab Reparenting Flow

```
1. User drags tab beyond tab bar threshold (15px vertical, matches Chrome)
2. Frontend sends `tab_tearoff` IPC: { tabId, screenX, screenY }
3. C++ WindowManager::CreateWindow() at drop position
4. Tab's CEF browser HWND reparented:
     HWND browser_hwnd = tab.browser->GetHost()->GetWindowHandle();
     SetParent(browser_hwnd, new_window.hwnd);
     MoveWindow(browser_hwnd, 0, header_height, width, tab_height, TRUE);
     tab.browser->GetHost()->WasResized();
5. Tab.window_id updated in TabManager
6. Source window's header browser notified → re-renders tab bar
7. New window's header browser notified → renders with the torn-off tab
8. If source window has 0 remaining tabs → WindowManager::CloseWindow()
```

### Tab Merge Flow (Drop onto Existing Window)

```
1. User drags tab from Window B over Window A's tab bar
2. Window A's header detects external drag (native drag data or IPC)
3. Frontend sends `tab_merge` IPC: { tabId, targetWindowId, insertIndex }
4. C++ reparents tab HWND to target window
5. Tab.window_id updated
6. Both windows' header browsers notified to re-render tab bars
7. If source window has 0 remaining tabs → close it
```

### IPC Changes

| IPC Message | Direction | Payload | Purpose |
|---|---|---|---|
| `tab_tearoff` | Frontend -> C++ | `{ tabId, screenX, screenY }` | Initiate tear-off |
| `tab_merge` | Frontend -> C++ | `{ tabId, targetWindowId, insertIndex }` | Move tab to existing window |
| `tab_list_response` | C++ -> Frontend | Tab array (filtered by window) | Each header only sees its own tabs |
| `window_created` | C++ -> Frontend | `{ windowId }` | Inform header of new window context |
| `new_window` | Frontend -> C++ | `{}` | Ctrl+N handler |

### Overlay Strategy

**Decision: Each window gets its own overlay set.**

Rationale: Overlays are positioned relative to their parent window's toolbar icons. Sharing overlays across windows would require repositioning on every show, and CEF overlay HWNDs are parented to a specific window. The simplest correct approach is one overlay set per `BrowserWindow`.

This is the largest code duplication concern but is structurally necessary. The overlay creation logic can be factored into a helper that `BrowserWindow` calls during construction.

---

## Implementation Phases

### Phase 1: BrowserWindow Class (Foundation Refactor)

**Goal**: Extract current global window state into `BrowserWindow` class. No behavior change; backwards-compatible refactor.

**Changes**:
- Create `include/core/BrowserWindow.h` and `src/core/BrowserWindow.cpp`
- Create `include/core/WindowManager.h` and `src/core/WindowManager.cpp`
- Move all 17 global HWNDs from `cef_browser_shell.cpp` into `BrowserWindow` members
- `WindowManager` creates a single `BrowserWindow` (window_id=0) at startup
- All existing code that references `g_hwnd` etc. updated to `WindowManager::GetInstance().GetActiveWindow()->hwnd`
- `WndProc` and overlay WndProcs route through `WindowManager` to find the correct `BrowserWindow`
- Add `int window_id = 0` to `Tab` struct (unused in this phase)
- Update `CMakeLists.txt` with new source files

**Acceptance criteria**: Browser builds, runs, and behaves identically to current behavior. All overlays work. All tabs work. This is purely a refactor.

**Estimated effort**: 1-1.5 days (large surface area, but mechanical changes)

**Risk**: High surface area — touches nearly every file that references global HWNDs. Must be tested carefully.

### Phase 2: Multi-Window Support (Ctrl+N)

**Goal**: Support creating additional browser windows via keyboard shortcut.

**Changes**:
- `WindowManager::CreateWindow()` creates a new `BrowserWindow` with its own HWND, header browser, and overlay set
- Register new window class or reuse `CEFShellWindow` with per-instance data (`SetWindowLongPtr`)
- `WndProc` dispatches to the correct `BrowserWindow` based on HWND
- Ctrl+N in `OnPreKeyEvent` calls `WindowManager::CreateWindow()` + creates NTP tab
- Window close (`WM_CLOSE`) calls `WindowManager::CloseWindow(window_id)`
- If last window closes, trigger `ShutdownApplication()`
- `get_tab_list` IPC response filtered by `window_id` so each header browser only shows its window's tabs
- Header browser role includes window ID: `"header_0"`, `"header_1"`, etc.
- Session save/restore updated for multi-window: `session.json` becomes array of window objects

**Acceptance criteria**:
- Ctrl+N opens a new window with NTP
- Each window has independent tab bar, toolbar, and overlays
- Closing all windows exits the app
- Session save captures all windows; restore recreates them

**Estimated effort**: 1-1.5 days

### Phase 3: Tab Tear-Off

**Goal**: Dragging a tab vertically out of the tab bar creates a new window with that tab.

**Changes**:
- `TabBar.tsx`: detect when drag Y exceeds 15px threshold below tab bar bottom
- On threshold exceeded, send `tab_tearoff` IPC with `{ tabId, screenX, screenY }`
- C++ handler:
  1. `WindowManager::CreateWindow(screenX, screenY, defaultWidth, defaultHeight)`
  2. `SetParent(tab.browser->GetHost()->GetWindowHandle(), new_window.hwnd)`
  3. Update window styles: remove `WS_CHILD`, add `WS_CHILD` to new parent
  4. `MoveWindow()` + `WasResized()`
  5. Update `tab.window_id`
  6. Notify both windows' header browsers to refresh tab lists
  7. If source window has 0 tabs, close it
- Cancel tear-off if user drags back up before releasing (return tab to original position)
- New window appears at the screen coordinates where the user dropped the tab
- Visual feedback during drag: ghost tab image follows cursor (optional polish)

**Acceptance criteria**:
- Drag tab down out of strip -> new window appears at drop position with that tab
- Page does NOT reload (state, scroll position, form data preserved)
- Login state preserved (cookies shared in same process)
- Source window still works with remaining tabs
- If last tab torn off, source window closes
- Ad blocking, fingerprint protection, wallet all work in new window

**Estimated effort**: 0.5-1 day (building on Phase 2 infrastructure)

### Phase 4: Tab Merge (Drop onto Existing Window)

**Goal**: Drag a tab from one window into another window's tab bar.

**Changes**:
- During tear-off drag, detect if cursor is over another window's tab bar area
- If so, highlight the target tab bar with a drop indicator
- On drop, send `tab_merge` IPC instead of `tab_tearoff`
- C++ handler reparents tab HWND to target window, updates `window_id`
- If source window has 0 tabs, close it
- Handle edge case: merging last tab of a window (window closes after merge)

**Acceptance criteria**:
- Drag tab from Window A into Window B's tab bar -> tab moves to Window B
- Tab appears at the correct insertion point in Window B
- Page state preserved (no reload)
- Source window closes if it was the last tab

**Estimated effort**: 0.5 day

### Phase 5: macOS Support

**Goal**: All multi-window and tear-off functionality on macOS.

**Changes**:
- `BrowserWindow` macOS variant uses `NSWindow` + `NSView` instead of `HWND`
- `WindowManager` macOS variant:
  - Window creation via `[[NSWindow alloc] initWithContentRect:...]`
  - Each window gets its own `NSWindowDelegate`
- Tab reparenting via `[tabView removeFromSuperview]` + `[newContentView addSubview:tabView]`
- Cocoa drag-and-drop: `NSDraggingSource` / `NSDraggingDestination` protocols for inter-window tab drag
- Overlay HWNDs replaced with `NSWindow` children (already the pattern in `cef_browser_shell_mac.mm`)
- Menu bar handling: macOS has a single app menu bar, not per-window — should "just work"

**Estimated effort**: 1-1.5 days

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Large refactor surface in Phase 1 | Could break existing functionality | Test every overlay, every keyboard shortcut, every IPC handler after refactor. Keep Phase 1 as a pure no-behavior-change refactor. |
| CEF focus management across windows | Wrong window gets keyboard input | Use `SetForegroundWindow()` + `browser->GetHost()->SetFocus(true)` on window activation. Test with text input in address bar across multiple windows. |
| Overlay positioning with multiple windows | Overlays appear on wrong window | Each `BrowserWindow` owns its overlays. Overlay show/hide IPC must route to the correct window's overlay HWND. |
| Session save/restore complexity | Multi-window session state is harder to serialize | Phase 2 updates `session.json` format. Include migration path: if `session.json` is old single-window format, load as window_id=0. |
| Header browser per window | Memory/process overhead | Each window spawns a header renderer process. Acceptable for reasonable window counts (< 10). Document this trade-off. |
| `EphemeralCookieManager` tab tracking | `OnTabClosed` / `OnTabNavigated` assume single window | These methods use tab IDs not window IDs, so they should work unchanged. Verify during Phase 2 testing. |
| `Ctrl+W` / `Ctrl+T` targeting | Must target the foreground window | `OnPreKeyEvent` fires on the browser that has focus. The handler must determine which `BrowserWindow` owns that browser and operate on it. Use `WindowManager::GetWindowForTab(tab_id)`. |

---

## Open Questions (Decide at Implementation Time)

1. **Should tear-off windows be resizable independently?** Almost certainly yes — each is a full `WS_OVERLAPPEDWINDOW`.
2. **What is the minimum window size?** Recommend 400x300 to prevent toolbar/tab bar overlap.
3. **Should closing the "main" window (window_id=0) close all windows?** Recommend no — closing any window just closes that window. Last window close exits the app. There is no "main" window distinction after Phase 2.
4. **Ctrl+Shift+T (reopen closed tab) across windows?** Start simple: reopen in whichever window the shortcut was pressed in. Cross-window undo is a future enhancement.
5. **Title bar / taskbar behavior?** Each window should appear as a separate taskbar entry on Windows. On macOS, `Cmd+backtick` cycles between windows (standard NSApp behavior).

---

## Test Plan

### Phase 1 (Refactor Verification)
- All existing functionality works identically (full regression)
- Every overlay opens/closes correctly
- Tab create/close/switch works
- Keyboard shortcuts (Ctrl+T/W/F/H/J/D, Alt+Left/Right) all work
- Session save and restore works
- Ad blocking, fingerprint protection, cookie management unchanged

### Phase 2 (Multi-Window)
- Ctrl+N opens a new window with NTP
- Each window has its own tab bar showing only its tabs
- Tabs created in Window B stay in Window B
- Closing all tabs in a window closes that window
- Closing last window exits app
- Overlays (wallet, settings, downloads, etc.) work in each window independently
- Session restore with 2+ windows recreates correct window/tab layout

### Phase 3 (Tab Tear-Off)
- Drag tab downward 15+ px below tab bar -> new window at drop coordinates
- Page does NOT reload (form data, scroll position, JS state preserved)
- Login cookies preserved in torn-off tab
- Source window functions normally with remaining tabs
- Tear off last tab -> source window closes, new window has the tab
- Ad blocking badge count correct in both windows
- Privacy shield panel shows correct domain in torn-off tab's window
- Find-in-page works in torn-off tab

### Phase 4 (Tab Merge)
- Drag tab from Window A into Window B's tab bar -> tab moves
- Correct insertion index in target tab bar
- No reload on merge
- Source window closes if last tab merged out
- Merge all tabs into one window -> only one window remains

### Phase 5 (macOS)
- All above tests pass on macOS
- Cmd+N opens new window
- Tab tear-off via NSView reparenting
- `Cmd+backtick` cycles between windows
- Native window chrome (traffic lights) on each window

---

## References

- [Chromium TabDragController](https://chromium.googlesource.com/chromium/src/+/2accab56152b3890473024585376ab231268c015/chrome/browser/ui/views/tabs/tab_drag_controller.cc) -- drag thresholds, detach/attach model
- [CEF Forum: HWND Reparenting](https://magpcss.org/ceforum/viewtopic.php?f=6&t=10290) -- confirmed working
- [CEF Forum: SingletonLock / Cache Sharing](https://magpcss.org/ceforum/viewtopic.php?t=19759) -- why multi-instance is wrong, must use single process
- [CEF Forum: Tab Management](https://www.magpcss.org/ceforum/viewtopic.php?f=10&t=16287) -- "CEF supports multiple browsers but your application is responsible for UX"
- [Igalia: Fallback Tab Dragging for Wayland](https://blogs.igalia.com/max/fallback-tab-dragging/) -- overview of Chrome's drag mechanics

---

**Last Updated**: 2026-03-02
