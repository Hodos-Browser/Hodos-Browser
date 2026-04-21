#include "../../include/core/HttpRequestInterceptor.h"
#include "../../include/core/CookieBlockManager.h"
#include "include/wrapper/cef_helpers.h"
#include "include/cef_urlrequest.h"
#include "include/cef_request.h"
#include "include/cef_request_context.h"
#include "include/cef_browser.h"
#include "include/cef_task.h"
#include "include/cef_v8.h"
#include "include/cef_frame.h"
#include "../handlers/simple_handler.h"
#include "../handlers/simple_app.h"
#include <iostream>

#include "../../include/core/PendingAuthRequest.h"
#include "../../include/core/SessionManager.h"

// Forward declaration
class AsyncWalletResourceHandler;

// g_pendingModalDomain kept as a quick-check for the overlay JS — will be
// removed once the notification UI (Phase 2.3) handles requestIds natively.
std::string g_pendingModalDomain = "";
#include <sstream>
#include <algorithm>
#include <fstream>
#include <mutex>
#include <condition_variable>
#include <atomic>
#include <set>
#include <nlohmann/json.hpp>
#include <cstdlib>
#include <ctime>
#include <chrono>
#include <iomanip>
#ifdef _WIN32
#include <windows.h>
#include <winhttp.h>
#pragma comment(lib, "winhttp.lib")
#endif
#include "../../include/core/SyncHttpClient.h"
#include "../../include/core/Logger.h"

// Logging macros for HTTP interceptor
#define LOG_DEBUG_HTTP(msg) Logger::Log(msg, 0, 2)
#define LOG_INFO_HTTP(msg) Logger::Log(msg, 1, 2)
#define LOG_WARNING_HTTP(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_HTTP(msg) Logger::Log(msg, 3, 2)

// In-memory cache for domain permissions backed by the Rust DB via REST
class DomainPermissionCache {
public:
    struct Permission {
        std::string trustLevel;         // "blocked"|"unknown"|"approved"
        int64_t perTxLimitCents = 100;       // $1.00
        int64_t perSessionLimitCents = 1000; // $10.00
        int64_t rateLimitPerMin = 30;
        int64_t maxTxPerSession = 100;       // max transactions per session
        bool adblockEnabled = true;     // Per-site ad blocking toggle (Sprint 8c)
    };

    static DomainPermissionCache& GetInstance() {
        static DomainPermissionCache instance;
        return instance;
    }

    // Lookup: cached first, then fetches from Rust backend synchronously
    Permission getPermission(const std::string& domain) {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            auto it = cache_.find(domain);
            if (it != cache_.end()) {
                return it->second;
            }
        }
        Permission perm = fetchFromBackend(domain);
        {
            std::lock_guard<std::mutex> lock(mutex_);
            cache_[domain] = perm;
        }
        return perm;
    }

    void set(const std::string& domain, const Permission& perm) {
        std::lock_guard<std::mutex> lock(mutex_);
        cache_[domain] = perm;
    }

    void invalidate(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        cache_.erase(domain);
    }

    void clear() {
        std::lock_guard<std::mutex> lock(mutex_);
        cache_.clear();
    }

private:
    DomainPermissionCache() = default;
    DomainPermissionCache(const DomainPermissionCache&) = delete;
    DomainPermissionCache& operator=(const DomainPermissionCache&) = delete;

    std::mutex mutex_;
    std::unordered_map<std::string, Permission> cache_;

#ifdef _WIN32
    // Reusable WinHTTP session handle — thread-safe per MSDN (P2 perf fix)
    HINTERNET hSession_ = nullptr;

    HINTERNET getSession() {
        if (!hSession_) {
            hSession_ = WinHttpOpen(L"DomainPermissionCache/1.0",
                                    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                                    WINHTTP_NO_PROXY_NAME,
                                    WINHTTP_NO_PROXY_BYPASS, 0);
        }
        return hSession_;
    }

    Permission fetchFromBackend(const std::string& domain) {
        Permission result;
        result.trustLevel = "unknown";

        HINTERNET hSession = getSession();
        if (!hSession) return result;

        HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 31301, 0);
        if (!hConnect) return result;

        std::string endpoint = "/domain/permissions?domain=" + domain;
        std::wstring wideEndpoint(endpoint.begin(), endpoint.end());

        HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"GET",
                                                wideEndpoint.c_str(),
                                                nullptr,
                                                WINHTTP_NO_REFERER,
                                                WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
        if (!hRequest) {
            WinHttpCloseHandle(hConnect);
            return result;
        }

        // 1s timeout for localhost (P2 perf fix — reduced from 5s)
        DWORD timeout = 1000;
        WinHttpSetOption(hRequest, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hRequest, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hRequest, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));

        if (!WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0, nullptr, 0, 0, 0) ||
            !WinHttpReceiveResponse(hRequest, nullptr)) {
            WinHttpCloseHandle(hRequest);
            WinHttpCloseHandle(hConnect);
            return result;
        }

        std::string responseBody;
        responseBody.reserve(512);
        DWORD bytesRead = 0;
        char buffer[4096];
        do {
            if (!WinHttpReadData(hRequest, buffer, sizeof(buffer), &bytesRead)) break;
            responseBody.append(buffer, bytesRead);
        } while (bytesRead > 0);

        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);

        try {
            auto json = nlohmann::json::parse(responseBody);
            result.trustLevel = json.value("trustLevel", "unknown");
            result.perTxLimitCents = json.value("perTxLimitCents", (int64_t)100);
            result.perSessionLimitCents = json.value("perSessionLimitCents", (int64_t)1000);
            result.rateLimitPerMin = json.value("rateLimitPerMin", (int64_t)30);
            result.maxTxPerSession = json.value("maxTxPerSession", (int64_t)100);
            result.adblockEnabled = json.value("adblockEnabled", true);
        } catch (const std::exception& e) {
            LOG_DEBUG_HTTP("🔒 Failed to parse domain permission response: " + std::string(e.what()));
        }

        return result;
    }
#else
    Permission fetchFromBackend(const std::string& domain) {
        Permission result;
        result.trustLevel = "unknown";

        std::string url = "http://localhost:31301/domain/permissions?domain=" + domain;
        HttpResponse resp = SyncHttpClient::Get(url, 5000);
        if (!resp.success) return result;

        try {
            auto json = nlohmann::json::parse(resp.body);
            result.trustLevel = json.value("trustLevel", "unknown");
            result.perTxLimitCents = json.value("perTxLimitCents", (int64_t)100);
            result.perSessionLimitCents = json.value("perSessionLimitCents", (int64_t)1000);
            result.rateLimitPerMin = json.value("rateLimitPerMin", (int64_t)30);
            result.maxTxPerSession = json.value("maxTxPerSession", (int64_t)100);
            result.adblockEnabled = json.value("adblockEnabled", true);
        } catch (const std::exception& e) {
            LOG_DEBUG_HTTP("Failed to parse domain permission response: " + std::string(e.what()));
        }

        return result;
    }
#endif
};

// Cached wallet existence check — avoids pointless domain approval when no wallet exists
class WalletStatusCache {
public:
    static WalletStatusCache& GetInstance() {
        static WalletStatusCache instance;
        return instance;
    }

    // P2 perf fix: mutex released before blocking I/O to allow concurrent cached reads
    bool walletExists() {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            auto now = std::chrono::steady_clock::now();
            if (valid_ && (now - lastCheck_) < std::chrono::seconds(30)) {
                return exists_;
            }
        }
        bool result = fetchWalletStatus();
        {
            std::lock_guard<std::mutex> lock(mutex_);
            exists_ = result;
            valid_ = true;
            lastCheck_ = std::chrono::steady_clock::now();
        }
        return result;
    }

    void invalidate() {
        std::lock_guard<std::mutex> lock(mutex_);
        valid_ = false;
    }

private:
    WalletStatusCache() = default;
    WalletStatusCache(const WalletStatusCache&) = delete;
    WalletStatusCache& operator=(const WalletStatusCache&) = delete;

    std::mutex mutex_;
    bool exists_ = false;
    bool valid_ = false;
    std::chrono::steady_clock::time_point lastCheck_;

#ifdef _WIN32
    HINTERNET hSession_ = nullptr;

    HINTERNET getSession() {
        if (!hSession_) {
            hSession_ = WinHttpOpen(L"WalletStatusCache/1.0",
                                    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                                    WINHTTP_NO_PROXY_NAME,
                                    WINHTTP_NO_PROXY_BYPASS, 0);
        }
        return hSession_;
    }

    bool fetchWalletStatus() {
        HINTERNET hSession = getSession();
        if (!hSession) return false;

        HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 31301, 0);
        if (!hConnect) return false;

        HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"GET",
                                                L"/wallet/status",
                                                nullptr,
                                                WINHTTP_NO_REFERER,
                                                WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
        if (!hRequest) {
            WinHttpCloseHandle(hConnect);
            return false;
        }

        DWORD timeout = 1000;
        WinHttpSetOption(hRequest, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hRequest, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hRequest, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));

        if (!WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0, nullptr, 0, 0, 0) ||
            !WinHttpReceiveResponse(hRequest, nullptr)) {
            WinHttpCloseHandle(hRequest);
            WinHttpCloseHandle(hConnect);
            return false;
        }

        std::string responseBody;
        responseBody.reserve(256);
        DWORD bytesRead = 0;
        char buffer[1024];
        do {
            if (!WinHttpReadData(hRequest, buffer, sizeof(buffer), &bytesRead)) break;
            responseBody.append(buffer, bytesRead);
        } while (bytesRead > 0);

        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);

        try {
            auto json = nlohmann::json::parse(responseBody);
            return json.value("exists", false);
        } catch (...) {
            return false;
        }
    }
#else
    bool fetchWalletStatus() {
        HttpResponse resp = SyncHttpClient::Get("http://localhost:31301/wallet/status", 1000);
        if (!resp.success) return false;

        try {
            auto json = nlohmann::json::parse(resp.body);
            return json.value("exists", false);
        } catch (...) {
            return false;
        }
    }
#endif
};

// Cached BSV/USD price from Rust backend — used by auto-approve engine for satoshi→USD conversion
class BSVPriceCache {
public:
    static BSVPriceCache& GetInstance() {
        static BSVPriceCache instance;
        return instance;
    }

    // P2 perf fix: mutex released before blocking I/O
    double getPrice() {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            auto now = std::chrono::steady_clock::now();
            if (valid_ && (now - lastCheck_) < std::chrono::minutes(5)) {
                return priceUsd_;
            }
        }
        double fetched = fetchFromBackend();
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (fetched > 0.0) {
                priceUsd_ = fetched;
                lastSuccessfulPrice_ = fetched;
            } else if (lastSuccessfulPrice_ > 0.0) {
                priceUsd_ = lastSuccessfulPrice_;
                LOG_DEBUG_HTTP("⚠️ BSVPriceCache: fetch failed, using stale price $" + std::to_string(lastSuccessfulPrice_));
            } else {
                priceUsd_ = -1.0;
                LOG_DEBUG_HTTP("⚠️ BSVPriceCache: fetch failed and no stale price available");
            }
            valid_ = true;
            lastCheck_ = std::chrono::steady_clock::now();
            return priceUsd_;
        }
    }

    void invalidate() {
        std::lock_guard<std::mutex> lock(mutex_);
        valid_ = false;
    }

