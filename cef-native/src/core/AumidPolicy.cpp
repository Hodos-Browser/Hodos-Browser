// cef-native/src/core/AumidPolicy.cpp
//
// Windows-only half of the AUMID policy: registering a display name for an AUMID
// that no shortcut declares. Rationale, and why a shortcut alone is insufficient,
// is in the header.

#include "../../include/core/AumidPolicy.h"

#ifdef _WIN32

#include <windows.h>

#include "../../include/core/Logger.h"

// Logger::Log(message, level, process) — level 1 = INFO, 2 = WARNING; process 0 = MAIN.
// The LOG_* macros live in cef_browser_shell.cpp and are not visible here, so this file
// follows the same local-macro convention as TaskbarProfile.cpp.
#define LOG_INFO_AUMID(msg) Logger::Log(std::string("[AUMID] ") + (msg), 1, 0)
#define LOG_WARNING_AUMID(msg) Logger::Log(std::string("[AUMID] ") + (msg), 2, 0)

namespace {

// HKCU\Software\Classes\AppUserModelId\<aumid>
std::wstring AumidKeyPath(const std::wstring& aumid) {
    return L"Software\\Classes\\AppUserModelId\\" + aumid;
}

// Narrow a wide string for the log only. AUMIDs are ASCII by construction
// (kAumidBase* plus a validated profile id), so this is lossless here.
std::string ForLog(const std::wstring& w) {
    return std::string(w.begin(), w.end());
}

bool WriteStringValue(HKEY key, const wchar_t* name, const std::wstring& value) {
    const DWORD bytes = static_cast<DWORD>((value.size() + 1) * sizeof(wchar_t));
    return RegSetValueExW(key, name, 0, REG_SZ,
                          reinterpret_cast<const BYTE*>(value.c_str()),
                          bytes) == ERROR_SUCCESS;
}

}  // namespace

namespace hodos {

bool RegisterAumidDisplayName(const std::wstring& aumid,
                              const std::wstring& displayName,
                              const std::wstring& iconPath) {
    if (aumid.empty() || displayName.empty()) {
        LOG_WARNING_AUMID("Refusing to register an empty AUMID or display name");
        return false;
    }

    HKEY key = nullptr;
    const LSTATUS st = RegCreateKeyExW(HKEY_CURRENT_USER, AumidKeyPath(aumid).c_str(),
                                       0, nullptr, REG_OPTION_NON_VOLATILE,
                                       KEY_SET_VALUE, nullptr, &key, nullptr);
    if (st != ERROR_SUCCESS || !key) {
        // Best-effort by design: an unnamed taskbar button is cosmetic, and refusing
        // to start over it would be a far worse trade.
        LOG_WARNING_AUMID("Could not open registry key for " + ForLog(aumid) +
                          " (err=" + std::to_string(st) + ") — taskbar button will "
                          "fall back to the exe filename");
        return false;
    }

    // "ApplicationName" is the value Windows reads for the taskbar/jump-list title.
    bool ok = WriteStringValue(key, L"ApplicationName", displayName);
    if (!ok) {
        LOG_WARNING_AUMID("Failed to write ApplicationName for " + ForLog(aumid));
    }

    // Optional: the icon shown alongside. Absent is not a failure of the naming fix,
    // so it does not affect the return value.
    if (!iconPath.empty()) {
        if (!WriteStringValue(key, L"ApplicationIcon", iconPath)) {
            LOG_WARNING_AUMID("Failed to write ApplicationIcon for " + ForLog(aumid));
        }
    }

    RegCloseKey(key);

    if (ok) {
        LOG_INFO_AUMID("Registered display name for " + ForLog(aumid) + " = \"" +
                       ForLog(displayName) + "\"");
    }
    return ok;
}

bool UnregisterAumidDisplayName(const std::wstring& aumid) {
    if (aumid.empty()) {
        return false;
    }
    const LSTATUS st = RegDeleteTreeW(HKEY_CURRENT_USER, AumidKeyPath(aumid).c_str());
    if (st == ERROR_SUCCESS) {
        LOG_INFO_AUMID("Removed display-name registration for " + ForLog(aumid));
        return true;
    }
    // ERROR_FILE_NOT_FOUND is the ordinary case for a profile that never ran.
    if (st != ERROR_FILE_NOT_FOUND) {
        LOG_WARNING_AUMID("Could not remove registration for " + ForLog(aumid) +
                          " (err=" + std::to_string(st) + ")");
    }
    return false;
}

}  // namespace hodos

#endif  // _WIN32
