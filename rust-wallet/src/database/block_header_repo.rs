//! Repository for block header caching operations
//!
//! Handles CRUD operations for cached block headers used in TSC proof enhancement.

use crate::cache_errors::{CacheError, CacheResult};
use super::models::BlockHeader;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct BlockHeaderRepository<'a> {
    conn: &'a Connection,
}

impl<'a> BlockHeaderRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get cached block header by hash
    pub fn get_by_hash(&self, block_hash: &str) -> CacheResult<Option<BlockHeader>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, block_hash, height, header_hex, cached_at
             FROM block_headers
             WHERE block_hash = ?"
        )?;

        let result = stmt.query_row([block_hash], |row| {
            Ok(BlockHeader {
                id: row.get(0)?,
                block_hash: row.get(1)?,
                height: row.get(2)?,
                header_hex: row.get(3)?,
                cached_at: row.get(4)?,
            })
        });

        match result {
            Ok(header) => Ok(Some(header)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Get cached block header by height
    pub fn get_by_height(&self, height: u32) -> CacheResult<Option<BlockHeader>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, block_hash, height, header_hex, cached_at
             FROM block_headers
             WHERE height = ?"
        )?;

        let result = stmt.query_row([height], |row| {
            Ok(BlockHeader {
                id: row.get(0)?,
                block_hash: row.get(1)?,
                height: row.get(2)?,
                header_hex: row.get(3)?,
                cached_at: row.get(4)?,
            })
        });

        match result {
            Ok(header) => Ok(Some(header)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Cache a block header
    pub fn upsert(&self, block_hash: &str, height: u32, header_hex: &str) -> CacheResult<i64> {
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Use INSERT OR REPLACE to handle duplicates
        self.conn.execute(
            "INSERT OR REPLACE INTO block_headers (block_hash, height, header_hex, cached_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![block_hash, height, header_hex, cached_at],
        )?;

        Ok(self.conn.last_insert_rowid())
    }
}