private:
    BSVPriceCache() = default;
    BSVPriceCache(const BSVPriceCache&) = delete;
    BSVPriceCache& operator=(const BSVPriceCache&) = delete;

    std::mutex mutex_;
    double priceUsd_ = -1.0;
    double lastSuccessfulPrice_ = -1.0;
    bool valid_ = false;
    std::chrono::steady_clock::time_point lastCheck_;

#ifdef _WIN32
    HINTERNET hSession_ = nullptr;

    HINTERNET getSession() {
        if (!hSession_) {
            hSession_ = WinHttpOpen(L"BSVPriceCache/1.0",
                                    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                                    WINHTTP_NO_PROXY_NAME,
                                    WINHTTP_NO_PROXY_BYPASS, 0);
        }
        return hSession_;
    }

    double fetchFromBackend() {
        HINTERNET hSession = getSession();
        if (!hSession) return -1.0;

        HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 31301, 0);
        if (!hConnect) return -1.0;

        HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"GET",
                                                L"/wallet/bsv-price",
                                                nullptr,
                                                WINHTTP_NO_REFERER,
                                                WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
        if (!hRequest) {
            WinHttpCloseHandle(hConnect);
            return -1.0;
        }

        DWORD timeout = 1000;
        WinHttpSetOption(hRequest, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hRequest, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hRequest, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));

        if (!WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0, nullptr, 0, 0, 0) ||
            !WinHttpReceiveResponse(hRequest, nullptr)) {
            WinHttpCloseHandle(hRequest);
            WinHttpCloseHandle(hConnect);
            return -1.0;
        }

        std::string responseBody;
        responseBody.reserve(256);
        DWORD bytesRead = 0;
        char buffer[1024];
        do {
            if (!WinHttpReadData(hRequest, buffer, sizeof(buffer), &bytesRead)) break;
            responseBody.append(buffer, bytesRead);
        } while (bytesRead > 0);

        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);

        try {
            auto json = nlohmann::json::parse(responseBody);
            double price = json.value("priceUsd", -1.0);
            return price > 0.0 ? price : -1.0;
        } catch (...) {
            return -1.0;
        }
    }
#else
    double fetchFromBackend() {
        HttpResponse resp = SyncHttpClient::Get("http://localhost:31301/wallet/bsv-price", 1000);
        if (!resp.success) return -1.0;

        try {
            auto json = nlohmann::json::parse(resp.body);
            double price = json.value("priceUsd", -1.0);
            return price > 0.0 ? price : -1.0;
        } catch (...) {
            return -1.0;
        }
    }
#endif
};

// URL-encode a string (for query parameters that may contain +, =, / etc.)
static std::string urlEncode(const std::string& value) {
    std::ostringstream encoded;
    encoded.fill('0');
    encoded << std::hex;
    for (unsigned char c : value) {
        if (isalnum(c) || c == '-' || c == '_' || c == '.' || c == '~') {
            encoded << c;
        } else {
            encoded << '%' << std::setw(2) << std::uppercase << (int)c;
        }
    }
    return encoded.str();
}

#ifdef _WIN32
// Fetch approved cert fields from Rust backend (synchronous WinHTTP).
// Returns set of field names that are already approved for this domain + cert_type.
static std::set<std::string> fetchCertFieldsFromBackend(const std::string& domain, const std::string& certType) {
    std::set<std::string> result;

    HINTERNET hSession = WinHttpOpen(L"CertFieldCache/1.0",
                                     WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                                     WINHTTP_NO_PROXY_NAME,
                                     WINHTTP_NO_PROXY_BYPASS, 0);
    if (!hSession) return result;

    HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 31301, 0);
    if (!hConnect) {
        WinHttpCloseHandle(hSession);
        return result;
    }

    // Build endpoint with URL-encoded cert_type (base64 may contain +, =, /)
    std::string endpoint = "/domain/permissions/certificate?domain=" + urlEncode(domain)
                         + "&cert_type=" + urlEncode(certType);
    std::wstring wideEndpoint(endpoint.begin(), endpoint.end());

    HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"GET",
                                            wideEndpoint.c_str(),
                                            nullptr,
                                            WINHTTP_NO_REFERER,
                                            WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
    if (!hRequest) {
        WinHttpCloseHandle(hConnect);
        WinHttpCloseHandle(hSession);
        return result;
    }

    DWORD timeout = 1000;  // P2 perf fix — reduced from 5s for localhost
    WinHttpSetOption(hRequest, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hRequest, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hRequest, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));

    if (!WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0, nullptr, 0, 0, 0) ||
        !WinHttpReceiveResponse(hRequest, nullptr)) {
        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);
        WinHttpCloseHandle(hSession);
        return result;
    }

    std::string responseBody;
    responseBody.reserve(1024);  // Typical cert fields JSON < 1KB
    DWORD bytesRead = 0;
    char buffer[4096];
    do {
        if (!WinHttpReadData(hRequest, buffer, sizeof(buffer), &bytesRead)) break;
        responseBody.append(buffer, bytesRead);
    } while (bytesRead > 0);

    WinHttpCloseHandle(hRequest);
    WinHttpCloseHandle(hConnect);
    WinHttpCloseHandle(hSession);

    try {
        auto json = nlohmann::json::parse(responseBody);
        if (json.contains("approvedFields") && json["approvedFields"].is_array()) {
            for (const auto& field : json["approvedFields"]) {
                if (field.is_string()) {
                    result.insert(field.get<std::string>());
                }
            }
        }
    } catch (const std::exception& e) {
        LOG_DEBUG_HTTP("📋 Failed to parse cert field permissions response: " + std::string(e.what()));
    }

    return result;
}
#else
static std::set<std::string> fetchCertFieldsFromBackend(const std::string& domain, const std::string& certType) {
    std::set<std::string> result;

    std::string url = "http://localhost:31301/domain/permissions/certificate?domain="
                    + urlEncode(domain) + "&cert_type=" + urlEncode(certType);
    HttpResponse resp = SyncHttpClient::Get(url, 5000);
    if (!resp.success) return result;

    try {
        auto json = nlohmann::json::parse(resp.body);
        if (json.contains("approvedFields") && json["approvedFields"].is_array()) {
            for (const auto& field : json["approvedFields"]) {
                if (field.is_string()) {
                    result.insert(field.get<std::string>());
                }
            }
        }
    } catch (const std::exception& e) {
        LOG_DEBUG_HTTP("Failed to parse cert field permissions response: " + std::string(e.what()));
    }

    return result;
}
#endif

// Escape string for safe embedding in single-quoted JS string literals.
// Prevents JS injection when concatenating into ExecuteJavaScript() calls.
static std::string escapeForJsSingleQuote(const std::string& input) {
    std::string escaped;
    escaped.reserve(input.length() + 16);
    for (char c : input) {
        switch (c) {
            case '\\': escaped += "\\\\"; break;
            case '\'': escaped += "\\'"; break;
            case '\n': escaped += "\\n"; break;
            case '\r': escaped += "\\r"; break;
            case '\0': escaped += "\\0"; break;
            default: escaped += c; break;
        }
    }
    return escaped;
}

// DomainVerifier (JSON file-based) removed — replaced by DomainPermissionCache (DB-backed)

// Forward declaration
class AsyncHTTPClient;

// UI-thread task to create a notification overlay (CreateWindowEx requires UI thread)
class CreateNotificationOverlayTask : public CefTask {
public:
    CreateNotificationOverlayTask(const std::string& type, const std::string& domain,
                                   const std::string& extraParams = "")
        : type_(type), domain_(domain), extraParams_(extraParams) {}
    void Execute() override {
        LOG_DEBUG_HTTP("🔔 CreateNotificationOverlayTask executing for " + type_ + " / " + domain_);
        g_pendingModalDomain = domain_;
#ifdef _WIN32
        extern HINSTANCE g_hInstance;
        CreateNotificationOverlay(g_hInstance, type_, domain_, extraParams_);
#elif defined(__APPLE__)
        CreateNotificationOverlay(type_, domain_, extraParams_);
#endif
    }
private:
    std::string type_;
    std::string domain_;
    std::string extraParams_;
    IMPLEMENT_REFCOUNTING(CreateNotificationOverlayTask);
    DISALLOW_COPY_AND_ASSIGN(CreateNotificationOverlayTask);
};

// Thread-safe tracker for no-wallet notifications (separate from PendingRequestManager
// to avoid stale entries blocking domain_approval after wallet creation)
class NoWalletNotificationTracker {
public:
    static NoWalletNotificationTracker& GetInstance() {
        static NoWalletNotificationTracker instance;
        return instance;
    }
    bool hasShownForDomain(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        return shownDomains_.count(domain) > 0;
    }
    void markShown(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        shownDomains_.insert(domain);
    }
    void clear() {
        std::lock_guard<std::mutex> lock(mutex_);
        shownDomains_.clear();
    }
private:
    NoWalletNotificationTracker() = default;
    std::mutex mutex_;
    std::set<std::string> shownDomains_;
};


// Async Resource Handler for managing wallet HTTP requests
class AsyncWalletResourceHandler : public CefResourceHandler {
public:
    AsyncWalletResourceHandler(const std::string& method,
                              const std::string& endpoint,
                              const std::string& body,
                              const std::string& requestDomain,
                              CefRefPtr<CefBrowser> browser,
                              const CefRequest::HeaderMap& headers)
        : method_(method), endpoint_(endpoint), body_(body), requestDomain_(requestDomain),
          responseOffset_(0), requestCompleted_(false), browser_(browser), originalHeaders_(headers) {
        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler constructor called for " + method + " " + endpoint + " from domain " + requestDomain);
        LOG_DEBUG_HTTP("🌐 Forwarding " + std::to_string(headers.size()) + " original headers");
    }

    // Declared here, implemented out-of-line after StartAsyncHTTPRequestTask is defined
    bool Open(CefRefPtr<CefRequest> request,
              bool& handle_request,
              CefRefPtr<CefCallback> callback) override;

    void GetResponseHeaders(scoped_refptr<CefResponse> response,
                           int64_t& response_length,
                           CefString& redirectUrl) override {
        CEF_REQUIRE_IO_THREAD();

        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler::GetResponseHeaders called");

        response->SetStatus(200);
        response->SetStatusText("OK");
        response->SetMimeType("application/json");
        response->SetHeaderByName("Access-Control-Allow-Origin", "*", true);
        response->SetHeaderByName("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS", true);
        response->SetHeaderByName("Access-Control-Allow-Headers", "Content-Type, Authorization", true);
        response->SetHeaderByName("Access-Control-Max-Age", "86400", true);

        // Use -1 (unknown length) when async response hasn't arrived yet.
        // Returning 0 makes CEF skip ReadResponse entirely (thinks body is empty).
        if (requestCompleted_ && !responseData_.empty()) {
            response_length = static_cast<int64_t>(responseData_.length());
        } else {
            response_length = -1;
        }
    }

