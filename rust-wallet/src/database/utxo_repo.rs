//! UTXO repository for database operations
//!
//! Handles CRUD operations for UTXOs in the database.

use rusqlite::{Connection, Result};
use log::{info, warn};
use std::collections::HashSet;
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
                // Update existing UTXO - only update last_updated timestamp
                // IMPORTANT: Do NOT change is_spent status here!
                // UTXOs marked as spent should only be un-spent if the spending transaction
                // was rejected by the network (handled separately in failed tx cleanup)
                self.conn.execute(
                    "UPDATE utxos SET last_updated = ?1 WHERE txid = ?2 AND vout = ?3",
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
    ///
    /// Excludes UTXOs from transactions that were never broadcast ('unsigned')
    /// or that failed to broadcast ('failed'). This prevents spending outputs
    /// that don't exist on-chain, which would cause ARC to reject the BEEF.
    pub fn get_unspent_by_addresses(&self, address_ids: &[i64]) -> Result<Vec<Utxo>> {
        if address_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Check if new_status column exists (migration v15)
        let has_new_status: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'new_status'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        // Build query with placeholders
        let placeholders: Vec<String> = (0..address_ids.len())
            .map(|_| "?".to_string())
            .collect();

        let query = if has_new_status {
            // LEFT JOIN with transactions to filter out UTXOs from never-broadcast
            // or failed transactions. The LEFT JOIN means:
            // - UTXOs from external incoming transactions (no match) → t.new_status IS NULL → included
            // - UTXOs from sending/unproven/completed transactions → included
            // - UTXOs from unsigned/failed transactions → excluded
            format!(
                "SELECT u.id, u.address_id, u.basket_id, u.txid, u.vout, u.satoshis, u.script,
                        u.first_seen, u.last_updated, u.is_spent, u.spent_txid, u.spent_at, u.custom_instructions, u.output_description
                 FROM utxos u
                 LEFT JOIN transactions t ON u.txid = t.txid
                 WHERE u.address_id IN ({}) AND u.is_spent = 0
                   AND (t.new_status IS NULL OR t.new_status NOT IN ('unsigned', 'failed'))
                 ORDER BY u.satoshis DESC",
                placeholders.join(",")
            )
        } else {
            // Pre-v15 fallback: check broadcast_status
            let has_broadcast_status: bool = self.conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'broadcast_status'",
                [],
                |row| Ok(row.get::<_, i64>(0)? > 0),
            ).unwrap_or(false);

            if has_broadcast_status {
                format!(
                    "SELECT u.id, u.address_id, u.basket_id, u.txid, u.vout, u.satoshis, u.script,
                            u.first_seen, u.last_updated, u.is_spent, u.spent_txid, u.spent_at, u.custom_instructions, u.output_description
                     FROM utxos u
                     LEFT JOIN transactions t ON u.txid = t.txid
                     WHERE u.address_id IN ({}) AND u.is_spent = 0
                       AND (t.broadcast_status IS NULL OR t.broadcast_status NOT IN ('pending', 'failed'))
                     ORDER BY u.satoshis DESC",
                    placeholders.join(",")
                )
            } else {
                format!(
                    "SELECT id, address_id, basket_id, txid, vout, satoshis, script,
                            first_seen, last_updated, is_spent, spent_txid, spent_at, custom_instructions, output_description
                     FROM utxos
                     WHERE address_id IN ({}) AND is_spent = 0
                     ORDER BY satoshis DESC",
                    placeholders.join(",")
                )
            }
        };

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
                    output_description: row.get(13).ok(),   // May not exist pre-v14
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
                    first_seen, last_updated, is_spent, spent_txid, spent_at, custom_instructions, output_description
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
                    output_description: row.get(13).ok(),   // May not exist pre-v14
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
                    first_seen, last_updated, is_spent, spent_txid, spent_at, custom_instructions, output_description
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
                    output_description: row.get(13).ok(),   // May not exist pre-v14
                })
            },
        ) {
            Ok(utxo) => Ok(Some(utxo)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Calculate total balance from unspent UTXOs for given addresses
    ///
    /// Excludes UTXOs from transactions that were never broadcast ('unsigned')
    /// or that failed to broadcast ('failed'), matching the same filter used
    /// in get_unspent_by_addresses() for consistency.
    pub fn calculate_balance(&self, address_ids: &[i64]) -> Result<i64> {
        if address_ids.is_empty() {
            return Ok(0);
        }

        // Check if new_status column exists (migration v15)
        let has_new_status: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'new_status'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        let placeholders: Vec<String> = (0..address_ids.len())
            .map(|_| "?".to_string())
            .collect();

        let query = if has_new_status {
            format!(
                "SELECT COALESCE(SUM(u.satoshis), 0)
                 FROM utxos u
                 LEFT JOIN transactions t ON u.txid = t.txid
                 WHERE u.address_id IN ({}) AND u.is_spent = 0
                   AND (t.new_status IS NULL OR t.new_status NOT IN ('unsigned', 'failed'))",
                placeholders.join(",")
            )
        } else {
            // Pre-v15 fallback
            let has_broadcast_status: bool = self.conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'broadcast_status'",
                [],
                |row| Ok(row.get::<_, i64>(0)? > 0),
            ).unwrap_or(false);

            if has_broadcast_status {
                format!(
                    "SELECT COALESCE(SUM(u.satoshis), 0)
                     FROM utxos u
                     LEFT JOIN transactions t ON u.txid = t.txid
                     WHERE u.address_id IN ({}) AND u.is_spent = 0
                       AND (t.broadcast_status IS NULL OR t.broadcast_status NOT IN ('pending', 'failed'))",
                    placeholders.join(",")
                )
            } else {
                format!(
                    "SELECT COALESCE(SUM(satoshis), 0)
                     FROM utxos
                     WHERE address_id IN ({}) AND is_spent = 0",
                    placeholders.join(",")
                )
            }
        };

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

    /// Insert a new UTXO with optional basket_id and status
    ///
    /// This method is used for BRC-100 outputs that need basket/tag tracking.
    /// Unlike `upsert_utxos`, this always creates a new record (no upsert).
    ///
    /// # Arguments
    /// * `address_id` - The address ID this UTXO belongs to
    /// * `txid` - Transaction ID
    /// * `vout` - Output index
    /// * `satoshis` - Amount in satoshis
    /// * `script_hex` - Hex-encoded locking script
    /// * `basket_id` - Optional basket ID for BRC-100 tracking
    /// * `custom_instructions` - Optional custom instructions (BRC-78)
    /// * `status` - UTXO status ('unproven', 'completed', 'failed')
    /// * `output_description` - Optional output description (BRC-100, 5-50 bytes UTF-8)
    ///
    /// # Returns
    /// The ID of the newly created UTXO
    pub fn insert_output_with_basket(
        &self,
        address_id: Option<i64>,
        txid: &str,
        vout: u32,
        satoshis: i64,
        script_hex: &str,
        basket_id: Option<i64>,
        custom_instructions: Option<&str>,
        status: &str,
        output_description: Option<&str>,
    ) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Check if status column exists (handles case where v9 migration hasn't run yet)
        let status_column_exists: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('utxos') WHERE name = 'status'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        if status_column_exists {
            // Full insert with status and output_description columns
            self.conn.execute(
                "INSERT INTO utxos (
                    address_id, basket_id, txid, vout, satoshis, script,
                    first_seen, last_updated, is_spent, custom_instructions, status, output_description
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10, ?11)",
                rusqlite::params![
                    address_id,
                    basket_id,
                    txid,
                    vout as i32,
                    satoshis,
                    script_hex,
                    now,
                    now,
                    custom_instructions.unwrap_or(""),
                    status,
                    output_description,
                ],
            )?;
            info!("   ✅ Inserted UTXO {}:{} with basket_id={:?}, status='{}'",
                  txid, vout, basket_id, status);
        } else {
            // Fallback insert without status column (pre-v9 migration)
            warn!("   ⚠️  status column not found - inserting without status (run migration v9)");
            self.conn.execute(
                "INSERT INTO utxos (
                    address_id, basket_id, txid, vout, satoshis, script,
                    first_seen, last_updated, is_spent, custom_instructions
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)",
                rusqlite::params![
                    address_id,
                    basket_id,
                    txid,
                    vout as i32,
                    satoshis,
                    script_hex,
                    now,
                    now,
                    custom_instructions.unwrap_or(""),
                ],
            )?;
            info!("   ✅ Inserted UTXO {}:{} with basket_id={:?} (no status column)",
                  txid, vout, basket_id);
        }

        let id = self.conn.last_insert_rowid();
        Ok(id)
    }

    /// Update UTXO status
    ///
    /// Used for optimistic UTXO creation flow:
    /// - Create with status='unproven' after signing
    /// - Update to 'completed' after broadcast confirmation
    /// - Update to 'failed' if broadcast fails
    pub fn update_status(&self, txid: &str, vout: u32, status: &str) -> Result<()> {
        // Check if status column exists (handles case where v9 migration hasn't run yet)
        let status_column_exists: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('utxos') WHERE name = 'status'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        if !status_column_exists {
            warn!("   ⚠️  status column not found - skipping status update (run migration v9)");
            return Ok(());
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE utxos SET status = ?1, last_updated = ?2 WHERE txid = ?3 AND vout = ?4",
            rusqlite::params![status, now, txid, vout as i32],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Updated UTXO {}:{} status to '{}'", txid, vout, status);
        } else {
            warn!("   ⚠️  No UTXO found for {}:{} to update status", txid, vout);
        }

        Ok(())
    }

    /// Assign a basket to an existing UTXO
    pub fn assign_basket(&self, utxo_id: i64, basket_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE utxos SET basket_id = ?1 WHERE id = ?2",
            rusqlite::params![basket_id, utxo_id],
        )?;
        info!("   ✅ Assigned basket_id={} to UTXO id={}", basket_id, utxo_id);
        Ok(())
    }

    /// Get unspent UTXOs by basket with tag filtering (SQL-based for efficiency)
    ///
    /// This is more efficient than the N+1 approach of querying tags per UTXO.
    /// Uses SQL JOINs to filter in a single query.
    ///
    /// # Arguments
    /// * `basket_id` - The basket ID to filter by
    /// * `tag_ids` - Optional tag IDs to filter by
    /// * `require_all_tags` - If true, UTXO must have ALL tags (AND). If false, ANY tag (OR).
    pub fn get_unspent_by_basket_with_tags(
        &self,
        basket_id: i64,
        tag_ids: Option<&[i64]>,
        require_all_tags: bool,
    ) -> Result<Vec<Utxo>> {
        // If no tags provided, use the simple basket query
        let tag_ids = match tag_ids {
            Some(ids) if !ids.is_empty() => ids,
            _ => return self.get_unspent_by_basket(basket_id),
        };

        let query = if require_all_tags {
            // ALL mode: UTXO must have every requested tag
            // Use GROUP BY and HAVING COUNT = number of tags
            let tag_count = tag_ids.len();
            let placeholders: String = (0..tag_ids.len())
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");

            format!(
                "SELECT u.id, u.address_id, u.basket_id, u.txid, u.vout, u.satoshis, u.script,
                        u.first_seen, u.last_updated, u.is_spent, u.spent_txid, u.spent_at, u.custom_instructions, u.output_description
                 FROM utxos u
                 INNER JOIN output_tag_map otm ON u.id = otm.output_id AND otm.is_deleted = 0
                 WHERE u.basket_id = ?1 AND u.is_spent = 0 AND otm.output_tag_id IN ({})
                 GROUP BY u.id
                 HAVING COUNT(DISTINCT otm.output_tag_id) = {}
                 ORDER BY u.satoshis DESC",
                placeholders, tag_count
            )
        } else {
            // ANY mode: UTXO must have at least one of the requested tags
            let placeholders: String = (0..tag_ids.len())
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");

            format!(
                "SELECT DISTINCT u.id, u.address_id, u.basket_id, u.txid, u.vout, u.satoshis, u.script,
                        u.first_seen, u.last_updated, u.is_spent, u.spent_txid, u.spent_at, u.custom_instructions, u.output_description
                 FROM utxos u
                 INNER JOIN output_tag_map otm ON u.id = otm.output_id AND otm.is_deleted = 0
                 WHERE u.basket_id = ?1 AND u.is_spent = 0 AND otm.output_tag_id IN ({})
                 ORDER BY u.satoshis DESC",
                placeholders
            )
        };

        let mut stmt = self.conn.prepare(&query)?;

        // Build params: basket_id followed by tag_ids
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        params.push(Box::new(basket_id));
        for tag_id in tag_ids {
            params.push(Box::new(*tag_id));
        }

        let utxos = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
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
                    is_spent: row.get(9)?,
                    spent_txid: row.get(10)?,
                    spent_at: row.get(11)?,
                    custom_instructions: row.get(12)?,
                    output_description: row.get(13).ok(),  // May not exist pre-v14
                })
            },
        )?;

        let mut result = Vec::new();
        for utxo in utxos {
            result.push(utxo?);
        }

        info!("   Found {} UTXOs in basket {} with tag filter (require_all={})",
              result.len(), basket_id, require_all_tags);
        Ok(result)
    }

    /// Update UTXO txid after signing
    ///
    /// When a transaction is signed, the txid changes (from unsigned to signed).
    /// This method updates any UTXOs that were created with the unsigned txid
    /// to use the new signed txid.
    ///
    /// This is critical for transaction chaining - without this update,
    /// a child transaction trying to spend the change output won't be able
    /// to find the parent transaction in the local database.
    pub fn update_txid(&self, old_txid: &str, new_txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE utxos SET txid = ?1, last_updated = ?2 WHERE txid = ?3",
            rusqlite::params![new_txid, now, old_txid],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Updated {} UTXO(s) txid: {} → {}", rows_affected, old_txid, new_txid);
        }

        Ok(rows_affected)
    }

    /// Restore UTXOs that were marked as spent by a specific transaction.
    ///
    /// Used during stale pending transaction cleanup: when a transaction was created
    /// but never broadcast, the inputs it consumed need to be restored to unspent.
    ///
    /// # Arguments
    /// * `spent_txid` - The txid of the never-broadcast transaction that consumed these UTXOs
    ///
    /// # Returns
    /// The number of UTXOs restored
    pub fn restore_spent_by_txid(&self, spent_txid: &str) -> Result<usize> {
        let rows_affected = self.conn.execute(
            "UPDATE utxos SET is_spent = 0, spent_txid = NULL, spent_at = NULL
             WHERE spent_txid = ?1 AND is_spent = 1",
            rusqlite::params![spent_txid],
        )?;

        if rows_affected > 0 {
            info!("   ♻️  Restored {} UTXO(s) that were spent by {}", rows_affected, &spent_txid[..std::cmp::min(16, spent_txid.len())]);
        }

        Ok(rows_affected)
    }

    /// Delete all UTXOs with the given txid
    ///
    /// Used for cleaning up ghost UTXOs from failed broadcasts.
    /// When a transaction fails to broadcast, any change UTXOs created
    /// for it need to be removed to prevent balance inflation.
    ///
    /// # Arguments
    /// * `txid` - The transaction ID whose UTXOs should be deleted
    ///
    /// # Returns
    /// The number of UTXOs deleted
    pub fn delete_by_txid(&self, txid: &str) -> Result<usize> {
        let rows_affected = self.conn.execute(
            "DELETE FROM utxos WHERE txid = ?1",
            rusqlite::params![txid],
        )?;

        if rows_affected > 0 {
            info!("   🗑️  Deleted {} UTXO(s) with txid {}", rows_affected, &txid[..std::cmp::min(16, txid.len())]);
        }

        Ok(rows_affected)
    }

    /// Restore all UTXOs that were reserved with a `pending-*` placeholder spent_txid.
    ///
    /// This handles the case where the wallet process crashed or the handler hung
    /// between UTXO reservation and broadcast. The placeholder format `pending-{timestamp}`
    /// indicates the UTXO was reserved but the transaction was never completed.
    ///
    /// # Returns
    /// The number of UTXOs restored
    pub fn restore_pending_placeholders(&self) -> Result<usize> {
        let rows_affected = self.conn.execute(
            "UPDATE utxos SET is_spent = 0, spent_txid = NULL, spent_at = NULL
             WHERE is_spent = 1 AND spent_txid LIKE 'pending-%'",
            [],
        )?;

        if rows_affected > 0 {
            info!("   ♻️  Restored {} UTXO(s) with stale placeholder reservations", rows_affected);
        }

        Ok(rows_affected)
    }

    /// Update the spent_txid for all UTXOs reserved with a given placeholder.
    ///
    /// After signing, the real txid is known. This updates the placeholder to the
    /// actual spending transaction ID so that cleanup/rollback can find them correctly.
    ///
    /// # Arguments
    /// * `placeholder_txid` - The `pending-{timestamp}` placeholder used during reservation
    /// * `real_txid` - The actual signed transaction ID
    ///
    /// # Returns
    /// The number of UTXOs updated
    pub fn update_spent_txid_batch(&self, placeholder_txid: &str, real_txid: &str) -> Result<usize> {
        let rows_affected = self.conn.execute(
            "UPDATE utxos SET spent_txid = ?1
             WHERE spent_txid = ?2 AND is_spent = 1",
            rusqlite::params![real_txid, placeholder_txid],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Updated spent_txid on {} UTXO(s): {} → {}",
                rows_affected,
                &placeholder_txid[..std::cmp::min(20, placeholder_txid.len())],
                &real_txid[..std::cmp::min(16, real_txid.len())]);
        }

        Ok(rows_affected)
    }

    /// Reconcile UTXOs for an address against the blockchain API response.
    ///
    /// Marks unspent UTXOs in the database as externally spent if they are
    /// NOT present in the API response (meaning they were spent on-chain
    /// by a transaction the wallet doesn't know about).
    ///
    /// Only reconciles UTXOs older than `grace_period_secs` to avoid marking
    /// recently-created wallet UTXOs that haven't propagated to the API yet.
    ///
    /// # Arguments
    /// * `address_id` - The database ID of the address to reconcile
    /// * `api_utxos` - The UTXOs returned by WhatsOnChain for this address
    /// * `grace_period_secs` - Don't reconcile UTXOs newer than this many seconds
    ///
    /// # Returns
    /// The number of stale UTXOs marked as externally spent
    pub fn reconcile_for_address(
        &self,
        address_id: i64,
        api_utxos: &[crate::utxo_fetcher::UTXO],
        grace_period_secs: i64,
    ) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let grace_cutoff = now - grace_period_secs;

        // Build set of (txid, vout) from API response
        let api_set: HashSet<(String, i32)> = api_utxos.iter()
            .map(|u| (u.txid.clone(), u.vout as i32))
            .collect();

        // Get all unspent UTXOs for this address from DB (older than grace period)
        let mut stmt = self.conn.prepare(
            "SELECT txid, vout FROM utxos WHERE address_id = ?1 AND is_spent = 0 AND first_seen < ?2"
        )?;
        let db_utxos: Vec<(String, i32)> = stmt.query_map(
            rusqlite::params![address_id, grace_cutoff],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
        )?.filter_map(|r| r.ok()).collect();

        let mut stale_count = 0;
        for (txid, vout) in &db_utxos {
            if !api_set.contains(&(txid.clone(), *vout)) {
                self.conn.execute(
                    "UPDATE utxos SET is_spent = 1, spent_txid = 'external-spend', spent_at = ?1
                     WHERE txid = ?2 AND vout = ?3 AND is_spent = 0",
                    rusqlite::params![now, txid, vout],
                )?;
                stale_count += 1;
                info!("   🔄 Marked stale UTXO {}:{} as externally spent (not in blockchain API)", txid, vout);
            }
        }

        Ok(stale_count)
    }

    /// Update the txid of a specific UTXO (e.g., after signing changes the txid).
    ///
    /// # Arguments
    /// * `old_txid` - The pre-signing transaction ID
    /// * `vout` - The output index
    /// * `new_txid` - The post-signing transaction ID
    pub fn update_utxo_txid(&self, old_txid: &str, vout: u32, new_txid: &str) -> Result<usize> {
        let rows_affected = self.conn.execute(
            "UPDATE utxos SET txid = ?1 WHERE txid = ?2 AND vout = ?3",
            rusqlite::params![new_txid, old_txid, vout as i32],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Updated UTXO txid: {}:{} → {}:{}",
                &old_txid[..std::cmp::min(16, old_txid.len())], vout,
                &new_txid[..std::cmp::min(16, new_txid.len())], vout);
        }

        Ok(rows_affected)
    }
}
