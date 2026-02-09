//! Database schema migrations
//!
//! Handles creation and updates of database schema.
//! Each migration function creates a specific version of the schema.

use rusqlite::{Connection, Result};
use log::{info, warn, error};
use bip39::{Mnemonic, Language};
use bip32::XPrv;
use secp256k1::{Secp256k1, SecretKey, PublicKey};

/// Create schema version 1 (initial schema)
///
/// Creates all 15 tables for the wallet database:
/// - wallets, addresses, baskets, utxos
/// - parent_transactions, merkle_proofs, block_headers
/// - transactions, transaction_labels, transaction_inputs, transaction_outputs
/// - certificates, messages, domain_whitelist
pub fn create_schema_v1(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 1...");

    // 1. wallets table
    info!("   Creating wallets table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS wallets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            mnemonic TEXT NOT NULL,
            current_index INTEGER NOT NULL DEFAULT 0,
            backed_up BOOLEAN NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    ).map_err(|e| {
        error!("❌ Failed to create wallets table: {}", e);
        e
    })?;
    info!("   ✅ wallets table created");

    info!("   Creating idx_wallets_id index...");
    conn.execute("CREATE INDEX IF NOT EXISTS idx_wallets_id ON wallets(id)", [])
        .map_err(|e| {
            error!("❌ Failed to create idx_wallets_id index: {}", e);
            e
        })?;
    info!("   ✅ Created wallets table and indexes");

    // 2. addresses table
    info!("   Creating addresses table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS addresses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            wallet_id INTEGER NOT NULL,
            \"index\" INTEGER NOT NULL,
            address TEXT NOT NULL UNIQUE,
            public_key TEXT NOT NULL,
            used BOOLEAN NOT NULL DEFAULT 0,
            balance INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (wallet_id) REFERENCES wallets(id) ON DELETE CASCADE,
            UNIQUE(wallet_id, \"index\")
        )",
        [],
    )?;
    info!("   Creating addresses indexes...");
    conn.execute("CREATE INDEX IF NOT EXISTS idx_addresses_wallet_id ON addresses(wallet_id)", [])
        .map_err(|e| { error!("❌ Failed at idx_addresses_wallet_id: {}", e); e })?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_addresses_address ON addresses(address)", [])
        .map_err(|e| { error!("❌ Failed at idx_addresses_address: {}", e); e })?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_addresses_index ON addresses(wallet_id, \"index\")", [])
        .map_err(|e| { error!("❌ Failed at idx_addresses_index: {}", e); e })?;
    info!("   ✅ Created addresses table");

    // 3. baskets table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS baskets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT,
            token_type TEXT,
            protocol_id TEXT,
            created_at INTEGER NOT NULL,
            last_used INTEGER,
            UNIQUE(name)
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_baskets_name ON baskets(name)", [])?;
    info!("   ✅ Created baskets table");

    // 4. utxos table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS utxos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            address_id INTEGER NOT NULL,
            basket_id INTEGER,
            txid TEXT NOT NULL,
            vout INTEGER NOT NULL,
            satoshis INTEGER NOT NULL,
            script TEXT NOT NULL,
            first_seen INTEGER NOT NULL,
            last_updated INTEGER NOT NULL,
            is_spent BOOLEAN NOT NULL DEFAULT 0,
            spent_txid TEXT,
            spent_at INTEGER,
            FOREIGN KEY (address_id) REFERENCES addresses(id) ON DELETE CASCADE,
            FOREIGN KEY (basket_id) REFERENCES baskets(id) ON DELETE SET NULL,
            UNIQUE(txid, vout)
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_utxos_address_id ON utxos(address_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_utxos_txid_vout ON utxos(txid, vout)", [])?;
    // Partial indexes (commented out temporarily to test)
    // conn.execute("CREATE INDEX IF NOT EXISTS idx_utxos_is_spent ON utxos(is_spent) WHERE is_spent = 0", [])?;
    // conn.execute("CREATE INDEX IF NOT EXISTS idx_utxos_address_unspent ON utxos(address_id, is_spent) WHERE is_spent = 0", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_utxos_basket_id ON utxos(basket_id)", [])?;
    // conn.execute("CREATE INDEX IF NOT EXISTS idx_utxos_basket_unspent ON utxos(basket_id, is_spent) WHERE is_spent = 0", [])?;
    info!("   ✅ Created utxos table");

    // 5. parent_transactions table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS parent_transactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            utxo_id INTEGER NOT NULL,
            txid TEXT NOT NULL UNIQUE,
            raw_hex TEXT NOT NULL,
            cached_at INTEGER NOT NULL,
            FOREIGN KEY (utxo_id) REFERENCES utxos(id) ON DELETE CASCADE
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_parent_txns_txid ON parent_transactions(txid)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_parent_txns_utxo_id ON parent_transactions(utxo_id)", [])?;
    info!("   ✅ Created parent_transactions table");

    // 6. merkle_proofs table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS merkle_proofs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            parent_txn_id INTEGER NOT NULL,
            block_height INTEGER NOT NULL,
            tx_index INTEGER NOT NULL,
            target_hash TEXT NOT NULL,
            nodes TEXT NOT NULL,
            cached_at INTEGER NOT NULL,
            FOREIGN KEY (parent_txn_id) REFERENCES parent_transactions(id) ON DELETE CASCADE,
            UNIQUE(parent_txn_id)
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_merkle_proofs_block_height ON merkle_proofs(block_height)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_merkle_proofs_parent_txn ON merkle_proofs(parent_txn_id)", [])?;
    info!("   ✅ Created merkle_proofs table");

    // 7. block_headers table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS block_headers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            block_hash TEXT NOT NULL UNIQUE,
            height INTEGER NOT NULL UNIQUE,
            header_hex TEXT NOT NULL,
            cached_at INTEGER NOT NULL,
            UNIQUE(block_hash, height)
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_block_headers_hash ON block_headers(block_hash)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_block_headers_height ON block_headers(height)", [])?;
    info!("   ✅ Created block_headers table");

    // 8. transactions table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            txid TEXT NOT NULL UNIQUE,
            reference_number TEXT NOT NULL UNIQUE,
            raw_tx TEXT NOT NULL,
            description TEXT,
            status TEXT NOT NULL,
            is_outgoing BOOLEAN NOT NULL,
            satoshis INTEGER NOT NULL,
            timestamp INTEGER NOT NULL,
            block_height INTEGER,
            confirmations INTEGER NOT NULL DEFAULT 0,
            version INTEGER NOT NULL DEFAULT 1,
            lock_time INTEGER NOT NULL DEFAULT 0,
            custom_instructions TEXT
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_transactions_txid ON transactions(txid)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_transactions_reference ON transactions(reference_number)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_transactions_timestamp ON transactions(timestamp DESC)", [])?;
    info!("   ✅ Created transactions table");

    // 9. transaction_labels table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transaction_labels (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            transaction_id INTEGER NOT NULL,
            label TEXT NOT NULL,
            FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE,
            UNIQUE(transaction_id, label)
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_transaction_labels_tx_id ON transaction_labels(transaction_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_transaction_labels_label ON transaction_labels(label)", [])?;
    info!("   ✅ Created transaction_labels table");

    // 10. transaction_inputs table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transaction_inputs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            transaction_id INTEGER NOT NULL,
            txid TEXT NOT NULL,
            vout INTEGER NOT NULL,
            satoshis INTEGER NOT NULL,
            script TEXT,
            FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tx_inputs_tx_id ON transaction_inputs(transaction_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tx_inputs_prev_tx ON transaction_inputs(txid, vout)", [])?;
    info!("   ✅ Created transaction_inputs table");

    // 11. transaction_outputs table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transaction_outputs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            transaction_id INTEGER NOT NULL,
            vout INTEGER NOT NULL,
            satoshis INTEGER NOT NULL,
            script TEXT,
            address TEXT,
            FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE,
            UNIQUE(transaction_id, vout)
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tx_outputs_tx_id ON transaction_outputs(transaction_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tx_outputs_address ON transaction_outputs(address)", [])?;
    info!("   ✅ Created transaction_outputs table");

    // 12. certificates table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS certificates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            certificate_txid TEXT UNIQUE,
            identity_key TEXT NOT NULL,
            attributes TEXT,
            acquired_at INTEGER NOT NULL,
            relinquished BOOLEAN NOT NULL DEFAULT 0,
            relinquished_at INTEGER
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_certificates_identity_key ON certificates(identity_key)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_certificates_txid ON certificates(certificate_txid)", [])?;
    // Partial index (commented out temporarily to test)
    // conn.execute("CREATE INDEX IF NOT EXISTS idx_certificates_active ON certificates(identity_key, relinquished) WHERE relinquished = 0", [])?;
    info!("   ✅ Created certificates table");

    // 13. messages table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_box TEXT NOT NULL,
            sender TEXT NOT NULL,
            recipient TEXT NOT NULL,
            body TEXT NOT NULL,
            received_at INTEGER NOT NULL,
            acknowledged BOOLEAN NOT NULL DEFAULT 0,
            acknowledged_at INTEGER
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_messages_recipient_box ON messages(recipient, message_box)", [])?;
    // Partial index (commented out temporarily to test)
    // conn.execute("CREATE INDEX IF NOT EXISTS idx_messages_unacknowledged ON messages(recipient, acknowledged) WHERE acknowledged = 0", [])?;
    info!("   ✅ Created messages table");

    // 14. output_tags table (for output categorization)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS output_tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tag TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            is_deleted BOOLEAN NOT NULL DEFAULT 0,
            UNIQUE(tag)
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tags_tag ON output_tags(tag)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tags_deleted ON output_tags(is_deleted) WHERE is_deleted = 0", [])?;
    info!("   ✅ Created output_tags table");

    // 15. output_tag_map table (many-to-many relationship between outputs and tags)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS output_tag_map (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            output_id INTEGER NOT NULL,
            output_tag_id INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            is_deleted BOOLEAN NOT NULL DEFAULT 0,
            FOREIGN KEY (output_id) REFERENCES utxos(id) ON DELETE CASCADE,
            FOREIGN KEY (output_tag_id) REFERENCES output_tags(id) ON DELETE CASCADE,
            UNIQUE(output_id, output_tag_id)
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tag_map_output_id ON output_tag_map(output_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tag_map_tag_id ON output_tag_map(output_tag_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tag_map_deleted ON output_tag_map(is_deleted) WHERE is_deleted = 0", [])?;
    info!("   ✅ Created output_tag_map table");

    // 14. domain_whitelist table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS domain_whitelist (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            domain TEXT NOT NULL UNIQUE,
            added_at INTEGER NOT NULL,
            last_used INTEGER
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_domain_whitelist_domain ON domain_whitelist(domain)", [])?;
    info!("   ✅ Created domain_whitelist table");

    info!("   ✅ All tables created successfully");
    Ok(())
}

/// Create schema version 2 (add pending_utxo_check to addresses)
///
/// Adds `pending_utxo_check` column to the `addresses` table to track
/// newly created addresses that need UTXO checking.
pub fn create_schema_v2(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 2...");

    // Add pending_utxo_check column to addresses table
    info!("   Adding pending_utxo_check column to addresses table...");
    conn.execute(
        "ALTER TABLE addresses ADD COLUMN pending_utxo_check BOOLEAN NOT NULL DEFAULT 0",
        [],
    )?;

    // Create index for faster queries of pending addresses
    info!("   Creating index for pending_utxo_check...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_addresses_pending_utxo_check ON addresses(pending_utxo_check) WHERE pending_utxo_check = 1",
        [],
    )?;

    info!("   ✅ Schema version 2 migration complete");
    Ok(())
}

/// Create schema version 3 (make parent_transactions.utxo_id nullable)
///
/// Allows caching parent transactions from external sources (not in our wallet).
/// SQLite doesn't support ALTER COLUMN directly, so we recreate the table.
pub fn create_schema_v3(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 3...");

    // SQLite doesn't support ALTER COLUMN directly, so we need to:
    // 1. Create new table with nullable utxo_id
    // 2. Copy data
    // 3. Drop old table
    // 4. Rename new table

    info!("   Step 1: Creating temporary parent_transactions table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS parent_transactions_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            utxo_id INTEGER,
            txid TEXT NOT NULL UNIQUE,
            raw_hex TEXT NOT NULL,
            cached_at INTEGER NOT NULL,
            FOREIGN KEY (utxo_id) REFERENCES utxos(id) ON DELETE CASCADE
        )",
        [],
    )?;

    info!("   Step 2: Copying data from old table...");
    conn.execute(
        "INSERT INTO parent_transactions_new (id, utxo_id, txid, raw_hex, cached_at)
         SELECT id, utxo_id, txid, raw_hex, cached_at FROM parent_transactions",
        [],
    )?;

    info!("   Step 3: Dropping old table...");
    conn.execute("DROP TABLE parent_transactions", [])?;

    info!("   Step 4: Renaming new table...");
    conn.execute("ALTER TABLE parent_transactions_new RENAME TO parent_transactions", [])?;

    // Recreate indexes
    conn.execute("CREATE INDEX IF NOT EXISTS idx_parent_txns_txid ON parent_transactions(txid)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_parent_txns_utxo_id ON parent_transactions(utxo_id)", [])?;

    info!("   ✅ Schema version 3 migration complete");
    Ok(())
}

/// Create schema version 4 (performance indexes)
///
/// Adds indexes for frequently queried columns to improve query performance:
/// - Balance calculations (utxos by address_id and is_spent)
/// - Transaction lookups (txid indexes)
/// - Parent transaction lookups
/// - Merkle proof lookups
/// - Block header lookups
pub fn create_schema_v4(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 4 (performance indexes)...");

    // Index for balance calculations (most critical for performance)
    // Partial index: only index unspent UTXOs (is_spent = 0)
    info!("   Creating index for balance calculations...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_utxos_balance_calc
         ON utxos(address_id) WHERE is_spent = 0",
        [],
    )?;
    info!("   ✅ Created idx_utxos_balance_calc");

    // Composite index for UTXO lookups (if not already exists from v1)
    info!("   Creating composite index for UTXO lookups...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_utxos_txid_vout
         ON utxos(txid, vout)",
        [],
    )?;
    info!("   ✅ Created idx_utxos_txid_vout");

    // Index for transaction lookups by txid
    info!("   Creating index for transaction lookups...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_transactions_txid
         ON transactions(txid)",
        [],
    )?;
    info!("   ✅ Created idx_transactions_txid");

    // Index for parent transaction lookups (may already exist from v1/v3)
    info!("   Verifying parent transaction index...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_parent_txns_txid
         ON parent_transactions(txid)",
        [],
    )?;
    info!("   ✅ Verified idx_parent_txns_txid");

    // Index for Merkle proof lookups
    info!("   Creating index for Merkle proof lookups...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_merkle_proofs_parent_txid
         ON merkle_proofs(parent_txn_id)",
        [],
    )?;
    info!("   ✅ Created idx_merkle_proofs_parent_txid");

    // Indexes for block header lookups
    info!("   Creating indexes for block header lookups...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_block_headers_hash
         ON block_headers(block_hash)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_block_headers_height
         ON block_headers(height)",
        [],
    )?;
    info!("   ✅ Created block header indexes");

    info!("   ✅ Schema version 4 migration complete");
    Ok(())
}

/// Create schema version 5 (Group C enhancements)
///
/// Adds support for:
/// - Custom instructions on UTXOs (for BRC-29 payments)
pub fn create_schema_v5(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 5 (Group C enhancements)...");

    // Add custom_instructions column to utxos table
    info!("   Adding custom_instructions column to utxos table...");
    // Check if column already exists (in case migration is run multiple times)
    let column_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('utxos') WHERE name = 'custom_instructions'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !column_exists {
        conn.execute(
            "ALTER TABLE utxos ADD COLUMN custom_instructions TEXT",
            [],
        )?;
        info!("   ✅ Added custom_instructions column to utxos table");
    } else {
        info!("   ✅ custom_instructions column already exists");
    }

    info!("   ✅ Schema version 5 migration complete");
    Ok(())
}

/// Create schema version 6 (Tag tables for listOutputs)
///
/// Adds support for:
/// - output_tags table (for output categorization)
/// - output_tag_map table (many-to-many relationship)
///
/// This migration is safe to run on existing databases - it only creates tables if they don't exist.
pub fn create_schema_v6(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 6 (Tag tables for listOutputs)...");

    // Check if output_tags table exists
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='output_tags'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !table_exists {
        info!("   Creating output_tags table...");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS output_tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tag TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                is_deleted BOOLEAN NOT NULL DEFAULT 0,
                UNIQUE(tag)
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tags_tag ON output_tags(tag)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tags_deleted ON output_tags(is_deleted) WHERE is_deleted = 0", [])?;
        info!("   ✅ Created output_tags table");
    } else {
        info!("   ✅ output_tags table already exists");
    }

    // Check if output_tag_map table exists
    let map_table_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='output_tag_map'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !map_table_exists {
        info!("   Creating output_tag_map table...");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS output_tag_map (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                output_id INTEGER NOT NULL,
                output_tag_id INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                is_deleted BOOLEAN NOT NULL DEFAULT 0,
                FOREIGN KEY (output_id) REFERENCES utxos(id) ON DELETE CASCADE,
                FOREIGN KEY (output_tag_id) REFERENCES output_tags(id) ON DELETE CASCADE,
                UNIQUE(output_id, output_tag_id)
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tag_map_output_id ON output_tag_map(output_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tag_map_tag_id ON output_tag_map(output_tag_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tag_map_deleted ON output_tag_map(is_deleted) WHERE is_deleted = 0", [])?;
        info!("   ✅ Created output_tag_map table");
    } else {
        info!("   ✅ output_tag_map table already exists");
    }

    info!("   ✅ Schema version 6 migration complete");
    Ok(())
}

/// Create schema version 7 (Certificate Management - Part 3)
///
/// Adds support for:
/// - certificate_fields table (for storing individual certificate fields separately)
/// - Enhanced certificates table with BRC-52 fields
///
/// This migration:
/// 1. Creates certificate_fields table for better querying and selective disclosure
/// 2. Adds missing BRC-52 fields to certificates table
/// 3. Migrates existing certificate data from JSON attributes to certificate_fields table
pub fn create_schema_v7(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 7 (Certificate Management - Part 3)...");

    // Step 1: Add missing BRC-52 fields to certificates table
    info!("   Step 1: Adding BRC-52 fields to certificates table...");

    // Check if relinquished column exists (for data migration)
    let relinquished_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('certificates') WHERE name = 'relinquished'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    // Check and add each column if it doesn't exist
    // Note: is_deleted is handled separately to migrate data from relinquished
    let columns_to_add = vec![
        ("type", "TEXT"),
        ("serial_number", "TEXT"),
        ("certifier", "TEXT"),
        ("subject", "TEXT"),
        ("verifier", "TEXT"),
        ("revocation_outpoint", "TEXT"),
        ("signature", "TEXT"),
    ];

    for (col_name, col_type) in columns_to_add {
        let column_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('certificates') WHERE name = ?1",
            [col_name],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);

        if !column_exists {
            let alter_sql = format!("ALTER TABLE certificates ADD COLUMN {} {}", col_name, col_type);
            conn.execute(&alter_sql, [])?;
            info!("   ✅ Added column: {}", col_name);
        } else {
            info!("   ✅ Column {} already exists", col_name);
        }
    }

    // Handle is_deleted column separately (migrate from relinquished if needed)
    let is_deleted_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('certificates') WHERE name = 'is_deleted'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !is_deleted_exists {
        // Add is_deleted column
        conn.execute(
            "ALTER TABLE certificates ADD COLUMN is_deleted BOOLEAN NOT NULL DEFAULT 0",
            [],
        )?;
        info!("   ✅ Added column: is_deleted");

        // Migrate data from relinquished if it exists
        if relinquished_exists {
            conn.execute(
                "UPDATE certificates SET is_deleted = relinquished WHERE relinquished = 1",
                [],
            )?;
            info!("   ✅ Migrated relinquished data to is_deleted");
        }
    } else {
        info!("   ✅ Column is_deleted already exists");
    }

    // Step 2: Create certificate_fields table
    info!("   Step 2: Creating certificate_fields table...");
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='certificate_fields'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !table_exists {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS certificate_fields (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                certificate_id INTEGER NOT NULL,
                field_name TEXT NOT NULL,
                field_value TEXT NOT NULL,
                master_key TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (certificate_id) REFERENCES certificates(id) ON DELETE CASCADE,
                UNIQUE(certificate_id, field_name)
            )",
            [],
        )?;
        info!("   ✅ Created certificate_fields table");
    } else {
        info!("   ✅ certificate_fields table already exists");
    }

    // Step 3: Create indexes for certificate_fields
    info!("   Step 3: Creating indexes for certificate_fields...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_certificate_fields_certificate_id ON certificate_fields(certificate_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_certificate_fields_field_name ON certificate_fields(field_name)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_certificate_fields_cert_field ON certificate_fields(certificate_id, field_name)",
        [],
    )?;
    info!("   ✅ Created certificate_fields indexes");

    // Step 4: Create additional indexes for certificates table
    info!("   Step 4: Creating additional indexes for certificates table...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_certificates_certifier ON certificates(certifier)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_certificates_subject ON certificates(subject)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_certificates_type ON certificates(type)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_certificates_is_deleted ON certificates(is_deleted) WHERE is_deleted = 0",
        [],
    )?;
    info!("   ✅ Created certificates indexes");

    // Step 5: Migrate existing certificate data (if any)
    info!("   Step 5: Migrating existing certificate data...");
    let existing_certs: i64 = conn.query_row(
        "SELECT COUNT(*) FROM certificates WHERE attributes IS NOT NULL AND attributes != ''",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if existing_certs > 0 {
        info!("   Found {} certificates with attributes to migrate", existing_certs);
        // Note: We'll parse JSON attributes and migrate to certificate_fields
        // This is a complex operation that may require JSON parsing
        // For now, we'll leave the attributes column and migrate on-demand
        // or in a separate data migration function
        info!("   ⚠️  Certificate data migration from attributes JSON will be handled separately");
        info!("   ⚠️  Existing certificates will continue to work with attributes column");
    } else {
        info!("   ✅ No existing certificates to migrate");
    }

    info!("   ✅ Schema version 7 migration complete");
    Ok(())
}

/// Create schema version 8 (BRC-33 Message Relay Persistence)
///
/// Adds persistent storage for the message relay system:
/// - relay_messages table (replaces in-memory storage)
///
/// This migration ensures messages survive wallet restarts.
pub fn create_schema_v8(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 8 (BRC-33 Message Relay Persistence)...");

    // Check if relay_messages table exists
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='relay_messages'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !table_exists {
        info!("   Creating relay_messages table...");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS relay_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                recipient TEXT NOT NULL,
                message_box TEXT NOT NULL,
                sender TEXT NOT NULL,
                body TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                expires_at INTEGER
            )",
            [],
        )?;
        info!("   ✅ Created relay_messages table");
    } else {
        info!("   ✅ relay_messages table already exists");
    }

    // Create indexes for efficient queries
    info!("   Creating indexes for relay_messages...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relay_recipient_box ON relay_messages(recipient, message_box)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relay_created_at ON relay_messages(created_at)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relay_expires ON relay_messages(expires_at) WHERE expires_at IS NOT NULL",
        [],
    )?;
    info!("   ✅ Created relay_messages indexes");

    info!("   ✅ Schema version 8 migration complete");
    Ok(())
}

/// Create schema version 9 (Basket & Tag Support Enhancements)
///
/// Adds support for:
/// - UTXO status tracking ('unproven', 'completed', 'failed')
/// - Basket partial index for efficient listOutputs queries
/// - Normalization of existing basket and tag names (trim + lowercase)
///
/// This migration enables BRC-100 basket and tag functionality.
pub fn create_schema_v9(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 9 (Basket & Tag Support Enhancements)...");

    // Step 1: Add status column to utxos table
    info!("   Step 1: Adding status column to utxos table...");
    let status_column_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('utxos') WHERE name = 'status'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !status_column_exists {
        conn.execute(
            "ALTER TABLE utxos ADD COLUMN status TEXT DEFAULT 'completed'",
            [],
        )?;
        info!("   ✅ Added status column to utxos table");

        // Explicitly set existing UTXOs to 'completed' (belt and suspenders)
        // Even though DEFAULT handles this, being explicit is safer
        let updated = conn.execute(
            "UPDATE utxos SET status = 'completed' WHERE status IS NULL",
            [],
        )?;
        info!("   ✅ Set status='completed' for {} existing UTXOs", updated);
    } else {
        info!("   ✅ status column already exists");
    }

    // Step 2: Add partial index for status queries (only index non-completed for performance)
    info!("   Step 2: Creating partial index for UTXO status...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_utxos_status ON utxos(status) WHERE status != 'completed'",
        [],
    )?;
    info!("   ✅ Created idx_utxos_status partial index");

    // Step 3: Add basket partial index for efficient listOutputs queries
    info!("   Step 3: Creating basket partial index for unspent UTXOs...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_utxos_basket_unspent ON utxos(basket_id, is_spent) WHERE is_spent = 0",
        [],
    )?;
    info!("   ✅ Created idx_utxos_basket_unspent partial index");

    // Step 4: Normalize existing basket names (trim + lowercase)
    info!("   Step 4: Normalizing existing basket names...");
    normalize_existing_baskets(conn)?;

    // Step 5: Normalize existing tag names (trim + lowercase)
    info!("   Step 5: Normalizing existing tag names...");
    normalize_existing_tags(conn)?;

    info!("   ✅ Schema version 9 migration complete");
    Ok(())
}

/// Normalize existing basket names to trim + lowercase
///
/// This ensures consistency with the new normalization rules:
/// - "Game Items" becomes "game items"
/// - "  WEAPONS  " becomes "weapons"
///
/// If normalization creates duplicates, they are merged.
fn normalize_existing_baskets(conn: &Connection) -> Result<()> {
    // Get all existing baskets
    let mut stmt = conn.prepare("SELECT id, name FROM baskets")?;
    let baskets: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    if baskets.is_empty() {
        info!("      No existing baskets to normalize");
        return Ok(());
    }

    let mut normalized_count = 0;
    let mut merged_count = 0;

    for (id, name) in &baskets {
        let normalized = name.trim().to_lowercase();

        if normalized != *name {
            // Check if normalized name already exists (different basket)
            let existing: std::result::Result<i64, rusqlite::Error> = conn.query_row(
                "SELECT id FROM baskets WHERE name = ?1 AND id != ?2",
                rusqlite::params![&normalized, id],
                |row| row.get(0),
            );

            match existing {
                Ok(existing_id) => {
                    // Merge: update all UTXOs to use existing basket, delete duplicate
                    conn.execute(
                        "UPDATE utxos SET basket_id = ?1 WHERE basket_id = ?2",
                        rusqlite::params![existing_id, id],
                    )?;
                    conn.execute(
                        "DELETE FROM baskets WHERE id = ?1",
                        rusqlite::params![id],
                    )?;
                    info!("      🔀 Merged basket '{}' (id={}) into '{}' (id={})",
                          name, id, normalized, existing_id);
                    merged_count += 1;
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    // No conflict: just rename
                    conn.execute(
                        "UPDATE baskets SET name = ?1 WHERE id = ?2",
                        rusqlite::params![&normalized, id],
                    )?;
                    info!("      📝 Normalized basket '{}' -> '{}'", name, normalized);
                    normalized_count += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    info!("      ✅ Normalized {} baskets, merged {} duplicates", normalized_count, merged_count);
    Ok(())
}

/// Normalize existing tag names to trim + lowercase
///
/// This ensures consistency with the new normalization rules.
/// If normalization creates duplicates, they are merged.
fn normalize_existing_tags(conn: &Connection) -> Result<()> {
    // Get all existing tags
    let mut stmt = conn.prepare("SELECT id, tag FROM output_tags")?;
    let tags: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    if tags.is_empty() {
        info!("      No existing tags to normalize");
        return Ok(());
    }

    let mut normalized_count = 0;
    let mut merged_count = 0;

    for (id, tag) in &tags {
        let normalized = tag.trim().to_lowercase();

        if normalized != *tag {
            let existing: std::result::Result<i64, rusqlite::Error> = conn.query_row(
                "SELECT id FROM output_tags WHERE tag = ?1 AND id != ?2",
                rusqlite::params![&normalized, id],
                |row| row.get(0),
            );

            match existing {
                Ok(existing_id) => {
                    // Merge: update tag mappings, delete duplicate
                    conn.execute(
                        "UPDATE output_tag_map SET output_tag_id = ?1 WHERE output_tag_id = ?2",
                        rusqlite::params![existing_id, id],
                    )?;
                    conn.execute(
                        "DELETE FROM output_tags WHERE id = ?1",
                        rusqlite::params![id],
                    )?;
                    info!("      🔀 Merged tag '{}' (id={}) into '{}' (id={})",
                          tag, id, normalized, existing_id);
                    merged_count += 1;
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    conn.execute(
                        "UPDATE output_tags SET tag = ?1 WHERE id = ?2",
                        rusqlite::params![&normalized, id],
                    )?;
                    info!("      📝 Normalized tag '{}' -> '{}'", tag, normalized);
                    normalized_count += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    info!("      ✅ Normalized {} tags, merged {} duplicates", normalized_count, merged_count);
    Ok(())
}

/// Create schema version 10 (Transaction Chaining Support)
///
/// Adds support for:
/// - broadcast_status column on transactions to track broadcast state
///   Values: 'pending', 'broadcast', 'confirmed', 'failed'
///
/// This enables proper BEEF building for child transactions that spend
/// outputs from unbroadcast parent transactions.
pub fn create_schema_v10(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 10 (Transaction Chaining Support)...");

    // Step 1: Add broadcast_status column to transactions table
    info!("   Step 1: Adding broadcast_status column to transactions table...");
    let column_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'broadcast_status'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !column_exists {
        conn.execute(
            "ALTER TABLE transactions ADD COLUMN broadcast_status TEXT DEFAULT 'pending'",
            [],
        )?;
        info!("   ✅ Added broadcast_status column to transactions table");

        // Set existing transactions to 'confirmed' (they were created before this tracking)
        // Transactions that have block_height are definitely confirmed
        let updated_confirmed = conn.execute(
            "UPDATE transactions SET broadcast_status = 'confirmed' WHERE block_height IS NOT NULL",
            [],
        )?;
        info!("   ✅ Set broadcast_status='confirmed' for {} transactions with block_height", updated_confirmed);

        // Transactions without block_height but with confirmations > 0 are also confirmed
        let updated_with_confirmations = conn.execute(
            "UPDATE transactions SET broadcast_status = 'confirmed' WHERE confirmations > 0 AND broadcast_status != 'confirmed'",
            [],
        )?;
        info!("   ✅ Set broadcast_status='confirmed' for {} transactions with confirmations", updated_with_confirmations);

        // Remaining transactions (no block_height, no confirmations) are assumed broadcast
        // (they were created before we added this tracking, so we don't know for sure)
        let updated_broadcast = conn.execute(
            "UPDATE transactions SET broadcast_status = 'broadcast' WHERE broadcast_status = 'pending' OR broadcast_status IS NULL",
            [],
        )?;
        info!("   ✅ Set broadcast_status='broadcast' for {} legacy transactions", updated_broadcast);
    } else {
        info!("   ✅ broadcast_status column already exists");
    }

    // Step 2: Create index for efficient queries by broadcast_status
    info!("   Step 2: Creating index for broadcast_status...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_transactions_broadcast_status ON transactions(broadcast_status)",
        [],
    )?;
    info!("   ✅ Created idx_transactions_broadcast_status index");

    // Step 3: Create index for txid lookups (should already exist, but ensure it does)
    info!("   Step 3: Ensuring txid index exists...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_transactions_txid ON transactions(txid)",
        [],
    )?;
    info!("   ✅ Verified idx_transactions_txid index");

    info!("   ✅ Schema version 10 migration complete");
    Ok(())
}

/// Create schema version 11 (Fix Orphan UTXOs)
///
/// This migration fixes UTXOs that were created with unsigned txids before
/// the signing process updated the transaction's txid. These "orphan" UTXOs
/// reference txids that no longer exist in the transactions table.
///
/// The fix works by matching orphan UTXOs with transactions based on:
/// - Output vout (position)
/// - Output satoshis (amount)
/// - Output script (locking script)
///
/// This is a one-time cleanup for legacy data. New transactions won't have
/// this problem because we now update UTXO txids after signing.
pub fn create_schema_v11(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 11 (Fix Orphan UTXOs)...");

    // Step 1: Find orphan UTXOs (UTXOs whose txid doesn't exist in transactions table)
    info!("   Step 1: Finding orphan UTXOs...");

    let mut orphan_stmt = conn.prepare(
        "SELECT u.id, u.txid, u.vout, u.satoshis, u.script
         FROM utxos u
         WHERE NOT EXISTS (
             SELECT 1 FROM transactions t WHERE t.txid = u.txid
         )
         AND u.is_spent = 0"
    )?;

    let orphans: Vec<(i64, String, i32, i64, Option<String>)> = orphan_stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,      // id
            row.get::<_, String>(1)?,   // txid
            row.get::<_, i32>(2)?,      // vout
            row.get::<_, i64>(3)?,      // satoshis
            row.get::<_, Option<String>>(4)?, // script
        ))
    })?.collect::<Result<Vec<_>>>()?;

    info!("   📊 Found {} orphan UTXOs to fix", orphans.len());

    if orphans.is_empty() {
        info!("   ✅ No orphan UTXOs found - nothing to fix");
        info!("   ✅ Schema version 11 migration complete");
        return Ok(());
    }

    // Step 2: For each orphan UTXO, try to find matching transaction by output characteristics
    info!("   Step 2: Matching orphan UTXOs with transactions...");

    let mut fixed_count = 0;
    let mut unfixed_count = 0;

    for (utxo_id, old_txid, vout, satoshis, script) in orphans {
        info!("   🔍 Processing orphan UTXO {} (txid: {}:{})", utxo_id, &old_txid[..16], vout);

        // Try to find a transaction with matching output
        // We match on vout, satoshis, and script (if available)
        let matching_txid: Option<String> = if let Some(ref script_hex) = script {
            // Match by vout + satoshis + script (most precise)
            conn.query_row(
                "SELECT t.txid
                 FROM transactions t
                 JOIN transaction_outputs o ON o.transaction_id = t.id
                 WHERE o.vout = ?1 AND o.satoshis = ?2 AND o.script = ?3
                 LIMIT 1",
                rusqlite::params![vout, satoshis, script_hex],
                |row| row.get(0),
            ).ok()
        } else {
            // Match by vout + satoshis only (less precise, but better than nothing)
            conn.query_row(
                "SELECT t.txid
                 FROM transactions t
                 JOIN transaction_outputs o ON o.transaction_id = t.id
                 WHERE o.vout = ?1 AND o.satoshis = ?2
                 LIMIT 1",
                rusqlite::params![vout, satoshis],
                |row| row.get(0),
            ).ok()
        };

        match matching_txid {
            Some(new_txid) => {
                // Update the UTXO's txid
                conn.execute(
                    "UPDATE utxos SET txid = ?1 WHERE id = ?2",
                    rusqlite::params![new_txid, utxo_id],
                )?;
                info!("   ✅ Fixed UTXO {}: {} → {}", utxo_id, &old_txid[..16], &new_txid[..16]);
                fixed_count += 1;
            }
            None => {
                // Could not find matching transaction
                // This UTXO might reference a transaction that was never stored locally
                // (e.g., synced from API but parent tx not in our DB)
                info!("   ⚠️  Could not match UTXO {} (txid: {}:{}) - may be from external source",
                      utxo_id, &old_txid[..16], vout);
                unfixed_count += 1;
            }
        }
    }

    info!("   📊 Migration results:");
    info!("      - Fixed: {} UTXOs", fixed_count);
    info!("      - Unfixed: {} UTXOs (external/legacy)", unfixed_count);

    info!("   ✅ Schema version 11 migration complete");
    Ok(())
}

/// Create schema version 12 (Basket Output Tracking - nullable address_id)
///
/// Makes address_id nullable in the utxos table to support basket outputs
/// that don't correspond to a known HD wallet address (e.g., token scripts,
/// custom OP_RETURN+P2PKH outputs from createAction with basket property).
///
/// SQLite doesn't support ALTER COLUMN to change NOT NULL constraints,
/// so we recreate the table with the new schema.
pub fn create_schema_v12(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 12 (Basket Output Tracking - nullable address_id)...");

    // Step 1: Recreate utxos table with nullable address_id
    info!("   Step 1: Recreating utxos table with nullable address_id...");

    conn.execute_batch("
        CREATE TABLE utxos_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            address_id INTEGER,
            basket_id INTEGER,
            txid TEXT NOT NULL,
            vout INTEGER NOT NULL,
            satoshis INTEGER NOT NULL,
            script TEXT NOT NULL,
            first_seen INTEGER NOT NULL,
            last_updated INTEGER NOT NULL,
            is_spent BOOLEAN NOT NULL DEFAULT 0,
            spent_txid TEXT,
            spent_at INTEGER,
            custom_instructions TEXT,
            status TEXT DEFAULT 'completed',
            FOREIGN KEY (address_id) REFERENCES addresses(id) ON DELETE CASCADE,
            FOREIGN KEY (basket_id) REFERENCES baskets(id) ON DELETE SET NULL,
            UNIQUE(txid, vout)
        );

        INSERT INTO utxos_new SELECT * FROM utxos;

        DROP TABLE utxos;

        ALTER TABLE utxos_new RENAME TO utxos;
    ")?;

    info!("   ✅ Recreated utxos table with nullable address_id");

    // Step 2: Recreate indexes
    info!("   Step 2: Recreating indexes...");

    conn.execute("CREATE INDEX IF NOT EXISTS idx_utxos_address_id ON utxos(address_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_utxos_txid_vout ON utxos(txid, vout)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_utxos_basket_id ON utxos(basket_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_utxos_status ON utxos(status) WHERE status != 'completed'", [])?;

    info!("   ✅ Indexes recreated");
    info!("   ✅ Schema version 12 migration complete");
    Ok(())
}

/// Schema V13: Persistent derived key cache for PushDrop signing
///
/// When getPublicKey is called with forSelf=true, the wallet derives a child key
/// for locking PushDrop tokens. To sign those tokens later (even after restart),
/// we need to remember the derivation parameters (invoice + counterparty).
pub fn create_schema_v13(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 13 (Persistent derived key cache)...");

    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS derived_key_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            derived_pubkey TEXT NOT NULL UNIQUE,
            invoice TEXT NOT NULL,
            counterparty_pubkey TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_derived_key_cache_pubkey ON derived_key_cache(derived_pubkey);
    ")?;

    info!("   ✅ Schema version 13 migration complete");
    Ok(())
}

/// Schema V14: Output description storage for basket outputs
///
/// BRC-100 createAction outputs can include an `outputDescription` field (5-50 bytes UTF-8)
/// that describes the output. This is stored alongside custom_instructions for retrieval
/// via listOutputs. Matches the SDK's `OutputX.outputDescription` field.
pub fn create_schema_v14(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 14 (Output description for basket outputs)...");

    conn.execute(
        "ALTER TABLE utxos ADD COLUMN output_description TEXT",
        [],
    )?;

    info!("   ✅ Schema version 14 migration complete");
    Ok(())
}

/// Schema V15: Status Consolidation + UnFail Foundation
///
/// Phase 1 of wallet-toolbox alignment. Consolidates the dual status system
/// (ActionStatus + broadcast_status) into a single `new_status` column aligned
/// with the BSV SDK wallet-toolbox TransactionStatus values.
///
/// Also adds `failed_at` timestamp for the UnFail mechanism: when a transaction
/// is marked failed, we record when. The cleanup job waits 30 minutes before
/// permanently cleaning up, giving time to re-check the blockchain in case the
/// transaction was actually mined (preventing fund loss from premature cleanup).
///
/// New status values: unsigned, sending, unproven, completed, nosend, nonfinal, failed, unprocessed
///
/// Mapping from old dual status:
///   (created, pending)       → unsigned
///   (signed, broadcast)      → sending
///   (unconfirmed, broadcast) → unproven
///   (pending, *)             → unproven
///   (confirmed, confirmed)   → completed
///   (aborted, *)             → nosend
///   (failed, failed)         → failed
pub fn create_schema_v15(conn: &Connection) -> Result<()> {
    info!("   Creating schema version 15 (Status Consolidation + UnFail)...");

    // Step 1: Add new_status column
    info!("   Step 1: Adding new_status column to transactions table...");
    let new_status_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'new_status'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !new_status_exists {
        conn.execute(
            "ALTER TABLE transactions ADD COLUMN new_status TEXT DEFAULT 'unsigned'",
            [],
        )?;
        info!("   ✅ Added new_status column");
    } else {
        info!("   ✅ new_status column already exists");
    }

    // Step 2: Add failed_at column (timestamp when status was set to 'failed')
    info!("   Step 2: Adding failed_at column to transactions table...");
    let failed_at_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'failed_at'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !failed_at_exists {
        conn.execute(
            "ALTER TABLE transactions ADD COLUMN failed_at INTEGER",
            [],
        )?;
        info!("   ✅ Added failed_at column");
    } else {
        info!("   ✅ failed_at column already exists");
    }

    // Step 3: Populate new_status from existing status + broadcast_status
    info!("   Step 3: Populating new_status from legacy columns...");

    // Check if broadcast_status column exists (v10 migration)
    let has_broadcast_status: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'broadcast_status'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if has_broadcast_status {
        // Full mapping using both columns
        // confirmed + confirmed → completed
        let updated = conn.execute(
            "UPDATE transactions SET new_status = 'completed'
             WHERE status = 'confirmed' OR (broadcast_status = 'confirmed')",
            [],
        )?;
        info!("      ✅ Set {} transactions to 'completed'", updated);

        // failed + failed → failed (also set failed_at)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let updated = conn.execute(
            "UPDATE transactions SET new_status = 'failed', failed_at = ?1
             WHERE new_status != 'completed' AND (status = 'failed' OR broadcast_status = 'failed')",
            rusqlite::params![now],
        )?;
        info!("      ✅ Set {} transactions to 'failed'", updated);

        // aborted → nosend
        let updated = conn.execute(
            "UPDATE transactions SET new_status = 'nosend'
             WHERE new_status NOT IN ('completed', 'failed') AND status = 'aborted'",
            [],
        )?;
        info!("      ✅ Set {} transactions to 'nosend'", updated);

        // unconfirmed/pending + broadcast → unproven
        let updated = conn.execute(
            "UPDATE transactions SET new_status = 'unproven'
             WHERE new_status NOT IN ('completed', 'failed', 'nosend')
               AND status IN ('unconfirmed', 'pending')
               AND broadcast_status IN ('broadcast', 'confirmed')",
            [],
        )?;
        info!("      ✅ Set {} transactions to 'unproven'", updated);

        // signed + broadcast → sending
        let updated = conn.execute(
            "UPDATE transactions SET new_status = 'sending'
             WHERE new_status NOT IN ('completed', 'failed', 'nosend', 'unproven')
               AND status = 'signed' AND broadcast_status = 'broadcast'",
            [],
        )?;
        info!("      ✅ Set {} transactions to 'sending'", updated);

        // created + pending → unsigned (everything else still in 'unsigned' default)
        let updated = conn.execute(
            "UPDATE transactions SET new_status = 'unsigned'
             WHERE new_status NOT IN ('completed', 'failed', 'nosend', 'unproven', 'sending')
               AND status = 'created'",
            [],
        )?;
        info!("      ✅ Set {} transactions to 'unsigned'", updated);

        // Catch-all: anything remaining that's broadcast but not categorized → unproven
        let updated = conn.execute(
            "UPDATE transactions SET new_status = 'unproven'
             WHERE new_status = 'unsigned' AND broadcast_status = 'broadcast'",
            [],
        )?;
        if updated > 0 {
            info!("      ✅ Catch-all: set {} remaining broadcast transactions to 'unproven'", updated);
        }
    } else {
        // No broadcast_status column — map from ActionStatus only
        info!("      ℹ️  No broadcast_status column - mapping from status only");

        conn.execute(
            "UPDATE transactions SET new_status = 'completed' WHERE status = 'confirmed'",
            [],
        )?;
        conn.execute(
            "UPDATE transactions SET new_status = 'unproven' WHERE status IN ('unconfirmed', 'pending')",
            [],
        )?;
        conn.execute(
            "UPDATE transactions SET new_status = 'sending' WHERE status = 'signed'",
            [],
        )?;
        conn.execute(
            "UPDATE transactions SET new_status = 'nosend' WHERE status = 'aborted'",
            [],
        )?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "UPDATE transactions SET new_status = 'failed', failed_at = ?1 WHERE status = 'failed'",
            rusqlite::params![now],
        )?;
        // created → unsigned (already the default)
    }

    // Step 4: Create index on new_status for efficient queries
    info!("   Step 4: Creating index on new_status...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_transactions_new_status ON transactions(new_status)",
        [],
    )?;
    info!("   ✅ Created idx_transactions_new_status index");

    // Step 5: Create partial index for UnFail queries (failed transactions with timestamp)
    info!("   Step 5: Creating index for UnFail queries...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_transactions_failed_at ON transactions(failed_at) WHERE new_status = 'failed'",
        [],
    )?;
    info!("   ✅ Created idx_transactions_failed_at index");

    info!("   ✅ Schema version 15 migration complete");
    Ok(())
}

/// Migration v16: Proven Transaction Model
///
/// Creates `proven_txs` and `proven_tx_reqs` tables. Adds `proven_tx_id` FK to
/// transactions. Migrates existing proof data from parent_transactions + merkle_proofs
/// into proven_txs. Links completed transactions to their proven_txs records.
pub fn create_schema_v16(conn: &Connection) -> Result<()> {
    info!("=== Migration v16: Proven Transaction Model ===");

    // Step 1: Create proven_txs table (immutable proof records)
    info!("   Step 1: Creating proven_txs table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS proven_txs (
            provenTxId INTEGER PRIMARY KEY AUTOINCREMENT,
            txid TEXT NOT NULL UNIQUE,
            height INTEGER NOT NULL,
            tx_index INTEGER NOT NULL,
            merkle_path BLOB NOT NULL,
            raw_tx BLOB NOT NULL,
            block_hash TEXT NOT NULL DEFAULT '',
            merkle_root TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_proven_txs_txid ON proven_txs(txid)",
        [],
    )?;
    info!("   ✅ Created proven_txs table");

    // Step 2: Create proven_tx_reqs table (mutable proof request lifecycle)
    info!("   Step 2: Creating proven_tx_reqs table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS proven_tx_reqs (
            provenTxReqId INTEGER PRIMARY KEY AUTOINCREMENT,
            proven_tx_id INTEGER REFERENCES proven_txs(provenTxId),
            status TEXT NOT NULL DEFAULT 'unknown',
            attempts INTEGER NOT NULL DEFAULT 0,
            notified INTEGER NOT NULL DEFAULT 0,
            txid TEXT NOT NULL UNIQUE,
            batch TEXT,
            history TEXT NOT NULL DEFAULT '{}',
            notify TEXT NOT NULL DEFAULT '{}',
            raw_tx BLOB NOT NULL,
            input_beef BLOB,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_proven_tx_reqs_status ON proven_tx_reqs(status)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_proven_tx_reqs_txid ON proven_tx_reqs(txid)",
        [],
    )?;
    info!("   ✅ Created proven_tx_reqs table");

    // Step 3: Add proven_tx_id FK column to transactions table
    info!("   Step 3: Adding proven_tx_id to transactions table...");
    let has_proven_tx_id: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'proven_tx_id'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !has_proven_tx_id {
        conn.execute(
            "ALTER TABLE transactions ADD COLUMN proven_tx_id INTEGER REFERENCES proven_txs(provenTxId)",
            [],
        )?;
        info!("   ✅ Added proven_tx_id column to transactions");
    } else {
        info!("   ⏭️  proven_tx_id column already exists, skipping");
    }

    // Step 4: Migrate existing proof data from parent_transactions + merkle_proofs → proven_txs
    info!("   Step 4: Migrating existing proof data to proven_txs...");

    // Check if merkle_proofs table exists (it should, but be safe)
    let has_merkle_proofs: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='merkle_proofs'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if has_merkle_proofs {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Query existing merkle proofs joined with parent transactions and block headers
        let mut stmt = conn.prepare(
            "SELECT pt.txid, mp.block_height, mp.tx_index, mp.target_hash, mp.nodes,
                    pt.raw_hex, pt.cached_at,
                    COALESCE(bh.block_hash, '') as bh_hash
             FROM merkle_proofs mp
             INNER JOIN parent_transactions pt ON mp.parent_txn_id = pt.id
             LEFT JOIN block_headers bh ON bh.height = mp.block_height"
        )?;

        let rows: Vec<(String, u32, u64, String, String, String, i64, String)> = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,      // txid
                row.get::<_, i64>(1)? as u32,  // block_height
                row.get::<_, i64>(2)? as u64,  // tx_index
                row.get::<_, String>(3)?,       // target_hash
                row.get::<_, String>(4)?,       // nodes (JSON string)
                row.get::<_, String>(5)?,       // raw_hex
                row.get::<_, i64>(6)?,          // cached_at
                row.get::<_, String>(7)?,       // block_hash
            ))
        })?.filter_map(|r| r.ok()).collect();

        let mut migrated_count = 0;
        for (txid, block_height, tx_index, target_hash, nodes_json, raw_hex, cached_at, block_hash) in &rows {
            // Reconstruct TSC JSON from merkle proof fields
            // The nodes column is already a JSON string like '["hash1","hash2","*"]'
            let nodes_value: serde_json::Value = serde_json::from_str(nodes_json)
                .unwrap_or_else(|_| serde_json::json!([]));

            let tsc_json = serde_json::json!({
                "index": tx_index,
                "target": target_hash,
                "nodes": nodes_value,
                "height": block_height
            });

            let merkle_path_bytes = serde_json::to_vec(&tsc_json)
                .unwrap_or_default();

            // Decode raw_hex to bytes for raw_tx BLOB
            let raw_tx_bytes = hex::decode(raw_hex).unwrap_or_default();

            // Insert into proven_txs (IGNORE if txid already exists)
            match conn.execute(
                "INSERT OR IGNORE INTO proven_txs
                 (txid, height, tx_index, merkle_path, raw_tx, block_hash, merkle_root, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', ?7, ?8)",
                rusqlite::params![
                    txid, *block_height as i64, *tx_index as i64,
                    merkle_path_bytes, raw_tx_bytes, block_hash,
                    cached_at, now
                ],
            ) {
                Ok(_) => migrated_count += 1,
                Err(e) => warn!("   ⚠️  Failed to migrate proof for {}: {}", txid, e),
            }
        }
        info!("   ✅ Migrated {} proof records to proven_txs", migrated_count);
    } else {
        info!("   ⏭️  No merkle_proofs table found, skipping data migration");
    }

    // Step 5: Link completed transactions to their proven_txs records
    info!("   Step 5: Linking completed transactions to proven_txs...");

    // Check which status column to use
    let has_new_status: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'new_status'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    let status_filter = if has_new_status {
        "new_status = 'completed'"
    } else {
        "status = 'confirmed'"
    };

    let linked = conn.execute(
        &format!(
            "UPDATE transactions SET proven_tx_id = (
                SELECT provenTxId FROM proven_txs WHERE proven_txs.txid = transactions.txid
            ) WHERE {} AND proven_tx_id IS NULL AND EXISTS (
                SELECT 1 FROM proven_txs WHERE proven_txs.txid = transactions.txid
            )", status_filter
        ),
        [],
    )?;
    info!("   ✅ Linked {} completed transactions to proven_txs", linked);

    info!("   ✅ Schema version 16 migration complete");
    Ok(())
}

/// Migration v17: Multi-User Foundation
///
/// Creates `users` table and adds `user_id` foreign keys to core tables.
/// Creates a default user from the existing wallet's master public key.
/// This is plumbing for wallet-toolbox alignment where data is scoped by user.
///
/// For single-user wallets (our current model), there is exactly one user
/// whose identity_key is the wallet's master public key.
pub fn create_schema_v17(conn: &Connection) -> Result<()> {
    info!("=== Migration v17: Multi-User Foundation ===");

    // Step 1: Create users table
    info!("   Step 1: Creating users table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            userId INTEGER PRIMARY KEY AUTOINCREMENT,
            identity_key TEXT NOT NULL UNIQUE,
            active_storage TEXT NOT NULL DEFAULT 'local',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_users_identity_key ON users(identity_key)",
        [],
    )?;
    info!("   ✅ Created users table");

    // Step 2: Derive identity key from wallet's master public key and create default user
    info!("   Step 2: Creating default user from wallet master key...");

    // Check if there's a wallet to derive from
    let wallet_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM wallets",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    let default_user_id: Option<i64> = if wallet_exists {
        // Get the mnemonic from the primary wallet
        let mnemonic_str: String = conn.query_row(
            "SELECT mnemonic FROM wallets ORDER BY id ASC LIMIT 1",
            [],
            |row| row.get(0),
        )?;

        // Derive master public key (identity key) from mnemonic
        let identity_key = derive_identity_key_from_mnemonic(&mnemonic_str)
            .map_err(|e| rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some(format!("Failed to derive identity key: {}", e))
            ))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert default user
        conn.execute(
            "INSERT INTO users (identity_key, active_storage, created_at, updated_at)
             VALUES (?1, 'local', ?2, ?3)",
            rusqlite::params![identity_key, now, now],
        )?;

        let user_id = conn.last_insert_rowid();
        info!("   ✅ Created default user with ID {} and identity_key: {}...",
              user_id, &identity_key[..16]);
        Some(user_id)
    } else {
        info!("   ⏭️  No wallet found, skipping default user creation");
        None
    };

    // Step 3: Add user_id columns to core tables
    info!("   Step 3: Adding user_id columns to core tables...");

    // transactions table
    let has_user_id: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('transactions') WHERE name = 'user_id'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !has_user_id {
        conn.execute(
            "ALTER TABLE transactions ADD COLUMN user_id INTEGER REFERENCES users(userId)",
            [],
        )?;
        info!("   ✅ Added user_id to transactions");
    } else {
        info!("   ⏭️  transactions.user_id already exists");
    }

    // baskets table
    let has_user_id: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('baskets') WHERE name = 'user_id'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !has_user_id {
        conn.execute(
            "ALTER TABLE baskets ADD COLUMN user_id INTEGER REFERENCES users(userId)",
            [],
        )?;
        info!("   ✅ Added user_id to baskets");
    } else {
        info!("   ⏭️  baskets.user_id already exists");
    }

    // output_tags table
    let has_user_id: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('output_tags') WHERE name = 'user_id'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !has_user_id {
        conn.execute(
            "ALTER TABLE output_tags ADD COLUMN user_id INTEGER REFERENCES users(userId)",
            [],
        )?;
        info!("   ✅ Added user_id to output_tags");
    } else {
        info!("   ⏭️  output_tags.user_id already exists");
    }

    // certificates table
    let has_user_id: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('certificates') WHERE name = 'user_id'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !has_user_id {
        conn.execute(
            "ALTER TABLE certificates ADD COLUMN user_id INTEGER REFERENCES users(userId)",
            [],
        )?;
        info!("   ✅ Added user_id to certificates");
    } else {
        info!("   ⏭️  certificates.user_id already exists");
    }

    // certificate_fields table
    let has_user_id: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('certificate_fields') WHERE name = 'user_id'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !has_user_id {
        conn.execute(
            "ALTER TABLE certificate_fields ADD COLUMN user_id INTEGER REFERENCES users(userId)",
            [],
        )?;
        info!("   ✅ Added user_id to certificate_fields");
    } else {
        info!("   ⏭️  certificate_fields.user_id already exists");
    }

    // Step 4: Backfill user_id for all existing data
    if let Some(user_id) = default_user_id {
        info!("   Step 4: Backfilling user_id for existing data...");

        let updated = conn.execute(
            "UPDATE transactions SET user_id = ?1 WHERE user_id IS NULL",
            rusqlite::params![user_id],
        )?;
        info!("      ✅ Updated {} transactions", updated);

        let updated = conn.execute(
            "UPDATE baskets SET user_id = ?1 WHERE user_id IS NULL",
            rusqlite::params![user_id],
        )?;
        info!("      ✅ Updated {} baskets", updated);

        let updated = conn.execute(
            "UPDATE output_tags SET user_id = ?1 WHERE user_id IS NULL",
            rusqlite::params![user_id],
        )?;
        info!("      ✅ Updated {} output_tags", updated);

        let updated = conn.execute(
            "UPDATE certificates SET user_id = ?1 WHERE user_id IS NULL",
            rusqlite::params![user_id],
        )?;
        info!("      ✅ Updated {} certificates", updated);

        let updated = conn.execute(
            "UPDATE certificate_fields SET user_id = ?1 WHERE user_id IS NULL",
            rusqlite::params![user_id],
        )?;
        info!("      ✅ Updated {} certificate_fields", updated);
    } else {
        info!("   Step 4: No default user to backfill (empty wallet)");
    }

    // Step 5: Create indexes for user_id lookups
    info!("   Step 5: Creating user_id indexes...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_transactions_user_id ON transactions(user_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_baskets_user_id ON baskets(user_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_output_tags_user_id ON output_tags(user_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_certificates_user_id ON certificates(user_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_certificate_fields_user_id ON certificate_fields(user_id)",
        [],
    )?;
    info!("   ✅ Created user_id indexes");

    info!("   ✅ Schema version 17 migration complete");
    Ok(())
}

/// Helper function to derive identity key (master public key as hex) from mnemonic
fn derive_identity_key_from_mnemonic(mnemonic_str: &str) -> std::result::Result<String, String> {
    // Parse mnemonic
    let mnemonic = Mnemonic::parse_in(Language::English, mnemonic_str)
        .map_err(|e| format!("Invalid mnemonic: {}", e))?;

    // Generate seed from mnemonic (no password)
    let seed = mnemonic.to_seed("");

    // Create BIP32 master key from seed
    let master_key = XPrv::new(&seed)
        .map_err(|e| format!("Failed to create master key: {}", e))?;

    // Extract 32-byte master private key
    let private_key_bytes = master_key.private_key().to_bytes();

    // Derive public key
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&private_key_bytes)
        .map_err(|e| format!("Invalid private key: {}", e))?;

    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // Return compressed public key as hex string
    Ok(hex::encode(public_key.serialize()))
}

