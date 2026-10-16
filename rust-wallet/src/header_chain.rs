//! B5-T1-P7a-1: the verified block-header chain — pure logic, no network, no database.
//!
//! The wallet keeps its own chain of block headers, starting from a checkpoint compiled into the
//! release, and believes a merkle root only if it is in a header this module accepted. A header is
//! accepted only if, in this order:
//! 1. its parent is a header we already hold (linkage), at or above the checkpoint;
//! 2. its hash meets the target its own `bits` encode, and that target is a sane compact encoding
//!    no easier than the proof-of-work limit (`pow.cpp :: CheckProofOfWork`);
//! 3. its `bits` equal what the difficulty rule requires — BSV mainnet's CW-144 rule
//!    (`pow.cpp :: GetNextCashWorkRequired`), so a source cannot hand us a cheap low-difficulty chain;
//! 4. its time is after the median of the previous 11 and no more than 2 h ahead of now, and its
//!    version is at least 4 (BIP34/66/65 heights are all below the checkpoint)
//!    (`validation.cpp :: ContextualCheckBlockHeader`);
//! 5. it does not fork from more than `MAX_FORK_DEPTH` below our tip (side branches cannot pile up).
//!
//! Chain work is the sum of each header's work; the active chain is the one with the most work (a
//! tie never displaces the current tip). A switch to a heavier branch is reported with the headers it
//! deactivated, so callers can re-check proofs in exactly those blocks. Nothing here reverts anything.
//!
//! Prior art (working rule 5, `development-docs/PRIOR_ART.md` 2026-10-09): bitcoin-sv `pow.cpp` /
//! `validation.cpp` are the authority and are ported line by line; ts-stack Chaintracks
//! (`insertHeader`) for the reorg shape; Teranode `catchup_difficulty.go` for checking the rule over a
//! held window. Hashes are kept in internal (natural, serialised) byte order throughout; display hex
//! is only produced at the edges.

use std::collections::HashMap;

use crypto_bigint::{Encoding, U256};
use sha2::{Digest, Sha256};

pub const HEADER_LEN: usize = 80;
/// `validation.cpp :: MAX_FUTURE_BLOCK_TIME`.
pub const MAX_FUTURE_BLOCK_TIME: u32 = 2 * 60 * 60;
/// Blocks in the CW-144 window, and blocks a checkpoint must carry before it so the rule can be
/// checked from the first header after it (window + the two extra for the median-of-3).
pub const DAA_WINDOW: u32 = 144;
pub const DAA_ANCHOR_HEADERS: usize = DAA_WINDOW as usize + 3;
const TARGET_SPACING: u64 = 600;
const MTP_SPAN: usize = 11;
/// A header whose parent is more than this far below our tip is refused: any fork that deep would
/// need more work than our whole chain above it to win, and holding it costs memory and startup time.
pub const MAX_FORK_DEPTH: u32 = 1000;

/// A block header. `prev_hash` and `merkle_root` are in internal byte order (as serialised).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub version: u32,
    pub prev_hash: [u8; 32],
    pub merkle_root: [u8; 32],
    pub time: u32,
    pub bits: u32,
    pub nonce: u32,
}

impl Header {
    pub fn parse(bytes: &[u8]) -> Result<Header, HeaderError> {
        if bytes.len() != HEADER_LEN {
            return Err(HeaderError::Malformed(format!("a header is 80 bytes, got {}", bytes.len())));
        }
        let u32_at = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        let mut prev_hash = [0u8; 32];
        prev_hash.copy_from_slice(&bytes[4..36]);
        let mut merkle_root = [0u8; 32];
        merkle_root.copy_from_slice(&bytes[36..68]);
        Ok(Header { version: u32_at(0), prev_hash, merkle_root, time: u32_at(68), bits: u32_at(72), nonce: u32_at(76) })
    }

    pub fn serialize(&self) -> [u8; HEADER_LEN] {
        let mut b = [0u8; HEADER_LEN];
        b[0..4].copy_from_slice(&self.version.to_le_bytes());
        b[4..36].copy_from_slice(&self.prev_hash);
        b[36..68].copy_from_slice(&self.merkle_root);
        b[68..72].copy_from_slice(&self.time.to_le_bytes());
        b[72..76].copy_from_slice(&self.bits.to_le_bytes());
        b[76..80].copy_from_slice(&self.nonce.to_le_bytes());
        b
    }

    /// sha256d of the 80 bytes, internal byte order.
    pub fn hash(&self) -> [u8; 32] {
        let d = Sha256::digest(Sha256::digest(self.serialize()));
        let mut h = [0u8; 32];
        h.copy_from_slice(&d);
        h
    }
}

/// Internal byte order → the display hex block explorers show.
pub fn display_hex(hash: &[u8; 32]) -> String {
    let mut r = *hash;
    r.reverse();
    hex::encode(r)
}

