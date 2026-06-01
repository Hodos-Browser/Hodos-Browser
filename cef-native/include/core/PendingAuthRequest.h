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
