#pragma once

#include <string>
#include <mutex>
#include <functional>
#include <nlohmann/json.hpp>

// Browser settings (general browsing behavior)
struct BrowserSettings {
    std::string homepage = "about:blank";
    std::string searchEngine = "google";  // google, duckduckgo
    double zoomLevel = 0.0;
    bool showBookmarkBar = false;
    std::string downloadsPath;  // Empty = system default
    bool restoreSessionOnStart = false;
    bool askWhereToSave = true;
    std::string autoUpdateMode = "silent";  // "off", "notify", or "silent"
};

// Privacy settings (ad blocking, tracking, etc.)
struct PrivacySettings {
    bool adBlockEnabled = true;
    bool thirdPartyCookieBlocking = true;
    bool doNotTrack = false;
    bool clearDataOnExit = false;
    bool fingerprintProtection = true;  // Sprint 12e
    bool paidContentCacheEnabled = true;  // Phase 1 BRC-121 (Paid Content Cache)
};

// Wallet settings (auto-approve, spending limits, PeerPay)
struct WalletSettings {
    bool autoApproveEnabled = true;
    int defaultPerTxLimitCents = 100;        // $1.00 per transaction
    int defaultPerSessionLimitCents = 1000;  // $10.00 per session
    int defaultRateLimitPerMin = 30;
    int defaultMaxTxPerSession = 100;        // max transactions per session
    bool peerpayAutoAccept = true;           // Auto-accept incoming PeerPay payments
};

// Custom JSON serialization for BrowserSettings (handles legacy bool migration)
inline void to_json(nlohmann::json& j, const BrowserSettings& s) {
    j = nlohmann::json{
        {"homepage", s.homepage}, {"searchEngine", s.searchEngine},
        {"zoomLevel", s.zoomLevel}, {"showBookmarkBar", s.showBookmarkBar},
        {"downloadsPath", s.downloadsPath}, {"restoreSessionOnStart", s.restoreSessionOnStart},
        {"askWhereToSave", s.askWhereToSave}, {"autoUpdateMode", s.autoUpdateMode}
    };
}

inline void from_json(const nlohmann::json& j, BrowserSettings& s) {
    BrowserSettings defaults;
    s.homepage = j.value("homepage", defaults.homepage);
    s.searchEngine = j.value("searchEngine", defaults.searchEngine);
    s.zoomLevel = j.value("zoomLevel", defaults.zoomLevel);
    s.showBookmarkBar = j.value("showBookmarkBar", defaults.showBookmarkBar);
    s.downloadsPath = j.value("downloadsPath", defaults.downloadsPath);
    s.restoreSessionOnStart = j.value("restoreSessionOnStart", defaults.restoreSessionOnStart);
    s.askWhereToSave = j.value("askWhereToSave", defaults.askWhereToSave);
    // Backward compat: migrate legacy bool autoUpdateEnabled → string autoUpdateMode.
    // NOTE: legacy `true` maps to "notify", NOT "silent": the old bool only ever meant
    // notify-era updates (silent auto-apply did not exist), so promoting those users to
    // silent would auto-apply updates they never consented to. Fresh installs still
    // default to "silent" (the struct default); only this legacy-upgrade path is notify.
    if (j.contains("autoUpdateMode")) {
        s.autoUpdateMode = j.value("autoUpdateMode", defaults.autoUpdateMode);
    } else if (j.contains("autoUpdateEnabled")) {
        bool legacy = j.value("autoUpdateEnabled", true);
        s.autoUpdateMode = legacy ? "notify" : "off";
    }
    // Validate
    if (s.autoUpdateMode != "off" && s.autoUpdateMode != "notify" && s.autoUpdateMode != "silent") {
        s.autoUpdateMode = defaults.autoUpdateMode;
    }
}

NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE_WITH_DEFAULT(PrivacySettings,
    adBlockEnabled, thirdPartyCookieBlocking, doNotTrack, clearDataOnExit,
    fingerprintProtection, paidContentCacheEnabled)

NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE_WITH_DEFAULT(WalletSettings,
    autoApproveEnabled, defaultPerTxLimitCents,
    defaultPerSessionLimitCents, defaultRateLimitPerMin,
    defaultMaxTxPerSession, peerpayAutoAccept)

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
    void SetAskWhereToSave(bool ask);
    void SetAutoUpdateMode(const std::string& mode);

    // Privacy settings
    void SetAdBlockEnabled(bool enabled);
    void SetThirdPartyCookieBlocking(bool enabled);
    void SetDoNotTrack(bool enabled);
    void SetClearDataOnExit(bool clear);
    void SetFingerprintProtection(bool enabled);
    void SetPaidContentCacheEnabled(bool enabled);

    // Wallet settings
    void SetAutoApproveEnabled(bool enabled);
    void SetDefaultPerTxLimitCents(int cents);
    void SetDefaultPerSessionLimitCents(int cents);
    void SetDefaultRateLimitPerMin(int rate);
    void SetDefaultMaxTxPerSession(int maxTx);
    void SetPeerpayAutoAccept(bool enabled);

    // Bulk update from JSON (for IPC from frontend)
    bool UpdateFromJson(const std::string& jsonStr);

    // --- Global (cross-profile) update mode (auto-update setting) ---
    // autoUpdateMode is machine/user-GLOBAL (Chrome's model), not per-profile: it is
    // sourced from and persisted to the global settings.json, so a change in any profile
    // applies to all. See AUTOUPDATE_SILENT_STATE_WRITER_DESIGN.md.

    // Register a callback invoked (with the validated mode string) whenever the update
    // mode CHANGES via SetAutoUpdateMode / UpdateFromJson. The shell wires this to the
    // silent-eligibility mirror (Windows + HODOS_SILENT_AUTOUPDATE only). Not called on
    // load. Thread note: invoked OUTSIDE mutex_.
    void SetUpdateModeChangeCallback(std::function<void(const std::string&)> cb);

    // True iff LoadInternal found NO global updateMode (first run under the global-mode
    // scheme) — the shell then does a one-time MOST-CONSERVATIVE collapse across profiles.
    bool GlobalUpdateModeWasAbsentAtLoad() const;

    // Set the authoritative global mode (persist global + set in memory). Used by the
    // shell's one-time conservative collapse. Does NOT invoke the change callback.
    void SetGlobalUpdateModeAuthoritative(const std::string& mode);

    // Read a specific profile's stored autoUpdateMode (applying legacy migration) from
    // <profilePath>/settings.json. Returns "" if absent/unreadable. Static — used by the
    // one-time collapse to find the most-conservative value across all profiles.
    static std::string ReadModeFromProfileSettings(const std::string& profilePath);

private:
    SettingsManager() = default;
    ~SettingsManager() = default;

    std::string GetGlobalSettingsFilePath() const;
    std::string GetActiveSettingsFilePath() const;
    void EnsureDirectoryExists(const std::string& path) const;
    void LoadInternal();  // Actual load logic (no mutex)

    // Global update mode helpers (operate on the global settings.json root "updateMode"
    // key). Read returns "" if absent/invalid. Persist is a read-modify-write of ONLY
    // that key (never clobbers other global settings). Neither takes mutex_.
    std::string LoadGlobalUpdateMode() const;
    bool PersistGlobalUpdateMode(const std::string& mode);

    mutable std::mutex mutex_;
    BrowserSettings browser_;
    PrivacySettings privacy_;
    WalletSettings wallet_;
    int version_ = 1;

    std::string settings_file_path_;  // Active settings path — GLOBAL app-data store (set by Initialize)
    bool initialized_ = false;

    std::function<void(const std::string&)> update_mode_change_cb_;  // -> silent mirror
    bool global_update_mode_absent_at_load_ = false;                 // first-run collapse flag

    // Prevent copying
    SettingsManager(const SettingsManager&) = delete;
    SettingsManager& operator=(const SettingsManager&) = delete;
};
