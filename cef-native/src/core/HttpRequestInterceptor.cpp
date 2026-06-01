#include "../../include/core/HttpRequestInterceptor.h"
#include "../../include/core/CookieBlockManager.h"
#include "../../include/core/ManifestFetcher.h"
#include "../../include/core/PermissionEngine.h"
#include "../../include/core/PermissionGate.h"
#include "../../include/core/SyncHttpClient.h"
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
#include "../../include/core/PaidContentCache.h"
#include "../../include/core/TabManager.h"

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
#include <unordered_map>
#include <unordered_set>
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

// Permission-prompt modal timeout (delay before postAuthTimeout fires the
// "{...timeout...}" error response to the dApp). Applies to all six prompt
// kinds: domain approval, identity-key reveal, key-linkage reveal, certificate
// disclosure, scoped permission (protocol/basket/counterparty), and payment
// confirmation.
//
// Bumped 60s → 600s on 2026-05-26 after a live SocialCert cert acquire failed:
// the counterparty_permission_prompt fired correctly (new "auth message
// signature" counterparty), the user took longer than 60s to read and click
// Allow, the modal returned "scoped permission timeout" to the dApp, and the
// cert acquire flow aborted. The user's eventual click only persisted the
// grant for next time — the in-flight request was already lost.
//
// 600s (10 min) is functionally "unbounded" for any reasonable human decision
// time while still bounding the worst case (orphaned modals, crashed
// subprocess, OS sleep). If practical experience shows we want truly "off",
// that becomes a Phase-2 cleanup-path audit (tab close, navigate-away,
// subprocess crash) before removing the backstop entirely.
static constexpr int kPromptAuthTimeoutMs = 600000;

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
        // Phase 1.5 Step 1 — persistent grant from the domain_approval
        // "Allow this site to identify you" checkbox. Default false; set
        // alongside trust_level=approved when user ticks the box.
        bool identityKeyDisclosureAllowed = false;
    };

    static DomainPermissionCache& GetInstance() {
        static DomainPermissionCache instance;
        return instance;
    }

    // Lookup: cached first, then fetches from Rust backend synchronously.
    // Failure cases (HTTP timeout, parse error) are NOT cached — they return
    // a "unknown" Permission to the caller but the next call retries the
    // fetch. Without this, a single transient failure (e.g., Rust busy at
    // startup serving many parallel requests) poisons the cache with
    // "unknown" for an already-approved domain, triggering a spurious
    // domain_approval modal. See the recurring user-visible race condition
    // reported during SocialCert testing (2026-05-13).
    Permission getPermission(const std::string& domain) {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            auto it = cache_.find(domain);
            if (it != cache_.end()) {
                return it->second;
            }
        }
        bool fetchSucceeded = false;
        Permission perm = fetchFromBackend(domain, fetchSucceeded);
        if (fetchSucceeded) {
            std::lock_guard<std::mutex> lock(mutex_);
            cache_[domain] = perm;
        } else {
            LOG_DEBUG_HTTP("🔒 DomainPermissionCache fetch failed for "
                           + domain + " — NOT caching, will retry next call");
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

    // fetchSucceeded out-param distinguishes "Rust returned an explicit
    // result" (worth caching) from "request failed or didn't reach Rust"
    // (NOT worth caching — we'd cache a phantom unknown state).
    Permission fetchFromBackend(const std::string& domain, bool& fetchSucceeded) {
        Permission result;
        result.trustLevel = "unknown";
        fetchSucceeded = false;

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

        // 3s timeout for localhost — was 1s but caused cache poisoning on
        // busy startup when Rust is serving many parallel requests. Long
        // enough to ride out load spikes, short enough that a truly down
        // Rust still surfaces quickly. Combined with the no-cache-on-failure
        // policy in getPermission(), this fully resolves the recurring
        // "site already approved but domain_approval modal fires anyway"
        // race condition.
        DWORD timeout = 3000;
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
            result.identityKeyDisclosureAllowed = json.value("identityKeyDisclosureAllowed", false);
            fetchSucceeded = true;  // Parsed OK — cache this result.
        } catch (const std::exception& e) {
            LOG_DEBUG_HTTP("🔒 Failed to parse domain permission response: " + std::string(e.what()));
        }

        return result;
    }
#else
    Permission fetchFromBackend(const std::string& domain, bool& fetchSucceeded) {
        Permission result;
        result.trustLevel = "unknown";
        fetchSucceeded = false;

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
            result.identityKeyDisclosureAllowed = json.value("identityKeyDisclosureAllowed", false);
            fetchSucceeded = true;
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

    // Phase 1 polish — distinguish the three outcomes of a /wallet/status
    // call so we can apply DIFFERENT cache TTLs:
    //
    //   Exists       — wallet is present. Cache 30s (positive TTL).
    //   DoesNotExist — Rust returned {exists:false}. Cache 30s (a fresh
    //                  wallet is unlikely to appear in 30s).
    //   FetchFailed  — network/timeout/parse error. The wallet IS likely
    //                  alive; we just couldn't talk to it this instant.
    //                  Cache only 2s so the next 402 retries cleanly.
    //
    // Before this fix, a single transient timeout would poison BRC-121 for
    // 30s with "no wallet" — observed during testing where /wallet/status
    // hit the 1s WinHTTP timeout while the wallet was responsive on a
    // different code path 2.7s later.
    enum class Status { Exists, DoesNotExist, FetchFailed };

    static constexpr int FETCH_TIMEOUT_MS = 3000;       // up from 1000
    static constexpr int POSITIVE_CACHE_SECS = 30;      // Exists / DoesNotExist
    static constexpr int TRANSIENT_CACHE_SECS = 2;      // FetchFailed

    // P2 perf fix: mutex released before blocking I/O to allow concurrent cached reads
    bool walletExists() {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            auto now = std::chrono::steady_clock::now();
            int ttl_secs = (lastStatus_ == Status::FetchFailed)
                ? TRANSIENT_CACHE_SECS : POSITIVE_CACHE_SECS;
            if (valid_ && (now - lastCheck_) < std::chrono::seconds(ttl_secs)) {
                return lastStatus_ == Status::Exists;
            }
        }
        Status s = fetchWalletStatus();
        {
            std::lock_guard<std::mutex> lock(mutex_);
            lastStatus_ = s;
            valid_ = true;
            lastCheck_ = std::chrono::steady_clock::now();
        }
        return s == Status::Exists;
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
    Status lastStatus_ = Status::FetchFailed;
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

    Status fetchWalletStatus() {
        HINTERNET hSession = getSession();
        if (!hSession) return Status::FetchFailed;

        HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 31301, 0);
        if (!hConnect) return Status::FetchFailed;

        HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"GET",
                                                L"/wallet/status",
                                                nullptr,
                                                WINHTTP_NO_REFERER,
                                                WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
        if (!hRequest) {
            WinHttpCloseHandle(hConnect);
            return Status::FetchFailed;
        }

        DWORD timeout = FETCH_TIMEOUT_MS;
        WinHttpSetOption(hRequest, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hRequest, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hRequest, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));

        if (!WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0, nullptr, 0, 0, 0) ||
            !WinHttpReceiveResponse(hRequest, nullptr)) {
            WinHttpCloseHandle(hRequest);
            WinHttpCloseHandle(hConnect);
            return Status::FetchFailed;
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
            return json.value("exists", false) ? Status::Exists : Status::DoesNotExist;
        } catch (...) {
            return Status::FetchFailed;
        }
    }
