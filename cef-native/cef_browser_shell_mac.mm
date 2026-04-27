// macOS Implementation of HodosBrowser Shell
// Uses Cocoa/AppKit for window management and CEF for browser rendering

#import <Cocoa/Cocoa.h>
#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <QuartzCore/QuartzCore.h>
#import <CoreImage/CoreImage.h>
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
#include "include/core/AppPaths.h"
#include "include/handlers/my_overlay_render_handler.h"
#include "include/wrapper/cef_library_loader.h"
#include "OverlayHelpers_mac.h"

#include <atomic>
#include <regex>
#include <dlfcn.h>
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
// NSApplicationDelegate — menu-bar-style keep-alive (Chromium macOS pattern)
//
// Mirrors chrome/browser/app_controller_mac.mm:
//   - applicationShouldHandleReopen:hasVisibleWindows: creates a new window
//     when the user clicks the dock icon with no windows open.
//   - applicationDockMenu: offers "New Window" in the dock right-click menu,
//     routed through newWindowFromDock: which activates first (matches
//     AppController::commandFromDock:).
//   - newWindow:/newTab: are the File-menu item targets (Cmd+N / Cmd+T when
//     no CEF browser has focus; when a browser has focus, these shortcuts
//     are still handled by SimpleHandler::OnPreKeyEvent).
// ============================================================================

@interface HodosAppDelegate : NSObject <NSApplicationDelegate>
@end

@implementation HodosAppDelegate

- (instancetype)init {
  self = [super init];
  if (self) {
    Logger::Log("✅ HodosAppDelegate initialized", 1, 0);
  }
  return self;
}

- (BOOL)applicationShouldHandleReopen:(NSApplication*)sender
                    hasVisibleWindows:(BOOL)hasVisibleWindows {
  Logger::Log(std::string("🔁 applicationShouldHandleReopen:hasVisibleWindows:") +
              (hasVisibleWindows ? "YES" : "NO"), 1, 0);
  if (!hasVisibleWindows) {
    Logger::Log("🔁 Dock reopen with no visible windows — creating new browser window", 1, 0);
    WindowManager::GetInstance().CreateFullWindow(/*createInitialTab=*/true);
  }
  return NO;
}

- (NSMenu*)applicationDockMenu:(NSApplication*)sender {
  fprintf(stderr, "🗂  applicationDockMenu: ENTRY (thread=%s, sender=%p)\n",
          [NSThread isMainThread] ? "main" : "bg", sender);
  Logger::Log("🗂  applicationDockMenu: called — building dock menu", 1, 0);
  NSMenu* dockMenu = [[NSMenu alloc] init];
  // Disable auto-validation: otherwise AppKit consults validateMenuItem:
  // on the target chain, and in the dock-menu context nothing in the
  // responder chain returns YES for a custom selector, so the item is
  // rendered disabled (gray) and may be suppressed.
  [dockMenu setAutoenablesItems:NO];

  NSMenuItem* newWindowItem =
      [[NSMenuItem alloc] initWithTitle:@"New Window"
                                 action:@selector(newWindowFromDock:)
                          keyEquivalent:@""];
  [newWindowItem setTarget:self];
  [newWindowItem setEnabled:YES];
  [dockMenu addItem:newWindowItem];
  return dockMenu;
}

- (void)newWindow:(id)sender {
  Logger::Log("🪟 File→New Window (Cmd+N via menu) — creating new browser window", 1, 0);
  WindowManager::GetInstance().CreateFullWindow(/*createInitialTab=*/true);
}

- (void)newWindowFromDock:(id)sender {
  // Activate first so the new window comes to the foreground (Chromium pattern:
  // AppController::commandFromDock: calls activateIgnoringOtherApps: before
  // dispatching the command).
  [NSApp activateIgnoringOtherApps:YES];
  [self newWindow:sender];
}

- (void)newTab:(id)sender {
  BrowserWindow* activeWin = WindowManager::GetInstance().GetActiveWindow();
  if (!activeWin || !activeWin->webview_view) {
    // No active window — open a new full window instead (same as Chrome's
    // Cmd+T with no window falling through to IDC_NEW_WINDOW).
    Logger::Log("🗂  File→New Tab with no active window — creating new full window", 1, 0);
    WindowManager::GetInstance().CreateFullWindow(/*createInitialTab=*/true);
    return;
  }

  NSView* parentView = (__bridge NSView*)activeWin->webview_view;
  NSRect b = [parentView bounds];
  Logger::Log("🗂  File→New Tab (Cmd+T via menu) — creating tab in window " +
              std::to_string(activeWin->window_id), 1, 0);
  TabManager::GetInstance().CreateTab(
      "http://127.0.0.1:5137/newtab",
      activeWin->webview_view,
      0, 0,
      (int)b.size.width, (int)b.size.height,
      activeWin->window_id);
  SimpleHandler::NotifyWindowTabListChanged(activeWin->window_id);
}

@end

// Strong reference so the delegate survives the app's lifetime.
static HodosAppDelegate* g_app_delegate = nil;

// ============================================================================
// Global Window References (macOS equivalents of Windows HWNDs)
// ============================================================================

NSWindow* g_main_window = nullptr;
NSView* g_header_view = nullptr;
NSView* g_webview_view = nullptr;

// Content fullscreen state (HTML5 Fullscreen API, e.g. YouTube video)
static bool g_content_fullscreen = false;
static NSRect g_pre_fullscreen_frame = NSZeroRect;
static id g_fullscreen_escape_monitor = nil;

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

