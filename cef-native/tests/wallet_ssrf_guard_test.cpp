// wallet_ssrf_guard_test.cpp — P0.5 E1 (the wallet_call SSRF)
//
// Covers hodos::IsWalletDispatchUrlSafe / IsValidWalletMethod in
// include/core/PortConfig.h. These two predicates are the whole fix: they run
// once, on the platform-neutral path, at the single shared dispatch choke
// (HttpRequestInterceptor.cpp :: dispatchWalletHttpByMethod), so the macOS
// libcurl arm can no longer be handed an "@evil.com" authority escape or a
// CRLF-bearing method.
//
// ⛔ NEGATIVE CONTROL (CLAUDE.md hard rule; HARNESS.md §6). Every assertion here
// must be seen to FAIL against the pre-fix code. There was NO validation pre-fix,
// so the control is to weaken each predicate to always-accept:
//
//   IsWalletDispatchUrlSafe := return true;   // pre-fix: url used verbatim
//   IsValidWalletMethod     := return true;   // pre-fix: method used verbatim
//
// Expected RED under those bodies:
//   UrlGuard.Rejects*   — every EXPECT_FALSE flips to a failure (the SSRF url,
//                         the userinfo pivot, the CRLF-in-path, the empty
//                         endpoint, and the foreign-scheme/host all "pass")
//   MethodGuard.Rejects* — every EXPECT_FALSE flips (CRLF method, spaces,
//                          lowercase, empty, over-long all "pass")
//
// The Accepts* suites are the other half of the control: they must stay GREEN
// under BOTH bodies, proving the guard does not over-reject the wallet's own
// first-party traffic (R-INTEXT: internal must keep working).
//
// A run in which the Rejects* pass without that always-accept control having been
// observed proves nothing.

#include <gtest/gtest.h>

#include "../include/core/PortConfig.h"

using hodos::IsValidWalletMethod;
using hodos::IsWalletDispatchUrlSafe;
using hodos::WalletBaseUrl;

// ---------------------------------------------------------------------------
// IsWalletDispatchUrlSafe — the URL half
//
// Built from WalletBaseUrl() (not a hardcoded port) so the test tracks the
// active HODOS_DEV port exactly as the production code does.
// ---------------------------------------------------------------------------

TEST(UrlGuard, AcceptsRealWalletEndpoints) {
    EXPECT_TRUE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "/wallet/status"));
    EXPECT_TRUE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "/wallet/balance"));
    EXPECT_TRUE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "/createAction"));
    EXPECT_TRUE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "/transaction/send"));
    // A query string is legitimate and printable-ASCII — must be allowed.
    EXPECT_TRUE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "/wallet/history?page=2"));
    // An '@' AFTER the leading '/' is in the path, not the authority — harmless.
    EXPECT_TRUE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "/paymail/a@b.example"));
}

// THE FINDING. endpoint "@evil.com/steal" ⇒ "http://127.0.0.1:<port>@evil.com/steal";
// curl takes the host up to the LAST '@', so the browser process fetches evil.com.
TEST(UrlGuard, RejectsUserinfoAuthorityEscape) {
    EXPECT_FALSE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "@evil.com/steal"));
    EXPECT_FALSE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "@example.com/"));
    // A second '@' does not save it — curl still keys on the last one.
    EXPECT_FALSE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "@a@evil.com/"));
}

// endpoint that injects a whole new scheme/host rather than a path.
TEST(UrlGuard, RejectsForeignSchemeOrHost) {
    EXPECT_FALSE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "http://evil.com"));
    EXPECT_FALSE(IsWalletDispatchUrlSafe("http://evil.com/wallet/export"));
    EXPECT_FALSE(IsWalletDispatchUrlSafe("http://127.0.0.1:9999/x"));  // wrong port
    EXPECT_FALSE(IsWalletDispatchUrlSafe("file:///etc/passwd"));
}

// Endpoint without a leading '/' — the char after the base is not '/'.
TEST(UrlGuard, RejectsMissingLeadingSlash) {
    EXPECT_FALSE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "wallet/status"));
    // Empty endpoint ⇒ url == base, which has no trailing slash ⇒ fails closed.
    EXPECT_FALSE(IsWalletDispatchUrlSafe(WalletBaseUrl()));
    EXPECT_FALSE(IsWalletDispatchUrlSafe(""));
}

// CRLF (or any C0 control / DEL) anywhere ⇒ request-splitting / header injection.
TEST(UrlGuard, RejectsControlCharacters) {
    EXPECT_FALSE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "/wallet/status\r\nHost: evil"));
    EXPECT_FALSE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "/wallet/\nstatus"));
    EXPECT_FALSE(IsWalletDispatchUrlSafe(WalletBaseUrl() + std::string("/wallet/\0x", 10)));
    EXPECT_FALSE(IsWalletDispatchUrlSafe(WalletBaseUrl() + "/wallet/\x7f"));
}

// ---------------------------------------------------------------------------
// IsValidWalletMethod — the method half
// ---------------------------------------------------------------------------

TEST(MethodGuard, AcceptsRealVerbs) {
    EXPECT_TRUE(IsValidWalletMethod("GET"));
    EXPECT_TRUE(IsValidWalletMethod("POST"));
    EXPECT_TRUE(IsValidWalletMethod("PUT"));
    EXPECT_TRUE(IsValidWalletMethod("DELETE"));
    EXPECT_TRUE(IsValidWalletMethod("PATCH"));
    EXPECT_TRUE(IsValidWalletMethod("HEAD"));
    EXPECT_TRUE(IsValidWalletMethod("OPTIONS"));
}

TEST(MethodGuard, RejectsInjectionAndJunk) {
    EXPECT_FALSE(IsValidWalletMethod("GET\r\nHost: evil.com"));  // CRLF into CUSTOMREQUEST
    EXPECT_FALSE(IsValidWalletMethod("GET HTTP/1.1"));           // embedded space
    EXPECT_FALSE(IsValidWalletMethod("get"));                    // lowercase
    EXPECT_FALSE(IsValidWalletMethod("POST3"));                  // digit
    EXPECT_FALSE(IsValidWalletMethod(""));                       // empty
    EXPECT_FALSE(IsValidWalletMethod("ABCDEFGHIJK"));            // over-long
    EXPECT_FALSE(IsValidWalletMethod("GET\t"));                  // tab
}
