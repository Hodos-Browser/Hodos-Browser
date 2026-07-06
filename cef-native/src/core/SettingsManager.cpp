#include "../../include/core/SettingsManager.h"
#include "../../include/core/Logger.h"
#include "../../include/core/AppPaths.h"
#include "../../include/core/PaidContentCache.h"
#include <fstream>
#include <cstdlib>
#include <filesystem>
#include <utility>

// Local logging macros (module ID 10 = SettingsManager)
#define LOG_INFO_SM(msg) Logger::Log(msg, 1, 10)
#define LOG_DEBUG_SM(msg) Logger::Log(msg, 0, 10)
#define LOG_ERROR_SM(msg) Logger::Log(msg, 3, 10)
#define LOG_WARNING_SM(msg) Logger::Log(msg, 2, 10)

#ifdef _WIN32
#include <windows.h>
#include <shlobj.h>
#else
#include <unistd.h>
#include <sys/stat.h>
#endif

namespace fs = std::filesystem;

SettingsManager& SettingsManager::GetInstance() {
    static SettingsManager instance;
    return instance;
}

std::string SettingsManager::GetGlobalSettingsFilePath() const {
#ifdef _WIN32
    const char* appdata = std::getenv("APPDATA");
    if (appdata) {
        return std::string(appdata) + "\\" + AppPaths::GetAppDirName() + "\\settings.json";
    }
    return "settings.json";
#elif defined(__APPLE__)
    const char* home = std::getenv("HOME");
    if (home) {
        return std::string(home) + "/Library/Application Support/" + AppPaths::GetAppDirName() + "/settings.json";
    }
    return "settings.json";
#else
    // Linux fallback
    const char* home = std::getenv("HOME");
    if (home) {
        return std::string(home) + "/.config/" + AppPaths::GetAppDirName() + "/settings.json";
    }
    return "settings.json";
#endif
}

std::string SettingsManager::GetActiveSettingsFilePath() const {
    if (initialized_ && !settings_file_path_.empty()) {
        return settings_file_path_;
    }
    return GetGlobalSettingsFilePath();
}

void SettingsManager::Initialize(const std::string& profile_path) {
    std::lock_guard<std::mutex> lock(mutex_);

#ifdef _WIN32
    settings_file_path_ = profile_path + "\\settings.json";
#else
    settings_file_path_ = profile_path + "/settings.json";
#endif

    LOG_INFO_SM("Initializing SettingsManager with profile path: " + settings_file_path_);

    // Migration: if profile settings.json doesn't exist but global one does, copy it
    if (!fs::exists(settings_file_path_)) {
        std::string globalPath = GetGlobalSettingsFilePath();
        if (fs::exists(globalPath)) {
            try {
                EnsureDirectoryExists(settings_file_path_);
                fs::copy_file(globalPath, settings_file_path_);
                LOG_INFO_SM("Migrated global settings.json to profile: " + settings_file_path_);
            } catch (const std::exception& e) {
                LOG_WARNING_SM("Failed to migrate global settings: " + std::string(e.what()));
            }
        }
    }

    initialized_ = true;
    LoadInternal();
}

void SettingsManager::EnsureDirectoryExists(const std::string& filePath) const {
    fs::path path(filePath);
    fs::path dir = path.parent_path();
    if (!dir.empty() && !fs::exists(dir)) {
        try {
            fs::create_directories(dir);
        } catch (const std::exception& e) {
            LOG_ERROR_SM("Failed to create settings directory: " + std::string(e.what()));
        }
    }
}

void SettingsManager::Load() {
    std::lock_guard<std::mutex> lock(mutex_);
    LoadInternal();
}

