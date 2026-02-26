#include "../../include/core/ProfileImporter.h"
#include "../../include/core/BookmarkManager.h"
#include "../../include/core/HistoryManager.h"
#include "../../include/core/Logger.h"

#include <nlohmann/json.hpp>
#include <sqlite3.h>
#include <fstream>
#include <sstream>
#include <filesystem>
#include <cstdlib>

#ifdef _WIN32
#include <windows.h>
#include <shlobj.h>
#else
#include <unistd.h>
#endif

namespace fs = std::filesystem;
using json = nlohmann::json;

// Local logging macros (module ID 11 = ProfileImporter)
#define LOG_INFO_PI(msg) Logger::Log(msg, 1, 11)
#define LOG_DEBUG_PI(msg) Logger::Log(msg, 0, 11)
#define LOG_ERROR_PI(msg) Logger::Log(msg, 3, 11)
#define LOG_WARNING_PI(msg) Logger::Log(msg, 2, 11)

// ============================================================================
// Path Helpers
// ============================================================================

std::string ProfileImporter::GetChromeProfilePath() {
#ifdef _WIN32
    const char* localAppData = std::getenv("LOCALAPPDATA");
    if (localAppData) {
        return std::string(localAppData) + "\\Google\\Chrome\\User Data\\Default";
    }
#elif defined(__APPLE__)
    const char* home = std::getenv("HOME");
    if (home) {
        return std::string(home) + "/Library/Application Support/Google/Chrome/Default";
    }
#endif
    return "";
}

std::string ProfileImporter::GetBraveProfilePath() {
#ifdef _WIN32
    const char* localAppData = std::getenv("LOCALAPPDATA");
    if (localAppData) {
        return std::string(localAppData) + "\\BraveSoftware\\Brave-Browser\\User Data\\Default";
    }
#elif defined(__APPLE__)
    const char* home = std::getenv("HOME");
    if (home) {
        return std::string(home) + "/Library/Application Support/BraveSoftware/Brave-Browser/Default";
    }
#endif
    return "";
}

std::string ProfileImporter::GetEdgeProfilePath() {
#ifdef _WIN32
    const char* localAppData = std::getenv("LOCALAPPDATA");
    if (localAppData) {
        return std::string(localAppData) + "\\Microsoft\\Edge\\User Data\\Default";
    }
#elif defined(__APPLE__)
    const char* home = std::getenv("HOME");
    if (home) {
        return std::string(home) + "/Library/Application Support/Microsoft Edge/Default";
    }
#endif
    return "";
}

std::string ProfileImporter::GetFirefoxProfilePath() {
    // Firefox uses a random profile directory name - skip for now
    // Would need to parse profiles.ini to find the default profile
    return "";
}

std::string ProfileImporter::GetTempFilePath(const std::string& filename) {
#ifdef _WIN32
    char tempPath[MAX_PATH];
    GetTempPathA(MAX_PATH, tempPath);
    return std::string(tempPath) + filename;
#else
    return "/tmp/" + filename;
#endif
}

bool ProfileImporter::CopyFilePortable(const std::string& src, const std::string& dst) {
#ifdef _WIN32
    return ::CopyFileA(src.c_str(), dst.c_str(), FALSE) != 0;
#else
    try {
        fs::copy_file(src, dst, fs::copy_options::overwrite_existing);
        return true;
    } catch (...) {
        return false;
    }
#endif
}

// ============================================================================
// Profile Detection
// ============================================================================

int ProfileImporter::CountBookmarksInFile(const std::string& bookmarksPath) {
    std::ifstream file(bookmarksPath);
    if (!file.is_open()) return 0;

    try {
        json bookmarks;
        file >> bookmarks;

        int count = 0;
        std::function<void(const json&)> countNodes = [&](const json& node) {
            if (node.contains("type")) {
                if (node["type"] == "url") {
                    count++;
                } else if (node["type"] == "folder" && node.contains("children")) {
                    for (const auto& child : node["children"]) {
                        countNodes(child);
                    }
                }
            }
        };

        if (bookmarks.contains("roots")) {
            if (bookmarks["roots"].contains("bookmark_bar")) {
                countNodes(bookmarks["roots"]["bookmark_bar"]);
            }
            if (bookmarks["roots"].contains("other")) {
                countNodes(bookmarks["roots"]["other"]);
            }
        }

        return count;
    } catch (...) {
        return 0;
    }
}

