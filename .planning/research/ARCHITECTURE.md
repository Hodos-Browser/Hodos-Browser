# Architecture Patterns: macOS CEF Overlay Porting

**Domain:** macOS Cocoa overlay windows for CEF-based browser
**Researched:** 2026-03-09
**Confidence:** HIGH (based on existing working code in cef_browser_shell_mac.mm)

## Recommended Architecture

The macOS overlay system mirrors the Windows WS_POPUP overlay pattern using Cocoa NSWindow instances with borderless styling and off-screen rendered (OSR) CEF browsers. Each overlay is a separate NSWindow containing a custom NSView subclass that forwards input events to CEF.

### NSWindow / NSView / CEF Hierarchy

```
g_main_window (NSWindow, NSWindowStyleMaskTitled)
  |
  +-- contentView
  |     +-- g_header_view (NSView) -- CEF windowed browser, React toolbar
  |     +-- g_webview_view (NSView) -- CEF windowed browser, web content tabs
  |
  +-- child windows (addChildWindow:ordered:NSWindowAbove)
  |     +-- g_settings_overlay_window -- NSWindowStyleMaskBorderless, OSR CEF
  |     +-- g_menu_overlay_window -- NSWindowStyleMaskBorderless, OSR CEF
  |     +-- g_cookie_panel_overlay_window -- NSWindowStyleMaskBorderless, OSR CEF
  |     +-- g_download_panel_overlay_window -- NSWindowStyleMaskBorderless, OSR CEF
  |     +-- g_notification_overlay_window -- NSWindowStyleMaskBorderless, OSR CEF
  |
  +-- independent floating windows (NOT child windows)
        +-- g_wallet_overlay_window (WalletOverlayWindow) -- NSFloatingWindowLevel
        +-- g_omnibox_overlay_window (OmniboxOverlayWindow) -- NSFloatingWindowLevel
        +-- g_profile_panel_overlay_window (ProfileOverlayWindow) -- NSFloatingWindowLevel
```

### Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| `g_main_window` (NSWindow) | Top-level app window. Contains header and webview as child NSViews. Owns child overlay windows. | MainWindowDelegate receives move/resize, propagates to all overlays |
| `MainWindowDelegate` | NSWindowDelegate handling windowDidMove, windowDidResize, windowDidResignKey. Repositions all visible overlays when main window moves/resizes. | Reads overlay globals, calls setFrame:display: on each |
| `OverlayView` subclasses (NSView) | Custom NSView per overlay type. Hosts CALayer for CEF OSR rendering. Forwards mouse/keyboard events to CEF browser. | Receives NSEvent from Cocoa, translates to CefMouseEvent/CefKeyEvent, sends to CefBrowserHost |
| `OverlayWindow` subclasses (NSWindow) | Custom NSWindow subclasses for overlays needing keyboard input. Override `canBecomeKeyWindow` to return YES. | Cocoa window system; becomes key window to receive keyboard events |
| `MyOverlayRenderHandler` (CefRenderHandler) | Receives OnPaint callbacks from CEF with pixel buffers, updates CALayer contents for display. | CEF render pipeline -> CALayer on the overlay's NSView |
| `SimpleHandler` (CefClient) | Browser-process handler. Routes IPC messages from React. Each overlay gets its own instance with a role string ("settings", "wallet", etc.). | React frontend via CefProcessMessage, overlay Create/Show/Hide C++ functions |
| `NSEvent monitors` | Local event monitors for click-outside detection. Installed per dropdown overlay. | NSApplication event stream -> Hide function for specific overlay |

### Two Overlay Categories

There are exactly two categories of overlay, distinguished by whether they need keyboard input:

**Category A: No Keyboard (child windows)**
- Settings, Menu, Cookie Panel, Download Panel, Notification
- Use `[g_main_window addChildWindow:overlay ordered:NSWindowAbove]`
- Child windows move automatically with parent (but still need manual repositioning logic for resize)
- Cannot become key window (fine -- no text input needed)
- Click-outside via NSEvent local monitor

