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

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use hodos_permission_engine::{decide as engine_decide, PermissionContext, PermissionDecision};

use super::audit::body_hash;

/// Default TTL for a minted pending approval (10 minutes).
///
/// Matches the C++ `kPromptAuthTimeoutMs = 600_000` in
/// `cef-native/src/core/HttpRequestInterceptor.cpp`. Per LD2: "approvalId — …
/// single-use, 10-minute TTL".
pub const APPROVAL_TTL_SECS: i64 = 600;

/// Rate-limit sliding window length in seconds.
///
/// Matches C++ SessionManager's 60s window: payment requests beyond
/// `rate_limit_per_min` within this window trigger the rate_limit_exceeded
/// prompt. Window resets the moment elapsed >= 60s.
pub const RATE_LIMIT_WINDOW_SECS: i64 = 60;

/// Per-(browser_id, domain) session counter snapshot.
///
/// Phase 2.6-E mirror of C++'s `BrowserSession` struct in
/// `cef-native/include/core/SessionManager.h`. Keyed by browser_id in the
/// outer map; `domain` lives inside the value and a domain change for the
/// same browser_id resets spent/rate counters (matches C++'s `getSession`
/// reset-on-domain-change semantics).
///
/// All four counter fields feed into `PermissionContext` for `decide_payment`:
/// `spent_cents` → `session_spent_cents`, `payment_count_this_session` →
/// same name, `payment_requests_this_minute` → same name (after window
/// expiry check).
#[derive(Debug, Clone, Default)]
pub struct SessionCounters {
    /// Domain this counter snapshot applies to. Domain-mismatch on the same
    /// browser_id triggers a reset (mirrors C++).
    pub domain: String,
    /// Total USD cents spent on this (browser_id, domain) session.
    /// Incremented by `record_spending` after Silent decisions land.
    pub spent_cents: i64,
    /// Total payment transactions on this (browser_id, domain) session.
    /// Incremented alongside the rate counter.
    pub payment_count_this_session: i32,
    /// Payment requests issued in the current 60s rate window. Window
    /// boundary lives in `minute_window_start`.
    pub payment_requests_this_minute: i32,
    /// Unix epoch seconds of the start of the current 60s rate window.
    /// `payment_requests_this_minute` resets when (now - this) >= 60.
    pub minute_window_start: i64,
}

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
    /// Phase 2.6-E.fix2 — payment amount (USD cents) this approval covers,
    /// stashed at mint time so the X-User-Approved replay can record session
    /// spend without the (fix1-absent) X-Payment-* headers. 0 for non-payment.
    pub cents: i64,
    /// Phase 2.6-E.fix2 — browser/session id, so the replay records spend in
    /// the same (browser_id, domain) bucket as the original prompt. 0 for
    /// non-payment approvals.
    pub browser_id: i32,
}

/// `PermissionService` — the actix-integrated layer above the pure engine.
///
/// One instance lives on `AppState.permission` (wired up in sub-phase 2.6-A.6).
pub struct PermissionService {
    /// Pending approvals map indexed by `approval_id`. RwLock because reads
    /// (lookup on X-User-Approved re-issue) are far more common than writes
    /// (initial mint + atomic consume on re-issue).
    pending_approvals: Arc<RwLock<HashMap<String, PendingApproval>>>,

    /// Per-domain session-level "Allow once / Allow this session" opt-in for
    /// IdentityKeyReveal. Phase 2.6-C.4 follow-up: migrated from C++'s
    /// IdentityKeyApprovalCache. Populated by `POST /wallet/session-approve`
    /// (called from C++ MarkIdentityKeyRevealApproved); consulted in
    /// build_privacy_perimeter_context so the engine returns Silent on
    /// subsequent identity-key calls from the same origin without
    /// re-prompting. Cleared on wallet restart (in-memory only — session
    /// scope by design).
    identity_key_session_approvals: Arc<RwLock<HashSet<String>>>,

    /// Same as identity_key_session_approvals but for the two BRC-72 key-
    /// linkage CallKinds (CounterpartyKeyLinkage + SpecificKeyLinkage).
    /// Populated by `POST /wallet/session-approve` with kind=key_linkage
    /// (called from C++ MarkKeyLinkageRevealApproved).
    key_linkage_session_approvals: Arc<RwLock<HashSet<String>>>,