int ProfileImporter::CountHistoryInFile(const std::string& historyPath) {
    // Need to copy the file first as it may be locked
    // Chrome uses WAL mode - copy all three files
    std::string tempPath = GetTempFilePath("hodos_history_count.db");
    if (!CopyFilePortable(historyPath, tempPath)) {
        return 0;
    }

    // Also copy WAL files if they exist
    std::string walFile = historyPath + "-wal";
    std::string shmFile = historyPath + "-shm";
    if (fs::exists(walFile)) {
        CopyFilePortable(walFile, tempPath + "-wal");
    }
    if (fs::exists(shmFile)) {
        CopyFilePortable(shmFile, tempPath + "-shm");
    }

    sqlite3* db = nullptr;
    if (sqlite3_open_v2(tempPath.c_str(), &db, SQLITE_OPEN_READONLY, nullptr) != SQLITE_OK) {
        fs::remove(tempPath);
        fs::remove(tempPath + "-wal");
        fs::remove(tempPath + "-shm");
        return 0;
    }

    int count = 0;
    const char* sql = "SELECT COUNT(*) FROM urls";
    sqlite3_stmt* stmt = nullptr;

    if (sqlite3_prepare_v2(db, sql, -1, &stmt, nullptr) == SQLITE_OK) {
        if (sqlite3_step(stmt) == SQLITE_ROW) {
            count = sqlite3_column_int(stmt, 0);
        }
        sqlite3_finalize(stmt);
    }

    sqlite3_close(db);
    fs::remove(tempPath);
    fs::remove(tempPath + "-wal");
    fs::remove(tempPath + "-shm");

    return count;
}

std::vector<DetectedProfile> ProfileImporter::DetectProfiles() {
    std::vector<DetectedProfile> profiles;

    // Chrome
    std::string chromePath = GetChromeProfilePath();
    if (!chromePath.empty() && fs::exists(chromePath)) {
        DetectedProfile profile;
        profile.browserName = "Chrome";
        profile.profilePath = chromePath;
        profile.profileName = "Default";

        std::string bookmarksFile = chromePath +
#ifdef _WIN32
            "\\Bookmarks";
#else
            "/Bookmarks";
#endif
        std::string historyFile = chromePath +
#ifdef _WIN32
            "\\History";
#else
            "/History";
#endif

        profile.hasBookmarks = fs::exists(bookmarksFile);
        profile.hasHistory = fs::exists(historyFile);
        profile.bookmarkCount = profile.hasBookmarks ? CountBookmarksInFile(bookmarksFile) : 0;
        profile.historyCount = profile.hasHistory ? CountHistoryInFile(historyFile) : 0;

        if (profile.hasBookmarks || profile.hasHistory) {
            profiles.push_back(profile);
            LOG_INFO_PI("📂 Detected Chrome profile: " + std::to_string(profile.bookmarkCount) +
                       " bookmarks, " + std::to_string(profile.historyCount) + " history entries");
        }
    }

    // Brave
    std::string bravePath = GetBraveProfilePath();
    if (!bravePath.empty() && fs::exists(bravePath)) {
        DetectedProfile profile;
        profile.browserName = "Brave";
        profile.profilePath = bravePath;
        profile.profileName = "Default";

        std::string bookmarksFile = bravePath +
#ifdef _WIN32
            "\\Bookmarks";
#else
            "/Bookmarks";
#endif
        std::string historyFile = bravePath +
#ifdef _WIN32
            "\\History";
#else
            "/History";
#endif

        profile.hasBookmarks = fs::exists(bookmarksFile);
        profile.hasHistory = fs::exists(historyFile);
        profile.bookmarkCount = profile.hasBookmarks ? CountBookmarksInFile(bookmarksFile) : 0;
        profile.historyCount = profile.hasHistory ? CountHistoryInFile(historyFile) : 0;

        if (profile.hasBookmarks || profile.hasHistory) {
            profiles.push_back(profile);
            LOG_INFO_PI("📂 Detected Brave profile: " + std::to_string(profile.bookmarkCount) +
                       " bookmarks, " + std::to_string(profile.historyCount) + " history entries");
        }
    }

    // Edge
    std::string edgePath = GetEdgeProfilePath();
    if (!edgePath.empty() && fs::exists(edgePath)) {
        DetectedProfile profile;
        profile.browserName = "Edge";
        profile.profilePath = edgePath;
        profile.profileName = "Default";

        std::string bookmarksFile = edgePath +
#ifdef _WIN32
            "\\Bookmarks";
#else
            "/Bookmarks";
#endif
        std::string historyFile = edgePath +
#ifdef _WIN32
            "\\History";
#else
            "/History";
#endif

        profile.hasBookmarks = fs::exists(bookmarksFile);
        profile.hasHistory = fs::exists(historyFile);
        profile.bookmarkCount = profile.hasBookmarks ? CountBookmarksInFile(bookmarksFile) : 0;
        profile.historyCount = profile.hasHistory ? CountHistoryInFile(historyFile) : 0;

        if (profile.hasBookmarks || profile.hasHistory) {
            profiles.push_back(profile);
            LOG_INFO_PI("📂 Detected Edge profile: " + std::to_string(profile.bookmarkCount) +
                       " bookmarks, " + std::to_string(profile.historyCount) + " history entries");
        }
    }

    LOG_INFO_PI("📂 Profile detection complete: " + std::to_string(profiles.size()) + " profiles found");
    return profiles;
}

