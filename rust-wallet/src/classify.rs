//! B5-T1-P5 — the classification seam: a coin is money only when the wallet has shown
//! it is, and what it cannot tell is held and shown, never spent.
//!
//! Owner decision 2 (option B, the TS `wallet-toolbox` rule): an output is money only
//! when **positively marked** `outputs.change = 1`. The money index (`money_utxos`,
//! B5-T1-P3) admits only stamped rows (`migrations.rs :: money_eligible`), so a row
//! written by any route that never reaches this module is selected by no route — the
//! default is refusal.
//!
//! Decision 7's tiered rule, in order ([`classify`]):
//! - **not observed** (`script_length IS NULL`, B5-T1-P4) ⇒ Unknown. The stored script of
//!   such a row was assumed from the address; reading it would be the vacuous-pass trap.
//! - **token recognisers** (B5-T2-P1, `crate::tokens`) on the observed script: a 1-sat
//!   ordinal or BSV-21 value output ⇒ [`Class::Token`], **filed** into `1sat` / `bsv21`
//!   (it leaves the default basket, so the balance and the held line); any other token
//!   shape ⇒ held (`token_shaped`); a script that will not parse ⇒ held (`unreadable`).
//!   👤 Owner, 2026-10-06: *"only a standard UTXO is money"* — a recogniser can only take
//!   a coin away from rule ②, never make one money.
//! - **rule ②** — money only when the observed script is **exactly** the 25-byte
//!   `OP_DUP OP_HASH160 <20> OP_EQUALVERIFY OP_CHECKSIG` template (never "starts with" /
//!   "ends with": BRC-162 puts a P2PKH after a token prefix), **and** `satoshis > 1`
//!   (**rule ③**: a 1-sat coin is held), **and** our key signs it.
//! - otherwise Unknown, with a reason, shown under the balance and retried every
//!   `TaskSyncPending` tick.
//!
//! Ownership is the signing derivation itself (`database::helpers::derive_key_for_output`,
//! the function every signer uses): `HASH160(pubkey)` must equal the template's hash.
//! A coin this wallet cannot sign for is not its money.
//!
//! ⛔ `change` is written only here (and by the V28 migration's one reset). A route that
//! writes `change = 1` itself forges the stamp — `preflight.ps1` gates it (`P5-A14`).

use log::{debug, info, warn};
use rusqlite::{params, Connection, OptionalExtension};

use crate::database::WalletDatabase;

/// Marker the "Treat as money" action appends to `output_description` (no schema change).
pub const TREATED_MARK: &str = "[treated as money by user";

/// Rows the per-tick pass examines at most.
/// Small on purpose: each ownership check runs the BIP39 seed stretch under the DB lock
/// (adversarial review #6); the tick runs the pass twice (start and end).
pub const PENDING_PASS_LIMIT: usize = 100;

/// Why a coin is not (yet) money. Serialised into `/wallet/held-outputs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// `script_length IS NULL` — the real script has not been read from the chain yet.
    NotObserved,
    /// The observed script is not exactly the 25-byte P2PKH template (an inscription,
    /// a token, a PushDrop, a listing …).
    NotPlainPayment,
    /// Exact template, 1 satoshi — decision 7 rule ③ (spending gains 1 sat and risks an NFT).
    /// Also the "Treat as money" refusal for a not-yet-read coin of ≤ 1 sat (B5-T2-P1b): the
    /// value objection, whatever the unread script turns out to be.
    OneSat,
    /// Exact template, but the key it pays is not one this wallet derives for the row.
    NotOurs,
    /// The wallet is locked, so ownership cannot be checked yet.
    WalletLocked,
    /// The row's derivation fields could not be turned into a key.
    DerivationFailed,
    /// Provably money, not stamped yet — the next pass stamps it (shown, not hidden).
    /// Also a recognised token not filed yet (the next pass files it).
    Pending,
    /// A token this wallet recognises but does not file (B5-T2-P1): Mandala, BSV-21
    /// non-value ops, legacy BSV-20, malformed BSV-21 JSON, a bad envelope, or an
    /// envelope on more than one satoshi — "token this wallet cannot show yet".
    TokenShaped,
    /// The observed script could not be read: it does not parse, or (above the inline
    /// cap) its parent's bytes are not held locally.
    Unreadable,
}

