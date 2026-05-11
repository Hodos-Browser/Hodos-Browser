// PermissionEngine — central decision logic for wallet permission gates.
//
// Phase 1.5 Step 3. This class encapsulates the decision matrix described in
// `development-docs/Sigma-BRC121-Sprint/phase-1.5-brc100-surface-completion/
// PERMISSION_UX_DESIGN.md` §3 (Matrix C). The same logic that today lives
// inline in `AsyncWalletResourceHandler::Open()` will move here in Step 6;
// for Step 3 the engine exists, is unit-tested, and is consumed by the new
// sub-permission CRUD flows.
//
// Design intent:
//   - PURE LOGIC. No CEF dependencies, no globals, no HTTP. Takes plain data
//     in (`PermissionContext`) and returns a plain enum + metadata
//     (`PermissionDecision`). This is what makes it the canonical first
//     example for the project's C++ test infrastructure (per
//     `development-docs/UNIT_TESTING.md` §5).
//   - DEFENSE IN DEPTH. Rust handlers also check gates -- engine wraps the
//     C++ side. If the engine misroutes, Rust still rejects.
//   - HOT PATH. Cache-hit decisions must be sub-millisecond per design
//     decision #2 (PERMISSION_UX_DESIGN.md:429). Engine takes data, doesn't
//     fetch it.

#pragma once

#include <cstdint>
#include <string>

namespace hodos {

// The kind of BRC-100 / wallet call being gated. The engine uses this to
// classify the request into one of the Matrix C branches before consulting
// scope-specific data.
enum class PermissionCallKind {
    // Privacy perimeter: always-prompt unless persistently opted in
    IdentityKeyReveal,        // getPublicKey({identityKey:true}) from external domain
    CounterpartyKeyLinkage,   // /revealCounterpartyKeyLinkage
    SpecificKeyLinkage,       // /revealSpecificKeyLinkage
    SensitiveCertField,       // proveCertificate touching a high-sensitivity field

    // Scoped grants (Step 2 child tables)
    ProtocolUse,              // any call carrying protocolID + keyID
    BasketAccess,             // listOutputs / relinquishOutput with a basket
    CounterpartyUse,          // level-2 protocols with a specific counterparty

    // Payment / spending
    Payment,                  // createAction / acquireCertificate / sendMessage

    // Domain trust
    DomainTrust,              // first BRC-100 hit from a fresh origin

    // Cert disclosure (existing pattern, included for completeness)
    CertificateDisclosure,    // proveCertificate (non-sensitive fields)

    // Anything else under an approved domain that has no extra gate
    GenericApproved,
};

// Input to PermissionEngine::Decide. Caller assembles this from the live
// DomainPermissionCache row, SessionManager counters, and the request-specific
// scope (protocol/basket/counterparty/field name etc.). Plain data — trivially
// mockable in tests.
struct PermissionContext {
    PermissionCallKind callKind = PermissionCallKind::GenericApproved;

    // Domain-level state (from DomainPermissionCache::Permission).
    std::string trustLevel;              // "unknown" | "approved" | "blocked"
    int64_t perTxLimitCents = 0;
    int64_t perSessionLimitCents = 0;
    int64_t rateLimitPerMin = 0;
    int64_t maxTxPerSession = 0;
    bool identityKeyDisclosureAllowed = false;   // V17 column

    // Session counters (from SessionManager).
    int64_t sessionSpentCents = 0;
    int paymentRequestsThisMinute = 0;
    int paymentCountThisSession = 0;

    // Privacy-perimeter session opt-ins (in-memory caches in HttpRequestInterceptor.cpp).
    bool identityKeySessionOptIn = false;
    bool keyLinkageSessionOptIn = false;

    // Request-specific cost (computed by caller for Payment kind).
    int64_t requestedCents = 0;

    // Scoped-grant evaluation (filled in by caller for ProtocolUse/BasketAccess/CounterpartyUse).
    // The caller queries the V18 sub-permission tables before calling Decide,
    // and reports whether a matching active grant exists.
    bool scopedGrantExists = false;
};

// The engine's decision.
struct PermissionDecision {
    enum class Kind {
        Silent,     // Forward to Rust without a prompt
        Prompt,     // Fire a modal; wait for user approval before forwarding
        Deny,       // Reject the request outright
    };
    Kind kind = Kind::Prompt;

    // For Kind::Prompt — which prompt type to fire (drives the
    // `notification_browser_` overlay's type-dispatch in
    // BRC100AuthOverlayRoot.tsx). For Kind::Silent and Kind::Deny this is empty.
    // Canonical values: "domain_approval", "payment_confirmation",
    // "rate_limit_exceeded", "certificate_disclosure", "identity_key_reveal",
    // "key_linkage_reveal", "protocol_permission_prompt",
    // "counterparty_permission_prompt".
    std::string promptType;

    // Human-readable explanation for logs and (when Kind::Deny) the response body.
    std::string reason;
};

class PermissionEngine {
public:
    PermissionEngine() = default;

    // The single decision entry point. Pure function — same input always
    // produces the same output. Caller is responsible for fetching state into
    // PermissionContext before invoking.
    static PermissionDecision Decide(const PermissionContext& ctx);

private:
    // Branch helpers, factored out so each can be tested in isolation if
    // needed. Order matches Matrix C top-to-bottom.

    // Branch 1: privacy perimeter (identity-key, key-linkage, sensitive cert).
    // Always prompts unless a persistent or session-scoped opt-in is set for
    // the kind. Returns std::nullopt-equivalent via Kind::Silent only when an
    // opt-in is present and the call is one of the privacy-perimeter kinds.
    static PermissionDecision DecidePrivacyPerimeter(const PermissionContext& ctx);

    // Branch 2: domain trust. Unknown → domain_approval prompt; blocked → deny.
    static PermissionDecision DecideDomainTrust(const PermissionContext& ctx);

    // Branch 3: scoped grants for protocol / basket / counterparty.
    // If scopedGrantExists is true, returns Silent; otherwise prompts the
    // matching sub-permission type.
    static PermissionDecision DecideScopedGrant(const PermissionContext& ctx);

    // Branch 4: payment caps + rate limit + session-tx count.
    static PermissionDecision DecidePayment(const PermissionContext& ctx);
};

} // namespace hodos
