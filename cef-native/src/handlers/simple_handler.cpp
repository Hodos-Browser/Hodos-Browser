// cef_native/src/simple_handler.cpp
#include "../../include/handlers/simple_handler.h"
#include "../../include/handlers/simple_app.h"

// Platform-specific includes
#ifdef _WIN32
    #include "../../include/core/WalletService.h"
    #include "../../include/core/HttpRequestInterceptor.h"
    #include "../../include/core/AdblockCache.h"
    #include "../../include/core/LocalFileResourceHandler.h"
    #include "../../include/core/LayoutHelpers.h"
    #include <windows.h>
    #include <shobjidl.h>
    #include <shlobj.h>
    #include <thread>
#endif

#ifdef __APPLE__
    #include "../../include/core/WalletService.h"
    #include "../../include/core/HttpRequestInterceptor.h"
    #include "../../include/core/AdblockCache.h"
#endif

// Cross-platform includes (available on both platforms)
#include <cmath>
#include "../../include/core/TabManager.h"
#include "../../include/core/HistoryManager.h"
#include "../../include/core/GoogleSuggestService.h"
#include "../../include/core/CookieManager.h"
#include "../../include/core/CookieBlockManager.h"
#include "../../include/core/EphemeralCookieManager.h"
#include "../../include/core/BookmarkManager.h"
#include "../../include/core/SettingsManager.h"
#include "../../include/core/FingerprintProtection.h"
#include "../../include/core/ProfileManager.h"
#include "../../include/core/ProfileImporter.h"

#ifdef __APPLE__
    // Forward declarations (no Cocoa.h in .cpp files)
    #ifdef __OBJC__
        #import <Cocoa/Cocoa.h>
    #else
        struct NSView;
    #endif
#endif

#include "include/wrapper/cef_helpers.h"
#include "include/base/cef_bind.h"
#include "include/cef_app.h"
#include "include/cef_v8.h"
#include "include/wrapper/cef_closure_task.h"
#include "include/cef_task.h"
#include "include/internal/cef_types.h"  // For CEF_WOD_* constants
#include "base/cef_callback.h"
#include "base/internal/cef_callback_internal.h"
#include <fstream>
#include <filesystem>
#include <cstdlib>
#include <iostream>
#include <string>
#include <sstream>
#include <nlohmann/json.hpp>

#include "../../include/core/Logger.h"
#include "../../include/core/WindowManager.h"

// Convenience macros for easier logging
#define LOG_DEBUG_BROWSER(msg) Logger::Log(msg, 0, 2)
#define LOG_INFO_BROWSER(msg) Logger::Log(msg, 1, 2)
#define LOG_WARNING_BROWSER(msg) Logger::Log(msg, 2, 2)

#include "../../include/core/PendingAuthRequest.h"
extern std::string g_pendingModalDomain;
#define LOG_ERROR_BROWSER(msg) Logger::Log(msg, 3, 2)

// Platform-specific overlay function declarations (already in simple_app.h, but repeated for clarity)
#ifdef _WIN32
    extern void CreateTestOverlayWithSeparateProcess(HINSTANCE hInstance);
    extern void CreateWalletOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
    extern void ShowWalletOverlay(int iconRightOffset, BrowserWindow* targetWin = nullptr);
    extern void HideWalletOverlay();
    extern void CreateBackupOverlayWithSeparateProcess(HINSTANCE hInstance);
#else
    // macOS global views
    extern NSView* g_webview_view;
    // macOS overlay helpers (defined in cef_browser_shell_mac.mm)
    extern "C" void CloseOverlayWindow(void* window, void* parent);
    extern "C" void HideNotificationOverlayWindow();
    extern "C" void SetOverlayIgnoresMouseEvents(void* window, bool ignores);
#endif

// Forward declaration for cross-platform tab creation helper (defined later in file)
static void CreateNewTabWithUrl(const std::string& url);

static bool IsInternalFrontendUrl(const std::string& url) {
    return url.rfind("http://127.0.0.1:5137", 0) == 0 ||
           url.rfind("http://localhost:5137", 0) == 0 ||
           url.rfind("hodos://", 0) == 0;
}

static Tab* GetZoomTargetTab() {
    Tab* active_tab = TabManager::GetInstance().GetActiveTab();
    if (!active_tab || !active_tab->browser) {
        return nullptr;
    }

    std::string url = active_tab->url;
    if (url.empty() && active_tab->browser->GetMainFrame()) {
        url = active_tab->browser->GetMainFrame()->GetURL().ToString();
    }

    if (IsInternalFrontendUrl(url)) {
        LOG_WARNING_BROWSER("Ignoring native zoom for internal frontend page: " + url);
        return nullptr;
    }

    return active_tab;
}

static int GetCurrentMenuZoomPercent() {
    Tab* zoom_tab = GetZoomTargetTab();
    if (!zoom_tab || !zoom_tab->browser) {
        return 100;
    }

    double zoomLevel = zoom_tab->browser->GetHost()->GetZoomLevel();
    return static_cast<int>(std::round(100.0 * std::pow(1.2, zoomLevel)));
}

static void SendCurrentZoomToMenuOverlay(CefRefPtr<CefBrowser> menu_browser) {
    if (!menu_browser || !menu_browser->GetMainFrame()) {
        return;
    }

    int zoomPercent = GetCurrentMenuZoomPercent();
    std::string js =
        "if (window.setMenuZoomLevel) { window.setMenuZoomLevel(" + std::to_string(zoomPercent) + "); }";
    menu_browser->GetMainFrame()->ExecuteJavaScript(js, menu_browser->GetMainFrame()->GetURL(), 0);
}

// Global backup modal state management
static bool g_backupModalShown = false;

// Helper functions for backup modal state
bool getBackupModalShown() {
    return g_backupModalShown;
}

void setBackupModalShown(bool shown) {
    g_backupModalShown = shown;
    LOG_DEBUG_BROWSER("💾 Backup modal state set to: " + std::to_string(shown));
}

// ===== Ghost Tab Window for Tear-off Preview =====
// A lightweight top-level popup that follows the cursor during tab tear-off.
// Not a CEF browser — just a GDI-painted HWND with title text and transparency.
#ifdef _WIN32
static HWND s_ghost_hwnd = nullptr;
static UINT_PTR s_ghost_timer_id = 0;
static std::wstring s_ghost_title;
static int s_ghost_width = 200;
static int s_ghost_height = 36;
static bool s_ghost_class_registered = false;

static LRESULT CALLBACK GhostTabWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
    case WM_PAINT: {
        PAINTSTRUCT ps;
        HDC hdc = BeginPaint(hwnd, &ps);
        RECT r;
        GetClientRect(hwnd, &r);

        // White background
        HBRUSH bg = CreateSolidBrush(RGB(255, 255, 255));
        FillRect(hdc, &r, bg);
        DeleteObject(bg);

        // Grey rounded border
        HPEN pen = CreatePen(PS_SOLID, 1, RGB(180, 180, 180));
        SelectObject(hdc, pen);
        SelectObject(hdc, GetStockObject(NULL_BRUSH));
        RoundRect(hdc, 0, 0, r.right, r.bottom, 14, 14);
        DeleteObject(pen);

        // Title text
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, RGB(60, 60, 60));
        HFONT font = CreateFontW(-12, 0, 0, 0, FW_MEDIUM, FALSE, FALSE, FALSE,
            DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY, DEFAULT_PITCH | FF_SWISS, L"Segoe UI");
        HFONT oldFont = (HFONT)SelectObject(hdc, font);

        RECT textRect = { 12, 0, r.right - 12, r.bottom };
        DrawTextW(hdc, s_ghost_title.c_str(), -1, &textRect,
                  DT_SINGLELINE | DT_VCENTER | DT_LEFT | DT_END_ELLIPSIS);

        SelectObject(hdc, oldFont);
        DeleteObject(font);
        EndPaint(hwnd, &ps);
        return 0;
    }
    case WM_TIMER:
        if (wParam == 1) {
            POINT pt;
            GetCursorPos(&pt);
            SetWindowPos(hwnd, HWND_TOPMOST,
                         pt.x - s_ghost_width / 2, pt.y - 10,
                         0, 0, SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
        }
        return 0;
    case WM_DESTROY:
        if (s_ghost_timer_id) {
            KillTimer(hwnd, s_ghost_timer_id);
            s_ghost_timer_id = 0;
        }
        return 0;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

static void EnsureGhostWindowClass() {
    if (s_ghost_class_registered) return;
    WNDCLASSEXW wc = {};
    wc.cbSize = sizeof(WNDCLASSEXW);
    wc.lpfnWndProc = GhostTabWndProc;
    wc.hInstance = GetModuleHandle(nullptr);
    wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
    wc.lpszClassName = L"HodosGhostTab";
    RegisterClassExW(&wc);
    s_ghost_class_registered = true;
}

static void HideGhostTab() {
    if (s_ghost_hwnd) {
        if (s_ghost_timer_id) {
            KillTimer(s_ghost_hwnd, s_ghost_timer_id);
            s_ghost_timer_id = 0;
        }
        DestroyWindow(s_ghost_hwnd);
        s_ghost_hwnd = nullptr;
        LOG_DEBUG_BROWSER("Ghost tab hidden");
    }
}

static void ShowGhostTab(const std::string& title, int width, int height) {
    HideGhostTab();
    EnsureGhostWindowClass();

    // Convert UTF-8 title to wide string
    int len = MultiByteToWideChar(CP_UTF8, 0, title.c_str(), -1, nullptr, 0);
    s_ghost_title.resize(len);
    MultiByteToWideChar(CP_UTF8, 0, title.c_str(), -1, &s_ghost_title[0], len);

    s_ghost_width = (width > 60) ? width : 200;
    s_ghost_height = (height > 10) ? height : 36;

    POINT pt;
    GetCursorPos(&pt);

    s_ghost_hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
        L"HodosGhostTab", nullptr, WS_POPUP,
        pt.x - s_ghost_width / 2, pt.y - 10,
        s_ghost_width, s_ghost_height,
        nullptr, nullptr, GetModuleHandle(nullptr), nullptr);

    if (s_ghost_hwnd) {
        SetLayeredWindowAttributes(s_ghost_hwnd, 0, 217, LWA_ALPHA);  // ~85% opaque
        ShowWindow(s_ghost_hwnd, SW_SHOWNOACTIVATE);
        s_ghost_timer_id = SetTimer(s_ghost_hwnd, 1, 16, nullptr);  // ~60fps cursor tracking
        LOG_DEBUG_BROWSER("Ghost tab shown: \"" + title + "\" (" +
                          std::to_string(s_ghost_width) + "x" + std::to_string(s_ghost_height) + ")");
    }
}
#else
// macOS ghost tab — implemented in WindowManager_mac.mm
extern "C" void ShowGhostTabMacOS(const char* title, int width, int height);
extern "C" void HideGhostTabMacOS();
extern "C" void* GetWindowAtScreenPointMacOS(int screenX, int screenY);
extern "C" void PositionWindowAtScreenPoint(void* ns_window_ptr, int screenX, int screenY);
static void HideGhostTab() { HideGhostTabMacOS(); }
static void ShowGhostTab(const std::string& title, int width, int height) {
    ShowGhostTabMacOS(title.c_str(), width, height);
}
#endif
// ===== End Ghost Tab Window =====

std::string SimpleHandler::pending_panel_;
bool SimpleHandler::needs_overlay_reload_ = false;

SimpleHandler::SimpleHandler(const std::string& role, int window_id)
    : role_(role), window_id_(window_id)
#ifdef _WIN32
    , is_windowed_browser_(role.empty() || role == "header" ||
                           role.compare(0, 4, "tab_") == 0)
#endif
{}

BrowserWindow* SimpleHandler::GetOwnerWindow() const {
    return WindowManager::GetInstance().GetWindow(window_id_);
}

SimpleHandler* SimpleHandler::GetHandlerForBrowser(int browser_id) {
    auto it = browser_handler_map_.find(browser_id);
    return (it != browser_handler_map_.end()) ? it->second : nullptr;
}

// Static helper to extract tab ID from role string (format: "tab_1", "tab_2", etc.)
int SimpleHandler::ExtractTabIdFromRole(const std::string& role) {
    if (role.rfind("tab_", 0) == 0) {
        // Role is "tab_X" - extract X
        std::string id_str = role.substr(4);  // Skip "tab_"
        try {
            return std::stoi(id_str);
        } catch (...) {
            return -1;
        }
    }
    return -1;
}

CefRefPtr<CefLifeSpanHandler> SimpleHandler::GetLifeSpanHandler() {
    return this;
}

CefRefPtr<CefDisplayHandler> SimpleHandler::GetDisplayHandler() {
    return this;
}

CefRefPtr<CefLoadHandler> SimpleHandler::GetLoadHandler() {
    return this;
}

CefRefPtr<CefBrowser> SimpleHandler::webview_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::header_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::wallet_panel_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::overlay_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::settings_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::wallet_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::backup_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::brc100_auth_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::notification_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::settings_menu_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::omnibox_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::cookie_panel_browser_ = nullptr;

// Static getters — redirect to WindowManager window 0 for backwards compatibility.
// Cross-browser IPC within a handler should use GetOwnerWindow() instead.
CefRefPtr<CefBrowser> SimpleHandler::GetOverlayBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->overlay_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetHeaderBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->header_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetWebviewBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->webview_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetWalletPanelBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->wallet_panel_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetSettingsBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->settings_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetWalletBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->wallet_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetBackupBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->backup_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetBRC100AuthBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->brc100_auth_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetNotificationBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->notification_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetSettingsMenuBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->settings_menu_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetOmniboxBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->omnibox_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetCookiePanelBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->cookie_panel_browser : nullptr;
}

// Download handler static storage
std::map<uint32_t, SimpleHandler::DownloadInfo> SimpleHandler::active_downloads_;
std::set<uint32_t> SimpleHandler::paused_downloads_;
bool SimpleHandler::download_notify_pending_ = false;
std::map<int, SimpleHandler*> SimpleHandler::browser_handler_map_;
CefRefPtr<CefBrowser> SimpleHandler::download_panel_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::profile_panel_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::menu_browser_ = nullptr;
std::string SimpleHandler::pending_shield_domain_;

CefRefPtr<CefDownloadHandler> SimpleHandler::GetDownloadHandler() {
    return this;
}

CefRefPtr<CefFindHandler> SimpleHandler::GetFindHandler() {
    LOG_DEBUG_BROWSER("🔍 GetFindHandler() called for role: " + role_);
    return this;
}

CefRefPtr<CefJSDialogHandler> SimpleHandler::GetJSDialogHandler() {
    return this;
}

CefRefPtr<CefBrowser> SimpleHandler::GetDownloadPanelBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->download_panel_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetProfilePanelBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->profile_panel_browser : nullptr;
}
CefRefPtr<CefBrowser> SimpleHandler::GetMenuBrowser() {
    auto* win = WindowManager::GetInstance().GetWindow(0);
    return win ? win->menu_browser : nullptr;
}

void SimpleHandler::TriggerDeferredPanel(const std::string& panel) {
    CefRefPtr<CefBrowser> overlay = SimpleHandler::GetOverlayBrowser();
    if (overlay && overlay->GetMainFrame()) {
        std::string js = "window.triggerPanel('" + panel + "')";
        overlay->GetMainFrame()->ExecuteJavaScript(js, overlay->GetMainFrame()->GetURL(), 0);
        LOG_DEBUG_BROWSER("🧠 Deferred panel triggered after delay: " + panel);
    } else {
        LOG_DEBUG_BROWSER("⚠️ Overlay browser still not ready. Skipping panel trigger.");
    }
}

// Static method to notify frontend of tab list changes (called from TabManager)
// Sends per-window filtered tab lists to each window's header browser.
// Send tab list to a single window's header browser
static void SendTabListToWindow(BrowserWindow* bw) {
    if (!bw || !bw->header_browser) return;

    std::vector<Tab*> allTabs = TabManager::GetInstance().GetAllTabs();
    int activeForWindow = TabManager::GetInstance().GetActiveTabIdForWindow(bw->window_id);

    nlohmann::json response;
    response["activeTabId"] = activeForWindow;
    response["tabs"] = nlohmann::json::array();

    for (Tab* tab : allTabs) {
        if (tab->window_id != bw->window_id) continue;

        nlohmann::json tab_json;
        tab_json["id"] = tab->id;
        tab_json["title"] = tab->title;
        tab_json["url"] = tab->url;
        tab_json["isActive"] = (tab->id == activeForWindow);
        tab_json["isLoading"] = tab->is_loading;
        tab_json["hasCertError"] = tab->has_cert_error;
        if (!tab->favicon_url.empty()) {
            tab_json["favicon"] = tab->favicon_url;
        }
        response["tabs"].push_back(tab_json);
    }

    std::string json_str = response.dump();

    CefRefPtr<CefProcessMessage> cef_response = CefProcessMessage::Create("tab_list_response");
    CefRefPtr<CefListValue> response_args = cef_response->GetArgumentList();
    response_args->SetString(0, json_str);
    bw->header_browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cef_response);
    LOG_DEBUG_BROWSER("📑 Tab list sent to window " + std::to_string(bw->window_id) + ": " + json_str);
}

void SimpleHandler::NotifyTabListChanged() {
    CEF_REQUIRE_UI_THREAD();

    std::vector<BrowserWindow*> windows = WindowManager::GetInstance().GetAllWindows();
    for (BrowserWindow* bw : windows) {
        SendTabListToWindow(bw);
    }
}

void SimpleHandler::NotifyWindowTabListChanged(int window_id) {
    CEF_REQUIRE_UI_THREAD();

    BrowserWindow* bw = WindowManager::GetInstance().GetWindow(window_id);
    SendTabListToWindow(bw);
}

void SimpleHandler::OnTitleChange(CefRefPtr<CefBrowser> browser, const CefString& title) {
    CEF_REQUIRE_UI_THREAD();

    // Check if this is a tab browser and update TabManager (both platforms)
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        TabManager::GetInstance().UpdateTabTitle(tab_id, title.ToString());
        // F13 perf fix: push title change immediately instead of waiting for 5s poll
        NotifyWindowTabListChanged(window_id_);
    }

#if defined(OS_WIN)
    SetWindowText(browser->GetHost()->GetWindowHandle(), std::wstring(title).c_str());
#endif
}

bool SimpleHandler::OnCursorChange(CefRefPtr<CefBrowser> browser,
                                    CefCursorHandle cursor,
                                    cef_cursor_type_t type,
                                    const CefCursorInfo& custom_cursor_info) {
#ifdef _WIN32
    // For OSR overlay browsers, apply the cursor directly since they have no
    // CEF-owned child window to handle WM_SETCURSOR.
    // For windowed browsers (tabs, header), return false so CEF updates the
    // window class cursor via SetClassLongPtr(GCLP_HCURSOR) — without this,
    // WM_SETCURSOR resets the cursor to IDC_ARROW on every mouse move.
    if (!is_windowed_browser_) {
        ::SetCursor(cursor);
        return true;
    }
#endif
    return false;  // Let CEF handle for windowed browsers (tabs, header)
}

void SimpleHandler::OnAddressChange(CefRefPtr<CefBrowser> browser,
                                   CefRefPtr<CefFrame> frame,
                                   const CefString& url) {
    CEF_REQUIRE_UI_THREAD();

    // Only track main frame address changes
    if (!frame->IsMain()) {
        return;
    }

    // Check if this is a tab browser and update TabManager (both platforms)
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        std::string url_str = url.ToString();
        TabManager::GetInstance().UpdateTabURL(tab_id, url_str);
        LOG_DEBUG_BROWSER("🔗 Tab " + std::to_string(tab_id) + " URL updated to: " + url_str);

        // Notify ephemeral cookie manager of navigation (for third-party cookie lifecycle)
        Tab* tab = TabManager::GetInstance().GetTab(tab_id);
        if (tab && tab->browser) {
            EphemeralCookieManager::GetInstance().OnTabNavigated(
                tab->browser->GetIdentifier(), url_str);
        }

        // Clear cert error flag when navigating away from cert-error page to a real site
        if (url_str.find("/cert-error") == std::string::npos) {
            Tab* tab = TabManager::GetInstance().GetTab(tab_id);
            if (tab && tab->has_cert_error) {
                // Keep the flag if this domain is in allowed exceptions (user proceeded)
                std::string domain;
                size_t scheme_end = url_str.find("://");
                if (scheme_end != std::string::npos) {
                    size_t host_start = scheme_end + 3;
                    size_t host_end = url_str.find('/', host_start);
                    if (host_end == std::string::npos) host_end = url_str.length();
                    std::string host_port = url_str.substr(host_start, host_end - host_start);
                    size_t colon = host_port.find(':');
                    domain = (colon != std::string::npos) ? host_port.substr(0, colon) : host_port;
                }
                if (allowed_cert_exceptions_.count(domain) == 0) {
                    tab->has_cert_error = false;
                    NotifyTabListChanged();
                }
            }
        }
    }
}

void SimpleHandler::OnFaviconURLChange(CefRefPtr<CefBrowser> browser,
                                      const std::vector<CefString>& icon_urls) {
    CEF_REQUIRE_UI_THREAD();

    // Only process if we have favicon URLs
    if (icon_urls.empty()) {
        return;
    }

    // Check if this is a tab browser and update TabManager (both platforms)
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        // Use the first favicon URL (usually the most appropriate)
        std::string favicon_url = icon_urls[0].ToString();
        TabManager::GetInstance().UpdateTabFavicon(tab_id, favicon_url);
        LOG_DEBUG_BROWSER("🖼️ Tab " + std::to_string(tab_id) + " favicon updated: " + favicon_url);
    }
}

// Forward declaration for fullscreen handler in cef_browser_shell.cpp
extern void HandleFullscreenChange(bool fullscreen);

void SimpleHandler::OnFullscreenModeChange(CefRefPtr<CefBrowser> browser,
                                           bool fullscreen) {
    CEF_REQUIRE_UI_THREAD();
    LOG_DEBUG_BROWSER(std::string("🖥️ Fullscreen mode change: ") + (fullscreen ? "ENTER" : "EXIT") + " (role: " + role_ + ")");
    HandleFullscreenChange(fullscreen);
}

void SimpleHandler::OnLoadError(CefRefPtr<CefBrowser> browser,
                                CefRefPtr<CefFrame> frame,
                                ErrorCode errorCode,
                                const CefString& errorText,
                                const CefString& failedUrl) {
    // Don't display an error for aborted requests (e.g., navigated away, cert error handled)
    if (errorCode == ERR_ABORTED)
        return;

    std::string failed_url_str = failedUrl.ToString();

    // Don't process data: URLs to prevent infinite error loops
    if (failed_url_str.find("data:") == 0)
        return;

    LOG_DEBUG_BROWSER("❌ Load error for role: " + role_);
    LOG_DEBUG_BROWSER("❌ Load error: " + failed_url_str + " - " + errorText.ToString());
    LOG_DEBUG_BROWSER("❌ Error code: " + std::to_string(errorCode));

    if (frame->IsMain()) {
        std::string html = "<html><body><h1>Failed to load</h1><p>URL: " +
                           failed_url_str + "</p><p>Error: " +
                           errorText.ToString() + "</p></body></html>";

        std::string encoded_html;
        for (char c : html) {
            if (isalnum(static_cast<unsigned char>(c)) || c == ' ' || c == '.' || c == '-' || c == '_' || c == ':')
                encoded_html += c;
            else {
                char buf[4];
                snprintf(buf, sizeof(buf), "%%%02X", static_cast<unsigned char>(c));
                encoded_html += buf;
            }
        }

        std::string data_url = "data:text/html," + encoded_html;
        frame->LoadURL(data_url);
    }
}

// Static storage for cert exceptions (session-only, shared across all handlers)
std::set<std::string> SimpleHandler::allowed_cert_exceptions_;

// URL-encode helper for cert error page query parameters
static std::string certUrlEncode(const std::string& value) {
    std::string encoded;
    for (unsigned char c : value) {
        if (isalnum(c) || c == '-' || c == '_' || c == '.' || c == '~') {
            encoded += c;
        } else {
            char buf[4];
            snprintf(buf, sizeof(buf), "%%%02X", c);
            encoded += buf;
        }
    }
    return encoded;
}

bool SimpleHandler::OnCertificateError(CefRefPtr<CefBrowser> browser,
                                       cef_errorcode_t cert_error,
                                       const CefString& request_url,
                                       CefRefPtr<CefSSLInfo> ssl_info,
                                       CefRefPtr<CefCallback> callback) {
    CEF_REQUIRE_UI_THREAD();

    std::string url = request_url.ToString();
    LOG_WARNING_BROWSER("🔒 Certificate error on: " + url + " (code: " + std::to_string(cert_error) + ")");

    // Extract domain from URL
    std::string domain;
    size_t scheme_end = url.find("://");
    if (scheme_end != std::string::npos) {
        size_t host_start = scheme_end + 3;
        size_t host_end = url.find('/', host_start);
        if (host_end == std::string::npos) host_end = url.length();
        // Strip port if present
        std::string host_port = url.substr(host_start, host_end - host_start);
        size_t colon = host_port.find(':');
        domain = (colon != std::string::npos) ? host_port.substr(0, colon) : host_port;
    }

    // Check if user already allowed this domain
    if (allowed_cert_exceptions_.count(domain) > 0) {
        LOG_INFO_BROWSER("🔒 Cert exception exists for " + domain + " - proceeding");
        callback->Continue();
        return true;
    }

    // Set cert error flag on the tab
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        Tab* tab = TabManager::GetInstance().GetTab(tab_id);
        if (tab) {
            tab->has_cert_error = true;
            NotifyTabListChanged();
        }
    }

    // Map error code to human-readable type
    std::string error_type;
    switch (cert_error) {
        case ERR_CERT_COMMON_NAME_INVALID: error_type = "name_mismatch"; break;
        case ERR_CERT_DATE_INVALID:        error_type = "date_invalid"; break;
        case ERR_CERT_AUTHORITY_INVALID:    error_type = "authority_invalid"; break;
        case ERR_CERT_REVOKED:             error_type = "revoked"; break;
        case ERR_CERT_INVALID:             error_type = "invalid"; break;
        default:                           error_type = "unknown"; break;
    }

    // URL-encode the original URL for the query parameter
    std::string encoded_url = certUrlEncode(url);

    // Navigate to our cert error page (loads in the tab)
    std::string cert_error_url = "http://127.0.0.1:5137/cert-error?domain=" +
        certUrlEncode(domain) +
        "&error=" + error_type +
        "&url=" + encoded_url +
        "&code=" + std::to_string(cert_error);

    browser->GetMainFrame()->LoadURL(cert_error_url);
    LOG_INFO_BROWSER("🔒 Navigating to cert error page for domain: " + domain);

    // Return true - we handled the error (don't show default CEF error page)
    // The callback is intentionally not called - the original request is cancelled
    return true;
}

void SimpleHandler::OnLoadingStateChange(CefRefPtr<CefBrowser> browser,
                                         bool isLoading,
                                         bool canGoBack,
                                         bool canGoForward) {
    CEF_REQUIRE_UI_THREAD();

    // Check if this is a tab browser and update TabManager (both platforms)
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        TabManager::GetInstance().UpdateTabLoadingState(tab_id, isLoading, canGoBack, canGoForward);
    }

    LOG_DEBUG_BROWSER("📡 Loading state for role " + role_ + ": " + (isLoading ? "loading..." : "done"));

    // Reset adblock blocked count and cosmetic dedup when starting a new page load (tabs only)
#ifdef _WIN32
    if (isLoading && tab_id != -1) {
        AdblockCache::GetInstance().resetBlockedCount(browser->GetIdentifier());
        last_cosmetic_url_.clear();
        // NOTE: Scriptlet pre-caching is handled in OnBeforeBrowse (fires earlier).
        // Duplicate fetch here was removed — F10 perf fix.
    }
