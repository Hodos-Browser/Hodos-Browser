//! TaskVerifyDoubleSpend — Independent verification of suspected double-spends
//!
//! When ARC reports DOUBLE_SPEND_ATTEMPTED, the wallet marks inputs as
//! `spending_description = 'dss:{our_txid}'` (suspected, not confirmed).
//! This task verifies each suspicion independently against WhatsOnChain:
//!
//! 1. Check our txid on WoC — is it known/mined/unknown?
//! 2. If known → false alarm, recover the transaction
//! 3. If unknown → check each input individually via /tx/{txid}/{vout}/spent
//! 4. If input is spent by another tx → confirmed double-spend
//! 5. If input is NOT spent → false alarm, restore as spendable
//!
//! Design rationale: The BSV SDK (wallet-toolbox) never trusts a single
//! broadcaster's double-spend report. `TaskReviewDoubleSpends` verifies
//! independently before marking anything as permanently lost. We replicate
//! that approach here.
//!
//! Interval: 60 seconds

use actix_web::web;
use log::{info, warn};
use std::collections::HashMap;
use std::time::Duration;

use crate::AppState;
use crate::arc_status::SUSPECTED_DOUBLE_SPEND_PREFIX;
use crate::database::{TransactionRepository, OutputRepository, ProvenTxReqRepository};

/// Maximum suspected transaction groups to process per tick.
/// Prevents the task from monopolizing network calls.
const MAX_GROUPS_PER_TICK: usize = 5;

/// Delay between individual WoC API calls (ms).
const WOC_CALL_DELAY_MS: u64 = 500;

/// After this many seconds, promote to confirmed double-spend regardless.
const ESCALATION_CONFIRM_SECS: i64 = 6 * 60 * 60; // 6 hours

/// Number of retries when checking our txid status on WoC.
const TXID_CHECK_RETRIES: u32 = 3;

/// Result of checking our txid against the network.
enum TxidStatus {
    /// Transaction is mined with confirmations.
    Mined { confirmations: u32 },
    /// Transaction is in the mempool (known but unconfirmed).
    InMempool,
    /// Transaction is not known to the network (404).
    Unknown,
    /// API error — could not determine status.
    Error(String),
}

/// Result of checking if a specific output is spent.
struct SpentInfo {
    /// The txid that spent this output.
    spending_txid: String,
}

