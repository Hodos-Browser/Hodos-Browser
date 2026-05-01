#include "../../include/core/CookieBlockManager.h"
#include "../../include/core/EphemeralCookieManager.h"
#include "../../include/core/SettingsManager.h"
#include "../../include/core/Logger.h"
#include "DefaultTrackerList.h"

#include "include/cef_task.h"
#include "include/wrapper/cef_helpers.h"

#include <nlohmann/json.hpp>
#include <chrono>
#include <algorithm>
#include <sstream>

// Logging macros
#define LOG_DEBUG_BLOCK(msg) Logger::Log(msg, 0, 2)
#define LOG_INFO_BLOCK(msg) Logger::Log(msg, 1, 2)
#define LOG_WARNING_BLOCK(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_BLOCK(msg) Logger::Log(msg, 3, 2)

// ============================================================================
// LogBlockedCookieTask - Posts blocked cookie log INSERT to FILE thread.
// SQLite must NOT run on IO thread.
// ============================================================================
class LogBlockedCookieTask : public CefTask {
public:
    LogBlockedCookieTask(sqlite3* db,
                         const std::string& domain,
                         const std::string& page_url,
                         const std::string& reason,
                         int64_t blocked_at)
        : db_(db), domain_(domain), page_url_(page_url),
          reason_(reason), blocked_at_(blocked_at) {}

    void Execute() override {
        if (!db_) return;

        const char* sql = "INSERT INTO block_log (cookie_domain, page_url, reason, blocked_at) "
                          "VALUES (?, ?, ?, ?)";
        sqlite3_stmt* stmt = nullptr;
        if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
            sqlite3_bind_text(stmt, 1, domain_.c_str(), -1, SQLITE_TRANSIENT);
            sqlite3_bind_text(stmt, 2, page_url_.c_str(), -1, SQLITE_TRANSIENT);
            sqlite3_bind_text(stmt, 3, reason_.c_str(), -1, SQLITE_TRANSIENT);
            sqlite3_bind_int64(stmt, 4, blocked_at_);
            sqlite3_step(stmt);
            sqlite3_finalize(stmt);
        }
    }

private:
    sqlite3* db_;
    std::string domain_;
    std::string page_url_;
    std::string reason_;
    int64_t blocked_at_;

    IMPLEMENT_REFCOUNTING(LogBlockedCookieTask);
};

// ============================================================================
// CookieBlockManager singleton
// ============================================================================

CookieBlockManager& CookieBlockManager::GetInstance() {
    static CookieBlockManager instance;
    return instance;
}

CookieBlockManager::~CookieBlockManager() {
    CloseDatabase();
}

// ============================================================================
// Initialize
// ============================================================================
bool CookieBlockManager::Initialize(const std::string& user_data_path) {
#ifdef _WIN32
    db_path_ = user_data_path + "\\cookie_blocks.db";
#else
    db_path_ = user_data_path + "/cookie_blocks.db";
#endif
    LOG_INFO_BLOCK("Initializing CookieBlockManager at: " + db_path_);

    if (!OpenDatabase()) {
        LOG_ERROR_BLOCK("Failed to open CookieBlockManager database");
        return false;
    }

    // Check meta table for initial population flag
    bool already_populated = false;
    {
        const char* sql = "SELECT value FROM meta WHERE key = 'initial_populated'";
        sqlite3_stmt* stmt = nullptr;
        if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
            if (sqlite3_step(stmt) == SQLITE_ROW) {
                already_populated = true;
            }
            sqlite3_finalize(stmt);
        }
    }

    if (!already_populated) {
        LOG_INFO_BLOCK("First run - populating default tracker list");
        if (PopulateDefaultTrackers()) {
            const char* sql = "INSERT INTO meta (key, value) VALUES ('initial_populated', 'true')";
            sqlite3_exec(db_, sql, nullptr, nullptr, nullptr);
            LOG_INFO_BLOCK("Default trackers populated successfully");
        } else {
            LOG_ERROR_BLOCK("Failed to populate default trackers");
        }
    }

    if (!LoadBlockListIntoMemory()) {
        LOG_ERROR_BLOCK("Failed to load block list into memory");
        return false;
    }

    PurgeOldLogs();

    LOG_INFO_BLOCK("CookieBlockManager initialized successfully. Blocked domains: "
                   + std::to_string(blocked_domains_.size())
                   + ", Wildcard suffixes: " + std::to_string(wildcard_suffixes_.size())
                   + ", Allowed third-party: " + std::to_string(allowed_third_party_.size()));
    return true;
}

