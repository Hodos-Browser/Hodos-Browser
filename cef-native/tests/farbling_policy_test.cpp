// C2 — FarblingPolicy: eTLD+1 keying, profile-seed persistence, domain-key derivation.
//
// These are the properties the whole farbling design rests on, and none of them can be
// checked by reading the code:
//   * a WRONG eTLD+1 in the over-grouping direction silently links unrelated first
//     parties to one fingerprint -- the exact harm first-party keying exists to prevent;
//   * a seed that does not survive a restart silently breaks the login stability this
//     migration was built to fix;
//   * a key that is not deterministic makes a site's fingerprint unstable, which is
//     itself a fingerprint.

#include <gtest/gtest.h>

#include <array>
#include <cstdio>
#include <fstream>
#include <string>

#include <nlohmann/json.hpp>

#include "../include/core/FarblingPolicy.h"

namespace {

// A scratch directory that cleans up after itself, so seed-persistence tests do not
// leak state into each other (or into a developer's real profile).
class TempProfileDir {
 public:
  TempProfileDir() {
    char tmpl[] = "hodos_fp_test_XXXXXX";
    // tmpnam is adequate here: single-threaded test, and we only need a unique name.
    path_ = std::string(std::tmpnam(nullptr));
#ifdef _WIN32
    system(("mkdir \"" + path_ + "\" >nul 2>&1").c_str());
#else
    system(("mkdir -p '" + path_ + "'").c_str());
#endif
    (void)tmpl;
  }
  ~TempProfileDir() {
#ifdef _WIN32
    system(("rmdir /s /q \"" + path_ + "\" >nul 2>&1").c_str());
#else
    system(("rm -rf '" + path_ + "'").c_str());
#endif
  }
  const std::string& path() const { return path_; }

 private:
  std::string path_;
};

std::string SettingsFile(const std::string& dir) {
#ifdef _WIN32
  return dir + "\\fingerprint_settings.json";
#else
  return dir + "/fingerprint_settings.json";
#endif
}

}  // namespace

// ---------------------------------------------------------------------------
// RegistrableDomain — the anti-linkage property
// ---------------------------------------------------------------------------

TEST(FarblingPolicyRegistrableDomain, StripsSubdomainsToTwoLabels) {
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("accounts.google.com"), "google.com");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("www.example.com"), "example.com");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("example.com"), "example.com");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("a.b.c.d.example.com"), "example.com");
}

TEST(FarblingPolicyRegistrableDomain, HandlesNewGtlds) {
  // New gTLDs are single-label suffixes; the default two-label rule is correct.
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("app.example.dev"), "example.dev");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("foo.example.xyz"), "example.xyz");
}

// THE test that matters. EphemeralCookieManager's helper returns "co.uk" here, which for
// farbling would put every unrelated UK site on one seed.
TEST(FarblingPolicyRegistrableDomain, DoesNotCollapseMultiLabelPublicSuffixes) {
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("www.example.co.uk"), "example.co.uk");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("example.co.uk"), "example.co.uk");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("shop.mysite.com.au"), "mysite.com.au");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("a.b.thing.co.jp"), "thing.co.jp");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("news.site.com.br"), "site.com.br");
}

TEST(FarblingPolicyRegistrableDomain, TwoUnrelatedSitesUnderOneSuffixStayDistinct) {
  // The linkage regression, stated directly.
  const std::string a = FarblingPolicy::RegistrableDomain("alice.co.uk");
  const std::string b = FarblingPolicy::RegistrableDomain("bob.co.uk");
  EXPECT_NE(a, b);
  EXPECT_EQ(a, "alice.co.uk");
  EXPECT_EQ(b, "bob.co.uk");
}

TEST(FarblingPolicyRegistrableDomain, BarePublicSuffixIsReturnedUnchanged) {
  // "co.uk" has no registrable domain; inventing one would be worse than passing through.
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("co.uk"), "co.uk");
}

