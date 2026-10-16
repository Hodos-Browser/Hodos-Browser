//! B5-T1-P7a-1: BRC-67's transaction-level checks on a transaction someone else built and handed us
//! (`internalizeAction`, PeerPay). Pure: no network, no database.
//!
//! - **Check 2 — fee:** the outputs the subject spends (taken from the parent transactions in its
//!   BEEF, found by their real txid) are worth **more than** its outputs (BRC-67's `>`, owner
//!   decision 3, 2026-10-09; `@bsv/sdk Transaction.verify` accepts `>=`). Values are range-checked
//!   like consensus `MoneyRange`, sums are overflow-checked, an outpoint spent twice is refused.
//! - **Check 4 — finality:** bitcoin-sv `validation.cpp :: IsFinalTx`: final when nLockTime is 0, or
//!   every nSequence is `0xFFFFFFFF`, or the lock is already past — a height lock against the next
//!   block (tip + 1), a time lock against the tip's median time past, both with a strict `<`.
//!
//! Neither check runs inside script; both are rules the node applies outside the script engine.

use std::collections::HashSet;

use crate::beef::{Beef, ParsedTransaction};
use crate::header_chain::RootVerdict;

/// `amount.h :: MAX_MONEY`.
pub const MAX_MONEY: i64 = 21_000_000 * 100_000_000;
/// `script.h :: LOCKTIME_THRESHOLD`: below it nLockTime is a block height, at or above it a Unix time.
pub const LOCKTIME_THRESHOLD: u32 = 500_000_000;
pub const SEQUENCE_FINAL: u32 = 0xFFFF_FFFF;
/// How many blocks our verified tip may trail the sender's before a height lock at its edge is
/// judged final or not.
pub const TIP_LAG_BLOCKS: u64 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// An input's parent is not in the BEEF (by its real txid), or has no such output.
    MissingParent { input: usize, prev_txid: String, prev_vout: u32 },
    /// An output value outside `0..=MAX_MONEY`, or a total above it.
    BadValue(String),
    /// The same outpoint is spent twice.
    DuplicateInput { input: usize },
    /// Inputs are not worth more than outputs.
    Fee { inputs: i64, outputs: i64 },
    /// The transaction can still be replaced: its lock has not passed.
    NonFinal { lock_time: u32 },
    /// The transaction is non-final in form and our headers are not synced yet, so whether its lock
    /// has passed cannot be told. Retryable — never a verdict.
    HeadersUnavailable { lock_time: u32 },
    /// A BUMP in the BEEF does not compute to the merkle root of our verified header at its height,
    /// or cannot be computed at all.
    BadProof { tx: usize, height: u32, reason: String },
}

impl Refusal {
    /// The wallet error code a route returns for this refusal.
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::MissingParent { .. } => "ERR_MISSING_PARENT",
            Refusal::BadValue(_) | Refusal::DuplicateInput { .. } | Refusal::Fee { .. } => "ERR_FEE",
            Refusal::NonFinal { .. } => "ERR_NONFINAL",
            Refusal::HeadersUnavailable { .. } => "ERR_HEADERS_UNAVAILABLE",
            Refusal::BadProof { .. } => "ERR_BAD_PROOF",
        }
    }

    pub fn description(&self) -> String {
        match self {
            Refusal::MissingParent { input, prev_txid, prev_vout } => {
                format!("Input {} spends {}:{}, which is not in the BEEF", input, prev_txid, prev_vout)
            }
            Refusal::BadValue(s) => s.clone(),
            Refusal::DuplicateInput { input } => format!("Input {} spends an outpoint another input already spends", input),
            Refusal::Fee { inputs, outputs } => {
                format!("Inputs are worth {} sats and outputs {} sats; the inputs must be worth more (BRC-67)", inputs, outputs)
            }
            Refusal::NonFinal { lock_time } => {
                format!("The transaction is not final (nLockTime {} has not passed and an input's sequence is not final); it can still be replaced", lock_time)
            }
            Refusal::HeadersUnavailable { lock_time } => {
                format!("The transaction is locked until {} and the wallet's block headers are not synced yet; try again shortly", lock_time)
            }
            Refusal::BadProof { tx, height, reason } => {
                format!("The merkle proof for transaction {} in the BEEF (block {}) does not match the wallet's verified block header: {}", tx, height, reason)
            }
        }
    }
}