// ============================================================================
// Database operations
// ============================================================================
bool CookieBlockManager::OpenDatabase() {
    int rc = sqlite3_open(db_path_.c_str(), &db_);
    if (rc != SQLITE_OK) {
        LOG_ERROR_BLOCK("sqlite3_open failed: " + std::string(sqlite3_errmsg(db_)));
        db_ = nullptr;
        return false;
    }

    // WAL mode for concurrent reads
    sqlite3_exec(db_, "PRAGMA journal_mode=WAL;", nullptr, nullptr, nullptr);
    sqlite3_busy_timeout(db_, 5000);

    // Create schema
    const char* schema = R"SQL(
        CREATE TABLE IF NOT EXISTS blocked_domains (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            domain TEXT NOT NULL UNIQUE,
            is_wildcard INTEGER DEFAULT 0,
            source TEXT DEFAULT 'user',
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS allowed_third_party (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            domain TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS block_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            cookie_domain TEXT NOT NULL,
            page_url TEXT NOT NULL,
            reason TEXT NOT NULL,
            blocked_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_blocked_domain ON blocked_domains(domain);
        CREATE INDEX IF NOT EXISTS idx_block_log_time ON block_log(blocked_at);
        CREATE INDEX IF NOT EXISTS idx_block_log_domain ON block_log(cookie_domain);
    )SQL";

    char* errMsg = nullptr;
    rc = sqlite3_exec(db_, schema, nullptr, nullptr, &errMsg);
    if (rc != SQLITE_OK) {
        LOG_ERROR_BLOCK("Schema creation failed: " + std::string(errMsg ? errMsg : "unknown error"));
        if (errMsg) sqlite3_free(errMsg);
        return false;
    }

    LOG_INFO_BLOCK("CookieBlockManager database opened and schema created");
    return true;
}

void CookieBlockManager::CloseDatabase() {
    if (db_) {
        sqlite3_close(db_);
        db_ = nullptr;
    }
}

bool CookieBlockManager::LoadBlockListIntoMemory() {
    if (!db_) return false;

    std::unique_lock<std::shared_mutex> lock(block_mutex_);

    blocked_domains_.clear();
    wildcard_suffixes_.clear();
    allowed_third_party_.clear();

    // Load blocked domains
    {
        const char* sql = "SELECT domain, is_wildcard FROM blocked_domains";
        sqlite3_stmt* stmt = nullptr;
        if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
            while (sqlite3_step(stmt) == SQLITE_ROW) {
                std::string domain = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));
                int is_wildcard = sqlite3_column_int(stmt, 1);
                if (is_wildcard) {
                    wildcard_suffixes_.insert(domain);
                } else {
                    blocked_domains_.insert(domain);
                }
            }
            sqlite3_finalize(stmt);
        }
    }

    // Load allowed third-party domains
    {
        const char* sql = "SELECT domain FROM allowed_third_party";
        sqlite3_stmt* stmt = nullptr;
        if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
            while (sqlite3_step(stmt) == SQLITE_ROW) {
                std::string domain = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));
                allowed_third_party_.insert(domain);
            }
            sqlite3_finalize(stmt);
        }
    }

    return true;
}

