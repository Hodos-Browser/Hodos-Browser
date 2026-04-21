//! Sync state repository for database operations
//!
//! Handles CRUD operations for multi-device synchronization state in the database.
//! Phase 5 of wallet-toolbox alignment.
//!
//! ## Purpose
//!
//! The sync_states table tracks synchronization between local wallet and remote
//! storage providers. Each sync state represents a sync session with:
//! - Reference number (unique identifier)
//! - Status (unknown, syncing, synced, error)
//! - Sync map (JSON tracking which entities have been synced)
//! - Error messages for debugging sync failures

use rusqlite::{Connection, Result};
use log::info;
use std::time::{SystemTime, UNIX_EPOCH};

use super::models::SyncState;

pub struct SyncStateRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SyncStateRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        SyncStateRepository { conn }
    }

    /// Create a new sync state record
    pub fn create(&self, sync_state: &SyncState) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO sync_states
             (user_id, storage_identity_key, storage_name, status, init, ref_num,
              sync_map, sync_when, satoshis, error_local, error_other, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                sync_state.user_id,
                sync_state.storage_identity_key,
                sync_state.storage_name,
                sync_state.status,
                sync_state.init as i32,
                sync_state.ref_num,
                sync_state.sync_map,
                sync_state.sync_when,
                sync_state.satoshis,
                sync_state.error_local,
                sync_state.error_other,
                now,
                now,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        info!("   ✅ Created sync state {} for user {} (ref: {})",
              id, sync_state.user_id, sync_state.ref_num);
        Ok(id)
    }

    /// Get a sync state by ID
    pub fn get_by_id(&self, sync_state_id: i64) -> Result<Option<SyncState>> {
        self.conn.query_row(
            "SELECT syncStateId, user_id, storage_identity_key, storage_name, status, init,
                    ref_num, sync_map, sync_when, satoshis, error_local, error_other,
                    created_at, updated_at
             FROM sync_states WHERE syncStateId = ?1",
            rusqlite::params![sync_state_id],
            |row| Ok(SyncState {
                sync_state_id: Some(row.get(0)?),
                user_id: row.get(1)?,
                storage_identity_key: row.get(2)?,
                storage_name: row.get(3)?,
                status: row.get(4)?,
                init: row.get::<_, i32>(5)? != 0,
                ref_num: row.get(6)?,
                sync_map: row.get(7)?,
                sync_when: row.get(8)?,
                satoshis: row.get(9)?,
                error_local: row.get(10)?,
                error_other: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            }),
        ).optional()
    }

    /// Get a sync state by reference number
    pub fn get_by_ref_num(&self, ref_num: &str) -> Result<Option<SyncState>> {
        self.conn.query_row(
            "SELECT syncStateId, user_id, storage_identity_key, storage_name, status, init,
                    ref_num, sync_map, sync_when, satoshis, error_local, error_other,
                    created_at, updated_at
             FROM sync_states WHERE ref_num = ?1",
            rusqlite::params![ref_num],
            |row| Ok(SyncState {
                sync_state_id: Some(row.get(0)?),
                user_id: row.get(1)?,
                storage_identity_key: row.get(2)?,
                storage_name: row.get(3)?,
                status: row.get(4)?,
                init: row.get::<_, i32>(5)? != 0,
                ref_num: row.get(6)?,
                sync_map: row.get(7)?,
                sync_when: row.get(8)?,
                satoshis: row.get(9)?,
                error_local: row.get(10)?,
                error_other: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            }),
        ).optional()
    }

    /// Get all sync states for a user
    pub fn get_by_user(&self, user_id: i64) -> Result<Vec<SyncState>> {
        let mut stmt = self.conn.prepare(
            "SELECT syncStateId, user_id, storage_identity_key, storage_name, status, init,
                    ref_num, sync_map, sync_when, satoshis, error_local, error_other,
                    created_at, updated_at
             FROM sync_states
             WHERE user_id = ?1
             ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map(
            rusqlite::params![user_id],
            |row| Ok(SyncState {
                sync_state_id: Some(row.get(0)?),
                user_id: row.get(1)?,
                storage_identity_key: row.get(2)?,
                storage_name: row.get(3)?,
                status: row.get(4)?,
                init: row.get::<_, i32>(5)? != 0,
                ref_num: row.get(6)?,
                sync_map: row.get(7)?,
                sync_when: row.get(8)?,
                satoshis: row.get(9)?,
                error_local: row.get(10)?,
                error_other: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            }),
        )?;

        let mut states = Vec::new();
        for row in rows {
            states.push(row?);
        }

        Ok(states)
    }

    /// Get pending/active sync states (not completed or errored)
    pub fn get_pending(&self, user_id: i64) -> Result<Vec<SyncState>> {
        let mut stmt = self.conn.prepare(
            "SELECT syncStateId, user_id, storage_identity_key, storage_name, status, init,
                    ref_num, sync_map, sync_when, satoshis, error_local, error_other,
                    created_at, updated_at
             FROM sync_states
             WHERE user_id = ?1 AND status IN ('unknown', 'syncing')
             ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map(
            rusqlite::params![user_id],
            |row| Ok(SyncState {
                sync_state_id: Some(row.get(0)?),
                user_id: row.get(1)?,
                storage_identity_key: row.get(2)?,
                storage_name: row.get(3)?,
                status: row.get(4)?,
                init: row.get::<_, i32>(5)? != 0,
                ref_num: row.get(6)?,
                sync_map: row.get(7)?,
                sync_when: row.get(8)?,
                satoshis: row.get(9)?,
                error_local: row.get(10)?,
                error_other: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            }),
        )?;

        let mut states = Vec::new();
        for row in rows {
            states.push(row?);
        }

        Ok(states)
    }

    /// Update sync state status
    pub fn update_status(&self, sync_state_id: i64, status: &str) -> Result<bool> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let updated = self.conn.execute(
            "UPDATE sync_states SET status = ?1, updated_at = ?2 WHERE syncStateId = ?3",
            rusqlite::params![status, now, sync_state_id],
        )?;

        Ok(updated > 0)
    }

    /// Update sync map (JSON sync state)
    pub fn update_sync_map(&self, sync_state_id: i64, sync_map: &str) -> Result<bool> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let updated = self.conn.execute(
            "UPDATE sync_states SET sync_map = ?1, sync_when = ?2, updated_at = ?3
             WHERE syncStateId = ?4",
            rusqlite::params![sync_map, now, now, sync_state_id],
        )?;

        Ok(updated > 0)
    }

    /// Mark sync as complete
    pub fn mark_synced(&self, sync_state_id: i64, satoshis: i64) -> Result<bool> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let updated = self.conn.execute(
            "UPDATE sync_states SET status = 'synced', satoshis = ?1, sync_when = ?2, updated_at = ?3
             WHERE syncStateId = ?4",
            rusqlite::params![satoshis, now, now, sync_state_id],
        )?;

        Ok(updated > 0)
    }

    /// Mark sync as errored
    pub fn mark_error(&self, sync_state_id: i64, error_local: Option<&str>, error_other: Option<&str>) -> Result<bool> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let updated = self.conn.execute(
            "UPDATE sync_states SET status = 'error', error_local = ?1, error_other = ?2, updated_at = ?3
             WHERE syncStateId = ?4",
            rusqlite::params![error_local, error_other, now, sync_state_id],
        )?;

        Ok(updated > 0)
    }

    /// Mark init as complete
    pub fn mark_init_complete(&self, sync_state_id: i64) -> Result<bool> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let updated = self.conn.execute(
            "UPDATE sync_states SET init = 1, updated_at = ?1 WHERE syncStateId = ?2",
            rusqlite::params![now, sync_state_id],
        )?;

        Ok(updated > 0)
    }

    /// Delete a sync state
    pub fn delete(&self, sync_state_id: i64) -> Result<bool> {
        let deleted = self.conn.execute(
            "DELETE FROM sync_states WHERE syncStateId = ?1",
            rusqlite::params![sync_state_id],
        )?;

        Ok(deleted > 0)
    }

    /// Clean up old completed sync states (keep last N per user)
    pub fn cleanup_old(&self, user_id: i64, keep_count: usize) -> Result<usize> {
        // Get IDs of sync states to keep (most recent N)
        let mut stmt = self.conn.prepare(
            "SELECT syncStateId FROM sync_states
             WHERE user_id = ?1 AND status = 'synced'
             ORDER BY created_at DESC
             LIMIT ?2"
        )?;

        let keep_ids: Vec<i64> = stmt.query_map(
            rusqlite::params![user_id, keep_count as i64],
            |row| row.get(0),
        )?.filter_map(|r| r.ok()).collect();

        if keep_ids.is_empty() {
            return Ok(0);
        }

        // Delete older synced states
        let placeholders: Vec<String> = (0..keep_ids.len())
            .map(|_| "?".to_string())
            .collect();

        let query = format!(
            "DELETE FROM sync_states
             WHERE user_id = ? AND status = 'synced' AND syncStateId NOT IN ({})",
            placeholders.join(",")
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(user_id)];
        for id in keep_ids {
            params.push(Box::new(id));
        }

        let deleted = self.conn.execute(
            &query,
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
        )?;

        Ok(deleted)
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
