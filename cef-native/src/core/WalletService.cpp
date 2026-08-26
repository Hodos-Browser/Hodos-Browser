#include "../../include/core/WalletService.h"
#include "../../include/core/Logger.h"
#include "../../include/core/PortConfig.h"
#include <iostream>
#include <sstream>
#include <fstream>
#include <chrono>
#include <iomanip>

// Convenience macros for easier logging
#define LOG_DEBUG_BROWSER(msg) Logger::Log(msg, 0, 2)
#define LOG_INFO_BROWSER(msg) Logger::Log(msg, 1, 2)
#define LOG_WARNING_BROWSER(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_BROWSER(msg) Logger::Log(msg, 3, 2)

// Static instance for console handler
static WalletService* g_walletService = nullptr;

WalletService::WalletService()
    : baseUrl_(hodos::WalletBaseUrl())
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

        // Initialize connection to Rust wallet
        if (!initializeConnection()) {
            LOG_WARNING_BROWSER("⚠️ Failed to connect to Rust wallet at " + baseUrl_);
        } else {
            LOG_DEBUG_BROWSER("✅ Connected to Rust wallet successfully");
        }

        LOG_DEBUG_BROWSER("✅ WalletService constructor completed");

    } catch (const std::exception& e) {
        LOG_ERROR_BROWSER("❌ WalletService constructor exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR_BROWSER("❌ WalletService constructor unknown exception");
    }
}

WalletService::~WalletService() {
    LOG_DEBUG_BROWSER(LogFmt() << "🛑 WalletService destructor called - shutting down daemon...");
    stopDaemon();
    cleanupConnection();
}

bool WalletService::initializeConnection() {
    // Initialize WinHTTP session
    hSession_ = WinHttpOpen(L"HodosBrowser/1.0",
                           WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                           WINHTTP_NO_PROXY_NAME,
                           WINHTTP_NO_PROXY_BYPASS,
                           0);

    if (!hSession_) {
        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to initialize WinHTTP session. Error: " << GetLastError());
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
        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to parse URL: " << baseUrl_);
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
        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to connect to Rust wallet at " << baseUrl_);
        return false;
    }

    connected_ = true;
    LOG_DEBUG_BROWSER(LogFmt() << "✅ Connected to Go wallet daemon at " << baseUrl_);
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

nlohmann::json WalletService::makeHttpRequest(const std::string& method, const std::string& endpoint,
                                              const std::string& body, int timeoutMs) {
    LOG_DEBUG_BROWSER("🔍 makeHttpRequest: " + method + " " + endpoint);

    if (!connected_) {
        LOG_ERROR_BROWSER("❌ Not connected to Rust wallet");
        return nlohmann::json::object();
    }

    try {
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
            DWORD error = GetLastError();
            LOG_ERROR_BROWSER("❌ Failed to create HTTP request. Error: " + std::to_string(error));
            return nlohmann::json::object();
        }

        // P2a-A1: bound the wait. Set PER REQUEST, not on the session: the session is
        // shared by every endpoint this WalletService serves, and /wallet/balance and
        // /transaction/send need very different budgets.
        //
        // ⛔ Before this, nothing in this function called WinHttpSetTimeouts at all, so
        // WinHTTP's 30 s receive default applied -- and because the call is synchronous on
        // the CEF UI thread, a non-answering wallet froze the whole browser for 30 s at a
        // time. MEASUREMENTS.md M5.
        //
        // Resolve is left at 0 (infinite) as WinHTTP recommends for a literal IP; connect
        // is capped short because this is loopback -- if 127.0.0.1 will not accept inside
        // a second, waiting longer does not help.
        //
        // ⚠️ WinHttpSetTimeouts is PER PHASE, while macOS's CURLOPT_TIMEOUT_MS is a TOTAL.
        // Passing timeoutMs to both send and receive therefore meant "up to 2 × timeoutMs"
        // on Windows and "exactly timeoutMs" on macOS -- measured as a 3.73 s failure
        // against a documented 2000 ms budget. Send gets the same short fixed budget as
        // connect (the request body is a few hundred bytes to loopback; if that will not
        // go out in a second, waiting is pointless), so recv carries the budget and the
        // number in the constant means what it says on both platforms.
        {
            const DWORD resolveMs = 0;
            const DWORD connectMs = 1000;
            const DWORD sendMs    = 1000;
            const DWORD recvMs    = static_cast<DWORD>(timeoutMs);
            if (!WinHttpSetTimeouts(hRequest, resolveMs, connectMs, sendMs, recvMs)) {
                // Not fatal -- we fall back to WinHTTP's defaults, which is the old
                // behaviour. Logged because silently keeping a 30 s freeze is exactly the
                // failure this row exists to prevent.
                LOG_WARNING_BROWSER("⚠️ WinHttpSetTimeouts failed (" + std::to_string(GetLastError())
                                    + ") - falling back to WinHTTP defaults for " + endpoint);
            }
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
            DWORD error = GetLastError();
            LOG_ERROR_BROWSER("❌ Failed to send HTTP request. Error: " + std::to_string(error));
            WinHttpCloseHandle(hRequest);
            return nlohmann::json::object();
        }

        // Receive response
        if (!WinHttpReceiveResponse(hRequest, nullptr)) {
            DWORD error = GetLastError();
            LOG_ERROR_BROWSER("❌ Failed to receive HTTP reply. Error: " + std::to_string(error));
            WinHttpCloseHandle(hRequest);
            return nlohmann::json::object();
        }

        // Read response body
        std::string responseBody = readResponse(hRequest);
        WinHttpCloseHandle(hRequest);

        // P0-A8: the response body is NEVER logged. /wallet/create's body carries the
        // BIP39 recovery phrase and fits inside the old 500-char cap, so any sink here
        // is one route change away from writing key material to disk. Length only, and
        // via a local that does not name the body (gate G5 flags the shape, not just
        // this instance).
        const size_t bodyLen = responseBody.length();
        LOG_DEBUG_BROWSER("✅ Response received (length: " + std::to_string(bodyLen) + ")");

        // Parse JSON response
        try {
            nlohmann::json parsed = nlohmann::json::parse(responseBody);
            return parsed;
        } catch (const std::exception& e) {
            // P0-A8: parser error only. The old code wrote the ENTIRE body here with no
            // size cap at all -- the widest of the three sinks in this function.
            LOG_ERROR_BROWSER("❌ Failed to parse wallet JSON (" + std::to_string(bodyLen)
                              + " bytes): " + std::string(e.what()));
            return nlohmann::json::object();
        }
    } catch (const std::exception& e) {
        LOG_ERROR_BROWSER("❌ Exception in makeHttpRequest: " + std::string(e.what()));
        return nlohmann::json::object();
    } catch (...) {
        LOG_ERROR_BROWSER("❌ Unknown exception in makeHttpRequest");
        return nlohmann::json::object();
    }
}

