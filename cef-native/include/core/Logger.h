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
    static void Initialize(ProcessType process, const std::string& filePath = "debug_output.log");
    static void Log(const std::string& message, int level = 1, int process = 0);
    static void Shutdown();
    static bool IsInitialized();
};
