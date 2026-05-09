#include "../../include/core/PaidContentCache.h"

#include <nlohmann/json.hpp>

#include <algorithm>
#include <cctype>
#include <chrono>
#include <cstring>
#include <sstream>

#include "../../include/core/Logger.h"

// Logging — module ID 12 = PaidContentCache (12 unused; pick a free slot).
#define LOG_INFO_PCC(msg) Logger::Log(msg, 1, 12)
#define LOG_WARNING_PCC(msg) Logger::Log(msg, 2, 12)
#define LOG_ERROR_PCC(msg) Logger::Log(msg, 3, 12)
#define LOG_DEBUG_PCC(msg) Logger::Log(msg, 0, 12)

namespace {

int64_t NowMs() {
    return std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::system_clock::now().time_since_epoch())
        .count();
}

std::string ToLower(std::string s) {
    std::transform(s.begin(), s.end(), s.begin(),
                   [](unsigned char c) { return std::tolower(c); });
    return s;
}

// Serialize a HeaderMap as a JSON object {lowercase_name: "value"}.
// HeaderMap is a multimap; concatenate same-name values with ", " (per RFC 7230).
std::string HeadersToJson(const CefResponse::HeaderMap& headers) {
    nlohmann::json j = nlohmann::json::object();
    for (const auto& [name, value] : headers) {
        std::string key = ToLower(name.ToString());
        std::string val = value.ToString();
        if (j.contains(key)) {
            j[key] = j[key].get<std::string>() + ", " + val;
        } else {
            j[key] = val;
        }
    }
    return j.dump();
}

std::map<std::string, std::string> HeadersFromJson(const std::string& json_text) {
    std::map<std::string, std::string> out;
    try {
        auto j = nlohmann::json::parse(json_text);
        for (auto it = j.begin(); it != j.end(); ++it) {
            if (it.value().is_string()) {
                out[it.key()] = it.value().get<std::string>();
            }
        }
    } catch (const std::exception& e) {
        LOG_WARNING_PCC(std::string("HeadersFromJson failed: ") + e.what());
    }
    return out;
}

}  // namespace

PaidContentCache& PaidContentCache::GetInstance() {
    static PaidContentCache instance;
    return instance;
}

PaidContentCache::~PaidContentCache() {
    CloseDatabase();
}

bool PaidContentCache::Initialize(const std::string& user_data_path) {
    std::lock_guard<std::mutex> lock(mutex_);
    if (db_ != nullptr) {
        return true;  // Already open.
    }

#ifdef _WIN32
    db_path_ = user_data_path + "\\paid_content_cache.db";
#else
    db_path_ = user_data_path + "/paid_content_cache.db";
#endif

    LOG_INFO_PCC("Initializing PaidContentCache at: " + db_path_);

    if (!OpenDatabase()) {
        LOG_ERROR_PCC("Failed to open PaidContentCache database");
        return false;
    }
    if (!EnsureSchema()) {
        LOG_ERROR_PCC("Failed to ensure PaidContentCache schema");
        CloseDatabase();
        return false;
    }
    LOG_INFO_PCC("PaidContentCache initialized successfully");
    return true;
}

bool PaidContentCache::OpenDatabase() {
    int rc = sqlite3_open_v2(db_path_.c_str(), &db_,
                             SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE,
                             nullptr);
    if (rc != SQLITE_OK) {
        LOG_ERROR_PCC(std::string("sqlite3_open_v2 failed: ") +
                      sqlite3_errmsg(db_));
        db_ = nullptr;
        return false;
    }
    sqlite3_exec(db_, "PRAGMA journal_mode = WAL;", nullptr, nullptr, nullptr);
    sqlite3_busy_timeout(db_, 5000);
    return true;
}

void PaidContentCache::CloseDatabase() {
    if (db_) {
        sqlite3_close(db_);
        db_ = nullptr;
    }
}

bool PaidContentCache::EnsureSchema() {
    const char* schema = R"SQL(
        CREATE TABLE IF NOT EXISTS paid_content (
            url TEXT PRIMARY KEY,
            status INTEGER NOT NULL,
            response_headers TEXT NOT NULL,
            response_body BLOB NOT NULL,
            byte_size INTEGER NOT NULL,
            paid_at INTEGER NOT NULL,
            last_access INTEGER NOT NULL,
            expires_at INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_last_access ON paid_content(last_access);
    )SQL";
    char* err = nullptr;
    int rc = sqlite3_exec(db_, schema, nullptr, nullptr, &err);
    if (rc != SQLITE_OK) {
        LOG_ERROR_PCC(std::string("schema exec failed: ") + (err ? err : "?"));
        if (err) sqlite3_free(err);
        return false;
    }
    return true;
}

