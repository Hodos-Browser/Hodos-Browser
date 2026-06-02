//! HTTP handlers exposed by the permission_service module.
//!
//! Phase 2.6-A.5: **placeholder**. The real handlers land later:
//!   - `POST /engine/shadow-decide` in 2.6-B (shadow comparison endpoint)
//!   - `GET /engine/audit-stats` in 2.6-A.6+ (optional CLI-friendly summary)
//!
//! Routes are NOT registered in main.rs yet — that happens in 2.6-A.6 (with
//! the shadow-decide route gated behind a feature/setting since it shouldn't
//! be reachable until 2.6-B's writer lands).

// Intentionally empty for 2.6-A.5. Documented placeholder so the module
// structure is in place when 2.6-B starts.
