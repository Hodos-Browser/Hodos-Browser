// Logger::Log must be safe to call from several threads at once.
//
// WHY THE LOCK EXISTS. Logger::Log is called from the UI thread, the IO thread (every
// resource request) and CEF's worker threads, and had NO synchronisation. Unsynchronised
// concurrent writes to one std::ofstream are a data race -- undefined behaviour by the
// standard -- regardless of what any single implementation happens to do. That alone
// justifies the lock, and this test guards it.
//
// ⛔ WHAT THIS TEST DOES **NOT** SHOW, stated up front so nobody re-derives it:
// it does NOT demonstrate that the missing lock caused the 8,208 torn lines measured in
// the shipped production log (MEASUREMENTS.md M2). Three attempts to reproduce tearing
// with the lock disabled -- 8 threads x 400 lines, then 8 KB payloads, then 8 x 12,000 =
// 96,000 lines -- ALL produced ZERO torn lines on MSVC, whose CRT locks the underlying
// FILE*. The production fragments are real and their shape is real (a line losing its
// ~90-byte head), but their CAUSE is NOT established.
//
// ⛔ There is therefore no negative control here, and this test must not be cited as
// evidence that the tearing is fixed. A test that has never been seen to fail proves
// nothing; rather than ship one that passes vacuously, the gap is recorded.

#include "../include/core/Logger.h"

#include <gtest/gtest.h>

#include <atomic>
#include <cstdio>
#include <fstream>
#include <string>
#include <thread>
#include <vector>

namespace {

constexpr int kThreads = 8;

// ⭐ VOLUME is what makes this race observable, not line length -- and getting that wrong
// made the first two versions of this negative control VACUOUS.
//
// In the shipped production log the tear rate is 8,208 / 9,762,742 = **0.084 %**. A 1,600-
// line test therefore expects ~1.3 torn lines and routinely sees zero, which is exactly what
// happened: the control "passed" only because the classifier was miscounting Logger's own
// bookkeeping lines. Lengthening the payload to 8 KB did not help either -- MSVC's CRT locks
// the FILE*, so a single large insertion is not where the window is.
//
// At 8 x 12,000 the expected count is ~80, comfortably clear of zero.
//
// ⛔ If kLinesPerThread is ever lowered, the negative control stops failing and the locked
// test silently stops proving anything. The test says so itself when it fails.
constexpr int kLinesPerThread = 12000;

// Production-shaped: a request-log line is ~100-200 bytes. The observed tears lost a ~90-byte
// head, so the payload carries a distinctive head AND tail and the classifier checks both.
std::string Payload(int thread_id, int n) {
    return "HEAD" + std::string(120, static_cast<char>('A' + thread_id)) + "|t" +
           std::to_string(thread_id) + "|n" + std::to_string(n) + "|END";
}

struct Counts {
    int total = 0;
    int wellFormed = 0;
    int torn = 0;
};

// "Torn" uses the shape actually observed in the production log (M2): a fragment that does
// not begin with the '[' of a timestamp. A payload line that starts correctly but lost its
// "|END" marker is torn too -- that is the other half of the same interleave.
//
// ⛔ Logger writes two bookkeeping lines of its own ("Logger initialized for ...",
// "Logger shutting down"). They start with '[' and carry no payload marker, so they must be
// excluded explicitly. The first version of this classifier counted them as TORN and failed
// the locked case against perfectly correct code -- a wrong instrument, not a wrong lock.
Counts Classify(const std::string& path) {
    Counts c;
    std::ifstream in(path);
    std::string line;
    while (std::getline(in, line)) {
        if (line.empty()) continue;
        c.total++;

        if (line.empty() || line[0] != '[') {
            c.torn++;  // a fragment with no timestamp: the production shape
            continue;
        }
        if (line.find("|t") == std::string::npos) {
            continue;  // Logger's own bookkeeping line, not part of the payload set
        }
        if (line.size() >= 4 && line.compare(line.size() - 4, 4, "|END") == 0) {
            c.wellFormed++;
        } else {
            c.torn++;  // started clean, lost its tail to another thread's write
        }
    }
    return c;
}

Counts HammerLogger(const std::string& path, bool lockEnabled) {
    std::remove(path.c_str());
    // This test counts lines in ONE file, so rotation must be off -- otherwise 96,000 lines
    // cross the 10 MB default and the tail ends up in .1 where the classifier cannot see it.
    Logger::SetRotation(0, 0);
    Logger::SetMinLevel(LogLevel::INFO);
    Logger::SetLockEnabledForTesting(lockEnabled);
    Logger::Initialize(ProcessType::BROWSER, path);

    std::atomic<bool> go{false};
    std::vector<std::thread> threads;
    for (int t = 0; t < kThreads; ++t) {
        threads.emplace_back([t, &go] {
            while (!go.load(std::memory_order_acquire)) {
            }
            for (int n = 0; n < kLinesPerThread; ++n) {
                Logger::Log(Payload(t, n), 1, 2);
            }
        });
    }
    go.store(true, std::memory_order_release);
    for (auto& th : threads) th.join();

    Logger::Shutdown();
    Logger::SetLockEnabledForTesting(true);  // never leave the control off
    Logger::SetRotation(10u * 1024u * 1024u, 5);
    return Classify(path);
}

}  // namespace

// 🟢 GREEN — every line written is intact, and there are exactly as many as were written.
TEST(LoggerConcurrency, LockedWritesAreNeverTorn) {
    const Counts c = HammerLogger("logger_concurrency_locked.log", /*lockEnabled=*/true);

    EXPECT_EQ(c.torn, 0) << "a locked Logger must not interleave writes";
    EXPECT_EQ(c.wellFormed, kThreads * kLinesPerThread)
        << "every line written must appear exactly once -- a lock must not lose lines either";
}

// ⛔ The negative control that belongs here does not exist yet: see the header comment.
// Do not add one that passes -- the first two attempts "passed" only because the
// classifier was miscounting Logger's own bookkeeping lines.