impl Reason {
    /// The rule that fired — named in every classification log line.
    pub fn rule(self) -> &'static str {
        match self {
            Reason::NotObserved => "not_observed",
            Reason::NotPlainPayment => "rule2:exact",
            Reason::OneSat => "rule2:value",
            Reason::NotOurs => "rule2:owner",
            Reason::WalletLocked => "owner:locked",
            Reason::DerivationFailed => "owner:derivation",
            Reason::Pending => "pending",
            Reason::TokenShaped => "token:held",
            Reason::Unreadable => "token:unreadable",
        }
    }

    /// Whether the user may promote a coin of `satoshis` held for this reason ("Treat as
    /// money"). Only where spending it would work and the wallet simply could not read the
    /// script yet — and never a coin of 1 satoshi or less (B5-T2-P1b): a transferred 1Sat
    /// Ordinal is a plain 1-sat P2PKH, so no script check can tell it from money, and the
    /// indexer that could answers 404 for ordinary payments. The 1Sat SDK's
    /// `fetchPayUtxos` never pays with a 1-sat coin either.
    pub fn user_may_treat_as_money(self, satoshis: i64) -> bool {
        self == Reason::NotObserved && satoshis > 1
    }

    /// Whether this reason says the coin is NOT money (vs "could not check"). Only a
    /// definitive reason may remove a stamp — a locked wallet or a derivation hiccup
    /// must never unmark money (adversarial review #3).
    pub fn is_definitive(self) -> bool {
        !matches!(self, Reason::WalletLocked | Reason::DerivationFailed | Reason::Pending)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    Money,
    /// A recognised token, filed out of the default basket (B5-T2-P1).
    Token(crate::tokens::Filing),
    Unknown(Reason),
}

/// Answer of the ownership check for a 20-byte public-key hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerCheck {
    Ours,
    NotOurs,
    Locked,
    Failed(String),
}

/// What the seam reads about a row. `script` is the inline `locking_script`; it is read
/// **only** when `script_length` says the script was observed (or built by us).
#[derive(Debug, Clone, Copy)]
pub struct Evidence<'a> {
    pub script_length: Option<i64>,
    pub script: Option<&'a [u8]>,
    pub satoshis: i64,
}

/// The seam's rule. Pure: the ownership check is a closure, called only for an exact
/// template of more than one satoshi.
pub fn classify(ev: &Evidence, owner: impl FnOnce(&[u8]) -> OwnerCheck) -> Class {
    let Some(len) = ev.script_length else {
        return Class::Unknown(Reason::NotObserved);
    };
    // B5-T2-P1 token recognisers, before rule ② (T2-P1 §11). They read the observed
    // script only — never a script assumed from the address (the check above).
    let Some(observed) = ev.script else {
        return Class::Unknown(Reason::Unreadable);
    };
    match crate::tokens::verdict(observed, ev.satoshis) {
        crate::tokens::Verdict::NotToken => {}
        crate::tokens::Verdict::File(f) => return Class::Token(f),
        crate::tokens::Verdict::Held(_) => return Class::Unknown(Reason::TokenShaped),
        crate::tokens::Verdict::Unreadable => return Class::Unknown(Reason::Unreadable),
    }
    let script = match ev.script {
        Some(s) if len == 25 && s.len() == 25 && crate::utxo_fetcher::is_p2pkh_script(s) => s,
        _ => return Class::Unknown(Reason::NotPlainPayment),
    };
    if ev.satoshis <= 1 {
        return Class::Unknown(Reason::OneSat);
    }
    match owner(&script[3..23]) {
        OwnerCheck::Ours => Class::Money,
        OwnerCheck::NotOurs => Class::Unknown(Reason::NotOurs),
        OwnerCheck::Locked => Class::Unknown(Reason::WalletLocked),
        OwnerCheck::Failed(_) => Class::Unknown(Reason::DerivationFailed),
    }
}

