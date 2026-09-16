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

use super::Output;

/// A `pending-%` reservation that has outlived the sweep threshold.
///
/// Produced by [`OutputRepository::list_stale_pending_reservations`]; a candidate for
/// release, **not** a decision to release. See that method for why the decision needs
/// on-chain evidence.
#[derive(Debug, Clone)]
pub struct StaleReservation {
    pub txid: String,
    pub vout: u32,
    pub satoshis: i64,
    /// The `pending-{ts}-{seq}` (or `pending-backup-{ts}`) placeholder still held.
    pub placeholder: String,
    /// Unix seconds the reservation was taken (written by `mark_multiple_spent`).
    pub updated_at: i64,
}

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
             LEFT JOIN output_baskets b ON o.basket_id = b.basketId
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nosend', 'nonfinal'))
               AND o.derivation_prefix IS NOT NULL
               AND (o.basket_id IS NULL OR b.name = 'default')
               AND (o.transaction_id IS NOT NULL OR o.confirmed = 1)
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
             LEFT JOIN output_baskets b ON o.basket_id = b.basketId
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status = 'completed' OR (o.transaction_id IS NULL AND o.confirmed = 1))
               AND o.derivation_prefix IS NOT NULL
               AND (o.basket_id IS NULL OR b.name = 'default')
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
        // NOTE: nosend outputs are INCLUDED in balance display so users see expected change
        // after overlay/nosend transactions. They are NOT available for UTXO selection
        // (get_spendable_by_user still excludes nosend). If the nosend tx fails,
        // TaskCheckForProofs deletes the change output and restores inputs automatically.
        // beta.3 Phase 10e, 👤 owner 2026-09-16 — an output filed in a NON-DEFAULT
        // basket is not money. It is a token carrier: a 1Sat Ordinal, an OpNS name,
        // an app's own marker. `get_spendable_by_user` has always excluded them from
        // coin selection (`o.basket_id IS NULL OR b.name = 'default'`), so counting
        // them here showed the user a balance containing satoshis the wallet would
        // refuse to spend — and, worse, spending one would destroy the asset
        // (BRC-147, and `R-DUST`'s whole reason for existing).
        //
        // 📏 Measured on the dev wallet the day this landed: 9 rows, 9 satoshis —
        // a name token, a todo token and seven upvote tokens. Every satoshi removed
        // is a carrier.
        //
        // ⛔ Deliberately NOT also filtering `derivation_prefix IS NULL`, even though
        // `get_spendable_by_user` does. Measured: **0 rows, 0 satoshis** on this
        // wallet, so adding it would change nothing here and could HIDE real money on
        // a wallet that has such rows — an output with no derivation recipe is locked
        // to the master key itself, which we hold. That mismatch between what the
        // balance counts and what the selector will spend is recorded as an open
        // question rather than guessed at; see `10e-panel-remainder/README.md`.
        //
        // ⚠️ `nosend` stays INCLUDED — see the note above; that is deliberate UX.
        let balance: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(o.satoshis), 0)
             FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             LEFT JOIN output_baskets b ON o.basket_id = b.basketId
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nonfinal'))
               AND COALESCE(o.derivation_prefix, '') != '1-wallet-backup'
               AND (o.basket_id IS NULL OR b.name = 'default')",
            rusqlite::params![user_id],
            |row| row.get(0),
        )?;

        Ok(balance)
    }

    /// Calculate total balance from spendable outputs (all users)
    ///
    /// This is useful for single-user wallets where we don't need to filter by user.
    ///
    /// ⚠️ Deliberately NOT given `calculate_balance`'s basket filter (2026-09-16).
    /// This one is not shown to the user — its callers are `wallet_recover_onchain`
    /// and `task_backup`'s significance threshold — and narrowing it would change
    /// when backups fire, which nobody asked for. The difference is the token
    /// carriers only (9 satoshis when measured). Recorded, not fixed.
    pub fn calculate_total_balance(&self) -> Result<i64> {
        // NOTE: nosend outputs included in balance (see calculate_balance comment)
        let balance: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(o.satoshis), 0)
             FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             WHERE o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nonfinal'))
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
        self.upsert_received_utxo_with_confirmed(user_id, txid, vout, satoshis, script_hex, address_index, true)
    }

    /// Insert a received UTXO with explicit confirmed flag.
    ///
    /// `confirmed = true` for UTXOs from the confirmed bulk API.
    /// `confirmed = false` for UTXOs only seen in mempool (single-address API, height <= 0).
    pub fn upsert_received_utxo_with_confirmed(
        &self,
        user_id: i64,
        txid: &str,
        vout: u32,
        satoshis: i64,
        script_hex: &str,
        address_index: i32,
        confirmed: bool,
    ) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Convert hex script to bytes
        let locking_script = hex::decode(script_hex).ok();

        // Determine derivation prefix/suffix from address index
        // Regular addresses (index >= 0): BRC-42 self-derivation
        // Master address (index -1): master private key directly — spendable
        // Backup address (index -3): backup marker — NOT spendable by regular sends
        let (derivation_prefix, derivation_suffix): (Option<&str>, Option<String>) = if address_index >= 0 {
            (Some("2-receive address"), Some(address_index.to_string()))
        } else if address_index == -1 {
            (Some("master"), Some("-1".to_string()))
        } else {
            // Negative indices other than -1 (e.g., -3 backup marker)
            // Leave as NULL — not spendable by regular sends
            (None, None)
        };

        let confirmed_int: i32 = if confirmed { 1 } else { 0 };

        let rows_affected = self.conn.execute(
            "INSERT OR IGNORE INTO outputs (
                user_id, txid, vout, satoshis, locking_script,
                derivation_prefix, derivation_suffix,
                spendable, change, provided_by, purpose, type, confirmed, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 0, 'you', 'receive', 'P2PKH', ?8, ?9, ?10)",
            rusqlite::params![
                user_id,
                txid,
                vout as i32,
                satoshis,
                locking_script,
                derivation_prefix,
                derivation_suffix,
                confirmed_int,
                now,
                now,
            ],
        )?;

        if rows_affected > 0 {
            let status_str = if confirmed { "confirmed" } else { "unconfirmed" };
            info!("   ✅ Inserted {} received output {}:{} ({} sats, addr_idx={})",
                  status_str, &txid[..std::cmp::min(16, txid.len())], vout, satoshis, address_index);
        }

        Ok(rows_affected)
    }

    /// Mark a previously-unconfirmed output as confirmed.
    /// Called when an output that was only in mempool now appears in the confirmed API.
    pub fn mark_output_confirmed(&self, txid: &str, vout: i32) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let rows = self.conn.execute(
            "UPDATE outputs SET confirmed = 1, updated_at = ?1 WHERE txid = ?2 AND vout = ?3 AND confirmed = 0",
            rusqlite::params![now, txid, vout],
        )?;
        if rows > 0 {
            info!("   ✅ Marked output {}:{} as confirmed", &txid[..std::cmp::min(16, txid.len())], vout);
        }
        Ok(rows)
    }

    /// Get unconfirmed outputs older than the given timeout.
    /// Used by TaskSyncPending to detect unconfirmed receives that failed to confirm.
    /// Returns `(txid, vout, satoshis, locking_script)` — the script travels with
    /// the row so stale promotion can compare it with the chain (beta.3 10a).
    pub fn get_stale_unconfirmed(&self, user_id: i64, timeout_secs: i64) -> Result<Vec<(String, u32, i64, Vec<u8>)>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let cutoff = now - timeout_secs;

        let mut stmt = self.conn.prepare(
            "SELECT txid, vout, satoshis, locking_script FROM outputs
             WHERE user_id = ?1 AND confirmed = 0 AND spendable = 1 AND created_at < ?2 AND txid IS NOT NULL"
        )?;
        let results = stmt.query_map(
            rusqlite::params![user_id, cutoff],
            |row| Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<Vec<u8>>>(3)?.unwrap_or_default(),
            )),
        )?.filter_map(|r| r.ok()).collect();
        Ok(results)
    }

    /// Delete an unconfirmed output (failed to confirm after timeout).
    pub fn delete_unconfirmed_output(&self, txid: &str, vout: u32) -> Result<usize> {
        let rows = self.conn.execute(
            "DELETE FROM outputs WHERE txid = ?1 AND vout = ?2 AND confirmed = 0",
            rusqlite::params![txid, vout as i32],
        )?;
        Ok(rows)
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
    /// * `confirmed` - false when the UTXO was only seen in mempool (height <= 0)
    ///
    /// ⛔ `confirmed` is an EXPLICIT parameter, never the schema default. (P0.5 panel #2, 1.6)
    ///
    /// This INSERT used to list 14 columns and omit `confirmed`, so every row it
    /// wrote took the `DEFAULT 1` from the V14 migration and a mempool-only output
    /// was recorded as CONFIRMED. 4eacb51 fixed exactly that bug on the sibling
    /// `upsert_received_utxo_with_confirmed`, but its audit was file-scoped rather
    /// than dataflow-scoped and missed this function — so the bug stayed live on
    /// `wallet_recover` and `wallet_rescan`, the two paths where a user is most
    /// exposed, because a restore is when the DB has nothing to cross-check against.
    /// The value was already sitting on `utxo_fetcher::UTXO::confirmed` at both call
    /// sites; it simply was not written.
    pub fn upsert_received_utxo_with_derivation(
        &self,
        user_id: i64,
        txid: &str,
        vout: u32,
        satoshis: i64,
        script_hex: &str,
        address_index: i32,
        derivation_method: &str,
        confirmed: bool,
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
                spendable, change, provided_by, purpose, type, created_at, updated_at,
                confirmed
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 0, 'you', 'receive', 'P2PKH', ?8, ?9, ?10)",
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
                if confirmed { 1i32 } else { 0i32 },
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

    /// Disable (not delete) all outputs with the given txid
    ///
    /// Used for cleaning up outputs from failed broadcasts. Instead of deleting,
    /// marks outputs as non-spendable with 'failed-tx-output' description.
    /// If TaskUnFail later discovers the tx was actually mined, it can re-enable
    /// these outputs by flipping spendable back to 1 — preserving all metadata
    /// (derivation info, basket, etc.) that would be lost on delete.
    ///
    /// # Arguments
    /// * `txid` - The transaction ID whose outputs should be disabled
    pub fn disable_by_txid(&self, txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET spendable = 0, spending_description = 'failed-tx-output', updated_at = ?1
             WHERE txid = ?2 AND spendable = 1",
            rusqlite::params![now, txid],
        )?;

        if rows_affected > 0 {
            info!("   🚫 Disabled {} output(s) with txid {} (failed-tx-output)",
                rows_affected, &txid[..std::cmp::min(16, txid.len())]);
        }

        Ok(rows_affected)
    }

    /// Re-enable outputs that were disabled during a false failure cleanup
    ///
    /// Called by TaskUnFail when a transaction that was marked failed turns out
    /// to have been mined on-chain. Restores the outputs to spendable state.
    pub fn reenable_failed_outputs(&self, txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET spendable = 1, spending_description = NULL, updated_at = ?1
             WHERE txid = ?2 AND spending_description = 'failed-tx-output'",
            rusqlite::params![now, txid],
        )?;

        if rows_affected > 0 {
            info!("   ♻️  Re-enabled {} output(s) with txid {} (recovered from false failure)",
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
        // ⭐ RIG SEAM for `PAYMENT_TEST_BATCH.md` M4 (`P8b-A1`) — and it is safe by
        // construction, not by promise.
        //
        // P8b's rule is "if the wallet cannot record which coins a transaction
        // spends, it must not broadcast that transaction". Proving it needs this
        // function to fail ON DEMAND in a RELEASE build, because the dev browser is
        // a release build, so `#[cfg(test)]` cannot reach it.
        //
        // ⛔ Two locks, both required: the env var must be set AND `hodos::is_dev()`
        // must be true. `main.rs :: enforce_dev_safeguard` runs first in `main()` and
        // SCRUBS `HODOS_DEV` from the environment of any binary that is not running
        // from a build directory — so a shipped wallet cannot enter this branch even
        // if someone sets the variable. That reuses a safeguard that already exists
        // and is already trusted, rather than inventing a second one.
        if std::env::var("HODOS_DEV").as_deref() == Ok("1")
            && std::env::var("HODOS_FAIL_SPEND_RESOLUTION").is_ok()
        {
            log::warn!("🧪 RIG: HODOS_FAIL_SPEND_RESOLUTION is set — failing spend resolution on purpose (M4)");
            return Err(rusqlite::Error::InvalidQuery);
        }
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

    /// List reservations still holding a `pending-%` placeholder older than
    /// `older_than_secs`.
    ///
    /// This is the *read* half of the stale-reservation sweep. It deliberately does
    /// NOT release anything: releasing requires proving the outpoint is still unspent
    /// on-chain, which needs the network and therefore cannot live in the repository.
    ///
    /// ## Why this replaced the old blanket restore
    ///
    /// The previous `restore_pending_placeholders()` was a single
    /// `UPDATE ... WHERE spending_description LIKE 'pending-%'` with no age filter and
    /// no on-chain check, run unconditionally at startup. A row can legitimately hold a
    /// `pending-` placeholder while its transaction is already broadcast: every path
    /// resolves placeholder -> real txid *before* broadcasting, but
    /// `update_spending_description_batch` failure is only logged as a warning and the
    /// broadcast proceeds anyway. Un-spending such a row makes the wallet re-offer an
    /// output it has already spent -- a double-spend (`R-NODOUBLE`).
    ///
    /// `updated_at` is written by `mark_multiple_spent` at reservation time, so it is
    /// the reservation age; no schema change is needed to age these out.
    ///
    /// Note the `LIKE 'pending-%'` pattern also matches the `pending-backup-{ts}`
    /// namespace used by the on-chain backup path. That is intentional: that path
    /// releases via `rollback_backup` on its own error returns, but a process kill can
    /// still strand it, and the same on-chain proof gates its release.
    pub fn list_stale_pending_reservations(&self, older_than_secs: i64) -> Result<Vec<StaleReservation>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let cutoff = now - older_than_secs;

        let mut stmt = self.conn.prepare(
            "SELECT txid, vout, satoshis, spending_description, updated_at
             FROM outputs
             WHERE spendable = 0
               AND txid IS NOT NULL
               AND spending_description LIKE 'pending-%'
               AND updated_at <= ?1
             ORDER BY updated_at",
        )?;

        let rows = stmt.query_map(rusqlite::params![cutoff], |row| {
            Ok(StaleReservation {
                txid: row.get(0)?,
                vout: row.get::<_, i64>(1)? as u32,
                satoshis: row.get(2)?,
                placeholder: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Release exactly one reserved outpoint, and only if it is *still* reserved under
    /// `placeholder`.
    ///
    /// Scoping the `UPDATE` to the placeholder is what keeps the sweep from widening the
    /// selection race (`R-NORACE`): between listing a stale reservation and releasing it,
    /// a concurrent `createAction` may have legitimately re-reserved the row under a new
    /// placeholder. The `spending_description = ?placeholder` predicate makes that a
    /// 0-row update instead of freeing a live reservation.
    ///
    /// Returns the number of rows released (0 or 1).
    pub fn restore_outpoint_if_reserved(&self, txid: &str, vout: u32, placeholder: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET spendable = 1, spent_by = NULL, spending_description = NULL, updated_at = ?1
             WHERE txid = ?2 AND vout = ?3 AND spending_description = ?4 AND spendable = 0",
            rusqlite::params![now, txid, vout, placeholder],
        )?;

        if rows_affected > 0 {
            info!("   ♻️  Released stale reservation {}:{} (placeholder {})",
                &txid[..std::cmp::min(16, txid.len())],
                vout,
                &placeholder[..std::cmp::min(24, placeholder.len())]);
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

    // reconcile_for_derivation — REMOVED (2026-04-20)
    //
    // This function marked spendable outputs as "externally spent" if they were
    // absent from the WoC bulk address UTXO response. This was fundamentally
    // broken because:
    // 1. The bulk endpoint only returns confirmed UTXOs — unconfirmed outputs
    //    (valid and spendable via BEEF) were falsely killed
    // 2. PushDrop/nonstandard outputs don't appear in address queries at all
    // 3. Partial/stale API responses were treated as complete truth
    // 4. No individual verification (GET /tx/{txid}/{vout}/spent) was done
    //    before permanently marking outputs as non-spendable
    //
    // Result: $15+ in valid on-chain outputs wrongly marked non-spendable.
    //
    // If output reconciliation is needed in the future (e.g., multi-device sync),
    // the correct approach is: for each output in question, call
    // GET /tx/{txid}/{vout}/spent — if it returns 200 with a spending txid,
    // THEN mark as spent (recording the spending txid). Never infer spent
    // status from absence in a bulk query.

    // =========================================================================
    // Double-spend verification methods
    // =========================================================================

    /// Get all outputs marked as suspected double-spend (pending verification).
    /// Returns (output_id, txid, vout, locking_script, spending_description, updated_at).
    pub fn get_suspected_double_spends(&self) -> Result<Vec<(i64, String, u32, Vec<u8>, String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT outputId, txid, vout, locking_script, spending_description, updated_at
             FROM outputs
             WHERE spending_description LIKE 'dss:%'
             ORDER BY updated_at ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, Vec<u8>>(3).unwrap_or_default(),
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Confirm a suspected double-spend as real after independent verification.
    /// Changes spending_description from 'dss:...' to 'double-spend-detected'.
    pub fn confirm_double_spend(&self, output_id: i64) -> Result<usize> {
        let rows = self.conn.execute(
            "UPDATE outputs SET spending_description = ?1, updated_at = strftime('%s','now')
             WHERE outputId = ?2 AND spending_description LIKE 'dss:%'",
            rusqlite::params![crate::arc_status::CONFIRMED_DOUBLE_SPEND, output_id],
        )?;
        Ok(rows)
    }

    /// Clear a false double-spend suspicion — restore the output as spendable.
    pub fn clear_suspected_double_spend(&self, output_id: i64) -> Result<usize> {
        let rows = self.conn.execute(
            "UPDATE outputs SET spendable = 1, spending_description = NULL, spent_by = NULL,
                    updated_at = strftime('%s','now')
             WHERE outputId = ?1 AND spending_description LIKE 'dss:%'",
            rusqlite::params![output_id],
        )?;
        Ok(rows)
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

#[cfg(test)]
mod stale_reservation_tests {
    use super::*;
    use crate::database::migrations;

    const HOUR: i64 = 3600;

    fn now() -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
    }

    fn seed_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        migrations::create_schema_v1(&conn).unwrap();
        conn.execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at)
             VALUES (1, 'test_identity_key', 'local', 0, 0)",
            [],
        )
        .unwrap();
        conn
    }

    /// Insert an output reserved under `placeholder`, `age_secs` seconds ago.
    fn reserved(conn: &Connection, txid: &str, vout: u32, sats: i64, placeholder: &str, age_secs: i64) {
        let ts = now() - age_secs;
        conn.execute(
            "INSERT INTO outputs (user_id, spendable, change, vout, satoshis, provided_by, purpose,
                                  type, txid, spending_description, confirmed, created_at, updated_at)
             VALUES (1, 0, 0, ?1, ?2, 'you', '', 'P2PKH', ?3, ?4, 1, ?5, ?5)",
            rusqlite::params![vout, sats, txid, placeholder, ts],
        )
        .unwrap();
    }

    fn is_spendable(conn: &Connection, txid: &str, vout: u32) -> bool {
        conn.query_row(
            "SELECT spendable FROM outputs WHERE txid = ?1 AND vout = ?2",
            rusqlite::params![txid, vout],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            == 1
    }

    /// The age filter is the whole point: a reservation younger than the threshold belongs to
    /// a `createAction` that may still be in flight, and must not be a sweep candidate.
    #[test]
    fn age_filter_excludes_fresh_reservations_and_includes_old_ones() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);

        reserved(&conn, "aa", 0, 1_000, "pending-1-0", 30);
        reserved(&conn, "bb", 1, 2_000, "pending-2-0", 2 * HOUR);

        let stale = repo.list_stale_pending_reservations(15 * 60).unwrap();

        assert_eq!(stale.len(), 1, "only the 2h-old reservation is a candidate");
        assert_eq!(stale[0].txid, "bb");
        assert_eq!(stale[0].vout, 1);
        assert_eq!(stale[0].satoshis, 2_000);
        assert_eq!(stale[0].placeholder, "pending-2-0");
    }

    /// The LIKE pattern deliberately spans every reservation namespace that can strand:
    /// createAction, on-chain backup (`pending-backup-`) and certificate unpublish
    /// (`pending-unpub-`). It must not pick up unrelated spending_descriptions -- a real txid,
    /// an `external-spend` marker, a `dss:` suspected double-spend, or the dust consolidator's
    /// `consolidate-` namespace.
    #[test]
    fn matches_every_pending_namespace_and_nothing_else() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);

        reserved(&conn, "c1", 0, 1, "pending-1787411964292-0", 2 * HOUR);
        reserved(&conn, "c2", 0, 1, "pending-backup-1787411964292", 2 * HOUR);
        reserved(&conn, "c3", 0, 1, "pending-unpub-1787411964292", 2 * HOUR);
        reserved(&conn, "d1", 0, 1, "consolidate-1787411964292", 2 * HOUR);
        reserved(&conn, "d2", 0, 1, "external-spend", 2 * HOUR);
        reserved(&conn, "d3", 0, 1, "dss:abc123", 2 * HOUR);
        reserved(&conn, "d4", 0, 1, "50939442970f532a2d764d70a75a98c9b80695430b5ffd3b52e9a0348c3ab1be", 2 * HOUR);

        let mut got: Vec<String> = repo
            .list_stale_pending_reservations(15 * 60)
            .unwrap()
            .into_iter()
            .map(|r| r.txid)
            .collect();
        got.sort();

        assert_eq!(got, vec!["c1", "c2", "c3"]);
    }

    /// A spendable row is not a reservation, however old it looks.
    #[test]
    fn ignores_rows_that_are_not_reserved() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        let ts = now() - 2 * HOUR;
        conn.execute(
            "INSERT INTO outputs (user_id, spendable, change, vout, satoshis, provided_by, purpose,
                                  type, txid, spending_description, confirmed, created_at, updated_at)
             VALUES (1, 1, 0, 0, 500, 'you', '', 'P2PKH', 'ee', 'pending-9-0', 1, ?1, ?1)",
            rusqlite::params![ts],
        )
        .unwrap();

        assert!(repo.list_stale_pending_reservations(15 * 60).unwrap().is_empty());
    }

    #[test]
    fn release_returns_the_outpoint_to_spendable() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        reserved(&conn, "ff", 2, 7_000, "pending-3-0", 2 * HOUR);

        assert_eq!(repo.restore_outpoint_if_reserved("ff", 2, "pending-3-0").unwrap(), 1);
        assert!(is_spendable(&conn, "ff", 2));

        let desc: Option<String> = conn
            .query_row("SELECT spending_description FROM outputs WHERE txid = 'ff'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(desc, None, "the placeholder is cleared on release");
    }

    /// R-NORACE. Between listing a stale reservation and releasing it, a concurrent
    /// `createAction` can legitimately re-reserve the row under a *new* placeholder. Releasing
    /// by outpoint alone would free that live reservation and let two transactions select the
    /// same UTXO -- strictly worse than the leak this sweep fixes. The release is therefore
    /// scoped to the placeholder it was listed under.
    #[test]
    fn release_does_not_touch_a_row_re_reserved_under_a_new_placeholder() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        reserved(&conn, "ab", 0, 9_000, "pending-old-0", 2 * HOUR);

        conn.execute(
            "UPDATE outputs SET spending_description = 'pending-new-1' WHERE txid = 'ab'",
            [],
        )
        .unwrap();

        assert_eq!(
            repo.restore_outpoint_if_reserved("ab", 0, "pending-old-0").unwrap(),
            0,
            "stale placeholder must not free a live reservation"
        );
        assert!(!is_spendable(&conn, "ab", 0), "the row stays reserved for the live caller");
    }

    /// The same scoping protects the ordinary success path: once `sign_action` has resolved
    /// the placeholder to the real txid, a late sweep must not un-spend it.
    #[test]
    fn release_does_not_touch_a_row_already_resolved_to_a_real_txid() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        reserved(&conn, "ac", 0, 4_000, "pending-4-0", 2 * HOUR);
        conn.execute(
            "UPDATE outputs SET spending_description = '50939442970f532a' WHERE txid = 'ac'",
            [],
        )
        .unwrap();

        assert_eq!(repo.restore_outpoint_if_reserved("ac", 0, "pending-4-0").unwrap(), 0);
        assert!(!is_spendable(&conn, "ac", 0));
    }

    /// Releasing one outpoint must not disturb the siblings reserved alongside it -- the
    /// sweep verifies on-chain state per outpoint, so it releases per outpoint.
    #[test]
    fn release_is_scoped_to_a_single_outpoint() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        reserved(&conn, "ad", 0, 1_000, "pending-5-0", 2 * HOUR);
        reserved(&conn, "ad", 1, 2_000, "pending-5-0", 2 * HOUR);

        assert_eq!(repo.restore_outpoint_if_reserved("ad", 0, "pending-5-0").unwrap(), 1);
        assert!(is_spendable(&conn, "ad", 0));
        assert!(!is_spendable(&conn, "ad", 1), "sibling reservation is untouched");
    }
}

