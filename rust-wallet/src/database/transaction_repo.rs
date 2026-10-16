//! Transaction repository for database operations
//!
//! Handles CRUD operations for transactions in the database.

use rusqlite::{Connection, Result};
use log::{info, error};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::action_storage::{StoredAction, ActionStatus, ActionInput, ActionOutput, TransactionStatus};

pub struct TransactionRepository<'a> {
    conn: &'a Connection,
}

impl<'a> TransactionRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        TransactionRepository { conn }
    }

    /// Add a new transaction (action) to the database
    ///
    /// `user_id` is needed for the tx_labels table (label deduplication is per-user).
    pub fn add_transaction(&self, action: &StoredAction, user_id: i64) -> Result<i64> {
        // Compute the consolidated status from legacy ActionStatus
        let tx_status = TransactionStatus::from_legacy(&action.status, None);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Atomicity (OD-2 commit 2): wrap the whole multi-statement insert
        // (transactions + labels + inputs + outputs) in ONE SQLite transaction so an
        // unclean process kill mid-sequence rolls back to nothing rather than leaving a
        // partial transaction row that startup recovery then has to reconcile. Any early
        // `return` below drops `tx` uncommitted → automatic rollback.
        let tx = self.conn.unchecked_transaction()?;

        // Insert into transactions table with single status column
        tx.execute(
            "INSERT INTO transactions (
                txid, reference_number, raw_tx, description, status, is_outgoing,
                satoshis, block_height, confirmations, version, lock_time, price_usd_cents, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            rusqlite::params![
                action.txid,
                action.reference_number,
                action.raw_tx,
                action.description,
                tx_status.as_str(),
                action.is_outgoing,
                action.satoshis,
                action.block_height.map(|h| h as i32),
                action.confirmations as i32,
                action.version as i32,
                action.lock_time as i32,
                action.price_usd_cents,
                now,
                now,
            ],
        )?;

        let transaction_id = tx.last_insert_rowid();
        Self::insert_children(&tx, transaction_id, action, user_id, now)?;

        tx.commit()?;

        info!("   ✅ Transaction {} saved to database (ID: {})", action.txid, transaction_id);
        Ok(transaction_id)
    }

    /// Labels, inputs and outputs of a stored action (shared by `add_transaction` and
    /// `fill_reservation_row`). Runs inside the caller's SQLite transaction.
    fn insert_children(tx: &Connection, transaction_id: i64, action: &StoredAction, user_id: i64, now: i64) -> Result<()> {
        // Insert transaction labels into tx_labels/tx_labels_map
        for label in &action.labels {
            let normalized = label.trim().to_lowercase();
            if normalized.is_empty() { continue; }

            // Find or insert the deduplicated label entity
            let label_id: i64 = match tx.query_row(
                "SELECT txLabelId FROM tx_labels WHERE label = ?1 AND user_id = ?2 AND is_deleted = 0",
                rusqlite::params![&normalized, user_id],
                |row| row.get(0),
            ) {
                Ok(id) => id,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    tx.execute(
                        "INSERT INTO tx_labels (user_id, label, is_deleted, created_at, updated_at) VALUES (?1, ?2, 0, ?3, ?4)",
                        rusqlite::params![user_id, &normalized, now, now],
                    )?;
                    tx.last_insert_rowid()
                }
                Err(e) => return Err(e),
            };

            // Create the label→transaction mapping
            tx.execute(
                "INSERT OR IGNORE INTO tx_labels_map (txLabelId, transaction_id, is_deleted, created_at, updated_at) VALUES (?1, ?2, 0, ?3, ?4)",
                rusqlite::params![label_id, transaction_id, now, now],
            )?;
        }

        // Insert transaction inputs
        for input in &action.inputs {
            tx.execute(
                "INSERT INTO transaction_inputs (
                    transaction_id, txid, vout, satoshis, script
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    transaction_id,
                    input.txid,
                    input.vout as i32,
                    input.satoshis,
                    input.script,
                ],
            )?;
        }

        // Insert transaction outputs
        for output in &action.outputs {
            tx.execute(
                "INSERT INTO transaction_outputs (
                    transaction_id, vout, satoshis, script, address
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    transaction_id,
                    output.vout as i32,
                    output.satoshis,
                    output.script,
                    output.address,
                ],
            )?;
        }
        Ok(())
    }

    /// The txid a reservation row carries until its transaction is built. Not hex, so it
    /// can never collide with a real txid or be looked up on chain; readers that need a
    /// non-NULL `txid` (activity, listActions) keep working while a spend is in flight.
    pub fn unbuilt_txid(reference: &str) -> String {
        format!("unbuilt-{}", reference)
    }

    /// B5-T1-P3: the row a spend owns its coins by, created **before** any coin is claimed
    /// (wallet-toolbox `createAction.ts`: transaction row first, `status: 'unsigned'`).
    /// Status `unsigned`, so a crash before it is filled is failed — and its coins
    /// released — by `TaskFailAbandoned` after 5 minutes. Call inside the claim's SQLite
    /// transaction.
    pub fn create_reservation_row(&self, reference: &str, description: Option<&str>) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        self.conn.execute(
            "INSERT INTO transactions (
                txid, reference_number, raw_tx, description, status, is_outgoing,
                satoshis, confirmations, version, lock_time, created_at, updated_at
            ) VALUES (?1, ?2, '', ?3, 'unsigned', 1, 0, 0, 1, 0, ?4, ?4)",
            rusqlite::params![Self::unbuilt_txid(reference), reference, description, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Fill a reservation row with the built transaction, keeping its id (the coins it
    /// claimed stay owned by it). Same columns and children as `add_transaction`.
    ///
    /// ⛔ Only an `unsigned` row is filled: a backstop (`TaskFailAbandoned`, startup
    /// cleanup) that failed the row while the spend was still building has already
    /// released its coins, and filling would un-fail it and let the spend broadcast with
    /// free inputs (B5-T1-P3 review #1). `created_at` restarts here: the row's age is the
    /// age of a built transaction, as it was when the row was created at this point.
    pub fn fill_reservation_row(&self, id: i64, action: &StoredAction, user_id: i64) -> Result<i64> {
        let tx_status = TransactionStatus::from_legacy(&action.status, None);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let tx = self.conn.unchecked_transaction()?;
        let n = tx.execute(
            "UPDATE transactions SET
                txid = ?1, raw_tx = ?2, description = ?3, status = ?4, is_outgoing = ?5,
                satoshis = ?6, block_height = ?7, confirmations = ?8, version = ?9, lock_time = ?10,
                price_usd_cents = ?11, updated_at = ?12, created_at = ?12
             WHERE id = ?13 AND reference_number = ?14 AND status = 'unsigned'",
            rusqlite::params![
                action.txid,
                action.raw_tx,
                action.description,
                tx_status.as_str(),
                action.is_outgoing,
                action.satoshis,
                action.block_height.map(|h| h as i32),
                action.confirmations as i32,
                action.version as i32,
                action.lock_time as i32,
                action.price_usd_cents,
                now,
                id,
                action.reference_number,
            ],
        )?;
        if n != 1 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
                Some(format!("reservation row {} for {} not found or no longer unsigned", id, action.reference_number)),
            ));
        }
        Self::insert_children(&tx, id, action, user_id, now)?;
        tx.commit()?;
        info!("   ✅ Transaction {} saved to database (ID: {}, reserved first)", action.txid, id);
        Ok(id)
    }

    /// Remove a reservation row that never got a transaction (a refusal before the
    /// transaction was built). Only an **unbuilt** row is removed; the caller releases its
    /// coins first, in the same SQLite transaction. Returns rows deleted (0 or 1).
    pub fn discard_unbuilt_row(&self, id: i64) -> Result<usize> {
        self.conn.execute("DELETE FROM money_utxos WHERE reserved_by = ?1", rusqlite::params![id])?;
        self.conn.execute(
            "DELETE FROM transactions WHERE id = ?1 AND txid LIKE 'unbuilt-%'",
            rusqlite::params![id],
        )
    }

    /// Get transaction by txid
    pub fn get_by_txid(&self, txid: &str) -> Result<Option<StoredAction>> {
        // Get transaction
        let mut stmt = self.conn.prepare(
            "SELECT id, txid, reference_number, raw_tx, description, status, is_outgoing,
                    satoshis, created_at, block_height, confirmations, version, lock_time, price_usd_cents
             FROM transactions
             WHERE txid = ?1"
        )?;

        let tx_result = stmt.query_row(
            rusqlite::params![txid],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,  // id
                    row.get::<_, String>(1)?,  // txid
                    row.get::<_, String>(2)?,  // reference_number
                    row.get::<_, Option<String>>(3)?,  // raw_tx (nullable in new schema)
                    row.get::<_, Option<String>>(4)?,  // description
                    row.get::<_, String>(5)?,  // status (TransactionStatus string)
                    row.get::<_, bool>(6)?,  // is_outgoing
                    row.get::<_, i64>(7)?,  // satoshis
                    row.get::<_, i64>(8)?,  // created_at
                    row.get::<_, Option<i32>>(9)?,  // block_height
                    row.get::<_, i32>(10)?,  // confirmations
                    row.get::<_, i32>(11)?,  // version
                    row.get::<_, i32>(12)?,  // lock_time
                    row.get::<_, Option<i64>>(13)?,  // price_usd_cents
                ))
            },
        );

        let (transaction_id, txid_val, ref_num, raw_tx_opt, desc, status_str, is_out, sats, ts, bh, conf, ver, lt, price_usd_cents) = match tx_result {
            Ok(t) => t,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e),
        };

        // Get labels from tx_labels/tx_labels_map (Phase 5 normalized tables)
        let mut label_stmt = self.conn.prepare(
            "SELECT tl.label
             FROM tx_labels tl
             INNER JOIN tx_labels_map tlm ON tl.txLabelId = tlm.txLabelId
             WHERE tlm.transaction_id = ?1 AND tlm.is_deleted = 0 AND tl.is_deleted = 0
             ORDER BY tl.label"
        )?;
        let labels: Vec<String> = label_stmt.query_map(
            rusqlite::params![transaction_id],
            |row| Ok(row.get(0)?),
        )?.filter_map(|r| r.ok()).collect();

        // Get inputs
        let mut input_stmt = self.conn.prepare(
            "SELECT txid, vout, satoshis, script FROM transaction_inputs WHERE transaction_id = ?1"
        )?;
        let inputs: Vec<ActionInput> = input_stmt.query_map(
            rusqlite::params![transaction_id],
            |row| {
                Ok(ActionInput {
                    txid: row.get(0)?,
                    vout: row.get::<_, i32>(1)? as u32,
                    satoshis: row.get(2)?,
                    script: row.get(3)?,
                })
            },
        )?.collect::<Result<Vec<_>>>()?;

        // Get outputs
        let mut output_stmt = self.conn.prepare(
            "SELECT vout, satoshis, script, address FROM transaction_outputs WHERE transaction_id = ?1"
        )?;
        let outputs: Vec<ActionOutput> = output_stmt.query_map(
            rusqlite::params![transaction_id],
            |row| {
                Ok(ActionOutput {
                    vout: row.get::<_, i32>(0)? as u32,
                    satoshis: row.get(1)?,
                    script: row.get(2)?,
                    address: row.get(3)?,
                })
            },
        )?.collect::<Result<Vec<_>>>()?;

        // Parse status: stored as TransactionStatus string, map to ActionStatus at boundary
        let tx_status = TransactionStatus::from_str(&status_str);
        let status = tx_status.to_action_status();

        Ok(Some(StoredAction {
            txid: txid_val,
            reference_number: ref_num,
            raw_tx: raw_tx_opt.unwrap_or_default(),
            description: desc,
            labels,
            status,
            is_outgoing: is_out,
            satoshis: sats,
            timestamp: ts,
            block_height: bh.map(|h| h as u32),
            confirmations: conf as u32,
            version: ver as u32,
            lock_time: lt as u32,
            inputs,
            outputs,
            price_usd_cents,
        }))
    }

    /// Get transaction by reference number
    pub fn get_by_reference(&self, reference_number: &str) -> Result<Option<StoredAction>> {
        let mut stmt = self.conn.prepare(
            "SELECT txid FROM transactions WHERE reference_number = ?1"
        )?;

        let txid: Option<String> = match stmt.query_row(
            rusqlite::params![reference_number],
            |row| Ok(row.get(0)?),
        ) {
            Ok(t) => Some(t),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };

        match txid {
            Some(t) => self.get_by_txid(&t),
            None => Ok(None),
        }
    }

    /// Update transaction status (legacy ActionStatus interface)
    ///
    /// Converts ActionStatus to TransactionStatus and writes to `status` column.
    pub fn update_status(&self, txid: &str, status: ActionStatus) -> Result<()> {
        let tx_status = TransactionStatus::from_legacy(&status, None);
        self.set_transaction_status(txid, tx_status)
    }

    /// Set the consolidated transaction status (wallet-toolbox aligned)
    ///
    /// Single `status` column. If setting to Failed, also records `failed_at` timestamp
    /// for the UnFail mechanism.
    pub fn set_transaction_status(&self, txid: &str, status: TransactionStatus) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        if status == TransactionStatus::Failed {
            self.conn.execute(
                "UPDATE transactions SET status = ?1, failed_at = ?2, updated_at = ?3 WHERE txid = ?4",
                rusqlite::params![status.as_str(), now, now, txid],
            )?;
        } else {
            // Clear failed_at if transitioning away from failed (UnFail success)
            self.conn.execute(
                "UPDATE transactions SET status = ?1, failed_at = NULL, updated_at = ?2 WHERE txid = ?3",
                rusqlite::params![status.as_str(), now, txid],
            )?;
        }

        info!("   ✅ Set transaction status to '{}' for txid {}", status.as_str(), txid);
        Ok(())
    }

    /// Update transaction TXID (after signing)
    ///
    /// B5-T1-P3: the row is updated **in place** — its `id` never changes. The coins a
    /// transaction reserves are owned by that id (`outputs.spent_by`,
    /// `money_utxos.reserved_by`) from the moment they are claimed, so signing must not
    /// re-create the row. (Until P3 this deleted the row and re-inserted it under a new id,
    /// which detached `spent_by` for the length of the call and dropped the
    /// `commissions` row recorded for it.)
    ///
    /// `user_id` is kept for signature compatibility; nothing is re-inserted any more.
    pub fn update_txid(&self, reference_number: &str, new_txid: String, new_raw_tx: String, _user_id: i64) -> Result<()> {
        let (id, old_txid): (i64, Option<String>) = self.conn.query_row(
            "SELECT id, txid FROM transactions WHERE reference_number = ?1",
            rusqlite::params![reference_number],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
                Some(format!("Transaction not found: {}", reference_number)),
            ),
            other => other,
        })?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE transactions SET txid = ?1, raw_tx = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![&new_txid, &new_raw_tx, now, id],
        )?;

        // Inputs labelled with the pre-signing txid (phase 1 of two-phase signing) follow
        // the new txid and are owned by this row.
        if let Some(ref old) = old_txid {
            let relabelled = tx.execute(
                "UPDATE outputs SET spent_by = ?1, spending_description = ?2
                 WHERE spending_description = ?3 AND spendable = 0",
                rusqlite::params![id, &new_txid, old],
            )?;
            if relabelled > 0 {
                info!("   ✅ Re-labelled {} spent input(s) {} → {}", relabelled, old, new_txid);
            }
        }
        tx.commit()?;

        info!("   ✅ Updated TXID for reference {} (id={}): {:?} → {}", reference_number, id, old_txid, new_txid);
        Ok(())
    }

    /// Update confirmations and block height
    pub fn update_confirmations(&self, txid: &str, confirmations: u32, block_height: Option<u32>) -> Result<()> {
        self.conn.execute(
            "UPDATE transactions SET confirmations = ?1, block_height = ?2 WHERE txid = ?3",
            rusqlite::params![
                confirmations as i32,
                block_height.map(|h| h as i32),
                txid,
            ],
        )?;
        Ok(())
    }

    /// Get raw transaction hex by txid
    ///
    /// This is more efficient than get_by_txid when you only need the raw tx.
    pub fn get_raw_tx(&self, txid: &str) -> Result<Option<String>> {
        let result: std::result::Result<String, rusqlite::Error> = self.conn.query_row(
            "SELECT raw_tx FROM transactions WHERE txid = ?1",
            rusqlite::params![txid],
            |row| row.get(0),
        );

        match result {
            Ok(raw_tx) => Ok(Some(raw_tx)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get raw transaction hex for a local (unbroadcast or unconfirmed) transaction
    ///
    /// Used for BEEF building when a child transaction spends outputs from
    /// a parent transaction that hasn't been confirmed yet.
    /// Returns the raw_tx if found and status is not 'completed' (confirmed).
    pub fn get_local_parent_tx(&self, txid: &str) -> Result<Option<String>> {
        info!("   🔍 Looking for local parent tx {} in transactions table...", txid);

        let result: std::result::Result<String, rusqlite::Error> = self.conn.query_row(
            "SELECT raw_tx FROM transactions WHERE txid = ?1 AND status != 'completed'",
            rusqlite::params![txid],
            |row| row.get(0),
        );

        match result {
            Ok(raw_tx) => {
                info!("   📋 Found local parent tx {} in database ({} bytes)", txid, raw_tx.len());
                Ok(Some(raw_tx))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                info!("   ❌ Local parent tx {} NOT found in transactions table", txid);
                Ok(None)
            }
            Err(e) => {
                error!("   ❌ Error querying local parent tx {}: {}", txid, e);
                Err(e)
            }
        }
    }

    /// Update broadcast status (legacy interface)
    ///
    /// Maps legacy broadcast_status strings to TransactionStatus and calls
    /// set_transaction_status(). Kept for backward compatibility with callers.
    pub fn update_broadcast_status(&self, txid: &str, status: &str) -> Result<()> {
        let tx_status = match status {
            "confirmed" => TransactionStatus::Completed,
            "broadcast" => TransactionStatus::Unproven,
            "failed" => TransactionStatus::Failed,
            "pending" => TransactionStatus::Unsigned,
            _ => TransactionStatus::Unprocessed,
        };
        self.set_transaction_status(txid, tx_status)
    }

    /// Update raw_tx for a transaction (e.g., after signing replaces unsigned tx with signed tx).
    ///
    /// This is critical for BEEF building: when a subsequent transaction spends outputs from
    /// this transaction, the BEEF builder needs the SIGNED raw_tx to include as a parent.
    /// The unsigned version has a different txid and would make the BEEF invalid.
    pub fn update_raw_tx(&self, txid: &str, raw_tx: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE transactions SET raw_tx = ?1 WHERE txid = ?2",
            rusqlite::params![raw_tx, txid],
        )?;

        if rows > 0 {
            info!("   ✅ Updated raw_tx for txid {} ({} hex chars)", txid, raw_tx.len());
        } else {
            info!("   ℹ️  No transaction found with txid {} to update raw_tx", txid);
        }
        Ok(())
    }

    /// Update the txid of a transaction record (e.g., after signing changes the txid).
    ///
    /// BSV txids include unlocking scripts, so signing changes the txid.
    /// This updates the stored record from the pre-signing txid to the post-signing txid.
    pub fn rename_txid(&self, old_txid: &str, new_txid: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE transactions SET txid = ?1 WHERE txid = ?2",
            rusqlite::params![new_txid, old_txid],
        )?;

        if rows > 0 {
            info!("   ✅ Updated transaction txid: {} → {}",
                &old_txid[..std::cmp::min(16, old_txid.len())],
                &new_txid[..std::cmp::min(16, new_txid.len())]);
        } else {
            log::debug!("   ℹ️  No transaction found with txid {} to rename (likely already updated)", &old_txid[..std::cmp::min(16, old_txid.len())]);
        }

        Ok(())
    }

    /// Get the transaction status string by txid.
    ///
    /// Returns the `status` column value (TransactionStatus string).
    pub fn get_broadcast_status(&self, txid: &str) -> Result<Option<String>> {
        match self.conn.query_row(
            "SELECT status FROM transactions WHERE txid = ?1",
            rusqlite::params![txid],
            |row| row.get::<_, Option<String>>(0),
        ) {
            Ok(status) => Ok(status),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get the TransactionStatus enum value for a transaction.
    ///
    /// Returns None if tx doesn't exist.
    pub fn get_transaction_status(&self, txid: &str) -> Result<Option<TransactionStatus>> {
        match self.conn.query_row(
            "SELECT status FROM transactions WHERE txid = ?1",
            rusqlite::params![txid],
            |row| row.get::<_, Option<String>>(0),
        ) {
            Ok(Some(status_str)) => Ok(Some(TransactionStatus::from_str(&status_str))),
            Ok(None) => Ok(None),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get all transactions with unsigned status that are older than the given age in seconds.
    ///
    /// These are transactions that were created but never broadcast (e.g., process crashed
    /// between creating the transaction and broadcasting it). Their UTXOs are ghost outputs
    /// that don't exist on-chain and should be cleaned up.
    ///
    /// Returns a list of (txid, list of input txid:vout pairs) for cleanup.
    pub fn get_stale_pending_transactions(&self, max_age_secs: i64) -> Result<Vec<(String, Vec<(String, u32)>)>> {
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 - max_age_secs;

        let mut stmt = self.conn.prepare(
            "SELECT id, txid FROM transactions WHERE status = 'unsigned' AND created_at < ?1"
        )?;

        let rows: Vec<(i64, String)> = stmt.query_map(
            rusqlite::params![cutoff],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
        .collect::<Result<Vec<_>>>()?;

        let mut result = Vec::new();

        for (tx_db_id, txid) in rows {
            // Get the inputs for this transaction so we can restore them
            let mut input_stmt = self.conn.prepare(
                "SELECT txid, vout FROM transaction_inputs WHERE transaction_id = ?1"
            )?;

            let inputs: Vec<(String, u32)> = input_stmt.query_map(
                rusqlite::params![tx_db_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)? as u32)),
            )?
            .collect::<Result<Vec<_>>>()?;

            result.push((txid, inputs));
        }

        Ok(result)
    }

    /// List all transactions with optional filters
    pub fn list_transactions(&self, label_filter: Option<&Vec<String>>, label_mode: Option<&str>) -> Result<Vec<StoredAction>> {
        // Get all transaction IDs
        // B5-T1-P3: a reservation row whose transaction is not built yet (`unbuilt-…`
        // txid) is not a transaction — it is not listed.
        let mut stmt = self.conn.prepare(
            "SELECT txid FROM transactions WHERE txid NOT LIKE 'unbuilt-%' ORDER BY created_at DESC")?;
        let txids: Vec<String> = stmt.query_map([], |row| Ok(row.get(0)?))?
            .collect::<Result<Vec<_>>>()?;

        // Load each transaction
        let mut transactions = Vec::new();
        for txid in txids {
            if let Some(Some(tx)) = self.get_by_txid(&txid).ok() {
                // Apply label filter if provided
                if let Some(labels) = label_filter {
                    let matches = match label_mode {
                        Some("all") => labels.iter().all(|l| tx.labels.contains(l)),
                        _ => labels.iter().any(|l| tx.labels.contains(l)), // "any" mode
                    };
                    if matches {
                        transactions.push(tx);
                    }
                } else {
                    transactions.push(tx);
                }
            }
        }

        Ok(transactions)
    }
}

// ============================================================================
// B5-T1-P3 — signing keeps the transaction row's id.
// The coins a transaction reserves are owned by `transactions.id`; a signing step that
// re-creates the row orphans them and drops the row's commission record.
// RED: the pre-P3 `update_txid` (delete + re-insert) ⇒ the id changes and the
// commission count is 0.
// ============================================================================
#[cfg(test)]
mod update_txid_keeps_id_tests {
    use super::*;

    fn db() -> crate::database::WalletDatabase {
        let path = std::env::temp_dir().join(format!("hodos_t1p3_txid_{}.db", uuid::Uuid::new_v4()));
        let db = crate::database::WalletDatabase::new(path).unwrap();
        db.connection().execute(
            "INSERT OR IGNORE INTO users (userId, identity_key, active_storage, created_at, updated_at)
             VALUES (1, 'test_identity_key', 'local', 0, 0)", []).unwrap();
        db
    }

    fn action(txid: &str, reference: &str) -> StoredAction {
        StoredAction {
            txid: txid.to_string(), reference_number: reference.to_string(), raw_tx: "00".into(),
            description: Some("t".into()), labels: vec![], status: ActionStatus::Created,
            is_outgoing: true, satoshis: -1000, timestamp: 0, block_height: None, confirmations: 0,
            version: 1, lock_time: 0, inputs: vec![], outputs: vec![], price_usd_cents: None,
        }
    }

    #[test]
    fn signing_keeps_the_row_id_its_commission_and_its_inputs() {
        let db = db();
        let conn = db.connection();
        let repo = TransactionRepository::new(conn);
        let unsigned = "a".repeat(64);
        let signed = "b".repeat(64);

        let id = repo.add_transaction(&action(&unsigned, "ref-p3"), 1).unwrap();
        conn.execute(
            "INSERT INTO commissions (user_id, transaction_id, satoshis, key_offset, is_redeemed, locking_script, created_at, updated_at)
             VALUES (1, ?1, 1000, 'hodos-service-fee', 0, x'00', 0, 0)", [id]).unwrap();
        // an input reserved by this row in phase 1 of two-phase signing
        conn.execute(
            "INSERT INTO outputs (user_id, spendable, change, vout, satoshis, provided_by, purpose, type,
                                  txid, spent_by, spending_description, confirmed, created_at, updated_at)
             VALUES (1, 0, 0, 0, 5000, 'you', '', 'P2PKH', 'parent', ?1, ?2, 1, 0, 0)",
            rusqlite::params![id, &unsigned]).unwrap();

        repo.update_txid("ref-p3", signed.clone(), "01".into(), 1).unwrap();

        let after: i64 = conn.query_row("SELECT id FROM transactions WHERE txid = ?1", [&signed], |r| r.get(0)).unwrap();
        assert_eq!(after, id, "signing must not re-create the transaction row");
        let commissions: i64 = conn.query_row("SELECT COUNT(*) FROM commissions WHERE transaction_id = ?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(commissions, 1, "the commission recorded for the row survives signing");
        let (spent_by, desc): (Option<i64>, Option<String>) = conn.query_row(
            "SELECT spent_by, spending_description FROM outputs WHERE txid = 'parent'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(spent_by, Some(id), "the input stays owned by the same row");
        assert_eq!(desc.as_deref(), Some(signed.as_str()), "the input's label follows the signed txid");
    }
    /// B5-T1-P3 review #1: a reservation row a backstop failed while the spend was still
    /// building (its coins already released) is never filled — filling would un-fail it
    /// and the spend would broadcast with free inputs. RED (scratch): drop
    /// `AND status = 'unsigned'` from `fill_reservation_row` ⇒ Ok, and the row is
    /// `unsigned` again.
    #[test]
    fn a_failed_reservation_row_is_never_filled() {
        let db = db();
        let repo = TransactionRepository::new(db.connection());
        let id = repo.create_reservation_row("ref-x", None).unwrap();
        db.connection().execute("UPDATE transactions SET status = 'failed' WHERE id = ?1", [id]).unwrap();
        let mut a = action(&"c".repeat(64), "ref-x");
        a.status = ActionStatus::Created;
        assert!(repo.fill_reservation_row(id, &a, 1).is_err(), "a failed row must not be filled");
        let status: String = db.connection().query_row("SELECT status FROM transactions WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(status, "failed");
    }
}