TEST(FarblingPolicyRegistrableDomain, NormalisesCaseTrailingDotAndPort) {
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("WWW.Example.COM"), "example.com");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("www.example.com."), "example.com");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("www.example.com:8443"), "example.com");
}

TEST(FarblingPolicyRegistrableDomain, LeavesIpLiteralsAndSingleLabelHostsAlone) {
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("127.0.0.1"), "127.0.0.1");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("192.168.1.10"), "192.168.1.10");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("localhost"), "localhost");
  EXPECT_EQ(FarblingPolicy::RegistrableDomain("[::1]"), "[::1]");
}

TEST(FarblingPolicyRegistrableDomain, EmptyInputYieldsEmpty) {
  EXPECT_EQ(FarblingPolicy::RegistrableDomain(""), "");
}

TEST(FarblingPolicyRegistrableDomain, FromUrlStripsSchemePathAndUserinfo) {
  EXPECT_EQ(FarblingPolicy::RegistrableDomainFromUrl("https://accounts.google.com/signin"),
            "google.com");
  EXPECT_EQ(FarblingPolicy::RegistrableDomainFromUrl("http://www.example.co.uk:8080/a?b=c"),
            "example.co.uk");
  EXPECT_EQ(FarblingPolicy::RegistrableDomainFromUrl("https://user:pw@sub.example.com/x"),
            "example.com");
}

// ---------------------------------------------------------------------------
// Hex round-trip
// ---------------------------------------------------------------------------

TEST(FarblingPolicyHex, RoundTrips) {
  std::array<uint8_t, 32> in{};
  for (size_t i = 0; i < in.size(); ++i) in[i] = static_cast<uint8_t>(i * 7 + 1);
  std::array<uint8_t, 32> out{};
  ASSERT_TRUE(FarblingPolicy::DecodeHex(FarblingPolicy::EncodeHex(in), out));
  EXPECT_EQ(in, out);
}

TEST(FarblingPolicyHex, RejectsMalformedRatherThanDecodingPartially) {
  std::array<uint8_t, 32> out{};
  EXPECT_FALSE(FarblingPolicy::DecodeHex("", out));
  EXPECT_FALSE(FarblingPolicy::DecodeHex("abcd", out));                 // too short
  EXPECT_FALSE(FarblingPolicy::DecodeHex(std::string(64, 'z'), out));   // non-hex
  EXPECT_FALSE(FarblingPolicy::DecodeHex(std::string(65, 'a'), out));   // odd length
}

// ---------------------------------------------------------------------------
// ComputeDomainKey — determinism and separation
// ---------------------------------------------------------------------------

TEST(FarblingPolicyDomainKey, IsDeterministicForSameSeedAndDomain) {
  std::array<uint8_t, 32> seed{};
  seed.fill(0xA5);
  std::array<uint8_t, 32> k1{}, k2{};
  ASSERT_TRUE(FarblingPolicy::ComputeDomainKey(seed, "example.com", k1));
  ASSERT_TRUE(FarblingPolicy::ComputeDomainKey(seed, "example.com", k2));
  EXPECT_EQ(k1, k2);  // A site's farbled values must be stable, or instability is the tell.
}

TEST(FarblingPolicyDomainKey, DiffersPerDomain) {
  std::array<uint8_t, 32> seed{};
  seed.fill(0xA5);
  std::array<uint8_t, 32> a{}, b{};
  ASSERT_TRUE(FarblingPolicy::ComputeDomainKey(seed, "example.com", a));
  ASSERT_TRUE(FarblingPolicy::ComputeDomainKey(seed, "other.com", b));
  EXPECT_NE(a, b);  // Cross-site unlinkability.
}

TEST(FarblingPolicyDomainKey, DiffersPerProfileSeed) {
  std::array<uint8_t, 32> s1{}, s2{};
  s1.fill(0x01);
  s2.fill(0x02);
  std::array<uint8_t, 32> a{}, b{};
  ASSERT_TRUE(FarblingPolicy::ComputeDomainKey(s1, "example.com", a));
  ASSERT_TRUE(FarblingPolicy::ComputeDomainKey(s2, "example.com", b));
  EXPECT_NE(a, b);  // Two profiles are two identities.
}

