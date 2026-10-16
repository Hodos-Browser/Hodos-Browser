//! TaskRetryPeerPayOutbox — Retry MessageBox delivery for failed PeerPay sends
//!
//! When a PeerPay send succeeds on-chain but MessageBox delivery fails,
//! the payload is queued in peerpay_outbox. This task retries delivery
//! on an escalating schedule:
//!   - First 10 retries (~10 min): every 60s
//!   - Next 10 retries (~20 min): every 120s
//!   - After 20 retries (~30 min total): mark as 'exhausted' (user can manually retry)
//!
//! Interval: 30 seconds (fast tick — actual retry governed by next_retry_at timestamps)

use actix_web::web;
use log::{info, warn, error, debug};

use crate::AppState;
use crate::messagebox::{DeliveryVerdict, Fallback, MessageBoxClient, PreparedMessage};
use crate::database::peerpay_repo::OUTBOX_MAX_ATTEMPTS;
use crate::database::PeerPayRepository;

/// Run the TaskRetryPeerPayOutbox task
pub async fn run(state: &web::Data<AppState>, _client: &reqwest::Client) -> Result<(), String> {
    // 1. Get wallet master keys (brief DB lock)
    let (master_privkey, master_pubkey) = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let privkey = match crate::database::get_master_private_key_from_db(&db) {
            Ok(k) => k,
            Err(_) => {
                debug!("TaskRetryPeerPayOutbox: no wallet yet, skipping");
                return Ok(());
            }
        };
        let pubkey = match crate::database::get_master_public_key_from_db(&db) {
            Ok(k) => k,
            Err(_) => {
                debug!("TaskRetryPeerPayOutbox: can't get public key, skipping");
                return Ok(());
            }
        };
        (privkey, pubkey)
    };

    // 2. Get due outbox entries (brief DB lock)
    let entries = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        // beta.3 Phase 10d (`P10d-A5`): every payment whose recipient was not told
        // ('exhausted' retries, or 'undeliverable') gets ONE dismissable notification
        // row — that row, not the outbox, drives the yellow header dot, so Dismiss
        // clears it. Idempotent (INSERT OR IGNORE on `undeliverable:{txid}`); also
        // covers rows that went 'exhausted' before this build existed.
        if let Err(e) = PeerPayRepository::backfill_undeliverable_notifications(db.connection()) {
            warn!("TaskRetryPeerPayOutbox: notification backfill failed: {}", e);
        }
        PeerPayRepository::get_due_outbox_entries(db.connection())
            .map_err(|e| format!("Failed to query outbox: {}", e))?
    };

    if entries.is_empty() {
        return Ok(());
    }

    info!("📤 TaskRetryPeerPayOutbox: {} outbox entry(s) due for retry", entries.len());

    // 3. Build MessageBox client once (all entries use the same wallet keys)
    let mb_client = MessageBoxClient::new(master_privkey, master_pubkey);

    // 4. Retry the due entries
    // Review 178-5 #13: entries are retried at once (each within its own budgets), so one dead host
    // does not hold the Monitor loop for every pending payment in turn; at most `ENTRIES_PER_TICK`.
    let results = futures_util::future::join_all(
        entries.iter().take(ENTRIES_PER_TICK).map(|entry| retry_entry(state, &mb_client, entry))
    ).await;
    let mut delivered_count = 0;
    let mut failed_count = 0;
    for r in results {
        match r? {
            Some(true) => delivered_count += 1,
            Some(false) => failed_count += 1,
            None => {}
        }
    }

    if delivered_count > 0 || failed_count > 0 {
        info!("📤 TaskRetryPeerPayOutbox: {} delivered, {} failed", delivered_count, failed_count);
        super::log_monitor_event(
            state,
            "TaskRetryPeerPayOutbox:completed",
            Some(&format!("{} delivered, {} failed", delivered_count, failed_count)),
        );
    }

    Ok(())
}

/// Most outbox entries retried in one tick (review 178-5 #13); the rest are still due next tick.
const ENTRIES_PER_TICK: usize = 10;

