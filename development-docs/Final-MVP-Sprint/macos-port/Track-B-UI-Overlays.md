# Track B: macOS Port -- UI & Overlays

**Owner:** Person B
**Branch:** `Ishaan` (pushed to `origin/Ishaan`)
**Master tracking issue:** [#31](https://github.com/BSVArchie/Hodos-Browser/issues/31)

---

## Goal

Port all keyboard shortcuts and overlay windows from Windows to macOS so that the browser has full single-window feature parity. This track has no dependency on Track A (HTTP/backend) for the foundation work and only a soft dependency (AdblockCache) for the Cookie Panel overlay.

**All work must be done on the `Ishaan` branch.** Before starting any task, verify you are on the correct branch with `git branch --show-current`. Do not commit directly to `main`.

---

## Key Safety Rules

These rules are non-negotiable for all work on this track:

1. **All changes use `#ifdef _WIN32` / `#elif defined(__APPLE__)` guards.** Never write bare Windows or macOS code without a platform conditional.
2. **Never modify code inside existing `#ifdef _WIN32` blocks.** Only add `#elif defined(__APPLE__)` blocks alongside them.
3. **Build on macOS after every issue.** Run `cmake --build build --config Release` in `cef-native/` and verify zero errors before committing.
4. **Verify Windows still builds** before merging to main (CI or second machine). A macOS change must never break the Windows build.
5. **Coordinate on shared files** with Track A to minimize merge conflicts:
   - `cef_browser_shell_mac.mm` -- Track B will add ~1200 lines (overlays). Add in clearly separated sections.
   - `simple_handler.cpp` -- Track B changes the keyboard shortcut section (~line 5700). Track A changes around line 5620. Different regions, low risk.
   - `CMakeLists.txt` -- Coordinate if either track adds new source files.

---

## Current State

The macOS shell (`cef_browser_shell_mac.mm`, 1824 lines) already has:
- Working **Settings overlay** (`CreateSettingsOverlayWithSeparateProcess`) -- full pattern reference
- Working **Wallet overlay** (`CreateWalletOverlayWithSeparateProcess`) -- includes keyboard forwarding
- **Menu overlay stubs** (lines 1813-1824) -- `Create/Show/HideMenuOverlay` log "not yet implemented"
- No stubs for Omnibox, Cookie Panel, Download Panel, Profile Panel, or Notification overlays

The Windows implementations live in `simple_app.cpp` (lines 997-2393) and provide the reference for each overlay's size, positioning, behavior, and IPC messages.

---

## Task Breakdown

### Phase 1 -- Foundation, No Dependencies

#### Task 1: Keyboard Shortcuts -- Ctrl to Cmd (#10)

**File:** `cef-native/src/handlers/simple_handler.cpp` (keyboard handler section, ~lines 5700-5850)

**Problem:** All 8 keyboard shortcut checks use `EVENTFLAG_CONTROL_DOWN`. On macOS, users expect Cmd (which CEF maps to `EVENTFLAG_COMMAND_DOWN`).

**Approach:**
- Define a platform macro at the top of the keyboard handler section:
  ```cpp
  #ifdef __APPLE__
      #define HODOS_MOD_FLAG EVENTFLAG_COMMAND_DOWN
  #else
      #define HODOS_MOD_FLAG EVENTFLAG_CONTROL_DOWN
  #endif
  ```
- Replace all 8 occurrences of `EVENTFLAG_CONTROL_DOWN` in the keyboard handler with `HODOS_MOD_FLAG`.
- Handle special cases:
  - **DevTools** (Ctrl+Shift+I -> Cmd+Option+I): needs `EVENTFLAG_ALT_DOWN` on macOS instead of `EVENTFLAG_SHIFT_DOWN`
  - **Quit** (Alt+F4 -> Cmd+Q): add new shortcut block for macOS
  - **Preferences** (Cmd+,): macOS-only, add under `#ifdef __APPLE__`

**Shortcuts to verify (12):**

| Action | Windows | macOS |
|--------|---------|-------|
| New Tab | Ctrl+T | Cmd+T |
| Close Tab | Ctrl+W | Cmd+W |
| Find | Ctrl+F | Cmd+F |
| Reload | Ctrl+R | Cmd+R |
| DevTools | Ctrl+Shift+I | Cmd+Option+I |
| Address bar | Ctrl+L | Cmd+L |
| Bookmark | Ctrl+D | Cmd+D |
| Print | Ctrl+P | Cmd+P |
| Downloads | Ctrl+J | Cmd+J |
| History | Ctrl+H | Cmd+H |
| Quit | Alt+F4 | Cmd+Q |
| Preferences | -- | Cmd+, |

**Acceptance criteria:**
- All shortcuts work with Cmd on macOS
- All shortcuts still work with Ctrl on Windows (unchanged code paths)
- DevTools uses Cmd+Option+I, not Cmd+Shift+I

---

#### Task 2: NSEvent Click-Outside Detection System (#11)

**File:** `cef-native/cef_browser_shell_mac.mm` -- new section near overlay globals (after line ~98)

**Problem:** Windows uses `WH_MOUSE_LL` global mouse hooks to detect clicks outside dropdown-style overlays and close them. macOS has no equivalent API. Need `NSEvent addLocalMonitorForEventsMatchingMask:`.

**Approach:**
- Create a reusable pattern for click-outside detection that all 6 dropdown overlays will use.
- Each overlay gets its own monitor ID stored in a global so it can be removed on cleanup.
- The monitor checks: is the overlay visible? Did the click land outside the overlay window? If both true, hide the overlay.

**Design:**
```objc
// Globals (one per overlay)
static id g_menu_event_monitor = nil;
static id g_omnibox_event_monitor = nil;
static id g_cookie_panel_event_monitor = nil;
static id g_download_panel_event_monitor = nil;
static id g_profile_panel_event_monitor = nil;
static id g_notification_event_monitor = nil;

// Reusable helper
void InstallClickOutsideMonitor(id* monitorRef, NSWindow* overlayWindow, void(*hideFunc)()) {
    if (*monitorRef) {
        [NSEvent removeMonitor:*monitorRef];
        *monitorRef = nil;
    }
    *monitorRef = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (overlayWindow && [overlayWindow isVisible] && ![overlayWindow isEqual:[event window]]) {
                hideFunc();
            }
            return event;
        }];
}

void RemoveClickOutsideMonitor(id* monitorRef) {
    if (*monitorRef) {
        [NSEvent removeMonitor:*monitorRef];
        *monitorRef = nil;
    }
}
```

**Safety considerations:**
- Monitor must be removed when overlay is destroyed (memory leak + crash prevention)
- Nil checks on overlay window before accessing
- Must run on the main thread (NSEvent monitors are main-thread only, which aligns with CEF UI thread)
- Test with existing Settings/Wallet overlays first if possible

**Acceptance criteria:**
- Helper functions compile and are callable from overlay Create/Show/Hide functions
- Monitor correctly detects clicks outside an overlay and triggers its hide function
- Monitor is cleanly removed when overlay is destroyed

**This task blocks all 6 overlay tasks below.**

---

### Phase 2 -- Overlay Ports

All overlays follow the same general pattern established by the existing `CreateSettingsOverlayWithSeparateProcess`. For each overlay:

1. Add a global `NSWindow*` variable
2. Implement `CreateXxxOverlay()` -- create NSWindow + OSR CEF browser loading the React route
3. Implement `ShowXxxOverlay()` -- position relative to toolbar icon, make visible
4. Implement `HideXxxOverlay()` -- hide window (keep-alive, do not destroy)
5. Wire up NSEvent click-outside monitor from Task 2
6. Replace any existing stubs

**Common patterns to follow:**
- NSWindow: `NSWindowStyleMaskBorderless`, `NSBackingStoreBuffered`, opaque=NO, clear background, child of `g_main_window`
- CEF: `SetAsWindowless`, framerate=30, transparent background, `javascript_access_clipboard = STATE_ENABLED`
- Positioning: Cocoa origin is **bottom-left** (Y-axis is flipped vs Windows). Account for this in all coordinate math.
- Retina: `NSWindow backingScaleFactor` affects pixel calculations for icon offsets

**Reference implementation:** `CreateSettingsOverlayWithSeparateProcess()` at `cef_browser_shell_mac.mm:986`

---

#### Task 3: Menu Overlay (#15)

**Depends on:** Task 2 (click-outside detection)
**Difficulty:** Low -- simplest overlay, no text input, no special state
**Stubs to replace:** Lines 1813-1824 in `cef_browser_shell_mac.mm`

| Property | Value |
|----------|-------|
| Size | 280x450px |
| Position | Anchored below toolbar, right-offset from three-dot icon |
| Route | `/menu` |
| IPC | `menu_panel_show` / `menu_panel_hide` (already sent by frontend) |
| Lifecycle | Keep-alive (create once, show/hide) |
| Keyboard | Not needed (no text input) |
| Click-outside | Yes, via NSEvent monitor |

**Windows reference:** `simple_app.cpp` `CreateMenuOverlay()` ~line 1957

**Acceptance criteria:**
- Three-dot menu opens as a dropdown positioned under the icon
- All menu items (New Tab, Find, Print, Zoom, Bookmark, Downloads, History, DevTools, Settings, Exit) are clickable
- Clicking outside the menu closes it
- Menu positions correctly on window resize/move

---

#### Task 4: Omnibox Overlay (#16)

**Depends on:** Task 2 (click-outside detection)
**Soft dependency on:** Track A's AdblockCache port (#12) -- omnibox may trigger suggestion fetches, but core functionality works without it
**Difficulty:** Medium -- requires keyboard event forwarding

| Property | Value |
|----------|-------|
| Size | Full toolbar width x 350px height |
| Position | Directly below address bar |
| Route | `/omnibox` |
| IPC | `omnibox_show` / `omnibox_hide` |
| Lifecycle | Keep-alive |
| Keyboard | Required -- user types in omnibox search/URL field |
| Click-outside | Yes |

**Keyboard forwarding is critical.** The Wallet overlay's keyboard handling in `cef_browser_shell_mac.mm` (the `WalletOverlayWindow` class) is the reference pattern. Forward `NSEventTypeKeyDown`, `NSEventTypeKeyUp`, `NSEventTypeFlagsChanged` to CEF via `SendKeyEvent`.

**Windows reference:** `simple_app.cpp` `CreateOmniboxOverlay()` ~line 1223 (97 lines) + `ShowOmniboxOverlay()` ~72 lines

**Acceptance criteria:**
- Omnibox appears when user clicks address bar or presses Cmd+L
- Typing in the omnibox works (characters appear, backspace works, Cmd+A selects all)
- Clicking a suggestion navigates to it
- Clicking outside closes the omnibox
- Omnibox spans full width of the toolbar area

---

#### Task 5: Cookie Panel / Privacy Shield Overlay (#17)

**Depends on:** Task 2 (click-outside detection)
**Soft dependency on:** Track A's AdblockCache port (#12) -- panel displays adblock toggle state. Can stub/default until AdblockCache is ported.
**Difficulty:** Low-Medium

| Property | Value |
|----------|-------|
| Size | 450x370px |
| Position | Anchored to shield icon (right-offset via `g_cookie_icon_right_offset`) |
| Route | `/cookie-panel` |
| IPC | `cookie_panel_show` / `cookie_panel_hide`, icon offset passed as parameter |
| Lifecycle | Keep-alive |
| Keyboard | Not needed |
| Click-outside | Yes |

**Note:** Icon offset calculation must account for Retina scaling (`backingScaleFactor`). Clamp overlay to main window bottom edge so it doesn't extend off-screen.

**Windows reference:** `simple_app.cpp` `CreateCookiePanelOverlay()` ~line 1441 (128 lines) + `ShowCookiePanelOverlay()` ~82 lines

**Acceptance criteria:**
- Privacy shield icon opens the panel as a dropdown
- Panel shows adblock/cookie blocking toggle state (may show defaults if AdblockCache not yet ported)
- Panel positions correctly relative to the shield icon
- Clicking outside closes it

---

#### Task 6: Download Panel Overlay (#18)

**Depends on:** Task 2 (click-outside detection)
**Difficulty:** Low -- download state tracking is already cross-platform via CEF's `CefDownloadHandler`

| Property | Value |
|----------|-------|
| Size | 380x400px |
| Position | Anchored to download icon (right-offset) |
| Route | `/downloads` |
| IPC | `download_panel_show` / `download_panel_hide` |
| Lifecycle | Keep-alive |
| Keyboard | Not needed |
| Click-outside | Yes |

**Note:** Download file paths must use macOS conventions (`~/Downloads/`). The download handler itself is cross-platform (CEF provides it), so only the overlay window needs porting.

**Windows reference:** `simple_app.cpp` `CreateDownloadPanelOverlay()` ~line 1698 (130 lines) + `ShowDownloadPanelOverlay()` ~83 lines. Also `cef_browser_shell.cpp` globals.

**Acceptance criteria:**
- Download icon opens the panel
- Active downloads show progress bars
- Completed downloads show open/show-in-folder actions
- Pause/resume/cancel work
- Clicking outside closes it

---

#### Task 7: Profile Panel Overlay (#19)

**Depends on:** Task 2 (click-outside detection)
**Difficulty:** Medium-High -- has text input for profile creation, requires keyboard event forwarding

| Property | Value |
|----------|-------|
| Size | 380x380px |
| Position | Anchored to profile icon |
| Route | `/profile-picker` |
| IPC | `profile_panel_show` / `profile_panel_hide` |
| Lifecycle | Keep-alive |
| Keyboard | Required -- text input for new profile name |
| Click-outside | Yes |

**Keyboard forwarding** is the hard part, same pattern as Omnibox (Task 4). Must also support clipboard paste (Cmd+V) via macOS Pasteboard API. Set `javascript_access_clipboard = STATE_ENABLED` and `javascript_dom_paste = STATE_ENABLED` in browser settings.

**Windows reference:** `simple_app.cpp` `CreateProfilePanelOverlay()` ~line 2189 (128 lines) + `ShowProfilePanelOverlay()` ~76 lines

**Acceptance criteria:**
- Profile icon opens the picker panel
- Existing profiles are listed and selectable
- Text input works for creating a new profile name (typing + paste)
- Clicking outside closes the panel

---

#### Task 8: Notification Overlay (#20)

**Depends on:** Task 2 (soft -- may not use click-outside at all)
**Difficulty:** Medium -- different lifecycle than other overlays

| Property | Value |
|----------|-------|
| Size | 400x200px |
| Position | Top-right of main window (NOT anchored to a toolbar icon) |
| Route | Notification route (receives type/domain/extraParams via JS injection) |
| IPC | Triggered by C++ when BRC-100 auth/payment/cert approval is needed |
| Lifecycle | Create once, reuse across domains. Updated via `window.showNotification()` JS call |
| Keyboard | Not needed |
| Click-outside | Not standard -- closes via JS callback (user approves/denies) |

**Key differences from other overlays:**
- Not triggered by a toolbar icon click -- triggered programmatically by C++ when a website requests BRC-100 authentication
- Position is top-right (not anchored to an icon), so coordinate math is different
- Close is via React callback, not click-outside. The user must approve or deny the request
- Must handle Cocoa Y-axis flip for top-right positioning

**Windows reference:** `simple_app.cpp` `CreateNotificationOverlay()` ~line 997 (138 lines)

**Acceptance criteria:**
- Notification appears top-right when a BRC-100 auth request arrives
- Shows correct domain, type, and parameters
- Approve/deny buttons work and send response back to C++
- Notification hides after user action
- Multiple notifications reuse the same overlay window

---

## Dependency Summary

```
Task 1: Keyboard Shortcuts (#10)    -- independent, start immediately
Task 2: Click-Outside System (#11)  -- independent, start immediately

Task 2 ──┬──> Task 3: Menu Overlay (#15)
          ├──> Task 4: Omnibox Overlay (#16)
          ├──> Task 5: Cookie Panel (#17)      [soft dep on Track A #12]
          ├──> Task 6: Download Panel (#18)
          ├──> Task 7: Profile Panel (#19)
          └──> Task 8: Notification (#20)      [may not need click-outside]
```

**Recommended execution order:**
1. Tasks 1 + 2 in parallel
2. Task 3 (Menu) first -- simplest overlay, validates the pattern
3. Task 6 (Downloads) -- simple, no keyboard needed
4. Task 5 (Cookie Panel) -- simple, but may need AdblockCache stub
5. Task 4 (Omnibox) -- needs keyboard forwarding, test thoroughly
6. Task 7 (Profile Panel) -- needs keyboard + clipboard, highest risk
7. Task 8 (Notification) -- unique lifecycle, do last

---

## Shared Files & Conflict Avoidance

| File | Track B Changes | Track A Changes | Risk |
|------|----------------|----------------|------|
| `cef_browser_shell_mac.mm` | ~1200 lines: 6 overlay Create/Show/Hide functions, click-outside monitors, globals | ~50 lines: process launch, singleton init | **Medium** -- add in clearly separated sections with header comments |
| `simple_handler.cpp` | Keyboard shortcut section (~lines 5700-5850) | HTTP interceptor changes (~line 5620) | **Low** -- different regions |
| `CMakeLists.txt` | Possibly none unless new `.mm` files | Possibly new source files | **Low** -- coordinate |

**Mitigation:** Use clear section headers (e.g. `// === TRACK B: Menu Overlay ===`) and keep changes in contiguous blocks. Both tracks work on the `Ishaan` branch, so communicate before pushing to avoid conflicts.

**All work must be committed and pushed to the `Ishaan` branch.** Do not commit directly to `main`.

---

## Testing Strategy

After each overlay is implemented:
1. **Build check:** `cmake --build build --config Release` (zero errors, zero warnings ideally)
2. **Visual check:** Overlay appears at correct position, correct size
3. **Interaction check:** Click-outside closes it, IPC messages work, content renders
4. **Keyboard check (Tasks 4, 7 only):** Typing works, Cmd+V paste works, Cmd+A select-all works
5. **Resize/move check:** Overlay repositions correctly when main window moves or resizes

After all overlays are done:
- Run minimal test basket (youtube.com, x.com, github.com) to verify nothing regressed
- Test all overlays in sequence (open menu, close, open downloads, close, etc.) to verify no cross-overlay state leakage
