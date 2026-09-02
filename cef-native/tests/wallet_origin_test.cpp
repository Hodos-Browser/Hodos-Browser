// wallet_origin_test.cpp — beta.3 Phase 5 (W0/W1)
//
// Covers hodos::IsLoopbackHost / SplitAuthority / IsWalletOrigin /
// IsOurWalletOrigin / IsMessageboxOrigin / IsWellKnownAuthRequest /
// RepointLoopbackToWallet in include/core/PortConfig.h.
//
// ⭐ WHY THIS FILE EXISTS, AND WHY IT ASSERTS THE OLD BEHAVIOUR TOO
//
// The C++ interception layer is what marks traffic as UNTRUSTED: Rust's
// domain_trust_mw reads X-Requesting-Domain, and a MISSING header means
// "internal, fully trusted". So a matcher that fails to match does not leave
// traffic ungated — it leaves it TRUSTED, and every narrowing here is a
// privilege change.
//
// That makes a one-sided test useless. Each behavioural case below is written as
// a PAIR: the new predicate's verdict, and the LEGACY predicate's verdict on the
// same input. The legacy assertions are the negative control — they fail if
// someone deletes the old gate before beta.4's W8, and more importantly they are
// the proof that the "fixed" cases were genuinely broken.
//
// ⛔ No libcef here on purpose: cef-native/tests/CMakeLists.txt does not link it,
// which is exactly why this phase built the predicate on PortConfig.h's own
// string parsing instead of CefParseURL.

#include <gtest/gtest.h>

#include <string>

#include "core/PortConfig.h"

using hodos::IsLoopbackHost;
using hodos::IsMessageboxOrigin;
using hodos::IsOurWalletOrigin;
using hodos::IsWalletOrigin;
using hodos::IsWellKnownAuthRequest;
using hodos::LegacyWalletGateMatch;
using hodos::RepointLoopbackToWallet;
using hodos::SplitAuthority;

namespace {
// Never hardcode the port — it is 31301 release / 31401 under HODOS_DEV=1, and
// the test binary must pass either way.
std::string P() { return hodos::WalletPortStr(); }
std::string WalletUrlWithPath(const std::string& path) {
    return "http://127.0.0.1:" + P() + path;
}
}  // namespace

// ---------------------------------------------------------------------------
// SplitAuthority
// ---------------------------------------------------------------------------

TEST(SplitAuthority, HostAndPort) {
    std::string h, p;
    SplitAuthority("127.0.0.1:3321", h, p);
    EXPECT_EQ(h, "127.0.0.1");
    EXPECT_EQ(p, "3321");
}

TEST(SplitAuthority, HostWithoutPort) {
    std::string h, p;
    SplitAuthority("localhost", h, p);
    EXPECT_EQ(h, "localhost");
    EXPECT_EQ(p, "");
}

TEST(SplitAuthority, BracketedIPv6KeepsItsBrackets) {
    std::string h, p;
    SplitAuthority("[::1]:2121", h, p);
    EXPECT_EQ(h, "[::1]");
    EXPECT_EQ(p, "2121");

    SplitAuthority("[::1]", h, p);
    EXPECT_EQ(h, "[::1]");
    EXPECT_EQ(p, "");
}

TEST(SplitAuthority, MalformedBracketFailsClosed) {
    std::string h, p;
    SplitAuthority("[::1:3321", h, p);
    EXPECT_EQ(h, "");  // no host -> every caller refuses
}

// ---------------------------------------------------------------------------
// IsLoopbackHost — deliberately BROAD. See rule 1 in PortConfig.h.
// ---------------------------------------------------------------------------

TEST(LoopbackHost, TheOrdinarySpellings) {
    EXPECT_TRUE(IsLoopbackHost("localhost"));
    EXPECT_TRUE(IsLoopbackHost("127.0.0.1"));
    EXPECT_TRUE(IsLoopbackHost("[::1]"));
}

TEST(LoopbackHost, EveryAddressChromiumRoutesToThisMachine) {
    // RFC 6761 — the whole *.localhost tree.
    EXPECT_TRUE(IsLoopbackHost("x.localhost"));
    EXPECT_TRUE(IsLoopbackHost("api.dev.localhost"));
    // 127.0.0.0/8, including the short forms GURL canonicalises.
    EXPECT_TRUE(IsLoopbackHost("127.0.0.2"));
    EXPECT_TRUE(IsLoopbackHost("127.1"));
    EXPECT_TRUE(IsLoopbackHost("127.255.255.254"));
}

