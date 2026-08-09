#include "../../include/core/Logger.h"

// Static member definitions
std::ofstream Logger::logFile;
bool Logger::initialized = false;
ProcessType Logger::currentProcess = ProcessType::MAIN;
std::string Logger::logFilePath = "";
Logger::LogSinkFn Logger::sink = nullptr;

// Method implementations
void Logger::Initialize(ProcessType process, const std::string& filePath) {
    if (initialized) return;

    currentProcess = process;
    logFilePath = filePath;

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

void Logger::Log(const std::string& message, int level, int process) {
    LogLevel logLevel = static_cast<LogLevel>(level);
    ProcessType processType = static_cast<ProcessType>(process);

    std::string logEntry = "[" + GetTimestamp() + "] [" + GetProcessName(processType) + "] [" + GetLogLevelName(logLevel) + "] " + message;

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
