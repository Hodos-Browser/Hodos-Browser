//! Permission audit log repository — Phase 2.6-A.5.
//!
//! Records every permission engine decision for long-lived audit/forensic
//! purposes. Schema created by V20 migration; 90-day retention enforced by
//! a background purge task added in a later sub-phase.
//!
//! Per OQ2, the request body is stored as a sha256 hex hash (`VARCHAR(64)`),
//! NOT the raw body — captures call identity for forensic provenance without
//! storing raw payload bytes.
//!
//! See: `development-docs/Sigma-BRC121-Sprint/phase-2.6-engine-to-rust/PHASE_2_6_ENGINE_TO_RUST.md`
//! §11 (OQ1 + OQ2 RESOLVED).

use rusqlite::{params, Connection, OptionalExtension, Result};

/// One row of the `permission_audit_log` table.
#[derive(Debug, Clone)]
pub struct PermissionAuditEntry {
    pub id: Option<i64>,
    /// 128-bit hex nonce. `None` for Silent decisions (no approval needed) and
    /// for Deny decisions (no approval flow). `Some(...)` for Prompt decisions
    /// that produced an approval token.
    pub approval_id: Option<String>,
    pub domain: String,
    pub endpoint: String,
    /// CallKind as a string (e.g. "Payment", "IdentityKeyReveal"). Matches
    /// `hodos_permission_engine::CallKind` serde-PascalCase output.
    pub call_kind: String,
    /// Engine reason as a string (e.g. "per_tx_limit", "silent_within_caps").
    /// Matches `hodos_permission_engine::EngineReason` serde-snake_case output.
    pub engine_reason: String,
    /// Decision kind: "silent" | "prompt" | "deny".
    pub decision: String,
    /// User's response: "approve" | "deny" | None (still pending or never resolved).
    pub user_decision: Option<String>,
    /// sha256 hex of the request body (64 chars). Privacy-safe.
    pub body_hash: String,
    /// Unix timestamp seconds when the engine made the decision.
    pub created_at: i64,
    /// Unix timestamp seconds when the user resolved the modal. `None` for
    /// Silent/Deny decisions and pending prompts.
    pub resolved_at: Option<i64>,
    /// How the prompt was resolved: "modal_approve" | "modal_deny" | "timeout" | None.
    pub resolved_via: Option<String>,
}

pub struct PermissionAuditRepository<'a> {
    conn: &'a Connection,
}

