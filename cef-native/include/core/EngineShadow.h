// EngineShadow — fire-and-forget shadow-comparison helper for Phase 2.6-B.
//
// Posts the C++ permission engine's decision + the original PermissionContext
// to the Rust permission service at POST /engine/shadow-decide on a worker
// thread. The Rust service runs its own engine against the same context and
// logs agreement / disagreement in the engine_shadow_log table for review.
//
// Hard invariants (LD5 of PHASE_2_6_ENGINE_TO_RUST.md):
//   1. The POST is fire-and-forget. C++ NEVER reads Rust's response.
//   2. The wallet's critical path latency is unchanged — the POST runs on
//      TID_FILE_USER_BLOCKING and the calling thread returns immediately.
//   3. If the Rust service is slow, down, or returns an error, the wallet
//      call still completes normally.
//   4. Gated by HODOS_ENGINE_SHADOW_LOG env var (read once on first call).
//      Default OFF — the seam stays dormant until 2.6-B.3+ wire callers and
//      a developer turns it on for dev-time validation.
//
// Lifecycle: deleted in 2.6-H alongside the C++ permission engine.

#pragma once

#include "PermissionEngine.h"  // hodos::PermissionContext
#include "PermissionGate.h"    // hodos::GateDecision

namespace hodos {

// Submit a shadow-comparison POST. Safe to call from any CEF thread.
//
// Pre-condition: cppResult MUST be the GateDecision returned by the
// RunPermissionGate call that consumed ctx. Calling without a matching pair
// pollutes the shadow log with garbage comparisons.
//
// Behavior:
//   - If HODOS_ENGINE_SHADOW_LOG is not set or != "1"/"true", returns
//     immediately (no allocation, no task post).
//   - Otherwise posts a TID_FILE_USER_BLOCKING task that builds a JSON
//     envelope and calls SyncHttpClient::Post with a short timeout. The
//     response body is read into HttpResponse and discarded.
//   - Any exception during JSON build or POST is caught and logged once at
//     DEBUG. Never propagates.
void SubmitShadowComparison(const PermissionContext& ctx,
                            const GateDecision& cppResult);

} // namespace hodos
