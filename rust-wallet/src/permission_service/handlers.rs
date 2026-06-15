//! HTTP handlers exposed by the permission_service module.
//!
//! These are the small session-cache endpoints called fire-and-forget from
//! C++: `/wallet/session-approve`, `/wallet/session-revoke`, and
//! `/wallet/session/close`. The actual permission decisions live in
//! `request_gate` (dispatched from the main handlers); this file only manages
//! the in-memory session opt-in / payment-counter state.
//!
//! (Phase 2.6-H removed `/engine/shadow-decide` + the shadow-log infrastructure
//! along with the C++ PermissionEngine.)

use std::sync::Arc;

use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;

use super::state::PermissionService;

// ============================================================================
// Phase 2.6-C.4 follow-up — POST /wallet/session-approve
// ============================================================================
//
// Mirrors the C++ Mark*RevealApproved entry points (cef-native/src/core/
// HttpRequestInterceptor.cpp:1345-1350). Called fire-and-forget from C++ after
// the user clicks Approve on an identity_key_reveal / key_linkage_reveal
// modal. Updates the matching in-memory session cache on PermissionService
// so build_privacy_perimeter_context returns Silent on subsequent calls
// from the same origin within the same wallet-process lifetime.
//
// Wire shape:
//   POST /wallet/session-approve
//   Content-Type: application/json
//   { "domain": "example.com", "kind": "identity_key" | "key_linkage" }
//
// Response: 200 with { "status": "approved" } on success, 400 on malformed
// body or unknown kind. No auth (localhost-only callable, matches the rest of
// the wallet API surface).

#[derive(Debug, Deserialize)]
pub struct SessionApproveRequest {
    pub domain: String,
    pub kind: String,
}

/// `POST /wallet/session-approve`. See module-level comment for wire shape.
pub async fn session_approve(
    permission: web::Data<Arc<PermissionService>>,
    body: web::Json<serde_json::Value>,
) -> impl Responder {
    let req: SessionApproveRequest = match serde_json::from_value(body.into_inner()) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid body: {}", e),
                "status": "bad_request"
            }));
        }
    };
    if req.domain.trim().is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "domain is required",
            "status": "bad_request"
        }));
    }
    match req.kind.as_str() {
        "identity_key" => {
            permission.approve_identity_key_session(&req.domain);
            log::info!(
                "🛡️ session-approve: identity_key for {} (Rust cache updated)",
                req.domain
            );
        }
        "key_linkage" => {
            permission.approve_key_linkage_session(&req.domain);
            log::info!(
                "🛡️ session-approve: key_linkage for {} (Rust cache updated)",
                req.domain
            );
        }
        other => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Unknown kind '{}', expected 'identity_key' or 'key_linkage'", other),
                "status": "bad_request"
            }));
        }
    }
    HttpResponse::Ok().json(serde_json::json!({
        "status": "approved",
        "domain": req.domain,
        "kind": req.kind
    }))
}

// ============================================================================
// Phase 2.6-C.4 follow-up — POST /wallet/session-revoke
// ============================================================================
//
// Parallel to /wallet/session-approve. Called fire-and-forget from C++'s
// revokeIdentityKeyApprovalForDomain + revokeKeyLinkageApprovalForDomain
// (HttpRequestInterceptor.cpp), which fire when the user revokes a domain's
// permissions from the wallet UI or right-click "Manage Site Permissions".
//
// Drops BOTH session cache entries for the supplied domain — matches the C++
// side where the `domain_permission_invalidate` IPC always invokes both
// revoke functions together (the cache choice is at the engine level, not
// per-revoke). Idempotent: revoking a never-approved domain is a 200.
//
// Wire shape:
//   POST /wallet/session-revoke
//   Content-Type: application/json
//   { "domain": "example.com" }

#[derive(Debug, Deserialize)]
pub struct SessionRevokeRequest {
    pub domain: String,
}

/// `POST /wallet/session-revoke`. See module-level comment for wire shape.
pub async fn session_revoke(
    permission: web::Data<Arc<PermissionService>>,
    body: web::Json<serde_json::Value>,
) -> impl Responder {
    let req: SessionRevokeRequest = match serde_json::from_value(body.into_inner()) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid body: {}", e),
                "status": "bad_request"
            }));
        }
    };
    if req.domain.trim().is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "domain is required",
            "status": "bad_request"
        }));
    }
    permission.revoke_session_approvals_for_domain(&req.domain);
    log::info!(
        "🛡️ session-revoke: dropped identity_key + key_linkage session approvals for {} (Rust caches cleared)",
        req.domain
    );
    HttpResponse::Ok().json(serde_json::json!({
        "status": "revoked",
        "domain": req.domain
    }))
}

// ============================================================================
// Phase 2.6-E — POST /wallet/session/close
// ============================================================================
//
// Fired fire-and-forget from C++ when a tab closes (`TabManager::CloseTab`).
// Drops the matching browser_id's payment session counters from
// PermissionService — mirrors C++'s SessionManager::clearSession exactly so
// reopening a tab to the same domain starts fresh (same UX as
// session_spent reset on tab close that has shipped since Phase 1).
//
// Wire shape:
//   POST /wallet/session/close
//   Content-Type: application/json
//   { "browser_id": 42 }
//
// Idempotent: closing an unknown browser_id is a 200.

#[derive(Debug, Deserialize)]
pub struct SessionCloseRequest {
    pub browser_id: i32,
}

pub async fn session_close(
    permission: web::Data<Arc<PermissionService>>,
    body: web::Json<serde_json::Value>,
) -> impl Responder {
    let req: SessionCloseRequest = match serde_json::from_value(body.into_inner()) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid body: {}", e),
                "status": "bad_request"
            }));
        }
    };
    permission.clear_session_for_browser(req.browser_id);
    log::info!(
        "🛡️ session/close: cleared payment session counters for browser_id={} (Rust counters dropped)",
        req.browser_id
    );
    HttpResponse::Ok().json(serde_json::json!({
        "status": "closed",
        "browser_id": req.browser_id
    }))
}
