#pragma once

#include <string>
#include <vector>
#include <mutex>
#include <algorithm>

struct ProfileInfo {
    std::string id;
    std::string name;
    std::string color;
    std::string path;
    std::string createdAt;
    std::string avatarInitial;  // First letter of name, uppercase
    std::string avatarImage;    // Base64 data URL for custom avatar (optional)
};

class ProfileManager {
public:
    static ProfileManager& GetInstance();

    // Initialize with base app data path
    bool Initialize(const std::string& app_data_path);

    // Profile CRUD
    std::vector<ProfileInfo> GetAllProfiles();
    ProfileInfo GetCurrentProfile();
    ProfileInfo GetProfileById(const std::string& id);
    bool CreateProfile(const std::string& name, const std::string& color, const std::string& avatarImage = "");
    bool DeleteProfile(const std::string& id);
    bool RenameProfile(const std::string& id, const std::string& newName);
    bool SetProfileColor(const std::string& id, const std::string& color);
    bool SetProfileAvatar(const std::string& id, const std::string& avatarImage);

    // Default profile
    bool SetDefaultProfile(const std::string& id);
    std::string GetDefaultProfileId() const;

    // Current profile management.
    // `persist` controls R5 semantics: write lastUsedProfile to profiles.json
    // ONLY on an explicit user/shortcut choice (a valid --profile= or a picker
    // selection). A plain no-arg last-used launch sets the in-memory current id
    // WITHOUT rewriting the registry (avoids the boot-rewrite churn + torn-write
    // race the old unconditional Save() caused).
    void SetCurrentProfileId(const std::string& id, bool persist);
    std::string GetCurrentProfileId() const;
    
    // Path helpers
    std::string GetProfileDataPath(const std::string& id);
    std::string GetCurrentProfileDataPath();

    // Startup behavior
    bool ShouldShowPickerOnStartup() const;
    void SetShowPickerOnStartup(bool show);

    // Launch new instance with profile.
    // linkParentExitHandle (Windows only): when true, hand the spawned child an inheritable,
    // SYNCHRONIZE|QUERY handle to THIS process so the child's silent-update sole-instance gate
    // can wait for THIS (transient picker) process to fully exit before counting. Set ONLY by
    // the pre-window profile picker (g_picker_mode) — NOT by the in-browser profile switch,
    // whose parent stays alive. Best-effort: any handle-plumbing failure falls back to a plain
    // spawn. No-op on macOS (Launch Services can't inherit Win32 handles; the picker-defer bug
    // is Windows-only — mac silent update is Sparkle install-on-quit). See
    // DevOps-CICD/AUTOUPDATE_PICKER_GATE_DESIGN.md (v2).
    bool LaunchWithProfile(const std::string& profileId, bool linkParentExitHandle = false);

    // F5 (audit): syntactic validation of a profile id. The id is generated
    // internally ("Default", "Profile_N", legacy "Profile N") and is used BOTH
    // as a directory name AND, historically, interpolated toward a process-launch
    // argument. Allow only [A-Za-z0-9_ -]: that covers every generated shape
    // (including the legacy space form) while excluding all shell metacharacters
    // and path separators (no '.', '/', '\\', '"', '\'', ';', '$', backtick…),
    // so a hostile id can neither traverse paths nor inject a command. Inline so
    // it can be unit-tested without linking the CEF-heavy .cpp.
    static bool IsValidProfileId(const std::string& id) {
        if (id.empty() || id.size() > 64) return false;
        for (unsigned char c : id) {
            const bool ok = (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') ||
                            (c >= '0' && c <= '9') ||
                            c == '_' || c == ' ' || c == '-';
            if (!ok) return false;
        }
        return true;
    }

    // Result of resolving which profile a process should open at startup.
    struct StartupResolution {
        std::string profileId;     // profile to open (ignored when showPicker)
        bool showPicker = false;   // enter the pre-window picker instead? (Windows)
    };

    // Pure startup-profile resolver (R7 + picker gate). Header-only so it can be
    // unit-tested without linking the CEF-heavy .cpp. Decision table:
    //   - explicit --profile, valid AND exists  -> open it
    //   - explicit --profile, invalid/unknown   -> COHERENT default fallback (R7)
    //   - no --profile, <=1 profile             -> the sole/default profile
    //   - no --profile, >1, pickerEnabled        -> PICKER mode
    //   - no --profile, >1, picker disabled      -> the default (starred) profile
    // There is intentionally NO "last-used" concept: with the picker the user
    // chooses each cold start, and when the picker is off a deterministic default
    // beats a surprising last-used. Exact registry-existence match is the R7
    // coherence guarantee — a mangled id (e.g. a quote-stripped legacy "Profile 2")
    // misses the registry and falls back to the default coherently, never silently
    // landing in the wrong profile. The default fallback is itself guarded:
    // defaultProfileId is persisted independently and could name a deleted profile,
    // so we drop to the first real profile rather than return an id with no dir.
    static StartupResolution ResolveStartup(
            const std::string& argProfile,
            const std::vector<std::string>& existingIds,
            const std::string& defaultProfileId,
            bool pickerEnabled) {
        auto exists = [&](const std::string& id) {
            return std::find(existingIds.begin(), existingIds.end(), id) != existingIds.end();
        };
        auto coherentDefault = [&]() -> std::string {
            if (exists(defaultProfileId)) return defaultProfileId;
            return existingIds.empty() ? std::string("Default") : existingIds.front();
        };
        StartupResolution r;
        if (!argProfile.empty()) {
            r.profileId = (IsValidProfileId(argProfile) && exists(argProfile))
                ? argProfile
                : coherentDefault();   // R7 coherent fallback
            return r;
        }
        // No --profile argument.
        if (existingIds.size() <= 1) {
            r.profileId = coherentDefault();
            return r;
        }
        // More than one profile: picker (if enabled) else the default profile.
        r.profileId = coherentDefault();   // also the bypass target if the picker is skipped
        r.showPicker = pickerEnabled;
        return r;
    }

    // Parse --profile argument from command line
    static std::string ParseProfileArgument(int argc, char* argv[]);
    static std::string ParseProfileArgument(const std::wstring& cmdLine);

private:
    ProfileManager() = default;
    ~ProfileManager() = default;

    void Load();
    void Save();          // acquires the cross-process registry lock, then SaveUnlocked()
    void SaveUnlocked();  // atomic tmp+rename write; caller already holds the registry lock
    std::string GenerateProfileId();
    std::string GetCurrentTimestamp();

    mutable std::mutex mutex_;
    std::string app_data_path_;
    std::string profiles_file_path_;
    std::vector<ProfileInfo> profiles_;
    std::string currentProfileId_ = "Default";
    bool showPickerOnStartup_ = true;   // CHUNK 2: picker ON by default (gated to >1 profile at the call site)
    std::string defaultProfileId_ = "Default";
    bool initialized_ = false;

    // Prevent copying
    ProfileManager(const ProfileManager&) = delete;
    ProfileManager& operator=(const ProfileManager&) = delete;
};
