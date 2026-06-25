#include "../../include/core/HttpRequestInterceptor.h"
#include "../../include/core/CookieBlockManager.h"
#include "../../include/core/ManifestFetcher.h"
#include "../../include/core/SensitiveCertFields.h"
#include "../../include/core/SyncHttpClient.h"
#include "../../include/core/PortConfig.h"
#include "include/wrapper/cef_helpers.h"
#include "include/cef_urlrequest.h"
#include "include/cef_request.h"
#include "include/cef_request_context.h"
#include "include/cef_browser.h"
#include "include/cef_task.h"
#include "include/cef_v8.h"
#include "include/cef_frame.h"
// Phase 2.5 Commit 6 sub-step 6.d.A — base::BindOnce + CefPostTask overload
// for the IPC bridge's worker-thread dispatch. simple_handler.cpp includes
// the same headers for its wallet_call worker dispatch (commits 1-4).
#include "include/base/cef_bind.h"
#include "include/base/cef_callback.h"
#include "include/wrapper/cef_closure_task.h"  // base::OnceClosure → CefTask adapter
#include "../handlers/simple_handler.h"
#include "../handlers/simple_app.h"
#include <iostream>

#include "../../include/core/PendingAuthRequest.h"
#include "../../include/core/PaidContentCache.h"
#include "../../include/core/TabManager.h"

// Forward declaration
class AsyncWalletResourceHandler;

// g_pendingModalDomain kept as a quick-check for the overlay JS — will be
// removed once the notification UI (Phase 2.3) handles requestIds natively.
std::string g_pendingModalDomain = "";
#include <sstream>
#include <algorithm>
#include <vector>
#include <utility>
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

        HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", hodos::WalletPort(), 0);
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

        std::string url = hodos::WalletUrl("/domain/permissions?domain=") + domain;
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

        HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", hodos::WalletPort(), 0);
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
        HttpResponse resp = SyncHttpClient::Get(hodos::WalletUrl("/wallet/status"),
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

        HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", hodos::WalletPort(), 0);
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
        HttpResponse resp = SyncHttpClient::Get(hodos::WalletUrl("/wallet/bsv-price"), 1000);
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

    HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", hodos::WalletPort(), 0);
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

    std::string url = hodos::WalletUrl("/domain/permissions/certificate?domain=")
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

} // anonymous namespace

// Thin entry points for simple_handler.cpp's IPC dispatchers (declared in
// include/core/HttpRequestInterceptor.h).
//
// Phase 2.6-C.4 follow-up — after updating the C++-local session cache, fire
// a one-shot POST to /wallet/session-approve on a worker thread so the Rust
// PermissionService session cache also picks up the approval. The Rust cache
// is what `build_privacy_perimeter_context` reads on subsequent calls; without
// this echo, "Allow once" would only suppress one C++ inline check (now
// deleted in C.4 — so without the echo it would suppress zero) and the next
// call from the same origin would re-prompt. Fire-and-forget: a brief race
// window exists between the cache update here and Rust receiving the POST
// (sub-millisecond on localhost), which is acceptable for a session-scope
// cache populated by a human-time interaction.
namespace {
void fireSessionApproveToRust(const std::string& domain, const char* kind) {
    if (domain.empty()) return;
    std::string body = std::string("{\"domain\":\"") + domain + "\",\"kind\":\"" + kind + "\"}";
    CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](
        std::string body
    ) {
        std::map<std::string, std::string> headers;
        headers["Content-Type"] = "application/json";
        HttpResponse resp = SyncHttpClient::Post(
            hodos::WalletUrl("/wallet/session-approve"),
            body, headers, /*timeoutMs=*/3000);
        if (!resp.success || resp.statusCode < 200 || resp.statusCode >= 300) {
            LOG_DEBUG_HTTP(std::string("🛡️ session-approve POST failed (statusCode=")
                + std::to_string(resp.statusCode) + ") — Rust session cache "
                + "may not be updated; harmless until next call from same origin");
        }
    }, body));
}

// Phase 2.6-E — drop Rust payment session counters for a browser_id.
// Called fire-and-forget from TabManager::CloseTab when a tab closes so
// reopening the same domain in a new tab starts with fresh counters
// (mirrors C++'s SessionManager::clearSession). Idempotent — Rust returns
// 200 even for an unknown browser_id.
void fireSessionCloseToRust(int browserId) {
    std::string body = std::string("{\"browser_id\":") + std::to_string(browserId) + "}";
    CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](
        std::string body
    ) {
        std::map<std::string, std::string> headers;
        headers["Content-Type"] = "application/json";
        HttpResponse resp = SyncHttpClient::Post(
            hodos::WalletUrl("/wallet/session/close"),
            body, headers, /*timeoutMs=*/3000);
        if (!resp.success || resp.statusCode < 200 || resp.statusCode >= 300) {
            LOG_DEBUG_HTTP(std::string("🛡️ session/close POST failed (statusCode=")
                + std::to_string(resp.statusCode) + ") — Rust payment counters "
                + "may still hold stale state; harmless until same browser_id "
                + "fires another payment (rare in practice — closed tab is gone)");
        }
    }, body));
}

// Phase 2.6-C.4 follow-up — drop both Rust session caches for a domain.
// Called from revokeIdentityKeyApprovalForDomain / revokeKeyLinkageApprovalForDomain
// (both fire on the same `domain_permission_invalidate` IPC chain on the C++
// side, so back-to-back POSTs here are intentional and idempotent on the
// Rust side).
void fireSessionRevokeToRust(const std::string& domain) {
    if (domain.empty()) return;
    std::string body = std::string("{\"domain\":\"") + domain + "\"}";
    CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](
        std::string body
    ) {
        std::map<std::string, std::string> headers;
        headers["Content-Type"] = "application/json";
        HttpResponse resp = SyncHttpClient::Post(
            hodos::WalletUrl("/wallet/session-revoke"),
            body, headers, /*timeoutMs=*/3000);
        if (!resp.success || resp.statusCode < 200 || resp.statusCode >= 300) {
            LOG_DEBUG_HTTP(std::string("🛡️ session-revoke POST failed (statusCode=")
                + std::to_string(resp.statusCode) + ") — Rust session cache "
                + "may still hold stale opt-in; next call from origin would "
                + "silently pass instead of re-prompting");
        }
    }, body));
}
} // namespace

// Phase 2.6-H.2 — the C++ session-opt-in caches were deleted; Rust now owns the
// per-domain identity-key / key-linkage session approvals
// (PermissionService::{identity_key,key_linkage}_session_approvals), populated
// by these /wallet/session-approve POSTs and read by the privacy-perimeter gate.
void MarkIdentityKeyRevealApproved(const std::string& domain) {
    fireSessionApproveToRust(domain, "identity_key");
}
void MarkKeyLinkageRevealApproved(const std::string& domain) {
    fireSessionApproveToRust(domain, "key_linkage");
}

// Phase 2.6-E — public entry point for TabManager's tab-close path.
// Drops Rust's payment session counters for the closing browser_id so a
// reopened tab on the same domain starts with fresh caps. Mirrors C++'s
// `SessionManager::clearSession` which fires immediately above this call
// in TabManager::CloseTab.
void ClearRustPaymentSessionForBrowser(int browserId) {
    fireSessionCloseToRust(browserId);
}

// Phase 2.6-E cap-modal auto-resume — read the current identityKeyDisclosureAllowed
// flag from the DomainPermissionCache row for `domain` so the cap-modal
// modifyLimits handler in simple_handler.cpp can preserve it when posting an
// updated perm row. Returns the default (false) when no cache row exists.
// Defined here because DomainPermissionCache lives in this translation unit.
bool GetDomainIdentityKeyDisclosureAllowed(const std::string& domain) {
    DomainPermissionCache::Permission perm =
        DomainPermissionCache::GetInstance().getPermission(domain);
    return perm.identityKeyDisclosureAllowed;
}

// Phase 2.5 Commit 6 (sub-step 6.b) — single source of truth for the
// post-success auto-approved-payment cluster. See header for design intent.
//
// 6.d.BE+1: removed `cents <= 0` guard. The legacy AsyncHTTPClient::OnRequestComplete
// block fired the indicator on any auto-approved success regardless of cents
// (matches BRC-121 firePaymentSuccessIpc). For payments under ~16,667 sats at
// current BSV price, cents rounds to 0 — but the user still expects the gold-pill
// tab animation as their visual safeguard against silent payment abuse. The React
// renderer handles 0-cent display ("< $0.01"). Guard now is just wasAutoApprovedPayment.
void OnWalletCallSuccess(int browserId,
                         const std::string& domain,
                         int64_t cents,
                         bool wasAutoApprovedPayment,
                         const std::string& endpoint) {
    if (!wasAutoApprovedPayment) return;

    // OQ5 — session spend is now recorded in Rust at payment-decision time
    // (dispatch_payment on Silent / X-User-Approved replay) for both
    // createAction and BRC-121. The former C++ SessionManager::recordSpending
    // was removed here; this helper keeps ONLY the gold-pill IPC below.

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

// Phase 2.5 Commit 6 sub-step 6.d — exact-or-port-suffix check for internal
// origins. Replaces the prior `requestDomain_.find("127.0.0.1") == 0` prefix
// match which had a defense-in-depth weakness: any hostname starting with
// "127.0.0.1" (e.g. attacker-registered "127.0.0.1.evil.com") would bypass
// the engine. Rust's check_domain_approved still caught it as a backstop,
// but defense-in-depth degraded. This tightened check matches exactly the
// two trusted prefixes and only when followed by ':' or end-of-string.
//
// Matches:  "127.0.0.1"  "127.0.0.1:31301"  "localhost"  "localhost:5137"  ""
// Rejects:  "127.0.0.1.evil.com"  "localhost.evil.com"  "localhostevil.com"
//
// Empty origin is treated as internal (matches existing behavior — empty
// request domain falls back to wallet-internal trust).
bool IsInternalOrigin(const std::string& origin) {
    if (origin.empty()) return true;
    auto matchesHostOrHostColon = [&origin](const std::string& host) -> bool {
        if (origin.size() < host.size()) return false;
        if (origin.compare(0, host.size(), host) != 0) return false;
        // Exact match OR followed by ':' (for port suffix).
        return origin.size() == host.size() || origin[host.size()] == ':';
    };
    return matchesHostOrHostColon("127.0.0.1") || matchesHostOrHostColon("localhost");
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
//
// Phase 2.6-C.3: if resume.isInternalResume is set, override resumeKind to
// kInternal — handler / frame fields stay populated so resumeInternalResponse
// can dispatch resolution to whichever path is wired (HTTP handler or IPC
// frame). The kInternal arm exists for logging clarity per kickoff Q6.
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
    if (resume.isInternalResume) {
        req.resumeKind = ResumeKind::kInternal;
    } else if (resume.handler) {
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
    req.originalIpcRequestId = resume.originalIpcRequestId;
    return req;
}

std::string openDomainApprovalModal(const ModalContext& ctx, const ResumeContext& resume) {
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
        return requestId;
    }

    CefPostTask(TID_UI, new CreateNotificationOverlayTask("domain_approval", ctx.domain));
    LOG_DEBUG_HTTP("🔒 Domain approval needed for: " + ctx.domain
                   + " requesting " + ctx.method + " " + ctx.endpoint);
    return requestId;
}

std::string openBRC100AuthApprovalModal(const ModalContext& ctx, const ResumeContext& resume) {
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
        return requestId;
    }

    CefPostTask(TID_UI, new CreateNotificationOverlayTask("domain_approval", ctx.domain));
    LOG_DEBUG_HTTP("🔐 BRC-100 auth approval needed for: " + ctx.domain
                   + " requesting " + ctx.method + " " + ctx.endpoint);
    return requestId;
}

std::string openManifestConnectBundleModal(const ModalContext& ctx, const ResumeContext& resume,
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
        return requestId;
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
    return requestId;
}

std::string openIdentityKeyRevealModal(const ModalContext& ctx, const ResumeContext& resume) {
    LOG_DEBUG_HTTP("🛡️ Triggering identity_key_reveal for " + ctx.domain);

    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("identity_key_reveal", ctx, resume));

    CefPostTask(TID_UI, new CreateNotificationOverlayTask("identity_key_reveal", ctx.domain));
    LOG_DEBUG_HTTP("🛡️ identity_key_reveal notification queued (requestId: " + requestId + ")");
    return requestId;
}

std::string openKeyLinkageRevealModal(const ModalContext& ctx, const ResumeContext& resume) {
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
    return requestId;
}

