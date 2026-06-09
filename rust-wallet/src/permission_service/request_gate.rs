//! Per-request gate dispatch for privacy-perimeter + scoped-grant wallet
//! endpoints.
//!
//! Phase 2.6-C.2. Provides `dispatch_privacy_perimeter`, the helper each of
//! the 4 privacy-perimeter handlers (`get_public_key` identityKey path,
//! `reveal_counterparty_key_linkage`, `reveal_specific_key_linkage`,
//! `prove_certificate` sensitive-field path) calls before its handler body.
//!
//! Phase 2.6-D. Adds `dispatch_scoped_grant` for the 10+ scoped-grant
//! handlers (`create_signature`, `create_hmac`, `verify_hmac`, `encrypt`,
//! `decrypt`, `encrypt_bie1`, `decrypt_bie1`, `list_outputs`,
//! `relinquish_output`, `list_messages`, `acknowledge_message`). Same wire
//! contract (LD2). Engine handles ProtocolUse / BasketAccess /
//! CounterpartyUse classification based on the `ScopedCall` shape the caller
//! passes in (which the caller derives from its already-parsed body).
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

    // Phase 2.6-C.4 follow-up — session opt-ins are now Rust-side. Pull both
    // flags for `domain` from PermissionService's caches; only the matching
    // CallKind branch in the engine reads them, but we populate both for
    // schema completeness (and so an audit dump of the ctx is self-explanatory).
    let identity_key_session_opt_in =
        permission.is_identity_key_session_approved(&domain);
    let key_linkage_session_opt_in =
        permission.is_key_linkage_session_approved(&domain);

    let ctx = context_builder::build_privacy_perimeter_context(
        call_kind,
        perm_row.as_ref(),
        identity_key_session_opt_in,
        key_linkage_session_opt_in,
    );
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

// ============================================================================
// Phase 2.6-D — scoped-grant dispatch
// ============================================================================

/// Shape of a scoped-grant call. The caller (each scoped-grant handler) parses
/// its body once and threads the relevant scope fields through. Two variants
/// because that's the BRC-100 surface: protocol calls (which may carry a
/// specific counterparty) and basket calls.
///
/// CallKind classification:
///   - `Protocol { counterparty: Some(_) }` → `CallKind::CounterpartyUse`
///   - `Protocol { counterparty: None }`    → `CallKind::ProtocolUse`
///   - `Basket { .. }`                       → `CallKind::BasketAccess`
#[derive(Debug, Clone)]
pub enum ScopedCall<'a> {
    Protocol {
        level: u8,
        name: &'a str,
        key_id: &'a str,
        counterparty: Option<&'a str>,
    },
    Basket {
        basket: &'a str,
        access: &'a str, // "read" | "read_write"
    },
}

impl<'a> ScopedCall<'a> {
    fn call_kind(&self) -> hodos_permission_engine::CallKind {
        use hodos_permission_engine::CallKind;
        match self {
            ScopedCall::Protocol { counterparty: Some(_), .. } => CallKind::CounterpartyUse,
            ScopedCall::Protocol { counterparty: None, .. } => CallKind::ProtocolUse,
            ScopedCall::Basket { .. } => CallKind::BasketAccess,
        }
    }
}

/// Hardcoded protected basket names — never auto-grant, always prompt.
///
/// Phase 2.6-D defense-in-depth, mirrors Phase 1.5 Step 5 (commit `b1d85c8`)
/// where the React Customize subview disables checkboxes for these names.
/// Backend was missing the enforcement; D.3 adds it on BOTH sides.
///
/// Rule:
///   - `default` — the wallet's catch-all basket containing every untagged
///     output. Auto-grant = blanket UTXO disclosure.
///   - `backup-*` (any name starting with `backup-`) — the wallet's own
///     backup baskets used for the on-chain wallet-backup feature.
///   - `admin *` (any name starting with `admin `) — administrative scoped
///     baskets reserved for wallet-management calls.
pub fn is_protected_basket(name: &str) -> bool {
    name == "default" || name.starts_with("backup-") || name.starts_with("admin ")
}

