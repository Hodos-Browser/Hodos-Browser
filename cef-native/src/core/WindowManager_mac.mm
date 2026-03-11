// macOS implementation of WindowManager::CreateFullWindow
// Creates a new top-level browser window with header, webview, and optional initial tab.

#import <Cocoa/Cocoa.h>
#import <objc/runtime.h>

#include "../../include/core/WindowManager.h"
#include "../../include/core/TabManager.h"
#include "../../include/handlers/simple_handler.h"
#include "../../include/core/Logger.h"
#include "include/cef_browser.h"
#include "include/cef_request_context.h"
#include <algorithm>

#define LOG_INFO_WM(msg) Logger::Log(msg, 1, 2)
#define LOG_ERROR_WM(msg) Logger::Log(msg, 3, 2)

// Forward declaration
extern void ShutdownApplication();

// ============================================================================
// Per-Window Delegate (multi-window aware)
// ============================================================================

@interface BrowserWindowDelegate : NSObject <NSWindowDelegate>
@property (nonatomic) int window_id;
@end

@implementation BrowserWindowDelegate

- (void)windowDidResize:(NSNotification *)notification {
    BrowserWindow* bw = WindowManager::GetInstance().GetWindow(self.window_id);
    if (!bw) return;

    NSWindow* nsWindow = (__bridge NSWindow*)bw->ns_window;
    NSRect contentRect = [[nsWindow contentView] bounds];
    int headerHeight = 99;

    // Resize header view (fixed 99px at top)
    NSView* headerView = (__bridge NSView*)bw->header_view;
    if (headerView) {
        [headerView setFrame:NSMakeRect(0, contentRect.size.height - headerHeight,
                                        contentRect.size.width, headerHeight)];
    }

    // Resize webview view (fills below header)
    NSView* webviewView = (__bridge NSView*)bw->webview_view;
    if (webviewView) {
        [webviewView setFrame:NSMakeRect(0, 0, contentRect.size.width,
                                         contentRect.size.height - headerHeight)];
    }

    // Notify CEF browsers of resize
    if (bw->header_browser) {
        bw->header_browser->GetHost()->WasResized();
    }

    // Notify active tab in this window
    Tab* activeTab = TabManager::GetInstance().GetActiveTabForWindow(self.window_id);
    if (activeTab && activeTab->browser) {
        activeTab->browser->GetHost()->WasResized();
    }
}

- (void)windowDidMove:(NSNotification *)notification {
    // Overlay repositioning for this window's overlays would go here
    // Currently overlays are only on window 0 (handled by MainWindowDelegate)
}

- (BOOL)windowShouldClose:(NSWindow *)sender {
    int windowCount = WindowManager::GetInstance().GetWindowCount();

    if (windowCount <= 1) {
        // Last window — quit the application
        ShutdownApplication();
        return YES;
    }

    // Not the last window — close tabs and clean up
    auto allTabs = TabManager::GetInstance().GetAllTabs();
    std::vector<int> tabsToClose;
    for (auto* tab : allTabs) {
        if (tab->window_id == self.window_id) {
            tabsToClose.push_back(tab->id);
        }
    }
    for (int tabId : tabsToClose) {
        TabManager::GetInstance().CloseTab(tabId);
    }

    WindowManager::GetInstance().RemoveWindow(self.window_id);
    LOG_INFO_WM("Window " + std::to_string(self.window_id) + " closed and removed [macOS]");
    return YES;
}

@end

// ============================================================================
// CreateFullWindow — macOS implementation
// ============================================================================

