//! Basket repository for database operations
//!
//! Handles CRUD operations for baskets in the database.
//!
//! ## Normalization
//!
//! All basket names are normalized (trim + lowercase) before storage and lookup.
//! This ensures "Game Items", "game items", and "  GAME ITEMS  " all resolve to
//! the same basket ("game items").
//!
//! ## Reserved Names (BRC-99)
//!
//! - `"default"` - Reserved for internal wallet change basket
//! - Names starting with `"p "` (p + space) - Reserved for permissioned baskets

use rusqlite::{Connection, Result};
use log::info;
use std::time::{SystemTime, UNIX_EPOCH};
use super::models::Basket;

/// Validate and normalize a basket name per BRC-100 and BRC-99 specifications.
///
/// Based on ts-brc100 reference implementation (validationHelpers.ts):
/// - Trim whitespace
/// - Convert to lowercase
/// - Check length 1-300 bytes (UTF-8)
/// - Reject "default" (reserved for wallet change)
/// - Reject names starting with "p " (reserved for permissioned baskets per BRC-99)
///
/// # Returns
/// - `Ok(normalized_name)` if valid
/// - `Err(error_message)` if invalid
///
/// # Examples
/// ```
/// assert_eq!(validate_and_normalize_basket_name("Game Items").unwrap(), "game items");
/// assert!(validate_and_normalize_basket_name("default").is_err());
/// assert!(validate_and_normalize_basket_name("p mytoken").is_err());
/// assert!(validate_and_normalize_basket_name("payment").is_ok()); // "p" without space is valid
/// ```
pub fn validate_and_normalize_basket_name(name: &str) -> std::result::Result<String, String> {
    // Normalize: trim and lowercase (matches ts-brc100 validateIdentifier)
    let normalized = name.trim().to_lowercase();

    // Check empty after trimming
    if normalized.is_empty() {
        return Err("Basket name cannot be empty".into());
    }

    // Check length in UTF-8 bytes (BRC-100: BasketStringUnder300Bytes)
    let byte_len = normalized.as_bytes().len();
    if byte_len > 300 {
        return Err(format!("Basket name must be ≤300 bytes, got {}", byte_len));
    }

    // Reserved name: "default" (used internally for wallet change basket)
    if normalized == "default" {
        return Err("Basket name 'default' is reserved for wallet change outputs".into());
    }

    // Reserved prefix: "p " (BRC-99 permissioned baskets)
    // Note: It's "p " (p + space), NOT just "p"
    if normalized.starts_with("p ") {
        return Err("Basket names starting with 'p ' are reserved for permissioned baskets (BRC-99)".into());
    }

    Ok(normalized)
}

pub struct BasketRepository<'a> {
    conn: &'a Connection,
}

impl<'a> BasketRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        BasketRepository { conn }
    }

    /// Find or insert a basket by name.
    ///
    /// **IMPORTANT**: This function normalizes the input name (trim + lowercase).
    /// This makes the function idempotent: calling with "Game Items", "game items",
    /// or "  GAME ITEMS  " will all resolve to the same basket.
    ///
    /// Callers do NOT need to pre-normalize - just pass the raw user input.
    /// However, callers SHOULD validate with `validate_and_normalize_basket_name()`
    /// first to provide clear error messages for reserved names.
    ///
    /// # Returns
    /// The basket ID (existing or newly created)
    pub fn find_or_insert(&self, name: &str, user_id: i64) -> Result<i64> {
        // Always normalize - makes function idempotent
        // trim().to_lowercase() is itself idempotent, so double-normalization is safe
        let normalized_name = name.trim().to_lowercase();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Try to find existing basket with normalized name for this user
        let basket_id: Result<i64> = self.conn.query_row(
            "SELECT basketId FROM output_baskets WHERE name = ?1 AND user_id = ?2",
            rusqlite::params![normalized_name, user_id],
            |row| row.get(0),
        );

        match basket_id {
            Ok(id) => {
                // Update updated_at timestamp
                self.conn.execute(
                    "UPDATE output_baskets SET updated_at = ?1 WHERE basketId = ?2",
                    rusqlite::params![now, id],
                )?;
                Ok(id)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Insert new basket with normalized name
                self.conn.execute(
                    "INSERT INTO output_baskets (user_id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![user_id, normalized_name, now, now],
                )?;
                let id = self.conn.last_insert_rowid();
                info!("   ✅ Created new basket '{}' with id {} for user {}", normalized_name, id, user_id);
                Ok(id)
            }
            Err(e) => Err(e),
        }
    }

    /// Find basket by name.
    ///
    /// **Note**: This function normalizes the input name for lookup.
    pub fn find_by_name(&self, name: &str) -> Result<Option<Basket>> {
        // Normalize for lookup
        let normalized_name = name.trim().to_lowercase();

        let mut stmt = self.conn.prepare(
            "SELECT basketId, user_id, name, description, token_type, protocol_id, is_deleted, created_at, updated_at
             FROM output_baskets WHERE name = ?1"
        )?;

        let basket = stmt.query_row(
            rusqlite::params![normalized_name],
            |row| {
                Ok(Basket {
                    id: Some(row.get(0)?),
                    user_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    token_type: row.get(4)?,
                    protocol_id: row.get(5)?,
                    is_deleted: row.get::<_, i32>(6)? != 0,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        );

        match basket {
            Ok(b) => Ok(Some(b)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get basket by ID
    pub fn get_by_id(&self, id: i64) -> Result<Option<Basket>> {
        let mut stmt = self.conn.prepare(
            "SELECT basketId, user_id, name, description, token_type, protocol_id, is_deleted, created_at, updated_at
             FROM output_baskets WHERE basketId = ?1"
        )?;

        let basket = stmt.query_row(
            rusqlite::params![id],
            |row| {
                Ok(Basket {
                    id: Some(row.get(0)?),
                    user_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    token_type: row.get(4)?,
                    protocol_id: row.get(5)?,
                    is_deleted: row.get::<_, i32>(6)? != 0,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        );

        match basket {
            Ok(b) => Ok(Some(b)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
