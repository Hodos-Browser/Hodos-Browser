// macOS Implementation of HodosBrowser Shell
// Uses Cocoa/AppKit for window management and CEF for browser rendering

#import <Cocoa/Cocoa.h>
#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <QuartzCore/QuartzCore.h>
#import <CoreImage/CoreImage.h>
#import <mach-o/dyld.h>

#include "include/cef_application_mac.h"
#include "include/core/PendingPermissionRequest.h"
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
#include "include/core/PortConfig.h"
#include "include/core/LayoutHelpers.h"   // kMacHeaderHeightPt — D-h1
#include "include/core/JsStringEscape.h"  // escapeJsonForJs — notification query hardening (P0.5 panel #3)
#include "include/handlers/my_overlay_render_handler.h"
#include "include/wrapper/cef_library_loader.h"
#include "OverlayHelpers_mac.h"

#include <atomic>
#include <thread>   // P8d-A8: the backend supervisor's detached thread
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
// D-h2 overlay-ownership helper, defined with the other three further down — declared
// here because DestroyMenuOverlayWindow() (above them) has to detach the menu overlay
// from whichever window actually owns it.
static void DetachOverlayFromParentMac(NSWindow* overlay);
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
#include "include/core/AutoUpdater.h"
#include "include/core/SilentStateWriter.h"  // hodos::MoreConservativeMode (pure)
#include "include/core/SyncHttpClient.h"
#include "include/core/AdblockCache.h"
#include "include/core/FingerprintProtection.h"
#include "include/core/FarblingPolicy.h"
#include "include/core/CookieBlockManager.h"
#include "include/core/BookmarkManager.h"
#include "include/core/FaviconStore.h"
#include "include/core/PaidContentCache.h"
#include "include/core/SitePermissionStore.h"
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

// Native macOS Space fullscreen state (green button / toggleFullScreen:)
static bool g_native_fullscreen = false;

// Guard against tab close cascading to window close via responder chain
bool g_closing_tab = false;

// Overlay windows (created on-demand)
NSWindow* g_settings_overlay_window = nullptr;
NSWindow* g_wallet_overlay_window = nullptr;
NSWindow* g_brc100_auth_overlay_window = nullptr;
NSWindow* g_notification_overlay_window = nullptr;
NSWindow* g_settings_menu_overlay_window = nullptr;
NSWindow* g_menu_overlay_window = nullptr;
NSWindow* g_cookie_panel_overlay_window = nullptr;
NSWindow* g_omnibox_overlay_window = nullptr;
NSWindow* g_download_panel_overlay_window = nullptr;
NSWindow* g_profile_panel_overlay_window = nullptr;
NSWindow* g_bookmarks_panel_overlay_window = nullptr;
NSWindow* g_siteinfo_panel_overlay_window = nullptr;
NSWindow* g_tablist_panel_overlay_window = nullptr;
// Overlay #15 — the tab context menu (beta.3 Phase 4). ⚠️ The ONLY overlay of the 15
// anchored to the CURSOR rather than to a toolbar icon, so its create/show take an
// (anchorX, anchorY) pair instead of an icon offset.
NSWindow* g_tabmenu_overlay_window = nullptr;

// QR screen capture overlay
static NSWindow* g_qr_selection_window = nullptr;
static NSView*   g_qr_selection_view = nullptr;

// OverlayBrowserRef instances for overlays using GenericOverlayView
static OverlayBrowserRef* g_menu_overlay_browser_ref = nullptr;
static CefRefPtr<MyOverlayRenderHandler> g_menu_overlay_render_handler = nullptr;
static CFAbsoluteTime g_menu_overlay_last_hide_time = 0;
static id g_menu_click_monitor = nil;

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

// Bookmarks panel overlay monitors
static id g_bookmarks_panel_click_monitor = nil;
static CFAbsoluteTime g_bookmarks_panel_last_hide_time = 0;

// Site-info panel overlay monitors
static id g_siteinfo_panel_click_monitor = nil;
static CFAbsoluteTime g_siteinfo_panel_last_hide_time = 0;

// Tab-list panel overlay monitors
static id g_tablist_panel_click_monitor = nil;
static CFAbsoluteTime g_tablist_panel_last_hide_time = 0;

// Tab context menu overlay monitor (overlay #15).
// ⚠️ Two monitors, not one: every other dropdown here watches LEFT mouse-down only,
// but this menu is OPENED by a right-click. Watching left only would let a right-click
// on a second tab arrive at the header while the menu for the first tab is still on
// screen — the menu would jump to the new anchor with the old target still remembered
// in s_tabmenu_target_tab_id, which is P4-A2's defect surfaced from the other side.
static id g_tabmenu_click_monitor = nil;
static id g_tabmenu_rclick_monitor = nil;

// HttpRequestInterceptor.cpp — drop the 30 s "exists" cache the instant the wallet dies.
// ⛔ Without this a cached "exists" outlives the wallet by up to 30 s and dApp calls fail as
// "HTTP 0" instead of WALLET_UNAVAILABLE (Phase 8d contract D-8). Same declaration Windows
// carries in cef_browser_shell.cpp.
void invalidateWalletStatusCache();

// Phase 8d `P8d-A8` — forward declarations. ShutdownApplication() calls
// StopBackendSupervisor() well before the supervisor block is defined further down this
// same TU, so the declarations have to come first.
static void StartBackendSupervisor();
static void StopBackendSupervisor();

// Server process management
static pid_t g_wallet_server_pid = -1;
static pid_t g_adblock_server_pid = -1;
std::atomic<bool> g_walletServerRunning{false};
std::atomic<bool> g_adblockServerRunning{false};
bool g_app_shutting_down = false;
bool g_header_browser_loaded = false;
bool g_picker_mode = false;

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
//
// ✅ WINDOW-SCOPED since the macOS Phase 3 port (2026-09-19). `win` is the window whose
// tab asked, resolved by the shared caller in simple_handler.cpp.
//
// ⛔ It used to take `win`, record one flag on it, and then drive the process globals
// g_main_window / g_header_view / g_webview_view / g_native_fullscreen /
// g_pre_fullscreen_frame and TabManager::GetActiveTab() — the exact shape Windows fixed
// in Phase 3 and the comment here said so. 📏 MEASURED on macOS before this change, two
// windows in one process, B torn off: a page calling requestFullscreen() in a tab of
// window **B** put window **A** into fullscreen (A 795 -> 900, B unchanged at 697) and
// hid **A's** header. The window that asked did nothing.
//
// ⚠️ The Escape monitor stays ONE process-wide NSEvent monitor — that is correct, because
// it is a keyboard hook and only one window can hold content fullscreen at a time — but
// it now resolves the tab of the window that entered, not the process-wide active tab.
// macOS Phase 3 port — the two fullscreen flags are per-window state now
// (BrowserWindow::is_window_fullscreen / is_content_fullscreen). These two read that
// record and fall back to the old globals only if the window record has gone, which is
// the shutdown path.
static bool WindowIsNativeFullscreen(int windowId) {
    BrowserWindow* w = WindowManager::GetInstance().GetWindow(windowId);
    return w ? w->is_window_fullscreen : g_native_fullscreen;
}

static bool WindowIsContentFullscreen(int windowId) {
    BrowserWindow* w = WindowManager::GetInstance().GetWindow(windowId);
    return w ? w->is_content_fullscreen : g_content_fullscreen;
}

static void SetWindowNativeFullscreen(int windowId, bool on) {
    BrowserWindow* w = WindowManager::GetInstance().GetWindow(windowId);
    if (w) w->is_window_fullscreen = on;
    // ⚠️ The global is kept in step ONLY for window 0, purely so the shutdown-path
    // fallbacks above stay meaningful. Nothing reads it to make a decision any more.
    if (windowId == 0) g_native_fullscreen = on;
}

static NSWindow* FullscreenHostWindow(BrowserWindow* win) {
    if (win && win->ns_window) return (__bridge NSWindow*)win->ns_window;
    return g_main_window;
}

void HandleFullscreenChange(BrowserWindow* win, bool fullscreen) {
    LOG_INFO("HandleFullscreenChange: " + std::string(fullscreen ? "ENTER" : "EXIT") +
             " (window " + std::to_string(win ? win->window_id : -1) + ")");

    if (win) win->is_content_fullscreen = fullscreen;

    // ⛔ Resolve the window's views on the calling thread and capture them, rather than
    // re-reading `win` inside the block: the window could be closed between the post and
    // the run, and BrowserWindow is owned by WindowManager, not by us.
    NSWindow* nsWin = FullscreenHostWindow(win);
    NSView* headerView = (win && win->header_view)
                             ? (__bridge NSView*)win->header_view : g_header_view;
    NSView* webviewView = (win && win->webview_view)
                              ? (__bridge NSView*)win->webview_view : g_webview_view;
    int windowId = win ? win->window_id : 0;
    CefRefPtr<CefBrowser> headerBrowser =
        (win && win->header_browser) ? win->header_browser : SimpleHandler::GetHeaderBrowser();

    dispatch_async(dispatch_get_main_queue(), ^{
        if (!nsWin || !headerView || !webviewView) return;

        BrowserWindow* w = WindowManager::GetInstance().GetWindow(windowId);
        // Native (menu) fullscreen is per-window too — read it off the window, falling
        // back to the global only if the record has gone.
        bool nativeFs = w ? w->is_window_fullscreen : g_native_fullscreen;

        if (fullscreen) {
            if (!nativeFs && w) {
                NSRect f = [nsWin frame];
                w->pre_fullscreen_frame[0] = f.origin.x;
                w->pre_fullscreen_frame[1] = f.origin.y;
                w->pre_fullscreen_frame[2] = f.size.width;
                w->pre_fullscreen_frame[3] = f.size.height;
                w->has_pre_fullscreen_frame = true;
            }

            [headerView setHidden:YES];

            NSRect contentRect = [[nsWin contentView] bounds];
            [webviewView setFrame:contentRect];

            auto* activeTab = TabManager::GetInstance().GetActiveTabForWindow(windowId);
            if (activeTab && activeTab->browser) {
                activeTab->browser->GetHost()->WasResized();
            }

            if (!nativeFs) {
                [NSApp setPresentationOptions:
                    NSApplicationPresentationAutoHideMenuBar |
                    NSApplicationPresentationAutoHideDock];

                NSRect screenFrame = [[nsWin screen] frame];
                [nsWin setFrame:screenFrame display:YES animate:YES];
            }

            // Catch Escape at the NSEvent level — macOS presentation
            // options can swallow keys before they reach the CEF view.
            if (g_fullscreen_escape_monitor) {
                [NSEvent removeMonitor:g_fullscreen_escape_monitor];
            }
            g_fullscreen_escape_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:^NSEvent* (NSEvent* event) {
                if ([event keyCode] == 53) {  // 53 = kVK_Escape
                    // ⛔ GetActiveTabForWindow, not GetActiveTab: Escape must exit the
                    // fullscreen that is actually on screen, not whichever tab the
                    // process last made active.
                    auto* tab = TabManager::GetInstance().GetActiveTabForWindow(windowId);
                    if (tab && tab->browser && tab->browser->GetMainFrame()) {
                        tab->browser->GetMainFrame()->ExecuteJavaScript(
                            "document.exitFullscreen()", "", 0);
                    }
                    return nil;  // consume
                }
                return event;
            }];
        } else {
            if (g_fullscreen_escape_monitor) {
                [NSEvent removeMonitor:g_fullscreen_escape_monitor];
                g_fullscreen_escape_monitor = nil;
            }

            if (!nativeFs) {
                [NSApp setPresentationOptions:NSApplicationPresentationDefault];

                if (w && w->has_pre_fullscreen_frame) {
                    NSRect restore = NSMakeRect(w->pre_fullscreen_frame[0],
                                                w->pre_fullscreen_frame[1],
                                                w->pre_fullscreen_frame[2],
                                                w->pre_fullscreen_frame[3]);
                    if (!NSIsEmptyRect(restore)) {
                        [nsWin setFrame:restore display:YES animate:YES];
                    }
                    w->has_pre_fullscreen_frame = false;
                }
            }

            [headerView setHidden:NO];

            NSRect contentRect = [[nsWin contentView] bounds];
            int headerHeight = kMacHeaderHeightPt;  // D-h1
            NSRect headerRect = NSMakeRect(0, contentRect.size.height - headerHeight,
                                           contentRect.size.width, headerHeight);
            [headerView setFrame:headerRect];

            NSRect webviewRect = NSMakeRect(0, 0, contentRect.size.width,
                                            contentRect.size.height - headerHeight);
            [webviewView setFrame:webviewRect];

            auto* activeTab = TabManager::GetInstance().GetActiveTabForWindow(windowId);
            if (activeTab && activeTab->browser) {
                activeTab->browser->GetHost()->WasResized();
            }

            if (headerBrowser) {
                headerBrowser->GetHost()->WasResized();
            }
        }
    });
}

// ⬜ DEAD as of the macOS Phase 3 port: declared at the top of this file and defined
// here, with ZERO callers anywhere in cef-native/ or frontend/. The live entry point is
// ToggleMainWindowFullscreen() below, which simple_handler.cpp's menu_action arm calls.
// Reported rather than deleted (root CLAUDE.md rule 3 — report unrelated dead code).
void ToggleFullScreenMacOS() {
    if (g_content_fullscreen) {
        LOG_WARNING("Ignoring native fullscreen toggle — content fullscreen active");
        return;
    }
    if (g_main_window) {
        [g_main_window toggleFullScreen:nil];
    }
}