bool CookieBlockManager::PopulateDefaultTrackers() {
    if (!db_) return false;

    auto now = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::system_clock::now().time_since_epoch()).count();

    const char* sql = "INSERT OR IGNORE INTO blocked_domains (domain, is_wildcard, source, created_at) "
                      "VALUES (?, ?, 'default', ?)";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
        LOG_ERROR_BLOCK("Failed to prepare default tracker insert statement");
        return false;
    }

    for (const auto& tracker : DEFAULT_TRACKERS) {
        sqlite3_reset(stmt);
        sqlite3_bind_text(stmt, 1, tracker.first.c_str(), -1, SQLITE_TRANSIENT);
        sqlite3_bind_int(stmt, 2, tracker.second ? 1 : 0);
        sqlite3_bind_int64(stmt, 3, now);
        sqlite3_step(stmt);
    }

    sqlite3_finalize(stmt);
    LOG_INFO_BLOCK("Populated " + std::to_string(DEFAULT_TRACKERS.size()) + " default tracker domains");
    return true;
}

void CookieBlockManager::PurgeOldLogs() {
    if (!db_) return;

    auto now = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::system_clock::now().time_since_epoch()).count();
    int64_t thirty_days_ms = static_cast<int64_t>(30) * 24 * 60 * 60 * 1000;
    int64_t cutoff = now - thirty_days_ms;

    const char* sql = "DELETE FROM block_log WHERE blocked_at < ?";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
        sqlite3_bind_int64(stmt, 1, cutoff);
        sqlite3_step(stmt);
        int deleted = sqlite3_changes(db_);
        sqlite3_finalize(stmt);
        if (deleted > 0) {
            LOG_INFO_BLOCK("Purged " + std::to_string(deleted) + " old block log entries");
        }
    }
}

// ============================================================================
// Domain blocking methods
// ============================================================================
bool CookieBlockManager::AddBlockedDomain(const std::string& domain, bool is_wildcard,
                                           const std::string& source) {
    if (!db_) return false;

    std::string normalized = NormalizeDomain(domain);

    auto now = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::system_clock::now().time_since_epoch()).count();

    const char* sql = "INSERT OR IGNORE INTO blocked_domains (domain, is_wildcard, source, created_at) "
                      "VALUES (?, ?, ?, ?)";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
        return false;
    }
    sqlite3_bind_text(stmt, 1, normalized.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_int(stmt, 2, is_wildcard ? 1 : 0);
    sqlite3_bind_text(stmt, 3, source.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_int64(stmt, 4, now);
    int rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE) {
        LOG_ERROR_BLOCK("Failed to add blocked domain: " + normalized);
        return false;
    }

    // Update in-memory set
    {
        std::unique_lock<std::shared_mutex> lock(block_mutex_);
        if (is_wildcard) {
            wildcard_suffixes_.insert(normalized);
        } else {
            blocked_domains_.insert(normalized);
        }
    }

    LOG_INFO_BLOCK("Added blocked domain: " + normalized + " (wildcard: " + (is_wildcard ? "true" : "false") + ")");
    return true;
}

bool CookieBlockManager::RemoveBlockedDomain(const std::string& domain) {
    if (!db_) return false;

    std::string normalized = NormalizeDomain(domain);

    const char* sql = "DELETE FROM blocked_domains WHERE domain = ?";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
        return false;
    }
    sqlite3_bind_text(stmt, 1, normalized.c_str(), -1, SQLITE_TRANSIENT);
    int rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE) {
        LOG_ERROR_BLOCK("Failed to remove blocked domain: " + normalized);
        return false;
    }

    // Update in-memory sets
    {
        std::unique_lock<std::shared_mutex> lock(block_mutex_);
        blocked_domains_.erase(normalized);
        wildcard_suffixes_.erase(normalized);
    }

    LOG_INFO_BLOCK("Removed blocked domain: " + normalized);
    return true;
}

std::string CookieBlockManager::GetBlockedDomains() {
    if (!db_) return "[]";

    nlohmann::json result = nlohmann::json::array();

    const char* sql = "SELECT domain, is_wildcard, source, created_at FROM blocked_domains ORDER BY created_at DESC";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
        while (sqlite3_step(stmt) == SQLITE_ROW) {
            nlohmann::json entry;
            entry["domain"] = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));
            entry["isWildcard"] = sqlite3_column_int(stmt, 1) != 0;
            entry["source"] = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 2));
            entry["createdAt"] = sqlite3_column_int64(stmt, 3);
            result.push_back(entry);
        }
        sqlite3_finalize(stmt);
    }

    return result.dump();
}

