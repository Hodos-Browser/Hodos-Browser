//! Settings repository for database operations
//!
//! Handles CRUD operations for wallet settings in the database.
//! Phase 5 of wallet-toolbox alignment.
//!
//! ## Purpose
//!
//! The settings table stores persistent wallet configuration including:
//! - Cloud storage identity and name
//! - Network chain (main/test)
//! - Database type
//! - Script size limits
//!
//! There is typically one settings row per wallet instance.

use rusqlite::{Connection, Result};
use log::info;
use std::time::{SystemTime, UNIX_EPOCH};

use super::models::Setting;

pub struct SettingsRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SettingsRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        SettingsRepository { conn }
    }

    /// Get the current settings (first row)
    pub fn get(&self) -> Result<Option<Setting>> {
        self.conn.query_row(
            "SELECT storage_identity_key, storage_name, chain, dbtype, max_output_script,
                    sender_display_name, created_at, updated_at
             FROM settings LIMIT 1",
            [],
            |row| Ok(Setting {
                storage_identity_key: row.get(0)?,
                storage_name: row.get(1)?,
                chain: row.get(2)?,
                db_type: row.get(3)?,
                max_output_script: row.get(4)?,
                sender_display_name: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            }),
        ).optional()
    }

    /// Create or update settings
    ///
    /// Since there's typically only one settings row, this replaces any existing settings.
    pub fn upsert(&self, setting: &Setting) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Check if settings exist
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings LIMIT 1)",
            [],
            |row| row.get(0),
        )?;

        if exists {
            // Update existing settings
            self.conn.execute(
                "UPDATE settings SET
                    storage_identity_key = ?1,
                    storage_name = ?2,
                    chain = ?3,
                    dbtype = ?4,
                    max_output_script = ?5,
                    sender_display_name = ?6,
                    updated_at = ?7",
                rusqlite::params![
                    setting.storage_identity_key,
                    setting.storage_name,
                    setting.chain,
                    setting.db_type,
                    setting.max_output_script,
                    setting.sender_display_name,
                    now,
                ],
            )?;
            info!("   ✅ Updated settings");
        } else {
            // Insert new settings
            self.conn.execute(
                "INSERT INTO settings
                 (storage_identity_key, storage_name, chain, dbtype, max_output_script,
                  sender_display_name, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    setting.storage_identity_key,
                    setting.storage_name,
                    setting.chain,
                    setting.db_type,
                    setting.max_output_script,
                    setting.sender_display_name,
                    now,
                    now,
                ],
            )?;
            info!("   ✅ Created settings");
        }

        Ok(())
    }

    /// Create default settings if none exist
    pub fn ensure_defaults(&self) -> Result<()> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings LIMIT 1)",
            [],
            |row| row.get(0),
        )?;

        if !exists {
            let default_setting = Setting {
                storage_identity_key: String::new(),
                storage_name: String::new(),
                chain: "main".to_string(),
                db_type: "sqlite".to_string(),
                max_output_script: 500000,
                sender_display_name: "Anonymous".to_string(),
                created_at: 0, // Will be set by upsert
                updated_at: 0,
            };
            self.upsert(&default_setting)?;
            info!("   ✅ Created default settings");
        }

        Ok(())
    }

    /// Get the sender display name for paymail transactions
    pub fn get_sender_display_name(&self) -> Result<String> {
        match self.get()? {
            Some(s) => Ok(s.sender_display_name),
            None => Ok("Anonymous".to_string()),
        }
    }

    /// Get the current chain (main or test)
    pub fn get_chain(&self) -> Result<String> {
        match self.get()? {
            Some(s) => Ok(s.chain),
            None => Ok("main".to_string()), // Default to mainnet
        }
    }

    /// Update the chain setting
    pub fn set_chain(&self, chain: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings LIMIT 1)",
            [],
            |row| row.get(0),
        )?;

        if exists {
            self.conn.execute(
                "UPDATE settings SET chain = ?1, updated_at = ?2",
                rusqlite::params![chain, now],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO settings
                 (storage_identity_key, storage_name, chain, dbtype, max_output_script,
                  created_at, updated_at)
                 VALUES ('', '', ?1, 'sqlite', 500000, ?2, ?3)",
                rusqlite::params![chain, now, now],
            )?;
        }

        Ok(())
    }

    /// Get the maximum output script size
    pub fn get_max_output_script(&self) -> Result<i32> {
        match self.get()? {
            Some(s) => Ok(s.max_output_script),
            None => Ok(500000), // Default
        }
    }

    /// Update the maximum output script size
    pub fn set_max_output_script(&self, max_size: i32) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings LIMIT 1)",
            [],
            |row| row.get(0),
        )?;

        if exists {
            self.conn.execute(
                "UPDATE settings SET max_output_script = ?1, updated_at = ?2",
                rusqlite::params![max_size, now],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO settings
                 (storage_identity_key, storage_name, chain, dbtype, max_output_script,
                  created_at, updated_at)
                 VALUES ('', '', 'main', 'sqlite', ?1, ?2, ?3)",
                rusqlite::params![max_size, now, now],
            )?;
        }

        Ok(())
    }

    /// Update the sender display name
    pub fn set_sender_display_name(&self, name: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings LIMIT 1)",
            [],
            |row| row.get(0),
        )?;

        if exists {
            self.conn.execute(
                "UPDATE settings SET sender_display_name = ?1, updated_at = ?2",
                rusqlite::params![name, now],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO settings
                 (storage_identity_key, storage_name, chain, dbtype, max_output_script,
                  sender_display_name, created_at, updated_at)
                 VALUES ('', '', 'main', 'sqlite', 500000, ?1, ?2, ?3)",
                rusqlite::params![name, now, now],
            )?;
        }

        Ok(())
    }

    /// Get default auto-approve limits for new domains
    pub fn get_default_limits(&self) -> Result<(i64, i64, i64)> {
        let result = self.conn.query_row(
            "SELECT default_per_tx_limit_cents, default_per_session_limit_cents, default_rate_limit_per_min
             FROM settings LIMIT 1",
            [],
            |row| Ok((
                row.get::<_, i64>(0).unwrap_or(1000),
                row.get::<_, i64>(1).unwrap_or(5000),
                row.get::<_, i64>(2).unwrap_or(10),
            )),
        );
        match result {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok((1000, 5000, 10)),
            Err(e) => {
                // Column may not exist yet (pre-V10)
                info!("   default_limits query failed (pre-V10?): {}", e);
                Ok((1000, 5000, 10))
            }
        }
    }

    /// Set default auto-approve limits for new domains
    pub fn set_default_limits(&self, per_tx: i64, per_session: i64, rate: i64) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings LIMIT 1)",
            [],
            |row| row.get(0),
        )?;

        if exists {
            self.conn.execute(
                "UPDATE settings SET default_per_tx_limit_cents = ?1,
                 default_per_session_limit_cents = ?2, default_rate_limit_per_min = ?3,
                 updated_at = ?4",
                rusqlite::params![per_tx, per_session, rate, now],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO settings
                 (storage_identity_key, storage_name, chain, dbtype, max_output_script,
                  sender_display_name, default_per_tx_limit_cents, default_per_session_limit_cents,
                  default_rate_limit_per_min, created_at, updated_at)
                 VALUES ('', '', 'main', 'sqlite', 500000, 'Anonymous', ?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![per_tx, per_session, rate, now, now],
            )?;
        }

        Ok(())
    }

    /// Get the stored backup hash (SHA256 hex of last successful backup's compressed payload)
    pub fn get_backup_hash(&self) -> Result<Option<String>> {
        let result: rusqlite::Result<Option<String>> = self.conn.query_row(
            "SELECT backup_hash FROM settings LIMIT 1",
            [],
            |row| row.get(0),
        );
        match result {
            Ok(v) => Ok(v),
            Err(_) => Ok(None), // Column may not exist yet
        }
    }

    /// Store the backup hash after successful backup
    pub fn set_backup_hash(&self, hash: &str) -> Result<()> {
        let _ = self.conn.execute(
            "UPDATE settings SET backup_hash = ?1",
            rusqlite::params![hash],
        );
        Ok(())
    }

    /// Get the last backup timestamp (Unix seconds, 0 = never)
    pub fn get_last_backup_at(&self) -> Result<i64> {
        let result: rusqlite::Result<i64> = self.conn.query_row(
            "SELECT last_backup_at FROM settings LIMIT 1",
            [],
            |row| row.get(0),
        );
        match result {
            Ok(v) => Ok(v),
            Err(_) => Ok(0),
        }
    }

    /// Set the last backup timestamp
    pub fn set_last_backup_at(&self, timestamp: i64) -> Result<()> {
        let _ = self.conn.execute(
            "UPDATE settings SET last_backup_at = ?1",
            rusqlite::params![timestamp],
        );
        Ok(())
    }

    /// Update storage configuration
    pub fn set_storage(&self, identity_key: &str, name: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings LIMIT 1)",
            [],
            |row| row.get(0),
        )?;

        if exists {
            self.conn.execute(
                "UPDATE settings SET storage_identity_key = ?1, storage_name = ?2, updated_at = ?3",
                rusqlite::params![identity_key, name, now],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO settings
                 (storage_identity_key, storage_name, chain, dbtype, max_output_script,
                  created_at, updated_at)
                 VALUES (?1, ?2, 'main', 'sqlite', 500000, ?3, ?4)",
                rusqlite::params![identity_key, name, now, now],
            )?;
        }

        Ok(())
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