// Native (menu) fullscreen — the three-dot menu's expand button (MenuOverlay.tsx).
//
// ✅ WINDOW-SCOPED since the macOS Phase 3 port. 📏 MEASURED before the change, two
// windows in one process: clicking Fullscreen in the torn-off window **B** fullscreened
// window **A** (A 795 -> 900) and left B untouched. `targetWin` is GetOwnerWindow() from
// the IPC arm, i.e. the window whose menu was clicked.
//
// ⚠️ The content-fullscreen guard is read PER WINDOW: another window being in content
// fullscreen is not a reason to refuse this one's menu toggle.
void ToggleMainWindowFullscreen(BrowserWindow* targetWin) {
    NSWindow* nsWin = FullscreenHostWindow(targetWin);
    int windowId = targetWin ? targetWin->window_id : 0;
    dispatch_async(dispatch_get_main_queue(), ^{
        BrowserWindow* w = WindowManager::GetInstance().GetWindow(windowId);
        bool contentFs = w ? w->is_content_fullscreen : g_content_fullscreen;
        if (contentFs) {
            LOG_WARNING("Ignoring native fullscreen toggle — content fullscreen active "
                        "on window " + std::to_string(windowId));
            return;
        }
        if (nsWin) {
            LOG_INFO("Native fullscreen toggle on window " + std::to_string(windowId));
            [nsWin toggleFullScreen:nil];
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
    if (g_menu_click_monitor) {
        [NSEvent removeMonitor:g_menu_click_monitor];
        g_menu_click_monitor = nil;
    }

    if (renderHandler) {
        renderHandler->DetachView();
    }

    if (overlayWindow) {
        NSView* contentView = [overlayWindow contentView];
        if ([contentView isKindOfClass:[GenericOverlayView class]]) {
            [(GenericOverlayView*)contentView detachBrowser];
        }

        // ⛔ D-h2 FOLLOW-UP: this read `[g_main_window removeChildWindow:...]`, which
        // since the Phase 3.5 macOS port is the WRONG parent whenever the menu was
        // opened from a secondary window — and `removeChildWindow:` on a window that
        // is not the parent is a silent no-op, so the link would be left dangling on
        // a window we are about to close out from under. Detach from the ACTUAL parent.
        DetachOverlayFromParentMac(overlayWindow);

        [overlayWindow orderOut:nil];
        [overlayWindow close];
    }

    if (closeBrowser && menuBrowser) {
        menuBrowser->GetHost()->CloseBrowser(false);
    }

    delete browserRef;
    LOG_INFO("Menu overlay destroyed");
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

// ⭐ `BrowserWindow* targetWin` on the Create/Show pairs below is the macOS port
// of Phase 3.5 (`D-h2`): the overlay anchors to the window that ASKED, not to the
// process-global primary. It defaults to nullptr, which resolves back to the
// primary — see `OverlayHostWindow`. Mirrors Windows' `simple_app.cpp` signatures.
// ⛔ The four overlays WITHOUT it (settings, settings menu, BRC-100 auth,
// notification) are the same four Windows leaves primary-owned. Keeping that set
// identical across platforms is deliberate; see the relay round for the reasoning.
void ToggleWalletPanel();  // C++ callable function
void CreateMainWindow();
void CreateSettingsOverlayWithSeparateProcess(int iconRightOffset);
// ⚠️ NO default arg here — simple_app.h already declares it with one, and repeating
// a default argument is an error. That header is included above.
void CreateWalletOverlayWithSeparateProcess(int iconRightOffset, BrowserWindow* targetWin);
void CreateBRC100AuthOverlayWithSeparateProcess();
void CreateNotificationOverlay(const std::string& type, const std::string& domain, const std::string& extraParams);
void CreateSettingsMenuOverlay();
void CreateMenuOverlayMac(int iconRightOffset, BrowserWindow* targetWin = nullptr);
void ShowSettingsMenuOverlay();
void HideSettingsMenuOverlay();
bool IsSettingsMenuOverlayVisible();
bool WasSettingsMenuJustHidden();
void CreateCookiePanelOverlayWithSeparateProcess(int iconRightOffset, BrowserWindow* targetWin = nullptr);
void ShowCookiePanelOverlay(int iconRightOffset, BrowserWindow* targetWin = nullptr);
void HideCookiePanelOverlay();
bool IsCookiePanelOverlayVisible();
void CreateOmniboxOverlayMacOS(BrowserWindow* targetWin = nullptr);
void ShowOmniboxOverlayMacOS(BrowserWindow* targetWin = nullptr);
void HideOmniboxOverlayMacOS();
bool OmniboxOverlayExists();
void CreateDownloadPanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin = nullptr);
void ShowDownloadPanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin = nullptr);
void HideDownloadPanelOverlayMacOS();
void CreateProfilePanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin = nullptr);
void ShowProfilePanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin = nullptr);
void HideProfilePanelOverlayMacOS();
void CreateBookmarksPanelOverlayMacOS(int iconLeftOffset, BrowserWindow* targetWin = nullptr);
void ShowBookmarksPanelOverlayMacOS(int iconLeftOffset, BrowserWindow* targetWin = nullptr);
void HideBookmarksPanelOverlayMacOS();
bool IsBookmarksPanelOverlayVisible();
bool WasBookmarksPanelJustHidden();
void CreateSiteInfoPanelOverlayMacOS(int iconLeftOffset, BrowserWindow* targetWin = nullptr);
void ShowSiteInfoPanelOverlayMacOS(int iconLeftOffset, BrowserWindow* targetWin = nullptr);
void HideSiteInfoPanelOverlayMacOS();
bool IsSiteInfoPanelOverlayVisible();
bool WasSiteInfoPanelJustHidden();
void CreateTabListPanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin = nullptr);
void ShowTabListPanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin = nullptr);
void HideTabListPanelOverlayMacOS();
bool IsTabListPanelOverlayVisible();
bool WasTabListPanelJustHidden();
// Overlay #15 — tab context menu. ⚠️ Cursor-anchored: (anchorX, anchorY) are CSS px in
// the HEADER browser's viewport (top-left origin, Y down), straight from the React
// onContextMenu event's clientX/clientY.
void CreateTabContextMenuOverlayMacOS(int anchorX, int anchorY, BrowserWindow* targetWin = nullptr);
void ShowTabContextMenuOverlayMacOS(int anchorX, int anchorY, BrowserWindow* targetWin = nullptr);
void HideTabContextMenuOverlayMacOS();
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
@property (nonatomic, strong) NSTrackingArea* overlayTrackingArea;
@end

@implementation SettingsOverlayView

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

- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }

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

- (NSView *)hitTest:(NSPoint)point {
    NSPoint localPoint = [self convertPoint:point fromView:[self superview]];
    if (NSPointInRect(localPoint, [self bounds])) return self;
    return nil;
}

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

- (void)flagsChanged:(NSEvent *)event {
    CefRefPtr<CefBrowser> b = [self overlayBrowser];
    if (!b) return;

    CefKeyEvent key_event;
    key_event.native_key_code = [event keyCode];
    key_event.modifiers = 0;
    if ([event modifierFlags] & NSEventModifierFlagShift) key_event.modifiers |= EVENTFLAG_SHIFT_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagControl) key_event.modifiers |= EVENTFLAG_CONTROL_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagOption) key_event.modifiers |= EVENTFLAG_ALT_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagCommand) key_event.modifiers |= EVENTFLAG_COMMAND_DOWN;

    NSEventModifierFlags flags = [event modifierFlags];
    unsigned short keyCode = [event keyCode];
    bool isDown = false;
    switch (keyCode) {
        case 56: case 60: isDown = (flags & NSEventModifierFlagShift) != 0; break;
        case 59: case 62: isDown = (flags & NSEventModifierFlagControl) != 0; break;
        case 58: case 61: isDown = (flags & NSEventModifierFlagOption) != 0; break;
        case 55: case 54: isDown = (flags & NSEventModifierFlagCommand) != 0; break;
        default: isDown = true; break;
    }

    key_event.type = isDown ? KEYEVENT_RAWKEYDOWN : KEYEVENT_KEYUP;
    b->GetHost()->SendKeyEvent(key_event);
}

@end

// Cookie Panel (Privacy Shield) Overlay View
@interface CookiePanelOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@property (nonatomic, strong) NSTrackingArea* overlayTrackingArea;
@end

@implementation CookiePanelOverlayView

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

- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }

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

- (NSView *)hitTest:(NSPoint)point {
    NSPoint localPoint = [self convertPoint:point fromView:[self superview]];
    if (NSPointInRect(localPoint, [self bounds])) return self;
    return nil;
}

- (void)mouseDown:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> wallet = SimpleHandler::GetWalletBrowser();
    if (wallet) {
        wallet->GetHost()->SetFocus(true);
        wallet->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
        wallet->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
    }
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


// BRC-100 Auth Overlay View
@interface BRC100AuthOverlayView : NSView
@property (nonatomic, strong) CALayer* renderLayer;
@property (nonatomic, strong) NSTrackingArea* overlayTrackingArea;
@end

@implementation BRC100AuthOverlayView

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

- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }

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

// Without this override NSView's default passes the wheel up the responder chain and
// it never reaches the OSR browser — a consent modal whose permission list overflows
// its scroll box then hides the overflow with no way to reach it.
- (void)scrollWheel:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];

    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> auth = SimpleHandler::GetBRC100AuthBrowser();
    if (auth) {
        // macOS scroll deltas: positive deltaY = scroll up
        int deltaX = (int)([event scrollingDeltaX] * 2);
        int deltaY = (int)([event scrollingDeltaY] * 2);
        auth->GetHost()->SendMouseWheelEvent(mouse_event, deltaX, deltaY);
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
@property (nonatomic, strong) NSTrackingArea* overlayTrackingArea;
@end

@implementation NotificationOverlayView

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

- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }

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

// NotificationOverlayWindow::sendEvent routes NSEventTypeScrollWheel here; without this
// override the wheel was dropped, so manifest_connect_bundle's permission list (maxHeight
// 240px, 784px of content for zanaadu.com's 10 protocols) could not be scrolled at all.
- (void)scrollWheel:(NSEvent *)event {
    NSPoint location = [self convertPoint:[event locationInWindow] fromView:nil];
    CefMouseEvent mouse_event;
    mouse_event.x = (int)location.x;
    mouse_event.y = (int)(self.bounds.size.height - location.y);
    mouse_event.modifiers = 0;

    CefRefPtr<CefBrowser> notif = SimpleHandler::GetNotificationBrowser();
    if (notif) {
        // macOS scroll deltas: positive deltaY = scroll up
        int deltaX = (int)([event scrollingDeltaX] * 2);
        int deltaY = (int)([event scrollingDeltaY] * 2);
        notif->GetHost()->SendMouseWheelEvent(mouse_event, deltaX, deltaY);
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
    NSPoint localPoint = [self convertPoint:point fromView:[self superview]];
    if (NSPointInRect(localPoint, [self bounds])) return self;
    return nil;
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
        NSRect sf = CalculateToolbarOverlayFrame(g_main_window, 450, 450, kMacHeaderHeightPt);
        [g_settings_overlay_window setFrame:sf display:YES];
    }

    // Wallet is a child window — moves automatically

    if (g_brc100_auth_overlay_window && [g_brc100_auth_overlay_window isVisible]) {
        [g_brc100_auth_overlay_window setFrame:mainFrame display:YES];
    }
}

- (void)windowDidResize:(NSNotification *)notification {
    LOG_DEBUG("🔄 Main window resized - updating layout");

    NSRect contentRect = [[g_main_window contentView] bounds];

    if (WindowIsContentFullscreen(0)) {
        // In content fullscreen: webview fills entire content area, header stays hidden
        [g_webview_view setFrame:contentRect];
    } else {
        int headerHeight = kMacHeaderHeightPt;  // D-h1
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
        NSRect sf = CalculateToolbarOverlayFrame(g_main_window, 450, 450, kMacHeaderHeightPt);
        [g_settings_overlay_window setFrame:sf display:YES];
        CefRefPtr<CefBrowser> settings = SimpleHandler::GetSettingsBrowser();
        if (settings) settings->GetHost()->WasResized();
    }

    // Wallet is a child window — moves automatically, no resize needed for fixed-size panel

    if (g_brc100_auth_overlay_window && [g_brc100_auth_overlay_window isVisible]) {
        [g_brc100_auth_overlay_window setFrame:mainFrame display:YES];
        CefRefPtr<CefBrowser> auth = SimpleHandler::GetBRC100AuthBrowser();
        if (auth) auth->GetHost()->WasResized();
    }

}

- (void)windowDidBecomeKey:(NSNotification *)notification {
    WindowManager::GetInstance().SetActiveWindowId(0);
    LOG_INFO("Main window became key");
}

- (BOOL)windowShouldClose:(NSWindow *)sender {
    // Reject close if any tab in this window is mid-close (async CloseBrowser
    // triggers focus changes that cascade to windowShouldClose via the responder
    // chain). Check both the synchronous guard AND per-tab is_closing state.
    if (g_closing_tab) {
        LOG_WARNING("❌ Window close rejected — tab close in progress (responder chain cascade)");
        return NO;
    }
    for (auto* tab : TabManager::GetInstance().GetAllTabs()) {
        if (tab->window_id == 0 && tab->is_closing) {
            LOG_WARNING("❌ Window close rejected — tab " + std::to_string(tab->id) + " still closing async");
            return NO;
        }
    }

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
    NSResponder* firstResponder = [g_main_window firstResponder];
    NSString* responderDesc = firstResponder ? [firstResponder description] : @"(nil)";
    LOG_INFO("Main window resigned key — firstResponder: " + std::string([responderDesc UTF8String]));
}

- (void)windowDidEnterFullScreen:(NSNotification *)notification {
    SetWindowNativeFullscreen(0, true);
    LOG_INFO("Native fullscreen ENTERED (window 0)");
}