/// Does the key this wallet derives for the row's derivation fields hash to `pkh`?
pub fn owner_check(
    db: &WalletDatabase,
    prefix: Option<&str>,
    suffix: Option<&str>,
    sender: Option<&str>,
    pkh: &[u8],
) -> OwnerCheck {
    use ripemd::Ripemd160;
    use secp256k1::{PublicKey, Secp256k1, SecretKey};
    use sha2::{Digest, Sha256};

    if db.get_cached_mnemonic().is_err() {
        return OwnerCheck::Locked;
    }
    let privkey = match crate::database::helpers::derive_key_for_output(db, prefix, suffix, sender) {
        Ok(k) => k,
        Err(e) => return OwnerCheck::Failed(e.to_string()),
    };
    let secret = match SecretKey::from_slice(&privkey) {
        Ok(s) => s,
        Err(e) => return OwnerCheck::Failed(e.to_string()),
    };
    let pubkey = PublicKey::from_secret_key(&Secp256k1::new(), &secret).serialize();
    let hash = Ripemd160::digest(Sha256::digest(pubkey));
    if hash.as_slice() == pkh { OwnerCheck::Ours } else { OwnerCheck::NotOurs }
}

/// One `outputs` row as the seam needs it.
#[derive(Debug, Clone)]
pub struct Row {
    pub output_id: i64,
    pub user_id: i64,
    pub txid: Option<String>,
    pub vout: i32,
    pub satoshis: i64,
    pub script_length: Option<i64>,
    pub script_offset: Option<i64>,
    pub locking_script: Option<Vec<u8>>,
    pub derivation_prefix: Option<String>,
    pub derivation_suffix: Option<String>,
    pub sender_identity_key: Option<String>,
    pub change: bool,
    pub output_description: Option<String>,
}

impl Row {
    fn outpoint(&self) -> String {
        format!("{}:{}", self.txid.as_deref().unwrap_or("?"), self.vout)
    }
    fn user_treated(&self) -> bool {
        self.output_description.as_deref().map_or(false, |d| d.contains(TREATED_MARK))
    }
}

/// The class of one row, as the seam decides it (no write).
pub fn class_of(db: &WalletDatabase, row: &Row) -> Class {
    // A script above the inline cap is held only in its pinned parent (B5-T1-P4); the
    // recognisers need it (a 2.6 MB inscription is still an inscription).
    let inline = row.locking_script.as_deref().filter(|s| !s.is_empty());
    let from_parent = match (inline, row.txid.as_deref(), row.script_length, row.script_offset) {
        (None, Some(txid), Some(len), Some(off)) =>
            crate::database::output_repo::script_in_parent(db.connection(), txid, len, off),
        _ => None,
    };
    let ev = Evidence {
        script_length: row.script_length,
        script: inline.or(from_parent.as_deref()),
        satoshis: row.satoshis,
    };
    let class = classify(&ev, |pkh| owner_check(
        db,
        row.derivation_prefix.as_deref(),
        row.derivation_suffix.as_deref(),
        row.sender_identity_key.as_deref(),
        pkh,
    ));
    // A coin the user promoted stays money while the only objection is one the user
    // was allowed to overrule — and only while its STORED script is still the exact
    // template paying a key we sign with (the checks "Treat as money" ran at the click).
    // The mark is plain text in `output_description` and travels in backups, so it is
    // never trusted alone (adversarial review #1). Evidence the chain adds later (a real
    // non-template script) still wins.
    match class {
        Class::Unknown(r) if r.user_may_treat_as_money(row.satoshis) && row.user_treated()
            && stored_script_is_ours(db, row) => Class::Money,
        c => c,
    }
}

/// The stored script is the exact P2PKH template and our key for this row signs it.
fn stored_script_is_ours(db: &WalletDatabase, row: &Row) -> bool {
    let stored = row.locking_script.as_deref();
    let ev = Evidence { script_length: stored.map(|s| s.len() as i64), script: stored, satoshis: 2 };
    classify(&ev, |pkh| owner_check(db, row.derivation_prefix.as_deref(),
        row.derivation_suffix.as_deref(), row.sender_identity_key.as_deref(), pkh)) == Class::Money
}

/// Rows the seam classifies: unspent, in the default basket (or none) — exactly the
/// rows the balance counts (`OutputRepository :: calculate_balance` + `calculate_held`).
const CANDIDATES: &str = "
    SELECT o.outputId, o.txid, o.vout, o.satoshis, o.script_length, o.locking_script,
           o.derivation_prefix, o.derivation_suffix, o.sender_identity_key, o.change,
           o.output_description, o.user_id, o.script_offset
      FROM outputs o
      LEFT JOIN output_baskets b ON o.basket_id = b.basketId
     WHERE o.spendable = 1
       AND (o.basket_id IS NULL OR b.name = 'default')
       AND COALESCE(o.derivation_prefix, '') != '1-wallet-backup'";

