#include "../../include/core/HistoryManager.h"
#include "../../include/core/Logger.h"
#include <iostream>
#include <sstream>
#include <fstream>
#include <algorithm>

#define LOG_DEBUG_HISTORY(msg) Logger::Log(msg, 0, 0)
#define LOG_INFO_HISTORY(msg) Logger::Log(msg, 1, 0)
#define LOG_ERROR_HISTORY(msg) Logger::Log(msg, 3, 0)

HistoryManager& HistoryManager::GetInstance() {
    static HistoryManager instance;
    return instance;
}

HistoryManager::~HistoryManager() {
    CloseDatabase();
}

bool HistoryManager::Initialize(const std::string& user_data_path) {
    history_db_path_ = user_data_path + "/HodosHistory";
    LOG_INFO_HISTORY("📚 HistoryManager initializing with OUR database: " + history_db_path_);

    // Open and create our database immediately
    if (!OpenDatabase()) {
        LOG_ERROR_HISTORY("❌ Failed to create History database");
        return false;
    }

    LOG_INFO_HISTORY("📚 HistoryManager initialization complete");
    return true;
}

bool HistoryManager::OpenDatabase() {
    if (history_db_) {
        // Already open
        return true;
    }

    LOG_INFO_HISTORY("📚 Creating/Opening our History database at: " + history_db_path_);

    // Create or open the database (CREATE flag)
    int rc = sqlite3_open_v2(history_db_path_.c_str(), &history_db_,
                            SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE, nullptr);

    if (rc != SQLITE_OK) {
        LOG_ERROR_HISTORY("❌ Failed to create/open History database: " + std::string(sqlite3_errmsg(history_db_)));
        if (history_db_) {
            sqlite3_close(history_db_);
        }
        history_db_ = nullptr;
        return false;
    }

    LOG_INFO_HISTORY("📚 History database opened, creating schema...");

    // Create Chromium-compatible schema
    const char* schema_sql = R"(
        CREATE TABLE IF NOT EXISTS urls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT NOT NULL UNIQUE,
            title TEXT,
            visit_count INTEGER DEFAULT 0,
            typed_count INTEGER DEFAULT 0,
            last_visit_time INTEGER NOT NULL,
            hidden INTEGER DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS visits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url INTEGER NOT NULL,
            visit_time INTEGER NOT NULL,
            from_visit INTEGER,
            transition INTEGER NOT NULL,
            segment_id INTEGER,
            visit_duration INTEGER DEFAULT 0,
            FOREIGN KEY (url) REFERENCES urls(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_urls_url ON urls(url);
        CREATE INDEX IF NOT EXISTS idx_urls_last_visit_time ON urls(last_visit_time);
        CREATE INDEX IF NOT EXISTS idx_visits_url ON visits(url);
        CREATE INDEX IF NOT EXISTS idx_visits_visit_time ON visits(visit_time);
    )";

    char* err_msg = nullptr;
    rc = sqlite3_exec(history_db_, schema_sql, nullptr, nullptr, &err_msg);
    if (rc != SQLITE_OK) {
        LOG_ERROR_HISTORY("❌ Failed to create schema: " + std::string(err_msg ? err_msg : "unknown"));
        if (err_msg) sqlite3_free(err_msg);
        CloseDatabase();
        return false;
    }

    LOG_INFO_HISTORY("✅ History database schema created");

    // Enable WAL mode for better concurrency
    rc = sqlite3_exec(history_db_, "PRAGMA journal_mode=WAL;", nullptr, nullptr, &err_msg);
    if (rc != SQLITE_OK) {
        LOG_INFO_HISTORY("⚠️ Warning: Could not enable WAL mode: " + std::string(err_msg ? err_msg : "unknown"));
        if (err_msg) sqlite3_free(err_msg);
    }

    // Set busy timeout to handle locks
    sqlite3_busy_timeout(history_db_, 5000);

    LOG_INFO_HISTORY("✅ History database ready");
    return true;
}

void HistoryManager::CloseDatabase() {
    if (history_db_) {
        sqlite3_close(history_db_);
        history_db_ = nullptr;
        LOG_INFO_HISTORY("📚 History database closed");
    }
}