TEST(LoopbackHost, CaseIsNotASmugglingChannel) {
    EXPECT_TRUE(IsLoopbackHost("LOCALHOST"));
    EXPECT_TRUE(IsLoopbackHost("X.LocalHost"));
}

TEST(LoopbackHost, SuffixExtensionIsRefused) {
    // The classic attacker-registrable family. These are ordinary internet
    // hosts that merely start or end with loopback-looking text.
    EXPECT_FALSE(IsLoopbackHost("127.0.0.1.evil.com"));
    EXPECT_FALSE(IsLoopbackHost("localhost.evil.com"));
    EXPECT_FALSE(IsLoopbackHost("localhostevil.com"));
    EXPECT_FALSE(IsLoopbackHost("127.evil.com"));
    EXPECT_FALSE(IsLoopbackHost("notlocalhost"));
    EXPECT_FALSE(IsLoopbackHost("example.com"));
    EXPECT_FALSE(IsLoopbackHost(""));
}

TEST(LoopbackHost, OtherPrivateRangesAreNotLoopback) {
    EXPECT_FALSE(IsLoopbackHost("192.168.1.1"));
    EXPECT_FALSE(IsLoopbackHost("10.0.0.1"));
    EXPECT_FALSE(IsLoopbackHost("128.0.0.1"));  // one off 127
}

// ---------------------------------------------------------------------------
// 🚨 THE PRIVILEGE-ESCALATION CASE (ticket §5.1)
//
// This is the single most important test in the file. A naive "clean up the
// substring match into a strict equality test" refactor breaks exactly this,
// and the symptom is not a 404 — it is the wallet treating a page's request as
// its own first-party call.
// ---------------------------------------------------------------------------

TEST(PrivilegeEscalation, StarDotLocalhostMustStillBeInterceptedOnOurPort) {
    const std::string url = "http://x.localhost:" + P() + "/createAction";

    // Chromium resolves x.localhost to loopback, so this request DOES reach the
    // wallet. If we stop matching it, it arrives with no X-Requesting-Domain and
    // Rust reads it as internal + fully trusted.
    EXPECT_TRUE(IsWalletOrigin(url));
    EXPECT_TRUE(IsOurWalletOrigin(url));

    // Control: the old gate matched it too (by accident — "x.localhost:PORT"
    // contains "localhost:PORT"). Parity here is the whole point; a narrowing
    // would be a regression, not a cleanup.
    EXPECT_TRUE(LegacyWalletGateMatch(url));
}

TEST(PrivilegeEscalation, StarDotLocalhostOnACompatBridgePortToo) {
    EXPECT_TRUE(IsWalletOrigin("http://x.localhost:3321/getVersion"));
    EXPECT_TRUE(IsWalletOrigin("https://x.localhost:2121/getVersion"));
}

// ---------------------------------------------------------------------------
// IsWalletOrigin — the gate
// ---------------------------------------------------------------------------

TEST(WalletOrigin, OurOwnPortInEveryHostForm) {
    EXPECT_TRUE(IsWalletOrigin("http://127.0.0.1:" + P() + "/createAction"));
    EXPECT_TRUE(IsWalletOrigin("http://localhost:" + P() + "/createAction"));
    EXPECT_TRUE(IsWalletOrigin("http://[::1]:" + P() + "/createAction"));
}

TEST(WalletOrigin, TheCompatBridgePortsInBothHostFormsAndBothSchemes) {
    // These four are the App Lab's entire transport. MEASURED reaching our
    // wallet 2026-09-02 (phase-5 MEASUREMENTS.md M1).
    EXPECT_TRUE(IsWalletOrigin("http://127.0.0.1:3321/getVersion"));
    EXPECT_TRUE(IsWalletOrigin("http://localhost:3321/getVersion"));
    EXPECT_TRUE(IsWalletOrigin("https://127.0.0.1:2121/getVersion"));
    EXPECT_TRUE(IsWalletOrigin("https://localhost:2121/getVersion"));
}

TEST(WalletOrigin, Port8080IsGoneAndTheOldGateProvesItUsedToMatch) {
    const std::string dev_server = "http://127.0.0.1:8080/health";

    // 👤 Owner decision 2026-09-02. 8080 is in no BRC and no wallet; it is the
    // default port of Spring Boot / Tomcat / http-server, and matching it meant
    // a developer's own /health, /encrypt and /getVersion were answered by the
    // wallet instead of their server.
    EXPECT_FALSE(IsWalletOrigin(dev_server));

    // 🔴 The control: it DID match before this phase.
    EXPECT_TRUE(LegacyWalletGateMatch(dev_server));
}