bool CookieBlockManager::IsDomainBlocked(const std::string& domain) {
    return MatchesDomain(NormalizeDomain(domain));
}

// ============================================================================
// Third-party allow list
// ============================================================================
bool CookieBlockManager::AddAllowedThirdParty(const std::string& domain) {
    if (!db_) return false;

    std::string normalized = NormalizeDomain(domain);

    auto now = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::system_clock::now().time_since_epoch()).count();

    const char* sql = "INSERT OR IGNORE INTO allowed_third_party (domain, created_at) VALUES (?, ?)";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
        return false;
    }
    sqlite3_bind_text(stmt, 1, normalized.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_int64(stmt, 2, now);
    int rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE) return false;

    // Update in-memory set
    {
        std::unique_lock<std::shared_mutex> lock(block_mutex_);
        allowed_third_party_.insert(normalized);
    }

    LOG_INFO_BLOCK("Added allowed third-party domain: " + normalized);
    return true;
}

bool CookieBlockManager::RemoveAllowedThirdParty(const std::string& domain) {
    if (!db_) return false;

    std::string normalized = NormalizeDomain(domain);

    const char* sql = "DELETE FROM allowed_third_party WHERE domain = ?";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
        return false;
    }
    sqlite3_bind_text(stmt, 1, normalized.c_str(), -1, SQLITE_TRANSIENT);
    int rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE) return false;

    // Update in-memory set
    {
        std::unique_lock<std::shared_mutex> lock(block_mutex_);
        allowed_third_party_.erase(normalized);
    }

    LOG_INFO_BLOCK("Removed allowed third-party domain: " + normalized);
    return true;
}

bool CookieBlockManager::IsThirdPartyAllowed(const std::string& domain) {
    std::shared_lock<std::shared_mutex> lock(block_mutex_);
    return allowed_third_party_.count(NormalizeDomain(domain)) > 0;
}

// ============================================================================
// Block log
// ============================================================================
std::string CookieBlockManager::GetBlockLog(int limit, int offset) {
    if (!db_) return "[]";

    nlohmann::json result = nlohmann::json::array();

    const char* sql = "SELECT cookie_domain, page_url, reason, blocked_at "
                      "FROM block_log ORDER BY blocked_at DESC LIMIT ? OFFSET ?";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
        sqlite3_bind_int(stmt, 1, limit);
        sqlite3_bind_int(stmt, 2, offset);
        while (sqlite3_step(stmt) == SQLITE_ROW) {
            nlohmann::json entry;
            entry["cookie_domain"] = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 0));
            entry["page_url"] = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 1));
            entry["reason"] = reinterpret_cast<const char*>(sqlite3_column_text(stmt, 2));
            entry["blocked_at"] = sqlite3_column_int64(stmt, 3);
            result.push_back(entry);
        }
        sqlite3_finalize(stmt);
    }

    return result.dump();
}

bool CookieBlockManager::ClearBlockLog() {
    if (!db_) return false;

    int rc = sqlite3_exec(db_, "DELETE FROM block_log", nullptr, nullptr, nullptr);
    if (rc != SQLITE_OK) {
        LOG_ERROR_BLOCK("Failed to clear block log");
        return false;
    }

    LOG_INFO_BLOCK("Block log cleared");
    return true;
}

// ============================================================================
// Per-browser blocked counts
// ============================================================================
int CookieBlockManager::GetBlockedCountForBrowser(int browser_id) {
    std::shared_lock<std::shared_mutex> lock(count_mutex_);
    auto it = blocked_counts_.find(browser_id);
    return (it != blocked_counts_.end()) ? it->second : 0;
}

void CookieBlockManager::ResetBlockedCount(int browser_id) {
    std::unique_lock<std::shared_mutex> lock(count_mutex_);
    blocked_counts_[browser_id] = 0;
}