/// Migration v18: Output Model Transition (Phase 4A)
///
/// Creates `outputs` table to replace `utxos` with wallet-toolbox compatible schema.
/// Migrates existing UTXO data to outputs table with proper derivation info.
/// The `utxos` table is kept intact for parallel operation during transition.
///
/// Key changes:
/// - `spendable` (bool) replaces `is_spent` (inverted logic)
/// - `spent_by` (FK) replaces `spent_txid` (text)
/// - `derivation_prefix`/`derivation_suffix` replace `address_id` lookup
/// - `transaction_id` FK links output to creating transaction
/// - `user_id` FK for multi-user support (from Phase 3)
pub fn create_schema_v18(conn: &Connection) -> Result<()> {
    info!("=== Migration v18: Output Model Transition (Phase 4A) ===");

    // Step 1: Create outputs table
    info!("   Step 1: Creating outputs table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS outputs (
            outputId INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL REFERENCES users(userId),
            transaction_id INTEGER REFERENCES transactions(id),
            basket_id INTEGER REFERENCES baskets(id),
            spendable INTEGER NOT NULL DEFAULT 0,
            change INTEGER NOT NULL DEFAULT 0,
            vout INTEGER NOT NULL,
            satoshis INTEGER NOT NULL,
            provided_by TEXT NOT NULL DEFAULT 'you',
            purpose TEXT NOT NULL DEFAULT '',
            type TEXT NOT NULL DEFAULT '',
            output_description TEXT,
            txid TEXT,
            sender_identity_key TEXT,
            derivation_prefix TEXT,
            derivation_suffix TEXT,
            custom_instructions TEXT,
            spent_by INTEGER REFERENCES transactions(id),
            sequence_number INTEGER,
            spending_description TEXT,
            script_length INTEGER,
            script_offset INTEGER,
            locking_script BLOB,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;
    info!("   ✅ Created outputs table");

    // Step 2: Create indexes
    info!("   Step 2: Creating indexes...");
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_outputs_spendable ON outputs(spendable)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_outputs_user_id ON outputs(user_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_outputs_txid ON outputs(txid)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_outputs_basket_id ON outputs(basket_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_outputs_transaction_id ON outputs(transaction_id)",
        [],
    )?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_outputs_txid_vout ON outputs(txid, vout)",
        [],
    )?;
    info!("   ✅ Created indexes");

    // Step 3: Get default user ID
    let default_user_id: i64 = conn.query_row(
        "SELECT userId FROM users ORDER BY userId ASC LIMIT 1",
        [],
        |row| row.get(0),
    ).unwrap_or(1);
    info!("   Using default user ID: {}", default_user_id);

    // Step 4: Migrate data from utxos → outputs
    info!("   Step 4: Migrating data from utxos to outputs...");

    // Count utxos for progress reporting
    let utxo_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM utxos",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    info!("      Found {} UTXOs to migrate", utxo_count);

    if utxo_count > 0 {
        // Migrate basic fields with SQL INSERT...SELECT
        // We'll handle derivation_prefix/suffix in a second pass
        let migrated = conn.execute(
            "INSERT INTO outputs (
                user_id, transaction_id, basket_id, spendable, change, vout, satoshis,
                provided_by, purpose, type, output_description, txid,
                custom_instructions, locking_script, created_at, updated_at
            )
            SELECT
                ?1,                                           -- user_id (default user)
                t.id,                                         -- transaction_id (from txid lookup)
                u.basket_id,                                  -- basket_id
                CASE
                    WHEN u.is_spent = 1 THEN 0                -- spent = not spendable
                    WHEN u.status = 'failed' THEN 0           -- failed = not spendable
                    ELSE 1                                     -- unspent + not failed = spendable
                END,                                          -- spendable
                0,                                            -- change (unknown for legacy)
                u.vout,                                       -- vout
                u.satoshis,                                   -- satoshis
                'you',                                        -- provided_by
                '',                                           -- purpose
                '',                                           -- type
                u.output_description,                         -- output_description
                u.txid,                                       -- txid
                u.custom_instructions,                        -- custom_instructions
                CAST(u.script AS BLOB),                       -- locking_script (hex→blob)
                u.first_seen,                                 -- created_at
                u.last_updated                                -- updated_at
            FROM utxos u
            LEFT JOIN transactions t ON t.txid = u.txid",
            rusqlite::params![default_user_id],
        )?;
        info!("      ✅ Migrated {} rows to outputs", migrated);

        // Step 5: Set spent_by for spent outputs
        info!("   Step 5: Setting spent_by for spent outputs...");
        let spent_updated = conn.execute(
            "UPDATE outputs SET spent_by = (
                SELECT t.id FROM transactions t
                WHERE t.txid = (
                    SELECT u.spent_txid FROM utxos u
                    WHERE u.txid = outputs.txid AND u.vout = outputs.vout
                )
            )
            WHERE spendable = 0 AND spent_by IS NULL",
            [],
        )?;
        info!("      ✅ Set spent_by for {} spent outputs", spent_updated);

        // Step 6: Populate derivation_prefix/suffix from address index
        info!("   Step 6: Populating derivation info from addresses...");
        populate_derivation_info(conn)?;
    } else {
        info!("      ✅ No UTXOs to migrate (empty wallet)");
    }

    // Step 7: Verify migration
    info!("   Step 7: Verifying migration...");
    let output_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM outputs",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if output_count == utxo_count {
        info!("      ✅ Row count matches: {} outputs", output_count);
    } else {
        warn!("      ⚠️  Row count mismatch: {} utxos vs {} outputs", utxo_count, output_count);
    }

    // Verify spendable counts
    let unspent_utxos: i64 = conn.query_row(
        "SELECT COUNT(*) FROM utxos WHERE is_spent = 0",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let spendable_outputs: i64 = conn.query_row(
        "SELECT COUNT(*) FROM outputs WHERE spendable = 1",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if unspent_utxos == spendable_outputs {
        info!("      ✅ Spendable count matches: {} spendable outputs", spendable_outputs);
    } else {
        warn!("      ⚠️  Spendable mismatch: {} unspent utxos vs {} spendable outputs", unspent_utxos, spendable_outputs);
    }

    // Verify balance
    let utxo_balance: i64 = conn.query_row(
        "SELECT COALESCE(SUM(satoshis), 0) FROM utxos WHERE is_spent = 0",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let output_balance: i64 = conn.query_row(
        "SELECT COALESCE(SUM(satoshis), 0) FROM outputs WHERE spendable = 1",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if utxo_balance == output_balance {
        info!("      ✅ Balance matches: {} satoshis", output_balance);
    } else {
        warn!("      ⚠️  Balance mismatch: {} utxo balance vs {} output balance", utxo_balance, output_balance);
    }

    info!("   ✅ Schema version 18 migration complete");
    Ok(())
}

/// Populate derivation_prefix/suffix for outputs based on address index
///
/// Derivation mapping:
/// - index >= 0: derivation_prefix = "2-receive address", derivation_suffix = "{index}"
/// - index == -1: derivation_prefix = NULL, derivation_suffix = NULL (master pubkey)
/// - index < -1: Parse custom_instructions for BRC-29 derivation params
fn populate_derivation_info(conn: &Connection) -> Result<()> {
    // Query outputs joined with utxos and addresses to get derivation info
    let mut stmt = conn.prepare(
        "SELECT o.outputId, u.address_id, a.\"index\", o.custom_instructions
         FROM outputs o
         INNER JOIN utxos u ON u.txid = o.txid AND u.vout = o.vout
         LEFT JOIN addresses a ON a.id = u.address_id"
    )?;

    let rows: Vec<(i64, Option<i64>, Option<i32>, Option<String>)> = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,           // outputId
            row.get::<_, Option<i64>>(1)?,   // address_id
            row.get::<_, Option<i32>>(2)?,   // index
            row.get::<_, Option<String>>(3)?, // custom_instructions
        ))
    })?.filter_map(|r| r.ok()).collect();

    let mut hd_count = 0;
    let mut master_count = 0;
    let mut brc29_count = 0;
    let mut no_address_count = 0;

    for (output_id, address_id, index, custom_instructions) in rows {
        match (address_id, index) {
            (Some(_), Some(idx)) if idx >= 0 => {
                // Regular HD address: "2-receive address-{index}"
                conn.execute(
                    "UPDATE outputs SET derivation_prefix = ?1, derivation_suffix = ?2 WHERE outputId = ?3",
                    rusqlite::params!["2-receive address", idx.to_string(), output_id],
                )?;
                hd_count += 1;
            }
            (Some(_), Some(-1)) => {
                // Master pubkey address: NULL derivation (root key)
                // Already NULL by default, just count
                master_count += 1;
            }
            (Some(_), Some(idx)) if idx < -1 => {
                // BRC-29 derived address: parse custom_instructions
                if let Some(ref instructions) = custom_instructions {
                    if let Some((prefix, suffix)) = parse_brc29_derivation(instructions) {
                        conn.execute(
                            "UPDATE outputs SET derivation_prefix = ?1, derivation_suffix = ?2 WHERE outputId = ?3",
                            rusqlite::params![prefix, suffix, output_id],
                        )?;
                        brc29_count += 1;
                    } else {
                        warn!("      ⚠️  Could not parse BRC-29 derivation for output {}", output_id);
                    }
                } else {
                    warn!("      ⚠️  BRC-29 output {} has no custom_instructions", output_id);
                }
            }
            (None, _) => {
                // Basket output without address (e.g., token scripts)
                // No derivation info needed - these use custom logic
                no_address_count += 1;
            }
            _ => {
                // Unexpected case
                warn!("      ⚠️  Unexpected address/index for output {}: address_id={:?}, index={:?}",
                      output_id, address_id, index);
            }
        }
    }

    info!("      ✅ Set derivation for {} HD addresses", hd_count);
    if master_count > 0 {
        info!("      ✅ {} master pubkey outputs (no derivation)", master_count);
    }
    if brc29_count > 0 {
        info!("      ✅ Parsed derivation for {} BRC-29 outputs", brc29_count);
    }
    if no_address_count > 0 {
        info!("      ✅ {} basket outputs (no address derivation)", no_address_count);
    }

    Ok(())
}

