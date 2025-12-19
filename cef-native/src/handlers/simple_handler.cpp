// cef_native/src/simple_handler.cpp
#include "../../include/handlers/simple_handler.h"
#include "../../include/handlers/simple_app.h"
#include "../../include/core/TabManager.h"
#include "include/wrapper/cef_helpers.h"
#include "include/base/cef_bind.h"
#include "include/cef_v8.h"
#include "include/wrapper/cef_closure_task.h"
#include "include/cef_task.h"
#include "include/internal/cef_types.h"  // For CEF_WOD_* constants
#include "base/cef_callback.h"
#include "base/internal/cef_callback_internal.h"
#include <fstream>
#include <filesystem>
#include <cstdlib>
#include "../../include/core/WalletService.h"
#include "../../include/core/HttpRequestInterceptor.h"
#include <windows.h>
#include <iostream>
#include <string>
#include <sstream>
#include <nlohmann/json.hpp>

// Forward declaration of Logger class from main shell
class Logger {
public:
    static void Log(const std::string& message, int level = 1, int process = 2);
};

// Convenience macros for easier logging
#define LOG_DEBUG_BROWSER(msg) Logger::Log(msg, 0, 2)
#define LOG_INFO_BROWSER(msg) Logger::Log(msg, 1, 2)
#define LOG_WARNING_BROWSER(msg) Logger::Log(msg, 2, 2)

#include "../../include/core/PendingAuthRequest.h"
#define LOG_ERROR_BROWSER(msg) Logger::Log(msg, 3, 2)

extern void CreateTestOverlayWithSeparateProcess(HINSTANCE hInstance);
extern void CreateWalletOverlayWithSeparateProcess(HINSTANCE hInstance);
extern void CreateBackupOverlayWithSeparateProcess(HINSTANCE hInstance);

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

std::string SimpleHandler::pending_panel_;
bool SimpleHandler::needs_overlay_reload_ = false;

SimpleHandler::SimpleHandler(const std::string& role) : role_(role) {}

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
CefRefPtr<CefBrowser> SimpleHandler::overlay_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::settings_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::wallet_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::backup_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::brc100_auth_browser_ = nullptr;
CefRefPtr<CefBrowser> SimpleHandler::GetOverlayBrowser() {
    return overlay_browser_;
}
CefRefPtr<CefBrowser> SimpleHandler::GetHeaderBrowser() {
    return header_browser_;
}

CefRefPtr<CefBrowser> SimpleHandler::GetWebviewBrowser() {
    return webview_browser_;
}

CefRefPtr<CefBrowser> SimpleHandler::GetSettingsBrowser() {
    return settings_browser_;
}
CefRefPtr<CefBrowser> SimpleHandler::GetWalletBrowser() {
    return wallet_browser_;
}
CefRefPtr<CefBrowser> SimpleHandler::GetBackupBrowser() {
    return backup_browser_;
}

CefRefPtr<CefBrowser> SimpleHandler::GetBRC100AuthBrowser() {
    return brc100_auth_browser_;
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
void SimpleHandler::NotifyTabListChanged() {
    CEF_REQUIRE_UI_THREAD();

    std::vector<Tab*> tabs = TabManager::GetInstance().GetAllTabs();
    int active_tab_id = TabManager::GetInstance().GetActiveTabId();

    // Build JSON response using nlohmann::json (handles escaping automatically)
    nlohmann::json response;
    response["activeTabId"] = active_tab_id;
    response["tabs"] = nlohmann::json::array();

    for (Tab* tab : tabs) {
        nlohmann::json tab_json;
        tab_json["id"] = tab->id;
        tab_json["title"] = tab->title;
        tab_json["url"] = tab->url;
        tab_json["isActive"] = (tab->id == active_tab_id);
        tab_json["isLoading"] = tab->is_loading;
        if (!tab->favicon_url.empty()) {
            tab_json["favicon"] = tab->favicon_url;
        }
        response["tabs"].push_back(tab_json);
    }

    std::string json_str = response.dump();

    // Send response to header browser
    CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
    if (header) {
        CefRefPtr<CefProcessMessage> cef_response = CefProcessMessage::Create("tab_list_response");
        CefRefPtr<CefListValue> response_args = cef_response->GetArgumentList();
        response_args->SetString(0, json_str);
        header->GetMainFrame()->SendProcessMessage(PID_RENDERER, cef_response);
        LOG_DEBUG_BROWSER("📑 Tab list updated and sent to header: " + json_str);
    }
}

void SimpleHandler::OnTitleChange(CefRefPtr<CefBrowser> browser, const CefString& title) {
    CEF_REQUIRE_UI_THREAD();

    // Check if this is a tab browser and update TabManager
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        TabManager::GetInstance().UpdateTabTitle(tab_id, title.ToString());
    }

#if defined(OS_WIN)
    SetWindowText(browser->GetHost()->GetWindowHandle(), std::wstring(title).c_str());
#endif
}

