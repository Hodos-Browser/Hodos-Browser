#pragma once

#include <string>
#include <map>
#include <mutex>
#include <chrono>

// Per-browser session tracking for the auto-approve engine.
// Keyed by browser ID (not tab ID) because Open() runs on IO thread
// where only CefBrowser is available. TabManager is UI-thread-only.
struct BrowserSession {
    int browserId = 0;
    std::string domain;
    int64_t spentCents = 0;              // USD cents spent this session
    int paymentRequestsThisMinute = 0;   // rate limit counter
    int paymentCountThisSession = 0;     // total transaction count this session
    std::chrono::steady_clock::time_point minuteWindowStart;
};

class SessionManager {
public:
    static SessionManager& GetInstance() {
        static SessionManager instance;
        return instance;
    }

    // Get-or-create session. Resets if domain changed (navigated away).
    BrowserSession& getSession(int browserId, const std::string& domain);

    // Record spending after successful payment
    void recordSpending(int browserId, int64_t cents);

    // Check if under rate limit. Returns true if OK, false if over limit.
    bool checkRateLimit(int browserId, int64_t limitPerMin);

    // Increment payment request counter (call before forwarding payment endpoint)
    void incrementRateCounter(int browserId);

    // Clear session on tab close
    void clearSession(int browserId);

    // Get current spent cents for a browser session
    int64_t getSpentCents(int browserId, const std::string& domain);

    // Get total payment transaction count for a browser session
    int getPaymentCount(int browserId, const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = sessions_.find(browserId);
        if (it != sessions_.end() && it->second.domain == domain) {
            return it->second.paymentCountThisSession;
        }
        return 0;
    }

    // Increment total payment transaction count (call alongside incrementRateCounter)
    void incrementPaymentCount(int browserId) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = sessions_.find(browserId);
        if (it != sessions_.end()) {
            it->second.paymentCountThisSession++;
        }
    }

private:
    SessionManager() = default;
    SessionManager(const SessionManager&) = delete;
    SessionManager& operator=(const SessionManager&) = delete;

    std::mutex mutex_;
    std::map<int, BrowserSession> sessions_;
};
