#include "../../include/core/ProfileManager.h"
#include "../../include/core/ProfileLock.h"
#include "../../include/core/Logger.h"
#include "../../include/core/AumidPolicy.h"
#include <fstream>
#include <sstream>
#include <chrono>
#include <iomanip>
#include <filesystem>
#include <iostream>
#include <vector>
#include <set>
#include <nlohmann/json.hpp>

#ifdef _WIN32
#include <windows.h>
#include <shellapi.h>
#elif defined(__APPLE__)
#include <mach-o/dyld.h>
#include <unistd.h>
#include <limits.h>
#include <spawn.h>      // F5: posix_spawn (no shell)
#include <sys/wait.h>   // F5: waitpid
#include <cstring>      // strerror
#include <cerrno>
#include <fcntl.h>      // R5: open() for the registry lock file
#include <sys/file.h>   // R5: flock()
extern char** environ;  // pass the real environment to `open`
#endif

using json = nlohmann::json;
namespace fs = std::filesystem;

// ⛔ MEASURED 2026-08-26: this file's std::cout / std::cerr output reached NO log — not
// debug_output.log, not cef_debug.log. Zero occurrences of "📁 ProfileManager initializing",
// a line that runs on every single startup, across 2.5 GB and 128 sessions. Every refusal
// below ("cannot delete the default profile", "…the profile this window is running on") was
// silent, and so was the one warning an operator can act on (a deletion that did not finish).
//
// ROOT CAUSE, now established: Logger::Initialize holds the log file open, so the freopen
// that was supposed to redirect stdout into it ALWAYS fails EACCES(13) and the fallback
// reopens stdout on NUL. The claim in cef-native/CLAUDE.md that "stdout is redirected to the
// same file anyway" was false and never was true — corrected there.
//
// FIXED in beta.3 Phase 2b: every site in this file now goes through Logger.
// ⛔ Do not reintroduce std::cout here, and do not "fix" the blackhole by making the freopen
// succeed — that puts two writers on one file.
#define LOG_INFO_PM(msg)    Logger::Log(msg, 1, 2)
#define LOG_WARNING_PM(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_PM(msg)   Logger::Log(msg, 3, 2)

namespace {

// R5: cross-process lock around profiles.json read+write. The per-process
// std::mutex in ProfileManager guards threads within ONE process; it does NOT
// protect the shared registry file when several profile instances (separate
// processes) run concurrently. Without this, a Save() (non-atomic truncate-
// rewrite) interleaving with another process's Load() can produce a torn read
// → the reader hits the empty-profiles catch and Save()s, DESTROYING the list.
//
// Scoped RAII. Acquired as the first statement inside Load()/SaveUnlocked()'s
// callers so the lock order is always per-process mutex_ -> this cross-process
// lock (no deadlock). On acquisition timeout/failure we proceed anyway (best
// effort) rather than block startup forever.
//
// NOTE: do NOT nest two RegistryLocks on one thread on macOS — flock() on a
// second fd from the same process would deadlock. Hence Load() and Save() take
// the lock and call SaveUnlocked() (no re-lock) internally.
class RegistryLock {
public:
    explicit RegistryLock(const std::string& appDataPath) {
#ifdef _WIN32
        // Name the mutex per data-root so dev (HodosBrowserDev) and prod don't
        // cross-lock. Local\ namespace = per-session, fine for same-user instances.
        std::string name = "Local\\HodosProfilesLock_";
        for (char c : appDataPath) name += (c == '\\' || c == '/' || c == ':') ? '_' : c;
        if (name.size() > 240) name.resize(240);
        handle_ = CreateMutexA(nullptr, FALSE, name.c_str());
        if (handle_) {
            // WAIT_ABANDONED (prior owner crashed) still grants ownership — fine.
            DWORD w = WaitForSingleObject(handle_, 5000);
            owned_ = (w == WAIT_OBJECT_0 || w == WAIT_ABANDONED);
            if (!owned_) {
                // Best-effort: proceed without the lock rather than block startup
                // forever, but make the degraded (unlocked) window observable.
                LOG_WARNING_PM("⚠️ RegistryLock: profiles.json lock wait timed out; "
                               "proceeding unlocked");
            }
        }
#elif defined(__APPLE__)
        lockPath_ = appDataPath + "/.profiles.lock";
        fd_ = open(lockPath_.c_str(), O_CREAT | O_RDWR | O_CLOEXEC, 0600);
        if (fd_ >= 0) {
            // Non-blocking flock with bounded retry (matches Windows 5s timeout).
            // A crashed peer's flock auto-releases on process death, but an fd
            // inherited by a child (wallet/adblock) can hold it longer.
            for (int i = 0; i < 10; ++i) {
                if (flock(fd_, LOCK_EX | LOCK_NB) == 0) break;
                if (i == 9) {
                    LOG_WARNING_PM("⚠️ RegistryLock: .profiles.lock timed out; "
                                   "proceeding unlocked");
                    close(fd_);
                    fd_ = -1;
                    break;
                }
                usleep(500000);  // 500ms × 10 = 5s max
            }
        }
#else
        (void)appDataPath;
#endif
    }
    ~RegistryLock() {
#ifdef _WIN32
        if (handle_) {
            if (owned_) ReleaseMutex(handle_);  // never release a mutex we don't own
            CloseHandle(handle_);
        }
#elif defined(__APPLE__)
        if (fd_ >= 0) { flock(fd_, LOCK_UN); close(fd_); }
#endif
    }
    RegistryLock(const RegistryLock&) = delete;
    RegistryLock& operator=(const RegistryLock&) = delete;
private:
#ifdef _WIN32
    HANDLE handle_ = nullptr;
    bool owned_ = false;
#elif defined(__APPLE__)
    int fd_ = -1;
    std::string lockPath_;
#endif
};

}  // namespace

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

    LOG_INFO_PM(LogFmt() << "📁 ProfileManager initializing with path: " << app_data_path_);

    // Ensure app data directory exists
    try {
        fs::create_directories(app_data_path_);
    } catch (const std::exception& e) {
        LOG_ERROR_PM(LogFmt() << "❌ Failed to create app data directory: " << e.what());
        return false;
    }

    Load();
    SweepOrphanedProfileDirs();
    initialized_ = true;
    
    LOG_INFO_PM(LogFmt() << "✅ ProfileManager initialized with " << profiles_.size() << " profiles");
    return true;
}

