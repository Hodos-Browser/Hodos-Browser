# G4: New Tab Page, Tab Drag-Reorder & Tab Tear-Off

**Status**: Phase 1 Complete (2026-03-02)
**Complexity**: High (multi-phase)
**Estimated Phases**: 7 (added Phase 1b: Ctrl+T keyboard shortcut fix)

---

## Current State

- **Homepage**: Set to `coingeek.com` by default, configurable via General settings
- **New tab**: Opens branded Hodos NTP (`hodos://newtab`) with logo, search bar, most-visited tiles
- **NTP caching**: Tile data + favicon base64 cached in localStorage — renders instantly on subsequent opens
- **Default tiles**: coingeek.com + metanetapps.com shown for first-time users (replaced by history as user browses)
- **No separation**: Homepage and new tab page are not independently configurable
- **No right-click "Set as homepage"** option
- **No tab reordering**: Tabs render in creation order, no drag-and-drop
- **No tab tear-off**: Cannot pull a tab out to create a new window
- **Ctrl+T bug**: Opens new Chromium window instead of Hodos tab (Phase 1b)

---

## What Needs to Happen

### Phase 1: New Tab Page Component

**Goal**: Create a branded new tab page that loads when the user opens a new tab (Ctrl+T or "+" button).

**Design**:
```
┌─────────────────────────────────────────────┐
│                                             │
│              [Hodos Logo]                   │
│                                             │
│     ┌─────────────────────────────┐         │
│     │  🔍  Search or enter URL    │         │
│     └─────────────────────────────┘         │
│                                             │
│     ┌────┐  ┌────┐  ┌────┐  ┌────┐         │
│     │ 🌐 │  │ 🌐 │  │ 🌐 │  │ 🌐 │         │
│     │Site│  │Site│  │Site│  │Site│         │
│     └────┘  └────┘  └────┘  └────┘         │
│     ┌────┐  ┌────┐  ┌────┐  ┌────┐         │
│     │ 🌐 │  │ 🌐 │  │ 🌐 │  │ 🌐 │         │
│     │Site│  │Site│  │Site│  │Site│         │
│     └────┘  └────┘  └────┘  └────┘         │
│                                             │
│        🛡️ 142 trackers blocked today        │
│                                             │
└─────────────────────────────────────────────┘
```

**Status**: COMPLETE (2026-03-02)

