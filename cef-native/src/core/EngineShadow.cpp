// EngineShadow.cpp — see include/core/EngineShadow.h for design intent.
//
// Phase 2.6-B.1: scaffolding only. No caller is wired. The function builds the
// POST envelope and dispatches to a worker thread; 2.6-B.3+ adds call sites
// alongside the existing RunPermissionGate sites in HttpRequestInterceptor.cpp.
//
// Wire shape (must stay in lockstep with the Rust handler at
// rust-wallet/src/permission_service/handlers.rs that lands in 2.6-B.2):
//
//   POST http://127.0.0.1:31301/engine/shadow-decide
//   Content-Type: application/json
//   {
//     "context": { /* PermissionContext, snake_case, see context.rs */ },
//     "cpp_decision": "silent" | "prompt" | "deny",
//     "cpp_prompt_type": "<string>" | null,
//     "cpp_reason": "<string>" | null
//   }
//
// Field-name casing matches Rust's serde defaults: PermissionContext fields
// are snake_case; CallKind uses PascalCase (#[serde(rename_all = "PascalCase")]);
// TrustLevel and PaymentScopeKind use lowercase.

#include "../../include/core/EngineShadow.h"

#include "../../include/core/Logger.h"
#include "../../include/core/SyncHttpClient.h"

#include "include/cef_task.h"
#include "include/base/cef_bind.h"
#include "include/base/cef_callback.h"
#include "include/wrapper/cef_closure_task.h"

#include <nlohmann/json.hpp>

#include <atomic>
#include <cstdlib>
#include <map>
#include <mutex>
#include <string>

// File-local logging macros. Logger::Log signature is (msg, level, source);
// source 2 = HTTP, matching HttpRequestInterceptor's existing channel since
// the shadow log is part of the wallet-routing subsystem.
#define LOG_DEBUG_SHADOW(msg) Logger::Log(msg, 0, 2)
#define LOG_WARNING_SHADOW(msg) Logger::Log(msg, 2, 2)