/// Display hex → internal byte order.
pub fn from_display_hex(s: &str) -> Result<[u8; 32], HeaderError> {
    let v = hex::decode(s).map_err(|e| HeaderError::Malformed(format!("bad hash hex: {}", e)))?;
    if v.len() != 32 {
        return Err(HeaderError::Malformed(format!("a hash is 32 bytes, got {}", v.len())));
    }
    let mut h = [0u8; 32];
    h.copy_from_slice(&v);
    h.reverse();
    Ok(h)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderError {
    Malformed(String),
    /// The parent is not a header we hold.
    UnknownParent,
    /// The header would sit at or below the checkpoint — history we ship is never rewritten.
    BelowCheckpoint,
    /// `bits` is negative, zero, overflowing or easier than the proof-of-work limit.
    BadTarget,
    /// The hash does not meet the target its own `bits` encode.
    ProofOfWork,
    /// `bits` is not what the difficulty rule requires.
    BadDifficulty { expected: u32, got: u32 },
    /// Time not after the median of the previous 11.
    TimeTooOld,
    /// Time more than 2 h ahead of now.
    TimeTooNew,
    /// Version below what the chain requires at this height (`bad-version`).
    BadVersion(u32),
    /// The parent is more than `MAX_FORK_DEPTH` below our tip.
    ForkTooDeep,
    /// The shipped anchor headers do not link, or do not end at the checkpoint.
    BadAnchor(String),
}

impl std::fmt::Display for HeaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// `arith_uint256::SetCompact`. Returns `(target, negative, overflow)`.
pub fn decode_compact(bits: u32) -> (U256, bool, bool) {
    let size = bits >> 24;
    let mut word = bits & 0x007f_ffff;
    let target = if size <= 3 {
        word >>= 8 * (3 - size);
        U256::from_u32(word)
    } else {
        U256::from_u32(word).shl_vartime(8 * (size as usize - 3))
    };
    let negative = word != 0 && (bits & 0x0080_0000) != 0;
    let overflow = word != 0 && (size > 34 || (word > 0xff && size > 33) || (word > 0xffff && size > 32));
    (target, negative, overflow)
}

/// `arith_uint256::GetCompact` (non-negative).
pub fn encode_compact(value: &U256) -> u32 {
    let mut size = ((value.bits_vartime() + 7) / 8) as u32;
    let low64 = |v: &U256| -> u64 {
        let b = v.to_le_bytes();
        u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    };
    let mut compact: u32 = if size <= 3 {
        (low64(value) << (8 * (3 - size))) as u32
    } else {
        low64(&value.shr_vartime(8 * (size as usize - 3))) as u32
    };
    if compact & 0x0080_0000 != 0 {
        compact >>= 8;
        size += 1;
    }
    compact | (size << 24)
}

/// `pow.cpp :: CheckProofOfWork`.
pub fn check_proof_of_work(hash: &[u8; 32], bits: u32, pow_limit: &U256) -> Result<(), HeaderError> {
    let (target, negative, overflow) = decode_compact(bits);
    if negative || overflow || target == U256::ZERO || target > *pow_limit {
        return Err(HeaderError::BadTarget);
    }
    if U256::from_le_slice(hash) > target {
        return Err(HeaderError::ProofOfWork);
    }
    Ok(())
}

/// `chain.cpp :: GetBlockProof`: 2^256 / (target + 1), as (~target / (target + 1)) + 1.
pub fn work_for_bits(bits: u32) -> U256 {
    let (target, negative, overflow) = decode_compact(bits);
    if negative || overflow || target == U256::ZERO {
        return U256::ZERO;
    }
    let denom = target.wrapping_add(&U256::ONE);
    if denom == U256::ZERO {
        return U256::ONE; // target was 2^256 - 1
    }
    target.not().wrapping_div(&denom).wrapping_add(&U256::ONE)
}

/// How `bits` are checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retarget {
    /// Mainnet: `bits` must equal what CW-144 requires.
    Cw144,
    /// Regtest's `fPowNoRetargeting`: `bits` must equal the parent's (cheap synthetic test chains).
    Fixed,
    /// Tests only: any `bits` that meets proof of work (to build branches of different work).
    #[cfg(test)]
    AnyBits,
}

/// Consensus parameters for header acceptance.
#[derive(Clone, Debug)]
pub struct ChainParams {
    pub pow_limit: U256,
    pub retarget: Retarget,
    /// Lowest version accepted (`ContextualCheckBlockHeader`'s `bad-version`): 4 on mainnet above
    /// BIP65Height (388,381), far below the checkpoint.
    pub min_version: i32,
}

impl ChainParams {
    pub fn mainnet() -> ChainParams {
        ChainParams {
            pow_limit: U256::from_be_hex("00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            retarget: Retarget::Cw144,
            min_version: 4,
        }
    }

    pub fn regtest() -> ChainParams {
        ChainParams {
            pow_limit: U256::from_be_hex("7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            retarget: Retarget::Fixed,
            min_version: 0,
        }
    }
}

/// The compiled-in starting point: the checkpoint header's height, hash and absolute chain work.
#[derive(Clone, Copy, Debug)]
pub struct Checkpoint {
    pub height: u32,
    pub hash: [u8; 32],
    pub chainwork: U256,
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub header: Header,
    pub hash: [u8; 32],
    pub height: u32,
    pub chainwork: U256,
}

/// What adding one header did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddOutcome {
    /// Already held.
    Known,
    /// Extended the active chain by one.
    Extended { height: u32 },
    /// Held on a side branch with no more work than the active tip.
    Fork { height: u32 },
    /// The active chain switched to a heavier branch. `deactivated` are the `(height, hash)` of the
    /// headers that left the main chain, lowest first.
    Reorg { fork_height: u32, new_tip_height: u32, deactivated: Vec<(u32, [u8; 32])> },
}

/// The answer to "is this the merkle root of the main-chain block at this height?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootVerdict {
    Valid,
    Invalid,
    /// We do not hold a main-chain header at that height (above our tip, or below our anchor).
    /// Never a verdict: a caller must neither store the proof as verified nor discard it as bad.
    Unknown,
}