bool HistoryManager::AddVisit(const std::string& url, const std::string& title, int transition_type) {
    if (!history_db_) {
        LOG_ERROR_HISTORY("❌ Cannot add visit - database not open");
        return false;
    }

    // Debounce: skip if we've logged this URL within DEBOUNCE_SECONDS
    {
        std::lock_guard<std::mutex> lock(recent_visits_mutex_);
        auto now_clock = std::chrono::steady_clock::now();
        auto it = recent_visits_.find(url);
        if (it != recent_visits_.end()) {
            auto elapsed = std::chrono::duration_cast<std::chrono::seconds>(now_clock - it->second).count();
            if (elapsed < DEBOUNCE_SECONDS) {
                LOG_INFO_HISTORY("📚 Skipping duplicate visit (debounced): " + url);
                return true; // Return true - not an error, just skipped
            }
        }
        // Update the timestamp for this URL
        recent_visits_[url] = now_clock;
        
        // Cleanup old entries (keep map from growing unbounded)
        if (recent_visits_.size() > 100) {
            for (auto it = recent_visits_.begin(); it != recent_visits_.end(); ) {
                auto elapsed = std::chrono::duration_cast<std::chrono::seconds>(now_clock - it->second).count();
                if (elapsed > 60) {  // Remove entries older than 60 seconds
                    it = recent_visits_.erase(it);
                } else {
                    ++it;
                }
            }
        }
    }

    int64_t now = GetCurrentChromiumTime();

    LOG_INFO_HISTORY("📚 Adding visit: " + url);

    // First, check if URL exists
    const char* check_sql = "SELECT id, visit_count FROM urls WHERE url = ?";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(history_db_, check_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        LOG_ERROR_HISTORY("❌ Failed to prepare check query: " + std::string(sqlite3_errmsg(history_db_)));
        return false;
    }

    sqlite3_bind_text(stmt, 1, url.c_str(), -1, SQLITE_STATIC);

    int64_t url_id = -1;
    int visit_count = 0;

    if (sqlite3_step(stmt) == SQLITE_ROW) {
        // URL exists
        url_id = sqlite3_column_int64(stmt, 0);
        visit_count = sqlite3_column_int(stmt, 1);
        LOG_INFO_HISTORY("📚 URL exists, updating (current visits: " + std::to_string(visit_count) + ")");
    }
    sqlite3_finalize(stmt);

    if (url_id == -1) {
        // Insert new URL
        const char* insert_url_sql = "INSERT INTO urls (url, title, visit_count, last_visit_time) VALUES (?, ?, 1, ?)";
        rc = sqlite3_prepare_v2(history_db_, insert_url_sql, -1, &stmt, nullptr);
        if (rc != SQLITE_OK) {
            LOG_ERROR_HISTORY("❌ Failed to prepare insert URL: " + std::string(sqlite3_errmsg(history_db_)));
            return false;
        }

        sqlite3_bind_text(stmt, 1, url.c_str(), -1, SQLITE_STATIC);
        sqlite3_bind_text(stmt, 2, title.c_str(), -1, SQLITE_STATIC);
        sqlite3_bind_int64(stmt, 3, now);

        rc = sqlite3_step(stmt);
        sqlite3_finalize(stmt);

        if (rc != SQLITE_DONE) {
            LOG_ERROR_HISTORY("❌ Failed to insert URL: " + std::string(sqlite3_errmsg(history_db_)));
            return false;
        }

        url_id = sqlite3_last_insert_rowid(history_db_);
        LOG_INFO_HISTORY("✅ New URL inserted with ID: " + std::to_string(url_id));
    } else {
        // Update existing URL
        const char* update_url_sql = "UPDATE urls SET visit_count = visit_count + 1, last_visit_time = ?, title = ? WHERE id = ?";
        rc = sqlite3_prepare_v2(history_db_, update_url_sql, -1, &stmt, nullptr);
        if (rc != SQLITE_OK) {
            LOG_ERROR_HISTORY("❌ Failed to prepare update URL: " + std::string(sqlite3_errmsg(history_db_)));
            return false;
        }

        sqlite3_bind_int64(stmt, 1, now);
        sqlite3_bind_text(stmt, 2, title.c_str(), -1, SQLITE_STATIC);
        sqlite3_bind_int64(stmt, 3, url_id);

        rc = sqlite3_step(stmt);
        sqlite3_finalize(stmt);

        if (rc != SQLITE_DONE) {
            LOG_ERROR_HISTORY("❌ Failed to update URL: " + std::string(sqlite3_errmsg(history_db_)));
            return false;
        }

        LOG_INFO_HISTORY("✅ URL updated, new visit count: " + std::to_string(visit_count + 1));
    }

    // Insert visit record
    const char* insert_visit_sql = "INSERT INTO visits (url, visit_time, transition) VALUES (?, ?, ?)";
    rc = sqlite3_prepare_v2(history_db_, insert_visit_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        LOG_ERROR_HISTORY("❌ Failed to prepare insert visit: " + std::string(sqlite3_errmsg(history_db_)));
        return false;
    }

    sqlite3_bind_int64(stmt, 1, url_id);
    sqlite3_bind_int64(stmt, 2, now);
    sqlite3_bind_int(stmt, 3, transition_type);

    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE) {
        LOG_ERROR_HISTORY("❌ Failed to insert visit: " + std::string(sqlite3_errmsg(history_db_)));
        return false;
    }

    LOG_INFO_HISTORY("✅ Visit recorded successfully");
    return true;
}

