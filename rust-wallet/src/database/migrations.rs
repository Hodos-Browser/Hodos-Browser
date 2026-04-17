//! Database schema migrations
//!
//! Single clean V1 migration creating the final target schema.
//! All 24 incremental migrations have been consolidated.

use rusqlite::{Connection, Result};
use log::info;

/// Create schema version 1 — consolidated final schema
///
/// Creates all tables, indexes, and constraints for the wallet database.
/// This replaces the previous 24 incremental migrations.
pub fn create_schema_v1(conn: &Connection) -> Result<()> {
    info!("   Creating consolidated schema V1...");

    conn.execute_batch("
        -- =====================================================================
        -- Browser-specific tables
        -- =====================================================================

        CREATE TABLE IF NOT EXISTS wallets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            mnemonic TEXT NOT NULL,
            pin_salt TEXT,
            mnemonic_dpapi BLOB,
            current_index INTEGER NOT NULL DEFAULT 0,
            backed_up INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS addresses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            wallet_id INTEGER NOT NULL,
            \"index\" INTEGER NOT NULL,
            address TEXT NOT NULL UNIQUE,
            public_key TEXT NOT NULL,
            used INTEGER NOT NULL DEFAULT 0,
            balance INTEGER NOT NULL DEFAULT 0,
            pending_utxo_check INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (wallet_id) REFERENCES wallets(id) ON DELETE CASCADE,
            UNIQUE(wallet_id, \"index\")
        );

        CREATE TABLE IF NOT EXISTS parent_transactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            utxo_id INTEGER,
            txid TEXT NOT NULL UNIQUE,
            raw_hex TEXT NOT NULL,
            cached_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS block_headers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            block_hash TEXT NOT NULL UNIQUE,
            height INTEGER NOT NULL UNIQUE,
            header_hex TEXT NOT NULL,
            cached_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS transaction_inputs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            transaction_id INTEGER NOT NULL,
            txid TEXT NOT NULL,
            vout INTEGER NOT NULL,
            satoshis INTEGER NOT NULL,
            script TEXT,
            FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS transaction_outputs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            transaction_id INTEGER NOT NULL,
            vout INTEGER NOT NULL,
            satoshis INTEGER NOT NULL,
            script TEXT,
            address TEXT,
            FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE,
            UNIQUE(transaction_id, vout)
        );

        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_box TEXT,
            sender TEXT,
            recipient TEXT,
            body TEXT,
            received_at INTEGER,
            acknowledged INTEGER NOT NULL DEFAULT 0,
            acknowledged_at INTEGER
        );

        CREATE TABLE IF NOT EXISTS relay_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recipient TEXT,
            message_box TEXT,
            sender TEXT,
            body TEXT,
            created_at INTEGER DEFAULT (strftime('%s','now')),
            expires_at INTEGER
        );

        -- =====================================================================
        -- Toolbox-aligned tables
        -- =====================================================================

        CREATE TABLE IF NOT EXISTS users (
            userId INTEGER PRIMARY KEY AUTOINCREMENT,
            identity_key TEXT UNIQUE NOT NULL,
            active_storage TEXT NOT NULL DEFAULT 'local',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS proven_txs (
            provenTxId INTEGER PRIMARY KEY AUTOINCREMENT,
            txid TEXT UNIQUE NOT NULL,
            height INTEGER NOT NULL,
            tx_index INTEGER NOT NULL,
            merkle_path BLOB NOT NULL,
            raw_tx BLOB NOT NULL,
            block_hash TEXT NOT NULL DEFAULT '',
            merkle_root TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS proven_tx_reqs (
            provenTxReqId INTEGER PRIMARY KEY AUTOINCREMENT,
            proven_tx_id INTEGER,
            txid TEXT UNIQUE NOT NULL,
            status TEXT NOT NULL DEFAULT 'unknown',
            attempts INTEGER NOT NULL DEFAULT 0,
            notified INTEGER NOT NULL DEFAULT 0,
            batch TEXT,
            history TEXT NOT NULL DEFAULT '{}',
            notify TEXT NOT NULL DEFAULT '{}',
            raw_tx BLOB NOT NULL,
            input_beef BLOB,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (proven_tx_id) REFERENCES proven_txs(provenTxId)
        );

        CREATE TABLE IF NOT EXISTS transactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER,
            proven_tx_id INTEGER,
            txid TEXT UNIQUE,
            reference_number TEXT UNIQUE NOT NULL,
            raw_tx TEXT,
            description TEXT,
            status TEXT NOT NULL,
            is_outgoing INTEGER NOT NULL,
            satoshis INTEGER NOT NULL,
            input_beef BLOB,
            version INTEGER NOT NULL DEFAULT 1,
            lock_time INTEGER NOT NULL DEFAULT 0,
            block_height INTEGER,
            confirmations INTEGER NOT NULL DEFAULT 0,
            failed_at INTEGER,
            price_usd_cents INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(userId),
            FOREIGN KEY (proven_tx_id) REFERENCES proven_txs(provenTxId)
        );

        CREATE TABLE IF NOT EXISTS certificates (
            certificateId INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            type TEXT NOT NULL,
            serial_number TEXT NOT NULL,
            certifier TEXT NOT NULL,
            subject TEXT NOT NULL,
            verifier TEXT,
            revocation_outpoint TEXT NOT NULL,
            signature TEXT NOT NULL,
            is_deleted INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(userId),
            UNIQUE(user_id, type, certifier, serial_number)
        );

        CREATE TABLE IF NOT EXISTS certificate_fields (
            certificateId INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            field_name TEXT NOT NULL,
            field_value TEXT NOT NULL,
            master_key TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (certificateId) REFERENCES certificates(certificateId) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES users(userId),
            UNIQUE(field_name, certificateId)
        );

        CREATE TABLE IF NOT EXISTS output_baskets (
            basketId INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER,
            name TEXT NOT NULL,
            number_of_desired_utxos INTEGER NOT NULL DEFAULT 6,
            minimum_desired_utxo_value INTEGER NOT NULL DEFAULT 10000,
            is_deleted INTEGER NOT NULL DEFAULT 0,
            description TEXT,
            token_type TEXT,
            protocol_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(userId),
            UNIQUE(name, user_id)
        );

        CREATE TABLE IF NOT EXISTS outputs (
            outputId INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            transaction_id INTEGER, -- FK to transactions.id (internal row ID, NOT the on-chain txid). NULL for externally-received outputs (address sync, PeerPay).
            basket_id INTEGER,
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
            spent_by INTEGER,
            sequence_number INTEGER,
            spending_description TEXT,
            script_length INTEGER,
            script_offset INTEGER,
            locking_script BLOB,
            confirmed INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(userId),
            FOREIGN KEY (transaction_id) REFERENCES transactions(id),
            FOREIGN KEY (basket_id) REFERENCES output_baskets(basketId),
            FOREIGN KEY (spent_by) REFERENCES transactions(id),
            UNIQUE(txid, vout)
        );
        CREATE INDEX IF NOT EXISTS idx_outputs_confirmed ON outputs(confirmed);

        CREATE TABLE IF NOT EXISTS output_tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER,
            tag TEXT NOT NULL,
            is_deleted INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(userId),
            UNIQUE(tag, user_id)
        );

        CREATE TABLE IF NOT EXISTS output_tag_map (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            output_id INTEGER NOT NULL,
            output_tag_id INTEGER NOT NULL,
            is_deleted INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (output_id) REFERENCES outputs(outputId) ON DELETE CASCADE,
            FOREIGN KEY (output_tag_id) REFERENCES output_tags(id) ON DELETE CASCADE,
            UNIQUE(output_id, output_tag_id)
        );

        CREATE TABLE IF NOT EXISTS tx_labels (
            txLabelId INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            label TEXT NOT NULL,
            is_deleted INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(userId),
            UNIQUE(label, user_id)
        );

        CREATE TABLE IF NOT EXISTS tx_labels_map (
            txLabelId INTEGER NOT NULL,
            transaction_id INTEGER NOT NULL,
            is_deleted INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (txLabelId) REFERENCES tx_labels(txLabelId),
            FOREIGN KEY (transaction_id) REFERENCES transactions(id),
            UNIQUE(txLabelId, transaction_id)
        );

        CREATE TABLE IF NOT EXISTS commissions (
            commissionId INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            transaction_id INTEGER UNIQUE NOT NULL,
            satoshis INTEGER NOT NULL,
            key_offset TEXT NOT NULL,
            is_redeemed INTEGER NOT NULL DEFAULT 0,
            locking_script BLOB NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(userId),
            FOREIGN KEY (transaction_id) REFERENCES transactions(id)
        );

        CREATE TABLE IF NOT EXISTS monitor_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event TEXT NOT NULL,
            details TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_states (
            syncStateId INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            storage_identity_key TEXT NOT NULL DEFAULT '',
            storage_name TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'unknown',
            init INTEGER NOT NULL DEFAULT 0,
            ref_num TEXT UNIQUE NOT NULL,
            sync_map TEXT NOT NULL,
            sync_when INTEGER,
            satoshis INTEGER,
            error_local TEXT,
            error_other TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(userId)
        );

        CREATE TABLE IF NOT EXISTS settings (
            storage_identity_key TEXT NOT NULL,
            storage_name TEXT NOT NULL,
            chain TEXT NOT NULL DEFAULT 'main',
            dbtype TEXT NOT NULL DEFAULT 'sqlite',
            max_output_script INTEGER NOT NULL DEFAULT 500000,
            sender_display_name TEXT NOT NULL DEFAULT 'Anonymous',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS derived_key_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            derived_pubkey TEXT NOT NULL UNIQUE,
            invoice TEXT NOT NULL,
            counterparty_pubkey TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        -- =====================================================================
        -- Indexes
        -- =====================================================================

        -- wallets
        CREATE INDEX IF NOT EXISTS idx_wallets_id ON wallets(id);

        -- addresses
        CREATE INDEX IF NOT EXISTS idx_addresses_wallet_id ON addresses(wallet_id);
        CREATE INDEX IF NOT EXISTS idx_addresses_address ON addresses(address);
        CREATE INDEX IF NOT EXISTS idx_addresses_index ON addresses(wallet_id, \"index\");
        CREATE INDEX IF NOT EXISTS idx_addresses_pending_utxo_check ON addresses(pending_utxo_check) WHERE pending_utxo_check = 1;

        -- parent_transactions
        CREATE INDEX IF NOT EXISTS idx_parent_txns_txid ON parent_transactions(txid);

        -- block_headers
        CREATE INDEX IF NOT EXISTS idx_block_headers_hash ON block_headers(block_hash);
        CREATE INDEX IF NOT EXISTS idx_block_headers_height ON block_headers(height);

        -- transaction_inputs
        CREATE INDEX IF NOT EXISTS idx_tx_inputs_tx_id ON transaction_inputs(transaction_id);
        CREATE INDEX IF NOT EXISTS idx_tx_inputs_prev_tx ON transaction_inputs(txid, vout);

        -- transaction_outputs
        CREATE INDEX IF NOT EXISTS idx_tx_outputs_tx_id ON transaction_outputs(transaction_id);
        CREATE INDEX IF NOT EXISTS idx_tx_outputs_address ON transaction_outputs(address);

        -- messages
        CREATE INDEX IF NOT EXISTS idx_messages_recipient_box ON messages(recipient, message_box);

        -- relay_messages
        CREATE INDEX IF NOT EXISTS idx_relay_messages_recipient ON relay_messages(recipient, message_box);
        CREATE INDEX IF NOT EXISTS idx_relay_messages_expires ON relay_messages(expires_at);

        -- users
        CREATE INDEX IF NOT EXISTS idx_users_identity_key ON users(identity_key);

        -- proven_txs
        CREATE INDEX IF NOT EXISTS idx_proven_txs_txid ON proven_txs(txid);
        CREATE INDEX IF NOT EXISTS idx_proven_txs_height ON proven_txs(height);

        -- proven_tx_reqs
        CREATE INDEX IF NOT EXISTS idx_proven_tx_reqs_status ON proven_tx_reqs(status);
        CREATE INDEX IF NOT EXISTS idx_proven_tx_reqs_txid ON proven_tx_reqs(txid);
        CREATE INDEX IF NOT EXISTS idx_proven_tx_reqs_proven_tx ON proven_tx_reqs(proven_tx_id);

        -- transactions
        CREATE INDEX IF NOT EXISTS idx_transactions_txid ON transactions(txid);
        CREATE INDEX IF NOT EXISTS idx_transactions_reference ON transactions(reference_number);
        CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);
        CREATE INDEX IF NOT EXISTS idx_transactions_created_at ON transactions(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_transactions_user_id ON transactions(user_id);
        CREATE INDEX IF NOT EXISTS idx_transactions_proven_tx ON transactions(proven_tx_id);
        CREATE INDEX IF NOT EXISTS idx_transactions_failed_at ON transactions(failed_at) WHERE status = 'failed';

        -- certificates
        CREATE INDEX IF NOT EXISTS idx_certificates_type ON certificates(type);
        CREATE INDEX IF NOT EXISTS idx_certificates_certifier ON certificates(certifier);
        CREATE INDEX IF NOT EXISTS idx_certificates_subject ON certificates(subject);
        CREATE INDEX IF NOT EXISTS idx_certificates_user_id ON certificates(user_id);
        CREATE INDEX IF NOT EXISTS idx_certificates_active ON certificates(is_deleted) WHERE is_deleted = 0;

        -- certificate_fields
        CREATE INDEX IF NOT EXISTS idx_certificate_fields_cert_id ON certificate_fields(certificateId);
        CREATE INDEX IF NOT EXISTS idx_certificate_fields_user_id ON certificate_fields(user_id);

        -- output_baskets
        CREATE INDEX IF NOT EXISTS idx_output_baskets_name ON output_baskets(name);
        CREATE INDEX IF NOT EXISTS idx_output_baskets_user_id ON output_baskets(user_id);

        -- outputs
        CREATE INDEX IF NOT EXISTS idx_outputs_spendable ON outputs(spendable);
        CREATE INDEX IF NOT EXISTS idx_outputs_user_id ON outputs(user_id);
        CREATE INDEX IF NOT EXISTS idx_outputs_txid ON outputs(txid);
        CREATE INDEX IF NOT EXISTS idx_outputs_basket_id ON outputs(basket_id);
        CREATE INDEX IF NOT EXISTS idx_outputs_transaction_id ON outputs(transaction_id);
        CREATE INDEX IF NOT EXISTS idx_outputs_txid_vout ON outputs(txid, vout);

        -- output_tags
        CREATE INDEX IF NOT EXISTS idx_output_tags_tag ON output_tags(tag);
        CREATE INDEX IF NOT EXISTS idx_output_tags_user_id ON output_tags(user_id);
        CREATE INDEX IF NOT EXISTS idx_output_tags_deleted ON output_tags(is_deleted) WHERE is_deleted = 0;

        -- output_tag_map
        CREATE INDEX IF NOT EXISTS idx_output_tag_map_output_id ON output_tag_map(output_id);
        CREATE INDEX IF NOT EXISTS idx_output_tag_map_tag_id ON output_tag_map(output_tag_id);
        CREATE INDEX IF NOT EXISTS idx_output_tag_map_deleted ON output_tag_map(is_deleted) WHERE is_deleted = 0;

        -- tx_labels
        CREATE INDEX IF NOT EXISTS idx_tx_labels_label ON tx_labels(label);
        CREATE INDEX IF NOT EXISTS idx_tx_labels_user_id ON tx_labels(user_id);

        -- tx_labels_map
        CREATE INDEX IF NOT EXISTS idx_tx_labels_map_tx_id ON tx_labels_map(transaction_id);
        CREATE INDEX IF NOT EXISTS idx_tx_labels_map_label_id ON tx_labels_map(txLabelId);

        -- commissions
        CREATE INDEX IF NOT EXISTS idx_commissions_user_id ON commissions(user_id);
        CREATE INDEX IF NOT EXISTS idx_commissions_transaction_id ON commissions(transaction_id);

        -- monitor_events
        CREATE INDEX IF NOT EXISTS idx_monitor_events_event ON monitor_events(event);
        CREATE INDEX IF NOT EXISTS idx_monitor_events_created_at ON monitor_events(created_at);

        -- sync_states
        CREATE INDEX IF NOT EXISTS idx_sync_states_user_id ON sync_states(user_id);
        CREATE INDEX IF NOT EXISTS idx_sync_states_ref_num ON sync_states(ref_num);

        -- =====================================================================
        -- Domain permissions (Phase 2.1, updated Phase 2.3)
        -- =====================================================================

        CREATE TABLE IF NOT EXISTS domain_permissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            domain TEXT NOT NULL,
            trust_level TEXT NOT NULL DEFAULT 'unknown',
            per_tx_limit_cents INTEGER NOT NULL DEFAULT 100,
            per_session_limit_cents INTEGER NOT NULL DEFAULT 1000,
            rate_limit_per_min INTEGER NOT NULL DEFAULT 30,
            max_tx_per_session INTEGER NOT NULL DEFAULT 100,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(userId),
            UNIQUE(user_id, domain)
        );

        CREATE INDEX IF NOT EXISTS idx_domain_permissions_domain ON domain_permissions(domain);
        CREATE INDEX IF NOT EXISTS idx_domain_permissions_user_id ON domain_permissions(user_id);

        CREATE TABLE IF NOT EXISTS cert_field_permissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            domain_permission_id INTEGER NOT NULL,
            cert_type TEXT NOT NULL,
            field_name TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (domain_permission_id) REFERENCES domain_permissions(id) ON DELETE CASCADE,
            UNIQUE(domain_permission_id, cert_type, field_name)
        );

        CREATE INDEX IF NOT EXISTS idx_cert_field_perms_domain ON cert_field_permissions(domain_permission_id);

        -- =====================================================================
        -- PeerPay / Notification tracking (V7 + V8)
        -- =====================================================================

        CREATE TABLE IF NOT EXISTS peerpay_received (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id TEXT NOT NULL UNIQUE,
            sender_identity_key TEXT NOT NULL,
            amount_satoshis INTEGER NOT NULL,
            derivation_prefix TEXT NOT NULL,
            derivation_suffix TEXT NOT NULL,
            txid TEXT,
            accepted_at TEXT NOT NULL DEFAULT (datetime('now')),
            dismissed INTEGER NOT NULL DEFAULT 0,
            source TEXT NOT NULL DEFAULT 'peerpay',
            price_usd_cents INTEGER,
            notification_type TEXT NOT NULL DEFAULT 'receive'
        );
        CREATE INDEX IF NOT EXISTS idx_peerpay_dismissed ON peerpay_received(dismissed);
        CREATE INDEX IF NOT EXISTS idx_peerpay_source ON peerpay_received(source);
    ")?;

    info!("   ✅ Consolidated schema V1 created successfully");
    Ok(())
}

