// P2-A3 / P2-A6 — the level gate and log rotation.
//
// ⚠️ A logging gate is unusually easy to fake: "the log is smaller" is satisfied perfectly
// by writing nothing at all. Every GREEN here is therefore paired with a check that the
// lines which SHOULD still be there still are. The flood and the blackhole are the same
// phase, and fixing one by widening the other is the obvious failure mode.

#include "../include/core/Logger.h"

#include <gtest/gtest.h>

#include <cstdio>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

namespace {

std::string ReadAll(const std::string& path) {
    std::ifstream in(path, std::ios::binary);
    return std::string((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
}

int CountLines(const std::string& path) {
    std::ifstream in(path);
    std::string line;
    int n = 0;
    while (std::getline(in, line)) {
        if (!line.empty()) n++;
    }
    return n;
}

// Fresh file, fresh Logger state.
void Reset(const std::string& path) {
    Logger::Shutdown();
    std::error_code ec;
    std::filesystem::remove(std::filesystem::u8path(path), ec);
    for (int i = 1; i <= 12; ++i) {
        std::filesystem::remove(std::filesystem::u8path(path + "." + std::to_string(i)), ec);
    }
    Logger::SetRotation(10u * 1024u * 1024u, 5);
    Logger::SetMinLevel(LogLevel::INFO);
}

}  // namespace

// ── The level gate ───────────────────────────────────────────────────────────────────

// 🟢 At the production default, DEBUG is dropped and everything else survives.
TEST(LoggerLevelGate, ProductionDefaultDropsDebugAndKeepsTheRest) {
    const std::string path = "logger_gate_prod.log";
    Reset(path);
    Logger::Initialize(ProcessType::BROWSER, path);

    Logger::Log("NOISE-resource-request", 0, 2);   // DEBUG
    Logger::Log("KEEP-startup", 1, 2);             // INFO
    Logger::Log("KEEP-warning", 2, 2);             // WARNING
    Logger::Log("KEEP-error", 3, 2);               // ERROR
    Logger::Shutdown();

    const std::string body = ReadAll(path);
    EXPECT_EQ(body.find("NOISE-resource-request"), std::string::npos)
        << "DEBUG must not reach disk at the production default";

    // ⭐ The anti-fake half. Without these three, "no NOISE" is satisfied by a logger that
    // writes nothing at all -- which is exactly how a level gate gets shipped broken.
    EXPECT_NE(body.find("KEEP-startup"), std::string::npos);
    EXPECT_NE(body.find("KEEP-warning"), std::string::npos);
    EXPECT_NE(body.find("KEEP-error"), std::string::npos);
}

// 🔴 The control for the row above: the SAME call, the SAME binary, level lowered. If this
// line does not appear, the gate is not discriminating -- it is just broken.
TEST(LoggerLevelGate, NegativeControlDebugLevelLetsTheSameLineThrough) {
    const std::string path = "logger_gate_debug.log";
    Reset(path);
    Logger::SetMinLevel(LogLevel::DEBUG);
    Logger::Initialize(ProcessType::BROWSER, path);

    Logger::Log("NOISE-resource-request", 0, 2);
    Logger::Shutdown();
    Logger::SetMinLevel(LogLevel::INFO);

    EXPECT_NE(ReadAll(path).find("NOISE-resource-request"), std::string::npos)
        << "the gate suppresses DEBUG at every level -- it is disabling logging, not gating it";
}

// ERROR must survive the strictest setting. The failure mode being ruled out is a gate that
// silences the log entirely and is then called fixed.
TEST(LoggerLevelGate, ErrorSurvivesEvenAtTheStrictestLevel) {
    const std::string path = "logger_gate_strict.log";
    Reset(path);
    Logger::SetMinLevel(LogLevel::ERROR_LEVEL);
    Logger::Initialize(ProcessType::BROWSER, path);

    Logger::Log("dropped-info", 1, 2);
    Logger::Log("KEEP-error", 3, 2);
    Logger::Shutdown();
    Logger::SetMinLevel(LogLevel::INFO);

    const std::string body = ReadAll(path);
    EXPECT_EQ(body.find("dropped-info"), std::string::npos);
    EXPECT_NE(body.find("KEEP-error"), std::string::npos);
}

TEST(LoggerLevelGate, IsEnabledAgreesWithWhatActuallyGetsWritten) {
    Logger::SetMinLevel(LogLevel::INFO);
    EXPECT_FALSE(Logger::IsEnabled(0));
    EXPECT_TRUE(Logger::IsEnabled(1));
    EXPECT_TRUE(Logger::IsEnabled(3));
    Logger::SetMinLevel(LogLevel::DEBUG);
    EXPECT_TRUE(Logger::IsEnabled(0));
    Logger::SetMinLevel(LogLevel::INFO);
}

// ── Rotation ─────────────────────────────────────────────────────────────────────────

// 🟢 Two-sided by construction: the total is capped AND a line written before the boundary
// is still findable afterwards. A "rotation" that simply truncates passes the first half
// and fails the second.
TEST(LoggerRotation, CapsTotalBytesAndKeepsContentAcrossTheBoundary) {
    const std::string path = "logger_rotate.log";
    Reset(path);
    Logger::SetRotation(4096, 3);
    Logger::Initialize(ProcessType::BROWSER, path);

    Logger::Log("MARKER-written-before-any-rotation", 1, 2);
    // ~40 x 215 B = 8.6 KB: crosses the 4 KB boundary twice, but stays well inside the
    // retained capacity (4 files x 4 KB). ⛔ The first version wrote 43 KB into 16 KB of
    // capacity, so the marker was dropped for the RIGHT reason and the test failed against
    // correct code -- retention doing its job is not a truncation bug.
    const std::string filler(200, 'x');
    for (int i = 0; i < 40; ++i) {
        Logger::Log(filler + "|" + std::to_string(i), 1, 2);
    }
    Logger::Shutdown();

    std::error_code ec;
    std::uintmax_t total = 0;
    int files = 0;
    for (const std::string& p : {path, path + ".1", path + ".2", path + ".3", path + ".4"}) {
        const auto fp = std::filesystem::u8path(p);
        if (std::filesystem::exists(fp, ec)) {
            total += std::filesystem::file_size(fp, ec);
            files++;
        }
    }

    EXPECT_LE(files, 4) << "keepFiles=3 means the live file plus at most 3 rotations";
    EXPECT_LE(total, static_cast<std::uintmax_t>(4096) * 5)
        << "rotation is not bounding total bytes on disk";

    // ⭐ The anti-truncation half: the marker must still exist SOMEWHERE in the set.
    bool found = false;
    for (const std::string& p : {path, path + ".1", path + ".2", path + ".3"}) {
        if (ReadAll(p).find("MARKER-written-before-any-rotation") != std::string::npos) {
            found = true;
            break;
        }
    }
    EXPECT_TRUE(found)
        << "the line written before the first rotation was destroyed -- that is truncation "
           "wearing rotation's clothes";
}

// 🔴 Control: with rotation disabled the same workload blows straight past the cap. If this
// does not exceed it, the workload is too small and the GREEN above proves nothing.
TEST(LoggerRotation, NegativeControlWithoutRotationTheFileExceedsTheCap) {
    const std::string path = "logger_rotate_off.log";
    Reset(path);
    Logger::SetRotation(0, 0);  // 0 = never rotate
    Logger::Initialize(ProcessType::BROWSER, path);

    const std::string filler(200, 'x');
    for (int i = 0; i < 200; ++i) {
        Logger::Log(filler + "|" + std::to_string(i), 1, 2);
    }
    Logger::Shutdown();
    Logger::SetRotation(10u * 1024u * 1024u, 5);

    std::error_code ec;
    const auto size = std::filesystem::file_size(std::filesystem::u8path(path), ec);
    EXPECT_GT(size, static_cast<std::uintmax_t>(4096))
        << "the workload never reached the cap, so the rotation test above is vacuous";
}

// A restart must not reset the rotation budget -- otherwise a long-lived install grows
// without bound one session at a time, which is precisely the shipped defect.
TEST(LoggerRotation, RestartDoesNotResetTheBudget) {
    const std::string path = "logger_rotate_restart.log";
    Reset(path);
    Logger::SetRotation(4096, 3);

    for (int session = 0; session < 6; ++session) {
        Logger::Initialize(ProcessType::BROWSER, path);
        const std::string filler(200, 'y');
        for (int i = 0; i < 20; ++i) Logger::Log(filler + "|" + std::to_string(i), 1, 2);
        Logger::Shutdown();
    }

    std::error_code ec;
    const auto live = std::filesystem::file_size(std::filesystem::u8path(path), ec);
    EXPECT_LE(live, static_cast<std::uintmax_t>(4096) * 2)
        << "the live file grew past the cap across restarts -- Initialize is not seeding "
           "bytesWritten from what is already on disk";
}

// ── Retention ────────────────────────────────────────────────────────────────────────

TEST(LoggerRetention, PrunesByAgeAndTotalSizeButNeverTouchesForeignFiles) {
    const std::string dir = "logger_retention_dir";
    std::error_code ec;
    std::filesystem::remove_all(std::filesystem::u8path(dir), ec);
    std::filesystem::create_directories(std::filesystem::u8path(dir), ec);

    auto write = [&](const std::string& name, std::size_t bytes) {
        std::ofstream f(dir + "/" + name, std::ios::binary);
        f << std::string(bytes, 'z');
    };
    write("debug_output-111.log", 4000);
    write("debug_output-111.log.1", 4000);
    write("debug_output-222.log", 4000);
    // ⛔ The wallet's rotating logs share this directory and are NOT ours to delete.
    write("wallet_rCURRENT.log", 4000);
    write("cef_debug.log", 4000);

    Logger::Shutdown();  // clear logFilePath so nothing is treated as "current"
    const int removed = Logger::PruneOldLogs(dir, /*maxAgeDays=*/0, /*maxTotalBytes=*/5000);

    EXPECT_GT(removed, 0) << "nothing was pruned even though the total was over the cap";
    EXPECT_TRUE(std::filesystem::exists(std::filesystem::u8path(dir + "/wallet_rCURRENT.log"), ec))
        << "pruning deleted the WALLET's log -- it shares the directory and is not ours";
    EXPECT_TRUE(std::filesystem::exists(std::filesystem::u8path(dir + "/cef_debug.log"), ec))
        << "pruning deleted Chromium's log, which this function does not own";

    std::uintmax_t total = 0;
    for (const auto& de : std::filesystem::directory_iterator(std::filesystem::u8path(dir), ec)) {
        std::string n;
        try { n = de.path().filename().string(); } catch (...) { continue; }
        if (n.rfind("debug_output", 0) == 0) total += de.file_size(ec);
    }
    EXPECT_LE(total, static_cast<std::uintmax_t>(5000));

    std::filesystem::remove_all(std::filesystem::u8path(dir), ec);
}
