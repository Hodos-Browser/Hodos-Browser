// port_config_origin_test.cpp — P0.5 C1 (row P0.5-X4)
//
// Covers hodos::OriginFromUrl / AuthorityHasHost / IsInternalFrontendUrl /
// IsLoopbackUrl in include/core/PortConfig.h.
//
// ⛔ NEGATIVE CONTROL (CLAUDE.md hard rule). Every assertion here must be seen
// to FAIL against the pre-fix code. To run the control, restore the old bodies:
//
//   IsInternalFrontendUrl := url.rfind("http://127.0.0.1:5137", 0) == 0
//                         || url.rfind("http://localhost:5137", 0) == 0
//                         || url.rfind("hodos://", 0) == 0;
//   IsLoopbackUrl         := prefix list {http,https}x{127.0.0.1,localhost,[::1]}
//   OriginFromUrl         := u.find("://") + substr to first '/'   (unanchored)
//
// Expected RED under those bodies, per suite:
//   SchemeIsAnchored.*      — OriginFromUrl did not exist; the unanchored parse
//                             returns "127.0.0.1:5137" for the data: URLs below
//   UserinfoIsStripped.*    — returns "127.0.0.1:5137@evil.com" (or prefix-matches)
//   InternalFrontend.Userinfo{,Https}IsNotInternal — returned TRUE (the bypass)
//   Loopback.HostIsTerminated                     — returned TRUE (localhost.evil.com)
//
// A run in which these pass without that control having been observed proves
// nothing. See development-docs/0.4.0-beta.3/HARNESS.md §6.

#include <gtest/gtest.h>

#include "../include/core/PortConfig.h"

using hodos::AuthorityHasHost;
using hodos::IsInternalFrontendUrl;
using hodos::IsLoopbackUrl;
using hodos::OriginFromUrl;

// ---------------------------------------------------------------------------
// OriginFromUrl — scheme anchoring
// ---------------------------------------------------------------------------

TEST(SchemeIsAnchored, OrdinaryUrlsParseToTheirAuthority) {
    EXPECT_EQ(OriginFromUrl("http://127.0.0.1:5137/"), "127.0.0.1:5137");
    EXPECT_EQ(OriginFromUrl("http://127.0.0.1:5137"), "127.0.0.1:5137");
    EXPECT_EQ(OriginFromUrl("https://example.com/a/b?c=d"), "example.com");
    EXPECT_EQ(OriginFromUrl("https://example.com:8443/x"), "example.com:8443");
}

// THE C1 BYPASS. The old parse searched the whole URL for "://", so an
// attacker-authored frame URL supplied its own "origin" and the caller's
// ancestor cascade + opaque-origin sentinel were never reached.
TEST(SchemeIsAnchored, CraftedDataUrlYieldsNoOrigin) {
    EXPECT_EQ(OriginFromUrl("data:text/html,a://127.0.0.1:5137/"), "");
    EXPECT_EQ(OriginFromUrl("data:text/html,a://127.0.0.1:5137/<script>x</script>"), "");
    EXPECT_EQ(OriginFromUrl("about:blank?a://127.0.0.1:5137/"), "");
    EXPECT_EQ(OriginFromUrl("data:text/html,a://trusted-dapp.example/"), "");
}

TEST(SchemeIsAnchored, NonSpecialSchemesYieldNoOrigin) {
    EXPECT_EQ(OriginFromUrl("about:blank"), "");
    EXPECT_EQ(OriginFromUrl("hodos://settings"), "");   // deliberately no origin
    EXPECT_EQ(OriginFromUrl("file:///C:/x"), "");
    EXPECT_EQ(OriginFromUrl("javascript:alert(1)"), "");
    EXPECT_EQ(OriginFromUrl(""), "");
}

// A REAL scheme always puts its "://" first, so these must still parse
// correctly — the fix must not over-reject. (Panel "what survived" item 4.)
TEST(SchemeIsAnchored, RealSchemesAreNotBrokenByTheFix) {
    EXPECT_EQ(OriginFromUrl("https://evil.com/?x=a://127.0.0.1/"), "evil.com");
    EXPECT_EQ(OriginFromUrl("https://evil.com/#a://127.0.0.1:5137/"), "evil.com");
    // blob: has no authority of its own; the inner URL must not be harvested.
    EXPECT_EQ(OriginFromUrl("blob:https://evil.com/uuid"), "");
}

// ---------------------------------------------------------------------------
// OriginFromUrl — userinfo
// ---------------------------------------------------------------------------

TEST(UserinfoIsStripped, CredentialsAreNotTheHost) {
    EXPECT_EQ(OriginFromUrl("http://127.0.0.1:9@evil.com/"), "evil.com");
    EXPECT_EQ(OriginFromUrl("http://127.0.0.1:5137@evil.com/"), "evil.com");
    EXPECT_EQ(OriginFromUrl("https://user:pass@example.com/x"), "example.com");
    // Last '@' wins — an embedded '@' in the userinfo must not shorten the strip.
    EXPECT_EQ(OriginFromUrl("http://a@b@real.example/"), "real.example");
}