/// Migrate V1 → V2: Add pin_salt column to wallets table
pub fn migrate_v1_to_v2(conn: &Connection) -> Result<()> {
    // V1 fresh DBs already have pin_salt — only ALTER for pre-V2 existing DBs
    let has_pin_salt: bool = {
        let mut stmt = conn.prepare("PRAGMA table_info(wallets)")?;
        let cols: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        cols.iter().any(|c| c == "pin_salt")
    };
    if has_pin_salt {
        info!("   pin_salt column already exists — skipping ALTER");
    } else {
        info!("   Adding pin_salt column to wallets...");
        conn.execute("ALTER TABLE wallets ADD COLUMN pin_salt TEXT", [])?;
    }
    info!("   ✅ V2 migration applied (PIN support)");
    Ok(())
}

/// Migrate V2 → V3: Add domain_permissions and cert_field_permissions tables
pub fn migrate_v2_to_v3(conn: &Connection) -> Result<()> {
    info!("   Adding domain_permissions and cert_field_permissions tables...");

    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS domain_permissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            domain TEXT NOT NULL,
            trust_level TEXT NOT NULL DEFAULT 'unknown',
            per_tx_limit_cents INTEGER NOT NULL DEFAULT 100,
            per_session_limit_cents INTEGER NOT NULL DEFAULT 1000,
            rate_limit_per_min INTEGER NOT NULL DEFAULT 30,
            max_tx_per_session INTEGER NOT NULL DEFAULT 100,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(userId),
            UNIQUE(user_id, domain)
        );

        CREATE INDEX IF NOT EXISTS idx_domain_permissions_domain ON domain_permissions(domain);
        CREATE INDEX IF NOT EXISTS idx_domain_permissions_user_id ON domain_permissions(user_id);

        CREATE TABLE IF NOT EXISTS cert_field_permissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            domain_permission_id INTEGER NOT NULL,
            cert_type TEXT NOT NULL,
            field_name TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (domain_permission_id) REFERENCES domain_permissions(id) ON DELETE CASCADE,
            UNIQUE(domain_permission_id, cert_type, field_name)
        );

        CREATE INDEX IF NOT EXISTS idx_cert_field_perms_domain ON cert_field_permissions(domain_permission_id);
    ")?;

    info!("   ✅ V3 migration applied (domain permissions)");
    Ok(())
}

