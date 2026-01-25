// macOS Implementation of HodosBrowser Shell
// Uses Cocoa/AppKit for window management and CEF for browser rendering

#import <Cocoa/Cocoa.h>
#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <QuartzCore/QuartzCore.h>
#import <mach-o/dyld.h>

#include "include/cef_application_mac.h"
#include "cef_app.h"
#include "cef_client.h"
#include "cef_browser.h"
#include "cef_command_line.h"
#include "cef_life_span_handler.h"
#include "wrapper/cef_helpers.h"
#include "include/cef_render_process_handler.h"
#include "include/cef_v8.h"
#include "include/cef_browser.h"
#include "include/internal/cef_types.h"
#include "include/handlers/simple_handler.h"
#include "include/handlers/simple_render_process_handler.h"
#include "include/handlers/simple_app.h"
#include "include/handlers/my_overlay_render_handler.h"
#include "include/wrapper/cef_library_loader.h"

#include <iostream>
#include <fstream>
#include <chrono>
#include <iomanip>
#include <sstream>
#include <algorithm>

// ============================================================================
// Forward Declarations
// ============================================================================
void ShutdownApplication();

// ============================================================================
// Custom NSApplication for CEF (REQUIRED on macOS)
// ============================================================================

@interface HodosBrowserApplication : NSApplication <CefAppProtocol> {
 @private
  BOOL handlingSendEvent_;
}
@end

@implementation HodosBrowserApplication

- (BOOL)isHandlingSendEvent {
  return handlingSendEvent_;
}

- (void)setHandlingSendEvent:(BOOL)handlingSendEvent {
  handlingSendEvent_ = handlingSendEvent;
}

- (void)sendEvent:(NSEvent*)event {
  CefScopedSendingEvent sendingEventScoper;
  [super sendEvent:event];
}

- (void)terminate:(id)sender {
  // Override to prevent immediate exit - let CEF shut down properly
  ShutdownApplication();
}

@end

// ============================================================================
// Shared Core Components
// ============================================================================
#include "include/core/Logger.h"
#include "include/core/TabManager.h"
#include "include/core/HistoryManager.h"

// ============================================================================
// Global Window References (macOS equivalents of Windows HWNDs)
// ============================================================================

NSWindow* g_main_window = nullptr;
NSView* g_header_view = nullptr;
NSView* g_webview_view = nullptr;

// Overlay windows (created on-demand)
NSWindow* g_settings_overlay_window = nullptr;
NSWindow* g_wallet_overlay_window = nullptr;
NSWindow* g_backup_overlay_window = nullptr;
NSWindow* g_brc100_auth_overlay_window = nullptr;
NSWindow* g_settings_menu_overlay_window = nullptr;
NSWindow* g_omnibox_overlay_window = nullptr;

// Convenience macros for easier logging
#define LOG_DEBUG(msg) Logger::Log(msg, 0, 0)
#define LOG_INFO(msg) Logger::Log(msg, 1, 0)
#define LOG_WARNING(msg) Logger::Log(msg, 2, 0)
#define LOG_ERROR(msg) Logger::Log(msg, 3, 0)

// Legacy function for backward compatibility
void DebugLog(const std::string& message) {
    LOG_INFO(message);
}

// ============================================================================
// Forward Declarations
// ============================================================================

void ToggleWalletPanel();  // C++ callable function
void CreateMainWindow();
void CreateSettingsOverlayWithSeparateProcess();
void CreateWalletOverlayWithSeparateProcess();
void CreateBackupOverlayWithSeparateProcess();
void CreateBRC100AuthOverlayWithSeparateProcess();
void CreateSettingsMenuOverlay();
void CreateOmniboxOverlay();
void ShutdownApplication();

// ============================================================================
// Helper Functions (C++ callable from simple_app.cpp)
// ============================================================================

ViewDimensions GetViewDimensions(void* nsview) {
    ViewDimensions dims = {0, 0};

    if (!nsview) {
        LOG_ERROR("GetViewDimensions: nsview is null");
        return dims;
    }

    NSView* view = (__bridge NSView*)nsview;
    NSRect bounds = [view bounds];

    dims.width = (int)bounds.size.width;
    dims.height = (int)bounds.size.height;

    LOG_DEBUG("GetViewDimensions: " + std::to_string(dims.width) + "x" + std::to_string(dims.height));

    return dims;
}

// ============================================================================
// NSView Subclasses for Overlay Event Handling
// ============================================================================

// Settings Overlay View
@interface SettingsOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@end

@implementation SettingsOverlayView

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
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;  // Flip Y coordinate
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> settings = SimpleHandler::GetSettingsBrowser();
    if (settings) {
        settings->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
        settings->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
        LOG_DEBUG("🖱️ Settings overlay: Left-click forwarded to CEF");
    }
}

- (void)rightMouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> settings = SimpleHandler::GetSettingsBrowser();
    if (settings) {
        settings->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
        settings->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
        LOG_DEBUG("🖱️ Settings overlay: Right-click forwarded to CEF");
    }
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> settings = SimpleHandler::GetSettingsBrowser();
    if (settings) {
        settings->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)keyDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> settings = SimpleHandler::GetSettingsBrowser();
    if (!settings) return;

    NSString* chars = [event characters];
    NSEventModifierFlags flags = [event modifierFlags];

    int modifiers = 0;
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;

    // Send RAWKEYDOWN event
    CefKeyEvent key_event;
    key_event.type = KEYEVENT_RAWKEYDOWN;
    key_event.native_key_code = [event keyCode];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }
    key_event.modifiers = modifiers;
    settings->GetHost()->SendKeyEvent(key_event);

    // Send CHAR event for character input (critical for typing)
    if (chars.length > 0) {
        key_event.type = KEYEVENT_CHAR;
        key_event.character = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
        settings->GetHost()->SendKeyEvent(key_event);
    }

    LOG_DEBUG("⌨️ Settings overlay: Key events forwarded to CEF");
}

- (void)keyUp:(NSEvent *)event {
    CefKeyEvent key_event;
    key_event.type = KEYEVENT_KEYUP;
    key_event.native_key_code = [event keyCode];

    NSString* chars = [event characters];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }

    int modifiers = 0;
    NSEventModifierFlags flags = [event modifierFlags];
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;
    key_event.modifiers = modifiers;

    CefRefPtr<CefBrowser> settings = SimpleHandler::GetSettingsBrowser();
    if (settings) {
        settings->GetHost()->SendKeyEvent(key_event);
    }
}

@end

// Wallet Overlay View
@interface WalletOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@end

@implementation WalletOverlayView

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
    NSLog(@"🔍 WalletOverlayView mouseDown called!");
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        // CRITICAL: Set focus when user clicks
        wallet->GetHost()->SetFocus(true);
        NSLog(@"🔍 CEF focus enabled on click");

        wallet->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
        wallet->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
        LOG_DEBUG("🖱️ Wallet overlay: Left-click forwarded to CEF");
    } else {
        NSLog(@"❌ Wallet browser not available!");
    }
}

