#include "../../include/core/TabManager.h"
#include "../../include/core/SessionManager.h"
#include "../../include/core/EphemeralCookieManager.h"
#include "../../include/handlers/simple_handler.h"
#include "include/cef_app.h"
#include "include/wrapper/cef_helpers.h"
#include "include/wrapper/cef_closure_task.h"
#include "include/base/cef_bind.h"
#include "include/cef_task.h"
#include <algorithm>
#include <sstream>

// External global variables from cef_browser_shell.cpp
extern HINSTANCE g_hInstance;
extern HWND g_hwnd;  // Main shell window

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
    LOG(INFO) << "TabManager initialized";
}

TabManager::~TabManager() {
    // Close all tabs on destruction
    for (auto& pair : tabs_) {
        Tab& tab = pair.second;
        if (tab.browser) {
            tab.browser->GetHost()->CloseBrowser(false);
        }
        if (tab.hwnd && IsWindow(tab.hwnd)) {
            DestroyWindow(tab.hwnd);
        }
    }
    tabs_.clear();
    LOG(INFO) << "TabManager destroyed - all tabs closed";
}

// ========== Tab Lifecycle Methods ==========

int TabManager::CreateTab(const std::string& url, HWND parent_hwnd, int x, int y, int width, int height) {
    CEF_REQUIRE_UI_THREAD();

    int tab_id = GetNextTabId();
    std::string tab_url = url.empty() ? "http://127.0.0.1:5137/newtab" : url;

    LOG(INFO) << "Creating tab " << tab_id << " with URL: " << tab_url;
    LOG(INFO) << "Tab position: x=" << x << ", y=" << y << ", width=" << width << ", height=" << height;

    // Create Tab struct
    Tab new_tab(tab_id, tab_url);

    // Create HWND for this tab's browser
    // Use same window style as header (which works correctly)
    HWND tab_hwnd = CreateWindow(
        L"CEFHostWindow",
        nullptr,
        WS_CHILD | WS_VISIBLE,  // Same as header - no WS_CLIPCHILDREN
        x, y, width, height,
        parent_hwnd,
        nullptr,
        g_hInstance,
        nullptr
    );

    if (!tab_hwnd) {
        LOG(ERROR) << "Failed to create HWND for tab " << tab_id;
        return -1;
    }

    new_tab.hwnd = tab_hwnd;

    // Create SimpleHandler for this tab
    // Role string format: "tab_{id}" - used to identify tab in callbacks
    std::stringstream role_ss;
    role_ss << "tab_" << tab_id;
    std::string role = role_ss.str();

    new_tab.handler = new SimpleHandler(role);
    LOG(INFO) << "Created SimpleHandler for tab " << tab_id << " with role: " << role;

    // Add tab to map BEFORE creating browser
    // This is important because OnAfterCreated will be called asynchronously
    // and it needs to find the tab in the map
    tabs_[tab_id] = new_tab;
    tab_order_.push_back(tab_id);

    // Notify frontend immediately so tab UI renders before page navigation starts
    // This prevents the jarring effect where the page loads before the tab appears
    SimpleHandler::NotifyTabListChanged();

    // Configure CEF browser settings
    RECT cef_rect;
    GetClientRect(tab_hwnd, &cef_rect);
    int cef_width = cef_rect.right - cef_rect.left;
    int cef_height = cef_rect.bottom - cef_rect.top;

    CefWindowInfo window_info;
    window_info.SetAsChild(tab_hwnd, CefRect(0, 0, cef_width, cef_height));

    CefBrowserSettings browser_settings;
    // Use default settings for now

    // Create browser asynchronously
    // The browser will be associated with the tab in RegisterTabBrowser()
    // when OnAfterCreated is called
    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        new_tab.handler,
        tab_url,
        browser_settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (!result) {
        LOG(ERROR) << "Failed to create browser for tab " << tab_id;
        DestroyWindow(tab_hwnd);
        tabs_.erase(tab_id);
        return -1;
    }

    LOG(INFO) << "Browser creation initiated for tab " << tab_id;

    // Switch to the new tab (will be shown when browser is created)
    SwitchToTab(tab_id);

    // Queue WM_SIZE to fix positioning (important for maximized windows)
    RECT parentRect;
    GetClientRect(parent_hwnd, &parentRect);
    PostMessage(parent_hwnd, WM_SIZE, SIZE_RESTORED,
                MAKELPARAM(parentRect.right - parentRect.left, parentRect.bottom - parentRect.top));

    return tab_id;
}

