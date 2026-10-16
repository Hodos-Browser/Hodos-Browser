//! TaskCheckPeerPay — Poll MessageBox for incoming BRC-29 PeerPay payments (auto-accept)
//!
//! Correct implementation using:
//! - BRC-103 AuthFetch for authenticated MessageBox API calls
//! - BRC-2 encrypted message decryption
//! - BRC-42 key derivation for payment verification
//! - Persistent tracking in `peerpay_received` table (deduplication)
//! - Auto-accept: derives private key, verifies P2PKH output, stores as spendable
//!
//! Interval: 60 seconds

use actix_web::web;
use log::{info, debug, warn, error};

use crate::AppState;
use crate::messagebox::{IncomingMessage, MessageBoxClient};
use crate::database::{PeerPayRepository, ParentTransactionRepository};

/// PaymentToken — parsed flexibly from the decrypted message body.
/// Different BRC-29 senders may use slightly different formats
/// (e.g., transaction as base64 string OR as byte array).
struct PaymentToken {
    custom_instructions: Option<PaymentInstructions>,
    transaction_bytes: Option<Vec<u8>>,  // Decoded transaction (from string or array)
    amount: Option<i64>,
}

struct PaymentInstructions {
    derivation_prefix: Option<String>,
    derivation_suffix: Option<String>,
}

/// Parse PaymentToken flexibly from JSON value
fn parse_payment_token(data: &[u8]) -> Result<PaymentToken, String> {
    let val: serde_json::Value = serde_json::from_slice(data)
        .map_err(|e| format!("invalid JSON: {}", e))?;

    let obj = val.as_object().ok_or("payment token is not a JSON object")?;

    // Parse customInstructions
    let instructions = obj.get("customInstructions").and_then(|ci| {
        let ci_obj = ci.as_object()?;
        Some(PaymentInstructions {
            derivation_prefix: ci_obj.get("derivationPrefix").and_then(|v| v.as_str().map(|s| s.to_string())),
            derivation_suffix: ci_obj.get("derivationSuffix").and_then(|v| v.as_str().map(|s| s.to_string())),
        })
    });

    // Parse transaction — accept base64 string OR byte array
    let transaction_bytes = if let Some(tx_val) = obj.get("transaction") {
        if let Some(tx_str) = tx_val.as_str() {
            // Base64 string
            Some(base64::Engine::decode(&base64::engine::general_purpose::STANDARD, tx_str)
                .map_err(|e| format!("invalid base64 transaction: {}", e))?)
        } else if let Some(tx_arr) = tx_val.as_array() {
            // Byte array [1, 2, 3, ...]
            let bytes: Result<Vec<u8>, _> = tx_arr.iter().map(|v| {
                v.as_u64().ok_or("non-numeric value in transaction array").map(|n| n as u8)
            }).collect();
            Some(bytes.map_err(|e| format!("invalid transaction array: {}", e))?)
        } else {
            None
        }
    } else {
        None
    };

    // Parse amount — accept number or string
    let amount = obj.get("amount").and_then(|a| {
        a.as_i64().or_else(|| a.as_str().and_then(|s| s.parse::<i64>().ok()))
    });

    Ok(PaymentToken {
        custom_instructions: instructions,
        transaction_bytes,
        amount,
    })
}

/// What one PeerPay message resolves to before any I/O: the transaction the
/// credit is read from, and our output inside it.
///
/// beta.3 Phase 10a (`P10a-A1`) — extracted from `run` so the parse → bind →
/// match step is a pure function a unit test can drive with a hand-built
/// envelope and assert on the *value*, not a log line.
pub(crate) struct Brc29Credit {
    /// Display-format txid the poller checks on chain and stores the output under.
    pub subject_txid: String,
    pub vout: u32,
    pub satoshis: i64,
    /// Locking script of the credited output.
    pub script: Vec<u8>,
    /// Raw bytes of the transaction the credit was read from — the poller
    /// broadcasts these when the subject is not yet on chain.
    pub credited_tx_bytes: Vec<u8>,
    /// The parsed bundle, kept for the parent-transaction cache.
    pub beef: crate::beef::Beef,
}

/// Why a message is not credited. [`CreditReject::decision`] says whether it is held (stored, then
/// acknowledged) or rejected (B5-T1-P7b).
#[derive(Debug)]
pub(crate) enum CreditReject {
    NotAtomicBeef(String),
    NoMainTransaction,
    TxParse(String),
    NoMatchingOutput,
    /// The message's own `amount` field disagrees with the output it points at.
    AmountMismatch { declared: i64, found: i64 },
    /// B5-T1-P7a-1: a BEEF proof that does not match our headers, or BRC-67 check 2 / 4.
    Spv(crate::spv_checks::Refusal),
    /// Review 163 #10: an input spends an output its parent (present in the bundle) does not have —
    /// invalid by consensus, never valid later.
    NoSuchParentOutput(String),
    /// B5-T1-P7b (V15): a PLAIN BEEF whose last transaction pays us nothing while another one pays the
    /// key derived for this message — a real coin may be behind it (the sender broke the last-tx rule).
    PaysOutsideSubject,
}

/// A proof for a block this close to our tip may be from a block that is about to win, or one just
/// orphaned while the payment was re-mined (adversarial review 2026-10-09): held and retried.
pub(crate) const RECENT_PROOF_WINDOW: u32 = 6;

/// A subject proof for a block above our synced tip by at most this much is "our headers are behind"
/// (one day of blocks); further out it is not a block we will hold soon.
pub(crate) const UNHELD_PROOF_WINDOW: u32 = 144;

/// B5-T1-P7b caps (owner decision 1, 2026-10-10). A message that would be HELD beyond them is
/// rejected as before this phase — the inbox is writable by anyone who knows our identity key.
pub(crate) use crate::database::peerpay_repo::HELD_BODY_MAX;
pub(crate) const HELD_PER_SENDER_MAX: i64 = 20;
pub(crate) use crate::database::peerpay_repo::HELD_TOTAL_MAX;
/// Held rows looked at again in one tick; the rest wait for the next (adversarial review: the
/// re-check runs inline in the Monitor).
pub(crate) const RECHECKS_PER_TICK: usize = 20;
/// Held messages that name no derivation are deleted after this long (owner, 2026-10-12).
pub(crate) const JUNK_TTL_SECS: i64 = 7 * 24 * 3600;

/// Hourly tries of a payment every broadcaster refuses before it is rejected (owner, 2026-10-12).
pub(crate) const REFUSED_TRIES: i64 = 24;
/// A not-final payment whose lock is further out than this is a lasting hold, not a temporary one
/// (owner, 2026-10-12): about a year of blocks, or a year of seconds for a time lock.
pub(crate) const LOCK_HORIZON_BLOCKS: u32 = 52_560;
pub(crate) const LOCK_HORIZON_SECS: u32 = 365 * 24 * 3600;

/// Parents fetched to re-check one restored payment (`beef_helpers` caps an ancestry walk at 50 too).
const MAX_RESTORED_PARENTS: usize = 500;

/// Reasons that do not pass by waiting — only a code change or the sender can change them. Over
/// the caps such a message is rejected as before this phase; a message held for a TRANSIENT reason
/// (our outage, our headers, its lock) is instead left in the inbox, so a full table can never cost
/// an honest payment its derivation (adversarial review HIGH, 2026-10-10).
pub(crate) fn is_lasting(reason: &str) -> bool {
    matches!(reason, "unreadable" | "missing_fields" | "not_beef" | "amount_mismatch" | "missing_parent" | "broadcast_refused"
        | "not_subject" | "bad_proof" | "non_final_far")
}

/// When a held message is looked at again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wait {
    /// Nothing will change by waiting (unreadable, a wrong amount, a missing parent): at the next
    /// wallet start only — a code upgrade may be able to read it.
    Start,
    /// Not final: once our chain passes this nLockTime.
    Lock(u32),
    /// Our headers or a very recent block: every 60 s.
    Flat,
    /// Our chain check or broadcast failed: 60 s doubling to an hour. Never dropped.
    Backoff,
    /// Refused by the network, or a restored row we cannot fetch: once an hour (review 164 #7/#11 — a
    /// look at the next start only could leave a real payment uncredited for days).
    Hourly,
}

/// What happens to a message that was not credited.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    /// Stored (V30), then acknowledged; re-checked per `wait`.
    Hold { reason: &'static str, wait: Wait },
    /// Not our money, or invalid by consensus: recorded as a rejection and acknowledged, no copy.
    Reject,
}

impl CreditReject {
    /// B5-T1-P7b (owner decisions 1–3, 2026-10-10): a message is acknowledged only after its payment
    /// is credited or stored. Only a payment that can never be ours, or never be valid, is rejected.
    pub(crate) fn decision(&self, tip_height: u32) -> Decision {
        use crate::spv_checks::Refusal;
        let hold = |reason, wait| Decision::Hold { reason, wait };
        match self {
            CreditReject::NotAtomicBeef(_) | CreditReject::NoMainTransaction | CreditReject::TxParse(_) => hold("not_beef", Wait::Start),
            CreditReject::AmountMismatch { .. } => hold("amount_mismatch", Wait::Start),
            CreditReject::NoMatchingOutput | CreditReject::NoSuchParentOutput(_) => Decision::Reject,
            CreditReject::PaysOutsideSubject => hold("not_subject", Wait::Start),
            CreditReject::Spv(Refusal::NonFinal { lock_time }) => {
                let far = if *lock_time < crate::spv_checks::LOCKTIME_THRESHOLD {
                    *lock_time > tip_height.saturating_add(LOCK_HORIZON_BLOCKS)
                } else {
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                    *lock_time as u64 > now + LOCK_HORIZON_SECS as u64
                };
                hold(if far { "non_final_far" } else { "non_final" }, Wait::Lock(*lock_time))
            }
            CreditReject::Spv(Refusal::HeadersUnavailable { .. }) => hold("headers_unavailable", Wait::Flat),
            CreditReject::Spv(Refusal::MissingParent { .. }) => hold("missing_parent", Wait::Start),
            CreditReject::Spv(Refusal::BadProof { height, reason, .. })
                if reason == crate::spv_checks::ROOT_MISMATCH && height.saturating_add(RECENT_PROOF_WINDOW) > tip_height =>
                hold("recent_bad_proof", Wait::Flat),
            // V15: a BUMP that cannot be computed proves nothing either way about the payment — held,
            // as `@bsv/sdk` never deletes a payment it could not accept. A computed root that is not our
            // settled header's is a verdict: rejected.
            CreditReject::Spv(Refusal::BadProof { reason, .. }) if reason != crate::spv_checks::ROOT_MISMATCH =>
                hold("bad_proof", Wait::Start),
            CreditReject::Spv(_) => Decision::Reject,
        }
    }

