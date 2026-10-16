//! Repository for the verified header chain (`verified_headers`, V29, B5-T1-P7a-1).
//!
//! Rows are a persisted copy of what `header_chain::HeaderChain` accepted. They are **not** an
//! authority: `chain_tracker::ChainTracker::load` re-checks every row against every header rule at
//! startup and deletes any that fail. Hashes are display hex; `header` is the raw 80 bytes.

use rusqlite::{params, Connection, Result};

pub struct VerifiedHeaderRepository<'a> {
    conn: &'a Connection,
}

/// One stored row, as read back for re-validation.
pub struct StoredHeader {
    pub rowid: i64,
    pub hash: String,
    pub header: Vec<u8>,
    pub is_active: bool,
}

impl<'a> VerifiedHeaderRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, hash: &str, height: u32, prev_hash: &str, header: &[u8; 80], chainwork: &[u8; 32], is_active: bool) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO verified_headers (hash, height, prev_hash, header, chainwork, is_active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![hash, height, prev_hash, &header[..], &chainwork[..], is_active as i64],
        )?;
        Ok(())
    }

    pub fn set_active(&self, hash: &str, is_active: bool) -> Result<()> {
        self.conn.execute("UPDATE verified_headers SET is_active = ?2 WHERE hash = ?1", params![hash, is_active as i64])?;
        Ok(())
    }

    pub fn delete(&self, hash: &str) -> Result<()> {
        self.conn.execute("DELETE FROM verified_headers WHERE hash = ?1", params![hash])?;
        Ok(())
    }

    pub fn delete_rowid(&self, rowid: i64) -> Result<()> {
        self.conn.execute("DELETE FROM verified_headers WHERE rowid = ?1", params![rowid])?;
        Ok(())
    }

    /// Every row, parents before children (lowest height first); at each height the row that was
    /// active comes first, so an equal-work tie keeps its winner across a restart.
    /// A row that cannot be read as the right types is returned as `Err(rowid)` on its own, so one bad
    /// row never hides the rest (`/code-review max 159` V20).
    pub fn load_all(&self) -> Result<Vec<std::result::Result<StoredHeader, i64>>> {
        let mut stmt = self.conn.prepare("SELECT rowid, hash, header, is_active FROM verified_headers ORDER BY height ASC, is_active DESC, hash ASC")?;
        let rows = stmt.query_map([], |r| {
            let rowid: i64 = r.get(0)?;
            let row = (|| -> Result<StoredHeader> {
                Ok(StoredHeader { rowid, hash: r.get(1)?, header: r.get(2)?, is_active: r.get::<_, i64>(3)? != 0 })
            })();
            Ok(row.map_err(|_| rowid))
        })?;
        rows.collect()
    }
}
