#pragma once

#include <string>
#include <vector>
#include <functional>

// Info about a detected browser profile
struct DetectedProfile {
    std::string browserName;    // "Chrome", "Brave", "Edge", "Firefox"
    std::string profilePath;    // Full path to profile directory
    std::string profileName;    // "Default", "Profile 1", etc.
    bool hasBookmarks;
    bool hasHistory;
    int bookmarkCount;          // Approximate count
    int historyCount;           // Approximate count
};

// Import result
struct ImportResult {
    bool success;
    std::string error;
    int bookmarksImported;
    int foldersImported;
    int historyImported;
    int skipped;                // Duplicates or invalid entries
};

// Progress callback: (phase, current, total, message)
using ImportProgressCallback = std::function<void(const std::string&, int, int, const std::string&)>;

class ProfileImporter {
public:
    // Detect all available browser profiles on the system
    static std::vector<DetectedProfile> DetectProfiles();
    
    // Import bookmarks from a profile path
    // Returns result with counts and any error message
    static ImportResult ImportBookmarks(
        const std::string& profilePath,
        ImportProgressCallback progress = nullptr
    );
    
    // Import history from a profile path
    // maxEntries: limit to prevent importing years of history (default 10000)
    static ImportResult ImportHistory(
        const std::string& profilePath,
        int maxEntries = 10000,
        ImportProgressCallback progress = nullptr
    );
    
    // Import both bookmarks and history
    static ImportResult ImportAll(
        const std::string& profilePath,
        int maxHistoryEntries = 10000,
        ImportProgressCallback progress = nullptr
    );

    // Convert results to JSON string (for IPC)
    static std::string ResultToJson(const ImportResult& result);
    static std::string ProfilesToJson(const std::vector<DetectedProfile>& profiles);

private:
    // Get default profile paths for each browser
    static std::string GetChromeProfilePath();
    static std::string GetBraveProfilePath();
    static std::string GetEdgeProfilePath();
    static std::string GetFirefoxProfilePath();
    
    // Count items in a profile (quick scan without full import)
    static int CountBookmarksInFile(const std::string& bookmarksPath);
    static int CountHistoryInFile(const std::string& historyPath);
    
    // Internal import helpers
    static void ImportBookmarkNode(
        const std::string& nodeJson,
        int parentFolderId,
        ImportResult& result,
        ImportProgressCallback progress
    );
    
    // Copy file (needed because browsers lock their DBs)
    // Named CopyFilePortable to avoid Windows CopyFile macro conflict
    static bool CopyFilePortable(const std::string& src, const std::string& dst);
    static std::string GetTempFilePath(const std::string& filename);
};
