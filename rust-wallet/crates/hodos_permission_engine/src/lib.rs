//! hodos_permission_engine — pure decision logic for the wallet permission engine.
//!
//! Phase 2.6-A.3: branch logic ported from C++. The full 33-test port lands in 2.6-A.4.
//!
//! Architecture:
//!   - PURE LOGIC. No actix, no sqlite, no http, no AppState dependency.
//!   - Mirrors the C++ `PermissionEngine` at `cef-native/src/core/PermissionEngine.cpp`.
//!   - Becomes the canonical implementation during Phase 2.6; C++ engine deleted in 2.6-H.
//!
//! Public surface:
//!   - [`PermissionContext`] — engine input (caller builds from DB + caches + body)
//!   - [`PermissionDecision`] — engine output (one of Silent / Prompt / Deny)
//!   - [`decide()`] — single decision entry point
//!
//! See: `development-docs/Sigma-BRC121-Sprint/phase-2.6-engine-to-rust/`

pub mod context;
pub mod decision;
mod matrix_c;

pub use context::{CallKind, PaymentScopeKind, PermissionContext, TrustLevel};
pub use decision::{EngineReason, PermissionDecision, PromptType};

/// The single decision entry point. Pure function — same input always produces
/// the same output. Caller is responsible for fetching state into
/// [`PermissionContext`] before invoking.
///
/// Branch order matches Matrix C top-to-bottom per
/// `PERMISSION_UX_DESIGN.md` §3:
///   1. Domain trust (blocked → Deny, unknown → Prompt, approved → continue)
///   2. Privacy perimeter (identity-key, key-linkage, sensitive cert)
///   3. Scoped grants (protocol, basket, counterparty)
///   4. Payment caps + scope-missing precedence
///   5. Cert disclosure (non-sensitive fields)
///   6. Generic approved → Silent
///
/// Translates `PermissionEngine::Decide` at
/// `cef-native/src/core/PermissionEngine.cpp:204-270`.
///
/// **2.6-A.3 status:** branch logic ported and exercised by ~16 light sanity
/// tests in `matrix_c::tests`. The full 33-test port (1:1 with
/// `cef-native/tests/permission_engine_test.cpp`) lands in 2.6-A.4. This
/// function should NOT be called from production code paths until the
/// relevant `engine_rust_*` flag in `EngineFlags` is set (all flags default
/// OFF in 2.6-A; no production path will reach here until at least 2.6-C).
pub fn decide(ctx: &PermissionContext) -> PermissionDecision {
    matrix_c::decide(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_api_surface_compiles() {
        // Compile-check that the re-exports are usable.
        let _ctx: PermissionContext = PermissionContext::default();
        let _kind: CallKind = CallKind::Payment;
        let _trust: TrustLevel = TrustLevel::Approved;
        let _scope: Option<PaymentScopeKind> = Some(PaymentScopeKind::Protocol);
        let _decision: PermissionDecision = PermissionDecision::silent(EngineReason::SilentWithinCaps);
        let _prompt: PromptType = PromptType::PaymentConfirmation;
    }

    #[test]
    fn entry_point_delegates_to_matrix_c() {
        // Verify decide() returns the same result as the underlying cascade.
        // Cascade-level coverage lives in matrix_c::tests.
        let ctx = PermissionContext {
            trust_level: TrustLevel::Blocked,
            ..Default::default()
        };
        assert!(decide(&ctx).is_deny());
    }
}
