//! Domain permission repository for database operations
//!
//! Handles CRUD operations for domain permissions and certificate field permissions.
//! Phase 2.1 of UX improvements — replaces JSON-file domain whitelist with
//! granular per-domain trust levels, spending limits, and cert field tracking.

use rusqlite::{Connection, Result, params};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use super::models::DomainPermission;

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
            "SELECT id, user_id, domain, trust_level, per_tx_limit_cents, per_day_limit_cents,
                    daily_spent_cents, daily_reset_at, rate_limit_per_min, created_at, updated_at
             FROM domain_permissions WHERE user_id = ?1 AND domain = ?2",
            params![user_id, domain],
            |row| Ok(DomainPermission {
                id: Some(row.get(0)?),
                user_id: row.get(1)?,
                domain: row.get(2)?,
                trust_level: row.get(3)?,
                per_tx_limit_cents: row.get(4)?,
                per_day_limit_cents: row.get(5)?,
                daily_spent_cents: row.get(6)?,
                daily_reset_at: row.get(7)?,
                rate_limit_per_min: row.get(8)?,
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
                    per_day_limit_cents = ?3,
                    rate_limit_per_min = ?4,
                    updated_at = ?5
                 WHERE id = ?6",
                params![
                    perm.trust_level,
                    perm.per_tx_limit_cents,
                    perm.per_day_limit_cents,
                    perm.rate_limit_per_min,
                    now,
                    id,
                ],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO domain_permissions
                 (user_id, domain, trust_level, per_tx_limit_cents, per_day_limit_cents,
                  daily_spent_cents, daily_reset_at, rate_limit_per_min, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    perm.user_id,
                    perm.domain,
                    perm.trust_level,
                    perm.per_tx_limit_cents,
                    perm.per_day_limit_cents,
                    perm.daily_spent_cents,
                    perm.daily_reset_at,
                    perm.rate_limit_per_min,
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

    /// Record spending against a domain's daily limit.
    /// Caller should call reset_daily_if_expired first.
    pub fn record_spending(&self, id: i64, amount_cents: i64) -> Result<()> {
        let now = unix_now();
        self.conn.execute(
            "UPDATE domain_permissions SET daily_spent_cents = daily_spent_cents + ?1, updated_at = ?2 WHERE id = ?3",
            params![amount_cents, now, id],
        )?;
        Ok(())
    }

    /// Reset daily spending counter if the last reset was before today's midnight UTC
    pub fn reset_daily_if_expired(&self, id: i64) -> Result<()> {
        let now = unix_now();
        let today_midnight = now - (now % 86400);
        self.conn.execute(
            "UPDATE domain_permissions SET daily_spent_cents = 0, daily_reset_at = ?1
             WHERE id = ?2 AND daily_reset_at < ?3",
            params![now, id, today_midnight],
        )?;
        Ok(())
    }

    /// List all domain permissions for a user
    pub fn list_all(&self, user_id: i64) -> Result<Vec<DomainPermission>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, user_id, domain, trust_level, per_tx_limit_cents, per_day_limit_cents,
                    daily_spent_cents, daily_reset_at, rate_limit_per_min, created_at, updated_at
             FROM domain_permissions WHERE user_id = ?1 ORDER BY domain"
        )?;
        let rows = stmt.query_map(params![user_id], |row| Ok(DomainPermission {
            id: Some(row.get(0)?),
            user_id: row.get(1)?,
            domain: row.get(2)?,
            trust_level: row.get(3)?,
            per_tx_limit_cents: row.get(4)?,
            per_day_limit_cents: row.get(5)?,
            daily_spent_cents: row.get(6)?,
            daily_reset_at: row.get(7)?,
            rate_limit_per_min: row.get(8)?,
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
}