    bool ReadResponse(void* data_out,
                     int bytes_to_read,
                     int& bytes_read,
                     CefRefPtr<CefCallback> callback) override {
        CEF_REQUIRE_IO_THREAD();

        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler::ReadResponse called, completed: " + std::to_string(requestCompleted_));

        if (!requestCompleted_) {
            // Store callback for later use — Continue() called when response arrives
            bytes_read = 0;
            readCallback_ = callback;
            return true; // Wait for HTTP response
        }

        // Send response data to frontend
        if (responseOffset_ >= responseData_.length()) {
            bytes_read = 0;
            return false; // No more data
        }

        int remaining = static_cast<int>(responseData_.length() - responseOffset_);
        bytes_read = (bytes_to_read < remaining) ? bytes_to_read : remaining;
        memcpy(data_out, responseData_.c_str() + responseOffset_, bytes_read);
        responseOffset_ += bytes_read;

        return true;
    }

    void Cancel() override {
        CEF_REQUIRE_IO_THREAD();
        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler::Cancel called");

        if (urlRequest_) {
            urlRequest_->Cancel();
            urlRequest_ = nullptr;
        }
    }

    // Called by AsyncHTTPClient when HTTP response is received
    void onHTTPResponseReceived(const std::string& data) {
        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler received HTTP response: " + data);

        // Atomically claim the response slot — only the first caller proceeds.
        // Prevents crash from double Continue() if timeout and real response race.
        bool expected = false;
        if (!httpCompleted_.compare_exchange_strong(expected, true)) {
            LOG_DEBUG_HTTP("🌐 Response already delivered, ignoring duplicate");
            return;
        }

        responseData_ = data;
        requestCompleted_ = true;

        LOG_DEBUG_HTTP("🌐 About to call readCallback_->Continue()");
        // Now we can continue with the response
        if (readCallback_) {
            readCallback_->Continue();
            LOG_DEBUG_HTTP("🌐 readCallback_->Continue() called successfully");
        }
    }

    // Called when user approves authentication request
    void onAuthResponseReceived(const std::string& data) {
        LOG_DEBUG_HTTP("🔐 AsyncWalletResourceHandler received auth response: " + data);

        // Atomically claim the response slot — only the first caller proceeds.
        bool expected = false;
        if (!httpCompleted_.compare_exchange_strong(expected, true)) {
            LOG_DEBUG_HTTP("🔐 Auth response already delivered, ignoring duplicate");
            return;
        }

        responseData_ = data;
        requestCompleted_ = true;

        LOG_DEBUG_HTTP("🔐 About to call readCallback_->Continue() for auth response");
        // Now we can continue with the response
        if (readCallback_) {
            readCallback_->Continue();
            LOG_DEBUG_HTTP("🔐 readCallback_->Continue() called successfully for auth response");
        }
    }

    // Timeout handling - called by WalletTimeoutTask
    void handleHttpTimeout() {
        if (httpCompleted_.load()) return;  // Already responded, skip
        LOG_DEBUG_HTTP("⏱️ Wallet HTTP request timeout - sending error");
        onHTTPResponseReceived("{\"error\":\"Wallet request timeout\",\"status\":\"error\"}");
        // Cancel the in-flight request (safe even if already completed)
        if (urlRequest_) {
            urlRequest_->Cancel();
            urlRequest_ = nullptr;
        }
    }

    void handleAuthTimeout(const std::string& errorJson) {
        if (httpCompleted_.load()) return;  // Already responded, skip
        LOG_DEBUG_HTTP("⏱️ Approval timeout - sending error");
        onAuthResponseReceived(errorJson);
    }

    // Trigger domain approval notification overlay
    void triggerDomainApprovalModal(const std::string& domain, const std::string& method, const std::string& endpoint) {
        LOG_DEBUG_HTTP("🔒 Triggering domain approval for " + domain);

        bool modalAlreadyShowing = PendingRequestManager::GetInstance().hasPendingForDomain(domain);

        // Always add the request — even if modal is already showing, we queue it
        // so ALL pending requests for this domain get resolved when user approves.
        std::string requestId = PendingRequestManager::GetInstance().addRequest(
            domain, method, endpoint, "", this);

        if (modalAlreadyShowing) {
            LOG_DEBUG_HTTP("🔒 Modal already pending for domain " + domain + ", request queued (requestId: " + requestId + ")");
            return;
        }

        // Post to UI thread — CreateWindowEx requires UI thread
        CefPostTask(TID_UI, new CreateNotificationOverlayTask("domain_approval", domain));
        LOG_DEBUG_HTTP("🔒 Domain approval needed for: " + domain + " requesting " + method + " " + endpoint);
    }


    // Trigger BRC-100 authentication approval notification overlay
    void triggerBRC100AuthApprovalModal(const std::string& domain, const std::string& method, const std::string& endpoint, const std::string& body, CefRefPtr<AsyncWalletResourceHandler> handler) {
        LOG_DEBUG_HTTP("🔐 Triggering BRC-100 auth approval for " + domain);

        bool modalAlreadyShowing = PendingRequestManager::GetInstance().hasPendingForDomain(domain);

        // Always add — duplicate requests are queued and resolved together
        std::string requestId = PendingRequestManager::GetInstance().addRequest(
            domain, method, endpoint, body, handler);

        if (modalAlreadyShowing) {
            LOG_DEBUG_HTTP("🔐 Modal already pending for domain " + domain + ", request queued (requestId: " + requestId + ")");
            return;
        }

        // Post to UI thread — CreateWindowEx requires UI thread
        CefPostTask(TID_UI, new CreateNotificationOverlayTask("domain_approval", domain));
        LOG_DEBUG_HTTP("🔐 BRC-100 auth approval needed for: " + domain + " requesting " + method + " " + endpoint);
    }


    // Trigger payment confirmation notification overlay with limit context
    void triggerPaymentConfirmationModal(const std::string& domain, int64_t satoshis, int64_t cents, double bsvPrice,
                                          const std::string& exceededLimit, int64_t perTxLimit, int64_t perSessionLimit, int64_t sessionSpent) {
        LOG_DEBUG_HTTP("💰 Triggering payment confirmation for " + domain + " (" + std::to_string(satoshis) + " sats, " + std::to_string(cents) + " cents, exceeded: " + exceededLimit + ")");

        // Store request in PendingRequestManager with type "payment_confirmation"
        std::string requestId = PendingRequestManager::GetInstance().addRequest(
            domain, method_, endpoint_, body_, this, "payment_confirmation");

        // Build extra params for overlay URL (includes limit context for frontend)
        std::string extraParams = "&satoshis=" + std::to_string(satoshis)
                                + "&cents=" + std::to_string(cents)
                                + "&bsvPrice=" + std::to_string(bsvPrice)
                                + "&exceededLimit=" + exceededLimit
                                + "&perTxLimit=" + std::to_string(perTxLimit)
                                + "&perSessionLimit=" + std::to_string(perSessionLimit)
                                + "&sessionSpent=" + std::to_string(sessionSpent);

        // Post to UI thread — CreateWindowEx requires UI thread
        CefPostTask(TID_UI, new CreateNotificationOverlayTask("payment_confirmation", domain, extraParams));
        LOG_DEBUG_HTTP("💰 Payment confirmation notification queued (requestId: " + requestId + ")");
    }

    // Check if endpoint is a payment-relevant BRC-100 endpoint
    static bool isPaymentEndpoint(const std::string& endpoint) {
        return endpoint.find("/createAction") != std::string::npos
            || endpoint.find("/acquireCertificate") != std::string::npos
            || endpoint.find("/sendMessage") != std::string::npos;
    }

    // Check if endpoint is proveCertificate (identity field disclosure)
    static bool isProveCertificateEndpoint(const std::string& endpoint) {
        return endpoint.find("/proveCertificate") != std::string::npos;
    }

    // Parse request body JSON and sum outputs[].satoshis
    static int64_t extractOutputSatoshis(const std::string& body) {
        if (body.empty()) return 0;
        try {
            auto json = nlohmann::json::parse(body);
            if (!json.contains("outputs") || !json["outputs"].is_array()) return 0;
            int64_t total = 0;
            for (const auto& output : json["outputs"]) {
                if (output.contains("satoshis") && output["satoshis"].is_number()) {
                    total += output["satoshis"].get<int64_t>();
                }
            }
            return total;
        } catch (...) {
            return 0;
        }
    }

    // Parsed certificate disclosure info from proveCertificate request body
    struct CertDisclosureInfo {
        std::string certType;
        std::string certifier;
        std::vector<std::string> fieldsToReveal;
        bool valid = false;
    };

    // Extract certificate disclosure info from proveCertificate request body
    static CertDisclosureInfo extractCertDisclosureInfo(const std::string& body) {
        CertDisclosureInfo info;
        if (body.empty()) return info;
        try {
            auto json = nlohmann::json::parse(body);

            // Extract fieldsToReveal array (top-level)
            if (json.contains("fieldsToReveal") && json["fieldsToReveal"].is_array()) {
                for (const auto& field : json["fieldsToReveal"]) {
                    if (field.is_string()) {
                        info.fieldsToReveal.push_back(field.get<std::string>());
                    }
                }
            }

            // Extract certificate.type and certificate.certifier
            if (json.contains("certificate") && json["certificate"].is_object()) {
                auto& cert = json["certificate"];
                if (cert.contains("type") && cert["type"].is_string()) {
                    info.certType = cert["type"].get<std::string>();
                }
                if (cert.contains("certifier") && cert["certifier"].is_string()) {
                    info.certifier = cert["certifier"].get<std::string>();
                }
            }

            // Also check top-level certType/certifier (alternative JSON shape)
            if (info.certType.empty() && json.contains("certType") && json["certType"].is_string()) {
                info.certType = json["certType"].get<std::string>();
            }
            if (info.certifier.empty() && json.contains("certifier") && json["certifier"].is_string()) {
                info.certifier = json["certifier"].get<std::string>();
            }

            info.valid = !info.fieldsToReveal.empty();
        } catch (...) {
            // Parse failure — return invalid info
        }
        return info;
    }