/// Run the TaskVerifyDoubleSpend task.
pub async fn run(state: &web::Data<AppState>, client: &reqwest::Client) -> Result<(), String> {
    // Step 1: Load all suspected outputs from DB (quick lock, release).
    let suspects: Vec<(i64, String, u32, Vec<u8>, String, i64)> = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let output_repo = OutputRepository::new(db.connection());
        output_repo.get_suspected_double_spends()
            .map_err(|e| format!("Query suspected: {}", e))?
    };

    if suspects.is_empty() {
        return Ok(());
    }

    info!("🔍 TaskVerifyDoubleSpend: checking {} suspected output(s)", suspects.len());

    // Group by our_txid (parsed from 'dss:{txid}').
    let mut grouped: HashMap<String, Vec<(i64, String, u32, Vec<u8>, i64)>> = HashMap::new();
    for (output_id, txid, vout, script, spending_desc, updated_at) in &suspects {
        if let Some(our_txid) = spending_desc.strip_prefix(SUSPECTED_DOUBLE_SPEND_PREFIX) {
            grouped.entry(our_txid.to_string())
                .or_default()
                .push((*output_id, txid.clone(), *vout, script.clone(), *updated_at));
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut verified = 0;
    let mut false_alarms = 0;
    let mut confirmed = 0;
    let mut escalated = 0;

    for (our_txid, inputs) in grouped.iter().take(MAX_GROUPS_PER_TICK) {
        let short_txid = &our_txid[..our_txid.len().min(16)];

        // Check escalation timeouts first.
        let oldest_updated = inputs.iter().map(|(_, _, _, _, ts)| *ts).min().unwrap_or(now);
        let age_secs = now - oldest_updated;

        if age_secs > ESCALATION_CONFIRM_SECS {
            // Past final escalation — promote all to confirmed.
            warn!("   ⏰ {} suspected for {}h — promoting to confirmed double-spend",
                short_txid, age_secs / 3600);
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            let output_repo = OutputRepository::new(db.connection());
            for (output_id, _, _, _, _) in inputs {
                let _ = output_repo.confirm_double_spend(*output_id);
            }
            drop(db);
            state.balance_cache.invalidate();
            escalated += inputs.len();
            continue;
        }

        // Step 1: Check our txid on WoC (up to 3 retries, 1s waits).
        let status = check_our_txid_on_woc(client, our_txid).await;
        tokio::time::sleep(Duration::from_millis(WOC_CALL_DELAY_MS)).await;

        match status {
            TxidStatus::Mined { confirmations } => {
                // FALSE ALARM — our tx is mined! Recover it.
                info!("   ✅ {} is MINED ({} confs) — false alarm! Recovering", short_txid, confirmations);
                handle_false_alarm_mined(state, our_txid, inputs);
                false_alarms += inputs.len();
            }
            TxidStatus::InMempool => {
                // FALSE ALARM — our tx is in mempool. Recover it.
                info!("   ✅ {} is in MEMPOOL — false alarm! Recovering", short_txid);
                handle_false_alarm_mempool(state, our_txid, inputs);
                false_alarms += inputs.len();
            }
            TxidStatus::Unknown => {
                // Our tx genuinely failed. Verify each input individually.
                info!("   🔍 {} is UNKNOWN on WoC — verifying {} input(s) individually", short_txid, inputs.len());
                for (output_id, input_txid, input_vout, _, _) in inputs {
                    let spent_result = check_output_spent(client, input_txid, *input_vout).await;
                    tokio::time::sleep(Duration::from_millis(WOC_CALL_DELAY_MS)).await;

                    match spent_result {
                        Ok(Some(spent_info)) => {
                            // Input IS spent by another tx — confirmed double-spend.
                            info!("   ❌ Input {}:{} is spent by {} — confirmed double-spend",
                                &input_txid[..input_txid.len().min(16)], input_vout,
                                &spent_info.spending_txid[..spent_info.spending_txid.len().min(16)]);
                            if let Ok(db) = state.database.lock() {
                                let output_repo = OutputRepository::new(db.connection());
                                let _ = output_repo.confirm_double_spend(*output_id);
                            }
                            confirmed += 1;
                        }
                        Ok(None) => {
                            // Input is NOT spent — false alarm from ARC.
                            info!("   ✅ Input {}:{} is UNSPENT — false alarm, restoring",
                                &input_txid[..input_txid.len().min(16)], input_vout);
                            if let Ok(db) = state.database.lock() {
                                let output_repo = OutputRepository::new(db.connection());
                                let _ = output_repo.clear_suspected_double_spend(*output_id);
                            }
                            false_alarms += 1;
                        }
                        Err(e) => {
                            // API error — skip this input, retry next tick.
                            warn!("   ⚠️ WoC spent-check failed for {}:{}: {} — will retry",
                                &input_txid[..input_txid.len().min(16)], input_vout, e);
                        }
                    }
                    verified += 1;
                }
            }
            TxidStatus::Error(e) => {
                // Network error — skip this group, retry next tick.
                warn!("   ⚠️ WoC error checking {}: {} — will retry", short_txid, e);
                continue;
            }
        }

        // Invalidate balance cache after any changes.
        if false_alarms > 0 || confirmed > 0 {
            state.balance_cache.invalidate();
        }
    }

    if verified > 0 || false_alarms > 0 || confirmed > 0 || escalated > 0 {
        info!("✅ TaskVerifyDoubleSpend: {} verified, {} false alarms, {} confirmed, {} escalated",
            verified, false_alarms, confirmed, escalated);
        super::log_monitor_event(state, "TaskVerifyDoubleSpend:completed",
            Some(&format!("{} false_alarms, {} confirmed, {} escalated", false_alarms, confirmed, escalated)));
    }

    Ok(())
}

/// Check if our txid is known on the network via WhatsOnChain.
/// Retries up to TXID_CHECK_RETRIES times with 1s waits between attempts.
async fn check_our_txid_on_woc(client: &reqwest::Client, txid: &str) -> TxidStatus {
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/hash/{}", txid);

    for attempt in 0..TXID_CHECK_RETRIES {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        match client.get(&url).send().await {
            Ok(resp) => {
                let status_code = resp.status().as_u16();
                if status_code == 404 {
                    return TxidStatus::Unknown;
                }
                if status_code == 200 {
                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                        let confirmations = body["confirmations"].as_u64().unwrap_or(0) as u32;
                        if confirmations > 0 {
                            return TxidStatus::Mined { confirmations };
                        } else {
                            return TxidStatus::InMempool;
                        }
                    }
                }
                // Non-200/404 — retry
            }
            Err(e) => {
                if attempt == TXID_CHECK_RETRIES - 1 {
                    return TxidStatus::Error(format!("HTTP error after {} retries: {}", TXID_CHECK_RETRIES, e));
                }
                // Retry
            }
        }
    }

    TxidStatus::Error("Max retries exceeded".to_string())
}

