// ManifestFetcher — parses a dApp's wallet manifest into the struct the
// connect-bundle modal renders.
//
// ⚠️ TWO-LAYER PARSER. `rust-wallet/src/manifest.rs :: parse_manifest` is the
// other half. Rust fetches the bytes and embeds them verbatim in its 202
// PENDING envelope; `HttpRequestInterceptor.cpp :: tryHandlePendingResponse`
// re-parses those same bytes HERE to build the modal. So the count in the
// "📦 Triggering manifest_connect_bundle … (N protocols…)" log line comes from
// THIS parse — a Rust-only fix cannot move it (`P0.8-A1`). **Keep the two in
// step; a change to one without the other is a bug.**
//
// Design intent:
//   - PURE LOGIC where possible. `ParseFromJson` is a pure function suitable
//     for unit testing without network. Network-touching `Fetch` is a thin
//     wrapper around SyncHttpClient.
//   - FORWARD-COMPATIBLE, BUT NOT LENIENT ABOUT PERMISSIONS. Unknown JSON
//     fields are ignored and malformed array entries are dropped, but see
//     `valid` below — a document declaring no recognised permission is NOT a
//     manifest for our purposes.
//   - DEFENSIVE NETWORK. Hard 3-second timeout, 64 KB size cap.
//   - BOUNDED PARSE. The 64 KB fetch cap is not a parse cap: 64 KB of JSON can
//     still declare thousands of entries, every one of which the modal renders.
//     Arrays are length-capped and strings truncated (`kMax*` below).
//   - NEVER THROWS. Any failure path returns `Manifest{valid=false}`.
//
// Shapes understood, in precedence order:
//   1. `metanet.groupPermissions`  — BRC-73, the canonical namespace
//   2. `babbage.groupPermissions`  — deprecated legacy namespace
//   3. top-level `permissions`     — ours, PERMISSION_UX_DESIGN.md §5

#pragma once

#include <cstdint>
#include <string>
#include <vector>

namespace hodos {

// ── Parse bounds. Mirrors the MAX_* constants in `manifest.rs`. ──
// Max entries kept per permission array (a hostile manifest can declare
// thousands; 64 is ~16x the largest real one observed).
constexpr size_t kMaxManifestEntries = 64;
// Max certificate fields kept per certificate entry.
constexpr size_t kMaxManifestFields = 64;
// Max length of a free-text display string (name / description / purpose).
constexpr size_t kMaxManifestStr = 512;
// Max length of an identifier-ish string (protocol name, basket, cert type,
// counterparty / verifier pubkey hex — a compressed pubkey is 66 chars).
constexpr size_t kMaxManifestIdStr = 256;

// Which declaration shape the permissions came from. Display + diagnostics
// only — never a decision input.
inline constexpr const char* kSourceMetanet     = "metanet";
inline constexpr const char* kSourceBabbage     = "babbage";
inline constexpr const char* kSourceHodosLegacy = "hodos-legacy";

// One declared protocol permission.
struct ManifestProtocol {
    int securityLevel = 2;    // 0, 1, or 2 per BRC-43
    std::string name;         // e.g. "identity key retrieval", "server hmac"
    std::string keyId;        // "*" = wildcard (BRC-73 has no keyID concept)
    // BRC-73 `counterparty`: 'self', 'anyone', or a compressed pubkey. Empty =
    // unspecified. Level 1 MAY omit it. A Level 2 entry that omits it is KEPT
    // and shown as "any counterparty" rather than dropped — dropping would
    // understate what the site asked for, which is the defect Phase 0.8 closes.
    std::string counterparty;
    std::string purpose;      // plain-language UI string (untrusted)
};

// One declared basket permission.
struct ManifestBasket {
    std::string name;         // e.g. "1sat-ordinals"
    std::string access;       // "read" | "read_write"
    std::string purpose;      // plain-language UI string (untrusted)
};

// One declared certificate permission.
struct ManifestCertificate {
    std::string type;                 // BRC-52 certificate type
    std::vector<std::string> fields;  // fields the dApp wants revealed
    // BRC-73 `verifierPublicKey` — who receives the disclosed fields. BRC-116
    // §4.4 scopes certificate access by type + verifier + fields, so the user
    // is shown the verifier. Display only: `cert_field_permissions` has no
    // verifier column and adding one would be a schema change.
    std::string verifierPublicKey;
    std::string purpose;
};

// Declared spending. Two different things live here, deliberately apart.
struct ManifestSpending {
    // Our legacy shape only. Same unit and period as
    // `domain_permissions.per_tx_limit_cents`, so these are the only declared
    // figures that may ever pre-fill a limit field — and then only behind the
    // user's `default_prefill_from_manifest` opt-in.
    int64_t perTransactionUsd = 0;   // 0 = unset
    int64_t perSessionUsd = 0;       // 0 = unset
    // 🚨 BRC-73 `spendingAuthorization.amount` — the authorized MONTHLY spend
    // limit in SATOSHIS. Neither the unit nor the period matches anything we
    // enforce, and we have no monthly concept at all. DISPLAYED as the site's
    // request, NEVER written (`R-CAPS`, `P0.8-A5`). Must never be mapped onto
    // the two fields above.
    int64_t monthlySatoshis = 0;     // 0 = unset
    std::string purpose;
};

// One declared counterparty (our legacy shape only). Either a specific
// counterparty pubkey hex OR a category type the dApp will introduce later.
//
// ⚠️ NOT populated from BRC-116's `metanet.counterpartyPermissions`. That
// declares Level-2 protocol NAMES for PACT (the concrete counterparty arrives
// at request time); this list holds specific pubkeys that become
// `domain_counterparty_permissions` rows. Folding one into the other would
// write a grant nobody asked for. PACT is out of scope for Phase 0.8.
struct ManifestCounterparty {
    std::string type;         // category (optional)
    std::string counterparty; // hex pubkey (optional)
    std::string purpose;
};

// Top-level manifest.
//
// 🚨 `valid == true` means the document parsed as a JSON object AND declared at
// least one recognised permission. It used to be set "even if some sections are
// empty", which is precisely why a plain W3C web-app manifest produced a
// `manifest_connect_bundle` modal itemising nothing — see
// `development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/PHASE_CONTRACT.md` §1.
// The fail-loud fallback in `tryHandlePendingResponse` already existed; this is
// what lets it fire. ⛔ Do not relax this back to "parsed OK".
struct Manifest {
    bool valid = false;
    std::string version;
    std::string name;
    std::string description;
    std::string iconUrl;      // https:// only; anything else is dropped
    int64_t expiresAt = 0;    // optional manifest-self-expiry (server-set)
    std::string sourceNamespace;   // "metanet" | "babbage" | "hodos-legacy"
    std::string groupDescription;  // groupPermissions.description (untrusted)

