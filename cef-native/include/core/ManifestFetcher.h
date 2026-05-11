// ManifestFetcher — fetches and parses wallet-manifest.json from dApp origins.
//
// Phase 1.5 Step 4. Stand-alone fetcher with no consumer yet; Step 5 wires it
// into `HttpRequestInterceptor::Open()` at the unknown-trust branch (see
// Phase 1.5 README "Manifest integration design" for the three-mode flow).
//
// Design intent:
//   - PURE LOGIC where possible. `ParseFromJson` is a pure function suitable
//     for unit testing without network. Network-touching `Fetch` is a thin
//     wrapper around SyncHttpClient.
//   - LENIENT PARSE. Forward-compatible: unknown JSON fields are ignored, not
//     errors. Missing optional fields fall back to safe defaults. Malformed
//     JSON returns an invalid Manifest (no throw).
//   - DEFENSIVE NETWORK. Hard 3-second timeout, 64 KB size cap. Hostile
//     servers can't hang the user or exhaust memory.
//   - NEVER THROWS. Any failure path returns `Manifest{valid=false}`.
//
// Manifest format per development-docs/Sigma-BRC121-Sprint/
//   phase-1.5-brc100-surface-completion/PERMISSION_UX_DESIGN.md §5.

#pragma once

#include <cstdint>
#include <string>
#include <vector>

namespace hodos {

// One entry in `permissions.protocols[]`.
// dApp says: "we want to use protocolID [level, name] under keyID for <purpose>."
struct ManifestProtocol {
    int securityLevel = 2;    // 0, 1, or 2 per BRC-43
    std::string name;         // e.g. "1sat ordinal", "messagebox"
    std::string keyId;        // "*" = wildcard (default)
    std::string purpose;      // plain-language UI string
};

// One entry in `permissions.baskets[]`.
struct ManifestBasket {
    std::string name;         // e.g. "1sat-ordinals"
    std::string access;       // "read" | "read_write"
    std::string purpose;      // plain-language UI string
};

// One entry in `permissions.certificates[]`.
struct ManifestCertificate {
    std::string type;         // certificate type URI/identifier
    std::vector<std::string> fields;  // field names the dApp wants to read
    std::string purpose;
};

// `permissions.spending` — optional, advisory caps the dApp prefers.
// Wallet still enforces its own global default limits if the user doesn't
// explicitly accept these.
struct ManifestSpending {
    int64_t perTransactionUsd = 0;   // 0 = unset
    int64_t perSessionUsd = 0;       // 0 = unset
    std::string purpose;
};

// One entry in `permissions.counterparties[]`.
// Either a specific counterparty pubkey hex OR a category type the dApp
// will introduce later (e.g., "list-1sat-marketplace" = "anyone in the
// 1sat marketplace listings"). Step 5's UI handles both shapes.
struct ManifestCounterparty {
    std::string type;         // category (optional)
    std::string counterparty; // hex pubkey (optional)
    std::string purpose;
};

// Top-level manifest. `valid == false` means fetch or parse failed; consumers
// should fall back to the existing per-call prompt flow.
struct Manifest {
    bool valid = false;
    std::string version;
    std::string name;
    std::string description;
    std::string iconUrl;
    int64_t expiresAt = 0;    // optional manifest-self-expiry (server-set)

    std::vector<ManifestProtocol> protocols;
    std::vector<ManifestBasket> baskets;
    std::vector<ManifestCertificate> certificates;
    ManifestSpending spending;
    std::vector<ManifestCounterparty> counterparties;
};

class ManifestFetcher {
public:
    // Fetch + parse the manifest at https://<origin>/.well-known/wallet-manifest.json.
    // Returns Manifest{valid=false} on any failure path:
    //   - non-2xx HTTP response (404 is the common case for manifest-less sites)
    //   - timeout (3 seconds hard cap)
    //   - response body > 64 KB
    //   - malformed JSON
    //   - JSON with no recognisable permission entries
    //
    // Origin may be either a bare host ("app.com") or full URL ("https://app.com").
    // Bare hosts are prefixed with https://. http:// and explicit schemes are honored as-is.
    static Manifest Fetch(const std::string& origin);

    // Pure parse — exposed for unit tests. No network, no I/O. Same lenient
    // failure semantics as Fetch.
    static Manifest ParseFromJson(const std::string& json);
};

} // namespace hodos
