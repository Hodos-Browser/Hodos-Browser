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

// ---------------------------------------------------------------------------
// P0.5 E1 — the wallet_call SSRF guard. macOS-only-EXPLOITABLE, cross-platform-FIXED.
//
// The IPC bridge builds `WalletBaseUrl() + endpoint`, where `endpoint` is a raw,
// page-controlled string off a wallet_call message (args[2]) with no route or
// shape check, and `httpMethod` (args[4]) reaches CURLOPT_CUSTOMREQUEST verbatim.
// WalletBaseUrl() has NO trailing slash (above), so an endpoint of "@evil.com/x"
// yields "http://127.0.0.1:31301@evil.com/x" — curl reads the host up to the LAST
// '@', so the browser process fetches evil.com. Windows fails closed only by
// ACCIDENT (ParseUrl's digits-only port check, SyncHttpClient.cpp #ifdef _WIN32);
// the macOS libcurl arm (#elif __APPLE__) has no validation of any kind and
// honours it, with page-controlled method + body ⇒ an arbitrary-method,
// arbitrary-body loopback request primitive.
//
// Fixed HERE — one predicate pair, both platforms, applied ONCE at the single
// shared dispatch choke (dispatchWalletHttpByMethod). ⛔ NOT by porting ParseUrl
// to macOS: replicating Windows' accidental digits-only safety would give two
// derivations of one value on two platforms, the exact failure mode CLAUDE.md
// warns of for RegistrableDomainFromUrl. Validate the INPUT, once.
//
// This closes the authority-escape and the CRLF/method-injection class. The
// endpoint ALLOWLIST (accept only known routes) is deliberately Phase 5, not
// here — that is the route table, a different and larger change.

// True iff `url` targets the local wallet base and nothing else: exactly
// WalletBaseUrl() followed by a leading-'/' path, with no control characters.
// Anchoring to `WalletBaseUrl() + "/"` enforces BOTH properties at once — the
// endpoint began with '/', and no userinfo/host was injected right after the
// authority (the "@evil.com" pivot fails because the char after the base is '@',
// not '/'). The no-control-char sweep stops CRLF request-splitting in the path
// or query. Fails closed on the empty endpoint (url == base, no trailing slash).
inline bool IsWalletDispatchUrlSafe(const std::string& url) {
    const std::string prefix = WalletBaseUrl() + "/";
    if (url.rfind(prefix, 0) != 0) return false;
    for (unsigned char c : url) {
        if (c < 0x20 || c == 0x7f) return false;  // CR, LF, NUL, DEL, other C0
    }
    return true;
}

// True iff `method` is a plausible HTTP verb: non-empty, all uppercase ASCII
// letters, short. The real verbs (GET/POST/PUT/DELETE/PATCH/HEAD/OPTIONS) all
// satisfy this; anything carrying CRLF, a space, digits or lowercase — the
// header-injection vectors into CURLOPT_CUSTOMREQUEST — fails closed. The longest
// standard verb is OPTIONS (7); 8 leaves one char of slack without admitting
// junk.
inline bool IsValidWalletMethod(const std::string& method) {
    if (method.empty() || method.size() > 8) return false;
    for (char c : method) {
        if (c < 'A' || c > 'Z') return false;
    }
    return true;
}

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
// The authority's [start, end) span within `url`, with userinfo already skipped.
// Returns false when the URL carries no real scheme.
//
// ⭐ ONE derivation of "where is the authority". OriginFromUrl reads it and
// RepointLoopbackToWallet rewrites through it, so the reader and the writer
// cannot disagree about which bytes are the host — the failure mode CLAUDE.md
// warns about for RegistrableDomainFromUrl.
inline bool AuthoritySpan(const std::string& url, size_t& start, size_t& end) {
    if (url.rfind("http://", 0) == 0)        start = 7;
    else if (url.rfind("https://", 0) == 0)  start = 8;
    else return false;

    end = url.size();
    for (size_t i = start; i < url.size(); ++i) {
        const char c = url[i];
        if (c == '/' || c == '?' || c == '#') { end = i; break; }
    }

    // Userinfo: everything through the LAST '@' is credentials, not the host.
    if (end > start) {
        const size_t at = url.rfind('@', end - 1);
        if (at != std::string::npos && at >= start && at < end) start = at + 1;
    }
    return true;
}