#endif

    // Track history when page finishes loading (for tabs - both platforms)
    if (!isLoading && tab_id != -1) {
        CefRefPtr<CefFrame> frame = browser->GetMainFrame();
        if (frame && frame->IsValid()) {
            std::string url = frame->GetURL().ToString();

            // Get title from TabManager which tracks it properly
            std::string title;
            auto& tab_mgr = TabManager::GetInstance();
            auto tab = tab_mgr.GetTab(tab_id);
            if (tab) {
                title = tab->title;
            }

            // Don't track internal URLs
            if (url.find("http://127.0.0.1:5137") != 0 &&
                url.find("devtools://") != 0 &&
                url.find("chrome://") != 0 &&
                url.find("about:") != 0 &&
                !url.empty()) {

                LOG_INFO_BROWSER("📚 Recording history: " + url + " [" + title + "]");
                HistoryManager::GetInstance().AddVisit(url, title, 0);
            }
        }
    }

    // Special debug for BRC-100 auth overlay
    if (role_ == "brc100auth") {
            LOG_DEBUG_BROWSER("🔐 BRC-100 AUTH Loading state: " + std::string(isLoading ? "loading..." : "done"));
            LOG_DEBUG_BROWSER("🔐 BRC-100 AUTH Browser ID: " + std::to_string(browser->GetIdentifier()));
            LOG_DEBUG_BROWSER("🔐 BRC-100 AUTH URL: " + browser->GetMainFrame()->GetURL().ToString());
            LOG_DEBUG_BROWSER("🔐 BRC-100 AUTH Can go back: " + std::string(canGoBack ? "true" : "false"));
            LOG_DEBUG_BROWSER("🔐 BRC-100 AUTH Can go forward: " + std::string(canGoForward ? "true" : "false"));
        }

    if (role_ == "overlay") {
        LOG_DEBUG_BROWSER("📡 Overlay URL: " + browser->GetMainFrame()->GetURL().ToString());
    }

    if (role_ == "backup") {
        LOG_DEBUG_BROWSER("📡 Backup URL: " + browser->GetMainFrame()->GetURL().ToString());
    }

    // Deferred shield domain injection: when cookie panel finishes loading,
    // inject any pending domain. This fires AFTER React has mounted and registered
    // the setShieldDomain callback, fixing the first-open race condition.
    if (!isLoading && role_ == "cookiepanel" && !pending_shield_domain_.empty()) {
        CefRefPtr<CefFrame> frame = browser->GetMainFrame();
        if (frame && frame->IsValid()) {
            std::string domain = pending_shield_domain_;
            pending_shield_domain_.clear();
            // Escape domain for safe JS injection
            std::string escapedDomain = domain;
            for (size_t i = 0; i < escapedDomain.size(); ++i) {
                if (escapedDomain[i] == '\\' || escapedDomain[i] == '\'') {
                    escapedDomain.insert(i, "\\");
                    ++i;
                }
            }
            std::string js = "if (window.setShieldDomain) { window.setShieldDomain('" + escapedDomain + "'); }";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            LOG_INFO_BROWSER("Deferred shield domain injected after page load: " + domain);
        }
    }

    // API injection logic (cross-platform)
    if (!isLoading) {
        if (role_ == "overlay") {
            // Log that we're about to inject the API
            LOG_DEBUG_BROWSER("🔧 OVERLAY LOADED - About to inject hodosBrowser API");

            // Inject the hodosBrowser API when overlay finishes loading
            extern void InjectHodosBrowserAPI(CefRefPtr<CefBrowser> browser);
            InjectHodosBrowserAPI(browser);
        } else if (role_ == "webview") {
            // Inject the hodosBrowser API into webview browser as well
            LOG_DEBUG_BROWSER("🔧 WEBVIEW BROWSER LOADED - Injecting hodosBrowser API");

            extern void InjectHodosBrowserAPI(CefRefPtr<CefBrowser> browser);
            InjectHodosBrowserAPI(browser);
        } else if (role_ == "header") {
            // Show main window shortly after header load (smooth startup).
            // Delay 150ms to give React time to mount and paint the toolbar,
            // so the window appears with the header fully rendered.
#ifdef _WIN32
            {
                extern bool g_window_shown;
                if (!g_window_shown) {
                    CefPostDelayedTask(TID_UI, base::BindOnce([]() {
                        extern HWND g_hwnd;
                        extern bool g_window_shown;
                        if (!g_window_shown && g_hwnd && IsWindow(g_hwnd)) {
                            ShowWindow(g_hwnd, SW_SHOW);
                            UpdateWindow(g_hwnd);
                            g_window_shown = true;
                            Logger::Log("Main window shown - header browser rendered", 1, 2);
                        }
                    }), 150);
                }
            }
#endif
            // Inject the hodosBrowser API into header browser (where React app runs)
            LOG_DEBUG_BROWSER("🔧 HEADER BROWSER LOADED - Injecting hodosBrowser API");

            extern void InjectHodosBrowserAPI(CefRefPtr<CefBrowser> browser);
            InjectHodosBrowserAPI(browser);

            // Pre-create notification overlay browser (hidden) so first notification is instant
#ifdef _WIN32
            CefPostDelayedTask(TID_UI, base::BindOnce([]() {
                extern void CreateNotificationOverlay(HINSTANCE hInstance, const std::string& type, const std::string& domain, const std::string& extraParams);
                extern HINSTANCE g_hInstance;
                extern HWND g_notification_overlay_hwnd;
                if (!g_notification_overlay_hwnd || !IsWindow(g_notification_overlay_hwnd)) {
                    CreateNotificationOverlay(g_hInstance, "preload", "", "");
                }
            }), 2000);
#endif
        } else if (role_ == "settings") {
            // Inject the hodosBrowser API into settings browser
            LOG_DEBUG_BROWSER("🔧 SETTINGS BROWSER LOADED - Injecting hodosBrowser API");

            extern void InjectHodosBrowserAPI(CefRefPtr<CefBrowser> browser);
            InjectHodosBrowserAPI(browser);
        } else if (role_ == "brc100auth") {
            // Inject the hodosBrowser API into BRC-100 auth browser
            LOG_DEBUG_BROWSER("🔧 BRC-100 AUTH BROWSER LOADED - Injecting hodosBrowser API");

            extern void InjectHodosBrowserAPI(CefRefPtr<CefBrowser> browser);
            InjectHodosBrowserAPI(browser);

            // Send pending auth request data to the overlay after React app loads
            // Add a small delay to ensure React is fully mounted
            CefPostDelayedTask(TID_UI, base::BindOnce([]() {
                extern void sendAuthRequestDataToOverlay();
                sendAuthRequestDataToOverlay();
            }), 500);
        } else if (role_ == "notification") {
            // Inject the hodosBrowser API into notification overlay browser
            LOG_DEBUG_BROWSER("🔔 NOTIFICATION BROWSER LOADED - Injecting hodosBrowser API");

            extern void InjectHodosBrowserAPI(CefRefPtr<CefBrowser> browser);
            InjectHodosBrowserAPI(browser);
            // No delayed data send needed — notification data comes from URL query params
        } else if (ExtractTabIdFromRole(role_) != -1) {
            // Inject the hodosBrowser API into tab browsers
            LOG_DEBUG_BROWSER("🔧 TAB BROWSER LOADED - Injecting hodosBrowser API for tab " + role_);

            extern void InjectHodosBrowserAPI(CefRefPtr<CefBrowser> browser);
            InjectHodosBrowserAPI(browser);

            // Cosmetic filtering: fetch CSS selectors + scriptlets and inject into tab (Sprint 8e)
#ifdef _WIN32
            {
                CefRefPtr<CefFrame> mainFrame = browser->GetMainFrame();
                if (mainFrame && mainFrame->IsValid()) {
                    std::string pageUrl = mainFrame->GetURL().ToString();

                    // Dedup: skip if we already injected for this exact URL
                    if (pageUrl == last_cosmetic_url_) {
                        // Already processed — skip redundant cosmetic injection
                    } else if (!shouldSkipAdblockCheck(pageUrl) && g_adblockServerRunning && AdblockCache::GetInstance().IsGlobalEnabled()) {
                        last_cosmetic_url_ = pageUrl;
                        // Fetch cosmetic resources from adblock engine
                        // Sprint 10b: Check if scriptlets are disabled for this domain
                        bool skipScriptlets = !AdblockCache::GetInstance().isScriptletsEnabled(pageUrl);
                        auto cosmetic = AdblockCache::GetInstance().fetchCosmeticResources(pageUrl, skipScriptlets);

                        LOG_DEBUG_BROWSER("🎨 Cosmetic P1: css=" + std::to_string(cosmetic.cssSelectors.size()) +
                            " script=" + std::to_string(cosmetic.injectedScript.size()) +
                            " generichide=" + std::to_string(cosmetic.generichide) + " url=" + pageUrl);

                        if (!cosmetic.cssSelectors.empty()) {
                            CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("inject_cosmetic_css");
                            CefRefPtr<CefListValue> args = msg->GetArgumentList();
                            args->SetString(0, cosmetic.cssSelectors);
                            mainFrame->SendProcessMessage(PID_RENDERER, msg);
                        }

                        if (!cosmetic.injectedScript.empty()) {
                            LOG_INFO_BROWSER("💉 Injecting scriptlets for " + pageUrl +
                                " (" + std::to_string(cosmetic.injectedScript.size()) + " chars)");

                            // Send scriptlets to renderer via IPC for injection
                            CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("inject_cosmetic_script");
                            CefRefPtr<CefListValue> args = msg->GetArgumentList();
                            args->SetString(0, cosmetic.injectedScript);
                            mainFrame->SendProcessMessage(PID_RENDERER, msg);
                        }

                        // Phase 2: Inject JS to collect DOM class names and IDs,
                        // then query engine for generic cosmetic selectors
                        if (!cosmetic.generichide) {
                            std::string collectJs = R"JS(
                                (function() {
                                    function collectAndSend() {
                                        var classes = new Set();
                                        var ids = new Set();
                                        var elems = document.querySelectorAll('[class],[id]');
                                        for (var i = 0; i < elems.length; i++) {
                                            var el = elems[i];
                                            if (el.id) ids.add(el.id);
                                            if (el.classList) {
                                                for (var j = 0; j < el.classList.length; j++) {
                                                    classes.add(el.classList[j]);
                                                }
                                            }
                                        }
                                        if (classes.size > 0 || ids.size > 0) {
                                            window.cefMessage.send('cosmetic_class_id_query',
                                                JSON.stringify({
                                                    url: window.location.href,
                                                    classes: Array.from(classes),
                                                    ids: Array.from(ids)
                                                })
                                            );
                                        }
                                    }
                                    if (document.readyState === 'loading') {
                                        document.addEventListener('DOMContentLoaded', collectAndSend);
                                    } else {
                                        collectAndSend();
                                    }
                                })();
                            )JS";
                            mainFrame->ExecuteJavaScript(collectJs, mainFrame->GetURL(), 0);
                        }
                    }
                } else {
                    LOG_INFO_BROWSER("🎨 Cosmetic skip: mainFrame null or invalid for tab " + role_);
                }
            }
#endif
        }

        // Overlay-specific logic
        if (role_ == "overlay") {
            // Check if we need to reload the overlay
            if (needs_overlay_reload_) {
                LOG_DEBUG_BROWSER("🔄 Overlay finished loading, now reloading React app");
                needs_overlay_reload_ = false;
                browser->GetMainFrame()->LoadURL("http://127.0.0.1:5137/overlay");
                LOG_DEBUG_BROWSER("🔄 LoadURL called for overlay reload");
                return; // Don't process pending panels yet, wait for reload to complete
            }

            // Handle pending panel triggers
            if (!pending_panel_.empty()) {
                std::string panel = pending_panel_;
                LOG_DEBUG_BROWSER("🕒 OnLoadingStateChange: Creating deferred trigger for panel: " + panel);

                // Clear pending_panel_ immediately to prevent duplicate deferred triggers
                SimpleHandler::pending_panel_.clear();

                // Delay JS execution slightly to ensure React is mounted
                // Use a simple function call instead of lambda to avoid CEF bind issues
                CefPostDelayedTask(TID_UI, base::BindOnce(&SimpleHandler::TriggerDeferredPanel, panel), 100);
            }
        }
    }
}

void SimpleHandler::OnAfterCreated(CefRefPtr<CefBrowser> browser) {
    CEF_REQUIRE_UI_THREAD();

    // Register handler in static map for overlay retargeting
    browser_handler_map_[browser->GetIdentifier()] = this;

    LOG_DEBUG_BROWSER("✅ OnAfterCreated for role: " + role_);

    // Check if this is a tab browser - register with TabManager (both platforms)
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        // This is a tab browser - register with TabManager
        TabManager::GetInstance().RegisterTabBrowser(tab_id, browser);
        LOG_DEBUG_BROWSER("📑 Tab browser registered: ID " + std::to_string(tab_id) +
                         ", Browser ID: " + std::to_string(browser->GetIdentifier()));

        // Delayed WasResized() + Invalidate() to fix first-render black screen
        // CEF needs time for view to be fully initialized before rendering
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
                LOG(INFO) << "Tab browser delayed resize/invalidate completed";
            }
        }, browser_ref), 150);  // 150ms delay for view initialization

        return;  // Tab browsers don't need the overlay/header/webview handling below
    }

    // Store browser ref in BrowserWindow via WindowManager
    BrowserWindow* owner_win = GetOwnerWindow();
    if (owner_win) {
        owner_win->SetBrowserForRole(role_, browser);
    }

    if (role_ == "webview") {
        LOG_DEBUG_BROWSER("📡 WebView browser reference stored.");
        LOG_DEBUG_BROWSER("📡 WebView browser reference stored. ID: " + std::to_string(browser->GetIdentifier()));

        // Trigger initial resize to ensure content renders on startup
        browser->GetHost()->WasResized();
        LOG_DEBUG_BROWSER("🔄 Initial WasResized() called for webview browser");
    } else if (role_ == "header") {
        LOG_DEBUG_BROWSER("🧭 header browser initialized.");
        LOG_DEBUG_BROWSER("🧭 header browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

#ifdef __APPLE__
        // macOS: Aggressively trigger paint for windowless rendering
        LOG_DEBUG_BROWSER("🎨 Forcing header browser paint on macOS");
        CefRefPtr<CefBrowser> browser_ref = browser;

        // Immediate resize and invalidate
        if (browser->GetHost()) {
            browser->GetHost()->WasResized();
            browser->GetHost()->Invalidate(PET_VIEW);
            LOG_DEBUG_BROWSER("🎨 Called WasResized() and Invalidate() for header");
        }

        // Delayed paint trigger (ensure view is ready)
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
                LOG_DEBUG_BROWSER("🎨 Delayed WasResized() and Invalidate() for header");
            }
        }, browser_ref), 100);
#else
        // Windows code stays as-is
        // Trigger initial resize to ensure content renders on startup
        browser->GetHost()->WasResized();
        LOG_DEBUG_BROWSER("🔄 Initial WasResized() called for header browser");

        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);

        // Auto-show profile picker on startup if configured
        CefPostDelayedTask(TID_UI, base::BindOnce([]() {
            auto& pm = ProfileManager::GetInstance();
            if (pm.ShouldShowPickerOnStartup() && pm.GetAllProfiles().size() >= 2) {
                extern void ShowProfilePanelOverlay(int, BrowserWindow* targetWin = nullptr);
                extern void CreateProfilePanelOverlay(HINSTANCE, bool, int);
                extern HWND g_profile_panel_overlay_hwnd;
                extern HINSTANCE g_hInstance;
                if (!g_profile_panel_overlay_hwnd || !IsWindow(g_profile_panel_overlay_hwnd)) {
                    CreateProfilePanelOverlay(g_hInstance, true, 0);
                } else {
                    ShowProfilePanelOverlay(0);
                }
            }
        }), 500);
#endif
    } else if (role_ == "wallet_panel") {
        LOG_DEBUG_BROWSER("💰 Wallet panel browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        // Trigger initial resize
        browser->GetHost()->WasResized();
        LOG_DEBUG_BROWSER("🔄 Initial WasResized() called for wallet panel browser");
    } else if (role_ == "overlay") {
        LOG_DEBUG_BROWSER("🪟 Overlay browser initialized.");
        LOG_DEBUG_BROWSER("🪟 Overlay browser initialized. ID: " + std::to_string(browser->GetIdentifier()));
    } else if (role_ == "settings") {
        LOG_DEBUG_BROWSER("⚙️ Settings browser initialized.");
        LOG_DEBUG_BROWSER("⚙️ Settings browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        // CRITICAL: Set focus so keyboard input works in React input fields
        browser->GetHost()->SetFocus(true);
        LOG_DEBUG_BROWSER("⌨️ Settings browser focus enabled");

        // Delayed resize/invalidate to fix first-render issue
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);

    } else if (role_ == "wallet") {
        LOG_DEBUG_BROWSER("💰 Wallet browser initialized.");
        LOG_DEBUG_BROWSER("💰 Wallet browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        // CRITICAL: Set focus so keyboard input works in React input fields
        browser->GetHost()->SetFocus(true);
        LOG_DEBUG_BROWSER("⌨️ Wallet browser focus enabled");

        // Delayed resize/invalidate to fix first-render issue
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);

    } else if (role_ == "backup") {
        LOG_DEBUG_BROWSER("💾 Backup browser initialized.");
        LOG_DEBUG_BROWSER("💾 Backup browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        // CRITICAL: Set focus so keyboard input works in React input fields
        browser->GetHost()->SetFocus(true);
        LOG_DEBUG_BROWSER("⌨️ Backup browser focus enabled");

        // Delayed resize/invalidate to fix first-render issue
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);

    } else if (role_ == "brc100auth") {
        LOG_DEBUG_BROWSER("🔐 BRC-100 Auth browser initialized.");
        LOG_DEBUG_BROWSER("🔐 BRC-100 Auth browser initialized. ID: " + std::to_string(browser->GetIdentifier()));
        LOG_DEBUG_BROWSER("🔐 BRC-100 Auth browser main frame URL: " + browser->GetMainFrame()->GetURL().ToString());

        // CRITICAL: Set focus so keyboard input works in React input fields
        browser->GetHost()->SetFocus(true);
        LOG_DEBUG_BROWSER("⌨️ BRC-100 Auth browser focus enabled");

        // Delayed resize/invalidate to fix first-render issue
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);

    } else if (role_ == "notification") {
        LOG_DEBUG_BROWSER("🔔 Notification browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        browser->GetHost()->SetFocus(true);

        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);

    } else if (role_ == "settings_menu") {
        LOG_DEBUG_BROWSER("📋 Settings menu browser initialized.");
        LOG_DEBUG_BROWSER("📋 Settings menu browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        // Delayed resize/invalidate to fix first-render issue
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);
    } else if (role_ == "omnibox") {
        LOG_DEBUG_BROWSER("🔍 Omnibox overlay browser initialized.");
        LOG_DEBUG_BROWSER("🔍 Omnibox overlay browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        // CRITICAL: Set focus so address bar input continues working
        browser->GetHost()->SetFocus(true);
        LOG_DEBUG_BROWSER("⌨️ Omnibox browser focus enabled");

        // Delayed resize/invalidate to fix first-render black screen issue
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);
    } else if (role_ == "cookiepanel") {
        LOG_DEBUG_BROWSER("🍪 Cookie panel overlay browser initialized.");
        LOG_DEBUG_BROWSER("🍪 Cookie panel overlay browser initialized. ID: " + std::to_string(browser->GetIdentifier()));
    } else if (role_ == "downloadpanel") {
        LOG_DEBUG_BROWSER("📥 Download panel overlay browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        // CRITICAL: Set focus so interactions work
        browser->GetHost()->SetFocus(true);
        LOG_DEBUG_BROWSER("⌨️ Download panel browser focus enabled");

        // Delayed resize/invalidate to fix first-render black screen issue
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);
    } else if (role_ == "profilepanel") {
        LOG_DEBUG_BROWSER("👤 Profile panel overlay browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        browser->GetHost()->SetFocus(true);
        LOG_DEBUG_BROWSER("⌨️ Profile panel browser focus enabled");

        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);
    } else if (role_ == "menu") {
        LOG_DEBUG_BROWSER("Menu overlay browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        SendCurrentZoomToMenuOverlay(browser);

#ifdef __APPLE__
        // Populate the OverlayBrowserRef so GenericOverlayView can forward events.
        // SetMenuOverlayBrowser is defined in cef_browser_shell_mac.mm.
        {
            extern void SetMenuOverlayBrowser(CefRefPtr<CefBrowser> browser);
            SetMenuOverlayBrowser(browser);
            LOG_DEBUG_BROWSER("Menu overlay OverlayBrowserRef populated via SetMenuOverlayBrowser");
        }
#endif

        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);
    }

    LOG_DEBUG_BROWSER("🧭 Browser Created → role: " + role_ + ", ID: " + std::to_string(browser->GetIdentifier()) + ", IsPopup: " + (browser->IsPopup() ? "true" : "false") + ", MainFrame URL: " + browser->GetMainFrame()->GetURL().ToString());
}

void SimpleHandler::OnBeforeClose(CefRefPtr<CefBrowser> browser) {
    CEF_REQUIRE_UI_THREAD();

    // Unregister from handler map
    browser_handler_map_.erase(browser->GetIdentifier());

    // Check shutdown IMMEDIATELY after erase, before any early returns.
    // Tab and popup branches return early, so this must come first.
#ifdef _WIN32
    {
        extern bool g_app_shutting_down;
        if (g_app_shutting_down && browser_handler_map_.empty()) {
            LOG_INFO_BROWSER("🛑 All browsers closed during shutdown — quitting CEF message loop");
            CefQuitMessageLoop();
            return;  // No further cleanup needed during shutdown
        }
    }
#endif

    std::cout << "🔴 OnBeforeClose ENTERED" << std::endl;
    std::cout << "  Role: " << role_ << std::endl;
    std::cout << "  Browser ID: " << browser->GetIdentifier() << std::endl;
    std::cout << "  IsPopup: " << (browser->IsPopup() ? "YES" : "NO") << std::endl;

    LOG_DEBUG_BROWSER("🔴 OnBeforeClose for role: " + role_ + ", Browser ID: " + std::to_string(browser->GetIdentifier()));

    // CRITICAL: Check if this is a popup (DevTools, etc.)
    if (browser->IsPopup()) {
        std::cout << "  → Detected as popup, skipping cleanup" << std::endl;
        LOG_DEBUG_BROWSER("🔧 Popup browser (DevTools or other) closing - ignoring");
        std::cout << "🔴 OnBeforeClose EXITING (popup)" << std::endl;
        return;
    }

    std::cout << "  → Not a popup, checking if tab browser..." << std::endl;

    // Check if this is a tab browser (both platforms)
    int tab_id = ExtractTabIdFromRole(role_);
    std::cout << "  → Extracted tab ID: " << tab_id << std::endl;

    if (tab_id != -1) {
        std::cout << "  → Is tab browser, calling OnTabBrowserClosed" << std::endl;
        TabManager::GetInstance().OnTabBrowserClosed(tab_id);
        LOG_DEBUG_BROWSER("📑 Tab browser closed callback: ID " + std::to_string(tab_id));
        std::cout << "🔴 OnBeforeClose EXITING (tab)" << std::endl;
        return;
    }

    std::cout << "  → Not a tab, checking overlays..." << std::endl;

    // Handle overlay browser cleanup via WindowManager
    BrowserWindow* owner_win = GetOwnerWindow();
    if (owner_win) {
        CefRefPtr<CefBrowser> existing = owner_win->GetBrowserForRole(role_);
        if (existing && existing->GetIdentifier() == browser->GetIdentifier()) {
            std::cout << "  → Clearing " << role_ << " browser from BrowserWindow" << std::endl;
            owner_win->ClearBrowserForRole(role_);
        } else {
            std::cout << "  → No matching browser for role (might be DevTools)" << std::endl;
        }
    } else {
        std::cout << "  → No owner window found for window_id " << window_id_ << std::endl;
    }

    std::cout << "🔴 OnBeforeClose EXITING (overlay)" << std::endl;
}

bool SimpleHandler::OnBeforePopup(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    int popup_id,
    const CefString& target_url,
    const CefString& target_frame_name,
    CefLifeSpanHandler::WindowOpenDisposition target_disposition,
    bool user_gesture,
    const CefPopupFeatures& popupFeatures,
    CefWindowInfo& windowInfo,
    CefRefPtr<CefClient>& client,
    CefBrowserSettings& settings,
    CefRefPtr<CefDictionaryValue>& extra_info,
    bool* no_javascript_access) {

    CEF_REQUIRE_UI_THREAD();

    std::string url = target_url.ToString();

    // Log disposition value and role for debugging
    std::string disposition_str;
    switch (target_disposition) {
        case CEF_WOD_UNKNOWN: disposition_str = "UNKNOWN"; break;
        case CEF_WOD_CURRENT_TAB: disposition_str = "CURRENT_TAB"; break;
        case CEF_WOD_SINGLETON_TAB: disposition_str = "SINGLETON_TAB"; break;
        case CEF_WOD_NEW_FOREGROUND_TAB: disposition_str = "NEW_FOREGROUND_TAB"; break;
        case CEF_WOD_NEW_BACKGROUND_TAB: disposition_str = "NEW_BACKGROUND_TAB"; break;
        case CEF_WOD_NEW_POPUP: disposition_str = "NEW_POPUP"; break;
        case CEF_WOD_NEW_WINDOW: disposition_str = "NEW_WINDOW"; break;
        case CEF_WOD_SAVE_TO_DISK: disposition_str = "SAVE_TO_DISK"; break;
        case CEF_WOD_OFF_THE_RECORD: disposition_str = "OFF_THE_RECORD"; break;
        case CEF_WOD_IGNORE_ACTION: disposition_str = "IGNORE_ACTION"; break;
        case CEF_WOD_SWITCH_TO_TAB: disposition_str = "SWITCH_TO_TAB"; break;
        case CEF_WOD_NEW_PICTURE_IN_PICTURE: disposition_str = "NEW_PICTURE_IN_PICTURE"; break;
        default: disposition_str = "UNKNOWN_VALUE(" + std::to_string(target_disposition) + ")"; break;
    }

    LOG_DEBUG_BROWSER("🔗 Popup requested: " + url + " (disposition: " + disposition_str + ", role: " + role_ + ")");

    // Allow DevTools and other special popups to open normally
    if (url.find("devtools://") == 0 || url.find("chrome://") == 0 || url.empty()) {
        LOG_DEBUG_BROWSER("🔧 Allowing special popup (DevTools/Chrome): " + url);
        return false;  // Allow default popup behavior
    }

    // Convert ALL popup requests to new tabs (including NEW_POPUP and NEW_WINDOW)
    // This ensures that right-click "Open in new tab", middle-click, Ctrl+Click,
    // and target="_blank" links all open in tabs instead of separate windows
    bool should_create_tab = (
        target_disposition == CEF_WOD_NEW_FOREGROUND_TAB ||
        target_disposition == CEF_WOD_NEW_BACKGROUND_TAB ||
        target_disposition == CEF_WOD_SINGLETON_TAB ||
        target_disposition == CEF_WOD_NEW_POPUP ||
        target_disposition == CEF_WOD_NEW_WINDOW
    );

    if (should_create_tab) {
#ifdef _WIN32
        // Create new tab for ANY browser (tab browser, webview, etc.)
        LOG_DEBUG_BROWSER("📑 Converting popup to new tab: " + url + " (disposition: " + disposition_str + ", role: " + role_ + ")");

        // Use the owning window for this browser
        BrowserWindow* popupWin = GetOwnerWindow();
        HWND popupHwnd = popupWin ? popupWin->hwnd : nullptr;
        int popupWid = popupWin ? popupWin->window_id : 0;
        if (!popupHwnd) {
            extern HWND g_hwnd;
            popupHwnd = g_hwnd;
        }
        RECT rect;
        GetClientRect(popupHwnd, &rect);
        int width = rect.right - rect.left;
        int height = rect.bottom - rect.top;
        int shellHeight = GetHeaderHeightPx(popupHwnd);
        int tabHeight = height - shellHeight;

        // Create new tab with the popup URL in the same window
        TabManager::GetInstance().CreateTab(url, popupHwnd, 0, shellHeight, width, tabHeight, popupWid);

        // Return true to cancel the popup window creation (we handled it with a new tab)
        return true;
#else
        // TODO(macOS): Implement tab creation for popup handling
        LOG_DEBUG_BROWSER("📑 Popup to tab conversion not implemented on macOS: " + url);
        return false;  // Allow default behavior for now
#endif
    }

    // For other dispositions (CURRENT_TAB, SAVE_TO_DISK, etc.), allow default behavior
    LOG_DEBUG_BROWSER("🔧 Allowing default behavior (disposition: " + disposition_str + ")");
    return false;
}

bool SimpleHandler::OnProcessMessageReceived(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefProcessId source_process,
    CefRefPtr<CefProcessMessage> message
) {
    CEF_REQUIRE_UI_THREAD();

    std::string message_name = message->GetName();
    LOG_DEBUG_BROWSER("📨 Message received: " + message_name + ", Browser ID: " + std::to_string(browser->GetIdentifier()));

    // ========== TAB MANAGEMENT MESSAGES ==========
    // Tab management available on both platforms now

    if (message_name == "tab_create") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string url = args->GetSize() > 0 ? args->GetString(0).ToString() : "";
        // Empty URL flows through to TabManager::CreateTab which defaults to NTP

#ifdef _WIN32
        // Determine which window this request came from
        BrowserWindow* ownerWin = GetOwnerWindow();
        HWND parentHwnd = ownerWin ? ownerWin->hwnd : nullptr;

        // Fallback to global g_hwnd if owner not found
        if (!parentHwnd) {
            extern HWND g_hwnd;
            parentHwnd = g_hwnd;
        }

        RECT rect;
        GetClientRect(parentHwnd, &rect);
        int width = rect.right - rect.left;
        int height = rect.bottom - rect.top;

        int shellHeight = GetHeaderHeightPx(parentHwnd);
        int tabHeight = height - shellHeight;

        int tab_id = TabManager::GetInstance().CreateTab(url, parentHwnd, 0, shellHeight, width, tabHeight, window_id_);
#else
        // macOS: Use the requesting window's webview, fallback to global
        BrowserWindow* ownerWin = GetOwnerWindow();
        void* parentView = (ownerWin && ownerWin->webview_view) ? ownerWin->webview_view : g_webview_view;

        ViewDimensions dims = GetViewDimensions(parentView);
        int tab_id = TabManager::GetInstance().CreateTab(
            url, parentView, 0, 0, dims.width, dims.height, window_id_);
#endif

        LOG_DEBUG_BROWSER("📑 Tab created: ID " + std::to_string(tab_id) + " in window " + std::to_string(window_id_));

        // Notify the owning window of tab list change
        NotifyWindowTabListChanged(window_id_);
        return true;
    }

    if (message_name == "tab_close") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() > 0) {
            int tab_id = args->GetInt(0);
            bool success = TabManager::GetInstance().CloseTab(tab_id);

            LOG_DEBUG_BROWSER("📑 Tab close: ID " + std::to_string(tab_id) +
                             (success ? " succeeded" : " failed"));

            if (success) {
                NotifyWindowTabListChanged(window_id_);
            }
        }
        return true;
    }

    if (message_name == "tab_switch") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() > 0) {
            int tab_id = args->GetInt(0);
            bool success = TabManager::GetInstance().SwitchToTab(tab_id);

            LOG_DEBUG_BROWSER("📑 Tab switch: ID " + std::to_string(tab_id) +
                             (success ? " succeeded" : " failed"));
        }
        return true;
    }

    if (message_name == "tab_reorder") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() > 0) {
            std::string json_str = args->GetString(0).ToString();
            try {
                nlohmann::json order_json = nlohmann::json::parse(json_str);
                std::vector<int> order;
                for (const auto& id : order_json) {
                    order.push_back(id.get<int>());
                }
                bool success = TabManager::GetInstance().ReorderTabs(order);
                LOG_DEBUG_BROWSER("📑 Tab reorder: " + std::string(success ? "succeeded" : "failed"));
            } catch (const std::exception& e) {
                LOG_WARNING_BROWSER("📑 Tab reorder: failed to parse JSON: " + std::string(e.what()));
            }
        }
        // Do NOT send tab list update — frontend already has correct order
        return true;
    }

    if (message_name == "tab_ghost_show") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() >= 3) {
            std::string title = args->GetString(0).ToString();
            int width = args->GetInt(1);
            int height = args->GetInt(2);
            ShowGhostTab(title, width, height);
        }
        return true;
    }

    if (message_name == "tab_ghost_hide") {
        HideGhostTab();
        return true;
    }

    if (message_name == "tab_tearoff") {
        HideGhostTab();  // Always hide ghost when tear-off fires
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() >= 3) {
            int tab_id = args->GetInt(0);
            int screen_x = args->GetInt(1);
            int screen_y = args->GetInt(2);

            Tab* tab = TabManager::GetInstance().GetTab(tab_id);
            if (!tab) {
                LOG_WARNING_BROWSER("tab_tearoff: tab " + std::to_string(tab_id) + " not found");
                return true;
            }

            int source_window_id = tab->window_id;

            // Don't tear off if it's the only tab in the window
            int tabs_in_window = 0;
            for (auto* t : TabManager::GetInstance().GetAllTabs()) {
                if (t->window_id == source_window_id) tabs_in_window++;
            }
            if (tabs_in_window <= 1) {
                LOG_DEBUG_BROWSER("tab_tearoff: can't tear off last tab in window " + std::to_string(source_window_id));
                return true;
            }

#ifdef _WIN32
            // Check if drop point is over another Hodos browser window
            POINT pt = { screen_x, screen_y };
            HWND target_hwnd = WindowFromPoint(pt);

            // Walk up parent chain to find the top-level shell window
            if (target_hwnd) {
                HWND top = GetAncestor(target_hwnd, GA_ROOT);
                if (top) target_hwnd = top;
            }

            BrowserWindow* target_bw = nullptr;
            if (target_hwnd) {
                target_bw = WindowManager::GetInstance().GetWindowByHwnd(target_hwnd);
            }

            // Don't merge into the same window we're tearing from
            if (target_bw && target_bw->window_id == source_window_id) {
                target_bw = nullptr;
            }

            if (target_bw) {
                // MERGE: Move tab into the existing target window
                LOG_INFO_BROWSER("tab_tearoff: merging tab " + std::to_string(tab_id) +
                                 " into window " + std::to_string(target_bw->window_id));
                TabManager::GetInstance().MoveTabToWindow(tab_id, target_bw->window_id);
            } else {
                // TEAR-OFF: Create new window at drop position, move tab into it
                LOG_INFO_BROWSER("tab_tearoff: tearing off tab " + std::to_string(tab_id) +
                                 " to new window at (" + std::to_string(screen_x) + "," + std::to_string(screen_y) + ")");
                BrowserWindow* new_bw = WindowManager::GetInstance().CreateFullWindow(false);
                if (new_bw) {
                    // Position the new window at the drop point (offset so title bar is near cursor)
                    SetWindowPos(new_bw->hwnd, nullptr, screen_x - 100, screen_y - 50,
                                 0, 0, SWP_NOSIZE | SWP_NOZORDER);
                    TabManager::GetInstance().MoveTabToWindow(tab_id, new_bw->window_id);
                }
            }
#elif defined(__APPLE__)
            // macOS merge detection — check if drop point is over another window
            BrowserWindow* target_bw = (BrowserWindow*)GetWindowAtScreenPointMacOS(screen_x, screen_y);

            // Don't merge into the same window we're tearing from
            if (target_bw && target_bw->window_id == source_window_id) {
                target_bw = nullptr;
            }

            if (target_bw) {
                LOG_INFO_BROWSER("tab_tearoff: merging tab " + std::to_string(tab_id) +
                                 " into window " + std::to_string(target_bw->window_id));
                TabManager::GetInstance().MoveTabToWindow(tab_id, target_bw->window_id);
            } else {
                LOG_INFO_BROWSER("tab_tearoff: tearing off tab " + std::to_string(tab_id) +
                                 " to new window at (" + std::to_string(screen_x) + "," + std::to_string(screen_y) + ")");
                BrowserWindow* new_bw = WindowManager::GetInstance().CreateFullWindow(false);
                if (new_bw) {
                    PositionWindowAtScreenPoint(new_bw->ns_window, screen_x, screen_y);
                    TabManager::GetInstance().MoveTabToWindow(tab_id, new_bw->window_id);
                }
            }
#endif
        }
        return true;
    }

    if (message_name == "get_tab_list") {
        // Send per-window filtered tab list back to the requesting browser
        BrowserWindow* win = GetOwnerWindow();
        int wid = win ? win->window_id : 0;

        std::vector<Tab*> allTabs = TabManager::GetInstance().GetAllTabs();
        int activeForWindow = TabManager::GetInstance().GetActiveTabIdForWindow(wid);

        nlohmann::json response;
        response["activeTabId"] = activeForWindow;
        response["tabs"] = nlohmann::json::array();

        for (Tab* tab : allTabs) {
            if (tab->window_id != wid) continue;
            nlohmann::json tab_json;
            tab_json["id"] = tab->id;
            tab_json["title"] = tab->title;
            tab_json["url"] = tab->url;
            tab_json["isActive"] = (tab->id == activeForWindow);
            tab_json["isLoading"] = tab->is_loading;
            tab_json["hasCertError"] = tab->has_cert_error;
            if (!tab->favicon_url.empty()) {
                tab_json["favicon"] = tab->favicon_url;
            }
            response["tabs"].push_back(tab_json);
        }

        std::string json_str = response.dump();
        CefRefPtr<CefProcessMessage> cef_response = CefProcessMessage::Create("tab_list_response");
        cef_response->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cef_response);
        LOG_DEBUG_BROWSER("📑 Tab list sent to window " + std::to_string(wid) + " (on-demand): " + json_str);
        return true;
    }

    // ========== NAVIGATION MESSAGES ==========
    // Navigation now uses TabManager on both platforms

    if (message_name == "navigate") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string path = args->GetString(0);

        // Dismiss omnibox overlay on navigation
#ifdef _WIN32
        extern void HideOmniboxOverlay();
        HideOmniboxOverlay();
