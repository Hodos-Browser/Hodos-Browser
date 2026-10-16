//! Hodos Adblock Engine — Standalone ad & tracker blocking service
//!
//! Separate process from the wallet backend. Runs on port 31302.
//! C++ starts this via CreateProcessA + Job Object (auto-kill on browser exit).
//!
//! Two-phase startup:
//! 1. HTTP server starts immediately (GET /health returns "loading")
//! 2. Engine loads async (compile the cached lists, or download them on a first start)
//! 3. Once ready, GET /health returns "ready"

#[allow(deprecated)]  // assemble_scriptlet_resources deprecated in newer adblock, correct for v0.10.3
mod engine;
mod handlers;
mod js_scan;
mod preparser;
mod ubo_resources;

use actix_web::{web, App, HttpServer};
use std::path::PathBuf;

/// Adblock HTTP port. Dev builds (`HODOS_DEV=1`) bind 31402 so the dev browser
/// and the INSTALLED browser (31302) can run simultaneously. Gated on the SAME
/// `HODOS_DEV` condition as `app_dir_name()`; a release build always uses 31302.
fn adblock_port() -> u16 {
    match std::env::var("HODOS_DEV").as_deref() {
        Ok("1") => 31402,
        _ => 31302,
    }
}

fn app_dir_name() -> &'static str {
    match std::env::var("HODOS_DEV").as_deref() {
        Ok("1") => "HodosBrowserDev",
        _ => "HodosBrowser",
    }
}

