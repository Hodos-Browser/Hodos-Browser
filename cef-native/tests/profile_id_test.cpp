// Tests for ProfileManager::IsValidProfileId — F5 (HelicOps audit) profile-id
// validation. The id is used as a directory name and historically reached a
// process-launch / shell boundary, so it must reject path-traversal and shell
// metacharacters while still accepting every internally-generated id shape.

#include "core/ProfileManager.h"

#include <gtest/gtest.h>
#include <string>

// ---- accepts the generated id shapes (must not break legit profiles) ----

TEST(ProfileId, AcceptsDefault) {
    EXPECT_TRUE(ProfileManager::IsValidProfileId("Default"));
}

TEST(ProfileId, AcceptsModernProfileN) {
    EXPECT_TRUE(ProfileManager::IsValidProfileId("Profile_1"));
    EXPECT_TRUE(ProfileManager::IsValidProfileId("Profile_42"));
}

TEST(ProfileId, AcceptsLegacyProfileWithSpace) {
    // Legacy ids generated before the underscore form — must still validate or
    // we'd lock existing users out of their profiles.
    EXPECT_TRUE(ProfileManager::IsValidProfileId("Profile 1"));
}

TEST(ProfileId, AcceptsHyphenAndUnderscore) {
    EXPECT_TRUE(ProfileManager::IsValidProfileId("my-work_profile"));
}

// ---- rejects empties / overlong ----

TEST(ProfileId, RejectsEmpty) {
    EXPECT_FALSE(ProfileManager::IsValidProfileId(""));
}

TEST(ProfileId, RejectsOverlong) {
    EXPECT_FALSE(ProfileManager::IsValidProfileId(std::string(65, 'a')));
}

// ---- rejects shell metacharacters (the F5 injection vectors) ----

TEST(ProfileId, RejectsShellInjectionPayloads) {
    EXPECT_FALSE(ProfileManager::IsValidProfileId("\"; rm -rf ~ #"));
    EXPECT_FALSE(ProfileManager::IsValidProfileId("Default\" --args \"x"));
    EXPECT_FALSE(ProfileManager::IsValidProfileId("$(whoami)"));
    EXPECT_FALSE(ProfileManager::IsValidProfileId("a`id`b"));
    EXPECT_FALSE(ProfileManager::IsValidProfileId("a;b"));
    EXPECT_FALSE(ProfileManager::IsValidProfileId("a|b"));
    EXPECT_FALSE(ProfileManager::IsValidProfileId("a&b"));
    EXPECT_FALSE(ProfileManager::IsValidProfileId("a'b"));
}

// ---- rejects path traversal / separators (id is also a directory name) ----

TEST(ProfileId, RejectsPathTraversalAndSeparators) {
    EXPECT_FALSE(ProfileManager::IsValidProfileId("../etc"));
    EXPECT_FALSE(ProfileManager::IsValidProfileId("a/b"));
    EXPECT_FALSE(ProfileManager::IsValidProfileId("a\\b"));
    EXPECT_FALSE(ProfileManager::IsValidProfileId("."));
    EXPECT_FALSE(ProfileManager::IsValidProfileId(".."));
}

TEST(ProfileId, RejectsControlAndNewline) {
    EXPECT_FALSE(ProfileManager::IsValidProfileId("a\nb"));
    EXPECT_FALSE(ProfileManager::IsValidProfileId(std::string("a\0b", 3)));
}

// ============================================================================
// ProfileManager::ResolveStartup — CHUNK 1/2 + R7 startup-profile resolver.
// Pure decision logic; no CEF / filesystem. No "last-used" concept: picker-off
// falls back to the default (starred) profile. (PROFILE_STARTUP_PICKER_DESIGN.md §2)
// ============================================================================

namespace {
const std::vector<std::string> kTwo  = {"Default", "Profile_1"};
const std::vector<std::string> kOne  = {"Default"};
}  // namespace

// ---- explicit --profile= ----

TEST(ResolveStartup, ExplicitValidExistingOpensIt) {
    auto r = ProfileManager::ResolveStartup("Profile_1", kTwo, "Default",
                                            /*pickerEnabled=*/true);
    EXPECT_EQ(r.profileId, "Profile_1");
    EXPECT_FALSE(r.showPicker);        // explicit --profile bypasses the picker
}

