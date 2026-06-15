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
//! individual `web::Data` extractors (rather than going through `AppState`).

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

/// Header name C++ injects on a certificate-disclosure replay carrying the
/// user-approved field set (a JSON array of field names). The cert replay
/// path narrows the request body to the approved subset, which breaks the
/// body-sha256 binding `consume_and_verify` relies on — so instead the gate
/// consumes the approval id (single-use) and checks `requested ⊆ approved`
/// against this header. C++-injected (trusted), never page-supplied on the
/// IPC resume path.
pub const X_CERT_APPROVED_FIELDS: &str = "X-Cert-Approved-Fields";

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

// ============================================================================
// Phase 2.6-F — certificate disclosure dispatch (non-sensitive)
// ============================================================================

/// Parse the `X-Cert-Approved-Fields` header into the user-approved field set.
/// The header value is a JSON array of field-name strings (C++-injected on the
/// cert replay). Returns `None` when the header is absent, empty, or malformed
/// — the caller then falls back to body-sha256 verification so a missing
/// header never weakens the gate below today's payment-style binding.
fn parse_approved_cert_fields(http_req: &HttpRequest) -> Option<Vec<String>> {
    http_req
        .headers()
        .get(X_CERT_APPROVED_FIELDS)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
}

/// True iff every requested field is contained in the approved set. Bounds a
/// cert replay's disclosure to exactly what the user checked, even if an
/// approval id were replayed with a tampered (expanded) body.
fn cert_fields_subset(requested: &[String], approved: &[String]) -> bool {
    requested.iter().all(|f| approved.contains(f))
}