#endif

        // Normalize protocol
        if (!(path.rfind("http://", 0) == 0 || path.rfind("https://", 0) == 0)) {
            path = "http://" + path;
        }

        // Use per-window active tab for navigation
        BrowserWindow* nav_win = GetOwnerWindow();
        int nav_wid = nav_win ? nav_win->window_id : 0;
        Tab* active_tab = TabManager::GetInstance().GetActiveTabForWindow(nav_wid);
        if (active_tab && active_tab->browser && active_tab->browser->GetMainFrame()) {
            active_tab->browser->GetMainFrame()->LoadURL(path);
            LOG_DEBUG_BROWSER("🔁 Navigate to " + path + " on tab " + std::to_string(active_tab->id) + " (window " + std::to_string(nav_wid) + ")");
        } else {
            // No active tab (e.g. first launch before tabs are ready) — create one
            LOG_INFO_BROWSER("📑 No active tab for navigation, creating new tab with: " + path);
            CreateNewTabWithUrl(path);
            SimpleHandler::NotifyTabListChanged();
        }

        return true;
    }

    if (message_name == "navigate_back") {
        LOG_DEBUG_BROWSER("🔙 navigate_back message received from role: " + role_);

        BrowserWindow* back_win = GetOwnerWindow();
        int back_wid = back_win ? back_win->window_id : 0;
        Tab* active_tab = TabManager::GetInstance().GetActiveTabForWindow(back_wid);
        if (active_tab && active_tab->browser) {
            active_tab->browser->GoBack();
            LOG_DEBUG_BROWSER("🔙 GoBack() called on tab " + std::to_string(active_tab->id) + " (window " + std::to_string(back_wid) + ")");
        } else {
            LOG_WARNING_BROWSER("⚠️ No active tab available for GoBack in window " + std::to_string(back_wid));
        }
        return true;
    }

    if (message_name == "navigate_forward") {
        LOG_DEBUG_BROWSER("🔜 navigate_forward message received from role: " + role_);

        BrowserWindow* fwd_win = GetOwnerWindow();
        int fwd_wid = fwd_win ? fwd_win->window_id : 0;
        Tab* active_tab = TabManager::GetInstance().GetActiveTabForWindow(fwd_wid);
        if (active_tab && active_tab->browser) {
            active_tab->browser->GoForward();
            LOG_DEBUG_BROWSER("🔜 GoForward() called on tab " + std::to_string(active_tab->id) + " (window " + std::to_string(fwd_wid) + ")");
        } else {
            LOG_WARNING_BROWSER("⚠️ No active tab available for GoForward in window " + std::to_string(fwd_wid));
        }
        return true;
    }

    if (message_name == "navigate_reload") {
        LOG_DEBUG_BROWSER("🔄 navigate_reload message received from role: " + role_);

        BrowserWindow* rl_win = GetOwnerWindow();
        int rl_wid = rl_win ? rl_win->window_id : 0;
        Tab* active_tab = TabManager::GetInstance().GetActiveTabForWindow(rl_wid);
        if (active_tab && active_tab->browser) {
            active_tab->browser->Reload();
            LOG_DEBUG_BROWSER("🔄 Reload() called on tab " + std::to_string(active_tab->id) + " (window " + std::to_string(rl_wid) + ")");
        } else {
            LOG_WARNING_BROWSER("⚠️ No active tab available for Reload in window " + std::to_string(rl_wid));
        }
        return true;
    }

    // ========== CERTIFICATE ERROR IPC HANDLERS ==========

    if (message_name == "cert_error_proceed") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string original_url = args->GetSize() > 0 ? args->GetString(0).ToString() : "";
        LOG_INFO_BROWSER("🔒 cert_error_proceed for URL: " + original_url);

        if (!original_url.empty()) {
            // Extract domain and add to allowed exceptions
            std::string domain;
            size_t scheme_end = original_url.find("://");
            if (scheme_end != std::string::npos) {
                size_t host_start = scheme_end + 3;
                size_t host_end = original_url.find('/', host_start);
                if (host_end == std::string::npos) host_end = original_url.length();
                std::string host_port = original_url.substr(host_start, host_end - host_start);
                size_t colon = host_port.find(':');
                domain = (colon != std::string::npos) ? host_port.substr(0, colon) : host_port;
            }
            allowed_cert_exceptions_.insert(domain);
            LOG_INFO_BROWSER("🔒 Added cert exception for domain: " + domain);

            // Navigate the active tab to the original URL
            // OnCertificateError will fire again but find the exception and call Continue()
            Tab* active_tab = TabManager::GetInstance().GetActiveTab();
            if (active_tab && active_tab->browser) {
                active_tab->browser->GetMainFrame()->LoadURL(original_url);
            }
        }
        return true;
    }

    if (message_name == "cert_error_go_back") {
        LOG_INFO_BROWSER("🔒 cert_error_go_back received");

        Tab* active_tab = TabManager::GetInstance().GetActiveTab();
        if (active_tab) {
            active_tab->has_cert_error = false;
            if (active_tab->browser) {
                if (active_tab->can_go_back) {
                    active_tab->browser->GoBack();
                } else {
                    active_tab->browser->GetMainFrame()->LoadURL("about:blank");
                }
            }
            NotifyTabListChanged();
        }
        return true;
    }

    // Duplicate address_generate handler removed - keeping the one at line 489

    // ========== OMNIBOX OVERLAY MESSAGES ==========

    if (message_name == "omnibox_create") {
#ifdef _WIN32
        extern void CreateOmniboxOverlay(HINSTANCE hInstance, bool showImmediately);
        extern HINSTANCE g_hInstance;
        // Create overlay but don't show it (showImmediately = false)
        CreateOmniboxOverlay(g_hInstance, false);
        LOG_DEBUG_BROWSER("🔍 Omnibox overlay created (hidden) for preemptive loading");
#else
        LOG_DEBUG_BROWSER("🔍 Omnibox not implemented on macOS");
#endif
        return true;
    }

    if (message_name == "omnibox_create_or_show") {
#ifdef _WIN32
        extern void CreateOmniboxOverlay(HINSTANCE hInstance, bool showImmediately);
        extern HINSTANCE g_hInstance;
        CreateOmniboxOverlay(g_hInstance, true);
        LOG_DEBUG_BROWSER("🔍 Omnibox overlay create_or_show triggered");
#else
        LOG_DEBUG_BROWSER("🔍 Omnibox not implemented on macOS");
#endif
        return true;
    }

    if (message_name == "omnibox_show") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string query = args->GetSize() > 0 ? args->GetString(0).ToString() : "";

#ifdef _WIN32
        extern void CreateOmniboxOverlay(HINSTANCE hInstance, bool showImmediately);
        extern void ShowOmniboxOverlay(BrowserWindow* targetWin = nullptr);
        extern HWND g_omnibox_overlay_hwnd;
        extern HINSTANCE g_hInstance;

        // Create if doesn't exist, otherwise show (positioned relative to requesting window)
        if (!g_omnibox_overlay_hwnd || !IsWindow(g_omnibox_overlay_hwnd)) {
            CreateOmniboxOverlay(g_hInstance, true);
        } else {
            ShowOmniboxOverlay(GetOwnerWindow());
        }

        LOG_DEBUG_BROWSER("🔍 Omnibox overlay shown with query: " + query);
        // TODO Phase 2: Send query to overlay browser for suggestion rendering
#elif defined(__APPLE__)
        extern void CreateOmniboxOverlayMacOS();
        extern void ShowOmniboxOverlayMacOS();
        extern bool IsOmniboxOverlayVisible();
        extern bool OmniboxOverlayExists();

        // Keep-alive: create once, then just show/hide
        if (IsOmniboxOverlayVisible()) {
            // Already showing
        } else if (OmniboxOverlayExists()) {
            ShowOmniboxOverlayMacOS();
        } else {
            CreateOmniboxOverlayMacOS();
        }
        LOG_DEBUG_BROWSER("Omnibox overlay shown (macOS)");
#endif
        return true;
    }

    if (message_name == "omnibox_hide") {
#ifdef _WIN32
        extern void HideOmniboxOverlay();
        HideOmniboxOverlay();
        LOG_DEBUG_BROWSER("🔍 Omnibox overlay hidden");
#elif defined(__APPLE__)
        extern void HideOmniboxOverlayMacOS();
        HideOmniboxOverlayMacOS();
        LOG_DEBUG_BROWSER("Omnibox overlay hidden (macOS)");
#endif
        return true;
    }

    if (message_name == "cookie_panel_show") {
        // Parse icon right offset and domain from args
        int iconRightOffset = 0;
        std::string shieldDomain;
        CefRefPtr<CefListValue> cp_args = message->GetArgumentList();
        if (cp_args->GetSize() > 0) {
            try { iconRightOffset = std::stoi(cp_args->GetString(0).ToString()); } catch(...) {}
        }
        if (cp_args->GetSize() > 1) {
            shieldDomain = cp_args->GetString(1).ToString();
        }

        // Fallback: if React didn't provide a domain (e.g. internal page), extract from active tab URL
        if (shieldDomain.empty()) {
            int window_id = window_id_;
            auto* activeTab = TabManager::GetInstance().GetActiveTabForWindow(window_id);
            if (activeTab && !activeTab->url.empty()) {
                std::string tabUrl = activeTab->url;
                size_t scheme_end = tabUrl.find("://");
                if (scheme_end != std::string::npos) {
                    size_t host_start = scheme_end + 3;
                    size_t host_end = tabUrl.find('/', host_start);
                    if (host_end == std::string::npos) host_end = tabUrl.length();
                    std::string host_port = tabUrl.substr(host_start, host_end - host_start);
                    size_t colon = host_port.find(':');
                    shieldDomain = (colon != std::string::npos) ? host_port.substr(0, colon) : host_port;
                    LOG_INFO_BROWSER("🛡️ Shield domain fallback from active tab: " + shieldDomain);
                }
            }
        }

#ifdef _WIN32
        extern void CreateCookiePanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void ShowCookiePanelOverlay(int iconRightOffset, BrowserWindow* targetWin = nullptr);
        extern HWND g_cookie_panel_overlay_hwnd;
        extern HINSTANCE g_hInstance;

        bool alreadyExists = g_cookie_panel_overlay_hwnd && IsWindow(g_cookie_panel_overlay_hwnd);

        // Create if doesn't exist, otherwise show (positioned relative to requesting window)
        if (!alreadyExists) {
            CreateCookiePanelOverlay(g_hInstance, true, iconRightOffset);
        } else {
            ShowCookiePanelOverlay(iconRightOffset, GetOwnerWindow());
        }

        // Inject domain into overlay via JS callback
        if (!shieldDomain.empty()) {
            CefRefPtr<CefBrowser> cookie_browser = GetCookiePanelBrowser();
            if (cookie_browser && cookie_browser->GetMainFrame()) {
                // Escape domain for safe JS injection
                std::string escapedDomain = shieldDomain;
                for (size_t i = 0; i < escapedDomain.size(); ++i) {
                    if (escapedDomain[i] == '\\' || escapedDomain[i] == '\'') {
                        escapedDomain.insert(i, "\\");
                        ++i;
                    }
                }
                std::string js = "if (window.setShieldDomain) { window.setShieldDomain('" + escapedDomain + "'); } else { console.warn('setShieldDomain not found'); }";
                cookie_browser->GetMainFrame()->ExecuteJavaScript(js, cookie_browser->GetMainFrame()->GetURL(), 0);
                LOG_INFO_BROWSER("🛡️ Shield domain injected immediately: " + shieldDomain);

                // Delayed retry: ensure domain is set even if React hasn't re-rendered
                CefRefPtr<CefBrowser> retry_browser = cookie_browser;
                std::string retry_domain = escapedDomain;
                CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b, std::string d) {
                    if (b && b->GetMainFrame()) {
                        std::string retryJs = "if (window.setShieldDomain) { window.setShieldDomain('" + d + "'); }";
                        b->GetMainFrame()->ExecuteJavaScript(retryJs, b->GetMainFrame()->GetURL(), 0);
                    }
                }, retry_browser, retry_domain), 200);
            } else {
                LOG_WARNING_BROWSER("🛡️ Shield domain injection FAILED - cookie browser or frame null (domain=" + shieldDomain + ")");
            }
            // Also store as pending for deferred injection (defense-in-depth):
            // If the browser was just created (pre-created hidden), React may not
            // have mounted yet when ExecuteJavaScript runs above.
            pending_shield_domain_ = shieldDomain;
        } else {
            LOG_WARNING_BROWSER("🛡️ Shield domain is EMPTY in cookie_panel_show IPC");
        }

        LOG_INFO_BROWSER("🛡️ Privacy shield overlay shown with iconRightOffset=" + std::to_string(iconRightOffset));
#elif defined(__APPLE__)
        extern void CreateCookiePanelOverlayWithSeparateProcess(int iconRightOffset);
        extern void ShowCookiePanelOverlay(int iconRightOffset);
        extern void HideCookiePanelOverlay();
        extern bool IsCookiePanelOverlayVisible();
        extern bool WasCookiePanelJustHidden();

        // Toggle behavior: if already visible, hide it
        // Also suppress re-show if click-outside just hid it (debounce race condition)
        if (IsCookiePanelOverlayVisible() || WasCookiePanelJustHidden()) {
            if (IsCookiePanelOverlayVisible()) {
                HideCookiePanelOverlay();
            }
            LOG_INFO_BROWSER("🛡️ Privacy shield toggled OFF (macOS)");
            return true;
        }

        // Show or create the overlay
        CefRefPtr<CefBrowser> existing_cookie = GetCookiePanelBrowser();
        if (!existing_cookie) {
            CreateCookiePanelOverlayWithSeparateProcess(iconRightOffset);
        } else {
            ShowCookiePanelOverlay(iconRightOffset);
        }

        // Inject domain into overlay via JS callback (same logic as Windows)
        if (!shieldDomain.empty()) {
            // Need delayed injection since browser may just have been created
            std::string escapedDomain = shieldDomain;
            for (size_t i = 0; i < escapedDomain.size(); ++i) {
                if (escapedDomain[i] == '\\' || escapedDomain[i] == '\'') {
                    escapedDomain.insert(i, "\\");
                    ++i;
                }
            }

            // Immediate injection attempt
            CefRefPtr<CefBrowser> cookie_browser = GetCookiePanelBrowser();
            if (cookie_browser && cookie_browser->GetMainFrame()) {
                std::string js = "if (window.setShieldDomain) { window.setShieldDomain('" + escapedDomain + "'); }";
                cookie_browser->GetMainFrame()->ExecuteJavaScript(js, cookie_browser->GetMainFrame()->GetURL(), 0);
                LOG_INFO_BROWSER("🛡️ Shield domain injected immediately (macOS): " + shieldDomain);
            }

            // Delayed retry for newly created browsers where React may not have mounted yet
            std::string retry_domain = escapedDomain;
            CefPostDelayedTask(TID_UI, base::BindOnce([](std::string d) {
                CefRefPtr<CefBrowser> b = SimpleHandler::GetCookiePanelBrowser();
                if (b && b->GetMainFrame()) {
                    std::string retryJs = "if (window.setShieldDomain) { window.setShieldDomain('" + d + "'); }";
                    b->GetMainFrame()->ExecuteJavaScript(retryJs, b->GetMainFrame()->GetURL(), 0);
                }
            }, retry_domain), 500);

            pending_shield_domain_ = shieldDomain;
        } else {
            LOG_WARNING_BROWSER("🛡️ Shield domain is EMPTY in cookie_panel_show IPC (macOS)");
        }

        LOG_INFO_BROWSER("🛡️ Privacy shield overlay shown (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
#endif
        return true;
    }

    if (message_name == "cookie_panel_hide") {
#ifdef _WIN32
        extern void HideCookiePanelOverlay();
        HideCookiePanelOverlay();
        LOG_DEBUG_BROWSER("🍪 Cookie panel overlay hidden");
#elif defined(__APPLE__)
        extern void HideCookiePanelOverlay();
        HideCookiePanelOverlay();
        LOG_DEBUG_BROWSER("🍪 Cookie panel overlay hidden (macOS)");
#endif
        return true;
    }

    // Profile Panel IPC handlers
    if (message_name == "profile_panel_show") {
        int iconRightOffset = 0;
        CefRefPtr<CefListValue> pp_args = message->GetArgumentList();
        if (pp_args->GetSize() > 0) {
            try { iconRightOffset = std::stoi(pp_args->GetString(0).ToString()); } catch(...) {}
        }

#ifdef _WIN32
        extern void CreateProfilePanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void ShowProfilePanelOverlay(int iconRightOffset, BrowserWindow* targetWin = nullptr);
        extern void HideProfilePanelOverlay();
        extern HWND g_profile_panel_overlay_hwnd;
        extern HINSTANCE g_hInstance;
        extern ULONGLONG g_profile_last_hide_tick;

        if (!g_profile_panel_overlay_hwnd || !IsWindow(g_profile_panel_overlay_hwnd)) {
            CreateProfilePanelOverlay(g_hInstance, true, iconRightOffset);
        } else if (IsWindowVisible(g_profile_panel_overlay_hwnd)) {
            HideProfilePanelOverlay();
        } else {
            // Toggle race: if just hidden by WM_ACTIVATE, don't re-open
            ULONGLONG elapsed = GetTickCount64() - g_profile_last_hide_tick;
            if (elapsed < 200) {
                LOG_DEBUG_BROWSER("Profile toggle suppressed — hidden " + std::to_string(elapsed) + "ms ago (race)");
            } else {
                ShowProfilePanelOverlay(iconRightOffset, GetOwnerWindow());
            }
        }

        LOG_DEBUG_BROWSER("Profile panel toggle handled");
#elif defined(__APPLE__)
        extern void CreateProfilePanelOverlayMacOS(int iconRightOffset);
        extern void ShowProfilePanelOverlayMacOS(int iconRightOffset);
        extern void HideProfilePanelOverlayMacOS();
        extern bool IsProfilePanelOverlayVisible();
        extern bool WasProfilePanelJustHidden();

        if (IsProfilePanelOverlayVisible() || WasProfilePanelJustHidden()) {
            if (IsProfilePanelOverlayVisible()) {
                HideProfilePanelOverlayMacOS();
            }
            return true;
        }
        CreateProfilePanelOverlayMacOS(iconRightOffset);
        LOG_DEBUG_BROWSER("Profile panel overlay shown (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
#endif
        return true;
    }

    if (message_name == "profile_panel_hide") {
#ifdef _WIN32
        extern void HideProfilePanelOverlay();
        HideProfilePanelOverlay();
        LOG_DEBUG_BROWSER("👤 Profile panel overlay hidden");
#elif defined(__APPLE__)
        extern void HideProfilePanelOverlayMacOS();
        HideProfilePanelOverlayMacOS();
        LOG_DEBUG_BROWSER("Profile panel overlay hidden (macOS)");
#endif
        return true;
    }

    // Menu Overlay IPC handlers
    if (message_name == "menu_show") {
        int iconRightOffset = 0;
        CefRefPtr<CefListValue> menu_args = message->GetArgumentList();
        if (menu_args->GetSize() > 0) {
            try { iconRightOffset = std::stoi(menu_args->GetString(0).ToString()); } catch(...) {}
        }

#ifdef _WIN32
        extern void CreateMenuOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void ShowMenuOverlay(int iconRightOffset, BrowserWindow* targetWin = nullptr);
        extern HWND g_menu_overlay_hwnd;
        extern HINSTANCE g_hInstance;

        if (!g_menu_overlay_hwnd || !IsWindow(g_menu_overlay_hwnd)) {
            CreateMenuOverlay(g_hInstance, true, iconRightOffset);
        } else {
            ShowMenuOverlay(iconRightOffset, GetOwnerWindow());
        }

        SendCurrentZoomToMenuOverlay(GetMenuBrowser());

        LOG_DEBUG_BROWSER("Menu overlay shown with iconRightOffset=" + std::to_string(iconRightOffset));
#elif defined(__APPLE__)
        extern void CreateMenuOverlayMac(int iconRightOffset);
        CreateMenuOverlayMac(iconRightOffset);
        LOG_DEBUG_BROWSER("Menu overlay shown (macOS) with iconRightOffset=" + std::to_string(iconRightOffset));
#endif
        return true;
    }

    if (message_name == "menu_hide") {
#ifdef _WIN32
        extern void HideMenuOverlay();
        HideMenuOverlay();
        LOG_DEBUG_BROWSER("Menu overlay hidden");
#elif defined(__APPLE__)
        extern void HideMenuOverlay();
        HideMenuOverlay();
        LOG_DEBUG_BROWSER("Menu overlay hidden (macOS)");
#endif
        return true;
    }

    if (message_name == "menu_action") {
        CefRefPtr<CefListValue> action_args = message->GetArgumentList();
        std::string action;
        if (action_args->GetSize() > 0) {
            action = action_args->GetString(0).ToString();
        }

        LOG_DEBUG_BROWSER("Menu action received: " + action);

        // Auto-hide menu first
        extern void HideMenuOverlay();
        HideMenuOverlay();

        // Dispatch actions
        if (action == "new_tab") {
            CreateNewTabWithUrl("");  // Always NTP
            SimpleHandler::NotifyTabListChanged();
        } else if (action == "history") {
            CreateNewTabWithUrl("http://127.0.0.1:5137/browser-data");
            SimpleHandler::NotifyTabListChanged();
        } else if (action == "settings") {
            CreateNewTabWithUrl("http://127.0.0.1:5137/settings-page/general");
            SimpleHandler::NotifyTabListChanged();
        } else if (action == "wallet") {
            CreateNewTabWithUrl("http://127.0.0.1:5137/wallet");
            SimpleHandler::NotifyTabListChanged();
        } else if (action == "about") {
            CreateNewTabWithUrl("http://127.0.0.1:5137/settings-page/about");
            SimpleHandler::NotifyTabListChanged();
        } else if (action == "downloads") {
#ifdef _WIN32
            extern void ShowDownloadPanelOverlay(int iconRightOffset, BrowserWindow* targetWin = nullptr);
            extern void CreateDownloadPanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
            extern HWND g_download_panel_overlay_hwnd;
            extern HINSTANCE g_hInstance;
            if (!g_download_panel_overlay_hwnd || !IsWindow(g_download_panel_overlay_hwnd)) {
                CreateDownloadPanelOverlay(g_hInstance, true, 100);
            } else {
                ShowDownloadPanelOverlay(100, GetOwnerWindow());
            }
            NotifyDownloadStateChanged();
#elif defined(__APPLE__)
            CreateNewTabWithUrl("http://127.0.0.1:5137/downloads");
            SimpleHandler::NotifyTabListChanged();
#endif
        } else if (action == "find") {
            // Send find_show IPC to header browser
            CefRefPtr<CefBrowser> header = GetHeaderBrowser();
            if (header) {
                CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("find_show");
                header->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
            }
        } else if (action == "print") {
            auto* active_tab = TabManager::GetInstance().GetActiveTab();
            if (active_tab && active_tab->browser) {
                active_tab->browser->GetHost()->Print();
            }
        } else if (action == "devtools") {
            auto* active_tab = TabManager::GetInstance().GetActiveTab();
            if (active_tab && active_tab->browser) {
                CefWindowInfo windowInfo;
#ifdef _WIN32
                windowInfo.SetAsPopup(nullptr, "Developer Tools");
#endif
                // macOS: default CefWindowInfo — ShowDevTools creates a new top-level window
                CefBrowserSettings devSettings;
                active_tab->browser->GetHost()->ShowDevTools(windowInfo, nullptr, devSettings, CefPoint());
            }
        } else if (action == "zoom_in") {
            auto* active_tab = GetZoomTargetTab();
            if (active_tab && active_tab->browser) {
                double level = active_tab->browser->GetHost()->GetZoomLevel();
                active_tab->browser->GetHost()->SetZoomLevel(level + 0.5);
            }
        } else if (action == "zoom_out") {
            auto* active_tab = GetZoomTargetTab();
            if (active_tab && active_tab->browser) {
                double level = active_tab->browser->GetHost()->GetZoomLevel();
                active_tab->browser->GetHost()->SetZoomLevel(level - 0.5);
            }
        } else if (action == "zoom_reset") {
            auto* active_tab = GetZoomTargetTab();
            if (active_tab && active_tab->browser) {
                active_tab->browser->GetHost()->SetZoomLevel(0.0);
            }
        } else if (action == "fullscreen") {
            // Toggle fullscreen via Windows API
#ifdef _WIN32
            extern HWND g_hwnd;
            extern bool g_is_fullscreen;
            if (!g_is_fullscreen) {
                // Enter fullscreen
                HMONITOR hMon = MonitorFromWindow(g_hwnd, MONITOR_DEFAULTTONEAREST);
                MONITORINFO mi = { sizeof(mi) };
                GetMonitorInfo(hMon, &mi);
                SetWindowLong(g_hwnd, GWL_STYLE, WS_POPUP | WS_VISIBLE);
                SetWindowPos(g_hwnd, HWND_TOP,
                    mi.rcMonitor.left, mi.rcMonitor.top,
                    mi.rcMonitor.right - mi.rcMonitor.left,
                    mi.rcMonitor.bottom - mi.rcMonitor.top,
                    SWP_FRAMECHANGED);
            } else {
                // Exit fullscreen
                SetWindowLong(g_hwnd, GWL_STYLE, WS_OVERLAPPEDWINDOW | WS_VISIBLE);
                SetWindowPos(g_hwnd, nullptr, 0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
            }
#elif defined(__APPLE__)
            extern void ToggleMainWindowFullscreen();
            ToggleMainWindowFullscreen();
#endif
        } else if (action == "exit") {
#ifdef _WIN32
            extern HWND g_hwnd;
            PostMessage(g_hwnd, WM_CLOSE, 0, 0);
#elif defined(__APPLE__)
            extern void ShowQuitConfirmationAndShutdown();
            ShowQuitConfirmationAndShutdown();
#endif
        } else if (action == "settings_privacy") {
            CreateNewTabWithUrl("http://127.0.0.1:5137/settings-page/privacy");
            SimpleHandler::NotifyTabListChanged();
        } else if (action == "bookmarks") {
            // TODO: bookmarks page
            LOG_DEBUG_BROWSER("Bookmarks action not yet implemented");
        }

        return true;
    }

    // Dedicated settings close — bypasses role_ check, works from any browser process
    if (message_name == "settings_close") {
        LOG_DEBUG_BROWSER("⚙️ settings_close message received");
#ifdef _WIN32
        extern HWND g_settings_overlay_hwnd;
        extern HHOOK g_settings_mouse_hook;
        if (g_settings_mouse_hook) {
            UnhookWindowsHookEx(g_settings_mouse_hook);
            g_settings_mouse_hook = nullptr;
        }
        CefRefPtr<CefBrowser> settings_browser = GetSettingsBrowser();
        if (settings_browser) {
            settings_browser->GetHost()->CloseBrowser(false);
        }
        if (g_settings_overlay_hwnd && IsWindow(g_settings_overlay_hwnd)) {
            DestroyWindow(g_settings_overlay_hwnd);
            g_settings_overlay_hwnd = nullptr;
            LOG_DEBUG_BROWSER("✅ Settings overlay destroyed via settings_close");
        }
#elif defined(__APPLE__)
        if (g_settings_overlay_window) {
            CefRefPtr<CefBrowser> settings_browser = GetSettingsBrowser();
            if (settings_browser) {
                settings_browser->GetHost()->CloseBrowser(false);
            }
            CloseOverlayWindow((void*)g_settings_overlay_window, (void*)g_main_window);
            g_settings_overlay_window = nullptr;
            LOG_DEBUG_BROWSER("Settings overlay destroyed via settings_close (macOS)");
        }
#endif
        return true;
    }

    // Settings persistence IPC handlers
    if (message_name == "settings_get_all") {
        LOG_DEBUG_BROWSER("⚙️ settings_get_all requested");
        
        std::string settingsJson = SettingsManager::GetInstance().ToJson();
        
        // Send response back to the requesting browser
        CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("settings_response");
        response->GetArgumentList()->SetString(0, settingsJson);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        
        LOG_DEBUG_BROWSER("⚙️ Settings response sent: " + settingsJson.substr(0, 100) + "...");
        return true;
    }

    if (message_name == "settings_set") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() < 2) {
            LOG_ERROR_BROWSER("❌ settings_set: missing key/value arguments");
            return true;
        }
        
        std::string key = args->GetString(0).ToString();
        std::string value = args->GetString(1).ToString();
        
        LOG_DEBUG_BROWSER("⚙️ settings_set: " + key + " = " + value);
        
        auto& settings = SettingsManager::GetInstance();
        
        // Browser settings
        if (key == "browser.homepage") {
            settings.SetHomepage(value);
        } else if (key == "browser.searchEngine") {
            settings.SetSearchEngine(value);
        } else if (key == "browser.zoomLevel") {
            settings.SetZoomLevel(std::stod(value));
        } else if (key == "browser.showBookmarkBar") {
            settings.SetShowBookmarkBar(value == "true");
        } else if (key == "browser.downloadsPath") {
            settings.SetDownloadsPath(value);
        } else if (key == "browser.restoreSessionOnStart") {
            settings.SetRestoreSessionOnStart(value == "true");
        } else if (key == "browser.askWhereToSave") {
            settings.SetAskWhereToSave(value == "true");
        }
        // Privacy settings
        else if (key == "privacy.adBlockEnabled") {
            settings.SetAdBlockEnabled(value == "true");
            AdblockCache::GetInstance().SetGlobalEnabled(value == "true");
        } else if (key == "privacy.thirdPartyCookieBlocking") {
            settings.SetThirdPartyCookieBlocking(value == "true");
        } else if (key == "privacy.doNotTrack") {
            settings.SetDoNotTrack(value == "true");
        } else if (key == "privacy.clearDataOnExit") {
            settings.SetClearDataOnExit(value == "true");
        } else if (key == "privacy.fingerprintProtection") {
            settings.SetFingerprintProtection(value == "true");
            FingerprintProtection::GetInstance().SetEnabled(value == "true");
        }
        // Wallet settings
        else if (key == "wallet.autoApproveEnabled") {
            settings.SetAutoApproveEnabled(value == "true");
        } else if (key == "wallet.defaultPerTxLimitCents") {
            settings.SetDefaultPerTxLimitCents(std::stoi(value));
        } else if (key == "wallet.defaultPerSessionLimitCents") {
            settings.SetDefaultPerSessionLimitCents(std::stoi(value));
        } else if (key == "wallet.defaultRateLimitPerMin") {
            settings.SetDefaultRateLimitPerMin(std::stoi(value));
        } else if (key == "wallet.peerpayAutoAccept") {
            settings.SetPeerpayAutoAccept(value == "true");
        } else {
            LOG_WARNING_BROWSER("⚠️ Unknown settings key: " + key);
        }

        // Broadcast settings_updated to the header browser so it picks up changes
        // (e.g. search engine change made in settings tab)
        if (header_browser_ && header_browser_->GetIdentifier() != browser->GetIdentifier()) {
            std::string updatedJson = SettingsManager::GetInstance().ToJson();
            CefRefPtr<CefProcessMessage> updateMsg = CefProcessMessage::Create("settings_response");
            updateMsg->GetArgumentList()->SetString(0, updatedJson);
            header_browser_->GetMainFrame()->SendProcessMessage(PID_RENDERER, updateMsg);
            LOG_DEBUG_BROWSER("⚙️ Broadcast settings_updated to header browser");
        }

        return true;
    }

    // Browse for download folder — opens native folder picker, sends result back to JS
    if (message_name == "download_browse_folder") {
        LOG_INFO_BROWSER("📂 download_browse_folder: opening folder picker");

#ifdef _WIN32
        // Use Win32 IFileOpenDialog directly — CEF's RunFileDialog labels the button "Upload"
        CefRefPtr<CefBrowser> dialogBrowser = browser;
        auto task = [dialogBrowser]() {
            CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
            IFileOpenDialog* pDialog = nullptr;
            HRESULT hr = CoCreateInstance(CLSID_FileOpenDialog, nullptr, CLSCTX_ALL,
                                          IID_IFileOpenDialog, reinterpret_cast<void**>(&pDialog));
            if (SUCCEEDED(hr) && pDialog) {
                pDialog->SetTitle(L"Select Download Folder");
                pDialog->SetOkButtonLabel(L"Select Folder");
                DWORD options = 0;
                pDialog->GetOptions(&options);
                pDialog->SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM);

                // Set initial directory if one is configured
                auto bs = SettingsManager::GetInstance().GetBrowserSettings();
                if (!bs.downloadsPath.empty()) {
                    IShellItem* pInitDir = nullptr;
                    std::wstring wpath(bs.downloadsPath.begin(), bs.downloadsPath.end());
                    if (SUCCEEDED(SHCreateItemFromParsingName(wpath.c_str(), nullptr,
                                                               IID_IShellItem, reinterpret_cast<void**>(&pInitDir)))) {
                        pDialog->SetFolder(pInitDir);
                        pInitDir->Release();
                    }
                }

                hr = pDialog->Show(nullptr);
                if (SUCCEEDED(hr)) {
                    IShellItem* pItem = nullptr;
                    if (SUCCEEDED(pDialog->GetResult(&pItem)) && pItem) {
                        PWSTR pszPath = nullptr;
                        if (SUCCEEDED(pItem->GetDisplayName(SIGDN_FILESYSPATH, &pszPath)) && pszPath) {
                            // Convert wide string to UTF-8
                            int len = WideCharToMultiByte(CP_UTF8, 0, pszPath, -1, nullptr, 0, nullptr, nullptr);
                            std::string selected(len - 1, '\0');
                            WideCharToMultiByte(CP_UTF8, 0, pszPath, -1, &selected[0], len, nullptr, nullptr);
                            CoTaskMemFree(pszPath);

                            // Send result back on UI thread
                            CefRefPtr<CefBrowser> b = dialogBrowser;
                            std::string path = selected;
                            CefPostTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> browser, std::string path) {
                                CefRefPtr<CefProcessMessage> msg =
                                    CefProcessMessage::Create("download_folder_selected");
                                msg->GetArgumentList()->SetString(0, path);
                                browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
                            }, b, path));
                        }
                        pItem->Release();
                    }
                }
                pDialog->Release();
            }
            CoUninitialize();
        };

        // Run dialog on a separate thread to avoid blocking CEF UI thread
        std::thread(task).detach();
