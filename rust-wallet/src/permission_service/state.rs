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

use super::audit::body_hash;
use super::flags::EngineFlags;

/// Default TTL for a minted pending approval (10 minutes).
///
/// Matches the C++ `kPromptAuthTimeoutMs = 600_000` in
/// `cef-native/src/core/HttpRequestInterceptor.cpp`. Per LD2: "approvalId — …
/// single-use, 10-minute TTL".
pub const APPROVAL_TTL_SECS: i64 = 600;

/// Outcome of `PermissionService::consume_and_verify`.
///
/// Maps to LD2's 403 `reason` values that the C++ side or test smoke can
/// surface in error envelopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalConsumeError {
    /// No pending approval with that ID — never minted, already consumed, or
    /// expired and dropped by `purge_expired_approvals`. Maps to 403
    /// `approval_expired_or_consumed`.
    NotFound,
    /// The approval existed but `expires_at < now`. Also dropped from the map
    /// (single-use semantics — see `consume_pending_approval`). Maps to 403
    /// `approval_expired_or_consumed`.
    Expired,
    /// The approval existed and was fresh, but the supplied body's sha256
    /// does not match the body hash stored at mint time. Indicates the client
    /// changed the request body between the initial Prompt and the
    /// X-User-Approved replay — must reject. Maps to 403 `body_mismatch`.
    BodyMismatch,
}

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

    /// Mint a new pending approval for a Prompt decision and return its id.
    ///
    /// Phase 2.6-C.2 entry point for the 4 privacy-perimeter handlers. Builds
    /// a fresh 128-bit hex id, computes the body sha256, and inserts the
    /// PendingApproval with a 10-minute TTL (LD2 `approvalId.ttlMs = 600000`).
    ///
    /// The returned id is what the 202 PENDING envelope's `approvalId` field
    /// carries to the client. The client (C++) opens the modal, and on user
    /// approval re-issues the original request with
    /// `X-User-Approved: <approval_id>`. Rust's `consume_and_verify` then
    /// looks up the entry, verifies the body hash, and atomically removes it.
    pub fn mint_pending_approval(
        &self,
        domain: &str,
        endpoint: &str,
        body: &[u8],
        now: i64,
    ) -> String {
        let approval_id = generate_approval_id();
        let approval = PendingApproval {
            approval_id: approval_id.clone(),
            domain: domain.to_string(),
            endpoint: endpoint.to_string(),
            body_hash: body_hash(body),
            created_at: now,
            expires_at: now + APPROVAL_TTL_SECS,
        };
        self.insert_pending_approval(approval);
        approval_id
    }

    /// Look up, verify body hash, and atomically consume a pending approval.
    ///
    /// Phase 2.6-C.2 entry point for the X-User-Approved replay path. Per
    /// kickoff Q5 (a): body sha256 must match the value stored at mint time;
    /// mismatch returns 403 `body_mismatch`. Per LD2: the approval is
    /// single-use — a successful consume removes the entry from the map.
    ///
    /// Important: on `Expired` and `BodyMismatch` the entry is still removed
    /// from the map. For Expired this matches `consume_pending_approval`'s
    /// existing semantics (prevents replay even if the clock moves backward).
    /// For BodyMismatch it prevents an attacker from probing for the correct
    /// body by holding the approval_id open across many guesses.
    pub fn consume_and_verify(
        &self,
        approval_id: &str,
        body: &[u8],
        now: i64,
    ) -> Result<PendingApproval, ApprovalConsumeError> {
        let mut guard = self
            .pending_approvals
            .write()
            .expect("pending_approvals lock poisoned");
        let approval = match guard.remove(approval_id) {
            Some(a) => a,
            None => return Err(ApprovalConsumeError::NotFound),
        };
        // Drop the lock once we've taken ownership of the approval — the
        // remaining checks are pure.
        drop(guard);

        if approval.expires_at < now {
            return Err(ApprovalConsumeError::Expired);
        }
        let observed_hash = body_hash(body);
        if observed_hash != approval.body_hash {
            return Err(ApprovalConsumeError::BodyMismatch);
        }
        Ok(approval)
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

/// Generate a 128-bit hex-encoded approval id (32 lowercase hex chars).
///
/// Matches LD2: `rand::random::<u128>()` formatted as 32-char hex. CSPRNG
/// source is whatever `rand::random` picks (typically `ThreadRng`,
/// `getrandom` under the hood). Collision probability across the lifetime
/// of the process is negligible.
fn generate_approval_id() -> String {
    format!("{:032x}", rand::random::<u128>())
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
        assert!(!svc.flags().shadow_log_enabled);
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

    // ---------- Phase 2.6-C.2: mint / consume_and_verify ----------

    #[test]
    fn generate_approval_id_is_32_hex_chars() {
        let id = generate_approval_id();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn generate_approval_id_is_unique_across_calls() {
        // Collision probability for 128 random bits across 100 draws is
        // ~2.9e-37 — effectively zero.
        let mut ids = std::collections::HashSet::new();
        for _ in 0..100 {
            assert!(ids.insert(generate_approval_id()));
        }
    }

    #[test]
    fn mint_pending_approval_inserts_and_returns_id() {
        let svc = PermissionService::new(EngineFlags::default());
        let id = svc.mint_pending_approval(
            "example.com",
            "/getPublicKey",
            b"{\"identityKey\":true}",
            1_700_000_000,
        );
        assert_eq!(id.len(), 32);
        assert_eq!(svc.pending_approval_count(), 1);
    }

    #[test]
    fn mint_then_consume_with_matching_body_succeeds() {
        let svc = PermissionService::new(EngineFlags::default());
        let body = b"{\"identityKey\":true}";
        let id = svc.mint_pending_approval("example.com", "/getPublicKey", body, 1_700_000_000);
        let result = svc.consume_and_verify(&id, body, 1_700_000_100);
        let approval = result.expect("expected Ok on matching body");
        assert_eq!(approval.approval_id, id);
        assert_eq!(approval.domain, "example.com");
        assert_eq!(approval.endpoint, "/getPublicKey");
        // Single-use: second consume yields NotFound.
        assert!(matches!(
            svc.consume_and_verify(&id, body, 1_700_000_100),
            Err(ApprovalConsumeError::NotFound)
        ));
    }

    #[test]
    fn consume_with_mismatched_body_returns_body_mismatch() {
        let svc = PermissionService::new(EngineFlags::default());
        let body = b"{\"identityKey\":true}";
        let tampered = b"{\"identityKey\":false}";
        let id = svc.mint_pending_approval("example.com", "/getPublicKey", body, 1_700_000_000);
        assert!(matches!(
            svc.consume_and_verify(&id, tampered, 1_700_000_100),
            Err(ApprovalConsumeError::BodyMismatch)
        ));
        // Approval removed even on mismatch — prevents probe-for-body attacks.
        assert_eq!(svc.pending_approval_count(), 0);
    }

    #[test]
    fn consume_after_expiry_returns_expired() {
        let svc = PermissionService::new(EngineFlags::default());
        let body = b"{}";
        let id = svc.mint_pending_approval("example.com", "/getPublicKey", body, 1_000);
        // now > created_at + APPROVAL_TTL_SECS
        let late = 1_000 + APPROVAL_TTL_SECS + 1;
        assert!(matches!(
            svc.consume_and_verify(&id, body, late),
            Err(ApprovalConsumeError::Expired)
        ));
        assert_eq!(svc.pending_approval_count(), 0);
    }

    #[test]
    fn consume_unknown_id_returns_not_found() {
        let svc = PermissionService::new(EngineFlags::default());
        assert!(matches!(
            svc.consume_and_verify("0".repeat(32).as_str(), b"{}", 1_700_000_100),
            Err(ApprovalConsumeError::NotFound)
        ));
    }

    #[test]
    fn ttl_constant_matches_ld2_10_minutes() {
        // Guards against silent drift from the LD2 contract.
        assert_eq!(APPROVAL_TTL_SECS, 600);
    }
}
