// permission_engine_test.cpp — unit tests for the Hodos PermissionEngine.
//
// First C++ test in the project; designed as a reference example per
// development-docs/UNIT_TESTING.md §5.
//
// Test discipline:
//   - One concern per test, named in present-tense indicative form
//     (e.g. "ApprovedDomainWithinCapsIsSilent" not "TestPaymentApproval").
//   - Build a fresh PermissionContext per test; no shared fixture state.
//   - Cover every branch in Matrix C plus the obvious edge cases (caps at
//     boundary, rate limit exceeded, scoped grant present vs absent).

#include "core/PermissionEngine.h"

#include <gtest/gtest.h>

using hodos::PermissionCallKind;
using hodos::PermissionContext;
using hodos::PermissionDecision;
using hodos::PermissionEngine;
using Kind = PermissionDecision::Kind;

namespace {

// Helper: build a baseline "approved domain with healthy headroom" context.
// Individual tests override only the fields they care about.
PermissionContext baselineApproved() {
    PermissionContext ctx;
    ctx.trustLevel = "approved";
    ctx.perTxLimitCents = 100;
    ctx.perSessionLimitCents = 1000;
    ctx.rateLimitPerMin = 30;
    ctx.maxTxPerSession = 100;
    ctx.identityKeyDisclosureAllowed = false;
    ctx.sessionSpentCents = 0;
    ctx.paymentRequestsThisMinute = 0;
    ctx.paymentCountThisSession = 0;
    ctx.identityKeySessionOptIn = false;
    ctx.keyLinkageSessionOptIn = false;
    ctx.requestedCents = 0;
    ctx.scopedGrantExists = false;
    ctx.callKind = PermissionCallKind::GenericApproved;
    return ctx;
}

} // namespace

// ============================================================================
// Branch 1: Domain trust gates everything else
// ============================================================================

TEST(PermissionEngine, BlockedDomainAlwaysDeniesRegardlessOfCallKind) {
    auto ctx = baselineApproved();
    ctx.trustLevel = "blocked";
    ctx.callKind = PermissionCallKind::Payment;
    ctx.requestedCents = 10; // Well within caps — irrelevant for blocked

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Deny);
    EXPECT_EQ(d.reason, "domain is blocked");
}

TEST(PermissionEngine, UnknownDomainPromptsForDomainApproval) {
    auto ctx = baselineApproved();
    ctx.trustLevel = "unknown";
    ctx.callKind = PermissionCallKind::Payment;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "domain_approval");
}

TEST(PermissionEngine, EmptyTrustLevelTreatedAsUnknown) {
    auto ctx = baselineApproved();
    ctx.trustLevel.clear();
    ctx.callKind = PermissionCallKind::GenericApproved;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "domain_approval");
}

// ============================================================================
// Branch 2: Privacy perimeter — identity key
// ============================================================================

TEST(PermissionEngine, IdentityKeyRevealPromptsByDefault) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::IdentityKeyReveal;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "identity_key_reveal");
}

TEST(PermissionEngine, IdentityKeyRevealSilentWhenPersistentlyApproved) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::IdentityKeyReveal;
    ctx.identityKeyDisclosureAllowed = true; // V17 column = 1

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Silent);
}

TEST(PermissionEngine, IdentityKeyRevealSilentWhenSessionOptIn) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::IdentityKeyReveal;
    ctx.identityKeySessionOptIn = true; // In-memory cache hit

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Silent);
}

// ============================================================================
// Branch 2: Privacy perimeter — key linkage
// ============================================================================

TEST(PermissionEngine, CounterpartyLinkagePromptsByDefault) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::CounterpartyKeyLinkage;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "key_linkage_reveal");
}

TEST(PermissionEngine, SpecificLinkagePromptsByDefault) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::SpecificKeyLinkage;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "key_linkage_reveal");
}

TEST(PermissionEngine, KeyLinkageSilentWhenSessionOptIn) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::SpecificKeyLinkage;
    ctx.keyLinkageSessionOptIn = true;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Silent);
}

// ============================================================================
// Branch 2: Privacy perimeter — sensitive cert field always prompts
// ============================================================================

TEST(PermissionEngine, SensitiveCertFieldAlwaysPromptsEvenWithOptIn) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::SensitiveCertField;
    // Even if every opt-in is true, sensitive cert fields ignore them.
    ctx.identityKeyDisclosureAllowed = true;
    ctx.identityKeySessionOptIn = true;
    ctx.keyLinkageSessionOptIn = true;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "certificate_disclosure");
}

// ============================================================================
// Branch 3: Scoped grants — protocol / basket / counterparty
// ============================================================================

TEST(PermissionEngine, ProtocolUseSilentWhenScopedGrantExists) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::ProtocolUse;
    ctx.scopedGrantExists = true;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Silent);
}