pub struct HeaderChain {
    params: ChainParams,
    checkpoint: Checkpoint,
    entries: HashMap<[u8; 32], Entry>,
    /// `active[i]` = hash of the main-chain header at height `base_height + i`.
    active: Vec<[u8; 32]>,
    base_height: u32,
}

impl HeaderChain {
    /// Build the chain from the shipped anchor headers, which end at the checkpoint. Under CW-144
    /// there must be at least `DAA_ANCHOR_HEADERS` so the rule can be checked from the first new header.
    pub fn from_anchor(params: ChainParams, checkpoint: Checkpoint, anchor: &[Header]) -> Result<HeaderChain, HeaderError> {
        if anchor.is_empty() {
            return Err(HeaderError::BadAnchor("no anchor headers".into()));
        }
        if params.retarget == Retarget::Cw144 && anchor.len() < DAA_ANCHOR_HEADERS {
            return Err(HeaderError::BadAnchor(format!("CW-144 needs {} anchor headers, got {}", DAA_ANCHOR_HEADERS, anchor.len())));
        }
        let hashes: Vec<[u8; 32]> = anchor.iter().map(|h| h.hash()).collect();
        if *hashes.last().unwrap() != checkpoint.hash {
            return Err(HeaderError::BadAnchor("the last anchor header is not the checkpoint".into()));
        }
        for i in 1..anchor.len() {
            if anchor[i].prev_hash != hashes[i - 1] {
                return Err(HeaderError::BadAnchor(format!("anchor header {} does not link to {}", i, i - 1)));
            }
        }
        for (h, hash) in anchor.iter().zip(&hashes) {
            check_proof_of_work(hash, h.bits, &params.pow_limit)
                .map_err(|e| HeaderError::BadAnchor(format!("anchor proof of work: {}", e)))?;
        }
        let n = anchor.len() as u32;
        if checkpoint.height + 1 < n {
            return Err(HeaderError::BadAnchor("more anchor headers than the checkpoint height allows".into()));
        }
        let base_height = checkpoint.height + 1 - n;
        // Chain work counted back from the checkpoint's absolute value.
        let mut entries = HashMap::with_capacity(anchor.len());
        let mut cw = checkpoint.chainwork;
        for i in (0..anchor.len()).rev() {
            entries.insert(hashes[i], Entry { header: anchor[i], hash: hashes[i], height: base_height + i as u32, chainwork: cw });
            cw = cw.wrapping_sub(&work_for_bits(anchor[i].bits));
        }
        Ok(HeaderChain { params, checkpoint, entries, active: hashes, base_height })
    }

    pub fn checkpoint(&self) -> Checkpoint {
        self.checkpoint
    }

    pub fn params(&self) -> &ChainParams {
        &self.params
    }

    pub fn tip(&self) -> &Entry {
        &self.entries[self.active.last().expect("active chain is never empty")]
    }

    pub fn tip_height(&self) -> u32 {
        self.tip().height
    }

    /// The main-chain header at `height`, if held.
    pub fn header_at(&self, height: u32) -> Option<&Entry> {
        let i = height.checked_sub(self.base_height)? as usize;
        self.active.get(i).map(|h| &self.entries[h])
    }

    pub fn get(&self, hash: &[u8; 32]) -> Option<&Entry> {
        self.entries.get(hash)
    }

    /// `root` in internal byte order (as it sits in the header's bytes 36..68).
    pub fn is_valid_root_for_height(&self, root: &[u8; 32], height: u32) -> RootVerdict {
        match self.header_at(height) {
            None => RootVerdict::Unknown,
            Some(e) if e.header.merkle_root == *root => RootVerdict::Valid,
            Some(_) => RootVerdict::Invalid,
        }
    }

    /// Median time of the 11 headers ending at `hash` (fewer if the chain is shorter), as
    /// `CBlockIndex::GetMedianTimePast`.
    pub fn median_time_past(&self, hash: &[u8; 32]) -> Option<u32> {
        let mut times = Vec::with_capacity(MTP_SPAN);
        let mut cur = self.entries.get(hash)?;
        loop {
            times.push(cur.header.time);
            if times.len() == MTP_SPAN {
                break;
            }
            match self.entries.get(&cur.header.prev_hash) {
                Some(p) => cur = p,
                None => break,
            }
        }
        times.sort_unstable();
        Some(times[times.len() / 2])
    }

    fn ancestor<'a>(&'a self, from: &'a Entry, height: u32) -> Option<&'a Entry> {
        let mut cur = from;
        while cur.height > height {
            cur = self.entries.get(&cur.header.prev_hash)?;
        }
        (cur.height == height).then_some(cur)
    }

    /// `pow.cpp :: GetSuitableBlock`: the median-by-time of a header and its two parents.
    fn suitable(&self, e: &Entry) -> Option<Entry> {
        let p1 = self.entries.get(&e.header.prev_hash)?;
        let p0 = self.entries.get(&p1.header.prev_hash)?;
        let mut b = [*p0, *p1, *e];
        if b[0].header.time > b[2].header.time {
            b.swap(0, 2);
        }
        if b[0].header.time > b[1].header.time {
            b.swap(0, 1);
        }
        if b[1].header.time > b[2].header.time {
            b.swap(1, 2);
        }
        Some(b[1])
    }