std::string openPaymentConfirmationModal(const ModalContext& ctx, const ResumeContext& resume,
                                   const std::string& extraParams) {
    LOG_DEBUG_HTTP("💰 Triggering payment_confirmation for " + ctx.domain);
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("payment_confirmation", ctx, resume));
    CefPostTask(TID_UI, new CreateNotificationOverlayTask("payment_confirmation", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("💰 payment_confirmation notification queued (requestId: " + requestId + ")");
    return requestId;
}

std::string openRateLimitExceededModal(const ModalContext& ctx, const ResumeContext& resume,
                                 const std::string& extraParams) {
    LOG_DEBUG_HTTP("⏱️ Triggering rate_limit_exceeded for " + ctx.domain);
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("rate_limit_exceeded", ctx, resume));
    CefPostTask(TID_UI, new CreateNotificationOverlayTask("rate_limit_exceeded", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("⏱️ rate_limit_exceeded notification queued (requestId: " + requestId + ")");
    return requestId;
}

std::string openProtocolPermissionPromptModal(const ModalContext& ctx, const ResumeContext& resume,
                                        const std::string& extraParams) {
    LOG_DEBUG_HTTP("🔒 Triggering protocol_permission_prompt for " + ctx.domain);
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("protocol_permission_prompt", ctx, resume));
    CefPostTask(TID_UI, new CreateNotificationOverlayTask("protocol_permission_prompt", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("🔒 protocol_permission_prompt notification queued (requestId: " + requestId + ")");
    return requestId;
}

std::string openBasketPermissionPromptModal(const ModalContext& ctx, const ResumeContext& resume,
                                      const std::string& extraParams) {
    LOG_DEBUG_HTTP("🧺 Triggering basket_permission_prompt for " + ctx.domain);
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("basket_permission_prompt", ctx, resume));
    CefPostTask(TID_UI, new CreateNotificationOverlayTask("basket_permission_prompt", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("🧺 basket_permission_prompt notification queued (requestId: " + requestId + ")");
    return requestId;
}

std::string openCounterpartyPermissionPromptModal(const ModalContext& ctx, const ResumeContext& resume,
                                            const std::string& extraParams) {
    LOG_DEBUG_HTTP("🤝 Triggering counterparty_permission_prompt for " + ctx.domain);
    std::string requestId = PendingRequestManager::GetInstance().addRequest(
        buildPendingAuthRequest("counterparty_permission_prompt", ctx, resume));
    CefPostTask(TID_UI, new CreateNotificationOverlayTask("counterparty_permission_prompt", ctx.domain, extraParams));
    LOG_DEBUG_HTTP("🤝 counterparty_permission_prompt notification queued (requestId: " + requestId + ")");
    return requestId;
}

std::string openCertificateDisclosureModal(const ModalContext& ctx, const ResumeContext& resume,
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
    return requestId;
}

std::string OpenPromptModal(const std::string& promptType,
                            const ModalContext& ctx,
                            const ResumeContext& resume,
                            const std::string& extraParams) {
    if      (promptType == "domain_approval")                return openDomainApprovalModal(ctx, resume);
    else if (promptType == "brc100_auth")                    return openBRC100AuthApprovalModal(ctx, resume);
    else if (promptType == "identity_key_reveal")            return openIdentityKeyRevealModal(ctx, resume);
    else if (promptType == "key_linkage_reveal")             return openKeyLinkageRevealModal(ctx, resume);
    else if (promptType == "payment_confirmation")           return openPaymentConfirmationModal(ctx, resume, extraParams);
    else if (promptType == "rate_limit_exceeded")            return openRateLimitExceededModal(ctx, resume, extraParams);
    else if (promptType == "protocol_permission_prompt")     return openProtocolPermissionPromptModal(ctx, resume, extraParams);
    else if (promptType == "basket_permission_prompt")       return openBasketPermissionPromptModal(ctx, resume, extraParams);
    else if (promptType == "counterparty_permission_prompt") return openCounterpartyPermissionPromptModal(ctx, resume, extraParams);

    LOG_WARNING_HTTP("OpenPromptModal: unknown promptType '" + promptType + "' for " + ctx.domain);
    // Note: manifest_connect_bundle and certificate_disclosure are NOT in this
    // dispatcher — they require typed payloads (Manifest / CertDisclosureInfo).
    // Callers invoke their openers directly.
    return "";
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

    // (Removed in Phase 2.5-C sub-step 6.f: triggerPaymentConfirmationModal —
    // DEAD since 5.b inlined the payment-modal dispatch into the openModal
    // lambda. No callers since 5.b commit `e8168d6`. Use
    // openPaymentConfirmationModal at file scope for the modern entry point.)

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
    int64_t getPreCalculatedSatoshis() const { return preCalculatedSatoshis_; }
    bool getPreCalculatedBsvPriceAvailable() const { return preCalculatedBsvPriceAvailable_; }
    int getBrowserId() const { return browser_ ? browser_->GetIdentifier() : 0; }
    const std::string& getRequestDomain() const { return requestDomain_; }
    // Phase 2.6-C.3 — needed by AsyncHTTPClient::OnRequestComplete to build
    // the ModalContext on a 202 PENDING response from Rust.
    const std::string& getMethod() const { return method_; }
    const std::string& getEndpoint() const { return endpoint_; }
    const std::string& getBody() const { return body_; }

    // Phase 2.6-C.3 — promoted to public so tryHandlePendingResponse can arm
    // the prompt-modal timeout when intercepting a 202 PENDING from Rust on
    // the HTTP path (matches what the inline cascade does inline today).
    void postAuthTimeout(int delayMs, const std::string& errorJson);

private:
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

    // Auto-approve engine: pre-calculated spending for this request.
    // Phase 2.6-E — populated in Open() for payment endpoints from
    // BSVPriceCache + extractOutputSatoshis(body_); read by
    // startAsyncHTTPRequest to inject X-Payment-* headers + by
    // AsyncHTTPClient::OnRequestComplete to derive cents for the
    // payment_success_indicator IPC (green-dot animation).
    //
    // Pre-2.6-E this struct also tracked `wasAutoApprovedPayment_`, which the
    // deleted C++ payment cascade set to true on Silent. That signal is now
    // local-only and derived from `isPaymentEndpoint(endpoint_) && UR_SUCCESS
    // && !response_has_error` in OnRequestComplete (LD4: derivation stays C++).
    int64_t preCalculatedCents_ = 0;
    int64_t preCalculatedSatoshis_ = 0;
    bool preCalculatedBsvPriceAvailable_ = false;

    // Browser reference for modal triggering
    CefRefPtr<CefBrowser> browser_;

    // CEF request management
    CefRefPtr<CefURLRequest> urlRequest_;
    CefRefPtr<CefCallback> readCallback_;

    IMPLEMENT_REFCOUNTING(AsyncWalletResourceHandler);
    DISALLOW_COPY_AND_ASSIGN(AsyncWalletResourceHandler);
};

// ============================================================================
// Phase 2.6-C.3 — forward declarations for the 202 PENDING envelope handler.
// Definitions live near resumeIpcResponse (~L3760). The IPC bridge silent-
// forward worker below needs to call tryHandlePendingResponse on TID_UI, so
// the symbol must be visible at that earlier file position. File scope so the
// forward decls match the definitions' internal linkage.
// ============================================================================
struct PendingEnvelope;  // defined alongside the helpers
static bool tryHandlePendingResponse(int statusCode,
                                     const std::string& responseBody,
                                     const ModalContext& modalCtx,
                                     ResumeContext resume);

// ============================================================================
// Phase 2.5 Commit 6 sub-step 6.d.A — IPC bridge engine cascade helpers
// ============================================================================
//
// HandleIpcWalletCall (declared in HttpRequestInterceptor.h) runs the full
// engine cascade on the IPC path. Internal-origin requests take a fast-path
// bypass mirroring AsyncWalletResourceHandler::Open()'s IsInternalOrigin
// shortcut.
//
// Threading recap (from COMMIT_6_DESIGN.md §2):
//   - HandleIpcWalletCall runs on TID_UI (where IPC arrives)
//   - SyncHttpClient calls happen on TID_FILE_USER_BLOCKING workers
//   - All CefFrame methods (SendProcessMessage) must run on TID_UI
//   - Modal dispatch (CreateNotificationOverlayTask) posts to TID_UI
//
// All file-scope free functions in this block are static (internal linkage).

namespace {

// Payloads at or under this take the original single-message path UNCHANGED.
// Larger ones (e.g. /wallet/export of an active wallet) are chunked so neither
// the CEF process-message size limit nor the render-thread ExecuteJavaScript
// compile ever sees the whole multi-MB body at once. 256 KB keeps each chunk's
// escaped JS source small while keeping the chunk count low.
// See WALLET_UI_BRIDGE_MIGRATION.md §4 (Rule 3).
constexpr size_t kWalletResponseChunkBytes = 256 * 1024;

// Send wallet_response IPC back to the calling frame. UI thread only.
// Drops silently if frame is no longer valid (frame navigated, tab closed).
void sendWalletResponseIpc(CefRefPtr<CefFrame> frame,
                            const std::string& requestId,
                            bool ok,
                            const std::string& payload) {
    if (!frame || !frame->IsValid()) {
        LOG_DEBUG_HTTP("wallet_response dropped — frame invalid for " + requestId);
        return;
    }

    // Small-payload fast path — byte-identical to the original. The vast majority
    // of wallet responses (balance, status, activity pages) are far under the cap.
    if (payload.size() <= kWalletResponseChunkBytes) {
        CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("wallet_response");
        auto args = response->GetArgumentList();
        args->SetString(0, requestId);
        args->SetBool(1, ok);
        args->SetString(2, payload);
        frame->SendProcessMessage(PID_RENDERER, response);
        return;
    }

    // Large-payload chunked path. Each chunk ends on a UTF-8 character boundary so
    // per-chunk JS-string escaping + reassembly is lossless. The render side
    // forwards each chunk to window.__hodos_walletResponseChunk, which reassembles
    // by requestId and resolves the pending promise ONLY when all chunks have
    // arrived AND the reassembled byte length matches — otherwise it rejects
    // (never a silent truncation). frame validity was checked above; we are on the
    // UI thread for the whole loop so it cannot be invalidated mid-send.
    std::vector<std::pair<size_t, size_t>> spans;  // (offset, length) per chunk
    {
        const size_t n = payload.size();
        size_t pos = 0;
        while (pos < n) {
            size_t end = (std::min)(pos + kWalletResponseChunkBytes, n);
            if (end < n) {
                // Back off any UTF-8 continuation byte (10xxxxxx) so a multibyte
                // sequence is never split across two chunks.
                while (end > pos && (static_cast<unsigned char>(payload[end]) & 0xC0) == 0x80) {
                    --end;
                }
                // Degenerate guard (cannot happen for valid UTF-8): a full chunk of
                // continuation bytes. Force progress so we never loop forever.
                if (end == pos) {
                    end = (std::min)(pos + kWalletResponseChunkBytes, n);
                }
            }
            spans.emplace_back(pos, end - pos);
            pos = end;
        }
    }

    const int total = static_cast<int>(spans.size());
    const std::string totalLen = std::to_string(payload.size());
    LOG_DEBUG_HTTP("wallet_response chunked: " + requestId + " " + totalLen +
                   " bytes in " + std::to_string(total) + " chunks");
    for (int i = 0; i < total; ++i) {
        CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("wallet_response_chunk");
        auto args = msg->GetArgumentList();
        args->SetString(0, requestId);
        args->SetBool(1, ok);
        args->SetInt(2, i);
        args->SetInt(3, total);
        args->SetString(4, totalLen);  // string to avoid 32-bit int overflow
        args->SetString(5, payload.substr(spans[i].first, spans[i].second));
        frame->SendProcessMessage(PID_RENDERER, msg);
    }
}

// Dispatch wallet HTTP call by method string. Worker thread only.
HttpResponse dispatchWalletHttpByMethod(const std::string& httpMethod,
                                         const std::string& url,
                                         const std::string& bodyJson,
                                         const std::map<std::string, std::string>& headers) {
    if (httpMethod == "GET") {
        return SyncHttpClient::Get(url, headers, /*timeoutMs=*/30000);
    } else if (httpMethod == "POST") {
        return SyncHttpClient::Post(url, bodyJson, headers, /*timeoutMs=*/30000);
    } else {
        return SyncHttpClient::Request(httpMethod, url, bodyJson, headers, /*timeoutMs=*/30000);
    }
}

// Build the wallet_response payload string from an HttpResponse. Successful
// response: pass body verbatim (already JSON). Error: wrap in a minimal
// envelope if body is empty, else pass body (typically already JSON error).
std::string buildIpcResponsePayload(const HttpResponse& resp, bool ok) {
    if (ok) return resp.body.empty() ? std::string("null") : resp.body;
    if (resp.body.empty()) {
        return std::string("{\"error\":\"HTTP ") + std::to_string(resp.statusCode) +
               "\",\"status\":" + std::to_string(resp.statusCode) + "}";
    }
    return resp.body;
}

// Internal-origin bypass path — direct HTTP call, no engine. Matches today's
// pre-Commit-6 IPC handler behavior for localhost/127.0.0.1 origins.
void runIpcCallDirect(const std::string& requestId,
                      const std::string& /*methodName*/,
                      const std::string& endpoint,
                      const std::string& bodyJson,
                      const std::string& httpMethod,
                      const std::string& origin,
                      CefRefPtr<CefFrame> capturedFrame,
                      int browserId) {
    (void)browserId;  // unused on the direct path
    CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](
        std::string requestId, std::string endpoint,
        std::string bodyJson, std::string httpMethod, std::string origin,
        CefRefPtr<CefFrame> capturedFrame
    ) {
        std::map<std::string, std::string> headers;
        headers["Content-Type"] = "application/json";
        // Bridge migration (WALLET_UI_BRIDGE_MIGRATION.md §2): runIpcCallDirect is
        // the INTERNAL dispatch path — HandleIpcWalletCall routes only
        // IsInternalOrigin() callers here. Internal == wallet-internal (the
        // first-party UI served from 127.0.0.1:5137, or any loopback caller), and
        // it MUST reach Rust header-free so the X-Requesting-Domain trust gate
        // treats it as internal. Stamping the loopback origin here is exactly the
        // "naive break" that 403s wallet_backup/wallet_export and prompts the user
        // for their own identity key. Stamp ONLY a genuine external origin
        // (defensive — should never occur on this path).
        if (!origin.empty() && !IsInternalOrigin(origin))
            headers["X-Requesting-Domain"] = origin;

        std::string url = hodos::WalletBaseUrl() + endpoint;
        HttpResponse resp = dispatchWalletHttpByMethod(httpMethod, url, bodyJson, headers);
        bool ok = resp.success && resp.statusCode >= 200 && resp.statusCode < 300;
        std::string payload = buildIpcResponsePayload(resp, ok);

        CefPostTask(TID_UI, base::BindOnce([](
            std::string requestId, bool ok, std::string payload,
            CefRefPtr<CefFrame> capturedFrame
        ) {
            sendWalletResponseIpc(capturedFrame, requestId, ok, payload);
        }, requestId, ok, payload, capturedFrame));
    }, requestId, endpoint, bodyJson, httpMethod, origin, capturedFrame));
}

// Unknown-trust path — fetch manifest on a worker thread, then dispatch the
// appropriate modal on the UI thread (manifest_connect_bundle if declared
// permissions exist; domain_approval fallback). Mirrors Open()'s L1923-1971.
void handleIpcUnknownTrust(const std::string& requestId,
                            const std::string& methodName,
                            const std::string& endpoint,
                            const std::string& bodyJson,
                            const std::string& httpMethod,
                            const std::string& origin,
                            CefRefPtr<CefFrame> capturedFrame,
                            int browserId) {
    CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](
        std::string requestId, std::string methodName, std::string endpoint,
        std::string bodyJson, std::string httpMethod, std::string origin,
        CefRefPtr<CefFrame> capturedFrame, int browserId
    ) {
        hodos::Manifest manifest = hodos::ManifestFetcher::Fetch(origin);
        const bool hasDeclaredPerms = manifest.valid
            && (!manifest.protocols.empty()
                || !manifest.baskets.empty()
                || !manifest.certificates.empty()
                || !manifest.counterparties.empty()
                || manifest.spending.perTransactionUsd > 0);

        CefPostTask(TID_UI, base::BindOnce([](
            std::string requestId, std::string methodName, std::string endpoint,
            std::string bodyJson, std::string httpMethod, std::string origin,
            CefRefPtr<CefFrame> capturedFrame, int browserId,
            hodos::Manifest manifest, bool hasDeclaredPerms
        ) {
            ModalContext mctx{origin, methodName, endpoint, bodyJson};
            ResumeContext resume;
            resume.frame = capturedFrame;
            resume.browserId = browserId;
            resume.httpMethod = httpMethod;
            // Phase 2.6-C.5 fix — propagate page-supplied IPC requestId so
            // resumeIpcResponse delivers wallet_response with the id the
            // page's CWI shim is waiting on.
            resume.originalIpcRequestId = requestId;

            std::string newRequestId;
            if (hasDeclaredPerms) {
                LOG_DEBUG_HTTP("📦 IPC: Manifest found for " + origin
                                + " — firing manifest_connect_bundle prompt");
                newRequestId = openManifestConnectBundleModal(mctx, resume, manifest);
            } else {
                LOG_DEBUG_HTTP("🔒 IPC: No usable manifest for " + origin
                                + " — firing domain_approval prompt");
                newRequestId = openDomainApprovalModal(mctx, resume);
            }
            if (!newRequestId.empty()) {
                postIpcAuthTimeout(newRequestId, capturedFrame,
                    "{\"error\":\"Approval timeout\",\"status\":\"error\"}",
                    kPromptAuthTimeoutMs);
            }
        }, requestId, methodName, endpoint, bodyJson, httpMethod, origin,
           capturedFrame, browserId, manifest, hasDeclaredPerms));
    }, requestId, methodName, endpoint, bodyJson, httpMethod, origin, capturedFrame, browserId));
}

