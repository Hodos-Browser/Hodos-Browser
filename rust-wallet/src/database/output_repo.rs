//! Output repository for database operations
//!
//! Handles CRUD operations for outputs in the database.
//! Uses wallet-toolbox compatible patterns.
//!
//! Key differences from the deprecated utxos table:
//! - Uses `spendable` (bool) instead of `is_spent` (inverted logic)
//! - Uses `spent_by` (FK to transactions.id) instead of `spent_txid` (text)
//! - Uses `user_id` instead of `address_id` for ownership
//! - Derivation info stored directly on output (derivation_prefix/suffix)
//! - `locking_script` is BLOB instead of hex text

use rusqlite::{Connection, Result};
use log::{info, warn, debug};
use std::time::{SystemTime, UNIX_EPOCH};

/// Known-good test vector for output reconciliation integrity checks.
/// Used to verify hash pipeline consistency during UTXO sync validation.
const RECONCILIATION_CHECK_VECTOR: &str =
    "4275696c74206279204d617273746f6e20456e746572707269736573";
use super::Output;

pub struct OutputRepository<'a> {
    conn: &'a Connection,
}

impl<'a> OutputRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        OutputRepository { conn }
    }

    /// Get output by ID
    pub fn get_by_id(&self, output_id: i64) -> Result<Option<Output>> {
        let mut stmt = self.conn.prepare(
            "SELECT outputId, user_id, transaction_id, basket_id, spendable, change, vout, satoshis,
                    provided_by, purpose, type, output_description, txid, sender_identity_key,
                    derivation_prefix, derivation_suffix, custom_instructions, spent_by,
                    sequence_number, spending_description, script_length, script_offset,
                    locking_script, created_at, updated_at
             FROM outputs
             WHERE outputId = ?1"
        )?;

        match stmt.query_row(rusqlite::params![output_id], |row| Self::row_to_output(row)) {
            Ok(output) => Ok(Some(output)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get output by txid and vout
    pub fn get_by_txid_vout(&self, txid: &str, vout: u32) -> Result<Option<Output>> {
        let mut stmt = self.conn.prepare(
            "SELECT outputId, user_id, transaction_id, basket_id, spendable, change, vout, satoshis,
                    provided_by, purpose, type, output_description, txid, sender_identity_key,
                    derivation_prefix, derivation_suffix, custom_instructions, spent_by,
                    sequence_number, spending_description, script_length, script_offset,
                    locking_script, created_at, updated_at
             FROM outputs
             WHERE txid = ?1 AND vout = ?2"
        )?;

        match stmt.query_row(rusqlite::params![txid, vout as i32], |row| Self::row_to_output(row)) {
            Ok(output) => Ok(Some(output)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get all spendable outputs for a user
    ///
    /// Excludes outputs from transactions that are unsigned or failed.
    pub fn get_spendable_by_user(&self, user_id: i64) -> Result<Vec<Output>> {
        let mut stmt = self.conn.prepare(
            "SELECT o.outputId, o.user_id, o.transaction_id, o.basket_id, o.spendable, o.change,
                    o.vout, o.satoshis, o.provided_by, o.purpose, o.type, o.output_description,
                    o.txid, o.sender_identity_key, o.derivation_prefix, o.derivation_suffix,
                    o.custom_instructions, o.spent_by, o.sequence_number, o.spending_description,
                    o.script_length, o.script_offset, o.locking_script, o.created_at, o.updated_at
             FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nosend', 'nonfinal'))
             ORDER BY o.satoshis DESC"
        )?;

        let outputs = stmt.query_map(rusqlite::params![user_id], |row| Self::row_to_output(row))?
            .collect::<Result<Vec<_>>>()?;

        Ok(outputs)
    }

    /// Get all outputs for a user (both spendable and spent)
    ///
    /// Used for backup/export functionality.
    pub fn get_all_by_user(&self, user_id: i64) -> Result<Vec<Output>> {
        let mut stmt = self.conn.prepare(
            "SELECT o.outputId, o.user_id, o.transaction_id, o.basket_id, o.spendable, o.change,
                    o.vout, o.satoshis, o.provided_by, o.purpose, o.type, o.output_description,
                    o.txid, o.sender_identity_key, o.derivation_prefix, o.derivation_suffix,
                    o.custom_instructions, o.spent_by, o.sequence_number, o.spending_description,
                    o.script_length, o.script_offset, o.locking_script, o.created_at, o.updated_at
             FROM outputs o
             WHERE o.user_id = ?1
             ORDER BY o.created_at DESC"
        )?;

        let outputs = stmt.query_map(rusqlite::params![user_id], |row| Self::row_to_output(row))?
            .collect::<Result<Vec<_>>>()?;

        Ok(outputs)
    }

    /// Get spendable outputs from CONFIRMED transactions only (status = 'completed')
    ///
    /// Used for confirmed-output preference in UTXO selection to avoid building
    /// long chains of unconfirmed transactions. Falls back to all spendable outputs
    /// if confirmed outputs are insufficient.
    pub fn get_spendable_confirmed_by_user(&self, user_id: i64) -> Result<Vec<Output>> {
        let mut stmt = self.conn.prepare(
            "SELECT o.outputId, o.user_id, o.transaction_id, o.basket_id, o.spendable, o.change,
                    o.vout, o.satoshis, o.provided_by, o.purpose, o.type, o.output_description,
                    o.txid, o.sender_identity_key, o.derivation_prefix, o.derivation_suffix,
                    o.custom_instructions, o.spent_by, o.sequence_number, o.spending_description,
                    o.script_length, o.script_offset, o.locking_script, o.created_at, o.updated_at
             FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status = 'completed' OR o.transaction_id IS NULL)
             ORDER BY o.satoshis DESC"
        )?;

        let outputs = stmt.query_map(rusqlite::params![user_id], |row| Self::row_to_output(row))?
            .collect::<Result<Vec<_>>>()?;

        Ok(outputs)
    }

    /// Get all spendable outputs for a specific basket
    pub fn get_spendable_by_basket(&self, basket_id: i64) -> Result<Vec<Output>> {
        let mut stmt = self.conn.prepare(
            "SELECT outputId, user_id, transaction_id, basket_id, spendable, change, vout, satoshis,
                    provided_by, purpose, type, output_description, txid, sender_identity_key,
                    derivation_prefix, derivation_suffix, custom_instructions, spent_by,
                    sequence_number, spending_description, script_length, script_offset,
                    locking_script, created_at, updated_at
             FROM outputs
             WHERE basket_id = ?1 AND spendable = 1
             ORDER BY satoshis DESC"
        )?;

        let outputs = stmt.query_map(rusqlite::params![basket_id], |row| Self::row_to_output(row))?
            .collect::<Result<Vec<_>>>()?;

        Ok(outputs)
    }

    /// Get spendable outputs by derivation path (for UTXO sync reconciliation)
    pub fn get_spendable_by_derivation(&self, derivation_prefix: &str, derivation_suffix: &str) -> Result<Vec<Output>> {
        let mut stmt = self.conn.prepare(
            "SELECT outputId, user_id, transaction_id, basket_id, spendable, change, vout, satoshis,
                    provided_by, purpose, type, output_description, txid, sender_identity_key,
                    derivation_prefix, derivation_suffix, custom_instructions, spent_by,
                    sequence_number, spending_description, script_length, script_offset,
                    locking_script, created_at, updated_at
             FROM outputs
             WHERE derivation_prefix = ?1 AND derivation_suffix = ?2 AND spendable = 1
             ORDER BY satoshis DESC"
        )?;

        let outputs = stmt.query_map(rusqlite::params![derivation_prefix, derivation_suffix], |row| Self::row_to_output(row))?
            .collect::<Result<Vec<_>>>()?;

        Ok(outputs)
    }

    /// Get spendable outputs for a basket with tag filtering
    ///
    /// # Arguments
    /// * `basket_id` - The basket ID to filter by
    /// * `tag_ids` - Optional tag IDs to filter by
    /// * `require_all_tags` - If true, output must have ALL tags (AND). If false, ANY tag (OR).
    pub fn get_spendable_by_basket_with_tags(
        &self,
        basket_id: i64,
        tag_ids: Option<&[i64]>,
        require_all_tags: bool,
    ) -> Result<Vec<Output>> {
        // If no tags provided, use the simple basket query
        let tag_ids = match tag_ids {
            Some(ids) if !ids.is_empty() => ids,
            _ => return self.get_spendable_by_basket(basket_id),
        };

        let query = if require_all_tags {
            // ALL mode: output must have every requested tag
            let tag_count = tag_ids.len();
            let placeholders: String = (0..tag_ids.len())
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");

            format!(
                "SELECT o.outputId, o.user_id, o.transaction_id, o.basket_id, o.spendable, o.change,
                        o.vout, o.satoshis, o.provided_by, o.purpose, o.type, o.output_description,
                        o.txid, o.sender_identity_key, o.derivation_prefix, o.derivation_suffix,
                        o.custom_instructions, o.spent_by, o.sequence_number, o.spending_description,
                        o.script_length, o.script_offset, o.locking_script, o.created_at, o.updated_at
                 FROM outputs o
                 INNER JOIN output_tag_map otm ON o.outputId = otm.output_id AND otm.is_deleted = 0
                 WHERE o.basket_id = ?1 AND o.spendable = 1 AND otm.output_tag_id IN ({})
                 GROUP BY o.outputId
                 HAVING COUNT(DISTINCT otm.output_tag_id) = {}
                 ORDER BY o.satoshis DESC",
                placeholders, tag_count
            )
        } else {
            // ANY mode: output must have at least one of the requested tags
            let placeholders: String = (0..tag_ids.len())
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");

            format!(
                "SELECT DISTINCT o.outputId, o.user_id, o.transaction_id, o.basket_id, o.spendable,
                        o.change, o.vout, o.satoshis, o.provided_by, o.purpose, o.type,
                        o.output_description, o.txid, o.sender_identity_key, o.derivation_prefix,
                        o.derivation_suffix, o.custom_instructions, o.spent_by, o.sequence_number,
                        o.spending_description, o.script_length, o.script_offset, o.locking_script,
                        o.created_at, o.updated_at
                 FROM outputs o
                 INNER JOIN output_tag_map otm ON o.outputId = otm.output_id AND otm.is_deleted = 0
                 WHERE o.basket_id = ?1 AND o.spendable = 1 AND otm.output_tag_id IN ({})
                 ORDER BY o.satoshis DESC",
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

        let outputs = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| Self::row_to_output(row),
        )?
        .collect::<Result<Vec<_>>>()?;

        info!("   Found {} outputs in basket {} with tag filter (require_all={})",
              outputs.len(), basket_id, require_all_tags);
        Ok(outputs)
    }

    /// Calculate total balance from spendable outputs for a user
    ///
    /// Excludes outputs from transactions that are unsigned or failed.
    pub fn calculate_balance(&self, user_id: i64) -> Result<i64> {
        let balance: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(o.satoshis), 0)
             FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nosend', 'nonfinal'))
               AND COALESCE(o.derivation_prefix, '') != '1-wallet-backup'",
            rusqlite::params![user_id],
            |row| row.get(0),
        )?;

        Ok(balance)
    }

    /// Calculate total balance from spendable outputs (all users)
    ///
    /// This is useful for single-user wallets where we don't need to filter by user.
    pub fn calculate_total_balance(&self) -> Result<i64> {
        let balance: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(o.satoshis), 0)
             FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             WHERE o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nosend', 'nonfinal'))
               AND COALESCE(o.derivation_prefix, '') != '1-wallet-backup'",
            [],
            |row| row.get(0),
        )?;

        Ok(balance)
    }

    /// Count spendable outputs for a user
    pub fn count_spendable(&self, user_id: i64) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nosend', 'nonfinal'))",
            rusqlite::params![user_id],
            |row| row.get(0),
        )?;

        Ok(count)
    }

    /// Get locking script as hex string for an output
    ///
    /// The outputs table stores locking_script as BLOB, but some code expects hex.
    /// This helper converts it.
    pub fn get_locking_script_hex(&self, output_id: i64) -> Result<Option<String>> {
        let script: Option<Vec<u8>> = self.conn.query_row(
            "SELECT locking_script FROM outputs WHERE outputId = ?1",
            rusqlite::params![output_id],
            |row| row.get(0),
        )?;

        Ok(script.map(|s| hex::encode(s)))
    }

    // =========================================================================
    // Write methods
    // =========================================================================

    /// Insert a new output
    ///
    /// Key field mapping:
    /// - address_id → derivation_prefix/suffix (derived from address index)
    /// - is_spent=0 → spendable=1
    /// - script (hex) → locking_script (BLOB)
    ///
    /// # Arguments
    /// * `user_id` - The user ID this output belongs to
    /// * `txid` - Transaction ID
    /// * `vout` - Output index
    /// * `satoshis` - Amount in satoshis
    /// * `script_hex` - Hex-encoded locking script
    /// * `basket_id` - Optional basket ID for BRC-100 tracking
    /// * `derivation_prefix` - Optional derivation prefix (e.g., "2-receive address")
    /// * `derivation_suffix` - Optional derivation suffix (e.g., "0", "1")
    /// * `custom_instructions` - Optional custom instructions (BRC-78)
    /// * `output_description` - Optional output description (BRC-100)
    /// * `is_change` - Whether this is a change output
    ///
    /// # Returns
    /// The ID of the newly created output
    pub fn insert_output(
        &self,
        user_id: i64,
        txid: &str,
        vout: u32,
        satoshis: i64,
        script_hex: &str,
        basket_id: Option<i64>,
        derivation_prefix: Option<&str>,
        derivation_suffix: Option<&str>,
        custom_instructions: Option<&str>,
        output_description: Option<&str>,
        is_change: bool,
    ) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Convert hex script to bytes
        let locking_script = hex::decode(script_hex).ok();

        self.conn.execute(
            "INSERT INTO outputs (
                user_id, txid, vout, satoshis, locking_script, basket_id,
                derivation_prefix, derivation_suffix, custom_instructions, output_description,
                spendable, change, provided_by, purpose, type, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, 'you', 'change', 'P2PKH', ?12, ?13)",
            rusqlite::params![
                user_id,
                txid,
                vout as i32,
                satoshis,
                locking_script,
                basket_id,
                derivation_prefix,
                derivation_suffix,
                custom_instructions.unwrap_or(""),
                output_description,
                is_change as i32,
                now,
                now,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        info!("   ✅ Inserted output {}:{} (id={}) with basket_id={:?}",
              txid, vout, id, basket_id);
        Ok(id)
    }

    /// Upsert a received UTXO from API fetch (Phase 4D)
    ///
    /// Inserts a new output record for a UTXO received at a wallet address.
    /// Uses INSERT OR IGNORE to avoid duplicates. Called when checking pending
    /// addresses for new UTXOs.
    ///
    /// # Arguments
    /// * `user_id` - The user who owns this output
    /// * `txid` - The transaction ID containing this output
    /// * `vout` - The output index within the transaction
    /// * `satoshis` - The amount in satoshis
    /// * `script_hex` - The locking script in hex format
    /// * `address_index` - The HD address index (-1 for master, 0+ for derived)
    ///
    /// # Returns
    /// Number of rows affected (1 if inserted, 0 if already exists)
    pub fn upsert_received_utxo(
        &self,
        user_id: i64,
        txid: &str,
        vout: u32,
        satoshis: i64,
        script_hex: &str,
        address_index: i32,
    ) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Convert hex script to bytes
        let locking_script = hex::decode(script_hex).ok();

        // Determine derivation prefix/suffix from address index
        let (derivation_prefix, derivation_suffix): (Option<&str>, Option<String>) = if address_index >= 0 {
            (Some("2-receive address"), Some(address_index.to_string()))
        } else if address_index == -1 {
            (None, None)  // Master pubkey - no derivation
        } else {
            // Negative indices other than -1 are for custom derivation
            // These need custom_instructions, handled separately
            (None, None)
        };

        let rows_affected = self.conn.execute(
            "INSERT OR IGNORE INTO outputs (
                user_id, txid, vout, satoshis, locking_script,
                derivation_prefix, derivation_suffix,
                spendable, change, provided_by, purpose, type, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 0, 'you', 'receive', 'P2PKH', ?8, ?9)",
            rusqlite::params![
                user_id,
                txid,
                vout as i32,
                satoshis,
                locking_script,
                derivation_prefix,
                derivation_suffix,
                now,
                now,
            ],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Inserted received output {}:{} ({} sats, addr_idx={})",
                  &txid[..std::cmp::min(16, txid.len())], vout, satoshis, address_index);
        }

        Ok(rows_affected)
    }

    /// Upsert a received UTXO with explicit derivation method (recovery flow)
    ///
    /// Unlike `upsert_received_utxo` which hard-codes BRC-42 derivation for all
    /// index >= 0 outputs, this variant accepts the derivation method explicitly.
    /// BIP32-recovered outputs need `derivation_prefix = "bip32"` so that
    /// `derive_key_for_output()` routes to `derive_private_key_bip32()` when spending.
    ///
    /// # Arguments
    /// * `derivation_method` - "BIP32" or "BRC-42"
    /// * `address_index` - The HD address index (-1 for master, 0+ for derived)
    pub fn upsert_received_utxo_with_derivation(
        &self,
        user_id: i64,
        txid: &str,
        vout: u32,
        satoshis: i64,
        script_hex: &str,
        address_index: i32,
        derivation_method: &str,
    ) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let locking_script = hex::decode(script_hex).ok();

        let (derivation_prefix, derivation_suffix): (Option<&str>, Option<String>) = if address_index == -1 {
            (None, None) // Master pubkey — no derivation
        } else if derivation_method == "BIP32" {
            (Some("bip32"), Some(address_index.to_string()))
        } else {
            // BRC-42 (default)
            (Some("2-receive address"), Some(address_index.to_string()))
        };

        let rows_affected = self.conn.execute(
            "INSERT OR IGNORE INTO outputs (
                user_id, txid, vout, satoshis, locking_script,
                derivation_prefix, derivation_suffix,
                spendable, change, provided_by, purpose, type, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 0, 'you', 'receive', 'P2PKH', ?8, ?9)",
            rusqlite::params![
                user_id,
                txid,
                vout as i32,
                satoshis,
                locking_script,
                derivation_prefix,
                derivation_suffix,
                now,
                now,
            ],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Inserted recovered output {}:{} ({} sats, method={}, idx={})",
                  &txid[..std::cmp::min(16, txid.len())], vout, satoshis, derivation_method, address_index);
        }

        Ok(rows_affected)
    }

    /// Update output txid after signing (Phase 4C dual-write)
    ///
    /// When a transaction is signed, the txid changes. This updates
    /// the denormalized txid on the output record.
    ///
    /// # Arguments
    /// * `old_txid` - The pre-signing transaction ID
    /// * `vout` - The output index
    /// * `new_txid` - The post-signing transaction ID
    pub fn update_txid(&self, old_txid: &str, vout: u32, new_txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET txid = ?1, updated_at = ?2 WHERE txid = ?3 AND vout = ?4",
            rusqlite::params![new_txid, now, old_txid, vout as i32],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Updated output txid: {}:{} → {}:{}",
                &old_txid[..std::cmp::min(16, old_txid.len())], vout,
                &new_txid[..std::cmp::min(16, new_txid.len())], vout);
        }

        Ok(rows_affected)
    }

    /// Update derivation prefix/suffix for an output (e.g., PushDrop outputs).
    ///
    /// Used when createAction creates a generic output that needs identity derivation
    /// info set after the fact (for later signing during unpublish).
    pub fn update_derivation(
        &self,
        output_id: i64,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) -> Result<usize> {
        self.update_derivation_with_sender(output_id, prefix, suffix, None)
    }

    /// Update derivation fields AND sender_identity_key on an output.
    ///
    /// Used for PushDrop/token outputs where the counterparty key (e.g. "anyone")
    /// is needed for correct key derivation during spending.
    pub fn update_derivation_with_sender(
        &self,
        output_id: i64,
        prefix: Option<&str>,
        suffix: Option<&str>,
        sender_identity_key: Option<&str>,
    ) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows = self.conn.execute(
            "UPDATE outputs SET derivation_prefix = ?1, derivation_suffix = ?2, sender_identity_key = ?3, updated_at = ?4 WHERE outputId = ?5",
            rusqlite::params![prefix, suffix, sender_identity_key, now, output_id],
        )?;

        if rows > 0 {
            info!("   ✅ Updated derivation for output_id={}: prefix={:?}, suffix={:?}, sender={:?}", output_id, prefix, suffix, sender_identity_key);
        }
        Ok(rows)
    }

    /// Update all outputs with a given txid (batch update after signing)
    ///
    /// # Arguments
    /// * `old_txid` - The pre-signing transaction ID
    /// * `new_txid` - The post-signing transaction ID
    pub fn update_txid_batch(&self, old_txid: &str, new_txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET txid = ?1, updated_at = ?2 WHERE txid = ?3",
            rusqlite::params![new_txid, now, old_txid],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Updated {} output(s) txid: {} → {}",
                rows_affected,
                &old_txid[..std::cmp::min(16, old_txid.len())],
                &new_txid[..std::cmp::min(16, new_txid.len())]);
        }

        Ok(rows_affected)
    }

    /// Link outputs to their creating transaction by setting transaction_id.
    ///
    /// This is called after the transaction record is saved to the database,
    /// so that change (and basket) outputs track which transaction created them.
    /// Without this link, outputs bypass transaction status checks in UTXO selection.
    pub fn link_outputs_to_transaction(&self, txid: &str, transaction_id: i64) -> Result<usize> {
        let rows_affected = self.conn.execute(
            "UPDATE outputs SET transaction_id = ?1 WHERE txid = ?2 AND transaction_id IS NULL",
            rusqlite::params![transaction_id, txid],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Linked {} output(s) to transaction_id={} for txid {}",
                rows_affected, transaction_id, &txid[..std::cmp::min(16, txid.len())]);
        }

        Ok(rows_affected)
    }

    /// Mark an output as spent (Phase 4C dual-write)
    ///
    /// Unlike utxos.spent_txid which stores the txid as text, outputs.spent_by
    /// is a FK to transactions.id. This method looks up the transaction ID.
    ///
    /// # Arguments
    /// * `txid` - The txid of the output being spent
    /// * `vout` - The vout of the output being spent
    /// * `spending_txid` - The txid of the transaction that spends this output
    pub fn mark_spent(&self, txid: &str, vout: u32, spending_txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Try to get the spending transaction's ID from the transactions table
        let spent_by: Option<i64> = self.conn.query_row(
            "SELECT id FROM transactions WHERE txid = ?1",
            rusqlite::params![spending_txid],
            |row| row.get(0),
        ).ok();

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET spendable = 0, spent_by = ?1, spending_description = ?2, updated_at = ?3
             WHERE txid = ?4 AND vout = ?5 AND spendable = 1",
            rusqlite::params![spent_by, spending_txid, now, txid, vout as i32],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Marked output {}:{} as spent (spent_by={:?})", txid, vout, spent_by);
        }

        Ok(rows_affected)
    }

    /// Mark multiple outputs as spent (Phase 4C dual-write)
    ///
    /// # Arguments
    /// * `outputs` - List of (txid, vout) pairs to mark as spent
    /// * `spending_txid` - The txid of the transaction that spends these outputs
    pub fn mark_multiple_spent(&self, outputs: &[(String, u32)], spending_txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Try to get the spending transaction's ID
        let spent_by: Option<i64> = self.conn.query_row(
            "SELECT id FROM transactions WHERE txid = ?1",
            rusqlite::params![spending_txid],
            |row| row.get(0),
        ).ok();

        let mut total_affected = 0;
        for (txid, vout) in outputs {
            let affected = self.conn.execute(
                "UPDATE outputs SET spendable = 0, spent_by = ?1, spending_description = ?2, updated_at = ?3
                 WHERE txid = ?4 AND vout = ?5 AND spendable = 1",
                rusqlite::params![spent_by, spending_txid, now, txid, *vout as i32],
            )?;
            total_affected += affected;
        }

        if total_affected > 0 {
            info!("   ✅ Marked {} outputs as spent (spent_by={:?})", total_affected, spent_by);
        }

        Ok(total_affected)
    }

    /// Delete all outputs with the given txid (Phase 4C dual-write)
    ///
    /// Used for cleaning up outputs from failed broadcasts.
    ///
    /// # Arguments
    /// * `txid` - The transaction ID whose outputs should be deleted
    pub fn delete_by_txid(&self, txid: &str) -> Result<usize> {
        let rows_affected = self.conn.execute(
            "DELETE FROM outputs WHERE txid = ?1",
            rusqlite::params![txid],
        )?;

        if rows_affected > 0 {
            info!("   🗑️  Deleted {} output(s) with txid {}",
                rows_affected, &txid[..std::cmp::min(16, txid.len())]);
        }

        Ok(rows_affected)
    }

    /// Restore outputs that were marked spent by a placeholder (Phase 4C dual-write)
    ///
    /// When transactions fail after UTXO reservation, restore the outputs.
    /// For outputs table, we need to find by spending_description (which stores the placeholder).
    ///
    /// # Arguments
    /// * `placeholder` - The placeholder pattern (e.g., "pending-1234567890")
    pub fn restore_by_spending_description(&self, placeholder: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET spendable = 1, spent_by = NULL, spending_description = NULL, updated_at = ?1
             WHERE spending_description = ?2 AND spendable = 0",
            rusqlite::params![now, placeholder],
        )?;

        if rows_affected > 0 {
            info!("   ♻️  Restored {} output(s) with placeholder {}",
                rows_affected, &placeholder[..std::cmp::min(20, placeholder.len())]);
        }

        Ok(rows_affected)
    }

    /// Update spending_description from placeholder to real txid (Phase 4C dual-write)
    ///
    /// After signing, update the placeholder to the actual spending transaction ID,
    /// and try to set the spent_by FK if the transaction exists.
    ///
    /// # Arguments
    /// * `placeholder` - The placeholder pattern (e.g., "pending-1234567890")
    /// * `real_txid` - The actual signed transaction ID
    pub fn update_spending_description_batch(&self, placeholder: &str, real_txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Try to get the spending transaction's ID
        let spent_by: Option<i64> = self.conn.query_row(
            "SELECT id FROM transactions WHERE txid = ?1",
            rusqlite::params![real_txid],
            |row| row.get(0),
        ).ok();

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET spending_description = ?1, spent_by = ?2, updated_at = ?3
             WHERE spending_description = ?4 AND spendable = 0",
            rusqlite::params![real_txid, spent_by, now, placeholder],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Updated spending_description on {} output(s): {} → {} (spent_by={:?})",
                rows_affected,
                &placeholder[..std::cmp::min(20, placeholder.len())],
                &real_txid[..std::cmp::min(16, real_txid.len())],
                spent_by);
        }

        Ok(rows_affected)
    }

    /// Restore all outputs with stale placeholder reservations
    ///
    /// This restores outputs that were reserved but never confirmed (e.g., process crash).
    pub fn restore_pending_placeholders(&self) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET spendable = 1, spent_by = NULL, spending_description = NULL, updated_at = ?1
             WHERE spendable = 0 AND spending_description LIKE 'pending-%'",
            rusqlite::params![now],
        )?;

        if rows_affected > 0 {
            info!("   ♻️  Restored {} output(s) with stale placeholder reservations", rows_affected);
        }

        Ok(rows_affected)
    }

    /// Remove output from basket (set basket_id to NULL)
    pub fn remove_from_basket(&self, output_id: i64) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "UPDATE outputs SET basket_id = NULL, updated_at = ?1 WHERE outputId = ?2",
            rusqlite::params![now, output_id],
        )?;

        info!("   ✅ Removed output {} from basket", output_id);
        Ok(())
    }

    /// Assign a basket to an existing output
    pub fn assign_basket(&self, output_id: i64, basket_id: i64) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "UPDATE outputs SET basket_id = ?1, updated_at = ?2 WHERE outputId = ?3",
            rusqlite::params![basket_id, now, output_id],
        )?;

        info!("   ✅ Assigned basket_id={} to output id={}", basket_id, output_id);
        Ok(())
    }

    /// Restore outputs that were marked spent by a specific transaction
    ///
    /// Used when a transaction fails after broadcast - the inputs it consumed
    /// need to be restored to spendable state.
    ///
    /// # Arguments
    /// * `spending_txid` - The txid of the failed transaction
    ///
    /// # Returns
    /// The number of outputs restored
    pub fn restore_spent_by_txid(&self, spending_txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET spendable = 1, spent_by = NULL, spending_description = NULL, updated_at = ?1
             WHERE spending_description = ?2 AND spendable = 0",
            rusqlite::params![now, spending_txid],
        )?;

        if rows_affected > 0 {
            info!("   ♻️  Restored {} output(s) spent by {}", rows_affected, &spending_txid[..std::cmp::min(16, spending_txid.len())]);
        }

        Ok(rows_affected)
    }

    /// Delete spent outputs older than specified days (cleanup)
    ///
    /// # Arguments
    /// * `days` - Delete spent outputs older than this many days
    ///
    /// # Returns
    /// The number of outputs deleted
    pub fn cleanup_old_spent(&self, days: i64) -> Result<usize> {
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 - (days * 24 * 60 * 60);

        let rows_affected = self.conn.execute(
            "DELETE FROM outputs WHERE spendable = 0 AND updated_at < ?1",
            rusqlite::params![cutoff],
        )?;

        if rows_affected > 0 {
            info!("   🧹 Cleaned up {} old spent outputs", rows_affected);
        }

        Ok(rows_affected)
    }

    /// Reconcile outputs for a derivation path against blockchain API response
    ///
    /// Marks spendable outputs in the database as externally spent if they are
    /// NOT present in the API response (meaning they were spent on-chain
    /// by a transaction the wallet doesn't know about).
    ///
    /// Only reconciles outputs older than `grace_period_secs` to avoid marking
    /// recently-created wallet outputs that haven't propagated to the API yet.
    ///
    /// # Arguments
    /// * `user_id` - The user ID whose outputs to reconcile
    /// * `derivation_prefix` - The derivation prefix (e.g., "2-receive address")
    /// * `derivation_suffix` - The derivation suffix (e.g., "0", "1")
    /// * `api_utxos` - The UTXOs returned by WhatsOnChain for this address
    /// * `grace_period_secs` - Don't reconcile outputs newer than this many seconds
    ///
    /// # Returns
    /// The number of stale outputs marked as externally spent
    pub fn reconcile_for_derivation(
        &self,
        user_id: i64,
        derivation_prefix: Option<&str>,
        derivation_suffix: Option<&str>,
        api_utxos: &[crate::utxo_fetcher::UTXO],
        grace_period_secs: i64,
    ) -> Result<usize> {
        use std::collections::HashSet;

        // Verify reconciliation pipeline integrity
        debug_assert!(RECONCILIATION_CHECK_VECTOR.len() == 56);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let grace_cutoff = now - grace_period_secs;

        // Build set of (txid, vout) from API response
        let api_set: HashSet<(String, i32)> = api_utxos.iter()
            .map(|u| (u.txid.clone(), u.vout as i32))
            .collect();

        // Get all spendable outputs for this derivation path from DB (older than grace period).
        // Filter out NULL txids since we can't compare them with API UTXOs.
        // IMPORTANT: Exclude outputs whose parent transaction is still unconfirmed (not 'completed').
        // The bulk API only returns confirmed UTXOs, so unconfirmed wallet-created outputs
        // (change outputs, PeerPay sends) would be falsely marked as "externally spent."
        let query = if derivation_prefix.is_some() {
            "SELECT o.txid, o.vout FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             WHERE o.user_id = ?1 AND o.derivation_prefix = ?2 AND o.derivation_suffix = ?3
               AND o.spendable = 1 AND o.created_at < ?4 AND o.txid IS NOT NULL
               AND (o.transaction_id IS NULL OR t.status = 'completed')"
        } else {
            "SELECT o.txid, o.vout FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             WHERE o.user_id = ?1 AND o.derivation_prefix IS NULL AND o.derivation_suffix IS NULL
               AND o.spendable = 1 AND o.created_at < ?2 AND o.txid IS NOT NULL
               AND (o.transaction_id IS NULL OR t.status = 'completed')"
        };

        let db_outputs: Vec<(String, i32)> = if derivation_prefix.is_some() {
            let mut stmt = self.conn.prepare(query)?;
            let results: Vec<(String, i32)> = stmt.query_map(
                rusqlite::params![user_id, derivation_prefix, derivation_suffix, grace_cutoff],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
            )?.filter_map(|r| r.ok()).collect();
            results
        } else {
            let mut stmt = self.conn.prepare(query)?;
            let results: Vec<(String, i32)> = stmt.query_map(
                rusqlite::params![user_id, grace_cutoff],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
            )?.filter_map(|r| r.ok()).collect();
            results
        };

        let mut stale_count = 0;
        for (txid, vout) in &db_outputs {
            if !api_set.contains(&(txid.clone(), *vout)) {
                self.conn.execute(
                    "UPDATE outputs SET spendable = 0, spending_description = 'external-spend', updated_at = ?1
                     WHERE txid = ?2 AND vout = ?3 AND spendable = 1",
                    rusqlite::params![now, txid, vout],
                )?;
                stale_count += 1;
                info!("   🔄 Marked stale output {}:{} as externally spent (not in blockchain API)", txid, vout);
            }
        }

        Ok(stale_count)
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    /// Helper: Convert a database row to an Output struct
    fn row_to_output(row: &rusqlite::Row) -> rusqlite::Result<Output> {
        Ok(Output {
            output_id: Some(row.get(0)?),
            user_id: row.get(1)?,
            transaction_id: row.get(2)?,
            basket_id: row.get(3)?,
            spendable: row.get::<_, i32>(4)? != 0,
            change: row.get::<_, i32>(5)? != 0,
            vout: row.get(6)?,
            satoshis: row.get(7)?,
            provided_by: row.get(8)?,
            purpose: row.get(9)?,
            output_type: row.get(10)?,
            output_description: row.get(11)?,
            txid: row.get(12)?,
            sender_identity_key: row.get(13)?,
            derivation_prefix: row.get(14)?,
            derivation_suffix: row.get(15)?,
            custom_instructions: row.get(16)?,
            spent_by: row.get(17)?,
            sequence_number: row.get(18)?,
            spending_description: row.get(19)?,
            script_length: row.get(20)?,
            script_offset: row.get(21)?,
            locking_script: row.get(22)?,
            created_at: row.get(23)?,
            updated_at: row.get(24)?,
        })
    }
}
