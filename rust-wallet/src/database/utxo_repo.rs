//! UTXO repository for database operations
//!
//! Handles CRUD operations for UTXOs in the database.

use rusqlite::{Connection, Result};
use log::{info, warn};
use std::time::{SystemTime, UNIX_EPOCH};
use super::Utxo;

pub struct UtxoRepository<'a> {
    conn: &'a Connection,
}

impl<'a> UtxoRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        UtxoRepository { conn }
    }

    /// Insert or update UTXOs for an address
    /// Returns the number of new UTXOs inserted
    pub fn upsert_utxos(&self, address_id: i64, utxos: &[crate::utxo_fetcher::UTXO]) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut new_count = 0;

        for utxo in utxos {
            // Check if UTXO already exists
            let exists: bool = self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM utxos WHERE txid = ?1 AND vout = ?2)",
                rusqlite::params![utxo.txid, utxo.vout as i32],
                |row| row.get(0),
            )?;

            if exists {
                // Update existing UTXO (update last_updated, ensure not marked as spent if it's still unspent)
                self.conn.execute(
                    "UPDATE utxos
                     SET last_updated = ?1, is_spent = 0, spent_txid = NULL, spent_at = NULL
                     WHERE txid = ?2 AND vout = ?3 AND is_spent = 1",
                    rusqlite::params![now, utxo.txid, utxo.vout as i32],
                )?;
            } else {
                // Insert new UTXO
                self.conn.execute(
                    "INSERT INTO utxos (
                        address_id, txid, vout, satoshis, script,
                        first_seen, last_updated, is_spent
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                    rusqlite::params![
                        address_id,
                        utxo.txid,
                        utxo.vout as i32,
                        utxo.satoshis,
                        utxo.script,
                        now,
                        now,
                    ],
                )?;
                new_count += 1;
            }
        }

        if new_count > 0 {
            info!("   ✅ Inserted {} new UTXOs for address_id {}", new_count, address_id);
        }

        Ok(new_count)
    }

    /// Get all unspent UTXOs for a list of addresses
    pub fn get_unspent_by_addresses(&self, address_ids: &[i64]) -> Result<Vec<Utxo>> {
        if address_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Build query with placeholders
        let placeholders: Vec<String> = (0..address_ids.len())
            .map(|_| "?".to_string())
            .collect();
        let query = format!(
            "SELECT id, address_id, basket_id, txid, vout, satoshis, script,
                    first_seen, last_updated, is_spent, spent_txid, spent_at, custom_instructions
             FROM utxos
             WHERE address_id IN ({}) AND is_spent = 0
             ORDER BY satoshis DESC",
            placeholders.join(",")
        );

        let mut stmt = self.conn.prepare(&query)?;
        let utxos = stmt.query_map(
            rusqlite::params_from_iter(address_ids.iter()),
            |row| {
                Ok(Utxo {
                    id: Some(row.get(0)?),
                    address_id: row.get(1)?,
                    basket_id: row.get(2)?,
                    txid: row.get(3)?,
                    vout: row.get(4)?,
                    satoshis: row.get(5)?,
                    script: row.get(6)?,
                    first_seen: row.get(7)?,
                    last_updated: row.get(8)?,
                    is_spent: row.get::<_, i32>(9)? != 0,
                    spent_txid: row.get(10)?,
                    spent_at: row.get(11)?,
                    custom_instructions: row.get(12).ok(),  // May not exist in older schemas
                })
            },
        )?
        .collect::<Result<Vec<_>>>()?;

        Ok(utxos)
    }

    /// Get unspent UTXOs for a single address
    pub fn get_unspent_by_address(&self, address_id: i64) -> Result<Vec<Utxo>> {
        self.get_unspent_by_addresses(&[address_id])
    }

    /// Mark UTXOs as spent
    pub fn mark_spent(&self, txid: &str, vout: u32, spent_txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE utxos
             SET is_spent = 1, spent_txid = ?1, spent_at = ?2
             WHERE txid = ?3 AND vout = ?4 AND is_spent = 0",
            rusqlite::params![spent_txid, now, txid, vout as i32],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Marked UTXO {}:{} as spent (spent in {})", txid, vout, spent_txid);
        }

        Ok(rows_affected)
    }

    /// Mark multiple UTXOs as spent (for a transaction with multiple inputs)
    pub fn mark_multiple_spent(&self, utxos: &[(String, u32)], spent_txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut total_affected = 0;

        for (txid, vout) in utxos {
            let affected = self.conn.execute(
                "UPDATE utxos
                 SET is_spent = 1, spent_txid = ?1, spent_at = ?2
                 WHERE txid = ?3 AND vout = ?4 AND is_spent = 0",
                rusqlite::params![spent_txid, now, txid, *vout as i32],
            )?;
            total_affected += affected;
        }

        if total_affected > 0 {
            info!("   ✅ Marked {} UTXOs as spent (spent in {})", total_affected, spent_txid);
        }

        Ok(total_affected)
    }

    /// Get all unspent UTXOs for a specific basket
    pub fn get_unspent_by_basket(&self, basket_id: i64) -> Result<Vec<Utxo>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, address_id, basket_id, txid, vout, satoshis, script,
                    first_seen, last_updated, is_spent, spent_txid, spent_at, custom_instructions
             FROM utxos
             WHERE basket_id = ?1 AND is_spent = 0
             ORDER BY satoshis DESC"
        )?;

        let utxos = stmt.query_map(
            rusqlite::params![basket_id],
            |row| {
                Ok(Utxo {
                    id: Some(row.get(0)?),
                    address_id: row.get(1)?,
                    basket_id: row.get(2)?,
                    txid: row.get(3)?,
                    vout: row.get(4)?,
                    satoshis: row.get(5)?,
                    script: row.get(6)?,
                    first_seen: row.get(7)?,
                    last_updated: row.get(8)?,
                    is_spent: row.get::<_, i32>(9)? != 0,
                    spent_txid: row.get(10)?,
                    spent_at: row.get(11)?,
                    custom_instructions: row.get(12).ok(),  // May not exist in older schemas
                })
            },
        )?
        .collect::<Result<Vec<_>>>()?;

        Ok(utxos)
    }

    /// Get UTXO by txid and vout
    pub fn get_by_txid_vout(&self, txid: &str, vout: u32) -> Result<Option<Utxo>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, address_id, basket_id, txid, vout, satoshis, script,
                    first_seen, last_updated, is_spent, spent_txid, spent_at, custom_instructions
             FROM utxos
             WHERE txid = ?1 AND vout = ?2"
        )?;

        match stmt.query_row(
            rusqlite::params![txid, vout as i32],
            |row| {
                Ok(Utxo {
                    id: Some(row.get(0)?),
                    address_id: row.get(1)?,
                    basket_id: row.get(2)?,
                    txid: row.get(3)?,
                    vout: row.get(4)?,
                    satoshis: row.get(5)?,
                    script: row.get(6)?,
                    first_seen: row.get(7)?,
                    last_updated: row.get(8)?,
                    is_spent: row.get::<_, i32>(9)? != 0,
                    spent_txid: row.get(10)?,
                    spent_at: row.get(11)?,
                    custom_instructions: row.get(12).ok(),  // May not exist in older schemas
                })
            },
        ) {
            Ok(utxo) => Ok(Some(utxo)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Calculate total balance from unspent UTXOs for given addresses
    pub fn calculate_balance(&self, address_ids: &[i64]) -> Result<i64> {
        if address_ids.is_empty() {
            return Ok(0);
        }

        let placeholders: Vec<String> = (0..address_ids.len())
            .map(|_| "?".to_string())
            .collect();
        let query = format!(
            "SELECT COALESCE(SUM(satoshis), 0)
             FROM utxos
             WHERE address_id IN ({}) AND is_spent = 0",
            placeholders.join(",")
        );

        let balance: i64 = self.conn.query_row(
            &query,
            rusqlite::params_from_iter(address_ids.iter()),
            |row| row.get(0),
        )?;

        Ok(balance)
    }

    /// Delete spent UTXOs older than specified days (cleanup)
    pub fn cleanup_old_spent(&self, days: i64) -> Result<usize> {
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 - (days * 24 * 60 * 60);

        let rows_affected = self.conn.execute(
            "DELETE FROM utxos WHERE is_spent = 1 AND spent_at < ?1",
            rusqlite::params![cutoff],
        )?;

        if rows_affected > 0 {
            info!("   🧹 Cleaned up {} old spent UTXOs", rows_affected);
        }

        Ok(rows_affected)
    }

    /// Remove UTXO from basket (set basket_id to NULL)
    pub fn remove_from_basket(&self, utxo_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE utxos SET basket_id = NULL WHERE id = ?1",
            rusqlite::params![utxo_id],
        )?;
        Ok(())
    }
}
