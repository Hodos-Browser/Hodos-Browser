use actix_web::{web, App, HttpServer, middleware, Error};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::body::{BoxBody, MessageBody};
use actix_cors::Cors;
use std::path::PathBuf;
use std::sync::Mutex;

mod arc_status;  // Centralized ARC miner response status classification
mod json_storage;
mod action_storage;  // NEW: Action storage module
mod handlers;
mod crypto;
mod transaction;
mod utxo_fetcher;
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
mod price_cache;  // BSV/USD exchange rate cache
mod monitor;  // Phase 6: Monitor pattern (background task scheduler)
mod script;  // Bitcoin script parsing and PushDrop (BRC-48)
mod certificate;  // Certificate management (BRC-52)
mod authfetch;  // BRC-103 AuthFetch client for authenticated HTTP requests
mod messagebox;  // MessageBox API client with BRC-2 encryption
mod paymail;  // Paymail (bsvalias) client for human-readable address resolution
mod identity_resolver;  // Identity resolution via BSV Overlay Services (BRC-52 certificates)
mod overlay;  // BSV Overlay Services client for certificate publish/unpublish
mod services;  // Phase 1.6d.B: WalletServices facade — IndexerProvider trait + provider chains
mod permission_service;  // Phase 2.6-A.5: wrapper around hodos_permission_engine pure crate (dormant in A.5; wired into AppState in A.6)
mod manifest;  // Phase 2.6-G: Rust port of C++ ManifestFetcher (fetch + lenient parse of .well-known/wallet-manifest.json)
mod reconcile;  // Wallet-Hardening WS1: spent-input reconcile primitives (c1 check_outpoint_spent; c2/c3 dormant)

// Re-export for monitor tasks (avoids rust-analyzer resolution issues when only lib is checked)
pub use cache_helpers::verify_tsc_proof_against_block;