**Category B: Keyboard Required (independent floating windows)**
- Wallet, Omnibox, Profile Panel
- Must NOT be child windows -- child windows cannot become key windows on macOS
- Use `NSFloatingWindowLevel` to stay above main window
- Must manually sync position on main window move/resize via MainWindowDelegate
- Require custom NSWindow subclass with `canBecomeKeyWindow` returning YES
- Must call `[g_main_window resignKeyWindow]` before `[overlay makeKeyAndOrderFront:nil]`
- Must forward NSEventTypeKeyDown/KeyUp/FlagsChanged to CEF via SendKeyEvent

This is a critical architectural distinction discovered during the wallet overlay implementation. The existing code comments explicitly warn: "Do NOT make this a child window - child windows cannot become key windows and therefore cannot receive keyboard events."

## Data Flow

### IPC: React Frontend to C++ Overlay Lifecycle

```
React button click (e.g., three-dot menu icon)
  |
  v
window.cefMessage.send('menu_panel_show', [iconRightOffset])
  |
  v
CefProcessMessage (renderer process -> browser process)
  |
  v
SimpleHandler::OnProcessMessageReceived()
  |
  v
  +-- First call: CreateMenuOverlay() -- creates NSWindow + CEF browser
  +-- Subsequent: ShowMenuOverlay() -- repositions and shows existing window
```

### Keyboard Event Forwarding (Category B overlays only)

```
User types in overlay
  |
  v
NSApplication dispatches NSEvent to key window (overlay NSWindow)
  |
  v
NSWindow's firstResponder (OverlayView subclass) receives:
  - keyDown: / keyUp: / flagsChanged:
  |
  v
OverlayView translates NSEvent to CefKeyEvent:
  - [event keyCode] -> key_event.native_key_code
  - [event characters] -> key_event.character
  - NSEventModifierFlags -> EVENTFLAG_* modifiers
  - Sends KEYEVENT_RAWKEYDOWN, then KEYEVENT_CHAR, then KEYEVENT_KEYUP
  |
  v
CefBrowserHost::SendKeyEvent(key_event)
  |
  v
CEF renderer process receives keystroke -> React input field updates
```

Key translation for modifiers:
```
NSEventModifierFlagShift   -> EVENTFLAG_SHIFT_DOWN
NSEventModifierFlagControl -> EVENTFLAG_CONTROL_DOWN
NSEventModifierFlagOption  -> EVENTFLAG_ALT_DOWN
NSEventModifierFlagCommand -> EVENTFLAG_COMMAND_DOWN
```

### Mouse Event Forwarding (all OSR overlays)

```
User clicks in overlay
  |
  v
OverlayView mouseDown:/mouseUp:/mouseMoved:
  |
  v
Convert coordinates: Cocoa (bottom-left origin) to CEF (top-left origin)
  mouse_event.y = self.bounds.size.height - cocoa_y
  |
  v
CefBrowserHost::SendMouseClickEvent / SendMouseMoveEvent
```

### Click-Outside Detection (dropdown overlays)

```
NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
  |
  v
Handler block checks:
  1. Is overlay visible?
  2. Is event.window != overlay window?
  |
  v
If both true: call HideXxxOverlay()
  - [overlay orderOut:nil] (hides, does not destroy)
  - [NSEvent removeMonitor:monitor] (cleanup)
```

### Overlay Repositioning on Window Move/Resize

```
User drags/resizes main window
  |
  v
MainWindowDelegate windowDidMove: / windowDidResize:
  |
  v
For each visible overlay:
  +-- Dropdown overlays: recalculate position from main window frame + icon offset
  |   overlayX = mainFrame.x + mainFrame.width - iconRightOffset - panelWidth
  |   overlayY = mainFrame.y + mainFrame.height - panelHeight - headerHeight(104)
  |
  +-- Full-window overlays (wallet/backup): match main window frame exactly
  |   [overlay setFrame:mainFrame display:YES]
  |
  +-- Notify CEF browser of size change
      browser->GetHost()->WasResized()
```

## Patterns to Follow

### Pattern 1: Overlay Creation (no keyboard)

This is the standard pattern for overlays that do not need text input. Based on the working `CreateSettingsOverlayWithSeparateProcess()`.