void SettingsManager::LoadInternal() {
    std::string filePath = GetActiveSettingsFilePath();
    LOG_INFO_SM("Loading settings from: " + filePath);

    std::ifstream file(filePath);
    if (file.is_open()) {
        try {
            nlohmann::json j;
            file >> j;

            // Read version (for future migrations)
            if (j.contains("version")) {
                version_ = j["version"].get<int>();
            }

            // Read settings sections
            if (j.contains("browser")) {
                browser_ = j["browser"].get<BrowserSettings>();
            }
            if (j.contains("privacy")) {
                privacy_ = j["privacy"].get<PrivacySettings>();
            }
            if (j.contains("wallet")) {
                wallet_ = j["wallet"].get<WalletSettings>();
            }

            LOG_INFO_SM("Settings loaded successfully (version " + std::to_string(version_) + ")");
        } catch (const nlohmann::json::exception& e) {
            LOG_ERROR_SM("Failed to parse settings.json: " + std::string(e.what()));
            LOG_INFO_SM("Using default settings");
            // Reset to defaults
            browser_ = BrowserSettings();
            privacy_ = PrivacySettings();
            wallet_ = WalletSettings();
        }
    } else {
        LOG_INFO_SM("No settings file found, using defaults");
    }

    // autoUpdateMode is machine/user-GLOBAL (see AUTOUPDATE_SILENT_STATE_WRITER_DESIGN.md).
    // The global settings.json is authoritative. If it has a value, it overrides the
    // per-profile one so every profile agrees; if it does not, this is the first run under
    // the global scheme — seed the global from this profile's (post-legacy-migration)
    // value and flag the shell to do the one-time MOST-CONSERVATIVE collapse across all
    // profiles (so an explicit notify/off in any profile is never promoted to silent).
    std::string gm = LoadGlobalUpdateMode();
    if (!gm.empty()) {
        browser_.autoUpdateMode = gm;
        global_update_mode_absent_at_load_ = false;
    } else {
        global_update_mode_absent_at_load_ = true;
        PersistGlobalUpdateMode(browser_.autoUpdateMode);
    }
}

void SettingsManager::Save() {
    std::lock_guard<std::mutex> lock(mutex_);

    std::string filePath = GetActiveSettingsFilePath();
    EnsureDirectoryExists(filePath);
    
    nlohmann::json j;
    j["version"] = version_;
    j["browser"] = browser_;
    j["privacy"] = privacy_;
    j["wallet"] = wallet_;
    
    std::ofstream file(filePath);
    if (!file.is_open()) {
        LOG_ERROR_SM("❌ Failed to open settings file for writing: " + filePath);
        return;
    }
    
    try {
        file << j.dump(2);  // Pretty print with 2-space indent
        LOG_DEBUG_SM("💾 Settings saved to: " + filePath);
    } catch (const std::exception& e) {
        LOG_ERROR_SM("❌ Failed to write settings: " + std::string(e.what()));
    }
}

// Getters
BrowserSettings SettingsManager::GetBrowserSettings() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return browser_;
}

PrivacySettings SettingsManager::GetPrivacySettings() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return privacy_;
}

WalletSettings SettingsManager::GetWalletSettings() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return wallet_;
}

std::string SettingsManager::ToJson() const {
    std::lock_guard<std::mutex> lock(mutex_);
    
    nlohmann::json j;
    j["version"] = version_;
    j["browser"] = browser_;
    j["privacy"] = privacy_;
    j["wallet"] = wallet_;
    
    return j.dump();
}

// Browser settings setters
void SettingsManager::SetHomepage(const std::string& url) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        browser_.homepage = url;
    }
    Save();
}

void SettingsManager::SetSearchEngine(const std::string& engine) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        browser_.searchEngine = engine;
    }
    Save();
}

void SettingsManager::SetZoomLevel(double level) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        browser_.zoomLevel = level;
    }
    Save();
}

void SettingsManager::SetShowBookmarkBar(bool show) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        browser_.showBookmarkBar = show;
    }
    Save();
}

void SettingsManager::SetDownloadsPath(const std::string& path) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        browser_.downloadsPath = path;
    }
    Save();
}

void SettingsManager::SetRestoreSessionOnStart(bool restore) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        browser_.restoreSessionOnStart = restore;
    }
    Save();
}

