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

use rusqlite::{Connection, OptionalExtension, Result};
use log::{info, warn, debug};
use std::time::{SystemTime, UNIX_EPOCH};

use super::Output;

/// Outcome of claiming coins for a transaction row (B5-T1-P3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Claim {
    /// The coins are now owned by the transaction (count of coins marked).
    Claimed(usize),
    /// This coin was not free: another spend holds it. The caller must roll back.
    Conflict { txid: String, vout: u32 },
}

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

/// The `len` script bytes at `off` inside the locally held raw transaction `txid`, after
/// checking those bytes are that transaction (B5-T1-P4: scripts above the inline cap live
/// only there). `None` when the parent is absent or fails the txid check.
pub fn script_in_parent(conn: &Connection, txid: &str, len: i64, off: i64) -> Option<Vec<u8>> {
    let (len, off) = (usize::try_from(len).ok()?, usize::try_from(off).ok()?);
    let raw = crate::ingest::local_raw_tx(conn, txid)?;
    if !crate::reconcile::verify_raw_txid(&raw, txid) {
        return None;
    }
    raw.get(off..off.checked_add(len)?).map(|s| s.to_vec())
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

    /// Get all spendable **money** for a user — the coins every money-selecting route may
    /// spend (S1, S2 and the fallback pools of S3, S4).
    ///
    /// B5-T1-P3: reads the money index first. A coin is offered only if it has a **free**
    /// `money_utxos` row (membership = what the index's triggers say is money); the join
    /// back to `outputs`/`transactions` applies the rules that change on the transaction
    /// (parent `unsigned/failed/nosend/nonfinal`; unconfirmed only if we built it), and
    /// re-checks `spendable = 1` so a drifted index can never offer a spent coin
    /// (`R-NODOUBLE`).
    pub fn get_spendable_by_user(&self, user_id: i64) -> Result<Vec<Output>> {
        let mut stmt = self.conn.prepare(
            "SELECT o.outputId, o.user_id, o.transaction_id, o.basket_id, o.spendable, o.change,
                    o.vout, o.satoshis, o.provided_by, o.purpose, o.type, o.output_description,
                    o.txid, o.sender_identity_key, o.derivation_prefix, o.derivation_suffix,
                    o.custom_instructions, o.spent_by, o.sequence_number, o.spending_description,
                    o.script_length, o.script_offset, o.locking_script, o.created_at, o.updated_at
             FROM money_utxos m
             JOIN outputs o ON o.outputId = m.output_id
             LEFT JOIN transactions t ON o.transaction_id = t.id
             WHERE m.user_id = ?1 AND m.reserved_by IS NULL AND o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nosend', 'nonfinal'))
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
    ///
    /// B5-T1-P3: reads the money index first, like `get_spendable_by_user`; the confirmed
    /// rule is applied by the join (S3, S5, S6 and the confirmed-first pass of S1, S4).
    pub fn get_spendable_confirmed_by_user(&self, user_id: i64) -> Result<Vec<Output>> {
        let mut stmt = self.conn.prepare(
            "SELECT o.outputId, o.user_id, o.transaction_id, o.basket_id, o.spendable, o.change,
                    o.vout, o.satoshis, o.provided_by, o.purpose, o.type, o.output_description,
                    o.txid, o.sender_identity_key, o.derivation_prefix, o.derivation_suffix,
                    o.custom_instructions, o.spent_by, o.sequence_number, o.spending_description,
                    o.script_length, o.script_offset, o.locking_script, o.created_at, o.updated_at
             FROM money_utxos m
             JOIN outputs o ON o.outputId = m.output_id
             LEFT JOIN transactions t ON o.transaction_id = t.id
             WHERE m.user_id = ?1 AND m.reserved_by IS NULL AND o.spendable = 1
               AND (t.status = 'completed' OR (o.transaction_id IS NULL AND o.confirmed = 1))
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
        //
        // B5-T1-P5 (owner decision 2(d)): the balance is the **money** — rows the
        // classification seam stamped `change = 1`. The rest of what this query used to
        // count is returned by `calculate_held` and shown under the balance, so
        // `calculate_balance + calculate_held` is exactly the pre-P5 figure: no coin
        // disappears from view.
        let (balance, _) = self.balance_split(user_id, 1)?;
        Ok(balance)
    }

    /// B5-T1-P5: the coins the balance query counted before P5 that are **not** proven
    /// money (`change = 0`) — held / Unknown, shown under the balance. `(satoshis, count)`.
    pub fn calculate_held(&self, user_id: i64) -> Result<(i64, i64)> {
        self.balance_split(user_id, 0)
    }

    fn balance_split(&self, user_id: i64, change: i64) -> Result<(i64, i64)> {
        self.conn.query_row(
            "SELECT COALESCE(SUM(o.satoshis), 0), COUNT(*)
             FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             LEFT JOIN output_baskets b ON o.basket_id = b.basketId
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nonfinal'))
               AND COALESCE(o.derivation_prefix, '') != '1-wallet-backup'
               AND (o.basket_id IS NULL OR b.name = 'default')
               AND o.change = ?2",
            rusqlite::params![user_id, change],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
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
    /// * `is_change` - Whether the caller built this as change. ⛔ B5-T1-P5: **not**
    ///   written — every row is inserted `change = 0` (not money) and the caller runs the
    ///   classification seam (`classify::after_write`), which stamps it only when the
    ///   script is the exact P2PKH template to a key this wallet signs with.
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
        _is_change: bool,
    ) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // B5-T1-P4: we built this transaction, so the script is known — record its length.
        let (locking_script, script_length, _) =
            crate::ingest::script_columns(self.conn, script_hex, crate::ingest::ScriptSource::Built);

        self.conn.execute(
            "INSERT INTO outputs (
                user_id, txid, vout, satoshis, locking_script, basket_id,
                derivation_prefix, derivation_suffix, custom_instructions, output_description,
                spendable, change, provided_by, purpose, type, created_at, updated_at, script_length
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, 0, 'you', 'change', 'P2PKH', ?11, ?12, ?13)",
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
                now,
                now,
                script_length,
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
        source: crate::ingest::ScriptSource,
    ) -> Result<usize> {
        self.upsert_received_utxo_with_confirmed(user_id, txid, vout, satoshis, script_hex, address_index, true, source)
    }

    /// Insert a received UTXO with explicit confirmed flag.
    ///
    /// `confirmed = true` for UTXOs from the confirmed bulk API.
    /// `confirmed = false` for UTXOs only seen in mempool (single-address API, height <= 0).
    ///
    /// ⛔ B5-T1-P4: `source` says how the script is known. An address-sync caller passes
    /// `Observed` (read from the verified parent) or `Unchecked` (the address-derived
    /// script, written with `script_length` NULL so it is never read as observed).
    pub fn upsert_received_utxo_with_confirmed(
        &self,
        user_id: i64,
        txid: &str,
        vout: u32,
        satoshis: i64,
        script_hex: &str,
        address_index: i32,
        confirmed: bool,
        source: crate::ingest::ScriptSource,
    ) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let (locking_script, script_length, script_offset) =
            crate::ingest::script_columns(self.conn, script_hex, source);

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
                spendable, change, provided_by, purpose, type, confirmed, created_at, updated_at,
                script_length, script_offset
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 0, 'you', 'receive', 'P2PKH', ?8, ?9, ?10, ?11, ?12)",
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
                script_length,
                script_offset,
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
        source: crate::ingest::ScriptSource,
    ) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let (locking_script, script_length, script_offset) =
            crate::ingest::script_columns(self.conn, script_hex, source);

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
                confirmed, script_length, script_offset
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 0, 'you', 'receive', 'P2PKH', ?8, ?9, ?10, ?11, ?12)",
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
                script_length,
                script_offset,
            ],
        )?;

        if rows_affected > 0 {
            info!("   ✅ Inserted recovered output {}:{} ({} sats, method={}, idx={})",
                  &txid[..std::cmp::min(16, txid.len())], vout, satoshis, derivation_method, address_index);
        }

        Ok(rows_affected)
    }

    /// The full locking script of `output`, wherever it is kept (B5-T1-P4).
    ///
    /// Inline in `locking_script` when it fits the cap; otherwise read from the raw
    /// parent at `script_offset` (toolbox `maxOutputScript` rule). `None` when neither
    /// is available, or the raw parent no longer hashes to the txid — never an empty
    /// script, so a caller cannot sign over nothing.
    pub fn locking_script_for(&self, output: &Output) -> Option<Vec<u8>> {
        if let Some(s) = output.locking_script.as_ref().filter(|s| !s.is_empty()) {
            return Some(s.clone());
        }
        script_in_parent(self.conn, output.txid.as_ref()?, output.script_length? as i64, output.script_offset? as i64)
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


    // ------------------------------------------------------------------------
    // B5-T1-P3 — coins are owned by the transaction row that spends them
    // ------------------------------------------------------------------------

    /// Claim coins for transaction row `tx_id`. Call inside the caller's SQLite transaction
    /// (a `Conflict` is only safe to act on if the caller then rolls back).
    ///
    /// Per coin: the money index first — `reserved_by = tx_id` only where it is NULL, so a
    /// coin another spend holds updates **0 rows** (`R-NORACE`) — then `outputs`
    /// (`spendable` 1 → 0, `spent_by = tx_id`). Index first, because the
    /// `money_utxos_on_output_unspendable` trigger drops only *free* index rows.
    ///
    /// `strict` (wallet-selected money): a coin with no free index row, or whose output is
    /// no longer spendable, is a `Conflict`. Not strict (dApp-named inputs, which need not
    /// be money and need not be ours): coins we do not track are skipped, as before P3.
    pub fn claim_coins(&self, tx_id: i64, outpoints: &[(String, u32)], strict: bool) -> Result<Claim> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut claimed = 0usize;
        for (txid, vout) in outpoints {
            let output_id: Option<i64> = self.conn.query_row(
                "SELECT outputId FROM outputs WHERE txid = ?1 AND vout = ?2",
                rusqlite::params![txid, *vout as i64],
                |r| r.get(0),
            ).optional()?;
            let Some(output_id) = output_id else {
                if strict {
                    return Ok(Claim::Conflict { txid: txid.clone(), vout: *vout });
                }
                continue;
            };

            let indexed = self.conn.execute(
                "UPDATE money_utxos SET reserved_by = ?1 WHERE output_id = ?2 AND reserved_by IS NULL",
                rusqlite::params![tx_id, output_id],
            )?;
            if strict && indexed == 0 {
                return Ok(Claim::Conflict { txid: txid.clone(), vout: *vout });
            }

            let marked = self.conn.execute(
                "UPDATE outputs SET spendable = 0, spent_by = ?1, spending_description = NULL, updated_at = ?2
                 WHERE outputId = ?3 AND spendable = 1",
                rusqlite::params![tx_id, now, output_id],
            )?;
            if marked == 0 {
                if strict {
                    // The index said free but the output is not spendable: drift. Refuse
                    // rather than spend a coin `outputs` says is gone (R-NODOUBLE).
                    log::error!("   ⛔ money index offered {}:{} but its output is not spendable — refusing the claim",
                        &txid[..txid.len().min(16)], vout);
                    return Ok(Claim::Conflict { txid: txid.clone(), vout: *vout });
                }
                continue;
            }
            claimed += 1;
        }
        Ok(Claim::Claimed(claimed))
    }

    /// Free every coin transaction row `tx_id` holds **in the ordinary way**: `spent_by =
    /// tx_id` and labelled either not at all (claimed, not yet signed) or with the
    /// transaction's own txid. A coin carrying any other label — `dss:` (suspected double
    /// spend), `double-spend-detected`, `phantom…`, `failed-tx-output` — is a hold its own
    /// task releases, and is left alone. The money-index rows follow by trigger.
    pub fn release_coins_of(&self, tx_id: i64) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let released = self.conn.execute(
            "UPDATE outputs SET spendable = 1, spent_by = NULL, spending_description = NULL, updated_at = ?2
             WHERE spent_by = ?1 AND spendable = 0
               AND (spending_description IS NULL
                    OR spending_description = (SELECT txid FROM transactions WHERE id = ?1))",
            rusqlite::params![tx_id, now],
        )?;
        if released > 0 {
            info!("   ♻️  Released {} coin(s) held by transaction row {}", released, tx_id);
        }
        Ok(released)
    }

    /// Label the coins transaction row `tx_id` holds with its txid, once it has one.
    /// Readers that key on `spending_description = <txid>` (double-spend marking, unfail,
    /// the failure cleanups) depend on this label. An error makes the caller refuse to
    /// broadcast (P8b: if the wallet cannot record which coins a transaction spends, it
    /// must not broadcast it).
    ///
    /// 🧪 The `P8b-A1` rig seam moved here from `update_spending_description_batch`,
    /// unchanged: both `HODOS_FAIL_SPEND_RESOLUTION` and `HODOS_DEV=1` (which
    /// `enforce_dev_safeguard` scrubs outside a build directory) are required.
    pub fn label_coins_of(&self, tx_id: i64, txid: &str) -> Result<usize> {
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
        let labelled = self.conn.execute(
            "UPDATE outputs SET spending_description = ?2, updated_at = ?3
             WHERE spent_by = ?1 AND spendable = 0 AND spending_description IS NULL",
            rusqlite::params![tx_id, txid, now],
        )?;
        Ok(labelled)
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

    /// Resolve a reservation whose outpoint has been positively observed **SPENT** on-chain.
    ///
    /// The sweeper's other verdict, and the counterpart to [`Self::restore_outpoint_if_reserved`].
    /// That one answers *"observed unspent => release"*; this one answers *"observed **spent** =>
    /// stop asking, record the spender and close the row out"*. Before this existed the second
    /// case had no owner, so a reservation whose coin was genuinely spent sat in `spendable = 0`
    /// with `spent_by = NULL` and a `pending-` placeholder **forever**, declining every sweep --
    /// correctly, but with nothing ever reconciling it
    /// (`TICKET_reservation_can_be_held_indefinitely.md`).
    ///
    /// # This statement CANNOT set `spendable = 1`, by design
    ///
    /// It never writes the `spendable` column at all. The row is already `0` and must stay `0`:
    /// handing a spent output back to the selector is exactly the `P0.7` double-spend path the
    /// conservative sweeper was written to close. The absence of `spendable` from the SET clause
    /// is the load-bearing part of this method, and is what the negative control asserts.
    ///
    /// # `spent_by` is an FK to `transactions.id`, not a txid
    ///
    /// So it can only be filled when the spending transaction is already a row in our own
    /// `transactions` table. When it is not, `spent_by` stays `NULL` and the real txid is
    /// recorded in `spending_description` instead -- which is still a resolved row (the
    /// `pending-` placeholder is gone and a real txid is present), **not** an unreconciled one.
    /// Do not read a `NULL` `spent_by` here as "never resolved". Same convention as
    /// [`Self::mark_spent`].
    ///
    /// [`Self::mark_spent`] cannot be used for this: its predicate ends `AND spendable = 1`,
    /// so on a reserved row it matches nothing and returns `Ok(0)` -- a success value for work it
    /// did not do.
    ///
    /// Scoped to `placeholder` for the same `R-NORACE` reason as the release path: between
    /// listing a stale reservation and resolving it, a concurrent `createAction` may have
    /// re-reserved the row under a new placeholder. Returns rows affected (0 or 1).
    pub fn resolve_reserved_outpoint_as_spent(
        &self,
        txid: &str,
        vout: u32,
        placeholder: &str,
        spending_txid: &str,
    ) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Best-effort: absent from `transactions` simply leaves the FK NULL (see docstring).
        let spent_by: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM transactions WHERE txid = ?1",
                rusqlite::params![spending_txid],
                |row| row.get(0),
            )
            .ok();

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET spent_by = ?1, spending_description = ?2, updated_at = ?3
             WHERE txid = ?4 AND vout = ?5 AND spending_description = ?6 AND spendable = 0",
            rusqlite::params![spent_by, spending_txid, now, txid, vout, placeholder],
        )?;

        if rows_affected > 0 {
            info!(
                "   🔗 Resolved spent reservation {}:{} → spent by {} (spent_by={:?})",
                &txid[..std::cmp::min(16, txid.len())],
                vout,
                &spending_txid[..std::cmp::min(16, spending_txid.len())],
                spent_by
            );
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
    ///
    /// B5-T1-P3: also restores coins owned by that transaction's row and not yet labelled
    /// (claimed, never signed). Coins carrying another label (`dss:` …) are holds their own
    /// task releases and are never matched.
    pub fn restore_spent_by_txid(&self, spending_txid: &str) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE outputs SET spendable = 1, spent_by = NULL, spending_description = NULL, updated_at = ?1
             WHERE spendable = 0
               AND (spending_description = ?2
                    OR (spending_description IS NULL
                        AND spent_by = (SELECT id FROM transactions WHERE txid = ?2)))",
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
        migrations::install_money_index(&conn).unwrap();
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
        migrations::install_money_index(&conn).unwrap();
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
        migrations::install_money_index(&conn).unwrap();
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

        // ordinary money, default pool — stamped as the classifier would (B5-T1-P5)
        repo.upsert_received_utxo_with_confirmed(1, "ord", 0, 50_000, SCRIPT, 0, true, crate::ingest::ScriptSource::Unchecked)
            .unwrap();
        conn.execute("UPDATE outputs SET change = 1 WHERE txid = 'ord'", []).unwrap();
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
        assert_eq!(repo.calculate_held(1).unwrap(), (0, 0),
                   "B5-T1-P5: nor the held line — a filed carrier is T2's, not 'being held'");
    }

    /// Before B5-T1-P5 this test pinned the defect: a 1-satoshi payment arriving at a
    /// receive address became a spendable default-pool row ("the actual defect … the
    /// real fix is classification on ingest"). P5 is that fix: ingest writes the row
    /// **unstamped** (`change = 0`), so no spend path is offered it, and it is shown as
    /// held instead of vanishing.
    #[test]
    fn incoming_one_sat_payment_is_held_not_spendable() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);

        // Exactly what address sync does: address_index 0, confirmed.
        let inserted = repo
            .upsert_received_utxo_with_confirmed(1, "ord", 0, 1, SCRIPT, 0, true, crate::ingest::ScriptSource::Unchecked)
            .unwrap();
        assert_eq!(inserted, 1);
        assert!(repo.get_spendable_confirmed_by_user(1).unwrap().is_empty()
                && repo.get_spendable_by_user(1).unwrap().is_empty(),
            "an unclassified row is offered to no spend path");
        assert_eq!(repo.calculate_balance(1).unwrap(), 0);
        assert_eq!(repo.calculate_held(1).unwrap(), (1, 1), "…and it is shown as held, not lost");
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

        repo.upsert_received_utxo_with_confirmed(1, "ord", 0, 1, SCRIPT, 0, true, crate::ingest::ScriptSource::Unchecked)
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

    /// The ingest path applies no value filter — and since B5-T1-P5 it stamps nothing
    /// either: a 1-satoshi output and an ordinary one are both written unstamped, both
    /// held, neither in the spendable pool until the classifier has looked at them.
    #[test]
    fn ingest_applies_no_value_filter_and_stamps_nothing() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);

        repo.upsert_received_utxo_with_confirmed(1, "ord", 0, 1, SCRIPT, 0, true, crate::ingest::ScriptSource::Unchecked).unwrap();
        repo.upsert_received_utxo_with_confirmed(1, "coin", 0, 50_000, SCRIPT, 0, true, crate::ingest::ScriptSource::Unchecked).unwrap();

        assert!(repo.get_spendable_confirmed_by_user(1).unwrap().is_empty());
        assert_eq!(repo.calculate_held(1).unwrap(), (50_001, 2));
    }
}