// ============================================================================
// Bookmark Import
// ============================================================================

// Map from Chrome folder name to our folder ID
static std::map<std::string, int> importedFolders;

static void ImportBookmarkNodeRecursive(
    const json& node,
    int parentFolderId,
    ImportResult& result,
    ImportProgressCallback progress
) {
    if (!node.contains("type")) return;

    std::string type = node["type"].get<std::string>();
    std::string name = node.value("name", "Untitled");

    if (type == "url") {
        std::string url = node.value("url", "");
        if (url.empty()) {
            result.skipped++;
            return;
        }

        // Add bookmark using BookmarkManager
        std::vector<std::string> tags;  // No tags from Chrome import
        std::string response = BookmarkManager::GetInstance().AddBookmark(
            url, name, parentFolderId, tags
        );

        // Parse response to check success
        try {
            json resp = json::parse(response);
            if (resp.value("success", false)) {
                result.bookmarksImported++;
                if (progress) {
                    progress("bookmarks", result.bookmarksImported, 0, name);
                }
            } else {
                result.skipped++;
            }
        } catch (...) {
            result.skipped++;
        }

    } else if (type == "folder") {
        // Create folder
        std::string folderResponse = BookmarkManager::GetInstance().CreateFolder(
            name, parentFolderId
        );

        int newFolderId = parentFolderId;  // Fallback to parent if creation fails
        try {
            json resp = json::parse(folderResponse);
            if (resp.value("success", false)) {
                newFolderId = resp.value("id", parentFolderId);
                result.foldersImported++;
            }
        } catch (...) {
            // Use parent folder as fallback
        }

        // Recurse into children
        if (node.contains("children")) {
            for (const auto& child : node["children"]) {
                ImportBookmarkNodeRecursive(child, newFolderId, result, progress);
            }
        }
    }
}