bool TabManager::CloseTab(int tab_id) {
    CEF_REQUIRE_UI_THREAD();

    auto it = tabs_.find(tab_id);
    if (it == tabs_.end()) {
        LOG(WARNING) << "Attempted to close non-existent tab " << tab_id;
        return false;
    }

    Tab& tab = it->second;
    LOG(INFO) << "Closing tab " << tab_id << " (URL: " << tab.url << ")";

    // If this is the active tab, switch to another tab BEFORE closing
    if (tab_id == active_tab_id_ && tabs_.size() > 1) {
        int new_active_id = FindTabToSwitchTo(tab_id);
        if (new_active_id != -1) {
            SwitchToTab(new_active_id);
        }
    }

    // Clear session spending and notify ephemeral cookie manager
    if (tab.browser) {
        SessionManager::GetInstance().clearSession(tab.browser->GetIdentifier());
        EphemeralCookieManager::GetInstance().OnTabClosed(tab.browser->GetIdentifier());
    }

    // Request browser to close
    // OnBeforeClose will be called when CEF is ready, then we'll destroy the HWND
    if (tab.browser) {
        LOG(INFO) << "Requesting browser close for tab " << tab_id;
        tab.browser->GetHost()->CloseBrowser(false);
    } else {
        // No browser exists yet, safe to clean up immediately
        OnTabBrowserClosed(tab_id);
    }

    return true;
}

void TabManager::OnTabBrowserClosed(int tab_id) {
    CEF_REQUIRE_UI_THREAD();

    auto it = tabs_.find(tab_id);
    if (it == tabs_.end()) {
        LOG(WARNING) << "OnTabBrowserClosed called for non-existent tab " << tab_id;
        return;
    }

    Tab& tab = it->second;
    LOG(INFO) << "Browser closed callback for tab " << tab_id << " - cleaning up resources";

    // Clear the browser reference first (safe)
    tab.browser = nullptr;
    tab.is_closing = true;

    // Hide the HWND immediately (safe)
    if (tab.hwnd && IsWindow(tab.hwnd)) {
        ShowWindow(tab.hwnd, SW_HIDE);
        LOG(INFO) << "Tab " << tab_id << " HWND hidden";

        // Use PostMessage to destroy HWND asynchronously
        // This queues the destruction and gives CEF time to clean up child windows
        HWND hwnd_to_destroy = tab.hwnd;
        PostMessage(hwnd_to_destroy, WM_DESTROY, 0, 0);

        tab.hwnd = nullptr;
        LOG(INFO) << "Tab " << tab_id << " HWND destruction queued";
    }

    // Remove from display order and map
    tab_order_.erase(std::remove(tab_order_.begin(), tab_order_.end(), tab_id), tab_order_.end());
    tabs_.erase(it);

    LOG(INFO) << "Tab " << tab_id << " removed from map. Remaining tabs: " << tabs_.size();

    // Update active tab ID if needed
    if (tabs_.empty()) {
        active_tab_id_ = -1;
        LOG(INFO) << "All tabs closed";
    }
}

