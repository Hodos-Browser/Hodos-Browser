// silent_state_writer_test.cpp — pure-logic tests for the silent-eligibility mirror's
// safety-critical helpers (commit #1). See AUTOUPDATE_SILENT_STATE_WRITER_DESIGN.md.
//
// The FS read-modify-write (MirrorSilentEligibility) is exercised end-to-end by the
// real-build rig (scripts/setup-real-apply-test.ps1, with hand-seeding removed); here we
// cover the pure decision logic that keeps the "never silently enable what the user did
// not choose" guarantee (R1/R3/R5).

#include <gtest/gtest.h>
#include "core/SilentStateWriter.h"

using namespace hodos;

// off < notify < silent; unknown -> safest.
TEST(UpdateModeRank, OrdersByPermissiveness) {
    EXPECT_EQ(UpdateModeRank("off"), 0);
    EXPECT_EQ(UpdateModeRank("notify"), 1);
    EXPECT_EQ(UpdateModeRank("silent"), 2);
    EXPECT_EQ(UpdateModeRank("bogus"), 0);  // unknown -> safest
    EXPECT_EQ(UpdateModeRank(""), 0);
    EXPECT_LT(UpdateModeRank("off"), UpdateModeRank("notify"));
    EXPECT_LT(UpdateModeRank("notify"), UpdateModeRank("silent"));
}

// The one-time global collapse must never promote to silent over an explicit safer value.
TEST(MoreConservativeMode, PrefersTheSaferValue) {
    // R1: a stale global "silent" must not override a live per-profile "notify".
    EXPECT_EQ(MoreConservativeMode("silent", "notify"), "notify");
    EXPECT_EQ(MoreConservativeMode("notify", "silent"), "notify");
    // off beats everything.
    EXPECT_EQ(MoreConservativeMode("silent", "off"), "off");
    EXPECT_EQ(MoreConservativeMode("off", "notify"), "off");
    EXPECT_EQ(MoreConservativeMode("notify", "off"), "off");
    // Ties.
    EXPECT_EQ(MoreConservativeMode("silent", "silent"), "silent");
    EXPECT_EQ(MoreConservativeMode("notify", "notify"), "notify");
    EXPECT_EQ(MoreConservativeMode("off", "off"), "off");
}

// Mirror sets silent from the mode and preserves EVERY other field (never touches paused,
// highWater, lastFailure, rescan).
TEST(ComputeSilentEligibility, MirrorsModeAndPreservesAllOtherFields) {
    UpdateState s;
    s.silent = false;
    s.paused = true;
    s.highWaterBuild = 40199;
    s.signerThumbprint = "AABBCC";
    s.lastFailureBuild = 40100;
    s.lastFailureReason = "signer changed";
    s.rescanAfterRollback = true;

    ASSERT_TRUE(ComputeSilentEligibility(s, "silent"));  // changed false -> true
    EXPECT_TRUE(s.silent);
    // Everything else untouched — critically, `paused` (the bad-build latch) is preserved.
    EXPECT_TRUE(s.paused);
    EXPECT_EQ(s.highWaterBuild, 40199);
    EXPECT_EQ(s.signerThumbprint, "AABBCC");
    EXPECT_EQ(s.lastFailureBuild, 40100);
    EXPECT_EQ(s.lastFailureReason, "signer changed");
    EXPECT_TRUE(s.rescanAfterRollback);
}

// R5 fail-safe direction: notify / off / unknown / empty are all NOT silent.
TEST(ComputeSilentEligibility, NonSilentModesClearSilent) {
    for (const char* m : {"notify", "off", "bogus", ""}) {
        UpdateState s;
        s.silent = true;
        EXPECT_TRUE(ComputeSilentEligibility(s, m)) << "mode=" << m;   // changed true -> false
        EXPECT_FALSE(s.silent) << "mode=" << m;
    }
}

// No spurious write when the value already matches (esp. notify + fresh default false).
TEST(ComputeSilentEligibility, NoChangeReturnsFalse) {
    UpdateState alreadySilent;
    alreadySilent.silent = true;
    EXPECT_FALSE(ComputeSilentEligibility(alreadySilent, "silent"));
    EXPECT_TRUE(alreadySilent.silent);

    UpdateState freshDefault;  // silent defaults false
    EXPECT_FALSE(ComputeSilentEligibility(freshDefault, "notify"));
    EXPECT_FALSE(ComputeSilentEligibility(freshDefault, "off"));
    EXPECT_FALSE(freshDefault.silent);
}
