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
#include "OverlayHelpers_mac.h"

#include <iostream>
#include <fstream>
#include <chrono>
#include <iomanip>
#include <sstream>
#include <algorithm>
#include <filesystem>
#include <nlohmann/json.hpp>

// ============================================================================
// Forward Declarations
// ============================================================================
void ShutdownApplication();
void ShowQuitConfirmationAndShutdown();
void HideApplication();
void HandleCmdD();  // Cmd+D — Bookmark current page
void HandleCmdL();  // Cmd+L — Focus address bar

using json = nlohmann::json;
namespace fs = std::filesystem;

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
  // Intercept Cmd+D and Cmd+L before NSMenu swallows them.
  // These keys aren't standard Edit menu actions so macOS discards them
  // before CEF's OnPreKeyEvent ever fires.
  if (event.type == NSEventTypeKeyDown) {
    NSUInteger flags = event.modifierFlags & NSEventModifierFlagDeviceIndependentFlagsMask;
    if (flags & NSEventModifierFlagCommand) {
      NSString* chars = [event charactersIgnoringModifiers];
      if ([chars length] == 1) {
        unichar ch = [chars characterAtIndex:0];
        if (ch == 'd' || ch == 'D') {
          HandleCmdD();
          [CATransaction flush];
          return;
        }
        if (ch == 'l' || ch == 'L') {
          HandleCmdL();
          [CATransaction flush];
          return;
        }
      }
    }
  }

  CefScopedSendingEvent sendingEventScoper;
  [super sendEvent:event];

  // Flush Core Animation transactions after key events to ensure CEF's
  // compositor output is displayed immediately.  Without this, visual
  // changes triggered by keyboard shortcuts (e.g. Cmd+A text selection)
  // may not appear on screen until the window loses focus.
  if (event.type == NSEventTypeKeyDown &&
      (event.modifierFlags & NSEventModifierFlagCommand)) {
    [CATransaction flush];
  }
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
#include "include/core/ProfileManager.h"
#include "include/core/ProfileLock.h"
#include "include/core/SettingsManager.h"
#include "include/core/SyncHttpClient.h"
#include "include/core/AdblockCache.h"
#include "include/core/FingerprintProtection.h"
#include "include/core/CookieBlockManager.h"
#include "include/core/BookmarkManager.h"
#include "include/core/WindowManager.h"

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
NSWindow* g_notification_overlay_window = nullptr;
NSWindow* g_settings_menu_overlay_window = nullptr;
NSWindow* g_menu_overlay_window = nullptr;
NSWindow* g_cookie_panel_overlay_window = nullptr;
NSWindow* g_omnibox_overlay_window = nullptr;
NSWindow* g_download_panel_overlay_window = nullptr;
NSWindow* g_profile_panel_overlay_window = nullptr;

// OverlayBrowserRef instances for overlays using GenericOverlayView
static OverlayBrowserRef* g_menu_overlay_browser_ref = nullptr;
static CefRefPtr<MyOverlayRenderHandler> g_menu_overlay_render_handler = nullptr;

// Overlay state flags (mirrors Windows globals from cef_browser_shell.cpp)
bool g_file_dialog_active = false;
bool g_wallet_overlay_prevent_close = false;
int g_peerpay_count = 0;
int g_peerpay_amount = 0;

// Stored icon right offsets for repositioning overlays on move/resize (physical pixels)
static int g_mac_settings_icon_right_offset = 0;
static int g_mac_wallet_icon_right_offset = 0;
static int g_mac_cookie_panel_icon_right_offset = 0;

// Omnibox overlay monitors
static id g_omnibox_click_monitor = nil;
static CFAbsoluteTime g_omnibox_last_hide_time = 0;

// Download panel overlay monitors
static id g_download_panel_click_monitor = nil;
static CFAbsoluteTime g_download_panel_last_hide_time = 0;
static int g_mac_download_panel_icon_right_offset = 0;

// Profile panel overlay monitors
static id g_profile_panel_click_monitor = nil;
static CFAbsoluteTime g_profile_panel_last_hide_time = 0;
static int g_mac_profile_panel_icon_right_offset = 0;

// Server process management
static pid_t g_wallet_server_pid = -1;
static pid_t g_adblock_server_pid = -1;
bool g_walletServerRunning = false;
bool g_adblockServerRunning = false;

// Convenience macros for easier logging
#define LOG_DEBUG(msg) Logger::Log(msg, 0, 0)
#define LOG_INFO(msg) Logger::Log(msg, 1, 0)
#define LOG_WARNING(msg) Logger::Log(msg, 2, 0)
#define LOG_ERROR(msg) Logger::Log(msg, 3, 0)

// Legacy function for backward compatibility
void DebugLog(const std::string& message) {
    LOG_INFO(message);
}

// Handle fullscreen mode transitions (called from SimpleHandler::OnFullscreenModeChange)
void HandleFullscreenChange(bool fullscreen) {
    // TODO: Implement macOS fullscreen handling (NSWindow toggleFullScreen)
    LOG_INFO("HandleFullscreenChange: " + std::string(fullscreen ? "enter" : "exit") + " (macOS stub)");
}

// Toggle fullscreen for the main window (called from menu_action "fullscreen")
void ToggleFullScreenMacOS() {
    if (g_main_window) {
        [g_main_window toggleFullScreen:nil];
    }
}

void ToggleMainWindowFullscreen() {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (g_main_window) {
            [g_main_window toggleFullScreen:nil];
        }
    });
}

static void DestroyMenuOverlayWindow(bool closeBrowser) {
    if (!g_menu_overlay_window && !g_menu_overlay_browser_ref && !g_menu_overlay_render_handler) {
        return;
    }

    NSWindow* overlayWindow = g_menu_overlay_window;
    OverlayBrowserRef* browserRef = g_menu_overlay_browser_ref;
    CefRefPtr<MyOverlayRenderHandler> renderHandler = g_menu_overlay_render_handler;
    CefRefPtr<CefBrowser> menuBrowser =
        (browserRef && browserRef->browser) ? browserRef->browser : SimpleHandler::GetMenuBrowser();

    g_menu_overlay_window = nullptr;
    g_menu_overlay_browser_ref = nullptr;
    g_menu_overlay_render_handler = nullptr;

    if (overlayWindow) {
        RemoveClickOutsideMonitor(overlayWindow);
    }

    if (renderHandler) {
        renderHandler->DetachView();
    }

    if (overlayWindow) {
        NSView* contentView = [overlayWindow contentView];
        if ([contentView isKindOfClass:[GenericOverlayView class]]) {
            [(GenericOverlayView*)contentView detachBrowser];
        }

        if (g_main_window) {
            [g_main_window removeChildWindow:overlayWindow];
        }

        [overlayWindow orderOut:nil];
        [overlayWindow close];
    }

    if (closeBrowser && menuBrowser) {
        menuBrowser->GetHost()->CloseBrowser(false);
    }

    delete browserRef;
    LOG_INFO("Menu overlay hidden/closed");
}

static void ClearPersistedInternalFrontendZoom(const std::string& profileCachePath) {
    try {
        fs::path preferencesPath = fs::path(profileCachePath) / "Preferences";
        if (!fs::exists(preferencesPath)) {
            return;
        }

        std::ifstream input(preferencesPath);
        if (!input.is_open()) {
            LOG_WARNING("Could not open Preferences to clear internal frontend zoom");
            return;
        }

        json prefs;
        input >> prefs;
        input.close();

        bool changed = false;
        auto partitionIt = prefs.find("partition");
        if (partitionIt != prefs.end() && partitionIt->is_object()) {
            auto zoomsIt = partitionIt->find("per_host_zoom_levels");
            if (zoomsIt != partitionIt->end() && zoomsIt->is_object()) {
                for (auto& [partitionKey, hostMap] : zoomsIt->items()) {
                    if (!hostMap.is_object()) {
                        continue;
                    }
                    changed = hostMap.erase("127.0.0.1") > 0 || changed;
                    changed = hostMap.erase("localhost") > 0 || changed;
                }
            }
        }

        if (!changed) {
            return;
        }

        std::ofstream output(preferencesPath);
        if (!output.is_open()) {
            LOG_WARNING("Could not rewrite Preferences after clearing internal frontend zoom");
            return;
        }

        output << prefs.dump();
        output.close();
        LOG_INFO("Cleared persisted Chromium zoom for internal frontend hosts");
    } catch (const std::exception& ex) {
        LOG_WARNING("Failed to clear persisted internal frontend zoom: " + std::string(ex.what()));
    }
}

// Cmd+D — Bookmark current page (called from sendEvent: before NSMenu swallows it)
void HandleCmdD() {
    auto* activeTab = TabManager::GetInstance().GetActiveTab();
    if (activeTab && !activeTab->url.empty()) {
        LOG_INFO("⌨️ Cmd+D: Bookmarking " + activeTab->url);
        std::vector<std::string> emptyTags;
        BookmarkManager::GetInstance().AddBookmark(
            activeTab->url, activeTab->title, -1, emptyTags);
    }
}

// Cmd+L — Focus address bar (called from sendEvent: before NSMenu swallows it)
void HandleCmdL() {
    LOG_INFO("⌨️ Cmd+L: Focus address bar");
    CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
    if (header) {
        CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("focus_address_bar");
        header->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
        header->GetHost()->SetFocus(true);
    }
}

// ============================================================================
// Forward Declarations
// ============================================================================

void ToggleWalletPanel();  // C++ callable function
void CreateMainWindow();
void CreateSettingsOverlayWithSeparateProcess(int iconRightOffset);
void CreateWalletOverlayWithSeparateProcess(int iconRightOffset);
void CreateBackupOverlayWithSeparateProcess();
void CreateBRC100AuthOverlayWithSeparateProcess();
void CreateNotificationOverlay(const std::string& type, const std::string& domain, const std::string& extraParams);
void CreateSettingsMenuOverlay();
void CreateMenuOverlayMac(int iconRightOffset);
void ShowSettingsMenuOverlay();
void HideSettingsMenuOverlay();
bool IsSettingsMenuOverlayVisible();
bool WasSettingsMenuJustHidden();
void CreateCookiePanelOverlayWithSeparateProcess(int iconRightOffset);
void ShowCookiePanelOverlay(int iconRightOffset);
void HideCookiePanelOverlay();
bool IsCookiePanelOverlayVisible();
void CreateOmniboxOverlayMacOS();
void ShowOmniboxOverlayMacOS();
void HideOmniboxOverlayMacOS();
bool OmniboxOverlayExists();
void CreateDownloadPanelOverlayMacOS(int iconRightOffset);
void ShowDownloadPanelOverlayMacOS(int iconRightOffset);
void HideDownloadPanelOverlayMacOS();
void CreateProfilePanelOverlayMacOS(int iconRightOffset);
void ShowProfilePanelOverlayMacOS(int iconRightOffset);
void HideProfilePanelOverlayMacOS();
void ShutdownApplication();
void ToggleFullScreenMacOS();

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

// ============================================================================
// Generic Dropdown Overlay Classes (reusable for Omnibox, Download, Profile)
// ============================================================================

typedef CefRefPtr<CefBrowser> (^OverlayBrowserAccessor)(void);

@interface DropdownOverlayWindow : NSWindow
@end

@implementation DropdownOverlayWindow
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }

- (void)sendEvent:(NSEvent *)event {
    NSEventType type = [event type];
    NSView* view = [self contentView];
    switch (type) {
        case NSEventTypeLeftMouseDown:    [view mouseDown:event]; return;
        case NSEventTypeLeftMouseUp:      [view mouseUp:event]; return;
        case NSEventTypeLeftMouseDragged: [view mouseDragged:event]; return;
        case NSEventTypeRightMouseDown:   [view rightMouseDown:event]; return;
        case NSEventTypeRightMouseUp:     [view rightMouseUp:event]; return;
        case NSEventTypeMouseMoved:       [view mouseMoved:event]; return;
        case NSEventTypeScrollWheel:      [view scrollWheel:event]; return;
        case NSEventTypeMouseEntered:     [view mouseEntered:event]; return;
        case NSEventTypeMouseExited:      [view mouseExited:event]; return;
        case NSEventTypeKeyDown:          [view keyDown:event]; return;
        case NSEventTypeKeyUp:            [view keyUp:event]; return;
        default: [super sendEvent:event]; return;
    }
}
@end

@interface DropdownOverlayView : NSView
@property (nonatomic, copy) OverlayBrowserAccessor browserAccessor;
@property (nonatomic, strong) CALayer* renderLayer;
@property (nonatomic, strong) NSTrackingArea* overlayTrackingArea;
@end