ImportResult ProfileImporter::ImportBookmarks(
    const std::string& profilePath,
    ImportProgressCallback progress
) {
    ImportResult result = {true, "", 0, 0, 0, 0};
    importedFolders.clear();

    std::string bookmarksFile = profilePath +
#ifdef _WIN32
        "\\Bookmarks";
#else
        "/Bookmarks";
#endif

    LOG_INFO_PI("📚 Importing bookmarks from: " + bookmarksFile);

    std::ifstream file(bookmarksFile);
    if (!file.is_open()) {
        result.success = false;
        result.error = "Could not open Bookmarks file";
        LOG_ERROR_PI("❌ " + result.error);
        return result;
    }

    json bookmarks;
    try {
        file >> bookmarks;
    } catch (const std::exception& e) {
        result.success = false;
        result.error = std::string("Invalid JSON in Bookmarks file: ") + e.what();
        LOG_ERROR_PI("❌ " + result.error);
        return result;
    }

    if (!bookmarks.contains("roots")) {
        result.success = false;
        result.error = "Invalid Bookmarks file format (no 'roots' key)";
        LOG_ERROR_PI("❌ " + result.error);
        return result;
    }

    // Import bookmark bar
    if (bookmarks["roots"].contains("bookmark_bar") &&
        bookmarks["roots"]["bookmark_bar"].contains("children")) {

        if (progress) progress("bookmarks", 0, 0, "Importing Bookmark Bar...");

        for (const auto& child : bookmarks["roots"]["bookmark_bar"]["children"]) {
            ImportBookmarkNodeRecursive(child, -1, result, progress);  // -1 = root
        }
    }

    // Import "Other Bookmarks"
    if (bookmarks["roots"].contains("other") &&
        bookmarks["roots"]["other"].contains("children")) {

        if (progress) progress("bookmarks", 0, 0, "Importing Other Bookmarks...");

        // Create "Imported" folder for other bookmarks
        std::string folderResponse = BookmarkManager::GetInstance().CreateFolder("Imported", -1);
        int importedFolderId = -1;
        try {
            json resp = json::parse(folderResponse);
            if (resp.value("success", false)) {
                importedFolderId = resp.value("id", -1);
                result.foldersImported++;
            }
        } catch (...) {}

        for (const auto& child : bookmarks["roots"]["other"]["children"]) {
            ImportBookmarkNodeRecursive(child, importedFolderId, result, progress);
        }
    }

    LOG_INFO_PI("✅ Bookmark import complete: " + std::to_string(result.bookmarksImported) +
               " bookmarks, " + std::to_string(result.foldersImported) + " folders, " +
               std::to_string(result.skipped) + " skipped");

    return result;
}

// ============================================================================
// History Import
// ============================================================================

