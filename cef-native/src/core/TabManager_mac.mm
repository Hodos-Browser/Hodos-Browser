// macOS implementation of TabManager
// Uses NSView instead of HWND for tab management

#import <Cocoa/Cocoa.h>

#include "../../include/core/TabManager.h"
#include "../../include/core/WindowManager.h"
#include "../../include/core/EphemeralCookieManager.h"
#include "../../include/core/Logger.h"
#include "../../include/handlers/simple_handler.h"
#include "include/cef_app.h"
#include "include/wrapper/cef_helpers.h"
#include "include/wrapper/cef_closure_task.h"
#include "include/base/cef_bind.h"
#include "include/cef_task.h"
#include <algorithm>
#include <sstream>

// Convenience macros for logging
#define LOG_INFO(msg) Logger::Log(msg, 1, 0)
#define LOG_WARNING(msg) Logger::Log(msg, 2, 0)
#define LOG_ERROR(msg) Logger::Log(msg, 3, 0)

// Singleton instance
std::unique_ptr<TabManager> TabManager::instance_ = nullptr;

// ========== Singleton Pattern ==========

TabManager& TabManager::GetInstance() {
    if (!instance_) {
        instance_ = std::unique_ptr<TabManager>(new TabManager());
    }
    return *instance_;
}

TabManager::TabManager()
    : active_tab_id_(-1),
      next_tab_id_(1) {
    LOG_INFO("TabManager initialized (macOS)");
}

TabManager::~TabManager() {
    // Close all tabs on destruction
    for (auto& pair : tabs_) {
        Tab& tab = pair.second;
        if (tab.browser) {
            tab.browser->GetHost()->CloseBrowser(false);
        }
        if (tab.view_ptr) {
            NSView* view = (__bridge NSView*)tab.view_ptr;
            [view removeFromSuperview];
        }
    }
    tabs_.clear();
    LOG_INFO("TabManager destroyed - all tabs closed (macOS)");
}

// ========== Tab Lifecycle Methods ==========

int TabManager::CreateTab(const std::string& url, void* parent_view, int x, int y, int width, int height, int window_id) {
    CEF_REQUIRE_UI_THREAD();

    int tab_id = GetNextTabId();
    std::string tab_url = url.empty() ? "http://127.0.0.1:5137/newtab" : url;

    LOG_INFO("Creating tab " + std::to_string(tab_id) + " with URL: " + tab_url);
    LOG_INFO("Tab position: x=" + std::to_string(x) + ", y=" + std::to_string(y) +
             ", width=" + std::to_string(width) + ", height=" + std::to_string(height));

    // Create Tab struct
    Tab new_tab(tab_id, tab_url);
    new_tab.window_id = window_id;

    // Create NSView for this tab's browser. Right-click handling lives in
    // SimpleHandler::RunContextMenu (macOS) — CEF's internal NSView receives
    // the event and routes it through OnBeforeContextMenu / RunContextMenu.
    NSView* parentView = (__bridge NSView*)parent_view;
    NSRect tabFrame = NSMakeRect(x, y, width, height);
    NSView* tabView = [[NSView alloc] initWithFrame:tabFrame];

    if (!tabView) {
        LOG_ERROR("Failed to create NSView for tab " + std::to_string(tab_id));
        return -1;
    }

    // Set autoresizing mask so tab follows parent resizing
    [tabView setAutoresizingMask:NSViewWidthSizable | NSViewHeightSizable];

    // Add to parent view
    [parentView addSubview:tabView];

    new_tab.view_ptr = (__bridge void*)tabView;

    // Create SimpleHandler for this tab
    // Role string format: "tab_{id}" - used to identify tab in callbacks
    std::stringstream role_ss;
    role_ss << "tab_" << tab_id;
    std::string role = role_ss.str();

    new_tab.handler = new SimpleHandler(role, window_id);
    LOG_INFO("Created SimpleHandler for tab " + std::to_string(tab_id) + " with role: " + role + " window_id: " + std::to_string(window_id));

    // Add tab to map BEFORE creating browser
    // OnAfterCreated will be called asynchronously and needs to find the tab
    tabs_[tab_id] = new_tab;
    tab_order_.push_back(tab_id);

    // Notify frontend that tab list changed
    SimpleHandler::NotifyTabListChanged();

    // Configure CEF browser for this tab
    NSRect viewBounds = [tabView bounds];

    CefWindowInfo window_info;
    CefRect cefRect(0, 0, (int)viewBounds.size.width, (int)viewBounds.size.height);
    window_info.SetAsChild((__bridge void*)tabView, cefRect);

    CefBrowserSettings browser_settings;
    browser_settings.background_color = CefColorSetARGB(255, 26, 26, 26);

    // Create browser asynchronously
    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        new_tab.handler,
        tab_url,
        browser_settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG_ERROR("Failed to create browser for tab " + std::to_string(tab_id));
        [tabView removeFromSuperview];
        tabs_.erase(tab_id);
        return -1;
    }

    LOG_INFO("Browser creation initiated for tab " + std::to_string(tab_id));

    // Switch to the new tab
    SwitchToTab(tab_id);

    return tab_id;
}