#[cfg(test)]
mod reservation_race_tests {
    use super::*;
    use crate::database::migrations;

    fn seed_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        migrations::create_schema_v1(&conn).unwrap();
        conn.execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at)
             VALUES (1, 'test_identity_key', 'local', 0, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO outputs (user_id, spendable, change, vout, satoshis, provided_by, purpose,
                                  type, txid, confirmed, created_at, updated_at)
             VALUES (1, 1, 0, 0, 10000, 'you', '', 'P2PKH', 'race', 1, 0, 0)",
            [],
        )
        .unwrap();
        conn
    }

    /// R-NORACE, tested at the level where the race is actually prevented.
    ///
    /// Two `createAction`s that both select the same UTXO are only stopped by
    /// `mark_multiple_spent`'s `AND spendable = 1` predicate: the first reservation wins and
    /// the second updates **zero** rows, so the second caller cannot claim the coin.
    ///
    /// The assertion is deliberately `0`, not "the two callers picked different UTXOs". A
    /// harness that fires two concurrent sends at a 40-UTXO wallet reports "different UTXOs"
    /// whether or not any reservation exists — the correct answer would be the answer anyway,
    /// which is exactly the shape of test that has cost this sprint days before. Here, a
    /// broken reservation reports `1`, so the number only comes out right if the mechanism
    /// works.
    #[test]
    fn second_reservation_of_the_same_outpoint_claims_nothing() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        let outpoint = vec![("race".to_string(), 0u32)];

        let first = repo.mark_multiple_spent(&outpoint, "pending-caller-a-0").unwrap();
        assert_eq!(first, 1, "the first caller reserves the coin");

        let second = repo.mark_multiple_spent(&outpoint, "pending-caller-b-1").unwrap();
        assert_eq!(second, 0, "the second caller must claim nothing");

        // And the coin is still held by caller A, not silently re-labelled to caller B.
        let desc: String = conn
            .query_row("SELECT spending_description FROM outputs WHERE txid = 'race'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(desc, "pending-caller-a-0");
    }

    /// The losing caller's scope guard must not free the winner's reservation. This is the
    /// specific way a release bug would *widen* the race rather than fix the leak: caller B
    /// fails validation, B's guard fires, and A's coin goes back in the pool while A is still
    /// building its transaction.
    #[test]
    fn losing_callers_guard_does_not_free_the_winners_reservation() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        let outpoint = vec![("race".to_string(), 0u32)];

        repo.mark_multiple_spent(&outpoint, "pending-caller-a-0").unwrap();
        repo.mark_multiple_spent(&outpoint, "pending-caller-b-1").unwrap(); // claims nothing

        // Caller B hits one of the 19 early returns; its guard releases its own placeholder.
        let freed = repo.restore_by_spending_description("pending-caller-b-1").unwrap();
        assert_eq!(freed, 0, "B's guard must not free a coin B never reserved");

        let spendable: i64 = conn
            .query_row("SELECT spendable FROM outputs WHERE txid = 'race'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(spendable, 0, "A still holds the reservation");
    }
}

