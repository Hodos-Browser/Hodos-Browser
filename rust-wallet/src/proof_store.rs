//! B5-T1-P7a-2: the one door into `proven_txs`.
//!
//! Standing invariant: *no proof is stored as verified unless it matches a header the wallet
//! verified itself.* A proof is checked against the wallet's own header chain (`ChainTracker`, behind
//! [`RootOracle`]) and becomes a [`VerifiedProof`] only on a **Valid** verdict; [`store`] takes nothing
//! else, and gate G16 keeps every other `proven_txs` writer out of the crate.
//!
//! The three answers mean different things, and callers must keep them apart:
//! - **Valid** — store it, embed it, promote the transaction to `completed`.
//! - **Invalid** (`ProofCheck::Wrong`) — do not store, embed or promote. Never a reason to fail the
//!   transaction, delete a row or stop retrying: our chain can be stale (startup, a reorg in flight).
//! - **Unknown** (`ProofCheck::NotHeld`) — our chain holds no header at that height (above the tip,
//!   below the checkpoint). Not stored as verified, kept for a later pass; still embedded when spending
//!   (owner 2026-10-13, K-o), because a block older than the checkpoint is not wrong, only unchecked.

use rusqlite::Connection;
use serde_json::Value;

use crate::cache_errors::CacheResult;
use crate::database::ProvenTxRepository;
use crate::header_chain::RootVerdict;

/// What a proof is checked against: the wallet's own verified header chain.
pub trait RootOracle: Send + Sync {
    /// Is `root` (internal byte order, as in header bytes 36..68) the merkle root of the main-chain
    /// block at `height`?
    fn root_verdict(&self, root: &[u8; 32], height: u32) -> RootVerdict;
    /// The main-chain height of the block whose display-hex hash is `block_hash`, if we hold it.
    fn height_for_hash(&self, block_hash: &str) -> Option<u32>;
    /// Whether our main chain holds a header at `height`.
    fn holds_height(&self, height: u32) -> bool;
}

/// A proof that matched a header the wallet verified itself. Only [`check_tsc`] makes one.
#[derive(Debug, Clone)]
pub struct VerifiedProof {
    txid: String,
    height: u32,
    tx_index: u64,
    /// The TSC JSON, with `height` filled in (the `proven_txs.merkle_path` format).
    tsc: Value,
}

