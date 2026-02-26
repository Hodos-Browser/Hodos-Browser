#include "../../include/core/ProfileManager.h"
#include <fstream>
#include <sstream>
#include <chrono>
#include <iomanip>
#include <filesystem>
#include <iostream>
#include <nlohmann/json.hpp>

#ifdef _WIN32
#include <windows.h>
#include <shellapi.h>
#endif

using json = nlohmann::json;
namespace fs = std::filesystem;

ProfileManager& ProfileManager::GetInstance() {
    static ProfileManager instance;
    return instance;
}

bool ProfileManager::Initialize(const std::string& app_data_path) {
    std::lock_guard<std::mutex> lock(mutex_);
    
    if (initialized_) {
        return true;
    }

    app_data_path_ = app_data_path;
    profiles_file_path_ = app_data_path + "/profiles.json";

    std::cout << "📁 ProfileManager initializing with path: " << app_data_path_ << std::endl;

    // Ensure app data directory exists
    try {
        fs::create_directories(app_data_path_);
    } catch (const std::exception& e) {
        std::cerr << "❌ Failed to create app data directory: " << e.what() << std::endl;
        return false;
    }

    Load();
    initialized_ = true;
    
    std::cout << "✅ ProfileManager initialized with " << profiles_.size() << " profiles" << std::endl;
    return true;
}

void ProfileManager::Load() {
    // Check if profiles.json exists
    if (!fs::exists(profiles_file_path_)) {
        std::cout << "📁 No profiles.json found, creating default profile" << std::endl;
        
        // Create default profile
        ProfileInfo defaultProfile;
        defaultProfile.id = "Default";
        defaultProfile.name = "Default";
        defaultProfile.color = "#5f6368";  // Gray
        defaultProfile.path = "Default";
        defaultProfile.createdAt = GetCurrentTimestamp();
        defaultProfile.avatarInitial = "D";
        
        profiles_.push_back(defaultProfile);
        currentProfileId_ = "Default";
        
        // Create default profile directory
        try {
            fs::create_directories(app_data_path_ + "/Default");
        } catch (...) {}
        
        Save();
        return;
    }

    // Read and parse profiles.json
    try {
        std::ifstream file(profiles_file_path_);
        if (!file.is_open()) {
            std::cerr << "❌ Failed to open profiles.json" << std::endl;
            return;
        }

        json j;
        file >> j;
        file.close();

        // Parse profiles
        profiles_.clear();
        if (j.contains("profiles") && j["profiles"].is_array()) {
            for (const auto& p : j["profiles"]) {
                ProfileInfo profile;
                profile.id = p.value("id", "");
                profile.name = p.value("name", "");
                profile.color = p.value("color", "#5f6368");
                profile.path = p.value("path", profile.id);
                profile.createdAt = p.value("createdAt", "");
                profile.avatarInitial = profile.name.empty() ? "?" : std::string(1, std::toupper(profile.name[0]));
                profile.avatarImage = p.value("avatarImage", "");  // Optional custom avatar
                
                if (!profile.id.empty()) {
                    profiles_.push_back(profile);
                }
            }
        }

        // Parse other settings
        currentProfileId_ = j.value("lastUsedProfile", "Default");
        showPickerOnStartup_ = j.value("showPickerOnStartup", false);

        std::cout << "📁 Loaded " << profiles_.size() << " profiles from profiles.json" << std::endl;

    } catch (const std::exception& e) {
        std::cerr << "❌ Error parsing profiles.json: " << e.what() << std::endl;
    }

    // Ensure at least default profile exists
    if (profiles_.empty()) {
        ProfileInfo defaultProfile;
        defaultProfile.id = "Default";
        defaultProfile.name = "Default";
        defaultProfile.color = "#5f6368";
        defaultProfile.path = "Default";
        defaultProfile.createdAt = GetCurrentTimestamp();
        defaultProfile.avatarInitial = "D";
        profiles_.push_back(defaultProfile);
        currentProfileId_ = "Default";
        Save();
    }
}

void ProfileManager::Save() {
    try {
        json j;
        j["version"] = 1;
        j["lastUsedProfile"] = currentProfileId_;
        j["showPickerOnStartup"] = showPickerOnStartup_;

        json profilesArray = json::array();
        for (const auto& p : profiles_) {
            json pj;
            pj["id"] = p.id;
            pj["name"] = p.name;
            pj["color"] = p.color;
            pj["path"] = p.path;
            pj["createdAt"] = p.createdAt;
            if (!p.avatarImage.empty()) {
                pj["avatarImage"] = p.avatarImage;
            }
            profilesArray.push_back(pj);
        }
        j["profiles"] = profilesArray;

        std::ofstream file(profiles_file_path_);
        if (file.is_open()) {
            file << j.dump(2);
            file.close();
            std::cout << "💾 Saved profiles.json" << std::endl;
        }
    } catch (const std::exception& e) {
        std::cerr << "❌ Error saving profiles.json: " << e.what() << std::endl;
    }
}

