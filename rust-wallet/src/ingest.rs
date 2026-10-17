//! Ingest truth (B5-T1-P4) — what the wallet records about a coin is what the chain says.
//!
//! Address sync learns of a coin from an indexer's `{txid, vout, value}`. Until
//! B5-T1-P4 the locking script stored for it was **synthesised from the address**
//! (`utxo_fetcher :: generate_p2pkh_script_from_address`), so a 2.6 MB inscription
//! sent to one of our addresses was recorded as a 25-byte P2PKH. Here the script is
//! read from the parent transaction's raw bytes, after those bytes are checked:
//!
//! - `double-SHA256(raw)` reversed must equal the txid (a provider cannot hand us
//!   someone else's transaction), and
//! - the value at `vout` must equal the satoshis the indexer claimed.
//!
//! A row's `script_length` says whether its script is known:
//! - **set** ⇒ observed from verified bytes, or built by us;
//! - **NULL** ⇒ unchecked. Such a row keeps the address-derived P2PKH script (owner
//!   decision 1(b), 2026-10-02: it stays spendable exactly as before) and is retried by
//!   [`complete_unchecked`] until it is observed.
//!
//! Scripts longer than the cap (`settings.max_output_script`, 1,024 B) are not kept
//! inline: `locking_script` is NULL and readers use
//! [`crate::database::OutputRepository::locking_script_for`], which reads them from
//! the pinned raw parent. This is `@bsv/wallet-toolbox`'s `maxOutputScript` rule.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use log::{info, warn};
use rusqlite::Connection;

use crate::database::WalletDatabase;
use crate::services::WalletServices;

/// Inline cap used when no `settings` row exists (B5-T1-P4 §12 Q1: 1,024 B).
pub const DEFAULT_MAX_INLINE_SCRIPT: usize = 1024;

/// A locking script read from verified parent bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    pub script: Vec<u8>,
    /// Byte offset of the script inside the raw transaction.
    pub offset: usize,
}

/// Which check refused a parent's bytes. `Display` names the check — the warning
/// line must say which one failed, not just "parse error".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveError {
    TxidMismatch,
    Unparseable(String),
    NoSuchOutput { vout: u32, outputs: usize },
    ValueMismatch { chain: i64, claimed: i64 },
}

impl std::fmt::Display for ObserveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObserveError::TxidMismatch => write!(f, "txid check failed (bytes do not hash to the txid)"),
            ObserveError::Unparseable(e) => write!(f, "raw transaction unparseable: {}", e),
            ObserveError::NoSuchOutput { vout, outputs } =>
                write!(f, "vout check failed (vout {} of {} outputs)", vout, outputs),
            ObserveError::ValueMismatch { chain, claimed } =>
                write!(f, "value check failed (chain {} sats, indexer claimed {})", chain, claimed),
        }
    }
}

/// How the script being written came to be known.
#[derive(Debug, Clone, Copy)]
pub enum ScriptSource<'a> {
    /// We built this transaction; the script is the one we put in it.
    Built,
    /// Read from the parent's raw bytes by [`observe_output`].
    Observed(&'a Observed),
    /// Assumed from the address, not checked. Written, but never with a `script_length`.
    Unchecked,
}

/// Read output `vout`'s locking script from `raw`, after checking that `raw` is
/// transaction `txid` and that the output holds `claimed_sats`. Pure.
pub fn observe_output(raw: &[u8], txid: &str, vout: u32, claimed_sats: i64) -> Result<Observed, ObserveError> {
    if !crate::reconcile::verify_raw_txid(raw, txid) {
        return Err(ObserveError::TxidMismatch);
    }
    let spans = crate::reconcile::parse_tx_output_spans(raw).map_err(ObserveError::Unparseable)?;
    let &(value, offset, len) = spans.get(vout as usize)
        .ok_or(ObserveError::NoSuchOutput { vout, outputs: spans.len() })?;
    if value != claimed_sats {
        return Err(ObserveError::ValueMismatch { chain: value, claimed: claimed_sats });
    }
    Ok(Observed { script: raw[offset..offset + len].to_vec(), offset })
}

/// The inline cap, from `settings.max_output_script`.
pub fn max_inline_script(conn: &Connection) -> usize {
    conn.query_row("SELECT max_output_script FROM settings LIMIT 1", [], |r| r.get::<_, i64>(0))
        .ok()
        .filter(|n| *n >= 0)
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_MAX_INLINE_SCRIPT)
        // Never below a P2PKH: a cap under 25 B would offload every money coin.
        .max(25)
}

/// The three script columns for a row: `(locking_script, script_length, script_offset)`.
pub fn script_columns(conn: &Connection, script_hex: &str, source: ScriptSource) -> (Option<Vec<u8>>, Option<i64>, Option<i64>) {
    match source {
        ScriptSource::Built => {
            let bytes = hex::decode(script_hex).ok();
            let len = bytes.as_ref().map(|b| b.len() as i64);
            (bytes, len, None)
        }
        ScriptSource::Unchecked => (hex::decode(script_hex).ok(), None, None),
        ScriptSource::Observed(o) => {
            let inline = if o.script.len() <= max_inline_script(conn) { Some(o.script.clone()) } else { None };
            (inline, Some(o.script.len() as i64), Some(o.offset as i64))
        }
    }
}

