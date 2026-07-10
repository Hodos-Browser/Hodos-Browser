//! Spent-input reconciliation primitives (Wallet-Hardening WS1).
//!
//! Shared machinery for the `reconcile_spent_inputs` primitive that fixes both the
//! regular-send `"Missing inputs"` stuck-spend loop and the backup-token divergence.
//! Built in behavior-neutral phases (see
//! `development-docs/Wallet-Hardening/RECONCILE_PHASE2_DESIGN.md` §6):
//!   - **c1 (this commit):** [`check_outpoint_spent`] — the authoritative,
//!     cross-validated "is this outpoint spent?" check. Extracted from the inline
//!     single-WoC block in `do_onchain_backup` (`handlers.rs`) and hardened.
//!   - c2: `recover_change_index` (unwired).
//!   - c3: `reconcile_spent_inputs` (unwired).
//!
//! ## Spent-check decision rule (owner-locked 2026-07-10)
//!
//! Two independent providers are consulted — WhatsOnChain
//! (`/tx/{txid}/{vout}/spent`, TAAL infra) and GorillaPool ordinals
//! (`/txo/{txid}/{vout}/spend`). Each contributes exactly one
//! [`ProviderSignal`]: `ExplicitSpent(txid) | ExplicitUnspent | NoSignal`.
//!
//! A `404`, a down endpoint, a timeout, or an unparseable reply is ALWAYS
//! `NoSignal` — it can neither block an action nor be read as "unspent". This is
//! the owner's explicit refinement over design §5's `WoC 404 → Unspent`: only an
//! *explicit* spent/unspent answer moves the decision; absence never does.
//!
//! | Providers | Result |
//! |---|---|
//! | ≥1 `ExplicitSpent(Y)`, none `ExplicitUnspent` (agreeing on `Y`) | `Spent{Y}` |
//! | an `ExplicitUnspent`, none `ExplicitSpent` | `Unspent` |
//! | one spent + one unspent (flat contradiction), or two spent with **different** `Y`, or no explicit signal | `Unknown` |
//!
//! `Unknown` means *do nothing* — the caller fails closed.
//!
//! **Why one explicit "spent" is enough** (supersedes review finding D-P1's
//! stricter "both must agree"): a lagging node produces a false *unspent* — it
//! hasn't seen the spend yet — never a false *spent*. And every mark is further
//! gated downstream in c3 (fetch the successor `Y`, verify its txid hash, require
//! 1 confirmation + a valid merkle proof, and derive its recovered outputs to our
//! own keys), so a fabricated "spent" cannot survive to mutate money.
//!
//! **Asymmetry note (consequence of the owner rule, for c3):** because WoC's
//! `/spent` reports "unspent" as a `404` (→ `NoSignal` here), WhatsOnChain can
//! never emit `ExplicitUnspent`; only GorillaPool's explicit `200` unspent body
//! can. So the contradiction guard is effectively "GorillaPool may veto a WoC
//! spent", and `SpentStatus::Unspent` is GorillaPool-driven. c3's
//! "positive-unspent required to insert" gate must account for this.
//!
//! > **Live-probe TODO (P7 smoke):** the GorillaPool `/txo/{txid}/{vout}/spend`
//! > response shape is a Phase-1 assumption. [`parse_gorillapool_spend_body`]
//! > parses it defensively (any unrecognized shape → `NoSignal`), so a wrong
//! > endpoint degrades safely to WoC-only — which the owner accepts. Confirm the
//! > exact shape against the diverged dev wallet before c4 ships.

use ripemd::Ripemd160;
use rusqlite::Connection;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::crypto::brc42::derive_child_public_key;
use crate::database::AddressRepository;
use crate::recovery;

const WOC_BASE: &str = "https://api.whatsonchain.com/v1/bsv/main";
const GORILLAPOOL_BASE: &str = "https://ordinals.gorillapool.io/api";

/// Gap-scan window around `MAX(addresses.index)` for the uncached path (design §1
/// step 4b / §9 minor). Symmetric `back`/`gap_limit`, capped by guardrail #9 (≤50).
const GAP_BACK: i32 = 20;
const GAP_LIMIT: i32 = 20;

/// Authoritative result of a single-outpoint spent check. `Unknown` is the
/// fail-closed value: the caller must treat it as "do nothing".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpentStatus {
    /// Positively spent by `spending_txid` (a validated 64-hex-char txid).
    Spent { spending_txid: String },
    /// Positively unspent.
    Unspent,
    /// Inconclusive — fail closed (no explicit signal, or a flat contradiction).
    Unknown,
}