/// Parse BRC-29 custom_instructions JSON to extract derivation params
///
/// Expected format:
/// {
///   "senderPublicKey": "02abc...",
///   "invoiceNumber": "2-some protocol-keyid"
/// }
///
/// Returns (derivation_prefix, derivation_suffix) parsed from invoiceNumber
fn parse_brc29_derivation(instructions: &str) -> Option<(String, String)> {
    // Parse JSON
    let json: serde_json::Value = serde_json::from_str(instructions).ok()?;

    // Get invoice number
    let invoice = json.get("invoiceNumber")?.as_str()?;

    // Parse BRC-43 invoice format: "{securityLevel}-{protocolId}-{keyId}"
    // e.g., "2-payment protocol-abc123"
    let parts: Vec<&str> = invoice.splitn(3, '-').collect();
    if parts.len() >= 3 {
        // prefix = "{securityLevel}-{protocolId}", suffix = "{keyId}"
        let prefix = format!("{}-{}", parts[0], parts[1]);
        let suffix = parts[2].to_string();
        Some((prefix, suffix))
    } else if parts.len() == 2 {
        // Handle edge case: "{securityLevel}-{protocolId}" with no keyId
        let prefix = format!("{}-{}", parts[0], parts[1]);
        Some((prefix, String::new()))
    } else {
        None
    }
}

