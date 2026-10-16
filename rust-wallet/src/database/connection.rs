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

        // Money-DB durability: synchronous=FULL (per-commit fsync). For a wallet
        // holding real funds we want committed writes (completed sends, recovered
        // txs) to survive OS crash / power loss, not just a clean process exit.
        // FULL is the SQLite default for non-WAL and the safe choice here; the
        // per-commit fsync cost is negligible at the wallet's low write volume.
        // (Owner decision 2026-06-29: an earlier commit set NORMAL believing it was
        // the implicit WAL default — but with bundled SQLite the prior effective
        // level was FULL, so NORMAL had silently RELAXED durability. This restores
        // FULL. NORMAL's power-loss window on committed-but-unchecked writes is not
        // acceptable for the money DB.)
        conn.execute("PRAGMA synchronous=FULL", [])?;
        let synchronous: i32 = conn.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
        info!("   Synchronous: {} (0=OFF 1=NORMAL 2=FULL)", synchronous);

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

    /// Best-effort WAL checkpoint at shutdown (OD-2 graceful-exit).
    /// Runs `PRAGMA wal_checkpoint(TRUNCATE)` so the `-wal`/`-shm` are flushed into
    /// the main DB file and truncated, leaving a clean no-replay open on the next
    /// launch (and a clean image-file state for the Windows updater to swap the exe).
    /// Callers log-and-ignore any error — exit must never block on this.
    pub fn checkpoint_truncate(&self) -> rusqlite::Result<()> {
        // PRAGMA wal_checkpoint(TRUNCATE) returns one row (busy, log, checkpointed).
        self.conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_row| Ok(()))?;
        Ok(())
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

    /// ⛔ The ONE place anything may enter `cached_mnemonic`. Every caller goes through here.
    ///
    /// A value that is not a valid BIP39 phrase must NEVER be cached, because caching it
    /// makes `is_unlocked()` true — and a wallet that believes it is unlocked never offers
    /// the user the PIN screen, which is the only way back. MEASURED 2026-09-08: the owner's
    /// production wallet cached a 2-word value out of the macOS Keychain and was
    /// unrecoverable for 8 days while the correct encrypted phrase sat in the database.
    /// See `crypto::mnemonic_guard`.
    ///
    /// ⚠️ Keep this private and keep the three call sites going through it. A future
    /// `self.cached_mnemonic = Some(..)` written directly would silently reopen the hole —
    /// the per-arm-check mistake that `IpcAuth.h` was created to stop repeating.
    /// ⛔ INVARIANT: every `self.cached_mnemonic = Some(..)` in this file passes its value
    /// through here first. `grep -n "cached_mnemonic = Some" connection.rs` — each hit must
    /// contain `Self::validated_mnemonic(`. There are no other writers.
    ///
    /// (An associated fn rather than a `&mut self` setter because two call sites hold live
    /// repository borrows of `self.conn`; a whole-`self` mutable borrow will not compile
    /// there, while a disjoint field assignment does.)
    fn validated_mnemonic(phrase: String) -> std::result::Result<String, String> {
        if !crate::crypto::mnemonic_guard::is_valid_mnemonic(&phrase) {
            // Shape only — never the value. A bad value may still be a real secret.
            return Err(format!(
                "not a valid BIP39 recovery phrase ({})",
                crate::crypto::mnemonic_guard::describe_shape(&phrase)
            ));
        }
        Ok(phrase)
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

        // A correct PIN that decrypts to something which is not a phrase means the stored
        // ciphertext is damaged. Report it rather than caching it — a cached bad value would
        // present as "unlocked" and fail every operation afterwards.
        self.cached_mnemonic = Some(Self::validated_mnemonic(mnemonic).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_AUTH),
                Some(format!("Decrypted mnemonic is unusable: {}", e)))
        })?);
        Ok(())
    }

    /// Cache the plaintext mnemonic directly (used after create/recover when mnemonic is known).
    /// Returns Err if the value is not a valid phrase; the caller must leave the wallet locked.
    pub fn cache_mnemonic(&mut self, mnemonic: String) -> std::result::Result<(), String> {
        self.cached_mnemonic = Some(Self::validated_mnemonic(mnemonic)?);
        Ok(())
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
                // ⛔ Validate before caching. The credential store is not trusted input:
                // its contents can be wrong (corruption, a stale entry from an older service
                // name, an ACL-mangled in-place update) and we cannot tell from the read
                // itself. Returning Ok(false) makes the caller treat this as "no usable
                // auto-unlock", so the wallet stays LOCKED and the user is shown the PIN
                // screen — and `wallet_unlock`'s existing backfill then rewrites a correct
                // entry. That makes the failure self-healing in one prompt instead of
                // permanent.
                match Self::validated_mnemonic(mnemonic) {
                    Ok(good) => match self.phrase_belongs_to_this_wallet(&good) {
                        Ok(()) => { self.cached_mnemonic = Some(good); Ok(true) }
                        Err(why) => {
                            // ⛔ B5-T1-P2 (P2-A7/A8): a VALID phrase can still be another
                            // wallet's (a stale entry, a shared Keychain service name, a copied
                            // profile). Caching it would run this wallet on another wallet's keys.
                            // Same recovery as a malformed entry: stay locked, the PIN screen
                            // rewrites the entry. ⛔ Never log the phrase or either key.
                            error!("🔑 Credential-store auto-unlock REJECTED: {} — leaving the wallet locked so the PIN screen can repair the stored entry.", why);
                            Ok(false)
                        }
                    },
                    Err(e) => {
                        error!("🔑 Credential-store auto-unlock REJECTED: {} — the stored \
                                value is not a recovery phrase. Leaving the wallet locked so \
                                the PIN screen can repair it.", e);
                        Ok(false)
                    }
                }
            }
            Err(e) => {
                Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_AUTH),
                    Some(format!("DPAPI unlock failed: {}", e))
                ))
            }
        }
    }

    /// B5-T1-P2: does `phrase` derive this wallet's master public key
    /// (`users.identity_key`)? The `Err` text names the reason only — never the phrase,
    /// never a key. A wallet with no user row cannot be checked and is refused
    /// (fail closed: the PIN still unlocks it).
    fn phrase_belongs_to_this_wallet(&self, phrase: &str) -> std::result::Result<(), String> {
        let expected = super::UserRepository::new(&self.conn)
            .get_default()
            .map_err(|e| format!("could not read this wallet's identity key ({})", e))?
            .map(|u| u.identity_key)
            .ok_or_else(|| "this wallet has no identity key to check against".to_string())?;
        let actual = super::helpers::master_public_key_hex_from_phrase(phrase)?;
        if actual.eq_ignore_ascii_case(expected.trim()) {
            Ok(())
        } else {
            Err("the stored recovery phrase belongs to a different wallet (its master public key is not this wallet's identity key)".to_string())
        }
    }

    /// Store a DPAPI-encrypted copy of the mnemonic for an existing wallet.
    /// Used to backfill DPAPI for wallets created before DPAPI support was added.
    pub fn store_dpapi_blob(&self, wallet_id: i64, mnemonic: &str) -> Result<()> {
        // ⛔ Do NOT return Ok on failure. This is the repair path for a wallet that is
        // already locked-forever; reporting success when the write failed is how the user
        // ends up entering a PIN, believing it is fixed, and finding it locked again next
        // launch. Callers log it; the wallet stays usable for this session either way.
        match crate::crypto::dpapi::dpapi_encrypt_retrying(mnemonic.as_bytes()) {
            Ok(blob) => {
                self.conn.execute(
                    "UPDATE wallets SET mnemonic_dpapi = ?1 WHERE id = ?2",
                    rusqlite::params![blob, wallet_id],
                )?;
                log::info!("   ✅ Wallet key stored in OS credential store (wallet {})", wallet_id);
                Ok(())
            }
            Err(e) => {
                log::error!("   ❌ Could not store the wallet key: {} — this wallet will \
                             still require a PIN on every start", e);
                Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                    Some(format!("credential-store write failed: {}", e)),
                ))
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
        // Through the guard like every other cache write — see set_cached_mnemonic. Safe
        // today (it is parsed two lines below), routed anyway so no bypass exists to copy.
        self.cached_mnemonic = Some(Self::validated_mnemonic(mnemonic_phrase.clone()).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Generated mnemonic is invalid: {}", e)))
        })?);

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

        // 7. Create backup address (index -3) for on-chain wallet backup
        let backup_invoice = "1-wallet-backup-1";
        let backup_derived_pubkey = derive_child_public_key(&master_privkey, &master_pubkey, backup_invoice)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("BRC-42 backup address derivation failed: {}", e))
            ))?;

        let backup_sha_hash = Sha256::digest(&backup_derived_pubkey);
        let backup_pubkey_hash = Ripemd160::digest(&backup_sha_hash);

        let mut backup_addr_bytes = vec![0x00]; // Mainnet prefix
        backup_addr_bytes.extend_from_slice(backup_pubkey_hash.as_slice());
        let backup_checksum_full = Sha256::digest(&Sha256::digest(&backup_addr_bytes));
        backup_addr_bytes.extend_from_slice(&backup_checksum_full[0..4]);
        let backup_address = bs58::encode(&backup_addr_bytes).into_string();
        info!("   ✅ Backup address: {}", backup_address);

        let backup_address_model = super::Address {
            id: None,
            wallet_id,
            index: -3,  // Special index for on-chain backup address
            address: backup_address,
            public_key: hex::encode(&backup_derived_pubkey),
            used: false,
            balance: 0,
            pending_utxo_check: false,  // Not for UTXO sync — only used by backup system
            created_at,
        };

        address_repo.create(&backup_address_model)?;
        info!("   ✅ Backup address stored with index -3");

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
        // Through the guard like every other cache write — see set_cached_mnemonic.
        self.cached_mnemonic = Some(Self::validated_mnemonic(mnemonic_str.clone()).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Provided mnemonic is invalid: {}", e)))
        })?);

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

        // 7. Create backup address (index -3) for on-chain wallet backup
        let backup_invoice = "1-wallet-backup-1";
        let backup_derived_pubkey = derive_child_public_key(&master_privkey, &master_pubkey, backup_invoice)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("BRC-42 backup address derivation failed: {}", e))
            ))?;

        let backup_sha_hash = Sha256::digest(&backup_derived_pubkey);
        let backup_pubkey_hash = Ripemd160::digest(&backup_sha_hash);

        let mut backup_addr_bytes = vec![0x00];
        backup_addr_bytes.extend_from_slice(backup_pubkey_hash.as_slice());
        let backup_checksum_full = Sha256::digest(&Sha256::digest(&backup_addr_bytes));
        backup_addr_bytes.extend_from_slice(&backup_checksum_full[0..4]);
        let backup_address = bs58::encode(&backup_addr_bytes).into_string();

        let backup_address_model = super::Address {
            id: None,
            wallet_id,
            index: -3,  // Special index for on-chain backup address
            address: backup_address.clone(),
            public_key: hex::encode(&backup_derived_pubkey),
            used: false,
            balance: 0,
            pending_utxo_check: false,  // Not for UTXO sync — only used by backup system
            created_at,
        };
        address_repo.create(&backup_address_model)?;
        info!("   ✅ Backup address stored with index -3: {}", backup_address);

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

    /// Ensure the backup address (index -3) exists in the database.
    /// This should be called on startup for existing wallets that were created
    /// before on-chain backup support was added. Requires cached mnemonic.
    pub fn ensure_backup_address_exists(&self) -> Result<()> {
        use super::{WalletRepository, AddressRepository};
        use crate::crypto::brc42::derive_child_public_key;
        use crate::database::helpers::{get_master_private_key_from_db, get_master_public_key_from_db};
        use sha2::{Sha256, Digest};
        use ripemd::Ripemd160;
        use std::time::{SystemTime, UNIX_EPOCH};
        use bs58;

        info!("🔑 Checking if backup address exists...");

        let wallet_repo = WalletRepository::new(&self.conn);
        let address_repo = AddressRepository::new(&self.conn);

        // Get the primary wallet
        let wallet = match wallet_repo.get_primary_wallet()? {
            Some(w) => w,
            None => {
                info!("   No wallet found, skipping backup address check");
                return Ok(());
            }
        };
        let wallet_id = wallet.id.ok_or_else(|| rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
            Some("Wallet has no ID".to_string())
        ))?;

        // Check if backup address already exists (index -3)
        match address_repo.get_by_wallet_and_index(wallet_id, -3) {
            Ok(Some(_)) => {
                info!("   ✅ Backup address already exists");
                return Ok(());
            }
            Ok(None) => {
                info!("   Backup address not found, creating...");
            }
            Err(e) => {
                info!("   Error checking for backup address: {}, will try to create", e);
            }
        }

        // Derive backup address using BRC-42 self-counterparty
        let master_privkey = get_master_private_key_from_db(self)?;
        let master_pubkey = get_master_public_key_from_db(self)?;

        let backup_invoice = "1-wallet-backup-1";
        let backup_derived_pubkey = derive_child_public_key(&master_privkey, &master_pubkey, backup_invoice)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("BRC-42 backup address derivation failed: {}", e))
            ))?;

        // Convert to P2PKH address
        let sha_hash = Sha256::digest(&backup_derived_pubkey);
        let pubkey_hash = Ripemd160::digest(&sha_hash);

        let mut addr_bytes = vec![0x00]; // Mainnet prefix
        addr_bytes.extend_from_slice(pubkey_hash.as_slice());
        let checksum_full = Sha256::digest(&Sha256::digest(&addr_bytes));
        addr_bytes.extend_from_slice(&checksum_full[0..4]);
        let backup_address = bs58::encode(&addr_bytes).into_string();
        info!("   Backup address: {}", backup_address);

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let backup_address_model = super::Address {
            id: None,
            wallet_id,
            index: -3,  // Special index for on-chain backup address
            address: backup_address,
            public_key: hex::encode(&backup_derived_pubkey),
            used: false,
            balance: 0,
            pending_utxo_check: false,  // Not for UTXO sync — only used by backup system
            created_at,
        };

        address_repo.create(&backup_address_model)?;
        info!("   ✅ Backup address created with index -3");

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
    pub fn migrate(&self) -> Result<()> {
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

        if current_version < 13 {
            info!("   Applying migration V13 (recipient autocomplete)...");
            migrations::migrate_v12_to_v13(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (13)", [])?;
            info!("   ✅ Schema V13 applied");
        }

        if current_version < 14 {
            info!("   Applying migration V14 (confirmed outputs + notification types)...");
            migrations::migrate_v13_to_v14(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (14)", [])?;
            info!("   ✅ Schema V14 applied");
        }

        if current_version < 15 {
            info!("   Applying migration V15 (peerpay_pending_verification)...");
            migrations::migrate_v14_to_v15(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (15)", [])?;
            info!("   ✅ Schema V15 applied");
        }

        if current_version < 16 {
            info!("   Applying migration V16 (peerpay_outbox)...");
            migrations::migrate_v15_to_v16(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (16)", [])?;
            info!("   ✅ Schema V16 applied");
        }

        if current_version < 17 {
            info!("   Applying migration V17 (identity_key_disclosure_allowed)...");
            migrations::migrate_v16_to_v17(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (17)", [])?;
            info!("   ✅ Schema V17 applied");
        }

        if current_version < 18 {
            info!("   Applying migration V18 (domain sub-permission child tables)...");
            migrations::migrate_v17_to_v18(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (18)", [])?;
            info!("   ✅ Schema V18 applied");
        }

        if current_version < 19 {
            info!("   Applying migration V19 (default_identity_key_disclosure_allowed)...");
            migrations::migrate_v18_to_v19(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (19)", [])?;
            info!("   ✅ Schema V19 applied");
        }

        if current_version < 20 {
            // Phase 2.6-A.5: permission_audit_log + engine_shadow_log tables.
            // Engine port is dormant until at least 2.6-C (no flag flips yet),
            // but the audit/shadow tables exist so the writes from 2.6-B
            // shadow infrastructure have somewhere to land.
            info!("   Applying migration V20 (permission_audit_log + engine_shadow_log)...");
            migrations::migrate_v19_to_v20(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (20)", [])?;
            info!("   ✅ Schema V20 applied");
        }

        if current_version < 21 {
            // 2026-06-09: bsv_price_cache table for restart-survival of the
            // last known good BSV/USD price. Necessary after both upstream
            // sources (CryptoCompare auth-walled, CoinGecko slug renamed)
            // broke simultaneously and the in-memory-only cache left every
            // cold-start wallet with no fallback.
            info!("   Applying migration V21 (bsv_price_cache)...");
            migrations::migrate_v20_to_v21(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (21)", [])?;
            info!("   ✅ Schema V21 applied");
        }

        if current_version < 22 {
            // 2026-06-09: Phase 2.6-D Fix #4 — bundled_scope_grant column on
            // domain_permissions. Lets users opt in to "trust this site for
            // all wallet operations" on the connect modal, silencing
            // ProtocolUse/BasketAccess prompts (CounterpartyUse already
            // silent for approved domains via Fix #3).
            info!("   Applying migration V22 (bundled_scope_grant column)...");
            migrations::migrate_v21_to_v22(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (22)", [])?;
            info!("   ✅ Schema V22 applied");
        }

        if current_version < 23 {
            // Phase 2.6-H: the C++ PermissionEngine + the engine-to-Rust
            // shadow-comparison infrastructure were deleted. Drop the now-dead
            // engine_shadow_log table. permission_audit_log (the long-lived
            // audit surface) is kept.
            info!("   Applying migration V23 (drop engine_shadow_log)...");
            migrations::migrate_v22_to_v23(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (23)", [])?;
            info!("   ✅ Schema V23 applied");
        }

        if current_version < 24 {
            // beta.3 Phase 0.8 (owner-approved 2026-08-22): the
            // domain_manifest_snapshots child table — an informational record
            // of what a site asked for AS APPROVED — plus the
            // settings.default_prefill_from_manifest opt-in that decides
            // whether the connect modal's limit fields start from the user's
            // defaults (0, the default) or the site's suggestion (1).
            info!("   Applying migration V24 (domain_manifest_snapshots + prefill toggle)...");
            migrations::migrate_v23_to_v24(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (24)", [])?;
            info!("   ✅ Schema V24 applied");
        }

        if current_version < 25 {
            // beta.3 Phase 0.8 (owner-approved 2026-08-23): a user-level
            // default for QUIET MODE, the widest grant on the connect screen.
            // It was hardcoded ON in the modal with no way to change how a
            // fresh site starts. Ships 1 so behaviour is unchanged.
            info!("   Applying migration V25 (default_bundled_scope_grant)...");
            migrations::migrate_v24_to_v25(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (25)", [])?;
            info!("   ✅ Schema V25 applied");
        }

        if current_version < 26 {
            // B5-T1-P4 (owner decision 1 + §12 Q1, 2026-10-02): inline script cap 1024 B.
            info!("   Applying migration V26 (inline script cap)...");
            migrations::migrate_v25_to_v26(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (26)", [])?;
            info!("   ✅ Schema V26 applied");
        }

        if current_version < 27 {
            // B5-T1-P3 (owner decision 2a, invariant 2 approved 2026-09-27; kickoff Q5
            // 2026-10-04): the money index `money_utxos`, kept by triggers.
            info!("   Applying migration V27 (money index)...");
            migrations::migrate_v26_to_v27(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (27)", [])?;
            info!("   ✅ Schema V27 applied");
        }

        if current_version < 28 {
            // B5-T1-P5 (owner decisions 2 + 7; invariant 2 approved 2026-10-05): money
            // only when the classification seam stamped `change = 1`.
            info!("   Applying migration V28 (money = stamped coins only)...");
            migrations::migrate_v27_to_v28(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (28)", [])?;
            info!("   ✅ Schema V28 applied");
        }

        if current_version < 29 {
            // B5-T1-P7a-1 (owner decision 1; invariant 2 approved 2026-10-09): the verified
            // block-header chain.
            info!("   Applying migration V29 (verified_headers)...");
            migrations::migrate_v28_to_v29(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (29)", [])?;
            info!("   ✅ Schema V29 applied");
        }

        if current_version < 30 {
            // B5-T1-P7b (owner decision 1; invariant 2 approved 2026-10-10): PeerPay messages held
            // before they are acknowledged.
            info!("   Applying migration V30 (peerpay_held_messages)...");
            migrations::migrate_v29_to_v30(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (30)", [])?;
            info!("   ✅ Schema V30 applied");
        }

        if current_version < 31 {
            // B5-T5-P7a (invariant 2 approved 2026-10-15): which MessageBox hosts took an outbox message.
            info!("   Applying migration V31 (peerpay_outbox delivered_hosts, message_request)...");
            migrations::migrate_v30_to_v31(&self.conn)?;
            self.conn.execute("INSERT INTO schema_version (version) VALUES (31)", [])?;
            info!("   ✅ Schema V31 applied");
        }

        // B5-T1-P7b (review 164-5 #1): `checked_at` joined V30 after its first form; a table created by
        // that first form gets it here (nothing shipped it, but no build may run without the column).
        let held_has_checked_at = {
            let mut stmt = self.conn.prepare("PRAGMA table_info(peerpay_held_messages)")?;
            let cols: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?.filter_map(|r| r.ok()).collect();
            cols.is_empty() || cols.iter().any(|c| c == "checked_at")
        };
        if !held_has_checked_at {
            info!("   Repair: adding checked_at to peerpay_held_messages");
            self.conn.execute("ALTER TABLE peerpay_held_messages ADD COLUMN checked_at INTEGER", [])?;
        }

        // Startup repair: V12 migration may have recorded version but failed to add columns
        // (INSERT INTO schema_version succeeded but ALTER TABLE was skipped/failed).
        // Re-run the column checks unconditionally to patch any inconsistent DBs.
        let has_max_tx = {
            let mut stmt = self.conn.prepare("PRAGMA table_info(domain_permissions)")?;
            let cols: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
                .filter_map(|r| r.ok()).collect();
            cols.iter().any(|c| c == "max_tx_per_session")
        };
        if !has_max_tx {
            info!("   Repair: adding missing max_tx_per_session to domain_permissions");
            self.conn.execute(
                "ALTER TABLE domain_permissions ADD COLUMN max_tx_per_session INTEGER NOT NULL DEFAULT 100", [])?;
        }
        let has_default_max_tx = {
            let mut stmt = self.conn.prepare("PRAGMA table_info(settings)")?;
            let cols: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
                .filter_map(|r| r.ok()).collect();
            cols.iter().any(|c| c == "default_max_tx_per_session")
        };
        if !has_default_max_tx {
            info!("   Repair: adding missing default_max_tx_per_session to settings");
            self.conn.execute(
                "ALTER TABLE settings ADD COLUMN default_max_tx_per_session INTEGER DEFAULT 100", [])?;
        }

        // Repair: certificate publish columns (our old V12) lost during staging merge
        let has_publish_status = {
            let mut stmt = self.conn.prepare("PRAGMA table_info(certificates)")?;
            let cols: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
                .filter_map(|r| r.ok()).collect();
            cols.iter().any(|c| c == "publish_status")
        };
        if !has_publish_status {
            info!("   Repair: adding missing publish columns to certificates");
            self.conn.execute(
                "ALTER TABLE certificates ADD COLUMN publish_status TEXT NOT NULL DEFAULT 'unpublished'", [])?;
            self.conn.execute(
                "ALTER TABLE certificates ADD COLUMN publish_txid TEXT", [])?;
            self.conn.execute(
                "ALTER TABLE certificates ADD COLUMN publish_vout INTEGER", [])?;
        }

        // Repair: V13 recipient columns on transactions
        let has_recipient = {
            let mut stmt = self.conn.prepare("PRAGMA table_info(transactions)")?;
            let cols: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
                .filter_map(|r| r.ok()).collect();
            cols.iter().any(|c| c == "recipient")
        };
        if !has_recipient {
            info!("   Repair: adding missing recipient columns to transactions");
            self.conn.execute(
                "ALTER TABLE transactions ADD COLUMN recipient TEXT", [])?;
            self.conn.execute(
                "ALTER TABLE transactions ADD COLUMN recipient_name TEXT", [])?;
        }

        // Repair: backup hash and last backup timestamp on settings
        let has_backup_hash = {
            let mut stmt = self.conn.prepare("PRAGMA table_info(settings)")?;
            let cols: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
                .filter_map(|r| r.ok()).collect();
            cols.iter().any(|c| c == "backup_hash")
        };
        if !has_backup_hash {
            info!("   Repair: adding backup_hash and last_backup_at to settings");
            self.conn.execute(
                "ALTER TABLE settings ADD COLUMN backup_hash TEXT", [])?;
            self.conn.execute(
                "ALTER TABLE settings ADD COLUMN last_backup_at INTEGER NOT NULL DEFAULT 0", [])?;
        }

        // B5-T1-P3: the money index is derived — (re)install it if absent (a DB stamped
        // V27 by a merge without the table is repaired, not trusted) and rebuild it from
        // `outputs` at every open. A downgrade/upgrade cycle or a missed write cannot
        // survive a restart (contract §2, P3-A10/A11).
        // B5-T1-P5: and replace triggers an older rule wrote (they are `IF NOT EXISTS`).
        if migrations::ensure_money_index_current(&self.conn)? {
            log::warn!("   ⚠️ Money index triggers were from an older rule — reinstalled");
        }
        migrations::rebuild_money_index(&self.conn)?;

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

// ============================================================================
// B5-T1-P2 (P2-A7) — auto-unlock refuses a valid recovery phrase that is another
// wallet's. Real `WalletDatabase` on a temp file; on Windows the credential-store
// entry is a real DPAPI blob written by `store_dpapi_blob` into the TEMP DB only.
// ⛔ No macOS end-to-end test here: the Keychain item outlives a temp DB (contract
// P2-A7 residue note) — the Keychain arm is a manual row on a scratch profile.
// ============================================================================
#[cfg(test)]
mod auto_unlock_identity_tests {
    use super::*;

    /// BIP39 test vector — a valid phrase that is certainly not the fresh wallet's.
    const OTHER: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    fn fresh_wallet() -> (WalletDatabase, i64, String) {
        let path = std::env::temp_dir().join(format!("hodos_t1p2_unlock_{}.db", uuid::Uuid::new_v4()));
        let mut db = WalletDatabase::new(path).unwrap();
        let (wallet_id, phrase, _addr) = db.create_wallet_with_first_address(Some("1234")).unwrap();
        (db, wallet_id, phrase)
    }

    fn identity_key(db: &WalletDatabase) -> String {
        super::super::UserRepository::new(db.connection()).get_default().unwrap().unwrap().identity_key
    }

    /// The comparison's reference is right: the helper derives exactly the key the
    /// wallet stored at creation. (RED: derive a child — e.g. the `2-receive address`
    /// key — instead of the master ⇒ this fails, which is the mass-lockout story.)
    #[test]
    fn helper_derives_the_identity_key_written_at_creation() {
        let (db, _, phrase) = fresh_wallet();
        assert_eq!(super::super::helpers::master_public_key_hex_from_phrase(&phrase).unwrap(), identity_key(&db));
        assert_ne!(super::super::helpers::master_public_key_hex_from_phrase(OTHER).unwrap(), identity_key(&db),
            "fixture: OTHER must not be this wallet's phrase");
    }

    /// Other wallet's phrase in the credential store ⇒ stays locked.
    /// RED (scratch build, comparison reverted) ⇒ unlocks, and the cached phrase
    /// derives OTHER's master key, not this wallet's.
    #[cfg(windows)]
    #[test]
    fn another_wallets_phrase_leaves_the_wallet_locked() {
        let (mut db, wallet_id, _) = fresh_wallet();
        db.store_dpapi_blob(wallet_id, OTHER).unwrap();
        db.clear_cached_mnemonic();
        let unlocked = db.try_dpapi_unlock().unwrap();
        let cached_key = db.get_cached_mnemonic().ok()
            .map(|p| super::super::helpers::master_public_key_hex_from_phrase(p).unwrap());
        assert_eq!(cached_key, None, "wallet must not be running on another wallet's phrase");
        assert!(!unlocked && !db.is_unlocked());
    }

    /// This wallet's own phrase ⇒ unlocks as before (the other half's control).
    #[cfg(windows)]
    #[test]
    fn own_phrase_unlocks() {
        let (mut db, wallet_id, phrase) = fresh_wallet();
        db.store_dpapi_blob(wallet_id, &phrase).unwrap();
        db.clear_cached_mnemonic();
        assert!(db.try_dpapi_unlock().unwrap());
        assert!(db.is_unlocked());
        assert_eq!(super::super::helpers::get_master_public_key_from_db(&db).map(hex::encode).unwrap(), identity_key(&db));
    }
}

// ============================================================================
// B5-T1-P3 — real wallet databases: upgrade parity (P3-A1 on real data, P3-A10) and
// restore (P3-A12, T1 half). Opt-in: `HODOS_P3_FIXTURES=<dir of *.db snapshots>`.
// The snapshots are COPIES (SQLite backup API, read-only on the source); each is copied
// again to a temp file before anything opens it, so a fixture is never modified.
// ============================================================================
#[cfg(test)]
mod p3_real_wallet_fixture_tests {
    use super::*;
    use std::collections::BTreeSet;

    fn legacy(conn: &Connection, user: i64, confirmed_only: bool) -> BTreeSet<(String, i64)> {
        let sql = if confirmed_only {
            "SELECT o.txid, o.vout FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             LEFT JOIN output_baskets b ON o.basket_id = b.basketId
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status = 'completed' OR (o.transaction_id IS NULL AND o.confirmed = 1))
               AND o.derivation_prefix IS NOT NULL
               AND (o.basket_id IS NULL OR b.name = 'default')"
        } else {
            "SELECT o.txid, o.vout FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             LEFT JOIN output_baskets b ON o.basket_id = b.basketId
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nosend', 'nonfinal'))
               AND o.derivation_prefix IS NOT NULL
               AND (o.basket_id IS NULL OR b.name = 'default')
               AND (o.transaction_id IS NOT NULL OR o.confirmed = 1)"
        };
        let mut stmt = conn.prepare(sql).unwrap();
        let rows = stmt.query_map([user], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))).unwrap();
        rows.map(|r| r.unwrap()).collect()
    }

    fn current(conn: &Connection, user: i64, confirmed_only: bool) -> BTreeSet<(String, i64)> {
        let repo = super::super::OutputRepository::new(conn);
        let v = if confirmed_only { repo.get_spendable_confirmed_by_user(user) } else { repo.get_spendable_by_user(user) };
        v.unwrap().into_iter().map(|o| (o.txid.unwrap_or_default(), o.vout as i64)).collect()
    }

    fn users(conn: &Connection) -> Vec<i64> {
        let mut stmt = conn.prepare("SELECT userId FROM users").unwrap();
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0)).unwrap();
        rows.map(|r| r.unwrap()).collect()
    }

    fn temp_copy(src: &std::path::Path, tag: &str) -> std::path::PathBuf {
        let dst = std::env::temp_dir().join(format!("hodos_p3_fx_{}_{}.db", tag, uuid::Uuid::new_v4()));
        std::fs::copy(src, &dst).unwrap();
        dst
    }

    fn short(s: &BTreeSet<(String, i64)>) -> Vec<String> {
        s.iter().take(5).map(|(t, v)| format!("{}:{}", &t[..t.len().min(16)], v)).collect()
    }

    /// The pre-P5 balance query, verbatim (`calculate_balance` before B5-T1-P5 split it
    /// into money + held) — the figure the user saw before the upgrade.
    fn pre_p5_balance(conn: &Connection, user: i64) -> i64 {
        conn.query_row(
            "SELECT COALESCE(SUM(o.satoshis), 0) FROM outputs o
             LEFT JOIN transactions t ON o.transaction_id = t.id
             LEFT JOIN output_baskets b ON o.basket_id = b.basketId
             WHERE o.user_id = ?1 AND o.spendable = 1
               AND (t.status IS NULL OR t.status NOT IN ('unsigned', 'failed', 'nonfinal'))
               AND COALESCE(o.derivation_prefix, '') != '1-wallet-backup'
               AND (o.basket_id IS NULL OR b.name = 'default')",
            [user], |r| r.get(0)).unwrap()
    }

    /// Opt-in real-data run (`HODOS_P3_FIXTURES` = a folder of read-only `.db` snapshots
    /// taken with the SQLite backup API). B5-T1-P3's `P3-A1/A10/A12` re-based by
    /// B5-T1-P5 (`P5-A4`, `P5-A9`): P5 changes membership on purpose, so parity is now
    /// "nothing vanished": after the upgrade and the classifier, **money + held = the
    /// pre-upgrade balance**, the money set is a subset of the pre-P5 coin set, and
    /// every coin that left it is listed as held with a reason. The classifier needs the
    /// keys: the copy is unlocked with DPAPI (same Windows user as the wallet); if that
    /// fails the run says so and checks the locked state (money 0, everything held).
    #[test]
    #[ignore] // opt-in: HODOS_P3_FIXTURES
    fn p3_a1_a10_a12_real_wallets_upgrade_and_restore_with_the_same_money() {
        let dir = std::env::var("HODOS_P3_FIXTURES").expect("set HODOS_P3_FIXTURES");
        let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap()
            .map(|e| e.unwrap().path()).filter(|p| p.extension().map_or(false, |x| x == "db")).collect();
        paths.sort();
        assert!(!paths.is_empty(), "no fixtures in {}", dir);
        for src in paths {
            let name = src.file_stem().unwrap().to_string_lossy().to_string();

            // ---- upgrade (P3-A10 → P5-A4) ------------------------------------------------
            let copy = temp_copy(&src, &name);
            let (from_version, before): (i64, Vec<(i64, i64, BTreeSet<(String, i64)>)>) = {
                let raw = Connection::open(&copy).unwrap();
                let v: i64 = raw.query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0)).unwrap();
                let b = users(&raw).into_iter().map(|u| (u, pre_p5_balance(&raw, u), legacy(&raw, u, false))).collect();
                (v, b)
            };
            let mut db = WalletDatabase::new(copy.clone()).unwrap();
            let to_version: i64 = db.connection().query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0)).unwrap();
            assert_eq!(to_version, 30, "{name}: migrated to the latest schema (V30, B5-T1-P7b)");
            for (u, bal, _) in &before {
                let repo = super::super::OutputRepository::new(db.connection());
                assert_eq!(repo.calculate_balance(*u).unwrap(), 0, "{name}: before the classifier nothing is money");
                assert_eq!(repo.calculate_held(*u).unwrap().0, *bal, "{name}: …and every coin is shown as held");
            }
            let unlocked = matches!(db.try_dpapi_unlock(), Ok(true));
            if !unlocked {
                println!("{name}: V{from_version}→V{to_version} — could not unlock (another OS user's wallet; on macOS a test build reads the Keychain only with HODOS_KEYCHAIN_TEST=1 HODOS_DEV=1); ⚠️ locked state checked only — money + held = balance NOT checked");
            } else {
                crate::classify::stamp_all(&db, "fixture");
                let conn = db.connection();
                let held = crate::classify::list_held(&db, 1).unwrap();
                for (u, bal, all) in &before {
                    let repo = super::super::OutputRepository::new(conn);
                    let money = repo.calculate_balance(*u).unwrap();
                    let (held_sats, held_n) = repo.calculate_held(*u).unwrap();
                    let now_all = current(conn, *u, false);
                    println!("{name}: V{from_version}→V{to_version} user {u}: money {} sats ({} selectable coins), held {} sats in {} coin(s), pre-upgrade {} sats",
                             money, now_all.len(), held_sats, held_n, bal);
                    assert_eq!(money + held_sats, *bal, "{name} user {u}: money + held ≠ the pre-upgrade balance");
                    let extra: BTreeSet<_> = now_all.difference(all).cloned().collect();
                    assert!(extra.is_empty(), "{name} user {u}: P5 made coins selectable that were not before: {:?}", short(&extra));
                    for (t, v) in all.difference(&now_all) {
                        let reason = held.iter().find(|h| h.txid.as_deref() == Some(t.as_str()) && h.vout as i64 == *v)
                            .map(|h| format!("{:?}", h.reason)).unwrap_or_else(|| "NOT LISTED".into());
                        println!("{name}:   no longer selectable {}:{} — held ({})", &t[..t.len().min(16)], v, reason);
                        assert_ne!(reason, "NOT LISTED", "{name}: {}:{} left the money set without being shown as held", t, v);
                    }
                }
                // Built-in RED: the instrument can fail — an emptied index makes the
                // money set empty while `money + held` (read from `outputs`) is unchanged.
                if let Some((u, _, all)) = before.iter().find(|(_, _, all)| !all.is_empty()) {
                    conn.execute("SAVEPOINT red", []).unwrap();
                    conn.execute("DELETE FROM money_utxos", []).unwrap();
                    assert!(current(conn, *u, false).is_empty() && !all.is_empty(), "{name}: the comparison must be able to fail");
                    conn.execute("ROLLBACK TO red", []).unwrap();
                    conn.execute("RELEASE red", []).unwrap();
                }

                // ---- restore (P3-A12 → P5-A9): production encode / decode, fresh wallet from the same phrase
                let phrase = db.get_cached_mnemonic().unwrap().to_string();
                let ik: String = conn.query_row("SELECT identity_key FROM users ORDER BY userId LIMIT 1", [], |r| r.get(0)).unwrap();
                let key = [7u8; 32];
                let compressed = crate::backup::compress_for_onchain(conn, &ik, chrono::Utc::now().timestamp()).unwrap();
                let encrypted = crate::backup::encrypt_compressed(&key, &compressed).unwrap();
                let payload = crate::backup::deserialize_from_onchain(&encrypted, &key).unwrap();
                let fresh_path = std::env::temp_dir().join(format!("hodos_p5_fx_restore_{}_{}.db", name, uuid::Uuid::new_v4()));
                let mut fresh = WalletDatabase::new(fresh_path.clone()).unwrap();
                fresh.create_wallet_from_existing_mnemonic(&phrase, None).unwrap();
                {
                    let c = fresh.connection();
                    let wid: i64 = c.query_row("SELECT id FROM wallets ORDER BY id LIMIT 1", [], |r| r.get(0)).unwrap();
                    c.execute("DELETE FROM addresses WHERE wallet_id = ?1", [wid]).unwrap();
                    c.execute("DELETE FROM output_baskets", []).unwrap();
                    c.execute("DELETE FROM users", []).unwrap();
                    crate::backup::import_to_db_with_ids(c, &payload, wid, 1).unwrap();
                }
                crate::classify::stamp_all(&fresh, "fixture-restore");
                let src_money: BTreeSet<_> = before.iter().flat_map(|(u, _, _)| current(conn, *u, false)).collect();
                let restored = current(fresh.connection(), 1, false);
                let extra: Vec<_> = restored.difference(&src_money).filter(|(t, v)| {
                    // Known, owned by B5-T3a-P2.1 (BS-L3): an unconfirmed received coin restores confirmed.
                    let bs_l3: bool = conn.query_row(
                        "SELECT confirmed = 0 AND transaction_id IS NULL FROM outputs WHERE txid = ?1 AND vout = ?2",
                        rusqlite::params![t, v], |r| r.get(0)).unwrap_or(false);
                    !bs_l3
                }).cloned().collect();
                assert!(extra.is_empty(), "{name}: the restore offers coins the source did not: {:?}", extra);
                println!("{name}: restored wallet offers {} coin(s) as money (source: {})", restored.len(), src_money.len());
                drop(fresh);
                let _ = std::fs::remove_file(&fresh_path);
            }

            // ---- P3-A10 (ii): stamped by a merge without the table — repaired at open ----
            let stamped = temp_copy(&src, &format!("{name}_stamped"));
            {
                let raw = Connection::open(&stamped).unwrap();
                raw.execute("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY)", []).unwrap();
                raw.execute("INSERT OR IGNORE INTO schema_version (version) VALUES (28)", []).unwrap();
            }
            let sdb = WalletDatabase::new(stamped.clone()).unwrap();
            let has: i64 = sdb.connection().query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='money_utxos'", [], |r| r.get(0)).unwrap();
            assert_eq!(has, 1, "{name}: a DB stamped V28 without the table is repaired at open");
            drop(sdb);
            drop(db);
            for p in [&copy, &stamped] {
                for ext in ["", "-wal", "-shm"] {
                    let _ = std::fs::remove_file(format!("{}{}", p.display(), ext));
                }
            }
        }
    }
}