TEST(ResolveStartup, ExplicitUnknownFallsBackToDefault) {
    // Valid shape, but not in the registry (R7): coherent default.
    auto r = ProfileManager::ResolveStartup("Profile_9", kTwo, "Default",
                                            /*pickerEnabled=*/false);
    EXPECT_EQ(r.profileId, "Default");
    EXPECT_FALSE(r.showPicker);
}

TEST(ResolveStartup, ExplicitInjectionPayloadFallsBackToDefault) {
    auto r = ProfileManager::ResolveStartup("\"; rm -rf ~ #", kTwo, "Default",
                                            /*pickerEnabled=*/false);
    EXPECT_EQ(r.profileId, "Default");
}

TEST(ResolveStartup, ExplicitInvalidWithMissingDefaultFallsToFirstRealProfile) {
    // R7 coherence: if the persisted defaultProfileId names a profile that no
    // longer exists, the invalid-arg fallback must still resolve to a real dir.
    const std::vector<std::string> reg = {"Profile_1", "Profile_2"};  // no "Default"
    auto r = ProfileManager::ResolveStartup("Profile_9", reg, "Default",
                                            /*pickerEnabled=*/false);
    EXPECT_EQ(r.profileId, "Profile_1");  // front of registry, not phantom "Default"
}

TEST(ResolveStartup, MangledLegacySpaceIdFallsBackCoherently) {
    // Registry has legacy "Profile 2"; a quote-stripped relaunch yields "Profile".
    // Exact-match miss -> default (coherent), not a silent wrong-profile landing.
    const std::vector<std::string> reg = {"Default", "Profile 2"};
    auto mangled = ProfileManager::ResolveStartup("Profile", reg, "Default", false);
    EXPECT_EQ(mangled.profileId, "Default");
    // The intact id still resolves to the real legacy profile.
    auto intact = ProfileManager::ResolveStartup("Profile 2", reg, "Default", false);
    EXPECT_EQ(intact.profileId, "Profile 2");
}

// ---- no --profile= (taskbar / desktop / Start no-arg launch) ----

TEST(ResolveStartup, NoArgSingleProfileOpensItNoPicker) {
    auto r = ProfileManager::ResolveStartup("", kOne, "Default", /*pickerEnabled=*/true);
    EXPECT_EQ(r.profileId, "Default");
    EXPECT_FALSE(r.showPicker);         // 1 profile never shows the picker
}

TEST(ResolveStartup, NoArgSingleNonDefaultProfileOpensTheSoleProfile) {
    const std::vector<std::string> reg = {"Profile_1"};  // Default missing
    auto r = ProfileManager::ResolveStartup("", reg, "Default", true);
    EXPECT_EQ(r.profileId, "Profile_1");
    EXPECT_FALSE(r.showPicker);
}

TEST(ResolveStartup, NoArgMultiPickerDisabledOpensDefault) {
    // Picker off -> the default (starred) profile, NOT any "last-used".
    auto r = ProfileManager::ResolveStartup("", kTwo, "Default", /*pickerEnabled=*/false);
    EXPECT_EQ(r.profileId, "Default");
    EXPECT_FALSE(r.showPicker);
}

TEST(ResolveStartup, NoArgMultiPickerDisabledMissingDefaultOpensFirst) {
    const std::vector<std::string> reg = {"Profile_1", "Profile_2"};  // no "Default"
    auto r = ProfileManager::ResolveStartup("", reg, "Default", /*pickerEnabled=*/false);
    EXPECT_EQ(r.profileId, "Profile_1");
    EXPECT_FALSE(r.showPicker);
}

TEST(ResolveStartup, NoArgMultiPickerEnabledEntersPickerMode) {
    auto r = ProfileManager::ResolveStartup("", kTwo, "Default", /*pickerEnabled=*/true);
    EXPECT_TRUE(r.showPicker);
    EXPECT_EQ(r.profileId, "Default");  // bypass target if the picker is skipped
}
