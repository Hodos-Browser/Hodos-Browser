#include "../../include/core/BookmarkManager.h"
#include "../../include/core/Logger.h"

#include <nlohmann/json.hpp>
#include <chrono>
#include <algorithm>
#include <unordered_map>

// Logging macros
#define LOG_INFO_BM(msg) Logger::Log(msg, 1, 2)
#define LOG_ERROR_BM(msg) Logger::Log(msg, 3, 2)

// Helper: get current time in Unix milliseconds
static int64_t GetCurrentTimeMs() {
    return std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::system_clock::now().time_since_epoch()).count();
}

// Helper: trim leading/trailing whitespace from a string
static std::string TrimWhitespace(const std::string& str) {
    size_t start = str.find_first_not_of(" \t\n\r");
    if (start == std::string::npos) return "";
    size_t end = str.find_last_not_of(" \t\n\r");
    return str.substr(start, end - start + 1);
}

// ============================================================================
// BookmarkManager singleton
// ============================================================================

BookmarkManager& BookmarkManager::GetInstance() {
    static BookmarkManager instance;
    return instance;
}

BookmarkManager::~BookmarkManager() {
    CloseDatabase();
}

// ============================================================================
// Initialize
// ============================================================================
bool BookmarkManager::Initialize(const std::string& user_data_path) {
    db_path_ = user_data_path + "\\bookmarks.db";
    LOG_INFO_BM("Initializing BookmarkManager at: " + db_path_);

    if (!OpenDatabase()) {
        LOG_ERROR_BM("Failed to open BookmarkManager database");
        return false;
    }

    // Check if default "Favorites" folder exists
    const char* check_sql = "SELECT COUNT(*) FROM bookmark_folders WHERE name = 'Favorites' AND parent_id IS NULL";
    sqlite3_stmt* stmt = nullptr;
    bool needs_default = true;
    if (sqlite3_prepare_v2(db_, check_sql, -1, &stmt, nullptr) == SQLITE_OK) {
        if (sqlite3_step(stmt) == SQLITE_ROW) {
            needs_default = (sqlite3_column_int(stmt, 0) == 0);
        }
        sqlite3_finalize(stmt);
    }

    if (needs_default) {
        int64_t now = GetCurrentTimeMs();
        const char* insert_sql = "INSERT INTO bookmark_folders (name, parent_id, position, created_at, updated_at) "
                                 "VALUES ('Favorites', NULL, 0, ?, ?)";
        sqlite3_stmt* insert_stmt = nullptr;
        if (sqlite3_prepare_v2(db_, insert_sql, -1, &insert_stmt, nullptr) == SQLITE_OK) {
            sqlite3_bind_int64(insert_stmt, 1, now);
            sqlite3_bind_int64(insert_stmt, 2, now);
            if (sqlite3_step(insert_stmt) == SQLITE_DONE) {
                LOG_INFO_BM("Created default 'Favorites' folder");
            } else {
                LOG_ERROR_BM("Failed to create default 'Favorites' folder");
            }
            sqlite3_finalize(insert_stmt);
        }
    }

    LOG_INFO_BM("BookmarkManager initialized successfully");
    return true;
}