void SimpleHandler::OnAddressChange(CefRefPtr<CefBrowser> browser,
                                   CefRefPtr<CefFrame> frame,
                                   const CefString& url) {
    CEF_REQUIRE_UI_THREAD();

    // Only track main frame address changes
    if (!frame->IsMain()) {
        return;
    }

    // Check if this is a tab browser and update TabManager
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        TabManager::GetInstance().UpdateTabURL(tab_id, url.ToString());
        LOG_DEBUG_BROWSER("🔗 Tab " + std::to_string(tab_id) + " URL updated to: " + url.ToString());
    }
}

void SimpleHandler::OnFaviconURLChange(CefRefPtr<CefBrowser> browser,
                                      const std::vector<CefString>& icon_urls) {
    CEF_REQUIRE_UI_THREAD();

    // Only process if we have favicon URLs
    if (icon_urls.empty()) {
        return;
    }

    // Check if this is a tab browser and update TabManager
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        // Use the first favicon URL (usually the most appropriate)
        std::string favicon_url = icon_urls[0].ToString();
        TabManager::GetInstance().UpdateTabFavicon(tab_id, favicon_url);
        LOG_DEBUG_BROWSER("🖼️ Tab " + std::to_string(tab_id) + " favicon updated: " + favicon_url);
    }
}