#else
    Status fetchWalletStatus() {
        HttpResponse resp = SyncHttpClient::Get("http://localhost:31301/wallet/status",
                                                FETCH_TIMEOUT_MS);
        if (!resp.success) return Status::FetchFailed;

        try {
            auto json = nlohmann::json::parse(resp.body);
            return json.value("exists", false) ? Status::Exists : Status::DoesNotExist;
        } catch (...) {
            return Status::FetchFailed;
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

// Phase 1.5 Step 1 -- in-memory "Always allow for this site" caches for the two
// new privacy-perimeter prompt types. Step 2 will replace these with SQLite
// columns on the domain_permissions row (or a parallel child table). For Step
// 1 the cache is process-lifetime only; on browser restart the user re-prompts.
class IdentityKeyApprovalCache {
public:
    static IdentityKeyApprovalCache& GetInstance() {
        static IdentityKeyApprovalCache instance;
        return instance;
    }
    bool isApproved(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        return approved_.count(domain) > 0;
    }
    void approve(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        approved_.insert(domain);
    }
    void revoke(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        approved_.erase(domain);
    }
private:
    IdentityKeyApprovalCache() = default;
    std::mutex mutex_;
    std::set<std::string> approved_;
};

class KeyLinkageApprovalCache {
public:
    static KeyLinkageApprovalCache& GetInstance() {
        static KeyLinkageApprovalCache instance;
        return instance;
    }
    bool isApproved(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        return approved_.count(domain) > 0;
    }
    void approve(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        approved_.insert(domain);
    }
    void revoke(const std::string& domain) {
        std::lock_guard<std::mutex> lock(mutex_);
        approved_.erase(domain);
    }
private:
    KeyLinkageApprovalCache() = default;
    std::mutex mutex_;
    std::set<std::string> approved_;
};

// ============================================================================
// Phase 1.5 Step 6 (Commit A) — SubPermissionCache
// ============================================================================
// Caches the {protocol, basket, counterparty} sub-permission lookups from the
// V18 child tables (queried via the Step 3 /domain/permissions/{kind} GET
// endpoints). Without this, the engine would HTTP round-trip on every BRC-100
// call to decide if a scope is granted.
//
// Mirrors DomainPermissionCache's pattern:
//   - One read miss → blocking sync HTTP to Rust → populate cache → return
//   - Subsequent reads hit the cache
//   - Explicit invalidation on domain_permission_invalidate IPC
//
// Cache shape: per-domain map of (scope-tuple → bool). Cleared on any change
// to a domain's perms so re-grant / re-revoke take effect immediately.
class SubPermissionCache {
public:
    static SubPermissionCache& GetInstance() {
        static SubPermissionCache instance;
        return instance;
    }

    // Protocol grant: (level, name, keyId, counterparty) → granted-or-not.
    // Honors '*' wildcard keyId on the Rust side; we just relay the result.
    bool isProtocolGranted(const std::string& domain, int level, const std::string& name,
                            const std::string& keyId, const std::string& counterparty) {
        const std::string key = "p:" + std::to_string(level) + ":" + name + ":" + keyId + ":" + counterparty;
        return queryAndCache(domain, key, [&]() {
            LOG_DEBUG_HTTP("🛡️ isProtocolGranted FETCH domain=" + domain
                           + " level=" + std::to_string(level)
                           + " name='" + name + "'"
                           + " keyId.size=" + std::to_string(keyId.size())
                           + " counterparty.empty=" + (counterparty.empty() ? "1" : "0"));
            return fetchBool("/domain/permissions/protocol", domain, [&](const nlohmann::json& perms) {
                LOG_DEBUG_HTTP("🛡️ isProtocolGranted GOT " + std::to_string(perms.size())
                               + " rows for " + domain);
                int row = 0;
                for (const auto& p : perms) {
                    const int rowIdx = row++;
                    // Per-row try/catch: one malformed legacy row (binary
                    // keyId, null where string expected, etc.) must NOT
                    // poison the entire scan. Log the offender and skip.
                    try {
                        // nlohmann::json gotcha: p.value(key, default) throws
                        // json::type_error if the stored value is null and the
                        // default's type doesn't match. Counterparty IS null on
                        // every protocol row that's not counterparty-bound, so
                        // we MUST use null-safe accessors here. This bug was
                        // silently aborting the entire scan and triggering
                        // spurious "Always allow doesn't stick" reports.
                        auto safeStr = [](const nlohmann::json& obj, const char* key) -> std::string {
                            auto it = obj.find(key);
                            if (it == obj.end() || !it->is_string()) return "";
                            return it->get<std::string>();
                        };
                        int plvl = -1;
                        if (auto it = p.find("securityLevel"); it != p.end() && it->is_number_integer()) {
                            plvl = it->get<int>();
                        }
                        std::string pname = safeStr(p, "protocolName");
                        std::string pkey = safeStr(p, "keyId");
                        std::string pcp = safeStr(p, "counterparty");
                        const bool levelMatch = (plvl == level);
                        const bool nameMatch = (pname == name);
                        const bool keyMatch = (pkey == keyId || pkey == "*");
                        const bool cpMatch = (counterparty.empty() || pcp.empty() || pcp == counterparty);
                        LOG_DEBUG_HTTP("🛡️   row[" + std::to_string(rowIdx) + "]: lvl=" + std::to_string(plvl)
                                       + " name='" + pname + "'"
                                       + " keyStored=" + (pkey == "*" ? "WILDCARD" : ("size=" + std::to_string(pkey.size())))
                                       + " pcp.empty=" + (pcp.empty() ? "1" : "0")
                                       + " | levelMatch=" + (levelMatch ? "1" : "0")
                                       + " nameMatch=" + (nameMatch ? "1" : "0")
                                       + " keyMatch=" + (keyMatch ? "1" : "0")
                                       + " cpMatch=" + (cpMatch ? "1" : "0"));
                        if (levelMatch && nameMatch && keyMatch && cpMatch) {
                            LOG_DEBUG_HTTP("🛡️   → MATCH, returning true");
                            return true;
                        }
                    } catch (const std::exception& e) {
                        LOG_DEBUG_HTTP("🛡️   row[" + std::to_string(rowIdx)
                                       + "] EXCEPTION (" + std::string(e.what())
                                       + ") — skipping. Raw: "
                                       + p.dump().substr(0, std::min<size_t>(p.dump().size(), 200)));
                    } catch (...) {
                        LOG_DEBUG_HTTP("🛡️   row[" + std::to_string(rowIdx)
                                       + "] UNKNOWN EXCEPTION — skipping");
                    }
                }
                LOG_DEBUG_HTTP("🛡️   no row matched, returning false");
                return false;
            });
        });
    }

    // Basket grant: read_write satisfies a read check; read alone does not satisfy read_write.
    bool isBasketGranted(const std::string& domain, const std::string& basket, const std::string& requiredAccess) {
        const std::string key = "b:" + basket + ":" + requiredAccess;
        return queryAndCache(domain, key, [&]() {
            return fetchBool("/domain/permissions/basket", domain, [&](const nlohmann::json& perms) {
                // Same null-safe accessor as the protocol scan — nlohmann's
                // p.value(key, default) throws on null values.
                auto safeStr = [](const nlohmann::json& obj, const char* k) -> std::string {
                    auto it = obj.find(k);
                    if (it == obj.end() || !it->is_string()) return "";
                    return it->get<std::string>();
                };
                for (const auto& p : perms) {
                    try {
                        if (safeStr(p, "basket") != basket) continue;
                        const std::string access = safeStr(p, "access");
                        if (access == "read_write") return true;
                        if (access == "read" && requiredAccess == "read") return true;
                    } catch (...) {
                        // Skip malformed row, continue.
                    }
                }
                return false;
            });
        });
    }

    bool isCounterpartyGranted(const std::string& domain, const std::string& counterparty) {
        const std::string key = "c:" + counterparty;
        return queryAndCache(domain, key, [&]() {
            return fetchBool("/domain/permissions/counterparty", domain, [&](const nlohmann::json& perms) {
                auto safeStr = [](const nlohmann::json& obj, const char* k) -> std::string {
                    auto it = obj.find(k);
                    if (it == obj.end() || !it->is_string()) return "";
                    return it->get<std::string>();
                };
                for (const auto& p : perms) {
                    try {
                        if (safeStr(p, "counterparty") == counterparty) return true;
                    } catch (...) {
                        // Skip malformed row, continue.
                    }
                }
                return false;
            });
        });
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
    SubPermissionCache() = default;
    std::mutex mutex_;
    // Per-domain map of (scope-tuple-key → bool). Memoizes the lookup so
    // a single BRC-100 call sequence doesn't HTTP-fetch repeatedly.
    std::unordered_map<std::string, std::unordered_map<std::string, bool>> cache_;

    template <typename Compute>
    bool queryAndCache(const std::string& domain, const std::string& key, Compute compute) {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            auto domIt = cache_.find(domain);
            if (domIt != cache_.end()) {
                auto entryIt = domIt->second.find(key);
                if (entryIt != domIt->second.end()) {
                    return entryIt->second;
                }
            }
        }
        bool result = compute();
        {
            std::lock_guard<std::mutex> lock(mutex_);
            cache_[domain][key] = result;
        }
        return result;
    }

    // Helper: fetch the /domain/permissions/{kind} list and let the caller
    // scan it for the specific tuple. Returns false on any HTTP / parse error
    // (treat absence-of-grant as default-denied, callers will prompt).
    // Timeout bumped from 1.5s → 3s — same rationale as DomainPermissionCache:
    // Rust localhost can be busy during dApp page-load bursts (SocialCert hits
    // 30+ wallet endpoints in 6 seconds), and an aggressive timeout silently
    // turns "actually granted" into "default-denied" → spurious re-prompts.
    template <typename Scan>
    bool fetchBool(const std::string& endpoint, const std::string& domain, Scan scan) {
        const std::string url = "http://localhost:31301" + endpoint
            + "?domain=" + domain;
        HttpResponse resp = SyncHttpClient::Get(url, 3000);
        if (!resp.success || resp.statusCode != 200) return false;
        try {
            auto j = nlohmann::json::parse(resp.body);
            if (!j.contains("permissions") || !j["permissions"].is_array()) return false;
            return scan(j["permissions"]);
        } catch (const std::exception& e) {
            LOG_DEBUG_HTTP("🛡️ fetchBool EXCEPTION (" + std::string(e.what())
                           + ") body preview: "
                           + resp.body.substr(0, std::min<size_t>(resp.body.size(), 800)));
            return false;
        } catch (...) {
            LOG_DEBUG_HTTP("🛡️ fetchBool UNKNOWN EXCEPTION, body preview: "
                           + resp.body.substr(0, std::min<size_t>(resp.body.size(), 800)));
            return false;
        }
    }
};

// ============================================================================
// Phase 1.5 Step 6 (Commit A) — Body-peeking helpers
// ============================================================================
// Extract scope-specific data from BRC-100 request bodies so the
// PermissionContext builder can fill in the right fields. Each helper returns
// a `valid` flag — false means "this endpoint doesn't have this scope" and
// the caller should not check the matching scope.

namespace {

struct ProtocolScope {
    bool valid = false;
    int level = 2;
    std::string name;
    std::string keyId = "*";
    std::string counterparty; // empty = none / self
};

struct BasketScope {
    bool valid = false;
    std::string basket;
    std::string requiredAccess = "read";
};

// Parse protocolID JSON value into (level, name). Accepts [level, name] array
// or "level-name" string per @bsv/sdk shape. Returns false if unparseable.
static bool parseProtocolId(const nlohmann::json& v, int& outLevel, std::string& outName) {
    if (v.is_array() && v.size() >= 2) {
        if (v[0].is_number()) outLevel = v[0].get<int>();
        if (v[1].is_string()) outName = v[1].get<std::string>();
        return !outName.empty();
    }
    if (v.is_string()) {
        const std::string s = v.get<std::string>();
        auto dash = s.find('-');
        if (dash != std::string::npos) {
            try { outLevel = std::stoi(s.substr(0, dash)); } catch (...) {}
            outName = s.substr(dash + 1);
        } else {
            outName = s;
        }
        return !outName.empty();
    }
    return false;
}

// Extract protocol scope from a BRC-100 call body. Returns valid=true only
// for endpoints that genuinely use a protocolID/keyID tuple to derive a key
// FROM the user's wallet.
//
// Verify-only endpoints (verifySignature, verifyHmac) are deliberately
// excluded — they take a pubkey + message + signature as inputs and run
// pure verification. No key is derived from the user's wallet, no key
// material is exposed, and there is no privacy implication. Matrix A in
// PERMISSION_UX_DESIGN.md classifies them as "Silent always". Including
// them here caused a Commit E regression where socialcert.net's X
// verification flow fired 7+ unnecessary prompts in 60 seconds and
// timed out (see project_phase15_commit_e_verify_bug memory note).
static ProtocolScope extractProtocolScope(const std::string& endpoint, const std::string& body) {
    ProtocolScope s;
    const bool isProtocolEndpoint =
        endpoint.find("/createSignature") != std::string::npos ||
        endpoint.find("/createHmac") != std::string::npos ||
        endpoint.find("/encrypt") != std::string::npos ||
        endpoint.find("/decrypt") != std::string::npos;
    if (!isProtocolEndpoint || body.empty()) return s;
    try {
        auto j = nlohmann::json::parse(body);
        if (!j.contains("protocolID")) return s;
        if (!parseProtocolId(j["protocolID"], s.level, s.name)) return s;
        // keyID — defaults to wildcard if absent
        if (j.contains("keyID") && j["keyID"].is_string()) {
            s.keyId = j["keyID"].get<std::string>();
        }
        // counterparty — empty means 'self' or no specific party
        if (j.contains("counterparty") && j["counterparty"].is_string()) {
            const std::string cp = j["counterparty"].get<std::string>();
            if (cp != "self" && cp != "anyone") {
                s.counterparty = cp;
            }
        }
        s.valid = true;
    } catch (...) {
        // Malformed body — keep s.valid = false
    }
    return s;
}

// Extract basket scope from a BRC-100 call body.
static BasketScope extractBasketScope(const std::string& endpoint, const std::string& body) {
    BasketScope s;
    const bool isBasketEndpoint =
        endpoint.find("/listOutputs") != std::string::npos ||
        endpoint.find("/relinquishOutput") != std::string::npos;
    if (!isBasketEndpoint || body.empty()) return s;
    try {
        auto j = nlohmann::json::parse(body);
        if (j.contains("basket") && j["basket"].is_string()) {
            s.basket = j["basket"].get<std::string>();
            // relinquishOutput is destructive → read_write; listOutputs → read
            s.requiredAccess = (endpoint.find("/relinquishOutput") != std::string::npos)
                ? "read_write" : "read";
            s.valid = !s.basket.empty();
        }
    } catch (...) {}
    return s;
}

// Classify a request into a PermissionCallKind for the engine.
//
// Order matters — privacy-perimeter and scope kinds take precedence over
// payment/generic. The engine's branch ordering handles trust separately,
// so we don't classify DomainTrust here; the caller short-circuits on
// trust before calling buildPermissionContext.
static hodos::PermissionCallKind classifyCallKind(
    const std::string& endpoint,
    const std::string& body,
    const ProtocolScope& proto,
    const BasketScope& basket
) {
    using K = hodos::PermissionCallKind;

    // Privacy perimeter — endpoint-driven.
    if (endpoint.find("/revealCounterpartyKeyLinkage") != std::string::npos) return K::CounterpartyKeyLinkage;
    if (endpoint.find("/revealSpecificKeyLinkage") != std::string::npos) return K::SpecificKeyLinkage;

    // getPublicKey is identity-key only when body matches the identity-key style
    // (matches the existing inline isIdentityKeyStyleGetPublicKey logic).
    if (endpoint.find("/getPublicKey") != std::string::npos) {
        bool identityKeyStyle = body.empty();
        if (!identityKeyStyle) {
            try {
                auto j = nlohmann::json::parse(body);
                bool flag = j.contains("identityKey") && j["identityKey"].is_boolean() && j["identityKey"].get<bool>();
                bool hasProto = j.contains("protocolID") && !j["protocolID"].is_null();
                bool hasKey = j.contains("keyID") && j["keyID"].is_string() && !j["keyID"].get<std::string>().empty();
                identityKeyStyle = flag || !hasProto || !hasKey;
            } catch (...) { identityKeyStyle = true; }
        }
        return identityKeyStyle ? K::IdentityKeyReveal : K::GenericApproved;
    }

    // Cert disclosure
    if (endpoint.find("/proveCertificate") != std::string::npos) return K::CertificateDisclosure;

    // Payment endpoints (createAction / acquireCertificate / sendMessage).
    // Existing isPaymentEndpoint logic.
    if (endpoint.find("/createAction") != std::string::npos
        || endpoint.find("/acquireCertificate") != std::string::npos
        || endpoint.find("/sendMessage") != std::string::npos) {
        return K::Payment;
    }

    // Scoped grants — order: counterparty (more specific) before protocol.
    if (proto.valid && !proto.counterparty.empty()) return K::CounterpartyUse;
    if (proto.valid) return K::ProtocolUse;
    if (basket.valid) return K::BasketAccess;

    return K::GenericApproved;
}

// Build a fully-populated PermissionContext for engine.Decide().
// Caller must have already short-circuited on trustLevel == "unknown" / "blocked"
// (engine handles those branches, but we don't want to fetch sub-permissions
// for a domain we won't approve anyway).
static hodos::PermissionContext buildPermissionContext(
    const std::string& domain,
    const std::string& endpoint,
    const std::string& body,
    const DomainPermissionCache::Permission& perm,
    int64_t requestedCents,
    int64_t sessionSpentCents,
    int paymentRequestsThisMinute,
    int paymentCountThisSession,
    bool bsvPriceAvailable = true
) {
    using K = hodos::PermissionCallKind;
    hodos::PermissionContext ctx;

    ProtocolScope proto = extractProtocolScope(endpoint, body);
    BasketScope basket = extractBasketScope(endpoint, body);

    ctx.callKind = classifyCallKind(endpoint, body, proto, basket);

    // Domain-level state
    ctx.trustLevel = perm.trustLevel;
    ctx.perTxLimitCents = perm.perTxLimitCents;
    ctx.perSessionLimitCents = perm.perSessionLimitCents;
    ctx.rateLimitPerMin = perm.rateLimitPerMin;
    ctx.maxTxPerSession = perm.maxTxPerSession;
    ctx.identityKeyDisclosureAllowed = perm.identityKeyDisclosureAllowed;

    // Session counters
    ctx.sessionSpentCents = sessionSpentCents;
    ctx.paymentRequestsThisMinute = paymentRequestsThisMinute;
    ctx.paymentCountThisSession = paymentCountThisSession;

    // Session-scoped privacy-perimeter opt-ins (in-memory caches)
    ctx.identityKeySessionOptIn = IdentityKeyApprovalCache::GetInstance().isApproved(domain);
    ctx.keyLinkageSessionOptIn = KeyLinkageApprovalCache::GetInstance().isApproved(domain);

    // Payment-specific
    ctx.requestedCents = requestedCents;
    ctx.bsvPriceAvailable = bsvPriceAvailable;

    // Scoped grant lookups — only fetch the cache for the relevant kind, so
    // a Payment call doesn't HTTP-fetch the protocol cache pointlessly.
    // scopedGrantExists is true iff a matching V18 row exists. For "Allow
    // once" (single-call grant without V18 write), the existing approve →
    // re-issue flow in simple_handler.cpp bypasses Open() entirely via
    // CefURLRequest, so the in-flight call doesn't need a one-shot bypass
    // — it's satisfied by the re-issue. For "Always allow", simple_handler
    // writes the V18 row + invalidates SubPermissionCache before re-issuing,
    // so any subsequent calls from the page see the persistent grant.
    switch (ctx.callKind) {
        case K::ProtocolUse:
            if (proto.valid) {
                ctx.scopedGrantExists = SubPermissionCache::GetInstance().isProtocolGranted(
                    domain, proto.level, proto.name, proto.keyId, proto.counterparty);
            }
            break;
        case K::CounterpartyUse:
            // CounterpartyUse fires when the call has BOTH a protocolID and
            // a counterparty. Two distinct grants can satisfy it:
            //   1. A standalone counterparty grant ("this site can derive
            //      keys with this peer at all") — written when user clicks
            //      Always-allow on counterparty_permission_prompt.
            //   2. A protocol grant bound to this specific counterparty —
            //      narrower scope, when user wants to allow the specific
            //      protocol-with-peer combination.
            // Either match silences the gate. Check counterparty grant
            // first since it's the broader trust signal.
            if (proto.valid) {
                const bool cpGranted = SubPermissionCache::GetInstance().isCounterpartyGranted(
                    domain, proto.counterparty);
                const bool protoGranted = cpGranted
                    || SubPermissionCache::GetInstance().isProtocolGranted(
                        domain, proto.level, proto.name, proto.keyId, proto.counterparty);
                ctx.scopedGrantExists = cpGranted || protoGranted;
            }
            break;
        case K::BasketAccess:
            if (basket.valid) {
                ctx.scopedGrantExists = SubPermissionCache::GetInstance().isBasketGranted(
                    domain, basket.basket, basket.requiredAccess);
            }
            break;
        case K::Payment:
            // Commit E v1 — paymentScopeKindMissing is deliberately NOT
            // populated here yet. The engine handles it correctly when set
            // (see PermissionEngine.cpp::DecidePayment + unit tests
            // PaymentWithMissing{Protocol,Basket,Counterparty}*), but
            // populating it requires the approve flow to re-run Open() on
            // the original request so the cap gate fires AFTER scope is
            // granted. simple_handler.cpp's brc100_auth_response approve
            // path currently re-issues via CefURLRequest which bypasses
            // GetResourceRequestHandler entirely — so a Payment scope
            // approval would NOT re-trigger the cap check, silently
            // bypassing the cap gate. That breaks the "both gates apply
            // independently" invariant the user requires.
            //
            // Commit E v1 ships scope gates for non-payment endpoints
            // (createSignature, listOutputs, etc.) where there's a single
            // gate per call and the existing approve→re-issue flow works
            // correctly. Sequenced scope-then-cap for Payment lands in a
            // follow-up that wires AsyncWalletResourceHandler to re-run
            // its gate evaluation on approval rather than just re-issuing
            // the upstream request.
            break;
        default:
            // Other kinds don't use scopedGrantExists; leave default false.
            break;
    }

    return ctx;
}

// Human-readable string for a callKind. Used only for shadow-mode logging in
// Commit A — replaced by the engine driving production behavior in later commits.
static const char* callKindToString(hodos::PermissionCallKind k) {
    using K = hodos::PermissionCallKind;
    switch (k) {
        case K::IdentityKeyReveal: return "IdentityKeyReveal";
        case K::CounterpartyKeyLinkage: return "CounterpartyKeyLinkage";
        case K::SpecificKeyLinkage: return "SpecificKeyLinkage";
        case K::SensitiveCertField: return "SensitiveCertField";
        case K::ProtocolUse: return "ProtocolUse";
        case K::BasketAccess: return "BasketAccess";
        case K::CounterpartyUse: return "CounterpartyUse";
        case K::Payment: return "Payment";
        case K::DomainTrust: return "DomainTrust";
        case K::CertificateDisclosure: return "CertificateDisclosure";
        case K::GenericApproved: return "GenericApproved";
    }
    return "Unknown";
}

static const char* decisionKindToString(hodos::PermissionDecision::Kind k) {
    using DK = hodos::PermissionDecision::Kind;
    switch (k) {
        case DK::Silent: return "Silent";
        case DK::Prompt: return "Prompt";
        case DK::Deny: return "Deny";
    }
    return "Unknown";
}

} // anonymous namespace

// Thin entry points for simple_handler.cpp's IPC dispatchers (declared in
// include/core/HttpRequestInterceptor.h).
void MarkIdentityKeyRevealApproved(const std::string& domain) {
    IdentityKeyApprovalCache::GetInstance().approve(domain);
}
void MarkKeyLinkageRevealApproved(const std::string& domain) {
    KeyLinkageApprovalCache::GetInstance().approve(domain);
}

// Phase 2.5 Commit 6 (sub-step 6.b) — single source of truth for the
// post-success auto-approved-payment cluster. See header for design intent.
void OnWalletCallSuccess(int browserId,
                         const std::string& domain,
                         int64_t cents,
                         bool wasAutoApprovedPayment,
                         const std::string& endpoint) {
    if (!wasAutoApprovedPayment || cents <= 0) return;

    SessionManager::GetInstance().recordSpending(browserId, cents);

    CefRefPtr<CefBrowser> headerBrowser = SimpleHandler::GetHeaderBrowser();
    if (!headerBrowser || !headerBrowser->GetMainFrame()) return;

    // Phase 1.5 Step 0 — translate CEF browser identifier to TabManager's
    // Tab::id before sending. React's tab list keys by Tab::id, NOT CEF
    // browser identifier (they are different counters).
    int tabId = TabManager::GetInstance().GetTabIdForBrowserIdentifier(browserId);
    nlohmann::json payload;
    payload["browserId"] = tabId;  // field kept as "browserId" for React compat
    payload["domain"] = domain;
    payload["cents"] = cents;

    CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("payment_success_indicator");
    msg->GetArgumentList()->SetString(0, payload.dump());
    headerBrowser->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);

    LOG_DEBUG_HTTP("💰 OnWalletCallSuccess fired (" + std::to_string(cents)
                   + " cents from " + domain
                   + ", cefBrowserId=" + std::to_string(browserId)
                   + " → tabId=" + std::to_string(tabId)
                   + ", endpoint=" + endpoint + ")");
}

// ============================================================================
// Phase 2.5 Commit 6 sub-step 6.c — Decision 3: free-function modal openers
// ============================================================================
//
// Each opener enrolls a PendingAuthRequest from (ModalContext, ResumeContext)
// and posts the matching CreateNotificationOverlayTask. ResumeContext
// discriminates: HTTP path passes handler-only (resumeKind = kHttpCallback),
// IPC path passes frame+browserId+headersOnApprove (resumeKind = kIpcResponse).
//
// Member trigger functions on AsyncWalletResourceHandler now delegate to
// these openers (declared at file scope) by building a HTTP-path
// ResumeContext from `this`. The IPC bridge wiring in 6.d will call these
// openers directly with kIpcResponse ResumeContext.
//
// urlEncode is defined at file scope earlier in this TU (line ~539);
// CreateNotificationOverlayTask is visible from earlier in the file.

// Parsed certificate disclosure info from proveCertificate request body.
// Phase 2.5 Commit 6 sub-step 6.c — moved from inside AsyncWalletResourceHandler
// to file scope so the free-function modal opener openCertificateDisclosureModal
// can reference it. The static extractCertDisclosureInfo method on the handler
// continues to use this type unchanged via name lookup.
struct CertDisclosureInfo {
    std::string certType;
    std::string certifier;
    std::vector<std::string> fieldsToReveal;
    bool valid = false;
};

// Internal helper: build a PendingAuthRequest by stitching ModalContext +
// ResumeContext + a type string. The resume discriminator chooses kHttpCallback
// when a handler is set, kIpcResponse when a frame is set (and no handler).
// Defaults to kHttpCallback for safety if both are unset (caller error).
static PendingAuthRequest buildPendingAuthRequest(
    const std::string& type,
    const ModalContext& ctx,
    const ResumeContext& resume
) {
    PendingAuthRequest req;
    req.domain = ctx.domain;
    req.method = ctx.method;
    req.endpoint = ctx.endpoint;
    req.body = ctx.body;
    req.type = type;
    req.handler = resume.handler;
    if (resume.handler) {
        req.resumeKind = ResumeKind::kHttpCallback;
    } else if (resume.frame) {
        req.resumeKind = ResumeKind::kIpcResponse;
    } else {
        // No handler and no frame — should not happen in practice but default
        // to HTTP semantics so handleAuthResponse's default switch case is hit.
        req.resumeKind = ResumeKind::kHttpCallback;
    }
    req.frame = resume.frame;
    req.browserId = resume.browserId;
    req.headersOnApprove = resume.headersOnApprove;
    req.httpMethod = resume.httpMethod;
    return req;
}

void openDomainApprovalModal(const ModalContext& ctx, const ResumeContext& resume) {
    LOG_DEBUG_HTTP("🔒 Triggering domain approval for " + ctx.domain);

    // Per-domain dedup — multiple in-flight requests from the same fresh origin
    // share a single modal; all queued requests resolve on Approve.
    bool modalAlreadyShowing = PendingRequestManager::GetInstance().hasPendingForDomain(ctx.domain);

    // domain_approval historically used an empty body in the queued entry.
    // Preserve that by overriding ctx.body to "" before enrollment.
    PendingAuthRequest req = buildPendingAuthRequest("domain_approval", ctx, resume);
    req.body = "";  // historical: body cleared for domain_approval entries
    std::string requestId = PendingRequestManager::GetInstance().addRequest(std::move(req));

    if (modalAlreadyShowing) {
        LOG_DEBUG_HTTP("🔒 Modal already pending for domain " + ctx.domain
                       + ", request queued (requestId: " + requestId + ")");
        return;
    }

    CefPostTask(TID_UI, new CreateNotificationOverlayTask("domain_approval", ctx.domain));
    LOG_DEBUG_HTTP("🔒 Domain approval needed for: " + ctx.domain
                   + " requesting " + ctx.method + " " + ctx.endpoint);
}

void openBRC100AuthApprovalModal(const ModalContext& ctx, const ResumeContext& resume) {
    LOG_DEBUG_HTTP("🔐 Triggering BRC-100 auth approval for " + ctx.domain);

    bool modalAlreadyShowing = PendingRequestManager::GetInstance().hasPendingForDomain(ctx.domain);

    // BRC-100 auth historically stored body verbatim and used the default
    // "domain_approval" type (no explicit type override). The modal type
    // string used by React is also "domain_approval" — BRC-100 auth shares
    // the React modal page.
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("domain_approval", ctx, resume));

    if (modalAlreadyShowing) {
        LOG_DEBUG_HTTP("🔐 Modal already pending for domain " + ctx.domain
                       + ", request queued (requestId: " + requestId + ")");
        return;
    }

    CefPostTask(TID_UI, new CreateNotificationOverlayTask("domain_approval", ctx.domain));
    LOG_DEBUG_HTTP("🔐 BRC-100 auth approval needed for: " + ctx.domain
                   + " requesting " + ctx.method + " " + ctx.endpoint);
}