// ============================================================================
// Database operations
// ============================================================================
bool BookmarkManager::OpenDatabase() {
    int rc = sqlite3_open_v2(db_path_.c_str(), &db_,
                             SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE, nullptr);
    if (rc != SQLITE_OK) {
        LOG_ERROR_BM("sqlite3_open_v2 failed: " + std::string(sqlite3_errmsg(db_)));
        db_ = nullptr;
        return false;
    }

    // CRITICAL: Enable foreign keys BEFORE anything else
    sqlite3_exec(db_, "PRAGMA foreign_keys = ON;", nullptr, nullptr, nullptr);

    // WAL mode for concurrent reads
    sqlite3_exec(db_, "PRAGMA journal_mode = WAL;", nullptr, nullptr, nullptr);

    // Busy timeout
    sqlite3_busy_timeout(db_, 5000);

    // Create schema
    const char* schema = R"SQL(
        CREATE TABLE IF NOT EXISTS bookmark_folders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            parent_id INTEGER DEFAULT NULL,
            position INTEGER DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (parent_id) REFERENCES bookmark_folders(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS bookmarks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            folder_id INTEGER DEFAULT NULL,
            favicon_url TEXT DEFAULT '',
            position INTEGER DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            last_accessed INTEGER DEFAULT 0,
            FOREIGN KEY (folder_id) REFERENCES bookmark_folders(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS bookmark_tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            bookmark_id INTEGER NOT NULL,
            tag TEXT NOT NULL,
            FOREIGN KEY (bookmark_id) REFERENCES bookmarks(id) ON DELETE CASCADE,
            UNIQUE(bookmark_id, tag)
        );

        CREATE INDEX IF NOT EXISTS idx_bookmarks_url ON bookmarks(url);
        CREATE INDEX IF NOT EXISTS idx_bookmarks_folder ON bookmarks(folder_id);
        CREATE INDEX IF NOT EXISTS idx_bookmarks_last_accessed ON bookmarks(last_accessed);
        CREATE INDEX IF NOT EXISTS idx_bookmark_tags_bookmark ON bookmark_tags(bookmark_id);
        CREATE INDEX IF NOT EXISTS idx_bookmark_tags_tag ON bookmark_tags(tag);
        CREATE INDEX IF NOT EXISTS idx_folders_parent ON bookmark_folders(parent_id);
    )SQL";

    char* errMsg = nullptr;
    rc = sqlite3_exec(db_, schema, nullptr, nullptr, &errMsg);
    if (rc != SQLITE_OK) {
        LOG_ERROR_BM("Schema creation failed: " + std::string(errMsg ? errMsg : "unknown error"));
        if (errMsg) sqlite3_free(errMsg);
        return false;
    }

    LOG_INFO_BM("BookmarkManager database opened and schema created");
    return true;
}

void BookmarkManager::CloseDatabase() {
    if (db_) {
        sqlite3_close(db_);
        db_ = nullptr;
    }
}

// ============================================================================
// Helper: Get folder depth by walking parent chain
// ============================================================================
int BookmarkManager::GetFolderDepth(int folder_id) {
    if (folder_id <= 0) return -1; // Invalid or root level

    int depth = 0;
    int current_id = folder_id;

    while (current_id > 0) {
        const char* sql = "SELECT parent_id FROM bookmark_folders WHERE id = ?";
        sqlite3_stmt* stmt = nullptr;
        if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
            return -1;
        }

        sqlite3_bind_int(stmt, 1, current_id);
        if (sqlite3_step(stmt) == SQLITE_ROW) {
            if (sqlite3_column_type(stmt, 0) == SQLITE_NULL) {
                // Reached a root folder
                sqlite3_finalize(stmt);
                return depth;
            }
            current_id = sqlite3_column_int(stmt, 0);
            depth++;
        } else {
            // Folder not found
            sqlite3_finalize(stmt);
            return -1;
        }
        sqlite3_finalize(stmt);

        // Safety guard against circular references
        if (depth > 10) return -1;
    }

    return depth;
}

// ============================================================================
// Helper: Serialize a bookmark to JSON (fetches tags separately)
// ============================================================================
std::string BookmarkManager::BookmarkToJson(int64_t bookmark_id) {
    nlohmann::json bookmark;

    const char* sql = "SELECT id, url, title, folder_id, favicon_url, position, "
                      "created_at, updated_at, last_accessed FROM bookmarks WHERE id = ?";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
        return "{}";
    }

    sqlite3_bind_int64(stmt, 1, bookmark_id);
    if (sqlite3_step(stmt) != SQLITE_ROW) {
        sqlite3_finalize(stmt);
        return "{}";
    }

    bookmark["id"] = sqlite3_column_int64(stmt, 0);

    const unsigned char* url_text = sqlite3_column_text(stmt, 1);
    bookmark["url"] = url_text ? reinterpret_cast<const char*>(url_text) : "";

    const unsigned char* title_text = sqlite3_column_text(stmt, 2);
    bookmark["title"] = title_text ? reinterpret_cast<const char*>(title_text) : "";

    bookmark["folderId"] = sqlite3_column_type(stmt, 3) == SQLITE_NULL
        ? nlohmann::json(nullptr)
        : nlohmann::json(sqlite3_column_int(stmt, 3));

    const unsigned char* favicon_text = sqlite3_column_text(stmt, 4);
    bookmark["faviconUrl"] = favicon_text ? reinterpret_cast<const char*>(favicon_text) : "";

    bookmark["position"] = sqlite3_column_int(stmt, 5);
    bookmark["createdAt"] = sqlite3_column_int64(stmt, 6);
    bookmark["updatedAt"] = sqlite3_column_int64(stmt, 7);
    bookmark["lastAccessed"] = sqlite3_column_int64(stmt, 8);

    sqlite3_finalize(stmt);

    // Fetch tags
    nlohmann::json tags = nlohmann::json::array();
    const char* tag_sql = "SELECT tag FROM bookmark_tags WHERE bookmark_id = ? ORDER BY tag ASC";
    sqlite3_stmt* tag_stmt = nullptr;
    if (sqlite3_prepare_v2(db_, tag_sql, -1, &tag_stmt, nullptr) == SQLITE_OK) {
        sqlite3_bind_int64(tag_stmt, 1, bookmark_id);
        while (sqlite3_step(tag_stmt) == SQLITE_ROW) {
            const unsigned char* tag_text = sqlite3_column_text(tag_stmt, 0);
            if (tag_text) {
                tags.push_back(reinterpret_cast<const char*>(tag_text));
            }
        }
        sqlite3_finalize(tag_stmt);
    }
    bookmark["tags"] = tags;

    return bookmark.dump();
}

