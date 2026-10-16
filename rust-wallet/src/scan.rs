//! B5-T1-P6 — find late payments to the wallet's own receive addresses.
//!
//! **The scan** checks every receive address the wallet has handed out (BRC-42,
//! counterparty = self, `2-receive address-{i}`) and asks the chain whether unspent coins
//! the wallet does not already hold sit there. It exists because the wallet stops
//! watching an address after 90 days and a restore can drop address rows, so a payment
//! that arrives late would otherwise never reach the balance.
//!
//! Three callers, one scan and one writer: `POST /wallet/rescan` (the "Find late
//! payments" button), the on-chain restore, and the no-backup recovery.
//!
//! ⛔ **The rule the writer enforces** (SCOPE §5.2): a scan may only ever ADD a coin the
//! chain says is unspent and that we can prove we can sign for. It never marks a known
//! coin spendable again, and it never reports "none" where it could not ask.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use log::{info, warn};
use serde::Serialize;

use crate::database::WalletDatabase;
use crate::json_storage::AddressInfo;
use crate::reconcile::{SpentStatus, UnspentProbe};
use crate::recovery::RecoveryResult;
use crate::services::{BoxFut, WalletServices};
use crate::utxo_fetcher::UTXO;

/// `outputs.purpose` of every row the scan writes, so a rollback is a query (§7).
pub const SCAN_PURPOSE: &str = "address-scan";

/// The sentence the scan's response carries (and the Tools-tab card repeats — B5-T6-P5).
pub const SCAN_SELF_ONLY_NOTE: &str = "This finds payments to addresses this wallet generated. \
PeerPay payments are found through Claim a payment, not this scan.";

/// Every chain read the scan, its writer and the restore make (K10a). One live
/// implementation ([`LiveChain`]); tests script their own.
pub trait ScanChain: Send + Sync {
    /// UTXOs for a batch of addresses, plus the addresses that could not be read.
    fn fetch_batch(&self, batch: Vec<AddressInfo>) -> BoxFut<Result<(Vec<UTXO>, Vec<String>), String>>;
    /// Whether an address has ever had a transaction.
    fn has_history(&self, address: String) -> BoxFut<Result<bool, String>>;
    /// The raw bytes of a transaction (the caller checks they hash to the txid).
    fn raw_tx(&self, txid: String) -> BoxFut<Result<Vec<u8>, String>>;
    /// The chain's answer to "is this outpoint spent?" — `Unknown` means do nothing.
    fn spent(&self, txid: String, vout: u32) -> BoxFut<SpentStatus>;
    /// Whether `address`'s live unspent listing affirmatively contains the outpoint.
    fn unspent_listed(&self, address: String, txid: String, vout: u32) -> BoxFut<UnspentProbe>;
}

/// The production chain reads: today's fetchers and the Services raw-tx chain.
pub struct LiveChain {
    pub services: Arc<WalletServices>,
}

impl ScanChain for LiveChain {
    fn fetch_batch(&self, batch: Vec<AddressInfo>) -> BoxFut<Result<(Vec<UTXO>, Vec<String>), String>> {
        // The detailed fetch: its `failed` list is what keeps an outage "unchecked".
        Box::pin(async move { crate::utxo_fetcher::fetch_all_utxos_detailed(&batch).await })
    }
    fn has_history(&self, address: String) -> BoxFut<Result<bool, String>> {
        Box::pin(async move { crate::utxo_fetcher::address_has_history(&address).await })
    }
    fn raw_tx(&self, txid: String) -> BoxFut<Result<Vec<u8>, String>> {
        let services = self.services.clone();
        Box::pin(async move { services.get_raw_tx(&txid).await.map_err(|e| e.to_string()) })
    }
    fn spent(&self, txid: String, vout: u32) -> BoxFut<SpentStatus> {
        let client = self.services.client.clone();
        Box::pin(async move { crate::reconcile::check_outpoint_spent(&client, &txid, vout).await })
    }
    fn unspent_listed(&self, address: String, txid: String, vout: u32) -> BoxFut<UnspentProbe> {
        Box::pin(async move { crate::reconcile::check_outpoint_unspent(&address, &txid, vout).await })
    }
}

