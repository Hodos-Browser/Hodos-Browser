//! Context builder — assembles a `PermissionContext` from AppState + request data.
//!
//! Phase 2.6-A.5 status: **placeholder** for `build_context`. Real per-CallKind
//! construction lands in 2.6-C (privacy perimeter) through 2.6-G (domain trust).
//! Each sub-phase adds the body-parsing and DB-reading logic for its CallKind class.
//!
//! Phase 2.6-C.1: `sensitive_cert_fields` sub-module landed. Pure classifier
//! that mirrors `cef-native/include/core/SensitiveCertFields.h` 1:1. Used to
//! route /proveCertificate requests with sensitive fields to
//! `CallKind::SensitiveCertField` (always-prompt). Dormant in the live wallet
//! handlers until 2.6-C.2 wires the Rust prove_certificate path through
//! `PermissionService::decide()`.

use hodos_permission_engine::{CallKind, PermissionContext, TrustLevel};

pub mod sensitive_cert_fields;

/// Inputs to `build_context`. Carries what the request handler already has —
/// the domain, the endpoint, the parsed body (as raw bytes or a serde::Value),
/// and the CallKind class the handler has already classified.
///
/// 2.6-A.5 ships the type so the signature is stable; the real fields land
/// as each CallKind class is implemented.
#[derive(Debug, Clone)]
pub struct ContextBuilderInput<'a> {
    pub domain: &'a str,
    pub endpoint: &'a str,
    pub call_kind: CallKind,
    pub body_bytes: &'a [u8],
}

/// Build a `PermissionContext` from the request + AppState.
///
/// **2.6-A.5 placeholder:** returns a default context with `call_kind` and
/// `trust_level` populated from the input. Does NOT yet read the live
/// `domain_permissions` row, sub-permission tables, BSV price cache, or
/// session counters — those land in 2.6-C through 2.6-G per CallKind.
///
/// This function will eventually take an `&AppState` so it can read those
/// caches. The signature is decoupled from AppState in 2.6-A.5 to avoid
/// circular module dependencies before the wiring lands in 2.6-A.6.
pub fn build_context(input: &ContextBuilderInput<'_>) -> PermissionContext {
    // Placeholder: trust level is Unknown until DB read lands. That means
    // every request through this path would prompt for domain approval —
    // which is correct behavior for the dormant module (nothing reaches
    // this path in production until flags flip in 2.6-C+).
    PermissionContext {
        call_kind: input.call_kind,
        trust_level: TrustLevel::Unknown,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_returns_unknown_trust() {
        let input = ContextBuilderInput {
            domain: "example.com",
            endpoint: "/createAction",
            call_kind: CallKind::Payment,
            body_bytes: b"{}",
        };
        let ctx = build_context(&input);
        assert_eq!(ctx.call_kind, CallKind::Payment);
        assert_eq!(ctx.trust_level, TrustLevel::Unknown);
        // bsv_price_available default preserved.
        assert!(ctx.bsv_price_available);
    }
}