// ============================================================================
// CefCookieAccessFilter implementation (IO thread -- must be fast)
// ============================================================================
bool CookieBlockManager::CanSendCookie(CefRefPtr<CefBrowser> browser,
                                        CefRefPtr<CefFrame> frame,
                                        CefRefPtr<CefRequest> request,
                                        const CefCookie& cookie) {
    // Skip localhost/internal requests
    std::string url = request->GetURL().ToString();
    if (url.find("localhost") != std::string::npos ||
        url.find("127.0.0.1") != std::string::npos) {
        return true;
    }

    // Get cookie domain
    std::string cookie_domain = CefString(&cookie.domain).ToString();
    std::string normalized_cookie = NormalizeDomain(cookie_domain);

    // Check blocked domain list — known trackers always blocked
    if (MatchesDomain(normalized_cookie)) {
        LogBlockedCookie(normalized_cookie, url, "blocked_domain");

        if (browser) {
            std::unique_lock<std::shared_mutex> lock(count_mutex_);
            blocked_counts_[browser->GetIdentifier()]++;
        }
        return false;
    }

    // Global third-party cookie toggle — if off, allow all non-blocked third-party cookies
    if (!SettingsManager::GetInstance().GetPrivacySettings().thirdPartyCookieBlocking) {
        return true;
    }

    // Determine the page context for third-party checks.
    // IMPORTANT: Prefer GetFirstPartyForCookies() over frame->GetURL().
    // During navigation, frame->GetURL() returns the PREVIOUS page's URL (stale),
    // while GetFirstPartyForCookies() always reflects the correct target URL.
    std::string page_url = request->GetFirstPartyForCookies().ToString();
    if (page_url.empty() && frame) {
        page_url = frame->GetURL().ToString();
    }

    // Skip if page context is localhost (e.g. navigating away from dev server homepage)
    if (!page_url.empty() &&
        (page_url.find("localhost") != std::string::npos ||
         page_url.find("127.0.0.1") != std::string::npos)) {
        return true;
    }

    if (!page_url.empty()) {
        std::string page_domain = NormalizeDomain(ExtractDomain(page_url));
        if (!page_domain.empty() && IsThirdParty(normalized_cookie, page_domain)) {
            // Ephemeral check: use the TOP-LEVEL site the user is browsing.
            // For subframes, frame->GetURL() returns the iframe URL, not what's in the
            // address bar. Use browser->GetMainFrame() for the true top-level context,
            // falling back to GetFirstPartyForCookies().
            std::string ephemeral_url;
            if (browser) {
                auto main_frame = browser->GetMainFrame();
                if (main_frame) {
                    ephemeral_url = main_frame->GetURL().ToString();
                }
            }
            // Fall back if main frame URL is empty or localhost (during initial navigation)
            if (ephemeral_url.empty() ||
                ephemeral_url.find("localhost") != std::string::npos ||
                ephemeral_url.find("127.0.0.1") != std::string::npos) {
                ephemeral_url = page_url;
            }

            std::string top_level_site = EphemeralCookieManager::ExtractSiteFromUrl(ephemeral_url);
            if (!top_level_site.empty() &&
                EphemeralCookieManager::GetInstance().IsSiteActive(top_level_site)) {
                return true; // Site is active — allow third-party cookie
            }

            // Site not active — block third-party cookie
            LogBlockedCookie(normalized_cookie, url, "third_party");

            if (browser) {
                std::unique_lock<std::shared_mutex> lock(count_mutex_);
                blocked_counts_[browser->GetIdentifier()]++;
            }
            return false;
        }
    }

    return true;
}