- (void)rightMouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        wallet->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
        wallet->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
        LOG_DEBUG("🖱️ Wallet overlay: Right-click forwarded to CEF");
    }
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        wallet->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)keyDown:(NSEvent *)event {
    NSLog(@"🔍🔍🔍 WalletOverlayView keyDown called! keyCode: %d", (int)[event keyCode]);

    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (!wallet) {
        NSLog(@"❌ WalletOverlayView: Browser not available!");
        return;
    }

    NSString* chars = [event characters];
    NSLog(@"🔍 Key characters: '%@'", chars);
    NSEventModifierFlags flags = [event modifierFlags];

    int modifiers = 0;
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;

    // Send RAWKEYDOWN event
    CefKeyEvent key_event;
    key_event.type = KEYEVENT_RAWKEYDOWN;
    key_event.native_key_code = [event keyCode];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }
    key_event.modifiers = modifiers;
    wallet->GetHost()->SendKeyEvent(key_event);
    NSLog(@"🔍 Sent RAWKEYDOWN to CEF");

    // Send CHAR event for character input (critical for typing)
    if (chars.length > 0) {
        key_event.type = KEYEVENT_CHAR;
        key_event.character = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
        wallet->GetHost()->SendKeyEvent(key_event);
        NSLog(@"🔍 Sent CHAR to CEF: %c", (char)key_event.character);
    }

    LOG_DEBUG("⌨️ Wallet overlay: Key events forwarded to CEF");
}

- (void)keyUp:(NSEvent *)event {
    CefKeyEvent key_event;
    key_event.type = KEYEVENT_KEYUP;
    key_event.native_key_code = [event keyCode];

    NSString* chars = [event characters];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }

    int modifiers = 0;
    NSEventModifierFlags flags = [event modifierFlags];
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;
    key_event.modifiers = modifiers;

    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        wallet->GetHost()->SendKeyEvent(key_event);
    }
}

@end

// Custom overlay windows that can become key (required for keyboard input)
// Borderless NSWindows refuse to become key by default - must override canBecomeKeyWindow

@interface WalletOverlayWindow : NSWindow
@end

@implementation WalletOverlayWindow
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

@interface OmniboxOverlayWindow : NSWindow
@end

@implementation OmniboxOverlayWindow
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

// Omnibox Overlay View
@interface OmniboxOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@end

@implementation OmniboxOverlayView

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
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> omnibox = SimpleHandler::GetOmniboxOverlayBrowser();
    if (omnibox) {
        omnibox->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
        omnibox->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
        LOG_DEBUG("🖱️ Omnibox overlay: Left-click forwarded to CEF");
    }
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> omnibox = SimpleHandler::GetOmniboxOverlayBrowser();
    if (omnibox) {
        omnibox->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)keyDown:(NSEvent *)event {
    NSLog(@"🔍 OmniboxOverlayView keyDown called! keyCode: %d", (int)[event keyCode]);

    CefRefPtr<CefBrowser> omnibox = SimpleHandler::GetOmniboxOverlayBrowser();
    if (!omnibox) {
        NSLog(@"❌ OmniboxOverlayView: Browser not available!");
        return;
    }

    NSString* chars = [event characters];
    NSLog(@"🔍 Key characters: '%@'", chars);

    NSEventModifierFlags flags = [event modifierFlags];

    int modifiers = 0;
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;

    // Send RAWKEYDOWN event
    CefKeyEvent key_event;
    key_event.type = KEYEVENT_RAWKEYDOWN;
    key_event.native_key_code = [event keyCode];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }
    key_event.modifiers = modifiers;
    omnibox->GetHost()->SendKeyEvent(key_event);
    NSLog(@"🔍 Sent RAWKEYDOWN to CEF");

    // Send CHAR event for character input (critical for typing)
    if (chars.length > 0) {
        key_event.type = KEYEVENT_CHAR;
        key_event.character = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
        omnibox->GetHost()->SendKeyEvent(key_event);
        NSLog(@"🔍 Sent CHAR to CEF: %c", (char)key_event.character);
    }

    LOG_DEBUG("⌨️ Omnibox overlay: Key events forwarded to CEF");
}

- (void)keyUp:(NSEvent *)event {
    CefKeyEvent key_event;
    key_event.type = KEYEVENT_KEYUP;
    key_event.native_key_code = [event keyCode];

    NSString* chars = [event characters];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }

    int modifiers = 0;
    NSEventModifierFlags flags = [event modifierFlags];
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;
    key_event.modifiers = modifiers;

    CefRefPtr<CefBrowser> omnibox = SimpleHandler::GetOmniboxOverlayBrowser();
    if (omnibox) {
        omnibox->GetHost()->SendKeyEvent(key_event);
    }
}

@end

// Backup Overlay View
@interface BackupOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@end

@implementation BackupOverlayView

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
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> backup = SimpleHandler::GetBackupBrowser();
    if (backup) {
        backup->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
        backup->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
        LOG_DEBUG("🖱️ Backup overlay: Left-click forwarded to CEF");
    }
}

- (void)rightMouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> backup = SimpleHandler::GetBackupBrowser();
    if (backup) {
        backup->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
        backup->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
        LOG_DEBUG("🖱️ Backup overlay: Right-click forwarded to CEF");
    }
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> backup = SimpleHandler::GetBackupBrowser();
    if (backup) {
        backup->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)keyDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> backup = SimpleHandler::GetBackupBrowser();
    if (!backup) return;

    NSString* chars = [event characters];
    NSEventModifierFlags flags = [event modifierFlags];

    int modifiers = 0;
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;

    // Send RAWKEYDOWN event
    CefKeyEvent key_event;
    key_event.type = KEYEVENT_RAWKEYDOWN;
    key_event.native_key_code = [event keyCode];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }
    key_event.modifiers = modifiers;
    backup->GetHost()->SendKeyEvent(key_event);

    // Send CHAR event for character input (critical for typing)
    if (chars.length > 0) {
        key_event.type = KEYEVENT_CHAR;
        key_event.character = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
        backup->GetHost()->SendKeyEvent(key_event);
    }

    LOG_DEBUG("⌨️ Backup overlay: Key events forwarded to CEF");
}

- (void)keyUp:(NSEvent *)event {
    CefRefPtr<CefBrowser> backup = SimpleHandler::GetBackupBrowser();
    if (!backup) return;

    CefKeyEvent key_event;
    key_event.type = KEYEVENT_KEYUP;
    key_event.native_key_code = [event keyCode];

    NSString* chars = [event characters];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }

    NSEventModifierFlags flags = [event modifierFlags];
    int modifiers = 0;
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;
    key_event.modifiers = modifiers;

    backup->GetHost()->SendKeyEvent(key_event);
}

@end

// BRC-100 Auth Overlay View
@interface BRC100AuthOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@end

@implementation BRC100AuthOverlayView

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
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> auth = SimpleHandler::GetBRC100AuthBrowser();
    if (auth) {
        auth->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
        auth->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
        LOG_DEBUG("🖱️ BRC-100 auth overlay: Left-click forwarded to CEF");
    }
}