/// One outbox entry's attempt: `Some(true)` delivered, `Some(false)` not (retried or undeliverable),
/// `None` skipped.
async fn retry_entry(state: &web::Data<AppState>, mb_client: &MessageBoxClient, entry: &crate::database::peerpay_repo::OutboxEntry)
    -> Result<Option<bool>, String>
{
    let recipient_pubkey = match hex::decode(&entry.recipient_pubkey_hex) {
        Ok(b) if b.len() == 33 => b,
        _ => {
            // Review 178-6 #7: never deliverable — undeliverable (shown), not skipped: with at most
            // `ENTRIES_PER_TICK` per tick, a skipped row would stay at the head of the queue for good.
            warn!("TaskRetryPeerPayOutbox: invalid recipient key for txid {} — undeliverable", &entry.txid[..16.min(entry.txid.len())]);
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            if let Err(db_err) = PeerPayRepository::mark_outbox_undeliverable(db.connection(), entry.id) {
                error!("   ❌ Failed to mark outbox entry undeliverable: {}", db_err);
            }
            return Ok(Some(false));
        }
    };

    // B5-T5-P7a: looked up again on every attempt — the recipient may have advertised since — and
    // the SAME message (V31 `message_request`) posted only to the hosts that have not taken it.
    // A row from before V31 has no stored request: prepared now, and stored with the result.
    let prepared = match prepared_for(&mb_client, &recipient_pubkey, entry) {
        Ok(p) => std::sync::Arc::new(p),
        Err(e) => {
            warn!("   ⚠️  Outbox txid {}: message could not be built: {}", &entry.txid[..16.min(entry.txid.len())], e);
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            if let Err(db_err) = PeerPayRepository::update_outbox_retry_failed(db.connection(), entry.id, entry.retry_count) {
                error!("   ❌ Failed to update outbox retry: {}", db_err);
            }
            return Ok(Some(false));
        }
    };
    // Review 178-4 #1: from the third attempt (~2 min), an advertised host that fails in any way
    // (gone, unreachable) no longer holds the payment back from the known hosts.
    let fallback = if entry.retry_count >= 2 { Fallback::OnAnyFailure } else { Fallback::OnRefusal };
    let delivery = mb_client.deliver(&recipient_pubkey, prepared.clone(), &entry.delivered_hosts, fallback).await;
    info!("   Outbox txid {} (attempt {}): {}", &entry.txid[..16.min(entry.txid.len())], entry.retry_count + 1, delivery.summary());
    {
        // Review 178-4 #13: written only when something changed — the request once, the hosts when new.
        let taken = hosts_taken(&entry.delivered_hosts, &delivery.accepted);
        // Review 178-5 #9: a request prepared again (none stored, or the stored one unreadable) is
        // stored, so the next attempt re-posts this same message id.
        let request = (entry.message_request.as_deref() != Some(prepared.request_body())).then(|| prepared.request_body());
        if taken != entry.delivered_hosts || request.is_some() {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            if let Err(db_err) = PeerPayRepository::record_outbox_delivery(db.connection(), entry.id, &taken, request) {
                error!("   ❌ Failed to record which hosts took txid {}: {}", &entry.txid[..16.min(entry.txid.len())], db_err);
            }
        }
    }
    let verdict = attempt_verdict(&delivery, entry.retry_count, &entry.delivered_hosts);
    if verdict != delivery.verdict() {
        warn!("   Outbox txid {}: last attempt — recorded as {:?} ({})",
            &entry.txid[..16.min(entry.txid.len())], verdict, delivery.summary());
    }
    match verdict {
        DeliveryVerdict::Delivered => {
            info!("   ✅ Outbox retry succeeded for txid {} (attempt {})",
                &entry.txid[..16.min(entry.txid.len())], entry.retry_count + 1);

            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            let _ = PeerPayRepository::mark_outbox_delivered(db.connection(), entry.id);
            return Ok(Some(true));
        }
        verdict => {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            // beta.3 Phase 10d (`P10d-A4`): a refusal (4xx) is permanent for
            // these bytes — one attempt, then `undeliverable` + the yellow
            // notification. Only transient failures keep the retry schedule.
            let result = if verdict == DeliveryVerdict::Undeliverable {
                warn!("   🚫 Relay refused txid {} — marking undeliverable (no further retries)",
                    &entry.txid[..16.min(entry.txid.len())]);
                PeerPayRepository::mark_outbox_undeliverable(db.connection(), entry.id)
            } else {
                PeerPayRepository::update_outbox_retry_failed(db.connection(), entry.id, entry.retry_count)
            };
            if let Err(db_err) = result {
                error!("   ❌ Failed to update outbox retry: {}", db_err);
            }
            Ok(Some(false))
        }
    }
}

