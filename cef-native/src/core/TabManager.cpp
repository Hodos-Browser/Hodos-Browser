#include "../../include/core/TabManager.h"
#include "../../include/core/SessionManager.h"
#include "../../include/core/EphemeralCookieManager.h"
#include "../../include/core/WindowManager.h"
#include "../../include/handlers/simple_handler.h"
#include "include/cef_app.h"
#include "include/wrapper/cef_helpers.h"
#include "include/wrapper/cef_closure_task.h"
#include "include/base/cef_bind.h"
#include "include/cef_task.h"
#include <algorithm>
#include <set>
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

int TabManager::CreateTab(const std::string& url, HWND parent_hwnd, int x, int y, int width, int height, int window_id) {
    CEF_REQUIRE_UI_THREAD();

    int tab_id = GetNextTabId();
    std::string tab_url = url.empty() ? "http://127.0.0.1:5137/newtab" : url;

    LOG(INFO) << "Creating tab " << tab_id << " with URL: " << tab_url << " in window " << window_id;
    LOG(INFO) << "Tab position: x=" << x << ", y=" << y << ", width=" << width << ", height=" << height;

    // Create Tab struct
    Tab new_tab(tab_id, tab_url);
    new_tab.window_id = window_id;

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

    new_tab.handler = new SimpleHandler(role, window_id);
    LOG(INFO) << "Created SimpleHandler for tab " << tab_id << " with role: " << role;

    // Add tab to map BEFORE creating browser
    // This is important because OnAfterCreated will be called asynchronously
    // and it needs to find the tab in the map
    tabs_[tab_id] = new_tab;
    tab_order_.push_back(tab_id);

    // Notify the owning window's frontend so tab UI renders before page navigation starts.
    // Uses per-window notification to avoid triggering unnecessary re-renders in other windows.
    SimpleHandler::NotifyWindowTabListChanged(window_id);

    // Configure CEF browser settings
    RECT cef_rect;
    GetClientRect(tab_hwnd, &cef_rect);
    int cef_width = cef_rect.right - cef_rect.left;
    int cef_height = cef_rect.bottom - cef_rect.top;

    CefWindowInfo window_info;
    window_info.SetAsChild(tab_hwnd, CefRect(0, 0, cef_width, cef_height));

    CefBrowserSettings browser_settings;
    // Match new tab page background color to eliminate white/black flash during tab switch.
    // Same value as macOS TabManager_mac.mm — dark grey (26, 26, 26).
    browser_settings.background_color = CefColorSetARGB(255, 26, 26, 26);

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
    int win_id = tab.window_id;
    LOG(INFO) << "Closing tab " << tab_id << " (URL: " << tab.url << ") in window " << win_id;

    // If this is the active tab for its window, switch to another tab in the same window
    int activeForWindow = GetActiveTabIdForWindow(win_id);
    if (tab_id == activeForWindow) {
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

    // Mute audio before closing to prevent background playback during async teardown
    if (tab.browser) {
        tab.browser->GetHost()->SetAudioMuted(true);
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
    int closed_window_id = tab.window_id;
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

    // Auto-close window when its last tab closes (matches Chrome/Brave behavior)
    int remaining_in_window = 0;
    for (const auto& pair : tabs_) {
        if (pair.second.window_id == closed_window_id) {
            remaining_in_window++;
        }
    }

    if (remaining_in_window == 0) {
        BrowserWindow* bw = WindowManager::GetInstance().GetWindow(closed_window_id);
        if (bw && bw->hwnd && IsWindow(bw->hwnd)) {
            LOG(INFO) << "Last tab closed in window " << closed_window_id << " — closing window";
            PostMessage(bw->hwnd, WM_CLOSE, 0, 0);
        }
    }
}

bool TabManager::SwitchToTab(int tab_id) {
    CEF_REQUIRE_UI_THREAD();

    auto it = tabs_.find(tab_id);
    if (it == tabs_.end()) {
        LOG(WARNING) << "Attempted to switch to non-existent tab " << tab_id;
        return false;
    }

    int target_window = it->second.window_id;
    LOG(INFO) << "Switching to tab " << tab_id << " in window " << target_window;

    // Show the new tab FIRST, then hide the others.
    // This eliminates the black flash caused by the parent window's BLACK_BRUSH
    // background being exposed between hide-all and show-new.
    Tab& tab = it->second;
    if (tab.hwnd && IsWindow(tab.hwnd)) {
        ShowWindow(tab.hwnd, SW_SHOW);
        tab.is_visible = true;
        tab.last_accessed = std::chrono::system_clock::now();

        if (tab.browser) {
            tab.browser->GetHost()->SetFocus(true);
            tab.browser->GetHost()->WasResized();
        }
    }

    // Now hide all other tabs in the same window
    for (auto& pair : tabs_) {
        Tab& other = pair.second;
        if (other.id != tab_id && other.window_id == target_window &&
            other.hwnd && IsWindow(other.hwnd) && other.is_visible) {
            ShowWindow(other.hwnd, SW_HIDE);
            other.is_visible = false;
        }
    }

    active_tab_id_ = tab_id;
    active_tab_per_window_[target_window] = tab_id;
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
    // Validate: all IDs in the order must exist in tabs_
    for (int id : order) {
        if (tabs_.find(id) == tabs_.end()) {
            LOG(WARNING) << "ReorderTabs: tab ID " << id << " not found";
            return false;
        }
    }

    // Multi-window safe: the frontend sends only its window's tab IDs.
    // Rebuild tab_order_ keeping tabs NOT in this reorder set in their
    // original positions, then append the reordered tabs at end.
    std::set<int> reordered_set(order.begin(), order.end());
    std::vector<int> new_order;
    new_order.reserve(tab_order_.size());

    for (int id : tab_order_) {
        if (reordered_set.find(id) == reordered_set.end()) {
            new_order.push_back(id);
        }
    }
    for (int id : order) {
        new_order.push_back(id);
    }

    tab_order_ = new_order;
    LOG(INFO) << "Tabs reordered successfully (" << order.size() << " tabs in subset)";
    return true;
}

// ========== Tab Move (Tear-off / Merge) ==========

bool TabManager::MoveTabToWindow(int tab_id, int target_window_id, int insert_index) {
    CEF_REQUIRE_UI_THREAD();

    Tab* tab = GetTab(tab_id);
    if (!tab) {
        LOG(WARNING) << "MoveTabToWindow: tab " << tab_id << " not found";
        return false;
    }

    int source_window_id = tab->window_id;
    if (source_window_id == target_window_id) {
        LOG(WARNING) << "MoveTabToWindow: tab " << tab_id << " already in window " << target_window_id;
        return false;
    }

    BrowserWindow* target_bw = WindowManager::GetInstance().GetWindow(target_window_id);
    if (!target_bw) {
        LOG(WARNING) << "MoveTabToWindow: target window " << target_window_id << " not found";
        return false;
    }

#ifdef _WIN32
    // 1. Reparent the HWND to the target window
    if (tab->hwnd && target_bw->hwnd) {
        SetParent(tab->hwnd, target_bw->hwnd);

        // Resize tab to fit target window
        RECT r;
        GetClientRect(target_bw->hwnd, &r);
        int totalH = r.bottom - r.top;
        int shellH = (std::max)(100, static_cast<int>(totalH * 0.10));
        int tabH = totalH - shellH;
        SetWindowPos(tab->hwnd, nullptr, 0, shellH, r.right - r.left, tabH,
                     SWP_NOZORDER | SWP_NOACTIVATE);
    }
#endif

    // 2. Update tab ownership
    LOG(INFO) << "MoveTabToWindow: moving tab " << tab_id
              << " from window " << source_window_id << " to window " << target_window_id;
    tab->window_id = target_window_id;

    // 3. Update handler's window_id so IPC routes correctly
    if (tab->handler) {
        tab->handler->SetWindowId(target_window_id);
    }

    // 4. If this was the active tab in the source window, switch source to another tab
    int source_active = GetActiveTabIdForWindow(source_window_id);
    if (source_active == tab_id) {
        int replacement = -1;
        for (auto& [id, t] : tabs_) {
            if (id != tab_id && t.window_id == source_window_id) {
                replacement = id;
                break;
            }
        }
        if (replacement != -1) {
            SwitchToTab(replacement);
        } else {
            active_tab_per_window_.erase(source_window_id);
        }
    }

    // 5. Switch to the moved tab in the target window (show it, hide others)
    SwitchToTab(tab_id);

    // 6. Notify both windows
    SimpleHandler::NotifyWindowTabListChanged(source_window_id);
    SimpleHandler::NotifyWindowTabListChanged(target_window_id);

    // 7. Auto-close source window if it has no tabs left
    int remaining = 0;
    for (auto& [id, t] : tabs_) {
        if (t.window_id == source_window_id) remaining++;
    }
    if (remaining == 0) {
        BrowserWindow* src_bw = WindowManager::GetInstance().GetWindow(source_window_id);
        if (src_bw && src_bw->hwnd && IsWindow(src_bw->hwnd)) {
            LOG(INFO) << "MoveTabToWindow: source window " << source_window_id
                      << " has no tabs left — closing";
            PostMessage(src_bw->hwnd, WM_CLOSE, 0, 0);
        }
    }

    LOG(INFO) << "MoveTabToWindow: tab " << tab_id << " moved successfully";
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

    auto closing_it = tabs_.find(closing_tab_id);
    if (closing_it == tabs_.end()) return -1;
    int closing_window = closing_it->second.window_id;

    // If closing a tab that's not the active tab for its window, keep the window's active tab
    int activeForWindow = GetActiveTabIdForWindow(closing_window);
    if (closing_tab_id != activeForWindow) {
        return activeForWindow;
    }

    // Find most recently accessed tab in the SAME WINDOW (excluding the one being closed)
    int most_recent_tab_id = -1;
    std::chrono::system_clock::time_point most_recent_time;

    for (const auto& pair : tabs_) {
        if (pair.first == closing_tab_id) continue;
        if (pair.second.window_id != closing_window) continue;

        const Tab& tab = pair.second;
        if (most_recent_tab_id == -1 || tab.last_accessed > most_recent_time) {
            most_recent_tab_id = tab.id;
            most_recent_time = tab.last_accessed;
        }
    }

    return most_recent_tab_id;
}

int TabManager::GetActiveTabIdForWindow(int window_id) const {
    auto it = active_tab_per_window_.find(window_id);
    if (it != active_tab_per_window_.end()) {
        return it->second;
    }
    // Fallback: if no per-window tracking yet, check if global active tab is in this window
    if (active_tab_id_ != -1) {
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