/// One provider's contribution to the decision. `NoSignal` covers every
/// non-explicit outcome (`404` / down / timeout / parse error): it can neither
/// block an action nor be read as unspent.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ProviderSignal {
    /// A validated 64-hex-char spending txid.
    ExplicitSpent(String),
    ExplicitUnspent,
    NoSignal,
}

/// A BSV txid is 32 bytes → exactly 64 hex chars. Guards the D-P3 fail-open:
/// a `200` whose txid field isn't a real hex txid contributes `NoSignal`, never
/// a sentinel like `"unknown"`.
fn is_valid_txid_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Pure decision table (owner-locked rule). Exhaustively unit-tested. Keeping this
/// separate from the async fetchers is what makes the rule testable without network.
fn decide_spent(woc: &ProviderSignal, gp: &ProviderSignal) -> SpentStatus {
    use ProviderSignal::*;

    let any_unspent = matches!(woc, ExplicitUnspent) || matches!(gp, ExplicitUnspent);
    let spent_txids: Vec<&String> = [woc, gp]
        .into_iter()
        .filter_map(|s| match s {
            ExplicitSpent(t) => Some(t),
            _ => None,
        })
        .collect();

    match (spent_txids.first(), any_unspent) {
        // No explicit-spent signal at all.
        (None, true) => SpentStatus::Unspent, // an explicit unspent, none spent
        (None, false) => SpentStatus::Unknown, // nothing explicit → do nothing
        // At least one explicit-spent.
        (Some(_), true) => SpentStatus::Unknown, // flat contradiction → hold and retry
        (Some(first), false) => {
            // One or both say spent, none contradicts. If both, they must agree on Y.
            if spent_txids.iter().all(|t| t == first) {
                SpentStatus::Spent {
                    spending_txid: (*first).clone(),
                }
            } else {
                // Providers disagree on WHICH txid spent it → ambiguous successor.
                SpentStatus::Unknown
            }
        }
    }
}

/// Parse a WhatsOnChain `/tx/{txid}/{vout}/spent` response into a signal.
/// `200` + a valid hex `txid` → `ExplicitSpent`; everything else → `NoSignal`
/// (a `404` means "no spend record", which the owner rule treats as absence, not
/// an explicit unspent).
fn parse_woc_spent_body(status: u16, body: &Value) -> ProviderSignal {
    if status != 200 {
        return ProviderSignal::NoSignal;
    }
    match body.get("txid").and_then(|v| v.as_str()) {
        Some(t) if is_valid_txid_hex(t) => ProviderSignal::ExplicitSpent(t.to_string()),
        _ => ProviderSignal::NoSignal, // 200 without a valid hex txid (D-P3)
    }
}

/// Parse a GorillaPool ordinals `/txo/{txid}/{vout}/spend` response into a signal.
/// Defensive across the plausible shapes; anything unrecognized → `NoSignal`.
///   - `{ "spend": "<txid>" }` — non-empty valid hex → spent; empty string → unspent.
///   - `{ "spent": bool, "spentTxid": "<txid>" }` — bool + valid txid.
///   - a bare JSON string (the txid, or empty) — mirrors the `spend`-field rule.
fn parse_gorillapool_spend_body(status: u16, body: &Value) -> ProviderSignal {
    if status != 200 {
        return ProviderSignal::NoSignal;
    }

    // Shape A: { "spend": "<txid or empty>" } — matches GorillaPool's txo model.
    if let Some(spend) = body.get("spend").and_then(|v| v.as_str()) {
        return classify_spend_str(spend);
    }

    // Shape B: { "spent": bool, "spentTxid": "<txid>" }
    if let Some(spent) = body.get("spent").and_then(|v| v.as_bool()) {
        if !spent {
            return ProviderSignal::ExplicitUnspent;
        }
        return match body.get("spentTxid").and_then(|v| v.as_str()) {
            Some(t) if is_valid_txid_hex(t) => ProviderSignal::ExplicitSpent(t.to_string()),
            _ => ProviderSignal::NoSignal, // spent:true but no valid txid
        };
    }

    // Shape C: a bare JSON string (the spending txid, or "").
    if let Some(s) = body.as_str() {
        return classify_spend_str(s);
    }

    ProviderSignal::NoSignal
}