#else
        // macOS: use CEF's RunFileDialog (shows proper "Choose" button on Mac)
        class FolderDialogCallback : public CefRunFileDialogCallback {
        public:
            explicit FolderDialogCallback(CefRefPtr<CefBrowser> browser)
                : browser_(browser) {}

            void OnFileDialogDismissed(
                const std::vector<CefString>& file_paths) override {
                if (!file_paths.empty()) {
                    std::string selected = file_paths[0].ToString();
                    CefRefPtr<CefProcessMessage> msg =
                        CefProcessMessage::Create("download_folder_selected");
                    msg->GetArgumentList()->SetString(0, selected);
                    browser_->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
                }
            }

        private:
            CefRefPtr<CefBrowser> browser_;
            IMPLEMENT_REFCOUNTING(FolderDialogCallback);
        };

        auto browserSettings = SettingsManager::GetInstance().GetBrowserSettings();
        CefString title("Select Download Folder");
        CefString defaultPath(browserSettings.downloadsPath);
        std::vector<CefString> acceptFilters;

        browser->GetHost()->RunFileDialog(
            FILE_DIALOG_OPEN_FOLDER,
            title,
            defaultPath,
            acceptFilters,
            new FolderDialogCallback(browser));
#endif

        return true;
    }

    if (message_name == "settings_update_all") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() < 1) {
            LOG_ERROR_BROWSER("❌ settings_update_all: missing JSON argument");
            return true;
        }
        
        std::string jsonStr = args->GetString(0).ToString();
        LOG_DEBUG_BROWSER("⚙️ settings_update_all: updating from JSON");
        
        if (SettingsManager::GetInstance().UpdateFromJson(jsonStr)) {
            LOG_INFO_BROWSER("✅ Settings updated successfully");
        } else {
            LOG_ERROR_BROWSER("❌ Failed to update settings from JSON");
        }
        
        return true;
    }

    // Profile Manager IPC handlers
    if (message_name == "profiles_get_all") {
        LOG_DEBUG_BROWSER("👤 profiles_get_all requested");
        
        auto profiles = ProfileManager::GetInstance().GetAllProfiles();
        auto current = ProfileManager::GetInstance().GetCurrentProfile();
        
        // Build JSON response
        std::string json = "{\"currentProfileId\":\"" + current.id + "\",\"profiles\":[";
        for (size_t i = 0; i < profiles.size(); i++) {
            if (i > 0) json += ",";
            json += "{\"id\":\"" + profiles[i].id + "\"";
            json += ",\"name\":\"" + profiles[i].name + "\"";
            json += ",\"color\":\"" + profiles[i].color + "\"";
            json += ",\"avatarInitial\":\"" + profiles[i].avatarInitial + "\"";
            if (!profiles[i].avatarImage.empty()) {
                // Avatar image is already base64 data URL, safe to include
                json += ",\"avatarImage\":\"" + profiles[i].avatarImage + "\"";
            }
            json += "}";
        }
        json += "]}";
        
        CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("profiles_result");
        response->GetArgumentList()->SetString(0, json);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        
        LOG_DEBUG_BROWSER("👤 Sent " + std::to_string(profiles.size()) + " profiles");
        return true;
    }

    if (message_name == "profiles_create") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string name = args->GetSize() > 0 ? args->GetString(0).ToString() : "New Profile";
        std::string color = args->GetSize() > 1 ? args->GetString(1).ToString() : "#5f6368";
        std::string avatarImage = args->GetSize() > 2 ? args->GetString(2).ToString() : "";
        
        bool success = ProfileManager::GetInstance().CreateProfile(name, color, avatarImage);
        LOG_INFO_BROWSER("👤 Profile created: " + name + " = " + (success ? "success" : "failed"));
        
        // Send updated profile list
        CefRefPtr<CefProcessMessage> trigger = CefProcessMessage::Create("profiles_get_all");
        OnProcessMessageReceived(browser, browser->GetMainFrame(), PID_RENDERER, trigger);
        return true;
    }

    if (message_name == "profiles_rename") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() >= 2) {
            std::string id = args->GetString(0).ToString();
            std::string newName = args->GetString(1).ToString();
            ProfileManager::GetInstance().RenameProfile(id, newName);
            LOG_INFO_BROWSER("👤 Profile renamed: " + id + " -> " + newName);
        }
        return true;
    }

    if (message_name == "profiles_delete") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() >= 1) {
            std::string id = args->GetString(0).ToString();
            ProfileManager::GetInstance().DeleteProfile(id);
            LOG_INFO_BROWSER("👤 Profile deleted: " + id);
        }
        return true;
    }

    if (message_name == "profiles_switch") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() >= 1) {
            std::string id = args->GetString(0).ToString();
            LOG_INFO_BROWSER("👤 Launching new instance with profile: " + id);
            ProfileManager::GetInstance().LaunchWithProfile(id);
        }
        return true;
    }

    // Profile Import IPC handlers
    if (message_name == "import_detect_profiles") {
        LOG_DEBUG_BROWSER("📂 import_detect_profiles requested");
        
        std::vector<DetectedProfile> profiles = ProfileImporter::DetectProfiles();
        std::string profilesJson = ProfileImporter::ProfilesToJson(profiles);
        
        CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("import_profiles_result");
        response->GetArgumentList()->SetString(0, profilesJson);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        
        LOG_DEBUG_BROWSER("📂 Found " + std::to_string(profiles.size()) + " profiles");
        return true;
    }

    if (message_name == "import_bookmarks") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() < 1) {
            LOG_ERROR_BROWSER("❌ import_bookmarks: missing profile path");
            return true;
        }
        
        std::string profilePath = args->GetString(0).ToString();
        LOG_INFO_BROWSER("📚 Starting bookmark import from: " + profilePath);
        
        ImportResult result = ProfileImporter::ImportBookmarks(profilePath);
        std::string resultJson = ProfileImporter::ResultToJson(result);
        
        CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("import_complete");
        response->GetArgumentList()->SetString(0, resultJson);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        
        return true;
    }

    if (message_name == "import_history") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() < 1) {
            LOG_ERROR_BROWSER("❌ import_history: missing profile path");
            return true;
        }
        
        std::string profilePath = args->GetString(0).ToString();
        int maxEntries = 10000;  // default
        if (args->GetSize() > 1) {
            if (args->GetType(1) == VTYPE_STRING) {
                maxEntries = std::stoi(args->GetString(1).ToString());
            } else {
                maxEntries = args->GetInt(1);
            }
        }
        
        LOG_INFO_BROWSER("📜 Starting history import from: " + profilePath + " (max: " + std::to_string(maxEntries) + ")");
        
        ImportResult result = ProfileImporter::ImportHistory(profilePath, maxEntries);
        std::string resultJson = ProfileImporter::ResultToJson(result);
        
        CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("import_complete");
        response->GetArgumentList()->SetString(0, resultJson);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        
        return true;
    }

    if (message_name == "import_all") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() < 1) {
            LOG_ERROR_BROWSER("❌ import_all: missing profile path");
            return true;
        }
        
        std::string profilePath = args->GetString(0).ToString();
        int maxHistoryEntries = 10000;  // default
        if (args->GetSize() > 1) {
            // Handle both string and int input
            if (args->GetType(1) == VTYPE_STRING) {
                maxHistoryEntries = std::stoi(args->GetString(1).ToString());
            } else {
                maxHistoryEntries = args->GetInt(1);
            }
        }
        
        LOG_INFO_BROWSER("📦 Starting full import from: " + profilePath + " (maxHistory: " + std::to_string(maxHistoryEntries) + ")");
        
        ImportResult result = ProfileImporter::ImportAll(profilePath, maxHistoryEntries);
        std::string resultJson = ProfileImporter::ResultToJson(result);
        
        CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("import_complete");
        response->GetArgumentList()->SetString(0, resultJson);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        
        return true;
    }

    if (message_name == "omnibox_update_query") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string query = args->GetString(0);

        LOG_DEBUG_BROWSER("🔍 Omnibox query update received: " + query);

        // Forward to omnibox overlay browser's renderer process
        CefRefPtr<CefBrowser> omnibox_browser = SimpleHandler::GetOmniboxBrowser();
        if (omnibox_browser && omnibox_browser->GetMainFrame()) {
            CefRefPtr<CefProcessMessage> forward_msg = CefProcessMessage::Create("omnibox_query_update");
            CefRefPtr<CefListValue> forward_args = forward_msg->GetArgumentList();
            forward_args->SetString(0, query);
            omnibox_browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, forward_msg);
            LOG_DEBUG_BROWSER("🔍 Query forwarded to omnibox overlay: " + query);
        } else {
            LOG_DEBUG_BROWSER("⚠️ Omnibox browser not available for query forward");
        }

        return true;
    }

    if (message_name == "omnibox_select") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string direction = args->GetString(0).ToString();

        LOG_DEBUG_BROWSER("🔍 Omnibox select received: " + direction);

        // Forward to omnibox overlay browser's renderer process
        CefRefPtr<CefBrowser> omnibox_browser = SimpleHandler::GetOmniboxBrowser();
        if (omnibox_browser && omnibox_browser->GetMainFrame()) {
            CefRefPtr<CefProcessMessage> forward_msg = CefProcessMessage::Create("omnibox_select");
            CefRefPtr<CefListValue> forward_args = forward_msg->GetArgumentList();
            forward_args->SetString(0, direction);
            omnibox_browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, forward_msg);
            LOG_DEBUG_BROWSER("🔍 Select forwarded to omnibox overlay: " + direction);
        }

        return true;
    }

    if (message_name == "omnibox_autocomplete") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string suggestion = args->GetSize() > 0 ? args->GetString(0).ToString() : "";

        LOG_DEBUG_BROWSER("🔍 Omnibox autocomplete received: " + suggestion);

        // Forward to header browser's renderer process
        CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
        if (header_browser && header_browser->GetMainFrame()) {
            CefRefPtr<CefProcessMessage> forward_msg = CefProcessMessage::Create("omnibox_autocomplete_update");
            CefRefPtr<CefListValue> forward_args = forward_msg->GetArgumentList();
            forward_args->SetString(0, suggestion);
            header_browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, forward_msg);
            LOG_DEBUG_BROWSER("🔍 Autocomplete forwarded to header browser: " + suggestion);
        } else {
            LOG_DEBUG_BROWSER("⚠️ Header browser not available for autocomplete forward");
        }

        return true;
    }

    // Navigate message: dismiss overlay on navigation (already handled above, just ensure dismiss)
    // NOTE: The navigate handler already exists above (line ~895), we need to add dismiss logic there

    if (message_name == "force_repaint") {
        LOG_DEBUG_BROWSER("🔄 Force repaint requested for " + role_ + " browser");

        if (browser) {
            browser->GetHost()->Invalidate(PET_VIEW);
            LOG_DEBUG_BROWSER("🔄 Browser invalidated for " + role_ + " browser");
        }
        return true;
    }

    // ========== WALLET SERVICE MESSAGES (Cross-platform) ==========
    if (message_name == "wallet_status_check") {
        LOG_DEBUG_BROWSER("🔍 Wallet status check requested");

        // NOTE: This handler only responds to wallet status check requests.
        // It does NOT shut down the application based on wallet status.
        // The application should continue running regardless of wallet status.
        // Previous code that checked wallet status on startup and shut down
        // if wallet status was false has been removed/disabled.

        nlohmann::json response;
        response["exists"] = false;
        response["needsBackup"] = true;

        try {
            LOG_DEBUG_BROWSER("🔄 Attempting to get wallet status...");

            // Create WalletService instance for this operation
            WalletService walletService;

            // Call WalletService to get wallet status
            nlohmann::json walletStatus = walletService.getWalletStatus();

            if (walletStatus.contains("exists")) {
                bool exists = walletStatus["exists"].get<bool>();
                response["exists"] = exists;
                response["needsBackup"] = !exists; // If wallet doesn't exist, needs backup

                LOG_DEBUG_BROWSER("📁 Wallet exists: " + std::string(exists ? "YES" : "NO"));
            } else {
                LOG_DEBUG_BROWSER("⚠️ Wallet status response missing 'exists' field");
                if (walletStatus.contains("error")) {
                    LOG_DEBUG_BROWSER("⚠️ Wallet status error: " + walletStatus["error"].get<std::string>());
                }
            }

        } catch (const std::exception& e) {
            LOG_DEBUG_BROWSER("⚠️ Wallet status check exception: " + std::string(e.what()));
        } catch (...) {
            LOG_DEBUG_BROWSER("⚠️ Wallet status check unknown exception");
        }

        // Always send a response, even if it's just the default "no wallet" state
        // This allows the app to continue running regardless of wallet status
        CefRefPtr<CefProcessMessage> cefResponse = CefProcessMessage::Create("wallet_status_check_response");
        CefRefPtr<CefListValue> responseArgs = cefResponse->GetArgumentList();
        responseArgs->SetString(0, response.dump());

        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cefResponse);
        LOG_DEBUG_BROWSER("📤 Wallet status sent: " + response.dump());

        return true;
    }

    if (message_name == "create_wallet") {
        LOG_DEBUG_BROWSER("🆕 Create wallet requested");
        LOG_DEBUG_BROWSER("🆕 Browser ID: " + std::to_string(browser->GetIdentifier()));
        LOG_DEBUG_BROWSER("🆕 Frame URL: " + browser->GetMainFrame()->GetURL().ToString());

        nlohmann::json response;

        try {
            WalletService walletService;

            if (!walletService.isConnected()) {
                response["success"] = false;
                response["error"] = "Wallet daemon is not running. Please start the daemon manually.";

                LOG_DEBUG_BROWSER("❌ Cannot create wallet - daemon not running");
            } else {
                // Create new wallet
                nlohmann::json newWallet = walletService.createWallet();

                if (newWallet.contains("success") && newWallet["success"].get<bool>()) {
                    response["success"] = true;
                    response["wallet"] = newWallet;

                    LOG_DEBUG_BROWSER("✅ New wallet created successfully");
                } else {
                    response["success"] = false;
                    response["error"] = "Failed to create wallet: " + newWallet.dump();

                    LOG_DEBUG_BROWSER("❌ Failed to create wallet: " + newWallet.dump());
                }
            }

        } catch (const std::exception& e) {
            response["success"] = false;
            response["error"] = "Failed to create wallet: " + std::string(e.what());

            LOG_DEBUG_BROWSER("💥 Error creating wallet: " + std::string(e.what()));
        }

        // Send response back to frontend
        CefRefPtr<CefProcessMessage> cefResponse = CefProcessMessage::Create("create_wallet_response");
        CefRefPtr<CefListValue> responseArgs = cefResponse->GetArgumentList();
        responseArgs->SetString(0, response.dump());

        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cefResponse);
        LOG_DEBUG_BROWSER("📤 Create wallet response sent: " + response.dump());

        return true;
    }

    if (message_name == "mark_wallet_backed_up") {
        LOG_DEBUG_BROWSER("✅ Mark wallet as backed up requested");

        nlohmann::json response;

        try {
            WalletService walletService;

            if (!walletService.isConnected()) {
                response["success"] = false;
                response["error"] = "Wallet daemon is not running. Please start the daemon manually.";

                LOG_DEBUG_BROWSER("❌ Cannot mark as backed up - daemon not running");
            } else {
                // Mark wallet as backed up
                bool success = walletService.markWalletBackedUp();

                if (success) {
                    response["success"] = true;
                    LOG_DEBUG_BROWSER("✅ Wallet marked as backed up successfully");
                } else {
                    response["success"] = false;
                    response["error"] = "Failed to mark wallet as backed up";

                    LOG_DEBUG_BROWSER("❌ Failed to mark wallet as backed up");
                }
            }

        } catch (const std::exception& e) {
            response["success"] = false;
            response["error"] = "Failed to mark as backed up: " + std::string(e.what());

            LOG_DEBUG_BROWSER("💥 Error marking wallet as backed up: " + std::string(e.what()));
        }

        // Send response back to frontend
        CefRefPtr<CefProcessMessage> cefResponse = CefProcessMessage::Create("mark_wallet_backed_up_response");
        CefRefPtr<CefListValue> responseArgs = cefResponse->GetArgumentList();
        responseArgs->SetString(0, response.dump());

        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cefResponse);
        LOG_DEBUG_BROWSER("📤 Mark backed up response sent: " + response.dump());

        return true;
    }

    if (message_name == "get_wallet_info") {
        LOG_DEBUG_BROWSER("🔍 Get wallet info requested");

        nlohmann::json response;

        try {
            WalletService walletService;

            if (!walletService.isConnected()) {
                response["success"] = false;
                response["error"] = "Wallet daemon is not running. Please start the daemon manually.";

                LOG_DEBUG_BROWSER("❌ Cannot get wallet info - daemon not running");
            } else {
                // Get wallet info
                nlohmann::json walletInfo = walletService.getWalletInfo();

                if (walletInfo.contains("version")) {
                    response["success"] = true;
                    response["wallet"] = walletInfo;

                    LOG_DEBUG_BROWSER("✅ Wallet info retrieved successfully");
                } else {
                    response["success"] = false;
                    response["error"] = "Failed to get wallet info: " + walletInfo.dump();

                    LOG_DEBUG_BROWSER("❌ Failed to get wallet info: " + walletInfo.dump());
                }
            }

        } catch (const std::exception& e) {
            response["success"] = false;
            response["error"] = "Failed to get wallet info: " + std::string(e.what());

            LOG_DEBUG_BROWSER("💥 Error getting wallet info: " + std::string(e.what()));
        }

        // Send response back to frontend
        CefRefPtr<CefProcessMessage> cefResponse = CefProcessMessage::Create("get_wallet_info_response");
        CefRefPtr<CefListValue> responseArgs = cefResponse->GetArgumentList();
        responseArgs->SetString(0, response.dump());

        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cefResponse);
        LOG_DEBUG_BROWSER("📤 Get wallet info response sent: " + response.dump());

        return true;
    }

    if (message_name == "load_wallet") {
        LOG_DEBUG_BROWSER("📂 Load wallet requested");

        nlohmann::json response;

        try {
            WalletService walletService;

            if (!walletService.isConnected()) {
                response["success"] = false;
                response["error"] = "Wallet daemon is not running. Please start the daemon manually.";

                LOG_DEBUG_BROWSER("❌ Cannot load wallet - daemon not running");
            } else {
                // Load wallet
                nlohmann::json loadResult = walletService.loadWallet();

                if (loadResult.contains("success") && loadResult["success"].get<bool>()) {
                    response["success"] = true;
                    response["wallet"] = loadResult;

                    LOG_DEBUG_BROWSER("✅ Wallet loaded successfully");
                } else {
                    response["success"] = false;
                    response["error"] = "Failed to load wallet: " + loadResult.dump();

                    LOG_DEBUG_BROWSER("❌ Failed to load wallet: " + loadResult.dump());
                }
            }

        } catch (const std::exception& e) {
            response["success"] = false;
            response["error"] = "Failed to load wallet: " + std::string(e.what());

            LOG_DEBUG_BROWSER("💥 Error loading wallet: " + std::string(e.what()));
        }

        // Send response back to frontend
        CefRefPtr<CefProcessMessage> cefResponse = CefProcessMessage::Create("load_wallet_response");
        CefRefPtr<CefListValue> responseArgs = cefResponse->GetArgumentList();
        responseArgs->SetString(0, response.dump());

        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cefResponse);
        LOG_DEBUG_BROWSER("📤 Load wallet response sent: " + response.dump());

        return true;
    }

    if (message_name == "get_all_addresses") {
        LOG_DEBUG_BROWSER("📍 Get all addresses requested");

        nlohmann::json response;

        try {
            WalletService walletService;

            if (!walletService.isConnected()) {
                response["success"] = false;
                response["error"] = "Wallet daemon is not running. Please start the daemon manually.";

                LOG_DEBUG_BROWSER("❌ Cannot get addresses - daemon not running");
            } else {
                // Get all addresses
                nlohmann::json addresses = walletService.getAllAddresses();

                if (addresses.is_array()) {
                    response["success"] = true;
                    response["addresses"] = addresses;

                    LOG_DEBUG_BROWSER("✅ Addresses retrieved successfully");
                } else {
                    response["success"] = false;
                    response["error"] = "Failed to get addresses: " + addresses.dump();

                    LOG_DEBUG_BROWSER("❌ Failed to get addresses: " + addresses.dump());
                }
            }

        } catch (const std::exception& e) {
            response["success"] = false;
            response["error"] = "Failed to get addresses: " + std::string(e.what());

            LOG_DEBUG_BROWSER("💥 Error getting addresses: " + std::string(e.what()));
        }

        // Send response back to frontend
        CefRefPtr<CefProcessMessage> cefResponse = CefProcessMessage::Create("get_all_addresses_response");
        CefRefPtr<CefListValue> responseArgs = cefResponse->GetArgumentList();
        responseArgs->SetString(0, response.dump());

        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cefResponse);
        LOG_DEBUG_BROWSER("📤 Get all addresses response sent: " + response.dump());

        return true;
    }

    if (message_name == "get_current_address") {
        LOG_DEBUG_BROWSER("📍 Get current address requested");

        nlohmann::json response;

        try {
            WalletService walletService;

            if (!walletService.isConnected()) {
                response["success"] = false;
                response["error"] = "Wallet daemon is not running. Please start the daemon manually.";

                LOG_DEBUG_BROWSER("❌ Cannot get current address - daemon not running");
            } else {
                // Get current address
                nlohmann::json currentAddress = walletService.getCurrentAddress();

                if (currentAddress.contains("address")) {
                    response["success"] = true;
                    response["address"] = currentAddress;

                    LOG_DEBUG_BROWSER("✅ Current address retrieved successfully");
                } else {
                    response["success"] = false;
                    response["error"] = "Failed to get current address: " + currentAddress.dump();

                    LOG_DEBUG_BROWSER("❌ Failed to get current address: " + currentAddress.dump());
                }
            }

        } catch (const std::exception& e) {
            response["success"] = false;
            response["error"] = "Failed to get current address: " + std::string(e.what());

            LOG_DEBUG_BROWSER("💥 Error getting current address: " + std::string(e.what()));
        }

        // Send response back to frontend
        CefRefPtr<CefProcessMessage> cefResponse = CefProcessMessage::Create("get_current_address_response");
        CefRefPtr<CefListValue> responseArgs = cefResponse->GetArgumentList();
        responseArgs->SetString(0, response.dump());

        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cefResponse);
        LOG_DEBUG_BROWSER("📤 Get current address response sent: " + response.dump());

        return true;
    }

    if (message_name == "get_addresses") {
        LOG_DEBUG_BROWSER("📍 Get all addresses requested");

        nlohmann::json response;

        try {
            WalletService walletService;

            if (!walletService.isConnected()) {
                response["success"] = false;
                response["error"] = "Wallet daemon is not running. Please start the daemon manually.";
                LOG_DEBUG_BROWSER("❌ Wallet daemon not connected");
            } else {
                nlohmann::json addresses = walletService.getAllAddresses();

                if (addresses.is_array()) {
                    response["success"] = true;
                    response["addresses"] = addresses;
                    LOG_DEBUG_BROWSER("✅ All addresses retrieved successfully");
                } else {
                    response["success"] = false;
                    response["error"] = "Failed to retrieve addresses: " + addresses.dump();
                    LOG_DEBUG_BROWSER("❌ Failed to retrieve addresses: " + addresses.dump());
                }
            }
        } catch (const std::exception& e) {
            response["success"] = false;
            response["error"] = "Exception: " + std::string(e.what());
            LOG_DEBUG_BROWSER("❌ Exception in get_addresses: " + std::string(e.what()));
        }

        // Send response back to renderer
        CefRefPtr<CefProcessMessage> cefResponse = CefProcessMessage::Create("get_addresses_response");
        CefRefPtr<CefListValue> responseArgs = cefResponse->GetArgumentList();
        responseArgs->SetString(0, response.dump());

        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cefResponse);
        LOG_DEBUG_BROWSER("📤 Get addresses response sent: " + response.dump());

        return true;
    }

    if (message_name == "get_backup_modal_state") {
        LOG_DEBUG_BROWSER("📨 Message received: get_backup_modal_state");

        nlohmann::json response;
        response["shown"] = getBackupModalShown();

        CefRefPtr<CefProcessMessage> cefResponse = CefProcessMessage::Create("get_backup_modal_state_response");
        CefRefPtr<CefListValue> responseArgs = cefResponse->GetArgumentList();
        responseArgs->SetString(0, response.dump());

        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cefResponse);
        LOG_DEBUG_BROWSER("📤 Backup modal state sent: " + response.dump());

        return true;
    }

    if (message_name == "set_backup_modal_state") {
        LOG_DEBUG_BROWSER("📨 Message received: set_backup_modal_state");

        CefRefPtr<CefListValue> args = message->GetArgumentList();
        LOG_DEBUG_BROWSER("🔍 Args size: " + std::to_string(args->GetSize()));

        if (args->GetSize() > 0) {
            LOG_DEBUG_BROWSER("🔍 Arg 0 type: " + std::to_string(args->GetType(0)));
            LOG_DEBUG_BROWSER("🔍 Arg 0 as string: " + args->GetString(0).ToString());
            LOG_DEBUG_BROWSER("🔍 Arg 0 as int: " + std::to_string(args->GetInt(0)));
            LOG_DEBUG_BROWSER("🔍 Arg 0 as double: " + std::to_string(args->GetDouble(0)));
        }

        bool shown = args->GetBool(0);
        LOG_DEBUG_BROWSER("🔍 Parsed boolean: " + std::to_string(shown));
        setBackupModalShown(shown);

        // Send confirmation response
        nlohmann::json response;
        response["success"] = true;

        CefRefPtr<CefProcessMessage> cefResponse = CefProcessMessage::Create("set_backup_modal_state_response");
        CefRefPtr<CefListValue> responseArgs = cefResponse->GetArgumentList();
        responseArgs->SetString(0, response.dump());

        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, cefResponse);
        LOG_DEBUG_BROWSER("📤 Backup modal state updated: " + std::to_string(shown));

        return true;
    }

    // ========== WALLET CLOSE PREVENTION (mnemonic display / PIN entry) ==========
    if (message_name == "wallet_prevent_close") {
        extern bool g_wallet_overlay_prevent_close;
        g_wallet_overlay_prevent_close = true;
        LOG_INFO_BROWSER("🔒 Wallet overlay close prevention ENABLED (mnemonic/PIN step)");
        return true;
    }
    if (message_name == "wallet_allow_close") {
        extern bool g_wallet_overlay_prevent_close;
        g_wallet_overlay_prevent_close = false;
        LOG_INFO_BROWSER("🔓 Wallet overlay close prevention DISABLED");
        return true;
    }

    // ========== WALLET DELETE (bypasses CefURLRequest proxy — uses WinHTTP directly) ==========
    if (message_name == "wallet_delete_cancel") {
        LOG_INFO_BROWSER("🗑️ wallet_delete_cancel — calling Rust /wallet/delete via WinHTTP");
#ifdef _WIN32
        HINTERNET hSession = WinHttpOpen(L"HodosBrowser/WalletDelete",
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
        if (hSession) {
            DWORD timeout = 5000;
            WinHttpSetOption(hSession, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));
            WinHttpSetOption(hSession, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
            HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 31301, 0);
            if (hConnect) {
                HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"POST", L"/wallet/delete",
                    nullptr, WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
                if (hRequest) {
                    LPCWSTR headers = L"Content-Type: application/json\r\n";
                    const char* body = "{}";
                    BOOL ok = WinHttpSendRequest(hRequest, headers, -1,
                        (LPVOID)body, 2, 2, 0);
                    if (ok) ok = WinHttpReceiveResponse(hRequest, nullptr);
                    if (ok) {
                        DWORD statusCode = 0, statusSize = sizeof(statusCode);
                        WinHttpQueryHeaders(hRequest,
                            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                            WINHTTP_HEADER_NAME_BY_INDEX, &statusCode, &statusSize,
                            WINHTTP_NO_HEADER_INDEX);
                        LOG_INFO_BROWSER("🗑️ Wallet delete HTTP status: " + std::to_string(statusCode));
                    } else {
                        LOG_ERROR_BROWSER("🗑️ Wallet delete WinHTTP failed: " + std::to_string(GetLastError()));
                    }
                    WinHttpCloseHandle(hRequest);
                } else {
                    LOG_ERROR_BROWSER("🗑️ WinHttpOpenRequest failed");
                }
                WinHttpCloseHandle(hConnect);
            } else {
                LOG_ERROR_BROWSER("🗑️ WinHttpConnect failed");
            }
            WinHttpCloseHandle(hSession);
        } else {
            LOG_ERROR_BROWSER("🗑️ WinHttpOpen failed");
        }
        // localStorage cleanup
        CefRefPtr<CefBrowser> hdr = SimpleHandler::GetHeaderBrowser();
        if (hdr && hdr->GetMainFrame()) {
            hdr->GetMainFrame()->ExecuteJavaScript(
                "localStorage.removeItem('hodos_wallet_exists');", "", 0);
        }
#endif
        // React handles overlay close separately via wallet_allow_close + handleClose()
        return true;
    }

    // ========== OVERLAY CLOSE (Cross-platform) ==========
    if (message_name == "overlay_close") {
        LOG_DEBUG_BROWSER("🧠 [SimpleHandler] overlay_close message received for role: " + role_);

#ifdef _WIN32
        // Windows implementation
        HWND target_hwnd = nullptr;
        CefRefPtr<CefBrowser> target_browser = nullptr;

        if (role_ == "settings") {
            target_hwnd = FindWindow(L"CEFSettingsOverlayWindow", L"Settings Overlay");
            target_browser = GetSettingsBrowser();
            LOG_DEBUG_BROWSER("✅ Found settings overlay window: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        } else if (role_ == "wallet") {
            extern HWND g_wallet_overlay_hwnd;
            target_hwnd = g_wallet_overlay_hwnd;
            // Use per-window wallet browser (not always window 0)
            BrowserWindow* walletOwnerWin = GetOwnerWindow();
            target_browser = (walletOwnerWin && walletOwnerWin->wallet_browser)
                ? walletOwnerWin->wallet_browser : GetWalletBrowser();
            LOG_DEBUG_BROWSER("✅ Found wallet overlay window (global): " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        } else if (role_ == "backup") {
            target_hwnd = FindWindow(L"CEFBackupOverlayWindow", L"Backup Overlay");
            target_browser = GetBackupBrowser();
            LOG_DEBUG_BROWSER("✅ Found backup overlay window: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        } else if (role_ == "brc100auth") {
            extern HWND g_brc100_auth_overlay_hwnd;
            target_hwnd = g_brc100_auth_overlay_hwnd;
            target_browser = GetBRC100AuthBrowser();
            LOG_DEBUG_BROWSER("✅ Found BRC-100 auth overlay window: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        } else if (role_ == "notification") {
            extern HWND g_notification_overlay_hwnd;
            target_hwnd = g_notification_overlay_hwnd;
            target_browser = GetNotificationBrowser();
            LOG_DEBUG_BROWSER("🔔 Found notification overlay window: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        }

        // Keep-alive: wallet overlay hides instead of destroying
        if (role_ == "wallet") {
            HideWalletOverlay();
            LOG_DEBUG_BROWSER("Wallet overlay hidden (keep-alive)");
        } else if (role_ == "notification") {
            extern HWND g_notification_overlay_hwnd;
            if (g_notification_overlay_hwnd && IsWindow(g_notification_overlay_hwnd)) {
                ShowWindow(g_notification_overlay_hwnd, SW_HIDE);
                // Reset React state to idle (no page navigation, keeps JS bundle warm)
                CefRefPtr<CefBrowser> notif = GetNotificationBrowser();
                if (notif && notif->GetMainFrame()) {
                    notif->GetMainFrame()->ExecuteJavaScript(
                        "window.hideNotification && window.hideNotification()", "", 0);
                }
            }
            extern std::string g_pendingModalDomain;
            g_pendingModalDomain = "";
            LOG_DEBUG_BROWSER("🔔 Notification overlay hidden (keep-alive), cleared modal domain");
        } else if (target_hwnd && IsWindow(target_hwnd)) {
            LOG_DEBUG_BROWSER("✅ Found " + role_ + " overlay window: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));

            // Close the browser first
            if (target_browser) {
                LOG_DEBUG_BROWSER("🔄 Closing " + role_ + " browser");
                target_browser->GetHost()->CloseBrowser(false);
                // Clear the appropriate browser reference
                if (role_ == "settings") settings_browser_ = nullptr;
                else if (role_ == "wallet") wallet_browser_ = nullptr;
                else if (role_ == "backup") backup_browser_ = nullptr;
                else if (role_ == "brc100auth") brc100_auth_browser_ = nullptr;
            }

            // Then destroy the window
            LOG_DEBUG_BROWSER("🔄 Destroying " + role_ + " overlay window");
            SendMessage(target_hwnd, WM_CLOSE, 0, 0);

            // Clear the global HWND
            if (role_ == "wallet") {
                extern HWND g_wallet_overlay_hwnd;
                g_wallet_overlay_hwnd = nullptr;
                // Clear close-prevention flag on overlay destruction
                extern bool g_wallet_overlay_prevent_close;
                g_wallet_overlay_prevent_close = false;
            } else if (role_ == "settings") {
                extern HWND g_settings_overlay_hwnd;
                g_settings_overlay_hwnd = nullptr;
                // Remove click-outside mouse hook
                extern HHOOK g_settings_mouse_hook;
                if (g_settings_mouse_hook) {
                    UnhookWindowsHookEx(g_settings_mouse_hook);
                    g_settings_mouse_hook = nullptr;
                    LOG_DEBUG_BROWSER("✅ Settings mouse hook removed on overlay_close");
                }
            } else if (role_ == "backup") {
                extern HWND g_backup_overlay_hwnd;
                g_backup_overlay_hwnd = nullptr;
            } else if (role_ == "brc100auth") {
                extern HWND g_brc100_auth_overlay_hwnd;
                g_brc100_auth_overlay_hwnd = nullptr;
            }
        } else {
            LOG_DEBUG_BROWSER("❌ " + role_ + " overlay window not found");
        }
#elif defined(__APPLE__)
        // macOS implementation
        extern NSWindow* g_main_window;
        extern NSWindow* g_settings_overlay_window;
        extern NSWindow* g_wallet_overlay_window;
        extern NSWindow* g_backup_overlay_window;
        extern NSWindow* g_brc100_auth_overlay_window;
        extern NSWindow* g_notification_overlay_window;

        NSWindow* target_window = nullptr;
        CefRefPtr<CefBrowser> target_browser = nullptr;

        if (role_ == "settings") {
            target_window = g_settings_overlay_window;
            target_browser = GetSettingsBrowser();
        } else if (role_ == "wallet") {
            target_window = g_wallet_overlay_window;
            target_browser = GetWalletBrowser();
        } else if (role_ == "backup") {
            target_window = g_backup_overlay_window;
            target_browser = GetBackupBrowser();
        } else if (role_ == "brc100auth") {
            target_window = g_brc100_auth_overlay_window;
            target_browser = GetBRC100AuthBrowser();
        } else if (role_ == "notification") {
            target_window = g_notification_overlay_window;
            target_browser = GetNotificationBrowser();
        } else if (role_ == "omnibox") {
            extern NSWindow* g_omnibox_overlay_window;
            target_window = g_omnibox_overlay_window;
            target_browser = GetOmniboxBrowser();
        } else if (role_ == "downloadpanel") {
            extern NSWindow* g_download_panel_overlay_window;
            target_window = g_download_panel_overlay_window;
            target_browser = GetDownloadPanelBrowser();
        } else if (role_ == "profilepanel") {
            extern NSWindow* g_profile_panel_overlay_window;
            target_window = g_profile_panel_overlay_window;
            target_browser = GetProfilePanelBrowser();
        }

        // Keep-alive: notification overlay hides instead of destroying
        if (role_ == "notification" && target_window) {
            HideNotificationOverlayWindow();
            CefRefPtr<CefBrowser> notif = GetNotificationBrowser();
            if (notif && notif->GetMainFrame()) {
                notif->GetMainFrame()->ExecuteJavaScript(
                    "window.hideNotification && window.hideNotification()", "", 0);
            }
            extern std::string g_pendingModalDomain;
            g_pendingModalDomain = "";
            LOG_DEBUG_BROWSER("🔔 Notification overlay hidden (keep-alive), cleared modal domain");
        } else if (target_window) {
            LOG_DEBUG_BROWSER("✅ Found " + role_ + " overlay window");

            // Close the browser first
            if (target_browser) {
                LOG_DEBUG_BROWSER("🔄 Closing " + role_ + " browser");
                target_browser->GetHost()->CloseBrowser(false);
                // Clear the appropriate browser reference
                if (role_ == "settings") settings_browser_ = nullptr;
                else if (role_ == "wallet") wallet_browser_ = nullptr;
                else if (role_ == "backup") backup_browser_ = nullptr;
                else if (role_ == "brc100auth") brc100_auth_browser_ = nullptr;
                else if (role_ == "notification") notification_browser_ = nullptr;
            }

            // Remove from parent window and close
            LOG_DEBUG_BROWSER("🔄 Destroying " + role_ + " overlay window");
            CloseOverlayWindow(target_window, g_main_window);

            // Clear the global NSWindow pointer
            if (role_ == "wallet") {
                g_wallet_overlay_window = nullptr;
            } else if (role_ == "settings") {
                g_settings_overlay_window = nullptr;
            } else if (role_ == "backup") {
                g_backup_overlay_window = nullptr;
            } else if (role_ == "brc100auth") {
                g_brc100_auth_overlay_window = nullptr;
            } else if (role_ == "notification") {
                g_notification_overlay_window = nullptr;
            } else if (role_ == "omnibox") {
                extern NSWindow* g_omnibox_overlay_window;
                g_omnibox_overlay_window = nullptr;
            } else if (role_ == "downloadpanel") {
                extern NSWindow* g_download_panel_overlay_window;
                g_download_panel_overlay_window = nullptr;
            } else if (role_ == "profilepanel") {
                extern NSWindow* g_profile_panel_overlay_window;
                g_profile_panel_overlay_window = nullptr;
            }
        } else {
            LOG_DEBUG_BROWSER("❌ " + role_ + " overlay window not found");
        }
#endif

        return true;
    }

    // ========== OVERLAY MESSAGES (Cross-platform) ==========
#ifdef _WIN32
    if (false && message_name == "overlay_hide_NEVER_CALLED_12345") {
        LOG_DEBUG_BROWSER("🪟 Hiding overlay HWND");
        LOG_DEBUG_BROWSER("🪟 Before hide - EXSTYLE: 0x" + std::to_string(GetWindowLong(nullptr, GWL_EXSTYLE)));
        ShowWindow(nullptr, SW_HIDE);
        LOG_DEBUG_BROWSER("🪟 After hide - EXSTYLE: 0x" + std::to_string(GetWindowLong(nullptr, GWL_EXSTYLE)));
        return true;
    }
#endif

    if (message_name == "overlay_show_wallet") {
        LOG_DEBUG_BROWSER("overlay_show_wallet message received from role: " + role_);

#ifdef _WIN32
        extern HINSTANCE g_hInstance;
        extern HWND g_wallet_overlay_hwnd;
        extern ULONGLONG g_wallet_last_hide_tick;
        if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd)) {
            if (IsWindowVisible(g_wallet_overlay_hwnd)) {
                HideWalletOverlay();
            } else {
                ULONGLONG elapsed = GetTickCount64() - g_wallet_last_hide_tick;
                if (elapsed < 200) {
                    LOG_DEBUG_BROWSER("Toggle suppressed — wallet hidden " + std::to_string(elapsed) + "ms ago (race)");
                } else {
                    ShowWalletOverlay(0, GetOwnerWindow());
                }
            }
        } else {
            CreateWalletOverlay(g_hInstance, true, 0);
        }
#elif defined(__APPLE__)
        CreateWalletOverlayWithSeparateProcess();
#endif
        return true;
    }

    if (message_name == "overlay_show_backup") {
        LOG_DEBUG_BROWSER("💾 overlay_show_backup message received from role: " + role_);
        LOG_DEBUG_BROWSER("💾 Creating backup overlay with separate process");

#ifdef _WIN32
        extern HINSTANCE g_hInstance;
        CreateBackupOverlayWithSeparateProcess(g_hInstance);
#elif defined(__APPLE__)
        CreateBackupOverlayWithSeparateProcess();
#endif
        return true;
    }

    // ========== MENU ACTIONS (Sprint 11a) ==========

    if (message_name == "print") {
        auto* active_tab = TabManager::GetInstance().GetActiveTab();
        if (active_tab && active_tab->browser) {
            active_tab->browser->GetHost()->Print();
        }
        return true;
    }

    if (message_name == "devtools") {
        auto* active_tab = TabManager::GetInstance().GetActiveTab();
        if (active_tab && active_tab->browser) {
            CefWindowInfo windowInfo;
#ifdef _WIN32
            windowInfo.SetAsPopup(nullptr, "Developer Tools");
#endif
            CefBrowserSettings devSettings;
            active_tab->browser->GetHost()->ShowDevTools(windowInfo, nullptr, devSettings, CefPoint());
        }
        return true;
    }

    if (message_name == "zoom_in") {
        auto* active_tab = GetZoomTargetTab();
        if (active_tab && active_tab->browser) {
            double level = active_tab->browser->GetHost()->GetZoomLevel();
            active_tab->browser->GetHost()->SetZoomLevel(level + 0.5);
        }
        return true;
    }

    if (message_name == "zoom_out") {
        auto* active_tab = GetZoomTargetTab();
        if (active_tab && active_tab->browser) {
            double level = active_tab->browser->GetHost()->GetZoomLevel();
            active_tab->browser->GetHost()->SetZoomLevel(level - 0.5);
        }
        return true;
    }

    if (message_name == "zoom_reset") {
        auto* active_tab = GetZoomTargetTab();
        if (active_tab && active_tab->browser) {
            active_tab->browser->GetHost()->SetZoomLevel(0.0);
        }
        return true;
    }

    if (message_name == "exit") {
#ifdef _WIN32
        extern HWND g_hwnd;
        PostMessage(g_hwnd, WM_CLOSE, 0, 0);
#elif defined(__APPLE__)
        // macOS quit handled via NSApp terminate
#endif
        return true;
    }

    if (message_name == "overlay_show_settings") {
        // Parse icon right offset from args (physical pixels from right edge of header)
        int iconRightOffset = 0;
        CefRefPtr<CefListValue> settings_args = message->GetArgumentList();
        if (settings_args->GetSize() > 0) {
            try { iconRightOffset = std::stoi(settings_args->GetString(0).ToString()); } catch(...) {}
        }
        LOG_DEBUG_BROWSER("Creating settings overlay with iconRightOffset=" + std::to_string(iconRightOffset));

#ifdef _WIN32
        extern HINSTANCE g_hInstance;
        CreateSettingsOverlayWithSeparateProcess(g_hInstance, iconRightOffset);
#elif defined(__APPLE__)
        CreateSettingsOverlayWithSeparateProcess(iconRightOffset);
#endif
        return true;
    }

    if (message_name == "overlay_show_settings_menu") {
        LOG_DEBUG_BROWSER("📋 overlay_show_settings_menu message received from role: " + role_);

#ifdef _WIN32
        extern HINSTANCE g_hInstance;
        extern void CreateSettingsMenuOverlay(HINSTANCE);
        CreateSettingsMenuOverlay(g_hInstance);
#elif defined(__APPLE__)
        CreateSettingsMenuOverlay();
#endif
        return true;
    }

    if (message_name == "overlay_show_brc100_auth") {
        LOG_DEBUG_BROWSER("🔐 overlay_show_brc100_auth message received from role: " + role_);

        // Extract auth request data from message
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args && args->GetSize() >= 4) {
            std::string domain = args->GetString(0).ToString();
            std::string method = args->GetString(1).ToString();
            std::string endpoint = args->GetString(2).ToString();
            std::string body = args->GetString(3).ToString();
            std::string type = (args->GetSize() >= 5) ? args->GetString(4).ToString() : "domain_approval";

            LOG_DEBUG_BROWSER("🔐 Auth request data - Domain: " + domain + ", Type: " + type + ", Method: " + method);

            // Store auth request data for the overlay to use
            extern void storePendingAuthRequest(const std::string& domain, const std::string& method, const std::string& endpoint, const std::string& body, const std::string& type);
            storePendingAuthRequest(domain, method, endpoint, body, type);
        }

        LOG_DEBUG_BROWSER("🔐 Creating BRC-100 auth overlay with separate process");

#ifdef _WIN32
        extern HINSTANCE g_hInstance;
        CreateBRC100AuthOverlayWithSeparateProcess(g_hInstance);
#elif defined(__APPLE__)
        CreateBRC100AuthOverlayWithSeparateProcess();
#endif
        return true;
    }

    if (message_name == "overlay_hide") {
        LOG_DEBUG_BROWSER("🪟 overlay_hide message received from role: " + role_);

#ifdef _WIN32
        // Close the BRC-100 auth overlay window
        HWND auth_hwnd = FindWindow(L"CEFBRC100AuthOverlayWindow", L"BRC-100 Auth Overlay");
        LOG_DEBUG_BROWSER("🪟 FindWindow result: " + std::to_string((uintptr_t)auth_hwnd));
        if (auth_hwnd) {
            LOG_DEBUG_BROWSER("🪟 Closing BRC-100 auth overlay window");
            DestroyWindow(auth_hwnd);
        } else {
            LOG_DEBUG_BROWSER("🪟 BRC-100 auth overlay window not found");
        }
#elif defined(__APPLE__)
        extern NSWindow* g_brc100_auth_overlay_window;
        extern NSWindow* g_main_window;
        if (g_brc100_auth_overlay_window) {
            CefRefPtr<CefBrowser> auth = GetBRC100AuthBrowser();
            if (auth) auth->GetHost()->CloseBrowser(false);
            brc100_auth_browser_ = nullptr;
            CloseOverlayWindow((void*)g_brc100_auth_overlay_window, (void*)g_main_window);
            g_brc100_auth_overlay_window = nullptr;
            LOG_DEBUG_BROWSER("🪟 BRC-100 auth overlay closed on macOS");
        }
#endif
        return true;
    }

    if (message_name == "brc100_auth_response") {
        LOG_DEBUG_BROWSER("🔐 brc100_auth_response message received from role: " + role_);

        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args && args->GetSize() > 0) {
            std::string responseJson = args->GetString(0).ToString();
            LOG_DEBUG_BROWSER("🔐 Auth response JSON: " + responseJson);

            try {
                nlohmann::json responseData = nlohmann::json::parse(responseJson);
                bool approved = responseData["approved"];
                bool whitelist = responseData.value("whitelist", false);
                std::string requestId = responseData.value("requestId", "");

                LOG_DEBUG_BROWSER("🔐 Auth response - Approved: " + std::to_string(approved) +
                    ", Whitelist: " + std::to_string(whitelist) +
                    ", RequestId: " + requestId);

                // Look up the pending request
                PendingAuthRequest pendingReq;
                bool found = false;
                if (!requestId.empty()) {
                    found = PendingRequestManager::GetInstance().getRequest(requestId, pendingReq);
                } else {
                    // Legacy fallback: find by modal domain
                    extern std::string g_pendingModalDomain;
                    requestId = PendingRequestManager::GetInstance().getRequestIdForDomain(g_pendingModalDomain);
                    if (!requestId.empty()) {
                        found = PendingRequestManager::GetInstance().getRequest(requestId, pendingReq);
                    }
                }

                if (approved) {
                    LOG_DEBUG_BROWSER("🔐 User approved auth request");

                    if (found) {
                        LOG_DEBUG_BROWSER("🔐 Found pending auth request for: " + pendingReq.domain);

                        // For certificate_disclosure: if user selected a subset of fields,
                        // modify the request body to only reveal those fields
                        if (pendingReq.type == "certificate_disclosure" &&
                            responseData.contains("selectedFields") &&
                            responseData["selectedFields"].is_array()) {
                            try {
                                auto bodyJson = nlohmann::json::parse(pendingReq.body);
                                bodyJson["fieldsToReveal"] = responseData["selectedFields"];
                                pendingReq.body = bodyJson.dump();
                                LOG_DEBUG_BROWSER("📋 Modified proveCertificate body — fieldsToReveal reduced to "
                                    + std::to_string(responseData["selectedFields"].size()) + " field(s)");
                            } catch (const std::exception& e) {
                                LOG_DEBUG_BROWSER("📋 Failed to modify cert body: " + std::string(e.what()));
                            }
                        }

                        // Create HTTP request to generate authentication response
                        CefRefPtr<CefRequest> cefRequest = CefRequest::Create();
                        cefRequest->SetURL("http://localhost:31301" + pendingReq.endpoint);
                        cefRequest->SetMethod(pendingReq.method);
                        cefRequest->SetHeaderByName("Content-Type", "application/json", true);

                        if (!pendingReq.body.empty()) {
                            CefRefPtr<CefPostData> postData = CefPostData::Create();
                            CefRefPtr<CefPostDataElement> element = CefPostDataElement::Create();
                            element->SetToBytes(pendingReq.body.length(), pendingReq.body.c_str());
                            postData->AddElement(element);
                            cefRequest->SetPostData(postData);
                        }

                        // Capture requestId for the async callback
                        std::string capturedRequestId = requestId;

                        class AuthResponseHandler : public CefURLRequestClient {
                        public:
                            AuthResponseHandler(const std::string& reqId) : requestId_(reqId) {}

                            void OnRequestComplete(CefRefPtr<CefURLRequest> request) override {
                                CefURLRequest::Status status = request->GetRequestStatus();
                                if (status == UR_SUCCESS && !responseData_.empty()) {
                                    LOG_DEBUG_BROWSER("🔐 Authentication response generated successfully");
                                    extern void handleAuthResponse(const std::string& requestId, const std::string& responseData);
                                    handleAuthResponse(requestId_, responseData_);
                                } else {
                                    LOG_DEBUG_BROWSER("🔐 Failed to generate authentication response (status: " + std::to_string(status) + ")");
                                }
                            }

                            void OnDownloadData(CefRefPtr<CefURLRequest> request, const void* data, size_t data_length) override {
                                responseData_.append(static_cast<const char*>(data), data_length);
                            }

                            void OnUploadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {}
                            void OnDownloadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {}
                            bool GetAuthCredentials(bool isProxy, const CefString& host, int port, const CefString& realm, const CefString& scheme, CefRefPtr<CefAuthCallback> callback) override { return false; }

                        private:
                            std::string requestId_;
                            std::string responseData_;
                            IMPLEMENT_REFCOUNTING(AuthResponseHandler);
                            DISALLOW_COPY_AND_ASSIGN(AuthResponseHandler);
                        };

                        CefRefPtr<CefURLRequest> authRequest = CefURLRequest::Create(
                            cefRequest,
                            new AuthResponseHandler(capturedRequestId),
                            nullptr
                        );

                        LOG_DEBUG_BROWSER("🔐 Authentication request sent to wallet at localhost:31301");
                    } else {
                        LOG_DEBUG_BROWSER("🔐 No pending auth request found for requestId: " + requestId);
                    }
                } else {
                    LOG_DEBUG_BROWSER("🔐 User rejected auth request");

                    if (!requestId.empty()) {
                        extern void handleAuthResponse(const std::string& requestId, const std::string& responseData);
                        handleAuthResponse(requestId, "{\"error\":\"User rejected authentication\",\"status\":\"error\"}");
                    } else {
                        extern void handleAuthResponse(const std::string& responseData);
                        handleAuthResponse("{\"error\":\"User rejected authentication\",\"status\":\"error\"}");
                    }
                    g_pendingModalDomain = "";
                }
            } catch (const std::exception& e) {
                LOG_DEBUG_BROWSER("🔐 Error parsing auth response JSON: " + std::string(e.what()));
            }
        } else {
            LOG_DEBUG_BROWSER("🔐 Invalid arguments for brc100_auth_response");
        }
        return true;
    }

    if (message_name == "add_domain_permission") {
        LOG_DEBUG_BROWSER("🔐 add_domain_permission message received from role: " + role_);

        // Extract domain from JSON
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        LOG_DEBUG_BROWSER("🔐 Args size: " + std::to_string(args ? args->GetSize() : 0));
        if (args && args->GetSize() > 0) {
            std::string permJson = args->GetString(0).ToString();
            LOG_DEBUG_BROWSER("🔐 Permission JSON: " + permJson);

            // Parse JSON data
            try {
                nlohmann::json permData = nlohmann::json::parse(permJson);
                std::string domain = permData["domain"];

                LOG_DEBUG_BROWSER("🔐 Setting domain permission - Domain: " + domain);

                // Call the domain permission API
                extern void addDomainPermission(const std::string& domain);
                addDomainPermission(domain);
            } catch (const std::exception& e) {
                LOG_DEBUG_BROWSER("🔐 Error parsing permission JSON: " + std::string(e.what()));
            }
        } else {
            LOG_DEBUG_BROWSER("🔐 Invalid arguments for add_domain_permission");
        }
        return true;
    }

    if (message_name == "add_domain_permission_advanced") {
        LOG_DEBUG_BROWSER("🔐 add_domain_permission_advanced message received from role: " + role_);

        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args && args->GetSize() > 0) {
            std::string permJson = args->GetString(0).ToString();
            LOG_DEBUG_BROWSER("🔐 Advanced permission JSON: " + permJson);

            try {
                nlohmann::json permData = nlohmann::json::parse(permJson);
                std::string domain = permData["domain"];
                int64_t perTxLimitCents = permData.value("perTxLimitCents", (int64_t)10);
                int64_t perSessionLimitCents = permData.value("perSessionLimitCents", (int64_t)300);
                int64_t rateLimitPerMin = permData.value("rateLimitPerMin", (int64_t)10);
                int64_t maxTxPerSession = permData.value("maxTxPerSession", (int64_t)100);

                LOG_DEBUG_BROWSER("🔐 Setting advanced domain permission - Domain: " + domain +
                    " tx=" + std::to_string(perTxLimitCents) +
                    " session=" + std::to_string(perSessionLimitCents) +
                    " rate=" + std::to_string(rateLimitPerMin) +
                    " maxTxPerSession=" + std::to_string(maxTxPerSession));

                extern void addDomainPermissionAdvanced(const std::string& domain,
                    int64_t perTxLimitCents, int64_t perSessionLimitCents, int64_t rateLimitPerMin,
                    int64_t maxTxPerSession);
                addDomainPermissionAdvanced(domain, perTxLimitCents, perSessionLimitCents, rateLimitPerMin, maxTxPerSession);
            } catch (const std::exception& e) {
                LOG_DEBUG_BROWSER("🔐 Error parsing advanced permission JSON: " + std::string(e.what()));
            }
        } else {
            LOG_DEBUG_BROWSER("🔐 Invalid arguments for add_domain_permission_advanced");
        }
        return true;
    }

    if (message_name == "approve_cert_fields") {
        LOG_DEBUG_BROWSER("📋 approve_cert_fields message received from role: " + role_);

        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args && args->GetSize() > 0) {
            std::string certJson = args->GetString(0).ToString();
            LOG_DEBUG_BROWSER("📋 Cert fields JSON: " + certJson);

            try {
                nlohmann::json certData = nlohmann::json::parse(certJson);
                std::string domain = certData.value("domain", "");
                std::string certType = certData.value("certType", "");
                bool remember = certData.value("remember", true);
                std::vector<std::string> fields;
                if (certData.contains("fields") && certData["fields"].is_array()) {
                    for (const auto& f : certData["fields"]) {
                        if (f.is_string()) fields.push_back(f.get<std::string>());
                    }
                }

                if (remember && !domain.empty() && !certType.empty() && !fields.empty()) {
                    LOG_DEBUG_BROWSER("📋 Persisting cert field permissions for " + domain + " (" + std::to_string(fields.size()) + " fields)");

                    // Fire-and-forget POST to Rust backend
                    // Uses CefTask subclass pattern (same as DomainPermissionTask)
                    class CertFieldPermissionTask : public CefTask {
                    public:
                        CertFieldPermissionTask(const std::string& domain, const std::string& certType,
                                                const std::vector<std::string>& fields)
                            : domain_(domain), certType_(certType), fields_(fields) {}

                        void Execute() override {
                            // Build JSON body
                            nlohmann::json body;
                            body["domain"] = domain_;
                            body["cert_type"] = certType_;
                            body["fields"] = fields_;
                            body["remember"] = true;
                            std::string jsonBody = body.dump();

                            CefRefPtr<CefRequest> cefRequest = CefRequest::Create();
                            cefRequest->SetURL("http://localhost:31301/domain/permissions/certificate");
                            cefRequest->SetMethod("POST");
                            cefRequest->SetHeaderByName("Content-Type", "application/json", true);

                            CefRefPtr<CefPostData> postData = CefPostData::Create();
                            CefRefPtr<CefPostDataElement> element = CefPostDataElement::Create();
                            element->SetToBytes(jsonBody.length(), jsonBody.c_str());
                            postData->AddElement(element);
                            cefRequest->SetPostData(postData);

                            // Minimal CefURLRequestClient (fire-and-forget)
                            class CertFieldResponseHandler : public CefURLRequestClient {
                            public:
                                void OnRequestComplete(CefRefPtr<CefURLRequest> request) override {}
                                void OnUploadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {}
                                void OnDownloadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {}
                                void OnDownloadData(CefRefPtr<CefURLRequest> request, const void* data, size_t data_length) override {}
                                bool GetAuthCredentials(bool isProxy, const CefString& host, int port,
                                                        const CefString& realm, const CefString& scheme,
                                                        CefRefPtr<CefAuthCallback> callback) override { return false; }
                            private:
                                IMPLEMENT_REFCOUNTING(CertFieldResponseHandler);
                            };

                            CefURLRequest::Create(cefRequest, new CertFieldResponseHandler(), nullptr);
                        }
                    private:
                        std::string domain_;
                        std::string certType_;
                        std::vector<std::string> fields_;
                        IMPLEMENT_REFCOUNTING(CertFieldPermissionTask);
                    };

                    CefPostTask(TID_UI, new CertFieldPermissionTask(domain, certType, fields));
                } else {
                    LOG_DEBUG_BROWSER("📋 Cert field approval not persisted (remember=" + std::to_string(remember) + ")");
                }
            } catch (const std::exception& e) {
                LOG_DEBUG_BROWSER("📋 Error parsing cert fields JSON: " + std::string(e.what()));
            }
        } else {
            LOG_DEBUG_BROWSER("📋 Invalid arguments for approve_cert_fields");
        }
        return true;
    }

    if (message_name == "test_settings_message") {
        LOG_DEBUG_BROWSER("🧪 test_settings_message received from role: " + role_);
        return true;
    }

    if (false && message_name == "overlay_hide_NEVER_CALLED_67890" && role_ == "settings") {
        LOG_DEBUG_BROWSER("🪟 overlay_hide message received for settings overlay");

#ifdef _WIN32
        // Close the settings overlay window
        HWND settings_hwnd = FindWindow(L"CEFSettingsOverlayWindow", L"Settings Overlay");
        if (settings_hwnd) {
            LOG_DEBUG_BROWSER("🪟 Closing settings overlay window");
            DestroyWindow(settings_hwnd);
        }
#elif defined(__APPLE__)
        extern NSWindow* g_settings_overlay_window;
        extern NSWindow* g_main_window;
        if (g_settings_overlay_window) {
            CefRefPtr<CefBrowser> settings = GetSettingsBrowser();
            if (settings) settings->GetHost()->CloseBrowser(false);
            settings_browser_ = nullptr;
            CloseOverlayWindow((void*)g_settings_overlay_window, (void*)g_main_window);
            g_settings_overlay_window = nullptr;
            LOG_DEBUG_BROWSER("🪟 Settings overlay closed on macOS");
        }
#endif
        return true;
    }

    if (message_name == "overlay_input") {
        LOG_DEBUG_BROWSER("🪟 overlay_input message received from role: " + role_);

        CefRefPtr<CefListValue> args = message->GetArgumentList();
        bool enable = args->GetBool(0);
        LOG_DEBUG_BROWSER("🪟 Setting overlay input: " + std::string(enable ? "enabled" : "disabled") + " for role: " + role_);

#ifdef _WIN32
        // Handle input for the appropriate overlay based on role
        HWND target_hwnd = nullptr;
        if (role_ == "settings") {
            // Find the settings overlay window
            target_hwnd = FindWindow(L"CEFSettingsOverlayWindow", L"Settings Overlay");
            LOG_DEBUG_BROWSER("🪟 Settings overlay HWND found: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        } else if (role_ == "wallet") {
            // Find the wallet overlay window
            target_hwnd = FindWindow(L"CEFWalletOverlayWindow", L"Wallet Overlay");
            LOG_DEBUG_BROWSER("💰 Wallet overlay HWND found: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        } else if (role_ == "backup") {
            // Find the backup overlay window
            target_hwnd = FindWindow(L"CEFBackupOverlayWindow", L"Backup Overlay");
            LOG_DEBUG_BROWSER("💾 Backup overlay HWND found: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        }

        if (target_hwnd) {
            LONG exStyle = GetWindowLong(target_hwnd, GWL_EXSTYLE);
            if (enable) {
                SetWindowLong(target_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);
                LOG_DEBUG_BROWSER("🪟 Mouse input ENABLED for HWND: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
            } else {
                SetWindowLong(target_hwnd, GWL_EXSTYLE, exStyle | WS_EX_TRANSPARENT);
                LOG_DEBUG_BROWSER("🪟 Mouse input DISABLED for HWND: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
            }
        } else {
            LOG_DEBUG_BROWSER("❌ No target HWND found for overlay_input");
        }
#elif defined(__APPLE__)
        extern NSWindow* g_settings_overlay_window;
        extern NSWindow* g_wallet_overlay_window;
        extern NSWindow* g_backup_overlay_window;

        NSWindow* target_window = nullptr;
        if (role_ == "settings") target_window = g_settings_overlay_window;
        else if (role_ == "wallet") target_window = g_wallet_overlay_window;
        else if (role_ == "backup") target_window = g_backup_overlay_window;

        if (target_window) {
            SetOverlayIgnoresMouseEvents((void*)target_window, !enable);
            LOG_DEBUG_BROWSER("🪟 Mouse input " + std::string(enable ? "ENABLED" : "DISABLED") + " for " + role_ + " overlay");
        } else {
            LOG_DEBUG_BROWSER("❌ No target window found for overlay_input");
        }
#endif
        return true;
    }

    // ========== WALLET PANEL TOGGLE (Cross-platform: uses separate overlay window) ==========
    if (message_name == "toggle_wallet_panel") {
        // Parse comma-separated args: "iconRightOffset,peerpayCount,peerpayAmount"
        int iconRightOffset = 0;
        int peerpayCount = 0;
        int peerpayAmount = 0;
        CefRefPtr<CefListValue> wallet_args = message->GetArgumentList();
        if (wallet_args->GetSize() > 0) {
            std::string argsStr = wallet_args->GetString(0).ToString();
            // Split by comma
            std::istringstream ss(argsStr);
            std::string token;
            int idx = 0;
            while (std::getline(ss, token, ',')) {
                try {
                    if (idx == 0) iconRightOffset = std::stoi(token);
                    else if (idx == 1) peerpayCount = std::stoi(token);
                    else if (idx == 2) peerpayAmount = std::stoi(token);
                } catch(...) {}
                idx++;
            }
        }
        LOG_DEBUG_BROWSER("Toggle wallet panel with iconRightOffset=" + std::to_string(iconRightOffset) +
            " peerpayCount=" + std::to_string(peerpayCount) + " peerpayAmount=" + std::to_string(peerpayAmount));

        // Store peerpay data so CreateWalletOverlay can append to URL
        extern int g_peerpay_count;
        extern int g_peerpay_amount;
        g_peerpay_count = peerpayCount;
        g_peerpay_amount = peerpayAmount;

#ifdef _WIN32
        extern HINSTANCE g_hInstance;
        extern HWND g_wallet_overlay_hwnd;
        extern ULONGLONG g_wallet_last_hide_tick;
        if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd)) {
            if (IsWindowVisible(g_wallet_overlay_hwnd)) {
                HideWalletOverlay();
            } else {
                // Suppress toggle race: if the wallet was JUST hidden by WM_ACTIVATE
                // (because this click stole focus from the wallet), don't re-open it.
                // The hide and the IPC arrive within ~50ms of each other.
                ULONGLONG elapsed = GetTickCount64() - g_wallet_last_hide_tick;
                if (elapsed < 200) {
                    LOG_DEBUG_BROWSER("Toggle suppressed — wallet hidden " + std::to_string(elapsed) + "ms ago (race)");
                } else {
                    ShowWalletOverlay(iconRightOffset, GetOwnerWindow());
                }
            }
        } else {
            CreateWalletOverlay(g_hInstance, true, iconRightOffset);
        }
#elif defined(__APPLE__)
        CreateWalletOverlayWithSeparateProcess(iconRightOffset);
#else
        LOG_DEBUG_BROWSER("toggle_wallet_panel not implemented on this platform");
#endif
        return true;
    }

    // Forward wallet_payment_dismissed from wallet overlay → header browser (clears green dot)
    if (message_name == "wallet_payment_dismissed") {
        CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
        if (header_browser && header_browser->GetMainFrame()) {
            CefRefPtr<CefProcessMessage> forward_msg = CefProcessMessage::Create("wallet_payment_dismissed");
            header_browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, forward_msg);
            LOG_DEBUG_BROWSER("Forwarded wallet_payment_dismissed to header browser");
        }
        return true;
    }

#ifdef _WIN32
    if (message_name == "address_generate") {
        LOG_DEBUG_BROWSER("🔑 Address generation requested from browser ID: " + std::to_string(browser->GetIdentifier()));

        try {
            // Call WalletService to generate address
            WalletService walletService;
            nlohmann::json addressData = walletService.generateAddress();

            LOG_DEBUG_BROWSER("✅ Address generated successfully: " + addressData.dump());

            // Send result back to the requesting browser
            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("address_generate_response");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, addressData.dump());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
            LOG_DEBUG_BROWSER("📤 Address data sent back to browser");
            LOG_DEBUG_BROWSER("🔍 Browser ID: " + std::to_string(browser->GetIdentifier()));
            LOG_DEBUG_BROWSER("🔍 Frame URL: " + browser->GetMainFrame()->GetURL().ToString());

        } catch (const std::exception& e) {
            LOG_DEBUG_BROWSER("❌ Address generation failed: " + std::string(e.what()));

            // Send error response
            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("address_generate_error");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, e.what());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        }

        return true;
    }
#else
    if (message_name == "address_generate") {
        LOG_DEBUG_BROWSER("🔑 Address generation requested from browser ID: " + std::to_string(browser->GetIdentifier()));

        try {
            // Call WalletService to generate address
            WalletService walletService;
            nlohmann::json addressData = walletService.generateAddress();

            LOG_DEBUG_BROWSER("✅ Address generated successfully: " + addressData.dump());

            // Send result back to the requesting browser
            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("address_generate_response");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, addressData.dump());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
            LOG_DEBUG_BROWSER("📤 Address data sent back to browser");
            LOG_DEBUG_BROWSER("🔍 Browser ID: " + std::to_string(browser->GetIdentifier()));
            LOG_DEBUG_BROWSER("🔍 Frame URL: " + browser->GetMainFrame()->GetURL().ToString());

        } catch (const std::exception& e) {
            LOG_DEBUG_BROWSER("❌ Address generation failed: " + std::string(e.what()));

            // Send error response
            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("address_generate_error");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, e.what());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        }

        return true;
    }