void SettingsManager::SetAskWhereToSave(bool ask) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        browser_.askWhereToSave = ask;
    }
    Save();
}

void SettingsManager::SetAutoUpdateMode(const std::string& mode) {
    std::string validated = mode;
    if (validated != "off" && validated != "notify" && validated != "silent") {
        validated = "silent";
    }
    {
        std::lock_guard<std::mutex> lock(mutex_);
        browser_.autoUpdateMode = validated;
    }
    // autoUpdateMode is GLOBAL: persist to the global settings.json so every profile sees
    // the change. Per-profile Save() keeps the (display-only) per-profile copy consistent.
    PersistGlobalUpdateMode(validated);
    Save();
    // Bridge to the silent-eligibility mirror (Windows + HODOS_SILENT_AUTOUPDATE only).
    if (update_mode_change_cb_) update_mode_change_cb_(validated);
}

// Privacy settings setters
void SettingsManager::SetAdBlockEnabled(bool enabled) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        privacy_.adBlockEnabled = enabled;
    }
    Save();
}

void SettingsManager::SetThirdPartyCookieBlocking(bool enabled) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        privacy_.thirdPartyCookieBlocking = enabled;
    }
    Save();
}

void SettingsManager::SetDoNotTrack(bool enabled) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        privacy_.doNotTrack = enabled;
    }
    Save();
}

void SettingsManager::SetClearDataOnExit(bool clear) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        privacy_.clearDataOnExit = clear;
    }
    Save();
}

void SettingsManager::SetFingerprintProtection(bool enabled) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        privacy_.fingerprintProtection = enabled;
    }
    Save();
}

void SettingsManager::SetPaidContentCacheEnabled(bool enabled) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        privacy_.paidContentCacheEnabled = enabled;
    }
    Save();
    // Sync the live PaidContentCache singleton so the change takes effect
    // immediately without requiring a restart. Forward declared in header.
    PaidContentCache::GetInstance().SetEnabled(enabled);
}

// Wallet settings setters
void SettingsManager::SetAutoApproveEnabled(bool enabled) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        wallet_.autoApproveEnabled = enabled;
    }
    Save();
}

void SettingsManager::SetDefaultPerTxLimitCents(int cents) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        wallet_.defaultPerTxLimitCents = cents;
    }
    Save();
}

void SettingsManager::SetDefaultPerSessionLimitCents(int cents) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        wallet_.defaultPerSessionLimitCents = cents;
    }
    Save();
}

void SettingsManager::SetDefaultRateLimitPerMin(int rate) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        wallet_.defaultRateLimitPerMin = rate;
    }
    Save();
}

void SettingsManager::SetDefaultMaxTxPerSession(int maxTx) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        wallet_.defaultMaxTxPerSession = maxTx;
    }
    Save();
}

void SettingsManager::SetPeerpayAutoAccept(bool enabled) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        wallet_.peerpayAutoAccept = enabled;
    }
    Save();
}

// Bulk update from JSON
bool SettingsManager::UpdateFromJson(const std::string& jsonStr) {
    try {
        nlohmann::json j = nlohmann::json::parse(jsonStr);

        std::string oldMode, newMode;
        bool modeChanged = false;
        {
            std::lock_guard<std::mutex> lock(mutex_);

            oldMode = browser_.autoUpdateMode;
            if (j.contains("browser")) {
                // R9: if the incoming browser object OMITS autoUpdateMode, from_json would
                // reset it to the "silent" struct default — which could silently flip a
                // notify user on an unrelated bulk save. Preserve the current value instead.
                bool hadMode = j["browser"].is_object() && j["browser"].contains("autoUpdateMode");
                browser_ = j["browser"].get<BrowserSettings>();
                if (!hadMode) browser_.autoUpdateMode = oldMode;
            }
            if (j.contains("privacy")) {
                privacy_ = j["privacy"].get<PrivacySettings>();
            }
            if (j.contains("wallet")) {
                wallet_ = j["wallet"].get<WalletSettings>();
            }
            newMode = browser_.autoUpdateMode;
            modeChanged = (newMode != oldMode);
        }

        Save();
        // autoUpdateMode is GLOBAL — persist + bridge to the mirror only when it changed.
        if (modeChanged) {
            PersistGlobalUpdateMode(newMode);
            if (update_mode_change_cb_) update_mode_change_cb_(newMode);
        }
        return true;
    } catch (const std::exception& e) {
        LOG_ERROR_SM("❌ Failed to update settings from JSON: " + std::string(e.what()));
        return false;
    }
}