/// Dispatch the engine gate for the non-sensitive `CertificateDisclosure`
/// CallKind (`/proveCertificate`). Mirrors `dispatch_scoped_grant`: domain
/// header → X-User-Approved replay → perm-row read → resolve "every requested
/// field already approved" against `cert_field_permissions` → engine decide →
/// 200/202/403.
///
/// Sensitive fields never reach here — the handler routes them through
/// `dispatch_privacy_perimeter` as `SensitiveCertField` (always prompt). The
/// `certificate_disclosure` promptType + 202-interception already exist
/// (built in 2.6-C for the sensitive path); this reuses them verbatim.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_cert_disclosure(
    permission: &Arc<PermissionService>,
    database: &Arc<Mutex<WalletDatabase>>,
    current_user_id: i64,
    http_req: &HttpRequest,
    body: &[u8],
    endpoint: &str,
    cert_type: &str,
    requested_fields: &[String],
    build_prompt_payload: impl FnOnce() -> Value,
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

    // X-User-Approved replay path. Unlike payment / scoped-grant replays, the
    // cert path CANNOT body-sha256-verify: C++ narrows the request body's
    // `fieldsToReveal` to the user-approved subset before re-issuing, so the
    // replayed body never matches the hash minted at prompt time (that was
    // the "first-approve 403 body_mismatch" bug). Instead, when C++ injects
    // the trusted `X-Cert-Approved-Fields` header, consume the approval id
    // single-use (TTL-checked, no body hash) and verify the replayed request's
    // `requested_fields ⊆ approved_fields`. This bounds disclosure to exactly
    // what the user checked even if an approval id were replayed with a
    // tampered (expanded) body. Persistence of "remember" selections stays in
    // the separate approve_cert_fields IPC, so no DB read here → no race.
    //
    // Defensive fallback: if the header is absent (should not happen on the
    // IPC resume path post-fix; the direct-fetch cert path is still C++ inline
    // and never reaches this branch), fall back to body-sha256 verification so
    // we never regress below the existing payment-style binding.
    if let Some(approval_id) = http_req
        .headers()
        .get(X_USER_APPROVED)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        let now = chrono::Utc::now().timestamp();

        let approved_fields: Option<Vec<String>> = parse_approved_cert_fields(http_req);

        if let Some(approved) = approved_fields {
            // Header present → id-only single-use consume + subset check.
            match permission.consume_pending_approval(approval_id, now) {
                Some(approval) => {
                    if cert_fields_subset(requested_fields, &approved) {
                        log::info!(
                            "🔐 X-User-Approved consumed (cert) for domain={} endpoint={} id={} \
                             requested={} approved={}",
                            approval.domain, approval.endpoint, approval_id,
                            requested_fields.len(), approved.len(),
                        );
                        return GateOutcome::Proceed;
                    }
                    log::warn!(
                        "🛡️ cert replay requested fields NOT a subset of approved \
                         (domain={} id={} requested={} approved={}) — denying",
                        domain, approval_id, requested_fields.len(), approved.len(),
                    );
                    return GateOutcome::EarlyReturn(forbidden_envelope("fields_not_approved"));
                }
                None => {
                    log::warn!(
                        "🛡️ X-User-Approved not consumable (cert, domain={} id={})",
                        domain, approval_id,
                    );
                    return GateOutcome::EarlyReturn(forbidden_envelope(
                        "approval_expired_or_consumed",
                    ));
                }
            }
        }

        // Fallback: no X-Cert-Approved-Fields header → body-sha256 verify.
        log::warn!(
            "🛡️ cert replay missing X-Cert-Approved-Fields (domain={} id={}) — \
             falling back to body-sha256 verification",
            domain, approval_id,
        );
        match permission.consume_and_verify(approval_id, body, now) {
            Ok(approval) => {
                log::info!(
                    "🔐 X-User-Approved consumed (cert, body-hash fallback) for \
                     domain={} endpoint={} id={}",
                    approval.domain, approval.endpoint, approval_id,
                );
                return GateOutcome::Proceed;
            }
            Err(ApprovalConsumeError::BodyMismatch) => {
                log::warn!(
                    "🛡️ X-User-Approved body sha256 mismatch (cert, domain={} id={})",
                    domain, approval_id,
                );
                return GateOutcome::EarlyReturn(forbidden_envelope("body_mismatch"));
            }
            Err(ApprovalConsumeError::Expired) | Err(ApprovalConsumeError::NotFound) => {
                log::warn!(
                    "🛡️ X-User-Approved not consumable (cert, domain={} id={})",
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
                    "🛡️ database mutex poisoned in dispatch_cert_disclosure: {} — denying",
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

    // Resolve "every requested field already approved" from cert_field_permissions
    // (under a fresh lock). Empty requested_fields was rejected by the handler.
    let all_fields_approved = if let Some(ref perm) = perm_row {
        let dp_id = perm.id.unwrap_or(0);
        if dp_id == 0 {
            false
        } else {
            let db_guard = match database.lock() {
                Ok(g) => g,
                Err(_) => {
                    return GateOutcome::EarlyReturn(internal_error_envelope("db_mutex_poisoned"));
                }
            };
            let repo = DomainPermissionRepository::new(db_guard.connection());
            match repo.get_approved_fields(dp_id, cert_type) {
                Ok(approved) => {
                    !requested_fields.is_empty()
                        && requested_fields.iter().all(|f| approved.contains(f))
                }
                Err(e) => {
                    log::error!(
                        "🛡️ get_approved_fields(dp_id={}, cert_type='{}') failed: {} — treating as no grant",
                        dp_id, cert_type, e
                    );
                    false
                }
            }
        }
    } else {
        false
    };

    let ctx = context_builder::build_cert_disclosure_context(perm_row.as_ref(), all_fields_approved);
    let decision = permission.decide(&ctx);

    match decision {
        PermissionDecision::Silent { .. } => {
            log::debug!(
                "🔓 engine Silent (cert) for domain={} endpoint={} cert_type={} fields={}",
                domain, endpoint, cert_type, requested_fields.len(),
            );
            GateOutcome::Proceed
        }
        PermissionDecision::Prompt { ref prompt_type, ref reason } => {
            let now = chrono::Utc::now().timestamp();
            let approval_id = permission.mint_pending_approval(&domain, endpoint, body, now);
            let envelope = build_pending_envelope(
                &approval_id,
                *prompt_type,
                reason_to_snake_case(reason),
                build_prompt_payload(),
            );
            log::info!(
                "🛡️ engine Prompt (cert) minted approval id={} for domain={} endpoint={} reason={}",
                approval_id, domain, endpoint, reason_to_snake_case(reason),
            );
            GateOutcome::EarlyReturn(HttpResponse::Accepted().json(envelope))
        }
        PermissionDecision::Deny { ref reason } => {
            log::warn!(
                "🛡️ engine Deny (cert) for domain={} endpoint={} reason={:?}",
                domain, endpoint, reason,
            );
            GateOutcome::EarlyReturn(forbidden_envelope(&reason_to_snake_case(reason)))
        }
    }
}

// ============================================================================
// Phase 2.6-G — domain-trust pre-gate
// ============================================================================

/// Async pre-gate resolving the domain-trust layer (Matrix C step 1) before the
/// kind-specific dispatch runs. Moves the unknown/blocked + manifest handling
/// out of the C++ HandleIpcWalletCall (G.4 removes the C++ side).
///
/// - internal origin (no `X-Requesting-Domain`) or approved trust → `Proceed`
///   (caller falls through to the payment/scoped/cert/privacy dispatch);
/// - blocked trust → Deny 403;
/// - unknown trust → fetch the dApp manifest; engine returns 202 with
///   `domain_approval` (no manifest) or `manifest_connect_bundle` (manifest
///   present; payload carries the served bytes verbatim).
///
/// Async because the unknown branch fetches the manifest. Approved/blocked
/// domains short-circuit before any fetch, so the common path stays cheap.
pub async fn domain_trust_gate(
    permission: &Arc<PermissionService>,
    database: &Arc<Mutex<WalletDatabase>>,
    current_user_id: i64,
    domain: &str,
    endpoint: &str,
) -> GateOutcome {
    // `domain` is the requesting origin (non-empty). The domain-trust actix
    // middleware (main.rs `domain_trust_mw`) is the sole caller: it passes
    // internal calls (no `X-Requesting-Domain`) straight through and only
    // invokes this for external origins, so we never see an empty domain here.
    // Running as middleware makes domain-trust universal — every external call
    // to any wallet endpoint is gated, with no per-handler opt-in to forget.
    //
    // NOTE: this gate is intentionally trust-ONLY — it never inspects or
    // consumes `X-User-Approved`. That token is owned exclusively by the
    // kind-specific dispatch (payment / scoped / cert), which runs AFTER this
    // pre-gate returns Proceed (inside the handler). An approved domain's
    // over-cap replay carries its kind-dispatch token; if this pre-gate
    // consumed it, the kind dispatch would then see NotFound and wrongly 403.
    // Domain-trust approval works by a different mechanism: the modal writes
    // trust=approved, so by re-issue time `get_by_domain` returns "approved"
    // and we Proceed without any token. (The C++ approval path writes trust
    // SYNCHRONOUSLY before re-issuing — see addDomainPermission.)

    let perm_row = {
        let db_guard = match database.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!(
                    "🛡️ database mutex poisoned in domain_trust_gate: {} — denying",
                    poisoned
                );
                return GateOutcome::EarlyReturn(internal_error_envelope("db_mutex_poisoned"));
            }
        };
        let repo = DomainPermissionRepository::new(db_guard.connection());
        match repo.get_by_domain(current_user_id, domain) {
            Ok(p) => p,
            Err(e) => {
                log::error!(
                    "🛡️ get_by_domain('{}') failed in domain_trust_gate: {} — denying",
                    domain, e
                );
                return GateOutcome::EarlyReturn(internal_error_envelope("db_read_error"));
            }
        }
    };

    let trust = perm_row.as_ref().map(|p| p.trust_level.as_str()).unwrap_or("");
    // Approved → fall through to the kind dispatch. Short-circuit so we never
    // fetch a manifest for an already-trusted domain.
    if trust == "approved" {
        return GateOutcome::Proceed;
    }

    // Unknown (not blocked) → fetch the manifest to choose the richer prompt.
    let manifest = if trust == "blocked" {
        None
    } else {
        crate::manifest::fetch_manifest(domain).await
    };
    let manifest_present = manifest.is_some();

    let ctx = context_builder::build_domain_trust_context(perm_row.as_ref(), manifest_present);
    let decision = permission.decide(&ctx);

    match decision {
        PermissionDecision::Silent { .. } => GateOutcome::Proceed,
        PermissionDecision::Prompt { ref prompt_type, ref reason } => {
            let now = chrono::Utc::now().timestamp();
            // Empty body: domain-trust approvals are never replayed via
            // X-User-Approved (the connect re-issue relies on trust=approved),
            // so the stored body_hash is never consumed. The approval entry
            // exists only to give the 202 envelope an approvalId.
            let approval_id = permission.mint_pending_approval(domain, endpoint, b"", now);
            let prompt_payload = if matches!(
                *prompt_type,
                hodos_permission_engine::PromptType::ManifestConnectBundle
            ) {
                serde_json::json!({
                    "manifest": manifest.as_ref().map(|m| m.raw_json.clone()).unwrap_or_default(),
                })
            } else {
                serde_json::json!({})
            };
            let envelope = build_pending_envelope(
                &approval_id,
                *prompt_type,
                reason_to_snake_case(reason),
                prompt_payload,
            );
            log::info!(
                "🛡️ engine Prompt (domain-trust) minted approval id={} for domain={} endpoint={} type={:?} reason={}",
                approval_id, domain, endpoint, prompt_type, reason_to_snake_case(reason),
            );
            GateOutcome::EarlyReturn(HttpResponse::Accepted().json(envelope))
        }
        PermissionDecision::Deny { ref reason } => {
            log::warn!(
                "🛡️ engine Deny (domain-trust) for domain={} endpoint={} reason={:?}",
                domain, endpoint, reason,
            );
            GateOutcome::EarlyReturn(forbidden_envelope(&reason_to_snake_case(reason)))
        }
    }
}