/// Migration v19: Labels, Commissions, Supporting Tables (Phase 5)
///
/// Phase 5 of wallet-toolbox alignment. Restructures transaction labels to
/// normalized form (tx_labels + tx_labels_map) and adds supporting tables:
/// - tx_labels: Deduplicated label entities per user
/// - tx_labels_map: Many-to-many mapping between labels and transactions
/// - commissions: Fee tracking per transaction
/// - settings: Persistent wallet configuration
/// - sync_states: Multi-device synchronization state
///
/// Data migration: Existing transaction_labels data is migrated to the new tables.
pub fn create_schema_v19(conn: &Connection) -> Result<()> {
    info!("=== Migration v19: Labels, Commissions, Supporting Tables (Phase 5) ===");

    // Step 1: Create tx_labels table (label entities)
    info!("   Step 1: Creating tx_labels table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tx_labels (
            txLabelId INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL REFERENCES users(userId),
            label TEXT NOT NULL,
            is_deleted INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(label, user_id)
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tx_labels_user_id ON tx_labels(user_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tx_labels_label ON tx_labels(label)",
        [],
    )?;
    info!("   ✅ Created tx_labels table");

    // Step 2: Create tx_labels_map junction table
    info!("   Step 2: Creating tx_labels_map table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tx_labels_map (
            txLabelId INTEGER NOT NULL REFERENCES tx_labels(txLabelId),
            transaction_id INTEGER NOT NULL REFERENCES transactions(id),
            is_deleted INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(txLabelId, transaction_id)
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tx_labels_map_tx ON tx_labels_map(transaction_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tx_labels_map_label ON tx_labels_map(txLabelId)",
        [],
    )?;
    info!("   ✅ Created tx_labels_map table");

    // Step 3: Migrate existing transaction_labels data
    info!("   Step 3: Migrating transaction_labels data...");
    migrate_transaction_labels(conn)?;

    // Step 4: Create commissions table
    info!("   Step 4: Creating commissions table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS commissions (
            commissionId INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL REFERENCES users(userId),
            transaction_id INTEGER NOT NULL UNIQUE REFERENCES transactions(id),
            satoshis INTEGER NOT NULL,
            key_offset TEXT NOT NULL,
            is_redeemed INTEGER NOT NULL DEFAULT 0,
            locking_script BLOB NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_commissions_user_id ON commissions(user_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_commissions_tx ON commissions(transaction_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_commissions_unredeemed ON commissions(is_redeemed) WHERE is_redeemed = 0",
        [],
    )?;
    info!("   ✅ Created commissions table");

    // Step 5: Create settings table
    info!("   Step 5: Creating settings table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            storage_identity_key TEXT NOT NULL,
            storage_name TEXT NOT NULL,
            chain TEXT NOT NULL DEFAULT 'main',
            dbtype TEXT NOT NULL DEFAULT 'sqlite',
            max_output_script INTEGER NOT NULL DEFAULT 500000,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;
    info!("   ✅ Created settings table");

    // Step 6: Create sync_states table
    info!("   Step 6: Creating sync_states table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_states (
            syncStateId INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL REFERENCES users(userId),
            storage_identity_key TEXT NOT NULL DEFAULT '',
            storage_name TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'unknown',
            init INTEGER NOT NULL DEFAULT 0,
            ref_num TEXT NOT NULL UNIQUE,
            sync_map TEXT NOT NULL,
            sync_when INTEGER,
            satoshis INTEGER,
            error_local TEXT,
            error_other TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sync_states_status ON sync_states(status)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sync_states_user ON sync_states(user_id)",
        [],
    )?;
    info!("   ✅ Created sync_states table");

    info!("   ✅ Schema version 19 migration complete");
    Ok(())
}

/// Create schema version 20 (Monitor Pattern - Phase 6A)
///
/// Creates the monitor_events table for background task event logging.
pub fn create_schema_v20(conn: &Connection) -> Result<()> {
    info!("=== Migration v20: Monitor Events Table (Phase 6A) ===");

    // Step 1: Create monitor_events table for task event logging
    info!("   Step 1: Creating monitor_events table...");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS monitor_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event TEXT NOT NULL,
            details TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_monitor_events_event ON monitor_events(event)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_monitor_events_created ON monitor_events(created_at)",
        [],
    )?;
    info!("   ✅ Created monitor_events table with indexes");

    info!("   ✅ Schema version 20 migration complete");
    Ok(())
}

/// Migration v21: Repair proven_txs merkle_path BLOBs
///
/// Old code paths (cache_sync, some WoC fetches) stored TSC JSON in the
/// merkle_path BLOB without the "height" field. The standard TSC format
/// (BRC-61) doesn't include height, but our BEEF builder requires it for
/// BUMP construction. The height IS correctly stored in the proven_txs.height
/// column — this migration injects it into the BLOB JSON where missing.
pub fn create_schema_v21(conn: &Connection) -> Result<()> {
    info!("=== Migration v21: Repair proven_txs merkle_path height ===");

    // Read all proven_txs rows
    let mut stmt = conn.prepare(
        "SELECT provenTxId, height, merkle_path FROM proven_txs"
    )?;

    let rows: Vec<(i64, i64, Vec<u8>)> = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Vec<u8>>(2)?,
        ))
    })?.filter_map(|r| r.ok()).collect();

    let mut patched = 0;
    for (id, height, blob) in &rows {
        let tsc: serde_json::Value = match serde_json::from_slice(blob) {
            Ok(v) => v,
            Err(_) => continue, // skip corrupt BLOBs
        };

        // Check if height is missing or not a valid number
        if tsc.get("height").and_then(|h| h.as_u64()).is_some() {
            continue; // already has valid height
        }

        // Inject height from the column
        let mut patched_tsc = tsc.clone();
        if let Some(obj) = patched_tsc.as_object_mut() {
            obj.insert("height".to_string(), serde_json::json!(*height as u64));
        }

        let new_blob = match serde_json::to_vec(&patched_tsc) {
            Ok(b) => b,
            Err(_) => continue,
        };

        match conn.execute(
            "UPDATE proven_txs SET merkle_path = ?1 WHERE provenTxId = ?2",
            rusqlite::params![new_blob, id],
        ) {
            Ok(_) => patched += 1,
            Err(e) => warn!("   ⚠️  Failed to patch proven_tx {}: {}", id, e),
        }
    }

    if patched > 0 {
        info!("   ✅ Patched {} proven_txs records with missing height", patched);
    } else {
        info!("   ✅ All proven_txs records already have height in BLOB");
    }

    info!("   ✅ Schema version 21 migration complete");
    Ok(())
}

