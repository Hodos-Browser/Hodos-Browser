//! Transaction repository for database operations
//!
//! Handles CRUD operations for transactions in the database.

use rusqlite::{Connection, Result};
use log::{info, error};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::action_storage::{StoredAction, ActionStatus, ActionInput, ActionOutput};

pub struct TransactionRepository<'a> {
    conn: &'a Connection,
}

impl<'a> TransactionRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        TransactionRepository { conn }
    }

    /// Add a new transaction (action) to the database
    pub fn add_transaction(&self, action: &StoredAction) -> Result<i64> {
        // Insert into transactions table
        self.conn.execute(
            "INSERT INTO transactions (
                txid, reference_number, raw_tx, description, status, is_outgoing,
                satoshis, timestamp, block_height, confirmations, version, lock_time
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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

        // Get labels
        let mut label_stmt = self.conn.prepare(
            "SELECT label FROM transaction_labels WHERE transaction_id = ?1"
        )?;
        let labels: Vec<String> = label_stmt.query_map(
            rusqlite::params![transaction_id],
            |row| Ok(row.get(0)?),
        )?.collect::<Result<Vec<_>>>()?;

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

    /// Update transaction status
    pub fn update_status(&self, txid: &str, status: ActionStatus) -> Result<()> {
        self.conn.execute(
            "UPDATE transactions SET status = ?1 WHERE txid = ?2",
            rusqlite::params![status.to_string(), txid],
        )?;
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