// QR screen capture overlay
static NSWindow* g_qr_selection_window = nullptr;
static NSView*   g_qr_selection_view = nullptr;

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
std::atomic<bool> g_walletServerRunning{false};
std::atomic<bool> g_adblockServerRunning{false};
bool g_app_shutting_down = false;
bool g_header_browser_loaded = false;

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
// Uses presentation options to cover the screen (like Chrome's "tab fullscreen")
// rather than a native macOS Space transition via toggleFullScreen.
void HandleFullscreenChange(bool fullscreen) {
    LOG_INFO("HandleFullscreenChange: " + std::string(fullscreen ? "ENTER" : "EXIT"));

    dispatch_async(dispatch_get_main_queue(), ^{
        if (!g_main_window || !g_header_view || !g_webview_view) return;

        if (fullscreen) {
            g_pre_fullscreen_frame = [g_main_window frame];
            g_content_fullscreen = true;

            [g_header_view setHidden:YES];

            NSRect contentRect = [[g_main_window contentView] bounds];
            [g_webview_view setFrame:contentRect];

            auto* activeTab = TabManager::GetInstance().GetActiveTab();
            if (activeTab && activeTab->browser) {
                activeTab->browser->GetHost()->WasResized();
            }

            [NSApp setPresentationOptions:
                NSApplicationPresentationAutoHideMenuBar |
                NSApplicationPresentationAutoHideDock];

            NSRect screenFrame = [[g_main_window screen] frame];
            [g_main_window setFrame:screenFrame display:YES animate:YES];

            // Catch Escape at the NSEvent level — macOS presentation
            // options can swallow keys before they reach the CEF view.
            if (g_fullscreen_escape_monitor) {
                [NSEvent removeMonitor:g_fullscreen_escape_monitor];
            }
            g_fullscreen_escape_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:^NSEvent* (NSEvent* event) {
                if ([event keyCode] == 53) {  // 53 = kVK_Escape
                    auto* tab = TabManager::GetInstance().GetActiveTab();
                    if (tab && tab->browser && tab->browser->GetMainFrame()) {
                        tab->browser->GetMainFrame()->ExecuteJavaScript(
                            "document.exitFullscreen()", "", 0);
                    }
                    return nil;  // consume
                }
                return event;
            }];
        } else {
            g_content_fullscreen = false;

            if (g_fullscreen_escape_monitor) {
                [NSEvent removeMonitor:g_fullscreen_escape_monitor];
                g_fullscreen_escape_monitor = nil;
            }

            [NSApp setPresentationOptions:NSApplicationPresentationDefault];

            [g_header_view setHidden:NO];

            if (!NSIsEmptyRect(g_pre_fullscreen_frame)) {
                [g_main_window setFrame:g_pre_fullscreen_frame display:YES animate:YES];
            }

            NSRect contentRect = [[g_main_window contentView] bounds];
            int headerHeight = 96;
            NSRect headerRect = NSMakeRect(0, contentRect.size.height - headerHeight,
                                           contentRect.size.width, headerHeight);
            [g_header_view setFrame:headerRect];

            NSRect webviewRect = NSMakeRect(0, 0, contentRect.size.width,
                                            contentRect.size.height - headerHeight);
            [g_webview_view setFrame:webviewRect];

            auto* activeTab = TabManager::GetInstance().GetActiveTab();
            if (activeTab && activeTab->browser) {
                activeTab->browser->GetHost()->WasResized();
            }

            CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
            if (header) {
                header->GetHost()->WasResized();
            }
        }
    });
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
        NSString* appDirName = [NSString stringWithUTF8String:AppPaths::GetAppDirName().c_str()];
        NSString* logRelPath = [appDirName stringByAppendingPathComponent:@"wallet_events.log"];
        hitLogPath = std::string([[appSup stringByAppendingPathComponent:logRelPath] UTF8String]);
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
        NSString* mdAppDir = [NSString stringWithUTF8String:AppPaths::GetAppDirName().c_str()];
        NSString* mdLogRel = [mdAppDir stringByAppendingPathComponent:@"wallet_events.log"];
        mdLogPath = std::string([[appSup stringByAppendingPathComponent:mdLogRel] UTF8String]);
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
// Borderless NSWindow returns NO for canBecomeKeyWindow by default,
// which prevents keyboard events from reaching the view. Override it.
@interface NotificationOverlayWindow : NSWindow
@end

@implementation NotificationOverlayWindow
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }

- (void)sendEvent:(NSEvent *)event {
    NSEventType type = [event type];
    NSView* view = [self contentView];

    switch (type) {
        case NSEventTypeLeftMouseDown:  [view mouseDown:event]; return;
        case NSEventTypeLeftMouseUp:    [view mouseUp:event]; return;
        case NSEventTypeLeftMouseDragged: [view mouseDragged:event]; return;
        case NSEventTypeRightMouseDown: [view rightMouseDown:event]; return;
        case NSEventTypeRightMouseUp:   [view rightMouseUp:event]; return;
        case NSEventTypeMouseMoved:     [view mouseMoved:event]; return;
        case NSEventTypeScrollWheel:    [view scrollWheel:event]; return;
        case NSEventTypeKeyDown:        [view keyDown:event]; return;
        case NSEventTypeKeyUp:          [view keyUp:event]; return;
        default: [super sendEvent:event]; return;
    }
}
@end

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

    // Settings overlay: flush right, flush below header
    if (g_settings_overlay_window && [g_settings_overlay_window isVisible]) {
        NSRect sf = CalculateToolbarOverlayFrame(g_main_window, 450, 450, 96);
        [g_settings_overlay_window setFrame:sf display:YES];
    }

    // Wallet is a child window — moves automatically

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

    if (g_content_fullscreen) {
        // In content fullscreen: webview fills entire content area, header stays hidden
        [g_webview_view setFrame:contentRect];
    } else {
        int headerHeight = 96;
        int webviewHeight = contentRect.size.height - headerHeight;

        NSRect headerRect = NSMakeRect(0, contentRect.size.height - headerHeight,
                                       contentRect.size.width, headerHeight);
        [g_header_view setFrame:headerRect];

        NSRect webviewRect = NSMakeRect(0, 0, contentRect.size.width, webviewHeight);
        [g_webview_view setFrame:webviewRect];
    }

    // Notify CEF browsers of resize
    CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
    if (header) {
        header->GetHost()->WasResized();
        LOG_DEBUG("🔄 Header browser notified of resize");
    }

    // Legacy "webview" browser no longer exists (Bug #9 fix) — tab browsers
    // resize automatically via NSViewWidthSizable/Height autoresizing mask on
    // their host NSViews (see TabManager_mac.mm).
    CefRefPtr<CefBrowser> webview = SimpleHandler::GetWebviewBrowser();
    if (webview) {
        webview->GetHost()->WasResized();
        LOG_DEBUG("🔄 Webview browser notified of resize");
    }

    // Resize and notify overlay windows
    NSRect mainFrame = [g_main_window frame];

    // Settings overlay: flush right, flush below header
    if (g_settings_overlay_window && [g_settings_overlay_window isVisible]) {
        NSRect sf = CalculateToolbarOverlayFrame(g_main_window, 450, 450, 96);
        [g_settings_overlay_window setFrame:sf display:YES];
        CefRefPtr<CefBrowser> settings = SimpleHandler::GetSettingsBrowser();
        if (settings) settings->GetHost()->WasResized();
    }

    // Wallet is a child window — moves automatically, no resize needed for fixed-size panel

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

