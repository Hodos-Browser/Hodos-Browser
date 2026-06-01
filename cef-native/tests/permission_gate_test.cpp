// permission_gate_test.cpp — unit tests for RunPermissionGate.
//
// Phase 2.5-B (sub-step 5.a). Verifies the gate runner faithfully dispatches
// the three PermissionDecision::Kind values via the right callback. Mocks the
// callbacks with std::function lambdas that record their invocations; no CEF
// dependency.
//
// Test discipline mirrors permission_engine_test.cpp:
//   - One concern per test, present-tense indicative naming.
//   - Build a fresh CallTracker per test; no shared state.
//   - Cover Silent / Prompt / Deny + the orthogonal concerns (no double-fire,
//     correct extraction of promptType/reason, null-callback safety).

#include "core/PermissionGate.h"
#include "core/PermissionEngine.h"

#include <gtest/gtest.h>

#include <string>

using hodos::GateCallbacks;
using hodos::GateDecision;
using hodos::PermissionCallKind;
using hodos::PermissionContext;
using hodos::RunPermissionGate;

namespace {

// Records every callback invocation so a test can assert exactly which path
// fired and with what arguments. Each counter is bumped on the matching
// callback; the strings capture the most recent payload.
struct CallTracker {
    int openModalCount = 0;
    int forwardCount = 0;
    int denyCount = 0;
    std::string lastPromptType;
    std::string lastExtraParams;
    std::string lastErrorJson;

    GateCallbacks build() {
        GateCallbacks cb;
        cb.openModal = [this](const std::string& promptType,
                              const std::string& extraParams) {
            ++openModalCount;
            lastPromptType = promptType;
            lastExtraParams = extraParams;
        };
        cb.forwardToWallet = [this]() {
            ++forwardCount;
        };
        cb.denyWithError = [this](const std::string& errorJson) {
            ++denyCount;
            lastErrorJson = errorJson;
        };
        return cb;
    }
};

// Build a baseline approved-domain context. Individual tests override only the
// fields they care about (trustLevel, callKind, scopedGrantExists, etc.).
PermissionContext baselineApproved() {
    PermissionContext ctx;
    ctx.trustLevel = "approved";
    ctx.perTxLimitCents = 100;
    ctx.perSessionLimitCents = 1000;
    ctx.rateLimitPerMin = 30;
    ctx.maxTxPerSession = 100;
    ctx.callKind = PermissionCallKind::GenericApproved;
    return ctx;
}

} // namespace

// ============================================================================
// Silent dispatch
// ============================================================================

TEST(PermissionGate, SilentDecisionFiresForwardToWalletOnly) {
    auto ctx = baselineApproved();
    // GenericApproved on an approved domain returns Silent (engine Branch 6).
    CallTracker t;
    auto result = RunPermissionGate(ctx, t.build());

    EXPECT_EQ(result.action, GateDecision::Action::Silent);
    EXPECT_EQ(t.forwardCount, 1);
    EXPECT_EQ(t.openModalCount, 0);
    EXPECT_EQ(t.denyCount, 0);
}

// ============================================================================
// Prompt dispatch
// ============================================================================

TEST(PermissionGate, PromptDecisionFiresOpenModalWithEngineProvidedType) {
    auto ctx = baselineApproved();
    // IdentityKeyReveal on an approved domain WITHOUT persistent grant or
    // session opt-in prompts identity_key_reveal (engine Branch 1).
    ctx.callKind = PermissionCallKind::IdentityKeyReveal;
    ctx.identityKeyDisclosureAllowed = false;
    ctx.identityKeySessionOptIn = false;

    CallTracker t;
    auto result = RunPermissionGate(ctx, t.build());

    EXPECT_EQ(result.action, GateDecision::Action::Prompt);
    EXPECT_EQ(result.promptType, "identity_key_reveal");
    EXPECT_EQ(t.openModalCount, 1);
    EXPECT_EQ(t.lastPromptType, "identity_key_reveal");
    // 5.a passes empty extraParams — branch-specific payloads land in 5.b+.
    EXPECT_EQ(t.lastExtraParams, "");
    EXPECT_EQ(t.forwardCount, 0);
    EXPECT_EQ(t.denyCount, 0);
}

TEST(PermissionGate, PaymentOverCapPromptsPaymentConfirmation) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::Payment;
    ctx.requestedCents = 200; // exceeds perTxLimitCents=100

    CallTracker t;
    auto result = RunPermissionGate(ctx, t.build());

    EXPECT_EQ(result.action, GateDecision::Action::Prompt);
    EXPECT_EQ(result.promptType, "payment_confirmation");
    EXPECT_EQ(t.lastPromptType, "payment_confirmation");
    EXPECT_EQ(t.openModalCount, 1);
    EXPECT_EQ(t.forwardCount, 0);
}