/// Phase 2.6-G — universal domain-trust gate (actix middleware).
///
/// Runs ahead of every wallet route. Internal calls (no `X-Requesting-Domain`)
/// pass straight through — that header is injected by the C++ layer ONLY for
/// external (dApp) origins, so its absence means the wallet UI is calling its
/// own backend and domain-trust doesn't apply. External origins are gated by
/// `domain_trust_gate`:
///   - approved → Proceed (handler runs; its per-handler kind gate, if any,
///     runs after — this middleware never touches `X-User-Approved`)
///   - blocked  → 403
///   - unknown  → 202 (`domain_approval` / `manifest_connect_bundle`). C++ opens
///     the connect modal, writes trust=approved synchronously on Approve, and
///     re-issues; the re-issue sees "approved" here and Proceeds.
///
/// This single choke-point makes Rust authoritative for domain trust across
/// BOTH transports (IPC shim + direct-fetch `Open()`), per the Phase 2.6 goal:
/// "every API surface that talks to Rust automatically gets the engine."
async fn domain_trust_mw(
    req: ServiceRequest,
    next: middleware::Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let domain = req
        .request()
        .headers()
        .get("X-Requesting-Domain")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // R-INTEXT's SUBJECT, made observable.
    //
    // The standing regression set (development-docs/0.4.0-beta.3/REGRESSION_SET.md) asserts
    // on "the Rust log: ABSENCE of X-Requesting-Domain for an internal call, PRESENCE with
    // the exact page host for an external one", and says reading the C++ side alone proves
    // nothing. That was not satisfiable: nothing here logged it, so the sprint's
    // load-bearing invariant could not actually be run at a phase boundary.
    //
    // DEBUG, so it never reaches a user (the wallet ships at warn). Host only -- never the
    // path or query -- matching the browser side's LogSafeUrl rule, because this line
    // records which site the user is talking to.
    log::debug!(
        "R-INTEXT trust: path={} requesting_domain={}",
        req.request().path(),
        domain.as_deref().unwrap_or("<none:internal>")
    );

    // Internal call → no gate.
    let domain = match domain {
        Some(d) => d,
        None => return Ok(next.call(req).await?.map_into_boxed_body()),
    };

    // ⛔ THE PERMISSION TABLE IS FIRST-PARTY ONLY. (P0.5 §4k)
    //
    // MEASURED 2026-08-19 from a REAL approved dApp: a page-context
    // `POST /domain/permissions` carrying an arbitrary
    // `{domain, perTxLimitCents, identityKeyDisclosureAllowed}` body succeeded
    // SILENTLY and wrote the row — no modal, no notification. `set_domain_permission`
    // has no gate of its own, and this middleware used to wave approved domains
    // straight through. So ONE ordinary "approve this dApp" click — the most common
    // action in the product — handed that site the ability to raise its own spending
    // caps without limit, switch on silent identity-key disclosure, and approve
    // collaborator domains the user never visited.
    //
    // Why "has the header" is exactly "came from a web page": every legitimate
    // writer of this table is first-party and reaches Rust header-free. The C++
    // modal-approval writes use `SyncHttpClient::Post` / `CefRequest` with
    // Content-Type ONLY (verified at every `/domain/permissions*` and
    // `/wallet/session-*` call site in cef-native), and the wallet UI's own calls go
    // down the internal IPC path, which builds its header map from scratch.
    // `X-Requesting-Domain` is stamped solely on the dApp-request forwarding paths.
    //
    // ⛔ Gated HERE, once, over the WHOLE subtree — deliberately not per handler. A
    // sub-permission endpoint added later is refused by default instead of by
    // someone remembering. Gating arm-by-arm is exactly how the `send_transaction`
    // IPC arm was missed and how this phase came to be refuted.
    {
        let raw_path = req.request().path().to_string();

        // ⛔ MATCH THE PATH ACTIX ROUTES ON, NOT THE ONE IT REPORTS. (P0.5 panel #2, 1.3)
        //
        // actix-router 0.5.3 percent-DECODES the path before matching, while
        // `HttpRequest::path()` hands back the RAW request target. The first cut of
        // this gate compared the raw path, so `POST /domain/%70ermissions` was routed
        // to `set_domain_permission` while the gate saw a path it did not recognise
        // and waved it through. ONE character defeated the whole control.
        //
        // MEASURED 2026-08-20 against pinned actix-web 4.11.0, from an APPROVED
        // scratch domain:
        //   RED  `POST /domain/permissions`   + X-Requesting-Domain → 403, row unchanged
        //   BUG  `POST /domain/%70ermissions` + X-Requesting-Domain → 200, row REWRITTEN:
        //        perTxLimitCents 50 → 999999, perSessionLimitCents 100 → 999999,
        //        identityKeyDisclosureAllowed false → true.
        //
        // Both forms are matched below. Decoding here rather than re-deriving a
        // matcher keeps the gate and the router on ONE decoder — the same rule
        // CLAUDE.md states for `RegistrableDomainFromUrl`, where two independent
        // derivations of the same value fail closed and silently.
        let decoded_path = percent_encoding::percent_decode_str(&raw_path)
            .decode_utf8_lossy()
            .into_owned();

        let method = req.request().method().clone();
        let is_mutation = method == actix_web::http::Method::POST
            || method == actix_web::http::Method::DELETE;

        // ⛔ SUBTREES, NOT A LIST OF STRINGS. (P0.5 panel #2, 1.4)
        //
        // The previous revision enumerated three exact paths and missed
        // `/wallet/session/close`, which drops the whole per-browser counter entry —
        // per-session dollar cap, max-tx-per-session AND rate limit — so an approved
        // dApp reset its own spending limits on demand and spent without bound.
        // MEASURED 2026-08-20: pay(4c) → 500, pay → 202 session_cap, pay → 202
        // (control), page-origin `POST /wallet/session/close` → 200, pay → 500.
        //
        // It was missed even though the comment right above it warned that
        // enumerating instead of gating a subtree is exactly how the original hole
        // survived. So: match PREFIXES. Every route under `/domain/` and every route
        // under `/wallet/session` is permission, trust or session state (verified
        // against the full route table 2026-08-20 — 17 and 3 routes respectively).
        // A sub-permission or session endpoint added later is refused by default
        // instead of by someone remembering to extend a list.
        fn is_permission_surface(path: &str) -> bool {
            path.starts_with("/domain/")
                || path.starts_with("/wallet/session")
                // ⛔ SECOND CLASS: not the permission TABLE, but the same trust
                // property — first-party only. (Panel #2 Task 2, owner-approved
                // 2026-08-21.) Both handlers take `(state, body)` with NO
                // `HttpRequest` parameter, so — exactly like `sign_action` — they
                // structurally cannot gate themselves. The gate has to be here.
                //
                // `/wallet/reveal-mnemonic` returns the BIP39 RECOVERY PHRASE. On
                // the no-PIN branch it hands it over whenever the wallet is
                // unlocked (DPAPI/Keychain auto-unlock at startup means: always),
                // after one Allow click. That is key exfiltration of the whole
                // wallet, including funds not yet received, and it contradicts
                // CLAUDE.md Invariant #1's "never reachable from web content".
                //
                // `/wallet/settings` rewrites `default_per_tx/per_session/rate` —
                // the limits EVERY FUTURE approval inherits — plus
                // `default_identity_key_disclosure_allowed`. Same escalation as
                // §4k without needing the encoding trick: a site that can no
                // longer raise its own caps could still raise the defaults that
                // every later grant copies.
                //
                // MEASURED 2026-08-21 with these two arms removed, from an
                // APPROVED dApp origin:
                //   RED  POST /wallet/reveal-mnemonic -> 401 — it REACHED the
                //        handler; only the PIN stopped it, and the no-PIN branch
                //        has no PIN to stop it with.
                //   RED  POST /wallet/settings        -> 200 and the global
                //        defaults were REWRITTEN: per_tx 1000 -> 999999,
                //        per_session 5000 -> 999999.
                // With them in place both return 403 and neither handler runs,
                // while the header-free first-party path is untouched (401 on a
                // wrong PIN = the handler still runs for the wallet UI).
                || path == "/wallet/reveal-mnemonic"
                || path == "/wallet/settings"
                // ⛔ THIRD CLASS: the /wallet/debug subtree. (P0.5 panel re-run.)
                // These are developer/repair tools that move funds and rewrite
                // output state — `debug_broadcast_nosend` broadcasts a local tx
                // and re-marks its inputs spent, `debug_repair_nosend` rewrites
                // status. They are registered UNCONDITIONALLY (main.rs, no
                // HODOS_DEV gate) and none takes an `HttpRequest`, so they cannot
                // gate themselves. MEASURED 2026-08-21: from an APPROVED dApp
                // origin, `POST /wallet/debug/broadcast-nosend` reached the
                // handler (404 on a bogus txid) — a dApp has no business calling
                // a debug endpoint at all. A SUBTREE prefix closes all three
                // (validate-beef / repair-nosend / broadcast-nosend) and any
                // debug endpoint added later, by default; the header-free
                // first-party path (developer tooling / wallet UI) is untouched.
                || path.starts_with("/wallet/debug")
                // ⛔ FOURTH CLASS: no-HttpRequest wallet-management fund-movers /
                // destructive ops with NO legitimate dApp use. (P0.5 panel
                // re-run confirmation — the completeness critic found the
                // /wallet/debug fix left its own siblings open.) Each takes
                // `(state, _body)` with no `HttpRequest`, so — like the debug and
                // reveal-mnemonic handlers — it cannot gate itself, and each is
                // reachable from an approved dApp via the wallet_call bridge:
                //   /wallet/delete           — DELETES the wallet (HIGH; backs up
                //                              first, but an unprompted destructive
                //                              action a dApp must never trigger)
                //   /wallet/consolidate-dust — broadcasts a consolidation tx
                //                              (1000 sats to treasury + fee)
                //   /wallet/backup*          — broadcasts an on-chain backup tx
                //   /wallet/recover*         — wallet recovery / external sweep
                //   /wallet/broadcast-nosend — force-finalizes a held nosend tx
                // The sibling `wallet_export` DOES take `HttpRequest` and rejects
                // X-Requesting-Domain — proof this is the intended pattern and
                // these were omissions. Every internal/scheduled caller is
                // header-free (task_consolidate_dust::run_inner and
                // wallet_delete's do_onchain_backup are direct fn calls;
                // task_backup's POST /wallet/backup/onchain and the BRC-121
                // BroadcastTask's POST /wallet/broadcast-nosend both send only
                // Content-Type, no X-Requesting-Domain), so the first-party path
                // is untouched. Subtrees, not exact strings.
                || path.starts_with("/wallet/delete")
                || path.starts_with("/wallet/consolidate-dust")
                || path.starts_with("/wallet/backup")
                || path.starts_with("/wallet/recover")
                || path.starts_with("/wallet/broadcast-nosend")
        }
        let hits_surface =
            is_permission_surface(&raw_path) || is_permission_surface(&decoded_path);

        // Carve-out: a site may revoke ITSELF. (P0.5 panel #2, 1.5)
        //
        // `window.yours.disconnect()` / `window.panda.disconnect()` are page-context
        // JS in cef-native's own injected shim (`CWIShimScript.h`, the `disconnect`
        // legacy method) issuing `DELETE /domain/permissions?domain=<own host>`. The
        // gate above 403s it, silently breaking a shipped API — 81c054c's "verified
        // at every call site" audit missed it because it looked only at C++ call
        // sites, not at the JS that C++ injects.
        //
        // Permitted because it moves privilege DOWN: §4k exists to stop a site
        // ESCALATING, and revoking is the opposite. Narrow on purpose — DELETE only,
        // that one path only, and the target domain must equal the requesting domain,
        // so a site can drop its own grant and nobody else's.
        let is_self_revoke = method == actix_web::http::Method::DELETE
            && decoded_path == "/domain/permissions"
            && percent_encoding::percent_decode_str(req.request().query_string())
                .decode_utf8_lossy()
                .split('&')
                .any(|kv| kv.strip_prefix("domain=").is_some_and(|v| v == domain));

        if is_mutation && hits_surface && !is_self_revoke {
            log::warn!(
                "🛡️ REFUSED {} {} (decoded '{}') from dApp origin '{}' — first-party only",
                method, raw_path, decoded_path, domain
            );
            return Ok(req.into_response(
                actix_web::HttpResponse::Forbidden().json(serde_json::json!({
                    // Kept as-is: this string is the discriminator the §4k and
                    // panel-#2 evidence rows assert on, and renaming it would
                    // silently invalidate those tests.
                    "error": "permission_table_is_first_party_only",
                    "endpoint": decoded_path,
                })),
            ));
        }
    }

    let permission = req
        .app_data::<web::Data<Arc<permission_service::PermissionService>>>()
        .expect("PermissionService Data registered in App")
        .get_ref()
        .clone();
    let database = req
        .app_data::<web::Data<Arc<Mutex<WalletDatabase>>>>()
        .expect("WalletDatabase Data registered in App")
        .get_ref()
        .clone();
    let user_id = req
        .app_data::<web::Data<AppState>>()
        .map(|s| s.current_user_id)
        .unwrap_or(1);
    let endpoint = req.request().path().to_string();

    match permission_service::domain_trust_gate(&permission, &database, user_id, &domain, &endpoint).await {
        permission_service::GateOutcome::Proceed => {
            Ok(next.call(req).await?.map_into_boxed_body())
        }
        // 202 connect prompt / 403 blocked — short-circuit, handler never runs.
        permission_service::GateOutcome::EarlyReturn(resp) => Ok(req.into_response(resp)),
    }
}

use auth_session::AuthSessionManager;
use database::WalletDatabase;  // NEW: Import WalletDatabase
use std::sync::Arc;
use std::collections::HashMap;

fn app_dir_name() -> &'static str {
    match std::env::var("HODOS_DEV").as_deref() {
        Ok("1") => "HodosBrowserDev",
        _ => "HodosBrowser",
    }
}

/// Wallet HTTP port. Dev builds (`HODOS_DEV=1`) bind 31401 so the dev browser
/// and the INSTALLED browser (which uses 31301) can run simultaneously without
/// fighting over the port. Gated on the SAME `HODOS_DEV` condition as
/// `app_dir_name()` — a release build never sets the env var, so it always uses
/// 31301. Used by the bind below AND by `monitor::task_backup`'s self-call
/// (via `crate::wallet_port()`) so the dev monitor never POSTs the prod wallet.
pub fn wallet_port() -> u16 {
    match std::env::var("HODOS_DEV").as_deref() {
        Ok("1") => 31401,
        _ => 31301,
    }
}