void openManifestConnectBundleModal(const ModalContext& ctx, const ResumeContext& resume,
                                     const hodos::Manifest& m) {
    LOG_DEBUG_HTTP("📦 Triggering manifest_connect_bundle for " + ctx.domain
                    + " (app=" + m.name + ", " + std::to_string(m.protocols.size())
                    + " protocols, " + std::to_string(m.baskets.size())
                    + " baskets, " + std::to_string(m.certificates.size())
                    + " certs, " + std::to_string(m.counterparties.size())
                    + " counterparties)");

    bool modalAlreadyShowing = PendingRequestManager::GetInstance().hasPendingForDomain(ctx.domain);
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("manifest_connect_bundle", ctx, resume));

    if (modalAlreadyShowing) {
        LOG_DEBUG_HTTP("📦 Modal already pending for domain " + ctx.domain
                        + ", request queued (requestId: " + requestId + ")");
        return;
    }

    // Serialize manifest to JSON, URL-encode, pass as extraParams.
    // 64 KB cap from fetcher + ~33% base64-style inflation → ~85 KB URL-safe
    // string, well under CEF/Chromium's multi-MB URL handling capacity.
    nlohmann::json j;
    j["name"] = m.name;
    j["description"] = m.description;
    j["iconUrl"] = m.iconUrl;
    j["expiresAt"] = m.expiresAt;
    j["version"] = m.version;

    nlohmann::json protocols = nlohmann::json::array();
    for (const auto& p : m.protocols) {
        protocols.push_back({
            {"securityLevel", p.securityLevel},
            {"name", p.name},
            {"keyId", p.keyId},
            {"purpose", p.purpose},
        });
    }
    j["protocols"] = protocols;

    nlohmann::json baskets = nlohmann::json::array();
    for (const auto& b : m.baskets) {
        baskets.push_back({
            {"name", b.name},
            {"access", b.access},
            {"purpose", b.purpose},
        });
    }
    j["baskets"] = baskets;

    nlohmann::json certs = nlohmann::json::array();
    for (const auto& c : m.certificates) {
        certs.push_back({
            {"type", c.type},
            {"fields", c.fields},
            {"purpose", c.purpose},
        });
    }
    j["certificates"] = certs;

    j["spending"] = {
        {"perTransactionUsd", m.spending.perTransactionUsd},
        {"perSessionUsd", m.spending.perSessionUsd},
        {"purpose", m.spending.purpose},
    };

    nlohmann::json counterparties = nlohmann::json::array();
    for (const auto& cp : m.counterparties) {
        counterparties.push_back({
            {"type", cp.type},
            {"counterparty", cp.counterparty},
            {"purpose", cp.purpose},
        });
    }
    j["counterparties"] = counterparties;

    std::string extraParams = "&manifest=" + urlEncode(j.dump());

    CefPostTask(TID_UI, new CreateNotificationOverlayTask(
        "manifest_connect_bundle", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("📦 manifest_connect_bundle notification queued (requestId: " + requestId + ")");
}

void openIdentityKeyRevealModal(const ModalContext& ctx, const ResumeContext& resume) {
    LOG_DEBUG_HTTP("🛡️ Triggering identity_key_reveal for " + ctx.domain);

    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("identity_key_reveal", ctx, resume));

    CefPostTask(TID_UI, new CreateNotificationOverlayTask("identity_key_reveal", ctx.domain));
    LOG_DEBUG_HTTP("🛡️ identity_key_reveal notification queued (requestId: " + requestId + ")");
}