// ============================================================================
// Phase 2.6-E — payment dispatch
// ============================================================================

/// Payload the caller passes into `dispatch_payment`. Bundles the per-request
/// cost (satoshis + cents — caller computes cents from satoshis × BSV price
/// at call time so the engine sees a consistent denominator) and the browser
/// session this payment belongs to.
///
/// `browser_id` is the CEF browser identifier the C++ side captured in the
/// `X-Browser-Id` header. SessionCounters are keyed by this id; missing or
/// invalid header → browser_id=0 which still works (the counters just live
/// under a single shared slot, matching C++'s default-when-unknown shape).
#[derive(Debug, Clone, Copy)]
pub struct PaymentCall {
    pub satoshis: i64,
    /// USD cents at the BSV/USD rate this call observed. Caller computes
    /// from `satoshis × price_cents_per_bsv / 100_000_000` (BRC-100 wallets'
    /// standard formula). When the price cache is missing/stale, caller
    /// passes 0 and sets `bsv_price_available = false`.
    pub cents: i64,
    /// True iff caller had a usable BSV/USD price for this call. When false,
    /// engine returns Prompt with reason=price_unavailable.
    pub bsv_price_available: bool,
    /// CEF browser identifier of the originating tab. Maps to the C++
    /// SessionManager `browserId` key. Falls back to 0 when the C++ side
    /// can't supply one (purely defensive — the engine path always sets it).
    pub browser_id: i32,
}