fn read_rows(conn: &Connection, sql: &str, p: &[&dyn rusqlite::ToSql]) -> rusqlite::Result<Vec<Row>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(p, |r| Ok(Row {
        output_id: r.get(0)?,
        txid: r.get(1)?,
        vout: r.get(2)?,
        satoshis: r.get(3)?,
        script_length: r.get(4)?,
        locking_script: r.get(5)?,
        derivation_prefix: r.get(6)?,
        derivation_suffix: r.get(7)?,
        sender_identity_key: r.get(8)?,
        change: r.get::<_, i64>(9)? != 0,
        output_description: r.get(10)?,
        user_id: r.get(11)?,
        script_offset: r.get(12)?,
    }))?;
    rows.collect()
}

/// What one stamping call did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StampReport {
    pub examined: usize,
    /// Rows newly stamped money.
    pub stamped: usize,
    /// Rows whose stamp was removed (the seam is authoritative both ways).
    pub unstamped: usize,
    /// Rows left held / Unknown.
    pub held: usize,
    /// Recognised tokens moved into `1sat` / `bsv21` (B5-T2-P1).
    pub filed: usize,
}

/// Write the money stamp on one row. A row with no derivation recipe that the seam
/// proved is paid to the master key gets the recipe the signer and the index read
/// (`master` / `-1` — the label `main.rs` used to write on every boot without evidence,
/// B5-T1-P5 W1). `description` replaces `output_description` when given.
fn write_stamp(conn: &Connection, row: &Row, description: Option<&str>) -> rusqlite::Result<usize> {
    let master = row.derivation_prefix.is_none();
    conn.execute(
        "UPDATE outputs SET change = 1,
                derivation_prefix = CASE WHEN ?2 THEN 'master' ELSE derivation_prefix END,
                derivation_suffix = CASE WHEN ?2 THEN '-1' ELSE derivation_suffix END,
                output_description = COALESCE(?3, output_description)
          WHERE outputId = ?1 AND change = 0",
        params![row.output_id, master, description])
}

/// File a recognised token (B5-T2-P1): move the row out of the default basket into its
/// asset basket, clear any stamp, add the wallet's own tags, and write the wallet's
/// `customInstructions` only when the row carries none — a sender's string is kept
/// verbatim (BRC-37), and so are a sender's tags. All or nothing.
fn file_row(conn: &Connection, row: &Row, filing: &crate::tokens::Filing) -> rusqlite::Result<()> {
    let txid = row.txid.as_deref().unwrap_or("");
    conn.execute_batch("SAVEPOINT file_token")?;
    let r = (|| -> rusqlite::Result<()> {
        let basket_id = crate::database::BasketRepository::new(conn)
            .find_or_insert(filing.basket(), row.user_id)?;
        conn.execute(
            "UPDATE outputs SET basket_id = ?2, change = 0,
                    custom_instructions = CASE WHEN COALESCE(custom_instructions, '') = ''
                                               THEN ?3 ELSE custom_instructions END
              WHERE outputId = ?1",
            params![row.output_id, basket_id, filing.custom_instructions(txid, row.vout)])?;
        let tags = crate::database::TagRepository::new(conn);
        for tag in filing.tags(txid, row.vout) {
            tags.assign_tag_to_output(row.output_id, &tag)?;
        }
        Ok(())
    })();
    match r {
        Ok(()) => conn.execute_batch("RELEASE file_token"),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO file_token; RELEASE file_token");
            Err(e)
        }
    }
}