    fn description(&self) -> String {
        match self {
            CreditReject::NotAtomicBeef(e) => format!("not a valid Atomic or plain BEEF: {}", e),
            CreditReject::NoMainTransaction => "BEEF has no main transaction".to_string(),
            CreditReject::TxParse(e) => format!("failed to parse transaction: {}", e),
            CreditReject::NoMatchingOutput => "no output pays the key derived for this message".to_string(),
            CreditReject::PaysOutsideSubject => "a transaction other than the subject pays the key derived for this message".to_string(),
            CreditReject::NoSuchParentOutput(prev) => format!("an input spends an output its parent {} does not have", prev),
            CreditReject::AmountMismatch { declared, found } => format!("message declares {} sats but the output pays {}", declared, found),
            CreditReject::Spv(r) => r.description(),
        }
    }
}

/// `(subject txid, bundle)` of a payment's transaction bytes: an Atomic BEEF (BRC-95), or — B5-T1-P7b,
/// the PeerPay web app's converter — a plain BEEF (BRC-62, V1 or V2) whose subject is its LAST
/// transaction (`@bsv/sdk` `Transaction.fromBEEF`). Both strict: nothing may follow the bundle.
fn parse_payment_beef(tx_bytes: &[u8]) -> Result<(String, crate::beef::Beef), String> {
    if tx_bytes.starts_with(&crate::beef::ATOMIC_BEEF_MARKER) {
        return crate::beef::Beef::from_atomic_beef_bytes(tx_bytes);
    }
    if !(tx_bytes.starts_with(&crate::beef::BEEF_V1_MARKER) || tx_bytes.starts_with(&crate::beef::BEEF_V2_MARKER)) {
        return Err(format!("neither Atomic nor plain BEEF (starts {:02x?})", &tx_bytes[..tx_bytes.len().min(4)]));
    }
    let (beef, consumed) = crate::beef::Beef::from_bytes_consumed(tx_bytes)?;
    if consumed != tx_bytes.len() {
        return Err(format!("BEEF has {} trailing byte(s) after the bundle", tx_bytes.len() - consumed));
    }
    let last = beef.transactions.last().ok_or("BEEF has no transactions")?;
    use sha2::{Digest, Sha256};
    let txid = hex::encode(Sha256::digest(Sha256::digest(last)).iter().rev().copied().collect::<Vec<u8>>());
    Ok((txid, beef))
}

/// Resolve the credit for one message from its BEEF bytes, the BRC-42 child public key we derived
/// for it, and the amount the message declares (`None` / `0` = not declared). Pure — no DB, no
/// network: the header chain is the wallet's own, in memory. B5-T1-P7a-1: the BEEF's proofs are
/// checked against it and BRC-67 checks 2 + 4 run here, before the caller asks the chain or broadcasts.
pub(crate) fn resolve_brc29_credit(
    tx_bytes: &[u8],
    child_pubkey: &[u8],
    declared_amount: Option<i64>,
    chain: &crate::chain_tracker::ChainTracker,
) -> Result<Brc29Credit, CreditReject> {
    use sha2::{Sha256, Digest};
    use ripemd::Ripemd160;

    // Strict parse: rejects trailing bytes and a subject that hashes to no transaction in the
    // bundle (CU-3). The credit is read from the SUBJECT transaction — the one the on-chain check
    // is about.
    let (subject_txid, beef) = parse_payment_beef(tx_bytes).map_err(CreditReject::NotAtomicBeef)?;
    let plain = !tx_bytes.starts_with(&crate::beef::ATOMIC_BEEF_MARKER);

    let main_tx_bytes = beef.subject_transaction(&subject_txid)
        .cloned()
        .ok_or(CreditReject::NoMainTransaction)?;

    let parsed_tx = crate::beef::ParsedTransaction::from_bytes(&main_tx_bytes)
        .map_err(CreditReject::TxParse)?;

    // Expected P2PKH script from the derived pubkey
    let sha_hash = Sha256::digest(child_pubkey);
    let pubkey_hash = Ripemd160::digest(&sha_hash);

    // P2PKH: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG, to our derived key's hash.
    let pays_key = |script: &[u8]| script.len() == 25 && script[..3] == [0x76, 0xa9, 0x14]
        && script[23..] == [0x88, 0xac] && &script[3..23] == pubkey_hash.as_slice();
    for (i, output) in parsed_tx.outputs.iter().enumerate() {
        if pays_key(&output.script) {
            if let Some(declared) = declared_amount.filter(|a| *a > 0) {
                if declared != output.value {
                    return Err(CreditReject::AmountMismatch { declared, found: output.value });
                }
            }
            // B5-T1-P7a-1: the same checks as `internalize_action`, at the same point — after the
            // payment is known to be ours, before anything is asked of the chain or broadcast.
            let subject_idx = beef.transactions.iter().position(|t| *t == main_tx_bytes).unwrap_or(usize::MAX);
            let proofs = crate::spv_checks::check_beef_proofs(&beef, subject_idx, |r, h| chain.is_valid_root_for_height(r, h))
                .map_err(CreditReject::Spv)?;
            if !proofs.subject_valid {
                let view = crate::spv_checks::ChainView {
                    tip_height: chain.current_height(),
                    tip_median_time_past: chain.tip_median_time_past(),
                    synced: chain.is_synced(),
                };
                // V9 (B5-T1-P7b): the subject carries a proof for a block we do not hold yet, so a
                // sender may lawfully leave its parents out (BRC-62) — a missing parent is then our
                // headers' question, retried with them, not the payment's fault.
                // Review 164 #3: only a block just above our tip (or any above it while we are still
                // syncing) — a height we can never hold stays a missing parent.
                let tip = chain.current_height();
                let subject_unknown = proofs.unknown.iter().any(|(i, h)| *i == subject_idx
                    && *h > tip && (!chain.is_synced() || *h <= tip.saturating_add(UNHELD_PROOF_WINDOW)));
                crate::spv_checks::check_fee(&parsed_tx, &beef)
                    .map_err(|r| match r {
                        crate::spv_checks::Refusal::MissingParent { .. } if subject_unknown =>
                            crate::spv_checks::Refusal::HeadersUnavailable { lock_time: parsed_tx.lock_time },
                        r => r,
                    })
                    .and_then(|_| crate::spv_checks::check_final(&parsed_tx, view))
                    .map_err(|r| match r {
                        crate::spv_checks::Refusal::MissingParent { ref prev_txid, .. } if beef.find_txid(prev_txid).is_some() =>
                            CreditReject::NoSuchParentOutput(prev_txid.clone()),
                        r => CreditReject::Spv(r),
                    })?;
            }
            return Ok(Brc29Credit {
                subject_txid,
                vout: i as u32,
                satoshis: output.value,
                script: output.script.clone(),
                credited_tx_bytes: main_tx_bytes,
                beef,
            });
        }
    }

    // V15: in a PLAIN BEEF the subject is only inferred (the last transaction) — does another one pay
    // this key? An Atomic envelope names its subject, and another transaction paying us there is the
    // CU-3 attack shape (P10a-A1): rejected as before.
    let pays_us = |tx: &Vec<u8>| crate::beef::ParsedTransaction::from_bytes(tx)
        .map(|t| t.outputs.iter().any(|o| pays_key(&o.script))).unwrap_or(false);
    if plain && beef.transactions.iter().any(|t| *t != main_tx_bytes && pays_us(t)) {
        return Err(CreditReject::PaysOutsideSubject);
    }
    Err(CreditReject::NoMatchingOutput)
}

/// The first `n` characters of sender-controlled text, for a log line. A byte slice panics when a
/// multi-byte character straddles `n` (B5-T1-P7b; `messagebox.rs` fixed its own copy earlier).
fn short(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// beta.3 Phase 10a (`P10a-A7`, owner decision 2026-09-15) — an invalid incoming
/// payment is rejected, written to the wallet log as the audit line, and the user
/// is told **once per sender per wallet session** through the quiet notification
/// list — never a modal, because the inbox is writable by anyone who knows the
/// identity key and a modal per fake would be an attention-DoS handed to them.
///
/// beta.3 Phase 10e (panel `F3-10a`) — the dedupe is now the DB ROW ITSELF, with no
/// in-process state at all.
///
/// ⛔ What was here: an in-process `OnceLock<Mutex<HashSet<String>>>` of senders,
/// plus `INSERT OR IGNORE`. Two problems, in opposite directions. The set grew
/// without bound and was never cleared, and a fresh key costs an attacker nothing —
/// so the memory was paid by us and the quiet was paid by the honest case. And
/// because the row survives dismissal, a sender the user had dismissed could NEVER
/// raise a visible notice again for the life of the wallet, not even for a different
/// attack later. The doc comment claimed "only re-notifies after a restart", which
/// was wrong: after a restart the set is empty, the INSERT is ignored, and the
/// notice is silently dropped.
///
/// Now: at most ONE visible notice per sender at a time. Repeated rejections while
/// it is on screen change nothing. Dismiss silences it. The next rejection after
/// that raises it once more. Still never a modal.
fn record_rejected_message(conn: &rusqlite::Connection, sender: &str, message_id: &str, why: &str) {
    let sender_prefix = short(sender, 16);
    warn!("🚫 PeerPay REJECTED — sender {}… message {}…: {}", sender_prefix, short(message_id, 16), why);

    match PeerPayRepository::insert_rejected_notification(conn, sender) {
        Ok(true) => info!("   🔔 Rejected-payment notification recorded for sender {}…", sender_prefix),
        Ok(false) => debug!("   Rejected-payment notice for sender {}… is already on screen", sender_prefix),
        Err(e) => warn!("   Failed to record rejected-payment notification: {}", e),
    }
}

/// The MessageBox inbox the poller reads and acknowledges (B5-T1-P7b seam: tests drive `run_with`
/// with a fake inbox that records every acknowledge).
#[async_trait::async_trait]
pub(crate) trait PeerPayInbox: Send + Sync {
    async fn list(&self) -> Result<Vec<IncomingMessage>, String>;
    async fn acknowledge(&self, message_ids: &[String]) -> Result<(), String>;
    /// Which server this inbox is, for logs.
    fn host(&self) -> &str { "inbox" }
}

/// B5-T5-P7a: one MessageBox host's `payment_inbox`. A message is acknowledged on the host it was
/// listed from.
struct HostInbox<'a> {
    client: &'a MessageBoxClient,
    host: &'a str,
}