/// Shared classifier for a "spend" string: valid hex txid → spent; empty → unspent;
/// anything else → no signal.
fn classify_spend_str(s: &str) -> ProviderSignal {
    let s = s.trim();
    if s.is_empty() {
        ProviderSignal::ExplicitUnspent
    } else if is_valid_txid_hex(s) {
        ProviderSignal::ExplicitSpent(s.to_string())
    } else {
        ProviderSignal::NoSignal
    }
}

/// Probe WhatsOnChain for a single outpoint's spent status.
async fn probe_woc_spent(client: &reqwest::Client, txid: &str, vout: u32) -> ProviderSignal {
    let url = format!("{}/tx/{}/{}/spent", WOC_BASE, txid, vout);
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return ProviderSignal::NoSignal,
    };
    let status = resp.status().as_u16();
    if status != 200 {
        return ProviderSignal::NoSignal;
    }
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    parse_woc_spent_body(status, &body)
}

/// Probe GorillaPool ordinals for a single outpoint's spent status. Fetches as
/// text so a plain-text (non-JSON) reply is still classifiable via shape C.
async fn probe_gorillapool_spend(client: &reqwest::Client, txid: &str, vout: u32) -> ProviderSignal {
    let url = format!("{}/txo/{}/{}/spend", GORILLAPOOL_BASE, txid, vout);
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return ProviderSignal::NoSignal,
    };
    let status = resp.status().as_u16();
    if status != 200 {
        return ProviderSignal::NoSignal;
    }
    let text = match resp.text().await {
        Ok(t) => t,
        Err(_) => return ProviderSignal::NoSignal,
    };
    // Try JSON; fall back to treating the raw text as a bare string value (shape C).
    let body: Value =
        serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.trim().to_string()));
    parse_gorillapool_spend_body(status, &body)
}

/// Authoritative, cross-validated spent check for a single outpoint.
///
/// Consults WhatsOnChain and GorillaPool concurrently and applies the owner-locked
/// decision table ([`decide_spent`]). Fail-closed: returns [`SpentStatus::Unknown`]
/// on any ambiguity so callers mutate nothing.
///
/// This does pure network reads only — no DB, no locks. Callers own serialization
/// (see design §3 lock contract).
pub async fn check_outpoint_spent(
    client: &reqwest::Client,
    txid: &str,
    vout: u32,
) -> SpentStatus {
    let (woc, gp) = tokio::join!(
        probe_woc_spent(client, txid, vout),
        probe_gorillapool_spend(client, txid, vout),
    );
    let status = decide_spent(&woc, &gp);
    log::info!(
        "   🔎 check_outpoint_spent {}:{} → {:?} (woc={:?}, gp={:?})",
        &txid[..16.min(txid.len())],
        vout,
        status,
        woc,
        gp
    );
    status
}

// ---------------------------------------------------------------------------
// c2 — recover_change_index
//
// Given the locking script of one output of a spending tx `Y`, find the wallet
// self-derivation index `N` such that BRC-42 `"2-receive address-{N}"` reproduces
// that exact script — i.e. an index we hold the signable key for. Returns `None`
// for anything that isn't a verifiable BRC-42 self output (foreign, BIP32, master,
// backup), so a caller can only ever insert change it can actually spend.
//
// Two candidate sources, ONE authoritative verify:
//   - Cache-first: the change address is normally already in `addresses`
//     (`handlers.rs:5529`), so an exact P2PKH match proposes candidate `N` fast —
//     even when `N` is far from `MAX(index)`. But the `addresses` table has NO
//     derivation-method column, so a matched `index>=0` row could be BIP32, not
//     BRC-42 (review D-K1). The match is therefore only a *candidate*.
//   - Gap-scan: bounded window around `MAX(index)` for uncached indices.
//
// Every candidate is verified by re-deriving `"2-receive address-{N}"` and
// byte-comparing to the target (guardrail #4). `index<0` (master −1 / external −2 /
// backup −3) is skipped structurally; the `"1-wallet-backup"` invoice can never be
// produced by this scan, so backup outputs never match.
// ---------------------------------------------------------------------------

