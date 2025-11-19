#include "../../include/core/WalletService.h"
#include <iostream>
#include <sstream>
#include <fstream>
#include <chrono>
#include <iomanip>

// Logger class for proper debug logging
class Logger {
private:
    static std::string GetTimestamp() {
        auto now = std::chrono::system_clock::now();
        auto time_t = std::chrono::system_clock::to_time_t(now);
        auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
            now.time_since_epoch()) % 1000;

        std::stringstream ss;
        ss << std::put_time(std::localtime(&time_t), "%Y-%m-%d %H:%M:%S");
        ss << "." << std::setfill('0') << std::setw(3) << ms.count();
        return ss.str();
    }

    static std::string GetProcessName(int process) {
        switch (process) {
            case 0: return "MAIN";
            case 1: return "RENDER";
            case 2: return "BROWSER";
            default: return "UNKNOWN";
        }
    }

    static std::string GetLogLevelName(int level) {
        switch (level) {
            case 0: return "DEBUG";
            case 1: return "INFO";
            case 2: return "WARN";
            case 3: return "ERROR";
            default: return "UNKNOWN";
        }
    }

public:
    static void Log(const std::string& message, int level = 1, int process = 2) {
        std::string logEntry = "[" + GetTimestamp() + "] [" + GetProcessName(process) + "] [" + GetLogLevelName(level) + "] " + message;

        // Write to file
        std::ofstream logFile("debug_output.log", std::ios::app);
        if (logFile.is_open()) {
            logFile << logEntry << std::endl;
            logFile.close();
        }

        // Also write to stdout (for debugging)
        std::cout << logEntry << std::endl;
    }
};

// Convenience macros for easier logging
#define LOG_DEBUG_BROWSER(msg) Logger::Log(msg, 0, 2)
#define LOG_INFO_BROWSER(msg) Logger::Log(msg, 1, 2)
#define LOG_WARNING_BROWSER(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_BROWSER(msg) Logger::Log(msg, 3, 2)

// Static instance for console handler
static WalletService* g_walletService = nullptr;

