//! Database connection and initialization
//!
//! Handles SQLite database connection, initialization, and basic configuration.

use rusqlite::{Connection, Result};
use std::path::PathBuf;
use log::{info, warn, error};

/// Wallet database connection wrapper
pub struct WalletDatabase {
    conn: Connection,
    db_path: PathBuf,
}

impl WalletDatabase {
    /// Initialize database at the specified path
    ///
    /// Creates database file if it doesn't exist, runs migrations,
    /// and configures SQLite settings (WAL mode, foreign keys, etc.)
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    ///
    /// # Returns
    /// * `Result<Self>` - Database connection or error
    pub fn new(db_path: PathBuf) -> Result<Self> {
        info!("🗄️  Initializing database at: {}", db_path.display());

        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                    Some(format!("Failed to create directory: {}", e))
                ))?;
        }

        // Open or create database
        // If database file exists from a previous failed run, try to open it
        // If it's corrupted, we'll get a clear error
        let conn = match Connection::open(&db_path) {
            Ok(c) => {
                info!("✅ Database connection opened");
                c
            }
            Err(e) => {
                error!("Failed to open database: {}", e);
                // If database file exists but is corrupted, suggest deleting it
                if db_path.exists() {
                    warn!("   Database file exists but may be corrupted");
                    warn!("   You may need to delete: {}", db_path.display());
                }
                return Err(e);
            }
        };

        // Enable WAL mode for better concurrency
        // PRAGMA journal_mode returns a value, so we need to query it
        let journal_mode: String = conn.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
        info!("   WAL mode: {}", journal_mode);

        // Enable foreign keys
        // PRAGMA foreign_keys=ON sets it but doesn't return a value, so use execute()
        conn.execute("PRAGMA foreign_keys=ON", [])?;
        // Verify it's enabled by querying the value
        let foreign_keys: i32 = conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
        info!("   Foreign keys: {}", if foreign_keys != 0 { "enabled" } else { "disabled" });

        // Set busy timeout (wait up to 5 seconds if locked)
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        info!("   Busy timeout set to 5 seconds");

        let db = WalletDatabase {
            conn,
            db_path: db_path.clone(),
        };

        // Run migrations
        info!("📋 Running database migrations...");
        match db.migrate() {
            Ok(()) => {
                info!("✅ Database migrations complete");
            }
            Err(e) => {
                error!("❌ Migration failed with error: {}", e);
                error!("   Error details: {:?}", e);
                return Err(e);
            }
        }

        Ok(db)
    }

    /// Get a reference to the underlying SQLite connection
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Create a new wallet with its first address
    ///
    /// This generates a mnemonic, creates the wallet in the database,
    /// derives the first address using BRC-42, and saves it to the database.
    ///
    /// Returns: (wallet_id, mnemonic_phrase, first_address)
    pub fn create_wallet_with_first_address(&self) -> Result<(i64, String, String)> {
        use super::{WalletRepository, AddressRepository};
        use crate::crypto::brc42::derive_child_public_key;
        use bip39::{Mnemonic, Language};
        use bip32::XPrv;
        use secp256k1::{Secp256k1, SecretKey, PublicKey};
        use sha2::{Sha256, Digest};
        use ripemd::Ripemd160;
        use std::time::{SystemTime, UNIX_EPOCH};
        use hex;
        use bs58;

        info!("🔑 Creating new wallet with first address...");

        // Create repositories
        let wallet_repo = WalletRepository::new(&self.conn);
        let address_repo = AddressRepository::new(&self.conn);

        // Create wallet (generates mnemonic)
        let (wallet_id, mnemonic_phrase) = wallet_repo.create_wallet()?;

        // Parse mnemonic and derive master keys
        let mnemonic = Mnemonic::parse_in(Language::English, &mnemonic_phrase)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Invalid mnemonic: {}", e))
            ))?;

        let seed = mnemonic.to_seed("");
        let master_key = XPrv::new(&seed)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Failed to create master key: {}", e))
            ))?;

        let master_privkey = master_key.private_key().to_bytes();

        // Get master public key
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&master_privkey)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Invalid private key: {}", e))
            ))?;
        let master_pubkey = PublicKey::from_secret_key(&secp, &secret_key).serialize().to_vec();

        // Create BRC-43 invoice number for first address: "2-receive address-0"
        let invoice_number = "2-receive address-0";
        info!("   Invoice number: {}", invoice_number);

        // Derive child public key using BRC-42 (self-derivation)
        let derived_pubkey = derive_child_public_key(&master_privkey, &master_pubkey, invoice_number)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("BRC-42 derivation failed: {}", e))
            ))?;

        info!("   ✅ Derived pubkey: {}", hex::encode(&derived_pubkey));

        // Convert derived public key to Bitcoin address
        let sha_hash = Sha256::digest(&derived_pubkey);
        let pubkey_hash = Ripemd160::digest(&sha_hash);

        let mut addr_bytes = vec![0x00]; // Mainnet prefix
        addr_bytes.extend_from_slice(pubkey_hash.as_slice());

        let checksum_full = Sha256::digest(&Sha256::digest(&addr_bytes));
        let checksum = &checksum_full[0..4];
        addr_bytes.extend_from_slice(checksum);

        let address = bs58::encode(&addr_bytes).into_string();
        info!("   ✅ Generated address: {}", address);

        // Create address in database
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let address_model = super::Address {
            id: None,
            wallet_id,
            index: 0,
            address: address.clone(),
            public_key: hex::encode(&derived_pubkey),
            used: false,
            balance: 0,
            pending_utxo_check: false,  // First address will be checked when cache is empty
            created_at,
        };

        address_repo.create(&address_model)?;

        info!("   ✅ Wallet created successfully with first address");
        Ok((wallet_id, mnemonic_phrase, address))
    }

    /// Get the database file path
    pub fn path(&self) -> &PathBuf {
        &self.db_path
    }

    /// Run database migrations
    ///
    /// Checks current schema version and applies any pending migrations.
    fn migrate(&self) -> Result<()> {
        info!("   Starting migration process...");
        use crate::database::migrations;

        // Create schema_version table if it doesn't exist
        info!("   Step 1: Creating schema_version table...");
        match self.conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            )",
            [],
        ) {
            Ok(rows_affected) => {
                info!("   ✅ schema_version table created/verified (rows affected: {})", rows_affected);
            }
            Err(e) => {
                error!("❌ Failed to create schema_version table: {}", e);
                error!("   Error type: {:?}", e);
                return Err(e);
            }
        }

        // Check current version
        // If table is empty, default to version 0
        info!("   Step 2: Checking current schema version...");
        let current_version: i32 = match self.conn
            .query_row(
                "SELECT MAX(version) FROM schema_version",
                [],
                |row| row.get::<_, Option<i32>>(0),
            ) {
            Ok(Some(version)) => {
                info!("   ✅ Current version found: {}", version);
                version
            }
            Ok(None) => {
                info!("   ✅ No version found (empty table), defaulting to 0");
                0  // Table exists but is empty
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                info!("   ✅ No rows found, defaulting to 0");
                0  // No rows found
            }
            Err(e) => {
                error!("❌ Failed to query schema version: {}", e);
                error!("   Error type: {:?}", e);
                return Err(e);  // Other error
            }
        };

        info!("   Step 3: Current schema version is: {}", current_version);

        info!("   Current schema version: {}", current_version);

        // Apply migrations in order
        if current_version < 1 {
            info!("   Applying migration to version 1...");

            // Test: Try creating just the wallets table first
            info!("   TEST: Creating wallets table only...");
            match self.conn.execute(
                "CREATE TABLE IF NOT EXISTS wallets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    mnemonic TEXT NOT NULL,
                    current_index INTEGER NOT NULL DEFAULT 0,
                    backed_up BOOLEAN NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )",
                [],
            ) {
                Ok(_) => {
                    info!("   ✅ TEST: wallets table created successfully");
                }
                Err(e) => {
                    error!("❌ TEST: Failed to create wallets table: {}", e);
                    error!("   Error type: {:?}", e);
                    return Err(e);
                }
            }

            // If test succeeds, continue with full migration
            match migrations::create_schema_v1(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 1...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (1)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 1 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 1 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 2
        if current_version < 2 {
            info!("   Applying migration to version 2...");
            match migrations::create_schema_v2(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 2...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (2)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 2 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 2 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 3
        if current_version < 3 {
            info!("   Applying migration to version 3...");
            match migrations::create_schema_v3(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 3...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (3)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 3 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 3 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Test database connection
    ///
    /// Performs a simple query to verify the database is working.
    pub fn test_connection(&self) -> Result<()> {
        let version: String = self.conn.query_row(
            "SELECT sqlite_version()",
            [],
            |row| row.get(0),
        )?;
        info!("   SQLite version: {}", version);
        Ok(())
    }
}

impl Drop for WalletDatabase {
    fn drop(&mut self) {
        info!("🔒 Closing database connection");
    }
}
