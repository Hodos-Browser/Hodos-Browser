#ifndef ADBLOCK_CACHE_H_
#define ADBLOCK_CACHE_H_

// Sprint 8: Ad & tracker blocking — C++ integration
//
// AdblockCache: Singleton that caches adblock check results and calls the
// standalone adblock engine (localhost:3302) via sync WinHTTP.
//
// AdblockBlockHandler: Minimal CefResourceRequestHandler that cancels blocked requests.
//
// CefResourceTypeToAdblock(): Maps CEF resource types to adblock-rust string names.

#include "include/cef_request_handler.h"
#include "include/cef_request.h"
#include "include/cef_browser.h"
#include "include/cef_frame.h"
#include <string>
#include <unordered_map>
#include <mutex>
#include <vector>

#ifdef _WIN32
#include <windows.h>
#include <winhttp.h>
#endif

// ============================================================================
// Global flag — set by cef_browser_shell.cpp StartAdblockServer()
// ============================================================================
extern bool g_adblockServerRunning;

// ============================================================================
// CEF Resource Type → adblock-rust resource type string
// ============================================================================

inline const char* CefResourceTypeToAdblock(cef_resource_type_t type) {
    switch (type) {
        case RT_MAIN_FRAME:      return "document";
        case RT_SUB_FRAME:       return "subdocument";
        case RT_STYLESHEET:      return "stylesheet";
        case RT_SCRIPT:          return "script";
        case RT_IMAGE:           return "image";
        case RT_FONT_RESOURCE:   return "font";
        case RT_SUB_RESOURCE:    return "other";
        case RT_OBJECT:          return "object";
        case RT_MEDIA:           return "media";
        case RT_WORKER:          return "other";
        case RT_SHARED_WORKER:   return "other";
        case RT_PREFETCH:        return "other";
        case RT_FAVICON:         return "image";
        case RT_XHR:             return "xmlhttprequest";
        case RT_PING:            return "ping";
        case RT_SERVICE_WORKER:  return "other";
        case RT_CSP_REPORT:      return "csp";
        case RT_PLUGIN_RESOURCE: return "object";
        default:                 return "other";
    }
}

// ============================================================================
// AdblockBlockHandler — cancels a blocked request
// ============================================================================

class AdblockBlockHandler : public CefResourceRequestHandler {
public:
    cef_return_value_t OnBeforeResourceLoad(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefFrame> frame,
        CefRefPtr<CefRequest> request,
        CefRefPtr<CefCallback> callback) override {
        return RV_CANCEL;
    }

    IMPLEMENT_REFCOUNTING(AdblockBlockHandler);
};

// ============================================================================
// AdblockCache — singleton for URL check results + WinHTTP backend calls
// ============================================================================

class AdblockCache {
public:
    static AdblockCache& GetInstance() {
        static AdblockCache instance;
        return instance;
    }

    // Check if a URL should be blocked.
    // Returns true if blocked. Uses cache; calls backend on cache miss.
    bool check(const std::string& url, const std::string& sourceUrl,
               const std::string& resourceType) {
        if (!g_adblockServerRunning) return false;

        // Cache lookup
        {
            std::lock_guard<std::mutex> lock(mutex_);
            auto it = cache_.find(url);
            if (it != cache_.end()) {
                return it->second;
            }
        }

        // Cache miss — call backend
        bool blocked = fetchFromBackend(url, sourceUrl, resourceType);

        // Store result
        {
            std::lock_guard<std::mutex> lock(mutex_);
            cache_[url] = blocked;
        }

        return blocked;
    }

    // Clear entire cache (on global toggle, filter list update, site toggle)
    void clearAll() {
        std::lock_guard<std::mutex> lock(mutex_);
        cache_.clear();
    }

    // Get count of cached entries (for debugging/status)
    size_t size() const {
        std::lock_guard<std::mutex> lock(mutex_);
        return cache_.size();
    }

    // Per-browser blocked count tracking (Sprint 8c)
    void incrementBlockedCount(int browserId) {
        std::lock_guard<std::mutex> lock(countMutex_);
        blockedCounts_[browserId]++;
    }

    int getBlockedCount(int browserId) const {
        std::lock_guard<std::mutex> lock(countMutex_);
        auto it = blockedCounts_.find(browserId);
        return (it != blockedCounts_.end()) ? it->second : 0;
    }

    void resetBlockedCount(int browserId) {
        std::lock_guard<std::mutex> lock(countMutex_);
        blockedCounts_.erase(browserId);
    }