// ============================================================================
// Bookmark CRUD: AddBookmark
// ============================================================================
std::string BookmarkManager::AddBookmark(const std::string& url,
                                          const std::string& title,
                                          int folder_id,
                                          const std::vector<std::string>& tags) {
    nlohmann::json response;

    if (!db_) {
        response["success"] = false;
        response["error"] = "Database not initialized";
        return response.dump();
    }

    // Enforce tag cap
    if (tags.size() > 10) {
        response["success"] = false;
        response["error"] = "Maximum 10 tags per bookmark";
        return response.dump();
    }

    // Validate folder exists if specified
    if (folder_id > 0) {
        const char* check_sql = "SELECT COUNT(*) FROM bookmark_folders WHERE id = ?";
        sqlite3_stmt* check_stmt = nullptr;
        if (sqlite3_prepare_v2(db_, check_sql, -1, &check_stmt, nullptr) == SQLITE_OK) {
            sqlite3_bind_int(check_stmt, 1, folder_id);
            if (sqlite3_step(check_stmt) == SQLITE_ROW && sqlite3_column_int(check_stmt, 0) == 0) {
                sqlite3_finalize(check_stmt);
                response["success"] = false;
                response["error"] = "Folder not found";
                return response.dump();
            }
            sqlite3_finalize(check_stmt);
        }
    }

    int64_t now = GetCurrentTimeMs();

    // Calculate position: append to end of folder
    int new_position = 0;
    {
        const char* pos_sql = folder_id > 0
            ? "SELECT COALESCE(MAX(position), 0) + 1 FROM bookmarks WHERE folder_id = ?"
            : "SELECT COALESCE(MAX(position), 0) + 1 FROM bookmarks WHERE folder_id IS NULL";
        sqlite3_stmt* pos_stmt = nullptr;
        if (sqlite3_prepare_v2(db_, pos_sql, -1, &pos_stmt, nullptr) == SQLITE_OK) {
            if (folder_id > 0) {
                sqlite3_bind_int(pos_stmt, 1, folder_id);
            }
            if (sqlite3_step(pos_stmt) == SQLITE_ROW) {
                new_position = sqlite3_column_int(pos_stmt, 0);
            }
            sqlite3_finalize(pos_stmt);
        }
    }

    // Insert bookmark
    const char* sql = "INSERT INTO bookmarks (url, title, folder_id, position, created_at, updated_at, last_accessed) "
                      "VALUES (?, ?, ?, ?, ?, ?, ?)";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        response["success"] = false;
        response["error"] = std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    sqlite3_bind_text(stmt, 1, url.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_text(stmt, 2, title.c_str(), -1, SQLITE_TRANSIENT);
    if (folder_id > 0) {
        sqlite3_bind_int(stmt, 3, folder_id);
    } else {
        sqlite3_bind_null(stmt, 3);
    }
    sqlite3_bind_int(stmt, 4, new_position);
    sqlite3_bind_int64(stmt, 5, now);
    sqlite3_bind_int64(stmt, 6, now);
    sqlite3_bind_int64(stmt, 7, now);

    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE) {
        response["success"] = false;
        response["error"] = std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    int64_t bookmark_id = sqlite3_last_insert_rowid(db_);

    // Insert tags
    for (const auto& tag : tags) {
        std::string trimmed = TrimWhitespace(tag);
        if (trimmed.empty()) continue;

        const char* tag_sql = "INSERT OR IGNORE INTO bookmark_tags (bookmark_id, tag) VALUES (?, ?)";
        sqlite3_stmt* tag_stmt = nullptr;
        if (sqlite3_prepare_v2(db_, tag_sql, -1, &tag_stmt, nullptr) == SQLITE_OK) {
            sqlite3_bind_int64(tag_stmt, 1, bookmark_id);
            sqlite3_bind_text(tag_stmt, 2, trimmed.c_str(), -1, SQLITE_TRANSIENT);
            sqlite3_step(tag_stmt);
            sqlite3_finalize(tag_stmt);
        }
    }

    LOG_INFO_BM("Added bookmark: " + url + " (id: " + std::to_string(bookmark_id) + ")");

    response["success"] = true;
    response["id"] = bookmark_id;
    return response.dump();
}

// ============================================================================
// Bookmark CRUD: GetBookmark
// ============================================================================
std::string BookmarkManager::GetBookmark(int64_t id) {
    if (!db_) {
        nlohmann::json response;
        response["success"] = false;
        response["error"] = "Database not initialized";
        return response.dump();
    }

    std::string json_str = BookmarkToJson(id);
    if (json_str == "{}") {
        nlohmann::json response;
        response["success"] = false;
        response["error"] = "Bookmark not found";
        return response.dump();
    }

    return json_str;
}

// ============================================================================
// Bookmark CRUD: UpdateBookmark
// ============================================================================
std::string BookmarkManager::UpdateBookmark(int64_t id,
                                             const std::string& title,
                                             const std::string& url,
                                             int folder_id,
                                             const std::vector<std::string>& tags) {
    nlohmann::json response;

    if (!db_) {
        response["success"] = false;
        response["error"] = "Database not initialized";
        return response.dump();
    }

    // Enforce tag cap
    if (tags.size() > 10) {
        response["success"] = false;
        response["error"] = "Maximum 10 tags per bookmark";
        return response.dump();
    }

    int64_t now = GetCurrentTimeMs();

    // Update bookmark fields
    const char* sql = "UPDATE bookmarks SET title = ?, url = ?, folder_id = ?, updated_at = ? WHERE id = ?";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        response["success"] = false;
        response["error"] = std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    sqlite3_bind_text(stmt, 1, title.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_text(stmt, 2, url.c_str(), -1, SQLITE_TRANSIENT);
    if (folder_id > 0) {
        sqlite3_bind_int(stmt, 3, folder_id);
    } else {
        sqlite3_bind_null(stmt, 3);
    }
    sqlite3_bind_int64(stmt, 4, now);
    sqlite3_bind_int64(stmt, 5, id);

    rc = sqlite3_step(stmt);
    int changes = sqlite3_changes(db_);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE || changes == 0) {
        response["success"] = false;
        response["error"] = changes == 0 ? "Bookmark not found" : std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    // Replace tags: delete all existing, then insert new ones
    const char* delete_tags_sql = "DELETE FROM bookmark_tags WHERE bookmark_id = ?";
    sqlite3_stmt* del_stmt = nullptr;
    if (sqlite3_prepare_v2(db_, delete_tags_sql, -1, &del_stmt, nullptr) == SQLITE_OK) {
        sqlite3_bind_int64(del_stmt, 1, id);
        sqlite3_step(del_stmt);
        sqlite3_finalize(del_stmt);
    }

    for (const auto& tag : tags) {
        std::string trimmed = TrimWhitespace(tag);
        if (trimmed.empty()) continue;

        const char* tag_sql = "INSERT OR IGNORE INTO bookmark_tags (bookmark_id, tag) VALUES (?, ?)";
        sqlite3_stmt* tag_stmt = nullptr;
        if (sqlite3_prepare_v2(db_, tag_sql, -1, &tag_stmt, nullptr) == SQLITE_OK) {
            sqlite3_bind_int64(tag_stmt, 1, id);
            sqlite3_bind_text(tag_stmt, 2, trimmed.c_str(), -1, SQLITE_TRANSIENT);
            sqlite3_step(tag_stmt);
            sqlite3_finalize(tag_stmt);
        }
    }

    LOG_INFO_BM("Updated bookmark id: " + std::to_string(id));

    response["success"] = true;
    return response.dump();
}

// ============================================================================
// Bookmark CRUD: RemoveBookmark
// ============================================================================
std::string BookmarkManager::RemoveBookmark(int64_t id) {
    nlohmann::json response;

    if (!db_) {
        response["success"] = false;
        response["error"] = "Database not initialized";
        return response.dump();
    }

    // With PRAGMA foreign_keys = ON, CASCADE handles tag deletion
    const char* sql = "DELETE FROM bookmarks WHERE id = ?";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        response["success"] = false;
        response["error"] = std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    sqlite3_bind_int64(stmt, 1, id);
    rc = sqlite3_step(stmt);
    int changes = sqlite3_changes(db_);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE) {
        response["success"] = false;
        response["error"] = std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    if (changes == 0) {
        response["success"] = false;
        response["error"] = "Bookmark not found";
        return response.dump();
    }

    LOG_INFO_BM("Removed bookmark id: " + std::to_string(id));

    response["success"] = true;
    return response.dump();
}

// ============================================================================
// Bookmark CRUD: SearchBookmarks
// ============================================================================
std::string BookmarkManager::SearchBookmarks(const std::string& query, int limit, int offset) {
    nlohmann::json result;
    result["bookmarks"] = nlohmann::json::array();
    result["total"] = 0;

    if (!db_) return result.dump();

    std::string like_pattern = "%" + query + "%";

    // First, get total count of matching bookmarks
    const char* count_sql = "SELECT COUNT(DISTINCT b.id) FROM bookmarks b "
                            "LEFT JOIN bookmark_tags bt ON b.id = bt.bookmark_id "
                            "WHERE (b.title LIKE ? OR b.url LIKE ? OR bt.tag LIKE ?)";
    sqlite3_stmt* count_stmt = nullptr;
    if (sqlite3_prepare_v2(db_, count_sql, -1, &count_stmt, nullptr) == SQLITE_OK) {
        sqlite3_bind_text(count_stmt, 1, like_pattern.c_str(), -1, SQLITE_TRANSIENT);
        sqlite3_bind_text(count_stmt, 2, like_pattern.c_str(), -1, SQLITE_TRANSIENT);
        sqlite3_bind_text(count_stmt, 3, like_pattern.c_str(), -1, SQLITE_TRANSIENT);
        if (sqlite3_step(count_stmt) == SQLITE_ROW) {
            result["total"] = sqlite3_column_int(count_stmt, 0);
        }
        sqlite3_finalize(count_stmt);
    }

    // Then fetch matching bookmark IDs with pagination
    const char* sql = "SELECT DISTINCT b.id FROM bookmarks b "
                      "LEFT JOIN bookmark_tags bt ON b.id = bt.bookmark_id "
                      "WHERE (b.title LIKE ? OR b.url LIKE ? OR bt.tag LIKE ?) "
                      "ORDER BY b.last_accessed DESC "
                      "LIMIT ? OFFSET ?";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
        return result.dump();
    }

    sqlite3_bind_text(stmt, 1, like_pattern.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_text(stmt, 2, like_pattern.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_text(stmt, 3, like_pattern.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_int(stmt, 4, limit);
    sqlite3_bind_int(stmt, 5, offset);

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        int64_t bookmark_id = sqlite3_column_int64(stmt, 0);
        std::string bookmark_json = BookmarkToJson(bookmark_id);
        if (bookmark_json != "{}") {
            result["bookmarks"].push_back(nlohmann::json::parse(bookmark_json));
        }
    }
    sqlite3_finalize(stmt);

    return result.dump();
}

// ============================================================================
// Bookmark CRUD: GetAllBookmarks
// ============================================================================
std::string BookmarkManager::GetAllBookmarks(int folder_id, int limit, int offset) {
    nlohmann::json result;
    result["bookmarks"] = nlohmann::json::array();
    result["total"] = 0;

    if (!db_) return result.dump();

    // Get total count
    std::string count_sql_str;
    if (folder_id > 0) {
        count_sql_str = "SELECT COUNT(*) FROM bookmarks WHERE folder_id = ?";
    } else if (folder_id == 0) {
        // folder_id 0 means root (no folder)
        count_sql_str = "SELECT COUNT(*) FROM bookmarks WHERE folder_id IS NULL";
    } else {
        // folder_id -1 means ALL bookmarks
        count_sql_str = "SELECT COUNT(*) FROM bookmarks";
    }

    sqlite3_stmt* count_stmt = nullptr;
    if (sqlite3_prepare_v2(db_, count_sql_str.c_str(), -1, &count_stmt, nullptr) == SQLITE_OK) {
        if (folder_id > 0) {
            sqlite3_bind_int(count_stmt, 1, folder_id);
        }
        if (sqlite3_step(count_stmt) == SQLITE_ROW) {
            result["total"] = sqlite3_column_int(count_stmt, 0);
        }
        sqlite3_finalize(count_stmt);
    }

    // Fetch bookmark IDs with pagination
    std::string sql_str;
    if (folder_id > 0) {
        sql_str = "SELECT id FROM bookmarks WHERE folder_id = ? ORDER BY position ASC LIMIT ? OFFSET ?";
    } else if (folder_id == 0) {
        sql_str = "SELECT id FROM bookmarks WHERE folder_id IS NULL ORDER BY position ASC LIMIT ? OFFSET ?";
    } else {
        sql_str = "SELECT id FROM bookmarks ORDER BY position ASC LIMIT ? OFFSET ?";
    }

    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql_str.c_str(), -1, &stmt, nullptr) != SQLITE_OK) {
        return result.dump();
    }

    int bind_idx = 1;
    if (folder_id > 0) {
        sqlite3_bind_int(stmt, bind_idx++, folder_id);
    }
    sqlite3_bind_int(stmt, bind_idx++, limit);
    sqlite3_bind_int(stmt, bind_idx++, offset);

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        int64_t bookmark_id = sqlite3_column_int64(stmt, 0);
        std::string bookmark_json = BookmarkToJson(bookmark_id);
        if (bookmark_json != "{}") {
            result["bookmarks"].push_back(nlohmann::json::parse(bookmark_json));
        }
    }
    sqlite3_finalize(stmt);

    return result.dump();
}

