//! HTTP endpoint handlers for the adblock engine service.
//!
//! Endpoints:
//! - GET  /health  — lifecycle status ("loading" or "ready")
//! - POST /check   — check if a URL should be blocked
//! - GET  /status  — full engine status (enabled, list count, rules, etc.)
//! - POST /toggle  — enable/disable ad blocking globally

use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::engine::AdblockEngine;

// ============================================================================
// Health Check
// ============================================================================

/// GET /health — Engine lifecycle status
///
/// Response: { "status": "ready" } or { "status": "loading" }
/// C++ polls this during startup to know when the engine is ready.
pub async fn health(engine: web::Data<AdblockEngine>) -> HttpResponse {
    let status = engine.get_engine_status();
    HttpResponse::Ok().json(serde_json::json!({
        "status": status
    }))
}

// ============================================================================
// Request Check
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckRequest {
    pub url: String,
    pub source_url: String,
    pub resource_type: String,
}

/// POST /check — Check if a URL should be blocked
///
/// Request:  { "url": "...", "sourceUrl": "...", "resourceType": "script" }
/// Response: { "blocked": true, "filter": "||ads.example.com^", "redirect": null }
pub async fn check(
    engine: web::Data<AdblockEngine>,
    body: web::Json<CheckRequest>,
) -> HttpResponse {
    let (blocked, redirect, filter) = engine.check_request(
        &body.url,
        &body.source_url,
        &body.resource_type,
    );

    HttpResponse::Ok().json(serde_json::json!({
        "blocked": blocked,
        "redirect": redirect,
        "filter": filter,
        "version": engine.get_update_version(),
    }))
}

// ============================================================================
// Status
// ============================================================================

/// GET /status — Full engine status
///
/// Response: { "enabled": true, "status": "ready", "listCount": 2,
///             "totalRules": 85000, "lastUpdate": 1708700000, "lists": [...] }
pub async fn status(engine: web::Data<AdblockEngine>) -> HttpResponse {
    let status = engine.get_status();
    HttpResponse::Ok().json(serde_json::json!({
        "enabled": status.enabled,
        "status": status.status,
        "listCount": status.list_count,
        "totalRules": status.total_rules,
        "lastUpdate": status.last_update,
        "updateVersion": status.update_version,
        "lists": status.lists.iter().map(|l| serde_json::json!({
            "filename": l.filename,
            "url": l.url,
            "downloadedAt": l.downloaded_at,
            "sizeBytes": l.size_bytes,
            "ruleCount": l.rule_count,
            "expiresAt": l.expires_at,
        })).collect::<Vec<_>>(),
    }))
}

// ============================================================================
// Cosmetic Resources
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CosmeticRequest {
    pub url: String,
    /// When true, return empty injectedScript (user disabled scriptlets for this domain)
    #[serde(default)]
    pub skip_scriptlets: bool,
}

/// POST /cosmetic-resources — Get CSS selectors and scriptlets for a URL
///
/// Request:  { "url": "https://www.youtube.com/watch?v=xyz", "skipScriptlets": false }
/// Response: { "hideSelectors": [...], "injectedScript": "...", "generichide": false }
pub async fn cosmetic_resources(
    engine: web::Data<AdblockEngine>,
    body: web::Json<CosmeticRequest>,
) -> HttpResponse {
    let (selectors, injected_script, generichide) = engine.cosmetic_resources(&body.url);

    // If user disabled scriptlets for this domain, return empty injectedScript
    let final_script = if body.skip_scriptlets {
        String::new()
    } else {
        injected_script
    };

    HttpResponse::Ok().json(serde_json::json!({
        "hideSelectors": selectors,
        "injectedScript": final_script,
        "generichide": generichide,
    }))
}

// ============================================================================
// Hidden Class/ID Selectors (Phase 2 cosmetic filtering)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct HiddenIdsRequest {
    pub url: String,
    pub classes: Vec<String>,
    pub ids: Vec<String>,
}

/// POST /cosmetic-hidden-ids — Get generic selectors matching DOM classes/IDs
///
/// Two-phase cosmetic filtering:
/// 1. `/cosmetic-resources` returns hostname-specific selectors (Phase 1)
/// 2. This endpoint returns generic selectors matching actual DOM elements (Phase 2)
///
/// Request:  { "url": "https://...", "classes": ["ad-slot", "sponsored"], "ids": ["banner"] }
/// Response: { "selectors": [".ad-slot", "#banner > .wrapper"] }
pub async fn cosmetic_hidden_ids(
    engine: web::Data<AdblockEngine>,
    body: web::Json<HiddenIdsRequest>,
) -> HttpResponse {
    let selectors = engine.hidden_class_id_selectors(&body.url, &body.classes, &body.ids);

    HttpResponse::Ok().json(serde_json::json!({
        "selectors": selectors,
    }))
}

// ============================================================================
// Toggle
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ToggleRequest {
    pub enabled: bool,
}

/// POST /toggle — Enable/disable ad blocking globally
///
/// Request:  { "enabled": false }
/// Response: { "enabled": false }
pub async fn toggle(
    engine: web::Data<AdblockEngine>,
    body: web::Json<ToggleRequest>,
) -> HttpResponse {
    engine.set_enabled(body.enabled);
    HttpResponse::Ok().json(serde_json::json!({
        "enabled": engine.is_enabled()
    }))
}