    /// Phase 2.6-E — per-browser payment session counters. Migrated from
    /// C++'s `SessionManager` singleton in
    /// `cef-native/include/core/SessionManager.h`. Read at payment-gate
    /// time by `build_payment_context`; written by `record_spending` and
    /// `increment_payment_rate_counter` after Silent decisions land. Cleared
    /// per-browser via `clear_session_for_browser` (fired from
    /// `POST /wallet/session/close` when C++ closes a tab).
    ///
    /// Key is the CEF browser identifier (i32). One entry per browser;
    /// `domain` lives inside the value and a domain mismatch on the same
    /// browser triggers a counter reset (matches C++'s `getSession`).
    ///
    /// Phase 2.6-E intentionally leaves C++'s `SessionManager` alive for
    /// the BRC-121 paid-retry path (per OQ5 in
    /// `PHASE_2_6_ENGINE_TO_RUST.md` — BRC-121 cascade migration is post-
    /// 2.6 polish). So this Rust map and the C++ SessionManager are
    /// independent counter spaces during this phase. The engine-driven
    /// path (createAction / signAction / processAction / acquireCertificate
    /// / sendMessage / send_transaction) reads + writes Rust; the BRC-121
    /// path keeps reading + writing C++.
    session_counters: Arc<RwLock<HashMap<i32, SessionCounters>>>,
}

impl PermissionService {
    /// Construct a new PermissionService.
    pub fn new() -> Self {
        Self {
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            identity_key_session_approvals: Arc::new(RwLock::new(HashSet::new())),
            key_linkage_session_approvals: Arc::new(RwLock::new(HashSet::new())),
            session_counters: Arc::new(RwLock::new(HashMap::new())),
        }
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
        // Non-payment approvals (scoped grant, privacy perimeter, cert) carry
        // no spend amount — cents=0, browser_id=0.
        self.mint_pending_payment_approval(domain, endpoint, body, now, 0, 0)
    }

