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

// Phase 2.5 Commit 6 (sub-step 6.b) — single source of truth for the
// post-success "auto-approved payment" cluster: recordSpending +
// payment_success_indicator IPC (green-dot tab animation). Bundles the
// SessionManager update and the React-side indicator dispatch so callers
// on every path (HTTP createAction, BRC-121 paid retry, IPC bridge) fire
// the same code. Counter increments (rateCounter / paymentCount) stay
// with each caller because they happen at different lifecycle stages
// across paths (silent-approve time for createAction, success time for
// BRC-121).
//
// Guarded internally: returns immediately when wasAutoApprovedPayment is
// false or cents <= 0. The endpoint string is diagnostic only (passed to
// the debug log). Safe to call from any thread that the SessionManager
// + SimpleHandler::GetHeaderBrowser accessors permit (currently any
// thread — both are mutex-protected / refcounted).
void OnWalletCallSuccess(int browserId,
                         const std::string& domain,
                         int64_t cents,
                         bool wasAutoApprovedPayment,
                         const std::string& endpoint);

// ============================================================================
// Phase 2.5 Commit 6 (sub-step 6.c) — Decision 3: free-function modal openers
// ============================================================================
//
// Each opener fully handles the modal dispatch sequence:
//   1. Builds a PendingAuthRequest from ModalContext + ResumeContext.
//   2. Enrolls it via PendingRequestManager::addRequest (the new
//      PendingAuthRequest-by-value overload landed in 6.a).
//   3. Posts CreateNotificationOverlayTask to TID_UI.
//
// ResumeContext discriminates resume semantics:
//   - HTTP path: pass {handler = self, frame = null, ...} → request gets
//     resumeKind = kHttpCallback (handler-driven resume).
//   - IPC path: pass {handler = null, frame = capturedFrame, browserId,
//     headersOnApprove, httpMethod} → request gets resumeKind = kIpcResponse
//     (frame-driven wallet_response resume).
//
// The existing AsyncWalletResourceHandler::triggerXxxModal member functions
// are now thin wrappers that delegate to these free functions. New
// 6.d/6.e IPC-side code calls the free functions directly with kIpcResponse
// ResumeContext values.
//
// No call site behavior change is intended in 6.c — the member-trigger
// callers (Open()'s lambdas, unknown-trust branch, safety-net, fallback,
// drain path) all see the same external semantics.

#include "include/cef_browser.h"   // CefRefPtr<CefFrame> for ResumeContext

namespace hodos { struct Manifest; }
struct CertDisclosureInfo;          // forward decl; lives in HttpRequestInterceptor.cpp

// Context shared by every modal opener. Carries the request's identity for
// PendingAuthRequest enrollment + React-side rendering.
struct ModalContext {
    std::string domain;     // host[:port], used as the per-domain queue key
    std::string method;     // HTTP method on the calling request (e.g. "POST")
    std::string endpoint;   // wallet route ("/createAction", etc.)
    std::string body;       // JSON body — modal may parse for display data
};

// Discriminator for how a resolved request resumes.
// Populated by the caller before calling an opener; the opener writes the
// fields straight into the new PendingAuthRequest.
struct ResumeContext {
    CefRefPtr<CefResourceHandler> handler;       // non-null iff HTTP path
    CefRefPtr<CefFrame> frame;                   // non-null iff IPC path
    int browserId = 0;
    std::map<std::string, std::string> headersOnApprove;
    std::string httpMethod = "POST";
};

