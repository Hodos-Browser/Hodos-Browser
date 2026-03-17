// ============================================================================
// OverlayHelpers_mac.mm -- Shared overlay infrastructure for macOS
// ============================================================================
// Provides: click-outside detection, app focus-loss handling,
// GenericOverlayView (event forwarding), and screen edge clamping.
//
// Every dropdown/popup overlay reuses these helpers so that event
// handling, dismiss behavior, and positioning are consistent.

#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>

#include "include/cef_browser.h"
#include "include/internal/cef_types.h"
#include "OverlayHelpers_mac.h"

// Logging -- reuse the Logger singleton from cef_browser_shell_mac.mm
#include "include/core/Logger.h"
#define LOG_DEBUG(msg) Logger::Log(msg, 0, 0)
#define LOG_INFO(msg)  Logger::Log(msg, 1, 0)
#define LOG_WARNING(msg) Logger::Log(msg, 2, 0)
#define LOG_ERROR(msg) Logger::Log(msg, 3, 0)

// ============================================================================
// Extern overlay globals from cef_browser_shell_mac.mm
// ============================================================================
extern NSWindow* g_main_window;
extern NSWindow* g_settings_overlay_window;
extern NSWindow* g_settings_menu_overlay_window;
extern NSWindow* g_menu_overlay_window;
extern bool g_wallet_overlay_prevent_close;

// Forward declarations for proper overlay cleanup (close browser before window)
extern void HideMenuOverlay();

// ============================================================================
// INFRA-01: Click-Outside Detection
// ============================================================================
// Maps NSWindow* (wrapped in NSValue) -> NSArray of [localMonitor, globalMonitor].
// When a click lands outside the overlay window, the overlay is closed and
// the monitors are removed.

static NSMutableDictionary<NSValue*, NSArray*>* s_clickOutsideMonitors = nil;

static NSMutableDictionary<NSValue*, NSArray*>* GetMonitorDict() {
    if (!s_clickOutsideMonitors) {
        // No ARC in this target. Use an owned object so the static monitor
        // registry stays valid across event loop/autorelease pool drains.
        s_clickOutsideMonitors = [[NSMutableDictionary alloc] init];
    }
    return s_clickOutsideMonitors;
}

void RemoveClickOutsideMonitor(NSWindow* overlayWindow) {
    if (!overlayWindow) return;

    NSValue* key = [NSValue valueWithNonretainedObject:overlayWindow];
    NSArray* monitors = [GetMonitorDict() objectForKey:key];
    if (!monitors) return;

    // Remove local monitor
    if ([monitors count] > 0 && [monitors objectAtIndex:0] != [NSNull null]) {
        [NSEvent removeMonitor:[monitors objectAtIndex:0]];
    }
    // Remove global monitor
    if ([monitors count] > 1 && [monitors objectAtIndex:1] != [NSNull null]) {
        [NSEvent removeMonitor:[monitors objectAtIndex:1]];
    }

    [GetMonitorDict() removeObjectForKey:key];
    LOG_DEBUG("OverlayHelpers: Removed click-outside monitors");
}

