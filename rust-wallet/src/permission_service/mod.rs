//! permission_service — actix-integrated wrapper around the pure
//! `hodos_permission_engine` crate.
//!
//! Phase 2.6 layer that owns the migration-window state: the pending
//! approvals map (for the 202 PENDING re-issue flow), the migrated
//! SessionManager equivalent (lands in 2.6-E), the audit log writer, and
//! the shadow comparison writer.
//!
//! Module structure:
//!   - `flags`           — EngineFlags struct + env-var parser
//!   - `state`           — PermissionService struct + pending approvals
//!   - `audit`           — audit + shadow log write helpers
//!   - `context_builder` — request → PermissionContext (placeholder in 2.6-A.5)
//!   - `handlers`        — HTTP handlers (placeholder; lands in 2.6-B+)
//!
//! Phase 2.6-A.5 status: scaffolding only. Module is dormant — nothing in
//! production calls `PermissionService::decide()` yet. AppState wiring lands
//! in 2.6-A.6.
//!
//! See: PHASE_2_6_ENGINE_TO_RUST.md + SUBPHASE_2_6_A_DESIGN.md

pub mod audit;
pub mod context_builder;
pub mod flags;
pub mod handlers;
pub mod state;

pub use flags::EngineFlags;
pub use state::{PendingApproval, PermissionService};