// Modal openers, one per gate type. Each enrolls the PendingAuthRequest and
// fires the matching CreateNotificationOverlayTask. Modals with typed
// payloads (manifest, cert) take an additional argument; simple modals share
// the (ctx, resume, extraParams) shape.
//
// Returns the newly-created PendingAuthRequest requestId. The IPC path uses
// this to arm postIpcAuthTimeout on the same request. HTTP-path callers
// (member trigger delegates) currently ignore the return value.
std::string openDomainApprovalModal(const ModalContext& ctx, const ResumeContext& resume);
std::string openBRC100AuthApprovalModal(const ModalContext& ctx, const ResumeContext& resume);
std::string openManifestConnectBundleModal(const ModalContext& ctx, const ResumeContext& resume, const hodos::Manifest& manifest);
std::string openIdentityKeyRevealModal(const ModalContext& ctx, const ResumeContext& resume);
std::string openKeyLinkageRevealModal(const ModalContext& ctx, const ResumeContext& resume);
std::string openPaymentConfirmationModal(const ModalContext& ctx, const ResumeContext& resume, const std::string& extraParams);
std::string openRateLimitExceededModal(const ModalContext& ctx, const ResumeContext& resume, const std::string& extraParams);
std::string openProtocolPermissionPromptModal(const ModalContext& ctx, const ResumeContext& resume, const std::string& extraParams);
std::string openBasketPermissionPromptModal(const ModalContext& ctx, const ResumeContext& resume, const std::string& extraParams);
std::string openCounterpartyPermissionPromptModal(const ModalContext& ctx, const ResumeContext& resume, const std::string& extraParams);
std::string openCertificateDisclosureModal(const ModalContext& ctx, const ResumeContext& resume, const CertDisclosureInfo& info);

// String-keyed dispatcher used by the IPC path's openModal callback (where
// the engine's PermissionDecision::promptType is the input). For modals that
// require typed payloads (manifest, cert), callers must invoke the matching
// opener directly — those are unreachable via this dispatcher because their
// payloads cannot be expressed as a URL-style extraParams string.
//
// Returns the requestId of the enrolled PendingAuthRequest, or empty string
// if no opener matched the promptType.
std::string OpenPromptModal(const std::string& promptType,
                            const ModalContext& ctx,
                            const ResumeContext& resume,
                            const std::string& extraParams = "");

// ============================================================================
// Phase 2.5 Commit 6 sub-step 6.d — IPC path support helpers
// ============================================================================

// Exact-or-port-suffix check for internal origins (Hodos's own UI). Matches:
//   "127.0.0.1"      "127.0.0.1:31301"     "localhost"     "localhost:5137"     ""
// Does NOT match:
//   "127.0.0.1.evil.com"  "localhost.evil.com"  "localhostevil.com"
// Replaces the pre-existing prefix-match check at the top of Open() (which
// had this defense-in-depth weakness). Used by both HTTP path's Open() and
// the new IPC path's HandleIpcWalletCall.
bool IsInternalOrigin(const std::string& origin);

// IPC-side auth timeout. Mirrors AsyncWalletResourceHandler::postAuthTimeout
// but for requests with resumeKind == kIpcResponse (no handler instance to
// call back). After delayMs, if the request is still pending (not yet
// resolved by user Approve/Deny), pops it from PendingRequestManager and
// sends wallet_response IPC with errorJson. Frame validity is checked
// before SendProcessMessage.
void postIpcAuthTimeout(const std::string& requestId,
                        CefRefPtr<CefFrame> frame,
                        const std::string& errorJson,
                        int delayMs);

// Top-level entry point for wallet_call IPC dispatch. Encapsulates the
// internal-origin bypass, wallet-existence check, blocked/unknown/approved
// trust dispatch, modal opening, and response routing.
//
// Threading: invoked from simple_handler.cpp's wallet_call IPC handler on
// the UI thread. May post tasks to TID_FILE_USER_BLOCKING internally for
// SyncHttpClient calls and to TID_UI for response IPC.
//
// Phase 2.5 Commit 6 sub-step 6.d. After this lands AND 6.e closes the
// approve/deny resume loop, the engine cascade fires from external dApp
// traffic for the first time.
void HandleIpcWalletCall(
    const std::string& requestId,
    const std::string& methodName,
    const std::string& endpoint,
    const std::string& bodyJson,
    const std::string& httpMethod,
    const std::string& origin,
    CefRefPtr<CefFrame> capturedFrame,
    int browserId);
