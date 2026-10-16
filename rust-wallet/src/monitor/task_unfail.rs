//! TaskUnFail — Recover false failures by re-checking on-chain
//!
//! Re-checks recently failed transactions (within 30-minute window) to see if
//! they were actually mined despite the broadcast error report. If found on-chain,
//! recovers the transaction to 'completed' status.
//!
//! Ghost Transaction Safety:
//! - Only recovers if on-chain proof is confirmed (merkle path verified)
//! - Does NOT re-create deleted outputs — that's too dangerous. Instead, logs
//!   a warning so the user can investigate. The outputs from a recovered tx
//!   will be picked up by the next UTXO sync.
//!
//! Interval: 300 seconds (5 minutes)

use actix_web::web;
use log::{info, warn};
use std::time::Duration;

use crate::AppState;
use crate::database::{TransactionRepository, OutputRepository, ProvenTxReqRepository};
use crate::proof_store::{ProofCheck, VerifiedRow};

/// Only check transactions failed within this window (seconds)
/// Must be long enough for orphan mempool txs to get mined.
/// BSV node orphan pool expires after 20 min, but miners may have the tx
/// from other broadcasters. 6 hours matches UNPROVEN_TIMEOUT_SECS.
const UNFAIL_WINDOW_SECS: i64 = 6 * 60 * 60; // 6 hours