/// B5-T5-P7a: every known host — a sender who cannot find where we read posts to its own default.
fn host_inboxes(client: &MessageBoxClient) -> Vec<HostInbox<'_>> {
    crate::messagebox::MESSAGEBOX_HOSTS.iter().map(|host| HostInbox { client, host }).collect()
}

#[async_trait::async_trait]
impl PeerPayInbox for HostInbox<'_> {
    async fn list(&self) -> Result<Vec<IncomingMessage>, String> {
        self.client.list_messages(self.host, "payment_inbox").await.map_err(|e| e.to_string())
    }
    async fn acknowledge(&self, message_ids: &[String]) -> Result<(), String> {
        self.client.acknowledge(self.host, message_ids).await.map_err(|e| e.to_string())
    }
    fn host(&self) -> &str { self.host }
}

/// When this wallet process started: held rows that wait for "wallet start" are those not looked
/// at since (review 164 #1 — a one-shot flag could be spent on 20 rows and starve the rest).
pub(crate) fn process_started_at() -> i64 {
    static STARTED: once_cell::sync::Lazy<i64> = once_cell::sync::Lazy::new(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
    });
    *STARTED
}

/// Run the TaskCheckPeerPay task.
pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    // Get our master keys
    let (master_privkey, master_pubkey) = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let privkey = match crate::database::get_master_private_key_from_db(&db) {
            Ok(k) => k,
            Err(_) => {
                debug!("TaskCheckPeerPay: no wallet yet, skipping");
                return Ok(());
            }
        };
        let pubkey = match crate::database::get_master_public_key_from_db(&db) {
            Ok(k) => k,
            Err(_) => {
                debug!("TaskCheckPeerPay: can't get public key, skipping");
                return Ok(());
            }
        };
        (privkey, pubkey)
    };

    // Build MessageBox client with our identity
    let mb_client = MessageBoxClient::new(master_privkey, master_pubkey);
    // One poll at a time (Monitor tick and POST /wallet/peerpay/check); the clock is read once the
    // lock is ours, so a waiting caller never schedules with a stale `now` (review 163 #12).
    let _one_at_a_time = RUN_LOCK.lock().await;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let inboxes = host_inboxes(&mb_client);
    let inboxes: Vec<&dyn PeerPayInbox> = inboxes.iter().map(|i| i as &dyn PeerPayInbox).collect();
    run_with_inboxes(state, &inboxes, now, process_started_at()).await
}

/// How long one MessageBox host gets to list its whole inbox per poll (B5-T5-P7a). Each page has its own
/// 20 s budget (`messagebox.rs :: PAGE_BUDGET`), so a dead host is cut after one page; this outer bound
/// only stops a host that keeps answering slowly, and is large enough for every page a flooded inbox
/// has (review 178-2 #6: a whole-list budget threw away pages already fetched).
const LIST_BUDGET: std::time::Duration = std::time::Duration::from_secs(20 * 21);
/// How long one MessageBox host gets to acknowledge per poll (review 178 #7).
const ACK_BUDGET: std::time::Duration = std::time::Duration::from_secs(20);

/// The Monitor tick and `POST /wallet/peerpay/check` both run a poll; one at a time, so a message is
/// never decided twice concurrently (B5-T1-P7b: the kickoff audit found no lock between them).
static RUN_LOCK: once_cell::sync::Lazy<tokio::sync::Mutex<()>> = once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(()));

/// One message, from the inbox or from a held row.
struct Payment<'a> {
    message_id: &'a str,
    sender: &'a str,
    /// `None` only for a held row restored from a backup (derivation + txid alone).
    body: Option<&'a [u8]>,
    restored: Option<(String, String, String)>,
    /// Re-checked from a held row.
    held: bool,
    /// The row came from a backup (it may be older than a spend of its own coin: V4).
    from_backup: bool,
    /// B5-T5-P7a (review 178-3 #1): a copy whose body differs from the row held under the same id. If
    /// it credits, it is recorded as `{id}:{txid}` and removes the held row only if that row is this
    /// same payment — the held one may be a different, real payment.
    contested: bool,
}

/// What became of one message.
enum Outcome {
    Credited,
    Hold { reason: &'static str, wait: Wait, prefix: Option<String>, suffix: Option<String>, txid: Option<String> },
    Reject(String),
    /// A verdict that does not rest on the BEEF we assembled — the output is spent, a store conflict,
    /// an invalid sender key. Final even for a restored row (review 164 #2).
    Final(String),
    /// A transient local error (the database): not acknowledged, not stored; next tick.
    Retry,
    /// B5-T5-P7a: this payment's output is already credited — the same payment from another server,
    /// or in another message. Acknowledged, and recorded nowhere: credited once, notified once.
    Duplicate,
}

/// One poll (B5-T1-P7b): list the inbox and decide every message — credit it, or store it, or
/// reject it — and acknowledge ONLY those credited or stored (or rejected); then re-check the held
/// rows that are due.
#[cfg(test)]
pub(crate) async fn run_with(
    state: &web::Data<AppState>,
    inbox: &dyn PeerPayInbox,
    now: i64,
    started_at: i64,
) -> Result<(), String> {
    run_with_inboxes(state, &[inbox], now, started_at).await
}

/// One poll over several MessageBox hosts (B5-T5-P7a): each host's inbox in turn, then the held rows
/// once.
pub(crate) async fn run_with_inboxes(
    state: &web::Data<AppState>,
    inboxes: &[&dyn PeerPayInbox],
    now: i64,
    started_at: i64,
) -> Result<(), String> {
    let master_privkey = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        match crate::database::get_master_private_key_from_db(&db) {
            Ok(k) => k,
            Err(_) => {
                debug!("TaskCheckPeerPay: no wallet yet, skipping");
                return Ok(());
            }
        }
    };

    let mut processed_count = 0;
    // B5-T5-P7a (review 178-2 #1, #2, #13): several servers can list one message id with DIFFERENT
    // bodies (a buggy or hostile server). A shortcut taken on the id alone would let one server's junk
    // decide — and acknowledge away — another server's real payment. So this tick's shortcuts are keyed
    // by (id, body hash): the same copy is handled once; a different body under the same id is decided.
    // (Credited ids stay a shortcut on the id: the real payment is already in.)
    let body_key = |id: &str, body: &[u8]| -> (String, [u8; 32]) {
        use sha2::{Digest, Sha256};
        (id.to_string(), Sha256::digest(body).into())
    };
    let mut decided_now: std::collections::HashSet<(String, [u8; 32])> = std::collections::HashSet::new();
    // Acknowledged this tick: the same copy on another host is acknowledged there too.
    let mut acked_now: std::collections::HashSet<(String, [u8; 32])> = std::collections::HashSet::new();
    // Ids decided from an inbox this tick through the ordinary path (they may have just been held): the
    // held re-check below does not decide them again. A differing copy of a held id does NOT go here —
    // the held row keeps its own schedule (review 178-2 #2).
    let mut decided_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut contested_now = 0usize;