fn stamp_rows(db: &WalletDatabase, rows: Vec<Row>, route: &str) -> rusqlite::Result<StampReport> {
    let conn = db.connection();
    let mut rep = StampReport { examined: rows.len(), ..Default::default() };
    for row in rows {
        match class_of(db, &row) {
            Class::Token(filing) => {
                file_row(conn, &row, &filing)?;
                rep.filed += 1;
                info!("   🏷️ classify[{}] {} {} sats → filed '{}' (token)",
                      route, row.outpoint(), row.satoshis, filing.basket());
            }
            Class::Money => {
                if !row.change {
                    write_stamp(conn, &row, None)?;
                    rep.stamped += 1;
                    info!("   🏷️ classify[{}] {} {} sats → money (rule2:exact)", route, row.outpoint(), row.satoshis);
                }
            }
            Class::Unknown(reason) => {
                rep.held += 1;
                if row.change && reason.is_definitive() {
                    conn.execute("UPDATE outputs SET change = 0 WHERE outputId = ?1", params![row.output_id])?;
                    rep.unstamped += 1;
                    warn!("   🏷️ classify[{}] {} {} sats → held ({}) — stamp removed",
                          route, row.outpoint(), row.satoshis, reason.rule());
                } else {
                    debug!("   🏷️ classify[{}] {} {} sats → held ({})",
                           route, row.outpoint(), row.satoshis, reason.rule());
                }
            }
        }
    }
    Ok(rep)
}

/// Classify the given output rows (an ingest route, right after its write).
pub fn stamp_outputs(db: &WalletDatabase, output_ids: &[i64], route: &str) -> rusqlite::Result<StampReport> {
    let mut all = Vec::new();
    for id in output_ids {
        all.extend(read_rows(db.connection(), &format!("{CANDIDATES} AND o.outputId = ?1"), &[id])?);
    }
    stamp_rows(db, all, route)
}

/// The call every ingest route makes right after writing a coin: classify that
/// outpoint. A failure is logged and leaves the coin unstamped — held, shown, and
/// retried by [`stamp_pending`] — never money (fail closed).
pub fn after_write(db: &WalletDatabase, txid: &str, vout: u32, route: &str) {
    let r = read_rows(db.connection(), &format!("{CANDIDATES} AND o.txid = ?1 AND o.vout = ?2"),
                      &[&txid, &(vout as i64)])
        .and_then(|rows| stamp_rows(db, rows, route));
    if let Err(e) = r {
        warn!("   🏷️ classify[{}] {}:{} failed: {} — left held, retried next tick", route, txid, vout, e);
    }
}

/// B5-T2-P1: a route that writes a site's own outputs straight into `1sat` / `bsv21`
/// (`createAction` basket outputs, filed there by the recogniser at validation) calls this
/// once the transaction's **final** txid is on the rows — a `deploy+mint`'s token id is its
/// own outpoint. Adds the wallet's tags and CI, exactly as the seam files a row. A row
/// whose script no longer reads as a token is left alone and logged.
pub fn file_written_tokens(db: &WalletDatabase, txid: &str, route: &str) {
    let sql = format!("{} AND o.txid = ?1", CANDIDATES.replace(
        "AND (o.basket_id IS NULL OR b.name = 'default')", "AND b.name IN ('1sat', 'bsv21')"));
    let rows = match read_rows(db.connection(), &sql, &[&txid]) {
        Ok(rows) => rows,
        Err(e) => return warn!("   🏷️ classify[{}] {} filing failed: {}", route, txid, e),
    };
    // Each row on its own: one row's failure leaves the others filed (/code-review ultra #1).
    // A row that fails stays in its asset basket — protected, never money — without the
    // wallet's tags; nothing retries it (the background pass reads the default pool only).
    for row in rows {
        match class_of(db, &row) {
            Class::Token(f) => match file_row(db.connection(), &row, &f) {
                Ok(()) => info!("   🏷️ classify[{}] {} → filed '{}' (site-created token)", route, row.outpoint(), f.basket()),
                Err(e) => warn!("   🏷️ classify[{}] {} filing failed: {} — left in '{}' untagged", route, row.outpoint(), e, f.basket()),
            },
            c => warn!("   🏷️ classify[{}] {} is in an asset basket but reads as {:?}", route, row.outpoint(), c),
        }
    }
}

/// [`after_write`] for routes that hold the new row's id (its txid may still change at
/// signing).
pub fn after_write_id(db: &WalletDatabase, output_id: i64, route: &str) {
    if let Err(e) = stamp_outputs(db, &[output_id], route) {
        warn!("   🏷️ classify[{}] output {} failed: {} — left held, retried next tick", route, output_id, e);
    }
}

