#pragma once

#include "include/cef_cookie.h"
#include "include/cef_browser.h"
#include "include/cef_task.h"

#include <string>

// CookieManager: Static methods for cookie and cache management via CEF APIs.
// Cookies are managed by CEF internally (no custom database).
// All Handle* methods are called from the browser process UI thread
// (OnProcessMessageReceived) and send responses back to the renderer.
//
// Phase 8c batch 3 (2026-09-12): every entry point takes the bridge request id (arg 0
// of the IPC) and every reply is `[requestId, json]`, so the render process resolves the
// CALLER's promise instead of whichever `window.on*` global happened to be set. Several
// of these replies are produced asynchronously (cookie visitor, delete callback, cache
// walk), which is why the id is threaded through the helper classes rather than read
// back at send time.
class CookieManager {
public:
    // Enumerate all cookies and send JSON array to renderer
    static void HandleGetAllCookies(CefRefPtr<CefBrowser> browser, int requestId);

    // Delete a single cookie by URL and name
    static void HandleDeleteCookie(CefRefPtr<CefBrowser> browser,
                                   int requestId,
                                   const std::string& url,
                                   const std::string& name);

    // Delete all cookies for a given domain
    static void HandleDeleteDomainCookies(CefRefPtr<CefBrowser> browser,
                                          int requestId,
                                          const std::string& domain);

    // Delete every cookie
    static void HandleDeleteAllCookies(CefRefPtr<CefBrowser> browser, int requestId);

    // Clear browser cache via Chrome DevTools Protocol
    static void HandleClearCache(CefRefPtr<CefBrowser> browser, int requestId);

    // Get total cache directory size in bytes
    static void HandleGetCacheSize(CefRefPtr<CefBrowser> browser, int requestId);

private:
    CookieManager() = delete; // Static-only class, no instances
};