- (void)rightMouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> auth = SimpleHandler::GetBRC100AuthBrowser();
    if (auth) {
        auth->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
        auth->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
        LOG_DEBUG("🖱️ BRC-100 auth overlay: Right-click forwarded to CEF");
    }
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> auth = SimpleHandler::GetBRC100AuthBrowser();
    if (auth) {
        auth->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)keyDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> auth = SimpleHandler::GetBRC100AuthBrowser();
    if (!auth) return;

    NSString* chars = [event characters];
    NSEventModifierFlags flags = [event modifierFlags];

    int modifiers = 0;
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;

    // Send RAWKEYDOWN event
    CefKeyEvent key_event;
    key_event.type = KEYEVENT_RAWKEYDOWN;
    key_event.native_key_code = [event keyCode];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }
    key_event.modifiers = modifiers;
    auth->GetHost()->SendKeyEvent(key_event);

    // Send CHAR event for character input (critical for typing)
    if (chars.length > 0) {
        key_event.type = KEYEVENT_CHAR;
        key_event.character = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
        auth->GetHost()->SendKeyEvent(key_event);
    }

    LOG_DEBUG("⌨️ BRC-100 auth overlay: Key events forwarded to CEF");
}

- (void)keyUp:(NSEvent *)event {
    CefRefPtr<CefBrowser> auth = SimpleHandler::GetBRC100AuthBrowser();
    if (!auth) return;

    CefKeyEvent key_event;
    key_event.type = KEYEVENT_KEYUP;
    key_event.native_key_code = [event keyCode];

    NSString* chars = [event characters];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }

    NSEventModifierFlags flags = [event modifierFlags];
    int modifiers = 0;
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;
    key_event.modifiers = modifiers;

    auth->GetHost()->SendKeyEvent(key_event);
}

@end

// Settings Menu Overlay View (simplified - dropdown menu)
@interface SettingsMenuOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@end

@implementation SettingsMenuOverlayView

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