// IPC wallet-call forwarder (thin proxy). Computes payment context
// (satoshis/cents) for the X-Payment-* headers, then forwards every external
// IPC call to Rust, which is the single source of truth for all permission
// gates. Rust returns 200 / 202 (modal prompt) / 403; a 202 hops to TID_UI to
// open the matching modal. (Phase 2.6-H removed the old C++ engine cascade
// this function used to run after the forward.)
void runIpcEngineCascade(const std::string& requestId,
                          const std::string& methodName,
                          const std::string& endpoint,
                          const std::string& bodyJson,
                          const std::string& httpMethod,
                          const std::string& origin,
                          CefRefPtr<CefFrame> capturedFrame,
                          int browserId,
                          const DomainPermissionCache::Permission& perm) {
    using AWRH = AsyncWalletResourceHandler;

    // Compute payment context if this is a payment endpoint (matches 5.b).
    const bool isPaymentKind = AWRH::isPaymentEndpoint(endpoint);
    int64_t satoshis = 0;
    double bsvPrice = 0;
    bool priceAvailable = false;
    int64_t cents = 0;
    if (isPaymentKind) {
        satoshis = AWRH::extractOutputSatoshis(bodyJson);
        bsvPrice = BSVPriceCache::GetInstance().getPrice();
        priceAvailable = (bsvPrice > 0);
        if (priceAvailable && satoshis > 0) {
            cents = static_cast<int64_t>(
                (static_cast<double>(satoshis) / 100000000.0) * bsvPrice * 100.0);
        }
    }

    // Phase 2.6-G — C++ is a thin proxy. Domain-trust runs as a Rust
    // middleware and the per-handler kind gates (payment/scoped/cert/privacy)
    // all run in Rust, so EVERY external IPC call forwards to Rust
    // unconditionally; Rust returns 200 / 202 (connect or kind prompt) / 403.
    // The worker hops to TID_UI on 202 to open the matching modal via
    // tryHandlePendingResponse → resumeInternalResponse on user resolution.
    //
    // For Payment, C++ injects X-Browser-Id + X-Payment-Satoshis +
    // X-Payment-Cents + X-Bsv-Price-Available so Rust's dispatch_payment can
    // build the PermissionContext server-side. LD4 keeps satoshi/cents
    // derivation on the C++ side (BSVPriceCache) — values computed above.
    //
    // Phase 2.6-H — the old C++ engine cascade (buildPermissionContext +
    // RunPermissionGate + shadow) that used to follow this block has been
    // deleted along with the rest of the C++ PermissionEngine.
    {
        LOG_DEBUG_HTTP("🚪 IPC: forward to Rust (thin proxy, all kinds) for "
                       + origin + " endpoint=" + endpoint);
        CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](
            std::string requestId, std::string endpoint, std::string bodyJson,
            std::string httpMethod, std::string origin,
            CefRefPtr<CefFrame> capturedFrame, int browserId,
            bool isPaymentKind, int64_t satoshis, int64_t cents, bool priceAvailable
        ) {
            std::map<std::string, std::string> headers;
            headers["Content-Type"] = "application/json";
            if (!origin.empty()) headers["X-Requesting-Domain"] = origin;
            if (isPaymentKind) {
                headers["X-Browser-Id"] = std::to_string(browserId);
                headers["X-Payment-Satoshis"] = std::to_string(satoshis);
                headers["X-Payment-Cents"] = std::to_string(cents);
                headers["X-Bsv-Price-Available"] = priceAvailable ? "1" : "0";
            }
            std::string url = hodos::WalletBaseUrl() + endpoint;
            HttpResponse resp = dispatchWalletHttpByMethod(httpMethod, url, bodyJson, headers);

            if (resp.statusCode == 202) {
                std::string responseBody = resp.body;
                CefPostTask(TID_UI, base::BindOnce([](
                    std::string requestId, std::string origin, std::string endpoint,
                    std::string bodyJson, std::string httpMethod, std::string responseBody,
                    CefRefPtr<CefFrame> capturedFrame, int browserId
                ) {
                    ModalContext modalCtx{origin, httpMethod, endpoint, bodyJson};
                    ResumeContext resume;
                    resume.frame = capturedFrame;
                    resume.browserId = browserId;
                    resume.httpMethod = httpMethod;
                    // Phase 2.6-C.5 fix — propagate page-supplied IPC requestId
                    // so resumeInternalResponse delivers wallet_response with
                    // the id the page's CWI shim is waiting on.
                    resume.originalIpcRequestId = requestId;
                    if (!tryHandlePendingResponse(202, responseBody, modalCtx, resume)) {
                        LOG_WARNING_HTTP("🛡️ IPC Rust-authoritative 202 from Rust failed envelope handling — forwarding raw body");
                        sendWalletResponseIpc(capturedFrame, requestId, true, responseBody);
                    }
                }, requestId, origin, endpoint, bodyJson, httpMethod, responseBody,
                   capturedFrame, browserId));
                return;
            }

            bool ok = resp.success && resp.statusCode >= 200 && resp.statusCode < 300;
            std::string payload = buildIpcResponsePayload(resp, ok);

            // Phase 2.6-E — fire green-dot animation IPC on auto-approved
            // payment success (mirrors the HTTP path's
            // AsyncHTTPClient::OnRequestComplete derivation). Local-only:
            // payment endpoint + UR_SUCCESS + no "error" in response body.
            bool isErrorInResponse = false;
            if (ok && isPaymentKind) {
                try {
                    auto rj = nlohmann::json::parse(payload);
                    isErrorInResponse = rj.contains("error");
                } catch (...) {}
            }
            const bool wasAutoApprovedPayment =
                ok && isPaymentKind && !isErrorInResponse;

            CefPostTask(TID_UI, base::BindOnce([](
                std::string requestId, bool ok, std::string payload,
                CefRefPtr<CefFrame> capturedFrame, int browserId,
                std::string origin, std::string endpoint, int64_t cents,
                bool wasAutoApprovedPayment
            ) {
                if (wasAutoApprovedPayment) {
                    OnWalletCallSuccess(browserId, origin, cents, true, endpoint);
                }
                sendWalletResponseIpc(capturedFrame, requestId, ok, payload);
            }, requestId, ok, payload, capturedFrame, browserId,
               origin, endpoint, cents, wasAutoApprovedPayment));
        }, requestId, endpoint, bodyJson, httpMethod, origin, capturedFrame, browserId,
           isPaymentKind, satoshis, cents, priceAvailable));
        return;
    }

}

} // anonymous namespace — IPC bridge helpers

// Phase 2.5 Commit 6 sub-step 6.d.A — IPC-side auth timeout helper.
// See header for design intent. Mirrors AsyncWalletResourceHandler::postAuthTimeout
// for the IPC path (no handler instance to call back).
void postIpcAuthTimeout(const std::string& requestId,
                        CefRefPtr<CefFrame> frame,
                        const std::string& errorJson,
                        int delayMs) {
    CefPostDelayedTask(TID_UI, base::BindOnce([](
        std::string requestId, CefRefPtr<CefFrame> frame, std::string errorJson
    ) {
        // Only fire if the request is still pending (user hasn't resolved yet).
        // popRequest is atomic — only one of (approve, deny, timeout) wins.
        PendingAuthRequest req;
        if (!PendingRequestManager::GetInstance().popRequest(requestId, req)) return;
        sendWalletResponseIpc(frame, requestId, false, errorJson);
        LOG_DEBUG_HTTP("⏰ IPC auth timeout fired for " + requestId);
    }, requestId, frame, errorJson), delayMs);
}

