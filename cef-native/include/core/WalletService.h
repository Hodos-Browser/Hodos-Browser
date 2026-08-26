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

class WalletService {
public:
    WalletService();
    ~WalletService();

    // Initialization
    void ensureInitialized();

    // API Methods
    bool isHealthy();

    // Unified Wallet Methods
    nlohmann::json getWalletStatus();
    nlohmann::json getWalletInfo();
    nlohmann::json createWallet();
    nlohmann::json loadWallet();
    bool markWalletBackedUp();

    // Address Management
    nlohmann::json getAllAddresses();
    nlohmann::json generateAddress();
    nlohmann::json getCurrentAddress();

    // Transaction Methods
    nlohmann::json createTransaction(const nlohmann::json& transactionData);
    nlohmann::json signTransaction(const nlohmann::json& transactionData);
    nlohmann::json broadcastTransaction(const nlohmann::json& transactionData);
    nlohmann::json sendTransaction(const nlohmann::json& transactionData);
    nlohmann::json getBalance(const nlohmann::json& balanceData);
    nlohmann::json getTransactionHistory();

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