- (void)mouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> menu = SimpleHandler::GetSettingsMenuBrowser();
    if (menu) {
        menu->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
        menu->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
        LOG_DEBUG("🖱️ Settings menu overlay: Left-click forwarded to CEF");
    }
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> menu = SimpleHandler::GetSettingsMenuBrowser();
    if (menu) {
        menu->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

@end

// ============================================================================
// Main Window Delegate
// ============================================================================

@interface MainWindowDelegate : NSObject <NSWindowDelegate>
@end

@implementation MainWindowDelegate

- (void)windowDidMove:(NSNotification *)notification {
    LOG_DEBUG("🔄 Main window moved - synchronizing overlays");
    NSRect mainFrame = [g_main_window frame];

    // Move all visible overlays to match main window position
    if (g_settings_overlay_window && [g_settings_overlay_window isVisible]) {
        [g_settings_overlay_window setFrame:mainFrame display:YES];
        LOG_DEBUG("🔄 Settings overlay position synchronized");
    }

    if (g_wallet_overlay_window && [g_wallet_overlay_window isVisible]) {
        [g_wallet_overlay_window setFrame:mainFrame display:YES];
        LOG_DEBUG("🔄 Wallet overlay position synchronized");
    }

    if (g_backup_overlay_window && [g_backup_overlay_window isVisible]) {
        [g_backup_overlay_window setFrame:mainFrame display:YES];
        LOG_DEBUG("🔄 Backup overlay position synchronized");
    }

    if (g_brc100_auth_overlay_window && [g_brc100_auth_overlay_window isVisible]) {
        [g_brc100_auth_overlay_window setFrame:mainFrame display:YES];
        LOG_DEBUG("🔄 BRC-100 auth overlay position synchronized");
    }
}

- (void)windowDidResize:(NSNotification *)notification {
    LOG_DEBUG("🔄 Main window resized - updating layout");

    NSRect contentRect = [[g_main_window contentView] bounds];
    int headerHeight = 99;
    int walletPanelWidth = 250;    // Vertical panel width
    int walletPanelHeight = 200;   // Vertical panel height
    int webviewHeight = contentRect.size.height - headerHeight;  // Full height

    // Resize header view (fixed 80px at top)
    NSRect headerRect = NSMakeRect(0, contentRect.size.height - headerHeight,
                                   contentRect.size.width, headerHeight);
    [g_header_view setFrame:headerRect];

    // Resize webview (full height below header)
    NSRect webviewRect = NSMakeRect(0, 0, contentRect.size.width, webviewHeight);
    [g_webview_view setFrame:webviewRect];

    // Notify CEF browsers of resize
    CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
    if (header) {
        header->GetHost()->WasResized();
        LOG_DEBUG("🔄 Header browser notified of resize");
    }

    CefRefPtr<CefBrowser> webview = SimpleHandler::GetWebviewBrowser();
    if (webview) {
        webview->GetHost()->WasResized();
        LOG_DEBUG("🔄 Webview browser notified of resize");
    }

    // Resize and notify overlay windows
    NSRect mainFrame = [g_main_window frame];

    if (g_settings_overlay_window && [g_settings_overlay_window isVisible]) {
        [g_settings_overlay_window setFrame:mainFrame display:YES];
        CefRefPtr<CefBrowser> settings = SimpleHandler::GetSettingsBrowser();
        if (settings) settings->GetHost()->WasResized();
        LOG_DEBUG("🔄 Settings overlay resized and notified");
    }

    if (g_wallet_overlay_window && [g_wallet_overlay_window isVisible]) {
        [g_wallet_overlay_window setFrame:mainFrame display:YES];
        CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
        if (wallet) wallet->GetHost()->WasResized();
        LOG_DEBUG("🔄 Wallet overlay resized and notified");
    }

    if (g_backup_overlay_window && [g_backup_overlay_window isVisible]) {
        [g_backup_overlay_window setFrame:mainFrame display:YES];
        CefRefPtr<CefBrowser> backup = SimpleHandler::GetBackupBrowser();
        if (backup) backup->GetHost()->WasResized();
        LOG_DEBUG("🔄 Backup overlay resized and notified");
    }

    if (g_brc100_auth_overlay_window && [g_brc100_auth_overlay_window isVisible]) {
        [g_brc100_auth_overlay_window setFrame:mainFrame display:YES];
        CefRefPtr<CefBrowser> auth = SimpleHandler::GetBRC100AuthBrowser();
        if (auth) auth->GetHost()->WasResized();
        LOG_DEBUG("🔄 BRC-100 auth overlay resized and notified");
    }

    if (g_omnibox_overlay_window && [g_omnibox_overlay_window isVisible]) {
        // Omnibox overlay is only 300px tall, positioned at top
        int overlayHeight = 300;
        NSRect omniboxFrame = NSMakeRect(mainFrame.origin.x,
                                          mainFrame.origin.y + mainFrame.size.height - overlayHeight,
                                          mainFrame.size.width,
                                          overlayHeight);
        [g_omnibox_overlay_window setFrame:omniboxFrame display:YES];
        CefRefPtr<CefBrowser> omnibox = SimpleHandler::GetOmniboxOverlayBrowser();
        if (omnibox) omnibox->GetHost()->WasResized();
        LOG_DEBUG("🔄 Omnibox overlay resized and notified");
    }
}

- (BOOL)windowShouldClose:(NSWindow *)sender {
    // Safety check: Only shut down if window is actually being closed by user
    // Tab browser closes should NOT trigger window close
    LOG_INFO("⚠️ windowShouldClose called");

    // Check if this is a spurious close event (from tab removal)
    // If we still have tabs, don't shut down
    if (TabManager::GetInstance().GetTabCount() > 0) {
        LOG_WARNING("⚠️ windowShouldClose called but tabs still exist - ignoring");
        return NO;  // Don't close window
    }

    LOG_INFO("❌ Main window closing - shutting down application");
    ShutdownApplication();
    return YES;
}

- (void)windowWillClose:(NSNotification *)notification {
    LOG_INFO("❌ Main window will close");
}

- (void)windowDidResignKey:(NSNotification *)notification {
    // App is losing focus - close all overlays (matches Windows WM_ACTIVATEAPP behavior)
    LOG_DEBUG("📱 Main window resigned key - closing overlays if open");

    // Close wallet overlay
    if (g_wallet_overlay_window && [g_wallet_overlay_window isVisible]) {
        LOG_INFO("💰 Closing wallet overlay due to app focus loss");
        CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
        if (wallet_browser) {
            wallet_browser->GetHost()->CloseBrowser(false);
        }
        // No removeChildWindow needed - wallet overlay is not a child window
        [g_wallet_overlay_window close];
        g_wallet_overlay_window = nullptr;
    }

    // Close omnibox overlay
    if (g_omnibox_overlay_window && [g_omnibox_overlay_window isVisible]) {
        LOG_INFO("🔍 Closing omnibox overlay due to app focus loss");
        [g_omnibox_overlay_window orderOut:nil];
    }

    // Note: Settings and other overlays can remain open when app loses focus
    // Only wallet overlay auto-closes for security (matches Windows behavior)
}

@end

// ============================================================================
// Wallet Panel Toggle - REMOVED (now uses overlay approach like Windows)
// ============================================================================
// Note: Wallet panel now uses CreateWalletOverlayWithSeparateProcess()
// instead of embedded view to maintain parity with Windows implementation

// ============================================================================
// Helper function for closing overlay windows from C++ code
// ============================================================================

extern "C" void CloseOverlayWindow(void* window, void* parent) {
    if (!window) {
        LOG_WARNING("CloseOverlayWindow: window is null");
        return;
    }

    NSWindow* overlayWindow = (__bridge NSWindow*)window;
    NSWindow* parentWindow = (__bridge NSWindow*)parent;

    // Remove from parent (if parent exists)
    if (parentWindow) {
        [parentWindow removeChildWindow:overlayWindow];
    }

    // Close the window
    [overlayWindow close];

    LOG_INFO("✅ Overlay window closed successfully");
}

extern "C" void HideOmniboxOverlay() {
    if (g_omnibox_overlay_window) {
        [g_omnibox_overlay_window orderOut:nil];
        LOG_INFO("✅ Omnibox overlay hidden");
    }
}

extern "C" bool IsOmniboxOverlayVisible() {
    return g_omnibox_overlay_window && [g_omnibox_overlay_window isVisible];
}

// ============================================================================
// Main Window Creation
// ============================================================================

void CreateMainWindow() {
    LOG_INFO("🪟 Creating main browser window (macOS)");

    // Get screen dimensions (work area, excluding menu bar and dock)
    NSRect screenRect = [[NSScreen mainScreen] visibleFrame];
    LOG_INFO("📐 Screen dimensions: " + std::to_string((int)screenRect.size.width) + " x " + std::to_string((int)screenRect.size.height));

    // Create main window
    g_main_window = [[NSWindow alloc]
        initWithContentRect:screenRect
        styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                  NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_main_window) {
        LOG_ERROR("❌ Failed to create main window");
        return;
    }

    [g_main_window setTitle:@"Hodos Browser"];
    [g_main_window setDelegate:[[MainWindowDelegate alloc] init]];
    [g_main_window setReleasedWhenClosed:NO];  // We manage window lifecycle

    // Calculate heights
    int headerHeight = 99;         // Header with tabs/address bar
    int webviewHeight = screenRect.size.height - headerHeight;  // Full height below header

    LOG_INFO("📐 Header height: " + std::to_string(headerHeight) + "px");
    LOG_INFO("📐 Webview height: " + std::to_string(webviewHeight) + "px (full)");

    // Create header view (at very top)
    NSRect headerRect = NSMakeRect(0, screenRect.size.height - headerHeight,
                                   screenRect.size.width, headerHeight);
    g_header_view = [[NSView alloc] initWithFrame:headerRect];

    if (!g_header_view) {
        LOG_ERROR("❌ Failed to create header view");
        return;
    }

    [g_header_view setAutoresizingMask:NSViewWidthSizable | NSViewMinYMargin];
    [[g_main_window contentView] addSubview:g_header_view];
    LOG_INFO("✅ Header view created at Y=" + std::to_string((int)headerRect.origin.y));

    // Create webview/content area (full height below header)
    NSRect webviewRect = NSMakeRect(0, 0, screenRect.size.width, webviewHeight);
    g_webview_view = [[NSView alloc] initWithFrame:webviewRect];

    if (!g_webview_view) {
        LOG_ERROR("❌ Failed to create webview");
        return;
    }

    [g_webview_view setAutoresizingMask:NSViewWidthSizable | NSViewHeightSizable];
    [[g_main_window contentView] addSubview:g_webview_view];
    LOG_INFO("✅ Webview created (full height)");

    // Note: Wallet panel now uses overlay window approach (CreateWalletOverlayWithSeparateProcess)
    // instead of embedded view to maintain parity with Windows implementation

    // Show window
    [g_main_window makeKeyAndOrderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];

    LOG_INFO("✅ Main window created successfully");
}

// ============================================================================
// Overlay Window Creation Functions
// ============================================================================

void CreateSettingsOverlayWithSeparateProcess() {
    LOG_INFO("🎨 Creating settings overlay with separate process (macOS)");

    // Get main window frame for overlay alignment
    NSRect mainFrame = [g_main_window frame];
    LOG_INFO("📐 Overlay dimensions: " + std::to_string((int)mainFrame.size.width) + " x " + std::to_string((int)mainFrame.size.height));

    // Destroy existing overlay if present
    if (g_settings_overlay_window) {
        LOG_INFO("🔄 Destroying existing settings overlay");
        [g_settings_overlay_window close];
        g_settings_overlay_window = nullptr;
    }

    // Create borderless, transparent, floating window
    g_settings_overlay_window = [[NSWindow alloc]
        initWithContentRect:mainFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_settings_overlay_window) {
        LOG_ERROR("❌ Failed to create settings overlay window");
        return;
    }

    [g_settings_overlay_window setOpaque:NO];
    [g_settings_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_settings_overlay_window setLevel:NSNormalWindowLevel];  // Changed from NSFloatingWindowLevel
    [g_settings_overlay_window setIgnoresMouseEvents:NO];
    [g_settings_overlay_window setReleasedWhenClosed:NO];
    [g_settings_overlay_window setHasShadow:NO];

    // Make this a child window of the main window
    [g_main_window addChildWindow:g_settings_overlay_window ordered:NSWindowAbove];

    // Create custom view for event handling and rendering
    SettingsOverlayView* contentView = [[SettingsOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, mainFrame.size.width, mainFrame.size.height)];
    [g_settings_overlay_window setContentView:contentView];

    // Create CEF browser with windowless rendering
    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);  // Fully transparent
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("settings"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView,
                                   (int)mainFrame.size.width,
                                   (int)mainFrame.size.height);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        handler,
        "http://127.0.0.1:5137/settings",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("❌ Failed to create settings overlay CEF browser");
        return;
    }

    [g_settings_overlay_window makeKeyAndOrderFront:nil];
    LOG_INFO("✅ Settings overlay created successfully");
}

