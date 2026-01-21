// macOS implementation of WalletService
// Uses libcurl for HTTP requests to Rust wallet backend

#include "../../include/core/WalletService.h"
#include "../../include/core/Logger.h"
#include <curl/curl.h>
#include <iostream>
#include <sstream>

// Convenience macros
#define LOG_DEBUG_WALLET(msg) Logger::Log(msg, 0, 0)
#define LOG_INFO_WALLET(msg) Logger::Log(msg, 1, 0)
#define LOG_WARNING_WALLET(msg) Logger::Log(msg, 2, 0)
#define LOG_ERROR_WALLET(msg) Logger::Log(msg, 3, 0)

// Callback for libcurl to write response data
static size_t WriteCallback(void* contents, size_t size, size_t nmemb, void* userp) {
    size_t total_size = size * nmemb;
    std::string* response = static_cast<std::string*>(userp);
    response->append(static_cast<char*>(contents), total_size);
    return total_size;
}

// ========== Constructor/Destructor ==========

WalletService::WalletService()
    : baseUrl_("http://localhost:3301")
    , daemonPath_("")
    , connected_(false)
    , daemonRunning_(false) {

    LOG_INFO_WALLET("🚀 WalletService constructor (macOS)");

    // Initialize libcurl globally (once per process)
    static bool curl_initialized = false;
    if (!curl_initialized) {
        curl_global_init(CURL_GLOBAL_ALL);
        curl_initialized = true;
    }

    // Test connection to Rust wallet
    if (!initializeConnection()) {
        LOG_WARNING_WALLET("⚠️ Failed to connect to Rust wallet at " + baseUrl_);
    } else {
        LOG_INFO_WALLET("✅ Connected to Rust wallet successfully");
    }
}

WalletService::~WalletService() {
    LOG_INFO_WALLET("🛑 WalletService destructor (macOS)");
    cleanupConnection();
}

// ========== Connection Management ==========

bool WalletService::initializeConnection() {
    // Test if wallet is reachable by making a health check
    try {
        auto result = makeHttpRequest("GET", "/health", "");
        if (result.contains("status") || result.contains("message")) {
            connected_ = true;
            LOG_INFO_WALLET("✅ Rust wallet is reachable at " + baseUrl_);
            return true;
        }
    } catch (...) {
        LOG_WARNING_WALLET("⚠️ Rust wallet not reachable (will retry on first request)");
    }

    // Connection will be attempted on first actual request
    connected_ = false;
    return false;
}

void WalletService::cleanupConnection() {
    connected_ = false;
    LOG_INFO_WALLET("✅ WalletService connection cleaned up");
}

bool WalletService::isConnected() {
    return connected_;
}

void WalletService::setBaseUrl(const std::string& url) {
    baseUrl_ = url;
    LOG_INFO_WALLET("🔄 Base URL set to: " + baseUrl_);
    connected_ = false;
    initializeConnection();
}

// ========== HTTP Request Implementation (libcurl) ==========

nlohmann::json WalletService::makeHttpRequest(const std::string& method, const std::string& endpoint, const std::string& body) {
    LOG_DEBUG_WALLET("🔍 HTTP " + method + " " + endpoint);

    CURL* curl = curl_easy_init();
    if (!curl) {
        LOG_ERROR_WALLET("❌ Failed to initialize libcurl");
        return nlohmann::json::object();
    }

    std::string url = baseUrl_ + endpoint;
    std::string response_body;

    // Set URL
    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());

    // Set method
    if (method == "POST") {
        curl_easy_setopt(curl, CURLOPT_POST, 1L);
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body.c_str());
    } else if (method == "PUT") {
        curl_easy_setopt(curl, CURLOPT_CUSTOMREQUEST, "PUT");
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body.c_str());
    } else if (method == "DELETE") {
        curl_easy_setopt(curl, CURLOPT_CUSTOMREQUEST, "DELETE");
    }
    // GET is default

    // Set headers
    struct curl_slist* headers = nullptr;
    headers = curl_slist_append(headers, "Content-Type: application/json");
    curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);

    // Set write callback
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, WriteCallback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &response_body);

    // Set timeout
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, 30L);
    curl_easy_setopt(curl, CURLOPT_CONNECTTIMEOUT, 10L);

    // Perform request
    CURLcode res = curl_easy_perform(curl);

    // Cleanup
    curl_slist_free_all(headers);

    if (res != CURLE_OK) {
        LOG_ERROR_WALLET("❌ HTTP request failed: " + std::string(curl_easy_strerror(res)));
        curl_easy_cleanup(curl);
        return nlohmann::json::object();
    }

    // Check HTTP status code
    long http_code = 0;
    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &http_code);
    curl_easy_cleanup(curl);

    LOG_DEBUG_WALLET("✅ HTTP " + std::to_string(http_code) + " (response: " + std::to_string(response_body.length()) + " bytes)");

    // Parse JSON response
    try {
        if (response_body.empty()) {
            return nlohmann::json::object();
        }
        return nlohmann::json::parse(response_body);
    } catch (const nlohmann::json::exception& e) {
        LOG_ERROR_WALLET("❌ Failed to parse JSON response: " + std::string(e.what()));
        LOG_ERROR_WALLET("   Response was: " + response_body);
        return nlohmann::json::object();
    }
}

