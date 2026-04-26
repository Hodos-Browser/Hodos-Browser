//! TaskRetryPeerPayOutbox — Retry MessageBox delivery for failed PeerPay sends
//!
//! When a PeerPay send succeeds on-chain but MessageBox delivery fails,
//! the payload is queued in peerpay_outbox. This task retries delivery
//! on an escalating schedule:
//!   - First 10 retries (~10 min): every 60s
//!   - Next 10 retries (~20 min): every 120s
//!   - After 20 retries (~30 min total): mark as 'exhausted' (user can manually retry)
//!
//! Interval: 30 seconds (fast tick — actual retry governed by next_retry_at timestamps)

use actix_web::web;
use log::{info, warn, error, debug};

use crate::AppState;
use crate::messagebox::MessageBoxClient;
use crate::database::PeerPayRepository;

/// Run the TaskRetryPeerPayOutbox task
pub async fn run(state: &web::Data<AppState>, _client: &reqwest::Client) -> Result<(), String> {
    // 1. Get wallet master keys (brief DB lock)
    let (master_privkey, master_pubkey) = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let privkey = match crate::database::get_master_private_key_from_db(&db) {
            Ok(k) => k,
            Err(_) => {
                debug!("TaskRetryPeerPayOutbox: no wallet yet, skipping");
                return Ok(());
            }
        };
        let pubkey = match crate::database::get_master_public_key_from_db(&db) {
            Ok(k) => k,
            Err(_) => {
                debug!("TaskRetryPeerPayOutbox: can't get public key, skipping");
                return Ok(());
            }
        };
        (privkey, pubkey)
    };

    // 2. Get due outbox entries (brief DB lock)
    let entries = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        PeerPayRepository::get_due_outbox_entries(db.connection())
            .map_err(|e| format!("Failed to query outbox: {}", e))?
    };

    if entries.is_empty() {
        return Ok(());
    }

    info!("📤 TaskRetryPeerPayOutbox: {} outbox entry(s) due for retry", entries.len());

    // 3. Build MessageBox client once (all entries use the same wallet keys)
    let mb_client = MessageBoxClient::new(master_privkey, master_pubkey);

    // 4. Retry each entry
    let mut delivered_count = 0;
    let mut failed_count = 0;

    for entry in &entries {
        let recipient_pubkey = match hex::decode(&entry.recipient_pubkey_hex) {
            Ok(b) if b.len() == 33 => b,
            _ => {
                warn!("TaskRetryPeerPayOutbox: invalid recipient key for txid {}, skipping", &entry.txid[..16.min(entry.txid.len())]);
                continue;
            }
        };

        match mb_client.send_message(&recipient_pubkey, "payment_inbox", &entry.payload_bytes).await {
            Ok(_) => {
                info!("   ✅ Outbox retry succeeded for txid {} (attempt {})",
                    &entry.txid[..16.min(entry.txid.len())], entry.retry_count + 1);

                let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
                let _ = PeerPayRepository::mark_outbox_delivered(db.connection(), entry.id);
                delivered_count += 1;
            }
            Err(e) => {
                warn!("   ⚠️  Outbox retry failed for txid {} (attempt {}): {}",
                    &entry.txid[..16.min(entry.txid.len())], entry.retry_count + 1, e);

                let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
                if let Err(db_err) = PeerPayRepository::update_outbox_retry_failed(
                    db.connection(), entry.id, entry.retry_count
                ) {
                    error!("   ❌ Failed to update outbox retry: {}", db_err);
                }
                failed_count += 1;
            }
        }

        // Rate limit: 500ms between entries to avoid hammering MessageBox
        if entries.len() > 1 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    if delivered_count > 0 || failed_count > 0 {
        info!("📤 TaskRetryPeerPayOutbox: {} delivered, {} failed", delivered_count, failed_count);
        super::log_monitor_event(
            state,
            "TaskRetryPeerPayOutbox:completed",
            Some(&format!("{} delivered, {} failed", delivered_count, failed_count)),
        );
    }

    Ok(())
}
