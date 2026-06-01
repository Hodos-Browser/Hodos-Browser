// PermissionGate — reusable gate runner that wraps PermissionEngine::Decide.
//
// Phase 2.5-B (sub-step 5.a). This is the shared seam between the legacy HTTP
// interception path (AsyncWalletResourceHandler::Open in HttpRequestInterceptor.cpp)
// and the new IPC bridge path (wallet_call handler in simple_handler.cpp). For
// 5.a only the scaffolding lands — no caller invokes RunPermissionGate yet. The
// approved-trust cascade in Open() will migrate one branch at a time in 5.b
// through 5.f.
//
// Design intent (locked in PHASE_2_5_IPC_REFACTOR.md "Decisions" section):
//   - Pure data in, decision out + side effects via callbacks. Stateless.
//   - Wraps PermissionEngine::Decide() unchanged. The engine owns the decision;
//     this layer owns the side-effect dispatch.
//   - No CEF dependencies — same purity invariant as PermissionEngine, so this
//     file can be linked into hodos_tests with mock callbacks.
//   - Forward-compatible with Phase 2.6 (engine-to-Rust migration). When the
//     engine moves to Rust, RunPermissionGate's body changes to POST/await Rust
//     and translate 200/202/403 into Silent/Prompt/Deny — the call site and
//     callback set stay the same.
//
// Callback set will grow across 5.b-5.f as each branch migrates. The minimum
// viable set for 5.a is openModal / forwardToWallet / denyWithError — the
// three actions one of which fires for every PermissionDecision::Kind. Later
// sub-steps add slots for header injection (5.c/5.d), payment auto-approve
// flag setup (5.b), and session counter increments (5.b). Each addition is
// gated by an actual caller needing it, not speculation.

#pragma once

// Co-located header in include/core/; same-directory quote-include works for
// both the main shell build (no -I../include needed) and the test build (which
// adds -I../include via tests/CMakeLists.txt).
#include "PermissionEngine.h"

#include <functional>
#include <string>

namespace hodos {

// Side-effect callbacks fired by RunPermissionGate based on the engine's decision.
// HTTP path and IPC path build different callback sets; each slot encapsulates
// the per-path mechanics (CefPostTask, PendingRequestManager, handler-state
// updates) so the gate runner stays CEF-agnostic.
//
// All slots are optional — RunPermissionGate skips invocation if a slot is
// unset. Callers should set every slot they expect to fire for the call kinds
// they support.
struct GateCallbacks {
    // Fired when the engine returns Kind::Prompt. The caller's implementation
    // is responsible for the full modal-open cluster on its path:
    //   HTTP path: PendingRequestManager::addRequest + CreateNotificationOverlayTask
    //              + postAuthTimeout
    //   IPC path  (Commit 6):  same plus PendingAuthRequest::resumeKind=kIpcResponse
    // extraParams is a URL-style query suffix the React modal reads; 5.a leaves
    // this empty because no branch has migrated yet. 5.b onward will pass the
    // branch-specific payload (satoshis/cents for payment, etc.).
    std::function<void(const std::string& promptType, const std::string& extraParams)> openModal;

    // Fired when the engine returns Kind::Silent. The caller forwards the
    // original request to the wallet immediately.
    //   HTTP path: handle_request = true; CefPostTask(TID_IO, StartAsyncHTTPRequestTask)
    //   IPC path  (Commit 6):  SyncHttpClient::Post + wallet_response IPC
    std::function<void()> forwardToWallet;

    // Fired when the engine returns Kind::Deny. errorJson is a pre-formatted
    // JSON error body the caller surfaces to the renderer.
    //   HTTP path: onHTTPResponseReceived(errorJson) + handle_request = true
    //   IPC path  (Commit 6):  wallet_response IPC with ok=false + errorJson
    std::function<void(const std::string& errorJson)> denyWithError;
};

// Outcome of a RunPermissionGate call. Mirrors PermissionDecision but uses a
// gate-runner-local enum so the public surface doesn't leak PermissionDecision
// internals. promptType + reason are pass-through from the engine.
struct GateDecision {
    enum class Action {
        Silent,   // forwardToWallet was invoked (or would have been if set)
        Prompt,   // openModal was invoked
        Deny,     // denyWithError was invoked
    };
    Action action = Action::Prompt;
    std::string promptType;   // populated when action == Prompt
    std::string reason;       // engine's human-readable reason (empty when Silent)
};

// Run the permission gate for an approved-trust wallet request.
//
// Pre-conditions (caller must verify before invoking):
//   - Request is from a non-internal origin (NOT 127.0.0.1*, NOT localhost*)
//   - Wallet exists
//   - DomainPermissionCache returned trustLevel == "approved"
//   - PermissionContext.callKind was populated by classifyCallKind / buildPermissionContext
//
// What RunPermissionGate does:
//   1. Calls PermissionEngine::Decide(ctx) — engine is unchanged.
//   2. Dispatches via callbacks based on PermissionDecision::Kind:
//      - Silent → cb.forwardToWallet()
//      - Prompt → cb.openModal(promptType, "")
//      - Deny   → cb.denyWithError(error JSON)
//   3. Returns a GateDecision so the caller knows which path fired (e.g. for
//      logging or extra bookkeeping not modeled by callbacks yet).
//
// What RunPermissionGate does NOT do (5.a scope — additions deferred to 5.b+):
//   - Build per-branch extraParams (payment cents, scope name, etc.)
//   - Inject silent-approve headers (X-Identity-Key-Approved, X-Key-Linkage-Approved)
//   - Increment session counters on silent payment approve
//   - Record preCalculatedCents on the calling handler
GateDecision RunPermissionGate(const PermissionContext& ctx,
                               const GateCallbacks& cb);

} // namespace hodos
