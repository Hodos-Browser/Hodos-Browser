use actix_web::{web, App, HttpServer, middleware};
use actix_cors::Cors;
use std::path::PathBuf;
use std::sync::Mutex;

mod json_storage;
mod action_storage;  // NEW: Action storage module
mod handlers;
mod crypto;
mod transaction;
mod utxo_fetcher;
mod domain_whitelist;
mod message_relay;
mod auth_session;
mod beef;  // NEW: BEEF parser module
mod beef_helpers;  // BEEF building helpers for listOutputs
mod database;  // Database module
mod cache_errors;  // Unified error types for caching
mod cache_helpers;  // Helper functions for cache operations
mod balance_cache;  // In-memory balance cache
mod backup;  // Database backup and restore utilities
mod recovery;  // Wallet recovery + BIP32 legacy derivation (also in lib.rs for tests)
mod fee_rate_cache;  // Dynamic fee rate from ARC policy
mod monitor;  // Phase 6: Monitor pattern (background task scheduler)
mod script;  // Bitcoin script parsing and PushDrop (BRC-48)
mod certificate;  // Certificate management (BRC-52)

// JSON storage no longer used - all handlers use database
use domain_whitelist::DomainWhitelistManager;
use message_relay::MessageStore;
use auth_session::AuthSessionManager;
use database::WalletDatabase;  // NEW: Import WalletDatabase
use std::sync::Arc;
use std::collections::HashMap;

/// Info needed to re-derive a child private key for signing PushDrop inputs.
/// Populated by getPublicKey (forSelf=true), consumed by signAction.
#[derive(Debug, Clone)]
pub struct DerivedKeyInfo {
    pub invoice: String,              // BRC-43 invoice number (e.g., "2-todo tokens-1")
    pub counterparty_pubkey: Vec<u8>, // 33-byte compressed counterparty public key
}

// Global app state
pub struct AppState {
    pub database: Arc<Mutex<WalletDatabase>>,  // Database storage (primary)
    pub whitelist: Arc<DomainWhitelistManager>,
    pub message_store: MessageStore,
    pub auth_sessions: Arc<AuthSessionManager>,
    pub balance_cache: Arc<balance_cache::BalanceCache>,  // In-memory balance cache
    pub fee_rate_cache: Arc<fee_rate_cache::FeeRateCache>,  // ARC-sourced dynamic fee rate
    pub utxo_selection_lock: Arc<tokio::sync::Mutex<()>>,  // Prevents concurrent UTXO selection race conditions
    pub create_action_lock: Arc<tokio::sync::Mutex<()>>,  // Serializes entire createAction flow (select→sign→BEEF→broadcast)
    pub derived_key_cache: Arc<Mutex<HashMap<String, DerivedKeyInfo>>>,  // Maps derived pubkey hex → derivation params (for PushDrop signing)
    pub current_user_id: i64,  // Default user ID for all operations (multi-user foundation, Phase 3)
    pub shutdown: tokio_util::sync::CancellationToken,  // Graceful shutdown signal (Phase 8D)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("🦀 Bitcoin Browser Wallet (Rust)");
    println!("=================================");
    println!();

    // Get wallet path
    let appdata = std::env::var("APPDATA")
        .unwrap_or_else(|_| {
            println!("⚠️  APPDATA not set, using current directory");
            ".".to_string()
        });

    let wallet_dir = PathBuf::from(appdata)
        .join("HodosBrowser")
        .join("wallet");