/// Classify every candidate row of one transaction (an ingest route that knows the txid).
pub fn stamp_txid(db: &WalletDatabase, txid: &str, route: &str) -> rusqlite::Result<StampReport> {
    let rows = read_rows(db.connection(), &format!("{CANDIDATES} AND o.txid = ?1"), &[&txid])?;
    stamp_rows(db, rows, route)
}

/// The background pass: unstamped candidates with `outputId > after_id`, at most
/// [`PENDING_PASS_LIMIT`] (decision 7 ④'s automatic retry, and the one-time migration of
/// rows written before B5-T1-P5). Returns the report and where the next call resumes —
/// 0 once the end is reached, so held rows can never starve the rows after them.
/// Skips while locked: nothing it could stamp can be proven without the keys.
pub fn stamp_pending(db: &WalletDatabase, route: &str, after_id: i64) -> rusqlite::Result<(StampReport, i64)> {
    if db.get_cached_mnemonic().is_err() {
        return Ok((StampReport::default(), after_id));
    }
    let rows = read_rows(db.connection(),
        &format!("{CANDIDATES} AND o.change = 0 AND o.outputId > ?1 ORDER BY o.outputId LIMIT {PENDING_PASS_LIMIT}"),
        &[&after_id])?;
    let next = if rows.len() == PENDING_PASS_LIMIT { rows.last().map(|r| r.output_id).unwrap_or(0) } else { 0 };
    let rep = stamp_rows(db, rows, route)?;
    if rep.stamped > 0 {
        info!("   🏷️ classify[{}]: {} coin(s) stamped money, {} held", route, rep.stamped, rep.held);
    }
    Ok((rep, next))
}

/// [`stamp_pending`] over every unstamped candidate — for a route that has just written
/// many rows at once (a backup import). Errors are logged; the rows stay held and the
/// tick retries them.
pub fn stamp_all(db: &WalletDatabase, route: &str) {
    let mut after = 0;
    loop {
        match stamp_pending(db, route, after) {
            Ok((_, 0)) => break,
            Ok((_, next)) => after = next,
            Err(e) => { warn!("   🏷️ classify[{}] failed: {} — rows stay held, retried next tick", route, e); break; }
        }
    }
}

/// The held rows the user is shown — and therefore the only rows "Treat as money" may
/// act on: not an output of a transaction that is unsigned, failed or not yet final.
/// One clause for both, so the route can never stamp a row the list does not offer
/// (B5-T2-P1b, /code-review 176 #5).
const SHOWN_HELD: &str = "
       AND NOT EXISTS (SELECT 1 FROM transactions t WHERE t.id = o.transaction_id
                        AND t.status IN ('unsigned', 'failed', 'nonfinal'))";

/// A held coin, as `/wallet/held-outputs` lists it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HeldOutput {
    pub txid: Option<String>,
    pub vout: i32,
    pub satoshis: i64,
    pub reason: Reason,
    #[serde(rename = "canTreatAsMoney")]
    pub can_treat_as_money: bool,
}

/// Every candidate row that is not money, with the seam's reason — exactly the rows
/// `OutputRepository :: calculate_held` sums (same transaction-status filter), so the
/// list and the held total shown under the balance agree.
pub fn list_held(db: &WalletDatabase, user_id: i64) -> rusqlite::Result<Vec<HeldOutput>> {
    let rows = read_rows(db.connection(),
        &format!("{CANDIDATES} AND o.change = 0 AND o.user_id = ?1 {SHOWN_HELD} ORDER BY o.outputId"),
        &[&user_id])?;
    Ok(rows.into_iter().map(|row| match class_of(db, &row) {
        // Provable now, stamped by the next pass — listed so the list and the held total
        // under the balance agree meanwhile (adversarial review #2).
        Class::Money | Class::Token(_) => HeldOutput { txid: row.txid.clone(), vout: row.vout,
            satoshis: row.satoshis, reason: Reason::Pending, can_treat_as_money: false },
        Class::Unknown(reason) => HeldOutput {
            txid: row.txid.clone(),
            vout: row.vout,
            satoshis: row.satoshis,
            reason,
            can_treat_as_money: reason.user_may_treat_as_money(row.satoshis) && treat_check(db, &row).is_ok(),
        },
    }).collect())
}

