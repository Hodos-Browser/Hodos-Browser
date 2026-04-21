#pragma once

#include <sqlite3.h>
#include <string>
#include <vector>
#include <unordered_set>
#include <unordered_map>
#include <shared_mutex>
#include <cstdint>

#include "include/cef_resource_request_handler.h"
#include "include/cef_browser.h"
#include "include/cef_frame.h"
#include "include/cef_request.h"
#include "include/cef_response.h"
#include "include/cef_cookie.h"

struct BlockedDomainEntry {
    std::string domain;
    bool is_wildcard;
    std::string source;
    int64_t created_at;
};

struct BlockLogEntry {
    std::string cookie_domain;
    std::string page_url;
    std::string reason;
    int64_t blocked_at;
};

class CookieBlockManager {
public:
    static CookieBlockManager& GetInstance();

    // Initialize with CEF user data path - creates cookie_blocks.db
    bool Initialize(const std::string& user_data_path);

    // Domain blocking
    bool AddBlockedDomain(const std::string& domain, bool is_wildcard, const std::string& source);
    bool RemoveBlockedDomain(const std::string& domain);
    std::string GetBlockedDomains();
    bool IsDomainBlocked(const std::string& domain);

    // Third-party allow list
    bool AddAllowedThirdParty(const std::string& domain);
    bool RemoveAllowedThirdParty(const std::string& domain);
    bool IsThirdPartyAllowed(const std::string& domain);

    // Block log
    std::string GetBlockLog(int limit, int offset);
    bool ClearBlockLog();

    // Per-browser blocked counts (for badge display)
    int GetBlockedCountForBrowser(int browser_id);
    void ResetBlockedCount(int browser_id);

    // Cookie filtering methods (called on IO thread by CookieAccessFilterWrapper)
    bool CanSendCookie(CefRefPtr<CefBrowser> browser,
                       CefRefPtr<CefFrame> frame,
                       CefRefPtr<CefRequest> request,
                       const CefCookie& cookie);

    bool CanSaveCookie(CefRefPtr<CefBrowser> browser,
                       CefRefPtr<CefFrame> frame,
                       CefRefPtr<CefRequest> request,
                       CefRefPtr<CefResponse> response,
                       const CefCookie& cookie);

    // Check if initialized
    bool IsInitialized() const { return db_ != nullptr; }

private:
    CookieBlockManager() = default;
    ~CookieBlockManager();

    sqlite3* db_ = nullptr;
    std::string db_path_;

    // In-memory sets for O(1) IO-thread lookups (protected by shared_mutex)
    mutable std::shared_mutex block_mutex_;
    std::unordered_set<std::string> blocked_domains_;      // exact match
    std::unordered_set<std::string> wildcard_suffixes_;     // for *.domain patterns
    std::unordered_set<std::string> allowed_third_party_;   // third-party exceptions

    // Per-browser blocked cookie counts
    mutable std::shared_mutex count_mutex_;
    std::unordered_map<int, int> blocked_counts_;

    // Database operations
    bool OpenDatabase();
    void CloseDatabase();
    bool LoadBlockListIntoMemory();
    bool PopulateDefaultTrackers();
    void PurgeOldLogs();

    // Domain matching (thread-safe read with shared_lock)
    bool MatchesDomain(const std::string& cookie_domain);
    bool IsThirdParty(const std::string& cookie_domain, const std::string& page_domain);

    // Async log (posts to FILE thread)
    void LogBlockedCookie(const std::string& domain, const std::string& page_url, const std::string& reason);

    // Utility
    static std::string ExtractDomain(const std::string& url);
    static std::string NormalizeDomain(const std::string& domain);

    // Prevent copying
    CookieBlockManager(const CookieBlockManager&) = delete;
    CookieBlockManager& operator=(const CookieBlockManager&) = delete;
};

// CookieAccessFilterWrapper - Refcounted wrapper that delegates to singleton CookieBlockManager
// This is necessary because CEF expects refcounted objects, but CookieBlockManager is a singleton.
// Use this when you need to return a CefRefPtr<CefCookieAccessFilter>.
class CookieAccessFilterWrapper : public CefCookieAccessFilter {
public:
    bool CanSendCookie(CefRefPtr<CefBrowser> browser,
                       CefRefPtr<CefFrame> frame,
                       CefRefPtr<CefRequest> request,
                       const CefCookie& cookie) override {
        return CookieBlockManager::GetInstance().CanSendCookie(browser, frame, request, cookie);
    }

    bool CanSaveCookie(CefRefPtr<CefBrowser> browser,
                       CefRefPtr<CefFrame> frame,
                       CefRefPtr<CefRequest> request,
                       CefRefPtr<CefResponse> response,
                       const CefCookie& cookie) override {
        return CookieBlockManager::GetInstance().CanSaveCookie(browser, frame, request, response, cookie);
    }

    IMPLEMENT_REFCOUNTING(CookieAccessFilterWrapper);
};
