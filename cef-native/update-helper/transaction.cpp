// transaction.cpp — the supervisor's Phase B/C/E + --resume state machine (6b.2b).
// See transaction.h + AUTOUPDATE_6B_SUPERVISOR_DESIGN.md §9 v3.

#include "transaction.h"

#ifdef _WIN32

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <tlhelp32.h>
#include <winhttp.h>

#include <string>
#include <vector>

#include "core/AppPaths.h"
#include "core/UpdateApply.h"
#include "core/UpdateFs.h"

namespace fs_unused {}  // keep includes tidy

namespace hodos {
namespace helper {

namespace {

LogFn g_log = nullptr;
void L(const std::string& m) { if (g_log) g_log(m); }

// ---- narrow/wide -------------------------------------------------------------
std::wstring W(const std::string& s) {
    if (s.empty()) return L"";
    int n = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, nullptr, 0);
    std::wstring w(n > 0 ? n - 1 : 0, L'\0');
    if (n > 0) MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, &w[0], n);
    return w;
}
std::string N(const std::wstring& w) {
    if (w.empty()) return "";
    int n = WideCharToMultiByte(CP_UTF8, 0, w.c_str(), -1, nullptr, 0, nullptr, nullptr);
    std::string s(n > 0 ? n - 1 : 0, '\0');
    if (n > 0) WideCharToMultiByte(CP_UTF8, 0, w.c_str(), -1, &s[0], n, nullptr, nullptr);
    return s;
}

std::wstring Arg(const std::map<std::string, std::string>& a, const char* k) {
    auto it = a.find(k);
    return it == a.end() ? L"" : W(it->second);
}

// ---- timing knobs (V3-15: generous; first cut for soak) ----------------------
constexpr DWORD kBootstrapWaitMs   = 15000;
constexpr DWORD kUnlockPollMs      = 15000;
constexpr DWORD kChildShutdownMs   = 10000;
constexpr DWORD kInstallerWaitMs   = 120000;
constexpr DWORD kHealthWaitMs      = 120000;
constexpr DWORD kHttpTimeoutMs     = 2000;

// ---- paths -------------------------------------------------------------------
struct Paths {
    std::wstring appDir, walletDir, pendingDir, rollbackDir, applyPath, statePath;
    std::wstring browserExe, libcef;
};
Paths ResolvePaths(const std::map<std::string, std::string>& args) {
    Paths p;
    p.appDir = Arg(args, "app-dir");
    if (p.appDir.empty()) p.appDir = W(AppPaths::GetAppInstallDir());

    // --wallet-dir / --update-dir are arg overrides used ONLY by the fault-injection
    // rig to run in a fully isolated temp sandbox (production omits them -> the real
    // %APPDATA%\…\wallet and %LOCALAPPDATA%\…\update paths). The bootstrap already
    // passes --app-dir/--update-dir; --wallet-dir is rig-only.
    p.walletDir = Arg(args, "wallet-dir");
    if (p.walletDir.empty()) p.walletDir = W(AppPaths::GetWalletDir());

    std::wstring updateDir = Arg(args, "update-dir");
    if (updateDir.empty()) {
        p.pendingDir = W(AppPaths::GetPendingUpdateDir());
        p.statePath = W(AppPaths::GetUpdateStatePath());
    } else {
        p.pendingDir = updateDir + L"\\pending";
        p.statePath = updateDir + L"\\update-state.json";
    }
    p.rollbackDir = p.pendingDir.empty() ? L"" : p.pendingDir + L"\\rollback";
    p.applyPath = p.pendingDir.empty() ? L"" : p.pendingDir + L"\\apply.json";
    p.browserExe = p.appDir + L"\\HodosBrowser.exe";
    p.libcef = p.appDir + L"\\libcef.dll";
    return p;
}

// ---- apply.json / update-state persistence (atomic) --------------------------
bool WriteApply(const Paths& p, ApplyRecord& rec, ApplyPhase phase, const std::string& failure = "") {
    rec.phase = phase;
    if (!failure.empty()) rec.failureReason = failure;
    if (p.applyPath.empty()) return false;
    return updatefs::WriteFileAtomic(p.applyPath, SerializeApplyRecord(rec));
}
bool ReadApply(const Paths& p, ApplyRecord& out) {
    std::string c;
    if (p.applyPath.empty() || !updatefs::ReadFileAll(p.applyPath, c)) return false;
    return ParseApplyRecord(c, out);
}
UpdateState ReadStateOrDefault(const Paths& p) {
    UpdateState s;
    std::string c;
    if (!p.statePath.empty() && updatefs::ReadFileAll(p.statePath, c)) ParseUpdateState(c, s);
    return s;
}
void WriteState(const Paths& p, const UpdateState& s) {
    if (!p.statePath.empty()) updatefs::WriteFileAtomic(p.statePath, SerializeUpdateState(s));
}

// ---- process / lock helpers --------------------------------------------------
// True if the path is exclusively openable for write (== not image-locked). A
// non-existent file counts as "free" (nothing holds it).
bool ExclusiveOpenable(const std::wstring& path) {
    DWORD attr = GetFileAttributesW(path.c_str());
    if (attr == INVALID_FILE_ATTRIBUTES) return true;  // absent => not locked
    HANDLE h = CreateFileW(path.c_str(), GENERIC_WRITE, 0, nullptr,
                           OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) return false;
    CloseHandle(h);
    return true;
}
bool PollUnlocked(const std::vector<std::wstring>& paths, DWORD timeoutMs) {
    const DWORD start = GetTickCount();
    for (;;) {
        bool all = true;
        for (const auto& p : paths) if (!ExclusiveOpenable(p)) { all = false; break; }
        if (all) return true;
        if (GetTickCount() - start > timeoutMs) return false;
        Sleep(250);
    }
}

// Count live HodosBrowser.exe processes whose image path is under appDir (M6 — by
// full module path, so a dev build elsewhere isn't counted), excluding self's
// parentage. Returns -1 on snapshot failure (caller treats as "cannot prove gone").
int CountSiblingBrowsers(const std::wstring& appDir) {
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap == INVALID_HANDLE_VALUE) return -1;
    std::wstring needle = appDir + L"\\HodosBrowser.exe";
    for (auto& c : needle) c = (wchar_t)towlower(c);
    int count = 0;
    PROCESSENTRY32W pe{}; pe.dwSize = sizeof(pe);
    if (Process32FirstW(snap, &pe)) {
        do {
            if (_wcsicmp(pe.szExeFile, L"HodosBrowser.exe") != 0) continue;
            HANDLE h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pe.th32ProcessID);
            if (!h) continue;
            wchar_t path[MAX_PATH]; DWORD sz = MAX_PATH;
            if (QueryFullProcessImageNameW(h, 0, path, &sz)) {
                std::wstring lp(path); for (auto& c : lp) c = (wchar_t)towlower(c);
                if (lp == needle) ++count;
            }
            CloseHandle(h);
        } while (Process32NextW(snap, &pe));
    }
    CloseHandle(snap);
    return count;
}