inline std::string OriginFromUrl(const std::string& url) {
    size_t start = 0, end = 0;
    if (!AuthoritySpan(url, start, end)) return std::string();
    return url.substr(start, end - start);  // may be empty ("http:///x")
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

// ---------------------------------------------------------------------------
// beta.3 Phase 5 (W0) — THE wallet-traffic predicate. One derivation.
//
// ⭐ READ THIS BEFORE CHANGING ANY PREDICATE BELOW. The C++ interception layer
// is not only a permission gate — it is the component that marks traffic as
// UNTRUSTED. `domain_trust_mw` (rust-wallet/src/main.rs) reads
// X-Requesting-Domain, and a MISSING header means "internal, fully trusted,
// ungated". C++ attaches that header only for traffic it recognises as coming
// from a page.
//
// ⇒ A matcher that fails to match does NOT leave traffic ungated. It leaves it
//   TRUSTED. Every narrowing here is a PRIVILEGE CHANGE.
//
// That inverts the usual instinct, and it produces the two rules these
// predicates are built on:
//
//   1. BE BROAD ABOUT WHAT COUNTS AS LOOPBACK. Accept everything Chromium will
//      actually route to this machine. A strict `host == "localhost"` test is
//      NARROWER than the substring gate it replaces:
//      "http://x.localhost:31401/createAction" contains "localhost:31401", so it
//      is intercepted TODAY. Chromium resolves the whole *.localhost tree to
//      loopback (RFC 6761), so the request still reaches the wallet — but
//      unstamped, i.e. as a first-party call. That is a privilege escalation
//      shipped by a "cleanup".
//
//   2. BE STRICT ABOUT WHERE IN THE URL YOU LOOK. Only the authority. The old
//      whole-URL find() admitted any URL merely CONTAINING the host:port,
//      including in a query string the page author controls.
//      MEASURED 2026-09-02 from a live page (phase-5 MEASUREMENTS.md M2):
//      fetch('https://example.com/getNetwork?x=127.0.0.1:3321') was intercepted,
//      had its own query rewritten by redirectPort, was silently downgraded
//      https->http, and was answered by our wallet. example.com was never
//      contacted.
//
// ⛔ Built on OriginFromUrl + this file's own string parsing, NOT on
// CefParseURL. cef-native/tests/CMakeLists.txt does not link libcef, so a
// CefParseURL wrapper would be un-unit-testable — which is the exact reason the
// ticket moved this out of PortConfig.h in the first place. It belongs here
// after all, because the pure parser it needed already lives here.

// Split an authority into host and port. Understands the bracketed IPv6 form.
// A malformed bracketed authority yields an empty host, which fails closed in
// every caller below.
inline void SplitAuthority(const std::string& authority,
                           std::string& host, std::string& port) {
    host.clear();
    port.clear();
    if (authority.empty()) return;

    size_t colon;
    if (authority[0] == '[') {
        const size_t close = authority.find(']');
        if (close == std::string::npos) return;      // malformed -> no host
        host = authority.substr(0, close + 1);       // brackets kept: "[::1]"
        colon = authority.find(':', close + 1);
    } else {
        colon = authority.find(':');
        host = authority.substr(0, colon == std::string::npos ? authority.size() : colon);
    }
    if (colon != std::string::npos) port = authority.substr(colon + 1);
}

// "Will Chromium route this host to THIS machine?" — deliberately BROAD, per
// rule 1 above.
//
// ⚠️ Relies on the host already being URL-CANONICAL, which it is: every caller
// derives it from CefRequest::GetURL(), and GURL canonicalises the host before
// we ever see it — lowercasing it, and normalising every IPv4 spelling
// (decimal "2130706433", octal, hex) to dotted-quad. Without that guarantee the
// digits-and-dots test below would be bypassable. The tolower is belt-and-braces
// for any future caller that does not come from GetURL().
//
// ⛔ Do NOT narrow this to exact equality. See rule 1 — `AuthorityHasHost` above
// is the NARROW predicate and exists for the opposite job (granting privilege,
// where failing closed is correct). Broad for stamping, narrow for granting.
// They are different functions on purpose; do not "unify" them.
inline bool IsLoopbackHost(const std::string& host_in) {
    if (host_in.empty()) return false;
    std::string host = host_in;
    for (char& c : host) {
        if (c >= 'A' && c <= 'Z') c = static_cast<char>(c - 'A' + 'a');
    }

    if (host == "localhost" || host == "[::1]" || host == "::1") return true;

    // RFC 6761: the entire *.localhost tree resolves to loopback in Chromium.
    static const std::string kDotLocalhost = ".localhost";
    if (host.size() > kDotLocalhost.size() &&
        host.compare(host.size() - kDotLocalhost.size(),
                     kDotLocalhost.size(), kDotLocalhost) == 0) {
        return true;
    }

    // 127.0.0.0/8. Digits and dots only, so "127.0.0.1.evil.com" and
    // "127.evil.com" are correctly refused — they are ordinary hostnames that
    // merely start with the text.
    if (host.compare(0, 4, "127.") == 0) {
        for (char c : host) {
            if ((c < '0' || c > '9') && c != '.') return false;
        }
        return true;
    }
    return false;
}

inline bool IsLoopbackAuthority(const std::string& authority) {
    std::string host, port;
    SplitAuthority(authority, host, port);
    return IsLoopbackHost(host);
}

// The foreign BRC-100 bridge ports we deliberately answer for, so a dApp
// hardcoded to another wallet still reaches Hodos.
//
// ⛔ 8080 was REMOVED here in Phase 5 (owner decision, 2026-09-02). It is in no
// BRC, no MetaNet client and no App Lab path; `git log -S 'localhost:8080'`
// traces it to a single "initial commit from old repo". It IS the default port
// of Spring Boot, Tomcat, http-server and many Docker images, and because the
// path table carries bare substrings like /health, /encrypt and /getVersion, a
// developer's own server on 8080 was being answered by the wallet. Do not
// re-add it.
inline bool IsCompatBridgePort(const std::string& port) {
    return port == "3321"      // MetaNet Client
        || port == "2121";     // HandCash local bridge
}

// True iff `url`'s AUTHORITY is a loopback host on our wallet port.
// Replaces the unanchored IsWalletHostPort at every decision site.
inline bool IsOurWalletOrigin(const std::string& url) {
    std::string host, port;
    SplitAuthority(OriginFromUrl(url), host, port);
    return IsLoopbackHost(host) && port == WalletPortStr();
}

// ⭐ THE gate predicate: is this request addressed to a wallet on this machine —
// ours, or a foreign bridge we answer for?
inline bool IsWalletOrigin(const std::string& url) {
    std::string host, port;
    SplitAuthority(OriginFromUrl(url), host, port);
    if (!IsLoopbackHost(host)) return false;
    return port == WalletPortStr() || IsCompatBridgePort(port);
}

// The Babbage MessageBox relay, matched on the HOST rather than anywhere in the
// URL text. Behaviour for the real relay is unchanged; what stops matching is
// "https://evil.example/?x=messagebox.babbage.systems", which today skips the
// cookie filter and the BRC-121 402 response check by impersonating relay
// traffic.
inline bool IsMessageboxOrigin(const std::string& url) {
    std::string host, port;
    SplitAuthority(OriginFromUrl(url), host, port);
    return host == "messagebox.babbage.systems";
}

// BRC-104 authentication endpoint, matched on the PATH.
//
// ⭐ Minimal narrowing on purpose: the defect was that the old test searched the
// whole URL including the query, so "/x?y=/.well-known/auth" matched.
// RequestPathForMatching cuts the query and fragment first, so this keeps the
// old find()-anywhere-in-the-PATH semantics and fixes only the query injection.
// Whether the request is then re-pointed at OUR wallet is a separate decision,
// made on the authority — see HttpRequestInterceptor's /.well-known/auth arm.
inline bool IsWellKnownAuthRequest(const std::string& url) {
    return RequestPathForMatching(url).find("/.well-known/auth") != std::string::npos;
}

// beta.3 Phase 5 — re-point a foreign loopback bridge URL at OUR wallet port,
// rewriting ONLY the authority. Returns the URL unchanged if there is nothing to
// do.
//
// 🚨 This replaces a whole-URL search-and-replace that could MANUFACTURE the
// wallet host:port out of text the page controls. MEASURED 2026-09-02 (phase-5
// MEASUREMENTS.md M2), the old lambda turned
//   https://example.com/getNetwork?x=127.0.0.1:3321
// into
//   https://example.com/getNetwork?x=127.0.0.1:31401
// which then satisfied the very predicate that was supposed to prove the request
// was ours — after which the request was downgraded to http:// and answered by
// our wallet, with example.com never contacted.
//
// ⭐ Behaviour for real traffic is deliberately UNCHANGED: any loopback host on
// any explicit port that is not already ours is re-pointed, which is what the
// BRC-104 arm relies on for a local wallet on a non-standard port. The only
// difference is that the port must be in the authority.
inline std::string RepointLoopbackToWallet(const std::string& url) {
    size_t start = 0, end = 0;
    if (!AuthoritySpan(url, start, end)) return url;

    const std::string authority = url.substr(start, end - start);
    std::string host, port;
    SplitAuthority(authority, host, port);

    if (!IsLoopbackHost(host)) return url;
    if (port.empty()) return url;               // no explicit port — not a bridge
    if (port == WalletPortStr()) return url;    // already ours
    for (char c : port) {                       // a non-numeric port is malformed
        if (c < '0' || c > '9') return url;
    }
    if (port.size() > 5) return url;

    return url.substr(0, start) + host + ":" + WalletPortStr() + url.substr(end);
}

// beta.3 Phase 5 (W3) — the SHADOW predicate: the six-term gate exactly as it
// stood before this phase, kept solely so the new gate's disagreements can be
// logged and inspected. ⛔ Not a decision site. It is retired with
// IsWalletHostPort / IsLoopbackHostPort in beta.4 (ticket W8).
inline bool LegacyWalletGateMatch(const std::string& url) {
    return IsWalletHostPort(url)
        || IsLoopbackHostPort(url, "3321")
        || IsLoopbackHostPort(url, "2121")
        || IsLoopbackHostPort(url, "8080")
        || url.find("messagebox.babbage.systems") != std::string::npos
        || url.find("/.well-known/auth") != std::string::npos;
}

}  // namespace hodos