#endif

    // Transaction Message Handlers (Cross-platform)

    if (message_name == "create_transaction") {
        LOG_DEBUG_BROWSER("💰 Create transaction requested from browser ID: " + std::to_string(browser->GetIdentifier()));

        try {
            // Parse transaction data from message arguments
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            if (args->GetSize() > 0) {
                std::string transactionDataJson = args->GetString(0);
                nlohmann::json transactionData = nlohmann::json::parse(transactionDataJson);

                // Call WalletService to create transaction
                WalletService walletService;
                nlohmann::json result = walletService.createTransaction(transactionData);

                LOG_DEBUG_BROWSER("✅ Transaction creation result: " + result.dump());

                // Send result back to the requesting browser
                CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("create_transaction_response");
                CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
                responseArgs->SetString(0, result.dump());

                browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
                LOG_DEBUG_BROWSER("📤 Transaction creation response sent back to browser");
            } else {
                throw std::runtime_error("No transaction data provided");
            }

        } catch (const std::exception& e) {
            LOG_DEBUG_BROWSER("❌ Transaction creation failed: " + std::string(e.what()));

            // Send error response
            nlohmann::json errorResponse;
            errorResponse["error"] = e.what();

            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("create_transaction_error");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, errorResponse.dump());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        }

        return true;
    }

    if (message_name == "sign_transaction") {
        LOG_DEBUG_BROWSER("✍️ Sign transaction requested from browser ID: " + std::to_string(browser->GetIdentifier()));

        try {
            // Parse transaction data from message arguments
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            if (args->GetSize() > 0) {
                std::string transactionDataJson = args->GetString(0);
                nlohmann::json transactionData = nlohmann::json::parse(transactionDataJson);

                // Call WalletService to sign transaction
                WalletService walletService;
                nlohmann::json result = walletService.signTransaction(transactionData);

                LOG_DEBUG_BROWSER("✅ Transaction signing result: " + result.dump());

                // Send result back to the requesting browser
                CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("sign_transaction_response");
                CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
                responseArgs->SetString(0, result.dump());

                browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
                LOG_DEBUG_BROWSER("📤 Transaction signing response sent back to browser");
            } else {
                throw std::runtime_error("No transaction data provided");
            }

        } catch (const std::exception& e) {
            LOG_DEBUG_BROWSER("❌ Transaction signing failed: " + std::string(e.what()));

            // Send error response
            nlohmann::json errorResponse;
            errorResponse["error"] = e.what();

            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("sign_transaction_error");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, errorResponse.dump());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        }

        return true;
    }

    if (message_name == "broadcast_transaction") {
        LOG_DEBUG_BROWSER("📡 Broadcast transaction requested from browser ID: " + std::to_string(browser->GetIdentifier()));

        try {
            // Parse transaction data from message arguments
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            if (args->GetSize() > 0) {
                std::string transactionDataJson = args->GetString(0);
                nlohmann::json transactionData = nlohmann::json::parse(transactionDataJson);

                // Call WalletService to broadcast transaction
                WalletService walletService;
                nlohmann::json result = walletService.broadcastTransaction(transactionData);

                LOG_DEBUG_BROWSER("✅ Transaction broadcast result: " + result.dump());

                // Send result back to the requesting browser
                CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("broadcast_transaction_response");
                CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
                responseArgs->SetString(0, result.dump());

                browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
                LOG_DEBUG_BROWSER("📤 Transaction broadcast response sent back to browser");
            } else {
                throw std::runtime_error("No transaction data provided");
            }

        } catch (const std::exception& e) {
            LOG_DEBUG_BROWSER("❌ Transaction broadcast failed: " + std::string(e.what()));

            // Send error response
            nlohmann::json errorResponse;
            errorResponse["error"] = e.what();

            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("broadcast_transaction_error");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, errorResponse.dump());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        }

        return true;
    }


        if (message_name == "get_balance") {
        LOG_DEBUG_BROWSER("💰 Get balance requested from browser ID: " + std::to_string(browser->GetIdentifier()));

        try {
            // Call WalletService to get balance (no arguments needed)
            WalletService walletService;

            // Pass empty JSON object to satisfy the method signature
            nlohmann::json emptyData = nlohmann::json::object();
            nlohmann::json result = walletService.getBalance(emptyData);

            LOG_DEBUG_BROWSER("✅ Balance result: " + result.dump());

            // Send result back to the requesting browser
            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("get_balance_response");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, result.dump());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
            LOG_DEBUG_BROWSER("📤 Balance response sent back to browser");

        } catch (const std::exception& e) {
            LOG_DEBUG_BROWSER("❌ Get balance failed: " + std::string(e.what()));

            // Send error response
            nlohmann::json errorResponse;
            errorResponse["error"] = e.what();

            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("get_balance_error");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, errorResponse.dump());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        }

        return true;
    }

    if (message_name == "send_transaction") {
        LOG_DEBUG_BROWSER("🚀 Send transaction requested from browser ID: " + std::to_string(browser->GetIdentifier()));

        try {
            // Parse transaction data from message arguments
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            LOG_DEBUG_BROWSER("🔍 send_transaction: args->GetSize() = " + std::to_string(args->GetSize()));

            if (args->GetSize() > 0) {
                std::string transactionDataJson = args->GetString(0);
                LOG_DEBUG_BROWSER("🔍 send_transaction: received JSON = " + transactionDataJson);

                nlohmann::json transactionData = nlohmann::json::parse(transactionDataJson);

                // Call WalletService to send transaction
                LOG_DEBUG_BROWSER("🔍 About to call WalletService::sendTransaction");
                WalletService walletService;

                LOG_DEBUG_BROWSER("🔍 Calling sendTransaction...");
                std::cout.flush();
                std::cerr.flush();

                nlohmann::json result;
                try {
                    LOG_DEBUG_BROWSER("🔍 About to call walletService.sendTransaction()...");
                    std::cout.flush();
                    result = walletService.sendTransaction(transactionData);
                    LOG_DEBUG_BROWSER("✅ sendTransaction returned successfully");
                    std::cout.flush();
                } catch (const std::exception& e) {
                    LOG_DEBUG_BROWSER("❌ Exception in sendTransaction: " + std::string(e.what()));
                    std::cout.flush();
                    throw;
                } catch (...) {
                    LOG_DEBUG_BROWSER("❌ Unknown exception in sendTransaction");
                    std::cout.flush();
                    throw;
                }

                LOG_DEBUG_BROWSER("🔍 About to dump result...");
                std::string resultStr;
                try {
                    // Check if result is valid before dumping
                    if (result.is_null() || result.empty()) {
                        LOG_DEBUG_BROWSER("⚠️ Result is null or empty, using default error");
                        resultStr = "{\"success\":false,\"error\":\"Invalid response from wallet\"}";
                    } else {
                        resultStr = result.dump();
                        LOG_DEBUG_BROWSER("✅ Result dumped, length: " + std::to_string(resultStr.length()));

                        // Truncate if too long (CEF has message size limits)
                        const size_t MAX_MESSAGE_SIZE = 512; // Keep it small
                        if (resultStr.length() > MAX_MESSAGE_SIZE) {
                            LOG_DEBUG_BROWSER("⚠️ Result too long (" + std::to_string(resultStr.length()) + "), truncating");
                            // Try to preserve the error message if it exists
                            if (result.contains("error") && result["error"].is_string()) {
                                std::string errorMsg = result["error"].get<std::string>();
                                if (errorMsg.length() > 100) {
                                    errorMsg = errorMsg.substr(0, 100) + "...";
                                }
                                resultStr = "{\"success\":false,\"error\":\"" + errorMsg + "\",\"status\":\"failed\"}";
                            } else {
                                resultStr = resultStr.substr(0, MAX_MESSAGE_SIZE) + "...(truncated)";
                            }
                        }
                    }
                } catch (const std::exception& e) {
                    LOG_DEBUG_BROWSER("❌ Exception dumping result: " + std::string(e.what()));
                    resultStr = "{\"success\":false,\"error\":\"Failed to serialize response\"}";
                } catch (...) {
                    LOG_DEBUG_BROWSER("❌ Unknown exception dumping result");
                    resultStr = "{\"success\":false,\"error\":\"Unknown error\"}";
                }

                LOG_DEBUG_BROWSER("✅ Transaction result received, length: " + std::to_string(resultStr.length()));

                // Send result back to the requesting browser
                LOG_DEBUG_BROWSER("🔍 Creating process message");
                CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("send_transaction_response");
                if (!response) {
                    LOG_DEBUG_BROWSER("❌ Failed to create process message");
                    throw std::runtime_error("Failed to create process message");
                }

                LOG_DEBUG_BROWSER("🔍 Getting argument list");
                CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
                if (!responseArgs) {
                    LOG_DEBUG_BROWSER("❌ Failed to get argument list");
                    throw std::runtime_error("Failed to get argument list");
                }

                LOG_DEBUG_BROWSER("🔍 Setting string argument (length: " + std::to_string(resultStr.length()) + ")");
                std::cout.flush();

                // Check if string is too large (CEF has limits)
                if (resultStr.length() > 1000000) { // 1MB limit
                    LOG_DEBUG_BROWSER("⚠️ Response too large, truncating error message");
                    nlohmann::json truncated = result;
                    if (truncated.contains("error") && truncated["error"].is_string()) {
                        std::string error = truncated["error"].get<std::string>();
                        if (error.length() > 500) {
                            error = error.substr(0, 500) + "... (truncated)";
                            truncated["error"] = error;
                        }
                    }
                    resultStr = truncated.dump();
                }

                try {
                    responseArgs->SetString(0, resultStr);
                    LOG_DEBUG_BROWSER("✅ String argument set successfully");
                } catch (const std::exception& e) {
                    LOG_DEBUG_BROWSER("❌ Failed to set string argument: " + std::string(e.what()));
                    throw;
                }
                std::cout.flush();

                LOG_DEBUG_BROWSER("🔍 Getting main frame");
                CefRefPtr<CefFrame> mainFrame = browser->GetMainFrame();
                if (!mainFrame) {
                    LOG_DEBUG_BROWSER("❌ Failed to get main frame");
                    throw std::runtime_error("Failed to get main frame");
                }

                LOG_DEBUG_BROWSER("🔍 Sending process message to renderer");
                mainFrame->SendProcessMessage(PID_RENDERER, response);
                LOG_DEBUG_BROWSER("📤 Transaction response sent back to browser");
            } else {
                LOG_DEBUG_BROWSER("❌ send_transaction: No arguments provided, args->GetSize() = " + std::to_string(args->GetSize()));
                throw std::runtime_error("No transaction data provided");
            }

        } catch (const std::exception& e) {
            LOG_DEBUG_BROWSER("❌ Send transaction failed: " + std::string(e.what()));

            // Send error response
            nlohmann::json errorResponse;
            errorResponse["error"] = e.what();

            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("send_transaction_error");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, errorResponse.dump());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        }

        return true;
    }

    if (message_name == "get_transaction_history") {
        LOG_DEBUG_BROWSER("📜 Get transaction history requested from browser ID: " + std::to_string(browser->GetIdentifier()));

        try {
            // Call WalletService to get transaction history
            WalletService walletService;
            nlohmann::json result = walletService.getTransactionHistory();

            LOG_DEBUG_BROWSER("✅ Transaction history result: " + result.dump());

            // Send result back to the requesting browser
            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("get_transaction_history_response");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, result.dump());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
            LOG_DEBUG_BROWSER("📤 Transaction history response sent back to browser");

        } catch (const std::exception& e) {
            LOG_DEBUG_BROWSER("❌ Get transaction history failed: " + std::string(e.what()));

            // Send error response
            nlohmann::json errorResponse;
            errorResponse["error"] = e.what();

            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("get_transaction_history_error");
            CefRefPtr<CefListValue> responseArgs = response->GetArgumentList();
            responseArgs->SetString(0, errorResponse.dump());

            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        }

        return true;
    }
    // All wallet handlers now cross-platform (WalletService has platform implementations)

    // ========== NEW TAB PAGE (G4) ==========

    if (message_name == "get_most_visited") {
        auto topSites = HistoryManager::GetInstance().GetTopSites(8);

        nlohmann::json response = nlohmann::json::array();
        for (const auto& entry : topSites) {
            nlohmann::json site;
            site["url"] = entry.url;
            site["title"] = entry.title;
            site["visitCount"] = entry.visit_count;
            response.push_back(site);
        }

        std::string jsonStr = response.dump();

        // Send response back to the requesting browser (tab), NOT header_browser_
        CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("most_visited_response");
        msg->GetArgumentList()->SetString(0, jsonStr);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);

        LOG_DEBUG_BROWSER("📊 Sent most_visited_response with " + std::to_string(topSites.size()) + " sites");
        return true;
    }

    if (message_name == "get_session_blocked_total") {
        int total = AdblockCache::GetInstance().getTotalSessionBlocked();

        nlohmann::json response;
        response["total"] = total;
        std::string jsonStr = response.dump();

        CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("session_blocked_total_response");
        msg->GetArgumentList()->SetString(0, jsonStr);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);

        LOG_DEBUG_BROWSER("🛡️ Sent session_blocked_total_response: " + std::to_string(total));
        return true;
    }

    // ========== GOOGLE SUGGEST SERVICE ==========
    if (message_name == "google_suggest_request") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string query = args->GetSize() > 0 ? args->GetString(0).ToString() : "";
        int requestId = args->GetSize() > 1 ? args->GetInt(1) : 0;

        // Read search engine from settings
        std::string engine = SettingsManager::GetInstance().GetBrowserSettings().searchEngine;
        LOG_DEBUG_BROWSER("🔍 Suggest request for query: " + query + " engine: " + engine + " (requestId: " + std::to_string(requestId) + ")");

        // Fetch suggestions using the configured engine
        std::vector<std::string> suggestions = GoogleSuggestService::GetInstance().fetchSuggestions(query, engine);

        LOG_DEBUG_BROWSER("🔍 Got " + std::to_string(suggestions.size()) + " suggestions from Google");

        // Build JSON array response
        nlohmann::json response = nlohmann::json::array();
        for (const std::string& suggestion : suggestions) {
            response.push_back(suggestion);
        }

        // Send response back to render process with requestId
        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("google_suggest_response");
        CefRefPtr<CefListValue> responseArgs = responseMsg->GetArgumentList();
        responseArgs->SetString(0, response.dump());
        responseArgs->SetInt(1, requestId);

        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        LOG_DEBUG_BROWSER("📤 Google Suggest response sent: " + response.dump() + " (requestId: " + std::to_string(requestId) + ")");

        return true;
    }

    // ========== COOKIE MANAGEMENT MESSAGES ==========

    if (message_name == "cookie_get_all") {
        CookieManager::HandleGetAllCookies(browser);
        return true;
    }

    if (message_name == "cookie_delete") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string url = args->GetString(0).ToString();
        std::string name = args->GetString(1).ToString();
        CookieManager::HandleDeleteCookie(browser, url, name);
        return true;
    }

    if (message_name == "cookie_delete_domain") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string domain = args->GetString(0).ToString();
        CookieManager::HandleDeleteDomainCookies(browser, domain);
        return true;
    }

    if (message_name == "cookie_delete_all") {
        CookieManager::HandleDeleteAllCookies(browser);
        return true;
    }

    if (message_name == "cache_clear") {
        CookieManager::HandleClearCache(browser);
        return true;
    }

    if (message_name == "cache_get_size") {
        CookieManager::HandleGetCacheSize(browser);
        return true;
    }

    // ========== COOKIE BLOCKING MESSAGES ==========

    if (message_name == "cookie_block_domain") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string domain = args->GetString(0).ToString();
        std::string isWildcardStr = (args->GetSize() > 1) ? args->GetString(1).ToString() : "false";
        bool isWildcard = (isWildcardStr == "true");

        bool success = CookieBlockManager::GetInstance().AddBlockedDomain(domain, isWildcard, "user");

        nlohmann::json response;
        response["success"] = success;
        response["domain"] = domain;
        std::string json_str = response.dump();

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("cookie_block_domain_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "cookie_unblock_domain") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string domain = args->GetString(0).ToString();

        bool success = CookieBlockManager::GetInstance().RemoveBlockedDomain(domain);

        nlohmann::json response;
        response["success"] = success;
        response["domain"] = domain;
        std::string json_str = response.dump();

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("cookie_unblock_domain_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "cookie_get_blocklist") {
        std::string json_str = CookieBlockManager::GetInstance().GetBlockedDomains();

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("cookie_blocklist_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "cookie_allow_third_party") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string domain = args->GetString(0).ToString();

        bool success = CookieBlockManager::GetInstance().AddAllowedThirdParty(domain);

        nlohmann::json response;
        response["success"] = success;
        response["domain"] = domain;
        std::string json_str = response.dump();

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("cookie_allow_third_party_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "cookie_remove_third_party_allow") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string domain = args->GetString(0).ToString();

        bool success = CookieBlockManager::GetInstance().RemoveAllowedThirdParty(domain);

        nlohmann::json response;
        response["success"] = success;
        response["domain"] = domain;
        std::string json_str = response.dump();

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("cookie_remove_third_party_allow_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "cookie_get_block_log") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        int limit = 100;
        int offset = 0;
        if (args->GetSize() > 0) {
            try { limit = std::stoi(args->GetString(0).ToString()); } catch (...) {}
        }
        if (args->GetSize() > 1) {
            try { offset = std::stoi(args->GetString(1).ToString()); } catch (...) {}
        }

        std::string json_str = CookieBlockManager::GetInstance().GetBlockLog(limit, offset);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("cookie_block_log_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "cookie_clear_block_log") {
        bool success = CookieBlockManager::GetInstance().ClearBlockLog();

        nlohmann::json response;
        response["success"] = success;
        std::string json_str = response.dump();

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("cookie_clear_block_log_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "cookie_get_blocked_count") {
        // Use per-window active tab's browser ID
        int browser_id = 0;
        BrowserWindow* cookie_cnt_win = GetOwnerWindow();
        int cookie_cnt_wid = cookie_cnt_win ? cookie_cnt_win->window_id : 0;
        auto* active_tab = TabManager::GetInstance().GetActiveTabForWindow(cookie_cnt_wid);
        if (active_tab && active_tab->browser) {
            browser_id = active_tab->browser->GetIdentifier();
        }
        int count = CookieBlockManager::GetInstance().GetBlockedCountForBrowser(browser_id);

        nlohmann::json response;
        response["count"] = count;
        std::string json_str = response.dump();

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("cookie_blocked_count_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "cookie_reset_blocked_count") {
        int browser_id = browser->GetIdentifier();
        CookieBlockManager::GetInstance().ResetBlockedCount(browser_id);

        nlohmann::json response;
        response["success"] = true;
        std::string json_str = response.dump();

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("cookie_reset_blocked_count_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "cookie_check_site_allowed") {
        CefRefPtr<CefListValue> csa_args = message->GetArgumentList();
        std::string domain = (csa_args->GetSize() > 0) ? csa_args->GetString(0).ToString() : "";

        bool allowed = false;
        if (!domain.empty()) {
            allowed = CookieBlockManager::GetInstance().IsThirdPartyAllowed(domain);
        }

        nlohmann::json response;
        response["domain"] = domain;
        response["allowed"] = allowed;
        std::string json_str = response.dump();

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("cookie_check_site_allowed_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    // ========== ADBLOCK MESSAGES (Sprint 8c) ==========

    if (message_name == "adblock_get_blocked_count") {
        // Use per-window active tab's browser ID
        int browser_id = 0;
        BrowserWindow* adblock_cnt_win = GetOwnerWindow();
        int adblock_cnt_wid = adblock_cnt_win ? adblock_cnt_win->window_id : 0;
        auto* active_tab = TabManager::GetInstance().GetActiveTabForWindow(adblock_cnt_wid);
        if (active_tab && active_tab->browser) {
            browser_id = active_tab->browser->GetIdentifier();
        }
        int count = AdblockCache::GetInstance().getBlockedCount(browser_id);

        nlohmann::json response;
        response["count"] = count;

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("adblock_blocked_count_response");
        responseMsg->GetArgumentList()->SetString(0, response.dump());
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "adblock_reset_blocked_count") {
        // Use per-window active tab's browser ID
        int browser_id = 0;
        BrowserWindow* adblock_rst_win = GetOwnerWindow();
        int adblock_rst_wid = adblock_rst_win ? adblock_rst_win->window_id : 0;
        auto* active_tab = TabManager::GetInstance().GetActiveTabForWindow(adblock_rst_wid);
        if (active_tab && active_tab->browser) {
            browser_id = active_tab->browser->GetIdentifier();
        }
        AdblockCache::GetInstance().resetBlockedCount(browser_id);

        nlohmann::json response;
        response["success"] = true;

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("adblock_reset_blocked_count_response");
        responseMsg->GetArgumentList()->SetString(0, response.dump());
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "adblock_site_toggle") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string domain = args->GetString(0).ToString();
        std::string enabledStr = args->GetString(1).ToString();
        bool enabled = (enabledStr == "true" || enabledStr == "1");

        LOG_DEBUG_BROWSER("🛡️ Adblock site toggle: " + domain + " → " + (enabled ? "ON" : "OFF"));

        // Update local JSON-backed cache (no HTTP call needed)
        AdblockCache::GetInstance().setSiteEnabled(domain, enabled);

        // Send response back to renderer
        nlohmann::json response;
        response["domain"] = domain;
        response["adblockEnabled"] = enabled;
        response["success"] = true;

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("adblock_site_toggle_response");
        responseMsg->GetArgumentList()->SetString(0, response.dump());
        if (browser && browser->GetMainFrame()) {
            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        }
        return true;
    }

    // Check adblock site-enabled state (GET equivalent via IPC)
    if (message_name == "adblock_check_site_enabled") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string domain = args->GetString(0).ToString();

        bool enabled = AdblockCache::GetInstance().isSiteEnabled(domain);

        nlohmann::json response;
        response["domain"] = domain;
        response["adblockEnabled"] = enabled;

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("adblock_check_site_enabled_response");
        responseMsg->GetArgumentList()->SetString(0, response.dump());
        if (browser && browser->GetMainFrame()) {
            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        }
        return true;
    }

    // Per-site fingerprint protection toggle — get current state
    if (message_name == "fingerprint_get_site_enabled") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string domain = args->GetString(0).ToString();
        bool enabled = FingerprintProtection::GetInstance().IsSiteEnabled(domain);

        nlohmann::json response;
        response["domain"] = domain;
        response["enabled"] = enabled;

        CefRefPtr<CefProcessMessage> responseMsg =
            CefProcessMessage::Create("fingerprint_get_site_enabled_response");
        responseMsg->GetArgumentList()->SetString(0, response.dump());
        if (browser && browser->GetMainFrame()) {
            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        }
        return true;
    }

    // Per-site fingerprint protection toggle — set state
    if (message_name == "fingerprint_set_site_enabled") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string domain = args->GetString(0).ToString();
        bool enabled = (args->GetString(1).ToString() == "true");
        FingerprintProtection::GetInstance().SetSiteEnabled(domain, enabled);
        return true;
    }

    // Sprint 10b: Per-site scriptlet toggle
    if (message_name == "adblock_scriptlet_toggle") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string domain = args->GetString(0).ToString();
        std::string enabledStr = args->GetString(1).ToString();
        bool enabled = (enabledStr == "true" || enabledStr == "1");

        LOG_DEBUG_BROWSER("💉 Scriptlet toggle: " + domain + " → " + (enabled ? "ON" : "OFF"));

        // Update local JSON-backed cache (no HTTP call needed)
        AdblockCache::GetInstance().setScriptletsEnabled(domain, enabled);

        // Send response back to renderer
        nlohmann::json response;
        response["domain"] = domain;
        response["scriptletsEnabled"] = enabled;
        response["success"] = true;

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("adblock_scriptlet_toggle_response");
        responseMsg->GetArgumentList()->SetString(0, response.dump());
        if (browser && browser->GetMainFrame()) {
            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        }
        return true;
    }

    // Check scriptlet-enabled state (GET equivalent via IPC)
    if (message_name == "adblock_check_scriptlets_enabled") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string domain = args->GetString(0).ToString();

        bool enabled = AdblockCache::GetInstance().isScriptletsEnabled(domain);

        nlohmann::json response;
        response["domain"] = domain;
        response["scriptletsEnabled"] = enabled;

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("adblock_check_scriptlets_enabled_response");
        responseMsg->GetArgumentList()->SetString(0, response.dump());
        if (browser && browser->GetMainFrame()) {
            browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        }
        return true;
    }

    // ========== COSMETIC FILTERING PHASE 2 (Sprint 8e) ==========

    if (message_name == "cosmetic_class_id_query") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string dataJson = args->GetString(0).ToString();

        // Parse JSON: {"url":"...","classes":[...],"ids":[...]}
        try {
            auto data = nlohmann::json::parse(dataJson);
            std::string url = data.value("url", "");
            std::vector<std::string> classes;
            std::vector<std::string> ids;

            if (data.contains("classes") && data["classes"].is_array()) {
                for (const auto& c : data["classes"]) {
                    if (c.is_string()) classes.push_back(c.get<std::string>());
                }
            }
            if (data.contains("ids") && data["ids"].is_array()) {
                for (const auto& id : data["ids"]) {
                    if (id.is_string()) ids.push_back(id.get<std::string>());
                }
            }

            if (!url.empty() && (!classes.empty() || !ids.empty())) {
                std::string selectors = AdblockCache::GetInstance().fetchHiddenIdSelectors(url, classes, ids);

                if (!selectors.empty()) {
                    LOG_DEBUG_BROWSER("🎨 P2: " + std::to_string(selectors.size()) + " chars generic CSS for " + url);

                    // Send selectors back to the tab browser's renderer for injection
                    // The message came from a tab browser, so find its frame
                    auto* activeTab = TabManager::GetInstance().GetActiveTab();
                    if (activeTab && activeTab->browser) {
                        CefRefPtr<CefFrame> tabFrame = activeTab->browser->GetMainFrame();
                        if (tabFrame && tabFrame->IsValid()) {
                            CefRefPtr<CefProcessMessage> cssMsg = CefProcessMessage::Create("inject_cosmetic_css");
                            cssMsg->GetArgumentList()->SetString(0, selectors);
                            tabFrame->SendProcessMessage(PID_RENDERER, cssMsg);
                        }
                    }
                }
            }
        } catch (const std::exception& e) {
            LOG_ERROR_BROWSER("🎨 Phase 2: JSON parse error: " + std::string(e.what()));
        }
        return true;
    }

    // ========== BOOKMARK MESSAGES ==========

    if (message_name == "bookmark_add") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string url = args->GetString(0).ToString();
        std::string title = args->GetString(1).ToString();

        // Parse folder_id: empty string means -1 (root)
        int folder_id = -1;
        if (args->GetSize() > 2) {
            std::string folderStr = args->GetString(2).ToString();
            if (!folderStr.empty()) {
                try { folder_id = std::stoi(folderStr); } catch (...) { folder_id = -1; }
            }
        }

        // Parse tags: JSON array string
        std::vector<std::string> tagsVec;
        if (args->GetSize() > 3) {
            std::string tagsStr = args->GetString(3).ToString();
            if (!tagsStr.empty()) {
                try {
                    nlohmann::json tagsJson = nlohmann::json::parse(tagsStr);
                    if (tagsJson.is_array()) {
                        for (const auto& tag : tagsJson) {
                            if (tag.is_string()) {
                                tagsVec.push_back(tag.get<std::string>());
                            }
                        }
                    }
                } catch (...) {
                    // Invalid JSON, use empty tags
                }
            }
        }

        std::string json_str = BookmarkManager::GetInstance().AddBookmark(url, title, folder_id, tagsVec);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_add_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_get") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        int64_t id = 0;
        try { id = std::stoi(args->GetString(0).ToString()); } catch (...) {}

        std::string json_str = BookmarkManager::GetInstance().GetBookmark(id);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_get_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_update") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        int64_t id = 0;
        try { id = std::stoi(args->GetString(0).ToString()); } catch (...) {}

        // Parse fields JSON object
        std::string title = "";
        std::string url = "";
        int folder_id = -1;
        std::vector<std::string> tagsVec;
        bool hasTitle = false, hasUrl = false, hasFolderId = false, hasTags = false;

        if (args->GetSize() > 1) {
            std::string fieldsStr = args->GetString(1).ToString();
            if (!fieldsStr.empty()) {
                try {
                    nlohmann::json fields = nlohmann::json::parse(fieldsStr);
                    if (fields.contains("title") && fields["title"].is_string()) {
                        title = fields["title"].get<std::string>();
                        hasTitle = true;
                    }
                    if (fields.contains("url") && fields["url"].is_string()) {
                        url = fields["url"].get<std::string>();
                        hasUrl = true;
                    }
                    if (fields.contains("folderId")) {
                        if (fields["folderId"].is_null()) {
                            folder_id = -1;
                        } else if (fields["folderId"].is_number()) {
                            folder_id = fields["folderId"].get<int>();
                        }
                        hasFolderId = true;
                    }
                    if (fields.contains("tags") && fields["tags"].is_array()) {
                        for (const auto& tag : fields["tags"]) {
                            if (tag.is_string()) {
                                tagsVec.push_back(tag.get<std::string>());
                            }
                        }
                        hasTags = true;
                    }
                } catch (...) {
                    // Invalid JSON fields
                }
            }
        }

        // If fields not provided, get current bookmark data to fill in defaults
        std::string json_str;
        if (!hasTitle || !hasUrl || !hasFolderId || !hasTags) {
            // Get current bookmark to fill in missing fields
            std::string currentJson = BookmarkManager::GetInstance().GetBookmark(id);
            try {
                nlohmann::json current = nlohmann::json::parse(currentJson);
                if (current.contains("id")) {
                    if (!hasTitle) title = current.value("title", "");
                    if (!hasUrl) url = current.value("url", "");
                    if (!hasFolderId) {
                        if (current["folder_id"].is_null()) {
                            folder_id = -1;
                        } else {
                            folder_id = current.value("folder_id", -1);
                        }
                    }
                    if (!hasTags && current.contains("tags") && current["tags"].is_array()) {
                        for (const auto& tag : current["tags"]) {
                            if (tag.is_string()) {
                                tagsVec.push_back(tag.get<std::string>());
                            }
                        }
                    }
                }
            } catch (...) {
                // Could not parse current bookmark - will proceed with defaults
            }
        }

        json_str = BookmarkManager::GetInstance().UpdateBookmark(id, title, url, folder_id, tagsVec);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_update_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_remove") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        int64_t id = 0;
        try { id = std::stoi(args->GetString(0).ToString()); } catch (...) {}

        std::string json_str = BookmarkManager::GetInstance().RemoveBookmark(id);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_remove_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_search") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string query = args->GetString(0).ToString();
        int limit = 50;
        int offset = 0;
        if (args->GetSize() > 1) {
            try { limit = std::stoi(args->GetString(1).ToString()); } catch (...) {}
        }
        if (args->GetSize() > 2) {
            try { offset = std::stoi(args->GetString(2).ToString()); } catch (...) {}
        }

        std::string json_str = BookmarkManager::GetInstance().SearchBookmarks(query, limit, offset);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_search_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_get_all") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        int folder_id = -1;
        int limit = 50;
        int offset = 0;
        if (args->GetSize() > 0) {
            std::string folderStr = args->GetString(0).ToString();
            if (!folderStr.empty()) {
                try { folder_id = std::stoi(folderStr); } catch (...) { folder_id = -1; }
            }
        }
        if (args->GetSize() > 1) {
            try { limit = std::stoi(args->GetString(1).ToString()); } catch (...) {}
        }
        if (args->GetSize() > 2) {
            try { offset = std::stoi(args->GetString(2).ToString()); } catch (...) {}
        }

        std::string json_str = BookmarkManager::GetInstance().GetAllBookmarks(folder_id, limit, offset);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_get_all_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_is_bookmarked") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string url = args->GetString(0).ToString();

        std::string json_str = BookmarkManager::GetInstance().IsBookmarked(url);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_is_bookmarked_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_get_all_tags") {
        std::string json_str = BookmarkManager::GetInstance().GetAllTags();

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_get_all_tags_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_update_last_accessed") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        int64_t id = 0;
        try { id = std::stoi(args->GetString(0).ToString()); } catch (...) {}

        std::string json_str = BookmarkManager::GetInstance().UpdateLastAccessed(id);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_update_last_accessed_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_folder_create") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string name = args->GetString(0).ToString();
        int parent_id = -1;
        if (args->GetSize() > 1) {
            std::string parentStr = args->GetString(1).ToString();
            if (!parentStr.empty() && parentStr != "-1") {
                try { parent_id = std::stoi(parentStr); } catch (...) { parent_id = -1; }
            }
        }

        std::string json_str = BookmarkManager::GetInstance().CreateFolder(name, parent_id);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_folder_create_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_folder_list") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        int parent_id = -1;
        if (args->GetSize() > 0) {
            std::string parentStr = args->GetString(0).ToString();
            if (!parentStr.empty() && parentStr != "-1") {
                try { parent_id = std::stoi(parentStr); } catch (...) { parent_id = -1; }
            }
        }

        std::string json_str = BookmarkManager::GetInstance().ListFolders(parent_id);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_folder_list_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_folder_update") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        int64_t id = 0;
        try { id = std::stoi(args->GetString(0).ToString()); } catch (...) {}
        std::string name = (args->GetSize() > 1) ? args->GetString(1).ToString() : "";

        std::string json_str = BookmarkManager::GetInstance().UpdateFolder(id, name);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_folder_update_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_folder_remove") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        int64_t id = 0;
        try { id = std::stoi(args->GetString(0).ToString()); } catch (...) {}

        std::string json_str = BookmarkManager::GetInstance().RemoveFolder(id);

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_folder_remove_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    if (message_name == "bookmark_folder_get_tree") {
        std::string json_str = BookmarkManager::GetInstance().GetFolderTree();

        CefRefPtr<CefProcessMessage> responseMsg = CefProcessMessage::Create("bookmark_folder_get_tree_response");
        responseMsg->GetArgumentList()->SetString(0, json_str);
        browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, responseMsg);
        return true;
    }

    // ========== DOWNLOAD CONTROL MESSAGES ==========

    if (message_name == "download_cancel") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        uint32_t dl_id = 0;
        try { dl_id = static_cast<uint32_t>(std::stoul(args->GetString(0).ToString())); } catch (...) {}
        auto it = active_downloads_.find(dl_id);
        if (it != active_downloads_.end() && it->second.item_callback) {
            it->second.item_callback->Cancel();
            LOG_INFO_BROWSER("📥 Download canceled: id=" + std::to_string(dl_id));
        }
        return true;
    }

    if (message_name == "download_pause") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        uint32_t dl_id = 0;
        try { dl_id = static_cast<uint32_t>(std::stoul(args->GetString(0).ToString())); } catch (...) {}
        auto it = active_downloads_.find(dl_id);
        if (it != active_downloads_.end() && it->second.item_callback) {
            it->second.item_callback->Pause();
            paused_downloads_.insert(dl_id);
            it->second.is_paused = true;
            it->second.is_in_progress = false;
            NotifyDownloadStateChanged();
            LOG_INFO_BROWSER("📥 Download paused: id=" + std::to_string(dl_id));
        }
        return true;
    }

    if (message_name == "download_resume") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        uint32_t dl_id = 0;
        try { dl_id = static_cast<uint32_t>(std::stoul(args->GetString(0).ToString())); } catch (...) {}
        auto it = active_downloads_.find(dl_id);
        if (it != active_downloads_.end() && it->second.item_callback) {
            it->second.item_callback->Resume();
            paused_downloads_.erase(dl_id);
            it->second.is_paused = false;
            it->second.is_in_progress = true;
            NotifyDownloadStateChanged();
            LOG_INFO_BROWSER("📥 Download resumed: id=" + std::to_string(dl_id));
        }
        return true;
    }

    if (message_name == "download_open") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        uint32_t dl_id = 0;
        try { dl_id = static_cast<uint32_t>(std::stoul(args->GetString(0).ToString())); } catch (...) {}
        auto it = active_downloads_.find(dl_id);
        if (it != active_downloads_.end() && !it->second.full_path.empty()) {
#ifdef _WIN32
            std::wstring wpath(it->second.full_path.begin(), it->second.full_path.end());
            ShellExecuteW(NULL, L"open", wpath.c_str(), NULL, NULL, SW_SHOWNORMAL);
            LOG_INFO_BROWSER("📥 Opening downloaded file: " + it->second.full_path);
#elif defined(__APPLE__)
            // TODO(macOS): NSWorkspace openFile
            LOG_INFO_BROWSER("📥 download_open not yet implemented on macOS");
#endif
        }
        return true;
    }

    if (message_name == "download_show_folder") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        uint32_t dl_id = 0;
        try { dl_id = static_cast<uint32_t>(std::stoul(args->GetString(0).ToString())); } catch (...) {}
        auto it = active_downloads_.find(dl_id);
        if (it != active_downloads_.end() && !it->second.full_path.empty()) {
#ifdef _WIN32
            // Extract parent directory
            std::string path = it->second.full_path;
            size_t last_sep = path.find_last_of("\\/");
            std::string dir = (last_sep != std::string::npos) ? path.substr(0, last_sep) : path;
            std::wstring wdir(dir.begin(), dir.end());
            ShellExecuteW(NULL, L"open", wdir.c_str(), NULL, NULL, SW_SHOWNORMAL);
            LOG_INFO_BROWSER("📥 Showing folder: " + dir);
#elif defined(__APPLE__)
            // TODO(macOS): NSWorkspace selectFile:inFileViewerRootedAtPath:
            LOG_INFO_BROWSER("📥 download_show_folder not yet implemented on macOS");
#endif
        }
        return true;
    }

    if (message_name == "download_clear_completed") {
        auto it = active_downloads_.begin();
        while (it != active_downloads_.end()) {
            if (it->second.is_complete || it->second.is_canceled) {
                it = active_downloads_.erase(it);
            } else {
                ++it;
            }
        }
        NotifyDownloadStateChanged();
        LOG_INFO_BROWSER("📥 Cleared completed downloads, remaining: " + std::to_string(active_downloads_.size()));
        return true;
    }

    if (message_name == "download_get_state") {
        NotifyDownloadStateChanged();
        return true;
    }

    if (message_name == "download_panel_show") {
        int iconRightOffset = 0;
        CefRefPtr<CefListValue> dp_args = message->GetArgumentList();
        if (dp_args->GetSize() > 0) {
            try { iconRightOffset = std::stoi(dp_args->GetString(0).ToString()); } catch(...) {}
        }

#ifdef _WIN32
        extern void CreateDownloadPanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void ShowDownloadPanelOverlay(int iconRightOffset, BrowserWindow* targetWin = nullptr);
        extern HWND g_download_panel_overlay_hwnd;
        extern HINSTANCE g_hInstance;

        if (!g_download_panel_overlay_hwnd || !IsWindow(g_download_panel_overlay_hwnd)) {
            CreateDownloadPanelOverlay(g_hInstance, true, iconRightOffset);
        } else {
            ShowDownloadPanelOverlay(iconRightOffset, GetOwnerWindow());
        }

        // Push current state to the overlay
        NotifyDownloadStateChanged();

        LOG_DEBUG_BROWSER("📥 Download panel overlay shown with iconRightOffset=" + std::to_string(iconRightOffset));
#elif defined(__APPLE__)
        extern void CreateDownloadPanelOverlayMacOS(int iconRightOffset);
        extern void ShowDownloadPanelOverlayMacOS(int iconRightOffset);
        extern void HideDownloadPanelOverlayMacOS();
        extern bool IsDownloadPanelOverlayVisible();
        extern bool WasDownloadPanelJustHidden();

        if (IsDownloadPanelOverlayVisible() || WasDownloadPanelJustHidden()) {
            if (IsDownloadPanelOverlayVisible()) {
                HideDownloadPanelOverlayMacOS();
            }
            return true;
        }
        CreateDownloadPanelOverlayMacOS(iconRightOffset);
        NotifyDownloadStateChanged();
        LOG_DEBUG_BROWSER("Download panel overlay shown (macOS) iconRightOffset=" + std::to_string(iconRightOffset));
#endif
        return true;
    }

    // ========== FIND IN PAGE (JavaScript-based) ==========
    // CEF's CefBrowserHost::Find() / OnFindResult API does not callback in this
    // build (GetFindHandler never queried). Use window.find() + JS match counting.

    if (message_name == "find_text") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() >= 4) {
            std::string query = args->GetString(0).ToString();
            bool forward = args->GetBool(1);
            bool matchCase = args->GetBool(2);
            bool findNext = args->GetBool(3);

            LOG_DEBUG_BROWSER("🔍 find_text: query='" + query + "' forward=" + std::to_string(forward) +
                              " findNext=" + std::to_string(findNext));

            BrowserWindow* find_win = GetOwnerWindow();
            int find_wid = find_win ? find_win->window_id : 0;
            auto* active_tab = TabManager::GetInstance().GetActiveTabForWindow(find_wid);
            if (active_tab && active_tab->browser) {
                CefRefPtr<CefFrame> frame = active_tab->browser->GetMainFrame();
                if (frame) {
                    if (query.empty()) {
                        // Clear selection/highlights
                        frame->ExecuteJavaScript(
                            "window.getSelection().removeAllRanges();",
                            frame->GetURL(), 0);
                    } else {
                        // Escape query for embedding in JS string literal
                        std::string escaped;
                        for (char c : query) {
                            if (c == '\\') escaped += "\\\\";
                            else if (c == '\'') escaped += "\\'";
                            else if (c == '\n') escaped += "\\n";
                            else if (c == '\r') escaped += "\\r";
                            else escaped += c;
                        }

                        // JavaScript find-in-page implementation:
                        // 1. Inject CSS for yellow selection highlight
                        // 2. Count matches with wrapAround=FALSE (stops at document end)
                        // 3. Navigate with window.find() using wrapAround=true
                        std::string js = "(function() {"
                            "var q = '" + escaped + "';"
                            "var cs = " + (matchCase ? "true" : "false") + ";"
                            "var bk = " + (forward ? "false" : "true") + ";"
                            "var fn = " + (findNext ? "true" : "false") + ";"

                            // Inject yellow highlight CSS once
                            "if (!document.__hodosFindStyle) {"
                            "  var s = document.createElement('style');"
                            "  s.textContent = '::selection { background: #FFFF00 !important; color: #000 !important; }';"
                            "  document.head.appendChild(s);"
                            "  document.__hodosFindStyle = s;"
                            "}"

                            // New search: count matches and go to first
                            "if (!fn) {"
                            // Count by walking forward with wrapAround=FALSE
                            "  window.getSelection().removeAllRanges();"
                            "  window.getSelection().collapse(document.body, 0);"
                            "  var c = 0;"
                            "  while (window.find(q, cs, false, false, false, true, false)) {"
                            "    c++;"
                            "    if (c > 10000) break;"
                            "  }"
                            "  window.__hodosFindCount = c;"
                            // Navigate to first match
                            "  window.getSelection().removeAllRanges();"
                            "  window.getSelection().collapse(document.body, 0);"
                            "  if (c > 0) {"
                            "    window.find(q, cs, false, false, false, true, false);"
                            "    window.__hodosFindOrdinal = 1;"
                            "  } else {"
                            "    window.__hodosFindOrdinal = 0;"
                            "  }"
                            "} else {"
                            // findNext: navigate forward/backward with wrapAround=true
                            "  var found = window.find(q, cs, bk, true, false, true, false);"
                            "  if (found && window.__hodosFindCount > 0) {"
                            "    if (bk) {"
                            "      window.__hodosFindOrdinal--;"
                            "      if (window.__hodosFindOrdinal < 1) window.__hodosFindOrdinal = window.__hodosFindCount;"
                            "    } else {"
                            "      window.__hodosFindOrdinal++;"
                            "      if (window.__hodosFindOrdinal > window.__hodosFindCount) window.__hodosFindOrdinal = 1;"
                            "    }"
                            "  }"
                            "}"

                            "var cnt = window.__hodosFindCount || 0;"
                            "var ord = window.__hodosFindOrdinal || 0;"
                            "if (window.cefMessage) {"
                            "  window.cefMessage.send('find_result_js', [cnt, ord]);"
                            "}"
                            "})();";
                        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
                    }
                }
            }
        }
        return true;
    }

    // Handle find results from tab's JavaScript (forwarded from tab browser)
    if (message_name == "find_result_js") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() >= 2) {
            int count = args->GetInt(0);
            int ordinal = args->GetInt(1);
            LOG_DEBUG_BROWSER("🔍 find_result_js: count=" + std::to_string(count) +
                              " ordinal=" + std::to_string(ordinal));

            // Forward to header browser as find_result
            nlohmann::json result;
            result["count"] = count;
            result["activeMatchOrdinal"] = ordinal;
            result["finalUpdate"] = true;
            std::string json_str = result.dump();

            CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
            if (header) {
                CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("find_result");
                msg->GetArgumentList()->SetString(0, json_str);
                header->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
            }
        }
        return true;
    }

    if (message_name == "find_stop") {
        BrowserWindow* fstop_win = GetOwnerWindow();
        int fstop_wid = fstop_win ? fstop_win->window_id : 0;
        auto* active_tab = TabManager::GetInstance().GetActiveTabForWindow(fstop_wid);
        if (active_tab && active_tab->browser) {
            CefRefPtr<CefFrame> frame = active_tab->browser->GetMainFrame();
            if (frame) {
                frame->ExecuteJavaScript(
                    "window.getSelection().removeAllRanges();"
                    "if (document.__hodosFindStyle) {"
                    "  document.__hodosFindStyle.remove();"
                    "  document.__hodosFindStyle = null;"
                    "}"
                    "delete window.__hodosFindCount;"
                    "delete window.__hodosFindOrdinal;",
                    frame->GetURL(), 0);
            }
        }
        return true;
    }

    if (message_name == "download_panel_hide") {
#ifdef _WIN32
        extern void HideDownloadPanelOverlay();
        HideDownloadPanelOverlay();
        LOG_DEBUG_BROWSER("📥 Download panel overlay hidden");
#elif defined(__APPLE__)
        extern void HideDownloadPanelOverlayMacOS();
        HideDownloadPanelOverlayMacOS();
        LOG_DEBUG_BROWSER("Download panel overlay hidden (macOS)");
#endif
        return true;
    }

    return false;
}