// Phase 2.5 Commit 6 sub-step 6.d.A — top-level wallet_call IPC dispatch.
// See header for design intent. Called from simple_handler.cpp's wallet_call
// IPC handler in sub-step 6.d.B (which is when external dApp traffic first
// flows through the engine).
void HandleIpcWalletCall(
    const std::string& requestId,
    const std::string& methodName,
    const std::string& endpoint,
    const std::string& bodyJson,
    const std::string& httpMethod,
    const std::string& origin,
    CefRefPtr<CefFrame> capturedFrame,
    int browserId) {

    // 1. Internal origin — bypass the engine entirely. Matches Open()'s L2112
    //    behavior (tightened to exact-or-port-suffix match in 6.d.A).
    if (IsInternalOrigin(origin)) {
        LOG_DEBUG_HTTP("🔒 IPC internal origin " + origin + " — direct dispatch");
        runIpcCallDirect(requestId, methodName, endpoint, bodyJson, httpMethod,
                         origin, capturedFrame, browserId);
        return;
    }

    // 2. No wallet — send NO_WALLET error. Matches Open()'s L2121.
    if (!WalletStatusCache::GetInstance().walletExists()) {
        LOG_DEBUG_HTTP("🔒 IPC: no wallet exists, rejecting request from " + origin);
        sendWalletResponseIpc(capturedFrame, requestId, false,
            "{\"error\":\"No wallet exists. Please create or recover a wallet first.\","
            "\"code\":\"NO_WALLET\",\"status\":\"error\"}");
        return;
    }

    // 3. Domain trust lookup.
    auto perm = DomainPermissionCache::GetInstance().getPermission(origin);
    LOG_DEBUG_HTTP("🔒 IPC: domain " + origin + " trust_level: " + perm.trustLevel);

    // Phase 2.6-G G.4 — domain-trust is now Rust-authoritative. Blocked and
    // unknown are no longer intercepted here; the call forwards to Rust where
    // domain_trust_gate (wired into every shim handler in G.3b) runs FIRST and
    // returns 403 (blocked) or 202 domain_approval / manifest_connect_bundle
    // (unknown). The forward worker hops to TID_UI on 202 →
    // tryHandlePendingResponse, which opens the matching modal — the same path
    // the payment/scoped/cert prompts already use. (Pre-G.4 this rejected
    // blocked inline and ran handleIpcUnknownTrust for a C++-side manifest
    // fetch; both are superseded. handleIpcUnknownTrust is now dead code,
    // slated for removal in 2.6-H.)
    runIpcEngineCascade(requestId, methodName, endpoint, bodyJson, httpMethod,
                        origin, capturedFrame, browserId, perm);
}

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

    // Internal overlays (wallet panel, settings, etc.) are trusted — skip domain check.
    // Phase 2.5 Commit 6 sub-step 6.d — uses IsInternalOrigin (exact-or-port-suffix
    // match) instead of the prior prefix match that let "127.0.0.1.evil.com"
    // through. Same helper is called by the IPC bridge in HandleIpcWalletCall.
    if (IsInternalOrigin(requestDomain_)) {
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

    // Phase 2.6-G — C++ Open() is now a THIN PROXY. Domain-trust and all kind
    // gates (payment / scoped / cert / privacy) are Rust-authoritative: Rust's
    // domain-trust middleware runs on every external call and returns 200 /
    // 202 (connect prompt OR kind prompt) / 403. A 202 is intercepted in
    // AsyncHTTPClient::OnRequestComplete -> tryHandlePendingResponse, which
    // opens the matching modal; a 200 payment fires the gold pill there too.
    // So we forward EVERY external origin to Rust regardless of trust level.
    //
    // The DomainPermissionCache lookup is kept only for the ancillary BRC-100
    // auth-handshake modal below (a distinct login UX, not the engine's
    // domain_approval) — per the plan, the cache lingers for ancillary uses +
    // IsInternalOrigin even after the engine path stops consulting it.
    auto perm = DomainPermissionCache::GetInstance().getPermission(requestDomain_);
    LOG_DEBUG_HTTP("🚪 Open: domain " + requestDomain_ + " trust_level: "
                   + perm.trustLevel + " — forwarding to Rust (thin proxy)");

    if (perm.trustLevel == "unknown"
        && endpoint_.find("/brc100/auth/") != std::string::npos) {
        LOG_DEBUG_HTTP("🔐 BRC-100 auth request from unknown domain: " + requestDomain_);
        triggerBRC100AuthApprovalModal(requestDomain_, method_, endpoint_, body_, this);
        postAuthTimeout(kPromptAuthTimeoutMs, "{\"error\":\"Approval timeout\",\"status\":\"error\"}");
        handle_request = true;
        return true;
    }

    // LD4: derive + stash payment cost for payment endpoints BEFORE forwarding,
    // so the eventual re-issue (after a connect prompt) still carries X-Payment-*
    // for Rust's payment gate and the gold-pill indicator fires. Done for every
    // trust level since the first call may be a connect that re-issues later.
    if (isPaymentEndpoint(endpoint_)) {
        int64_t satoshis = extractOutputSatoshis(body_);
        double bsvPrice = BSVPriceCache::GetInstance().getPrice();
        const bool priceAvailable = (bsvPrice > 0);
        int64_t cents = 0;
        if (priceAvailable && satoshis > 0) {
            cents = static_cast<int64_t>((static_cast<double>(satoshis) / 100000000.0) * bsvPrice * 100.0);
        }
        preCalculatedCents_ = cents;
        preCalculatedSatoshis_ = satoshis;
        preCalculatedBsvPriceAvailable_ = priceAvailable;
    }

    handle_request = true;
    CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(this));
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
    DomainPermissionTask(const std::string& domain, bool identityKeyDisclosureAllowed = false,
                        bool bundledScopeGrant = false)
        : domain_(domain), identityKeyDisclosureAllowed_(identityKeyDisclosureAllowed),
          bundledScopeGrant_(bundledScopeGrant) {}

    void Execute() override {
        LOG_DEBUG_HTTP("🔐 DomainPermissionTask executing on UI thread for domain: " + domain_);

        // Create request
        CefRefPtr<CefRequest> cefRequest = CefRequest::Create();
        cefRequest->SetURL(hodos::WalletUrl("/domain/permissions"));
        cefRequest->SetMethod("POST");
        cefRequest->SetHeaderByName("Content-Type", "application/json", true);

        // Create JSON body — Phase 1.5 Step 1 adds identityKeyDisclosureAllowed,
        // Phase 2.6-D Fix #4 adds bundledScopeGrant.
        nlohmann::json bodyJson;
        bodyJson["domain"] = domain_;
        bodyJson["trustLevel"] = "approved";
        bodyJson["identityKeyDisclosureAllowed"] = identityKeyDisclosureAllowed_;
        bodyJson["bundledScopeGrant"] = bundledScopeGrant_;
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
    bool bundledScopeGrant_;
    IMPLEMENT_REFCOUNTING(DomainPermissionTask);
    DISALLOW_COPY_AND_ASSIGN(DomainPermissionTask);
};

// Task class for creating domain permission with advanced settings (custom limits)
class AdvancedDomainPermissionTask : public CefTask {
public:
    AdvancedDomainPermissionTask(const std::string& domain, int64_t perTxLimitCents,
                                  int64_t perSessionLimitCents, int64_t rateLimitPerMin,
                                  int64_t maxTxPerSession,
                                  bool identityKeyDisclosureAllowed = false,
                                  bool bundledScopeGrant = false)
        : domain_(domain), perTxLimitCents_(perTxLimitCents),
          perSessionLimitCents_(perSessionLimitCents), rateLimitPerMin_(rateLimitPerMin),
          maxTxPerSession_(maxTxPerSession),
          identityKeyDisclosureAllowed_(identityKeyDisclosureAllowed),
          bundledScopeGrant_(bundledScopeGrant) {}