**Implemented**:
- [x] `NewTabPage.tsx` — dark theme (#1a1a1a), Hodos logo SVG, pill-shaped search bar, 4-column tile grid
- [x] Route `/newtab` in `App.tsx`, display URL `hodos://newtab` in `MainBrowserView.tsx`
- [x] Search bar: native `<input>`, auto-focused, reads search engine from settings, uses `isUrl()`/`toSearchUrl()`
- [x] Most-visited tiles: `get_most_visited` IPC → `HistoryManager::GetTopSites(8)` → JSON response
- [x] Default tiles: coingeek.com + metanetapps.com for first-time users (no history)
- [x] **localStorage cache**: tile URLs + base64 favicon data URLs cached in `ntp_tiles_cache` — loads synchronously on mount, no IPC wait
- [x] Favicon pre-fetch: fetches favicon images via `fetch()`, converts to base64 `FileReader.readAsDataURL()`, caches for instant render
- [x] Default URL changed from `metanetapps.com` → `http://127.0.0.1:5137/newtab` in useTabManager.ts, TabManager.cpp, TabManager_mac.mm
- [x] C++ `HistoryManager::GetTopSites(limit)` — `ORDER BY visit_count DESC, last_visit_time DESC`
- [x] C++ `AdblockCache::totalSessionBlocked_` atomic counter (added but NTP stats removed per user request)
- [x] Scrollbar suppression: `overflow: hidden` on html/body, `position: fixed` on root container
- [x] Privacy stats line: removed (user decided not needed)

### Phase 1b: Fix Ctrl+T in Tab Browsers

**Goal**: Ctrl+T pressed while focus is in a tab browser should create a new Hodos tab, not open a new Chromium window.

**Root cause**: `useKeyboardShortcuts` in the header's React handles Ctrl+T, but when focus is in a **tab browser** (different CEF process), the header's JS doesn't see the keypress. CEF's default Ctrl+T behavior opens a new browser window.

**Fix**: Add Ctrl+T handler to `SimpleHandler::OnPreKeyEvent` for tab browsers (like existing Ctrl+F, Ctrl+H, Ctrl+J, Ctrl+D handlers):
```cpp
// In OnPreKeyEvent, after existing key handlers:
if (event.windows_key_code == 'T' && role_.find("tab_") == 0) {
    if (event.modifiers & EVENTFLAG_CONTROL_DOWN) {
        CreateNewTabWithUrl("http://127.0.0.1:5137/newtab");
        SimpleHandler::NotifyTabListChanged();
        return true; // Consume — prevent CEF default new window
    }
}
```

Also handle Ctrl+W (close tab) from tab browsers for consistency — currently only the header handles it.

**Files**: `simple_handler.cpp` (OnPreKeyEvent section)

### Phase 2: Homepage vs New Tab Separation

**Goal**: Let users independently configure homepage (browser launch) and new tab page behavior.

**Changes needed**:
- [ ] Add `browser.newTabPage` setting: `"default"` (Hodos new tab) | `"blank"` | `"homepage"` | custom URL
- [ ] Add UI in General settings: "New tab page" dropdown/selector
- [ ] Update `TabManager::CreateTab()` to read this setting
- [ ] Homepage setting controls browser launch URL (already works)
- [ ] New tab setting controls Ctrl+T / "+" button URL

**Options for new tab page setting**:
| Option | Behavior |
|--------|----------|
| Hodos New Tab (default) | Shows the branded new tab page with search + tiles |
| Blank page | Opens `about:blank` |
| Homepage | Opens whatever homepage is set to |
| Custom URL | Opens a user-specified URL |

### Phase 3: Right-Click "Set as Homepage"

**Goal**: Users can right-click on a tab or in the address bar area to set the current page as their homepage.

**Changes needed**:
- [ ] Add "Set as homepage" to tab context menu (right-click on tab)
- [ ] Add "Set as homepage" to page context menu (right-click on page background)
- [ ] Handler: read current tab URL → update `browser.homepage` setting via SettingsManager
- [ ] Show toast/confirmation: "Homepage set to example.com"

**C++ changes (Windows only for now)**:
- [ ] Add `MENU_ID_SET_AS_HOMEPAGE` to context menu in `simple_handler.cpp`
- [ ] In `OnContextMenuCommand`, read active tab URL and call `SettingsManager::SetHomepage()`
- [ ] Send IPC notification to header for toast display
- [ ] When implementing macOS support, update `development-docs/macos-port/MAC_PLATFORM_SUPPORT_PLAN.md` with equivalent context menu handling

### Phase 4: Tab Drag-to-Reorder (Frontend Only)

**Goal**: Users can drag tabs left/right to reorder them within the tab strip.

**Complexity**: Low — frontend-only change, no C++ modifications needed for the basic case.

**How Chrome does it**: Chrome's `TabDragController` uses a 3px mouse-movement threshold before initiating drag (prevents accidental drags from clicks). A placeholder gap appears at the drop position, and other tabs animate sideways to make room.

**Implementation**:
- [ ] Add HTML5 drag event handlers to `TabComponent.tsx` (`onDragStart`, `onDragOver`, `onDrop`, `onDragEnd`)
- [ ] On drag start: record source tab index, apply `dragging` CSS class (reduce opacity to ~0.4)
- [ ] On drag over: calculate insertion point from mouse X position, show placeholder gap via CSS transform on neighboring tabs
- [ ] On drop: reorder the local `tabs` array in `useTabManager` state
- [ ] Send `tab_reorder` IPC to C++ with `{ sourceTabId, targetIndex }` so tab order persists in `TabManager::tabs_`
- [ ] Add `ReorderTab(int tab_id, int new_index)` to `TabManager` (C++ — reorder `tabs_` map to vector or use ordered container)
- [ ] Handle `tab_reorder` in `simple_handler.cpp` → calls `TabManager::ReorderTab()`
- [ ] Drag threshold: 3px mouse movement before starting (matches Chrome)

**UX details**:
- Dragged tab follows cursor horizontally, clamped to tab strip bounds
- Other tabs smoothly slide left/right (CSS transition on `transform: translateX()`)
- Close button hidden while dragging
- Tab strip scrolls if dragged to edge (if tab count exceeds visible width)

**Data model note**: `TabManager::tabs_` is currently `std::map<int, Tab>` (ordered by ID, not display order). Two options:
1. Add an `int display_order` field to `Tab` struct — cheapest change
2. Switch to `std::vector<Tab>` and use position as order — more natural but bigger refactor
Recommend option 1 for minimal impact.

### Phase 5: Tab Tear-Off to New Window (C++ Heavy)

**Goal**: Users can drag a tab out of the tab strip to create a new browser window with that tab.

**Complexity**: High — requires multi-window support within the same process, C++ window management, and header browser cloning.

#### Research Findings

**How Chrome does it**: Chrome's `TabDragController::Detach()` removes the `WebContents` object from the source window's `TabStripModel` and creates a new `Browser` instance (same process, same profile). The `WebContents` (renderer process, DOM, JS state) is **transferred intact** — no page reload, no state loss. This works because all windows share the same browser process.

**CEF-specific approach**: CEF doesn't expose Chrome's `WebContents` transfer API, but it DOES support **HWND reparenting**. Each Hodos tab already has its own `CefBrowser` with its own HWND. We can use Win32 `SetParent()` to move a tab's HWND to a new parent window without destroying the browser. The renderer process, page state, cookies, and JS context all survive.

**Why single-process is mandatory**: CEF enforces `SingletonLock` per `root_cache_path`. Launching a second `HodosBrowserShell.exe` would hit the lock file AND our `profile.lock`. All windows MUST be within the same process, sharing the same `CefRequestContext`.

**Profile continuity is automatic**: Since all profile systems are singletons within the browser process (`SettingsManager`, `AdblockCache`, `FingerprintProtection`, `EphemeralCookieManager`, `SessionManager`), a new window in the same process inherits everything. The wallet backend (port 3301) and adblock engine (port 3302) are independent processes that serve any window.

**Dev vs Production**: No difference. Both environments run a single `HodosBrowserShell.exe` process. Tear-off creates a new HWND, not a new process. No installer changes needed.

#### Implementation Plan

**Architecture change — WindowManager**:
```
WindowManager (new singleton)
  ├── Window 1 (original g_hwnd) ─── tabs: [1, 2, 3]
  ├── Window 2 (tear-off)        ─── tabs: [4]
  └── Window 3 (tear-off)        ─── tabs: [5, 6]
```

Currently `TabManager` is a singleton that assumes one window. Multi-window requires one of:
- **Option A**: `WindowManager` singleton that owns multiple `TabManager` instances (clean but big refactor)
- **Option B**: Keep `TabManager` singleton, add `window_id` field to `Tab` struct (minimal change)

Recommend **Option B** initially — add `int window_id` to `Tab`, default 0 for main window, auto-increment for tear-offs.

**Steps**:
- [ ] Add `int window_id` to `Tab` struct (default 0 = main window)
- [ ] Detect tear-off: in Phase 4's drag handler, if mouse Y leaves tab strip by 15+ pixels (Chrome's threshold), send `tab_tearoff` IPC with `{ tabId, screenX, screenY }`
- [ ] Create new top-level HWND in C++ (similar to `g_hwnd` but with unique window class or reusing `CEFShellWindow`)
- [ ] Create header browser in new window (new CEF browser loading `http://127.0.0.1:5137/` with role `"header_N"`)
- [ ] Reparent tab HWND: `SetParent(tab.hwnd, new_window_hwnd)`, update `WS_CHILD` styles, resize
- [ ] Update `Tab.window_id` in `TabManager`
- [ ] Filter `get_tab_list` responses by window — each header browser only sees its own window's tabs
- [ ] Handle new window close: if last tab closed, destroy the window and header browser
- [ ] Handle drag-back: detect when tear-off tab is dragged over an existing window's tab strip, reparent back