/// Check if a specific output (txid:vout) has been spent on-chain.
///
/// CRITICAL: Uses the correct WoC endpoint `/tx/{txid}/{vout}/spent`.
/// The path `/tx/{txid}/out/{vout}/spent` returns 404 for everything — DO NOT USE.
async fn check_output_spent(
    client: &reqwest::Client,
    txid: &str,
    vout: u32,
) -> Result<Option<SpentInfo>, String> {
    let url = format!(
        "https://api.whatsonchain.com/v1/bsv/main/tx/{}/{}/spent",
        txid, vout
    );

    let resp = client.get(&url).send().await
        .map_err(|e| format!("HTTP error: {}", e))?;

    let status_code = resp.status().as_u16();
    if status_code == 404 {
        // Output is NOT spent.
        return Ok(None);
    }

    if status_code == 200 {
        if let Ok(body) = resp.json::<serde_json::Value>().await {
            if let Some(spending_txid) = body["txid"].as_str() {
                return Ok(Some(SpentInfo {
                    spending_txid: spending_txid.to_string(),
                }));
            }
        }
        // 200 but couldn't parse — treat as error.
        return Err("WoC returned 200 but response couldn't be parsed".to_string());
    }

    Err(format!("WoC spent-check returned HTTP {}", status_code))
}

/// Handle false alarm when our tx is confirmed mined.
/// Promote the transaction back to completed/unproven status and restore input tracking.
fn handle_false_alarm_mined(
    state: &web::Data<AppState>,
    our_txid: &str,
    inputs: &[(i64, String, u32, Vec<u8>, i64)],
) {
    if let Ok(db) = state.database.lock() {
        let conn = db.connection();
        let short_txid = &our_txid[..our_txid.len().min(16)];

        // Restore the transaction status from 'failed' to 'unproven'.
        // TaskCheckForProofs will handle getting the actual proof.
        let tx_repo = TransactionRepository::new(conn);
        if let Err(e) = tx_repo.update_broadcast_status(our_txid, "broadcast") {
            warn!("   ⚠️ Failed to promote {} to unproven: {}", short_txid, e);
        }

        // Clear the failed_at timestamp.
        let _ = conn.execute(
            "UPDATE transactions SET status = 'unproven', failed_at = NULL WHERE txid = ?1 AND status = 'failed'",
            rusqlite::params![our_txid],
        );

        // Ensure a proven_tx_req exists for proof tracking.
        let req_repo = ProvenTxReqRepository::new(conn);
        if let Ok(None) = req_repo.get_by_txid(our_txid) {
            let _ = req_repo.create(our_txid, &[], None, "unproven");
        }

        // Re-enable outputs from this transaction (they were disabled on failure).
        let re_enabled = conn.execute(
            "UPDATE outputs SET spendable = 1, updated_at = strftime('%s','now')
             WHERE txid = ?1 AND spendable = 0 AND spending_description IS NULL",
            rusqlite::params![our_txid],
        ).unwrap_or(0);
        if re_enabled > 0 {
            info!("   ♻️ Re-enabled {} output(s) from recovered tx {}", re_enabled, short_txid);
        }

        // Clear suspected double-spend marking on the inputs.
        // Mark them as spent by our (now recovered) transaction.
        for (output_id, _, _, _, _) in inputs {
            let _ = conn.execute(
                "UPDATE outputs SET spending_description = ?1, updated_at = strftime('%s','now')
                 WHERE outputId = ?2 AND spending_description LIKE 'dss:%'",
                rusqlite::params![our_txid, output_id],
            );
        }
    }
    state.balance_cache.invalidate();
}

/// Handle false alarm when our tx is in the mempool (not yet mined).
/// Same recovery as mined, but status goes to 'unproven' instead of 'completed'.
fn handle_false_alarm_mempool(
    state: &web::Data<AppState>,
    our_txid: &str,
    inputs: &[(i64, String, u32, Vec<u8>, i64)],
) {
    // Same logic as mined — the difference is just that there's no proof yet.
    // TaskCheckForProofs will handle proof acquisition.
    handle_false_alarm_mined(state, our_txid, inputs);
}