    // Trigger certificate disclosure notification overlay
    void triggerCertificateDisclosureModal(const std::string& domain, const CertDisclosureInfo& info) {
        LOG_DEBUG_HTTP("📋 Triggering certificate disclosure for " + domain + " (" + std::to_string(info.fieldsToReveal.size()) + " fields)");

        // Store request in PendingRequestManager with type "certificate_disclosure"
        std::string requestId = PendingRequestManager::GetInstance().addRequest(
            domain, method_, endpoint_, body_, this, "certificate_disclosure");

        // Build fields comma-separated list
        std::string fieldsList;
        for (size_t i = 0; i < info.fieldsToReveal.size(); ++i) {
            if (i > 0) fieldsList += ",";
            fieldsList += info.fieldsToReveal[i];
        }

        // Build extra params for overlay URL (URL-encode base64 values to preserve + chars)
        std::string extraParams = "&fields=" + fieldsList;
        if (!info.certType.empty()) {
            extraParams += "&certType=" + urlEncode(info.certType);
        }
        if (!info.certifier.empty()) {
            extraParams += "&certifier=" + urlEncode(info.certifier);
        }

        // Post to UI thread
        CefPostTask(TID_UI, new CreateNotificationOverlayTask("certificate_disclosure", domain, extraParams));
        LOG_DEBUG_HTTP("📋 Certificate disclosure notification queued (requestId: " + requestId + ", fields: " + fieldsList + ")");
    }

    // Public so handleAuthResponse() can forward queued sibling requests
    void startAsyncHTTPRequest();

    int64_t getPreCalculatedCents() const { return preCalculatedCents_; }
    bool getWasAutoApprovedPayment() const { return wasAutoApprovedPayment_; }
    int getBrowserId() const { return browser_ ? browser_->GetIdentifier() : 0; }
    const std::string& getRequestDomain() const { return requestDomain_; }

private:
    void postAuthTimeout(int delayMs, const std::string& errorJson);
    void postHttpTimeout();

    // Request data
    std::string method_;
    std::string endpoint_;
    std::string body_;
    std::string requestDomain_;
    CefRequest::HeaderMap originalHeaders_;  // BRC-31 authentication headers

    // Response management
    std::string responseData_;
    size_t responseOffset_;
    bool requestCompleted_;
    std::atomic<bool> httpCompleted_{false};

    // Auto-approve engine: pre-calculated spending for this request
    int64_t preCalculatedCents_ = 0;
    bool wasAutoApprovedPayment_ = false;

    // Browser reference for modal triggering
    CefRefPtr<CefBrowser> browser_;

    // CEF request management
    CefRefPtr<CefURLRequest> urlRequest_;
    CefRefPtr<CefCallback> readCallback_;

    IMPLEMENT_REFCOUNTING(AsyncWalletResourceHandler);
    DISALLOW_COPY_AND_ASSIGN(AsyncWalletResourceHandler);
};

// Task to defer CefURLRequest::Create to the next IO event loop iteration.
// CefURLRequest::Create blocks when called from within a CefResourceHandler::Open()
// callback on the IO thread (reentrancy issue). Deferring avoids the deadlock.
class StartAsyncHTTPRequestTask : public CefTask {
public:
    explicit StartAsyncHTTPRequestTask(CefRefPtr<AsyncWalletResourceHandler> handler)
        : handler_(handler) {}
    void Execute() override {
        handler_->startAsyncHTTPRequest();
    }
private:
    CefRefPtr<AsyncWalletResourceHandler> handler_;
    IMPLEMENT_REFCOUNTING(StartAsyncHTTPRequestTask);
    DISALLOW_COPY_AND_ASSIGN(StartAsyncHTTPRequestTask);
};

// Task to show no-wallet notification using the notification overlay system.
// Out-of-line implementation of Open() — must be after StartAsyncHTTPRequestTask
bool AsyncWalletResourceHandler::Open(CefRefPtr<CefRequest> request,
                                       bool& handle_request,
                                       CefRefPtr<CefCallback> callback) {
    CEF_REQUIRE_IO_THREAD();

    LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler::Open called");

    // Internal overlays (wallet panel, settings, etc.) are trusted — skip domain check
    if (requestDomain_.find("127.0.0.1") == 0 ||
        requestDomain_.find("localhost") == 0 ||
        requestDomain_.empty()) {
        LOG_DEBUG_HTTP("🔒 Internal origin " + requestDomain_ + " — bypassing domain check");
        handle_request = true;
        CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(this));
        return true;
    }

    // No wallet → no point showing domain approval modal; show notification instead
    if (!WalletStatusCache::GetInstance().walletExists()) {
        LOG_DEBUG_HTTP("🔒 No wallet exists — rejecting BRC-100 request from " + requestDomain_);
        onHTTPResponseReceived(
            R"({"error":"No wallet exists. Please create or recover a wallet first.","code":"NO_WALLET","status":"error"})");
        // Show once per domain per session (tracked separately from PendingRequestManager
        // so stale no_wallet entries don't block domain_approval after wallet creation)
        if (!NoWalletNotificationTracker::GetInstance().hasShownForDomain(requestDomain_)) {
            NoWalletNotificationTracker::GetInstance().markShown(requestDomain_);
            CefPostTask(TID_UI, new CreateNotificationOverlayTask("no_wallet", requestDomain_));
        }
        handle_request = true;
        return true;
    }

    // Check domain permission from DB-backed cache
    auto perm = DomainPermissionCache::GetInstance().getPermission(requestDomain_);
    LOG_DEBUG_HTTP("🔒 Domain " + requestDomain_ + " trust_level: " + perm.trustLevel);

    if (perm.trustLevel == "blocked") {
        // Silently reject blocked domains
        LOG_DEBUG_HTTP("🔒 Domain " + requestDomain_ + " is blocked, rejecting request");
        onHTTPResponseReceived("{\"error\":\"Domain blocked\",\"status\":\"error\"}");
        handle_request = true;
        return true;
    }

    if (perm.trustLevel == "unknown") {
        // Domain has no permission record — needs approval
        if (endpoint_.find("/brc100/auth/") != std::string::npos) {
            LOG_DEBUG_HTTP("🔐 BRC-100 auth request from unknown domain: " + requestDomain_);
            triggerBRC100AuthApprovalModal(requestDomain_, method_, endpoint_, body_, this);
        } else {
            LOG_DEBUG_HTTP("🔒 Domain " + requestDomain_ + " unknown, triggering approval modal");
            triggerDomainApprovalModal(requestDomain_, method_, endpoint_);
        }

        postAuthTimeout(60000, "{\"error\":\"Approval timeout\",\"status\":\"error\"}");
        handle_request = true;
        return true;
    }

    // "approved" — auto-approve engine with spending limits and rate limiting
    if (perm.trustLevel == "approved") {
        LOG_DEBUG_HTTP("🔒 Domain " + requestDomain_ + " is approved, checking spending limits");

        // Certificate disclosure check — intercept proveCertificate BEFORE the payment check
        if (isProveCertificateEndpoint(endpoint_)) {
            auto certInfo = extractCertDisclosureInfo(body_);
            if (certInfo.valid && !certInfo.certType.empty()) {
                LOG_DEBUG_HTTP("📋 proveCertificate from " + requestDomain_ + " — checking field permissions");

                // Fetch already-approved fields from DB
                auto approvedFields = fetchCertFieldsFromBackend(requestDomain_, certInfo.certType);

                // Check if ALL requested fields are approved
                bool allApproved = true;
                for (const auto& field : certInfo.fieldsToReveal) {
                    if (approvedFields.find(field) == approvedFields.end()) {
                        allApproved = false;
                        break;
                    }
                }

                if (allApproved) {
                    LOG_DEBUG_HTTP("📋 All cert fields already approved for " + requestDomain_ + ", auto-forwarding");
                    handle_request = true;
                    CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(this));
                    return true;
                } else {
                    LOG_DEBUG_HTTP("📋 Unapproved cert fields — showing disclosure notification for " + requestDomain_);
                    triggerCertificateDisclosureModal(requestDomain_, certInfo);
                    postAuthTimeout(60000, "{\"error\":\"Certificate disclosure timeout\",\"status\":\"error\"}");
                    handle_request = true;
                    return true;
                }
            }
            // Invalid or empty fields — fall through to normal forwarding
            LOG_DEBUG_HTTP("📋 proveCertificate with no/empty fields — forwarding directly");
        }

        if (isPaymentEndpoint(endpoint_)) {
            int browserId = browser_ ? browser_->GetIdentifier() : 0;

            // Ensure session exists for rate limiting and spending tracking
            SessionManager::GetInstance().getSession(browserId, requestDomain_);

            // Parse outputs and calculate USD cents (needed for all paths)
            int64_t satoshis = extractOutputSatoshis(body_);
            double bsvPrice = BSVPriceCache::GetInstance().getPrice();
            int64_t cents = 0;
            if (bsvPrice > 0 && satoshis > 0) {
                // satoshis → USD: (satoshis / 100_000_000) * bsvPrice * 100 (for cents)
                cents = static_cast<int64_t>((static_cast<double>(satoshis) / 100000000.0) * bsvPrice * 100.0);
            }

            // Safety: if price is unknown (never fetched or all fetches failed), require user confirmation
            if (bsvPrice <= 0 && satoshis > 0) {
                LOG_DEBUG_HTTP("⚠️ BSV price unavailable — requiring user confirmation for " + requestDomain_);
                preCalculatedCents_ = 0;

                std::string requestId = PendingRequestManager::GetInstance().addRequest(
                    requestDomain_, method_, endpoint_, body_, this, "payment_confirmation");

                std::string extraParams = "&satoshis=" + std::to_string(satoshis)
                                        + "&cents=0"
                                        + "&bsvPrice=0"
                                        + "&exceededLimit=price_unavailable"
                                        + "&perTxLimit=" + std::to_string(perm.perTxLimitCents)
                                        + "&perSessionLimit=" + std::to_string(perm.perSessionLimitCents)
                                        + "&sessionSpent=0";

                CefPostTask(TID_UI, new CreateNotificationOverlayTask("payment_confirmation", requestDomain_, extraParams));
                postAuthTimeout(60000, "{\"error\":\"Payment approval timeout\",\"status\":\"error\"}");
                handle_request = true;
                return true;
            }

            // Rate limit check — show notification instead of auto-rejecting
            if (!SessionManager::GetInstance().checkRateLimit(browserId, perm.rateLimitPerMin)) {
                LOG_DEBUG_HTTP("🔒 Rate limit exceeded for " + requestDomain_ + ", showing notification");
                preCalculatedCents_ = cents;

                std::string requestId = PendingRequestManager::GetInstance().addRequest(
                    requestDomain_, method_, endpoint_, body_, this, "rate_limit_exceeded");

                std::string extraParams = "&satoshis=" + std::to_string(satoshis)
                                        + "&cents=" + std::to_string(cents)
                                        + "&bsvPrice=" + std::to_string(bsvPrice)
                                        + "&rateLimit=" + std::to_string(perm.rateLimitPerMin)
                                        + "&perTxLimit=" + std::to_string(perm.perTxLimitCents)
                                        + "&perSessionLimit=" + std::to_string(perm.perSessionLimitCents);

                CefPostTask(TID_UI, new CreateNotificationOverlayTask("rate_limit_exceeded", requestDomain_, extraParams));
                postAuthTimeout(60000, "{\"error\":\"Rate limit approval timeout\",\"status\":\"error\"}");
                handle_request = true;
                return true;
            }

            // Session transaction count check — require confirmation if max tx count reached
            int txCount = SessionManager::GetInstance().getPaymentCount(browserId, requestDomain_);
            if (txCount >= perm.maxTxPerSession) {
                LOG_DEBUG_HTTP("🔒 Session transaction count exceeded for " + requestDomain_ + " (" + std::to_string(txCount) + " >= " + std::to_string(perm.maxTxPerSession) + "), showing notification");
                preCalculatedCents_ = cents;

                std::string requestId = PendingRequestManager::GetInstance().addRequest(
                    requestDomain_, method_, endpoint_, body_, this, "rate_limit_exceeded");

                std::string extraParams = "&satoshis=" + std::to_string(satoshis)
                                        + "&cents=" + std::to_string(cents)
                                        + "&bsvPrice=" + std::to_string(bsvPrice)
                                        + "&rateLimit=" + std::to_string(perm.rateLimitPerMin)
                                        + "&perTxLimit=" + std::to_string(perm.perTxLimitCents)
                                        + "&perSessionLimit=" + std::to_string(perm.perSessionLimitCents)
                                        + "&exceededLimit=session_tx_count"
                                        + "&maxTxPerSession=" + std::to_string(perm.maxTxPerSession)
                                        + "&txCount=" + std::to_string(txCount);

                CefPostTask(TID_UI, new CreateNotificationOverlayTask("rate_limit_exceeded", requestDomain_, extraParams));
                postAuthTimeout(60000, "{\"error\":\"Session transaction count approval timeout\",\"status\":\"error\"}");
                handle_request = true;
                return true;
            }

            // Check per-tx limit
            bool withinTxLimit = (cents <= perm.perTxLimitCents);

            // Check per-session cumulative limit
            int64_t sessionSpent = SessionManager::GetInstance().getSpentCents(browserId, requestDomain_);
            bool withinSessionLimit = ((sessionSpent + cents) <= perm.perSessionLimitCents);

            LOG_DEBUG_HTTP("💰 Payment: " + std::to_string(satoshis) + " sats = " + std::to_string(cents) + " cents"
                + " | tx_limit=" + std::to_string(perm.perTxLimitCents)
                + " session_spent=" + std::to_string(sessionSpent)
                + " session_limit=" + std::to_string(perm.perSessionLimitCents)
                + " tx_count=" + std::to_string(txCount)
                + " max_tx_per_session=" + std::to_string(perm.maxTxPerSession));

            if (withinTxLimit && withinSessionLimit) {
                // Auto-approve: within both limits
                LOG_DEBUG_HTTP("💰 Auto-approved payment for " + requestDomain_);
                preCalculatedCents_ = cents;
                wasAutoApprovedPayment_ = true;
                SessionManager::GetInstance().incrementRateCounter(browserId);
                SessionManager::GetInstance().incrementPaymentCount(browserId);
                handle_request = true;
                CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(this));
                return true;
            } else {
                // Over limit — show payment confirmation with limit context
                LOG_DEBUG_HTTP("💰 Over limit — showing payment confirmation for " + requestDomain_);
                preCalculatedCents_ = cents;

                std::string exceeded = "";
                if (!withinTxLimit && !withinSessionLimit) exceeded = "both";
                else if (!withinTxLimit) exceeded = "per_tx";
                else exceeded = "per_session";

                triggerPaymentConfirmationModal(requestDomain_, satoshis, cents, bsvPrice,
                    exceeded, perm.perTxLimitCents, perm.perSessionLimitCents, sessionSpent);
                postAuthTimeout(60000, "{\"error\":\"Payment confirmation timeout\",\"status\":\"error\"}");
                handle_request = true;
                return true;
            }
        }

        // Non-payment endpoint from approved domain — forward immediately
        LOG_DEBUG_HTTP("🔒 Non-payment endpoint from approved domain " + requestDomain_ + ", forwarding");
        handle_request = true;
        CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(this));
        return true;
    }

    // Fallback: unknown trust level — show domain approval
    LOG_DEBUG_HTTP("🔒 Domain " + requestDomain_ + " has unrecognized trust level: " + perm.trustLevel + ", triggering approval");
    triggerDomainApprovalModal(requestDomain_, method_, endpoint_);
    postAuthTimeout(60000, "{\"error\":\"Approval timeout\",\"status\":\"error\"}");
    handle_request = true;
    return true;
}

