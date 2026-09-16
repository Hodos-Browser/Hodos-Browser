#pragma once

#include "include/cef_resource_handler.h"
#include "include/cef_frame.h"
#include <string>
#include <vector>
#include <map>
#include <mutex>
#include <chrono>

// Phase 2.5 Commit 6 (sub-step 6.a) — discriminator for how a resolved
// pending request resumes work. The HTTP path keeps its existing behavior
// (kHttpCallback); the IPC bridge introduces kIpcResponse so modal
// resolution can re-issue the wallet call on a worker thread and send
// wallet_response back to the original frame. kInternal is reserved for
// the Phase 2.6 engine-to-Rust migration where Rust-initiated pending
// state will live in C++ for modal dispatch only.
//
// Default value (kHttpCallback) makes the existing parameter-list
// addRequest overload preserve today's HTTP-path semantics with zero
// caller changes — only the new addRequest(PendingAuthRequest) overload
// constructs IPC-flavored entries.
enum class ResumeKind {
    kHttpCallback,   // Resume via handler->onAuthResponseReceived
    kIpcResponse,    // Resume via frame->SendProcessMessage(wallet_response)
    kInternal,       // Reserved for Phase 2.6 Rust-initiated requests
};

struct PendingAuthRequest {
    std::string requestId;
    std::string domain;
    std::string method;
    std::string endpoint;
    std::string body;
    std::string type;  // "domain_approval", "brc100_auth", "no_wallet", "payment_confirmation", "rate_limit_exceeded", "certificate_disclosure", scoped-grant types
    CefRefPtr<CefResourceHandler> handler;  // valid iff resumeKind == kHttpCallback

    // Phase 2.5 Commit 6 — IPC path resume state.
    // Defaults preserve HTTP-path semantics: existing addRequest call sites
    // leave these zero/empty/null, and handleAuthResponse's switch on
    // resumeKind treats them as kHttpCallback (today's behavior unchanged).
    ResumeKind resumeKind = ResumeKind::kHttpCallback;
    CefRefPtr<CefFrame> frame;                            // valid iff resumeKind == kIpcResponse
    int browserId = 0;                                    // valid iff resumeKind == kIpcResponse
    std::map<std::string, std::string> headersOnApprove;  // injected by handleAuthResponse on Approve
    std::string httpMethod = "POST";                      // for IPC re-issue ("GET"/"POST"/"DELETE"/"PUT"/"PATCH")

    // Phase 2.6-C.5 fix — page-supplied requestId from the original wallet_call
    // IPC message. The CWI shim (and other wallet shims) register a JS-side
    // promise callback keyed by this id; on resume, sendWalletResponseIpc MUST
    // use this id rather than the C++-generated `requestId` field above, or
    // the page's promise never resolves and the page hangs. Empty when the
    // entry was enrolled via the HTTP path (kHttpCallback) where there is no
    // page-side IPC promise — the resource handler delivers the response
    // directly through CEF's URLRequest pipeline.
    std::string originalIpcRequestId;

    // beta.3 Phase 10b — one prompt on screen at a time. The notification overlay
    // is a single keep-alive page, so a second prompt posted while one is shown
    // used to REPLACE it on screen while both stayed pending — and one click then
    // resolved both (CU-1, measured). Now a prompt that arrives while another is
    // shown waits here and is posted when the shown one is resolved or expires.
    std::string overlayType;          // non-empty ⇒ this entry owns a modal (kind or connect)
    std::string overlayExtraParams;   // the query extras to post it with, later
    bool shown = false;               // its modal is (or was last) on screen
    std::chrono::steady_clock::time_point shownAt{};
    // beta.3 Phase 10b panel `F1` — when the entry was enrolled. A prompt that
    // has waited longer than the answer is worth must never be posted: 10b made
    // prompts WAIT instead of replacing each other, which opened a window in
    // which a queued entry could reach the screen long after its own transport
    // gave up on it. Answering one of those spends money into a response nobody
    // is listening for.
    std::chrono::steady_clock::time_point createdAt{std::chrono::steady_clock::now()};
    uint64_t seq = 0;                 // arrival order, for FIFO
};