ImportResult ProfileImporter::ImportHistory(
    const std::string& profilePath,
    int maxEntries,
    ImportProgressCallback progress
) {
    ImportResult result = {true, "", 0, 0, 0, 0};

    std::string historyFile = profilePath +
#ifdef _WIN32
        "\\History";
#else
        "/History";
#endif

    LOG_INFO_PI("📜 Importing history from: " + historyFile);

    // Copy the file (Chrome locks it while running)
    // Chrome uses WAL mode - we need to copy History, History-wal, and History-shm
    std::string tempPath = GetTempFilePath("hodos_history_import.db");
    std::string tempWalPath = tempPath + "-wal";
    std::string tempShmPath = tempPath + "-shm";

    LOG_DEBUG_PI("📜 Copying History to temp: " + tempPath);

    if (!CopyFilePortable(historyFile, tempPath)) {
        result.success = false;
        result.error = "Could not copy History file. Is the browser running?";
        LOG_ERROR_PI("❌ " + result.error);
        return result;
    }

    // Also copy WAL files if they exist (Chrome uses WAL mode)
    std::string walFile = historyFile + "-wal";
    std::string shmFile = historyFile + "-shm";
    if (fs::exists(walFile)) {
        CopyFilePortable(walFile, tempWalPath);
        LOG_DEBUG_PI("📜 Copied WAL file");
    }
    if (fs::exists(shmFile)) {
        CopyFilePortable(shmFile, tempShmPath);
        LOG_DEBUG_PI("📜 Copied SHM file");
    }

    // Check if temp file exists and has size
    if (fs::exists(tempPath)) {
        auto fileSize = fs::file_size(tempPath);
        LOG_DEBUG_PI("📜 Temp file size: " + std::to_string(fileSize) + " bytes");
    } else {
        LOG_ERROR_PI("❌ Temp file does not exist after copy!");
    }

    // Open the copy
    sqlite3* db = nullptr;
    int openResult = sqlite3_open_v2(tempPath.c_str(), &db, SQLITE_OPEN_READONLY, nullptr);
    if (openResult != SQLITE_OK) {
        result.success = false;
        result.error = "Could not open History database: " + std::string(sqlite3_errmsg(db));
        LOG_ERROR_PI("❌ " + result.error + " (code: " + std::to_string(openResult) + ")");
        fs::remove(tempPath);
        return result;
    }
    LOG_DEBUG_PI("📜 SQLite database opened successfully");
    
    // Debug: Check if there are any rows at all first
    int totalRows = 0;
    sqlite3_stmt* countStmt = nullptr;
    if (sqlite3_prepare_v2(db, "SELECT COUNT(*) FROM urls", -1, &countStmt, nullptr) == SQLITE_OK) {
        if (sqlite3_step(countStmt) == SQLITE_ROW) {
            totalRows = sqlite3_column_int(countStmt, 0);
        }
        sqlite3_finalize(countStmt);
    }
    LOG_DEBUG_PI("📜 Total rows in urls table: " + std::to_string(totalRows));
    
    // Debug: Try a simpler query first
    sqlite3_stmt* simpleStmt = nullptr;
    if (sqlite3_prepare_v2(db, "SELECT url FROM urls LIMIT 5", -1, &simpleStmt, nullptr) == SQLITE_OK) {
        int simpleCount = 0;
        while (sqlite3_step(simpleStmt) == SQLITE_ROW) {
            simpleCount++;
            const char* url = (const char*)sqlite3_column_text(simpleStmt, 0);
            LOG_DEBUG_PI("📜 Sample URL: " + std::string(url ? url : "NULL"));
        }
        sqlite3_finalize(simpleStmt);
        LOG_DEBUG_PI("📜 Simple query returned: " + std::to_string(simpleCount) + " rows");
    }
    
    // Debug: Try the full query with just 5 rows
    sqlite3_stmt* testStmt = nullptr;
    std::string testSql = "SELECT url, title, last_visit_time FROM urls ORDER BY last_visit_time DESC LIMIT 5";
    LOG_DEBUG_PI("📜 Testing full query: " + testSql);
    int testPrepare = sqlite3_prepare_v2(db, testSql.c_str(), -1, &testStmt, nullptr);
    if (testPrepare == SQLITE_OK) {
        int testCount = 0;
        int testStep = 0;
        while ((testStep = sqlite3_step(testStmt)) == SQLITE_ROW) {
            testCount++;
            const char* u = (const char*)sqlite3_column_text(testStmt, 0);
            const char* t = (const char*)sqlite3_column_text(testStmt, 1);
            int64_t ts = sqlite3_column_int64(testStmt, 2);
            LOG_DEBUG_PI("📜 Test row: url=" + std::string(u ? u : "NULL") + 
                        ", title=" + std::string(t ? t : "NULL") + 
                        ", time=" + std::to_string(ts));
        }
        LOG_DEBUG_PI("📜 Test query result: " + std::to_string(testCount) + " rows, final step=" + std::to_string(testStep));
        sqlite3_finalize(testStmt);
    } else {
        LOG_ERROR_PI("📜 Test query prepare failed: " + std::string(sqlite3_errmsg(db)));
    }
    
    // Now prepare the main query (after diagnostics)
    std::string sql = "SELECT url, title, last_visit_time FROM urls "
                      "ORDER BY last_visit_time DESC LIMIT " + std::to_string(maxEntries);
    sqlite3_stmt* stmt = nullptr;

    int prepareResult = sqlite3_prepare_v2(db, sql.c_str(), -1, &stmt, nullptr);
    if (prepareResult != SQLITE_OK) {
        result.success = false;
        result.error = "Could not query History database: " + std::string(sqlite3_errmsg(db));
        LOG_ERROR_PI("❌ " + result.error + " (code: " + std::to_string(prepareResult) + ")");
        sqlite3_close(db);
        fs::remove(tempPath);
        fs::remove(tempPath + "-wal");
        fs::remove(tempPath + "-shm");
        return result;
    }
    LOG_DEBUG_PI("📜 Main query prepared: " + sql);
    
    LOG_DEBUG_PI("📜 Starting main row iteration...");

    auto& historyMgr = HistoryManager::GetInstance();
    int count = 0;
    int stepResult = 0;
    int rowCount = 0;
    
    while ((stepResult = sqlite3_step(stmt)) == SQLITE_ROW) {
        rowCount++;
        const char* urlPtr = (const char*)sqlite3_column_text(stmt, 0);
        const char* titlePtr = (const char*)sqlite3_column_text(stmt, 1);

        std::string url = urlPtr ? urlPtr : "";
        std::string title = titlePtr ? titlePtr : "";

        if (url.empty()) {
            result.skipped++;
            continue;
        }

        // Skip internal Chrome pages
        if (url.find("chrome://") == 0 || url.find("chrome-extension://") == 0) {
            result.skipped++;
            continue;
        }

        // Add to our history (using transition_type 0 = typed)
        if (historyMgr.AddVisit(url, title, 0)) {
            result.historyImported++;
            count++;

            if (progress && count % 100 == 0) {
                progress("history", count, maxEntries, "Importing history...");
            }
        } else {
            result.skipped++;
        }
    }

    sqlite3_finalize(stmt);
    sqlite3_close(db);

    // Clean up temp files
    fs::remove(tempPath);
    fs::remove(tempPath + "-wal");
    fs::remove(tempPath + "-shm");

    LOG_DEBUG_PI("📜 Loop finished. stepResult=" + std::to_string(stepResult) +
                " rowCount=" + std::to_string(rowCount) +
                " (SQLITE_DONE=" + std::to_string(SQLITE_DONE) +
                " SQLITE_ROW=" + std::to_string(SQLITE_ROW) + ")");

    LOG_INFO_PI("✅ History import complete: " + std::to_string(result.historyImported) +
               " entries, " + std::to_string(result.skipped) + " skipped");

    return result;
}