/// Waits before each retry of a refused chain read. ⚠️ WhatsOnChain rate-limits bursts
/// (HTTP 429 "Too Many Requests"): the scan's one-history-call-per-address past the mark
/// hit it on every live run (2026-10-09 sitting), and the parent-transaction fetch fell
/// through to a provider that returned junk. Same code path in tests, shorter waits.
const BACKOFF_MS: [u64; 3] = if cfg!(test) { [1, 2, 4] } else { [1_000, 2_000, 4_000] };

/// A read refused for rate limiting — worth waiting for, unlike a provider that is down.
fn rate_limited(e: &str) -> bool {
    e.contains("429") || e.to_ascii_lowercase().contains("too many requests")
}

/// The most a whole scan may spend waiting between retries (/code-review ultra #2): once
/// it is spent, a refused read is not retried — the address is reported unchecked or the
/// coin unverified, never waited on without end.
const RETRY_BUDGET_MS: u64 = if cfg!(test) { 10 } else { 60_000 };

/// What is left of one scan's retry waiting time.
pub(crate) struct RetryBudget(std::sync::atomic::AtomicU64);

impl RetryBudget {
    pub(crate) fn new(ms: u64) -> Self {
        Self(std::sync::atomic::AtomicU64::new(ms))
    }
    /// Take `ms` if that much is left.
    fn take(&self, ms: u64) -> bool {
        use std::sync::atomic::Ordering;
        self.0.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| left.checked_sub(ms)).is_ok()
    }
}

/// Run `op`; while it fails with an error `retry_on` accepts and the scan's `budget` still
/// covers the wait, wait (1, 2, 4 s) and try again. The last answer is returned as it is —
/// an `Err` stays an `Err`.
async fn with_backoff<T, F, Fut>(what: &str, retry_on: fn(&str) -> bool, budget: &RetryBudget, op: F) -> Result<T, String>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let mut last = op().await;
    for ms in BACKOFF_MS {
        match &last {
            Err(e) if retry_on(e) => {
                if !budget.take(ms) {
                    warn!("   🔎 {} refused ({}) — the scan's retry budget is spent; not retried", what, e);
                    break;
                }
                warn!("   🔎 {} refused ({}) — retrying in {} ms", what, e, ms);
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                last = op().await;
            }
            _ => break,
        }
    }
    last
}

/// The scan of the wallet's own receive addresses, through `chain`.
pub(crate) async fn scan_own_addresses(
    master_privkey: &[u8],
    master_pubkey: &[u8],
    high_water: u32,
    chain: &dyn ScanChain,
    budget: &RetryBudget,
) -> RecoveryResult {
    crate::recovery::scan_brc42_self(
        master_privkey,
        master_pubkey,
        high_water,
        |batch| chain.fetch_batch(batch),
        |address| async move {
            with_backoff("address history", rate_limited, budget, || chain.has_history(address.clone())).await
        },
    ).await
}

/// What the writer did with the scan's hits. Every hit lands in exactly one list.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct WriteReport {
    /// New coins written (`txid:vout`).
    pub written: Vec<String>,
    /// Hits the wallet already held — left exactly as they were.
    pub already_known: u32,
    /// The chain's script does not re-derive under the claimed index: not ours to sign.
    pub not_ours: Vec<String>,
    /// The chain says spent: never written.
    pub spent: Vec<String>,
    /// Could not be checked (script unreadable, or spent status unknown). Not written,
    /// and not "nothing there" — the next scan asks again.
    pub unverified: Vec<String>,
    /// Highest receive index at which the scan saw a coin (known or new).
    pub highest_index_seen: Option<u32>,
}