/// Why "Treat as money" was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreatRefusal {
    NotConfirmed,
    NotFound,
    /// Filed in a non-default basket — a token or an app's own output (T2's class).
    NotDefaultBasket(String),
    AlreadyMoney,
    Refused(Reason),
}

/// The checks "Treat as money" applies to a held row: the reason must be one the user
/// may overrule, the coin must hold more than one satoshi, and the **stored** script must
/// still be the exact template paying a key this wallet signs with — so a promoted coin
/// can be spent and nothing else can be promoted.
fn treat_check(db: &WalletDatabase, row: &Row) -> Result<(), TreatRefusal> {
    let reason = match class_of(db, row) {
        Class::Money => return Err(TreatRefusal::AlreadyMoney),
        Class::Token(_) => return Err(TreatRefusal::Refused(Reason::TokenShaped)),
        Class::Unknown(r) => r,
    };
    if !reason.user_may_treat_as_money(row.satoshis) {
        // A held coin keeps its own reason (a 1-sat token says token); a not-yet-read coin
        // is refused here only for its value, so the refusal names the floor.
        return Err(TreatRefusal::Refused(if reason == Reason::NotObserved { Reason::OneSat } else { reason }));
    }
    let stored = row.locking_script.as_deref();
    let ev = Evidence {
        script_length: stored.map(|s| s.len() as i64),
        script: stored,
        satoshis: row.satoshis,
    };
    match classify(&ev, |pkh| owner_check(db, row.derivation_prefix.as_deref(),
        row.derivation_suffix.as_deref(), row.sender_identity_key.as_deref(), pkh)) {
        Class::Money => Ok(()),
        Class::Token(_) => Err(TreatRefusal::Refused(Reason::TokenShaped)),
        Class::Unknown(r) => Err(TreatRefusal::Refused(r)),
    }
}

/// "Treat as money": a confirmed user action on exactly one outpoint. Stamps it and
/// records who/when in `output_description`.
pub fn treat_as_money(db: &WalletDatabase, txid: &str, vout: i32, confirmed: bool) -> Result<i64, TreatRefusal> {
    if !confirmed {
        return Err(TreatRefusal::NotConfirmed);
    }
    let conn = db.connection();
    let basket: Option<Option<String>> = conn.query_row(
        "SELECT b.name FROM outputs o LEFT JOIN output_baskets b ON o.basket_id = b.basketId
          WHERE o.txid = ?1 AND o.vout = ?2 AND o.spendable = 1",
        params![txid, vout], |r| r.get(0)).optional().map_err(|_| TreatRefusal::NotFound)?;
    match basket {
        None => return Err(TreatRefusal::NotFound),
        Some(Some(name)) if name != "default" => return Err(TreatRefusal::NotDefaultBasket(name)),
        _ => {}
    }
    let row = read_rows(conn, &format!("{CANDIDATES} AND o.txid = ?1 AND o.vout = ?2 {SHOWN_HELD}"), &[&txid, &vout])
        .map_err(|_| TreatRefusal::NotFound)?
        .into_iter().next().ok_or(TreatRefusal::NotFound)?;
    let reason = match class_of(db, &row) {
        Class::Money => return Err(TreatRefusal::AlreadyMoney),
        Class::Token(_) => return Err(TreatRefusal::Refused(Reason::TokenShaped)),
        Class::Unknown(r) => r,
    };
    treat_check(db, &row)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs()).unwrap_or(0);
    let note = format!("{} at {}; was {}]", TREATED_MARK, now, reason.rule());
    let desc = match row.output_description.as_deref() {
        Some(d) if !d.is_empty() => format!("{} {}", d, note),
        _ => note,
    };
    write_stamp(conn, &row, Some(&desc)).map_err(|_| TreatRefusal::NotFound)?;
    info!("   🏷️ classify[treat-as-money] {} {} sats → money (user; was {})",
          row.outpoint(), row.satoshis, reason.rule());
    Ok(row.output_id)
}

#[cfg(test)]
#[path = "classify_tests.rs"]
pub(crate) mod p5_tests;

#[cfg(test)]
#[path = "classify_t2_tests.rs"]
mod t2_tests;

#[cfg(test)]
#[path = "a4_independent_tests.rs"]
mod a4_independent_tests;