/// `P8-A7` — evidence for the beta.3 dust-guard ticket's severity question.
///
/// The ticket left one thing unverified: *"whether an ordinary incoming 1-sat payment
/// becomes a tracked default-basket row without a recovery scan."* These tests answer
/// it by measurement rather than by reading the code. `upsert_received_utxo_with_confirmed`
/// is the exact insert used by `monitor::task_sync_pending` — an automatic task on a
/// 30-second tier — and by `handlers::wallet_sync`.
#[cfg(test)]
mod token_reserved_exposure_tests {
    use super::*;
    use crate::database::migrations;

    fn seed_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        migrations::create_schema_v1(&conn).unwrap();
        conn.execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at)
             VALUES (1, 'test_identity_key', 'local', 0, 0)",
            [],
        )
        .unwrap();
        conn
    }

    const SCRIPT: &str = "76a914abababababababababababababababababababab88ac";

    /// 👤 Owner 2026-09-16 — a token carrier in a basket is NOT money.
    ///
    /// `get_spendable_by_user` has always refused non-default baskets, so counting
    /// them in the balance showed satoshis the wallet would not spend — and spending
    /// one would destroy the asset. The two views of one wallet disagreed.
    #[test]
    fn basketed_outputs_are_not_counted_as_balance() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);

        // ordinary money, default pool
        repo.upsert_received_utxo_with_confirmed(1, "ord", 0, 50_000, SCRIPT, 0, true)
            .unwrap();
        let money_only = repo.calculate_balance(1).unwrap();
        assert_eq!(money_only, 50_000);

        // a 1-sat carrier filed into an app basket, exactly as internalizeAction files one
        let basket_repo = crate::database::BasketRepository::new(&conn);
        let basket_id = basket_repo.find_or_insert("xanaverse-upvotes", 1).unwrap();
        repo.insert_output(1, "carrier_tx", 0, 1, SCRIPT, Some(basket_id),
                           None, None, None, None, false)
            .unwrap();

        // the carrier is stored and spendable-flagged...
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM outputs WHERE spendable = 1", [], |r| r.get::<_, i64>(0)).unwrap(),
            2, "fixture check: the carrier row exists and carries spendable = 1");
        // ...and coin selection already refuses it
        assert!(!repo.get_spendable_by_user(1).unwrap().iter().any(|o| o.txid.as_deref() == Some("carrier_tx")),
                "control: the selector has always excluded it");
        // ⇒ so the balance must refuse it too
        assert_eq!(repo.calculate_balance(1).unwrap(), money_only,
                   "a basketed carrier must not move the displayed balance");
    }

    /// GREEN — a 1-satoshi payment arriving at a receive address becomes a fully
    /// spendable, default-pool row with no user action and no recovery scan.
    /// This is the exposure the ticket asked about, and it is real.
    #[test]
    fn incoming_one_sat_payment_becomes_a_spendable_default_pool_row() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);

        // Exactly what address sync does: address_index 0, confirmed.
        let inserted = repo
            .upsert_received_utxo_with_confirmed(1, "ord", 0, 1, SCRIPT, 0, true)
            .unwrap();
        assert_eq!(inserted, 1);

        let spendable = repo.get_spendable_confirmed_by_user(1).unwrap();
        assert_eq!(spendable.len(), 1,
            "the 1-sat output IS returned to every spend path that reads this query");
        assert_eq!(spendable[0].satoshis, 1);
        assert!(spendable[0].basket_id.is_none(),
            "no basket is assigned on ingest — this is the actual defect, and the \
             reason the existing basket exclusion never fires for a token");
    }

    /// RED half — the same row filed into a non-default basket is excluded.
    ///
    /// This proves the basket filter in `get_spendable_confirmed_by_user` WORKS, so
    /// the green half above is not a broken query: it is a missing classification.
    /// That is why the beta.3 fix is a value floor and the real fix (beta.4 sprint 1)
    /// is classification on ingest.
    #[test]
    fn the_same_row_in_a_protective_basket_is_excluded() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);

        repo.upsert_received_utxo_with_confirmed(1, "ord", 0, 1, SCRIPT, 0, true)
            .unwrap();
        conn.execute(
            "INSERT INTO output_baskets (basketId, user_id, name, created_at, updated_at)
             VALUES (1, 1, '1sat', 0, 0)",
            [],
        )
        .unwrap();
        conn.execute("UPDATE outputs SET basket_id = 1 WHERE txid = 'ord'", []).unwrap();

        assert!(repo.get_spendable_confirmed_by_user(1).unwrap().is_empty(),
            "a correctly-basketed token is already excluded from spending");
    }

    /// The ingest path applies no value filter of any kind — a 1-satoshi output and
    /// an ordinary one are written identically and both land in the spendable pool.
    #[test]
    fn ingest_applies_no_value_filter() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);

        repo.upsert_received_utxo_with_confirmed(1, "ord", 0, 1, SCRIPT, 0, true).unwrap();
        repo.upsert_received_utxo_with_confirmed(1, "coin", 0, 50_000, SCRIPT, 0, true).unwrap();

        let mut sats: Vec<i64> = repo.get_spendable_confirmed_by_user(1).unwrap()
            .iter().map(|o| o.satoshis).collect();
        sats.sort();
        assert_eq!(sats, vec![1, 50_000],
            "both are tracked as spendable value; nothing on ingest tells them apart");
    }
}

