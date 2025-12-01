//! Simplified migration using execute_batch
//! This is a test to see if execute_batch works better than individual execute() calls

use rusqlite::{Connection, Result};
use log::info;

/// Create schema version 1 using execute_batch
pub fn create_schema_v1_simple(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 1 (using execute_batch)...");

    let sql = r#"
        -- 1. wallets table
        CREATE TABLE IF NOT EXISTS wallets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            mnemonic TEXT NOT NULL,
            current_index INTEGER NOT NULL DEFAULT 0,
            backed_up BOOLEAN NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_wallets_id ON wallets(id);

        -- 2. addresses table
        CREATE TABLE IF NOT EXISTS addresses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            wallet_id INTEGER NOT NULL,
            index INTEGER NOT NULL,
            address TEXT NOT NULL UNIQUE,
            public_key TEXT NOT NULL,
            used BOOLEAN NOT NULL DEFAULT 0,
            balance INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (wallet_id) REFERENCES wallets(id) ON DELETE CASCADE,
            UNIQUE(wallet_id, index)
        );

        CREATE INDEX IF NOT EXISTS idx_addresses_wallet_id ON addresses(wallet_id);
        CREATE INDEX IF NOT EXISTS idx_addresses_address ON addresses(address);
        CREATE INDEX IF NOT EXISTS idx_addresses_index ON addresses(wallet_id, index);
    "#;

    conn.execute_batch(sql)?;
    info!("   ✅ Schema created successfully");
    Ok(())
}