/// P2PKH locking script for a pubkey: `OP_DUP OP_HASH160 <hash160> OP_EQUALVERIFY
/// OP_CHECKSIG`. `hash160 = RIPEMD160(SHA256(pubkey))` — the canonical derivation,
/// byte-identical to `recovery::address_to_p2pkh_script(pubkey_to_address(pubkey))`
/// and to a real on-chain P2PKH output.
fn pubkey_to_p2pkh_script(pubkey: &[u8]) -> Vec<u8> {
    let sha = Sha256::digest(pubkey);
    let hash160 = Ripemd160::digest(sha);
    let mut script = Vec::with_capacity(25);
    script.extend_from_slice(&[0x76, 0xa9, 0x14]); // OP_DUP OP_HASH160 PUSH(20)
    script.extend_from_slice(&hash160);
    script.extend_from_slice(&[0x88, 0xac]); // OP_EQUALVERIFY OP_CHECKSIG
    script
}

/// Derive the P2PKH script for self-derivation index `N` (`"2-receive address-{N}"`).
/// `None` on any derivation error (index is then simply skipped).
fn derive_receive_p2pkh_script(
    master_privkey: &[u8],
    master_pubkey: &[u8],
    index: i32,
) -> Option<Vec<u8>> {
    if index < 0 {
        return None; // never derive a receive script for special indices
    }
    let invoice = format!("2-receive address-{}", index);
    let pubkey = derive_child_public_key(master_privkey, master_pubkey, &invoice).ok()?;
    Some(pubkey_to_p2pkh_script(&pubkey))
}

/// True iff BRC-42 `"2-receive address-{index}"` reproduces `target_script` exactly.
/// The authoritative ownership+method check (guardrail #4).
fn verify_receive_index(
    master_privkey: &[u8],
    master_pubkey: &[u8],
    index: i32,
    target_script: &[u8],
) -> bool {
    derive_receive_p2pkh_script(master_privkey, master_pubkey, index)
        .map(|s| s == target_script)
        .unwrap_or(false)
}

/// Bounded gap-scan around `max_index` (design §1 step 4b). Returns the first index
/// in `[max−back, max+gap_limit]` (clamped at 0) whose BRC-42 derivation verifies.
fn gap_scan_receive_index(
    master_privkey: &[u8],
    master_pubkey: &[u8],
    max_index: i32,
    target_script: &[u8],
    back: i32,
    gap_limit: i32,
) -> Option<i32> {
    let lo = (max_index - back).max(0);
    let hi = max_index + gap_limit;
    (lo..=hi).find(|&i| verify_receive_index(master_privkey, master_pubkey, i, target_script))
}

/// Pure decision core: verify cache candidates first, else bounded gap-scan.
/// Separated from the DB reads so the whole rule (incl. the D-K1 wrong-key guard)
/// is unit-testable without a database.
fn recover_change_index_pure(
    master_privkey: &[u8],
    master_pubkey: &[u8],
    cache_candidates: &[i32],
    max_index: i32,
    target_script: &[u8],
) -> Option<i32> {
    // 1) Verify each cache-proposed candidate (skip special indices defensively).
    for &n in cache_candidates {
        if n >= 0 && verify_receive_index(master_privkey, master_pubkey, n, target_script) {
            return Some(n);
        }
    }
    // 2) Uncached — bounded gap-scan.
    gap_scan_receive_index(
        master_privkey,
        master_pubkey,
        max_index,
        target_script,
        GAP_BACK,
        GAP_LIMIT,
    )
}

/// Find the verified BRC-42 self index that owns `target_script`, or `None`.
///
/// Behavior-neutral (c2): unwired — c3 calls this to recover the spending tx's
/// wallet-owned change. Caller supplies the master keypair (fetched once via
/// `get_master_{private,public}_key_from_db`) and holds the DB lock for `conn`.
pub fn recover_change_index(
    conn: &Connection,
    master_privkey: &[u8],
    master_pubkey: &[u8],
    wallet_id: i64,
    target_script: &[u8],
) -> Option<i32> {
    let addr_repo = AddressRepository::new(conn);

    // Cache-first: index>=0 rows whose stored-address P2PKH byte-equals the target.
    // (A match is only a candidate — it might be a BIP32 row; the pure core re-verifies.)
    let cache_candidates: Vec<i32> = addr_repo
        .get_all_by_wallet(wallet_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|a| a.index >= 0)
        .filter(|a| {
            recovery::address_to_p2pkh_script(&a.address)
                .map(|s| s == target_script)
                .unwrap_or(false)
        })
        .map(|a| a.index)
        .collect();

    let max_index = addr_repo.get_max_index(wallet_id).ok().flatten().unwrap_or(-1);

    recover_change_index_pure(
        master_privkey,
        master_pubkey,
        &cache_candidates,
        max_index,
        target_script,
    )
}

