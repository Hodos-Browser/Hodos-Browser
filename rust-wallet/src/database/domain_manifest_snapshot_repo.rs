//! `domain_manifest_snapshots` — what a site asked for, as the user approved it.
//!
//! beta.3 Phase 0.8. Migration **V24** (`migrations.rs :: migrate_v23_to_v24`),
//! owner-approved 2026-08-22.
//!
//! ⛔ **Three rules, all load-bearing** — see
//! `development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/PHASE_CONTRACT.md` §6a:
//!
//! 1. **Informational only, never a decision input.** Authoritative permission
//!    state is `domain_permissions` plus the V18 child tables. Nothing in this
//!    table is ever read by `hodos_permission_engine::decide` — a
//!    `PermissionContext` has no field it could occupy, and it must stay that
//!    way (`R-SNAPSHOT`, `P0.8-A11`). This table exists to answer *"what did
//!    this site ask for when I approved it?"* and to back a future
//!    user-initiated "apply the site's recommended settings" button (beta.4).
//!
//! 2. 🚨 **As approved, not live.** `approved_at` non-NULL marks the row the
//!    user actually saw. A row with `approved_at IS NULL` is a later fetch
//!    parked for comparison, never something to restore from. Without this
//!    split, a site could publish modest recommendations, get approved, publish
//!    aggressive ones, and have a "restore recommended" click months later
//!    silently adopt numbers that were never on screen. A manifest that has
//!    *changed* is a re-consent event, not a silent update.
//!
//! 3. **Schema change ⇒ owner approval.** Granted for exactly this shape
//!    (CLAUDE.md invariant #2). Widening it needs a fresh yes.
//!
//! Child table joined by FK + `ON DELETE CASCADE` off `domain_permissions(id)`,
//! mirroring the `cert_field_permissions` pattern CLAUDE.md names as the reuse
//! anchor — so revoking a site disposes of its snapshot with it.

use rusqlite::{params, Connection, Result};

/// One stored manifest snapshot.
#[derive(Debug, Clone)]
pub struct DomainManifestSnapshot {
    pub id: i64,
    pub domain_permission_id: i64,
    /// Raw bytes exactly as the site served them (≤ 64 KB by the fetch cap).
    pub manifest_json: String,
    /// Which of the two locations served it.
    pub source_url: String,
    pub fetched_at: i64,
    /// `Some` = this is the snapshot the user approved. `None` = a later fetch
    /// parked for comparison.
    pub approved_at: Option<i64>,
}

pub struct DomainManifestSnapshotRepository<'a> {
    conn: &'a Connection,
}

impl<'a> DomainManifestSnapshotRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Record the manifest the user just approved for this domain.
    ///
    /// Replaces any previous approved snapshot for the same
    /// `domain_permission_id` — a re-approval is a fresh consent, and keeping
    /// the superseded one would make "which numbers did they agree to?"
    /// ambiguous. Non-approved (comparison) rows are left alone.
    ///
    /// Returns the new row id.
    pub fn record_approved(
        &self,
        domain_permission_id: i64,
        manifest_json: &str,
        source_url: &str,
        fetched_at: i64,
        approved_at: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "DELETE FROM domain_manifest_snapshots
             WHERE domain_permission_id = ?1 AND approved_at IS NOT NULL",
            params![domain_permission_id],
        )?;
        self.conn.execute(
            "INSERT INTO domain_manifest_snapshots
                (domain_permission_id, manifest_json, source_url, fetched_at, approved_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![domain_permission_id, manifest_json, source_url, fetched_at, approved_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// The snapshot the user approved for this domain, if any.
    pub fn get_approved(&self, domain_permission_id: i64) -> Result<Option<DomainManifestSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, domain_permission_id, manifest_json, source_url, fetched_at, approved_at
             FROM domain_manifest_snapshots
             WHERE domain_permission_id = ?1 AND approved_at IS NOT NULL
             ORDER BY approved_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![domain_permission_id], |row| {
            Ok(DomainManifestSnapshot {
                id: row.get(0)?,
                domain_permission_id: row.get(1)?,
                manifest_json: row.get(2)?,
                source_url: row.get(3)?,
                fetched_at: row.get(4)?,
                approved_at: row.get(5)?,
            })
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DomainPermissionRepository;

    /// Build an in-memory DB carrying just enough schema for the FK to bite.
    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE domain_permissions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                domain TEXT NOT NULL,
                trust_level TEXT NOT NULL DEFAULT 'unknown',
                per_tx_limit_cents INTEGER NOT NULL DEFAULT 100,
                per_session_limit_cents INTEGER NOT NULL DEFAULT 1000,
                rate_limit_per_min INTEGER NOT NULL DEFAULT 30,
                max_tx_per_session INTEGER NOT NULL DEFAULT 100,
                identity_key_disclosure_allowed INTEGER NOT NULL DEFAULT 1,
                bundled_scope_grant INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(user_id, domain)
             );
             CREATE TABLE settings (id INTEGER PRIMARY KEY);
             INSERT INTO settings (id) VALUES (1);",
        )
        .unwrap();
        crate::database::migrations::migrate_v23_to_v24(&conn).unwrap();
        conn
    }

    fn insert_domain(conn: &Connection, domain: &str) -> i64 {
        conn.execute(
            "INSERT INTO domain_permissions (user_id, domain, created_at, updated_at)
             VALUES (1, ?1, 0, 0)",
            params![domain],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn migration_is_idempotent() {
        let conn = setup();
        // Re-running must be a no-op, not an error.
        crate::database::migrations::migrate_v23_to_v24(&conn).unwrap();
        crate::database::migrations::migrate_v23_to_v24(&conn).unwrap();
    }

    #[test]
    fn record_and_read_back_the_approved_snapshot() {
        let conn = setup();
        let dpid = insert_domain(&conn, "bitgenius.net");
        let repo = DomainManifestSnapshotRepository::new(&conn);

        repo.record_approved(dpid, r#"{"metanet":{}}"#, "https://bitgenius.net/manifest.json", 111, 222)
            .unwrap();

        let got = repo.get_approved(dpid).unwrap().unwrap();
        assert_eq!(got.manifest_json, r#"{"metanet":{}}"#);
        assert_eq!(got.source_url, "https://bitgenius.net/manifest.json");
        assert_eq!(got.fetched_at, 111);
        assert_eq!(got.approved_at, Some(222));
    }

    #[test]
    fn no_snapshot_for_a_domain_that_never_served_one() {
        let conn = setup();
        let dpid = insert_domain(&conn, "plain.example");
        let repo = DomainManifestSnapshotRepository::new(&conn);
        assert!(repo.get_approved(dpid).unwrap().is_none());
    }

    /// Re-approving replaces rather than accumulating — otherwise "which
    /// numbers did they agree to?" has two answers.
    #[test]
    fn re_approval_replaces_the_previous_approved_snapshot() {
        let conn = setup();
        let dpid = insert_domain(&conn, "bitgenius.net");
        let repo = DomainManifestSnapshotRepository::new(&conn);

        repo.record_approved(dpid, r#"{"v":1}"#, "https://x/manifest.json", 1, 2).unwrap();
        repo.record_approved(dpid, r#"{"v":2}"#, "https://x/manifest.json", 3, 4).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM domain_manifest_snapshots WHERE domain_permission_id = ?1",
                params![dpid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "a re-approval supersedes, it does not accumulate");
        assert_eq!(repo.get_approved(dpid).unwrap().unwrap().manifest_json, r#"{"v":2}"#);
    }

    /// Snapshots are per-domain. A snapshot for one site must never surface
    /// under another (`Permission data MUST be isolated per originator`,
    /// BRC-116 §9.2).
    #[test]
    fn snapshots_are_isolated_per_domain() {
        let conn = setup();
        let a = insert_domain(&conn, "a.example");
        let b = insert_domain(&conn, "b.example");
        let repo = DomainManifestSnapshotRepository::new(&conn);
        repo.record_approved(a, r#"{"who":"a"}"#, "https://a.example/manifest.json", 1, 2).unwrap();
        assert!(repo.get_approved(b).unwrap().is_none());
        assert_eq!(repo.get_approved(a).unwrap().unwrap().manifest_json, r#"{"who":"a"}"#);
    }

    /// Revoking a site disposes of its snapshot — no orphaned record of a site
    /// the user removed. This is the whole reason it is a CASCADE child table
    /// rather than a column.
    #[test]
    fn deleting_the_domain_cascades_the_snapshot_away() {
        let conn = setup();
        let dpid = insert_domain(&conn, "bitgenius.net");
        let repo = DomainManifestSnapshotRepository::new(&conn);
        repo.record_approved(dpid, r#"{"v":1}"#, "https://x/manifest.json", 1, 2).unwrap();

        conn.execute("DELETE FROM domain_permissions WHERE id = ?1", params![dpid]).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM domain_manifest_snapshots", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "ON DELETE CASCADE did not fire — is PRAGMA foreign_keys on?");
    }

    /// The V24 settings column ships OFF: behaviour (b) — the user's own
    /// defaults — is the default state.
    #[test]
    fn prefill_toggle_defaults_to_off() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE settings (id INTEGER PRIMARY KEY);").unwrap();
        conn.execute("INSERT INTO settings (id) VALUES (1)", []).unwrap();
        crate::database::migrations::migrate_v23_to_v24(&conn).unwrap();
        let v: i64 = conn
            .query_row("SELECT default_prefill_from_manifest FROM settings LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 0, "the site-values pre-fill must be opt-in, never the default");
    }

    // Keeps the unused-import warning away when only some tests compile.
    #[allow(dead_code)]
    fn _uses_domain_permission_repo(conn: &Connection) -> DomainPermissionRepository<'_> {
        DomainPermissionRepository::new(conn)
    }
}