bool CookieBlockManager::CanSaveCookie(CefRefPtr<CefBrowser> browser,
                                        CefRefPtr<CefFrame> frame,
                                        CefRefPtr<CefRequest> request,
                                        CefRefPtr<CefResponse> response,
                                        const CefCookie& cookie) {
    // Skip localhost/internal requests
    std::string url = request->GetURL().ToString();
    if (url.find("localhost") != std::string::npos ||
        url.find("127.0.0.1") != std::string::npos) {
        return true;
    }

    // Get cookie domain
    std::string cookie_domain = CefString(&cookie.domain).ToString();
    std::string normalized_cookie = NormalizeDomain(cookie_domain);

    // Check blocked domain list — known trackers always blocked
    if (MatchesDomain(normalized_cookie)) {
        LogBlockedCookie(normalized_cookie, url, "blocked_domain");

        if (browser) {
            std::unique_lock<std::shared_mutex> lock(count_mutex_);
            blocked_counts_[browser->GetIdentifier()]++;
        }
        return false;
    }

    // Global third-party cookie toggle — if off, allow all non-blocked third-party cookies
    if (!SettingsManager::GetInstance().GetPrivacySettings().thirdPartyCookieBlocking) {
        return true;
    }

    // Determine the page context for third-party checks.
    // IMPORTANT: Prefer GetFirstPartyForCookies() over frame->GetURL().
    // During navigation, frame->GetURL() returns the PREVIOUS page's URL (stale),
    // while GetFirstPartyForCookies() always reflects the correct target URL.
    std::string page_url = request->GetFirstPartyForCookies().ToString();
    if (page_url.empty() && frame) {
        page_url = frame->GetURL().ToString();
    }

    // Skip if page context is localhost (e.g. navigating away from dev server homepage)
    if (!page_url.empty() &&
        (page_url.find("localhost") != std::string::npos ||
         page_url.find("127.0.0.1") != std::string::npos)) {
        return true;
    }

    if (!page_url.empty()) {
        std::string page_domain = NormalizeDomain(ExtractDomain(page_url));
        if (!page_domain.empty() && IsThirdParty(normalized_cookie, page_domain)) {
            // Ephemeral check: use the TOP-LEVEL site the user is browsing.
            // For subframes, frame->GetURL() returns the iframe URL, not what's in the
            // address bar. Use browser->GetMainFrame() for the true top-level context,
            // falling back to GetFirstPartyForCookies().
            std::string ephemeral_url;
            if (browser) {
                auto main_frame = browser->GetMainFrame();
                if (main_frame) {
                    ephemeral_url = main_frame->GetURL().ToString();
                }
            }
            // Fall back if main frame URL is empty or localhost (during initial navigation)
            if (ephemeral_url.empty() ||
                ephemeral_url.find("localhost") != std::string::npos ||
                ephemeral_url.find("127.0.0.1") != std::string::npos) {
                ephemeral_url = page_url;
            }

            std::string top_level_site = EphemeralCookieManager::ExtractSiteFromUrl(ephemeral_url);
            if (!top_level_site.empty() &&
                EphemeralCookieManager::GetInstance().IsSiteActive(top_level_site)) {
                // Record this third-party domain for cleanup when site session ends
                EphemeralCookieManager::GetInstance().RecordThirdPartyCookie(
                    top_level_site, normalized_cookie);
                return true; // Site is active — allow and record for later cleanup
            }

            // Site not active — block third-party cookie
            LogBlockedCookie(normalized_cookie, url, "third_party");

            if (browser) {
                std::unique_lock<std::shared_mutex> lock(count_mutex_);
                blocked_counts_[browser->GetIdentifier()]++;
            }
            return false;
        }
    }

    return true;
}

// ============================================================================
// Domain matching (IO thread - shared_lock for concurrent reads)
// ============================================================================
bool CookieBlockManager::MatchesDomain(const std::string& cookie_domain) {
    std::shared_lock<std::shared_mutex> lock(block_mutex_);

    // Exact match in blocked_domains_
    if (blocked_domains_.count(cookie_domain) > 0) {
        return true;
    }

    // Walk domain hierarchy checking wildcard_suffixes_
    // e.g., for "sub.tracker.com", check "sub.tracker.com", "tracker.com", "com"
    std::string domain = cookie_domain;
    while (true) {
        if (wildcard_suffixes_.count(domain) > 0) {
            return true;
        }
        size_t dot = domain.find('.');
        if (dot == std::string::npos) break;
        domain = domain.substr(dot + 1);
        if (domain.empty()) break;
    }

    return false;
}

