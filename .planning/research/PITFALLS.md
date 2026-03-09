# Domain Pitfalls: macOS CEF Overlay Porting

**Domain:** Porting Windows CEF off-screen-rendered (OSR) overlay windows to macOS Cocoa
**Researched:** 2026-03-09
**Confidence:** HIGH (based on existing codebase analysis + CEF issue tracker + Apple documentation)

---

## Critical Pitfalls

Mistakes that cause crashes, broken input, or major rework.

---

### Pitfall 1: Borderless NSWindow Cannot Become Key Window (Keyboard Death)

**What goes wrong:** Overlays with text input (Omnibox, Profile Panel) silently refuse all keyboard events. The user clicks into a text field, types, and nothing happens. No crash, no error -- just dead input.

**Why it happens:** `NSWindowStyleMaskBorderless` windows return `NO` from `canBecomeKeyWindow` by default. Only the key window receives keyboard events on macOS. Unlike Windows where `SetFocus(hwnd)` works on any visible window regardless of style, macOS enforces this at the framework level.

**Consequences:** Any overlay with text input is completely non-functional. The existing codebase already discovered this -- see `WalletOverlayWindow` subclass at `cef_browser_shell_mac.mm:435-441` which overrides `canBecomeKeyWindow` to return `YES`.

**Prevention:**
- Every overlay that needs keyboard input MUST use a custom `NSWindow` subclass that overrides `canBecomeKeyWindow` to return `YES`.
- Overlays without keyboard needs (Menu, Cookie Panel, Download Panel) can use plain `NSWindow`.
- Create a reusable `KeyableOverlayWindow` subclass rather than copy-pasting `WalletOverlayWindow` for each overlay.

**Detection (warning signs):**
- Keyboard events never reach `keyDown:` in the overlay's NSView.
- `[overlayWindow isKeyWindow]` returns `NO` after `makeKeyAndOrderFront:`.
- No crash or error -- just silent failure.

**Phase:** Phase 2 (Overlay Ports) -- affects Omnibox (Task 4) and Profile Panel (Task 7). Must be addressed at overlay creation time. The pattern already exists in the wallet overlay; the risk is forgetting to apply it to new overlays.

---

### Pitfall 2: Child Window vs. Floating Window -- The Focus Trap

**What goes wrong:** An overlay is made a child window of `g_main_window` via `addChildWindow:ordered:`, then keyboard input stops working even with `canBecomeKeyWindow` overridden. Or the overlay is made a floating window (`NSFloatingWindowLevel`), then it floats above ALL applications, not just the browser.

**Why it happens:** macOS child windows (added via `addChildWindow:ordered:`) cannot become the key window. Apple's documentation states that a parent-child relationship means the child moves with the parent but does NOT allow the child to take key status. This is a fundamental Cocoa constraint with no workaround. The existing codebase already hit this: see the comment at `cef_browser_shell_mac.mm:1117-1120` where `addChildWindow` is explicitly commented out for the wallet overlay.

The alternative -- using `NSFloatingWindowLevel` -- makes the overlay float above other apps' windows, which is wrong for dropdown panels that should only appear within the browser.

**Consequences:** Either keyboard input is broken (child window) or the overlay appears above all apps (floating level). Both are showstopper UX bugs.

**Prevention:**
- For overlays WITH keyboard input (Wallet, Omnibox, Profile): Use `NSFloatingWindowLevel` with manual position syncing in `MainWindowDelegate::windowDidMove/windowDidResize`. Accept the float-above-all tradeoff during input, or use `NSNormalWindowLevel` and manage z-ordering manually by calling `[overlayWindow orderFront:nil]` after main window activation.
- For overlays WITHOUT keyboard input (Menu, Cookie, Download, Notification): Use `addChildWindow:ordered:` freely. They get automatic position tracking and z-ordering. This is the pattern used by the Settings overlay (line 1043).
- Document which pattern each overlay uses and WHY.

**Detection:**
- Typing fails in an overlay that uses `addChildWindow:`.
- An overlay with `NSFloatingWindowLevel` is visible when the user switches to another app.
- `[overlayWindow isKeyWindow]` returns `NO` despite `canBecomeKeyWindow` returning `YES`.