    /// The `bits` a child of `parent` must carry.
    pub fn next_bits_required(&self, parent: &Entry) -> Result<u32, HeaderError> {
        if self.params.retarget == Retarget::Fixed {
            return Ok(parent.header.bits);
        }
        let missing = || HeaderError::BadAnchor("not enough held ancestors for the CW-144 window".into());
        let last = self.suitable(parent).ok_or_else(missing)?;
        let first_tip = self.ancestor(parent, parent.height.checked_sub(DAA_WINDOW).ok_or_else(missing)?).ok_or_else(missing)?;
        let first = self.suitable(first_tip).ok_or_else(missing)?;
        // `pow.cpp :: ComputeTarget`
        let mut work = last.chainwork.wrapping_sub(&first.chainwork);
        work = work.wrapping_mul(&U256::from_u64(TARGET_SPACING));
        let mut timespan = last.header.time as i64 - first.header.time as i64;
        let (lo, hi) = (72 * TARGET_SPACING as i64, 288 * TARGET_SPACING as i64);
        if timespan > hi {
            timespan = hi;
        } else if timespan < lo {
            timespan = lo;
        }
        work = work.wrapping_div(&U256::from_u64(timespan as u64));
        if work == U256::ZERO {
            return Err(HeaderError::BadAnchor("zero work in the CW-144 window".into()));
        }
        let mut target = work.wrapping_neg().wrapping_div(&work);
        if target > self.params.pow_limit {
            target = self.params.pow_limit;
        }
        Ok(encode_compact(&target))
    }

    /// Check and add one header. `now` is Unix seconds (injected so tests control the clock).
    pub fn add_header(&mut self, header: Header, now: u32) -> Result<AddOutcome, HeaderError> {
        let hash = header.hash();
        if self.entries.contains_key(&hash) {
            return Ok(AddOutcome::Known);
        }
        let parent = *self.entries.get(&header.prev_hash).ok_or(HeaderError::UnknownParent)?;
        if parent.height < self.checkpoint.height {
            return Err(HeaderError::BelowCheckpoint);
        }
        if parent.height.saturating_add(MAX_FORK_DEPTH) < self.tip_height() {
            return Err(HeaderError::ForkTooDeep);
        }
        if (header.version as i32) < self.params.min_version {
            return Err(HeaderError::BadVersion(header.version));
        }
        check_proof_of_work(&hash, header.bits, &self.params.pow_limit)?;
        #[cfg(test)]
        let any_bits = self.params.retarget == Retarget::AnyBits;
        #[cfg(not(test))]
        let any_bits = false;
        if !any_bits {
            let expected = self.next_bits_required(&parent)?;
            if header.bits != expected {
                return Err(HeaderError::BadDifficulty { expected, got: header.bits });
            }
        }
        let mtp = self.median_time_past(&parent.hash).expect("parent is held");
        if header.time <= mtp {
            return Err(HeaderError::TimeTooOld);
        }
        if header.time > now.saturating_add(MAX_FUTURE_BLOCK_TIME) {
            return Err(HeaderError::TimeTooNew);
        }
        let entry = Entry { header, hash, height: parent.height + 1, chainwork: parent.chainwork.wrapping_add(&work_for_bits(header.bits)) };
        self.entries.insert(hash, entry);

        let tip = *self.tip();
        if parent.hash == tip.hash {
            self.active.push(hash);
            return Ok(AddOutcome::Extended { height: entry.height });
        }
        if entry.chainwork <= tip.chainwork {
            return Ok(AddOutcome::Fork { height: entry.height });
        }
        // Heavier branch: walk it back to the main chain.
        let mut branch = vec![hash];
        let mut cur = parent;
        while self.header_at(cur.height).map(|e| e.hash) != Some(cur.hash) {
            branch.push(cur.hash);
            cur = *self.entries.get(&cur.header.prev_hash).expect("a held header's parent is held");
        }
        let fork_height = cur.height;
        let keep = (fork_height - self.base_height + 1) as usize;
        let deactivated = self.active[keep..].iter().map(|h| (self.entries[h].height, *h)).collect();
        self.active.truncate(keep);
        self.active.extend(branch.into_iter().rev());
        Ok(AddOutcome::Reorg { fork_height, new_tip_height: entry.height, deactivated })
    }
}

/// The release checkpoint: block 970,000. Hash and chain work agreed by WhatsOnChain, Bitails and
/// BSVA's chaintracks on 2026-10-09; the 147 anchor headers (969,854 – 970,000) were byte-identical
/// from WhatsOnChain's header file and BSVA's Arcade chaintracks (`P7a-A18`).
pub const MAINNET_CHECKPOINT_HEIGHT: u32 = 970_000;
pub const MAINNET_CHECKPOINT_HASH: &str = "00000000000000001894e5528239b925bf06f12ff48180718dd59374c894ac8c";
pub const MAINNET_CHECKPOINT_CHAINWORK: &str = "0000000000000000000000000000000000000000016ec2d61969590ca4704b2d";
static MAINNET_ANCHOR: &[u8] = include_bytes!("checkpoints/mainnet_970000_anchor.bin");

/// Split concatenated 80-byte headers.
pub fn parse_headers(bytes: &[u8]) -> Result<Vec<Header>, HeaderError> {
    if bytes.len() % HEADER_LEN != 0 {
        return Err(HeaderError::Malformed(format!("{} bytes is not a whole number of headers", bytes.len())));
    }
    bytes.chunks(HEADER_LEN).map(Header::parse).collect()
}

pub fn mainnet_checkpoint() -> Checkpoint {
    Checkpoint {
        height: MAINNET_CHECKPOINT_HEIGHT,
        hash: from_display_hex(MAINNET_CHECKPOINT_HASH).expect("compiled-in checkpoint hash"),
        chainwork: U256::from_be_hex(MAINNET_CHECKPOINT_CHAINWORK),
    }
}

