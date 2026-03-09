# Technology Stack: macOS CEF Overlay Porting

**Project:** Hodos Browser -- macOS UI & Overlay Parity
**Researched:** 2026-03-09 (updated with verification)

## Recommended Stack

This is not a greenfield stack selection. The stack is fixed (CEF + Cocoa + React). This document specifies the exact Cocoa APIs, CEF settings, and patterns needed to port 6 Windows overlays to macOS.

### Core APIs (Cocoa/AppKit)

| API | Version | Purpose | Why | Confidence |
|-----|---------|---------|-----|------------|
| `NSWindow` (borderless) | macOS 10.15+ | Overlay container | `NSWindowStyleMaskBorderless` -- proven in Settings/Wallet overlays already in codebase. No title bar, no chrome. | HIGH |
| `NSView` (layer-backed) | macOS 10.15+ | CEF OSR rendering target | `setWantsLayer:YES` + `CALayer` receives BGRA pixel buffers from CEF `OnPaint`. Already working in `MyOverlayRenderHandler.mm`. | HIGH |
| `CALayer` | macOS 10.15+ | Compositing/display | Receives `CGImageRef` from CEF paint buffer. Must use `[CATransaction setDisableActions:YES]` to prevent animation artifacts. | HIGH |
| `NSEvent addLocalMonitorForEventsMatchingMask:` | macOS 10.6+ | Click-outside detection | Replaces Windows `WH_MOUSE_LL` hooks. Monitors mouse events within the app's own event stream. Main-thread only. Stable API since 10.6. | HIGH |
| `NSWindow addChildWindow:ordered:` | macOS 10.2+ | Overlay-parent binding (non-keyboard overlays) | Automatically moves child with parent. Use for overlays that do NOT need keyboard input (Menu, Cookie, Download). | HIGH |
| Custom `NSWindow` subclass | N/A | Keyboard-capable overlay | Overrides `canBecomeKeyWindow` to return YES. Required for any overlay with text input. Borderless NSWindows refuse key status by default -- this is documented Apple behavior. | HIGH |

### CEF Configuration (per overlay)

| Setting | Value | Purpose | Why | Confidence |
|---------|-------|---------|-----|------------|
| `SetAsWindowless()` | `(__bridge void*)contentView` | OSR mode | All overlays render off-screen to CALayer. CEF calls `OnPaint` with pixel buffer. Only option for transparent overlays. | HIGH |
| `windowless_frame_rate` | 30 (no keyboard) / 60 (with keyboard) | Render rate | 60fps needed for smooth text cursor rendering in input fields. 30fps fine for static content like menus. | HIGH |
| `background_color` | `CefColorSetARGB(0, 0, 0, 0)` | Transparent | Overlays float over content; background must be transparent. React handles its own backdrop. | HIGH |
| `javascript_access_clipboard` | `STATE_ENABLED` | Clipboard access | Required for Cmd+V paste in input-capable overlays (Omnibox, Profile Panel). | HIGH |
| `javascript_dom_paste` | `STATE_ENABLED` | DOM paste | Required for paste into HTML input fields via JavaScript. | HIGH |
| `SimpleHandler` role string | `"menu"`, `"omnibox"`, etc. | Browser identification | IPC routing uses role to dispatch messages to correct overlay browser. | HIGH |

### Rendering Pipeline

| Component | Technology | Purpose | Why | Confidence |
|-----------|-----------|---------|-----|------------|
| `MyOverlayRenderHandler` | C++ (CefRenderHandler) | Receives pixel data from CEF | Converts BGRA buffer to `CGImageRef`, sets as `CALayer.contents`. Already implemented and working in `my_overlay_render_handler.mm`. | HIGH |
| `CGImageCreate` | CoreGraphics | Image creation from raw pixels | `kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little` matches CEF's BGRA output format. | HIGH |
| `dispatch_async(main)` | GCD | Thread safety | `CALayer` is not thread-safe. OnPaint may be called from CEF threads. Must dispatch to main queue. | HIGH |

## Two Overlay Categories (Critical Architectural Decision)

Overlays split into two categories based on keyboard input requirements. This distinction was discovered during the Wallet overlay implementation and is explicitly documented in the codebase comment at line 1117 of `cef_browser_shell_mac.mm`.

### Category 1: Display-Only Overlays (Child Windows -- No Keyboard)

**Overlays:** Menu, Cookie Panel, Download Panel, Notification

**Use `addChildWindow:ordered:NSWindowAbove` to parent overlay to main window.**

