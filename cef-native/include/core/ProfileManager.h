#pragma once

#include <string>
#include <vector>
#include <mutex>

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

    // Current profile management
    void SetCurrentProfileId(const std::string& id);
    std::string GetCurrentProfileId() const;
    
    // Path helpers
    std::string GetProfileDataPath(const std::string& id);
    std::string GetCurrentProfileDataPath();

    // Startup behavior
    bool ShouldShowPickerOnStartup() const;
    void SetShowPickerOnStartup(bool show);

    // Launch new instance with profile
    bool LaunchWithProfile(const std::string& profileId);

    // Parse --profile argument from command line
    static std::string ParseProfileArgument(int argc, char* argv[]);
    static std::string ParseProfileArgument(const std::wstring& cmdLine);

private:
    ProfileManager() = default;
    ~ProfileManager() = default;

    void Load();
    void Save();
    std::string GenerateProfileId();
    std::string GetCurrentTimestamp();

    mutable std::mutex mutex_;
    std::string app_data_path_;
    std::string profiles_file_path_;
    std::vector<ProfileInfo> profiles_;
    std::string currentProfileId_ = "Default";
    bool showPickerOnStartup_ = false;
    std::string defaultProfileId_ = "Default";
    bool initialized_ = false;

    // Prevent copying
    ProfileManager(const ProfileManager&) = delete;
    ProfileManager& operator=(const ProfileManager&) = delete;
};
