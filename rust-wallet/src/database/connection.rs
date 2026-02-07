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

        // Create the "default" basket for change outputs (BRC-99 requirement)
        use super::BasketRepository;
        let basket_repo = BasketRepository::new(&self.conn);
        basket_repo.find_or_insert("default")?;
        info!("   ✅ Created 'default' basket for change outputs");

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

        // Also create an entry for the master pubkey address (index -1)
        // This allows UTXO sync to detect payments sent directly to the master pubkey
        let master_sha_hash = Sha256::digest(&master_pubkey);
        let master_pubkey_hash = Ripemd160::digest(&master_sha_hash);

        let mut master_addr_bytes = vec![0x00]; // Mainnet prefix
        master_addr_bytes.extend_from_slice(master_pubkey_hash.as_slice());

        let master_checksum_full = Sha256::digest(&Sha256::digest(&master_addr_bytes));
        let master_checksum = &master_checksum_full[0..4];
        master_addr_bytes.extend_from_slice(master_checksum);

        let master_address = bs58::encode(&master_addr_bytes).into_string();
        info!("   ✅ Master pubkey address: {}", master_address);

        let master_address_model = super::Address {
            id: None,
            wallet_id,
            index: -1,  // Special index for master pubkey address
            address: master_address.clone(),
            public_key: hex::encode(&master_pubkey),
            used: false,
            balance: 0,
            pending_utxo_check: true,  // Check this address for UTXOs
            created_at,
        };

        address_repo.create(&master_address_model)?;
        info!("   ✅ Master pubkey address stored with index -1");

        info!("   ✅ Wallet created successfully with first address and master address");
        Ok((wallet_id, mnemonic_phrase, address))
    }

    /// Ensure the master pubkey address exists in the database
    /// This should be called on startup for existing wallets that were created
    /// before we started storing the master address.
    pub fn ensure_master_address_exists(&self) -> Result<()> {
        use super::{WalletRepository, AddressRepository};
        use crate::database::helpers::get_master_public_key_from_db;
        use sha2::{Sha256, Digest};
        use ripemd::Ripemd160;
        use std::time::{SystemTime, UNIX_EPOCH};
        use bs58;

        info!("🔑 Checking if master pubkey address exists...");

        let wallet_repo = WalletRepository::new(&self.conn);
        let address_repo = AddressRepository::new(&self.conn);

        // Get the primary wallet
        let wallet = match wallet_repo.get_primary_wallet()? {
            Some(w) => w,
            None => {
                info!("   No wallet found, skipping master address check");
                return Ok(());
            }
        };
        let wallet_id = wallet.id.ok_or_else(|| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
            Some("Wallet has no ID".to_string())
        ))?;

        // Check if master address already exists (index -1)
        match address_repo.get_by_wallet_and_index(wallet_id, -1) {
            Ok(Some(_)) => {
                info!("   ✅ Master pubkey address already exists");
                return Ok(());
            }
            Ok(None) => {
                info!("   Master pubkey address not found, creating...");
            }
            Err(e) => {
                info!("   Error checking for master address: {}, will try to create", e);
            }
        }

        // Get master public key
        let master_pubkey = get_master_public_key_from_db(self)?;

        // Calculate master pubkey address
        let sha_hash = Sha256::digest(&master_pubkey);
        let pubkey_hash = Ripemd160::digest(&sha_hash);

        let mut addr_bytes = vec![0x00]; // Mainnet prefix
        addr_bytes.extend_from_slice(pubkey_hash.as_slice());

        let checksum_full = Sha256::digest(&Sha256::digest(&addr_bytes));
        let checksum = &checksum_full[0..4];
        addr_bytes.extend_from_slice(checksum);

        let master_address = bs58::encode(&addr_bytes).into_string();
        info!("   Master pubkey address: {}", master_address);

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let master_address_model = super::Address {
            id: None,
            wallet_id,
            index: -1,  // Special index for master pubkey address
            address: master_address,
            public_key: hex::encode(&master_pubkey),
            used: false,
            balance: 0,
            pending_utxo_check: true,  // Check this address for UTXOs
            created_at,
        };

        address_repo.create(&master_address_model)?;
        info!("   ✅ Master pubkey address created with index -1");

        Ok(())
    }

    /// Ensure the "default" basket exists for wallet change outputs
    ///
    /// This should be called on startup for existing wallets that may have been
    /// created before the default basket was added. Safe to call multiple times.
    pub fn ensure_default_basket_exists(&self) -> Result<()> {
        use super::BasketRepository;

        info!("🧺 Checking if 'default' basket exists...");

        let basket_repo = BasketRepository::new(&self.conn);

        // find_or_insert is idempotent - will create if missing, return existing if present
        basket_repo.find_or_insert("default")?;
        info!("   ✅ 'default' basket exists (created if missing)");

        Ok(())
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

        // Apply migration to version 4
        if current_version < 4 {
            info!("   Applying migration to version 4...");
            match migrations::create_schema_v4(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 4...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (4)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 4 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 4 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 5 (Group C enhancements)
        if current_version < 5 {
            info!("   Applying migration to version 5...");
            match migrations::create_schema_v5(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 5...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (5)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 5 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 5 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 6 (Tag tables for listOutputs)
        if current_version < 6 {
            info!("   Applying migration to version 6...");
            match migrations::create_schema_v6(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 6...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (6)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 6 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 6 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 7 (Certificate Management - Part 3)
        if current_version < 7 {
            info!("   Applying migration to version 7...");
            match migrations::create_schema_v7(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 7...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (7)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 7 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 7 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 8 (BRC-33 Message Relay Persistence)
        if current_version < 8 {
            info!("   Applying migration to version 8...");
            match migrations::create_schema_v8(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 8...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (8)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 8 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 8 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 9 (Basket & Tag Support Enhancements)
        if current_version < 9 {
            info!("   Applying migration to version 9...");
            match migrations::create_schema_v9(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 9...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (9)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 9 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 9 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 10 (Transaction Chaining Support)
        if current_version < 10 {
            info!("   Applying migration to version 10...");
            match migrations::create_schema_v10(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 10...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (10)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 10 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 10 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 11 (Fix Orphan UTXOs)
        if current_version < 11 {
            info!("   Applying migration to version 11...");
            match migrations::create_schema_v11(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 11...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (11)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 11 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 11 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 12 (Basket Output Tracking - nullable address_id)
        if current_version < 12 {
            info!("   Applying migration to version 12...");
            match migrations::create_schema_v12(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 12...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (12)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 12 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 12 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        if current_version < 13 {
            info!("   Applying migration to version 13...");
            match migrations::create_schema_v13(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 13...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (13)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 13 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 13 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        if current_version < 14 {
            info!("   Applying migration to version 14...");
            match migrations::create_schema_v14(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 14...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (14)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 14 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 14 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 15 (Status Consolidation + UnFail)
        if current_version < 15 {
            info!("   Applying migration to version 15...");
            match migrations::create_schema_v15(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 15...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (15)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 15 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 15 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 16 (Proven Transaction Model)
        if current_version < 16 {
            info!("   Applying migration to version 16...");
            match migrations::create_schema_v16(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 16...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (16)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 16 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 16 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 17 (Multi-User Foundation)
        if current_version < 17 {
            info!("   Applying migration to version 17...");
            match migrations::create_schema_v17(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 17...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (17)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 17 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 17 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 18 (Output Model Transition - Phase 4A)
        if current_version < 18 {
            info!("   Applying migration to version 18...");
            match migrations::create_schema_v18(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 18...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (18)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 18 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 18 failed: {}", e);
                    error!("   Error details: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Apply migration to version 19 (Labels, Commissions, Supporting Tables - Phase 5)
        if current_version < 19 {
            info!("   Applying migration to version 19...");
            match migrations::create_schema_v19(&self.conn) {
                Ok(()) => {
                    info!("   Inserting schema version 19...");
                    match self.conn.execute(
                        "INSERT INTO schema_version (version) VALUES (19)",
                        [],
                    ) {
                        Ok(_) => {
                            info!("   ✅ Migration to version 19 complete");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert schema version: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Migration to version 19 failed: {}", e);
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