class PendingRequestManager {
public:
    static PendingRequestManager& GetInstance() {
        static PendingRequestManager instance;
        return instance;
    }

    // Store a request, returns the generated requestId. HTTP-path overload —
    // unchanged from Phase 1.5; new IPC-resume fields default to kHttpCallback
    // semantics so existing call sites in Open()'s lambdas continue to work
    // without any modification.
    std::string addRequest(const std::string& domain,
                           const std::string& method,
                           const std::string& endpoint,
                           const std::string& body,
                           CefRefPtr<CefResourceHandler> handler,
                           const std::string& type = "domain_approval") {
        std::lock_guard<std::mutex> lock(mutex_);
        std::string id = generateId();
        PendingAuthRequest req;
        req.requestId = id;
        req.domain = domain;
        req.method = method;
        req.endpoint = endpoint;
        req.body = body;
        req.type = type;
        req.handler = handler;
        // resumeKind defaults to kHttpCallback; other Commit-6 fields stay default.
        requests_[id] = req;
        return id;
    }

    // Phase 2.5 Commit 6 (sub-step 6.a) — fully-constructed-request overload.
    // The IPC path builds its own PendingAuthRequest (with resumeKind ==
    // kIpcResponse + frame + browserId + headersOnApprove + httpMethod set)
    // and hands it over by value. requestId is generated here and written
    // back into the moved struct before storage. Returns the new requestId.
    std::string addRequest(PendingAuthRequest req) {
        std::lock_guard<std::mutex> lock(mutex_);
        std::string id = generateId();
        req.requestId = id;
        requests_[id] = std::move(req);
        return id;
    }

    // beta.3 Phase 0.8 (`P0.8-A4`) — atomic "register, and tell me whether I am
    // the first for this domain".
    //
    // 🚨 Callers used to do this in two steps:
    //
    //     bool showing = hasPendingForDomain(domain);   // lock #1
    //     std::string id = addRequest(req);             // lock #2
    //     if (showing) { queue } else { open a modal }
    //
    // Those are two independent acquisitions of `mutex_`, so N concurrent
    // requests from ONE user gesture can all observe "none pending" before any
    // of them inserts, and all N open a modal. MEASURED 2026-08-21: a single
    // click on bitgenius.net produced three notification overlays in 56 ms,
    // three 202s and three minted approval ids, all for `/getVersion`.
    //
    // ⛔ Do not reintroduce the check-then-act pair. `wasFirstForDomain` is
    // written under the same lock that performs the insert, so exactly one
    // caller per domain can ever see `true`.
    std::string addRequestIfFirstForDomain(PendingAuthRequest req, bool& wasFirstForDomain,
                                           bool& showNow) {
        std::lock_guard<std::mutex> lock(mutex_);
        wasFirstForDomain = true;
        for (const auto& pair : requests_) {
            if (pair.second.domain == req.domain) { wasFirstForDomain = false; break; }
        }
        // beta.3 Phase 10e (panel `F2-10b`) — ⛔ being first for YOUR domain is not
        // permission to take the screen.
        //
        // This used to set `shown = true` on the strength of `wasFirstForDomain`
        // alone, while the queue's own gate (`anyLiveShownLocked`) is GLOBAL. So a
        // connect prompt from a second origin — an iframe to a domain the page
        // controls is enough — painted over a payment prompt that was already up,
        // leaving TWO entries flagged shown. Answering the connect one then called
        // `ShowNextQueuedPrompt`, which saw the painted-over entry still flagged and
        // refused to advance: its modal was gone and never came back, and EVERY
        // prompt from EVERY site waited invisibly until that entry's 10-minute
        // expiry. Fails closed, so it is a consent OUTAGE rather than a spend
        // bypass — one any site could trigger at will.
        //
        // Both conditions now, decided under the same lock as the insert so two
        // concurrent first-for-domain prompts cannot both conclude they may show.
        showNow = wasFirstForDomain && !anyLiveShownLocked();
        std::string id = generateId();
        req.requestId = id;
        req.seq = counter_;
        if (showNow) { req.shown = true; req.shownAt = std::chrono::steady_clock::now(); }
        requests_[id] = std::move(req);
        return id;
    }

