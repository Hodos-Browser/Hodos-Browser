//! Engine shadow log repository — Phase 2.6-A.5.
//!
//! Records every C++ vs Rust engine decision comparison during the Phase 2.6
//! migration window. Used to verify the Rust port produces identical decisions
//! to the C++ engine BEFORE each `engine_rust_*` flag flips from OFF to ON.
//!
//! This is **scaffolding**, not a long-lived audit surface. The table is created
//! by V20 (this sub-phase) and dropped by V21 (sub-phase 2.6-H cleanup),
//! together with the C++ engine deletion. After 2.6 closes, this repo will
//! also be deleted.
//!
//! Schema columns:
//!   - `call_kind_class`: one of 'privacy_perimeter', 'scoped_grant', 'payment',
//!     'cert_disclosure', 'domain_trust' — matches the 5 feature flag classes.
//!   - `cpp_decision` / `rust_decision`: 'silent' | 'prompt' | 'deny'.
//!   - `cpp_reason` / `rust_reason`: free-form (C++) and serde-snake_case (Rust).
//!   - `agreement`: 1 if decisions match, 0 otherwise.
//!   - `context_hash`: sha256 of the serialized PermissionContext (for dedup
//!     analytics; not enforced unique).
//!
//! See: `development-docs/Sigma-BRC121-Sprint/phase-2.6-engine-to-rust/PHASE_2_6_ENGINE_TO_RUST.md`
//! §LD5 (shadow mode design).

use rusqlite::{params, Connection, Result};

/// One row of the `engine_shadow_log` table.
#[derive(Debug, Clone)]
pub struct EngineShadowEntry {
    pub id: Option<i64>,
    pub call_kind_class: String,
    pub endpoint: String,
    pub domain: String,
    pub cpp_decision: String,
    pub rust_decision: String,
    pub cpp_reason: Option<String>,
    pub rust_reason: Option<String>,
    /// 1 = decisions agree, 0 = disagree.
    pub agreement: i64,
    /// sha256 hex of the serialized PermissionContext that produced both decisions.
    pub context_hash: String,
    pub observed_at: i64,
}

pub struct EngineShadowRepository<'a> {
    conn: &'a Connection,
}

impl<'a> EngineShadowRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Insert a new shadow comparison. Returns the rowid.
    pub fn insert(&self, entry: &EngineShadowEntry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO engine_shadow_log (
                call_kind_class, endpoint, domain, cpp_decision, rust_decision,
                cpp_reason, rust_reason, agreement, context_hash, observed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.call_kind_class,
                entry.endpoint,
                entry.domain,
                entry.cpp_decision,
                entry.rust_decision,
                entry.cpp_reason,
                entry.rust_reason,
                entry.agreement,
                entry.context_hash,
                entry.observed_at,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Most recent disagreements (for CLI inspection during migration).
    /// Used to drive flag-flip decisions: if `query_disagreements(N)` returns
    /// empty for the relevant `call_kind_class` over a soak period, the flag
    /// is ready to flip.
    pub fn query_disagreements(&self, limit: i64) -> Result<Vec<EngineShadowEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, call_kind_class, endpoint, domain, cpp_decision, rust_decision,
                    cpp_reason, rust_reason, agreement, context_hash, observed_at
             FROM engine_shadow_log WHERE agreement = 0
             ORDER BY observed_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(EngineShadowEntry {
                id: row.get(0)?,
                call_kind_class: row.get(1)?,
                endpoint: row.get(2)?,
                domain: row.get(3)?,
                cpp_decision: row.get(4)?,
                rust_decision: row.get(5)?,
                cpp_reason: row.get(6)?,
                rust_reason: row.get(7)?,
                agreement: row.get(8)?,
                context_hash: row.get(9)?,
                observed_at: row.get(10)?,
            })
        })?;
        rows.collect()
    }

    /// Agreement stats by CallKind class (for the flag-flip readiness check).
    /// Returns (call_kind_class, total_count, agreement_count) per class.
    pub fn agreement_stats_by_class(&self) -> Result<Vec<(String, i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT call_kind_class,
                    COUNT(*),
                    SUM(CASE WHEN agreement = 1 THEN 1 ELSE 0 END)
             FROM engine_shadow_log
             GROUP BY call_kind_class
             ORDER BY call_kind_class",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get(1)?, row.get(2)?))
        })?;
        rows.collect()
    }

    /// Total row count — used by background purge / dev sanity checks.
    pub fn count(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM engine_shadow_log", [], |row| {
                row.get(0)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::migrations;

    fn fresh_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE schema_version (version INTEGER NOT NULL)",
            [],
        )
        .unwrap();
        migrations::migrate_v19_to_v20(&conn).unwrap();
        conn
    }

    fn sample(class: &str, agreement: i64, observed_at: i64) -> EngineShadowEntry {
        EngineShadowEntry {
            id: None,
            call_kind_class: class.to_string(),
            endpoint: "/createAction".to_string(),
            domain: "example.com".to_string(),
            cpp_decision: "silent".to_string(),
            rust_decision: if agreement == 1 { "silent" } else { "prompt" }.to_string(),
            cpp_reason: Some("within caps".to_string()),
            rust_reason: Some("silent_within_caps".to_string()),
            agreement,
            context_hash: "0".repeat(64),
            observed_at,
        }
    }

    #[test]
    fn insert_returns_rowid() {
        let conn = fresh_db();
        let repo = EngineShadowRepository::new(&conn);
        let id = repo.insert(&sample("payment", 1, 1_700_000_000)).unwrap();
        assert!(id > 0);
        assert_eq!(repo.count().unwrap(), 1);
    }

    #[test]
    fn query_disagreements_filters_and_orders_desc() {
        let conn = fresh_db();
        let repo = EngineShadowRepository::new(&conn);

        repo.insert(&sample("payment", 1, 100)).unwrap();
        repo.insert(&sample("payment", 0, 200)).unwrap();
        repo.insert(&sample("payment", 0, 300)).unwrap();
        repo.insert(&sample("payment", 1, 400)).unwrap();

        let disagreements = repo.query_disagreements(10).unwrap();
        assert_eq!(disagreements.len(), 2);
        // Ordered DESC by observed_at — newest disagreement first.
        assert_eq!(disagreements[0].observed_at, 300);
        assert_eq!(disagreements[1].observed_at, 200);
    }

    #[test]
    fn agreement_stats_by_class_groups_correctly() {
        let conn = fresh_db();
        let repo = EngineShadowRepository::new(&conn);

        repo.insert(&sample("payment", 1, 1)).unwrap();
        repo.insert(&sample("payment", 1, 2)).unwrap();
        repo.insert(&sample("payment", 0, 3)).unwrap();
        repo.insert(&sample("privacy_perimeter", 1, 4)).unwrap();
        repo.insert(&sample("privacy_perimeter", 1, 5)).unwrap();

        let stats = repo.agreement_stats_by_class().unwrap();
        // ORDER BY name — alphabetic.
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].0, "payment");
        assert_eq!(stats[0].1, 3); // total
        assert_eq!(stats[0].2, 2); // agreements
        assert_eq!(stats[1].0, "privacy_perimeter");
        assert_eq!(stats[1].1, 2);
        assert_eq!(stats[1].2, 2); // 100% agreement
    }
}