/// Cross-platform base data directory for this app instance.
/// `<dirs::data_dir()>/<app_dir_name()>` — e.g.
/// Windows dev: `%APPDATA%\HodosBrowserDev`, macOS: `~/Library/Application Support/HodosBrowser`.
/// Subdirs `wallet/` (db) and `logs/` (rotating log files) hang off this root.
fn data_root() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| match std::env::var("APPDATA") {
            Ok(appdata) => PathBuf::from(appdata),
            Err(_) => PathBuf::from("."),
        })
        .join(app_dir_name())
}

/// Initialize logging: every `log::` record goes to a rotating file in
/// `<data_root>/logs/` AND is duplicated to stderr (so the dev terminal is
/// unchanged). Production installs get logs too — essential for supporting a
/// wallet that moves real money. Level honors `RUST_LOG` (default `info`).
/// `WriteMode::Direct` flushes each record so the file can be tailed live.
/// Returns the `LoggerHandle`, which MUST be kept alive for the process
/// lifetime (dropping it stops logging/flushing).
fn init_logging() -> flexi_logger::LoggerHandle {
    let logs_dir = data_root().join("logs");
    let _ = std::fs::create_dir_all(&logs_dir);
    // Option A (privacy-conscious prod default): dev logs at `info` (full
    // operational detail for debugging); production logs at `warn` so installed
    // users don't accumulate a local on-disk trail of every domain/payment at
    // info level. `RUST_LOG` overrides either default. Revisited holistically in
    // development-docs/0.4.0/HelicOps/LOGGING_REVIEW_AND_UPGRADE.md.
    let default_level = if app_dir_name() == "HodosBrowserDev" { "info" } else { "warn" };
    flexi_logger::Logger::try_with_env_or_str(default_level)
        .expect("flexi_logger: invalid RUST_LOG spec")
        .log_to_file(
            flexi_logger::FileSpec::default()
                .directory(&logs_dir)
                .basename("wallet"),
        )
        .duplicate_to_stderr(flexi_logger::Duplicate::Info)
        .rotate(
            flexi_logger::Criterion::Size(10_000_000), // 10 MB per file
            flexi_logger::Naming::Numbers,
            flexi_logger::Cleanup::KeepLogFiles(10), // ~100 MB ceiling
        )
        .write_mode(flexi_logger::WriteMode::Direct)
        .format_for_files(flexi_logger::detailed_format)
        .start()
        .expect("flexi_logger: failed to start")
}

/// Dev/prod deconfliction guard — runs FIRST in main(), before any HODOS_DEV read,
/// so any correction takes effect everywhere downstream (`app_dir_name`,
/// `wallet_port`, `keychain_service` all re-read the env var each call). Two rules:
///
///  1. A dev BUILD (running from `target/{release,debug}`) MUST have HODOS_DEV=1,
///     or it would silently hit the PRODUCTION data directory — refuse.
///  2. HODOS_DEV=1 is ONLY legitimate from a dev-build path. If an installed or
///     portable (non-dev-build) binary is launched with a stray HODOS_DEV=1 — e.g.
///     a developer left it set in their shell/user environment — scrub it so this
///     process (and any child that inherits its env) uses the PRODUCTION namespace
///     instead of silently opening the DEV wallet. This is the "namespace-flip"
///     defense from the dev/prod deconfliction audit (gap C1/Mode-B); it closes the
///     installed-app AND portable-anywhere case in one rule without needing to know
///     where prod is installed.
fn enforce_dev_safeguard() {
    let exe = std::env::current_exe().unwrap_or_default();
    let exe_str = exe.to_string_lossy();
    let is_dev_build = exe_str.contains("target\\release")
        || exe_str.contains("target\\debug")
        || exe_str.contains("target/release")
        || exe_str.contains("target/debug");
    let dev_flag = std::env::var("HODOS_DEV").as_deref() == Ok("1");

    // Rule 1: dev build without the flag → refuse (would corrupt prod data).
    if is_dev_build && !dev_flag {
        eprintln!("========================================================");
        eprintln!("  DEV SAFEGUARD: HODOS_DEV=1 is not set!");
        eprintln!("  Running a dev build without it would use the");
        eprintln!("  production database and risk corrupting real data.");
        eprintln!();
        eprintln!("  Use the launcher script instead:");
        eprintln!("    PowerShell: .\\dev-wallet.ps1");
        eprintln!("    Mac/Linux:  ./dev-wallet.sh");
        eprintln!("========================================================");
        std::process::exit(1);
    }

    // Rule 2 (Mode-B): stray HODOS_DEV on a non-dev-build binary → force prod.
    if !is_dev_build && dev_flag {
        eprintln!("========================================================");
        eprintln!("  DEV/PROD GUARD: HODOS_DEV=1 is set but this is NOT a dev");
        eprintln!("  build ({}).", exe_str);
        eprintln!("  Ignoring the stray flag and using the PRODUCTION namespace");
        eprintln!("  so the installed wallet never opens dev data.");
        eprintln!("========================================================");
        std::env::remove_var("HODOS_DEV");
    }
}

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
    pub auth_sessions: Arc<AuthSessionManager>,
    pub balance_cache: Arc<balance_cache::BalanceCache>,  // In-memory balance cache
    pub fee_rate_cache: Arc<fee_rate_cache::FeeRateCache>,  // ARC-sourced dynamic fee rate
    pub price_cache: Arc<price_cache::PriceCache>,  // BSV/USD exchange rate (WhatsOnChain + CoinGecko + MEXC; persisted to V21 bsv_price_cache)
    pub services: Arc<services::WalletServices>,  // Phase 1.6d.B: WalletServices facade (dormant — 1.6d.C wires call sites)
    pub ship_cache: Arc<overlay::ship_cache::ShipDiscoveryCache>,  // Phase 1.6d polish Step 1: SWR cache for SHIP host discovery
    pub utxo_selection_lock: Arc<tokio::sync::Mutex<()>>,  // Prevents concurrent UTXO selection race conditions
    pub create_action_lock: Arc<tokio::sync::Mutex<()>>,  // Serializes entire createAction flow (select→sign→BEEF→broadcast)
    pub derived_key_cache: Arc<Mutex<HashMap<String, DerivedKeyInfo>>>,  // Maps derived pubkey hex → derivation params (for PushDrop signing)
    pub current_user_id: i64,  // Default user ID for all operations (multi-user foundation, Phase 3)
    pub shutdown: tokio_util::sync::CancellationToken,  // Graceful shutdown signal (Phase 8D)
    pub sync_status: Arc<std::sync::RwLock<handlers::SyncStatus>>,  // Recovery sync progress
    pub backup_check_needed: Arc<Mutex<Option<(i64, i64)>>>,  // (first_event_ts, latest_event_ts) — backup runs 3 min after latest, hard cap 10 min from first
    pub recovery_just_completed: Arc<std::sync::atomic::AtomicBool>,  // Set after on-chain recovery — triggers immediate TaskCheckForProofs + TaskValidateUtxos
    pub pay402_reuse: Arc<Mutex<HashMap<(String, i64), handlers::Pay402ReuseEntry>>>,  // (URL, sats) → unbroadcast retry context, ~25s TTL — see pay_402
    pub permission: Arc<permission_service::PermissionService>,  // Phase 2.6-A.6: actix wrapper around hodos_permission_engine pure crate (dormant — all 5 flags default OFF)
}