    // Ensure wallet directory exists (needed for both JSON and database)
    if let Err(e) = std::fs::create_dir_all(&wallet_dir) {
        eprintln!("❌ Failed to create wallet directory: {}", e);
        eprintln!("   Path: {}", wallet_dir.display());
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, e));
    }

    // Database is now the primary storage - no JSON files needed
    println!("📁 Wallet directory: {}", wallet_dir.display());

    // Initialize domain whitelist manager
    let whitelist_manager = Arc::new(DomainWhitelistManager::new());
    println!("✅ Domain whitelist manager initialized");

    // Initialize BRC-33 message relay
    let message_store = MessageStore::new();
    println!("✅ BRC-33 message relay initialized");

    // Initialize BRC-103/104 auth session manager
    let auth_sessions = Arc::new(AuthSessionManager::new());
    println!("✅ Auth session manager initialized");

    // Initialize database (primary storage)
    let db_path = wallet_dir.join("wallet.db");
    let (database, default_user_id) = match WalletDatabase::new(db_path.clone()) {
        Ok(db) => {
            println!("✅ Database initialized");
            println!("   Database path: {}", db_path.display());

            // Test connection
            if let Err(e) = db.test_connection() {
                eprintln!("⚠️  Database connection test failed: {}", e);
            }

            // Check if wallet exists in database
            use database::{WalletRepository, AddressRepository};
            let wallet_repo = WalletRepository::new(db.connection());
            match wallet_repo.get_primary_wallet() {
                Ok(Some(wallet)) => {
                    println!("📋 Wallet found in database (ID: {})", wallet.id.unwrap());
                    println!("   Addresses: {}", wallet.current_index + 1);
                }
                Ok(None) => {
                    // No wallet in database - create new wallet
                    println!("🔑 No wallet in database - creating new wallet...");
                    match db.create_wallet_with_first_address() {
                        Ok((wallet_id, mnemonic, address)) => {
                            println!("   ✅ Wallet created!");
                            println!("   Wallet ID: {}", wallet_id);
                            println!("   First address: {}", address);
                            println!("   ⚠️  MNEMONIC (SAVE THIS SECURELY): {}", mnemonic);
                        }
                        Err(e) => {
                            eprintln!("   ❌ Failed to create wallet: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("   ⚠️  Error checking for wallet: {}", e);
                }
            }

            // Ensure master pubkey address exists (for existing wallets created before this feature)
            if let Err(e) = db.ensure_master_address_exists() {
                eprintln!("   ⚠️  Failed to ensure master address exists: {}", e);
            }

            // Ensure "default" basket exists (for existing wallets created before BRC-100 support)
            if let Err(e) = db.ensure_default_basket_exists() {
                eprintln!("   ⚠️  Failed to ensure default basket exists: {}", e);
            }

            // Cleanup stale pending transactions (created but never broadcast)
            // These occur when the process crashes between creating a transaction and broadcasting it.
            // Their change outputs are ghost outputs that don't exist on-chain.
            {
                use database::{TransactionRepository, OutputRepository};
                let conn = db.connection();
                let tx_repo = TransactionRepository::new(conn);

                // Find transactions stuck in 'unsigned' (never broadcast) for more than 5 minutes
                match tx_repo.get_stale_pending_transactions(300) {
                    Ok(stale_txs) if !stale_txs.is_empty() => {
                        println!("🧹 Found {} stale pending transaction(s) - cleaning up...", stale_txs.len());
                        let output_repo = OutputRepository::new(conn);

                        for (txid, inputs) in &stale_txs {
                            // 1. Delete ghost change outputs (outputs of the never-broadcast tx)
                            match output_repo.delete_by_txid(txid) {
                                Ok(count) if count > 0 => {
                                    println!("   🗑️  Deleted {} ghost output(s) from tx {}", count, &txid[..std::cmp::min(16, txid.len())]);
                                }
                                _ => {}
                            }

                            // 2. Restore input outputs that were marked as spent by this tx
                            match output_repo.restore_by_spending_description(txid) {
                                Ok(count) if count > 0 => {
                                    println!("   ♻️  Restored {} input output(s) from tx {}", count, &txid[..std::cmp::min(16, txid.len())]);
                                }
                                _ => {}
                            }

                            // 3. Mark the transaction as 'failed'
                            if let Err(e) = tx_repo.update_broadcast_status(txid, "failed") {
                                eprintln!("   ⚠️  Failed to update status for {}: {}", &txid[..std::cmp::min(16, txid.len())], e);
                            }

                            println!("   ✅ Cleaned up stale tx {} ({} inputs)", &txid[..std::cmp::min(16, txid.len())], inputs.len());
                        }
                        println!("   ✅ Stale transaction cleanup complete");
                    }
                    Ok(_) => {
                        // No stale transactions - normal case
                    }
                    Err(e) => {
                        eprintln!("   ⚠️  Failed to check for stale pending transactions: {}", e);
                    }
                }
            }

            // Restore any outputs with stale placeholder reservations.
            // This catches cases where the handler crashed between output reservation
            // and txid update (e.g., signing failure, deadlock, process kill).
            {
                use database::OutputRepository;
                let conn = db.connection();
                let output_repo = OutputRepository::new(conn);

                match output_repo.restore_pending_placeholders() {
                    Ok(count) if count > 0 => {
                        println!("♻️  Restored {} output(s) with stale placeholder reservations", count);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("   ⚠️  Failed to restore placeholder outputs: {}", e);
                    }
                }
            }

            // Get default user ID for AppState (multi-user foundation, Phase 3)
            let default_user_id: i64 = {
                use database::UserRepository;
                let conn = db.connection();
                let user_repo = UserRepository::new(conn);
                match user_repo.get_default() {
                    Ok(Some(user)) => {
                        let uid = user.user_id.unwrap_or(1);
                        println!("👤 Default user ID: {}", uid);
                        uid
                    }
                    Ok(None) => {
                        println!("⚠️  No default user found (new database without wallet)");
                        1  // Placeholder - will be created when wallet is created
                    }
                    Err(e) => {
                        eprintln!("   ⚠️  Failed to get default user: {}", e);
                        1  // Fallback to user ID 1
                    }
                }
            };

            (Arc::new(Mutex::new(db)), default_user_id)
        }
        Err(e) => {
            eprintln!("❌ Failed to initialize database: {}", e);
            eprintln!("   Database path: {}", db_path.display());
            eprintln!("   Continuing with JSON storage only...");
            // Continue without database for now (backward compatibility)
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Database init failed: {}", e)));
        }
    };

    // NOTE: Old background service clones removed in Phase 6I.
    // All background work is now handled by the Monitor pattern.
    let current_user_id = default_user_id;

    // Initialize balance cache and seed with current balance
    let balance_cache = Arc::new(balance_cache::BalanceCache::new());
    {
        let db = database.lock().unwrap();
        let output_repo = database::OutputRepository::new(db.connection());
        match output_repo.calculate_balance(current_user_id) {
            Ok(bal) => {
                balance_cache.set(bal);
                println!("✅ Balance cache initialized (seeded: {} satoshis)", bal);
            }
            Err(e) => {
                eprintln!("⚠️  Balance cache initialized (seed failed: {})", e);
            }
        }
    }

    // Initialize fee rate cache (fetches from ARC /v1/policy)
    let fee_rate_cache = Arc::new(fee_rate_cache::FeeRateCache::new());
    println!("✅ Fee rate cache initialized (ARC policy, 1-hour TTL)");

    // Create shutdown token for graceful shutdown (Phase 8D)
    let shutdown_token = tokio_util::sync::CancellationToken::new();

    // Create app state
    let app_state = web::Data::new(AppState {
        database,  // Database is the only storage now
        whitelist: whitelist_manager,
        message_store,
        auth_sessions,
        balance_cache,
        fee_rate_cache,
        utxo_selection_lock: Arc::new(tokio::sync::Mutex::new(())),  // Prevents concurrent UTXO selection
        create_action_lock: Arc::new(tokio::sync::Mutex::new(())),  // Serializes createAction end-to-end
        derived_key_cache: Arc::new(Mutex::new(HashMap::new())),  // PushDrop signing cache
        current_user_id,  // Multi-user foundation (Phase 3)
        shutdown: shutdown_token.clone(),  // Graceful shutdown signal (Phase 8D)
    });
    println!("✅ UTXO selection lock initialized");
    println!("✅ createAction serialization lock initialized");

    println!();
    println!("🌐 Starting HTTP server...");
    println!("   Port: 3301");
    println!("   URL: http://localhost:3301");
    println!();
    println!("📋 Available endpoints:");
    println!("   GET  /health");
    println!("   GET  /brc100/status");
    println!("   POST /getVersion");
    println!("   POST /getPublicKey");
    println!("   POST /isAuthenticated");
    println!("   POST /createHmac");
    println!("   POST /verifyHmac");
    println!("   POST /encrypt");
    println!("   POST /decrypt");
    println!("   POST /verifySignature");
    println!("   POST /.well-known/auth");
    println!("   GET  /wallet/status");
    println!("   GET  /wallet/balance");
    println!("   POST /wallet/sync");
    println!();
    println!("📬 BRC-33 Message Relay endpoints:");
    println!("   POST /sendMessage");
    println!("   POST /listMessages");
    println!("   POST /acknowledgeMessage");
    println!();
    println!("📊 Blockchain Query endpoints (Group C - Part 2):");
    println!("   POST /getHeight");
    println!("   POST /getHeaderForHeight");
    println!("   POST /getNetwork");
    println!();
    println!("📊 Blockchain Query endpoints:");
    println!("   POST /getHeight");
    println!("   POST /getHeaderForHeight");
    println!("   POST /getNetwork");
    println!();
    println!("✅ Server ready - CEF browser can now connect!");
    println!();

    // Wire up Ctrl+C signal handler for graceful shutdown (Phase 8D)
    let signal_token = shutdown_token.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!();
        println!("🛑 Ctrl+C received, shutting down gracefully...");
        signal_token.cancel();
    });

    // Start Monitor — the sole background task scheduler (Phase 6 complete)
    // Replaces: arc_status_poller, cache_sync, utxo_sync background services
    println!("🔄 Starting Monitor (background task scheduler)...");
    monitor::Monitor::start(app_state.clone());
    println!("   ✅ Monitor started with 7 tasks");
    println!();

    // Start HTTP server with graceful shutdown support (Phase 8D)
    let server = HttpServer::new(move || {
        // Configure CORS (allow all for development)
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .app_data(app_state.clone())
            .app_data(web::JsonConfig::default()
                .limit(10 * 1024 * 1024)  // 10MB limit for BEEF transactions
                .error_handler(|err, _req| {
                    // Custom JSON error handler to ensure proper error responses
                    let error_msg = err.to_string();
                    log::error!("   JSON deserialization error: {}", error_msg);
                    actix_web::error::InternalError::from_response(
                        err,
                        actix_web::HttpResponse::BadRequest().json(serde_json::json!({
                            "error": format!("Invalid JSON request: {}", error_msg)
                        }))
                    ).into()
                }))
            .app_data(web::PayloadConfig::new(100 * 1024 * 1024))  // 100MB limit for web::Bytes
            .wrap(cors)
            .wrap(middleware::Logger::new("%a \"%r\" %s %b \"%{Referer}i\" %T"))

            // Health check
            .route("/health", web::get().to(handlers::health))
            .route("/brc100/status", web::get().to(handlers::brc100_status))

            // BRC-100 standard endpoints
            .route("/getVersion", web::post().to(handlers::get_version))
            .route("/getVersion", web::get().to(handlers::get_version))
            .route("/getPublicKey", web::post().to(handlers::get_public_key))
            .route("/isAuthenticated", web::post().to(handlers::is_authenticated))
            .route("/waitForAuthentication", web::post().to(handlers::wait_for_authentication))  // BRC-100 Call Code 24
            .route("/createHmac", web::post().to(handlers::create_hmac))
            .route("/verifyHmac", web::post().to(handlers::verify_hmac))
            .route("/encrypt", web::post().to(handlers::encrypt))
            .route("/decrypt", web::post().to(handlers::decrypt))
            .route("/verifySignature", web::post().to(handlers::verify_signature))
            .route("/createSignature", web::post().to(handlers::create_signature))
            // createAction needs large payload support for inputBEEF (100MB limit)
            .service(
                web::resource("/createAction")
                    .app_data(web::PayloadConfig::new(100 * 1024 * 1024))
                    .route(web::post().to(handlers::create_action))
            )
            // signAction also needs large payload support (100MB limit)
            .service(
                web::resource("/signAction")
                    .app_data(web::PayloadConfig::new(100 * 1024 * 1024))
                    .route(web::post().to(handlers::sign_action))
            )
            .route("/processAction", web::post().to(handlers::process_action))
            .route("/abortAction", web::post().to(handlers::abort_action))
            .route("/listActions", web::post().to(handlers::list_actions))
            .route("/internalizeAction", web::post().to(handlers::internalize_action))
            .route("/updateConfirmations", web::post().to(handlers::update_confirmations_endpoint))  // NEW
            .route("/listOutputs", web::post().to(handlers::list_outputs))  // Group C - Part 1
            .route("/relinquishOutput", web::post().to(handlers::relinquish_output))  // Group C - Part 1

            // Part 2: Blockchain Queries
            .route("/getHeight", web::post().to(handlers::get_height))  // Group C - Part 2
            .route("/getHeaderForHeight", web::post().to(handlers::get_header_for_height))  // Group C - Part 2
            .route("/getNetwork", web::post().to(handlers::get_network))  // Group C - Part 2

            // Part 3: Certificate Management
            .route("/acquireCertificate", web::post().to(handlers::acquire_certificate))  // Group C - Part 3
            .route("/listCertificates", web::post().to(handlers::list_certificates))  // Group C - Part 3
            .route("/proveCertificate", web::post().to(handlers::prove_certificate))  // Group C - Part 3
            .route("/relinquishCertificate", web::post().to(handlers::relinquish_certificate))  // Group C - Part 3
            .route("/discoverByIdentityKey", web::post().to(handlers::discover_by_identity_key))  // Group C - Part 4
            .route("/discoverByAttributes", web::post().to(handlers::discover_by_attributes))  // Group C - Part 4

            // Authentication endpoints
            .route("/.well-known/auth", web::post().to(handlers::well_known_auth))

            // Custom wallet endpoints
            .route("/wallet/status", web::get().to(handlers::wallet_status))
            .route("/wallet/balance", web::get().to(handlers::wallet_balance))
            .route("/wallet/sync", web::post().to(handlers::wallet_sync))
            .route("/wallet/address/generate", web::post().to(handlers::generate_address))
            .route("/wallet/addresses", web::get().to(handlers::get_all_addresses))
            .route("/wallet/address/current", web::get().to(handlers::get_current_address))
            .route("/wallet/backup", web::post().to(handlers::wallet_backup))
            .route("/wallet/restore", web::post().to(handlers::wallet_restore))
            .route("/wallet/recover", web::post().to(handlers::wallet_recover))
            .route("/wallet/cleanup", web::post().to(handlers::wallet_cleanup))

            // Transaction endpoints
            .route("/transaction/send", web::post().to(handlers::send_transaction))

            // Domain whitelist endpoints
            .route("/domain/whitelist/check", web::get().to(handlers::check_domain))
            .route("/domain/whitelist/add", web::post().to(handlers::add_domain))

            // BRC-33 Message Relay endpoints
            .route("/sendMessage", web::post().to(handlers::send_message))
            .route("/listMessages", web::post().to(handlers::list_messages))
            .route("/acknowledgeMessage", web::post().to(handlers::acknowledge_message))
    })
    .bind(("127.0.0.1", 3301))?
    .run();

    // Spawn shutdown watcher that stops the HTTP server when Ctrl+C fires (Phase 8D)
    let server_handle = server.handle();
    tokio::spawn(async move {
        shutdown_token.cancelled().await;
        println!("🛑 Stopping HTTP server...");
        server_handle.stop(true).await; // graceful: finish in-flight requests
    });

    server.await
}
