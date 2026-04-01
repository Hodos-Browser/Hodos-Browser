//! TaskBackup — Periodic on-chain wallet backup
//!
//! Triggers an on-chain backup via the HTTP endpoint. The backup handler
//! uses hash comparison to skip if nothing changed. Skips silently if:
//! - No wallet exists
//! - Wallet is locked (PIN not entered)
//! - Insufficient funds (< 3000 sats)
//! - No changes since last backup (hash match)
//!
//! Interval: 30 minutes (1800s)

use actix_web::web;
use log::{info, warn};
use crate::AppState;
use crate::database::{WalletRepository, OutputRepository};

/// Minimum balance required to attempt a backup (token + marker + fee buffer)
const MIN_BACKUP_BALANCE_SATS: i64 = 3000;

pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    // Quick precondition checks (one DB lock, no network)
    {
        let db = match state.database.try_lock() {
            Ok(db) => db,
            Err(_) => return Ok(()), // DB busy, skip
        };

        let wallet_repo = WalletRepository::new(db.connection());
        if wallet_repo.get_primary_wallet().ok().flatten().is_none() {
            return Ok(()); // No wallet
        }

        if !db.is_unlocked() {
            return Ok(()); // Locked
        }

        let output_repo = OutputRepository::new(db.connection());
        let balance = output_repo.calculate_total_balance().unwrap_or(0);
        if balance < MIN_BACKUP_BALANCE_SATS {
            return Ok(()); // Insufficient funds
        }

        // Don't backup while transactions are still settling (nosend/sending/unproven).
        // A backup taken during this window could capture ghost outputs from txs that
        // never make it on-chain, leading to corrupt recovery state.
        let pending_count: i64 = db.connection().query_row(
            "SELECT COUNT(*) FROM transactions WHERE status IN ('nosend', 'sending', 'unproven')",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        if pending_count > 0 {
            info!("💾 TaskBackup: ⏳ {} pending transaction(s) — deferring backup until settled", pending_count);
            return Ok(());
        }
    } // DB lock dropped

    // Call the backup endpoint via HTTP.
    // The handler does hash comparison and skips if nothing changed.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    match client.post("http://127.0.0.1:31301/wallet/backup/onchain")
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                if body["success"].as_bool() == Some(true) {
                    let txid = body["txid"].as_str().unwrap_or("unknown");
                    info!("💾 TaskBackup: ✅ backup broadcast: {}", txid);
                } else {
                    let err = body["error"].as_str().unwrap_or("unknown");
                    if err.contains("skipped") {
                        // Hash unchanged — normal, don't log as warning
                    } else {
                        warn!("💾 TaskBackup: ⚠️  {}", err);
                    }
                }
            }
        }
        Err(e) => {
            warn!("💾 TaskBackup: ⚠️  HTTP error: {}", e);
        }
    }

    Ok(())
}