/// `P8b-A2` — the abort branch added by Phase 8b must be **reachable**.
///
/// The fix in `handlers.rs`, `certificate_handlers.rs` and `task_consolidate_dust.rs`
/// turns a failed placeholder→txid resolution into a refusal to broadcast. That is only
/// worth anything if `update_spending_description_batch` can actually return `Err` —
/// otherwise the new branch is dead code that will never run and never be seen to run.
///
/// ⛔ This does **not** prove "nothing was broadcast". That assertion needs the running
/// handler and a live network, and is recorded as owed (`P8b-A1`).
#[cfg(test)]
mod resolution_failure_tests {
    use super::*;
    use crate::database::migrations;

    fn seed_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        migrations::create_schema_v1(&conn).unwrap();
        conn.execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at)
             VALUES (1, 'k', 'local', 0, 0)",
            [],
        )
        .unwrap();
        conn
    }

    fn reserve(conn: &Connection, txid: &str, placeholder: &str) {
        conn.execute(
            "INSERT INTO outputs (user_id, spendable, change, vout, satoshis, provided_by,
                                  purpose, type, txid, spending_description, confirmed,
                                  created_at, updated_at)
             VALUES (1, 0, 0, 0, 5000, 'you', '', 'P2PKH', ?1, ?2, 1, 0, 0)",
            rusqlite::params![txid, placeholder],
        )
        .unwrap();
    }

    /// GREEN — the happy path resolves the reservation and sets the row's spending txid.
    #[test]
    fn resolution_succeeds_and_reports_the_row_count() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        reserve(&conn, "aa", "pending-1-0");
        reserve(&conn, "bb", "pending-1-0");

        let n = repo.update_spending_description_batch("pending-1-0", "realtxid").unwrap();
        assert_eq!(n, 2, "both reserved inputs are resolved");

        let desc: String = conn
            .query_row("SELECT spending_description FROM outputs WHERE txid='aa'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(desc, "realtxid");
    }

    /// `P8b-A2` — the failure the abort branch exists for is **real and reachable**.
    ///
    /// A SQLite write failure at this exact moment is what the ticket is about (disk
    /// full, corruption, a busy timeout that outlasts the 5 s `busy_timeout`). Dropping
    /// the table reproduces the same `Err` shape without needing any of those.
    #[test]
    fn a_write_failure_surfaces_as_err_so_the_abort_branch_can_fire() {
        let conn = seed_db();
        reserve(&conn, "aa", "pending-1-0");
        conn.execute("DROP TABLE outputs", []).unwrap();

        let repo = OutputRepository::new(&conn);
        let result = repo.update_spending_description_batch("pending-1-0", "realtxid");

        assert!(result.is_err(),
            "if this ever becomes Ok, every abort added by Phase 8b is dead code");
    }

    /// ⚠️ The failure mode the ticket does **not** mention: `Ok(0)`.
    ///
    /// A resolution that matches no rows is **not** an error, so the abort branch does
    /// not fire — the wallet broadcasts having recorded nothing. Documented here rather
    /// than guarded, because `Ok(0)` is also the legitimate answer when a transaction
    /// spends only external inputs that were never in our table.
    ///
    /// ⭐ Checked and NOT a live double-spend path: `TaskSweepReservations` skips any
    /// placeholder held by a live `PENDING_TRANSACTIONS` entry
    /// (`task_sweep_reservations.rs:56-62`, *"however old the reservation looks"*), so a
    /// slow createAction→signAction cannot have its reservation swept out from under it.
    /// Closing the remaining gap needs the reserved count threaded to the resolution
    /// site — deliberately out of scope; see the phase contract §8.
    #[test]
    fn a_resolution_matching_no_rows_is_ok_zero_not_an_error() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);

        let n = repo.update_spending_description_batch("pending-nothing-here", "realtxid").unwrap();
        assert_eq!(n, 0, "no rows matched — and this is Ok, not Err");
    }

    /// The resolution only claims rows holding *its own* placeholder. A concurrent
    /// transaction's reservation must be untouched, or one send would resolve another's
    /// inputs to the wrong txid.
    #[test]
    fn resolution_claims_only_its_own_placeholder() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        reserve(&conn, "mine", "pending-1-0");
        reserve(&conn, "theirs", "pending-2-0");

        assert_eq!(repo.update_spending_description_batch("pending-1-0", "tx-a").unwrap(), 1);

        let other: String = conn
            .query_row("SELECT spending_description FROM outputs WHERE txid='theirs'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(other, "pending-2-0", "the other transaction's reservation is untouched");
    }
}
