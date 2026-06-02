//! hodos_permission_engine — pure decision logic for the wallet permission engine.
//!
//! Phase 2.6-A.2: types in place; decide() body lands in 2.6-A.3; the 33 ported
//! tests land in 2.6-A.4.
//!
//! Architecture:
//!   - PURE LOGIC. No actix, no sqlite, no http, no AppState dependency.
//!   - Mirrors the C++ `PermissionEngine` at `cef-native/src/core/PermissionEngine.cpp`.
//!   - Becomes the canonical implementation during Phase 2.6; C++ engine deleted in 2.6-H.
//!
//! Public surface:
//!   - [`PermissionContext`] — engine input (caller builds from DB + caches + body)
//!   - [`PermissionDecision`] — engine output (one of Silent / Prompt / Deny)
//!   - [`decide()`] — single decision entry point (2.6-A.3, not yet implemented)
//!
//! See: `development-docs/Sigma-BRC121-Sprint/phase-2.6-engine-to-rust/`

pub mod context;
pub mod decision;

pub use context::{CallKind, PaymentScopeKind, PermissionContext, TrustLevel};
pub use decision::{EngineReason, PermissionDecision, PromptType};

/// The single decision entry point. Pure function — same input always produces
/// the same output. Caller is responsible for fetching state into
/// [`PermissionContext`] before invoking.
///
/// **2.6-A.2 stub.** Returns `Prompt(DomainApproval, NewDomainNoManifest)` for
/// all inputs — placeholder to keep the public API stable while 2.6-A.3 lands
/// the Matrix C branches. Real branch dispatch will replace this body in 2.6-A.3.
///
/// Until 2.6-A.3 ships, do NOT call this function from production code paths.
/// All callers must check the relevant `engine_rust_*` flag in `EngineFlags`
/// before reaching this code (which means the flag must be OFF in any binary
/// that gets shipped to anyone before 2.6-A.3 lands).
pub fn decide(_ctx: &PermissionContext) -> PermissionDecision {
    PermissionDecision::prompt(
        PromptType::DomainApproval,
        EngineReason::NewDomainNoManifest,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_returns_domain_approval_prompt() {
        // Sanity check that decide() can be called and returns something well-formed.
        // Real branch coverage lands in 2.6-A.4 (33 ported tests).
        let ctx = PermissionContext::default();
        let d = decide(&ctx);
        assert!(d.is_prompt());
    }

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
}