**Phase:** Phase 2 (all overlay tasks). The decision must be made PER OVERLAY at creation time. The existing code already demonstrates both patterns (Settings = child window, Wallet = floating window).

---

### Pitfall 3: Cocoa Y-Axis Flip -- Off-By-Header Positioning

**What goes wrong:** Overlay windows appear at the wrong vertical position. Common symptoms: overlay appears at the bottom of the screen instead of below the toolbar, or it appears partially off-screen, or it moves to the wrong position after window resize.

**Why it happens:** Cocoa's coordinate system has origin at bottom-left with Y increasing upward. Windows has origin at top-left with Y increasing downward. Every positioning calculation must flip the Y-axis: `cocoaY = mainWindowTop - desiredTopOffset - overlayHeight`. The existing code uses the formula `overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - 104` (line 1010) where `104` is the header offset in points.

The conversion is error-prone because:
1. `NSWindow frame` includes the title bar. `NSWindow contentView bounds` does not. Mixing them produces off-by-title-bar errors (~28px on standard resolution).
2. Screen coordinates vs. window-relative coordinates use different reference frames.
3. Multi-monitor setups: `[NSScreen mainScreen]` is NOT necessarily the screen with the menu bar. It is the screen containing the window with keyboard focus.

**Consequences:** Overlays appear in the wrong position, sometimes off-screen entirely. On multi-monitor setups, overlays may appear on the wrong screen.

**Prevention:**
- Always use `[g_main_window frame]` (screen coordinates) for overlay positioning, not content view bounds.
- Use a helper function that encapsulates the Y-flip: `CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - topOffsetFromMainWindowTop - overlayHeight`.
- The magic number `104` (header offset) MUST match the actual header height (currently 99px) plus any title bar. Hardcoding this is fragile -- compute it from `mainFrame.size.height - [[g_main_window contentView] bounds].size.height + headerHeight`.
- For notification overlay (top-right positioning), the formula is different: `overlayY = mainFrame.origin.y + mainFrame.size.height - notificationHeight - titleBarHeight`.
- Test on external monitors and with different window positions.

**Detection:**
- Overlay appears at window bottom instead of below toolbar.
- Overlay shifts vertically by ~28px (title bar height) from expected position.
- Overlay appears off-screen on multi-monitor setups.
- Moving the main window causes overlays to drift relative to their anchor icons.

**Phase:** Phase 2 (all overlay tasks). Build the positioning helper function during Task 3 (Menu Overlay, the simplest overlay) and reuse for all subsequent overlays.

---

### Pitfall 4: Retina/HiDPI Scale Factor Mismatch

**What goes wrong:** Overlay content renders at half the expected size (tiny, blurry), or mouse clicks hit the wrong targets (click point is offset from visual position), or the overlay renders at 1x on a 2x Retina display.

**Why it happens:** macOS Retina displays use a 2x backing scale factor. There are three different coordinate spaces that must stay in sync:
1. **NSWindow frame** -- in points (logical pixels). A 400pt window is 800 physical pixels on Retina.
2. **CEF view rect** (from `GetViewRect`) -- should be in points (logical pixels). CEF handles scaling internally.
3. **CEF paint buffer** (from `OnPaint`) -- in physical pixels. On Retina, a 400pt-wide view produces an 800px-wide buffer.

Mouse coordinates from NSView events are already in points. The existing `GetViewRect` returns `width_` and `height_` which are set from `mainFrame.size.width/height` (points). The `OnPaint` buffer dimensions will be 2x those values on Retina. The current `OnPaint` implementation creates a `CGImage` using the buffer's actual pixel dimensions, then sets it as `layer.contents`. CALayer handles the scaling correctly IF the layer's `contentsScale` matches the window's `backingScaleFactor`.

**Consequences:** Blurry rendering, misaligned click targets, or crashes from buffer size mismatches.

