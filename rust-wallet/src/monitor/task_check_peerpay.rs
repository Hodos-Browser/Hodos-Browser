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
use crate::messagebox::MessageBoxClient;
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

/// Why a message is not credited. Every arm is acknowledged (dropped) by the
/// poller — none of these is a transient condition worth a retry.
#[derive(Debug)]
pub(crate) enum CreditReject {
    NotAtomicBeef(String),
    NoMainTransaction,
    TxParse(String),
    NoMatchingOutput,
    /// The message's own `amount` field disagrees with the output it points at.
    AmountMismatch { declared: i64, found: i64 },
}

/// Resolve the credit for one message from its Atomic BEEF bytes, the BRC-42
/// child public key we derived for it, and the amount the message declares
/// (`None` / `0` = not declared). Pure — no DB, no network.
pub(crate) fn resolve_brc29_credit(
    tx_bytes: &[u8],
    child_pubkey: &[u8],
    declared_amount: Option<i64>,
) -> Result<Brc29Credit, CreditReject> {
    use sha2::{Sha256, Digest};
    use ripemd::Ripemd160;

    // Strict parser: rejects trailing bytes and a subject that hashes to no
    // transaction in the bundle (CU-3). The credit is read from the SUBJECT
    // transaction — the one the on-chain check is about — never from the last.
    let (subject_txid, beef) = crate::beef::Beef::from_atomic_beef_bytes(tx_bytes)
        .map_err(CreditReject::NotAtomicBeef)?;

    let main_tx_bytes = beef.subject_transaction(&subject_txid)
        .cloned()
        .ok_or(CreditReject::NoMainTransaction)?;

    let parsed_tx = crate::beef::ParsedTransaction::from_bytes(&main_tx_bytes)
        .map_err(CreditReject::TxParse)?;

    // Expected P2PKH script from the derived pubkey
    let sha_hash = Sha256::digest(child_pubkey);
    let pubkey_hash = Ripemd160::digest(&sha_hash);

    for (i, output) in parsed_tx.outputs.iter().enumerate() {
        // P2PKH: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
        if output.script.len() == 25
            && output.script[0] == 0x76
            && output.script[1] == 0xa9
            && output.script[2] == 0x14
            && output.script[23] == 0x88
            && output.script[24] == 0xac
            && &output.script[3..23] == pubkey_hash.as_slice()
        {
            if let Some(declared) = declared_amount.filter(|a| *a > 0) {
                if declared != output.value {
                    return Err(CreditReject::AmountMismatch { declared, found: output.value });
                }
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

    Err(CreditReject::NoMatchingOutput)
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
    let sender_prefix = &sender[..16.min(sender.len())];
    warn!("🚫 PeerPay REJECTED — sender {}… message {}…: {}",
        sender_prefix, &message_id[..16.min(message_id.len())], why);

    match PeerPayRepository::insert_rejected_notification(conn, sender) {
        Ok(true) => info!("   🔔 Rejected-payment notification recorded for sender {}…", sender_prefix),
        Ok(false) => debug!("   Rejected-payment notice for sender {}… is already on screen", sender_prefix),
        Err(e) => warn!("   Failed to record rejected-payment notification: {}", e),
    }
}

/// Run the TaskCheckPeerPay task
pub async fn run(state: &web::Data<AppState>, _client: &reqwest::Client) -> Result<(), String> {
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
    let mb_client = MessageBoxClient::new(master_privkey.clone(), master_pubkey.clone());

    // List messages from payment_inbox (authenticated + decrypted)
    let messages = match mb_client.list_messages("payment_inbox").await {
        Ok(msgs) => {
            info!("📬 TaskCheckPeerPay: polled payment_inbox — {} message(s)", msgs.len());
            msgs
        }
        Err(e) => {
            warn!("TaskCheckPeerPay: MessageBox API error: {}", e);
            return Ok(()); // Retry next tick
        }
    };

    if messages.is_empty() {
        return Ok(());
    }

    debug!("TaskCheckPeerPay: found {} message(s) in payment_inbox", messages.len());

    let mut processed_count = 0;
    let mut message_ids_to_ack: Vec<String> = Vec::new();

    for msg in &messages {
        // Check if already processed (deduplication)
        {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            if PeerPayRepository::is_already_processed(db.connection(), &msg.message_id)
                .unwrap_or(false)
            {
                debug!("TaskCheckPeerPay: message {} already processed, skipping", &msg.message_id[..16.min(msg.message_id.len())]);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        }

        // Parse the decrypted payment token (flexible: handles string or array for transaction)
        let token: PaymentToken = match parse_payment_token(&msg.body) {
            Ok(t) => t,
            Err(e) => {
                warn!("TaskCheckPeerPay: failed to parse payment token from {}: {}", &msg.message_id[..16.min(msg.message_id.len())], e);
                // Log the raw JSON so we can debug the format
                if let Ok(raw) = String::from_utf8(msg.body.clone()) {
                    warn!("TaskCheckPeerPay: raw token (first 500 chars): {}", &raw[..500.min(raw.len())]);
                }
                // Do NOT acknowledge — retry next tick so the payment isn't lost
                continue;
            }
        };

        let instructions = match token.custom_instructions {
            Some(ref ci) => ci,
            None => {
                warn!("TaskCheckPeerPay: message {} has no customInstructions", &msg.message_id[..16.min(msg.message_id.len())]);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        let prefix = match &instructions.derivation_prefix {
            Some(p) => p.clone(),
            None => {
                warn!("TaskCheckPeerPay: missing derivationPrefix");
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        let suffix = match &instructions.derivation_suffix {
            Some(s) => s.clone(),
            None => {
                warn!("TaskCheckPeerPay: missing derivationSuffix");
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        let tx_bytes = match token.transaction_bytes {
            Some(b) => b,
            None => {
                warn!("TaskCheckPeerPay: missing transaction data");
                // Don't acknowledge — might be a partial message
                continue;
            }
        };

        // Decode sender pubkey
        let sender_pubkey = match hex::decode(&msg.sender) {
            Ok(b) if b.len() == 33 => b,
            _ => {
                warn!("TaskCheckPeerPay: invalid sender key: {}", &msg.sender[..16.min(msg.sender.len())]);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        // BRC-29 invoice number: "2-3241645161d8-{prefix} {suffix}"
        let invoice_number = format!("2-3241645161d8-{} {}", prefix, suffix);

        // Derive our child private key using BRC-42 (recipient perspective)
        let child_privkey = match crate::crypto::brc42::derive_child_private_key(
            &master_privkey,
            &sender_pubkey,
            &invoice_number,
        ) {
            Ok(k) => k,
            Err(e) => {
                warn!("TaskCheckPeerPay: BRC-42 derivation failed: {:?}", e);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        // Get derived public key
        let secp = secp256k1::Secp256k1::new();
        let child_secret = match secp256k1::SecretKey::from_slice(&child_privkey) {
            Ok(s) => s,
            Err(e) => {
                warn!("TaskCheckPeerPay: invalid derived key: {}", e);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };
        let child_pubkey = secp256k1::PublicKey::from_secret_key(&secp, &child_secret).serialize().to_vec();

        // Parse the envelope and find our output (pure — see resolve_brc29_credit)
        let credit = match resolve_brc29_credit(&tx_bytes, &child_pubkey, token.amount) {
            Ok(c) => c,
            Err(reason) => {
                let why = match &reason {
                    CreditReject::NotAtomicBeef(e) => format!("not a valid Atomic BEEF: {}", e),
                    CreditReject::NoMainTransaction => "BEEF has no main transaction".to_string(),
                    CreditReject::TxParse(e) => format!("failed to parse transaction: {}", e),
                    CreditReject::NoMatchingOutput => "no output pays the key derived for this message".to_string(),
                    CreditReject::AmountMismatch { declared, found } => format!(
                        "message declares {} sats but the output pays {}", declared, found),
                };
                {
                    let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
                    record_rejected_message(db.connection(), &msg.sender, &msg.message_id, &why);
                }
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };
        let subject_txid = credit.subject_txid.clone();
        let found_vout = credit.vout;
        let found_satoshis = credit.satoshis;
        let main_tx_bytes = credit.credited_tx_bytes.clone();
        let beef = credit.beef;

        // ========================================================================
        // SECURITY: Verify transaction exists on-chain before storing as spendable.
        // This prevents phantom UTXOs from fabricated or never-broadcast BEEF.
        // Mirrors the same check in internalize_action (handlers.rs:10397-10435).
        // ========================================================================

        // 4a. Check retry count (brief DB lock, then drop before async calls)
        const MAX_PEERPAY_RETRIES: i32 = 10; // ~10 minutes at 60s intervals

        let retry_info = {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            PeerPayRepository::get_pending_retry_count(db.connection(), &msg.message_id)
                .unwrap_or(None)
        };

        if let Some((count, _first_seen)) = retry_info {
            if count >= MAX_PEERPAY_RETRIES {
                warn!("TaskCheckPeerPay: message {} exceeded {} retries for txid {}, giving up — invalid payment",
                    &msg.message_id[..16.min(msg.message_id.len())], MAX_PEERPAY_RETRIES, &subject_txid[..16]);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        }

        // 4b. Verify on-chain (async — no DB lock held)
        let tx_verified = match crate::handlers::check_tx_exists_on_chain(&state.services, &subject_txid).await {
            Ok(true) => {
                info!("   ✅ PeerPay tx {} verified on-chain", &subject_txid[..16]);
                true
            }
            Ok(false) => {
                info!("   📡 PeerPay tx {} NOT on-chain, attempting broadcast...", &subject_txid[..16]);
                let raw_tx_hex = hex::encode(&main_tx_bytes);
                match crate::handlers::broadcast_transaction(
                    &raw_tx_hex, &state.services, Some(&state.database), Some(&subject_txid)
                ).await {
                    Ok(broadcast_msg) => {
                        info!("   ✅ PeerPay tx broadcast succeeded: {}", broadcast_msg);
                        true
                    }
                    Err(e) => {
                        warn!("   ⚠️  PeerPay tx broadcast failed: {} — will retry next tick", e);
                        false
                    }
                }
            }
            Err(e) => {
                warn!("   ⚠️  Chain check failed for {}: {} — will retry next tick", &subject_txid[..16], e);
                false
            }
        };

        // 4c. Handle verification failure
        if !tx_verified {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            if let Err(e) = PeerPayRepository::upsert_pending_verification(
                db.connection(), &msg.message_id, &subject_txid
            ) {
                warn!("   Failed to track pending verification: {}", e);
            }
            // Do NOT acknowledge — retry next tick
            continue;
        }

        info!("📬 TaskCheckPeerPay: accepting payment {} sats from {}...",
            found_satoshis, &msg.sender[..16.min(msg.sender.len())]);

        // Store as spendable output in our wallet
        {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;

            let custom_instructions = serde_json::json!({
                "type": "brc29_payment",
                "senderIdentityKey": msg.sender,
                "derivationPrefix": prefix,
                "derivationSuffix": suffix
            });

            match crate::handlers::store_derived_utxo(
                &db,
                &subject_txid,
                found_vout,
                found_satoshis,
                &hex::encode(&credit.script),
                &child_pubkey,
                &custom_instructions,
            ) {
                Ok(_) => {
                    info!("   💾 Stored PeerPay output {}:{} ({} sats)", subject_txid, found_vout, found_satoshis);
                    // Clean up pending verification record if it existed
                    let _ = PeerPayRepository::remove_pending_verification(db.connection(), &msg.message_id);
                }
                Err(e) => {
                    // beta.3 Phase 10a (`P10a-A3`) — this now includes "already
                    // exists with a different value or script", which is a
                    // rejection to record and acknowledge, not a transient error
                    // to retry every tick forever.
                    error!("   ❌ Failed to store PeerPay output: {}", e);
                    if e.contains("refusing to overwrite") {
                        record_rejected_message(db.connection(), &msg.sender, &msg.message_id, &e);
                        message_ids_to_ack.push(msg.message_id.clone());
                    }
                    // Anything else (DB busy, etc.): don't acknowledge — retry next tick
                    continue;
                }
            }

            // Snapshot current BSV/USD price for historical display
            let price_usd_cents = state.price_cache.get_cached()
                .or_else(|| state.price_cache.get_stale())
                .map(|p| (p * 100.0) as i64);

            // Record in peerpay_received for notification tracking
            if let Err(e) = PeerPayRepository::insert_received(
                db.connection(),
                &msg.message_id,
                &msg.sender,
                found_satoshis,
                &prefix,
                &suffix,
                Some(&subject_txid),
                "peerpay",
                price_usd_cents,
            ) {
                error!("   ❌ Failed to record peerpay_received: {}", e);
            }

            // Cache BEEF ancestry data for future BEEF building.
            // The Atomic BEEF contains parent tx raw bytes that we'd otherwise
            // have to fetch from WhatsOnChain API during send — cache them now.
            {
                let parent_tx_repo = ParentTransactionRepository::new(db.connection());

                // Cache all transactions from the BEEF (parents + main tx)
                use sha2::{Sha256, Digest};
                for tx_raw in &beef.transactions {
                    let hash1 = Sha256::digest(tx_raw);
                    let hash2 = Sha256::digest(&hash1);
                    let tx_txid: Vec<u8> = hash2.into_iter().rev().collect();
                    let tx_txid_hex = hex::encode(&tx_txid);
                    let raw_hex = hex::encode(tx_raw);

                    match parent_tx_repo.upsert(None, &tx_txid_hex, &raw_hex) {
                        Ok(_) => debug!("   💾 Cached BEEF tx {} in parent_transactions", &tx_txid_hex[..16]),
                        Err(e) => warn!("   ⚠️  Failed to cache BEEF tx: {}", e),
                    }
                }
                info!("   📦 Cached {} BEEF transaction(s) for future BEEF building", beef.transactions.len());
            }
        }

        // Invalidate balance cache
        state.balance_cache.invalidate();

        // Request backup check if received amount is significant (> $3 USD)
        state.request_backup_check_if_significant(found_satoshis);

        processed_count += 1;
        message_ids_to_ack.push(msg.message_id.clone());
    }

    // Acknowledge all processed messages on the MessageBox server
    if !message_ids_to_ack.is_empty() {
        if let Err(e) = mb_client.acknowledge(&message_ids_to_ack).await {
            warn!("TaskCheckPeerPay: failed to acknowledge messages: {}", e);
            // Non-fatal — messages will be re-fetched and deduplicated next tick
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

// ============================================================================
// beta.3 Phase 10a — CU-3. The envelope's declared subject txid is what the
// poller checks on chain, so the credited value MUST be read from the
// transaction that hashes to it, never from "the last one in the bundle".
// Every envelope below is hand-built; the subject hash is computed here,
// independently of beef.rs.
// ============================================================================
#[cfg(test)]
mod tests {
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

        let r = resolve_brc29_credit(&env, &our_key(), None);

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

        let r = resolve_brc29_credit(&env, &our_key(), None);

        assert!(matches!(&r, Err(CreditReject::NotAtomicBeef(e)) if e.contains("not a transaction in the bundle")),
            "subject C is not in the bundle, yet: {}", summary(&r));
    }

    /// `P10a-A2` — one byte after the last transaction.
    #[test]
    fn a2_trailing_byte_after_the_bundle_is_rejected() {
        let b = raw_tx(0xbb, &[(700, p2pkh(&our_key()))]);
        let mut env = envelope(&txid_hex(&b), &[], b);
        env.push(0x00);

        let r = resolve_brc29_credit(&env, &our_key(), None);

        assert!(matches!(&r, Err(CreditReject::NotAtomicBeef(e)) if e.contains("trailing byte")),
            "trailing byte accepted: {}", summary(&r));
    }

    /// The two-sided control for A1, and the unit half of `P10a-A5`: a genuine
    /// envelope (subject = the paying transaction) is credited with that
    /// transaction's value and vout, exactly once.
    #[test]
    fn genuine_envelope_is_credited_with_the_subject_value() {
        let a = raw_tx(0xaa, &[(1, stranger_script())]);
        let b = raw_tx(0xbb, &[(5, stranger_script()), (700, p2pkh(&our_key()))]);
        let env = envelope(&txid_hex(&b), &[a], b.clone());

        let c = resolve_brc29_credit(&env, &our_key(), Some(700)).expect("genuine credit");

        assert_eq!(c.subject_txid, txid_hex(&b));
        assert_eq!((c.vout, c.satoshis), (1, 700));
        assert_eq!(c.credited_tx_bytes, b, "the broadcast bytes must be the subject's");
    }

    /// `P10a-A5`'s RED half — the message's own `amount` disagrees with the
    /// output it points at.
    #[test]
    fn a5_declared_amount_mismatch_is_rejected() {
        let b = raw_tx(0xbb, &[(700, p2pkh(&our_key()))]);
        let env = envelope(&txid_hex(&b), &[], b);

        let r = resolve_brc29_credit(&env, &our_key(), Some(701));
        assert!(matches!(r, Err(CreditReject::AmountMismatch { declared: 701, found: 700 })), "{}", summary(&r));

        // 0 / absent = "not declared", not "declared zero".
        assert!(resolve_brc29_credit(&env, &our_key(), Some(0)).is_ok());
        assert!(resolve_brc29_credit(&env, &our_key(), None).is_ok());
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
        crate::handlers::store_derived_utxo(&db, &txid, 0, 500, &script, &[], &first).unwrap();
        // The coin has since been spent (or reserved): spendable = 0.
        db.connection().execute("UPDATE outputs SET spendable = 0 WHERE txid = ?1", [&txid]).unwrap();
        let before = derived_row(&db, &txid);

        // A second message names the same txid:vout with a different value,
        // sender and derivation — the CU-3 overwrite.
        let attacker = serde_json::json!({
            "type": "brc29_payment", "senderIdentityKey": "02".to_string() + &"bb".repeat(32),
            "derivationPrefix": "p2", "derivationSuffix": "s2"
        });
        let r = crate::handlers::store_derived_utxo(&db, &txid, 0, 1_000_000, &script, &[], &attacker);

        assert!(r.is_err(), "overwrite accepted: {:?}", r);
        assert_eq!(derived_row(&db, &txid), before, "row changed by a refused receive");

        // Byte-identical re-delivery (same value + script) is a harmless no-op.
        assert!(crate::handlers::store_derived_utxo(&db, &txid, 0, 500, &script, &[], &attacker).is_ok());
        assert_eq!(derived_row(&db, &txid), before);
    }
}