std::vector<HistoryEntry> HistoryManager::GetHistory(int limit, int offset) {
    std::vector<HistoryEntry> entries;

    // Try to open database if not already open
    if (!history_db_ && !OpenDatabase()) {
        LOG_INFO_HISTORY("⚠️ History database not available yet");
        return entries;
    }

    if (!history_db_) {
        // Database still not open (doesn't exist yet)
        LOG_INFO_HISTORY("⚠️ History database still not available");
        return entries;
    }

    // Return unique URLs only (not every individual visit)
    // Uses MAX(visit_time) to get the most recent visit for each URL
    const char* sql = R"(
        SELECT u.id, u.url, u.title, u.visit_count, u.last_visit_time, 
               u.last_visit_time as visit_time, 0 as transition
        FROM urls u
        WHERE u.hidden = 0
        ORDER BY u.last_visit_time DESC
        LIMIT ? OFFSET ?
    )";

    LOG_INFO_HISTORY("📚 Preparing SQL query for GetHistory");

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(history_db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) {
        LOG_ERROR_HISTORY("❌ Failed to prepare history query: " + std::string(sqlite3_errmsg(history_db_)));
        return entries;
    }

    sqlite3_bind_int(stmt, 1, limit);
    sqlite3_bind_int(stmt, 2, offset);

    LOG_INFO_HISTORY("📚 Executing query with limit=" + std::to_string(limit) + ", offset=" + std::to_string(offset));

    int row_count = 0;
    while (sqlite3_step(stmt) == SQLITE_ROW) {
        row_count++;
        HistoryEntry entry;
        entry.id = sqlite3_column_int64(stmt, 0);

        const unsigned char* url_text = sqlite3_column_text(stmt, 1);
        entry.url = url_text ? reinterpret_cast<const char*>(url_text) : "";

        const unsigned char* title_text = sqlite3_column_text(stmt, 2);
        entry.title = title_text ? reinterpret_cast<const char*>(title_text) : "";

        entry.visit_count = sqlite3_column_int(stmt, 3);
        entry.last_visit_time = sqlite3_column_int64(stmt, 4);
        entry.visit_time = sqlite3_column_int64(stmt, 5);
        entry.transition = sqlite3_column_int(stmt, 6);

        entries.push_back(entry);
    }

    LOG_INFO_HISTORY("📚 Query returned " + std::to_string(row_count) + " rows");

    sqlite3_finalize(stmt);

    LOG_INFO_HISTORY("📚 Retrieved " + std::to_string(entries.size()) + " history entries");
    return entries;
}

