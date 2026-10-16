//! B5-T1-P7a-1: the chain tracker — the one place the wallet asks an indexer for a block, a
//! block header or the chain tip.
//!
//! Step 0 moved every WhatsOnChain block / header / tip lookup here, so a static gate (G15) can
//! refuse an indexer block URL anywhere else. The unverified `legacy_*` lookups it kept for the proof
//! writers were deleted in B5-T1-P7a-2, when their last caller moved to the verified chain.
//!
//! [`ChainTracker`] is the verified side: it owns the wallet's own header chain
//! (`header_chain.rs`), persists it (`verified_headers`, V29), and syncs it from several header
//! sources, none of which is trusted — every header is checked before it is kept.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};

use crypto_bigint::Encoding;

use crate::database::{VerifiedHeaderRepository, WalletDatabase};
use crate::header_chain::{self, AddOutcome, Header, HeaderChain, HeaderError, RootVerdict};

// ─────────────────────────────────────────────────────────────────────────────────────────────
// The verified chain
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// Headers asked of a source per request. 1,000 is the most BSVA's TS chaintracks server accepts
/// (`count must be an integer between 1 and 1000`, live 2026-10-09; Arcade accepts more).
pub const SYNC_BATCH: u32 = 1000;
/// How far below our tip a sync starts, so a source on a branch that forked just below our tip
/// connects; widened (×8) when a batch does not connect, up to `MAX_SYNC_OVERLAP`.
const SYNC_OVERLAP: u32 = 6;
/// Never wider than `SYNC_BATCH`: the widest retry's batch must still reach up to our tip, or a fork
/// deeper than the batch is asked only for headers we hold (`/code-review max 159` V7).
const MAX_SYNC_OVERLAP: u32 = SYNC_BATCH;
/// Our tip counts as current only when its header is at most this old. Blocks hours apart do happen;
/// a tip a day old means we are not synced, whatever a source said (adversarial review 2026-10-09).
pub const MAX_TIP_AGE_SECS: u32 = 6 * 60 * 60;

pub fn now_secs() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as u32).unwrap_or(0)
}

/// What one ingest did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IngestReport {
    pub added: usize,
    pub known: usize,
    /// `(fork_height, deactivated (height, display hash))` per reorg.
    pub reorgs: Vec<(u32, Vec<(u32, String)>)>,
    /// The first header refused; the rest of the batch was not looked at.
    pub refused: Option<HeaderError>,
}

/// The wallet's own block-header chain. Callers ask it, never an indexer.
pub struct ChainTracker {
    chain: RwLock<HeaderChain>,
    /// A source's tip was reached in the last sync AND our tip header is under `MAX_TIP_AGE_SECS`
    /// old. Cleared when the tip goes stale.
    synced: AtomicBool,
}

impl ChainTracker {
    pub fn from_chain(chain: HeaderChain) -> ChainTracker {
        ChainTracker { chain: RwLock::new(chain), synced: AtomicBool::new(false) }
    }

    /// The compiled mainnet checkpoint, extended by every stored header that still passes every
    /// rule. A stored row that fails is deleted (`P7a-A17`): the table is a cache, not an authority.
    pub fn load(db: &WalletDatabase) -> ChainTracker {
        let chain = header_chain::mainnet_header_chain().expect("the compiled checkpoint is valid (P7a-A18)");
        Self::load_onto(chain, db)
    }

    /// `load` over any starting chain (tests use regtest parameters). Stored rows are re-checked
    /// against every rule except "no more than 2 h ahead of now": they passed it when stored, and a
    /// clock that boots behind must not delete a valid chain (adversarial review).
    pub fn load_onto(mut chain: HeaderChain, db: &WalletDatabase) -> ChainTracker {
        let repo = VerifiedHeaderRepository::new(db.connection());
        let rows = match repo.load_all() {
            Ok(r) => r,
            Err(e) => {
                log::warn!("⛓️  verified_headers unreadable ({}); starting from the checkpoint", e);
                Vec::new()
            }
        };
        let mut unreadable = 0usize;
        let rows: Vec<crate::database::verified_header_repo::StoredHeader> = rows
            .into_iter()
            .filter_map(|r| match r {
                Ok(row) => Some(row),
                Err(rowid) => {
                    unreadable += 1;
                    log::warn!("⛓️  dropping unreadable stored header row {}", rowid);
                    let _ = repo.delete_rowid(rowid);
                    None
                }
            })
            .collect();
        let mut dropped = 0usize;
        for row in &rows {
            let ok = Header::parse(&row.header)
                .and_then(|h| {
                    if header_chain::display_hex(&h.hash()) == row.hash {
                        Ok(h)
                    } else {
                        Err(HeaderError::Malformed("stored hash is not the header's hash".into()))
                    }
                })
                .and_then(|h| chain.add_header(h, u32::MAX));
            if let Err(e) = ok {
                dropped += 1;
                log::warn!("⛓️  dropping stored header {} — it fails a header rule: {}", row.hash, e);
                let _ = repo.delete_rowid(row.rowid);
            }
        }
        for row in &rows {
            if let Ok(hash) = header_chain::from_display_hex(&row.hash) {
                if let Some(e) = chain.get(&hash) {
                    let active = chain.header_at(e.height).map(|a| a.hash) == Some(hash);
                    if active != row.is_active {
                        let _ = repo.set_active(&row.hash, active);
                    }
                }
            }
        }
        log::info!("⛓️  header chain loaded: tip {} ({} stored, {} dropped, {} unreadable)", chain.tip_height(), rows.len() - dropped, dropped, unreadable);
        ChainTracker::from_chain(chain)
    }

