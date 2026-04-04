//! TaskBackup — Periodic on-chain wallet backup
//!
//! Triggers an on-chain backup via the HTTP endpoint. The backup handler
//! uses hash comparison to skip if nothing changed. Returns BackupOutcome
//! to let the Monitor decide whether to clear the "soon" flag.
//!
//! Outcomes:
//! - Broadcast: backup was broadcast on-chain → clear flag
//! - Skipped: hash unchanged, nothing to back up → clear flag
//! - Deferred: precondition not met (no wallet, locked, DB busy, insufficient funds) → keep flag
//! - Failed: backup attempted but errored → keep flag

use actix_web::web;
use log::{info, warn};
use crate::AppState;
use crate::database::{WalletRepository, OutputRepository};

/// Minimum balance required to attempt a backup (token + marker + fee buffer)
const MIN_BACKUP_BALANCE_SATS: i64 = 3000;

/// Outcome of a backup attempt — determines whether the "soon" flag is cleared.
pub enum BackupOutcome {
    /// Backup transaction was broadcast on-chain
    Broadcast(String),
    /// Hash unchanged — no changes since last backup
    Skipped,
    /// Precondition not met — retry on next tick (DB busy, locked, no wallet, insufficient funds)
    Deferred(String),
    /// Backup attempted but failed — retry on next tick
    Failed(String),
}

impl BackupOutcome {
    /// Whether the backup state is current (flag can be cleared)
    pub fn is_current(&self) -> bool {
        matches!(self, BackupOutcome::Broadcast(_) | BackupOutcome::Skipped)
    }
}

pub async fn run(state: &web::Data<AppState>) -> BackupOutcome {
    // Quick precondition checks (one DB lock, no network)
    {
        let db = match state.database.try_lock() {
            Ok(db) => db,
            Err(_) => return BackupOutcome::Deferred("DB busy".into()),
        };

        let wallet_repo = WalletRepository::new(db.connection());
        if wallet_repo.get_primary_wallet().ok().flatten().is_none() {
            return BackupOutcome::Deferred("No wallet".into());
        }

        if !db.is_unlocked() {
            return BackupOutcome::Deferred("Wallet locked".into());
        }

        let output_repo = OutputRepository::new(db.connection());
        let balance = output_repo.calculate_total_balance().unwrap_or(0);
        if balance < MIN_BACKUP_BALANCE_SATS {
            return BackupOutcome::Deferred("Insufficient funds".into());
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
                    let txid = body["txid"].as_str().unwrap_or("unknown").to_string();
                    info!("💾 TaskBackup: ✅ backup broadcast: {}", txid);
                    BackupOutcome::Broadcast(txid)
                } else {
                    let err = body["error"].as_str().unwrap_or("unknown");
                    if err.contains("skipped") {
                        // Hash unchanged — normal, don't log as warning
                        BackupOutcome::Skipped
                    } else {
                        warn!("💾 TaskBackup: ⚠️  {}", err);
                        BackupOutcome::Failed(err.to_string())
                    }
                }
            } else {
                let status = resp.status();
                warn!("💾 TaskBackup: ⚠️  HTTP {}", status);
                BackupOutcome::Failed(format!("HTTP {}", status))
            }
        }
        Err(e) => {
            warn!("💾 TaskBackup: ⚠️  HTTP error: {}", e);
            BackupOutcome::Failed(format!("HTTP error: {}", e))
        }
    }
}
