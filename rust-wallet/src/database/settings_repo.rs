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
                    created_at, updated_at
             FROM settings LIMIT 1",
            [],
            |row| Ok(Setting {
                storage_identity_key: row.get(0)?,
                storage_name: row.get(1)?,
                chain: row.get(2)?,
                db_type: row.get(3)?,
                max_output_script: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
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
                    updated_at = ?6",
                rusqlite::params![
                    setting.storage_identity_key,
                    setting.storage_name,
                    setting.chain,
                    setting.db_type,
                    setting.max_output_script,
                    now,
                ],
            )?;
            info!("   ✅ Updated settings");
        } else {
            // Insert new settings
            self.conn.execute(
                "INSERT INTO settings
                 (storage_identity_key, storage_name, chain, dbtype, max_output_script,
                  created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    setting.storage_identity_key,
                    setting.storage_name,
                    setting.chain,
                    setting.db_type,
                    setting.max_output_script,
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
                created_at: 0, // Will be set by upsert
                updated_at: 0,
            };
            self.upsert(&default_setting)?;
            info!("   ✅ Created default settings");
        }

        Ok(())
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