@implementation DropdownOverlayView

- (instancetype)initWithFrame:(NSRect)frame {
    self = [super initWithFrame:frame];
    if (self) {
        _renderLayer = [CALayer layer];
        _renderLayer.opaque = NO;
        [self setLayer:_renderLayer];
        [self setWantsLayer:YES];

        _overlayTrackingArea = [[NSTrackingArea alloc]
            initWithRect:self.bounds
            options:(NSTrackingMouseMoved | NSTrackingMouseEnteredAndExited |
                     NSTrackingActiveAlways | NSTrackingInVisibleRect)
            owner:self
            userInfo:nil];
        [self addTrackingArea:_overlayTrackingArea];
    }
    return self;
}

- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    if (_overlayTrackingArea) {
        [self removeTrackingArea:_overlayTrackingArea];
    }
    _overlayTrackingArea = [[NSTrackingArea alloc]
        initWithRect:self.bounds
        options:(NSTrackingMouseMoved | NSTrackingMouseEnteredAndExited |
                 NSTrackingActiveAlways | NSTrackingInVisibleRect)
        owner:self
        userInfo:nil];
    [self addTrackingArea:_overlayTrackingArea];
}

- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }
- (BOOL)acceptsFirstMouse:(NSEvent *)event { return YES; }
- (BOOL)isOpaque { return NO; }
- (NSView *)hitTest:(NSPoint)point { return self; }

- (CefRefPtr<CefBrowser>)overlayBrowser {
    return _browserAccessor ? _browserAccessor() : nullptr;
}

- (void)mouseDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self overlayBrowser];
    if (!b) return;
    NSPoint loc = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent me;
    me.x = loc.x;
    me.y = self.bounds.size.height - loc.y;
    me.modifiers = 0;
    b->GetHost()->SetFocus(true);
    b->GetHost()->SendMouseClickEvent(me, MBT_LEFT, false, 1);
    b->GetHost()->SendMouseClickEvent(me, MBT_LEFT, true, 1);
}

- (void)mouseUp:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self overlayBrowser];
    if (!b) return;
    NSPoint loc = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent me;
    me.x = loc.x;
    me.y = self.bounds.size.height - loc.y;
    me.modifiers = 0;
    b->GetHost()->SendMouseClickEvent(me, MBT_LEFT, true, 1);
}

- (void)mouseDragged:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self overlayBrowser];
    if (!b) return;
    NSPoint loc = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent me;
    me.x = loc.x;
    me.y = self.bounds.size.height - loc.y;
    me.modifiers = EVENTFLAG_LEFT_MOUSE_BUTTON;
    b->GetHost()->SendMouseMoveEvent(me, false);
}

- (void)rightMouseDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self overlayBrowser];
    if (!b) return;
    NSPoint loc = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent me;
    me.x = loc.x;
    me.y = self.bounds.size.height - loc.y;
    me.modifiers = 0;
    b->GetHost()->SendMouseClickEvent(me, MBT_RIGHT, false, 1);
    b->GetHost()->SendMouseClickEvent(me, MBT_RIGHT, true, 1);
}

- (void)scrollWheel:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self overlayBrowser];
    if (!b) return;
    NSPoint loc = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent me;
    me.x = loc.x;
    me.y = self.bounds.size.height - loc.y;
    me.modifiers = 0;
    int deltaX = (int)([event scrollingDeltaX] * 2);
    int deltaY = (int)([event scrollingDeltaY] * 2);
    b->GetHost()->SendMouseWheelEvent(me, deltaX, deltaY);
}

- (void)mouseMoved:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self overlayBrowser];
    if (!b) return;
    NSPoint loc = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent me;
    me.x = loc.x;
    me.y = self.bounds.size.height - loc.y;
    me.modifiers = 0;
    b->GetHost()->SendMouseMoveEvent(me, false);
}

- (void)mouseEntered:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self overlayBrowser];
    if (!b) return;
    NSPoint loc = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent me;
    me.x = loc.x;
    me.y = self.bounds.size.height - loc.y;
    me.modifiers = 0;
    b->GetHost()->SendMouseMoveEvent(me, false);
}

- (void)mouseExited:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self overlayBrowser];
    if (!b) return;
    CefMouseEvent me;
    me.x = -1;
    me.y = -1;
    me.modifiers = 0;
    b->GetHost()->SendMouseMoveEvent(me, true);
}

- (void)keyDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self overlayBrowser];
    if (!b) return;

    CefKeyEvent key_event;
    key_event.native_key_code = [event keyCode];
    key_event.modifiers = 0;
    if ([event modifierFlags] & NSEventModifierFlagShift) key_event.modifiers |= EVENTFLAG_SHIFT_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagControl) key_event.modifiers |= EVENTFLAG_CONTROL_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagOption) key_event.modifiers |= EVENTFLAG_ALT_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagCommand) key_event.modifiers |= EVENTFLAG_COMMAND_DOWN;

    // Send raw key down
    key_event.type = KEYEVENT_RAWKEYDOWN;
    NSString* chars = [event charactersIgnoringModifiers];
    if ([chars length] > 0) {
        key_event.windows_key_code = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
    }
    b->GetHost()->SendKeyEvent(key_event);

    // Send char event for printable characters
    NSString* typedChars = [event characters];
    if ([typedChars length] > 0) {
        CefKeyEvent char_event;
        char_event.type = KEYEVENT_CHAR;
        char_event.windows_key_code = [typedChars characterAtIndex:0];
        char_event.character = [typedChars characterAtIndex:0];
        char_event.unmodified_character = [typedChars characterAtIndex:0];
        char_event.native_key_code = [event keyCode];
        char_event.modifiers = key_event.modifiers;
        b->GetHost()->SendKeyEvent(char_event);
    }
}

- (void)keyUp:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self overlayBrowser];
    if (!b) return;

    CefKeyEvent key_event;
    key_event.type = KEYEVENT_KEYUP;
    key_event.native_key_code = [event keyCode];
    key_event.modifiers = 0;
    if ([event modifierFlags] & NSEventModifierFlagShift) key_event.modifiers |= EVENTFLAG_SHIFT_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagControl) key_event.modifiers |= EVENTFLAG_CONTROL_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagOption) key_event.modifiers |= EVENTFLAG_ALT_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagCommand) key_event.modifiers |= EVENTFLAG_COMMAND_DOWN;

    NSString* chars = [event charactersIgnoringModifiers];
    if ([chars length] > 0) {
        key_event.windows_key_code = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
    }
    b->GetHost()->SendKeyEvent(key_event);
}

@end

// Cookie Panel (Privacy Shield) Overlay View
@interface CookiePanelOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@end

@implementation CookiePanelOverlayView

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

    CefRefPtr<CefBrowser> cookie = SimpleHandler::GetCookiePanelBrowser();
    if (cookie) {
        cookie->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
        cookie->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
        LOG_DEBUG("Cookie panel overlay: Left-click forwarded to CEF");
    }
}

- (void)rightMouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> cookie = SimpleHandler::GetCookiePanelBrowser();
    if (cookie) {
        cookie->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
        cookie->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
        LOG_DEBUG("Cookie panel overlay: Right-click forwarded to CEF");
    }
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> cookie = SimpleHandler::GetCookiePanelBrowser();
    if (cookie) {
        cookie->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)scrollWheel:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> cookie = SimpleHandler::GetCookiePanelBrowser();
    if (cookie) {
        int deltaX = (int)([event scrollingDeltaX] * 2);
        int deltaY = (int)([event scrollingDeltaY] * 2);
        cookie->GetHost()->SendMouseWheelEvent(mouse_event, deltaX, deltaY);
    }
}

- (void)keyDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> cookie = SimpleHandler::GetCookiePanelBrowser();
    if (!cookie) return;

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
    cookie->GetHost()->SendKeyEvent(key_event);

    // Send CHAR event for character input (critical for typing)
    if (chars.length > 0) {
        key_event.type = KEYEVENT_CHAR;
        key_event.character = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
        cookie->GetHost()->SendKeyEvent(key_event);
    }

    LOG_DEBUG("Cookie panel overlay: Key events forwarded to CEF");
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

    CefRefPtr<CefBrowser> cookie = SimpleHandler::GetCookiePanelBrowser();
    if (cookie) {
        cookie->GetHost()->SendKeyEvent(key_event);
    }
}

@end

// Wallet Overlay View
@interface WalletOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@property (nonatomic, strong) NSTrackingArea* walletTrackingArea;
@end

@implementation WalletOverlayView

- (instancetype)initWithFrame:(NSRect)frame {
    self = [super initWithFrame:frame];
    if (self) {
        _renderLayer = [CALayer layer];
        _renderLayer.opaque = NO;
        [self setLayer:_renderLayer];
        [self setWantsLayer:YES];

        // NSTrackingArea is REQUIRED for mouseMoved/mouseEntered/mouseExited on macOS
        _walletTrackingArea = [[NSTrackingArea alloc]
            initWithRect:self.bounds
            options:(NSTrackingMouseMoved | NSTrackingMouseEnteredAndExited |
                     NSTrackingActiveAlways | NSTrackingInVisibleRect)
            owner:self
            userInfo:nil];
        [self addTrackingArea:_walletTrackingArea];
    }
    return self;
}

- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    if (_walletTrackingArea) {
        [self removeTrackingArea:_walletTrackingArea];
    }
    _walletTrackingArea = [[NSTrackingArea alloc]
        initWithRect:self.bounds
        options:(NSTrackingMouseMoved | NSTrackingMouseEnteredAndExited |
                 NSTrackingActiveAlways | NSTrackingInVisibleRect)
        owner:self
        userInfo:nil];
    [self addTrackingArea:_walletTrackingArea];
}

- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }
- (BOOL)acceptsFirstMouse:(NSEvent *)event { return YES; }
- (BOOL)isOpaque { return NO; }

// CRITICAL: Override hitTest to ensure this view ALWAYS receives mouse events
- (NSView *)hitTest:(NSPoint)point {
    // Log to file for diagnostics
    static std::string hitLogPath;
    if (hitLogPath.empty()) {
        NSString* appSup = [NSSearchPathForDirectoriesInDomains(NSApplicationSupportDirectory, NSUserDomainMask, YES) firstObject];
        hitLogPath = std::string([[appSup stringByAppendingPathComponent:@"HodosBrowser/wallet_events.log"] UTF8String]);
    }
    static int hitCount = 0;
    if (hitCount < 10) {
        std::ofstream dbg(hitLogPath, std::ios::app);
        dbg << "hitTest called: point=(" << point.x << "," << point.y
            << ") bounds=(" << self.bounds.origin.x << "," << self.bounds.origin.y
            << "," << self.bounds.size.width << "," << self.bounds.size.height
            << ") subviews=" << [[self subviews] count]
            << " self=" << (void*)self
            << std::endl;
        // Log subviews if any exist
        for (NSView* sub in [self subviews]) {
            NSRect f = [sub frame];
            dbg << "  subview: " << [[sub className] UTF8String]
                << " frame=(" << f.origin.x << "," << f.origin.y
                << "," << f.size.width << "," << f.size.height
                << ") hidden=" << [sub isHidden]
                << std::endl;
        }
        dbg.close();
        hitCount++;
    }
    return self;  // Always return self — we handle ALL events
}

- (void)mouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = 0;

    // Use file logging (NSLog doesn't appear in log show for unsigned apps)
    static std::string mdLogPath;
    if (mdLogPath.empty()) {
        NSString* appSup = [NSSearchPathForDirectoriesInDomains(NSApplicationSupportDirectory, NSUserDomainMask, YES) firstObject];
        mdLogPath = std::string([[appSup stringByAppendingPathComponent:@"HodosBrowser/wallet_events.log"] UTF8String]);
    }
    std::ofstream dbg(mdLogPath, std::ios::app);
    dbg << "VIEW mouseDown: NSView(" << location.x << "," << location.y
        << ") → CEF(" << mouse_event.x << "," << mouse_event.y
        << ") bounds=" << self.bounds.size.width << "x" << self.bounds.size.height << std::endl;

    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        wallet->GetHost()->SetFocus(true);
        wallet->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
        wallet->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
        dbg << "  → Click sent to wallet browser ID=" << wallet->GetIdentifier() << std::endl;
    } else {
        dbg << "  ❌ GetWalletBrowser() returned NULL!" << std::endl;
    }
    dbg.close();
}

- (void)mouseUp:(NSEvent *)event {
    // mouseUp is now handled in mouseDown (combined down+up pattern)
    // Keep this for any future separation if needed
}