**macOS considerations**:
- Use `NSWindow` instead of `CreateWindowEx`
- Use `[tabView removeFromSuperview]` + `[newContentView addSubview:tabView]` instead of `SetParent()`
- Each window needs its own `NSWindowDelegate`

#### Open Questions (Decide at Implementation Time)

1. Should tear-off windows have the full toolbar (back/forward/refresh/address/wallet/etc)? **Yes** — each window needs independent navigation.
2. Should tear-off windows support their own overlays (wallet, settings, downloads)? **Probably share** — overlays are positioned relative to the window that opens them.
3. Should tabs be draggable between existing windows (not just tear-off/drop-back)? **Nice to have** — Chrome supports this but it's an incremental addition.
4. What happens when the main window closes but tear-off windows are open? **Transfer ownership** — promote one tear-off to be the new "main" window, or close all.

### Phase 6: Polish & Customization (Optional/Future)

**Goal**: Let users customize the new tab page + polish tab interactions.

**Changes needed**:
- [ ] Remove individual tiles (click X)
- [ ] Add custom shortcut tiles
- [ ] Background image selection (upload or choose from presets)
- [ ] Toggle privacy stats on/off
- [ ] Toggle search bar on/off
- [ ] Tab drag animations and visual polish

---

## Architecture Considerations