/// Safeguard: if running from a cargo build directory (dev build), require HODOS_DEV=1.
fn enforce_dev_safeguard() {
    let exe = std::env::current_exe().unwrap_or_default();
    let exe_str = exe.to_string_lossy();
    let is_dev_build = exe_str.contains("target\\release")
        || exe_str.contains("target\\debug")
        || exe_str.contains("target/release")
        || exe_str.contains("target/debug");
    if is_dev_build && std::env::var("HODOS_DEV").as_deref() != Ok("1") {
        eprintln!("========================================================");
        eprintln!("  DEV SAFEGUARD: HODOS_DEV=1 is not set!");
        eprintln!("  Running a dev build without it would use the");
        eprintln!("  production database and risk corrupting real data.");
        eprintln!();
        eprintln!("  Use the launcher script instead:");
        eprintln!("    PowerShell: .\\dev-adblock.ps1");
        eprintln!("    Mac/Linux:  ./dev-adblock.sh");
        eprintln!("========================================================");
        std::process::exit(1);
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    enforce_dev_safeguard();

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
    println!("Starting HTTP server on port {}...", adblock_port());

    // Start HTTP server (responds to /health with "loading" immediately)
    let server = HttpServer::new(move || {
        App::new()
            .app_data(engine_data.clone())
            .route("/health", web::get().to(handlers::health))
            .route("/shutdown", web::post().to(handlers::shutdown))
            .route("/check", web::post().to(handlers::check))
            .route("/status", web::get().to(handlers::status))
            .route("/toggle", web::post().to(handlers::toggle))
            .route("/cosmetic-resources", web::post().to(handlers::cosmetic_resources))
            .route("/cosmetic-hidden-ids", web::post().to(handlers::cosmetic_hidden_ids))
    })
    .workers(2)
    .bind(("127.0.0.1", adblock_port()))?
    .run();

    // Clone for the background update task
    let engine_for_update = engine_for_init.clone();

    // Spawn engine initialization in background (downloads lists on first run)
    tokio::spawn(async move {
        println!("Loading adblock engine...");
        // load() runs in its own task so a panic inside adblock is caught as a failure too,
        // not a dead init task that leaves the status at Loading forever.
        let loader = engine_for_init.clone();
        let outcome = match tokio::spawn(async move { loader.load().await }).await {
            Ok(result) => result,
            Err(e) => Err(format!("load panicked: {}", e)),
        };
        match outcome {
            Ok(()) => {
                let status = engine_for_init.get_status();
                println!(
                    "Ad blocker ready: {} lists, {} rules",
                    status.list_count, status.total_rules
                );
                // Ready on what was on disk; now fetch uBO's pinned scriptlets if missing.
                engine_for_init.refresh_scriptlets().await;
            }
            Err(e) => {
                log::error!("Ad blocker init failed: {}", e);
                eprintln!("Ad blocker init failed: {} — retrying on the next update check", e);
                engine_for_init.mark_load_failed();
            }
        }
    });

    // Spawn background filter list auto-update task (first check ~60 s after start, then hourly)
    tokio::spawn(run_update_schedule(move || {
        let engine_for_update = engine_for_update.clone();
        async move {
            match engine_for_update.get_engine_status() {
                engine::EngineStatus::Ready => {}
                // The first load failed (e.g. offline first start): try it again.
                engine::EngineStatus::Error => {
                    log::info!("Ad blocker: retrying the initial load");
                    let loader = engine_for_update.clone();
                    let outcome = match tokio::spawn(async move { loader.load().await }).await {
                        Ok(result) => result,
                        Err(e) => Err(format!("load panicked: {}", e)),
                    };
                    match outcome {
                        Ok(()) => engine_for_update.refresh_scriptlets().await,
                        Err(e) => {
                            log::warn!("Ad blocker: load retry failed: {}", e);
                            engine_for_update.mark_load_failed();
                        }
                    }
                    return;
                }
                engine::EngineStatus::Loading => return,
            }

            // Retry uBO's pinned scriptlets if a fetch failed earlier (no-op once running).
            engine_for_update.refresh_scriptlets().await;

            if !engine_for_update.needs_update() {
                log::info!("Ad blocker: filter lists still fresh, skipping update");
                return;
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
    }));

    println!("Server listening on http://127.0.0.1:{}", adblock_port());
    println!();

    server.await
}

/// Delay before the first update check — time for the initial load to finish.
const FIRST_UPDATE_CHECK_SECS: u64 = 60;
/// Interval between update checks after the first.
/// Hourly: a check is in-memory (`needs_update`), and a rebuild downloads only expired lists,
/// so quick-fixes (12 h expiry) refreshes within an hour of expiring instead of up to 6 h late.
const UPDATE_INTERVAL_SECS: u64 = 3600;

// A list that failed to refresh must be retried by the NEXT check, so its first retry window is
// shorter than the interval (/code-review 171 #7: equal windows meant a ~2 h retry). Checked at
// compile time.
const _: () = assert!(engine::RETRY_AFTER_FAILURE_SECS < UPDATE_INTERVAL_SECS);

/// Run `check` FIRST_UPDATE_CHECK_SECS after start, then every UPDATE_INTERVAL_SECS.
///
/// The first check must not wait a full interval: `load()` serves a cached engine without
/// looking at list expiry, so a browser closed before 6 h of uptime would never refresh
/// (B5-T0-P6, P6-A8).
async fn run_update_schedule<F, Fut>(mut check: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    use tokio::time::{interval, sleep, Duration};

    sleep(Duration::from_secs(FIRST_UPDATE_CHECK_SECS)).await;

    let mut tick = interval(Duration::from_secs(UPDATE_INTERVAL_SECS));
    loop {
        tick.tick().await; // the first tick completes immediately
        check().await;
    }
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
                .join(app_dir_name())
                .join("adblock");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join(app_dir_name())
                .join("adblock");
        }
    }

    // Fallback for dev/testing
    PathBuf::from(".")
        .join("adblock")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    /// P6-A8: the first update check runs ~60 s after start, not after a full interval,
    /// and later checks keep the interval.
    #[tokio::test(start_paused = true)]
    async fn test_update_schedule_first_check_soon_after_start() {
        let checks = Arc::new(AtomicUsize::new(0));
        let c = checks.clone();
        tokio::spawn(run_update_schedule(move || {
            let c = c.clone();
            async move { c.fetch_add(1, Ordering::SeqCst); }
        }));
        let at = |n: usize, label: &str| assert_eq!(checks.load(Ordering::SeqCst), n, "{}", label);

        sleep(Duration::from_secs(FIRST_UPDATE_CHECK_SECS - 1)).await;
        at(0, "no check before the first delay");
        sleep(Duration::from_secs(2)).await;
        at(1, "one check just after the first delay — not 6 h later");
        sleep(Duration::from_secs(UPDATE_INTERVAL_SECS - 2)).await;
        at(1, "no second check before the interval");
        sleep(Duration::from_secs(4)).await;
        at(2, "second check one interval after the first");
    }
}
