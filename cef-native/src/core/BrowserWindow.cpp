#include "../../include/core/BrowserWindow.h"

BrowserWindow::BrowserWindow(int id) : window_id(id) {}

void BrowserWindow::SetBrowserForRole(const std::string& role, CefRefPtr<CefBrowser> browser) {
    if (role == "header")            header_browser = browser;
    else if (role == "webview")      webview_browser = browser;
    else if (role == "wallet_panel") wallet_panel_browser = browser;
    else if (role == "overlay")      overlay_browser = browser;
    else if (role == "settings")     settings_browser = browser;
    else if (role == "wallet")       wallet_browser = browser;
    else if (role == "backup")       backup_browser = browser;
    else if (role == "brc100auth")   brc100_auth_browser = browser;
    else if (role == "notification") notification_browser = browser;
    else if (role == "settings_menu") settings_menu_browser = browser;
    else if (role == "omnibox")      omnibox_browser = browser;
    else if (role == "cookiepanel")  cookie_panel_browser = browser;
    else if (role == "downloadpanel") download_panel_browser = browser;
    else if (role == "profilepanel") profile_panel_browser = browser;
    else if (role == "menu")         menu_browser = browser;
}

CefRefPtr<CefBrowser> BrowserWindow::GetBrowserForRole(const std::string& role) const {
    if (role == "header")            return header_browser;
    if (role == "webview")           return webview_browser;
    if (role == "wallet_panel")      return wallet_panel_browser;
    if (role == "overlay")           return overlay_browser;
    if (role == "settings")          return settings_browser;
    if (role == "wallet")            return wallet_browser;
    if (role == "backup")            return backup_browser;
    if (role == "brc100auth")        return brc100_auth_browser;
    if (role == "notification")      return notification_browser;
    if (role == "settings_menu")     return settings_menu_browser;
    if (role == "omnibox")           return omnibox_browser;
    if (role == "cookiepanel")       return cookie_panel_browser;
    if (role == "downloadpanel")     return download_panel_browser;
    if (role == "profilepanel")      return profile_panel_browser;
    if (role == "menu")              return menu_browser;
    return nullptr;
}

void BrowserWindow::ClearBrowserForRole(const std::string& role) {
    SetBrowserForRole(role, nullptr);
}