bool TabManager::CloseTab(int tab_id) {
    CEF_REQUIRE_UI_THREAD();

    auto it = tabs_.find(tab_id);
    if (it == tabs_.end()) {
        LOG_WARNING("Attempted to close non-existent tab " + std::to_string(tab_id));
        return false;
    }

    Tab& tab = it->second;
    LOG_INFO("=== CLOSE TAB START ===");
    LOG_INFO("Closing tab " + std::to_string(tab_id));
    LOG_INFO("Active tab ID: " + std::to_string(active_tab_id_));
    LOG_INFO("Total tabs: " + std::to_string(tabs_.size()));
    LOG_INFO("Is active tab: " + std::string(tab_id == active_tab_id_ ? "YES" : "NO"));

    // Guard against close cascading to window via responder chain
    extern bool g_closing_tab;
    g_closing_tab = true;

    // Mark as closing
    tab.is_closing = true;

    // If this is the active tab, switch to another tab FIRST
    if (tab_id == active_tab_id_ && tabs_.size() > 1) {
        int next_tab = FindTabToSwitchTo(tab_id);
        if (next_tab != -1) {
            Tab* next_tab_ptr = GetTab(next_tab);
            if (next_tab_ptr && !next_tab_ptr->is_closing) {
                SwitchToTab(next_tab);
            } else {
                active_tab_id_ = -1;
            }
        } else {
            active_tab_id_ = -1;
        }
    } else if (tab_id == active_tab_id_) {
        active_tab_id_ = -1;
    }

    // Notify ephemeral cookie manager before closing
    if (tab.browser) {
        EphemeralCookieManager::GetInstance().OnTabClosed(tab.browser->GetIdentifier());
    }

    // Mute audio before closing to prevent background playback during async teardown
    if (tab.browser) {
        tab.browser->GetHost()->SetAudioMuted(true);
    }

    // Initiate browser close
    if (tab.browser) {
        tab.browser->GetHost()->CloseBrowser(false);
        LOG_INFO("CloseBrowser called for tab " + std::to_string(tab_id));
    }

    LOG_INFO("CloseBrowser returned, about to remove view");

    // CRITICAL: Remove view IMMEDIATELY after CloseBrowser() returns
    if (tab.view_ptr) {
        NSView* view = (__bridge NSView*)tab.view_ptr;
        LOG_INFO("Hiding view...");
        [view setHidden:YES];
        LOG_INFO("Removing from superview...");
        [view removeFromSuperview];
        LOG_INFO("View removed from superview");
        tab.view_ptr = nullptr;
        LOG_INFO("Tab " + std::to_string(tab_id) + " view removed synchronously");
    }

    // Clear browser reference immediately (don't wait for OnBeforeClose)
    tab.browser = nullptr;
    LOG_INFO("Browser reference cleared");

    // Remove from display order and map
    tab_order_.erase(std::remove(tab_order_.begin(), tab_order_.end(), tab_id), tab_order_.end());
    tabs_.erase(it);
    LOG_INFO("Tab " + std::to_string(tab_id) + " removed from map. Remaining: " + std::to_string(tabs_.size()));

    // Update active tab if all tabs closed
    if (tabs_.empty()) {
        active_tab_id_ = -1;
        LOG_INFO("All tabs closed");
    }

    // Notify frontend of updated tab list
    SimpleHandler::NotifyTabListChanged();
    LOG_INFO("Frontend notified of tab list change");

    LOG_INFO("=== CLOSE TAB END ===");

    g_closing_tab = false;
    return true;
}

