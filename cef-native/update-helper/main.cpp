// hodos-update-helper.exe — external rollback-supervisor (commit 6b).
//
// AUTOUPDATE_6B_SUPERVISOR_DESIGN.md §9 v3. The ONLY process that both runs after
// the install AND can overwrite a non-running HodosBrowser.exe — so it owns the
// apply transaction (Phase B install -> integrity -> launch -> health) and, on
// failure, the DB-first crash-atomic rollback (Phase C). Also the browser-
// independent recovery target invoked as `--resume` from a per-user RunOnce.
//
// 6b.1 (THIS commit): the standalone exe scaffold — arg parsing, CWD relocation
// out of the working area, self-contained logging, the two-MODE lock, the global
// state read, and the normal-vs-resume dispatch. The Phase B/C/E transaction +
// the filesystem restore primitives land in 6b.2; this exe is NOT spawned by
// anything until 6c wires Phase A, so it is inert in the shipped product.
//
// Tiny + low-churn by design (it must keep working across browser versions): only
// reuses the project's PURE update helpers (UpdateApply / UpdateStager / UpdateLock
// / AppPaths) + Win32; it never links CEF.

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>

#include <cstdlib>
#include <fstream>
#include <map>
#include <string>

#include "core/AppPaths.h"
#include "core/UpdateApply.h"
#include "core/UpdateFs.h"
#include "core/UpdateLock.h"
#include "splash.h"
#include "transaction.h"

namespace {

// ---- self-contained logging (the browser's Logger is not available here) -----
// Appends to <update>\pending\helper\helper.log. Best-effort; never throws.
std::string g_log_path;

std::string NowStampUtc() {
    SYSTEMTIME st;
    GetSystemTime(&st);
    char buf[32];
    wsprintfA(buf, "%04d-%02d-%02dT%02d:%02d:%02d.%03dZ",
              st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond, st.wMilliseconds);
    return buf;
}

void Log(const std::string& msg) {
    if (g_log_path.empty()) return;
    std::ofstream f(g_log_path, std::ios::app);
    if (f) f << "[" << NowStampUtc() << "] " << msg << "\n";
}

// ---- minimal arg parsing -----------------------------------------------------
// Accepts: --resume (flag); --app-dir, --update-dir, --installer, --from-build,
// --to-build, --health-timeout, --bootstrap-handle <value>. Unknown args ignored.
std::map<std::string, std::string> ParseArgs(int argc, wchar_t** argv, bool& isResume) {
    std::map<std::string, std::string> out;
    isResume = false;
    for (int i = 1; i < argc; ++i) {
        std::wstring w(argv[i]);
        int len = WideCharToMultiByte(CP_UTF8, 0, w.c_str(), -1, nullptr, 0, nullptr, nullptr);
        std::string a(len > 0 ? len - 1 : 0, '\0');
        if (len > 0) WideCharToMultiByte(CP_UTF8, 0, w.c_str(), -1, &a[0], len, nullptr, nullptr);
        if (a == "--resume") { isResume = true; continue; }
        if (a.rfind("--", 0) == 0 && i + 1 < argc) {
            std::wstring vw(argv[++i]);
            int vlen = WideCharToMultiByte(CP_UTF8, 0, vw.c_str(), -1, nullptr, 0, nullptr, nullptr);
            std::string v(vlen > 0 ? vlen - 1 : 0, '\0');
            if (vlen > 0) WideCharToMultiByte(CP_UTF8, 0, vw.c_str(), -1, &v[0], vlen, nullptr, nullptr);
            out[a.substr(2)] = v;
        }
    }
    return out;
}

std::wstring Widen(const std::string& s) {
    if (s.empty()) return L"";
    int n = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, nullptr, 0);
    std::wstring w(n > 0 ? n - 1 : 0, L'\0');
    if (n > 0) MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, &w[0], n);
    return w;
}

}  // namespace