    /// Check and keep a run of headers; stops at the first refusal. Persists what was kept.
    pub fn ingest(&self, db: &Mutex<WalletDatabase>, headers: &[Header], now: u32) -> IngestReport {
        let mut report = IngestReport::default();
        let mut inserts: Vec<(String, u32, String, [u8; 80], [u8; 32], bool)> = Vec::new();
        let mut flags: Vec<(String, bool)> = Vec::new();
        {
            let mut chain = self.chain.write().unwrap();
            for h in headers {
                match chain.add_header(*h, now) {
                    Ok(AddOutcome::Known) => report.known += 1,
                    Ok(outcome) => {
                        report.added += 1;
                        let e = *chain.get(&h.hash()).expect("just added");
                        let active = !matches!(outcome, AddOutcome::Fork { .. });
                        inserts.push((
                            header_chain::display_hex(&e.hash),
                            e.height,
                            header_chain::display_hex(&h.prev_hash),
                            h.serialize(),
                            e.chainwork.to_be_bytes(),
                            active,
                        ));
                        if let AddOutcome::Reorg { fork_height, new_tip_height, deactivated } = outcome {
                            for (_, hash) in &deactivated {
                                flags.push((header_chain::display_hex(hash), false));
                            }
                            for height in fork_height + 1..=new_tip_height {
                                if let Some(a) = chain.header_at(height) {
                                    flags.push((header_chain::display_hex(&a.hash), true));
                                }
                            }
                            let dropped: Vec<(u32, String)> = deactivated.iter().map(|(ht, hash)| (*ht, header_chain::display_hex(hash))).collect();
                            log::warn!("⛓️  reorg: fork at {}, new tip {}, deactivated {:?}", fork_height, new_tip_height, dropped);
                            report.reorgs.push((fork_height, dropped));
                        }
                    }
                    Err(e) => {
                        report.refused = Some(e);
                        break;
                    }
                }
            }
        }
        if !inserts.is_empty() || !flags.is_empty() {
            let guard = db.lock().unwrap();
            // One transaction per batch: the wallet DB runs synchronous=FULL, so a commit per header
            // was a disk flush per header under the DB lock (`/code-review max 159` F5).
            let began = guard.connection().execute_batch("BEGIN").is_ok();
            let repo = VerifiedHeaderRepository::new(guard.connection());
            for (hash, height, prev, header, cw, active) in &inserts {
                if let Err(e) = repo.insert(hash, *height, prev, header, cw, *active) {
                    log::warn!("⛓️  could not store header {}: {}", hash, e);
                }
            }
            for (hash, active) in &flags {
                let _ = repo.set_active(hash, *active);
            }
            if began {
                if let Err(e) = guard.connection().execute_batch("COMMIT") {
                    log::warn!("⛓️  could not commit stored headers: {}", e);
                    let _ = guard.connection().execute_batch("ROLLBACK");
                }
            }
        }
        report
    }

    /// `root` in internal byte order (as it sits in a header).
    pub fn is_valid_root_for_height(&self, root: &[u8; 32], height: u32) -> RootVerdict {
        self.chain.read().unwrap().is_valid_root_for_height(root, height)
    }

    /// The verified tip's height (never below the checkpoint).
    pub fn current_height(&self) -> u32 {
        self.chain.read().unwrap().tip_height()
    }

    /// Whether a sync in this process has reached a source's tip.
    pub fn is_synced(&self) -> bool {
        self.synced.load(Ordering::Relaxed)
    }

    /// The raw 80 bytes of the main-chain header at `height`, if held.
    pub fn header_for_height(&self, height: u32) -> Option<[u8; 80]> {
        self.chain.read().unwrap().header_at(height).map(|e| e.header.serialize())
    }

    /// Median time of the last 11 main-chain headers (`IsFinalTx`'s time cut-off).
    pub fn tip_median_time_past(&self) -> u32 {
        let chain = self.chain.read().unwrap();
        let tip = chain.tip().hash;
        chain.median_time_past(&tip).expect("the tip is held")
    }

    /// The main-chain height of the block with this display-hex hash, if held. A header held only
    /// on a side branch is `None`: its height is not where the main chain is (`P7a2-C12b`).
    pub fn height_for_hash(&self, block_hash: &str) -> Option<u32> {
        let mut hash: [u8; 32] = hex::decode(block_hash).ok()?.try_into().ok()?;
        hash.reverse(); // display → internal
        let chain = self.chain.read().unwrap();
        let entry = chain.get(&hash)?;
        (chain.header_at(entry.height)?.hash == hash).then_some(entry.height)
    }

    pub fn checkpoint_height(&self) -> u32 {
        self.chain.read().unwrap().checkpoint().height
    }

    fn tip_is_fresh(&self, now: u32) -> bool {
        self.chain.read().unwrap().tip().header.time.saturating_add(MAX_TIP_AGE_SECS) >= now
    }

    #[cfg(test)]
    pub fn mark_synced_for_test(&self) {
        self.synced.store(true, Ordering::Relaxed);
    }
}

impl crate::proof_store::RootOracle for ChainTracker {
    fn root_verdict(&self, root: &[u8; 32], height: u32) -> RootVerdict {
        self.is_valid_root_for_height(root, height)
    }
    fn height_for_hash(&self, block_hash: &str) -> Option<u32> {
        ChainTracker::height_for_hash(self, block_hash)
    }
    fn holds_height(&self, height: u32) -> bool {
        self.header_for_height(height).is_some()
    }
}

/// A place headers can be fetched from. Never trusted: whatever it returns goes through
/// `HeaderChain::add_header`.
#[async_trait::async_trait]
pub trait HeaderSource: Send + Sync {
    fn name(&self) -> &str;
    /// Up to `count` consecutive raw 80-byte headers of the source's own main chain, starting at
    /// `from_height` (fewer when the source's tip is closer).
    async fn headers_from(&self, from_height: u32, count: u32) -> Result<Vec<u8>, String>;
}

/// A chaintracks server's v2 binary range route (`{base}/headers.bin?height=&count=`): BSVA's
/// go-chaintracks behind Arcade and the TS chaintracks server both answer it (live 2026-10-09).
pub struct ChaintracksSource {
    pub name: String,
    pub base: String,
    pub client: reqwest::Client,
}

#[async_trait::async_trait]
impl HeaderSource for ChaintracksSource {
    fn name(&self) -> &str {
        &self.name
    }