nlohmann::json WalletService::makeHttpRequestPublic(const std::string& method, const std::string& endpoint, const std::string& body) {
    return makeHttpRequest(method, endpoint, body);
}

// ========== Daemon Management (Not Needed on macOS) ==========

// On macOS, developer runs Rust wallet manually in separate terminal
// No automatic daemon management needed

bool WalletService::startDaemon() {
    LOG_INFO_WALLET("ℹ️ Daemon management not implemented on macOS (run Rust wallet manually)");
    return false;
}

void WalletService::stopDaemon() {
    // No-op on macOS
}

bool WalletService::isDaemonRunning() {
    // Check if wallet is responding
    return isHealthy();
}

void WalletService::setDaemonPath(const std::string& path) {
    daemonPath_ = path;
    LOG_INFO_WALLET("ℹ️ Daemon path set (not used on macOS): " + path);
}

// ========== Initialization ==========

void WalletService::ensureInitialized() {
    if (!connected_) {
        initializeConnection();
    }
}

// ========== API Methods ==========

bool WalletService::isHealthy() {
    try {
        auto result = makeHttpRequest("GET", "/health", "");
        return !result.empty();
    } catch (...) {
        return false;
    }
}

nlohmann::json WalletService::getWalletStatus() {
    return makeHttpRequest("GET", "/wallet/status", "");
}

nlohmann::json WalletService::getWalletInfo() {
    return makeHttpRequest("GET", "/wallet/info", "");
}

nlohmann::json WalletService::createWallet() {
    return makeHttpRequest("POST", "/wallet/create", "");
}

nlohmann::json WalletService::loadWallet() {
    return makeHttpRequest("GET", "/wallet/load", "");
}

bool WalletService::markWalletBackedUp() {
    auto result = makeHttpRequest("POST", "/wallet/markBackedUp", "");
    return result.contains("success") && result["success"].get<bool>();
}

nlohmann::json WalletService::getAllAddresses() {
    return makeHttpRequest("GET", "/wallet/addresses", "");
}

nlohmann::json WalletService::generateAddress() {
    return makeHttpRequest("POST", "/wallet/address/generate", "");
}

nlohmann::json WalletService::getCurrentAddress() {
    return makeHttpRequest("GET", "/wallet/address/current", "");
}

nlohmann::json WalletService::createTransaction(const nlohmann::json& transactionData) {
    return makeHttpRequest("POST", "/createAction", transactionData.dump());
}

nlohmann::json WalletService::signTransaction(const nlohmann::json& transactionData) {
    return makeHttpRequest("POST", "/signAction", transactionData.dump());
}

nlohmann::json WalletService::broadcastTransaction(const nlohmann::json& transactionData) {
    return makeHttpRequest("POST", "/processAction", transactionData.dump());
}

nlohmann::json WalletService::sendTransaction(const nlohmann::json& transactionData) {
    return makeHttpRequest("POST", "/transaction/send", transactionData.dump());
}

nlohmann::json WalletService::getBalance(const nlohmann::json& balanceData) {
    return makeHttpRequest("GET", "/wallet/balance", "");
}

nlohmann::json WalletService::getTransactionHistory() {
    return makeHttpRequest("GET", "/wallet/transactions", "");
}