void InstallClickOutsideMonitor(NSWindow* overlayWindow) {
    if (!overlayWindow) return;

    // Prevent double-install
    RemoveClickOutsideMonitor(overlayWindow);

    NSEventMask mask = NSEventMaskLeftMouseDown | NSEventMaskRightMouseDown |
                       NSEventMaskOtherMouseDown;

    // Store overlay pointer for use in blocks. No ARC -- use __unsafe_unretained
    // semantics (raw pointer). The overlay is always kept alive by the caller
    // and RemoveClickOutsideMonitor is called before the window is released.
    NSWindow* __unsafe_unretained unsafeOverlay = overlayWindow;

    // Local monitor: clicks inside our app, but check if they landed on the overlay
    id localMonitor = [NSEvent addLocalMonitorForEventsMatchingMask:mask
        handler:^NSEvent*(NSEvent* event) {
            NSWindow* overlay = unsafeOverlay;
            if (!overlay) return event;

            // If the click is in the overlay window itself, let it through
            NSWindow* clickedWindow = [event window];
            if (clickedWindow == overlay) {
                return event;
            }

            // Click landed on a different window -- close the overlay.
            // Return nil to swallow the click (prevents crash from event delivery
            // to underlying CEF browser while overlay is tearing down).
            // This matches standard browser UX: first click dismisses the menu,
            // second click interacts with the page.
            LOG_DEBUG("OverlayHelpers: Click-outside detected (local) -- closing overlay");

            // Use dispatch_async to defer close to after event processing completes.
            // HideMenuOverlay handles DetachView + detachBrowser + CloseBrowser safely.
            dispatch_async(dispatch_get_main_queue(), ^{
                if (overlay == g_menu_overlay_window) {
                    HideMenuOverlay();
                    return;
                }

                // Ignore stale callbacks after another code path has already
                // removed the monitor and torn down the overlay window.
                NSValue* liveKey = [NSValue valueWithNonretainedObject:overlay];
                if (![GetMonitorDict() objectForKey:liveKey]) {
                    return;
                }

                {
                    RemoveClickOutsideMonitor(overlay);
                    [overlay close];
                }
            });
            return nil;  // Swallow the click event
        }];

    // Global monitor: clicks outside the app entirely (another app)
    id globalMonitor = [NSEvent addGlobalMonitorForEventsMatchingMask:mask
        handler:^(NSEvent* event) {
            NSWindow* overlay = unsafeOverlay;
            if (!overlay) return;

            LOG_DEBUG("OverlayHelpers: Click-outside detected (global) -- closing overlay");

            dispatch_async(dispatch_get_main_queue(), ^{
                if (overlay == g_menu_overlay_window) {
                    HideMenuOverlay();
                    return;
                }

                NSValue* liveKey = [NSValue valueWithNonretainedObject:overlay];
                if (![GetMonitorDict() objectForKey:liveKey]) {
                    return;
                }

                {
                    RemoveClickOutsideMonitor(overlay);
                    [overlay close];
                }
            });
        }];

    NSValue* key = [NSValue valueWithNonretainedObject:overlayWindow];
    [GetMonitorDict() setObject:@[localMonitor ?: [NSNull null],
                                   globalMonitor ?: [NSNull null]]
                         forKey:key];

    LOG_DEBUG("OverlayHelpers: Installed click-outside monitors for overlay");
}

// ============================================================================
// INFRA-02: App Focus Loss Handling
// ============================================================================
// NSApplicationDidResignActiveNotification fires when the user Cmd+Tabs away
// or clicks another application. Close dropdown overlays but NOT the wallet
// (wallet is exempt, matching Windows WM_ACTIVATEAPP behavior).

static id s_focusLossObserver = nil;

void InstallAppFocusLossHandler() {
    if (s_focusLossObserver) {
        // Already installed
        return;
    }

    s_focusLossObserver = [[NSNotificationCenter defaultCenter]
        addObserverForName:NSApplicationDidResignActiveNotification
        object:nil
        queue:[NSOperationQueue mainQueue]
        usingBlock:^(NSNotification* note) {
            LOG_DEBUG("OverlayHelpers: App resigned active -- closing dropdown overlays");

            // Close settings menu overlay (dropdown)
            if (g_settings_menu_overlay_window && [g_settings_menu_overlay_window isVisible]) {
                LOG_INFO("OverlayHelpers: Closing settings menu overlay on app focus loss");
                RemoveClickOutsideMonitor(g_settings_menu_overlay_window);
                [g_settings_menu_overlay_window close];
                g_settings_menu_overlay_window = nullptr;
            }

            // Close menu overlay (three-dot dropdown)
            if (g_menu_overlay_window && [g_menu_overlay_window isVisible]) {
                LOG_INFO("OverlayHelpers: Closing menu overlay on app focus loss");
                HideMenuOverlay();
            }

            // Future dropdown overlays (cookie panel, downloads, omnibox, profile picker)
            // will be added here as they are ported.

            // NOTE: Wallet overlay is NOT closed here. It has the prevent-close
            // exemption (user may Cmd+Tab to copy a mnemonic phrase). This matches
            // the Windows WM_ACTIVATEAPP behavior where wallet is guarded by
            // g_wallet_overlay_prevent_close.

            // NOTE: Settings overlay is NOT closed here. It is a large panel,
            // not a dropdown, and stays open on focus loss (matches current behavior).
        }];

    LOG_INFO("OverlayHelpers: App focus-loss handler installed");
}