```objc
// 1. Store icon offset for repositioning
static int g_xxx_icon_right_offset = 0;
NSWindow* g_xxx_overlay_window = nullptr;
static id g_xxx_event_monitor = nil;

void CreateXxxOverlay(int iconRightOffset) {
    g_xxx_icon_right_offset = iconRightOffset;
    NSRect mainFrame = [g_main_window frame];

    // 2. Destroy existing if present
    if (g_xxx_overlay_window) {
        RemoveClickOutsideMonitor(&g_xxx_event_monitor);
        [g_xxx_overlay_window close];
        g_xxx_overlay_window = nullptr;
    }

    // 3. Calculate position (Cocoa coordinates -- bottom-left origin)
    CGFloat panelWidth = XXX, panelHeight = YYY;
    CGFloat overlayX = mainFrame.origin.x + mainFrame.size.width
                       - iconRightOffset - panelWidth;
    CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height
                       - panelHeight - 104; // 104 = header offset

    // 4. Create borderless NSWindow
    g_xxx_overlay_window = [[NSWindow alloc]
        initWithContentRect:NSMakeRect(overlayX, overlayY, panelWidth, panelHeight)
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];
    [g_xxx_overlay_window setOpaque:NO];
    [g_xxx_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_xxx_overlay_window setLevel:NSNormalWindowLevel];
    [g_xxx_overlay_window setIgnoresMouseEvents:NO];
    [g_xxx_overlay_window setReleasedWhenClosed:NO];
    [g_xxx_overlay_window setHasShadow:NO];
    [g_main_window addChildWindow:g_xxx_overlay_window ordered:NSWindowAbove];

    // 5. Create custom NSView with event forwarding
    XxxOverlayView* contentView = [[XxxOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, panelWidth, panelHeight)];
    [g_xxx_overlay_window setContentView:contentView];

    // 6. Create OSR CEF browser
    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);
    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("xxx"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView,
                                   (int)panelWidth, (int)panelHeight);
    handler->SetRenderHandler(render_handler);

    CefBrowserHost::CreateBrowser(window_info, handler,
        "http://127.0.0.1:5137/xxx-route", settings,
        nullptr, CefRequestContext::GetGlobalContext());

    // 7. Show and install click-outside monitor
    [g_xxx_overlay_window makeKeyAndOrderFront:nil];
    InstallClickOutsideMonitor(&g_xxx_event_monitor,
                                g_xxx_overlay_window, HideXxxOverlay);
}
```

### Pattern 2: Overlay Creation (with keyboard)

For Omnibox, Profile Panel. Based on the working `CreateWalletOverlayWithSeparateProcess()`.

Key differences from Pattern 1:
- Custom NSWindow subclass with `canBecomeKeyWindow` returning YES
- NOT added as child window (independent floating window)
- NSFloatingWindowLevel instead of NSNormalWindowLevel
- Must resign main window key status before making overlay key
- Must set content view as first responder
- NSView subclass must implement keyDown:/keyUp:/flagsChanged: forwarding

```objc
@interface OmniboxOverlayWindow : NSWindow
@end
@implementation OmniboxOverlayWindow
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

// In CreateOmniboxOverlay:
// ... (same as Pattern 1 except):
g_omnibox_overlay_window = [[OmniboxOverlayWindow alloc] ...];
[g_omnibox_overlay_window setLevel:NSFloatingWindowLevel];
// Do NOT call: [g_main_window addChildWindow:...]
[g_main_window resignKeyWindow];
[g_omnibox_overlay_window makeKeyAndOrderFront:nil];
[g_omnibox_overlay_window makeFirstResponder:contentView];
```

### Pattern 3: NSView Event Forwarding

Every overlay needs a custom NSView subclass. The minimum for mouse-only overlays:

```objc
@interface XxxOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@end

@implementation XxxOverlayView
- (instancetype)initWithFrame:(NSRect)frame {
    self = [super initWithFrame:frame];
    if (self) {
        _renderLayer = [CALayer layer];
        _renderLayer.opaque = NO;
        [self setLayer:_renderLayer];
        [self setWantsLayer:YES];
    }
    return self;
}
- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }

- (void)mouseDown:(NSEvent *)event {
    NSPoint loc = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent me;
    me.x = loc.x;
    me.y = self.bounds.size.height - loc.y; // Flip Y
    me.modifiers = 0;
    CefRefPtr<CefBrowser> b = SimpleHandler::GetXxxBrowser();
    if (b) {
        b->GetHost()->SendMouseClickEvent(me, MBT_LEFT, false, 1);
        b->GetHost()->SendMouseClickEvent(me, MBT_LEFT, true, 1);
    }
}
// ... mouseMoved, rightMouseDown similarly
@end
```