    // beta.3 Phase 10b — register a prompt that owns a modal. `showNow` is true
    // only when no other live prompt is on screen; otherwise the entry waits for
    // `takeNextQueuedPrompt`. `queuedFromSite` counts the OTHER waiting prompts
    // from the same domain (the modal's "1 of N" line). One lock for the check
    // and the insert — the P0.8-A4 check-then-act lesson.
    std::string addPromptRequest(PendingAuthRequest req, bool& showNow, int& queuedFromSite) {
        std::lock_guard<std::mutex> lock(mutex_);
        showNow = !anyLiveShownLocked();
        std::string id = generateId();
        req.requestId = id;
        req.seq = counter_;
        if (showNow) { req.shown = true; req.shownAt = std::chrono::steady_clock::now(); }
        queuedFromSite = countWaitingForDomainLocked(req.domain, id);
        requests_[id] = std::move(req);
        return id;
    }

    // Pick the oldest waiting prompt, mark it shown, and hand back a copy to post.
    // Returns false while another live prompt is still on screen, or none waits.
    bool takeNextQueuedPrompt(PendingAuthRequest& out, int& queuedFromSite) {
        std::lock_guard<std::mutex> lock(mutex_);
        if (anyLiveShownLocked()) return false;
        PendingAuthRequest* best = nullptr;
        const auto now = std::chrono::steady_clock::now();
        for (auto& pair : requests_) {
            auto& r = pair.second;
            if (r.shown || r.overlayType.empty()) continue;
            // Panel `F1`: freshness. An entry older than the prompt timeout has
            // already told its caller "Approval timeout" — showing it now would
            // invite a click that resolves nothing the page can still receive,
            // and on the HTTP path re-issues the wallet call anyway.
            if (std::chrono::duration_cast<std::chrono::milliseconds>(now - r.createdAt).count()
                    >= kShownPromptExpiryMs) continue;
            if (!best || r.seq < best->seq) best = &r;
        }
        if (!best) return false;
        best->shown = true;
        best->shownAt = std::chrono::steady_clock::now();
        out = *best;
        queuedFromSite = countWaitingForDomainLocked(best->domain, best->requestId);
        return true;
    }

