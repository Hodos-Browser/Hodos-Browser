#include "../../include/core/TabManager.h"
#include "../../include/handlers/simple_handler.h"
#include "include/cef_app.h"
#include "include/wrapper/cef_helpers.h"
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
    std::string tab_url = url.empty() ? "https://metanetapps.com/" : url;

    LOG(INFO) << "Creating tab " << tab_id << " with URL: " << tab_url;
    LOG(INFO) << "Tab position: x=" << x << ", y=" << y << ", width=" << width << ", height=" << height;

    // Create Tab struct
    Tab new_tab(tab_id, tab_url);

    // Create HWND for this tab's browser
    // Each tab gets its own child window within the main window
    // Position below header, stacked with other tabs (show/hide to switch)
    HWND tab_hwnd = CreateWindowEx(
        0,
        L"CEFHostWindow",  // Window class (should already be registered)
        nullptr,
        WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,  // Visible child window
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
    LOG(INFO) << "Created HWND for tab " << tab_id << ": " << tab_hwnd;

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

    // Configure CEF browser settings
    CefWindowInfo window_info;
    window_info.SetAsChild(tab_hwnd, CefRect(0, 0, width, height));

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

    // Close the CEF browser
    if (tab.browser) {
        tab.browser->GetHost()->CloseBrowser(false);
        tab.browser = nullptr;
    }

    // Destroy the HWND
    if (tab.hwnd && IsWindow(tab.hwnd)) {
        DestroyWindow(tab.hwnd);
        tab.hwnd = nullptr;
    }

    // If this was the active tab, switch to another tab
    bool was_active = (tab_id == active_tab_id_);

    // Remove from map
    tabs_.erase(it);

    LOG(INFO) << "Tab " << tab_id << " closed. Remaining tabs: " << tabs_.size();

    // If we closed the active tab and there are other tabs, switch to one
    if (was_active && !tabs_.empty()) {
        int new_active_id = FindTabToSwitchTo(tab_id);
        if (new_active_id != -1) {
            SwitchToTab(new_active_id);
        } else {
            active_tab_id_ = -1;
        }
    } else if (tabs_.empty()) {
        active_tab_id_ = -1;
        LOG(INFO) << "All tabs closed";
    }

    return true;
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

    for (auto& pair : tabs_) {
        all_tabs.push_back(&pair.second);
    }

    return all_tabs;
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