/// Header name carrying the CEF browser identifier on outbound payment
/// calls. C++ injects this on every wallet_call that flows through the IPC
/// bridge so Rust's session counters key by the same identifier C++'s
/// SessionManager does.
pub const X_BROWSER_ID: &str = "X-Browser-Id";

/// Header carrying the satoshi cost C++ extracted from the request body.
/// LD4 keeps the satoshi/cents derivation on the C++ side (it already does
/// it for the green-dot IPC); Rust just reads the result.
pub const X_PAYMENT_SATOSHIS: &str = "X-Payment-Satoshis";

/// Header carrying the USD cents cost C++ computed via BSVPriceCache. When
/// `X-Bsv-Price-Available: 0`, this is 0 and the engine prompts
/// price_unavailable.
pub const X_PAYMENT_CENTS: &str = "X-Payment-Cents";

/// Header signaling whether C++'s BSVPriceCache had a usable price when
/// the call left the browser. "1" → available, "0" → unavailable.
pub const X_BSV_PRICE_AVAILABLE: &str = "X-Bsv-Price-Available";

impl PaymentCall {
    /// Extract a PaymentCall from the request headers C++ injects on payment
    /// endpoints. Returns `None` when the request lacks the headers C++
    /// adds on the engine path — caller treats that as an internal call and
    /// proceeds without a payment gate (mirrors the no-`X-Requesting-Domain`
    /// internal-bypass behavior).
    pub fn from_headers(http_req: &HttpRequest) -> Option<Self> {
        let satoshis = http_req
            .headers()
            .get(X_PAYMENT_SATOSHIS)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<i64>().ok())?;
        let cents = http_req
            .headers()
            .get(X_PAYMENT_CENTS)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let bsv_price_available = http_req
            .headers()
            .get(X_BSV_PRICE_AVAILABLE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s == "1")
            .unwrap_or(true);
        let browser_id = http_req
            .headers()
            .get(X_BROWSER_ID)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0);
        Some(PaymentCall {
            satoshis,
            cents,
            bsv_price_available,
            browser_id,
        })
    }
}

