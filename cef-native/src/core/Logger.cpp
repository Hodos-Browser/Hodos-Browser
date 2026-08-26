#include "../../include/core/Logger.h"

#include <algorithm>
#include <cstdio>
#include <filesystem>
#include <mutex>
#include <system_error>
#include <vector>

namespace {
// P2-A7: Logger::Log had NO synchronisation and is called from the UI thread, the IO
// thread (every resource request) and CEF's worker threads. `logFile << entry << endl`
// is not atomic across threads, so lines interleaved mid-write.
//
// MEASURED in the shipped production log: 8,208 fragments with no timestamp prefix --
// torn URLs and a half-written tab-state JSON. See MEASUREMENTS.md M2.
//
// This is not cosmetic. The log is the ONLY record of several shipped safeguards (the
// gold-pill auto-approve, the permission cascade, the profile-delete refusals), and a
// data race can mangle precisely the line someone later needs. It is also UB.
//
// Function-local static: no static-init-order dependency, and it outlives every caller.
std::mutex& LogMutex() {
    static std::mutex m;
    return m;
}
}  // namespace

// Static member definitions
std::ofstream Logger::logFile;
bool Logger::initialized = false;
ProcessType Logger::currentProcess = ProcessType::MAIN;
std::string Logger::logFilePath = "";
Logger::LogSinkFn Logger::sink = nullptr;
bool Logger::lockEnabled = true;
// Safe default = production behaviour. See the header for why this is INFO and not WARNING.
LogLevel Logger::minLevel = LogLevel::INFO;
std::size_t Logger::bytesWritten = 0;
std::size_t Logger::maxBytes = 10u * 1024u * 1024u;   // 10 MB per file
int Logger::keepFiles = 5;                            // -> 50 MB ceiling per writer

// Method implementations
void Logger::Initialize(ProcessType process, const std::string& filePath) {
    if (initialized) return;

    currentProcess = process;
    logFilePath = filePath;

    // Seed the counter from what is already on disk. Without this, every restart resets
    // the rotation budget to zero and a long-lived install grows without bound one session
    // at a time -- which is the defect this whole phase exists to close.
    {
        std::error_code ec;
        const auto existing = std::filesystem::file_size(std::filesystem::u8path(logFilePath), ec);
        bytesWritten = ec ? 0u : static_cast<std::size_t>(existing);
    }

    logFile.open(logFilePath, std::ios::app);
    if (logFile.is_open()) {
        initialized = true;
        Log("Logger initialized for " + GetProcessName(process), 1);
    } else {
        std::cout << "WARNING: Could not open log file: " << filePath << std::endl;
    }
}

void Logger::SetSink(LogSinkFn newSink) {
    sink = newSink;
}

void Logger::SetLockEnabledForTesting(bool enabled) {
    lockEnabled = enabled;
}

void Logger::SetMinLevel(LogLevel level) {
    minLevel = level;
}

LogLevel Logger::GetMinLevel() {
    return minLevel;
}

bool Logger::IsEnabled(int level) {
    return level >= static_cast<int>(minLevel);
}

void Logger::SetRotation(std::size_t newMaxBytes, int newKeepFiles) {
    maxBytes = newMaxBytes;
    keepFiles = newKeepFiles < 0 ? 0 : newKeepFiles;
}

// Caller must hold LogMutex(). Rotates "<path>" -> "<path>.1" -> ... -> "<path>.N",
// dropping whatever falls off the end.
void Logger::RotateIfNeededLocked() {
    if (maxBytes == 0 || bytesWritten < maxBytes) return;
    if (!logFile.is_open()) return;

    logFile.close();

    std::error_code ec;
    const std::filesystem::path base = std::filesystem::u8path(logFilePath);

    // Drop the oldest, then shift each survivor up one slot.
    std::filesystem::remove(std::filesystem::u8path(logFilePath + "." + std::to_string(keepFiles)), ec);
    for (int i = keepFiles - 1; i >= 1; --i) {
        const auto from = std::filesystem::u8path(logFilePath + "." + std::to_string(i));
        const auto to   = std::filesystem::u8path(logFilePath + "." + std::to_string(i + 1));
        if (std::filesystem::exists(from, ec)) std::filesystem::rename(from, to, ec);
    }
    if (keepFiles >= 1) {
        std::filesystem::rename(base, std::filesystem::u8path(logFilePath + ".1"), ec);
    } else {
        std::filesystem::remove(base, ec);
    }

    logFile.open(logFilePath, std::ios::app);
    bytesWritten = 0;
    // ⛔ No Log() call here: we hold the mutex, and Log() would deadlock on it.
}