    void removeBrowser(int browserId) {
        std::lock_guard<std::mutex> lock(countMutex_);
        blockedCounts_.erase(browserId);
    }

    // Check if adblock is enabled for a specific domain.
    // Queries Rust backend (GET /adblock/site-toggle?domain=X).
    // Cached per-domain to avoid repeated HTTP calls.
    bool isSiteEnabled(const std::string& domain) {
        if (domain.empty()) return true;

        // Check site-toggle cache
        {
            std::lock_guard<std::mutex> lock(siteMutex_);
            auto it = siteToggleCache_.find(domain);
            if (it != siteToggleCache_.end()) {
                return it->second;
            }
        }

        bool enabled = fetchSiteToggle(domain);

        {
            std::lock_guard<std::mutex> lock(siteMutex_);
            siteToggleCache_[domain] = enabled;
        }
        return enabled;
    }

    // Invalidate site toggle cache for a domain (after toggle change)
    void invalidateSiteToggle(const std::string& domain) {
        std::lock_guard<std::mutex> lock(siteMutex_);
        siteToggleCache_.erase(domain);
    }

    // Clear all site toggle cache entries
    void clearSiteToggles() {
        std::lock_guard<std::mutex> lock(siteMutex_);
        siteToggleCache_.clear();
    }

    // Cosmetic filtering result
    struct CosmeticResult {
        std::string cssSelectors;    // joined selectors for CSS injection
        std::string injectedScript;  // scriptlet JS (for 8e-2)
        bool generichide = false;
    };

    // Fetch cosmetic resources for a URL from the adblock engine.
    // Returns CSS selectors to hide and scriptlet JS to inject.
    CosmeticResult fetchCosmeticResources(const std::string& url) {
        if (!g_adblockServerRunning) return {};
        return fetchCosmeticFromBackend(url);
    }

    // Phase 2: Fetch generic cosmetic selectors matching DOM class names and IDs.
    // Returns comma-joined CSS selectors string.
    std::string fetchHiddenIdSelectors(const std::string& url,
                                       const std::vector<std::string>& classes,
                                       const std::vector<std::string>& ids) {
        if (!g_adblockServerRunning) return "";
        return fetchHiddenIdsFromBackend(url, classes, ids);
    }

private:
    AdblockCache() = default;
    AdblockCache(const AdblockCache&) = delete;
    AdblockCache& operator=(const AdblockCache&) = delete;

    mutable std::mutex mutex_;
    std::unordered_map<std::string, bool> cache_;

    mutable std::mutex countMutex_;
    std::unordered_map<int, int> blockedCounts_;  // browserId → blocked count

    mutable std::mutex siteMutex_;
    std::unordered_map<std::string, bool> siteToggleCache_;  // domain → adblockEnabled

    mutable std::mutex versionMutex_;
    int64_t lastKnownVersion_ = -1;  // -1 = not yet seen

#ifdef _WIN32
    // Sync WinHTTP GET to localhost:3301/adblock/site-toggle?domain=X
    bool fetchSiteToggle(const std::string& domain) {
        HINTERNET hSession = WinHttpOpen(L"AdblockSiteToggle/1.0",
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
        if (!hSession) return true;

        DWORD timeout = 2000;
        WinHttpSetOption(hSession, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hSession, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));

        HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 3301, 0);
        if (!hConnect) { WinHttpCloseHandle(hSession); return true; }

        std::string endpoint = "/adblock/site-toggle?domain=" + domain;
        std::wstring wideEndpoint(endpoint.begin(), endpoint.end());

        HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"GET",
            wideEndpoint.c_str(), nullptr, WINHTTP_NO_REFERER,
            WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
        if (!hRequest) {
            WinHttpCloseHandle(hConnect);
            WinHttpCloseHandle(hSession);
            return true;
        }