    // 1. The inbox, on every host. Listed at once, each within a budget, so the poll waits for the
    // slowest host only up to that budget (review 178 #7); then decided one host at a time. Deciding
    // each host as soon as its own listing completes is ticketed (review 178-4 #11, 178-7 #4).
    let listings = futures_util::future::join_all(inboxes.iter().map(|inbox| async move {
        match tokio::time::timeout(LIST_BUDGET, inbox.list()).await {
            Ok(r) => r,
            Err(_) => Err(format!("no answer within {} s", LIST_BUDGET.as_secs())),
        }
    })).await;
    for (inbox, listing) in inboxes.iter().zip(listings) {
    match listing {
        Ok(messages) => {
            if !messages.is_empty() {
                info!("📬 TaskCheckPeerPay: polled payment_inbox on {} — {} message(s)", inbox.host(), messages.len());
            }
            let mut message_ids_to_ack: Vec<String> = Vec::new();
            for msg in &messages {
                let key = body_key(&msg.message_id, &msg.body);
                if msg.message_id.is_empty() {
                    // It can be neither acknowledged nor stored under an id.
                    warn!("TaskCheckPeerPay: a message from {}… has no message id — skipped", short(&msg.sender, 16));
                    continue;
                }
                // Already credited, or already stored: the copy exists — acknowledge.
                let known = {
                    let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
                    let conn = db.connection();
                    (|| -> rusqlite::Result<Option<bool>> {
                        if PeerPayRepository::is_already_processed(conn, &msg.message_id)? {
                            return Ok(Some(true));
                        }
                        if !PeerPayRepository::is_held(conn, &msg.message_id)? {
                            return Ok(Some(false));
                        }
                        // Review 178 #3: one id, two servers, two bodies. A copy that differs from the
                        // one we hold is not "the copy exists" — acknowledging it could delete the real
                        // payment while we hold another server's junk. `None`: decide it, and remove it
                        // only if it credits.
                        match PeerPayRepository::held_body(conn, &msg.message_id)? {
                            Some(b) if b != msg.body => return Ok(None),
                            Some(_) => {}
                            // Review 178-3 #3: a row restored without its body takes a listed body only
                            // when that body names the row's derivation; anything else is a contesting copy.
                            None => {
                                let (pf, sf) = PeerPayRepository::held_derivation(conn, &msg.message_id)?;
                                // As `decide` reads a body: decrypted, else as it came.
                                let plain = decrypt_stored(&master_privkey, &msg.sender, &msg.body).unwrap_or_else(|| msg.body.clone());
                                let named = parse_payment_token(&plain).ok()
                                    .and_then(|t| t.custom_instructions)
                                    .map(|ci| (ci.derivation_prefix, ci.derivation_suffix));
                                if pf.is_none() || named != Some((pf, sf)) {
                                    return Ok(None);
                                }
                            }
                        }
                        // A held row restored without its body gets it now, before the acknowledge.
                        if msg.body.len() <= HELD_BODY_MAX {
                            PeerPayRepository::fill_held_body(conn, &msg.message_id, &msg.sender, &msg.body)?;
                        }
                        Ok(Some(true))
                    })()
                };
                match known {
                    Ok(Some(true)) => {
                        message_ids_to_ack.push(msg.message_id.clone());
                        continue;
                    }
                    Ok(None) => {
                        // Review 178-3 #7: once per tick per body, and no more often than the held row
                        // itself is looked at (a hostile host must not make us re-broadcast every minute).
                        let due = {
                            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
                            PeerPayRepository::held_contest_due(db.connection(), &msg.message_id, now, started_at)
                        };
                        // Review 178-6 #3: and at most `RECHECKS_PER_TICK` contesting copies per poll.
                        if decided_now.contains(&key) || !matches!(due, Ok(true)) || contested_now >= RECHECKS_PER_TICK {
                            continue;
                        }
                        contested_now += 1;
                        let payment = Payment { message_id: &msg.message_id, sender: &msg.sender, body: Some(&msg.body), restored: None, held: false, from_backup: false, contested: true };
                        match decide(state, &master_privkey, &payment).await {
                            Outcome::Credited => {
                                processed_count += 1;
                                message_ids_to_ack.push(msg.message_id.clone());
                            }
                            Outcome::Duplicate => message_ids_to_ack.push(msg.message_id.clone()),
                            _ => warn!("TaskCheckPeerPay: message {}… on {} differs from the copy we hold and does not credit — left on its server",
                                short(&msg.message_id, 16), inbox.host()),
                        }
                        decided_now.insert(key);
                        continue;
                    }
                    Ok(Some(false)) if acked_now.contains(&key) => {
                        message_ids_to_ack.push(msg.message_id.clone());
                        continue;
                    }
                    // Decided earlier this tick from another host and neither stored nor credited: it
                    // stays here for the next tick, not decided twice.
                    Ok(Some(false)) if decided_now.contains(&key) => continue,
                    Ok(Some(false)) => {}
                    Err(e) => {
                        // Not a verdict: look again next tick rather than decide twice.
                        warn!("TaskCheckPeerPay: could not read whether {}… was handled: {}", short(&msg.message_id, 16), e);
                        continue;
                    }
                }

                let payment = Payment { message_id: &msg.message_id, sender: &msg.sender, body: Some(&msg.body), restored: None, held: false, from_backup: false, contested: false };
                let outcome = decide(state, &master_privkey, &payment).await;
                decided_now.insert(key);
                decided_ids.insert(msg.message_id.clone());
                match outcome {
                    Outcome::Credited => {
                        processed_count += 1;
                        message_ids_to_ack.push(msg.message_id.clone());
                    }
                    Outcome::Duplicate => {
                        info!("TaskCheckPeerPay: message {}… on {} is a payment already credited — acknowledged", short(&msg.message_id, 16), inbox.host());
                        message_ids_to_ack.push(msg.message_id.clone());
                    }
                    Outcome::Reject(why) | Outcome::Final(why) => {
                        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
                        record_rejected_message(db.connection(), &msg.sender, &msg.message_id, &why);
                        message_ids_to_ack.push(msg.message_id.clone());
                    }
                    Outcome::Hold { reason, wait, prefix, suffix, txid } => {
                        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
                        let conn = db.connection();
                        // The caps turn a hold into today's reject — never a payment we could credit.
                        let over = if msg.body.len() > HELD_BODY_MAX {
                            Some(format!("message body {} bytes is over the {}-byte hold limit", msg.body.len(), HELD_BODY_MAX))
                        } else {
                            match PeerPayRepository::held_counts(conn, &msg.sender) {
                                // Owner, 2026-10-12 (re-decided with the scenario: an app paying many small
                                // PeerPays during our outage): over a sender's limit too, a TRANSIENT hold
                                // stays in the inbox and a lasting one is rejected — like the total.
                                Ok((s, _)) if s >= HELD_PER_SENDER_MAX => Some(format!("sender already has {} held messages", s)),
                                Ok((_, t)) if t >= HELD_TOTAL_MAX => Some(format!("{} messages are already held", t)),
                                Ok(_) => None,
                                Err(e) => {
                                    warn!("TaskCheckPeerPay: could not count held messages: {} — {}… stays in the inbox", e, short(&msg.message_id, 16));
                                    continue;
                                }
                            }
                        };
                        if let Some(why) = over {
                            if is_lasting(reason) {
                                record_rejected_message(conn, &msg.sender, &msg.message_id, &format!("{} ({})", why, reason));
                                message_ids_to_ack.push(msg.message_id.clone());
                            } else {
                                warn!("TaskCheckPeerPay: message {} ({}) not stored ({}); left in the inbox", short(&msg.message_id, 16), reason, why);
                            }
                            continue;
                        }
                        let (lock_time, next_check_at) = schedule(wait, 1, now);
                        let row = crate::database::HeldMessage {
                            message_id: msg.message_id.clone(),
                            sender: msg.sender.clone(),
                            body: Some(msg.body.clone()),
                            derivation_prefix: prefix,
                            derivation_suffix: suffix,
                            txid,
                            reason: reason.to_string(),
                            held_at: now,
                            lock_time,
                            next_check_at,
                            attempts: 1,
                        };
                        match PeerPayRepository::save_held(conn, &row) {
                            Ok(()) => {
                                // Review 164-5 #12: just decided — not due again at the next tick.
                                let _ = PeerPayRepository::mark_held_checked(conn, &msg.message_id, now);
                                info!("TaskCheckPeerPay: message {}… held ({}) — stored, then acknowledged", short(&msg.message_id, 16), reason);
                                message_ids_to_ack.push(msg.message_id.clone());
                            }
                            // Not stored ⇒ not acknowledged: it stays in the inbox for the next tick.
                            Err(e) => warn!("TaskCheckPeerPay: could not store message {}…: {} — not acknowledged", short(&msg.message_id, 16), e),
                        }
                    }
                    Outcome::Retry => {}
                }
            }
            // Review 178-6 #2: an id this host listed with more than one body is not carried to other
            // hosts as "acknowledged" — the body left behind may be the real payment.
            let mut bodies_per_id: std::collections::HashMap<&String, usize> = std::collections::HashMap::new();
            for m in &messages {
                *bodies_per_id.entry(&m.message_id).or_default() += 1;
            }
            let acked_ids: std::collections::HashSet<&String> = message_ids_to_ack.iter().collect();
            acked_now.extend(messages.iter()
                .filter(|m| acked_ids.contains(&m.message_id) && bodies_per_id.get(&m.message_id) == Some(&1))
                .map(|m| body_key(&m.message_id, &m.body)));
            if !message_ids_to_ack.is_empty() {
                let acked = match tokio::time::timeout(ACK_BUDGET, inbox.acknowledge(&message_ids_to_ack)).await {
                    Ok(r) => r,
                    Err(_) => Err(format!("no answer within {} s", ACK_BUDGET.as_secs())),
                };
                if let Err(e) = acked {
                    // Non-fatal: every acknowledged message is credited or stored, so the next
                    // tick finds it known and acknowledges it again.
                    warn!("TaskCheckPeerPay: failed to acknowledge messages on {}: {}", inbox.host(), e);
                }
            }
        }
        Err(e) => warn!("TaskCheckPeerPay: MessageBox API error on {}: {}", inbox.host(), e),
    }
    }

    // Owner, 2026-10-12: a held row that names no derivation (no prefix or suffix) can never lead to a
    // coin — nothing in it recovers one. Such junk is deleted after 7 days, so it cannot fill the
    // table for good. A row that names a derivation is never deleted this way.
    {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        match PeerPayRepository::expire_junk(db.connection(), now - JUNK_TTL_SECS) {
            Ok(n) if n > 0 => info!("TaskCheckPeerPay: {} held message(s) with no derivation expired after 7 days", n),
            Ok(_) => {}
            Err(e) => warn!("TaskCheckPeerPay: could not expire held junk: {}", e),
        }
    }