/// Write the coins `found` by the scan: the one writer all three callers share.
///
/// For each hit: (1) skip an outpoint the wallet already holds (any case); (2) read the
/// coin's real script from its parent transaction; (3) prove the script is ours — the
/// exact P2PKH of `2-receive address-{i}`, or a token whose lock is that P2PKH (K8);
/// (4) ask the chain — `Spent` is never written; `Unknown` is written only when the
/// address's live unspent listing affirmatively contains the coin (the rule
/// `reconcile_spent_inputs` follows: no provider ever answers an explicit "unspent" for a
/// plain P2PKH, measured 2026-10-07); (5) insert with the derivation that was
/// **verified**, txid lowercased, `purpose` = [`SCAN_PURPOSE`]; (6) classify it through
/// the same seam as any received coin.
pub(crate) async fn write_found_coins(
    db: &Mutex<WalletDatabase>,
    user_id: i64,
    wallet_id: i64,
    master_privkey: &[u8],
    master_pubkey: &[u8],
    found: &RecoveryResult,
    route: &str,
    chain: &dyn ScanChain,
    budget: &RetryBudget,
) -> WriteReport {
    let mut report = WriteReport::default();

    // Hits, txid lowercased (UNIQUE(txid, vout) is case-sensitive; indexers are not).
    let mut seen: HashSet<(String, u32)> = HashSet::new();
    let mut hits: Vec<(UTXO, u32)> = Vec::new();
    for addr in &found.addresses {
        report.highest_index_seen = report.highest_index_seen.max(Some(addr.index));
        for u in &addr.utxos {
            let mut u = u.clone();
            u.txid = u.txid.to_ascii_lowercase();
            if seen.insert((u.txid.clone(), u.vout)) {
                hits.push((u, addr.index));
            }
        }
    }

    // Every address the scan saw a coin at is ours (derived from our key): re-create its
    // row and watch it, so a coin left unverified here is picked up by the background
    // sync — through the same checks as any received coin — without another scan.
    {
        let Ok(d) = db.lock() else { return report };
        let indices: std::collections::BTreeSet<u32> = found.addresses.iter().map(|a| a.index).collect();
        for index in indices {
            ensure_receive_address_row(&d, wallet_id, index, master_privkey, master_pubkey);
        }
    }

    // (1) Known outpoints are never touched.
    let hits: Vec<(UTXO, u32)> = {
        let Ok(d) = db.lock() else { return report };
        hits.into_iter().filter(|(u, _)| {
            let known = d.connection().query_row(
                "SELECT 1 FROM outputs WHERE lower(txid) = ?1 AND vout = ?2",
                rusqlite::params![u.txid, u.vout as i64], |_| Ok(())).is_ok();
            if known { report.already_known += 1; }
            !known
        }).collect()
    };

    // (2) Real scripts, in chunks the observer will fetch in full.
    let mut observed: HashMap<(String, u32), crate::ingest::Observed> = HashMap::new();
    let txids: Vec<String> = hits.iter().map(|(u, _)| u.txid.clone())
        .collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    for chunk in txids.chunks(crate::ingest::OBSERVE_FETCH_BUDGET) {
        let coins: Vec<(String, u32, i64)> = hits.iter()
            .filter(|(u, _)| chunk.contains(&u.txid))
            .map(|(u, _)| (u.txid.clone(), u.vout, u.satoshis))
            .collect();
        observed.extend(crate::ingest::observe_coins(db, &coins, |t| async move {
            // Any failure is retried: the provider chain's last error need not say 429.
            with_backoff("parent transaction", |_| true, budget, || chain.raw_tx(t.clone())).await
        }).await);
    }

    for (u, index) in hits {
        let outpoint = format!("{}:{}", u.txid, u.vout);
        let Some(obs) = observed.get(&(u.txid.clone(), u.vout)) else {
            warn!("   🔎 scan[{}] {} — script could not be read; not written (unverified)", route, outpoint);
            report.unverified.push(outpoint);
            continue;
        };

        // (3) Ownership, against the chain's script — never the indexer's or a stored one.
        let Some(derived) = crate::reconcile::derive_receive_p2pkh_script(master_privkey, master_pubkey, index as i32) else {
            report.unverified.push(outpoint);
            continue;
        };
        let ours = obs.script == derived
            || crate::tokens::p2pkh_lock(&obs.script) == Some(&derived[3..23]);
        if !ours {
            warn!("   🔎 scan[{}] {} — chain script {} does not re-derive under index {} ({}); not written",
                  route, outpoint, hex::encode(&obs.script), index, hex::encode(&derived));
            report.not_ours.push(outpoint);
            continue;
        }

        // (4) Only a coin the chain proves unspent: an explicit answer, or — when the
        // spent check cannot say — the address's live unspent listing naming it.
        match chain.spent(u.txid.clone(), u.vout).await {
            SpentStatus::Unspent => {}
            SpentStatus::Spent { spending_txid } => {
                info!("   🔎 scan[{}] {} — spent by {} on chain; not written", route, outpoint, spending_txid);
                report.spent.push(outpoint);
                continue;
            }
            SpentStatus::Unknown => {
                let address = crate::reconcile::derive_receive_address(master_privkey, master_pubkey, index as i32)
                    .unwrap_or_default();
                match chain.unspent_listed(address, u.txid.clone(), u.vout).await {
                    UnspentProbe::Present => {}
                    probe => {
                        warn!("   🔎 scan[{}] {} — spent status unknown and not affirmatively listed ({:?}); not written (unverified)",
                              route, outpoint, probe);
                        report.unverified.push(outpoint);
                        continue;
                    }
                }
            }
        }

        // (5) + (6), under one lock.
        let Ok(d) = db.lock() else { report.unverified.push(outpoint); continue };
        let conn = d.connection();
        let repo = crate::database::OutputRepository::new(conn);
        match repo.upsert_received_utxo_with_derivation(
            user_id, &u.txid, u.vout, u.satoshis, &hex::encode(&obs.script),
            index as i32, "BRC-42", u.confirmed, crate::ingest::ScriptSource::Observed(obs),
        ) {
            Ok(1) => {
                if let Err(e) = conn.execute(
                    "UPDATE outputs SET purpose = ?1 WHERE txid = ?2 AND vout = ?3",
                    rusqlite::params![SCAN_PURPOSE, u.txid, u.vout as i64],
                ) {
                    warn!("   🔎 scan[{}] {} — purpose not recorded: {}", route, outpoint, e);
                }
                crate::classify::after_write(&d, &u.txid, u.vout, route);
                info!("   🔎 scan[{}] {} — written ({} sats, index {})", route, outpoint, u.satoshis, index);
                report.written.push(outpoint);
            }
            Ok(_) => report.already_known += 1,
            Err(e) => {
                warn!("   🔎 scan[{}] {} — insert failed: {}", route, outpoint, e);
                report.unverified.push(outpoint);
            }
        }
    }

    info!("   🔎 scan[{}]: {} written, {} already held, {} not ours, {} spent, {} unverified",
          route, report.written.len(), report.already_known, report.not_ours.len(),
          report.spent.len(), report.unverified.len());
    report
}

