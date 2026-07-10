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

use serde_json::Value;

const WOC_BASE: &str = "https://api.whatsonchain.com/v1/bsv/main";
const GORILLAPOOL_BASE: &str = "https://ordinals.gorillapool.io/api";

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
}