- (void)mouseDragged:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = EVENTFLAG_LEFT_MOUSE_BUTTON;

    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        wallet->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)rightMouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        wallet->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
        wallet->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
    }
}

- (void)scrollWheel:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        // macOS scroll deltas: positive deltaY = scroll up
        int deltaX = (int)([event scrollingDeltaX] * 2);
        int deltaY = (int)([event scrollingDeltaY] * 2);
        wallet->GetHost()->SendMouseWheelEvent(mouse_event, deltaX, deltaY);
    }
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = 0;

    // Log first few mouse moves to confirm event routing works
    static int moveCount = 0;
    if (moveCount < 5) {
        NSLog(@"🖱️ WalletOverlay mouseMoved: CEF(%d,%d)", mouse_event.x, mouse_event.y);
        moveCount++;
    }

    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        wallet->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)mouseEntered:(NSEvent *)event {
    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
        CefMouseEvent mouse_event;
        mouse_event.x = (int)location.x;
        mouse_event.y = (int)(self.bounds.size.height - location.y);
        mouse_event.modifiers = 0;
        wallet->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)mouseExited:(NSEvent *)event {
    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        CefMouseEvent mouse_event;
        mouse_event.x = -1;
        mouse_event.y = -1;
        mouse_event.modifiers = 0;
        wallet->GetHost()->SendMouseMoveEvent(mouse_event, true);  // true = mouse left
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

// CRITICAL FIX: NSWindow's internal dispatch does NOT forward mouse events to our
// content view in borderless+transparent+OSR windows. We must dispatch manually.
- (void)sendEvent:(NSEvent *)event {
    NSEventType type = [event type];
    NSView* view = [self contentView];

    switch (type) {
        case NSEventTypeLeftMouseDown:
            [view mouseDown:event];
            return;
        case NSEventTypeLeftMouseUp:
            [view mouseUp:event];
            return;
        case NSEventTypeLeftMouseDragged:
            [view mouseDragged:event];
            return;
        case NSEventTypeRightMouseDown:
            [view rightMouseDown:event];
            return;
        case NSEventTypeRightMouseUp:
            [view rightMouseUp:event];
            return;
        case NSEventTypeMouseMoved:
            [view mouseMoved:event];
            return;
        case NSEventTypeScrollWheel:
            [view scrollWheel:event];
            return;
        case NSEventTypeMouseEntered:
            [view mouseEntered:event];
            return;
        case NSEventTypeMouseExited:
            [view mouseExited:event];
            return;
        case NSEventTypeKeyDown:
            [view keyDown:event];
            return;
        case NSEventTypeKeyUp:
            [view keyUp:event];
            return;
        default:
            // All other events go through normal dispatch
            [super sendEvent:event];
            return;
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

// Notification Overlay View (domain approval, no-wallet, payment confirmation)
@interface NotificationOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@end

@implementation NotificationOverlayView

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

    CefRefPtr<CefBrowser> notif = SimpleHandler::GetNotificationBrowser();
    if (notif) {
        notif->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
        notif->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
    }
}

- (void)rightMouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> notif = SimpleHandler::GetNotificationBrowser();
    if (notif) {
        notif->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
        notif->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
    }
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent mouse_event;
    mouse_event.x = location.x;
    mouse_event.y = self.bounds.size.height - location.y;
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> notif = SimpleHandler::GetNotificationBrowser();
    if (notif) {
        notif->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)keyDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> notif = SimpleHandler::GetNotificationBrowser();
    if (!notif) return;

    NSString* chars = [event characters];
    NSEventModifierFlags flags = [event modifierFlags];

    int modifiers = 0;
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;

    CefKeyEvent key_event;
    key_event.type = KEYEVENT_RAWKEYDOWN;
    key_event.native_key_code = [event keyCode];
    if (chars.length > 0) {
        key_event.character = [chars characterAtIndex:0];
    }
    key_event.modifiers = modifiers;
    notif->GetHost()->SendKeyEvent(key_event);

    if (chars.length > 0) {
        key_event.type = KEYEVENT_CHAR;
        key_event.character = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
        notif->GetHost()->SendKeyEvent(key_event);
    }
}

- (void)keyUp:(NSEvent *)event {
    CefRefPtr<CefBrowser> notif = SimpleHandler::GetNotificationBrowser();
    if (!notif) return;

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

    notif->GetHost()->SendKeyEvent(key_event);
}

@end

// Settings Menu Overlay View (simplified - dropdown menu)
@interface SettingsMenuOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@property (nonatomic, strong) NSTrackingArea* menuTrackingArea;
@end

@implementation SettingsMenuOverlayView

- (instancetype)initWithFrame:(NSRect)frame {
    self = [super initWithFrame:frame];
    if (self) {
        _renderLayer = [CALayer layer];
        _renderLayer.opaque = NO;
        [self setLayer:_renderLayer];
        [self setWantsLayer:YES];

        _menuTrackingArea = [[NSTrackingArea alloc]
            initWithRect:self.bounds
            options:(NSTrackingMouseMoved | NSTrackingMouseEnteredAndExited |
                     NSTrackingActiveAlways | NSTrackingInVisibleRect)
            owner:self
            userInfo:nil];
        [self addTrackingArea:_menuTrackingArea];
    }
    return self;
}

- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    if (_menuTrackingArea) {
        [self removeTrackingArea:_menuTrackingArea];
    }
    _menuTrackingArea = [[NSTrackingArea alloc]
        initWithRect:self.bounds
        options:(NSTrackingMouseMoved | NSTrackingMouseEnteredAndExited |
                 NSTrackingActiveAlways | NSTrackingInVisibleRect)
        owner:self
        userInfo:nil];
    [self addTrackingArea:_menuTrackingArea];
}

- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }
- (BOOL)acceptsFirstMouse:(NSEvent *)event { return YES; }
- (BOOL)isOpaque { return NO; }

- (NSView *)hitTest:(NSPoint)point {
    return self;  // Always return self — we handle ALL events
}

- (void)mouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> menu = SimpleHandler::GetSettingsMenuBrowser();
    if (menu) {
        menu->GetHost()->SetFocus(true);
        menu->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
        menu->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
    }
}

- (void)mouseUp:(NSEvent *)event {
    // mouseUp handled in mouseDown (combined down+up)
}

- (void)mouseDragged:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = EVENTFLAG_LEFT_MOUSE_BUTTON;

    CefRefPtr<CefBrowser> menu = SimpleHandler::GetSettingsMenuBrowser();
    if (menu) {
        menu->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)rightMouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> menu = SimpleHandler::GetSettingsMenuBrowser();
    if (menu) {
        menu->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
        menu->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
    }
}

- (void)scrollWheel:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> menu = SimpleHandler::GetSettingsMenuBrowser();
    if (menu) {
        int deltaX = (int)([event scrollingDeltaX] * 2);
        int deltaY = (int)([event scrollingDeltaY] * 2);
        menu->GetHost()->SendMouseWheelEvent(mouse_event, deltaX, deltaY);
    }
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> menu = SimpleHandler::GetSettingsMenuBrowser();
    if (menu) {
        menu->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)mouseEntered:(NSEvent *)event {
    CefRefPtr<CefBrowser> menu = SimpleHandler::GetSettingsMenuBrowser();
    if (menu) {
        NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
        CefMouseEvent mouse_event;
        mouse_event.x = (int)location.x;
        mouse_event.y = (int)(self.bounds.size.height - location.y);
        mouse_event.modifiers = 0;
        menu->GetHost()->SendMouseMoveEvent(mouse_event, false);
    }
}

- (void)mouseExited:(NSEvent *)event {
    CefRefPtr<CefBrowser> menu = SimpleHandler::GetSettingsMenuBrowser();
    if (menu) {
        CefMouseEvent mouse_event;
        mouse_event.x = -1;
        mouse_event.y = -1;
        mouse_event.modifiers = 0;
        menu->GetHost()->SendMouseMoveEvent(mouse_event, true);
    }
}

- (void)keyDown:(NSEvent *)event {
    CefRefPtr<CefBrowser> menu = SimpleHandler::GetSettingsMenuBrowser();
    if (!menu) return;

    NSString* chars = [event characters];
    NSEventModifierFlags flags = [event modifierFlags];
    int modifiers = 0;
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;

    CefKeyEvent key_event;
    key_event.type = KEYEVENT_RAWKEYDOWN;
    key_event.native_key_code = [event keyCode];
    if (chars.length > 0) key_event.character = [chars characterAtIndex:0];
    key_event.modifiers = modifiers;
    menu->GetHost()->SendKeyEvent(key_event);

    if (chars.length > 0) {
        key_event.type = KEYEVENT_CHAR;
        key_event.character = [chars characterAtIndex:0];
        key_event.unmodified_character = [chars characterAtIndex:0];
        menu->GetHost()->SendKeyEvent(key_event);
    }
}

- (void)keyUp:(NSEvent *)event {
    CefRefPtr<CefBrowser> menu = SimpleHandler::GetSettingsMenuBrowser();
    if (!menu) return;

    CefKeyEvent key_event;
    key_event.type = KEYEVENT_KEYUP;
    key_event.native_key_code = [event keyCode];
    NSString* chars = [event characters];
    if (chars.length > 0) key_event.character = [chars characterAtIndex:0];

    NSEventModifierFlags flags = [event modifierFlags];
    int modifiers = 0;
    if (flags & NSEventModifierFlagShift) modifiers |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagControl) modifiers |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagOption) modifiers |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) modifiers |= EVENTFLAG_COMMAND_DOWN;
    key_event.modifiers = modifiers;
    menu->GetHost()->SendKeyEvent(key_event);
}

@end

// Custom NSWindow for Settings Menu overlay — borderless windows refuse to become key by default
@interface SettingsMenuOverlayWindow : NSWindow
@end

@implementation SettingsMenuOverlayWindow
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }

// CRITICAL: NSWindow's internal dispatch does NOT forward mouse events to content view
// in borderless+transparent+OSR windows. We must dispatch manually.
- (void)sendEvent:(NSEvent *)event {
    NSEventType type = [event type];
    NSView* view = [self contentView];

    switch (type) {
        case NSEventTypeLeftMouseDown:
            [view mouseDown:event];
            return;
        case NSEventTypeLeftMouseUp:
            [view mouseUp:event];
            return;
        case NSEventTypeLeftMouseDragged:
            [view mouseDragged:event];
            return;
        case NSEventTypeRightMouseDown:
            [view rightMouseDown:event];
            return;
        case NSEventTypeRightMouseUp:
            [view rightMouseUp:event];
            return;
        case NSEventTypeMouseMoved:
            [view mouseMoved:event];
            return;
        case NSEventTypeScrollWheel:
            [view scrollWheel:event];
            return;
        case NSEventTypeMouseEntered:
            [view mouseEntered:event];
            return;
        case NSEventTypeMouseExited:
            [view mouseExited:event];
            return;
        case NSEventTypeKeyDown:
            [view keyDown:event];
            return;
        case NSEventTypeKeyUp:
            [view keyUp:event];
            return;
        default:
            [super sendEvent:event];
            return;
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
    NSRect mainFrame = [g_main_window frame];

    // Settings overlay: 450x450 right-side popup, right edge under icon
    if (g_settings_overlay_window && [g_settings_overlay_window isVisible]) {
        CGFloat pw = 450, ph = 450;
        CGFloat ox = mainFrame.origin.x + mainFrame.size.width - g_mac_settings_icon_right_offset - pw;
        CGFloat oy = mainFrame.origin.y + mainFrame.size.height - ph - 104;
        [g_settings_overlay_window setFrame:NSMakeRect(ox, oy, pw, ph) display:YES];
    }

    // Wallet overlay: full-window
    if (g_wallet_overlay_window && [g_wallet_overlay_window isVisible]) {
        [g_wallet_overlay_window setFrame:mainFrame display:YES];
    }

    if (g_backup_overlay_window && [g_backup_overlay_window isVisible]) {
        [g_backup_overlay_window setFrame:mainFrame display:YES];
    }

    if (g_brc100_auth_overlay_window && [g_brc100_auth_overlay_window isVisible]) {
        [g_brc100_auth_overlay_window setFrame:mainFrame display:YES];
    }
}

- (void)windowDidResize:(NSNotification *)notification {
    LOG_DEBUG("🔄 Main window resized - updating layout");

    NSRect contentRect = [[g_main_window contentView] bounds];
    int headerHeight = 96;
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

    // Settings overlay: 450x450 right-side popup, right edge under icon
    if (g_settings_overlay_window && [g_settings_overlay_window isVisible]) {
        CGFloat pw = 450, ph = 450;
        CGFloat ox = mainFrame.origin.x + mainFrame.size.width - g_mac_settings_icon_right_offset - pw;
        CGFloat oy = mainFrame.origin.y + mainFrame.size.height - ph - 104;
        [g_settings_overlay_window setFrame:NSMakeRect(ox, oy, pw, ph) display:YES];
        CefRefPtr<CefBrowser> settings = SimpleHandler::GetSettingsBrowser();
        if (settings) settings->GetHost()->WasResized();
    }

    // Wallet overlay: full-window
    if (g_wallet_overlay_window && [g_wallet_overlay_window isVisible]) {
        [g_wallet_overlay_window setFrame:mainFrame display:YES];
        CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
        if (wallet) wallet->GetHost()->WasResized();
    }

    if (g_backup_overlay_window && [g_backup_overlay_window isVisible]) {
        [g_backup_overlay_window setFrame:mainFrame display:YES];
        CefRefPtr<CefBrowser> backup = SimpleHandler::GetBackupBrowser();
        if (backup) backup->GetHost()->WasResized();
    }

    if (g_brc100_auth_overlay_window && [g_brc100_auth_overlay_window isVisible]) {
        [g_brc100_auth_overlay_window setFrame:mainFrame display:YES];
        CefRefPtr<CefBrowser> auth = SimpleHandler::GetBRC100AuthBrowser();
        if (auth) auth->GetHost()->WasResized();
    }

}

- (BOOL)windowShouldClose:(NSWindow *)sender {
    int windowCount = WindowManager::GetInstance().GetWindowCount();

    if (windowCount <= 1) {
        LOG_INFO("❌ Last window close requested - shutting down application");
        ShutdownApplication();
        return YES;
    }

    // Not the last window — close tabs in window 0 and clean up
    LOG_INFO("❌ Window 0 close requested (" + std::to_string(windowCount - 1) + " other windows remain)");
    auto allTabs = TabManager::GetInstance().GetAllTabs();
    std::vector<int> tabsToClose;
    for (auto* tab : allTabs) {
        if (tab->window_id == 0) {
            tabsToClose.push_back(tab->id);
        }
    }
    for (int tabId : tabsToClose) {
        TabManager::GetInstance().CloseTab(tabId);
    }
    WindowManager::GetInstance().RemoveWindow(0);
    return YES;
}

- (void)windowWillClose:(NSNotification *)notification {
    LOG_INFO("❌ Main window will close");
}

- (void)windowDidResignKey:(NSNotification *)notification {
    // Overlay close-on-focus-loss is now handled by InstallAppFocusLossHandler()
    // in OverlayHelpers_mac.mm via NSApplicationDidResignActiveNotification.
    // This method is intentionally empty -- keeping it for documentation.
    LOG_DEBUG("Main window resigned key (overlay close handled by OverlayHelpers)");
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

extern "C" void SetOverlayIgnoresMouseEvents(void* window, bool ignores) {
    if (!window) return;
    NSWindow* overlayWindow = (__bridge NSWindow*)window;
    [overlayWindow setIgnoresMouseEvents:ignores];
    LOG_DEBUG("🪟 Overlay mouse events " + std::string(ignores ? "disabled" : "enabled"));
}

extern "C" void HideNotificationOverlayWindow() {
    if (g_notification_overlay_window) {
        [g_notification_overlay_window orderOut:nil];
        LOG_INFO("🔔 Notification overlay hidden (keep-alive)");
    }
}

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

// ============================================================================
// Main Window Creation
// ============================================================================

void CreateMainWindow() {
    LOG_INFO("🪟 Creating main browser window (macOS)");

    // Create application menu bar with Quit item (enables Cmd+Q)
    NSMenu* menuBar = [[NSMenu alloc] init];
    NSMenuItem* appMenuItem = [[NSMenuItem alloc] init];
    [menuBar addItem:appMenuItem];
    NSMenu* appMenu = [[NSMenu alloc] init];
    NSMenuItem* quitItem = [[NSMenuItem alloc] initWithTitle:@"Quit Hodos Browser"
                                                      action:@selector(terminate:)
                                               keyEquivalent:@"q"];
    [appMenu addItem:quitItem];
    [appMenuItem setSubmenu:appMenu];

    // Edit menu with standard text editing shortcuts (Cmd+A/C/V/X/Z).
    // Required on macOS: without this, Cmd+A bypasses the NSMenu action
    // dispatch pathway and goes through performKeyEquivalent/keyDown,
    // which can skip CEF's display invalidation for selection changes.
    // Menu items auto-validate against the first responder, so they are
    // automatically disabled when an overlay (OSR) view is focused and
    // Cmd+A falls through to the existing keyDown: forwarding path.
    NSMenuItem* editMenuItem = [[NSMenuItem alloc] init];
    [menuBar addItem:editMenuItem];
    NSMenu* editMenu = [[NSMenu alloc] initWithTitle:@"Edit"];
    [editMenu addItemWithTitle:@"Undo" action:@selector(undo:) keyEquivalent:@"z"];
    [editMenu addItemWithTitle:@"Redo" action:@selector(redo:) keyEquivalent:@"Z"];
    [editMenu addItem:[NSMenuItem separatorItem]];
    [editMenu addItemWithTitle:@"Cut" action:@selector(cut:) keyEquivalent:@"x"];
    [editMenu addItemWithTitle:@"Copy" action:@selector(copy:) keyEquivalent:@"c"];
    [editMenu addItemWithTitle:@"Paste" action:@selector(paste:) keyEquivalent:@"v"];
    [editMenu addItemWithTitle:@"Delete" action:@selector(delete:) keyEquivalent:@""];
    [editMenu addItem:[NSMenuItem separatorItem]];
    [editMenu addItemWithTitle:@"Select All" action:@selector(selectAll:) keyEquivalent:@"a"];
    [editMenuItem setSubmenu:editMenu];

    [NSApp setMainMenu:menuBar];

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
    int headerHeight = 96;         // Header with tabs (42px) + toolbar (54px)
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

void CreateSettingsOverlayWithSeparateProcess(int iconRightOffset) {
    LOG_INFO("Creating settings overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_settings_icon_right_offset = iconRightOffset;

    // Get main window frame for overlay alignment
    NSRect mainFrame = [g_main_window frame];

    // Right-side popup panel, right edge aligned under icon
    CGFloat panelWidth = 450;
    CGFloat panelHeight = 450;
    // Cocoa origin is bottom-left: position right edge under icon, offset from top by ~104px for header
    CGFloat overlayX = mainFrame.origin.x + mainFrame.size.width - iconRightOffset - panelWidth;
    CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - 104;
    NSRect panelFrame = NSMakeRect(overlayX, overlayY, panelWidth, panelHeight);

    LOG_INFO("📐 Settings panel: (" + std::to_string((int)overlayX) + ", " + std::to_string((int)overlayY)
             + ") " + std::to_string((int)panelWidth) + "x" + std::to_string((int)panelHeight));

    // Destroy existing overlay if present
    if (g_settings_overlay_window) {
        LOG_INFO("🔄 Destroying existing settings overlay");
        [g_settings_overlay_window close];
        g_settings_overlay_window = nullptr;
    }

    // Create borderless, transparent, floating window
    g_settings_overlay_window = [[NSWindow alloc]
        initWithContentRect:panelFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_settings_overlay_window) {
        LOG_ERROR("❌ Failed to create settings overlay window");
        return;
    }

    [g_settings_overlay_window setOpaque:NO];
    [g_settings_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_settings_overlay_window setLevel:NSNormalWindowLevel];
    [g_settings_overlay_window setIgnoresMouseEvents:NO];
    [g_settings_overlay_window setReleasedWhenClosed:NO];
    [g_settings_overlay_window setHasShadow:NO];

    // Make this a child window of the main window
    [g_main_window addChildWindow:g_settings_overlay_window ordered:NSWindowAbove];

    // Create custom view for event handling and rendering
    SettingsOverlayView* contentView = [[SettingsOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, panelWidth, panelHeight)];
    [g_settings_overlay_window setContentView:contentView];

    // Create CEF browser with windowless rendering
    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("settings"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView,
                                   (int)panelWidth,
                                   (int)panelHeight);
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

// ============================================================================
// Cookie Panel (Privacy Shield) Overlay
// ============================================================================

// Click-outside monitor for cookie panel overlay
static id g_cookie_panel_click_monitor = nil;
// Timestamp of last hide — used to debounce toggle vs click-outside race
static CFAbsoluteTime g_cookie_panel_last_hide_time = 0;

// Click-outside monitor for settings menu overlay
static id g_settings_menu_click_monitor = nil;
// Timestamp of last hide — used to debounce toggle vs click-outside race
static CFAbsoluteTime g_settings_menu_last_hide_time = 0;

// Forward declarations for monitor helpers
static void RemoveCookiePanelClickOutsideMonitor();

void HideCookiePanelOverlay() {
    if (g_cookie_panel_overlay_window) {
        [g_cookie_panel_overlay_window orderOut:nil];
        RemoveCookiePanelClickOutsideMonitor();
        g_cookie_panel_last_hide_time = CFAbsoluteTimeGetCurrent();
        LOG_INFO("Cookie panel overlay hidden (macOS)");
    }
}

static void InstallCookiePanelClickOutsideMonitor() {
    if (g_cookie_panel_click_monitor) return;  // Already installed

    g_cookie_panel_click_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (!g_cookie_panel_overlay_window || ![g_cookie_panel_overlay_window isVisible]) {
                return event;
            }

            // Check if click is inside the cookie panel overlay
            NSPoint screenLocation = [NSEvent mouseLocation];
            NSRect overlayFrame = [g_cookie_panel_overlay_window frame];
            if (!NSPointInRect(screenLocation, overlayFrame)) {
                // Click outside — hide the overlay
                HideCookiePanelOverlay();
            }
            return event;
        }];
    LOG_INFO("Cookie panel click-outside monitor installed");
}

static void RemoveCookiePanelClickOutsideMonitor() {
    if (g_cookie_panel_click_monitor) {
        [NSEvent removeMonitor:g_cookie_panel_click_monitor];
        g_cookie_panel_click_monitor = nil;
        LOG_INFO("Cookie panel click-outside monitor removed");
    }
}

bool IsCookiePanelOverlayVisible() {
    return g_cookie_panel_overlay_window && [g_cookie_panel_overlay_window isVisible];
}

// Returns true if the overlay was hidden very recently (within 300ms)
// Used by the IPC toggle to avoid show-after-click-outside race
bool WasCookiePanelJustHidden() {
    CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
    return (now - g_cookie_panel_last_hide_time) < 0.3;
}

void ShowCookiePanelOverlay(int iconRightOffset) {
    if (g_cookie_panel_overlay_window) {
        g_mac_cookie_panel_icon_right_offset = iconRightOffset;

        // Reposition based on current main window frame and icon offset
        NSRect mainFrame = [g_main_window frame];
        CGFloat panelWidth = 400;
        CGFloat panelHeight = 500;
        CGFloat overlayX = mainFrame.origin.x + mainFrame.size.width - iconRightOffset - panelWidth;
        CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - 104;
        NSRect panelFrame = NSMakeRect(overlayX, overlayY, panelWidth, panelHeight);

        [g_cookie_panel_overlay_window setFrame:panelFrame display:YES];
        [g_cookie_panel_overlay_window makeKeyAndOrderFront:nil];
        InstallCookiePanelClickOutsideMonitor();
        LOG_INFO("Cookie panel overlay shown (macOS)");
    }
}

void CreateCookiePanelOverlayWithSeparateProcess(int iconRightOffset) {
    LOG_INFO("Creating cookie panel overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_cookie_panel_icon_right_offset = iconRightOffset;

    // Get main window frame for overlay alignment
    NSRect mainFrame = [g_main_window frame];

    // Right-side popup panel, right edge aligned under icon
    CGFloat panelWidth = 400;
    CGFloat panelHeight = 500;
    // Cocoa origin is bottom-left: position right edge under icon, offset from top by ~104px for header
    CGFloat overlayX = mainFrame.origin.x + mainFrame.size.width - iconRightOffset - panelWidth;
    CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - 104;
    NSRect panelFrame = NSMakeRect(overlayX, overlayY, panelWidth, panelHeight);

    LOG_INFO("Cookie panel: (" + std::to_string((int)overlayX) + ", " + std::to_string((int)overlayY)
             + ") " + std::to_string((int)panelWidth) + "x" + std::to_string((int)panelHeight));

    // Destroy existing overlay if present
    if (g_cookie_panel_overlay_window) {
        LOG_INFO("Destroying existing cookie panel overlay");
        [g_cookie_panel_overlay_window close];
        g_cookie_panel_overlay_window = nullptr;
    }

    // Create borderless, transparent, floating window
    g_cookie_panel_overlay_window = [[NSWindow alloc]
        initWithContentRect:panelFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_cookie_panel_overlay_window) {
        LOG_ERROR("Failed to create cookie panel overlay window");
        return;
    }

    [g_cookie_panel_overlay_window setOpaque:NO];
    [g_cookie_panel_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_cookie_panel_overlay_window setLevel:NSNormalWindowLevel];
    [g_cookie_panel_overlay_window setIgnoresMouseEvents:NO];
    [g_cookie_panel_overlay_window setReleasedWhenClosed:NO];
    [g_cookie_panel_overlay_window setHasShadow:NO];

    // Make this a child window of the main window
    [g_main_window addChildWindow:g_cookie_panel_overlay_window ordered:NSWindowAbove];

    // Create custom view for event handling and rendering
    CookiePanelOverlayView* contentView = [[CookiePanelOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, panelWidth, panelHeight)];
    [g_cookie_panel_overlay_window setContentView:contentView];

    // Create CEF browser with windowless rendering
    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("cookiepanel"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView,
                                   (int)panelWidth,
                                   (int)panelHeight);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        handler,
        "http://127.0.0.1:5137/privacy-shield",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("Failed to create cookie panel overlay CEF browser");
        return;
    }

    [g_cookie_panel_overlay_window makeKeyAndOrderFront:nil];
    InstallCookiePanelClickOutsideMonitor();
    LOG_INFO("Cookie panel overlay created successfully");
}

void CreateWalletOverlayWithSeparateProcess(int iconRightOffset) {
    LOG_INFO("Creating wallet overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_wallet_icon_right_offset = iconRightOffset;

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
    [g_wallet_overlay_window setAcceptsMouseMovedEvents:YES];
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

    std::string walletUrl = "http://127.0.0.1:5137/wallet-panel?iro=" + std::to_string(iconRightOffset);
    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        handler,
        walletUrl,
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("Failed to create wallet overlay CEF browser");
        return;
    }

    // NOTE: Do NOT call [g_main_window resignKeyWindow] here.
    // resignKeyWindow triggers MainWindowDelegate::windowDidResignKey synchronously,
    // which would destroy the wallet overlay that was just created (self-destruction loop).
    // makeKeyAndOrderFront already handles focus transfer for floating windows.

    [g_wallet_overlay_window makeKeyAndOrderFront:nil];

    // Make content view first responder for keyboard events
    [g_wallet_overlay_window makeFirstResponder:contentView];

    NSLog(@"🔍 Wallet overlay is key window: %d", [g_wallet_overlay_window isKeyWindow]);
    NSLog(@"🔍 First responder: %@", [g_wallet_overlay_window firstResponder]);

    // Global event monitor - captures ALL mouse events in the app
    static id walletGlobalMonitor = nil;
    if (walletGlobalMonitor) {
        [NSEvent removeMonitor:walletGlobalMonitor];
    }
    NSString* monLogPath = [[NSSearchPathForDirectoriesInDomains(NSApplicationSupportDirectory, NSUserDomainMask, YES) firstObject]
                            stringByAppendingPathComponent:@"HodosBrowser/wallet_events.log"];
    std::string monLogPathStr = [monLogPath UTF8String];

    walletGlobalMonitor = [NSEvent addLocalMonitorForEventsMatchingMask:
        (NSEventMaskLeftMouseDown | NSEventMaskLeftMouseUp | NSEventMaskMouseMoved |
         NSEventMaskMouseEntered | NSEventMaskMouseExited)
        handler:^NSEvent*(NSEvent* event) {
            std::ofstream dbg(monLogPathStr, std::ios::app);
            NSWindow* w = [event window];
            dbg << "GLOBAL MONITOR: type=" << (int)[event type]
                << " window=" << (void*)w
                << " isWallet=" << (w == g_wallet_overlay_window ? 1 : 0)
                << " isMain=" << (w == g_main_window ? 1 : 0)
                << " at (" << [event locationInWindow].x << "," << [event locationInWindow].y << ")"
                << std::endl;
            dbg.close();
            return event;
        }];

    LOG_INFO("✅ Wallet overlay created successfully (with global event monitor)");
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

void CreateNotificationOverlay(const std::string& type, const std::string& domain, const std::string& extraParams) {
    LOG_INFO("🔔 Creating notification overlay (type: " + type + ", domain: " + domain + ") (macOS)");

    NSRect mainFrame = [g_main_window frame];

    // Build URL with query parameters
    std::string queryString = "type=" + type + "&domain=" + domain;
    if (!extraParams.empty()) queryString += extraParams;
    std::string url = "http://127.0.0.1:5137/brc100-auth?" + queryString;

    // Keep-alive: if window and browser already exist, use JS injection (instant)
    CefRefPtr<CefBrowser> existing = SimpleHandler::GetNotificationBrowser();
    if (g_notification_overlay_window && existing) {
        LOG_INFO("🔔 Reusing existing notification overlay (keep-alive, JS injection)");

        // Resize to match current main window
        [g_notification_overlay_window setFrame:mainFrame display:YES];

        if (type != "preload") {
            // Escape single quotes in the query string for JS
            std::string safeQuery = queryString;
            size_t pos = 0;
            while ((pos = safeQuery.find('\'', pos)) != std::string::npos) {
                safeQuery.replace(pos, 1, "\\'");
                pos += 2;
            }

            std::string js = "if(window.showNotification){window.showNotification('" + safeQuery + "')}else{window.location.search='?" + safeQuery + "'}";
            existing->GetMainFrame()->ExecuteJavaScript(js, "", 0);
        }

        existing->GetHost()->WasResized();
        [g_notification_overlay_window makeKeyAndOrderFront:nil];
        return;
    }

    // First time or stale: clean up and create fresh
    if (g_notification_overlay_window) {
        CefRefPtr<CefBrowser> old_browser = SimpleHandler::GetNotificationBrowser();
        if (old_browser) {
            old_browser->GetHost()->CloseBrowser(false);
        }
        [g_notification_overlay_window close];
        g_notification_overlay_window = nullptr;
    }

    g_notification_overlay_window = [[NSWindow alloc]
        initWithContentRect:mainFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_notification_overlay_window) {
        LOG_ERROR("❌ Failed to create notification overlay window");
        return;
    }

    [g_notification_overlay_window setOpaque:NO];
    [g_notification_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_notification_overlay_window setLevel:NSNormalWindowLevel];
    [g_notification_overlay_window setIgnoresMouseEvents:NO];
    [g_notification_overlay_window setReleasedWhenClosed:NO];
    [g_notification_overlay_window setHasShadow:NO];

    [g_main_window addChildWindow:g_notification_overlay_window ordered:NSWindowAbove];

    NotificationOverlayView* contentView = [[NotificationOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, mainFrame.size.width, mainFrame.size.height)];
    [g_notification_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("notification"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView,
                                   (int)mainFrame.size.width,
                                   (int)mainFrame.size.height);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        handler,
        url,
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("❌ Failed to create notification overlay CEF browser");
        return;
    }

    if (type == "preload") {
        [g_notification_overlay_window orderOut:nil];
        LOG_INFO("🔔 Notification overlay pre-created (hidden)");
    } else {
        [g_notification_overlay_window makeKeyAndOrderFront:nil];
    }

    LOG_INFO("✅ Notification overlay created successfully");
}

// ============================================================================
// Settings Menu Click-Outside Detection
// ============================================================================

// Forward declaration for monitor helper
static void RemoveSettingsMenuClickOutsideMonitor();

void HideSettingsMenuOverlay() {
    if (g_settings_menu_overlay_window) {
        [g_settings_menu_overlay_window orderOut:nil];
        RemoveSettingsMenuClickOutsideMonitor();
        g_settings_menu_last_hide_time = CFAbsoluteTimeGetCurrent();
        LOG_INFO("Settings menu overlay hidden (macOS)");
    }
}

static void InstallSettingsMenuClickOutsideMonitor() {
    if (g_settings_menu_click_monitor) return;  // Already installed

    g_settings_menu_click_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (!g_settings_menu_overlay_window || ![g_settings_menu_overlay_window isVisible]) {
                return event;
            }

            // Check if click is inside the settings menu overlay
            NSPoint screenLocation = [NSEvent mouseLocation];
            NSRect overlayFrame = [g_settings_menu_overlay_window frame];
            if (!NSPointInRect(screenLocation, overlayFrame)) {
                // Click outside — hide the overlay
                HideSettingsMenuOverlay();
            }
            return event;
        }];
    LOG_INFO("Settings menu click-outside monitor installed");
}

static void RemoveSettingsMenuClickOutsideMonitor() {
    if (g_settings_menu_click_monitor) {
        [NSEvent removeMonitor:g_settings_menu_click_monitor];
        g_settings_menu_click_monitor = nil;
        LOG_INFO("Settings menu click-outside monitor removed");
    }
}

bool IsSettingsMenuOverlayVisible() {
    return g_settings_menu_overlay_window && [g_settings_menu_overlay_window isVisible];
}

// Returns true if the overlay was hidden very recently (within 300ms)
// Used by the IPC toggle to avoid show-after-click-outside race
bool WasSettingsMenuJustHidden() {
    CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
    return (now - g_settings_menu_last_hide_time) < 0.3;
}

void ShowSettingsMenuOverlay() {
    if (g_settings_menu_overlay_window) {
        [g_settings_menu_overlay_window makeKeyAndOrderFront:nil];
        InstallSettingsMenuClickOutsideMonitor();
        LOG_INFO("Settings menu overlay shown (macOS)");
    }
}

// ============================================================================
// Settings Menu Overlay Creation
// ============================================================================

void CreateSettingsMenuOverlay() {
    LOG_INFO("🎨 Creating settings menu overlay (macOS)");

    // Settings menu is smaller (dropdown from settings button)
    // Position in top-right corner
    NSRect mainFrame = [g_main_window frame];
    int menuWidth = 300;
    int menuHeight = 480;
    NSRect menuFrame = NSMakeRect(mainFrame.origin.x + mainFrame.size.width - menuWidth - 20,
                                  mainFrame.origin.y + mainFrame.size.height - menuHeight - 60,
                                  menuWidth, menuHeight);

    if (g_settings_menu_overlay_window) {
        LOG_INFO("🔄 Destroying existing settings menu overlay");
        [g_settings_menu_overlay_window close];
        g_settings_menu_overlay_window = nullptr;
    }

    g_settings_menu_overlay_window = [[SettingsMenuOverlayWindow alloc]
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
    [g_settings_menu_overlay_window setHasShadow:YES];
    [g_settings_menu_overlay_window setAcceptsMouseMovedEvents:YES];

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
        "http://127.0.0.1:5137/menu",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("❌ Failed to create settings menu overlay CEF browser");
        return;
    }

    [g_settings_menu_overlay_window makeKeyAndOrderFront:nil];
    [g_settings_menu_overlay_window makeFirstResponder:contentView];
    InstallSettingsMenuClickOutsideMonitor();
    LOG_INFO("✅ Settings menu overlay created successfully");
}

// ============================================================================
// Omnibox Overlay (macOS)
// ============================================================================

static void RemoveOmniboxClickOutsideMonitor() {
    if (g_omnibox_click_monitor) {
        [NSEvent removeMonitor:g_omnibox_click_monitor];
        g_omnibox_click_monitor = nil;
    }
}

static void InstallOmniboxClickOutsideMonitor() {
    if (g_omnibox_click_monitor) return;

    g_omnibox_click_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (!g_omnibox_overlay_window || ![g_omnibox_overlay_window isVisible]) {
                return event;
            }
            NSPoint screenLocation = [NSEvent mouseLocation];
            NSRect overlayFrame = [g_omnibox_overlay_window frame];
            if (!NSPointInRect(screenLocation, overlayFrame)) {
                [g_omnibox_overlay_window orderOut:nil];
                RemoveOmniboxClickOutsideMonitor();
                g_omnibox_last_hide_time = CFAbsoluteTimeGetCurrent();
            }
            return event;
        }];
}

