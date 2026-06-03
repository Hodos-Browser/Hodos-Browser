//! Per-request gate dispatch for privacy-perimeter wallet endpoints.
//!
//! Phase 2.6-C.2. Provides `dispatch_privacy_perimeter`, the helper each of
//! the 4 privacy-perimeter handlers (`get_public_key` identityKey path,
//! `reveal_counterparty_key_linkage`, `reveal_specific_key_linkage`,
//! `prove_certificate` sensitive-field path) calls before its handler body.
//!
//! The helper implements the LD2 wire contract:
//!   - `X-User-Approved: <id>` header → atomic consume + body-sha256 verify;
//!     success = `GateOutcome::Proceed`; failure = 403 `body_mismatch` or
//!     `approval_expired_or_consumed`.
//!   - Otherwise → build PermissionContext from the live domain_permissions
//!     row, run `PermissionService::decide`, translate to 200 / 202 / 403.
//!
//! Defense-in-depth: this helper does NOT replace `check_domain_approved` —
//! the calling handler still runs that AFTER `dispatch_privacy_perimeter`
//! returns `Proceed`. Both gates apply independently per kickoff invariant.
//!
//! Lib-crate constraint: AppState lives in the binary so this module takes
//! individual `web::Data` extractors (matches the existing `shadow_decide`
//! pattern).

use std::sync::{Arc, Mutex};

use actix_web::{HttpRequest, HttpResponse};
use serde_json::Value;

use hodos_permission_engine::{CallKind, PermissionDecision, PromptType};

use crate::database::{DomainPermissionRepository, WalletDatabase};

use super::audit;
use super::context_builder;
use super::state::{ApprovalConsumeError, PermissionService, APPROVAL_TTL_SECS};

/// What the calling handler should do next.
pub enum GateOutcome {
    /// Engine said Silent OR the X-User-Approved replay verified successfully.
    /// Handler should run its normal body (and its own `check_domain_approved`
    /// defense-in-depth call afterwards).
    Proceed,
    /// Engine returned Prompt/Deny, or X-User-Approved verification failed.
    /// Handler must return this `HttpResponse` immediately, without doing any
    /// of its own work.
    EarlyReturn(HttpResponse),
}

/// Header name carrying the user-approved id on the replay request.
pub const X_USER_APPROVED: &str = "X-User-Approved";

/// Header name C++ injects on outbound calls so Rust knows the originating
/// origin. Pre-existing — used by `check_domain_approved` too.
pub const X_REQUESTING_DOMAIN: &str = "X-Requesting-Domain";

/// Dispatch the engine gate for a privacy-perimeter CallKind.
///
/// `call_kind` MUST be one of the 4 privacy-perimeter variants —
/// `IdentityKeyReveal`, `CounterpartyKeyLinkage`, `SpecificKeyLinkage`, or
/// `SensitiveCertField`. Passing any other variant is a logic error caught
/// by `build_privacy_perimeter_context`'s debug_assert.
///
/// `build_prompt_payload` is invoked ONLY when the engine returns Prompt; it
/// produces the per-modal-type `promptPayload` body shape from LD2. Returning
/// `Value::Null` is acceptable for prompt types that take no extra params
/// (e.g. identity_key_reveal — modal renders from origin alone).
///
/// Internal calls (no `X-Requesting-Domain` header) bypass the engine
/// entirely — the wallet UI doesn't need its own permission gate.
pub fn dispatch_privacy_perimeter(
    permission: &Arc<PermissionService>,
    database: &Arc<Mutex<WalletDatabase>>,
    current_user_id: i64,
    http_req: &HttpRequest,
    body: &[u8],
    endpoint: &str,
    call_kind: CallKind,
    build_prompt_payload: impl FnOnce() -> Value,
) -> GateOutcome {
    // 1. Identify the requesting origin. No header → internal call (wallet UI
    // calling its own backend) — engine doesn't apply; handler proceeds.
    let domain = match http_req
        .headers()
        .get(X_REQUESTING_DOMAIN)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        Some(d) => d.to_string(),
        None => return GateOutcome::Proceed,
    };

    // 2. X-User-Approved replay path. The C++ side sets this header on the
    //    re-issued request after the user approves the modal. Body must match
    //    sha256 stored at mint time.
    if let Some(approval_id) = http_req
        .headers()
        .get(X_USER_APPROVED)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        let now = chrono::Utc::now().timestamp();
        match permission.consume_and_verify(approval_id, body, now) {
            Ok(approval) => {
                log::info!(
                    "🔐 X-User-Approved consumed for domain={} endpoint={} id={}",
                    approval.domain,
                    approval.endpoint,
                    approval_id,
                );
                return GateOutcome::Proceed;
            }
            Err(ApprovalConsumeError::BodyMismatch) => {
                log::warn!(
                    "🛡️ X-User-Approved body sha256 mismatch (domain={} id={}) — rejecting",
                    domain,
                    approval_id,
                );
                return GateOutcome::EarlyReturn(forbidden_envelope("body_mismatch"));
            }
            Err(ApprovalConsumeError::Expired)
            | Err(ApprovalConsumeError::NotFound) => {
                log::warn!(
                    "🛡️ X-User-Approved id not consumable (domain={} id={}) — rejecting",
                    domain,
                    approval_id,
                );
                return GateOutcome::EarlyReturn(forbidden_envelope("approval_expired_or_consumed"));
            }
        }
    }

    // 3. No approval header → engine path. Read domain row, build context,
    //    decide.
    let perm_row = {
        let db_guard = match database.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!(
                    "🛡️ database mutex poisoned in dispatch_privacy_perimeter: {} — denying",
                    poisoned
                );
                return GateOutcome::EarlyReturn(internal_error_envelope("db_mutex_poisoned"));
            }
        };
        let repo = DomainPermissionRepository::new(db_guard.connection());
        match repo.get_by_domain(current_user_id, &domain) {
            Ok(p) => p,
            Err(e) => {
                log::error!(
                    "🛡️ DomainPermissionRepository::get_by_domain('{}') failed: {} — denying",
                    domain,
                    e
                );
                return GateOutcome::EarlyReturn(internal_error_envelope("db_read_error"));
            }
        }
    };

    let ctx = context_builder::build_privacy_perimeter_context(call_kind, perm_row.as_ref());
    let decision = permission.decide(&ctx);

    match decision {
        PermissionDecision::Silent { .. } => {
            log::debug!(
                "🔓 engine Silent for domain={} endpoint={} kind={:?}",
                domain,
                endpoint,
                call_kind
            );
            GateOutcome::Proceed
        }
        PermissionDecision::Prompt {
            ref prompt_type,
            ref reason,
        } => {
            // Mint a pending approval BEFORE building the envelope so the
            // approvalId is the live one the replay can consume.
            let now = chrono::Utc::now().timestamp();
            let approval_id = permission.mint_pending_approval(&domain, endpoint, body, now);
            let prompt_payload = build_prompt_payload();
            let envelope = build_pending_envelope(
                &approval_id,
                *prompt_type,
                reason_to_snake_case(reason),
                prompt_payload,
            );
            log::info!(
                "🛡️ engine Prompt minted approval id={} for domain={} endpoint={} kind={:?}",
                approval_id,
                domain,
                endpoint,
                call_kind
            );
            GateOutcome::EarlyReturn(HttpResponse::Accepted().json(envelope))
        }
        PermissionDecision::Deny { ref reason } => {
            log::warn!(
                "🛡️ engine Deny for domain={} endpoint={} kind={:?} reason={:?}",
                domain,
                endpoint,
                call_kind,
                reason
            );
            GateOutcome::EarlyReturn(forbidden_envelope(&reason_to_snake_case(reason)))
        }
    }
}

