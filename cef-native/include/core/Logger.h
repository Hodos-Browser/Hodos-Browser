#pragma once

#include <cstddef>
#include <fstream>
#include <iostream>
#include <string>
#include <chrono>
#include <iomanip>
#include <sstream>

// Log levels
enum class LogLevel {
    DEBUG = 0,
    INFO = 1,
    WARNING = 2,
    ERROR_LEVEL = 3
};

// Process types
enum class ProcessType {
    MAIN = 0,
    RENDER = 1,
    BROWSER = 2
};

/// Lets a `std::cout << a << b << c` chain become a Logger call without rewriting the
/// expression.
///
/// WHY. Dozens of call sites logged through std::cout — which, measured, reaches NOTHING:
/// Logger::Initialize holds the log file open, so the freopen that would have redirected
/// stdout ALWAYS fails EACCES(13) and the fallback reopens stdout on NUL. Every one of
/// those lines has been written into a bin since the first build. Converting them by hand
/// means retyping each chain as string concatenation and getting the types right —
/// `"n=" + count` does not compile, `"e=" + e.what()` does the wrong thing — which is a lot
/// of opportunities to introduce a bug while fixing a logging defect.
///
///     LOG_ERROR_PM(LogFmt() << "Failed for " << id << ": " << e.what());
///
/// ⛔ Do NOT "fix" the blackhole by making the freopen succeed instead. That would put two
/// writers on one file — Logger's ofstream and the CRT's stdout — which is a data race on
/// the log we are trying to make trustworthy.
class LogFmt {
public:
    template <typename T>
    LogFmt& operator<<(const T& value) {
        ss_ << value;
        return *this;
    }
    operator std::string() const { return ss_.str(); }
    std::string str() const { return ss_.str(); }

private:
    std::ostringstream ss_;
};

// Centralized Logger class (header-only for cross-compilation)
class Logger {
private:
    static std::ofstream logFile;
    static bool initialized;
    static ProcessType currentProcess;
    static std::string logFilePath;

    static std::string GetTimestamp() {
        auto now = std::chrono::system_clock::now();
        auto time_t = std::chrono::system_clock::to_time_t(now);
        auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
            now.time_since_epoch()) % 1000;

        std::stringstream ss;
        ss << std::put_time(std::localtime(&time_t), "%Y-%m-%d %H:%M:%S");
        ss << "." << std::setfill('0') << std::setw(3) << ms.count();
        return ss.str();
    }

    static std::string GetProcessName(ProcessType process) {
        switch (process) {
            case ProcessType::MAIN: return "MAIN";
            case ProcessType::RENDER: return "RENDER";
            case ProcessType::BROWSER: return "BROWSER";
            default: return "UNKNOWN";
        }
    }

    static std::string GetLogLevelName(LogLevel level) {
        switch (level) {
            case LogLevel::DEBUG: return "DEBUG";
            case LogLevel::INFO: return "INFO";
            case LogLevel::WARNING: return "WARN";
            case LogLevel::ERROR_LEVEL: return "ERROR";
            default: return "UNKNOWN";
        }
    }

public:
    /// Sink for processes that cannot open the log file themselves.
    ///
    /// WHY THIS EXISTS. Only the browser process ever calls Initialize(), so in every
    /// CHILD process `initialized` stayed false and Log() fell through to std::cout —
    /// which the browser process redirects into debug_output.log but a child does not.
    /// Result: every LOG_*_RENDER call in the codebase was a silent no-op and `[RENDER]`
    /// never once appeared in the log (verified: 0 occurrences). That is why a total
    /// farbling failure went unnoticed for the entire life of the feature — the one
    /// subsystem that would have reported it could not write.
    ///
    /// A child process cannot simply call Initialize() either: renderers run SANDBOXED at
    /// UNTRUSTED integrity and have no write access to %APPDATA%. The fix is to hand the
    /// line to Chromium's logging, which is already brokered across the sandbox and lands
    /// in cef_debug.log (settings.log_file). That code needs CEF headers, and this
    /// translation unit is compiled into the CEF-free unit-test target, so it arrives as
    /// an injected function pointer rather than an #include.
    ///
    /// Install with hodos::InstallChildProcessLogSink() (src/core/ChildProcessLogSink.h).
    using LogSinkFn = void (*)(const char* formattedLine, int level);
    static void SetSink(LogSinkFn sink);

    /// P2-A7 negative control ONLY. Disables Logger's write lock so the test binary can
    /// observe the torn lines the shipped log actually contains (8,208 of them), on the
    /// SAME build. ⛔ Never call this from production code.
    static void SetLockEnabledForTesting(bool enabled);

    /// Minimum level that reaches disk. Anything below is dropped BEFORE the log entry is
    /// formatted, so a suppressed DEBUG costs one integer compare.
    ///
    /// Default is INFO -- i.e. PRODUCTION behaviour -- deliberately. Until beta.3 there was
    /// no gate at all and DEBUG shipped to users: 99.0 % of a 2.5 GB log, growing ~91 MB/day
    /// and never pruned. A safe default means a caller that forgets to set anything gets the
    /// quiet behaviour, not the firehose.
    ///
    /// ⛔ Do NOT raise the production default to WARNING. Measured over 50 days of real use,
    /// WARNING would have kept 401 lines and ZERO errors -- it satisfies "the log got
    /// smaller" by destroying the log. See MEASUREMENTS.md M8.
    static void SetMinLevel(LogLevel level);
    static LogLevel GetMinLevel();

    /// True if a line at this level would be written. Use it to skip building an expensive
    /// message at the CALL SITE -- the gate inside Log() cannot avoid a concatenation the
    /// caller has already performed to produce the argument.
    static bool IsEnabled(int level);

    /// Size cap per file and how many rotations to keep. A file crossing kMaxBytes is closed
    /// and renamed to ".1", existing ".N" shift up, and anything past kKeepFiles is deleted.
    static void SetRotation(std::size_t maxBytes, int keepFiles);

    /// Delete stale logs in `dir`: anything matching debug_output*.log* older than
    /// maxAgeDays, then oldest-first until the directory's log total fits maxTotalBytes.
    /// Safe to call before Initialize(); returns the number of files removed.
    static int PruneOldLogs(const std::string& dir, int maxAgeDays, std::size_t maxTotalBytes);

    static void Initialize(ProcessType process, const std::string& filePath = "debug_output.log");
    static void Log(const std::string& message, int level = 1, int process = 0);
    static void Shutdown();
    static bool IsInitialized();

private:
    static void RotateIfNeededLocked();

    static LogSinkFn sink;
    static bool lockEnabled;
    static LogLevel minLevel;
    static std::size_t bytesWritten;
    static std::size_t maxBytes;
    static int keepFiles;
};