void HideOmniboxOverlayMacOS() {
    if (g_omnibox_overlay_window) {
        [g_omnibox_overlay_window orderOut:nil];
        RemoveOmniboxClickOutsideMonitor();
        g_omnibox_last_hide_time = CFAbsoluteTimeGetCurrent();
        LOG_INFO("Omnibox overlay hidden (macOS)");
    }
}

bool IsOmniboxOverlayVisible() {
    return g_omnibox_overlay_window && [g_omnibox_overlay_window isVisible];
}

bool OmniboxOverlayExists() {
    return g_omnibox_overlay_window != nullptr;
}

bool WasOmniboxJustHidden() {
    CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
    return (now - g_omnibox_last_hide_time) < 0.3;
}

void ShowOmniboxOverlayMacOS() {
    if (g_omnibox_overlay_window) {
        // Reposition in case window moved/resized since creation
        NSRect mainFrame = [g_main_window frame];
        int omniboxWidth = (int)(mainFrame.size.width * 0.6);
        if (omniboxWidth < 400) omniboxWidth = 400;
        int omniboxHeight = 420;
        CGFloat overlayX = mainFrame.origin.x + (mainFrame.size.width - omniboxWidth) / 2;
        CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - omniboxHeight - 104;
        [g_omnibox_overlay_window setFrame:NSMakeRect(overlayX, overlayY, omniboxWidth, omniboxHeight) display:YES];

        [g_omnibox_overlay_window orderFront:nil];
        InstallOmniboxClickOutsideMonitor();
        LOG_INFO("Omnibox overlay shown (macOS)");
    }
}

