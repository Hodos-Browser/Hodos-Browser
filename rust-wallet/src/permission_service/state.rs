//! `PermissionService` — actix-integrated wrapper around the pure
//! `hodos_permission_engine` crate.
//!
//! Holds the migration-window state that the pure engine doesn't (pending
//! approvals map for the 202 PENDING re-issue flow, session counters that
//! migrate from the C++ SessionManager in sub-phase 2.6-E, audit/shadow
//! write helpers).
//!
//! Phase 2.6-A.5 status: scaffolding only. Module is dormant — nothing calls
//! `decide()` from production yet. AppState wiring lands in 2.6-A.6, shadow
//! infrastructure lands in 2.6-B, real per-CallKind dispatch lands in 2.6-C
//! through 2.6-G.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use hodos_permission_engine::{decide as engine_decide, PermissionContext, PermissionDecision};

use super::flags::EngineFlags;

/// In-flight approval awaiting user resolution.
///
/// Held in `PermissionService::pending_approvals` keyed by `approval_id`.
/// On the X-User-Approved re-issue, the handler looks up the entry, verifies
/// it hasn't been consumed, and processes the original call. Single-use —
/// re-using a consumed approvalId returns 403.
#[derive(Debug, Clone)]
pub struct PendingApproval {
    /// 128-bit hex nonce (32 chars) — the same value returned in the 202
    /// PENDING body's `approvalId` field.
    pub approval_id: String,
    pub domain: String,
    pub endpoint: String,
    /// sha256 hex of the original request body for replay verification.
    pub body_hash: String,
    /// Unix timestamp seconds when the approval was minted.
    pub created_at: i64,
    /// Unix timestamp seconds when the approval becomes invalid.
    /// Default TTL is 10 minutes (600s) per LD2.
    pub expires_at: i64,
}

/// `PermissionService` — the actix-integrated layer above the pure engine.
///
/// One instance lives on `AppState.permission` (wired up in sub-phase 2.6-A.6).
/// Constructor reads `EngineFlags::from_env()` so per-class migration flags
/// are visible at request time.
pub struct PermissionService {
    /// Per-CallKind-class flags. Read at startup; immutable for the process
    /// lifetime. Flag flip requires a wallet restart.
    flags: EngineFlags,

    /// Pending approvals map indexed by `approval_id`. RwLock because reads
    /// (lookup on X-User-Approved re-issue) are far more common than writes
    /// (initial mint + atomic consume on re-issue).
    pending_approvals: Arc<RwLock<HashMap<String, PendingApproval>>>,
}

