//! Tag repository for database operations
//!
//! Handles CRUD operations for output tags in the database.

use rusqlite::{Connection, Result};
use log::info;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TagRepository<'a> {
    conn: &'a Connection,
}

impl<'a> TagRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        TagRepository { conn }
    }

    /// Find or insert a tag by name
    /// Returns the tag ID
    pub fn find_or_insert(&self, tag: &str) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Try to find existing tag
        let tag_id: Result<i64> = self.conn.query_row(
            "SELECT id FROM output_tags WHERE tag = ?1 AND is_deleted = 0",
            rusqlite::params![tag],
            |row| row.get(0),
        );

        match tag_id {
            Ok(id) => Ok(id),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Insert new tag
                self.conn.execute(
                    "INSERT INTO output_tags (tag, created_at, updated_at, is_deleted) VALUES (?1, ?2, ?3, 0)",
                    rusqlite::params![tag, now, now],
                )?;
                let id = self.conn.last_insert_rowid();
                info!("   ✅ Created new tag '{}' with id {}", tag, id);
                Ok(id)
            }
            Err(e) => Err(e),
        }
    }

    /// Find tag IDs by tag names
    pub fn find_tag_ids(&self, tags: &[String]) -> Result<Vec<i64>> {
        if tags.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders: Vec<String> = (0..tags.len())
            .map(|_| "?".to_string())
            .collect();
        let query = format!(
            "SELECT id FROM output_tags WHERE tag IN ({}) AND is_deleted = 0",
            placeholders.join(",")
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(tags.iter()),
            |row| row.get(0),
        )?;

        let mut tag_ids = Vec::new();
        for row in rows {
            tag_ids.push(row?);
        }

        Ok(tag_ids)
    }

    /// Get tags for an output (returns tag names)
    pub fn get_tags_for_output(&self, output_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT ot.tag
             FROM output_tags ot
             INNER JOIN output_tag_map otm ON ot.id = otm.output_tag_id
             WHERE otm.output_id = ?1 AND otm.is_deleted = 0 AND ot.is_deleted = 0
             ORDER BY ot.tag"
        )?;

        let rows = stmt.query_map(
            rusqlite::params![output_id],
            |row| row.get(0),
        )?;

        let mut tags = Vec::new();
        for row in rows {
            tags.push(row?);
        }

        Ok(tags)
    }

    /// Get tag IDs for an output
    pub fn get_tag_ids_for_output(&self, output_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT otm.output_tag_id
             FROM output_tag_map otm
             INNER JOIN output_tags ot ON ot.id = otm.output_tag_id
             WHERE otm.output_id = ?1 AND otm.is_deleted = 0 AND ot.is_deleted = 0"
        )?;

        let rows = stmt.query_map(
            rusqlite::params![output_id],
            |row| row.get(0),
        )?;

        let mut tag_ids = Vec::new();
        for row in rows {
            tag_ids.push(row?);
        }

        Ok(tag_ids)
    }

    /// Get labels for a transaction (from transaction_labels table)
    pub fn get_labels_for_transaction(&self, transaction_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT label FROM transaction_labels WHERE transaction_id = ?1 ORDER BY label"
        )?;

        let rows = stmt.query_map(
            rusqlite::params![transaction_id],
            |row| row.get(0),
        )?;

        let mut labels = Vec::new();
        for row in rows {
            labels.push(row?);
        }

        Ok(labels)
    }

    /// Get labels for a transaction by txid (looks up transaction_id first)
    pub fn get_labels_for_txid(&self, txid: &str) -> Result<Vec<String>> {
        // First, get transaction_id from transactions table
        let transaction_id: Option<i64> = self.conn.query_row(
            "SELECT id FROM transactions WHERE txid = ?1",
            rusqlite::params![txid],
            |row| row.get(0),
        ).ok();

        if let Some(tx_id) = transaction_id {
            self.get_labels_for_transaction(tx_id)
        } else {
            Ok(Vec::new())  // Transaction not found, return empty labels
        }
    }

    /// Assign a tag to an output
    /// Returns the tag map ID
    pub fn assign_tag_to_output(&self, output_id: i64, tag_name: &str) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Find or create tag
        let tag_id = self.find_or_insert(tag_name)?;

        // Check if mapping already exists
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM output_tag_map WHERE output_id = ?1 AND output_tag_id = ?2 AND is_deleted = 0)",
            rusqlite::params![output_id, tag_id],
            |row| row.get(0),
        )?;

        if exists {
            // Already assigned, return existing mapping ID
            let map_id: i64 = self.conn.query_row(
                "SELECT id FROM output_tag_map WHERE output_id = ?1 AND output_tag_id = ?2 AND is_deleted = 0",
                rusqlite::params![output_id, tag_id],
                |row| row.get(0),
            )?;
            return Ok(map_id);
        }

        // Check if there's a soft-deleted mapping we can restore
        let deleted_id: Option<i64> = self.conn.query_row(
            "SELECT id FROM output_tag_map WHERE output_id = ?1 AND output_tag_id = ?2 AND is_deleted = 1",
            rusqlite::params![output_id, tag_id],
            |row| Ok(Some(row.get(0)?)),
        ).ok().flatten();

        if let Some(map_id) = deleted_id {
            // Restore soft-deleted mapping
            self.conn.execute(
                "UPDATE output_tag_map SET is_deleted = 0, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, map_id],
            )?;
            Ok(map_id)
        } else {
            // Create new mapping
            self.conn.execute(
                "INSERT INTO output_tag_map (output_id, output_tag_id, created_at, updated_at, is_deleted) VALUES (?1, ?2, ?3, ?4, 0)",
                rusqlite::params![output_id, tag_id, now, now],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    /// Remove a tag from an output (soft delete)
    pub fn remove_tag_from_output(&self, output_id: i64, tag_name: &str) -> Result<()> {
        let tag_id = match self.find_tag_ids(&[tag_name.to_string()]) {
            Ok(ids) if !ids.is_empty() => ids[0],
            _ => return Ok(()),  // Tag doesn't exist, nothing to remove
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "UPDATE output_tag_map SET is_deleted = 1, updated_at = ?1 WHERE output_id = ?2 AND output_tag_id = ?3 AND is_deleted = 0",
            rusqlite::params![now, output_id, tag_id],
        )?;

        Ok(())
    }
}