    async fn headers_from(&self, from_height: u32, count: u32) -> Result<Vec<u8>, String> {
        let url = format!("{}/headers.bin?height={}&count={}", self.base, from_height, count);
        let mut resp = self.client.get(&url).send().await.map_err(|e| format!("{}: {}", self.name, e))?;
        if !resp.status().is_success() {
            return Err(format!("{}: HTTP {}", self.name, resp.status()));
        }
        // Never buffer more than the headers asked for (a source is not trusted with our memory).
        let max = count as usize * header_chain::HEADER_LEN;
        if resp.content_length().map_or(false, |n| n as usize > max) {
            return Err(format!("{}: response larger than {} bytes", self.name, max));
        }
        let mut buf = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| format!("{}: {}", self.name, e))? {
            if buf.len() + chunk.len() > max {
                return Err(format!("{}: response larger than {} bytes", self.name, max));
            }
            buf.extend_from_slice(&chunk);
        }
        Ok(buf)
    }
}

/// The header sources the wallet uses, in order (`KICKOFF_RESEARCH_PRIOR_ART.md` §2).
pub fn default_header_sources(client: reqwest::Client) -> Vec<Box<dyn HeaderSource>> {
    vec![
        Box::new(ChaintracksSource {
            name: "arcade-chaintracks".into(),
            base: "https://arcade-v2-us-1.bsvblockchain.tech/chaintracks/v2".into(),
            client: client.clone(),
        }),
        Box::new(ChaintracksSource { name: "bsvb-chaintracks".into(), base: "https://chaintracks-us-1.bsvb.tech/v2".into(), client }),
    ]
}

/// What a sync did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub source: String,
    pub added: usize,
    pub reorgs: Vec<(u32, Vec<(u32, String)>)>,
}

/// Bring the chain up to date from EVERY source in turn, so most-work selection compares their
/// chains (a stalled or withholding first source cannot hide a heavier one). A source that fails or
/// sends a header we refuse is skipped. `synced` is set only when a source's tip was reached AND our
/// tip is fresh, and cleared when it is not. `Err` only when every source failed — and an `Err` is
/// never a verdict about any proof (`P7a-A10`): the chain simply stays where it was.
pub async fn sync_once(tracker: &ChainTracker, db: &Mutex<WalletDatabase>, sources: &[Box<dyn HeaderSource>], now: u32) -> Result<SyncReport, String> {
    let mut errors = Vec::new();
    let mut merged: Option<SyncReport> = None;
    for source in sources {
        match sync_from(tracker, db, source.as_ref(), now).await {
            Ok(r) => {
                let m = merged.get_or_insert_with(SyncReport::default);
                if !m.source.is_empty() {
                    m.source.push(',');
                }
                m.source.push_str(&r.source);
                m.added += r.added;
                m.reorgs.extend(r.reorgs);
            }
            Err(e) => {
                log::warn!("⛓️  header source {} failed: {}", source.name(), e);
                errors.push(e);
            }
        }
    }
    let fresh = tracker.tip_is_fresh(now);
    tracker.synced.store(merged.is_some() && fresh, Ordering::Relaxed);
    match merged {
        Some(r) => {
            if !fresh {
                log::warn!("⛓️  sources answered but our tip ({}) is over {} h old — not synced", tracker.current_height(), MAX_TIP_AGE_SECS / 3600);
            }
            Ok(r)
        }
        None => Err(format!("every header source failed: {}", errors.join("; "))),
    }
}

async fn sync_from(tracker: &ChainTracker, db: &Mutex<WalletDatabase>, source: &dyn HeaderSource, now: u32) -> Result<SyncReport, String> {
    let mut report = SyncReport { source: source.name().to_string(), ..Default::default() };
    let floor = tracker.checkpoint_height() + 1;
    let mut overlap = SYNC_OVERLAP;
    loop {
        let tip = tracker.current_height();
        let from = (tip + 1).saturating_sub(overlap).max(floor);
        let bytes = source.headers_from(from, SYNC_BATCH).await?;
        let headers = header_chain::parse_headers(&bytes).map_err(|e| format!("{}: {}", source.name(), e))?;
        if headers.is_empty() {
            return Ok(report);
        }
        if headers.len() > SYNC_BATCH as usize {
            return Err(format!("{}: sent {} headers, more than the {} asked for", source.name(), headers.len(), SYNC_BATCH));
        }
        let r = tracker.ingest(db, &headers, now);
        report.added += r.added;
        report.reorgs.extend(r.reorgs);
        match r.refused {
            Some(HeaderError::UnknownParent) if r.added == 0 && r.known == 0 => {
                if from == floor || overlap >= MAX_SYNC_OVERLAP {
                    return Err(format!("{}: its chain does not connect to ours (checked back to height {})", source.name(), from));
                }
                overlap = (overlap * 8).min(MAX_SYNC_OVERLAP);
                continue;
            }
            Some(e) => return Err(format!("{}: sent a header we refuse at or after height {}: {}", source.name(), from, e)),
            None => {}
        }
        if (headers.len() as u32) < SYNC_BATCH {
            return Ok(report);
        }
        // A full batch that did not move our tip (all known, or a lighter fork) ends this source:
        // otherwise a source that keeps sending it would spin here forever (adversarial review).
        if tracker.current_height() <= tip {
            return Err(format!("{}: sent a full batch from height {} that did not advance our tip", source.name(), from));
        }
        overlap = SYNC_OVERLAP;
    }
}

/// How often headers are polled when no push arrives.
pub const HEADER_POLL: std::time::Duration = std::time::Duration::from_secs(30);
/// BSVA Arcade's chaintracks tip stream (SSE): an event per new block.
pub const TIP_STREAM_URL: &str = "https://arcade-v2-us-1.bsvblockchain.tech/chaintracks/v2/tip/stream";