    void Execute() override {
        LOG_DEBUG_HTTP("🔐 AdvancedDomainPermissionTask executing for domain: " + domain_);

        CefRefPtr<CefRequest> cefRequest = CefRequest::Create();
        cefRequest->SetURL(hodos::WalletUrl("/domain/permissions"));
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
        body["bundledScopeGrant"] = bundledScopeGrant_;
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
    bool bundledScopeGrant_;
    IMPLEMENT_REFCOUNTING(AdvancedDomainPermissionTask);
    DISALLOW_COPY_AND_ASSIGN(AdvancedDomainPermissionTask);
};

// Function to add domain permission with advanced settings.
// Phase 1.5 Step 1: identityKeyDisclosureAllowed bundles the privacy-perimeter
// grant into the same site approval, eliminating a second prompt on first connect.
// Phase 2.6-D Fix #4: bundledScopeGrant bundles the V22
// `bundled_scope_grant` column write so the engine can silence ProtocolUse +
// BasketAccess prompts for this domain.
void addDomainPermissionAdvanced(const std::string& domain, int64_t perTxLimitCents,
                                  int64_t perSessionLimitCents, int64_t rateLimitPerMin,
                                  int64_t maxTxPerSession,
                                  bool identityKeyDisclosureAllowed,
                                  bool bundledScopeGrant) {
    LOG_DEBUG_HTTP("🔐 Adding advanced domain permission: " + domain +
        " (tx=" + std::to_string(perTxLimitCents) + ", session=" + std::to_string(perSessionLimitCents) +
        ", rate=" + std::to_string(rateLimitPerMin) + ", maxTxPerSession=" + std::to_string(maxTxPerSession) +
        ", identityKeyDisclosure=" + (identityKeyDisclosureAllowed ? "1" : "0") +
        ", bundledScopeGrant=" + (bundledScopeGrant ? "1" : "0") + ")");

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

    // Phase 2.6-G G.4 — synchronous Rust DB write (see addDomainPermission for
    // the race rationale). The manifest_connect_bundle / advanced approval path
    // drains + re-issues immediately, so trust + caps must be committed to the
    // Rust DB before domain_trust_gate reads it on the re-issue.
    nlohmann::json permBody;
    permBody["domain"] = domain;
    permBody["trustLevel"] = "approved";
    permBody["perTxLimitCents"] = perTxLimitCents;
    permBody["perSessionLimitCents"] = perSessionLimitCents;
    permBody["rateLimitPerMin"] = rateLimitPerMin;
    permBody["maxTxPerSession"] = maxTxPerSession;
    permBody["identityKeyDisclosureAllowed"] = identityKeyDisclosureAllowed;
    permBody["bundledScopeGrant"] = bundledScopeGrant;
    HttpResponse permResp = SyncHttpClient::Post(
        hodos::WalletUrl("/domain/permissions"), permBody.dump());
    LOG_DEBUG_HTTP("🔐 Advanced domain permission sync write for " + domain
        + " -> status " + std::to_string(permResp.statusCode));
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
// Phase 2.6-D Fix #4: third arg bundles the V22 bundled_scope_grant column so
// the engine can silence ProtocolUse + BasketAccess on this domain.
void addDomainPermission(const std::string& domain, bool identityKeyDisclosureAllowed,
                         bool bundledScopeGrant) {
    LOG_DEBUG_HTTP("🔐 Adding domain permission: " + domain +
        " (identityKeyDisclosure=" + (identityKeyDisclosureAllowed ? "1" : "0") +
        ", bundledScopeGrant=" + (bundledScopeGrant ? "1" : "0") + ")");

    // Set the cache immediately so the next request sees "approved" without waiting
    // for the async DB write to complete (prevents modal loop race condition)
    DomainPermissionCache::Permission perm;
    perm.trustLevel = "approved";
    perm.identityKeyDisclosureAllowed = identityKeyDisclosureAllowed;
    DomainPermissionCache::GetInstance().set(domain, perm);

    // Phase 2.6-G G.4 — synchronous Rust DB write. domain_trust_gate (now the
    // authoritative trust check) reads the Rust DB, and the add_domain_permission
    // IPC handler drains + re-issues the pending request immediately after this
    // returns. A fire-and-forget write could lose that race (re-issue reaches
    // Rust before trust=approved lands → domain_trust_gate re-prompts → loop).
    // Block here so trust is committed first. Runs on TID_UI during a one-time
    // user approval; the localhost POST is ~ms. SyncHttpClient bypasses the CEF
    // interceptor, so there is no resource-handler reentrancy.
    nlohmann::json permBody;
    permBody["domain"] = domain;
    permBody["trustLevel"] = "approved";
    permBody["identityKeyDisclosureAllowed"] = identityKeyDisclosureAllowed;
    permBody["bundledScopeGrant"] = bundledScopeGrant;
    HttpResponse permResp = SyncHttpClient::Post(
        hodos::WalletUrl("/domain/permissions"), permBody.dump());
    LOG_DEBUG_HTTP("🔐 Domain permission sync write for " + domain
        + " -> status " + std::to_string(permResp.statusCode));
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
// Phase 1.5 — session-scoped trust: domain-revoke helpers
// ============================================================================
// Phase 2.6-H.2 deleted the in-memory C++ session caches
// (IdentityKeyApprovalCache / KeyLinkageApprovalCache / SubPermissionCache).
// The per-domain identity-key / key-linkage session opt-ins now live entirely
// in Rust (PermissionService::{identity_key,key_linkage}_session_approvals);
// scoped grants live in the Rust V18 child tables. These revoke helpers just
// forward to Rust so toggling identity-key disclosure off (or any permission
// change) drops the session opt-in immediately rather than leaking until restart.
//
// Called from simple_handler.cpp's `domain_permission_invalidate` IPC, which
// fires every time a domain's permissions change (revoke, edit, advanced
// save, etc.). Dropping all session-scoped trust on the invalidate edge is
// the right default — the user just touched this domain's permissions, so
// any prior session-only opt-in should be re-confirmed.
// Phase 2.6-C.5 fix — drained-pending resume helper. Routes per resumeKind:
//   kHttpCallback → ForwardPendingWalletRequest (existing path through
//                   StartAsyncHTTPRequestTask + Open() re-run; Open() now
//                   sees the freshly-approved trust_level and falls through
//                   to the catch-all forward).
//   kIpcResponse  → resumeIpcResponse with a synthetic "approved" envelope.
//                   The function ignores the envelope on non-error and
//                   re-issues via SyncHttpClient::Post with X-Requesting-Domain.
//   kInternal     → resumeInternalResponse, same pattern.
// Returns false for entries whose shape doesn't match any resume path —
// caller treats those as BRC-121 (TriggerPendingBrc121Reloads handles them).
//
// Forward declarations: resumeIpcResponse + resumeInternalResponse are
// defined later in the file (~L4161 and ~L3934 respectively) but used here.
static void resumeIpcResponse(const PendingAuthRequest& req,
                              const std::string& responseData);
static void resumeInternalResponse(const PendingAuthRequest& req,
                                   const std::string& responseData);
bool ResumeDrainedApprovedRequest(const PendingAuthRequest& req) {
    static constexpr const char* kApprovedStub = "{\"status\":\"approved\"}";
    if (req.handler && req.resumeKind == ResumeKind::kHttpCallback) {
        return ForwardPendingWalletRequest(req.handler);
    }
    if (req.resumeKind == ResumeKind::kIpcResponse && req.frame) {
        resumeIpcResponse(req, kApprovedStub);
        return true;
    }
    if (req.resumeKind == ResumeKind::kInternal && (req.handler || req.frame)) {
        resumeInternalResponse(req, kApprovedStub);
        return true;
    }
    return false;
}

void revokeIdentityKeyApprovalForDomain(const std::string& domain) {
    // Phase 2.6-H.2 — the C++ session caches were deleted; clear the Rust
    // session cache instead. Rust's revoke_session_approvals_for_domain clears
    // BOTH identity-key and key-linkage opt-ins, and the
    // domain_permission_invalidate IPC calls both revoke functions, so this
    // fires twice — the second POST is a cheap no-op.
    fireSessionRevokeToRust(domain);
}
void revokeKeyLinkageApprovalForDomain(const std::string& domain) {
    fireSessionRevokeToRust(domain);
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

// ============================================================================
// Phase 2.6-C.3 — 202 PENDING envelope handling
// ============================================================================
//
// LD2 wire contract: Rust handlers return 202 Accepted carrying a JSON envelope
// when the permission engine wants the user to resolve a prompt. C++ intercepts
// the 202 before the response reaches the renderer, opens the matching modal,
// then on user approval re-issues the original wallet call with
// `X-User-Approved: <approvalId>` so Rust's request_gate consumes the approval
// + sha256-verifies the body + processes the call (200 OK).
//
// Used by both the HTTP path (AsyncHTTPClient::OnRequestComplete) and the IPC
// path (runIpcEngineCascade's silent-forward worker). Per kickoff Q4, the
// helper is shared between both call sites.

// Parsed 202 envelope contents per LD2 §LD2 schema (PHASE_2_6_ENGINE_TO_RUST.md).
struct PendingEnvelope {
    std::string approvalId;
    std::string promptType;
    std::string engineReason;
    int64_t ttlMs = 0;
    nlohmann::json promptPayload;  // may be absent / null / object
};

// Parse a 202 PENDING envelope from Rust. Returns true and populates `out` on
// success; false on any JSON parse error, missing required field, or non-
// "pending" status.
static bool parsePendingEnvelope(const std::string& body, PendingEnvelope& out) {
    try {
        auto j = nlohmann::json::parse(body);
        if (!j.is_object()) return false;
        auto status = j.value("status", std::string());
        if (status != "pending") return false;
        out.approvalId = j.value("approvalId", std::string());
        out.promptType = j.value("promptType", std::string());
        out.engineReason = j.value("engineReason", std::string());
        out.ttlMs = j.value("ttlMs", static_cast<int64_t>(0));
        if (j.contains("promptPayload")) out.promptPayload = j["promptPayload"];
        return !out.approvalId.empty() && !out.promptType.empty();
    } catch (...) {
        return false;
    }
}

// Build the modal opener's `extraParams` query string from the 202 envelope's
// `promptPayload`, dispatching by `promptType`. Mirrors the field shapes the
// inline cascade builds at runIpcEngineCascade L2487-2517 so the React modal
// sees the same parameter set regardless of which engine produced the prompt.
// Returns an empty string for promptTypes whose modal needs no extra params
// (identity_key_reveal, domain_approval, brc100_auth) or whose payload is
// non-string-encodable (certificate_disclosure — handled separately).
static std::string buildExtraParamsFromPayload(
    const std::string& promptType,
    const nlohmann::json& payload
) {
    if (!payload.is_object()) return "";
    auto getStr = [&](const char* k) -> std::string {
        if (payload.contains(k) && payload[k].is_string()) return payload[k].get<std::string>();
        return "";
    };
    auto getI64 = [&](const char* k) -> int64_t {
        if (!payload.contains(k)) return 0;
        const auto& v = payload[k];
        if (v.is_number_integer()) return v.get<int64_t>();
        if (v.is_number()) return static_cast<int64_t>(v.get<double>());
        return 0;
    };

    if (promptType == "payment_confirmation" || promptType == "rate_limit_exceeded") {
        std::string s = "&satoshis=" + std::to_string(getI64("satoshis"))
                      + "&cents=" + std::to_string(getI64("cents"))
                      + "&exceededLimit=" + getStr("exceededLimit")
                      + "&perTxLimit=" + std::to_string(getI64("perTxLimit"))
                      + "&perSessionLimit=" + std::to_string(getI64("perSessionLimit"))
                      + "&sessionSpent=" + std::to_string(getI64("sessionSpent"));
        if (promptType == "rate_limit_exceeded") {
            s += "&rateLimit=" + std::to_string(getI64("rateLimit"))
               + "&maxTxPerSession=" + std::to_string(getI64("maxTxPerSession"));
        }
        return s;
    }
    if (promptType == "protocol_permission_prompt") {
        return "&protocolLevel=" + std::to_string(getI64("protocolLevel"))
             + "&protocolName=" + getStr("protocolName")
             + "&protocolKeyId=" + getStr("protocolKeyId")
             + "&protocolCounterparty=" + getStr("protocolCounterparty");
    }
    if (promptType == "basket_permission_prompt") {
        return "&basket=" + getStr("basket")
             + "&basketAccess=" + getStr("basketAccess");
    }
    if (promptType == "counterparty_permission_prompt") {
        return "&counterparty=" + getStr("counterparty");
    }
    if (promptType == "key_linkage_reveal") {
        // `protocol` may be null or an array; serialize verbatim so the React
        // modal sees the same shape Rust emitted.
        std::string protocolStr = (payload.contains("protocol") && !payload["protocol"].is_null())
            ? payload["protocol"].dump()
            : std::string("null");
        return "&kind=" + getStr("kind")
             + "&verifier=" + getStr("verifier")
             + "&counterparty=" + getStr("counterparty")
             + "&protocol=" + protocolStr
             + "&keyID=" + getStr("keyID");
    }
    // identity_key_reveal, domain_approval, brc100_auth — no extra params.
    // certificate_disclosure — typed payload, handled by tryHandlePendingResponse directly.
    return "";
}

// Attempt to handle a wallet HTTP response as a 202 PENDING envelope. MUST be
// called on TID_UI (modal dispatch + PendingRequestManager). Returns true if
// the response was an LD2 envelope and the modal was opened — in that case the
// caller MUST NOT deliver any response back to the renderer; the modal flow
// will eventually call resumeInternalResponse which re-issues the request with
// X-User-Approved and delivers the 200 result. Returns false otherwise; the
// caller delivers the response as it would normally.
//
// The supplied `resume` is taken by value because we mutate isInternalResume +
// headersOnApprove before passing it to the opener.
static bool tryHandlePendingResponse(
    int statusCode,
    const std::string& responseBody,
    const ModalContext& modalCtx,
    ResumeContext resume
) {
    if (statusCode != 202) return false;
    PendingEnvelope env;
    if (!parsePendingEnvelope(responseBody, env)) {
        LOG_WARNING_HTTP("tryHandlePendingResponse: 202 with un-parseable envelope from "
            + modalCtx.domain + " endpoint=" + modalCtx.endpoint);
        return false;
    }

    // Mark the new pending request as kInternal so handleAuthResponse routes
    // resolution through resumeInternalResponse. Inject the approval id so the
    // resume re-issue carries the X-User-Approved header that Rust's
    // request_gate consumes (consume_and_verify + sha256 body check per LD2).
    resume.isInternalResume = true;

    // Phase 2.6-G G.4 — domain-trust prompts (domain_approval /
    // manifest_connect_bundle) are satisfied by the add_domain_permission write
    // (trust=approved), NOT by replaying X-User-Approved. The re-issue must be
    // evaluated FRESH by the kind gate so the privacy-perimeter / payment / cert
    // gates still apply — e.g. getPublicKey({identityKey:true}) must still
    // prompt for identity-key reveal when the user left "allow identify"
    // unchecked on the connect modal. Propagating the connect approval token
    // here would let the kind gate's replay path Proceed without re-checking
    // that grant (a privacy-perimeter bypass). Only kind-dispatch prompts carry
    // the replay token.
    const bool isDomainTrustPrompt =
        env.promptType == "domain_approval" || env.promptType == "manifest_connect_bundle";
    if (!isDomainTrustPrompt) {
        resume.headersOnApprove["X-User-Approved"] = env.approvalId;
    }

    std::string newRequestId;
    if (env.promptType == "certificate_disclosure") {
        // Typed payload — direct opener call. The Rust /proveCertificate gate
        // (C.2) populates promptPayload as {certType, certifier, fields:[]}.
        CertDisclosureInfo info;
        if (env.promptPayload.is_object()) {
            if (env.promptPayload.contains("certType") && env.promptPayload["certType"].is_string()) {
                info.certType = env.promptPayload["certType"].get<std::string>();
            }
            if (env.promptPayload.contains("certifier") && env.promptPayload["certifier"].is_string()) {
                info.certifier = env.promptPayload["certifier"].get<std::string>();
            }
            if (env.promptPayload.contains("fields") && env.promptPayload["fields"].is_array()) {
                for (const auto& f : env.promptPayload["fields"]) {
                    if (f.is_string()) info.fieldsToReveal.push_back(f.get<std::string>());
                }
            }
            info.valid = !info.fieldsToReveal.empty();
        }
        newRequestId = openCertificateDisclosureModal(modalCtx, resume, info);
    } else if (env.promptType == "manifest_connect_bundle") {
        // Phase 2.6-G G.4 — Rust's domain_trust_gate embeds the served manifest
        // bytes as a JSON string in promptPayload.manifest. Re-parse to the
        // typed Manifest the modal opener needs (ParseFromJson is pure +
        // lenient). If the manifest is missing/unparseable, fall back to the
        // plain domain_approval modal so the connect still works.
        hodos::Manifest manifest;
        if (env.promptPayload.is_object()
            && env.promptPayload.contains("manifest")
            && env.promptPayload["manifest"].is_string()) {
            manifest = hodos::ManifestFetcher::ParseFromJson(
                env.promptPayload["manifest"].get<std::string>());
        }
        if (manifest.valid) {
            newRequestId = openManifestConnectBundleModal(modalCtx, resume, manifest);
        } else {
            LOG_WARNING_HTTP("tryHandlePendingResponse: manifest_connect_bundle with "
                "unparseable manifest from " + modalCtx.domain
                + " — falling back to domain_approval");
            newRequestId = openDomainApprovalModal(modalCtx, resume);
        }
    } else {
        std::string extraParams = buildExtraParamsFromPayload(env.promptType, env.promptPayload);
        newRequestId = OpenPromptModal(env.promptType, modalCtx, resume, extraParams);
    }

    if (newRequestId.empty()) {
        LOG_WARNING_HTTP("tryHandlePendingResponse: opener returned empty requestId for promptType '"
            + env.promptType + "' from " + modalCtx.domain);
        return false;
    }

    LOG_DEBUG_HTTP("🛡️ 202 PENDING intercepted — modal opened (requestId="
        + newRequestId + ", promptType=" + env.promptType + ", approvalId=" + env.approvalId
        + ", reason=" + env.engineReason + ", domain=" + modalCtx.domain + ")");

    // Arm a timeout so the original wallet call doesn't hang forever if the
    // user dismisses the modal without clicking anything. Mirrors the inline
    // cascade's timeout discipline.
    std::string timeoutMsg = "{\"error\":\"Approval timeout\",\"status\":\"error\",\"reason\":\"timeout\"}";
    if (resume.frame) {
        postIpcAuthTimeout(newRequestId, resume.frame, timeoutMsg, kPromptAuthTimeoutMs);
    } else if (resume.handler) {
        auto* walletHandler = static_cast<AsyncWalletResourceHandler*>(resume.handler.get());
        if (walletHandler) {
            walletHandler->postAuthTimeout(kPromptAuthTimeoutMs, timeoutMsg);
        }
    }

    return true;
}

// Phase 2.6-C.3 — resume path for a kInternal PendingAuthRequest (one that was
// enrolled from a 202 PENDING envelope returned by Rust). Mirrors
// resumeIpcResponse but dispatches the final result back to whichever resume
// path is populated on the stored request — handler for HTTP path,
// frame for IPC path. Per kickoff Q6 the resume body is structurally identical
// to resumeIpcResponse; the kInternal arm exists so handleAuthResponse can
// route through the correct delivery without sniffing handler/frame presence.
//
// On user Deny (responseData contains "error"): deliver the error verbatim to
// the original handler or frame; no wallet call is made.
// On user Approve: post to TID_FILE_USER_BLOCKING worker; inject
// headersOnApprove (which includes X-User-Approved: <approvalId>) +
// X-Requesting-Domain; dispatch via SyncHttpClient; hop back to TID_UI for
// delivery. Payment-indicator green-dot fires for auto-approved payments via
// OnWalletCallSuccess on the IPC side (HTTP side fires from
// AsyncHTTPClient::OnRequestComplete after onAuthResponseReceived returns the
// new payload through the original resource handler).
static void resumeInternalResponse(const PendingAuthRequest& req,
                                   const std::string& responseData) {
    bool isRejection = false;
    try {
        auto parsed = nlohmann::json::parse(responseData);
        isRejection = parsed.contains("error");
    } catch (...) {}

    if (isRejection) {
        LOG_DEBUG_HTTP("🔐 kInternal resume: deny for " + req.requestId
                       + " endpoint=" + req.endpoint);
        if (req.handler) {
            auto* walletHandler = static_cast<AsyncWalletResourceHandler*>(req.handler.get());
            if (walletHandler) walletHandler->onAuthResponseReceived(responseData);
        } else if (req.frame) {
            // Phase 2.6-C.5 fix — use page-supplied IPC requestId so the
            // CWI shim's promise (keyed by that id) actually resolves.
            const std::string& ipcId = !req.originalIpcRequestId.empty()
                ? req.originalIpcRequestId : req.requestId;
            sendWalletResponseIpc(req.frame, ipcId, false, responseData);
        }
        return;
    }

    LOG_DEBUG_HTTP("🔐 kInternal resume: approve for " + req.requestId
                   + " endpoint=" + req.endpoint
                   + " injectedHeaders=" + std::to_string(req.headersOnApprove.size()));

    std::string requestId = !req.originalIpcRequestId.empty()
        ? req.originalIpcRequestId : req.requestId;
    std::string domain = req.domain;
    std::string endpoint = req.endpoint;
    std::string body = req.body;
    std::string httpMethod = req.httpMethod.empty() ? "POST" : req.httpMethod;
    std::map<std::string, std::string> headersOnApprove = req.headersOnApprove;
    CefRefPtr<CefResourceHandler> handler = req.handler;
    CefRefPtr<CefFrame> frame = req.frame;
    int browserId = req.browserId;

    CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](
        std::string requestId, std::string domain, std::string endpoint,
        std::string body, std::string httpMethod,
        std::map<std::string, std::string> headersOnApprove,
        CefRefPtr<CefResourceHandler> handler,
        CefRefPtr<CefFrame> frame, int browserId
    ) {
        std::map<std::string, std::string> headers;
        headers["Content-Type"] = "application/json";
        if (!domain.empty()) headers["X-Requesting-Domain"] = domain;
        for (const auto& kv : headersOnApprove) headers[kv.first] = kv.second;

        std::string url = hodos::WalletBaseUrl() + endpoint;
        HttpResponse resp = dispatchWalletHttpByMethod(httpMethod, url, body, headers);
        bool ok = resp.success && resp.statusCode >= 200 && resp.statusCode < 300;
        std::string payload = buildIpcResponsePayload(resp, ok);

        // Payment-indicator state for IPC path (HTTP path's
        // AsyncHTTPClient::OnRequestComplete already handles the indicator
        // when it sees a fresh response come back via onAuthResponseReceived;
        // the handler's own flow re-runs the OnWalletCallSuccess derivation).
        bool isPaymentKind = AsyncWalletResourceHandler::isPaymentEndpoint(endpoint);
        int64_t cents = 0;
        if (isPaymentKind) {
            int64_t satoshis = AsyncWalletResourceHandler::extractOutputSatoshis(body);
            double bsvPrice = BSVPriceCache::GetInstance().getPrice();
            if (bsvPrice > 0 && satoshis > 0) {
                cents = static_cast<int64_t>(
                    (static_cast<double>(satoshis) / 100000000.0) * bsvPrice * 100.0);
            }
        }
        bool isErrorInResponse = false;
        if (ok && isPaymentKind) {
            try {
                auto rj = nlohmann::json::parse(payload);
                isErrorInResponse = rj.contains("error");
            } catch (...) {}
        }
        const bool wasAutoApprovedPayment =
            ok && isPaymentKind && !isErrorInResponse;

        CefPostTask(TID_UI, base::BindOnce([](
            std::string requestId, bool ok, std::string payload,
            CefRefPtr<CefResourceHandler> handler,
            CefRefPtr<CefFrame> frame, int browserId,
            std::string domain, std::string endpoint, int64_t cents,
            bool wasAutoApprovedPayment
        ) {
            if (handler) {
                // HTTP path: deliver re-issue result through the original
                // resource handler. The handler's payment indicator fires via
                // its own AsyncHTTPClient::OnRequestComplete chain on a fresh
                // request — for kInternal we deliver directly because the
                // resume bypasses the URLRequest flow.
                auto* walletHandler = static_cast<AsyncWalletResourceHandler*>(handler.get());
                if (walletHandler) {
                    if (wasAutoApprovedPayment) {
                        OnWalletCallSuccess(walletHandler->getBrowserId(), domain,
                                            cents, true, endpoint);
                    }
                    walletHandler->onAuthResponseReceived(payload);
                }
            } else if (frame) {
                if (wasAutoApprovedPayment) {
                    OnWalletCallSuccess(browserId, domain, cents, true, endpoint);
                }
                sendWalletResponseIpc(frame, requestId, ok, payload);
            }
        }, requestId, ok, payload, handler, frame, browserId,
           domain, endpoint, cents, wasAutoApprovedPayment));
    }, requestId, domain, endpoint, body, httpMethod, headersOnApprove,
       handler, frame, browserId));
}

// Phase 2.6-C.5 fix — kHttpCallback resume after modal resolution. Mirrors
// resumeIpcResponse but delivers the wallet response to the original
// resource handler instead of via wallet_response IPC. Replaces the legacy
// AuthResponseHandler URLRequest in simple_handler.cpp's brc100_auth_response
// dispatcher (which was returning status=UR_SUCCESS with empty bodies in
// CEF 136, leaving HTTP-path pages stuck waiting for ReadResponse).
//
// On user Deny (responseData contains "error"): deliver the error envelope
// verbatim to the resource handler; no wallet call is made.
// On user Approve: post to TID_FILE_USER_BLOCKING worker; inject
// headersOnApprove (legacy X-Identity-Key-Approved / X-Key-Linkage-Approved
// kept here for kHttpCallback path — those header injections were the
// pre-C.2 fast-path; Rust ignores them post-C.2 but the inline cascade may
// still set them via openIdentityKeyRevealModal et al.); SyncHttpClient::Post;
// hop back to TID_UI for OnWalletCallSuccess (if payment) +
// walletHandler->onAuthResponseReceived. The resource handler's atomic
// httpCompleted_ claim guards against double-delivery.
static void resumeHttpCallbackResponse(const PendingAuthRequest& req,
                                       const std::string& responseData) {
    if (!req.handler) {
        LOG_DEBUG_HTTP("resumeHttpCallbackResponse: no handler for requestId " + req.requestId);
        return;
    }

    bool isRejection = false;
    try {
        auto parsed = nlohmann::json::parse(responseData);
        isRejection = parsed.contains("error");
    } catch (...) {}

    if (isRejection) {
        LOG_DEBUG_HTTP("🔐 HTTP-callback resume: deny for " + req.requestId
                       + " endpoint=" + req.endpoint);
        auto* walletHandler = static_cast<AsyncWalletResourceHandler*>(req.handler.get());
        if (walletHandler) walletHandler->onAuthResponseReceived(responseData);
        return;
    }

    LOG_DEBUG_HTTP("🔐 HTTP-callback resume: approve for " + req.requestId
                   + " endpoint=" + req.endpoint
                   + " injectedHeaders=" + std::to_string(req.headersOnApprove.size()));

    std::string requestId = req.requestId;
    std::string domain = req.domain;
    std::string endpoint = req.endpoint;
    std::string body = req.body;
    std::string httpMethod = req.httpMethod.empty() ? req.method : req.httpMethod;
    if (httpMethod.empty()) httpMethod = "POST";
    std::map<std::string, std::string> headersOnApprove = req.headersOnApprove;
    CefRefPtr<CefResourceHandler> handler = req.handler;

    CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](
        std::string requestId, std::string domain, std::string endpoint,
        std::string body, std::string httpMethod,
        std::map<std::string, std::string> headersOnApprove,
        CefRefPtr<CefResourceHandler> handler
    ) {
        std::map<std::string, std::string> headers;
        headers["Content-Type"] = "application/json";
        if (!domain.empty()) headers["X-Requesting-Domain"] = domain;
        for (const auto& kv : headersOnApprove) headers[kv.first] = kv.second;

        std::string url = hodos::WalletBaseUrl() + endpoint;
        HttpResponse resp = dispatchWalletHttpByMethod(httpMethod, url, body, headers);
        bool ok = resp.success && resp.statusCode >= 200 && resp.statusCode < 300;
        std::string payload = buildIpcResponsePayload(resp, ok);

        // Payment indicator: same derivation as resumeIpcResponse. The
        // handler's pre-calculated cents may be stale (price moved between
        // modal-open and modal-approve), so re-extract here.
        bool isPaymentKind = AsyncWalletResourceHandler::isPaymentEndpoint(endpoint);
        int64_t cents = 0;
        if (isPaymentKind) {
            int64_t satoshis = AsyncWalletResourceHandler::extractOutputSatoshis(body);
            double bsvPrice = BSVPriceCache::GetInstance().getPrice();
            if (bsvPrice > 0 && satoshis > 0) {
                cents = static_cast<int64_t>(
                    (static_cast<double>(satoshis) / 100000000.0) * bsvPrice * 100.0);
            }
        }
        bool isErrorInResponse = false;
        if (ok && isPaymentKind) {
            try {
                auto rj = nlohmann::json::parse(payload);
                isErrorInResponse = rj.contains("error");
            } catch (...) {}
        }
        const bool wasAutoApprovedPayment =
            ok && isPaymentKind && !isErrorInResponse;

        CefPostTask(TID_UI, base::BindOnce([](
            std::string payload, CefRefPtr<CefResourceHandler> handler,
            std::string domain, std::string endpoint, int64_t cents,
            bool wasAutoApprovedPayment
        ) {
            auto* walletHandler = static_cast<AsyncWalletResourceHandler*>(handler.get());
            if (!walletHandler) return;
            if (wasAutoApprovedPayment) {
                OnWalletCallSuccess(walletHandler->getBrowserId(), domain,
                                    cents, true, endpoint);
            }
            walletHandler->onAuthResponseReceived(payload);
        }, payload, handler, domain, endpoint, cents, wasAutoApprovedPayment));
    }, requestId, domain, endpoint, body, httpMethod, headersOnApprove, handler));
}