**Most-visited data**: `HistoryManager` already tracks visit counts. Need a query like:
```sql
SELECT url, title, COUNT(*) as visits FROM history
GROUP BY url ORDER BY visits DESC LIMIT 8
```

**Favicon approach**:
- **Quick (Phase 1)**: Use Google's favicon API: `https://www.google.com/s2/favicons?domain=example.com&sz=64`
- **Better (Phase 2+)**: Cache favicons locally in profile directory
- **Privacy concern**: Google favicon API leaks visited sites to Google. For a privacy browser, local caching is preferred long-term.

**Search bar vs address bar**: The new tab search bar is separate from the toolbar address bar. Typing in either should work the same way. When the user starts typing in the new tab search bar, consider focusing the address bar instead (Chrome does this).

**Performance**: New tab page should load instantly. Pre-fetch most-visited data and cache it.

**Multi-window process model**: All windows run in the same process. Each window has:
- Its own top-level HWND
- Its own header browser (tab bar + toolbar)
- One or more tab browsers (reparented between windows during tear-off)
- Access to the same singletons (SettingsManager, AdblockCache, etc.)

**CEF HWND reparenting** (confirmed working in CEF forums):
```cpp
// Move tab's browser HWND to new parent window
HWND browser_hwnd = tab.browser->GetHost()->GetWindowHandle();
SetParent(browser_hwnd, new_window_hwnd);
// Resize to fill new window's content area
MoveWindow(browser_hwnd, 0, header_height, width, tab_height, TRUE);
tab.browser->GetHost()->WasResized();
```

---

## Test Checklist

### New Tab Page (Phase 1-3)
- [ ] Open new tab → Hodos new tab page appears with logo, search bar, tiles
- [ ] Click a tile → navigates to that site
- [ ] Type in search bar → searches with default engine (or navigates if URL)
- [ ] Most-visited tiles update as user browses
- [ ] Address bar shows `hodos://newtab`
- [ ] Tab title shows "New Tab"
- [ ] Privacy stats show accurate blocked count
- [ ] Homepage setting still controls browser launch separately
- [ ] Right-click tab → "Set as homepage" → setting updates
- [ ] Verify dark theme matches rest of browser

### Tab Drag-Reorder (Phase 4)
- [ ] Drag tab left → tab moves to new position
- [ ] Drag tab right → tab moves to new position
- [ ] Drag first tab to last position → works
- [ ] Drag last tab to first position → works
- [ ] Click without dragging → tab switches (no accidental reorder)
- [ ] Tab order persists after creating/closing other tabs
- [ ] Visual feedback: dragged tab semi-transparent, gap appears at drop position

### Tab Tear-Off (Phase 5)
- [ ] Drag tab downward out of tab strip → new window opens with that tab
- [ ] New window has full toolbar (back/forward/refresh/address bar)
- [ ] Page does NOT reload after tear-off (state preserved)
- [ ] Cookies/login state preserved in torn-off tab
- [ ] Profile button shows same profile in new window
- [ ] Wallet panel works from new window
- [ ] Ad blocking works in new window
- [ ] Close last tab in tear-off window → window closes
- [ ] Close main window with tear-off windows open → handled gracefully

---

## References

- [Chromium TabDragController](https://chromium.googlesource.com/chromium/src/+/2accab56152b3890473024585376ab231268c015/chrome/browser/ui/views/tabs/tab_drag_controller.cc) — drag thresholds, detach/attach model
- [Chromium Tab Strip Design (Mac)](https://www.chromium.org/developers/design-documents/tab-strip-mac/) — overlay window approach
- [Igalia: Fallback Tab Dragging for Wayland](https://blogs.igalia.com/max/fallback-tab-dragging/) — good overview of Chrome's drag mechanics
- [CEF Forum: HWND Reparenting](https://magpcss.org/ceforum/viewtopic.php?f=6&t=10290) — confirmed working
- [CEF Forum: Tab Management](https://www.magpcss.org/ceforum/viewtopic.php?f=10&t=16287) — "CEF supports multiple browsers but your application is responsible for UX"
- [CEF Forum: SingletonLock / Cache Sharing](https://magpcss.org/ceforum/viewtopic.php?t=19759) — why multi-instance is wrong

---

**Last Updated**: 2026-03-02