// ============================================================================
// Bookmark CRUD: IsBookmarked
// ============================================================================
std::string BookmarkManager::IsBookmarked(const std::string& url) {
    nlohmann::json response;

    if (!db_) {
        response["bookmarked"] = false;
        return response.dump();
    }

    const char* sql = "SELECT COUNT(*) FROM bookmarks WHERE url = ?";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        response["bookmarked"] = false;
        return response.dump();
    }

    sqlite3_bind_text(stmt, 1, url.c_str(), -1, SQLITE_STATIC);

    response["bookmarked"] = false;
    if (sqlite3_step(stmt) == SQLITE_ROW) {
        response["bookmarked"] = (sqlite3_column_int(stmt, 0) > 0);
    }
    sqlite3_finalize(stmt);

    return response.dump();
}

// ============================================================================
// Bookmark CRUD: UpdateLastAccessed
// ============================================================================
std::string BookmarkManager::UpdateLastAccessed(int64_t id) {
    nlohmann::json response;

    if (!db_) {
        response["success"] = false;
        response["error"] = "Database not initialized";
        return response.dump();
    }

    int64_t now = GetCurrentTimeMs();

    const char* sql = "UPDATE bookmarks SET last_accessed = ? WHERE id = ?";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        response["success"] = false;
        response["error"] = std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    sqlite3_bind_int64(stmt, 1, now);
    sqlite3_bind_int64(stmt, 2, id);
    rc = sqlite3_step(stmt);
    int changes = sqlite3_changes(db_);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE || changes == 0) {
        response["success"] = false;
        response["error"] = changes == 0 ? "Bookmark not found" : std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    response["success"] = true;
    return response.dump();
}

