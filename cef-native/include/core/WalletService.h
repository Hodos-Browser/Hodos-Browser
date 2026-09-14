#pragma once

#include <string>
#include <nlohmann/json.hpp>
#include <thread>
#include <atomic>

#ifdef _WIN32
    #include <windows.h>
    #include <winhttp.h>
#endif

// P2a-A1 -- per-call wallet HTTP timeouts.
//
// These are UI-thread budgets, not network budgets: every call through
// WalletService::makeHttpRequest blocks the CEF UI thread for its whole duration, so the
// number here is "how long the browser is allowed to freeze". Once P2a-A2 moves the
// balance call off the UI thread these stop being freeze budgets, but they stay as the
// safety net -- (a) is deliberately verifiable on its own.
//
// SyncHttpClient's own default is 5000 ms; kWalletDefaultTimeoutMs matches it so the two
// clients do not disagree about what "normal" means.
inline constexpr int kWalletDefaultTimeoutMs = 5000;

// Read-only, hot, and useless once stale -- the balance poller re-fires anyway, so a slow
// answer is worth less than a fast failure.
inline constexpr int kWalletBalanceTimeoutMs = 2000;

// Broadcasts to ARC / WhatsOnChain. MUST stay generous: this is real money leaving, and a
// timeout here aborts a send the user asked for. Matches the pre-beta.3 macOS constant.
inline constexpr int kWalletBroadcastTimeoutMs = 30000;

// How long a Phase 8c bridge call (`hodosBrowser.bridge.*`, resolved by request id in the
// render process) may stay unanswered before it is rejected. It is a backstop against a
// browser process that never replies, NOT a latency budget — and it MUST clear the slowest
// transport timeout above with margin, or a slow-but-successful broadcast gets reported to
// the user as "timed out" while the money is in fact leaving. beta.3 Phase 8c O5 found the
// two equal at 30 000 ms, which is exactly that defect. Owned here, next to the number it
// has to beat, so the relationship is checked by the compiler.
inline constexpr int64_t kBridgeCallTimeoutMs = 45000;
static_assert(kBridgeCallTimeoutMs > kWalletBroadcastTimeoutMs + 5000,
              "the bridge deadline must clear the broadcast timeout with margin (8c O5)");

class WalletService {
public:
    WalletService();
    ~WalletService();

    // Initialization
    void ensureInitialized();

    // API Methods
    bool isHealthy();

    // Unified Wallet Methods
    // (createWallet / loadWallet were removed in beta.3 Phase 8c batch 2: their only callers
    // were IPC handlers with no reachable JS sender, and the Rust wallet's create path needs
    // a PIN this client never sent. Wallet creation is the wallet overlay's `wallet_call`.)
    nlohmann::json getWalletStatus();
    nlohmann::json getWalletInfo();
    bool markWalletBackedUp();

    // Address Management
    // (getAllAddresses / getCurrentAddress removed with their orphaned IPC handlers, 8c batch 2.)
    nlohmann::json generateAddress();

    // Transaction Methods
    // (createTransaction / signTransaction / broadcastTransaction / getTransactionHistory
    // removed in 8c batch 2 — full C++ round trips with no JS sender, contract D-6 / D-7.)
    nlohmann::json sendTransaction(const nlohmann::json& transactionData);
    nlohmann::json getBalance(const nlohmann::json& balanceData);

    // Connection management
    bool isConnected();
    void setBaseUrl(const std::string& url);

    // Daemon process management
    bool startDaemon();
    void stopDaemon();
    bool isDaemonRunning();
    void setDaemonPath(const std::string& path);

    // Public HTTP method for interceptors
    nlohmann::json makeHttpRequestPublic(const std::string& method, const std::string& endpoint, const std::string& body = "");

private:
    std::string baseUrl_;
    std::string daemonPath_;
    bool connected_;
    std::atomic<bool> daemonRunning_;
    std::thread monitorThread_;

#ifdef _WIN32
    // Windows-specific HTTP and process management
    HINTERNET hSession_;
    HINTERNET hConnect_;
    PROCESS_INFORMATION daemonProcess_;

    // Windows-specific helper methods
    std::string readResponse(HINTERNET hRequest);
    static BOOL WINAPI ConsoleCtrlHandler(DWORD ctrlType);
#endif

    // HTTP helper methods.
    //
    // P2a-A1: timeoutMs is PER CALL and is not optional in spirit -- until beta.3 this
    // function set no timeout at all on Windows (WinHTTP's 30 s receive default applied)
    // and a flat 30 s on macOS. Because the call is synchronous on the CEF UI thread, a
    // wallet that accepts the connection and never answers froze the ENTIRE browser --
    // every tab, ordinary web pages included -- for 30 s at a time, re-armed by the
    // balance poller. Measured: CDP /json/list 31.8 s vs a 1 ms control, an unrelated tab
    // 128 s to navigate. See phase-2-logging-syncio/MEASUREMENTS.md M5.
    //
    // ⛔ Do NOT collapse this back to one constant. /wallet/balance must fail fast;
    // /transaction/send legitimately takes seconds while it broadcasts, and capping it
    // globally would abort real sends (row P2a-A4 exists to catch exactly that).
    nlohmann::json makeHttpRequest(const std::string& method, const std::string& endpoint,
                                   const std::string& body = "",
                                   int timeoutMs = kWalletDefaultTimeoutMs);
    bool initializeConnection();
    void cleanupConnection();

    // Daemon management helpers
    bool createDaemonProcess();
    void monitorDaemon();
    void cleanupDaemonProcess();
};
