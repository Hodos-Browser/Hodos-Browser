// update_apply_test.cpp — unit tests for the pure apply-transaction model
// (commit 6b.1). Covers ApplyPhase <-> string, the three JSON (de)serializers
// (round-trip + lenient/garbage handling), and manifest key normalization.
// All pure — no filesystem, no CEF, no process spawning.

#include "core/UpdateApply.h"

#include <gtest/gtest.h>

using namespace hodos;

// ---- ApplyPhase <-> string --------------------------------------------------
TEST(ApplyPhase, RoundTripsEveryNamedPhase) {
    const ApplyPhase phases[] = {
        ApplyPhase::None, ApplyPhase::Preparing, ApplyPhase::Armed,
        ApplyPhase::Installing, ApplyPhase::AwaitingHealth, ApplyPhase::Healthy,
        ApplyPhase::RolledBack, ApplyPhase::Aborted};
    for (ApplyPhase p : phases) {
        EXPECT_EQ(ApplyPhaseFromString(ApplyPhaseToString(p)), p);
    }
}

TEST(ApplyPhase, UnknownStringIsNone) {
    EXPECT_EQ(ApplyPhaseFromString("not-a-phase"), ApplyPhase::None);
    EXPECT_EQ(ApplyPhaseFromString(""), ApplyPhase::None);
}

TEST(ApplyPhase, OnDiskStringsAreStable) {
    // These exact strings are the on-disk contract a watchdog reads — pin them.
    EXPECT_STREQ(ApplyPhaseToString(ApplyPhase::Installing), "installing");
    EXPECT_STREQ(ApplyPhaseToString(ApplyPhase::AwaitingHealth), "awaiting-health");
    EXPECT_STREQ(ApplyPhaseToString(ApplyPhase::Healthy), "healthy");
}

// ---- ApplyRecord ------------------------------------------------------------
TEST(ApplyRecord, RoundTripsAllFields) {
    ApplyRecord in;
    in.schema = 1;
    in.phase = ApplyPhase::AwaitingHealth;
    in.fromBuild = 412;
    in.toBuild = 413;
    in.installerPath = "C:\\u\\pending\\HodosBrowser-0.4.1-setup.exe";
    in.rollbackDir = "C:\\u\\pending\\rollback";
    in.rollbackManifestPath = "C:\\u\\pending\\rollback\\manifest.json";
    in.expectedNewManifestPath = "C:\\u\\pending\\expected-new-manifest.json";
    in.profileId = "Default";
    in.stagedAt = "2026-06-29T12:00:00Z";
    in.failureReason = "";

    ApplyRecord out;
    ASSERT_TRUE(ParseApplyRecord(SerializeApplyRecord(in), out));
    EXPECT_EQ(out.phase, in.phase);
    EXPECT_EQ(out.fromBuild, in.fromBuild);
    EXPECT_EQ(out.toBuild, in.toBuild);
    EXPECT_EQ(out.installerPath, in.installerPath);
    EXPECT_EQ(out.rollbackDir, in.rollbackDir);
    EXPECT_EQ(out.rollbackManifestPath, in.rollbackManifestPath);
    EXPECT_EQ(out.expectedNewManifestPath, in.expectedNewManifestPath);
    EXPECT_EQ(out.profileId, in.profileId);
    EXPECT_EQ(out.stagedAt, in.stagedAt);
}

TEST(ApplyRecord, GarbageJsonReturnsFalse) {
    ApplyRecord out;
    EXPECT_FALSE(ParseApplyRecord("not json", out));
    EXPECT_FALSE(ParseApplyRecord("[1,2,3]", out));   // array, not object
    EXPECT_FALSE(ParseApplyRecord("", out));
}

TEST(ApplyRecord, MissingFieldsGetSafeDefaults) {
    ApplyRecord out;
    out.toBuild = 999;  // ensure it gets reset
    ASSERT_TRUE(ParseApplyRecord("{\"phase\":\"armed\"}", out));
    EXPECT_EQ(out.phase, ApplyPhase::Armed);
    EXPECT_EQ(out.toBuild, 0);                 // absent -> default
    EXPECT_TRUE(out.installerPath.empty());
}