impl AppState {
    /// Signal that a significant wallet event occurred and backup should be checked soon.
    /// The monitor task will check the DB hash after a 3-minute delay from the LATEST event.
    ///
    /// Timer behavior:
    /// - First event: sets timestamp, 3-minute countdown starts
    /// - Subsequent events: resets timestamp to now (extends the 3-min window)
    /// - Hard cap: 10 minutes from the FIRST event (prevents infinite deferral)
    ///
    /// The tuple is (first_event_ts, latest_event_ts). Monitor checks: now - latest >= 180.
    pub fn request_backup_check(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if let Ok(mut guard) = self.backup_check_needed.lock() {
            match *guard {
                None => {
                    // First event — start the countdown
                    *guard = Some((now, now));
                    log::info!("   📋 Backup check requested — will run in ~3 minutes");
                }
                Some((first_ts, _)) => {
                    // Subsequent event — reset timer but respect 10-min cap from first event
                    let cap = first_ts + 600; // 10-minute hard cap
                    if now + 180 <= cap {
                        // Still within cap — extend the 3-min window from now
                        *guard = Some((first_ts, now));
                        let remaining = cap - now;
                        log::info!("   📋 Backup timer extended (new event) — hard cap in {}s", remaining);
                    } else {
                        // Past the cap — don't push further, let it fire
                        log::info!("   📋 Backup timer at cap — will fire on next check");
                    }
                }
            }
        }
    }

    /// Check if a backup should be triggered based on the "soon" flag and USD threshold.
    /// Call this from event handlers after significant financial events.
    pub fn request_backup_check_if_significant(&self, satoshis: i64) {
        // Convert satoshis to USD using cached price (price_cache returns f64 in USD, e.g. 13.55)
        let price_usd = self.price_cache.get_cached()
            .or_else(|| self.price_cache.get_stale());
        if let Some(price) = price_usd {
            let usd_value = (satoshis as f64 / 100_000_000.0) * price;
            if usd_value >= 3.0 {
                self.request_backup_check();
            }
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Dev safeguard runs FIRST — before any logging or data-dir creation — so a
    // mis-launched dev build (HODOS_DEV unset) bails via eprintln! before we
    // touch the production data directory.
    enforce_dev_safeguard();

    // Initialize logging: console + rotating file in <data_root>/logs/.
    // Held for the process lifetime (drop = logging stops). Flushed explicitly in
    // the OD-2 graceful-exit sequence before std::process::exit(0).
    let logger = init_logging();

    log::info!("🦀 Bitcoin Browser Wallet (Rust)");
    log::info!("=================================");

    if app_dir_name() == "HodosBrowserDev" {
        log::info!("🔧 DEV MODE: Using HodosBrowserDev data directory");
    }

    // Get wallet path (cross-platform) — shares the data_root() resolver with logs.
    let wallet_dir = data_root().join("wallet");

    // Ensure wallet directory exists (needed for both JSON and database)
    if let Err(e) = std::fs::create_dir_all(&wallet_dir) {
        log::error!("❌ Failed to create wallet directory: {}", e);
        log::warn!("   Path: {}", wallet_dir.display());
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, e));
    }

    // Database is now the primary storage - no JSON files needed
    log::info!("📁 Wallet directory: {}", wallet_dir.display());

    // Initialize BRC-103/104 auth session manager
    let auth_sessions = Arc::new(AuthSessionManager::new());
    log::info!("✅ Auth session manager initialized");