fn money(v: i64, what: &str) -> Result<i64, Refusal> {
    if (0..=MAX_MONEY).contains(&v) {
        Ok(v)
    } else {
        Err(Refusal::BadValue(format!("{} has value {} (outside 0..={})", what, v, MAX_MONEY)))
    }
}

fn add(total: i64, v: i64, what: &str) -> Result<i64, Refusal> {
    total.checked_add(v).filter(|t| *t <= MAX_MONEY).ok_or_else(|| Refusal::BadValue(format!("{} total exceeds {}", what, MAX_MONEY)))
}

/// BRC-67 check 2.
pub fn check_fee(tx: &ParsedTransaction, beef: &Beef) -> Result<(), Refusal> {
    // Each bundle transaction hashed once, by its real txid (first wins, as `Beef::find_txid`):
    // looking a parent up per input by re-hashing the bundle was quadratic (`/code-review max 159` F4).
    let mut by_txid: std::collections::HashMap<String, usize> = std::collections::HashMap::with_capacity(beef.transactions.len());
    for (i, raw) in beef.transactions.iter().enumerate() {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::digest(Sha256::digest(raw)).to_vec();
        h.reverse();
        by_txid.entry(hex::encode(h)).or_insert(i);
    }
    let mut parsed: std::collections::HashMap<usize, ParsedTransaction> = std::collections::HashMap::new();
    let mut seen = HashSet::new();
    let mut inputs = 0i64;
    for (i, input) in tx.inputs.iter().enumerate() {
        if !seen.insert((input.prev_txid.as_str(), input.prev_vout)) {
            return Err(Refusal::DuplicateInput { input: i });
        }
        let missing = || Refusal::MissingParent { input: i, prev_txid: input.prev_txid.clone(), prev_vout: input.prev_vout };
        let idx = *by_txid.get(&input.prev_txid.to_lowercase()).ok_or_else(missing)?;
        // Each parent parsed once, however many inputs spend it (`/code-review max 159` V2).
        if !parsed.contains_key(&idx) {
            parsed.insert(idx, ParsedTransaction::from_bytes(&beef.transactions[idx]).map_err(|_| missing())?);
        }
        let out = parsed[&idx].outputs.get(input.prev_vout as usize).ok_or_else(missing)?;
        inputs = add(inputs, money(out.value, &format!("Input {}'s parent output", i))?, "Input")?;
    }
    let mut outputs = 0i64;
    for (o, out) in tx.outputs.iter().enumerate() {
        outputs = add(outputs, money(out.value, &format!("Output {}", o))?, "Output")?;
    }
    if inputs > outputs {
        Ok(())
    } else {
        Err(Refusal::Fee { inputs, outputs })
    }
}

/// Our view of the chain for check 4.
#[derive(Debug, Clone, Copy)]
pub struct ChainView {
    pub tip_height: u32,
    pub tip_median_time_past: u32,
    /// `false` until a header sync has reached a source's tip; a stale tip cannot judge a lock.
    pub synced: bool,
}

/// BRC-67 check 4 (`IsFinalTx` at the next block).
/// `IsFinalTx`'s lock comparison: a height lock against tip + 1, a time lock (≥ `LOCKTIME_THRESHOLD`)
/// against the median time past, both strict. Shared by `check_final` and the PeerPay held-row gate
/// (B5-T1-P7b: two copies could drift and strand a lock-held payment).
pub fn lock_passed(lock_time: u32, tip_height: u32, tip_median_time_past: u32) -> bool {
    let limit = if lock_time < LOCKTIME_THRESHOLD { tip_height as u64 + 1 } else { tip_median_time_past as u64 };
    (lock_time as u64) < limit
}