BrowserWindow* WindowManager::CreateFullWindow(bool createInitialTab) {
    int wid = CreateWindowRecord();
    BrowserWindow* bw = GetWindow(wid);
    if (!bw) return nullptr;

    LOG_INFO_WM("Creating new browser window (id=" + std::to_string(wid) + ") [macOS]");

    // Screen dimensions with offset stacking for each new window
    NSRect screenRect = [[NSScreen mainScreen] visibleFrame];
    int offset = wid * 30;
    CGFloat winW = std::max(800.0, screenRect.size.width - 100);
    CGFloat winH = std::max(600.0, screenRect.size.height - 100);
    CGFloat winX = screenRect.origin.x + 50 + offset;
    // macOS origin is bottom-left, so subtract from top
    CGFloat winY = screenRect.origin.y + screenRect.size.height - winH - 50 - offset;

    int headerHeight = 99;

    // Create NSWindow
    NSRect windowFrame = NSMakeRect(winX, winY, winW, winH);
    NSWindow* nsWindow = [[NSWindow alloc]
        initWithContentRect:windowFrame
        styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                  NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable
        backing:NSBackingStoreBuffered
        defer:NO];

    if (!nsWindow) {
        LOG_ERROR_WM("Failed to create NSWindow for window " + std::to_string(wid));
        RemoveWindow(wid);
        return nullptr;
    }

    [nsWindow setTitle:@"Hodos Browser"];
    [nsWindow setReleasedWhenClosed:NO];

    // Set per-window delegate (retain via associated object to prevent dealloc)
    BrowserWindowDelegate* delegate = [[BrowserWindowDelegate alloc] init];
    delegate.window_id = wid;
    [nsWindow setDelegate:delegate];
    objc_setAssociatedObject(nsWindow, "delegate", delegate, OBJC_ASSOCIATION_RETAIN_NONATOMIC);

    // Create header view (99px at top of content area)
    NSRect contentBounds = [[nsWindow contentView] bounds];
    NSRect headerRect = NSMakeRect(0, contentBounds.size.height - headerHeight,
                                   contentBounds.size.width, headerHeight);
    NSView* headerView = [[NSView alloc] initWithFrame:headerRect];
    [headerView setAutoresizingMask:NSViewWidthSizable | NSViewMinYMargin];
    [[nsWindow contentView] addSubview:headerView];

    // Create webview view (fills below header)
    NSRect webviewRect = NSMakeRect(0, 0, contentBounds.size.width,
                                    contentBounds.size.height - headerHeight);
    NSView* webviewView = [[NSView alloc] initWithFrame:webviewRect];
    [webviewView setAutoresizingMask:NSViewWidthSizable | NSViewHeightSizable];
    [[nsWindow contentView] addSubview:webviewView];

    // Store in BrowserWindow struct
    bw->ns_window = (__bridge void*)nsWindow;
    bw->header_view = (__bridge void*)headerView;
    bw->webview_view = (__bridge void*)webviewView;

    // Show window
    [nsWindow makeKeyAndOrderFront:nil];

    // Create header CEF browser (windowed rendering via SetAsChild)
    NSRect headerBounds = [headerView bounds];
    CefWindowInfo headerInfo;
    CefRect cefHeaderRect(0, 0, (int)headerBounds.size.width, (int)headerBounds.size.height);
    headerInfo.SetAsChild((__bridge void*)headerView, cefHeaderRect);

    CefBrowserSettings browserSettings;
    browserSettings.background_color = CefColorSetARGB(255, 255, 255, 255);
    CefRefPtr<SimpleHandler> headerHandler = new SimpleHandler("header", wid);
    CefBrowserHost::CreateBrowser(headerInfo, headerHandler,
        "http://127.0.0.1:5137", browserSettings,
        nullptr, CefRequestContext::GetGlobalContext());

    // Create initial NTP tab (unless restoring session)
    if (createInitialTab) {
        int tabId = TabManager::GetInstance().CreateTab(
            "http://127.0.0.1:5137/newtab",
            (__bridge void*)webviewView,
            0, 0,
            (int)webviewRect.size.width,
            (int)webviewRect.size.height,
            wid);
        SimpleHandler::NotifyWindowTabListChanged(wid);
    }

    SetActiveWindowId(wid);

    // Force WasResized on all OTHER windows to prevent stale render artifacts.
    // Collect refs under lock, then call WasResized outside lock to avoid deadlock.
    {
        std::vector<int> otherIds;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            for (auto& [id, win] : windows_) {
                if (id != wid) otherIds.push_back(id);
            }
        }
        for (int id : otherIds) {
            BrowserWindow* other = GetWindow(id);
            if (other && other->header_browser) {
                other->header_browser->GetHost()->WasResized();
            }
            Tab* otherTab = TabManager::GetInstance().GetActiveTabForWindow(id);
            if (otherTab && otherTab->browser) {
                otherTab->browser->GetHost()->WasResized();
            }
        }
    }

    LOG_INFO_WM("New browser window created: id=" + std::to_string(wid) + " [macOS]");
    return bw;
}

// ============================================================================
// Helper: Get BrowserWindow at a screen coordinate (for tab merge detection)
// Uses macOS coordinate system (Y-flip handled by caller or internally)
// ============================================================================