/// Headers by push, polling as backup (owner, 2026-10-07): sync now, then whenever `push` is
/// notified (a tip-stream event) and at least every `poll`. Runs until `shutdown`.
pub async fn run_header_sync(
    tracker: std::sync::Arc<ChainTracker>,
    db: std::sync::Arc<Mutex<WalletDatabase>>,
    sources: Vec<Box<dyn HeaderSource>>,
    push: std::sync::Arc<tokio::sync::Notify>,
    poll: std::time::Duration,
    shutdown: tokio_util::sync::CancellationToken,
) {
    loop {
        match sync_once(&tracker, &db, &sources, now_secs()).await {
            Ok(r) if r.added > 0 => log::info!("⛓️  headers: +{} from {}, tip {}", r.added, r.source, tracker.current_height()),
            Ok(_) => log::debug!("⛓️  headers: up to date at {}", tracker.current_height()),
            Err(e) => log::warn!("⛓️  header sync failed (tip stays {}): {}", tracker.current_height(), e),
        }
        tokio::select! {
            _ = shutdown.cancelled() => return,
            _ = push.notified() => {}
            _ = tokio::time::sleep(poll) => {}
        }
    }
}

/// Finds `data:` lines in a server-sent-event stream however the bytes are chunked.
#[derive(Default)]
pub struct SseEventDetector {
    line: Vec<u8>,
}

impl SseEventDetector {
    /// Feed one chunk; `true` when it completed at least one `data:` line.
    pub fn feed(&mut self, chunk: &[u8]) -> bool {
        let mut event = false;
        for &b in chunk {
            if b == b'\n' {
                if self.line.starts_with(b"data:") {
                    event = true;
                }
                self.line.clear();
            } else if b != b'\r' && self.line.len() < 64 {
                self.line.push(b);
            }
        }
        event
    }
}