void CreateOmniboxOverlayMacOS() {
    LOG_INFO("Creating omnibox overlay (macOS)");

    NSRect mainFrame = [g_main_window frame];
    // Position below header (99px) spanning most of the window width
    int omniboxWidth = (int)(mainFrame.size.width * 0.6);
    if (omniboxWidth < 400) omniboxWidth = 400;
    int omniboxHeight = 420;
    // Center horizontally, position below header
    CGFloat overlayX = mainFrame.origin.x + (mainFrame.size.width - omniboxWidth) / 2;
    CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - omniboxHeight - 104;
    NSRect omniboxFrame = NSMakeRect(overlayX, overlayY, omniboxWidth, omniboxHeight);

    // Keep-alive: don't destroy existing window
    if (g_omnibox_overlay_window) {
        ShowOmniboxOverlayMacOS();
        return;
    }

    g_omnibox_overlay_window = [[DropdownOverlayWindow alloc]
        initWithContentRect:omniboxFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_omnibox_overlay_window) {
        LOG_ERROR("Failed to create omnibox overlay window");
        return;
    }

    [g_omnibox_overlay_window setOpaque:NO];
    [g_omnibox_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_omnibox_overlay_window setLevel:NSNormalWindowLevel];
    [g_omnibox_overlay_window setIgnoresMouseEvents:NO];
    [g_omnibox_overlay_window setReleasedWhenClosed:NO];
    [g_omnibox_overlay_window setHasShadow:YES];
    [g_omnibox_overlay_window setAcceptsMouseMovedEvents:YES];

    // CRITICAL: Make child of main window so it doesn't steal focus from address bar
    [g_main_window addChildWindow:g_omnibox_overlay_window ordered:NSWindowAbove];

    DropdownOverlayView* contentView = [[DropdownOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, omniboxWidth, omniboxHeight)];
    contentView.browserAccessor = ^CefRefPtr<CefBrowser>{ return SimpleHandler::GetOmniboxBrowser(); };
    [g_omnibox_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("omnibox"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView, omniboxWidth, omniboxHeight);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info, handler,
        "http://127.0.0.1:5137/omnibox",
        settings, nullptr, CefRequestContext::GetGlobalContext());

    if (!result) {
        LOG_ERROR("Failed to create omnibox overlay CEF browser");
        return;
    }

    // Don't steal focus — keyboard must stay with the header browser (address bar)
    [g_omnibox_overlay_window orderFront:nil];
    InstallOmniboxClickOutsideMonitor();
    LOG_INFO("Omnibox overlay created successfully");
}

// ============================================================================
// Download Panel Overlay (macOS)
// ============================================================================

static void RemoveDownloadPanelClickOutsideMonitor() {
    if (g_download_panel_click_monitor) {
        [NSEvent removeMonitor:g_download_panel_click_monitor];
        g_download_panel_click_monitor = nil;
    }
}

static void InstallDownloadPanelClickOutsideMonitor() {
    if (g_download_panel_click_monitor) return;

    g_download_panel_click_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (!g_download_panel_overlay_window || ![g_download_panel_overlay_window isVisible]) {
                return event;
            }
            NSPoint screenLocation = [NSEvent mouseLocation];
            NSRect overlayFrame = [g_download_panel_overlay_window frame];
            if (!NSPointInRect(screenLocation, overlayFrame)) {
                [g_download_panel_overlay_window orderOut:nil];
                RemoveDownloadPanelClickOutsideMonitor();
                g_download_panel_last_hide_time = CFAbsoluteTimeGetCurrent();
            }
            return event;
        }];
}

void HideDownloadPanelOverlayMacOS() {
    if (g_download_panel_overlay_window) {
        [g_download_panel_overlay_window orderOut:nil];
        RemoveDownloadPanelClickOutsideMonitor();
        g_download_panel_last_hide_time = CFAbsoluteTimeGetCurrent();
        LOG_INFO("Download panel overlay hidden (macOS)");
    }
}

bool IsDownloadPanelOverlayVisible() {
    return g_download_panel_overlay_window && [g_download_panel_overlay_window isVisible];
}

bool WasDownloadPanelJustHidden() {
    CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
    return (now - g_download_panel_last_hide_time) < 0.3;
}

void ShowDownloadPanelOverlayMacOS(int iconRightOffset) {
    if (g_download_panel_overlay_window) {
        g_mac_download_panel_icon_right_offset = iconRightOffset;

        NSRect mainFrame = [g_main_window frame];
        CGFloat panelWidth = 400;
        CGFloat panelHeight = 500;
        CGFloat overlayX = mainFrame.origin.x + mainFrame.size.width - iconRightOffset - panelWidth;
        CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - 104;
        [g_download_panel_overlay_window setFrame:NSMakeRect(overlayX, overlayY, panelWidth, panelHeight) display:YES];

        [g_download_panel_overlay_window makeKeyAndOrderFront:nil];
        InstallDownloadPanelClickOutsideMonitor();
        LOG_INFO("Download panel overlay shown (macOS)");
    }
}

void CreateDownloadPanelOverlayMacOS(int iconRightOffset) {
    LOG_INFO("Creating download panel overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_download_panel_icon_right_offset = iconRightOffset;

    NSRect mainFrame = [g_main_window frame];
    CGFloat panelWidth = 400;
    CGFloat panelHeight = 500;
    CGFloat overlayX = mainFrame.origin.x + mainFrame.size.width - iconRightOffset - panelWidth;
    CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - 104;
    NSRect panelFrame = NSMakeRect(overlayX, overlayY, panelWidth, panelHeight);

    if (g_download_panel_overlay_window) {
        [g_download_panel_overlay_window close];
        g_download_panel_overlay_window = nullptr;
    }

    g_download_panel_overlay_window = [[DropdownOverlayWindow alloc]
        initWithContentRect:panelFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_download_panel_overlay_window) {
        LOG_ERROR("Failed to create download panel overlay window");
        return;
    }

    [g_download_panel_overlay_window setOpaque:NO];
    [g_download_panel_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_download_panel_overlay_window setLevel:NSPopUpMenuWindowLevel];
    [g_download_panel_overlay_window setIgnoresMouseEvents:NO];
    [g_download_panel_overlay_window setReleasedWhenClosed:NO];
    [g_download_panel_overlay_window setHasShadow:YES];
    [g_download_panel_overlay_window setAcceptsMouseMovedEvents:YES];

    DropdownOverlayView* contentView = [[DropdownOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, panelWidth, panelHeight)];
    contentView.browserAccessor = ^CefRefPtr<CefBrowser>{ return SimpleHandler::GetDownloadPanelBrowser(); };
    [g_download_panel_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("downloadpanel"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView, (int)panelWidth, (int)panelHeight);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info, handler,
        "http://127.0.0.1:5137/downloads",
        settings, nullptr, CefRequestContext::GetGlobalContext());

    if (!result) {
        LOG_ERROR("Failed to create download panel overlay CEF browser");
        return;
    }

    [g_download_panel_overlay_window makeKeyAndOrderFront:nil];
    [g_download_panel_overlay_window makeFirstResponder:contentView];
    InstallDownloadPanelClickOutsideMonitor();
    LOG_INFO("Download panel overlay created successfully");
}

// ============================================================================
// Profile Panel Overlay (macOS)
// ============================================================================

static void RemoveProfilePanelClickOutsideMonitor() {
    if (g_profile_panel_click_monitor) {
        [NSEvent removeMonitor:g_profile_panel_click_monitor];
        g_profile_panel_click_monitor = nil;
    }
}

static void InstallProfilePanelClickOutsideMonitor() {
    if (g_profile_panel_click_monitor) return;

    g_profile_panel_click_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (!g_profile_panel_overlay_window || ![g_profile_panel_overlay_window isVisible]) {
                return event;
            }
            NSPoint screenLocation = [NSEvent mouseLocation];
            NSRect overlayFrame = [g_profile_panel_overlay_window frame];
            if (!NSPointInRect(screenLocation, overlayFrame)) {
                [g_profile_panel_overlay_window orderOut:nil];
                RemoveProfilePanelClickOutsideMonitor();
                g_profile_panel_last_hide_time = CFAbsoluteTimeGetCurrent();
            }
            return event;
        }];
}

void HideProfilePanelOverlayMacOS() {
    if (g_profile_panel_overlay_window) {
        [g_profile_panel_overlay_window orderOut:nil];
        RemoveProfilePanelClickOutsideMonitor();
        g_profile_panel_last_hide_time = CFAbsoluteTimeGetCurrent();
        LOG_INFO("Profile panel overlay hidden (macOS)");
    }
}

bool IsProfilePanelOverlayVisible() {
    return g_profile_panel_overlay_window && [g_profile_panel_overlay_window isVisible];
}

bool WasProfilePanelJustHidden() {
    CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
    return (now - g_profile_panel_last_hide_time) < 0.3;
}

void ShowProfilePanelOverlayMacOS(int iconRightOffset) {
    if (g_profile_panel_overlay_window) {
        g_mac_profile_panel_icon_right_offset = iconRightOffset;

        NSRect mainFrame = [g_main_window frame];
        CGFloat panelWidth = 300;
        CGFloat panelHeight = 400;
        CGFloat overlayX = mainFrame.origin.x + mainFrame.size.width - iconRightOffset - panelWidth;
        CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - 104;
        [g_profile_panel_overlay_window setFrame:NSMakeRect(overlayX, overlayY, panelWidth, panelHeight) display:YES];

        [g_profile_panel_overlay_window makeKeyAndOrderFront:nil];
        InstallProfilePanelClickOutsideMonitor();
        LOG_INFO("Profile panel overlay shown (macOS)");
    }
}

