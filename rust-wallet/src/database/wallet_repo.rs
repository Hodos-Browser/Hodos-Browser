//! Wallet repository for database operations
//!
//! Handles CRUD operations for wallets in the database.

use rusqlite::{Connection, Result};
use log::info;
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
    /// If `pin` is provided, the mnemonic is encrypted before storage.
    /// Also encrypts with DPAPI for Windows auto-unlock (if available).
    /// Returns the wallet ID and the **plaintext** mnemonic phrase (for display to user).
    pub fn create_wallet(&self, pin: Option<&str>) -> Result<(i64, String)> {
        info!("   Creating new wallet in database...");

        // Generate new mnemonic (12 words = 128 bits of entropy)
        let mut entropy = [0u8; 16]; // 16 bytes = 128 bits = 12 words
        rand::thread_rng().fill_bytes(&mut entropy);

        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Failed to generate mnemonic: {}", e))
            ))?;

        let mnemonic_phrase = mnemonic.to_string();

        // Encrypt mnemonic if PIN provided
        let (stored_mnemonic, pin_salt) = if let Some(pin) = pin {
            let (salt_hex, encrypted_hex) = crate::crypto::pin::encrypt_mnemonic(&mnemonic_phrase, pin)
                .map_err(|e| rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                    Some(format!("Failed to encrypt mnemonic: {}", e))
                ))?;
            (encrypted_hex, Some(salt_hex))
        } else {
            (mnemonic_phrase.clone(), None)
        };

        // Encrypt with DPAPI for auto-unlock (non-fatal if unavailable)
        let dpapi_blob = match crate::crypto::dpapi::dpapi_encrypt(mnemonic_phrase.as_bytes()) {
            Ok(blob) => {
                info!("   ✅ DPAPI encryption succeeded ({} bytes)", blob.len());
                Some(blob)
            }
            Err(e) => {
                info!("   ⚠️  DPAPI encryption unavailable: {} — wallet will require PIN on startup", e);
                None
            }
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO wallets (mnemonic, pin_salt, mnemonic_dpapi, current_index, backed_up, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                stored_mnemonic,
                pin_salt,
                dpapi_blob,
                0,
                false,
                now,
                now,
            ],
        )?;

        let wallet_id = self.conn.last_insert_rowid();
        info!("   ✅ Wallet created with ID: {} (PIN: {}, DPAPI: {})", wallet_id, pin.is_some(), dpapi_blob.is_some());

        Ok((wallet_id, mnemonic_phrase))
    }

    /// Get wallet by ID
    pub fn get_by_id(&self, wallet_id: i64) -> Result<Option<Wallet>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, mnemonic, pin_salt, mnemonic_dpapi, current_index, backed_up, created_at
             FROM wallets
             WHERE id = ?1"
        )?;

        let wallet_result = stmt.query_row(
            rusqlite::params![wallet_id],
            |row| {
                Ok(Wallet {
                    id: Some(row.get(0)?),
                    mnemonic: row.get(1)?,
                    pin_salt: row.get(2)?,
                    mnemonic_dpapi: row.get(3)?,
                    current_index: row.get(4)?,
                    backed_up: row.get(5)?,
                    created_at: row.get(6)?,
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
            "SELECT id, mnemonic, pin_salt, mnemonic_dpapi, current_index, backed_up, created_at
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
                    pin_salt: row.get(2)?,
                    mnemonic_dpapi: row.get(3)?,
                    current_index: row.get(4)?,
                    backed_up: row.get(5)?,
                    created_at: row.get(6)?,
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

    /// Create a wallet from an existing mnemonic (recovery flow)
    ///
    /// Validates the mnemonic, inserts with `backed_up = true` (user already has it).
    /// If `pin` is provided, the mnemonic is encrypted before storage.
    /// Also encrypts with DPAPI for Windows auto-unlock (if available).
    /// Returns the wallet ID and the **plaintext** mnemonic phrase.
    pub fn create_wallet_with_mnemonic(&self, mnemonic_phrase: &str, pin: Option<&str>) -> Result<(i64, String)> {
        info!("   Creating wallet from existing mnemonic...");

        // Validate mnemonic
        let mnemonic = Mnemonic::parse_in(Language::English, mnemonic_phrase)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Invalid mnemonic: {}", e))
            ))?;

        let phrase = mnemonic.to_string();

        // Encrypt mnemonic if PIN provided
        let (stored_mnemonic, pin_salt) = if let Some(pin) = pin {
            let (salt_hex, encrypted_hex) = crate::crypto::pin::encrypt_mnemonic(&phrase, pin)
                .map_err(|e| rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                    Some(format!("Failed to encrypt mnemonic: {}", e))
                ))?;
            (encrypted_hex, Some(salt_hex))
        } else {
            (phrase.clone(), None)
        };

        // Encrypt with DPAPI for auto-unlock (non-fatal if unavailable)
        let dpapi_blob = match crate::crypto::dpapi::dpapi_encrypt(phrase.as_bytes()) {
            Ok(blob) => {
                info!("   ✅ DPAPI encryption succeeded ({} bytes)", blob.len());
                Some(blob)
            }
            Err(e) => {
                info!("   ⚠️  DPAPI encryption unavailable: {} — wallet will require PIN on startup", e);
                None
            }
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO wallets (mnemonic, pin_salt, mnemonic_dpapi, current_index, backed_up, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                stored_mnemonic,
                pin_salt,
                dpapi_blob,
                0,
                true,
                now,
                now,
            ],
        )?;

        let wallet_id = self.conn.last_insert_rowid();
        info!("   ✅ Wallet created from mnemonic with ID: {} (PIN: {}, DPAPI: {})", wallet_id, pin.is_some(), dpapi_blob.is_some());

        Ok((wallet_id, phrase))
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