**Prevention:**
- Set `layer.contentsScale = [[view window] backingScaleFactor]` in the overlay view's `initWithFrame:` and update it in `viewDidChangeBackingProperties`.
- `GetViewRect` must return POINT dimensions, not pixel dimensions. CEF multiplies by the scale factor from `GetScreenInfo` to determine the paint buffer size.
- `GetScreenInfo` must return the correct `device_scale_factor` (currently done correctly at `my_overlay_render_handler.mm:336`).
- Mouse coordinates from NSView events are already in points -- do NOT multiply by scale factor before sending to CEF.
- When moving between displays with different scale factors (e.g., Retina MacBook to 1x external), call `browser->GetHost()->NotifyScreenInfoChanged()`.

**Detection:**
- Content appears at half-size or quarter-size on Retina displays.
- Mouse clicks are offset from visual targets by a factor of 2.
- Content re-renders at wrong scale after dragging window to a different monitor.
- `OnPaint` receives buffer dimensions that are 2x the `GetViewRect` dimensions (correct on Retina) but the rendering looks wrong.

**Phase:** Phase 1/Phase 2 -- the existing render handler already handles basic Retina via `GetScreenInfo`. But each new overlay must verify `contentsScale` is set on its CALayer. Add verification to the first overlay (Task 3) and carry forward.

---

### Pitfall 5: CefKeyEvent Field Mapping -- `windows_key_code` Required on macOS

**What goes wrong:** Keyboard input partially works -- some keys produce characters but others (Backspace, Enter, Tab, arrow keys, Cmd+A, Cmd+V) do nothing. Or SendKeyEvent crashes with "Check failed: in_keyboard_event_".

**Why it happens:** CEF's `CefKeyEvent` requires BOTH platform-specific AND Windows-equivalent fields to be populated, even on macOS:
- `native_key_code` -- macOS virtual key code (from `[event keyCode]`)
- `windows_key_code` -- Windows VK_* equivalent (CEF needs this for cross-platform key identification)
- `character` -- Unicode character
- `unmodified_character` -- Unicode character without modifier keys applied

The existing overlay code (e.g., `WalletOverlayView keyDown:` at line 383) sets `native_key_code` and `character` but does NOT set `windows_key_code` or `unmodified_character` correctly for all key types. For printable characters, this works by accident because CEF can infer the key from `character`. For non-printable keys (Backspace=0x08, Enter=0x0D, Escape=0x1B, Tab=0x09, arrows), the mapping is critical.

Additionally, the existing code does not set `unmodified_character` from `[event charactersIgnoringModifiers]`, which breaks Cmd+key shortcuts in the overlay's web content.