    // Mark an entry as on screen (posts that bypass addPromptRequest).
    void markShown(const std::string& requestId) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = requests_.find(requestId);
        if (it == requests_.end()) return;
        it->second.shown = true;
        it->second.shownAt = std::chrono::steady_clock::now();
    }

    // Retrieve and remove a request by ID
    bool popRequest(const std::string& requestId, PendingAuthRequest& out) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = requests_.find(requestId);
        if (it == requests_.end()) {
            return false;
        }
        out = it->second;
        requests_.erase(it);
        return true;
    }

    // Get request data without removing (for sending to overlay)
    bool getRequest(const std::string& requestId, PendingAuthRequest& out) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = requests_.find(requestId);
        if (it == requests_.end()) {
            return false;
        }
        out = it->second;
        return true;
    }

    // Phase 2.6-C.5 fix — update the body of an in-flight request in place.
    // Used by simple_handler's brc100_auth_response dispatcher: when the user
    // approves a certificate_disclosure modal with a subset of fields, the
    // body's fieldsToReveal array is reduced before the wallet call re-runs.
    // Returns true if the entry existed and was updated.
    bool updateRequestBody(const std::string& requestId, const std::string& newBody) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = requests_.find(requestId);
        if (it == requests_.end()) {
            return false;
        }
        it->second.body = newBody;
        return true;
    }

    // Cert-replay fix — set a header to inject on the re-issued wallet call.
    // Used by simple_handler's brc100_auth_response dispatcher: when the user
    // approves a certificate_disclosure modal, the approved field set is only
    // known at approve-time (the 202-intercept already populated
    // X-User-Approved when the modal opened). This stashes
    // X-Cert-Approved-Fields onto the stored entry so resumeIpcResponse
    // carries it. Rust's dispatch_cert_disclosure replay path consumes the
    // approval id (single-use) and verifies requested ⊆ approved from this
    // header instead of body-sha256 (which a narrowed cert body would fail).
    // Returns true if the entry existed and was updated.
    bool setApproveHeader(const std::string& requestId,
                          const std::string& name,
                          const std::string& value) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = requests_.find(requestId);
        if (it == requests_.end()) {
            return false;
        }
        it->second.headersOnApprove[name] = value;
        return true;
    }

    // Check if any request is pending for a domain (for duplicate modal suppression)
    bool hasPendingForDomain(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        for (const auto& pair : requests_) {
            if (pair.second.domain == domain) {
                return true;
            }
        }
        return false;
    }

    // beta.3 Phase 10b — the connect-prompt types, the only ones whose queued
    // siblings may be resumed by one decision (they are re-issued WITHOUT a token
    // and re-evaluated fresh). A kind prompt (payment, rate limit, scoped grant,
    // certificate, key reveal) carries its own single-use approval and must be
    // answered by its own click.
    static bool isConnectPromptType(const std::string& type) {
        return type == "domain_approval" || type == "brc100_auth" || type == "manifest_connect_bundle";
    }

    // Pop only the connect-type requests for a domain; kind prompts stay queued.
    std::vector<PendingAuthRequest> popConnectForDomain(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        std::vector<PendingAuthRequest> result;
        for (auto it = requests_.begin(); it != requests_.end(); ) {
            if (it->second.domain == domain && isConnectPromptType(it->second.type)) {
                result.push_back(it->second);
                it = requests_.erase(it);
            } else {
                ++it;
            }
        }
        return result;
    }

    // Pop ALL requests for a domain (returns vector). Used when user approves —
    // resolves every queued request for that domain, not just the first.
    std::vector<PendingAuthRequest> popAllForDomain(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        std::vector<PendingAuthRequest> result;
        for (auto it = requests_.begin(); it != requests_.end(); ) {
            if (it->second.domain == domain) {
                result.push_back(it->second);
                it = requests_.erase(it);
            } else {
                ++it;
            }
        }
        return result;
    }

    // Get the most recent requestId for a domain (for overlay data sending)
    std::string getRequestIdForDomain(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        for (const auto& pair : requests_) {
            if (pair.second.domain == domain) {
                return pair.first;
            }
        }
        return "";
    }

private:
    PendingRequestManager() : counter_(0) {}

    // A shown prompt older than the prompt timeout no longer holds the screen:
    // the HTTP-transport timeout does not pop its entry, and an abandoned modal
    // must not stall every later prompt.
    static constexpr int kShownPromptExpiryMs = 600000;  // == kPromptAuthTimeoutMs

    bool anyLiveShownLocked() const {
        const auto now = std::chrono::steady_clock::now();
        for (const auto& pair : requests_) {
            const auto& r = pair.second;
            if (!r.shown) continue;
            if (std::chrono::duration_cast<std::chrono::milliseconds>(now - r.shownAt).count()
                    < kShownPromptExpiryMs) return true;
        }
        return false;
    }

    int countWaitingForDomainLocked(const std::string& domain, const std::string& excludeId) const {
        int n = 0;
        for (const auto& pair : requests_) {
            const auto& r = pair.second;
            if (pair.first != excludeId && r.domain == domain && !r.shown && !r.overlayType.empty()) ++n;
        }
        return n;
    }
    PendingRequestManager(const PendingRequestManager&) = delete;
    PendingRequestManager& operator=(const PendingRequestManager&) = delete;

    std::string generateId() {
        auto now = std::chrono::steady_clock::now().time_since_epoch();
        auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(now).count();
        return "req-" + std::to_string(ms) + "-" + std::to_string(++counter_);
    }

    std::mutex mutex_;
    std::map<std::string, PendingAuthRequest> requests_;
    uint64_t counter_;
};
