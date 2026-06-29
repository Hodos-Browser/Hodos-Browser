// UpdateLock.h — the two-MODE update.lock primitive (commit 6b.1).
//
// AUTOUPDATE_6B_SUPERVISOR_DESIGN.md §9 v3 "two-MODE lock" (V3-5/V3-6). `update.lock`
// is a zero-byte liveness token used two ways. Liveness is an OS guarantee (can a
// new exclusive open succeed?), NEVER a pid/heartbeat guess.
//
//   OWNER  (bootstrap, supervisor, every --resume/watchdog entry): exclusive
//          (share=0) CREATE_ALWAYS handle with DELETE_ON_CLOSE + the DELETE access
//          right, so the file vanishes when the last handle closes (crash/clean
//          both auto-clean; a power-loss remnant is harmless — see PROBE). Holding
//          this handle == "I am the live apply owner". Inheritable so the bootstrap
//          can pass it to the helper (V3-11). FIRST action of every entry point is
//          Acquire()-or-bail (V3-6); a 2nd owner gets SHARING_VIOLATION.
//
//   PROBE  (the 6a honor-at-launch on a NORMAL launch): permissive-share read
//          open, NO DELETE_ON_CLOSE. SHARING_VIOLATION => a live owner holds it =>
//          an apply is in progress => the caller DEFERS. Opens / absent => no live
//          owner => proceed. Permissive share so two concurrent probes never
//          false-defer on each other (V3-5/N1); never DELETE_ON_CLOSE on a probe
//          (would delete the owner's lock).

#pragma once
#ifdef _WIN32

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <string>

namespace hodos {

// RAII OWNER lock. Acquire() returns true and holds an exclusive handle, or false
// on SHARING_VIOLATION (another live owner) / any error. The handle is inheritable
// so it can be passed to a spawned helper via PROC_THREAD_ATTRIBUTE_HANDLE_LIST.
class UpdateLockOwner {
public:
    UpdateLockOwner() = default;
    ~UpdateLockOwner() { Release(); }
    UpdateLockOwner(const UpdateLockOwner&) = delete;
    UpdateLockOwner& operator=(const UpdateLockOwner&) = delete;

    bool Acquire(const std::wstring& path) {
        Release();
        SECURITY_ATTRIBUTES sa{};
        sa.nLength = sizeof(sa);
        sa.bInheritHandle = TRUE;  // pass to the helper (V3-11)
        sa.lpSecurityDescriptor = nullptr;
        handle_ = CreateFileW(
            path.c_str(),
            GENERIC_READ | GENERIC_WRITE | DELETE,
            0,                       // share=0 -> exclusive == the single-flight
            &sa,
            CREATE_ALWAYS,           // re-arm a power-loss remnant (V3-5/N4)
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_DELETE_ON_CLOSE,
            nullptr);
        return handle_ != INVALID_HANDLE_VALUE;
    }

    // Adopt an already-open handle the bootstrap inherited to us (the helper).
    void Adopt(HANDLE h) { Release(); handle_ = h; }

    bool held() const { return handle_ != INVALID_HANDLE_VALUE; }
    HANDLE raw() const { return handle_; }

    void Release() {
        if (handle_ != INVALID_HANDLE_VALUE) {
            CloseHandle(handle_);  // last close => DELETE_ON_CLOSE removes the file
            handle_ = INVALID_HANDLE_VALUE;
        }
    }

private:
    HANDLE handle_ = INVALID_HANDLE_VALUE;
};

// PROBE: returns true == "a live owner holds the lock" (the caller should DEFER).
// Pure read; never mutates or deletes the file.
inline bool UpdateLockIsHeld(const std::wstring& path) {
    HANDLE h = CreateFileW(
        path.c_str(),
        GENERIC_READ,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,  // permissive: no probe-vs-probe conflict
        nullptr,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        nullptr);
    if (h != INVALID_HANDLE_VALUE) {
        CloseHandle(h);
        return false;  // opened => no exclusive owner
    }
    const DWORD err = GetLastError();
    if (err == ERROR_SHARING_VIOLATION) return true;  // a live owner's share=0 handle blocks us => defer
    // Absent, or any other error (access-denied, path-not-found): do NOT brick the
    // launch — treat as "no live owner". A remnant file with no holder opens fine
    // above; this branch only covers absence and odd ACLs.
    return false;
}

}  // namespace hodos

#endif  // _WIN32
