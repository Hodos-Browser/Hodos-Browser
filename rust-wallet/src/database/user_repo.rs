//! User repository for database operations
//!
//! Handles CRUD operations for users in the database.
//! Users represent identities (master public keys) that own wallet data.

use rusqlite::{Connection, Result};
use log::{info, error};
use std::time::{SystemTime, UNIX_EPOCH};
use super::models::User;

pub struct UserRepository<'a> {
    conn: &'a Connection,
}

impl<'a> UserRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        UserRepository { conn }
    }

    /// Create a new user with the given identity key (master public key)
    /// Returns the user ID
    pub fn create(&self, identity_key: &str) -> Result<i64> {
        info!("Creating new user with identity key: {}...", &identity_key[..16]);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO users (identity_key, active_storage, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                identity_key,
                "local",  // Default storage mode
                now,
                now,
            ],
        )?;

        let user_id = self.conn.last_insert_rowid();
        info!("✅ User created with ID: {}", user_id);

        Ok(user_id)
    }

    /// Get user by ID
    pub fn get_by_id(&self, user_id: i64) -> Result<Option<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT userId, identity_key, active_storage, created_at, updated_at
             FROM users
             WHERE userId = ?1"
        )?;

        let user_result = stmt.query_row(
            rusqlite::params![user_id],
            |row| {
                Ok(User {
                    user_id: Some(row.get(0)?),
                    identity_key: row.get(1)?,
                    active_storage: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        );

        match user_result {
            Ok(user) => Ok(Some(user)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get user by identity key (master public key)
    pub fn get_by_identity_key(&self, identity_key: &str) -> Result<Option<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT userId, identity_key, active_storage, created_at, updated_at
             FROM users
             WHERE identity_key = ?1"
        )?;

        let user_result = stmt.query_row(
            rusqlite::params![identity_key],
            |row| {
                Ok(User {
                    user_id: Some(row.get(0)?),
                    identity_key: row.get(1)?,
                    active_storage: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        );

        match user_result {
            Ok(user) => Ok(Some(user)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get the default (first) user
    /// For single-user wallets, this returns the only user
    pub fn get_default(&self) -> Result<Option<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT userId, identity_key, active_storage, created_at, updated_at
             FROM users
             ORDER BY userId ASC
             LIMIT 1"
        )?;

        let user_result = stmt.query_row(
            [],
            |row| {
                Ok(User {
                    user_id: Some(row.get(0)?),
                    identity_key: row.get(1)?,
                    active_storage: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        );

        match user_result {
            Ok(user) => Ok(Some(user)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Update user's active storage mode
    pub fn update_active_storage(&self, user_id: i64, active_storage: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "UPDATE users SET active_storage = ?1, updated_at = ?2 WHERE userId = ?3",
            rusqlite::params![active_storage, now, user_id],
        )?;
        Ok(())
    }
}
