// Mac-specific helpers for SimpleHandler. Compiled as Objective-C++.
//
// RunContextMenu: CEF on macOS windowed rendering does not auto-present the
// CefMenuModel we build in OnBeforeContextMenu as a native NSMenu. We must
// convert CefMenuModel → NSMenu and popUp ourselves via AppKit. Without this,
// the right-click event still reaches CEF but Chromium's navigation layer
// treats it as a primary click and the link opens.

#import <Cocoa/Cocoa.h>

#include "include/cef_browser.h"
#include "include/cef_context_menu_handler.h"
#include "include/cef_menu_model.h"
#include "../../include/core/Logger.h"

#define LOG_INFO_CM(msg) Logger::Log(msg, 1, 0)
#define LOG_WARNING_CM(msg) Logger::Log(msg, 2, 0)

// Per-popup target: holds the callback until the user picks an item or the
// menu is dismissed. AppKit action selectors take an NSMenuItem* sender, so
// we stash the chosen command_id in the item's `tag` and the callback on a
// target object we retain via the menu.
@interface HodosContextMenuTarget : NSObject
@property(nonatomic) CefRefPtr<CefRunContextMenuCallback> callback;
@property(nonatomic) BOOL fired;
- (void)itemPicked:(NSMenuItem*)sender;
- (void)menuCancelled;
@end

@implementation HodosContextMenuTarget
- (instancetype)init {
    if ((self = [super init])) {
        _fired = NO;
    }
    return self;
}
- (void)itemPicked:(NSMenuItem*)sender {
    if (self.fired) return;
    self.fired = YES;
    int cmd = (int)[sender tag];
    LOG_INFO_CM("🖱️ Context menu item picked: command_id=" + std::to_string(cmd));
    if (self.callback) {
        self.callback->Continue(cmd, EVENTFLAG_NONE);
    }
}
- (void)menuCancelled {
    if (self.fired) return;
    self.fired = YES;
    LOG_INFO_CM("🖱️ Context menu dismissed without selection");
    if (self.callback) {
        self.callback->Cancel();
    }
}
@end

namespace {

// Convert a CefMenuModel into an NSMenu. Action target is `target`; each
// NSMenuItem's tag holds its CEF command_id so itemPicked: can forward it.
NSMenu* BuildNSMenuFromModel(CefRefPtr<CefMenuModel> model, HodosContextMenuTarget* target) {
    NSMenu* menu = [[NSMenu alloc] initWithTitle:@""];
    [menu setAutoenablesItems:NO];

    int count = static_cast<int>(model->GetCount());
    for (int i = 0; i < count; ++i) {
        cef_menu_item_type_t type = model->GetTypeAt(i);
        if (type == MENUITEMTYPE_SEPARATOR) {
            [menu addItem:[NSMenuItem separatorItem]];
            continue;
        }

        int cmd = model->GetCommandIdAt(i);
        std::string label = model->GetLabelAt(i).ToString();
        NSString* title = [NSString stringWithUTF8String:label.c_str()];
        if (!title) title = @"";

        NSMenuItem* item = [[NSMenuItem alloc] initWithTitle:title
                                                      action:@selector(itemPicked:)
                                               keyEquivalent:@""];
        [item setTarget:target];
        [item setTag:cmd];
        [item setEnabled:model->IsEnabledAt(i) ? YES : NO];

        if (type == MENUITEMTYPE_CHECK || type == MENUITEMTYPE_RADIO) {
            [item setState:model->IsCheckedAt(i) ? NSControlStateValueOn
                                                 : NSControlStateValueOff];
        }

        CefRefPtr<CefMenuModel> sub = model->GetSubMenuAt(i);
        if (sub) {
            [item setSubmenu:BuildNSMenuFromModel(sub, target)];
        }

        [menu addItem:item];
    }
    return menu;
}

}  // anonymous namespace

// C++ entry point — simple_handler.cpp's RunContextMenu calls this on mac.
// Returns true if we successfully presented a menu (CEF must not auto-show).
bool PresentContextMenuMac(CefRefPtr<CefBrowser> browser,
                           CefRefPtr<CefContextMenuParams> params,
                           CefRefPtr<CefMenuModel> model,
                           CefRefPtr<CefRunContextMenuCallback> callback) {
    if (!browser || !model || !callback) return false;
    if (model->GetCount() == 0) {
        // Our OnBeforeContextMenu cleared the model and added nothing — don't
        // show an empty menu; cancel so CEF doesn't wait.
        callback->Cancel();
        return true;
    }

    // Resolve the host NSView. On macOS GetWindowHandle returns the NSView*
    // that was passed to SetAsChild at browser creation.
    NSView* hostView = (__bridge NSView*)browser->GetHost()->GetWindowHandle();
    if (!hostView) {
        LOG_WARNING_CM("PresentContextMenuMac: host NSView is null — cancelling");
        callback->Cancel();
        return true;
    }

    HodosContextMenuTarget* target = [[HodosContextMenuTarget alloc] init];
    target.callback = callback;

    NSMenu* menu = BuildNSMenuFromModel(model, target);

    // CefContextMenuParams gives coords in browser-view space (top-left origin).
    // Convert to host NSView's coordinate space (bottom-left origin on macOS,
    // unless the view is flipped).
    int cef_x = params->GetXCoord();
    int cef_y = params->GetYCoord();
    NSPoint location;
    if ([hostView isFlipped]) {
        location = NSMakePoint((CGFloat)cef_x, (CGFloat)cef_y);
    } else {
        NSRect bounds = [hostView bounds];
        location = NSMakePoint((CGFloat)cef_x,
                               bounds.size.height - (CGFloat)cef_y);
    }

    LOG_INFO_CM("🖱️ Presenting context menu at (" + std::to_string(cef_x) + "," +
                std::to_string(cef_y) + ") with " +
                std::to_string(model->GetCount()) + " items");

    // popUpMenuPositioningItem:atLocation:inView: blocks until the menu closes.
    // It returns YES if an item was selected (itemPicked: already fired in
    // that case); NO if dismissed. In either case, HodosContextMenuTarget
    // guards against double-continue via its `fired` flag.
    BOOL picked = [menu popUpMenuPositioningItem:nil
                                      atLocation:location
                                          inView:hostView];
    if (!picked) {
        [target menuCancelled];
    }
    return true;
}
