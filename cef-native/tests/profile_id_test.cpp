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
