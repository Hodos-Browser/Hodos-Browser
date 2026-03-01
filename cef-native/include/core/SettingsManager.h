#pragma once

#include <string>
#include <mutex>
#include <nlohmann/json.hpp>

// Browser settings (general browsing behavior)
struct BrowserSettings {
    std::string homepage = "about:blank";
    std::string searchEngine = "google";  // google, bing, duckduckgo, brave
    double zoomLevel = 0.0;
    bool showBookmarkBar = false;
    std::string downloadsPath;  // Empty = system default
    bool restoreSessionOnStart = false;
};

// Privacy settings (ad blocking, tracking, etc.)
struct PrivacySettings {
    bool adBlockEnabled = true;
    bool thirdPartyCookieBlocking = true;
    bool doNotTrack = false;
    bool clearDataOnExit = false;
    bool fingerprintProtection = true;  // Sprint 12e
};

// Wallet settings (auto-approve, spending limits)
struct WalletSettings {
    bool autoApproveEnabled = true;
    int defaultPerTxLimitCents = 10;       // $0.10 per transaction
    int defaultPerSessionLimitCents = 300;  // $3.00 per session
    int defaultRateLimitPerMin = 10;
};

// JSON serialization
NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE_WITH_DEFAULT(BrowserSettings,
    homepage, searchEngine, zoomLevel, showBookmarkBar, 
    downloadsPath, restoreSessionOnStart)

NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE_WITH_DEFAULT(PrivacySettings,
    adBlockEnabled, thirdPartyCookieBlocking, doNotTrack, clearDataOnExit,
    fingerprintProtection)

NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE_WITH_DEFAULT(WalletSettings,
    autoApproveEnabled, defaultPerTxLimitCents, 
    defaultPerSessionLimitCents, defaultRateLimitPerMin)

class SettingsManager {
public:
    static SettingsManager& GetInstance();

    // Initialize with profile-specific path (call before Load)
    void Initialize(const std::string& profile_path);

    // Lifecycle
    void Load();
    void Save();

    // Getters (thread-safe)
    BrowserSettings GetBrowserSettings() const;
    PrivacySettings GetPrivacySettings() const;
    WalletSettings GetWalletSettings() const;

    // Get all settings as JSON string (for IPC to frontend)
    std::string ToJson() const;

    // Individual setters (auto-save after change)
    // Browser settings
    void SetHomepage(const std::string& url);
    void SetSearchEngine(const std::string& engine);
    void SetZoomLevel(double level);
    void SetShowBookmarkBar(bool show);
    void SetDownloadsPath(const std::string& path);
    void SetRestoreSessionOnStart(bool restore);

    // Privacy settings
    void SetAdBlockEnabled(bool enabled);
    void SetThirdPartyCookieBlocking(bool enabled);
    void SetDoNotTrack(bool enabled);
    void SetClearDataOnExit(bool clear);
    void SetFingerprintProtection(bool enabled);

    // Wallet settings
    void SetAutoApproveEnabled(bool enabled);
    void SetDefaultPerTxLimitCents(int cents);
    void SetDefaultPerSessionLimitCents(int cents);
    void SetDefaultRateLimitPerMin(int rate);

    // Bulk update from JSON (for IPC from frontend)
    bool UpdateFromJson(const std::string& jsonStr);

private:
    SettingsManager() = default;
    ~SettingsManager() = default;

    std::string GetGlobalSettingsFilePath() const;
    std::string GetActiveSettingsFilePath() const;
    void EnsureDirectoryExists(const std::string& path) const;
    void LoadInternal();  // Actual load logic (no mutex)

    mutable std::mutex mutex_;
    BrowserSettings browser_;
    PrivacySettings privacy_;
    WalletSettings wallet_;
    int version_ = 1;

    std::string settings_file_path_;  // Per-profile path (set by Initialize)
    bool initialized_ = false;

    // Prevent copying
    SettingsManager(const SettingsManager&) = delete;
    SettingsManager& operator=(const SettingsManager&) = delete;
};