/// Build the 202 PENDING envelope per LD2.
fn build_pending_envelope(
    approval_id: &str,
    prompt_type: PromptType,
    engine_reason: String,
    prompt_payload: Value,
) -> Value {
    serde_json::json!({
        "status": "pending",
        "approvalId": approval_id,
        "promptType": prompt_type_snake_case(prompt_type),
        "engineReason": engine_reason,
        "ttlMs": APPROVAL_TTL_SECS * 1000,
        "schemaVersion": 1,
        "promptPayload": prompt_payload,
    })
}

/// PromptType → snake_case string (matches React's BRC100AuthOverlayRoot
/// dispatch).
fn prompt_type_snake_case(p: PromptType) -> String {
    serde_json::to_value(p)
        .and_then(|v| serde_json::from_value::<String>(v))
        .unwrap_or_else(|_| format!("{:?}", p).to_lowercase())
}

/// `EngineReason` → snake_case string.
fn reason_to_snake_case<T: serde::Serialize>(reason: &T) -> String {
    serde_json::to_value(reason)
        .and_then(|v| serde_json::from_value::<String>(v))
        .unwrap_or_else(|_| String::from("unknown_reason"))
}

/// 403 envelope with structured reason. Matches the existing privacy-perimeter
/// error shape (`error` + `code` fields) plus an explicit `reason` field for
/// the new LD2-typed reasons.
fn forbidden_envelope(reason: &str) -> HttpResponse {
    HttpResponse::Forbidden().json(serde_json::json!({
        "error": format!("Request denied: {}", reason),
        "code": "ERR_PERMISSION_DENIED",
        "reason": reason,
    }))
}

/// 500 envelope for internal errors (DB lock poisoning, DB read failure).
fn internal_error_envelope(reason: &str) -> HttpResponse {
    HttpResponse::InternalServerError().json(serde_json::json!({
        "error": "Internal permission gate error",
        "reason": reason,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hodos_permission_engine::{EngineReason, PromptType};

    #[test]
    fn pending_envelope_has_ld2_fields() {
        let envelope = build_pending_envelope(
            "abc123",
            PromptType::IdentityKeyReveal,
            "privacy_perimeter_no_grant".to_string(),
            serde_json::json!({}),
        );
        assert_eq!(envelope["status"], "pending");
        assert_eq!(envelope["approvalId"], "abc123");
        assert_eq!(envelope["promptType"], "identity_key_reveal");
        assert_eq!(envelope["engineReason"], "privacy_perimeter_no_grant");
        assert_eq!(envelope["ttlMs"], APPROVAL_TTL_SECS * 1000);
        assert_eq!(envelope["schemaVersion"], 1);
        assert!(envelope["promptPayload"].is_object());
    }

    #[test]
    fn prompt_type_serializes_snake_case() {
        assert_eq!(prompt_type_snake_case(PromptType::IdentityKeyReveal), "identity_key_reveal");
        assert_eq!(prompt_type_snake_case(PromptType::KeyLinkageReveal), "key_linkage_reveal");
        assert_eq!(prompt_type_snake_case(PromptType::CertificateDisclosure), "certificate_disclosure");
    }

    #[test]
    fn reason_serializes_snake_case() {
        let s = reason_to_snake_case(&EngineReason::PrivacyPerimeterNoGrant);
        assert_eq!(s, "privacy_perimeter_no_grant");
    }
}