For keyboard overlays, add keyDown:/keyUp:/flagsChanged: methods following the BackupOverlayView pattern already in the codebase (lines 511-543 of cef_browser_shell_mac.mm).

### Pattern 4: Click-Outside Detection Helper

Reusable for all 6 dropdown overlays. Implemented once, called by each Create/Show function.

```objc
void InstallClickOutsideMonitor(id* monitorRef, NSWindow* overlayWindow,
                                 void(*hideFunc)()) {
    if (*monitorRef) {
        [NSEvent removeMonitor:*monitorRef];
        *monitorRef = nil;
    }
    *monitorRef = [NSEvent addLocalMonitorForEventsMatchingMask:
        NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (overlayWindow && [overlayWindow isVisible]
                && ![overlayWindow isEqual:[event window]]) {
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

Important: `addLocalMonitorForEventsMatchingMask:` only captures events delivered to the application. It does not capture clicks in other applications. For app-level focus loss, use `windowDidResignKey:` in MainWindowDelegate (already implemented).

## Anti-Patterns to Avoid

### Anti-Pattern 1: Making keyboard overlays child windows
**What:** Using `[g_main_window addChildWindow:overlay ordered:NSWindowAbove]` for overlays that need text input.
**Why bad:** Child windows on macOS cannot become key windows. `canBecomeKeyWindow` returns NO by default, and even overriding it in a subclass does not work for child windows. Keyboard events will never reach the overlay.
**Instead:** Use independent floating windows with `NSFloatingWindowLevel` and manual position sync.

### Anti-Pattern 2: Using WS_POPUP / WH_MOUSE_LL patterns from Windows
**What:** Trying to port Windows mouse hooks or window styles directly.
**Why bad:** macOS has no WH_MOUSE_LL equivalent. NSEvent monitors serve a similar purpose but have different semantics (local vs global, main-thread only).
**Instead:** Use `NSEvent addLocalMonitorForEventsMatchingMask:` for click-outside detection.

### Anti-Pattern 3: Forgetting Cocoa Y-axis flip
**What:** Using Windows coordinate math (origin top-left) directly.
**Why bad:** Cocoa origin is bottom-left. An overlay positioned at Y=100 from top on Windows needs Y = mainFrame.height - 100 - overlayHeight on macOS.
**Instead:** Always calculate: `overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - headerOffset`

### Anti-Pattern 4: Forgetting WasResized() after setFrame:display:
**What:** Repositioning an overlay window without notifying CEF.
**Why bad:** CEF's render handler still thinks the browser is the old size. Content will clip or render incorrectly.
**Instead:** After every `setFrame:display:YES`, call `browser->GetHost()->WasResized()`.

### Anti-Pattern 5: Leaking NSEvent monitors
**What:** Creating event monitors in Show functions without removing them in Hide/Destroy.
**Why bad:** Leaked monitors accumulate, fire on stale window references, cause crashes.
**Instead:** Always pair InstallClickOutsideMonitor with RemoveClickOutsideMonitor in Hide/Destroy.

## Suggested Build Order

The build order is driven by two factors: (1) dependency graph and (2) incremental complexity to validate patterns early.

```
Phase 1 (Foundation) -- no overlay dependencies
  Task 1: Keyboard Shortcuts (Ctrl -> Cmd)
  Task 2: Click-Outside Detection Helper (InstallClickOutsideMonitor/RemoveClickOutsideMonitor)

Phase 2a (Simple overlays -- validates Pattern 1)
  Task 3: Menu Overlay        -- simplest, no keyboard, no state
  Task 6: Download Panel      -- simple, no keyboard, CEF download handler is cross-platform

Phase 2b (Medium overlays -- extends Pattern 1)
  Task 5: Cookie Panel        -- simple, but needs icon offset math + Retina scaling
  Task 8: Notification        -- different positioning (top-right, not icon-anchored), different close (React callback, not click-outside)

