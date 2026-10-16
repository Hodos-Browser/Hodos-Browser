//! Database schema migrations
//!
//! Single clean V1 migration creating the final target schema.
//! All 24 incremental migrations have been consolidated.

use rusqlite::{Connection, OptionalExtension, Result};
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

pub fn migrate_v14_to_v15(conn: &Connection) -> Result<()> {
    info!("   Adding peerpay_pending_verification table for chain validation tracking...");

    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS peerpay_pending_verification (
            message_id TEXT NOT NULL PRIMARY KEY,
            txid TEXT NOT NULL,
            first_seen_at INTEGER NOT NULL,
            retry_count INTEGER NOT NULL DEFAULT 0,
            last_retry_at INTEGER NOT NULL
        );
    ")?;

    info!("   ✅ V15 migration applied (peerpay_pending_verification)");
    Ok(())
}

pub fn migrate_v15_to_v16(conn: &Connection) -> Result<()> {
    info!("   Adding peerpay_outbox table for MessageBox delivery retry...");

    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS peerpay_outbox (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            txid TEXT NOT NULL UNIQUE,
            recipient_pubkey_hex TEXT NOT NULL,
            payload_bytes BLOB NOT NULL,
            amount_satoshis INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            retry_count INTEGER NOT NULL DEFAULT 0,
            next_retry_at INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_peerpay_outbox_status ON peerpay_outbox(status);
        CREATE INDEX IF NOT EXISTS idx_peerpay_outbox_next_retry ON peerpay_outbox(next_retry_at);
    ")?;

    info!("   ✅ V16 migration applied (peerpay_outbox)");
    Ok(())
}

/// Migrate V16 → V17: Add identity_key_disclosure_allowed to domain_permissions.
///
/// Phase 1.5 Step 1 — privacy-perimeter persistence. When the user approves a
/// site (domain_approval modal) with the "Allow this site to identify you"
/// checkbox ticked (default ON), this column is set to 1 so subsequent
/// getPublicKey({identityKey:true}) requests bypass the privacy-perimeter
/// prompt for that site. 0 means always prompt (the safe default for any
/// pre-existing row).
///
/// Idempotent: checks PRAGMA table_info before ALTER.
pub fn migrate_v16_to_v17(conn: &Connection) -> Result<()> {
    info!("   Adding identity_key_disclosure_allowed column to domain_permissions...");

    let dp_cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(domain_permissions)")?;
        let result: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    if !dp_cols.iter().any(|c| c == "identity_key_disclosure_allowed") {
        conn.execute(
            "ALTER TABLE domain_permissions ADD COLUMN identity_key_disclosure_allowed INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    info!("   ✅ V17 migration applied (identity_key_disclosure_allowed)");
    Ok(())
}

/// Migrate V17 → V18: Three child tables of domain_permissions for BRC-100
/// fine-grained sub-permissions (protocol / basket / counterparty).
///
/// Phase 1.5 Step 2. Each table mirrors the cert_field_permissions pattern:
///   - FK to domain_permissions(id) ON DELETE CASCADE (revoking a site nukes
///     all its sub-permissions)
///   - UNIQUE constraint on the logical key for idempotent INSERT-or-update
///   - expires_at INTEGER nullable (NULL = never; matches @bsv/wallet-toolbox
///     cert lifecycle convention)
///   - revoked_at INTEGER nullable (NULL = active; Unix epoch when revoked)
///     -- chosen over is_deleted INTEGER because it captures both fact AND
///     timestamp in one column, staying in the project's Unix-epoch convention
///     used by created_at/updated_at/failed_at
///   - Companion index on domain_permission_id for FK join performance
///
/// No handlers consume these tables yet; Step 6 wires them through the new
/// permission engine. Empty tables sitting on existing dev DBs is by design.
pub fn migrate_v17_to_v18(conn: &Connection) -> Result<()> {
    info!("   Adding three child tables of domain_permissions (protocol / basket / counterparty)...");

    // Idempotent: SQLite's IF NOT EXISTS guards each CREATE.
    conn.execute_batch("
        -- Per-protocol grants (BRC-100 PermissionRequest type='protocol').
        -- key_id default '*' = wildcard for any keyID under this protocol.
        -- counterparty NULL = any counterparty.
        CREATE TABLE IF NOT EXISTS domain_protocol_permissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            domain_permission_id INTEGER NOT NULL,
            protocol_security_level INTEGER NOT NULL,
            protocol_name TEXT NOT NULL,
            key_id TEXT NOT NULL DEFAULT '*',
            counterparty TEXT,
            expires_at INTEGER,
            revoked_at INTEGER,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (domain_permission_id) REFERENCES domain_permissions(id) ON DELETE CASCADE,
            UNIQUE(domain_permission_id, protocol_security_level, protocol_name, key_id, counterparty)
        );
        CREATE INDEX IF NOT EXISTS idx_domain_protocol_perms_domain
            ON domain_protocol_permissions(domain_permission_id);

        -- Per-basket grants (BRC-100 PermissionRequest type='basket').
        -- access: 'read' | 'read_write'
        CREATE TABLE IF NOT EXISTS domain_basket_permissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            domain_permission_id INTEGER NOT NULL,
            basket TEXT NOT NULL,
            access TEXT NOT NULL,
            expires_at INTEGER,
            revoked_at INTEGER,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (domain_permission_id) REFERENCES domain_permissions(id) ON DELETE CASCADE,
            UNIQUE(domain_permission_id, basket)
        );
        CREATE INDEX IF NOT EXISTS idx_domain_basket_perms_domain
            ON domain_basket_permissions(domain_permission_id);

        -- Per-counterparty grants (BRC-100 CounterpartyPermissionRequest, level-2 protocols).
        -- counterparty: hex compressed pubkey (33 bytes = 66 hex chars)
        CREATE TABLE IF NOT EXISTS domain_counterparty_permissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            domain_permission_id INTEGER NOT NULL,
            counterparty TEXT NOT NULL,
            expires_at INTEGER,
            revoked_at INTEGER,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (domain_permission_id) REFERENCES domain_permissions(id) ON DELETE CASCADE,
            UNIQUE(domain_permission_id, counterparty)
        );
        CREATE INDEX IF NOT EXISTS idx_domain_counterparty_perms_domain
            ON domain_counterparty_permissions(domain_permission_id);
    ")?;

    info!("   ✅ V18 migration applied (three child tables: protocol / basket / counterparty)");
    Ok(())
}

/// Migrate V18 → V19: Add `default_identity_key_disclosure_allowed` to settings.
///
/// Phase 1.5 Step 5 — global default for the bundle checkbox that appears on
/// domain_approval and manifest_connect_bundle modals. When the user changes
/// this in the "Default Limits for New Sites" form, future fresh-site prompts
/// initialize the bundle checkbox to this value. Default 1 (ON) preserves the
/// Step 1 behavior — most users will never change it.
pub fn migrate_v18_to_v19(conn: &Connection) -> Result<()> {
    info!("   Adding default_identity_key_disclosure_allowed column to settings...");

    let cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(settings)")?;
        let result: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    if !cols.iter().any(|c| c == "default_identity_key_disclosure_allowed") {
        conn.execute(
            "ALTER TABLE settings ADD COLUMN default_identity_key_disclosure_allowed INTEGER NOT NULL DEFAULT 1",
            [],
        )?;
    }

    info!("   ✅ V19 migration applied (default_identity_key_disclosure_allowed)");
    Ok(())
}

/// V19 → V20 — Phase 2.6-A.5.
///
/// Creates two new tables for the engine-to-Rust migration:
///
/// 1. `permission_audit_log` — long-lived audit surface for engine decisions.
///    Retention: 90 days (OQ1 resolved 2026-06-02). Background purge task
///    drops rows older than 90 days; index on `created_at` supports efficient
///    pruning. Per OQ2, the request body is stored as a sha256 hex hash
///    (`VARCHAR(64)`), NOT the raw body — captures call identity for forensic
///    provenance without storing raw payload bytes.
///
/// 2. `engine_shadow_log` — short-lived shadow comparison surface used during
///    the Phase 2.6 migration window. Records every C++ vs Rust engine
///    disagreement so we can verify the port is correct before flipping flags
///    in 2.6-C through 2.6-G. Will be dropped in the eventual 2.6-H cleanup
///    migration (originally planned as V21; bumped to a later version since
///    V21 was used 2026-06-09 by the BSV-price persistence change — see
///    `migrate_v20_to_v21` below).
///
/// Idempotent: uses `CREATE TABLE IF NOT EXISTS` so re-running on a
/// partially-migrated DB is safe.
///
/// See: `development-docs/Sigma-BRC121-Sprint/phase-2.6-engine-to-rust/SUBPHASE_2_6_A_DESIGN.md` §5.
pub fn migrate_v19_to_v20(conn: &Connection) -> Result<()> {
    info!("   Creating permission_audit_log + engine_shadow_log tables (V20)...");

    // permission_audit_log — long-lived audit surface (90-day retention via
    // background purge task added in 2.6-A.6 or later).
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS permission_audit_log (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            approval_id     VARCHAR(32),
            domain          TEXT    NOT NULL,
            endpoint        TEXT    NOT NULL,
            call_kind       TEXT    NOT NULL,
            engine_reason   TEXT    NOT NULL,
            decision        TEXT    NOT NULL,
            user_decision   TEXT,
            body_hash       VARCHAR(64) NOT NULL,
            created_at      INTEGER NOT NULL,
            resolved_at     INTEGER,
            resolved_via    TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_audit_created_at ON permission_audit_log(created_at);
        CREATE INDEX IF NOT EXISTS idx_audit_domain     ON permission_audit_log(domain);
        CREATE INDEX IF NOT EXISTS idx_audit_approval   ON permission_audit_log(approval_id);"
    )?;

    // engine_shadow_log — dropped in 2.6-H cleanup (V23, migrate_v22_to_v23).
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS engine_shadow_log (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            call_kind_class TEXT    NOT NULL,
            endpoint        TEXT    NOT NULL,
            domain          TEXT    NOT NULL,
            cpp_decision    TEXT    NOT NULL,
            rust_decision   TEXT    NOT NULL,
            cpp_reason      TEXT,
            rust_reason     TEXT,
            agreement       INTEGER NOT NULL,
            context_hash    VARCHAR(64) NOT NULL,
            observed_at     INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shadow_observed_at ON engine_shadow_log(observed_at);
        CREATE INDEX IF NOT EXISTS idx_shadow_agreement   ON engine_shadow_log(agreement);
        CREATE INDEX IF NOT EXISTS idx_shadow_class       ON engine_shadow_log(call_kind_class);"
    )?;

    info!("   ✅ V20 migration applied (permission_audit_log + engine_shadow_log)");
    Ok(())
}

/// V20 → V21 — 2026-06-09 BSV price-cache persistence.
///
/// Adds the `bsv_price_cache` table so the wallet has a fallback BSV/USD
/// price across process restarts. Pre-this-migration, `price_cache.rs` was
/// in-memory only — when both upstream price sources (CryptoCompare and
/// CoinGecko's old `bitcoin-sv` slug) broke under us on the same day, every
/// cold-start wallet had no fallback and every payment had to prompt with
/// `engineReason=price_unavailable`. With this migration, the last known good
/// price + timestamp + source survives restart; the in-memory cache loads
/// from the table at startup and persists back on every successful live
/// fetch.
///
/// Schema:
///   - `id INTEGER PRIMARY KEY CHECK (id = 1)` — single-row pattern. The
///     CHECK enforces it so an accidental INSERT can't create a second row.
///   - `price_usd REAL NOT NULL` — last known good price.
///   - `fetched_at INTEGER NOT NULL` — Unix epoch seconds when fetched.
///     Lets the engine + frontend show "(price is N hours old)" warnings.
///   - `source TEXT NOT NULL` — which upstream provided the value
///     ("whatsonchain" / "coingecko" / "mexc"). Useful for diagnosing
///     a stuck-but-persistent stale value.
///
/// Idempotent: `CREATE TABLE IF NOT EXISTS`. Re-running on an already-V21
/// DB is a no-op.
pub fn migrate_v20_to_v21(conn: &Connection) -> Result<()> {
    info!("   Creating bsv_price_cache table (V21)...");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS bsv_price_cache (
            id          INTEGER PRIMARY KEY CHECK (id = 1),
            price_usd   REAL    NOT NULL,
            fetched_at  INTEGER NOT NULL,
            source      TEXT    NOT NULL
        );"
    )?;

    info!("   ✅ V21 migration applied (bsv_price_cache table)");
    Ok(())
}