CefRefPtr<CefRequestHandler> SimpleHandler::GetRequestHandler() {
    return this;
}

// OnBeforeBrowse fires on the UI thread BEFORE the renderer creates a new V8
// context, so we can pre-cache scriptlets for the navigation target URL.
// When the renderer's OnContextCreated fires, it checks s_scriptCache and
// injects the scripts synchronously before any page JS runs.
// This complements the existing OnLoadingStateChange pre-cache (which often
// fails for initial navigations because mainFrame->GetURL() returns empty).
bool SimpleHandler::OnBeforeBrowse(CefRefPtr<CefBrowser> browser,
                                    CefRefPtr<CefFrame> frame,
                                    CefRefPtr<CefRequest> request,
                                    bool user_gesture,
                                    bool is_redirect) {
    CEF_REQUIRE_UI_THREAD();

    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id == -1) return false;  // Only pre-cache for tab browsers

#ifdef _WIN32
    if (g_adblockServerRunning && AdblockCache::GetInstance().IsGlobalEnabled() && frame->IsMain()) {
        std::string navUrl = request->GetURL().ToString();
        if (!navUrl.empty() && !shouldSkipAdblockCheck(navUrl)) {
            // Sprint 10b: Check if scriptlets are disabled for this domain
            bool skipScriptlets = !AdblockCache::GetInstance().isScriptletsEnabled(navUrl);
            auto cosmetic = AdblockCache::GetInstance().fetchCosmeticResources(navUrl, skipScriptlets);
            if (!cosmetic.injectedScript.empty()) {
                LOG_INFO_BROWSER("💉 OnBeforeBrowse: pre-caching scriptlets for " + navUrl +
                    " (" + std::to_string(cosmetic.injectedScript.size()) + " chars)");
                CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("preload_cosmetic_script");
                CefRefPtr<CefListValue> args = msg->GetArgumentList();
                args->SetString(0, navUrl);
                args->SetString(1, cosmetic.injectedScript);
                frame->SendProcessMessage(PID_RENDERER, msg);
            }
        }
    }