/// `P8b-A2` — the abort branch added by Phase 8b must be **reachable**.
///
/// B5-T1-P3 ported these from `update_spending_description_batch` (deleted) to
/// `label_coins_of`, its replacement: the refusal to broadcast fires when labelling a
/// transaction's coins fails, so that `Err` must be real and reachable.
///
/// ⛔ This does **not** prove "nothing was broadcast". That assertion needs the running
/// handler (`P3-A8`).
#[cfg(test)]
mod resolution_failure_tests {
    use super::*;
    use crate::database::migrations;

    fn seed_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        migrations::create_schema_v1(&conn).unwrap();
        migrations::install_money_index(&conn).unwrap();
        conn.execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at)
             VALUES (1, 'k', 'local', 0, 0)",
            [],
        )
        .unwrap();
        conn
    }

    /// A transaction row (the owner) — returns its id.
    fn row(conn: &Connection, reference: &str) -> i64 {
        conn.execute(
            "INSERT INTO transactions (txid, reference_number, raw_tx, status, is_outgoing, satoshis, created_at, updated_at)
             VALUES (?1, ?1, '', 'unsigned', 1, 0, 0, 0)",
            [reference],
        ).unwrap();
        conn.last_insert_rowid()
    }

    /// A coin claimed by row `owner`, not yet labelled.
    fn reserve(conn: &Connection, txid: &str, owner: i64) {
        conn.execute(
            "INSERT INTO outputs (user_id, spendable, change, vout, satoshis, provided_by,
                                  purpose, type, txid, spent_by, confirmed,
                                  created_at, updated_at)
             VALUES (1, 0, 0, 0, 5000, 'you', '', 'P2PKH', ?1, ?2, 1, 0, 0)",
            rusqlite::params![txid, owner],
        )
        .unwrap();
    }

    /// GREEN — labelling sets each owned coin's spending txid.
    #[test]
    fn resolution_succeeds_and_reports_the_row_count() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        let me = row(&conn, "ref-1");
        reserve(&conn, "aa", me);
        reserve(&conn, "bb", me);

        let n = repo.label_coins_of(me, "realtxid").unwrap();
        assert_eq!(n, 2, "both reserved inputs are labelled");

        let desc: String = conn
            .query_row("SELECT spending_description FROM outputs WHERE txid='aa'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(desc, "realtxid");
    }

    /// `P8b-A2` — the failure the abort branch exists for is **real and reachable**.
    /// Dropping the table reproduces a SQLite write failure's `Err` shape.
    #[test]
    fn a_write_failure_surfaces_as_err_so_the_abort_branch_can_fire() {
        let conn = seed_db();
        let me = row(&conn, "ref-1");
        reserve(&conn, "aa", me);
        conn.execute("DROP TABLE money_utxos", []).unwrap();
        conn.execute("DROP TABLE outputs", []).unwrap();

        let repo = OutputRepository::new(&conn);
        let result = repo.label_coins_of(me, "realtxid");

        assert!(result.is_err(),
            "if this ever becomes Ok, every abort added by Phase 8b is dead code");
    }

    /// ⚠️ `Ok(0)` is not an error: a transaction that spends only inputs we do not track
    /// owns no coins. B5-T1-P3 removed the gap this used to document — a slow
    /// createAction → signAction can no longer have its reservation swept, because the
    /// coins are owned by the row, not by a string a sweeper matches.
    #[test]
    fn a_resolution_matching_no_rows_is_ok_zero_not_an_error() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        let me = row(&conn, "ref-1");

        let n = repo.label_coins_of(me, "realtxid").unwrap();
        assert_eq!(n, 0, "no rows matched — and this is Ok, not Err");
    }

    /// Labelling only touches coins *its own* row owns. Another transaction's coins are
    /// untouched, or one send would label another's inputs with the wrong txid.
    #[test]
    fn resolution_claims_only_its_own_placeholder() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        let me = row(&conn, "ref-1");
        let them = row(&conn, "ref-2");
        reserve(&conn, "mine", me);
        reserve(&conn, "theirs", them);

        assert_eq!(repo.label_coins_of(me, "tx-a").unwrap(), 1);

        let other: Option<String> = conn
            .query_row("SELECT spending_description FROM outputs WHERE txid='theirs'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(other, None, "the other transaction's coin is untouched");
    }
}