bool TabManager::SwitchToTab(int tab_id) {
    CEF_REQUIRE_UI_THREAD();

    auto it = tabs_.find(tab_id);
    if (it == tabs_.end()) {
        LOG_WARNING("Attempted to switch to non-existent tab " + std::to_string(tab_id));
        return false;
    }

    LOG_INFO("Switching to tab " + std::to_string(tab_id));

    // Hide only tabs within the same window. Hiding across windows causes
    // other windows to go white when we switch tabs here (multi-window bug).
    int target_window_id = it->second.window_id;
    for (auto& pair : tabs_) {
        Tab& tab = pair.second;
        if (tab.is_closing) continue;
        if (tab.window_id != target_window_id) continue;
        if (tab.view_ptr) {
            NSView* view = (__bridge NSView*)tab.view_ptr;
            [view setHidden:YES];
        }
        tab.is_visible = false;
    }

    // Show the selected tab (safety check)
    Tab& tab = it->second;

    if (tab.is_closing) {
        LOG_WARNING("Attempted to switch to closing tab " + std::to_string(tab_id));
        return false;
    }

    if (tab.view_ptr) {
        NSView* view = (__bridge NSView*)tab.view_ptr;
        [view setHidden:NO];
        tab.is_visible = true;
        tab.last_accessed = std::chrono::system_clock::now();

        if (tab.browser && tab.browser->GetHost()) {
            [[view window] makeFirstResponder:view];
            tab.browser->GetHost()->SetFocus(true);
            tab.browser->GetHost()->WasResized();
        }
    }

    active_tab_id_ = tab_id;
    LOG_INFO("Switched to tab " + std::to_string(tab_id) + " (URL: " + tab.url + ")");

    return true;
}

// ========== Tab Query Methods ==========

Tab* TabManager::GetTab(int tab_id) {
    auto it = tabs_.find(tab_id);
    if (it != tabs_.end()) {
        return &it->second;
    }
    return nullptr;
}

Tab* TabManager::GetActiveTab() {
    if (active_tab_id_ == -1) {
        return nullptr;
    }
    return GetTab(active_tab_id_);
}

std::vector<Tab*> TabManager::GetAllTabs() {
    std::vector<Tab*> all_tabs;
    all_tabs.reserve(tabs_.size());

    // Return tabs in explicit display order
    for (int id : tab_order_) {
        auto it = tabs_.find(id);
        if (it != tabs_.end()) {
            all_tabs.push_back(&it->second);
        }
    }

    // Safety: include any tabs not in tab_order_ (shouldn't happen, but defensive)
    if (all_tabs.size() < tabs_.size()) {
        for (auto& pair : tabs_) {
            if (std::find(tab_order_.begin(), tab_order_.end(), pair.first) == tab_order_.end()) {
                all_tabs.push_back(&pair.second);
            }
        }
    }

    return all_tabs;
}

bool TabManager::ReorderTabs(const std::vector<int>& order) {
    // Validate: all IDs must exist and count must match
    if (order.size() != tabs_.size()) {
        LOG_WARNING("ReorderTabs: size mismatch - order has " + std::to_string(order.size())
                    + " but " + std::to_string(tabs_.size()) + " tabs exist");
        return false;
    }
    for (int id : order) {
        if (tabs_.find(id) == tabs_.end()) {
            LOG_WARNING("ReorderTabs: tab ID " + std::to_string(id) + " not found");
            return false;
        }
    }
    tab_order_ = order;
    LOG_INFO("Tabs reordered successfully");
    return true;
}