pub fn check_final(tx: &ParsedTransaction, chain: ChainView) -> Result<(), Refusal> {
    let lock_time = tx.lock_time;
    if lock_time == 0 || tx.inputs.iter().all(|i| i.sequence == SEQUENCE_FINAL) {
        return Ok(());
    }
    if !chain.synced {
        return Err(Refusal::HeadersUnavailable { lock_time });
    }
    if lock_passed(lock_time, chain.tip_height, chain.tip_median_time_past) {
        Ok(())
    } else if lock_time < LOCKTIME_THRESHOLD && (lock_time as u64) <= chain.tip_height as u64 + TIP_LAG_BLOCKS {
        // Our tip may be a block or two behind the sender's (a header lags its block by seconds):
        // a lock at the edge is "try again shortly", not "replaceable" (adversarial review).
        Err(Refusal::HeadersUnavailable { lock_time })
    } else {
        Err(Refusal::NonFinal { lock_time })
    }
}

/// What the proofs in a BEEF said.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BeefProofs {
    /// The subject's own BUMP matched our verified header: it is mined, so consensus already
    /// enforced the fee and finality rules for it.
    pub subject_valid: bool,
    /// BUMPs that matched our headers, as `(tx index, height)`.
    pub valid: Vec<(usize, u32)>,
    /// BUMPs for blocks we do not hold yet (above our tip or below our checkpoint): neither believed
    /// nor refused (owner decision 4) — nothing from them is stored as verified.
    pub unknown: Vec<(usize, u32)>,
}

/// The `BadProof` reason for a well-formed BUMP whose root is not our header's — as opposed to a BUMP
/// that cannot be computed at all (B5-T1-P7b: the first may be a block about to win, the second never
/// becomes valid).
pub const ROOT_MISMATCH: &str = "the computed merkle root is not that block's";

/// Check every BUMP a transaction in `beef` points at against our verified headers. One that
/// computes to the wrong root — or cannot be computed — refuses the whole BEEF.
pub fn check_beef_proofs(beef: &Beef, subject_idx: usize, verdict: impl Fn(&[u8; 32], u32) -> RootVerdict) -> Result<BeefProofs, Refusal> {
    let mut out = BeefProofs::default();
    for (i, bump_idx) in beef.tx_to_bump.iter().enumerate() {
        let Some(b) = *bump_idx else { continue };
        let bump = beef.bumps.get(b).ok_or_else(|| Refusal::BadProof { tx: i, height: 0, reason: format!("it points at BUMP {} and the BEEF has {}", b, beef.bumps.len()) })?;
        let raw = beef.transactions.get(i).ok_or_else(|| Refusal::BadProof { tx: i, height: bump.block_height, reason: "no such transaction".into() })?;
        let root = crate::beef::bump_root_for_tx(bump, raw).map_err(|reason| Refusal::BadProof { tx: i, height: bump.block_height, reason })?;
        match verdict(&root, bump.block_height) {
            RootVerdict::Valid => {
                out.valid.push((i, bump.block_height));
                if i == subject_idx {
                    out.subject_valid = true;
                }
            }
            RootVerdict::Unknown => out.unknown.push((i, bump.block_height)),
            RootVerdict::Invalid => {
                return Err(Refusal::BadProof { tx: i, height: bump.block_height, reason: ROOT_MISMATCH.into() })
            }
        }
    }
    Ok(out)
}