    /// Phase 2.6-E.fix2 — payment variant of `mint_pending_approval` that also
    /// stashes the payment amount (USD cents) and browser/session id on the
    /// approval. The X-User-Approved replay path lacks the X-Payment-* headers
    /// (fix1), so it reads these back from the approval to record session spend
    /// + tx count for a prompted-then-approved over-cap payment (B1/B3).
    pub fn mint_pending_payment_approval(
        &self,
        domain: &str,
        endpoint: &str,
        body: &[u8],
        now: i64,
        cents: i64,
        browser_id: i32,
    ) -> String {
        let approval_id = generate_approval_id();
        let approval = PendingApproval {
            approval_id: approval_id.clone(),
            domain: domain.to_string(),
            endpoint: endpoint.to_string(),
            body_hash: body_hash(body),
            created_at: now,
            expires_at: now + APPROVAL_TTL_SECS,
            cents,
            browser_id,
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

    // ------------------------------------------------------------------------
    // Phase 2.6-C.4 follow-up — session opt-in caches
    // ------------------------------------------------------------------------
    //
    // Migrated from C++'s IdentityKeyApprovalCache + KeyLinkageApprovalCache so
    // build_privacy_perimeter_context can consult them and return Silent on
    // subsequent calls from a session-opted-in origin. Populated via
    // `POST /wallet/session-approve` which C++ fires after the user clicks
    // Approve on an identity_key_reveal / key_linkage_reveal modal.

    /// Approve identity-key disclosure for `domain` for the rest of this
    /// wallet session. Idempotent — repeated calls are no-ops.
    pub fn approve_identity_key_session(&self, domain: &str) {
        let mut guard = self
            .identity_key_session_approvals
            .write()
            .expect("identity_key_session_approvals lock poisoned");
        guard.insert(domain.to_string());
    }

    /// Returns true iff identity-key disclosure has been session-approved
    /// for `domain`. Consulted by build_privacy_perimeter_context for the
    /// IdentityKeyReveal CallKind.
    pub fn is_identity_key_session_approved(&self, domain: &str) -> bool {
        let guard = self
            .identity_key_session_approvals
            .read()
            .expect("identity_key_session_approvals lock poisoned");
        guard.contains(domain)
    }

    /// Approve BRC-72 key-linkage reveal for `domain` for the rest of this
    /// wallet session. Covers both CounterpartyKeyLinkage and
    /// SpecificKeyLinkage CallKinds (matches C++'s single
    /// KeyLinkageApprovalCache scope).
    pub fn approve_key_linkage_session(&self, domain: &str) {
        let mut guard = self
            .key_linkage_session_approvals
            .write()
            .expect("key_linkage_session_approvals lock poisoned");
        guard.insert(domain.to_string());
    }

    /// Returns true iff key-linkage reveal has been session-approved for
    /// `domain`. Consulted by build_privacy_perimeter_context for the
    /// CounterpartyKeyLinkage / SpecificKeyLinkage CallKinds.
    pub fn is_key_linkage_session_approved(&self, domain: &str) -> bool {
        let guard = self
            .key_linkage_session_approvals
            .read()
            .expect("key_linkage_session_approvals lock poisoned");
        guard.contains(domain)
    }

    /// Drop both session-opt-in entries for `domain`. Called when a domain's
    /// permission is revoked from the wallet UI (matches C++'s
    /// revokeIdentityKeyApprovalForDomain + revokeKeyLinkageApprovalForDomain
    /// semantics). Idempotent — domain absent from either cache is a no-op.
    pub fn revoke_session_approvals_for_domain(&self, domain: &str) {
        {
            let mut guard = self
                .identity_key_session_approvals
                .write()
                .expect("identity_key_session_approvals lock poisoned");
            guard.remove(domain);
        }
        {
            let mut guard = self
                .key_linkage_session_approvals
                .write()
                .expect("key_linkage_session_approvals lock poisoned");
            guard.remove(domain);
        }
    }

    // ------------------------------------------------------------------------
    // Phase 2.6-E — payment session counters
    // ------------------------------------------------------------------------
    //
    // Mirror of C++ SessionManager in cef-native/include/core/SessionManager.h.
    // Read by build_payment_context, written by record_spending +
    // increment_payment_rate_counter, cleared by clear_session_for_browser.

    /// Read current counters for (browser_id, domain). Domain-mismatch on the
    /// same browser_id returns a zeroed snapshot — matches C++'s `getSession`
    /// behavior where navigating to a new origin resets spent/rate counters.
    ///
    /// Side effect: a missing entry is NOT created (engine reads are
    /// non-mutating). Counters materialize on the first `record_spending` or
    /// `increment_payment_rate_counter` call.
    ///
    /// `now` is unix epoch seconds. Used to expire the rate window.
    pub fn get_session_counters_snapshot(
        &self,
        browser_id: i32,
        domain: &str,
        now: i64,
    ) -> SessionCounters {
        let guard = self
            .session_counters
            .read()
            .expect("session_counters lock poisoned");
        match guard.get(&browser_id) {
            Some(c) if c.domain == domain => {
                // Apply 60s rate window expiry on read so the engine sees the
                // current count, not the pre-expiry one. Mirrors C++'s
                // checkRateLimit + incrementRateCounter window logic.
                let effective_rate = if now - c.minute_window_start >= RATE_LIMIT_WINDOW_SECS {
                    0
                } else {
                    c.payment_requests_this_minute
                };
                SessionCounters {
                    domain: c.domain.clone(),
                    spent_cents: c.spent_cents,
                    payment_count_this_session: c.payment_count_this_session,
                    payment_requests_this_minute: effective_rate,
                    minute_window_start: c.minute_window_start,
                }
            }
            _ => SessionCounters {
                domain: domain.to_string(),
                ..Default::default()
            },
        }
    }

    /// Record `cents` of spending for (browser_id, domain). Called after the
    /// engine returns Silent and the wallet processes the payment. Creates a
    /// new session entry on first call; mutates the existing one on subsequent
    /// calls (resetting spent + rate counters if the domain changed).
    ///
    /// Mirrors C++'s `SessionManager::recordSpending` semantics — adds to the
    /// existing entry without resetting. The reset-on-domain-change happens
    /// inside `get_or_create_for_write` (called here) which mirrors C++'s
    /// `getSession` behavior.
    pub fn record_spending(&self, browser_id: i32, domain: &str, cents: i64, now: i64) {
        let mut guard = self
            .session_counters
            .write()
            .expect("session_counters lock poisoned");
        let entry = Self::get_or_create_for_write(&mut guard, browser_id, domain, now);
        entry.spent_cents += cents;
    }

    /// Increment the per-session payment count + per-minute rate counter for
    /// (browser_id, domain). Called after the engine returns Silent for a
    /// payment — mirrors the C++ `incrementRateCounter` + `incrementPaymentCount`
    /// pair that fires together at gate-decision time.
    ///
    /// Handles the 60s rate window: if the window has expired, resets the
    /// minute counter to 1 (this call) and restarts the window. Otherwise
    /// increments the in-window counter.
    pub fn increment_payment_rate_counter(&self, browser_id: i32, domain: &str, now: i64) {
        let mut guard = self
            .session_counters
            .write()
            .expect("session_counters lock poisoned");
        let entry = Self::get_or_create_for_write(&mut guard, browser_id, domain, now);

        // Rate window expiry — matches C++ SessionManager::incrementRateCounter
        // L60-68. Window reset BEFORE the increment so this call is counted in
        // the new window, not as the last call of the expired one.
        if now - entry.minute_window_start >= RATE_LIMIT_WINDOW_SECS {
            entry.payment_requests_this_minute = 0;
            entry.minute_window_start = now;
        }
        entry.payment_requests_this_minute += 1;
        entry.payment_count_this_session += 1;
    }

    /// Drop counters for `browser_id`. Fired from C++ via
    /// `POST /wallet/session/close` when a tab closes. Mirrors C++'s
    /// `SessionManager::clearSession` exactly.
    pub fn clear_session_for_browser(&self, browser_id: i32) {
        let mut guard = self
            .session_counters
            .write()
            .expect("session_counters lock poisoned");
        guard.remove(&browser_id);
    }

    /// Count of distinct browser sessions currently tracked. For dev sanity.
    pub fn session_counter_browser_count(&self) -> usize {
        self.session_counters
            .read()
            .expect("session_counters lock poisoned")
            .len()
    }

    /// Internal helper: get-or-create a SessionCounters entry for write,
    /// applying the C++ reset-on-domain-change semantics. Caller holds the
    /// write lock.
    fn get_or_create_for_write<'a>(
        guard: &'a mut HashMap<i32, SessionCounters>,
        browser_id: i32,
        domain: &str,
        now: i64,
    ) -> &'a mut SessionCounters {
        let entry = guard.entry(browser_id).or_insert_with(|| SessionCounters {
            domain: domain.to_string(),
            minute_window_start: now,
            ..Default::default()
        });
        if entry.domain != domain {
            // Domain navigated — reset spent + rate per C++'s getSession L17-21.
            // C++ does NOT reset payment_count_this_session on domain change;
            // matching that here. The asymmetry is a pre-existing C++ behavior
            // worth a follow-up but not a 2.6-E regression target.
            entry.domain = domain.to_string();
            entry.spent_cents = 0;
            entry.payment_requests_this_minute = 0;
            entry.minute_window_start = now;
        }
        entry
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
            cents: 0,
            browser_id: 0,
        }
    }

    #[test]
    fn new_service_has_no_pending_approvals() {
        let svc = PermissionService::new();
        assert_eq!(svc.pending_approval_count(), 0);
    }

    #[test]
    fn insert_and_consume_single_use() {
        let svc = PermissionService::new();
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
        let svc = PermissionService::new();
        svc.insert_pending_approval(sample_approval("abc", 100));
        let consumed = svc.consume_pending_approval("abc", 999); // now > expires_at
        assert!(consumed.is_none());
        // Entry still removed even though expired — prevents replay.
        assert_eq!(svc.pending_approval_count(), 0);
    }

    #[test]
    fn purge_expired_drops_only_expired_entries() {
        let svc = PermissionService::new();
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

        let svc = PermissionService::new();
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
        let svc = PermissionService::new();
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
        let svc = PermissionService::new();
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
        let svc = PermissionService::new();
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
        let svc = PermissionService::new();
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
        let svc = PermissionService::new();
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

    // ---------- Phase 2.6-C.4 follow-up: session opt-in caches ----------

    #[test]
    fn identity_key_session_approve_then_check() {
        let svc = PermissionService::new();
        assert!(!svc.is_identity_key_session_approved("example.com"));
        svc.approve_identity_key_session("example.com");
        assert!(svc.is_identity_key_session_approved("example.com"));
        // Other domains stay unaffected.
        assert!(!svc.is_identity_key_session_approved("other.com"));
    }

    #[test]
    fn identity_key_session_approve_is_idempotent() {
        let svc = PermissionService::new();
        svc.approve_identity_key_session("example.com");
        svc.approve_identity_key_session("example.com");
        svc.approve_identity_key_session("example.com");
        assert!(svc.is_identity_key_session_approved("example.com"));
    }

    #[test]
    fn key_linkage_session_approve_then_check() {
        let svc = PermissionService::new();
        assert!(!svc.is_key_linkage_session_approved("example.com"));
        svc.approve_key_linkage_session("example.com");
        assert!(svc.is_key_linkage_session_approved("example.com"));
        assert!(!svc.is_key_linkage_session_approved("other.com"));
    }

    #[test]
    fn identity_key_and_key_linkage_session_caches_are_independent() {
        let svc = PermissionService::new();
        svc.approve_identity_key_session("example.com");
        assert!(svc.is_identity_key_session_approved("example.com"));
        assert!(!svc.is_key_linkage_session_approved("example.com"));

        svc.approve_key_linkage_session("other.com");
        assert!(svc.is_key_linkage_session_approved("other.com"));
        assert!(!svc.is_identity_key_session_approved("other.com"));
    }

    #[test]
    fn revoke_session_approvals_clears_both_caches() {
        let svc = PermissionService::new();
        svc.approve_identity_key_session("example.com");
        svc.approve_key_linkage_session("example.com");
        assert!(svc.is_identity_key_session_approved("example.com"));
        assert!(svc.is_key_linkage_session_approved("example.com"));

        svc.revoke_session_approvals_for_domain("example.com");
        assert!(!svc.is_identity_key_session_approved("example.com"));
        assert!(!svc.is_key_linkage_session_approved("example.com"));
    }

    #[test]
    fn revoke_session_approvals_is_idempotent_when_domain_absent() {
        let svc = PermissionService::new();
        // No prior approvals — revoke should be a no-op, not a panic.
        svc.revoke_session_approvals_for_domain("never-approved.com");
        assert!(!svc.is_identity_key_session_approved("never-approved.com"));
        assert!(!svc.is_key_linkage_session_approved("never-approved.com"));
    }

    // ---------- Phase 2.6-E: SessionCounters ----------

    const T0: i64 = 1_700_000_000;

    #[test]
    fn snapshot_with_no_session_returns_zeroed_with_domain() {
        let svc = PermissionService::new();
        let c = svc.get_session_counters_snapshot(42, "example.com", T0);
        assert_eq!(c.domain, "example.com");
        assert_eq!(c.spent_cents, 0);
        assert_eq!(c.payment_count_this_session, 0);
        assert_eq!(c.payment_requests_this_minute, 0);
        // Read does NOT create an entry.
        assert_eq!(svc.session_counter_browser_count(), 0);
    }

    #[test]
    fn record_spending_creates_then_accumulates() {
        let svc = PermissionService::new();
        svc.record_spending(42, "example.com", 50, T0);
        let c = svc.get_session_counters_snapshot(42, "example.com", T0);
        assert_eq!(c.spent_cents, 50);
        assert_eq!(svc.session_counter_browser_count(), 1);

        svc.record_spending(42, "example.com", 25, T0 + 1);
        let c = svc.get_session_counters_snapshot(42, "example.com", T0 + 1);
        assert_eq!(c.spent_cents, 75);
    }

    #[test]
    fn record_spending_with_different_domain_resets_spent_and_rate() {
        let svc = PermissionService::new();
        svc.record_spending(42, "example.com", 100, T0);
        svc.increment_payment_rate_counter(42, "example.com", T0);
        // Switch domain — spent + rate reset, payment_count carries over
        // (matches C++ behavior).
        svc.record_spending(42, "other.com", 30, T0 + 5);
        let c = svc.get_session_counters_snapshot(42, "other.com", T0 + 5);
        assert_eq!(c.spent_cents, 30);
        assert_eq!(c.payment_requests_this_minute, 0);
        assert_eq!(c.payment_count_this_session, 1);
        // Original domain now reads as zeroed since the entry was reassigned.
        let c2 = svc.get_session_counters_snapshot(42, "example.com", T0 + 5);
        assert_eq!(c2.spent_cents, 0);
    }

    #[test]
    fn increment_rate_counter_within_window_accumulates() {
        let svc = PermissionService::new();
        svc.increment_payment_rate_counter(42, "example.com", T0);
        svc.increment_payment_rate_counter(42, "example.com", T0 + 5);
        svc.increment_payment_rate_counter(42, "example.com", T0 + 10);
        let c = svc.get_session_counters_snapshot(42, "example.com", T0 + 15);
        assert_eq!(c.payment_requests_this_minute, 3);
        assert_eq!(c.payment_count_this_session, 3);
    }

    #[test]
    fn increment_rate_counter_after_window_expiry_resets_minute_only() {
        let svc = PermissionService::new();
        svc.increment_payment_rate_counter(42, "example.com", T0);
        svc.increment_payment_rate_counter(42, "example.com", T0 + 30);
        // 60s elapsed since window started at T0 — next increment opens a new
        // window with this call counted as 1 in it.
        svc.increment_payment_rate_counter(42, "example.com", T0 + 65);
        let c = svc.get_session_counters_snapshot(42, "example.com", T0 + 66);
        // Window resets to 1 (this call only), but total session count keeps
        // climbing across windows.
        assert_eq!(c.payment_requests_this_minute, 1);
        assert_eq!(c.payment_count_this_session, 3);
    }

    #[test]
    fn snapshot_applies_window_expiry_on_read() {
        let svc = PermissionService::new();
        svc.increment_payment_rate_counter(42, "example.com", T0);
        svc.increment_payment_rate_counter(42, "example.com", T0 + 30);
        // Read at T0+65 — window has expired, snapshot should report 0 even
        // though no new increment has fired the in-place reset yet.
        let c = svc.get_session_counters_snapshot(42, "example.com", T0 + 65);
        assert_eq!(c.payment_requests_this_minute, 0);
        // Total session count unaffected by window expiry.
        assert_eq!(c.payment_count_this_session, 2);
    }

    #[test]
    fn snapshot_with_mismatched_domain_returns_zeroed() {
        let svc = PermissionService::new();
        svc.record_spending(42, "example.com", 100, T0);
        let c = svc.get_session_counters_snapshot(42, "other.com", T0 + 1);
        assert_eq!(c.spent_cents, 0);
        assert_eq!(c.payment_count_this_session, 0);
        assert_eq!(c.payment_requests_this_minute, 0);
    }

    #[test]
    fn clear_session_for_browser_drops_entry() {
        let svc = PermissionService::new();
        svc.record_spending(42, "example.com", 100, T0);
        svc.record_spending(43, "other.com", 50, T0);
        assert_eq!(svc.session_counter_browser_count(), 2);
        svc.clear_session_for_browser(42);
        assert_eq!(svc.session_counter_browser_count(), 1);
        let c = svc.get_session_counters_snapshot(42, "example.com", T0 + 1);
        assert_eq!(c.spent_cents, 0);
        // Other browser unaffected.
        let c2 = svc.get_session_counters_snapshot(43, "other.com", T0 + 1);
        assert_eq!(c2.spent_cents, 50);
    }

    #[test]
    fn clear_session_for_unknown_browser_is_noop() {
        let svc = PermissionService::new();
        svc.clear_session_for_browser(999);
        assert_eq!(svc.session_counter_browser_count(), 0);
    }

    #[test]
    fn per_browser_isolation() {
        let svc = PermissionService::new();
        svc.record_spending(1, "example.com", 100, T0);
        svc.record_spending(2, "example.com", 50, T0);
        let c1 = svc.get_session_counters_snapshot(1, "example.com", T0);
        let c2 = svc.get_session_counters_snapshot(2, "example.com", T0);
        assert_eq!(c1.spent_cents, 100);
        assert_eq!(c2.spent_cents, 50);
    }
}
