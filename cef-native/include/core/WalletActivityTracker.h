#pragma once

// beta.3 Phase 0.9 — "is a wallet connect modal still coming for this host?"
//
// WHY THIS EXISTS
// ---------------
// A loopback permission is parked the moment Chromium raises it, and a wallet
// connect modal may claim it a moment later. The first implementation guessed at
// that gap with a fixed 750 ms timer. MEASURED gaps on one machine, same site:
//
//     239 ms   (first bitgenius run)
//     282 ms   (Profile_2)
//     830 ms   (Profile_1)  <-- blew straight through the 750 ms window
//
// The 830 ms case showed the standalone loopback prompt for ~71 ms before the
// connect modal replaced it: a prompt the user should never have seen. Widening
// the timer would only move the guess; the gap is a network round-trip to the Rust
// wallet plus a modal round-trip, and it has no upper bound worth betting on.
//
// So we stop timing and watch ACTIVITY instead: while the host is actively talking
// to the wallet, a connect modal may still be coming, so keep waiting. When it goes
// quiet and no modal is pending, nothing is coming and the standalone prompt shows.
//
// ⚠️ Deliberately a TIMESTAMP, not an in-flight counter. A counter has to be
// decremented on every completion, cancel and error path of the resource handler,
// and one missed decrement would pin the count above zero and suppress the prompt
// FOREVER for that host — failing silent and open. A timestamp cannot leak: worst
// case it is stale and the prompt appears a beat early, which is the same behaviour
// we already have today.

#include <chrono>
#include <map>
#include <mutex>
#include <string>

namespace hodos {

class WalletActivityTracker {
public:
    static WalletActivityTracker& GetInstance() {
        static WalletActivityTracker instance;
        return instance;
    }

    // Called wherever a wallet endpoint request is recognised, before it is
    // forwarded. `host` is the REQUESTING page's host, not the wallet's.
    void NoteWalletRequest(const std::string& host) {
        if (host.empty()) return;
        std::lock_guard<std::mutex> lock(mutex_);
        lastRequestMs_[host] = nowMs();
    }

    // Milliseconds since this host last touched a wallet endpoint, or a very large
    // number if it never has.
    int64_t MsSinceLastWalletRequest(const std::string& host) {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = lastRequestMs_.find(host);
        if (it == lastRequestMs_.end()) return kNever;
        return nowMs() - it->second;
    }

    // True while the host looks mid-conversation with the wallet, i.e. a connect
    // modal may still be on its way.
    bool IsTalkingToWallet(const std::string& host, int64_t quietMs) {
        return MsSinceLastWalletRequest(host) < quietMs;
    }

    static constexpr int64_t kNever = 1LL << 40;

private:
    WalletActivityTracker() = default;
    WalletActivityTracker(const WalletActivityTracker&) = delete;
    WalletActivityTracker& operator=(const WalletActivityTracker&) = delete;

    static int64_t nowMs() {
        return std::chrono::duration_cast<std::chrono::milliseconds>(
            std::chrono::steady_clock::now().time_since_epoch()).count();
    }

    std::mutex mutex_;
    std::map<std::string, int64_t> lastRequestMs_;
};

}  // namespace hodos
