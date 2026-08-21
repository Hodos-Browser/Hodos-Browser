// PortConfig.h — single source of truth for the wallet/adblock backend ports.
//
// Dev builds (env HODOS_DEV=1) talk to the wallet on 31401 and adblock on 31402
// so the dev browser and the INSTALLED browser (which use 31301/31302) can run
// at the SAME TIME without fighting over the ports. A release build never sets
// HODOS_DEV, so it always uses 31301/31302. This mirrors the Rust side's
// wallet_port()/adblock_port() gate (rust-wallet/src/main.rs,
// adblock-engine/src/main.rs) — keep them in lockstep.
//
// HARD RELEASE-SAFETY RULE: the dev port must apply ONLY when HODOS_DEV=1. Never
// hardcode 31401/31402 anywhere; always route through these helpers so the dev
// port can never leak into a release build.
//
// Usage:
//   - URL string literals:  hodos::WalletUrl("/wallet/status")  // http://127.0.0.1:<port>/wallet/status
//   - WinHttpConnect / integer port args:  hodos::WalletPort()
//   - Port-recognition gates (find("localhost:31301")):  hodos::IsWalletHostPort(url)
//     (checks BOTH "localhost:<port>" AND "127.0.0.1:<port>" — the codebase
//      uses both host forms and they must move in lockstep).

#pragma once

#include <cstdlib>
#include <string>