#endif

    // Sprint 12c: Send fingerprint seed to renderer for this navigation
    if (frame->IsMain() && FingerprintProtection::GetInstance().IsEnabled()) {
        std::string navUrl = request->GetURL().ToString();
        if (!navUrl.empty() && navUrl.find("127.0.0.1") == std::string::npos &&
            navUrl.find("localhost") == std::string::npos) {

            // Extract domain for per-site check (mirrors FingerprintProtection::ExtractDomain)
            std::string domain;
            {
                size_t start = navUrl.find("://");
                if (start != std::string::npos) {
                    start += 3;
                    size_t end = navUrl.find_first_of(":/", start);
                    if (end == std::string::npos) end = navUrl.size();
                    domain = navUrl.substr(start, end - start);
                } else {
                    domain = navUrl;
                }
            }

            if (FingerprintProtection::IsAuthDomain(navUrl) ||
                !FingerprintProtection::GetInstance().IsSiteEnabled(domain)) {
                // Send disable signal to renderer — skip fingerprint injection for this URL
                CefRefPtr<CefProcessMessage> msg =
                    CefProcessMessage::Create("fingerprint_site_disabled");
                msg->GetArgumentList()->SetString(0, navUrl);
                frame->SendProcessMessage(PID_RENDERER, msg);
            } else {
                // Normal path: send seed for farbling
                uint32_t seed = FingerprintProtection::GetInstance().GetDomainSeed(navUrl);
                CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("fingerprint_seed");
                msg->GetArgumentList()->SetInt(0, static_cast<int>(seed));
                msg->GetArgumentList()->SetString(1, navUrl);
                frame->SendProcessMessage(PID_RENDERER, msg);
            }
        }
    }

    return false;  // Never cancel navigation
}

// ============================================================================
// AdblockResponseFilter — strips ad-related JSON keys from YouTube responses
// ============================================================================
//
// Applied to YouTube API (application/json) and HTML (text/html) responses.
// Buffers the complete response body, renames ad-configuration keys
// (e.g. "adPlacements" → "adPlacements_"), then outputs the modified data.
// YouTube's player JS looks for the original key names and doesn't find them,
// so ads never load.
//
// This operates at the network level (CefResponseFilter runs in the browser
// process IO thread, before any renderer/JavaScript sees the data), solving
// the scriptlet injection timing problem where scriptlets injected via
// OnLoadingStateChange arrive after YouTube's inline scripts have already
// processed ad configuration from ytInitialPlayerResponse.
class AdblockResponseFilter : public CefResponseFilter {
public:
    AdblockResponseFilter() = default;

    bool InitFilter() override { return true; }

    FilterStatus Filter(void* data_in,
                        size_t data_in_size,
                        size_t& data_in_read,
                        void* data_out,
                        size_t data_out_size,
                        size_t& data_out_written) override {
        // Always consume all available input
        data_in_read = data_in_size;

        if (data_in_size > 0) {
            // Accumulate response body — produce no output yet
            buffer_.append(static_cast<const char*>(data_in), data_in_size);
            data_out_written = 0;
            return RESPONSE_FILTER_NEED_MORE_DATA;
        }

        // data_in_size == 0 → entire response body received
        if (!processed_) {
            processBuffer();
            processed_ = true;
            write_offset_ = 0;
        }

        // Stream processed output back to CEF
        size_t remaining = buffer_.size() - write_offset_;
        size_t to_write = (remaining < data_out_size) ? remaining : data_out_size;
        if (to_write > 0) {
            memcpy(data_out, buffer_.data() + write_offset_, to_write);
        }
        data_out_written = to_write;
        write_offset_ += to_write;

        return (write_offset_ >= buffer_.size())
            ? RESPONSE_FILTER_DONE
            : RESPONSE_FILTER_NEED_MORE_DATA;
    }

private:
    void processBuffer() {
        // Rename ad-related JSON keys so YouTube's player JS can't find them.
        // The replacement adds an underscore to the key name, preserving JSON
        // validity while making the key invisible to YouTube's ad rendering code.
        struct Replacement {
            const char* find;
            const char* replace;
        };
        static const Replacement replacements[] = {
            { "\"adPlacements\":",           "\"adPlacements_\":" },
            { "\"playerAds\":",              "\"playerAds_\":" },
            { "\"adSlots\":",                "\"adSlots_\":" },
            { "\"adBreakParams\":",          "\"adBreakParams_\":" },
            { "\"adBreakHeartbeatParams\":", "\"adBreakHeartbeatParams_\":" },
        };

        int total = 0;
        for (const auto& r : replacements) {
            std::string f(r.find);
            std::string rep(r.replace);
            size_t pos = 0;
            while ((pos = buffer_.find(f, pos)) != std::string::npos) {
                buffer_.replace(pos, f.length(), rep);
                pos += rep.length();
                total++;
            }
        }

        if (total > 0) {
            LOG_DEBUG_BROWSER("🛡️ AdblockResponseFilter: " +
                std::to_string(total) + " ad keys renamed in " +
                std::to_string(buffer_.size()) + " byte response");
        }
    }

    std::string buffer_;
    size_t write_offset_ = 0;
    bool processed_ = false;

    IMPLEMENT_REFCOUNTING(AdblockResponseFilter);
};

// CookieFilterResourceHandler - Returns CookieAccessFilterWrapper for non-wallet requests
// so cookie blocking applies to all browsing. Also returns AdblockResponseFilter
// for YouTube responses to strip ad configuration at the network level.
class CookieFilterResourceHandler : public CefResourceRequestHandler {
public:
    CefRefPtr<CefCookieAccessFilter> GetCookieAccessFilter(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefFrame> frame,
        CefRefPtr<CefRequest> request) override {
        if (CookieBlockManager::GetInstance().IsInitialized()) {
            return new CookieAccessFilterWrapper();
        }
        return nullptr;
    }

    CefRefPtr<CefResponseFilter> GetResourceResponseFilter(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefFrame> frame,
        CefRefPtr<CefRequest> request,
        CefRefPtr<CefResponse> response) override {
        if (!g_adblockServerRunning || !AdblockCache::GetInstance().IsGlobalEnabled()) return nullptr;

        std::string url = request->GetURL().ToString();

        // Check YouTube host — use scheme+host prefix to avoid false positives
        // (e.g. accounts.google.com/...?continue=youtube.com in query params)
        bool isYouTube = (url.find("://www.youtube.com/") != std::string::npos ||
                          url.find("://youtube.com/") != std::string::npos ||
                          url.find("://m.youtube.com/") != std::string::npos);
        if (!isYouTube) return nullptr;

        std::string contentType = response->GetHeaderByName("Content-Type").ToString();

        // YouTube API responses (JSON) — /youtubei/v1/player, /next, /get_watch, etc.
        if (url.find("/youtubei/") != std::string::npos &&
            contentType.find("application/json") != std::string::npos) {
            LOG_DEBUG_BROWSER("🛡️ Response filter: YouTube API " + url);
            return new AdblockResponseFilter();
        }

        // YouTube main page HTML — contains inline ytInitialPlayerResponse with ad data.
        // Only filter top-level navigations (RT_MAIN_FRAME), not iframes, tracking
        // pixels (/ptracking), live chat embeds, or other sub-resources.
        if (request->GetResourceType() == RT_MAIN_FRAME &&
            contentType.find("text/html") != std::string::npos) {
            LOG_DEBUG_BROWSER("🛡️ Response filter: YouTube HTML " + url);
            return new AdblockResponseFilter();
        }
        return nullptr;
    }

    IMPLEMENT_REFCOUNTING(CookieFilterResourceHandler);
};

CefRefPtr<CefResourceRequestHandler> SimpleHandler::GetResourceRequestHandler(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefRefPtr<CefRequest> request,
    bool is_navigation,
    bool is_download,
    const CefString& request_initiator,
    bool& disable_default_handling) {

    CEF_REQUIRE_IO_THREAD();

    std::string url = request->GetURL().ToString();
    std::string method = request->GetMethod().ToString();
    std::string connection = request->GetHeaderByName("Connection");
    std::string upgrade = request->GetHeaderByName("Upgrade");

    LOG_DEBUG_BROWSER("🌐 Resource request: " + url + " (role: " + role_ + ")");
    LOG_DEBUG_BROWSER("🌐 Method: " + method + ", Connection: " + connection + ", Upgrade: " + upgrade);

    // Production frontend serving: if frontend/ exists next to .exe,
    // serve files from disk instead of hitting the Vite dev server.
    // MUST be before adblock/cookie checks — those handlers would try to
    // network-fetch from port 5137, which has no server in production.
    {
        std::string frontend_dir;
        if (url.find("127.0.0.1:5137") != std::string::npos &&
            IsFrontendAvailable(frontend_dir)) {
            return new LocalFileResourceRequestHandler(frontend_dir, url);
        }
    }

    // Trusted internal overlays (wallet, settings, backup) talking directly to the Rust wallet
    // bypass ALL resource handlers — let CEF's native network stack handle them.
    // This avoids CefURLRequest forwarding issues on macOS.
    if (url.find("127.0.0.1:31301") != std::string::npos &&
        (role_ == "wallet" || role_ == "wallet_panel" || role_ == "settings" || role_ == "backup")) {
        LOG_DEBUG_BROWSER("🔒 Trusted overlay direct wallet request — bypassing all handlers");
        return nullptr;
    }

    // Sprint 11b: Inject Do Not Track headers if enabled
    if (SettingsManager::GetInstance().GetPrivacySettings().doNotTrack) {
        request->SetHeaderByName("DNT", "1", true);
        request->SetHeaderByName("Sec-GPC", "1", true);
    }

    // Ad & tracker blocking — check before wallet interception
    // Only for tab browsers making external requests (skip internal/overlay URLs)
    if (g_adblockServerRunning && AdblockCache::GetInstance().IsGlobalEnabled() && !shouldSkipAdblockCheck(url)) {
        std::string sourceUrl;
        if (frame && frame->GetURL().length() > 0) {
            sourceUrl = frame->GetURL().ToString();
        }

        // Check per-site toggle: extract domain from source URL
        bool siteAdblockEnabled = true;
        if (!sourceUrl.empty()) {
            std::string domain;
            size_t start = sourceUrl.find("://");
            if (start != std::string::npos) {
                start += 3;
                size_t end = sourceUrl.find_first_of(":/", start);
                domain = sourceUrl.substr(start, end - start);
            }
            if (!domain.empty()) {
                siteAdblockEnabled = AdblockCache::GetInstance().isSiteEnabled(domain);
            }
        }

        if (siteAdblockEnabled) {
            const char* resourceType = CefResourceTypeToAdblock(request->GetResourceType());
            bool adblockBlocked = AdblockCache::GetInstance().check(url, sourceUrl, resourceType);
            if (adblockBlocked) {
                LOG_DEBUG_BROWSER("🛡️ Blocked by adblock: " + url);
                if (browser) {
                    AdblockCache::GetInstance().incrementBlockedCount(browser->GetIdentifier());
                }
                return new AdblockBlockHandler();
            }
        }
    }

    // Intercept HTTP requests for all browsers when they're making external requests
    // Check if the request is to localhost ports that BRC-100 sites commonly use
    // OR if it's a BRC-104 /.well-known/auth request (standard wallet authentication endpoint)
    if (url.find("localhost:31301") != std::string::npos ||
        url.find("localhost:3321") != std::string::npos ||
        url.find("localhost:2121") != std::string::npos ||
        url.find("localhost:8080") != std::string::npos ||
        url.find("messagebox.babbage.systems") != std::string::npos ||
        url.find("/.well-known/auth") != std::string::npos) {
        LOG_DEBUG_BROWSER("🌐 Intercepting wallet request from browser role: " + role_);
        return new HttpRequestInterceptor();
    }

    // For non-wallet requests, return CookieFilterResourceHandler
    // to apply cookie blocking and ad response filtering (YouTube API/HTML stripping)
    {
        bool needsCookieFilter = CookieBlockManager::GetInstance().IsInitialized();
        bool needsResponseFilter = g_adblockServerRunning;
        if (needsCookieFilter || needsResponseFilter) {
            return new CookieFilterResourceHandler();
        }
    }

    return nullptr;
}

CefRefPtr<CefContextMenuHandler> SimpleHandler::GetContextMenuHandler() {
    return this;
}

CefRefPtr<CefDialogHandler> SimpleHandler::GetDialogHandler() {
    return this;
}

CefRefPtr<CefKeyboardHandler> SimpleHandler::GetKeyboardHandler() {
    return this;
}

CefRefPtr<CefPermissionHandler> SimpleHandler::GetPermissionHandler() {
    return this;
}

bool SimpleHandler::OnFileDialog(CefRefPtr<CefBrowser> browser,
                                  FileDialogMode mode,
                                  const CefString& title,
                                  const CefString& default_file_path,
                                  const std::vector<CefString>& accept_filters,
                                  const std::vector<CefString>& accept_extensions,
                                  const std::vector<CefString>& accept_descriptions,
                                  CefRefPtr<CefFileDialogCallback> callback) {
    // Set the file dialog guard to prevent WM_ACTIVATEAPP from closing overlays
    // while the native file dialog is open (it temporarily steals window activation).
    extern bool g_file_dialog_active;
    g_file_dialog_active = true;
    LOG_DEBUG_BROWSER("📂 File dialog requested - setting guard flag");

    // Return false to let CEF show the default native file dialog.
    // The guard flag will be cleared when the app regains focus (WM_ACTIVATEAPP wParam=TRUE).
    return false;
}

// Forward declaration — defined in context menu section below
static void CreateNewTabWithUrl(const std::string& url);