TEST(WalletOrigin, AnOrdinaryWebsiteIsNotWalletTraffic) {
    EXPECT_FALSE(IsWalletOrigin("https://example.com/getVersion"));
    EXPECT_FALSE(IsWalletOrigin("https://127.0.0.1.evil.com:3321/getVersion"));
    EXPECT_FALSE(IsWalletOrigin("https://localhost.evil.com:3321/getVersion"));
}

TEST(WalletOrigin, AnUnlistedLoopbackPortIsNotWalletTraffic) {
    // NC-1 from MEASUREMENTS.md M1, as a unit test: one digit off 3321.
    EXPECT_FALSE(IsWalletOrigin("http://127.0.0.1:3322/getNetwork"));
    EXPECT_FALSE(IsWalletOrigin("http://127.0.0.1:3000/getVersion"));
    EXPECT_FALSE(IsWalletOrigin("http://127.0.0.1/getVersion"));  // no port
}

TEST(WalletOrigin, UserinfoIsNotTheHost) {
    // http://127.0.0.1:PORT@evil.com/ is a page on evil.com. Chromium does not
    // strip credentials from a committed URL, so this must not prefix-match.
    EXPECT_FALSE(IsWalletOrigin("http://127.0.0.1:" + P() + "@evil.com/createAction"));
    EXPECT_FALSE(IsWalletOrigin("http://localhost:3321@evil.com/getVersion"));
}

TEST(WalletOrigin, NonHttpSchemesYieldNothing) {
    EXPECT_FALSE(IsWalletOrigin("data:text/html,a://127.0.0.1:3321/"));
    EXPECT_FALSE(IsWalletOrigin("about:blank"));
    EXPECT_FALSE(IsWalletOrigin("file:///c:/x"));
    EXPECT_FALSE(IsWalletOrigin(""));
}

// ---------------------------------------------------------------------------
// 🚨 ROW P5-A4 — the defect reproduced live on 2026-09-02, frozen as a test.
//
// MEASUREMENTS.md M2: from a real page,
//   fetch('https://example.com/getNetwork?x=127.0.0.1:3321')
// was intercepted, had its own query string rewritten, was silently downgraded
// https->http, and was answered by our wallet. example.com was never contacted.
// ---------------------------------------------------------------------------

TEST(A4_QueryStringCannotForgeWalletTraffic, TheMeasuredExploitUrl) {
    const std::string measured = "https://example.com/getNetwork?x=127.0.0.1:3321";

    EXPECT_FALSE(IsWalletOrigin(measured));
    EXPECT_FALSE(IsOurWalletOrigin(measured));

    // 🔴 RED, and it was OBSERVED in a running browser, not merely predicted.
    EXPECT_TRUE(LegacyWalletGateMatch(measured));
}

TEST(A4_QueryStringCannotForgeWalletTraffic, TheWholeFamily) {
    const std::string cases[] = {
        "https://example.com/?x=127.0.0.1:" + P(),
        "https://example.com/health?x=127.0.0.1:3321",
        "https://evil.example/a?b=localhost:2121",
        "https://evil.example/#127.0.0.1:3321",
        "https://evil.example/path/127.0.0.1:3321/more",
    };
    for (const auto& u : cases) {
        EXPECT_FALSE(IsWalletOrigin(u)) << u;
        EXPECT_TRUE(LegacyWalletGateMatch(u)) << "legacy control failed for " << u;
    }
}

// ---------------------------------------------------------------------------
// RepointLoopbackToWallet — row P5-A5
// ---------------------------------------------------------------------------

TEST(Repoint, ForeignBridgePortIsRePointedAtOurWallet) {
    EXPECT_EQ(RepointLoopbackToWallet("http://127.0.0.1:3321/getVersion"),
              "http://127.0.0.1:" + P() + "/getVersion");
    EXPECT_EQ(RepointLoopbackToWallet("https://localhost:2121/getVersion"),
              "https://localhost:" + P() + "/getVersion");
    // Any loopback port, which is what the BRC-104 arm relies on.
    EXPECT_EQ(RepointLoopbackToWallet("http://localhost:9999/.well-known/auth"),
              "http://localhost:" + P() + "/.well-known/auth");
}

TEST(Repoint, QueryAndPathAreLeftAlone) {
    // 🚨 The measured defect: the old lambda rewrote the FIRST match anywhere in
    // the string, so a page's own query became the wallet's address.
    EXPECT_EQ(RepointLoopbackToWallet("https://example.com/getNetwork?x=127.0.0.1:3321"),
              "https://example.com/getNetwork?x=127.0.0.1:3321");
    // Query on a genuine wallet URL survives verbatim; only the authority moves.
    EXPECT_EQ(RepointLoopbackToWallet("http://127.0.0.1:3321/x?y=127.0.0.1:8080"),
              "http://127.0.0.1:" + P() + "/x?y=127.0.0.1:8080");
}