namespace hodos {

// Computed once (HODOS_DEV cannot change mid-run) so this is cheap on hot paths
// such as the per-request interceptor.
inline bool IsDevEnv() {
    static const bool dev = []() {
        const char* v = std::getenv("HODOS_DEV");
        return v != nullptr && std::string(v) == "1";
    }();
    return dev;
}

inline int  WalletPort()      { return IsDevEnv() ? 31401 : 31301; }
inline int  AdblockPort()     { return IsDevEnv() ? 31402 : 31302; }
inline std::string WalletPortStr()  { return std::to_string(WalletPort()); }
inline std::string AdblockPortStr() { return std::to_string(AdblockPort()); }

// Base URLs (127.0.0.1 host form — the canonical one for outbound C++ calls).
inline std::string WalletBaseUrl()  { return "http://127.0.0.1:" + WalletPortStr(); }
inline std::string AdblockBaseUrl() { return "http://127.0.0.1:" + AdblockPortStr(); }

// Convenience: full wallet URL for a leading-slash path.
inline std::string WalletUrl(const std::string& path)  { return WalletBaseUrl()  + path; }
inline std::string AdblockUrl(const std::string& path) { return AdblockBaseUrl() + path; }

// True if `url` targets the local wallet on the active port, in EITHER host form.
// Replaces literal find("localhost:31301") / find("127.0.0.1:31301") gates.
// ---------------------------------------------------------------------------
// P0.5 — the FRONTEND origin. Deliberately here beside IsWalletHostPort: same
// shape of predicate, same "one spelling only" rule.
//
// 5137 is NOT a backend port and does NOT take a dev offset. It is a URL
// NAMESPACE: in production nothing listens on it — GetResourceRequestHandler
// intercepts these URLs and LocalFileResourceRequestHandler serves {app}\frontend\
// from disk. Same string in dev and release, which is why it is a plain literal
// rather than a WalletPort()-style helper.
//
// TRUST BOUNDARY - this MUST be a prefix match. An unanchored substring search
// for the host:port admits any URL that merely CONTAINS it, anywhere - including
// in a query string the page author controls. (Spelled indirectly on purpose: the
// preflight G2 gate greps for that pattern, and a comment quoting it verbatim
// counts as a violation.) Measured 2026-08-19 against a live dev build: a page at
// https://example.com/?x=127.0.0.1:5137 received hodosBrowser.identity,
// .navigation and .history, while the SAME page without the query string
// received none of them.
//
// Mirrors the three prefixes simple_handler.cpp has always used, including
// hodos:// — NavigationHandler rewrites that scheme to http://127.0.0.1:5137/,
// so a frame can legitimately carry it.
// ---------------------------------------------------------------------------
// P0.5 C1 — THE origin derivation. One spelling, used everywhere.
//
// ⛔ Prefix-anchoring alone is NECESSARY BUT NOT SUFFICIENT, and believing
// otherwise is what shipped the C1 bypass. Two separate defects it must stop:
//
//   1. UNANCHORED SCHEME. The old derivation in simple_handler.cpp searched the
//      WHOLE frame URL for "://" with no scheme check, so
//      data:text/html,a://127.0.0.1:5137/<script>...  parsed to the authority
//      127.0.0.1:5137 and was accepted as wallet-internal. The frame is
//      attacker-authored and same-origin scriptable, and it carries cefMessage.
//      MEASURED as a live unprompted-spend path, 2026-08-19.
//
//   2. USERINFO. http://127.0.0.1:5137@evil.com/ is a page on evil.com, but it
//      PREFIX-MATCHES "http://127.0.0.1:5137". Chromium does not strip
//      credentials from a committed document URL, and CefFrameImpl::GetURL
//      returns that spec verbatim.
//
// So: only a REAL scheme yields an authority, and the authority is taken up to
// the first '/', '?' or '#' with everything through the LAST '@' discarded.
// Anything else returns EMPTY — which is what lets the caller's ancestor cascade
// and its opaque-origin sentinel actually run instead of being short-circuited
// by an attacker-supplied string.
//
// ⛔ hodos:// deliberately yields NO origin. NavigationHandler rewrites that
// scheme to http://127.0.0.1:5137/ before navigation and nothing registers it
// (no AddCustomScheme / OnRegisterCustomSchemes anywhere), so no frame can carry
// it — and if one ever could, "hodos://evil.com/" must not be privileged. Fails
// closed here while IsInternalFrontendUrl below keeps its historical prefix arm.
inline std::string OriginFromUrl(const std::string& url) {
    size_t authStart = std::string::npos;
    if (url.rfind("http://", 0) == 0)        authStart = 7;
    else if (url.rfind("https://", 0) == 0)  authStart = 8;
    if (authStart == std::string::npos) return std::string();

    size_t authEnd = url.size();
    for (size_t i = authStart; i < url.size(); ++i) {
        const char c = url[i];
        if (c == '/' || c == '?' || c == '#') { authEnd = i; break; }
    }
    std::string authority = url.substr(authStart, authEnd - authStart);

    // Userinfo: everything through the LAST '@' is credentials, not the host.
    const size_t at = authority.rfind('@');
    if (at != std::string::npos) authority = authority.substr(at + 1);

    return authority;  // may be empty ("http:///x") — caller must fail closed
}

// Host-terminated match: `host` exactly, or `host` followed by ':' + port.
// Rejects the suffix-extension family — "127.0.0.1.evil.com",
// "localhost.evil.com", "localhostevil.com". Same rule as
// HttpRequestInterceptor.cpp :: IsInternalOrigin's matchesHostOrHostColon; kept
// spelled once here so the two cannot drift.
inline bool AuthorityHasHost(const std::string& authority, const std::string& host) {
    if (authority.size() < host.size()) return false;
    if (authority.compare(0, host.size(), host) != 0) return false;
    return authority.size() == host.size() || authority[host.size()] == ':';
}

inline bool IsInternalFrontendUrl(const std::string& url) {
    // Historical arm, behaviour unchanged: see the hodos:// note above.
    if (url.rfind("hodos://", 0) == 0) return true;
    const std::string authority = OriginFromUrl(url);
    return authority == "127.0.0.1:5137" || authority == "localhost:5137";
}

// P0.5 — "is this URL served by something on THIS machine's loopback?"
//
// Companion to IsInternalFrontendUrl. The render process excludes loopback pages
// from the dApp provider shim on purpose (a local dev server is not a dApp; see the
// gating cascade in simple_render_process_handler.cpp). That exclusion was written
// as an unanchored search of the whole URL for "127.0.0.1"/"localhost", so
// https://example.com/?x=127.0.0.1:5137 counted as loopback. Anchored on the
// scheme+host prefix, it cannot be spoofed from a query string or path.
// P0.5 C1 — now host-TERMINATED, not merely prefixed. The prefix list this
// replaced matched "http://localhost.evil.com/" as loopback, because it never
// checked the character after the host.
inline bool IsLoopbackUrl(const std::string& url) {
    const std::string authority = OriginFromUrl(url);
    if (authority.empty()) return false;
    return AuthorityHasHost(authority, "127.0.0.1")
        || AuthorityHasHost(authority, "localhost")
        || AuthorityHasHost(authority, "[::1]");
}

inline bool IsWalletHostPort(const std::string& url) {
    const std::string p = WalletPortStr();
    return url.find("localhost:" + p) != std::string::npos
        || url.find("127.0.0.1:" + p) != std::string::npos;
}

// ⛔ MATCH THE PATH THE SERVER ROUTES ON, NOT THE RAW REQUEST TARGET.
// (P0.5 panel #3 — `/%70rocessAction`.)
//
// Every endpoint predicate on the C++ side exists to predict which Rust handler
// actix will run. actix-router percent-DECODES the path before matching, while
// our matchers read the RAW target — so `POST /%70rocessAction` was routed to
// `process_action` while `IsPaymentEndpoint` returned false. C++ never stamped
// the X-Payment-* headers, the spend was priced blind, there was no gold pill,
// and the per-session dollar cap never advanced. `isWalletEndpoint` missed it by
// the same character, so the request was not intercepted at all.
//
// This is panel #2's finding 1.3 in a third location. `main.rs` closed it by
// running its predicate over BOTH the raw and the decoded path. The C++ side is
// closed HERE instead — one normalizer, called INSIDE the predicates, so a
// caller cannot forget it. Adding one more exact string to a list already known
// to be the wrong shape is how this defect was created; do not do that again.
//
// TWO steps, and the ORDER is load-bearing:
//
//   1. Cut the query and fragment FIRST. `endpoint_` is the raw target INCLUDING
//      the query string, so decoding first would let `/foo?x=%2FcreateAction`
//      decode into `/foo?x=/createAction` and match — inventing a payment
//      endpoint out of page-controlled text. Cutting first puts the query beyond
//      the reach of every matcher, which also retires the pre-existing
//      false-positive where `https://site/?r=/createAction` was intercepted and
//      forwarded to the wallet.
//
//   2. Decode EXACTLY ONCE, because actix-router decodes exactly once.
//      `%2570rocessAction` decodes to `%70rocessAction`, which actix does NOT
//      route to `process_action` (404). Decoding twice would price a call the
//      wallet will never run — the same desync in the other direction.
//
// Invalid escapes are left verbatim (`%zz` stays `%zz`) and a trailing `%` or
// `%A` cannot run off the end, matching what a lenient decoder yields.
//
// Returns the path only. Accepts either an absolute URL (what `isWalletEndpoint`
// is handed) or a bare origin-relative target (what `endpoint_` holds).
inline std::string RequestPathForMatching(const std::string& target) {
    // Step 1 — cut the query and fragment FIRST, over the WHOLE target. This has
    // to precede scheme detection: `find("://")` over the raw target would
    // otherwise match a `://` a page put INSIDE the query
    // (`/createAction?z=a://b`), read the query as an authority, find no path
    // slash after it, and return "" — so `IsPaymentEndpoint` said "not a payment"
    // while actix (which drops the query) still routed `/createAction`. That
    // desync stripped the gold pill and the cap metering. (P0.5 panel re-run —
    // the code did NOT match its own "cut query first" comment.)
    size_t end = target.size();
    for (size_t i = 0; i < target.size(); ++i) {
        const char c = target[i];
        if (c == '?' || c == '#') { end = i; break; }
    }

    // Step 2 — find the scheme WITHIN the path portion only. `schemeEnd < end`
    // keeps a query-embedded `://` from being treated as an authority, and the
    // path slash must also fall before `end`.
    size_t start = 0;
    const size_t schemeEnd = target.find("://");
    if (schemeEnd != std::string::npos && schemeEnd < end) {
        const size_t slash = target.find('/', schemeEnd + 3);
        if (slash == std::string::npos || slash >= end) return std::string();  // authority only
        start = slash;
    }

    auto hexVal = [](char c) -> int {
        if (c >= '0' && c <= '9') return c - '0';
        if (c >= 'a' && c <= 'f') return c - 'a' + 10;
        if (c >= 'A' && c <= 'F') return c - 'A' + 10;
        return -1;
    };

    std::string out;
    out.reserve(end - start);
    for (size_t i = start; i < end; ++i) {
        if (target[i] == '%' && i + 2 < end) {
            const int hi = hexVal(target[i + 1]);
            const int lo = hexVal(target[i + 2]);
            if (hi >= 0 && lo >= 0) {
                out.push_back(static_cast<char>((hi << 4) | lo));
                i += 2;
                continue;
            }
        }
        out.push_back(target[i]);
    }
    return out;
}

// A loopback host:port in EITHER host spelling.
//
// ⛔ Exists because three hardcoded literals in simple_handler.cpp's resource
// dispatch matched only `localhost:<port>`, so a dApp using the `127.0.0.1`
// spelling of the SAME port was never intercepted. MEASURED 2026-08-19 against
// the HandCash App Lab, whose CSP pins connect-src to https://127.0.0.1:2121 and
// http://127.0.0.1:3321: `localhost:3321` was intercepted and re-pointed at our
// wallet, `127.0.0.1:3321` was ignored entirely, and the site reported "Bridge
// unavailable". Same port, same path, only the host form differed.
//
// The two spellings must always move in lockstep — that is the whole reason
// IsWalletHostPort above checks both. Route every foreign-bridge port through
// THIS helper rather than writing a literal, so the pair cannot drift again.
inline bool IsLoopbackHostPort(const std::string& url, const std::string& port) {
    return url.find("localhost:" + port) != std::string::npos
        || url.find("127.0.0.1:" + port) != std::string::npos;
}

}  // namespace hodos