// ============================================================================
// INFRA-04: Screen Edge Clamping
// ============================================================================

NSRect ClampOverlayToScreen(NSRect proposedFrame) {
    NSRect screenFrame = [[NSScreen mainScreen] visibleFrame];

    CGFloat x = proposedFrame.origin.x;
    CGFloat y = proposedFrame.origin.y;
    CGFloat w = proposedFrame.size.width;
    CGFloat h = proposedFrame.size.height;

    CGFloat minX = NSMinX(screenFrame);
    CGFloat maxX = NSMaxX(screenFrame);
    CGFloat minY = NSMinY(screenFrame);
    CGFloat maxY = NSMaxY(screenFrame);

    // Clamp right edge
    if (x + w > maxX) x = maxX - w;
    // Clamp left edge
    if (x < minX) x = minX;
    // Clamp bottom edge (Cocoa: Y grows upward)
    if (y < minY) y = minY;
    // Clamp top edge
    if (y + h > maxY) y = maxY - h;

    return NSMakeRect(x, y, w, h);
}

// ============================================================================
// GenericOverlayWindow -- borderless window that can become key
// ============================================================================

@implementation GenericOverlayWindow
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

// ============================================================================
// INFRA-03: GenericOverlayView
// ============================================================================
// NSView subclass with full event forwarding to CefBrowser.
// Replaces per-overlay SettingsOverlayView, BackupOverlayView, etc.

@implementation GenericOverlayView {
    OverlayBrowserRef* _browserRef;
}

- (instancetype)initWithFrame:(NSRect)frame {
    self = [super initWithFrame:frame];
    if (self) {
        _browserRef = nullptr;
        _renderLayer = [CALayer layer];
        _renderLayer.opaque = NO;
        [self setLayer:_renderLayer];
        [self setWantsLayer:YES];

        // Tracking area for mouse moved/entered/exited (hover effects)
        _trackingArea = [[NSTrackingArea alloc]
            initWithRect:frame
            options:(NSTrackingMouseMoved | NSTrackingMouseEnteredAndExited |
                     NSTrackingActiveAlways | NSTrackingInVisibleRect)
            owner:self
            userInfo:nil];
        [self addTrackingArea:_trackingArea];
    }
    return self;
}

- (void)attachBrowser:(OverlayBrowserRef*)ref {
    _browserRef = ref;
}

- (void)detachBrowser {
    _browserRef = nullptr;
}

- (CefRefPtr<CefBrowser>)browser {
    if (_browserRef) {
        return _browserRef->browser;
    }
    return nullptr;
}

- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }
- (BOOL)acceptsFirstMouse:(NSEvent *)event { return YES; }
- (BOOL)isFlipped { return YES; }

// Guarantee this view receives ALL hits within its bounds
- (NSView *)hitTest:(NSPoint)point {
    NSPoint localPoint = [self convertPoint:point fromView:[self superview]];
    if (NSPointInRect(localPoint, [self bounds])) {
        return self;
    }
    return nil;
}

// ----- Coordinate conversion helper -----
// Since isFlipped returns YES, view coordinates already have top-left origin.
// No Y-flip needed.
- (CefMouseEvent)cefMouseEventFromEvent:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    // isFlipped=YES means the view uses top-left origin, matching CEF.
    mouse_event.y = (int)location.y;
    mouse_event.modifiers = 0;
    return mouse_event;
}

// ----- Modifier helper -----
- (int)cefModifiersFromEvent:(NSEvent *)event {
    int modifiers = 0;
    NSEventModifierFlags flags = [event modifierFlags];
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;
    return modifiers;
}

// ===== Mouse Events =====

- (void)mouseDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;
    CefMouseEvent me = [self cefMouseEventFromEvent:event];
    b->GetHost()->SetFocus(true);
    b->GetHost()->SendMouseClickEvent(me, MBT_LEFT, false, [event clickCount]);
}

