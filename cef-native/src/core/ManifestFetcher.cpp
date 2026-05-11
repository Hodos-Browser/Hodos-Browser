// ManifestFetcher implementation — see header for design intent.
//
// Parse is pure; Fetch is the thin networking wrapper. Both never throw —
// any failure returns Manifest{valid=false}.

#include "../../include/core/ManifestFetcher.h"
#include "../../include/core/SyncHttpClient.h"

#include <nlohmann/json.hpp>

namespace hodos {

namespace {

// 3-second hard cap. Per PERMISSION_UX_DESIGN.md design decision #5,
// manifest fetch is sync at first-visit; user is already waiting on connect,
// but we cap aggressively so a slow/hostile server can't hang the prompt.
constexpr int kFetchTimeoutMs = 3000;

// 64 KB cap. Manifests are tiny by nature (the example in PERMISSION_UX_DESIGN.md §5
// is ~600 bytes). Anything bigger is suspicious; reject without parsing.
constexpr size_t kMaxManifestBytes = 64 * 1024;

// Build the manifest URL. origin may already include scheme; we normalize.
// We do NOT downgrade https:// to http://, and bare hosts default to https://
// so manifests can't be served over plaintext as a bug.
std::string buildManifestUrl(const std::string& origin) {
    if (origin.empty()) return std::string();
    std::string url;
    if (origin.find("://") != std::string::npos) {
        url = origin;
    } else {
        url = "https://" + origin;
    }
    // Strip trailing slash before appending path.
    if (!url.empty() && url.back() == '/') {
        url.pop_back();
    }
    url += "/.well-known/wallet-manifest.json";
    return url;
}

} // anonymous namespace

Manifest ManifestFetcher::Fetch(const std::string& origin) {
    Manifest result;  // valid=false by default

    const std::string url = buildManifestUrl(origin);
    if (url.empty()) return result;

    HttpResponse resp = SyncHttpClient::Get(url, kFetchTimeoutMs);
    if (!resp.success || resp.statusCode != 200) {
        // 404, timeout, network error — all treated identically.
        return result;
    }
    if (resp.body.size() > kMaxManifestBytes) {
        // Oversized body — bail without parsing to avoid DoS.
        return result;
    }

    return ParseFromJson(resp.body);
}

Manifest ManifestFetcher::ParseFromJson(const std::string& json) {
    Manifest m;  // valid=false by default

    try {
        auto j = nlohmann::json::parse(json);

        // Top-level must be an object. A top-level array or scalar (e.g. `[1,2,3]`,
        // `"hello"`, `42`) is not a valid manifest — bail out as invalid rather
        // than silently producing an empty one. nlohmann's contains() doesn't throw
        // on non-objects so this guard is needed explicitly.
        if (!j.is_object()) {
            return m;  // valid=false
        }

        // Top-level fields — all optional, all string except expiresAt.
        if (j.contains("version") && j["version"].is_string()) {
            m.version = j["version"].get<std::string>();
        }
        if (j.contains("name") && j["name"].is_string()) {
            m.name = j["name"].get<std::string>();
        }
        if (j.contains("description") && j["description"].is_string()) {
            m.description = j["description"].get<std::string>();
        }
        if (j.contains("iconUrl") && j["iconUrl"].is_string()) {
            m.iconUrl = j["iconUrl"].get<std::string>();
        }
        if (j.contains("expiresAt") && j["expiresAt"].is_number()) {
            m.expiresAt = j["expiresAt"].get<int64_t>();
        }

        // permissions object — drives the bundled connect prompt.
        if (j.contains("permissions") && j["permissions"].is_object()) {
            const auto& perms = j["permissions"];

            // protocols[]
            if (perms.contains("protocols") && perms["protocols"].is_array()) {
                for (const auto& p : perms["protocols"]) {
                    if (!p.is_object()) continue;
                    ManifestProtocol mp;
                    // protocolID is [securityLevel, name] per BRC-43.
                    if (p.contains("protocolID") && p["protocolID"].is_array()
                        && p["protocolID"].size() >= 2) {
                        const auto& arr = p["protocolID"];
                        if (arr[0].is_number()) {
                            int lvl = arr[0].get<int>();
                            if (lvl >= 0 && lvl <= 2) mp.securityLevel = lvl;
                        }
                        if (arr[1].is_string()) {
                            mp.name = arr[1].get<std::string>();
                        }
                    }
                    if (p.contains("keyID") && p["keyID"].is_string()) {
                        mp.keyId = p["keyID"].get<std::string>();
                    } else {
                        mp.keyId = "*";  // default wildcard per Phase 1.5
                    }
                    if (p.contains("purpose") && p["purpose"].is_string()) {
                        mp.purpose = p["purpose"].get<std::string>();
                    }
                    // Reject malformed: no name = nothing to grant.
                    if (!mp.name.empty()) {
                        m.protocols.push_back(std::move(mp));
                    }
                }
            }

            // baskets[]
            if (perms.contains("baskets") && perms["baskets"].is_array()) {
                for (const auto& b : perms["baskets"]) {
                    if (!b.is_object()) continue;
                    ManifestBasket mb;
                    if (b.contains("name") && b["name"].is_string()) {
                        mb.name = b["name"].get<std::string>();
                    }
                    if (b.contains("access") && b["access"].is_string()) {
                        mb.access = b["access"].get<std::string>();
                    } else {
                        mb.access = "read";
                    }
                    if (b.contains("purpose") && b["purpose"].is_string()) {
                        mb.purpose = b["purpose"].get<std::string>();
                    }
                    // Reject malformed: no name OR invalid access level.
                    if (!mb.name.empty()
                        && (mb.access == "read" || mb.access == "read_write")) {
                        m.baskets.push_back(std::move(mb));
                    }
                }
            }

            // certificates[]
            if (perms.contains("certificates") && perms["certificates"].is_array()) {
                for (const auto& c : perms["certificates"]) {
                    if (!c.is_object()) continue;
                    ManifestCertificate mc;
                    if (c.contains("type") && c["type"].is_string()) {
                        mc.type = c["type"].get<std::string>();
                    }
                    if (c.contains("fields") && c["fields"].is_array()) {
                        for (const auto& f : c["fields"]) {
                            if (f.is_string()) {
                                mc.fields.push_back(f.get<std::string>());
                            }
                        }
                    }
                    if (c.contains("purpose") && c["purpose"].is_string()) {
                        mc.purpose = c["purpose"].get<std::string>();
                    }
                    // Reject malformed: no type = nothing to grant.
                    if (!mc.type.empty()) {
                        m.certificates.push_back(std::move(mc));
                    }
                }
            }

            // spending (single object, optional)
            if (perms.contains("spending") && perms["spending"].is_object()) {
                const auto& s = perms["spending"];
                if (s.contains("perTransactionUsd") && s["perTransactionUsd"].is_number()) {
                    m.spending.perTransactionUsd = s["perTransactionUsd"].get<int64_t>();
                }
                if (s.contains("perSessionUsd") && s["perSessionUsd"].is_number()) {
                    m.spending.perSessionUsd = s["perSessionUsd"].get<int64_t>();
                }
                if (s.contains("purpose") && s["purpose"].is_string()) {
                    m.spending.purpose = s["purpose"].get<std::string>();
                }
            }

            // counterparties[]
            if (perms.contains("counterparties") && perms["counterparties"].is_array()) {
                for (const auto& cp : perms["counterparties"]) {
                    if (!cp.is_object()) continue;
                    ManifestCounterparty mcp;
                    if (cp.contains("type") && cp["type"].is_string()) {
                        mcp.type = cp["type"].get<std::string>();
                    }
                    if (cp.contains("counterparty") && cp["counterparty"].is_string()) {
                        mcp.counterparty = cp["counterparty"].get<std::string>();
                    }
                    if (cp.contains("purpose") && cp["purpose"].is_string()) {
                        mcp.purpose = cp["purpose"].get<std::string>();
                    }
                    // Need at least one identifier (type or counterparty) to be useful.
                    if (!mcp.type.empty() || !mcp.counterparty.empty()) {
                        m.counterparties.push_back(std::move(mcp));
                    }
                }
            }
        }

        // Mark valid even if some sections are empty — a manifest with just
        // `name` + `description` and no permissions still describes the dApp
        // and is a legitimate (if minimal) document.
        m.valid = true;
    } catch (...) {
        // nlohmann throws on malformed JSON. Swallow and return invalid.
        m.valid = false;
    }

    return m;
}

} // namespace hodos