int Logger::PruneOldLogs(const std::string& dir, int maxAgeDays, std::size_t maxTotalBytes) {
    std::error_code ec;
    const auto root = std::filesystem::u8path(dir);
    if (!std::filesystem::is_directory(root, ec)) return 0;

    struct Entry {
        std::filesystem::path path;
        std::filesystem::file_time_type mtime;
        std::uintmax_t size = 0;
    };
    std::vector<Entry> logs;

    for (const auto& de : std::filesystem::directory_iterator(root, ec)) {
        if (ec) break;
        if (!de.is_regular_file(ec)) continue;
        // Only our own logs. ⛔ Never widen this: the wallet's rotating logs live in the
        // same directory and are not ours to delete.
        std::string name;
        try {
            name = de.path().filename().string();
        } catch (...) {
            continue;  // a name this locale cannot render is not one of ours
        }
        if (name.rfind("debug_output", 0) != 0) continue;
        Entry e;
        e.path = de.path();
        e.mtime = de.last_write_time(ec);
        e.size = de.file_size(ec);
        if (!ec) logs.push_back(e);
    }

    int removed = 0;
    const auto now = std::filesystem::file_time_type::clock::now();

    // 1. Age. The file currently open for writing is skipped -- it is by definition fresh,
    //    but a clock change could still make it look old.
    if (maxAgeDays > 0) {
        const auto maxAge = std::chrono::hours(24 * maxAgeDays);
        for (auto it = logs.begin(); it != logs.end();) {
            const bool isCurrent = !logFilePath.empty() &&
                it->path == std::filesystem::u8path(logFilePath);
            if (!isCurrent && (now - it->mtime) > maxAge) {
                std::filesystem::remove(it->path, ec);
                if (!ec) ++removed;
                it = logs.erase(it);
            } else {
                ++it;
            }
        }
    }

    // 2. Total size, oldest first.
    if (maxTotalBytes > 0) {
        std::uintmax_t total = 0;
        for (const auto& e : logs) total += e.size;
        std::sort(logs.begin(), logs.end(),
                  [](const Entry& a, const Entry& b) { return a.mtime < b.mtime; });
        for (const auto& e : logs) {
            if (total <= maxTotalBytes) break;
            if (!logFilePath.empty() && e.path == std::filesystem::u8path(logFilePath)) continue;
            std::filesystem::remove(e.path, ec);
            if (!ec) { total -= e.size; ++removed; }
        }
    }
    return removed;
}

void Logger::Log(const std::string& message, int level, int process) {
    // The level gate. Deliberately the FIRST thing: everything below allocates, and a
    // suppressed DEBUG must cost one integer compare, not a timestamp + three concatenations.
    //
    // ⚠️ Known limit, stated rather than papered over: this cannot avoid the concatenation
    // the CALLER already did to build `message`. Call sites on genuinely hot paths should
    // guard with Logger::IsEnabled(level) as well.
    if (level < static_cast<int>(minLevel)) return;

    LogLevel logLevel = static_cast<LogLevel>(level);
    ProcessType processType = static_cast<ProcessType>(process);

    std::string logEntry = "[" + GetTimestamp() + "] [" + GetProcessName(processType) + "] [" + GetLogLevelName(logLevel) + "] " + message;

    // Everything below writes to a shared sink. Formatting above is thread-local and is
    // deliberately left outside the lock -- it is the expensive part, and holding the lock
    // across it would serialise every logging thread on string building.
    //
    // The lock is runtime-switchable purely so P2-A7's negative control can observe torn
    // lines on the SAME binary (a red produced by a different build leaves "did the binary
    // change, or did the behaviour?" unanswered). Nothing in production ever disables it.
    std::unique_lock<std::mutex> guard(LogMutex(), std::defer_lock);
    if (lockEnabled) guard.lock();

    if (!initialized) {
        // A child process lands here: it never calls Initialize() and, being sandboxed,
        // could not open the log file if it tried. Hand the line to the installed sink
        // (Chromium logging -> cef_debug.log). Without a sink this falls back to stdout,
        // which is where every renderer log line used to go to die.
        if (sink) {
            sink(logEntry.c_str(), level);
        } else {
            std::cout << logEntry << std::endl;
        }
        return;
    }

    if (logFile.is_open()) {
        logFile << logEntry << std::endl;
        logFile.flush();
        bytesWritten += logEntry.size() + 1;
        RotateIfNeededLocked();
    }

    std::cout << logEntry << std::endl;
}

void Logger::Shutdown() {
    if (initialized && logFile.is_open()) {
        Log("Logger shutting down", 1);
        logFile.close();
        initialized = false;
    }
}

bool Logger::IsInitialized() {
    return initialized;
}