- (void)mouseUp:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;
    CefMouseEvent me = [self cefMouseEventFromEvent:event];
    b->GetHost()->SendMouseClickEvent(me, MBT_LEFT, true, [event clickCount]);
}

- (void)rightMouseDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;
    CefMouseEvent me = [self cefMouseEventFromEvent:event];
    b->GetHost()->SendMouseClickEvent(me, MBT_RIGHT, false, [event clickCount]);
}

- (void)rightMouseUp:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;
    CefMouseEvent me = [self cefMouseEventFromEvent:event];
    b->GetHost()->SendMouseClickEvent(me, MBT_RIGHT, true, [event clickCount]);
}

- (void)mouseMoved:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;
    CefMouseEvent me = [self cefMouseEventFromEvent:event];
    b->GetHost()->SendMouseMoveEvent(me, false);
}

- (void)mouseDragged:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;
    CefMouseEvent me = [self cefMouseEventFromEvent:event];
    b->GetHost()->SendMouseMoveEvent(me, false);
}

- (void)mouseEntered:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;
    CefMouseEvent me = [self cefMouseEventFromEvent:event];
    b->GetHost()->SendMouseMoveEvent(me, false);
}

- (void)mouseExited:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;
    CefMouseEvent me = [self cefMouseEventFromEvent:event];
    b->GetHost()->SendMouseMoveEvent(me, true);  // true = mouse left the view
}

// ===== Scroll Events =====

- (void)scrollWheel:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;
    CefMouseEvent me = [self cefMouseEventFromEvent:event];
    int deltaX = (int)([event scrollingDeltaX] * 10);
    int deltaY = (int)([event scrollingDeltaY] * 10);
    b->GetHost()->SendMouseWheelEvent(me, deltaX, deltaY);
}

// ===== Keyboard Events =====

- (void)keyDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;

    NSString* chars = [event characters];
    int modifiers = [self cefModifiersFromEvent:event];

    // RAWKEYDOWN
    CefKeyEvent key_event;
    key_event.type = KEYEVENT_RAWKEYDOWN;
    key_event.native_key_code = [event keyCode];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }
    key_event.modifiers = modifiers;
    b->GetHost()->SendKeyEvent(key_event);

    // CHAR (critical for text input)
    if (chars.length > 0) {
        key_event.type = KEYEVENT_CHAR;
        key_event.character = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
        b->GetHost()->SendKeyEvent(key_event);
    }
}

- (void)keyUp:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;

    CefKeyEvent key_event;
    key_event.type = KEYEVENT_KEYUP;
    key_event.native_key_code = [event keyCode];

    NSString* chars = [event characters];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }

    key_event.modifiers = [self cefModifiersFromEvent:event];
    b->GetHost()->SendKeyEvent(key_event);
}

- (void)flagsChanged:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self browser];
    if (!b) return;

    CefKeyEvent key_event;
    key_event.native_key_code = [event keyCode];
    key_event.modifiers = [self cefModifiersFromEvent:event];

    // Determine if a modifier key was pressed or released
    NSEventModifierFlags flags = [event modifierFlags];
    unsigned short keyCode = [event keyCode];

    // Common modifier key codes on macOS
    bool isDown = false;
    switch (keyCode) {
        case 56: // Left Shift
        case 60: // Right Shift
            isDown = (flags & NSEventModifierFlagShift) != 0;
            break;
        case 59: // Left Control
        case 62: // Right Control
            isDown = (flags & NSEventModifierFlagControl) != 0;
            break;
        case 58: // Left Option
        case 61: // Right Option
            isDown = (flags & NSEventModifierFlagOption) != 0;
            break;
        case 55: // Left Command
        case 54: // Right Command
            isDown = (flags & NSEventModifierFlagCommand) != 0;
            break;
        default:
            isDown = true;
            break;
    }

    key_event.type = isDown ? KEYEVENT_RAWKEYDOWN : KEYEVENT_KEYUP;
    b->GetHost()->SendKeyEvent(key_event);
}

@end