extern "C" void* GetWindowAtScreenPointMacOS(int screenX, int screenY) {
    // macOS screen coordinates: origin at bottom-left
    // The caller passes screen coords from CEF/React which are top-left origin
    // Flip Y to get macOS coordinates
    CGFloat screenHeight = [[NSScreen mainScreen] frame].size.height;
    NSPoint point = NSMakePoint((CGFloat)screenX, screenHeight - (CGFloat)screenY);

    auto allWindows = WindowManager::GetInstance().GetAllWindows();
    for (BrowserWindow* bw : allWindows) {
        if (!bw->ns_window) continue;
        NSWindow* nsWindow = (__bridge NSWindow*)bw->ns_window;
        if (NSPointInRect(point, [nsWindow frame])) {
            return bw;
        }
    }
    return nullptr;
}

// ============================================================================
// Helper: Position a window at a screen point (for tab tear-off)
// screenX/screenY are in top-left-origin coordinates (from CEF/React)
// ============================================================================

extern "C" void PositionWindowAtScreenPoint(void* ns_window_ptr, int screenX, int screenY) {
    if (!ns_window_ptr) return;
    NSWindow* nsWindow = (__bridge NSWindow*)ns_window_ptr;
    CGFloat screenHeight = [[NSScreen mainScreen] frame].size.height;
    // Offset so title bar is near cursor
    NSPoint origin = NSMakePoint((CGFloat)(screenX - 100),
                                 screenHeight - (CGFloat)screenY - [nsWindow frame].size.height + 50);
    [nsWindow setFrameOrigin:origin];
}

// ============================================================================
// Ghost Tab Window for macOS (tear-off preview)
// ============================================================================

static NSWindow* s_ghost_window = nil;
static NSTimer* s_ghost_timer = nil;
static NSTextField* s_ghost_label = nil;

extern "C" void ShowGhostTabMacOS(const char* title, int width, int height) {
    // Hide existing
    if (s_ghost_window) {
        [s_ghost_timer invalidate];
        s_ghost_timer = nil;
        [s_ghost_window close];
        s_ghost_window = nil;
    }

    int ghostWidth = (width > 60) ? width : 200;
    int ghostHeight = (height > 10) ? height : 36;

    NSPoint mouseLoc = [NSEvent mouseLocation];
    NSRect frame = NSMakeRect(mouseLoc.x - ghostWidth / 2, mouseLoc.y - 10,
                              ghostWidth, ghostHeight);

    s_ghost_window = [[NSWindow alloc]
        initWithContentRect:frame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    [s_ghost_window setLevel:NSFloatingWindowLevel];
    [s_ghost_window setOpaque:NO];
    [s_ghost_window setAlphaValue:0.85];
    [s_ghost_window setBackgroundColor:[NSColor whiteColor]];
    [s_ghost_window setHasShadow:YES];
    [s_ghost_window setIgnoresMouseEvents:YES];
    [s_ghost_window setReleasedWhenClosed:NO];

    // Rounded corners
    [[s_ghost_window contentView] setWantsLayer:YES];
    [s_ghost_window contentView].layer.cornerRadius = 7;
    [s_ghost_window contentView].layer.masksToBounds = YES;

    // Title label
    NSString* titleStr = [NSString stringWithUTF8String:title];
    s_ghost_label = [[NSTextField alloc] initWithFrame:NSMakeRect(12, 0, ghostWidth - 24, ghostHeight)];
    [s_ghost_label setStringValue:titleStr];
    [s_ghost_label setBezeled:NO];
    [s_ghost_label setDrawsBackground:NO];
    [s_ghost_label setEditable:NO];
    [s_ghost_label setSelectable:NO];
    [s_ghost_label setTextColor:[NSColor colorWithWhite:0.25 alpha:1.0]];
    [s_ghost_label setFont:[NSFont systemFontOfSize:12 weight:NSFontWeightMedium]];
    [s_ghost_label setLineBreakMode:NSLineBreakByTruncatingTail];
    [[s_ghost_window contentView] addSubview:s_ghost_label];

    [s_ghost_window orderFrontRegardless];

    // Timer to follow cursor (~60fps)
    s_ghost_timer = [NSTimer scheduledTimerWithTimeInterval:1.0/60.0 repeats:YES block:^(NSTimer* timer) {
        if (!s_ghost_window) {
            [timer invalidate];
            return;
        }
        NSPoint loc = [NSEvent mouseLocation];
        [s_ghost_window setFrameOrigin:NSMakePoint(loc.x - ghostWidth / 2, loc.y - 10)];
    }];
}

extern "C" void HideGhostTabMacOS() {
    if (s_ghost_timer) {
        [s_ghost_timer invalidate];
        s_ghost_timer = nil;
    }
    if (s_ghost_window) {
        [s_ghost_window close];
        s_ghost_window = nil;
    }
    s_ghost_label = nil;
}
