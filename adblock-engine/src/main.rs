//! Hodos Adblock Engine — Standalone ad & tracker blocking service
//!
//! Separate process from the wallet backend. Runs on port 3302.
//! C++ starts this via CreateProcessA + Job Object (auto-kill on browser exit).
//!
//! Two-phase startup:
//! 1. HTTP server starts immediately (GET /health returns "loading")
//! 2. Engine loads async (deserialize engine.dat or download filter lists)
//! 3. Once ready, GET /health returns "ready"

#[allow(deprecated)]  // assemble_scriptlet_resources deprecated in newer adblock, correct for v0.10.3
mod engine;
mod handlers;

use actix_web::{web, App, HttpServer};
use std::path::PathBuf;

const ADBLOCK_PORT: u16 = 3302;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("Hodos Adblock Engine");
    println!("====================");
    println!();

    // Resolve storage directory
    let adblock_dir = resolve_adblock_dir();
    println!("Storage: {}", adblock_dir.display());

    // Create engine in "loading" state
    let engine = engine::AdblockEngine::new(adblock_dir);
    let engine_data = web::Data::new(engine);

    // Clone for the background init task
    let engine_for_init = engine_data.clone();

    println!();
    println!("Starting HTTP server on port {}...", ADBLOCK_PORT);

    // Start HTTP server (responds to /health with "loading" immediately)
    let server = HttpServer::new(move || {
        App::new()
            .app_data(engine_data.clone())
            .route("/health", web::get().to(handlers::health))
            .route("/check", web::post().to(handlers::check))
            .route("/status", web::get().to(handlers::status))
            .route("/toggle", web::post().to(handlers::toggle))
            .route("/cosmetic-resources", web::post().to(handlers::cosmetic_resources))
            .route("/cosmetic-hidden-ids", web::post().to(handlers::cosmetic_hidden_ids))
    })
    .workers(2)
    .bind(("127.0.0.1", ADBLOCK_PORT))?
    .run();

    // Clone for the background update task
    let engine_for_update = engine_for_init.clone();

    // Spawn engine initialization in background (downloads lists on first run)
    tokio::spawn(async move {
        println!("Loading adblock engine...");
        match engine_for_init.load().await {
            Ok(()) => {
                let status = engine_for_init.get_status();
                println!(
                    "Ad blocker ready: {} lists, {} rules",
                    status.list_count, status.total_rules
                );
            }
            Err(e) => {
                log::error!("Ad blocker init failed: {}", e);
                eprintln!("Ad blocker init failed: {} — blocking disabled", e);
            }
        }
    });

    // Spawn background filter list auto-update task (checks every 6 hours)
    tokio::spawn(async move {
        use tokio::time::{interval, Duration};
        const UPDATE_INTERVAL_SECS: u64 = 6 * 3600; // 6 hours

        // Wait for initial load to complete before starting update checks
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut tick = interval(Duration::from_secs(UPDATE_INTERVAL_SECS));
        tick.tick().await; // consume the immediate first tick

        loop {
            tick.tick().await;

            if engine_for_update.get_engine_status() != engine::EngineStatus::Ready {
                continue;
            }

            if !engine_for_update.needs_update() {
                log::info!("Ad blocker: filter lists still fresh, skipping update");
                continue;
            }

            log::info!("Ad blocker: filter lists expired, downloading updates...");
            match engine_for_update.rebuild_engine().await {
                Ok(()) => {
                    let status = engine_for_update.get_status();
                    log::info!(
                        "Ad blocker: updated to version {} ({} lists, {} rules)",
                        status.update_version, status.list_count, status.total_rules
                    );
                }
                Err(e) => {
                    log::error!("Ad blocker: update failed: {} — continuing with old lists", e);
                }
            }
        }
    });

    println!("Server listening on http://127.0.0.1:{}", ADBLOCK_PORT);
    println!();

    server.await
}

/// Resolve the adblock storage directory.
///
/// Windows: %APPDATA%/HodosBrowser/adblock/
/// macOS:   ~/Library/Application Support/HodosBrowser/adblock/
/// Fallback: ./adblock/
fn resolve_adblock_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata)
                .join("HodosBrowser")
                .join("adblock");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("HodosBrowser")
                .join("adblock");
        }
    }

    // Fallback for dev/testing
    PathBuf::from(".")
        .join("adblock")
}