std::string WalletService::readResponse(HINTERNET hRequest) {
    std::string response;
    DWORD dwSize = 0;
    DWORD dwDownloaded = 0;
    int chunks = 0;

    do {
        dwSize = 0;
        if (!WinHttpQueryDataAvailable(hRequest, &dwSize)) {
            LOG_ERROR_BROWSER("❌ WinHttpQueryDataAvailable failed. Error: "
                              + std::to_string(GetLastError()));
            break;
        }

        if (dwSize == 0) {
            break;
        }

        std::vector<char> buffer(dwSize + 1);
        if (!WinHttpReadData(hRequest, buffer.data(), dwSize, &dwDownloaded)) {
            LOG_ERROR_BROWSER("❌ WinHttpReadData failed. Error: "
                              + std::to_string(GetLastError()));
            break;
        }

        response.append(buffer.data(), dwDownloaded);
        chunks++;
    } while (dwSize > 0);

    // P0-A8: the assembled body is NEVER logged (the old sink wrote it whole under a
    // 1000-char cap). Per-chunk progress lines are gone too: they were the bulk of the
    // four open/write/close cycles this file did on EVERY wallet call.
    return response;
}

bool WalletService::isHealthy() {
    LOG_DEBUG_BROWSER(LogFmt() << "🔍 Checking Rust wallet health...");

    auto response = makeHttpRequest("GET", "/health");

    if (response.contains("status") && response["status"] == "ok") {
        LOG_DEBUG_BROWSER(LogFmt() << "✅ Rust wallet is healthy");
        return true;
    } else {
        LOG_ERROR_BROWSER(LogFmt() << "❌ Rust wallet health check failed");
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
            // Production: same directory as browser exe
            daemonPath_ = exeDir + "\\hodos-wallet.exe";
            if (GetFileAttributesA(daemonPath_.c_str()) == INVALID_FILE_ATTRIBUTES) {
                // Dev fallback: source tree relative path
                daemonPath_ = exeDir + "\\..\\..\\..\\..\\rust-wallet\\target\\release\\hodos-wallet.exe";
            }
        }

        // Set up console control handler
        SetConsoleCtrlHandler(ConsoleCtrlHandler, TRUE);

        // Initialize HTTP connection to Rust wallet
        if (initializeConnection()) {
            LOG_DEBUG_BROWSER("✅ HTTP connection to Rust wallet established");
        } else {
            LOG_WARNING_BROWSER("⚠️ Failed to establish HTTP connection to Rust wallet");
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
    LOG_DEBUG_BROWSER("🔍 Getting wallet status from Rust wallet...");

    // Ensure WalletService is properly initialized
    ensureInitialized();

    try {
        // Make actual HTTP request to Rust wallet
        LOG_DEBUG_BROWSER("🔄 Making HTTP request to /wallet/status...");

        auto response = makeHttpRequest("GET", "/wallet/status");

        if (response.contains("exists")) {
            LOG_DEBUG_BROWSER("✅ Wallet status retrieved successfully from Rust wallet");

            // Add needsBackup field if not present (for backward compatibility)
            if (!response.contains("needsBackup")) {
                response["needsBackup"] = false;
            }

            return response;
        } else {
            LOG_WARNING_BROWSER("⚠️ Unexpected reply format from Rust wallet");
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
    fallbackResponse["error"] = "Failed to connect to Rust wallet";

    LOG_WARNING_BROWSER("📤 Returning fallback payload due to connection error");

    return fallbackResponse;
}

// ⛔ P0-A8 / gate G5: these functions must NOT log values read out of a wallet HTTP
// response. Five such lines were removed in beta.3 P2b -- wallet version, backed-up flag,
// address count, current address and newly generated address.
//
// They were harmless while std::cout went to NUL. Converting the blackhole to a REAL sink
// is exactly what turns a dormant line into disclosure: a BSV address in a log kept for 30
// days links the user to their on-chain activity, and G5 guards the SHAPE (a wallet
// response reaching a sink) precisely so nobody has to re-litigate which field is safe.
// The success/failure events are kept; the values are not.
nlohmann::json WalletService::getWalletInfo() {
    LOG_DEBUG_BROWSER(LogFmt() << "🔍 Getting wallet info from Rust wallet...");

    auto response = makeHttpRequest("GET", "/wallet/info");

    if (response.contains("version")) {
        LOG_INFO_BROWSER(LogFmt() << "✅ Wallet info retrieved successfully");
        return response;
    } else {
        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to get wallet info from Rust wallet");
        return nlohmann::json::object();
    }
}

nlohmann::json WalletService::createWallet() {
    LOG_INFO_BROWSER(LogFmt() << "🔍 Creating new wallet via Rust wallet...");

    auto response = makeHttpRequest("POST", "/wallet/create");

    if (response.contains("success") && response["success"].get<bool>()) {
        LOG_INFO_BROWSER(LogFmt() << "✅ Wallet created successfully");
        return response;
    } else {
        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to create wallet from Rust wallet");
        return nlohmann::json::object();
    }
}

nlohmann::json WalletService::loadWallet() {
    LOG_INFO_BROWSER(LogFmt() << "🔍 Loading wallet from Rust wallet...");

    auto response = makeHttpRequest("POST", "/wallet/load");

    if (response.contains("success") && response["success"].get<bool>()) {
        LOG_INFO_BROWSER(LogFmt() << "✅ Wallet loaded successfully");
        return response;
    } else {
        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to load wallet from Rust wallet");
        return nlohmann::json::object();
    }
}

bool WalletService::markWalletBackedUp() {
    LOG_INFO_BROWSER(LogFmt() << "🔍 Marking wallet as backed up...");

    auto response = makeHttpRequest("POST", "/wallet/markBackedUp");

    if (response.contains("success") && response["success"] == true) {
        LOG_INFO_BROWSER(LogFmt() << "✅ Wallet marked as backed up successfully");
        return true;
    } else {
        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to mark wallet as backed up");
        return false;
    }
}

// Address Management Methods

nlohmann::json WalletService::getAllAddresses() {
    LOG_DEBUG_BROWSER(LogFmt() << "🔍 Getting all addresses from Rust wallet...");

    auto response = makeHttpRequest("GET", "/wallet/addresses");

    if (response.is_array()) {
        LOG_INFO_BROWSER(LogFmt() << "✅ Addresses retrieved successfully");
        return response;
    } else {
        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to get addresses from Rust wallet");
        return nlohmann::json::array();
    }
}

nlohmann::json WalletService::getCurrentAddress() {
    LOG_DEBUG_BROWSER(LogFmt() << "🔍 Getting current address from Rust wallet...");

    auto response = makeHttpRequest("GET", "/wallet/address/current");

    if (response.contains("address")) {
        LOG_INFO_BROWSER(LogFmt() << "✅ Current address retrieved successfully");
        return response;
    } else {
        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to get current address from Rust wallet");
        return nlohmann::json::object();
    }
}

nlohmann::json WalletService::generateAddress() {
    LOG_INFO_BROWSER(LogFmt() << "🔍 Generating new address from Rust wallet...");

    auto response = makeHttpRequest("POST", "/wallet/address/generate");

    if (response.contains("address")) {
        LOG_INFO_BROWSER(LogFmt() << "✅ Address generated successfully");
        return response;
    } else {
        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to generate address from Rust wallet");
        return nlohmann::json::object();
    }
}


// Transaction Methods Implementation

nlohmann::json WalletService::createTransaction(const nlohmann::json& transactionData) {
    // P0-A1: the request body is no longer logged -- it carries destination addresses
    // and amounts, and this ran unconditionally in production.
    LOG_DEBUG_BROWSER("💰 Creating transaction via Rust wallet...");

    auto response = makeHttpRequest("POST", "/transaction/create", transactionData.dump());

    if (response.contains("txid")) {
        const std::string txid = response["txid"].get<std::string>();
        LOG_INFO_BROWSER("✅ Transaction created successfully: " + txid);
        return response;
    } else {
        // P0-A8: the error FIELD, not the whole envelope.
        const std::string err = response.value("error", std::string("unknown error"));
        LOG_ERROR_BROWSER("❌ Failed to create transaction: " + err);
        return response; // Return the error envelope
    }
}

nlohmann::json WalletService::signTransaction(const nlohmann::json& transactionData) {
    // P0-A1: the request body is no longer logged -- it carries destination addresses
    // and amounts, and this ran unconditionally in production.
    LOG_DEBUG_BROWSER("✍️ Signing transaction via Rust wallet...");

    auto response = makeHttpRequest("POST", "/transaction/sign", transactionData.dump());

    if (response.contains("txid")) {
        const std::string txid = response["txid"].get<std::string>();
        LOG_INFO_BROWSER("✅ Transaction signed successfully: " + txid);
        return response;
    } else {
        // P0-A8: the error FIELD, not the whole envelope.
        const std::string err = response.value("error", std::string("unknown error"));
        LOG_ERROR_BROWSER("❌ Failed to sign transaction: " + err);
        return response; // Return the error envelope
    }
}

nlohmann::json WalletService::broadcastTransaction(const nlohmann::json& transactionData) {
    // P0-A1: the request body is no longer logged -- it carries destination addresses
    // and amounts, and this ran unconditionally in production.
    LOG_DEBUG_BROWSER("📡 Broadcasting transaction via Rust wallet...");

    auto response = makeHttpRequest("POST", "/transaction/broadcast", transactionData.dump(),
                                    kWalletBroadcastTimeoutMs);

    if (response.contains("txid")) {
        const std::string txid = response["txid"].get<std::string>();
        LOG_INFO_BROWSER("✅ Transaction broadcast successfully: " + txid);
        return response;
    } else {
        // P0-A8: the error FIELD, not the whole envelope.
        const std::string err = response.value("error", std::string("unknown error"));
        LOG_ERROR_BROWSER("❌ Failed to broadcast transaction: " + err);
        return response; // Return the error envelope
    }
}

nlohmann::json WalletService::getBalance(const nlohmann::json& balanceData) {
    // P0-A1: this is the hottest path in the browser -- 13,643 calls in one dev log,
    // 2,551 in a single production session. It opened/wrote/closed a file in {app}
    // FOUR times per call. Now one level-gated Logger line, and the request payload
    // (always "{}" here) is not logged at all.
    LOG_DEBUG_BROWSER("💰 Getting total balance from Rust wallet...");

    // Use the total balance endpoint (no address needed)
    std::string url = "/wallet/balance";
    auto response = makeHttpRequest("GET", url, "", kWalletBalanceTimeoutMs);

    if (response.contains("balance")) {
        int64_t totalBalance = response["balance"].get<int64_t>();
        double bsvPrice = response.value("bsvPrice", 0.0);

        LOG_DEBUG_BROWSER("💵 Total Balance: " + std::to_string(totalBalance)
                          + " satoshis, BSV/USD: $" + std::to_string(bsvPrice));

        // Return balance + price in expected format
        nlohmann::json balanceResponse;
        balanceResponse["balance"] = totalBalance;
        balanceResponse["bsvPrice"] = bsvPrice;
        return balanceResponse;
    } else {
        const std::string err = response.value("error", std::string("unknown error"));
        LOG_ERROR_BROWSER("❌ Failed to get total balance: " + err);

        // Return error response
        nlohmann::json errorResponse;
        errorResponse["error"] = "Failed to fetch total balance";
        return errorResponse;
    }
}

nlohmann::json WalletService::getTransactionHistory() {
    LOG_DEBUG_BROWSER("📜 Getting transaction history from Rust wallet...");

    auto response = makeHttpRequest("GET", "/transaction/history");

    if (response.is_array() || response.contains("transactions")) {
        LOG_DEBUG_BROWSER("✅ Transaction history retrieved successfully");
        return response;
    } else {
        const std::string err = response.value("error", std::string("unknown error"));
        LOG_ERROR_BROWSER("❌ Failed to get transaction history: " + err);
        return response; // Return the error envelope
    }
}

// Daemon Process Management Methods

bool WalletService::startDaemon() {
    if (daemonRunning_) {
        LOG_INFO_BROWSER(LogFmt() << "🔄 Go daemon already running");
        return true;
    }

    LOG_INFO_BROWSER(LogFmt() << "🚀 Starting Go wallet daemon...");

    if (createDaemonProcess()) {
        daemonRunning_ = true;
        monitorThread_ = std::thread(&WalletService::monitorDaemon, this);
        LOG_INFO_BROWSER(LogFmt() << "✅ Go daemon started successfully");
        return true;
    } else {
        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to start Go daemon");
        return false;
    }
}

void WalletService::stopDaemon() {
    if (!daemonRunning_) {
        return;
    }

    LOG_INFO_BROWSER(LogFmt() << "🛑 Stopping Go wallet daemon...");

    daemonRunning_ = false;

    if (monitorThread_.joinable()) {
        monitorThread_.join();
    }

    cleanupDaemonProcess();
    LOG_INFO_BROWSER(LogFmt() << "✅ Go daemon stopped");
}

bool WalletService::isDaemonRunning() {
    return daemonRunning_;
}

void WalletService::setDaemonPath(const std::string& path) {
    daemonPath_ = path;
}

bool WalletService::createDaemonProcess() {
    if (daemonPath_.empty()) {
        LOG_ERROR_BROWSER(LogFmt() << "❌ Daemon path not set");
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

        LOG_ERROR_BROWSER(LogFmt() << "❌ Failed to create daemon process. Error: " << GetLastError());
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
                    LOG_WARNING_BROWSER(LogFmt() << "⚠️ Go daemon process exited with code: " << exitCode);
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
            LOG_INFO_BROWSER(LogFmt() << "\n🛑 Console shutdown signal received - cleaning up daemon...");
            if (g_walletService) {
                g_walletService->stopDaemon();
            }
            return TRUE;
        default:
            return FALSE;
    }
}

nlohmann::json WalletService::sendTransaction(const nlohmann::json& transactionData) {
    try {
        // Call the /transaction/send endpoint and forward the response directly to the frontend
        // The frontend will parse and handle success/failure
        std::string url = "/transaction/send";
        auto response = makeHttpRequest("POST", url, transactionData.dump(),
                                        kWalletBroadcastTimeoutMs);
        return response;
    } catch (const std::exception& e) {
        LOG_ERROR_BROWSER("❌ Exception in sendTransaction: " + std::string(e.what()));

        // Return a safe error response
        nlohmann::json errorResponse;
        errorResponse["success"] = false;
        errorResponse["error"] = "Failed to process transaction response";
        errorResponse["message"] = std::string("Error: ") + e.what();
        errorResponse["status"] = "failed";
        return errorResponse;
    } catch (...) {
        LOG_ERROR_BROWSER("❌ Unknown exception in sendTransaction");

        // Return a safe error response
        nlohmann::json errorResponse;
        errorResponse["success"] = false;
        errorResponse["error"] = "Unknown error occurred";
        errorResponse["message"] = "Unknown error occurred";
        errorResponse["status"] = "failed";
        return errorResponse;
    }
}

nlohmann::json WalletService::makeHttpRequestPublic(const std::string& method, const std::string& endpoint, const std::string& body) {
    return makeHttpRequest(method, endpoint, body);
}
