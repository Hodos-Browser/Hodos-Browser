#pragma once

#include "include/cef_resource_request_handler.h"
#include "include/cef_resource_handler.h"
#include "include/cef_request.h"
#include "include/cef_response.h"
#include "include/cef_callback.h"
#include "include/cef_browser.h"
#include "include/cef_frame.h"
#include "include/cef_urlrequest.h"
#include <string>

// BRC-121 (Simple HTTP 402 Payment) detection. Shared between
// HttpRequestInterceptor and CookieFilterResourceHandler so 402 detection
// fires for *every* external HTTP response, not only wallet endpoints.
//
// Returns true if a BRC-121 402 was detected, the wallet was called,
// the x-bsv-payment header was attached to `request`, and the caller
// should `return true` from OnResourceResponse to make CEF retry.
// Returns false in all other cases (caller should `return false` so the
// response propagates to the page normally).
bool TryHandleBrc121_402(CefRefPtr<CefBrowser> browser,
                         CefRefPtr<CefFrame> frame,
                         CefRefPtr<CefRequest> request,
                         CefRefPtr<CefResponse> response);

// Called from simple_handler.cpp after a domain is approved. Reloads any
// browser that hit a BRC-121 402 for this domain and is now stuck on the
// CEF error page (data:text/html ERR_HTTP_RESPONSE_CODE_FAILURE), navigating
// it back to the original URL so the auto-approve path can run.
void TriggerPendingBrc121Reloads(const std::string& domain);

// Called from each CefResourceRequestHandler's GetResourceHandler. If the
// (browserId, url) on this request has a registered paid retry context
// (set by TryHandleBrc121_402 after a successful /wallet/pay402 call),
// returns an Async402ResourceHandler that will issue the paid request
// with all 5 BRC-121 headers, broadcast the nosend tx after the server
// returns 200, fire the green-dot animation IPC, and stream the response
// body to the page. Returns nullptr if no paid retry is pending —
// caller should return its own default (or nullptr for default CEF handling).
CefRefPtr<CefResourceHandler> InstallAsync402HandlerIfPending(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefRequest> request);

class HttpRequestInterceptor : public CefResourceRequestHandler {
public:
    HttpRequestInterceptor();
    virtual ~HttpRequestInterceptor();

    // CefResourceRequestHandler methods
    CefRefPtr<CefResourceHandler> GetResourceHandler(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefFrame> frame,
        CefRefPtr<CefRequest> request) override;

    CefRefPtr<CefCookieAccessFilter> GetCookieAccessFilter(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefFrame> frame,
        CefRefPtr<CefRequest> request) override;

    void OnResourceRedirect(CefRefPtr<CefBrowser> browser,
                           CefRefPtr<CefFrame> frame,
                           CefRefPtr<CefRequest> request,
                           CefRefPtr<CefResponse> response,
                           CefString& new_url) override;

    bool OnResourceResponse(CefRefPtr<CefBrowser> browser,
                           CefRefPtr<CefFrame> frame,
                           CefRefPtr<CefRequest> request,
                           CefRefPtr<CefResponse> response) override;

    // Public helper: extracts the page's origin domain from the browser
    // (main frame URL) or referrer fallback. Used by free functions in
    // this translation unit AND by CookieFilterResourceHandler in
    // simple_handler.cpp via the TryHandleBrc121_402 path.
    static std::string extractDomain(CefRefPtr<CefBrowser> browser,
                                     CefRefPtr<CefRequest> request);

private:
    // Helper methods
    bool isWalletEndpoint(const std::string& url);
    bool isSocketIOConnection(const std::string& url);

    IMPLEMENT_REFCOUNTING(HttpRequestInterceptor);
    DISALLOW_COPY_AND_ASSIGN(HttpRequestInterceptor);
};

// Global functions for BRC-100 auth modal
void sendAuthRequestDataToOverlay();
void handleAuthResponse(const std::string& requestId, const std::string& responseData);
void handleAuthResponse(const std::string& responseData);  // legacy overload