// ---------------------------------------------------------------------------
// c3 — reconcile_spent_inputs building blocks (pure, lib-testable).
//
// The async orchestration `reconcile_spent_inputs(state: &AppState, ...)` lives in
// `handlers.rs` (it needs `AppState` + `cache_helpers::verify_tsc_proof_against_block`,
// both main-only). These pure helpers keep the parse + txid-verify + report logic
// unit-testable here; the orchestration's full exercise is integration/smoke (§7).
// ---------------------------------------------------------------------------

/// Outcome of a `reconcile_spent_inputs` pass. `changed()` gates the single
/// `balance_cache.invalidate()`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReconcileReport {
    /// Phantom outpoints marked spent with their real successor txid.
    pub marked_spent: u32,
    /// Wallet-owned change outputs of successors inserted as spendable.
    pub change_recovered: u32,
}

impl ReconcileReport {
    pub fn changed(&self) -> bool {
        self.marked_spent > 0 || self.change_recovered > 0
    }
}

/// Parse every output of a raw BSV tx into `(satoshis, locking_script_bytes)`.
/// Pure + bounds-checked (a malformed/truncated tx → `Err`, never a panic), so the
/// caller fails closed. Skips the input section; ignores witness (BSV has none).
pub fn parse_tx_outputs(raw_tx: &[u8]) -> Result<Vec<(i64, Vec<u8>)>, String> {
    use crate::transaction::decode_varint;

    let slice = |from: usize, len: usize| -> Result<&[u8], String> {
        raw_tx
            .get(from..from + len)
            .ok_or_else(|| "truncated tx".to_string())
    };

    let mut pos = 4usize; // skip version
    let (input_count, c) =
        decode_varint(raw_tx.get(pos..).ok_or("truncated")?).map_err(|e| format!("input count: {:?}", e))?;
    pos += c;
    for _ in 0..input_count {
        pos += 36; // prev txid (32) + vout (4)
        let (script_len, c) = decode_varint(raw_tx.get(pos..).ok_or("truncated")?)
            .map_err(|e| format!("input script len: {:?}", e))?;
        pos += c + script_len as usize + 4; // script + sequence
    }

    let (output_count, c) =
        decode_varint(raw_tx.get(pos..).ok_or("truncated")?).map_err(|e| format!("output count: {:?}", e))?;
    pos += c;

    let mut outputs = Vec::with_capacity(output_count as usize);
    for _ in 0..output_count {
        let value = u64::from_le_bytes(slice(pos, 8)?.try_into().unwrap());
        pos += 8;
        let (script_len, c) = decode_varint(raw_tx.get(pos..).ok_or("truncated")?)
            .map_err(|e| format!("output script len: {:?}", e))?;
        pos += c;
        let script = slice(pos, script_len as usize)?.to_vec();
        pos += script_len as usize;
        outputs.push((value as i64, script));
    }
    Ok(outputs)
}

/// True iff `SHA256d(raw)` (little-endian hex, i.e. the on-chain txid) equals
/// `expected_txid`. Closes fallback-provider poisoning: verify a fetched successor
/// before parsing it (design §1 step 2 / review D-K2).
pub fn verify_raw_txid(raw: &[u8], expected_txid: &str) -> bool {
    let h1 = Sha256::digest(raw);
    let h2 = Sha256::digest(h1);
    let computed: String = h2.iter().rev().map(|b| format!("{:02x}", b)).collect();
    computed.eq_ignore_ascii_case(expected_txid)
}

#[cfg(test)]
mod tests {
    use super::ProviderSignal::*;
    use super::*;
    use serde_json::json;

    const TXID_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const TXID_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    // --- is_valid_txid_hex ---

    #[test]
    fn valid_txid_is_64_hex() {
        assert!(is_valid_txid_hex(TXID_A));
        assert!(is_valid_txid_hex(
            "0123456789abcdefABCDEF0000000000000000000000000000000000000000ff"
        ));
    }

    #[test]
    fn invalid_txid_rejected() {
        assert!(!is_valid_txid_hex(""));
        assert!(!is_valid_txid_hex("unknown"));
        assert!(!is_valid_txid_hex("abc")); // too short
        assert!(!is_valid_txid_hex(&"a".repeat(63))); // 63 chars
        assert!(!is_valid_txid_hex(&"a".repeat(65))); // 65 chars
        assert!(!is_valid_txid_hex(&format!("{}g", &TXID_A[..63]))); // non-hex char
    }