// --- Global (cross-profile) update mode ---------------------------------------

namespace {
bool IsValidUpdateMode(const std::string& m) {
    return m == "off" || m == "notify" || m == "silent";
}
}  // namespace

std::string SettingsManager::LoadGlobalUpdateMode() const {
    std::string path = GetGlobalSettingsFilePath();
    std::ifstream f(path);
    if (!f.is_open()) return "";
    try {
        nlohmann::json j;
        f >> j;
        if (j.contains("updateMode") && j["updateMode"].is_string()) {
            std::string m = j["updateMode"].get<std::string>();
            if (IsValidUpdateMode(m)) return m;
        }
    } catch (const std::exception&) {
        // Corrupt/partial global file -> treat as absent (fall through to "").
    }
    return "";
}

bool SettingsManager::PersistGlobalUpdateMode(const std::string& mode) {
    if (!IsValidUpdateMode(mode)) return false;
    std::string path = GetGlobalSettingsFilePath();
    EnsureDirectoryExists(path);

    // Read-modify-write: preserve every OTHER key in the global settings.json (it may
    // also hold the pre-per-profile-split settings). Only touch the "updateMode" key.
    nlohmann::json j = nlohmann::json::object();
    {
        std::ifstream f(path);
        if (f.is_open()) {
            try { f >> j; } catch (const std::exception&) { j = nlohmann::json::object(); }
        }
    }
    if (!j.is_object()) j = nlohmann::json::object();
    j["updateMode"] = mode;

    std::ofstream out(path);
    if (!out.is_open()) {
        LOG_ERROR_SM("❌ Failed to open global settings for updateMode write: " + path);
        return false;
    }
    try {
        out << j.dump(2);
    } catch (const std::exception& e) {
        LOG_ERROR_SM("❌ Failed to write global updateMode: " + std::string(e.what()));
        return false;
    }
    LOG_DEBUG_SM("💾 Global updateMode persisted: " + mode);
    return true;
}

void SettingsManager::SetUpdateModeChangeCallback(std::function<void(const std::string&)> cb) {
    std::lock_guard<std::mutex> lock(mutex_);
    update_mode_change_cb_ = std::move(cb);
}

bool SettingsManager::GlobalUpdateModeWasAbsentAtLoad() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return global_update_mode_absent_at_load_;
}

void SettingsManager::SetGlobalUpdateModeAuthoritative(const std::string& mode) {
    if (!IsValidUpdateMode(mode)) return;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        browser_.autoUpdateMode = mode;
    }
    PersistGlobalUpdateMode(mode);
}

std::string SettingsManager::ReadModeFromProfileSettings(const std::string& profilePath) {
    if (profilePath.empty()) return "";
#ifdef _WIN32
    std::string file = profilePath + "\\settings.json";
#else
    std::string file = profilePath + "/settings.json";
#endif
    std::ifstream f(file);
    if (!f.is_open()) return "";
    try {
        nlohmann::json j;
        f >> j;
        if (j.contains("browser") && j["browser"].is_object()) {
            // from_json applies the legacy-bool migration (true -> "notify") + validation.
            BrowserSettings b = j["browser"].get<BrowserSettings>();
            if (IsValidUpdateMode(b.autoUpdateMode)) return b.autoUpdateMode;
        }
    } catch (const std::exception&) {
        // Corrupt profile settings -> ignore (treat as no signal).
    }
    return "";
}