std::vector<HistoryEntry> HistoryManager::SearchHistory(const HistorySearchParams& params) {
    std::vector<HistoryEntry> entries;

    // Try to open database if not already open
    if (!history_db_ && !OpenDatabase()) {
        std::cerr << "⚠️ History database not available yet" << std::endl;
        return entries;
    }

    if (!history_db_) {
        return entries;
    }

    // Return unique URLs only (not every individual visit)
    std::stringstream sql;
    sql << "SELECT u.id, u.url, u.title, u.visit_count, u.last_visit_time, "
        << "u.last_visit_time as visit_time, 0 as transition "
        << "FROM urls u "
        << "WHERE u.hidden = 0";

    bool has_search = !params.search_term.empty();
    bool has_start = params.start_time > 0;
    bool has_end = params.end_time > 0;

    if (has_search) {
        sql << " AND (u.url LIKE ? OR u.title LIKE ?)";
    }

    if (has_start) {
        sql << " AND u.last_visit_time >= ?";
    }

    if (has_end) {
        sql << " AND u.last_visit_time <= ?";
    }

    sql << " ORDER BY u.last_visit_time DESC LIMIT ? OFFSET ?";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(history_db_, sql.str().c_str(), -1, &stmt, nullptr);

    if (rc != SQLITE_OK) {
        std::cerr << "❌ Failed to prepare search query: " << sqlite3_errmsg(history_db_) << std::endl;
        return entries;
    }

    int param_index = 1;

    if (has_search) {
        std::string pattern = "%" + params.search_term + "%";
        sqlite3_bind_text(stmt, param_index++, pattern.c_str(), -1, SQLITE_TRANSIENT);
        sqlite3_bind_text(stmt, param_index++, pattern.c_str(), -1, SQLITE_TRANSIENT);
    }

    if (has_start) {
        sqlite3_bind_int64(stmt, param_index++, params.start_time);
    }

    if (has_end) {
        sqlite3_bind_int64(stmt, param_index++, params.end_time);
    }

    sqlite3_bind_int(stmt, param_index++, params.limit);
    sqlite3_bind_int(stmt, param_index++, params.offset);

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        HistoryEntry entry;
        entry.id = sqlite3_column_int64(stmt, 0);

        const unsigned char* url_text = sqlite3_column_text(stmt, 1);
        entry.url = url_text ? reinterpret_cast<const char*>(url_text) : "";

        const unsigned char* title_text = sqlite3_column_text(stmt, 2);
        entry.title = title_text ? reinterpret_cast<const char*>(title_text) : "";

        entry.visit_count = sqlite3_column_int(stmt, 3);
        entry.last_visit_time = sqlite3_column_int64(stmt, 4);
        entry.visit_time = sqlite3_column_int64(stmt, 5);
        entry.transition = sqlite3_column_int(stmt, 6);

        entries.push_back(entry);
    }

    sqlite3_finalize(stmt);

    std::cout << "🔍 Search returned " << entries.size() << " entries" << std::endl;
    return entries;
}

HistoryEntry HistoryManager::GetHistoryEntryByUrl(const std::string& url) {
    HistoryEntry entry;
    entry.id = -1; // Invalid ID

    if (!history_db_) {
        return entry;
    }

    const char* sql = R"(
        SELECT u.id, u.url, u.title, u.visit_count, u.last_visit_time
        FROM urls u
        WHERE u.url = ?
        LIMIT 1
    )";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(history_db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) {
        return entry;
    }

    sqlite3_bind_text(stmt, 1, url.c_str(), -1, SQLITE_STATIC);

    if (sqlite3_step(stmt) == SQLITE_ROW) {
        entry.id = sqlite3_column_int64(stmt, 0);

        const unsigned char* url_text = sqlite3_column_text(stmt, 1);
        entry.url = url_text ? reinterpret_cast<const char*>(url_text) : "";

        const unsigned char* title_text = sqlite3_column_text(stmt, 2);
        entry.title = title_text ? reinterpret_cast<const char*>(title_text) : "";

        entry.visit_count = sqlite3_column_int(stmt, 3);
        entry.last_visit_time = sqlite3_column_int64(stmt, 4);
        entry.visit_time = entry.last_visit_time; // Use last visit as visit time
        entry.transition = 0;
    }

    sqlite3_finalize(stmt);
    return entry;
}