    // --- decide_spent: the owner-locked decision table ---

    #[test]
    fn one_explicit_spent_other_silent_is_spent() {
        // Single provider positive is enough (lag can't fabricate a spent).
        assert_eq!(
            decide_spent(&ExplicitSpent(TXID_A.into()), &NoSignal),
            SpentStatus::Spent { spending_txid: TXID_A.into() }
        );
        assert_eq!(
            decide_spent(&NoSignal, &ExplicitSpent(TXID_A.into())),
            SpentStatus::Spent { spending_txid: TXID_A.into() }
        );
    }

    #[test]
    fn both_spent_agreeing_is_spent() {
        assert_eq!(
            decide_spent(&ExplicitSpent(TXID_A.into()), &ExplicitSpent(TXID_A.into())),
            SpentStatus::Spent { spending_txid: TXID_A.into() }
        );
    }

    #[test]
    fn both_spent_disagreeing_is_unknown() {
        // Providers disagree on the successor txid → ambiguous, fail closed.
        assert_eq!(
            decide_spent(&ExplicitSpent(TXID_A.into()), &ExplicitSpent(TXID_B.into())),
            SpentStatus::Unknown
        );
    }

    #[test]
    fn flat_contradiction_is_unknown() {
        // One says spent, the other explicitly says unspent → hold and retry.
        assert_eq!(
            decide_spent(&ExplicitSpent(TXID_A.into()), &ExplicitUnspent),
            SpentStatus::Unknown
        );
        assert_eq!(
            decide_spent(&ExplicitUnspent, &ExplicitSpent(TXID_A.into())),
            SpentStatus::Unknown
        );
    }

    #[test]
    fn explicit_unspent_alone_is_unspent() {
        assert_eq!(decide_spent(&ExplicitUnspent, &NoSignal), SpentStatus::Unspent);
        assert_eq!(decide_spent(&NoSignal, &ExplicitUnspent), SpentStatus::Unspent);
        assert_eq!(
            decide_spent(&ExplicitUnspent, &ExplicitUnspent),
            SpentStatus::Unspent
        );
    }

    #[test]
    fn no_signal_at_all_is_unknown() {
        // Both providers down / 404 → nothing happens (owner: absence never acts).
        assert_eq!(decide_spent(&NoSignal, &NoSignal), SpentStatus::Unknown);
    }

    // --- parse_woc_spent_body ---

    #[test]
    fn woc_200_with_valid_txid_is_spent() {
        assert_eq!(
            parse_woc_spent_body(200, &json!({ "txid": TXID_A })),
            ExplicitSpent(TXID_A.into())
        );
    }

    #[test]
    fn woc_200_without_valid_txid_is_no_signal() {
        // D-P3: never a "unknown" sentinel — an unusable 200 contributes nothing.
        assert_eq!(parse_woc_spent_body(200, &json!({ "txid": "unknown" })), NoSignal);
        assert_eq!(parse_woc_spent_body(200, &json!({})), NoSignal);
        assert_eq!(parse_woc_spent_body(200, &Value::Null), NoSignal);
    }

    #[test]
    fn woc_404_and_errors_are_no_signal() {
        // Owner rule: a 404 / down endpoint never blocks and never reads as unspent.
        assert_eq!(parse_woc_spent_body(404, &Value::Null), NoSignal);
        assert_eq!(parse_woc_spent_body(500, &Value::Null), NoSignal);
    }

    // --- parse_gorillapool_spend_body ---

    #[test]
    fn gp_spend_field_nonempty_is_spent() {
        assert_eq!(
            parse_gorillapool_spend_body(200, &json!({ "spend": TXID_A })),
            ExplicitSpent(TXID_A.into())
        );
    }

    #[test]
    fn gp_spend_field_empty_is_unspent() {
        assert_eq!(
            parse_gorillapool_spend_body(200, &json!({ "spend": "" })),
            ExplicitUnspent
        );
    }

    #[test]
    fn gp_spend_field_garbage_is_no_signal() {
        assert_eq!(
            parse_gorillapool_spend_body(200, &json!({ "spend": "not-a-txid" })),
            NoSignal
        );
    }

