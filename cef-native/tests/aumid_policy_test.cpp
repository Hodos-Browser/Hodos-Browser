// Tests for hodos::ComputeAumid — beta.3 Phase 3 (WS2), evidence row P3-A1.
//
// WHAT THIS ROW DOES AND DOES NOT PROVE.
// It pins the AUMID *decision*: given (isDev, pickerMode, profileId), which identity
// string does this process claim? ⛔ It says NOTHING about whether Windows then names
// the taskbar button correctly — that needs a real install and a human (P3-A4/A5).
// Never cite this file as evidence that the taskbar is fixed.
//
// THE NEGATIVE CONTROL for this row, per HARNESS.md §2 — reintroduce the old gate in
// ComputeAumid on the same binary:
//
//     if (!isDev && profileCount <= 1) return std::nullopt;   // the pre-fix behaviour
//
// and `ProdSingleProfileGetsAnIdentity` below goes RED, because that is exactly the
// case the old code left unset. That test is the whole point of this file: the bug was
// not a wrong string, it was NO string in production single-profile — and, worse, an
// identity that changed the moment a second profile appeared. See AumidPolicy.h.

#include "core/AumidPolicy.h"

#include <gtest/gtest.h>
#include <string>

using hodos::ComputeAumid;

namespace {
// Readability at the call sites: ComputeAumid(isDev, pickerMode, profileId).
constexpr bool kProd = false;
constexpr bool kDev = true;
constexpr bool kNormal = false;
constexpr bool kPicker = true;
}  // namespace

// ---- the regression this phase exists to fix ----

TEST(AumidPolicy, ProdSingleProfileGetsAnIdentity) {
    // ⭐ THE ROW THAT MATTERS. Pre-fix this configuration — production, one profile,
    // i.e. the ordinary user — set no AUMID at all, so Windows could not match the
    // window to its shortcut and fell back to displaying "HodosBrowser.exe".
    const auto aumid = ComputeAumid(kProd, kNormal, "Default");
    ASSERT_TRUE(aumid.has_value());
    EXPECT_EQ(*aumid, L"HodosBrowser");
}

TEST(AumidPolicy, IdentityDoesNotDependOnProfileCount) {
    // The measured root cause: a user's identity CHANGED when they created a second
    // profile (2026-07-06), orphaning their pinned taskbar icon. ComputeAumid takes no
    // profile count at all, so that class of bug is now unrepresentable — this test
    // guards the signature as much as the value.
    EXPECT_EQ(*ComputeAumid(kProd, kNormal, "Default"), L"HodosBrowser");
    EXPECT_EQ(*ComputeAumid(kProd, kNormal, "Default"), L"HodosBrowser");
}

// ---- per-profile buttons are a FEATURE — the coexistence half ----

TEST(AumidPolicy, NonDefaultProfileGetsItsOwnIdentity) {
    // Extra profiles intentionally get their own taskbar button, as Chrome does.
    // This is the requirement pulling against the one above; P3-A5 is its live control.
    EXPECT_EQ(*ComputeAumid(kProd, kNormal, "Profile_1"), L"HodosBrowser.Profile_1");
    EXPECT_EQ(*ComputeAumid(kProd, kNormal, "Profile_42"), L"HodosBrowser.Profile_42");
}

TEST(AumidPolicy, DefaultProfileIsUnsuffixed) {
    // Must stay unsuffixed so it matches the plain shortcut the installer declares.
    // If this ever gained a suffix, the Default profile would stop grouping with the
    // Start Menu / desktop icon and the original bug would return for everyone.
    EXPECT_EQ(*ComputeAumid(kProd, kNormal, "Default"), L"HodosBrowser");
}

TEST(AumidPolicy, ProfilesDoNotCollide) {
    EXPECT_NE(*ComputeAumid(kProd, kNormal, "Default"),
              *ComputeAumid(kProd, kNormal, "Profile_1"));
    EXPECT_NE(*ComputeAumid(kProd, kNormal, "Profile_1"),
              *ComputeAumid(kProd, kNormal, "Profile_2"));
}

