//! TaskCheckForProofs — Acquire merkle proofs for unproven transactions
//!
//! Replaces arc_status_poller.rs and cache_sync.rs.
//! Queries ARC and WhatsOnChain for merkle proofs, creates proven_txs records,
//! and updates transaction status to 'completed'.
//!
//! Interval: 60 seconds

use actix_web::web;
use log::{info, warn, error};
use std::time::Duration;

use crate::AppState;
use crate::database::{TransactionRepository, OutputRepository, ProvenTxReqRepository, ParentTransactionRepository, BlockHeaderRepository};

/// Maximum number of transactions to check per cycle (rate limit protection)
const MAX_BATCH: usize = 20;

/// Timeout for transactions WE broadcast (unproven/sending) - 6 hours
const UNPROVEN_TIMEOUT_SECS: i64 = 6 * 60 * 60;

/// Threshold after which a tx that three independent txid-based indexers
/// (WhatsOnChain, JungleBus/GorillaPool, Bitails) ALL return 404 for is
/// considered failed. Each indexer covers both mempool and chain; a
/// three-way quorum 404 after this window is strong evidence the tx was
/// never successfully broadcast (e.g. process killed mid-broadcast) or
/// dropped from every mempool. TaskUnFail's 6h recovery window catches
/// false positives: if we prematurely fail a tx that later appears on
/// chain, inputs are re-reserved and proof is linked.
///
/// 5 min × 60s tick × 3 oracles = up to 15 independent "not found" signals
/// before rollback. Well above typical mempool propagation (<30s).
const ALL_ORACLES_NOT_FOUND_TIMEOUT_SECS: i64 = 5 * 60;

/// Timeout for transactions the APP broadcasts (nosend) - 10 minutes
/// Apps broadcast immediately after receiving the Atomic BEEF from createAction.
/// If the tx isn't on ARC or WoC within 10 minutes, the broadcast likely failed.
const NOSEND_TIMEOUT_SECS: i64 = 10 * 60;

/// After this many seconds in mempool, cross-verify with WhatsOnChain
const MEMPOOL_VERIFY_THRESHOLD_SECS: i64 = 30 * 60;

/// ANNOUNCED_TO_NETWORK is a weak signal — ARC told peers but they may not
/// have accepted. Cross-verify with WoC much sooner than the standard 30 min.
const ANNOUNCED_VERIFY_THRESHOLD_SECS: i64 = 3 * 60;

// ORPHAN_TIMEOUT_SECS removed — orphan/stale txs now fail immediately

struct PendingTxInfo {
    txid: String,
    status: String,
    age_secs: i64,
    /// An indexer has reported it mined (`confirmations > 0`). Such a transaction is never failed by a
    /// timeout: it waits, `unproven`, for a proof our own headers verify (B5-T1-P7a-2 `C21`).
    reported_mined: bool,
}