namespace hodos {

namespace {

constexpr const char* kShadowEndpoint =
    "http://127.0.0.1:31301/engine/shadow-decide";
constexpr int kShadowTimeoutMs = 500;
constexpr const char* kEnabledEnvVar = "HODOS_ENGINE_SHADOW_LOG";

// Lazy-init the enabled flag. Read the env var once on first call; cache the
// result for the lifetime of the process. Matches the spirit of HODOS_DEV in
// the dev-safeguard — no live reconfiguration mid-session.
bool isShadowEnabled() {
    static std::once_flag once;
    static std::atomic<bool> enabled{false};
    std::call_once(once, []() {
        const char* raw = std::getenv(kEnabledEnvVar);
        if (raw == nullptr) {
            return;
        }
        const std::string val(raw);
        enabled.store(val == "1" || val == "true" || val == "TRUE");
    });
    return enabled.load();
}

// CallKind → PascalCase string (matches #[serde(rename_all = "PascalCase")]
// on the Rust CallKind enum at hodos_permission_engine/src/context.rs:17).
//
// Intentional duplicate of HttpRequestInterceptor.cpp's anonymous-namespace
// callKindToString — that one is TU-private and not exposed. If a third
// caller ever appears, promote to PermissionEngine.h as a public utility.
const char* callKindString(PermissionCallKind k) {
    using K = PermissionCallKind;
    switch (k) {
        case K::IdentityKeyReveal:      return "IdentityKeyReveal";
        case K::CounterpartyKeyLinkage: return "CounterpartyKeyLinkage";
        case K::SpecificKeyLinkage:     return "SpecificKeyLinkage";
        case K::SensitiveCertField:     return "SensitiveCertField";
        case K::ProtocolUse:            return "ProtocolUse";
        case K::BasketAccess:           return "BasketAccess";
        case K::CounterpartyUse:        return "CounterpartyUse";
        case K::Payment:                return "Payment";
        case K::DomainTrust:            return "DomainTrust";
        case K::CertificateDisclosure:  return "CertificateDisclosure";
        case K::GenericApproved:        return "GenericApproved";
    }
    return "GenericApproved";
}

// C++ side stores raw trust-level strings from the DomainPermissionCache row;
// sanitize to one of the three values Rust's TrustLevel enum recognizes.
const char* trustLevelString(const std::string& s) {
    if (s == "approved") return "approved";
    if (s == "blocked")  return "blocked";
    return "unknown";
}

// Maps the C++ paymentScopeKindMissing free-form string to one of Rust's
// PaymentScopeKind variants. Returns nullptr for the no-missing-scope case so
// the caller can emit JSON null (which deserializes to Option::None on Rust).
const char* paymentScopeKindStringOrNull(const std::string& s) {
    if (s == "protocol")     return "protocol";
    if (s == "basket")       return "basket";
    if (s == "counterparty") return "counterparty";
    return nullptr;
}

const char* gateActionString(GateDecision::Action a) {
    switch (a) {
        case GateDecision::Action::Silent: return "silent";
        case GateDecision::Action::Prompt: return "prompt";
        case GateDecision::Action::Deny:   return "deny";
    }
    return "silent";
}

// Build the JSON envelope POST body. snake_case field names mirror the Rust
// PermissionContext struct (rust-wallet/crates/hodos_permission_engine/src/
// context.rs:97-140). May throw nlohmann::json::exception on internal errors;
// caller catches.
std::string buildEnvelope(const PermissionContext& ctx,
                          const GateDecision& cppResult) {
    nlohmann::json context;
    context["call_kind"]                       = callKindString(ctx.callKind);
    context["trust_level"]                     = trustLevelString(ctx.trustLevel);
    context["per_tx_limit_cents"]              = ctx.perTxLimitCents;
    context["per_session_limit_cents"]         = ctx.perSessionLimitCents;
    context["rate_limit_per_min"]              = ctx.rateLimitPerMin;
    context["max_tx_per_session"]              = ctx.maxTxPerSession;
    context["identity_key_disclosure_allowed"] = ctx.identityKeyDisclosureAllowed;
    context["session_spent_cents"]             = ctx.sessionSpentCents;
    context["payment_requests_this_minute"]    = ctx.paymentRequestsThisMinute;
    context["payment_count_this_session"]      = ctx.paymentCountThisSession;
    context["identity_key_session_opt_in"]     = ctx.identityKeySessionOptIn;
    context["key_linkage_session_opt_in"]      = ctx.keyLinkageSessionOptIn;
    context["requested_cents"]                 = ctx.requestedCents;
    context["bsv_price_available"]             = ctx.bsvPriceAvailable;
    context["scoped_grant_exists"]             = ctx.scopedGrantExists;

    if (const char* scope = paymentScopeKindStringOrNull(ctx.paymentScopeKindMissing)) {
        context["payment_scope_kind_missing"] = scope;
    } else {
        context["payment_scope_kind_missing"] = nullptr;
    }

    nlohmann::json envelope;
    envelope["context"]       = std::move(context);
    envelope["cpp_decision"]  = gateActionString(cppResult.action);
    envelope["cpp_prompt_type"] = cppResult.promptType.empty()
        ? nlohmann::json(nullptr)
        : nlohmann::json(cppResult.promptType);
    envelope["cpp_reason"] = cppResult.reason.empty()
        ? nlohmann::json(nullptr)
        : nlohmann::json(cppResult.reason);

    return envelope.dump();
}

// Worker-thread entry point. Pure side-effects: one HTTP call, response
// discarded. Any exception is swallowed so a transport failure can never
// surface to the wallet's critical path.
void postShadowOnWorker(std::string body) {
    try {
        std::map<std::string, std::string> headers;
        headers["Content-Type"] = "application/json";
        (void)SyncHttpClient::Post(kShadowEndpoint, body, headers, kShadowTimeoutMs);
    } catch (const std::exception& e) {
        LOG_DEBUG_SHADOW(std::string("🧪 [engine-shadow] POST threw: ") + e.what());
    } catch (...) {
        LOG_DEBUG_SHADOW("🧪 [engine-shadow] POST threw unknown exception");
    }
}

} // namespace

void SubmitShadowComparison(const PermissionContext& ctx,
                            const GateDecision& cppResult) {
    if (!isShadowEnabled()) {
        return;
    }

    std::string body;
    try {
        body = buildEnvelope(ctx, cppResult);
    } catch (const std::exception& e) {
        LOG_WARNING_SHADOW(std::string("🧪 [engine-shadow] envelope build failed: ") + e.what());
        return;
    } catch (...) {
        LOG_WARNING_SHADOW("🧪 [engine-shadow] envelope build threw unknown");
        return;
    }

    CefPostTask(TID_FILE_USER_BLOCKING,
                base::BindOnce(&postShadowOnWorker, std::move(body)));
}

} // namespace hodos