std::vector<HistoryEntry> HistoryManager::GetTopSites(int limit) {
    std::vector<HistoryEntry> entries;

    if (!history_db_ && !OpenDatabase()) {
        LOG_INFO_HISTORY("⚠️ History database not available for GetTopSites");
        return entries;
    }

    if (!history_db_) {
        return entries;
    }

    const char* sql = R"(
        SELECT id, url, title, visit_count, last_visit_time, 0, 0
        FROM urls WHERE hidden = 0 AND visit_count > 0
        ORDER BY visit_count DESC, last_visit_time DESC LIMIT ?
    )";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(history_db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) {
        LOG_ERROR_HISTORY("❌ Failed to prepare GetTopSites query: " + std::string(sqlite3_errmsg(history_db_)));
        return entries;
    }

    sqlite3_bind_int(stmt, 1, limit);

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        HistoryEntry entry;
        entry.id = sqlite3_column_int64(stmt, 0);

        const unsigned char* url_text = sqlite3_column_text(stmt, 1);
        entry.url = url_text ? reinterpret_cast<const char*>(url_text) : "";

        const unsigned char* title_text = sqlite3_column_text(stmt, 2);
        entry.title = title_text ? reinterpret_cast<const char*>(title_text) : "";

        entry.visit_count = sqlite3_column_int(stmt, 3);
        entry.last_visit_time = sqlite3_column_int64(stmt, 4);
        entry.visit_time = entry.last_visit_time;
        entry.transition = 0;

        entries.push_back(entry);
    }

    sqlite3_finalize(stmt);

    LOG_INFO_HISTORY("📊 GetTopSites returned " + std::to_string(entries.size()) + " entries");
    return entries;
}

bool HistoryManager::DeleteHistoryEntry(const std::string& url) {
    // Try to open database if not already open
    if (!history_db_ && !OpenDatabase()) {
        std::cerr << "⚠️ History database not available yet" << std::endl;
        return false;
    }

    if (!history_db_) {
        return false;
    }

    // First get the url_id
    const char* get_id_sql = "SELECT id FROM urls WHERE url = ?";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(history_db_, get_id_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        std::cerr << "❌ Failed to query URL ID: " << sqlite3_errmsg(history_db_) << std::endl;
        return false;
    }

    sqlite3_bind_text(stmt, 1, url.c_str(), -1, SQLITE_STATIC);

    int64_t url_id = -1;
    if (sqlite3_step(stmt) == SQLITE_ROW) {
        url_id = sqlite3_column_int64(stmt, 0);
    }
    sqlite3_finalize(stmt);

    if (url_id < 0) {
        std::cout << "⚠️ URL not found in history: " << url << std::endl;
        return false;
    }

    // Delete visits for this URL
    const char* delete_visits_sql = "DELETE FROM visits WHERE url = ?";
    rc = sqlite3_prepare_v2(history_db_, delete_visits_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        std::cerr << "❌ Failed to delete visits: " << sqlite3_errmsg(history_db_) << std::endl;
        return false;
    }

    sqlite3_bind_int64(stmt, 1, url_id);
    sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    // Delete the URL entry
    const char* delete_url_sql = "DELETE FROM urls WHERE id = ?";
    rc = sqlite3_prepare_v2(history_db_, delete_url_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        std::cerr << "❌ Failed to delete URL: " << sqlite3_errmsg(history_db_) << std::endl;
        return false;
    }

    sqlite3_bind_int64(stmt, 1, url_id);
    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    if (rc == SQLITE_DONE) {
        std::cout << "✅ Deleted history entry: " << url << std::endl;
        return true;
    }

    return false;
}

bool HistoryManager::DeleteAllHistory() {
    // Try to open database if not already open
    if (!history_db_ && !OpenDatabase()) {
        std::cerr << "⚠️ History database not available yet" << std::endl;
        return false;
    }

    if (!history_db_) {
        return false;
    }

    const char* delete_sql = R"(
        DELETE FROM visits;
        DELETE FROM urls;
        DELETE FROM keyword_search_terms;
    )";

    char* err_msg = nullptr;
    int rc = sqlite3_exec(history_db_, delete_sql, nullptr, nullptr, &err_msg);

    if (rc != SQLITE_OK) {
        std::cerr << "❌ Failed to clear history: " << err_msg << std::endl;
        sqlite3_free(err_msg);
        return false;
    }

    std::cout << "✅ All history cleared" << std::endl;
    return true;
}