void CreateProfilePanelOverlayMacOS(int iconRightOffset) {
    LOG_INFO("Creating profile panel overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_profile_panel_icon_right_offset = iconRightOffset;

    NSRect mainFrame = [g_main_window frame];
    CGFloat panelWidth = 300;
    CGFloat panelHeight = 400;
    CGFloat overlayX = mainFrame.origin.x + mainFrame.size.width - iconRightOffset - panelWidth;
    CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - 104;
    NSRect panelFrame = NSMakeRect(overlayX, overlayY, panelWidth, panelHeight);

    if (g_profile_panel_overlay_window) {
        [g_profile_panel_overlay_window close];
        g_profile_panel_overlay_window = nullptr;
    }

    g_profile_panel_overlay_window = [[DropdownOverlayWindow alloc]
        initWithContentRect:panelFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_profile_panel_overlay_window) {
        LOG_ERROR("Failed to create profile panel overlay window");
        return;
    }

    [g_profile_panel_overlay_window setOpaque:NO];
    [g_profile_panel_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_profile_panel_overlay_window setLevel:NSPopUpMenuWindowLevel];
    [g_profile_panel_overlay_window setIgnoresMouseEvents:NO];
    [g_profile_panel_overlay_window setReleasedWhenClosed:NO];
    [g_profile_panel_overlay_window setHasShadow:YES];
    [g_profile_panel_overlay_window setAcceptsMouseMovedEvents:YES];

    DropdownOverlayView* contentView = [[DropdownOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, panelWidth, panelHeight)];
    contentView.browserAccessor = ^CefRefPtr<CefBrowser>{ return SimpleHandler::GetProfilePanelBrowser(); };
    [g_profile_panel_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("profilepanel"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView, (int)panelWidth, (int)panelHeight);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info, handler,
        "http://127.0.0.1:5137/profile-picker",
        settings, nullptr, CefRequestContext::GetGlobalContext());

    if (!result) {
        LOG_ERROR("Failed to create profile panel overlay CEF browser");
        return;
    }

    [g_profile_panel_overlay_window makeKeyAndOrderFront:nil];
    [g_profile_panel_overlay_window makeFirstResponder:contentView];
    InstallProfilePanelClickOutsideMonitor();
    LOG_INFO("Profile panel overlay created successfully");
}

// ============================================================================
// Hide Application (Cmd+H)
// ============================================================================

void HideApplication() {
    dispatch_async(dispatch_get_main_queue(), ^{
        [NSApp hide:nil];
    });
}

// ============================================================================
// Quit Confirmation and Shutdown
// ============================================================================

void ShowQuitConfirmationAndShutdown() {
    dispatch_async(dispatch_get_main_queue(), ^{
        // Check if there are multiple tabs open
        auto allTabs = TabManager::GetInstance().GetAllTabs();
        if (allTabs.size() > 1) {
            NSAlert* alert = [[NSAlert alloc] init];
            [alert setMessageText:@"Quit HodosBrowser?"];
            [alert setInformativeText:[NSString stringWithFormat:@"You have %lu tabs open. Are you sure you want to quit?", (unsigned long)allTabs.size()]];
            [alert addButtonWithTitle:@"Quit"];
            [alert addButtonWithTitle:@"Cancel"];
            [alert setAlertStyle:NSAlertStyleWarning];
            if ([alert runModal] == NSAlertFirstButtonReturn) {
                ShutdownApplication();
            }
        } else {
            ShutdownApplication();
        }
    });
}

// ============================================================================
// Graceful Shutdown
// ============================================================================

void ShutdownApplication() {
    // Guard against re-entrant calls (terminate: → ShutdownApplication → terminate:)
    static bool shutting_down = false;
    if (shutting_down) return;
    shutting_down = true;

    LOG_INFO("🛑 Starting graceful application shutdown (macOS)...");

    // TODO: Save session when macOS tab system is implemented
    // (Windows implementation: SaveSession() in cef_browser_shell.cpp)

    // TODO: Clear browsing data on exit (PS3) when macOS tab system is implemented
    // (Windows implementation: ClearBrowsingDataOnExit() in cef_browser_shell.cpp)

    // Step 1: Close all CEF browsers
    LOG_INFO("🔄 Closing CEF browsers...");

    CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
    CefRefPtr<CefBrowser> webview_browser = SimpleHandler::GetWebviewBrowser();
    CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
    CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
    CefRefPtr<CefBrowser> backup_browser = SimpleHandler::GetBackupBrowser();
    CefRefPtr<CefBrowser> brc100_auth_browser = SimpleHandler::GetBRC100AuthBrowser();
    CefRefPtr<CefBrowser> settings_menu_browser = SimpleHandler::GetSettingsMenuBrowser();
    CefRefPtr<CefBrowser> cookie_panel_browser = SimpleHandler::GetCookiePanelBrowser();

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

    // Close menu overlay browser
    if (g_menu_overlay_browser_ref && g_menu_overlay_browser_ref->browser) {
        LOG_INFO("Closing menu overlay browser...");
        g_menu_overlay_browser_ref->browser->GetHost()->CloseBrowser(false);
    }

    if (cookie_panel_browser) {
        LOG_INFO("🔄 Closing cookie panel browser...");
        cookie_panel_browser->GetHost()->CloseBrowser(false);
    }

    CefRefPtr<CefBrowser> omnibox_browser = SimpleHandler::GetOmniboxBrowser();
    CefRefPtr<CefBrowser> download_panel_browser = SimpleHandler::GetDownloadPanelBrowser();
    CefRefPtr<CefBrowser> profile_panel_browser = SimpleHandler::GetProfilePanelBrowser();

    if (omnibox_browser) {
        LOG_INFO("Closing omnibox browser...");
        omnibox_browser->GetHost()->CloseBrowser(false);
    }
    if (download_panel_browser) {
        LOG_INFO("Closing download panel browser...");
        download_panel_browser->GetHost()->CloseBrowser(false);
    }
    if (profile_panel_browser) {
        LOG_INFO("Closing profile panel browser...");
        profile_panel_browser->GetHost()->CloseBrowser(false);
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

    if (g_menu_overlay_window || g_menu_overlay_browser_ref || g_menu_overlay_render_handler) {
        LOG_INFO("Closing menu overlay window...");
        DestroyMenuOverlayWindow(false);
    }

    if (g_cookie_panel_overlay_window) {
        LOG_INFO("🔄 Closing cookie panel overlay window...");
        [g_cookie_panel_overlay_window close];
        g_cookie_panel_overlay_window = nullptr;
    }

    if (g_omnibox_overlay_window) {
        [g_omnibox_overlay_window close];
        g_omnibox_overlay_window = nullptr;
    }
    if (g_download_panel_overlay_window) {
        [g_download_panel_overlay_window close];
        g_download_panel_overlay_window = nullptr;
    }
    if (g_profile_panel_overlay_window) {
        [g_profile_panel_overlay_window close];
        g_profile_panel_overlay_window = nullptr;
    }

    // Step 3: Close main window
    if (g_main_window) {
        LOG_INFO("🔄 Closing main window...");
        [g_main_window close];
        g_main_window = nullptr;
    }

    // Step 4: Kill background server processes
    if (g_wallet_server_pid > 0) {
        LOG_INFO("🔄 Killing wallet server (pid " + std::to_string(g_wallet_server_pid) + ")...");
        kill(g_wallet_server_pid, SIGTERM);
    }
    if (g_adblock_server_pid > 0) {
        LOG_INFO("🔄 Killing adblock server (pid " + std::to_string(g_adblock_server_pid) + ")...");
        kill(g_adblock_server_pid, SIGTERM);
    }

    LOG_INFO("✅ Application shutdown complete (macOS)");
    Logger::Shutdown();

    // Quit the CEF message loop, then exit the process.
    // Do NOT call [NSApp terminate:nil] — that re-enters our terminate: override.
    CefQuitMessageLoop();

    // Post a delayed exit in case CefQuitMessageLoop doesn't fully tear down
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(1.0 * NSEC_PER_SEC)),
                   dispatch_get_main_queue(), ^{
        _exit(0);
    });
}

// ============================================================================
// Health Check Functions (macOS) — uses SyncHttpClient (libcurl)
// ============================================================================

// Returns true if wallet server at localhost:31301 responds with "ok"
static bool QuickHealthCheck() {
    HttpResponse resp = SyncHttpClient::Get("http://localhost:31301/health", 2000);
    return resp.success && resp.body.find("\"ok\"") != std::string::npos;
}

// Returns true if adblock engine at localhost:31302 responds with "ready"
static bool QuickAdblockHealthCheck() {
    HttpResponse resp = SyncHttpClient::Get("http://localhost:31302/health", 2000);
    return resp.success && resp.body.find("\"ready\"") != std::string::npos;
}

// Send POST /shutdown to a localhost service for graceful shutdown
static bool SendShutdownRequest(int port) {
    std::string url = "http://localhost:" + std::to_string(port) + "/shutdown";
    HttpResponse resp = SyncHttpClient::Post(url, "", "application/json", 2000);
    return resp.success;
}

// ============================================================================
// Server Process Management (macOS)
// ============================================================================

#include <spawn.h>
#include <signal.h>
#include <sys/wait.h>

extern char **environ;

static void StartWalletServer() {
    // Check if already running (dev mode: cargo run separately)
    if (QuickHealthCheck()) {
        LOG_INFO("Wallet server already running (dev mode) - skipping launch");
        g_walletServerRunning = true;
        return;
    }

    // Resolve exe path relative to browser executable
    char exec_path[1024];
    uint32_t exec_path_size = sizeof(exec_path);
    if (_NSGetExecutablePath(exec_path, &exec_path_size) != 0) {
        LOG_WARNING("Failed to get executable path for wallet server resolution");
        return;
    }

    std::string exeDir(exec_path);
    size_t lastSlash = exeDir.find_last_of('/');
    if (lastSlash != std::string::npos) {
        exeDir = exeDir.substr(0, lastSlash);
    }

    // Try relative path from build dir: ../../../../../../rust-wallet/target/release/hodos-wallet
    std::string walletExe = exeDir + "/../../../../../../rust-wallet/target/release/hodos-wallet";

    // Check if file exists
    if (access(walletExe.c_str(), X_OK) != 0) {
        // Try alternate path for development
        walletExe = exeDir + "/../../../../../rust-wallet/target/release/hodos-wallet";
        if (access(walletExe.c_str(), X_OK) != 0) {
            LOG_WARNING("Wallet server executable not found - browser will run without auto-launched wallet");
            LOG_WARNING("Start wallet manually: cd rust-wallet && cargo run --release");
            return;
        }
    }

    LOG_INFO("Launching wallet server: " + walletExe);

    char* argv[] = { const_cast<char*>(walletExe.c_str()), nullptr };
    int status = posix_spawn(&g_wallet_server_pid, walletExe.c_str(), nullptr, nullptr, argv, environ);
    if (status != 0) {
        LOG_ERROR("Failed to launch wallet server: posix_spawn error " + std::to_string(status));
        return;
    }

    LOG_INFO("Wallet server launched with PID: " + std::to_string(g_wallet_server_pid));

    // Wait for it to become healthy (up to 10 seconds)
    for (int i = 0; i < 20; i++) {
        usleep(500000); // 500ms
        if (QuickHealthCheck()) {
            g_walletServerRunning = true;
            LOG_INFO("Wallet server is healthy");
            return;
        }
    }
    LOG_WARNING("Wallet server launched but health check timed out");
}

static void StartAdblockServer() {
    if (QuickAdblockHealthCheck()) {
        LOG_INFO("Adblock engine already running (dev mode) - skipping launch");
        g_adblockServerRunning = true;
        return;
    }

    char exec_path[1024];
    uint32_t exec_path_size = sizeof(exec_path);
    if (_NSGetExecutablePath(exec_path, &exec_path_size) != 0) return;

    std::string exeDir(exec_path);
    size_t lastSlash = exeDir.find_last_of('/');
    if (lastSlash != std::string::npos) exeDir = exeDir.substr(0, lastSlash);

    std::string adblockExe = exeDir + "/../../../../../../adblock-engine/target/release/hodos-adblock";
    if (access(adblockExe.c_str(), X_OK) != 0) {
        adblockExe = exeDir + "/../../../../../adblock-engine/target/release/hodos-adblock";
        if (access(adblockExe.c_str(), X_OK) != 0) {
            LOG_WARNING("Adblock engine not found - browser will run without ad blocking");
            return;
        }
    }

    LOG_INFO("Launching adblock engine: " + adblockExe);
    char* argv[] = { const_cast<char*>(adblockExe.c_str()), nullptr };
    int status = posix_spawn(&g_adblock_server_pid, adblockExe.c_str(), nullptr, nullptr, argv, environ);
    if (status != 0) {
        LOG_ERROR("Failed to launch adblock engine: posix_spawn error " + std::to_string(status));
        return;
    }

    LOG_INFO("Adblock engine launched with PID: " + std::to_string(g_adblock_server_pid));

    for (int i = 0; i < 20; i++) {
        usleep(500000);
        if (QuickAdblockHealthCheck()) {
            g_adblockServerRunning = true;
            LOG_INFO("Adblock engine is healthy");
            return;
        }
    }
    LOG_WARNING("Adblock engine launched but health check timed out");
}

