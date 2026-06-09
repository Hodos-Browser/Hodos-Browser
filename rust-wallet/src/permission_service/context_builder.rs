//! Context builder — assembles a `PermissionContext` from AppState + request data.
//!
//! Phase 2.6-C.1: `sensitive_cert_fields` sub-module — pure classifier that
//! mirrors `cef-native/include/core/SensitiveCertFields.h` 1:1.
//!
//! Phase 2.6-C.2: `build_privacy_perimeter_context` — reads the live
//! `domain_permissions` row (V17 column) to populate the per-call
//! PermissionContext for the 4 privacy-perimeter CallKinds.
//!
//! Phase 2.6-C.4 follow-up: session opt-in flags are now sourced from
//! `PermissionService`'s session caches (migrated from C++'s
//! IdentityKeyApprovalCache + KeyLinkageApprovalCache). Caller threads them
//! into `build_privacy_perimeter_context`; C++ keeps its own caches updated
//! independently and additionally fires `POST /wallet/session-approve` after
//! the user approves an identity_key / key_linkage modal so the Rust side
//! sees Silent on subsequent calls from the same origin.
//!
//! Phase 2.6-D: `build_scoped_grant_context` — populates `scoped_grant_exists`
//! by querying the V18 sub-permission tables. Used by the
//! `dispatch_scoped_grant` request gate to drive ProtocolUse / BasketAccess /
//! CounterpartyUse engine decisions.
//!
//! Other per-CallKind construction (scoped grants, payment, cert disclosure,
//! domain trust) lands in 2.6-D through 2.6-G.

use hodos_permission_engine::{CallKind, PermissionContext, TrustLevel};

use crate::database::DomainPermission;

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

/// Build a `PermissionContext` for one of the 4 privacy-perimeter CallKinds.
///
/// Phase 2.6-C.2 / C.4. Reads the per-domain row (V17
/// `identity_key_disclosure_allowed` column) and translates `trust_level` to
/// the engine's `TrustLevel` enum. Session opt-in flags are now passed in by
/// the caller (typically pulled from `PermissionService::is_identity_key_session_approved`
/// / `is_key_linkage_session_approved`), per Phase 2.6-C.4 follow-up that
/// migrated the C++ session caches into the Rust service.
///
/// `domain_perm`:
///   - `Some(perm)` — domain row exists; trust_level is whatever the row says
///     ("approved" most commonly, but also "blocked" or "unknown" if the C++
///     side hasn't reset to approved yet).
///   - `None` — no row for this (user, domain). Trust level is Unknown —
///     engine's `DecideDomainTrust` will return a `DomainApproval` prompt.
pub fn build_privacy_perimeter_context(
    call_kind: CallKind,
    domain_perm: Option<&DomainPermission>,
    identity_key_session_opt_in: bool,
    key_linkage_session_opt_in: bool,
) -> PermissionContext {
    debug_assert!(
        matches!(
            call_kind,
            CallKind::IdentityKeyReveal
                | CallKind::CounterpartyKeyLinkage
                | CallKind::SpecificKeyLinkage
                | CallKind::SensitiveCertField
        ),
        "build_privacy_perimeter_context called with non-privacy-perimeter CallKind"
    );

    let (trust_level, identity_key_disclosure_allowed) = match domain_perm {
        Some(perm) => (
            match perm.trust_level.as_str() {
                "approved" => TrustLevel::Approved,
                "blocked" => TrustLevel::Blocked,
                _ => TrustLevel::Unknown,
            },
            perm.identity_key_disclosure_allowed,
        ),
        None => (TrustLevel::Unknown, false),
    };

    PermissionContext {
        call_kind,
        trust_level,
        identity_key_disclosure_allowed,
        identity_key_session_opt_in,
        key_linkage_session_opt_in,
        ..Default::default()
    }
}

