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
    bool boundToConnectModal = false;  // P0.9: the wallet connect modal answers this one
    std::string boundModalDomain;      // P0.9: host of the modal that claimed it
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

    // --- beta.3 P0.9: BINDING a loopback/local-network permission to the wallet
    // connect modal. Owner decision 2026-08-24, superseding the 2026-08-21
    // "no auto-allow for loopback" line and recorded in PHASE_CONTRACT.md.
    //
    // Rationale: the loopback grant is BLANKET (Chromium keys it on the requesting
    // origin only — TOP_ORIGIN_ONLY_SCOPE — with no target port), and connecting a
    // wallet already hands the site local access. Two prompts for one decision is
    // worse UX and no more protective, PROVIDED the connect modal discloses that it
    // also grants access to other local apps. That disclosure is the condition on
    // which this is safe; do not remove one without removing the other. ---

    // Is a network-class permission (loopback / local network) parked for `host`?
    // `types` carries our stable ids: 6 = LocalNetwork, 7 = Loopback.
    bool hasParkedNetworkPermissionForHost(const std::string& host) {
        std::lock_guard<std::mutex> lock(mutex_);
        for (auto& kv : requests_) {
            if (kv.second.host != host || kv.second.isMedia) continue;
            for (auto t : kv.second.types) {
                if (t == SitePermissionType::LocalNetwork ||
                    t == SitePermissionType::Loopback) return true;
            }
        }
        return false;
    }

    // Bind every parked network permission for `host` to a connect modal: the
    // delayed "show our own prompt" task must NOT fire for these.
    void bindNetworkPermissionsForHost(const std::string& host) {
        std::lock_guard<std::mutex> lock(mutex_);
        for (auto& kv : requests_) {
            if (kv.second.host != host || kv.second.isMedia) continue;
            for (auto t : kv.second.types) {
                if (t == SitePermissionType::LocalNetwork ||
                    t == SitePermissionType::Loopback) {
                    kv.second.boundToConnectModal = true;
                    kv.second.boundModalDomain = host;
                    break;
                }
            }
        }
    }

    bool isBound(const std::string& requestId) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = requests_.find(requestId);
        return it != requests_.end() && it->second.boundToConnectModal;
    }

    // Pop every network permission bound to a connect modal for `host`, so the
    // caller can resolve them with the modal's answer.
    std::vector<PendingPermissionRequest> popBoundNetworkPermissionsForHost(const std::string& host) {
        std::lock_guard<std::mutex> lock(mutex_);
        std::vector<PendingPermissionRequest> out;
        for (auto it = requests_.begin(); it != requests_.end();) {
            if (it->second.boundToConnectModal && it->second.host == host && !it->second.isMedia) {
                out.push_back(it->second);
                it = requests_.erase(it);
            } else {
                ++it;
            }
        }
        return out;
    }

    // --- beta.3 P0.9: pre-emption latch. Set at the MOMENT another overlay type
    // takes the shared notification overlay while a permission prompt is parked.
    // ⛔ Do NOT infer this later from g_pendingModalDomain: the modal's own
    // response handler clears that string in the same millisecond it closes the
    // overlay (MEASURED 2026-08-24 08:32:39.741 — brc100_auth_response then
    // overlay_close, same ms), so a check at close time always reads empty. ---
    void markPreempted() {
        std::lock_guard<std::mutex> lock(mutex_);
        if (!requests_.empty()) preempted_ = true;
    }
    bool consumePreempted() {
        std::lock_guard<std::mutex> lock(mutex_);
        const bool p = preempted_ && !requests_.empty();
        preempted_ = false;
        return p;
    }

    // Look at the parked request WITHOUT removing it, and restart its staleness
    // clock. Used when the shared notification overlay was taken over by a wallet
    // modal and we need to re-show the prompt once the overlay is free again
    // (beta.3 P0.9): the CEF callback must stay parked across that hand-off, and
    // the 60s watchdog must not reap it while it is queued behind the modal.
    // ⛔ Skips entries already BOUND to a connect modal: those are that modal's to
    // answer, and re-showing one produces the double-consent this phase removes
    // (and lets the user Block while the modal is still promising access).
    bool peekParked(PendingPermissionRequest& out) {
        std::lock_guard<std::mutex> lock(mutex_);
        for (auto& kv : requests_) {
            if (kv.second.boundToConnectModal) continue;
            kv.second.createdAtMs = nowMs();   // restart the watchdog for the re-show
            out = kv.second;
            return true;
        }
        return false;
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
    bool preempted_ = false;   // a modal stole the overlay from a parked prompt
};