bool HistoryManager::DeleteHistoryRange(int64_t start_time, int64_t end_time) {
    // Try to open database if not already open
    if (!history_db_ && !OpenDatabase()) {
        std::cerr << "⚠️ History database not available yet" << std::endl;
        return false;
    }

    if (!history_db_) {
        return false;
    }

    // Delete visits in range
    const char* delete_visits_sql = "DELETE FROM visits WHERE visit_time >= ? AND visit_time <= ?";
    sqlite3_stmt* stmt;

    int rc = sqlite3_prepare_v2(history_db_, delete_visits_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        std::cerr << "❌ Failed to prepare delete range query: " << sqlite3_errmsg(history_db_) << std::endl;
        return false;
    }

    sqlite3_bind_int64(stmt, 1, start_time);
    sqlite3_bind_int64(stmt, 2, end_time);
    sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    // Clean up orphaned URLs (URLs with no visits)
    const char* cleanup_sql = "DELETE FROM urls WHERE id NOT IN (SELECT DISTINCT url FROM visits)";
    rc = sqlite3_prepare_v2(history_db_, cleanup_sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        std::cerr << "❌ Failed to clean up orphaned URLs: " << sqlite3_errmsg(history_db_) << std::endl;
        return false;
    }

    sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    std::cout << "✅ History range cleared" << std::endl;
    return true;
}

int64_t HistoryManager::GetCurrentChromiumTime() {
    // Chromium time: microseconds since January 1, 1601 UTC
    auto now = std::chrono::system_clock::now();
    auto unix_time = std::chrono::duration_cast<std::chrono::microseconds>(
        now.time_since_epoch()
    ).count();

    // Convert Unix epoch (1970) to Windows epoch (1601)
    // 11644473600 seconds = difference between epochs
    return unix_time + (11644473600LL * 1000000LL);
}

int64_t HistoryManager::ChromiumTimeToUnix(int64_t chromium_time) {
    // Convert Chromium microseconds to Unix seconds
    return (chromium_time / 1000000) - 11644473600LL;
}

int64_t HistoryManager::UnixToChromiumTime(int64_t unix_time) {
    // Convert Unix seconds to Chromium microseconds
    return (unix_time + 11644473600LL) * 1000000LL;
}

std::vector<HistoryEntry> HistoryManager::GetHistorySimple(int limit) {
    std::vector<HistoryEntry> entries;

    // Try to open database if not already open
    if (!history_db_ && !OpenDatabase()) {
        LOG_INFO_HISTORY("⚠️ History database not available for simple query");
        return entries;
    }

    if (!history_db_) {
        return entries;
    }

    // Simple query - just get URLs without JOIN to diagnose
    const char* sql = "SELECT id, url, title, visit_count, last_visit_time FROM urls LIMIT ?";

    LOG_INFO_HISTORY("📚 Running simple test query");

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(history_db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) {
        LOG_ERROR_HISTORY("❌ Failed to prepare simple query: " + std::string(sqlite3_errmsg(history_db_)));
        return entries;
    }

    sqlite3_bind_int(stmt, 1, limit);

    int row_count = 0;
    while (sqlite3_step(stmt) == SQLITE_ROW) {
        row_count++;
        HistoryEntry entry;
        entry.id = sqlite3_column_int64(stmt, 0);

        const unsigned char* url_text = sqlite3_column_text(stmt, 1);
        entry.url = url_text ? reinterpret_cast<const char*>(url_text) : "";

        const unsigned char* title_text = sqlite3_column_text(stmt, 2);
        entry.title = title_text ? reinterpret_cast<const char*>(title_text) : "";

        entry.visit_count = sqlite3_column_int(stmt, 3);
        entry.last_visit_time = sqlite3_column_int64(stmt, 4);
        entry.visit_time = entry.last_visit_time;
        entry.transition = 0;

        entries.push_back(entry);

        LOG_INFO_HISTORY("📚 Found URL: " + entry.url + " (visits: " + std::to_string(entry.visit_count) + ")");
    }

    LOG_INFO_HISTORY("📚 Simple query returned " + std::to_string(row_count) + " rows");

    sqlite3_finalize(stmt);
    return entries;
}

std::string HistoryManager::extractDomain(const std::string& url) {
    // Extract domain from URL (between :// and first /)
    // Returns lowercase domain

    size_t protocol_pos = url.find("://");
    if (protocol_pos == std::string::npos) {
        return "";
    }

    size_t domain_start = protocol_pos + 3;
    size_t domain_end = url.find("/", domain_start);

    std::string domain;
    if (domain_end == std::string::npos) {
        domain = url.substr(domain_start);
    } else {
        domain = url.substr(domain_start, domain_end - domain_start);
    }

    // Convert to lowercase
    for (char& c : domain) {
        c = std::tolower(static_cast<unsigned char>(c));
    }

    return domain;
}