impl VerifiedProof {
    pub fn txid(&self) -> &str {
        &self.txid
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn tx_index(&self) -> u64 {
        self.tx_index
    }
    pub fn tsc(&self) -> &Value {
        &self.tsc
    }
}

/// The result of checking one proof for one transaction.
#[derive(Debug)]
pub enum ProofCheck {
    Verified(VerifiedProof),
    /// Our chain holds no main-chain header for the proof's block. Keep it, retry later.
    NotHeld,
    /// The proof does not match the header we hold at its height.
    Wrong { height: u32 },
    /// The proof cannot be read for this transaction (no nodes, no index, no height or block hash).
    Malformed(String),
}

impl ProofCheck {
    pub fn verdict(&self) -> Option<RootVerdict> {
        match self {
            ProofCheck::Verified(_) => Some(RootVerdict::Valid),
            ProofCheck::Wrong { .. } => Some(RootVerdict::Invalid),
            ProofCheck::NotHeld => Some(RootVerdict::Unknown),
            ProofCheck::Malformed(_) => None,
        }
    }
}

/// Check a TSC proof (`{index, nodes, height?, target?}`) for `txid` against our own chain.
///
/// The height is the proof's own `height` when it has one; otherwise its `target` block hash must be
/// a block on our main chain. The root is computed from `txid` itself, so a proof for another
/// transaction cannot match.
pub fn check_tsc(oracle: &dyn RootOracle, txid: &str, tsc: &Value) -> ProofCheck {
    let Some(tx_index) = tsc["index"].as_u64() else {
        return ProofCheck::Malformed("no index".to_string());
    };
    let Some(nodes) = tsc["nodes"].as_array() else {
        return ProofCheck::Malformed("no nodes".to_string());
    };
    // The index names a position in a tree as deep as the proof: extra high bits do not change the
    // root, and would be stored as a false position (adversarial review).
    // No block holds 2^64 transactions; a longer list is not a path (and would truncate as a u8
    // tree height further down — xhigh 168 #7).
    if nodes.len() >= 64 {
        return ProofCheck::Malformed(format!("a {}-level proof", nodes.len()));
    }
    if tx_index >> nodes.len() != 0 {
        return ProofCheck::Malformed(format!("index {} is beyond a {}-level proof", tx_index, nodes.len()));
    }
    let height = match tsc["height"].as_u64().filter(|h| *h > 0) {
        Some(h) => match u32::try_from(h) {
            Ok(h) => h,
            Err(_) => return ProofCheck::Malformed(format!("height {} out of range", h)),
        },
        None => match tsc["target"].as_str().filter(|t| !t.is_empty()) {
            Some(target) => match oracle.height_for_hash(target) {
                Some(h) => h,
                None => return ProofCheck::NotHeld,
            },
            None => return ProofCheck::Malformed("no height or block hash".to_string()),
        },
    };
    let root_display = match crate::beef::compute_merkle_root_from_tsc(txid, height, tx_index, nodes) {
        Ok(r) => r,
        Err(e) => return ProofCheck::Malformed(e),
    };
    let mut root = [0u8; 32];
    match hex::decode(&root_display) {
        Ok(b) if b.len() == 32 => root.copy_from_slice(&b),
        _ => return ProofCheck::Malformed(format!("root '{}' is not 32 bytes", root_display)),
    }
    root.reverse(); // display → internal (header) order

    match oracle.root_verdict(&root, height) {
        RootVerdict::Valid => {
            let mut tsc = tsc.clone();
            tsc["height"] = serde_json::json!(height);
            log::info!("   🔐 proof for {} verified against our header at height {}", txid, height);
            ProofCheck::Verified(VerifiedProof { txid: txid.to_string(), height, tx_index, tsc })
        }
        RootVerdict::Invalid => {
            log::warn!("   ❌ proof for {} does not match our header at height {}", txid, height);
            ProofCheck::Wrong { height }
        }
        RootVerdict::Unknown => {
            log::info!("   ⏳ proof for {}: our chain holds no header at height {} yet", txid, height);
            ProofCheck::NotHeld
        }
    }
}

/// Check a BUMP (BRC-74 hex, as ARC returns it) for `txid`.
pub fn check_bump_hex(oracle: &dyn RootOracle, txid: &str, bump_hex: &str) -> ProofCheck {
    match crate::beef::parse_bump_hex(bump_hex) {
        Ok(bump) => check_bump(oracle, txid, &bump),
        Err(e) => ProofCheck::Malformed(e),
    }
}

/// Check a BUMP for `txid` along `txid`'s own path (a BUMP can hold several transactions).
pub fn check_bump(oracle: &dyn RootOracle, txid: &str, bump: &crate::beef::MerkleProof) -> ProofCheck {
    let mut leaf = [0u8; 32];
    match hex::decode(txid) {
        Ok(b) if b.len() == 32 => leaf.copy_from_slice(&b),
        _ => return ProofCheck::Malformed(format!("txid '{}' is not 32 bytes of hex", txid)),
    }
    leaf.reverse(); // display → internal
    match crate::beef::bump_path_tsc(bump, &leaf) {
        Ok(tsc) => check_tsc(oracle, txid, &tsc),
        Err(e) => ProofCheck::Malformed(e),
    }
}

/// A `proven_txs` row whose proof our chain says is Valid. Only this module makes one, so holding one
/// is the evidence a transaction may be promoted to `completed` (`recover_transaction` takes it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedRow {
    proven_tx_id: i64,
    height: u32,
}

impl VerifiedRow {
    pub fn proven_tx_id(&self) -> i64 {
        self.proven_tx_id
    }
    pub fn height(&self) -> u32 {
        self.height
    }
}

/// The one writer of `proven_txs`. Stores `proof` and returns its row. An existing row for the txid
/// is kept when our chain says it is Valid (rows are immutable); one that is Invalid or unchecked is
/// replaced, so an earlier unverified write cannot block the verified one.
pub fn store(
    conn: &Connection,
    oracle: &dyn RootOracle,
    proof: &VerifiedProof,
    raw_tx: &[u8],
    block_hash: &str,
) -> CacheResult<VerifiedRow> {
    let repo = ProvenTxRepository::new(conn);
    if let Some(row) = stored_verified_proof(conn, oracle, &proof.txid)? {
        return Ok(row);
    }
    let merkle_path = serde_json::to_vec(&proof.tsc)?;
    let id = if repo.get_by_txid(&proof.txid)?.is_some() {
        log::warn!("   🔁 replacing an unverified stored proof for {}", proof.txid);
        repo.replace_with_verified(proof, &merkle_path, raw_tx, block_hash)?
    } else {
        repo.insert_verified(proof, &merkle_path, raw_tx, block_hash)?
    };
    Ok(VerifiedRow { proven_tx_id: id, height: proof.height })
}