int wmain(int argc, wchar_t** argv) {
    // CWD OUT of the working area (V3 / M5) so the helper never blocks deletion of
    // its own running image's parent during the SUCCESS cleanup.
    {
        wchar_t tmp[MAX_PATH];
        DWORD n = GetTempPathW(MAX_PATH, tmp);
        if (n > 0 && n < MAX_PATH) SetCurrentDirectoryW(tmp);
    }

    // --splash-preview: show the "Hodos is updating…" indicator for ~6s then exit — a quick
    // visual check of the apply splash without running a full apply cycle.
    for (int i = 1; i < argc; ++i) {
        if (std::wstring(argv[i]) == L"--splash-preview") {
            UpdateSplash splash;
            Sleep(6000);
            return 0;
        }
    }

    bool isResume = false;
    std::map<std::string, std::string> args = ParseArgs(argc, argv, isResume);

    const std::string helperDir = AppPaths::GetHelperStageDir();
    if (!helperDir.empty()) g_log_path = helperDir + "\\helper.log";
    hodos::helper::SetLogger(&Log);
    Log(std::string("hodos-update-helper start (") + (isResume ? "--resume" : "apply") + ")");

    // The owner lock is the single-flight for EVERY entry point (V3-6).
    hodos::UpdateLockOwner lock;
    auto lh = args.find("lock-handle");
    if (lh != args.end()) {
        // APPLY path (6c.2): the bootstrap inherited an OPEN owner handle to us via
        // PROC_THREAD_ATTRIBUTE_HANDLE_LIST (same handle value in the child). ADOPT it —
        // a fresh share=0 open here would SHARING_VIOLATION against our own inherited
        // handle (the 6c carry-forward).
        HANDLE inherited = reinterpret_cast<HANDLE>(
            static_cast<uintptr_t>(strtoull(lh->second.c_str(), nullptr, 10)));
        if (inherited == nullptr || inherited == INVALID_HANDLE_VALUE) {
            Log("inherited --lock-handle is invalid — exiting");
            return 0;
        }
        lock.Adopt(inherited);
    }

    // --update-dir overrides the working area (the rig's temp sandbox; production
    // omits it -> the real %LOCALAPPDATA%\…\update). Resolve the lock + apply paths
    // from it so main and the transaction (ResolvePaths) agree.
    std::string updateDir;
    if (auto ud = args.find("update-dir"); ud != args.end() && !ud->second.empty()) updateDir = ud->second;
    const std::string lockPath = updateDir.empty() ? AppPaths::GetUpdateLockPath() : (updateDir + "\\update.lock");
    const std::string pendingDir = updateDir.empty() ? AppPaths::GetPendingUpdateDir() : (updateDir + "\\pending");

    if (lh == args.end()) {
        // --resume / standalone: open it fresh (bounded retry rides out a probe's
        // open-close / delete-pending window — RISK-B).
        if (lockPath.empty() || !lock.AcquireWithRetry(Widen(lockPath))) {
            Log("Could not acquire owner lock (another supervisor live, or no update dir) — exiting");
            return 0;  // benign: another owner is handling the transaction
        }
    }

    // Read the durable transaction state (WIDE-safe, F4). Missing/corrupt => no-op.
    hodos::ApplyRecord rec;
    const std::string applyPath = pendingDir.empty() ? "" : pendingDir + "\\apply.json";
    {
        std::string content;
        if (!applyPath.empty() && hodos::updatefs::ReadFileAll(Widen(applyPath), content)) {
            if (!hodos::ParseApplyRecord(content, rec)) {
                Log("apply.json present but unparseable — treating as no-op");
                rec = hodos::ApplyRecord{};
            }
        } else {
            Log("no apply.json — nothing to do");
        }
    }

    const int rc = isResume ? hodos::helper::RunResume(args, lock, rec)
                            : hodos::helper::RunApplyTransaction(args, lock, rec);
    Log("hodos-update-helper exit rc=" + std::to_string(rc));
    return rc;
}
