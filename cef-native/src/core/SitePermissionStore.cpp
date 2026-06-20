#include "../../include/core/SitePermissionStore.h"
#include "../../include/core/Logger.h"

#include <nlohmann/json.hpp>
#include <algorithm>
#include <cctype>
#include <chrono>

// Logging macros (source 2 = BROWSER)
#define LOG_INFO_SP(msg) Logger::Log(msg, 1, 2)
#define LOG_ERROR_SP(msg) Logger::Log(msg, 3, 2)

static int64_t NowMs() {
    return std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::system_clock::now().time_since_epoch()).count();
}

SitePermissionStore& SitePermissionStore::GetInstance() {
    static SitePermissionStore instance;
    return instance;
}

SitePermissionStore::~SitePermissionStore() {
    CloseDatabase();
}

// Strip scheme + path + port, lowercase. Per-host keying (Chrome is per-origin;
// host-level is a deliberate v1 simplification — documented in the design doc).
std::string SitePermissionStore::NormalizeHost(const std::string& origin) {
    std::string s = origin;
    // scheme://
    auto schemePos = s.find("://");
    if (schemePos != std::string::npos) s = s.substr(schemePos + 3);
    // path / query / fragment
    auto slash = s.find_first_of("/?#");
    if (slash != std::string::npos) s = s.substr(0, slash);
    // userinfo@
    auto at = s.find('@');
    if (at != std::string::npos) s = s.substr(at + 1);
    // host[:port]. IPv6 literals ("[::1]:443") carry colons INSIDE the brackets,
    // so a naive first-colon strip would collapse every IPv6 origin to "[" and
    // bleed one address's decision onto all others — keep up to the closing ']'.
    if (!s.empty() && s.front() == '[') {
        auto close = s.find(']');
        if (close != std::string::npos) s = s.substr(0, close + 1);  // keep "[::1]"
    } else {
        auto colon = s.find(':');
        if (colon != std::string::npos) s = s.substr(0, colon);
    }
    // Strip a single trailing FQDN dot so "example.com." keys as "example.com".
    if (!s.empty() && s.back() == '.') s.pop_back();
    std::transform(s.begin(), s.end(), s.begin(),
                   [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return s;
}

bool SitePermissionStore::Initialize(const std::string& user_data_path) {
#ifdef _WIN32
    db_path_ = user_data_path + "\\site_permissions.db";
#else
    db_path_ = user_data_path + "/site_permissions.db";
#endif
    LOG_INFO_SP("Initializing SitePermissionStore at: " + db_path_);
    std::lock_guard<std::mutex> lock(mutex_);
    if (!OpenDatabase()) {
        LOG_ERROR_SP("Failed to open SitePermissionStore database");
        return false;
    }
    LOG_INFO_SP("SitePermissionStore initialized successfully");
    return true;
}

bool SitePermissionStore::OpenDatabase() {
    if (db_) return true;  // idempotent
    if (sqlite3_open(db_path_.c_str(), &db_) != SQLITE_OK) {
        LOG_ERROR_SP(std::string("sqlite3_open failed: ") + (db_ ? sqlite3_errmsg(db_) : "null"));
        if (db_) { sqlite3_close(db_); db_ = nullptr; }
        return false;
    }
    sqlite3_busy_timeout(db_, 3000);
    char* err = nullptr;
    sqlite3_exec(db_, "PRAGMA journal_mode=WAL;", nullptr, nullptr, nullptr);
    const char* schema =
        "CREATE TABLE IF NOT EXISTS site_permissions ("
        "  domain          TEXT    NOT NULL,"   // normalized host
        "  permission_type INTEGER NOT NULL,"   // Hodos-stable enum (NOT raw CEF bit)
        "  state           INTEGER NOT NULL,"   // 1=allow, 2=block (0/absent = ask)
        "  updated_at      INTEGER NOT NULL,"
        "  PRIMARY KEY (domain, permission_type)"
        ");";
    if (sqlite3_exec(db_, schema, nullptr, nullptr, &err) != SQLITE_OK) {
        LOG_ERROR_SP(std::string("create table failed: ") + (err ? err : "?"));
        if (err) sqlite3_free(err);
        sqlite3_close(db_);
        db_ = nullptr;
        return false;
    }
    return true;
}

void SitePermissionStore::CloseDatabase() {
    std::lock_guard<std::mutex> lock(mutex_);
    if (db_) {
        sqlite3_exec(db_, "PRAGMA wal_checkpoint(TRUNCATE);", nullptr, nullptr, nullptr);
        sqlite3_close(db_);
        db_ = nullptr;
    }
}

SitePermissionState SitePermissionStore::GetState(const std::string& host, SitePermissionType type) {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!db_ || host.empty()) return SitePermissionState::Ask;
    sqlite3_stmt* stmt = nullptr;
    SitePermissionState result = SitePermissionState::Ask;
    const char* sql = "SELECT state FROM site_permissions WHERE domain=? AND permission_type=? LIMIT 1;";
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
        sqlite3_bind_text(stmt, 1, host.c_str(), -1, SQLITE_TRANSIENT);
        sqlite3_bind_int(stmt, 2, static_cast<int>(type));
        if (sqlite3_step(stmt) == SQLITE_ROW) {
            int v = sqlite3_column_int(stmt, 0);
            if (v == 1) result = SitePermissionState::Allow;
            else if (v == 2) result = SitePermissionState::Block;
        }
        sqlite3_finalize(stmt);
    }
    return result;
}