/// Restore (B5-T1-P7a-2 `C11`): fill a restored row's stripped proof with `proof`, only where the row
/// is empty and names the same block height and position. Returns whether a row was filled.
pub fn fill_restored_proof(conn: &Connection, proof: &VerifiedProof) -> CacheResult<bool> {
    let merkle_path = serde_json::to_vec(&proof.tsc)?;
    Ok(ProvenTxRepository::new(conn).fill_empty_merkle_path(proof, &merkle_path)? > 0)
}

/// Restore: fill a restored row's stripped raw transaction — only with bytes that hash to `txid`.
pub fn fill_restored_raw_tx(conn: &Connection, txid: &str, raw_tx: &[u8]) -> CacheResult<bool> {
    if !crate::reconcile::verify_raw_txid(raw_tx, txid) {
        return Err(crate::cache_errors::CacheError::InvalidData(format!("raw tx does not hash to {}", txid)));
    }
    Ok(ProvenTxRepository::new(conn).fill_empty_raw_tx(txid, raw_tx)? > 0)
}

/// Link `txid`'s transaction (and its `proven_tx_reqs` row, if any) to a verified row.
pub fn link(conn: &Connection, row: &VerifiedRow, txid: &str) -> CacheResult<()> {
    ProvenTxRepository::new(conn).link_transaction(txid, row.proven_tx_id)?;
    let req_repo = crate::database::ProvenTxReqRepository::new(conn);
    if let Some(req) = req_repo.get_by_txid(txid)? {
        req_repo.update_status(req.proven_tx_req_id, "completed")?;
        req_repo.link_proven_tx(req.proven_tx_req_id, row.proven_tx_id)?;
    }
    Ok(())
}

/// The stored proof for `txid`, re-checked now (K-q: rows can come from an import, a restored file or
/// an older build). `Some` only when our chain says it is Valid — the bar for promoting a status.
pub fn stored_verified_proof(conn: &Connection, oracle: &dyn RootOracle, txid: &str) -> CacheResult<Option<VerifiedRow>> {
    let repo = ProvenTxRepository::new(conn);
    let Some(row) = repo.get_by_txid(txid)? else { return Ok(None) };
    let Some(tsc) = readable_tsc(&repo, txid)? else { return Ok(None) };
    Ok(match check_tsc(oracle, txid, &tsc) {
        ProofCheck::Verified(p) => Some(VerifiedRow { proven_tx_id: row.proven_tx_id, height: p.height }),
        _ => None,
    })
}

/// The stored proof for `txid` as a TSC, for embedding in a BEEF: anything except a proof our chain
/// shows is wrong (K-o — an unchecked proof for an older block is still the best evidence we have).
pub fn stored_proof_for_beef(conn: &Connection, oracle: &dyn RootOracle, txid: &str) -> CacheResult<Option<Value>> {
    let Some(tsc) = readable_tsc(&ProvenTxRepository::new(conn), txid)? else { return Ok(None) };
    Ok(match check_tsc(oracle, txid, &tsc) {
        // The checked copy: it carries the height our chain resolved from the block hash (older rows
        // were stored with height 0 — `/code-review xhigh 167` #4).
        ProofCheck::Verified(p) => Some(p.tsc().clone()),
        // Unchecked, embedded only at a height the row really has; with none, the caller fetches a
        // fresh proof (and its height hint) instead of embedding at block 0.
        ProofCheck::NotHeld => tsc["height"].as_u64().filter(|h| *h > 0).map(|_| tsc),
        // Wrong, or not a proof at all (it could not be put in a BEEF either).
        ProofCheck::Wrong { .. } | ProofCheck::Malformed(_) => None,
    })
}

/// The stored TSC for `txid`; a row whose bytes are not a TSC reads as no proof (it is replaced by
/// the next verified store), only a database failure is an error.
fn readable_tsc(repo: &ProvenTxRepository, txid: &str) -> CacheResult<Option<Value>> {
    match repo.get_merkle_proof_as_tsc(txid) {
        Ok(v) => Ok(v),
        Err(crate::cache_errors::CacheError::Database(e)) => Err(crate::cache_errors::CacheError::Database(e)),
        Err(e) => {
            log::warn!("   ⚠️  stored proof for {} is unreadable ({}) — treated as no proof", txid, e);
            Ok(None)
        }
    }
}