/// Rows `P7a-B1`…`B15` at the function. Route rows (the same refusals through `internalizeAction`
/// and PeerPay, nothing broadcast or stored) are in `handlers_p7a_tests.rs` / `task_check_peerpay.rs`.
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    /// Every field explicit (the old route fixtures defaulted to "final, no parents needed").
    pub(crate) struct TxIn {
        pub prev_txid: String,
        pub vout: u32,
        pub sequence: u32,
    }

    pub(crate) fn raw_tx(inputs: &[TxIn], output_values: &[u64], lock_time: u32) -> Vec<u8> {
        let mut b = 1u32.to_le_bytes().to_vec();
        b.push(inputs.len() as u8);
        for i in inputs {
            let mut t = hex::decode(&i.prev_txid).unwrap();
            t.reverse();
            b.extend_from_slice(&t);
            b.extend_from_slice(&i.vout.to_le_bytes());
            b.push(0);
            b.extend_from_slice(&i.sequence.to_le_bytes());
        }
        b.push(output_values.len() as u8);
        for v in output_values {
            b.extend_from_slice(&v.to_le_bytes()); // raw u64 bytes: a "negative" i64 is writable
            b.push(1);
            b.push(0x51); // OP_TRUE
        }
        b.extend_from_slice(&lock_time.to_le_bytes());
        b
    }

    pub(crate) fn txid(raw: &[u8]) -> String {
        let mut h = Sha256::digest(Sha256::digest(raw)).to_vec();
        h.reverse();
        hex::encode(h)
    }

    pub(crate) fn beef(txs: Vec<Vec<u8>>) -> Beef {
        let n = txs.len();
        Beef { version: crate::beef::BEEF_V1_MARKER, bumps: vec![], transactions: txs, tx_to_bump: vec![None; n] }
    }

    fn input(prev: &[u8], vout: u32) -> TxIn {
        TxIn { prev_txid: txid(prev), vout, sequence: SEQUENCE_FINAL }
    }

    fn parent(values: &[u64]) -> Vec<u8> {
        raw_tx(&[TxIn { prev_txid: "77".repeat(32), vout: 0, sequence: SEQUENCE_FINAL }], values, 0)
    }

    fn subject_of(raw: &[u8]) -> ParsedTransaction {
        ParsedTransaction::from_bytes(raw).unwrap()
    }

    // ---------- check 2 ----------

    #[test]
    fn b1_spending_1000_into_1001_is_refused() {
        let p = parent(&[1000]);
        let s = raw_tx(&[input(&p, 0)], &[1001], 0);
        assert_eq!(check_fee(&subject_of(&s), &beef(vec![p, s.clone()])), Err(Refusal::Fee { inputs: 1000, outputs: 1001 }));
    }

    #[test]
    fn b2_zero_fee_is_refused_brc67_strict() {
        let p = parent(&[1000]);
        let s = raw_tx(&[input(&p, 0)], &[1000], 0);
        assert_eq!(check_fee(&subject_of(&s), &beef(vec![p, s.clone()])), Err(Refusal::Fee { inputs: 1000, outputs: 1000 }));
    }

    #[test]
    fn b3_a_one_sat_fee_is_accepted() {
        let p = parent(&[1001]);
        let s = raw_tx(&[input(&p, 0)], &[1000], 0);
        assert_eq!(check_fee(&subject_of(&s), &beef(vec![p, s.clone()])), Ok(()));
    }

    #[test]
    fn b4_a_missing_parent_is_refused_even_when_the_present_one_covers_the_outputs() {
        let a = parent(&[2000]);
        let absent = parent(&[5]);
        let s = raw_tx(&[input(&a, 0), input(&absent, 0)], &[1500], 0);
        assert_eq!(check_fee(&subject_of(&s), &beef(vec![a, s.clone()])),
                   Err(Refusal::MissingParent { input: 1, prev_txid: txid(&absent), prev_vout: 0 }));
    }

    #[test]
    fn b5_a_forged_parent_is_not_found_by_position() {
        // Bytes worth 10,000 sit where the real parent would; their txid is not the input's.
        let real = parent(&[100]);
        let forged = parent(&[10_000]);
        let s = raw_tx(&[input(&real, 0)], &[1000], 0);
        assert!(matches!(check_fee(&subject_of(&s), &beef(vec![forged, s.clone()])), Err(Refusal::MissingParent { .. })));
    }

    #[test]
    fn b6_a_vout_beyond_the_parent_is_refused_not_a_panic() {
        let a = parent(&[2000]);
        let b = parent(&[300]);
        let s = raw_tx(&[input(&a, 0), input(&b, 7)], &[1000], 0);
        assert!(matches!(check_fee(&subject_of(&s), &beef(vec![a, b, s.clone()])), Err(Refusal::MissingParent { input: 1, .. })));
    }

    #[test]
    fn b7_a_negative_output_value_is_refused() {
        let p = parent(&[1000]);
        let s = raw_tx(&[input(&p, 0)], &[2000, (-1001i64) as u64], 0);
        assert!(matches!(check_fee(&subject_of(&s), &beef(vec![p, s.clone()])), Err(Refusal::BadValue(_))));
    }

    #[test]
    fn b8_values_and_totals_above_max_money_are_refused() {
        let huge = parent(&[(MAX_MONEY + 1) as u64]);
        let s = raw_tx(&[input(&huge, 0)], &[1], 0);
        assert!(matches!(check_fee(&subject_of(&s), &beef(vec![huge, s.clone()])), Err(Refusal::BadValue(_))));
        let a = parent(&[MAX_MONEY as u64]);
        let b = parent(&[MAX_MONEY as u64 - 1]);
        let s2 = raw_tx(&[input(&a, 0), input(&b, 0)], &[1], 0);
        assert!(matches!(check_fee(&subject_of(&s2), &beef(vec![a, b, s2.clone()])), Err(Refusal::BadValue(_))));
    }

    #[test]
    fn b9_the_same_outpoint_spent_twice_is_refused() {
        let p = parent(&[600]);
        let s = raw_tx(&[input(&p, 0), input(&p, 0)], &[1000], 0);
        assert_eq!(check_fee(&subject_of(&s), &beef(vec![p, s.clone()])), Err(Refusal::DuplicateInput { input: 1 }));
    }

    #[test]
    fn b10_a_null_prevout_is_refused() {
        let s = raw_tx(&[TxIn { prev_txid: "00".repeat(32), vout: 0xFFFF_FFFF, sequence: SEQUENCE_FINAL }], &[1], 0);
        assert!(matches!(check_fee(&subject_of(&s), &beef(vec![s.clone()])), Err(Refusal::MissingParent { .. })));
    }

    /// `/code-review max 159` F4: one input per parent, 3,000 parents of ~1 KB. Looking each parent up
    /// by re-hashing the whole bundle was quadratic (~9 GB hashed); it must finish within 5 s.
    #[test]
    fn b_review_f4_many_inputs_finish_in_bounded_time() {
        let parents: Vec<Vec<u8>> = (0..3000u32)
            .map(|i| {
                let mut script = vec![0x6a; 1000];
                script[..4].copy_from_slice(&i.to_le_bytes());
                let mut t = 1u32.to_le_bytes().to_vec();
                t.push(0); // no inputs (only hashed and parsed here)
                t.push(1);
                t.extend(10u64.to_le_bytes());
                t.extend([0xfd, 0xe8, 0x03]); // script length 1000
                t.extend(script);
                t.extend(0u32.to_le_bytes());
                t
            })
            .collect();
        let ins: Vec<TxIn> = parents.iter().map(|p| TxIn { prev_txid: txid(p), vout: 0, sequence: SEQUENCE_FINAL }).collect();
        let mut subject = 1u32.to_le_bytes().to_vec();
        subject.extend([0xfd, 0xb8, 0x0b]); // 3000 inputs
        for i in &ins {
            let mut t = hex::decode(&i.prev_txid).unwrap();
            t.reverse();
            subject.extend(t);
            subject.extend(0u32.to_le_bytes());
            subject.push(0);
            subject.extend(SEQUENCE_FINAL.to_le_bytes());
        }
        subject.push(1);
        subject.extend(1u64.to_le_bytes());
        subject.push(1);
        subject.push(0x51);
        subject.extend(0u32.to_le_bytes());
        let parsed = subject_of(&subject);
        let mut txs = parents;
        txs.push(subject);
        let b = beef(txs);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(check_fee(&parsed, &b));
        });
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(Ok(())), "3,000 inputs must be checked within 5 s");
    }

    /// `/code-review max 159` V2: 10,000 inputs all spending one 10,000-output parent. Parsing the
    /// parent once per input was quadratic (measured 6.8 s); it must finish within 2 s.
    #[test]
    fn b_review_v2_many_inputs_from_one_parent_finish_in_bounded_time() {
        let k: u32 = 10_000;
        let mut p = 1u32.to_le_bytes().to_vec();
        p.push(0); // no inputs (only hashed and parsed here)
        p.extend([0xfd, (k & 0xff) as u8, (k >> 8) as u8]);
        for _ in 0..k {
            p.extend(10u64.to_le_bytes());
            p.push(1);
            p.push(0x51);
        }
        p.extend(0u32.to_le_bytes());
        let pid = txid(&p);
        let mut subject = 1u32.to_le_bytes().to_vec();
        subject.extend([0xfd, (k & 0xff) as u8, (k >> 8) as u8]);
        let mut prev = hex::decode(&pid).unwrap();
        prev.reverse();
        for v in 0..k {
            subject.extend(&prev);
            subject.extend(v.to_le_bytes());
            subject.push(0);
            subject.extend(SEQUENCE_FINAL.to_le_bytes());
        }
        subject.push(1);
        subject.extend(1u64.to_le_bytes());
        subject.push(1);
        subject.push(0x51);
        subject.extend(0u32.to_le_bytes());
        let parsed = subject_of(&subject);
        let b = beef(vec![p, subject]);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(check_fee(&parsed, &b));
        });
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(2)), Ok(Ok(())), "10,000 inputs from one parent within 2 s");
    }

    // ---------- check 4 ----------

    const TIP: u32 = 970_300;
    const MTP: u32 = 1_791_500_000;
    fn view() -> ChainView {
        ChainView { tip_height: TIP, tip_median_time_past: MTP, synced: true }
    }
    fn locked(lock_time: u32, sequences: &[u32]) -> ParsedTransaction {
        let p = parent(&[10]);
        let ins: Vec<TxIn> = sequences.iter().map(|s| TxIn { prev_txid: txid(&p), vout: 0, sequence: *s }).collect();
        subject_of(&raw_tx(&ins, &[1], lock_time))
    }

    #[test]
    fn b11_a_far_future_height_lock_with_a_non_final_sequence_is_refused() {
        assert_eq!(check_final(&locked(TIP + 10_000, &[SEQUENCE_FINAL, 0]), view()), Err(Refusal::NonFinal { lock_time: TIP + 10_000 }));
    }

    #[test]
    fn b12_a_past_height_lock_is_final_and_the_boundary_is_tip_plus_one() {
        assert_eq!(check_final(&locked(TIP - 10, &[0]), view()), Ok(()));
        assert_eq!(check_final(&locked(TIP, &[0]), view()), Ok(()), "lock = tip: final in the next block (tip + 1 > lock)");
        // Our tip may trail the sender's by a block or two: the edge is retryable, not a verdict.
        assert_eq!(check_final(&locked(TIP + 1, &[0]), view()), Err(Refusal::HeadersUnavailable { lock_time: TIP + 1 }), "strict <, at the edge");
        assert_eq!(check_final(&locked(TIP + 2, &[0]), view()), Err(Refusal::HeadersUnavailable { lock_time: TIP + 2 }));
        assert_eq!(check_final(&locked(TIP + 3, &[0]), view()), Err(Refusal::NonFinal { lock_time: TIP + 3 }), "past the lag margin");
    }

    #[test]
    fn b13_a_time_lock_is_compared_with_the_median_time_past() {
        assert_eq!(check_final(&locked(MTP - 1, &[0]), view()), Ok(()));
        assert_eq!(check_final(&locked(MTP, &[0]), view()), Err(Refusal::NonFinal { lock_time: MTP }), "strict <");
        assert_eq!(check_final(&locked(MTP + 1, &[0]), view()), Err(Refusal::NonFinal { lock_time: MTP + 1 }));
    }

    #[test]
    fn b14_all_final_sequences_or_lock_zero_are_final_whatever_else() {
        assert_eq!(check_final(&locked(TIP + 10_000, &[SEQUENCE_FINAL, SEQUENCE_FINAL]), view()), Ok(()));
        assert_eq!(check_final(&locked(0, &[0, 1]), view()), Ok(()));
    }

    #[test]
    fn b15_unsynced_headers_cannot_judge_a_lock_but_need_not_for_a_final_tx() {
        let stale = ChainView { synced: false, ..view() };
        assert_eq!(check_final(&locked(TIP - 10, &[0]), stale), Err(Refusal::HeadersUnavailable { lock_time: TIP - 10 }));
        assert_eq!(check_final(&locked(0, &[0]), stale), Ok(()));
        assert_eq!(check_final(&locked(TIP + 5, &[SEQUENCE_FINAL]), stale), Ok(()));
    }
}