// Timeout task for delayed execution on UI thread
class WalletTimeoutTask : public CefTask {
public:
    enum Type { HTTP_TIMEOUT, AUTH_TIMEOUT };

    WalletTimeoutTask(CefRefPtr<AsyncWalletResourceHandler> handler, Type type,
                      const std::string& errorJson)
        : handler_(handler), type_(type), errorJson_(errorJson) {}

    void Execute() override {
        if (type_ == HTTP_TIMEOUT) {
            handler_->handleHttpTimeout();
        } else {
            handler_->handleAuthTimeout(errorJson_);
        }
    }

private:
    CefRefPtr<AsyncWalletResourceHandler> handler_;
    Type type_;
    std::string errorJson_;

    IMPLEMENT_REFCOUNTING(WalletTimeoutTask);
    DISALLOW_COPY_AND_ASSIGN(WalletTimeoutTask);
};

void AsyncWalletResourceHandler::postAuthTimeout(int delayMs, const std::string& errorJson) {
    CefPostDelayedTask(TID_UI, new WalletTimeoutTask(this, WalletTimeoutTask::AUTH_TIMEOUT, errorJson), delayMs);
}

void AsyncWalletResourceHandler::postHttpTimeout() {
    // Recovery scans can take 60+ seconds; use 120s timeout for recover endpoints
    int timeoutMs = 45000;
    if (endpoint_.find("/wallet/recover") != std::string::npos) {
        timeoutMs = 120000;
    }
    CefPostDelayedTask(TID_UI, new WalletTimeoutTask(this, WalletTimeoutTask::HTTP_TIMEOUT, ""), timeoutMs);
}

// Function to store pending auth request data (called from overlay_show_brc100_auth IPC — no handler)
void storePendingAuthRequest(const std::string& domain, const std::string& method, const std::string& endpoint, const std::string& body, const std::string& type) {
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        domain, method, endpoint, body, nullptr, type);
    g_pendingModalDomain = domain;
    LOG_DEBUG_HTTP("🔐 Stored pending auth request data (requestId: " + requestId + ", type: " + type + ")");
}

// Handler for domain permission requests
class AsyncDomainPermissionHandler : public CefURLRequestClient {
public:
    explicit AsyncDomainPermissionHandler(const std::string& domain)
        : domain_(domain) {}

    void OnRequestComplete(CefRefPtr<CefURLRequest> request) override {
        LOG_DEBUG_HTTP("🔐 AsyncDomainPermissionHandler::OnRequestComplete called for domain: " + domain_);
        CefURLRequest::Status status = request->GetRequestStatus();
        LOG_DEBUG_HTTP("🔐 Request status: " + std::to_string(status));
        if (status == UR_SUCCESS) {
            LOG_DEBUG_HTTP("🔐 Successfully set domain permission: " + domain_);
        } else {
            LOG_DEBUG_HTTP("🔐 Failed to set domain permission: " + domain_ + " (status: " + std::to_string(status) + ")");
        }
    }

    void OnDownloadData(CefRefPtr<CefURLRequest> request, const void* data, size_t data_length) override {
        // Handle response data if needed
    }

    void OnUploadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {
        // Not needed for this use case
    }

    void OnDownloadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {
        // Not needed for this use case
    }

    bool GetAuthCredentials(bool isProxy, const CefString& host, int port, const CefString& realm, const CefString& scheme, CefRefPtr<CefAuthCallback> callback) override {
        // No authentication needed
        return false;
    }

private:
    std::string domain_;
    IMPLEMENT_REFCOUNTING(AsyncDomainPermissionHandler);
    DISALLOW_COPY_AND_ASSIGN(AsyncDomainPermissionHandler);
};

// Task class for creating domain permission request on UI thread
class DomainPermissionTask : public CefTask {
public:
    explicit DomainPermissionTask(const std::string& domain)
        : domain_(domain) {}

    void Execute() override {
        LOG_DEBUG_HTTP("🔐 DomainPermissionTask executing on UI thread for domain: " + domain_);

        // Create request
        CefRefPtr<CefRequest> cefRequest = CefRequest::Create();
        cefRequest->SetURL("http://localhost:31301/domain/permissions");
        cefRequest->SetMethod("POST");
        cefRequest->SetHeaderByName("Content-Type", "application/json", true);

        // Create JSON body
        std::string jsonBody = "{\"domain\":\"" + domain_ + "\",\"trustLevel\":\"approved\"}";
        LOG_DEBUG_HTTP("🔐 Domain permission JSON body: " + jsonBody);

        // Create post data
        CefRefPtr<CefPostData> postData = CefPostData::Create();
        CefRefPtr<CefPostDataElement> element = CefPostDataElement::Create();
        element->SetToBytes(jsonBody.length(), jsonBody.c_str());
        postData->AddElement(element);
        cefRequest->SetPostData(postData);

        LOG_DEBUG_HTTP("🔐 About to create CefURLRequest for domain permission");
        // Make HTTP request to set domain permission
        CefRefPtr<CefURLRequest> request = CefURLRequest::Create(
            cefRequest,
            new AsyncDomainPermissionHandler(domain_),
            nullptr
        );

        if (request) {
            LOG_DEBUG_HTTP("🔐 Domain permission request created successfully");
        } else {
            LOG_DEBUG_HTTP("🔐 Failed to create domain permission request");
        }
    }

private:
    std::string domain_;
    IMPLEMENT_REFCOUNTING(DomainPermissionTask);
    DISALLOW_COPY_AND_ASSIGN(DomainPermissionTask);
};

// Task class for creating domain permission with advanced settings (custom limits)
class AdvancedDomainPermissionTask : public CefTask {
public:
    AdvancedDomainPermissionTask(const std::string& domain, int64_t perTxLimitCents,
                                  int64_t perSessionLimitCents, int64_t rateLimitPerMin,
                                  int64_t maxTxPerSession)
        : domain_(domain), perTxLimitCents_(perTxLimitCents),
          perSessionLimitCents_(perSessionLimitCents), rateLimitPerMin_(rateLimitPerMin),
          maxTxPerSession_(maxTxPerSession) {}