/// V21 → V22 — 2026-06-09 bundled scope grant column on `domain_permissions`.
///
/// Phase 2.6-D Fix #4: when the user ticks "Allow this site to perform wallet
/// operations without prompting each time" on the connect modal
/// (`domain_approval` / `manifest_connect_bundle`), this column is set to 1.
/// The Rust permission engine then returns `Silent(SilentBundledScopeGrant)`
/// for ProtocolUse and BasketAccess calls from that domain, regardless of
/// whether a matching V18 row exists.
///
/// Protected baskets (`default` / `backup-*` / `admin *`) are NOT silenced
/// even with `bundled_scope_grant=1` — `dispatch_scoped_grant` overrides the
/// flag to false before calling the engine for those targets.
///
/// CounterpartyUse is already silenced by Fix #3 in the engine and does not
/// consult this column.
///
/// Idempotent: adds the column only if missing, so re-running on an
/// already-V22 DB is a no-op.
pub fn migrate_v21_to_v22(conn: &Connection) -> Result<()> {
    info!("   Adding bundled_scope_grant column to domain_permissions (V22)...");

    let has_column = {
        let mut stmt = conn.prepare("PRAGMA table_info(domain_permissions)")?;
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        cols.iter().any(|c| c == "bundled_scope_grant")
    };

    if !has_column {
        conn.execute(
            "ALTER TABLE domain_permissions
             ADD COLUMN bundled_scope_grant INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
        info!("   ✅ V22 migration applied (bundled_scope_grant column)");
    } else {
        info!("   ℹ️  V22 migration: bundled_scope_grant column already exists, skipping");
    }

    Ok(())
}

/// V22 → V23 — Phase 2.6-H cleanup.
///
/// Drops the `engine_shadow_log` table created in V20. The shadow-comparison
/// infrastructure (C++ `EngineShadow` + the Rust `/engine/shadow-decide`
/// handler + `engine_shadow_repo`) was deleted in 2.6-H along with the rest of
/// the C++ PermissionEngine — all permission decisions are now Rust-native, so
/// there is no second engine to compare against. `permission_audit_log` (the
/// long-lived audit surface) is intentionally kept.
///
/// Idempotent: `DROP TABLE IF EXISTS` is a no-op on a DB that never had the
/// table (or already dropped it).
pub fn migrate_v22_to_v23(conn: &Connection) -> Result<()> {
    info!("   Dropping engine_shadow_log table (V23)...");
    conn.execute_batch("DROP TABLE IF EXISTS engine_shadow_log;")?;
    info!("   ✅ V23 migration applied (engine_shadow_log dropped)");
    Ok(())
}

/// V23 → V24 — beta.3 Phase 0.8. Manifest snapshots + the pre-fill toggle.
///
/// Owner-approved 2026-08-22 (`development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/
/// PHASE_CONTRACT.md` §6b) as EXACTLY this shape. Two additive changes; no existing
/// column or table is altered or dropped.
///
/// **1. `domain_manifest_snapshots`** — what a site asked for, as approved.
///
/// A child table joined by FK + `ON DELETE CASCADE` off `domain_permissions(id)`,
/// mirroring the `cert_field_permissions` pattern CLAUDE.md names as the reuse
/// anchor. Revoking a site disposes of its snapshot — no orphaned record of a
/// site the user removed. Storing a newer fetch beside the approved one is what
/// lets a *changed* manifest be detected and treated as re-consent.
///
/// ⛔ Three rules, all load-bearing (contract §6a):
///
/// 1. **Informational only — never a decision input.** Authoritative permission
///    state stays in `domain_permissions` and the V18 child tables. This table
///    feeds display and a future user-initiated "apply these" action, nothing
///    else. (A discipline we adopt by choice; BRC-116's rule about caches not
///    being authoritative is about cached *permission state*, and a record of
///    what a site *asked for* is not a grant. Do not cite it as compelling this.)
/// 2. **As approved, not live.** `approved_at` non-NULL marks the row the user
///    actually saw. 🚨 Storing the live manifest instead would let a site publish
///    modest recommendations, get approved, publish aggressive ones, and have a
///    later "restore recommended" click silently adopt numbers never shown.
/// 3. Schema change ⇒ owner approval. Granted for this shape only.
///
/// `manifest_json` holds the raw bytes as fetched, already ≤ 64 KB by the fetch
/// cap (`manifest.rs :: MAX_MANIFEST_BYTES`). bitgenius, the largest real one
/// observed, is 3358 bytes — one row per approved domain is negligible.
///
/// **2. `settings.default_prefill_from_manifest`** — the pre-fill opt-in.
///
/// Default `0` = behaviour (b): the connect modal's limit fields carry the
/// USER's defaults and the site's suggestion is shown beside them. `1` = (a):
/// the site's suggested values are pre-filled instead. ⛔ The toggle changes
/// only WHICH values are pre-filled — site-sourced fields stay visibly marked
/// and "Use my defaults" still works in both states (`R-PROV`, `P0.8-A12`).
/// Mirrors the `default_identity_key_disclosure_allowed` precedent (V19).
///
/// Idempotent: `CREATE TABLE IF NOT EXISTS` + an existence-checked `ALTER`, so
/// re-running on an already-V24 DB is a no-op.
pub fn migrate_v23_to_v24(conn: &Connection) -> Result<()> {
    info!("   Creating domain_manifest_snapshots + default_prefill_from_manifest (V24)...");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS domain_manifest_snapshots (
            id                   INTEGER PRIMARY KEY AUTOINCREMENT,
            domain_permission_id INTEGER NOT NULL,
            manifest_json        TEXT    NOT NULL,
            source_url           TEXT    NOT NULL,
            fetched_at           INTEGER NOT NULL,
            approved_at          INTEGER,
            FOREIGN KEY (domain_permission_id) REFERENCES domain_permissions(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_domain_manifest_snapshots_domain
            ON domain_manifest_snapshots(domain_permission_id);"
    )?;

    let cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(settings)")?;
        let out: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        out
    };
    if !cols.iter().any(|c| c == "default_prefill_from_manifest") {
        conn.execute(
            "ALTER TABLE settings ADD COLUMN default_prefill_from_manifest INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    } else {
        info!("   ℹ️  V24: default_prefill_from_manifest already exists, skipping");
    }

    info!("   ✅ V24 migration applied (domain_manifest_snapshots + default_prefill_from_manifest)");
    Ok(())
}

/// Migrate V24 → V25: add `settings.default_bundled_scope_grant`.
///
/// beta.3 Phase 0.8, owner-approved 2026-08-23 during live testing.
///
/// "Quiet mode" (`domain_permissions.bundled_scope_grant`) already exists
/// PER SITE — it is the connect modal's *"let this site use any protocol or
/// basket without asking"* checkbox, and the engine consults it in
/// `matrix_c.rs :: decide_scoped_grant` before any per-scope grant. What did
/// not exist was a USER-LEVEL DEFAULT for how that checkbox starts on a fresh
/// site, so it was hardcoded ON in the modal.
///
/// That is the same gap V19 closed for identity-key disclosure and V24 closed
/// for manifest pre-fill, and it is the more consequential of the three:
/// quiet mode is the widest grant on the screen, covering protocols and
/// baskets the site never declared. A user who wants to be asked every time
/// had to untick it on every single site.
///
/// ⚠️ Ships `1` (ON), deliberately matching the modal's existing hardcoded
/// default. This migration must not change behaviour for anyone — it only
/// makes the existing default configurable. ⛔ Do not "improve" it to 0 here:
/// that would silently start prompting existing users on every protocol call,
/// which reads as a regression, not a hardening. Changing the shipped default
/// is a separate, deliberate decision.
///
/// See also `TICKET_quiet_mode_wider_than_manifest.md` — narrowing quiet mode
/// to what a manifest actually declared is the open follow-up, and it would
/// change what this default MEANS without changing its storage.
///
/// Idempotent: existence-checked `ALTER`, so re-running on a V25 DB is a no-op.
pub fn migrate_v24_to_v25(conn: &Connection) -> Result<()> {
    info!("   Adding default_bundled_scope_grant column to settings (V25)...");

    let cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(settings)")?;
        let out: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        out
    };
    if !cols.iter().any(|c| c == "default_bundled_scope_grant") {
        conn.execute(
            "ALTER TABLE settings ADD COLUMN default_bundled_scope_grant INTEGER NOT NULL DEFAULT 1",
            [],
        )?;
    } else {
        info!("   ℹ️  V25: default_bundled_scope_grant already exists, skipping");
    }

    info!("   ✅ V25 migration applied (default_bundled_scope_grant)");
    Ok(())
}

/// Migrate V25 → V26 — B5-T1-P4 (ingest truth). **Data only, no schema change.**
///
/// `settings.max_output_script` (default 500000) was never read until B5-T1-P4; it is
/// now the inline cap for `outputs.locking_script` (`@bsv/wallet-toolbox`'s
/// `maxOutputScript`). The phase's §12 Q1, accepted by the owner 2026-10-02, sets it to
/// 1,024 B: every money coin, an 860 B OrdLock listing and a BSV-21 prefix fit. Only
/// the untouched default is changed; a value someone set on purpose is kept.
///
/// The backfill of `outputs.script_length` (owner decision 1(a)) is deliberately NOT
/// here: a row may only gain a length from verified parent bytes, which needs the
/// network — `ingest::complete_unchecked`, run by TaskSyncPending, does it.
pub fn migrate_v25_to_v26(conn: &Connection) -> Result<()> {
    let n = conn.execute(
        "UPDATE settings SET max_output_script = 1024 WHERE max_output_script = 500000",
        [],
    )?;
    info!("   ✅ V26 migration applied (inline script cap 1024 B; {} settings row(s))", n);
    Ok(())
}

// ============================================================================
// V27 — B5-T1-P3: the money index (`money_utxos`)
// ============================================================================

/// The output-local half of "this output is money". `o` is the outputs row alias.
/// B5-T1-P5 (owner decision 2): money only when **stamped** `change = 1` by the
/// classification seam (`classify.rs`); the derivation and basket terms are P3's rule,
/// kept. ⚠️ The triggers carry a copy of this text: changing it changes nothing in an
/// existing DB until [`ensure_money_index_current`] reinstalls them.
/// The transaction-status half (`unsigned/failed/nosend/nonfinal`) and the
/// confirmed-only rule of S3/S5/S6 are applied by the selectors' join, because they
/// change on `transactions`, not on the output (contract §2a).
fn money_eligible(o: &str) -> String {
    format!(
        "{o}.change = 1
         AND {o}.derivation_prefix IS NOT NULL
         AND ({o}.basket_id IS NULL OR EXISTS (SELECT 1 FROM output_baskets mb
                                               WHERE mb.basketId = {o}.basket_id AND mb.name = 'default'))",
        o = o
    )
}

/// Create `money_utxos` and the triggers that keep it, if absent. Idempotent: run by
/// V27, at every open (`WalletDatabase::migrate`'s repair step — a DB stamped V27
/// without the table is repaired, not trusted), and by test fixtures.
///
/// Shape follows go-wallet-toolbox v0.187.1 `models/user_utxo.go :: UserUTXO`
/// (`UserID`, `OutputID`, `Satoshis`, `ReservedByID`). Dropped, on purpose:
/// - `BasketName` — money is the `default` basket only; the eligibility rule says so.
/// - `UTXOStatus` — confirmation stays in `outputs`/`transactions`; the selectors join it.
/// - `EstimatedInputSize` — every money coin is P2PKH; the selector already assumes it.
///
/// ⭐ The rules the triggers encode (`outputs.spendable` stays authoritative):
/// 1. An output that becomes spendable money gets a **free** row (`reserved_by` NULL);
///    a reserved row for it is freed — whatever path restored the coin, including an
///    older binary that knows nothing of this table.
/// 2. An output that stops being spendable loses its **free** row only. A claim marks
///    the index row first and `outputs` second, so the claimed row survives this.
/// 3. An output that stops being money (basket, derivation) loses its row outright.
/// 4. A transaction that is consumed or fails drops the rows it reserved; if its inputs
///    are then restored, rule 1 re-adds them free.
/// 5. Deleting an output deletes its row (FK cascade, and a trigger for a connection
///    with `foreign_keys` off).
pub fn install_money_index(conn: &Connection) -> Result<()> {
    conn.execute_batch(&format!("
        CREATE TABLE IF NOT EXISTS money_utxos (
            output_id   INTEGER PRIMARY KEY REFERENCES outputs(outputId) ON DELETE CASCADE,
            user_id     INTEGER NOT NULL,
            satoshis    INTEGER NOT NULL,
            reserved_by INTEGER REFERENCES transactions(id),
            created_at  INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_money_utxos_user_free ON money_utxos(user_id, reserved_by, satoshis);
        CREATE INDEX IF NOT EXISTS idx_money_utxos_reserved_by ON money_utxos(reserved_by);

        CREATE TRIGGER IF NOT EXISTS money_utxos_on_output_insert
        AFTER INSERT ON outputs
        WHEN NEW.spendable = 1 AND {elig}
        BEGIN
            INSERT OR IGNORE INTO money_utxos (output_id, user_id, satoshis, reserved_by, created_at)
            VALUES (NEW.outputId, NEW.user_id, NEW.satoshis, NULL, CAST(strftime('%s','now') AS INTEGER));
        END;

        CREATE TRIGGER IF NOT EXISTS money_utxos_on_output_spendable
        AFTER UPDATE OF spendable, change, derivation_prefix, basket_id, satoshis, user_id ON outputs
        WHEN NEW.spendable = 1 AND {elig}
        BEGIN
            UPDATE money_utxos SET reserved_by = NULL, satoshis = NEW.satoshis, user_id = NEW.user_id
             WHERE output_id = NEW.outputId;
            INSERT OR IGNORE INTO money_utxos (output_id, user_id, satoshis, reserved_by, created_at)
            VALUES (NEW.outputId, NEW.user_id, NEW.satoshis, NULL, CAST(strftime('%s','now') AS INTEGER));
        END;

        CREATE TRIGGER IF NOT EXISTS money_utxos_on_output_unspendable
        AFTER UPDATE OF spendable ON outputs
        WHEN NEW.spendable = 0
        BEGIN
            DELETE FROM money_utxos WHERE output_id = NEW.outputId AND reserved_by IS NULL;
        END;

        CREATE TRIGGER IF NOT EXISTS money_utxos_on_output_not_money
        AFTER UPDATE OF change, derivation_prefix, basket_id ON outputs
        WHEN NOT ({elig})
        BEGIN
            DELETE FROM money_utxos WHERE output_id = NEW.outputId;
        END;

        CREATE TRIGGER IF NOT EXISTS money_utxos_on_output_delete
        AFTER DELETE ON outputs
        BEGIN
            DELETE FROM money_utxos WHERE output_id = OLD.outputId;
        END;

        CREATE TRIGGER IF NOT EXISTS money_utxos_on_tx_settled
        AFTER UPDATE OF status ON transactions
        WHEN NEW.status IN ('unproven', 'completed', 'failed')
        BEGIN
            DELETE FROM money_utxos WHERE reserved_by = NEW.id;
        END;
    ", elig = money_eligible("NEW")))
}

/// What a rebuild changed. A non-zero `added`/`removed` on a running wallet means the
/// index had drifted from `outputs` — logged at `warn`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MoneyIndexRebuild {
    pub free_before: usize,
    pub free_after: usize,
    pub added: usize,
    pub removed: usize,
}

/// Recompute `money_utxos` from `outputs` (the index is derived — contract §2, §7).
/// One SQLite transaction. Free rows are recomputed from scratch; a reserved row is kept
/// only while it is consistent (its output is `spendable=0`, `spent_by` = the reserver,
/// and the reserver is still in flight). Run at every open, by V27, and after a backup
/// import.
pub fn rebuild_money_index(conn: &Connection) -> Result<MoneyIndexRebuild> {
    let report = rebuild_money_index_quiet(conn)?;
    if report.added > 0 || report.removed > 0 {
        log::warn!("   ⚠️ Money index rebuilt with changes: {:?}", report);
    } else {
        info!("   ✅ Money index rebuilt: {} free coin(s), no drift", report.free_after);
    }
    Ok(report)
}

/// The rebuild without the drift log — for V27's first fill of an empty index, where
/// every row is "added" and none of it is drift.
fn rebuild_money_index_quiet(conn: &Connection) -> Result<MoneyIndexRebuild> {
    fn free_ids(c: &Connection) -> Result<std::collections::BTreeSet<i64>> {
        let mut stmt = c.prepare("SELECT output_id FROM money_utxos WHERE reserved_by IS NULL")?;
        let ids = stmt.query_map([], |r| r.get::<_, i64>(0))?.collect::<Result<_>>()?;
        Ok(ids)
    }

    let tx = conn.unchecked_transaction()?;
    let before = free_ids(&tx)?;
    tx.execute_batch(&format!("
        DELETE FROM money_utxos WHERE output_id NOT IN (SELECT outputId FROM outputs);
        DELETE FROM money_utxos WHERE reserved_by IS NULL;
        DELETE FROM money_utxos WHERE reserved_by IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM outputs o JOIN transactions t ON t.id = money_utxos.reserved_by
             WHERE o.outputId = money_utxos.output_id
               AND o.spendable = 0 AND o.spent_by = money_utxos.reserved_by
               AND t.status NOT IN ('unproven', 'completed', 'failed'));
        INSERT OR IGNORE INTO money_utxos (output_id, user_id, satoshis, reserved_by, created_at)
            SELECT o.outputId, o.user_id, o.satoshis, NULL, CAST(strftime('%s','now') AS INTEGER)
              FROM outputs o
             WHERE o.spendable = 1 AND {elig};
    ", elig = money_eligible("o")))?;
    let after = free_ids(&tx)?;
    tx.commit()?;

    Ok(MoneyIndexRebuild {
        free_before: before.len(),
        free_after: after.len(),
        added: after.difference(&before).count(),
        removed: before.difference(&after).count(),
    })
}

/// The six triggers [`install_money_index`] creates.
const MONEY_INDEX_TRIGGERS: [&str; 6] = [
    "money_utxos_on_output_insert",
    "money_utxos_on_output_spendable",
    "money_utxos_on_output_unspendable",
    "money_utxos_on_output_not_money",
    "money_utxos_on_output_delete",
    "money_utxos_on_tx_settled",
];

/// Install the money index, replacing triggers written by an older rule.
/// `install_money_index` uses `CREATE TRIGGER IF NOT EXISTS`, so a trigger body from a
/// V27 binary (no `change` term) would otherwise survive forever. Any trigger whose
/// stored SQL lacks the current eligibility text ⇒ all six are dropped and reinstalled.
/// Returns whether it reinstalled. Run by V28 and at every open.
pub fn ensure_money_index_current(conn: &Connection) -> Result<bool> {
    let want = money_eligible("NEW");
    let want: String = want.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut stale = false;
    for name in ["money_utxos_on_output_insert", "money_utxos_on_output_spendable", "money_utxos_on_output_not_money"] {
        let sql: Option<String> = conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = ?1", [name], |r| r.get(0),
        ).optional()?;
        match sql {
            Some(sql) if sql.split_whitespace().collect::<Vec<_>>().join(" ").contains(&want) => {}
            Some(_) => stale = true,
            None => {}
        }
    }
    if stale {
        for name in MONEY_INDEX_TRIGGERS {
            conn.execute_batch(&format!("DROP TRIGGER IF EXISTS {name};"))?;
        }
    }
    install_money_index(conn)?;
    Ok(stale)
}

/// V28 (B5-T1-P5, owner decisions 2 + 7, invariant 2 approved 2026-10-05): the money
/// index admits only rows the classification seam stamped `change = 1`.
///
/// Every existing `change` value is reset to 0 first: before P5 `change` meant "the
/// wallet built this as change", never "proven money", and a backup could carry any
/// value. The seam re-stamps what it proves (`classify::stamp_pending`, run at every
/// `TaskSyncPending` tick once the wallet is unlocked). Until then those coins are held
/// and **shown** (`/wallet/balance` `held`), never dropped from view. No column added.
pub fn migrate_v27_to_v28(conn: &Connection) -> Result<()> {
    let reset = conn.execute("UPDATE outputs SET change = 0 WHERE change != 0", [])?;
    ensure_money_index_current(conn)?;
    let r = rebuild_money_index_quiet(conn)?;
    info!("   ✅ V28 migration applied (money = stamped only; {} change mark(s) reset; index {} → {} free coin(s), re-stamped by the classifier)",
          reset, r.free_before, r.free_after);
    Ok(())
}

/// V29 (B5-T1-P7a-1, owner decision 1, invariant 2 approved 2026-10-09): the wallet's own verified
/// block-header chain above the compiled checkpoint (`header_chain.rs`). Derived data: never exported
/// or backed up, and every row is re-checked against every header rule at startup
/// (`chain_tracker::ChainTracker::load`), so a copied or planted row is never trusted.
/// `chainwork` is 32 bytes big-endian; `is_active` = on the most-work chain.
pub fn migrate_v28_to_v29(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS verified_headers (
            hash TEXT PRIMARY KEY,
            height INTEGER NOT NULL,
            prev_hash TEXT NOT NULL,
            header BLOB NOT NULL CHECK (length(header) = 80),
            chainwork BLOB NOT NULL CHECK (length(chainwork) = 32),
            is_active INTEGER NOT NULL CHECK (is_active IN (0, 1))
        );
        CREATE INDEX IF NOT EXISTS idx_verified_headers_height_active ON verified_headers(height, is_active);
        CREATE INDEX IF NOT EXISTS idx_verified_headers_prev ON verified_headers(prev_hash);",
    )?;
    info!("   ✅ V29 migration applied (verified_headers)");
    Ok(())
}

/// V30 (B5-T1-P7b, owner decision 1, invariant 2 approved 2026-10-10): PeerPay messages the wallet
/// cannot credit yet, stored BEFORE the MessageBox message is acknowledged — so no payment is lost
/// when it is deleted from the server. `body` is the decrypted message exactly as received (NULL only
/// for a row restored from a backup, which carries the derivation and txid alone). `next_check_at`
/// NULL = re-checked at wallet start; `lock_time` set = re-checked once our chain passes it.
pub fn migrate_v29_to_v30(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS peerpay_held_messages (
            message_id TEXT PRIMARY KEY,
            sender TEXT NOT NULL,
            derivation_prefix TEXT,
            derivation_suffix TEXT,
            txid TEXT,
            reason TEXT NOT NULL,
            held_at INTEGER NOT NULL,
            lock_time INTEGER,
            next_check_at INTEGER,
            attempts INTEGER NOT NULL DEFAULT 0,
            checked_at INTEGER,
            body BLOB
        );
        CREATE INDEX IF NOT EXISTS idx_peerpay_held_sender ON peerpay_held_messages(sender);",
    )?;
    info!("   ✅ V30 migration applied (peerpay_held_messages)");
    Ok(())
}

/// V31 (B5-T5-P7a; invariant 2 approved by the owner 2026-10-15): per outbox row, the MessageBox hosts
/// that took the message (`delivered_hosts`, a JSON array; NULL = none) and the exact request posted
/// (`message_request`; NULL for a row written before V31 — prepared again at its next attempt).
/// ⭐ DEVELOPER NOTE (owner, 2026-10-15). Hodos posts one PeerPay message to SEVERAL MessageBox servers
/// because finding out which server a wallet reads is not reliable yet: few wallets publish an
/// `ls_messagebox` advertisement (BSV Browser cannot; BSV Desktop only on request), the lookup hosts
/// disagree (2026-10-14: Babbage's had none of the owner's three), and the servers are separate
/// mailboxes. So the outbox remembers which servers took each message and re-posts the SAME message
/// (same id) only to the ones that did not. Once discovery matures — most recipients advertised,
/// our own advertisement published (B5-T5-P7 Part B) — most sends go to one host and this per-host
/// record can be simplified.
pub fn migrate_v30_to_v31(conn: &Connection) -> Result<()> {
    let cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(peerpay_outbox)")?;
        let cols = stmt.query_map([], |row| row.get::<_, String>(1))?.collect::<Result<Vec<_>>>()?;
        cols
    };
    if !cols.iter().any(|c| c == "delivered_hosts") {
        conn.execute("ALTER TABLE peerpay_outbox ADD COLUMN delivered_hosts TEXT", [])?;
    }
    if !cols.iter().any(|c| c == "message_request") {
        conn.execute("ALTER TABLE peerpay_outbox ADD COLUMN message_request BLOB", [])?;
    }
    info!("   ✅ V31 migration applied (peerpay_outbox per-host delivery)");
    Ok(())
}

/// V27 (B5-T1-P3, owner decision 2a + kickoff Q5): the money index.
pub fn migrate_v26_to_v27(conn: &Connection) -> Result<()> {
    install_money_index(conn)?;
    let r = rebuild_money_index_quiet(conn)?;
    info!("   ✅ V27 migration applied (money index filled: {} free coin(s))", r.free_after);
    Ok(())
}
