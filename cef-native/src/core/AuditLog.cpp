#include "../../include/core/AuditLog.h"

#include <chrono>
#include <ctime>
#include <fstream>
#include <iomanip>
#include <mutex>
#include <sstream>

#ifdef _WIN32
#include <windows.h>
#include <process.h>
#else
#include <unistd.h>
#endif

namespace hodos {
namespace {

std::mutex& AuditMutex() {
    static std::mutex m;
    return m;
}

std::ofstream& AuditFile() {
    static std::ofstream f;
    return f;
}

std::string& AuditPath() {
    static std::string p;
    return p;
}

unsigned long CurrentPid() {
#ifdef _WIN32
    return static_cast<unsigned long>(::GetCurrentProcessId());
#else
    return static_cast<unsigned long>(::getpid());
#endif
}

std::string Timestamp() {
    const auto now = std::chrono::system_clock::now();
    const auto t = std::chrono::system_clock::to_time_t(now);
    const auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
                        now.time_since_epoch()) % 1000;
    std::stringstream ss;
#ifdef _WIN32
    std::tm tmv{};
    localtime_s(&tmv, &t);
    ss << std::put_time(&tmv, "%Y-%m-%d %H:%M:%S");
#else
    ss << std::put_time(std::localtime(&t), "%Y-%m-%d %H:%M:%S");
#endif
    ss << "." << std::setfill('0') << std::setw(3) << ms.count();
    return ss.str();
}

// Keep one line on one line. A newline injected through `detail` would let a caller forge a
// second audit record, which is precisely the sort of thing an audit log must not permit.
std::string OneLine(const std::string& in) {
    std::string out;
    out.reserve(in.size());
    for (const char c : in) {
        out += (c == '\n' || c == '\r' || c == '\t') ? ' ' : c;
    }
    return out;
}

}  // namespace

void InitAuditLog(const std::string& dir) {
    std::lock_guard<std::mutex> guard(AuditMutex());
    if (AuditFile().is_open()) return;
    if (dir.empty()) return;

#ifdef _WIN32
    const std::string sep = "\\";
#else
    const std::string sep = "/";
#endif
    AuditPath() = dir + sep + "audit-" + std::to_string(CurrentPid()) + ".log";
    AuditFile().open(AuditPath(), std::ios::app);
}

void AuditEvent(const std::string& event,
                const std::string& origin,
                const std::string& detail) {
    std::lock_guard<std::mutex> guard(AuditMutex());
    if (!AuditFile().is_open()) return;  // never break a payment over a log
    AuditFile() << "[" << Timestamp() << "] " << OneLine(event)
                << " | " << OneLine(origin.empty() ? "-" : origin)
                << " | " << OneLine(detail) << std::endl;
    AuditFile().flush();
}

unsigned long long AuditLogSize() {
    std::lock_guard<std::mutex> guard(AuditMutex());
    if (AuditPath().empty()) return 0;
    std::ifstream in(AuditPath(), std::ios::binary | std::ios::ate);
    if (!in) return 0;
    return static_cast<unsigned long long>(in.tellg());
}

void ShutdownAuditLog() {
    std::lock_guard<std::mutex> guard(AuditMutex());
    if (AuditFile().is_open()) AuditFile().close();
}

}  // namespace hodos