void ProfileManager::SweepOrphanedProfileDirs() {
    // Caller holds mutex_ and has already run Load(), so profiles_ is current.
    std::error_code ec;
    if (app_data_path_.empty() || !fs::exists(app_data_path_, ec)) return;

    std::set<std::string> listed;
    for (const auto& p : profiles_) {
        listed.insert(p.path.empty() ? p.id : p.path);
    }

    // Positive evidence that a browser session actually RAN on this directory.
    //
    // ⚠️ This is what keeps the sweep off a profile another instance is creating right now:
    // CreateProfile makes the directory (and copies settings.json into it) BEFORE the
    // profile appears in profiles.json, so for a moment a legitimate new profile looks
    // exactly like an orphan. None of these artifacts exist until Chromium has run on it.
    // ⛔ settings.json is deliberately NOT in this list — it is copied at create time and
    // would make a half-created profile look used.
    static const char* kUsedMarkers[] = {
        "Preferences", "History", "Cookies", "bookmarks.db", "Network", "Local Storage"
    };

    // Collect first, rename second — renaming entries while iterating a directory is not
    // safe to assume.
    std::vector<fs::path> candidates;
    for (const auto& entry : fs::directory_iterator(app_data_path_, ec)) {
        if (ec) break;
        if (!entry.is_directory(ec)) continue;

        const std::string name = entry.path().filename().string();
        if (!IsGeneratedProfileDirName(name)) continue;   // excludes Default, logs, wallet, prior sweeps
        if (listed.count(name)) continue;                 // a live profile

        bool used = false;
        for (const char* marker : kUsedMarkers) {
            if (fs::exists(entry.path() / marker, ec)) { used = true; break; }
        }
        if (!used) continue;

        candidates.push_back(entry.path());
    }

    if (candidates.empty()) return;

    const std::string stamp = std::to_string(
        std::chrono::duration_cast<std::chrono::seconds>(
            std::chrono::system_clock::now().time_since_epoch()).count());

    for (const auto& dir : candidates) {
        const std::string name = dir.filename().string();

        // ⛔ Never touch a directory a live instance holds. An unlisted-but-locked directory
        // means another process is running a profile this profiles.json does not know about
        // — a torn read, or a registry write we raced. Leave it and say so.
        if (IsProfileLockedByAnotherInstance(dir.string())) {
            LOG_WARNING_PM("👤 Orphan sweep: '" + name +
                           "' is unlisted but LOCKED by a running instance — left alone");
            continue;
        }

        std::error_code rec;
        const fs::path dest = dir.string() + ".orphaned-" + stamp;
        fs::rename(dir, dest, rec);
        if (rec) {
            // Another instance may have swept it, or it is busy. Harmless either way: the
            // id stays taken, so nothing can be reissued over it.
            LOG_WARNING_PM("👤 Orphan sweep: could not move '" + name + "' (" +
                           rec.message() + ") — left in place");
        } else {
            LOG_INFO_PM("👤 Orphan sweep: '" + name + "' was unlisted but held profile data; "
                        "moved to '" + dest.filename().string() +
                        "' so a new profile cannot inherit it. Safe to delete by hand.");
        }
    }
}