/// A raw transaction already held locally: the parent cache, or one of our own.
pub fn local_raw_tx(conn: &Connection, txid: &str) -> Option<Vec<u8>> {
    let cached: Option<String> = conn.query_row(
        "SELECT raw_hex FROM parent_transactions WHERE txid = ?1", [txid], |r| r.get(0)).ok();
    if let Some(bytes) = cached.and_then(|h| hex::decode(h).ok()) {
        return Some(bytes);
    }
    conn.query_row(
        "SELECT raw_tx FROM transactions WHERE txid = ?1 AND raw_tx IS NOT NULL AND LENGTH(raw_tx) > 0",
        [txid], |r| r.get::<_, Vec<u8>>(0)).ok()
}

/// Raw bytes for `txid` that hash to `txid`: local first, then `fetch_raw`. A fetched
/// copy that verifies is cached in `parent_transactions` (so it is in place **before**
/// any row that depends on it is written); one that does not is never cached.
async fn verified_raw_tx<F, Fut>(db: &Mutex<WalletDatabase>, txid: &str, fetch_raw: &F) -> Option<Vec<u8>>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let local = db.lock().ok().and_then(|d| local_raw_tx(d.connection(), txid));
    if let Some(raw) = local {
        if crate::reconcile::verify_raw_txid(&raw, txid) {
            return Some(raw);
        }
        warn!("   ⚠️  ingest: cached raw tx for {} fails the txid check — fetching again", short(txid));
    }
    match fetch_raw(txid.to_string()).await {
        Ok(raw) if crate::reconcile::verify_raw_txid(&raw, txid) => {
            if let Ok(d) = db.lock() {
                let repo = crate::database::ParentTransactionRepository::new(d.connection());
                if let Err(e) = repo.upsert(None, txid, &hex::encode(&raw)) {
                    warn!("   ⚠️  ingest: failed to cache parent tx {}: {}", short(txid), e);
                }
            }
            Some(raw)
        }
        // Returned so `observe_output` reports the txid check by name.
        Ok(raw) => Some(raw),
        Err(e) => {
            warn!("   ⚠️  ingest: parent tx {} unavailable: {}", short(txid), e);
            None
        }
    }
}

/// Most distinct parents [`observe_coins`] reads per call (adversarial review MED 2).
pub const OBSERVE_FETCH_BUDGET: usize = 20;

/// Observe the real script of each coin `(txid, vout, claimed_sats)` **before** its row
/// is written. Coins whose outpoint is already stored are skipped (their insert is a
/// no-op; [`complete_unchecked`] owns existing rows). A coin missing from the returned
/// map was not observed and is written [`ScriptSource::Unchecked`].
pub async fn observe_coins<F, Fut>(
    db: &Mutex<WalletDatabase>,
    coins: &[(String, u32, i64)],
    fetch_raw: F,
) -> HashMap<(String, u32), Observed>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let new_coins: Vec<&(String, u32, i64)> = match db.lock() {
        Ok(d) => coins.iter().filter(|(txid, vout, _)| {
            d.connection().query_row(
                "SELECT 1 FROM outputs WHERE txid = ?1 AND vout = ?2",
                rusqlite::params![txid, *vout as i64], |_| Ok(())).is_err()
        }).collect(),
        Err(_) => return HashMap::new(),
    };

    let mut raws: HashMap<String, Option<Vec<u8>>> = HashMap::new();
    let mut observed = HashMap::new();
    for (txid, vout, sats) in new_coins {
        if !raws.contains_key(txid) {
            // Bounded: a payment or restore never waits on more than this many parent
            // fetches. Coins past the budget are written unchecked and completed by
            // `complete_unchecked` on later ticks.
            if raws.len() >= OBSERVE_FETCH_BUDGET {
                continue;
            }
            let raw = verified_raw_tx(db, txid, &fetch_raw).await;
            raws.insert(txid.clone(), raw);
        }
        let Some(raw) = raws.get(txid).and_then(|r| r.as_ref()) else { continue };
        match observe_output(raw, txid, *vout, *sats) {
            Ok(o) => { observed.insert((txid.clone(), *vout), o); }
            Err(e) => warn!("   ⚠️  ingest: {}:{} written NOT OBSERVED — {}", short(txid), vout, e),
        }
    }
    observed
}