// ============================================================================
// Bookmark CRUD: GetAllTags
// ============================================================================
std::string BookmarkManager::GetAllTags() {
    nlohmann::json tags = nlohmann::json::array();

    if (!db_) return tags.dump();

    const char* sql = "SELECT DISTINCT tag FROM bookmark_tags ORDER BY tag ASC";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) == SQLITE_OK) {
        while (sqlite3_step(stmt) == SQLITE_ROW) {
            const unsigned char* text = sqlite3_column_text(stmt, 0);
            if (text) {
                tags.push_back(reinterpret_cast<const char*>(text));
            }
        }
        sqlite3_finalize(stmt);
    }

    return tags.dump();
}

// ============================================================================
// Folder CRUD: CreateFolder
// ============================================================================
std::string BookmarkManager::CreateFolder(const std::string& name, int parent_id) {
    nlohmann::json response;

    if (!db_) {
        response["success"] = false;
        response["error"] = "Database not initialized";
        return response.dump();
    }

    if (name.empty()) {
        response["success"] = false;
        response["error"] = "Folder name cannot be empty";
        return response.dump();
    }

    // Enforce depth cap: max 3 levels (root=0, sub=1, subsub=2)
    // If parent_id is specified, check its depth
    if (parent_id > 0) {
        int parent_depth = GetFolderDepth(parent_id);
        if (parent_depth < 0) {
            response["success"] = false;
            response["error"] = "Parent folder not found";
            return response.dump();
        }
        if (parent_depth >= 2) {
            response["success"] = false;
            response["error"] = "Maximum folder nesting depth (3 levels) exceeded";
            return response.dump();
        }
    }

    int64_t now = GetCurrentTimeMs();

    // Calculate position: append to end of parent
    int new_position = 0;
    {
        const char* pos_sql = parent_id > 0
            ? "SELECT COALESCE(MAX(position), 0) + 1 FROM bookmark_folders WHERE parent_id = ?"
            : "SELECT COALESCE(MAX(position), 0) + 1 FROM bookmark_folders WHERE parent_id IS NULL";
        sqlite3_stmt* pos_stmt = nullptr;
        if (sqlite3_prepare_v2(db_, pos_sql, -1, &pos_stmt, nullptr) == SQLITE_OK) {
            if (parent_id > 0) {
                sqlite3_bind_int(pos_stmt, 1, parent_id);
            }
            if (sqlite3_step(pos_stmt) == SQLITE_ROW) {
                new_position = sqlite3_column_int(pos_stmt, 0);
            }
            sqlite3_finalize(pos_stmt);
        }
    }

    const char* sql = "INSERT INTO bookmark_folders (name, parent_id, position, created_at, updated_at) "
                      "VALUES (?, ?, ?, ?, ?)";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        response["success"] = false;
        response["error"] = std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    sqlite3_bind_text(stmt, 1, name.c_str(), -1, SQLITE_TRANSIENT);
    if (parent_id > 0) {
        sqlite3_bind_int(stmt, 2, parent_id);
    } else {
        sqlite3_bind_null(stmt, 2);
    }
    sqlite3_bind_int(stmt, 3, new_position);
    sqlite3_bind_int64(stmt, 4, now);
    sqlite3_bind_int64(stmt, 5, now);

    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE) {
        response["success"] = false;
        response["error"] = std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    int64_t folder_id = sqlite3_last_insert_rowid(db_);

    LOG_INFO_BM("Created folder: " + name + " (id: " + std::to_string(folder_id) + ")");

    response["success"] = true;
    response["id"] = folder_id;
    return response.dump();
}

