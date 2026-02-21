#pragma once

#include "include/cef_resource_handler.h"
#include <string>
#include <vector>
#include <map>
#include <mutex>
#include <chrono>

struct PendingAuthRequest {
    std::string requestId;
    std::string domain;
    std::string method;
    std::string endpoint;
    std::string body;
    std::string type;  // "domain_approval", "brc100_auth", "no_wallet", "payment_confirmation", "rate_limit_exceeded", "certificate_disclosure"
    CefRefPtr<CefResourceHandler> handler;
};

class PendingRequestManager {
public:
    static PendingRequestManager& GetInstance() {
        static PendingRequestManager instance;
        return instance;
    }

    // Store a request, returns the generated requestId
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
        requests_[id] = req;
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
