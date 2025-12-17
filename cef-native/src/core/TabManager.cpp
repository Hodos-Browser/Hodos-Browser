#include "../../include/core/TabManager.h"
#include "../../include/handlers/simple_handler.h"
#include "include/cef_app.h"
#include "include/wrapper/cef_helpers.h"
#include "include/wrapper/cef_closure_task.h"
#include "include/base/cef_bind.h"
#include "include/cef_task.h"
#include <algorithm>
#include <sstream>
#include <iostream>
#include <fstream>

// Helper function to write to diagnostic log file
void LogToFile(const std::string& message) {
    std::ofstream logFile("tab_diagnostics.log", std::ios::app);
    if (logFile.is_open()) {
        logFile << message << std::endl;
        logFile.close();
    }
}

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
    LOG(INFO) << "Created HWND for tab " << tab_id << ": " << tab_hwnd;

    // COMPREHENSIVE DIAGNOSTIC LOGGING TO FILE
    std::stringstream log;
    log << "\n========================================\n";
    log << "📐 TAB " << tab_id << " CREATION DIAGNOSTICS\n";
    log << "========================================\n";
    log << "URL: " << tab_url << "\n";

    // Check parent window extended styles (RTL layout check)
    LONG_PTR parentExStyle = GetWindowLongPtr(parent_hwnd, GWL_EXSTYLE);
    log << "\nPARENT WINDOW EXTENDED STYLES:\n";
    log << "  Extended Style Flags: 0x" << std::hex << parentExStyle << std::dec << "\n";
    log << "  Has WS_EX_LAYOUTRTL: " << ((parentExStyle & WS_EX_LAYOUTRTL) ? "YES (RTL LAYOUT ENABLED!)" : "NO") << "\n";
    log << "  Has WS_EX_RIGHT: " << ((parentExStyle & WS_EX_RIGHT) ? "YES" : "NO") << "\n";
    log << "  Has WS_EX_RTLREADING: " << ((parentExStyle & WS_EX_RTLREADING) ? "YES" : "NO") << "\n";

    // Check tab window extended styles
    LONG_PTR tabExStyle = GetWindowLongPtr(tab_hwnd, GWL_EXSTYLE);
    log << "\nTAB WINDOW EXTENDED STYLES:\n";
    log << "  Extended Style Flags: 0x" << std::hex << tabExStyle << std::dec << "\n";
    log << "  Has WS_EX_LAYOUTRTL: " << ((tabExStyle & WS_EX_LAYOUTRTL) ? "YES" : "NO") << "\n";

    log << "\nREQUESTED DIMENSIONS:\n";
    log << "  Position: (" << x << ", " << y << ")\n";
    log << "  Size: " << width << "x" << height << "\n";

    // Get window rect BEFORE SetWindowPos
    RECT windowRectBefore;
    GetWindowRect(tab_hwnd, &windowRectBefore);
    log << "\nWINDOW RECT IMMEDIATELY AFTER CreateWindow:\n";
    log << "  Left: " << windowRectBefore.left << "\n";
    log << "  Top: " << windowRectBefore.top << "\n";
    log << "  Right: " << windowRectBefore.right << "\n";
    log << "  Bottom: " << windowRectBefore.bottom << "\n";

    // Get window rect AFTER SetWindowPos
    RECT windowRect;
    GetWindowRect(tab_hwnd, &windowRect);
    log << "\nWINDOW RECT AFTER SetWindowPos CORRECTION:\n";
    log << "  Left: " << windowRect.left << "\n";
    log << "  Top: " << windowRect.top << "\n";
    log << "  Right: " << windowRect.right << "\n";
    log << "  Bottom: " << windowRect.bottom << "\n";
    log << "  Width: " << (windowRect.right - windowRect.left) << "\n";
    log << "  Height: " << (windowRect.bottom - windowRect.top) << "\n";

    // Get client rect (content area)
    RECT clientRect;
    GetClientRect(tab_hwnd, &clientRect);
    int clientWidth = clientRect.right - clientRect.left;
    int clientHeight = clientRect.bottom - clientRect.top;
    log << "\nCLIENT RECT (content area):\n";
    log << "  Width: " << clientWidth << "\n";
    log << "  Height: " << clientHeight << "\n";

    // Check for mismatches
    if (clientWidth != width || clientHeight != height) {
        log << "\n⚠️ CLIENT AREA MISMATCH!\n";
        log << "  Expected: " << width << "x" << height << "\n";
        log << "  Got: " << clientWidth << "x" << clientHeight << "\n";
        log << "  Difference: " << (width - clientWidth) << "x" << (height - clientHeight) << "\n";
    }

    // Get parent window info for comparison
    RECT parentClientRect;
    GetClientRect(parent_hwnd, &parentClientRect);
    RECT parentWindowRect;
    GetWindowRect(parent_hwnd, &parentWindowRect);

    log << "\nPARENT WINDOW (g_hwnd) INFO:\n";
    log << "  Client Rect Width: " << (parentClientRect.right - parentClientRect.left) << "\n";
    log << "  Client Rect Height: " << (parentClientRect.bottom - parentClientRect.top) << "\n";
    log << "  Screen Position: Left=" << parentWindowRect.left << ", Top=" << parentWindowRect.top << "\n";
    log << "  Screen Size: " << (parentWindowRect.right - parentWindowRect.left) << "x"
        << (parentWindowRect.bottom - parentWindowRect.top) << "\n";

    // Calculate where child SHOULD be based on parent position
    int expectedScreenX = parentWindowRect.left + x;
    int expectedScreenY = parentWindowRect.top + y;
    log << "\nEXPECTED CHILD SCREEN POSITION:\n";
    log << "  Should be at screen: (" << expectedScreenX << ", " << expectedScreenY << ")\n";
    log << "  Actual screen: (" << windowRect.left << ", " << windowRect.top << ")\n";
    log << "  Offset: (" << (windowRect.left - expectedScreenX) << ", " << (windowRect.top - expectedScreenY) << ")\n";

    log << "========================================\n";

    LogToFile(log.str());
    std::cout << "✅ Tab " << tab_id << " diagnostics written to tab_diagnostics.log" << std::endl;

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
    RECT cef_rect;
    GetClientRect(tab_hwnd, &cef_rect);
    int cef_width = cef_rect.right - cef_rect.left;
    int cef_height = cef_rect.bottom - cef_rect.top;

    std::stringstream cef_log;
    cef_log << "\nCEF BROWSER CONFIGURATION:\n";
    cef_log << "  CefRect: (0, 0, " << cef_width << ", " << cef_height << ")\n";
    cef_log << "  Parent HWND: " << tab_hwnd << "\n";
    LogToFile(cef_log.str());

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

    // CRITICAL FIX: Queue WM_SIZE to force correct tab positioning
    // This fixes the offset issue when creating tabs while window is maximized
    // PostMessage queues WM_SIZE for after current operations complete
    RECT parentRect;
    GetClientRect(parent_hwnd, &parentRect);
    int parentWidth = parentRect.right - parentRect.left;
    int parentHeight = parentRect.bottom - parentRect.top;

    // Use PostMessage (async) instead of SendMessage (sync) to queue repositioning
    PostMessage(parent_hwnd, WM_SIZE, SIZE_RESTORED, MAKELPARAM(parentWidth, parentHeight));

    LOG(INFO) << "Queued WM_SIZE to reposition tab " << tab_id;

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

    // Remove from map now that resources are queued for cleanup
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
