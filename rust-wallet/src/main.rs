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
mod beef_helpers;  // NEW: BEEF building helpers for listOutputs
mod database;  // NEW: Database module
mod utxo_sync;  // NEW: Background UTXO sync service
mod cache_errors;  // NEW: Unified error types for caching
mod cache_helpers;  // NEW: Helper functions for cache operations
mod cache_sync;  // NEW: Background cache sync service
mod balance_cache;  // NEW: In-memory balance cache
mod backup;  // NEW: Database backup and restore utilities
mod recovery;  // NEW: Wallet recovery from mnemonic
mod script;  // NEW: Bitcoin script parsing and PushDrop (BRC-48)
mod certificate;  // NEW: Certificate management (BRC-52)

// JSON storage no longer used - all handlers use database
use domain_whitelist::DomainWhitelistManager;
use message_relay::MessageStore;
use auth_session::AuthSessionManager;
use database::WalletDatabase;  // NEW: Import WalletDatabase
use std::sync::Arc;

// Global app state
pub struct AppState {
    pub database: Arc<Mutex<WalletDatabase>>,  // Database storage (primary)
    pub whitelist: Arc<DomainWhitelistManager>,
    pub message_store: MessageStore,
    pub auth_sessions: Arc<AuthSessionManager>,
    pub balance_cache: Arc<balance_cache::BalanceCache>,  // NEW: In-memory balance cache
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
    let database = match WalletDatabase::new(db_path.clone()) {
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


            Arc::new(Mutex::new(db))
        }
        Err(e) => {
            eprintln!("❌ Failed to initialize database: {}", e);
            eprintln!("   Database path: {}", db_path.display());
            eprintln!("   Continuing with JSON storage only...");
            // Continue without database for now (backward compatibility)
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Database init failed: {}", e)));
        }
    };

    // Clone database for background sync (before moving into app_state)
    let database_for_sync = database.clone();

    // Initialize balance cache
    let balance_cache = Arc::new(balance_cache::BalanceCache::new());
    println!("✅ Balance cache initialized");

    // Create app state
    let app_state = web::Data::new(AppState {
        database,  // Database is the only storage now
        whitelist: whitelist_manager,
        message_store,
        auth_sessions,
        balance_cache,
    });

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

    // Start background UTXO sync task
    println!("🔄 Starting background UTXO sync service...");
    utxo_sync::start_background_sync(database_for_sync);
    println!("   ✅ Background sync will run every {} seconds", utxo_sync::SYNC_INTERVAL_SECONDS);
    println!();

    // Start background cache sync service
    println!("🔄 Starting background BEEF cache sync service...");
    let app_state_for_cache = app_state.clone();
    tokio::spawn(async move {
        cache_sync::start_cache_sync_service(app_state_for_cache).await;
    });
    println!("   ✅ Cache sync will run every 10 minutes");
    println!();

    // Start HTTP server
    HttpServer::new(move || {
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
            .route("/wallet/address/generate", web::post().to(handlers::generate_address))
            .route("/wallet/backup", web::post().to(handlers::wallet_backup))
            .route("/wallet/restore", web::post().to(handlers::wallet_restore))
            .route("/wallet/recover", web::post().to(handlers::wallet_recover))

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
    .run()
    .await
}