impl PermissionService {
    /// Construct a new PermissionService. Takes the env-derived flags; caller
    /// is responsible for env loading at startup (done in main.rs in 2.6-A.6).
    pub fn new(flags: EngineFlags) -> Self {
        Self {
            flags,
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Return the engine flags. Used by request handlers to decide which
    /// engine path (C++ via existing handlers, or Rust via this service) to
    /// take.
    pub fn flags(&self) -> EngineFlags {
        self.flags
    }

    /// Pure-logic decision. Delegates to `hodos_permission_engine::decide`.
    ///
    /// This is the SAME function that the shadow infrastructure calls in
    /// sub-phase 2.6-B. The Rust engine produces a decision either as the
    /// shadow comparison (flag OFF) or as the authoritative decision (flag
    /// ON). Same code path, different consumer.
    pub fn decide(&self, ctx: &PermissionContext) -> PermissionDecision {
        engine_decide(ctx)
    }

    /// Insert a new pending approval. Used by the 2.6-C+ handlers when the
    /// engine returns a Prompt decision and we need to remember the call for
    /// the eventual re-issue.
    pub fn insert_pending_approval(&self, approval: PendingApproval) {
        let mut guard = self
            .pending_approvals
            .write()
            .expect("pending_approvals lock poisoned");
        guard.insert(approval.approval_id.clone(), approval);
    }

    /// Look up and atomically consume a pending approval by id. Returns the
    /// approval if it exists and hasn't expired; `None` if it doesn't exist,
    /// already consumed, or has expired.
    ///
    /// Single-use semantics per LD2: a successful lookup removes the entry.
    /// A leaked approvalId can't be reused.
    pub fn consume_pending_approval(
        &self,
        approval_id: &str,
        now: i64,
    ) -> Option<PendingApproval> {
        let mut guard = self
            .pending_approvals
            .write()
            .expect("pending_approvals lock poisoned");
        let approval = guard.remove(approval_id)?;
        if approval.expires_at < now {
            // Expired — drop it. We still remove from the map so it can't be
            // reused even by a clock that moves backward.
            return None;
        }
        Some(approval)
    }

    /// Number of currently-pending approvals. For dev sanity / metrics.
    pub fn pending_approval_count(&self) -> usize {
        self.pending_approvals
            .read()
            .expect("pending_approvals lock poisoned")
            .len()
    }

    /// Background-task hook: drop all approvals with `expires_at < now`.
    /// Called periodically by a 2.6-A.6+ monitor task.
    pub fn purge_expired_approvals(&self, now: i64) -> usize {
        let mut guard = self
            .pending_approvals
            .write()
            .expect("pending_approvals lock poisoned");
        let before = guard.len();
        guard.retain(|_, v| v.expires_at >= now);
        before - guard.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_approval(id: &str, expires_at: i64) -> PendingApproval {
        PendingApproval {
            approval_id: id.to_string(),
            domain: "example.com".to_string(),
            endpoint: "/createAction".to_string(),
            body_hash: "0".repeat(64),
            created_at: 1_700_000_000,
            expires_at,
        }
    }

    #[test]
    fn new_service_has_no_pending_approvals() {
        let svc = PermissionService::new(EngineFlags::default());
        assert_eq!(svc.pending_approval_count(), 0);
        assert!(!svc.flags().any_enabled());
    }

    #[test]
    fn insert_and_consume_single_use() {
        let svc = PermissionService::new(EngineFlags::default());
        svc.insert_pending_approval(sample_approval("abc", 1_700_000_600));
        assert_eq!(svc.pending_approval_count(), 1);

        // First consume succeeds.
        let consumed = svc.consume_pending_approval("abc", 1_700_000_100);
        assert!(consumed.is_some());
        assert_eq!(consumed.unwrap().approval_id, "abc");
        assert_eq!(svc.pending_approval_count(), 0);

        // Second consume returns None — single-use semantics.
        assert!(svc.consume_pending_approval("abc", 1_700_000_100).is_none());
    }

    #[test]
    fn expired_approval_returns_none_on_consume() {
        let svc = PermissionService::new(EngineFlags::default());
        svc.insert_pending_approval(sample_approval("abc", 100));
        let consumed = svc.consume_pending_approval("abc", 999); // now > expires_at
        assert!(consumed.is_none());
        // Entry still removed even though expired — prevents replay.
        assert_eq!(svc.pending_approval_count(), 0);
    }

    #[test]
    fn purge_expired_drops_only_expired_entries() {
        let svc = PermissionService::new(EngineFlags::default());
        svc.insert_pending_approval(sample_approval("old", 100));
        svc.insert_pending_approval(sample_approval("fresh", 1_700_000_600));
        let purged = svc.purge_expired_approvals(500);
        assert_eq!(purged, 1);
        assert_eq!(svc.pending_approval_count(), 1);
        // The fresh one is still consumable.
        assert!(svc.consume_pending_approval("fresh", 600).is_some());
    }

    #[test]
    fn decide_delegates_to_pure_engine() {
        use hodos_permission_engine::{CallKind, PermissionContext, TrustLevel};

        let svc = PermissionService::new(EngineFlags::default());
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Blocked,
            ..Default::default()
        };
        let d = svc.decide(&ctx);
        assert!(d.is_deny(), "blocked trust should deny");
    }
}