/// Build a `PermissionContext` for one of the 3 scoped-grant CallKinds.
///
/// Phase 2.6-D. Reads the per-domain row (trust_level), takes the caller's
/// pre-computed `scoped_grant_exists` (the V18 lookup happens in the
/// `dispatch_scoped_grant` request gate since it needs the parsed scope
/// shape from the request body), and assembles the context the engine's
/// `DecideScopedGrant` branch will consult.
///
/// Note: caller is responsible for the protected-basket guardrail — when
/// `call_kind == BasketAccess` and the basket name is protected
/// (`default`, `backup-*`, `admin *`), the caller MUST pass
/// `bundled_scope_grant_override = Some(false)` AND force
/// `scoped_grant_exists = false`, so the engine always prompts regardless of
/// any V18 row or the column on `domain_permissions`. The defense-in-depth
/// pair (REJECTING POSTs to write protected-basket grants) lives in the
/// `grant_basket_permission` handler.
///
/// `bundled_scope_grant_override`:
///   - `None` — read `bundled_scope_grant` from `domain_perm` (the V22 column)
///   - `Some(b)` — override; used for protected-basket force-prompt
pub fn build_scoped_grant_context(
    call_kind: CallKind,
    domain_perm: Option<&DomainPermission>,
    scoped_grant_exists: bool,
    bundled_scope_grant_override: Option<bool>,
) -> PermissionContext {
    debug_assert!(
        matches!(
            call_kind,
            CallKind::ProtocolUse | CallKind::BasketAccess | CallKind::CounterpartyUse
        ),
        "build_scoped_grant_context called with non-scoped-grant CallKind"
    );

    let trust_level = match domain_perm {
        Some(perm) => match perm.trust_level.as_str() {
            "approved" => TrustLevel::Approved,
            "blocked" => TrustLevel::Blocked,
            _ => TrustLevel::Unknown,
        },
        None => TrustLevel::Unknown,
    };

    // Read the V22 column from the permission row, with override support for
    // the protected-basket guardrail.
    let bundled_scope_grant = match bundled_scope_grant_override {
        Some(b) => b,
        None => domain_perm.map(|p| p.bundled_scope_grant).unwrap_or(false),
    };

    PermissionContext {
        call_kind,
        trust_level,
        scoped_grant_exists,
        bundled_scope_grant,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_perm(trust: &str, identity_key_allowed: bool) -> DomainPermission {
        DomainPermission {
            id: Some(1),
            user_id: 1,
            domain: "example.com".to_string(),
            trust_level: trust.to_string(),
            per_tx_limit_cents: 100,
            per_session_limit_cents: 1000,
            rate_limit_per_min: 30,
            max_tx_per_session: 100,
            identity_key_disclosure_allowed: identity_key_allowed,
            bundled_scope_grant: false,
            created_at: 1,
            updated_at: 1,
        }
    }

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

    #[test]
    fn privacy_perimeter_context_reads_v17_column_for_identity_key() {
        let perm = sample_perm("approved", true);
        let ctx = build_privacy_perimeter_context(
            CallKind::IdentityKeyReveal, Some(&perm),
            /*identity_key_session_opt_in=*/ false,
            /*key_linkage_session_opt_in=*/ false,
        );
        assert_eq!(ctx.call_kind, CallKind::IdentityKeyReveal);
        assert_eq!(ctx.trust_level, TrustLevel::Approved);
        assert!(ctx.identity_key_disclosure_allowed);
        assert!(!ctx.identity_key_session_opt_in);
        assert!(!ctx.key_linkage_session_opt_in);
    }

    #[test]
    fn privacy_perimeter_context_translates_unknown_trust_when_no_perm_row() {
        let ctx = build_privacy_perimeter_context(
            CallKind::IdentityKeyReveal, None, false, false,
        );
        assert_eq!(ctx.trust_level, TrustLevel::Unknown);
        assert!(!ctx.identity_key_disclosure_allowed);
    }

    #[test]
    fn privacy_perimeter_context_translates_blocked_trust() {
        let perm = sample_perm("blocked", true);
        let ctx = build_privacy_perimeter_context(
            CallKind::CounterpartyKeyLinkage, Some(&perm), false, false,
        );
        assert_eq!(ctx.trust_level, TrustLevel::Blocked);
        // identity_key_disclosure_allowed read regardless — engine ignores it
        // for non-identity-key call kinds via the branch logic.
        assert!(ctx.identity_key_disclosure_allowed);
    }

    #[test]
    fn privacy_perimeter_context_translates_unrecognized_trust_to_unknown() {
        let perm = sample_perm("future-value-not-mapped", false);
        let ctx = build_privacy_perimeter_context(
            CallKind::SpecificKeyLinkage, Some(&perm), false, false,
        );
        assert_eq!(ctx.trust_level, TrustLevel::Unknown);
    }

    #[test]
    fn privacy_perimeter_context_for_sensitive_cert_field() {
        let perm = sample_perm("approved", true);
        let ctx = build_privacy_perimeter_context(
            CallKind::SensitiveCertField, Some(&perm), false, false,
        );
        assert_eq!(ctx.call_kind, CallKind::SensitiveCertField);
        assert_eq!(ctx.trust_level, TrustLevel::Approved);
        // SensitiveCertField branch always prompts regardless of these flags,
        // but we still populate them per spec.
        assert!(ctx.identity_key_disclosure_allowed);
    }

    // Phase 2.6-C.4 follow-up — session opt-in flags propagate.

    #[test]
    fn privacy_perimeter_context_propagates_identity_key_session_opt_in() {
        let perm = sample_perm("approved", false);
        let ctx = build_privacy_perimeter_context(
            CallKind::IdentityKeyReveal, Some(&perm),
            /*identity_key_session_opt_in=*/ true,
            /*key_linkage_session_opt_in=*/ false,
        );
        // V17 says NOT pre-approved, but session opt-in is set — engine
        // returns Silent via DecidePrivacyPerimeter.
        assert!(!ctx.identity_key_disclosure_allowed);
        assert!(ctx.identity_key_session_opt_in);
        assert!(!ctx.key_linkage_session_opt_in);
    }

    #[test]
    fn privacy_perimeter_context_propagates_key_linkage_session_opt_in() {
        let perm = sample_perm("approved", false);
        let ctx = build_privacy_perimeter_context(
            CallKind::CounterpartyKeyLinkage, Some(&perm),
            false, true,
        );
        assert!(!ctx.identity_key_session_opt_in);
        assert!(ctx.key_linkage_session_opt_in);
    }

    // ------------- Phase 2.6-D — scoped grant context builder -------------

    #[test]
    fn scoped_grant_context_for_protocol_use_silent_path() {
        let perm = sample_perm("approved", false);
        let ctx = build_scoped_grant_context(
            CallKind::ProtocolUse, Some(&perm),
            /*scoped_grant_exists=*/ true,
            /*bundled_scope_grant_override=*/ None,
        );
        assert_eq!(ctx.call_kind, CallKind::ProtocolUse);
        assert_eq!(ctx.trust_level, TrustLevel::Approved);
        assert!(ctx.scoped_grant_exists);
        assert!(!ctx.bundled_scope_grant); // perm.bundled_scope_grant defaults to false
    }

    #[test]
    fn scoped_grant_context_for_basket_access_prompt_path() {
        let perm = sample_perm("approved", false);
        let ctx = build_scoped_grant_context(
            CallKind::BasketAccess, Some(&perm),
            /*scoped_grant_exists=*/ false,
            None,
        );
        assert!(!ctx.scoped_grant_exists);
        assert!(!ctx.bundled_scope_grant);
    }

    #[test]
    fn scoped_grant_context_for_counterparty_use() {
        let perm = sample_perm("approved", false);
        let ctx = build_scoped_grant_context(
            CallKind::CounterpartyUse, Some(&perm),
            true,
            None,
        );
        assert_eq!(ctx.call_kind, CallKind::CounterpartyUse);
        assert!(ctx.scoped_grant_exists);
    }

    #[test]
    fn scoped_grant_context_without_perm_row_falls_to_unknown() {
        let ctx = build_scoped_grant_context(CallKind::ProtocolUse, None, true, None);
        assert_eq!(ctx.trust_level, TrustLevel::Unknown);
        // Engine will hit DomainTrust gate first and Prompt domain_approval
        // regardless of scoped_grant_exists — the flag still gets populated
        // for downstream branches but DomainTrust short-circuits.
        assert!(ctx.scoped_grant_exists);
        // Unknown trust + no perm row → bundled_scope_grant defaults to false
        assert!(!ctx.bundled_scope_grant);
    }

    #[test]
    fn scoped_grant_context_translates_blocked_trust() {
        let perm = sample_perm("blocked", false);
        let ctx = build_scoped_grant_context(CallKind::ProtocolUse, Some(&perm), true, None);
        assert_eq!(ctx.trust_level, TrustLevel::Blocked);
    }

    // ------------- Phase 2.6-D Fix #4 — bundled_scope_grant propagation -------------

    #[test]
    fn scoped_grant_context_reads_bundled_flag_from_perm_row() {
        let mut perm = sample_perm("approved", false);
        perm.bundled_scope_grant = true;
        let ctx = build_scoped_grant_context(
            CallKind::ProtocolUse, Some(&perm),
            /*scoped_grant_exists=*/ false,
            /*bundled_scope_grant_override=*/ None,
        );
        assert!(ctx.bundled_scope_grant);
    }

    #[test]
    fn scoped_grant_context_protected_basket_override_forces_false() {
        // Even when the V22 column says bundled grant is on, the protected
        // basket override must force it false so the engine prompts.
        let mut perm = sample_perm("approved", false);
        perm.bundled_scope_grant = true;
        let ctx = build_scoped_grant_context(
            CallKind::BasketAccess, Some(&perm),
            false,
            /*bundled_scope_grant_override=*/ Some(false),
        );
        assert!(!ctx.bundled_scope_grant);
    }

    #[test]
    fn scoped_grant_context_override_some_true_pins_to_true() {
        // No real production caller does this, but the API is general — verify
        // an explicit Some(true) override pins true even if perm row says false.
        let perm = sample_perm("approved", false); // bundled_scope_grant=false in sample
        let ctx = build_scoped_grant_context(
            CallKind::ProtocolUse, Some(&perm),
            false,
            Some(true),
        );
        assert!(ctx.bundled_scope_grant);
    }
}