/// Run the TaskUnFail task
pub async fn run(state: &web::Data<AppState>, client: &reqwest::Client) -> Result<(), String> {
    // Get recently failed transactions within the UnFail window
    let failed_txs: Vec<(i64, String)> = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let conn = db.connection();

        let unfail_cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64 - UNFAIL_WINDOW_SECS;

        let mut stmt = conn.prepare(
            "SELECT id, txid FROM transactions
             WHERE status = 'failed'
             AND failed_at IS NOT NULL
             AND failed_at >= ?1
             AND txid NOT LIKE 'unbuilt-%'"
        ).map_err(|e| format!("SQL prepare: {}", e))?;

        let rows = stmt.query_map(
            rusqlite::params![unfail_cutoff],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| format!("SQL query: {}", e))?
        .filter_map(|r| r.ok())
        .collect();
        rows
    };

    if failed_txs.is_empty() {
        return Ok(());
    }

    info!("🔍 TaskUnFail: checking {} recently failed transaction(s)...", failed_txs.len());

    let mut recovered_count = 0;

    for (_tx_id, txid) in &failed_txs {
        let short_txid = &txid[..txid.len().min(16)];

        // Step 1: a stored proof recovers it only if our own headers verify it now (B5-T1-P7a-2
        // `C7`: a row's existence is not evidence — it can come from an import or an older build).
        let already_proven = {
            let db = match state.database.lock() {
                Ok(g) => g,
                Err(e) => { warn!("   ⚠️ DB lock: {}", e); continue; }
            };
            crate::proof_store::stored_verified_proof(db.connection(), &*state.chain_tracker, txid).ok().flatten()
        };

        if let Some(row) = already_proven {
            info!("   ✅ {} has a verified proof! Recovering from failed → completed", short_txid);
            recover_transaction(state, txid, row);
            recovered_count += 1;
            continue;
        }

        // Step 2: Query via Services chain (ARC GP → WoC → JungleBus → Bitails).
        // Only ARC populates `merkle_path_bump` from tx_status; for the other tiers the
        // proof is fetched from the proof chain (`/code-review xhigh 167` #8).
        match state.services.tx_status(txid).await {
            Ok(status) => {
                use crate::services::TxState;
                if matches!(status.state, TxState::Mined) {
                    info!("   ⛏️ {} found MINED via Services chain — checking its proof...", short_txid);

                    let checked = match status.merkle_path_bump {
                        Some(ref merkle_path_hex) => Some((
                            crate::proof_store::check_bump_hex(&*state.chain_tracker, txid, merkle_path_hex),
                            status.block_hash.clone().unwrap_or_default(),
                        )),
                        None => match crate::cache_helpers::fetch_tsc_proof_from_api(&state.services, txid).await {
                            Ok(Some(tsc)) => Some((
                                crate::proof_store::check_tsc(&*state.chain_tracker, txid, &tsc),
                                tsc["target"].as_str().unwrap_or("").to_string(),
                            )),
                            Ok(None) => { info!("   ⏳ {} mined but no proof published yet", short_txid); None }
                            Err(e) => { warn!("   ⚠️ proof fetch for {} failed: {}", short_txid, e); None }
                        },
                    };
                    if let Some((check, block_hash)) = checked {
                        if let Some(row) = store_verified(state, txid, check, None, &block_hash) {
                            if recover_transaction(state, txid, row) {
                                recovered_count += 1;
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // Step 3: Fallback to WhatsOnChain
                let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/hash/{}", txid);
                if let Ok(response) = client.get(&url).timeout(Duration::from_secs(15)).send().await {
                    if response.status().is_success() {
                        if let Ok(json) = response.json::<serde_json::Value>().await {
                            let confirmations = json["confirmations"].as_u64().unwrap_or(0);
                            if confirmations > 0 {
                                info!("   ⛏️ {} found confirmed on WhatsOnChain! Recovering...", short_txid);

                                // Fetch the proof (Services chain) and recover only if our headers verify it.
                                match crate::cache_helpers::fetch_tsc_proof_from_api(&state.services, txid).await {
                                    Ok(Some(tsc)) => {
                                        let block_hash = tsc["target"].as_str().unwrap_or("").to_string();
                                        let check = crate::proof_store::check_tsc(&*state.chain_tracker, txid, &tsc);
                                        if let Some(row) = store_verified(state, txid, check, None, &block_hash) {
                                            if recover_transaction(state, txid, row) {
                                                recovered_count += 1;
                                            }
                                        }
                                    }
                                    Ok(None) => info!("   ⏳ {} confirmed but no proof published yet", short_txid),
                                    Err(e) => warn!("   ⚠️ proof fetch for {} failed: {}", short_txid, e),
                                }
                            }
                        }
                    }
                }
            }
        }

        // Rate limiting
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    if recovered_count > 0 {
        info!("✅ TaskUnFail: recovered {} transaction(s) from false failure", recovered_count);
        super::log_monitor_event(state, "TaskUnFail:recovered", Some(&format!("{} recovered", recovered_count)));
    }

    Ok(())
}

/// Recover a failed transaction that was actually mined. Returns `true` iff the
/// recovery transaction committed (callers that must know — e.g. reconcile's promotion
/// path — rely on this; the Monitor callers ignore it).
pub(crate) fn recover_transaction(state: &AppState, txid: &str, row: VerifiedRow) -> bool {
    let height = row.height();
    let committed = if let Ok(db) = state.database.lock() {
        let conn = db.connection();
        let short_txid = &txid[..txid.len().min(16)];

        // Per-tx atomicity (OD-2 commit 2): wrap the whole recovery (status update, proof
        // link, input re-mark, output re-enable) in ONE transaction so an unclean kill
        // mid-sequence rolls back rather than leaving a partially-recovered tx. Existing
        // best-effort sub-step handling is preserved; any early `return` rolls back.
        let tx = match conn.unchecked_transaction() {
            Ok(t) => t,
            Err(e) => { warn!("   ⚠️ Failed to begin recovery txn for {}: {}", short_txid, e); return false; }
        };

        // Update transaction status to completed
        let tx_repo = TransactionRepository::new(&tx);
        if let Err(e) = tx_repo.update_broadcast_status(txid, "confirmed") {
            warn!("   ⚠️ Failed to recover tx {}: {}", short_txid, e);
            return false;
        }
        if let Err(e) = tx_repo.update_confirmations(txid, 1, Some(height)) {
            warn!("   ⚠️ Failed to update confirmations for {}: {}", short_txid, e);
        }

        // Link the verified proof (and the proven_tx_reqs row, if any)
        let _ = crate::proof_store::link(&tx, &row, txid);
        let req_repo = ProvenTxReqRepository::new(&tx);
        if let Ok(Some(req)) = req_repo.get_by_txid(txid) {
            let _ = req_repo.add_history_note(
                req.proven_tx_req_id,
                "completed",
                &format!("UnFail recovery: tx found on-chain at height {}", height),
            );
        }

        // Re-mark inputs as spent by this transaction.
        // When mark_failed ran, it restored inputs as "spendable" and NULLed
        // spending_description. Now that the tx is confirmed on-chain, those
        // inputs are actually spent. Parse raw_tx to find which inputs to re-mark.
        let output_repo = OutputRepository::new(&tx);
        let tx_repo_inner = TransactionRepository::new(&tx);
        if let Ok(Some(stored_tx)) = tx_repo_inner.get_by_txid(txid) {
            match crate::transaction::extract_input_outpoints(&stored_tx.raw_tx) {
                Ok(input_outpoints) => {
                    let mut re_spent_count = 0;
                    for (prev_txid, prev_vout) in &input_outpoints {
                        // First: clear any 'dss:*' suspected marking (spendable=0, so mark_spent won't match).
                        // This handles the case where inputs were marked suspected by TaskSendWaiting
                        // but the tx was actually mined.
                        let dss_cleared = tx.execute(
                            "UPDATE outputs SET spendable = 1, spending_description = NULL
                             WHERE txid = ?1 AND vout = ?2 AND spending_description LIKE 'dss:%'",
                            rusqlite::params![prev_txid, *prev_vout],
                        ).unwrap_or(0);

                        match output_repo.mark_spent(prev_txid, *prev_vout, txid) {
                            Ok(n) if n > 0 => re_spent_count += n,
                            _ => {
                                if dss_cleared > 0 {
                                    // The dss clear + mark_spent should have worked together
                                    re_spent_count += dss_cleared;
                                }
                            }
                        }
                    }
                    if re_spent_count > 0 {
                        info!("   🔒 Re-marked {} input(s) as spent by recovered tx {}", re_spent_count, short_txid);
                    }
                }
                Err(e) => {
                    warn!("   ⚠️ Failed to parse raw_tx inputs for {}: {} — run /wallet/sync to reconcile", short_txid, e);
                }
            }
        }

        // Re-enable change outputs that were disabled during false failure cleanup.
        // When mark_failed ran, it set spendable=0 and spending_description='failed-tx-output'.
        // Now that the tx is confirmed on-chain, those outputs are real — flip them back.
        match output_repo.reenable_failed_outputs(txid) {
            Ok(count) if count > 0 => {
                info!("   ♻️ Re-enabled {} change output(s) from recovered tx {}", count, short_txid);
            }
            Err(e) => {
                warn!("   ⚠️ Failed to re-enable outputs for {}: {} — run /wallet/sync to reconcile", short_txid, e);
            }
            _ => {}
        }

        // Only log success if the txn actually committed — on commit failure the
        // recovery rolled back (heals next cycle), so don't claim "recovered".
        match tx.commit() {
            Ok(()) => { info!("   ✅ Recovered tx {} → completed (proof at height {})", short_txid, height); true }
            Err(e) => { warn!("   ⚠️ Recovery txn for {} failed to commit — rolled back, will retry next cycle: {}", short_txid, e); false }
        }
    } else {
        false
    };

    // Invalidate balance cache since status may have changed.
    state.balance_cache.invalidate();
    committed
}

/// Store a checked proof through the one verified seam (B5-T1-P7a-2 `C6`). `Some` only when our own
/// headers verified it; anything else stores nothing, and the transaction stays as it is.
fn store_verified(state: &AppState, txid: &str, check: ProofCheck, raw: Option<&[u8]>, block_hash: &str) -> Option<VerifiedRow> {
    let short = &txid[..txid.len().min(16)];
    let proof = match check {
        ProofCheck::Verified(p) => p,
        ProofCheck::NotHeld => { info!("   ⏳ {} proof: block not in our headers yet — not recovered", short); return None; }
        ProofCheck::Wrong { height } => { warn!("   ❌ {} proof does not match our header at {} — not recovered", short, height); return None; }
        ProofCheck::Malformed(e) => { warn!("   ⚠️ {} proof unreadable: {}", short, e); return None; }
    };
    let db = state.database.lock().ok()?;
    let conn = db.connection();
    let raw_tx_bytes: Vec<u8> = match raw {
        Some(r) => r.to_vec(),
        None => match TransactionRepository::new(conn).get_by_txid(txid) {
            Ok(Some(stored)) => hex::decode(&stored.raw_tx).unwrap_or_default(),
            _ => Vec::new(),
        },
    };
    match crate::proof_store::store(conn, &*state.chain_tracker, &proof, &raw_tx_bytes, block_hash) {
        Ok(row) => Some(row),
        Err(e) => { warn!("   ⚠️ storing proof for {} failed: {}", short, e); None }
    }
}

/// Promote a mined-but-not-`completed` LOCAL transaction to completed, reusing the
/// UnFail recovery path.
///
/// Used by spent-input reconcile (WS1). When a phantom's successor `Y` is one of our
/// OWN transactions still marked `failed` (a broadcast false-negative aged past the
/// UnFail window — the exact origin of the phantom), marking the phantom
/// `spent_by = Y.id` would be reverted within ~60s by `TaskReviewStatus` step 3, which
/// restores the inputs of `failed` txs. Promoting `Y` first — using the merkle proof
/// reconcile already fetched and validated — re-marks Y's inputs spent (including the
/// phantom) AND fixes Y's status at the root, so the mark sticks.
///
/// Returns `true` iff it promoted a local tx. `false` means `txid` is foreign (not in
/// our `transactions` table) or already `completed`; the caller then marks explicitly.
///
/// `tsc` is the TSC merkle proof (as from `services.get_merkle_proof_tsc`), checked again here
/// against our own headers (B5-T1-P7a-2); `raw` is the txid-verified raw tx.
/// How the reconcile caller should treat the mark of the input a successor spent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromoteOutcome {
    /// Local tx promoted + committed → `recover_transaction` already marked its inputs
    /// spent (including the phantom) and set it `completed`; the caller credits the mark.
    Promoted,
    /// Foreign tx, or already `completed` → the caller marks the input explicitly
    /// (`spent_by` stays NULL for a foreign txid, or the tx is already completed — both
    /// step-3-revert-safe).
    NotApplicable,
    /// Local tx that could NOT be committed (serialize/insert/commit rolled back) → the
    /// caller must NOT mark the input: explicit-marking a still-non-`completed` local tx
    /// could be reverted by `TaskReviewStatus` step 3. Leave the phantom for the next pass.
    TransientFailed,
}

pub(crate) fn promote_proven_local_tx(
    state: &AppState,
    txid: &str,
    tsc: &serde_json::Value,
    raw: &[u8],
) -> PromoteOutcome {
    let short = &txid[..txid.len().min(16)];

    // Act only on a LOCAL, non-completed tx (foreign / already-completed → caller marks explicitly).
    let promotable = match state.database.lock() {
        Ok(db) => {
            let tx_repo = TransactionRepository::new(db.connection());
            matches!(
                tx_repo.get_transaction_status(txid),
                Ok(Some(status)) if status != crate::action_storage::TransactionStatus::Completed
            )
        }
        Err(_) => false,
    };
    if !promotable {
        return PromoteOutcome::NotApplicable;
    }

    // B5-T1-P7a-2 `C8`: promoted only on a proof our own headers verify, stored at the height our
    // chain gives it (a WhatsOnChain proof names its block by hash, never by height — not 0).
    let check = crate::proof_store::check_tsc(&*state.chain_tracker, txid, tsc);
    let block_hash = tsc["target"].as_str().unwrap_or("").to_string();
    let Some(row) = store_verified(state, txid, check, Some(raw), &block_hash) else {
        return PromoteOutcome::TransientFailed;
    }; // lock dropped before recover_transaction re-locks

    if recover_transaction(state, txid, row) {
        info!("   ⤴️  reconcile-promote: {} was mined but not completed → recovered to completed", short);
        PromoteOutcome::Promoted
    } else {
        // Rolled back — leave it non-completed for a later reconcile pass.
        PromoteOutcome::TransientFailed
    }
}