pub fn create_schema_v22(conn: &Connection) -> Result<()> {
    info!("=== Migration v22: Fix array-format proven_txs merkle_path BLOBs ===");

    let mut stmt = conn.prepare(
        "SELECT provenTxId, height, merkle_path FROM proven_txs"
    )?;

    let rows: Vec<(i64, i64, Vec<u8>)> = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Vec<u8>>(2)?,
        ))
    })?.filter_map(|r| r.ok()).collect();

    let mut patched = 0;
    for (id, height, blob) in &rows {
        let tsc: serde_json::Value = match serde_json::from_slice(blob) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Normalize array to object (WoC returns [{...}] sometimes)
        let mut obj_tsc = if tsc.is_array() {
            match tsc.as_array().and_then(|a| a.first()).cloned() {
                Some(first) if first.is_object() => first,
                _ => continue,
            }
        } else if tsc.is_object() {
            tsc.clone()
        } else {
            continue;
        };

        let was_array = tsc.is_array();

        // Inject height if missing
        let needs_height = obj_tsc.get("height").and_then(|h| h.as_u64()).is_none();
        if needs_height {
            if let Some(obj) = obj_tsc.as_object_mut() {
                obj.insert("height".to_string(), serde_json::json!(*height as u64));
            }
        }

        if !was_array && !needs_height {
            continue; // Already correct
        }

        let new_blob = match serde_json::to_vec(&obj_tsc) {
            Ok(b) => b,
            Err(_) => continue,
        };

        match conn.execute(
            "UPDATE proven_txs SET merkle_path = ?1 WHERE provenTxId = ?2",
            rusqlite::params![new_blob, id],
        ) {
            Ok(_) => {
                if was_array {
                    info!("   ✅ Fixed array-format BLOB for proven_tx {} (height={})", id, height);
                } else {
                    info!("   ✅ Injected height for proven_tx {} (height={})", id, height);
                }
                patched += 1;
            }
            Err(e) => warn!("   ⚠️  Failed to patch proven_tx {}: {}", id, e),
        }
    }

    if patched > 0 {
        info!("   ✅ Fixed {} proven_txs records (array format and/or missing height)", patched);
    } else {
        info!("   ✅ All proven_txs records already correct");
    }

    info!("   ✅ Schema version 22 migration complete");
    Ok(())
}

