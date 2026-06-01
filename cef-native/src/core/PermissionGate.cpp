// PermissionGate.cpp — see include/core/PermissionGate.h for design intent.
//
// Phase 2.5-B (sub-step 5.a) scaffolding implementation. No caller uses this
// yet — Open() in HttpRequestInterceptor.cpp keeps its inline cascade until
// 5.b migrates the first branch (payment).

#include "../../include/core/PermissionGate.h"

namespace hodos {

namespace {

// Build the standard JSON error envelope for Deny decisions. Matches the inline
// shape used at HttpRequestInterceptor.cpp's existing engine branches (e.g.
// L2065, L2117) so HTTP-path consumers don't see a format change when the
// branches migrate in 5.b-5.f.
std::string buildDenyJson(const std::string& reason) {
    return std::string("{\"error\":\"") + reason + "\",\"status\":\"error\"}";
}

} // namespace

GateDecision RunPermissionGate(const PermissionContext& ctx,
                               const GateCallbacks& cb) {
    const PermissionDecision decision = PermissionEngine::Decide(ctx);

    GateDecision result;
    result.promptType = decision.promptType;
    result.reason = decision.reason;

    switch (decision.kind) {
        case PermissionDecision::Kind::Silent:
            result.action = GateDecision::Action::Silent;
            if (cb.forwardToWallet) {
                cb.forwardToWallet();
            }
            break;

        case PermissionDecision::Kind::Prompt:
            result.action = GateDecision::Action::Prompt;
            if (cb.openModal) {
                // 5.a passes empty extraParams. 5.b onward will overload the
                // gate runner (or extend GateDecision) so branch-specific
                // payloads can be threaded through. For 5.a there are no
                // consumers, so this is unobservable.
                cb.openModal(decision.promptType, std::string());
            }
            break;

        case PermissionDecision::Kind::Deny:
            result.action = GateDecision::Action::Deny;
            if (cb.denyWithError) {
                cb.denyWithError(buildDenyJson(decision.reason));
            }
            break;
    }

    return result;
}

} // namespace hodos
