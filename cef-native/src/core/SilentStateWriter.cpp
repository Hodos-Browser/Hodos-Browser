// SilentStateWriter.cpp — see SilentStateWriter.h + AUTOUPDATE_SILENT_STATE_WRITER_DESIGN.md.

#include "../../include/core/SilentStateWriter.h"

namespace hodos {

// --- Pure helpers (cross-platform, unit-tested) ------------------------------

int UpdateModeRank(const std::string& mode) {
    if (mode == "notify") return 1;
    if (mode == "silent") return 2;
    return 0;  // "off" and anything unknown/empty -> safest
}

std::string MoreConservativeMode(const std::string& a, const std::string& b) {
    return UpdateModeRank(a) <= UpdateModeRank(b) ? a : b;
}

bool ComputeSilentEligibility(UpdateState& state, const std::string& mode) {
    const bool wantSilent = (mode == "silent");
    if (state.silent == wantSilent) return false;  // no change
    state.silent = wantSilent;  // ONLY touch `silent`; never clear `paused` (that is commit #2)
    return true;
}

}  // namespace hodos

#ifdef _WIN32

#include "../../include/core/AppPaths.h"
#include "../../include/core/UpdateFs.h"
#include "../../include/core/UpdateLock.h"
#include "../../include/core/Logger.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

// Module ID 10 (shared with SettingsManager) — this is the settings->update bridge.
#define LOG_INFO_SSW(msg) Logger::Log(msg, 1, 10)
#define LOG_WARN_SSW(msg) Logger::Log(msg, 2, 10)

namespace hodos {

static std::wstring WidenUtf8(const std::string& s) {
    if (s.empty()) return std::wstring();
    int n = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), static_cast<int>(s.size()), nullptr, 0);
    if (n <= 0) return std::wstring();
    std::wstring w(static_cast<size_t>(n), L'\0');
    MultiByteToWideChar(CP_UTF8, 0, s.c_str(), static_cast<int>(s.size()), &w[0], n);
    return w;
}

void MirrorSilentEligibility(const std::string& mode) {
    const std::string statePath = AppPaths::GetUpdateStatePath();
    if (statePath.empty()) {
        LOG_WARN_SSW("Silent mirror: update-state path unavailable (no LOCALAPPDATA) — skip");
        return;
    }

    // Defense-in-depth: if an apply owner holds update.lock, the helper is the
    // authoritative writer of update-state.json — skip (we refresh on a later boot).
    // Permissive PROBE only: never creates or mutates the lock (so it can't trip the
    // honor-at-launch defer that other launches rely on). In practice a normal launch
    // already deferred at the honor-probe if the lock was held, and the picker /
    // health-probe launches are guarded out by the caller.
    const std::string lockPath = AppPaths::GetUpdateLockPath();
    if (!lockPath.empty() && UpdateLockIsHeld(WidenUtf8(lockPath))) {
        LOG_INFO_SSW("Silent mirror: apply in progress (update.lock held) — skip (helper authoritative)");
        return;
    }

    const std::wstring statePathW = WidenUtf8(statePath);

    UpdateState state;  // fail-safe defaults: silent=false, paused=false, ...
    std::string content;
    if (updatefs::ReadFileAll(statePathW, content)) {
        ParseUpdateState(content, state);  // preserve existing fields; garbage -> defaults
    }

    if (!ComputeSilentEligibility(state, mode)) {
        // No change — avoid needless churn (esp. notify/off + absent file: default
        // silent=false already matches, so we never create a spurious file).
        return;
    }

    const std::string updateDir = AppPaths::GetUpdateDir();
    if (!updateDir.empty()) updatefs::EnsureDirExists(WidenUtf8(updateDir));

    if (!updatefs::WriteFileAtomic(statePathW, SerializeUpdateState(state))) {
        LOG_WARN_SSW("Silent mirror: failed to write update-state.json");
        return;
    }
    LOG_INFO_SSW(std::string("Silent mirror: update eligibility silent=") + (state.silent ? "true" : "false"));
}

}  // namespace hodos

#endif  // _WIN32