/// B5-T5-P7a: the message an outbox row re-posts — its stored request (V31), else (a row from before
/// V31, or a stored request that does not parse — review 178-2 #11) prepared again from the payload.
fn prepared_for(client: &MessageBoxClient, recipient_pubkey: &[u8], entry: &crate::database::peerpay_repo::OutboxEntry)
    -> Result<PreparedMessage, crate::messagebox::MessageBoxError>
{
    if let Some(r) = entry.message_request.clone() {
        match PreparedMessage::from_request(r) {
            Ok(p) => return Ok(p),
            Err(e) => warn!("   Outbox txid {}: stored request unreadable ({}) — prepared again", &entry.txid[..16.min(entry.txid.len())], e),
        }
    }
    client.prepare_message(recipient_pubkey, "payment_inbox", &entry.payload_bytes)
}

/// B5-T5-P7a (review 178-2 #3): this attempt's verdict. On the last attempt, a message some host has
/// taken is delivered — the recipient's own server may well be among them — never "recipient was
/// not told".
fn attempt_verdict(delivery: &crate::messagebox::Delivery, retry_count: i32, previous: &[String]) -> DeliveryVerdict {
    let v = delivery.verdict();
    let last = v == DeliveryVerdict::Retry && retry_count + 1 >= OUTBOX_MAX_ATTEMPTS;
    if last && !hosts_taken(previous, &delivery.accepted).is_empty() {
        DeliveryVerdict::Delivered
    } else if last && delivery.failed.iter().any(|(_, e)| e.is_permanent()) {
        // Review 178-5 #8: nobody took it and a host refused it for good — undeliverable (its cause
        // and claim block shown), not "exhausted".
        DeliveryVerdict::Undeliverable
    } else {
        v
    }
}