// ============================================================================
// Deny dispatch
// ============================================================================

TEST(PermissionGate, DenyDecisionFiresDenyWithErrorContainingReason) {
    auto ctx = baselineApproved();
    ctx.trustLevel = "blocked";
    ctx.callKind = PermissionCallKind::Payment;

    CallTracker t;
    auto result = RunPermissionGate(ctx, t.build());

    EXPECT_EQ(result.action, GateDecision::Action::Deny);
    EXPECT_EQ(t.denyCount, 1);
    EXPECT_EQ(t.openModalCount, 0);
    EXPECT_EQ(t.forwardCount, 0);
    // Engine's reason for blocked domain.
    EXPECT_NE(result.reason.find("blocked"), std::string::npos);
    // Error JSON contains the engine's reason verbatim and the status field.
    EXPECT_NE(t.lastErrorJson.find("blocked"), std::string::npos);
    EXPECT_NE(t.lastErrorJson.find("\"status\":\"error\""), std::string::npos);
}

// ============================================================================
// Null-callback safety
// ============================================================================

TEST(PermissionGate, NullCallbacksDoNotCrashOnAnyDecision) {
    GateCallbacks emptyCb; // all slots default-constructed (null)

    // Silent path
    {
        auto ctx = baselineApproved();
        auto result = RunPermissionGate(ctx, emptyCb);
        EXPECT_EQ(result.action, GateDecision::Action::Silent);
    }
    // Prompt path
    {
        auto ctx = baselineApproved();
        ctx.callKind = PermissionCallKind::IdentityKeyReveal;
        auto result = RunPermissionGate(ctx, emptyCb);
        EXPECT_EQ(result.action, GateDecision::Action::Prompt);
        EXPECT_EQ(result.promptType, "identity_key_reveal");
    }
    // Deny path
    {
        auto ctx = baselineApproved();
        ctx.trustLevel = "blocked";
        auto result = RunPermissionGate(ctx, emptyCb);
        EXPECT_EQ(result.action, GateDecision::Action::Deny);
        EXPECT_FALSE(result.reason.empty());
    }
}

// ============================================================================
// Result fidelity
// ============================================================================

TEST(PermissionGate, GateDecisionPromptTypeMatchesEngineDecisionPromptType) {
    auto ctx = baselineApproved();
    ctx.callKind = PermissionCallKind::Payment;
    ctx.requestedCents = 200; // over cap

    auto engineDecision = hodos::PermissionEngine::Decide(ctx);
    ASSERT_EQ(engineDecision.kind, hodos::PermissionDecision::Kind::Prompt);

    CallTracker t;
    auto gateDecision = RunPermissionGate(ctx, t.build());

    EXPECT_EQ(gateDecision.promptType, engineDecision.promptType);
    EXPECT_EQ(gateDecision.reason, engineDecision.reason);
}

TEST(PermissionGate, GateDecisionActionMatchesEngineDecisionKindAcrossAllThreeOutcomes) {
    // Silent
    {
        auto ctx = baselineApproved();
        auto eng = hodos::PermissionEngine::Decide(ctx);
        ASSERT_EQ(eng.kind, hodos::PermissionDecision::Kind::Silent);
        CallTracker t;
        auto gate = RunPermissionGate(ctx, t.build());
        EXPECT_EQ(gate.action, GateDecision::Action::Silent);
    }
    // Prompt
    {
        auto ctx = baselineApproved();
        ctx.callKind = PermissionCallKind::IdentityKeyReveal;
        auto eng = hodos::PermissionEngine::Decide(ctx);
        ASSERT_EQ(eng.kind, hodos::PermissionDecision::Kind::Prompt);
        CallTracker t;
        auto gate = RunPermissionGate(ctx, t.build());
        EXPECT_EQ(gate.action, GateDecision::Action::Prompt);
    }
    // Deny
    {
        auto ctx = baselineApproved();
        ctx.trustLevel = "blocked";
        auto eng = hodos::PermissionEngine::Decide(ctx);
        ASSERT_EQ(eng.kind, hodos::PermissionDecision::Kind::Deny);
        CallTracker t;
        auto gate = RunPermissionGate(ctx, t.build());
        EXPECT_EQ(gate.action, GateDecision::Action::Deny);
    }
}
