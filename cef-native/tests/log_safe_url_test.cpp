// P2-A5 — URL redaction for log lines.
//
// The cases below are drawn from the SHIPPED production log, not invented: these are the
// exact URL shapes that were being persisted in plaintext.

#include "../include/core/LogSafeUrl.h"

#include <gtest/gtest.h>

using hodos::LogSafeUrl;

// ── The query string is where the damage is ──────────────────────────────────────────

TEST(LogSafeUrl, DropsQueryStringCarryingTokensAndSearchTerms) {
    // Real line from the production log (a YouTube timedtext request).
    EXPECT_EQ(LogSafeUrl("https://www.youtube.com/api/timedtext?v=oDyyp0NGmcI&ei=y6Zwasf7A6"
                         "&caps=asr&opi=112496729&expire=1785792827&signature=deadbeef"),
              "https://www.youtube.com");
}

TEST(LogSafeUrl, DropsThePathThatRevealsWhatTheUserWasDoing) {
    EXPECT_EQ(LogSafeUrl("https://x.com/i/api/2/badge_count/badge_count.json"
                         "?supports_ntab_urt=1&include_xchat_count=1"),
              "https://x.com");
    EXPECT_EQ(LogSafeUrl("https://video.twimg.com/amplify_video/2092386260014825472/vid/"
                         "avc1/36000/38700/1080x1920/tEw_pwMt4qW9WX2I.m4s"),
              "https://video.twimg.com");
    EXPECT_EQ(LogSafeUrl("https://www.linkedin.com/feed/"), "https://www.linkedin.com");
}

TEST(LogSafeUrl, DropsTheFragment) {
    EXPECT_EQ(LogSafeUrl("https://example.com/page#section-with-state"), "https://example.com");
}

// ⛔ A URL can carry credentials in the authority. They must never reach a log.
TEST(LogSafeUrl, DropsCredentialsFromTheAuthority) {
    EXPECT_EQ(LogSafeUrl("https://alice:hunter2@example.com/private"), "https://example.com");
}

// ── Ports are kept: they are diagnostically load-bearing here ────────────────────────

TEST(LogSafeUrl, KeepsThePortBecauseWalletRoutingDependsOnIt) {
    EXPECT_EQ(LogSafeUrl("https://example.com:8443/some/path"), "https://example.com:8443");
}

// ── Loopback keeps its path: it is our own UI, not the user's browsing ───────────────

TEST(LogSafeUrl, KeepsLoopbackPathSoOverlayDiagnosticsStillWork) {
    EXPECT_EQ(LogSafeUrl("http://127.0.0.1:5137/wallet-panel?iro=50"),
              "http://127.0.0.1:5137/wallet-panel");
    EXPECT_EQ(LogSafeUrl("http://localhost:5137/settings-page/privacy"),
              "http://localhost:5137/settings-page/privacy");
    EXPECT_EQ(LogSafeUrl("http://127.0.0.1:31401/wallet/balance"),
              "http://127.0.0.1:31401/wallet/balance");
}

TEST(LogSafeUrl, LoopbackStillDropsItsQueryString) {
    // The wallet bridge puts request ids and endpoints in query strings.
    EXPECT_EQ(LogSafeUrl("http://127.0.0.1:5137/brc100-auth?type=payment&token=secret"),
              "http://127.0.0.1:5137/brc100-auth");
}

TEST(LogSafeUrl, IPv6LoopbackIsRecognised) {
    EXPECT_EQ(LogSafeUrl("http://[::1]:5137/menu"), "http://[::1]:5137/menu");
}

// ── Opaque schemes collapse entirely ─────────────────────────────────────────────────

// ⛔ A data: URL can inline a whole document. Logging any of it is unbounded disclosure.
TEST(LogSafeUrl, CollapsesOpaqueSchemesToTheSchemeAlone) {
    EXPECT_EQ(LogSafeUrl("data:text/html;base64,PGh0bWw+PGJvZHk+c2VjcmV0"), "data:(redacted)");
    EXPECT_EQ(LogSafeUrl("blob:https://example.com/6d1f-4c3a"), "blob:(redacted)");
    EXPECT_EQ(LogSafeUrl("javascript:alert(document.cookie)"), "javascript:(redacted)");
    EXPECT_EQ(LogSafeUrl("about:blank"), "about:(redacted)");
}

TEST(LogSafeUrl, HandlesDegenerateInputWithoutThrowing) {
    EXPECT_EQ(LogSafeUrl(""), "");
    EXPECT_EQ(LogSafeUrl("not-a-url"), "(opaque)");
    EXPECT_EQ(LogSafeUrl("https://"), "https://");
    EXPECT_EQ(LogSafeUrl("https://host-with-no-path"), "https://host-with-no-path");
}

// ── The property that matters, stated directly ───────────────────────────────────────

// 🔴 This is the assertion that would have caught the shipped defect: no output may retain
// a query string. If redaction is ever loosened, this fails.
TEST(LogSafeUrl, NoRedactedOutputEverRetainsAQueryString) {
    const char* kRealShapes[] = {
        "https://x.com/i/api/2/badge_count/badge_count.json?supports_ntab_urt=1",
        "https://www.youtube.com/api/timedtext?v=abc&signature=xyz",
        "http://127.0.0.1:5137/brc100-auth?type=payment&token=secret",
        "https://knov-prod.onrender.com/api/v1/bsv/tdp?_t=1787759053719",
        "https://abs.twimg.com/favicons/twitter-pip.3.ico?v=2",
    };
    for (const char* u : kRealShapes) {
        const std::string out = LogSafeUrl(u);
        EXPECT_EQ(out.find('?'), std::string::npos)
            << "a query string survived redaction for: " << u << " -> " << out;
        EXPECT_EQ(out.find("token"), std::string::npos) << u << " -> " << out;
        EXPECT_EQ(out.find("signature"), std::string::npos) << u << " -> " << out;
    }
}