    void Execute() override {
        LOG_DEBUG_HTTP("🔐 AdvancedDomainPermissionTask executing for domain: " + domain_);

        CefRefPtr<CefRequest> cefRequest = CefRequest::Create();
        cefRequest->SetURL("http://localhost:31301/domain/permissions");
        cefRequest->SetMethod("POST");
        cefRequest->SetHeaderByName("Content-Type", "application/json", true);

        nlohmann::json body;
        body["domain"] = domain_;
        body["trustLevel"] = "approved";
        body["perTxLimitCents"] = perTxLimitCents_;
        body["perSessionLimitCents"] = perSessionLimitCents_;
        body["rateLimitPerMin"] = rateLimitPerMin_;
        body["maxTxPerSession"] = maxTxPerSession_;
        std::string jsonBody = body.dump();

        CefRefPtr<CefPostData> postData = CefPostData::Create();
        CefRefPtr<CefPostDataElement> element = CefPostDataElement::Create();
        element->SetToBytes(jsonBody.length(), jsonBody.c_str());
        postData->AddElement(element);
        cefRequest->SetPostData(postData);

        CefRefPtr<CefURLRequest> request = CefURLRequest::Create(
            cefRequest, new AsyncDomainPermissionHandler(domain_), nullptr);

        if (request) {
            LOG_DEBUG_HTTP("🔐 Advanced domain permission request created for " + domain_);
        }
    }

private:
    std::string domain_;
    int64_t perTxLimitCents_;
    int64_t perSessionLimitCents_;
    int64_t rateLimitPerMin_;
    int64_t maxTxPerSession_;
    IMPLEMENT_REFCOUNTING(AdvancedDomainPermissionTask);
    DISALLOW_COPY_AND_ASSIGN(AdvancedDomainPermissionTask);
};

// Function to add domain permission with advanced settings
void addDomainPermissionAdvanced(const std::string& domain, int64_t perTxLimitCents,
                                  int64_t perSessionLimitCents, int64_t rateLimitPerMin,
                                  int64_t maxTxPerSession) {
    LOG_DEBUG_HTTP("🔐 Adding advanced domain permission: " + domain +
        " (tx=" + std::to_string(perTxLimitCents) + ", session=" + std::to_string(perSessionLimitCents) +
        ", rate=" + std::to_string(rateLimitPerMin) + ", maxTxPerSession=" + std::to_string(maxTxPerSession) + ")");

    // Set cache immediately with full settings
    DomainPermissionCache::Permission perm;
    perm.trustLevel = "approved";
    perm.perTxLimitCents = perTxLimitCents;
    perm.perSessionLimitCents = perSessionLimitCents;
    perm.rateLimitPerMin = rateLimitPerMin;
    perm.maxTxPerSession = maxTxPerSession;
    DomainPermissionCache::GetInstance().set(domain, perm);

    // Post async DB write
    CefPostTask(TID_UI, new AdvancedDomainPermissionTask(domain, perTxLimitCents, perSessionLimitCents, rateLimitPerMin, maxTxPerSession));
}

// Function to add domain permission (sets "approved" trust level)
void addDomainPermission(const std::string& domain) {
    LOG_DEBUG_HTTP("🔐 Adding domain permission: " + domain);

    // Set the cache immediately so the next request sees "approved" without waiting
    // for the async DB write to complete (prevents modal loop race condition)
    DomainPermissionCache::Permission perm;
    perm.trustLevel = "approved";
    DomainPermissionCache::GetInstance().set(domain, perm);

    // Post task to UI thread for the async DB write
    CefPostTask(TID_UI, new DomainPermissionTask(domain));
    LOG_DEBUG_HTTP("🔐 Domain permission task posted to UI thread");
}

// Invalidate a single domain in the permission cache (called from simple_handler.cpp IPC)
void invalidateDomainPermissionCache(const std::string& domain) {
    DomainPermissionCache::GetInstance().invalidate(domain);
}

// Clear entire domain permission cache (called from simple_handler.cpp IPC)
void clearDomainPermissionCache() {
    DomainPermissionCache::GetInstance().clear();
}

// Cache-warming helpers (called from simple_handler.cpp startup / navigation)
void warmWalletStatusCache() {
    WalletStatusCache::GetInstance().walletExists();
}

void warmBSVPriceCache() {
    BSVPriceCache::GetInstance().getPrice();
}

void warmDomainPermissionCache(const std::string& domain) {
    DomainPermissionCache::GetInstance().getPermission(domain);
}

// Function to handle auth response and send it back to the original request
void handleAuthResponse(const std::string& requestId, const std::string& responseData) {
    LOG_DEBUG_HTTP("🔐 handleAuthResponse called for requestId: " + requestId);

    std::string domain;

    // 1. Resolve the primary request (the one whose response we have)
    PendingAuthRequest req;
    if (PendingRequestManager::GetInstance().popRequest(requestId, req)) {
        domain = req.domain;
        if (req.handler) {
            LOG_DEBUG_HTTP("🔐 Found pending auth request for domain: " + req.domain);
            AsyncWalletResourceHandler* walletHandler = static_cast<AsyncWalletResourceHandler*>(req.handler.get());
            if (walletHandler) {
                // Record spending for user-approved payment confirmations
                int64_t cents = walletHandler->getPreCalculatedCents();
                if (cents > 0) {
                    bool respIsError = false;
                    try {
                        auto rj = nlohmann::json::parse(responseData);
                        respIsError = rj.contains("error");
                    } catch (...) {}
                    if (!respIsError) {
                        int browserId = walletHandler->getBrowserId();
                        SessionManager::GetInstance().recordSpending(browserId, cents);
                        LOG_DEBUG_HTTP("💰 Recorded user-approved spending: " + std::to_string(cents) + " cents for browser " + std::to_string(browserId));
                    }
                }
                walletHandler->onAuthResponseReceived(responseData);
                LOG_DEBUG_HTTP("🔐 Auth response sent to original HTTP request");
            }
        } else {
            LOG_DEBUG_HTTP("🔐 Pending request had no handler (overlay-initiated flow)");
        }
    } else {
        LOG_DEBUG_HTTP("🔐 No pending auth request found for requestId: " + requestId);
    }

    // Detect whether this is a rejection (error response) or an approval
    bool isRejection = false;
    try {
        auto parsed = nlohmann::json::parse(responseData);
        isRejection = parsed.contains("error");
    } catch (...) {}

    // On rejection of a DOMAIN APPROVAL: block domain in-memory for the rest of this session.
    // Future requests from this domain will be silently rejected (no repeat modals).
    // Clears on browser restart — domain returns to "unknown".
    // Payment/rate-limit denials are one-time — domain stays "approved", same checks apply next request.
    bool isDomainApproval = (req.type == "domain_approval" || req.type == "brc100_auth");
    if (isRejection && !domain.empty() && isDomainApproval) {
        DomainPermissionCache::Permission blockedPerm;
        blockedPerm.trustLevel = "blocked";
        DomainPermissionCache::GetInstance().set(domain, blockedPerm);
        LOG_DEBUG_HTTP("🔐 Domain " + domain + " blocked in-memory for this session");
    }

    // 2. Resolve ALL remaining queued requests for this domain.
    // These are requests that arrived while the modal was showing.
    if (!domain.empty()) {
        auto siblings = PendingRequestManager::GetInstance().popAllForDomain(domain);
        if (!siblings.empty()) {
            LOG_DEBUG_HTTP("🔐 Resolving " + std::to_string(siblings.size()) + " queued request(s) for domain: " + domain +
                           (isRejection ? " (rejected)" : " (approved)"));
        }
        for (auto& sibling : siblings) {
            if (sibling.handler) {
                AsyncWalletResourceHandler* walletHandler = static_cast<AsyncWalletResourceHandler*>(sibling.handler.get());
                if (walletHandler) {
                    if (isRejection) {
                        // Rejection: send error to all queued siblings (don't forward to Rust)
                        walletHandler->onAuthResponseReceived(responseData);
                        LOG_DEBUG_HTTP("🔐 Sent rejection to queued request " + sibling.requestId + " for " + sibling.endpoint);
                    } else {
                        // Approval: forward siblings to Rust backend
                        CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(walletHandler));
                        LOG_DEBUG_HTTP("🔐 Forwarded queued request " + sibling.requestId + " for " + sibling.endpoint);
                    }
                }
            }
        }
    }

    g_pendingModalDomain = "";
}

// Legacy overload — resolves the requestId from the domain (backward compat for overlay_show_brc100_auth path)
void handleAuthResponse(const std::string& responseData) {
    std::string requestId = PendingRequestManager::GetInstance().getRequestIdForDomain(g_pendingModalDomain);
    if (requestId.empty()) {
        LOG_DEBUG_HTTP("🔐 handleAuthResponse (legacy): no pending request found for domain: " + g_pendingModalDomain);
        g_pendingModalDomain = "";
        return;
    }
    handleAuthResponse(requestId, responseData);
}

// Function to send auth request data to overlay (called after overlay loads)
void sendAuthRequestDataToOverlay() {
    // Find the pending request for the current modal domain
    std::string requestId = PendingRequestManager::GetInstance().getRequestIdForDomain(g_pendingModalDomain);
    if (requestId.empty()) {
        LOG_DEBUG_HTTP("🔐 No pending auth request data to send");
        return;
    }

    PendingAuthRequest req;
    if (!PendingRequestManager::GetInstance().getRequest(requestId, req)) {
        LOG_DEBUG_HTTP("🔐 Failed to get pending request for requestId: " + requestId);
        return;
    }

    CefRefPtr<CefBrowser> auth_browser = SimpleHandler::GetBRC100AuthBrowser();
    if (auth_browser && auth_browser->GetMainFrame()) {
        CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("brc100_auth_request");
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        args->SetString(0, req.domain);
        args->SetString(1, req.method);
        args->SetString(2, req.endpoint);
        args->SetString(3, req.body);
        args->SetString(4, req.requestId);
        args->SetString(5, req.type);

        auth_browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, message);
        LOG_DEBUG_HTTP("🔐 Sent auth request data to overlay (requestId: " + requestId + ", type: " + req.type + ")");
    } else {
        LOG_DEBUG_HTTP("🔐 Auth browser not available for sending data");
    }
}

// Async HTTP Client for handling CEF URL requests
class AsyncHTTPClient : public CefURLRequestClient {
public:
    explicit AsyncHTTPClient(CefRefPtr<AsyncWalletResourceHandler> parent)
        : parent_(parent), completed_(false) {
        LOG_DEBUG_HTTP("🌐 AsyncHTTPClient constructor called");
    }

    void OnRequestComplete(CefRefPtr<CefURLRequest> request) override {
        std::lock_guard<std::mutex> lock(mutex_);
        completed_ = true;

        LOG_DEBUG_HTTP("🌐 AsyncHTTPClient::OnRequestComplete called, response size: " + std::to_string(responseData_.length()));

        // Record spending on successful auto-approve payment
        if (parent_) {
            CefURLRequest::Status status = request->GetRequestStatus();
            int64_t cents = parent_->getPreCalculatedCents();
            if (status == UR_SUCCESS && parent_->getWasAutoApprovedPayment()) {
                // Check response is not an error
                bool isError = false;
                try {
                    auto rj = nlohmann::json::parse(responseData_);
                    isError = rj.contains("error");
                } catch (...) {}

                if (!isError) {
                    int browserId = parent_->getBrowserId();
                    SessionManager::GetInstance().recordSpending(browserId, cents);
                    LOG_DEBUG_HTTP("💰 Recorded spending: " + std::to_string(cents) + " cents for browser " + std::to_string(browserId));

                    // Notify header browser for tab payment badge animation
                    CefRefPtr<CefBrowser> headerBrowser = SimpleHandler::GetHeaderBrowser();
                    if (headerBrowser && headerBrowser->GetMainFrame()) {
                        nlohmann::json payload;
                        payload["browserId"] = browserId;
                        payload["domain"] = parent_->getRequestDomain();
                        payload["cents"] = cents;

                        CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("payment_success_indicator");
                        msg->GetArgumentList()->SetString(0, payload.dump());
                        headerBrowser->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
                        LOG_DEBUG_HTTP("💰 Sent payment indicator to header: " + std::to_string(cents) + " cents from " + parent_->getRequestDomain());
                    }
                }
            }
            parent_->onHTTPResponseReceived(responseData_);
        }
    }

