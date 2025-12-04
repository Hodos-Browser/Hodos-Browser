//! Repository for parent transaction caching operations
//!
//! Handles CRUD operations for cached parent transactions used in BEEF building.

use crate::cache_errors::{CacheError, CacheResult};
use super::models::ParentTransaction;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ParentTransactionRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ParentTransactionRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get cached parent transaction by TXID
    pub fn get_by_txid(&self, txid: &str) -> CacheResult<Option<ParentTransaction>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, utxo_id, txid, raw_hex, cached_at
             FROM parent_transactions
             WHERE txid = ?"
        )?;

        let result = stmt.query_row([txid], |row| {
            Ok(ParentTransaction {
                id: row.get(0)?,
                utxo_id: row.get(1)?,
                txid: row.get(2)?,
                raw_hex: row.get(3)?,
                cached_at: row.get(4)?,
            })
        });

        match result {
            Ok(tx) => Ok(Some(tx)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Cache a parent transaction (utxo_id can be None for external transactions)
    pub fn upsert(&self, utxo_id: Option<i64>, txid: &str, raw_hex: &str) -> CacheResult<i64> {
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Use INSERT OR REPLACE to handle duplicates
        self.conn.execute(
            "INSERT OR REPLACE INTO parent_transactions (utxo_id, txid, raw_hex, cached_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![utxo_id, txid, raw_hex, cached_at],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Get parent transaction ID by TXID (for linking merkle proofs)
    pub fn get_id_by_txid(&self, txid: &str) -> CacheResult<Option<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM parent_transactions WHERE txid = ?"
        )?;

        match stmt.query_row([txid], |row| row.get::<_, i64>(0)) {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Verify cached transaction TXID matches expected
    pub fn verify_txid(&self, txid: &str, raw_hex: &str) -> CacheResult<bool> {
        use sha2::{Sha256, Digest};
        let tx_bytes = hex::decode(raw_hex)?;
        let hash1 = Sha256::digest(&tx_bytes);
        let hash2 = Sha256::digest(&hash1);
        let calculated_txid: Vec<u8> = hash2.into_iter().rev().collect();
        let calculated_txid_hex = hex::encode(calculated_txid);
        Ok(calculated_txid_hex == txid)
    }
}
