//! Repository for proven transaction request lifecycle tracking
//!
//! Tracks the state of proof acquisition for broadcast transactions.
//! Unlike proven_txs (immutable), these records are mutable and progress
//! through a lifecycle: sending → unproven → completed (or failed states).

use crate::cache_errors::{CacheError, CacheResult};
use super::models::ProvenTxReq;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ProvenTxReqRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ProvenTxReqRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    /// Create a new proof request for a broadcast transaction.
    /// Uses INSERT OR IGNORE to avoid duplicates on the UNIQUE txid constraint.
    pub fn create(
        &self,
        txid: &str,
        raw_tx: &[u8],
        input_beef: Option<&[u8]>,
        status: &str,
    ) -> CacheResult<i64> {
        let now = Self::now();

        self.conn.execute(
            "INSERT OR IGNORE INTO proven_tx_reqs
             (txid, raw_tx, input_beef, status, attempts, notified, history, notify, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, 0, '{}', '{}', ?5, ?6)",
            rusqlite::params![txid, raw_tx, input_beef, status, now, now],
        )?;

        // Return the ID (whether just inserted or already existed)
        let id = self.conn.query_row(
            "SELECT provenTxReqId FROM proven_tx_reqs WHERE txid = ?1",
            rusqlite::params![txid],
            |row| row.get::<_, i64>(0),
        )?;

        Ok(id)
    }

    /// Get a proof request by TXID
    pub fn get_by_txid(&self, txid: &str) -> CacheResult<Option<ProvenTxReq>> {
        let mut stmt = self.conn.prepare(
            "SELECT provenTxReqId, proven_tx_id, status, attempts, notified,
                    txid, batch, history, notify, raw_tx, input_beef,
                    created_at, updated_at
             FROM proven_tx_reqs
             WHERE txid = ?1"
        )?;

        let result = stmt.query_row(rusqlite::params![txid], |row| {
            Ok(ProvenTxReq {
                proven_tx_req_id: row.get(0)?,
                proven_tx_id: row.get(1)?,
                status: row.get(2)?,
                attempts: row.get(3)?,
                notified: row.get::<_, i32>(4)? != 0,
                txid: row.get(5)?,
                batch: row.get(6)?,
                history: row.get(7)?,
                notify: row.get(8)?,
                raw_tx: row.get(9)?,
                input_beef: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        });

        match result {
            Ok(req) => Ok(Some(req)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Update the status of a proof request
    pub fn update_status(&self, id: i64, status: &str) -> CacheResult<()> {
        let now = Self::now();
        self.conn.execute(
            "UPDATE proven_tx_reqs SET status = ?1, updated_at = ?2 WHERE provenTxReqId = ?3",
            rusqlite::params![status, now, id],
        )?;
        Ok(())
    }

    /// Increment the attempt counter for a proof request
    pub fn increment_attempts(&self, id: i64) -> CacheResult<()> {
        let now = Self::now();
        self.conn.execute(
            "UPDATE proven_tx_reqs SET attempts = attempts + 1, updated_at = ?1 WHERE provenTxReqId = ?2",
            rusqlite::params![now, id],
        )?;
        Ok(())
    }

    /// Get all pending proof requests (not in terminal states)
    pub fn get_pending(&self) -> CacheResult<Vec<ProvenTxReq>> {
        let mut stmt = self.conn.prepare(
            "SELECT provenTxReqId, proven_tx_id, status, attempts, notified,
                    txid, batch, history, notify, raw_tx, input_beef,
                    created_at, updated_at
             FROM proven_tx_reqs
             WHERE status NOT IN ('completed', 'invalid')
             ORDER BY created_at ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ProvenTxReq {
                proven_tx_req_id: row.get(0)?,
                proven_tx_id: row.get(1)?,
                status: row.get(2)?,
                attempts: row.get(3)?,
                notified: row.get::<_, i32>(4)? != 0,
                txid: row.get(5)?,
                batch: row.get(6)?,
                history: row.get(7)?,
                notify: row.get(8)?,
                raw_tx: row.get(9)?,
                input_beef: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Link a proof request to its proven_txs record once proof is acquired
    pub fn link_proven_tx(&self, id: i64, proven_tx_id: i64) -> CacheResult<()> {
        let now = Self::now();
        self.conn.execute(
            "UPDATE proven_tx_reqs SET proven_tx_id = ?1, updated_at = ?2 WHERE provenTxReqId = ?3",
            rusqlite::params![proven_tx_id, now, id],
        )?;
        Ok(())
    }

    /// Delete a proof request by txid.
    ///
    /// Used when txid changes during two-phase signing: the old proven_tx_req
    /// for the partially-signed txid is stale and would be polled forever.
    pub fn delete_by_txid(&self, txid: &str) -> CacheResult<()> {
        let rows = self.conn.execute(
            "DELETE FROM proven_tx_reqs WHERE txid = ?1",
            rusqlite::params![txid],
        )?;
        if rows > 0 {
            log::info!("   🗑️  Deleted stale proven_tx_req for txid {}", &txid[..std::cmp::min(16, txid.len())]);
        }
        Ok(())
    }

    /// Add a timestamped history note to the proof request's history JSON
    pub fn add_history_note(&self, id: i64, event: &str, details: &str) -> CacheResult<()> {
        let now = Self::now();
        let timestamp_key = now.to_string();

        // Read current history
        let current_history: String = self.conn.query_row(
            "SELECT history FROM proven_tx_reqs WHERE provenTxReqId = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )?;

        // Parse, add entry, serialize back
        let mut history: serde_json::Value = serde_json::from_str(&current_history)
            .unwrap_or_else(|_| serde_json::json!({}));

        let entry = format!("{}: {}", event, details);
        history[&timestamp_key] = serde_json::json!(entry);

        let updated_history = serde_json::to_string(&history)
            .unwrap_or_else(|_| "{}".to_string());

        self.conn.execute(
            "UPDATE proven_tx_reqs SET history = ?1, updated_at = ?2 WHERE provenTxReqId = ?3",
            rusqlite::params![updated_history, now, id],
        )?;

        Ok(())
    }
}