    // 2. Held rows that are due.
    let tip = state.chain_tracker.current_height();
    let mtp = state.chain_tracker.tip_median_time_past();
    let due = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        PeerPayRepository::held_due(db.connection(), now, started_at).map_err(|e| format!("held rows: {}", e))?
    };
    let mut rechecked = 0usize;
    for row in due {
        if decided_ids.contains(&row.message_id) {
            continue;
        }
        if let (Some(lock), None) = (row.lock_time, row.next_check_at) {
            if !crate::spv_checks::lock_passed(lock.clamp(0, u32::MAX as i64) as u32, tip, mtp) {
                continue; // its lock has not passed at our tip
            }
        }
        if rechecked == RECHECKS_PER_TICK {
            break; // the rest are still due next tick
        }
        rechecked += 1;
        // V6: `held_due` reads no bodies; only the row being decided loads one.
        let body = {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            match PeerPayRepository::held_body(db.connection(), &row.message_id) {
                Ok(b) => b,
                Err(e) => {
                    warn!("TaskCheckPeerPay: could not read held row {}: {}", short(&row.message_id, 16), e);
                    continue;
                }
            }
        };
        let restored = match (&body, &row.derivation_prefix, &row.derivation_suffix, &row.txid) {
            (None, Some(p), Some(s), Some(t)) => Some((p.clone(), s.clone(), t.clone())),
            _ => None,
        };
        let from_backup = restored.is_some() || row.reason == "restored";
        let payment = Payment { message_id: &row.message_id, sender: &row.sender, body: body.as_deref(), restored, held: true, from_backup, contested: false };
        let outcome = match decide(state, &master_privkey, &payment).await {
            // Review 163 #2: a row restored without its message is judged on a BEEF our own code
            // built from our providers — a refusal of THAT is not the sender's, and the row is the only
            // copy of the derivation. Held, never deleted.
            Outcome::Reject(why) if from_backup => {
                warn!("TaskCheckPeerPay: restored row {} refused on our own evidence ({}) — held", short(&row.message_id, 16), why);
                Outcome::Hold { reason: "restored", wait: Wait::Hourly, prefix: row.derivation_prefix.clone(),
                    suffix: row.derivation_suffix.clone(), txid: row.txid.clone() }
            }
            o => o,
        };
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let conn = db.connection();
        // Review 164 #8: a transient local failure (Retry) is not a look at the row.
        if !matches!(outcome, Outcome::Retry) {
            if let Err(e) = PeerPayRepository::mark_held_checked(conn, &row.message_id, now) {
                warn!("TaskCheckPeerPay: could not stamp held row {}: {}", short(&row.message_id, 16), e);
            }
        }
        match outcome {
            Outcome::Credited => processed_count += 1, // the credit removed the held row
            Outcome::Duplicate => {
                info!("TaskCheckPeerPay: held row {}… is a payment already credited — removed", short(&row.message_id, 16));
                if let Err(e) = PeerPayRepository::delete_held(conn, &row.message_id) {
                    warn!("TaskCheckPeerPay: could not remove duplicate held row {}…: {}", short(&row.message_id, 16), e);
                }
            }
            Outcome::Reject(why) | Outcome::Final(why) => {
                record_rejected_message(conn, &row.sender, &row.message_id, &why);
                if let Err(e) = PeerPayRepository::delete_held(conn, &row.message_id) {
                    warn!("TaskCheckPeerPay: could not remove rejected held row {}…: {}", short(&row.message_id, 16), e);
                }
            }
            Outcome::Hold { reason, wait, prefix, suffix, txid } => {
                // Review 163 #11: the backoff counts this reason's own run — a first outage after many
                // 60 s header checks waits 60 s, not an hour.
                // Review 164-5 #6: reasons that share a schedule and alarm share one run, so flapping
                // between them neither keeps the backoff at 60 s nor hides the alarm.
                fn group(r: &str) -> &str {
                    match r {
                        "chain_check_error" | "broadcast_failed" | "restored" => "chain",
                        "headers_unavailable" | "recent_bad_proof" => "headers",
                        other => other,
                    }
                }
                let attempts = if group(&row.reason) == group(reason) { row.attempts + 1 } else { 1 };
                // Owner, 2026-10-12: a payment every broadcaster refuses is tried hourly for a day, then
                // rejected like any invalid payment.
                if reason == "broadcast_refused" && attempts > REFUSED_TRIES && !from_backup {
                    record_rejected_message(conn, &row.sender, &row.message_id, "refused by every broadcaster for 24 hours");
                    if let Err(e) = PeerPayRepository::delete_held(conn, &row.message_id) {
                        warn!("TaskCheckPeerPay: could not remove refused held row {}: {}", short(&row.message_id, 16), e);
                    }
                    continue;
                }
                let (lock_time, next_check_at) = schedule(wait, attempts, now);
                let updated = crate::database::HeldMessage {
                    derivation_prefix: prefix, derivation_suffix: suffix, txid,
                    reason: reason.to_string(), lock_time, next_check_at, attempts, ..row
                };
                if let Err(e) = PeerPayRepository::save_held(conn, &updated) {
                    warn!("TaskCheckPeerPay: could not update held row {}…: {}", short(&updated.message_id, 16), e);
                }
            }
            Outcome::Retry => {}
        }
    }

    if processed_count > 0 {
        info!("📬 TaskCheckPeerPay: accepted {} incoming payment(s)", processed_count);
        super::log_monitor_event(
            state,
            "TaskCheckPeerPay:accepted",
            Some(&format!("{} payments", processed_count)),
        );
    }

    Ok(())
}

/// `(lock_time, next_check_at)` for a held row on its `attempts`-th hold (K-a: the first hold is 1).
pub(crate) fn schedule(wait: Wait, attempts: i64, now: i64) -> (Option<i64>, Option<i64>) {
    match wait {
        Wait::Start => (None, None),
        Wait::Lock(l) => (Some(l as i64), None),
        Wait::Flat => (None, Some(now + 60)),
        Wait::Hourly => (None, Some(now + 3600)),
        Wait::Backoff => {
            let shift = (attempts.max(1) - 1).min(6) as u32;
            (None, Some(now + (60i64 << shift).min(3600)))
        }
    }
}

/// The display txid of a payment's subject, when its bytes parse as a BEEF — so a held row carries
/// the txid a restored copy re-checks by.
fn subject_txid_of(tx_bytes: &[u8]) -> Option<String> {
    parse_payment_beef(tx_bytes).ok().map(|(txid, _)| txid)
}

/// The Atomic BEEF a held row restored from a backup (derivation + txid only) is re-checked with: the
/// subject and its direct parents, each fetched by txid and hash-checked — then `resolve_brc29_credit`
/// checks it like any message. V3: no proof of our own is attached — a wrong one from our provider
/// would read as a settled mismatch and delete the only copy of the derivation; the parents suffice.
async fn restored_beef(state: &web::Data<AppState>, txid: &str) -> Result<Vec<u8>, String> {
    let subject = crate::beef_helpers::fetch_transaction_for_beef(txid, &state.database, &state.services).await?.bytes;
    let parsed = crate::beef::ParsedTransaction::from_bytes(&subject)?;
    let parents: std::collections::BTreeSet<&str> = parsed.inputs.iter().map(|i| i.prev_txid.as_str()).collect();
    if parents.len() > MAX_RESTORED_PARENTS {
        return Err(format!("{} parents to fetch — more than {}", parents.len(), MAX_RESTORED_PARENTS));
    }
    let mut beef = crate::beef::Beef::new();
    for prev in &parents {
        let parent = crate::beef_helpers::fetch_transaction_for_beef(prev, &state.database, &state.services).await?.bytes;
        beef.add_parent_transaction(parent);
    }
    beef.set_main_transaction(subject);
    hex::decode(beef.to_atomic_beef_hex(txid)?).map_err(|e| e.to_string())
}

/// Review 163 #9: a body still in its BRC-2 envelope (decryption failed when it was listed) is
/// decrypted again from the stored copy, so a later release that can read it does.
fn decrypt_stored(master_privkey: &[u8], sender: &str, body: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(body).ok()?;
    if !text.contains("encryptedMessage") {
        return None;
    }
    crate::messagebox::decrypt_body(master_privkey, sender, text).ok()
}

