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
    pub fn add_transaction(&self, action: &StoredAction) -> Result<i64> {
        // Compute the consolidated new_status from legacy ActionStatus
        let new_status = TransactionStatus::from_legacy(&action.status, None);

        // Insert into transactions table (writes both legacy status and new_status)
        self.conn.execute(
            "INSERT INTO transactions (
                txid, reference_number, raw_tx, description, status, is_outgoing,
                satoshis, timestamp, block_height, confirmations, version, lock_time, new_status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                action.txid,
                action.reference_number,
                action.raw_tx,
                action.description,
                action.status.to_string(),
                action.is_outgoing,
                action.satoshis,
                action.timestamp,
                action.block_height.map(|h| h as i32),
                action.confirmations as i32,
                action.version as i32,
                action.lock_time as i32,
                new_status.as_str(),
            ],
        )?;

        let transaction_id = self.conn.last_insert_rowid();

        // Insert transaction labels
        for label in &action.labels {
            self.conn.execute(
                "INSERT INTO transaction_labels (transaction_id, label) VALUES (?1, ?2)",
                rusqlite::params![transaction_id, label],
            )?;
        }

        // Insert transaction inputs
        for input in &action.inputs {
            self.conn.execute(
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
            self.conn.execute(
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

        info!("   ✅ Transaction {} saved to database (ID: {})", action.txid, transaction_id);
        Ok(transaction_id)
    }

    /// Get transaction by txid
    pub fn get_by_txid(&self, txid: &str) -> Result<Option<StoredAction>> {
        // Get transaction
        let mut stmt = self.conn.prepare(
            "SELECT id, txid, reference_number, raw_tx, description, status, is_outgoing,
                    satoshis, timestamp, block_height, confirmations, version, lock_time
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
                    row.get::<_, String>(3)?,  // raw_tx
                    row.get::<_, Option<String>>(4)?,  // description
                    row.get::<_, String>(5)?,  // status
                    row.get::<_, bool>(6)?,  // is_outgoing
                    row.get::<_, i64>(7)?,  // satoshis
                    row.get::<_, i64>(8)?,  // timestamp
                    row.get::<_, Option<i32>>(9)?,  // block_height
                    row.get::<_, i32>(10)?,  // confirmations
                    row.get::<_, i32>(11)?,  // version
                    row.get::<_, i32>(12)?,  // lock_time
                ))
            },
        );

        let (transaction_id, txid_val, ref_num, raw_tx, desc, status_str, is_out, sats, ts, bh, conf, ver, lt) = match tx_result {
            Ok(t) => t,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e),
        };

        // Get labels (Phase 5: try new tables first, fallback to old)
        let labels: Vec<String> = {
            let mut new_stmt = self.conn.prepare(
                "SELECT tl.label
                 FROM tx_labels tl
                 INNER JOIN tx_labels_map tlm ON tl.txLabelId = tlm.txLabelId
                 WHERE tlm.transaction_id = ?1 AND tlm.is_deleted = 0 AND tl.is_deleted = 0
                 ORDER BY tl.label"
            )?;
            let new_labels: Vec<String> = new_stmt.query_map(
                rusqlite::params![transaction_id],
                |row| Ok(row.get(0)?),
            )?.filter_map(|r| r.ok()).collect();

            if !new_labels.is_empty() {
                new_labels
            } else {
                // Fallback to old table
                let mut old_stmt = self.conn.prepare(
                    "SELECT label FROM transaction_labels WHERE transaction_id = ?1"
                )?;
                let old_labels: Vec<String> = old_stmt.query_map(
                    rusqlite::params![transaction_id],
                    |row| Ok(row.get(0)?),
                )?.filter_map(|r| r.ok()).collect();
                old_labels
            }
        };

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

        // Parse status
        let status = match status_str.as_str() {
            "created" => ActionStatus::Created,
            "signed" => ActionStatus::Signed,
            "unconfirmed" => ActionStatus::Unconfirmed,
            "pending" => ActionStatus::Pending,
            "confirmed" => ActionStatus::Confirmed,
            "aborted" => ActionStatus::Aborted,
            "failed" => ActionStatus::Failed,
            _ => ActionStatus::Created,
        };

        Ok(Some(StoredAction {
            txid: txid_val,
            reference_number: ref_num,
            raw_tx,
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

    /// Update transaction status (legacy ActionStatus)
    ///
    /// Prefer `set_transaction_status()` for new code. This method also updates
    /// the new_status column to keep both in sync during the transition period.
    pub fn update_status(&self, txid: &str, status: ActionStatus) -> Result<()> {
        // Also derive and write new_status
        let new_status = TransactionStatus::from_legacy(&status, None);
        self.conn.execute(
            "UPDATE transactions SET status = ?1, new_status = ?2 WHERE txid = ?3",
            rusqlite::params![status.to_string(), new_status.as_str(), txid],
        )?;
        Ok(())
    }

    /// Set the consolidated transaction status (wallet-toolbox aligned)
    ///
    /// This is the primary status update method for Phase 1+. Writes to `new_status`
    /// and also keeps the legacy `status` column in sync for backward compatibility.
    /// If setting to Failed, also records `failed_at` timestamp for UnFail mechanism.
    pub fn set_transaction_status(&self, txid: &str, status: TransactionStatus) -> Result<()> {
        let legacy_status = status.to_action_status();

        if status == TransactionStatus::Failed {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            self.conn.execute(
                "UPDATE transactions SET new_status = ?1, status = ?2, failed_at = ?3 WHERE txid = ?4",
                rusqlite::params![status.as_str(), legacy_status.to_string(), now, txid],
            )?;
        } else {
            // Clear failed_at if transitioning away from failed (UnFail success)
            self.conn.execute(
                "UPDATE transactions SET new_status = ?1, status = ?2, failed_at = NULL WHERE txid = ?3",
                rusqlite::params![status.as_str(), legacy_status.to_string(), txid],
            )?;
        }

        info!("   ✅ Set transaction status to '{}' for txid {}", status.as_str(), txid);
        Ok(())
    }

    /// Update transaction TXID (after signing)
    pub fn update_txid(&self, reference_number: &str, new_txid: String, new_raw_tx: String) -> Result<()> {
        // Get old transaction
        let old_tx = self.get_by_reference(reference_number)?
            .ok_or_else(|| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
                Some(format!("Transaction not found: {}", reference_number))
            ))?;

        // Delete old transaction and related data
        let old_txid = old_tx.txid.clone(); // Capture old TXID before moving old_tx
        let mut stmt = self.conn.prepare("SELECT id FROM transactions WHERE txid = ?1")?;
        let old_id: i64 = stmt.query_row(rusqlite::params![&old_txid], |row| row.get(0))?;

        self.conn.execute("DELETE FROM transaction_outputs WHERE transaction_id = ?1", rusqlite::params![old_id])?;
        self.conn.execute("DELETE FROM transaction_inputs WHERE transaction_id = ?1", rusqlite::params![old_id])?;
        self.conn.execute("DELETE FROM transaction_labels WHERE transaction_id = ?1", rusqlite::params![old_id])?;
        self.conn.execute("DELETE FROM transactions WHERE id = ?1", rusqlite::params![old_id])?;

        // Create new transaction with new TXID
        let mut new_action = old_tx;
        new_action.txid = new_txid.clone();
        new_action.raw_tx = new_raw_tx;
        self.add_transaction(&new_action)?;

        info!("   ✅ Updated TXID for reference {}: {} → {}", reference_number, old_txid, new_txid);
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

        // Check if new_status column exists (migration v15)
        let has_new_status: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'new_status'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        // First, check if ANY transaction with this txid exists (for debugging)
        let any_exists: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE txid = ?1",
            rusqlite::params![txid],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        info!("   📊 Transaction with txid {} exists in table: {}", txid, any_exists);

        if any_exists {
            // Log the new_status for debugging
            if has_new_status {
                if let Ok(status) = self.conn.query_row::<String, _, _>(
                    "SELECT COALESCE(new_status, 'NULL') FROM transactions WHERE txid = ?1",
                    rusqlite::params![txid],
                    |row| row.get(0),
                ) {
                    info!("   📊 Transaction {} has new_status: '{}'", txid, status);
                }
            }
            if let Ok(raw_tx_len) = self.conn.query_row::<i64, _, _>(
                "SELECT LENGTH(raw_tx) FROM transactions WHERE txid = ?1",
                rusqlite::params![txid],
                |row| row.get(0),
            ) {
                info!("   📊 Transaction {} has raw_tx length: {}", txid, raw_tx_len);
            }
        }

        let result: std::result::Result<String, rusqlite::Error> = if has_new_status {
            // Only return raw_tx if transaction is not completed
            // (completed transactions should be fetched from API to get merkle proof)
            self.conn.query_row(
                "SELECT raw_tx FROM transactions WHERE txid = ?1 AND new_status != 'completed'",
                rusqlite::params![txid],
                |row| row.get(0),
            )
        } else {
            // Pre-v15 fallback: check broadcast_status if available
            let has_broadcast_status: bool = self.conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'broadcast_status'",
                [],
                |row| Ok(row.get::<_, i64>(0)? > 0),
            ).unwrap_or(false);

            if has_broadcast_status {
                self.conn.query_row(
                    "SELECT raw_tx FROM transactions WHERE txid = ?1 AND broadcast_status != 'confirmed'",
                    rusqlite::params![txid],
                    |row| row.get(0),
                )
            } else {
                self.conn.query_row(
                    "SELECT raw_tx FROM transactions WHERE txid = ?1",
                    rusqlite::params![txid],
                    |row| row.get(0),
                )
            }
        };

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

    /// Update broadcast status (legacy method)
    ///
    /// Prefer `set_transaction_status()` for new code. This method also updates
    /// the new_status column to keep both in sync during the transition period.
    pub fn update_broadcast_status(&self, txid: &str, status: &str) -> Result<()> {
        // Check if broadcast_status column exists
        let column_exists: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'broadcast_status'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        if column_exists {
            // Map broadcast_status string to TransactionStatus
            let new_status = match status {
                "confirmed" => "completed",
                "broadcast" => "unproven",
                "failed" => "failed",
                "pending" => "unsigned",
                _ => "unprocessed",
            };

            if status == "failed" {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                self.conn.execute(
                    "UPDATE transactions SET broadcast_status = ?1, new_status = ?2, failed_at = ?3 WHERE txid = ?4",
                    rusqlite::params![status, new_status, now, txid],
                )?;
            } else {
                self.conn.execute(
                    "UPDATE transactions SET broadcast_status = ?1, new_status = ?2 WHERE txid = ?3",
                    rusqlite::params![status, new_status, txid],
                )?;
            }
            info!("   ✅ Updated broadcast_status to '{}' (new_status='{}') for txid {}", status, new_status, txid);
        }
        Ok(())
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
            log::warn!("   ⚠️  No transaction found with txid {} to update", &old_txid[..std::cmp::min(16, old_txid.len())]);
        }

        Ok(())
    }

    /// Get the transaction status (new consolidated status) by txid.
    ///
    /// Returns the new_status value if available, falls back to broadcast_status for pre-v15 DBs.
    ///
    /// Returns:
    /// - `Ok(Some(status))` if the transaction exists and has a status
    /// - `Ok(None)` if the transaction doesn't exist or the column doesn't exist
    /// - `Err` on database errors
    pub fn get_broadcast_status(&self, txid: &str) -> Result<Option<String>> {
        // Prefer new_status column (v15+)
        let has_new_status: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'new_status'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        if has_new_status {
            return match self.conn.query_row(
                "SELECT new_status FROM transactions WHERE txid = ?1",
                rusqlite::params![txid],
                |row| row.get::<_, Option<String>>(0),
            ) {
                Ok(status) => Ok(status),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            };
        }

        // Fallback to broadcast_status for pre-v15 databases
        let column_exists: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'broadcast_status'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        if !column_exists {
            return Ok(None);
        }

        match self.conn.query_row(
            "SELECT broadcast_status FROM transactions WHERE txid = ?1",
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
    /// Reads from new_status column (v15+). Returns None if tx doesn't exist.
    pub fn get_transaction_status(&self, txid: &str) -> Result<Option<TransactionStatus>> {
        let has_new_status: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'new_status'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        if !has_new_status {
            return Ok(None);
        }

        match self.conn.query_row(
            "SELECT new_status FROM transactions WHERE txid = ?1",
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

        // Prefer new_status column (v15+)
        let has_new_status: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'new_status'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        let query = if has_new_status {
            "SELECT id, txid FROM transactions WHERE new_status = 'unsigned' AND timestamp < ?1"
        } else {
            // Fallback to broadcast_status for pre-v15 databases
            let column_exists: bool = self.conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'broadcast_status'",
                [],
                |row| Ok(row.get::<_, i64>(0)? > 0),
            ).unwrap_or(false);

            if !column_exists {
                return Ok(Vec::new());
            }
            "SELECT id, txid FROM transactions WHERE broadcast_status = 'pending' AND timestamp < ?1"
        };

        let mut stmt = self.conn.prepare(query)?;

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
        let mut stmt = self.conn.prepare("SELECT txid FROM transactions ORDER BY timestamp DESC")?;
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