- (void)windowDidExitFullScreen:(NSNotification *)notification {
    SetWindowNativeFullscreen(0, false);
    LOG_INFO("Native fullscreen EXITED (window 0)");
    if (WindowIsContentFullscreen(0)) {
        NSRect contentRect = [[g_main_window contentView] bounds];
        [g_webview_view setFrame:contentRect];
        [g_header_view setHidden:YES];
    }
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

    // Picker mode: small centered launcher window (mirrors Windows sizing).
    NSRect windowRect = screenRect;
    NSWindowStyleMask styleMask = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                                  NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable |
                                  NSWindowStyleMaskFullSizeContentView;
    if (g_picker_mode) {
        int sw = (int)screenRect.size.width;
        int sh = (int)screenRect.size.height;
        int pw = sw * 60 / 100;  if (pw > 980) pw = 980;  if (pw > sw) pw = sw;
        int ph = sh * 78 / 100;  if (ph > 660) ph = 660;  if (ph > sh) ph = sh;
        int ox = (int)screenRect.origin.x + (sw - pw) / 2;
        int oy = (int)screenRect.origin.y + (sh - ph) / 2;
        windowRect = NSMakeRect(ox, oy, pw, ph);
        styleMask = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                    NSWindowStyleMaskFullSizeContentView;
        LOG_INFO("Picker launcher window: " + std::to_string(pw) + "x" + std::to_string(ph) + " centered");
    }

    // Create main window. NSWindowStyleMaskFullSizeContentView lets our
    // React header render underneath the titlebar — combined with
    // titlebarAppearsTransparent + titleVisibility hidden, the grey titlebar
    // strip disappears visually while the native traffic-light buttons
    // continue to render on top at their default macOS position. Our React
    // TabBar reserves 86px of left padding for them (see TabBar.tsx isMac).
    g_main_window = [[NSWindow alloc]
        initWithContentRect:windowRect
        styleMask:styleMask
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
    [g_main_window setBackgroundColor:[NSColor colorWithSRGBRed:0.102 green:0.114 blue:0.137 alpha:1.0]];  // #1a1d23

    // In picker mode the header fills the entire window (no tab/webview area).
    int winW = (int)windowRect.size.width;
    int winH = (int)windowRect.size.height;
    int headerHeight = g_picker_mode
        ? winH
        : kMacHeaderHeightPt;  // D-h1: tab strip 50 (mac) + toolbar 54
    int webviewHeight = winH - headerHeight;

    LOG_INFO("📐 Header height: " + std::to_string(headerHeight) + "px" +
             (g_picker_mode ? " [picker — full window]" : ""));

    // Create header view (at very top, or full window in picker mode)
    NSRect headerRect = NSMakeRect(0, winH - headerHeight, winW, headerHeight);
    g_header_view = [[NSView alloc] initWithFrame:headerRect];

    if (!g_header_view) {
        LOG_ERROR("❌ Failed to create header view");
        return;
    }

    [g_header_view setAutoresizingMask:NSViewWidthSizable | (g_picker_mode
        ? NSViewHeightSizable : NSViewMinYMargin)];
    [[g_main_window contentView] addSubview:g_header_view];
    LOG_INFO("✅ Header view created at Y=" + std::to_string((int)headerRect.origin.y));

    if (!g_picker_mode) {
        // Create webview/content area (full height below header)
        NSRect webviewRect = NSMakeRect(0, 0, winW, webviewHeight);
        g_webview_view = [[NSView alloc] initWithFrame:webviewRect];

        if (!g_webview_view) {
            LOG_ERROR("❌ Failed to create webview");
            return;
        }

        [g_webview_view setAutoresizingMask:NSViewWidthSizable | NSViewHeightSizable];
        [[g_main_window contentView] addSubview:g_webview_view];
        LOG_INFO("✅ Webview created (full height)");
    }

    // Note: Wallet panel now uses overlay window approach (CreateWalletOverlayWithSeparateProcess)
    // instead of embedded view to maintain parity with Windows implementation

    // Show window
    [g_main_window makeKeyAndOrderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];

    LOG_INFO("✅ Main window created successfully");
}

// ============================================================================
// P3.5 macOS port (`D-h2`) — an overlay belongs to the window that ASKED
// ============================================================================
// Windows fixed this in Phase 3.5 (`simple_app.cpp :: OwnOverlayToRequestingWindow`,
// `GWLP_HWNDPARENT`). macOS had no counterpart: every anchor site named the
// process-global `g_main_window`, so a dropdown opened in a torn-off window B
// opened over the PRIMARY window A.
//
// 📏 MEASURED pre-fix, 2026-09-19 (relay round f §3, re-run this round on
// `88e6592` before any code was written): tab 2 torn off to a window B at
// x=600 overlapping A at x=0, every dropdown driven over CDP from EACH header,
// frames read with `CGWindowListCopyWindowInfo` — **8/8 overlays opened at
// byte-identical A-relative coordinates whichever window asked** (menu 1160 =
// A.right−280, profile 1060, download/cookie 1040, tablist 1100, bookmarks and
// siteinfo 0, omnibox 223). The requesting window had no influence at all.
//
// ⭐ THE macOS SHAPE IS NOT THE WINDOWS SHAPE, and that is what keeps this small.
// Windows had to engineer the z-order fix because every overlay was OWNED by the
// primary and `SetWindowPos` raises the owner's group. On macOS only FOUR of
// these overlays are `addChildWindow:` children (cookie, wallet, omnibox, menu);
// the other dropdowns attach to nothing and therefore already have the property
// Windows had to build — see the `CreateTabContextMenuOverlayMacOS` note, which
// chose "attach to nothing" for exactly this reason. So:
//     * all ten overlays get their GEOMETRY from the requesting window;
//     * only the four real child windows also get their PARENT moved.
// Converting the other six into child windows would be unrequested scope AND a
// behaviour change — it would introduce the parent coupling the tabmenu comment
// deliberately avoided.
//
// ⚠️ NO `ScalePx` COUNTERPART, deliberately. Windows' `P3.5-A3` converted 12
// `ScalePx(x, g_hwnd)` sites because its overlays are sized in PHYSICAL pixels
// and took DPI from the primary. A macOS OSR overlay is sized in POINTS and
// React CSS px are points, so there is no scale step here to get wrong. Adding
// one would double every offset on a Retina display.

// The NSWindow an overlay should anchor to and hang under.
// ⛔ Falls back to the primary whenever the requesting window is unusable, so a
// null `targetWin` reproduces exactly today's behaviour — that fallback is what
// makes the whole port revertible one call site at a time.
static NSWindow* OverlayHostWindow(BrowserWindow* targetWin) {
    if (targetWin && targetWin->ns_window) {
        NSWindow* w = (__bridge NSWindow*)targetWin->ns_window;
        if (w && [w isVisible]) return w;
    }
    return g_main_window;
}

// Hand a CHILD overlay to the window that asked. Only the four overlays that are
// already `addChildWindow:` children call this; the rest need geometry only.
//
// 📖 Prior art read before writing this (root CLAUDE.md rule 4 — AppKit child
// windows are not an API this file had used this way). Chromium
// `components/remote_cocoa/app_shim/`:
//   1. `native_widget_ns_window_bridge.mm :: SetParent` removes the window from
//      its old parent BEFORE adding it to the new one. A window has one parent.
//   2. `native_widget_mac_nswindow.mm :: -addChildWindow:ordered:` saves and
//      restores `childWin.level`, with the comment *"Attaching a window to be a
//      child window resets the window level"*. ⛔ Without this every
//      `NSPopUpMenuWindowLevel` dropdown silently drops to normal level.
//   3. `OrderChildren()` bails when the parent is not visible or not on the
//      active space — *"Adding a child to a window that isn't visible on the
//      active space will switch to that space"* (crbug 783521, 798792).
//   4. `SetVisible` REMOVES the child from its parent when it becomes invisible:
//      *"Cocoa's childWindow management breaks down when child windows are
//      hidden."* That is why the hide path detaches rather than re-parenting
//      back to the primary the way Windows does.
static void OwnOverlayToRequestingWindowMac(NSWindow* overlay, BrowserWindow* targetWin) {
    if (!overlay) return;
    NSWindow* newParent = OverlayHostWindow(targetWin);
    if (!newParent) return;

    NSWindow* current = [overlay parentWindow];
    if (current == newParent) return;
    if (current) [current removeChildWindow:overlay];          // (1)
    if (![newParent isVisible] || ![newParent isOnActiveSpace]) return;  // (3)

    NSInteger level = [overlay level];                          // (2)
    [newParent addChildWindow:overlay ordered:NSWindowAbove];
    [overlay setLevel:level];
}

// The hide half. ⚠️ Detaches rather than handing back to the primary: on macOS a
// parentless window is a normal state, and Cocoa's own child bookkeeping is
// documented to misbehave for hidden children (4). The next show re-attaches it
// to whichever window asks then.
static void DetachOverlayFromParentMac(NSWindow* overlay) {
    if (!overlay) return;
    NSWindow* current = [overlay parentWindow];
    if (current) [current removeChildWindow:overlay];
}

// Safety net for the one case the show/hide pair cannot cover: a window closed
// while one of its overlays is still attached. AppKit orders a child out with
// its parent, so the overlay would stay alive but invisible with every
// `g_*_overlay_window` still pointing at it.
// ⛔ An overlay added to the child-window set later is NOT covered unless it is
// added here in the same change — same contract as Windows' ReleaseOverlaysOwnedBy.
void ReleaseOverlaysOwnedByMac(NSWindow* closing) {
    if (!closing) return;
    NSWindow* overlays[] = {
        g_cookie_panel_overlay_window, g_wallet_overlay_window,
        g_omnibox_overlay_window, g_menu_overlay_window,
    };
    int moved = 0;
    for (NSWindow* ov : overlays) {
        if (!ov || [ov parentWindow] != closing) continue;
        [closing removeChildWindow:ov];
        [ov orderOut:nil];
        moved++;
    }
    if (moved > 0) {
        LOG_INFO("Detached " + std::to_string(moved) +
                 " overlay(s) from a closing window (macOS P3.5)");
    }
}

// ============================================================================
// Overlay Window Creation Functions
// ============================================================================

void CreateSettingsOverlayWithSeparateProcess(int iconRightOffset) {
    LOG_INFO("Creating settings overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_settings_icon_right_offset = iconRightOffset;

    CGFloat panelWidth = 450;
    CGFloat panelHeight = 450;
    NSRect panelFrame = CalculateToolbarOverlayFrame(g_main_window, panelWidth, panelHeight, kMacHeaderHeightPt);

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
        // D-h2: drop the parent link once hidden — Cocoa's child bookkeeping is
        // documented to break down for hidden children, and the next show
        // re-attaches to whichever window asks then.
        DetachOverlayFromParentMac(g_cookie_panel_overlay_window);
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
            if (!OverlayHitsContent(g_cookie_panel_overlay_window, screenLocation)) {  // transparent pixel = outside
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

void ShowCookiePanelOverlay(int iconRightOffset, BrowserWindow* targetWin) {
    if (g_cookie_panel_overlay_window) {
        g_mac_cookie_panel_icon_right_offset = iconRightOffset;

        // D-h2: anchor to and hang under the window that asked, BEFORE showing, so
        // the primary is never the reference frame and never pulled forward.
        NSWindow* host = OverlayHostWindow(targetWin);
        OwnOverlayToRequestingWindowMac(g_cookie_panel_overlay_window, targetWin);

        // Reposition: flush right, flush below header
        NSRect panelFrame = CalculateToolbarOverlayFrame(host, 400, 500, kMacHeaderHeightPt);
        [g_cookie_panel_overlay_window setFrame:panelFrame display:YES];
        [g_cookie_panel_overlay_window makeKeyAndOrderFront:nil];
        InstallCookiePanelClickOutsideMonitor();
        LOG_INFO("Cookie panel overlay shown (macOS)");
    }
}

void CreateCookiePanelOverlayWithSeparateProcess(int iconRightOffset, BrowserWindow* targetWin) {
    LOG_INFO("Creating cookie panel overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_cookie_panel_icon_right_offset = iconRightOffset;

    // D-h2: the CREATE path needs the requesting window too — Windows' `P3.5-A7`
    // measured the omnibox being created at A-relative coordinates and only
    // corrected on the later Show, which masks the defect for everything except
    // the first open.
    NSWindow* host = OverlayHostWindow(targetWin);

    CGFloat panelWidth = 400;
    CGFloat panelHeight = 500;
    NSRect panelFrame = CalculateToolbarOverlayFrame(host, panelWidth, panelHeight, kMacHeaderHeightPt);

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

    // Make this a child window of the REQUESTING window (D-h2), not of the primary.
    OwnOverlayToRequestingWindowMac(g_cookie_panel_overlay_window, targetWin);

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

    // D-h2: detach from whatever window owns it — after the port that is not
    // necessarily the primary, and removeChildWindow: on the wrong parent is a no-op
    // that would leave the link dangling.
    DetachOverlayFromParentMac(g_wallet_overlay_window);
    [g_wallet_overlay_window orderOut:nil];
    [g_wallet_overlay_window close];
    g_wallet_overlay_window = nullptr;
}

void CreateWalletOverlayWithSeparateProcess(int iconRightOffset, BrowserWindow* targetWin) {
    LOG_INFO("Creating wallet overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));

    if (!g_main_window || ![g_main_window isVisible] || [g_main_window frame].size.width < 100) {
        LOG_WARNING("Wallet overlay skipped — main window not ready (visible=" +
            std::string(g_main_window && [g_main_window isVisible] ? "yes" : "no") +
            " width=" + std::to_string(g_main_window ? (int)[g_main_window frame].size.width : 0) + ")");
        return;
    }

    g_mac_wallet_icon_right_offset = iconRightOffset;

    // D-h2: the wallet is full-height, so BOTH its anchor and its height come from
    // the requesting window — a torn-off window is shorter than the primary (697 vs
    // 795 pt as measured), and taking the height from the primary would overhang it.
    NSWindow* host = OverlayHostWindow(targetWin);

    // Position: fixed-width panel, flush right, flush below header, full remaining height
    CGFloat walletWidth = 400;
    NSRect contentScreen = [host convertRectToScreen:[[host contentView] frame]];
    CGFloat walletHeight = contentScreen.size.height - kMacHeaderHeightPt;
    NSRect walletFrame = CalculateToolbarOverlayFrame(host, walletWidth, walletHeight, kMacHeaderHeightPt);
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

    // Child window of the REQUESTING window (moves/minimizes together) — D-h2
    OwnOverlayToRequestingWindowMac(g_wallet_overlay_window, targetWin);

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
    DetachOverlayFromParentMac(g_wallet_overlay_window);  // D-h2
}

void ShowWalletOverlay(BrowserWindow* targetWin = nullptr) {
    if (!g_wallet_overlay_window) return;
    LOG_INFO("Showing wallet overlay (macOS)");

    // D-h2: re-anchor and re-parent to the window that asked, before showing.
    NSWindow* host = OverlayHostWindow(targetWin);
    OwnOverlayToRequestingWindowMac(g_wallet_overlay_window, targetWin);
    NSRect hostContent = [host convertRectToScreen:[[host contentView] frame]];
    [g_wallet_overlay_window setFrame:CalculateToolbarOverlayFrame(
        host, 400, hostContent.size.height - kMacHeaderHeightPt,
        kMacHeaderHeightPt) display:YES];

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
// Allowlist of payment schemes: bitcoin: and bsv:. Do NOT widen to "any scheme"
// — the scheme signals intent-to-pay (money path). See TICKET_qr_bsv_uri_scheme_rejected.md.
// Windows twin: QRScreenCapture.cpp (keep the two in lockstep).
static const std::regex RE_BIP21(R"(^(bitcoin|bsv):)", std::regex_constants::icase);
// A BIP21 amount is emitted UNQUOTED into JSON that is later concatenated into
// JavaScript run in the wallet overlay, so it MUST be a plain decimal number.
// See MEASUREMENT_amount_injection.md.
static const std::regex RE_BIP21_AMOUNT(R"(^[0-9]+(\.[0-9]+)?$)");

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
        // Emit amount ONLY if it is a plain decimal number — it is unquoted in the
        // JSON that becomes JavaScript downstream. Anything else is dropped.
        if (!amount.empty() && std::regex_match(amount, RE_BIP21_AMOUNT))
            json += ",\"amount\":" + amount;
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

    // P0.5 M2 (Mac 2026-08-22): overlay ROLE must be "brc100auth" (no underscore)
    // to match every role consumer — BrowserWindow's role slot, the 9 role arms in
    // simple_handler.cpp (OnAfterCreated inject, OnLoadingStateChange auth-data
    // send, the close paths), and the self-nav grant gate. Windows already uses
    // "brc100auth" (simple_app.cpp). The underscore left this overlay's role slot
    // unset and those branches dead on macOS. NOTE: the modal/prompt-TYPE string
    // "brc100_auth" (HttpRequestInterceptor / PendingAuthRequest) is a DIFFERENT,
    // internally-consistent namespace and is deliberately left untouched.
    CefRefPtr<SimpleHandler> handler(new SimpleHandler("brc100auth"));
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

    // beta.3 P0.9 — if a permission prompt is parked and something ELSE is taking
    // the shared overlay, latch it now so the prompt can be re-shown when the
    // overlay is released. Recorded here (the single choke point that knows the
    // incoming `type`) rather than inferred at close time, which races.
    if (type != "permission_request" && type != "preload") {
        PendingPermissionManager::GetInstance().markPreempted();
    }

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
            // ⛔ escapeJsonForJs, NOT a hand-rolled '-only escape. (P0.5 panel #3
            // — modal query-string JS injection.) Mirrors the Windows fix in
            // simple_app.cpp :: CreateNotificationOverlay — the '-only loop left
            // `\` unescaped, so a backslash in a dApp-controlled query value broke
            // out into arbitrary JS at 127.0.0.1:5137. escapeJsonForJs leaves
            // `&`/`=` intact so React still parses the query params.
            const std::string safeQuery = escapeJsonForJs(queryString);

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
            if (!OverlayHitsContent(g_settings_menu_overlay_window, screenLocation)) {  // transparent pixel = outside
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
    NSRect menuFrame = CalculateToolbarOverlayFrame(g_main_window, menuWidth, menuHeight, kMacHeaderHeightPt);

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
            if (!OverlayHitsContent(g_omnibox_overlay_window, screenLocation)) {  // transparent pixel = outside
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
        DetachOverlayFromParentMac(g_omnibox_overlay_window);  // D-h2
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

void ShowOmniboxOverlayMacOS(BrowserWindow* targetWin) {
    if (g_omnibox_overlay_window) {
        // D-h2: re-anchor and re-parent to the requesting window first.
        NSWindow* host = OverlayHostWindow(targetWin);
        OwnOverlayToRequestingWindowMac(g_omnibox_overlay_window, targetWin);

        // Reposition in case window moved/resized since creation
        NSRect contentScreen = [host convertRectToScreen:[[host contentView] frame]];
        int omniboxWidth = (int)(contentScreen.size.width * 0.69);
        if (omniboxWidth < 400) omniboxWidth = 400;
        int omniboxHeight = 420;
        CGFloat overlayX = contentScreen.origin.x + (contentScreen.size.width - omniboxWidth) / 2;
        CGFloat contentTop = contentScreen.origin.y + contentScreen.size.height;
        CGFloat overlayY = contentTop - kMacHeaderHeightPt - omniboxHeight;
        [g_omnibox_overlay_window setFrame:NSMakeRect(overlayX, overlayY, omniboxWidth, omniboxHeight) display:YES];

        [g_omnibox_overlay_window orderFront:nil];
        InstallOmniboxClickOutsideMonitor();
        LOG_INFO("Omnibox overlay shown (macOS)");
    }
}

void CreateOmniboxOverlayMacOS(BrowserWindow* targetWin) {
    LOG_INFO("Creating omnibox overlay (macOS)");

    // D-h2: ⭐ the omnibox is the row Windows called `P3.5-A7` — its CREATE path
    // positioned against the primary and the later Show corrected it, so the defect
    // was only visible on the first open. Both paths take the requesting window here.
    NSWindow* host = OverlayHostWindow(targetWin);
    NSRect contentScreen = [host convertRectToScreen:[[host contentView] frame]];
    // Position below header (kMacHeaderHeightPt) spanning most of the window width
    int omniboxWidth = (int)(contentScreen.size.width * 0.69);
    if (omniboxWidth < 400) omniboxWidth = 400;
    int omniboxHeight = 420;
    // Center horizontally, position below header
    CGFloat overlayX = contentScreen.origin.x + (contentScreen.size.width - omniboxWidth) / 2;
    CGFloat contentTop = contentScreen.origin.y + contentScreen.size.height;
    CGFloat overlayY = contentTop - kMacHeaderHeightPt - omniboxHeight;
    NSRect omniboxFrame = NSMakeRect(overlayX, overlayY, omniboxWidth, omniboxHeight);

    // Keep-alive: don't destroy existing window
    if (g_omnibox_overlay_window) {
        ShowOmniboxOverlayMacOS(targetWin);
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

    // CRITICAL: child of the REQUESTING window (D-h2) so it doesn't steal focus from
    // that window's address bar — parenting it to the primary is what made a torn-off
    // window's omnibox open over the primary.
    OwnOverlayToRequestingWindowMac(g_omnibox_overlay_window, targetWin);

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
            if (!OverlayHitsContent(g_download_panel_overlay_window, screenLocation)) {  // transparent pixel = outside
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

void ShowDownloadPanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin) {
    if (g_download_panel_overlay_window) {
        g_mac_download_panel_icon_right_offset = iconRightOffset;

        // D-h2: geometry only — this overlay is not an addChildWindow: child, so
        // it already has no parent to drag forward.
        NSRect panelFrame = CalculateToolbarOverlayFrame(OverlayHostWindow(targetWin), 400, 500, kMacHeaderHeightPt);
        [g_download_panel_overlay_window setFrame:panelFrame display:YES];

        [g_download_panel_overlay_window makeKeyAndOrderFront:nil];
        InstallDownloadPanelClickOutsideMonitor();
        LOG_INFO("Download panel overlay shown (macOS)");
    }
}

void CreateDownloadPanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin) {
    LOG_INFO("Creating download panel overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_download_panel_icon_right_offset = iconRightOffset;

    NSWindow* host = OverlayHostWindow(targetWin);  // D-h2
    CGFloat panelWidth = 400;
    CGFloat panelHeight = 500;
    NSRect panelFrame = CalculateToolbarOverlayFrame(host, panelWidth, panelHeight, kMacHeaderHeightPt);

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
            if (g_file_dialog_active) {
                return event;
            }
            NSPoint screenLocation = [NSEvent mouseLocation];
            if (!OverlayHitsContent(g_profile_panel_overlay_window, screenLocation)) {  // transparent pixel = outside
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

// ⛔ The overlay's NSWindow size and the CEF view size are ONE contract. The window is
// what the user clicks; the view is the coordinate space the DOM receives. If they
// disagree, every click lands on the wrong element, with the error growing the further
// it is from the top-left — the macOS twin of the Windows P1 coordinate defect.
//
// MEASURED 2026-08-26: ShowProfilePanelOverlayMacOS resized the window to a hardcoded
// 300x400 while CreateProfilePanelOverlayMacOS built a 380x520 view, so the FIRST open
// was correct and every RE-open was 26.7% out horizontally and 30% vertically. Reported
// by the owner as "I click Edit and it opens the profile instead" — at the bottom of the
// panel that is ~120px of drift. Differential CGWindowList capture: opening the panel
// added a 300x400 window whose DOM reported 380x520.
//
// These constants exist so the two paths cannot drift again. Do not inline them.
static const CGFloat kProfilePanelWidth  = 380;
static const CGFloat kProfilePanelHeight = 520;

void ShowProfilePanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin) {
    if (g_profile_panel_overlay_window) {
        g_mac_profile_panel_icon_right_offset = iconRightOffset;

        NSRect panelFrame = CalculateToolbarOverlayFrame(          // D-h2
            OverlayHostWindow(targetWin), kProfilePanelWidth, kProfilePanelHeight, kMacHeaderHeightPt);
        [g_profile_panel_overlay_window setFrame:panelFrame display:YES];

        [g_profile_panel_overlay_window makeKeyAndOrderFront:nil];
        InstallProfilePanelClickOutsideMonitor();
        LOG_INFO("Profile panel overlay shown (macOS)");
    }
}

void CreateProfilePanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin) {
    LOG_INFO("Creating profile panel overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
    g_mac_profile_panel_icon_right_offset = iconRightOffset;

    NSWindow* host = OverlayHostWindow(targetWin);  // D-h2
    CGFloat panelWidth = kProfilePanelWidth;
    CGFloat panelHeight = kProfilePanelHeight;
    NSRect panelFrame = CalculateToolbarOverlayFrame(host, panelWidth, panelHeight, kMacHeaderHeightPt);

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
// Left-anchored overlay frame helper (for bookmarks, site-info, tab-list)
// ============================================================================

static NSRect CalculateLeftAnchoredOverlayFrame(NSWindow* mainWindow, CGFloat width, CGFloat height, CGFloat toolbarHeight, int iconLeftOffset) {
    NSRect contentScreen = [mainWindow convertRectToScreen:[[mainWindow contentView] frame]];
    CGFloat x = contentScreen.origin.x + (CGFloat)iconLeftOffset;
    CGFloat contentTop = contentScreen.origin.y + contentScreen.size.height;
    CGFloat y = contentTop - toolbarHeight - height;

    // Clamp to screen
    NSRect screenFrame = [[mainWindow screen] visibleFrame];
    if (x + width > NSMaxX(screenFrame)) x = NSMaxX(screenFrame) - width;
    if (x < NSMinX(screenFrame)) x = NSMinX(screenFrame);
    if (y < NSMinY(screenFrame)) y = NSMinY(screenFrame);

    return NSMakeRect(x, y, width, height);
}

static NSRect CalculateRightAnchoredOverlayFrame(NSWindow* mainWindow, CGFloat width, CGFloat height, CGFloat toolbarHeight, int iconRightOffset) {
    NSRect contentScreen = [mainWindow convertRectToScreen:[[mainWindow contentView] frame]];
    CGFloat contentRight = contentScreen.origin.x + contentScreen.size.width;
    CGFloat x = contentRight - (CGFloat)iconRightOffset - width;
    CGFloat contentTop = contentScreen.origin.y + contentScreen.size.height;
    CGFloat y = contentTop - toolbarHeight - height;

    NSRect screenFrame = [[mainWindow screen] visibleFrame];
    if (x + width > NSMaxX(screenFrame)) x = NSMaxX(screenFrame) - width;
    if (x < NSMinX(screenFrame)) x = NSMinX(screenFrame);
    if (y < NSMinY(screenFrame)) y = NSMinY(screenFrame);

    return NSMakeRect(x, y, width, height);
}

// ============================================================================
// Bookmarks Panel Overlay (macOS) — A5
// ============================================================================

static void RemoveBookmarksPanelClickOutsideMonitor() {
    if (g_bookmarks_panel_click_monitor) {
        [NSEvent removeMonitor:g_bookmarks_panel_click_monitor];
        g_bookmarks_panel_click_monitor = nil;
    }
}

static void InstallBookmarksPanelClickOutsideMonitor() {
    if (g_bookmarks_panel_click_monitor) return;

    g_bookmarks_panel_click_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (!g_bookmarks_panel_overlay_window || ![g_bookmarks_panel_overlay_window isVisible]) {
                return event;
            }
            if (g_file_dialog_active) {
                return event;
            }
            NSPoint screenLocation = [NSEvent mouseLocation];
            if (!OverlayHitsContent(g_bookmarks_panel_overlay_window, screenLocation)) {  // transparent pixel = outside
                [g_bookmarks_panel_overlay_window orderOut:nil];
                RemoveBookmarksPanelClickOutsideMonitor();
                g_bookmarks_panel_last_hide_time = CFAbsoluteTimeGetCurrent();
            }
            return event;
        }];
}

void HideBookmarksPanelOverlayMacOS() {
    if (g_bookmarks_panel_overlay_window) {
        [g_bookmarks_panel_overlay_window orderOut:nil];
        RemoveBookmarksPanelClickOutsideMonitor();
        g_bookmarks_panel_last_hide_time = CFAbsoluteTimeGetCurrent();
        LOG_INFO("Bookmarks panel overlay hidden (macOS)");
    }
}

bool IsBookmarksPanelOverlayVisible() {
    return g_bookmarks_panel_overlay_window && [g_bookmarks_panel_overlay_window isVisible];
}

bool WasBookmarksPanelJustHidden() {
    CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
    return (now - g_bookmarks_panel_last_hide_time) < 0.3;
}

void ShowBookmarksPanelOverlayMacOS(int iconLeftOffset, BrowserWindow* targetWin) {
    if (g_bookmarks_panel_overlay_window) {
        NSRect panelFrame = CalculateLeftAnchoredOverlayFrame(       // D-h2
            OverlayHostWindow(targetWin), 380, 520, kMacHeaderHeightPt, iconLeftOffset);
        [g_bookmarks_panel_overlay_window setFrame:panelFrame display:YES];

        [g_bookmarks_panel_overlay_window makeKeyAndOrderFront:nil];
        NSView* cv = [g_bookmarks_panel_overlay_window contentView];
        [g_bookmarks_panel_overlay_window makeFirstResponder:cv];
        InstallBookmarksPanelClickOutsideMonitor();
        LOG_INFO("Bookmarks panel overlay shown (macOS)");
    }
}

void CreateBookmarksPanelOverlayMacOS(int iconLeftOffset, BrowserWindow* targetWin) {
    LOG_INFO("Creating bookmarks panel overlay (macOS) iconLeftOffset=" + std::to_string(iconLeftOffset));

    NSWindow* host = OverlayHostWindow(targetWin);  // D-h2
    CGFloat panelWidth = 380;
    CGFloat panelHeight = 520;
    NSRect panelFrame = CalculateLeftAnchoredOverlayFrame(host, panelWidth, panelHeight, kMacHeaderHeightPt, iconLeftOffset);

    if (g_bookmarks_panel_overlay_window) {
        [g_bookmarks_panel_overlay_window close];
        g_bookmarks_panel_overlay_window = nullptr;
    }

    g_bookmarks_panel_overlay_window = [[DropdownOverlayWindow alloc]
        initWithContentRect:panelFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_bookmarks_panel_overlay_window) {
        LOG_ERROR("Failed to create bookmarks panel overlay window");
        return;
    }

    [g_bookmarks_panel_overlay_window setOpaque:NO];
    [g_bookmarks_panel_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_bookmarks_panel_overlay_window setLevel:NSPopUpMenuWindowLevel];
    [g_bookmarks_panel_overlay_window setIgnoresMouseEvents:NO];
    [g_bookmarks_panel_overlay_window setReleasedWhenClosed:NO];
    [g_bookmarks_panel_overlay_window setHasShadow:YES];
    [g_bookmarks_panel_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];
    [g_bookmarks_panel_overlay_window setAcceptsMouseMovedEvents:YES];

    DropdownOverlayView* contentView = [[DropdownOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, panelWidth, panelHeight)];
    contentView.browserAccessor = ^CefRefPtr<CefBrowser>{ return SimpleHandler::GetBookmarksPanelBrowser(); };
    [g_bookmarks_panel_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("bookmarkspanel"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView, (int)panelWidth, (int)panelHeight);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info, handler,
        "http://127.0.0.1:5137/bookmarks",
        settings, nullptr, CefRequestContext::GetGlobalContext());

    if (!result) {
        LOG_ERROR("Failed to create bookmarks panel overlay CEF browser");
        return;
    }

    [g_bookmarks_panel_overlay_window makeKeyAndOrderFront:nil];
    [g_bookmarks_panel_overlay_window makeFirstResponder:contentView];
    InstallBookmarksPanelClickOutsideMonitor();
    LOG_INFO("Bookmarks panel overlay created successfully");
}

// ============================================================================
// Site-Info Panel Overlay (macOS) — A6
// ============================================================================

static void RemoveSiteInfoPanelClickOutsideMonitor() {
    if (g_siteinfo_panel_click_monitor) {
        [NSEvent removeMonitor:g_siteinfo_panel_click_monitor];
        g_siteinfo_panel_click_monitor = nil;
    }
}

static void InstallSiteInfoPanelClickOutsideMonitor() {
    if (g_siteinfo_panel_click_monitor) return;

    g_siteinfo_panel_click_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (!g_siteinfo_panel_overlay_window || ![g_siteinfo_panel_overlay_window isVisible]) {
                return event;
            }
            if (g_file_dialog_active) {
                return event;
            }
            NSPoint screenLocation = [NSEvent mouseLocation];
            if (!OverlayHitsContent(g_siteinfo_panel_overlay_window, screenLocation)) {  // transparent pixel = outside
                [g_siteinfo_panel_overlay_window orderOut:nil];
                RemoveSiteInfoPanelClickOutsideMonitor();
                g_siteinfo_panel_last_hide_time = CFAbsoluteTimeGetCurrent();
            }
            return event;
        }];
}

void HideSiteInfoPanelOverlayMacOS() {
    if (g_siteinfo_panel_overlay_window) {
        [g_siteinfo_panel_overlay_window orderOut:nil];
        RemoveSiteInfoPanelClickOutsideMonitor();
        g_siteinfo_panel_last_hide_time = CFAbsoluteTimeGetCurrent();
        LOG_INFO("Site-info panel overlay hidden (macOS)");
    }
}

bool IsSiteInfoPanelOverlayVisible() {
    return g_siteinfo_panel_overlay_window && [g_siteinfo_panel_overlay_window isVisible];
}

bool WasSiteInfoPanelJustHidden() {
    CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
    return (now - g_siteinfo_panel_last_hide_time) < 0.3;
}

void ShowSiteInfoPanelOverlayMacOS(int iconLeftOffset, BrowserWindow* targetWin) {
    if (g_siteinfo_panel_overlay_window) {
        NSRect panelFrame = CalculateLeftAnchoredOverlayFrame(       // D-h2
            OverlayHostWindow(targetWin), 360, 480, kMacHeaderHeightPt, iconLeftOffset);
        [g_siteinfo_panel_overlay_window setFrame:panelFrame display:YES];

        [g_siteinfo_panel_overlay_window makeKeyAndOrderFront:nil];
        InstallSiteInfoPanelClickOutsideMonitor();
        LOG_INFO("Site-info panel overlay shown (macOS)");
    }
}

void CreateSiteInfoPanelOverlayMacOS(int iconLeftOffset, BrowserWindow* targetWin) {
    LOG_INFO("Creating site-info panel overlay (macOS) iconLeftOffset=" + std::to_string(iconLeftOffset));

    NSWindow* host = OverlayHostWindow(targetWin);  // D-h2
    CGFloat panelWidth = 360;
    CGFloat panelHeight = 480;
    NSRect panelFrame = CalculateLeftAnchoredOverlayFrame(host, panelWidth, panelHeight, kMacHeaderHeightPt, iconLeftOffset);

    if (g_siteinfo_panel_overlay_window) {
        [g_siteinfo_panel_overlay_window close];
        g_siteinfo_panel_overlay_window = nullptr;
    }

    g_siteinfo_panel_overlay_window = [[DropdownOverlayWindow alloc]
        initWithContentRect:panelFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_siteinfo_panel_overlay_window) {
        LOG_ERROR("Failed to create site-info panel overlay window");
        return;
    }

    [g_siteinfo_panel_overlay_window setOpaque:NO];
    [g_siteinfo_panel_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_siteinfo_panel_overlay_window setLevel:NSPopUpMenuWindowLevel];
    [g_siteinfo_panel_overlay_window setIgnoresMouseEvents:NO];
    [g_siteinfo_panel_overlay_window setReleasedWhenClosed:NO];
    [g_siteinfo_panel_overlay_window setHasShadow:YES];
    [g_siteinfo_panel_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];
    [g_siteinfo_panel_overlay_window setAcceptsMouseMovedEvents:YES];

    DropdownOverlayView* contentView = [[DropdownOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, panelWidth, panelHeight)];
    contentView.browserAccessor = ^CefRefPtr<CefBrowser>{ return SimpleHandler::GetSiteInfoPanelBrowser(); };
    [g_siteinfo_panel_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("siteinfopanel"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView, (int)panelWidth, (int)panelHeight);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info, handler,
        "http://127.0.0.1:5137/site-info",
        settings, nullptr, CefRequestContext::GetGlobalContext());

    if (!result) {
        LOG_ERROR("Failed to create site-info panel overlay CEF browser");
        return;
    }

    [g_siteinfo_panel_overlay_window makeKeyAndOrderFront:nil];
    InstallSiteInfoPanelClickOutsideMonitor();
    LOG_INFO("Site-info panel overlay created successfully");
}

// ============================================================================
// Tab-List Panel Overlay (macOS) — A7
// ============================================================================

static void RemoveTabListPanelClickOutsideMonitor() {
    if (g_tablist_panel_click_monitor) {
        [NSEvent removeMonitor:g_tablist_panel_click_monitor];
        g_tablist_panel_click_monitor = nil;
    }
}

static void InstallTabListPanelClickOutsideMonitor() {
    if (g_tablist_panel_click_monitor) return;

    g_tablist_panel_click_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (!g_tablist_panel_overlay_window || ![g_tablist_panel_overlay_window isVisible]) {
                return event;
            }
            if (g_file_dialog_active) {
                return event;
            }
            NSPoint screenLocation = [NSEvent mouseLocation];
            if (!OverlayHitsContent(g_tablist_panel_overlay_window, screenLocation)) {  // transparent pixel = outside
                [g_tablist_panel_overlay_window orderOut:nil];
                RemoveTabListPanelClickOutsideMonitor();
                g_tablist_panel_last_hide_time = CFAbsoluteTimeGetCurrent();
            }
            return event;
        }];
}

void HideTabListPanelOverlayMacOS() {
    if (g_tablist_panel_overlay_window) {
        [g_tablist_panel_overlay_window orderOut:nil];
        RemoveTabListPanelClickOutsideMonitor();
        g_tablist_panel_last_hide_time = CFAbsoluteTimeGetCurrent();
        LOG_INFO("Tab-list panel overlay hidden (macOS)");
    }
}

bool IsTabListPanelOverlayVisible() {
    return g_tablist_panel_overlay_window && [g_tablist_panel_overlay_window isVisible];
}

bool WasTabListPanelJustHidden() {
    CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
    return (now - g_tablist_panel_last_hide_time) < 0.3;
}

void ShowTabListPanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin) {
    if (g_tablist_panel_overlay_window) {
        NSRect panelFrame = CalculateRightAnchoredOverlayFrame(      // D-h2
            OverlayHostWindow(targetWin), 340, 480, kMacHeaderHeightPt, iconRightOffset);
        [g_tablist_panel_overlay_window setFrame:panelFrame display:YES];

        [g_tablist_panel_overlay_window makeKeyAndOrderFront:nil];
        NSView* cv = [g_tablist_panel_overlay_window contentView];
        [g_tablist_panel_overlay_window makeFirstResponder:cv];
        InstallTabListPanelClickOutsideMonitor();

        // Inject tabListRefresh on (re)show
        CefRefPtr<CefBrowser> tl_browser = SimpleHandler::GetTabListPanelBrowser();
        if (tl_browser && tl_browser->GetMainFrame()) {
            tl_browser->GetMainFrame()->ExecuteJavaScript(
                "if(window.tabListRefresh)window.tabListRefresh();",
                tl_browser->GetMainFrame()->GetURL(), 0);
        }

        LOG_INFO("Tab-list panel overlay shown (macOS)");
    }
}

void CreateTabListPanelOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin) {
    LOG_INFO("Creating tab-list panel overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));

    NSWindow* host = OverlayHostWindow(targetWin);  // D-h2
    CGFloat panelWidth = 340;
    CGFloat panelHeight = 480;
    NSRect panelFrame = CalculateRightAnchoredOverlayFrame(host, panelWidth, panelHeight, kMacHeaderHeightPt, iconRightOffset);

    if (g_tablist_panel_overlay_window) {
        [g_tablist_panel_overlay_window close];
        g_tablist_panel_overlay_window = nullptr;
    }

    g_tablist_panel_overlay_window = [[DropdownOverlayWindow alloc]
        initWithContentRect:panelFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_tablist_panel_overlay_window) {
        LOG_ERROR("Failed to create tab-list panel overlay window");
        return;
    }

    [g_tablist_panel_overlay_window setOpaque:NO];
    [g_tablist_panel_overlay_window setBackgroundColor:[NSColor clearColor]];
    [g_tablist_panel_overlay_window setLevel:NSPopUpMenuWindowLevel];
    [g_tablist_panel_overlay_window setIgnoresMouseEvents:NO];
    [g_tablist_panel_overlay_window setReleasedWhenClosed:NO];
    [g_tablist_panel_overlay_window setHasShadow:YES];
    [g_tablist_panel_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];
    [g_tablist_panel_overlay_window setAcceptsMouseMovedEvents:YES];

    DropdownOverlayView* contentView = [[DropdownOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, panelWidth, panelHeight)];
    contentView.browserAccessor = ^CefRefPtr<CefBrowser>{ return SimpleHandler::GetTabListPanelBrowser(); };
    [g_tablist_panel_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> handler(new SimpleHandler("tablistpanel"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView, (int)panelWidth, (int)panelHeight);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info, handler,
        "http://127.0.0.1:5137/tab-list",
        settings, nullptr, CefRequestContext::GetGlobalContext());

    if (!result) {
        LOG_ERROR("Failed to create tab-list panel overlay CEF browser");
        return;
    }

    [g_tablist_panel_overlay_window makeKeyAndOrderFront:nil];
    [g_tablist_panel_overlay_window makeFirstResponder:contentView];
    InstallTabListPanelClickOutsideMonitor();
    LOG_INFO("Tab-list panel overlay created successfully");
}

// ============================================================================
// TAB CONTEXT MENU OVERLAY (overlay #15) — beta.3 Phase 4, macOS half
// ============================================================================
// Windows equivalent: simple_app.cpp :: Create/Show/HideTabContextMenuOverlay.
// Relayed as the one genuinely macOS-shaped item of Phase 4 in
// MAC_RELAY_P35_P4_ROUND.md (M3/M6). Everything else — the React page
// (/tab-context-menu), the four IPC arms, the `tabmenu` role slot, the target-tab
// bookkeeping and the mute state — is already cross-platform and starts working the
// moment this window exists.
//
// ⭐ WHAT MAKES THIS ONE DIFFERENT FROM THE OTHER 14: it is anchored to the CURSOR,
// not to a toolbar icon. The positioning helpers here take an anchor point; the
// CalculateToolbarOverlayFrame / CalculateRightAnchoredOverlayFrame pair every other
// dropdown uses does not apply.
//
// ⛔ NO `addChildWindow:`, and that is STILL right after the Phase 3.5 macOS port
// (`D-h2`, 2026-09-19). AppKit child windows order WITH their parent, so attaching to
// the process-global `g_main_window` is what made a dropdown opened in a secondary
// window drag the primary forward — MAC_RELAY_P35_P4_ROUND.md M2 predicted it and
// round f measured it. `D-h2` fixed that by making the parent FOLLOW the requesting
// window for the four overlays that are real children; this one keeps attaching to
// nothing, so it never had the coupling and still does not. It takes only the
// geometry half of the port (ComputeTabMenuFrameMac resolves the requesting window's
// frame AND header view). ⚠️ Attaching to nothing also means it does not inherit
// parent-window hide/minimise for free, which is why it is registered in BOTH
// InstallAppFocusLossHandler() and ShutdownApplication() below.
//
// ⚠️ NO DPI SCALING, deliberately — this is NOT an omission of the Windows ScalePx
// step. Windows converts CSS px -> physical px because its overlays are sized in
// physical pixels; on macOS a CEF OSR overlay is sized in POINTS and React CSS px are
// points, so the anchor arrives in the right unit already. Scaling it here would put
// the menu at roughly twice the offset on a Retina display.

// Menu geometry in points, pinned against the React side: 7 rows (7 x 32 = 224) +
// 1 divider block (9) + container padding (2 x 4) = 241.
// ⚠️ `ROW_HEIGHT` in TabContextMenuOverlayRoot.tsx carries the matching note. Adding a
// menu item without changing this clips the last row — same contract as Windows'
// kTabMenuWidthDip / kTabMenuHeightDip.
static const CGFloat kTabMenuWidthPt  = 240;
static const CGFloat kTabMenuHeightPt = 241;

// Cursor anchor (header-local, top-down CSS px) -> Cocoa screen rect (bottom-left origin).
static NSRect ComputeTabMenuFrameMac(int anchorX, int anchorY, BrowserWindow* targetWin) {
    // D-h2: the cursor anchor is relative to the REQUESTING window's header, so both
    // the window and the header view have to come from that window. Taking the header
    // from the process global while the anchor came from window B put the menu at B's
    // offset inside A's frame.
    NSWindow* host = OverlayHostWindow(targetWin);
    NSView* hostHeader = (targetWin && targetWin->header_view)
                             ? (__bridge NSView*)targetWin->header_view
                             : g_header_view;
    NSRect contentScreen =
        [host convertRectToScreen:[[host contentView] frame]];

    // The anchor is relative to the HEADER browser's viewport, so resolve the header
    // view's own screen rect rather than the window's content rect — they differ by the
    // title bar, and using the window would push the menu down by that much.
    NSRect anchorBase = contentScreen;
    if (hostHeader) {
        NSRect headerInWindow = [hostHeader convertRect:[hostHeader bounds] toView:nil];
        anchorBase = [host convertRectToScreen:headerInWindow];
    }

    CGFloat menuX = NSMinX(anchorBase) + (CGFloat)anchorX;
    // Cocoa's Y grows UPWARD and the anchor is measured DOWNWARD from the header's top,
    // so subtract. Then drop by the menu height, because an NSWindow frame's origin is
    // its BOTTOM-left corner while the menu should hang below the cursor.
    CGFloat menuTopY = NSMaxY(anchorBase) - (CGFloat)anchorY;
    CGFloat menuY = menuTopY - kTabMenuHeightPt;

    // Keep the menu inside the window it belongs to. Flipping left off the right edge is
    // what a native context menu does; clamping is enough vertically because the anchor
    // is always in the tab strip at the very top. Mirrors Windows' ComputeTabMenuRect.
    if (menuX + kTabMenuWidthPt > NSMaxX(contentScreen)) {
        menuX = NSMaxX(contentScreen) - kTabMenuWidthPt;
    }
    if (menuX < NSMinX(contentScreen)) menuX = NSMinX(contentScreen);
    if (menuY < NSMinY(contentScreen)) menuY = NSMinY(contentScreen);
    if (menuY + kTabMenuHeightPt > NSMaxY(contentScreen)) {
        menuY = NSMaxY(contentScreen) - kTabMenuHeightPt;
    }

    // Final guard: a window narrower than the menu, or dragged part-way off screen,
    // would otherwise place it outside the visible frame.
    return ClampOverlayToScreen(NSMakeRect(menuX, menuY, kTabMenuWidthPt, kTabMenuHeightPt));
}

static void RemoveTabMenuClickOutsideMonitor() {
    if (g_tabmenu_click_monitor) {
        [NSEvent removeMonitor:g_tabmenu_click_monitor];
        g_tabmenu_click_monitor = nil;
    }
    if (g_tabmenu_rclick_monitor) {
        [NSEvent removeMonitor:g_tabmenu_rclick_monitor];
        g_tabmenu_rclick_monitor = nil;
    }
}

static void InstallTabMenuClickOutsideMonitor() {
    if (g_tabmenu_click_monitor && g_tabmenu_rclick_monitor) return;

    // One block, two masks. Returning the event unmodified lets the click through, so a
    // right-click on another tab both dismisses this menu AND reaches the header, which
    // re-opens the menu on the new tab with the new target id.
    NSEvent* (^dismiss)(NSEvent*) = ^NSEvent*(NSEvent* event) {
        if (!g_tabmenu_overlay_window || ![g_tabmenu_overlay_window isVisible]) {
            return event;
        }
        // A native file dialog steals activation without the user having "clicked away";
        // g_file_dialog_active is the shared guard the other dropdowns honour.
        if (g_file_dialog_active) {
            return event;
        }
        NSPoint screenLocation = [NSEvent mouseLocation];
        if (!OverlayHitsContent(g_tabmenu_overlay_window, screenLocation)) {  // transparent pixel = outside
            HideTabContextMenuOverlayMacOS();
        }
        return event;
    };

    if (!g_tabmenu_click_monitor) {
        g_tabmenu_click_monitor =
            [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
                                                  handler:dismiss];
    }
    if (!g_tabmenu_rclick_monitor) {
        g_tabmenu_rclick_monitor =
            [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskRightMouseDown
                                                  handler:dismiss];
    }
}

void HideTabContextMenuOverlayMacOS() {
    if (!g_tabmenu_overlay_window) return;

    [g_tabmenu_overlay_window orderOut:nil];
    RemoveTabMenuClickOutsideMonitor();

    CefRefPtr<CefBrowser> tabmenu_browser = SimpleHandler::GetTabMenuBrowser();
    if (tabmenu_browser && tabmenu_browser->GetHost()) {
        tabmenu_browser->GetHost()->SetFocus(false);
    }

    // Hand the keyboard back to the header, or the tab strip stays dead after the menu
    // closes. Windows reads the owner HWND to pick the window; macOS has no owner here
    // (see the addChildWindow note above), so it goes to the header browser directly.
    CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
    if (header_browser && header_browser->GetHost()) {
        header_browser->GetHost()->SetFocus(true);
    }

    LOG_INFO("Tab context menu overlay hidden (macOS)");
}

void ShowTabContextMenuOverlayMacOS(int anchorX, int anchorY, BrowserWindow* targetWin) {
    if (!g_tabmenu_overlay_window) {
        LOG_WARNING("Cannot show tab context menu overlay - window does not exist");
        return;
    }

    // ⚠️ D-h2 geometry only — this overlay deliberately has NO addChildWindow: parent
    // (see the note above its creator), so there is nothing to re-own here.
    NSRect menuFrame = ComputeTabMenuFrameMac(anchorX, anchorY, targetWin);
    [g_tabmenu_overlay_window setFrame:menuFrame display:YES];
    [g_tabmenu_overlay_window makeKeyAndOrderFront:nil];

    NSView* contentView = [g_tabmenu_overlay_window contentView];
    [g_tabmenu_overlay_window makeFirstResponder:contentView];

    // The window moved; the OSR browser has to be told or it keeps painting at the old
    // size/scale after a move between displays of different backing scale.
    CefRefPtr<CefBrowser> tabmenu_browser = SimpleHandler::GetTabMenuBrowser();
    if (tabmenu_browser && tabmenu_browser->GetHost()) {
        tabmenu_browser->GetHost()->NotifyScreenInfoChanged();
        tabmenu_browser->GetHost()->WasResized();
        tabmenu_browser->GetHost()->Invalidate(PET_VIEW);
    }

    InstallTabMenuClickOutsideMonitor();
    LOG_INFO("Tab context menu overlay shown (macOS) at " +
             std::to_string((int)menuFrame.origin.x) + "," +
             std::to_string((int)menuFrame.origin.y));
}

void CreateTabContextMenuOverlayMacOS(int anchorX, int anchorY, BrowserWindow* targetWin) {
    LOG_INFO("Creating tab context menu overlay (macOS) anchor=" +
             std::to_string(anchorX) + "," + std::to_string(anchorY));

    if (!g_main_window) {
        LOG_ERROR("Cannot create tab context menu overlay: main window is null");
        return;
    }

    // Keep-alive, like every other overlay here: reuse the window and just reposition.
    if (g_tabmenu_overlay_window) {
        ShowTabContextMenuOverlayMacOS(anchorX, anchorY);
        return;
    }

    NSRect menuFrame = ComputeTabMenuFrameMac(anchorX, anchorY, targetWin);

    g_tabmenu_overlay_window = [[DropdownOverlayWindow alloc]
        initWithContentRect:menuFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!g_tabmenu_overlay_window) {
        LOG_ERROR("Failed to create tab context menu overlay window");
        return;
    }

    [g_tabmenu_overlay_window setOpaque:NO];
    [g_tabmenu_overlay_window setBackgroundColor:[NSColor clearColor]];
    // NSPopUpMenuWindowLevel so it sits above the other dropdowns, matching what a
    // native context menu does and mirroring Windows' HWND_TOPMOST.
    [g_tabmenu_overlay_window setLevel:NSPopUpMenuWindowLevel];
    [g_tabmenu_overlay_window setIgnoresMouseEvents:NO];
    [g_tabmenu_overlay_window setReleasedWhenClosed:NO];
    [g_tabmenu_overlay_window setHasShadow:YES];
    [g_tabmenu_overlay_window setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];
    [g_tabmenu_overlay_window setAcceptsMouseMovedEvents:YES];

    DropdownOverlayView* contentView = [[DropdownOverlayView alloc]
        initWithFrame:NSMakeRect(0, 0, kTabMenuWidthPt, kTabMenuHeightPt)];
    contentView.browserAccessor = ^CefRefPtr<CefBrowser>{
        return SimpleHandler::GetTabMenuBrowser();
    };
    [g_tabmenu_overlay_window setContentView:contentView];

    CefWindowInfo window_info;
    window_info.SetAsWindowless((__bridge void*)contentView);

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    // Role "tabmenu" — the same string the 9 role consumers and BrowserWindow's slot
    // use. ⚠️ GetTabMenuBrowser() reads a process STATIC rather than the window slot
    // (simple_handler.cpp), so this works on the first open before any ref is filed.
    CefRefPtr<SimpleHandler> handler(new SimpleHandler("tabmenu"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler((__bridge void*)contentView,
                                   (int)kTabMenuWidthPt, (int)kTabMenuHeightPt);
    handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info, handler,
        "http://127.0.0.1:5137/tab-context-menu",
        settings, nullptr, CefRequestContext::GetGlobalContext());

    if (!result) {
        LOG_ERROR("Failed to create tab context menu overlay CEF browser");
        [g_tabmenu_overlay_window close];
        g_tabmenu_overlay_window = nullptr;
        return;
    }

    [g_tabmenu_overlay_window makeKeyAndOrderFront:nil];
    [g_tabmenu_overlay_window makeFirstResponder:contentView];
    InstallTabMenuClickOutsideMonitor();

    // ⚠️ The page asks for its own context via tab_context_menu_request_context once its
    // React effect runs, and that arm answers the browser that asked — so nothing needs
    // to be pushed here, and pushing it would race the load.
    LOG_INFO("Tab context menu overlay created successfully (macOS)");
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

static void SaveSession() {
    auto browserSettings = SettingsManager::GetInstance().GetBrowserSettings();
    if (!browserSettings.restoreSessionOnStart) {
        LOG_INFO("Session restore disabled — skipping session save");
        return;
    }

    std::string profilePath = ProfileManager::GetInstance().GetCurrentProfileDataPath();
    if (profilePath.empty()) {
        LOG_WARNING("No profile path — cannot save session");
        return;
    }

    std::vector<Tab*> allTabs = TabManager::GetInstance().GetAllTabs();
    int activeTabId = TabManager::GetInstance().GetActiveTabId();
    std::vector<BrowserWindow*> windows = WindowManager::GetInstance().GetAllWindows();

    nlohmann::json sessionJson;
    sessionJson["version"] = 2;
    sessionJson["windows"] = nlohmann::json::array();

    int totalSavedTabs = 0;

    for (BrowserWindow* bw : windows) {
        if (!bw) continue;

        nlohmann::json winJson;
        winJson["tabs"] = nlohmann::json::array();
        winJson["activeTabIndex"] = 0;

        if (bw->ns_window) {
            NSWindow* nsWin = (__bridge NSWindow*)bw->ns_window;
            NSRect frame = [nsWin frame];
            winJson["x"] = static_cast<int>(frame.origin.x);
            winJson["y"] = static_cast<int>(frame.origin.y);
            winJson["width"] = static_cast<int>(frame.size.width);
            winJson["height"] = static_cast<int>(frame.size.height);
        }

        int tabIndex = 0;
        int activeIndex = 0;
        for (Tab* tab : allTabs) {
            if (!tab || tab->window_id != bw->window_id) continue;

            std::string url = tab->url;
            if (url.empty() || url == "about:blank") continue;
            if (url.find("127.0.0.1:5137") != std::string::npos) continue;

            nlohmann::json tabEntry;
            tabEntry["url"] = url;
            tabEntry["title"] = tab->title;
            winJson["tabs"].push_back(tabEntry);

            if (tab->id == activeTabId) {
                activeIndex = tabIndex;
            }
            tabIndex++;
        }

        winJson["activeTabIndex"] = activeIndex;

        if (!winJson["tabs"].empty()) {
            sessionJson["windows"].push_back(winJson);
            totalSavedTabs += static_cast<int>(winJson["tabs"].size());
        }
    }

    if (totalSavedTabs == 0) {
        LOG_INFO("No restorable tabs — skipping session save");
        return;
    }

    std::string sessionPath = profilePath + "/session.json";

    try {
        std::ofstream out(sessionPath);
        if (out.is_open()) {
            out << sessionJson.dump(2);
            out.close();
            LOG_INFO("Session saved: " + std::to_string(totalSavedTabs) + " tabs across " +
                     std::to_string(sessionJson["windows"].size()) + " windows to " + sessionPath);
        } else {
            LOG_ERROR("Failed to open session.json for writing: " + sessionPath);
        }
    } catch (const std::exception& e) {
        LOG_ERROR("Failed to save session: " + std::string(e.what()));
    }
}

static void ClearBrowsingDataOnExit() {
    auto privacySettings = SettingsManager::GetInstance().GetPrivacySettings();
    if (!privacySettings.clearDataOnExit) {
        return;
    }

    LOG_INFO("Clear-on-exit enabled — clearing browsing data...");

    try {
        if (HistoryManager::GetInstance().DeleteAllHistory()) {
            LOG_INFO("History cleared");
        }
    } catch (const std::exception& e) {
        LOG_ERROR("History clear exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR("History clear unknown exception");
    }

    try {
        CefRefPtr<CefCookieManager> cookieMgr = CefCookieManager::GetGlobalManager(nullptr);
        if (cookieMgr) {
            cookieMgr->DeleteCookies("", "", nullptr);
            LOG_INFO("Cookie deletion requested");
        }
    } catch (const std::exception& e) {
        LOG_ERROR("Cookie clear exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR("Cookie clear unknown exception");
    }

    try {
        CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
        if (header_browser) {
            header_browser->GetHost()->ExecuteDevToolsMethod(0, "Network.clearBrowserCache", nullptr);
            LOG_INFO("Cache clear requested via CDP");
        }
    } catch (const std::exception& e) {
        LOG_ERROR("Cache clear exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR("Cache clear unknown exception");
    }

    try {
        if (CookieBlockManager::GetInstance().ClearBlockLog()) {
            LOG_INFO("Cookie block log cleared");
        }
    } catch (const std::exception& e) {
        LOG_ERROR("Block log clear exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR("Block log clear unknown exception");
    }

    try {
        std::string profilePath = ProfileManager::GetInstance().GetCurrentProfileDataPath();
        if (!profilePath.empty()) {
            std::string sessionPath = profilePath + "/session.json";
            if (std::filesystem::exists(sessionPath)) {
                std::filesystem::remove(sessionPath);
                LOG_INFO("session.json deleted (clear-on-exit overrides session restore)");
            }
        }
    } catch (const std::exception& e) {
        LOG_ERROR("Session delete exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR("Session delete unknown exception");
    }

    LOG_INFO("Clear-on-exit complete");
}

void ShutdownApplication() {
    // Guard against re-entrant calls (terminate: → ShutdownApplication → terminate:)
    static bool shutting_down = false;
    if (shutting_down) return;
    shutting_down = true;

    LOG_INFO("🛑 Starting graceful application shutdown (macOS)...");

    SaveSession();
    ClearBrowsingDataOnExit();

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
            "wallet", "brc100auth", "notification", "settings_menu",
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

    // Overlay #15 (tab context menu). ⚠️ It attaches to no parent window by design, so
    // unlike the child-window overlays it is NOT torn down for free — Phase 3.5's K12
    // hazard, in its macOS form. Its event monitors must go with it: a live NSEvent
    // monitor whose block captures a freed window is a crash at the next click.
    if (g_tabmenu_overlay_window) {
        LOG_INFO("🔄 Closing tab context menu overlay window...");
        RemoveTabMenuClickOutsideMonitor();
        [g_tabmenu_overlay_window close];
        g_tabmenu_overlay_window = nullptr;
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
    // ⛔ Phase 8d: stop the supervisor BEFORE the SIGTERMs below. This is the earlier of the
    // two macOS shutdown paths (the other is StopServers() after the message loop exits);
    // without this the supervisor observes the SIGTERM as a crash and respawns the wallet
    // while the browser is tearing down. StopBackendSupervisor() is an idempotent flag set,
    // so calling it on both paths is correct.
    StopBackendSupervisor();

    if (g_wallet_server_pid > 0) {
        LOG_INFO("🔄 Killing wallet server (pid " + std::to_string(g_wallet_server_pid) + ")...");
        kill(g_wallet_server_pid, SIGTERM);
    }
    if (g_adblock_server_pid > 0) {
        LOG_INFO("🔄 Killing adblock server (pid " + std::to_string(g_adblock_server_pid) + ")...");
        kill(g_adblock_server_pid, SIGTERM);
    }

    LOG_INFO("✅ Application shutdown complete (macOS) — exiting message loop...");

    // Quit the CEF message loop, then exit the process.
    // Do NOT call [NSApp terminate:nil] — that re-enters our terminate: override.
    CefQuitMessageLoop();

    // Safety net: if CefQuitMessageLoop doesn't cause CefRunMessageLoop to return
    // within 5 seconds, force-exit. The post-message-loop cleanup (DB shutdown,
    // server stop, lock release) runs BEFORE this fires in the normal path —
    // _exit here is a last resort, not the happy path.
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(5.0 * NSEC_PER_SEC)),
                   dispatch_get_main_queue(), ^{
        LOG_ERROR("Force-exiting after 5s — CefRunMessageLoop did not return");
        _exit(0);
    });
}

// ============================================================================
// Health Check Functions (macOS) — uses SyncHttpClient (libcurl)
// ============================================================================

// Returns true if wallet server at localhost:31301 responds with "ok"
static bool QuickHealthCheck() {
    HttpResponse resp = SyncHttpClient::Get(hodos::WalletUrl("/health"), 2000);
    return resp.success && resp.body.find("\"ok\"") != std::string::npos;
}

// Returns true if adblock engine at localhost:31302 responds with "ready"
static bool QuickAdblockHealthCheck() {
    HttpResponse resp = SyncHttpClient::Get(hodos::AdblockUrl("/health"), 2000);
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
// P8d-A8 — IsPortListeningMac(): non-blocking loopback connect + select.
#include <sys/socket.h>
#include <sys/select.h>
#include <netinet/in.h>
#include <fcntl.h>
#include <errno.h>
#include <unistd.h>

extern char **environ;

// Spawn wallet/adblock processes without blocking the main thread.
// Blocking the main thread after CefInitialize prevents CEF's network
// service from completing setup, causing initial navigations to load
// empty pages.

static void SpawnWalletServer() {
    if (QuickHealthCheck()) {
        LOG_INFO("Wallet server already running (dev mode) - skipping launch");
        g_walletServerRunning = true;
        return;
    }

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

    std::string walletExe = exeDir + "/hodos-wallet";

    if (access(walletExe.c_str(), X_OK) != 0) {
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
}

// Phase 8d `P8d-A8` — the real supervisor (and `RequestWalletRestart`) live below
// SpawnAdblockServer, because they drive both spawners. The 3-line logging stub that
// used to sit here is gone: 📏 2026-09-19 it was measured to make the wallet panel's
// "Restart wallet service" button a **dead control** — the IPC arrived, this logged, and
// the user was told nothing.

static void SpawnAdblockServer() {
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

    std::string adblockExe = exeDir + "/hodos-adblock";
    if (access(adblockExe.c_str(), X_OK) != 0) {
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
}

// ============================================================================
// Phase 8d `P8d-A8` — macOS backend supervisor
// ============================================================================
// Mirrors cef_browser_shell.cpp :: BackendSupervisorLoop (Windows, 2026-09-14). Same
// period, same bound, same honest-flag shape, same HODOS_NO_SUPERVISE seam — the
// differences below are all forced by the platform, and each is commented where it bites.
//
//   - one detached std::thread, period 2 s. ⛔ NOT a CefPostDelayedTask(TID_FILE_*) loop:
//     all three file ids are ONE shared thread (8c O12), and a probe that blocks there
//     stalls balance / cookie / adblock tasks behind it. This thread touches no CEF.
//   - relaunch is BOUNDED: 3 attempts, 2 / 4 / 8 s backoff, then it stays down with the
//     manual Restart still live. ⛔ Never a hot loop — the exe may be quarantined or the
//     port taken, and a respawn loop there is worse than the outage.
//   - on death the interceptor's WalletStatusCache is invalidated, or a cached "exists"
//     outlives the wallet by 30 s and dApp calls fail as "HTTP 0" (contract D-8).
//   - HODOS_NO_SUPERVISE=1 (rig only, read once in the browser process) disables the
//     thread: the negative control for `P8d-A4`.
//
// 🍎 THREE macOS-specific hazards, all of which would produce a silently wrong supervisor:
//
//  1. ⛔ `kill(pid, 0)` IS NOT A LIVENESS TEST for our own child. An exited but unreaped
//     child is a ZOMBIE, and `kill(pid, 0)` succeeds on a zombie — so a supervisor built
//     on it never notices the wallet died. `waitpid(pid, &st, WNOHANG)` is the correct
//     instrument: it answers the question AND reaps, so three relaunch attempts cannot
//     leave three zombies behind.
//  2. ⛔ `waitpid` is ONE-SHOT. Once it reaps, every later call for that pid returns -1
//     (ECHILD). A naive `waitpid(...) != 0` therefore latches "dead" forever and would
//     re-relaunch on every tick. We clear the pid to -1 the moment we observe the exit.
//  3. ⛔ Windows' `IsPortListening` is winsock; macOS needs its own. Used only for the
//     "we did not launch it" case (a dev rig `cargo run` wallet) — there is no child to
//     waitpid for, so liveness has to come from the port.
// ============================================================================

static std::atomic<bool> g_supervisorStarted{false};
static std::atomic<bool> g_supervisorStop{false};
static std::atomic<bool> g_walletRestartRequested{false};
static const int kSupervisorPeriodMs   = 2000;
static const int kRelaunchMaxAttempts  = 3;
static const int kRelaunchBackoffMs[kRelaunchMaxAttempts] = {2000, 4000, 8000};

// macOS equivalent of Windows' IsPortListening: a non-blocking loopback connect with a
// short select() window. Loopback either answers immediately or refuses, so 150 ms is
// generous. ⛔ Deliberately NOT QuickHealthCheck() here — that is a 2000 ms libcurl GET,
// and a 2 s probe inside a 2 s loop would leave no gap between ticks.
static bool IsPortListeningMac(int port) {
    int sock = ::socket(AF_INET, SOCK_STREAM, 0);
    if (sock < 0) return false;
    int flags = ::fcntl(sock, F_GETFL, 0);
    ::fcntl(sock, F_SETFL, flags | O_NONBLOCK);

    struct sockaddr_in addr = {};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(static_cast<uint16_t>(port));
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);

    bool listening = false;
    int rc = ::connect(sock, (struct sockaddr*)&addr, sizeof(addr));
    if (rc == 0) {
        listening = true;                       // connected immediately
    } else if (errno == EINPROGRESS) {
        fd_set wset;
        FD_ZERO(&wset);
        FD_SET(sock, &wset);
        struct timeval tv = {0, 150 * 1000};    // 150 ms
        if (::select(sock + 1, nullptr, &wset, nullptr, &tv) > 0) {
            int err = 0; socklen_t len = sizeof(err);
            if (::getsockopt(sock, SOL_SOCKET, SO_ERROR, &err, &len) == 0 && err == 0) {
                listening = true;
            }
        }
    }
    ::close(sock);
    return listening;
}

// Interruptible sleep so shutdown never waits out a backoff.
static void SupervisorSleep(int ms) {
    for (int t = 0; t < ms && !g_supervisorStop; t += 100) usleep(100 * 1000);
}

// Hazards 1 and 2 above, in one place. Returns true when the child is gone, and clears
// `pid` so the next tick does not re-observe the same death through an ECHILD.
static bool ChildExited(pid_t& pid) {
    if (pid <= 0) return true;                  // nothing of ours is running
    int status = 0;
    const pid_t r = ::waitpid(pid, &status, WNOHANG);
    if (r == 0) return false;                   // still running
    pid = -1;                                   // r > 0 reaped now; r < 0 == ECHILD, already gone
    return true;
}

// One relaunch attempt. True when /health answers (or someone else already holds the port).
static bool RelaunchWalletProcess() {
    g_wallet_server_pid = -1;                   // forget the dead child before respawning
    SpawnWalletServer();
    if (g_walletServerRunning) return true;     // the port was already listening (dev rig)
    if (g_wallet_server_pid <= 0) return false; // exe missing or posix_spawn failed
    for (int i = 0; i < 12 && !g_supervisorStop; i++) {
        usleep(500 * 1000);
        if (QuickHealthCheck()) { g_walletServerRunning = true; return true; }
    }
    return false;
}

static bool RelaunchAdblockProcess() {
    g_adblock_server_pid = -1;
    SpawnAdblockServer();
    if (g_adblockServerRunning) return true;
    if (g_adblock_server_pid <= 0) return false;
    for (int i = 0; i < 12 && !g_supervisorStop; i++) {
        usleep(500 * 1000);
        if (QuickAdblockHealthCheck()) { g_adblockServerRunning = true; return true; }
    }
    return false;
}

static void BackendSupervisorLoop() {
    LOG_INFO("Backend supervisor started (period " + std::to_string(kSupervisorPeriodMs) +
             " ms; relaunch bounded to " + std::to_string(kRelaunchMaxAttempts) +
             " attempts, 2/4/8 s backoff)");
    // "Owned" = we launched it at least once, so a relaunch is ours to attempt. A dev-rig
    // wallet (`cargo run`) is not owned: report only — until the user presses Restart,
    // which launches our own child exactly as startup would with the port free.
    bool walletOwned  = (g_wallet_server_pid  > 0);
    bool adblockOwned = (g_adblock_server_pid > 0);
    int  walletAttempts = 0, adblockAttempts = 0;
    bool walletGaveUp = false, adblockGaveUp = false;

    while (!g_supervisorStop) {
        SupervisorSleep(kSupervisorPeriodMs);
        if (g_supervisorStop) break;

        // ---------------- wallet ----------------
        const bool manual = g_walletRestartRequested.exchange(false);
        if (manual) { walletOwned = true; walletAttempts = 0; walletGaveUp = false; }

        // ⛔ Order matters: ChildExited() REAPS, so it must be called on every tick where
        // we have a child, not short-circuited behind `manual`.
        const bool childGone = (g_wallet_server_pid > 0)
            ? ChildExited(g_wallet_server_pid)
            : !IsPortListeningMac(hodos::WalletPort());

        // 🍎 Divergence from Windows, deliberate: a MANUAL restart with the child still
        // alive must kill it first. SpawnWalletServer() early-returns "already running"
        // when /health answers, so without this the Restart button would be a no-op
        // against a wedged-but-listening wallet — the same dead-control shape this row
        // exists to remove. Windows' LaunchWalletProcess has the same early return; worth
        // mirroring there.
        if (manual && g_wallet_server_pid > 0 && !childGone) {
            LOG_INFO("Manual restart: terminating the running wallet child (pid " +
                     std::to_string(g_wallet_server_pid) + ") before relaunching");
            ::kill(g_wallet_server_pid, SIGTERM);
            for (int i = 0; i < 40 && !ChildExited(g_wallet_server_pid); i++) usleep(50 * 1000);
            if (g_wallet_server_pid > 0) {      // still not gone after ~2 s
                ::kill(g_wallet_server_pid, SIGKILL);
                ChildExited(g_wallet_server_pid);
            }
        }

        const bool walletDead = childGone || manual;

        if (walletDead) {
            if (g_walletServerRunning) {
                LOG_WARNING(std::string("Wallet server is DOWN (") +
                            (walletOwned ? "child exited" : "port not listening") + ")");
            }
            g_walletServerRunning = false;
            invalidateWalletStatusCache();

            if (walletOwned && !walletGaveUp) {
                if (walletAttempts < kRelaunchMaxAttempts) {
                    const int delay = manual ? 0 : kRelaunchBackoffMs[walletAttempts];
                    walletAttempts++;
                    LOG_INFO("Relaunching wallet server, attempt " + std::to_string(walletAttempts) +
                             "/" + std::to_string(kRelaunchMaxAttempts) + " after " +
                             std::to_string(delay) + " ms");
                    SupervisorSleep(delay);
                    if (g_supervisorStop) break;
                    if (RelaunchWalletProcess()) {
                        LOG_INFO("Wallet server is back (pid " +
                                 std::to_string(g_wallet_server_pid) + ")");
                        walletAttempts = 0;
                        invalidateWalletStatusCache();
                    } else {
                        LOG_WARNING("Wallet server relaunch attempt " +
                                    std::to_string(walletAttempts) + " failed");
                    }
                } else {
                    walletGaveUp = true;
                    LOG_ERROR("Wallet server relaunch gave up after " +
                              std::to_string(kRelaunchMaxAttempts) +
                              " attempts — staying down until the user restarts it");
                }
            }
        } else if (!g_walletServerRunning) {
            // Alive (our child, or the port came back on its own — a dev-rig restart, or a
            // slow startup the health wait gave up on). Flip the honest flag.
            if (IsPortListeningMac(hodos::WalletPort())) {
                g_walletServerRunning = true;
                walletAttempts = 0; walletGaveUp = false;
                invalidateWalletStatusCache();
                LOG_INFO("Wallet server is reachable again");
            }
        }

        // ---------------- adblock (restart-only, no UI) ----------------
        const bool adblockDead = (g_adblock_server_pid > 0)
            ? ChildExited(g_adblock_server_pid)
            : !IsPortListeningMac(hodos::AdblockPort());
        if (adblockDead) {
            if (g_adblockServerRunning) {
                LOG_WARNING(std::string("Adblock engine is DOWN (") +
                            (adblockOwned ? "child exited" : "port not listening") + ")");
            }
            g_adblockServerRunning = false;
            if (adblockOwned && !adblockGaveUp) {
                if (adblockAttempts < kRelaunchMaxAttempts) {
                    const int delay = kRelaunchBackoffMs[adblockAttempts];
                    adblockAttempts++;
                    LOG_INFO("Relaunching adblock engine, attempt " +
                             std::to_string(adblockAttempts) + "/" +
                             std::to_string(kRelaunchMaxAttempts) + " after " +
                             std::to_string(delay) + " ms");
                    SupervisorSleep(delay);
                    if (g_supervisorStop) break;
                    if (RelaunchAdblockProcess()) {
                        LOG_INFO("Adblock engine is back (pid " +
                                 std::to_string(g_adblock_server_pid) + ")");
                        adblockAttempts = 0;
                    } else {
                        LOG_WARNING("Adblock engine relaunch attempt " +
                                    std::to_string(adblockAttempts) + " failed");
                    }
                } else {
                    adblockGaveUp = true;
                    LOG_ERROR("Adblock engine relaunch gave up after " +
                              std::to_string(kRelaunchMaxAttempts) + " attempts");
                }
            }
        } else if (!g_adblockServerRunning && IsPortListeningMac(hodos::AdblockPort())) {
            g_adblockServerRunning = true;
            adblockAttempts = 0; adblockGaveUp = false;
            LOG_INFO("Adblock engine is reachable again");
        }
    }
    LOG_INFO("Backend supervisor stopped");
}

static void StartBackendSupervisor() {
    // ⛔ Read ONCE, in the browser process (a sandboxed child does not inherit the environment).
    static const bool kNoSupervise = [] {
        const char* v = std::getenv("HODOS_NO_SUPERVISE");
        return v && std::string(v) == "1";
    }();
    if (kNoSupervise) {
        LOG_WARNING("⚠️ HODOS_NO_SUPERVISE=1 — backend supervisor NOT started (P8d-A4 negative control)");
        return;
    }
    if (g_supervisorStarted.exchange(true)) return;
    std::thread(BackendSupervisorLoop).detach();
}

static void StopBackendSupervisor() {
    g_supervisorStop = true;
}

// The wallet panel's "Restart wallet service" button, via the shared `wallet_restart` IPC
// arm in simple_handler.cpp. 📏 Until 2026-09-19 this was a logging stub and the button was
// a measured dead control on macOS.
void RequestWalletRestart() {
    if (!g_supervisorStarted) {
        LOG_WARNING("wallet_restart requested but the supervisor is not running "
                    "(HODOS_NO_SUPERVISE, or before startup finished)");
        return;
    }
    LOG_INFO("wallet_restart requested — handing it to the supervisor");
    g_walletRestartRequested = true;
}

static void StartBackendServices() {
    SpawnWalletServer();
    SpawnAdblockServer();

    // Health-check the spawned servers on a background thread so the main
    // thread can pump the CEF message loop while they start up.
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        for (int i = 0; i < 20 && !g_walletServerRunning; i++) {
            usleep(500000);
            if (QuickHealthCheck()) {
                g_walletServerRunning = true;
                LOG_INFO("Wallet server is healthy");
                break;
            }
        }
        if (!g_walletServerRunning) {
            LOG_WARNING("Wallet server launched but health check timed out");
        }

        for (int i = 0; i < 20 && !g_adblockServerRunning; i++) {
            usleep(500000);
            if (QuickAdblockHealthCheck()) {
                g_adblockServerRunning = true;
                LOG_INFO("Adblock engine is healthy");
                break;
            }
        }
        if (!g_adblockServerRunning) {
            LOG_WARNING("Adblock engine launched but health check timed out");
        }

        // Phase 8d `P8d-A8`: supervision begins only AFTER the startup health wait, on this
        // already-dispatched block — nothing is added to the critical path (`P8d-A7`).
        // ⛔ Starting it earlier would have the supervisor racing startup: during the health
        // wait the wallet is legitimately not yet listening, and a supervisor running then
        // reads that as death and burns its three relaunch attempts before the first
        // wallet has finished booting. ⚠️ On this machine that window is not hypothetical —
        // the wallet Keychain dialog can hold the wallet at main.rs:568, alive but not
        // listening, for minutes. That case correctly relaunches NOTHING: the child is
        // alive, so waitpid says "running" and only the honest flag stays false.
        StartBackendSupervisor();
    });
}

static void StopServers() {
    // ⛔ Phase 8d: tell the supervisor FIRST, or it relaunches exactly what we are about to
    // stop — and on the way out, where the relaunched wallet would outlive the browser.
    StopBackendSupervisor();

    // Graceful shutdown via HTTP
    if (g_walletServerRunning) {
        SendShutdownRequest(hodos::WalletPort());
    }
    if (g_adblockServerRunning) {
        SendShutdownRequest(hodos::AdblockPort());
    }

    // R2/R3: adaptive wait for graceful exit instead of a blind 1s sleep. Poll for
    // the process to exit, returning the instant it's reaped, up to a bounded cap;
    // only force-kill (SIGTERM) if it overruns. Mirrors Windows StopWalletServer's
    // WaitForSingleObject(5000) early-exit semantics. The wallet gets the full
    // window (it owns the money DB + may be mid-broadcast); adblock a shorter one.
    // (waitpid: >0 == reaped, <0 == already gone/ECHILD, 0 == still running.)
    auto stopPid = [](pid_t& pid, int max_wait_ms, const char* name) {
        if (pid <= 0) return;
        int status;
        const int step_ms = 50;
        for (int waited = 0; waited < max_wait_ms; waited += step_ms) {
            if (waitpid(pid, &status, WNOHANG) != 0) {  // reaped or already gone
                pid = -1;
                return;
            }
            usleep(step_ms * 1000);
        }
        LOG_WARNING(std::string(name) + " did not exit gracefully — sending SIGTERM");
        kill(pid, SIGTERM);
        for (int i = 0; i < 10 && waitpid(pid, &status, WNOHANG) == 0; i++) {
            usleep(50000);
        }
        pid = -1;
    };

    stopPid(g_wallet_server_pid, 5000, "Wallet server");   // money process — full window
    stopPid(g_adblock_server_pid, 1500, "Adblock engine"); // shorter window
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

        // persist_session_cookies MUST stay disabled — causes extra windows on startup.
        // See cef_browser_shell.cpp for full explanation.
        // settings.persist_session_cookies = true;

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
        std::string argProfile = "";
        NSArray* arguments = [[NSProcessInfo processInfo] arguments];
        for (NSString* arg in arguments) {
            std::string argStr = [arg UTF8String];
            if (argStr.find("--profile=") == 0) {
                argProfile = argStr.substr(10);
                if (!argProfile.empty() && argProfile.front() == '"') argProfile = argProfile.substr(1);
                if (!argProfile.empty() && argProfile.back() == '"') argProfile.pop_back();
                break;
            }
        }
        auto& pm = ProfileManager::GetInstance();
        std::vector<std::string> existingIds;
        for (const auto& p : pm.GetAllProfiles()) existingIds.push_back(p.id);
        ProfileManager::StartupResolution res = ProfileManager::ResolveStartup(
            argProfile, existingIds, pm.GetDefaultProfileId(),
            pm.ShouldShowPickerOnStartup());
        std::string profileId = res.profileId;
        g_picker_mode = res.showPicker;
        pm.SetCurrentProfileId(profileId, /*persist=*/false);
        LOG_INFO("Using profile: " + profileId +
                 (g_picker_mode ? " [picker mode]" : ""));

        // Data directory: picker uses a neutral cache that touches no real profile.
        std::string profile_cache = g_picker_mode
            ? (user_data_path + "/.picker-cache")
            : ProfileManager::GetInstance().GetCurrentProfileDataPath();
        LOG_INFO(std::string(g_picker_mode ? "Picker cache path: " : "Profile data path: ") + profile_cache);

        // Picker mode: no profile lock, no DBs, no backends — just the chooser UI.
        if (!g_picker_mode) {
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
        }

        if (!g_picker_mode) {
            // Initialize SettingsManager with profile-specific path
            SettingsManager::GetInstance().Initialize(profile_cache);
            LOG_INFO("Settings loaded for profile: " + profileId);

            ClearPersistedInternalFrontendZoom(profile_cache);

            AdblockCache::GetInstance().Initialize(profile_cache);
            AdblockCache::GetInstance().SetGlobalEnabled(
                SettingsManager::GetInstance().GetPrivacySettings().adBlockEnabled);
            LOG_INFO("AdblockCache initialized");

            FingerprintProtection::GetInstance().Initialize();
            FingerprintProtection::GetInstance().LoadSiteSettings(profile_cache);
            // C2: load/generate the persistent per-profile farbling seed from the same
            // file, so the per-navigation path never touches disk.
            FarblingPolicy::InitializeForProfile(profile_cache);
            FingerprintProtection::GetInstance().SetEnabled(
                SettingsManager::GetInstance().GetPrivacySettings().fingerprintProtection);
            LOG_INFO("Fingerprint protection initialized");

            if (CookieBlockManager::GetInstance().Initialize(profile_cache)) {
                LOG_INFO("CookieBlockManager initialized");
            } else {
                LOG_WARNING("CookieBlockManager initialization failed");
            }

            BookmarkManager::GetInstance().Initialize(profile_cache);
            LOG_INFO("BookmarkManager initialized");

            if (SitePermissionStore::GetInstance().Initialize(profile_cache)) {
                LOG_INFO("SitePermissionStore initialized successfully");
            } else {
                LOG_ERROR("Failed to initialize SitePermissionStore");
            }

            // beta.3 Phase 7b — the local favicon store. Mirrors cef_browser_shell.cpp.
            //
            // ⛔ MEASURED-BY-READING 2026-09-08 (Mac): this call was MISSING on macOS while
            // the Windows entry point had it, and the failure is entirely SILENT. Both
            // consumers are guarded rather than fallible:
            //   • simple_handler.cpp :: OnFaviconURLChange gates the DownloadImage on
            //     `store.IsInitialized()`, so nothing is ever fetched or stored — no error;
            //   • the `favicon_get` IPC returns GetDataUri()=="" for every host, so hosts are
            //     simply omitted from the reply and React draws its initial-letter tile.
            // ⇒ macOS showed letter tiles on the omnibox, new tab and bookmarks forever and
            // never created favicons.db, which reads as a rendering bug rather than an
            // uninitialised singleton. Predicted in MAC_RELAY_P7_ROUND.md M2, confirmed here.
            //
            // ⚠️ Note what was NOT broken: the phase's PRIVACY subject held on macOS anyway,
            // because the React surfaces stopped emitting google.com/s2/favicons regardless of
            // store state. The de-Googling was intact; only the replacement was dead.
            if (hodos::FaviconStore::GetInstance().Initialize(profile_cache)) {
                LOG_INFO("FaviconStore initialized successfully");
            } else {
                LOG_ERROR("Failed to initialize FaviconStore");
            }

            if (PaidContentCache::GetInstance().Initialize(profile_cache)) {
                PaidContentCache::GetInstance().SetEnabled(
                    SettingsManager::GetInstance().GetPrivacySettings().paidContentCacheEnabled);
                LOG_INFO("PaidContentCache initialized");
            } else {
                LOG_WARNING("PaidContentCache initialization failed");
            }
        }

        // Set root_cache_path AND cache_path to profile-specific directory.
        // CRITICAL: root_cache_path must be unique per CEF instance — two instances
        // sharing the same root_cache_path will cause CefInitialize to fail.
        std::string cache_path = profile_cache;
        CefString(&settings.root_cache_path).FromString(cache_path);
        CefString(&settings.cache_path).FromString(cache_path);

        // Remote debugging port: 9222 for Default profile, disabled for others
        // (avoids port conflict when multiple instances run simultaneously)
        //
        // ⛔ PICKER MODE GETS NO PORT. `ResolveStartup` returns `coherentDefault()` — i.e. the
        // literal string "Default" — for the picker as well as for a real Default launch
        // (`ProfileManager.h`, the `existingIds.size() > 1 && pickerEnabled` arm), so a bare
        // `profileId == "Default"` test is TRUE in picker mode and bound CDP there. Measured
        // 2026-08-11 before this guard: a no-`--profile` launch logged
        // `Using profile: Default [picker mode]` followed by `Remote debugging port: 9322`,
        // and `http://127.0.0.1:5137/profile-picker?mode=window` was live on CDP.
        // Windows has guarded this since its own port block was written
        // (`cef_browser_shell.cpp`, `if (g_picker_mode) ... = 0;`); macOS had not.
        //
        // Why it is worth a guard, stated from what was actually measured here:
        //   * the picker is a chooser UI the user never asked to be debuggable, and an open CDP
        //     port is a full-control surface to any local process for as long as it is open;
        //   * the picker HOLDS 9222/9322 for its whole lifetime, so a launch racing it reads as
        //     "the browser failed to start" — a symptom both platforms have chased once.
        // ⚠️ Deliberately NOT claimed: the Windows comment's `navigator.webdriver` rationale.
        // Measured on the picker page here with the port bound, `navigator.webdriver` is
        // **false** — CefSettings.remote_debugging_port does not trip the wire that forwarding
        // `--remote-debugging-port` on the command line does. The guard is still right; that
        // particular justification is Windows-specific and unverified on macOS.
        if (g_picker_mode) {
            settings.remote_debugging_port = 0;
        } else {
            settings.remote_debugging_port = (profileId == "Default") ? 9222 : 0;
        }
        // D2 (Phase 9, `67a9ab6` on Windows) — mirrored here 2026-09-19. A RELEASE build binds
        // NO debug port at all; only a dev build gets one, offset +100 so it can never collide
        // with the installed build's 9222.
        //
        // ⛔ This was measured missing on macOS: the INSTALLED build (pid 56785, argv
        // `/Applications/HodosBrowser.app/Contents/MacOS/HodosBrowser --profile=Default`, with
        // NO `--remote-debugging-port` switch) was holding `127.0.0.1:9222` LISTEN. So until
        // this line, every shipped macOS build exposed a full-control CDP surface to any local
        // process. Windows has been gated since `67a9ab6`; the macOS mirror was item 4 of the
        // Mac queue and is this block.
        //
        // ⚠️ SUBJECT trap, cost Windows a run: passing `--remote-debugging-port` on the command
        // line binds CDP **regardless** of this setting — CEF forwards the switch to Chromium
        // ahead of CefSettings. Verify the gate by launching WITHOUT that switch and then
        // `lsof -nP -iTCP:9222 -sTCP:LISTEN`.
        if (!hodos::IsDevEnv()) {
            settings.remote_debugging_port = 0;   // D2: release binds no debug port at all
        } else if (settings.remote_debugging_port != 0) {
            settings.remote_debugging_port += 100;
        }

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

        if (!g_picker_mode) {
            NSLog(@"🔄 Starting backend services...");
            StartBackendServices();
            NSLog(@"✅ Backend services spawned (health checks async)");
        }

        // Create the primary BrowserWindow record (window 0) in WindowManager.
        // This MUST happen before any browser creation so SetBrowserForRole works.
        WindowManager::GetInstance().CreateWindowRecord();
        LOG_INFO("✅ WindowManager window 0 created");

        // Create windows after CEF is initialized
        // (Activation policy already set above for main process only)
        CreateMainWindow();

        // Install shared overlay focus-loss handler (closes dropdown overlays on Cmd+Tab)
        InstallAppFocusLossHandler();

        if (!g_main_window || !g_header_view || (!g_picker_mode && !g_webview_view)) {
            LOG_ERROR("❌ Window creation failed - exiting");
            CefShutdown();
            return 1;
        }

        // Store window references for later use
        app->SetMacOSWindow((__bridge void*)g_main_window,
                           (__bridge void*)g_header_view,
                           g_webview_view ? (__bridge void*)g_webview_view : nullptr);

        // Populate window 0's BrowserWindow struct for multi-window support
        BrowserWindow* bw0 = WindowManager::GetInstance().GetWindow(0);
        if (bw0) {
            bw0->ns_window = (__bridge void*)g_main_window;
            bw0->header_view = (__bridge void*)g_header_view;
            bw0->webview_view = g_webview_view ? (__bridge void*)g_webview_view : nullptr;
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

        std::string header_url = g_picker_mode
            ? "http://127.0.0.1:5137/profile-picker?mode=window"
            : "http://127.0.0.1:5137";
        bool browser_created = CefBrowserHost::CreateBrowser(
            header_window_info,
            header_handler,
            header_url,
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

        if (!g_picker_mode) {
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

            LOG_INFO("🔄 Initializing HistoryManager...");
            if (HistoryManager::GetInstance().Initialize(cache_path)) {
                LOG_INFO("✅ HistoryManager initialized successfully");
            } else {
                LOG_ERROR("❌ Failed to initialize HistoryManager");
            }
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

        if (!g_picker_mode && !hodos::IsDevEnv()) {
            auto& settings = SettingsManager::GetInstance();
            auto browserSettings = settings.GetBrowserSettings();
            std::string appVersion = APP_VERSION;
            std::string appcastUrl = "https://hodosbrowser.com/appcast.xml";

            // #1 (macOS): autoUpdateMode is machine/user-GLOBAL. On the FIRST run under the
            // global scheme, collapse to the MOST CONSERVATIVE mode across all profiles so an
            // explicit notify/off in ANY profile is never promoted to silent (mirrors the
            // Windows collapse in cef_browser_shell.cpp). mac needs no update-state mirror —
            // Sparkle reads the (now global) autoUpdateMode directly below.
            if (settings.GlobalUpdateModeWasAbsentAtLoad()) {
                std::string updMode = browserSettings.autoUpdateMode;
                for (const auto& p : ProfileManager::GetInstance().GetAllProfiles()) {
                    std::string pm = SettingsManager::ReadModeFromProfileSettings(p.path);
                    if (!pm.empty()) updMode = hodos::MoreConservativeMode(updMode, pm);
                }
                settings.SetGlobalUpdateModeAuthoritative(updMode);
                browserSettings.autoUpdateMode = updMode;  // reflect for SetUpdateMode + log below
                LOG_INFO("Update mode: one-time global collapse -> " + updMode);
            }

            auto& updater = AutoUpdater::GetInstance();
            updater.Initialize(appVersion, appcastUrl, true);

            UpdateMode mode = UpdateMode::Silent;
            if (browserSettings.autoUpdateMode == "off") mode = UpdateMode::Off;
            else if (browserSettings.autoUpdateMode == "notify") mode = UpdateMode::Notify;
            updater.SetUpdateMode(mode);

            LOG_INFO("Auto-updater initialized (version=" + appVersion +
                     ", mode=" + browserSettings.autoUpdateMode + ")");
        } else if (hodos::IsDevEnv()) {
            LOG_INFO("Auto-updater skipped (dev build)");
        }

        LOG_INFO("🚀 Entering CEF message loop...");

        // Run CEF message loop (blocks until quit)
        CefRunMessageLoop();

        LOG_INFO("CEF message loop exited — running post-loop cleanup...");
        if (!g_picker_mode) {
            StopServers();

            // R2/R3: deterministically checkpoint + close the SQLite browser DBs
            // while the profile lock is STILL held, THEN release the lock. All
            // browsers are closed (message loop exited above), so the DBs are
            // quiescent. This closes the quick-restart SQLITE_BUSY race where a
            // relaunch won the freed lock while the old process still held
            // live-WAL DB handles. Matches Windows post-loop cleanup ordering.
            LOG_INFO("Closing browser databases (checkpoint + close)...");
            HistoryManager::GetInstance().Shutdown();
            BookmarkManager::GetInstance().Shutdown();
            SitePermissionStore::GetInstance().Shutdown();
            hodos::FaviconStore::GetInstance().Shutdown();
            CookieBlockManager::GetInstance().Shutdown();
            PaidContentCache::GetInstance().Shutdown();

            LOG_INFO("Releasing profile lock...");
            ReleaseProfileLock();
        }

        LOG_INFO("✅ Application exited cleanly");
        Logger::Shutdown();
        CefShutdown();

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

static void RemoveMenuClickOutsideMonitor() {
    if (g_menu_click_monitor) {
        [NSEvent removeMonitor:g_menu_click_monitor];
        g_menu_click_monitor = nil;
    }
}

static void InstallMenuClickOutsideMonitor() {
    if (g_menu_click_monitor) return;

    g_menu_click_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown
        handler:^NSEvent*(NSEvent* event) {
            if (!g_menu_overlay_window || ![g_menu_overlay_window isVisible]) {
                return event;
            }
            NSPoint screenLocation = [NSEvent mouseLocation];
            if (!OverlayHitsContent(g_menu_overlay_window, screenLocation)) {  // transparent pixel = outside
                [g_menu_overlay_window orderOut:nil];
                RemoveMenuClickOutsideMonitor();
                g_menu_overlay_last_hide_time = CFAbsoluteTimeGetCurrent();
            }
            return event;
        }];
}

void HideMenuOverlayMacOS() {
    if (g_menu_overlay_window) {
        [g_menu_overlay_window orderOut:nil];
        DetachOverlayFromParentMac(g_menu_overlay_window);  // D-h2
        RemoveMenuClickOutsideMonitor();
        g_menu_overlay_last_hide_time = CFAbsoluteTimeGetCurrent();
        LOG_INFO("Menu overlay hidden (macOS)");
    }
}

bool IsMenuOverlayVisible() {
    return g_menu_overlay_window && [g_menu_overlay_window isVisible];
}

bool WasMenuOverlayJustHidden() {
    CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
    return (now - g_menu_overlay_last_hide_time) < 0.3;
}

void ShowMenuOverlayMacOS(int iconRightOffset, BrowserWindow* targetWin = nullptr) {
    if (g_menu_overlay_window) {
        // D-h2: anchor and re-parent to the requesting window before showing.
        NSWindow* host = OverlayHostWindow(targetWin);
        OwnOverlayToRequestingWindowMac(g_menu_overlay_window, targetWin);

        NSRect menuFrame = CalculateToolbarOverlayFrame(host, 280, 450, kMacHeaderHeightPt);
        [g_menu_overlay_window setFrame:menuFrame display:YES];

        [g_menu_overlay_window makeKeyAndOrderFront:nil];
        NSView* cv = [g_menu_overlay_window contentView];
        [g_menu_overlay_window makeFirstResponder:cv];
        InstallMenuClickOutsideMonitor();
        LOG_INFO("Menu overlay shown (macOS)");
    }
}

void CreateMenuOverlayMac(int iconRightOffset, BrowserWindow* targetWin) {
    LOG_INFO("Creating menu overlay (macOS) iconRightOffset=" + std::to_string(iconRightOffset));

    if (!g_main_window) {
        LOG_ERROR("Cannot create menu overlay: main window is null");
        return;
    }

    NSWindow* host = OverlayHostWindow(targetWin);  // D-h2

    // Destroy existing menu overlay
    if (g_menu_overlay_window) {
        LOG_INFO("Destroying existing menu overlay");
        DestroyMenuOverlayWindow(true);
    }

    CGFloat menuWidth = 280;
    CGFloat menuHeight = 450;

    // Position: flush right, flush below header
    NSRect menuFrame = CalculateToolbarOverlayFrame(host, menuWidth, menuHeight, kMacHeaderHeightPt);

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

    // Child window of the REQUESTING window (moves/minimizes together) — D-h2
    OwnOverlayToRequestingWindowMac(g_menu_overlay_window, targetWin);

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

    [g_menu_overlay_window makeKeyAndOrderFront:nil];
    [g_menu_overlay_window makeFirstResponder:contentView];
    InstallMenuClickOutsideMonitor();

    LOG_INFO("Menu overlay created successfully (using GenericOverlayView)");
}

// C-linkage stubs called from simple_handler.cpp via extern
void CreateMenuOverlay(void* hInstance, bool showImmediately, int iconRightOffset) {
    // hInstance is Windows-only, ignored on macOS
    CreateMenuOverlayMac(iconRightOffset);
}

void ShowMenuOverlay(int iconRightOffset) {
    ShowMenuOverlayMacOS(iconRightOffset);
}

void HideMenuOverlay() {
    HideMenuOverlayMacOS();
}