bool PaidContentCache::Get(const std::string& url, PaidContentEntry& out) {
    if (!enabled_.load()) return false;

    std::lock_guard<std::mutex> lock(mutex_);
    if (!db_) return false;

    const char* sql =
        "SELECT status, response_headers, response_body, byte_size, "
        "paid_at, last_access, expires_at FROM paid_content WHERE url = ?";

    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
        LOG_WARNING_PCC(std::string("Get prepare failed: ") +
                        sqlite3_errmsg(db_));
        return false;
    }
    sqlite3_bind_text(stmt, 1, url.c_str(), -1, SQLITE_TRANSIENT);

    bool found = false;
    if (sqlite3_step(stmt) == SQLITE_ROW) {
        int status = sqlite3_column_int(stmt, 0);
        const char* headers_text =
            reinterpret_cast<const char*>(sqlite3_column_text(stmt, 1));
        const void* body_blob = sqlite3_column_blob(stmt, 2);
        int body_size = sqlite3_column_bytes(stmt, 2);
        int64_t paid_at = sqlite3_column_int64(stmt, 4);
        int64_t last_access = sqlite3_column_int64(stmt, 5);
        bool has_expiry = sqlite3_column_type(stmt, 6) != SQLITE_NULL;
        int64_t expires_at = has_expiry ? sqlite3_column_int64(stmt, 6) : 0;

        int64_t now = NowMs();
        if (has_expiry && now >= expires_at) {
            // Expired — fall through, will delete below.
            sqlite3_finalize(stmt);

            sqlite3_stmt* del = nullptr;
            const char* del_sql = "DELETE FROM paid_content WHERE url = ?";
            if (sqlite3_prepare_v2(db_, del_sql, -1, &del, nullptr) ==
                SQLITE_OK) {
                sqlite3_bind_text(del, 1, url.c_str(), -1, SQLITE_TRANSIENT);
                sqlite3_step(del);
                sqlite3_finalize(del);
            }
            return false;
        }

        out.url = url;
        out.status = status;
        out.headers = HeadersFromJson(headers_text ? headers_text : "{}");
        out.body.assign(static_cast<const uint8_t*>(body_blob),
                        static_cast<const uint8_t*>(body_blob) + body_size);
        out.paid_at_ms = paid_at;
        out.last_access_ms = last_access;
        out.expires_at_ms =
            has_expiry ? std::optional<int64_t>(expires_at) : std::nullopt;
        found = true;
    }
    sqlite3_finalize(stmt);

    if (found) {
        // Update last_access for LRU.
        sqlite3_stmt* upd = nullptr;
        const char* upd_sql =
            "UPDATE paid_content SET last_access = ? WHERE url = ?";
        if (sqlite3_prepare_v2(db_, upd_sql, -1, &upd, nullptr) == SQLITE_OK) {
            sqlite3_bind_int64(upd, 1, NowMs());
            sqlite3_bind_text(upd, 2, url.c_str(), -1, SQLITE_TRANSIENT);
            sqlite3_step(upd);
            sqlite3_finalize(upd);
        }
        LOG_DEBUG_PCC("PaidContentCache HIT: " + url +
                      " (" + std::to_string(out.body.size()) + " bytes)");
    }
    return found;
}

void PaidContentCache::Put(const std::string& url,
                            int status,
                            const CefResponse::HeaderMap& headers,
                            const std::vector<uint8_t>& body,
                            std::optional<int64_t> expires_at_ms) {
    if (!enabled_.load()) return;

    try {
        std::lock_guard<std::mutex> lock(mutex_);
        if (!db_) return;

        std::string headers_json = HeadersToJson(headers);
        int64_t now = NowMs();
        int64_t byte_size = static_cast<int64_t>(body.size());

        const char* sql =
            "INSERT OR REPLACE INTO paid_content "
            "(url, status, response_headers, response_body, byte_size, "
            " paid_at, last_access, expires_at) "
            "VALUES (?, ?, ?, ?, ?, ?, ?, ?)";

        sqlite3_stmt* stmt = nullptr;
        if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
            LOG_WARNING_PCC(std::string("Put prepare failed: ") +
                            sqlite3_errmsg(db_));
            return;
        }
        sqlite3_bind_text(stmt, 1, url.c_str(), -1, SQLITE_TRANSIENT);
        sqlite3_bind_int(stmt, 2, status);
        sqlite3_bind_text(stmt, 3, headers_json.c_str(), -1, SQLITE_TRANSIENT);
        if (body.empty()) {
            sqlite3_bind_zeroblob(stmt, 4, 0);
        } else {
            sqlite3_bind_blob(stmt, 4, body.data(),
                              static_cast<int>(body.size()), SQLITE_TRANSIENT);
        }
        sqlite3_bind_int64(stmt, 5, byte_size);
        sqlite3_bind_int64(stmt, 6, now);
        sqlite3_bind_int64(stmt, 7, now);
        if (expires_at_ms.has_value()) {
            sqlite3_bind_int64(stmt, 8, *expires_at_ms);
        } else {
            sqlite3_bind_null(stmt, 8);
        }
        int rc = sqlite3_step(stmt);
        sqlite3_finalize(stmt);
        if (rc != SQLITE_DONE) {
            LOG_WARNING_PCC(std::string("Put step failed: ") +
                            sqlite3_errmsg(db_));
            return;
        }

        LOG_INFO_PCC("PaidContentCache PUT: " + url +
                     " (" + std::to_string(byte_size) + " bytes)" +
                     (expires_at_ms.has_value()
                          ? " expires=" + std::to_string(*expires_at_ms)
                          : " (no expiry)"));

        EvictIfOverCap();
    } catch (const std::exception& e) {
        LOG_WARNING_PCC(std::string("Put threw: ") + e.what());
    } catch (...) {
        LOG_WARNING_PCC("Put threw unknown exception");
    }
}

