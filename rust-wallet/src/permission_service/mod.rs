//! permission_service — actix-integrated wrapper around the pure
//! `hodos_permission_engine` crate.
//!
//! Phase 2.6 layer that owns the migration-window state: the pending
//! approvals map (for the 202 PENDING re-issue flow), the migrated
//! SessionManager equivalent (lands in 2.6-E), the audit log writer, and
//! the shadow comparison writer.
//!
//! Module structure:
//!   - `flags`           — EngineFlags struct + env-var parser (Phase 2.6-C.2:
//!                          per-class booleans removed; only shadow flag survives)
//!   - `state`           — PermissionService struct + pending approvals
//!   - `audit`           — audit + shadow log write helpers
//!   - `context_builder` — request → PermissionContext (per-CallKind builders)
//!   - `request_gate`    — Phase 2.6-C.2 dispatch helper for the 4
//!                          privacy-perimeter handlers
//!   - `handlers`        — HTTP handlers (shadow-decide today; more in 2.6-D+)
//!
//! Phase 2.6-C.2: Privacy Perimeter migration complete. The four
//! privacy-perimeter handlers (`get_public_key` identityKey path,
//! `reveal_counterparty_key_linkage`, `reveal_specific_key_linkage`,
//! `prove_certificate` sensitive-field path) now flow through
//! `request_gate::dispatch_privacy_perimeter` → `PermissionService::decide`,
//! returning 200/202/403 per LD2.
//!
//! See: PHASE_2_6_ENGINE_TO_RUST.md + SUBPHASE_2_6_A_DESIGN.md
//! See: memory phase26-2-6-c-kickoff-2026-06-03

pub mod audit;
pub mod context_builder;
pub mod flags;
pub mod handlers;
pub mod request_gate;
pub mod state;

pub use flags::EngineFlags;
pub use request_gate::{dispatch_privacy_perimeter, GateOutcome};
pub use state::{ApprovalConsumeError, PendingApproval, PermissionService};
