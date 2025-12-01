//! Wallet repository for database operations
//!
//! Handles CRUD operations for wallets in the database.

use rusqlite::{Connection, Result};
use log::{info, error};
use bip39::{Mnemonic, Language};
use std::time::{SystemTime, UNIX_EPOCH};
use rand::RngCore;
use super::models::Wallet;

pub struct WalletRepository<'a> {
    conn: &'a Connection,
}

impl<'a> WalletRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        WalletRepository { conn }
    }

    /// Create a new wallet with a generated mnemonic
    /// Returns the wallet ID and the mnemonic phrase
    pub fn create_wallet(&self) -> Result<(i64, String)> {
        info!("   Creating new wallet in database...");

        // Generate new mnemonic (12 words = 128 bits of entropy)
        // bip39 2.0 API: generate entropy first, then create mnemonic from it
        use rand::RngCore;
        let mut entropy = [0u8; 16]; // 16 bytes = 128 bits = 12 words
        rand::thread_rng().fill_bytes(&mut entropy);

        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Failed to generate mnemonic: {}", e))
            ))?;

        let mnemonic_phrase = mnemonic.to_string();
        info!("   Generated mnemonic: {}", mnemonic_phrase);

        // Get current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert wallet into database
        self.conn.execute(
            "INSERT INTO wallets (mnemonic, current_index, backed_up, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                mnemonic_phrase,
                0,  // Start with index 0
                false,  // Not backed up yet
                now,
                now,  // updated_at same as created_at initially
            ],
        )?;

        let wallet_id = self.conn.last_insert_rowid();
        info!("   ✅ Wallet created with ID: {}", wallet_id);

        Ok((wallet_id, mnemonic_phrase))
    }

    /// Get wallet by ID
    pub fn get_by_id(&self, wallet_id: i64) -> Result<Option<Wallet>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, mnemonic, current_index, backed_up, created_at
             FROM wallets
             WHERE id = ?1"
        )?;

        let wallet_result = stmt.query_row(
            rusqlite::params![wallet_id],
            |row| {
                Ok(Wallet {
                    id: Some(row.get(0)?),
                    mnemonic: row.get(1)?,
                    current_index: row.get(2)?,
                    backed_up: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        );

        match wallet_result {
            Ok(wallet) => Ok(Some(wallet)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get the first (primary) wallet
    /// In the future, we might support multiple wallets, but for now there's only one
    pub fn get_primary_wallet(&self) -> Result<Option<Wallet>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, mnemonic, current_index, backed_up, created_at
             FROM wallets
             ORDER BY id ASC
             LIMIT 1"
        )?;

        let wallet_result = stmt.query_row(
            [],
            |row| {
                Ok(Wallet {
                    id: Some(row.get(0)?),
                    mnemonic: row.get(1)?,
                    current_index: row.get(2)?,
                    backed_up: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        );

        match wallet_result {
            Ok(wallet) => Ok(Some(wallet)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Update wallet's current_index
    pub fn update_current_index(&self, wallet_id: i64, new_index: i32) -> Result<()> {
        self.conn.execute(
            "UPDATE wallets SET current_index = ?1 WHERE id = ?2",
            rusqlite::params![new_index, wallet_id],
        )?;
        Ok(())
    }

    /// Mark wallet as backed up
    pub fn mark_backed_up(&self, wallet_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE wallets SET backed_up = 1 WHERE id = ?2",
            rusqlite::params![wallet_id],
        )?;
        Ok(())
    }
}
