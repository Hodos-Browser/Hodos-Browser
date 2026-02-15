//! Commission repository for database operations
//!
//! Handles CRUD operations for transaction commissions in the database.
//! Phase 5 of wallet-toolbox alignment.
//!
//! ## Purpose
//!
//! Commissions track fee outputs paid to wallet service providers.
//! Each transaction can have at most one commission (enforced by unique constraint).
//! Commissions can be redeemed (claimed) by the service provider.

use rusqlite::{Connection, Result};
use log::info;
use std::time::{SystemTime, UNIX_EPOCH};

use super::models::Commission;

pub struct CommissionRepository<'a> {
    conn: &'a Connection,
}

impl<'a> CommissionRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        CommissionRepository { conn }
    }

    /// Create a new commission record
    pub fn create(&self, commission: &Commission) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO commissions
             (user_id, transaction_id, satoshis, key_offset, is_redeemed, locking_script, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                commission.user_id,
                commission.transaction_id,
                commission.satoshis,
                commission.key_offset,
                commission.is_redeemed as i32,
                commission.locking_script,
                now,
                now,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        info!("   ✅ Created commission {} for transaction {} ({} sats)",
              id, commission.transaction_id, commission.satoshis);
        Ok(id)
    }

    /// Get a commission by ID
    pub fn get_by_id(&self, commission_id: i64) -> Result<Option<Commission>> {
        self.conn.query_row(
            "SELECT commissionId, user_id, transaction_id, satoshis, key_offset,
                    is_redeemed, locking_script, created_at, updated_at
             FROM commissions WHERE commissionId = ?1",
            rusqlite::params![commission_id],
            |row| Ok(Commission {
                commission_id: Some(row.get(0)?),
                user_id: row.get(1)?,
                transaction_id: row.get(2)?,
                satoshis: row.get(3)?,
                key_offset: row.get(4)?,
                is_redeemed: row.get::<_, i32>(5)? != 0,
                locking_script: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            }),
        ).optional()
    }

    /// Get commission for a transaction
    pub fn get_by_transaction_id(&self, transaction_id: i64) -> Result<Option<Commission>> {
        self.conn.query_row(
            "SELECT commissionId, user_id, transaction_id, satoshis, key_offset,
                    is_redeemed, locking_script, created_at, updated_at
             FROM commissions WHERE transaction_id = ?1",
            rusqlite::params![transaction_id],
            |row| Ok(Commission {
                commission_id: Some(row.get(0)?),
                user_id: row.get(1)?,
                transaction_id: row.get(2)?,
                satoshis: row.get(3)?,
                key_offset: row.get(4)?,
                is_redeemed: row.get::<_, i32>(5)? != 0,
                locking_script: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            }),
        ).optional()
    }

    /// Get all unredeemed commissions for a user
    pub fn get_unredeemed(&self, user_id: i64) -> Result<Vec<Commission>> {
        let mut stmt = self.conn.prepare(
            "SELECT commissionId, user_id, transaction_id, satoshis, key_offset,
                    is_redeemed, locking_script, created_at, updated_at
             FROM commissions
             WHERE user_id = ?1 AND is_redeemed = 0
             ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map(
            rusqlite::params![user_id],
            |row| Ok(Commission {
                commission_id: Some(row.get(0)?),
                user_id: row.get(1)?,
                transaction_id: row.get(2)?,
                satoshis: row.get(3)?,
                key_offset: row.get(4)?,
                is_redeemed: row.get::<_, i32>(5)? != 0,
                locking_script: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            }),
        )?;

        let mut commissions = Vec::new();
        for row in rows {
            commissions.push(row?);
        }

        Ok(commissions)
    }

    /// Get all commissions for a user
    pub fn get_all(&self, user_id: i64) -> Result<Vec<Commission>> {
        let mut stmt = self.conn.prepare(
            "SELECT commissionId, user_id, transaction_id, satoshis, key_offset,
                    is_redeemed, locking_script, created_at, updated_at
             FROM commissions
             WHERE user_id = ?1
             ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map(
            rusqlite::params![user_id],
            |row| Ok(Commission {
                commission_id: Some(row.get(0)?),
                user_id: row.get(1)?,
                transaction_id: row.get(2)?,
                satoshis: row.get(3)?,
                key_offset: row.get(4)?,
                is_redeemed: row.get::<_, i32>(5)? != 0,
                locking_script: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            }),
        )?;

        let mut commissions = Vec::new();
        for row in rows {
            commissions.push(row?);
        }

        Ok(commissions)
    }

    /// Mark a commission as redeemed
    pub fn mark_redeemed(&self, commission_id: i64) -> Result<bool> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let updated = self.conn.execute(
            "UPDATE commissions SET is_redeemed = 1, updated_at = ?1
             WHERE commissionId = ?2 AND is_redeemed = 0",
            rusqlite::params![now, commission_id],
        )?;

        Ok(updated > 0)
    }

    /// Calculate total unredeemed commission satoshis for a user
    pub fn get_total_unredeemed(&self, user_id: i64) -> Result<i64> {
        self.conn.query_row(
            "SELECT COALESCE(SUM(satoshis), 0) FROM commissions
             WHERE user_id = ?1 AND is_redeemed = 0",
            rusqlite::params![user_id],
            |row| row.get(0),
        )
    }

    /// Delete commission by transaction ID
    /// Used when a transaction fails and needs cleanup
    pub fn delete_by_transaction_id(&self, transaction_id: i64) -> Result<bool> {
        let deleted = self.conn.execute(
            "DELETE FROM commissions WHERE transaction_id = ?1",
            rusqlite::params![transaction_id],
        )?;

        Ok(deleted > 0)
    }
}

// Add the optional() trait for Result<T>
trait ResultExt<T> {
    fn optional(self) -> Result<Option<T>>;
}

impl<T> ResultExt<T> for Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
