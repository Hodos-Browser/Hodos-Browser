#pragma once

#include <sqlite3.h>

#include <atomic>
#include <cstdint>
#include <map>
#include <mutex>
#include <optional>
#include <string>
#include <vector>

#include "include/cef_response.h"

// Cached paid HTTP response — bytes already paid for via BRC-121.
// Replayed by CachedContentResourceHandler when the same URL is re-requested
// before expiry. Total store is capped at TOTAL_SIZE_LIMIT_BYTES via LRU
// eviction on last_access.
struct PaidContentEntry {
    std::string url;
    int status = 200;
    std::map<std::string, std::string> headers;  // case-insensitive map serialized to JSON
    std::vector<uint8_t> body;
    int64_t paid_at_ms = 0;     // ms since epoch
    int64_t last_access_ms = 0; // ms since epoch (LRU key)
    std::optional<int64_t> expires_at_ms;  // empty = forever-with-size-cap
};

class PaidContentCache {
public:
    static PaidContentCache& GetInstance();

    // Open / create the SQLite store at <profile>/paid_content_cache.db.
    // Safe to call multiple times; only the first call opens.
    bool Initialize(const std::string& user_data_path);

    bool IsInitialized() const { return db_ != nullptr; }

    // Toggle (mirrors PrivacySettings.paidContentCacheEnabled). Defaults true;
    // Set from SettingsManager at startup and on user toggle.
    void SetEnabled(bool enabled) { enabled_.store(enabled); }
    bool IsEnabled() const { return enabled_.load(); }

    // Look up a cached entry. Returns true if hit AND not expired; updates
    // last_access on hit. Returns false on miss, expired, or DB error.
    bool Get(const std::string& url, PaidContentEntry& out);

    // Store a paid response. Best-effort: failures are logged and swallowed
    // so a cache write can never break the green-dot animation or session
    // accounting in the calling Async402ResourceHandler. expires_at_ms NULL
    // means forever-with-size-cap.
    void Put(const std::string& url,
             int status,
             const CefResponse::HeaderMap& headers,
             const std::vector<uint8_t>& body,
             std::optional<int64_t> expires_at_ms);

    // Wipe all cached entries (Clear browsing data → Paid content row).
    void Clear();

    // Total bytes currently stored (sum of byte_size column). For UI display.
    int64_t GetTotalSize();

    // Parse Cache-Control: max-age=N from a HeaderMap. Returns now+N*1000 if
    // present and parseable; nullopt otherwise.
    static std::optional<int64_t> ParseCacheControl(
        const CefResponse::HeaderMap& headers);

    // 500 MB LRU cap.
    static constexpr int64_t TOTAL_SIZE_LIMIT_BYTES = 500LL * 1024 * 1024;

private:
    PaidContentCache() = default;
    ~PaidContentCache();

    PaidContentCache(const PaidContentCache&) = delete;
    PaidContentCache& operator=(const PaidContentCache&) = delete;

    bool OpenDatabase();
    void CloseDatabase();
    bool EnsureSchema();

    // Called inside Put after insert; deletes oldest-by-last_access rows
    // until total size is back under TOTAL_SIZE_LIMIT_BYTES.
    void EvictIfOverCap();

    sqlite3* db_ = nullptr;
    std::string db_path_;
    std::mutex mutex_;  // guards db_
    std::atomic<bool> enabled_{true};
};