- (void)windowDidBecomeKey:(NSNotification *)notification {
    WindowManager::GetInstance().SetActiveWindowId(0);
}

- (BOOL)windowShouldClose:(NSWindow *)sender {
    // Close only this window's tabs and remove the window record. Do NOT call
    // ShutdownApplication() here — on macOS we follow the Chromium convention
    // (see chrome/browser/app_controller_mac.mm ScopedKeepAlive) where closing
    // the last browser window leaves the process alive. Real app quit routes
    // through HodosBrowserApplication::terminate: (Cmd-Q, menu Quit, dock Quit).
    LOG_INFO("❌ Window 0 close requested");
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
        // Restore focus to main window after overlay hides
        if (g_main_window) {
            [g_main_window makeKeyAndOrderFront:nil];
        }
        LOG_INFO("🔔 Notification overlay hidden (keep-alive), focus restored to main window");
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

    // File menu — New Window (Cmd+N), New Tab (Cmd+T). Targets the
    // NSApplicationDelegate (HodosAppDelegate) directly so these shortcuts
    // work even when no CEF browser has focus (e.g. after the last window
    // closes but the process is still alive). When a browser IS focused,
    // AppKit menu-key-equivalent dispatch runs first and preempts
    // SimpleHandler::OnPreKeyEvent's duplicate handling of Cmd+N/T —
    // intentional, matches Chromium's macOS behaviour.
    NSMenuItem* fileMenuItem = [[NSMenuItem alloc] init];
    [menuBar addItem:fileMenuItem];
    NSMenu* fileMenu = [[NSMenu alloc] initWithTitle:@"File"];
    NSMenuItem* newWindowItem =
        [[NSMenuItem alloc] initWithTitle:@"New Window"
                                   action:@selector(newWindow:)
                            keyEquivalent:@"n"];
    [newWindowItem setTarget:g_app_delegate];
    [fileMenu addItem:newWindowItem];
    NSMenuItem* newTabItem =
        [[NSMenuItem alloc] initWithTitle:@"New Tab"
                                   action:@selector(newTab:)
                            keyEquivalent:@"t"];
    [newTabItem setTarget:g_app_delegate];
    [fileMenu addItem:newTabItem];
    [fileMenuItem setSubmenu:fileMenu];

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

    // Create main window. NSWindowStyleMaskFullSizeContentView lets our
    // React header render underneath the titlebar — combined with
    // titlebarAppearsTransparent + titleVisibility hidden, the grey titlebar
    // strip disappears visually while the native traffic-light buttons
    // continue to render on top at their default macOS position. Our React
    // TabBar reserves 86px of left padding for them (see TabBar.tsx isMac).
    g_main_window = [[NSWindow alloc]
        initWithContentRect:screenRect
        styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                  NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable |
                  NSWindowStyleMaskFullSizeContentView
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_main_window) {
        LOG_ERROR("❌ Failed to create main window");
        return;
    }

    [g_main_window setTitle:@"Hodos Browser"];
    [g_main_window setTitlebarAppearsTransparent:YES];
    [g_main_window setTitleVisibility:NSWindowTitleHidden];
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

    CGFloat panelWidth = 450;
    CGFloat panelHeight = 450;
    NSRect panelFrame = CalculateToolbarOverlayFrame(g_main_window, panelWidth, panelHeight, 96);

    LOG_INFO("📐 Settings panel: (" + std::to_string((int)panelFrame.origin.x) + ", " + std::to_string((int)panelFrame.origin.y)
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
    [g_settings_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];

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

        // Reposition: flush right, flush below header
        NSRect panelFrame = CalculateToolbarOverlayFrame(g_main_window, 400, 500, 96);
        [g_cookie_panel_overlay_window setFrame:panelFrame display:YES];
        [g_cookie_panel_overlay_window makeKeyAndOrderFront:nil];
        InstallCookiePanelClickOutsideMonitor();
        LOG_INFO("Cookie panel overlay shown (macOS)");
    }
}

