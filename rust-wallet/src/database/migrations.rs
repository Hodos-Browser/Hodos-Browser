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
            certificate_txid TEXT NOT NULL UNIQUE,
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