/// Rows `P7a-C1` (the proof half) and `P7a-B16`: a real mainnet BUMP checked against the wallet's own
/// headers, synced from the real fixture — no network.
#[cfg(test)]
pub(crate) mod proof_tests {
    use super::*;
    use crate::header_chain::{mainnet_header_chain, parse_headers, HeaderChain};

    const ARCADE_970001_970300: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/headers/arcade_970001_970300.bin"));
    /// FX2: a real mainnet transaction, index 3 in block 970,100
    /// (`000000000000000019d460cab1cc0b74848265dc6f77571c3b6ff0d49a86af4e`); raw bytes and TSC proof from
    /// WhatsOnChain, block and proof agreed by Bitails (2026-10-09).
    pub(crate) const FX2_TXID: &str = "eb9c87502c82cbea8e1ebd72059ef3ef42286a536c06bbd203e19cc56e3864b2";
    pub(crate) const FX2_RAW: &str = "010000000173487cf161e672cf9ff174f4f22f729747085d61541c877aede78509e2ec6e48000000006a47304402203e2899491164d72ce7a90cd02f2fc3ab6658d570c3d54dcec7a807f0deb82ab102200a63a2e6a1b7b7984768fb8838a78b8a7ae9e0d1b8f23d578b3e94c8c3cfa795412103f32dea2dfe53b8ba9a9853ad64f31e5a5ac71f3914aa0caf1e5b4b3b17557ba1ffffffff0157020000000000001976a914b5277186fa12916e877bcfe978f6499eec00c99b88ac00000000";
    pub(crate) const FX2_HEIGHT: u32 = 970_100;
    pub(crate) const FX2_INDEX: u64 = 3;
    pub(crate) const FX2_NODES: [&str; 10] = [
        "ceb4dda2683a193300e669d2dc4449c45a81a5e6a410a816af68245c13e65da8",
        "f7fddd5d698749526a8230bdfc4cb6659df9247d9eaed82a8a6394429623badc",
        "be5b1ca480266e66f5561c7c1480a86b24fd7c0ef1f5a0118242902e52731103",
        "2c254074ad1a614373f77242e2e8522ca9a6286879931ada060d94c729d9397c",
        "67277603d6f15e56727b7e821a22cd028a680fe462fd63cc78744b1993358d0d",
        "569e809adc11a506e8f9d35eb0976155b646df11e6c02458dae90f5c9c7f12cd",
        "a134d723854fd4733a4045c0f7e7969eb70b869efeca8c10c2a1add14b08e17b",
        "b40dba5aa2ce02665f5cdea7303834d5332961953f8353f65ef50eabe7297e03",
        "cff2829562e6a4a95bab4fab99a91eb098c933f13ebcd9d398ad2643039c6f86",
        "d6d919ec16073ee6d09763b295924c1d6a50ca6fd20e83dc399e7bbd78b582c2",
    ];