/// Phase 7A: Re-tag outputs with proper derivation_prefix/derivation_suffix
///
/// Classifies outputs that have NULL derivation fields by matching their
/// locking scripts against the addresses table. Tags them as:
/// - "2-receive address" + "{index}" for BRC-42 self-derived addresses
/// - "bip32" + "{index}" for legacy BIP32 addresses
/// - NULL/NULL for master key outputs (index -1) — left unchanged
///
/// For outputs that don't match any known address (e.g. PushDrop tokens,
/// certificate outputs with custom scripts), derivation remains NULL —
/// these use custom_instructions for signing, not derivation fields.
pub fn create_schema_v23(conn: &Connection) -> Result<()> {
    use sha2::{Sha256, Digest};
    use ripemd::Ripemd160;

    info!("=== Migration v23: Phase 7A — Re-tag output derivation fields ===");

    // Step 1: Get all outputs with NULL derivation_prefix
    let mut stmt = conn.prepare(
        "SELECT outputId, locking_script, txid, vout, satoshis, spendable
         FROM outputs WHERE derivation_prefix IS NULL"
    )?;
    let null_outputs: Vec<(i64, Option<Vec<u8>>, Option<String>, i32, i64, bool)> = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<Vec<u8>>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i32>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, bool>(5)?,
        ))
    })?.filter_map(|r| r.ok()).collect();

    info!("   Found {} outputs with NULL derivation_prefix", null_outputs.len());
    if null_outputs.is_empty() {
        info!("   ✅ No outputs need re-tagging");
        info!("   ✅ Schema version 23 migration complete");
        return Ok(());
    }

    // Step 2: Build a lookup map of pubkey_hash → (address_index, address_string)
    // from the addresses table
    let mut addr_stmt = conn.prepare(
        "SELECT \"index\", address, public_key FROM addresses"
    )?;
    let addresses: Vec<(i32, String, String)> = addr_stmt.query_map([], |row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?.filter_map(|r| r.ok()).collect();
    info!("   Loaded {} addresses for matching", addresses.len());

    // Build pubkey_hash → index lookup
    // P2PKH locking script format: OP_DUP OP_HASH160 <20-byte hash> OP_EQUALVERIFY OP_CHECKSIG
    // Bytes: 76 a9 14 <20 bytes> 88 ac
    let mut hash_to_index: std::collections::HashMap<Vec<u8>, i32> = std::collections::HashMap::new();
    for (index, _address, pubkey_hex) in &addresses {
        if let Ok(pubkey_bytes) = hex::decode(pubkey_hex) {
            let sha_hash = Sha256::digest(&pubkey_bytes);
            let pubkey_hash = Ripemd160::digest(&sha_hash);
            hash_to_index.insert(pubkey_hash.to_vec(), *index);
        }
    }

    // Step 3: Get master keys for BIP32 vs BRC-42 verification
    let has_wallet: bool = conn.query_row(
        "SELECT COUNT(*) FROM wallets", [], |row| Ok(row.get::<_, i64>(0)? > 0)
    ).unwrap_or(false);

    // We need master keys to verify BIP32 vs BRC-42 for each matched address
    let master_keys: Option<(Vec<u8>, Vec<u8>)> = if has_wallet {
        match conn.query_row(
            "SELECT mnemonic FROM wallets ORDER BY id ASC LIMIT 1", [], |row| row.get::<_, String>(0)
        ) {
            Ok(mnemonic_str) => {
                match bip39::Mnemonic::parse_in(bip39::Language::English, &mnemonic_str) {
                    Ok(mnemonic) => {
                        let seed = mnemonic.to_seed("");
                        match bip32::XPrv::new(&seed) {
                            Ok(master_key) => {
                                let privkey = master_key.private_key().to_bytes().to_vec();
                                let secp = secp256k1::Secp256k1::new();
                                match secp256k1::SecretKey::from_slice(&privkey) {
                                    Ok(sk) => {
                                        let pubkey = secp256k1::PublicKey::from_secret_key(&secp, &sk).serialize().to_vec();
                                        Some((privkey, pubkey))
                                    }
                                    Err(_) => None,
                                }
                            }
                            Err(_) => None,
                        }
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // Step 4: Classify and tag each NULL-derivation output
    let mut master_count = 0u32;
    let mut brc42_count = 0u32;
    let mut bip32_count = 0u32;
    let mut no_match_count = 0u32;
    let mut no_script_count = 0u32;

    for (output_id, locking_script, txid, vout, satoshis, spendable) in &null_outputs {
        let script = match locking_script {
            Some(s) if s.len() >= 25 => s,
            _ => {
                // No locking script or too short for P2PKH — skip
                no_script_count += 1;
                log::debug!("   Output {} ({}:{}) — no/short locking script, skipping", output_id, txid.as_deref().unwrap_or("?"), vout);
                continue;
            }
        };

        // Extract pubkey hash from P2PKH script (bytes 3..23)
        // Expected format: 76 a9 14 <20 bytes> 88 ac
        if script.len() < 25 || script[0] != 0x76 || script[1] != 0xa9 || script[2] != 0x14 {
            // Not a standard P2PKH script (could be PushDrop, multisig, etc.)
            no_match_count += 1;
            log::info!("   Output {} ({}:{}, {} sats, spendable={}) — non-P2PKH script, derivation stays NULL",
                output_id, txid.as_deref().unwrap_or("?"), vout, satoshis, spendable);
            continue;
        }

        let pubkey_hash = &script[3..23];

        // Look up in address index map
        match hash_to_index.get(pubkey_hash) {
            Some(&addr_index) if addr_index == -1 => {
                // Master key output — leave NULL (correct behavior)
                master_count += 1;
                log::info!("   Output {} ({}:{}, {} sats) — master key (index -1), derivation stays NULL",
                    output_id, txid.as_deref().unwrap_or("?"), vout, satoshis);
            }
            Some(&addr_index) if addr_index >= 0 => {
                // Matched a positive-index address. Determine BIP32 vs BRC-42.
                let (prefix, suffix) = determine_derivation_method(
                    &master_keys, addr_index as u32, pubkey_hash,
                );

                conn.execute(
                    "UPDATE outputs SET derivation_prefix = ?1, derivation_suffix = ?2 WHERE outputId = ?3",
                    rusqlite::params![prefix, suffix, output_id],
                )?;

                if prefix == "bip32" {
                    bip32_count += 1;
                    log::info!("   Output {} ({}:{}, {} sats) — tagged BIP32 index {}",
                        output_id, txid.as_deref().unwrap_or("?"), vout, satoshis, addr_index);
                } else {
                    brc42_count += 1;
                    log::info!("   Output {} ({}:{}, {} sats) — tagged BRC-42 index {}",
                        output_id, txid.as_deref().unwrap_or("?"), vout, satoshis, addr_index);
                }
            }
            Some(&addr_index) => {
                // Negative index other than -1 (BRC-29) — these should use custom_instructions
                no_match_count += 1;
                log::info!("   Output {} ({}:{}, {} sats) — BRC-29 address (index {}), derivation stays NULL (uses custom_instructions)",
                    output_id, txid.as_deref().unwrap_or("?"), vout, satoshis, addr_index);
            }
            None => {
                // No address match — script doesn't correspond to any known address
                no_match_count += 1;
                log::info!("   Output {} ({}:{}, {} sats, spendable={}) — no address match, derivation stays NULL",
                    output_id, txid.as_deref().unwrap_or("?"), vout, satoshis, spendable);
            }
        }
    }

    info!("   === Migration v23 Summary ===");
    info!("   Total NULL-derivation outputs: {}", null_outputs.len());
    info!("   Tagged BRC-42 self-derived:    {}", brc42_count);
    info!("   Tagged BIP32 legacy:           {}", bip32_count);
    info!("   Master key (stays NULL):       {}", master_count);
    info!("   Non-P2PKH/no match (stays NULL): {}", no_match_count);
    info!("   No/short script (skipped):     {}", no_script_count);

    // Step 5: Verification — count remaining NULL derivation on spendable P2PKH outputs
    let remaining_null_spendable: i64 = conn.query_row(
        "SELECT COUNT(*) FROM outputs
         WHERE derivation_prefix IS NULL
         AND spendable = 1
         AND LENGTH(locking_script) >= 25",
        [],
        |row| row.get(0),
    )?;

    if remaining_null_spendable > 0 {
        warn!("   ⚠️  {} spendable outputs still have NULL derivation — these are master key or custom script outputs", remaining_null_spendable);
    } else {
        info!("   ✅ All spendable P2PKH outputs now have derivation fields populated");
    }

    info!("   ✅ Schema version 23 migration complete");
    Ok(())
}

/// Determine whether a positive-index address was derived via BIP32 or BRC-42
///
/// Tries BRC-42 self-derivation for the given index. If the resulting pubkey hash
/// matches the actual locking script hash, it's BRC-42. If not, it must be BIP32.
///
/// This avoids needing the BIP32 chain code (which requires the seed, not just
/// the master private key).
fn determine_derivation_method(
    master_keys: &Option<(Vec<u8>, Vec<u8>)>,
    index: u32,
    actual_pubkey_hash: &[u8],
) -> (String, String) {
    use sha2::{Sha256, Digest};
    use ripemd::Ripemd160;
    use crate::crypto::brc42::derive_child_public_key;

    let suffix = index.to_string();

    let (privkey, pubkey) = match master_keys {
        Some((priv_k, pub_k)) => (priv_k, pub_k),
        None => {
            // Can't verify — default to BRC-42 (current standard)
            return ("2-receive address".to_string(), suffix);
        }
    };

    // Try BRC-42 self-derivation: invoice = "2-receive address-{index}"
    // If the resulting pubkey hash matches → it's BRC-42
    // If not → must be BIP32
    let brc42_invoice = format!("2-receive address-{}", index);
    if let Ok(brc42_pubkey) = derive_child_public_key(privkey, pubkey, &brc42_invoice) {
        let sha_hash = Sha256::digest(&brc42_pubkey);
        let brc42_hash = Ripemd160::digest(&sha_hash);

        if brc42_hash.as_slice() == actual_pubkey_hash {
            return ("2-receive address".to_string(), suffix);
        }
    }

    // BRC-42 doesn't match — must be BIP32
    ("bip32".to_string(), suffix)
}

/// Migrate data from old transaction_labels table to new tx_labels + tx_labels_map
///
/// The old table has (transaction_id, label) pairs. The new model separates
/// labels into a deduplicated entity table with a junction table for mappings.
fn migrate_transaction_labels(conn: &Connection) -> Result<()> {
    // Check if transaction_labels table exists
    let has_old_table: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='transaction_labels'",
        [],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    ).unwrap_or(false);

    if !has_old_table {
        info!("      ⏭️  No transaction_labels table found, skipping migration");
        return Ok(());
    }

    // Get default user ID
    let default_user_id: i64 = conn.query_row(
        "SELECT userId FROM users ORDER BY userId ASC LIMIT 1",
        [],
        |row| row.get(0),
    ).unwrap_or(1);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Step 1: Insert unique labels into tx_labels (deduplicate)
    let labels_inserted = conn.execute(
        "INSERT OR IGNORE INTO tx_labels (user_id, label, is_deleted, created_at, updated_at)
         SELECT DISTINCT ?1, LOWER(TRIM(label)), 0, ?2, ?3
         FROM transaction_labels
         WHERE TRIM(label) != ''",
        rusqlite::params![default_user_id, now, now],
    )?;
    info!("      ✅ Inserted {} unique labels into tx_labels", labels_inserted);

    // Step 2: Create mappings in tx_labels_map
    let mappings_inserted = conn.execute(
        "INSERT OR IGNORE INTO tx_labels_map (txLabelId, transaction_id, is_deleted, created_at, updated_at)
         SELECT tl.txLabelId, old.transaction_id, 0, ?1, ?2
         FROM transaction_labels old
         INNER JOIN tx_labels tl ON tl.label = LOWER(TRIM(old.label)) AND tl.user_id = ?3
         WHERE TRIM(old.label) != ''",
        rusqlite::params![now, now, default_user_id],
    )?;
    info!("      ✅ Created {} label mappings in tx_labels_map", mappings_inserted);

    // Verify migration
    let old_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transaction_labels",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let new_map_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tx_labels_map",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if old_count == new_map_count {
        info!("      ✅ Migration verified: {} mappings", new_map_count);
    } else {
        // Some may have been skipped due to empty labels
        let empty_labels: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transaction_labels WHERE TRIM(label) = ''",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        info!("      ℹ️  {} original labels, {} migrated, {} empty labels skipped",
              old_count, new_map_count, empty_labels);
    }

    Ok(())
}

/// Migration V24: Phase 8C — Schema Cleanup
///
/// 1. Drop `merkle_proofs` table (replaced by `proven_txs` in V16)
/// 2. Drop `domain_whitelist` DB table (JSON file used instead)
/// 3. Drop `transaction_labels` table (replaced by `tx_labels`/`tx_labels_map` in V19, data migrated)
/// 4. Recreate `output_tag_map` with correct FK (was `utxos(id)`, now `outputs(outputId)`)
/// 5. Clean up orphaned nosend transactions older than 48 hours
pub fn create_schema_v24(conn: &Connection) -> Result<()> {
    info!("=== Migration V24: Phase 8C — Schema Cleanup ===");

    // Step 1: Drop merkle_proofs (replaced by proven_txs in V16)
    info!("   Step 1: Dropping merkle_proofs table...");
    conn.execute("DROP TABLE IF EXISTS merkle_proofs", [])?;
    info!("   ✅ Dropped merkle_proofs table");

    // Step 2: Drop domain_whitelist DB table (JSON file used instead)
    info!("   Step 2: Dropping domain_whitelist DB table...");
    conn.execute("DROP TABLE IF EXISTS domain_whitelist", [])?;
    info!("   ✅ Dropped domain_whitelist table");

    // Step 3: Drop transaction_labels (replaced by tx_labels/tx_labels_map, data migrated in V19)
    info!("   Step 3: Dropping transaction_labels table...");
    conn.execute("DROP TABLE IF EXISTS transaction_labels", [])?;
    info!("   ✅ Dropped transaction_labels table");

    // Step 4: Recreate output_tag_map with correct FK
    // The old table had FOREIGN KEY (output_id) REFERENCES utxos(id) — wrong table.
    // The new table references outputs(outputId) with ON DELETE CASCADE.
    info!("   Step 4: Rebuilding output_tag_map with correct FK...");

    let old_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM output_tag_map",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    // 4a. Create new table with correct FK
    conn.execute(
        "CREATE TABLE IF NOT EXISTS output_tag_map_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            output_id INTEGER NOT NULL,
            output_tag_id INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            is_deleted INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (output_id) REFERENCES outputs(outputId) ON DELETE CASCADE,
            FOREIGN KEY (output_tag_id) REFERENCES output_tags(id) ON DELETE CASCADE,
            UNIQUE(output_id, output_tag_id)
        )",
        [],
    )?;

    // 4b. Copy only valid rows (output_id must exist in outputs table)
    let copied = conn.execute(
        "INSERT INTO output_tag_map_new (id, output_id, output_tag_id, created_at, updated_at, is_deleted)
         SELECT otm.id, otm.output_id, otm.output_tag_id, otm.created_at, otm.updated_at, otm.is_deleted
         FROM output_tag_map otm
         WHERE EXISTS (SELECT 1 FROM outputs WHERE outputId = otm.output_id)",
        [],
    )?;

    let orphaned = old_count - copied as i64;
    if orphaned > 0 {
        info!("   ℹ️  Pruned {} orphaned output_tag_map rows (referenced deleted outputs)", orphaned);
    }

    // 4c. Drop old, rename new
    conn.execute("DROP TABLE output_tag_map", [])?;
    conn.execute("ALTER TABLE output_tag_map_new RENAME TO output_tag_map", [])?;

    // 4d. Recreate indexes
    conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tag_map_output_id ON output_tag_map(output_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tag_map_tag_id ON output_tag_map(output_tag_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_output_tag_map_deleted ON output_tag_map(is_deleted) WHERE is_deleted = 0", [])?;

    info!("   ✅ Rebuilt output_tag_map with FK → outputs(outputId) ({} rows kept)", copied);

    // Step 5: Clean up orphaned nosend transactions
    // These are signed-but-never-broadcast transactions older than 48 hours.
    // Their basket outputs may still show up as spendable in listOutputs.
    info!("   Step 5: Cleaning up orphaned nosend transactions...");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let cutoff = now - (48 * 3600); // 48 hours ago

    // Find old nosend transaction IDs
    let mut nosend_stmt = conn.prepare(
        "SELECT id, txid FROM transactions WHERE new_status = 'nosend' AND timestamp < ?1"
    )?;
    let nosend_txs: Vec<(i64, String)> = nosend_stmt.query_map(
        rusqlite::params![cutoff],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?.filter_map(|r| r.ok()).collect();

    if nosend_txs.is_empty() {
        info!("   ✅ No orphaned nosend transactions to clean up");
    } else {
        let mut ghost_outputs_deleted = 0usize;

        for (tx_id, txid) in &nosend_txs {
            // Delete ghost outputs created by this nosend transaction
            let deleted = conn.execute(
                "DELETE FROM outputs WHERE transaction_id = ?1 AND spendable = 1",
                rusqlite::params![tx_id],
            )?;
            ghost_outputs_deleted += deleted;

            // Restore any inputs that were marked as spent by this transaction
            conn.execute(
                "UPDATE outputs SET spendable = 1, spent_by = NULL, spending_description = NULL, updated_at = ?1
                 WHERE spent_by = ?2 AND spendable = 0",
                rusqlite::params![now, tx_id],
            )?;

            // Mark the transaction as failed
            conn.execute(
                "UPDATE transactions SET new_status = 'failed', status = 'failed', failed_at = ?1 WHERE id = ?2",
                rusqlite::params![now, tx_id],
            )?;

            let short = &txid[..std::cmp::min(16, txid.len())];
            info!("   🗑️  Cleaned up nosend tx {} (deleted {} ghost outputs)", short, deleted);
        }

        info!("   ✅ Cleaned up {} nosend transactions, deleted {} ghost outputs",
              nosend_txs.len(), ghost_outputs_deleted);
    }

    info!("=== Migration V24 complete ===");
    Ok(())
}