std::vector<ProfileInfo> ProfileManager::GetAllProfiles() {
    std::lock_guard<std::mutex> lock(mutex_);
    return profiles_;
}

ProfileInfo ProfileManager::GetCurrentProfile() {
    std::lock_guard<std::mutex> lock(mutex_);
    for (const auto& p : profiles_) {
        if (p.id == currentProfileId_) {
            return p;
        }
    }
    // Return first profile if current not found
    if (!profiles_.empty()) {
        return profiles_[0];
    }
    // Return empty profile
    return ProfileInfo{};
}

ProfileInfo ProfileManager::GetProfileById(const std::string& id) {
    std::lock_guard<std::mutex> lock(mutex_);
    for (const auto& p : profiles_) {
        if (p.id == id) {
            return p;
        }
    }
    return ProfileInfo{};
}

bool ProfileManager::CreateProfile(const std::string& name, const std::string& color, const std::string& avatarImage) {
    std::lock_guard<std::mutex> lock(mutex_);

    ProfileInfo profile;
    profile.id = GenerateProfileId();
    profile.name = name;
    profile.color = color.empty() ? "#5f6368" : color;
    profile.path = profile.id;
    profile.createdAt = GetCurrentTimestamp();
    profile.avatarInitial = name.empty() ? "?" : std::string(1, std::toupper(name[0]));
    profile.avatarImage = avatarImage;  // Optional base64 data URL

    // Create profile directory
    std::string profilePath = app_data_path_ + "/" + profile.path;
    try {
        fs::create_directories(profilePath);
    } catch (const std::exception& e) {
        std::cerr << "Failed to create profile directory: " << e.what() << std::endl;
        return false;
    }

    // Copy settings.json from current profile so new profiles inherit settings
    std::string currentProfilePath = app_data_path_ + "/" + currentProfileId_;
#ifdef _WIN32
    std::string srcSettings = currentProfilePath + "\\settings.json";
    std::string dstSettings = profilePath + "\\settings.json";
#else
    std::string srcSettings = currentProfilePath + "/settings.json";
    std::string dstSettings = profilePath + "/settings.json";
#endif
    try {
        if (fs::exists(srcSettings)) {
            fs::copy_file(srcSettings, dstSettings, fs::copy_options::skip_existing);
        }
    } catch (const std::exception& e) {
        // Non-fatal — new profile will use defaults
        std::cerr << "Note: Could not copy settings to new profile: " << e.what() << std::endl;
    }

    profiles_.push_back(profile);
    Save();

    std::cout << "✅ Created profile: " << name << " (" << profile.id << ")" << std::endl;
    return true;
}

bool ProfileManager::DeleteProfile(const std::string& id) {
    std::lock_guard<std::mutex> lock(mutex_);

    // Can't delete the last profile
    if (profiles_.size() <= 1) {
        std::cerr << "❌ Cannot delete the last profile" << std::endl;
        return false;
    }

    // Can't delete Default profile
    if (id == "Default") {
        std::cerr << "❌ Cannot delete the Default profile" << std::endl;
        return false;
    }

    auto it = std::find_if(profiles_.begin(), profiles_.end(),
        [&id](const ProfileInfo& p) { return p.id == id; });

    if (it == profiles_.end()) {
        return false;
    }

    // Delete profile directory (optional - could move to trash instead)
    std::string profilePath = app_data_path_ + "/" + it->path;
    // Note: Not deleting files for safety - user can manually delete

    profiles_.erase(it);

    // If we deleted the current profile, switch to Default
    if (currentProfileId_ == id) {
        currentProfileId_ = "Default";
    }

    Save();
    std::cout << "✅ Deleted profile: " << id << std::endl;
    return true;
}

bool ProfileManager::RenameProfile(const std::string& id, const std::string& newName) {
    std::lock_guard<std::mutex> lock(mutex_);

    for (auto& p : profiles_) {
        if (p.id == id) {
            p.name = newName;
            p.avatarInitial = newName.empty() ? "?" : std::string(1, std::toupper(newName[0]));
            Save();
            return true;
        }
    }
    return false;
}

bool ProfileManager::SetProfileColor(const std::string& id, const std::string& color) {
    std::lock_guard<std::mutex> lock(mutex_);

    for (auto& p : profiles_) {
        if (p.id == id) {
            p.color = color;
            Save();
            return true;
        }
    }
    return false;
}

void ProfileManager::SetCurrentProfileId(const std::string& id) {
    std::lock_guard<std::mutex> lock(mutex_);
    currentProfileId_ = id;
    Save();
}

std::string ProfileManager::GetCurrentProfileId() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return currentProfileId_;
}