impl<'a> PermissionAuditRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Insert a new audit entry. Returns the rowid.
    pub fn insert(&self, entry: &PermissionAuditEntry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO permission_audit_log (
                approval_id, domain, endpoint, call_kind, engine_reason,
                decision, user_decision, body_hash, created_at, resolved_at, resolved_via
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                entry.approval_id,
                entry.domain,
                entry.endpoint,
                entry.call_kind,
                entry.engine_reason,
                entry.decision,
                entry.user_decision,
                entry.body_hash,
                entry.created_at,
                entry.resolved_at,
                entry.resolved_via,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Mark a pending approval as resolved (modal_approve / modal_deny / timeout).
    pub fn mark_resolved(
        &self,
        approval_id: &str,
        user_decision: &str,
        resolved_via: &str,
        resolved_at: i64,
    ) -> Result<usize> {
        self.conn.execute(
            "UPDATE permission_audit_log
             SET user_decision = ?1, resolved_via = ?2, resolved_at = ?3
             WHERE approval_id = ?4 AND resolved_at IS NULL",
            params![user_decision, resolved_via, resolved_at, approval_id],
        )
    }

    /// Fetch a single entry by approval_id (for the X-User-Approved re-issue path).
    pub fn get_by_approval_id(&self, approval_id: &str) -> Result<Option<PermissionAuditEntry>> {
        self.conn
            .query_row(
                "SELECT id, approval_id, domain, endpoint, call_kind, engine_reason,
                        decision, user_decision, body_hash, created_at, resolved_at, resolved_via
                 FROM permission_audit_log WHERE approval_id = ?1 LIMIT 1",
                params![approval_id],
                |row| {
                    Ok(PermissionAuditEntry {
                        id: row.get(0)?,
                        approval_id: row.get(1)?,
                        domain: row.get(2)?,
                        endpoint: row.get(3)?,
                        call_kind: row.get(4)?,
                        engine_reason: row.get(5)?,
                        decision: row.get(6)?,
                        user_decision: row.get(7)?,
                        body_hash: row.get(8)?,
                        created_at: row.get(9)?,
                        resolved_at: row.get(10)?,
                        resolved_via: row.get(11)?,
                    })
                },
            )
            .optional()
    }

    /// Background purge: delete entries older than `cutoff_secs` Unix timestamp.
    /// Per OQ1, cutoff is 90 days ago (computed by caller).
    pub fn purge_older_than(&self, cutoff_secs: i64) -> Result<usize> {
        self.conn.execute(
            "DELETE FROM permission_audit_log WHERE created_at < ?1",
            params![cutoff_secs],
        )
    }

    /// Count entries for a domain since a given timestamp (for analytics).
    pub fn count_recent(&self, domain: &str, since: i64) -> Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM permission_audit_log WHERE domain = ?1 AND created_at >= ?2",
            params![domain, since],
            |row| row.get(0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::migrations;

    fn fresh_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // V20 migration needs the schema_version table to exist; we apply
        // V19→V20 directly since we don't care about prior migrations for the
        // audit table test.
        conn.execute(
            "CREATE TABLE schema_version (version INTEGER NOT NULL)",
            [],
        )
        .unwrap();
        migrations::migrate_v19_to_v20(&conn).unwrap();
        conn
    }

    fn sample_entry() -> PermissionAuditEntry {
        PermissionAuditEntry {
            id: None,
            approval_id: Some("abc123".to_string()),
            domain: "example.com".to_string(),
            endpoint: "/createAction".to_string(),
            call_kind: "Payment".to_string(),
            engine_reason: "per_tx_limit".to_string(),
            decision: "prompt".to_string(),
            user_decision: None,
            body_hash: "0".repeat(64),
            created_at: 1_700_000_000,
            resolved_at: None,
            resolved_via: None,
        }
    }

    #[test]
    fn insert_and_lookup_by_approval_id() {
        let conn = fresh_db();
        let repo = PermissionAuditRepository::new(&conn);
        let entry = sample_entry();
        let rowid = repo.insert(&entry).unwrap();
        assert!(rowid > 0);

        let fetched = repo.get_by_approval_id("abc123").unwrap().unwrap();
        assert_eq!(fetched.domain, "example.com");
        assert_eq!(fetched.call_kind, "Payment");
        assert_eq!(fetched.engine_reason, "per_tx_limit");
        assert!(fetched.resolved_at.is_none());
    }

    #[test]
    fn mark_resolved_updates_fields() {
        let conn = fresh_db();
        let repo = PermissionAuditRepository::new(&conn);
        repo.insert(&sample_entry()).unwrap();

        let updated = repo
            .mark_resolved("abc123", "approve", "modal_approve", 1_700_000_300)
            .unwrap();
        assert_eq!(updated, 1);

        let fetched = repo.get_by_approval_id("abc123").unwrap().unwrap();
        assert_eq!(fetched.user_decision.as_deref(), Some("approve"));
        assert_eq!(fetched.resolved_via.as_deref(), Some("modal_approve"));
        assert_eq!(fetched.resolved_at, Some(1_700_000_300));
    }

    #[test]
    fn mark_resolved_is_idempotent_only_for_pending() {
        let conn = fresh_db();
        let repo = PermissionAuditRepository::new(&conn);
        repo.insert(&sample_entry()).unwrap();
        repo.mark_resolved("abc123", "approve", "modal_approve", 1).unwrap();

        // Second call must NOT overwrite — guarded by "resolved_at IS NULL".
        let updated = repo.mark_resolved("abc123", "deny", "modal_deny", 9999).unwrap();
        assert_eq!(updated, 0, "should not overwrite already-resolved entry");

        let fetched = repo.get_by_approval_id("abc123").unwrap().unwrap();
        assert_eq!(fetched.user_decision.as_deref(), Some("approve"));
    }

    #[test]
    fn purge_older_than_cutoff() {
        let conn = fresh_db();
        let repo = PermissionAuditRepository::new(&conn);
        let mut old = sample_entry();
        old.approval_id = Some("old1".to_string());
        old.created_at = 100;
        repo.insert(&old).unwrap();

        let mut new = sample_entry();
        new.approval_id = Some("new1".to_string());
        new.created_at = 2_000_000_000;
        repo.insert(&new).unwrap();

        let deleted = repo.purge_older_than(1_000).unwrap();
        assert_eq!(deleted, 1);

        assert!(repo.get_by_approval_id("old1").unwrap().is_none());
        assert!(repo.get_by_approval_id("new1").unwrap().is_some());
    }

    #[test]
    fn count_recent_filters_by_domain_and_time() {
        let conn = fresh_db();
        let repo = PermissionAuditRepository::new(&conn);
        for i in 0..3 {
            let mut e = sample_entry();
            e.approval_id = Some(format!("e{}", i));
            e.created_at = 1_000 + i;
            repo.insert(&e).unwrap();
        }
        let mut other = sample_entry();
        other.approval_id = Some("other".to_string());
        other.domain = "other.com".to_string();
        other.created_at = 9_999;
        repo.insert(&other).unwrap();

        let count = repo.count_recent("example.com", 1_001).unwrap();
        assert_eq!(count, 2); // entries at 1001 and 1002
    }
}
