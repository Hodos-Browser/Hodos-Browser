// cef-native/src/core/AumidPolicy.cpp
//
// Windows-only half of the AUMID policy.
//
// What names a taskbar button is a SHORTCUT declaring the same AUMID — not a registry
// value, and not the exe's version resource. Both alternatives were implemented or
// asserted and then measured to fail; see the header for the evidence.
//
// ⚠️ This file no longer WRITES such shortcuts. The owner rejected per-profile Start Menu
// entries on 2026-08-31 (one entry, "Hodos Browser", opening the picker). What is left
// here is the cleanup path plus the record of what was learned — see the block above
// RemoveProfileShortcut.

#include "../../include/core/AumidPolicy.h"

#ifdef _WIN32

#include <windows.h>
#include <objbase.h>
#include <shlobj.h>
#include <shobjidl.h>
#include <propkey.h>
#include <propvarutil.h>

#include <vector>

#include "../../include/core/Logger.h"
#include "../../include/core/PortConfig.h"  // hodos::IsDevEnv

// Logger::Log(message, level, process) — level 1 = INFO, 2 = WARNING; process 0 = MAIN.
// The LOG_* macros live in cef_browser_shell.cpp and are not visible here, so this file
// follows the same local-macro convention as TaskbarProfile.cpp.
#define LOG_INFO_AUMID(msg) Logger::Log(std::string("[AUMID] ") + (msg), 1, 0)
#define LOG_WARNING_AUMID(msg) Logger::Log(std::string("[AUMID] ") + (msg), 2, 0)

namespace {

// Narrow a wide string for the log only. These are ASCII by construction here.
std::string ForLog(const std::wstring& w) {
    return std::string(w.begin(), w.end());
}

std::wstring Widen(const std::string& s) {
    return std::wstring(s.begin(), s.end());
}

// %APPDATA%\Microsoft\Windows\Start Menu\Programs
// ⛔ CSIDL_PROGRAMS (per-user), never CSIDL_COMMON_PROGRAMS — a per-user install must not
// need elevation, and one user's profiles must not appear in another user's Start Menu.
std::wstring StartMenuProgramsDir() {
    PWSTR path = nullptr;
    if (FAILED(SHGetKnownFolderPath(FOLDERID_Programs, 0, nullptr, &path)) || !path) {
        if (path) CoTaskMemFree(path);
        return L"";
    }
    std::wstring result(path);
    CoTaskMemFree(path);
    return result;
}

// Strip characters that are illegal in a filename. A profile name is user-supplied, so
// this is the boundary where it becomes a path component.
std::wstring SanitizeForFileName(const std::wstring& in) {
    static const std::wstring illegal = L"\\/:*?\"<>|";
    std::wstring out;
    out.reserve(in.size());
    for (wchar_t c : in) {
        if (c < 32 || illegal.find(c) != std::wstring::npos) continue;
        out += c;
    }
    // Trailing dots and spaces are silently dropped by the filesystem — remove them here
    // so the name we compute matches the name that ends up on disk (otherwise the delete
    // path would look for a file that does not exist under that name).
    while (!out.empty() && (out.back() == L' ' || out.back() == L'.')) out.pop_back();
    return out;
}

}  // namespace

namespace hodos {

std::wstring ProfileShortcutFileName(const std::string& profileName) {
    std::wstring safe = SanitizeForFileName(Widen(profileName));
    if (safe.empty()) safe = L"Profile";
    return std::wstring(kAumidDisplayName) + L" - " + safe + L".lnk";
}

// ⛔ EnsureProfileShortcut was REMOVED 2026-08-31 at the owner's decision.
//
// It wrote one Start Menu shortcut per non-Default profile, which is what names that
// profile's taskbar button (the mechanism is real -- see the header, and it was confirmed
// live). The owner rejected the UX, not the mechanism:
//
//   "the start menu should just say Hodos Browser, nothing else and then that opens the
//    profile picker if the user has more than one profile"
//
// The picker is already the front door: ResolveStartup returns picker mode for a
// no-argument launch whenever more than one profile exists, so the single installer
// shortcut already does the right thing.
//
// ⚠️ What is NOT solved by removing it: a non-Default window's taskbar button still has
// no shortcut to take its name from and falls back to the exe filename. The owner does
// want the profile name shown there. That needs a WINDOW-level mechanism
// (PKEY_AppUserModel_RelaunchDisplayNameResource / RelaunchCommand on the window's
// property store) rather than a shortcut. UNVERIFIED -- and this file has already been
// wrong twice about what names a taskbar button, so it gets measured before it gets
// written. Tracked as P3-A5d.
//
// RemoveProfileShortcut is KEPT: it cleans up shortcuts written by the intermediate
// build on a developer machine. Nothing shipped with the creation path.

bool RemoveProfileShortcut(const std::string& profileName) {
    const std::wstring dir = StartMenuProgramsDir();
    if (dir.empty()) return false;
    const std::wstring lnkPath = dir + L"\\" + ProfileShortcutFileName(profileName);
    if (DeleteFileW(lnkPath.c_str())) {
        LOG_INFO_AUMID("Removed profile shortcut " + ForLog(lnkPath));
        return true;
    }
    // ERROR_FILE_NOT_FOUND is the ordinary case for a profile that never ran.
    const DWORD err = GetLastError();
    if (err != ERROR_FILE_NOT_FOUND && err != ERROR_PATH_NOT_FOUND) {
        LOG_WARNING_AUMID("Could not remove profile shortcut " + ForLog(lnkPath) +
                          " (err=" + std::to_string(err) + ")");
    }
    return false;
}

}  // namespace hodos

#endif  // _WIN32