TEST(Repoint, NothingToDoIsIdentity) {
    const std::string ours = WalletUrlWithPath("/createAction");
    EXPECT_EQ(RepointLoopbackToWallet(ours), ours);            // already ours
    EXPECT_EQ(RepointLoopbackToWallet("https://example.com/"), "https://example.com/");
    EXPECT_EQ(RepointLoopbackToWallet("http://127.0.0.1/x"), "http://127.0.0.1/x");  // no port
    EXPECT_EQ(RepointLoopbackToWallet("about:blank"), "about:blank");
}

TEST(Repoint, MalformedPortIsNotRewritten) {
    EXPECT_EQ(RepointLoopbackToWallet("http://127.0.0.1:80a1/x"),
              "http://127.0.0.1:80a1/x");
    EXPECT_EQ(RepointLoopbackToWallet("http://127.0.0.1:1234567/x"),
              "http://127.0.0.1:1234567/x");
}

TEST(Repoint, UserinfoIsNotRewrittenIntoTheWallet) {
    // The host here is evil.com; nothing about it is loopback.
    const std::string u = "http://127.0.0.1:3321@evil.com/x";
    EXPECT_EQ(RepointLoopbackToWallet(u), u);
}

// ---------------------------------------------------------------------------
// MessageBox + BRC-104
// ---------------------------------------------------------------------------

TEST(Messagebox, TheRealRelayStillMatches) {
    EXPECT_TRUE(IsMessageboxOrigin("https://messagebox.babbage.systems/sendMessage"));
    EXPECT_TRUE(IsMessageboxOrigin("https://messagebox.babbage.systems/socket.io/?x=1"));
}

TEST(Messagebox, TextElsewhereInTheUrlDoesNot) {
    const std::string forged = "https://evil.example/?x=messagebox.babbage.systems";
    EXPECT_FALSE(IsMessageboxOrigin(forged));
    EXPECT_TRUE(LegacyWalletGateMatch(forged));  // 🔴 control
    EXPECT_FALSE(IsMessageboxOrigin("https://messagebox.babbage.systems.evil.com/x"));
}

TEST(WellKnownAuth, TheRealPathStillMatches) {
    EXPECT_TRUE(IsWellKnownAuthRequest("https://someapp.example/.well-known/auth"));
    EXPECT_TRUE(IsWellKnownAuthRequest("http://127.0.0.1:3321/.well-known/auth"));
}

TEST(WellKnownAuth, TheQueryStringDoesNot) {
    const std::string forged = "https://evil.example/x?next=/.well-known/auth";
    EXPECT_FALSE(IsWellKnownAuthRequest(forged));
    EXPECT_TRUE(LegacyWalletGateMatch(forged));  // 🔴 control
}

// ---------------------------------------------------------------------------
// IsOurWalletOrigin — used by the /health arm, the scheme downgrade, socket.io
// and the trusted-overlay bypass.
// ---------------------------------------------------------------------------

TEST(OurWalletOrigin, OnlyOurPort) {
    EXPECT_TRUE(IsOurWalletOrigin(WalletUrlWithPath("/health")));
    EXPECT_FALSE(IsOurWalletOrigin("http://127.0.0.1:3321/health"));  // pre-repoint
    EXPECT_FALSE(IsOurWalletOrigin("https://example.com/health"));
}

TEST(OurWalletOrigin, AfterRepointTheCompatPortQualifies) {
    // This is the real sequence inside the interceptor, and it is why the
    // /health arm reaches the App Lab's probe at all.
    const std::string probe = "http://127.0.0.1:3321/health";
    EXPECT_FALSE(IsOurWalletOrigin(probe));
    EXPECT_TRUE(IsOurWalletOrigin(RepointLoopbackToWallet(probe)));
}

TEST(OurWalletOrigin, TheHealthArmsScopingNowActuallyHolds) {
    // 🚨 The comment on that arm claimed "SCOPED to our own host:port ON
    // PURPOSE" while IsWalletHostPort was a whole-URL find(). It asserted a
    // protection the code did not provide. Both halves are parsed now.
    const std::string forged = "https://any.site/health?x=127.0.0.1:" + P();
    EXPECT_FALSE(IsOurWalletOrigin(forged));
    EXPECT_EQ(RepointLoopbackToWallet(forged), forged);  // and cannot be manufactured
}
