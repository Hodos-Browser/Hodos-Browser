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
mod database;  // NEW: Database module

use json_storage::JsonStorage;
use action_storage::ActionStorage;  // NEW: Import ActionStorage
use domain_whitelist::DomainWhitelistManager;
use message_relay::MessageStore;
use auth_session::AuthSessionManager;
use database::WalletDatabase;  // NEW: Import WalletDatabase
use std::sync::Arc;

// Global app state
pub struct AppState {
    pub storage: Mutex<JsonStorage>,  // Keep for backward compatibility during transition
    pub action_storage: Mutex<ActionStorage>,  // Keep for backward compatibility during transition
    pub database: Arc<Mutex<WalletDatabase>>,  // NEW: Database storage (primary)
    pub whitelist: Arc<DomainWhitelistManager>,
    pub message_store: MessageStore,
    pub auth_sessions: Arc<AuthSessionManager>,
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

    let wallet_path = wallet_dir.join("wallet.json");
    println!("📁 Wallet path: {}", wallet_path.display());

    // Load wallet
    let storage = match JsonStorage::new(wallet_path.clone()) {
        Ok(s) => {
            let wallet = s.get_wallet().unwrap();
            println!("✅ Wallet loaded successfully");
            println!("   Addresses: {}", wallet.addresses.len());
            println!("   Current index: {}", wallet.current_index);
            println!("   Backed up: {}", wallet.backed_up);

            if let Ok(addr) = s.get_current_address() {
                println!("   Current address: {}", addr.address);
                println!("   Current pubkey: {}", addr.public_key);
            }
            s
        }
        Err(e) => {
            eprintln!("❌ Failed to load wallet: {}", e);
            eprintln!("   Expected path: {}", wallet_path.display());
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, e));
        }
    };

    // Initialize action storage (transaction history)
    let actions_path = wallet_path.parent().unwrap().join("actions.json");
    let action_storage = match ActionStorage::new(actions_path.clone()) {
        Ok(s) => {
            println!("✅ Action storage initialized");
            println!("   Actions path: {}", actions_path.display());
            println!("   Total actions: {}", s.count());
            s
        }
        Err(e) => {
            eprintln!("❌ Failed to initialize action storage: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
        }
    };

    // Initialize domain whitelist manager
    let whitelist_manager = Arc::new(DomainWhitelistManager::new());
    println!("✅ Domain whitelist manager initialized");

    // Initialize BRC-33 message relay
    let message_store = MessageStore::new();
    println!("✅ BRC-33 message relay initialized");

    // Initialize BRC-103/104 auth session manager
    let auth_sessions = Arc::new(AuthSessionManager::new());
    println!("✅ Auth session manager initialized");

    // Initialize database (Phase 1: Foundation)
    let db_path = wallet_path.parent().unwrap().join("wallet.db");
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
                    // No wallet in database - check if wallet.json exists to migrate
                    if wallet_path.exists() {
                        println!("🔑 No wallet in database, but wallet.json exists");
                        println!("   To migrate your existing wallet.json to the database:");
                        println!("   1. Uncomment the migration code in main.rs (around line 135)");
                        println!("   2. Or run the migration manually");
                        println!();
                        println!("   For now, creating a new wallet...");
                        match db.create_wallet_with_first_address() {
                            Ok((wallet_id, mnemonic, address)) => {
                                println!("   ✅ New wallet created!");
                                println!("   Wallet ID: {}", wallet_id);
                                println!("   First address: {}", address);
                                println!("   ⚠️  MNEMONIC (SAVE THIS SECURELY): {}", mnemonic);
                                println!();
                                println!("   ⚠️  NOTE: This is a NEW wallet. Your wallet.json is still intact.");
                                println!("   ⚠️  To migrate wallet.json, uncomment migration code in main.rs");
                            }
                            Err(e) => {
                                eprintln!("   ❌ Failed to create wallet: {}", e);
                            }
                        }
                    } else {
                        // No wallet.json either - create new wallet
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

    // Create app state
    let app_state = web::Data::new(AppState {
        storage: Mutex::new(storage),  // Keep for backward compatibility during transition
        action_storage: Mutex::new(action_storage),  // Keep for backward compatibility during transition
        database,  // Database is primary storage now
        whitelist: whitelist_manager,
        message_store,
        auth_sessions,
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
    println!("✅ Server ready - CEF browser can now connect!");
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
            .route("/createHmac", web::post().to(handlers::create_hmac))
            .route("/verifyHmac", web::post().to(handlers::verify_hmac))
            .route("/verifySignature", web::post().to(handlers::verify_signature))
            .route("/createSignature", web::post().to(handlers::create_signature))
            .route("/createAction", web::post().to(handlers::create_action))
            .route("/signAction", web::post().to(handlers::sign_action))
            .route("/processAction", web::post().to(handlers::process_action))
            .route("/abortAction", web::post().to(handlers::abort_action))
            .route("/listActions", web::post().to(handlers::list_actions))
            .route("/internalizeAction", web::post().to(handlers::internalize_action))
            .route("/updateConfirmations", web::post().to(handlers::update_confirmations_endpoint))  // NEW

            // Authentication endpoints
            .route("/.well-known/auth", web::post().to(handlers::well_known_auth))

            // Custom wallet endpoints
            .route("/wallet/status", web::get().to(handlers::wallet_status))
            .route("/wallet/balance", web::get().to(handlers::wallet_balance))
            .route("/wallet/address/generate", web::post().to(handlers::generate_address))

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
