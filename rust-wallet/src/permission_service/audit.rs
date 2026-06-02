//! Audit + shadow log write helpers.
//!
//! Thin wrappers over `PermissionAuditRepository` and `EngineShadowRepository`
//! that handle the sha256 hashing of bodies/contexts and the
//! decision/CallKind/EngineReason serde-string conversion.
//!
//! Scaffolding only in 2.6-A.5 — call sites land in 2.6-B (shadow) and 2.6-C+
//! (audit). The helpers are tested in isolation here.

use sha2::{Digest, Sha256};

use hodos_permission_engine::{CallKind, PermissionContext, PermissionDecision, PromptType};

use crate::database::{
    engine_shadow_repo::EngineShadowEntry, permission_audit_repo::PermissionAuditEntry,
};

use super::flags::EngineFlags;

/// Compute the sha256 hex hash of a byte slice. 64-char `VARCHAR(64)` output
/// matches the V20 schema.
pub fn body_hash(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body);
    let result = hasher.finalize();
    hex_encode(&result)
}

/// Compute the sha256 hex hash of the serialized PermissionContext. Used as
/// the `context_hash` column of `engine_shadow_log` for deduplication
/// analytics (not enforced unique).
pub fn context_hash(ctx: &PermissionContext) -> String {
    let json = serde_json::to_vec(ctx).unwrap_or_default();
    body_hash(&json)
}

/// Convert a `PermissionDecision` into the (decision_string, reason_string)
/// pair stored by both `permission_audit_log` and `engine_shadow_log`.
///
/// decision_string is one of: "silent" | "prompt" | "deny".
/// reason_string is the serde-snake_case `EngineReason` (e.g. "per_tx_limit").
pub fn decision_to_strings(d: &PermissionDecision) -> (String, String) {
    match d {
        PermissionDecision::Silent { reason } => (
            "silent".to_string(),
            serde_json::to_value(reason)
                .and_then(|v| serde_json::from_value::<String>(v))
                .unwrap_or_else(|_| format!("{:?}", reason).to_lowercase()),
        ),
        PermissionDecision::Prompt { reason, .. } => (
            "prompt".to_string(),
            serde_json::to_value(reason)
                .and_then(|v| serde_json::from_value::<String>(v))
                .unwrap_or_else(|_| format!("{:?}", reason).to_lowercase()),
        ),
        PermissionDecision::Deny { reason } => (
            "deny".to_string(),
            serde_json::to_value(reason)
                .and_then(|v| serde_json::from_value::<String>(v))
                .unwrap_or_else(|_| format!("{:?}", reason).to_lowercase()),
        ),
    }
}

/// Extract the prompt_type string from a Prompt decision, or `None` for
/// Silent/Deny.
pub fn prompt_type_string(d: &PermissionDecision) -> Option<String> {
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        // Serialize via serde to get the snake_case representation. Falls back
        // to debug-format if serialization fails (should never happen for a
        // typed enum).
        Some(
            serde_json::to_value(prompt_type)
                .and_then(|v| serde_json::from_value::<String>(v))
                .unwrap_or_else(|_| format!("{:?}", prompt_type).to_lowercase()),
        )
    } else {
        None
    }
}

/// Build a `PermissionAuditEntry` ready for insertion.
///
/// `approval_id` is `Some(...)` only for Prompt decisions. `user_decision`,
/// `resolved_at`, `resolved_via` start as `None` and are updated by
/// `mark_resolved` when the user responds (or never updated for Silent/Deny).
pub fn build_audit_entry(
    decision: &PermissionDecision,
    call_kind: CallKind,
    domain: &str,
    endpoint: &str,
    body: &[u8],
    approval_id: Option<&str>,
    now_secs: i64,
) -> PermissionAuditEntry {
    let (decision_str, reason_str) = decision_to_strings(decision);
    PermissionAuditEntry {
        id: None,
        approval_id: approval_id.map(|s| s.to_string()),
        domain: domain.to_string(),
        endpoint: endpoint.to_string(),
        call_kind: call_kind_string(call_kind),
        engine_reason: reason_str,
        decision: decision_str,
        user_decision: None,
        body_hash: body_hash(body),
        created_at: now_secs,
        resolved_at: None,
        resolved_via: None,
    }
}

/// Build an `EngineShadowEntry` from a C++ decision (string-typed because
/// C++ uses raw strings) and a Rust decision (typed PermissionDecision).
///
/// Used by the 2.6-B shadow infrastructure to record one row per wallet call.
pub fn build_shadow_entry(
    cpp_decision: &str,
    cpp_prompt_type: Option<&str>,
    cpp_reason: Option<&str>,
    rust_decision: &PermissionDecision,
    call_kind: CallKind,
    ctx: &PermissionContext,
    domain: &str,
    endpoint: &str,
    now_secs: i64,
) -> EngineShadowEntry {
    let (rust_decision_str, rust_reason_str) = decision_to_strings(rust_decision);
    let rust_prompt_type = prompt_type_string(rust_decision);

    // Agreement is true iff both engines made the same Silent/Prompt/Deny
    // call AND (for Prompt) selected the same prompt_type. Reason text is
    // diagnostic only — C++ uses free-form strings that wouldn't match Rust's
    // typed enum names — so we don't compare reasons for the agreement check.
    let kinds_match = cpp_decision == rust_decision_str;
    let prompt_types_match = match (cpp_prompt_type, rust_prompt_type.as_deref()) {
        (Some(c), Some(r)) => c == r,
        (None, None) => true, // both Silent or both Deny
        _ => false,
    };
    let agreement = if kinds_match && prompt_types_match { 1 } else { 0 };

    EngineShadowEntry {
        id: None,
        call_kind_class: EngineFlags::class_name_for(call_kind).to_string(),
        endpoint: endpoint.to_string(),
        domain: domain.to_string(),
        cpp_decision: cpp_decision.to_string(),
        rust_decision: rust_decision_str,
        cpp_reason: cpp_reason.map(|s| s.to_string()),
        rust_reason: Some(rust_reason_str),
        agreement,
        context_hash: context_hash(ctx),
        observed_at: now_secs,
    }
}