**Consequences:** Non-printable keys and keyboard shortcuts do not work in overlay text inputs. Possible crash on certain key combinations (CEF issue #3666).

**Prevention:**
- Create a shared helper function `NSEventToCefKeyEvent(NSEvent* event, CefKeyEvent& key_event, cef_key_event_type_t type)` that correctly maps:
  - `native_key_code = [event keyCode]`
  - `character = [[event characters] characterAtIndex:0]`
  - `unmodified_character = [[event charactersIgnoringModifiers] characterAtIndex:0]`
  - `windows_key_code` = mapped from macOS keyCode to VK_* constants (use CEF's own mapping table from `cefclient` sample)
  - `modifiers` = NSEventModifierFlags to CEF EVENTFLAG_* conversion
- Use this helper in ALL overlay views instead of duplicating conversion code.
- Handle `NSEventTypeFlagsChanged` events (modifier key press/release) -- the existing code does not forward these, which breaks Cmd+A select-all and Cmd+V paste.
- Reference: CEF's `cefclient/browser/osr_window_mac.mm` has the authoritative implementation.

**Detection:**
- Backspace does not delete characters in overlay text inputs.
- Enter does not submit forms.
- Cmd+A does not select all text.
- Cmd+V does not paste.
- Arrow keys do not move the cursor.

**Phase:** Phase 1 (build the helper function), then apply in Phase 2 for every overlay with keyboard input (Tasks 4, 7). The existing wallet/backup/settings overlays should also be updated to use the helper.

---

### Pitfall 6: NSWindow Close/Destroy Lifecycle Crash

**What goes wrong:** The app crashes when closing an overlay, typically with `EXC_BAD_ACCESS` or a zombie object exception. The crash may be intermittent -- works fine 9 out of 10 times, then crashes.

**Why it happens:** Multiple lifecycle hazards:
1. **releasedWhenClosed:** NSWindow defaults `releasedWhenClosed` to `YES`. When `[overlayWindow close]` is called, the window is deallocated. If any code later references the global pointer (`g_wallet_overlay_window`), it accesses freed memory. The existing code correctly sets `setReleasedWhenClosed:NO` (line 1039), but forgetting this on any new overlay causes crashes.
2. **CEF browser lifecycle:** The CEF browser inside the overlay must be closed (`CloseBrowser(false)`) BEFORE the NSWindow is closed. CEF's browser cleanup is asynchronous -- calling `[window close]` immediately after `CloseBrowser` can destroy the NSView while CEF is still rendering to it.
3. **Child window removal:** If the overlay is a child window, it must be removed from the parent (`[g_main_window removeChildWindow:overlay]`) before closing, otherwise the parent may reference a deallocated child.
4. **Existing race in `windowDidResignKey`:** The delegate at line 876-884 calls `CloseBrowser(false)` and then immediately `[g_wallet_overlay_window close]` and sets the pointer to nil. This is a race condition -- the browser close is asynchronous but the window destruction is synchronous.

**Consequences:** Intermittent crashes on overlay close, especially under fast open/close cycles. Use-after-free if the global pointer is not nil-checked.

**Prevention:**
- ALWAYS set `setReleasedWhenClosed:NO` on every overlay NSWindow.
- Close CEF browser first, then close NSWindow in the `OnBeforeClose` callback (not immediately after `CloseBrowser`).
- For keep-alive overlays (show/hide pattern), do NOT call `[window close]` -- call `[window orderOut:nil]` to hide without destroying.
- Set global pointer to nil AFTER close, not before.
- Add nil checks before every operation on overlay window globals.
- The show/hide pattern (hide = `orderOut`, show = `makeKeyAndOrderFront`) is safer than create/destroy for frequently toggled overlays.

**Detection:**
- Crash in `dealloc` or `EXC_BAD_ACCESS` when closing overlays.
- Crash happens intermittently, especially under fast open-close-open sequences.
- "Zombie object" in crash log.
- Visual artifacts (brief flash of stale content) when reopening a previously closed overlay.

**Phase:** Phase 2 (all overlay tasks). Establish the correct lifecycle pattern in Task 3 (Menu Overlay) and enforce for all subsequent overlays. Fix the existing `windowDidResignKey` race condition for the wallet overlay during wallet crash fix.

---

## Moderate Pitfalls

---

### Pitfall 7: NSEvent Local Monitor -- Missing Global Clicks

**What goes wrong:** Click-outside detection works when clicking within the browser app, but fails when the user clicks on another application (Finder, Terminal, etc.). The overlay stays visible when it should have closed.

**Why it happens:** `NSEvent addLocalMonitorForEventsMatchingMask:` only monitors events within the application's own event stream. Clicks on other applications are NOT delivered to local monitors. On Windows, `WH_MOUSE_LL` is a global hook that sees ALL mouse events system-wide.

**Prevention:**
- Use `NSEvent addLocalMonitorForEventsMatchingMask:` for clicks within the app (primary mechanism).
- ADDITIONALLY observe `NSApplicationDidResignActiveNotification` to catch when the user switches to another app. This covers the Alt+Tab / Cmd+Tab / click-on-Dock / click-on-other-app cases.
- The existing `windowDidResignKey` handler (line 870) already does this for the wallet overlay, but only for wallet. Extend this pattern to all dropdown overlays.
- Do NOT use `addGlobalMonitorForEventsMatchingMask:` as an alternative -- it cannot modify events, has security implications, and requires accessibility permissions.

**Detection:**
- Overlay stays visible after clicking on another application.
- Overlay stays visible after Cmd+Tab to another app.
- Overlay closes correctly when clicking within the browser but outside the overlay.

**Phase:** Phase 1, Task 2 (Click-Outside Detection System). Build both mechanisms from the start.

---

### Pitfall 8: Cmd+H (Hide Application) Conflicts with History Shortcut

**What goes wrong:** Pressing Cmd+H hides the entire application instead of opening the History panel. The user loses their browser window.

**Why it happens:** Cmd+H is the system-wide "Hide Application" shortcut on macOS, managed by the application's main menu. Unless the CEF key event handler intercepts it before the menu system processes it, the default behavior fires. Windows has no system-level Ctrl+H mapping, so this conflict does not exist there.

**Consequences:** The History shortcut is unusable, and the app unexpectedly hides. Users lose context.

**Prevention:**
- Option A: Remap History to Cmd+Y on macOS (Safari's convention). This is the simplest and most macOS-native approach.
- Option B: Override `performKeyEquivalent:` in the main window or a custom NSApplication subclass to intercept Cmd+H before the menu system.
- Option C: Remove or remap the "Hide" menu item's key equivalent in the application menu. This requires explicit menu bar construction.
- The Track B document (line 82) lists Cmd+H for History but does not flag this conflict.

**Detection:**
- Pressing Cmd+H hides the app instead of opening History.
- The keyboard shortcut handler in `simple_handler.cpp` never receives the Cmd+H event.

**Phase:** Phase 1, Task 1 (Keyboard Shortcuts). Must be decided before implementing the shortcut mappings.

---

### Pitfall 9: Missing Scroll Event Forwarding

**What goes wrong:** Scrolling in overlay content (e.g., scrolling through settings or download list) either does not work, scrolls in the wrong direction, or scrolls at the wrong speed.

**Why it happens:** macOS scroll events have inverted delta values compared to Windows (macOS "natural scrolling"). Additionally, macOS uses continuous scroll values (floating point `deltaY`) while Windows uses discrete `WHEEL_DELTA` units (120 per notch). The existing overlay views forward `mouseDown`, `mouseMoved`, `rightMouseDown` but do NOT implement `scrollWheel:` forwarding to CEF via `SendMouseWheelEvent`.

**Prevention:**
- Add `scrollWheel:` handler to every overlay NSView subclass.
- Convert `[event deltaY]` to CEF wheel delta: multiply by a factor (typically 40-120) to match expected scroll speed.
- Respect `[event hasPreciseScrollingDeltas]` for trackpad vs. mouse wheel distinction.
- Do NOT flip the delta sign -- macOS "natural scrolling" is already the user's preference and should be respected.

**Detection:**
- Cannot scroll in overlay content.
- Scrolling direction is reversed.
- Scrolling is extremely fast or extremely slow.

**Phase:** Phase 2 (all overlay tasks). None of the existing overlay views implement `scrollWheel:`, so every overlay that has scrollable content needs this.

---

### Pitfall 10: CALayer contentsScale Not Updated on Display Change

**What goes wrong:** Overlay renders crisply on the MacBook's Retina screen, but becomes blurry when the browser window is dragged to an external 1x display (or vice versa). The content stays at the wrong scale factor until the overlay is destroyed and recreated.

**Why it happens:** The `CALayer.contentsScale` property determines how the layer's contents bitmap is mapped to the screen. When a window moves to a display with a different backing scale factor, the layer's `contentsScale` must be updated. The `NSView` method `viewDidChangeBackingProperties` is called when this happens, but the existing overlay views do not override it.

**Prevention:**
- Override `viewDidChangeBackingProperties` in each overlay NSView subclass (or in a shared base class).
- In the override, update `self.layer.contentsScale = [[self window] backingScaleFactor]`.
- Also call `browser->GetHost()->NotifyScreenInfoChanged()` to tell CEF to re-render at the new scale factor.
- Call `browser->GetHost()->WasResized()` to force a re-render.

**Detection:**
- Content becomes blurry or tiny after moving window between displays.
- Mouse click targets are offset after display change.

**Phase:** Phase 2. Add to the first overlay implementation (Task 3) and include in all subsequent overlays.

---

### Pitfall 11: GetScreenPoint Y-Coordinate Bug (Existing)

**What goes wrong:** Context menus, dropdown selects, and tooltips in overlay content appear at the wrong vertical position -- often mirrored across the horizontal center of the window.

**Why it happens:** The current `GetScreenPoint` implementation in `my_overlay_render_handler.mm` (line 292-303) does a naive conversion:
```cpp
screenX = screenPoint.x + viewX;
screenY = screenPoint.y + viewY;
```
This is INCORRECT for Cocoa because `screenPoint.y` is the BOTTOM of the window (Cocoa origin), but `viewY` is measured from the TOP (CEF's coordinate system). The correct conversion should flip: `screenY = (windowFrame.origin.y + windowFrame.size.height) - viewY`.

**Consequences:** CEF popups (select dropdowns, context menus, tooltips) appear at wrong positions. This is an EXISTING BUG in the codebase.

**Prevention:**
- Fix `GetScreenPoint` to properly convert from CEF top-left coordinates to Cocoa bottom-left screen coordinates.
- Correct formula: `screenY = windowFrame.origin.y + windowFrame.size.height - viewY`.
- Test with HTML `<select>` dropdowns and right-click context menus in overlay content.

**Detection:**
- HTML `<select>` dropdowns appear at wrong vertical position.
- Right-click context menus appear mirrored vertically.
- Tooltips appear below content instead of above (or vice versa).

**Phase:** Phase 1 (fix before building new overlays, as it affects all of them).

---

## Minor Pitfalls

---

### Pitfall 12: NSEvent Monitor Memory Leak on Overlay Destroy

**What goes wrong:** Slow memory growth over time. Each time an overlay is created and destroyed, the event monitor is not properly removed.

**Prevention:**
- Always call `[NSEvent removeMonitor:monitorRef]` before setting the monitor reference to nil.
- Remove monitors in both the overlay hide function AND the overlay destroy function.
- The proposed `RemoveClickOutsideMonitor(id* monitorRef)` helper in the Track B doc handles this, but only if it is called from every code path that destroys an overlay.

**Phase:** Phase 1, Task 2.

---

### Pitfall 13: mouseMoved Events Not Delivered to Non-Key NSWindow

**What goes wrong:** Hover effects (button highlights, tooltips) do not work in overlays that are NOT the key window.

**Why it happens:** By default, macOS only delivers `mouseMoved:` events to the key window. Non-key windows receive `mouseEntered:` and `mouseExited:` but NOT `mouseMoved:`. To receive `mouseMoved:` without being key, the window must call `[self setAcceptsMouseMovedEvents:YES]`.

**Prevention:**
- Call `[overlayWindow setAcceptsMouseMovedEvents:YES]` when creating each overlay window.
- Consider adding tracking areas via `NSTrackingArea` for more reliable mouse tracking.

**Phase:** Phase 2 (all overlay tasks).

---

### Pitfall 14: Duplicate Overlay View Boilerplate

**What goes wrong:** Not a runtime bug, but a maintenance and consistency risk. The existing code has 5 near-identical NSView subclasses (SettingsOverlayView, WalletOverlayView, BackupOverlayView, BRC100AuthOverlayView, SettingsMenuOverlayView) each with copy-pasted mouse/keyboard forwarding code (~80 lines each). Adding 6 more overlays means 11 total copies of the same pattern.

**Why it happens:** Objective-C does not have C++ templates, so the pattern gets copy-pasted. Each copy diverges slightly (e.g., WalletOverlayView has `SetFocus(true)` on click but SettingsOverlayView does not).

**Prevention:**
- Create a single `GenericOverlayView` base class that takes a browser-getter block or callback.
- Each overlay view is either an instance of `GenericOverlayView` configured with its browser getter, or a thin subclass that only overrides what is different.
- This ensures that fixes (like the missing `scrollWheel:`, `windows_key_code`, or `viewDidChangeBackingProperties`) propagate to all overlays automatically.

**Phase:** Phase 1 or Phase 2 Task 3. Refactor before building 6 new overlays, not after.

---

### Pitfall 15: Overlay Shows Before CEF Browser Finishes Loading

**What goes wrong:** Overlay window is made visible immediately but CEF browser has not loaded the React route yet. User sees a blank or partially rendered panel for a fraction of a second.

**Prevention:** Consider deferring `makeKeyAndOrderFront:` until `OnLoadEnd` fires for the overlay browser. Or accept the brief blank state since CEF loads from localhost which is fast. For keep-alive overlays (show/hide pattern), this only matters on first show.

**Phase:** Phase 2 (polish, low priority).

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Task 1: Keyboard Shortcuts | Cmd+H hides app instead of opening History (Pitfall 8) | Remap History to Cmd+Y or intercept before menu system |
| Task 2: Click-Outside System | Local monitor misses clicks on other apps (Pitfall 7) | Combine with `NSApplicationDidResignActiveNotification` |
| Task 2: Click-Outside System | Monitor leak on destroy (Pitfall 12) | Reusable helper with cleanup in ALL destruction paths |
| Task 3: Menu Overlay | Y-axis positioning wrong (Pitfall 3) | Build/test the positioning helper here, reuse everywhere |
| Task 3: Menu Overlay | Hover effects broken (Pitfall 13) | Set `acceptsMouseMovedEvents:YES` |
| Task 4: Omnibox Overlay | Keyboard input dead (Pitfalls 1, 2, 5) | Must use `KeyableOverlayWindow` subclass + correct key event mapping + floating window (not child) |
| Task 5: Cookie Panel | Retina icon offset wrong (Pitfall 4) | Divide icon offset by `backingScaleFactor` if it comes from React in CSS pixels |
| Task 6: Download Panel | Scroll does not work in download list (Pitfall 9) | Add `scrollWheel:` forwarding to the overlay view |
| Task 7: Profile Panel | Cmd+V paste broken (Pitfall 5) | Must forward `NSEventTypeFlagsChanged` and set `unmodified_character` |
| Task 8: Notification | Wrong vertical position -- appears at bottom (Pitfall 3) | Top-right positioning needs different Y-flip formula |
| All overlays | Blurry on display change (Pitfall 10) | Override `viewDidChangeBackingProperties` in shared base view |
| All overlays | Crash on close (Pitfall 6) | Use `orderOut:` for hide, not `close`. Set `releasedWhenClosed:NO` |
| All overlays | Select dropdowns at wrong position (Pitfall 11) | Fix `GetScreenPoint` Y-flip in render handler |
| Pre-Phase 2 | Code duplication explosion (Pitfall 14) | Build `GenericOverlayView` base class before porting 6 overlays |

---

## Sources

- [CEF Issue #3666: SendKeyEvent crashing on macOS](https://github.com/chromiumembedded/cef/issues/3666) -- Confidence: HIGH
- [CEF Bitbucket Issue #2150: macOS OSR rendering on display scale change](https://bitbucket.org/chromiumembedded/cef/issues/2150/macos-osr-rendering-issue-when-moving-to) -- Confidence: HIGH
- [CEF Forum: CefKeyEvent cross-platform fields](https://magpcss.org/ceforum/viewtopic.php?f=6&t=13560) -- Confidence: MEDIUM
- [CEF Forum: Keyboard not working with OSR](https://magpcss.org/ceforum/viewtopic.php?f=6&t=16583) -- Confidence: MEDIUM
- [CEF Issue #3602: Shutdown crash from zombie NSWindow](https://github.com/chromiumembedded/cef/issues/3602) -- Confidence: HIGH
- [Apple Developer: addLocalMonitorForEventsMatchingMask:](https://developer.apple.com/documentation/appkit/nsevent/1534971-addlocalmonitorforeventsmatching) -- Confidence: HIGH
- [Apple Developer: Monitoring Events (local vs global)](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html) -- Confidence: HIGH
- [Apple Developer: canBecomeKeyWindow](https://developer.apple.com/documentation/appkit/nswindow/1419543-canbecomekeywindow) -- Confidence: HIGH
- [Apple Developer: Cocoa Coordinate Systems](https://developer.apple.com/library/archive/documentation/General/Devpedia-CocoaApp-MOSX/CoordinateSystem.html) -- Confidence: HIGH
- [Apple Developer: addChildWindow:ordered:](https://developer.apple.com/documentation/appkit/nswindow/1419152-addchildwindow) -- Confidence: HIGH
- [Noodlesoft: Understanding Flipped Coordinate Systems](https://www.noodlesoft.com/blog/2009/02/02/understanding-flipped-coordinate-systems/) -- Confidence: MEDIUM
- [wxWidgets Issue #12466: Window positioning on multi-monitor](https://github.com/wxWidgets/wxWidgets/issues/12466) -- Confidence: MEDIUM
- Existing codebase analysis: `cef_browser_shell_mac.mm` (1824 lines), `my_overlay_render_handler.mm` -- Confidence: HIGH (direct code inspection)
