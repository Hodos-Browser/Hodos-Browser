// ManifestFetcher implementation — see header for design intent.
//
// ⚠️ Mirror of `rust-wallet/src/manifest.rs`. Change both or neither.
//
// Parse is pure; Fetch is the thin networking wrapper. Both never throw —
// any failure returns Manifest{valid=false}.

#include "../../include/core/ManifestFetcher.h"
#include "../../include/core/SyncHttpClient.h"

#include <nlohmann/json.hpp>

#include <algorithm>
#include <cctype>

namespace hodos {

namespace {

// 3-second hard cap. Per PERMISSION_UX_DESIGN.md design decision #5,
// manifest fetch is sync at first-visit; user is already waiting on connect,
// but we cap aggressively so a slow/hostile server can't hang the prompt.
constexpr int kFetchTimeoutMs = 3000;

// 64 KB cap. Manifests are tiny by nature (bitgenius, the largest real one
// observed, is 3358 bytes). Anything bigger is suspicious; reject without
// parsing.
constexpr size_t kMaxManifestBytes = 64 * 1024;

// ── Bounded field readers ───────────────────────────────────────────────────

// Truncate to `max` characters. UTF-8 aware only to the extent of not splitting
// a multi-byte sequence: we walk back off any continuation byte (0b10xxxxxx) so
// the result is still valid UTF-8 for the JSON re-serialization that follows.
std::string clampStr(const std::string& s, size_t max) {
    if (s.size() <= max) return s;
    size_t cut = max;
    while (cut > 0 && (static_cast<unsigned char>(s[cut]) & 0xC0) == 0x80) --cut;
    return s.substr(0, cut);
}

std::string strField(const nlohmann::json& obj, const char* key, size_t max) {
    if (obj.contains(key) && obj[key].is_string()) {
        return clampStr(obj[key].get<std::string>(), max);
    }
    return std::string();
}

int64_t intField(const nlohmann::json& obj, const char* key) {
    if (obj.contains(key) && obj[key].is_number_integer()) {
        return obj[key].get<int64_t>();
    }
    if (obj.contains(key) && obj[key].is_number()) {
        return static_cast<int64_t>(obj[key].get<double>());
    }
    return 0;
}

// An iconUrl is rendered straight into the connect modal's <img src>. Accept
// only an absolute https:// URL; anything else (a data: blob, a javascript:
// string, a relative path, a plaintext http:// icon) is dropped and the modal
// falls back to its favicon path.
std::string safeIconUrl(const std::string& raw) {
    const std::string s = clampStr(raw, kMaxManifestIdStr);
    const std::string prefix = "https://";
    if (s.size() > prefix.size() && s.compare(0, prefix.size(), prefix) == 0) return s;
    return std::string();
}

// Return the nested object at obj[key], or nullptr.
const nlohmann::json* objField(const nlohmann::json& obj, const char* key) {
    if (obj.contains(key) && obj[key].is_object()) return &obj[key];
    return nullptr;
}

// ── BRC-73 `groupPermissions` ───────────────────────────────────────────────

// Translate a `metanet`/`babbage` groupPermissions object into our Manifest.
// Mirrors `manifest.rs :: apply_group_permissions`.
void applyGroupPermissions(Manifest& m, const nlohmann::json& gp) {
    m.groupDescription = strField(gp, "description", kMaxManifestStr);

    // protocolPermissions[] — BRC-73: protocolID is the BRC-43 tuple
    // [securityLevel, protocolName]; `counterparty` is required for a specific
    // Level 2 and MAY be omitted at Level 1; `description` is for the user.
    if (gp.contains("protocolPermissions") && gp["protocolPermissions"].is_array()) {
        for (const auto& p : gp["protocolPermissions"]) {
            if (m.protocols.size() >= kMaxManifestEntries) break;
            if (!p.is_object()) continue;
            ManifestProtocol mp;
            // BRC-73 has no keyID concept — a declaration covers the protocol,
            // so the wildcard our grant rows already understand is the faithful
            // translation.
            mp.keyId = "*";
            mp.counterparty = strField(p, "counterparty", kMaxManifestIdStr);
            mp.purpose = strField(p, "description", kMaxManifestStr);
            if (p.contains("protocolID") && p["protocolID"].is_array()
                && p["protocolID"].size() >= 2) {
                const auto& arr = p["protocolID"];
                if (arr[0].is_number()) {
                    const int lvl = arr[0].get<int>();
                    if (lvl >= 0 && lvl <= 2) mp.securityLevel = lvl;
                }
                if (arr[1].is_string()) {
                    mp.name = clampStr(arr[1].get<std::string>(), kMaxManifestIdStr);
                }
            }
            // Reject malformed: no name = nothing to grant.
            if (!mp.name.empty()) m.protocols.push_back(std::move(mp));
        }
    }

    // spendingAuthorization — a single object. ⛔ `amount` lands in
    // monthlySatoshis and NOWHERE else (R-CAPS / P0.8-A5).
    if (const auto* sa = objField(gp, "spendingAuthorization")) {
        const int64_t amount = intField(*sa, "amount");
        m.spending.monthlySatoshis = amount > 0 ? amount : 0;
        m.spending.purpose = strField(*sa, "description", kMaxManifestStr);
    }

    // basketAccess[] — BRC-73 declares only a basket name. BRC-116 §4.3:
    // basket grants are binary and cover insertion, listing and removal, so
    // read_write is the faithful translation; "read" would silently
    // under-grant and re-introduce the per-call prompts the bundle avoids.
    if (gp.contains("basketAccess") && gp["basketAccess"].is_array()) {
        for (const auto& b : gp["basketAccess"]) {
            if (m.baskets.size() >= kMaxManifestEntries) break;
            if (!b.is_object()) continue;
            ManifestBasket mb;
            mb.name = strField(b, "basket", kMaxManifestIdStr);
            mb.access = "read_write";
            mb.purpose = strField(b, "description", kMaxManifestStr);
            if (!mb.name.empty()) m.baskets.push_back(std::move(mb));
        }
    }

    // certificateAccess[] — type + fields + verifierPublicKey + description.
    if (gp.contains("certificateAccess") && gp["certificateAccess"].is_array()) {
        for (const auto& c : gp["certificateAccess"]) {
            if (m.certificates.size() >= kMaxManifestEntries) break;
            if (!c.is_object()) continue;
            ManifestCertificate mc;
            mc.type = strField(c, "type", kMaxManifestIdStr);
            mc.verifierPublicKey = strField(c, "verifierPublicKey", kMaxManifestIdStr);
            mc.purpose = strField(c, "description", kMaxManifestStr);
            if (c.contains("fields") && c["fields"].is_array()) {
                for (const auto& f : c["fields"]) {
                    if (mc.fields.size() >= kMaxManifestFields) break;
                    if (f.is_string()) {
                        mc.fields.push_back(clampStr(f.get<std::string>(), kMaxManifestIdStr));
                    }
                }
            }
            // Reject malformed: no type = nothing to grant.
            if (!mc.type.empty()) m.certificates.push_back(std::move(mc));
        }
    }

    // ⚠️ NOT mapped: BRC-116's `metanet.counterpartyPermissions` (PACT). See
    // the ManifestCounterparty comment in the header.
}

// ── Our own top-level `permissions` shape (PERMISSION_UX_DESIGN.md §5) ──────

void applyHodosLegacyPermissions(Manifest& m, const nlohmann::json& perms) {
    if (perms.contains("protocols") && perms["protocols"].is_array()) {
        for (const auto& p : perms["protocols"]) {
            if (m.protocols.size() >= kMaxManifestEntries) break;
            if (!p.is_object()) continue;
            ManifestProtocol mp;
            if (p.contains("keyID") && p["keyID"].is_string()) {
                mp.keyId = clampStr(p["keyID"].get<std::string>(), kMaxManifestIdStr);
            } else {
                mp.keyId = "*";  // default wildcard per Phase 1.5
            }
            mp.counterparty = strField(p, "counterparty", kMaxManifestIdStr);
            mp.purpose = strField(p, "purpose", kMaxManifestStr);
            // protocolID is [securityLevel, name] per BRC-43. Our shape also
            // allows the flattened `name` + `securityLevel` spelling — which
            // our OWN documented example fixture uses and which never parsed
            // before Phase 0.8.
            if (p.contains("protocolID") && p["protocolID"].is_array()
                && p["protocolID"].size() >= 2) {
                const auto& arr = p["protocolID"];
                if (arr[0].is_number()) {
                    const int lvl = arr[0].get<int>();
                    if (lvl >= 0 && lvl <= 2) mp.securityLevel = lvl;
                }
                if (arr[1].is_string()) {
                    mp.name = clampStr(arr[1].get<std::string>(), kMaxManifestIdStr);
                }
            } else {
                if (p.contains("securityLevel") && p["securityLevel"].is_number()) {
                    const int lvl = p["securityLevel"].get<int>();
                    if (lvl >= 0 && lvl <= 2) mp.securityLevel = lvl;
                }
                mp.name = strField(p, "name", kMaxManifestIdStr);
            }
            if (!mp.name.empty()) m.protocols.push_back(std::move(mp));
        }
    }

    if (perms.contains("baskets") && perms["baskets"].is_array()) {
        for (const auto& b : perms["baskets"]) {
            if (m.baskets.size() >= kMaxManifestEntries) break;
            if (!b.is_object()) continue;
            ManifestBasket mb;
            mb.name = strField(b, "name", kMaxManifestIdStr);
            if (b.contains("access") && b["access"].is_string()) {
                mb.access = b["access"].get<std::string>();
            } else {
                mb.access = "read";
            }
            mb.purpose = strField(b, "purpose", kMaxManifestStr);
            // Reject malformed: no name OR invalid access level.
            if (!mb.name.empty() && (mb.access == "read" || mb.access == "read_write")) {
                m.baskets.push_back(std::move(mb));
            }
        }
    }

    if (perms.contains("certificates") && perms["certificates"].is_array()) {
        for (const auto& c : perms["certificates"]) {
            if (m.certificates.size() >= kMaxManifestEntries) break;
            if (!c.is_object()) continue;
            ManifestCertificate mc;
            mc.type = strField(c, "type", kMaxManifestIdStr);
            mc.verifierPublicKey = strField(c, "verifierPublicKey", kMaxManifestIdStr);
            mc.purpose = strField(c, "purpose", kMaxManifestStr);
            if (c.contains("fields") && c["fields"].is_array()) {
                for (const auto& f : c["fields"]) {
                    if (mc.fields.size() >= kMaxManifestFields) break;
                    if (f.is_string()) {
                        mc.fields.push_back(clampStr(f.get<std::string>(), kMaxManifestIdStr));
                    }
                }
            }
            if (!mc.type.empty()) m.certificates.push_back(std::move(mc));
        }
    }

    if (const auto* s = objField(perms, "spending")) {
        const int64_t perTx = intField(*s, "perTransactionUsd");
        const int64_t perSession = intField(*s, "perSessionUsd");
        m.spending.perTransactionUsd = perTx > 0 ? perTx : 0;
        m.spending.perSessionUsd = perSession > 0 ? perSession : 0;
        m.spending.purpose = strField(*s, "purpose", kMaxManifestStr);
    }

    if (perms.contains("counterparties") && perms["counterparties"].is_array()) {
        for (const auto& cp : perms["counterparties"]) {
            if (m.counterparties.size() >= kMaxManifestEntries) break;
            if (!cp.is_object()) continue;
            ManifestCounterparty mcp;
            mcp.type = strField(cp, "type", kMaxManifestIdStr);
            mcp.counterparty = strField(cp, "counterparty", kMaxManifestIdStr);
            mcp.purpose = strField(cp, "purpose", kMaxManifestStr);
            // Need at least one identifier (type or counterparty) to be useful.
            if (!mcp.type.empty() || !mcp.counterparty.empty()) {
                m.counterparties.push_back(std::move(mcp));
            }
        }
    }
}

} // anonymous namespace

std::vector<std::string> ManifestFetcher::ManifestUrls(const std::string& origin) {
    // Mirrors `manifest.rs :: manifest_origin_base` + `manifest_urls`.
    std::vector<std::string> out;

    // Trim surrounding whitespace (a header value may carry a stray newline).
    size_t b = 0, e = origin.size();
    while (b < e && std::isspace(static_cast<unsigned char>(origin[b]))) ++b;
    while (e > b && std::isspace(static_cast<unsigned char>(origin[e - 1]))) --e;
    std::string trimmed = origin.substr(b, e - b);
    if (trimmed.empty()) return out;

    bool explicitHttp = false;
    std::string authority;
    if (trimmed.rfind("https://", 0) == 0) {
        authority = trimmed.substr(8);
    } else if (trimmed.rfind("http://", 0) == 0) {
        explicitHttp = true;
        authority = trimmed.substr(7);
    } else if (trimmed.find("://") != std::string::npos) {
        // Some other scheme entirely (file://, javascript:…) — not an origin
        // we will ever fetch from.
        return out;
    } else {
        authority = trimmed;
    }

    // Tolerate exactly one trailing slash on an otherwise clean authority.
    if (!authority.empty() && authority.back() == '/') authority.pop_back();
    if (authority.empty()) return out;

    // R-PATH: a path, query, fragment, userinfo or interior whitespace means
    // this is not a bare authority. Reject rather than concatenate.
    for (const char ch : authority) {
        const unsigned char uc = static_cast<unsigned char>(ch);
        if (ch == '/' || ch == '\\' || ch == '?' || ch == '#' || ch == '@'
            || std::isspace(uc) || std::iscntrl(uc)) {
            return out;
        }
    }

    // BRC-116 §9.2 — HTTPS only, except localhost for development.
    const auto startsWith = [&authority](const char* p) {
        const size_t n = std::char_traits<char>::length(p);
        return authority.size() >= n && authority.compare(0, n, p) == 0;
    };
    const bool isLoopback = authority == "localhost" || startsWith("localhost:")
                         || authority == "127.0.0.1" || startsWith("127.0.0.1:")
                         || startsWith("[::1]");
    const std::string scheme = (explicitHttp && isLoopback) ? "http" : "https";

    const std::string base = scheme + "://" + authority;
    out.push_back(base + "/manifest.json");                          // BRC-73
    out.push_back(base + "/.well-known/wallet-manifest.json");       // ours
    return out;
}

Manifest ManifestFetcher::Fetch(const std::string& origin) {
    Manifest result;  // valid=false by default

    for (const std::string& url : ManifestUrls(origin)) {
        HttpResponse resp = SyncHttpClient::Get(url, kFetchTimeoutMs);
        if (!resp.success || resp.statusCode != 200) {
            // 404, timeout, network error — all treated identically.
            continue;
        }
        if (resp.body.size() > kMaxManifestBytes) {
            // Oversized body — bail without parsing to avoid DoS.
            continue;
        }
        // ⛔ Status code is not evidence. Only a successful parse counts.
        Manifest m = ParseFromJson(resp.body);
        if (m.valid) return m;
    }
    return result;
}

Manifest ManifestFetcher::ParseFromJson(const std::string& json) {
    Manifest m;  // valid=false by default

    try {
        auto j = nlohmann::json::parse(json);

        // Top-level must be an object. A top-level array or scalar (e.g.
        // `[1,2,3]`, `"hello"`, `42`) is not a valid manifest. nlohmann's
        // contains() doesn't throw on non-objects so this guard is explicit.
        if (!j.is_object()) {
            return m;  // valid=false
        }

        // Top-level fields — all optional.
        m.version = strField(j, "version", kMaxManifestIdStr);
        m.name = strField(j, "name", kMaxManifestStr);
        if (m.name.empty()) m.name = strField(j, "short_name", kMaxManifestStr);
        m.description = strField(j, "description", kMaxManifestStr);
        m.iconUrl = safeIconUrl(strField(j, "iconUrl", kMaxManifestIdStr));
        m.expiresAt = intField(j, "expiresAt");

        // Namespace precedence. BRC-116 Backwards Compatibility: "Check for
        // `metanet` first. If present, use it. If `metanet` is absent, fall
        // back to `babbage`." We condition on `groupPermissions` actually being
        // present, not merely on the namespace key — socialcert.net serves a
        // bare `babbage` object with no `groupPermissions`, and a
        // `metanet: {schemaVersion: 1}` with nothing else must not shadow a
        // populated `babbage`.
        const nlohmann::json* metanetNs = objField(j, kSourceMetanet);
        const nlohmann::json* babbageNs = objField(j, kSourceBabbage);
        const nlohmann::json* metanetGp = metanetNs ? objField(*metanetNs, "groupPermissions") : nullptr;
        const nlohmann::json* babbageGp = babbageNs ? objField(*babbageNs, "groupPermissions") : nullptr;

        if (metanetGp) {
            m.sourceNamespace = kSourceMetanet;
            applyGroupPermissions(m, *metanetGp);
        } else if (babbageGp) {
            m.sourceNamespace = kSourceBabbage;
            applyGroupPermissions(m, *babbageGp);
        } else if (const auto* perms = objField(j, "permissions")) {
            m.sourceNamespace = kSourceHodosLegacy;
            applyHodosLegacyPermissions(m, *perms);
        }

        // 🚨 The Phase 0.8 tightening. A document with no recognised permission
        // is not a wallet manifest for our purposes, whatever else it contains.
        // This is what makes the existing `else openDomainApprovalModal(...)`
        // fallback in HttpRequestInterceptor.cpp actually fire.
        // ⛔ Do NOT relax back to "valid even if some sections are empty".
        m.valid = m.HasDeclaredPermissions();
    } catch (...) {
        // nlohmann throws on malformed JSON. Swallow and return invalid.
        m.valid = false;
    }

    return m;
}

} // namespace hodos