    std::vector<ManifestProtocol> protocols;
    std::vector<ManifestBasket> baskets;
    std::vector<ManifestCertificate> certificates;
    ManifestSpending spending;
    std::vector<ManifestCounterparty> counterparties;

    // True iff at least one recognised permission was declared. This is what
    // `valid` is gated on.
    bool HasDeclaredPermissions() const {
        return !protocols.empty()
            || !baskets.empty()
            || !certificates.empty()
            || !counterparties.empty()
            || spending.perTransactionUsd > 0
            || spending.perSessionUsd > 0
            || spending.monthlySatoshis > 0;
    }
};

class ManifestFetcher {
public:
    // The locations a manifest may be served from, in the order they are tried:
    //   1. https://<origin>/manifest.json                        (BRC-73)
    //   2. https://<origin>/.well-known/wallet-manifest.json     (ours)
    //
    // Returns EMPTY when `origin` is not a usable bare authority. `R-PATH`: an
    // origin carrying a path, query, fragment, userinfo or interior whitespace
    // is REJECTED, never concatenated — BRC-116 §9.2 constrains retrieval to
    // the origin's own manifest resource. Scheme policy also follows §9.2:
    // https always, except an explicit http:// on a loopback host.
    //
    // Mirrors `manifest.rs :: manifest_urls`.
    static std::vector<std::string> ManifestUrls(const std::string& origin);

    // Fetch + parse, trying both locations. Returns Manifest{valid=false} on
    // every failure path: non-200, timeout, body > 64 KB, malformed JSON, a
    // 200 text/html SPA catch-all, or JSON declaring no recognised permission.
    //
    // ⛔ The HTTP status code is never trusted on its own — `200` does not mean
    // "manifest" (`P0.8-A7`; four of ten surveyed BSV properties return
    // `200 text/html` for both paths). Only a successful parse counts.
    //
    // NOTE: not on the production path. Rust owns the fetch since Phase 2.6-G.2
    // (`manifest.rs :: fetch_manifest`, called from
    // `permission_service/request_gate.rs`). Kept so the two layers stay
    // symmetrical and testable; its only C++ caller,
    // `HttpRequestInterceptor.cpp :: handleIpcUnknownTrust`, is dead code.
    static Manifest Fetch(const std::string& origin);

    // Pure parse — exposed for unit tests. No network, no I/O. THIS is the
    // production path: the interceptor re-parses Rust's embedded bytes with it.
    static Manifest ParseFromJson(const std::string& json);
};

} // namespace hodos
