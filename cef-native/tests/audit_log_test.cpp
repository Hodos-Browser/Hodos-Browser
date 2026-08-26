// The audit log — the record that must survive the debug log being deleted.
//
// ⛔ This file exists because an audit log that has never been observed to write is
// indistinguishable from one that does not work, and "the log got smaller" was already
// satisfiable by writing nothing. The debug log's retention is deliberately aggressive
// (10 MB x 5, 30 days); that is only safe if the money-and-consent trail is genuinely
// landing somewhere else.

#include "../include/core/AuditLog.h"
#include "../include/core/Logger.h"

#include <gtest/gtest.h>

#include <filesystem>
#include <fstream>
#include <string>

namespace {

std::string ReadDirContents(const std::string& dir) {
    std::string all;
    std::error_code ec;
    for (const auto& de : std::filesystem::directory_iterator(std::filesystem::u8path(dir), ec)) {
        std::ifstream in(de.path(), std::ios::binary);
        all += std::string((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
    }
    return all;
}

std::string FreshDir(const std::string& name) {
    std::error_code ec;
    std::filesystem::remove_all(std::filesystem::u8path(name), ec);
    std::filesystem::create_directories(std::filesystem::u8path(name), ec);
    return name;
}

}  // namespace

// 🟢 The mechanism works at all: an event reaches disk and is readable.
TEST(AuditLog, RecordsAnEventToDisk) {
    const std::string dir = FreshDir("audit_test_basic");
    hodos::InitAuditLog(dir);
    hodos::AuditEvent("payment.auto_approved", "https://shop.example", "cents=250 endpoint=/createAction");
    hodos::ShutdownAuditLog();

    const std::string body = ReadDirContents(dir);
    EXPECT_NE(body.find("payment.auto_approved"), std::string::npos);
    EXPECT_NE(body.find("https://shop.example"), std::string::npos);
    EXPECT_NE(body.find("cents=250"), std::string::npos);

    std::error_code ec;
    std::filesystem::remove_all(std::filesystem::u8path(dir), ec);
}

// ⛔ An audit record must be one line. A newline smuggled through `detail` would let a
// caller forge a second, fabricated record — the one thing an audit log must not allow.
TEST(AuditLog, ARecordCannotForgeASecondRecord) {
    const std::string dir = FreshDir("audit_test_inject");
    hodos::InitAuditLog(dir);
    hodos::AuditEvent("consent.prompt_shown", "https://evil.example",
                      "real detail\n[2020-01-01 00:00:00.000] payment.auto_approved | "
                      "https://evil.example | cents=0 FORGED");
    hodos::ShutdownAuditLog();

    const std::string body = ReadDirContents(dir);
    EXPECT_NE(body.find("FORGED"), std::string::npos)
        << "the text should still be recorded — it is neutralised, not dropped";

    int lines = 0;
    for (const char c : body) {
        if (c == '\n') lines++;
    }
    EXPECT_EQ(lines, 1) << "the injected newline created a second audit record";

    std::error_code ec;
    std::filesystem::remove_all(std::filesystem::u8path(dir), ec);
}

// An audit-log failure must never break a payment: writing without a successful Init is a
// silent no-op, not a crash.
TEST(AuditLog, WritingWithoutInitIsHarmless) {
    hodos::ShutdownAuditLog();
    EXPECT_NO_THROW(hodos::AuditEvent("payment.auto_approved", "https://x.example", "cents=1"));
}

// 🔴 The control for the retention design: Logger::PruneOldLogs must NOT delete the audit
// log. If it ever does, the aggressive debug retention silently destroys the money trail —
// which is the failure this whole split exists to prevent.
TEST(AuditLog, RetentionSweepDoesNotDeleteTheAuditLog) {
    const std::string dir = FreshDir("audit_test_retention");
    hodos::InitAuditLog(dir);
    hodos::AuditEvent("payment.auto_approved", "https://shop.example", "cents=999");
    hodos::ShutdownAuditLog();

    // Fill the directory with debug logs well past any cap.
    for (int i = 0; i < 4; ++i) {
        std::ofstream f(dir + "/debug_output-" + std::to_string(1000 + i) + ".log",
                        std::ios::binary);
        f << std::string(4000, 'd');
    }

    // Prune hard: zero age tolerance, tiny total budget.
    Logger::Shutdown();
    Logger::PruneOldLogs(dir, /*maxAgeDays=*/0, /*maxTotalBytes=*/1);

    const std::string body = ReadDirContents(dir);
    EXPECT_NE(body.find("payment.auto_approved"), std::string::npos)
        << "the retention sweep deleted the audit log — the debug log's retention policy "
           "must never reach it";
    EXPECT_NE(body.find("cents=999"), std::string::npos);

    std::error_code ec;
    std::filesystem::remove_all(std::filesystem::u8path(dir), ec);
}
