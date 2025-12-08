//! Basket repository for database operations
//!
//! Handles CRUD operations for baskets in the database.

use rusqlite::{Connection, Result};
use log::info;
use std::time::{SystemTime, UNIX_EPOCH};
use super::models::Basket;

pub struct BasketRepository<'a> {
    conn: &'a Connection,
}

impl<'a> BasketRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        BasketRepository { conn }
    }

    /// Find or insert a basket by name
    /// Returns the basket ID
    pub fn find_or_insert(&self, name: &str) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Try to find existing basket
        let basket_id: Result<i64> = self.conn.query_row(
            "SELECT id FROM baskets WHERE name = ?1",
            rusqlite::params![name],
            |row| row.get(0),
        );

        match basket_id {
            Ok(id) => {
                // Update last_used timestamp
                self.conn.execute(
                    "UPDATE baskets SET last_used = ?1 WHERE id = ?2",
                    rusqlite::params![now, id],
                )?;
                Ok(id)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Insert new basket
                self.conn.execute(
                    "INSERT INTO baskets (name, created_at, last_used) VALUES (?1, ?2, ?3)",
                    rusqlite::params![name, now, now],
                )?;
                let id = self.conn.last_insert_rowid();
                info!("   ✅ Created new basket '{}' with id {}", name, id);
                Ok(id)
            }
            Err(e) => Err(e),
        }
    }

    /// Find basket by name
    pub fn find_by_name(&self, name: &str) -> Result<Option<Basket>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, token_type, protocol_id, created_at, last_used
             FROM baskets WHERE name = ?1"
        )?;

        let basket = stmt.query_row(
            rusqlite::params![name],
            |row| {
                Ok(Basket {
                    id: Some(row.get(0)?),
                    name: row.get(1)?,
                    description: row.get(2)?,
                    token_type: row.get(3)?,
                    protocol_id: row.get(4)?,
                    created_at: row.get(5)?,
                    last_used: row.get(6)?,
                })
            },
        );

        match basket {
            Ok(b) => Ok(Some(b)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get basket by ID
    pub fn get_by_id(&self, id: i64) -> Result<Option<Basket>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, token_type, protocol_id, created_at, last_used
             FROM baskets WHERE id = ?1"
        )?;

        let basket = stmt.query_row(
            rusqlite::params![id],
            |row| {
                Ok(Basket {
                    id: Some(row.get(0)?),
                    name: row.get(1)?,
                    description: row.get(2)?,
                    token_type: row.get(3)?,
                    protocol_id: row.get(4)?,
                    created_at: row.get(5)?,
                    last_used: row.get(6)?,
                })
            },
        );

        match basket {
            Ok(b) => Ok(Some(b)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