/// The BRC-42 address row for a scanned index, so the address is watched again. A row
/// already at that index (a legacy BIP-32 row, say) is left alone — the coin is kept
/// either way.
fn ensure_receive_address_row(db: &WalletDatabase, wallet_id: i64, index: u32, master_privkey: &[u8], master_pubkey: &[u8]) {
    let repo = crate::database::AddressRepository::new(db.connection());
    match repo.get_by_wallet_and_index(wallet_id, index as i32) {
        Ok(Some(existing)) => {
            if crate::reconcile::derive_receive_address(master_privkey, master_pubkey, index as i32).as_deref()
                != Some(existing.address.as_str())
            {
                warn!("   🔎 scan: address row at index {} is not its BRC-42 address (legacy row) — left as is", index);
            } else if let Err(e) = db.connection().execute(
                "UPDATE addresses SET pending_utxo_check = 1, created_at = strftime('%s', 'now') WHERE wallet_id = ?1 AND \"index\" = ?2",
                rusqlite::params![wallet_id, index as i32],
            ) {
                warn!("   🔎 scan: address {} not re-watched: {}", index, e);
            }
        }
        Ok(None) => {
            let Some(address) = crate::reconcile::derive_receive_address(master_privkey, master_pubkey, index as i32) else { return };
            let public_key = crate::crypto::brc42::derive_child_public_key(
                master_privkey, master_pubkey, &format!("2-receive address-{}", index))
                .map(hex::encode).unwrap_or_default();
            let row = crate::database::Address {
                id: None, wallet_id, index: index as i32, address, public_key,
                used: true, balance: 0, pending_utxo_check: true, created_at: 0,
            };
            if let Err(e) = repo.create(&row) {
                warn!("   🔎 scan: address row for index {} not created: {}", index, e);
            }
        }
        Err(e) => warn!("   🔎 scan: address row lookup for index {} failed: {}", index, e),
    }
}