void SimpleHandler::OnLoadError(CefRefPtr<CefBrowser> browser,
                                CefRefPtr<CefFrame> frame,
                                ErrorCode errorCode,
                                const CefString& errorText,
                                const CefString& failedUrl) {
    LOG_DEBUG_BROWSER("❌ Load error for role: " + role_);
    LOG_DEBUG_BROWSER("❌ Load error: " + failedUrl.ToString() + " - " + errorText.ToString());
    LOG_DEBUG_BROWSER("❌ Error code: " + std::to_string(errorCode));

    if (frame->IsMain()) {
        std::string html = "<html><body><h1>Failed to load</h1><p>URL: " +
                           failedUrl.ToString() + "</p><p>Error: " +
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

void SimpleHandler::OnLoadingStateChange(CefRefPtr<CefBrowser> browser,
                                         bool isLoading,
                                         bool canGoBack,
                                         bool canGoForward) {
    CEF_REQUIRE_UI_THREAD();

    // Check if this is a tab browser and update TabManager
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        TabManager::GetInstance().UpdateTabLoadingState(tab_id, isLoading, canGoBack, canGoForward);
    }

    LOG_DEBUG_BROWSER("📡 Loading state for role " + role_ + ": " + (isLoading ? "loading..." : "done"));

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
            // Inject the hodosBrowser API into header browser (where React app runs)
            LOG_DEBUG_BROWSER("🔧 HEADER BROWSER LOADED - Injecting hodosBrowser API");

            extern void InjectHodosBrowserAPI(CefRefPtr<CefBrowser> browser);
            InjectHodosBrowserAPI(browser);
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
        } else if (ExtractTabIdFromRole(role_) != -1) {
            // Inject the hodosBrowser API into tab browsers
            LOG_DEBUG_BROWSER("🔧 TAB BROWSER LOADED - Injecting hodosBrowser API for tab " + role_);

            extern void InjectHodosBrowserAPI(CefRefPtr<CefBrowser> browser);
            InjectHodosBrowserAPI(browser);
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

    LOG_DEBUG_BROWSER("✅ OnAfterCreated for role: " + role_);

    // Check if this is a tab browser - register with TabManager first
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        // This is a tab browser - register with TabManager
        TabManager::GetInstance().RegisterTabBrowser(tab_id, browser);
        LOG_DEBUG_BROWSER("📑 Tab browser registered: ID " + std::to_string(tab_id) +
                         ", Browser ID: " + std::to_string(browser->GetIdentifier()));

        // Delayed WasResized() + Invalidate() to fix first-render black screen
        // CEF needs time for HWND to be fully initialized before rendering
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
                LOG(INFO) << "Tab browser delayed resize/invalidate completed";
            }
        }, browser_ref), 150);  // 150ms delay for window initialization

        return;  // Tab browsers don't need the overlay/header/webview handling below
    }

    if (role_ == "webview") {
        webview_browser_ = browser;
        LOG_DEBUG_BROWSER("📡 WebView browser reference stored.");
        LOG_DEBUG_BROWSER("📡 WebView browser reference stored. ID: " + std::to_string(browser->GetIdentifier()));

        // Trigger initial resize to ensure content renders on startup
        browser->GetHost()->WasResized();
        LOG_DEBUG_BROWSER("🔄 Initial WasResized() called for webview browser");
    } else if (role_ == "header") {
        header_browser_ = browser;
        LOG_DEBUG_BROWSER("🧭 header browser initialized.");
        LOG_DEBUG_BROWSER("🧭 header browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        // Trigger initial resize to ensure content renders on startup
        browser->GetHost()->WasResized();
        LOG_DEBUG_BROWSER("🔄 Initial WasResized() called for header browser");
    } else if (role_ == "overlay") {
        overlay_browser_ = browser;
        LOG_DEBUG_BROWSER("🪟 Overlay browser initialized.");
        LOG_DEBUG_BROWSER("🪟 Overlay browser initialized. ID: " + std::to_string(browser->GetIdentifier()));
    } else if (role_ == "settings") {
        settings_browser_ = browser;
        LOG_DEBUG_BROWSER("⚙️ Settings browser initialized.");
        LOG_DEBUG_BROWSER("⚙️ Settings browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        // Delayed resize/invalidate to fix first-render issue
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);

    } else if (role_ == "wallet") {
        wallet_browser_ = browser;
        LOG_DEBUG_BROWSER("💰 Wallet browser initialized.");
        LOG_DEBUG_BROWSER("💰 Wallet browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        // Delayed resize/invalidate to fix first-render issue
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);

    } else if (role_ == "backup") {
        backup_browser_ = browser;
        LOG_DEBUG_BROWSER("💾 Backup browser initialized.");
        LOG_DEBUG_BROWSER("💾 Backup browser initialized. ID: " + std::to_string(browser->GetIdentifier()));

        // Delayed resize/invalidate to fix first-render issue
        CefRefPtr<CefBrowser> browser_ref = browser;
        CefPostDelayedTask(TID_UI, base::BindOnce([](CefRefPtr<CefBrowser> b) {
            if (b && b->GetHost()) {
                b->GetHost()->WasResized();
                b->GetHost()->Invalidate(PET_VIEW);
            }
        }, browser_ref), 150);

    } else if (role_ == "brc100auth") {
        brc100_auth_browser_ = browser;
        LOG_DEBUG_BROWSER("🔐 BRC-100 Auth browser initialized.");
        LOG_DEBUG_BROWSER("🔐 BRC-100 Auth browser initialized. ID: " + std::to_string(browser->GetIdentifier()));
        LOG_DEBUG_BROWSER("🔐 BRC-100 Auth browser main frame URL: " + browser->GetMainFrame()->GetURL().ToString());

        // Delayed resize/invalidate to fix first-render issue
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

    // Check if this is a tab browser
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

    // Handle overlay browser cleanup
    if (role_ == "settings" && browser == settings_browser_) {
        std::cout << "  → Settings browser cleanup" << std::endl;
        settings_browser_ = nullptr;
    } else if (role_ == "wallet" && browser == wallet_browser_) {
        std::cout << "  → Wallet browser cleanup" << std::endl;
        wallet_browser_ = nullptr;
    } else if (role_ == "backup" && browser == backup_browser_) {
        std::cout << "  → Backup browser cleanup" << std::endl;
        backup_browser_ = nullptr;
    } else if (role_ == "brc100auth" && browser == brc100_auth_browser_) {
        std::cout << "  → BRC100 auth browser cleanup" << std::endl;
        brc100_auth_browser_ = nullptr;
    } else if (role_ == "overlay" && browser == overlay_browser_) {
        std::cout << "  → Overlay browser cleanup" << std::endl;
        overlay_browser_ = nullptr;
    } else if (role_ == "webview" && browser == webview_browser_) {
        std::cout << "  → Webview browser cleanup" << std::endl;
        webview_browser_ = nullptr;
    } else if (role_ == "header" && browser == header_browser_) {
        std::cout << "  → Header browser cleanup" << std::endl;
        header_browser_ = nullptr;
    } else {
        std::cout << "  → No matching browser type (might be DevTools)" << std::endl;
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
        // Create new tab for ANY browser (tab browser, webview, etc.)
        LOG_DEBUG_BROWSER("📑 Converting popup to new tab: " + url + " (disposition: " + disposition_str + ", role: " + role_ + ")");

        // Get main window dimensions
        extern HWND g_hwnd;
        RECT rect;
        GetClientRect(g_hwnd, &rect);
        int width = rect.right - rect.left;
        int height = rect.bottom - rect.top;
        int shellHeight = (std::max)(100, static_cast<int>(height * 0.12));
        int tabHeight = height - shellHeight;

        // Create new tab with the popup URL
        TabManager::GetInstance().CreateTab(url, g_hwnd, 0, shellHeight, width, tabHeight);

        // Return true to cancel the popup window creation (we handled it with a new tab)
        return true;
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

    // Additional logging for debugging
    LOG_DEBUG_BROWSER("📨 Message received: " + message_name + ", Browser ID: " + std::to_string(browser->GetIdentifier()));

    // ========== TAB MANAGEMENT MESSAGES ==========

    if (message_name == "tab_create") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string url = args->GetSize() > 0 ? args->GetString(0).ToString() : "";

        // Get main window dimensions for tab size
        extern HWND g_hwnd;
        RECT rect;
        GetClientRect(g_hwnd, &rect);
        int width = rect.right - rect.left;
        int height = rect.bottom - rect.top;

        // Account for header height (12% for tab bar + toolbar)
        int shellHeight = (std::max)(100, static_cast<int>(height * 0.12));
        int tabHeight = height - shellHeight;

        int tab_id = TabManager::GetInstance().CreateTab(url, g_hwnd, 0, shellHeight, width, tabHeight);

        LOG_DEBUG_BROWSER("📑 Tab created: ID " + std::to_string(tab_id));

        // TODO: Send tab list update to frontend
        return true;
    }

    if (message_name == "tab_close") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() > 0) {
            int tab_id = args->GetInt(0);
            bool success = TabManager::GetInstance().CloseTab(tab_id);

            LOG_DEBUG_BROWSER("📑 Tab close: ID " + std::to_string(tab_id) +
                             (success ? " succeeded" : " failed"));

            // TODO: Send tab list update to frontend
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

    // Helper function to send tab list to frontend (used by get_tab_list and after tab creation)
    auto SendTabListToFrontend = []() {
        std::vector<Tab*> tabs = TabManager::GetInstance().GetAllTabs();
        int active_tab_id = TabManager::GetInstance().GetActiveTabId();

        // Build JSON response using nlohmann::json (handles escaping automatically)
        nlohmann::json response;
        response["activeTabId"] = active_tab_id;
        response["tabs"] = nlohmann::json::array();

        for (Tab* tab : tabs) {
            nlohmann::json tab_json;
            tab_json["id"] = tab->id;
            tab_json["title"] = tab->title;
            tab_json["url"] = tab->url;
            tab_json["isActive"] = (tab->id == active_tab_id);
            tab_json["isLoading"] = tab->is_loading;
            if (!tab->favicon_url.empty()) {
                tab_json["favicon"] = tab->favicon_url;
            }
            response["tabs"].push_back(tab_json);
        }

        std::string json_str = response.dump();

        // Send response to header browser
        CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
        if (header) {
            CefRefPtr<CefProcessMessage> cef_response = CefProcessMessage::Create("tab_list_response");
            CefRefPtr<CefListValue> response_args = cef_response->GetArgumentList();
            response_args->SetString(0, json_str);
            header->GetMainFrame()->SendProcessMessage(PID_RENDERER, cef_response);
            LOG_DEBUG_BROWSER("📑 Tab list sent to header: " + json_str);
        }
    };

    if (message_name == "get_tab_list") {
        SendTabListToFrontend();
        return true;
    }

    // ========== NAVIGATION MESSAGES ==========

    if (message_name == "navigate") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string path = args->GetString(0);

        // Normalize protocol
        if (!(path.rfind("http://", 0) == 0 || path.rfind("https://", 0) == 0)) {
            path = "http://" + path;
        }

        // Use TabManager to get active tab
        Tab* active_tab = TabManager::GetInstance().GetActiveTab();
        if (active_tab && active_tab->browser && active_tab->browser->GetMainFrame()) {
            active_tab->browser->GetMainFrame()->LoadURL(path);
            LOG_DEBUG_BROWSER("🔁 Navigate to " + path + " on active tab " + std::to_string(active_tab->id));
        } else {
            LOG_DEBUG_BROWSER("⚠️ No active tab available for navigation");
        }

        return true;
    }

    if (message_name == "navigate_back") {
        LOG_DEBUG_BROWSER("🔙 navigate_back message received from role: " + role_);

        Tab* active_tab = TabManager::GetInstance().GetActiveTab();
        if (active_tab && active_tab->browser) {
            active_tab->browser->GoBack();
            LOG_DEBUG_BROWSER("🔙 GoBack() called on active tab " + std::to_string(active_tab->id));
        } else {
            LOG_WARNING_BROWSER("⚠️ No active tab available for GoBack");
        }
        return true;
    }

    if (message_name == "navigate_forward") {
        LOG_DEBUG_BROWSER("🔜 navigate_forward message received from role: " + role_);

        Tab* active_tab = TabManager::GetInstance().GetActiveTab();
        if (active_tab && active_tab->browser) {
            active_tab->browser->GoForward();
            LOG_DEBUG_BROWSER("🔜 GoForward() called on active tab " + std::to_string(active_tab->id));
        } else {
            LOG_WARNING_BROWSER("⚠️ No active tab available for GoForward");
        }
        return true;
    }

    if (message_name == "navigate_reload") {
        LOG_DEBUG_BROWSER("🔄 navigate_reload message received from role: " + role_);

        Tab* active_tab = TabManager::GetInstance().GetActiveTab();
        if (active_tab && active_tab->browser) {
            active_tab->browser->Reload();
            LOG_DEBUG_BROWSER("🔄 Reload() called on active tab " + std::to_string(active_tab->id));
        } else {
            LOG_WARNING_BROWSER("⚠️ No active tab available for Reload");
        }
        return true;
    }

    // Duplicate address_generate handler removed - keeping the one at line 489


    if (message_name == "force_repaint") {
        LOG_DEBUG_BROWSER("🔄 Force repaint requested for " + role_ + " browser");

        if (browser) {
            browser->GetHost()->Invalidate(PET_VIEW);
            LOG_DEBUG_BROWSER("🔄 Browser invalidated for " + role_ + " browser");
        }
        return true;
    }

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

    if (message_name == "overlay_close") {
        LOG_DEBUG_BROWSER("🧠 [SimpleHandler] overlay_close message received");

        // Find and destroy overlay windows based on role
        HWND target_hwnd = nullptr;
        CefRefPtr<CefBrowser> target_browser = nullptr;

        if (role_ == "settings") {
            target_hwnd = FindWindow(L"CEFSettingsOverlayWindow", L"Settings Overlay");
            target_browser = GetSettingsBrowser();
            LOG_DEBUG_BROWSER("✅ Found settings overlay window: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        } else if (role_ == "wallet") {
            target_hwnd = FindWindow(L"CEFWalletOverlayWindow", L"Wallet Overlay");
            target_browser = GetWalletBrowser();
            LOG_DEBUG_BROWSER("✅ Found wallet overlay window: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        } else if (role_ == "backup") {
            target_hwnd = FindWindow(L"CEFBackupOverlayWindow", L"Backup Overlay");
            target_browser = GetBackupBrowser();
            LOG_DEBUG_BROWSER("✅ Found backup overlay window: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        } else if (role_ == "brc100auth") {
            extern HWND g_brc100_auth_overlay_hwnd;
            target_hwnd = g_brc100_auth_overlay_hwnd;
            target_browser = GetBRC100AuthBrowser();
            LOG_DEBUG_BROWSER("✅ Found BRC-100 auth overlay window: " + std::to_string(reinterpret_cast<uintptr_t>(target_hwnd)));
        }

        if (target_hwnd && IsWindow(target_hwnd)) {
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
        } else {
            LOG_DEBUG_BROWSER("❌ " + role_ + " overlay window not found");
        }

        return true;
    }

    if (false && message_name == "overlay_hide_NEVER_CALLED_12345") {
        LOG_DEBUG_BROWSER("🪟 Hiding overlay HWND");
        LOG_DEBUG_BROWSER("🪟 Before hide - EXSTYLE: 0x" + std::to_string(GetWindowLong(nullptr, GWL_EXSTYLE)));
        ShowWindow(nullptr, SW_HIDE);
        LOG_DEBUG_BROWSER("🪟 After hide - EXSTYLE: 0x" + std::to_string(GetWindowLong(nullptr, GWL_EXSTYLE)));
        return true;
    }

    if (message_name == "overlay_show_wallet") {
        LOG_DEBUG_BROWSER("💰 overlay_show_wallet message received from role: " + role_);

        LOG_DEBUG_BROWSER("💰 Creating wallet overlay with separate process");
        // Create new process for wallet overlay
        extern HINSTANCE g_hInstance;
        CreateWalletOverlayWithSeparateProcess(g_hInstance);
        return true;
    }

    if (message_name == "overlay_show_backup") {
        LOG_DEBUG_BROWSER("💾 overlay_show_backup message received from role: " + role_);

        LOG_DEBUG_BROWSER("💾 Creating backup overlay with separate process");
        // Create new process for backup overlay
        extern HINSTANCE g_hInstance;
        CreateBackupOverlayWithSeparateProcess(g_hInstance);
        return true;
    }

    if (message_name == "overlay_show_settings") {
        LOG_DEBUG_BROWSER("🪟 overlay_show_settings message received from role: " + role_);

        LOG_DEBUG_BROWSER("🪟 Creating settings overlay with separate process");
        // Create new process for settings overlay
        extern HINSTANCE g_hInstance;
        CreateSettingsOverlayWithSeparateProcess(g_hInstance);
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

            LOG_DEBUG_BROWSER("🔐 Auth request data - Domain: " + domain + ", Method: " + method + ", Endpoint: " + endpoint);

            // Store auth request data for the overlay to use
            extern void storePendingAuthRequest(const std::string& domain, const std::string& method, const std::string& endpoint, const std::string& body);
            storePendingAuthRequest(domain, method, endpoint, body);
        }

        LOG_DEBUG_BROWSER("🔐 Creating BRC-100 auth overlay with separate process");
        // Create new process for BRC-100 auth overlay
        extern HINSTANCE g_hInstance;
        CreateBRC100AuthOverlayWithSeparateProcess(g_hInstance);
        return true;
    }

    if (message_name == "overlay_hide") {
        LOG_DEBUG_BROWSER("🪟 overlay_hide message received from role: " + role_);

        // Close the BRC-100 auth overlay window
        HWND auth_hwnd = FindWindow(L"CEFBRC100AuthOverlayWindow", L"BRC-100 Auth Overlay");
        LOG_DEBUG_BROWSER("🪟 FindWindow result: " + std::to_string((uintptr_t)auth_hwnd));
        if (auth_hwnd) {
            LOG_DEBUG_BROWSER("🪟 Closing BRC-100 auth overlay window");
            DestroyWindow(auth_hwnd);
        } else {
            LOG_DEBUG_BROWSER("🪟 BRC-100 auth overlay window not found");
        }
        return true;
    }

    if (message_name == "brc100_auth_response") {
        LOG_DEBUG_BROWSER("🔐 brc100_auth_response message received from role: " + role_);

        // Extract response data from JSON
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        LOG_DEBUG_BROWSER("🔐 Auth response args size: " + std::to_string(args ? args->GetSize() : 0));
        if (args && args->GetSize() > 0) {
            std::string responseJson = args->GetString(0).ToString();
            LOG_DEBUG_BROWSER("🔐 Auth response JSON: " + responseJson);

            // Parse JSON response
            try {
                nlohmann::json responseData = nlohmann::json::parse(responseJson);
                bool approved = responseData["approved"];
                bool whitelist = responseData["whitelist"];

                LOG_DEBUG_BROWSER("🔐 Auth response - Approved: " + std::to_string(approved) + ", Whitelist: " + std::to_string(whitelist));

                if (approved) {
                    // User approved the authentication request
                    LOG_DEBUG_BROWSER("🔐 User approved auth request, generating authentication response");

                    // Get the pending auth request data from the HTTP interceptor
                    if (g_pendingAuthRequest.isValid) {
                        LOG_DEBUG_BROWSER("🔐 Found pending auth request, generating response for: " + g_pendingAuthRequest.domain);

                        // Create HTTP request to generate authentication response
                        CefRefPtr<CefRequest> cefRequest = CefRequest::Create();
                        cefRequest->SetURL("http://localhost:3301" + g_pendingAuthRequest.endpoint);
                        cefRequest->SetMethod(g_pendingAuthRequest.method);
                        cefRequest->SetHeaderByName("Content-Type", "application/json", true);

                        // Set the original request body
                        if (!g_pendingAuthRequest.body.empty()) {
                            CefRefPtr<CefPostData> postData = CefPostData::Create();
                            CefRefPtr<CefPostDataElement> element = CefPostDataElement::Create();
                            element->SetToBytes(g_pendingAuthRequest.body.length(), g_pendingAuthRequest.body.c_str());
                            postData->AddElement(element);
                            cefRequest->SetPostData(postData);
                        }

                        // Create a handler to process the authentication response
                        class AuthResponseHandler : public CefURLRequestClient {
                        public:
                            AuthResponseHandler(CefRefPtr<CefResourceHandler> originalHandler) : originalHandler_(originalHandler) {}

                            void OnRequestComplete(CefRefPtr<CefURLRequest> request) override {
                                CefURLRequest::Status status = request->GetRequestStatus();
                                if (status == UR_SUCCESS) {
                                    LOG_DEBUG_BROWSER("🔐 Authentication response generated successfully");

                                    // Send the response back to the original HTTP request
                                    if (!responseData_.empty()) {
                                        LOG_DEBUG_BROWSER("🔐 Sending auth response back to original request: " + responseData_);

                                        // Call the handleAuthResponse function in HttpRequestInterceptor
                                        extern void handleAuthResponse(const std::string& responseData);
                                        handleAuthResponse(responseData_);
                                    }
                                } else {
                                    LOG_DEBUG_BROWSER("🔐 Failed to generate authentication response (status: " + std::to_string(status) + ")");
                                }
                            }

                            void OnDownloadData(CefRefPtr<CefURLRequest> request, const void* data, size_t data_length) override {
                                // Store the response data
                                responseData_.append(static_cast<const char*>(data), data_length);
                            }

                            void OnUploadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {}
                            void OnDownloadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {}
                            bool GetAuthCredentials(bool isProxy, const CefString& host, int port, const CefString& realm, const CefString& scheme, CefRefPtr<CefAuthCallback> callback) override { return false; }

                        private:
                            std::string responseData_;
                            CefRefPtr<CefResourceHandler> originalHandler_;
                            IMPLEMENT_REFCOUNTING(AuthResponseHandler);
                            DISALLOW_COPY_AND_ASSIGN(AuthResponseHandler);
                        };

                        // Make the HTTP request to generate the authentication response
                        CefRefPtr<CefURLRequest> authRequest = CefURLRequest::Create(
                            cefRequest,
                            new AuthResponseHandler(g_pendingAuthRequest.handler),
                            nullptr
                        );

                        LOG_DEBUG_BROWSER("🔐 Authentication request sent to wallet at localhost:3301");

                        // Don't clear the pending request here - it will be cleared in handleAuthResponse
                    } else {
                        LOG_DEBUG_BROWSER("🔐 No pending auth request found");
                    }
                } else {
                    // User rejected the authentication request
                    LOG_DEBUG_BROWSER("🔐 User rejected auth request");

                    // Clear the pending request
                    g_pendingAuthRequest.isValid = false;
                }
            } catch (const std::exception& e) {
                LOG_DEBUG_BROWSER("🔐 Error parsing auth response JSON: " + std::string(e.what()));
            }
        } else {
            LOG_DEBUG_BROWSER("🔐 Invalid arguments for brc100_auth_response");
        }
        return true;
    }

    if (message_name == "add_domain_to_whitelist") {
        LOG_DEBUG_BROWSER("🔐 add_domain_to_whitelist message received from role: " + role_);

        // Extract domain and permanent flag from JSON
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        LOG_DEBUG_BROWSER("🔐 Args size: " + std::to_string(args ? args->GetSize() : 0));
        if (args && args->GetSize() > 0) {
            std::string whitelistJson = args->GetString(0).ToString();
            LOG_DEBUG_BROWSER("🔐 Whitelist JSON: " + whitelistJson);

            // Parse JSON data
            try {
                nlohmann::json whitelistData = nlohmann::json::parse(whitelistJson);
                std::string domain = whitelistData["domain"];
                bool permanent = whitelistData["permanent"];

                LOG_DEBUG_BROWSER("🔐 Adding domain to whitelist - Domain: " + domain + ", Permanent: " + std::to_string(permanent));

                // Call the domain whitelist API
                extern void addDomainToWhitelist(const std::string& domain, bool permanent);
                addDomainToWhitelist(domain, permanent);
            } catch (const std::exception& e) {
                LOG_DEBUG_BROWSER("🔐 Error parsing whitelist JSON: " + std::string(e.what()));
            }
        } else {
            LOG_DEBUG_BROWSER("🔐 Invalid arguments for add_domain_to_whitelist");
        }
        return true;
    }


    if (message_name == "test_settings_message") {
        LOG_DEBUG_BROWSER("🧪 test_settings_message received from role: " + role_);
        return true;
    }

    if (false && message_name == "overlay_hide_NEVER_CALLED_67890" && role_ == "settings") {
        LOG_DEBUG_BROWSER("🪟 overlay_hide message received for settings overlay");

        // Close the settings overlay window
        HWND settings_hwnd = FindWindow(L"CEFSettingsOverlayWindow", L"Settings Overlay");
        if (settings_hwnd) {
            LOG_DEBUG_BROWSER("🪟 Closing settings overlay window");
            DestroyWindow(settings_hwnd);
        }
        return true;
    }

    if (message_name == "overlay_input") {
        LOG_DEBUG_BROWSER("🪟 overlay_input message received from role: " + role_);

        CefRefPtr<CefListValue> args = message->GetArgumentList();
        bool enable = args->GetBool(0);
        LOG_DEBUG_BROWSER("🪟 Setting overlay input: " + std::string(enable ? "enabled" : "disabled") + " for role: " + role_);

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
        return true;
    }

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

    // Transaction Message Handlers

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

    return false;
}

CefRefPtr<CefRequestHandler> SimpleHandler::GetRequestHandler() {
    return this;
}

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

    // Intercept HTTP requests for all browsers when they're making external requests
    // Check if the request is to localhost ports that BRC-100 sites commonly use
    // OR if it's a BRC-104 /.well-known/auth request (standard wallet authentication endpoint)
    if (url.find("localhost:3301") != std::string::npos ||
        url.find("localhost:3321") != std::string::npos ||
        url.find("localhost:2121") != std::string::npos ||
        url.find("localhost:8080") != std::string::npos ||
        url.find("messagebox.babbage.systems") != std::string::npos ||
        url.find("/.well-known/auth") != std::string::npos) {
        LOG_DEBUG_BROWSER("🌐 Intercepting wallet request from browser role: " + role_);
        return new HttpRequestInterceptor();
    }

    // For other requests, use default handling
    return nullptr;
}

CefRefPtr<CefContextMenuHandler> SimpleHandler::GetContextMenuHandler() {
    return this;
}

CefRefPtr<CefKeyboardHandler> SimpleHandler::GetKeyboardHandler() {
    return this;
}

bool SimpleHandler::OnPreKeyEvent(CefRefPtr<CefBrowser> browser,
                                  const CefKeyEvent& event,
                                  CefEventHandle os_event,
                                  bool* is_keyboard_shortcut) {
    // Log keyboard events for debugging
    LOG_DEBUG_BROWSER("⌨️ OnPreKeyEvent - type: " + std::to_string(event.type) +
                      ", key: " + std::to_string(event.windows_key_code) +
                      ", modifiers: " + std::to_string(event.modifiers));

    // For overlay windows, we want normal input processing, not shortcuts
    if (role_ == "wallet" || role_ == "settings") {
        *is_keyboard_shortcut = false;
        return false; // Let the event be processed normally
    }

    return false; // Let other handlers process the event
}

void SimpleHandler::SetRenderHandler(CefRefPtr<CefRenderHandler> handler) {
    render_handler_ = handler;
}

CefRefPtr<CefRenderHandler> SimpleHandler::GetRenderHandler() {
    return render_handler_;
}

void SimpleHandler::OnBeforeContextMenu(CefRefPtr<CefBrowser> browser,
                                        CefRefPtr<CefFrame> frame,
                                        CefRefPtr<CefContextMenuParams> params,
                                        CefRefPtr<CefMenuModel> model) {
    // Enable DevTools for overlay windows in development
    if (role_ == "settings" || role_ == "wallet" || role_ == "backup" || role_ == "brc100auth") {
        // Add Inspect Element option - use custom menu ID
        const int MENU_ID_DEV_TOOLS_INSPECT = MENU_ID_USER_FIRST + 1;
        model->AddItem(MENU_ID_DEV_TOOLS_INSPECT, "Inspect Element");
        model->AddSeparator();

        LOG_DEBUG_BROWSER("🔧 Context menu enabled for " + role_ + " overlay - DevTools available");
    }
}

bool SimpleHandler::OnContextMenuCommand(CefRefPtr<CefBrowser> browser,
                                         CefRefPtr<CefFrame> frame,
                                         CefRefPtr<CefContextMenuParams> params,
                                         int command_id,
                                         EventFlags event_flags) {
    // Log all context menu commands for debugging
    LOG_DEBUG_BROWSER("🔘 Context menu command: " + std::to_string(command_id) + " (role: " + role_ + ")");

    // Handle DevTools for overlay windows
    if ((role_ == "settings" || role_ == "wallet" || role_ == "backup" || role_ == "brc100auth") && command_id == (MENU_ID_USER_FIRST + 1)) {
        // Open DevTools
        browser->GetHost()->ShowDevTools(CefWindowInfo(), nullptr, CefBrowserSettings(), CefPoint());
        LOG_DEBUG_BROWSER("🔧 DevTools opened for " + role_ + " overlay");
        return true;
    }

    // Intercept "Open link in new tab" command (Chromium internal command ID: 50100)
    // This is called when user right-clicks and selects "Open in new tab"
    // OnBeforePopup is NOT called for this action, so we handle it here
    if (command_id == 50100 && role_.find("tab_") == 0) {
        std::string link_url = params->GetLinkUrl().ToString();

        if (!link_url.empty()) {
            LOG_DEBUG_BROWSER("📑 Intercepting 'Open in new tab' command, creating tab for: " + link_url);

            // Get main window dimensions
            extern HWND g_hwnd;
            RECT rect;
            GetClientRect(g_hwnd, &rect);
            int width = rect.right - rect.left;
            int height = rect.bottom - rect.top;
            int shellHeight = (std::max)(100, static_cast<int>(height * 0.12));
            int tabHeight = height - shellHeight;

            // Create new tab with the link URL
            TabManager::GetInstance().CreateTab(link_url, g_hwnd, 0, shellHeight, width, tabHeight);

            // Return true to prevent default behavior (opening in separate window)
            return true;
        } else {
            LOG_DEBUG_BROWSER("⚠️ Command 50100 called but no link URL available");
        }
    }

    // Allow default handling for other commands
    return false;
}
