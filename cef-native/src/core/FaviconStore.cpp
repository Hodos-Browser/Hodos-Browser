#include "../../include/core/FaviconStore.h"
#include "../../include/core/Logger.h"

#include "include/cef_parser.h"

#include <ctime>

#define LOG_DEBUG_FAV(msg)   Logger::Log(msg, 0, 2)
#define LOG_INFO_FAV(msg)    Logger::Log(msg, 1, 2)
#define LOG_WARNING_FAV(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_FAV(msg)   Logger::Log(msg, 3, 2)

namespace hodos {

FaviconStore& FaviconStore::GetInstance() {
    static FaviconStore instance;
    return instance;
}

FaviconStore::~FaviconStore() { CloseDatabase(); }

void FaviconStore::CloseDatabase() {
    std::lock_guard<std::mutex> lock(mutex_);
    if (db_) {
        sqlite3_close(db_);
        db_ = nullptr;
    }
}

bool FaviconStore::Initialize(const std::string& user_data_path) {
#ifdef _WIN32
    const std::string path = user_data_path + "\\favicons.db";
#else
    const std::string path = user_data_path + "/favicons.db";
#endif
    std::lock_guard<std::mutex> lock(mutex_);
    if (db_) return true;   // idempotent
    if (sqlite3_open(path.c_str(), &db_) != SQLITE_OK) {
        LOG_ERROR_FAV("Failed to open favicon store at " + path);
        if (db_) { sqlite3_close(db_); db_ = nullptr; }
        return false;
    }
    if (!EnsureSchema()) {
        sqlite3_close(db_); db_ = nullptr;
        return false;
    }
    LOG_INFO_FAV("FaviconStore initialized at " + path);
    return true;
}

bool FaviconStore::EnsureSchema() {
    // One row per host. See the header for why this is host-keyed rather than
    // Chromium's/Firefox's two-table icon+mapping shape.
    const char* sql =
        "CREATE TABLE IF NOT EXISTS favicons ("
        "  host       TEXT PRIMARY KEY,"
        "  icon_url   TEXT NOT NULL,"
        "  png        BLOB NOT NULL,"
        "  width      INTEGER NOT NULL,"
        "  updated_at INTEGER NOT NULL"
        ");";
    char* err = nullptr;
    if (sqlite3_exec(db_, sql, nullptr, nullptr, &err) != SQLITE_OK) {
        LOG_ERROR_FAV(std::string("favicon schema failed: ") + (err ? err : "?"));
        if (err) sqlite3_free(err);
        return false;
    }
    return true;
}

bool FaviconStore::Put(const std::string& host, const std::string& icon_url,
                       const std::vector<uint8_t>& png, int width) {
    if (host.empty() || png.empty()) return false;
    // ⛔ A site controls both the icon URL and its bytes. Cap the size so a
    // hostile or simply broken site cannot grow our profile without bound.
    if (png.size() > kMaxPngBytes) {
        LOG_WARNING_FAV("favicon for " + host + " rejected: " +
                        std::to_string(png.size()) + " bytes exceeds cap");
        return false;
    }
    std::lock_guard<std::mutex> lock(mutex_);
    if (!db_) return false;

    const char* sql =
        "INSERT INTO favicons (host, icon_url, png, width, updated_at) "
        "VALUES (?1, ?2, ?3, ?4, ?5) "
        "ON CONFLICT(host) DO UPDATE SET icon_url=?2, png=?3, width=?4, updated_at=?5;";
    sqlite3_stmt* st = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &st, nullptr) != SQLITE_OK) return false;
    sqlite3_bind_text(st, 1, host.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_text(st, 2, icon_url.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_blob(st, 3, png.data(), static_cast<int>(png.size()), SQLITE_TRANSIENT);
    sqlite3_bind_int(st, 4, width);
    sqlite3_bind_int64(st, 5, static_cast<int64_t>(std::time(nullptr)));
    const bool ok = sqlite3_step(st) == SQLITE_DONE;
    sqlite3_finalize(st);
    if (ok) LOG_DEBUG_FAV("stored favicon for " + host + " (" +
                          std::to_string(png.size()) + " bytes)");
    return ok;
}

std::vector<uint8_t> FaviconStore::Get(const std::string& host) {
    std::vector<uint8_t> out;
    if (host.empty()) return out;
    std::lock_guard<std::mutex> lock(mutex_);
    if (!db_) return out;

    const char* sql = "SELECT png, updated_at FROM favicons WHERE host = ?1;";
    sqlite3_stmt* st = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &st, nullptr) != SQLITE_OK) return out;
    sqlite3_bind_text(st, 1, host.c_str(), -1, SQLITE_TRANSIENT);
    if (sqlite3_step(st) == SQLITE_ROW) {
        const int64_t age = static_cast<int64_t>(std::time(nullptr)) - sqlite3_column_int64(st, 1);
        // Serve a stale icon rather than nothing — a slightly out-of-date icon is
        // a far better outcome than a blank tile, and HasFresh() is what drives
        // the re-download. Only the freshness DECISION uses the age.
        const void* blob = sqlite3_column_blob(st, 0);
        const int n = sqlite3_column_bytes(st, 0);
        if (blob && n > 0) {
            const uint8_t* p = static_cast<const uint8_t*>(blob);
            out.assign(p, p + n);
        }
        (void)age;
    }
    sqlite3_finalize(st);
    return out;
}

bool FaviconStore::HasFresh(const std::string& host) {
    if (host.empty()) return false;
    std::lock_guard<std::mutex> lock(mutex_);
    if (!db_) return false;
    const char* sql = "SELECT updated_at FROM favicons WHERE host = ?1;";
    sqlite3_stmt* st = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &st, nullptr) != SQLITE_OK) return false;
    sqlite3_bind_text(st, 1, host.c_str(), -1, SQLITE_TRANSIENT);
    bool fresh = false;
    if (sqlite3_step(st) == SQLITE_ROW) {
        const int64_t age = static_cast<int64_t>(std::time(nullptr)) - sqlite3_column_int64(st, 0);
        fresh = age >= 0 && age < kMaxAgeSeconds;
    }
    sqlite3_finalize(st);
    return fresh;
}

std::string FaviconStore::GetDataUri(const std::string& host) {
    const std::vector<uint8_t> png = Get(host);
    if (png.empty()) return "";
    // ⭐ A data: URI means the React side needs no new URL scheme and — the point
    // of the whole change — issues NO network request to render an icon.
    const CefString b64 = CefBase64Encode(png.data(), png.size());
    return "data:image/png;base64," + b64.ToString();
}

bool FaviconStore::Clear() {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!db_) return false;
    char* err = nullptr;
    const bool ok = sqlite3_exec(db_, "DELETE FROM favicons;", nullptr, nullptr, &err) == SQLITE_OK;
    if (err) sqlite3_free(err);
    if (ok) LOG_INFO_FAV("favicon store cleared");
    return ok;
}

}  // namespace hodos