/// One run of the scan, as every caller reports it.
#[derive(Debug)]
pub struct ScanOutcome {
    pub high_water: u32,
    pub result: RecoveryResult,
    pub write: WriteReport,
}

impl ScanOutcome {
    /// Complete only when every index was read **and** every hit was checked — an
    /// unverified coin is "could not ask", never "nothing there" (rule 7, trip-wire 2).
    pub fn complete(&self) -> bool {
        self.result.complete() && self.write.unverified.is_empty()
    }

    /// Why the scan is not complete, or `None` when it is — the one wording every caller shows.
    pub fn incomplete_message(&self) -> Option<String> {
        match (self.result.incomplete_message(), self.write.unverified.is_empty()) {
            (Some(m), _) => Some(m),
            (None, false) => Some(format!("incomplete — {} coin(s) found could not be checked; run Find late payments again",
                                          self.write.unverified.len())),
            (None, true) => None,
        }
    }

    /// The scan's part of a response: found, incomplete ranges, unverified coins, note.
    pub fn to_json(&self) -> serde_json::Value {
        let message = self.incomplete_message();
        serde_json::json!({
            "high_water": self.high_water,
            "addresses_scanned": self.result.addresses_found,
            "new_utxos_found": self.write.written.len(),
            "written": self.write.written,
            "already_known": self.write.already_known,
            "spent": self.write.spent,
            "not_ours": self.write.not_ours,
            "unverified": self.write.unverified,
            "complete": self.complete(),
            "unchecked_ranges": self.result.unchecked_ranges,
            "message": message,
            "note": SCAN_SELF_ONLY_NOTE,
        })
    }
}

/// The scan + its writer, for any caller: reads the high-water mark, scans, writes what
/// the chain proves, and moves the address counter up to the highest index seen. The
/// caller holds no lock (this awaits the network).
pub async fn find_late_payments(
    db: &Mutex<WalletDatabase>,
    user_id: i64,
    wallet_id: i64,
    master_privkey: &[u8],
    master_pubkey: &[u8],
    route: &str,
    chain: &dyn ScanChain,
) -> Result<ScanOutcome, String> {
    let high_water = {
        let d = db.lock().map_err(|e| format!("database lock: {}", e))?;
        crate::database::WalletRepository::new(d.connection())
            .receive_high_water(wallet_id)
            .map_err(|e| format!("high-water mark: {}", e))?
    };
    let high_water = high_water.max(0) as u32;
    let budget = RetryBudget::new(RETRY_BUDGET_MS);
    let result = scan_own_addresses(master_privkey, master_pubkey, high_water, chain, &budget).await;
    let write = write_found_coins(db, user_id, wallet_id, master_privkey, master_pubkey, &result, route, chain, &budget).await;
    if let Some(top) = write.highest_index_seen {
        let d = db.lock().map_err(|e| format!("database lock: {}", e))?;
        if let Err(e) = crate::database::WalletRepository::new(d.connection()).raise_current_index(wallet_id, top as i32) {
            warn!("   🔎 scan[{}]: address counter not raised to {}: {}", route, top, e);
        }
    }
    Ok(ScanOutcome { high_water, result, write })
}

#[cfg(test)]
#[path = "scan_tests.rs"]
pub(crate) mod tests;