/// Dispatch the engine gate for a scoped-grant CallKind. Mirrors
/// `dispatch_privacy_perimeter` exactly except for context construction.
///
/// Steps:
///   1. No `X-Requesting-Domain` → internal call → `Proceed` immediately.
///   2. `X-User-Approved` present → consume + body-sha256 verify → `Proceed`
///      on success, 403 envelope on failure.
///   3. Look up `domain_permissions` row.
///   4. Compute `scoped_grant_exists` from V18 sub-permission tables.
///      - For ProtocolUse: `is_protocol_granted`
///      - For CounterpartyUse: `is_counterparty_granted` OR `is_protocol_granted`
///        (either match silences — broader trust signal first per memory
///        `phase15_step6_commit_e`)
///      - For BasketAccess: `is_basket_granted`, UNLESS basket name is
///        protected (`is_protected_basket`) — protected baskets force
///        `scoped_grant_exists = false` so engine always prompts (D.3
///        defense-in-depth)
///   5. Build context, run engine, translate decision to 200 / 202 / 403.
///
/// Defense-in-depth: caller still runs `check_domain_approved` after
/// `Proceed` (same as privacy-perimeter dispatch).
pub fn dispatch_scoped_grant(
    permission: &Arc<PermissionService>,
    database: &Arc<Mutex<WalletDatabase>>,
    current_user_id: i64,
    http_req: &HttpRequest,
    body: &[u8],
    endpoint: &str,
    scoped_call: ScopedCall<'_>,
) -> GateOutcome {
    let domain = match http_req
        .headers()
        .get(X_REQUESTING_DOMAIN)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        Some(d) => d.to_string(),
        None => return GateOutcome::Proceed,
    };

    // X-User-Approved replay path (same as privacy perimeter).
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
                    "🔐 X-User-Approved consumed (scoped) for domain={} endpoint={} id={}",
                    approval.domain, approval.endpoint, approval_id,
                );
                return GateOutcome::Proceed;
            }
            Err(ApprovalConsumeError::BodyMismatch) => {
                log::warn!(
                    "🛡️ X-User-Approved body sha256 mismatch (scoped, domain={} id={})",
                    domain, approval_id,
                );
                return GateOutcome::EarlyReturn(forbidden_envelope("body_mismatch"));
            }
            Err(ApprovalConsumeError::Expired) | Err(ApprovalConsumeError::NotFound) => {
                log::warn!(
                    "🛡️ X-User-Approved not consumable (scoped, domain={} id={})",
                    domain, approval_id,
                );
                return GateOutcome::EarlyReturn(forbidden_envelope(
                    "approval_expired_or_consumed",
                ));
            }
        }
    }

    let perm_row = {
        let db_guard = match database.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!(
                    "🛡️ database mutex poisoned in dispatch_scoped_grant: {} — denying",
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
                    domain, e
                );
                return GateOutcome::EarlyReturn(internal_error_envelope("db_read_error"));
            }
        }
    };

    // Compute scoped_grant_exists from V18 lookup (under a fresh lock).
    let scoped_grant_exists = if let Some(ref perm) = perm_row {
        let dp_id = perm.id.unwrap_or(0);
        if dp_id == 0 {
            // No id means the row hasn't been persisted yet — treat as no grant.
            false
        } else {
            let db_guard = match database.lock() {
                Ok(g) => g,
                Err(_) => {
                    return GateOutcome::EarlyReturn(internal_error_envelope("db_mutex_poisoned"));
                }
            };
            let repo = DomainPermissionRepository::new(db_guard.connection());
            match &scoped_call {
                ScopedCall::Protocol { level, name, key_id, counterparty: None } => {
                    repo.is_protocol_granted(dp_id, *level, name, key_id, None)
                        .unwrap_or(false)
                }
                ScopedCall::Protocol {
                    level,
                    name,
                    key_id,
                    counterparty: Some(cp),
                } => {
                    // CounterpartyUse — either match silences the gate.
                    repo.is_counterparty_granted(dp_id, cp).unwrap_or(false)
                        || repo
                            .is_protocol_granted(dp_id, *level, name, key_id, Some(*cp))
                            .unwrap_or(false)
                }
                ScopedCall::Basket { basket, access } => {
                    // Phase 2.6-D protected-basket guardrail — force-Prompt by
                    // pretending no grant exists, regardless of what V18 says.
                    if is_protected_basket(basket) {
                        false
                    } else {
                        repo.is_basket_granted(dp_id, basket, access).unwrap_or(false)
                    }
                }
            }
        }
    } else {
        false
    };

    let call_kind = scoped_call.call_kind();
    // Phase 2.6-D Fix #4 — protected-basket override.
    //
    // The V22 `bundled_scope_grant` column normally silences ProtocolUse +
    // BasketAccess for the domain. For BasketAccess against a protected
    // basket (`default` / `backup-*` / `admin *`), we must NOT let the bundle
    // grant silence the engine — protected baskets ALWAYS prompt. So we pass
    // `Some(false)` as the override, which pins ctx.bundled_scope_grant=false
    // regardless of what the perm row says.
    //
    // For everything else, pass `None` so the builder reads the column from
    // the perm row directly.
    let bundled_override = match &scoped_call {
        ScopedCall::Basket { basket, .. } if is_protected_basket(basket) => Some(false),
        _ => None,
    };
    let ctx = context_builder::build_scoped_grant_context(
        call_kind,
        perm_row.as_ref(),
        scoped_grant_exists,
        bundled_override,
    );
    let decision = permission.decide(&ctx);

    match decision {
        PermissionDecision::Silent { .. } => {
            log::debug!(
                "🔓 engine Silent (scoped) for domain={} endpoint={} kind={:?}",
                domain, endpoint, call_kind,
            );
            GateOutcome::Proceed
        }
        PermissionDecision::Prompt { ref prompt_type, ref reason } => {
            let now = chrono::Utc::now().timestamp();
            let approval_id = permission.mint_pending_approval(&domain, endpoint, body, now);
            let prompt_payload = build_scoped_prompt_payload(&scoped_call);
            let envelope = build_pending_envelope(
                &approval_id,
                *prompt_type,
                reason_to_snake_case(reason),
                prompt_payload,
            );
            log::info!(
                "🛡️ engine Prompt (scoped) minted approval id={} for domain={} endpoint={} kind={:?}",
                approval_id, domain, endpoint, call_kind,
            );
            GateOutcome::EarlyReturn(HttpResponse::Accepted().json(envelope))
        }
        PermissionDecision::Deny { ref reason } => {
            log::warn!(
                "🛡️ engine Deny (scoped) for domain={} endpoint={} kind={:?} reason={:?}",
                domain, endpoint, call_kind, reason,
            );
            GateOutcome::EarlyReturn(forbidden_envelope(&reason_to_snake_case(reason)))
        }
    }
}