/// A fresh mainnet chain holding only the compiled-in anchor.
pub fn mainnet_header_chain() -> Result<HeaderChain, HeaderError> {
    HeaderChain::from_anchor(ChainParams::mainnet(), mainnet_checkpoint(), &parse_headers(MAINNET_ANCHOR)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WOC_968001_970000: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/headers/woc_968001_970000.bin"));
    const ARCADE_970001_970300: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/headers/arcade_970001_970300.bin"));
    /// Block 968,147 as WhatsOnChain and Bitails report it (2026-10-09).
    const H968147_HASH: &str = "00000000000000000c75e1ca4da1228320cf8426a02f0c3ed358049a04f76da5";
    const H968147_CHAINWORK: &str = "0000000000000000000000000000000000000000016e8d9e073f616a1a2f7402";
    const H970300_HASH: &str = "000000000000000010afe10ee8a63dc63cbe4dff28a9ed07c89bbaae854a6fb6";

    fn post() -> Vec<Header> {
        parse_headers(ARCADE_970001_970300).unwrap()
    }

    fn clock(h: &Header) -> u32 {
        h.time + 60
    }

    // ---------- arithmetic ----------

    #[test]
    fn compact_and_work_known_values() {
        // Bitcoin's genesis difficulty has work 0x100010001.
        assert_eq!(work_for_bits(0x1d00ffff), U256::from_u64(0x1_0001_0001));
        let (t, neg, of) = decode_compact(0x1d00ffff);
        assert!(!neg && !of);
        assert_eq!(t, U256::from_be_hex("00000000ffff0000000000000000000000000000000000000000000000000000"));
        assert_eq!(encode_compact(&t), 0x1d00ffff);
        for bits in [0x1820dc19u32, 0x182071c6, 0x1820ab78, 0x207fffff, 0x03123456] {
            assert_eq!(encode_compact(&decode_compact(bits).0), bits, "{:#x}", bits);
        }
    }

    #[test]
    fn a4_compact_target_sanity() {
        let limit = ChainParams::mainnet().pow_limit;
        let any = [0u8; 32];
        assert_eq!(check_proof_of_work(&any, 0x1d80ffff, &limit), Err(HeaderError::BadTarget), "negative");
        assert_eq!(check_proof_of_work(&any, 0x1d000000, &limit), Err(HeaderError::BadTarget), "zero");
        assert_eq!(check_proof_of_work(&any, 0xff123456, &limit), Err(HeaderError::BadTarget), "overflow");
        assert_eq!(check_proof_of_work(&any, 0x1e00ffff, &limit), Err(HeaderError::BadTarget), "easier than the limit");
        assert_eq!(check_proof_of_work(&any, 0x1d00ffff, &limit), Ok(()), "the limit itself, all-zero hash");
    }

    // ---------- the real chain ----------

    #[test]
    fn a1_real_mainnet_span_matches_the_rule_and_the_chainwork_two_sources_report() {
        // Anchor 968,001 – 968,147; then 1,853 real headers through add_header, each under CW-144.
        let all = parse_headers(WOC_968001_970000).unwrap();
        assert_eq!(all.len(), 2000);
        let cp = Checkpoint { height: 968_147, hash: from_display_hex(H968147_HASH).unwrap(), chainwork: U256::from_be_hex(H968147_CHAINWORK) };
        let mut chain = HeaderChain::from_anchor(ChainParams::mainnet(), cp, &all[..147]).unwrap();
        for (i, h) in all[147..].iter().enumerate() {
            let out = chain.add_header(*h, clock(h)).unwrap_or_else(|e| panic!("height {} refused: {}", 968_148 + i, e));
            assert_eq!(out, AddOutcome::Extended { height: 968_148 + i as u32 });
        }
        assert_eq!(chain.tip_height(), 970_000);
        assert_eq!(display_hex(&chain.tip().hash), MAINNET_CHECKPOINT_HASH);
        // Our own work sum lands exactly on the chain work WoC and Bitails report for 970,000.
        assert_eq!(chain.tip().chainwork, U256::from_be_hex(MAINNET_CHECKPOINT_CHAINWORK));
    }

    #[test]
    fn a1_production_checkpoint_extends_with_real_headers() {
        let mut chain = mainnet_header_chain().unwrap();
        assert_eq!(chain.tip_height(), MAINNET_CHECKPOINT_HEIGHT);
        for h in post() {
            assert!(matches!(chain.add_header(h, clock(&h)).unwrap(), AddOutcome::Extended { .. }));
        }
        assert_eq!(chain.tip_height(), 970_300);
        assert_eq!(display_hex(&chain.tip().hash), H970300_HASH);
    }

    #[test]
    fn a18_compiled_checkpoint_is_the_recorded_one() {
        let anchor = parse_headers(MAINNET_ANCHOR).unwrap();
        assert_eq!(anchor.len(), DAA_ANCHOR_HEADERS);
        assert_eq!(display_hex(&anchor.last().unwrap().hash()), MAINNET_CHECKPOINT_HASH);
        assert_eq!(display_hex(&anchor[0].hash()), "0000000000000000023ccb97e1dea5e12f113521824361df07b8bda315b66b5b");
    }

    // ---------- refusals ----------

    #[test]
    fn a2_hash_above_its_own_target_is_refused() {
        let mut chain = mainnet_header_chain().unwrap();
        let mut h = post()[0];
        h.nonce = h.nonce.wrapping_add(1);
        assert_eq!(chain.add_header(h, clock(&h)), Err(HeaderError::ProofOfWork));
        assert_eq!(chain.tip_height(), MAINNET_CHECKPOINT_HEIGHT);
    }

    /// FX4: a header really mined on top of checkpoint 970,000 at the EASIEST allowed difficulty
    /// (`bits 0x1d00ffff`, ~2^32 hashes: 28 s on one PC, 2026-10-09; nonce committed). It links, meets
    /// its own target and may carry any merkle root (here `F…`, a forged one) — the cheap fake chain.
    const FX4_MIN_DIFFICULTY_ON_970000: &str = "000000208cac94c87493d58d718081f42ff106bf25b9398252e5941800000000000000000900000000000000464646464646464646464646464646464646464646464646a0e2c56affff001d9ed70c07";

    #[test]
    fn a5_a_cheap_min_difficulty_header_is_refused_by_the_difficulty_rule() {
        let mut chain = mainnet_header_chain().unwrap();
        let fake = Header::parse(&hex::decode(FX4_MIN_DIFFICULTY_ON_970000).unwrap()).unwrap();
        // Preconditions: it links to our tip and its proof of work is genuine for its own bits, so
        // only the difficulty rule stands between it and the chain.
        assert_eq!(fake.prev_hash, chain.tip().hash);
        assert_eq!(check_proof_of_work(&fake.hash(), fake.bits, &ChainParams::mainnet().pow_limit), Ok(()));
        let real_bits = post()[0].bits;
        assert_eq!(chain.add_header(fake, clock(&fake)), Err(HeaderError::BadDifficulty { expected: real_bits, got: 0x1d00ffff }));
        assert_eq!(chain.is_valid_root_for_height(&fake.merkle_root, 970_001), RootVerdict::Unknown, "its forged root is never believed");
    }

    #[test]
    fn a6_broken_linkage_is_refused() {
        let mut chain = mainnet_header_chain().unwrap();
        let mut h = post()[0];
        h.prev_hash[0] ^= 1;
        assert_eq!(chain.add_header(h, clock(&h)), Err(HeaderError::UnknownParent));
    }

    #[test]
    fn a7_anchor_not_ending_at_the_checkpoint_is_refused() {
        let anchor = parse_headers(MAINNET_ANCHOR).unwrap();
        let mut cp = mainnet_checkpoint();
        cp.hash[0] ^= 1;
        assert!(matches!(HeaderChain::from_anchor(ChainParams::mainnet(), cp, &anchor), Err(HeaderError::BadAnchor(_))));
        let mut broken = anchor.clone();
        broken[10].prev_hash[0] ^= 1;
        assert!(matches!(HeaderChain::from_anchor(ChainParams::mainnet(), mainnet_checkpoint(), &broken), Err(HeaderError::BadAnchor(_))));
    }

    #[test]
    fn a8_a_fork_below_the_checkpoint_is_refused() {
        // A sibling of the checkpoint: its parent is anchor header 969,999.
        let mut chain = mainnet_header_chain().unwrap();
        let anchor = parse_headers(MAINNET_ANCHOR).unwrap();
        let mut sibling = *anchor.last().unwrap();
        sibling.nonce ^= 1;
        assert_eq!(chain.add_header(sibling, clock(&sibling)), Err(HeaderError::BelowCheckpoint));
    }

    #[test]
    fn time_too_new_is_refused() {
        let mut chain = mainnet_header_chain().unwrap();
        let h = post()[0];
        assert_eq!(chain.add_header(h, h.time - MAX_FUTURE_BLOCK_TIME - 1), Err(HeaderError::TimeTooNew));
        assert!(chain.add_header(h, h.time - MAX_FUTURE_BLOCK_TIME).is_ok(), "exactly 2 h ahead is allowed");
    }

    // ---------- verdicts ----------

    #[test]
    fn a9_root_verdict_is_three_way() {
        let mut chain = mainnet_header_chain().unwrap();
        let hs = post();
        for h in &hs[..10] {
            chain.add_header(*h, clock(h)).unwrap();
        }
        assert_eq!(chain.tip_height(), 970_010);
        let root = hs[4].merkle_root; // block 970,005
        assert_eq!(chain.is_valid_root_for_height(&root, 970_005), RootVerdict::Valid);
        assert_eq!(chain.is_valid_root_for_height(&root, 970_006), RootVerdict::Invalid, "right root, wrong height");
        let mut wrong = root;
        wrong[0] ^= 1;
        assert_eq!(chain.is_valid_root_for_height(&wrong, 970_005), RootVerdict::Invalid);
        assert_eq!(chain.is_valid_root_for_height(&root, 970_011), RootVerdict::Unknown, "above the tip");
        assert_eq!(chain.is_valid_root_for_height(&root, 900_000), RootVerdict::Unknown, "below the anchor");
    }

    // ---------- reorgs (regtest params: cheap synthetic chains) ----------

    fn mine(prev: &Entry, time: u32, tag: u8) -> Header {
        let mut h = Header { version: 0x2000_0000, prev_hash: prev.hash, merkle_root: [tag; 32], time, bits: 0x207fffff, nonce: 0 };
        let limit = ChainParams::regtest().pow_limit;
        while check_proof_of_work(&h.hash(), h.bits, &limit).is_err() {
            h.nonce += 1;
        }
        h
    }

    fn regtest_chain() -> HeaderChain {
        let mut g = Header { version: 0x2000_0000, prev_hash: [0; 32], merkle_root: [0xaa; 32], time: 1_000_000, bits: 0x207fffff, nonce: 0 };
        while check_proof_of_work(&g.hash(), g.bits, &ChainParams::regtest().pow_limit).is_err() {
            g.nonce += 1;
        }
        let cp = Checkpoint { height: 100, hash: g.hash(), chainwork: work_for_bits(g.bits) };
        HeaderChain::from_anchor(ChainParams::regtest(), cp, &[g]).unwrap()
    }

    fn extend(chain: &mut HeaderChain, from: [u8; 32], n: usize, tag: u8) -> Vec<AddOutcome> {
        let mut out = Vec::new();
        let mut cur = *chain.get(&from).unwrap();
        for i in 0..n {
            let h = mine(&cur, cur.header.time + 600, tag.wrapping_add(i as u8));
            out.push(chain.add_header(h, u32::MAX).unwrap());
            cur = *chain.get(&h.hash()).unwrap();
        }
        out
    }

    #[test]
    fn a11_heavier_branch_reorgs_and_reports_the_deactivated_headers() {
        let mut chain = regtest_chain();
        let cp = chain.checkpoint().hash;
        extend(&mut chain, cp, 3, 0x10); // branch A: 101..103
        let old: Vec<[u8; 32]> = (101..=103).map(|h| chain.header_at(h).unwrap().hash).collect();
        let root_102 = chain.header_at(102).unwrap().header.merkle_root;
        let fork_from = chain.header_at(101).unwrap().hash;
        let out = extend(&mut chain, fork_from, 3, 0x20); // branch B: 102'..104'
        assert_eq!(out[0], AddOutcome::Fork { height: 102 });
        assert_eq!(out[1], AddOutcome::Fork { height: 103 }, "equal work never displaces the tip");
        assert_eq!(out[2], AddOutcome::Reorg { fork_height: 101, new_tip_height: 104, deactivated: vec![(102, old[1]), (103, old[2])] });
        assert_eq!(chain.tip_height(), 104);
        assert_eq!(chain.header_at(101).unwrap().hash, old[0], "the common part stays");
        assert_eq!(chain.is_valid_root_for_height(&root_102, 102), RootVerdict::Invalid, "the dropped block's root no longer verifies");
    }

    #[test]
    fn a12_a_lighter_branch_never_displaces_the_tip() {
        let mut chain = regtest_chain();
        let cp = chain.checkpoint().hash;
        extend(&mut chain, cp, 5, 0x10);
        let tip = chain.tip().hash;
        let from = chain.header_at(102).unwrap().hash;
        let out = extend(&mut chain, from, 2, 0x30);
        assert!(out.iter().all(|o| matches!(o, AddOutcome::Fork { .. })));
        assert_eq!(chain.tip().hash, tip);
    }

    #[test]
    fn a5b_regtest_rule_refuses_other_bits() {
        let mut chain = regtest_chain();
        let cp = *chain.get(&chain.checkpoint().hash).unwrap();
        let mut h = mine(&cp, cp.header.time + 600, 1);
        h.bits = 0x207ffffe;
        while check_proof_of_work(&h.hash(), h.bits, &ChainParams::regtest().pow_limit).is_err() {
            h.nonce += 1;
        }
        assert!(matches!(chain.add_header(h, u32::MAX), Err(HeaderError::BadDifficulty { .. })));
    }

    #[test]
    fn time_too_old_is_refused() {
        let mut chain = regtest_chain();
        let cp = *chain.get(&chain.checkpoint().hash).unwrap();
        let h = mine(&cp, cp.header.time, 1); // not after the median (the anchor's own time)
        assert_eq!(chain.add_header(h, u32::MAX), Err(HeaderError::TimeTooOld));
    }

    // ---------- adversarial review 2026-10-09 ----------

    fn mine_bits(prev: [u8; 32], time: u32, bits: u32, tag: u8, version: u32) -> Header {
        let mut h = Header { version, prev_hash: prev, merkle_root: [tag; 32], time, bits, nonce: 0 };
        let limit = ChainParams::regtest().pow_limit;
        while check_proof_of_work(&h.hash(), h.bits, &limit).is_err() {
            h.nonce += 1;
        }
        h
    }

    #[test]
    fn version_below_4_is_refused_on_mainnet_params() {
        // A header otherwise valid under the real 970,000 window: only its version differs.
        let mut chain = mainnet_header_chain().unwrap();
        let real = post()[0];
        for v in [1u32, 3, 0x8000_0000] {
            let mut h = real;
            h.version = v;
            // The hash changes with the version, so only BadVersion can come before PoW.
            assert_eq!(chain.add_header(h, clock(&h)), Err(HeaderError::BadVersion(v)), "version {:#x}", v);
        }
        assert!(matches!(chain.add_header(real, clock(&real)), Ok(AddOutcome::Extended { .. })));
    }

    #[test]
    fn a_fork_deeper_than_max_fork_depth_is_refused() {
        let mut chain = regtest_chain();
        let cp = chain.checkpoint().hash;
        extend(&mut chain, cp, MAX_FORK_DEPTH as usize + 2, 0x10);
        let deep = *chain.header_at(101).unwrap();
        let h = mine(&deep, deep.header.time + 1, 0x99);
        assert_eq!(chain.add_header(h, u32::MAX), Err(HeaderError::ForkTooDeep));
        let shallow = *chain.header_at(chain.tip_height() - 5).unwrap();
        let ok = mine(&shallow, shallow.header.time + 1, 0x98);
        assert!(matches!(chain.add_header(ok, u32::MAX), Ok(AddOutcome::Fork { .. })), "a shallow fork is still held");
    }

    /// CW-144 timespan clamps (72x / 288x the spacing), which no real fixture window reaches
    /// (73,881 - 104,295 s). Expected bits computed outside the module from bitcoin-sv
    /// `ComputeTarget` (Python, 2026-10-09): 1,300 s spacing -> `0x2001ffff` (unclamped would be
    /// `0x20022b63`); 200 s -> `0x1f7fffff` (unclamped `0x1f555555`); 600 s -> unchanged.
    fn cw144_window(spacing: u32) -> HeaderChain {
        let params = ChainParams { pow_limit: ChainParams::regtest().pow_limit, retarget: Retarget::Cw144, min_version: 0 };
        let mut anchor = Vec::new();
        let mut prev = [0u8; 32];
        for i in 0..DAA_ANCHOR_HEADERS as u32 {
            let h = mine_bits(prev, 1_000_000 + i * spacing, 0x2000ffff, i as u8, 0x2000_0000);
            prev = h.hash();
            anchor.push(h);
        }
        let work: U256 = (0..anchor.len()).fold(U256::ZERO, |a, _| a.wrapping_add(&work_for_bits(0x2000ffff)));
        let cp = Checkpoint { height: 10_000, hash: prev, chainwork: work };
        HeaderChain::from_anchor(params, cp, &anchor).unwrap()
    }

    #[test]
    fn cw144_timespan_clamps_match_the_node() {
        for (spacing, expected, unclamped) in [(1300u32, 0x2001ffffu32, 0x20022b63u32), (200, 0x1f7fffff, 0x1f555555), (600, 0x2000ffff, 0x2000ffff)] {
            let mut chain = cw144_window(spacing);
            let tip = *chain.tip();
            assert_eq!(chain.next_bits_required(&tip).unwrap(), expected, "spacing {}", spacing);
            if unclamped != expected {
                let wrong = mine_bits(tip.hash, tip.header.time + spacing, unclamped, 0xee, 0x2000_0000);
                assert_eq!(chain.add_header(wrong, u32::MAX), Err(HeaderError::BadDifficulty { expected, got: unclamped }));
            }
        }
    }

    #[test]
    fn a12_a_longer_branch_with_less_work_is_ignored() {
        let params = ChainParams { pow_limit: ChainParams::regtest().pow_limit, retarget: Retarget::AnyBits, min_version: 0 };
        let g = mine_bits([0; 32], 1_000_000, 0x207fffff, 0xaa, 0x2000_0000);
        let cp = Checkpoint { height: 100, hash: g.hash(), chainwork: work_for_bits(g.bits) };
        let mut chain = HeaderChain::from_anchor(params, cp, &[g]).unwrap();
        // Main: 2 harder headers. Fork: 4 easy ones from the checkpoint.
        let m1 = mine_bits(g.hash(), 1_000_600, 0x2000ffff, 1, 0x2000_0000);
        let m2 = mine_bits(m1.hash(), 1_001_200, 0x2000ffff, 2, 0x2000_0000);
        chain.add_header(m1, u32::MAX).unwrap();
        chain.add_header(m2, u32::MAX).unwrap();
        let tip = chain.tip().hash;
        let mut prev = g.hash();
        let mut fork = Vec::new();
        for i in 0..4u32 {
            let h = mine_bits(prev, 1_000_600 + i * 600, 0x207fffff, 0x50 + i as u8, 0x2000_0000);
            prev = h.hash();
            fork.push(h);
        }
        // Independently: 4 easy blocks carry less work than 2 hard ones.
        let main_work = work_for_bits(0x2000ffff).wrapping_add(&work_for_bits(0x2000ffff));
        let fork_work = (0..4).fold(U256::ZERO, |a, _| a.wrapping_add(&work_for_bits(0x207fffff)));
        assert!(fork_work < main_work);
        for h in fork {
            assert!(matches!(chain.add_header(h, u32::MAX), Ok(AddOutcome::Fork { .. })));
        }
        assert_eq!(chain.tip().hash, tip, "a longer but lighter branch never displaces the tip");
        assert_eq!(chain.tip_height(), 102);
    }

    #[test]
    fn median_time_past_is_the_median_of_the_last_11() {
        let mut chain = regtest_chain();
        let mut prev = *chain.get(&chain.checkpoint().hash).unwrap();
        // Out of order, so MTP-11 differs from the tip's time and from a median of 10.
        let offsets = [1000u32, 2000, 1500, 3000, 2500, 4000, 3500, 5000, 4500, 6000, 5500, 7000];
        let mut times = vec![prev.header.time];
        for (i, o) in offsets.iter().enumerate() {
            let h = mine(&prev, 1_000_000 + o, i as u8);
            chain.add_header(h, u32::MAX).unwrap();
            prev = *chain.get(&h.hash()).unwrap();
            times.push(h.time);
        }
        let mut last11 = times[times.len() - 11..].to_vec();
        last11.sort_unstable();
        let expected = last11[5];
        let tip = chain.tip().hash;
        assert_eq!(chain.median_time_past(&tip), Some(expected));
        assert!(expected < chain.tip().header.time);
        let mut last10 = times[times.len() - 10..].to_vec();
        last10.sort_unstable();
        assert_ne!(last10[5], expected, "the fixture tells 11 from 10");
    }
}
