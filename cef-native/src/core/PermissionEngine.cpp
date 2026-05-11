// PermissionEngine — central decision logic for wallet permission gates.
//
// See PermissionEngine.h for design intent. This file is pure logic over
// PermissionContext / PermissionDecision; no CEF includes, no HTTP, no globals.
// That property is load-bearing for the unit tests in
// `cef-native/tests/permission_engine_test.cpp` — keep it pure.

#include "../../include/core/PermissionEngine.h"

namespace hodos {

// ---------------------------------------------------------------------------
// Branch helpers
// ---------------------------------------------------------------------------

PermissionDecision PermissionEngine::DecidePrivacyPerimeter(const PermissionContext& ctx) {
    PermissionDecision d;

    switch (ctx.callKind) {
        case PermissionCallKind::IdentityKeyReveal:
            if (ctx.identityKeyDisclosureAllowed || ctx.identityKeySessionOptIn) {
                d.kind = PermissionDecision::Kind::Silent;
                d.reason = "identity-key disclosure pre-approved for site";
            } else {
                d.kind = PermissionDecision::Kind::Prompt;
                d.promptType = "identity_key_reveal";
                d.reason = "identity-key request requires explicit user approval";
            }
            return d;

        case PermissionCallKind::CounterpartyKeyLinkage:
        case PermissionCallKind::SpecificKeyLinkage:
            if (ctx.keyLinkageSessionOptIn) {
                d.kind = PermissionDecision::Kind::Silent;
                d.reason = "key-linkage reveal pre-approved for session";
            } else {
                d.kind = PermissionDecision::Kind::Prompt;
                d.promptType = "key_linkage_reveal";
                d.reason = "key-linkage reveal requires explicit user approval";
            }
            return d;

        case PermissionCallKind::SensitiveCertField:
            // High-sensitivity cert fields ALWAYS prompt — no opt-out path in
            // Step 3. Step 5+ refines this with per-(domain, field) grants
            // via cert_field_permissions, but the privacy-perimeter floor
            // remains in place per design principle #1.
            d.kind = PermissionDecision::Kind::Prompt;
            d.promptType = "certificate_disclosure";
            d.reason = "sensitive cert field requires explicit user approval";
            return d;

        default:
            // Not a privacy-perimeter kind — caller should fall through to the
            // next branch. We signal "no decision yet" via Kind::Silent with
            // an empty reason; callers MUST consult callKind to know whether
            // to keep going.
            d.kind = PermissionDecision::Kind::Silent;
            d.reason.clear();
            return d;
    }
}

PermissionDecision PermissionEngine::DecideDomainTrust(const PermissionContext& ctx) {
    PermissionDecision d;
    if (ctx.trustLevel == "blocked") {
        d.kind = PermissionDecision::Kind::Deny;
        d.reason = "domain is blocked";
        return d;
    }
    if (ctx.trustLevel == "unknown" || ctx.trustLevel.empty()) {
        d.kind = PermissionDecision::Kind::Prompt;
        d.promptType = "domain_approval";
        d.reason = "fresh origin requires user approval before any wallet call";
        return d;
    }
    // "approved" or any other recognised allowed value — caller continues.
    d.kind = PermissionDecision::Kind::Silent;
    d.reason.clear();
    return d;
}

PermissionDecision PermissionEngine::DecideScopedGrant(const PermissionContext& ctx) {
    PermissionDecision d;
    if (ctx.scopedGrantExists) {
        d.kind = PermissionDecision::Kind::Silent;
        d.reason = "scoped grant covers this call";
        return d;
    }

    switch (ctx.callKind) {
        case PermissionCallKind::ProtocolUse:
            d.kind = PermissionDecision::Kind::Prompt;
            d.promptType = "protocol_permission_prompt";
            d.reason = "new protocol/keyID tuple requires user approval";
            return d;
        case PermissionCallKind::BasketAccess:
            d.kind = PermissionDecision::Kind::Prompt;
            d.promptType = "basket_permission_prompt";
            d.reason = "new basket access requires user approval";
            return d;
        case PermissionCallKind::CounterpartyUse:
            d.kind = PermissionDecision::Kind::Prompt;
            d.promptType = "counterparty_permission_prompt";
            d.reason = "new counterparty requires user approval";
            return d;
        default:
            // Not a scoped-grant kind — caller should fall through.
            d.kind = PermissionDecision::Kind::Silent;
            d.reason.clear();
            return d;
    }
}

PermissionDecision PermissionEngine::DecidePayment(const PermissionContext& ctx) {
    PermissionDecision d;

    if (ctx.callKind != PermissionCallKind::Payment) {
        d.kind = PermissionDecision::Kind::Silent;
        d.reason.clear();
        return d;
    }

    // Rate limit first — if exceeded, fire the rate-limit prompt regardless of cap.
    if (ctx.paymentRequestsThisMinute >= ctx.rateLimitPerMin && ctx.rateLimitPerMin > 0) {
        d.kind = PermissionDecision::Kind::Prompt;
        d.promptType = "rate_limit_exceeded";
        d.reason = "rate limit exceeded for site";
        return d;
    }

    // Max tx per session.
    if (ctx.paymentCountThisSession >= ctx.maxTxPerSession && ctx.maxTxPerSession > 0) {
        d.kind = PermissionDecision::Kind::Prompt;
        d.promptType = "rate_limit_exceeded";
        d.reason = "session transaction count exceeded";
        return d;
    }

    // Per-tx cap.
    if (ctx.requestedCents > ctx.perTxLimitCents) {
        d.kind = PermissionDecision::Kind::Prompt;
        d.promptType = "payment_confirmation";
        d.reason = "payment exceeds per-tx limit";
        return d;
    }

    // Cumulative session cap.
    if ((ctx.sessionSpentCents + ctx.requestedCents) > ctx.perSessionLimitCents) {
        d.kind = PermissionDecision::Kind::Prompt;
        d.promptType = "payment_confirmation";
        d.reason = "payment would exceed per-session limit";
        return d;
    }

    // Within all caps — auto-approve.
    d.kind = PermissionDecision::Kind::Silent;
    d.reason = "payment within all configured caps";
    return d;
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

PermissionDecision PermissionEngine::Decide(const PermissionContext& ctx) {
    // Branch order matches Matrix C top-to-bottom.

    // 1. Domain trust gates everything else: a blocked domain can't even prompt,
    //    and an unknown domain prompts for site approval before any other check.
    {
        auto d = DecideDomainTrust(ctx);
        if (d.kind == PermissionDecision::Kind::Deny ||
            (d.kind == PermissionDecision::Kind::Prompt && !d.promptType.empty())) {
            return d;
        }
        // Otherwise (approved trust) — fall through.
    }

    // 2. Privacy perimeter (identity-key, key-linkage, sensitive cert field).
    //    Privacy perimeter takes precedence over scoped/payment gates because
    //    a privacy-perimeter call MUST always prompt (or honor an explicit
    //    opt-in) regardless of any spending caps in play.
    switch (ctx.callKind) {
        case PermissionCallKind::IdentityKeyReveal:
        case PermissionCallKind::CounterpartyKeyLinkage:
        case PermissionCallKind::SpecificKeyLinkage:
        case PermissionCallKind::SensitiveCertField:
            return DecidePrivacyPerimeter(ctx);
        default:
            break;
    }

    // 3. Scoped grants — protocol/basket/counterparty.
    switch (ctx.callKind) {
        case PermissionCallKind::ProtocolUse:
        case PermissionCallKind::BasketAccess:
        case PermissionCallKind::CounterpartyUse:
            return DecideScopedGrant(ctx);
        default:
            break;
    }

    // 4. Payment caps.
    if (ctx.callKind == PermissionCallKind::Payment) {
        return DecidePayment(ctx);
    }

    // 5. Cert disclosure (non-sensitive). Existing behavior: prompt if cert
    //    fields aren't pre-approved; the caller resolves "pre-approved" via
    //    the existing cert_field_permissions table and signals it via
    //    scopedGrantExists.
    if (ctx.callKind == PermissionCallKind::CertificateDisclosure) {
        if (ctx.scopedGrantExists) {
            PermissionDecision d;
            d.kind = PermissionDecision::Kind::Silent;
            d.reason = "all requested cert fields pre-approved";
            return d;
        }
        PermissionDecision d;
        d.kind = PermissionDecision::Kind::Prompt;
        d.promptType = "certificate_disclosure";
        d.reason = "unapproved cert fields require user approval";
        return d;
    }

    // 6. Generic approved-domain call — silent forward.
    PermissionDecision d;
    d.kind = PermissionDecision::Kind::Silent;
    d.reason = "approved domain, no additional gate";
    return d;
}

} // namespace hodos
