//! TaskReplayOverlay — Retry overlay notification for unpublished certificates
//!
//! When a certificate is unpublished (spent on-chain) but the overlay wasn't notified
//! (due to WoC rate limiting, network issues, etc.), this task retries the BEEF
//! submission to the overlay.
//!
//! Certificates with publish_status = 'unpublished_pending_overlay' are candidates.
//! The task builds a minimal BEEF (publish tx + spending tx, both with merkle proofs)
//! and submits to the overlay. Once the overlay confirms removal (or lookup shows
//! the cert is gone), the status is updated to 'unpublished'.
//!
//! Retry strategy:
//! - First attempt ~10 minutes after unpublish (wait for block confirmation)
//! - Then every 5 minutes (this task's interval)
//! - Max 20 attempts tracked via overlay_retry_count column
//! - After 20 failures, status stays as 'unpublished_pending_overlay' for manual attention
//!
//! Interval: 300 seconds (5 minutes)

use actix_web::web;
use log::{info, warn, error};

use crate::AppState;
use crate::database::CertificateRepository;

/// Maximum retry attempts before giving up
const MAX_OVERLAY_RETRIES: i32 = 20;

/// Run the TaskReplayOverlay task
pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    // Ensure the overlay_retry_count column exists (idempotent)
    {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let _ = db.connection().execute(
            "ALTER TABLE certificates ADD COLUMN overlay_retry_count INTEGER DEFAULT 0",
            [],
        ); // Silently fails if column already exists
    }

    // Find certificates pending overlay notification
    let pending_certs: Vec<(Vec<u8>, Vec<u8>, Vec<u8>, String, i32)> = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let mut stmt = db.connection().prepare(
            "SELECT type, serial_number, certifier, publish_txid, COALESCE(overlay_retry_count, 0)
             FROM certificates
             WHERE publish_status = 'unpublished_pending_overlay'
             AND COALESCE(overlay_retry_count, 0) < ?1"
        ).map_err(|e| format!("Prepare: {}", e))?;

        let rows = stmt.query_map(rusqlite::params![MAX_OVERLAY_RETRIES], |row| {
            // type, serial_number, certifier are stored as base64/hex strings
            let type_str: String = row.get(0)?;
            let serial_str: String = row.get(1)?;
            let certifier_str: String = row.get(2)?;
            let publish_txid: Option<String> = row.get(3)?;
            let retry_count: i32 = row.get(4)?;
            Ok((type_str, serial_str, certifier_str, publish_txid.unwrap_or_default(), retry_count))
        }).map_err(|e| format!("Query: {}", e))?;

        let mut results = Vec::new();
        for row in rows {
            if let Ok((type_str, serial_str, certifier_str, publish_txid, retry_count)) = row {
                results.push((
                    type_str.into_bytes(),
                    serial_str.into_bytes(),
                    certifier_str.into_bytes(),
                    publish_txid,
                    retry_count,
                ));
            }
        }
        results
    };

    if pending_certs.is_empty() {
        return Ok(());
    }

    info!("🔄 TaskReplayOverlay: {} certificate(s) pending overlay notification", pending_certs.len());

    for (type_bytes, serial_bytes, certifier_bytes, publish_txid, retry_count) in &pending_certs {
        let publish_txid_str = String::from_utf8_lossy(publish_txid.as_bytes());
        if publish_txid.is_empty() {
            // Orphan row — no publish_txid means we can't rebuild the removal-BEEF.
            // Increment retry count so this row eventually self-quiets at 20 attempts
            // (~100 min) instead of spamming the warning forever.
            warn!("   ⚠️ Certificate has no publish_txid — cannot replay (orphan; will self-quiet)");
            increment_retry_count(state, publish_txid);
            continue;
        }

        let txid_short = &publish_txid[..16.min(publish_txid.len())];
        info!("   🔄 Replaying overlay removal for publish tx {} (attempt {}/{})",
            txid_short, retry_count + 1, MAX_OVERLAY_RETRIES);

        // Find the spending tx via outputs.spent_by
        let spending_tx_info: Option<(String, Vec<u8>)> = {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            db.connection().query_row(
                "SELECT t.txid, t.raw_tx FROM outputs o
                 JOIN transactions t ON o.spent_by = t.id
                 WHERE o.txid = ?1 AND o.vout = 0 AND o.spendable = 0
                 AND t.status != 'failed'",
                rusqlite::params![publish_txid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).ok()
        };

        let (spending_txid, spending_raw_tx) = match spending_tx_info {
            Some(info) => info,
            None => {
                warn!("   ⚠️ No spending tx found for {} — skipping", txid_short);
                increment_retry_count(state, publish_txid);
                continue;
            }
        };

        let stxid_short = &spending_txid[..16.min(spending_txid.len())];

        // Build BEEF: publish tx (from WoC) + spending tx (from DB), both with merkle proofs
        let mut beef = crate::beef::Beef::new();

        // Add publish tx via Services (Phase 1.6d.C — was inline WoC)
        if let Ok(parent_hex) = crate::cache_helpers::fetch_parent_transaction_from_api(&state.services, publish_txid).await {
            if let Ok(parent_bytes) = hex::decode(&parent_hex) {
                beef.add_parent_transaction(parent_bytes);
            }
        }

        // Add merkle proof for publish tx via Services
        if let Ok(Some(proof_obj)) = crate::cache_helpers::fetch_tsc_proof_from_api(&state.services, publish_txid).await {
            // B5-T1-P7a-2 `C15`: checked against our own headers before it is embedded.
            if let Some(tsc) = crate::beef_helpers::embeddable_proof(publish_txid, &proof_obj, &state.services, &*state.chain_tracker).await {
                if let Some(tx_idx) = beef.find_txid(publish_txid) {
                    let _ = beef.add_tsc_merkle_proof(publish_txid, tx_idx, &tsc);
                }
            }
        }

        // Add spending tx as main transaction + its merkle proof from DB
        beef.set_main_transaction(spending_raw_tx);
        {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            if let Ok(Some(tsc_json)) = crate::proof_store::stored_proof_for_beef(db.connection(), &*state.chain_tracker, &spending_txid) {
                if let Some(tx_idx) = beef.find_txid(&spending_txid) {
                    let _ = beef.add_tsc_merkle_proof(&spending_txid, tx_idx, &tsc_json);
                }
            }
        }

        beef.sort_topologically();
        let beef_v1 = match beef.to_v1_bytes() {
            Ok(b) => b,
            Err(e) => {
                warn!("   ⚠️ BEEF serialization failed for {}: {}", txid_short, e);
                increment_retry_count(state, publish_txid);
                continue;
            }
        };

        info!("   📡 Submitting {} bytes BEEF to overlay (spending tx {})", beef_v1.len(), stxid_short);

        // Submit to overlay
        let overlay_ok = match crate::overlay::submit_to_identity_overlay(&state.ship_cache, &beef_v1).await {
            Ok(true) => {
                info!("   ✅ Overlay accepted removal for {}", txid_short);
                true
            }
            Ok(false) => {
                // Ambiguous — verify via lookup
                let serial_str = String::from_utf8_lossy(serial_bytes);
                match crate::overlay::lookup_published_certificate(&serial_str).await {
                    Ok(None) => {
                        info!("   ✅ Overlay lookup confirms token removed for {}", txid_short);
                        true
                    }
                    Ok(Some(_)) => {
                        info!("   ℹ️ Overlay still has token for {} — will retry", txid_short);
                        false
                    }
                    Err(_) => false,
                }
            }
            Err(e) => {
                warn!("   ⚠️ Overlay submission failed for {}: {}", txid_short, e);
                false
            }
        };

        if overlay_ok {
            // Update status to fully unpublished
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            let _ = db.connection().execute(
                "UPDATE certificates SET publish_status = 'unpublished'
                 WHERE publish_status = 'unpublished_pending_overlay'
                 AND serial_number = ?1",
                rusqlite::params![String::from_utf8_lossy(serial_bytes).to_string()],
            );
            info!("   ✅ Certificate {} updated to 'unpublished'", txid_short);
        } else {
            increment_retry_count(state, publish_txid);
        }
    }

    Ok(())
}

/// Increment the overlay_retry_count for a certificate
fn increment_retry_count(state: &web::Data<AppState>, publish_txid: &str) {
    if let Ok(db) = state.database.lock() {
        // Use publish_txid to find the cert — but publish_txid is cleared on unpublish.
        // Instead, find by status and increment.
        let _ = db.connection().execute(
            "UPDATE certificates SET overlay_retry_count = COALESCE(overlay_retry_count, 0) + 1
             WHERE publish_status = 'unpublished_pending_overlay'",
            [],
        );
    }
}

