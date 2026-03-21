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
    /// Cached plaintext mnemonic. None = locked (PIN not entered yet).
    cached_mnemonic: Option<String>,
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
            cached_mnemonic: None,
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

    // ── PIN / mnemonic cache methods ──

    /// Check if the wallet uses PIN protection (has an encrypted mnemonic)
    pub fn is_pin_protected(&self) -> bool {
        use super::WalletRepository;
        let wallet_repo = WalletRepository::new(&self.conn);
        wallet_repo.get_primary_wallet()
            .ok()
            .flatten()
            .map(|w| w.pin_salt.is_some())
            .unwrap_or(false)
    }

    /// Check if the wallet is currently unlocked (mnemonic cached in memory)
    pub fn is_unlocked(&self) -> bool {
        self.cached_mnemonic.is_some()
    }

    /// Unlock the wallet by decrypting the mnemonic with the user's PIN.
    /// Caches the plaintext mnemonic in memory for the session.
    pub fn unlock(&mut self, pin: &str) -> Result<()> {
        use super::WalletRepository;
        let wallet_repo = WalletRepository::new(&self.conn);
        let wallet = wallet_repo.get_primary_wallet()?
            .ok_or_else(|| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
                Some("No wallet found".to_string())
            ))?;

        let salt_hex = wallet.pin_salt.as_ref()
            .ok_or_else(|| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some("Wallet is not PIN-protected".to_string())
            ))?;

        let mnemonic = crate::crypto::pin::decrypt_mnemonic(&wallet.mnemonic, pin, salt_hex)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_AUTH),
                Some(e)
            ))?;

        self.cached_mnemonic = Some(mnemonic);
        Ok(())
    }

    /// Cache the plaintext mnemonic directly (used after create/recover when mnemonic is known)
    pub fn cache_mnemonic(&mut self, mnemonic: String) {
        self.cached_mnemonic = Some(mnemonic);
    }

    /// Try to auto-unlock the wallet using Windows DPAPI.
    /// Returns Ok(true) if successfully unlocked, Ok(false) if DPAPI blob not available,
    /// Err if DPAPI decryption failed (e.g., different Windows user or DB moved to another machine).
    pub fn try_dpapi_unlock(&mut self) -> Result<bool> {
        use super::WalletRepository;

        let wallet_repo = WalletRepository::new(&self.conn);
        let wallet = match wallet_repo.get_primary_wallet()? {
            Some(w) => w,
            None => return Ok(false),
        };

        let dpapi_blob = match wallet.mnemonic_dpapi {
            Some(blob) if !blob.is_empty() => blob,
            _ => return Ok(false),  // No DPAPI blob stored
        };

        match crate::crypto::dpapi::dpapi_decrypt(&dpapi_blob) {
            Ok(plaintext_bytes) => {
                let mnemonic = String::from_utf8(plaintext_bytes)
                    .map_err(|e| rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                        Some(format!("Invalid UTF-8 in DPAPI-decrypted mnemonic: {}", e))
                    ))?;
                self.cached_mnemonic = Some(mnemonic);
                Ok(true)
            }
            Err(e) => {
                Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_AUTH),
                    Some(format!("DPAPI unlock failed: {}", e))
                ))
            }
        }
    }

    /// Store a DPAPI-encrypted copy of the mnemonic for an existing wallet.
    /// Used to backfill DPAPI for wallets created before DPAPI support was added.
    pub fn store_dpapi_blob(&self, wallet_id: i64, mnemonic: &str) -> Result<()> {
        match crate::crypto::dpapi::dpapi_encrypt(mnemonic.as_bytes()) {
            Ok(blob) => {
                self.conn.execute(
                    "UPDATE wallets SET mnemonic_dpapi = ?1 WHERE id = ?2",
                    rusqlite::params![blob, wallet_id],
                )?;
                log::info!("   ✅ DPAPI blob stored for wallet {}", wallet_id);
                Ok(())
            }
            Err(e) => {
                log::warn!("   ⚠️  DPAPI encryption unavailable: {}", e);
                Ok(()) // Non-fatal — wallet still works with PIN
            }
        }
    }

    /// Clear the cached mnemonic from memory (used after wallet deletion).
    pub fn clear_cached_mnemonic(&mut self) {
        self.cached_mnemonic = None;
    }

    /// Get the cached plaintext mnemonic. Returns error if wallet is locked.
    pub fn get_cached_mnemonic(&self) -> Result<&str> {
        self.cached_mnemonic.as_deref()
            .ok_or_else(|| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_AUTH),
                Some("Wallet is locked. Enter PIN to unlock.".to_string())
            ))
    }

    /// Create a new wallet with its first address
    ///
    /// This generates a mnemonic, creates the wallet in the database,
    /// derives the first address using BRC-42, and saves it to the database.
    /// If `pin` is provided, the mnemonic is encrypted before storage.
    ///
    /// Returns: (wallet_id, mnemonic_phrase, first_address)
    pub fn create_wallet_with_first_address(&mut self, pin: Option<&str>) -> Result<(i64, String, String)> {
        use super::{WalletRepository, AddressRepository, UserRepository};
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
        let user_repo = UserRepository::new(&self.conn);

        // 1. Create wallet (generates mnemonic, encrypts if PIN provided)
        let (wallet_id, mnemonic_phrase) = wallet_repo.create_wallet(pin)?;
        self.cached_mnemonic = Some(mnemonic_phrase.clone());

        // 2. Parse mnemonic and derive master keys (needed for user creation)
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

        // 3. Create default user (identity_key = master pubkey hex)
        let identity_key_hex = hex::encode(&master_pubkey);
        let user_id = user_repo.create(&identity_key_hex)?;
        info!("   ✅ Default user created with ID: {}", user_id);

        // 4. Create the "default" basket for change outputs (BRC-99 requirement)
        use super::BasketRepository;
        let basket_repo = BasketRepository::new(&self.conn);
        basket_repo.find_or_insert("default", user_id)?;
        info!("   ✅ Created 'default' basket for change outputs");

        // 5. Create BRC-43 invoice number for first address: "2-receive address-0"
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

    /// Create a wallet from an existing mnemonic with first address (recovery flow)
    ///
    /// Mirrors `create_wallet_with_first_address()` but uses the provided mnemonic
    /// instead of generating a new one. Creates: wallet -> master keys -> user ->
    /// default basket -> first BRC-42 address (index 0) -> master pubkey address (index -1).
    ///
    /// Returns: (wallet_id, user_id, first_address, master_pubkey_hex)
    pub fn create_wallet_from_existing_mnemonic(&mut self, mnemonic_phrase: &str, pin: Option<&str>) -> Result<(i64, i64, String, String)> {
        use super::{WalletRepository, AddressRepository, UserRepository, BasketRepository};
        use crate::crypto::brc42::derive_child_public_key;
        use bip39::{Mnemonic, Language};
        use bip32::XPrv;
        use secp256k1::{Secp256k1, SecretKey, PublicKey};
        use sha2::{Sha256, Digest};
        use ripemd::Ripemd160;
        use std::time::{SystemTime, UNIX_EPOCH};
        use hex;
        use bs58;

        info!("🔑 Creating wallet from existing mnemonic with first address...");

        let wallet_repo = WalletRepository::new(&self.conn);
        let address_repo = AddressRepository::new(&self.conn);
        let user_repo = UserRepository::new(&self.conn);

        // 1. Create wallet from existing mnemonic (validates + inserts with backed_up=true)
        let (wallet_id, mnemonic_str) = wallet_repo.create_wallet_with_mnemonic(mnemonic_phrase, pin)?;
        self.cached_mnemonic = Some(mnemonic_str.clone());

        // 2. Parse mnemonic and derive master keys
        let mnemonic = Mnemonic::parse_in(Language::English, &mnemonic_str)
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

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&master_privkey)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Invalid private key: {}", e))
            ))?;
        let master_pubkey = PublicKey::from_secret_key(&secp, &secret_key).serialize().to_vec();

        // 3. Create default user
        let identity_key_hex = hex::encode(&master_pubkey);
        let user_id = user_repo.create(&identity_key_hex)?;
        info!("   ✅ Default user created with ID: {}", user_id);

        // 4. Create "default" basket
        let basket_repo = BasketRepository::new(&self.conn);
        basket_repo.find_or_insert("default", user_id)?;
        info!("   ✅ Created 'default' basket for change outputs");

        // 5. Derive first BRC-42 address (index 0)
        let invoice_number = "2-receive address-0";
        let derived_pubkey = derive_child_public_key(&master_privkey, &master_pubkey, invoice_number)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("BRC-42 derivation failed: {}", e))
            ))?;

        let sha_hash = Sha256::digest(&derived_pubkey);
        let pubkey_hash = Ripemd160::digest(&sha_hash);

        let mut addr_bytes = vec![0x00];
        addr_bytes.extend_from_slice(pubkey_hash.as_slice());
        let checksum_full = Sha256::digest(&Sha256::digest(&addr_bytes));
        addr_bytes.extend_from_slice(&checksum_full[0..4]);
        let address = bs58::encode(&addr_bytes).into_string();

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
            pending_utxo_check: true,
            created_at,
        };
        address_repo.create(&address_model)?;
        info!("   ✅ First BRC-42 address: {}", address);

        // 6. Create master pubkey address (index -1)
        let master_sha_hash = Sha256::digest(&master_pubkey);
        let master_pubkey_hash = Ripemd160::digest(&master_sha_hash);

        let mut master_addr_bytes = vec![0x00];
        master_addr_bytes.extend_from_slice(master_pubkey_hash.as_slice());
        let master_checksum_full = Sha256::digest(&Sha256::digest(&master_addr_bytes));
        master_addr_bytes.extend_from_slice(&master_checksum_full[0..4]);
        let master_address = bs58::encode(&master_addr_bytes).into_string();

        let master_address_model = super::Address {
            id: None,
            wallet_id,
            index: -1,
            address: master_address,
            public_key: hex::encode(&master_pubkey),
            used: false,
            balance: 0,
            pending_utxo_check: true,
            created_at,
        };
        address_repo.create(&master_address_model)?;
        info!("   ✅ Master pubkey address stored with index -1");

        let master_pubkey_hex = hex::encode(&master_pubkey);
        info!("   ✅ Wallet recovery: DB records created successfully");
        Ok((wallet_id, user_id, address, master_pubkey_hex))
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
        // Use default user_id = 1 (single-user wallet)
        basket_repo.find_or_insert("default", 1)?;
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
    /// Consolidated: single V1 creates the full target schema.
    fn migrate(&self) -> Result<()> {
        info!("   Starting migration process...");
        use crate::database::migrations;

        // Create schema_version table if it doesn't exist
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            )",
            [],
        )?;

        // Check current version (0 if empty)
        let current_version: i32 = self.conn
            .query_row(
                "SELECT MAX(version) FROM schema_version",
                [],
                |row| row.get::<_, Option<i32>>(0),
            )
            .unwrap_or(None)
            .unwrap_or(0);

        info!("   Current schema version: {}", current_version);

        if current_version < 1 {
            info!("   Applying consolidated schema V1...");
            migrations::create_schema_v1(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (1)", [])?;
            info!("   ✅ Schema V1 applied");
        }

        if current_version < 2 {
            info!("   Applying migration V2 (PIN support)...");
            migrations::migrate_v1_to_v2(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (2)", [])?;
            info!("   ✅ Schema V2 applied");
        }

        if current_version < 3 {
            info!("   Applying migration V3 (domain permissions)...");
            migrations::migrate_v2_to_v3(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (3)", [])?;
            info!("   ✅ Schema V3 applied");
        }

        if current_version < 4 {
            info!("   Applying migration V4 (DPAPI auto-unlock)...");
            migrations::migrate_v3_to_v4(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (4)", [])?;
            info!("   ✅ Schema V4 applied");
        }

        if current_version < 5 {
            info!("   Applying migration V5 (per-site ad blocking toggle)...");
            migrations::migrate_v4_to_v5(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (5)", [])?;
            info!("   ✅ Schema V5 applied");
        }

        if current_version < 6 {
            info!("   Applying migration V6 (per-site scriptlet toggle)...");
            migrations::migrate_v5_to_v6(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (6)", [])?;
            info!("   ✅ Schema V6 applied");
        }

        if current_version < 7 {
            info!("   Applying migration V7 (PeerPay received tracking)...");
            migrations::migrate_v6_to_v7(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (7)", [])?;
            info!("   ✅ Schema V7 applied");
        }

        if current_version < 8 {
            info!("   Applying migration V8 (unified notifications)...");
            migrations::migrate_v7_to_v8(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (8)", [])?;
            info!("   ✅ Schema V8 applied");
        }

        if current_version < 9 {
            info!("   Applying migration V9 (sender display name)...");
            migrations::migrate_v8_to_v9(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (9)", [])?;
            info!("   ✅ Schema V9 applied");
        }

        if current_version < 10 {
            info!("   Applying migration V10 (default auto-approve limits)...");
            migrations::migrate_v9_to_v10(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (10)", [])?;
            info!("   ✅ Schema V10 applied");
        }

        if current_version < 11 {
            info!("   Applying migration V11 (price at transaction time)...");
            migrations::migrate_v10_to_v11(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (11)", [])?;
            info!("   ✅ Schema V11 applied");
        }

        if current_version < 12 {
            info!("   Applying migration V12 (max_tx_per_session + updated defaults)...");
            migrations::migrate_v11_to_v12(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (12)", [])?;
            info!("   ✅ Schema V12 applied");
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
