//! Transaction label repository for database operations
//!
//! Handles CRUD operations for transaction labels in the database.
//! Phase 5 of wallet-toolbox alignment.
//!
//! ## Schema
//!
//! The label system uses two tables:
//! - `tx_labels`: Deduplicated label entities per user
//! - `tx_labels_map`: Many-to-many mapping between labels and transactions
//!
//! ## Normalization
//!
//! All label names are normalized (trim + lowercase) before storage and lookup.
//! This ensures "Payment", "payment", and "  PAYMENT  " all resolve to
//! the same label ("payment").

use rusqlite::{Connection, Result};
use log::info;
use std::time::{SystemTime, UNIX_EPOCH};

use super::models::TxLabel;

/// Validate and normalize a label name.
///
/// - Trim whitespace
/// - Convert to lowercase
/// - Check length 1-300 bytes (UTF-8)
///
/// # Returns
/// - `Ok(normalized_name)` if valid
/// - `Err(error_message)` if invalid
pub fn validate_and_normalize_label(name: &str) -> std::result::Result<String, String> {
    let normalized = name.trim().to_lowercase();

    if normalized.is_empty() {
        return Err("Label cannot be empty".into());
    }

    let byte_len = normalized.as_bytes().len();
    if byte_len > 300 {
        return Err(format!("Label must be ≤300 bytes, got {}", byte_len));
    }

    Ok(normalized)
}

pub struct TxLabelRepository<'a> {
    conn: &'a Connection,
}

