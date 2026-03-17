#pragma once
#ifdef __APPLE__

#import <Cocoa/Cocoa.h>
#include "include/cef_browser.h"

// ============================================================================
// Shared Overlay Infrastructure for macOS
// ============================================================================
// All overlay types (dropdown menus, settings, etc.) use these helpers.
// This avoids duplicating click-outside detection, event forwarding,
// and focus-loss handling in every overlay.

// Click-outside detection (INFRA-01)
// Installs NSEvent local + global monitors that close the overlay
// when the user clicks anywhere outside it.
void InstallClickOutsideMonitor(NSWindow* overlayWindow);
void RemoveClickOutsideMonitor(NSWindow* overlayWindow);

// App focus loss (INFRA-02)
// Registers NSApplicationDidResignActiveNotification to close
// dropdown overlays when the user Cmd+Tabs away. Wallet overlay
// is exempt (matches Windows behavior). Call once at startup.
void InstallAppFocusLossHandler();

// Edge clamping (INFRA-04)
// Adjusts proposed overlay frame to stay within the visible screen area.
NSRect ClampOverlayToScreen(NSRect proposedFrame);

// C++ bridge for CefBrowser storage in ObjC views.
// ObjC cannot hold CefRefPtr directly, so we wrap it in a C++ struct
// and store a void* pointer in the ObjC view.
struct OverlayBrowserRef {
    CefRefPtr<CefBrowser> browser;
};

// GenericOverlayView (INFRA-03)
// Base NSView subclass with full mouse, keyboard, and scroll event
// forwarding to CEF. New overlays use this instead of duplicating
// event handling code in per-overlay view subclasses.
@interface GenericOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@property (nonatomic, strong) NSTrackingArea* trackingArea;
- (void)attachBrowser:(OverlayBrowserRef*)ref;
- (void)detachBrowser;
@end

// GenericOverlayWindow -- borderless NSWindow that can become key
// (required for keyboard input in OSR overlays).
@interface GenericOverlayWindow : NSWindow
@end

#endif // __APPLE__