/// Decide one message: credit it (store the output), or say why it is held or rejected.
async fn decide(state: &web::Data<AppState>, master_privkey: &[u8], p: &Payment<'_>) -> Outcome {
    let id = short(p.message_id, 16);
    let hold = |reason: &'static str, wait: Wait, prefix: Option<String>, suffix: Option<String>, txid: Option<String>| {
        Outcome::Hold { reason, wait, prefix, suffix, txid }
    };

    // 1. The token: from the message, or (a restored row) rebuilt from the chain by txid.
    let (prefix, suffix, tx_bytes, amount) = match (p.body, &p.restored) {
        (Some(body), _) => {
            // Review 163 #9: a body still in its envelope parses as JSON with no token fields, so the
            // decryption is tried first, not as a parse fallback.
            let decrypted = decrypt_stored(master_privkey, p.sender, body);
            let body = decrypted.as_deref().unwrap_or(body);
            let token = match parse_payment_token(body) {
                Ok(t) => t,
                Err(e) => {
                    warn!("TaskCheckPeerPay: message {}… is not a payment token ({}): {}", id, e,
                        short(&String::from_utf8_lossy(body), 500));
                    return hold("unreadable", Wait::Start, None, None, None);
                }
            };
            let prefix = token.custom_instructions.as_ref().and_then(|ci| ci.derivation_prefix.clone());
            let suffix = token.custom_instructions.as_ref().and_then(|ci| ci.derivation_suffix.clone());
            match (prefix, suffix, token.transaction_bytes) {
                (Some(pf), Some(sf), Some(tx)) => (pf, sf, tx, token.amount),
                (pf, sf, tx) => {
                    warn!("TaskCheckPeerPay: message {}… lacks a derivation prefix, suffix or transaction", id);
                    let txid = tx.as_deref().and_then(subject_txid_of);
                    return hold("missing_fields", Wait::Start, pf, sf, txid);
                }
            }
        }
        (None, Some((pf, sf, txid))) => match restored_beef(state, txid).await {
            Ok(bytes) => (pf.clone(), sf.clone(), bytes, None),
            Err(e) => {
                warn!("TaskCheckPeerPay: restored message {}…: could not fetch {} ({})", id, short(txid, 16), e);
                return hold("restored", Wait::Hourly, Some(pf.clone()), Some(sf.clone()), Some(txid.clone()));
            }
        },
        (None, None) => return hold("unreadable", Wait::Start, None, None, None),
    };
    let held = |reason, wait| Outcome::Hold { reason, wait, prefix: Some(prefix.clone()), suffix: Some(suffix.clone()), txid: subject_txid_of(&tx_bytes) };

    // 2. Our key for this payment. A sender key or derivation that is not valid can never be: reject.
    let sender_pubkey = match hex::decode(p.sender) {
        Ok(b) if b.len() == 33 => b,
        _ => return Outcome::Final(format!("sender key {}… is not a 33-byte public key", short(p.sender, 16))),
    };
    let invoice_number = format!("2-3241645161d8-{} {}", prefix, suffix);
    let child_pubkey = match crate::crypto::brc42::derive_child_private_key(master_privkey, &sender_pubkey, &invoice_number)
        .map_err(|e| format!("{:?}", e))
        .and_then(|k| secp256k1::SecretKey::from_slice(&k).map_err(|e| e.to_string()))
    {
        Ok(sk) => secp256k1::PublicKey::from_secret_key(&secp256k1::Secp256k1::new(), &sk).serialize().to_vec(),
        Err(e) => return Outcome::Final(format!("BRC-42 derivation failed: {}", e)),
    };

    // 3. The credit, checked against our own headers (pure).
    let credit = match resolve_brc29_credit(&tx_bytes, &child_pubkey, amount, &state.chain_tracker) {
        Ok(c) => c,
        Err(reason) => {
            return match reason.decision(state.chain_tracker.current_height()) {
                Decision::Hold { reason: r, wait } => {
                    info!("TaskCheckPeerPay: message {}… held: {}", id, reason.description());
                    held(r, wait)
                }
                Decision::Reject => Outcome::Reject(reason.description()),
            };
        }
    };
    let subject_txid = credit.subject_txid.clone();

    // B5-T5-P7a: the same payment arriving again — from a second MessageBox server, or re-sent in a
    // new message — is credited once. The key is the credited output (txid:vout, same value and
    // script), not the message id, which differs between copies.
    {
        let db = match state.database.lock() {
            Ok(db) => db,
            Err(e) => {
                warn!("TaskCheckPeerPay: DB lock: {}", e);
                return Outcome::Retry;
            }
        };
        // Review 178-3 #9: stored but never recorded (the process stopped between the two) is not a
        // duplicate — the credit runs on and records it (`store_derived_utxo` is a no-op for it).
        match crate::handlers::derived_output_already_stored(db.connection(), &subject_txid, credit.vout, credit.satoshis, &credit.script)
            .and_then(|stored| Ok(stored && PeerPayRepository::has_receive_for_txid(db.connection(), &subject_txid).map_err(|e| e.to_string())?))
        {
            Ok(true) => return Outcome::Duplicate,
            Ok(false) => {}
            // Not a verdict: look again next tick (rule 7, trip-wire 2).
            Err(e) => {
                warn!("TaskCheckPeerPay: could not read whether {}:{} is stored: {}", short(&subject_txid, 16), credit.vout, e);
                return Outcome::Retry;
            }
        }
    }

    // 4. SECURITY: the transaction exists on chain before it is stored as spendable (phantom UTXOs
    // from fabricated or never-broadcast BEEF). Not on chain: we broadcast the sender's BEEF. Any
    // failure here is OURS or the network's, never the payment's: held and retried, never dropped.
    match crate::handlers::check_tx_exists_on_chain(&state.services, &subject_txid).await {
        Ok(true) => info!("   ✅ PeerPay tx {} verified on-chain", short(&subject_txid, 16)),
        Ok(false) => {
            info!("   📡 PeerPay tx {} NOT on-chain, attempting broadcast...", short(&subject_txid, 16));
            // B5-T1-P7b: the sender's BEEF, subject last, so ARC gets the parents.
            let broadcast_hex = match credit.beef.broadcast_v1_for(&subject_txid) {
                Ok(v1) => hex::encode(v1),
                Err(e) => {
                    warn!("   ⚠️ Could not build the broadcast BEEF ({}) — broadcasting the raw transaction", e);
                    hex::encode(&credit.credited_tx_bytes)
                }
            };
            match crate::handlers::broadcast_transaction_classified(&broadcast_hex, &state.services, Some(crate::handlers::ProofSink::of(&state)), Some(&subject_txid)).await {
                Ok(m) => info!("   ✅ PeerPay tx broadcast succeeded: {}", m),
                // Every provider that answered refused the transaction itself (double spend, missing
                // inputs): rebroadcasting cannot help. Kept, and looked at again at wallet start (if
                // it was mined meanwhile, the chain check finds it then).
                Err(f) if f.permanent => {
                    warn!("   ⚠️  PeerPay tx refused by the network: {} — held, looked at again in an hour", f.reason);
                    return held("broadcast_refused", Wait::Hourly);
                }
                Err(f) => {
                    warn!("   ⚠️  PeerPay tx broadcast failed: {} — held, retried later", f.reason);
                    return held("broadcast_failed", Wait::Backoff);
                }
            }
        }
        Err(e) => {
            warn!("   ⚠️  Chain check failed for {}: {} — held, retried later", short(&subject_txid, 16), e);
            return held("chain_check_error", Wait::Backoff);
        }
    }

    // V4: a held row may come from a backup older than a spend of this very coin (credited and spent
    // on the old device). The chain says whether our output is spent; spent ⇒ not ours to credit now.
    if p.restored.is_some() || p.from_backup {
        match state.services.outspend(&subject_txid, credit.vout).await {
            Ok(crate::services::OutspendStatus::Spent { spending_txid, .. }) =>
                return Outcome::Final(format!("{}:{} is already spent by {}", short(&subject_txid, 16), credit.vout, short(&spending_txid, 16))),
            Ok(crate::services::OutspendStatus::Unspent) => {}
            // Review 164 #6: no answer is not "unspent" — held and asked again.
            Err(e) => {
                warn!("   ⚠️  Could not tell whether {}:{} is spent ({}) — held", short(&subject_txid, 16), credit.vout, e);
                return held("chain_check_error", Wait::Backoff);
            }
        }
    }

    // 5. Store the output, record it, drop any held copy.
    info!("📬 TaskCheckPeerPay: accepting payment {} sats from {}...", credit.satoshis, short(p.sender, 16));
    let db = match state.database.lock() {
        Ok(db) => db,
        Err(e) => {
            warn!("TaskCheckPeerPay: DB lock: {}", e);
            return Outcome::Retry;
        }
    };
    let custom_instructions = serde_json::json!({
        "type": "brc29_payment",
        "senderIdentityKey": p.sender,
        "derivationPrefix": prefix,
        "derivationSuffix": suffix
    });
    if let Err(e) = crate::handlers::store_derived_utxo(
        &db, &subject_txid, credit.vout, credit.satoshis, &hex::encode(&credit.script), &child_pubkey,
        &custom_instructions, &credit.credited_tx_bytes,
    ) {
        error!("   ❌ Failed to store PeerPay output: {}", e);
        // beta.3 Phase 10a (`P10a-A3`): "already exists with a different value or script" is a
        // rejection; anything else (DB busy, …) is retried next tick.
        return if e.contains("refusing to overwrite") { Outcome::Final(e) } else { Outcome::Retry };
    }
    info!("   💾 Stored PeerPay output {}:{} ({} sats)", subject_txid, credit.vout, credit.satoshis);

    // Snapshot current BSV/USD price for historical display
    let price_usd_cents = state.price_cache.get_cached()
        .or_else(|| state.price_cache.get_stale())
        .map(|p| (p * 100.0) as i64);
    let received_id = if p.contested { format!("{}:{}", p.message_id, subject_txid) } else { p.message_id.to_string() };
    if let Err(e) = PeerPayRepository::insert_received(
        db.connection(), &received_id, p.sender, credit.satoshis, &prefix, &suffix, Some(&subject_txid), "peerpay", price_usd_cents,
    ) {
        error!("   ❌ Failed to record peerpay_received: {}", e);
    }
    let removed = if p.contested {
        PeerPayRepository::delete_held_if_same(db.connection(), p.message_id, &subject_txid)
    } else {
        PeerPayRepository::delete_held(db.connection(), p.message_id)
    };
    if let Err(e) = removed {
        warn!("   Failed to remove the held copy of {}…: {}", id, e);
    }

    // Cache BEEF ancestry data for future BEEF building: the parents the sender included, which we
    // would otherwise have to fetch from WhatsOnChain during a send.
    {
        let parent_tx_repo = ParentTransactionRepository::new(db.connection());
        use sha2::{Sha256, Digest};
        for tx_raw in &credit.beef.transactions {
            let tx_txid: Vec<u8> = Sha256::digest(Sha256::digest(tx_raw)).into_iter().rev().collect();
            let tx_txid_hex = hex::encode(&tx_txid);
            match parent_tx_repo.upsert(None, &tx_txid_hex, &hex::encode(tx_raw)) {
                Ok(_) => debug!("   💾 Cached BEEF tx {} in parent_transactions", short(&tx_txid_hex, 16)),
                Err(e) => warn!("   ⚠️  Failed to cache BEEF tx: {}", e),
            }
        }
        info!("   📦 Cached {} BEEF transaction(s) for future BEEF building", credit.beef.transactions.len());
    }
    drop(db);

    state.balance_cache.invalidate();
    // Request backup check if received amount is significant (> $3 USD)
    state.request_backup_check_if_significant(credit.satoshis);
    Outcome::Credited
}