TEST(FarblingPolicyDomainKey, RejectsEmptyDomain) {
  std::array<uint8_t, 32> seed{};
  seed.fill(0xA5);
  std::array<uint8_t, 32> k{};
  EXPECT_FALSE(FarblingPolicy::ComputeDomainKey(seed, "", k));
}

// ---------------------------------------------------------------------------
// EnsureProfileSeed — persistence, and co-existence with the user's per-site toggles
// ---------------------------------------------------------------------------

TEST(FarblingPolicySeed, GeneratesThenReloadsTheSameSeed) {
  TempProfileDir dir;
  std::array<uint8_t, 32> first{}, second{};
  ASSERT_TRUE(FarblingPolicy::EnsureProfileSeed(dir.path(), first));
  ASSERT_TRUE(FarblingPolicy::EnsureProfileSeed(dir.path(), second));
  EXPECT_EQ(first, second)
      << "the seed must survive a restart -- that is the login-stability fix";

  std::array<uint8_t, 32> zero{};
  EXPECT_NE(first, zero) << "a zero seed would mean the CSPRNG silently failed";
}

TEST(FarblingPolicySeed, PreservesExistingPerSiteTogglesInTheSharedFile) {
  TempProfileDir dir;
  // Simulate a user who has already turned Privacy Shield off for one site.
  {
    nlohmann::json j;
    j["siteSettings"]["example.com"]["enabled"] = false;
    std::ofstream out(SettingsFile(dir.path()));
    ASSERT_TRUE(out.is_open());
    out << j.dump(2);
  }

  std::array<uint8_t, 32> seed{};
  ASSERT_TRUE(FarblingPolicy::EnsureProfileSeed(dir.path(), seed));

  nlohmann::json j;
  std::ifstream in(SettingsFile(dir.path()));
  ASSERT_TRUE(in.is_open());
  in >> j;

  ASSERT_TRUE(j.contains("siteSettings"));
  ASSERT_TRUE(j["siteSettings"].contains("example.com"));
  EXPECT_FALSE(j["siteSettings"]["example.com"]["enabled"].get<bool>())
      << "writing the seed must not clobber a shipped user setting";
  EXPECT_TRUE(j.contains("profileSeed"));
}

TEST(FarblingPolicySeed, RegeneratesRatherThanFailingOnACorruptSeedField) {
  TempProfileDir dir;
  {
    nlohmann::json j;
    j["profileSeed"] = "not-valid-hex";
    std::ofstream out(SettingsFile(dir.path()));
    ASSERT_TRUE(out.is_open());
    out << j.dump(2);
  }
  std::array<uint8_t, 32> seed{};
  EXPECT_TRUE(FarblingPolicy::EnsureProfileSeed(dir.path(), seed));
  std::array<uint8_t, 32> zero{};
  EXPECT_NE(seed, zero);
}

TEST(FarblingPolicySeed, SurvivesAnEntirelyCorruptSettingsFile) {
  TempProfileDir dir;
  {
    std::ofstream out(SettingsFile(dir.path()));
    ASSERT_TRUE(out.is_open());
    out << "{ this is not json";
  }
  std::array<uint8_t, 32> seed{};
  EXPECT_TRUE(FarblingPolicy::EnsureProfileSeed(dir.path(), seed))
      << "a corrupt settings file must not brick farbling";
}

TEST(FarblingPolicySeed, TwoProfilesGetDifferentSeeds) {
  TempProfileDir a, b;
  std::array<uint8_t, 32> sa{}, sb{};
  ASSERT_TRUE(FarblingPolicy::EnsureProfileSeed(a.path(), sa));
  ASSERT_TRUE(FarblingPolicy::EnsureProfileSeed(b.path(), sb));
  EXPECT_NE(sa, sb);
}