/// CallKind → serde-PascalCase string (e.g. "Payment", "IdentityKeyReveal").
fn call_kind_string(kind: CallKind) -> String {
    serde_json::to_value(kind)
        .and_then(|v| serde_json::from_value::<String>(v))
        .unwrap_or_else(|_| format!("{:?}", kind))
}

/// Hex-encode a byte slice as a lowercase string. We don't pull in the `hex`
/// crate because it's not a dependency of this module's parent — sha2 + a
/// 2-line helper is enough.
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use hodos_permission_engine::{EngineReason, PermissionDecision, PromptType, TrustLevel};

    #[test]
    fn body_hash_is_64_hex_chars() {
        let h = body_hash(b"hello");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        // Known sha256("hello") prefix:
        assert!(h.starts_with("2cf24dba"));
    }

    #[test]
    fn context_hash_changes_with_context() {
        let ctx_a = PermissionContext::default();
        let mut ctx_b = PermissionContext::default();
        ctx_b.trust_level = TrustLevel::Blocked;
        assert_ne!(context_hash(&ctx_a), context_hash(&ctx_b));
    }

    #[test]
    fn decision_strings_for_silent_have_no_prompt_type() {
        let d = PermissionDecision::silent(EngineReason::SilentWithinCaps);
        let (kind, reason) = decision_to_strings(&d);
        assert_eq!(kind, "silent");
        assert_eq!(reason, "silent_within_caps");
        assert!(prompt_type_string(&d).is_none());
    }

    #[test]
    fn decision_strings_for_prompt_carry_prompt_type() {
        let d = PermissionDecision::prompt(
            PromptType::PaymentConfirmation,
            EngineReason::PerTxLimit,
        );
        let (kind, reason) = decision_to_strings(&d);
        assert_eq!(kind, "prompt");
        assert_eq!(reason, "per_tx_limit");
        assert_eq!(prompt_type_string(&d).as_deref(), Some("payment_confirmation"));
    }

    #[test]
    fn decision_strings_for_deny_have_no_prompt_type() {
        let d = PermissionDecision::deny(EngineReason::TrustBlocked);
        let (kind, reason) = decision_to_strings(&d);
        assert_eq!(kind, "deny");
        assert_eq!(reason, "trust_blocked");
        assert!(prompt_type_string(&d).is_none());
    }

    #[test]
    fn build_audit_entry_populates_required_fields() {
        let d = PermissionDecision::silent(EngineReason::SilentWithinCaps);
        let entry = build_audit_entry(
            &d,
            CallKind::Payment,
            "example.com",
            "/createAction",
            b"body bytes",
            None,
            1_700_000_000,
        );
        assert_eq!(entry.domain, "example.com");
        assert_eq!(entry.endpoint, "/createAction");
        assert_eq!(entry.call_kind, "Payment");
        assert_eq!(entry.decision, "silent");
        assert_eq!(entry.engine_reason, "silent_within_caps");
        assert_eq!(entry.body_hash.len(), 64);
        assert!(entry.approval_id.is_none());
        assert!(entry.user_decision.is_none());
    }

    #[test]
    fn build_shadow_entry_agreement_for_matching_silent() {
        let rust_d = PermissionDecision::silent(EngineReason::SilentWithinCaps);
        let ctx = PermissionContext::default();
        let entry = build_shadow_entry(
            "silent",
            None,
            Some("payment within all configured caps"),
            &rust_d,
            CallKind::Payment,
            &ctx,
            "example.com",
            "/createAction",
            1_700_000_000,
        );
        assert_eq!(entry.agreement, 1);
        assert_eq!(entry.call_kind_class, "payment");
        assert_eq!(entry.cpp_decision, "silent");
        assert_eq!(entry.rust_decision, "silent");
    }

    #[test]
    fn build_shadow_entry_disagreement_when_prompt_types_differ() {
        let rust_d = PermissionDecision::prompt(
            PromptType::PaymentConfirmation,
            EngineReason::PerTxLimit,
        );
        let ctx = PermissionContext::default();
        let entry = build_shadow_entry(
            "prompt",
            Some("rate_limit_exceeded"), // C++ said rate-limit
            None,
            &rust_d,                     // Rust said payment_confirmation
            CallKind::Payment,
            &ctx,
            "example.com",
            "/createAction",
            1_700_000_000,
        );
        assert_eq!(entry.agreement, 0);
    }

    #[test]
    fn build_shadow_entry_disagreement_when_kinds_differ() {
        let rust_d = PermissionDecision::silent(EngineReason::SilentWithinCaps);
        let ctx = PermissionContext::default();
        let entry = build_shadow_entry(
            "prompt",
            Some("payment_confirmation"),
            None,
            &rust_d,
            CallKind::Payment,
            &ctx,
            "example.com",
            "/createAction",
            1_700_000_000,
        );
        assert_eq!(entry.agreement, 0);
    }
}