WalletService::WalletService()
    : baseUrl_("http://localhost:3301")
    , daemonPath_("")
    , hSession_(nullptr)
    , hConnect_(nullptr)
    , connected_(false)
    , daemonRunning_(false) {

    try {
        // Set global instance for console handler
        g_walletService = this;

        // Initialize daemon process info
        ZeroMemory(&daemonProcess_, sizeof(PROCESS_INFORMATION));

        LOG_DEBUG_BROWSER("🚀 WalletService constructor starting...");

        // Initialize connection to Go daemon
        if (!initializeConnection()) {
            LOG_WARNING_BROWSER("⚠️ Failed to connect to Go daemon at " + baseUrl_);
        } else {
            LOG_DEBUG_BROWSER("✅ Connected to Go daemon successfully");
        }

        LOG_DEBUG_BROWSER("✅ WalletService constructor completed");

    } catch (const std::exception& e) {
        LOG_ERROR_BROWSER("❌ WalletService constructor exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR_BROWSER("❌ WalletService constructor unknown exception");
    }
}

WalletService::~WalletService() {
    std::cout << "🛑 WalletService destructor called - shutting down daemon..." << std::endl;
    stopDaemon();
    cleanupConnection();
}

bool WalletService::initializeConnection() {
    // Initialize WinHTTP session
    hSession_ = WinHttpOpen(L"BitcoinBrowser/1.0",
                           WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                           WINHTTP_NO_PROXY_NAME,
                           WINHTTP_NO_PROXY_BYPASS,
                           0);

    if (!hSession_) {
        std::cerr << "❌ Failed to initialize WinHTTP session. Error: " << GetLastError() << std::endl;
        return false;
    }

    // Parse URL
    URL_COMPONENTS urlComp = {0};
    urlComp.dwStructSize = sizeof(urlComp);
    urlComp.dwSchemeLength = -1;
    urlComp.dwHostNameLength = -1;
    urlComp.dwUrlPathLength = -1;
    urlComp.dwExtraInfoLength = -1;

    std::wstring wideUrl(baseUrl_.begin(), baseUrl_.end());
    if (!WinHttpCrackUrl(wideUrl.c_str(), 0, 0, &urlComp)) {
        std::cerr << "❌ Failed to parse URL: " << baseUrl_ << std::endl;
        return false;
    }

    // Extract hostname and port
    std::wstring hostname(urlComp.lpszHostName, urlComp.dwHostNameLength);
    INTERNET_PORT port = urlComp.nPort;
    if (port == 0) {
        port = (urlComp.nScheme == INTERNET_SCHEME_HTTPS) ? 443 : 80;
    }

    // Connect to server
    hConnect_ = WinHttpConnect(hSession_, hostname.c_str(), port, 0);
    if (!hConnect_) {
        std::cerr << "❌ Failed to connect to Go daemon at " << baseUrl_ << std::endl;
        return false;
    }

    connected_ = true;
    std::cout << "✅ Connected to Go wallet daemon at " << baseUrl_ << std::endl;
    return true;
}

void WalletService::cleanupConnection() {
    if (hConnect_) {
        WinHttpCloseHandle(hConnect_);
        hConnect_ = nullptr;
    }
    if (hSession_) {
        WinHttpCloseHandle(hSession_);
        hSession_ = nullptr;
    }
    connected_ = false;
}

bool WalletService::isConnected() {
    return connected_;
}

void WalletService::setBaseUrl(const std::string& url) {
    if (baseUrl_ != url) {
        cleanupConnection();
        baseUrl_ = url;
        initializeConnection();
    }
}

nlohmann::json WalletService::makeHttpRequest(const std::string& method, const std::string& endpoint, const std::string& body) {
    if (!connected_) {
        std::cerr << "❌ Not connected to Go daemon" << std::endl;
        return nlohmann::json::object();
    }

    // Convert endpoint to wide string
    std::wstring wideEndpoint(endpoint.begin(), endpoint.end());

    // Create request
    HINTERNET hRequest = WinHttpOpenRequest(hConnect_,
                                           std::wstring(method.begin(), method.end()).c_str(),
                                           wideEndpoint.c_str(),
                                           nullptr,
                                           WINHTTP_NO_REFERER,
                                           WINHTTP_DEFAULT_ACCEPT_TYPES,
                                           0);

    if (!hRequest) {
        std::cerr << "❌ Failed to create HTTP request. Error: " << GetLastError() << std::endl;
        return nlohmann::json::object();
    }

    // Set headers
    std::string contentType = "application/json";
    std::wstring wideContentType(contentType.begin(), contentType.end());
    WinHttpAddRequestHeaders(hRequest,
                           std::wstring(L"Content-Type: " + wideContentType).c_str(),
                           -1,
                           WINHTTP_ADDREQ_FLAG_ADD);

    // Send request
    BOOL result = WinHttpSendRequest(hRequest,
                                   WINHTTP_NO_ADDITIONAL_HEADERS,
                                   0,
                                   body.empty() ? WINHTTP_NO_REQUEST_DATA : (LPVOID)body.c_str(),
                                   body.length(),
                                   body.length(),
                                   0);

    if (!result) {
        std::cerr << "❌ Failed to send HTTP request. Error: " << GetLastError() << std::endl;
        WinHttpCloseHandle(hRequest);
        return nlohmann::json::object();
    }

    // Receive response
    if (!WinHttpReceiveResponse(hRequest, nullptr)) {
        std::cerr << "❌ Failed to receive HTTP response. Error: " << GetLastError() << std::endl;
        WinHttpCloseHandle(hRequest);
        return nlohmann::json::object();
    }

    // Read response body
    std::string responseBody = readResponse(hRequest);
    WinHttpCloseHandle(hRequest);

    // Parse JSON response
    try {
        return nlohmann::json::parse(responseBody);
    } catch (const std::exception& e) {
        std::cerr << "❌ Failed to parse JSON response: " << e.what() << std::endl;
        std::cerr << "Response body: " << responseBody << std::endl;
        return nlohmann::json::object();
    }
}

std::string WalletService::readResponse(HINTERNET hRequest) {
    std::string response;
    DWORD dwSize = 0;
    DWORD dwDownloaded = 0;

    do {
        dwSize = 0;
        if (!WinHttpQueryDataAvailable(hRequest, &dwSize)) {
            break;
        }

        if (dwSize == 0) {
            break;
        }

        std::vector<char> buffer(dwSize + 1);
        if (!WinHttpReadData(hRequest, buffer.data(), dwSize, &dwDownloaded)) {
            break;
        }

        response.append(buffer.data(), dwDownloaded);
    } while (dwSize > 0);

    return response;
}

bool WalletService::isHealthy() {
    std::cout << "🔍 Checking Go daemon health..." << std::endl;

    auto response = makeHttpRequest("GET", "/health");

    if (response.contains("status") && response["status"] == "healthy") {
        std::cout << "✅ Go daemon is healthy" << std::endl;
        return true;
    } else {
        std::cerr << "❌ Go daemon health check failed" << std::endl;
        return false;
    }
}

// Unified Wallet Methods Implementation

void WalletService::ensureInitialized() {
    static bool initialized = false;
    if (initialized) return;

    try {
        LOG_DEBUG_BROWSER("🔧 Initializing WalletService...");

        // Set default daemon path (relative to executable)
        char exePath[MAX_PATH];
        GetModuleFileNameA(nullptr, exePath, MAX_PATH);
        std::string exeDir = std::string(exePath);
        size_t lastSlash = exeDir.find_last_of("\\/");
        if (lastSlash != std::string::npos) {
            exeDir = exeDir.substr(0, lastSlash);
            daemonPath_ = exeDir + "\\..\\..\\..\\..\\go-wallet\\wallet.exe";
        }

        // Set up console control handler
        SetConsoleCtrlHandler(ConsoleCtrlHandler, TRUE);

        // Initialize HTTP connection to Go daemon
        if (initializeConnection()) {
            LOG_DEBUG_BROWSER("✅ HTTP connection to Go daemon established");
        } else {
            LOG_WARNING_BROWSER("⚠️ Failed to establish HTTP connection to Go daemon");
        }

        LOG_DEBUG_BROWSER("✅ WalletService initialization completed");

        initialized = true;
    } catch (const std::exception& e) {
        LOG_ERROR_BROWSER("❌ WalletService initialization exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR_BROWSER("❌ WalletService initialization unknown exception");
    }
}

nlohmann::json WalletService::getWalletStatus() {
    LOG_DEBUG_BROWSER("🔍 Getting wallet status from Go daemon...");

    // Ensure WalletService is properly initialized
    ensureInitialized();

    try {
        // Make actual HTTP request to Go daemon
        LOG_DEBUG_BROWSER("🔄 Making HTTP request to /wallet/status...");

        auto response = makeHttpRequest("GET", "/wallet/status");

        if (response.contains("exists")) {
            LOG_DEBUG_BROWSER("✅ Wallet status retrieved successfully from Go daemon");

            // Add needsBackup field if not present (for backward compatibility)
            if (!response.contains("needsBackup")) {
                response["needsBackup"] = false;
            }

            return response;
        } else {
            LOG_WARNING_BROWSER("⚠️ Unexpected response format from Go daemon");
        }
    } catch (const std::exception& e) {
        LOG_ERROR_BROWSER("❌ Error getting wallet status: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR_BROWSER("❌ Unknown error getting wallet status");
    }

    // Fallback response if connection fails
    nlohmann::json fallbackResponse;
    fallbackResponse["exists"] = false;
    fallbackResponse["needsBackup"] = true;
    fallbackResponse["error"] = "Failed to connect to Go daemon";

    LOG_WARNING_BROWSER("📤 Returning fallback response due to connection error");

    return fallbackResponse;
}

nlohmann::json WalletService::getWalletInfo() {
    std::cout << "🔍 Getting wallet info from Go daemon..." << std::endl;

    auto response = makeHttpRequest("GET", "/wallet/info");

    if (response.contains("version")) {
        std::cout << "✅ Wallet info retrieved successfully" << std::endl;
        std::cout << "📁 Version: " << response["version"].get<std::string>() << std::endl;
        std::cout << "🔑 Backed up: " << (response["backedUp"].get<bool>() ? "Yes" : "No") << std::endl;
        return response;
    } else {
        std::cerr << "❌ Failed to get wallet info from Go daemon" << std::endl;
        return nlohmann::json::object();
    }
}

nlohmann::json WalletService::createWallet() {
    std::cout << "🔍 Creating new wallet via Go daemon..." << std::endl;

    auto response = makeHttpRequest("POST", "/wallet/create");

    if (response.contains("success") && response["success"].get<bool>()) {
        std::cout << "✅ Wallet created successfully" << std::endl;
        std::cout << "🔑 Mnemonic: " << response["mnemonic"].get<std::string>() << std::endl;
        return response;
    } else {
        std::cerr << "❌ Failed to create wallet from Go daemon" << std::endl;
        return nlohmann::json::object();
    }
}

nlohmann::json WalletService::loadWallet() {
    std::cout << "🔍 Loading wallet from Go daemon..." << std::endl;

    auto response = makeHttpRequest("POST", "/wallet/load");

    if (response.contains("success") && response["success"].get<bool>()) {
        std::cout << "✅ Wallet loaded successfully" << std::endl;
        return response;
    } else {
        std::cerr << "❌ Failed to load wallet from Go daemon" << std::endl;
        return nlohmann::json::object();
    }
}

bool WalletService::markWalletBackedUp() {
    std::cout << "🔍 Marking wallet as backed up..." << std::endl;

    auto response = makeHttpRequest("POST", "/wallet/markBackedUp");

    if (response.contains("success") && response["success"] == true) {
        std::cout << "✅ Wallet marked as backed up successfully" << std::endl;
        return true;
    } else {
        std::cerr << "❌ Failed to mark wallet as backed up" << std::endl;
        return false;
    }
}

// Address Management Methods

nlohmann::json WalletService::getAllAddresses() {
    std::cout << "🔍 Getting all addresses from Go daemon..." << std::endl;

    auto response = makeHttpRequest("GET", "/wallet/addresses");

    if (response.is_array()) {
        std::cout << "✅ Addresses retrieved successfully" << std::endl;
        std::cout << "📍 Address count: " << response.size() << std::endl;
        return response;
    } else {
        std::cerr << "❌ Failed to get addresses from Go daemon" << std::endl;
        return nlohmann::json::array();
    }
}

nlohmann::json WalletService::getCurrentAddress() {
    std::cout << "🔍 Getting current address from Go daemon..." << std::endl;

    auto response = makeHttpRequest("GET", "/wallet/address/current");

    if (response.contains("address")) {
        std::cout << "✅ Current address retrieved successfully" << std::endl;
        std::cout << "📍 Address: " << response["address"].get<std::string>() << std::endl;
        return response;
    } else {
        std::cerr << "❌ Failed to get current address from Go daemon" << std::endl;
        return nlohmann::json::object();
    }
}

nlohmann::json WalletService::generateAddress() {
    std::cout << "🔍 Generating new address from Go daemon..." << std::endl;

    auto response = makeHttpRequest("POST", "/wallet/address/generate");

    if (response.contains("address")) {
        std::cout << "✅ Address generated successfully" << std::endl;
        std::cout << "📍 New Address: " << response["address"].get<std::string>() << std::endl;
        return response;
    } else {
        std::cerr << "❌ Failed to generate address from Go daemon" << std::endl;
        return nlohmann::json::object();
    }
}


// Transaction Methods Implementation

nlohmann::json WalletService::createTransaction(const nlohmann::json& transactionData) {
    std::cout << "💰 Creating transaction via Go daemon..." << std::endl;
    std::cout << "📋 Transaction data: " << transactionData.dump() << std::endl;
    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "💰 Creating transaction via Go daemon..." << std::endl;
    debugLog << "📋 Transaction data: " << transactionData.dump() << std::endl;
    debugLog.close();

    auto response = makeHttpRequest("POST", "/transaction/create", transactionData.dump());

    if (response.contains("txid")) {
        std::cout << "✅ Transaction created successfully" << std::endl;
        std::cout << "🆔 Transaction ID: " << response["txid"].get<std::string>() << std::endl;
        std::ofstream debugLog2("debug_output.log", std::ios::app);
        debugLog2 << "✅ Transaction created successfully" << std::endl;
        debugLog2 << "🆔 Transaction ID: " << response["txid"].get<std::string>() << std::endl;
        debugLog2.close();
        return response;
    } else {
        std::cerr << "❌ Failed to create transaction: " << response.dump() << std::endl;
        std::ofstream debugLog3("debug_output.log", std::ios::app);
        debugLog3 << "❌ Failed to create transaction: " << response.dump() << std::endl;
        debugLog3.close();
        return response; // Return the error response
    }
}

nlohmann::json WalletService::signTransaction(const nlohmann::json& transactionData) {
    std::cout << "✍️ Signing transaction via Go daemon..." << std::endl;
    std::cout << "📋 Transaction data: " << transactionData.dump() << std::endl;
    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "✍️ Signing transaction via Go daemon..." << std::endl;
    debugLog << "📋 Transaction data: " << transactionData.dump() << std::endl;
    debugLog.close();

    auto response = makeHttpRequest("POST", "/transaction/sign", transactionData.dump());

    if (response.contains("txid")) {
        std::cout << "✅ Transaction signed successfully" << std::endl;
        std::cout << "🆔 Transaction ID: " << response["txid"].get<std::string>() << std::endl;
        std::ofstream debugLog2("debug_output.log", std::ios::app);
        debugLog2 << "✅ Transaction signed successfully" << std::endl;
        debugLog2 << "🆔 Transaction ID: " << response["txid"].get<std::string>() << std::endl;
        debugLog2.close();
        return response;
    } else {
        std::cerr << "❌ Failed to sign transaction: " << response.dump() << std::endl;
        std::ofstream debugLog3("debug_output.log", std::ios::app);
        debugLog3 << "❌ Failed to sign transaction: " << response.dump() << std::endl;
        debugLog3.close();
        return response; // Return the error response
    }
}

nlohmann::json WalletService::broadcastTransaction(const nlohmann::json& transactionData) {
    std::cout << "📡 Broadcasting transaction via Go daemon..." << std::endl;
    std::cout << "📋 Transaction data: " << transactionData.dump() << std::endl;
    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "📡 Broadcasting transaction via Go daemon..." << std::endl;
    debugLog << "📋 Transaction data: " << transactionData.dump() << std::endl;
    debugLog.close();

    auto response = makeHttpRequest("POST", "/transaction/broadcast", transactionData.dump());

    if (response.contains("txid")) {
        std::cout << "✅ Transaction broadcast successfully" << std::endl;
        std::cout << "🆔 Transaction ID: " << response["txid"].get<std::string>() << std::endl;
        std::ofstream debugLog2("debug_output.log", std::ios::app);
        debugLog2 << "✅ Transaction broadcast successfully" << std::endl;
        debugLog2 << "🆔 Transaction ID: " << response["txid"].get<std::string>() << std::endl;
        debugLog2.close();
        return response;
    } else {
        std::cerr << "❌ Failed to broadcast transaction: " << response.dump() << std::endl;
        std::ofstream debugLog3("debug_output.log", std::ios::app);
        debugLog3 << "❌ Failed to broadcast transaction: " << response.dump() << std::endl;
        debugLog3.close();
        return response; // Return the error response
    }
}

nlohmann::json WalletService::getBalance(const nlohmann::json& balanceData) {
    std::cout << "💰 Getting total balance from Go daemon..." << std::endl;
    std::cout << "📋 Balance data: " << balanceData.dump() << std::endl;
    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "💰 Getting total balance from Go daemon..." << std::endl;
    debugLog << "📋 Balance data: " << balanceData.dump() << std::endl;
    debugLog.close();

    // Use the total balance endpoint (no address needed)
    std::string url = "/wallet/balance";
    auto response = makeHttpRequest("GET", url, "");

    if (response.contains("balance")) {
        int64_t totalBalance = response["balance"].get<int64_t>();

        std::cout << "✅ Total balance retrieved successfully" << std::endl;
        std::cout << "💵 Total Balance: " << totalBalance << " satoshis" << std::endl;
        std::ofstream debugLog2("debug_output.log", std::ios::app);
        debugLog2 << "✅ Total balance retrieved successfully" << std::endl;
        debugLog2 << "💵 Total Balance: " << totalBalance << " satoshis" << std::endl;
        debugLog2.close();

        // Return balance in expected format
        nlohmann::json balanceResponse;
        balanceResponse["balance"] = totalBalance;
        return balanceResponse;
    } else {
        std::cerr << "❌ Failed to get total balance: " << response.dump() << std::endl;
        std::ofstream debugLog3("debug_output.log", std::ios::app);
        debugLog3 << "❌ Failed to get total balance: " << response.dump() << std::endl;
        debugLog3.close();

        // Return error response
        nlohmann::json errorResponse;
        errorResponse["error"] = "Failed to fetch total balance";
        return errorResponse;
    }
}

nlohmann::json WalletService::getTransactionHistory() {
    std::cout << "📜 Getting transaction history from Go daemon..." << std::endl;
    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "📜 Getting transaction history from Go daemon..." << std::endl;
    debugLog.close();

    auto response = makeHttpRequest("GET", "/transaction/history");

    if (response.is_array() || response.contains("transactions")) {
        std::cout << "✅ Transaction history retrieved successfully" << std::endl;
        std::ofstream debugLog2("debug_output.log", std::ios::app);
        debugLog2 << "✅ Transaction history retrieved successfully" << std::endl;
        debugLog2.close();
        return response;
    } else {
        std::cerr << "❌ Failed to get transaction history: " << response.dump() << std::endl;
        std::ofstream debugLog3("debug_output.log", std::ios::app);
        debugLog3 << "❌ Failed to get transaction history: " << response.dump() << std::endl;
        debugLog3.close();
        return response; // Return the error response
    }
}

// Daemon Process Management Methods

bool WalletService::startDaemon() {
    if (daemonRunning_) {
        std::cout << "🔄 Go daemon already running" << std::endl;
        return true;
    }

    std::cout << "🚀 Starting Go wallet daemon..." << std::endl;

    if (createDaemonProcess()) {
        daemonRunning_ = true;
        monitorThread_ = std::thread(&WalletService::monitorDaemon, this);
        std::cout << "✅ Go daemon started successfully" << std::endl;
        return true;
    } else {
        std::cerr << "❌ Failed to start Go daemon" << std::endl;
        return false;
    }
}

void WalletService::stopDaemon() {
    if (!daemonRunning_) {
        return;
    }

    std::cout << "🛑 Stopping Go wallet daemon..." << std::endl;

    daemonRunning_ = false;

    if (monitorThread_.joinable()) {
        monitorThread_.join();
    }

    cleanupDaemonProcess();
    std::cout << "✅ Go daemon stopped" << std::endl;
}

bool WalletService::isDaemonRunning() {
    return daemonRunning_;
}

void WalletService::setDaemonPath(const std::string& path) {
    daemonPath_ = path;
}

bool WalletService::createDaemonProcess() {
    if (daemonPath_.empty()) {
        std::cerr << "❌ Daemon path not set" << std::endl;
        return false;
    }

    STARTUPINFOA si;
    ZeroMemory(&si, sizeof(si));
    si.cb = sizeof(si);
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = SW_HIDE; // Hide the daemon window

    ZeroMemory(&daemonProcess_, sizeof(PROCESS_INFORMATION));

    // Create the daemon process
    if (!CreateProcessA(
        daemonPath_.c_str(),    // Application name
        nullptr,                // Command line
        nullptr,                // Process security attributes
        nullptr,                // Thread security attributes
        FALSE,                  // Inherit handles
        CREATE_NO_WINDOW,       // Creation flags
        nullptr,                // Environment
        nullptr,                // Current directory
        &si,                    // Startup info
        &daemonProcess_)) {     // Process information

        std::cerr << "❌ Failed to create daemon process. Error: " << GetLastError() << std::endl;
        return false;
    }

    return true;
}

void WalletService::monitorDaemon() {
    while (daemonRunning_) {
        if (daemonProcess_.hProcess) {
            DWORD exitCode;
            if (GetExitCodeProcess(daemonProcess_.hProcess, &exitCode)) {
                if (exitCode != STILL_ACTIVE) {
                    std::cerr << "⚠️ Go daemon process exited with code: " << exitCode << std::endl;
                    daemonRunning_ = false;
                    connected_ = false;
                    break;
                }
            }
        }

        // Check every 5 seconds
        std::this_thread::sleep_for(std::chrono::seconds(5));
    }
}

void WalletService::cleanupDaemonProcess() {
    if (daemonProcess_.hProcess) {
        // Try to terminate gracefully first
        if (TerminateProcess(daemonProcess_.hProcess, 0)) {
            // Wait for process to exit
            WaitForSingleObject(daemonProcess_.hProcess, 5000);
        }

        CloseHandle(daemonProcess_.hProcess);
        CloseHandle(daemonProcess_.hThread);

        ZeroMemory(&daemonProcess_, sizeof(PROCESS_INFORMATION));
    }
}

// Console Control Handler Implementation
BOOL WINAPI WalletService::ConsoleCtrlHandler(DWORD ctrlType) {
    switch (ctrlType) {
        case CTRL_C_EVENT:
        case CTRL_BREAK_EVENT:
        case CTRL_CLOSE_EVENT:
        case CTRL_SHUTDOWN_EVENT:
            std::cout << "\n🛑 Console shutdown signal received - cleaning up daemon..." << std::endl;
            if (g_walletService) {
                g_walletService->stopDaemon();
            }
            return TRUE;
        default:
            return FALSE;
    }
}

nlohmann::json WalletService::sendTransaction(const nlohmann::json& transactionData) {
    std::cout << "🚀 Sending complete transaction..." << std::endl;
    std::cout << "📋 Transaction data: " << transactionData.dump() << std::endl;

    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "🚀 Sending complete transaction..." << std::endl;
    debugLog << "📋 Transaction data: " << transactionData.dump() << std::endl;
    debugLog.close();

    // Call the new /transaction/send endpoint
    std::string url = "/transaction/send";
    auto response = makeHttpRequest("POST", url, transactionData.dump());

    if (response.contains("success") && response["success"].get<bool>()) {
        std::cout << "✅ Transaction sent successfully" << std::endl;
        std::cout << "🔗 TxID: " << response["txid"].get<std::string>() << std::endl;

        std::ofstream debugLog2("debug_output.log", std::ios::app);
        debugLog2 << "✅ Transaction sent successfully" << std::endl;
        debugLog2 << "🔗 TxID: " << response["txid"].get<std::string>() << std::endl;
        debugLog2.close();

        return response;
    } else {
        std::cerr << "❌ Transaction failed: " << response.dump() << std::endl;

        std::ofstream debugLog3("debug_output.log", std::ios::app);
        debugLog3 << "❌ Transaction failed: " << response.dump() << std::endl;
        debugLog3.close();

        nlohmann::json errorResponse;
        errorResponse["error"] = "Transaction failed";
        return errorResponse;
    }
}

nlohmann::json WalletService::makeHttpRequestPublic(const std::string& method, const std::string& endpoint, const std::string& body) {
    return makeHttpRequest(method, endpoint, body);
}
