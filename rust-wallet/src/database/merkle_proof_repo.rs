//! Repository for Merkle proof caching operations
//!
//! Handles CRUD operations for cached Merkle proofs (TSC/BUMP format) used in BEEF building.

use crate::cache_errors::{CacheError, CacheResult};
use super::models::MerkleProof;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct MerkleProofRepository<'a> {
    conn: &'a Connection,
}

impl<'a> MerkleProofRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get cached Merkle proof for a parent transaction (by TXID)
    pub fn get_by_parent_txid(&self, txid: &str) -> CacheResult<Option<MerkleProof>> {
        let mut stmt = self.conn.prepare(
            "SELECT mp.id, mp.parent_txn_id, mp.block_height, mp.tx_index,
                    mp.target_hash, mp.nodes, mp.cached_at
             FROM merkle_proofs mp
             JOIN parent_transactions pt ON mp.parent_txn_id = pt.id
             WHERE pt.txid = ?"
        )?;

        let result = stmt.query_row([txid], |row| {
            let nodes_json: String = row.get(5)?;
            let nodes: Vec<String> = serde_json::from_str(&nodes_json)
                .unwrap_or_default();

            Ok(MerkleProof {
                id: row.get(0)?,
                parent_txn_id: row.get(1)?,
                block_height: row.get(2)?,
                tx_index: row.get(3)?,
                target_hash: row.get(4)?,
                nodes,
                cached_at: row.get(6)?,
            })
        });

        match result {
            Ok(proof) => Ok(Some(proof)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Database(e)),
        }
    }

    /// Cache a Merkle proof (TSC format from WhatsOnChain)
    pub fn upsert(
        &self,
        parent_txn_id: i64,
        block_height: u32,
        tx_index: u64,
        target_hash: &str,
        nodes_json: &str,
    ) -> CacheResult<i64> {
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Use INSERT OR REPLACE to handle duplicates
        self.conn.execute(
            "INSERT OR REPLACE INTO merkle_proofs
             (parent_txn_id, block_height, tx_index, target_hash, nodes, cached_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![parent_txn_id, block_height, tx_index, target_hash, nodes_json, cached_at],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Convert cached proof to TSC JSON format for BEEF building
    pub fn to_tsc_json(&self, proof: &MerkleProof) -> serde_json::Value {
        serde_json::json!({
            "index": proof.tx_index,
            "target": proof.target_hash,
            "nodes": proof.nodes,
            "height": proof.block_height,
        })
    }
}