/// Txids whose raw transaction holds an unspent output's offloaded script.
pub fn offloaded_txids(conn: &Connection) -> Vec<String> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT DISTINCT txid FROM outputs WHERE txid IS NOT NULL AND spent_by IS NULL
           AND script_length IS NOT NULL AND (locking_script IS NULL OR LENGTH(locking_script) = 0)") else {
        return Vec::new();
    };
    stmt.query_map([], |r| r.get(0))
        .map(|it| it.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

/// Backup restore (`refetch_stripped_data`): fill in the script of every output of
/// `txid` that has none, from `raw` — only through [`observe_output`] (so the bytes
/// must be `txid` and the value must match the row) and only up to the inline cap. A
/// row whose script is longer stays NULL with its length/offset set and is read from
/// the parent. Returns the number of rows restored or completed.
pub fn restore_scripts_from_raw(conn: &Connection, txid: &str, raw: &[u8]) -> u32 {
    let Ok(mut stmt) = conn.prepare(
        "SELECT outputId, vout, satoshis FROM outputs
         WHERE txid = ?1 AND (locking_script IS NULL OR LENGTH(locking_script) = 0)") else {
        return 0;
    };
    let missing: Vec<(i64, u32, i64)> = stmt
        .query_map([txid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map(|it| it.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    drop(stmt);

    let mut restored = 0;
    for (id, vout, sats) in missing {
        match observe_output(raw, txid, vout, sats) {
            Ok(o) => {
                let (inline, len, off) = script_columns(conn, "", ScriptSource::Observed(&o));
                if conn.execute(
                    "UPDATE outputs SET locking_script = ?1, script_length = ?2, script_offset = ?3 WHERE outputId = ?4",
                    rusqlite::params![inline, len, off, id]).map(|n| n > 0).unwrap_or(false) {
                    restored += 1;
                }
            }
            Err(e) => warn!("   ⚠️  restore: {}:{} script not restored — {}", short(txid), vout, e),
        }
    }
    restored
}

/// `(txid, vout, satoshis)` for each coin an indexer reported.
pub fn coin_keys<'a>(utxos: impl IntoIterator<Item = &'a crate::utxo_fetcher::UTXO>) -> Vec<(String, u32, i64)> {
    utxos.into_iter().map(|u| (u.txid.clone(), u.vout, u.satoshis)).collect()
}

/// The [`ScriptSource`] to write for one coin, given [`observe_coins`]' result.
pub fn source_for<'a>(observed: &'a HashMap<(String, u32), Observed>, txid: &str, vout: u32) -> ScriptSource<'a> {
    match observed.get(&(txid.to_string(), vout)) {
        Some(o) => ScriptSource::Observed(o),
        None => ScriptSource::Unchecked,
    }
}

fn chain_fetcher(services: &Arc<WalletServices>)
    -> impl Fn(String) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + Send>>
{
    let services = services.clone();
    move |txid: String| {
        let services = services.clone();
        Box::pin(async move { services.get_raw_tx(&txid).await.map_err(|e| e.to_string()) })
    }
}

/// [`observe_coins`] against the Services raw-tx chain (ARC GP → WoC → JungleBus → Bitails).
pub async fn observe_coins_from_chain(
    db: &Mutex<WalletDatabase>,
    services: &Arc<WalletServices>,
    coins: &[(String, u32, i64)],
) -> HashMap<(String, u32), Observed> {
    observe_coins(db, coins, chain_fetcher(services)).await
}

/// [`complete_unchecked`] against the Services raw-tx chain.
pub async fn complete_unchecked_from_chain(
    db: &Mutex<WalletDatabase>,
    services: &Arc<WalletServices>,
    after_id: i64,
    limit: usize,
) -> SweepReport {
    complete_unchecked(db, after_id, limit, chain_fetcher(services)).await
}

/// Result of one [`complete_unchecked`] pass.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SweepReport {
    pub completed: u32,
    pub still_unchecked: u32,
    /// Of `still_unchecked`: rows whose parent could not be fetched this pass — worth
    /// retrying (an indexer refusing or down). The rest cannot be observed by waiting.
    pub unavailable: u32,
    /// Resume after this `outputId` next pass (0 = start over).
    pub next_after_id: i64,
}

/// Complete up to `limit` unspent rows whose script is unchecked (`script_length IS
/// NULL`), resuming after `after_id` so a row that can never be completed cannot
/// starve the rest. This is both the retry for a failed parent fetch and the owner's
/// one-time backfill of rows written before B5-T1-P4 (decision 1(a)): an old row gets
/// a `script_length` only from verified bytes, never by copying its stored script.
pub async fn complete_unchecked<F, Fut>(
    db: &Mutex<WalletDatabase>,
    after_id: i64,
    limit: usize,
    fetch_raw: F,
) -> SweepReport
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let rows: Vec<(i64, String, u32, i64, Option<Vec<u8>>)> = match db.lock() {
        Ok(d) => {
            let conn = d.connection();
            let mut stmt = match conn.prepare(
                "SELECT outputId, txid, vout, satoshis, locking_script FROM outputs
                 WHERE script_length IS NULL AND spent_by IS NULL AND spendable = 1 AND txid IS NOT NULL AND outputId > ?1
                 ORDER BY outputId LIMIT ?2") {
                Ok(s) => s,
                Err(e) => { warn!("   ⚠️  ingest sweep: {}", e); return SweepReport::default(); }
            };
            stmt.query_map(rusqlite::params![after_id, limit as i64],
                |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, u32>(2)?, r.get(3)?, r.get(4)?)))
                .map(|it| it.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        }
        Err(_) => return SweepReport::default(),
    };

    let mut report = SweepReport {
        // A short page means we reached the end: start over next time.
        next_after_id: if rows.len() < limit { 0 } else { rows.last().map(|r| r.0).unwrap_or(0) },
        ..Default::default()
    };
    let mut raws: HashMap<String, Option<Vec<u8>>> = HashMap::new();
    for (id, txid, vout, sats, stored) in rows {
        if !raws.contains_key(&txid) {
            let raw = verified_raw_tx(db, &txid, &fetch_raw).await;
            raws.insert(txid.clone(), raw);
        }
        let obs = match raws.get(&txid).and_then(|r| r.as_ref()) {
            Some(raw) => observe_output(raw, &txid, vout, sats),
            None => { report.still_unchecked += 1; report.unavailable += 1; continue; }
        };
        match obs {
            Ok(o) => {
                let Ok(d) = db.lock() else { report.still_unchecked += 1; continue; };
                let conn = d.connection();
                let (mut inline, len, off) = script_columns(conn, "", ScriptSource::Observed(&o));
                // ⛔ Adversarial review HIGH 1: a stored script the chain CONFIRMS stays
                // inline whatever its size. Rows written before P4 (our own backup and
                // certificate PushDrops, > 1 KB) are read by `get_locking_script_hex`,
                // which reads the column; offloading them would strand the previous
                // backup and break certificate unpublish. Only a script that was wrong
                // (or absent) is replaced, and only that one is offloaded.
                if stored.as_deref() == Some(o.script.as_slice()) {
                    inline = stored.clone();
                }
                match conn.execute(
                    "UPDATE outputs SET locking_script = ?1, script_length = ?2, script_offset = ?3
                     WHERE outputId = ?4 AND script_length IS NULL",
                    rusqlite::params![inline, len, off, id]) {
                    Ok(n) if n > 0 => {
                        report.completed += 1;
                        // B5-T1-P5: the evidence changed — classify the row again (a coin
                        // the user promoted while unobserved loses the stamp if the chain
                        // shows it is not a plain payment).
                        crate::classify::after_write_id(&d, id, "P4:observed");
                        info!("   ✅ ingest: {}:{} observed ({} B script{})", short(&txid), vout,
                              o.script.len(), if inline.is_none() { ", kept in the raw parent" } else { "" });
                    }
                    Ok(_) => {}
                    Err(e) => { report.still_unchecked += 1; warn!("   ⚠️  ingest: update {}:{}: {}", short(&txid), vout, e); }
                }
            }
            Err(e) => {
                report.still_unchecked += 1;
                warn!("   ⚠️  ingest: {}:{} stays NOT OBSERVED — {}", short(&txid), vout, e);
            }
        }
    }
    report
}

