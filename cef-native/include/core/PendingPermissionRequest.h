#pragma once

#include "include/cef_permission_handler.h"
#include "SitePermissionStore.h"
#include <string>
#include <vector>
#include <map>
#include <set>
#include <mutex>
#include <chrono>

// b1b — parks a CEF permission callback while the Hodos-branded prompt is shown,
// resolved when React replies (permission_response IPC). Exactly ONE of mediaCb /
// promptCb is set. All access is on the browser-process UI thread; the mutex is a
// belt-and-suspenders guard.
struct PendingPermissionRequest {
    std::string requestId;
    std::string host;                 // normalized host the decision applies to
    bool isMedia = false;             // true → mediaCb, false → promptCb
    CefRefPtr<CefMediaAccessCallback> mediaCb;
    CefRefPtr<CefPermissionPromptCallback> promptCb;
    uint32_t requestedMask = 0;       // media: the device bits to grant on Allow
    uint64_t promptId = 0;            // prompt path: CEF prompt_id (for OnDismiss match)
    int browserId = 0;                // the requesting tab browser (for close/nav cleanup)
    int64_t createdAtMs = 0;          // park time (for the stale-entry sweep)
    std::vector<SitePermissionType> types;  // permission type(s) to persist on Allow/Block
};

class PendingPermissionManager {
public:
    static PendingPermissionManager& GetInstance() {
        static PendingPermissionManager instance;
        return instance;
    }

    // True if a permission prompt is already parked (b1a/b1b show one at a time).
    bool hasPending() {
        std::lock_guard<std::mutex> lock(mutex_);
        return !requests_.empty();
    }

    // Park a request; returns the generated id (written into req.requestId).
    std::string add(PendingPermissionRequest req) {
        std::lock_guard<std::mutex> lock(mutex_);
        std::string id = generateId();
        req.requestId = id;
        req.createdAtMs = nowMs();
        requests_[id] = std::move(req);
        return id;
    }

    // Pop all entries for a browser id (tab close / navigation cleanup).
    std::vector<PendingPermissionRequest> popForBrowser(int browserId) {
        std::lock_guard<std::mutex> lock(mutex_);
        std::vector<PendingPermissionRequest> out;
        for (auto it = requests_.begin(); it != requests_.end();) {
            if (it->second.browserId == browserId) { out.push_back(it->second); it = requests_.erase(it); }
            else ++it;
        }
        return out;
    }

    // Pop entries older than maxAgeMs (watchdog: an unanswered prompt must not
    // strand its CEF callback or jam the single-prompt-at-a-time gate forever).
    std::vector<PendingPermissionRequest> popExpired(int64_t maxAgeMs) {
        std::lock_guard<std::mutex> lock(mutex_);
        std::vector<PendingPermissionRequest> out;
        const int64_t now = nowMs();
        for (auto it = requests_.begin(); it != requests_.end();) {
            if (now - it->second.createdAtMs > maxAgeMs) { out.push_back(it->second); it = requests_.erase(it); }
            else ++it;
        }
        return out;
    }

    // Retrieve + remove by id.
    bool pop(const std::string& requestId, PendingPermissionRequest& out) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = requests_.find(requestId);
        if (it == requests_.end()) return false;
        out = it->second;
        requests_.erase(it);
        return true;
    }

    // Retrieve + remove by CEF prompt_id (OnDismissPermissionPrompt cleanup).
    // Guarded on promptId != 0 so a stray dismiss with id 0 can't pop a
    // prompt entry that never recorded a real id.
    bool popByPromptId(uint64_t promptId, PendingPermissionRequest& out) {
        std::lock_guard<std::mutex> lock(mutex_);
        if (promptId == 0) return false;
        for (auto it = requests_.begin(); it != requests_.end(); ++it) {
            if (!it->second.isMedia && it->second.promptId == promptId) {
                out = it->second;
                requests_.erase(it);
                return true;
            }
        }
        return false;
    }

    // --- b1b.1: allow-once SESSION grants (ephemeral; NOT persisted to SQLite).
    // Keyed by (browserId, host, permission-type-int). "Allow this time" records
    // one here so re-requests in the same tab/site are silently allowed (Chrome
    // parity), cleared on navigate-away (host change) + tab close. ---
    void grantSession(int browserId, const std::string& host, int type) {
        std::lock_guard<std::mutex> lock(mutex_);
        sessionGrants_[browserId][host].insert(type);
    }
    bool isSessionGranted(int browserId, const std::string& host, int type) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto b = sessionGrants_.find(browserId);
        if (b == sessionGrants_.end()) return false;
        auto h = b->second.find(host);
        if (h == b->second.end()) return false;
        return h->second.count(type) > 0;
    }
    void clearSessionForBrowser(int browserId) {
        std::lock_guard<std::mutex> lock(mutex_);
        sessionGrants_.erase(browserId);
    }
    // Navigate-away: drop a browser's session grants for hosts other than the one
    // it just navigated to (same-host reload keeps the grant).
    void clearSessionForBrowserExceptHost(int browserId, const std::string& host) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto b = sessionGrants_.find(browserId);
        if (b == sessionGrants_.end()) return;
        for (auto it = b->second.begin(); it != b->second.end();) {
            if (it->first != host) it = b->second.erase(it);
            else ++it;
        }
        if (b->second.empty()) sessionGrants_.erase(b);
    }

private:
    PendingPermissionManager() = default;
    PendingPermissionManager(const PendingPermissionManager&) = delete;
    PendingPermissionManager& operator=(const PendingPermissionManager&) = delete;

    static int64_t nowMs() {
        return std::chrono::duration_cast<std::chrono::milliseconds>(
            std::chrono::steady_clock::now().time_since_epoch()).count();
    }

    std::string generateId() {
        return "perm-" + std::to_string(nowMs()) + "-" + std::to_string(++counter_);
    }

    std::mutex mutex_;
    std::map<std::string, PendingPermissionRequest> requests_;
    // browserId -> host -> set<permission-type-int> (ephemeral allow-once grants)
    std::map<int, std::map<std::string, std::set<int>>> sessionGrants_;
    uint64_t counter_ = 0;
};