// Function to handle auth response and send it back to the original request
// Phase 2.5 Commit 6 sub-step 6.d.BE — IPC-path resume after modal resolution.
// Mirror of the HTTP-path resume but ending in wallet_response IPC instead
// of walletHandler->onAuthResponseReceived / StartAsyncHTTPRequestTask.
//
// On user Deny (responseData contains "error"): send wallet_response with
// the error payload; no wallet call is made.
// On user Approve: post to TID_FILE_USER_BLOCKING worker; inject
// headersOnApprove + standard headers; SyncHttpClient::Post; hop back to
// TID_UI for OnWalletCallSuccess (if payment) + sendWalletResponseIpc.
//
// Cents re-extracted from body at re-issue time per design Q4 (matches
// HTTP path's preCalculatedCents freshness behavior).
static void resumeIpcResponse(const PendingAuthRequest& req,
                              const std::string& responseData) {
    if (!req.frame) {
        LOG_DEBUG_HTTP("resumeIpcResponse: no frame for requestId " + req.requestId);
        return;
    }

    bool isRejection = false;
    try {
        auto parsed = nlohmann::json::parse(responseData);
        isRejection = parsed.contains("error");
    } catch (...) {}

    // Phase 2.6-C.5 fix — use page-supplied IPC requestId (the one the CWI
    // shim is waiting on) for sendWalletResponseIpc. Fall back to the C++
    // internal id for entries enrolled before this field was threaded
    // through (defense-in-depth — pre-fix entries that survive into a
    // post-fix process would otherwise have an empty id).
    const std::string ipcId = !req.originalIpcRequestId.empty()
        ? req.originalIpcRequestId : req.requestId;

    if (isRejection) {
        // User Denied — surface the error envelope directly. No wallet call.
        LOG_DEBUG_HTTP("🔐 IPC resume: deny for " + req.requestId
                       + " endpoint=" + req.endpoint
                       + " (ipcId=" + ipcId + ")");
        sendWalletResponseIpc(req.frame, ipcId, false, responseData);
        return;
    }

    // User Approved — re-issue the wallet call with headersOnApprove injected.
    LOG_DEBUG_HTTP("🔐 IPC resume: approve for " + req.requestId
                   + " endpoint=" + req.endpoint
                   + " injectedHeaders=" + std::to_string(req.headersOnApprove.size())
                   + " (ipcId=" + ipcId + ")");

    // Capture all needed state by-value into the worker lambda. req is by
    // const ref here so we copy the fields we need.
    std::string requestId = ipcId;
    std::string domain = req.domain;
    std::string endpoint = req.endpoint;
    std::string body = req.body;
    std::string httpMethod = req.httpMethod.empty() ? "POST" : req.httpMethod;
    std::map<std::string, std::string> headersOnApprove = req.headersOnApprove;
    CefRefPtr<CefFrame> frame = req.frame;
    int browserId = req.browserId;

    CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](
        std::string requestId, std::string domain, std::string endpoint,
        std::string body, std::string httpMethod,
        std::map<std::string, std::string> headersOnApprove,
        CefRefPtr<CefFrame> frame, int browserId
    ) {
        std::map<std::string, std::string> headers;
        headers["Content-Type"] = "application/json";
        if (!domain.empty()) headers["X-Requesting-Domain"] = domain;
        for (const auto& kv : headersOnApprove) headers[kv.first] = kv.second;

        std::string url = hodos::WalletBaseUrl() + endpoint;
        HttpResponse resp = dispatchWalletHttpByMethod(httpMethod, url, body, headers);
        bool ok = resp.success && resp.statusCode >= 200 && resp.statusCode < 300;
        std::string payload = buildIpcResponsePayload(resp, ok);

        // Q4: re-extract cents at re-issue time so any BSV-price movement between
        // modal-open and modal-approve is reflected in the recorded spend.
        bool isPaymentKind = AsyncWalletResourceHandler::isPaymentEndpoint(endpoint);
        int64_t cents = 0;
        if (isPaymentKind) {
            int64_t satoshis = AsyncWalletResourceHandler::extractOutputSatoshis(body);
            double bsvPrice = BSVPriceCache::GetInstance().getPrice();
            if (bsvPrice > 0 && satoshis > 0) {
                cents = static_cast<int64_t>(
                    (static_cast<double>(satoshis) / 100000000.0) * bsvPrice * 100.0);
            }
        }
        bool isErrorInResponse = false;
        if (ok && isPaymentKind) {
            try {
                auto rj = nlohmann::json::parse(payload);
                isErrorInResponse = rj.contains("error");
            } catch (...) {}
        }
        // 6.d.BE+1: dropped `cents > 0` — tiny payments still deserve the
        // gold-pill animation as the user-visible safeguard (matches
        // legacy AsyncHTTPClient::OnRequestComplete behavior).
        const bool wasAutoApprovedPayment =
            ok && isPaymentKind && !isErrorInResponse;

        CefPostTask(TID_UI, base::BindOnce([](
            std::string requestId, bool ok, std::string payload,
            CefRefPtr<CefFrame> frame, int browserId,
            std::string domain, std::string endpoint, int64_t cents,
            bool wasAutoApprovedPayment
        ) {
            if (wasAutoApprovedPayment) {
                OnWalletCallSuccess(browserId, domain, cents, true, endpoint);
            }
            sendWalletResponseIpc(frame, requestId, ok, payload);
        }, requestId, ok, payload, frame, browserId,
           domain, endpoint, cents, wasAutoApprovedPayment));
    }, requestId, domain, endpoint, body, httpMethod, headersOnApprove,
       frame, browserId));
}