    /// The checkpoint plus the real headers 970,001 – 970,300.
    pub(crate) fn synced_chain() -> HeaderChain {
        let mut c = mainnet_header_chain().unwrap();
        for h in parse_headers(ARCADE_970001_970300).unwrap() {
            c.add_header(h, u32::MAX).unwrap();
        }
        c
    }

    /// FX2 as a one-transaction BEEF whose subject carries its real BUMP; `nodes` lets a test tamper.
    pub(crate) fn fx2_beef(height: u32, nodes: &[String]) -> Beef {
        let json: Vec<serde_json::Value> = nodes.iter().map(|n| serde_json::json!(n)).collect();
        let bump = crate::beef::tsc_proof_to_bump(FX2_TXID, height, FX2_INDEX, &json).unwrap();
        Beef { version: crate::beef::BEEF_V1_MARKER, bumps: vec![bump], transactions: vec![hex::decode(FX2_RAW).unwrap()], tx_to_bump: vec![Some(0)] }
    }

    fn real_nodes() -> Vec<String> {
        FX2_NODES.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn c1_a_real_mainnet_bump_is_valid_against_our_own_headers() {
        let chain = synced_chain();
        let beef = fx2_beef(FX2_HEIGHT, &real_nodes());
        // SUBJECT: the root we check is the header's own bytes 36..68 at that height.
        let root = crate::beef::bump_root_for_tx(&beef.bumps[0], &beef.transactions[0]).unwrap();
        assert_eq!(root, chain.header_at(FX2_HEIGHT).unwrap().header.merkle_root);
        let r = check_beef_proofs(&beef, 0, |r, h| chain.is_valid_root_for_height(r, h)).unwrap();
        assert!(r.subject_valid);
        assert_eq!((r.valid, r.unknown), (vec![(0, FX2_HEIGHT)], vec![]));
    }

    #[test]
    fn c1_one_flipped_sibling_byte_is_refused() {
        let chain = synced_chain();
        let mut nodes = real_nodes();
        nodes[4].replace_range(0..2, "68"); // 67… → 68…
        let r = check_beef_proofs(&fx2_beef(FX2_HEIGHT, &nodes), 0, |r, h| chain.is_valid_root_for_height(r, h));
        assert!(matches!(r, Err(Refusal::BadProof { tx: 0, height: FX2_HEIGHT, .. })), "{:?}", r);
    }

    #[test]
    fn c1_the_right_root_claimed_at_the_wrong_height_is_refused() {
        let chain = synced_chain();
        let r = check_beef_proofs(&fx2_beef(FX2_HEIGHT + 1, &real_nodes()), 0, |r, h| chain.is_valid_root_for_height(r, h));
        assert!(matches!(r, Err(Refusal::BadProof { .. })), "{:?}", r);
    }

    #[test]
    fn c1_a_block_we_do_not_hold_is_unknown_not_refused_and_not_valid() {
        let chain = synced_chain();
        let r = check_beef_proofs(&fx2_beef(970_400, &real_nodes()), 0, |r, h| chain.is_valid_root_for_height(r, h)).unwrap();
        assert!(!r.subject_valid);
        assert_eq!(r.unknown, vec![(0, 970_400)]);
    }

    #[test]
    fn c1_a_bump_index_past_the_end_is_refused() {
        let chain = synced_chain();
        let mut beef = fx2_beef(FX2_HEIGHT, &real_nodes());
        beef.tx_to_bump = vec![Some(5)];
        assert!(matches!(check_beef_proofs(&beef, 0, |r, h| chain.is_valid_root_for_height(r, h)), Err(Refusal::BadProof { .. })));
    }
}