// ============================================================================
// beta.3 Phase 10a — CU-3. The envelope's declared subject txid is what the
// poller checks on chain, so the credited value MUST be read from the
// transaction that hashes to it, never from "the last one in the bundle".
// Every envelope below is hand-built; the subject hash is computed here,
// independently of beef.rs.
// ============================================================================
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sha2::{Sha256, Digest};
    use ripemd::Ripemd160;

    fn push_varint(v: &mut Vec<u8>, n: u64) {
        if n < 0xfd { v.push(n as u8); } else { v.push(0xfd); v.extend(&(n as u16).to_le_bytes()); }
    }

    /// Minimal raw transaction: version 1, one dummy input, the given outputs, locktime 0.
    fn raw_tx(prev_txid_byte: u8, outputs: &[(i64, Vec<u8>)]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend(&1u32.to_le_bytes());
        push_varint(&mut v, 1);
        v.extend(&[prev_txid_byte; 32]);
        v.extend(&0u32.to_le_bytes());
        push_varint(&mut v, 0);
        v.extend(&0xffff_ffffu32.to_le_bytes());
        push_varint(&mut v, outputs.len() as u64);
        for (value, script) in outputs {
            v.extend(&value.to_le_bytes());
            push_varint(&mut v, script.len() as u64);
            v.extend(script);
        }
        v.extend(&0u32.to_le_bytes());
        v
    }

    /// Display-format txid, computed here so the test does not trust beef.rs's hashing.
    fn txid_hex(tx: &[u8]) -> String {
        let h = Sha256::digest(&Sha256::digest(tx));
        hex::encode(h.iter().rev().copied().collect::<Vec<u8>>())
    }

    fn p2pkh(pubkey: &[u8]) -> Vec<u8> {
        let h = Ripemd160::digest(&Sha256::digest(pubkey));
        let mut s = vec![0x76, 0xa9, 0x14];
        s.extend(h.as_slice());
        s.extend(&[0x88, 0xac]);
        s
    }

    fn our_key() -> Vec<u8> {
        let secp = secp256k1::Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&[0x11u8; 32]).unwrap();
        secp256k1::PublicKey::from_secret_key(&secp, &sk).serialize().to_vec()
    }

    /// A P2PKH script to a key that is not ours (the script only hashes the bytes).
    fn stranger_script() -> Vec<u8> { p2pkh(&[0x02u8; 33]) }

    /// Atomic envelope whose header names `subject`; bundle = `parents` then `last`.
    fn envelope(subject: &str, parents: &[Vec<u8>], last: Vec<u8>) -> Vec<u8> {
        let mut beef = crate::beef::Beef::new();
        for p in parents { beef.add_parent_transaction(p.clone()); }
        beef.set_main_transaction(last);
        hex::decode(beef.to_atomic_beef_hex(subject).unwrap()).unwrap()
    }

    /// The wallet's header chain at the compiled checkpoint (not synced): final payments need no more.
    fn tracker() -> crate::chain_tracker::ChainTracker {
        crate::chain_tracker::ChainTracker::from_chain(crate::header_chain::mainnet_header_chain().unwrap())
    }

    /// A 1-BSV parent and a subject spending it (B5-T1-P7a-1, BRC-67 check 2: the BEEF must carry
    /// the parent and the inputs must be worth more than the outputs). Returns (parent, subject).
    fn funded(outputs: &[(i64, Vec<u8>)]) -> (Vec<u8>, Vec<u8>) {
        let parent = raw_tx(0xf0, &[(100_000_000, vec![0x51])]);
        let mut subject = raw_tx(0x00, outputs);
        subject[5..37].copy_from_slice(&Sha256::digest(Sha256::digest(&parent)));
        (parent, subject)
    }

    fn summary(r: &Result<Brc29Credit, CreditReject>) -> String {
        match r {
            Ok(c) => format!("CREDITED {} sats at {}:{}", c.satoshis, c.subject_txid, c.vout),
            Err(e) => format!("rejected: {:?}", e),
        }
    }

    /// `P10a-A1` — the attack. Header names A (a real, mined transaction that
    /// pays nobody we own); the last transaction is fabricated B paying us
    /// 1,000,000 sats. The only correct answer is a rejection: A is the
    /// subject and A pays us nothing.
    #[test]
    fn a1_credit_is_read_from_the_subject_transaction_not_the_last_one() {
        let a = raw_tx(0xaa, &[(1, stranger_script())]);
        let b = raw_tx(0xbb, &[(1_000_000, p2pkh(&our_key()))]);
        let env = envelope(&txid_hex(&a), &[a.clone()], b);

        let r = resolve_brc29_credit(&env, &our_key(), None, &tracker());

        // Post-fix the credit is read from A, and A has no output for our key.
        assert!(matches!(r, Err(CreditReject::NoMatchingOutput)), "subject A pays us nothing, yet: {}", summary(&r));
    }

    /// `P10a-A1` (second shape) — the header names a txid that is in no
    /// transaction of the bundle at all.
    #[test]
    fn a1_subject_absent_from_the_bundle_is_rejected() {
        let a = raw_tx(0xaa, &[(1, stranger_script())]);
        let b = raw_tx(0xbb, &[(1_000_000, p2pkh(&our_key()))]);
        let c = raw_tx(0xcc, &[(5, stranger_script())]);
        let env = envelope(&txid_hex(&c), &[a], b);

        let r = resolve_brc29_credit(&env, &our_key(), None, &tracker());

        assert!(matches!(&r, Err(CreditReject::NotAtomicBeef(e)) if e.contains("not a transaction in the bundle")),
            "subject C is not in the bundle, yet: {}", summary(&r));
    }

    /// `P10a-A2` — one byte after the last transaction.
    #[test]
    fn a2_trailing_byte_after_the_bundle_is_rejected() {
        let b = raw_tx(0xbb, &[(700, p2pkh(&our_key()))]);
        let mut env = envelope(&txid_hex(&b), &[], b);
        env.push(0x00);

        let r = resolve_brc29_credit(&env, &our_key(), None, &tracker());

        assert!(matches!(&r, Err(CreditReject::NotAtomicBeef(e)) if e.contains("trailing byte")),
            "trailing byte accepted: {}", summary(&r));
    }

    /// The two-sided control for A1, and the unit half of `P10a-A5`: a genuine
    /// envelope (subject = the paying transaction) is credited with that
    /// transaction's value and vout, exactly once.
    #[test]
    fn genuine_envelope_is_credited_with_the_subject_value() {
        let a = raw_tx(0xaa, &[(1, stranger_script())]);
        let (f, b) = funded(&[(5, stranger_script()), (700, p2pkh(&our_key()))]);
        let env = envelope(&txid_hex(&b), &[a, f], b.clone());

        let c = resolve_brc29_credit(&env, &our_key(), Some(700), &tracker()).expect("genuine credit");

        assert_eq!(c.subject_txid, txid_hex(&b));
        assert_eq!((c.vout, c.satoshis), (1, 700));
        assert_eq!(c.credited_tx_bytes, b, "the broadcast bytes must be the subject's");
    }

    /// `P10a-A5`'s RED half — the message's own `amount` disagrees with the
    /// output it points at.
    #[test]
    fn a5_declared_amount_mismatch_is_rejected() {
        let (f, b) = funded(&[(700, p2pkh(&our_key()))]);
        let env = envelope(&txid_hex(&b), &[f], b);

        let r = resolve_brc29_credit(&env, &our_key(), Some(701), &tracker());
        assert!(matches!(r, Err(CreditReject::AmountMismatch { declared: 701, found: 700 })), "{}", summary(&r));

        // 0 / absent = "not declared", not "declared zero".
        assert!(resolve_brc29_credit(&env, &our_key(), Some(0), &tracker()).is_ok());
        assert!(resolve_brc29_credit(&env, &our_key(), None, &tracker()).is_ok());
    }

    /// `P7a-B17`: PeerPay refuses what `internalizeAction` refuses (same shared checks), with the
    /// exact `Refusal` — 1,000 in / 1,001 out, a missing parent, a far-future lock.
    #[test]
    fn b17_peerpay_runs_brc67_checks_2_and_4() {
        let parent = raw_tx(0xf0, &[(1_000, vec![0x51])]);
        let mut over = raw_tx(0x00, &[(1_001, p2pkh(&our_key()))]);
        over[5..37].copy_from_slice(&Sha256::digest(Sha256::digest(&parent)));
        let env = envelope(&txid_hex(&over), &[parent], over);
        let r = resolve_brc29_credit(&env, &our_key(), None, &tracker());
        assert!(matches!(r, Err(CreditReject::Spv(crate::spv_checks::Refusal::Fee { inputs: 1_000, outputs: 1_001 }))), "{}", summary(&r));

        let (_, orphan) = funded(&[(700, p2pkh(&our_key()))]);
        let env = envelope(&txid_hex(&orphan), &[], orphan);
        let r = resolve_brc29_credit(&env, &our_key(), None, &tracker());
        assert!(matches!(r, Err(CreditReject::Spv(crate::spv_checks::Refusal::MissingParent { .. }))), "{}", summary(&r));
    }

    /// `P7a-B19` (owner decision 5), B5-T1-P7b form: a payment that is not final yet is HELD until its
    /// lock — stored, then acknowledged — not dropped.
    #[test]
    fn b19_a_not_yet_final_payment_is_held_until_its_lock() {
        let (f, mut b) = funded(&[(700, p2pkh(&our_key()))]);
        let n = b.len();
        b[n - 4..].copy_from_slice(&(980_000u32).to_le_bytes()); // a height lock far above any tip here
        b[42..46].copy_from_slice(&0u32.to_le_bytes()); // sequence 0: version 4 + count 1 + prevout 36 + empty script 1
        let env = envelope(&txid_hex(&b), &[f], b);
        let t = tracker();
        t.mark_synced_for_test();
        let r = resolve_brc29_credit(&env, &our_key(), None, &t);
        assert!(matches!(&r, Err(CreditReject::Spv(crate::spv_checks::Refusal::NonFinal { lock_time: 980_000 }))), "{}", summary(&r));
        assert_eq!(r.err().map(|e| e.decision(t.current_height())), Some(Decision::Hold { reason: "non_final", wait: Wait::Lock(980_000) }));
        assert_eq!(CreditReject::NoMatchingOutput.decision(970_000), Decision::Reject);
        assert_eq!(CreditReject::Spv(crate::spv_checks::Refusal::Fee { inputs: 1, outputs: 1 }).decision(970_000), Decision::Reject);
        assert_eq!(CreditReject::Spv(crate::spv_checks::Refusal::HeadersUnavailable { lock_time: 5 }).decision(970_000),
            Decision::Hold { reason: "headers_unavailable", wait: Wait::Flat });
    }

    // ---------- `P7a-C2`: PeerPay checks the BEEF's proofs against our own headers ----------
    //
    // The subject pays our key and is the only transaction of a synthetic block (regtest params), so
    // its BUMP is `[[txid]]` and the block's merkle root IS its txid.

    pub(super) fn bump_beef(subject: &[u8], height: u32) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        let txid_natural = Sha256::digest(Sha256::digest(subject));
        let mut node = vec![0x00, 0x02]; // offset 0, txid flag
        node.extend_from_slice(&txid_natural);
        let bump = crate::beef::MerkleProof { block_height: height, tree_height: 1, levels: vec![vec![node]] };
        let beef = crate::beef::Beef { version: crate::beef::BEEF_V1_MARKER, bumps: vec![bump], transactions: vec![subject.to_vec()], tx_to_bump: vec![Some(0)] };
        hex::decode(beef.to_atomic_beef_hex(&txid_hex(subject)).unwrap()).unwrap()
    }

    /// A regtest chain whose block 101 has `root_of_101` as its merkle root, then `more` blocks.
    pub(crate) fn chain_with_block(root_of_101: [u8; 32], more: u32) -> crate::chain_tracker::ChainTracker {
        use crate::header_chain::{check_proof_of_work, work_for_bits, ChainParams, Checkpoint, Header, HeaderChain};
        let limit = ChainParams::regtest().pow_limit;
        let mine = |prev: [u8; 32], time: u32, root: [u8; 32]| {
            let mut h = Header { version: 0x2000_0000, prev_hash: prev, merkle_root: root, time, bits: 0x207fffff, nonce: 0 };
            while check_proof_of_work(&h.hash(), h.bits, &limit).is_err() { h.nonce += 1; }
            h
        };
        let g = mine([0; 32], 1_000_000, [0xaa; 32]);
        let cp = Checkpoint { height: 100, hash: g.hash(), chainwork: work_for_bits(g.bits) };
        let mut c = HeaderChain::from_anchor(ChainParams::regtest(), cp, &[g]).unwrap();
        let mut prev = mine(g.hash(), 1_000_600, root_of_101);
        c.add_header(prev, u32::MAX).unwrap();
        for i in 0..more {
            let h = mine(prev.hash(), 1_001_200 + i * 600, [i as u8; 32]);
            c.add_header(h, u32::MAX).unwrap();
            prev = h;
        }
        crate::chain_tracker::ChainTracker::from_chain(c)
    }

    pub(crate) fn txid_natural(raw: &[u8]) -> [u8; 32] {
        Sha256::digest(Sha256::digest(raw)).into()
    }

    #[test]
    fn c2_a_subject_proven_against_our_headers_needs_no_parents() {
        let (_, b) = funded(&[(700, p2pkh(&our_key()))]);
        let t = chain_with_block(txid_natural(&b), 0);
        let r = resolve_brc29_credit(&bump_beef(&b, 101), &our_key(), None, &t);
        assert!(r.is_ok(), "{}", summary(&r));
    }

    #[test]
    fn c2_a_proof_that_does_not_match_our_header_is_refused_and_dropped_once_settled() {
        let (_, b) = funded(&[(700, p2pkh(&our_key()))]);
        let t = chain_with_block([0x99; 32], 10); // block 101's root is something else; tip 111
        let r = resolve_brc29_credit(&bump_beef(&b, 101), &our_key(), None, &t);
        assert!(matches!(&r, Err(CreditReject::Spv(crate::spv_checks::Refusal::BadProof { height: 101, .. }))), "{}", summary(&r));
        assert_eq!(r.err().map(|e| e.decision(t.current_height())), Some(Decision::Reject));
    }

    #[test]
    fn c2_a_mismatch_near_our_tip_is_kept_for_retry() {
        let (_, b) = funded(&[(700, p2pkh(&our_key()))]);
        let t = chain_with_block([0x99; 32], 2); // tip 103: block 101 is within the recent window
        let r = resolve_brc29_credit(&bump_beef(&b, 101), &our_key(), None, &t);
        assert_eq!(r.err().map(|e| e.decision(t.current_height())), Some(Decision::Hold { reason: "recent_bad_proof", wait: Wait::Flat }));
    }

    /// B5-T1-P7b (V9, owner 2026-10-11): a proof for a block we do not hold yet, with no parents in
    /// the BEEF (lawful for a proven transaction), is our headers' question — held flat until they
    /// catch up — not a missing-parent verdict re-checked only at the next wallet start.
    #[test]
    fn c2_a_proof_far_above_our_tip_is_a_missing_parent() {
        // Review 164 #3: a height we will not hold soon (beyond a day of blocks above a synced tip)
        // is not "our headers are behind": it stays a missing parent.
        let (_, b) = funded(&[(700, p2pkh(&our_key()))]);
        let t = chain_with_block(txid_natural(&b), 0);
        t.mark_synced_for_test();
        let r = resolve_brc29_credit(&bump_beef(&b, 101 + UNHELD_PROOF_WINDOW + 1), &our_key(), None, &t);
        assert!(matches!(&r, Err(CreditReject::Spv(crate::spv_checks::Refusal::MissingParent { .. }))), "{}", summary(&r));
    }

    #[test]
    fn c2_a_block_we_do_not_hold_waits_for_our_headers() {
        let (_, b) = funded(&[(700, p2pkh(&our_key()))]);
        let t = chain_with_block(txid_natural(&b), 0);
        let r = resolve_brc29_credit(&bump_beef(&b, 150), &our_key(), None, &t);
        assert!(matches!(&r, Err(CreditReject::Spv(crate::spv_checks::Refusal::HeadersUnavailable { .. }))), "{}", summary(&r));
        assert_eq!(r.err().map(|e| e.decision(t.current_height())), Some(Decision::Hold { reason: "headers_unavailable", wait: Wait::Flat }));
    }
}