/// Run the TaskCheckForProofs task
pub async fn run(state: &web::Data<AppState>, client: &reqwest::Client) -> Result<(), String> {
    // Step 1: Get transactions needing proof
    let pending_txs: Vec<PendingTxInfo> = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let conn = db.connection();

        let mut stmt = conn.prepare(
            "SELECT txid, status, (strftime('%s', 'now') - created_at) as age_secs, confirmations > 0
             FROM transactions
             WHERE status IN ('sending', 'unproven', 'nosend')
             ORDER BY confirmations > 0, created_at DESC LIMIT ?1"
        ).map_err(|e| format!("SQL prepare: {}", e))?;

        let rows = stmt.query_map(
            rusqlite::params![MAX_BATCH as i64],
            |row| Ok(PendingTxInfo {
                txid: row.get(0)?,
                status: row.get(1)?,
                age_secs: row.get(2)?,
                reported_mined: row.get(3)?,
            }),
        ).map_err(|e| format!("SQL query: {}", e))?;

        rows.filter_map(|r| r.ok()).collect()
    };

    if pending_txs.is_empty() {
        return Ok(());
    }

    info!("🔍 TaskCheckForProofs: checking {} pending transactions...", pending_txs.len());

    let mut confirmed_count = 0;

    for tx_info in &pending_txs {
        let txid = &tx_info.txid;
        let is_nosend = tx_info.status == "nosend";
        let timeout_secs = if is_nosend { NOSEND_TIMEOUT_SECS } else { UNPROVEN_TIMEOUT_SECS };
        let is_timed_out = tx_info.age_secs > timeout_secs;

        // A proof stored by another path counts only if our own headers verify it now (B5-T1-P7a-2:
        // a row can come from an import, a restored file or an older build).
        let already_proven = {
            let db = match state.database.lock() {
                Ok(g) => g,
                Err(e) => { warn!("   ⚠️ DB lock failed: {}", e); continue; }
            };
            crate::proof_store::stored_verified_proof(db.connection(), &*state.chain_tracker, txid).ok().flatten()
        };

        if let Some(row) = already_proven {
            info!("   ✅ {} already proven (ID {}), reconciling", txid, row.proven_tx_id());
            reconcile_proven_tx(state, txid, row);
            confirmed_count += 1;
            continue;
        }

        // Hard timeout backstop — any tx past UNPROVEN_TIMEOUT (6h) or NOSEND_TIMEOUT (10m)
        // gets one final oracle check, then force-fail. Catches any status that slipped
        // through the normal handling paths (e.g. ARC returning mempool status indefinitely).
        // ⛔ Never for a transaction an indexer reported mined: under "completed only with a verified
        // proof" it stays unproven until our headers hold its block, and failing it would take its
        // coins out of the balance and make inputs the chain has spent selectable again (`C21`).
        let provider_status = state.services.tx_status(txid).await;
        let provider_says_mined = matches!(provider_status, Ok(ref t) if t.state == crate::services::TxState::Mined);
        // Reported mined before, not now (a reorg, or a wrong report): only every oracle's
        // authoritative "not found" fails it; an undecided vote waits (adversarial review: the
        // exemption must not be a permanent latch).
        if is_timed_out && tx_info.reported_mined && !provider_says_mined {
            match oracle_quorum_check(client, txid).await {
                OracleVerdict::AllNotFound => {
                    warn!("   ❌ {} was reported mined, now not found on any oracle past its timeout — marking failed",
                          &txid[..txid.len().min(16)]);
                    mark_failed(state, txid);
                    continue;
                }
                OracleVerdict::Present { any_confirmed: true } => {
                    // Still on chain: ask the proof chain (xhigh 168 #2).
                    if let Some(count) = try_whatsonchain_confirmation(state, client, txid).await {
                        confirmed_count += count;
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                }
                other => {
                    info!("   ⏳ {} reported mined earlier; no authoritative answer now ({:?}) — waiting",
                          &txid[..txid.len().min(16)], other);
                    // The tick ends here: nothing else may fail it on this pass (fix review).
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                }
            }
        }
        if is_timed_out && !tx_info.reported_mined && !provider_says_mined {
            let hours = tx_info.age_secs / 3600;
            let mins = (tx_info.age_secs % 3600) / 60;
            warn!("   ⏰ {} exceeded timeout ({}h{}m) — final oracle check",
                  &txid[..txid.len().min(16)], hours, mins);
            match oracle_quorum_check(client, txid).await {
                OracleVerdict::Present { any_confirmed: true } => {
                    if let Some(count) = try_whatsonchain_confirmation(state, client, txid).await {
                        confirmed_count += count;
                    } else {
                        info!("   ⏳ {} oracle saw confirmed but proof fetch lagged — retrying next tick",
                              &txid[..txid.len().min(16)]);
                    }
                }
                OracleVerdict::Present { any_confirmed: false } => {
                    // In someone's mempool but past timeout — still fail
                    warn!("   ❌ {} in mempool but past timeout — marking failed",
                          &txid[..txid.len().min(16)]);
                    mark_failed(state, txid);
                }
                OracleVerdict::AllNotFound => {
                    warn!("   ❌ {} timed out and not found on any oracle — marking failed",
                          &txid[..txid.len().min(16)]);
                    mark_failed(state, txid);
                }
                OracleVerdict::Inconclusive(reasons) => {
                    warn!("   ⚠️ {} timed out but oracle quorum inconclusive ({}) — forcing failure",
                          &txid[..txid.len().min(16)], reasons);
                    mark_failed(state, txid);
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        }

        // For nosend txs older than 60s: the app should have broadcast by now.
        // Check WoC directly — if not found, the app's broadcast likely failed.
        // This catches ARC 409 DeadlineExceeded failures where the app thinks it
        // broadcast but the tx never actually made it to miners.
        if is_nosend && tx_info.age_secs > 60 && tx_info.age_secs < timeout_secs {
            let woc_confirmed = try_whatsonchain_confirmation(state, client, txid).await;
            if let Some(count) = woc_confirmed {
                confirmed_count += count;
                continue;
            }
            // Not on WoC yet — still check ARC below (it might be in mempool)
        }

        // Query via Services chain (ARC GP → WoC → JungleBus → Bitails). When ARC
        // responds, `raw_provider_status` carries ARC's full vocabulary
        // (ANNOUNCED_TO_NETWORK, SEEN_IN_ORPHAN_MEMPOOL, etc.). When a fallback
        // tier responds, that field is `None` and we map `TxState` → a synthetic
        // ARC-style label so the rich match below still works.
        match provider_status {
            Ok(tx_status) => {
                use crate::services::TxState;
                let status: &str = match tx_status.raw_provider_status.as_deref() {
                    Some(s) => s,
                    None => match tx_status.state {
                        TxState::Mined => "MINED",
                        TxState::InMempool => "SEEN_ON_NETWORK",
                        TxState::Rejected => "REJECTED",
                        TxState::DoubleSpendAttempted => "DOUBLE_SPEND_ATTEMPTED",
                        TxState::Unknown => "UNKNOWN",
                    },
                };

                match status {
                    "MINED" => {
                        let block_height = tx_status.block_height.unwrap_or(0);
                        info!("   ⛏️ {} MINED (block {})", txid, block_height);

                        let verified = match tx_status.merkle_path_bump {
                            Some(ref hex) => store_checked_proof(
                                state, txid,
                                crate::proof_store::check_bump_hex(&*state.chain_tracker, txid, hex),
                                tx_status.block_hash.as_deref().unwrap_or(""), "ARC",
                            ),
                            // Only ARC sends a BUMP with its status: ask the proof chain otherwise —
                            // when our chain holds that block (an older block cannot be checked
                            // before P7a-3; asking every tick would only spend indexer calls).
                            None if block_height == 0 || state.chain_tracker.header_for_height(block_height).is_some() =>
                                fetch_and_store_woc_proof(state, txid).await,
                            None => None,
                        };
                        settle_mined(state, txid, block_height, verified);
                        confirmed_count += 1;
                    }
                    "ANNOUNCED_TO_NETWORK" => {
                        // ANNOUNCED = weak signal (ARC told peers, but peers may not
                        // have accepted). Cross-verify sooner than standard mempool.
                        if tx_info.age_secs > ANNOUNCED_VERIFY_THRESHOLD_SECS {
                            if let Some(count) = try_whatsonchain_confirmation(state, client, txid).await {
                                confirmed_count += count;
                            } else {
                                // WoC doesn't have it — run three-oracle quorum
                                info!("   🔍 {} ANNOUNCED but WoC 404 after {}m — running oracle quorum",
                                      &txid[..txid.len().min(16)], tx_info.age_secs / 60);
                                match oracle_quorum_check(client, txid).await {
                                    OracleVerdict::AllNotFound => {
                                        warn!("   🧹 {} ANNOUNCED but 404 on all oracles after {}m — rolling back",
                                              &txid[..txid.len().min(16)], tx_info.age_secs / 60);
                                        mark_failed(state, txid);
                                    }
                                    OracleVerdict::Present { any_confirmed } => {
                                        if any_confirmed {
                                            if let Some(count) = try_whatsonchain_confirmation(state, client, txid).await {
                                                confirmed_count += count;
                                            }
                                        } else {
                                            info!("   ⏳ {} seen in a mempool by oracle — awaiting mining",
                                                  &txid[..txid.len().min(16)]);
                                        }
                                    }
                                    OracleVerdict::Inconclusive(reasons) => {
                                        warn!("   ⚠️ {} oracle quorum inconclusive ({}), skipping tick",
                                              &txid[..txid.len().min(16)], reasons);
                                    }
                                }
                            }
                        } else {
                            info!("   ⏳ {} ANNOUNCED ({}s age, verifying at {}s)",
                                  &txid[..txid.len().min(16)], tx_info.age_secs, ANNOUNCED_VERIFY_THRESHOLD_SECS);
                        }
                    }
                    "SEEN_ON_NETWORK" | "REQUESTED_BY_NETWORK" | "SENT_TO_NETWORK"
                    | "ACCEPTED_BY_NETWORK" | "STORED" | "QUEUED" | "RECEIVED" => {
                        // Strong mempool signals — standard 30-min threshold
                        if tx_info.age_secs > MEMPOOL_VERIFY_THRESHOLD_SECS {
                            if let Some(count) = try_whatsonchain_confirmation(state, client, txid).await {
                                confirmed_count += count;
                            } else {
                                // WoC doesn't have it despite ARC mempool status — run oracle quorum
                                info!("   🔍 {} in mempool per ARC but WoC 404 after {}m — running oracle quorum",
                                      &txid[..txid.len().min(16)], tx_info.age_secs / 60);
                                match oracle_quorum_check(client, txid).await {
                                    OracleVerdict::AllNotFound => {
                                        warn!("   🧹 {} in mempool per ARC but 404 on all oracles after {}m — rolling back",
                                              &txid[..txid.len().min(16)], tx_info.age_secs / 60);
                                        mark_failed(state, txid);
                                    }
                                    OracleVerdict::Present { any_confirmed } => {
                                        if any_confirmed {
                                            if let Some(count) = try_whatsonchain_confirmation(state, client, txid).await {
                                                confirmed_count += count;
                                            }
                                        } else {
                                            info!("   ⏳ {} seen in a mempool by oracle — awaiting mining",
                                                  &txid[..txid.len().min(16)]);
                                        }
                                    }
                                    OracleVerdict::Inconclusive(reasons) => {
                                        warn!("   ⚠️ {} oracle quorum inconclusive ({}), skipping tick",
                                              &txid[..txid.len().min(16)], reasons);
                                    }
                                }
                            }
                        } else {
                            info!("   ⏳ {} in mempool ({})", &txid[..txid.len().min(16)], status);
                        }
                    }
                    "SEEN_IN_ORPHAN_MEMPOOL" => {
                        // Orphan mempool = BEEF ancestry validation failure.
                        // The orphan pool is a graveyard — ARC never re-processes.
                        // But check WoC first: if tx exists (even in mempool), don't fail.
                        warn!("   ⚠️ {} in orphan mempool — checking WoC before failing", &txid[..txid.len().min(16)]);

                        match check_whatsonchain_confirmation(client, txid).await {
                            Ok(Some((confirmations, block_height))) => {
                                if confirmations > 0 {
                                    info!("   ✅ {} confirmed on WoC despite orphan mempool status", &txid[..txid.len().min(16)]);
                                    let verified = fetch_and_store_woc_proof(state, txid).await;
                                    settle_mined(state, txid, block_height.unwrap_or(0), verified);
                                    confirmed_count += 1;
                                } else {
                                    // In mempool — tx exists despite ARC orphan status. Wait.
                                    info!("   ⏳ {} in mempool despite orphan status — waiting", &txid[..txid.len().min(16)]);
                                }
                            }
                            Ok(None) => {
                                // 404 — genuinely not on chain
                                mark_failed(state, txid);
                            }
                            Err(_) => {
                                // WoC error — fail anyway for orphan (it's a graveyard)
                                mark_failed(state, txid);
                            }
                        }
                    }
                    "MINED_IN_STALE_BLOCK" => {
                        // Block was orphaned — tx may still be valid but needs re-broadcast.
                        warn!("   ⚠️ {} was in a stale block — marking failed for re-broadcast",
                              &txid[..txid.len().min(16)]);
                        mark_failed(state, txid);
                    }
                    "DOUBLE_SPEND_ATTEMPTED" => {
                        // ARC marks BOTH competing txs as DOUBLE_SPEND_ATTEMPTED during
                        // the first-seen conflict window. The winning tx will get mined.
                        // Cross-verify with WoC: if tx exists (even 0 confirmations in
                        // mempool), don't fail — it may still get mined. Only fail if
                        // WoC returns 404 (tx genuinely not found).
                        warn!("   ⚠️ {} DOUBLE_SPEND_ATTEMPTED — cross-verifying with WoC", &txid[..txid.len().min(16)]);
                        match check_whatsonchain_confirmation(client, txid).await {
                            Ok(Some((confirmations, block_height))) => {
                                if confirmations > 0 {
                                    // Already confirmed — recover it
                                    info!("   ✅ {} actually mined despite DOUBLE_SPEND_ATTEMPTED ({} confirmations)",
                                          &txid[..txid.len().min(16)], confirmations);
                                    let verified = fetch_and_store_woc_proof(state, txid).await;
                                    settle_mined(state, txid, block_height.unwrap_or(0), verified);
                                    confirmed_count += 1;
                                } else {
                                    // In mempool (0 confirmations) — tx exists, don't fail.
                                    // It's competing but may still win and get mined.
                                    info!("   ⏳ {} in mempool despite DOUBLE_SPEND_ATTEMPTED — waiting for resolution",
                                          &txid[..txid.len().min(16)]);
                                }
                            }
                            Ok(None) => {
                                // 404 — tx genuinely not found on WoC
                                warn!("   ❌ {} not found on WoC (404) — marking failed", &txid[..txid.len().min(16)]);
                                mark_failed(state, txid);
                            }
                            Err(e) => {
                                // WoC API error — skip this tick, don't make irreversible decisions
                                warn!("   ⚠️ {} WoC check failed ({}), skipping tick", &txid[..txid.len().min(16)], e);
                            }
                        }
                    }
                    "REJECTED" => {
                        warn!("   ⚠️ {} status: REJECTED — marking failed", txid);
                        mark_failed(state, txid);
                    }
                    other => {
                        info!("   ℹ️ {} status: {}", txid, other);
                    }
                }
            }
            Err(e) => {
                // ARC gave us nothing useful (404 or 5xx or transport error).
                // Fall over to a three-oracle quorum: WhatsOnChain, JungleBus
                // (GorillaPool non-ARC), and Bitails — all keyless, all indexing
                // both mempool and chain by txid. ARC's health is no longer a
                // prerequisite for detecting a failed broadcast.
                let err_str = e.to_string();
                info!("   ℹ️ {} chain exhausted ({}), running oracle quorum",
                      &txid[..txid.len().min(16)],
                      err_str.lines().next().unwrap_or(&err_str));
                match oracle_quorum_check(client, txid).await {
                    OracleVerdict::Present { any_confirmed } => {
                        if any_confirmed {
                            // At least one oracle reports confirmations. Try to fetch
                            // the merkle proof via WoC (same path ARC-OK/MINED uses
                            // when we have no ARC BUMP).
                            if let Some(count) = try_whatsonchain_confirmation(state, client, txid).await {
                                confirmed_count += count;
                            } else {
                                // Oracle saw it mined but WoC proof fetch raced or
                                // /proof/tsc lagged — leave for next tick.
                                info!("   ⏳ {} oracle saw confirmed but proof fetch lagged — retrying next tick",
                                      &txid[..txid.len().min(16)]);
                            }
                        } else {
                            info!("   ⏳ {} seen in a mempool — awaiting mining",
                                  &txid[..txid.len().min(16)]);
                        }
                    }
                    OracleVerdict::AllNotFound => {
                        if tx_info.age_secs > ALL_ORACLES_NOT_FOUND_TIMEOUT_SECS {
                            let mins = tx_info.age_secs / 60;
                            warn!("   🧹 {} 404 on WoC, JungleBus, and Bitails after {}m — rolling back",
                                  &txid[..txid.len().min(16)], mins);
                            mark_failed(state, txid);
                        } else {
                            let remaining = (ALL_ORACLES_NOT_FOUND_TIMEOUT_SECS - tx_info.age_secs).max(0) / 60;
                            info!("   ⏳ {} not found on any oracle yet ({}m age, rolling back in ~{}m)",
                                  &txid[..txid.len().min(16)], tx_info.age_secs / 60, remaining);
                        }
                    }
                    OracleVerdict::Inconclusive(reasons) => {
                        warn!("   ⚠️ {} oracle quorum inconclusive ({}), skipping tick",
                              &txid[..txid.len().min(16)], reasons);
                    }
                }
            }
        }

        // Rate limiting
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    if confirmed_count > 0 {
        info!("✅ TaskCheckForProofs: {} transactions confirmed", confirmed_count);
        super::log_monitor_event(state, "TaskCheckForProofs:completed", Some(&format!("{} confirmed", confirmed_count)));
    }

    Ok(())
}

/// Try to confirm a transaction via WhatsOnChain, returning Some(1) if confirmed
async fn try_whatsonchain_confirmation(
    state: &web::Data<AppState>,
    client: &reqwest::Client,
    txid: &str,
) -> Option<usize> {
    match check_whatsonchain_confirmation(client, txid).await {
        Ok(Some((confirmations, block_height))) if confirmations > 0 => {
            info!("   ⛏️ {} confirmed on WhatsOnChain ({} confirmations)", &txid[..txid.len().min(16)], confirmations);

            let verified = fetch_and_store_woc_proof(state, txid).await;
            settle_mined(state, txid, block_height.unwrap_or(0), verified);
            Some(1)
        }
        Ok(Some(_)) => {
            // 0 confirmations — still in mempool
            None
        }
        Ok(None) => {
            // Not found on WhatsOnChain
            None
        }
        Err(e) => {
            warn!("   ⚠️ WhatsOnChain check failed for {}: {}", txid, e);
            None
        }
    }
}

/// An indexer says `txid` is mined. It becomes `completed` only with a proof our own headers verified
/// (owner decision 6, B5-T1-P7a-2 `C14`); otherwise it is `unproven` — its coins count in the balance,
/// and the next pass asks for the proof again. `confirmations` records the indexer's report, which
/// keeps the timeout from failing it (`C21`).
fn settle_mined(state: &web::Data<AppState>, txid: &str, block_height: u32,
                verified: Option<crate::proof_store::VerifiedRow>) {
    if let Ok(db) = state.database.lock() {
        let conn = db.connection();
        let tx_repo = TransactionRepository::new(conn);
        let height = verified.map(|r| r.height()).or((block_height > 0).then_some(block_height));
        if let Err(e) = tx_repo.update_confirmations(txid, 1, height) {
            warn!("   ⚠️ Failed to update confirmations for {}: {}", txid, e);
        }
        match verified {
            Some(row) => {
                if let Err(e) = crate::proof_store::link(conn, &row, txid) {
                    warn!("   ⚠️ Failed to link proof for {}: {}", txid, e);
                }
                if let Err(e) = tx_repo.update_broadcast_status(txid, "confirmed") {
                    warn!("   ⚠️ Failed to update status for {}: {}", txid, e);
                }
            }
            None => {
                if matches!(tx_repo.get_broadcast_status(txid), Ok(Some(ref s)) if s == "sending" || s == "nosend") {
                    let _ = tx_repo.update_broadcast_status(txid, "broadcast"); // → unproven
                }
                info!("   ⏳ {} mined per indexer; unproven until our headers verify its proof", txid);
            }
        }
    }
    state.balance_cache.invalidate();
}

/// Mark a transaction as failed with full ghost output cleanup.
///
/// Ghost Transaction Safety:
/// Order: mark failed → delete ghost outputs → restore inputs → invalidate cache
fn mark_failed(state: &web::Data<AppState>, txid: &str) {
    let short_txid = &txid[..txid.len().min(16)];

    if let Ok(db) = state.database.lock() {
        let conn = db.connection();

        // 1. Mark as failed with timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        if let Err(e) = conn.execute(
            "UPDATE transactions SET status = 'failed', failed_at = ?1 WHERE txid = ?2 AND status != 'failed'",
            rusqlite::params![now, txid],
        ) {
            warn!("   ⚠️ Failed to mark {} as failed: {}", short_txid, e);
            return;
        }

        // 2. Disable (not delete) change outputs — TaskUnFail can re-enable if tx was actually mined
        let output_repo = OutputRepository::new(conn);
        match output_repo.disable_by_txid(txid) {
            Ok(count) if count > 0 => {
                info!("   🚫 Disabled {} output(s) from tx {} (recoverable by TaskUnFail)", count, short_txid);
            }
            Err(e) => {
                warn!("   ⚠️ Failed to disable outputs for {}: {}", short_txid, e);
            }
            _ => {}
        }

        // 3. Restore input outputs that were spent by this transaction
        match output_repo.restore_spent_by_txid(txid) {
            Ok(count) if count > 0 => {
                info!("   ♻️ Restored {} input(s) from tx {}", count, short_txid);
            }
            Err(e) => {
                warn!("   ⚠️ Failed to restore inputs for {}: {}", short_txid, e);
            }
            _ => {
                // Fallback for placeholder reservations
                if let Ok(count) = output_repo.restore_by_spending_description(txid) {
                    if count > 0 {
                        info!("   ♻️ Restored {} input(s) via spending_description from tx {}", count, short_txid);
                    }
                }
            }
        }

        info!("   ✅ Failed tx {} cleaned up (ghost outputs deleted, inputs restored)", short_txid);
    }

    // 4. Invalidate balance cache (outside DB lock)
    state.balance_cache.invalidate();
}

/// Reconcile a transaction whose stored proof our headers verify.
fn reconcile_proven_tx(state: &web::Data<AppState>, txid: &str, row: crate::proof_store::VerifiedRow) {
    if let Ok(db) = state.database.lock() {
        let conn = db.connection();

        let tx_repo = TransactionRepository::new(conn);
        if let Err(e) = tx_repo.update_broadcast_status(txid, "confirmed") {
            warn!("   ⚠️ Failed to update status for {}: {}", txid, e);
        }
        if let Err(e) = tx_repo.update_confirmations(txid, 1, Some(row.height())) {
            warn!("   ⚠️ Failed to update confirmations for {}: {}", txid, e);
        }
        if let Err(e) = crate::proof_store::link(conn, &row, txid) {
            warn!("   ⚠️ Failed to link proof for {}: {}", txid, e);
        }
        let req_repo = ProvenTxReqRepository::new(conn);
        if let Ok(Some(req)) = req_repo.get_by_txid(txid) {
            let _ = req_repo.add_history_note(req.proven_tx_req_id, "completed", "Reconciled from an existing proof, verified against our headers");
        }
    }
}

/// Store a checked proof through the one verified seam. `Some` only when our own headers verified it;
/// a proof for a block we do not hold yet, or one our header refutes, stores nothing and leaves the
/// `proven_tx_reqs` row open for the next pass (B5-T1-P7a-2: no "storing anyway").
fn store_checked_proof(
    state: &web::Data<AppState>,
    txid: &str,
    check: crate::proof_store::ProofCheck,
    block_hash: &str,
    source: &str,
) -> Option<crate::proof_store::VerifiedRow> {
    use crate::proof_store::ProofCheck;
    let proof = match check {
        ProofCheck::Verified(p) => p,
        ProofCheck::NotHeld => {
            info!("   ⏳ {} proof for {}: block not in our headers yet — kept pending", source, txid);
            return None;
        }
        ProofCheck::Wrong { height } => {
            warn!("   ❌ {} proof for {} does not match our header at {} — not stored", source, txid, height);
            return None;
        }
        ProofCheck::Malformed(e) => {
            warn!("   ⚠️ {} proof for {} unreadable: {}", source, txid, e);
            return None;
        }
    };
    let db = state.database.lock().ok()?;
    let conn = db.connection();
    let raw_tx_bytes: Vec<u8> = match TransactionRepository::new(conn).get_by_txid(txid) {
        Ok(Some(stored)) => hex::decode(&stored.raw_tx).unwrap_or_default(),
        _ => Vec::new(),
    };
    let row = match crate::proof_store::store(conn, &*state.chain_tracker, &proof, &raw_tx_bytes, block_hash) {
        Ok(r) => r,
        Err(e) => {
            warn!("   ⚠️ Failed to store proof for {}: {}", txid, e);
            return None;
        }
    };
    let req_repo = ProvenTxReqRepository::new(conn);
    if let Ok(Some(req)) = req_repo.get_by_txid(txid) {
        let _ = req_repo.add_history_note(
            req.proven_tx_req_id,
            "completed",
            &format!("Proof from {} at height {} verified against our headers", source, row.height()),
        );
    }
    Some(row)
}

/// Check WhatsOnChain for transaction confirmation status
async fn check_whatsonchain_confirmation(
    client: &reqwest::Client,
    txid: &str,
) -> Result<Option<(u32, Option<u32>)>, String> {
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/hash/{}", txid);

    let response = client.get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if response.status().as_u16() == 404 {
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(format!("WoC status: {}", response.status()));
    }

    let json: serde_json::Value = response.json()
        .await
        .map_err(|e| format!("JSON parse: {}", e))?;

    let confirmations = json["confirmations"].as_u64().unwrap_or(0) as u32;
    let block_height = json["blockheight"].as_u64().map(|h| h as u32);

    Ok(Some((confirmations, block_height)))
}

/// Fetch the merkle proof through the Services chain and store it if our headers verify it.
async fn fetch_and_store_woc_proof(state: &web::Data<AppState>, txid: &str) -> Option<crate::proof_store::VerifiedRow> {
    let tsc = match crate::cache_helpers::fetch_tsc_proof_from_api(&state.services, txid).await {
        Ok(Some(tsc)) => tsc,
        Ok(None) => {
            info!("   ⏳ no proof published yet for {}", txid);
            return None;
        }
        Err(e) => {
            warn!("   ⚠️ proof fetch failed for {}: {}", txid, e);
            return None;
        }
    };
    let block_hash = tsc["target"].as_str().unwrap_or("").to_string();
    let check = crate::proof_store::check_tsc(&*state.chain_tracker, txid, &tsc);
    store_checked_proof(state, txid, check, &block_hash, "the proof chain")
}

/// Per-oracle verdict for a single txid lookup.
#[derive(Debug)]
enum OracleStatus {
    /// Oracle returned 200 with a parsed response. `confirmations` is 0 for
    /// mempool-only visibility, >0 for on-chain.
    Present(u32),
    /// Oracle returned HTTP 404 — authoritative "not found in mempool or chain".
    NotFound,
    /// Anything else: 5xx, timeout, transport error, bad JSON. Cannot
    /// distinguish "unknown tx" from "oracle broken" — treat as no signal.
    Error(String),
}

/// Aggregate verdict across all queried oracles.
#[derive(Debug)]
enum OracleVerdict {
    /// At least one oracle saw the tx.
    Present { any_confirmed: bool },
    /// Every oracle returned an authoritative 404. Safe to mark failed after
    /// the age threshold.
    AllNotFound,
    /// Nothing authoritative either way (at least one oracle errored and none
    /// saw the tx). Skip this tick.
    Inconclusive(String),
}

async fn oracle_quorum_check(client: &reqwest::Client, txid: &str) -> OracleVerdict {
    let (woc, jb, bt) = tokio::join!(
        query_woc_txid(client, txid),
        query_junglebus_txid(client, txid),
        query_bitails_txid(client, txid),
    );

    let confirmations_if_present = |s: &OracleStatus| -> Option<u32> {
        if let OracleStatus::Present(c) = s { Some(*c) } else { None }
    };

    let present_confs: Vec<u32> = [&woc, &jb, &bt]
        .iter()
        .filter_map(|s| confirmations_if_present(s))
        .collect();

    if !present_confs.is_empty() {
        return OracleVerdict::Present {
            any_confirmed: present_confs.iter().any(|&c| c > 0),
        };
    }

    let all_not_found = matches!(woc, OracleStatus::NotFound)
        && matches!(jb, OracleStatus::NotFound)
        && matches!(bt, OracleStatus::NotFound);

    if all_not_found {
        return OracleVerdict::AllNotFound;
    }

    let mut reasons: Vec<String> = Vec::new();
    if let OracleStatus::Error(e) = &woc { reasons.push(format!("WoC:{}", e)); }
    if let OracleStatus::Error(e) = &jb { reasons.push(format!("JB:{}", e)); }
    if let OracleStatus::Error(e) = &bt { reasons.push(format!("BT:{}", e)); }
    OracleVerdict::Inconclusive(reasons.join("; "))
}

async fn query_woc_txid(client: &reqwest::Client, txid: &str) -> OracleStatus {
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/hash/{}", txid);
    match client.get(&url).timeout(Duration::from_secs(10)).send().await {
        Ok(resp) => {
            let code = resp.status().as_u16();
            if code == 404 { return OracleStatus::NotFound; }
            if !resp.status().is_success() {
                return OracleStatus::Error(format!("HTTP {}", code));
            }
            match resp.json::<serde_json::Value>().await {
                Ok(v) => OracleStatus::Present(v["confirmations"].as_u64().unwrap_or(0) as u32),
                Err(e) => OracleStatus::Error(format!("json:{}", e)),
            }
        }
        Err(e) => OracleStatus::Error(format!("transport:{}", e)),
    }
}

async fn query_junglebus_txid(client: &reqwest::Client, txid: &str) -> OracleStatus {
    let url = format!("https://junglebus.gorillapool.io/v1/transaction/get/{}", txid);
    match client.get(&url).timeout(Duration::from_secs(10)).send().await {
        Ok(resp) => {
            let code = resp.status().as_u16();
            if code == 404 { return OracleStatus::NotFound; }
            if !resp.status().is_success() {
                return OracleStatus::Error(format!("HTTP {}", code));
            }
            match resp.json::<serde_json::Value>().await {
                Ok(v) => {
                    // JungleBus returns { block_height: u64, ... }. 0 = mempool/unknown.
                    // We don't have current tip to compute exact confirmations; treat
                    // block_height > 0 as "confirmed" (conf >= 1).
                    let height = v["block_height"].as_u64().unwrap_or(0);
                    OracleStatus::Present(if height > 0 { 1 } else { 0 })
                }
                Err(e) => OracleStatus::Error(format!("json:{}", e)),
            }
        }
        Err(e) => OracleStatus::Error(format!("transport:{}", e)),
    }
}

async fn query_bitails_txid(client: &reqwest::Client, txid: &str) -> OracleStatus {
    let url = format!("https://api.bitails.io/tx/{}", txid);
    match client.get(&url).timeout(Duration::from_secs(10)).send().await {
        Ok(resp) => {
            let code = resp.status().as_u16();
            if code == 404 { return OracleStatus::NotFound; }
            if !resp.status().is_success() {
                return OracleStatus::Error(format!("HTTP {}", code));
            }
            match resp.json::<serde_json::Value>().await {
                Ok(v) => {
                    let height = v["blockHeight"].as_u64()
                        .or_else(|| v["block"]["height"].as_u64())
                        .unwrap_or(0);
                    OracleStatus::Present(if height > 0 { 1 } else { 0 })
                }
                Err(e) => OracleStatus::Error(format!("json:{}", e)),
            }
        }
        Err(e) => OracleStatus::Error(format!("transport:{}", e)),
    }
}