    void OnUploadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {
        // Not needed for our use case
    }

    void OnDownloadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {
        // Not needed for our use case
    }

    void OnDownloadData(CefRefPtr<CefURLRequest> request, const void* data, size_t data_length) override {
        std::lock_guard<std::mutex> lock(mutex_);
        responseData_.append(static_cast<const char*>(data), data_length);
        LOG_DEBUG_HTTP("🌐 AsyncHTTPClient::OnDownloadData received " + std::to_string(data_length) + " bytes");
    }

    bool GetAuthCredentials(bool isProxy,
                           const CefString& host,
                           int port,
                           const CefString& realm,
                           const CefString& scheme,
                           CefRefPtr<CefAuthCallback> callback) override {
        return false; // No authentication needed
    }

private:
    CefRefPtr<AsyncWalletResourceHandler> parent_;
    std::mutex mutex_;
    bool completed_;
    std::string responseData_;

    IMPLEMENT_REFCOUNTING(AsyncHTTPClient);
    DISALLOW_COPY_AND_ASSIGN(AsyncHTTPClient);
};

// Implementation of AsyncWalletResourceHandler::startAsyncHTTPRequest
void AsyncWalletResourceHandler::startAsyncHTTPRequest() {
    LOG_DEBUG_HTTP("🌐 Starting async HTTP request to: " + endpoint_);

    // Create CEF HTTP request
    CefRefPtr<CefRequest> httpRequest = CefRequest::Create();
    std::string fullUrl = "http://localhost:31301" + endpoint_;
    httpRequest->SetURL(fullUrl);
    httpRequest->SetMethod(method_);

    // Start with standard headers
    CefRequest::HeaderMap headers;
    headers.insert(std::make_pair("Content-Type", "application/json"));
    headers.insert(std::make_pair("Accept", "application/json"));

    // Pass the requesting domain to Rust for defense-in-depth permission checks
    if (!requestDomain_.empty()) {
        headers.insert(std::make_pair("X-Requesting-Domain", requestDomain_));
    }

    // Forward original headers (including BRC-31 Authrite headers)
    for (const auto& header : originalHeaders_) {
        std::string headerName = header.first.ToString();
        std::string headerValue = header.second.ToString();

        if (headerName.find("x-authrite-") != std::string::npos ||
            headerName.find("X-Authrite-") != std::string::npos ||
            headerName.find("x-bsv-") != std::string::npos ||
            headerName.find("X-BSV-") != std::string::npos) {
            LOG_DEBUG_HTTP("🔐 Forwarding auth header: " + headerName + " = " + headerValue.substr(0, 50) + "...");
        }

        headers.insert(std::make_pair(headerName, headerValue));
    }

    httpRequest->SetHeaderMap(headers);

    // Set POST body — always include at least one PostData element for POST requests.
    // CefURLRequest with zero-element PostData may not fire OnDownloadData callbacks.
    if (method_ == "POST") {
        CefRefPtr<CefPostData> postData = CefPostData::Create();
        CefRefPtr<CefPostDataElement> element = CefPostDataElement::Create();
        if (!body_.empty()) {
            element->SetToBytes(body_.length(), body_.c_str());
        } else {
            element->SetToBytes(0, "");
        }
        postData->AddElement(element);
        httpRequest->SetPostData(postData);
    }

    CefRefPtr<AsyncHTTPClient> client = new AsyncHTTPClient(this);
    CefRefPtr<CefRequestContext> context = CefRequestContext::GetGlobalContext();

    LOG_DEBUG_HTTP("🌐 Creating CefURLRequest for " + method_ + " " + fullUrl);

    try {
        // CefURLRequest::Create works on any valid CEF thread including IO
        urlRequest_ = CefURLRequest::Create(httpRequest, client, context);
        LOG_DEBUG_HTTP("🌐 CefURLRequest created successfully");

        // Timeout: cancel request after 45s if no response
        postHttpTimeout();

    } catch (const std::exception& e) {
        LOG_DEBUG_HTTP("🌐 Exception creating CefURLRequest: " + std::string(e.what()));
    } catch (...) {
        LOG_DEBUG_HTTP("🌐 Unknown exception creating CefURLRequest");
    }
}

HttpRequestInterceptor::HttpRequestInterceptor() {
    LOG_DEBUG_HTTP("🌐 HttpRequestInterceptor created");
}

HttpRequestInterceptor::~HttpRequestInterceptor() {
    LOG_DEBUG_HTTP("🌐 HttpRequestInterceptor destroyed");
}

CefRefPtr<CefResourceHandler> HttpRequestInterceptor::GetResourceHandler(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefRefPtr<CefRequest> request) {

    CEF_REQUIRE_IO_THREAD();

    std::string url = request->GetURL().ToString();
    std::string method = request->GetMethod().ToString();

    LOG_DEBUG_HTTP("🌐 HTTP Request intercepted: " + method + " " + url);

    // Normalize BRC-100 wallet requests to our standard port 31301
    std::string originalUrl = url;

    // Handle localhost/127.0.0.1 port redirection (string ops instead of regex — F5 perf fix)
    auto redirectPort = [&](const std::string& host, const std::string& target) {
        size_t pos = url.find(host);
        if (pos == std::string::npos) return;
        if (url.find(target) != std::string::npos) return;  // Already correct port
        // Find the 4-digit port after the host prefix
        size_t portStart = pos + host.length();
        size_t portEnd = portStart;
        while (portEnd < url.length() && url[portEnd] >= '0' && url[portEnd] <= '9') portEnd++;
        if (portEnd > portStart && (portEnd - portStart) <= 5) {
            url.replace(pos, portEnd - pos, target);
            LOG_DEBUG_HTTP("🌐 Port redirection: " + originalUrl + " -> " + url);
            request->SetURL(url);
        }
    };
    redirectPort("localhost:", "localhost:31301");
    redirectPort("127.0.0.1:", "127.0.0.1:31301");

    LOG_DEBUG_HTTP("🌐 About to check if wallet endpoint...");

    // BRC-104 authentication endpoint interception
    // ONLY redirect /.well-known/auth if it's meant for the LOCAL WALLET (localhost/127.0.0.1)
    // DO NOT redirect auth requests to external app backends!
    if (url.find("/.well-known/auth") != std::string::npos) {
        // Check if this is a request to localhost or 127.0.0.1 (wallet auth)
        bool isLocalhost = (url.find("localhost") != std::string::npos || url.find("127.0.0.1") != std::string::npos);

        if (isLocalhost) {
            LOG_DEBUG_HTTP("🌐 BRC-104 /.well-known/auth request to localhost detected, redirecting to local wallet");

            // Extract the original domain for logging
            std::string originalDomain = url;
            size_t protocolEnd = originalDomain.find("://");
            if (protocolEnd != std::string::npos) {
                originalDomain = originalDomain.substr(protocolEnd + 3);
                size_t pathStart = originalDomain.find("/");
                if (pathStart != std::string::npos) {
                    originalDomain = originalDomain.substr(0, pathStart);
                }
            }

            // Replace the domain with localhost:31301 (string ops instead of regex — F5 perf fix)
            size_t schemeEnd = url.find("://");
            if (schemeEnd != std::string::npos) {
                size_t hostEnd = url.find('/', schemeEnd + 3);
                if (hostEnd != std::string::npos) {
                    url = "http://localhost:31301" + url.substr(hostEnd);
                } else {
                    url = "http://localhost:31301";
                }
            }

            LOG_DEBUG_HTTP("🌐 BRC-104 auth redirection: " + originalUrl + " -> " + url);
            request->SetURL(url);
        } else {
            LOG_DEBUG_HTTP("🌐 BRC-104 /.well-known/auth request to external backend detected: " + url);
            LOG_DEBUG_HTTP("🌐 NOT intercepting - allowing CEF to handle normally");
            return nullptr; // Let CEF handle external backend auth requests normally
        }
    }

    // Check if this is a Babbage messagebox request that needs redirection
    if (url.find("messagebox.babbage.systems") != std::string::npos) {
        LOG_DEBUG_HTTP("🌐 ===== MESSAGEBOX REQUEST DETECTED =====");
        LOG_DEBUG_HTTP("🌐 Method: " + method);
        LOG_DEBUG_HTTP("🌐 Full URL: " + url);

        // Let ALL messagebox.babbage.systems requests pass through to the real server
        // Messages (containing BEEF + paymentRemittance) are stored on Babbage's infrastructure.
        // The flow is:
        // 1. Sender calls sendMessage to messagebox.babbage.systems - message stored there
        // 2. Recipient calls listMessages to messagebox.babbage.systems - gets message
        // 3. App calls internalizeAction on LOCAL wallet - we store the UTXO
        // We don't intercept messagebox - we only handle wallet-specific BRC-100 endpoints.
        LOG_DEBUG_HTTP("🌐 Messagebox request - passing through to real Babbage server");
        LOG_DEBUG_HTTP("🌐 (Messages are stored on Babbage infrastructure, not locally)");
        return nullptr; // Let CEF handle it normally - goes to real Babbage server
    }

    // Check if this is a Socket.IO connection first
    if (isSocketIOConnection(url)) {
        LOG_DEBUG_HTTP("🌐 Socket.IO connection detected");

        // Extract domain using existing logic
        std::string domain = extractDomain(browser, request);
        LOG_DEBUG_HTTP("🌐 Extracted domain for Socket.IO: " + domain);

        // Check domain permission (for logging only - no modal for Socket.IO)
        auto socketPerm = DomainPermissionCache::GetInstance().getPermission(domain);
        if (socketPerm.trustLevel == "unknown") {
            LOG_DEBUG_HTTP("🔒 Socket.IO connection from unknown domain: " + domain + " - allowing for now");
        } else {
            LOG_DEBUG_HTTP("🔒 Socket.IO connection from " + socketPerm.trustLevel + " domain: " + domain);
        }

        // Create AsyncWalletResourceHandler for Socket.IO requests
        LOG_DEBUG_HTTP("🌐 Creating AsyncWalletResourceHandler for Socket.IO request");

        // Extract endpoint from URL
        std::string endpoint;
        size_t pos = url.find("://");
        if (pos != std::string::npos) {
            pos = url.find("/", pos + 3);
            if (pos != std::string::npos) {
                endpoint = url.substr(pos);
            }
        }

        LOG_DEBUG_HTTP("🌐 Socket.IO endpoint: " + endpoint);

        // Get request body
        std::string body;
        CefRefPtr<CefPostData> postData = request->GetPostData();
        if (postData) {
            LOG_DEBUG_HTTP("🌐 Processing Socket.IO POST data...");
            CefPostData::ElementVector elements;
            postData->GetElements(elements);
            for (auto& element : elements) {
                if (element->GetType() == PDE_TYPE_BYTES) {
                    size_t size = element->GetBytesCount();
                    std::vector<char> buffer(size);
                    element->GetBytes(size, buffer.data());
                    body = std::string(buffer.data(), size);
                }
            }
        }

        // Get headers for Socket.IO forwarding
        CefRequest::HeaderMap socketHeaders;
        request->GetHeaderMap(socketHeaders);

        // Create AsyncWalletResourceHandler for Socket.IO
        return new AsyncWalletResourceHandler(method, endpoint, body, domain, browser, socketHeaders);
    }

    // Check if this is a wallet endpoint
    if (!isWalletEndpoint(url)) {
        LOG_DEBUG_HTTP("🌐 Not a wallet endpoint, allowing normal processing");
        return nullptr; // Let CEF handle it normally
    }

    LOG_DEBUG_HTTP("🌐 Wallet endpoint detected, creating async handler");

    // Get request body
    std::string body;
    CefRefPtr<CefPostData> postData = request->GetPostData();
    if (postData) {
        LOG_DEBUG_HTTP("🌐 Processing POST data...");
        CefPostData::ElementVector elements;
        postData->GetElements(elements);
        for (auto& element : elements) {
            if (element->GetType() == PDE_TYPE_BYTES) {
                size_t size = element->GetBytesCount();
                std::vector<char> buffer(size);
                element->GetBytes(size, buffer.data());
                body = std::string(buffer.data(), size);
            }
        }
    }

    // Extract endpoint from URL
    std::string endpoint;
    size_t pos = url.find("://");
    if (pos != std::string::npos) {
        pos = url.find("/", pos + 3);
        if (pos != std::string::npos) {
            endpoint = url.substr(pos);
        }
    }

    LOG_DEBUG_HTTP("🌐 Extracted endpoint: " + endpoint);

    // Log all available frame information
    LOG_DEBUG_HTTP("🌐 === FRAME DEBUGGING START ===");

    if (frame) {
        LOG_DEBUG_HTTP("🌐 Frame exists: YES");
        LOG_DEBUG_HTTP("🌐 Frame URL: " + frame->GetURL().ToString());
        LOG_DEBUG_HTTP("🌐 Frame Name: " + frame->GetName().ToString());
        LOG_DEBUG_HTTP("🌐 Frame Identifier: " + frame->GetIdentifier().ToString());
        LOG_DEBUG_HTTP("🌐 Frame Is Main: " + std::string(frame->IsMain() ? "YES" : "NO"));
        LOG_DEBUG_HTTP("🌐 Frame Is Valid: " + std::string(frame->IsValid() ? "YES" : "NO"));
    } else {
        LOG_DEBUG_HTTP("🌐 Frame exists: NO");
    }

    if (browser) {
        LOG_DEBUG_HTTP("🌐 Browser exists: YES");
        CefRefPtr<CefFrame> mainFrame = browser->GetMainFrame();
        if (mainFrame) {
            LOG_DEBUG_HTTP("🌐 Main Frame URL: " + mainFrame->GetURL().ToString());
            LOG_DEBUG_HTTP("🌐 Main Frame Name: " + mainFrame->GetName().ToString());
            LOG_DEBUG_HTTP("🌐 Main Frame Identifier: " + mainFrame->GetIdentifier().ToString());
        } else {
            LOG_DEBUG_HTTP("🌐 Main Frame: NULL");
        }
    } else {
        LOG_DEBUG_HTTP("🌐 Browser exists: NO");
    }

    // Log request information
    LOG_DEBUG_HTTP("🌐 Request URL: " + request->GetURL().ToString());
    LOG_DEBUG_HTTP("🌐 Request Method: " + request->GetMethod().ToString());
    LOG_DEBUG_HTTP("🌐 Request Referrer URL: " + request->GetReferrerURL().ToString());
    LOG_DEBUG_HTTP("🌐 Request Referrer Policy: " + std::to_string(request->GetReferrerPolicy()));

    // Log request headers
    CefRequest::HeaderMap headers;
    request->GetHeaderMap(headers);
    LOG_DEBUG_HTTP("🌐 Request Headers Count: " + std::to_string(headers.size()));
    for (const auto& header : headers) {
        LOG_DEBUG_HTTP("🌐 Header: " + header.first.ToString() + " = " + header.second.ToString());
    }

    LOG_DEBUG_HTTP("🌐 === FRAME DEBUGGING END ===");

    // Extract source domain from the main frame that made the request
    std::string domain = extractDomain(browser, request);
    LOG_DEBUG_HTTP("🌐 Final extracted source domain: " + domain);

    if (!endpoint.empty()) {
        LOG_DEBUG_HTTP("🌐 About to create AsyncWalletResourceHandler...");
        // Create and return async handler
        AsyncWalletResourceHandler* handler = new AsyncWalletResourceHandler(method, endpoint, body, domain, browser, headers);
        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler created successfully");
        return handler;
    }

    LOG_DEBUG_HTTP("🌐 Could not extract endpoint from URL: " + url);
    return nullptr;
}

