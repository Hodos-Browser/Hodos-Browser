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

// Reject counterpart. Called from simple_handler.cpp's brc100_auth_response
// reject branch when the user declines a domain_approval modal for BRC-121.
// Drains the pending reload queue for the domain and navigates each waiting
// tab back (or to about:blank if no history), so the payment-pending
// placeholder doesn't linger after rejection.
void CancelPendingBrc121Reloads(const std::string& domain);

// Called from simple_handler.cpp's OnLoadError to decide whether the failed
// load is a BRC-121 402 awaiting user approval. If so, the OnLoadError
// handler swaps the data:text/html "Failed to load" page for a clean
// /payment-pending placeholder URL with the spinning Hodos logo.
bool HasPendingBrc121ReloadForDomain(const std::string& domain);

// Per-domain price snapshot stored when the most recent 402 is detected for
// the unapproved-domain branch. Read by OnLoadError to build the placeholder
// URL with the right "X sats" caption. 0 if no price has been stored.
int64_t GetPendingBrc121PriceForDomain(const std::string& domain);

// B+3 polish — mark a BRC-121 article URL as one-shot approved. Called from
// simple_handler.cpp's brc100_auth_response handler when the user approves
// a payment_confirmation / rate_limit_exceeded modal whose stored endpoint
// is an http(s) article URL. On the next 402 for that URL,
// TryHandleBrc121_402 atomically pops the marker and bypasses the cap check
// exactly once, so the user's just-approved payment proceeds without
// re-prompting. Subsequent visits to the same URL re-check caps normally.
void MarkBrc121PaymentApproved(const std::string& url);


// Phase 1 polish — failed-URL registry. Async402ResourceHandler registers a
// URL after MAX_UPSTREAM_RETRIES with a non-2xx upstream status. OnLoadError
// in simple_handler.cpp consumes the entry and swaps the failed-load page
// for /payment-failed (Hodos error page with Retry button). One-shot.
void RegisterBrc121FailedUrl(const std::string& url,
                             const std::string& domain,
                             int64_t satoshis,
                             int upstreamStatus);

bool ConsumeBrc121FailedUrl(const std::string& url,
                            std::string& outDomain,
                            int64_t& outSatoshis,
                            int& outStatus);

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

// Phase 1.5 Step 1 — privacy-perimeter "Always allow for this site" opt-ins.
// In-memory cache only for key-linkage; identity-key now persists via the new
// domain_permissions.identity_key_disclosure_allowed column (V17). These are
// safe no-ops if storage drifts -- the gate falls back to prompting.
void MarkIdentityKeyRevealApproved(const std::string& domain);
void MarkKeyLinkageRevealApproved(const std::string& domain);

// Phase 1.5 Step 1 — forward a queued AsyncWalletResourceHandler entry to Rust.
// Used by simple_handler.cpp's add_domain_permission{,_advanced} drain path to
// resume sibling requests after the user approves a domain. Returns false if
// the handler is null (BRC-121 nullptr-handler entry — caller handles via
// TriggerPendingBrc121Reloads instead). Implementation casts to the file-local
// AsyncWalletResourceHandler class inside HttpRequestInterceptor.cpp.
bool ForwardPendingWalletRequest(CefRefPtr<CefResourceHandler> handler);