Phase 2c (Keyboard overlays -- validates Pattern 2)
  Task 4: Omnibox             -- keyboard forwarding, full-width positioning
  Task 7: Profile Panel       -- keyboard + clipboard (Cmd+V), highest complexity
```

### Rationale for this order:

1. **Tasks 1+2 are independent** and unblock everything else. Do them first, in parallel.
2. **Menu (Task 3) first among overlays** because it is the simplest overlay with no special requirements. It validates the entire creation pattern (NSWindow + OSR CEF + click-outside + IPC) with minimal risk.
3. **Download Panel (Task 6) second** because it is also simple and the download handler is already cross-platform. Quick win.
4. **Cookie Panel (Task 5) third** because it introduces Retina backingScaleFactor math for icon offset positioning. This math will be reused by all subsequent dropdown overlays.
5. **Notification (Task 8) before keyboard overlays** because it has a unique lifecycle (programmatic trigger, React-controlled close) but no keyboard input. Good to isolate its positioning pattern (top-right) before dealing with keyboard complexity.
6. **Omnibox (Task 4) before Profile** because it needs keyboard forwarding but not clipboard. Validates the Pattern 2 (floating window + keyboard) architecture.
7. **Profile Panel (Task 7) last** because it combines keyboard forwarding + clipboard paste + text input -- highest complexity, benefits from all prior pattern validation.

### Dependencies Between Overlays

```
Task 2 (Click-Outside) -----> All dropdown overlays (Tasks 3-7)
Task 3 (Menu) validates -----> Pattern 1 reused by Tasks 5, 6, 8
Task 4 (Omnibox) validates --> Pattern 2 reused by Task 7
Task 5 (Cookie) validates --> Retina icon offset math reused by Task 6
```

### Key Risk: Wallet Overlay Crash

The existing wallet overlay reportedly crashes. The Track B doc lists "Fix wallet crash before building new overlays" as the first key decision. The wallet overlay contains the reference keyboard forwarding pattern needed for Omnibox and Profile Panel. Fixing this crash should be Phase 0 work before or in parallel with Phase 1.

## MainWindowDelegate Integration

Every new overlay must be added to MainWindowDelegate in two places:

1. **windowDidMove:** -- reposition overlay if visible
2. **windowDidResize:** -- reposition overlay + call `browser->GetHost()->WasResized()`

For dropdown overlays (icon-anchored), the repositioning formula is:
```
overlayX = mainFrame.origin.x + mainFrame.size.width - iconRightOffset - panelWidth
overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - 104
```

For full-window overlays (wallet, backup):
```
[overlay setFrame:mainFrame display:YES]
```

For notification overlay (top-right):
```
overlayX = mainFrame.origin.x + mainFrame.size.width - panelWidth - margin
overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - 104
```

## Overlay Dimensions Reference (from Windows implementations)

| Overlay | Width | Height | Position Type | Keyboard |
|---------|-------|--------|---------------|----------|
| Menu | 280 | 450 | Icon-anchored dropdown | No |
| Omnibox | toolbar width | 350 | Full-width below address bar | Yes |
| Cookie Panel | 450 | 370 | Icon-anchored dropdown | No |
| Download Panel | 380 | 400 | Icon-anchored dropdown | No |
| Profile Panel | 380 | 380 | Icon-anchored dropdown | Yes |
| Notification | 400 | 200 | Top-right of window | No |
| Settings | 450 | 450 | Icon-anchored dropdown | No (existing) |
| Wallet | full window | full window | Full-window overlay | Yes (existing) |

## Sources

- `cef_browser_shell_mac.mm` -- existing working implementations (Settings overlay at line 986, Wallet overlay at line 1086, MainWindowDelegate at line 754)
- `simple_app.cpp` -- Windows reference implementations (lines 997-2393)
- `development-docs/Final-MVP-Sprint/macos-port/Track-B-UI-Overlays.md` -- task breakdown and dependency analysis
- Root `CLAUDE.md` -- overlay lifecycle documentation, CEF input patterns, close prevention patterns
- `cef-native/CLAUDE.md` -- HWND/process architecture, rendering modes, focus management