    // Initialize database (primary storage)
    let db_path = wallet_dir.join("wallet.db");
    let (database, default_user_id, wallet_exists) = match WalletDatabase::new(db_path.clone()) {
        Ok(mut db) => {
            log::info!("✅ Database initialized");
            log::info!("   Database path: {}", db_path.display());

            // Test connection
            if let Err(e) = db.test_connection() {
                log::warn!("⚠️  Database connection test failed: {}", e);
            }

            // Check if wallet exists in database
            use database::{WalletRepository, AddressRepository};
            let wallet_repo = WalletRepository::new(db.connection());
            let wallet_exists = match wallet_repo.get_primary_wallet() {
                Ok(Some(wallet)) => {
                    log::info!("📋 Wallet found in database (ID: {})", wallet.id.unwrap());
                    log::info!("   Addresses: {}", wallet.current_index + 1);

                    let wallet_id = wallet.id.unwrap();
                    let has_dpapi = wallet.mnemonic_dpapi.is_some();
                    let is_pin_protected = wallet.pin_salt.is_some();

                    // Auto-unlock via DPAPI (Windows user account binding)
                    match db.try_dpapi_unlock() {
                        Ok(true) => {
                            log::info!("🔓 DPAPI auto-unlock succeeded");
                        }
                        Ok(false) => {
                            // No DPAPI blob — legacy wallet or non-Windows
                            if !is_pin_protected {
                                // Legacy unencrypted wallet: cache mnemonic directly
                                log::info!("🔓 Legacy wallet (no PIN, no DPAPI) — caching plaintext mnemonic");
                                db.cache_mnemonic(wallet.mnemonic.clone());
                            } else {
                                // PIN-protected but no DPAPI blob — backfill if we can unlock
                                // This case shouldn't happen for new wallets but handles
                                // wallets created before DPAPI support was added
                                log::info!("🔒 PIN-protected wallet without DPAPI blob — wallet locked");
                                log::info!("   Use POST /wallet/unlock with PIN to unlock");
                            }
                        }
                        Err(e) => {
                            // DPAPI blob exists but decryption failed (DB moved to another machine/user)
                            log::info!("🔒 DPAPI unlock failed: {} — wallet locked", e);
                            log::info!("   Use POST /wallet/unlock with PIN to unlock");
                        }
                    }

                    // If wallet was unlocked (via DPAPI or legacy), do startup tasks
                    if db.is_unlocked() {
                        // Backfill DPAPI blob for wallets that don't have one yet
                        if !has_dpapi {
                            if let Ok(mnemonic) = db.get_cached_mnemonic() {
                                let mnemonic_owned = mnemonic.to_string();
                                if let Err(e) = db.store_dpapi_blob(wallet_id, &mnemonic_owned) {
                                    log::error!("   Auto-unlock repair failed ({}): {} — the wallet will still ask for a PIN next start", "startup", e);
                                }
                            }
                        }

                        // Ensure master pubkey address exists (needs cached mnemonic)
                        if let Err(e) = db.ensure_master_address_exists() {
                            log::warn!("   ⚠️  Failed to ensure master address exists: {}", e);
                        }

                        // Ensure backup address exists (needs cached mnemonic)
                        if let Err(e) = db.ensure_backup_address_exists() {
                            log::warn!("   ⚠️  Failed to ensure backup address exists: {}", e);
                        }
                    }
                    true
                }
                Ok(None) => {
                    log::info!("🔑 No wallet in database - server ready for user-initiated creation");
                    false
                }
                Err(e) => {
                    log::warn!("   ⚠️  Error checking for wallet: {}", e);
                    false
                }
            };

            if wallet_exists {

                // Ensure "default" basket exists (for existing wallets created before BRC-100 support)
                if let Err(e) = db.ensure_default_basket_exists() {
                    log::warn!("   ⚠️  Failed to ensure default basket exists: {}", e);
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
                            log::info!("🧹 Found {} stale pending transaction(s) - cleaning up...", stale_txs.len());
                            let output_repo = OutputRepository::new(conn);

                            for (txid, inputs) in &stale_txs {
                                // 1. Delete ghost change outputs (outputs of the never-broadcast tx)
                                match output_repo.delete_by_txid(txid) {
                                    Ok(count) if count > 0 => {
                                        log::info!("   🗑️  Deleted {} ghost output(s) from tx {}", count, &txid[..std::cmp::min(16, txid.len())]);
                                    }
                                    _ => {}
                                }

                                // 2. Restore input outputs that were marked as spent by this tx
                                match output_repo.restore_by_spending_description(txid) {
                                    Ok(count) if count > 0 => {
                                        log::info!("   ♻️  Restored {} input output(s) from tx {}", count, &txid[..std::cmp::min(16, txid.len())]);
                                    }
                                    _ => {}
                                }

                                // 3. Mark the transaction as 'failed'
                                if let Err(e) = tx_repo.update_broadcast_status(txid, "failed") {
                                    log::warn!("   ⚠️  Failed to update status for {}: {}", &txid[..std::cmp::min(16, txid.len())], e);
                                }

                                log::info!("   ✅ Cleaned up stale tx {} ({} inputs)", &txid[..std::cmp::min(16, txid.len())], inputs.len());
                            }
                            log::info!("   ✅ Stale transaction cleanup complete");
                        }
                        Ok(_) => {
                            // No stale transactions - normal case
                        }
                        Err(e) => {
                            log::warn!("   ⚠️  Failed to check for stale pending transactions: {}", e);
                        }
                    }
                }

                // Stale `pending-%` placeholder reservations (handler crashed or the
                // process was killed between reserving outputs and resolving the
                // placeholder to a real txid) are NOT released here.
                //
                // This used to be an unconditional blanket
                // `UPDATE ... WHERE spending_description LIKE 'pending-%'` with no age
                // filter and no on-chain check. That can un-spend an output whose
                // transaction actually broadcast -- every path resolves placeholder ->
                // real txid before broadcasting, but that update's failure is only a
                // warning and the broadcast still proceeds -- which makes the wallet
                // re-offer an already-spent output (`R-NODOUBLE`, a double-spend).
                //
                // Release now belongs to `monitor::task_sweep_reservations`, which runs
                // on the first monitor tick after startup and proves each outpoint is
                // still unspent on-chain before releasing it. The in-process case is
                // covered earlier still, by the `ReservationGuard` scope guard in
                // `create_action_internal`.

                // Clean up interrupted backup transactions.
                // If a backup tx was created but the process was killed before broadcast
                // completed, ghost outputs and stale reservations may remain.
                // With the post-broadcast output creation fix, only the placeholder
                // reservation should remain (handled above). This catches legacy cases
                // where outputs were created before broadcast.
                {
                    use database::OutputRepository;
                    use database::TransactionRepository;
                    let conn = db.connection();

                    let mut stmt = conn.prepare(
                        "SELECT id, txid FROM transactions \
                         WHERE status = 'sending' AND description = 'On-chain wallet backup'"
                    ).ok();
                    if let Some(ref mut stmt) = stmt {
                        let stuck_backups: Vec<(i64, String)> = stmt.query_map([], |row| {
                            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                        }).ok()
                            .map(|rows| rows.filter_map(|r| r.ok()).collect())
                            .unwrap_or_default();

                        if !stuck_backups.is_empty() {
                            let output_repo = OutputRepository::new(conn);
                            let tx_repo = TransactionRepository::new(conn);
                            for (_tx_id, txid) in &stuck_backups {
                                // Standard ghost cleanup: mark failed, delete outputs, restore inputs
                                let _ = tx_repo.set_transaction_status(
                                    txid,
                                    crate::action_storage::TransactionStatus::Failed,
                                );
                                let deleted = output_repo.delete_by_txid(txid).unwrap_or(0);
                                let restored = output_repo.restore_by_spending_description(txid).unwrap_or(0);
                                log::info!(
                                    "🧹 Cleaned up stuck backup tx {}: deleted {} ghost output(s), restored {} input(s)",
                                    &txid[..16.min(txid.len())], deleted, restored
                                );
                            }
                        }
                    }
                }

                // Restore backup outputs incorrectly marked stale by old adoption code
                {
                    let conn = db.connection();
                    let restored = conn.execute(
                        "UPDATE outputs SET spendable = 1, spending_description = NULL \
                         WHERE spending_description = 'stale-backup'",
                        [],
                    ).unwrap_or(0);
                    if restored > 0 {
                        log::info!("🧹 Restored {} backup output(s) incorrectly marked stale", restored);
                    }
                }

                // Fix certificates whose PushDrop token was consumed externally.
                // If publish_status='published' but the token output is external-spend,
                // the certificate can't be unpublished — reset to unpublished.
                {
                    let conn = db.connection();
                    let fixed = conn.execute(
                        "UPDATE certificates SET publish_status = 'unpublished', \
                         publish_txid = NULL, publish_vout = NULL \
                         WHERE publish_status = 'published' \
                         AND publish_txid IS NOT NULL \
                         AND EXISTS ( \
                             SELECT 1 FROM outputs \
                             WHERE outputs.txid = certificates.publish_txid \
                             AND outputs.vout = certificates.publish_vout \
                             AND outputs.spending_description = 'external-spend' \
                         )",
                        [],
                    ).unwrap_or(0);
                    if fixed > 0 {
                        log::info!("🧹 Reset {} certificate(s) whose publish token was spent externally", fixed);
                    }
                }

                // Fix master key outputs (index -1) that have NULL derivation.
                // These are service fee outputs synced from chain before the
                // derivation_prefix='master' fix. They need the prefix to be
                // selectable for spending. Exclude 546-sat outputs (orphaned
                // backup markers at index -3 that also have NULL derivation).
                {
                    let conn = db.connection();
                    let fixed = conn.execute(
                        "UPDATE outputs SET derivation_prefix = 'master', derivation_suffix = '-1' \
                         WHERE derivation_prefix IS NULL AND derivation_suffix IS NULL \
                         AND spendable = 1 AND satoshis != 546",
                        [],
                    ).unwrap_or(0);
                    if fixed > 0 {
                        log::info!("🧹 Tagged {} master-key output(s) with derivation prefix", fixed);
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
                        log::info!("👤 Default user ID: {}", uid);
                        uid
                    }
                    Ok(None) => {
                        log::info!("⚠️  No default user found (new database without wallet)");
                        1  // Placeholder - will be created when wallet is created
                    }
                    Err(e) => {
                        log::warn!("   ⚠️  Failed to get default user: {}", e);
                        1  // Fallback to user ID 1
                    }
                }
            };

