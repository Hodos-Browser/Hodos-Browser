#pragma once
#include <cstdlib>
#include <string>
#include <iostream>
#ifdef _WIN32
#include <filesystem>
#endif

namespace AppPaths {

inline std::string GetAppDirName() {
    const char* dev = std::getenv("HODOS_DEV");
    if (dev && std::string(dev) == "1") {
        return "HodosBrowserDev";
    }
    return "HodosBrowser";
}

#ifdef _WIN32
/// Read a Windows environment variable as UTF-8 (wide _wgetenv -> UTF-8). Using
/// the narrow getenv would return CP_ACP, which the helper then mis-decodes as
/// UTF-8 -> mojibake paths for non-ASCII usernames (F3). u8string() yields UTF-8
/// that the helper's CP_UTF8 widen round-trips correctly. "" if unset.
inline std::string EnvUtf8_(const wchar_t* name) {
    const wchar_t* v = _wgetenv(name);
    if (!v || !*v) return "";
    const auto u8 = std::filesystem::path(v).u8string();  // C++17: std::string (UTF-8)
    return std::string(u8.begin(), u8.end());
}

/// The Inno install root `{app}` = %LOCALAPPDATA%\<HodosBrowser|HodosBrowserDev>
/// (Local; holds HodosBrowser.exe + the children + libcef.dll + paks). The update
/// working area is its `update\` subdir. "" if LOCALAPPDATA is unavailable.
inline std::string GetAppInstallDir() {
    const std::string localAppData = EnvUtf8_(L"LOCALAPPDATA");
    if (localAppData.empty()) return "";
    return localAppData + "\\" + GetAppDirName();
}

/// Root of the auto-update working area. **All** update state lives under this
/// ONE subtree — `update\` — which is deliberately OUTSIDE the `{app}` install
/// root's backed-up file set (commit 6b / V3-10): the rollback backup copies the
/// `[Files]` closure with root-level (non-recursive) globs, so a dedicated
/// `update\` subdir is never captured, and the installer/uninstaller never touch
/// it. Under %LOCALAPPDATA% (NON-roaming: no domain-profile syncing a ~95 MB
/// installer; same-volume with `{app}` for the §D.5 orphan-only rename).
/// Dev/prod-namespaced via GetAppDirName(). Returns "" if LOCALAPPDATA is
/// unavailable — callers MUST skip (never fall back to a relative path).
inline std::string GetUpdateDir() {
    const std::string localAppData = EnvUtf8_(L"LOCALAPPDATA");
    if (localAppData.empty()) return "";
    return localAppData + "\\" + GetAppDirName() + "\\update";
}

/// Staging dir for the downloaded installer + markers (commit 4 download->stage;
/// commit 6 apply-on-next-launch). Single source of truth shared by the staging
/// and apply paths so they cannot diverge. "" if GetUpdateDir() is unavailable.
inline std::string GetPendingUpdateDir() {
    std::string root = GetUpdateDir();
    return root.empty() ? "" : root + "\\pending";
}

/// Full pre-apply `{app}` + money-DB rollback backup (commit 6b). Under pending\.
inline std::string GetRollbackDir() {
    std::string p = GetPendingUpdateDir();
    return p.empty() ? "" : p + "\\rollback";
}

/// Where the supervisor exe is copied OUT of `{app}` before the installer runs
/// (commit 6b), so the installer can freely replace `{app}\hodos-update-helper.exe`.
inline std::string GetHelperStageDir() {
    std::string p = GetPendingUpdateDir();
    return p.empty() ? "" : p + "\\helper";
}

/// Fleet-wide silent-update lock (commit 6a / OD-C). The apply supervisor
/// (commit 6b) creates+holds this (exclusive handle) for the WHOLE install ->
/// relaunch -> health window; every NORMAL launch (profile OR picker) defers
/// while a live owner holds it. Lives in update\ (its own subtree), so a "clear
/// the stage" sweep of pending\ can't delete an in-flight lock. "" if unavailable
/// (caller treats that as "no lock", proceeds).
inline std::string GetUpdateLockPath() {
    std::string root = GetUpdateDir();
    return root.empty() ? "" : root + "\\update.lock";
}

/// GLOBAL cross-profile update state (commit 6b): schemaVer, silent, paused,
/// highWaterBuild, signerThumbprint, lastFailure, rescanAfterRollback. Lives in
/// update\ (outside the backed-up `{app}` set, V3-10/F8). "" if unavailable.
inline std::string GetUpdateStatePath() {
    std::string root = GetUpdateDir();
    return root.empty() ? "" : root + "\\update-state.json";
}

/// The money DB (commit 6b / V3-1). It is in %APPDATA% (**ROAMING**, alongside
/// the wallet), NOT under `{app}` (Local) — getting this wrong makes a rollback
/// snapshot the wrong file and silently fail. Dev/prod-namespaced. "" if APPDATA
/// is unavailable. Returns the DIRECTORY; the DB file is `<dir>\wallet.db`
/// (+`-wal`/`-shm`).
inline std::string GetWalletDir() {
    const std::string appData = EnvUtf8_(L"APPDATA");
    if (appData.empty()) return "";
    return appData + "\\" + GetAppDirName() + "\\wallet";
}

/// Session-namespace mutex name marking ANY live HodosBrowser.exe (all profiles +
/// the picker) for the auto-update all-instances-gone gate (WINDOWS_AUTOUPDATE_PLAN
/// §D.0, commit 6a). **Local\ (NOT Global\):** a PrivilegesRequired=lowest install
/// runs as a standard user that cannot reliably create Global\ objects
/// (SeCreateGlobalPrivilege), and every profile runs in the SAME session, so the
/// session namespace already sees them all (cross-session is moot — each Windows
/// user has its own per-user install + %LOCALAPPDATA%). Dev/prod-namespaced so a
/// dev build never blocks a prod silent-apply (and vice-versa). Inno's [Setup]
/// AppMutex uses the UNPREFIXED form ("HodosBrowser_AnyInstance") which resolves to
/// this same session object. ASCII-only -> the narrow->wide widening below is safe.
inline std::wstring GetInstanceMutexNameW() {
    std::string n = "Local\\" + GetAppDirName() + "_AnyInstance";
    return std::wstring(n.begin(), n.end());
}
#endif

/// Safeguard: if running from a dev build directory, require HODOS_DEV=1.
/// The installed app runs from Program Files / app bundle, so this won't trigger for users.
/// Returns true if safe to proceed, false if the process should exit.
inline bool EnforceDevSafeguard(const std::string& exe_path) {
    bool is_dev_build = (exe_path.find("build\\bin\\Release") != std::string::npos)
                     || (exe_path.find("build/bin/Release") != std::string::npos)
                     || (exe_path.find("build\\bin\\Debug") != std::string::npos)
                     || (exe_path.find("build/bin/Debug") != std::string::npos)
                     || (exe_path.find("build/bin/HodosBrowser") != std::string::npos);
    if (!is_dev_build) return true;

    const char* dev = std::getenv("HODOS_DEV");
    if (dev && std::string(dev) == "1") return true;

    std::cerr << "========================================================" << std::endl;
    std::cerr << "  DEV SAFEGUARD: HODOS_DEV=1 is not set!" << std::endl;
    std::cerr << "  Running a dev build without it would use the" << std::endl;
    std::cerr << "  production database and risk corrupting real data." << std::endl;
    std::cerr << std::endl;
    std::cerr << "  Use the launcher script instead:" << std::endl;
    std::cerr << "    Windows: .\\win_build_run.sh" << std::endl;
    std::cerr << "    Mac:     ./mac_build_run.sh" << std::endl;
    std::cerr << "========================================================" << std::endl;
    return false;
}

} // namespace AppPaths