// ========== Tab State Update Methods ==========

void TabManager::UpdateTabTitle(int tab_id, const std::string& title) {
    Tab* tab = GetTab(tab_id);
    if (tab) {
        tab->title = title;
        SimpleHandler::NotifyTabListChanged();
    }
}

void TabManager::UpdateTabURL(int tab_id, const std::string& url) {
    Tab* tab = GetTab(tab_id);
    if (tab) {
        tab->url = url;
        SimpleHandler::NotifyTabListChanged();
    }
}

void TabManager::UpdateTabLoadingState(int tab_id, bool is_loading, bool can_go_back, bool can_go_forward) {
    Tab* tab = GetTab(tab_id);
    if (tab) {
        tab->is_loading = is_loading;
        tab->can_go_back = can_go_back;
        tab->can_go_forward = can_go_forward;
        SimpleHandler::NotifyTabListChanged();
    }
}

void TabManager::UpdateTabFavicon(int tab_id, const std::string& favicon_url) {
    Tab* tab = GetTab(tab_id);
    if (tab) {
        tab->favicon_url = favicon_url;
        SimpleHandler::NotifyTabListChanged();
    }
}

// ========== Tab Reparenting (Multi-Window) ==========

bool TabManager::MoveTabToWindow(int tab_id, int target_window_id, int insert_index) {
    CEF_REQUIRE_UI_THREAD();

    // macOS implementation note (long-term architectural decision):
    //
    // CEF provides no API to reparent a windowed browser across NSWindows
    // (SetAsChild bakes in the parent view at creation; see cef_mac.h). Our
    // earlier approach — [tabView removeFromSuperview] + addSubview: into
    // the target window's container — left the source window's CALayer tree
    // in an undefined state and produced white-screen regressions for the
    // tab(s) left behind. WasHidden/Invalidate kicks do not reliably recover
    // it; WasHidden is documented as "only for windowless rendering".
    //
    // The correct long-term solution that respects CEF's lifecycle: destroy
    // the tab's CEF browser and recreate a fresh tab in the target window
    // with the same URL. History for that single tab is not preserved — an
    // accepted tradeoff, since Chromium/Brave preserve history only via
    // WebContents move which CEF does not expose. URL + title are kept.

    Tab* tab = GetTab(tab_id);
    if (!tab) {
        LOG_WARNING("MoveTabToWindow: tab " + std::to_string(tab_id) + " not found");
        return false;
    }

    int source_window_id = tab->window_id;
    if (source_window_id == target_window_id) {
        LOG_WARNING("MoveTabToWindow: tab " + std::to_string(tab_id) +
                    " already in window " + std::to_string(target_window_id));
        return false;
    }

    BrowserWindow* target_bw = WindowManager::GetInstance().GetWindow(target_window_id);
    if (!target_bw || !target_bw->webview_view) {
        LOG_WARNING("MoveTabToWindow: target window " + std::to_string(target_window_id) + " not found");
        return false;
    }

    std::string url = tab->url.empty() ? "http://127.0.0.1:5137/newtab" : tab->url;
    LOG_INFO("MoveTabToWindow: destroy+recreate tab " + std::to_string(tab_id) +
             " (" + url + ") from window " + std::to_string(source_window_id) +
             " to window " + std::to_string(target_window_id));

    // 1. Close the source tab's CEF browser cleanly. CloseTab handles view
    //    removal, active-tab replacement in the source window, and tab-list
    //    notifications.
    CloseTab(tab_id);

    // 2. Create a fresh tab in the target window at the same URL.
    NSView* targetView = (__bridge NSView*)target_bw->webview_view;
    NSRect bounds = [targetView bounds];
    int new_tab_id = CreateTab(url,
                               target_bw->webview_view,
                               0, 0,
                               (int)bounds.size.width,
                               (int)bounds.size.height,
                               target_window_id);

    if (new_tab_id < 0) {
        LOG_ERROR("MoveTabToWindow: failed to create replacement tab in target");
        return false;
    }

    // 3. Notify target window (source was notified inside CloseTab via
    //    NotifyWindowTabListChanged). CreateTab already calls
    //    NotifyTabListChanged; add the window-scoped notify too.
    SimpleHandler::NotifyWindowTabListChanged(target_window_id);

    // 4. If the source window has no tabs left, close it.
    int remaining = 0;
    for (auto& [id, t] : tabs_) {
        if (t.window_id == source_window_id) remaining++;
    }
    if (remaining == 0) {
        BrowserWindow* src_bw = WindowManager::GetInstance().GetWindow(source_window_id);
        if (src_bw && src_bw->ns_window) {
            NSWindow* srcWindow = (__bridge NSWindow*)src_bw->ns_window;
            LOG_INFO("MoveTabToWindow: source window " + std::to_string(source_window_id) +
                     " has no tabs left — closing");
            dispatch_async(dispatch_get_main_queue(), ^{
                [srcWindow close];
            });
        }
    }

    LOG_INFO("MoveTabToWindow: recreated as tab " + std::to_string(new_tab_id) +
             " in window " + std::to_string(target_window_id));
    return true;
}