void CreateCookiePanelOverlayWithSeparateProcess(int iconRightOffset) {
    LOG_INFO("Creating cookie panel overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_cookie_panel_icon_right_offset = iconRightOffset;

    CGFloat panelWidth = 400;
    CGFloat panelHeight = 500;
    NSRect panelFrame = CalculateToolbarOverlayFrame(g_main_window, panelWidth, panelHeight, 96);

    LOG_INFO("Cookie panel: (" + std::to_string((int)panelFrame.origin.x) + ", " + std::to_string((int)panelFrame.origin.y)
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
    [g_cookie_panel_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];

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

void CloseWalletOverlay() {
    if (!g_wallet_overlay_window) return;

    LOG_INFO("Closing wallet overlay (click-outside)");
    RemoveClickOutsideMonitor(g_wallet_overlay_window);

    CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
    if (wallet_browser) {
        wallet_browser->GetHost()->CloseBrowser(false);
    }

    if (g_main_window) {
        [g_main_window removeChildWindow:g_wallet_overlay_window];
    }
    [g_wallet_overlay_window orderOut:nil];
    [g_wallet_overlay_window close];
    g_wallet_overlay_window = nullptr;
}

void CreateWalletOverlayWithSeparateProcess(int iconRightOffset) {
    LOG_INFO("Creating wallet overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));

    if (!g_main_window || ![g_main_window isVisible] || [g_main_window frame].size.width < 100) {
        LOG_WARNING("Wallet overlay skipped — main window not ready (visible=" +
            std::string(g_main_window && [g_main_window isVisible] ? "yes" : "no") +
            " width=" + std::to_string(g_main_window ? (int)[g_main_window frame].size.width : 0) + ")");
        return;
    }

    g_mac_wallet_icon_right_offset = iconRightOffset;

    // Position: fixed-width panel, flush right, flush below header, full remaining height
    CGFloat walletWidth = 400;
    NSRect contentScreen = [g_main_window convertRectToScreen:[[g_main_window contentView] frame]];
    CGFloat walletHeight = contentScreen.size.height - 96;
    NSRect walletFrame = CalculateToolbarOverlayFrame(g_main_window, walletWidth, walletHeight, 96);
    LOG_INFO("📐 Wallet overlay: " + std::to_string((int)walletFrame.size.width) + " x " + std::to_string((int)walletFrame.size.height));

    if (g_wallet_overlay_window) {
        LOG_INFO("🔄 Destroying existing wallet overlay");
        [g_wallet_overlay_window close];
        g_wallet_overlay_window = nullptr;
    }

    g_wallet_overlay_window = [[WalletOverlayWindow alloc]
        initWithContentRect:walletFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_wallet_overlay_window) {
        LOG_ERROR("❌ Failed to create wallet overlay window");
        return;
    }

    [g_wallet_overlay_window setOpaque:NO];
    [g_wallet_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_wallet_overlay_window setLevel:NSFloatingWindowLevel];
    [g_wallet_overlay_window setIgnoresMouseEvents:NO];
    [g_wallet_overlay_window setAcceptsMouseMovedEvents:YES];
    [g_wallet_overlay_window setReleasedWhenClosed:NO];
    [g_wallet_overlay_window setHasShadow:YES];
    [g_wallet_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];

    // Child window of main window (moves/minimizes together)
    [g_main_window addChildWindow:g_wallet_overlay_window ordered:NSWindowAbove];

    WalletOverlayView* contentView = [[WalletOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, walletFrame.size.width, walletFrame.size.height)];
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
                                   (int)walletFrame.size.width,
                                   (int)walletFrame.size.height);
    handler->SetRenderHandler(render_handler);

    // Pass pending PeerPay count/amount as query params so the React panel
    // can render the notification banner on first paint. Mirrors the Windows
    // code path in simple_app.cpp:777. g_peerpay_count/amount are set by the
    // toggle_wallet_panel IPC handler just before this function runs.
    std::string walletUrl = "http://127.0.0.1:5137/wallet-panel?iro=" + std::to_string(iconRightOffset);
    if (g_peerpay_count > 0) {
        walletUrl += "&ppc=" + std::to_string(g_peerpay_count) +
                     "&ppa=" + std::to_string(g_peerpay_amount);
    }
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
    [g_wallet_overlay_window makeFirstResponder:contentView];
    InstallClickOutsideMonitor(g_wallet_overlay_window);

    LOG_INFO("✅ Wallet overlay created successfully");
}

// ============================================================================
// QR Screen Capture — Phase 2 macOS
// ============================================================================

// Forward declarations for QR screen capture
void FinishQRScreenCaptureMacOS(bool cancelled, NSRect selection);

void HideWalletOverlay() {
    if (!g_wallet_overlay_window) return;
    LOG_INFO("Hiding wallet overlay (macOS)");
    RemoveClickOutsideMonitor(g_wallet_overlay_window);

    CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
    if (wallet_browser) {
        wallet_browser->GetMainFrame()->ExecuteJavaScript(
            "window.postMessage({type:'wallet_hidden'},'*');", "", 0);
        wallet_browser->GetHost()->SetFocus(false);
    }

    [g_wallet_overlay_window orderOut:nil];
}

void ShowWalletOverlay() {
    if (!g_wallet_overlay_window) return;
    LOG_INFO("Showing wallet overlay (macOS)");
    [g_wallet_overlay_window makeKeyAndOrderFront:nil];
    InstallClickOutsideMonitor(g_wallet_overlay_window);

    CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
    if (wallet_browser) {
        wallet_browser->GetHost()->SetFocus(true);
    }
}

// --- BSV pattern classification (mirrors QRScreenCapture.cpp) ---

static const std::regex RE_BSV_ADDRESS(R"(^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$)");
static const std::regex RE_IDENTITY_KEY(R"(^(02|03)[0-9a-fA-F]{64}$)");
static const std::regex RE_PAYMAIL(R"(^(\$[a-zA-Z0-9_]+|[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,})$)");
static const std::regex RE_BIP21(R"(^bitcoin:)", std::regex_constants::icase);

static std::string QRUrlDecode(const std::string& s) {
    std::string result;
    result.reserve(s.size());
    for (size_t i = 0; i < s.size(); ++i) {
        if (s[i] == '%' && i + 2 < s.size()) {
            int hi = 0, lo = 0;
            if (sscanf(s.c_str() + i + 1, "%1x%1x", &hi, &lo) == 2) {
                result += static_cast<char>((hi << 4) | lo);
                i += 2;
                continue;
            }
        }
        if (s[i] == '+') { result += ' '; continue; }
        result += s[i];
    }
    return result;
}

static std::string QRJsonEscape(const std::string& s) {
    std::string out;
    out.reserve(s.size() + 8);
    for (char c : s) {
        switch (c) {
            case '"':  out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n";  break;
            case '\r': out += "\\r";  break;
            case '\t': out += "\\t";  break;
            default:   out += c;      break;
        }
    }
    return out;
}

static std::string ClassifyBSVContent(const std::string& text) {
    if (std::regex_search(text, RE_BIP21)) {
        std::string address, amount, label;
        size_t colon = text.find(':');
        std::string rest = (colon != std::string::npos) ? text.substr(colon + 1) : text;

        size_t q = rest.find('?');
        address = (q != std::string::npos) ? rest.substr(0, q) : rest;

        if (q != std::string::npos) {
            std::string params = rest.substr(q + 1);
            std::istringstream ps(params);
            std::string pair;
            while (std::getline(ps, pair, '&')) {
                size_t eq = pair.find('=');
                if (eq == std::string::npos) continue;
                std::string key = pair.substr(0, eq);
                std::string val = QRUrlDecode(pair.substr(eq + 1));
                if (key == "amount") amount = val;
                else if (key == "label") label = val;
            }
        }

        std::string json = "{\"type\":\"bip21\",\"value\":\"" + QRJsonEscape(text) + "\"";
        if (!address.empty()) json += ",\"address\":\"" + QRJsonEscape(address) + "\"";
        if (!amount.empty())  json += ",\"amount\":" + amount;
        if (!label.empty())   json += ",\"label\":\"" + QRJsonEscape(label) + "\"";
        json += ",\"source\":\"screen\"}";
        return json;
    }

    if (std::regex_match(text, RE_BSV_ADDRESS)) {
        return "{\"type\":\"address\",\"value\":\"" + QRJsonEscape(text) +
               "\",\"address\":\"" + QRJsonEscape(text) + "\",\"source\":\"screen\"}";
    }

    if (std::regex_match(text, RE_IDENTITY_KEY)) {
        return "{\"type\":\"identity_key\",\"value\":\"" + QRJsonEscape(text) +
               "\",\"source\":\"screen\"}";
    }

    if (std::regex_match(text, RE_PAYMAIL)) {
        return "{\"type\":\"paymail\",\"value\":\"" + QRJsonEscape(text) +
               "\",\"source\":\"screen\"}";
    }

    return "";
}

extern CefRefPtr<CefBrowser> g_qr_scan_requester;

static void DeliverQRResultMacOS(const std::string& json) {
    if (g_qr_scan_requester && g_qr_scan_requester->GetMainFrame()) {
        CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("qr_screen_capture_result");
        msg->GetArgumentList()->SetString(0, json);
        g_qr_scan_requester->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
        LOG_INFO("📷 Screen capture result delivered: " + json.substr(0, 200));
    } else {
        LOG_WARNING("📷 No QR scan requester to deliver result to");
    }
    g_qr_scan_requester = nullptr;
}

// --- QRSelectionView: drag-to-select overlay ---

@interface QRSelectionView : NSView
@property (nonatomic) BOOL isDragging;
@property (nonatomic) NSPoint dragStart;
@property (nonatomic) NSPoint dragCurrent;
@end

@implementation QRSelectionView

- (BOOL)acceptsFirstResponder {
    return YES;
}

- (void)mouseDown:(NSEvent*)event {
    self.dragStart = [self convertPoint:event.locationInWindow fromView:nil];
    self.dragCurrent = self.dragStart;
    self.isDragging = YES;
    [self setNeedsDisplay:YES];
}

- (void)mouseDragged:(NSEvent*)event {
    if (self.isDragging) {
        self.dragCurrent = [self convertPoint:event.locationInWindow fromView:nil];
        [self setNeedsDisplay:YES];
    }
}

- (void)mouseUp:(NSEvent*)event {
    if (self.isDragging) {
        self.isDragging = NO;
        self.dragCurrent = [self convertPoint:event.locationInWindow fromView:nil];

        CGFloat x1 = fmin(self.dragStart.x, self.dragCurrent.x);
        CGFloat y1 = fmin(self.dragStart.y, self.dragCurrent.y);
        CGFloat x2 = fmax(self.dragStart.x, self.dragCurrent.x);
        CGFloat y2 = fmax(self.dragStart.y, self.dragCurrent.y);
        NSRect selectionRect = NSMakeRect(x1, y1, x2 - x1, y2 - y1);

        FinishQRScreenCaptureMacOS(false, selectionRect);
    }
}

- (void)keyDown:(NSEvent*)event {
    if (event.keyCode == 53) { // ESC
        self.isDragging = NO;
        FinishQRScreenCaptureMacOS(true, NSZeroRect);
    }
}

- (void)rightMouseDown:(NSEvent*)event {
    self.isDragging = NO;
    FinishQRScreenCaptureMacOS(true, NSZeroRect);
}

- (void)drawRect:(NSRect)dirtyRect {
    // Semi-transparent black overlay
    [[NSColor colorWithCalibratedWhite:0.0 alpha:0.6] set];
    NSRectFill(self.bounds);

    if (self.isDragging) {
        CGFloat x1 = fmin(self.dragStart.x, self.dragCurrent.x);
        CGFloat y1 = fmin(self.dragStart.y, self.dragCurrent.y);
        CGFloat x2 = fmax(self.dragStart.x, self.dragCurrent.x);
        CGFloat y2 = fmax(self.dragStart.y, self.dragCurrent.y);
        NSRect selRect = NSMakeRect(x1, y1, x2 - x1, y2 - y1);

        // Clear selection area (transparent cutout)
        [[NSColor clearColor] set];
        NSRectFillUsingOperation(selRect, NSCompositingOperationCopy);

        // 2px gold border (#a67c00)
        NSBezierPath* border = [NSBezierPath bezierPathWithRect:NSInsetRect(selRect, -1, -1)];
        [border setLineWidth:2.0];
        [[NSColor colorWithCalibratedRed:166.0/255.0 green:124.0/255.0 blue:0.0 alpha:1.0] set];
        [border stroke];
    } else {
        // Instruction text when not dragging
        NSString* text = @"Drag to select a QR code. Press ESC to cancel.";
        NSDictionary* attrs = @{
            NSFontAttributeName: [NSFont systemFontOfSize:18 weight:NSFontWeightMedium],
            NSForegroundColorAttributeName: [NSColor whiteColor]
        };
        NSSize textSize = [text sizeWithAttributes:attrs];
        NSPoint textPoint = NSMakePoint(
            (self.bounds.size.width - textSize.width) / 2,
            (self.bounds.size.height - textSize.height) / 2
        );
        [text drawAtPoint:textPoint withAttributes:attrs];
    }
}

@end

// --- Screen capture lifecycle ---

void StartQRScreenCaptureMacOS() {
    LOG_INFO("📷 Starting QR screen capture (macOS)");

    // Request Screen Recording permission if not yet granted (macOS 11+).
    // Don't block — proceed with capture attempt anyway. CGWindowListCreateImage
    // returns a blank/null image if denied, which we handle gracefully below.
    if (@available(macOS 11.0, *)) {
        if (!CGPreflightScreenCaptureAccess()) {
            LOG_INFO("📷 Screen recording permission not yet granted — requesting");
            CGRequestScreenCaptureAccess();
        }
    }

    // Clean up any existing selection window
    if (g_qr_selection_window) {
        [g_qr_selection_window orderOut:nil];
        [g_qr_selection_window close];
        g_qr_selection_window = nullptr;
        g_qr_selection_view = nullptr;
    }

    // Cover all screens
    NSRect unionRect = NSZeroRect;
    for (NSScreen* screen in [NSScreen screens]) {
        unionRect = NSUnionRect(unionRect, [screen frame]);
    }

    LOG_INFO("📷 Selection overlay: " + std::to_string((int)unionRect.size.width) + "x" +
             std::to_string((int)unionRect.size.height) + " at (" +
             std::to_string((int)unionRect.origin.x) + "," +
             std::to_string((int)unionRect.origin.y) + ")");

    g_qr_selection_window = [[NSWindow alloc]
        initWithContentRect:unionRect
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    [g_qr_selection_window setLevel:NSScreenSaverWindowLevel];
    [g_qr_selection_window setOpaque:NO];
    [g_qr_selection_window setBackgroundColor:[NSColor clearColor]];
    [g_qr_selection_window setIgnoresMouseEvents:NO];
    [g_qr_selection_window setAcceptsMouseMovedEvents:YES];
    [g_qr_selection_window setReleasedWhenClosed:NO];

    g_qr_selection_view = [[QRSelectionView alloc]
        initWithFrame:NSMakeRect(0, 0, unionRect.size.width, unionRect.size.height)];
    [g_qr_selection_window setContentView:g_qr_selection_view];
    [g_qr_selection_window makeKeyAndOrderFront:nil];
    [g_qr_selection_window makeFirstResponder:g_qr_selection_view];

    [[NSCursor crosshairCursor] push];
}

void FinishQRScreenCaptureMacOS(bool cancelled, NSRect selection) {
    LOG_INFO("📷 Finishing QR screen capture (macOS) cancelled=" +
             std::string(cancelled ? "true" : "false"));

    // Destroy selection window BEFORE capture (so it doesn't appear in screenshot)
    if (g_qr_selection_window) {
        [g_qr_selection_window orderOut:nil];
        [g_qr_selection_window close];
        g_qr_selection_window = nullptr;
        g_qr_selection_view = nullptr;
    }

    [NSCursor pop];

    if (cancelled) {
        ShowWalletOverlay();
        DeliverQRResultMacOS("{\"status\":\"cancelled\"}");
        return;
    }

    CGFloat w = selection.size.width;
    CGFloat h = selection.size.height;

    if (w < 10 || h < 10) {
        LOG_WARNING("📷 Selection too small (" + std::to_string((int)w) + "x" + std::to_string((int)h) + ")");
        ShowWalletOverlay();
        DeliverQRResultMacOS("{\"status\":\"not_found\"}");
        return;
    }

    // Convert NSView coordinates (bottom-left origin) to CG coordinates (top-left origin)
    // The selection window covered all screens starting from unionRect
    NSRect unionRect = NSZeroRect;
    for (NSScreen* screen in [NSScreen screens]) {
        unionRect = NSUnionRect(unionRect, [screen frame]);
    }

    // selection is in the view's coordinate system (origin at bottom-left of union rect)
    // CG display coordinates: origin at top-left of primary display
    CGRect captureRect = CGRectMake(
        unionRect.origin.x + selection.origin.x,
        (unionRect.size.height + unionRect.origin.y) - (selection.origin.y + selection.size.height),
        selection.size.width,
        selection.size.height
    );

    LOG_INFO("📷 Capturing region: " + std::to_string((int)captureRect.size.width) + "x" +
             std::to_string((int)captureRect.size.height) + " at (" +
             std::to_string((int)captureRect.origin.x) + "," +
             std::to_string((int)captureRect.origin.y) + ")");

    // CGWindowListCreateImage is marked unavailable in macOS 15 SDK (replaced by
    // ScreenCaptureKit), but still functions at runtime and is the only synchronous
    // screen capture API. Use dlsym to bypass the SDK availability check.
    typedef CGImageRef (*CGWindowListCreateImageFunc)(CGRect, CGWindowListOption, CGWindowID, CGWindowImageOption);
    static CGWindowListCreateImageFunc captureFunc = (CGWindowListCreateImageFunc)dlsym(RTLD_DEFAULT, "CGWindowListCreateImage");
    CGImageRef cgImage = captureFunc ? captureFunc(
        captureRect,
        kCGWindowListOptionOnScreenOnly,
        kCGNullWindowID,
        kCGWindowImageDefault
    ) : nullptr;

    if (!cgImage) {
        LOG_WARNING("📷 CGWindowListCreateImage returned null");
        ShowWalletOverlay();
        DeliverQRResultMacOS("{\"status\":\"not_found\"}");
        return;
    }

    // Decode QR via CIDetector
    CIImage* ciImage = [CIImage imageWithCGImage:cgImage];
    CGImageRelease(cgImage);

    CIDetector* detector = [CIDetector detectorOfType:CIDetectorTypeQRCode
                                              context:nil
                                              options:@{CIDetectorAccuracy: CIDetectorAccuracyHigh}];
    NSArray* features = [detector featuresInImage:ciImage];

    LOG_INFO("📷 CIDetector found " + std::to_string((int)features.count) + " QR code(s)");

    std::string bestResult;
    for (CIFeature* feature in features) {
        if ([feature isKindOfClass:[CIQRCodeFeature class]]) {
            CIQRCodeFeature* qr = (CIQRCodeFeature*)feature;
            if (qr.messageString) {
                std::string payload = [qr.messageString UTF8String];
                LOG_INFO("📷 QR payload: " + payload.substr(0, 200));
                std::string json = ClassifyBSVContent(payload);
                if (!json.empty()) {
                    bestResult = json;
                    break;
                }
            }
        }
    }

    ShowWalletOverlay();

    if (bestResult.empty()) {
        LOG_INFO("📷 No BSV QR code found in selection");
        DeliverQRResultMacOS("{\"status\":\"not_found\"}");
    } else {
        LOG_INFO("📷 BSV QR code found: " + bestResult.substr(0, 200));
        DeliverQRResultMacOS("{\"status\":\"found\",\"result\":" + bestResult + "}");
    }
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
    [g_backup_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];

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
    [g_brc100_auth_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];

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
        // Ensure the view is first responder for keyboard input (needed for text fields)
        [g_notification_overlay_window makeFirstResponder:[g_notification_overlay_window contentView]];
        existing->GetHost()->SetFocus(true);
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

    g_notification_overlay_window = [[NotificationOverlayWindow alloc]
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
    [g_notification_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];

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
        [g_notification_overlay_window makeFirstResponder:[g_notification_overlay_window contentView]];
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

    // Settings menu: flush right, flush below header
    int menuWidth = 300;
    int menuHeight = 480;
    NSRect menuFrame = CalculateToolbarOverlayFrame(g_main_window, menuWidth, menuHeight, 96);

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
    [g_settings_menu_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];
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
        NSRect contentScreen = [g_main_window convertRectToScreen:[[g_main_window contentView] frame]];
        int omniboxWidth = (int)(contentScreen.size.width * 0.69);
        if (omniboxWidth < 400) omniboxWidth = 400;
        int omniboxHeight = 420;
        CGFloat overlayX = contentScreen.origin.x + (contentScreen.size.width - omniboxWidth) / 2;
        CGFloat contentTop = contentScreen.origin.y + contentScreen.size.height;
        CGFloat overlayY = contentTop - 78 - omniboxHeight;
        [g_omnibox_overlay_window setFrame:NSMakeRect(overlayX, overlayY, omniboxWidth, omniboxHeight) display:YES];

        [g_omnibox_overlay_window orderFront:nil];
        InstallOmniboxClickOutsideMonitor();
        LOG_INFO("Omnibox overlay shown (macOS)");
    }
}

void CreateOmniboxOverlayMacOS() {
    LOG_INFO("Creating omnibox overlay (macOS)");

    NSRect contentScreen = [g_main_window convertRectToScreen:[[g_main_window contentView] frame]];
    // Position below header (99px) spanning most of the window width
    int omniboxWidth = (int)(contentScreen.size.width * 0.69);
    if (omniboxWidth < 400) omniboxWidth = 400;
    int omniboxHeight = 420;
    // Center horizontally, position below header
    CGFloat overlayX = contentScreen.origin.x + (contentScreen.size.width - omniboxWidth) / 2;
    CGFloat contentTop = contentScreen.origin.y + contentScreen.size.height;
    CGFloat overlayY = contentTop - 78 - omniboxHeight;
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
    [g_omnibox_overlay_window setHasShadow:NO];
    [g_omnibox_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];
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

        NSRect panelFrame = CalculateToolbarOverlayFrame(g_main_window, 400, 500, 96);
        [g_download_panel_overlay_window setFrame:panelFrame display:YES];

        [g_download_panel_overlay_window makeKeyAndOrderFront:nil];
        InstallDownloadPanelClickOutsideMonitor();
        LOG_INFO("Download panel overlay shown (macOS)");
    }
}

void CreateDownloadPanelOverlayMacOS(int iconRightOffset) {
    LOG_INFO("Creating download panel overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_download_panel_icon_right_offset = iconRightOffset;

    CGFloat panelWidth = 400;
    CGFloat panelHeight = 500;
    NSRect panelFrame = CalculateToolbarOverlayFrame(g_main_window, panelWidth, panelHeight, 96);

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
    [g_download_panel_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];
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

        NSRect panelFrame = CalculateToolbarOverlayFrame(g_main_window, 300, 400, 96);
        [g_profile_panel_overlay_window setFrame:panelFrame display:YES];

        [g_profile_panel_overlay_window makeKeyAndOrderFront:nil];
        InstallProfilePanelClickOutsideMonitor();
        LOG_INFO("Profile panel overlay shown (macOS)");
    }
}

void CreateProfilePanelOverlayMacOS(int iconRightOffset) {
    LOG_INFO("Creating profile panel overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_profile_panel_icon_right_offset = iconRightOffset;

    CGFloat panelWidth = 300;
    CGFloat panelHeight = 400;
    NSRect panelFrame = CalculateToolbarOverlayFrame(g_main_window, panelWidth, panelHeight, 96);

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
    [g_profile_panel_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];
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

    // Step 1: Force-close ALL CEF browsers (tabs, overlays, header)
    // Using CloseBrowser(true) = force close, skips beforeunload handlers.
    // This prevents audio/video from continuing to play after the window disappears.
    // Matches Windows behavior (see cef_browser_shell.cpp ShutdownApplication).
    LOG_INFO("🔄 Force-closing all CEF browsers...");

    // 1a: Close all tab browsers (each tab has its own CefBrowser + renderer process)
    {
        std::vector<Tab*> allTabs = TabManager::GetInstance().GetAllTabs();
        LOG_INFO("🔄 Closing " + std::to_string(allTabs.size()) + " tab browser(s)...");
        for (Tab* tab : allTabs) {
            if (tab && tab->browser) {
                tab->browser->GetHost()->CloseBrowser(true);
            }
        }
    }

    // 1b: Close all overlay and window browsers via BrowserWindow refs
    {
        std::vector<BrowserWindow*> allWindows = WindowManager::GetInstance().GetAllWindows();
        const std::string roles[] = {
            "header", "wallet_panel", "overlay", "settings",
            "wallet", "backup", "brc100auth", "notification", "settings_menu",
            "omnibox", "cookiepanel", "downloadpanel", "profilepanel", "menu"
        };
        for (BrowserWindow* bw : allWindows) {
            if (!bw) continue;
            for (const auto& role : roles) {
                CefRefPtr<CefBrowser> b = bw->GetBrowserForRole(role);
                if (b) {
                    LOG_INFO("🔄 Force-closing browser for role: " + role);
                    b->GetHost()->CloseBrowser(true);
                }
            }
        }
    }

    // 1c: Close any remaining browser refs not tracked by WindowManager
    CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
    CefRefPtr<CefBrowser> webview_browser = SimpleHandler::GetWebviewBrowser();
    CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
    CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
    CefRefPtr<CefBrowser> backup_browser = SimpleHandler::GetBackupBrowser();
    CefRefPtr<CefBrowser> brc100_auth_browser = SimpleHandler::GetBRC100AuthBrowser();
    CefRefPtr<CefBrowser> settings_menu_browser = SimpleHandler::GetSettingsMenuBrowser();
    CefRefPtr<CefBrowser> cookie_panel_browser = SimpleHandler::GetCookiePanelBrowser();
    CefRefPtr<CefBrowser> omnibox_browser = SimpleHandler::GetOmniboxBrowser();
    CefRefPtr<CefBrowser> download_panel_browser = SimpleHandler::GetDownloadPanelBrowser();
    CefRefPtr<CefBrowser> profile_panel_browser = SimpleHandler::GetProfilePanelBrowser();

    if (header_browser) header_browser->GetHost()->CloseBrowser(true);
    if (webview_browser) webview_browser->GetHost()->CloseBrowser(true);
    if (settings_browser) settings_browser->GetHost()->CloseBrowser(true);
    if (wallet_browser) wallet_browser->GetHost()->CloseBrowser(true);
    if (backup_browser) backup_browser->GetHost()->CloseBrowser(true);
    if (brc100_auth_browser) brc100_auth_browser->GetHost()->CloseBrowser(true);
    if (settings_menu_browser) settings_menu_browser->GetHost()->CloseBrowser(true);
    if (cookie_panel_browser) cookie_panel_browser->GetHost()->CloseBrowser(true);
    if (omnibox_browser) omnibox_browser->GetHost()->CloseBrowser(true);
    if (download_panel_browser) download_panel_browser->GetHost()->CloseBrowser(true);
    if (profile_panel_browser) profile_panel_browser->GetHost()->CloseBrowser(true);

    if (g_menu_overlay_browser_ref && g_menu_overlay_browser_ref->browser) {
        g_menu_overlay_browser_ref->browser->GetHost()->CloseBrowser(true);
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

    // Production: binary alongside main exe in Contents/MacOS/
    std::string walletExe = exeDir + "/hodos-wallet";

    if (access(walletExe.c_str(), X_OK) != 0) {
        // Dev fallback: relative path from build dir
        walletExe = exeDir + "/../../../../../../rust-wallet/target/release/hodos-wallet";
        if (access(walletExe.c_str(), X_OK) != 0) {
            walletExe = exeDir + "/../../../../../rust-wallet/target/release/hodos-wallet";
            if (access(walletExe.c_str(), X_OK) != 0) {
                LOG_WARNING("Wallet server executable not found - browser will run without auto-launched wallet");
                LOG_WARNING("Start wallet manually: cd rust-wallet && cargo run --release");
                return;
            }
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

    // Production: binary alongside main exe in Contents/MacOS/
    std::string adblockExe = exeDir + "/hodos-adblock";
    if (access(adblockExe.c_str(), X_OK) != 0) {
        // Dev fallback: relative path from build dir
        adblockExe = exeDir + "/../../../../../../adblock-engine/target/release/hodos-adblock";
        if (access(adblockExe.c_str(), X_OK) != 0) {
            adblockExe = exeDir + "/../../../../../adblock-engine/target/release/hodos-adblock";
            if (access(adblockExe.c_str(), X_OK) != 0) {
                LOG_WARNING("Adblock engine not found - browser will run without ad blocking");
                return;
            }
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

    // Dev safeguard: refuse to run from build directory without HODOS_DEV=1
    if (!AppPaths::EnforceDevSafeguard(std::string(exec_path))) {
        fprintf(stderr, "Exiting due to dev safeguard.\n");
        return 1;
    }

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

        // Install NSApplicationDelegate for menu-bar-style keep-alive (dock
        // reopen, applicationShouldHandleReopen:, dock menu). See
        // HodosAppDelegate comment for the Chromium pattern we mirror.
        g_app_delegate = [[HodosAppDelegate alloc] init];
        [NSApp setDelegate:g_app_delegate];
        fprintf(stderr, "✅ NSApp.delegate installed: %s (responds to applicationDockMenu: = %d)\n",
                [NSApp delegate] == g_app_delegate ? "yes" : "NO",
                [g_app_delegate respondsToSelector:@selector(applicationDockMenu:)]);

        // Initialize centralized logger with absolute path
        // (CWD is '/' when launched via 'open', so relative paths fail)
        {
            NSString* appSupDir = [NSSearchPathForDirectoriesInDomains(NSApplicationSupportDirectory, NSUserDomainMask, YES) firstObject];
            NSString* logDir = [appSupDir stringByAppendingPathComponent:
                [NSString stringWithUTF8String:AppPaths::GetAppDirName().c_str()]];
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
        NSString* hodosBrowserDir = [appSupport stringByAppendingPathComponent:
            [NSString stringWithUTF8String:AppPaths::GetAppDirName().c_str()]];

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
        // Load per-site fingerprint overrides from fingerprint_settings.json
        FingerprintProtection::GetInstance().LoadSiteSettings(profile_cache);
        // Sync global toggle from persisted settings
        FingerprintProtection::GetInstance().SetEnabled(
            SettingsManager::GetInstance().GetPrivacySettings().fingerprintProtection);
        LOG_INFO("Fingerprint protection initialized (enabled=" +
            std::string(SettingsManager::GetInstance().GetPrivacySettings().fingerprintProtection ? "true" : "false") + ")");

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
        // Seed first tab via TabManager (window 0)
        //
        // Replaces the previous standalone "webview" CEF browser (Bug #9):
        // the standalone browser parented a CEF-managed NSWindow subtree to
        // g_webview_view, and React tab-close clicks cascaded up the responder
        // chain into [NSWindow close] on the main window — shutting down the
        // whole app. Routing the first tab through TabManager mirrors what
        // WindowManager_mac.mm:186-194 already does for secondary windows and
        // matches upstream Chromium (all content WebContents enter via
        // TabStripModel; see chrome/browser/ui/browser_tabstrip.cc).
        // ===================================================================

        if (TabManager::GetInstance().GetAllTabs().empty()) {
            NSView* webviewView = (__bridge NSView*)g_webview_view;
            NSRect webviewBounds = [webviewView bounds];

            LOG_INFO("🔧 Seeding first tab via TabManager::CreateTab (window 0)");
            LOG_INFO("📐 Webview bounds: " + std::to_string((int)webviewBounds.size.width) +
                     "x" + std::to_string((int)webviewBounds.size.height));

            int tabId = TabManager::GetInstance().CreateTab(
                "http://127.0.0.1:5137/newtab",
                g_webview_view,
                0, 0,
                (int)webviewBounds.size.width,
                (int)webviewBounds.size.height,
                /*window_id=*/0);

            LOG_INFO("✅ First tab seeded: tab id " + std::to_string(tabId));
            SimpleHandler::NotifyWindowTabListChanged(0);
        } else {
            LOG_INFO("ℹ️ Skipping first-tab seed — TabManager already has tabs (session restore?)");
        }

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

    CGFloat menuWidth = 280;
    CGFloat menuHeight = 450;

    // Position: flush right, flush below header (96px header)
    NSRect menuFrame = CalculateToolbarOverlayFrame(g_main_window, menuWidth, menuHeight, 96);

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
    [g_menu_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];

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