        bool enabled = true;
        if (WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0, nullptr, 0, 0, 0) &&
            WinHttpReceiveResponse(hRequest, nullptr)) {
            DWORD dwSize = 0;
            WinHttpQueryDataAvailable(hRequest, &dwSize);
            if (dwSize > 0 && dwSize < 4096) {
                std::vector<char> buf(dwSize + 1, 0);
                DWORD dwRead = 0;
                WinHttpReadData(hRequest, buf.data(), dwSize, &dwRead);
                std::string response(buf.data(), dwRead);
                // Look for "adblockEnabled":false
                if (response.find("\"adblockEnabled\":false") != std::string::npos) {
                    enabled = false;
                }
            }
        }

        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);
        WinHttpCloseHandle(hSession);
        return enabled;
    }

    // Sync WinHTTP POST to localhost:3302/check
    bool fetchFromBackend(const std::string& url, const std::string& sourceUrl,
                          const std::string& resourceType) {
        HINTERNET hSession = WinHttpOpen(L"AdblockCache/1.0",
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
        if (!hSession) return false;

        DWORD timeout = 2000; // 2s timeout — fast for localhost
        WinHttpSetOption(hSession, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hSession, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hSession, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));

        HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 3302, 0);
        if (!hConnect) {
            WinHttpCloseHandle(hSession);
            return false;
        }

        HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"POST", L"/check",
            nullptr, WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
        if (!hRequest) {
            WinHttpCloseHandle(hConnect);
            WinHttpCloseHandle(hSession);
            return false;
        }

        // Build JSON body
        std::string body = "{\"url\":\"";
        body += escapeJson(url);
        body += "\",\"sourceUrl\":\"";
        body += escapeJson(sourceUrl);
        body += "\",\"resourceType\":\"";
        body += resourceType;
        body += "\"}";

        // Set content type
        LPCWSTR contentType = L"Content-Type: application/json";
        BOOL ok = WinHttpSendRequest(hRequest, contentType, -1L,
            (LPVOID)body.c_str(), (DWORD)body.size(), (DWORD)body.size(), 0);

        bool blocked = false;

        if (ok) {
            ok = WinHttpReceiveResponse(hRequest, nullptr);
            if (ok) {
                DWORD dwSize = 0;
                WinHttpQueryDataAvailable(hRequest, &dwSize);
                if (dwSize > 0 && dwSize < 4096) {
                    std::vector<char> buf(dwSize + 1, 0);
                    DWORD dwRead = 0;
                    WinHttpReadData(hRequest, buf.data(), dwSize, &dwRead);
                    std::string response(buf.data(), dwRead);
                    // Simple parse: look for "blocked":true
                    blocked = (response.find("\"blocked\":true") != std::string::npos);

                    // Check engine version for cache invalidation (Phase 8d)
                    auto vpos = response.find("\"version\":");
                    if (vpos != std::string::npos) {
                        int64_t ver = 0;
                        size_t numStart = vpos + 10;
                        while (numStart < response.size() && response[numStart] == ' ') numStart++;
                        if (numStart < response.size()) {
                            try { ver = std::stoll(response.substr(numStart)); } catch (...) {}
                        }
                        std::lock_guard<std::mutex> vlock(versionMutex_);
                        if (lastKnownVersion_ >= 0 && ver != lastKnownVersion_) {
                            // Engine was rebuilt — clear URL cache
                            std::lock_guard<std::mutex> lock(mutex_);
                            cache_.clear();
                        }
                        lastKnownVersion_ = ver;
                    }
                }
            }
        }

        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);
        WinHttpCloseHandle(hSession);
        return blocked;
    }

    // Sync WinHTTP POST to localhost:3302/cosmetic-resources
    CosmeticResult fetchCosmeticFromBackend(const std::string& url) {
        CosmeticResult result;

        HINTERNET hSession = WinHttpOpen(L"AdblockCosmetic/1.0",
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
        if (!hSession) return result;

        DWORD timeout = 3000; // 3s — cosmetic responses can be larger
        WinHttpSetOption(hSession, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hSession, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hSession, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));

        HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 3302, 0);
        if (!hConnect) {
            WinHttpCloseHandle(hSession);
            return result;
        }

        HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"POST", L"/cosmetic-resources",
            nullptr, WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
        if (!hRequest) {
            WinHttpCloseHandle(hConnect);
            WinHttpCloseHandle(hSession);
            return result;
        }

        // Build JSON body
        std::string body = "{\"url\":\"";
        body += escapeJson(url);
        body += "\"}";

        LPCWSTR contentType = L"Content-Type: application/json";
        BOOL ok = WinHttpSendRequest(hRequest, contentType, -1L,
            (LPVOID)body.c_str(), (DWORD)body.size(), (DWORD)body.size(), 0);

        if (ok) {
            ok = WinHttpReceiveResponse(hRequest, nullptr);
            if (ok) {
                // Read all response data (cosmetic responses can be large)
                std::string response;
                DWORD dwSize = 0;
                do {
                    dwSize = 0;
                    WinHttpQueryDataAvailable(hRequest, &dwSize);
                    if (dwSize > 0) {
                        std::vector<char> buf(dwSize + 1, 0);
                        DWORD dwRead = 0;
                        WinHttpReadData(hRequest, buf.data(), dwSize, &dwRead);
                        response.append(buf.data(), dwRead);
                    }
                } while (dwSize > 0);

                if (response.size() < 512 * 1024) { // Sanity limit: 512KB
                    // Parse hideSelectors array → join into CSS selector string
                    // Response: {"hideSelectors":["sel1","sel2"],"injectedScript":"...","generichide":false}
                    auto selStart = response.find("\"hideSelectors\":");
                    if (selStart != std::string::npos) {
                        auto arrStart = response.find('[', selStart);
                        auto arrEnd = response.find(']', arrStart);
                        if (arrStart != std::string::npos && arrEnd != std::string::npos) {
                            std::string arrStr = response.substr(arrStart + 1, arrEnd - arrStart - 1);
                            // Parse quoted strings from array
                            std::string selectors;
                            size_t pos = 0;
                            while (pos < arrStr.size()) {
                                auto qStart = arrStr.find('"', pos);
                                if (qStart == std::string::npos) break;
                                auto qEnd = arrStr.find('"', qStart + 1);
                                // Handle escaped quotes
                                while (qEnd != std::string::npos && arrStr[qEnd - 1] == '\\') {
                                    qEnd = arrStr.find('"', qEnd + 1);
                                }
                                if (qEnd == std::string::npos) break;
                                std::string sel = arrStr.substr(qStart + 1, qEnd - qStart - 1);
                                if (!sel.empty()) {
                                    if (!selectors.empty()) selectors += ", ";
                                    selectors += sel;
                                }
                                pos = qEnd + 1;
                            }
                            result.cssSelectors = selectors;
                        }
                    }

                    // Parse injectedScript string
                    auto scriptStart = response.find("\"injectedScript\":\"");
                    if (scriptStart != std::string::npos) {
                        size_t valStart = scriptStart + 18; // past "injectedScript":"
                        // Find unescaped closing quote
                        size_t valEnd = valStart;
                        while (valEnd < response.size()) {
                            if (response[valEnd] == '"' && response[valEnd - 1] != '\\') break;
                            valEnd++;
                        }
                        if (valEnd < response.size()) {
                            result.injectedScript = response.substr(valStart, valEnd - valStart);
                            // Unescape JSON string escapes
                            std::string& s = result.injectedScript;
                            std::string unescaped;
                            unescaped.reserve(s.size());
                            for (size_t i = 0; i < s.size(); i++) {
                                if (s[i] == '\\' && i + 1 < s.size()) {
                                    switch (s[i + 1]) {
                                        case 'n': unescaped += '\n'; i++; break;
                                        case 'r': unescaped += '\r'; i++; break;
                                        case 't': unescaped += '\t'; i++; break;
                                        case '"': unescaped += '"'; i++; break;
                                        case '\\': unescaped += '\\'; i++; break;
                                        default: unescaped += s[i]; break;
                                    }
                                } else {
                                    unescaped += s[i];
                                }
                            }
                            result.injectedScript = unescaped;
                        }
                    }

                    // Parse generichide boolean
                    result.generichide = (response.find("\"generichide\":true") != std::string::npos);
                }
            }
        }

        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);
        WinHttpCloseHandle(hSession);
        return result;
    }

    // Sync WinHTTP POST to localhost:3302/cosmetic-hidden-ids
    // Phase 2: generic selectors matching DOM class names and element IDs
    std::string fetchHiddenIdsFromBackend(const std::string& url,
                                          const std::vector<std::string>& classes,
                                          const std::vector<std::string>& ids) {
        HINTERNET hSession = WinHttpOpen(L"AdblockHiddenIds/1.0",
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
        if (!hSession) return "";

        DWORD timeout = 3000;
        WinHttpSetOption(hSession, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hSession, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
        WinHttpSetOption(hSession, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));

        HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 3302, 0);
        if (!hConnect) {
            WinHttpCloseHandle(hSession);
            return "";
        }

        HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"POST", L"/cosmetic-hidden-ids",
            nullptr, WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
        if (!hRequest) {
            WinHttpCloseHandle(hConnect);
            WinHttpCloseHandle(hSession);
            return "";
        }

        // Build JSON body: {"url":"...","classes":["c1","c2"],"ids":["i1","i2"]}
        std::string body = "{\"url\":\"";
        body += escapeJson(url);
        body += "\",\"classes\":[";
        for (size_t i = 0; i < classes.size(); i++) {
            if (i > 0) body += ",";
            body += "\"";
            body += escapeJson(classes[i]);
            body += "\"";
        }
        body += "],\"ids\":[";
        for (size_t i = 0; i < ids.size(); i++) {
            if (i > 0) body += ",";
            body += "\"";
            body += escapeJson(ids[i]);
            body += "\"";
        }
        body += "]}";

        LPCWSTR contentType = L"Content-Type: application/json";
        BOOL ok = WinHttpSendRequest(hRequest, contentType, -1L,
            (LPVOID)body.c_str(), (DWORD)body.size(), (DWORD)body.size(), 0);

        std::string selectors;

        if (ok) {
            ok = WinHttpReceiveResponse(hRequest, nullptr);
            if (ok) {
                std::string response;
                DWORD dwSize = 0;
                do {
                    dwSize = 0;
                    WinHttpQueryDataAvailable(hRequest, &dwSize);
                    if (dwSize > 0) {
                        std::vector<char> buf(dwSize + 1, 0);
                        DWORD dwRead = 0;
                        WinHttpReadData(hRequest, buf.data(), dwSize, &dwRead);
                        response.append(buf.data(), dwRead);
                    }
                } while (dwSize > 0);

                if (response.size() < 512 * 1024) {
                    // Parse selectors array: {"selectors":["sel1","sel2"]}
                    auto arrStart = response.find('[');
                    auto arrEnd = response.rfind(']');
                    if (arrStart != std::string::npos && arrEnd != std::string::npos && arrEnd > arrStart) {
                        std::string arrStr = response.substr(arrStart + 1, arrEnd - arrStart - 1);
                        size_t pos = 0;
                        while (pos < arrStr.size()) {
                            auto qStart = arrStr.find('"', pos);
                            if (qStart == std::string::npos) break;
                            auto qEnd = arrStr.find('"', qStart + 1);
                            while (qEnd != std::string::npos && arrStr[qEnd - 1] == '\\') {
                                qEnd = arrStr.find('"', qEnd + 1);
                            }
                            if (qEnd == std::string::npos) break;
                            std::string sel = arrStr.substr(qStart + 1, qEnd - qStart - 1);
                            if (!sel.empty()) {
                                if (!selectors.empty()) selectors += ", ";
                                selectors += sel;
                            }
                            pos = qEnd + 1;
                        }
                    }
                }
            }
        }

        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);
        WinHttpCloseHandle(hSession);
        return selectors;
    }

    // Escape JSON string (minimal: backslash and double-quote)
    static std::string escapeJson(const std::string& s) {
        std::string result;
        result.reserve(s.size() + 16);
        for (char c : s) {
            switch (c) {
                case '"':  result += "\\\""; break;
                case '\\': result += "\\\\"; break;
                case '\n': result += "\\n";  break;
                case '\r': result += "\\r";  break;
                case '\t': result += "\\t";  break;
                default:   result += c;      break;
            }
        }
        return result;
    }