// ============================================================================
// Combined Import
// ============================================================================

ImportResult ProfileImporter::ImportAll(
    const std::string& profilePath,
    int maxHistoryEntries,
    ImportProgressCallback progress
) {
    ImportResult result = {true, "", 0, 0, 0, 0};

    // Import bookmarks first
    if (progress) progress("starting", 0, 2, "Importing bookmarks...");
    ImportResult bookmarkResult = ImportBookmarks(profilePath, progress);

    result.bookmarksImported = bookmarkResult.bookmarksImported;
    result.foldersImported = bookmarkResult.foldersImported;
    result.skipped += bookmarkResult.skipped;

    if (!bookmarkResult.success) {
        result.error = "Bookmark import: " + bookmarkResult.error;
        // Continue with history even if bookmarks failed
    }

    // Import history
    if (progress) progress("starting", 1, 2, "Importing history...");
    ImportResult historyResult = ImportHistory(profilePath, maxHistoryEntries, progress);

    result.historyImported = historyResult.historyImported;
    result.skipped += historyResult.skipped;

    if (!historyResult.success) {
        if (!result.error.empty()) result.error += "; ";
        result.error += "History import: " + historyResult.error;
    }

    result.success = bookmarkResult.success || historyResult.success;

    return result;
}

// ============================================================================
// JSON Serialization
// ============================================================================

std::string ProfileImporter::ResultToJson(const ImportResult& result) {
    json j;
    j["success"] = result.success;
    j["error"] = result.error;
    j["bookmarksImported"] = result.bookmarksImported;
    j["foldersImported"] = result.foldersImported;
    j["historyImported"] = result.historyImported;
    j["skipped"] = result.skipped;
    return j.dump();
}

std::string ProfileImporter::ProfilesToJson(const std::vector<DetectedProfile>& profiles) {
    json j = json::array();
    for (const auto& p : profiles) {
        json profile;
        profile["browserName"] = p.browserName;
        profile["profilePath"] = p.profilePath;
        profile["profileName"] = p.profileName;
        profile["hasBookmarks"] = p.hasBookmarks;
        profile["hasHistory"] = p.hasHistory;
        profile["bookmarkCount"] = p.bookmarkCount;
        profile["historyCount"] = p.historyCount;
        j.push_back(profile);
    }
    return j.dump();
}