std::string ProfileManager::GetProfileDataPath(const std::string& id) {
    std::lock_guard<std::mutex> lock(mutex_);
    for (const auto& p : profiles_) {
        if (p.id == id) {
            return app_data_path_ + "/" + p.path;
        }
    }
    return app_data_path_ + "/Default";
}

std::string ProfileManager::GetCurrentProfileDataPath() {
    return GetProfileDataPath(currentProfileId_);
}

bool ProfileManager::ShouldShowPickerOnStartup() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return showPickerOnStartup_;
}

void ProfileManager::SetShowPickerOnStartup(bool show) {
    std::lock_guard<std::mutex> lock(mutex_);
    showPickerOnStartup_ = show;
    Save();
}

bool ProfileManager::LaunchWithProfile(const std::string& profileId) {
#ifdef _WIN32
    // Get current executable path
    wchar_t exePath[MAX_PATH];
    GetModuleFileNameW(NULL, exePath, MAX_PATH);

    // Build command line with --profile argument
    std::wstring cmdLine = L"\"";
    cmdLine += exePath;
    cmdLine += L"\" --profile=\"";
    cmdLine += std::wstring(profileId.begin(), profileId.end());
    cmdLine += L"\"";

    // Launch new instance
    STARTUPINFOW si = { sizeof(si) };
    PROCESS_INFORMATION pi;

    if (CreateProcessW(
        NULL,
        const_cast<LPWSTR>(cmdLine.c_str()),
        NULL, NULL, FALSE,
        0, NULL, NULL,
        &si, &pi
    )) {
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);
        std::cout << "🚀 Launched new instance with profile: " << profileId << std::endl;
        return true;
    } else {
        std::cerr << "❌ Failed to launch new instance: " << GetLastError() << std::endl;
        return false;
    }
#else
    // macOS implementation would go here
    std::cerr << "❌ LaunchWithProfile not implemented for this platform" << std::endl;
    return false;
#endif
}

std::string ProfileManager::ParseProfileArgument(int argc, char* argv[]) {
    for (int i = 1; i < argc; i++) {
        std::string arg = argv[i];
        if (arg.find("--profile=") == 0) {
            return arg.substr(10);  // Length of "--profile="
        }
    }
    return "Default";
}

std::string ProfileManager::ParseProfileArgument(const std::wstring& cmdLine) {
    size_t pos = cmdLine.find(L"--profile=");
    if (pos != std::wstring::npos) {
        size_t start = pos + 10;  // Length of "--profile="
        std::wstring profileW;

        // Handle quoted values (e.g., --profile="Profile 2")
        if (start < cmdLine.length() && cmdLine[start] == L'"') {
            size_t quoteEnd = cmdLine.find(L'"', start + 1);
            if (quoteEnd == std::wstring::npos) {
                quoteEnd = cmdLine.length();
            }
            profileW = cmdLine.substr(start + 1, quoteEnd - start - 1);
        } else {
            // Unquoted: find next -- flag or end of string
            // This handles shells that strip quotes (e.g., --profile=Profile 2)
            size_t end = cmdLine.find(L" --", start);
            if (end == std::wstring::npos) {
                end = cmdLine.length();
            }
            profileW = cmdLine.substr(start, end - start);
            // Trim trailing whitespace
            while (!profileW.empty() && profileW.back() == L' ') {
                profileW.pop_back();
            }
        }
        // Convert wstring to string properly (handles Unicode)
#ifdef _WIN32
        if (profileW.empty()) return "Default";
        int size_needed = WideCharToMultiByte(CP_UTF8, 0, profileW.c_str(), (int)profileW.size(), NULL, 0, NULL, NULL);
        std::string result(size_needed, 0);
        WideCharToMultiByte(CP_UTF8, 0, profileW.c_str(), (int)profileW.size(), &result[0], size_needed, NULL, NULL);
        return result;
#else
        return std::string(profileW.begin(), profileW.end());
#endif
    }
    return "Default";
}

std::string ProfileManager::GenerateProfileId() {
    // Generate "Profile_N" where N is the next available number
    // No spaces — avoids shell quoting issues with --profile= argument
    int maxNum = 1;
    for (const auto& p : profiles_) {
        // Match both old "Profile N" and new "Profile_N" formats
        std::string prefix;
        if (p.id.find("Profile_") == 0) {
            prefix = p.id.substr(8);
        } else if (p.id.find("Profile ") == 0) {
            prefix = p.id.substr(8);
        } else {
            continue;
        }
        try {
            int num = std::stoi(prefix);
            if (num >= maxNum) {
                maxNum = num + 1;
            }
        } catch (...) {}
    }
    return "Profile_" + std::to_string(maxNum);
}

std::string ProfileManager::GetCurrentTimestamp() {
    auto now = std::chrono::system_clock::now();
    auto time = std::chrono::system_clock::to_time_t(now);
    std::stringstream ss;
    ss << std::put_time(std::gmtime(&time), "%Y-%m-%dT%H:%M:%SZ");
    return ss.str();
}