// ---- dev must never merge with the installed build ----

TEST(AumidPolicy, DevIsAlwaysDistinctFromProd) {
    EXPECT_EQ(*ComputeAumid(kDev, kNormal, "Default"), L"HodosBrowser.Dev");
    EXPECT_NE(*ComputeAumid(kDev, kNormal, "Default"),
              *ComputeAumid(kProd, kNormal, "Default"));
}

TEST(AumidPolicy, DevKeepsPerProfileSuffix) {
    EXPECT_EQ(*ComputeAumid(kDev, kNormal, "Profile_1"), L"HodosBrowser.Dev.Profile_1");
}

TEST(AumidPolicy, DevAndProdNeverCollideForAnyProfile) {
    // Guards the dev/prod deconfliction that f9408fd introduced: a dev build must not
    // take over the installed build's taskbar button, for ANY profile.
    const char* profiles[] = {"Default", "Profile_1", "Profile_2", "my-work_profile"};
    for (const char* p : profiles) {
        EXPECT_NE(*ComputeAumid(kDev, kNormal, p), *ComputeAumid(kProd, kNormal, p))
            << "dev and prod collided for profile " << p;
    }
}

// ---- the picker (Phase 3, Q5) ----

TEST(AumidPolicy, PickerTakesTheBaseIdentity) {
    // The picker owns no profile. Pre-fix it was skipped entirely and got NO identity,
    // despite a comment claiming it "keeps the base AUMID" — code and comment
    // disagreed. It now genuinely keeps the base one.
    EXPECT_EQ(*ComputeAumid(kProd, kPicker, "Default"), L"HodosBrowser");
    EXPECT_EQ(*ComputeAumid(kDev, kPicker, "Default"), L"HodosBrowser.Dev");
}

TEST(AumidPolicy, PickerIgnoresAnyProfileIdItWasHanded) {
    // The picker must not inherit a profile suffix even if one is passed — otherwise
    // it would masquerade as that profile's window in the taskbar.
    EXPECT_EQ(*ComputeAumid(kProd, kPicker, "Profile_1"), L"HodosBrowser");
}

// ---- defensive ----

TEST(AumidPolicy, UnresolvedProfileFallsBackToBaseRatherThanEmptySuffix) {
    // An empty id means the profile never resolved. Appending nothing would silently
    // produce the base identity and hide the failure; the caller logs the difference.
    EXPECT_EQ(*ComputeAumid(kProd, kNormal, ""), L"HodosBrowser");
}

TEST(AumidPolicy, AlwaysProducesAnIdentity) {
    // The whole fix in one assertion: there is no input for which a real launch ends
    // up with no identity. ⛔ Reintroducing any gate breaks this first.
    const char* profiles[] = {"Default", "Profile_1", ""};
    for (bool dev : {false, true}) {
        for (bool picker : {false, true}) {
            for (const char* p : profiles) {
                EXPECT_TRUE(ComputeAumid(dev, picker, p).has_value());
            }
        }
    }
}

// ---- the contract with the installer ----

TEST(AumidPolicy, ProdBaseMatchesTheInstallerDeclaredString) {
    // ⛔ installer/hodos-browser.iss [Icons] declares AppUserModelID: "HodosBrowser"
    // on both shortcut entries. Windows compares these strings to decide whether a
    // window and a shortcut are the same application, so if one side is edited and
    // the other is not, the taskbar bug returns silently. This test is the tripwire
    // for that drift — if you change it, change the .iss in the same commit.
    EXPECT_EQ(std::wstring(hodos::kAumidBaseProd), L"HodosBrowser");
    EXPECT_EQ(*ComputeAumid(kProd, kNormal, "Default"), std::wstring(hodos::kAumidBaseProd));
}
