#include "../../include/core/ProfileLock.h"

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>

static HANDLE g_profile_lock_handle = INVALID_HANDLE_VALUE;

bool AcquireProfileLock(const std::string& profile_path) {
    std::string lock_file = profile_path + "\\profile.lock";

    const int MAX_RETRIES = 6;
    const DWORD RETRY_DELAY_MS = 500;

    for (int attempt = 0; attempt < MAX_RETRIES; attempt++) {
        HANDLE handle = CreateFileA(
            lock_file.c_str(),
            GENERIC_WRITE,
            0,  // No sharing — exclusive access
            NULL,
            CREATE_ALWAYS,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_DELETE_ON_CLOSE,  // Auto-cleanup on crash
            NULL
        );

        if (handle != INVALID_HANDLE_VALUE) {
            g_profile_lock_handle = handle;
            return true;
        }

        // Retry after delay (previous instance may still be shutting down)
        if (attempt < MAX_RETRIES - 1) {
            Sleep(RETRY_DELAY_MS);
        }
    }

    return false;
}

void ReleaseProfileLock() {
    if (g_profile_lock_handle != INVALID_HANDLE_VALUE) {
        CloseHandle(g_profile_lock_handle);
        g_profile_lock_handle = INVALID_HANDLE_VALUE;
    }
}

bool IsProfileLockedByAnotherInstance(const std::string& profile_path) {
    std::string lock_file = profile_path + "\\profile.lock";

    // OPEN_EXISTING, no sharing. The lock is held with FILE_FLAG_DELETE_ON_CLOSE, so the
    // file exists only while an instance holds it.
    HANDLE handle = CreateFileA(
        lock_file.c_str(),
        GENERIC_READ,
        0,  // no sharing — a live holder makes this fail
        NULL,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        NULL);

    if (handle != INVALID_HANDLE_VALUE) {
        CloseHandle(handle);  // we got it, so nobody holds it
        return false;
    }

    const DWORD err = GetLastError();
    if (err == ERROR_FILE_NOT_FOUND || err == ERROR_PATH_NOT_FOUND) {
        return false;  // no lock file at all — not in use
    }
    // ERROR_SHARING_VIOLATION, or anything we cannot interpret: treat as in use.
    // Fail closed — a refused delete is recoverable, a deleted live profile is not.
    return true;
}

#elif defined(__APPLE__) || defined(__linux__)

#include <sys/file.h>
#include <fcntl.h>
#include <unistd.h>

static int g_profile_lock_fd = -1;

bool AcquireProfileLock(const std::string& profile_path) {
    std::string lock_file = profile_path + "/profile.lock";

    const int MAX_RETRIES = 6;
    const useconds_t RETRY_DELAY_US = 500000;  // 500ms

    for (int attempt = 0; attempt < MAX_RETRIES; attempt++) {
        int fd = open(lock_file.c_str(), O_WRONLY | O_CREAT | O_CLOEXEC, 0644);
        if (fd < 0) {
            if (attempt < MAX_RETRIES - 1) {
                usleep(RETRY_DELAY_US);
                continue;
            }
            return false;
        }

        // Non-blocking exclusive lock
        if (flock(fd, LOCK_EX | LOCK_NB) == 0) {
            g_profile_lock_fd = fd;
            return true;
        }

        close(fd);

        // Retry after delay (previous instance may still be shutting down)
        if (attempt < MAX_RETRIES - 1) {
            usleep(RETRY_DELAY_US);
        }
    }

    return false;
}

void ReleaseProfileLock() {
    if (g_profile_lock_fd >= 0) {
        flock(g_profile_lock_fd, LOCK_UN);
        close(g_profile_lock_fd);
        g_profile_lock_fd = -1;
    }
}

bool IsProfileLockedByAnotherInstance(const std::string& profile_path) {
    std::string lock_file = profile_path + "/profile.lock";

    // ⚠️ Unlike Windows, this lock file is NOT delete-on-close, so it survives a normal
    // exit and its mere existence proves nothing. The lock itself is what matters, so the
    // probe must try to TAKE it and release again.
    int fd = open(lock_file.c_str(), O_WRONLY | O_CLOEXEC);
    if (fd < 0) {
        return false;  // no lock file — never used, or already cleaned up
    }

    if (flock(fd, LOCK_EX | LOCK_NB) == 0) {
        flock(fd, LOCK_UN);  // release immediately — this is a probe, not an acquisition
        close(fd);
        return false;
    }

    close(fd);
    // EWOULDBLOCK (held), or anything else we cannot interpret. Fail closed, matching the
    // Windows arm: a refused delete is recoverable, a deleted live profile is not.
    return true;
}

#endif