/// Dispatch the engine gate for the Payment CallKind. Mirrors
/// `dispatch_scoped_grant`'s structure: replay path, perm-row read, context
/// build, engine decide, 200/202/403 translation.
///
/// On Silent (auto-approve): increments the per-browser-per-domain payment
/// + rate counters (C++ used to do this at gate-decision time in
/// `incrementRateCounter` + `incrementPaymentCount` at
/// `HttpRequestInterceptor.cpp:2552-2553`). Caller invokes
/// `record_spending` separately AFTER the wallet processes the payment.
///
/// On X-User-Approved replay: counters are NOT re-incremented (already
/// counted on the original Silent attempt the user later modified). Replay
/// just proceeds.
///
/// Phase 2.6-E.fix1: PaymentCall headers (X-Payment-Satoshis, X-Payment-Cents,
/// X-Bsv-Price-Available, X-Browser-Id) are read INTERNALLY rather than
/// passed in by the caller. The X-User-Approved replay path doesn't need
/// them — the original engine decision already validated the body hash —
/// so it short-circuits before PaymentCall is read. The engine path (no
/// X-User-Approved header) reads PaymentCall and falls back to a
/// price-unavailable Prompt when the headers are missing (which forces a
/// modal so the user has the final say). Prior to .fix1, missing headers
/// caused the caller to skip dispatch_payment entirely, leaving X-User-Approved
/// unconsumed and the over-cap defense-in-depth check in the handler
/// rejecting the user-approved payment with 403.
///
/// Defense-in-depth: caller still runs `check_domain_approved` after
/// `Proceed` (same contract as scoped-grant and privacy-perimeter). The
/// caller MUST skip the spending-limit portion of its own defense check
/// when `X-User-Approved` is present in the request — by the time we
/// return Proceed via the replay path, the engine has consumed the
/// approval (body-hash bound at mint time) and the user explicitly
/// approved this exact payment.
pub fn dispatch_payment(
    permission: &Arc<PermissionService>,
    database: &Arc<Mutex<WalletDatabase>>,
    current_user_id: i64,
    http_req: &HttpRequest,
    body: &[u8],
    endpoint: &str,
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

    // X-User-Approved replay path. Approved payments skip the counter
    // increment because they were already counted on the original Silent
    // attempt the user then chose to modify. Replay's only purpose is to
    // bypass the cap check that flagged the original. PaymentCall headers
    // are intentionally NOT required here — only the engine path needs them.
    if let Some(approval_id) = http_req
        .headers()
        .get(X_USER_APPROVED)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        let now = chrono::Utc::now().timestamp();
        match permission.consume_and_verify(approval_id, body, now) {
            Ok(approval) => {
                // Phase 2.6-E.fix2 (B1/B3): a prompted-then-approved payment was
                // NOT counted at prompt time (only Silent decisions count). Record
                // its spend + tx count now so cumulative session limits include
                // user-approved over-cap payments. cents/browser_id come from the
                // approval (stashed at mint time — the replay lacks X-Payment-*).
                permission.record_spending(approval.browser_id, &approval.domain, approval.cents, now);
                permission.increment_payment_rate_counter(approval.browser_id, &approval.domain, now);
                log::info!(
                    "🔐 X-User-Approved consumed (payment) for domain={} endpoint={} id={} recorded_cents={}",
                    approval.domain, approval.endpoint, approval_id, approval.cents,
                );
                return GateOutcome::Proceed;
            }
            Err(ApprovalConsumeError::BodyMismatch) => {
                log::warn!(
                    "🛡️ X-User-Approved body sha256 mismatch (payment, domain={} id={})",
                    domain, approval_id,
                );
                return GateOutcome::EarlyReturn(forbidden_envelope("body_mismatch"));
            }
            Err(ApprovalConsumeError::Expired) | Err(ApprovalConsumeError::NotFound) => {
                log::warn!(
                    "🛡️ X-User-Approved not consumable (payment, domain={} id={})",
                    domain, approval_id,
                );
                return GateOutcome::EarlyReturn(forbidden_envelope(
                    "approval_expired_or_consumed",
                ));
            }
        }
    }

    // Engine path — fresh payment, no replay. Read PaymentCall now. When the
    // C++ side forgot to inject the X-Payment-* headers (regression bug or
    // direct curl), fall back to a price_unavailable shape that forces the
    // engine to Prompt — better to ask the user than to silently bypass caps
    // by treating the call as 0 cents.
    let payment = match PaymentCall::from_headers(http_req) {
        Some(p) => p,
        None => {
            log::warn!(
                "🛡️ dispatch_payment: payment call from {} has no X-Payment-* headers — forcing price_unavailable prompt (C++ injection bug or non-engine caller)",
                domain
            );
            PaymentCall {
                satoshis: 0,
                cents: 0,
                bsv_price_available: false,
                browser_id: 0,
            }
        }
    };

    let perm_row = {
        let db_guard = match database.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!(
                    "🛡️ database mutex poisoned in dispatch_payment: {} — denying",
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

    let now = chrono::Utc::now().timestamp();
    let counters = permission.get_session_counters_snapshot(payment.browser_id, &domain, now);

    let ctx = context_builder::build_payment_context(
        perm_row.as_ref(),
        payment.cents,
        payment.bsv_price_available,
        counters.spent_cents,
        counters.payment_requests_this_minute,
        counters.payment_count_this_session,
    );
    let decision = permission.decide(&ctx);

    match decision {
        PermissionDecision::Silent { .. } => {
            // C++ incremented rate + count at gate time (Open path L2552-2553).
            // Mirror that here so a Silent decision bumps the in-window counters
            // immediately — subsequent calls in the same minute see the updated
            // count without waiting for the response cycle.
            permission.increment_payment_rate_counter(payment.browser_id, &domain, now);
            // Phase 2.6-E.fix2 (B1): record the session spend so the cumulative
            // per-session DOLLAR cap actually enforces (previously dead —
            // record_spending had no production caller, so session_spent_cents
            // stayed 0). Recorded at gate-Silent time alongside the rate/count
            // counters — conservative: a payment that later fails to build still
            // counts toward the cap, the safe direction for a spending guard.
            permission.record_spending(payment.browser_id, &domain, payment.cents, now);
            log::debug!(
                "🔓 engine Silent (payment) for domain={} endpoint={} cents={} sats={} session_spent_now={}",
                domain, endpoint, payment.cents, payment.satoshis,
                permission.get_session_counters_snapshot(payment.browser_id, &domain, now).spent_cents,
            );
            GateOutcome::Proceed
        }
        PermissionDecision::Prompt { ref prompt_type, ref reason } => {
            let approval_id = permission.mint_pending_payment_approval(
                &domain, endpoint, body, now, payment.cents, payment.browser_id,
            );
            let prompt_payload = build_payment_prompt_payload(
                &payment,
                &ctx,
                &counters,
                prompt_type,
            );
            let envelope = build_pending_envelope(
                &approval_id,
                *prompt_type,
                reason_to_snake_case(reason),
                prompt_payload,
            );
            log::info!(
                "🛡️ engine Prompt (payment) minted approval id={} for domain={} endpoint={} reason={}",
                approval_id, domain, endpoint, reason_to_snake_case(reason),
            );
            GateOutcome::EarlyReturn(HttpResponse::Accepted().json(envelope))
        }
        PermissionDecision::Deny { ref reason } => {
            log::warn!(
                "🛡️ engine Deny (payment) for domain={} endpoint={} reason={:?}",
                domain, endpoint, reason,
            );
            GateOutcome::EarlyReturn(forbidden_envelope(&reason_to_snake_case(reason)))
        }
    }
}

/// Build the LD2 promptPayload for a payment prompt. Mirrors the C++
/// extraParams string built in `HttpRequestInterceptor.cpp` (~L3243-3253 for
/// the Open path, ~L2689-2700 for the IPC path) so the React modal renders
/// the same amount + limit fields.
///
/// `exceededLimit` translates the engine reason into the legacy URL-param
/// vocabulary the React modal consumes via `URLSearchParams`. The C++→React
/// translator at the modal-open seam reads these LD2 promptPayload fields
/// and emits the legacy query string.
fn build_payment_prompt_payload(
    payment: &PaymentCall,
    ctx: &hodos_permission_engine::PermissionContext,
    counters: &super::state::SessionCounters,
    prompt_type: &PromptType,
) -> Value {
    use hodos_permission_engine::PromptType as P;
    let exceeded = match prompt_type {
        P::PaymentConfirmation => {
            // PriceUnavailable / PerTxLimit / SessionCap all surface here.
            if !payment.bsv_price_available {
                "price_unavailable"
            } else {
                let over_tx = payment.cents > ctx.per_tx_limit_cents;
                let over_session =
                    counters.spent_cents + payment.cents > ctx.per_session_limit_cents;
                if over_tx && over_session {
                    "both"
                } else if over_tx {
                    "per_tx"
                } else {
                    "per_session"
                }
            }
        }
        P::RateLimitExceeded => {
            // RateLimit vs MaxTxPerSession both arrive on rate_limit_exceeded.
            if counters.payment_count_this_session as i64 >= ctx.max_tx_per_session
                && ctx.max_tx_per_session > 0
            {
                "session_tx_count"
            } else {
                "rate_limit"
            }
        }
        _ => "",
    };

    let mut payload = serde_json::json!({
        "satoshis": payment.satoshis,
        "cents": payment.cents,
        "bsvPrice": 0, // C++ supplied a numeric price; React already handles 0 + price_unavailable display.
        "exceededLimit": exceeded,
        "perTxLimit": ctx.per_tx_limit_cents,
        "perSessionLimit": ctx.per_session_limit_cents,
        "sessionSpent": counters.spent_cents,
    });
    if matches!(prompt_type, P::RateLimitExceeded) {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("rateLimit".to_string(), Value::from(ctx.rate_limit_per_min));
            obj.insert(
                "maxTxPerSession".to_string(),
                Value::from(ctx.max_tx_per_session),
            );
            obj.insert(
                "txCount".to_string(),
                Value::from(counters.payment_count_this_session),
            );
        }
    }
    payload
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

    // -- Phase 2.6-E -- payment dispatch helpers

    use super::super::state::SessionCounters;
    use hodos_permission_engine::PermissionContext;

    fn payment_ctx_with_caps(
        per_tx: i64,
        per_session: i64,
        rate: i64,
        max_tx: i64,
    ) -> PermissionContext {
        PermissionContext {
            per_tx_limit_cents: per_tx,
            per_session_limit_cents: per_session,
            rate_limit_per_min: rate,
            max_tx_per_session: max_tx,
            ..Default::default()
        }
    }

    fn counters_with(spent: i64, rate: i32, count: i32) -> SessionCounters {
        SessionCounters {
            domain: "example.com".to_string(),
            spent_cents: spent,
            payment_requests_this_minute: rate,
            payment_count_this_session: count,
            minute_window_start: 0,
        }
    }

    #[test]
    fn payment_prompt_payload_per_tx_exceeded() {
        let payment = PaymentCall {
            satoshis: 100_000,
            cents: 150,
            bsv_price_available: true,
            browser_id: 1,
        };
        let ctx = payment_ctx_with_caps(100, 1000, 0, 0);
        let counters = counters_with(0, 0, 0);
        let p = build_payment_prompt_payload(
            &payment,
            &ctx,
            &counters,
            &PromptType::PaymentConfirmation,
        );
        assert_eq!(p["satoshis"], 100_000);
        assert_eq!(p["cents"], 150);
        assert_eq!(p["exceededLimit"], "per_tx");
        assert_eq!(p["perTxLimit"], 100);
        assert_eq!(p["perSessionLimit"], 1000);
        assert_eq!(p["sessionSpent"], 0);
    }

    #[test]
    fn payment_prompt_payload_session_cap_exceeded() {
        let payment = PaymentCall {
            satoshis: 50_000,
            cents: 50,
            bsv_price_available: true,
            browser_id: 1,
        };
        let ctx = payment_ctx_with_caps(100, 100, 0, 0);
        let counters = counters_with(75, 0, 0);
        let p = build_payment_prompt_payload(
            &payment,
            &ctx,
            &counters,
            &PromptType::PaymentConfirmation,
        );
        assert_eq!(p["exceededLimit"], "per_session");
        assert_eq!(p["sessionSpent"], 75);
    }

    #[test]
    fn payment_prompt_payload_both_caps_exceeded() {
        let payment = PaymentCall {
            satoshis: 200_000,
            cents: 250,
            bsv_price_available: true,
            browser_id: 1,
        };
        let ctx = payment_ctx_with_caps(100, 200, 0, 0);
        let counters = counters_with(50, 0, 0);
        let p = build_payment_prompt_payload(
            &payment,
            &ctx,
            &counters,
            &PromptType::PaymentConfirmation,
        );
        assert_eq!(p["exceededLimit"], "both");
    }

    #[test]
    fn payment_prompt_payload_price_unavailable() {
        let payment = PaymentCall {
            satoshis: 50_000,
            cents: 0,
            bsv_price_available: false,
            browser_id: 1,
        };
        let ctx = payment_ctx_with_caps(100, 1000, 0, 0);
        let counters = counters_with(0, 0, 0);
        let p = build_payment_prompt_payload(
            &payment,
            &ctx,
            &counters,
            &PromptType::PaymentConfirmation,
        );
        assert_eq!(p["exceededLimit"], "price_unavailable");
        assert_eq!(p["cents"], 0);
    }

    #[test]
    fn payment_prompt_payload_rate_limit_carries_extra_fields() {
        let payment = PaymentCall {
            satoshis: 1000,
            cents: 1,
            bsv_price_available: true,
            browser_id: 1,
        };
        let ctx = payment_ctx_with_caps(100, 1000, 5, 50);
        let counters = counters_with(0, 5, 3);
        let p = build_payment_prompt_payload(
            &payment,
            &ctx,
            &counters,
            &PromptType::RateLimitExceeded,
        );
        assert_eq!(p["exceededLimit"], "rate_limit");
        assert_eq!(p["rateLimit"], 5);
        assert_eq!(p["maxTxPerSession"], 50);
        assert_eq!(p["txCount"], 3);
    }

    #[test]
    fn payment_prompt_payload_session_tx_count_classified() {
        let payment = PaymentCall {
            satoshis: 1000,
            cents: 1,
            bsv_price_available: true,
            browser_id: 1,
        };
        let ctx = payment_ctx_with_caps(100, 1000, 30, 5);
        // count >= max_tx_per_session — should classify as session_tx_count
        // (not rate_limit).
        let counters = counters_with(0, 0, 5);
        let p = build_payment_prompt_payload(
            &payment,
            &ctx,
            &counters,
            &PromptType::RateLimitExceeded,
        );
        assert_eq!(p["exceededLimit"], "session_tx_count");
    }

    // -- Phase 2.6-F.fix -- cert-replay subset + header parse

    use actix_web::test::TestRequest;

    fn fields(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn cert_subset_exact_match_passes() {
        // The common case: body narrowed to exactly the approved set.
        assert!(cert_fields_subset(&fields(&["name", "email"]), &fields(&["name", "email"])));
    }

    #[test]
    fn cert_subset_proper_subset_passes() {
        assert!(cert_fields_subset(&fields(&["email"]), &fields(&["name", "email"])));
    }

    #[test]
    fn cert_subset_empty_requested_passes() {
        // Vacuously true; the handler separately rejects empty fieldsToReveal.
        assert!(cert_fields_subset(&[], &fields(&["name"])));
    }

    #[test]
    fn cert_subset_extra_field_denied() {
        // Replay body requests a field the user did NOT approve → must fail.
        assert!(!cert_fields_subset(&fields(&["name", "ssn"]), &fields(&["name"])));
    }

    #[test]
    fn cert_subset_empty_approved_denies_nonempty() {
        assert!(!cert_fields_subset(&fields(&["name"]), &[]));
    }

    #[test]
    fn parse_approved_fields_valid_json_array() {
        let req = TestRequest::default()
            .insert_header((X_CERT_APPROVED_FIELDS, r#"["name","email"]"#))
            .to_http_request();
        assert_eq!(parse_approved_cert_fields(&req), Some(fields(&["name", "email"])));
    }

    #[test]
    fn parse_approved_fields_absent_is_none() {
        let req = TestRequest::default().to_http_request();
        assert_eq!(parse_approved_cert_fields(&req), None);
    }

    #[test]
    fn parse_approved_fields_empty_is_none() {
        let req = TestRequest::default()
            .insert_header((X_CERT_APPROVED_FIELDS, ""))
            .to_http_request();
        assert_eq!(parse_approved_cert_fields(&req), None);
    }

    #[test]
    fn parse_approved_fields_malformed_is_none() {
        // Not a JSON array → None → caller falls back to body-sha256.
        let req = TestRequest::default()
            .insert_header((X_CERT_APPROVED_FIELDS, "name,email"))
            .to_http_request();
        assert_eq!(parse_approved_cert_fields(&req), None);
    }

    #[test]
    fn parse_approved_fields_empty_array_is_some_empty() {
        // A well-formed empty array is distinct from absent: header present,
        // zero approved fields → any non-empty requested set is denied.
        let req = TestRequest::default()
            .insert_header((X_CERT_APPROVED_FIELDS, "[]"))
            .to_http_request();
        assert_eq!(parse_approved_cert_fields(&req), Some(vec![]));
    }
}
