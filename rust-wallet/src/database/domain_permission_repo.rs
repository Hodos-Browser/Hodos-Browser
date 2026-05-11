//! Domain permission repository for database operations
//!
//! Handles CRUD operations for domain permissions and certificate field permissions.
//! Phase 2.1 of UX improvements — replaces JSON-file domain whitelist with
//! granular per-domain trust levels, spending limits, and cert field tracking.

use rusqlite::{Connection, Result, params};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use super::models::{
    DomainPermission,
    DomainProtocolPermission,
    DomainBasketPermission,
    DomainCounterpartyPermission,
};

pub struct DomainPermissionRepository<'a> {
    conn: &'a Connection,
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

impl<'a> DomainPermissionRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        DomainPermissionRepository { conn }
    }

    // ========================================================================
    // Domain permissions
    // ========================================================================

    /// Get permission record for a specific domain and user
    pub fn get_by_domain(&self, user_id: i64, domain: &str) -> Result<Option<DomainPermission>> {
        match self.conn.query_row(
            "SELECT id, user_id, domain, trust_level, per_tx_limit_cents, per_session_limit_cents,
                    rate_limit_per_min, max_tx_per_session, identity_key_disclosure_allowed,
                    created_at, updated_at
             FROM domain_permissions WHERE user_id = ?1 AND domain = ?2",
            params![user_id, domain],
            |row| Ok(DomainPermission {
                id: Some(row.get(0)?),
                user_id: row.get(1)?,
                domain: row.get(2)?,
                trust_level: row.get(3)?,
                per_tx_limit_cents: row.get(4)?,
                per_session_limit_cents: row.get(5)?,
                rate_limit_per_min: row.get(6)?,
                max_tx_per_session: row.get(7)?,
                identity_key_disclosure_allowed: row.get::<_, i64>(8)? != 0,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            }),
        ) {
            Ok(perm) => Ok(Some(perm)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Insert or update a domain permission record.
    /// Returns the row ID.
    pub fn upsert(&self, perm: &DomainPermission) -> Result<i64> {
        let now = unix_now();

        // Try to find existing
        let existing_id: Option<i64> = match self.conn.query_row(
            "SELECT id FROM domain_permissions WHERE user_id = ?1 AND domain = ?2",
            params![perm.user_id, perm.domain],
            |row| row.get(0),
        ) {
            Ok(id) => Some(id),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };

        if let Some(id) = existing_id {
            self.conn.execute(
                "UPDATE domain_permissions SET
                    trust_level = ?1,
                    per_tx_limit_cents = ?2,
                    per_session_limit_cents = ?3,
                    rate_limit_per_min = ?4,
                    max_tx_per_session = ?5,
                    identity_key_disclosure_allowed = ?6,
                    updated_at = ?7
                 WHERE id = ?8",
                params![
                    perm.trust_level,
                    perm.per_tx_limit_cents,
                    perm.per_session_limit_cents,
                    perm.rate_limit_per_min,
                    perm.max_tx_per_session,
                    if perm.identity_key_disclosure_allowed { 1_i64 } else { 0_i64 },
                    now,
                    id,
                ],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO domain_permissions
                 (user_id, domain, trust_level, per_tx_limit_cents, per_session_limit_cents,
                  rate_limit_per_min, max_tx_per_session, identity_key_disclosure_allowed,
                  created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    perm.user_id,
                    perm.domain,
                    perm.trust_level,
                    perm.per_tx_limit_cents,
                    perm.per_session_limit_cents,
                    perm.rate_limit_per_min,
                    perm.max_tx_per_session,
                    if perm.identity_key_disclosure_allowed { 1_i64 } else { 0_i64 },
                    now,
                    now,
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    /// Update only the trust level for a permission
    pub fn update_trust_level(&self, id: i64, trust_level: &str) -> Result<()> {
        let now = unix_now();
        self.conn.execute(
            "UPDATE domain_permissions SET trust_level = ?1, updated_at = ?2 WHERE id = ?3",
            params![trust_level, now, id],
        )?;
        Ok(())
    }

    /// List all domain permissions for a user
    pub fn list_all(&self, user_id: i64) -> Result<Vec<DomainPermission>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, user_id, domain, trust_level, per_tx_limit_cents, per_session_limit_cents,
                    rate_limit_per_min, max_tx_per_session, identity_key_disclosure_allowed,
                    created_at, updated_at
             FROM domain_permissions WHERE user_id = ?1 ORDER BY domain"
        )?;
        let rows = stmt.query_map(params![user_id], |row| Ok(DomainPermission {
            id: Some(row.get(0)?),
            user_id: row.get(1)?,
            domain: row.get(2)?,
            trust_level: row.get(3)?,
            per_tx_limit_cents: row.get(4)?,
            per_session_limit_cents: row.get(5)?,
            rate_limit_per_min: row.get(6)?,
            max_tx_per_session: row.get(7)?,
            identity_key_disclosure_allowed: row.get::<_, i64>(8)? != 0,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
        }))?.collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Delete a domain permission (and cascade-delete its cert field permissions)
    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM domain_permissions WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Reset all domain permissions to the given limits
    pub fn reset_all_limits(&self, user_id: i64, per_tx: i64, per_session: i64, rate: i64, max_tx_per_session: i64) -> Result<usize> {
        let now = unix_now();
        let count = self.conn.execute(
            "UPDATE domain_permissions SET per_tx_limit_cents = ?1, per_session_limit_cents = ?2,
             rate_limit_per_min = ?3, max_tx_per_session = ?4, updated_at = ?5 WHERE user_id = ?6",
            params![per_tx, per_session, rate, max_tx_per_session, now, user_id],
        )?;
        Ok(count)
    }

    // ========================================================================
    // Certificate field permissions
    // ========================================================================

    /// Get list of approved field names for a domain+cert_type combination
    pub fn get_approved_fields(&self, domain_perm_id: i64, cert_type: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT field_name FROM cert_field_permissions
             WHERE domain_permission_id = ?1 AND cert_type = ?2
             ORDER BY field_name"
        )?;
        let rows = stmt.query_map(params![domain_perm_id, cert_type], |row| {
            row.get::<_, String>(0)
        })?.collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Approve cert fields for a domain (idempotent — uses INSERT OR IGNORE)
    pub fn approve_fields(&self, domain_perm_id: i64, cert_type: &str, fields: &[&str]) -> Result<()> {
        let now = unix_now();
        let mut stmt = self.conn.prepare(
            "INSERT OR IGNORE INTO cert_field_permissions
             (domain_permission_id, cert_type, field_name, created_at)
             VALUES (?1, ?2, ?3, ?4)"
        )?;
        for field in fields {
            stmt.execute(params![domain_perm_id, cert_type, field, now])?;
        }
        Ok(())
    }

    /// Revoke a single cert field approval
    pub fn revoke_field(&self, domain_perm_id: i64, cert_type: &str, field_name: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM cert_field_permissions
             WHERE domain_permission_id = ?1 AND cert_type = ?2 AND field_name = ?3",
            params![domain_perm_id, cert_type, field_name],
        )?;
        Ok(())
    }

    /// Check which fields are approved vs unapproved for a domain+cert_type.
    /// Returns (approved_fields, unapproved_fields).
    pub fn check_fields_approved(
        &self,
        domain_perm_id: i64,
        cert_type: &str,
        fields: &[&str],
    ) -> Result<(Vec<String>, Vec<String>)> {
        let approved = self.get_approved_fields(domain_perm_id, cert_type)?;
        let approved_set: HashSet<&str> = approved.iter().map(|s| s.as_str()).collect();

        let mut yes = Vec::new();
        let mut no = Vec::new();
        for f in fields {
            if approved_set.contains(*f) {
                yes.push(f.to_string());
            } else {
                no.push(f.to_string());
            }
        }
        Ok((yes, no))
    }

    // ========================================================================
    // Phase 1.5 Step 2 — Protocol permissions (V18)
    // ========================================================================

    /// Grant a per-protocol permission for a domain (idempotent).
    ///
    /// On UNIQUE conflict (same domain + level + name + key_id + counterparty),
    /// clears any prior `revoked_at` instead of failing — so re-granting after
    /// a soft-delete reactivates the row.
    ///
    /// Returns the row id.
    pub fn grant_protocol(
        &self,
        domain_perm_id: i64,
        security_level: u8,
        protocol_name: &str,
        key_id: &str,
        counterparty: Option<&str>,
        expires_at: Option<i64>,
    ) -> Result<i64> {
        let now = unix_now();

        // Existing row?
        let existing: Option<i64> = match self.conn.query_row(
            "SELECT id FROM domain_protocol_permissions
             WHERE domain_permission_id = ?1
               AND protocol_security_level = ?2
               AND protocol_name = ?3
               AND key_id = ?4
               AND ((counterparty IS NULL AND ?5 IS NULL) OR counterparty = ?5)",
            params![domain_perm_id, security_level as i64, protocol_name, key_id, counterparty],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(id) => Some(id),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };

        if let Some(id) = existing {
            self.conn.execute(
                "UPDATE domain_protocol_permissions
                 SET expires_at = ?1, revoked_at = NULL
                 WHERE id = ?2",
                params![expires_at, id],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO domain_protocol_permissions
                 (domain_permission_id, protocol_security_level, protocol_name,
                  key_id, counterparty, expires_at, revoked_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)",
                params![
                    domain_perm_id,
                    security_level as i64,
                    protocol_name,
                    key_id,
                    counterparty,
                    expires_at,
                    now,
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    /// Soft-revoke a protocol permission row (sets `revoked_at = now`).
    pub fn revoke_protocol(&self, id: i64) -> Result<()> {
        let now = unix_now();
        self.conn.execute(
            "UPDATE domain_protocol_permissions SET revoked_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    /// List ACTIVE protocol permissions (revoked_at IS NULL, not yet expired).
    pub fn list_protocols(&self, domain_perm_id: i64) -> Result<Vec<DomainProtocolPermission>> {
        let now = unix_now();
        let mut stmt = self.conn.prepare(
            "SELECT id, domain_permission_id, protocol_security_level, protocol_name,
                    key_id, counterparty, expires_at, revoked_at, created_at
             FROM domain_protocol_permissions
             WHERE domain_permission_id = ?1
               AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > ?2)
             ORDER BY protocol_name, key_id"
        )?;
        let rows = stmt.query_map(params![domain_perm_id, now], |row| Ok(DomainProtocolPermission {
            id: Some(row.get(0)?),
            domain_permission_id: row.get(1)?,
            protocol_security_level: row.get::<_, i64>(2)? as u8,
            protocol_name: row.get(3)?,
            key_id: row.get(4)?,
            counterparty: row.get(5)?,
            expires_at: row.get(6)?,
            revoked_at: row.get(7)?,
            created_at: row.get(8)?,
        }))?.collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// List ALL protocol permissions (including revoked + expired) for audit views.
    pub fn list_protocols_all(&self, domain_perm_id: i64) -> Result<Vec<DomainProtocolPermission>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, domain_permission_id, protocol_security_level, protocol_name,
                    key_id, counterparty, expires_at, revoked_at, created_at
             FROM domain_protocol_permissions
             WHERE domain_permission_id = ?1
             ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map(params![domain_perm_id], |row| Ok(DomainProtocolPermission {
            id: Some(row.get(0)?),
            domain_permission_id: row.get(1)?,
            protocol_security_level: row.get::<_, i64>(2)? as u8,
            protocol_name: row.get(3)?,
            key_id: row.get(4)?,
            counterparty: row.get(5)?,
            expires_at: row.get(6)?,
            revoked_at: row.get(7)?,
            created_at: row.get(8)?,
        }))?.collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Check whether a specific (level, protocol, key_id, counterparty) tuple is
    /// currently granted (active + not expired). `key_id = "*"` matches any.
    pub fn is_protocol_granted(
        &self,
        domain_perm_id: i64,
        security_level: u8,
        protocol_name: &str,
        key_id: &str,
        counterparty: Option<&str>,
    ) -> Result<bool> {
        let now = unix_now();
        // Match either the exact key_id OR a wildcard "*" row.
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM domain_protocol_permissions
             WHERE domain_permission_id = ?1
               AND protocol_security_level = ?2
               AND protocol_name = ?3
               AND (key_id = ?4 OR key_id = '*')
               AND ((counterparty IS NULL AND ?5 IS NULL) OR counterparty = ?5 OR counterparty IS NULL)
               AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > ?6)",
            params![domain_perm_id, security_level as i64, protocol_name, key_id, counterparty, now],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // ========================================================================
    // Phase 1.5 Step 2 — Basket permissions (V18)
    // ========================================================================

    /// Grant a per-basket permission. Re-granting an existing basket updates
    /// access + clears revoked_at.
    pub fn grant_basket(
        &self,
        domain_perm_id: i64,
        basket: &str,
        access: &str,
        expires_at: Option<i64>,
    ) -> Result<i64> {
        let now = unix_now();

        let existing: Option<i64> = match self.conn.query_row(
            "SELECT id FROM domain_basket_permissions
             WHERE domain_permission_id = ?1 AND basket = ?2",
            params![domain_perm_id, basket],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(id) => Some(id),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };

        if let Some(id) = existing {
            self.conn.execute(
                "UPDATE domain_basket_permissions
                 SET access = ?1, expires_at = ?2, revoked_at = NULL
                 WHERE id = ?3",
                params![access, expires_at, id],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO domain_basket_permissions
                 (domain_permission_id, basket, access, expires_at, revoked_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
                params![domain_perm_id, basket, access, expires_at, now],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    pub fn revoke_basket(&self, id: i64) -> Result<()> {
        let now = unix_now();
        self.conn.execute(
            "UPDATE domain_basket_permissions SET revoked_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn list_baskets(&self, domain_perm_id: i64) -> Result<Vec<DomainBasketPermission>> {
        let now = unix_now();
        let mut stmt = self.conn.prepare(
            "SELECT id, domain_permission_id, basket, access, expires_at, revoked_at, created_at
             FROM domain_basket_permissions
             WHERE domain_permission_id = ?1
               AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > ?2)
             ORDER BY basket"
        )?;
        let rows = stmt.query_map(params![domain_perm_id, now], |row| Ok(DomainBasketPermission {
            id: Some(row.get(0)?),
            domain_permission_id: row.get(1)?,
            basket: row.get(2)?,
            access: row.get(3)?,
            expires_at: row.get(4)?,
            revoked_at: row.get(5)?,
            created_at: row.get(6)?,
        }))?.collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn list_baskets_all(&self, domain_perm_id: i64) -> Result<Vec<DomainBasketPermission>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, domain_permission_id, basket, access, expires_at, revoked_at, created_at
             FROM domain_basket_permissions
             WHERE domain_permission_id = ?1
             ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map(params![domain_perm_id], |row| Ok(DomainBasketPermission {
            id: Some(row.get(0)?),
            domain_permission_id: row.get(1)?,
            basket: row.get(2)?,
            access: row.get(3)?,
            expires_at: row.get(4)?,
            revoked_at: row.get(5)?,
            created_at: row.get(6)?,
        }))?.collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Check whether a basket is currently accessible (active + not expired).
    /// `required_access`: if "read_write", a "read" grant returns false.
    pub fn is_basket_granted(
        &self,
        domain_perm_id: i64,
        basket: &str,
        required_access: &str,
    ) -> Result<bool> {
        let now = unix_now();
        let access: Option<String> = match self.conn.query_row(
            "SELECT access FROM domain_basket_permissions
             WHERE domain_permission_id = ?1
               AND basket = ?2
               AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > ?3)",
            params![domain_perm_id, basket, now],
            |row| row.get::<_, String>(0),
        ) {
            Ok(a) => Some(a),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };
        Ok(match (access.as_deref(), required_access) {
            (Some("read_write"), _) => true,        // read_write satisfies both
            (Some("read"), "read") => true,
            _ => false,
        })
    }

    // ========================================================================
    // Phase 1.5 Step 2 — Counterparty permissions (V18)
    // ========================================================================

    pub fn grant_counterparty(
        &self,
        domain_perm_id: i64,
        counterparty: &str,
        expires_at: Option<i64>,
    ) -> Result<i64> {
        let now = unix_now();

        let existing: Option<i64> = match self.conn.query_row(
            "SELECT id FROM domain_counterparty_permissions
             WHERE domain_permission_id = ?1 AND counterparty = ?2",
            params![domain_perm_id, counterparty],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(id) => Some(id),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };

        if let Some(id) = existing {
            self.conn.execute(
                "UPDATE domain_counterparty_permissions
                 SET expires_at = ?1, revoked_at = NULL
                 WHERE id = ?2",
                params![expires_at, id],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO domain_counterparty_permissions
                 (domain_permission_id, counterparty, expires_at, revoked_at, created_at)
                 VALUES (?1, ?2, ?3, NULL, ?4)",
                params![domain_perm_id, counterparty, expires_at, now],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    pub fn revoke_counterparty(&self, id: i64) -> Result<()> {
        let now = unix_now();
        self.conn.execute(
            "UPDATE domain_counterparty_permissions SET revoked_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn list_counterparties(&self, domain_perm_id: i64) -> Result<Vec<DomainCounterpartyPermission>> {
        let now = unix_now();
        let mut stmt = self.conn.prepare(
            "SELECT id, domain_permission_id, counterparty, expires_at, revoked_at, created_at
             FROM domain_counterparty_permissions
             WHERE domain_permission_id = ?1
               AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > ?2)
             ORDER BY counterparty"
        )?;
        let rows = stmt.query_map(params![domain_perm_id, now], |row| Ok(DomainCounterpartyPermission {
            id: Some(row.get(0)?),
            domain_permission_id: row.get(1)?,
            counterparty: row.get(2)?,
            expires_at: row.get(3)?,
            revoked_at: row.get(4)?,
            created_at: row.get(5)?,
        }))?.collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn list_counterparties_all(&self, domain_perm_id: i64) -> Result<Vec<DomainCounterpartyPermission>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, domain_permission_id, counterparty, expires_at, revoked_at, created_at
             FROM domain_counterparty_permissions
             WHERE domain_permission_id = ?1
             ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map(params![domain_perm_id], |row| Ok(DomainCounterpartyPermission {
            id: Some(row.get(0)?),
            domain_permission_id: row.get(1)?,
            counterparty: row.get(2)?,
            expires_at: row.get(3)?,
            revoked_at: row.get(4)?,
            created_at: row.get(5)?,
        }))?.collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn is_counterparty_granted(
        &self,
        domain_perm_id: i64,
        counterparty: &str,
    ) -> Result<bool> {
        let now = unix_now();
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM domain_counterparty_permissions
             WHERE domain_permission_id = ?1
               AND counterparty = ?2
               AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > ?3)",
            params![domain_perm_id, counterparty, now],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

// ============================================================================
// Phase 1.5 Step 2 — inline tests for the three child-table CRUD method groups.
// Uses an in-memory SQLite DB seeded with the V1 schema + V17 + V18 migrations.
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::migrations;

    /// Create an in-memory DB at the V18 schema state, with one users row
    /// (user_id = 1) and one domain_permissions row (id = 1) pre-seeded so
    /// child-table tests have a valid FK target.
    fn seed_db() -> (Connection, i64) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        migrations::create_schema_v1(&conn).unwrap();
        // Walk up the migration chain. The consolidated V1 already creates the
        // domain_permissions and cert_field_permissions tables, so V2-V16 are
        // mostly no-ops on this in-memory DB; we just need V17 and V18 to land.
        migrations::migrate_v16_to_v17(&conn).unwrap();
        migrations::migrate_v17_to_v18(&conn).unwrap();

        // Seed user + domain_permissions row for FK target.
        conn.execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at)
             VALUES (1, 'test_identity_key', 'local', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO domain_permissions
             (user_id, domain, trust_level, per_tx_limit_cents, per_session_limit_cents,
              rate_limit_per_min, max_tx_per_session, created_at, updated_at)
             VALUES (1, 'example.com', 'approved', 100, 1000, 30, 100, 0, 0)",
            [],
        ).unwrap();
        let domain_perm_id: i64 = conn.query_row(
            "SELECT id FROM domain_permissions WHERE domain = 'example.com'",
            [],
            |row| row.get(0),
        ).unwrap();
        (conn, domain_perm_id)
    }

    #[test]
    fn protocol_grant_revoke_round_trip() {
        let (conn, dpid) = seed_db();
        let repo = DomainPermissionRepository::new(&conn);

        let id = repo.grant_protocol(dpid, 2, "test protocol", "key1", None, None).unwrap();
        assert!(repo.is_protocol_granted(dpid, 2, "test protocol", "key1", None).unwrap());
        assert_eq!(repo.list_protocols(dpid).unwrap().len(), 1);

        repo.revoke_protocol(id).unwrap();
        assert!(!repo.is_protocol_granted(dpid, 2, "test protocol", "key1", None).unwrap());
        assert_eq!(repo.list_protocols(dpid).unwrap().len(), 0);
        // Audit list still shows the row
        assert_eq!(repo.list_protocols_all(dpid).unwrap().len(), 1);
    }

    #[test]
    fn protocol_regrant_clears_revoked_at() {
        let (conn, dpid) = seed_db();
        let repo = DomainPermissionRepository::new(&conn);
        let id = repo.grant_protocol(dpid, 2, "p", "k", None, None).unwrap();
        repo.revoke_protocol(id).unwrap();
        let id2 = repo.grant_protocol(dpid, 2, "p", "k", None, None).unwrap();
        assert_eq!(id, id2, "re-grant should reuse the same row");
        assert!(repo.is_protocol_granted(dpid, 2, "p", "k", None).unwrap());
    }

    #[test]
    fn protocol_wildcard_key_id_matches_any_key() {
        let (conn, dpid) = seed_db();
        let repo = DomainPermissionRepository::new(&conn);
        repo.grant_protocol(dpid, 2, "wildcard proto", "*", None, None).unwrap();
        // Any specific key_id should now be granted under the wildcard.
        assert!(repo.is_protocol_granted(dpid, 2, "wildcard proto", "anything", None).unwrap());
        assert!(repo.is_protocol_granted(dpid, 2, "wildcard proto", "else", None).unwrap());
    }

    #[test]
    fn protocol_expiry_blocks_is_granted() {
        let (conn, dpid) = seed_db();
        let repo = DomainPermissionRepository::new(&conn);
        // Grant that expired 1000 seconds ago
        repo.grant_protocol(dpid, 2, "p", "k", None, Some(unix_now() - 1000)).unwrap();
        assert!(!repo.is_protocol_granted(dpid, 2, "p", "k", None).unwrap());
    }

    #[test]
    fn basket_read_write_satisfies_read_check() {
        let (conn, dpid) = seed_db();
        let repo = DomainPermissionRepository::new(&conn);
        repo.grant_basket(dpid, "test_basket", "read_write", None).unwrap();
        assert!(repo.is_basket_granted(dpid, "test_basket", "read").unwrap());
        assert!(repo.is_basket_granted(dpid, "test_basket", "read_write").unwrap());
    }

    #[test]
    fn basket_read_does_not_satisfy_read_write_check() {
        let (conn, dpid) = seed_db();
        let repo = DomainPermissionRepository::new(&conn);
        repo.grant_basket(dpid, "test_basket", "read", None).unwrap();
        assert!(repo.is_basket_granted(dpid, "test_basket", "read").unwrap());
        assert!(!repo.is_basket_granted(dpid, "test_basket", "read_write").unwrap());
    }

    #[test]
    fn counterparty_grant_revoke_round_trip() {
        let (conn, dpid) = seed_db();
        let repo = DomainPermissionRepository::new(&conn);
        let hex = "020000000000000000000000000000000000000000000000000000000000000001";
        let id = repo.grant_counterparty(dpid, hex, None).unwrap();
        assert!(repo.is_counterparty_granted(dpid, hex).unwrap());
        repo.revoke_counterparty(id).unwrap();
        assert!(!repo.is_counterparty_granted(dpid, hex).unwrap());
    }

    #[test]
    fn cascade_delete_from_domain_permissions_nukes_all_children() {
        let (conn, dpid) = seed_db();
        let repo = DomainPermissionRepository::new(&conn);
        repo.grant_protocol(dpid, 2, "p", "k", None, None).unwrap();
        repo.grant_basket(dpid, "b", "read", None).unwrap();
        repo.grant_counterparty(dpid, "02aa", None).unwrap();

        // Sanity: all three children exist
        assert_eq!(repo.list_protocols(dpid).unwrap().len(), 1);
        assert_eq!(repo.list_baskets(dpid).unwrap().len(), 1);
        assert_eq!(repo.list_counterparties(dpid).unwrap().len(), 1);

        // Delete the parent
        repo.delete(dpid).unwrap();

        // CASCADE should have wiped all child rows.
        let proto_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM domain_protocol_permissions WHERE domain_permission_id = ?1",
            params![dpid],
            |row| row.get(0),
        ).unwrap();
        let basket_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM domain_basket_permissions WHERE domain_permission_id = ?1",
            params![dpid],
            |row| row.get(0),
        ).unwrap();
        let cp_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM domain_counterparty_permissions WHERE domain_permission_id = ?1",
            params![dpid],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(proto_count, 0);
        assert_eq!(basket_count, 0);
        assert_eq!(cp_count, 0);
    }

    #[test]
    fn unique_constraint_dedupes_protocol_grant() {
        let (conn, dpid) = seed_db();
        let repo = DomainPermissionRepository::new(&conn);
        let id1 = repo.grant_protocol(dpid, 1, "p", "k", Some("02ab"), None).unwrap();
        // Identical key tuple should resolve to the same row, not duplicate.
        let id2 = repo.grant_protocol(dpid, 1, "p", "k", Some("02ab"), None).unwrap();
        assert_eq!(id1, id2);
        assert_eq!(repo.list_protocols(dpid).unwrap().len(), 1);
    }

    #[test]
    fn different_counterparty_creates_different_protocol_row() {
        let (conn, dpid) = seed_db();
        let repo = DomainPermissionRepository::new(&conn);
        let id_any = repo.grant_protocol(dpid, 1, "p", "k", None, None).unwrap();
        let id_cp = repo.grant_protocol(dpid, 1, "p", "k", Some("02ab"), None).unwrap();
        assert_ne!(id_any, id_cp);
        assert_eq!(repo.list_protocols(dpid).unwrap().len(), 2);
    }
}