void ProfileManager::Load() {
    // R5: hold the cross-process registry lock for the whole read (and any
    // first-run write below) so we never read a half-written profiles.json.
    RegistryLock registryLock(app_data_path_);

    // Check if profiles.json exists
    if (!fs::exists(profiles_file_path_)) {
        LOG_INFO_PM(LogFmt() << "📁 No profiles.json found, creating default profile");
        
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

        SaveUnlocked();  // registry lock already held by this Load()
        return;
    }

    // Read and parse profiles.json
    try {
        std::ifstream file(profiles_file_path_);
        if (!file.is_open()) {
            LOG_ERROR_PM(LogFmt() << "❌ Failed to open profiles.json");
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
        int loadedVersion = j.value("version", 1);
        currentProfileId_ = j.value("lastUsedProfile", "Default");
        showPickerOnStartup_ = j.value("showPickerOnStartup", true);  // CHUNK 2: default ON
        defaultProfileId_ = j.value("defaultProfileId", "Default");

        LOG_INFO_PM(LogFmt() << "📁 Loaded " << profiles_.size() << " profiles from profiles.json");

        // CHUNK 2 — one-time migration to v2: turn the startup picker ON for
        // existing users (v1 persisted it false by the old default). Runs once;
        // SaveUnlocked() bumps version to 2 so a later explicit user choice (or a
        // manual edit) is respected and the flag is never auto-flipped again.
        if (loadedVersion < 2) {
            showPickerOnStartup_ = true;
            LOG_INFO_PM("🔁 Migrating profiles.json to v2: enabling startup picker");
            SaveUnlocked();  // registry lock already held by this Load()
        }

    } catch (const std::exception& e) {
        LOG_ERROR_PM(LogFmt() << "❌ Error parsing profiles.json: " << e.what());
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
        SaveUnlocked();  // registry lock already held by this Load()
    }
}

void ProfileManager::Save() {
    // R5: take the cross-process registry lock, then do the atomic write.
    RegistryLock registryLock(app_data_path_);
    SaveUnlocked();
}

void ProfileManager::SaveUnlocked() {
    // R5: atomic write — serialize to a sibling .tmp then rename() over the
    // target. rename() is atomic on the same volume, so a crash mid-write can
    // never leave a truncated/torn profiles.json (the failure mode that, on the
    // next Load(), makes a reader fall into the empty-profiles path and clobber
    // the list). The caller already holds the registry lock.
    try {
        json j;
        j["version"] = 2;  // CHUNK 2: v2 = startup-picker migration applied
        j["lastUsedProfile"] = currentProfileId_;
        j["showPickerOnStartup"] = showPickerOnStartup_;
        j["defaultProfileId"] = defaultProfileId_;

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

        const std::string payload = j.dump(2);
        const std::string tmpPath = profiles_file_path_ + ".tmp";

        {
            std::ofstream file(tmpPath, std::ios::binary | std::ios::trunc);
            if (!file.is_open()) {
                LOG_ERROR_PM(LogFmt() << "❌ Error saving profiles.json: cannot open temp file");
                return;
            }
            file << payload;
            file.flush();
            file.close();
        }

        std::error_code ec;
        fs::rename(tmpPath, profiles_file_path_, ec);
        if (ec) {
            // rename can fail across devices or on transient locks — fall back to
            // a direct write so we never silently lose the registry, and clean up.
            std::ofstream file(profiles_file_path_, std::ios::binary | std::ios::trunc);
            if (file.is_open()) {
                file << payload;
                file.close();
            }
            std::error_code rmEc;
            fs::remove(tmpPath, rmEc);
        }
        LOG_INFO_PM(LogFmt() << "💾 Saved profiles.json");
    } catch (const std::exception& e) {
        LOG_ERROR_PM(LogFmt() << "❌ Error saving profiles.json: " << e.what());
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
        LOG_ERROR_PM(LogFmt() << "Failed to create profile directory: " << e.what());
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
        LOG_ERROR_PM(LogFmt() << "Note: Could not copy settings to new profile: " << e.what());
    }

    profiles_.push_back(profile);
    Save();

    LOG_INFO_PM(LogFmt() << "✅ Created profile: " << name << " (" << profile.id << ")");
    return true;
}

bool ProfileManager::DeleteProfile(const std::string& id) {
    std::lock_guard<std::mutex> lock(mutex_);

    // Can't delete the last profile
    if (profiles_.size() <= 1) {
        LOG_ERROR_PM("👤 Delete REFUSED: this is the last profile");
        return false;
    }

    // Can't delete the default profile
    if (id == defaultProfileId_) {
        LOG_ERROR_PM("👤 Delete REFUSED for '" + id + "': it is the default profile");
        return false;
    }

    // ⛔ Can't delete the profile THIS process is running on. Added 2026-08-24
    // after it was done by accident during beta.3 P0.9 testing (log 11:05:13
    // launch Profile_4 -> 11:05:41 delete Profile_4). Nothing stopped it, and the
    // consequences were both confusing and unsafe:
    //   * the live process kept its data directory and its --profile= argument,
    //     but profiles.json no longer listed that id, so the header UI failed its
    //     own lookup and fell back to displaying the DEFAULT profile's identity —
    //     the browser looked like it had silently switched profiles;
    //   * the freed id was immediately reissued to the next profile created (see
    //     GenerateProfileId), which then adopted ~200 MB of the deleted profile's
    //     history, cookies and content settings.
    // ⚠️ Read currentProfileId_ DIRECTLY — GetCurrentProfileId() takes the same
    // mutex this function already holds and would deadlock.
    if (id == currentProfileId_) {
        LOG_ERROR_PM("👤 Delete REFUSED for '" + id + "': this window is running it");
        return false;
    }

    auto it = std::find_if(profiles_.begin(), profiles_.end(),
        [&id](const ProfileInfo& p) { return p.id == id; });

    if (it == profiles_.end()) {
        return false;
    }

    std::string profilePath = app_data_path_ + "/" + it->path;
    // Captured before the erase below invalidates `it`. Used to remove this profile's
    // Start Menu shortcut once the deletion has actually succeeded.
    const std::string profileName = it->name;

    // ⛔ Is another INSTANCE running this profile? currentProfileId_ above only knows about
    // this process; a second Hodos window running another profile is invisible to it. Since
    // this function now destroys files, deleting a profile another window is using would
    // pull data out from under a live browser. The lock file is the only thing that knows.
    if (IsProfileLockedByAnotherInstance(profilePath)) {
        LOG_ERROR_PM("👤 Delete REFUSED for '" + id + "': another Hodos window holds its profile.lock");
        return false;
    }

    // ── Deleting the data. Changed 2026-08-26; this used to keep the files:
    //      // Note: Not deleting files for safety - user can manually delete
    //    That was the wrong trade for a privacy browser. A user deleting a profile is
    //    usually deleting it to be rid of the browsing data, and we were silently keeping
    //    every cookie, session and history entry forever (173.8 MB measured on one
    //    abandoned profile) with no UI that ever mentioned them.
    //
    // ⭐ RENAME FIRST, then delete. The rename is atomic and instant; a recursive delete of
    //    a few hundred MB is neither, and on Windows it routinely fails partway on a file
    //    still held open. Deleting in place and failing halfway would leave the directory
    //    PRESENT, still name-matchable by GenerateProfileId, and now half-gutted — the worst
    //    of both. Renaming first makes "the id can never be reissued over live data"
    //    independent of whether the delete succeeds.
    // ⚠️ The suffix must not match "Profile_<N>", or GenerateProfileId could pick it up again.
    std::error_code ec;
    const std::string stashPath =
        profilePath + ".deleted-" + std::to_string(
            std::chrono::duration_cast<std::chrono::seconds>(
                std::chrono::system_clock::now().time_since_epoch()).count());

    if (fs::exists(profilePath)) {
        fs::rename(profilePath, stashPath, ec);
        if (ec) {
            // Could not even rename — the directory is busy. Do NOT fall through to a
            // delete-in-place; abort while everything is still consistent.
            LOG_ERROR_PM("👤 Delete REFUSED for '" + id + "': could not move its data (" + ec.message() + ")");
            return false;
        }

        std::uintmax_t removed = fs::remove_all(stashPath, ec);
        if (ec) {
            // Best effort. The remains are under a name GenerateProfileId cannot match, so
            // the security property holds; the leftovers are a disk-space matter only.
            LOG_WARNING_PM("👤 Profile '" + id + "' unlisted and its data moved to " + stashPath +
                           ", but removal did not complete (" + ec.message() + "). Safe to delete by hand.");
        } else {
            LOG_INFO_PM("🗑️ Deleted " + std::to_string(removed) + " file(s) for profile " + id);
        }
    }

    profiles_.erase(it);

    // Unreachable since the guard above — kept as belt-and-braces only. If you find
    // yourself relying on this, the guard has been weakened and that is the bug.
    if (currentProfileId_ == id) {
        currentProfileId_ = defaultProfileId_;
    }

    Save();

#ifdef _WIN32
    // Remove the Start Menu shortcut that named this profile's taskbar button
    // (hodos::EnsureProfileShortcut writes it at startup). Done only after the profile is
    // genuinely gone — every failure path above returns early, so a refused delete never
    // removes a shortcut for a profile that still exists. Best-effort: a shortcut we
    // cannot delete is Start Menu clutter, not a reason to report the delete as failed.
    hodos::RemoveProfileShortcut(profileName);
#endif

    LOG_INFO_PM("✅ Deleted profile: " + id);
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

bool ProfileManager::SetProfileAvatar(const std::string& id, const std::string& avatarImage) {
    std::lock_guard<std::mutex> lock(mutex_);

    for (auto& p : profiles_) {
        if (p.id == id) {
            p.avatarImage = avatarImage;
            Save();
            return true;
        }
    }
    return false;
}

bool ProfileManager::SetDefaultProfile(const std::string& id) {
    std::lock_guard<std::mutex> lock(mutex_);

    for (const auto& p : profiles_) {
        if (p.id == id) {
            defaultProfileId_ = id;
            Save();
            return true;
        }
    }
    return false;
}

std::string ProfileManager::GetDefaultProfileId() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return defaultProfileId_;
}

void ProfileManager::SetCurrentProfileId(const std::string& id, bool persist) {
    std::lock_guard<std::mutex> lock(mutex_);
    currentProfileId_ = id;
    // R5: only an explicit choice (valid --profile= / picker selection) writes
    // lastUsedProfile. A plain no-arg last-used launch sets the in-memory id
    // only — no registry rewrite, no torn-write exposure, no boot churn.
    if (persist) {
        Save();
    }
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

bool ProfileManager::LaunchWithProfile(const std::string& profileId, bool linkParentExitHandle) {
    // F5 (audit): reject any id that isn't a well-formed profile id BEFORE it
    // reaches a process-launch boundary (or, on macOS, the shell). Cross-platform
    // primary gate; the macOS branch below additionally avoids the shell entirely.
    if (!IsValidProfileId(profileId)) {
        LOG_ERROR_PM(LogFmt() << "❌ Refusing to launch: invalid profile id");
        return false;
    }
    (void)linkParentExitHandle;  // consumed on Windows below; no-op on macOS/other
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

    // ── Picker-gate v2 (AUTOUPDATE_PICKER_GATE_DESIGN.md): if asked, hand the child an
    // inheritable handle to OUR OWN process so its silent-update sole-instance gate can wait
    // for this transient picker to EXIT before counting (instead of blind-polling an 8s cap
    // that expired mid-teardown on slow boxes). Mirrors the bootstrap→helper bootstrap-handle
    // pattern (SU_SpawnHelper): dup the GetCurrentProcess() pseudo-handle into a real
    // inheritable SYNCHRONIZE|QUERY handle, then restrict inheritance to EXACTLY that one
    // handle via PROC_THREAD_ATTRIBUTE_HANDLE_LIST (so bInheritHandles=TRUE can't leak the
    // ~8-proc picker's other inheritable handles). BEST-EFFORT: any failure here falls
    // through to the plain spawn below — a profile must ALWAYS launch (G-1).
    HANDLE pickerSelf = nullptr;                 // our own process handle, inheritable copy
    STARTUPINFOEXW six{}; six.StartupInfo.cb = sizeof(six);
    std::vector<unsigned char> attrBuf;
    LPPROC_THREAD_ATTRIBUTE_LIST attrList = nullptr;  // set ONLY on full success (used, then deleted)
    if (linkParentExitHandle) {
        SIZE_T attrSize = 0;
        InitializeProcThreadAttributeList(nullptr, 1, 0, &attrSize);  // size query
        attrBuf.resize(attrSize);
        LPPROC_THREAD_ATTRIBUTE_LIST tmp =
            reinterpret_cast<LPPROC_THREAD_ATTRIBUTE_LIST>(attrBuf.data());
        HANDLE dup = nullptr;
        // Short-circuit each step; DeleteProcThreadAttributeList is only valid after a
        // SUCCESSFUL Initialize (mirrors SU_SpawnHelper), so track attrInit separately.
        bool ok = DuplicateHandle(GetCurrentProcess(), GetCurrentProcess(), GetCurrentProcess(),
                                  &dup, SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                                  /*bInheritHandle=*/TRUE, 0) != 0;
        bool attrInit = ok && InitializeProcThreadAttributeList(tmp, 1, 0, &attrSize) != 0;
        bool attrSet  = attrInit &&
            UpdateProcThreadAttribute(tmp, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                                      &dup, sizeof(HANDLE), nullptr, nullptr) != 0;
        if (attrSet) {
            pickerSelf = dup;
            attrList = tmp;
            six.lpAttributeList = attrList;
            cmdLine += L" --picker-handle " +
                       std::to_wstring(reinterpret_cast<uintptr_t>(dup));
        } else {
            // Plumbing failed — tear down partial state and spawn plainly (no handle, no arg).
            if (attrInit) DeleteProcThreadAttributeList(tmp);
            if (dup) CloseHandle(dup);
        }
    }

    // Launch new instance. With a picker handle: inherit EXACTLY it (extended startupinfo).
    // Without: the original plain spawn (bInheritHandles=FALSE), byte-for-byte prior behavior.
    STARTUPINFOW siPlain = { sizeof(siPlain) };
    PROCESS_INFORMATION pi{};
    const BOOL   inheritHandles = pickerSelf ? TRUE : FALSE;
    const DWORD  createFlags    = pickerSelf ? EXTENDED_STARTUPINFO_PRESENT : 0;
    LPSTARTUPINFOW siPtr = pickerSelf ? &six.StartupInfo : &siPlain;

    BOOL launched = CreateProcessW(
        NULL,
        const_cast<LPWSTR>(cmdLine.c_str()),
        NULL, NULL, inheritHandles,
        createFlags, NULL, NULL,
        siPtr, &pi);

    if (attrList) DeleteProcThreadAttributeList(attrList);
    if (pickerSelf) CloseHandle(pickerSelf);   // child inherited its own independent reference

    if (launched) {
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);
        LOG_INFO_PM(LogFmt() << "🚀 Launched new instance with profile: " << profileId);
        return true;
    } else {
        LOG_ERROR_PM(LogFmt() << "❌ Failed to launch new instance: " << GetLastError());
        return false;
    }
#elif defined(__APPLE__)
    // Get the executable path and walk up to the .app bundle
    // Executable: .../HodosBrowser.app/Contents/MacOS/HodosBrowser
    char exePath[PATH_MAX];
    uint32_t size = sizeof(exePath);
    if (_NSGetExecutablePath(exePath, &size) != 0) {
        LOG_ERROR_PM(LogFmt() << "❌ Failed to get executable path");
        return false;
    }

    // Strip /Contents/MacOS/HodosBrowser (3 path components) to get .app bundle
    std::string appPath(exePath);
    for (int i = 0; i < 3; i++) {
        size_t pos = appPath.rfind('/');
        if (pos != std::string::npos) appPath = appPath.substr(0, pos);
    }

    // Verify this looks like an .app bundle
    if (appPath.find(".app") == std::string::npos) {
        LOG_ERROR_PM(LogFmt() << "❌ Could not find .app bundle in path: " << appPath);
        return false;
    }

    // F5: launch via posix_spawn with an explicit argv array — NO shell, so the
    // (already-validated) profileId cannot be interpreted as a command even if a
    // future change weakened the validation. `open -n -a <app> --args <arg…>`.
    // `open` uses Launch Services which does NOT inherit env vars, so forward
    // HODOS_DEV and HODOS_MAC_DEV_FLAGS via --env (macOS 12.3+) for dev builds.
    std::string profileArg = "--profile=" + profileId;
    std::vector<const char*> argVec = {
        "/usr/bin/open", "-n", "-a", appPath.c_str()
    };
    const char* hodosDev = getenv("HODOS_DEV");
    const char* hodosFlags = getenv("HODOS_MAC_DEV_FLAGS");
    std::string envDev, envFlags;
    if (hodosDev) {
        envDev = std::string("HODOS_DEV=") + hodosDev;
        argVec.push_back("--env");
        argVec.push_back(envDev.c_str());
    }
    if (hodosFlags) {
        envFlags = std::string("HODOS_MAC_DEV_FLAGS=") + hodosFlags;
        argVec.push_back("--env");
        argVec.push_back(envFlags.c_str());
    }
    argVec.push_back("--args");
    argVec.push_back(profileArg.c_str());
    argVec.push_back(nullptr);
    const char** argv = argVec.data();
    pid_t pid;
    int spawn_rc = posix_spawn(&pid, "/usr/bin/open", nullptr, nullptr,
                               const_cast<char* const*>(argv), environ);
    if (spawn_rc != 0) {
        LOG_ERROR_PM(LogFmt() << "❌ Failed to spawn /usr/bin/open: " << strerror(spawn_rc));
        return false;
    }
    // `open` delegates to Launch Services and returns quickly. CEF's SIGCHLD
    // handler often reaps the child before we call waitpid, causing ECHILD.
    // posix_spawn already confirmed the exec succeeded, so treat any
    // waitpid outcome as success — Launch Services handles the rest.
    int status = 0;
    pid_t rc = waitpid(pid, &status, 0);
    if (rc < 0 && errno == ECHILD) {
        LOG_INFO_PM(LogFmt() << "🚀 Launched new instance with profile: " << profileId
                             << " (reaped by SIGCHLD handler)");
    } else if (rc > 0 && WIFEXITED(status) && WEXITSTATUS(status) != 0) {
        LOG_WARNING_PM(LogFmt() << "⚠️ open returned exit code " << WEXITSTATUS(status)
                                << " for profile '" << profileId << "' — proceeding (Launch Services may still succeed)");
    } else {
        LOG_INFO_PM(LogFmt() << "🚀 Launched new instance with profile: " << profileId);
    }
    return true;
#else
    LOG_ERROR_PM(LogFmt() << "❌ LaunchWithProfile not implemented for this platform");
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
    return "";  // Empty = no --profile flag; caller uses defaultProfileId
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
        if (profileW.empty()) return "";
        int size_needed = WideCharToMultiByte(CP_UTF8, 0, profileW.c_str(), (int)profileW.size(), NULL, 0, NULL, NULL);
        std::string result(size_needed, 0);
        WideCharToMultiByte(CP_UTF8, 0, profileW.c_str(), (int)profileW.size(), &result[0], size_needed, NULL, NULL);
        return result;
#else
        return std::string(profileW.begin(), profileW.end());
#endif
    }
    return "";  // Empty = no --profile flag; caller uses defaultProfileId
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

    // ⛔ ALSO scan the filesystem. DeleteProfile deliberately does NOT remove files
    // ("Not deleting files for safety"), so a deleted profile's directory outlives
    // its registry entry. Scanning only `profiles_` therefore frees the id for
    // reuse, and because `profile.path = profile.id`, the next profile created
    // ADOPTS the deleted profile's data wholesale — history, cookies, and the
    // Chromium content settings that carry permission grants. Observed 2026-08-24:
    // a new profile named "Profile_2" took id Profile_4 and inherited ~200 MB.
    // An id is only free if BOTH the registry and the disk say so.
    try {
        for (const auto& entry : fs::directory_iterator(app_data_path_)) {
            if (!entry.is_directory()) continue;
            const std::string dirName = entry.path().filename().string();
            if (dirName.rfind("Profile_", 0) != 0) continue;
            try {
                const int num = std::stoi(dirName.substr(8));
                if (num >= maxNum) maxNum = num + 1;
            } catch (...) {}
        }
    } catch (const std::exception& e) {
        // A scan failure must not hand back a possibly-colliding id silently.
        LOG_WARNING_PM(LogFmt() << "⚠️ Profile id scan failed (" << e.what()
                                << ") — id may collide with an orphaned directory");
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