// ============================================================================
// Folder CRUD: ListFolders
// ============================================================================
std::string BookmarkManager::ListFolders(int parent_id) {
    nlohmann::json result;
    result["folders"] = nlohmann::json::array();

    if (!db_) return result.dump();

    std::string sql_str = parent_id > 0
        ? "SELECT id, name, parent_id, position, created_at, updated_at FROM bookmark_folders WHERE parent_id = ? ORDER BY position ASC"
        : "SELECT id, name, parent_id, position, created_at, updated_at FROM bookmark_folders WHERE parent_id IS NULL ORDER BY position ASC";

    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql_str.c_str(), -1, &stmt, nullptr) != SQLITE_OK) {
        return result.dump();
    }

    if (parent_id > 0) {
        sqlite3_bind_int(stmt, 1, parent_id);
    }

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        nlohmann::json folder;
        folder["id"] = sqlite3_column_int64(stmt, 0);

        const unsigned char* name_text = sqlite3_column_text(stmt, 1);
        folder["name"] = name_text ? reinterpret_cast<const char*>(name_text) : "";

        folder["parentId"] = sqlite3_column_type(stmt, 2) == SQLITE_NULL
            ? nlohmann::json(nullptr)
            : nlohmann::json(sqlite3_column_int(stmt, 2));

        folder["position"] = sqlite3_column_int(stmt, 3);
        folder["createdAt"] = sqlite3_column_int64(stmt, 4);
        folder["updatedAt"] = sqlite3_column_int64(stmt, 5);

        result["folders"].push_back(folder);
    }
    sqlite3_finalize(stmt);

    return result.dump();
}