// ============================================================================
// beta.3 Phase 10a — `P10a-A3`: a receive never rewrites an existing outputs
// row. Lives here (bin crate) because `store_derived_utxo` is in `handlers`,
// which the lib test target does not compile. Uses the real WalletDatabase on
// a temp file because that is the type the function takes.
// ============================================================================
#[cfg(test)]
mod store_derived_utxo_tests {
    fn wallet_db() -> crate::database::WalletDatabase {
        let path = std::env::temp_dir().join(format!("hodos_p10a_a3_{}.db", uuid::Uuid::new_v4()));
        let db = crate::database::WalletDatabase::new(path).unwrap();
        db.connection().execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at)
             VALUES (1, 'test_identity_key', 'local', 0, 0)",
            [],
        ).unwrap();
        db
    }

    /// Everything a receive could change, read back by value.
    fn derived_row(db: &crate::database::WalletDatabase, txid: &str) -> (i64, Vec<u8>, String, String, i64, Option<String>) {
        db.connection().query_row(
            "SELECT satoshis, locking_script, sender_identity_key, derivation_suffix, spendable, custom_instructions
             FROM outputs WHERE txid = ?1 AND vout = 0",
            [txid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        ).unwrap()
    }

    #[test]
    fn a3_receive_for_an_existing_output_never_rewrites_the_row() {
        let db = wallet_db();
        let txid = "aa".repeat(32);
        let script = "76a914".to_string() + &"11".repeat(20) + "88ac";
        let first = serde_json::json!({
            "type": "brc29_payment", "senderIdentityKey": "02".to_string() + &"aa".repeat(32),
            "derivationPrefix": "p1", "derivationSuffix": "s1"
        });
        crate::handlers::store_derived_utxo(&db, &txid, 0, 500, &script, &[], &first, &[]).unwrap();
        // The coin has since been spent (or reserved): spendable = 0.
        db.connection().execute("UPDATE outputs SET spendable = 0 WHERE txid = ?1", [&txid]).unwrap();
        let before = derived_row(&db, &txid);

        // A second message names the same txid:vout with a different value,
        // sender and derivation — the CU-3 overwrite.
        let attacker = serde_json::json!({
            "type": "brc29_payment", "senderIdentityKey": "02".to_string() + &"bb".repeat(32),
            "derivationPrefix": "p2", "derivationSuffix": "s2"
        });
        let r = crate::handlers::store_derived_utxo(&db, &txid, 0, 1_000_000, &script, &[], &attacker, &[]);

        assert!(r.is_err(), "overwrite accepted: {:?}", r);
        assert_eq!(derived_row(&db, &txid), before, "row changed by a refused receive");

        // Byte-identical re-delivery (same value + script) is a harmless no-op.
        assert!(crate::handlers::store_derived_utxo(&db, &txid, 0, 500, &script, &[], &attacker, &[]).is_ok());
        assert_eq!(derived_row(&db, &txid), before);
    }
}

#[cfg(test)]
#[path = "task_check_peerpay_p7b_tests.rs"]
pub(crate) mod p7b_poller_tests;

#[cfg(test)]
#[path = "task_check_peerpay_p7b_held_tests.rs"]
mod p7b_held_tests;

#[cfg(test)]
#[path = "task_check_peerpay_p7a_tests.rs"]
mod p7a_cross_server_tests;