/// B5-T5-P7a: every host that has taken the message so far — the earlier ones are kept even when this
/// attempt's lookup no longer names them, so they are never posted to again.
fn hosts_taken(previous: &[String], accepted: &[String]) -> Vec<String> {
    let mut taken = previous.to_vec();
    taken.extend(accepted.iter().filter(|h| !previous.contains(h)).cloned());
    taken
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `T5P7a-A14` — the hosts that took the message accumulate across attempts.
    #[test]
    fn t5p7a_a14_hosts_taken_accumulate() {
        let s = |v: &[&str]| v.iter().map(|h| h.to_string()).collect::<Vec<_>>();
        assert_eq!(hosts_taken(&s(&["a", "b"]), &s(&["x"])), s(&["a", "b", "x"]), "earlier hosts kept");
        assert_eq!(hosts_taken(&s(&["a"]), &s(&["a", "c"])), s(&["a", "c"]), "no host twice");
        assert_eq!(hosts_taken(&[], &[]), Vec::<String>::new());
    }

    fn delivery(accepted: &[&str], transient: bool) -> crate::messagebox::Delivery {
        crate::messagebox::Delivery {
            lookup_failed: false,
            fallback_due: false,
            accepted: accepted.iter().map(|h| h.to_string()).collect(),
            failed: if transient { vec![("down".into(), crate::messagebox::MessageBoxError::Api("503".into()))] } else { vec![] },
        }
    }

    /// `T5P7a-A15` (review 178-2 #3) — one host that stays down does not end a payment other hosts
    /// took as "recipient was not told": the last attempt records it delivered. Before the last
    /// attempt it is retried; a payment no host ever took is still exhausted.
    #[test]
    fn t5p7a_a15_last_attempt_with_some_host_is_delivered() {
        let last = OUTBOX_MAX_ATTEMPTS - 1;
        assert_eq!(attempt_verdict(&delivery(&["a"], true), last, &[]), DeliveryVerdict::Delivered);
        assert_eq!(attempt_verdict(&delivery(&[], true), last, &["a".to_string()]), DeliveryVerdict::Delivered, "taken earlier");
        assert_eq!(attempt_verdict(&delivery(&["a"], true), last - 1, &[]), DeliveryVerdict::Retry, "not the last attempt");
        assert_eq!(attempt_verdict(&delivery(&[], true), last, &[]), DeliveryVerdict::Retry, "no host took it: exhausted as before");
        // Review 178-5 #8: nobody took it and one host refused it for good ⇒ undeliverable.
        let mut d = delivery(&[], true);
        d.failed.push(("b".into(), crate::messagebox::MessageBoxError::Rejected { status: 413, body: "too large".into() }));
        assert_eq!(attempt_verdict(&d, last, &[]), DeliveryVerdict::Undeliverable);
        assert_eq!(attempt_verdict(&d, last - 1, &[]), DeliveryVerdict::Retry, "not before the last attempt");
    }

    /// `T5P7a-A35` (review 178-6 #7) — a row whose recipient key cannot be parsed is marked
    /// undeliverable, not skipped (a skipped row would pin the head of the queue for good).
    #[tokio::test]
    async fn t5p7a_a35_an_invalid_recipient_key_is_undeliverable_not_skipped() {
        use crate::monitor::task_check_peerpay::p7b_poller_tests::{wallet, state_with};
        let (db, path) = wallet();
        crate::database::PeerPayRepository::insert_outbox(db.connection(), "tx-bad", "zz", b"pay", 5, &[], None).unwrap();
        let mut p = crate::handlers::p2_test_support::ScriptedProvider::new("arc_gorillapool");
        p.status_for = Box::new(|_| Err(crate::services::IndexerError::Transport("unused".into())));
        let state = state_with(db, std::sync::Arc::new(p));
        let entry = {
            let conn = rusqlite::Connection::open(&path).unwrap();
            crate::database::PeerPayRepository::get_outbox_entries_by_status(&conn, "pending").unwrap().remove(0)
        };
        let client = MessageBoxClient::new(vec![1u8; 32], vec![2u8; 33]);
        assert_eq!(retry_entry(&state, &client, &entry).await.unwrap(), Some(false));
        let conn = rusqlite::Connection::open(&path).unwrap();
        let status: String = conn.query_row("SELECT status FROM peerpay_outbox WHERE txid='tx-bad'", [], |r| r.get(0)).unwrap();
        assert_eq!(status, "undeliverable");
    }

    /// `T5P7a-A15` (review 178-2 #11) — a stored request that does not parse is prepared again from the
    /// payload, not failed 20 times.
    #[test]
    fn t5p7a_a15_unreadable_stored_request_is_prepared_again() {
        use secp256k1::{Secp256k1, SecretKey, PublicKey};
        let secp = Secp256k1::new();
        let pk = |n: u8| PublicKey::from_secret_key(&secp, &SecretKey::from_slice(&[n; 32]).unwrap()).serialize().to_vec();
        let client = MessageBoxClient::new(vec![1u8; 32], pk(1));
        let entry = crate::database::peerpay_repo::OutboxEntry {
            id: 1, txid: "t".into(), recipient_pubkey_hex: hex::encode(pk(2)), payload_bytes: b"pay".to_vec(),
            amount_satoshis: 1, retry_count: 3, delivered_hosts: vec![], message_request: Some(b"not json".to_vec()),
        };
        assert!(prepared_for(&client, &pk(2), &entry).is_ok());
        let good = client.prepare_message(&pk(2), "payment_inbox", b"pay").unwrap();
        let entry = crate::database::peerpay_repo::OutboxEntry { message_request: Some(good.request_body().to_vec()), ..entry };
        assert_eq!(prepared_for(&client, &pk(2), &entry).unwrap().message_id, good.message_id, "a readable request keeps its id");
    }
}
