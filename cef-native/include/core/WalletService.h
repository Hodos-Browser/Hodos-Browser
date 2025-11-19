#pragma once

#include <string>
#include <nlohmann/json.hpp>
#include <windows.h>
#include <winhttp.h>
#include <thread>
#include <atomic>

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
    HINTERNET hSession_;
    HINTERNET hConnect_;
    bool connected_;

    // Process management
    PROCESS_INFORMATION daemonProcess_;
    std::atomic<bool> daemonRunning_;
    std::thread monitorThread_;

    // HTTP helper methods
    nlohmann::json makeHttpRequest(const std::string& method, const std::string& endpoint, const std::string& body = "");
    bool initializeConnection();
    void cleanupConnection();
    std::string readResponse(HINTERNET hRequest);

    // Daemon management helpers
    bool createDaemonProcess();
    void monitorDaemon();
    void cleanupDaemonProcess();

    // Console control handler
    static BOOL WINAPI ConsoleCtrlHandler(DWORD ctrlType);
};
