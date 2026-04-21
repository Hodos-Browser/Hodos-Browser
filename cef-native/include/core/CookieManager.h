#pragma once

#include "include/cef_cookie.h"
#include "include/cef_browser.h"
#include "include/cef_task.h"

#include <string>

// CookieManager: Static methods for cookie and cache management via CEF APIs.
// Cookies are managed by CEF internally (no custom database).
// All Handle* methods are called from the browser process UI thread
// (OnProcessMessageReceived) and send responses back to the renderer.
class CookieManager {
public:
    // Enumerate all cookies and send JSON array to renderer
    static void HandleGetAllCookies(CefRefPtr<CefBrowser> browser);

    // Delete a single cookie by URL and name
    static void HandleDeleteCookie(CefRefPtr<CefBrowser> browser,
                                   const std::string& url,
                                   const std::string& name);

    // Delete all cookies for a given domain
    static void HandleDeleteDomainCookies(CefRefPtr<CefBrowser> browser,
                                          const std::string& domain);

    // Delete every cookie
    static void HandleDeleteAllCookies(CefRefPtr<CefBrowser> browser);

    // Clear browser cache via Chrome DevTools Protocol
    static void HandleClearCache(CefRefPtr<CefBrowser> browser);

    // Get total cache directory size in bytes
    static void HandleGetCacheSize(CefRefPtr<CefBrowser> browser);

private:
    CookieManager() = delete; // Static-only class, no instances
};
