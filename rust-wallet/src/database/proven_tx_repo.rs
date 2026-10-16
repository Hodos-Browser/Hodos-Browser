//! Repository for immutable proven transaction records
//!
//! Handles storage and retrieval of proven transactions — confirmed transactions
//! with their merkle proofs. Records are IMMUTABLE: once inserted, they are never
//! updated or deleted. This matches the wallet-toolbox proven_txs table pattern.

use crate::cache_errors::{CacheError, CacheResult};
use super::models::ProvenTx;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ProvenTxRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ProvenTxRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Insert a proof our chain verified (B5-T1-P7a-2: only `proof_store::store` calls this), or
    /// return the existing ID if txid already exists (INSERT OR IGNORE + SELECT).
    pub fn insert_verified(
        &self,
        proof: &crate::proof_store::VerifiedProof,
        merkle_path: &[u8],
        raw_tx: &[u8],
        block_hash: &str,
    ) -> CacheResult<i64> {
        self.insert_or_get_raw(proof.txid(), proof.height(), proof.tx_index(), merkle_path, raw_tx, block_hash, "")
    }

    /// Test-only: plant a row with no check (fixtures for unverified and wrong proofs).
    #[cfg(test)]
    pub fn insert_or_get(
        &self,
        txid: &str,
        height: u32,
        tx_index: u64,
        merkle_path: &[u8],
        raw_tx: &[u8],
        block_hash: &str,
        merkle_root: &str,
    ) -> CacheResult<i64> {
        self.insert_or_get_raw(txid, height, tx_index, merkle_path, raw_tx, block_hash, merkle_root)
    }

    fn insert_or_get_raw(
        &self,
        txid: &str,
        height: u32,
        tx_index: u64,
        merkle_path: &[u8],
        raw_tx: &[u8],
        block_hash: &str,
        merkle_root: &str,
    ) -> CacheResult<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Try to insert — will be ignored if txid already exists (UNIQUE constraint)
        self.conn.execute(
            "INSERT OR IGNORE INTO proven_txs
             (txid, height, tx_index, merkle_path, raw_tx, block_hash, merkle_root, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![txid, height as i64, tx_index as i64, merkle_path, raw_tx, block_hash, merkle_root, now, now],
        )?;

        // Always SELECT to get the ID (whether just inserted or already existed)
        let id = self.conn.query_row(
            "SELECT provenTxId FROM proven_txs WHERE txid = ?1",
            rusqlite::params![txid],
            |row| row.get::<_, i64>(0),
        )?;

        Ok(id)
    }

    /// Get a proven transaction by TXID
    pub fn get_by_txid(&self, txid: &str) -> CacheResult<Option<ProvenTx>> {
        let mut stmt = self.conn.prepare(
            "SELECT provenTxId, txid, height, tx_index, merkle_path, raw_tx,
                    block_hash, merkle_root, created_at, updated_at
             FROM proven_txs
             WHERE txid = ?1"
        )?;

        let result = stmt.query_row(rusqlite::params![txid], |row| {
            Ok(ProvenTx {
                proven_tx_id: row.get(0)?,
                txid: row.get(1)?,
                height: row.get::<_, i64>(2)? as u32,
                tx_index: row.get::<_, i64>(3)? as u64,
                merkle_path: row.get(4)?,
                raw_tx: row.get(5)?,
                block_hash: row.get(6)?,
                merkle_root: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        });

        match result {
            Ok(ptx) => Ok(Some(ptx)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Get a proven transaction by ID
    pub fn get_by_id(&self, id: i64) -> CacheResult<Option<ProvenTx>> {
        let mut stmt = self.conn.prepare(
            "SELECT provenTxId, txid, height, tx_index, merkle_path, raw_tx,
                    block_hash, merkle_root, created_at, updated_at
             FROM proven_txs
             WHERE provenTxId = ?1"
        )?;

        let result = stmt.query_row(rusqlite::params![id], |row| {
            Ok(ProvenTx {
                proven_tx_id: row.get(0)?,
                txid: row.get(1)?,
                height: row.get::<_, i64>(2)? as u32,
                tx_index: row.get::<_, i64>(3)? as u64,
                merkle_path: row.get(4)?,
                raw_tx: row.get(5)?,
                block_hash: row.get(6)?,
                merkle_root: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        });

        match result {
            Ok(ptx) => Ok(Some(ptx)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Get the merkle proof for a txid as a TSC JSON Value, ready for BEEF construction.
    ///
    /// Returns the deserialized merkle_path BLOB as a serde_json::Value containing:
    /// { "index": u64, "target": String, "nodes": [String], "height": u32 }
    ///
    /// If the BLOB JSON is missing the "height" field (legacy data from old code paths
    /// that stored raw TSC without height), the height is injected from the proven_txs
    /// `height` column which always has the correct value.
    pub fn get_merkle_proof_as_tsc(&self, txid: &str) -> CacheResult<Option<serde_json::Value>> {
        let mut stmt = self.conn.prepare(
            "SELECT merkle_path, height FROM proven_txs WHERE txid = ?1"
        )?;

        let result = stmt.query_row(rusqlite::params![txid], |row| {
            let blob: Vec<u8> = row.get(0)?;
            let height: i64 = row.get(1)?;
            Ok((blob, height))
        });

        match result {
            Ok((blob, height)) => {
                let mut tsc: serde_json::Value = serde_json::from_slice(&blob)?;
                // WoC sometimes returns TSC proofs as JSON arrays — normalize to object
                if tsc.is_array() {
                    if let Some(first) = tsc.as_array().and_then(|a| a.first()).cloned() {
                        log::info!("   ℹ️  Normalized array TSC proof to object for {}", txid);
                        tsc = first;
                    }
                }
                // Inject height from column if missing from the BLOB JSON.
                // Standard TSC format (BRC-61) doesn't include height, but BEEF
                // building requires it for BUMP construction.
                if tsc.get("height").and_then(|h| h.as_u64()).is_none() {
                    if let Some(obj) = tsc.as_object_mut() {
                        obj.insert("height".to_string(), serde_json::json!(height as u64));
                        log::info!("   ℹ️  Injected height {} into TSC proof for {}", height, txid);
                    }
                }
                Ok(Some(tsc))
            },
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Restore: fill a row's stripped `merkle_path` with a proof our chain verified — only when the
    /// row is empty and the proof agrees with the row's height and index (B5-T1-P7a-2 `C11`; only
    /// `proof_store::fill_restored_proof` calls this). Returns rows updated.
    pub fn fill_empty_merkle_path(&self, proof: &crate::proof_store::VerifiedProof, merkle_path: &[u8]) -> CacheResult<usize> {
        Ok(self.conn.execute(
            "UPDATE proven_txs SET merkle_path = ?1, height = ?3
             WHERE txid = ?2 AND (height = ?3 OR height = 0) AND tx_index = ?4 AND (merkle_path IS NULL OR LENGTH(merkle_path) = 0)",
            rusqlite::params![merkle_path, proof.txid(), proof.height() as i64, proof.tx_index() as i64],
        )?)
    }

    /// Restore: fill a row's stripped `raw_tx` (the caller has checked the bytes hash to `txid`).
    pub fn fill_empty_raw_tx(&self, txid: &str, raw_tx: &[u8]) -> CacheResult<usize> {
        Ok(self.conn.execute(
            "UPDATE proven_txs SET raw_tx = ?1 WHERE txid = ?2 AND (raw_tx IS NULL OR LENGTH(raw_tx) = 0)",
            rusqlite::params![raw_tx, txid],
        )?)
    }

    /// Link a transaction to its proven_txs record by setting proven_tx_id on the transactions table.
    pub fn link_transaction(&self, txid: &str, proven_tx_id: i64) -> CacheResult<()> {
        self.conn.execute(
            "UPDATE transactions SET proven_tx_id = ?1 WHERE txid = ?2",
            rusqlite::params![proven_tx_id, txid],
        )?;
        Ok(())
    }

    /// Delete a proven_txs record by txid.
    ///
    /// Used to clean up corrupt or invalid merkle proofs (e.g., ARC returned
    /// wrong BUMP data). Also unlinks any transactions referencing this record.
    /// Private (xhigh 168 #11): only `replace_with_verified` deletes a stored proof.
    fn delete_by_txid(&self, txid: &str) -> CacheResult<usize> {
        // Unlink any transactions first
        self.conn.execute(
            "UPDATE transactions SET proven_tx_id = NULL WHERE txid = ?1",
            rusqlite::params![txid],
        )?;
        // ...and any proof request (a foreign key: with foreign_keys=ON the delete fails otherwise —
        // B5-T1-P7a-2 adversarial review).
        self.conn.execute(
            "UPDATE proven_tx_reqs SET proven_tx_id = NULL
             WHERE proven_tx_id IN (SELECT provenTxId FROM proven_txs WHERE txid = ?1)",
            rusqlite::params![txid],
        )?;

        let count = self.conn.execute(
            "DELETE FROM proven_txs WHERE txid = ?1",
            rusqlite::params![txid],
        )?;

        Ok(count)
    }

    /// Replace an existing row with a proof our chain verified (only `proof_store::store` calls this).
    /// Deletes the old record (unlinking its transaction) and inserts the new one.
    pub fn replace_with_verified(
        &self,
        proof: &crate::proof_store::VerifiedProof,
        merkle_path: &[u8],
        raw_tx: &[u8],
        block_hash: &str,
    ) -> CacheResult<i64> {
        // One unit: a failure part-way must not leave the transaction unlinked from both rows.
        self.conn.execute_batch("SAVEPOINT replace_with_verified")?;
        let r = self.delete_by_txid(proof.txid())
            .and_then(|_| self.insert_verified(proof, merkle_path, raw_tx, block_hash));
        let released = match r {
            Ok(_) => self.conn.execute_batch("RELEASE replace_with_verified"),
            Err(_) => self.conn.execute_batch("ROLLBACK TO replace_with_verified; RELEASE replace_with_verified"),
        };
        if let Err(e) = released {
            // Never leave the savepoint open on the shared connection (fix review).
            let _ = self.conn.execute_batch("ROLLBACK TO replace_with_verified; RELEASE replace_with_verified");
            return Err(e.into());
        }
        r
    }
}