void HttpRequestInterceptor::OnResourceRedirect(CefRefPtr<CefBrowser> browser,
                                               CefRefPtr<CefFrame> frame,
                                               CefRefPtr<CefRequest> request,
                                               CefRefPtr<CefResponse> response,
                                               CefString& new_url) {
    CEF_REQUIRE_IO_THREAD();
    LOG_DEBUG_HTTP("🌐 Resource redirect: " + new_url.ToString());
}

bool HttpRequestInterceptor::OnResourceResponse(CefRefPtr<CefBrowser> browser,
                                              CefRefPtr<CefFrame> frame,
                                              CefRefPtr<CefRequest> request,
                                              CefRefPtr<CefResponse> response) {
    CEF_REQUIRE_IO_THREAD();
    return false;
}


CefRefPtr<CefCookieAccessFilter> HttpRequestInterceptor::GetCookieAccessFilter(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefRefPtr<CefRequest> request) {
    if (CookieBlockManager::GetInstance().IsInitialized()) {
        return new CookieAccessFilterWrapper();
    }
    return nullptr;
}

bool HttpRequestInterceptor::isWalletEndpoint(const std::string& url) {
    // Check if URL contains wallet endpoints
    return (url.find("/brc100/") != std::string::npos ||
            url.find("/wallet/") != std::string::npos ||
            url.find("/transaction/") != std::string::npos ||
            url.find("/getVersion") != std::string::npos ||
            url.find("/getPublicKey") != std::string::npos ||
            url.find("/createAction") != std::string::npos ||
            url.find("/signAction") != std::string::npos ||
            url.find("/processAction") != std::string::npos ||
            url.find("/internalizeAction") != std::string::npos ||
            url.find("/abortAction") != std::string::npos ||
            url.find("/listActions") != std::string::npos ||
            url.find("/isAuthenticated") != std::string::npos ||
            url.find("/createSignature") != std::string::npos ||
            url.find("/api/brc-100/") != std::string::npos ||
            url.find("/waitForAuthentication") != std::string::npos ||
            url.find("/listOutputs") != std::string::npos ||
            url.find("/relinquishOutput") != std::string::npos ||
            url.find("/createHmac") != std::string::npos ||
            url.find("/verifyHmac") != std::string::npos ||
            url.find("/encrypt") != std::string::npos ||
            url.find("/decrypt") != std::string::npos ||
            url.find("/verifySignature") != std::string::npos ||
            url.find("/getNetwork") != std::string::npos ||
            url.find("/getHeight") != std::string::npos ||
            url.find("/getHeaderForHeight") != std::string::npos ||
            url.find("/acquireCertificate") != std::string::npos ||
            url.find("/listCertificates") != std::string::npos ||
            url.find("/proveCertificate") != std::string::npos ||
            url.find("/relinquishCertificate") != std::string::npos ||
            url.find("/discoverByIdentityKey") != std::string::npos ||
            url.find("/discoverByAttributes") != std::string::npos ||
            url.find("/socket.io/") != std::string::npos ||
            url.find("/.well-known/auth") != std::string::npos ||
            url.find("/listMessages") != std::string::npos ||
            url.find("/sendMessage") != std::string::npos ||
            url.find("/acknowledgeMessage") != std::string::npos);
}

bool HttpRequestInterceptor::isSocketIOConnection(const std::string& url) {
    // Check if this is a Socket.IO connection to our daemon or Babbage messagebox
    bool isLocalhost = url.find("localhost:31301") != std::string::npos;
    bool isBabbageMessagebox = url.find("messagebox.babbage.systems/socket.io/") != std::string::npos;
    bool isSocketIO = url.find("/socket.io/") != std::string::npos;

    LOG_DEBUG_HTTP("🌐 Checking Socket.IO connection: " + url + " - localhost: " + (isLocalhost ? "true" : "false") + ", babbage: " + (isBabbageMessagebox ? "true" : "false") + ", socket.io: " + (isSocketIO ? "true" : "false"));

    return (isLocalhost && isSocketIO) || isBabbageMessagebox;
}

std::string HttpRequestInterceptor::extractDomain(CefRefPtr<CefBrowser> browser, CefRefPtr<CefRequest> request) {
    std::string domain;

    // Use main frame URL as the primary source (most reliable)
    if (browser) {
        CefRefPtr<CefFrame> mainFrame = browser->GetMainFrame();
        if (mainFrame && mainFrame->GetURL().length() > 0) {
            std::string mainFrameUrl = mainFrame->GetURL().ToString();
            LOG_DEBUG_HTTP("🌐 Using main frame URL for domain extraction: " + mainFrameUrl);
            size_t protocolPos = mainFrameUrl.find("://");
            if (protocolPos != std::string::npos) {
                size_t domainStart = protocolPos + 3;
                size_t domainEnd = mainFrameUrl.find("/", domainStart);
                if (domainEnd != std::string::npos) {
                    domain = mainFrameUrl.substr(domainStart, domainEnd - domainStart);
                } else {
                    domain = mainFrameUrl.substr(domainStart);
                }
            }
        }
    }

    // Fallback to referrer URL if main frame URL is not available
    if (domain.empty()) {
        std::string referrerUrl = request->GetReferrerURL().ToString();
        if (!referrerUrl.empty()) {
            LOG_DEBUG_HTTP("🌐 Using referrer URL for domain extraction (fallback): " + referrerUrl);
            size_t protocolPos = referrerUrl.find("://");
            if (protocolPos != std::string::npos) {
                size_t domainStart = protocolPos + 3;
                size_t domainEnd = referrerUrl.find("/", domainStart);
                if (domainEnd != std::string::npos) {
                    domain = referrerUrl.substr(domainStart, domainEnd - domainStart);
                } else {
                    domain = referrerUrl.substr(domainStart);
                }
            }
        }
    }

    LOG_DEBUG_HTTP("🌐 Extracted domain: " + domain);
    return domain;
}