```objc
NSWindow* overlay = [[NSWindow alloc]
    initWithContentRect:frame
    styleMask:NSWindowStyleMaskBorderless
    backing:NSBackingStoreBuffered
    defer:NO];

[overlay setOpaque:NO];
[overlay setBackgroundColor:[NSColor clearColor]];
[overlay setLevel:NSNormalWindowLevel];
[overlay setIgnoresMouseEvents:NO];
[overlay setReleasedWhenClosed:NO];
[overlay setHasShadow:NO];

// Child window: moves with parent, does NOT steal key status
[g_main_window addChildWindow:overlay ordered:NSWindowAbove];
```

**Why child window:** Automatically repositions when main window moves. Does not steal keyboard focus. Cannot become key window -- acceptable because these overlays have no text inputs.

**Why NOT NSFloatingWindowLevel for these:** `NSFloatingWindowLevel` keeps windows above ALL other windows, including other apps. That is wrong for a dropdown menu that should hide behind other apps. `addChildWindow` + `NSNormalWindowLevel` gives correct z-ordering behavior.

### Category 2: Keyboard-Input Overlays (Floating Windows -- Need Key Status)

**Overlays:** Omnibox, Profile Panel, Wallet (already implemented)

**Use custom NSWindow subclass. Do NOT use `addChildWindow`. Use `NSFloatingWindowLevel` with manual position sync.**

```objc
// Custom NSWindow subclass (one definition, reuse for all keyboard overlays)
@interface KeyboardOverlayWindow : NSWindow
@end
@implementation KeyboardOverlayWindow
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

// Creation
KeyboardOverlayWindow* overlay = [[KeyboardOverlayWindow alloc]
    initWithContentRect:frame
    styleMask:NSWindowStyleMaskBorderless
    backing:NSBackingStoreBuffered
    defer:NO];

[overlay setLevel:NSFloatingWindowLevel];
// Do NOT call addChildWindow -- child windows cannot become key

// After creation: transfer key status and set first responder
[g_main_window resignKeyWindow];
[overlay makeKeyAndOrderFront:nil];
[overlay makeFirstResponder:contentView];
```

**Why NOT child window:** Child windows of an NSWindow cannot become the key window on macOS. `keyDown:` events are never delivered to the child's content view. The codebase comment at line 1117 explicitly says: "Do NOT make this a child window - child windows cannot become key windows and therefore cannot receive keyboard events (input fields won't work)."

**Why `canBecomeMainWindow` returns NO:** Prevents the overlay from showing as a separate "main" window in Mission Control or taking the title bar activation state from the main browser window.

**Why `resignKeyWindow` before `makeKeyAndOrderFront:`:** Cocoa may not transfer key status if the current key window does not resign first. The Wallet overlay at line 1159 demonstrates this pattern.

**Tradeoff -- manual position sync required:** Since these are not child windows, they do not move automatically with the parent. Must add repositioning logic in `MainWindowDelegate`'s `windowDidMove:` and `windowDidResize:` delegates.

## Click-Outside Detection System

### API: `NSEvent addLocalMonitorForEventsMatchingMask:`

This is the standard Cocoa replacement for Windows' `WH_MOUSE_LL` global mouse hook.

| Aspect | Detail | Confidence |
|--------|--------|------------|
| **Scope** | App-local events only (not system-wide). Sufficient -- we only care about clicks within our app. | HIGH |
| **Thread** | Handler always called on main thread. Aligns with CEF UI thread requirement. | HIGH |
| **Lifecycle** | Must call `[NSEvent removeMonitor:]` on cleanup. Leaking monitors causes crashes from dangling references. | HIGH |
| **Return value** | Must return the event (or nil to consume it). Return event unchanged for click-outside. | HIGH |
| **Limitation** | Does NOT detect clicks in other applications. Must also observe `NSApplicationDidResignActiveNotification` to hide overlays when app loses focus. | HIGH |

**Reusable helper pattern:**

```objc
static id g_menu_event_monitor = nil;  // One global per overlay

void InstallClickOutsideMonitor(id* monitorRef, NSWindow* overlayWindow, void(*hideFunc)()) {
    if (*monitorRef) {
        [NSEvent removeMonitor:*monitorRef];
        *monitorRef = nil;
    }
    *monitorRef = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (overlayWindow && [overlayWindow isVisible]) {
                NSWindow* eventWindow = [event window];
                if (eventWindow != overlayWindow) {
                    hideFunc();
                }
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

**Why local, not global:** `addGlobalMonitorForEventsMatchingMask:` requires Accessibility permissions (user must grant access in System Preferences). Local monitors need no special permissions. Combined with `NSApplicationDidResignActiveNotification`, local monitors cover all cases.

## Keyboard Event Forwarding (for OSR Overlays)

CEF's off-screen rendering mode does not receive keyboard events natively. The NSView must capture `keyDown:`, `keyUp:`, and convert them to `CefKeyEvent` structs.

### Required NSView Methods

```objc
- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }

- (void)keyDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> browser = /* get overlay browser */;
    if (!browser) return;

    NSString* chars = [event characters];
    NSEventModifierFlags flags = [event modifierFlags];

    int modifiers = 0;
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;

    // 1. RAWKEYDOWN first (handles shortcuts, navigation)
    CefKeyEvent key_event;
    key_event.type = KEYEVENT_RAWKEYDOWN;
    key_event.native_key_code = [event keyCode];
    if (chars.length > 0) key_event.character = [chars characterAtIndex:0];
    key_event.modifiers = modifiers;
    browser->GetHost()->SendKeyEvent(key_event);

    // 2. Then CHAR event (handles character insertion into text fields)
    if (chars.length > 0) {
        key_event.type = KEYEVENT_CHAR;
        key_event.character = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
        browser->GetHost()->SendKeyEvent(key_event);
    }
}

- (void)keyUp:(NSEvent *)event {
    CefKeyEvent key_event;
    key_event.type = KEYEVENT_KEYUP;
    key_event.native_key_code = [event keyCode];
    // ... same modifier extraction ...
    browser->GetHost()->SendKeyEvent(key_event);
}
```

**Why RAWKEYDOWN + CHAR (both required):** RAWKEYDOWN triggers key-based handlers (shortcuts, tab navigation). CHAR triggers character insertion into text fields. Missing either one breaks input -- RAWKEYDOWN-only means typing does nothing; CHAR-only means shortcuts do not work.

**Confidence:** HIGH -- WalletOverlayView already implements this exact pattern (lines 364-428 of `cef_browser_shell_mac.mm`) and it works for text input.

### Known Issue: CEF SendKeyEvent Crash -- RESOLVED

CEF issue [#3666](https://github.com/chromiumembedded/cef/issues/3666) reported `SendKeyEvent` crashing on macOS with `FATAL:render_widget_host_view_mac.mm Check failed: in_keyboard_event_`.

**Status:** The CEF maintainer (magreenblatt) confirmed "Not seeing any crashes at M145 with `cefclient --off-screen-rendering-enabled` or OSR tests." The crash was caused by incorrect `CefKeyEvent` population (wrong native_key_code, not using the `CefScopedSendingEvent` wrapper). Hodos Browser uses CEF 136 (later than M145) and already has the correct `HodosBrowserApplication` subclass with `CefScopedSendingEvent`.

**Impact on port:** The Wallet overlay crash mentioned in PROJECT.md is likely NOT this CEF bug, but rather something specific to the overlay's initialization or lifecycle. Must debug independently.

**Confidence:** HIGH -- verified via GitHub issue tracker and CEF maintainer response.

## Coordinate System

### Cocoa Y-Axis Flip

```
Windows:  (0,0) = top-left,     Y increases downward
Cocoa:    (0,0) = bottom-left,  Y increases upward
```

**Positioning formula for dropdown-style overlays (anchored below toolbar):**

```objc
NSRect mainFrame = [g_main_window frame];
CGFloat headerHeight = 104;  // toolbar + tab bar height in points
CGFloat overlayX = mainFrame.origin.x + mainFrame.size.width - iconRightOffset - panelWidth;
CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - headerHeight;
```

**For notification overlay (top-right, not icon-anchored):**

```objc
CGFloat margin = 10;
CGFloat overlayX = mainFrame.origin.x + mainFrame.size.width - panelWidth - margin;
CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - headerHeight;
```

**Retina scaling:** Use `[g_main_window backingScaleFactor]` when converting pixel offsets from the React toolbar (which reports in CSS pixels) to Cocoa points. On Retina: 1 CSS pixel may equal 1 Cocoa point (if Vite/CEF reports logical pixels), but verify empirically.

**Confidence:** HIGH -- Settings overlay at line 1009 already uses this formula successfully.

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Overlay window type | `NSWindow` (borderless) | `NSPanel` | NSPanel adds automatic key/main behavior that may conflict with our explicit focus management. NSWindow with `canBecomeKeyWindow` override gives precise control. |
| Overlay window type | `NSWindow` (borderless) | `NSPopover` | NSPopover cannot host a CEF OSR browser. Would require complete architecture rewrite. |
| Click-outside detection | `addLocalMonitorForEventsMatchingMask:` | `addGlobalMonitorForEventsMatchingMask:` | Global monitor requires Accessibility permissions. Local monitor is sufficient since we only care about in-app clicks. |
| Rendering mode | OSR + CALayer | Windowed CEF (`SetAsChild`) | SetAsChild does not support transparent backgrounds. Overlays must be transparent to float over browser content. |
| Position sync (keyboard overlays) | Manual sync in `windowDidMove:/windowDidResize:` | `addChildWindow` | Child windows cannot become key. Keyboard input is impossible. Manual sync is a few lines per overlay. |
| Event forwarding | NSView `keyDown:`/`keyUp:` | `NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskKeyDown` | Event monitors receive events before the responder chain. Using `keyDown:` on the first responder view is the standard Cocoa pattern and matches CEF's cefclient reference implementation. |
| Overlay lifecycle | Keep-alive (create once, show/hide) | Create/destroy each time | CEF browser creation spawns a subprocess -- expensive (200-500ms). Keep-alive with `orderFront:`/`orderOut:` is near-instant. |

## What NOT To Do (and Why)

1. **Do NOT use `addChildWindow` for keyboard overlays.** Child windows cannot become key on macOS. Text input will silently fail. Use floating independent windows instead.

2. **Do NOT use `NSFloatingWindowLevel` for non-keyboard overlays.** Floating level keeps windows above ALL other apps. A menu dropdown should go behind other apps when the user switches. Use `NSNormalWindowLevel` + `addChildWindow` instead.

3. **Do NOT forget `[CATransaction setDisableActions:YES]` in OnPaint.** Without it, Core Animation implicitly animates every CALayer content change, causing visible ghosting/fade effects on every paint cycle.

4. **Do NOT send only RAWKEYDOWN without CHAR.** Both event types are required. RAWKEYDOWN alone means typing in text fields does nothing.

5. **Do NOT leak NSEvent monitors.** Every `addLocalMonitorForEventsMatchingMask:` must have a matching `removeMonitor:` in hide/destroy paths. Leaked monitors reference deallocated windows and cause EXC_BAD_ACCESS crashes.

6. **Do NOT override Cmd+H for History on macOS.** It is a system shortcut for "Hide Application." Chrome, Safari, and Firefox all respect this. Use Cmd+Y or the menu for History instead.

7. **Do NOT call `setFrame:display:` without `WasResized()`.** After repositioning an overlay window, CEF's render handler still thinks the browser is the old size. Content will clip or render incorrectly.

## Sources

- [Apple: addLocalMonitorForEvents(matching:handler:)](https://developer.apple.com/documentation/appkit/nsevent/1534971-addlocalmonitorforeventsmatching?preferredLanguage=occ) -- HIGH confidence
- [Apple: canBecomeKeyWindow](https://developer.apple.com/documentation/appkit/nswindow/1419543-canbecomekeywindow) -- HIGH confidence
- [Apple: addChildWindow(_:ordered:)](https://developer.apple.com/documentation/appkit/nswindow/1419152-addchildwindow) -- HIGH confidence
- [Apple: NSWindow](https://developer.apple.com/documentation/appkit/nswindow) -- HIGH confidence
- [Apple: Monitoring Events](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html) -- HIGH confidence
- [CEF Issue #3666: SendKeyEvent crash on macOS](https://github.com/chromiumembedded/cef/issues/3666) -- HIGH confidence, verified resolved in M145+
- [CEF Forum: Keyboard not working with OSR](https://magpcss.org/ceforum/viewtopic.php?f=6&t=16583) -- MEDIUM confidence
- [CEF Forum: Transparent overlay](https://www.magpcss.org/ceforum/viewtopic.php?f=6&t=19411) -- MEDIUM confidence
- [CocoaDev: KeyEventsInBorderlessWindow](https://cocoadev.github.io/KeyEventsInBorderlessWindow/) -- HIGH confidence
- [CocoaDev: BorderlessWindow](https://cocoadev.github.io/BorderlessWindow/) -- HIGH confidence
- [NSWindow child window and key status](https://cocoa-dev.apple.narkive.com/5N79IKHS/nswindow-child-window-and-key-status) -- HIGH confidence
- Existing codebase: `cef_browser_shell_mac.mm` (1824 lines, 5 working overlays) -- HIGH confidence
- Existing codebase: `my_overlay_render_handler.mm` (working OSR rendering pipeline) -- HIGH confidence