#elif defined(__APPLE__)
    // macOS stub — TODO: implement with libcurl or NSURLSession
    bool fetchSiteToggle(const std::string& domain) { return true; }
    bool fetchFromBackend(const std::string& url, const std::string& sourceUrl,
                          const std::string& resourceType) {
        return false;
    }
    CosmeticResult fetchCosmeticFromBackend(const std::string& url) { return {}; }
    std::string fetchHiddenIdsFromBackend(const std::string& url,
                                          const std::vector<std::string>& classes,
                                          const std::vector<std::string>& ids) { return ""; }
#else
    bool fetchSiteToggle(const std::string& domain) { return true; }
    bool fetchFromBackend(const std::string& url, const std::string& sourceUrl,
                          const std::string& resourceType) {
        return false;
    }
    CosmeticResult fetchCosmeticFromBackend(const std::string& url) { return {}; }
    std::string fetchHiddenIdsFromBackend(const std::string& url,
                                          const std::vector<std::string>& classes,
                                          const std::vector<std::string>& ids) { return ""; }
#endif
};

// ============================================================================
// Helper: check if a URL should skip adblock checking
// ============================================================================

inline bool shouldSkipAdblockCheck(const std::string& url) {
    // Skip internal/local URLs
    if (url.find("localhost") != std::string::npos) return true;
    if (url.find("127.0.0.1") != std::string::npos) return true;
    if (url.substr(0, 5) == "data:") return true;
    if (url.substr(0, 5) == "blob:") return true;
    if (url.substr(0, 7) == "chrome:") return true;
    if (url.substr(0, 17) == "chrome-extension:") return true;
    if (url.substr(0, 16) == "devtools://") return true;
    return false;
}

#endif // ADBLOCK_CACHE_H_