void openKeyLinkageRevealModal(const ModalContext& ctx, const ResumeContext& resume) {
    LOG_DEBUG_HTTP("🛡️ Triggering key_linkage_reveal for " + ctx.domain + " endpoint=" + ctx.endpoint);

    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("key_linkage_reveal", ctx, resume));

    // Verifier + linkage kind + (specific) protocol/keyID — best-effort body parse.
    std::string verifier;
    std::string linkageKind = (ctx.endpoint.find("/revealSpecificKeyLinkage") != std::string::npos)
        ? "specific" : "counterparty";
    std::string protocolName;
    std::string keyId;
    if (!ctx.body.empty()) {
        try {
            auto json = nlohmann::json::parse(ctx.body);
            if (json.contains("verifier") && json["verifier"].is_string()) {
                verifier = json["verifier"].get<std::string>();
            }
            if (json.contains("keyID") && json["keyID"].is_string()) {
                keyId = json["keyID"].get<std::string>();
            }
            if (json.contains("protocolID")) {
                auto& pid = json["protocolID"];
                if (pid.is_array() && pid.size() >= 2 && pid[1].is_string()) {
                    protocolName = pid[1].get<std::string>();
                } else if (pid.is_string()) {
                    protocolName = pid.get<std::string>();
                }
            }
        } catch (...) {
            // Body unparseable — React copy degrades gracefully.
        }
    }

    std::string extraParams = "&kind=" + linkageKind;
    if (!verifier.empty())     extraParams += "&verifier=" + urlEncode(verifier);
    if (!protocolName.empty()) extraParams += "&protocol=" + urlEncode(protocolName);
    if (!keyId.empty())        extraParams += "&keyID=" + urlEncode(keyId);

    CefPostTask(TID_UI, new CreateNotificationOverlayTask("key_linkage_reveal", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("🛡️ key_linkage_reveal notification queued (requestId: " + requestId + ", kind=" + linkageKind + ")");
}

void openPaymentConfirmationModal(const ModalContext& ctx, const ResumeContext& resume,
                                   const std::string& extraParams) {
    LOG_DEBUG_HTTP("💰 Triggering payment_confirmation for " + ctx.domain);
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("payment_confirmation", ctx, resume));
    CefPostTask(TID_UI, new CreateNotificationOverlayTask("payment_confirmation", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("💰 payment_confirmation notification queued (requestId: " + requestId + ")");
}

void openRateLimitExceededModal(const ModalContext& ctx, const ResumeContext& resume,
                                 const std::string& extraParams) {
    LOG_DEBUG_HTTP("⏱️ Triggering rate_limit_exceeded for " + ctx.domain);
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("rate_limit_exceeded", ctx, resume));
    CefPostTask(TID_UI, new CreateNotificationOverlayTask("rate_limit_exceeded", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("⏱️ rate_limit_exceeded notification queued (requestId: " + requestId + ")");
}

void openProtocolPermissionPromptModal(const ModalContext& ctx, const ResumeContext& resume,
                                        const std::string& extraParams) {
    LOG_DEBUG_HTTP("🔒 Triggering protocol_permission_prompt for " + ctx.domain);
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("protocol_permission_prompt", ctx, resume));
    CefPostTask(TID_UI, new CreateNotificationOverlayTask("protocol_permission_prompt", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("🔒 protocol_permission_prompt notification queued (requestId: " + requestId + ")");
}

void openBasketPermissionPromptModal(const ModalContext& ctx, const ResumeContext& resume,
                                      const std::string& extraParams) {
    LOG_DEBUG_HTTP("🧺 Triggering basket_permission_prompt for " + ctx.domain);
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("basket_permission_prompt", ctx, resume));
    CefPostTask(TID_UI, new CreateNotificationOverlayTask("basket_permission_prompt", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("🧺 basket_permission_prompt notification queued (requestId: " + requestId + ")");
}

void openCounterpartyPermissionPromptModal(const ModalContext& ctx, const ResumeContext& resume,
                                            const std::string& extraParams) {
    LOG_DEBUG_HTTP("🤝 Triggering counterparty_permission_prompt for " + ctx.domain);
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("counterparty_permission_prompt", ctx, resume));
    CefPostTask(TID_UI, new CreateNotificationOverlayTask("counterparty_permission_prompt", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("🤝 counterparty_permission_prompt notification queued (requestId: " + requestId + ")");
}

void openCertificateDisclosureModal(const ModalContext& ctx, const ResumeContext& resume,
                                     const CertDisclosureInfo& info) {
    LOG_DEBUG_HTTP("📋 Triggering certificate_disclosure for " + ctx.domain
                   + " (" + std::to_string(info.fieldsToReveal.size()) + " fields)");

    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("certificate_disclosure", ctx, resume));

    std::string fieldsList;
    for (size_t i = 0; i < info.fieldsToReveal.size(); ++i) {
        if (i > 0) fieldsList += ",";
        fieldsList += info.fieldsToReveal[i];
    }

    std::string extraParams = "&fields=" + fieldsList;
    if (!info.certType.empty()) {
        extraParams += "&certType=" + urlEncode(info.certType);
    }
    if (!info.certifier.empty()) {
        extraParams += "&certifier=" + urlEncode(info.certifier);
    }

    CefPostTask(TID_UI, new CreateNotificationOverlayTask("certificate_disclosure", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("📋 certificate_disclosure notification queued (requestId: " + requestId
                   + ", fields: " + fieldsList + ")");
}

void OpenPromptModal(const std::string& promptType,
                     const ModalContext& ctx,
                     const ResumeContext& resume,
                     const std::string& extraParams) {
    if      (promptType == "domain_approval")               openDomainApprovalModal(ctx, resume);
    else if (promptType == "brc100_auth")                   openBRC100AuthApprovalModal(ctx, resume);
    else if (promptType == "identity_key_reveal")           openIdentityKeyRevealModal(ctx, resume);
    else if (promptType == "key_linkage_reveal")            openKeyLinkageRevealModal(ctx, resume);
    else if (promptType == "payment_confirmation")          openPaymentConfirmationModal(ctx, resume, extraParams);
    else if (promptType == "rate_limit_exceeded")           openRateLimitExceededModal(ctx, resume, extraParams);
    else if (promptType == "protocol_permission_prompt")    openProtocolPermissionPromptModal(ctx, resume, extraParams);
    else if (promptType == "basket_permission_prompt")      openBasketPermissionPromptModal(ctx, resume, extraParams);
    else if (promptType == "counterparty_permission_prompt") openCounterpartyPermissionPromptModal(ctx, resume, extraParams);
    else LOG_WARNING_HTTP("OpenPromptModal: unknown promptType '" + promptType + "' for " + ctx.domain);
    // Note: manifest_connect_bundle and certificate_disclosure are NOT in this
    // dispatcher — they require typed payloads (Manifest / CertDisclosureInfo).
    // Callers invoke their openers directly.
}

// Forward declared in HttpRequestInterceptor.h; implementation here since
// AsyncWalletResourceHandler and StartAsyncHTTPRequestTask are file-local
// to this translation unit. See header docstring for usage and rationale.
// Note: defined here forward-only because both classes are declared later
// in the file; we use a separate translation-unit-internal trampoline.


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

    // Trigger domain approval notification overlay.
    // Phase 2.5 Commit 6 sub-step 6.c — delegates to free-function opener
    // openDomainApprovalModal (Decision 3). Existing call sites unchanged.
    void triggerDomainApprovalModal(const std::string& domain, const std::string& method, const std::string& endpoint) {
        ResumeContext resume;
        resume.handler = this;
        openDomainApprovalModal(ModalContext{domain, method, endpoint, /*body=*/""}, resume);
    }


    // Trigger BRC-100 authentication approval notification overlay.
    // Phase 2.5 Commit 6 sub-step 6.c — delegates to openBRC100AuthApprovalModal.
    void triggerBRC100AuthApprovalModal(const std::string& domain, const std::string& method, const std::string& endpoint, const std::string& body, CefRefPtr<AsyncWalletResourceHandler> handler) {
        ResumeContext resume;
        resume.handler = handler;  // explicit handler arg, may differ from `this`
        openBRC100AuthApprovalModal(ModalContext{domain, method, endpoint, body}, resume);
    }

    // Phase 1.5 Step 5 — manifest-aware bundled connect prompt.
    // Fires when ManifestFetcher::Fetch returned a valid manifest with at
    // least one declared permission. Passes the entire manifest as a
    // URL-encoded JSON payload in extraParams so the React side can render
    // the bundled connect UX without re-fetching.
    //
    // Subsequent BRC-100 calls from the same fresh origin queue under the
    // same modal via PendingRequestManager — UNCHANGED from existing pattern.
    // Phase 2.5 Commit 6 sub-step 6.c — delegates to openManifestConnectBundleModal.
    void triggerManifestConnectBundleModal(const std::string& domain,
                                            const hodos::Manifest& m) {
        ResumeContext resume;
        resume.handler = this;
        openManifestConnectBundleModal(ModalContext{domain, method_, endpoint_, body_}, resume, m);
    }


    // Phase 1.5 Step 1 — privacy-perimeter prompt triggers (identity key + key linkage).
    // Mirror triggerCertificateDisclosureModal: store pending request keyed to this
    // handler, then post CreateNotificationOverlayTask with a new type string. The
    // shared notification_browser_ overlay multiplexes on `type` query param so no
    // new HWND / NSPanel creation paths are needed Win-side or Mac-side.

    // Phase 2.5 Commit 6 sub-step 6.c — delegates to openIdentityKeyRevealModal.
    void triggerIdentityKeyRevealModal(const std::string& domain) {
        ResumeContext resume;
        resume.handler = this;
        openIdentityKeyRevealModal(ModalContext{domain, method_, endpoint_, body_}, resume);
    }

    // Phase 2.5 Commit 6 sub-step 6.c — delegates to openKeyLinkageRevealModal.
    // Existing callers pass endpoint/body which equal this->endpoint_/this->body_
    // at every call site; threading the args through ModalContext preserves
    // that mapping verbatim.
    void triggerKeyLinkageRevealModal(const std::string& domain, const std::string& endpoint, const std::string& body) {
        ResumeContext resume;
        resume.handler = this;
        openKeyLinkageRevealModal(ModalContext{domain, method_, endpoint, body}, resume);
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

    // Phase 1.5 Step 1 — privacy-perimeter endpoint checks.
    static bool isGetPublicKeyEndpoint(const std::string& endpoint) {
        return endpoint.find("/getPublicKey") != std::string::npos;
    }

    static bool isKeyLinkageEndpoint(const std::string& endpoint) {
        return endpoint.find("/revealCounterpartyKeyLinkage") != std::string::npos
            || endpoint.find("/revealSpecificKeyLinkage") != std::string::npos;
    }

    // True iff the /getPublicKey body would route through the master-identity-key
    // codepath in handlers.rs (identityKey=true OR missing protocolID OR missing keyID).
    static bool isIdentityKeyStyleGetPublicKey(const std::string& body) {
        if (body.empty()) {
            return true; // empty body → identity key
        }
        try {
            auto json = nlohmann::json::parse(body);
            bool identityKeyFlag = json.contains("identityKey") && json["identityKey"].is_boolean() && json["identityKey"].get<bool>();
            bool hasProtocolId = json.contains("protocolID") && !json["protocolID"].is_null();
            bool hasKeyId = json.contains("keyID")
                          && json["keyID"].is_string()
                          && !json["keyID"].get<std::string>().empty();
            if (identityKeyFlag) return true;
            if (!hasProtocolId || !hasKeyId) return true;
            return false;
        } catch (...) {
            // If we can't parse, err on the safe side -- treat as identity-key request
            // so the prompt fires (Rust would reject malformed JSON anyway).
            return true;
        }
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

    // CertDisclosureInfo moved to file scope (above) in Phase 2.5 Commit 6
    // sub-step 6.c so the free-function modal opener can reference the type.
    // The static extractCertDisclosureInfo below continues to use the type
    // via the same name (now resolved at file scope).

    // Extract certificate disclosure info from proveCertificate request body
    static ::CertDisclosureInfo extractCertDisclosureInfo(const std::string& body) {
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
    // Phase 2.5 Commit 6 sub-step 6.c — delegates to openCertificateDisclosureModal.
    void triggerCertificateDisclosureModal(const std::string& domain, const CertDisclosureInfo& info) {
        ResumeContext resume;
        resume.handler = this;
        openCertificateDisclosureModal(ModalContext{domain, method_, endpoint_, body_}, resume, info);
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

    // Phase 1.5 Step 1 — privacy-perimeter pre-approval flags. Set in Open() when
    // the per-domain "Always allow" cache hit lets us skip the prompt; consumed in
    // startAsyncHTTPRequest() to inject the corresponding pre-approval header so
    // Rust's defense-in-depth gate passes through silently.
    bool identityKeyApproved_ = false;
    bool keyLinkageApproved_ = false;

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
        // Domain has no permission record — needs approval.
        //
        // Phase 1.5 Step 5: three-mode dispatch documented in Phase 1.5 README's
        // Step 5 section. Before firing the existing domain_approval modal, try
        // to fetch the dApp's /.well-known/wallet-manifest.json. If it succeeds
        // with declared permissions, fire the new manifest_connect_bundle modal
        // instead. If the fetch 404s, times out, or returns nothing actionable,
        // fall back to the existing flow with ZERO regression.
        //
        // PendingRequestManager queue interaction is UNCHANGED — concurrent calls
        // from the same fresh origin still stack under whichever modal fires.

        if (endpoint_.find("/brc100/auth/") != std::string::npos) {
            // BRC-100 auth handshake — manifest UX doesn't apply here.
            LOG_DEBUG_HTTP("🔐 BRC-100 auth request from unknown domain: " + requestDomain_);
            triggerBRC100AuthApprovalModal(requestDomain_, method_, endpoint_, body_, this);
        } else {
            LOG_DEBUG_HTTP("🔒 Domain " + requestDomain_ + " unknown — attempting manifest fetch first");
            hodos::Manifest manifest = hodos::ManifestFetcher::Fetch(requestDomain_);
            const bool hasDeclaredPerms = manifest.valid
                && (!manifest.protocols.empty()
                    || !manifest.baskets.empty()
                    || !manifest.certificates.empty()
                    || !manifest.counterparties.empty()
                    || manifest.spending.perTransactionUsd > 0);

            if (hasDeclaredPerms) {
                LOG_DEBUG_HTTP("📦 Manifest found for " + requestDomain_
                                + " — firing manifest_connect_bundle prompt");
                triggerManifestConnectBundleModal(requestDomain_, manifest);
            } else {
                // Mode 1 (404 / no permissions declared) or Mode 3 (timeout).
                // Both fall through to the existing pre-Step-5 flow.
                if (!manifest.valid) {
                    LOG_DEBUG_HTTP("✗ No manifest at " + requestDomain_
                                    + "/.well-known/wallet-manifest.json — using domain_approval");
                } else {
                    LOG_DEBUG_HTTP("✗ Manifest at " + requestDomain_
                                    + " declared no permissions — using domain_approval");
                }
                triggerDomainApprovalModal(requestDomain_, method_, endpoint_);
            }
        }

        postAuthTimeout(kPromptAuthTimeoutMs, "{\"error\":\"Approval timeout\",\"status\":\"error\"}");
        handle_request = true;
        return true;
    }

    // "approved" — auto-approve engine with spending limits and rate limiting
    if (perm.trustLevel == "approved") {
        LOG_DEBUG_HTTP("🔒 Domain " + requestDomain_ + " is approved, checking spending limits");

        // ====================================================================
        // Phase 1.5 Step 6 — Commit A: shadow-mode PermissionEngine logging.
        // ====================================================================
        // The engine runs alongside the inline gates and logs its decision so
        // we can verify agreement before any branch is migrated. Behavior is
        // unchanged — only logging. Inline gates below stay in full control.
        //
        // For Payment kind we precompute satoshis/cents so the engine sees the
        // same input the inline gates will. For non-Payment kinds requestedCents
        // stays 0; the engine ignores it for those branches.
        {
            const int browserIdShadow = browser_ ? browser_->GetIdentifier() : 0;
            int64_t shadowRequestedCents = 0;
            if (isPaymentEndpoint(endpoint_)) {
                const int64_t shadowSats = extractOutputSatoshis(body_);
                const double shadowPrice = BSVPriceCache::GetInstance().getPrice();
                if (shadowPrice > 0 && shadowSats > 0) {
                    shadowRequestedCents = static_cast<int64_t>(
                        (static_cast<double>(shadowSats) / 100000000.0) * shadowPrice * 100.0);
                }
            }
            const int64_t shadowSessionSpent =
                SessionManager::GetInstance().getSpentCents(browserIdShadow, requestDomain_);
            const int shadowRateCount =
                SessionManager::GetInstance().getRateCounter(browserIdShadow, requestDomain_);
            const int shadowPayCount =
                SessionManager::GetInstance().getPaymentCount(browserIdShadow, requestDomain_);

            hodos::PermissionContext shadowCtx = buildPermissionContext(
                requestDomain_, endpoint_, body_, perm,
                shadowRequestedCents, shadowSessionSpent,
                shadowRateCount, shadowPayCount);
            hodos::PermissionDecision shadowDecision =
                hodos::PermissionEngine::Decide(shadowCtx);

            LOG_INFO_HTTP(std::string("🧪 [engine-shadow] domain=") + requestDomain_
                + " endpoint=" + endpoint_
                + " callKind=" + callKindToString(shadowCtx.callKind)
                + " engineDecision=" + decisionKindToString(shadowDecision.kind)
                + " promptType=" + shadowDecision.promptType
                + " reason=" + shadowDecision.reason);
        }

        // Phase 2.5-B sub-step 5.c — identity-key reveal gate now flows
        // through the shared RunPermissionGate helper. Behavior is identical
        // to the prior Phase 1.5 Step 6 Commit C implementation; the
        // reorganization is prep work for Commit 6 (IPC bridge consuming
        // the same helper) per PHASE_2_5_IPC_REFACTOR.md.
        //
        // The C++ side still owns:
        //   1. The endpoint+body classification (the outer if-check
        //      narrows us to identity-key-style /getPublicKey).
        //   2. Modal dispatch via triggerIdentityKeyRevealModal +
        //      postAuthTimeout (wired into the openModal callback).
        //   3. Auto-approve fast path: identityKeyApproved_ +
        //      StartAsyncHTTPRequestTask (wired into the forwardToWallet
        //      callback). identityKeyApproved_ is the header-injection
        //      flag StartAsyncHTTPRequestTask reads to add the
        //      X-Identity-Key-Approved header on the wallet request,
        //      which Rust's get_public_key handler requires (Phase 1.5
        //      Step 1).
        // The engine owns Silent / Deny / Prompt based on the persistent
        // V17 column (perm.identityKeyDisclosureAllowed) and the in-memory
        // session opt-in cache (IdentityKeyApprovalCache), both of which
        // buildPermissionContext populates.
        if (isGetPublicKeyEndpoint(endpoint_) && isIdentityKeyStyleGetPublicKey(body_)) {
            // Payment fields unused for identity-key reveal; pass 0s. The
            // engine never reads them when callKind != Payment.
            hodos::PermissionContext ctx = buildPermissionContext(
                requestDomain_, endpoint_, body_, perm,
                /*requestedCents=*/0, /*sessionSpentCents=*/0,
                /*paymentRequestsThisMinute=*/0, /*paymentCountThisSession=*/0,
                /*bsvPriceAvailable=*/true);

            hodos::GateCallbacks cb;
            cb.forwardToWallet = [this, &handle_request]() {
                LOG_DEBUG_HTTP("🛡️ identity-key reveal silently approved for " + requestDomain_);
                identityKeyApproved_ = true;
                handle_request = true;
                CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(this));
            };
            // Defensive: DecidePrivacyPerimeter does not return Deny for
            // IdentityKeyReveal today, but if a future engine version does
            // (e.g. a blocked-identity-key list), surface the reason
            // cleanly instead of falling through to a prompt.
            cb.denyWithError = [this, &handle_request](const std::string& errorJson) {
                LOG_DEBUG_HTTP("🛡️ identity-key reveal denied for " + requestDomain_);
                onHTTPResponseReceived(errorJson);
                handle_request = true;
            };
            cb.openModal = [this, &handle_request](
                const std::string& /*promptType*/,
                const std::string& /*emptyExtraParams*/) {
                LOG_DEBUG_HTTP("🛡️ identity-key reveal prompt required for " + requestDomain_);
                triggerIdentityKeyRevealModal(requestDomain_);
                postAuthTimeout(kPromptAuthTimeoutMs,
                    "{\"error\":\"identity_key_reveal timeout\",\"status\":\"error\"}");
                handle_request = true;
            };

            hodos::GateDecision gateResult = hodos::RunPermissionGate(ctx, cb);

            LOG_DEBUG_HTTP(std::string("🛡️ Identity-key engine decision: ")
                + (gateResult.action == hodos::GateDecision::Action::Silent ? "Silent"
                   : gateResult.action == hodos::GateDecision::Action::Prompt ? "Prompt" : "Deny")
                + " promptType=" + gateResult.promptType
                + " | persistent_grant=" + (perm.identityKeyDisclosureAllowed ? "1" : "0")
                + " session_optin=" + (IdentityKeyApprovalCache::GetInstance().isApproved(requestDomain_) ? "1" : "0")
                + " | reason=" + gateResult.reason);

            return true;
        }

        // Phase 2.5-B sub-step 5.d — key-linkage reveal gate now flows
        // through the shared RunPermissionGate helper. Behavior is identical
        // to the prior Phase 1.5 Step 6 Commit D implementation; the
        // reorganization mirrors 5.c's identity-key migration.
        //
        // /revealCounterpartyKeyLinkage and /revealSpecificKeyLinkage are
        // always privacy-perimeter: never silent-pass without the cached
        // session opt-in (BRC-72 has no equivalent of identity-key's
        // persistent V17 column today — only the in-memory opt-in cache).
        // buildPermissionContext populates ctx.keyLinkageSessionOptIn from
        // KeyLinkageApprovalCache; the engine's DecidePrivacyPerimeter
        // branch for CounterpartyKeyLinkage/SpecificKeyLinkage covers
        // Silent vs Prompt based on that single field.
        //
        // keyLinkageApproved_ is the header-injection flag StartAsyncHTTPRequestTask
        // reads to add X-Key-Linkage-Approved on the wallet request, which
        // Rust's reveal_*_key_linkage handlers require (Phase 1.5 Step 1
        // defense-in-depth).
        if (isKeyLinkageEndpoint(endpoint_)) {
            hodos::PermissionContext ctx = buildPermissionContext(
                requestDomain_, endpoint_, body_, perm,
                /*requestedCents=*/0, /*sessionSpentCents=*/0,
                /*paymentRequestsThisMinute=*/0, /*paymentCountThisSession=*/0,
                /*bsvPriceAvailable=*/true);

            hodos::GateCallbacks cb;
            cb.forwardToWallet = [this, &handle_request]() {
                LOG_DEBUG_HTTP("🛡️ key-linkage reveal silently approved for " + requestDomain_);
                keyLinkageApproved_ = true;
                handle_request = true;
                CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(this));
            };
            // Defensive: DecidePrivacyPerimeter does not return Deny for
            // key-linkage kinds today. Surfacing the reason here keeps
            // future engine extensions (e.g. blocked-verifier list)
            // self-consistent rather than collapsing to a prompt.
            cb.denyWithError = [this, &handle_request](const std::string& errorJson) {
                LOG_DEBUG_HTTP("🛡️ key-linkage reveal denied for " + requestDomain_);
                onHTTPResponseReceived(errorJson);
                handle_request = true;
            };
            // Prompt branch passes endpoint + body through so the modal can
            // surface which linkage kind (counterparty vs specific) and the
            // verifier identity the page is asking about. The trigger fn
            // captures requestDomain_, endpoint_, body_ via this closure.
            cb.openModal = [this, &handle_request](
                const std::string& /*promptType*/,
                const std::string& /*emptyExtraParams*/) {
                LOG_DEBUG_HTTP("🛡️ key-linkage reveal prompt required for " + requestDomain_);
                triggerKeyLinkageRevealModal(requestDomain_, endpoint_, body_);
                postAuthTimeout(kPromptAuthTimeoutMs,
                    "{\"error\":\"key_linkage_reveal timeout\",\"status\":\"error\"}");
                handle_request = true;
            };

            hodos::GateDecision gateResult = hodos::RunPermissionGate(ctx, cb);

            LOG_DEBUG_HTTP(std::string("🛡️ Key-linkage engine decision: ")
                + (gateResult.action == hodos::GateDecision::Action::Silent ? "Silent"
                   : gateResult.action == hodos::GateDecision::Action::Prompt ? "Prompt" : "Deny")
                + " promptType=" + gateResult.promptType
                + " | session_optin=" + (KeyLinkageApprovalCache::GetInstance().isApproved(requestDomain_) ? "1" : "0")
                + " | reason=" + gateResult.reason);

            return true;
        }

        // Phase 2.5-B sub-step 5.f — cert-disclosure gate now flows through
        // the shared RunPermissionGate helper. This sub-step ALSO wires the
        // gate into PermissionEngine::Decide() for the first time: prior to
        // 5.f the inline cascade did its own DB lookup + dispatch and never
        // called the engine, even though the engine's CertificateDisclosure
        // branch (PermissionEngine.cpp:247-263) has been ready to consume
        // ctx.scopedGrantExists as "all requested fields pre-approved" since
        // Phase 1.5 Step 3. Net behavior unchanged — just routed properly.
        //
        // classifyCallKind sets ctx.callKind to CertificateDisclosure for
        // /proveCertificate. buildPermissionContext leaves scopedGrantExists
        // default-false for cert kind (no SubPermissionCache lookup applies),
        // so we compute it here from fetchCertFieldsFromBackend + the
        // requested-vs-approved field set check and override the field
        // BEFORE calling RunPermissionGate.
        //
        // Known gap not addressed in 5.f: classifyCallKind does NOT detect
        // sensitive fields (email, dob, etc.) and route them to
        // SensitiveCertField. The engine's DecidePrivacyPerimeter branch
        // for SensitiveCertField is wired but unreachable today via the
        // classifier. Flagged as a separate follow-up gap; tracked in
        // memory project_sensitive_cert_field_classifier_gap.
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

                hodos::PermissionContext ctx = buildPermissionContext(
                    requestDomain_, endpoint_, body_, perm,
                    /*requestedCents=*/0, /*sessionSpentCents=*/0,
                    /*paymentRequestsThisMinute=*/0, /*paymentCountThisSession=*/0,
                    /*bsvPriceAvailable=*/true);
                // Engine's CertificateDisclosure branch reads scopedGrantExists
                // as "all requested fields pre-approved" → Silent; else
                // Prompt(certificate_disclosure). Caller-overridden because
                // buildPermissionContext doesn't query the cert table.
                ctx.scopedGrantExists = allApproved;

                hodos::GateCallbacks cb;
                cb.forwardToWallet = [this, &handle_request]() {
                    LOG_DEBUG_HTTP("📋 All cert fields already approved for " + requestDomain_ + ", auto-forwarding");
                    handle_request = true;
                    CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(this));
                };
                // Defensive: DecideCertificateDisclosure does not return Deny
                // today, but kept for future engine extensions (e.g.
                // blocked-certifier list, revoked-cert detection).
                cb.denyWithError = [this, &handle_request](const std::string& errorJson) {
                    LOG_DEBUG_HTTP("📋 cert disclosure denied for " + requestDomain_);
                    onHTTPResponseReceived(errorJson);
                    handle_request = true;
                };
                cb.openModal = [this, certInfo, &handle_request](
                    const std::string& /*promptType*/,
                    const std::string& /*emptyExtraParams*/) {
                    LOG_DEBUG_HTTP("📋 Unapproved cert fields — showing disclosure notification for " + requestDomain_);
                    triggerCertificateDisclosureModal(requestDomain_, certInfo);
                    postAuthTimeout(kPromptAuthTimeoutMs,
                        "{\"error\":\"Certificate disclosure timeout\",\"status\":\"error\"}");
                    handle_request = true;
                };

                hodos::GateDecision gateResult = hodos::RunPermissionGate(ctx, cb);

                LOG_DEBUG_HTTP(std::string("📋 Cert-disclosure engine decision: ")
                    + (gateResult.action == hodos::GateDecision::Action::Silent ? "Silent"
                       : gateResult.action == hodos::GateDecision::Action::Prompt ? "Prompt" : "Deny")
                    + " | all_approved=" + (allApproved ? "1" : "0")
                    + " | requested_fields=" + std::to_string(certInfo.fieldsToReveal.size())
                    + " | reason=" + gateResult.reason);

                return true;
            }
            // Invalid or empty fields — fall through to normal forwarding
            LOG_DEBUG_HTTP("📋 proveCertificate with no/empty fields — forwarding directly");
        }

        // Phase 2.5-B sub-step 5.e — scoped-grant gate for non-payment,
        // non-perimeter, non-cert endpoints now flows through the shared
        // RunPermissionGate helper. Behavior is identical to the prior
        // Phase 1.5 Step 6 Commit E implementation; the reorganization
        // mirrors 5.b-5.d.
        //
        // Targets endpoints like /createSignature, /createHmac, /encrypt,
        // /decrypt, /listOutputs that carry a protocolID / basket /
        // counterparty body field. The engine's DecideScopedGrant returns
        // Silent when a matching V18 grant or one-shot approval exists,
        // else Prompt with the appropriate promptType
        // (protocol_permission_prompt / basket_permission_prompt /
        // counterparty_permission_prompt).
        //
        // Important behavior preserved: Silent falls THROUGH to the
        // catch-all "Non-payment endpoint from approved domain — forward
        // immediately" forwarding below, NOT through forwardToWallet here.
        // The callback is intentionally left unset for Silent; that path
        // takes no action so the outer code's catch-all forward runs.
        // Prompt and Deny return true after the matching callback fires.
        //
        // Payment endpoints (createAction etc.) are handled below by the
        // existing payment branch, which also surfaces missing-scope
        // prompts via PermissionContext.paymentScopeKindMissing before the
        // cap check fires (Commit E v1 deferred Payment-scope sequencing).
        if (!isPaymentEndpoint(endpoint_)
            && !isGetPublicKeyEndpoint(endpoint_)
            && !isKeyLinkageEndpoint(endpoint_)
            && !isProveCertificateEndpoint(endpoint_)) {
            ProtocolScope proto_scope = extractProtocolScope(endpoint_, body_);
            BasketScope basket_scope = extractBasketScope(endpoint_, body_);
            const bool hasScope = proto_scope.valid || basket_scope.valid;
            if (hasScope) {
                hodos::PermissionContext ctx = buildPermissionContext(
                    requestDomain_, endpoint_, body_, perm,
                    /*requestedCents=*/0, /*sessionSpentCents=*/0,
                    /*paymentRequestsThisMinute=*/0, /*paymentCountThisSession=*/0,
                    /*bsvPriceAvailable=*/true);

                hodos::GateCallbacks cb;
                // Silent case: forwardToWallet intentionally LEFT UNSET so
                // RunPermissionGate's Silent branch is a no-op; the outer
                // code's catch-all forwarding below handles the request.
                // This preserves the prior "Silent — fall through to normal
                // forwarding below" comment's semantics exactly.
                cb.denyWithError = [this, &handle_request](const std::string& errorJson) {
                    LOG_DEBUG_HTTP("🛡️ Scoped grant denied for " + requestDomain_);
                    onHTTPResponseReceived(errorJson);
                    handle_request = true;
                };
                cb.openModal = [this, &proto_scope, &basket_scope, &handle_request](
                    const std::string& promptType,
                    const std::string& /*emptyExtraParams*/) {
                    // Build extraParams so the React modal can render the
                    // specific scope being requested. The scope fields are
                    // distinct per kind; React reads only the ones it needs
                    // for the current promptType.
                    std::string extraParams;
                    if (promptType == "protocol_permission_prompt") {
                        // Level/keyId/counterparty default-safe if unset.
                        extraParams = "&protocolLevel=" + std::to_string(proto_scope.level)
                                    + "&protocolName=" + proto_scope.name
                                    + "&protocolKeyId=" + proto_scope.keyId
                                    + "&protocolCounterparty=" + proto_scope.counterparty;
                    } else if (promptType == "basket_permission_prompt") {
                        extraParams = "&basket=" + basket_scope.basket
                                    + "&basketAccess=" + basket_scope.requiredAccess;
                    } else if (promptType == "counterparty_permission_prompt") {
                        extraParams = "&counterparty=" + proto_scope.counterparty;
                    }
                    LOG_DEBUG_HTTP("🛡️ Scoped grant prompt required for " + requestDomain_
                                   + " (" + promptType + ")");
                    PendingRequestManager::GetInstance().addRequest(
                        requestDomain_, method_, endpoint_, body_, this, promptType);
                    CefPostTask(TID_UI, new CreateNotificationOverlayTask(
                        promptType, requestDomain_, extraParams));
                    postAuthTimeout(kPromptAuthTimeoutMs,
                        "{\"error\":\"scoped permission timeout\",\"status\":\"error\"}");
                    handle_request = true;
                };

                hodos::GateDecision gateResult = hodos::RunPermissionGate(ctx, cb);

                LOG_DEBUG_HTTP(std::string("🛡️ Scoped-grant engine decision: ")
                    + (gateResult.action == hodos::GateDecision::Action::Silent ? "Silent"
                       : gateResult.action == hodos::GateDecision::Action::Prompt ? "Prompt" : "Deny")
                    + " promptType=" + gateResult.promptType
                    + " callKind=" + callKindToString(ctx.callKind)
                    + " | reason=" + gateResult.reason);

                // Prompt and Deny set handle_request inside their callbacks
                // and require return-true so the catch-all forward below
                // doesn't fire. Silent intentionally falls through.
                if (gateResult.action != hodos::GateDecision::Action::Silent) {
                    return true;
                }
                // Silent — fall through to normal forwarding below.
            }
        }

        if (isPaymentEndpoint(endpoint_)) {
            // Phase 2.5-B sub-step 5.b — payment gate now flows through the
            // shared RunPermissionGate helper. Behavior is identical to the
            // Phase 1.5 Step 6 Commit B implementation; the reorganization
            // is the prep work for Commit 6 (IPC bridge consuming the same
            // helper) per development-docs/Sigma-BRC121-Sprint/
            // phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md.
            //
            // The C++ side still owns the same four concerns:
            //   1. State collection (satoshis/cents/sessionSpent/etc.)
            //   2. extraParams string the React modal expects
            //   3. PendingRequestManager queue + CreateNotificationOverlayTask
            //   4. Session-counter increments on auto-approve
            // Those concerns are wired into the GateCallbacks below; the gate
            // helper picks the matching callback based on the engine's
            // decision.
            int browserId = browser_ ? browser_->GetIdentifier() : 0;
            SessionManager::GetInstance().getSession(browserId, requestDomain_);

            int64_t satoshis = extractOutputSatoshis(body_);
            double bsvPrice = BSVPriceCache::GetInstance().getPrice();
            const bool priceAvailable = (bsvPrice > 0);
            int64_t cents = 0;
            if (priceAvailable && satoshis > 0) {
                cents = static_cast<int64_t>((static_cast<double>(satoshis) / 100000000.0) * bsvPrice * 100.0);
            }
            const int64_t sessionSpent = SessionManager::GetInstance().getSpentCents(browserId, requestDomain_);
            const int rateCount = SessionManager::GetInstance().getRateCounter(browserId, requestDomain_);
            const int txCount = SessionManager::GetInstance().getPaymentCount(browserId, requestDomain_);

            hodos::PermissionContext ctx = buildPermissionContext(
                requestDomain_, endpoint_, body_, perm,
                cents, sessionSpent, rateCount, txCount,
                priceAvailable);

            // Silent path — bypass any modal, mark this as an auto-approved
            // payment so AsyncHTTPClient::OnRequestComplete fires the
            // payment_success_indicator IPC (green-dot animation) on the
            // wallet's response, then forward to the wallet.
            hodos::GateCallbacks cb;
            cb.forwardToWallet =
                [this, cents, browserId, &handle_request]() {
                LOG_DEBUG_HTTP("💰 Auto-approved payment for " + requestDomain_);
                preCalculatedCents_ = cents;
                wasAutoApprovedPayment_ = true;
                SessionManager::GetInstance().incrementRateCounter(browserId);
                SessionManager::GetInstance().incrementPaymentCount(browserId);
                handle_request = true;
                CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(this));
            };

            // Deny path — surface the engine-formatted error envelope.
            cb.denyWithError =
                [this, &handle_request](const std::string& errorJson) {
                LOG_DEBUG_HTTP("💰 Engine denied payment for " + requestDomain_);
                onHTTPResponseReceived(errorJson);
                handle_request = true;
            };

            // Prompt path — build branch-specific extraParams from the
            // promptType, register with PendingRequestManager, fire the
            // notification overlay task, arm the auth timeout. The two
            // sub-cases:
            //   (a) Scope-permission prompt (Commit E v1 path, dormant in
            //       production today because buildPermissionContext does
            //       NOT populate paymentScopeKindMissing for Payment kind —
            //       see PermissionEngine context builder comment).
            //   (b) Standard payment / rate-limit prompt with the
            //       exceededLimit string the React modal renders.
            cb.openModal =
                [this, satoshis, cents, bsvPrice, priceAvailable, rateCount,
                 txCount, sessionSpent, &perm, &handle_request](
                    const std::string& promptType,
                    const std::string& /*emptyExtraParams*/) {
                preCalculatedCents_ = cents;

                std::string extraParams;
                if (promptType == "protocol_permission_prompt"
                    || promptType == "basket_permission_prompt"
                    || promptType == "counterparty_permission_prompt") {
                    ProtocolScope proto_scope_pay = extractProtocolScope(endpoint_, body_);
                    BasketScope basket_scope_pay = extractBasketScope(endpoint_, body_);
                    if (promptType == "protocol_permission_prompt") {
                        extraParams = "&protocolLevel=" + std::to_string(proto_scope_pay.level)
                                    + "&protocolName=" + proto_scope_pay.name
                                    + "&protocolKeyId=" + proto_scope_pay.keyId
                                    + "&protocolCounterparty=" + proto_scope_pay.counterparty
                                    + "&satoshis=" + std::to_string(satoshis)
                                    + "&cents=" + std::to_string(cents);
                    } else if (promptType == "basket_permission_prompt") {
                        extraParams = "&basket=" + basket_scope_pay.basket
                                    + "&basketAccess=" + basket_scope_pay.requiredAccess
                                    + "&satoshis=" + std::to_string(satoshis)
                                    + "&cents=" + std::to_string(cents);
                    } else {
                        extraParams = "&counterparty=" + proto_scope_pay.counterparty
                                    + "&satoshis=" + std::to_string(satoshis)
                                    + "&cents=" + std::to_string(cents);
                    }
                    LOG_DEBUG_HTTP("🛡️ Payment scope prompt required for " + requestDomain_
                                   + " (" + promptType + ")");
                    PendingRequestManager::GetInstance().addRequest(
                        requestDomain_, method_, endpoint_, body_, this, promptType);
                    CefPostTask(TID_UI, new CreateNotificationOverlayTask(
                        promptType, requestDomain_, extraParams));
                    postAuthTimeout(kPromptAuthTimeoutMs,
                        "{\"error\":\"scoped permission timeout\",\"status\":\"error\"}");
                    handle_request = true;
                    return;
                }

                // Standard payment / rate-limit prompt path.
                std::string exceeded;
                if (!priceAvailable) {
                    exceeded = "price_unavailable";
                } else if (rateCount >= perm.rateLimitPerMin && perm.rateLimitPerMin > 0) {
                    exceeded = "rate_limit";
                } else if (txCount >= perm.maxTxPerSession && perm.maxTxPerSession > 0) {
                    exceeded = "session_tx_count";
                } else {
                    const bool overTx = cents > perm.perTxLimitCents;
                    const bool overSession = (sessionSpent + cents) > perm.perSessionLimitCents;
                    if (overTx && overSession) exceeded = "both";
                    else if (overTx) exceeded = "per_tx";
                    else exceeded = "per_session";
                }

                extraParams = "&satoshis=" + std::to_string(satoshis)
                            + "&cents=" + std::to_string(cents)
                            + "&bsvPrice=" + (priceAvailable ? std::to_string(bsvPrice) : std::string("0"))
                            + "&exceededLimit=" + exceeded
                            + "&perTxLimit=" + std::to_string(perm.perTxLimitCents)
                            + "&perSessionLimit=" + std::to_string(perm.perSessionLimitCents)
                            + "&sessionSpent=" + std::to_string(sessionSpent);
                if (promptType == "rate_limit_exceeded") {
                    extraParams += "&rateLimit=" + std::to_string(perm.rateLimitPerMin)
                                 + "&maxTxPerSession=" + std::to_string(perm.maxTxPerSession)
                                 + "&txCount=" + std::to_string(txCount);
                }

                PendingRequestManager::GetInstance().addRequest(
                    requestDomain_, method_, endpoint_, body_, this, promptType);
                CefPostTask(TID_UI, new CreateNotificationOverlayTask(
                    promptType, requestDomain_, extraParams));
                postAuthTimeout(kPromptAuthTimeoutMs,
                    "{\"error\":\"Payment confirmation timeout\",\"status\":\"error\"}");
                handle_request = true;
            };

            hodos::GateDecision gateResult = hodos::RunPermissionGate(ctx, cb);

            LOG_DEBUG_HTTP(std::string("💰 Payment engine decision: ")
                + (gateResult.action == hodos::GateDecision::Action::Silent ? "Silent"
                   : gateResult.action == hodos::GateDecision::Action::Prompt ? "Prompt" : "Deny")
                + " promptType=" + gateResult.promptType
                + " | " + std::to_string(satoshis) + " sats = " + std::to_string(cents) + " cents"
                + " tx_limit=" + std::to_string(perm.perTxLimitCents)
                + " session_spent=" + std::to_string(sessionSpent)
                + " session_limit=" + std::to_string(perm.perSessionLimitCents)
                + " tx_count=" + std::to_string(txCount)
                + " max_tx=" + std::to_string(perm.maxTxPerSession)
                + " | reason=" + gateResult.reason);

            return true;
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
    postAuthTimeout(kPromptAuthTimeoutMs, "{\"error\":\"Approval timeout\",\"status\":\"error\"}");
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
    // CEF outer safety net. Primary timeouts live inside the wallet per call-class
    // (Phase 1.6 Services facade tiers). This cap exists so a truly hung wallet
    // (deadlock / panic) cannot freeze the page indefinitely — it is NOT the
    // primary timeout for any call.
    //
    // Default short enough to surface true hangs quickly.
    int timeoutMs = 45000;

    // Endpoints with legitimate slow paths must cap above the wallet's internal
    // max so the wallet's structured response (success or proper error) always
    // wins over this outer fallback. Otherwise CEF chops the call, sends back
    // {"error":"Wallet request timeout","status":"error"} as HTTP 200, and the
    // page treats the timeout body as the response — silent corruption.
    //
    //   /wallet/recover      — gap-limit address scan over WoC (60–120s)
    //   /acquireCertificate  — BRC-53 handshake + third-party /signCertificate
    //                          (certifier server, observed 100–120s on degraded
    //                          days; wallet caps internally at 240s via
    //                          CallClass::ThirdPartyNoFallback). 300s outer
    //                          gives 60s buffer above the wallet cap so wallet's
    //                          structured response always wins.
    //   /proveCertificate    — per-field decrypt + re-encrypt for verifier;
    //                          paired with /acquireCertificate flows
    //
    // TODO (1.6d.D follow-up): replace this CEF-side endpoint switch with the
    // per-call-class timeout tier policy (8s indexer-with-fallback / 15s
    // indexer-single-shot / 240s third-party / 120s long-scan) living in
    // WalletServices. CEF then keeps one or two outer caps as a safety net only.
    if (endpoint_.find("/wallet/recover")     != std::string::npos) {
        timeoutMs = 120000;
    }
    if (endpoint_.find("/acquireCertificate") != std::string::npos ||
        endpoint_.find("/proveCertificate")   != std::string::npos) {
        timeoutMs = 300000;
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
    DomainPermissionTask(const std::string& domain, bool identityKeyDisclosureAllowed = false)
        : domain_(domain), identityKeyDisclosureAllowed_(identityKeyDisclosureAllowed) {}

    void Execute() override {
        LOG_DEBUG_HTTP("🔐 DomainPermissionTask executing on UI thread for domain: " + domain_);

        // Create request
        CefRefPtr<CefRequest> cefRequest = CefRequest::Create();
        cefRequest->SetURL("http://localhost:31301/domain/permissions");
        cefRequest->SetMethod("POST");
        cefRequest->SetHeaderByName("Content-Type", "application/json", true);

        // Create JSON body — Phase 1.5 Step 1 adds identityKeyDisclosureAllowed
        nlohmann::json bodyJson;
        bodyJson["domain"] = domain_;
        bodyJson["trustLevel"] = "approved";
        bodyJson["identityKeyDisclosureAllowed"] = identityKeyDisclosureAllowed_;
        std::string jsonBody = bodyJson.dump();
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
    bool identityKeyDisclosureAllowed_;
    IMPLEMENT_REFCOUNTING(DomainPermissionTask);
    DISALLOW_COPY_AND_ASSIGN(DomainPermissionTask);
};

// Task class for creating domain permission with advanced settings (custom limits)
class AdvancedDomainPermissionTask : public CefTask {
public:
    AdvancedDomainPermissionTask(const std::string& domain, int64_t perTxLimitCents,
                                  int64_t perSessionLimitCents, int64_t rateLimitPerMin,
                                  int64_t maxTxPerSession,
                                  bool identityKeyDisclosureAllowed = false)
        : domain_(domain), perTxLimitCents_(perTxLimitCents),
          perSessionLimitCents_(perSessionLimitCents), rateLimitPerMin_(rateLimitPerMin),
          maxTxPerSession_(maxTxPerSession),
          identityKeyDisclosureAllowed_(identityKeyDisclosureAllowed) {}

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
        body["identityKeyDisclosureAllowed"] = identityKeyDisclosureAllowed_;
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
    bool identityKeyDisclosureAllowed_;
    IMPLEMENT_REFCOUNTING(AdvancedDomainPermissionTask);
    DISALLOW_COPY_AND_ASSIGN(AdvancedDomainPermissionTask);
};

// Function to add domain permission with advanced settings.
// Phase 1.5 Step 1: identityKeyDisclosureAllowed bundles the privacy-perimeter
// grant into the same site approval, eliminating a second prompt on first connect.
void addDomainPermissionAdvanced(const std::string& domain, int64_t perTxLimitCents,
                                  int64_t perSessionLimitCents, int64_t rateLimitPerMin,
                                  int64_t maxTxPerSession,
                                  bool identityKeyDisclosureAllowed) {
    LOG_DEBUG_HTTP("🔐 Adding advanced domain permission: " + domain +
        " (tx=" + std::to_string(perTxLimitCents) + ", session=" + std::to_string(perSessionLimitCents) +
        ", rate=" + std::to_string(rateLimitPerMin) + ", maxTxPerSession=" + std::to_string(maxTxPerSession) +
        ", identityKeyDisclosure=" + (identityKeyDisclosureAllowed ? "1" : "0") + ")");

    // Set cache immediately with full settings — synchronous, so the next request
    // from this domain sees both "approved" trust AND the identity-key grant
    // before the async DB write lands.
    DomainPermissionCache::Permission perm;
    perm.trustLevel = "approved";
    perm.perTxLimitCents = perTxLimitCents;
    perm.perSessionLimitCents = perSessionLimitCents;
    perm.rateLimitPerMin = rateLimitPerMin;
    perm.maxTxPerSession = maxTxPerSession;
    perm.identityKeyDisclosureAllowed = identityKeyDisclosureAllowed;
    DomainPermissionCache::GetInstance().set(domain, perm);

    // Also mirror into the session-level in-memory cache so drain-forwarded
    // sibling requests (which bypass Open()) get the X-Identity-Key-Approved
    // header injected by startAsyncHTTPRequest while the Rust DB write is in flight.
    if (identityKeyDisclosureAllowed) {
        IdentityKeyApprovalCache::GetInstance().approve(domain);
    }

    // Post async DB write
    CefPostTask(TID_UI, new AdvancedDomainPermissionTask(
        domain, perTxLimitCents, perSessionLimitCents, rateLimitPerMin,
        maxTxPerSession, identityKeyDisclosureAllowed));
}

// Phase 1.5 Step 1 — defined here (after AsyncWalletResourceHandler +
// StartAsyncHTTPRequestTask are visible) so simple_handler.cpp can forward
// pending wallet requests without needing the file-local class definitions.
bool ForwardPendingWalletRequest(CefRefPtr<CefResourceHandler> handler) {
    if (!handler) return false;
    AsyncWalletResourceHandler* walletHandler =
        static_cast<AsyncWalletResourceHandler*>(handler.get());
    CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(walletHandler));
    return true;
}

// Function to add domain permission (sets "approved" trust level).
// Phase 1.5 Step 1: bundles the identity-key privacy-perimeter grant via the
// optional second arg so the simple-approval modal can ship both grants in one click.
void addDomainPermission(const std::string& domain, bool identityKeyDisclosureAllowed) {
    LOG_DEBUG_HTTP("🔐 Adding domain permission: " + domain +
        " (identityKeyDisclosure=" + (identityKeyDisclosureAllowed ? "1" : "0") + ")");

    // Set the cache immediately so the next request sees "approved" without waiting
    // for the async DB write to complete (prevents modal loop race condition)
    DomainPermissionCache::Permission perm;
    perm.trustLevel = "approved";
    perm.identityKeyDisclosureAllowed = identityKeyDisclosureAllowed;
    DomainPermissionCache::GetInstance().set(domain, perm);

    if (identityKeyDisclosureAllowed) {
        IdentityKeyApprovalCache::GetInstance().approve(domain);
    }

    // Post task to UI thread for the async DB write
    CefPostTask(TID_UI, new DomainPermissionTask(domain, identityKeyDisclosureAllowed));
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

// ============================================================================
// Phase 1.5 — session-scoped trust caches: domain-revoke helpers
// ============================================================================
// IdentityKeyApprovalCache and KeyLinkageApprovalCache are in-memory caches
// populated when the user clicks "Approve with Always allow" on the privacy-
// perimeter prompts. They were designed as a fast-path while the V17 column /
// future linkage column DB write is in flight. SubPermissionCache (Commit A)
// memoizes V18 child-table lookups.
//
// Without these helpers, when the user toggles identity-key disclosure off in
// DomainPermissionsTab or DomainPermissionForm, only the V17 column flips —
// the session cache still says approved, so the inline gate's
// `persistent_grant || cache_grant` test silently leaks identity keys until
// the browser is restarted.
//
// Called from simple_handler.cpp's `domain_permission_invalidate` IPC, which
// fires every time a domain's permissions change (revoke, edit, advanced
// save, etc.). Dropping all session-scoped trust on the invalidate edge is
// the right default — the user just touched this domain's permissions, so
// any prior session-only opt-in should be re-confirmed.
void revokeIdentityKeyApprovalForDomain(const std::string& domain) {
    IdentityKeyApprovalCache::GetInstance().revoke(domain);
}
void revokeKeyLinkageApprovalForDomain(const std::string& domain) {
    KeyLinkageApprovalCache::GetInstance().revoke(domain);
}
void invalidateSubPermissionCacheForDomain(const std::string& domain) {
    SubPermissionCache::GetInstance().invalidate(domain);
}
void clearSubPermissionCache() {
    SubPermissionCache::GetInstance().clear();
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

        // Phase 2.5 Commit 6 sub-step 6.b — record spending + fire green-dot
        // animation via the shared OnWalletCallSuccess helper. The error-check
        // stays at the caller because OnWalletCallSuccess doesn't have access
        // to the response body; the wasAutoApprovedPayment flag is gated to
        // false on an error response so the helper's internal guard does the
        // rest.
        if (parent_) {
            CefURLRequest::Status status = request->GetRequestStatus();
            int64_t cents = parent_->getPreCalculatedCents();
            const bool wasAutoApprovedPayment = parent_->getWasAutoApprovedPayment();
            bool successAndNotError = false;
            if (status == UR_SUCCESS && wasAutoApprovedPayment) {
                bool isError = false;
                try {
                    auto rj = nlohmann::json::parse(responseData_);
                    isError = rj.contains("error");
                } catch (...) {}
                successAndNotError = !isError;
            }
            OnWalletCallSuccess(parent_->getBrowserId(),
                                parent_->getRequestDomain(),
                                cents,
                                /*wasAutoApprovedPayment=*/successAndNotError,
                                /*endpoint=*/"createAction");
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

    // Phase 1.5 Step 1 — privacy-perimeter safety net for drain-forwarded
    // siblings (and any other code path that lands here without going through
    // Open()'s gate). If this is an identity-key-style /getPublicKey for an
    // external domain that has NEITHER the persistent grant nor the in-memory
    // cache hit, we must trigger the privacy-perimeter prompt -- forwarding
    // the bare request to Rust would just return identity_key_prompt_required
    // and break the page. The "unchecked bundle checkbox" flow lands here.
    if (!requestDomain_.empty()
        && isGetPublicKeyEndpoint(endpoint_)
        && isIdentityKeyStyleGetPublicKey(body_)
        && !identityKeyApproved_) {
        bool persistent_grant =
            DomainPermissionCache::GetInstance()
                .getPermission(requestDomain_)
                .identityKeyDisclosureAllowed;
        bool cache_grant =
            IdentityKeyApprovalCache::GetInstance().isApproved(requestDomain_);
        if (!persistent_grant && !cache_grant) {
            LOG_DEBUG_HTTP("🛡️ identity-key-style /getPublicKey bypassed Open() for "
                + requestDomain_ + " (drain-forward or sibling-forward) — firing "
                + "identity_key_reveal prompt instead of sending a doomed request to Rust");
            triggerIdentityKeyRevealModal(requestDomain_);
            postAuthTimeout(kPromptAuthTimeoutMs, "{\"error\":\"identity_key_reveal timeout\",\"status\":\"error\"}");
            return;
        }
    }

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

    // Phase 1.5 Step 1 — propagate privacy-perimeter pre-approval to Rust.
    // X-Identity-Key-Approved: true lets get_public_key bypass its
    // identity_key_prompt_required gate (defense-in-depth). The header is added
    // when EITHER (a) Open() set identityKeyApproved_ after a cache hit, OR
    // (b) we're being forwarded via the drain-after-domain-approval path (which
    // skips Open()) and the in-memory IdentityKeyApprovalCache already says the
    // domain is approved. Case (b) covers the race where Rust's DB write of
    // identity_key_disclosure_allowed hasn't landed yet.
    bool sendIdentityKeyApproved = identityKeyApproved_
        || (!requestDomain_.empty()
            && IdentityKeyApprovalCache::GetInstance().isApproved(requestDomain_));
    if (sendIdentityKeyApproved) {
        headers.insert(std::make_pair("X-Identity-Key-Approved", "true"));
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

// ============================================================================
// BRC-121 (Simple HTTP 402 Payment) — Phase 1 architecture
// ============================================================================
//
// Server returns 402 + (x-bsv-sats, x-bsv-server). We respond by:
//   1. Calling /wallet/pay402 (Rust) which mints a signed BRC-29 BEEF tx
//      with no_send=true (NOT broadcast yet). Returns 5 BRC-121 retry
//      headers: x-bsv-beef, x-bsv-sender, x-bsv-nonce, x-bsv-time, x-bsv-vout.
//   2. Storing those headers in s_brc121_paid_retries keyed by (browserId, url).
//   3. Programmatically reloading the page (frame->LoadURL).
//   4. The reload navigation hits SimpleHandler::GetResourceRequestHandler,
//      which checks the registry and returns a handler chain ending in
//      Async402ResourceHandler. That handler issues a fresh CefURLRequest
//      with all 5 headers attached (we control every byte → no CEF middleware
//      can strip them) plus UR_FLAG_DISABLE_CACHE.
//   5. On 200: the handler calls /wallet/broadcast-nosend (broadcasts the
//      tx now that the server has confirmed acceptance — eliminates the
//      isMerge race AND the nosend auto-fail race in one move), fires the
//      payment_success_indicator IPC for the green-dot animation, and
//      streams the response body back to the page. Article renders.
//   6. On 4xx: handler delivers the error response, doesn't broadcast.
//      Funds preserved (tx stays in nosend status; user can manually clear
//      or it will eventually fail via the Monitor's 10-min timeout).
//
// Modal-approval path (unapproved domain): TryHandleBrc121_402 fires
// triggerDomainApprovalModal as before, registering a context-less reload
// in s_brc121_pending_reloads (URL only). On approval, the IPC handler in
// simple_handler.cpp triggers a reload, which hits TryHandleBrc121_402 a
// second time — now with domain approved — proceeding through the normal
// auto-approve path and registering a paid retry context.

namespace {
// Paid retry context — populated when /wallet/pay402 returns successfully,
// drained by GetResourceRequestHandler when the reload navigation arrives.
struct PaidRetryContext {
    std::string url;
    std::string method;
    CefRequest::HeaderMap originalHeaders;
    // 5 BRC-121 retry headers from /wallet/pay402:
    std::string beefBase64;
    std::string senderPubkeyHex;
    std::string nonceB64;
    std::string timeMs;
    std::string voutStr;
    // Payment metadata:
    std::string txid;
    int64_t cents = 0;
    int64_t satoshis = 0;
    std::string domain;
};

std::string brc121RetryKey(int browserId, const std::string& url) {
    return std::to_string(browserId) + "|" + url;
}

std::mutex s_brc121_paid_retries_mutex;
std::unordered_map<std::string, PaidRetryContext> s_brc121_paid_retries;

void registerPaidRetryContext(int browserId, const std::string& url, PaidRetryContext ctx) {
    std::lock_guard<std::mutex> lock(s_brc121_paid_retries_mutex);
    s_brc121_paid_retries[brc121RetryKey(browserId, url)] = std::move(ctx);
}

bool popPaidRetryContext(int browserId, const std::string& url, PaidRetryContext& out) {
    std::lock_guard<std::mutex> lock(s_brc121_paid_retries_mutex);
    auto it = s_brc121_paid_retries.find(brc121RetryKey(browserId, url));
    if (it == s_brc121_paid_retries.end()) return false;
    out = std::move(it->second);
    s_brc121_paid_retries.erase(it);
    return true;
}

// Modal-approval reload registry — URL-only, used when the user approves a
// previously-unapproved domain via the modal flow. Drained by
// TriggerPendingBrc121Reloads from simple_handler.cpp's approval IPC.
struct PendingReload {
    CefRefPtr<CefBrowser> browser;
    std::string url;
};
std::mutex s_brc121_pending_reloads_mutex;
std::unordered_map<std::string, std::vector<PendingReload>> s_brc121_pending_reloads;
// Per-domain price snapshot from the most recent 402 — used by OnLoadError
// when building the /payment-pending placeholder URL so the placeholder can
// show "X sats" alongside the spinning Hodos logo.
std::unordered_map<std::string, int64_t> s_brc121_pending_sats;

// Phase 1 polish — registry for upstream paid-retry failures that exhausted
// MAX_UPSTREAM_RETRIES. Keyed by URL. Consumed by OnLoadError to swap the
// failed-load page for /payment-failed (Hodos error page with Retry button).
struct Brc121FailedEntry {
    std::string domain;
    int64_t satoshis;
    int upstreamStatus;
};
std::mutex s_brc121_failed_urls_mutex;
std::unordered_map<std::string, Brc121FailedEntry> s_brc121_failed_urls;

// B+3 polish — one-shot approved-URL registry for BRC-121 over-cap modals.
// Populated by MarkBrc121PaymentApproved() (called from simple_handler.cpp's
// brc100_auth_response approval handler) when the user approves a
// payment_confirmation / rate_limit_exceeded modal whose stored endpoint is
// an http(s) article URL. Atomically popped by TryHandleBrc121_402 BEFORE
// the cap-check on the reload that follows approval, so the user's just-
// approved payment proceeds without re-prompting. Strict one-shot: pop
// consumes, so subsequent visits to the same URL re-check caps normally —
// no permanent cap bypass, no infinite-loop risk.
std::mutex s_brc121_approved_urls_mutex;
std::unordered_set<std::string> s_brc121_approved_urls;

bool popBrc121ApprovedUrl(const std::string& url) {
    std::lock_guard<std::mutex> lock(s_brc121_approved_urls_mutex);
    auto it = s_brc121_approved_urls.find(url);
    if (it == s_brc121_approved_urls.end()) return false;
    s_brc121_approved_urls.erase(it);
    return true;
}

void registerPendingBrc121Reload(const std::string& domain,
                                 CefRefPtr<CefBrowser> browser,
                                 const std::string& url) {
    if (!browser) return;
    std::lock_guard<std::mutex> lock(s_brc121_pending_reloads_mutex);
    auto& v = s_brc121_pending_reloads[domain];
    int browserId = browser->GetIdentifier();
    for (const auto& p : v) {
        if (p.browser && p.browser->GetIdentifier() == browserId && p.url == url) {
            return;
        }
    }
    v.push_back(PendingReload{browser, url});
}

// SetPendingBrc121PriceForDomain stays in the anonymous namespace — it's
// only called from TryHandleBrc121_402 within this translation unit. The
// other two cross-TU helpers (Has…, GetPending…) move outside the namespace
// at the bottom of this file so they get external linkage for
// simple_handler.cpp's OnLoadError to call.
void SetPendingBrc121PriceForDomain(const std::string& domain, int64_t sats) {
    std::lock_guard<std::mutex> lock(s_brc121_pending_reloads_mutex);
    s_brc121_pending_sats[domain] = sats;
}

class Brc121ReloadTask : public CefTask {
public:
    // replace_history=true uses window.location.replace so the current
    // history entry (typically /payment-pending) is REPLACED instead of
    // appended-then-pushed. Used by TriggerPendingBrc121Reloads from the
    // user-approve modal flow so the back button after the article loads
    // skips the placeholder and goes back to the real previous page.
    // The auto-approve internal reload (post-pay_402) leaves replace_history
    // false because it navigates to the SAME URL the page is already on
    // (Chromium treats same-URL LoadURL as a reload, not a new entry).
    Brc121ReloadTask(CefRefPtr<CefBrowser> browser, std::string url,
                     bool replace_history = false)
        : browser_(std::move(browser)), url_(std::move(url)),
          replace_history_(replace_history) {}
    void Execute() override {
        if (!browser_) return;
        auto frame = browser_->GetMainFrame();
        if (!frame) return;
        if (replace_history_) {
            // JS-encode single quotes + backslashes; strip newlines (none
            // expected in a URL but defense-in-depth).
            std::string escaped;
            escaped.reserve(url_.size());
            for (char c : url_) {
                if (c == '\\') { escaped += "\\\\"; }
                else if (c == '\'') { escaped += "\\'"; }
                else if (c == '\n' || c == '\r') { /* skip */ }
                else { escaped += c; }
            }
            std::string js = "window.location.replace('" + escaped + "');";
            LOG_INFO_HTTP("💰 BRC-121: location.replace " + url_);
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        } else {
            LOG_INFO_HTTP("💰 BRC-121: reloading " + url_);
            frame->LoadURL(url_);
        }
    }
private:
    CefRefPtr<CefBrowser> browser_;
    std::string url_;
    bool replace_history_;
    IMPLEMENT_REFCOUNTING(Brc121ReloadTask);
    DISALLOW_COPY_AND_ASSIGN(Brc121ReloadTask);
};
}  // namespace

// ============================================================================
// Async402ResourceHandler — the canonical Phase 1 architecture
// ============================================================================

class Async402ResourceHandler;  // forward

class Async402HTTPClient : public CefURLRequestClient {
public:
    explicit Async402HTTPClient(CefRefPtr<Async402ResourceHandler> parent)
        : parent_(parent) {}

    void OnRequestComplete(CefRefPtr<CefURLRequest> request) override;
    void OnDownloadData(CefRefPtr<CefURLRequest>, const void* data, size_t data_length) override {
        std::lock_guard<std::mutex> lock(mutex_);
        responseData_.append(static_cast<const char*>(data), data_length);
    }
    void OnUploadProgress(CefRefPtr<CefURLRequest>, int64_t, int64_t) override {}
    void OnDownloadProgress(CefRefPtr<CefURLRequest>, int64_t, int64_t) override {}
    bool GetAuthCredentials(bool, const CefString&, int, const CefString&,
                            const CefString&, CefRefPtr<CefAuthCallback>) override { return false; }

private:
    CefRefPtr<Async402ResourceHandler> parent_;
    std::mutex mutex_;
    std::string responseData_;

    IMPLEMENT_REFCOUNTING(Async402HTTPClient);
    DISALLOW_COPY_AND_ASSIGN(Async402HTTPClient);
};

class Async402ResourceHandler : public CefResourceHandler {
public:
    // Phase 1 polish — retry budget for transient upstream rejections.
    // Cloudflare in front of bsvblockchain.tech sometimes returns HTTP 431
    // ("Request Header Fields Too Large") for the BEEF base64 retry header
    // even though the same URL+headers will succeed seconds later. One
    // automatic retry catches most of these without bothering the user.
    static constexpr int MAX_UPSTREAM_RETRIES = 1;
    static constexpr int RETRY_DELAY_MS = 250;

    Async402ResourceHandler(PaidRetryContext ctx, CefRefPtr<CefBrowser> browser)
        : ctx_(std::move(ctx)), browser_(std::move(browser)),
          responseStatus_(0), responseOffset_(0), completed_(false),
          retryAttempts_(0) {
        LOG_DEBUG_HTTP("🌐 Async402ResourceHandler created for " + ctx_.url);
    }

    bool Open(CefRefPtr<CefRequest> /*request*/, bool& handle_request,
              CefRefPtr<CefCallback> callback) override {
        CEF_REQUIRE_IO_THREAD();
        // Decide later — we need to wait for the upstream response before
        // CEF calls GetResponseHeaders, otherwise we'd commit to a placeholder
        // 502/text-plain and the browser would lock in plain-text rendering
        // (manifests as <pre>-wrapped HTML in the page).
        handle_request = false;
        openCallback_ = callback;
        CefPostTask(TID_IO, new StartTask(this));
        return true;
    }

    void GetResponseHeaders(CefRefPtr<CefResponse> response,
                           int64_t& response_length,
                           CefString& redirectUrl) override {
        CEF_REQUIRE_IO_THREAD();
        std::string mimeSet;
        if (responseStatus_ > 0) {
            response->SetStatus(responseStatus_);
            if (!responseStatusText_.empty()) {
                response->SetStatusText(responseStatusText_);
            }
            if (!responseHeaders_.empty()) {
                response->SetHeaderMap(responseHeaders_);
            }
            // Pull MIME type from received headers (browser uses this to
            // decide how to render the body).
            for (const auto& h : responseHeaders_) {
                std::string n = h.first.ToString();
                std::transform(n.begin(), n.end(), n.begin(), ::tolower);
                if (n == "content-type") {
                    std::string ct = h.second.ToString();
                    auto semi = ct.find(';');
                    mimeSet = (semi == std::string::npos ? ct : ct.substr(0, semi));
                    response->SetMimeType(mimeSet);
                    break;
                }
            }
            if (mimeSet.empty()) {
                // No Content-Type from upstream — default to text/html for
                // BRC-121 paywalled pages (most common case). Without this,
                // CEF defaults to binary download or text/plain.
                response->SetMimeType("text/html");
                mimeSet = "text/html (defaulted)";
            }
        } else {
            response->SetStatus(502);
            response->SetStatusText("Bad Gateway");
            response->SetMimeType("text/plain");
            mimeSet = "text/plain (502 fallback)";
        }
        response_length = completed_ ? static_cast<int64_t>(responseBody_.size()) : -1;
        LOG_INFO_HTTP("🌐 Async402: GetResponseHeaders status=" + std::to_string(responseStatus_)
                      + " mime='" + mimeSet + "' length=" + std::to_string(response_length));
    }

    bool ReadResponse(void* data_out, int bytes_to_read, int& bytes_read,
                     CefRefPtr<CefCallback> callback) override {
        CEF_REQUIRE_IO_THREAD();
        if (!completed_) {
            bytes_read = 0;
            readCallback_ = callback;
            return true;  // Wait for response.
        }
        if (responseOffset_ >= responseBody_.size()) {
            bytes_read = 0;
            return false;  // No more data.
        }
        size_t remaining = responseBody_.size() - responseOffset_;
        size_t to_copy = static_cast<size_t>(bytes_to_read) < remaining
                             ? static_cast<size_t>(bytes_to_read) : remaining;
        memcpy(data_out, responseBody_.data() + responseOffset_, to_copy);
        responseOffset_ += to_copy;
        bytes_read = static_cast<int>(to_copy);
        return true;
    }

    void Cancel() override {
        CEF_REQUIRE_IO_THREAD();
        if (urlRequest_) {
            urlRequest_->Cancel();
            urlRequest_ = nullptr;
        }
        // Release any held callbacks so CEF doesn't wait forever.
        if (openCallback_) {
            openCallback_->Cancel();
            openCallback_ = nullptr;
        }
        if (readCallback_) {
            readCallback_->Cancel();
            readCallback_ = nullptr;
        }
    }

    // Called by Async402HTTPClient when the upstream response completes.
    void onUpstreamComplete(int status,
                            CefResponse::HeaderMap headers,
                            std::string statusText,
                            std::string body) {
        responseStatus_ = status;
        responseStatusText_ = std::move(statusText);

        // === DIAGNOSTIC LOGGING (page-rendering investigation 2026-05-08) ===
        // Capture status, body shape, and ALL upstream headers so we can
        // diagnose why the page renders as black/raw without burning more
        // sats on test cycles.
        {
            LOG_INFO_HTTP("🌐 Async402: upstream complete — status=" + std::to_string(status)
                          + " statusText='" + responseStatusText_ + "'"
                          + " bodyBytes=" + std::to_string(body.size()));

            // Log all upstream headers (before strip) so we can see exactly
            // what the server sent.
            for (const auto& h : headers) {
                LOG_DEBUG_HTTP("🌐 Async402: upstream header [" + h.first.ToString()
                               + "] = [" + h.second.ToString() + "]");
            }

            // Log body preview — first 256 bytes. If body starts with `<`
            // it's HTML text; if it starts with 0x1f 0x8b it's still gzip;
            // if it starts with 0x00 0x00 it's something else.
            std::string preview;
            preview.reserve(256);
            size_t n = body.size() < 256 ? body.size() : 256;
            bool printable = true;
            for (size_t i = 0; i < n; ++i) {
                unsigned char c = static_cast<unsigned char>(body[i]);
                if (c < 0x09 || (c > 0x0d && c < 0x20) || c == 0x7f) {
                    printable = false;
                    break;
                }
            }
            if (printable) {
                preview = body.substr(0, n);
                std::replace(preview.begin(), preview.end(), '\n', ' ');
            } else {
                // Hex-encode first 64 bytes for binary inspection
                static const char hex[] = "0123456789abcdef";
                size_t m = body.size() < 64 ? body.size() : 64;
                for (size_t i = 0; i < m; ++i) {
                    unsigned char c = static_cast<unsigned char>(body[i]);
                    preview.push_back(hex[c >> 4]);
                    preview.push_back(hex[c & 0x0f]);
                }
                preview = "[hex first 64B] " + preview;
            }
            LOG_INFO_HTTP("🌐 Async402: body preview: " + preview);
        }
        // === END DIAGNOSTIC LOGGING ===

        // Strip transport-level headers that don't apply to the body we're
        // about to deliver. CefURLRequest auto-decompresses gzip/brotli, so
        // OnDownloadData gives us the DECODED body. If we forward the
        // upstream's `Content-Encoding: gzip` (or br) to the page, the page's
        // browser tries to decompress already-decompressed bytes → garbage
        // (manifests as black screen / raw HTML). Same for Content-Length
        // (the upstream value is the COMPRESSED length, not the decoded
        // length we're delivering) and Transfer-Encoding (chunked framing
        // is also unwound by the URL stack).
        for (auto it = headers.begin(); it != headers.end();) {
            std::string n = it->first.ToString();
            std::transform(n.begin(), n.end(), n.begin(), ::tolower);
            if (n == "content-encoding" || n == "content-length" ||
                n == "transfer-encoding") {
                it = headers.erase(it);
            } else {
                ++it;
            }
        }
        responseHeaders_ = std::move(headers);
        responseBody_ = std::move(body);
        completed_ = true;

        // Phase 1 polish — auto-retry on transient upstream failures
        // (Cloudflare 431 against the BEEF base64 header is the common
        // case). The same paid retry context is reused; we don't recreate
        // the nosend tx — the wallet still has it.
        bool retryable = (status == 431) || (status >= 500 && status < 600);
        if (retryable && retryAttempts_ < MAX_UPSTREAM_RETRIES) {
            retryAttempts_++;
            LOG_WARNING_HTTP("💰 BRC-121: server returned status="
                             + std::to_string(status) + " — auto-retry "
                             + std::to_string(retryAttempts_) + "/"
                             + std::to_string(MAX_UPSTREAM_RETRIES)
                             + " for " + ctx_.url);
            // Reset response state so the retry's complete handler sees
            // a clean slate. Keep openCallback_ / readCallback_ — they
            // hold CEF open until the retry completes.
            responseStatus_ = 0;
            responseStatusText_.clear();
            responseHeaders_.clear();
            responseBody_.clear();
            completed_ = false;
            urlRequest_ = nullptr;
            CefPostDelayedTask(TID_IO, new StartTask(this), RETRY_DELAY_MS);
            return;
        }

        // On success: broadcast our nosend tx (server confirmed acceptance,
        // safe to commit on chain) and fire the payment animation IPC.
        if (status >= 200 && status < 300) {
            broadcastNosendAsync();
            firePaymentSuccessIpc();

            // Cache the paid response so reload doesn't re-pay. Order matters:
            // run AFTER firePaymentSuccessIpc() so a cache-write failure
            // (disk full, SQLite error) cannot silently break the green-dot
            // animation or session accounting. Put() is best-effort and
            // swallows exceptions internally — see PaidContentCache.cpp.
            std::vector<uint8_t> body_bytes(responseBody_.begin(),
                                            responseBody_.end());
            auto expiresAt =
                PaidContentCache::ParseCacheControl(responseHeaders_);
            PaidContentCache::GetInstance().Put(
                ctx_.url, status, responseHeaders_, body_bytes, expiresAt);
        } else {
            LOG_WARNING_HTTP("💰 BRC-121: server returned status=" + std::to_string(status)
                             + " on retry — NOT broadcasting (funds preserved). txid="
                             + ctx_.txid);
            // Phase 1 polish — register the failure so OnLoadError can
            // swap CEF's data:text/html "Failed to load" for a Hodos
            // /payment-failed page with a Retry button.
            RegisterBrc121FailedUrl(ctx_.url, ctx_.domain,
                                    ctx_.satoshis, status);
        }

        // Tell CEF we're ready: it will now call GetResponseHeaders (which
        // sees real status + headers) and ReadResponse (which streams body).
        if (openCallback_) {
            openCallback_->Continue();
            openCallback_ = nullptr;
        }
        if (readCallback_) {
            readCallback_->Continue();
        }
    }

private:
    // CefPostTask wrapper for the upstream URL request kickoff.
    class StartTask : public CefTask {
    public:
        explicit StartTask(CefRefPtr<Async402ResourceHandler> parent)
            : parent_(std::move(parent)) {}
        void Execute() override { parent_->startUpstreamRequest(); }
    private:
        CefRefPtr<Async402ResourceHandler> parent_;
        IMPLEMENT_REFCOUNTING(StartTask);
        DISALLOW_COPY_AND_ASSIGN(StartTask);
    };

    void startUpstreamRequest() {
        LOG_INFO_HTTP("🌐 Async402ResourceHandler: issuing paid request to " + ctx_.url
                      + " (txid=" + ctx_.txid.substr(0, std::min<size_t>(16, ctx_.txid.size())) + "...)");

        CefRefPtr<CefRequest> req = CefRequest::Create();
        req->SetURL(ctx_.url);
        req->SetMethod(ctx_.method);
        // Disable cache so we don't replay the previously-cached 402 response.
        req->SetFlags(UR_FLAG_DISABLE_CACHE);

        // Start with the page's original headers (User-Agent, Accept-Language,
        // cookies, etc.) so the request looks browser-like to Cloudflare/WAF.
        CefRequest::HeaderMap headers = ctx_.originalHeaders;
        // Strip any pre-existing BRC-121 headers (shouldn't be there, but be safe).
        for (auto it = headers.begin(); it != headers.end();) {
            std::string n = it->first.ToString();
            std::transform(n.begin(), n.end(), n.begin(), ::tolower);
            if (n == "x-bsv-beef" || n == "x-bsv-sender" || n == "x-bsv-nonce" ||
                n == "x-bsv-time" || n == "x-bsv-vout") {
                it = headers.erase(it);
            } else {
                ++it;
            }
        }
        headers.insert({"x-bsv-beef",   ctx_.beefBase64});
        headers.insert({"x-bsv-sender", ctx_.senderPubkeyHex});
        headers.insert({"x-bsv-nonce",  ctx_.nonceB64});
        headers.insert({"x-bsv-time",   ctx_.timeMs});
        headers.insert({"x-bsv-vout",   ctx_.voutStr});
        req->SetHeaderMap(headers);

        CefRefPtr<Async402HTTPClient> client = new Async402HTTPClient(this);
        // Use the page's own request context so the cookie jar is shared
        // between the upstream paid retry and the page. In a multi-profile
        // browser, profile-specific contexts have their own cookie jars; the
        // global context's jar would be invisible to the page's next nav.
        CefRefPtr<CefRequestContext> reqCtx = browser_
            ? browser_->GetHost()->GetRequestContext()
            : CefRequestContext::GetGlobalContext();
        urlRequest_ = CefURLRequest::Create(req, client, reqCtx);
    }

    void firePaymentSuccessIpc() {
        // Phase 2.5 Commit 6 sub-step 6.b — fire indicator + record spending
        // via the shared OnWalletCallSuccess helper. BRC-121 ALSO needs to
        // increment the per-session rate / payment counters here (NOT inside
        // OnWalletCallSuccess) because BRC-121 has no Open()-stage
        // silent-approve step — the createAction path increments at Open()
        // time, BRC-121 increments at success time. The 2-line counter block
        // below intentionally lives adjacent to the call so a future reader
        // sees the timing-difference at a glance.
        int cefBrowserId = browser_ ? browser_->GetIdentifier() : 0;
        OnWalletCallSuccess(cefBrowserId,
                            ctx_.domain,
                            ctx_.cents,
                            /*wasAutoApprovedPayment=*/true,
                            /*endpoint=*/"pay402");

        // BRC-121-specific counter increments (no Open()-stage silent-approve
        // to do these at gate-decision time).
        SessionManager::GetInstance().incrementRateCounter(cefBrowserId);
        SessionManager::GetInstance().incrementPaymentCount(cefBrowserId);
    }

    void broadcastNosendAsync() {
        // POST /wallet/broadcast-nosend with the txid we just paid.
        // Uses the SyncHttpClient via a posted task so we don't block the IO
        // thread that's about to deliver the response body to the page.
        std::string txid = ctx_.txid;
        CefPostTask(TID_FILE_USER_BLOCKING, new BroadcastTask(txid));
    }

    class BroadcastTask : public CefTask {
    public:
        explicit BroadcastTask(std::string txid) : txid_(std::move(txid)) {}
        void Execute() override {
            nlohmann::json body;
            body["txid"] = txid_;
            HttpResponse r = SyncHttpClient::Post(
                "http://localhost:31301/wallet/broadcast-nosend",
                body.dump(), "application/json", 30000);
            if (!r.success) {
                LOG_WARNING_HTTP("💰 BRC-121: broadcast-nosend failed (status="
                                 + std::to_string(r.statusCode) + ") for txid=" + txid_
                                 + " — Monitor's task_check_for_proofs will reconcile");
            } else {
                LOG_INFO_HTTP("💰 BRC-121: broadcast-nosend OK for txid=" + txid_);
            }
        }
    private:
        std::string txid_;
        IMPLEMENT_REFCOUNTING(BroadcastTask);
        DISALLOW_COPY_AND_ASSIGN(BroadcastTask);
    };

    PaidRetryContext ctx_;
    CefRefPtr<CefBrowser> browser_;

    int responseStatus_;
    std::string responseStatusText_;
    CefResponse::HeaderMap responseHeaders_;
    std::string responseBody_;
    size_t responseOffset_;
    bool completed_;
    int retryAttempts_;  // Phase 1 polish — see MAX_UPSTREAM_RETRIES
    CefRefPtr<CefCallback> openCallback_;   // Fired in onUpstreamComplete to release Open()
    CefRefPtr<CefCallback> readCallback_;
    CefRefPtr<CefURLRequest> urlRequest_;

    IMPLEMENT_REFCOUNTING(Async402ResourceHandler);
    DISALLOW_COPY_AND_ASSIGN(Async402ResourceHandler);
};

// Async402HTTPClient::OnRequestComplete — out-of-line because it references
// Async402ResourceHandler which is forward-declared above the client.
void Async402HTTPClient::OnRequestComplete(CefRefPtr<CefURLRequest> request) {
    int status = 0;
    std::string statusText;
    CefResponse::HeaderMap headers;
    auto resp = request->GetResponse();
    if (resp) {
        status = resp->GetStatus();
        statusText = resp->GetStatusText().ToString();
        resp->GetHeaderMap(headers);
    }
    std::string body;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        body = std::move(responseData_);
    }
    if (parent_) {
        parent_->onUpstreamComplete(status, std::move(headers), std::move(statusText), std::move(body));
    }
}

// Public hook used by SimpleHandler::GetResourceRequestHandler (and the
// per-handler GetResourceHandler overrides) to install Async402ResourceHandler
// when a navigation arrives that has a registered paid-retry context.
CefRefPtr<CefResourceHandler> InstallAsync402HandlerIfPending(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefRequest> request) {
    int browserId = browser ? browser->GetIdentifier() : 0;
    std::string url = request ? request->GetURL().ToString() : "";
    if (url.empty()) return nullptr;
    PaidRetryContext ctx;
    if (!popPaidRetryContext(browserId, url, ctx)) {
        return nullptr;
    }
    LOG_INFO_HTTP("💰 BRC-121: installing Async402ResourceHandler for " + url
                  + " (browser " + std::to_string(browserId) + ")");
    return new Async402ResourceHandler(std::move(ctx), browser);
}

bool TryHandleBrc121_402(CefRefPtr<CefBrowser> browser,
                         CefRefPtr<CefFrame> frame,
                         CefRefPtr<CefRequest> request,
                         CefRefPtr<CefResponse> response) {
    CEF_REQUIRE_IO_THREAD();

    // Fast path: only consider 402 Payment Required responses.
    if (response->GetStatus() != 402) {
        return false;
    }

    // BRC-121 protocol headers.
    std::string satsStr = response->GetHeaderByName("x-bsv-sats").ToString();
    std::string serverPubkey = response->GetHeaderByName("x-bsv-server").ToString();
    if (satsStr.empty() || serverPubkey.empty()) {
        return false;  // Not a BRC-121 402 — let the page handle whatever it is.
    }

    int64_t satoshis = 0;
    try {
        satoshis = std::stoll(satsStr);
    } catch (...) {
        LOG_DEBUG_HTTP("💰 BRC-121: invalid x-bsv-sats value '" + satsStr + "' — falling through");
        return false;
    }
    if (satoshis <= 0) {
        return false;
    }

    // Server pubkey shape check: 33-byte compressed = 66 hex chars, prefix 02 or 03.
    if (serverPubkey.size() != 66 ||
        (serverPubkey[0] != '0') ||
        (serverPubkey[1] != '2' && serverPubkey[1] != '3')) {
        LOG_DEBUG_HTTP("💰 BRC-121: invalid x-bsv-server shape — falling through");
        return false;
    }

    std::string url = request->GetURL().ToString();

    // For BRC-121, the requesting "domain" is the HOST OF THE URL BEING FETCHED
    // (the payee), not the page's main frame. Different from createAction-style
    // wallet API calls where the embedding page is the actor — here the server
    // demanding payment is what the user is approving / what gets logged in the
    // approved-sites list. Without this distinction, an embedded 402 fetch from
    // a page like google.com would falsely claim "google.com wants 100 sats"
    // when really the payee is whatever host the request is going to.
    std::string domain;
    {
        size_t schemeEnd = url.find("://");
        if (schemeEnd == std::string::npos) {
            LOG_DEBUG_HTTP("💰 BRC-121: malformed URL '" + url + "' — falling through");
            return false;
        }
        size_t hostStart = schemeEnd + 3;
        size_t pathStart = url.find('/', hostStart);
        domain = (pathStart == std::string::npos)
                     ? url.substr(hostStart)
                     : url.substr(hostStart, pathStart - hostStart);
    }
    if (domain.empty()) {
        LOG_DEBUG_HTTP("💰 BRC-121: empty request host — falling through");
        return false;
    }

    LOG_INFO_HTTP("💰 BRC-121 402 detected: " + std::to_string(satoshis) + " sats from " + domain
                  + " → server " + serverPubkey.substr(0, 16) + "...");

    // No wallet → can't pay. Page sees the 402 (may show its own UI / sign-in).
    if (!WalletStatusCache::GetInstance().walletExists()) {
        LOG_DEBUG_HTTP("💰 BRC-121: no wallet — falling through to native 402");
        return false;
    }

    auto perm = DomainPermissionCache::GetInstance().getPermission(domain);

    // Unapproved domain → fire domain_approval modal (same as createAction).
    if (perm.trustLevel != "approved") {
        LOG_DEBUG_HTTP("💰 BRC-121: domain trust='" + perm.trustLevel
                       + "' — firing domain_approval modal");
        bool modalAlreadyShowing = PendingRequestManager::GetInstance().hasPendingForDomain(domain);
        PendingRequestManager::GetInstance().addRequest(
            domain, "GET", url, "", nullptr, "domain_approval");
        // Register this browser+url so simple_handler.cpp's approval IPC can
        // navigate us back here after the user accepts. Without this, CEF
        // shows its data:text/html ERR_HTTP_RESPONSE_CODE_FAILURE page and a
        // manual refresh just refreshes the error page (the original URL is
        // gone from the address bar). User would have to close+reopen the tab.
        // Also acts as the marker for OnLoadError → /payment-pending placeholder
        // (see HasPendingBrc121ReloadForDomain in HttpRequestInterceptor.h).
        registerPendingBrc121Reload(domain, browser, url);
        // Stash the per-payment context (sats/server) so OnLoadError can build
        // a proper placeholder URL with the right amount.
        SetPendingBrc121PriceForDomain(domain, satoshis);
        if (!modalAlreadyShowing) {
            CefPostTask(TID_UI, new CreateNotificationOverlayTask("domain_approval", domain));
        }
        return false;  // Page sees 402 once; OnLoadError swaps the
                       // failed-load page for /payment-pending; reload after
                       // approval auto-pays.
    }

    // B+3 polish — one-shot bypass for previously-approved over-cap payments.
    // If the user approved this exact URL on a prior payment_confirmation /
    // rate_limit_exceeded modal, MarkBrc121PaymentApproved put the URL in
    // s_brc121_approved_urls. popBrc121ApprovedUrl consumes the entry
    // atomically; subsequent visits to the same URL re-check caps normally
    // (no permanent bypass). When set, skip ALL cap-check branches below
    // and fall through to the auto-approve path.
    const bool oneShotApproved = popBrc121ApprovedUrl(url);
    if (oneShotApproved) {
        LOG_INFO_HTTP("💰 BRC-121: one-shot approved URL — bypassing cap check for "
                      + url);
    }

    // Sats → USD cents. If BSV price is unavailable, fire payment_confirmation
    // with exceededLimit=price_unavailable (same as createAction's safety branch).
    double bsvPrice = BSVPriceCache::GetInstance().getPrice();
    int browserId = browser ? browser->GetIdentifier() : 0;
    SessionManager::GetInstance().getSession(browserId, domain);

    if (!oneShotApproved && bsvPrice <= 0) {
        LOG_DEBUG_HTTP("💰 BRC-121: BSV price unavailable — firing payment_confirmation modal");
        bool modalAlreadyShowing = PendingRequestManager::GetInstance().hasPendingForDomain(domain);
        PendingRequestManager::GetInstance().addRequest(
            domain, "GET", url, "", nullptr, "payment_confirmation");
        // Register the pending reload + price so OnLoadError swaps the
        // failed-load page for /payment-pending and TriggerPendingBrc121Reloads
        // can re-navigate the tab after approval (mirrors the unapproved-
        // domain branch above).
        registerPendingBrc121Reload(domain, browser, url);
        SetPendingBrc121PriceForDomain(domain, satoshis);
        if (!modalAlreadyShowing) {
            std::string extraParams = "&satoshis=" + std::to_string(satoshis)
                                    + "&cents=0"
                                    + "&bsvPrice=0"
                                    + "&exceededLimit=price_unavailable"
                                    + "&perTxLimit=" + std::to_string(perm.perTxLimitCents)
                                    + "&perSessionLimit=" + std::to_string(perm.perSessionLimitCents)
                                    + "&sessionSpent=0";
            CefPostTask(TID_UI, new CreateNotificationOverlayTask("payment_confirmation", domain, extraParams));
        }
        return false;
    }

    int64_t cents = (bsvPrice > 0)
        ? static_cast<int64_t>(
            (static_cast<double>(satoshis) / 100000000.0) * bsvPrice * 100.0)
        : 0;
    int64_t spent = SessionManager::GetInstance().getSpentCents(browserId, domain);
    int txCount = SessionManager::GetInstance().getPaymentCount(browserId, domain);

    // Helper lambda: fire payment_confirmation modal with limit context.
    // Also registers the pending reload + per-domain price so OnLoadError
    // swaps the failed-load page for /payment-pending (matches the
    // unapproved-domain branch) and TriggerPendingBrc121Reloads can
    // re-navigate the tab after the user clicks Approve.
    auto firePaymentModal = [&](const std::string& exceededLimit) {
        bool modalAlreadyShowing = PendingRequestManager::GetInstance().hasPendingForDomain(domain);
        PendingRequestManager::GetInstance().addRequest(
            domain, "GET", url, "", nullptr, "payment_confirmation");
        registerPendingBrc121Reload(domain, browser, url);
        SetPendingBrc121PriceForDomain(domain, satoshis);
        if (!modalAlreadyShowing) {
            std::string extraParams = "&satoshis=" + std::to_string(satoshis)
                                    + "&cents=" + std::to_string(cents)
                                    + "&bsvPrice=" + std::to_string(bsvPrice)
                                    + "&exceededLimit=" + exceededLimit
                                    + "&perTxLimit=" + std::to_string(perm.perTxLimitCents)
                                    + "&perSessionLimit=" + std::to_string(perm.perSessionLimitCents)
                                    + "&sessionSpent=" + std::to_string(spent);
            CefPostTask(TID_UI, new CreateNotificationOverlayTask("payment_confirmation", domain, extraParams));
        }
    };

    // Auto-approve gates (mirrors createAction logic at AsyncWalletResourceHandler::Open).
    bool withinTxLimit = cents <= perm.perTxLimitCents;
    bool withinSessionLimit = (spent + cents) <= perm.perSessionLimitCents;

    if (!oneShotApproved && (!withinTxLimit || !withinSessionLimit)) {
        std::string exceeded = "both";
        if (withinTxLimit && !withinSessionLimit) exceeded = "per_session";
        else if (!withinTxLimit && withinSessionLimit) exceeded = "per_tx";
        LOG_DEBUG_HTTP("💰 BRC-121: spend cap exceeded (" + exceeded
                       + ") — firing payment_confirmation modal");
        firePaymentModal(exceeded);
        return false;
    }

    if (!oneShotApproved && txCount >= perm.maxTxPerSession) {
        LOG_DEBUG_HTTP("💰 BRC-121: max-tx-per-session reached — firing rate_limit_exceeded modal");
        bool modalAlreadyShowing = PendingRequestManager::GetInstance().hasPendingForDomain(domain);
        PendingRequestManager::GetInstance().addRequest(
            domain, "GET", url, "", nullptr, "rate_limit_exceeded");
        registerPendingBrc121Reload(domain, browser, url);
        SetPendingBrc121PriceForDomain(domain, satoshis);
        if (!modalAlreadyShowing) {
            std::string extraParams = "&satoshis=" + std::to_string(satoshis)
                                    + "&cents=" + std::to_string(cents)
                                    + "&bsvPrice=" + std::to_string(bsvPrice)
                                    + "&rateLimit=" + std::to_string(perm.rateLimitPerMin)
                                    + "&perTxLimit=" + std::to_string(perm.perTxLimitCents)
                                    + "&perSessionLimit=" + std::to_string(perm.perSessionLimitCents)
                                    + "&exceededLimit=session_tx_count"
                                    + "&maxTxPerSession=" + std::to_string(perm.maxTxPerSession)
                                    + "&txCount=" + std::to_string(txCount);
            CefPostTask(TID_UI, new CreateNotificationOverlayTask("rate_limit_exceeded", domain, extraParams));
        }
        return false;
    }

    if (!oneShotApproved && !SessionManager::GetInstance().checkRateLimit(browserId, perm.rateLimitPerMin)) {
        LOG_DEBUG_HTTP("💰 BRC-121: rate limit (" + std::to_string(perm.rateLimitPerMin)
                       + "/min) exceeded — firing rate_limit_exceeded modal");
        bool modalAlreadyShowing = PendingRequestManager::GetInstance().hasPendingForDomain(domain);
        PendingRequestManager::GetInstance().addRequest(
            domain, "GET", url, "", nullptr, "rate_limit_exceeded");
        registerPendingBrc121Reload(domain, browser, url);
        SetPendingBrc121PriceForDomain(domain, satoshis);
        if (!modalAlreadyShowing) {
            std::string extraParams = "&satoshis=" + std::to_string(satoshis)
                                    + "&cents=" + std::to_string(cents)
                                    + "&bsvPrice=" + std::to_string(bsvPrice)
                                    + "&rateLimit=" + std::to_string(perm.rateLimitPerMin)
                                    + "&perTxLimit=" + std::to_string(perm.perTxLimitCents)
                                    + "&perSessionLimit=" + std::to_string(perm.perSessionLimitCents);
            CefPostTask(TID_UI, new CreateNotificationOverlayTask("rate_limit_exceeded", domain, extraParams));
        }
        return false;
    }

    // All gates passed — call the Rust wallet to build the BRC-29 BEEF.
    nlohmann::json reqBody;
    reqBody["server_pubkey_hex"] = serverPubkey;
    reqBody["satoshis"] = satoshis;
    reqBody["original_url"] = url;
    std::string body = reqBody.dump();

    LOG_INFO_HTTP("💰 BRC-121 auto-approved (" + std::to_string(cents) + " cents) — calling /wallet/pay402");

    // Synchronous: localhost wallet, fast path. 10s ceiling — createAction may need
    // to fetch BEEF ancestry from external indexers in worst cases.
    HttpResponse rresp = SyncHttpClient::Post(
        "http://localhost:31301/wallet/pay402",
        body,
        "application/json",
        10000);

    if (!rresp.success || rresp.statusCode != 200) {
        LOG_WARNING_HTTP("💰 BRC-121: /wallet/pay402 failed (status="
                         + std::to_string(rresp.statusCode) + ") — falling through to native 402");
        return false;
    }

    std::string beefBase64;
    std::string txid;
    std::string nonceB64;
    std::string timeMs;
    std::string senderPubkeyHex;
    int voutIdx = 0;
    try {
        auto rj = nlohmann::json::parse(rresp.body);
        if (!rj.value("success", false)) {
            std::string err = rj.value("error", "(no error message)");
            LOG_WARNING_HTTP("💰 BRC-121: /wallet/pay402 success=false: " + err
                             + " — falling through to native 402");
            return false;
        }
        beefBase64       = rj.value("beef_base64", "");
        txid             = rj.value("txid", "");
        nonceB64         = rj.value("derivation_prefix", "");
        timeMs           = rj.value("time_ms", "");
        senderPubkeyHex  = rj.value("sender_pubkey_hex", "");
        voutIdx          = rj.value("vout", 0);
    } catch (const std::exception& e) {
        LOG_WARNING_HTTP("💰 BRC-121: /wallet/pay402 response parse failed: "
                         + std::string(e.what()) + " — falling through");
        return false;
    }

    if (beefBase64.empty() || nonceB64.empty() || timeMs.empty() || senderPubkeyHex.empty()) {
        LOG_WARNING_HTTP("💰 BRC-121: incomplete /wallet/pay402 response (missing beef/nonce/time/sender) — falling through");
        return false;
    }

    // Capture the page's original headers for the paid retry. Async402ResourceHandler
    // will use these (User-Agent, Accept-Language, cookies, etc.) so the retry
    // looks browser-like to Cloudflare/WAF.
    CefRequest::HeaderMap originalHeaders;
    request->GetHeaderMap(originalHeaders);

    // Register the paid retry context for this (browserId, url). On the
    // reload that follows, SimpleHandler::GetResourceRequestHandler (or one
    // of its handler subclasses) calls InstallAsync402HandlerIfPending,
    // which pops this context and returns Async402ResourceHandler to take
    // over the request lifecycle with all 5 BRC-121 headers attached.
    PaidRetryContext ctx;
    ctx.url = url;
    ctx.method = request->GetMethod().ToString();
    ctx.originalHeaders = std::move(originalHeaders);
    ctx.beefBase64 = std::move(beefBase64);
    ctx.senderPubkeyHex = std::move(senderPubkeyHex);
    ctx.nonceB64 = std::move(nonceB64);
    ctx.timeMs = std::move(timeMs);
    ctx.voutStr = std::to_string(voutIdx);
    ctx.txid = txid;
    ctx.cents = cents;
    ctx.satoshis = satoshis;
    ctx.domain = domain;
    registerPaidRetryContext(browserId, url, std::move(ctx));

    LOG_INFO_HTTP("💰 BRC-121 paid (txid=" + txid.substr(0, std::min<size_t>(16, txid.size()))
                  + "...) — registered paid retry; triggering reload to install handler");

    // Trigger a reload of the URL on the UI thread. The reload's GetResourceRequestHandler
    // chain will see the registered paid retry context and install
    // Async402ResourceHandler, which issues the actual request with all 5 BRC-121
    // headers and broadcasts the nosend tx after the server returns 200.
    CefPostTask(TID_UI, new Brc121ReloadTask(browser, url));
    return false;
}

// Thin wrapper — defers to the free function so the BRC-121 path is shared
// with CookieFilterResourceHandler (which sees all non-wallet URLs).
bool HttpRequestInterceptor::OnResourceResponse(CefRefPtr<CefBrowser> browser,
                                              CefRefPtr<CefFrame> frame,
                                              CefRefPtr<CefRequest> request,
                                              CefRefPtr<CefResponse> response) {
    return TryHandleBrc121_402(browser, frame, request, response);
}

// Called from simple_handler.cpp's add_domain_permission(_advanced) IPC handlers
// after the domain has been written to the DB and the cache has been
// invalidated. Reloads any browsers that hit a BRC-121 402 for this domain
// while it was unapproved — they're stuck on CEF's error page and won't
// recover even on manual refresh because the address-bar URL is now data:text/html.
// Cross-TU helpers for simple_handler.cpp's OnLoadError. Need external
// linkage, so they live outside the file's anonymous namespace. Both
// access the anonymous-namespace storage (visible at file scope).
bool HasPendingBrc121ReloadForDomain(const std::string& domain) {
    std::lock_guard<std::mutex> lock(s_brc121_pending_reloads_mutex);
    auto it = s_brc121_pending_reloads.find(domain);
    return it != s_brc121_pending_reloads.end() && !it->second.empty();
}

int64_t GetPendingBrc121PriceForDomain(const std::string& domain) {
    std::lock_guard<std::mutex> lock(s_brc121_pending_reloads_mutex);
    auto it = s_brc121_pending_sats.find(domain);
    return it == s_brc121_pending_sats.end() ? 0 : it->second;
}

// B+3 polish — external-linkage helper called from simple_handler.cpp's
// brc100_auth_response approval IPC when the user approves a BRC-121
// over-cap modal. Adds the article URL to the one-shot approved-URL
// registry; TryHandleBrc121_402 atomically pops it on the next 402 for
// the same URL and bypasses the cap-check exactly once.
void MarkBrc121PaymentApproved(const std::string& url) {
    std::lock_guard<std::mutex> lock(s_brc121_approved_urls_mutex);
    s_brc121_approved_urls.insert(url);
    LOG_INFO_HTTP("💰 BRC-121: URL marked one-shot approved (will bypass next cap check): "
                  + url);
}

// Phase 1 polish — failed-URL registry. RegisterBrc121FailedUrl is called
// from Async402ResourceHandler::onUpstreamComplete when the paid retry
// exhausts MAX_UPSTREAM_RETRIES with a non-2xx status. ConsumeBrc121FailedUrl
// is called from OnLoadError; if the failed load matches a registered URL,
// the entry is returned and removed (one-shot — a new attempt re-registers
// only if it also fails). HasBrc121FailedUrl peeks without consuming.
void RegisterBrc121FailedUrl(const std::string& url,
                             const std::string& domain,
                             int64_t satoshis,
                             int upstreamStatus) {
    std::lock_guard<std::mutex> lock(s_brc121_failed_urls_mutex);
    s_brc121_failed_urls[url] = Brc121FailedEntry{domain, satoshis, upstreamStatus};
    LOG_INFO_HTTP("💰 BRC-121: failed URL registered for /payment-failed swap: "
                  + url + " (status=" + std::to_string(upstreamStatus) + ")");
}

bool ConsumeBrc121FailedUrl(const std::string& url,
                            std::string& outDomain,
                            int64_t& outSatoshis,
                            int& outStatus) {
    std::lock_guard<std::mutex> lock(s_brc121_failed_urls_mutex);
    auto it = s_brc121_failed_urls.find(url);
    if (it == s_brc121_failed_urls.end()) return false;
    outDomain = it->second.domain;
    outSatoshis = it->second.satoshis;
    outStatus = it->second.upstreamStatus;
    s_brc121_failed_urls.erase(it);
    return true;
}

void TriggerPendingBrc121Reloads(const std::string& domain) {
    std::vector<PendingReload> drained;
    {
        std::lock_guard<std::mutex> lock(s_brc121_pending_reloads_mutex);
        auto it = s_brc121_pending_reloads.find(domain);
        if (it == s_brc121_pending_reloads.end() || it->second.empty()) {
            return;
        }
        drained = std::move(it->second);
        s_brc121_pending_reloads.erase(it);
        s_brc121_pending_sats.erase(domain);
    }
    LOG_INFO_HTTP("💰 BRC-121: domain '" + domain + "' approved — reloading "
                  + std::to_string(drained.size()) + " pending tab(s)");
    for (auto& p : drained) {
        if (!p.browser) continue;
        // replace_history=true: the tab is currently on /payment-pending,
        // and we want the eventually-loaded article to REPLACE the
        // placeholder in history. Otherwise back-from-article lands on
        // /payment-pending instead of whatever real page preceded the flow.
        CefPostTask(TID_UI, new Brc121ReloadTask(p.browser, p.url, true));
    }
}

// Reject counterpart to TriggerPendingBrc121Reloads. Called from the
// brc100_auth_response reject branch in simple_handler.cpp when the user
// declines a domain_approval modal. Drains the pending reload queue (so
// stale entries don't leak) and navigates each waiting tab back so the
// payment-pending placeholder doesn't linger after the user said no.
namespace {
class Brc121GoBackTask : public CefTask {
public:
    explicit Brc121GoBackTask(CefRefPtr<CefBrowser> browser)
        : browser_(std::move(browser)) {}
    void Execute() override {
        if (!browser_) return;
        if (browser_->CanGoBack()) {
            LOG_INFO_HTTP("💰 BRC-121: rejected — going back");
            browser_->GoBack();
        } else {
            // No history (user typed/pasted the URL or navigated from a new tab).
            // Land them on about:blank rather than leaving the placeholder up.
            auto frame = browser_->GetMainFrame();
            if (frame) {
                LOG_INFO_HTTP("💰 BRC-121: rejected, no back history — about:blank");
                frame->LoadURL("about:blank");
            }
        }
    }
private:
    CefRefPtr<CefBrowser> browser_;
    IMPLEMENT_REFCOUNTING(Brc121GoBackTask);
    DISALLOW_COPY_AND_ASSIGN(Brc121GoBackTask);
};
}  // namespace

void CancelPendingBrc121Reloads(const std::string& domain) {
    std::vector<PendingReload> drained;
    {
        std::lock_guard<std::mutex> lock(s_brc121_pending_reloads_mutex);
        auto it = s_brc121_pending_reloads.find(domain);
        if (it == s_brc121_pending_reloads.end() || it->second.empty()) {
            return;
        }
        drained = std::move(it->second);
        s_brc121_pending_reloads.erase(it);
        s_brc121_pending_sats.erase(domain);
    }
    LOG_INFO_HTTP("💰 BRC-121: domain '" + domain + "' rejected — backing out "
                  + std::to_string(drained.size()) + " pending tab(s)");
    for (auto& p : drained) {
        if (!p.browser) continue;
        CefPostTask(TID_UI, new Brc121GoBackTask(p.browser));
    }
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
            url.find("/revealCounterpartyKeyLinkage") != std::string::npos ||
            url.find("/revealSpecificKeyLinkage") != std::string::npos ||
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