void PaidContentCache::Clear() {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!db_) return;
    char* err = nullptr;
    int rc = sqlite3_exec(db_, "DELETE FROM paid_content;", nullptr, nullptr,
                          &err);
    if (rc != SQLITE_OK) {
        LOG_WARNING_PCC(std::string("Clear failed: ") + (err ? err : "?"));
        if (err) sqlite3_free(err);
    } else {
        LOG_INFO_PCC("PaidContentCache cleared");
    }
}

int64_t PaidContentCache::GetTotalSize() {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!db_) return 0;

    sqlite3_stmt* stmt = nullptr;
    const char* sql = "SELECT COALESCE(SUM(byte_size), 0) FROM paid_content";
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
        return 0;
    }
    int64_t total = 0;
    if (sqlite3_step(stmt) == SQLITE_ROW) {
        total = sqlite3_column_int64(stmt, 0);
    }
    sqlite3_finalize(stmt);
    return total;
}

void PaidContentCache::EvictIfOverCap() {
    // Caller holds mutex_ and db_ is non-null.
    sqlite3_stmt* stmt = nullptr;
    const char* size_sql =
        "SELECT COALESCE(SUM(byte_size), 0) FROM paid_content";
    if (sqlite3_prepare_v2(db_, size_sql, -1, &stmt, nullptr) != SQLITE_OK) {
        return;
    }
    int64_t total = 0;
    if (sqlite3_step(stmt) == SQLITE_ROW) {
        total = sqlite3_column_int64(stmt, 0);
    }
    sqlite3_finalize(stmt);

    if (total <= TOTAL_SIZE_LIMIT_BYTES) return;

    LOG_INFO_PCC("Cache over cap: " + std::to_string(total) + " bytes; evicting");

    // Evict oldest-by-last_access until under cap.
    while (total > TOTAL_SIZE_LIMIT_BYTES) {
        sqlite3_stmt* sel = nullptr;
        const char* sel_sql =
            "SELECT url, byte_size FROM paid_content "
            "ORDER BY last_access ASC LIMIT 1";
        if (sqlite3_prepare_v2(db_, sel_sql, -1, &sel, nullptr) != SQLITE_OK) {
            return;
        }
        if (sqlite3_step(sel) != SQLITE_ROW) {
            sqlite3_finalize(sel);
            return;  // Empty table (shouldn't happen; defensive).
        }
        const char* url_text =
            reinterpret_cast<const char*>(sqlite3_column_text(sel, 0));
        std::string victim = url_text ? url_text : "";
        int64_t victim_bytes = sqlite3_column_int64(sel, 1);
        sqlite3_finalize(sel);

        if (victim.empty()) return;

        sqlite3_stmt* del = nullptr;
        const char* del_sql = "DELETE FROM paid_content WHERE url = ?";
        if (sqlite3_prepare_v2(db_, del_sql, -1, &del, nullptr) == SQLITE_OK) {
            sqlite3_bind_text(del, 1, victim.c_str(), -1, SQLITE_TRANSIENT);
            sqlite3_step(del);
            sqlite3_finalize(del);
            total -= victim_bytes;
            LOG_DEBUG_PCC("Evicted " + victim + " (" +
                          std::to_string(victim_bytes) + " bytes)");
        } else {
            return;
        }
    }
}

// static
std::optional<int64_t> PaidContentCache::ParseCacheControl(
    const CefResponse::HeaderMap& headers) {
    for (const auto& [name, value] : headers) {
        if (ToLower(name.ToString()) != "cache-control") continue;
        std::string val = value.ToString();
        // Look for max-age=N (case-insensitive).
        std::string lower = ToLower(val);
        const std::string token = "max-age=";
        size_t pos = lower.find(token);
        if (pos == std::string::npos) return std::nullopt;
        pos += token.size();
        size_t end = pos;
        while (end < val.size() &&
               (std::isdigit(static_cast<unsigned char>(val[end])))) {
            ++end;
        }
        if (end == pos) return std::nullopt;
        try {
            int64_t seconds = std::stoll(val.substr(pos, end - pos));
            if (seconds <= 0) return std::nullopt;
            return NowMs() + seconds * 1000;
        } catch (...) {
            return std::nullopt;
        }
    }
    return std::nullopt;
}