bool SimpleHandler::OnPreKeyEvent(CefRefPtr<CefBrowser> browser,
                                  const CefKeyEvent& event,
                                  CefEventHandle os_event,
                                  bool* is_keyboard_shortcut) {
    // Log keyboard events for debugging
    LOG_DEBUG_BROWSER("⌨️ OnPreKeyEvent [" + role_ + "] - type: " + std::to_string(event.type) +
                      ", key: " + std::to_string(event.windows_key_code) +
                      ", modifiers: " + std::to_string(event.modifiers));

    // Handle DevTools keyboard shortcuts for all windows
    if (event.type == KEYEVENT_RAWKEYDOWN) {
        // F12 - universal DevTools shortcut (cross-platform)
        // Key code 123 is F12 on both Windows and macOS
        if (event.windows_key_code == 123) {
            ShowOrFocusDevTools(browser);
            return true; // Consume the event
        }

        // Ctrl+F / Cmd+F - Find in Page (tab browsers only)
        if (event.windows_key_code == 'F' && role_.find("tab_") == 0) {
#ifdef __APPLE__
            if (event.modifiers & EVENTFLAG_COMMAND_DOWN) {
#else
            if (event.modifiers & EVENTFLAG_CONTROL_DOWN) {
#endif
                // Send find_show to header browser so React can show the find bar
                CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
                if (header) {
                    CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("find_show");
                    header->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
                }
                // Move CEF focus to header browser so keyboard input reaches the find bar
                if (header) {
                    header->GetHost()->SetFocus(true);
                }
                return true;
            }
        }

        // Ctrl+L / Cmd+L — Focus address bar
        if (event.windows_key_code == 'L') {
#ifdef __APPLE__
            if (event.modifiers & EVENTFLAG_COMMAND_DOWN) {
#else
            if (event.modifiers & EVENTFLAG_CONTROL_DOWN) {
#endif
                LOG_INFO_BROWSER("⌨️ Ctrl+L: Focus address bar");
                CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
                if (header) {
                    CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("focus_address_bar");
                    header->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
                    header->GetHost()->SetFocus(true);
                }
                return true;
            }
        }

        // Check for 'I' key shortcuts
        if (event.windows_key_code == 'I') {
#ifdef __APPLE__
            // macOS: Cmd+Option+I
            if ((event.modifiers & EVENTFLAG_COMMAND_DOWN) && (event.modifiers & EVENTFLAG_ALT_DOWN)) {
                ShowOrFocusDevTools(browser);
                return true; // Consume the event
            }
#endif
#ifdef _WIN32
            // Windows: Ctrl+Shift+I
            if ((event.modifiers & EVENTFLAG_CONTROL_DOWN) && (event.modifiers & EVENTFLAG_SHIFT_DOWN)) {
                ShowOrFocusDevTools(browser);
                return true; // Consume the event
            }
#endif
        }

        // Ctrl+N / Cmd+N — New window
        if (event.windows_key_code == 'N') {
#ifdef __APPLE__
            if (event.modifiers & EVENTFLAG_COMMAND_DOWN) {
#else
            if (event.modifiers & EVENTFLAG_CONTROL_DOWN) {
#endif
                LOG_INFO_BROWSER("⌨️ Ctrl+N: Creating new window");
                WindowManager::GetInstance().CreateFullWindow();
                return true;
            }
        }

        // Ctrl+T / Cmd+T — New tab (intercept chrome://newtab)
        if (event.windows_key_code == 'T') {
#ifdef __APPLE__
            if (event.modifiers & EVENTFLAG_COMMAND_DOWN) {
#else
            if (event.modifiers & EVENTFLAG_CONTROL_DOWN) {
#endif
                LOG_INFO_BROWSER("⌨️ Ctrl+T: Creating new tab");
                CreateNewTabWithUrl("");  // Empty URL → NTP via TabManager default
                SimpleHandler::NotifyTabListChanged();
                return true;
            }
        }

        // Ctrl+W / Cmd+W — Close active tab
        if (event.windows_key_code == 'W') {
#ifdef __APPLE__
            if (event.modifiers & EVENTFLAG_COMMAND_DOWN) {
#else
            if (event.modifiers & EVENTFLAG_CONTROL_DOWN) {
#endif
                BrowserWindow* ctrlw_win = GetOwnerWindow();
                int ctrlw_wid = ctrlw_win ? ctrlw_win->window_id : 0;
                int activeTabId = TabManager::GetInstance().GetActiveTabIdForWindow(ctrlw_wid);
                if (activeTabId != -1) {
                    LOG_INFO_BROWSER("⌨️ Ctrl+W: Closing tab " + std::to_string(activeTabId) + " in window " + std::to_string(ctrlw_wid));
                    TabManager::GetInstance().CloseTab(activeTabId);
                    SimpleHandler::NotifyTabListChanged();
                    // Check if this window still has tabs
                    bool windowHasTabs = false;
                    for (Tab* t : TabManager::GetInstance().GetAllTabs()) {
                        if (t->window_id == ctrlw_wid) { windowHasTabs = true; break; }
                    }
                    if (!windowHasTabs) {
                        CreateNewTabWithUrl("");  // Never leave window empty
                        SimpleHandler::NotifyTabListChanged();
                    }
                }
                return true;
            }
        }

        // Ctrl+H / Cmd+H — Open history in new tab (intercept chrome://history)
        if (event.windows_key_code == 'H') {
#ifdef __APPLE__
            if (event.modifiers & EVENTFLAG_COMMAND_DOWN) {
#else
            if (event.modifiers & EVENTFLAG_CONTROL_DOWN) {
#endif
                LOG_INFO_BROWSER("⌨️ Ctrl+H: Opening browser data in new tab");
                CreateNewTabWithUrl("http://127.0.0.1:5137/browser-data");
                SimpleHandler::NotifyTabListChanged();
                return true;
            }
        }

        // Ctrl+J / Cmd+J — Show download panel (intercept chrome://downloads)
        if (event.windows_key_code == 'J') {
#ifdef __APPLE__
            if (event.modifiers & EVENTFLAG_COMMAND_DOWN) {
#else
            if (event.modifiers & EVENTFLAG_CONTROL_DOWN) {
#endif
                LOG_INFO_BROWSER("⌨️ Ctrl+J: Showing download panel");
#ifdef _WIN32
                extern void CreateDownloadPanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
                extern void ShowDownloadPanelOverlay(int iconRightOffset, BrowserWindow* targetWin = nullptr);
                extern HWND g_download_panel_overlay_hwnd;
                extern HINSTANCE g_hInstance;

                if (!g_download_panel_overlay_hwnd || !IsWindow(g_download_panel_overlay_hwnd)) {
                    CreateDownloadPanelOverlay(g_hInstance, true, 0);
                } else {
                    ShowDownloadPanelOverlay(0, GetOwnerWindow());
                }
                NotifyDownloadStateChanged();
#elif defined(__APPLE__)
                extern void CreateDownloadPanelOverlayMacOS(int iconRightOffset);
                extern bool IsDownloadPanelOverlayVisible();
                extern void HideDownloadPanelOverlayMacOS();
                if (IsDownloadPanelOverlayVisible()) {
                    HideDownloadPanelOverlayMacOS();
                } else {
                    CreateDownloadPanelOverlayMacOS(0);
                    NotifyDownloadStateChanged();
                }
#endif
                return true;
            }
        }

        // Ctrl+D / Cmd+D — Bookmark current page
        if (event.windows_key_code == 'D') {
#ifdef __APPLE__
            if (event.modifiers & EVENTFLAG_COMMAND_DOWN) {
#else
            if (event.modifiers & EVENTFLAG_CONTROL_DOWN) {
#endif
                Tab* activeTab = TabManager::GetInstance().GetActiveTab();
                if (activeTab && !activeTab->url.empty()) {
                    LOG_INFO_BROWSER("⌨️ Ctrl+D: Bookmarking " + activeTab->url);
                    std::vector<std::string> emptyTags;
                    BookmarkManager::GetInstance().AddBookmark(
                        activeTab->url, activeTab->title, -1, emptyTags);
                }
                return true;
            }
        }

        // Ctrl+P / Cmd+P — Print current page
        if (event.windows_key_code == 'P') {
#ifdef __APPLE__
            if (event.modifiers & EVENTFLAG_COMMAND_DOWN) {
#else
            if (event.modifiers & EVENTFLAG_CONTROL_DOWN) {
#endif
                LOG_INFO_BROWSER("⌨️ Ctrl+P: Print current page");
                auto* active_tab = TabManager::GetInstance().GetActiveTab();
                if (active_tab && active_tab->browser) {
                    active_tab->browser->GetHost()->Print();
                }
                return true;
            }
        }

#ifdef __APPLE__
        // Cmd+, — Open Preferences/Settings (macOS standard)
        if (event.windows_key_code == 0xBC && (event.modifiers & EVENTFLAG_COMMAND_DOWN)) {
            LOG_INFO_BROWSER("⌨️ Cmd+,: Opening settings");
            CreateNewTabWithUrl("http://127.0.0.1:5137/settings-page/general");
            SimpleHandler::NotifyTabListChanged();
            return true;
        }
#endif

        // Alt+Left — Navigate back (active tab)
        // 0x25 = VK_LEFT (cross-platform: CEF uses Windows key codes on all platforms)
        if (event.windows_key_code == 0x25 && (event.modifiers & EVENTFLAG_ALT_DOWN)) {
            Tab* activeTab = TabManager::GetInstance().GetActiveTab();
            if (activeTab && activeTab->browser && activeTab->can_go_back) {
                LOG_DEBUG_BROWSER("⌨️ Alt+Left: GoBack");
                activeTab->browser->GoBack();
            }
            return true;
        }

        // Alt+Right — Navigate forward (active tab)
        // 0x27 = VK_RIGHT (cross-platform: CEF uses Windows key codes on all platforms)
        if (event.windows_key_code == 0x27 && (event.modifiers & EVENTFLAG_ALT_DOWN)) {
            Tab* activeTab = TabManager::GetInstance().GetActiveTab();
            if (activeTab && activeTab->browser && activeTab->can_go_forward) {
                LOG_DEBUG_BROWSER("⌨️ Alt+Right: GoForward");
                activeTab->browser->GoForward();
            }
            return true;
        }
    }

    return false; // Let other handlers process the event
}

void SimpleHandler::ShowOrFocusDevTools(CefRefPtr<CefBrowser> browser) {
    if (!browser || !browser->GetHost()) {
        LOG_DEBUG_BROWSER("⚠️ Cannot show DevTools - invalid browser");
        return;
    }

    // Check if DevTools already open
    if (!browser->GetHost()->HasDevTools()) {
        CefWindowInfo windowInfo;
        CefBrowserSettings settings;

#ifdef _WIN32
        // Windows: Use SetAsPopup for detached DevTools window (prevents blank window issues)
        windowInfo.SetAsPopup(NULL, "DevTools");
#endif
        // macOS: Default CefWindowInfo creates a popup window automatically

        // Use nullptr for client - CEF will create default handler for DevTools
        // This prevents lifecycle issues when DevTools window closes
        browser->GetHost()->ShowDevTools(windowInfo, nullptr, settings, CefPoint());
        LOG_DEBUG_BROWSER("🔧 DevTools opened via keyboard shortcut");
    } else {
        LOG_DEBUG_BROWSER("🔧 DevTools already open - focusing existing window");
    }
}

void SimpleHandler::SetRenderHandler(CefRefPtr<CefRenderHandler> handler) {
    render_handler_ = handler;
}

CefRefPtr<CefRenderHandler> SimpleHandler::GetRenderHandler() {
    return render_handler_;
}

// All custom context menu command IDs (MENU_ID_USER_FIRST = 26500)
// We use custom IDs for everything because CEF auto-disables built-in IDs
// when the menu is cleared and rebuilt.
static const int MENU_ID_DEV_TOOLS_INSPECT     = MENU_ID_USER_FIRST + 1;
static const int MENU_ID_OPEN_LINK_NEW_TAB     = MENU_ID_USER_FIRST + 2;
static const int MENU_ID_COPY_LINK_ADDRESS      = MENU_ID_USER_FIRST + 3;
static const int MENU_ID_SAVE_IMAGE_AS          = MENU_ID_USER_FIRST + 4;
static const int MENU_ID_COPY_IMAGE_URL         = MENU_ID_USER_FIRST + 5;
static const int MENU_ID_OPEN_IMAGE_NEW_TAB     = MENU_ID_USER_FIRST + 6;
static const int MENU_ID_CUSTOM_BACK            = MENU_ID_USER_FIRST + 10;
static const int MENU_ID_CUSTOM_FORWARD         = MENU_ID_USER_FIRST + 11;
static const int MENU_ID_CUSTOM_RELOAD          = MENU_ID_USER_FIRST + 12;
static const int MENU_ID_CUSTOM_UNDO            = MENU_ID_USER_FIRST + 13;
static const int MENU_ID_CUSTOM_REDO            = MENU_ID_USER_FIRST + 14;
static const int MENU_ID_CUSTOM_CUT             = MENU_ID_USER_FIRST + 15;
static const int MENU_ID_CUSTOM_COPY            = MENU_ID_USER_FIRST + 16;
static const int MENU_ID_CUSTOM_PASTE           = MENU_ID_USER_FIRST + 17;
static const int MENU_ID_CUSTOM_DELETE          = MENU_ID_USER_FIRST + 18;
static const int MENU_ID_CUSTOM_SELECT_ALL      = MENU_ID_USER_FIRST + 19;
static const int MENU_ID_CUSTOM_VIEW_SOURCE     = MENU_ID_USER_FIRST + 20;
static const int MENU_ID_SET_HOMEPAGE            = MENU_ID_USER_FIRST + 21;

void SimpleHandler::OnBeforeContextMenu(CefRefPtr<CefBrowser> browser,
                                        CefRefPtr<CefFrame> frame,
                                        CefRefPtr<CefContextMenuParams> params,
                                        CefRefPtr<CefMenuModel> model) {
    // Clear default Chromium context menu — we build our own
    model->Clear();

    // Only show full context menu for tab browsers (web content)
    bool isTab = (role_.find("tab_") == 0);

    if (!isTab) {
        // For header, overlays, etc. — just show Inspect
        model->AddItem(MENU_ID_DEV_TOOLS_INSPECT, "Inspect Element");
        return;
    }

    // Detect context from CefContextMenuParams
    cef_context_menu_type_flags_t flags = static_cast<cef_context_menu_type_flags_t>(params->GetTypeFlags());
    bool hasLink = (flags & CM_TYPEFLAG_LINK) != 0;
    bool hasSelection = (flags & CM_TYPEFLAG_SELECTION) != 0;
    bool isEditable = (flags & CM_TYPEFLAG_EDITABLE) != 0;
    bool hasMedia = (flags & CM_TYPEFLAG_MEDIA) != 0;
    bool isImage = hasMedia && (params->GetMediaType() == CM_MEDIATYPE_IMAGE);

    // --- Link context ---
    if (hasLink) {
        model->AddItem(MENU_ID_OPEN_LINK_NEW_TAB, "Open Link in New Tab");
        model->AddItem(MENU_ID_COPY_LINK_ADDRESS, "Copy Link Address");
        model->AddSeparator();
    }

    // --- Image context ---
    if (isImage) {
        model->AddItem(MENU_ID_SAVE_IMAGE_AS, "Save Image As...");
        model->AddItem(MENU_ID_COPY_IMAGE_URL, "Copy Image Address");
        model->AddItem(MENU_ID_OPEN_IMAGE_NEW_TAB, "Open Image in New Tab");
        model->AddSeparator();
    }

    // --- Editable field context (input, textarea, contenteditable) ---
    if (isEditable) {
        model->AddItem(MENU_ID_CUSTOM_UNDO, "Undo");
        model->AddItem(MENU_ID_CUSTOM_REDO, "Redo");
        model->AddSeparator();
        model->AddItem(MENU_ID_CUSTOM_CUT, "Cut");
        model->AddItem(MENU_ID_CUSTOM_COPY, "Copy");
        model->AddItem(MENU_ID_CUSTOM_PASTE, "Paste");
        model->AddItem(MENU_ID_CUSTOM_DELETE, "Delete");
        model->AddSeparator();
        model->AddItem(MENU_ID_CUSTOM_SELECT_ALL, "Select All");
    }
    // --- Text selection context (non-editable) ---
    else if (hasSelection) {
        model->AddItem(MENU_ID_CUSTOM_COPY, "Copy");
        model->AddSeparator();
        model->AddItem(MENU_ID_CUSTOM_SELECT_ALL, "Select All");
    }
    // --- Plain page context (no selection, no link, no image) ---
    else if (!hasLink && !isImage) {
        model->AddItem(MENU_ID_CUSTOM_BACK, "Back");
        model->AddItem(MENU_ID_CUSTOM_FORWARD, "Forward");
        model->AddItem(MENU_ID_CUSTOM_RELOAD, "Reload");
        model->AddSeparator();
        model->AddItem(MENU_ID_CUSTOM_SELECT_ALL, "Select All");
        model->AddItem(MENU_ID_CUSTOM_VIEW_SOURCE, "View Page Source");
        model->AddSeparator();
        model->AddItem(MENU_ID_SET_HOMEPAGE, "Set as Home Page");
    }

    // --- Always add Inspect at the bottom ---
    model->AddSeparator();
    model->AddItem(MENU_ID_DEV_TOOLS_INSPECT, "Inspect Element");

    // Enable/disable Back and Forward based on navigation state
    if (model->GetIndexOf(MENU_ID_CUSTOM_BACK) != -1) {
        model->SetEnabled(MENU_ID_CUSTOM_BACK, browser->CanGoBack());
    }
    if (model->GetIndexOf(MENU_ID_CUSTOM_FORWARD) != -1) {
        model->SetEnabled(MENU_ID_CUSTOM_FORWARD, browser->CanGoForward());
    }

    LOG_DEBUG_BROWSER("Context menu built - flags: " + std::to_string(flags) +
                      " link:" + std::to_string(hasLink) +
                      " sel:" + std::to_string(hasSelection) +
                      " edit:" + std::to_string(isEditable) +
                      " img:" + std::to_string(isImage));
}

/// Helper: create a new tab with the given URL in the active window (cross-platform)
static void CreateNewTabWithUrl(const std::string& url) {
#ifdef _WIN32
    // Use active window, falling back to window 0
    BrowserWindow* activeWin = WindowManager::GetInstance().GetActiveWindow();
    HWND parentHwnd = activeWin ? activeWin->hwnd : nullptr;
    int winId = activeWin ? activeWin->window_id : 0;

    if (!parentHwnd) {
        extern HWND g_hwnd;
        parentHwnd = g_hwnd;
    }

    RECT rect;
    GetClientRect(parentHwnd, &rect);
    int width = rect.right - rect.left;
    int height = rect.bottom - rect.top;
    int shellHeight = GetHeaderHeightPx(parentHwnd);
    int tabHeight = height - shellHeight;
    TabManager::GetInstance().CreateTab(url, parentHwnd, 0, shellHeight, width, tabHeight, winId);
#else
    BrowserWindow* activeWin = WindowManager::GetInstance().GetActiveWindow();
    void* parentView = (activeWin && activeWin->webview_view) ? activeWin->webview_view : nullptr;
    int winId = activeWin ? activeWin->window_id : 0;

    if (!parentView) {
        extern NSView* g_webview_view;
        parentView = g_webview_view;
    }

    ViewDimensions dims = GetViewDimensions(parentView);
    TabManager::GetInstance().CreateTab(url, parentView, 0, 0, dims.width, dims.height, winId);
#endif
}

/// Helper: copy a string to the OS clipboard (cross-platform)
static void CopyTextToClipboard(const std::string& text) {
#ifdef _WIN32
    if (OpenClipboard(nullptr)) {
        EmptyClipboard();
        HGLOBAL hGlob = GlobalAlloc(GMEM_MOVEABLE, text.size() + 1);
        if (hGlob) {
            memcpy(GlobalLock(hGlob), text.c_str(), text.size() + 1);
            GlobalUnlock(hGlob);
            SetClipboardData(CF_TEXT, hGlob);
        }
        CloseClipboard();
    }
#elif defined(__APPLE__)
    // macOS: Use pipe to pbcopy (avoids shell escaping / injection issues)
    FILE* pipe = popen("pbcopy", "w");
    if (pipe) {
        fwrite(text.c_str(), 1, text.size(), pipe);
        pclose(pipe);
    }
#endif
}

bool SimpleHandler::OnContextMenuCommand(CefRefPtr<CefBrowser> browser,
                                         CefRefPtr<CefFrame> frame,
                                         CefRefPtr<CefContextMenuParams> params,
                                         int command_id,
                                         EventFlags event_flags) {
    LOG_DEBUG_BROWSER("Context menu command: " + std::to_string(command_id) + " (role: " + role_ + ")");

    // --- Inspect Element (all windows) ---
    if (command_id == MENU_ID_DEV_TOOLS_INSPECT) {
        ShowOrFocusDevTools(browser);
        return true;
    }

    // --- Open Link in New Tab ---
    if (command_id == MENU_ID_OPEN_LINK_NEW_TAB || command_id == 50100) {
        std::string link_url = params->GetLinkUrl().ToString();
        if (!link_url.empty()) {
            LOG_DEBUG_BROWSER("Opening link in new tab: " + link_url);
            CreateNewTabWithUrl(link_url);
            return true;
        }
        return false;
    }

    // --- Copy Link Address ---
    if (command_id == MENU_ID_COPY_LINK_ADDRESS) {
        std::string link_url = params->GetLinkUrl().ToString();
        if (!link_url.empty()) {
            CopyTextToClipboard(link_url);
            LOG_DEBUG_BROWSER("Copied link address: " + link_url);
        }
        return true;
    }

    // --- Save Image As (triggers download handler) ---
    if (command_id == MENU_ID_SAVE_IMAGE_AS) {
        std::string src_url = params->GetSourceUrl().ToString();
        if (!src_url.empty()) {
            browser->GetHost()->StartDownload(src_url);
            LOG_DEBUG_BROWSER("Starting image download: " + src_url);
        }
        return true;
    }

    // --- Copy Image Address ---
    if (command_id == MENU_ID_COPY_IMAGE_URL) {
        std::string src_url = params->GetSourceUrl().ToString();
        if (!src_url.empty()) {
            CopyTextToClipboard(src_url);
            LOG_DEBUG_BROWSER("Copied image address: " + src_url);
        }
        return true;
    }

    // --- Open Image in New Tab ---
    if (command_id == MENU_ID_OPEN_IMAGE_NEW_TAB) {
        std::string src_url = params->GetSourceUrl().ToString();
        if (!src_url.empty()) {
            LOG_DEBUG_BROWSER("Opening image in new tab: " + src_url);
            CreateNewTabWithUrl(src_url);
        }
        return true;
    }

    // --- View Page Source (open view-source: URL in new tab) ---
    if (command_id == MENU_ID_CUSTOM_VIEW_SOURCE) {
        std::string current_url = browser->GetMainFrame()->GetURL().ToString();
        if (!current_url.empty()) {
            std::string source_url = "view-source:" + current_url;
            LOG_DEBUG_BROWSER("Opening page source in new tab: " + source_url);
            CreateNewTabWithUrl(source_url);
        }
        return true;
    }

    // --- Set as Home Page ---
    if (command_id == MENU_ID_SET_HOMEPAGE) {
        std::string url = browser->GetMainFrame()->GetURL().ToString();
        if (!url.empty()) {
            SettingsManager::GetInstance().SetHomepage(url);
            LOG_INFO_BROWSER("🏠 Homepage set to: " + url);
        }
        return true;
    }

    // --- Navigation commands ---
    if (command_id == MENU_ID_CUSTOM_BACK) {
        browser->GoBack();
        return true;
    }
    if (command_id == MENU_ID_CUSTOM_FORWARD) {
        browser->GoForward();
        return true;
    }
    if (command_id == MENU_ID_CUSTOM_RELOAD) {
        browser->Reload();
        return true;
    }

    // --- Editing commands (executed via JavaScript in the focused frame) ---
    if (command_id == MENU_ID_CUSTOM_UNDO) {
        frame->ExecuteJavaScript("document.execCommand('undo')", frame->GetURL(), 0);
        return true;
    }
    if (command_id == MENU_ID_CUSTOM_REDO) {
        frame->ExecuteJavaScript("document.execCommand('redo')", frame->GetURL(), 0);
        return true;
    }
    if (command_id == MENU_ID_CUSTOM_CUT) {
        frame->ExecuteJavaScript("document.execCommand('cut')", frame->GetURL(), 0);
        return true;
    }
    if (command_id == MENU_ID_CUSTOM_COPY) {
        frame->ExecuteJavaScript("document.execCommand('copy')", frame->GetURL(), 0);
        return true;
    }
    if (command_id == MENU_ID_CUSTOM_PASTE) {
        frame->ExecuteJavaScript("document.execCommand('paste')", frame->GetURL(), 0);
        return true;
    }
    if (command_id == MENU_ID_CUSTOM_DELETE) {
        frame->ExecuteJavaScript("document.execCommand('delete')", frame->GetURL(), 0);
        return true;
    }
    if (command_id == MENU_ID_CUSTOM_SELECT_ALL) {
        frame->ExecuteJavaScript("document.execCommand('selectAll')", frame->GetURL(), 0);
        return true;
    }

    // Allow default handling for any unrecognized commands
    return false;
}

// ========== DOWNLOAD HANDLER ==========

bool SimpleHandler::CanDownload(CefRefPtr<CefBrowser> browser,
                                const CefString& url,
                                const CefString& request_method) {
    LOG_INFO_BROWSER("📥 Download requested: " + url.ToString());
    return true;
}

bool SimpleHandler::OnBeforeDownload(CefRefPtr<CefBrowser> browser,
                                     CefRefPtr<CefDownloadItem> download_item,
                                     const CefString& suggested_name,
                                     CefRefPtr<CefBeforeDownloadCallback> callback) {
    CEF_REQUIRE_UI_THREAD();
    std::string suggestedName = suggested_name.ToString();
    LOG_INFO_BROWSER("📥 OnBeforeDownload: " + suggestedName +
                     " (id: " + std::to_string(download_item->GetId()) + ")");

    auto browserSettings = SettingsManager::GetInstance().GetBrowserSettings();
    LOG_INFO_BROWSER("📥 Settings: downloadsPath='" + browserSettings.downloadsPath +
                     "', askWhereToSave=" + (browserSettings.askWhereToSave ? "true" : "false"));

    std::string folder = browserSettings.downloadsPath;
    bool folderValid = false;

    if (!folder.empty()) {
        try {
            folderValid = std::filesystem::is_directory(folder);
        } catch (...) {
            folderValid = false;
        }
        if (!folderValid) {
            LOG_WARNING_BROWSER("📥 Downloads folder does not exist: " + folder);
        }
    }

    // Case 1: No dialog — silent download to configured (or system default) folder
    if (!browserSettings.askWhereToSave) {
        if (folderValid) {
#ifdef _WIN32
            std::string path = folder + "\\" + suggestedName;
#else
            std::string path = folder + "/" + suggestedName;
#endif
            LOG_INFO_BROWSER("📥 Silent download to: " + path);
            callback->Continue(path, false);
        } else {
            LOG_INFO_BROWSER("📥 Silent download to system default");
            callback->Continue("", false);
        }
        return true;
    }

    // Case 2: Ask where to save — need Save As dialog
#ifdef _WIN32
    // Use Win32 IFileSaveDialog directly so we control the initial directory.
    // CEF's built-in Save As ignores the directory from the path we pass.
    if (folderValid) {
        CefRefPtr<CefBeforeDownloadCallback> cb = callback;
        std::string name = suggestedName;
        std::string dir = folder;

        std::thread([cb, dir, name]() {
            CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
            IFileSaveDialog* pDialog = nullptr;
            HRESULT hr = CoCreateInstance(CLSID_FileSaveDialog, nullptr, CLSCTX_ALL,
                                          IID_IFileSaveDialog, reinterpret_cast<void**>(&pDialog));
            if (SUCCEEDED(hr) && pDialog) {
                pDialog->SetTitle(L"Save As");

                // Set initial directory
                int dirLen = MultiByteToWideChar(CP_UTF8, 0, dir.c_str(), -1, nullptr, 0);
                std::wstring wdir(dirLen - 1, L'\0');
                MultiByteToWideChar(CP_UTF8, 0, dir.c_str(), -1, &wdir[0], dirLen);

                IShellItem* pDir = nullptr;
                if (SUCCEEDED(SHCreateItemFromParsingName(wdir.c_str(), nullptr,
                                                           IID_IShellItem, reinterpret_cast<void**>(&pDir)))) {
                    pDialog->SetFolder(pDir);
                    pDir->Release();
                }

                // Set suggested filename and default extension
                int nameLen = MultiByteToWideChar(CP_UTF8, 0, name.c_str(), -1, nullptr, 0);
                std::wstring wname(nameLen - 1, L'\0');
                MultiByteToWideChar(CP_UTF8, 0, name.c_str(), -1, &wname[0], nameLen);
                pDialog->SetFileName(wname.c_str());

                size_t dotPos = wname.rfind(L'.');
                if (dotPos != std::wstring::npos) {
                    pDialog->SetDefaultExtension(wname.substr(dotPos + 1).c_str());
                }

                // All file types filter
                COMDLG_FILTERSPEC filter = { L"All Files", L"*.*" };
                pDialog->SetFileTypes(1, &filter);

                extern HWND g_hwnd;
                hr = pDialog->Show(g_hwnd);
                if (SUCCEEDED(hr)) {
                    IShellItem* pItem = nullptr;
                    if (SUCCEEDED(pDialog->GetResult(&pItem)) && pItem) {
                        PWSTR pszPath = nullptr;
                        if (SUCCEEDED(pItem->GetDisplayName(SIGDN_FILESYSPATH, &pszPath)) && pszPath) {
                            int len = WideCharToMultiByte(CP_UTF8, 0, pszPath, -1, nullptr, 0, nullptr, nullptr);
                            std::string selectedPath(len - 1, '\0');
                            WideCharToMultiByte(CP_UTF8, 0, pszPath, -1, &selectedPath[0], len, nullptr, nullptr);
                            CoTaskMemFree(pszPath);

                            CefRefPtr<CefBeforeDownloadCallback> ref = cb;
                            std::string p = selectedPath;
                            CefPostTask(TID_UI, base::BindOnce(
                                [](CefRefPtr<CefBeforeDownloadCallback> callback, std::string path) {
                                    callback->Continue(path, false);
                                }, ref, p));
                        }
                        pItem->Release();
                    }
                }
                // If user cancelled, Continue() is never called — download stays pending
                // and is cleaned up by CEF when the callback ref is released.
                pDialog->Release();
            }
            CoUninitialize();
        }).detach();

        return true;
    }
#endif

    // Fallback: no custom folder or macOS — use CEF's built-in Save As
    LOG_INFO_BROWSER("📥 Using CEF Save As dialog (system default)");
    callback->Continue("", true);
    return true;
}

void SimpleHandler::OnDownloadUpdated(CefRefPtr<CefBrowser> browser,
                                      CefRefPtr<CefDownloadItem> download_item,
                                      CefRefPtr<CefDownloadItemCallback> callback) {
    CEF_REQUIRE_UI_THREAD();

    uint32_t id = download_item->GetId();
    std::string full_path = download_item->GetFullPath().ToString();
    bool in_progress = download_item->IsInProgress();
    bool complete = download_item->IsComplete();
    bool canceled = download_item->IsCanceled();

    // Don't track downloads until the user has chosen a save location.
    // CEF fires OnDownloadUpdated while the Save As dialog is still open
    // (full_path is empty). Also skip if user cancelled the Save As dialog
    // and we never tracked this item.
    bool already_tracked = active_downloads_.count(id) > 0;
    if (full_path.empty() && !already_tracked) {
        LOG_DEBUG_BROWSER("📥 Skipping download update (no save path yet): id=" + std::to_string(id));
        return;
    }

    // CEF 136 has no IsPaused() — track pause state ourselves via paused_downloads_ set
    bool paused = paused_downloads_.count(id) > 0;
    // Clear paused flag if download completed or canceled
    if (complete || canceled) {
        paused_downloads_.erase(id);
        paused = false;
    }

    DownloadInfo info;
    info.id = id;
    info.url = download_item->GetURL().ToString();
    info.filename = download_item->GetSuggestedFileName().ToString();
    info.full_path = full_path;
    info.received_bytes = download_item->GetReceivedBytes();
    info.total_bytes = download_item->GetTotalBytes();
    info.percent_complete = download_item->GetPercentComplete();
    info.current_speed = download_item->GetCurrentSpeed();
    info.is_in_progress = in_progress && !paused;
    info.is_complete = complete;
    info.is_canceled = canceled;
    info.is_paused = paused;

    // Store callback for pause/resume/cancel control (only while download is active)
    if (in_progress) {
        info.item_callback = callback;
    } else {
        info.item_callback = nullptr;
    }

    active_downloads_[id] = info;

    LOG_DEBUG_BROWSER("📥 Download update: id=" + std::to_string(id) +
                      " path=" + full_path +
                      " progress=" + std::to_string(info.percent_complete) + "%" +
                      " bytes=" + std::to_string(info.received_bytes) + "/" + std::to_string(info.total_bytes) +
                      " speed=" + std::to_string(info.current_speed) +
                      " complete=" + std::to_string(complete) +
                      " canceled=" + std::to_string(canceled));

    // Throttle progress notifications to ~2/sec (500ms debounce).
    // Completion/cancellation notify immediately so UI updates instantly.
    if (download_item->IsComplete() || download_item->IsCanceled()) {
        download_notify_pending_ = false;
        NotifyDownloadStateChanged();
    } else if (!download_notify_pending_) {
        download_notify_pending_ = true;
        CefPostDelayedTask(TID_UI, base::BindOnce([]() {
            SimpleHandler::download_notify_pending_ = false;
            SimpleHandler::NotifyDownloadStateChanged();
        }), 500);
    }
}

void SimpleHandler::NotifyDownloadStateChanged() {
    CEF_REQUIRE_UI_THREAD();

    nlohmann::json downloads_array = nlohmann::json::array();
    for (const auto& pair : active_downloads_) {
        const DownloadInfo& d = pair.second;
        nlohmann::json item;
        item["id"] = d.id;
        item["url"] = d.url;
        item["filename"] = d.filename;
        item["fullPath"] = d.full_path;
        item["receivedBytes"] = d.received_bytes;
        item["totalBytes"] = d.total_bytes;
        item["percentComplete"] = d.percent_complete;
        item["currentSpeed"] = d.current_speed;
        item["isInProgress"] = d.is_in_progress;
        item["isComplete"] = d.is_complete;
        item["isCanceled"] = d.is_canceled;
        item["isPaused"] = d.is_paused;
        downloads_array.push_back(item);
    }

    std::string json_str = downloads_array.dump();

    CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
    if (header) {
        CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("download_state_update");
        msg->GetArgumentList()->SetString(0, json_str);
        header->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
    }

    // Also send to download panel overlay browser if it exists
    CefRefPtr<CefBrowser> dl_panel = SimpleHandler::GetDownloadPanelBrowser();
    if (dl_panel) {
        CefRefPtr<CefProcessMessage> msg2 = CefProcessMessage::Create("download_state_update");
        msg2->GetArgumentList()->SetString(0, json_str);
        dl_panel->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg2);
    }

    LOG_DEBUG_BROWSER("📥 Download state sent: " + std::to_string(active_downloads_.size()) + " items");
}

// ========== FIND HANDLER ==========

void SimpleHandler::OnFindResult(CefRefPtr<CefBrowser> browser,
                                  int identifier,
                                  int count,
                                  const CefRect& selectionRect,
                                  int activeMatchOrdinal,
                                  bool finalUpdate) {
    CEF_REQUIRE_UI_THREAD();

    LOG_INFO_BROWSER("🔍 OnFindResult: count=" + std::to_string(count) +
                      " activeMatch=" + std::to_string(activeMatchOrdinal) +
                      " final=" + std::to_string(finalUpdate) +
                      " browser=" + std::to_string(browser->GetIdentifier()));

    // Build JSON with find result data
    nlohmann::json result;
    result["count"] = count;
    result["activeMatchOrdinal"] = activeMatchOrdinal;
    result["finalUpdate"] = finalUpdate;

    std::string json_str = result.dump();

    // Send find_result to header browser (React find bar lives there)
    CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
    if (header) {
        CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("find_result");
        msg->GetArgumentList()->SetString(0, json_str);
        header->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
    } else {
        LOG_WARNING_BROWSER("🔍 OnFindResult: header browser is null!");
    }
}

// ========== JS DIALOG HANDLER ==========

bool SimpleHandler::OnBeforeUnloadDialog(CefRefPtr<CefBrowser> browser,
                                          const CefString& message_text,
                                          bool is_reload,
                                          CefRefPtr<CefJSDialogCallback> callback) {
    CEF_REQUIRE_UI_THREAD();

    // Auto-allow navigation away from pages with beforeunload handlers.
    // This prevents malicious sites from trapping users with repeated
    // "Are you sure you want to leave?" dialogs.
    // Chrome's native dialog handling covers legitimate alert/confirm/prompt.
    LOG_DEBUG_BROWSER("🔒 OnBeforeUnloadDialog: auto-allowing navigation (suppressing beforeunload trap)");
    callback->Continue(true, CefString());
    return true;
}