impl<'a> TxLabelRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        TxLabelRepository { conn }
    }

    /// Find or insert a label by name for a user.
    ///
    /// **IMPORTANT**: This function normalizes the input name (trim + lowercase).
    /// This makes the function idempotent: calling with "Payment", "payment",
    /// or "  PAYMENT  " will all resolve to the same label.
    ///
    /// # Returns
    /// The label ID (existing or newly created)
    pub fn find_or_insert(&self, user_id: i64, label: &str) -> Result<i64> {
        let normalized_label = label.trim().to_lowercase();

        if normalized_label.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "Label cannot be empty".to_string()
            ));
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Try to find existing label
        let label_id: Result<i64> = self.conn.query_row(
            "SELECT txLabelId FROM tx_labels WHERE label = ?1 AND user_id = ?2 AND is_deleted = 0",
            rusqlite::params![normalized_label, user_id],
            |row| row.get(0),
        );

        match label_id {
            Ok(id) => Ok(id),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Check for soft-deleted label to restore
                let deleted_id: Option<i64> = self.conn.query_row(
                    "SELECT txLabelId FROM tx_labels WHERE label = ?1 AND user_id = ?2 AND is_deleted = 1",
                    rusqlite::params![normalized_label, user_id],
                    |row| Ok(Some(row.get(0)?)),
                ).ok().flatten();

                if let Some(id) = deleted_id {
                    // Restore soft-deleted label
                    self.conn.execute(
                        "UPDATE tx_labels SET is_deleted = 0, updated_at = ?1 WHERE txLabelId = ?2",
                        rusqlite::params![now, id],
                    )?;
                    Ok(id)
                } else {
                    // Insert new label
                    self.conn.execute(
                        "INSERT INTO tx_labels (user_id, label, is_deleted, created_at, updated_at)
                         VALUES (?1, ?2, 0, ?3, ?4)",
                        rusqlite::params![user_id, normalized_label, now, now],
                    )?;
                    let id = self.conn.last_insert_rowid();
                    info!("   ✅ Created new label '{}' with id {}", normalized_label, id);
                    Ok(id)
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Get a label by ID
    pub fn get_by_id(&self, label_id: i64) -> Result<Option<TxLabel>> {
        self.conn.query_row(
            "SELECT txLabelId, user_id, label, is_deleted, created_at, updated_at
             FROM tx_labels WHERE txLabelId = ?1",
            rusqlite::params![label_id],
            |row| Ok(TxLabel {
                tx_label_id: Some(row.get(0)?),
                user_id: row.get(1)?,
                label: row.get(2)?,
                is_deleted: row.get::<_, i32>(3)? != 0,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            }),
        ).optional()
    }

    /// Find label IDs by label names for a user.
    ///
    /// **Note**: This function normalizes input names for lookup.
    pub fn find_label_ids(&self, user_id: i64, labels: &[String]) -> Result<Vec<i64>> {
        if labels.is_empty() {
            return Ok(Vec::new());
        }

        // Normalize all labels for lookup
        let normalized_labels: Vec<String> = labels.iter()
            .map(|l| l.trim().to_lowercase())
            .collect();

        let placeholders: Vec<String> = (0..normalized_labels.len())
            .map(|_| "?".to_string())
            .collect();

        let query = format!(
            "SELECT txLabelId FROM tx_labels
             WHERE label IN ({}) AND user_id = ? AND is_deleted = 0",
            placeholders.join(",")
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = normalized_labels
            .iter()
            .map(|s| Box::new(s.clone()) as Box<dyn rusqlite::ToSql>)
            .collect();
        params.push(Box::new(user_id));

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;

        let mut label_ids = Vec::new();
        for row in rows {
            label_ids.push(row?);
        }

        Ok(label_ids)
    }

    /// Get all labels for a transaction
    pub fn get_labels_for_transaction(&self, transaction_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT tl.label
             FROM tx_labels tl
             INNER JOIN tx_labels_map tlm ON tl.txLabelId = tlm.txLabelId
             WHERE tlm.transaction_id = ?1 AND tlm.is_deleted = 0 AND tl.is_deleted = 0
             ORDER BY tl.label"
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

    /// Get labels for a transaction by txid
    pub fn get_labels_for_txid(&self, txid: &str) -> Result<Vec<String>> {
        let transaction_id: Option<i64> = self.conn.query_row(
            "SELECT id FROM transactions WHERE txid = ?1",
            rusqlite::params![txid],
            |row| row.get(0),
        ).ok();

        if let Some(tx_id) = transaction_id {
            self.get_labels_for_transaction(tx_id)
        } else {
            Ok(Vec::new())
        }
    }

    /// Assign a label to a transaction
    ///
    /// Returns true if the mapping was created (or already existed)
    pub fn assign_label_to_transaction(&self, user_id: i64, transaction_id: i64, label: &str) -> Result<bool> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Find or create label
        let label_id = self.find_or_insert(user_id, label)?;

        // Check if mapping already exists (not deleted)
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tx_labels_map
             WHERE txLabelId = ?1 AND transaction_id = ?2 AND is_deleted = 0)",
            rusqlite::params![label_id, transaction_id],
            |row| row.get(0),
        )?;

        if exists {
            return Ok(true);
        }

        // Check for soft-deleted mapping to restore
        let deleted_exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tx_labels_map
             WHERE txLabelId = ?1 AND transaction_id = ?2 AND is_deleted = 1)",
            rusqlite::params![label_id, transaction_id],
            |row| row.get(0),
        )?;

        if deleted_exists {
            // Restore soft-deleted mapping
            self.conn.execute(
                "UPDATE tx_labels_map SET is_deleted = 0, updated_at = ?1
                 WHERE txLabelId = ?2 AND transaction_id = ?3",
                rusqlite::params![now, label_id, transaction_id],
            )?;
        } else {
            // Create new mapping
            self.conn.execute(
                "INSERT INTO tx_labels_map (txLabelId, transaction_id, is_deleted, created_at, updated_at)
                 VALUES (?1, ?2, 0, ?3, ?4)",
                rusqlite::params![label_id, transaction_id, now, now],
            )?;
        }

        Ok(true)
    }

    /// Remove a label from a transaction (soft delete)
    pub fn remove_label_from_transaction(&self, user_id: i64, transaction_id: i64, label: &str) -> Result<bool> {
        let label_ids = self.find_label_ids(user_id, &[label.to_string()])?;
        if label_ids.is_empty() {
            return Ok(false); // Label doesn't exist
        }

        let label_id = label_ids[0];
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let updated = self.conn.execute(
            "UPDATE tx_labels_map SET is_deleted = 1, updated_at = ?1
             WHERE txLabelId = ?2 AND transaction_id = ?3 AND is_deleted = 0",
            rusqlite::params![now, label_id, transaction_id],
        )?;

        Ok(updated > 0)
    }

    /// Assign multiple labels to a transaction
    pub fn assign_labels_to_transaction(&self, user_id: i64, transaction_id: i64, labels: &[String]) -> Result<()> {
        for label in labels {
            self.assign_label_to_transaction(user_id, transaction_id, label)?;
        }
        Ok(())
    }

    /// Get all active labels for a user
    pub fn get_all_labels(&self, user_id: i64) -> Result<Vec<TxLabel>> {
        let mut stmt = self.conn.prepare(
            "SELECT txLabelId, user_id, label, is_deleted, created_at, updated_at
             FROM tx_labels
             WHERE user_id = ?1 AND is_deleted = 0
             ORDER BY label"
        )?;

        let rows = stmt.query_map(
            rusqlite::params![user_id],
            |row| Ok(TxLabel {
                tx_label_id: Some(row.get(0)?),
                user_id: row.get(1)?,
                label: row.get(2)?,
                is_deleted: row.get::<_, i32>(3)? != 0,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            }),
        )?;

        let mut labels = Vec::new();
        for row in rows {
            labels.push(row?);
        }

        Ok(labels)
    }

    /// Soft delete a label (marks label and all mappings as deleted)
    pub fn delete_label(&self, label_id: i64) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Soft delete the label
        self.conn.execute(
            "UPDATE tx_labels SET is_deleted = 1, updated_at = ?1 WHERE txLabelId = ?2",
            rusqlite::params![now, label_id],
        )?;

        // Soft delete all mappings for this label
        self.conn.execute(
            "UPDATE tx_labels_map SET is_deleted = 1, updated_at = ?1 WHERE txLabelId = ?2",
            rusqlite::params![now, label_id],
        )?;

        Ok(())
    }
}

// Add the optional() trait for Result<T>
trait ResultExt<T> {
    fn optional(self) -> Result<Option<T>>;
}

impl<T> ResultExt<T> for Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