    #[test]
    fn gp_spent_bool_shape() {
        assert_eq!(
            parse_gorillapool_spend_body(200, &json!({ "spent": true, "spentTxid": TXID_A })),
            ExplicitSpent(TXID_A.into())
        );
        assert_eq!(
            parse_gorillapool_spend_body(200, &json!({ "spent": false })),
            ExplicitUnspent
        );
        // spent:true but no usable txid → no signal (don't invent a successor).
        assert_eq!(
            parse_gorillapool_spend_body(200, &json!({ "spent": true })),
            NoSignal
        );
    }

    #[test]
    fn gp_bare_string_shape() {
        assert_eq!(
            parse_gorillapool_spend_body(200, &Value::String(TXID_A.into())),
            ExplicitSpent(TXID_A.into())
        );
        assert_eq!(
            parse_gorillapool_spend_body(200, &Value::String("".into())),
            ExplicitUnspent
        );
    }

    #[test]
    fn gp_non_200_is_no_signal() {
        assert_eq!(parse_gorillapool_spend_body(404, &json!({ "spend": TXID_A })), NoSignal);
        assert_eq!(parse_gorillapool_spend_body(503, &Value::Null), NoSignal);
    }

    #[test]
    fn gp_unrecognized_shape_is_no_signal() {
        assert_eq!(parse_gorillapool_spend_body(200, &json!({ "foo": "bar" })), NoSignal);
        assert_eq!(parse_gorillapool_spend_body(200, &json!([1, 2, 3])), NoSignal);
    }

    // --- c2: recover_change_index (derivation-verify + gap-scan) ---

    /// Deterministic master keypair from a fixed 32-byte scalar.
    fn master_keys(seed: u8) -> (Vec<u8>, Vec<u8>) {
        use secp256k1::{PublicKey, Secp256k1, SecretKey};
        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(&[seed; 32]).expect("valid scalar");
        let pk = PublicKey::from_secret_key(&secp, &sk);
        (sk.secret_bytes().to_vec(), pk.serialize().to_vec())
    }

    /// The real change-output script for BRC-42 self index `n` under these keys.
    fn receive_script(priv_: &[u8], pub_: &[u8], n: i32) -> Vec<u8> {
        derive_receive_p2pkh_script(priv_, pub_, n).expect("derivable")
    }

    #[test]
    fn derive_skips_negative_indices() {
        let (pk, pub_) = master_keys(0x11);
        assert!(derive_receive_p2pkh_script(&pk, &pub_, -1).is_none());
        assert!(derive_receive_p2pkh_script(&pk, &pub_, -3).is_none());
        assert!(derive_receive_p2pkh_script(&pk, &pub_, 0).is_some());
    }

    #[test]
    fn verify_true_for_matching_index_only() {
        let (pk, pub_) = master_keys(0x11);
        let target = receive_script(&pk, &pub_, 7);
        assert!(verify_receive_index(&pk, &pub_, 7, &target));
        assert!(!verify_receive_index(&pk, &pub_, 6, &target)); // wrong index
        assert!(!verify_receive_index(&pk, &pub_, 8, &target));
    }

    #[test]
    fn verify_false_for_wrong_master_key() {
        // The D-K1 crux at the crypto level: a script derived under a DIFFERENT key
        // (e.g. a BIP32 row / foreign key) never re-derives under our BRC-42 key.
        let (pk_a, pub_a) = master_keys(0x11);
        let (pk_b, pub_b) = master_keys(0x22);
        let target = receive_script(&pk_a, &pub_a, 5);
        assert!(!verify_receive_index(&pk_b, &pub_b, 5, &target));
    }

    #[test]
    fn recover_returns_verified_cache_candidate() {
        let (pk, pub_) = master_keys(0x11);
        let target = receive_script(&pk, &pub_, 5);
        assert_eq!(
            recover_change_index_pure(&pk, &pub_, &[5], 100, &target),
            Some(5)
        );
    }

    #[test]
    fn recover_rejects_bip32_cache_poison() {
        // Cache proposes index 3, but the target is NOT the BRC-42 derivation of 3
        // (it's a foreign key's script — models a BIP32 index>=0 row). Verify fails,
        // gap-scan finds nothing → None. Never inserts an unsignable output.
        let (pk, pub_) = master_keys(0x11);
        let (pk_foreign, pub_foreign) = master_keys(0x22);
        let foreign_target = receive_script(&pk_foreign, &pub_foreign, 3);
        assert_eq!(
            recover_change_index_pure(&pk, &pub_, &[3], 10, &foreign_target),
            None
        );
    }