// ========== Browser Registration ==========

bool TabManager::RegisterTabBrowser(int tab_id, CefRefPtr<CefBrowser> browser) {
    Tab* tab = GetTab(tab_id);
    if (!tab) {
        LOG_ERROR("RegisterTabBrowser called for non-existent tab " + std::to_string(tab_id));
        return false;
    }

    tab->browser = browser;
    LOG_INFO("Browser registered for tab " + std::to_string(tab_id));

    // Trigger initial resize/paint
    if (tab->is_visible) {
        browser->GetHost()->WasResized();
        browser->GetHost()->Invalidate(PET_VIEW);
    }

    return true;
}

void TabManager::OnTabBrowserClosed(int tab_id) {
    auto it = tabs_.find(tab_id);
    if (it == tabs_.end()) {
        LOG_WARNING("OnTabBrowserClosed called for non-existent tab " + std::to_string(tab_id));
        return;
    }

    Tab& tab = it->second;
    LOG_INFO("OnBeforeClose callback for tab " + std::to_string(tab_id));

    // ONLY clear browser reference (view already removed in CloseTab)
    tab.browser = nullptr;

    // Remove from display order and map
    tab_order_.erase(std::remove(tab_order_.begin(), tab_order_.end(), tab_id), tab_order_.end());
    tabs_.erase(it);
    LOG_INFO("Tab " + std::to_string(tab_id) + " removed from map. Remaining: " + std::to_string(tabs_.size()));

    // Update active tab if needed
    if (tabs_.empty()) {
        active_tab_id_ = -1;
        LOG_INFO("All tabs closed");
    }

    // Notify frontend
    SimpleHandler::NotifyTabListChanged();
}

// ========== Private Helper Methods ==========

int TabManager::FindTabToSwitchTo(int closing_tab_id) {
    if (tabs_.size() <= 1) {
        return -1;  // No other tabs
    }

    // Find most recently accessed tab (excluding the closing one)
    int best_tab_id = -1;
    auto latest_time = std::chrono::system_clock::time_point::min();

    for (const auto& pair : tabs_) {
        if (pair.first != closing_tab_id) {
            const Tab& tab = pair.second;
            if (tab.last_accessed > latest_time) {
                latest_time = tab.last_accessed;
                best_tab_id = pair.first;
            }
        }
    }

    return best_tab_id;
}

int TabManager::GetActiveTabIdForWindow(int window_id) const {
    auto it = active_tab_per_window_.find(window_id);
    if (it != active_tab_per_window_.end()) {
        return it->second;
    }
    // Fallback: check if global active tab is in this window
    if (active_tab_id_ >= 0) {
        auto tab_it = tabs_.find(active_tab_id_);
        if (tab_it != tabs_.end() && tab_it->second.window_id == window_id) {
            return active_tab_id_;
        }
    }
    return -1;
}

Tab* TabManager::GetActiveTabForWindow(int window_id) {
    int id = GetActiveTabIdForWindow(window_id);
    if (id == -1) return nullptr;
    return GetTab(id);
}
