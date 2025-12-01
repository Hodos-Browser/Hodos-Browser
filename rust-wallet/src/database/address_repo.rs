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
            "INSERT INTO addresses (wallet_id, \"index\", address, public_key, used, balance, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                address.wallet_id,
                address.index,
                address.address,
                address.public_key,
                address.used,
                address.balance,
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
            "SELECT id, wallet_id, \"index\", address, public_key, used, balance, created_at
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
                    created_at: row.get(7)?,
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
            "SELECT id, wallet_id, \"index\", address, public_key, used, balance, created_at
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
                    created_at: row.get(7)?,
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
            "SELECT id, wallet_id, \"index\", address, public_key, used, balance, created_at
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
                    created_at: row.get(7)?,
                })
            },
        )?;

        let mut addresses = Vec::new();
        for addr_result in address_iter {
            addresses.push(addr_result?);
        }

        Ok(addresses)
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
}