    #[test]
    fn recover_gap_scans_when_uncached() {
        let (pk, pub_) = master_keys(0x11);
        let target = receive_script(&pk, &pub_, 8);
        // No cache, index 8 within [10-20, 10+20].
        assert_eq!(recover_change_index_pure(&pk, &pub_, &[], 10, &target), Some(8));
    }

    #[test]
    fn recover_misses_far_uncached_index() {
        let (pk, pub_) = master_keys(0x11);
        let target = receive_script(&pk, &pub_, 100);
        // No cache, index 100 far outside the window around max=10 → miss.
        assert_eq!(recover_change_index_pure(&pk, &pub_, &[], 10, &target), None);
        // ...but the cache rescues a far index.
        assert_eq!(
            recover_change_index_pure(&pk, &pub_, &[100], 10, &target),
            Some(100)
        );
    }

    #[test]
    fn recover_none_for_foreign_script() {
        let (pk, pub_) = master_keys(0x11);
        let (pk_foreign, pub_foreign) = master_keys(0x33);
        let foreign = receive_script(&pk_foreign, &pub_foreign, 2);
        assert_eq!(recover_change_index_pure(&pk, &pub_, &[], 10, &foreign), None);
    }

    #[test]
    fn recover_skips_negative_cache_candidates() {
        let (pk, pub_) = master_keys(0x11);
        let target = receive_script(&pk, &pub_, 5);
        // −3 (backup) proposed alongside 5 — must be skipped, 5 wins.
        assert_eq!(
            recover_change_index_pure(&pk, &pub_, &[-3, 5], 100, &target),
            Some(5)
        );
    }

    // --- c3: parse_tx_outputs / verify_raw_txid / ReconcileReport ---

    /// A hand-built raw tx: 1 input, 2 P2PKH-ish outputs (1000 & 2000 sats).
    fn sample_raw_tx() -> Vec<u8> {
        let mut tx = Vec::new();
        tx.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // version
        tx.push(0x01); // 1 input
        tx.extend_from_slice(&[0xAB; 32]); // prev txid
        tx.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // vout 0
        tx.push(0x00); // empty scriptSig
        tx.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // sequence
        tx.push(0x02); // 2 outputs
        tx.extend_from_slice(&1000u64.to_le_bytes()); // value
        tx.push(0x03); // scriptlen
        tx.extend_from_slice(&[0xAA, 0xBB, 0xCC]); // script
        tx.extend_from_slice(&2000u64.to_le_bytes()); // value
        tx.push(0x02); // scriptlen
        tx.extend_from_slice(&[0xDD, 0xEE]); // script
        tx.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // locktime
        tx
    }

    #[test]
    fn parse_tx_outputs_reads_all_outputs() {
        let outs = parse_tx_outputs(&sample_raw_tx()).expect("parse");
        assert_eq!(outs.len(), 2);
        assert_eq!(outs[0], (1000, vec![0xAA, 0xBB, 0xCC]));
        assert_eq!(outs[1], (2000, vec![0xDD, 0xEE]));
    }

    #[test]
    fn parse_tx_outputs_fails_closed_on_truncation() {
        let full = sample_raw_tx();
        // Cut mid-tx — must Err, never panic (fail closed).
        assert!(parse_tx_outputs(&full[..full.len() - 5]).is_err());
        assert!(parse_tx_outputs(&[0x01, 0x00]).is_err());
    }

    #[test]
    fn verify_raw_txid_matches_sha256d() {
        use sha2::{Digest, Sha256};
        let raw = sample_raw_tx();
        let h2 = Sha256::digest(Sha256::digest(&raw));
        let expected: String = h2.iter().rev().map(|b| format!("{:02x}", b)).collect();
        assert!(verify_raw_txid(&raw, &expected));
        assert!(verify_raw_txid(&raw, &expected.to_uppercase())); // case-insensitive
        assert!(!verify_raw_txid(&raw, &"0".repeat(64))); // wrong txid
        assert!(!verify_raw_txid(&raw, "deadbeef")); // malformed
    }

    #[test]
    fn reconcile_report_changed_predicate() {
        assert!(!ReconcileReport::default().changed());
        assert!(ReconcileReport { marked_spent: 1, change_recovered: 0 }.changed());
        assert!(ReconcileReport { marked_spent: 0, change_recovered: 1 }.changed());
    }
}