bool TabManager::SwitchToTab(int tab_id) {
    CEF_REQUIRE_UI_THREAD();

    auto it = tabs_.find(tab_id);
    if (it == tabs_.end()) {
        LOG(WARNING) << "Attempted to switch to non-existent tab " << tab_id;
        return false;
    }

    LOG(INFO) << "Switching to tab " << tab_id;

    // Hide all tabs
    for (auto& pair : tabs_) {
        Tab& tab = pair.second;
        if (tab.hwnd && IsWindow(tab.hwnd)) {
            ShowWindow(tab.hwnd, SW_HIDE);
        }
        tab.is_visible = false;
    }

    // Show the selected tab
    Tab& tab = it->second;
    if (tab.hwnd && IsWindow(tab.hwnd)) {
        ShowWindow(tab.hwnd, SW_SHOW);
        tab.is_visible = true;
        tab.last_accessed = std::chrono::system_clock::now();

        // Set focus and notify CEF of resize
        if (tab.browser) {
            tab.browser->GetHost()->SetFocus(true);
            tab.browser->GetHost()->WasResized();
        }
    }

    active_tab_id_ = tab_id;
    LOG(INFO) << "Switched to tab " << tab_id << " (URL: " << tab.url << ")";

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
        LOG(WARNING) << "ReorderTabs: size mismatch - order has " << order.size()
                     << " but " << tabs_.size() << " tabs exist";
        return false;
    }
    for (int id : order) {
        if (tabs_.find(id) == tabs_.end()) {
            LOG(WARNING) << "ReorderTabs: tab ID " << id << " not found";
            return false;
        }
    }
    tab_order_ = order;
    LOG(INFO) << "Tabs reordered successfully";
    return true;
}

// ========== Tab State Update Methods ==========

void TabManager::UpdateTabTitle(int tab_id, const std::string& title) {
    Tab* tab = GetTab(tab_id);
    if (tab) {
        tab->title = title;
        LOG(INFO) << "Tab " << tab_id << " title updated to: " << title;
    }
}

void TabManager::UpdateTabURL(int tab_id, const std::string& url) {
    Tab* tab = GetTab(tab_id);
    if (tab) {
        tab->url = url;
        LOG(INFO) << "Tab " << tab_id << " URL updated to: " << url;
    }
}

void TabManager::UpdateTabLoadingState(int tab_id, bool is_loading, bool can_go_back, bool can_go_forward) {
    Tab* tab = GetTab(tab_id);
    if (tab) {
        tab->is_loading = is_loading;
        tab->can_go_back = can_go_back;
        tab->can_go_forward = can_go_forward;
        LOG(INFO) << "Tab " << tab_id << " loading state: "
                  << (is_loading ? "loading" : "loaded")
                  << ", back: " << (can_go_back ? "yes" : "no")
                  << ", forward: " << (can_go_forward ? "yes" : "no");
    }
}

void TabManager::UpdateTabFavicon(int tab_id, const std::string& favicon_url) {
    Tab* tab = GetTab(tab_id);
    if (tab) {
        tab->favicon_url = favicon_url;
        LOG(INFO) << "Tab " << tab_id << " favicon updated to: " << favicon_url;
    }
}

// ========== Browser Registration ==========

bool TabManager::RegisterTabBrowser(int tab_id, CefRefPtr<CefBrowser> browser) {
    CEF_REQUIRE_UI_THREAD();

    Tab* tab = GetTab(tab_id);
    if (!tab) {
        LOG(ERROR) << "Cannot register browser - tab " << tab_id << " not found";
        return false;
    }

    tab->browser = browser;
    LOG(INFO) << "Registered browser for tab " << tab_id
              << " (Browser ID: " << browser->GetIdentifier() << ")";

    // If this tab is visible, make sure browser has focus and is resized
    if (tab->is_visible && tab->hwnd) {
        browser->GetHost()->SetFocus(true);
        browser->GetHost()->WasResized();
    }

    return true;
}

// ========== Private Helper Methods ==========

int TabManager::FindTabToSwitchTo(int closing_tab_id) {
    if (tabs_.empty()) {
        return -1;
    }

    // If closing a tab that's not active, keep current active tab
    if (closing_tab_id != active_tab_id_) {
        return active_tab_id_;
    }

    // Find most recently accessed tab (excluding the one being closed)
    int most_recent_tab_id = -1;
    std::chrono::system_clock::time_point most_recent_time;

    for (const auto& pair : tabs_) {
        if (pair.first == closing_tab_id) {
            continue; // Skip the tab being closed
        }

        const Tab& tab = pair.second;
        if (most_recent_tab_id == -1 || tab.last_accessed > most_recent_time) {
            most_recent_tab_id = tab.id;
            most_recent_time = tab.last_accessed;
        }
    }

    return most_recent_tab_id;
}
