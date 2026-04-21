#pragma once

#include <sqlite3.h>
#include <string>
#include <vector>
#include <cstdint>

struct BookmarkData {
    int64_t id;
    std::string url;
    std::string title;
    int folder_id;        // -1 means root (no folder)
    std::string favicon_url;
    int position;
    int64_t created_at;
    int64_t updated_at;
    int64_t last_accessed;
    std::vector<std::string> tags;
};

struct FolderData {
    int64_t id;
    std::string name;
    int parent_id;        // -1 means root (no parent)
    int position;
    int64_t created_at;
    int64_t updated_at;
};

class BookmarkManager {
public:
    static BookmarkManager& GetInstance();

    // Initialize with CEF user data path - creates bookmarks.db
    bool Initialize(const std::string& user_data_path);

    // Check if initialized
    bool IsInitialized() const { return db_ != nullptr; }

    // ========================================================================
    // Bookmark CRUD methods (all return JSON string responses)
    // ========================================================================

    // Add a bookmark. Returns {"success": true, "id": N} or {"success": false, "error": "..."}
    std::string AddBookmark(const std::string& url,
                            const std::string& title,
                            int folder_id,
                            const std::vector<std::string>& tags);

    // Get a single bookmark by ID. Returns full bookmark JSON with tags.
    std::string GetBookmark(int64_t id);

    // Update bookmark fields and replace tags.
    std::string UpdateBookmark(int64_t id,
                               const std::string& title,
                               const std::string& url,
                               int folder_id,
                               const std::vector<std::string>& tags);

    // Remove a bookmark by ID. CASCADE handles tag deletion.
    std::string RemoveBookmark(int64_t id);

    // Search bookmarks by substring across title, URL, and tags.
    // Ordered by last_accessed DESC. Returns {"bookmarks": [...], "total": N}
    std::string SearchBookmarks(const std::string& query, int limit, int offset);

    // Get bookmarks in a folder (-1 for all). Returns {"bookmarks": [...], "total": N}
    std::string GetAllBookmarks(int folder_id, int limit, int offset);

    // Check if a URL is bookmarked. Returns {"bookmarked": true/false}
    std::string IsBookmarked(const std::string& url);

    // Update last_accessed timestamp to now.
    std::string UpdateLastAccessed(int64_t id);

    // Get all distinct tags. Returns JSON array of strings.
    std::string GetAllTags();

    // ========================================================================
    // Folder CRUD methods (all return JSON string responses)
    // ========================================================================

    // Create a folder. Depth capped at 3 levels (root=0, sub=1, subsub=2).
    // Returns {"success": true, "id": N} or {"success": false, "error": "..."}
    std::string CreateFolder(const std::string& name, int parent_id);

    // List direct children folders of parent (-1 for root). Returns {"folders": [...]}
    std::string ListFolders(int parent_id);

    // Rename a folder.
    std::string UpdateFolder(int64_t id, const std::string& name);

    // Delete a folder. CASCADE deletes children, bookmarks, and tags.
    std::string RemoveFolder(int64_t id);

    // Get full folder tree as nested JSON array.
    std::string GetFolderTree();

private:
    BookmarkManager() = default;
    ~BookmarkManager();

    sqlite3* db_ = nullptr;
    std::string db_path_;

    // Database operations
    bool OpenDatabase();
    void CloseDatabase();

    // Helper: walk parent chain to get folder depth (root folders = depth 0)
    int GetFolderDepth(int folder_id);

    // Helper: serialize a bookmark row from prepared statement + fetch its tags
    std::string BookmarkToJson(int64_t bookmark_id);

    // Prevent copying
    BookmarkManager(const BookmarkManager&) = delete;
    BookmarkManager& operator=(const BookmarkManager&) = delete;
};