TEST(UserinfoIsStripped, EmptyAuthorityFailsClosed) {
    EXPECT_EQ(OriginFromUrl("http:///path"), "");
    EXPECT_EQ(OriginFromUrl("http://@evil.com/"), "evil.com");
}

// ---------------------------------------------------------------------------
// AuthorityHasHost — termination
// ---------------------------------------------------------------------------

TEST(AuthorityTermination, ExactOrPortOnly) {
    EXPECT_TRUE(AuthorityHasHost("127.0.0.1", "127.0.0.1"));
    EXPECT_TRUE(AuthorityHasHost("127.0.0.1:31301", "127.0.0.1"));
    EXPECT_TRUE(AuthorityHasHost("localhost:5137", "localhost"));

    EXPECT_FALSE(AuthorityHasHost("127.0.0.1.evil.com", "127.0.0.1"));
    EXPECT_FALSE(AuthorityHasHost("localhost.evil.com", "localhost"));
    EXPECT_FALSE(AuthorityHasHost("localhostevil.com", "localhost"));
    EXPECT_FALSE(AuthorityHasHost("", "localhost"));
}

// ---------------------------------------------------------------------------
// IsInternalFrontendUrl — the trust boundary
// ---------------------------------------------------------------------------

TEST(InternalFrontend, TheRealFrontendStillQualifies) {
    // Every first-party literal in cef-native/ uses this host form.
    EXPECT_TRUE(IsInternalFrontendUrl("http://127.0.0.1:5137"));
    EXPECT_TRUE(IsInternalFrontendUrl("http://127.0.0.1:5137/"));
    EXPECT_TRUE(IsInternalFrontendUrl("http://127.0.0.1:5137/wallet-panel?iro=1"));
    EXPECT_TRUE(IsInternalFrontendUrl("http://127.0.0.1:5137/brc100-auth?type=idle"));
    EXPECT_TRUE(IsInternalFrontendUrl("http://localhost:5137/menu"));
    EXPECT_TRUE(IsInternalFrontendUrl("hodos://settings"));  // historical arm
}

TEST(InternalFrontend, UserinfoIsNotInternal) {
    EXPECT_FALSE(IsInternalFrontendUrl("http://127.0.0.1:5137@evil.com/"));
    EXPECT_FALSE(IsInternalFrontendUrl("http://localhost:5137@evil.com/"));
}

TEST(InternalFrontend, UserinfoHttpsIsNotInternal) {
    EXPECT_FALSE(IsInternalFrontendUrl("https://127.0.0.1:5137@evil.com/"));
}

TEST(InternalFrontend, SubstringAndWrongPortAreNotInternal) {
    EXPECT_FALSE(IsInternalFrontendUrl("https://example.com/?x=127.0.0.1:5137"));
    EXPECT_FALSE(IsInternalFrontendUrl("http://127.0.0.1:51370/"));
    EXPECT_FALSE(IsInternalFrontendUrl("http://127.0.0.1:8000/"));
    EXPECT_FALSE(IsInternalFrontendUrl("data:text/html,a://127.0.0.1:5137/"));
    EXPECT_FALSE(IsInternalFrontendUrl("about:blank"));
}

// ---------------------------------------------------------------------------
// IsLoopbackUrl
// ---------------------------------------------------------------------------

TEST(Loopback, RealLoopbackUrlsMatch) {
    EXPECT_TRUE(IsLoopbackUrl("http://127.0.0.1:5137/"));
    EXPECT_TRUE(IsLoopbackUrl("http://127.0.0.1/"));
    EXPECT_TRUE(IsLoopbackUrl("https://localhost:8443/x"));
    EXPECT_TRUE(IsLoopbackUrl("http://[::1]:3000/"));
    EXPECT_TRUE(IsLoopbackUrl("http://[::1]/"));
}

TEST(Loopback, HostIsTerminated) {
    EXPECT_FALSE(IsLoopbackUrl("http://localhost.evil.com/"));
    EXPECT_FALSE(IsLoopbackUrl("http://127.0.0.1.evil.com/"));
    EXPECT_FALSE(IsLoopbackUrl("http://localhostevil.com/"));
}

TEST(Loopback, NonLoopbackAndOpaqueDoNotMatch) {
    EXPECT_FALSE(IsLoopbackUrl("https://example.com/?x=127.0.0.1"));
    EXPECT_FALSE(IsLoopbackUrl("about:blank"));
    EXPECT_FALSE(IsLoopbackUrl("data:text/html,a://127.0.0.1/"));
    EXPECT_FALSE(IsLoopbackUrl(""));
}

// The sentinel must never be mistaken for a trusted identity.
TEST(Sentinel, OpaqueOriginInvalidIsNotLoopbackOrInternal) {
    EXPECT_FALSE(IsLoopbackUrl("opaque-origin.invalid"));
    EXPECT_FALSE(IsInternalFrontendUrl("opaque-origin.invalid"));
    EXPECT_FALSE(AuthorityHasHost("opaque-origin.invalid", "localhost"));
    EXPECT_FALSE(AuthorityHasHost("opaque-origin.invalid", "127.0.0.1"));
}