static void StopServers() {
    // Graceful shutdown via HTTP
    if (g_walletServerRunning) {
        SendShutdownRequest(31301);
    }
    if (g_adblockServerRunning) {
        SendShutdownRequest(31302);
    }

    // Wait briefly for graceful shutdown
    usleep(1000000); // 1 second

    // Force kill if still running
    if (g_wallet_server_pid > 0) {
        int status;
        pid_t result = waitpid(g_wallet_server_pid, &status, WNOHANG);
        if (result == 0) {
            // Still running — send SIGTERM
            kill(g_wallet_server_pid, SIGTERM);
            usleep(500000);
            waitpid(g_wallet_server_pid, &status, WNOHANG);
        }
        g_wallet_server_pid = -1;
    }

    if (g_adblock_server_pid > 0) {
        int status;
        pid_t result = waitpid(g_adblock_server_pid, &status, WNOHANG);
        if (result == 0) {
            kill(g_adblock_server_pid, SIGTERM);
            usleep(500000);
            waitpid(g_adblock_server_pid, &status, WNOHANG);
        }
        g_adblock_server_pid = -1;
    }
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

        // Initialize centralized logger with absolute path
        // (CWD is '/' when launched via 'open', so relative paths fail)
        {
            NSString* appSupDir = [NSSearchPathForDirectoriesInDomains(NSApplicationSupportDirectory, NSUserDomainMask, YES) firstObject];
            NSString* logDir = [appSupDir stringByAppendingPathComponent:@"HodosBrowser"];
            std::string logPath = std::string([logDir UTF8String]) + "/debug_output.log";
            Logger::Initialize(ProcessType::MAIN, logPath);
        }
        LOG_INFO("=== NEW SESSION STARTED (macOS) ===");
        LOG_INFO("🍎 HodosBrowser Shell starting on macOS...");

        // Configure CEF settings
        CefSettings settings;
        settings.command_line_args_disabled = false;
        CefString(&settings.log_file).FromASCII("debug.log");
        settings.log_severity = LOGSEVERITY_INFO;
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

        std::string user_data_path = [hodosBrowserDir UTF8String];
        // NOTE: root_cache_path is set AFTER profile resolution below (must be unique per instance)

        // Initialize ProfileManager BEFORE CefInitialize so cache_path is correct
        LOG_INFO("Initializing ProfileManager...");
        if (!ProfileManager::GetInstance().Initialize(user_data_path)) {
            LOG_ERROR("Failed to initialize ProfileManager");
        }

        // Parse --profile argument from command line
        // macOS: use argc/argv from main_args
        std::string profileId = "Default";
        NSArray* arguments = [[NSProcessInfo processInfo] arguments];
        for (NSString* arg in arguments) {
            std::string argStr = [arg UTF8String];
            if (argStr.find("--profile=") == 0) {
                profileId = argStr.substr(10);
                // Remove quotes if present
                if (!profileId.empty() && profileId.front() == '"') profileId = profileId.substr(1);
                if (!profileId.empty() && profileId.back() == '"') profileId.pop_back();
                break;
            }
        }
        ProfileManager::GetInstance().SetCurrentProfileId(profileId);
        LOG_INFO("Using profile: " + profileId);

        // Get profile-specific data directory
        std::string profile_cache = ProfileManager::GetInstance().GetCurrentProfileDataPath();
        LOG_INFO("Profile data path: " + profile_cache);

        // Acquire exclusive lock on profile directory (prevents SQLite corruption)
        if (!AcquireProfileLock(profile_cache)) {
            NSAlert* alert = [[NSAlert alloc] init];
            [alert setMessageText:@"Profile Locked"];
            NSString* infoText = [NSString stringWithFormat:
                @"Profile \"%s\" is already in use by another instance.\n\nClose the other instance first, or launch with a different profile.",
                profileId.c_str()];
            [alert setInformativeText:infoText];
            [alert setAlertStyle:NSAlertStyleCritical];
            [alert runModal];
            return 1;
        }
        LOG_INFO("Profile lock acquired");

        // Initialize SettingsManager with profile-specific path
        SettingsManager::GetInstance().Initialize(profile_cache);
        LOG_INFO("Settings loaded for profile: " + profileId);

        // Internal UI pages also live on 127.0.0.1. Chromium persists zoom by
        // host, so a user zoom on an internal tab can otherwise scale the header
        // and overlay chrome across restarts.
        ClearPersistedInternalFrontendZoom(profile_cache);

        // Initialize AdblockCache with profile path (loads per-site settings)
        AdblockCache::GetInstance().Initialize(profile_cache);
        AdblockCache::GetInstance().SetGlobalEnabled(
            SettingsManager::GetInstance().GetPrivacySettings().adBlockEnabled);
        LOG_INFO("AdblockCache initialized");

        // Initialize fingerprint protection session token
        FingerprintProtection::GetInstance().Initialize();
        LOG_INFO("Fingerprint protection initialized");

        // Initialize CookieBlockManager
        if (CookieBlockManager::GetInstance().Initialize(profile_cache)) {
            LOG_INFO("CookieBlockManager initialized");
        } else {
            LOG_WARNING("CookieBlockManager initialization failed");
        }

        // Initialize BookmarkManager
        BookmarkManager::GetInstance().Initialize(profile_cache);
        LOG_INFO("BookmarkManager initialized");

        // Set root_cache_path AND cache_path to profile-specific directory.
        // CRITICAL: root_cache_path must be unique per CEF instance — two instances
        // sharing the same root_cache_path will cause CefInitialize to fail.
        std::string cache_path = profile_cache;
        CefString(&settings.root_cache_path).FromString(cache_path);
        CefString(&settings.cache_path).FromString(cache_path);

        // Remote debugging port: 9222 for Default profile, disabled for others
        // (avoids port conflict when multiple instances run simultaneously)
        settings.remote_debugging_port = (profileId == "Default") ? 9222 : 0;

        LOG_INFO("Cache path: " + cache_path);
        LOG_INFO("Root cache path: " + cache_path);
        LOG_INFO("Remote debugging port: " + std::to_string(settings.remote_debugging_port));

        // Enable JavaScript features
        CefString(&settings.javascript_flags).FromASCII("--expose-gc");

        // Set subprocess path to helper bundle
        // On macOS, helpers are in Contents/Frameworks/HodosBrowser Helper.app/Contents/MacOS/HodosBrowser Helper
        NSString* appPath = [[NSBundle mainBundle] bundlePath];
        NSString* helperPath = [appPath stringByAppendingPathComponent:@"Contents/Frameworks/HodosBrowser Helper.app/Contents/MacOS/HodosBrowser Helper"];
        std::string helper_path = [helperPath UTF8String];
        CefString(&settings.browser_subprocess_path).FromString(helper_path);
        LOG_INFO("📁 Subprocess path: " + helper_path);

        // Initialize CEF first (before creating windows)
        LOG_INFO("🔄 Initializing CEF...");
        NSLog(@"🔄 About to call CefInitialize...");
        bool cef_success = CefInitialize(main_args, settings, app, nullptr);
        NSLog(@"✅ CefInitialize returned: %s", cef_success ? "true" : "false");
        LOG_INFO("CefInitialize result: " + std::string(cef_success ? "✅ SUCCESS" : "❌ FAILED"));

        if (!cef_success) {
            LOG_ERROR("❌ CEF initialization failed - exiting");
            return 1;
        }

        // Start backend services (wallet + adblock)
        NSLog(@"🔄 Starting backend services...");
        StartWalletServer();
        StartAdblockServer();
        NSLog(@"✅ Backend services started");

        // Create the primary BrowserWindow record (window 0) in WindowManager.
        // This MUST happen before any browser creation so SetBrowserForRole works.
        WindowManager::GetInstance().CreateWindowRecord();
        LOG_INFO("✅ WindowManager window 0 created");

        // Create windows after CEF is initialized
        // (Activation policy already set above for main process only)
        CreateMainWindow();

        // Install shared overlay focus-loss handler (closes dropdown overlays on Cmd+Tab)
        InstallAppFocusLossHandler();

        if (!g_main_window || !g_header_view || !g_webview_view) {
            LOG_ERROR("❌ Window creation failed - exiting");
            CefShutdown();
            return 1;
        }

        // Store window references for later use
        app->SetMacOSWindow((__bridge void*)g_main_window,
                           (__bridge void*)g_header_view,
                           (__bridge void*)g_webview_view);

        // Populate window 0's BrowserWindow struct for multi-window support
        BrowserWindow* bw0 = WindowManager::GetInstance().GetWindow(0);
        if (bw0) {
            bw0->ns_window = (__bridge void*)g_main_window;
            bw0->header_view = (__bridge void*)g_header_view;
            bw0->webview_view = (__bridge void*)g_webview_view;
            LOG_INFO("✅ BrowserWindow[0] populated with main window views");
        }

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
        LOG_INFO("CEF message loop exited - shutting down...");
        StopServers();
        ReleaseProfileLock();
        CefShutdown();

        LOG_INFO("✅ Application exited cleanly");
        Logger::Shutdown();

        return 0;
    }
}

// ============================================================================
// Menu Overlay (three-dot menu) -- uses GenericOverlayView infrastructure
// ============================================================================

// Called from simple_handler.cpp OnAfterCreated when the "menu" role browser is ready
void SetMenuOverlayBrowser(CefRefPtr<CefBrowser> browser) {
    if (g_menu_overlay_browser_ref) {
        g_menu_overlay_browser_ref->browser = browser;
        LOG_INFO("SetMenuOverlayBrowser: browser ref populated");
    } else {
        LOG_WARNING("SetMenuOverlayBrowser: overlay already closed before browser attach; closing browser");
        if (browser) {
            browser->GetHost()->CloseBrowser(false);
        }
    }
}

void CreateMenuOverlayMac(int iconRightOffset) {
    LOG_INFO("Creating menu overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));

    if (!g_main_window) {
        LOG_ERROR("Cannot create menu overlay: main window is null");
        return;
    }

    // Destroy existing menu overlay
    if (g_menu_overlay_window) {
        LOG_INFO("Destroying existing menu overlay");
        DestroyMenuOverlayWindow(true);
    }

    NSRect mainFrame = [g_main_window frame];
    CGFloat menuWidth = 280;
    CGFloat menuHeight = 450;

    // Position: below the toolbar, right edge aligned under the menu icon
    CGFloat overlayX = mainFrame.origin.x + mainFrame.size.width - iconRightOffset - menuWidth;
    // Cocoa: Y=0 is bottom. Header is ~99px from top of window.
    // Title bar is ~28px. So top of content = origin.y + height - 28 (approx).
    CGFloat overlayY = mainFrame.origin.y + mainFrame.size.height - menuHeight - 104;
    NSRect menuFrame = NSMakeRect(overlayX, overlayY, menuWidth, menuHeight);

    // Clamp to screen edges
    menuFrame = ClampOverlayToScreen(menuFrame);

    LOG_INFO("Menu overlay frame: (" + std::to_string((int)menuFrame.origin.x) + ", "
             + std::to_string((int)menuFrame.origin.y) + ") "
             + std::to_string((int)menuFrame.size.width) + "x"
             + std::to_string((int)menuFrame.size.height));

    // Create borderless floating window using GenericOverlayWindow
    g_menu_overlay_window = [[GenericOverlayWindow alloc]
        initWithContentRect:menuFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_menu_overlay_window) {
        LOG_ERROR("Failed to create menu overlay window");
        return;
    }

    [g_menu_overlay_window setOpaque:NO];
    [g_menu_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_menu_overlay_window setLevel:NSFloatingWindowLevel];
    [g_menu_overlay_window setIgnoresMouseEvents:NO];
    [g_menu_overlay_window setReleasedWhenClosed:NO];
    [g_menu_overlay_window setHasShadow:YES];

    // Child window of main window (moves/minimizes together)
    [g_main_window addChildWindow:g_menu_overlay_window ordered:NSWindowAbove];

    // Create GenericOverlayView as content view
    GenericOverlayView* contentView = [[GenericOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, menuWidth, menuHeight)];
    [g_menu_overlay_window setContentView:contentView];

    // Create CEF browser with OSR
    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("menu"));
    g_menu_overlay_render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView,
                                   (int)menuWidth, (int)menuHeight);
    handler->SetRenderHandler(g_menu_overlay_render_handler);

    std::string menuUrl = "http://127.0.0.1:5137/menu";
    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        handler,
        menuUrl,
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("Failed to create menu overlay CEF browser");
        [g_menu_overlay_window close];
        g_menu_overlay_window = nullptr;
        return;
    }

    // Allocate OverlayBrowserRef -- will be populated when OnAfterCreated fires
    // via SimpleHandler, which sets the browser by role "menu".
    g_menu_overlay_browser_ref = new OverlayBrowserRef();

    // Attach browser ref to the GenericOverlayView (will be populated async)
    [contentView attachBrowser:g_menu_overlay_browser_ref];

    // Show and install click-outside monitor
    [g_menu_overlay_window makeKeyAndOrderFront:nil];
    [g_menu_overlay_window makeFirstResponder:contentView];
    InstallClickOutsideMonitor(g_menu_overlay_window);

    LOG_INFO("Menu overlay created successfully (using GenericOverlayView)");
}

// C-linkage stubs called from simple_handler.cpp via extern
void CreateMenuOverlay(void* hInstance, bool showImmediately, int iconRightOffset) {
    // hInstance is Windows-only, ignored on macOS
    CreateMenuOverlayMac(iconRightOffset);
}

void ShowMenuOverlay(int iconRightOffset) {
    CreateMenuOverlayMac(iconRightOffset);
}

void HideMenuOverlay() {
    DestroyMenuOverlayWindow(true);
}
