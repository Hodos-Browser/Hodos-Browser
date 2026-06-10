//! HTTP handlers exposed by the permission_service module.
//!
//! Phase 2.6-B.2: **`/engine/shadow-decide`** lands here.
//!
//! Wire shape (must stay in lockstep with C++ caller at
//! `cef-native/src/core/EngineShadow.cpp`):
//!
//! ```text
//! POST /engine/shadow-decide
//! Content-Type: application/json
//! {
//!   "context": { /* PermissionContext, snake_case */ },
//!   "cpp_decision": "silent" | "prompt" | "deny",
//!   "cpp_prompt_type": "<string>" | null,
//!   "cpp_reason": "<string>" | null
//! }
//! ```
//!
//! Response: always `200 OK` (or `204 No Content` when the shadow flag is
//! OFF). The C++ side is fire-and-forget and **discards the body** — we still
//! emit a tiny JSON envelope for any future curl-based debugging.
//!
//! Hard invariants:
//!   1. Never returns an error status. C++ doesn't read the body; bubbling 4xx
//!      or 5xx up to the worker thread would only confuse log output.
//!   2. Off-by-default behind `HODOS_ENGINE_SHADOW_LOG=1`. When the flag is
//!      OFF, the handler short-circuits BEFORE the JSON deserialize — a
//!      malformed body in shadow-OFF state is still 204, not 400.
//!   3. All DB errors are logged and swallowed. A failing SQLite write must
//!      not block a future shadow POST or the C++ wallet call that triggered it.
//!
//! The handler is dormant from a production-traffic perspective in 2.6-B.2 —
//! the C++ caller exists (2.6-B.1) but no production code site fires it yet
//! (that's 2.6-B.3). End-to-end smoke is via `curl` with a hand-crafted body.

use std::sync::{Arc, Mutex};

use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;

use hodos_permission_engine::PermissionContext;

use crate::database::{EngineShadowRepository, WalletDatabase};

use super::audit;
use super::state::PermissionService;

/// POST body for `/engine/shadow-decide`. Mirrors the JSON envelope built by
/// `cef-native/src/core/EngineShadow.cpp::buildEnvelope`.
#[derive(Debug, Deserialize)]
pub struct ShadowDecideRequest {
    pub context: PermissionContext,
    pub cpp_decision: String,
    #[serde(default)]
    pub cpp_prompt_type: Option<String>,
    #[serde(default)]
    pub cpp_reason: Option<String>,
}

/// `POST /engine/shadow-decide` handler. Per the contract above, ALWAYS
/// returns a 2xx. Errors are logged and dropped so the C++ critical path is
/// never affected by Rust-side issues.
///
/// Takes `permission` and `database` as individual `web::Data` extractors
/// (rather than going through `AppState`) so this module can live in `lib.rs`
/// without forcing `AppState` to move into the lib crate. main.rs registers
/// matching `.app_data(...)` entries alongside the existing AppState data.
pub async fn shadow_decide(
    permission: web::Data<Arc<PermissionService>>,
    database: web::Data<Arc<Mutex<WalletDatabase>>>,
    body: web::Json<serde_json::Value>,
) -> impl Responder {
    // Gate 1: shadow flag must be ON. When OFF we short-circuit BEFORE any
    // JSON validation — the C++ side may have HODOS_ENGINE_SHADOW_LOG set
    // even when the Rust side hasn't been restarted with it; that mismatch
    // must be a silent no-op, not a 400.
    if !permission.flags().shadow_log_enabled {
        return HttpResponse::NoContent().finish();
    }

    // Gate 2: parse the envelope. A malformed body when shadow is ON is a
    // genuine wire-shape bug — log it loudly but still return 200 so the
    // worker thread doesn't see a failure.
    let req: ShadowDecideRequest = match serde_json::from_value(body.into_inner()) {
        Ok(r) => r,
        Err(e) => {
            log::warn!(
                "🧪 [engine-shadow] malformed POST envelope: {} — dropping",
                e
            );
            return HttpResponse::Ok().json(serde_json::json!({"status": "malformed"}));
        }
    };

    // Run the Rust engine against the same context the C++ engine just
    // consumed. This is the entire reason the endpoint exists.
    let rust_decision = permission.decide(&req.context);

    // Build the row that goes into engine_shadow_log. `build_shadow_entry`
    // already encapsulates the agreement-vs-disagreement logic and was
    // unit-tested in 2.6-A.5.
    let entry = audit::build_shadow_entry(
        &req.cpp_decision,
        req.cpp_prompt_type.as_deref(),
        req.cpp_reason.as_deref(),
        &rust_decision,
        req.context.call_kind,
        &req.context,
        // C++ doesn't echo domain/endpoint in the shadow envelope (LD2 — Rust
        // is supposed to derive these from the request the user already has on
        // the C++ side). Until 2.6-B.3 wires real call sites that thread these
        // in, the shadow entry records empty placeholders. Disagreement
        // analytics (`call_kind_class`, `agreement`, `context_hash`) still
        // work; only the per-domain/per-endpoint breakdown is unavailable.
        "",
        "",
        chrono::Utc::now().timestamp(),
    );

    // Persist. SQLite write happens under the database mutex; lock scope is
    // tight — no `.await` while holding it. Any error is logged and dropped.
    let insert_result = {
        let db = match database.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::warn!(
                    "🧪 [engine-shadow] database mutex poisoned: {} — dropping write",
                    poisoned
                );
                return HttpResponse::Ok().json(serde_json::json!({"status": "db_lock_poisoned"}));
            }
        };
        let repo = EngineShadowRepository::new(db.connection());
        repo.insert(&entry)
    };

    match insert_result {
        Ok(rowid) => {
            // Disagreements are the diagnostic signal — log them at INFO so
            // a `grep engine-shadow.*disagree` finds them in dev logs.
            // Agreements stay at DEBUG to keep the steady-state log volume low.
            if entry.agreement == 0 {
                log::info!(
                    "🧪 [engine-shadow] DISAGREE class={} cpp={} rust={} (cpp_reason={:?} rust_reason={:?}) rowid={}",
                    entry.call_kind_class,
                    entry.cpp_decision,
                    entry.rust_decision,
                    entry.cpp_reason,
                    entry.rust_reason,
                    rowid,
                );
            } else {
                log::debug!(
                    "🧪 [engine-shadow] agree class={} decision={} rowid={}",
                    entry.call_kind_class,
                    entry.cpp_decision,
                    rowid,
                );
            }
            HttpResponse::Ok().json(serde_json::json!({"status": "logged", "rowid": rowid}))
        }
        Err(e) => {
            log::warn!("🧪 [engine-shadow] insert failed: {} — dropping", e);
            HttpResponse::Ok().json(serde_json::json!({"status": "db_error"}))
        }
    }
}

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