/// `TICKET_reservation_can_be_held_indefinitely.md` -- the sweeper's SECOND verdict.
///
/// The load-bearing property is a NEGATIVE one: resolving a reservation whose outpoint is
/// spent must never hand that outpoint back to the selector. `P0.7` is the double-spend that
/// caused, so the first test below is written to go RED the moment `spendable = 1` appears in
/// the statement, and has been observed doing exactly that.
#[cfg(test)]
mod spent_reservation_resolution_tests {
    use super::*;
    use crate::database::migrations;

    fn seed_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        migrations::create_schema_v1(&conn).unwrap();
        migrations::install_money_index(&conn).unwrap();
        conn.execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at)
             VALUES (1, 'k', 'local', 0, 0)",
            [],
        )
        .unwrap();
        conn
    }

    /// A reserved row: `spendable = 0`, holding a `pending-` placeholder, spender unknown.
    fn reserve(conn: &Connection, txid: &str, placeholder: &str) {
        conn.execute(
            "INSERT INTO outputs (user_id, spendable, change, vout, satoshis, provided_by,
                                  purpose, type, txid, spending_description, confirmed,
                                  created_at, updated_at)
             VALUES (1, 0, 0, 0, 1419268, 'you', '', 'P2PKH', ?1, ?2, 1, 0, 0)",
            rusqlite::params![txid, placeholder],
        )
        .unwrap();
    }

    fn insert_tx(conn: &Connection, txid: &str) -> i64 {
        conn.execute(
            "INSERT INTO transactions (user_id, txid, reference_number, status, is_outgoing,
                                       satoshis, created_at, updated_at)
             VALUES (1, ?1, ?1, 'completed', 1, 5000, 0, 0)",
            rusqlite::params![txid],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn spendable_of(conn: &Connection, txid: &str) -> i64 {
        conn.query_row(
            "SELECT spendable FROM outputs WHERE txid = ?1",
            rusqlite::params![txid],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// NEGATIVE CONTROL. Resolving a spent reservation records the spender and closes the row
    /// out -- but the coin is SPENT, so it must stay unspendable. This assertion is the whole
    /// reason the method omits `spendable` from its SET clause.
    #[test]
    fn resolving_a_spent_reservation_never_makes_it_spendable() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        reserve(&conn, "aa", "pending-1789591461036-0");

        let n = repo
            .resolve_reserved_outpoint_as_spent("aa", 0, "pending-1789591461036-0", "spender")
            .unwrap();
        assert_eq!(n, 1, "the reservation is resolved");

        assert_eq!(
            spendable_of(&conn, "aa"),
            0,
            "a SPENT outpoint must never be handed back to the selector (P0.7)"
        );

        let desc: String = conn
            .query_row("SELECT spending_description FROM outputs WHERE txid='aa'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(desc, "spender", "the pending- placeholder is replaced by the real txid");
    }

    /// The row is only reachable while it still holds the placeholder we listed it under.
    /// A concurrent `createAction` that re-reserved it under a new one must be left alone
    /// (`R-NORACE`), exactly as on the release path.
    #[test]
    fn resolution_is_scoped_to_the_placeholder_it_was_listed_under() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        reserve(&conn, "aa", "pending-NEW-0");

        let n = repo
            .resolve_reserved_outpoint_as_spent("aa", 0, "pending-OLD-0", "spender")
            .unwrap();
        assert_eq!(n, 0, "a row re-reserved under a new placeholder is not touched");

        let desc: String = conn
            .query_row("SELECT spending_description FROM outputs WHERE txid='aa'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(desc, "pending-NEW-0", "the live reservation survives intact");
    }

    /// `spent_by` is an FK to `transactions.id`. When the spender is a transaction we already
    /// know about -- the M4 case in the ticket -- the row is linked to it.
    #[test]
    fn resolution_links_the_spender_when_the_transaction_is_ours() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        reserve(&conn, "aa", "pending-1-0");
        let tx_id = insert_tx(&conn, "spender");

        repo.resolve_reserved_outpoint_as_spent("aa", 0, "pending-1-0", "spender")
            .unwrap();

        let spent_by: Option<i64> = conn
            .query_row("SELECT spent_by FROM outputs WHERE txid='aa'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(spent_by, Some(tx_id), "the row is linked to the spending transaction");
    }

    /// And when the spender is NOT ours, `spent_by` stays NULL while the real txid still lands
    /// in `spending_description`. That is a RESOLVED row, not an unreconciled one -- the
    /// distinction the docstring warns about, pinned here so nobody "fixes" it later.
    #[test]
    fn resolution_leaves_spent_by_null_when_the_spender_is_unknown_to_us() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        reserve(&conn, "aa", "pending-1-0");

        repo.resolve_reserved_outpoint_as_spent("aa", 0, "pending-1-0", "a_stranger_tx")
            .unwrap();

        let spent_by: Option<i64> = conn
            .query_row("SELECT spent_by FROM outputs WHERE txid='aa'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(spent_by, None, "no FK to point at -- left NULL, by design");

        let desc: String = conn
            .query_row("SELECT spending_description FROM outputs WHERE txid='aa'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(desc, "a_stranger_tx", "but the row is still resolved: real txid, no placeholder");
        assert_eq!(spendable_of(&conn, "aa"), 0, "and still not spendable");
    }

    /// Why `mark_spent` could NOT be reused: its predicate ends `AND spendable = 1`, so on a
    /// reserved row it matches nothing and returns `Ok(0)` -- a SUCCESS value for work it did
    /// not do. Pinned so that a future change to either method surfaces here rather than
    /// silently doing nothing on the money path.
    #[test]
    fn mark_spent_is_a_silent_noop_on_a_reserved_row() {
        let conn = seed_db();
        let repo = OutputRepository::new(&conn);
        reserve(&conn, "aa", "pending-1-0");

        let n = repo.mark_spent("aa", 0, "spender").unwrap();
        assert_eq!(n, 0, "mark_spent cannot see a reserved row -- this is why the sibling exists");

        let desc: String = conn
            .query_row("SELECT spending_description FROM outputs WHERE txid='aa'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(desc, "pending-1-0", "and it changed nothing while reporting Ok");
    }
}

// ============================================================================
// B5-T1-P3 — the money index and reservation by transaction row.
// Contract: development-docs/0.4.0-beta.7/track-1-money-path/phase-P3-reservations-and-money-index/
// Each test names the contract row it serves; its RED is recorded in the evidence table.
// ============================================================================
#[cfg(test)]
mod p3_money_index_tests {
    use super::*;
    use crate::database::migrations;
    use std::collections::BTreeSet;

    const STATUSES: [&str; 7] = ["unsigned", "failed", "nosend", "nonfinal", "completed", "unproven", "sending"];

    fn seed_conn(conn: &Connection) {
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        migrations::create_schema_v1(conn).unwrap();
        migrations::install_money_index(conn).unwrap();
        conn.execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at) VALUES (1, 'k', 'local', 0, 0)",
            [],
        ).unwrap();
        conn.execute("INSERT INTO output_baskets (basketId, user_id, name, created_at, updated_at) VALUES (1, 1, 'default', 0, 0)", []).unwrap();
        conn.execute("INSERT INTO output_baskets (basketId, user_id, name, created_at, updated_at) VALUES (2, 1, '1sat', 0, 0)", []).unwrap();
    }

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        seed_conn(&conn);
        conn
    }

    fn tx_row(conn: &Connection, reference: &str, status: &str) -> i64 {
        conn.execute(
            "INSERT INTO transactions (txid, reference_number, raw_tx, status, is_outgoing, satoshis, created_at, updated_at)
             VALUES (?1, ?1, '', ?2, 1, 0, 0, 0)",
            rusqlite::params![reference, status],
        ).unwrap();
        conn.last_insert_rowid()
    }

    /// A coin the classifier has stamped money (`change = 1`, B5-T1-P5) — the fixtures
    /// below test P3's mechanics, which act on stamped coins.
    #[allow(clippy::too_many_arguments)]
    fn out(conn: &Connection, txid: &str, sats: i64, spendable: bool, deriv: bool,
           basket: Option<i64>, parent: Option<i64>, confirmed: bool) -> i64 {
        out_c(conn, txid, sats, spendable, deriv, basket, parent, confirmed, true)
    }

    #[allow(clippy::too_many_arguments)]
    fn out_c(conn: &Connection, txid: &str, sats: i64, spendable: bool, deriv: bool,
             basket: Option<i64>, parent: Option<i64>, confirmed: bool, stamped: bool) -> i64 {
        conn.execute(
            "INSERT INTO outputs (user_id, transaction_id, basket_id, spendable, change, vout, satoshis, provided_by,
                                  purpose, type, txid, derivation_prefix, confirmed, created_at, updated_at)
             VALUES (1, ?1, ?2, ?3, ?8, 0, ?4, 'you', '', 'P2PKH', ?5, ?6, ?7, 0, 0)",
            rusqlite::params![parent, basket, spendable as i64, sats, txid,
                              if deriv { Some("2-receive address") } else { None }, confirmed as i64,
                              stamped as i64],
        ).unwrap();
        conn.last_insert_rowid()
    }

    fn ids(v: Vec<Output>) -> BTreeSet<i64> {
        v.into_iter().map(|o| o.output_id.unwrap()).collect()
    }

    /// The pre-P3 selectors, verbatim (origin/0.4.0 at 02fa1c6) — the parity reference —
    /// plus the one term B5-T1-P5 adds on purpose: `AND o.change = 1` (owner decision 2).
    fn legacy(conn: &Connection, confirmed_only: bool) -> BTreeSet<i64> {
        let sql = if confirmed_only {
            "SELECT o.outputId FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             LEFT JOIN output_baskets b ON o.basket_id = b.basketId
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status = 'completed' OR (o.transaction_id IS NULL AND o.confirmed = 1))
               AND o.derivation_prefix IS NOT NULL
               AND (o.basket_id IS NULL OR b.name = 'default')
               AND o.change = 1"
        } else {
            "SELECT o.outputId FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             LEFT JOIN output_baskets b ON o.basket_id = b.basketId
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nosend', 'nonfinal'))
               AND o.derivation_prefix IS NOT NULL
               AND (o.basket_id IS NULL OR b.name = 'default')
               AND (o.transaction_id IS NOT NULL OR o.confirmed = 1)
               AND o.change = 1"
        };
        let mut stmt = conn.prepare(sql).unwrap();
        let rows = stmt.query_map([1i64], |r| r.get::<_, i64>(0)).unwrap();
        rows.map(|r| r.unwrap()).collect()
    }

    /// Every combination of the predicate's inputs: stamped × spendable × derivation ×
    /// basket × parent (none + confirmed 0/1, or a parent in each status).
    /// 2·2·2·3·(2+7) = 216 rows (B5-T1-P5 added the stamp dimension).
    fn every_combination(conn: &Connection) {
        let parents: Vec<(&str, i64)> = STATUSES.iter().map(|s| (*s, tx_row(conn, &format!("parent-{}", s), s))).collect();
        let mut n = 0;
        for stamped in [false, true] {
        for spendable in [false, true] {
            for deriv in [false, true] {
                for basket in [None, Some(1), Some(2)] {
                    for confirmed in [false, true] {
                        n += 1;
                        out_c(conn, &format!("c{}", n), 1000 + n, spendable, deriv, basket, None, confirmed, stamped);
                    }
                    for (_, pid) in &parents {
                        n += 1;
                        out_c(conn, &format!("c{}", n), 1000 + n, spendable, deriv, basket, Some(*pid), false, stamped);
                    }
                }
            }
        }
        }
    }

    /// `P3-A1` (T1 half) — parity: on every combination, each index-reading selector offers
    /// exactly the outputs the pre-P3 selector offered, by id; after the triggers AND after
    /// a rebuild. Two comparisons, because the two selectors have different predicates.
    #[test]
    fn p3_a1_index_selectors_match_the_pre_p3_selectors_on_every_combination() {
        let conn = db();
        every_combination(&conn);
        let repo = OutputRepository::new(&conn);
        let check = |label: &str| {
            let new_all = ids(repo.get_spendable_by_user(1).unwrap());
            let new_conf = ids(repo.get_spendable_confirmed_by_user(1).unwrap());
            assert_eq!(new_all, legacy(&conn, false), "{}: get_spendable_by_user differs from the pre-P3 selector", label);
            assert_eq!(new_conf, legacy(&conn, true), "{}: get_spendable_confirmed_by_user differs from the pre-P3 selector", label);
            assert!(!new_all.is_empty() && !new_conf.is_empty() && new_conf != new_all,
                "{}: the fixture must exercise both predicates non-trivially", label);
        };
        check("after triggers");
        let r = migrations::rebuild_money_index(&conn).unwrap();
        assert_eq!((r.added, r.removed), (0, 0), "the triggers left nothing for the rebuild to fix: {:?}", r);
        check("after rebuild");
    }

    /// `P3-A1` RED, built in: the parity instrument CAN fail, and names the outpoint.
    /// With the trigger that removes a non-money row disabled, moving one coin into a
    /// token basket leaves it in the index — the diff is exactly that coin. (A comparison
    /// that cannot go red here would pass with the index mirroring `outputs` blindly.)
    #[test]
    fn p3_a1_red_parity_names_the_outpoint_when_the_index_drifts() {
        let conn = db();
        every_combination(&conn);
        let repo = OutputRepository::new(&conn);
        let victim: i64 = conn.query_row(
            "SELECT output_id FROM money_utxos ORDER BY satoshis DESC LIMIT 1", [], |r| r.get(0)).unwrap();
        conn.execute("DROP TRIGGER money_utxos_on_output_not_money", []).unwrap();
        conn.execute("UPDATE outputs SET basket_id = 2 WHERE outputId = ?1", [victim]).unwrap();
        let new_all = ids(repo.get_spendable_by_user(1).unwrap());
        let diff: Vec<i64> = new_all.symmetric_difference(&legacy(&conn, false)).copied().collect();
        assert_eq!(diff, vec![victim], "the diff must name exactly the drifted outpoint");
    }

    /// `P3-A2` (T1 half, the shared read) — a money output absent from the index is never
    /// offered, by either selector. (Route-by-route: the handler tests.)
    #[test]
    fn p3_a2_an_output_missing_from_the_index_is_never_offered() {
        let conn = db();
        let big = out(&conn, "big", 900_000, true, true, None, None, true);
        let small = out(&conn, "small", 5_000, true, true, None, None, true);
        conn.execute("DELETE FROM money_utxos WHERE output_id = ?1", [big]).unwrap();
        let repo = OutputRepository::new(&conn);
        assert_eq!(ids(repo.get_spendable_by_user(1).unwrap()), BTreeSet::from([small]));
        assert_eq!(ids(repo.get_spendable_confirmed_by_user(1).unwrap()), BTreeSet::from([small]));
        // …and `outputs` still says it is spendable: the index is what refused it.
        assert!(legacy(&conn, false).contains(&big));
        // Second layer: even a route that selected it some other way cannot claim it.
        let t = tx_row(&conn, "T", "unsigned");
        assert_eq!(OutputRepository::new(&conn).claim_coins(t, &[("big".into(), 0)], true).unwrap(),
                   Claim::Conflict { txid: "big".into(), vout: 0 });
    }

    fn index_row(conn: &Connection, output_id: i64) -> Option<Option<i64>> {
        conn.query_row("SELECT reserved_by FROM money_utxos WHERE output_id = ?1", [output_id], |r| r.get(0)).ok()
    }

    /// The triggers' rules (install_money_index doc 1–5), one step at a time.
    #[test]
    fn p3_triggers_keep_the_index_in_step_with_outputs() {
        let conn = db();
        let c = out(&conn, "coin", 5_000, true, true, None, None, true);
        assert_eq!(index_row(&conn, c), Some(None), "rule 1: spendable money ⇒ a free row");

        conn.execute("UPDATE outputs SET spendable = 0 WHERE outputId = ?1", [c]).unwrap();
        assert_eq!(index_row(&conn, c), None, "rule 2: not spendable ⇒ the free row goes");
        conn.execute("UPDATE outputs SET spendable = 1 WHERE outputId = ?1", [c]).unwrap();
        assert_eq!(index_row(&conn, c), Some(None), "rule 1: restored ⇒ free again");

        conn.execute("UPDATE outputs SET basket_id = 2 WHERE outputId = ?1", [c]).unwrap();
        assert_eq!(index_row(&conn, c), None, "rule 3: filed in a token basket ⇒ not money");
        conn.execute("UPDATE outputs SET basket_id = 1 WHERE outputId = ?1", [c]).unwrap();
        assert_eq!(index_row(&conn, c), Some(None), "back in the default basket ⇒ money");
        conn.execute("UPDATE outputs SET derivation_prefix = NULL WHERE outputId = ?1", [c]).unwrap();
        assert_eq!(index_row(&conn, c), None, "rule 3: no derivation ⇒ not money");
        conn.execute("UPDATE outputs SET derivation_prefix = '2-receive address' WHERE outputId = ?1", [c]).unwrap();

        let t = tx_row(&conn, "spend", "unsigned");
        let repo = OutputRepository::new(&conn);
        assert_eq!(repo.claim_coins(t, &[("coin".into(), 0)], true).unwrap(), Claim::Claimed(1));
        assert_eq!(index_row(&conn, c), Some(Some(t)), "rule 2: a claimed row survives spendable → 0");

        conn.execute("UPDATE transactions SET status = 'failed' WHERE id = ?1", [t]).unwrap();
        assert_eq!(index_row(&conn, c), None, "rule 4: a failed reserver drops its rows");
        repo.restore_spent_by_txid("spend").unwrap();
        assert_eq!(index_row(&conn, c), Some(None), "rule 4 + 1: inputs restored ⇒ free again");

        conn.execute("DELETE FROM outputs WHERE outputId = ?1", [c]).unwrap();
        assert_eq!(index_row(&conn, c), None, "rule 5: deleted output ⇒ no row");
    }

    /// A restore by ANY path — including an older binary that knows nothing of the index —
    /// frees a reserved row (rule 1). Before rule 1, a reserved row whose coin was restored
    /// behind its back would hide money until the next restart.
    #[test]
    fn p3_a_restore_behind_the_index_frees_the_reservation() {
        let conn = db();
        let c = out(&conn, "coin", 5_000, true, true, None, None, true);
        let t = tx_row(&conn, "spend", "unsigned");
        OutputRepository::new(&conn).claim_coins(t, &[("coin".into(), 0)], true).unwrap();
        // what a beta.4 binary's restore looks like
        conn.execute("UPDATE outputs SET spendable = 1, spent_by = NULL, spending_description = NULL WHERE outputId = ?1", [c]).unwrap();
        assert_eq!(index_row(&conn, c), Some(None));
        assert!(ids(OutputRepository::new(&conn).get_spendable_by_user(1).unwrap()).contains(&c));
    }

    /// `P3-A3` (T1, sequential half): the second claim takes 0 rows and changes nothing.
    #[test]
    fn p3_a3_a_second_claim_of_the_same_coin_is_a_conflict_and_changes_nothing() {
        let conn = db();
        let c = out(&conn, "coin", 5_000, true, true, None, None, true);
        let a = tx_row(&conn, "a", "unsigned");
        let b = tx_row(&conn, "b", "unsigned");
        let repo = OutputRepository::new(&conn);
        assert_eq!(repo.claim_coins(a, &[("coin".into(), 0)], true).unwrap(), Claim::Claimed(1));
        assert_eq!(repo.claim_coins(b, &[("coin".into(), 0)], true).unwrap(),
                   Claim::Conflict { txid: "coin".into(), vout: 0 });
        let spent_by: i64 = conn.query_row("SELECT spent_by FROM outputs WHERE outputId = ?1", [c], |r| r.get(0)).unwrap();
        assert_eq!((spent_by, index_row(&conn, c)), (a, Some(Some(a))), "the coin stays A's");
    }

    /// `P3-A3` (T1, the overlap). Production has ONE connection behind a mutex, so two
    /// spends never overlap inside SQLite — they overlap *logically*: both select the same
    /// coin before either claims it. Two threads on a shared connection: each selects
    /// (the barrier proves both selections saw the coin free), then each claims. Exactly
    /// one wins; the other takes 0 rows. The sequential test cannot tell a closed race
    /// from two calls that never overlapped — this one can.
    /// RED (scratch): drop BOTH guards in `claim_coins` (`reserved_by IS NULL` on the
    /// index, `spendable = 1` on outputs) ⇒ both report `Claimed(1)`. Each guard alone
    /// keeps this green — they are independent.
    #[test]
    fn p3_a3_overlapping_claims_one_wins() {
        use std::sync::{Arc, Barrier, Mutex};
        let conn = db();
        let c = out(&conn, "coin", 5_000, true, true, None, None, true);
        let a = tx_row(&conn, "a", "unsigned");
        let b = tx_row(&conn, "b", "unsigned");
        let shared = Arc::new(Mutex::new(conn));
        let both_selected = Arc::new(Barrier::new(2));

        let spend = |row: i64| {
            let (db, barrier) = (shared.clone(), both_selected.clone());
            std::thread::spawn(move || {
                let saw = {
                    let conn = db.lock().unwrap();
                    ids(OutputRepository::new(&conn).get_spendable_by_user(1).unwrap()).contains(&c)
                };
                barrier.wait(); // neither claims until both have selected
                let conn = db.lock().unwrap();
                let txn = conn.unchecked_transaction().unwrap();
                let got = OutputRepository::new(&txn).claim_coins(row, &[("coin".into(), 0)], true).unwrap();
                txn.commit().unwrap();
                (saw, got)
            })
        };
        let (ta, tb) = (spend(a), spend(b));
        let (saw_a, got_a) = ta.join().unwrap();
        let (saw_b, got_b) = tb.join().unwrap();
        assert!(saw_a && saw_b, "SUBJECT: both selections saw the coin free — the calls overlapped");
        let wins = [&got_a, &got_b].iter().filter(|g| matches!(g, Claim::Claimed(1))).count();
        let conflicts = [&got_a, &got_b].iter().filter(|g| matches!(g, Claim::Conflict { .. })).count();
        assert_eq!((wins, conflicts), (1, 1), "exactly one claim wins: {:?} / {:?}", got_a, got_b);
    }

    /// `P3-A6` (T1): a failed transaction frees EXACTLY its own coins — A's freed, B's kept.
    /// REDs (scratch): release `WHERE reserved_by IS NOT NULL` / by outpoint ⇒ B's freed
    /// too; release keyed on `A + 1` ⇒ A's stay held. Both halves asserted here.
    #[test]
    fn p3_a6_release_frees_exactly_the_failed_transactions_coins() {
        let conn = db();
        let a1 = out(&conn, "a1", 5_000, true, true, None, None, true);
        let a2 = out(&conn, "a2", 6_000, true, true, None, None, true);
        let b1 = out(&conn, "b1", 7_000, true, true, None, None, true);
        let a = tx_row(&conn, "A", "unsigned");
        let b = tx_row(&conn, "B", "unsigned");
        let repo = OutputRepository::new(&conn);
        repo.claim_coins(a, &[("a1".into(), 0), ("a2".into(), 0)], true).unwrap();
        repo.claim_coins(b, &[("b1".into(), 0)], true).unwrap();

        assert_eq!(repo.release_coins_of(a).unwrap(), 2);
        let state = |id: i64| -> (i64, Option<i64>, Option<Option<i64>>) {
            let (sp, sb) = conn.query_row("SELECT spendable, spent_by FROM outputs WHERE outputId = ?1", [id],
                |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
            (sp, sb, index_row(&conn, id))
        };
        assert_eq!(state(a1), (1, None, Some(None)), "A's first coin is free");
        assert_eq!(state(a2), (1, None, Some(None)), "A's second coin is free");
        assert_eq!(state(b1), (0, Some(b), Some(Some(b))), "B's coin is still B's");
    }

    /// A coin held under a special label — suspected double spend — is never released
    /// by its transaction's release (`R-NODOUBLE`): its own task decides.
    #[test]
    fn p3_release_never_frees_a_suspected_double_spend_hold() {
        let conn = db();
        let c = out(&conn, "coin", 5_000, true, true, None, None, true);
        let t = tx_row(&conn, "T", "unsigned");
        let repo = OutputRepository::new(&conn);
        repo.claim_coins(t, &[("coin".into(), 0)], true).unwrap();
        conn.execute("UPDATE outputs SET spending_description = 'dss:T' WHERE outputId = ?1", [c]).unwrap();
        assert_eq!(repo.release_coins_of(t).unwrap(), 0);
        assert_eq!(repo.restore_spent_by_txid("T").unwrap(), 0);
        let sp: i64 = conn.query_row("SELECT spendable FROM outputs WHERE outputId = ?1", [c], |r| r.get(0)).unwrap();
        assert_eq!(sp, 0);
    }

    /// Strict claim refuses a coin the index says is free but `outputs` says is gone
    /// (drift) — it never claims a spent coin (`R-NODOUBLE`), and rolls nothing forward.
    #[test]
    fn p3_strict_claim_refuses_a_drifted_index_row() {
        let conn = db();
        let c = out(&conn, "coin", 5_000, true, true, None, None, true);
        conn.execute("DROP TRIGGER money_utxos_on_output_unspendable", []).unwrap();
        conn.execute("UPDATE outputs SET spendable = 0 WHERE outputId = ?1", [c]).unwrap();
        assert_eq!(index_row(&conn, c), Some(None), "drift seeded: free index row, spent output");
        let t = tx_row(&conn, "T", "unsigned");
        assert_eq!(OutputRepository::new(&conn).claim_coins(t, &[("coin".into(), 0)], true).unwrap(),
                   Claim::Conflict { txid: "coin".into(), vout: 0 });
        assert!(OutputRepository::new(&conn).get_spendable_by_user(1).unwrap().is_empty(),
                "and the selector's spendable = 1 check never offered it");
    }

    /// `P3-A11` (T1 half): the rebuild repairs drift in both directions and reports it.
    #[test]
    fn p3_a11_rebuild_repairs_drift_both_ways_and_reports_it() {
        let conn = db();
        let spent = out(&conn, "spent", 5_000, true, true, None, None, true);
        let missing = out(&conn, "missing", 6_000, true, true, None, None, true);
        let done = tx_row(&conn, "done", "unsigned");
        let held = out(&conn, "held", 7_000, true, true, None, None, true);
        OutputRepository::new(&conn).claim_coins(done, &[("held".into(), 0)], true).unwrap();
        // drift, as an older binary with no triggers would leave it
        for t in ["money_utxos_on_output_unspendable", "money_utxos_on_tx_settled"] {
            conn.execute(&format!("DROP TRIGGER {}", t), []).unwrap();
        }
        conn.execute("UPDATE outputs SET spendable = 0 WHERE outputId = ?1", [spent]).unwrap(); // spent elsewhere
        conn.execute("DELETE FROM money_utxos WHERE output_id = ?1", [missing]).unwrap();       // received, unindexed
        conn.execute("UPDATE transactions SET status = 'completed' WHERE id = ?1", [done]).unwrap();

        let r = migrations::rebuild_money_index(&conn).unwrap();
        assert_eq!((r.added, r.removed), (1, 1), "{:?}", r);
        assert_eq!(index_row(&conn, spent), None, "a spent coin leaves the index");
        assert_eq!(index_row(&conn, missing), Some(None), "a received coin enters it");
        assert_eq!(index_row(&conn, held), None, "a completed reserver's row is pruned");
    }
}