            (Arc::new(Mutex::new(db)), default_user_id, wallet_exists)
        }
        Err(e) => {
            log::error!("❌ Failed to initialize database: {}", e);
            log::warn!("   Database path: {}", db_path.display());
            log::warn!("   Continuing with JSON storage only...");
            // Continue without database for now (backward compatibility)
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Database init failed: {}", e)));
        }
    };

    // NOTE: Old background service clones removed in Phase 6I.
    // All background work is now handled by the Monitor pattern.
    let current_user_id = default_user_id;

    // Initialize balance cache and seed with current balance (only if wallet exists)
    let balance_cache = Arc::new(balance_cache::BalanceCache::new());
    if wallet_exists {
        let db = database.lock().unwrap();
        let output_repo = database::OutputRepository::new(db.connection());
        match output_repo.calculate_balance(current_user_id) {
            Ok(bal) => {
                balance_cache.set(bal);
                log::info!("✅ Balance cache initialized (seeded: {} satoshis)", bal);
            }
            Err(e) => {
                log::warn!("⚠️  Balance cache initialized (seed failed: {})", e);
            }
        }
    }

    // Initialize fee rate cache (fetches from ARC /v1/policy)
    let fee_rate_cache = Arc::new(fee_rate_cache::FeeRateCache::new());
    log::info!("✅ Fee rate cache initialized (ARC policy, 1-hour TTL)");

    // Initialize BSV/USD price cache (2026-06-09: WhatsOnChain primary,
    // CoinGecko fallback, MEXC final fallback). Loads the last known good
    // price from `bsv_price_cache` SQLite table (V21) so cold-start has a
    // fallback when all three live sources are down.
    let price_cache = Arc::new(price_cache::PriceCache::new(Some(database.clone())));
    price_cache.load_persisted();
    log::info!("✅ Price cache initialized (BSV/USD, 5-min TTL, restart-survival via V21)");

    // Create shutdown token for graceful shutdown (Phase 8D)
    let shutdown_token = tokio_util::sync::CancellationToken::new();

    // Phase 1.6d.B: WalletServices facade — dormant infrastructure; 1.6d.C wires call sites
    let services = Arc::new(services::WalletServices::new());
    log::info!("✅ WalletServices facade initialized (dormant — 1.6d.C wires call sites)");

    // Phase 1.6d polish Step 1: SHIP discovery SWR cache.
    // Eliminates the ~75s blocking SHIP round-trip on every certificate
    // publish/unpublish. Kept warm by monitor::task_refresh_ship_cache.
    let ship_cache = overlay::ship_cache::ShipDiscoveryCache::new();
    log::info!("✅ SHIP discovery cache initialized (SWR: 5-min fresh, 30-min stale)");

    // Phase 2.6-A.6 / C.2: build permission_service.
    //
    // Phase 2.6-C dropped the per-class env-var fallback model (kickoff Q1) —
    // each CallKind class becomes Rust-authoritative the instant its sub-commit
    // lands, with no runtime opt-out. By 2.6-G every permission gate (domain
    // trust, payment, scoped-grant, cert disclosure, privacy perimeter) decides
    // in Rust; 2.6-H removed the last engine flag (shadow-log) and its handler.
    let permission = Arc::new(permission_service::PermissionService::new());
    log::info!("✅ Permission engine: all gates are Rust-authoritative (Phase 2.6)");

    // Create app state
    let app_state = web::Data::new(AppState {
        database,  // Database is the only storage now
        auth_sessions,
        balance_cache,
        fee_rate_cache,
        price_cache,
        services,
        ship_cache,
        utxo_selection_lock: Arc::new(tokio::sync::Mutex::new(())),  // Prevents concurrent UTXO selection
        create_action_lock: Arc::new(tokio::sync::Mutex::new(())),  // Serializes createAction end-to-end
        derived_key_cache: Arc::new(Mutex::new(HashMap::new())),  // PushDrop signing cache
        current_user_id,  // Multi-user foundation (Phase 3)
        shutdown: shutdown_token.clone(),  // Graceful shutdown signal (Phase 8D)
        sync_status: Arc::new(std::sync::RwLock::new(handlers::SyncStatus::default())),
        backup_check_needed: Arc::new(Mutex::new(None)),
        recovery_just_completed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        pay402_reuse: Arc::new(Mutex::new(HashMap::new())),
        permission,  // Phase 2.6-A.6: hodos_permission_engine actix wrapper (dormant)
    });
    log::info!("✅ UTXO selection lock initialized");
    log::info!("✅ createAction serialization lock initialized");

    log::info!("");
    log::info!("🌐 Starting HTTP server...");
    log::info!("   Port: {}", wallet_port());
    log::info!("   URL: http://localhost:{}", wallet_port());
    log::info!("");
    log::info!("📋 Available endpoints:");
    log::info!("   GET  /health");
    log::info!("   GET  /brc100/status");
    log::info!("   POST /getVersion");
    log::info!("   POST /getPublicKey");
    log::info!("   POST /isAuthenticated");
    log::info!("   POST /createHmac");
    log::info!("   POST /verifyHmac");
    log::info!("   POST /encrypt");
    log::info!("   POST /decrypt");
    log::info!("   POST /verifySignature");
    log::info!("   POST /.well-known/auth");
    log::info!("   GET  /wallet/status");
    log::info!("   GET  /wallet/balance");
    log::info!("   POST /wallet/sync");
    log::info!("");
    log::info!("📬 PeerPay (BRC-29) endpoints:");
    log::info!("   POST /wallet/peerpay/send");
    log::info!("   POST /wallet/peerpay/check");
    log::info!("   GET  /wallet/peerpay/status");
    log::info!("   POST /wallet/peerpay/dismiss");
    log::info!("");
    log::info!("💳 BRC-121 Simple HTTP 402 Payment:");
    log::info!("   POST /wallet/pay402");
    log::info!("   POST /wallet/broadcast-nosend");
    log::info!("");
    log::info!("📧 Paymail (bsvalias) endpoints:");
    log::info!("   POST /wallet/paymail/send");
    log::info!("   GET  /wallet/paymail/resolve");
    log::info!("");
    log::info!("📊 Blockchain Query endpoints (Group C - Part 2):");
    log::info!("   POST /getHeight");
    log::info!("   POST /getHeaderForHeight");
    log::info!("   POST /getNetwork");
    log::info!("");
    log::info!("📊 Blockchain Query endpoints:");
    log::info!("   POST /getHeight");
    log::info!("   POST /getHeaderForHeight");
    log::info!("   POST /getNetwork");
    log::info!("");
    log::info!("✅ Server ready - CEF browser can now connect!");
    log::info!("");

    // Wire up Ctrl+C signal handler for graceful shutdown (Phase 8D)
    let signal_token = shutdown_token.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        log::info!("");
        log::info!("🛑 Ctrl+C received, shutting down gracefully...");
        signal_token.cancel();
    });

    // OD-2 graceful-exit: capture the Monitor's JoinHandle (so the shutdown sequence
    // can quiesce it with a bounded join) and a DB handle (for the shutdown WAL
    // checkpoint), BEFORE app_state is moved into the HttpServer factory closure below.
    let mut monitor_handle: Option<tokio::task::JoinHandle<()>> = None;
    let db_for_shutdown = app_state.database.clone();

    // Start Monitor — the sole background task scheduler (Phase 6 complete)
    // Replaces: arc_status_poller, cache_sync, utxo_sync background services
    // Only start if wallet exists — no background work to do without a wallet
    if wallet_exists {
        // Startup phantom UTXO sweep: check outputs with transaction_id=NULL that claim
        // to be confirmed. These are externally-received outputs (PeerPay, address sync)
        // that may reference parent txs that were never mined. Before the April 13 fix,
        // store_derived_utxo() defaulted confirmed=1 on insert. This sweep catches any
        // historical ghosts by verifying each parent txid exists on WoC.
        {
            let sweep_state = app_state.clone();
            tokio::spawn(async move {
                // Brief delay to let the HTTP server start first
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                let candidates: Vec<(String, i32, i64)> = {
                    let db = match sweep_state.database.lock() {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                    let conn = db.connection();
                    let mut stmt = match conn.prepare(
                        "SELECT txid, vout, satoshis FROM outputs \
                         WHERE transaction_id IS NULL AND confirmed = 1 AND spendable = 1 \
                           AND txid IS NOT NULL"
                    ) {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                    stmt.query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?, row.get::<_, i64>(2)?))
                    }).ok()
                        .map(|rows| rows.filter_map(|r| r.ok()).collect())
                        .unwrap_or_default()
                }; // DB lock dropped

                if candidates.is_empty() {
                    return;
                }

                log::info!("🔍 Startup phantom sweep: checking {} externally-received output(s)...", candidates.len());

                let client = reqwest::Client::builder()
                    .timeout(crate::services::CallClass::IndexerSync.timeout())
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());

                // Deduplicate by txid (multiple outputs can share a parent tx)
                let mut checked_txids = std::collections::HashSet::new();
                let mut phantom_txids = std::collections::HashSet::new();

                for (txid, _vout, _sats) in &candidates {
                    if checked_txids.contains(txid) {
                        if phantom_txids.contains(txid) {
                            // Already confirmed phantom — will be cleaned up below
                        }
                        continue;
                    }
                    checked_txids.insert(txid.clone());

                    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/hash/{}", txid);
                    match client.get(&url).send().await {
                        Ok(resp) if resp.status().as_u16() == 404 => {
                            log::warn!(
                                "   🔴 Phantom output detected: tx {} not found on WoC (404)",
                                &txid[..std::cmp::min(16, txid.len())]
                            );
                            phantom_txids.insert(txid.clone());
                        }
                        Ok(resp) if resp.status().is_success() => {
                            log::info!(
                                "   ✅ Output tx {} verified on WoC",
                                &txid[..std::cmp::min(16, txid.len())]
                            );
                        }
                        Ok(resp) => {
                            log::warn!(
                                "   ⚠️  WoC returned {} for {} — skipping",
                                resp.status(), &txid[..std::cmp::min(16, txid.len())]
                            );
                        }
                        Err(e) => {
                            log::warn!(
                                "   ⚠️  WoC check failed for {}: {} — skipping",
                                &txid[..std::cmp::min(16, txid.len())], e
                            );
                        }
                    }
                    // Rate limit
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }

                // Clean up phantoms
                if !phantom_txids.is_empty() {
                    let db = match sweep_state.database.lock() {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                    let conn = db.connection();
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;

                    let mut total_cleaned = 0u32;
                    for txid in &phantom_txids {
                        let cleaned = conn.execute(
                            "UPDATE outputs SET spendable = 0, \
                             spending_description = 'phantom: parent tx not on chain (startup sweep)', \
                             updated_at = ?1 \
                             WHERE txid = ?2 AND spendable = 1",
                            rusqlite::params![now, txid],
                        ).unwrap_or(0) as u32;
                        if cleaned > 0 {
                            log::warn!(
                                "   🗑️  Marked {} phantom output(s) from tx {} as not spendable",
                                cleaned, &txid[..std::cmp::min(16, txid.len())]
                            );
                            total_cleaned += cleaned;
                        }
                    }

                    if total_cleaned > 0 {
                        drop(db); // Release lock before invalidating cache
                        sweep_state.balance_cache.invalidate();
                        log::info!(
                            "🧹 Startup phantom sweep complete: cleaned {} phantom output(s) from {} ghost tx(s)",
                            total_cleaned, phantom_txids.len()
                        );
                    }
                } else {
                    log::info!("✅ Startup phantom sweep: all externally-received outputs verified");
                }
            });
        }

        log::info!("🔄 Starting Monitor (background task scheduler)...");
        monitor_handle = monitor::Monitor::start(app_state.clone());
        log::info!("   ✅ Monitor started with 7 tasks");
        log::info!("");
    } else {
        log::info!("⏸️  Monitor skipped (no wallet yet)");
        log::info!("");
    }

    // Start HTTP server with graceful shutdown support (Phase 8D)
    let server = HttpServer::new(move || {
        // CORS: Only allow requests from our own frontend origins.
        // In production, CEF intercepts wallet requests at the C++ layer before they
        // reach Rust — CORS here is defense-in-depth against any bypass.
        // Website JS uses window.hodosBrowser.* (V8 IPC), never direct fetch to :31301.
        let cors = Cors::default()
            .allowed_origin("http://127.0.0.1:5137")
            .allowed_origin("http://localhost:5137")
            .allowed_origin("http://127.0.0.1")
            .allowed_origin("http://localhost")
            // P0.5 finding 4. These two are NOT a page-reachable origin — they are
            // the wallet's own base URL, which is what Chromium stamps on the POST
            // the C++ interceptor RE-ISSUES on a dApp's behalf: CORS overwrites
            // Origin with request_initiator->Serialize(), and CEF sets
            // request_initiator from the TARGET url. Without them,
            // block_on_origin_mismatch(true) below returns 400 before any handler
            // runs and every external dApp on @bsv/sdk WalletClient's default
            // transport breaks. MEASURED 2026-08-19: 400 with the flag on, 200 with
            // the one line removed.
            //
            // ⛔ Safe because `Origin` is a FORBIDDEN HEADER NAME — page JS cannot
            // set it, only the browser can, so a hostile page always gets its own
            // origin stamped and stays blocked. Both ports are listed because the
            // dev build runs on 31401 and the release build on 31301, and the value
            // is derived from whichever port THIS process bound.
            .allowed_origin("http://127.0.0.1:31301")
            .allowed_origin("http://127.0.0.1:31401")
            .allow_any_method()
            .allow_any_header()
            // P0.5-C1. Without this, actix-cors only omits the CORS response headers on
            // an origin mismatch — the browser hides the RESPONSE but the handler has
            // already RUN. For a wallet that means a cross-origin simple POST could still
            // move funds while the attacker simply never reads the reply. A blocked read
            // is not a blocked write. This makes the mismatch terminate the request
            // before it reaches a handler.
            .block_on_origin_mismatch(true)
            .max_age(3600);

        App::new()
            .app_data(app_state.clone())
            // Phase 2.6-B.2 — individual extractors for permission_service::handlers
            // (lives in lib.rs so it can't import the binary-only AppState struct).
            // Cloning the Arc fields is cheap; both AppState and the new Data<T>
            // entries point at the same backing objects.
            .app_data(web::Data::new(app_state.permission.clone()))
            .app_data(web::Data::new(app_state.database.clone()))
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
            // Phase 2.6-G — universal domain-trust gate. Runs per-request before
            // the route handler; external origins are trust-checked in Rust, the
            // single source of truth. Internal (wallet-UI) calls pass through.
            .wrap(middleware::from_fn(domain_trust_mw))

            // Health check
            .route("/health", web::get().to(handlers::health))
            .route("/shutdown", web::post().to(handlers::shutdown))
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
            // BRC-72 key linkage revelation (Phase 1.5 Step 1)
            .route("/revealCounterpartyKeyLinkage", web::post().to(handlers::reveal_counterparty_key_linkage))
            .route("/revealSpecificKeyLinkage", web::post().to(handlers::reveal_specific_key_linkage))
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
            .route("/wallet/tokens", web::get().to(handlers::list_token_outputs))  // Token list for wallet UI

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
            .route("/wallet/certificate/publish", web::post().to(handlers::publish_certificate))  // Certificate publish to overlay
            .route("/wallet/certificate/unpublish", web::post().to(handlers::unpublish_certificate))  // Certificate unpublish from overlay
            .route("/wallet/certificate/cleanup", web::post().to(handlers::cleanup_overlay_certificates))  // Cleanup stale certs on overlay
            .route("/admin/prepare-unpublish", web::post().to(handlers::admin_prepare_unpublish))  // Admin: populate DB for unpublish
            .route("/wallet/debug/validate-beef", web::post().to(handlers::debug_validate_beef))  // Debug: BEEF ancestry validation
            .route("/wallet/debug/repair-nosend", web::post().to(handlers::debug_repair_nosend))  // Debug: repair DB after nosend broadcast
            .route("/wallet/debug/broadcast-nosend", web::post().to(handlers::debug_broadcast_nosend))  // Debug: broadcast nosend tx to ARC
            // Authentication endpoints
            .route("/.well-known/auth", web::post().to(handlers::well_known_auth))

            // Custom wallet endpoints
            .route("/wallet/status", web::get().to(handlers::wallet_status))
            .route("/wallet/create", web::post().to(handlers::wallet_create))
            .route("/wallet/delete", web::post().to(handlers::wallet_delete))
            .route("/wallet/balance", web::get().to(handlers::wallet_balance))
            .route("/wallet/sync", web::post().to(handlers::wallet_sync))
            .route("/wallet/address/generate", web::post().to(handlers::generate_address))
            .route("/wallet/addresses", web::get().to(handlers::get_all_addresses))
            .route("/wallet/address/current", web::get().to(handlers::get_current_address))
            // Phase 2 Step 3b.1: consolidated legacy Yours address derivation.
            // Returns {bsvAddress, ordAddress, identityAddress} in a single round-trip
            // using BRC-42 with the yours-legacy-v1 protocol IDs and `yours-{origin}` keyID.
            .route("/wallet/yours-legacy-addresses", web::post().to(handlers::yours_legacy_addresses))
            // Phase 2 Step 3b.2: address → mainnet P2PKH locking script.
            // HTTP wrapper around the Step 3b.0 unified `recovery::address_to_p2pkh_script`
            // (Base58Check + checksum + mainnet version-byte check). Used by the legacy
            // `yours.sendBsv` translator to resolve each {address, amount} payment to a
            // canonical createAction output (lockingScript + script_type).
            .route("/wallet/address-to-script", web::post().to(handlers::address_to_script))
            // Phase 2 Step 3c.2: BIE1 (ECIES Electrum) legacy encrypt/decrypt for Yours-era
            // dApps that store / receive ciphertexts under the pre-BRC-2 format. Both gated
            // by check_domain_approved just like canonical /encrypt and /decrypt.
            .route("/wallet/encrypt-bie1", web::post().to(handlers::encrypt_bie1_handler))
            .route("/wallet/decrypt-bie1", web::post().to(handlers::decrypt_bie1_handler))
            .route("/wallet/backup", web::post().to(handlers::wallet_backup))
            .route("/wallet/backup/onchain", web::post().to(handlers::wallet_backup_onchain))
            .route("/wallet/backup/onchain/verify", web::post().to(handlers::wallet_backup_onchain_verify))
            .route("/wallet/recover/onchain", web::post().to(handlers::wallet_recover_onchain))
            .route("/wallet/restore", web::post().to(handlers::wallet_restore))
            .route("/wallet/unlock", web::post().to(handlers::wallet_unlock))
            .route("/wallet/recover", web::post().to(handlers::wallet_recover))
            .route("/wallet/recover-external", web::post().to(handlers::wallet_recover_external))
            .route("/wallet/rescan", web::post().to(handlers::wallet_rescan))
            .route("/wallet/cleanup", web::post().to(handlers::wallet_cleanup))
            .route("/wallet/consolidate-dust", web::post().to(handlers::wallet_consolidate_dust))
            .route("/wallet/export", web::post().to(handlers::wallet_export))
            .service(
                web::resource("/wallet/import")
                    .app_data(web::PayloadConfig::new(100 * 1024 * 1024))  // 100MB for large backups
                    .route(web::post().to(handlers::wallet_import))
            )

            // Price endpoint (Phase 2.3 — for C++ auto-approve engine)
            .route("/wallet/bsv-price", web::get().to(handlers::get_bsv_price))

            // Sync status endpoints (recovery progress tracking)
            .route("/wallet/sync-status", web::get().to(handlers::get_sync_status))
            .route("/wallet/sync-status/seen", web::post().to(handlers::mark_sync_seen))

            // Transaction endpoints
            .route("/transaction/send", web::post().to(handlers::send_transaction))

            // Domain permissions endpoints (Phase 2.1)
            .route("/domain/permissions", web::get().to(handlers::get_domain_permission))
            .route("/domain/permissions", web::post().to(handlers::set_domain_permission))
            .route("/domain/permissions", web::delete().to(handlers::delete_domain_permission))
            .route("/domain/permissions/all", web::get().to(handlers::list_domain_permissions))
            .route("/domain/permissions/certificate", web::get().to(handlers::check_cert_permissions))
            .route("/domain/permissions/certificate", web::post().to(handlers::approve_cert_fields))
            .route("/domain/permissions/certificate", web::delete().to(handlers::revoke_cert_fields))
            // Phase 1.5 Step 3 — domain sub-permission CRUD (V18 child tables)
            .route("/domain/permissions/protocol", web::post().to(handlers::grant_protocol_permission))
            .route("/domain/permissions/protocol", web::delete().to(handlers::revoke_protocol_permission))
            .route("/domain/permissions/protocol", web::get().to(handlers::list_protocol_permissions))
            .route("/domain/permissions/basket", web::post().to(handlers::grant_basket_permission))
            .route("/domain/permissions/basket", web::delete().to(handlers::revoke_basket_permission))
            .route("/domain/permissions/basket", web::get().to(handlers::list_basket_permissions))
            .route("/domain/permissions/counterparty", web::post().to(handlers::grant_counterparty_permission))
            .route("/domain/permissions/counterparty", web::delete().to(handlers::revoke_counterparty_permission))
            .route("/domain/permissions/counterparty", web::get().to(handlers::list_counterparty_permissions))

            .route("/wallet/session-approve", web::post().to(permission_service::handlers::session_approve))
            .route("/wallet/session-revoke", web::post().to(permission_service::handlers::session_revoke))
            .route("/wallet/session/close", web::post().to(permission_service::handlers::session_close))

            // NOTE: Adblock per-site toggles moved to C++ AdblockCache (JSON file in profile dir).
            // No longer served by wallet backend — see AdblockCache.h.

            // BRC-33 Message Relay endpoints
            .route("/sendMessage", web::post().to(handlers::send_message))
            .route("/listMessages", web::post().to(handlers::list_messages))
            .route("/acknowledgeMessage", web::post().to(handlers::acknowledge_message))

            // PeerPay (BRC-29) endpoints
            .route("/wallet/peerpay/send", web::post().to(handlers::peerpay_send))
            .route("/wallet/peerpay/check", web::post().to(handlers::peerpay_check))
            .route("/wallet/peerpay/status", web::get().to(handlers::peerpay_status))
            .route("/wallet/peerpay/dismiss", web::post().to(handlers::peerpay_dismiss))
            .route("/wallet/peerpay/outbox-retry", web::post().to(handlers::peerpay_outbox_retry))

            // BRC-121 Simple HTTP 402 Payment
            .route("/wallet/pay402", web::post().to(handlers::pay_402))
            .route("/wallet/broadcast-nosend", web::post().to(handlers::broadcast_nosend))

            // Paymail (bsvalias) endpoints
            .route("/wallet/paymail/send", web::post().to(handlers::paymail_send))
            .route("/wallet/paymail/resolve", web::get().to(handlers::paymail_resolve))

            // Unified recipient resolution (identity key, paymail, BSV address)
            .route("/wallet/recipient/resolve", web::get().to(handlers::recipient_resolve))

            // Recipient autocomplete suggestions (Issue #38)
            .route("/wallet/recipient/suggest", web::get().to(handlers::recipient_suggest))

            // Unified activity feed (sent + received transactions)
            .route("/wallet/activity", web::get().to(handlers::wallet_activity))

            // Wallet settings (Phase 4 - Advanced Wallet Dashboard)
            .route("/wallet/settings", web::get().to(handlers::wallet_settings_get))
            .route("/wallet/settings", web::post().to(handlers::wallet_settings_set))
            .route("/wallet/reveal-mnemonic", web::post().to(handlers::reveal_mnemonic))
            .route("/domain/permissions/reset-all", web::post().to(handlers::domain_permissions_reset_all))

    })
    .bind(("127.0.0.1", wallet_port()))?
    .run();

    // Spawn shutdown watcher that stops the HTTP server when Ctrl+C / POST /shutdown
    // cancels the token (Phase 8D). Keeps the watcher minimal: cancel → drain.
    let server_handle = server.handle();
    tokio::spawn(async move {
        shutdown_token.cancelled().await;
        log::info!("🛑 Stopping HTTP server...");
        server_handle.stop(true).await; // graceful: finish in-flight requests
    });

    // Block here until the server has fully drained in-flight requests.
    server.await?;

    // ── OD-2 graceful-exit sequence ─────────────────────────────────────────────
    // The server is stopped (in-flight HTTP drained). Quiesce the Monitor, checkpoint
    // the WAL so the next open is clean (and the image-file lock releases cleanly for
    // the Windows updater), flush logs, then exit deterministically — all well within
    // the C++ StopWalletServer 5s WaitForSingleObject budget so the clean-exit branch
    // fires instead of TerminateProcess.
    // See development-docs/DevOps-CICD/WALLET_GRACEFUL_EXIT_SPEC.md.
    log::info!("🛑 HTTP server stopped — beginning clean shutdown sequence");

    // 1. Quiesce the Monitor: bounded 2s join. The run loop breaks on the cancelled
    //    token between tasks; a task blocked mid-network-await can't be interrupted, so
    //    it burns the full cap before being abandoned — WAL + startup recovery make that
    //    safe (never worse than the old TerminateProcess hard-kill). Cap is 2s (not 3s)
    //    to keep drain+join+checkpoint comfortably under the C++ 5s WaitForSingleObject
    //    even when an in-flight HTTP request makes the drain itself multi-second
    //    (smoke 2026-06-24: idle drain ~1.2s + 3s join ≈ 4.2s left too little headroom).
    if let Some(handle) = monitor_handle {
        match tokio::time::timeout(std::time::Duration::from_secs(2), handle).await {
            Ok(_) => log::info!("   ✅ Monitor quiesced"),
            Err(_) => log::warn!("   ⚠️  Monitor did not quiesce within 2s — abandoning (WAL-safe)"),
        }
    }

    // 2. Best-effort WAL checkpoint(TRUNCATE). Never block exit: try_lock with a short
    //    bounded retry, then proceed regardless (an un-checkpointed WAL replays cleanly
    //    on the next open).
    {
        let mut done = false;
        for _ in 0..10 {
            if let Ok(db) = db_for_shutdown.try_lock() {
                match db.checkpoint_truncate() {
                    Ok(()) => log::info!("   ✅ WAL checkpoint(TRUNCATE) complete"),
                    Err(e) => log::warn!("   ⚠️  WAL checkpoint failed (non-fatal): {}", e),
                }
                done = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        if !done {
            log::warn!("   ⚠️  WAL checkpoint skipped — DB busy at shutdown (WAL replays clean on next open)");
        }
    }

    // 3. Final log line + flush, then deterministic clean exit.
    log::info!("✅ Wallet clean exit (OD-2 graceful-exit)");
    logger.flush();
    std::process::exit(0);
}