/// Migrate V3 → V4: Add mnemonic_dpapi column for Windows DPAPI auto-unlock
pub fn migrate_v3_to_v4(conn: &Connection) -> Result<()> {
    // V1 fresh DBs already have mnemonic_dpapi — only ALTER for pre-V4 existing DBs
    let has_dpapi: bool = {
        let mut stmt = conn.prepare("PRAGMA table_info(wallets)")?;
        let cols: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        cols.iter().any(|c| c == "mnemonic_dpapi")
    };
    if has_dpapi {
        info!("   mnemonic_dpapi column already exists — skipping ALTER");
    } else {
        info!("   Adding mnemonic_dpapi column to wallets...");
        conn.execute("ALTER TABLE wallets ADD COLUMN mnemonic_dpapi BLOB", [])?;
    }
    info!("   ✅ V4 migration applied (DPAPI auto-unlock)");
    Ok(())
}

/// Migrate V4 → V5: No-op (adblock_enabled moved to C++ AdblockCache JSON)
pub fn migrate_v4_to_v5(_conn: &Connection) -> Result<()> {
    info!("   ✅ V5 migration — no-op (adblock settings moved to C++ AdblockCache)");
    Ok(())
}

/// Migrate V5 → V6: No-op (scriptlets_enabled moved to C++ AdblockCache JSON)
pub fn migrate_v5_to_v6(_conn: &Connection) -> Result<()> {
    info!("   ✅ V6 migration — no-op (scriptlet settings moved to C++ AdblockCache)");
    Ok(())
}