/// Build the promptPayload object per LD2 for a scoped prompt. Field shapes
/// must match `buildExtraParamsFromPayload` in C.3's
/// `HttpRequestInterceptor.cpp` (~L3760) so the React modal renders the
/// scope correctly.
fn build_scoped_prompt_payload(scoped_call: &ScopedCall<'_>) -> Value {
    match scoped_call {
        ScopedCall::Protocol { level, name, key_id, counterparty: None } => {
            serde_json::json!({
                "protocolLevel": level,
                "protocolName": name,
                "protocolKeyId": key_id,
                "protocolCounterparty": "",
            })
        }
        ScopedCall::Protocol { counterparty: Some(cp), .. } => {
            serde_json::json!({ "counterparty": cp })
        }
        ScopedCall::Basket { basket, access } => {
            serde_json::json!({
                "basket": basket,
                "basketAccess": access,
            })
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

    // -- Phase 2.6-D --

    #[test]
    fn scoped_call_classifies_protocol_use() {
        let c = ScopedCall::Protocol {
            level: 1,
            name: "tests",
            key_id: "1",
            counterparty: None,
        };
        assert!(matches!(c.call_kind(), CallKind::ProtocolUse));
    }

    #[test]
    fn scoped_call_classifies_counterparty_use() {
        let c = ScopedCall::Protocol {
            level: 2,
            name: "tests",
            key_id: "1",
            counterparty: Some("02abc"),
        };
        assert!(matches!(c.call_kind(), CallKind::CounterpartyUse));
    }

    #[test]
    fn scoped_call_classifies_basket_access() {
        let c = ScopedCall::Basket { basket: "test-basket", access: "read" };
        assert!(matches!(c.call_kind(), CallKind::BasketAccess));
    }

    #[test]
    fn protected_basket_default() {
        assert!(is_protected_basket("default"));
    }

    #[test]
    fn protected_basket_backup_prefix() {
        assert!(is_protected_basket("backup-2025-06-09"));
        assert!(is_protected_basket("backup-"));
    }

    #[test]
    fn protected_basket_admin_prefix() {
        assert!(is_protected_basket("admin certificates"));
        assert!(is_protected_basket("admin "));
    }

    #[test]
    fn protected_basket_user_named_is_not() {
        assert!(!is_protected_basket("my-basket"));
        assert!(!is_protected_basket("admin-basket")); // dash, no space → user-named
        assert!(!is_protected_basket("backupthing")); // no dash
        assert!(!is_protected_basket(""));
    }

    #[test]
    fn scoped_prompt_payload_protocol_use() {
        let c = ScopedCall::Protocol {
            level: 1,
            name: "tests",
            key_id: "1",
            counterparty: None,
        };
        let p = build_scoped_prompt_payload(&c);
        assert_eq!(p["protocolLevel"], 1);
        assert_eq!(p["protocolName"], "tests");
        assert_eq!(p["protocolKeyId"], "1");
        assert_eq!(p["protocolCounterparty"], "");
    }

    #[test]
    fn scoped_prompt_payload_counterparty_use() {
        let c = ScopedCall::Protocol {
            level: 2,
            name: "tests",
            key_id: "1",
            counterparty: Some("02abc"),
        };
        let p = build_scoped_prompt_payload(&c);
        assert_eq!(p["counterparty"], "02abc");
    }

    #[test]
    fn scoped_prompt_payload_basket_access() {
        let c = ScopedCall::Basket { basket: "tasks", access: "read_write" };
        let p = build_scoped_prompt_payload(&c);
        assert_eq!(p["basket"], "tasks");
        assert_eq!(p["basketAccess"], "read_write");
    }
}
