#pragma once

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

    static void Initialize(ProcessType process, const std::string& filePath = "debug_output.log");
    static void Log(const std::string& message, int level = 1, int process = 0);
    static void Shutdown();
    static bool IsInitialized();

private:
    static LogSinkFn sink;
};
