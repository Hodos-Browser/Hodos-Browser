#pragma once

#include <sqlite3.h>
#include <string>
#include <vector>
#include <chrono>

struct HistoryEntry {
    int64_t id;
    std::string url;
    std::string title;
    int visit_count;
    int64_t last_visit_time;
    int64_t visit_time;
    int transition;
};

struct HistorySearchParams {
    std::string search_term;
    int64_t start_time;
    int64_t end_time;
    int limit;
    int offset;
};

class HistoryManager {
public:
    static HistoryManager& GetInstance();

    // Initialize with CEF user data path - creates our own History database
    bool Initialize(const std::string& user_data_path);

    // Add visit (called when a page loads successfully)
    bool AddVisit(const std::string& url, const std::string& title, int transition_type = 0);

    // Query our History database
    std::vector<HistoryEntry> GetHistory(int limit, int offset);
    std::vector<HistoryEntry> SearchHistory(const HistorySearchParams& params);
    HistoryEntry GetHistoryEntryByUrl(const std::string& url);

    // Test function to debug schema
    std::vector<HistoryEntry> GetHistorySimple(int limit);

    // Delete operations
    bool DeleteHistoryEntry(const std::string& url);
    bool DeleteAllHistory();
    bool DeleteHistoryRange(int64_t start_time, int64_t end_time);

    // Utility functions
    static int64_t GetCurrentChromiumTime();
    static int64_t ChromiumTimeToUnix(int64_t chromium_time);
    static int64_t UnixToChromiumTime(int64_t unix_time);

    // Check if initialized
    bool IsInitialized() const { return history_db_ != nullptr; }

private:
    HistoryManager() = default;
    ~HistoryManager();

    sqlite3* history_db_;
    std::string history_db_path_;

    bool OpenDatabase();
    void CloseDatabase();

    // Prevent copying
    HistoryManager(const HistoryManager&) = delete;
    HistoryManager& operator=(const HistoryManager&) = delete;
};
