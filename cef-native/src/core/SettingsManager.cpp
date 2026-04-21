#include "../../include/core/SettingsManager.h"
#include "../../include/core/Logger.h"
#include "../../include/core/AppPaths.h"
#include <fstream>
#include <cstdlib>
#include <filesystem>

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
    if (!file.is_open()) {
        LOG_INFO_SM("No settings file found, using defaults");
        return;
    }

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

void SettingsManager::SetAutoUpdateEnabled(bool enabled) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        browser_.autoUpdateEnabled = enabled;
    }
    Save();
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
        
        {
            std::lock_guard<std::mutex> lock(mutex_);
            
            if (j.contains("browser")) {
                browser_ = j["browser"].get<BrowserSettings>();
            }
            if (j.contains("privacy")) {
                privacy_ = j["privacy"].get<PrivacySettings>();
            }
            if (j.contains("wallet")) {
                wallet_ = j["wallet"].get<WalletSettings>();
            }
        }
        
        Save();
        return true;
    } catch (const std::exception& e) {
        LOG_ERROR_SM("❌ Failed to update settings from JSON: " + std::string(e.what()));
        return false;
    }
}