// ============================================================================
// Folder CRUD: UpdateFolder
// ============================================================================
std::string BookmarkManager::UpdateFolder(int64_t id, const std::string& name) {
    nlohmann::json response;

    if (!db_) {
        response["success"] = false;
        response["error"] = "Database not initialized";
        return response.dump();
    }

    if (name.empty()) {
        response["success"] = false;
        response["error"] = "Folder name cannot be empty";
        return response.dump();
    }

    int64_t now = GetCurrentTimeMs();

    const char* sql = "UPDATE bookmark_folders SET name = ?, updated_at = ? WHERE id = ?";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        response["success"] = false;
        response["error"] = std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    sqlite3_bind_text(stmt, 1, name.c_str(), -1, SQLITE_TRANSIENT);
    sqlite3_bind_int64(stmt, 2, now);
    sqlite3_bind_int64(stmt, 3, id);

    rc = sqlite3_step(stmt);
    int changes = sqlite3_changes(db_);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE || changes == 0) {
        response["success"] = false;
        response["error"] = changes == 0 ? "Folder not found" : std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    LOG_INFO_BM("Updated folder id: " + std::to_string(id) + " name: " + name);

    response["success"] = true;
    return response.dump();
}

// ============================================================================
// Folder CRUD: RemoveFolder
// ============================================================================
std::string BookmarkManager::RemoveFolder(int64_t id) {
    nlohmann::json response;

    if (!db_) {
        response["success"] = false;
        response["error"] = "Database not initialized";
        return response.dump();
    }

    // With PRAGMA foreign_keys = ON, this single DELETE cascades to:
    // 1. All bookmarks in this folder (via bookmarks.folder_id FK)
    // 2. All sub-folders (via bookmark_folders.parent_id FK)
    // 3. All bookmarks in sub-folders (via cascading from step 2)
    // 4. All tags for deleted bookmarks (via bookmark_tags.bookmark_id FK)
    const char* sql = "DELETE FROM bookmark_folders WHERE id = ?";
    sqlite3_stmt* stmt = nullptr;
    int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
    if (rc != SQLITE_OK) {
        response["success"] = false;
        response["error"] = std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    sqlite3_bind_int64(stmt, 1, id);
    rc = sqlite3_step(stmt);
    int changes = sqlite3_changes(db_);
    sqlite3_finalize(stmt);

    if (rc != SQLITE_DONE) {
        response["success"] = false;
        response["error"] = std::string(sqlite3_errmsg(db_));
        return response.dump();
    }

    if (changes == 0) {
        response["success"] = false;
        response["error"] = "Folder not found";
        return response.dump();
    }

    LOG_INFO_BM("Removed folder id: " + std::to_string(id));

    response["success"] = true;
    return response.dump();
}

// ============================================================================
// Folder CRUD: GetFolderTree
// ============================================================================
std::string BookmarkManager::GetFolderTree() {
    nlohmann::json tree = nlohmann::json::array();

    if (!db_) return tree.dump();

    // Get all folders in one query (small dataset, build tree in memory)
    const char* sql = "SELECT id, name, parent_id, position, created_at, updated_at "
                      "FROM bookmark_folders ORDER BY position ASC";
    sqlite3_stmt* stmt = nullptr;
    if (sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr) != SQLITE_OK) {
        return tree.dump();
    }

    // Collect all folders
    struct FolderNode {
        nlohmann::json data;
        int parent_id;  // -1 for root
    };

    std::vector<FolderNode> all_folders;
    while (sqlite3_step(stmt) == SQLITE_ROW) {
        nlohmann::json folder;
        folder["id"] = sqlite3_column_int64(stmt, 0);

        const unsigned char* name_text = sqlite3_column_text(stmt, 1);
        folder["name"] = name_text ? reinterpret_cast<const char*>(name_text) : "";

        int pid = -1;
        if (sqlite3_column_type(stmt, 2) != SQLITE_NULL) {
            pid = sqlite3_column_int(stmt, 2);
        }

        folder["parentId"] = pid < 0 ? nlohmann::json(nullptr) : nlohmann::json(pid);
        folder["position"] = sqlite3_column_int(stmt, 3);
        folder["createdAt"] = sqlite3_column_int64(stmt, 4);
        folder["updatedAt"] = sqlite3_column_int64(stmt, 5);
        folder["children"] = nlohmann::json::array();

        all_folders.push_back({folder, pid});
    }
    sqlite3_finalize(stmt);

    // Two-pass tree building:
    // Pass 1: Add root folders to tree, build ID -> pointer map
    std::unordered_map<int64_t, nlohmann::json*> folder_map;
    for (auto& node : all_folders) {
        if (node.parent_id < 0) {
            tree.push_back(node.data);
            folder_map[node.data["id"].get<int64_t>()] = &tree.back();
        }
    }

    // Pass 2: Attach children to their parents
    for (auto& node : all_folders) {
        if (node.parent_id >= 0) {
            auto it = folder_map.find(static_cast<int64_t>(node.parent_id));
            if (it != folder_map.end()) {
                it->second->at("children").push_back(node.data);
                // Update map to include this child for deeper nesting
                auto& children = it->second->at("children");
                folder_map[node.data["id"].get<int64_t>()] = &children.back();
            }
        }
    }

    return tree.dump();
}