/// TaskPurge's parent cleanup: delete confirmed `parent_transactions` cached before
/// `cutoff`. Confirmed txs have merkle proofs — their parents aren't needed for BEEF.
/// Only deletes records that are both old AND confirmed (`proven_txs` exists).
///
/// ⛔ `P4-A6`: a parent holding an **offloaded** script (an output whose
/// `locking_script` is NULL but `script_length` is set) is never purged — that
/// output's script exists only in these bytes.
pub fn purge_parent_transactions(conn: &Connection, cutoff: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM parent_transactions WHERE cached_at < ?1 AND txid IN (SELECT txid FROM proven_txs)
           AND txid NOT IN (SELECT txid FROM outputs
                            WHERE txid IS NOT NULL AND locking_script IS NULL AND script_length IS NOT NULL)",
        rusqlite::params![cutoff],
    )
}

fn short(txid: &str) -> &str {
    &txid[..txid.len().min(16)]
}

#[cfg(test)]
#[path = "a7_sync_tests.rs"]
mod a7_sync_tests;

#[cfg(test)]
mod tests {
    //! B5-T1-P4 evidence rows. Every fixture is a real, hash-consistent transaction
    //! built in memory, so the txid check is genuinely exercised — never stubbed.
    use super::*;
    use crate::database::OutputRepository;
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicUsize, Ordering};

    const P2PKH: &str = "76a914abababababababababababababababababababab88ac";
    const DAY: i64 = 24 * 3600;

    fn varint(n: usize) -> Vec<u8> {
        if n < 0xfd { vec![n as u8] }
        else if n <= 0xffff { let mut v = vec![0xfd]; v.extend_from_slice(&(n as u16).to_le_bytes()); v }
        else { let mut v = vec![0xfe]; v.extend_from_slice(&(n as u32).to_le_bytes()); v }
    }

    /// A one-input transaction with these `(satoshis, script)` outputs.
    fn raw_tx(outputs: &[(u64, Vec<u8>)]) -> Vec<u8> {
        let mut t = 1u32.to_le_bytes().to_vec();
        t.extend(varint(1));
        t.extend([7u8; 32]);
        t.extend(0u32.to_le_bytes());
        t.extend(varint(0));
        t.extend(0xffff_ffffu32.to_le_bytes());
        t.extend(varint(outputs.len()));
        for (v, s) in outputs {
            t.extend(v.to_le_bytes());
            t.extend(varint(s.len()));
            t.extend(s);
        }
        t.extend(0u32.to_le_bytes());
        t
    }

    fn txid_of(raw: &[u8]) -> String {
        let h = Sha256::digest(Sha256::digest(raw));
        h.iter().rev().map(|b| format!("{:02x}", b)).collect()
    }

    /// The ticket's measured inscription prefix (`76a914…88ac 0063036f7264 5109
    /// image/png`), padded with a pushdata body to `total` bytes.
    fn inscription_script(total: usize) -> Vec<u8> {
        let mut s = hex::decode(P2PKH).unwrap();
        s.extend(hex::decode("0063036f72645109").unwrap());
        s.extend(b"image/png");
        s.push(0x00); // OP_0 separator
        let body = total - s.len() - 1 - 4 - 1; // OP_PUSHDATA4 + len + body + OP_ENDIF
        s.push(0x4e);
        s.extend((body as u32).to_le_bytes());
        s.extend(std::iter::repeat(0x89u8).take(body));
        s.push(0x68);
        assert_eq!(s.len(), total);
        s
    }

    fn test_db(tag: &str) -> Mutex<WalletDatabase> {
        let path = std::env::temp_dir().join(format!("hodos_ingest_{}_{}_{}.db", tag, std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let db = WalletDatabase::new(path).unwrap();
        db.connection().execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at) VALUES (1, 'k', 'local', 0, 0)",
            []).unwrap();
        Mutex::new(db)
    }

    fn set_cap(db: &Mutex<WalletDatabase>, cap: i64) {
        db.lock().unwrap().connection().execute(
            "INSERT INTO settings (storage_identity_key, storage_name, chain, dbtype, max_output_script, created_at, updated_at)
             VALUES ('', '', 'main', 'sqlite', ?1, 0, 0)", [cap]).unwrap();
    }

    fn fetch_ok(raw: Vec<u8>) -> impl Fn(String) -> std::future::Ready<Result<Vec<u8>, String>> {
        move |_| std::future::ready(Ok(raw.clone()))
    }

    fn fetch_err(_: String) -> std::future::Ready<Result<Vec<u8>, String>> {
        std::future::ready(Err("all providers down".into()))
    }

    /// Ingest one synced coin exactly as the sync paths do: observe, then write.
    async fn ingest<F, Fut>(db: &Mutex<WalletDatabase>, txid: &str, vout: u32, sats: i64, fetch: F)
    where F: Fn(String) -> Fut, Fut: Future<Output = Result<Vec<u8>, String>> {
        let observed = observe_coins(db, &[(txid.to_string(), vout, sats)], fetch).await;
        let d = db.lock().unwrap();
        OutputRepository::new(d.connection())
            .upsert_received_utxo_with_confirmed(1, txid, vout, sats, P2PKH, 0, true, source_for(&observed, txid, vout))
            .unwrap();
    }

    /// `(locking_script, script_length, script_offset)` of the stored row, by outpoint.
    fn row(db: &Mutex<WalletDatabase>, txid: &str, vout: u32) -> (Option<Vec<u8>>, Option<i64>, Option<i64>) {
        db.lock().unwrap().connection().query_row(
            "SELECT locking_script, script_length, script_offset FROM outputs WHERE txid = ?1 AND vout = ?2",
            rusqlite::params![txid, vout], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap()
    }

    fn accessor(db: &Mutex<WalletDatabase>, txid: &str, vout: u32) -> Option<Vec<u8>> {
        let d = db.lock().unwrap();
        let repo = OutputRepository::new(d.connection());
        let out = repo.get_by_txid_vout(txid, vout).unwrap().unwrap();
        repo.locking_script_for(&out)
    }

    // ---- P4-A1 ---------------------------------------------------------------

    /// `P4-A1` (T1): the ticket's measured fresh inscription, 2,596,810 B, synced from
    /// its real parent ⇒ `script_length` is the TRUE length, not a 25-byte P2PKH.
    #[tokio::test]
    async fn p4_a1_inscription_is_stored_with_its_true_length() {
        let db = test_db("a1");
        let script = inscription_script(2_596_810);
        let raw = raw_tx(&[(1, script.clone())]);
        let txid = txid_of(&raw);
        ingest(&db, &txid, 0, 1, fetch_ok(raw)).await;

        let (inline, len, off) = row(&db, &txid, 0);
        assert_eq!(len, Some(2_596_810), "script_length must be the chain's length");
        assert_ne!(inline.as_ref().map(|s| s.len()), Some(25), "must not be stored as a 25-byte P2PKH");
        assert!(inline.is_none(), "above the cap: not inline");
        assert!(off.is_some());
        assert_eq!(accessor(&db, &txid, 0), Some(script), "the accessor returns the chain's script");
    }

    /// `P4-A1` RED (ii) mechanism: a row written BEFORE its parent is observed stays
    /// unchecked — `observe_coins` skips stored outpoints, so the order is load-bearing.
    #[tokio::test]
    async fn p4_a1_row_written_before_the_parent_stays_unchecked() {
        let db = test_db("a1ii");
        let raw = raw_tx(&[(1, inscription_script(4_000))]);
        let txid = txid_of(&raw);
        {
            let d = db.lock().unwrap();
            OutputRepository::new(d.connection())
                .upsert_received_utxo_with_confirmed(1, &txid, 0, 1, P2PKH, 0, true, ScriptSource::Unchecked).unwrap();
        }
        let observed = observe_coins(&db, &[(txid.clone(), 0, 1)], fetch_ok(raw)).await;
        assert!(observed.is_empty(), "an already-stored outpoint is not observed by the ingest step");
        assert_eq!(row(&db, &txid, 0).1, None, "the row as first read is unchecked");
    }

    // ---- P4-A2 / A3 ------------------------------------------------------------

    /// `P4-A2`: an 860 B OrdLock-sized script is stored byte-for-byte (compared
    /// through the accessor), and with the cap set below 860 it is offloaded yet the
    /// accessor still returns the chain's bytes.
    #[tokio::test]
    async fn p4_a2_ordlock_sized_script_is_stored_truly_inline_and_offloaded() {
        let ordlock: Vec<u8> = (0..860u32).map(|i| (i % 251) as u8).collect();
        let raw = raw_tx(&[(1000, hex::decode(P2PKH).unwrap()), (1, ordlock.clone())]);
        let txid = txid_of(&raw);

        let db = test_db("a2");
        ingest(&db, &txid, 1, 1, fetch_ok(raw.clone())).await;
        let (inline, len, _) = row(&db, &txid, 1);
        assert_eq!(len, Some(860));
        assert_eq!(inline.as_ref(), Some(&ordlock), "under the 1,024 B cap: inline, byte-for-byte");
        assert_eq!(accessor(&db, &txid, 1), Some(ordlock.clone()));

        let db = test_db("a2cap");
        set_cap(&db, 512);
        ingest(&db, &txid, 1, 1, fetch_ok(raw)).await;
        let (inline, len, _) = row(&db, &txid, 1);
        assert_eq!((inline, len), (None, Some(860)), "cap below 860: offloaded");
        assert_eq!(accessor(&db, &txid, 1), Some(ordlock), "the accessor reads it from the raw parent");
    }

    /// `P4-A3`: a plain 25-byte P2PKH coin is stored with exactly the parent's bytes
    /// at its vout — the money path is unchanged.
    #[tokio::test]
    async fn p4_a3_p2pkh_coin_is_stored_with_the_parents_bytes() {
        // ⚠️ `ours` differs from the ASSUMED script (`P2PKH`), so an unobserved row
        // cannot pass the byte compare by accident.
        let ours = hex::decode("76a914efefefefefefefefefefefefefefefefefefefef88ac").unwrap();
        let other = hex::decode("76a914cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd88ac").unwrap();
        let raw = raw_tx(&[(5000, other), (7000, ours.clone())]);
        let txid = txid_of(&raw);
        let db = test_db("a3");
        ingest(&db, &txid, 1, 7000, fetch_ok(raw)).await;
        let (inline, len, _) = row(&db, &txid, 1);
        assert_eq!(inline, Some(ours), "the parent's output-1 bytes, not output 0's");
        assert_eq!(len, Some(25));
    }

    // ---- P4-A4 ---------------------------------------------------------------

    /// `P4-A4`: a parent whose hash is not the txid, or whose value at vout is not the
    /// indexer's ⇒ the row is written NOT OBSERVED, and the error names the check.
    #[tokio::test]
    async fn p4_a4_bad_parent_is_refused_by_name() {
        let raw = raw_tx(&[(1, inscription_script(3_000))]);
        let txid = txid_of(&raw);
        let mut flipped = raw.clone();
        let n = flipped.len();
        flipped[n - 100] ^= 0x01; // inside the output script

        assert_eq!(observe_output(&flipped, &txid, 0, 1), Err(ObserveError::TxidMismatch));
        assert!(ObserveError::TxidMismatch.to_string().contains("txid check"));
        let db = test_db("a4");
        ingest(&db, &txid, 0, 1, fetch_ok(flipped)).await;
        assert_eq!(row(&db, &txid, 0).1, None, "flipped bytes ⇒ not observed");
        assert_eq!(row(&db, &txid, 0).0, Some(hex::decode(P2PKH).unwrap()),
                   "decision 1(b): the assumed script is kept, flagged unchecked");

        let err = observe_output(&raw, &txid, 0, 999).unwrap_err();
        assert_eq!(err, ObserveError::ValueMismatch { chain: 1, claimed: 999 });
        assert!(err.to_string().contains("value check"));
        let db = test_db("a4v");
        ingest(&db, &txid, 0, 999, fetch_ok(raw)).await;
        assert_eq!(row(&db, &txid, 0).1, None, "indexer value ≠ bytes ⇒ not observed");
    }

    // ---- P4-A5 ---------------------------------------------------------------

    /// `P4-A5`: providers down on tick 1 ⇒ row written not observed; tick 2 (providers
    /// back) completes it through the sweep.
    #[tokio::test]
    async fn p4_a5_failed_fetch_is_retried_and_completed() {
        let script = inscription_script(2_000);
        let raw = raw_tx(&[(1, script.clone())]);
        let txid = txid_of(&raw);
        let db = test_db("a5");

        ingest(&db, &txid, 0, 1, fetch_err).await;
        assert_eq!(row(&db, &txid, 0).1, None, "tick 1: not observed");

        let report = complete_unchecked(&db, 0, 25, fetch_ok(raw)).await;
        assert_eq!(report.completed, 1);
        let (inline, len, _) = row(&db, &txid, 0);
        assert_eq!(len, Some(2_000), "tick 2: completed from the chain");
        assert!(inline.is_none(), "and the assumed P2PKH is gone (offloaded above the cap)");
        assert_eq!(accessor(&db, &txid, 0), Some(script));
    }

    // ---- P4-A6 ---------------------------------------------------------------

    /// `P4-A6`: an offloaded script is still readable through the accessor after
    /// TaskPurge's parent cleanup runs 8 days later. The purge CAN fire — proven_txs
    /// seeded, cached_at backdated, and an unreferenced parent IS deleted — so the
    /// control is not vacuous.
    #[tokio::test]
    async fn p4_a6_offloaded_script_survives_a_forced_purge() {
        let script = inscription_script(5_000);
        let raw = raw_tx(&[(1, script.clone())]);
        let txid = txid_of(&raw);
        let other_raw = raw_tx(&[(9, hex::decode(P2PKH).unwrap())]);
        let other = txid_of(&other_raw);
        let db = test_db("a6");
        ingest(&db, &txid, 0, 1, fetch_ok(raw)).await;
        {
            let d = db.lock().unwrap();
            let c = d.connection();
            crate::database::ParentTransactionRepository::new(c).upsert(None, &other, &hex::encode(&other_raw)).unwrap();
            c.execute("UPDATE parent_transactions SET cached_at = 0", []).unwrap();
            for t in [&txid, &other] {
                c.execute("INSERT INTO proven_txs (txid, height, tx_index, merkle_path, raw_tx, created_at, updated_at)
                           VALUES (?1, 1, 0, x'00', x'00', 0, 0)", [t]).unwrap();
            }
            let now = 8 * DAY;
            let purged = purge_parent_transactions(c, now - 7 * DAY).unwrap();
            assert_eq!(purged, 1, "the purge fires: the unreferenced parent is deleted");
        }
        assert_eq!(accessor(&db, &txid, 0), Some(script), "the offloaded script's parent survived");
    }

    // ---- P4-A13 --------------------------------------------------------------

    /// `P4-A13`: a row with no inline script never enters a coin pool; a normal row does.
    #[tokio::test]
    async fn p4_a13_row_without_inline_script_never_enters_the_pool() {
        let raw = raw_tx(&[(50_000, inscription_script(2_000)), (40_000, hex::decode(P2PKH).unwrap()),
                           (30_000, inscription_script(300))]);
        let txid = txid_of(&raw);
        let db = test_db("a13");
        ingest(&db, &txid, 0, 50_000, fetch_ok(raw.clone())).await; // offloaded (A)
        ingest(&db, &txid, 1, 40_000, fetch_ok(raw.clone())).await; // P2PKH (B)
        ingest(&db, &txid, 2, 30_000, fetch_ok(raw)).await;         // inline inscription (C)

        let d = db.lock().unwrap();
        // B5-T1-P5: the classifier would never stamp A or C. Stamp all three by hand (a
        // forged stamp) so they reach the pool filter — this row tests the SECOND layer,
        // the selection-time backstop, which must hold even if a stamp is wrong.
        d.connection().execute("UPDATE outputs SET change = 1 WHERE txid = ?1", [&txid]).unwrap();
        let spendable = OutputRepository::new(d.connection()).get_spendable_by_user(1).unwrap();
        assert!(spendable.iter().any(|o| o.vout == 0), "precondition: A passes every pool filter");
        let pool: Vec<_> = spendable.iter().filter_map(crate::database::output_to_fetcher_utxo).collect();
        assert!(!pool.iter().any(|u| u.txid == txid && u.vout == 0), "A (no inline script) is refused");
        assert!(pool.iter().any(|u| u.txid == txid && u.vout == 1), "B is in the pool — the pool is not just empty");
        // Owner 2026-10-04: a multi-sat inscription (P2PKH + envelope, inline) is never money.
        assert!(spendable.iter().any(|o| o.vout == 2), "precondition: C passes every pool filter");
        assert!(!pool.iter().any(|u| u.txid == txid && u.vout == 2), "C (not plain P2PKH) is refused");
    }

    // ---- P4-A14 --------------------------------------------------------------

    /// `P4-A14`: backfill. A pre-P4 row (assumed script, `script_length NULL`) whose
    /// parent is cached locally is completed with NO network call; one whose cached
    /// parent disagrees stays NULL.
    #[tokio::test]
    async fn p4_a14_backfill_completes_from_local_bytes_and_refuses_bad_ones() {
        let good_raw = raw_tx(&[(3000, hex::decode(P2PKH).unwrap())]);
        let good = txid_of(&good_raw);
        let bad_src = raw_tx(&[(4000, hex::decode(P2PKH).unwrap())]);
        let bad = txid_of(&bad_src);
        let mut bad_raw = bad_src.clone();
        let n = bad_raw.len();
        bad_raw[n - 10] ^= 0x01;
        let db = test_db("a14");
        {
            let d = db.lock().unwrap();
            let c = d.connection();
            let repo = OutputRepository::new(c);
            repo.upsert_received_utxo_with_confirmed(1, &good, 0, 3000, P2PKH, 0, true, ScriptSource::Unchecked).unwrap();
            repo.upsert_received_utxo_with_confirmed(1, &bad, 0, 4000, P2PKH, 0, true, ScriptSource::Unchecked).unwrap();
            let p = crate::database::ParentTransactionRepository::new(c);
            p.upsert(None, &good, &hex::encode(&good_raw)).unwrap();
            p.upsert(None, &bad, &hex::encode(&bad_raw)).unwrap();
        }
        let calls = AtomicUsize::new(0);
        let report = complete_unchecked(&db, 0, 25, |t: String| {
            calls.fetch_add(1, Ordering::SeqCst);
            fetch_err(t)
        }).await;
        assert_eq!(row(&db, &good, 0).1, Some(25), "completed from the local parent");
        assert_eq!(row(&db, &bad, 0).1, None, "disagreeing local parent ⇒ stays unchecked");
        assert_eq!(report, SweepReport { completed: 1, still_unchecked: 1, unavailable: 1, next_after_id: 0 });
        assert_eq!(calls.load(Ordering::SeqCst), 1, "only the bad parent went to the network");
    }

    /// A coin the verified parent contradicts (wrong value, or no such vout) cannot be
    /// observed by waiting: it is `still_unchecked` but never `unavailable`, so it never
    /// holds the startup check (/code-review 179 round 2, #11).
    #[tokio::test]
    async fn a_coin_its_parent_contradicts_is_not_counted_unavailable() {
        let raw = raw_tx(&[(3000, hex::decode(P2PKH).unwrap())]);
        let txid = txid_of(&raw);
        let db = test_db("unreadable");
        {
            let d = db.lock().unwrap();
            let repo = OutputRepository::new(d.connection());
            repo.upsert_received_utxo_with_confirmed(1, &txid, 0, 9999, P2PKH, 0, true, ScriptSource::Unchecked).unwrap();
            repo.upsert_received_utxo_with_confirmed(1, &txid, 5, 3000, P2PKH, 0, true, ScriptSource::Unchecked).unwrap();
        }
        let report = complete_unchecked(&db, 0, 25, fetch_ok(raw)).await;
        assert_eq!(report, SweepReport { completed: 0, still_unchecked: 2, unavailable: 0, next_after_id: 0 });
    }

    // ---- P4-A15 --------------------------------------------------------------

    /// `P4-A15`: restore fills a stripped script only through the verified
    /// observation and never re-inlines one above the cap; the parent of an
    /// offloaded row is listed for re-fetch.
    #[tokio::test]
    async fn p4_a15_restore_is_verified_and_capped() {
        let big = inscription_script(3_000);
        let raw = raw_tx(&[(1, big), (2000, hex::decode(P2PKH).unwrap())]);
        let txid = txid_of(&raw);
        let db = test_db("a15");
        let d = db.lock().unwrap();
        let c = d.connection();
        for (vout, sats, len) in [(0u32, 1i64, Some(3_000i64)), (1, 2000, None)] {
            c.execute("INSERT INTO outputs (user_id, txid, vout, satoshis, locking_script, script_length, spendable, created_at, updated_at)
                       VALUES (1, ?1, ?2, ?3, NULL, ?4, 1, 0, 0)", rusqlite::params![txid, vout, sats, len]).unwrap();
        }
        assert_eq!(offloaded_txids(c), vec![txid.clone()], "the offloaded row's parent is listed for re-fetch");

        let mut tampered = raw.clone();
        let n = tampered.len();
        tampered[n - 10] ^= 0x01;
        assert_eq!(restore_scripts_from_raw(c, &txid, &tampered), 0, "tampered bytes restore nothing");
        let s1: Option<Vec<u8>> = c.query_row("SELECT locking_script FROM outputs WHERE vout = 1", [], |r| r.get(0)).unwrap();
        assert!(s1.is_none());

        assert_eq!(restore_scripts_from_raw(c, &txid, &raw), 2);
        let (s0, l0): (Option<Vec<u8>>, Option<i64>) = c.query_row(
            "SELECT locking_script, script_length FROM outputs WHERE vout = 0", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((s0, l0), (None, Some(3_000)), "above the cap: never re-inlined");
        let (s1, l1): (Option<Vec<u8>>, Option<i64>) = c.query_row(
            "SELECT locking_script, script_length FROM outputs WHERE vout = 1", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((s1, l1), (Some(hex::decode(P2PKH).unwrap()), Some(25)));
    }

    // ---- adversarial review fixes ------------------------------------------------

    /// Review HIGH 1: a row written before P4 for our OWN transaction — e.g. the
    /// on-chain backup PushDrop, > 1 KB, inline, `script_length` NULL — is completed by
    /// the backfill WITHOUT being offloaded, so `get_locking_script_hex` (which reads
    /// the column: `do_onchain_backup`, certificate unpublish) still returns it.
    #[tokio::test]
    async fn review_high1_backfill_keeps_a_confirmed_large_script_inline() {
        let pushdrop: Vec<u8> = (0..4_000u32).map(|i| (i % 249) as u8).collect();
        let raw = raw_tx(&[(1, pushdrop.clone())]);
        let txid = txid_of(&raw);
        let db = test_db("high1");
        let id = {
            let d = db.lock().unwrap();
            let c = d.connection();
            c.execute("INSERT INTO transactions (user_id, txid, status, reference_number, is_outgoing, satoshis, raw_tx, created_at, updated_at)
                       VALUES (1, ?1, 'completed', 'r', 1, 0, ?2, 0, 0)", rusqlite::params![txid, raw]).unwrap();
            c.execute("INSERT INTO outputs (user_id, txid, vout, satoshis, locking_script, spendable, created_at, updated_at)
                       VALUES (1, ?1, 0, 1, ?2, 1, 0, 0)", rusqlite::params![txid, pushdrop]).unwrap();
            c.last_insert_rowid()
        };
        let report = complete_unchecked(&db, 0, 25, fetch_err).await;
        assert_eq!(report.completed, 1, "completed from our own raw tx, no network");
        let (inline, len, _) = row(&db, &txid, 0);
        assert_eq!(len, Some(4_000));
        assert_eq!(inline.as_ref(), Some(&pushdrop), "a script the chain confirms stays inline");
        let d = db.lock().unwrap();
        assert_eq!(OutputRepository::new(d.connection()).get_locking_script_hex(id).unwrap(),
                   Some(hex::encode(&pushdrop)), "the column reader still sees it");
    }

    /// Review MED 2: one `observe_coins` call never makes more than
    /// `OBSERVE_FETCH_BUDGET` network fetches; coins past it are left unobserved.
    #[tokio::test]
    async fn review_med2_observe_coins_is_bounded() {
        let db = test_db("med2");
        let coins: Vec<(String, u32, i64)> = (0..(OBSERVE_FETCH_BUDGET + 5))
            .map(|i| (format!("{:064x}", i + 1), 0, 1000)).collect();
        let calls = AtomicUsize::new(0);
        let observed = observe_coins(&db, &coins, |t: String| {
            calls.fetch_add(1, Ordering::SeqCst);
            fetch_err(t)
        }).await;
        assert!(observed.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), OBSERVE_FETCH_BUDGET);
    }
}