TEST(PermissionEngine, ProtocolUsePromptsWhenNoScopedGrant) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::ProtocolUse;
    ctx.scopedGrantExists = false;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "protocol_permission_prompt");
}

TEST(PermissionEngine, BasketAccessPromptsWhenNoScopedGrant) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::BasketAccess;
    ctx.scopedGrantExists = false;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "basket_permission_prompt");
}

TEST(PermissionEngine, CounterpartyUseSilentWhenGrantExists) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::CounterpartyUse;
    ctx.scopedGrantExists = true;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Silent);
}

// ============================================================================
// Branch 4: Payment caps
// ============================================================================

TEST(PermissionEngine, PaymentWithinAllCapsIsSilent) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::Payment;
    ctx.requestedCents = 50;       // under $1/tx cap
    ctx.sessionSpentCents = 100;   // way under $10/session cap
    ctx.paymentRequestsThisMinute = 5;
    ctx.paymentCountThisSession = 10;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Silent);
}

TEST(PermissionEngine, PaymentExceedingPerTxCapPromptsConfirmation) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::Payment;
    ctx.requestedCents = 200; // exceeds $1/tx cap

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "payment_confirmation");
}

TEST(PermissionEngine, PaymentExceedingPerSessionCapPromptsConfirmation) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::Payment;
    ctx.requestedCents = 50;
    ctx.sessionSpentCents = 980; // 980 + 50 = 1030 > 1000 cap

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "payment_confirmation");
}

TEST(PermissionEngine, PaymentExceedingRateLimitPromptsRateLimit) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::Payment;
    ctx.requestedCents = 50;
    ctx.paymentRequestsThisMinute = 30; // at limit

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "rate_limit_exceeded");
}

TEST(PermissionEngine, PaymentAtSessionTxCountPromptsRateLimit) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::Payment;
    ctx.requestedCents = 50;
    ctx.paymentCountThisSession = 100; // at maxTxPerSession

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "rate_limit_exceeded");
}

TEST(PermissionEngine, PaymentExactlyAtPerTxCapIsSilent) {
    // Boundary: requestedCents == perTxLimitCents should be allowed.
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::Payment;
    ctx.requestedCents = 100; // exactly at cap

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Silent);
}

TEST(PermissionEngine, PaymentPriceUnavailablePromptsConfirmation) {
    // BSV price cache cold / network down — engine cannot trust cap math
    // (requestedCents would be 0 even for a real spend). Prompt so the user
    // sees the satoshi amount.
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::Payment;
    ctx.bsvPriceAvailable = false;
    ctx.requestedCents = 0; // caller couldn't convert satoshis → cents

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "payment_confirmation");
    // Reason is human-readable; checking presence rather than exact match.
    EXPECT_NE(d.reason.find("price unavailable"), std::string::npos);
}

TEST(PermissionEngine, PaymentPriceAvailableWithZeroCentsStillSilent) {
    // Defensive: a free output (satoshis=0 → cents=0) should NOT be blocked
    // by the price-unavailable check. The new branch only fires when price
    // is unavailable AND requestedCents would otherwise be 0 as a proxy for
    // "we couldn't convert."
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::Payment;
    ctx.bsvPriceAvailable = true; // price IS available
    ctx.requestedCents = 0;       // genuine zero-cost payment

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Silent);
}

// ============================================================================
// Branch 5: Cert disclosure (non-sensitive fields)
// ============================================================================

TEST(PermissionEngine, CertDisclosureSilentWhenAllFieldsPreApproved) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::CertificateDisclosure;
    ctx.scopedGrantExists = true;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Silent);
}

TEST(PermissionEngine, CertDisclosurePromptsWhenFieldsUnapproved) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::CertificateDisclosure;
    ctx.scopedGrantExists = false;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "certificate_disclosure");
}

// ============================================================================
// Branch 6: Generic approved-domain calls — silent fall-through
// ============================================================================

TEST(PermissionEngine, GenericApprovedCallIsSilent) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::GenericApproved;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Silent);
}

// ============================================================================
// Branch ordering: blocked domain wins over privacy perimeter
// ============================================================================

TEST(PermissionEngine, BlockedDomainWinsOverIdentityKeyOptIn) {
    auto ctx = baselineApproved();
    ctx.trustLevel = "blocked";
    ctx.callKind = PermissionCallKind::IdentityKeyReveal;
    ctx.identityKeyDisclosureAllowed = true; // would normally make it Silent

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Deny);
}

TEST(PermissionEngine, UnknownDomainWinsOverPrivacyPerimeter) {
    // First-visit identity-key request hits domain_approval, not identity_key_reveal.
    auto ctx = baselineApproved();
    ctx.trustLevel = "unknown";
    ctx.callKind = PermissionCallKind::IdentityKeyReveal;

    auto d = PermissionEngine::Decide(ctx);
    EXPECT_EQ(d.kind, Kind::Prompt);
    EXPECT_EQ(d.promptType, "domain_approval");
}
