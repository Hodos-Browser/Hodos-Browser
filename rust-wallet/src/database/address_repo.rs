//! Address repository for database operations
//!
//! Handles CRUD operations for addresses in the database.

use rusqlite::{Connection, Result};
use log::{info, error};
use std::time::{SystemTime, UNIX_EPOCH};
use super::models::Address;

pub struct AddressRepository<'a> {
    conn: &'a Connection,
}

impl<'a> AddressRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        AddressRepository { conn }
    }

    /// Create a new address in the database
    pub fn create(&self, address: &Address) -> Result<i64> {
        info!("   Creating address at index {}: {}", address.index, address.address);

        // Get current timestamp
        let created_at = if address.created_at == 0 {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
        } else {
            address.created_at
        };

        self.conn.execute(
            "INSERT INTO addresses (wallet_id, \"index\", address, public_key, used, balance, pending_utxo_check, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                address.wallet_id,
                address.index,
                address.address,
                address.public_key,
                address.used,
                address.balance,
                address.pending_utxo_check,
                created_at,
            ],
        )?;

        let address_id = self.conn.last_insert_rowid();
        info!("   ✅ Address created with ID: {}", address_id);

        Ok(address_id)
    }

    /// Get address by wallet ID and index
    pub fn get_by_wallet_and_index(&self, wallet_id: i64, index: i32) -> Result<Option<Address>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, wallet_id, \"index\", address, public_key, used, balance, pending_utxo_check, created_at
             FROM addresses
             WHERE wallet_id = ?1 AND \"index\" = ?2"
        )?;

        let address_result = stmt.query_row(
            rusqlite::params![wallet_id, index],
            |row| {
                Ok(Address {
                    id: Some(row.get(0)?),
                    wallet_id: row.get(1)?,
                    index: row.get(2)?,
                    address: row.get(3)?,
                    public_key: row.get(4)?,
                    used: row.get(5)?,
                    balance: row.get(6)?,
                    pending_utxo_check: row.get(7)?,
                    created_at: row.get(8)?,
                })
            },
        );

        match address_result {
            Ok(addr) => Ok(Some(addr)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get address by address string
    pub fn get_by_address(&self, address_str: &str) -> Result<Option<Address>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, wallet_id, \"index\", address, public_key, used, balance, pending_utxo_check, created_at
             FROM addresses
             WHERE address = ?1"
        )?;

        let address_result = stmt.query_row(
            rusqlite::params![address_str],
            |row| {
                Ok(Address {
                    id: Some(row.get(0)?),
                    wallet_id: row.get(1)?,
                    index: row.get(2)?,
                    address: row.get(3)?,
                    public_key: row.get(4)?,
                    used: row.get(5)?,
                    balance: row.get(6)?,
                    pending_utxo_check: row.get(7)?,
                    created_at: row.get(8)?,
                })
            },
        );

        match address_result {
            Ok(addr) => Ok(Some(addr)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get all addresses for a wallet
    pub fn get_all_by_wallet(&self, wallet_id: i64) -> Result<Vec<Address>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, wallet_id, \"index\", address, public_key, used, balance, pending_utxo_check, created_at
             FROM addresses
             WHERE wallet_id = ?1
             ORDER BY \"index\" ASC"
        )?;

        let address_iter = stmt.query_map(
            rusqlite::params![wallet_id],
            |row| {
                Ok(Address {
                    id: Some(row.get(0)?),
                    wallet_id: row.get(1)?,
                    index: row.get(2)?,
                    address: row.get(3)?,
                    public_key: row.get(4)?,
                    used: row.get(5)?,
                    balance: row.get(6)?,
                    pending_utxo_check: row.get(7)?,
                    created_at: row.get(8)?,
                })
            },
        )?;

        let mut addresses = Vec::new();
        for addr_result in address_iter {
            addresses.push(addr_result?);
        }

        Ok(addresses)
    }

    /// Get the maximum address index for a wallet (excluding special indices like -1)
    ///
    /// This is more reliable than trusting wallet.current_index, which can get out of sync.
    /// Returns None if no addresses exist (excluding index -1).
    pub fn get_max_index(&self, wallet_id: i64) -> Result<Option<i32>> {
        let result: rusqlite::Result<i32> = self.conn.query_row(
            "SELECT MAX(\"index\") FROM addresses WHERE wallet_id = ?1 AND \"index\" >= 0",
            rusqlite::params![wallet_id],
            |row| row.get(0),
        );

        match result {
            Ok(max_index) => Ok(Some(max_index)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(rusqlite::Error::InvalidColumnType(_, _, rusqlite::types::Type::Null)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Update address balance
    pub fn update_balance(&self, address_id: i64, balance: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE addresses SET balance = ?1 WHERE id = ?2",
            rusqlite::params![balance, address_id],
        )?;
        Ok(())
    }

    /// Mark address as used
    pub fn mark_used(&self, address_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE addresses SET used = 1 WHERE id = ?1",
            rusqlite::params![address_id],
        )?;
        Ok(())
    }

    /// Get all addresses that need UTXO checking (pending_utxo_check = 1 OR master address)
    /// The master pubkey address (index = -1) is always checked regardless of pending flag
    pub fn get_pending_utxo_check(&self, wallet_id: i64) -> Result<Vec<Address>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, wallet_id, \"index\", address, public_key, used, balance, pending_utxo_check, created_at
             FROM addresses
             WHERE wallet_id = ?1 AND (pending_utxo_check = 1 OR \"index\" = -1)
             ORDER BY \"index\" ASC"
        )?;

        let address_iter = stmt.query_map(
            rusqlite::params![wallet_id],
            |row| {
                Ok(Address {
                    id: Some(row.get(0)?),
                    wallet_id: row.get(1)?,
                    index: row.get(2)?,
                    address: row.get(3)?,
                    public_key: row.get(4)?,
                    used: row.get(5)?,
                    balance: row.get(6)?,
                    pending_utxo_check: row.get(7)?,
                    created_at: row.get(8)?,
                })
            },
        )?;

        let mut addresses = Vec::new();
        for addr_result in address_iter {
            addresses.push(addr_result?);
        }

        Ok(addresses)
    }

    /// Clear pending_utxo_check flag for an address (after checking UTXOs)
    pub fn clear_pending_utxo_check(&self, address_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE addresses SET pending_utxo_check = 0 WHERE id = ?1",
            rusqlite::params![address_id],
        )?;
        Ok(())
    }

    /// Clear pending_utxo_check flag for multiple addresses
    pub fn clear_pending_utxo_check_batch(&self, address_ids: &[i64]) -> Result<()> {
        if address_ids.is_empty() {
            return Ok(());
        }

        // Use a transaction for batch update
        let tx = self.conn.unchecked_transaction()?;
        for &address_id in address_ids {
            tx.execute(
                "UPDATE addresses SET pending_utxo_check = 0 WHERE id = ?1",
                rusqlite::params![address_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Set pending_utxo_check flag for ALL addresses in a wallet.
    /// Used by wallet rescan to restart the monitoring window.
    pub fn set_all_pending_utxo_check(&self, wallet_id: i64) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE addresses SET pending_utxo_check = 1, created_at = ?1
             WHERE wallet_id = ?2 AND (\"index\" >= 0 OR \"index\" = -1)",
            rusqlite::params![now, wallet_id],
        )?;

        if rows_affected > 0 {
            info!("   🔄 Re-enabled UTXO monitoring for {} address(es)", rows_affected);
        }

        Ok(rows_affected)
    }

    /// Clear pending_utxo_check flag for addresses older than max_age_hours
    ///
    /// This prevents addresses from being pending forever if no UTXOs are ever
    /// received. The address can still receive UTXOs later - they'll be detected
    /// by periodic UTXO sync.
    ///
    /// # Arguments
    /// * `max_age_hours` - Maximum hours an address can remain pending
    ///
    /// # Returns
    /// Number of addresses that had their pending flag cleared
    pub fn clear_stale_pending_addresses(&self, max_age_hours: i64) -> Result<usize> {
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 - (max_age_hours * 3600);

        let rows_affected = self.conn.execute(
            "UPDATE addresses SET pending_utxo_check = 0
             WHERE pending_utxo_check = 1 AND created_at < ?1",
            rusqlite::params![cutoff],
        )?;

        if rows_affected > 0 {
            info!("   🧹 Cleared {} stale pending address(es) older than {} hours", rows_affected, max_age_hours);
        }

        Ok(rows_affected)
    }

    /// Get or create a placeholder address for external/custom script outputs.
    ///
    /// This is used for basket insertion outputs where we can't derive keys
    /// (custom scripts, external tokens, etc.) but still need to track the UTXO.
    ///
    /// Uses index = -2 to distinguish from:
    /// - Regular derived addresses (index >= 0)
    /// - Master pubkey address (index = -1)
    ///
    /// # Arguments
    /// * `wallet_id` - The wallet ID to create the address for
    ///
    /// # Returns
    /// The address ID (existing or newly created)
    pub fn get_or_create_external_address(&self, wallet_id: i64) -> Result<i64> {
        // Use index -2 to indicate "external" address
        // (index -1 is already used for master pubkey address)
        let external_index = -2;

        // Try to find existing external address
        let existing: Result<i64> = self.conn.query_row(
            "SELECT id FROM addresses WHERE wallet_id = ?1 AND \"index\" = ?2",
            rusqlite::params![wallet_id, external_index],
            |row| row.get(0),
        );

        match existing {
            Ok(id) => {
                info!("   Using existing external address id={}", id);
                Ok(id)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;

                self.conn.execute(
                    "INSERT INTO addresses (wallet_id, \"index\", address, public_key, used, balance, pending_utxo_check, created_at)
                     VALUES (?1, ?2, 'EXTERNAL', 'EXTERNAL', 0, 0, 0, ?3)",
                    rusqlite::params![wallet_id, external_index, now],
                )?;

                let id = self.conn.last_insert_rowid();
                info!("   ✅ Created external placeholder address with id={}", id);
                Ok(id)
            }
            Err(e) => {
                error!("   ❌ Failed to check for external address: {}", e);
                Err(e)
            }
        }
    }
}