/// Listen to a chaintracks tip stream and wake the header sync on every event. Reconnects after
/// 30 s when the stream drops; polling covers the gap.
pub async fn run_tip_stream(url: String, push: std::sync::Arc<tokio::sync::Notify>, shutdown: tokio_util::sync::CancellationToken) {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    loop {
        if let Ok(mut resp) = client.get(&url).header("Accept", "text/event-stream").send().await {
            if resp.status().is_success() {
                log::info!("⛓️  tip stream connected: {}", url);
                let mut detector = SseEventDetector::default();
                loop {
                    tokio::select! {
                        _ = shutdown.cancelled() => return,
                        chunk = tokio::time::timeout(std::time::Duration::from_secs(60), resp.chunk()) => match chunk {
                            // `: keepalive` comments carry no `data:` and wake nothing.
                            Ok(Ok(Some(bytes))) => if detector.feed(&bytes) { push.notify_one(); },
                            _ => break,
                        },
                    }
                }
                log::info!("⛓️  tip stream dropped; polling covers it until it reconnects");
            }
        }
        tokio::select! {
            _ = shutdown.cancelled() => return,
            _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header_chain::{display_hex, mainnet_header_chain, parse_headers, ChainParams, Checkpoint};

    const ARCADE_970001_970300: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/headers/arcade_970001_970300.bin"));

    fn temp_db(tag: &str) -> Mutex<WalletDatabase> {
        let path = std::env::temp_dir().join(format!("hodos_chain_tracker_{}_{}_{}.db", tag, std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        Mutex::new(WalletDatabase::new(path).unwrap())
    }

    /// A minute after the fixture's last header (970,300): fresh for a synced tip, and every
    /// fixture header is in the past.
    fn far_future() -> u32 {
        Header::parse(&ARCADE_970001_970300[299 * 80..]).unwrap().time + 60
    }

    /// Serves `data` (consecutive headers starting at `base`) like a chaintracks range route.
    /// ⚠️ Its default is not "a working chain": a source made with `fail` answers every call with Err.
    struct SliceSource {
        name: String,
        base: u32,
        data: Vec<u8>,
        fail: bool,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl SliceSource {
        fn serving(name: &str, base: u32, data: &[u8]) -> Box<dyn HeaderSource> {
            Box::new(SliceSource { name: name.into(), base, data: data.to_vec(), fail: false, calls: Default::default() })
        }
        fn failing(name: &str) -> Box<dyn HeaderSource> {
            Box::new(SliceSource { name: name.into(), base: 0, data: vec![], fail: true, calls: Default::default() })
        }
    }

    #[async_trait::async_trait]
    impl HeaderSource for SliceSource {
        fn name(&self) -> &str {
            &self.name
        }
        async fn headers_from(&self, from: u32, count: u32) -> Result<Vec<u8>, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if self.fail {
                return Err(format!("{}: unreachable", self.name));
            }
            let n = (self.data.len() / 80) as u32;
            let start = from.saturating_sub(self.base).min(n);
            let end = (start + count).min(n);
            Ok(self.data[(start * 80) as usize..(end * 80) as usize].to_vec())
        }
    }

    fn stored_rows(db: &Mutex<WalletDatabase>) -> i64 {
        db.lock().unwrap().connection().query_row("SELECT COUNT(*) FROM verified_headers", [], |r| r.get(0)).unwrap()
    }

    #[tokio::test]
    async fn sync_brings_the_chain_to_the_source_tip_and_persists_it() {
        let db = temp_db("sync");
        let tracker = ChainTracker::from_chain(mainnet_header_chain().unwrap());
        assert!(!tracker.is_synced());
        let sources = vec![SliceSource::serving("fake", 970_001, ARCADE_970001_970300)];
        let r = sync_once(&tracker, &db, &sources, far_future()).await.unwrap();
        assert_eq!(r.added, 300);
        assert_eq!(tracker.current_height(), 970_300);
        assert!(tracker.is_synced());
        assert_eq!(stored_rows(&db), 300);
        // A second pass adds nothing.
        assert_eq!(sync_once(&tracker, &db, &sources, far_future()).await.unwrap().added, 0);
    }

    #[tokio::test]
    async fn a10_every_source_failing_is_an_error_never_a_verdict() {
        let db = temp_db("a10");
        let tracker = ChainTracker::from_chain(mainnet_header_chain().unwrap());
        let sources = vec![SliceSource::failing("a"), SliceSource::failing("b")];
        assert!(sync_once(&tracker, &db, &sources, far_future()).await.is_err());
        assert!(!tracker.is_synced());
        assert_eq!(tracker.current_height(), 970_000, "the chain stays where it was");
        let any_root = [0x11u8; 32];
        assert_eq!(tracker.is_valid_root_for_height(&any_root, 970_001), RootVerdict::Unknown, "not held is never Invalid");
    }

    #[tokio::test]
    async fn a_source_that_sends_a_bad_header_is_skipped_for_the_next() {
        let db = temp_db("badsrc");
        let tracker = ChainTracker::from_chain(mainnet_header_chain().unwrap());
        let mut tampered = ARCADE_970001_970300.to_vec();
        tampered[149 * 80 + 76] ^= 1; // nonce of 970,150
        let sources = vec![SliceSource::serving("liar", 970_001, &tampered), SliceSource::serving("honest", 970_001, ARCADE_970001_970300)];
        let r = sync_once(&tracker, &db, &sources, far_future()).await.unwrap();
        assert_eq!(r.source, "honest");
        assert_eq!(tracker.current_height(), 970_300);
        assert_eq!(display_hex(&Header::parse(&tracker.header_for_height(970_150).unwrap()).unwrap().hash()),
                   display_hex(&Header::parse(&ARCADE_970001_970300[149 * 80..150 * 80]).unwrap().hash()), "the real 970,150 is held");
    }

    #[tokio::test]
    async fn a7_a_source_on_another_chain_does_not_connect() {
        let db = temp_db("a7");
        let tracker = ChainTracker::from_chain(mainnet_header_chain().unwrap());
        // Real headers, but served as if they started at 980,000: their parents are not ours.
        let sources = vec![SliceSource::serving("elsewhere", 970_001, &ARCADE_970001_970300[160 * 80..])];
        let e = sync_once(&tracker, &db, &sources, far_future()).await.unwrap_err();
        assert!(e.contains("does not connect"), "{}", e);
        assert_eq!(tracker.current_height(), 970_000);
    }

    #[tokio::test]
    async fn a17_restart_keeps_the_tip_and_drops_a_planted_row() {
        let db = temp_db("a17");
        let tracker = ChainTracker::from_chain(mainnet_header_chain().unwrap());
        sync_once(&tracker, &db, &[SliceSource::serving("fake", 970_001, ARCADE_970001_970300)], far_future()).await.unwrap();
        // Plant a row the chain never accepted: the real next header's shape with a bad nonce.
        let mut planted = Header::parse(&ARCADE_970001_970300[299 * 80..]).unwrap();
        planted.prev_hash = Header::parse(&ARCADE_970001_970300[299 * 80..]).unwrap().hash();
        planted.nonce ^= 7;
        {
            let g = db.lock().unwrap();
            VerifiedHeaderRepository::new(g.connection())
                .insert(&display_hex(&planted.hash()), 970_301, &display_hex(&planted.prev_hash), &planted.serialize(), &[0u8; 32], true)
                .unwrap();
        }
        assert_eq!(stored_rows(&db), 301);
        let reloaded = ChainTracker::load_onto(mainnet_header_chain().unwrap(), &db.lock().unwrap());
        assert_eq!(reloaded.current_height(), 970_300, "the tip survives a restart without a re-sync");
        assert_eq!(stored_rows(&db), 300, "the planted row is deleted, not trusted");
        assert!(reloaded.header_for_height(970_301).is_none());
    }

    // ---------- a reorg through ingest is persisted (regtest params) ----------

    fn regtest_mine(prev_hash: [u8; 32], time: u32, tag: u8) -> Header {
        let mut h = Header { version: 1, prev_hash, merkle_root: [tag; 32], time, bits: 0x207fffff, nonce: 0 };
        while header_chain::check_proof_of_work(&h.hash(), h.bits, &ChainParams::regtest().pow_limit).is_err() {
            h.nonce += 1;
        }
        h
    }

    #[test]
    fn a11_reorg_through_ingest_flips_the_stored_active_flags() {
        let db = temp_db("a11");
        let g = regtest_mine([0; 32], 1_000_000, 0xaa);
        let cp = Checkpoint { height: 100, hash: g.hash(), chainwork: header_chain::work_for_bits(g.bits) };
        let tracker = ChainTracker::from_chain(HeaderChain::from_anchor(ChainParams::regtest(), cp, &[g]).unwrap());
        let a1 = regtest_mine(g.hash(), 1_000_600, 1);
        let a2 = regtest_mine(a1.hash(), 1_001_200, 2);
        assert_eq!(tracker.ingest(&db, &[a1, a2], far_future()).added, 2);
        let b2 = regtest_mine(a1.hash(), 1_001_200, 3);
        let b3 = regtest_mine(b2.hash(), 1_001_800, 4);
        let r = tracker.ingest(&db, &[b2, b3], far_future());
        assert_eq!(r.reorgs, vec![(101, vec![(102, display_hex(&a2.hash()))])]);
        let active = |h: &Header| -> i64 {
            db.lock().unwrap().connection().query_row("SELECT is_active FROM verified_headers WHERE hash = ?1", [display_hex(&h.hash())], |r| r.get(0)).unwrap()
        };
        assert_eq!((active(&a1), active(&a2), active(&b2), active(&b3)), (1, 0, 1, 1));
    }

    /// `P7a2-C12b`: a block's height comes from our main chain only — a header that left it in a
    /// reorg has no height to give.
    #[test]
    fn c12b_height_for_hash_answers_only_for_the_main_chain() {
        let db = temp_db("c12b");
        let g = regtest_mine([0; 32], 1_000_000, 0xaa);
        let cp = Checkpoint { height: 100, hash: g.hash(), chainwork: header_chain::work_for_bits(g.bits) };
        let tracker = ChainTracker::from_chain(HeaderChain::from_anchor(ChainParams::regtest(), cp, &[g]).unwrap());
        let a1 = regtest_mine(g.hash(), 1_000_600, 1);
        let a2 = regtest_mine(a1.hash(), 1_001_200, 2);
        tracker.ingest(&db, &[a1, a2], far_future());
        assert_eq!(tracker.height_for_hash(&display_hex(&a2.hash())), Some(102));
        let b2 = regtest_mine(a1.hash(), 1_001_200, 3);
        let b3 = regtest_mine(b2.hash(), 1_001_800, 4);
        tracker.ingest(&db, &[b2, b3], far_future());
        assert_eq!(tracker.height_for_hash(&display_hex(&a2.hash())), None, "a header off the main chain");
        assert_eq!(tracker.height_for_hash(&display_hex(&b2.hash())), Some(102));
        assert_eq!(tracker.height_for_hash(&display_hex(&a1.hash())), Some(101));
        assert_eq!(tracker.height_for_hash(&"cd".repeat(32)), None);
        assert_eq!(tracker.height_for_hash("not hex"), None);
    }

    // ---------- `P7a-A16`: push, with polling as backup ----------

    async fn wait_for_tip(tracker: &ChainTracker, height: u32, within: std::time::Duration) -> bool {
        let deadline = tokio::time::Instant::now() + within;
        while tokio::time::Instant::now() < deadline {
            if tracker.current_height() == height {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        false
    }

    /// Serves `ARCADE_970001_970300` up to a tip the test moves.
    struct GrowingSource {
        tip: std::sync::Arc<std::sync::atomic::AtomicU32>,
    }

    #[async_trait::async_trait]
    impl HeaderSource for GrowingSource {
        fn name(&self) -> &str {
            "growing"
        }
        async fn headers_from(&self, from: u32, count: u32) -> Result<Vec<u8>, String> {
            let tip = self.tip.load(Ordering::Relaxed);
            if from > tip {
                return Ok(vec![]);
            }
            let start = (from - 970_001) as usize;
            let end = ((tip - 970_001 + 1) as usize).min(start + count as usize);
            Ok(ARCADE_970001_970300[start * 80..end * 80].to_vec())
        }
    }

    async fn a16_rig(poll: std::time::Duration) -> (std::sync::Arc<ChainTracker>, std::sync::Arc<std::sync::atomic::AtomicU32>, std::sync::Arc<tokio::sync::Notify>, tokio_util::sync::CancellationToken) {
        let tracker = std::sync::Arc::new(ChainTracker::from_chain(mainnet_header_chain().unwrap()));
        let tip = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(970_100));
        let push = std::sync::Arc::new(tokio::sync::Notify::new());
        let stop = tokio_util::sync::CancellationToken::new();
        let db = std::sync::Arc::new(temp_db("a16"));
        let sources: Vec<Box<dyn HeaderSource>> = vec![Box::new(GrowingSource { tip: tip.clone() })];
        tokio::spawn(run_header_sync(tracker.clone(), db, sources, push.clone(), poll, stop.clone()));
        assert!(wait_for_tip(&tracker, 970_100, std::time::Duration::from_secs(5)).await, "first sync runs at once");
        (tracker, tip, push, stop)
    }

    #[tokio::test]
    async fn a16_a_push_syncs_without_waiting_for_the_poll() {
        let (tracker, tip, push, stop) = a16_rig(std::time::Duration::from_secs(3600)).await;
        tip.store(970_200, Ordering::Relaxed);
        push.notify_one();
        assert!(wait_for_tip(&tracker, 970_200, std::time::Duration::from_secs(5)).await, "a push wakes the sync");
        stop.cancel();
    }

    #[tokio::test]
    async fn a16_polling_catches_up_when_no_push_arrives() {
        let (tracker, tip, _push, stop) = a16_rig(std::time::Duration::from_millis(50)).await;
        tip.store(970_300, Ordering::Relaxed);
        assert!(wait_for_tip(&tracker, 970_300, std::time::Duration::from_secs(5)).await, "the poll catches up");
        stop.cancel();
    }

    // ---------- adversarial review 2026-10-09 ----------

    /// Always answers the same full batch, whatever height is asked.
    struct StuckSource;

    #[async_trait::async_trait]
    impl HeaderSource for StuckSource {
        fn name(&self) -> &str {
            "stuck"
        }
        async fn headers_from(&self, _from: u32, _count: u32) -> Result<Vec<u8>, String> {
            let mut v = ARCADE_970001_970300.to_vec();
            while v.len() < SYNC_BATCH as usize * 80 {
                v.extend_from_slice(ARCADE_970001_970300);
            }
            v.truncate(SYNC_BATCH as usize * 80);
            Ok(v)
        }
    }

    // Multi-thread runtime: the bug this guards is a loop that never yields, which a timeout on
    // the same worker could not interrupt — the sync runs on another worker so the timeout fires.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_source_whose_full_batch_never_advances_us_ends_and_the_next_is_asked() {
        let db = std::sync::Arc::new(temp_db("stuck"));
        let tracker = std::sync::Arc::new(ChainTracker::from_chain(mainnet_header_chain().unwrap()));
        let (t, d) = (tracker.clone(), db.clone());
        let run = tokio::spawn(async move {
            let sources: Vec<Box<dyn HeaderSource>> = vec![Box::new(StuckSource), SliceSource::serving("honest", 970_001, ARCADE_970001_970300)];
            sync_once(&t, &d, &sources, far_future()).await
        });
        let r = tokio::time::timeout(std::time::Duration::from_secs(10), run)
            .await
            .expect("sync_once returns: a source cannot spin it forever")
            .unwrap()
            .unwrap();
        assert_eq!(r.source, "honest");
        assert_eq!(tracker.current_height(), 970_300);
    }

    #[tokio::test]
    async fn every_source_is_asked_so_an_empty_first_source_cannot_hide_the_chain() {
        let db = temp_db("empty1st");
        let tracker = ChainTracker::from_chain(mainnet_header_chain().unwrap());
        let sources = vec![SliceSource::serving("empty", 970_001, &[]), SliceSource::serving("honest", 970_001, ARCADE_970001_970300)];
        sync_once(&tracker, &db, &sources, far_future()).await.unwrap();
        assert_eq!(tracker.current_height(), 970_300);
        assert!(tracker.is_synced());
    }

    #[tokio::test]
    async fn an_empty_answer_at_a_stale_tip_is_not_synced() {
        let db = temp_db("stale");
        let tracker = ChainTracker::from_chain(mainnet_header_chain().unwrap());
        let sources = vec![SliceSource::serving("empty", 970_001, &[])];
        assert!(sync_once(&tracker, &db, &sources, far_future()).await.is_ok());
        assert!(!tracker.is_synced(), "the checkpoint is two days older than the clock");
        // And synced clears when the tip goes stale later.
        sync_once(&tracker, &db, &[SliceSource::serving("honest", 970_001, ARCADE_970001_970300)], far_future()).await.unwrap();
        assert!(tracker.is_synced());
        assert!(sync_once(&tracker, &db, &sources, far_future() + MAX_TIP_AGE_SECS + 1).await.is_ok());
        assert!(!tracker.is_synced());
    }

    #[test]
    fn load_keeps_rows_a_slow_clock_would_call_too_new() {
        let db = temp_db("clock");
        let tracker = ChainTracker::from_chain(mainnet_header_chain().unwrap());
        let headers = parse_headers(ARCADE_970001_970300).unwrap();
        assert_eq!(tracker.ingest(&db, &headers, far_future()).added, 300);
        // load() no longer takes a clock; a reload keeps all 300 whatever time it is.
        let reloaded = ChainTracker::load_onto(mainnet_header_chain().unwrap(), &db.lock().unwrap());
        assert_eq!(reloaded.current_height(), 970_300);
        assert_eq!(stored_rows(&db), 300);
    }

    #[test]
    fn an_equal_work_tie_keeps_its_winner_across_a_restart() {
        let db = temp_db("tie");
        let g = regtest_mine([0; 32], 1_000_000, 0xaa);
        let cp = Checkpoint { height: 100, hash: g.hash(), chainwork: header_chain::work_for_bits(g.bits) };
        let chain = || HeaderChain::from_anchor(ChainParams::regtest(), cp, &[g]).unwrap();
        let x = regtest_mine(g.hash(), 1_000_600, 1);
        let y = regtest_mine(g.hash(), 1_000_600, 2);
        // The first seen wins; make it the one whose hash sorts LATER, so hash order alone would flip it.
        let (first, second) = if display_hex(&x.hash()) > display_hex(&y.hash()) { (x, y) } else { (y, x) };
        let tracker = ChainTracker::from_chain(chain());
        tracker.ingest(&db, &[first, second], far_future());
        assert_eq!(Header::parse(&tracker.header_for_height(101).unwrap()).unwrap().hash(), first.hash());
        let reloaded = ChainTracker::load_onto(chain(), &db.lock().unwrap());
        assert_eq!(Header::parse(&reloaded.header_for_height(101).unwrap()).unwrap().hash(), first.hash(), "same winner after restart");
    }

    #[test]
    fn a18_the_constructor_main_calls_is_mainnet_and_revalidates() {
        let db = temp_db("a18");
        // A planted row the real loader must drop.
        let mut planted = Header::parse(&ARCADE_970001_970300[..80]).unwrap();
        planted.nonce ^= 1;
        VerifiedHeaderRepository::new(db.lock().unwrap().connection())
            .insert(&display_hex(&planted.hash()), 970_001, &display_hex(&planted.prev_hash), &planted.serialize(), &[0u8; 32], true)
            .unwrap();
        let tracker = ChainTracker::load(&db.lock().unwrap());
        {
            let chain = tracker.chain.read().unwrap();
            let p = chain.params();
            assert_eq!((p.retarget, p.min_version, p.pow_limit), (header_chain::Retarget::Cw144, 4, ChainParams::mainnet().pow_limit));
        }
        assert_eq!(tracker.current_height(), 970_000);
        assert_eq!(display_hex(&Header::parse(&tracker.header_for_height(970_000).unwrap()).unwrap().hash()),
                   "00000000000000001894e5528239b925bf06f12ff48180718dd59374c894ac8c");
        assert_eq!(stored_rows(&db), 0, "the planted row is dropped by the real loader");
        // Mainnet difficulty: the FX4 min-difficulty header is refused (regtest params would take it).
        let fx4 = Header::parse(&hex::decode("000000208cac94c87493d58d718081f42ff106bf25b9398252e5941800000000000000000900000000000000464646464646464646464646464646464646464646464646a0e2c56affff001d9ed70c07").unwrap()).unwrap();
        let r = tracker.ingest(&db, &[fx4], fx4.time + 60);
        assert!(matches!(r.refused, Some(HeaderError::BadDifficulty { .. })), "{:?}", r);
        // The tie to production: main.rs builds the tracker with exactly this constructor, once.
        let main = include_str!("main.rs");
        assert_eq!(main.matches("ChainTracker::load(").count(), 1);
        assert_eq!(main.matches("ChainTracker::from_chain(").count() + main.matches("ChainTracker::load_onto(").count(), 0);
    }

    #[test]
    fn tip_median_time_past_is_the_chain_s_mtp_11() {
        let g = regtest_mine([0; 32], 1_000_000, 0xaa);
        let cp = Checkpoint { height: 100, hash: g.hash(), chainwork: header_chain::work_for_bits(g.bits) };
        let tracker = ChainTracker::from_chain(HeaderChain::from_anchor(ChainParams::regtest(), cp, &[g]).unwrap());
        let db = temp_db("mtp");
        let mut prev = g;
        let mut times = vec![g.time];
        for (i, o) in [1000u32, 2000, 1500, 3000, 2500, 4000, 3500, 5000, 4500, 6000, 5500, 7000].iter().enumerate() {
            let h = regtest_mine(prev.hash(), 1_000_000 + o, i as u8);
            assert_eq!(tracker.ingest(&db, &[h], far_future()).added, 1);
            prev = h;
            times.push(h.time);
        }
        let mut last11 = times[times.len() - 11..].to_vec();
        last11.sort_unstable();
        assert_eq!(tracker.tip_median_time_past(), last11[5]);
        assert!(tracker.tip_median_time_past() < prev.time);
    }

    #[test]
    fn sse_detector_finds_data_lines_however_chunked() {
        let mut d = SseEventDetector::default();
        assert!(!d.feed(b": keepalive\n\n"));
        assert!(!d.feed(b"da"));
        assert!(d.feed(b"ta: {\"height\":1}\n\n"), "a data: line split across chunks");
        assert!(d.feed(b"data: a\n\ndata: b\n\n"));
        assert!(!d.feed(b"event: x\n"));
    }

    /// Live (opt-in): every default source answers a batch request of SYNC_BATCH from the checkpoint with headers
    /// that link to it. Seen red 2026-10-09 with SYNC_BATCH = 2000 (bsvb: HTTP 400, count <= 1000).
    #[tokio::test]
    #[ignore] // network: cargo test --lib live_default_sources -- --ignored
    async fn live_default_sources_answer_a_full_batch() {
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().unwrap();
        for src in default_header_sources(client) {
            let bytes = src.headers_from(970_001, SYNC_BATCH).await.unwrap_or_else(|e| panic!("{}: {}", src.name(), e));
            let hs = parse_headers(&bytes).unwrap();
            assert!(!hs.is_empty() && hs.len() <= SYNC_BATCH as usize, "{}: {} headers", src.name(), hs.len());
            let mut chain = mainnet_header_chain().unwrap();
            for h in &hs {
                chain.add_header(*h, u32::MAX).unwrap_or_else(|e| panic!("{}: {}", src.name(), e));
            }
        }
    }

    /// `/code-review max 159` F5 (measurement, opt-in): the wallet DB runs `synchronous=FULL`, so one
    /// commit per header is one disk flush per header, under the DB lock every request needs.
    #[test]
    #[ignore] // timing: cargo test --lib ingest_300_headers_timing -- --ignored --nocapture
    fn ingest_300_headers_timing() {
        let db = temp_db("f5");
        let tracker = ChainTracker::from_chain(mainnet_header_chain().unwrap());
        let headers = parse_headers(ARCADE_970001_970300).unwrap();
        let t = std::time::Instant::now();
        assert_eq!(tracker.ingest(&db, &headers, far_future()).added, 300);
        println!("F5: ingest of 300 headers took {:?}", t.elapsed());
    }

    /// `/code-review max 159` V7: a heavier branch that forked 500 blocks below our tip. The widest
    /// retry must still reach the fork point: an overlap wider than the batch asked only for headers
    /// we already held, stored nothing, and the wallet could never reorg.
    #[tokio::test]
    async fn a_heavier_branch_forked_500_blocks_back_is_reached() {
        let g = regtest_mine([0; 32], 1_000_000, 0xaa);
        let cp = Checkpoint { height: 100, hash: g.hash(), chainwork: header_chain::work_for_bits(g.bits) };
        let tracker = ChainTracker::from_chain(HeaderChain::from_anchor(ChainParams::regtest(), cp, &[g]).unwrap());
        let db = temp_db("v7");
        let mut ours = Vec::new();
        let mut prev = g;
        for i in 0..2500u32 {
            let h = regtest_mine(prev.hash(), 1_000_000 + (i + 1) * 600, (i % 250) as u8);
            ours.push(h);
            prev = h;
        }
        assert_eq!(tracker.ingest(&db, &ours, u32::MAX).added, 2500);
        assert_eq!(tracker.current_height(), 2600);
        // The source's chain: our first 2,000 blocks (to height 2100), then 501 of its own.
        let mut theirs: Vec<Header> = ours[..2000].to_vec();
        let mut prev = ours[1999];
        for i in 0..501u32 {
            let h = regtest_mine(prev.hash(), prev.time + 600, 0xf0u8.wrapping_add((i % 13) as u8));
            theirs.push(h);
            prev = h;
        }
        let data: Vec<u8> = theirs.iter().flat_map(|h| h.serialize()).collect();
        let sources = vec![SliceSource::serving("heavier", 101, &data)];
        let r = sync_once(&tracker, &db, &sources, u32::MAX).await;
        let r2 = sync_once(&tracker, &db, &sources, u32::MAX).await;
        assert_eq!(tracker.current_height(), 2601, "reorged to the heavier branch ({:?} / {:?})", r, r2);
        assert_eq!(tracker.header_for_height(2601).map(|b| Header::parse(&b).unwrap().hash()), Some(prev.hash()));
    }

    /// `/code-review max 159` V20: one row of the wrong storage class (an 80-character TEXT passes
    /// `length(header) = 80`) used to fail the whole load: every valid row was ignored at every start
    /// and the bad one was never deleted. It must be dropped on its own.
    #[test]
    fn a17_an_unreadable_row_is_dropped_and_the_rest_still_load() {
        let db = temp_db("v20");
        let tracker = ChainTracker::from_chain(mainnet_header_chain().unwrap());
        assert_eq!(tracker.ingest(&db, &parse_headers(ARCADE_970001_970300).unwrap(), far_future()).added, 300);
        db.lock().unwrap().connection().execute(
            "INSERT INTO verified_headers (hash, height, prev_hash, header, chainwork, is_active) VALUES ('planted', 970301, 'x', ?1, zeroblob(32), 1)",
            ["x".repeat(80)]).unwrap();
        let reloaded = ChainTracker::load_onto(mainnet_header_chain().unwrap(), &db.lock().unwrap());
        assert_eq!(reloaded.current_height(), 970_300, "the valid rows still load");
        assert_eq!(stored_rows(&db), 300, "the unreadable row is deleted");
    }
}
