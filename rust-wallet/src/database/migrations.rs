//! Database schema migrations
//!
//! Handles creation and updates of database schema.
//! Each migration function creates a specific version of the schema.

use rusqlite::{Connection, Result};
use log::{info, error};

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