TEST(ApplyRecord, WrongTypeFieldIsIgnoredNotCrashed) {
    ApplyRecord out;
    // toBuild as a string should be ignored (lenient), not throw.
    ASSERT_TRUE(ParseApplyRecord("{\"toBuild\":\"oops\",\"phase\":\"healthy\"}", out));
    EXPECT_EQ(out.toBuild, 0);
    EXPECT_EQ(out.phase, ApplyPhase::Healthy);
}

// ---- UpdateState ------------------------------------------------------------
TEST(UpdateState, RoundTripsAllFields) {
    UpdateState in;
    in.silent = true;
    in.paused = true;
    in.highWaterBuild = 413;
    in.signerThumbprint = "AB12CD34";
    in.lastFailureBuild = 414;
    in.lastFailureReason = "health timeout";
    in.rescanAfterRollback = true;

    UpdateState out;
    ASSERT_TRUE(ParseUpdateState(SerializeUpdateState(in), out));
    EXPECT_TRUE(out.silent);
    EXPECT_TRUE(out.paused);
    EXPECT_EQ(out.highWaterBuild, 413);
    EXPECT_EQ(out.signerThumbprint, "AB12CD34");
    EXPECT_EQ(out.lastFailureBuild, 414);
    EXPECT_EQ(out.lastFailureReason, "health timeout");
    EXPECT_TRUE(out.rescanAfterRollback);
}

TEST(UpdateState, GarbageReturnsFalse) {
    UpdateState out;
    EXPECT_FALSE(ParseUpdateState("nope", out));
    EXPECT_FALSE(ParseUpdateState("42", out));
}

TEST(UpdateState, DefaultsAreFailSafeOff) {
    // A fresh/empty object must NOT read as silent-eligible (fail-safe-off, V3-7).
    UpdateState out;
    ASSERT_TRUE(ParseUpdateState("{}", out));
    EXPECT_FALSE(out.silent);
    EXPECT_FALSE(out.paused);
    EXPECT_EQ(out.highWaterBuild, 0);
}

// ---- FileManifest + key normalization ---------------------------------------
TEST(FileManifest, RoundTrips) {
    FileManifest in;
    in.entries["hodosbrowser.exe"] = "aa";
    in.entries["locales/en-us.pak"] = "bb";

    FileManifest out;
    ASSERT_TRUE(ParseManifest(SerializeManifest(in), out));
    EXPECT_EQ(out.entries.size(), 2u);
    EXPECT_EQ(out.entries["hodosbrowser.exe"], "aa");
    EXPECT_EQ(out.entries["locales/en-us.pak"], "bb");
}

TEST(FileManifest, ParseNormalizesKeys) {
    // Keys written with backslashes / mixed case / leading ./ must normalize so
    // the backup-verify and the install-tree match regardless of how they were written.
    FileManifest out;
    ASSERT_TRUE(ParseManifest(
        "{\"files\":{\".\\\\Locales\\\\EN-US.pak\":\"bb\",\"HodosBrowser.exe\":\"aa\"}}", out));
    EXPECT_EQ(out.entries.count("locales/en-us.pak"), 1u);
    EXPECT_EQ(out.entries.count("hodosbrowser.exe"), 1u);
}

TEST(FileManifest, MissingFilesObjectIsFalse) {
    FileManifest out;
    EXPECT_FALSE(ParseManifest("{\"schema\":1}", out));   // no "files"
    EXPECT_FALSE(ParseManifest("garbage", out));
}

TEST(NormalizeManifestKey, BackslashLowercaseStrip) {
    EXPECT_EQ(NormalizeManifestKey("Locales\\EN-US.pak"), "locales/en-us.pak");
    EXPECT_EQ(NormalizeManifestKey("./HodosBrowser.exe"), "hodosbrowser.exe");
    EXPECT_EQ(NormalizeManifestKey("/libcef.dll"), "libcef.dll");
    EXPECT_EQ(NormalizeManifestKey("frontend/assets/x.JS"), "frontend/assets/x.js");
}
