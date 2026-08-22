#pragma once

// Pure BSV QR-payload classifier for the screen-capture path.
//
// Extracted from QRScreenCapture.cpp so the scheme allowlist and the BIP21 amount
// validation are unit-testable WITHOUT CEF, GDI or quirc (the same reason
// JsStringEscape.h was extracted by the F6 HelicOps audit). CEF-free: <regex> /
// <string> / <sstream> / <cctype> / <cstdio> only.
//
// ⚠️ SECURITY: the returned JSON is later string-concatenated into JavaScript and
// executed in the wallet overlay (simple_render_process_handler.cpp). `amount` is
// emitted UNQUOTED, so it MUST be a plain decimal number — an unvalidated value is
// arbitrary JS in the privileged overlay. See MEASUREMENT_amount_injection.md.
//
// The macOS twin (cef_browser_shell_mac.mm :: ClassifyBSVContent) is still a
// hand-copy of this logic — kept in lockstep by hand for now (deduplication is a
// separate, later change). If you touch the rule here, mirror it there.

#include <regex>
#include <string>
#include <sstream>
#include <cctype>
#include <cstdio>

namespace hodos {

inline const std::regex& QrReBsvAddress() {
    static const std::regex re(R"(^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$)");
    return re;
}
inline const std::regex& QrReIdentityKey() {
    static const std::regex re(R"(^(02|03)[0-9a-fA-F]{64}$)");
    return re;
}
inline const std::regex& QrRePaymail() {
    static const std::regex re(R"(^(\$[a-zA-Z0-9_]+|[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,})$)");
    return re;
}
// Allowlist of payment schemes: bitcoin: and bsv:. Do NOT widen to "any scheme".
inline const std::regex& QrReBip21() {
    static const std::regex re(R"(^(bitcoin|bsv):)", std::regex_constants::icase);
    return re;
}
// A BIP21 amount is a decimal number of whole coins. Emitted unquoted downstream.
inline const std::regex& QrReBip21Amount() {
    static const std::regex re(R"(^[0-9]+(\.[0-9]+)?$)");
    return re;
}

// URL-decode a string (for BIP21 label parsing).
inline std::string QrUrlDecode(const std::string& s) {
    std::string result;
    result.reserve(s.size());
    for (size_t i = 0; i < s.size(); ++i) {
        if (s[i] == '%' && i + 2 < s.size()) {
            int hi = 0, lo = 0;
            if (sscanf(s.c_str() + i + 1, "%1x%1x", &hi, &lo) == 2) {
                result += static_cast<char>((hi << 4) | lo);
                i += 2;
                continue;
            }
        }
        if (s[i] == '+') { result += ' '; continue; }
        result += s[i];
    }
    return result;
}

// Escape a string for JSON embedding.
inline std::string QrJsonEscape(const std::string& s) {
    std::string out;
    out.reserve(s.size() + 8);
    for (char c : s) {
        switch (c) {
            case '"':  out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n";  break;
            case '\r': out += "\\r";  break;
            case '\t': out += "\\t";  break;
            default:   out += c;      break;
        }
    }
    return out;
}

// Classify a QR payload and build a JSON result object (source="screen").
// Returns "" if the payload doesn't match any BSV pattern.
inline std::string ClassifyQRPayload(const std::string& text) {
    // BIP21 URI (bitcoin: or bsv:)
    if (std::regex_search(text, QrReBip21())) {
        std::string address, amount, label;
        size_t colon = text.find(':');
        std::string rest = (colon != std::string::npos) ? text.substr(colon + 1) : text;

        size_t q = rest.find('?');
        address = (q != std::string::npos) ? rest.substr(0, q) : rest;

        if (q != std::string::npos) {
            std::string params = rest.substr(q + 1);
            std::istringstream ps(params);
            std::string pair;
            while (std::getline(ps, pair, '&')) {
                size_t eq = pair.find('=');
                if (eq == std::string::npos) continue;
                std::string key = pair.substr(0, eq);
                std::string val = QrUrlDecode(pair.substr(eq + 1));
                if (key == "amount") amount = val;
                else if (key == "label") label = val;
            }
        }

        std::string json = "{\"type\":\"bip21\",\"value\":\"" + QrJsonEscape(text) + "\"";
        if (!address.empty()) json += ",\"address\":\"" + QrJsonEscape(address) + "\"";
        // Emit amount ONLY if it is a plain decimal number. Anything else (an
        // injected JS expression, or a non-numeric string) is dropped.
        if (!amount.empty() && std::regex_match(amount, QrReBip21Amount()))
            json += ",\"amount\":" + amount;
        if (!label.empty())   json += ",\"label\":\"" + QrJsonEscape(label) + "\"";
        json += ",\"source\":\"screen\"}";
        return json;
    }

    if (std::regex_match(text, QrReBsvAddress())) {
        return "{\"type\":\"address\",\"value\":\"" + QrJsonEscape(text) +
               "\",\"address\":\"" + QrJsonEscape(text) + "\",\"source\":\"screen\"}";
    }
    if (std::regex_match(text, QrReIdentityKey())) {
        return "{\"type\":\"identity_key\",\"value\":\"" + QrJsonEscape(text) +
               "\",\"source\":\"screen\"}";
    }
    if (std::regex_match(text, QrRePaymail())) {
        return "{\"type\":\"paymail\",\"value\":\"" + QrJsonEscape(text) +
               "\",\"source\":\"screen\"}";
    }
    return ""; // Not a BSV pattern
}

// Extract the scheme (up to the first ':') for a user-facing "decoded but not a BSV
// payment" message. Sanitized so a hostile payload can't bloat the message:
// letters/digits/+/-/. only, capped at 12 chars.
inline std::string SchemeForMessage(const std::string& payload) {
    size_t colon = payload.find(':');
    std::string s = (colon != std::string::npos) ? payload.substr(0, colon)
                                                  : payload.substr(0, 12);
    std::string out;
    for (char c : s) {
        if (out.size() >= 12) break;
        if (std::isalnum(static_cast<unsigned char>(c)) || c == '+' || c == '-' || c == '.')
            out += c;
    }
    return out;
}

} // namespace hodos
