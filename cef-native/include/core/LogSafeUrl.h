#pragma once

#include <string>

// Redaction for URLs that reach a log line.
//
// WHY THIS EXISTS. The shipped log is a complete plaintext browsing history: 2.5 GB, every
// URL the user ever loaded, growing ~91 MB/day, never pruned, and surviving the user
// clearing their own history. On a browser whose product thesis is privacy.
//
// ⛔ The level gate does NOT solve this on its own, and assuming it did was the ticket's
// mistake. 77,911 INFO/WARN lines carry a full URL — ~87,000 of them HistoryManager
// narrating every visit WITH the page title. Those levels survive any gate we would
// sensibly ship, so redaction is a separate, independently necessary change.
// See development-docs/0.4.0-beta.3/phase-2-logging-syncio/MEASUREMENTS.md M7.
//
// THE RULE
//   * The query string and fragment are ALWAYS dropped. That is where session tokens,
//     search terms and identifiers live — the most sensitive part of a URL and the part
//     with the least diagnostic value.
//   * The path is dropped for ordinary hosts. "https://x.com/i/api/2/badge_count" says
//     what the user was doing; "https://x.com" says which site was involved, which is all
//     a log needs.
//   * The path is KEPT for loopback, because that is our OWN UI, not the user's browsing.
//     "http://127.0.0.1:5137/wallet-panel" is the difference between a usable log and a
//     useless one, and it discloses nothing about the user.
//   * Opaque and potentially huge schemes (data:, blob:, javascript:) collapse to the
//     scheme alone — a data: URL can inline an entire document.
//
// Header-only and dependency-free so cef-native/tests can cover it without CEF.

namespace hodos {

inline bool IsLoopbackLogHost(const std::string& host) {
    // Bare host, no port (the caller splits it off first).
    return host == "127.0.0.1" || host == "localhost" || host == "[::1]" || host == "::1";
}

/// Origin-only rendering of `url`, safe to persist in a log. Never throws.
inline std::string LogSafeUrl(const std::string& url) {
    if (url.empty()) return "";

    // The scheme is everything before the FIRST colon, and an authority follows only if
    // that colon is immediately followed by "//".
    //
    // ⛔ Do not look for the first "://" instead. "blob:https://example.com/6d1f" contains
    // one, and searching for it yields the scheme "blob:https" and treats an opaque URL as
    // though it had a host. The unit test for opaque schemes exists because that is exactly
    // what the first version of this function did.
    const std::string::size_type colon = url.find(':');
    if (colon == std::string::npos) return "(opaque)";
    if (url.compare(colon, 3, "://") != 0) {
        // No authority: data:, blob:, javascript:, about:, mailto:, chrome-extension: ...
        // Report the scheme and nothing else — the remainder can be an entire document.
        return url.substr(0, colon) + ":(redacted)";
    }

    const std::string::size_type schemeEnd = colon;
    const std::string scheme = url.substr(0, schemeEnd);
    const std::string::size_type authorityStart = schemeEnd + 3;

    // The authority ends at the first '/', '?' or '#'.
    std::string::size_type authorityEnd = url.size();
    for (std::string::size_type i = authorityStart; i < url.size(); ++i) {
        const char c = url[i];
        if (c == '/' || c == '?' || c == '#') {
            authorityEnd = i;
            break;
        }
    }
    std::string authority = url.substr(authorityStart, authorityEnd - authorityStart);

    // ⛔ Credentials in the authority ("user:pass@host") are dropped, not logged.
    const std::string::size_type at = authority.rfind('@');
    if (at != std::string::npos) authority = authority.substr(at + 1);

    std::string hostOnly = authority;
    const std::string::size_type portColon = authority.rfind(':');
    const std::string::size_type closeBracket = authority.rfind(']');
    if (portColon != std::string::npos &&
        (closeBracket == std::string::npos || portColon > closeBracket)) {
        hostOnly = authority.substr(0, portColon);
    }

    const std::string origin = scheme + "://" + authority;
    if (!IsLoopbackLogHost(hostOnly)) return origin;

    // Loopback: keep the path, drop query and fragment.
    std::string::size_type pathEnd = url.size();
    for (std::string::size_type i = authorityEnd; i < url.size(); ++i) {
        if (url[i] == '?' || url[i] == '#') {
            pathEnd = i;
            break;
        }
    }
    if (authorityEnd >= pathEnd) return origin;
    return origin + url.substr(authorityEnd, pathEnd - authorityEnd);
}

}  // namespace hodos