/// Migrate V6 → V7: Add peerpay_received table for persistent PeerPay payment tracking
pub fn migrate_v6_to_v7(conn: &Connection) -> Result<()> {
    info!("   Adding peerpay_received table...");

    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS peerpay_received (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id TEXT NOT NULL UNIQUE,
            sender_identity_key TEXT NOT NULL,
            amount_satoshis INTEGER NOT NULL,
            derivation_prefix TEXT NOT NULL,
            derivation_suffix TEXT NOT NULL,
            txid TEXT,
            accepted_at TEXT NOT NULL DEFAULT (datetime('now')),
            dismissed INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_peerpay_dismissed ON peerpay_received(dismissed);
    ")?;

    info!("   ✅ V7 migration applied (peerpay_received table)");
    Ok(())
}

/// Migrate V7 → V8: Add source column to peerpay_received for unified notifications
pub fn migrate_v7_to_v8(conn: &Connection) -> Result<()> {
    info!("   Adding source column to peerpay_received...");

    // Safe guard: column may already exist in consolidated V1 schema
    let has_source: bool = {
        let mut stmt = conn.prepare("PRAGMA table_info(peerpay_received)")?;
        let cols: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        cols.iter().any(|c| c == "source")
    };

    if has_source {
        info!("   source column already exists — skipping ALTER");
    } else {
        conn.execute(
            "ALTER TABLE peerpay_received ADD COLUMN source TEXT NOT NULL DEFAULT 'peerpay'",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_peerpay_source ON peerpay_received(source)",
            [],
        )?;
    }

    info!("   ✅ V8 migration applied (unified notifications)");
    Ok(())
}

/// Migrate V8 → V9: Add sender_display_name column to settings
pub fn migrate_v8_to_v9(conn: &Connection) -> Result<()> {
    info!("   Adding sender_display_name column to settings...");

    let has_col: bool = {
        let mut stmt = conn.prepare("PRAGMA table_info(settings)")?;
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        cols.iter().any(|c| c == "sender_display_name")
    };

    if has_col {
        info!("   sender_display_name column already exists — skipping ALTER");
    } else {
        conn.execute(
            "ALTER TABLE settings ADD COLUMN sender_display_name TEXT NOT NULL DEFAULT 'Anonymous'",
            [],
        )?;
    }

    info!("   ✅ V9 migration applied (sender display name)");
    Ok(())
}

/// Migrate V9 → V10: Add default auto-approve limit columns to settings
pub fn migrate_v9_to_v10(conn: &Connection) -> Result<()> {
    info!("   Adding default auto-approve limit columns to settings...");

    let cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(settings)")?;
        let result: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    if !cols.iter().any(|c| c == "default_per_tx_limit_cents") {
        conn.execute(
            "ALTER TABLE settings ADD COLUMN default_per_tx_limit_cents INTEGER NOT NULL DEFAULT 1000",
            [],
        )?;
    }

    if !cols.iter().any(|c| c == "default_per_session_limit_cents") {
        conn.execute(
            "ALTER TABLE settings ADD COLUMN default_per_session_limit_cents INTEGER NOT NULL DEFAULT 5000",
            [],
        )?;
    }

    if !cols.iter().any(|c| c == "default_rate_limit_per_min") {
        conn.execute(
            "ALTER TABLE settings ADD COLUMN default_rate_limit_per_min INTEGER NOT NULL DEFAULT 10",
            [],
        )?;
    }

    info!("   ✅ V10 migration applied (default auto-approve limits)");
    Ok(())
}

/// Migrate V10 → V11: Add price_usd_cents column to transactions and peerpay_received
///
/// Records BSV/USD price in cents at the time of each transaction.
/// Nullable — old transactions will show current price only.
pub fn migrate_v10_to_v11(conn: &Connection) -> Result<()> {
    info!("   Adding price_usd_cents column to transactions and peerpay_received...");

    // transactions table
    let tx_cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(transactions)")?;
        let result: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    if !tx_cols.iter().any(|c| c == "price_usd_cents") {
        conn.execute(
            "ALTER TABLE transactions ADD COLUMN price_usd_cents INTEGER",
            [],
        )?;
    }

    // peerpay_received table
    let pp_cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(peerpay_received)")?;
        let result: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    if !pp_cols.iter().any(|c| c == "price_usd_cents") {
        conn.execute(
            "ALTER TABLE peerpay_received ADD COLUMN price_usd_cents INTEGER",
            [],
        )?;
    }

    info!("   ✅ V11 migration applied (price at transaction time)");
    Ok(())
}

/// Migrate V11 → V12: Add max_tx_per_session to domain_permissions and settings;
/// update default limit values to reflect production-ready settings.
pub fn migrate_v11_to_v12(conn: &Connection) -> Result<()> {
    info!("   Adding max_tx_per_session column and updating defaults...");

    // domain_permissions: add max_tx_per_session column
    let dp_cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(domain_permissions)")?;
        let result: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    if !dp_cols.iter().any(|c| c == "max_tx_per_session") {
        conn.execute(
            "ALTER TABLE domain_permissions ADD COLUMN max_tx_per_session INTEGER NOT NULL DEFAULT 100",
            [],
        )?;
    }

    // settings: add default_max_tx_per_session column
    let settings_cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(settings)")?;
        let result: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    if !settings_cols.iter().any(|c| c == "default_max_tx_per_session") {
        conn.execute(
            "ALTER TABLE settings ADD COLUMN default_max_tx_per_session INTEGER DEFAULT 100",
            [],
        )?;
    }

    // Update existing settings row to use the new production defaults
    conn.execute(
        "UPDATE settings SET default_per_tx_limit_cents = 100, default_per_session_limit_cents = 1000, default_rate_limit_per_min = 30",
        [],
    )?;

    info!("   ✅ V12 migration applied (max_tx_per_session + updated defaults)");
    Ok(())
}

/// Migrate V12 → V13: Add recipient and recipient_name columns to transactions
///
/// Stores the raw recipient value (BSV address, paymail, or identity key) and
/// resolved display name at send time. Enables recipient autocomplete from history.
/// Nullable — old transactions will have NULL (suggest endpoint falls back to description parsing).
pub fn migrate_v12_to_v13(conn: &Connection) -> Result<()> {
    info!("   Adding recipient and recipient_name columns to transactions...");

    let tx_cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(transactions)")?;
        let result: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    if !tx_cols.iter().any(|c| c == "recipient") {
        conn.execute(
            "ALTER TABLE transactions ADD COLUMN recipient TEXT",
            [],
        )?;
    }

    if !tx_cols.iter().any(|c| c == "recipient_name") {
        conn.execute(
            "ALTER TABLE transactions ADD COLUMN recipient_name TEXT",
            [],
        )?;
    }

    info!("   ✅ V13 migration applied (recipient autocomplete)");
    Ok(())
}

/// Migrate V13 → V14: Add confirmed column to outputs, notification_type to peerpay_received
///
/// - `confirmed` tracks whether a received UTXO has been seen in confirmed API (vs unconfirmed mempool only)
/// - `notification_type` distinguishes green receive notifications from red failure notifications
pub fn migrate_v13_to_v14(conn: &Connection) -> Result<()> {
    info!("   Adding confirmed column to outputs and notification_type to peerpay_received...");

    // outputs: add confirmed column (1 = confirmed, 0 = unconfirmed/mempool-only)
    let output_cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(outputs)")?;
        let result: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    if !output_cols.iter().any(|c| c == "confirmed") {
        conn.execute(
            "ALTER TABLE outputs ADD COLUMN confirmed INTEGER NOT NULL DEFAULT 1",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_outputs_confirmed ON outputs(confirmed)",
            [],
        )?;
    }

    // peerpay_received: add notification_type column ('receive' or 'failure')
    let pp_cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(peerpay_received)")?;
        let result: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    if !pp_cols.iter().any(|c| c == "notification_type") {
        conn.execute(
            "ALTER TABLE peerpay_received ADD COLUMN notification_type TEXT NOT NULL DEFAULT 'receive'",
            [],
        )?;
    }

    info!("   ✅ V14 migration applied (confirmed outputs + notification types)");
    Ok(())
}