void CreateWalletOverlayWithSeparateProcess() {
    LOG_INFO("🎨 Creating wallet overlay with separate process (macOS)");

    NSRect mainFrame = [g_main_window frame];
    LOG_INFO("📐 Overlay dimensions: " + std::to_string((int)mainFrame.size.width) + " x " + std::to_string((int)mainFrame.size.height));

    if (g_wallet_overlay_window) {
        LOG_INFO("🔄 Destroying existing wallet overlay");
        [g_wallet_overlay_window close];
        g_wallet_overlay_window = nullptr;
    }

    g_wallet_overlay_window = [[WalletOverlayWindow alloc]
        initWithContentRect:mainFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_wallet_overlay_window) {
        LOG_ERROR("❌ Failed to create wallet overlay window");
        return;
    }

    [g_wallet_overlay_window setOpaque:NO];
    [g_wallet_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_wallet_overlay_window setLevel:NSFloatingWindowLevel];  // Must be floating to stay above main window (not a child)
    [g_wallet_overlay_window setIgnoresMouseEvents:NO];
    [g_wallet_overlay_window setReleasedWhenClosed:NO];
    [g_wallet_overlay_window setHasShadow:NO];

    // CRITICAL: Do NOT make this a child window - child windows cannot become key windows
    // and therefore cannot receive keyboard events (input fields won't work)
    // Window position sync is handled in MainWindowDelegate::windowDidMove/windowDidResize
    // [g_main_window addChildWindow:g_wallet_overlay_window ordered:NSWindowAbove];

    WalletOverlayView* contentView = [[WalletOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, mainFrame.size.width, mainFrame.size.height)];
    [g_wallet_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 60;  // Increased from 30 to 60fps for smoother text input
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("wallet"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView,
                                   (int)mainFrame.size.width,
                                   (int)mainFrame.size.height);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        handler,
        "http://127.0.0.1:5137/wallet-panel",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("❌ Failed to create wallet overlay CEF browser");
        return;
    }

    // CRITICAL: Resign main window first so overlay can become key
    [g_main_window resignKeyWindow];

    [g_wallet_overlay_window makeKeyAndOrderFront:nil];

    // Make content view first responder for keyboard events
    [g_wallet_overlay_window makeFirstResponder:contentView];

    NSLog(@"🔍 Wallet overlay is key window: %d", [g_wallet_overlay_window isKeyWindow]);
    NSLog(@"🔍 First responder: %@", [g_wallet_overlay_window firstResponder]);

    LOG_INFO("✅ Wallet overlay created successfully");
}

void CreateBackupOverlayWithSeparateProcess() {
    LOG_INFO("🎨 Creating backup overlay with separate process (macOS)");

    NSRect mainFrame = [g_main_window frame];

    if (g_backup_overlay_window) {
        LOG_INFO("🔄 Destroying existing backup overlay");
        [g_backup_overlay_window close];
        g_backup_overlay_window = nullptr;
    }

    g_backup_overlay_window = [[NSWindow alloc]
        initWithContentRect:mainFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_backup_overlay_window) {
        LOG_ERROR("❌ Failed to create backup overlay window");
        return;
    }

    [g_backup_overlay_window setOpaque:NO];
    [g_backup_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_backup_overlay_window setLevel:NSNormalWindowLevel];  // Changed from NSFloatingWindowLevel
    [g_backup_overlay_window setIgnoresMouseEvents:NO];
    [g_backup_overlay_window setReleasedWhenClosed:NO];
    [g_backup_overlay_window setHasShadow:NO];

    // Make this a child window of the main window
    [g_main_window addChildWindow:g_backup_overlay_window ordered:NSWindowAbove];

    BackupOverlayView* contentView = [[BackupOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, mainFrame.size.width, mainFrame.size.height)];
    [g_backup_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("backup"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView,
                                   (int)mainFrame.size.width,
                                   (int)mainFrame.size.height);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        handler,
        "http://127.0.0.1:5137/backup",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("❌ Failed to create backup overlay CEF browser");
        return;
    }

    [g_backup_overlay_window makeKeyAndOrderFront:nil];
    LOG_INFO("✅ Backup overlay created successfully");
}

void CreateBRC100AuthOverlayWithSeparateProcess() {
    LOG_INFO("🎨 Creating BRC-100 auth overlay with separate process (macOS)");

    NSRect mainFrame = [g_main_window frame];

    if (g_brc100_auth_overlay_window) {
        LOG_INFO("🔄 Destroying existing BRC-100 auth overlay");
        [g_brc100_auth_overlay_window close];
        g_brc100_auth_overlay_window = nullptr;
    }

    g_brc100_auth_overlay_window = [[NSWindow alloc]
        initWithContentRect:mainFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_brc100_auth_overlay_window) {
        LOG_ERROR("❌ Failed to create BRC-100 auth overlay window");
        return;
    }

    [g_brc100_auth_overlay_window setOpaque:NO];
    [g_brc100_auth_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_brc100_auth_overlay_window setLevel:NSNormalWindowLevel];  // Changed from NSFloatingWindowLevel
    [g_brc100_auth_overlay_window setIgnoresMouseEvents:NO];
    [g_brc100_auth_overlay_window setReleasedWhenClosed:NO];
    [g_brc100_auth_overlay_window setHasShadow:NO];

    // Make this a child window of the main window
    [g_main_window addChildWindow:g_brc100_auth_overlay_window ordered:NSWindowAbove];

    BRC100AuthOverlayView* contentView = [[BRC100AuthOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, mainFrame.size.width, mainFrame.size.height)];
    [g_brc100_auth_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("brc100_auth"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView,
                                   (int)mainFrame.size.width,
                                   (int)mainFrame.size.height);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        handler,
        "http://127.0.0.1:5137/brc100-auth",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("❌ Failed to create BRC-100 auth overlay CEF browser");
        return;
    }

    [g_brc100_auth_overlay_window makeKeyAndOrderFront:nil];
    LOG_INFO("✅ BRC-100 auth overlay created successfully");
}