void handleAuthResponse(const std::string& requestId, const std::string& responseData) {
    LOG_DEBUG_HTTP("🔐 handleAuthResponse called for requestId: " + requestId);

    std::string domain;

    // 1. Resolve the primary request (the one whose response we have)
    PendingAuthRequest req;
    if (PendingRequestManager::GetInstance().popRequest(requestId, req)) {
        domain = req.domain;
        if (req.handler && req.resumeKind == ResumeKind::kHttpCallback) {
            // Phase 2.6-C.5 fix — HTTP-path resume now mirrors the IPC path:
            // re-issue the wallet call via SyncHttpClient::Post on a worker
            // thread, then deliver the response to the resource handler.
            // The legacy AuthResponseHandler URLRequest in simple_handler.cpp
            // was unreliable under CEF 136 (status=UR_SUCCESS with empty
            // bodies — see resumeHttpCallbackResponse docstring).
            LOG_DEBUG_HTTP("🔐 Found pending HTTP auth request for domain: " + req.domain);
            // OQ5 — user-approved over-cap spend is now recorded in Rust by
            // dispatch_payment's X-User-Approved replay path (request_gate.rs),
            // for both createAction and BRC-121. The former C++ SessionManager
            // ::recordSpending here was removed to avoid double-counting once
            // Rust owns the per-session counters.
            resumeHttpCallbackResponse(req, responseData);
        } else if (req.resumeKind == ResumeKind::kIpcResponse) {
            // Phase 2.5 Commit 6 sub-step 6.d.BE — IPC resume branch.
            // HTTP path's branch above is unchanged; IPC requests (no
            // handler, resumeKind=kIpcResponse) flow here.
            LOG_DEBUG_HTTP("🔐 IPC resume for primary requestId " + requestId
                           + " domain=" + req.domain);
            resumeIpcResponse(req, responseData);
        } else if (req.resumeKind == ResumeKind::kInternal) {
            // Phase 2.6-C.3 — kInternal resume branch. The request was
            // enrolled when Rust returned a 202 PENDING envelope; on user
            // resolution we re-issue the wallet call with X-User-Approved
            // header injected via headersOnApprove (which carries the
            // approvalId from the original envelope). resumeInternalResponse
            // dispatches the result to handler (HTTP path) or frame (IPC
            // path) based on which field is populated on the stored req.
            LOG_DEBUG_HTTP("🔐 kInternal resume for primary requestId " + requestId
                           + " domain=" + req.domain);
            resumeInternalResponse(req, responseData);
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
            if (sibling.handler && sibling.resumeKind == ResumeKind::kHttpCallback) {
                // Phase 2.6-C.5 fix — sibling HTTP resume goes through the
                // same SyncHttpClient-based re-fetch as the primary. The old
                // StartAsyncHTTPRequestTask path went through
                // AsyncWalletResourceHandler::Open() which would re-run the
                // engine cascade (now mostly gone post-C.4) and could in
                // principle return another 202 — which would have driven
                // C.3's intercept path. resumeHttpCallbackResponse keeps the
                // behavior consistent with the primary path: re-fetch via
                // SyncHttpClient + deliver to the resource handler.
                if (isRejection) {
                    auto* walletHandler =
                        static_cast<AsyncWalletResourceHandler*>(sibling.handler.get());
                    if (walletHandler) walletHandler->onAuthResponseReceived(responseData);
                    LOG_DEBUG_HTTP("🔐 Sent rejection to queued HTTP request "
                                   + sibling.requestId + " for " + sibling.endpoint);
                } else {
                    LOG_DEBUG_HTTP("🔐 HTTP-callback resume for queued sibling "
                                   + sibling.requestId + " endpoint=" + sibling.endpoint);
                    resumeHttpCallbackResponse(sibling, responseData);
                }
            } else if (sibling.resumeKind == ResumeKind::kIpcResponse) {
                // Phase 2.5 Commit 6 sub-step 6.d.BE — IPC sibling resume.
                // For approve, resumeIpcResponse re-issues via worker; for
                // reject, it sends wallet_response with the error envelope.
                LOG_DEBUG_HTTP("🔐 IPC resume for queued sibling " + sibling.requestId
                               + " endpoint=" + sibling.endpoint);
                resumeIpcResponse(sibling, responseData);
            } else if (sibling.resumeKind == ResumeKind::kInternal) {
                // Phase 2.6-C.3 — kInternal sibling resume.
                LOG_DEBUG_HTTP("🔐 kInternal resume for queued sibling " + sibling.requestId
                               + " endpoint=" + sibling.endpoint);
                resumeInternalResponse(sibling, responseData);
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

        if (!parent_) return;

        // Phase 2.6-C.3 — intercept 202 PENDING envelopes from Rust before
        // forwarding to the renderer. tryHandlePendingResponse parses the
        // envelope, enrolls a kInternal PendingAuthRequest, and fires the
        // matching modal. We DO NOT touch onHTTPResponseReceived in that case
        // — the modal resolution flow (handleAuthResponse → resumeInternalResponse)
        // re-issues the call with X-User-Approved and delivers the eventual
        // 200 result through the same handler.
        //
        // Hop to TID_UI because tryHandlePendingResponse fires modal-opening
        // tasks that expect UI-thread access. OnRequestComplete itself runs
        // on the IO thread.
        auto resp = request->GetResponse();
        int statusCode = resp ? resp->GetStatus() : 0;
        if (statusCode == 202) {
            ModalContext modalCtx{
                parent_->getRequestDomain(),
                parent_->getMethod(),
                parent_->getEndpoint(),
                parent_->getBody(),
            };
            ResumeContext resume;
            resume.handler = parent_;
            resume.httpMethod = parent_->getMethod();
            // isInternalResume + headersOnApprove are populated inside
            // tryHandlePendingResponse.
            std::string responseBody = responseData_;
            CefPostTask(TID_UI, base::BindOnce([](
                int statusCode, std::string responseBody,
                ModalContext modalCtx, ResumeContext resume,
                CefRefPtr<AsyncWalletResourceHandler> parent
            ) {
                if (!tryHandlePendingResponse(statusCode, responseBody, modalCtx, resume)) {
                    // Envelope malformed or unsupported promptType — fall back
                    // to delivering the raw 202 body to the renderer so the
                    // caller can see the error rather than hang.
                    LOG_WARNING_HTTP("🛡️ 202 from Rust failed envelope handling — forwarding raw body");
                    if (parent) parent->onHTTPResponseReceived(responseBody);
                }
            }, statusCode, responseBody, modalCtx, resume, parent_));
            return;
        }

        // Phase 2.5 Commit 6 sub-step 6.b — record spending + fire green-dot
        // animation via the shared OnWalletCallSuccess helper. The error-check
        // stays at the caller because OnWalletCallSuccess doesn't have access
        // to the response body.
        //
        // Phase 2.6-E LD4 — wasAutoApprovedPayment is derived locally now
        // (the engine signal lived in the deleted C++ payment cascade).
        // A payment endpoint that came back UR_SUCCESS with no error in the
        // response body was, by definition, auto-approved by Rust's
        // dispatch_payment Silent branch OR replayed via X-User-Approved
        // after the user accepted the prompt. Either way the user has
        // sanctioned the spend — green-dot fires.
        CefURLRequest::Status status = request->GetRequestStatus();
        int64_t cents = parent_->getPreCalculatedCents();
        const bool isPaymentKind =
            AsyncWalletResourceHandler::isPaymentEndpoint(parent_->getEndpoint());
        bool successAndNotError = false;
        if (status == UR_SUCCESS && isPaymentKind) {
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
                            /*endpoint=*/parent_->getEndpoint());
        parent_->onHTTPResponseReceived(responseData_);
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

    // Phase 2.6-C.4 — DELETED: the Phase 1.5 Step 1 drain-forward safety net
    // that fired triggerIdentityKeyRevealModal here when an identity-key
    // /getPublicKey reached startAsyncHTTPRequest without going through the
    // (now-removed) Open() gate. Post-C.2, Rust's privacy-perimeter handler
    // is authoritative — bare identity-key requests return a 202 PENDING
    // envelope (not the old 403 identity_key_prompt_required), and C.3's
    // AsyncHTTPClient::OnRequestComplete opens the modal from that envelope.
    // Keeping the safety net would cause a double-modal race with C.3's
    // intercept path.

    // Create CEF HTTP request
    CefRefPtr<CefRequest> httpRequest = CefRequest::Create();
    std::string fullUrl = hodos::WalletBaseUrl() + endpoint_;
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

    // Phase 2.6-H.2 — the legacy X-Identity-Key-Approved header injection was
    // removed. Rust's get_public_key stopped consulting that header post-2.6-C.2
    // (handlers.rs: "Rust trusts only its own approval mechanism") and now
    // authorizes identity-key disclosure via the V17 DB column + the
    // X-User-Approved re-issue + its own session cache. The C++ session cache
    // that fed this header was deleted in 2.6-H.2.

    // Phase 2.6-E — inject payment headers on payment endpoints so Rust's
    // dispatch_payment can build the PermissionContext server-side. LD4 keeps
    // satoshis + cents derivation on the C++ side (BSVPriceCache stays here);
    // the values were stashed on the handler by Open()'s payment-endpoint
    // branch. Missing/zero values are safe — Rust treats them as
    // best-effort and prompts price_unavailable when X-Bsv-Price-Available=0.
    if (isPaymentEndpoint(endpoint_)) {
        int browserId = browser_ ? browser_->GetIdentifier() : 0;
        headers.insert(std::make_pair("X-Browser-Id", std::to_string(browserId)));
        headers.insert(std::make_pair("X-Payment-Satoshis",
            std::to_string(preCalculatedSatoshis_)));
        headers.insert(std::make_pair("X-Payment-Cents",
            std::to_string(preCalculatedCents_)));
        headers.insert(std::make_pair("X-Bsv-Price-Available",
            preCalculatedBsvPriceAvailable_ ? "1" : "0"));
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
    redirectPort("localhost:", "localhost:" + hodos::WalletPortStr());
    redirectPort("127.0.0.1:", "127.0.0.1:" + hodos::WalletPortStr());

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
                    url = hodos::WalletBaseUrl() + url.substr(hostEnd);
                } else {
                    url = hodos::WalletBaseUrl();
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

// OQ5 — BRC-121 over-cap approval registry. When Rust returns 202 for a
// payment_confirmation / rate_limit_exceeded prompt, TryHandleBrc121_402
// stashes the Rust approvalId here (PENDING) keyed by URL. When the user clicks
// Approve, MarkBrc121PaymentApproved (called from simple_handler.cpp's
// brc100_auth_response handler) moves it PENDING → ARMED. The reload that
// follows pops the ARMED approvalId and re-POSTs /wallet/pay402 with
// X-User-Approved, so Rust's dispatch_payment replay records the spend +
// proceeds. Two states keep consent honest: a reload WITHOUT an Approve (e.g.
// a manual refresh) finds nothing armed and gets a fresh engine decision —
// no permanent cap bypass, no infinite-loop risk.
std::mutex s_brc121_approvals_mutex;
std::unordered_map<std::string, std::string> s_brc121_pending_approvals;  // url → approvalId (from 202)
std::unordered_map<std::string, std::string> s_brc121_armed_approvals;    // url → approvalId (user approved)

// Stash the Rust approvalId for a 202-prompted URL (pending user approval).
void SetBrc121PendingApproval(const std::string& url, const std::string& approvalId) {
    std::lock_guard<std::mutex> lock(s_brc121_approvals_mutex);
    s_brc121_pending_approvals[url] = approvalId;
}

// Pop an ARMED approvalId for this URL. Returns true if the user had approved
// (approvalIdOut set — may be empty if Rust sent no id); false if not armed.
bool PopBrc121ArmedApproval(const std::string& url, std::string& approvalIdOut) {
    std::lock_guard<std::mutex> lock(s_brc121_approvals_mutex);
    auto it = s_brc121_armed_approvals.find(url);
    if (it == s_brc121_armed_approvals.end()) return false;
    approvalIdOut = it->second;
    s_brc121_armed_approvals.erase(it);
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
        // Phase 2.5 Commit 6 sub-step 6.b — fire the gold-pill indicator via the
        // shared OnWalletCallSuccess helper. OQ5: the per-session rate / payment
        // counters + spend are now recorded in Rust at pay_402 decision time
        // (dispatch_payment, on Silent / X-User-Approved replay), mirroring
        // createAction — so the former C++ SessionManager increments here were
        // removed. This keeps ONLY the gold-pill payment_success_indicator IPC,
        // which MUST still fire on every auto-approved BRC-121 payment.
        int cefBrowserId = browser_ ? browser_->GetIdentifier() : 0;
        OnWalletCallSuccess(cefBrowserId,
                            ctx_.domain,
                            ctx_.cents,
                            /*wasAutoApprovedPayment=*/true,
                            /*endpoint=*/"pay402");
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
                hodos::WalletUrl("/wallet/broadcast-nosend"),
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

    // OQ5 — the per-tx / per-session / max-tx / rate / price_unavailable
    // DECISION now lives in Rust. C++ extracts the payee + sats (above) and
    // computes cents (BSVPriceCache, kept C++-side per the createAction
    // contract), then forwards to /wallet/pay402 via the X-Payment-* headers —
    // exactly mirroring createAction's dispatch_payment path. Rust returns:
    //   200 → Silent (within caps) OR user-approved replay → BEEF minted below
    //   202 → prompt (cap/rate/price_unavailable) — no mint; surface the modal
    //   403 → deny (e.g. domain blocked mid-flight) — page sees native 402
    // SessionManager and the C++ cap cascade are GONE from this path.
    double bsvPrice = BSVPriceCache::GetInstance().getPrice();
    int browserId = browser ? browser->GetIdentifier() : 0;
    int64_t cents = (bsvPrice > 0)
        ? static_cast<int64_t>(
            (static_cast<double>(satoshis) / 100000000.0) * bsvPrice * 100.0)
        : 0;

    // Post-modal approval replay: if the user approved this exact URL on a
    // prior 202 prompt, MarkBrc121PaymentApproved armed the Rust approvalId.
    // Send it as X-User-Approved so Rust's dispatch_payment replay path records
    // the spend + proceeds (single-use, body-sha256-bound). Not armed ⇒ fresh
    // engine decision. Loop-safe: arming is consumed by the pop.
    std::string armedApprovalId;
    const bool replayApproved = PopBrc121ArmedApproval(url, armedApprovalId);

    nlohmann::json reqBody;
    reqBody["server_pubkey_hex"] = serverPubkey;
    reqBody["satoshis"] = satoshis;
    reqBody["original_url"] = url;
    std::string body = reqBody.dump();

    std::map<std::string, std::string> payHeaders;
    payHeaders["Content-Type"] = "application/json";
    payHeaders["X-Requesting-Domain"] = domain;  // payee host — Rust caps key on this
    payHeaders["X-Browser-Id"] = std::to_string(browserId);
    payHeaders["X-Payment-Satoshis"] = std::to_string(satoshis);
    payHeaders["X-Payment-Cents"] = std::to_string(cents);
    payHeaders["X-Bsv-Price-Available"] = (bsvPrice > 0) ? "1" : "0";
    if (replayApproved && !armedApprovalId.empty()) {
        payHeaders["X-User-Approved"] = armedApprovalId;
    }

    LOG_INFO_HTTP("💰 BRC-121 → /wallet/pay402 (" + std::to_string(cents) + " cents, "
                  + (replayApproved ? "user-approved replay" : "engine decision") + ")");

    // Synchronous: localhost wallet, fast path. 10s ceiling — createAction may need
    // to fetch BEEF ancestry from external indexers in worst cases.
    HttpResponse rresp = SyncHttpClient::Post(
        hodos::WalletUrl("/wallet/pay402"),
        body,
        payHeaders,
        10000);

    // 202 → Rust's engine prompted. Surface the matching modal from the Rust
    // promptPayload and arm the BRC-121 reload machinery; on Approve the reload
    // re-POSTs with the armed X-User-Approved approvalId (stashed here).
    if (rresp.statusCode == 202) {
        std::string approvalId;
        std::string promptType = "payment_confirmation";
        std::string extraParams;
        try {
            auto pj = nlohmann::json::parse(rresp.body);
            approvalId = pj.value("approvalId", "");
            promptType = pj.value("promptType", "payment_confirmation");
            auto pp = pj.value("promptPayload", nlohmann::json::object());
            // Rust hardcodes bsvPrice=0 in the payload — use C++'s real price.
            extraParams = "&satoshis=" + std::to_string(pp.value("satoshis", satoshis))
                        + "&cents=" + std::to_string(pp.value("cents", cents))
                        + "&bsvPrice=" + std::to_string(bsvPrice)
                        + "&exceededLimit=" + pp.value("exceededLimit", std::string())
                        + "&perTxLimit=" + std::to_string(pp.value("perTxLimit", static_cast<int64_t>(0)))
                        + "&perSessionLimit=" + std::to_string(pp.value("perSessionLimit", static_cast<int64_t>(0)))
                        + "&sessionSpent=" + std::to_string(pp.value("sessionSpent", static_cast<int64_t>(0)));
            if (promptType == "rate_limit_exceeded") {
                extraParams += "&rateLimit=" + std::to_string(pp.value("rateLimit", static_cast<int64_t>(0)))
                             + "&maxTxPerSession=" + std::to_string(pp.value("maxTxPerSession", static_cast<int64_t>(0)))
                             + "&txCount=" + std::to_string(pp.value("txCount", static_cast<int64_t>(0)));
            }
        } catch (const std::exception& e) {
            LOG_WARNING_HTTP("💰 BRC-121: 202 payload parse failed: " + std::string(e.what())
                             + " — falling through to native 402");
            return false;
        }
        LOG_INFO_HTTP("💰 BRC-121: Rust prompted (" + promptType + ") — surfacing modal for " + domain);
        // Stash the Rust approvalId keyed by URL (PENDING); MarkBrc121PaymentApproved
        // arms it (PENDING → ARMED) when the user clicks Approve.
        SetBrc121PendingApproval(url, approvalId);
        bool modalAlreadyShowing = PendingRequestManager::GetInstance().hasPendingForDomain(domain);
        PendingRequestManager::GetInstance().addRequest(
            domain, "GET", url, "", nullptr, promptType);
        registerPendingBrc121Reload(domain, browser, url);
        SetPendingBrc121PriceForDomain(domain, satoshis);
        if (!modalAlreadyShowing) {
            CefPostTask(TID_UI, new CreateNotificationOverlayTask(promptType, domain, extraParams));
        }
        return false;
    }

    // 403 → Rust denied (domain blocked, etc.). Page sees the native 402.
    if (rresp.statusCode == 403) {
        LOG_WARNING_HTTP("💰 BRC-121: /wallet/pay402 denied (403) — falling through to native 402");
        return false;
    }

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
    std::lock_guard<std::mutex> lock(s_brc121_approvals_mutex);
    // Move the Rust approvalId from PENDING (stashed when the 202 arrived) to
    // ARMED so the post-approval reload replays it via X-User-Approved. If no
    // pending id exists (shouldn't happen in the engine flow), arm empty — the
    // reload then gets a fresh engine decision rather than silently bypassing caps.
    auto it = s_brc121_pending_approvals.find(url);
    std::string id = (it != s_brc121_pending_approvals.end()) ? it->second : std::string();
    if (it != s_brc121_pending_approvals.end()) s_brc121_pending_approvals.erase(it);
    s_brc121_armed_approvals[url] = id;
    LOG_INFO_HTTP("💰 BRC-121: payment approved — armed replay approvalId for " + url);
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
    bool isLocalhost = hodos::IsWalletHostPort(url);
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