// Best-effort graceful child shutdown: POST /shutdown with a hard timeout so a
// poisoned/wedged wallet can't hang us (E.3). Never throws; ignores the result.
void HttpPostShutdown(INTERNET_PORT port) {
    HINTERNET sess = WinHttpOpen(L"hodos-update-helper/1.0", WINHTTP_ACCESS_TYPE_NO_PROXY,
                                 WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
    if (!sess) return;
    WinHttpSetTimeouts(sess, kHttpTimeoutMs, kHttpTimeoutMs, kHttpTimeoutMs, kHttpTimeoutMs);
    HINTERNET conn = WinHttpConnect(sess, L"127.0.0.1", port, 0);
    if (conn) {
        HINTERNET req = WinHttpOpenRequest(conn, L"POST", L"/shutdown", nullptr,
                                           WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
        if (req) {
            if (WinHttpSendRequest(req, WINHTTP_NO_ADDITIONAL_HEADERS, 0,
                                   WINHTTP_NO_REQUEST_DATA, 0, 0, 0)) {
                WinHttpReceiveResponse(req, nullptr);
            }
            WinHttpCloseHandle(req);
        }
        WinHttpCloseHandle(conn);
    }
    WinHttpCloseHandle(sess);
}

// Spawn a process. `cmdline` is mutable per CreateProcessW contract. Returns the
// process HANDLE (caller closes) or nullptr. `detached` => no window, own group.
HANDLE Spawn(const std::wstring& cmdline, bool detached, bool waitInherit = false) {
    std::vector<wchar_t> buf(cmdline.begin(), cmdline.end());
    buf.push_back(L'\0');
    STARTUPINFOW si{}; si.cb = sizeof(si);
    PROCESS_INFORMATION pi{};
    DWORD flags = CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT;
    if (detached) flags |= DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB;
    if (!CreateProcessW(nullptr, buf.data(), nullptr, nullptr, FALSE, flags,
                        nullptr, nullptr, &si, &pi)) {
        // CREATE_BREAKAWAY_FROM_JOB fails with ACCESS_DENIED if the parent's job
        // forbids breakaway (M2) — retry without it.
        if (detached && GetLastError() == ERROR_ACCESS_DENIED) {
            flags &= ~CREATE_BREAKAWAY_FROM_JOB;
            std::vector<wchar_t> b2(cmdline.begin(), cmdline.end()); b2.push_back(L'\0');
            if (!CreateProcessW(nullptr, b2.data(), nullptr, nullptr, FALSE, flags,
                                nullptr, nullptr, &si, &pi)) return nullptr;
        } else {
            return nullptr;
        }
    }
    (void)waitInherit;
    CloseHandle(pi.hThread);
    return pi.hProcess;
}

void TaskkillTree(DWORD pid) {
    wchar_t cmd[128];
    wsprintfW(cmd, L"taskkill /F /T /PID %lu", pid);
    HANDLE h = Spawn(cmd, false);
    if (h) { WaitForSingleObject(h, 10000); CloseHandle(h); }
}

// ---- RunOnce (per-user, no admin) -------------------------------------------
std::wstring SelfPathResumeCmd() {
    wchar_t self[MAX_PATH]; GetModuleFileNameW(nullptr, self, MAX_PATH);
    return std::wstring(L"\"") + self + L"\" --resume";
}
void ArmRunOnce() {
    HKEY k;
    if (RegCreateKeyExW(HKEY_CURRENT_USER,
            L"Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce", 0, nullptr,
            0, KEY_SET_VALUE, nullptr, &k, nullptr) == ERROR_SUCCESS) {
        std::wstring cmd = SelfPathResumeCmd();
        RegSetValueExW(k, L"HodosUpdateResume", 0, REG_SZ,
                       reinterpret_cast<const BYTE*>(cmd.c_str()),
                       static_cast<DWORD>((cmd.size() + 1) * sizeof(wchar_t)));
        RegCloseKey(k);
    }
}
void ClearRunOnce() {
    HKEY k;
    if (RegOpenKeyExW(HKEY_CURRENT_USER,
            L"Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce", 0, KEY_SET_VALUE, &k)
        == ERROR_SUCCESS) {
        RegDeleteValueW(k, L"HodosUpdateResume");
        RegCloseKey(k);
    }
}

// ---- the integrity gate (B4 / V3-8) -----------------------------------------
// Verify the freshly-installed {app} tree against the build-time expected-new
// manifest — but ONLY after verifying the manifest's OWN detached Ed25519 signature
// (sidecar <manifest>.ed) with the embedded key, so a local attacker can't swap
// both the new exe AND the manifest. An ABSENT manifest is skipped (a real 6b.3
// release always ships+signs one; the silent path is compiled-off/unwired until
// then). A PRESENT manifest is fail-closed: bad/missing signature => FAIL.
bool IntegrityGate(const Paths& p, const ApplyRecord& rec) {
    if (rec.expectedNewManifestPath.empty()) {
        L("IntegrityGate: no expected-new-manifest in apply.json — SKIPPED (6b.3 release ships+signs it)");
        return true;
    }
    std::string mjson;
    if (!updatefs::ReadFileAll(W(rec.expectedNewManifestPath), mjson)) {
        // The path is SET (6c.3 always populates it) -> the manifest MUST be readable.
        // Unreadable here means deleted/tampered in the window -> FAIL (review 6c.3 GAP-A).
        L("IntegrityGate: expected-new-manifest path set but unreadable -> FAIL (deleted/tampered)");
        return false;
    }
    // Signature FIRST (V3-8): verify the sidecar before trusting any byte of it.
    std::string msig;
    if (!updatefs::ReadFileAll(W(rec.expectedNewManifestPath) + L".ed", msig)) {
        L("IntegrityGate: manifest signature sidecar (.ed) missing -> FAIL (fail-closed)");
        return false;
    }
    if (!updatefs::VerifyManifestSignature(mjson, msig)) {
        L("IntegrityGate: manifest signature INVALID -> FAIL (tamper/replay)");
        return false;
    }
    FileManifest m;
    if (!ParseManifest(mjson, m)) { L("IntegrityGate: manifest parse failed -> FAIL"); return false; }
    updatefs::VerifyResult r = updatefs::VerifyTreeAgainstManifest(p.appDir, m);
    if (!r.ok) { L("IntegrityGate: " + r.reason + " @ " + r.failedPath + " -> FAIL"); return false; }
    L("IntegrityGate: manifest signature OK + new tree matches (" + std::to_string(m.entries.size()) + " files)");
    return true;
}

// ---- Phase C: ROLLBACK (DB-FIRST crash-atomic; I9/V3-3) ----------------------
// p3 may be nullptr (--resume: no tracked health-probe process).
bool DoRollback(const Paths& p, ApplyRecord& rec, UpdateLockOwner& lock,
                HANDLE p3, const std::string& reason) {
    L("ROLLBACK begin: " + reason);
    WriteApply(p, rec, ApplyPhase::Installing, reason);  // keep "installing/awaiting" semantics for re-resume

    // 1. Kill the tracked health-probe build FIRST (apply path). Graceful-first on
    //    its wallet/adblock (H6) then taskkill /F /T (H3 — tree, so CEF subprocs
    //    holding libcef.dll die too).
    if (p3) {
        HttpPostShutdown(31301);
        HttpPostShutdown(31302);
        WaitForSingleObject(p3, 3000);
        if (WaitForSingleObject(p3, 0) != WAIT_OBJECT_0) TaskkillTree(GetProcessId(p3));
    }
    // 2. After the probe-kill, any remaining {app}\HodosBrowser.exe is a GENUINE
    //    sibling (a user-launched browser). NEVER restore over / disturb a live
    //    sibling's shared wallet (F5/F9): defer + retry next --resume.
    if (CountSiblingBrowsers(p.appDir) != 0) {
        L("ROLLBACK: a sibling browser is live — defer restore (never disturb its wallet)");
        return false;
    }
    // 3. (resume path, p3==nullptr) — no tracked probe; now that no sibling exists,
    //    clean up an orphaned new-build wallet on the ports before restore.
    if (!p3) { HttpPostShutdown(31301); HttpPostShutdown(31302); }

    // 4. Wait for the {app} images to ACTUALLY unlock (death != unlocked, H3). If
    //    still locked we cannot restore safely -> leave RunOnce armed, retry next.
    if (!PollUnlocked({p.browserExe, p.libcef,
                       p.appDir + L"\\hodos-wallet.exe", p.appDir + L"\\hodos-adblock.exe"},
                      kUnlockPollMs)) {
        L("ROLLBACK: {app} still locked after kill — deferring restore to next --resume");
        return false;
    }

    // 5. MONEY DB FIRST (I9/V3-3a): restore the full {db,-wal,-shm} set BEFORE the
    //    binaries. Distinguish "no snapshot was ever taken" (benign — the new build
    //    never migrated, continue) from "snapshot present but restore FAILED" (F1 —
    //    DANGEROUS: would pair an OLD binary with a migrated NEW-schema DB => abort
    //    the binary swap and defer, leaving the consistent new-exe+new-DB for retry).
    const std::wstring snapWallet = p.rollbackDir + L"\\wallet";
    const bool haveDbSnapshot =
        GetFileAttributesW((snapWallet + L"\\wallet.db").c_str()) != INVALID_FILE_ATTRIBUTES;
    if (haveDbSnapshot) {
        if (!updatefs::RestoreWalletDbSet(snapWallet, p.walletDir)) {
            L("ROLLBACK: money-DB restore FAILED with a snapshot present — ABORT binary swap, "
              "defer to next --resume (I9: never pair old binary with new-schema DB)");
            return false;
        }
    } else {
        L("ROLLBACK: no wallet snapshot (stage never snapshotted) — DB untouched, continuing");
    }

    // 6. Crash-atomic binary restore: copy rollback\ -> .restore-tmp\, then swap each
    //    file into {app}, HodosBrowser.exe + libcef.dll coherent pair LAST. .restore-tmp
    //    is the consumable copy (rollback\ is preserved for an idempotent re-resume).
    const std::wstring tmp = p.appDir + L"\\.restore-tmp";
    if (!updatefs::CopyTreeRecursive(p.rollbackDir, tmp, {L"wallet"})) {
        L("ROLLBACK: failed to stage .restore-tmp — deferring to next --resume");
        return false;
    }
    FileManifest rb;
    if (!updatefs::BuildManifestForTree(tmp, rb)) { L("ROLLBACK: cannot enumerate .restore-tmp"); return false; }
    auto swapOne = [&](const std::string& relKey) -> bool {
        std::wstring rel = W(relKey); for (auto& c : rel) if (c == L'/') c = L'\\';
        return updatefs::SwapFileReplace(tmp + L"\\" + rel, p.appDir + L"\\" + rel);
    };
    // Swap EVERY non-pair file first; bail IMMEDIATELY on any failure so the
    // exe+libcef commit-switch never flips over a partially-restored tree (F2). A
    // locked pak (lingering CEF subproc) => defer + retry, leaving the bootable NEW
    // exe consistent with the NEW resources.
    for (const auto& kv : rb.entries) {
        if (kv.first == "hodosbrowser.exe" || kv.first == "libcef.dll") continue;
        if (!swapOne(kv.first)) {
            L("ROLLBACK: non-pair swap failed @ " + kv.first + " — defer (exe NOT flipped)");
            return false;
        }
    }
    // Only now (every other file is the OLD version) flip the coherent pair: libcef then exe.
    if (rb.entries.count("libcef.dll") && !swapOne("libcef.dll")) {
        L("ROLLBACK: libcef.dll swap failed — defer (exe NOT flipped)"); return false;
    }
    if (rb.entries.count("hodosbrowser.exe") && !swapOne("hodosbrowser.exe")) {
        L("ROLLBACK: HodosBrowser.exe swap failed — defer"); return false;
    }

    // 5. paused + rescan; do NOT advance highWater (I6).
    UpdateState st = ReadStateOrDefault(p);
    st.paused = true;
    st.lastFailureBuild = rec.toBuild;
    st.lastFailureReason = reason;
    st.rescanAfterRollback = true;   // V3-4
    WriteState(p, st);
    WriteApply(p, rec, ApplyPhase::RolledBack, reason);

    // 6. Clear RunOnce, release the lock BEFORE relaunching old (else the old
    //    browser self-defers on the still-held lock), relaunch old.
    ClearRunOnce();
    lock.Release();  // DELETE_ON_CLOSE removes update.lock
    HANDLE old = Spawn(L"\"" + p.browserExe + L"\"", /*detached=*/true);
    if (old) CloseHandle(old);
    L("ROLLBACK complete: old build relaunched, updates paused");
    return true;
}

// ---- Phase E: SUCCESS --------------------------------------------------------
void DoSuccess(const Paths& p, ApplyRecord& rec, UpdateLockOwner& lock) {
    UpdateState st = ReadStateOrDefault(p);
    if (rec.toBuild > st.highWaterBuild) st.highWaterBuild = rec.toBuild;  // anti-rollback floor
    if (!rec.signerThumbprint.empty()) st.signerThumbprint = rec.signerThumbprint;  // I5 cache
    st.paused = false;
    WriteState(p, st);
    WriteApply(p, rec, ApplyPhase::Healthy);
    ClearRunOnce();
    // The healthy P3 is the running session. Delete pending\ via a detached cmd
    // AFTER we exit (the helper runs from pending\helper\ and can't delete its own
    // image, M5). CWD is already outside the working area (main).
    if (!p.pendingDir.empty()) {
        std::wstring rm = L"cmd.exe /c ping 127.0.0.1 -n 3 >nul & rmdir /s /q \"" + p.pendingDir + L"\"";
        HANDLE h = Spawn(rm, /*detached=*/true);
        if (h) CloseHandle(h);
    }
    lock.Release();
    L("SUCCESS: highWater=" + std::to_string(rec.toBuild) + ", pending cleanup scheduled");
}

// ---- Phase B: wait for healthy ----------------------------------------------
// Returns true if the new build wrote phase==Healthy in time; false on timeout /
// P3 crash. Honors the wallet's "alive-but-migrating" via the generous timeout (V3-15).
bool WaitForHealthy(const Paths& p, HANDLE p3, DWORD timeoutMs) {
    const DWORD start = GetTickCount();
    for (;;) {
        ApplyRecord cur;
        if (ReadApply(p, cur) && cur.phase == ApplyPhase::Healthy) return true;
        if (p3 && WaitForSingleObject(p3, 0) == WAIT_OBJECT_0) {
            L("WaitForHealthy: P3 exited before writing healthy");
            return false;
        }
        if (GetTickCount() - start > timeoutMs) { L("WaitForHealthy: timeout"); return false; }
        Sleep(500);
    }
}

}  // namespace

void SetLogger(LogFn fn) { g_log = fn; }

// ====================== Phase B/C/E — the normal apply ========================
int RunApplyTransaction(const std::map<std::string, std::string>& args,
                        UpdateLockOwner& lock, ApplyRecord rec) {
    const Paths p = ResolvePaths(args);
    if (p.appDir.empty() || p.applyPath.empty()) { L("apply: no appDir/pending — abort"); return 0; }

    // 1. Wait for the bootstrap (P0) to exit via its inherited HANDLE (PID-reuse-
    //    immune, V3-11), so {app}\HodosBrowser.exe + libcef.dll unlock.
    {
        std::wstring hs = Arg(args, "bootstrap-handle");
        if (!hs.empty()) {
            HANDLE bh = reinterpret_cast<HANDLE>(static_cast<uintptr_t>(_wcstoui64(hs.c_str(), nullptr, 10)));
            if (bh) { WaitForSingleObject(bh, kBootstrapWaitMs); CloseHandle(bh); }
        }
    }
    // 2. Confirm the {app} images actually unlocked (death != unlocked, V3-12).
    if (!PollUnlocked({p.browserExe, p.libcef}, kUnlockPollMs)) {
        L("apply: {app} images still locked — abort, retry next launch");
        WriteApply(p, rec, ApplyPhase::Aborted, "app-locked");
        ClearRunOnce();
        return 0;
    }
    // 3. RE-confirm all-instances-gone (F6/F9): a sibling slipping in => ABORT,
    //    never taskkill the shared wallet.
    int sib = CountSiblingBrowsers(p.appDir);
    if (sib != 0) {
        L("apply: sibling browser present (" + std::to_string(sib) + ") — abort, never install over a live wallet");
        WriteApply(p, rec, ApplyPhase::Aborted, "sibling-present");
        ClearRunOnce();
        return 0;
    }
    // 4. Belt-and-suspenders child shutdown (E.3): normally already dead.
    HttpPostShutdown(31301);
    HttpPostShutdown(31302);
    PollUnlocked({p.appDir + L"\\hodos-wallet.exe", p.appDir + L"\\hodos-adblock.exe"}, kChildShutdownMs);

    // 5. INSTALLING (before spawn, M7) -> run the installer /VERYSILENT.
    WriteApply(p, rec, ApplyPhase::Installing);
    if (rec.installerPath.empty()) { L("apply: no installer path — abort"); WriteApply(p, rec, ApplyPhase::Aborted, "no-installer"); return 0; }
    const std::wstring instCmd = L"\"" + W(rec.installerPath) + L"\" /VERYSILENT /SP- /SUPPRESSMSGBOXES /NORESTART";
    HANDLE inst = Spawn(instCmd, /*detached=*/false);
    if (!inst) { L("apply: installer spawn failed -> rollback"); DoRollback(p, rec, lock, nullptr, "installer-spawn-failed"); return 1; }
    DWORD wr = WaitForSingleObject(inst, kInstallerWaitMs);
    DWORD code = 1;
    if (wr == WAIT_OBJECT_0) {
        GetExitCodeProcess(inst, &code);
    } else {
        // Wedged installer (F6): terminate it so it can't keep writing {app} while
        // we roll back (the unlock-poll would otherwise defer indefinitely).
        TerminateProcess(inst, 1);
        WaitForSingleObject(inst, 5000);
    }
    CloseHandle(inst);
    if (wr != WAIT_OBJECT_0 || code != 0) {
        L("apply: installer failed (wait=" + std::to_string(wr) + " code=" + std::to_string(code) + ") -> rollback");
        DoRollback(p, rec, lock, nullptr, "installer-failed");
        return 1;
    }

    // 6. Integrity gate the new tree (B4).
    if (!IntegrityGate(p, rec)) { DoRollback(p, rec, lock, nullptr, "integrity-failed"); return 1; }

    // 7. AWAITING-HEALTH -> launch the health-probe (H1: explicit profile, !picker).
    WriteApply(p, rec, ApplyPhase::AwaitingHealth);
    std::wstring probe = L"\"" + p.browserExe + L"\" --post-update-health-probe";
    if (!rec.profileId.empty()) probe += L" --profile " + W(rec.profileId);
    HANDLE p3 = Spawn(probe, /*detached=*/false);
    if (!p3) { L("apply: health-probe launch failed -> rollback"); DoRollback(p, rec, lock, nullptr, "probe-launch-failed"); return 1; }

    // 8. Wait for healthy -> SUCCESS, else ROLLBACK.
    bool healthy = WaitForHealthy(p, p3, kHealthWaitMs);
    if (healthy) {
        CloseHandle(p3);
        DoSuccess(p, rec, lock);
        return 0;
    }
    DoRollback(p, rec, lock, p3, "health-timeout-or-crash");
    CloseHandle(p3);
    return 1;
}

// ============================ --resume watchdog ===============================
int RunResume(const std::map<std::string, std::string>& args,
              UpdateLockOwner& lock, ApplyRecord rec) {
    const Paths p = ResolvePaths(args);
    if (p.appDir.empty() || p.applyPath.empty()) { L("resume: no appDir/pending — nothing to do"); return 0; }

    // Re-arm RunOnce at entry (V3-14): a 2nd power-loss DURING recovery re-fires.
    ArmRunOnce();

    switch (rec.phase) {
        case ApplyPhase::Installing:
        case ApplyPhase::AwaitingHealth:
            // The supervisor died after install but before confirming health (or mid
            // power-loss). Restore (DB-first, idempotent), pause, relaunch old.
            DoRollback(p, rec, lock, /*p3=*/nullptr, "resume-unconfirmed-apply");
            return 0;
        case ApplyPhase::Healthy:
            // Success cleanup was interrupted — finish it.
            DoSuccess(p, rec, lock);
            return 0;
        case ApplyPhase::Armed:
        case ApplyPhase::Preparing:
        case ApplyPhase::RolledBack:
        case ApplyPhase::Aborted:
        case ApplyPhase::None:
        default:
            // Installer never ran (or already fully resolved). Just clear RunOnce.
            L("resume: phase=" + std::string(ApplyPhaseToString(rec.phase)) + " — nothing to restore, clearing RunOnce");
            ClearRunOnce();
            return 0;
    }
}

}  // namespace helper
}  // namespace hodos

#endif  // _WIN32