void CreateOmniboxOverlay() {
    LOG_INFO("🔍 Creating omnibox overlay (macOS)");

    // Check if overlay already exists
    if (g_omnibox_overlay_window) {
        LOG_WARNING("🔍 Omnibox overlay already exists, showing it");

        // CRITICAL: Resign main window as key so overlay can become key
        [g_main_window resignKeyWindow];

        [g_omnibox_overlay_window makeKeyAndOrderFront:nil];

        // Force it to become key
        [g_omnibox_overlay_window becomeKeyWindow];

        NSLog(@"🔍 Overlay is key window: %d", [g_omnibox_overlay_window isKeyWindow]);
        NSLog(@"🔍 Main window is key: %d", [g_main_window isKeyWindow]);

        // Make the content view first responder to receive keyboard events
        BOOL didBecome = [[g_omnibox_overlay_window contentView] becomeFirstResponder];
        NSLog(@"🔍 Content view becomeFirstResponder result: %d", didBecome);
        NSLog(@"🔍 First responder is now: %@", [g_omnibox_overlay_window firstResponder]);
        return;
    }

    // Get main window position and size
    NSRect mainFrame = [g_main_window frame];
    int width = (int)mainFrame.size.width;
    int overlayHeight = 300;  // 300px height for dropdown

    // Position at top of main window
    NSRect overlayFrame = NSMakeRect(mainFrame.origin.x,
                                      mainFrame.origin.y + mainFrame.size.height - overlayHeight,
                                      width,
                                      overlayHeight);

    g_omnibox_overlay_window = [[OmniboxOverlayWindow alloc]
        initWithContentRect:overlayFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_omnibox_overlay_window) {
        LOG_ERROR("❌ Failed to create omnibox overlay window");
        return;
    }

    [g_omnibox_overlay_window setOpaque:NO];
    [g_omnibox_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_omnibox_overlay_window setLevel:NSFloatingWindowLevel];  // Use floating for top-most behavior
    [g_omnibox_overlay_window setIgnoresMouseEvents:NO];
    [g_omnibox_overlay_window setReleasedWhenClosed:NO];
    [g_omnibox_overlay_window setHasShadow:NO];
    [g_omnibox_overlay_window setAcceptsMouseMovedEvents:YES];

    // Critical: Allow window to become key to receive keyboard events
    [g_omnibox_overlay_window setStyleMask:NSWindowStyleMaskBorderless];
    [g_omnibox_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorStationary];

    // DON'T make it a child window - child windows can't become key and receive keyboard events
    // Instead, position it as a separate floating window
    // [g_main_window addChildWindow:g_omnibox_overlay_window ordered:NSWindowAbove];

    OmniboxOverlayView* contentView = [[OmniboxOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, width, overlayHeight)];
    [g_omnibox_overlay_window setContentView:contentView];

    // Enable keyboard event handling
    [g_omnibox_overlay_window makeFirstResponder:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("omnibox_overlay"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView,
                                   width,
                                   overlayHeight);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        handler,
        "http://127.0.0.1:5137/omnibox-overlay",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("❌ Failed to create omnibox overlay CEF browser");
        return;
    }

    // CRITICAL: Resign main window as key so overlay can become key
    [g_main_window resignKeyWindow];

    [g_omnibox_overlay_window makeKeyAndOrderFront:nil];

    // Force it to become key
    [g_omnibox_overlay_window becomeKeyWindow];

    NSLog(@"🔍 Initial overlay is key window: %d", [g_omnibox_overlay_window isKeyWindow]);
    NSLog(@"🔍 Main window is key: %d", [g_main_window isKeyWindow]);

    // Make the content view first responder so it receives keyboard events
    BOOL didMake = [g_omnibox_overlay_window makeFirstResponder:contentView];
    NSLog(@"🔍 Initial makeFirstResponder result: %d", didMake);
    NSLog(@"🔍 First responder is now: %@", [g_omnibox_overlay_window firstResponder]);
    NSLog(@"🔍 Content view: %@", contentView);

    LOG_INFO("✅ Omnibox overlay created successfully");
}

void CreateSettingsMenuOverlay() {
    LOG_INFO("🎨 Creating settings menu overlay (macOS)");

    // Settings menu is smaller (dropdown from settings button)
    // Position in top-right corner
    NSRect mainFrame = [g_main_window frame];
    int menuWidth = 300;
    int menuHeight = 400;
    NSRect menuFrame = NSMakeRect(mainFrame.origin.x + mainFrame.size.width - menuWidth - 20,
                                  mainFrame.origin.y + mainFrame.size.height - menuHeight - 60,
                                  menuWidth, menuHeight);

    if (g_settings_menu_overlay_window) {
        LOG_INFO("🔄 Destroying existing settings menu overlay");
        [g_settings_menu_overlay_window close];
        g_settings_menu_overlay_window = nullptr;
    }

    g_settings_menu_overlay_window = [[NSWindow alloc]
        initWithContentRect:menuFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_settings_menu_overlay_window) {
        LOG_ERROR("❌ Failed to create settings menu overlay window");
        return;
    }

    [g_settings_menu_overlay_window setOpaque:NO];
    [g_settings_menu_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_settings_menu_overlay_window setLevel:NSPopUpMenuWindowLevel];  // Higher than floating
    [g_settings_menu_overlay_window setIgnoresMouseEvents:NO];
    [g_settings_menu_overlay_window setReleasedWhenClosed:NO];
    [g_settings_menu_overlay_window setHasShadow:YES];  // Menu has shadow

    SettingsMenuOverlayView* contentView = [[SettingsMenuOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, menuWidth, menuHeight)];
    [g_settings_menu_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("settings_menu"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView, menuWidth, menuHeight);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        handler,
        "http://127.0.0.1:5137/settings-menu",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("❌ Failed to create settings menu overlay CEF browser");
        return;
    }

    [g_settings_menu_overlay_window makeKeyAndOrderFront:nil];
    LOG_INFO("✅ Settings menu overlay created successfully");
}

// ============================================================================
// Graceful Shutdown
// ============================================================================

void ShutdownApplication() {
    LOG_INFO("🛑 Starting graceful application shutdown (macOS)...");

    // Step 1: Close all CEF browsers
    LOG_INFO("🔄 Closing CEF browsers...");

    CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
    CefRefPtr<CefBrowser> webview_browser = SimpleHandler::GetWebviewBrowser();
    CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
    CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
    CefRefPtr<CefBrowser> backup_browser = SimpleHandler::GetBackupBrowser();
    CefRefPtr<CefBrowser> brc100_auth_browser = SimpleHandler::GetBRC100AuthBrowser();
    CefRefPtr<CefBrowser> settings_menu_browser = SimpleHandler::GetSettingsMenuBrowser();

    if (header_browser) {
        LOG_INFO("🔄 Closing header browser...");
        header_browser->GetHost()->CloseBrowser(false);
    }

    if (webview_browser) {
        LOG_INFO("🔄 Closing webview browser...");
        webview_browser->GetHost()->CloseBrowser(false);
    }

    if (settings_browser) {
        LOG_INFO("🔄 Closing settings browser...");
        settings_browser->GetHost()->CloseBrowser(false);
    }

    if (wallet_browser) {
        LOG_INFO("🔄 Closing wallet browser...");
        wallet_browser->GetHost()->CloseBrowser(false);
    }

    if (backup_browser) {
        LOG_INFO("🔄 Closing backup browser...");
        backup_browser->GetHost()->CloseBrowser(false);
    }

    if (brc100_auth_browser) {
        LOG_INFO("🔄 Closing BRC-100 auth browser...");
        brc100_auth_browser->GetHost()->CloseBrowser(false);
    }

    if (settings_menu_browser) {
        LOG_INFO("🔄 Closing settings menu browser...");
        settings_menu_browser->GetHost()->CloseBrowser(false);
    }

    CefRefPtr<CefBrowser> omnibox_browser = SimpleHandler::GetOmniboxOverlayBrowser();
    if (omnibox_browser) {
        LOG_INFO("🔄 Closing omnibox overlay browser...");
        omnibox_browser->GetHost()->CloseBrowser(false);
    }

    // Step 2: Close overlay windows
    LOG_INFO("🔄 Closing overlay windows...");

    if (g_settings_overlay_window) {
        LOG_INFO("🔄 Closing settings overlay window...");
        [g_settings_overlay_window close];
        g_settings_overlay_window = nullptr;
    }

    if (g_wallet_overlay_window) {
        LOG_INFO("🔄 Closing wallet overlay window...");
        [g_wallet_overlay_window close];
        g_wallet_overlay_window = nullptr;
    }

    if (g_backup_overlay_window) {
        LOG_INFO("🔄 Closing backup overlay window...");
        [g_backup_overlay_window close];
        g_backup_overlay_window = nullptr;
    }

    if (g_brc100_auth_overlay_window) {
        LOG_INFO("🔄 Closing BRC-100 auth overlay window...");
        [g_brc100_auth_overlay_window close];
        g_brc100_auth_overlay_window = nullptr;
    }

    if (g_settings_menu_overlay_window) {
        LOG_INFO("🔄 Closing settings menu overlay window...");
        [g_settings_menu_overlay_window close];
        g_settings_menu_overlay_window = nullptr;
    }

    if (g_omnibox_overlay_window) {
        LOG_INFO("🔄 Closing omnibox overlay window...");
        [g_omnibox_overlay_window close];
        g_omnibox_overlay_window = nullptr;
    }

    // Step 3: Close main window
    if (g_main_window) {
        LOG_INFO("🔄 Closing main window...");
        [g_main_window close];
        g_main_window = nullptr;
    }

    LOG_INFO("✅ Application shutdown complete (macOS)");
    Logger::Shutdown();

    // Quit application
    [NSApp terminate:nil];
}

