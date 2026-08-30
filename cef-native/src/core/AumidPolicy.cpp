// cef-native/src/core/AumidPolicy.cpp
//
// Windows-only half of the AUMID policy: making sure a per-profile identity has a
// shortcut that declares it, because that — and NOT a registry value, and NOT the exe's
// version resource — is what names a taskbar button. Both alternatives were tried and
// measured to fail; see the header for the evidence.

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

bool EnsureProfileShortcut(const std::wstring& aumid,
                           const std::string& profileId,
                           const std::string& profileName) {
    if (aumid.empty() || profileId.empty()) return false;

    // The installer owns the Default profile's shortcut. Writing our own would duplicate
    // the Start Menu entry, and the duplicate would not be removed on uninstall.
    if (profileId == "Default") return false;

    // ⛔ NEVER in a dev build. A .lnk cannot carry an environment variable, so a shortcut
    // to build\bin\Release\HodosBrowser.exe launches WITHOUT HODOS_DEV=1 and the dev
    // safeguard correctly refuses to start — the user gets a "DEV SAFEGUARD" dialog.
    // Found the honest way: the owner clicked one on 2026-08-30 and got exactly that.
    // So in dev these shortcuts are INERT BY CONSTRUCTION, and creating them only
    // litters the developer's Start Menu with entries that can never work.
    // ⚠️ Guarded HERE rather than at the call site so a future caller cannot reintroduce
    // it. The production path is unaffected: an installed build needs no env var.
    if (IsDevEnv()) {
        LOG_INFO_AUMID("Dev build — skipping profile shortcut for " + ForLog(aumid) +
                       " (a .lnk cannot set HODOS_DEV=1, so it could never launch)");
        return false;
    }

    const std::wstring dir = StartMenuProgramsDir();
    if (dir.empty()) {
        LOG_WARNING_AUMID("Could not resolve the Start Menu Programs folder");
        return false;
    }
    const std::wstring lnkPath = dir + L"\\" + ProfileShortcutFileName(profileName);

    wchar_t exePath[MAX_PATH] = {0};
    if (GetModuleFileNameW(nullptr, exePath, MAX_PATH) == 0) {
        LOG_WARNING_AUMID("Could not resolve the executable path");
        return false;
    }

    // ⚠️ COM is initialised by the caller (WinMain does CoInitializeEx for taskbar work).
    // Do not initialise it here — a second, mismatched apartment on the UI thread is a
    // subtle way to break the shell integration this file exists to support.
    IShellLinkW* link = nullptr;
    HRESULT hr = CoCreateInstance(CLSID_ShellLink, nullptr, CLSCTX_INPROC_SERVER,
                                  IID_PPV_ARGS(&link));
    if (FAILED(hr) || !link) {
        LOG_WARNING_AUMID("CoCreateInstance(ShellLink) failed, hr=" + std::to_string(hr));
        return false;
    }

    const std::wstring args = L"--profile=\"" + Widen(profileId) + L"\"";
    link->SetPath(exePath);
    link->SetArguments(args.c_str());
    link->SetIconLocation(exePath, 0);
    link->SetDescription((std::wstring(kAumidDisplayName) + L" — " +
                          Widen(profileName)).c_str());

    // THE POINT OF THE WHOLE FUNCTION: stamp the AUMID so Windows can match this shortcut
    // to the running window and take the button's name from it.
    bool ok = false;
    IPropertyStore* store = nullptr;
    if (SUCCEEDED(link->QueryInterface(IID_PPV_ARGS(&store))) && store) {
        PROPVARIANT pv;
        if (SUCCEEDED(InitPropVariantFromString(aumid.c_str(), &pv))) {
            if (SUCCEEDED(store->SetValue(PKEY_AppUserModel_ID, pv)) &&
                SUCCEEDED(store->Commit())) {
                ok = true;
            }
            PropVariantClear(&pv);
        }
        store->Release();
    }

    if (ok) {
        IPersistFile* file = nullptr;
        if (SUCCEEDED(link->QueryInterface(IID_PPV_ARGS(&file))) && file) {
            ok = SUCCEEDED(file->Save(lnkPath.c_str(), TRUE));
            file->Release();
        } else {
            ok = false;
        }
    }

    link->Release();

    if (ok) {
        LOG_INFO_AUMID("Profile shortcut ensured for " + ForLog(aumid) + " -> " +
                       ForLog(lnkPath));
    } else {
        LOG_WARNING_AUMID("Failed to write profile shortcut for " + ForLog(aumid) +
                          " — its taskbar button will fall back to the exe filename");
    }
    return ok;
}

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