std::vector<HistoryEntryWithScore> HistoryManager::SearchHistoryWithFrecency(const std::string& query, int limit) {
    std::vector<HistoryEntryWithScore> entries;

    // Try to open database if not already open
    if (!history_db_ && !OpenDatabase()) {
        LOG_INFO_HISTORY("⚠️ History database not available for frecency search");
        return entries;
    }

    if (!history_db_) {
        return entries;
    }

    LOG_INFO_HISTORY("🔍 SearchHistoryWithFrecency called with query: " + query);

    // SQL query with frecency scoring
    const char* sql = R"(
        WITH norm AS (
            SELECT MAX(visit_count) AS max_visits, MAX(last_visit_time) AS max_time
            FROM urls WHERE hidden = 0
        )
        SELECT u.id, u.url, u.title, u.visit_count, u.last_visit_time,
            CASE
                WHEN norm.max_visits > 0 AND norm.max_time > 0 THEN
                    ((CAST(u.visit_count AS REAL) / norm.max_visits) * 0.5 +
                     (CAST(u.last_visit_time AS REAL) / norm.max_time) * 0.5)
                ELSE 0.0
            END AS frecency_score
        FROM urls u, norm
        WHERE u.hidden = 0
          AND (LOWER(u.url) LIKE LOWER(?) OR LOWER(u.title) LIKE LOWER(?))
        ORDER BY frecency_score DESC
        LIMIT ?
    )";

    sqlite3_stmt* stmt;
    int rc = sqlite3_prepare_v2(history_db_, sql, -1, &stmt, nullptr);

    if (rc != SQLITE_OK) {
        LOG_ERROR_HISTORY("❌ Failed to prepare frecency query: " + std::string(sqlite3_errmsg(history_db_)));
        return entries;
    }

    // Bind parameters with wildcards for LIKE matching
    std::string pattern = "%" + query + "%";
    sqlite3_bind_text(stmt, 1, pattern.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_text(stmt, 2, pattern.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_int(stmt, 3, limit);

    LOG_INFO_HISTORY("📚 Executing frecency query with pattern: " + pattern);

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        HistoryEntryWithScore entry_with_score;
        entry_with_score.entry.id = sqlite3_column_int64(stmt, 0);

        const unsigned char* url_text = sqlite3_column_text(stmt, 1);
        entry_with_score.entry.url = url_text ? reinterpret_cast<const char*>(url_text) : "";

        const unsigned char* title_text = sqlite3_column_text(stmt, 2);
        entry_with_score.entry.title = title_text ? reinterpret_cast<const char*>(title_text) : "";

        entry_with_score.entry.visit_count = sqlite3_column_int(stmt, 3);
        entry_with_score.entry.last_visit_time = sqlite3_column_int64(stmt, 4);
        entry_with_score.entry.visit_time = entry_with_score.entry.last_visit_time;
        entry_with_score.entry.transition = 0;

        entry_with_score.frecency_score = sqlite3_column_double(stmt, 5);

        entries.push_back(entry_with_score);
    }

    sqlite3_finalize(stmt);

    LOG_INFO_HISTORY("📚 Frecency query returned " + std::to_string(entries.size()) + " entries before domain boost");

    // Apply 1.5x domain boost post-query
    std::string query_lower = query;
    for (char& c : query_lower) {
        c = std::tolower(static_cast<unsigned char>(c));
    }

    for (auto& entry : entries) {
        std::string domain = extractDomain(entry.entry.url);

        // Check if query matches domain (domain starts with query or equals query)
        if (!domain.empty() && (domain == query_lower || domain.find(query_lower) == 0)) {
            entry.frecency_score *= 1.5;
            LOG_INFO_HISTORY("📈 Domain boost applied to: " + entry.entry.url + " (new score: " + std::to_string(entry.frecency_score) + ")");
        }
    }

    // Re-sort by final score descending
    std::sort(entries.begin(), entries.end(),
        [](const HistoryEntryWithScore& a, const HistoryEntryWithScore& b) {
            return a.frecency_score > b.frecency_score;
        });

    LOG_INFO_HISTORY("✅ SearchHistoryWithFrecency returning " + std::to_string(entries.size()) + " entries");

    return entries;
}