// ============================================================================
// Main Entry Point (macOS)
// ============================================================================

int main(int argc, char* argv[]) {
    // CRITICAL: Load CEF framework before calling any CEF functions (macOS only)
    // Get executable path
    char exec_path[1024];
    uint32_t exec_path_size = sizeof(exec_path);
    if (_NSGetExecutablePath(exec_path, &exec_path_size) != 0) {
        fprintf(stderr, "❌ Failed to get executable path\n");
        return 1;
    }

    // Build framework path
    NSString* execPath = [NSString stringWithUTF8String:exec_path];
    NSString* execDir = [execPath stringByDeletingLastPathComponent];
    NSString* frameworkPath = [execDir stringByAppendingPathComponent:@"../Frameworks/Chromium Embedded Framework.framework/Chromium Embedded Framework"];
    frameworkPath = [frameworkPath stringByStandardizingPath];

    // Load CEF framework library
    if (!cef_load_library([frameworkPath UTF8String])) {
        fprintf(stderr, "❌ Failed to load CEF framework at: %s\n", [frameworkPath UTF8String]);
        return 1;
    }

    fprintf(stderr, "✅ CEF framework loaded successfully\n");

    // CEF subprocess handling
    CefMainArgs main_args(argc, argv);

    @autoreleasepool {
        // CRITICAL: Initialize NSApplication BEFORE CefExecuteProcess and CefInitialize
        [HodosBrowserApplication sharedApplication];

        // Verify we got the right NSApplication subclass
        if (![NSApp isKindOfClass:[HodosBrowserApplication class]]) {
            fprintf(stderr, "❌ NSApp is not HodosBrowserApplication!\n");
            return 1;
        }

        fprintf(stderr, "✅ HodosBrowserApplication initialized\n");

        CefRefPtr<SimpleApp> app(new SimpleApp());

        // Handle subprocesses (render, GPU, plugin processes)
        // CRITICAL: Helpers should NOT activate as regular apps
        int exit_code = CefExecuteProcess(main_args, app, nullptr);
        if (exit_code >= 0) {
            // Subprocess - exit immediately WITHOUT activating as regular app
            // This prevents helper processes from appearing in Dock
            return exit_code;
        }

        // Main process continues...
        // Only the main process should be a regular application
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];

        // Initialize centralized logger
        Logger::Initialize(ProcessType::MAIN, "debug_output.log");
        LOG_INFO("=== NEW SESSION STARTED (macOS) ===");
        LOG_INFO("🍎 HodosBrowser Shell starting on macOS...");

        // Configure CEF settings
        CefSettings settings;
        settings.command_line_args_disabled = false;
        CefString(&settings.log_file).FromASCII("debug.log");
        settings.log_severity = LOGSEVERITY_INFO;
        settings.remote_debugging_port = 9222;
        settings.windowless_rendering_enabled = true;  // Required for overlays

        // CRITICAL: Disable sandbox on macOS for development (requires code signing otherwise)
        settings.no_sandbox = true;

        // Set macOS bundle paths
        NSBundle* mainBundle = [NSBundle mainBundle];
        NSString* bundlePath = [mainBundle bundlePath];
        NSString* frameworksPath = [bundlePath stringByAppendingPathComponent:@"Contents/Frameworks"];
        NSString* cefFramework = [frameworksPath stringByAppendingPathComponent:@"Chromium Embedded Framework.framework"];
        NSString* resourcesPath = [cefFramework stringByAppendingPathComponent:@"Resources"];

        std::string resources_dir = [resourcesPath UTF8String];
        std::string locales_dir = resources_dir + "/locales";

        CefString(&settings.resources_dir_path).FromString(resources_dir);
        CefString(&settings.locales_dir_path).FromString(locales_dir);

        LOG_INFO("📁 CEF resources path: " + resources_dir);
        LOG_INFO("📁 CEF locales path: " + locales_dir);

        // Set cache path (browser data: history, cookies, etc.)
        NSArray* paths = NSSearchPathForDirectoriesInDomains(
            NSApplicationSupportDirectory, NSUserDomainMask, YES);
        NSString* appSupport = [paths firstObject];
        NSString* hodosBrowserDir = [appSupport stringByAppendingPathComponent:@"HodosBrowser"];
        NSString* defaultDir = [hodosBrowserDir stringByAppendingPathComponent:@"Default"];

        std::string cache_path = [defaultDir UTF8String];
        CefString(&settings.root_cache_path).FromString([hodosBrowserDir UTF8String]);
        CefString(&settings.cache_path).FromString(cache_path);

        LOG_INFO("📁 Cache path: " + cache_path);

        // Enable JavaScript features
        CefString(&settings.javascript_flags).FromASCII("--expose-gc --allow-running-insecure-content");

        // Set subprocess path to helper bundle
        // On macOS, helpers are in Contents/Frameworks/HodosBrowser Helper.app/Contents/MacOS/HodosBrowser Helper
        NSString* appPath = [[NSBundle mainBundle] bundlePath];
        NSString* helperPath = [appPath stringByAppendingPathComponent:@"Contents/Frameworks/HodosBrowser Helper.app/Contents/MacOS/HodosBrowser Helper"];
        std::string helper_path = [helperPath UTF8String];
        CefString(&settings.browser_subprocess_path).FromString(helper_path);
        LOG_INFO("📁 Subprocess path: " + helper_path);

        // Initialize CEF first (before creating windows)
        LOG_INFO("🔄 Initializing CEF...");
        bool cef_success = CefInitialize(main_args, settings, app, nullptr);
        LOG_INFO("CefInitialize result: " + std::string(cef_success ? "✅ SUCCESS" : "❌ FAILED"));

        if (!cef_success) {
            LOG_ERROR("❌ CEF initialization failed - exiting");
            return 1;
        }

        // Create windows after CEF is initialized
        // (Activation policy already set above for main process only)
        CreateMainWindow();

        if (!g_main_window || !g_header_view || !g_webview_view) {
            LOG_ERROR("❌ Window creation failed - exiting");
            CefShutdown();
            return 1;
        }

        // Store window references for later use
        app->SetMacOSWindow((__bridge void*)g_main_window,
                           (__bridge void*)g_header_view,
                           (__bridge void*)g_webview_view);

        LOG_INFO("✅ Windows created, now manually creating header browser");

        // Manually create header browser using SetAsChild (standard macOS approach)
        NSView* headerView = (__bridge NSView*)g_header_view;
        NSRect headerBounds = [headerView bounds];

        LOG_INFO("🔧 Creating header browser with SetAsChild (child window rendering)");
        LOG_INFO("📐 Header bounds: " + std::to_string((int)headerBounds.size.width) +
                 "x" + std::to_string((int)headerBounds.size.height));

        CefWindowInfo header_window_info;
        // Use child window rendering (CEF handles rendering automatically)
        CefRect headerRect(0, 0, (int)headerBounds.size.width, (int)headerBounds.size.height);
        header_window_info.SetAsChild((__bridge void*)headerView, headerRect);

        CefRefPtr<SimpleHandler> header_handler = new SimpleHandler("header");

        CefBrowserSettings header_settings;
        header_settings.background_color = CefColorSetARGB(255, 255, 255, 255);

        bool browser_created = CefBrowserHost::CreateBrowser(
            header_window_info,
            header_handler,
            "http://127.0.0.1:5137",  // Load React frontend (localhost now allowed)
            header_settings,
            nullptr,
            CefRequestContext::GetGlobalContext()
        );

        LOG_INFO("✅ Header browser creation result: " + std::string(browser_created ? "SUCCESS" : "FAILED"));

        // ===================================================================
        // Create Webview Browser (for actual webpage content)
        // ===================================================================

        NSView* webviewView = (__bridge NSView*)g_webview_view;
        NSRect webviewBounds = [webviewView bounds];

        LOG_INFO("🔧 Creating webview browser with SetAsChild (for webpage content)");
        LOG_INFO("📐 Webview bounds: " + std::to_string((int)webviewBounds.size.width) +
                 "x" + std::to_string((int)webviewBounds.size.height));

        CefWindowInfo webview_window_info;
        CefRect webviewRect(0, 0, (int)webviewBounds.size.width, (int)webviewBounds.size.height);
        webview_window_info.SetAsChild((__bridge void*)webviewView, webviewRect);

        CefRefPtr<SimpleHandler> webview_handler = new SimpleHandler("webview");

        CefBrowserSettings webview_settings;
        webview_settings.background_color = CefColorSetARGB(255, 255, 255, 255);

        bool webview_created = CefBrowserHost::CreateBrowser(
            webview_window_info,
            webview_handler,
            "https://metanetapps.com",  // Default page
            webview_settings,
            nullptr,
            CefRequestContext::GetGlobalContext()
        );

        LOG_INFO("✅ Webview browser creation result: " + std::string(webview_created ? "SUCCESS" : "FAILED"));

        // Note: Wallet panel browser removed - now uses overlay window approach
        // (CreateWalletOverlayWithSeparateProcess) to match Windows implementation

        // Debug: Check webview hierarchy after browsers created
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 3 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
            NSView* webview = (__bridge NSView*)g_webview_view;
            LOG_INFO("🔍 Webview has " + std::to_string([[webview subviews] count]) + " subviews");
            LOG_INFO("🔍 Webview frame: Y=" + std::to_string((int)[webview frame].origin.y) +
                     " H=" + std::to_string((int)[webview frame].size.height));

            for (NSView* subview in [webview subviews]) {
                NSRect frame = [subview frame];
                LOG_INFO("  CEF subview: " + std::string([[subview className] UTF8String]) +
                         " Y=" + std::to_string((int)frame.origin.y) +
                         " H=" + std::to_string((int)frame.size.height));
            }
        });

        // ===================================================================
        // Initialize HistoryManager
        // ===================================================================

        LOG_INFO("🔄 Initializing HistoryManager...");
        if (HistoryManager::GetInstance().Initialize(cache_path)) {
            LOG_INFO("✅ HistoryManager initialized successfully");
        } else {
            LOG_ERROR("❌ Failed to initialize HistoryManager");
        }

        // Debug: Check if CEF added a child view to our header view
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 2 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
            NSArray* subviews = [headerView subviews];
            LOG_INFO("🔍 Header view has " + std::to_string([subviews count]) + " subviews");
            for (NSView* subview in subviews) {
                NSRect frame = [subview frame];
                BOOL hidden = [subview isHidden];
                CGFloat alpha = [subview alphaValue];
                LOG_INFO("  Subview: " + std::string([[subview className] UTF8String]) +
                         " origin: (" + std::to_string((int)frame.origin.x) + "," + std::to_string((int)frame.origin.y) + ")" +
                         " size: " + std::to_string((int)frame.size.width) + "x" + std::to_string((int)frame.size.height) +
                         " hidden: " + std::string(hidden ? "YES" : "NO") +
                         " alpha: " + std::to_string(alpha));

                // Force correct position and size
                NSRect correctFrame = NSMakeRect(0, 0, [headerView bounds].size.width, [headerView bounds].size.height);
                if (!NSEqualRects(frame, correctFrame)) {
                    [subview setFrame:correctFrame];
                    LOG_INFO("  → Corrected frame to: " + std::to_string((int)correctFrame.size.width) + "x" + std::to_string((int)correctFrame.size.height));
                }

                // Force subview to be visible
                if (hidden) {
                    [subview setHidden:NO];
                    LOG_INFO("  → Unhid subview");
                }
                if (alpha < 1.0) {
                    [subview setAlphaValue:1.0];
                    LOG_INFO("  → Set alpha to 1.0");
                }

                // Force display update
                [subview setNeedsDisplay:YES];
                [subview displayIfNeeded];
                [headerView setNeedsDisplay:YES];
                [headerView displayIfNeeded];
                LOG_INFO("  → Forced display update on CEF view and parent");
            }
        });

        // TODO: Initialize HistoryManager on macOS
        // HistoryManager is currently Windows-only (uses SQLite with Windows APIs)
        LOG_INFO("🔧 HistoryManager not implemented on macOS yet");

        LOG_INFO("🚀 Entering CEF message loop...");

        // Run CEF message loop (blocks until quit)
        CefRunMessageLoop();

        // Cleanup
        LOG_INFO("🔄 CEF message loop exited - shutting down...");
        CefShutdown();

        LOG_INFO("✅ Application exited cleanly");
        Logger::Shutdown();

        return 0;
    }
}