// Same-entity domain groups where cookie sharing is needed for auth/functionality.
// Intentionally NOT the full disconnect.me entity list — we exclude tracking/ad
// domains (e.g. doubleclick.net) that share a parent org with auth domains.
// Both inputs should be NormalizeDomain() output (eTLD+1, lowercase).
static bool AreSameAuthEntity(const std::string& a, const std::string& b) {
    struct EntityGroup {
        const char* domains[8];  // null-terminated
    };
    static const EntityGroup groups[] = {
        {{"x.com", "twitter.com", "twimg.com", nullptr}},
        {{"google.com", "youtube.com", "googleapis.com", "gstatic.com", "googlevideo.com", nullptr}},
        {{"facebook.com", "instagram.com", "fbcdn.net", "facebook.net", nullptr}},
        {{"microsoft.com", "microsoftonline.com", "live.com", "msn.com", "bing.com", nullptr}},
        {{"amazon.com", "amazonaws.com", nullptr}},
        {{"apple.com", "icloud.com", "mzstatic.com", nullptr}},
        {{"github.com", "githubassets.com", "githubusercontent.com", nullptr}},
    };
    for (const auto& group : groups) {
        bool a_found = false, b_found = false;
        for (int i = 0; group.domains[i] != nullptr; i++) {
            if (a == group.domains[i]) a_found = true;
            if (b == group.domains[i]) b_found = true;
        }
        if (a_found && b_found) return true;
    }
    return false;
}

bool CookieBlockManager::IsThirdParty(const std::string& cookie_domain,
                                       const std::string& page_domain) {
    // Same domain = first-party
    if (cookie_domain == page_domain) {
        return false;
    }

    // cookie_domain is subdomain of page_domain (first-party)
    // e.g., cookie from "static.example.com", page is "example.com"
    if (cookie_domain.length() > page_domain.length()) {
        size_t offset = cookie_domain.length() - page_domain.length();
        if (cookie_domain[offset - 1] == '.' &&
            cookie_domain.substr(offset) == page_domain) {
            return false;
        }
    }

    // page_domain is subdomain of cookie_domain (first-party)
    // e.g., cookie domain ".example.com", page is "www.example.com"
    if (page_domain.length() > cookie_domain.length()) {
        size_t offset = page_domain.length() - cookie_domain.length();
        if (page_domain[offset - 1] == '.' &&
            page_domain.substr(offset) == cookie_domain) {
            return false;
        }
    }

    // Same-entity check: e.g. twitter.com cookies on x.com are first-party
    if (AreSameAuthEntity(cookie_domain, page_domain)) {
        return false;
    }

    return true; // Different domains = third-party
}

// ============================================================================
// Async logging (posts to FILE thread to avoid IO thread SQLite)
// ============================================================================
void CookieBlockManager::LogBlockedCookie(const std::string& domain,
                                           const std::string& page_url,
                                           const std::string& reason) {
    if (!db_) return;

    auto now = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::system_clock::now().time_since_epoch()).count();

    CefPostTask(TID_FILE_USER_BLOCKING,
                new LogBlockedCookieTask(db_, domain, page_url, reason, now));
}

// ============================================================================
// Utility methods
// ============================================================================
std::string CookieBlockManager::ExtractDomain(const std::string& url) {
    // Extract domain from URL: "https://www.example.com/path" -> "www.example.com"
    size_t protocol_end = url.find("://");
    size_t domain_start = (protocol_end != std::string::npos) ? protocol_end + 3 : 0;
    size_t domain_end = url.find('/', domain_start);
    std::string domain;
    if (domain_end != std::string::npos) {
        domain = url.substr(domain_start, domain_end - domain_start);
    } else {
        domain = url.substr(domain_start);
    }

    // Remove port if present
    size_t colon = domain.find(':');
    if (colon != std::string::npos) {
        domain = domain.substr(0, colon);
    }

    return domain;
}

std::string CookieBlockManager::NormalizeDomain(const std::string& domain) {
    // Strip leading dot (cookie domains often start with '.')
    std::string normalized = domain;
    if (!normalized.empty() && normalized[0] == '.') {
        normalized = normalized.substr(1);
    }

    // Lowercase
    std::transform(normalized.begin(), normalized.end(), normalized.begin(),
                   [](unsigned char c) { return std::tolower(c); });

    return normalized;
}