bool SitePermissionStore::SetState(const std::string& host, SitePermissionType type, SitePermissionState state) {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!db_ || host.empty()) return false;
    sqlite3_stmt* stmt = nullptr;
    bool ok = false;
    if (state == SitePermissionState::Ask) {
        // Ask = absence of a row → delete any stored decision.
        const char* sql = "DELETE FROM site_permissions WHERE domain=? AND permission_type=?;";
        if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
            sqlite3_bind_text(stmt, 1, host.c_str(), -1, SQLITE_TRANSIENT);
            sqlite3_bind_int(stmt, 2, static_cast<int>(type));
            ok = (sqlite3_step(stmt) == SQLITE_DONE);
            sqlite3_finalize(stmt);
        }
        return ok;
    }
    const char* sql =
        "INSERT INTO site_permissions (domain, permission_type, state, updated_at) VALUES (?,?,?,?) "
        "ON CONFLICT(domain, permission_type) DO UPDATE SET state=excluded.state, updated_at=excluded.updated_at;";
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
        sqlite3_bind_text(stmt, 1, host.c_str(), -1, SQLITE_TRANSIENT);
        sqlite3_bind_int(stmt, 2, static_cast<int>(type));
        sqlite3_bind_int(stmt, 3, static_cast<int>(state));
        sqlite3_bind_int64(stmt, 4, NowMs());
        ok = (sqlite3_step(stmt) == SQLITE_DONE);
        sqlite3_finalize(stmt);
    }
    return ok;
}

bool SitePermissionStore::ResetDomain(const std::string& host) {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!db_ || host.empty()) return false;
    sqlite3_stmt* stmt = nullptr;
    bool ok = false;
    const char* sql = "DELETE FROM site_permissions WHERE domain=?;";
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
        sqlite3_bind_text(stmt, 1, host.c_str(), -1, SQLITE_TRANSIENT);
        ok = (sqlite3_step(stmt) == SQLITE_DONE);
        sqlite3_finalize(stmt);
    }
    return ok;
}

std::string SitePermissionStore::GetAllForHost(const std::string& host) {
    std::lock_guard<std::mutex> lock(mutex_);
    nlohmann::json arr = nlohmann::json::array();
    if (db_ && !host.empty()) {
        sqlite3_stmt* stmt = nullptr;
        const char* sql = "SELECT permission_type, state FROM site_permissions WHERE domain=?;";
        if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
            sqlite3_bind_text(stmt, 1, host.c_str(), -1, SQLITE_TRANSIENT);
            while (sqlite3_step(stmt) == SQLITE_ROW) {
                arr.push_back({{"type", sqlite3_column_int(stmt, 0)},
                               {"state", sqlite3_column_int(stmt, 1)}});
            }
            sqlite3_finalize(stmt);
        }
    }
    return arr.dump();
}
