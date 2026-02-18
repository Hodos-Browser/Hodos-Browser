#include "../../include/core/SessionManager.h"

BrowserSession& SessionManager::getSession(int browserId, const std::string& domain) {
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = sessions_.find(browserId);
    if (it == sessions_.end()) {
        // New session
        BrowserSession session;
        session.browserId = browserId;
        session.domain = domain;
        session.minuteWindowStart = std::chrono::steady_clock::now();
        sessions_[browserId] = session;
        return sessions_[browserId];
    }

    // If domain changed (user navigated to different site), reset session
    if (it->second.domain != domain) {
        it->second.domain = domain;
        it->second.spentCents = 0;
        it->second.paymentRequestsThisMinute = 0;
        it->second.minuteWindowStart = std::chrono::steady_clock::now();
    }

    return it->second;
}

void SessionManager::recordSpending(int browserId, int64_t cents) {
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = sessions_.find(browserId);
    if (it != sessions_.end()) {
        it->second.spentCents += cents;
    }
}

bool SessionManager::checkRateLimit(int browserId, int64_t limitPerMin) {
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = sessions_.find(browserId);
    if (it == sessions_.end()) {
        return true;  // No session = no rate limit hit
    }

    auto now = std::chrono::steady_clock::now();
    auto elapsed = std::chrono::duration_cast<std::chrono::seconds>(
        now - it->second.minuteWindowStart).count();

    // Reset window if >60 seconds have passed
    if (elapsed >= 60) {
        it->second.paymentRequestsThisMinute = 0;
        it->second.minuteWindowStart = now;
        return true;
    }

    return it->second.paymentRequestsThisMinute < limitPerMin;
}

void SessionManager::incrementRateCounter(int browserId) {
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = sessions_.find(browserId);
    if (it != sessions_.end()) {
        auto now = std::chrono::steady_clock::now();
        auto elapsed = std::chrono::duration_cast<std::chrono::seconds>(
            now - it->second.minuteWindowStart).count();

        // Reset window if >60 seconds have passed
        if (elapsed >= 60) {
            it->second.paymentRequestsThisMinute = 0;
            it->second.minuteWindowStart = now;
        }

        it->second.paymentRequestsThisMinute++;
    }
}

void SessionManager::clearSession(int browserId) {
    std::lock_guard<std::mutex> lock(mutex_);
    sessions_.erase(browserId);
}

int64_t SessionManager::getSpentCents(int browserId, const std::string& domain) {
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = sessions_.find(browserId);
    if (it == sessions_.end() || it->second.domain != domain) {
        return 0;
    }
    return it->second.spentCents;
}
