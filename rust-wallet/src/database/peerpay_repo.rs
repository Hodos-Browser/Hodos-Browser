//! PeerPay Repository — Persistent storage for received PeerPay payments
//!
//! Replaces the in-memory MessageStore with SQLite-backed tracking.
//! Supports deduplication (message_id uniqueness), undismissed summaries
//! for the notification badge, and dismiss-all for clearing notifications.

use rusqlite::{Connection, Result, params};
use serde::Serialize;

/// A received payment record (PeerPay or address sync)
#[derive(Debug, Clone, Serialize)]
pub struct ReceivedPayment {
    pub id: i64,
    pub message_id: String,
    pub sender_identity_key: String,
    pub amount_satoshis: i64,
    pub derivation_prefix: String,
    pub derivation_suffix: String,
    pub txid: Option<String>,
    pub accepted_at: String,
    pub dismissed: bool,
    pub source: String,
    pub price_usd_cents: Option<i64>,
}

/// An outbox entry for a failed MessageBox delivery (sender side)
#[derive(Debug, Clone)]
pub struct OutboxEntry {
    pub id: i64,
    pub txid: String,
    pub recipient_pubkey_hex: String,
    pub payload_bytes: Vec<u8>,
    pub amount_satoshis: i64,
    pub retry_count: i32,
    /// B5-T5-P7a (V31): the MessageBox hosts that already took this message.
    pub delivered_hosts: Vec<String>,
    /// B5-T5-P7a (V31): the exact request posted, so a retry re-posts the same message id.
    pub message_request: Option<Vec<u8>>,
}

/// `delivered_hosts` as stored. An unreadable value is read as "none": the retry then posts to every
/// target again — duplicates, never a host skipped.
fn parse_delivered_hosts(raw: Option<String>) -> Vec<String> {
    match raw {
        None => vec![],
        Some(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            log::warn!("peerpay_outbox: unreadable delivered_hosts {:?} ({}) — read as none", s, e);
            vec![]
        }),
    }
}

/// Attempts before an outbox row is `exhausted` (`update_outbox_retry_failed`).
pub const OUTBOX_MAX_ATTEMPTS: i32 = 20;

/// Most PeerPay messages held at once (B5-T1-P7b, owner decision 1; also bounds a backup import).
pub const HELD_TOTAL_MAX: i64 = 500;
/// Largest message body ever held (B5-T1-P7b, owner decision 1); also bounds a backup import.
pub const HELD_BODY_MAX: usize = 256 * 1024;

/// A PeerPay message the wallet could not credit yet, stored before it was acknowledged
/// (B5-T1-P7b, schema V30).
#[derive(Debug, Clone, PartialEq)]
pub struct HeldMessage {
    pub message_id: String,
    pub sender: String,
    /// The decrypted message as received; `None` only for a row restored from a backup.
    pub body: Option<Vec<u8>>,
    pub derivation_prefix: Option<String>,
    pub derivation_suffix: Option<String>,
    pub txid: Option<String>,
    pub reason: String,
    pub held_at: i64,
    pub lock_time: Option<i64>,
    pub next_check_at: Option<i64>,
    pub attempts: i64,
}

/// PeerPay database repository
pub struct PeerPayRepository;

impl PeerPayRepository {
    /// Insert a newly received and accepted payment
    pub fn insert_received(
        conn: &Connection,
        message_id: &str,
        sender: &str,
        amount: i64,
        prefix: &str,
        suffix: &str,
        txid: Option<&str>,
        source: &str,
        price_usd_cents: Option<i64>,
    ) -> Result<()> {
        conn.execute(
            "INSERT OR IGNORE INTO peerpay_received (message_id, sender_identity_key, amount_satoshis, derivation_prefix, derivation_suffix, txid, source, price_usd_cents)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![message_id, sender, amount, prefix, suffix, txid, source, price_usd_cents],
        )?;
        Ok(())
    }

    /// Insert a notification for a newly discovered address sync UTXO
    ///
    /// Uses `utxo:{txid}:{vout}` as message_id for natural deduplication.
    /// INSERT OR IGNORE ensures idempotency across repeated syncs.
    pub fn insert_address_sync_notification(
        conn: &Connection,
        txid: &str,
        vout: i64,
        amount: i64,
        price_usd_cents: Option<i64>,
    ) -> Result<()> {
        let message_id = format!("utxo:{}:{}", txid, vout);
        Self::insert_received(conn, &message_id, "unknown", amount, "", "", Some(txid), "address_sync", price_usd_cents)
    }

    /// Check if a message_id has already been processed (deduplication)
    pub fn is_already_processed(conn: &Connection, message_id: &str) -> Result<bool> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM peerpay_received WHERE message_id = ?1",
            params![message_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get all undismissed received payments
    pub fn get_undismissed(conn: &Connection) -> Result<Vec<ReceivedPayment>> {
        let mut stmt = conn.prepare(
            "SELECT id, message_id, sender_identity_key, amount_satoshis, derivation_prefix, derivation_suffix, txid, accepted_at, dismissed, source, price_usd_cents
             FROM peerpay_received
             WHERE dismissed = 0
             ORDER BY id DESC"
        )?;

        let payments = stmt.query_map([], |row| {
            Ok(ReceivedPayment {
                id: row.get(0)?,
                message_id: row.get(1)?,
                sender_identity_key: row.get(2)?,
                amount_satoshis: row.get(3)?,
                derivation_prefix: row.get(4)?,
                derivation_suffix: row.get(5)?,
                txid: row.get(6)?,
                accepted_at: row.get(7)?,
                dismissed: row.get::<_, i32>(8)? != 0,
                source: row.get(9)?,
                price_usd_cents: row.get(10)?,
            })
        })?.collect::<Result<Vec<_>>>()?;

        Ok(payments)
    }

    /// Get summary of undismissed payments: (count, total_satoshis)
    pub fn get_undismissed_summary(conn: &Connection) -> Result<(i64, i64)> {
        let result = conn.query_row(
            "SELECT COALESCE(COUNT(*), 0), COALESCE(SUM(amount_satoshis), 0)
             FROM peerpay_received
             WHERE dismissed = 0",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        Ok(result)
    }

    /// Dismiss all undismissed payments (mark as seen)
    pub fn dismiss_all(conn: &Connection) -> Result<()> {
        conn.execute(
            "UPDATE peerpay_received SET dismissed = 1 WHERE dismissed = 0",
            [],
        )?;
        Ok(())
    }

    /// Insert a failure notification for an unconfirmed UTXO that timed out.
    ///
    /// Uses `fail:{txid}:{vout}` as message_id (distinct from `utxo:{txid}:{vout}`).
    /// notification_type = 'failure' (red dot/banner in frontend).
    pub fn insert_failure_notification(
        conn: &Connection,
        txid: &str,
        vout: i64,
        amount: i64,
        price_usd_cents: Option<i64>,
    ) -> Result<()> {
        let message_id = format!("fail:{}:{}", txid, vout);
        conn.execute(
            "INSERT OR IGNORE INTO peerpay_received (
                message_id, sender_identity_key, amount_satoshis,
                derivation_prefix, derivation_suffix, txid,
                source, notification_type, price_usd_cents
            ) VALUES (?1, 'unknown', ?2, '', '', ?3, 'address_sync', 'failure', ?4)",
            params![message_id, amount, txid, price_usd_cents],
        )?;
        Ok(())
    }

    /// beta.3 Phase 10a (`P10a-A7`, owner decision 2026-09-15) — one quiet
    /// notification when an incoming PeerPay message is rejected as invalid
    /// (fabricated envelope, amount mismatch, overwrite attempt).
    ///
    /// `message_id` is `reject:{sender identity key}`, so `INSERT OR IGNORE`
    /// makes it **one row per sender**, however many fakes that sender posts —
    /// the inbox is writable by anyone who knows the identity key, so a row per
    /// envelope would hand an attacker a badge counter. `notification_type =
    /// 'rejected'`; amount 0 (nothing was credited).
    pub fn insert_rejected_notification(
        conn: &Connection,
        sender_identity_key: &str,
    ) -> Result<bool> {
        let message_id = format!("reject:{}", sender_identity_key);
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO peerpay_received (
                message_id, sender_identity_key, amount_satoshis,
                derivation_prefix, derivation_suffix, txid,
                source, notification_type, price_usd_cents
            ) VALUES (?1, ?2, 0, '', '', NULL, 'peerpay', 'rejected', NULL)",
            params![message_id, sender_identity_key],
        )?;
        if inserted > 0 {
            return Ok(true);
        }
        // beta.3 Phase 10e (panel `F3-10a`) — the row already exists. If the user
        // DISMISSED it, this is a LATER attack from that same sender and they should
        // hear about it again.
        //
        // ⛔ Before this, `INSERT OR IGNORE` alone meant a dismissed sender could
        // never raise a visible notice again for the life of the wallet — a
        // different attack shape from the same key was log-only, forever. Meanwhile
        // a fresh key costs an attacker nothing, so the quiet-by-default design was
        // being paid for by the honest case and not by the hostile one.
        //
        // Semantics now: at most ONE visible notice per sender at a time. Repeated
        // rejections while it is still on screen change nothing. Dismiss silences
        // it. The next rejection after that raises it once more.
        let raised = conn.execute(
            "UPDATE peerpay_received
                SET dismissed = 0, accepted_at = datetime('now')
              WHERE message_id = ?1 AND dismissed = 1",
            params![message_id],
        )?;
        Ok(raised > 0)
    }

    /// Dismiss all notifications matching a txid prefix (e.g., `utxo:{txid}:%`).
    ///
    /// Used to auto-dismiss green receive notifications when a red failure
    /// notification is created for the same transaction.
    pub fn dismiss_by_txid_prefix(conn: &Connection, txid: &str) -> Result<usize> {
        let pattern = format!("utxo:{}:%", txid);
        let rows = conn.execute(
            "UPDATE peerpay_received SET dismissed = 1 WHERE message_id LIKE ?1 AND dismissed = 0",
            params![pattern],
        )?;
        Ok(rows)
    }

    // --- Pending verification tracking: retired by B5-T1-P7b (held messages replace it); the
    // purge below still clears rows an older build left. ---

    /// Clean up expired pending verification records older than max_age_secs.
    pub fn cleanup_expired_pending(conn: &Connection, max_age_secs: i64) -> Result<usize> {
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 - max_age_secs;

        let rows = conn.execute(
            "DELETE FROM peerpay_pending_verification WHERE first_seen_at < ?1",
            params![cutoff],
        )?;
        Ok(rows)
    }

    // --- Held messages (B5-T1-P7b, V30): stored before the MessageBox message is acknowledged ---

    pub fn is_held(conn: &Connection, message_id: &str) -> Result<bool> {
        conn.query_row("SELECT EXISTS(SELECT 1 FROM peerpay_held_messages WHERE message_id = ?1)", params![message_id], |r| r.get(0))
    }

    /// (rows held for this sender, rows held in total) — for the caps.
    pub fn held_counts(conn: &Connection, sender: &str) -> Result<(i64, i64)> {
        conn.query_row(
            "SELECT (SELECT COUNT(*) FROM peerpay_held_messages WHERE sender = ?1), (SELECT COUNT(*) FROM peerpay_held_messages)",
            params![sender], |r| Ok((r.get(0)?, r.get(1)?)))
    }

    /// What the user is shown: held messages that name a derivation (a possible payment) — stranger
    /// garbage is not counted (V12) — and of those, the ones held for more than an hour (owner,
    /// 2026-10-11: a real payment is on chain within seconds; after an hour something is wrong).
    ///
    /// Review 164 #5: "over an hour" is measured by the run of tries in the row's CURRENT reason (the
    /// backoff reaches 1 h after 7 tries, the 60 s checks after 60, a restored row after its 2nd hourly
    /// look) — not by `held_at`, which an earlier lawful wait or an old backup would make look overdue.
    /// A refused broadcast is the network's verdict, not ours: never an alarm (owner, 2026-10-12).
    pub fn held_summary(conn: &Connection) -> Result<(i64, i64)> {
        // Review 164 #12: a possible payment names a derivation AND a transaction we could read; the
        // alarm is for reasons that should have reached the chain by now (not a lawful lock, not a
        // payment held on purpose such as an amount mismatch).
        conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM((reason IN ('chain_check_error', 'broadcast_failed') AND attempts >= 7)
                              OR (reason IN ('headers_unavailable', 'recent_bad_proof') AND attempts >= 60)
                              OR (reason = 'restored' AND attempts >= 2)), 0)
               FROM peerpay_held_messages
              WHERE derivation_prefix IS NOT NULL AND derivation_suffix IS NOT NULL AND txid IS NOT NULL",
            [], |r| Ok((r.get(0)?, r.get(1)?)))
    }

    /// The stored message of one held row (`None` for a row restored without it).
    pub fn held_body(conn: &Connection, message_id: &str) -> Result<Option<Vec<u8>>> {
        conn.query_row("SELECT body FROM peerpay_held_messages WHERE message_id = ?1", params![message_id], |r| r.get(0))
    }

    /// Give a held row restored without its body the body of the message, when it turns up again —
    /// only from the same sender, and the row becomes due at once (review 164 #10).
    pub fn fill_held_body(conn: &Connection, message_id: &str, sender: &str, body: &[u8]) -> Result<()> {
        conn.execute(
            "UPDATE peerpay_held_messages SET body = ?3, next_check_at = 0, lock_time = NULL
              WHERE message_id = ?1 AND sender = ?2 AND body IS NULL",
            params![message_id, sender, body])?;
        Ok(())
    }

    /// Rows for the backup, with the body only where a restore needs it: a payment not on chain yet
    /// (owner, 2026-10-11 — removed with the row once it is credited).
    pub fn held_for_backup(conn: &Connection) -> Result<Vec<HeldMessage>> {
        let mut stmt = conn.prepare(
            "SELECT message_id, sender,
                    CASE WHEN reason IN ('non_final', 'non_final_far', 'broadcast_failed', 'chain_check_error', 'headers_unavailable')
                              AND length(body) <= 32768 THEN body END,
                    derivation_prefix, derivation_suffix, txid, reason, held_at, lock_time, next_check_at, attempts
               FROM peerpay_held_messages ORDER BY held_at, message_id")?;
        let rows = stmt.query_map([], Self::held_row)?;
        rows.collect()
    }

    /// Insert a held row, or update an existing one's reason, schedule and attempts (the body and
    /// `held_at` of an existing row never change; a derivation once known is kept).
    pub fn save_held(conn: &Connection, m: &HeldMessage) -> Result<()> {
        conn.execute(
            "INSERT INTO peerpay_held_messages
                (message_id, sender, body, derivation_prefix, derivation_suffix, txid, reason, held_at, lock_time, next_check_at, attempts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(message_id) DO UPDATE SET
                derivation_prefix = COALESCE(excluded.derivation_prefix, derivation_prefix),
                derivation_suffix = COALESCE(excluded.derivation_suffix, derivation_suffix),
                txid = COALESCE(excluded.txid, txid),
                reason = excluded.reason,
                lock_time = excluded.lock_time,
                next_check_at = excluded.next_check_at,
                attempts = excluded.attempts",
            params![m.message_id, m.sender, m.body, m.derivation_prefix, m.derivation_suffix, m.txid,
                    m.reason, m.held_at, m.lock_time, m.next_check_at, m.attempts],
        )?;
        Ok(())
    }

    /// The row was looked at now (review 164 #1/#2: the order and the start pass read this).
    pub fn mark_held_checked(conn: &Connection, message_id: &str, now: i64) -> Result<()> {
        conn.execute("UPDATE peerpay_held_messages SET checked_at = ?2 WHERE message_id = ?1", params![message_id, now])?;
        Ok(())
    }

    /// Delete held rows first held before `before` that name no derivation — nothing in them can ever
    /// recover a coin (owner, 2026-10-12). Returns how many were deleted.
    pub fn expire_junk(conn: &Connection, before: i64) -> Result<usize> {
        conn.execute(
            "DELETE FROM peerpay_held_messages
              WHERE held_at < ?1 AND (derivation_prefix IS NULL OR derivation_suffix IS NULL)",
            params![before])
    }

    /// B5-T5-P7a (review 178-3 #1): remove a held row only when it is the payment just credited — its
    /// txid is that txid, or it names none (junk). A different payment listed under the same id must
    /// never delete it.
    pub fn delete_held_if_same(conn: &Connection, message_id: &str, txid: &str) -> Result<()> {
        conn.execute("DELETE FROM peerpay_held_messages WHERE message_id = ?1 AND (txid IS NULL OR txid = ?2)", params![message_id, txid])?;
        Ok(())
    }

    /// B5-T5-P7a (review 178-3 #7): may a copy that differs from the held row be decided now? Yes when
    /// the held row names no derivation (junk: the copy may be the real payment), or when the held row
    /// itself is due — a differing copy is never decided more often than the row it contests.
    pub fn held_contest_due(conn: &Connection, message_id: &str, now: i64, started_at: i64) -> Result<bool> {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM peerpay_held_messages WHERE message_id = ?1 AND (
                 derivation_prefix IS NULL OR derivation_suffix IS NULL
                 OR (next_check_at IS NOT NULL AND next_check_at <= ?2)
                 OR (next_check_at IS NULL AND lock_time IS NULL AND COALESCE(checked_at, 0) < ?3)))",
            params![message_id, now, started_at], |r| r.get(0))
    }

    /// The derivation a held row names (B5-T5-P7a, review 178-3 #3).
    pub fn held_derivation(conn: &Connection, message_id: &str) -> Result<(Option<String>, Option<String>)> {
        conn.query_row("SELECT derivation_prefix, derivation_suffix FROM peerpay_held_messages WHERE message_id = ?1",
            params![message_id], |r| Ok((r.get(0)?, r.get(1)?)))
    }

    /// A `receive` row exists for this txid (B5-T5-P7a, review 178-3 #9).
    pub fn has_receive_for_txid(conn: &Connection, txid: &str) -> Result<bool> {
        conn.query_row("SELECT EXISTS(SELECT 1 FROM peerpay_received WHERE txid = ?1 AND notification_type = 'receive')",
            params![txid], |r| r.get(0))
    }

    pub fn delete_held(conn: &Connection, message_id: &str) -> Result<()> {
        conn.execute("DELETE FROM peerpay_held_messages WHERE message_id = ?1", params![message_id])?;
        Ok(())
    }

    /// Rows to look at now: those whose `next_check_at` has come, those that wait for a lock (the
    /// caller tests the lock against its chain), and — at wallet start only — those that wait for
    /// a start.
    /// V6: no bodies are read here (up to 256 KiB each, under the wallet's DB lock) — `held_body`
    /// loads the one row being decided.
    ///
    /// Review 164 #1/#2: ordered least-recently-checked first (no 20 rows can starve the rest), and
    /// "wallet start" rows are those not checked since `started_at` (this process), so the start pass
    /// spreads over as many ticks as it needs.
    pub fn held_due(conn: &Connection, now: i64, started_at: i64) -> Result<Vec<HeldMessage>> {
        let mut stmt = conn.prepare(
            "SELECT message_id, sender, NULL, derivation_prefix, derivation_suffix, txid, reason, held_at, lock_time, next_check_at, attempts
               FROM peerpay_held_messages
              WHERE (next_check_at IS NOT NULL AND next_check_at <= ?1)
                 OR lock_time IS NOT NULL
                 OR (next_check_at IS NULL AND lock_time IS NULL AND COALESCE(checked_at, 0) < ?2)
              ORDER BY COALESCE(checked_at, 0), held_at, message_id")?;
        let rows = stmt.query_map(params![now, started_at], Self::held_row)?;
        rows.collect()
    }

    /// Every held row with its body (tests read rows back by value).
    #[cfg(test)]
    pub fn all_held(conn: &Connection) -> Result<Vec<HeldMessage>> {
        let mut stmt = conn.prepare(
            "SELECT message_id, sender, body, derivation_prefix, derivation_suffix, txid, reason, held_at, lock_time, next_check_at, attempts
               FROM peerpay_held_messages ORDER BY held_at, message_id")?;
        let rows = stmt.query_map([], Self::held_row)?;
        rows.collect()
    }

    fn held_row(r: &rusqlite::Row) -> Result<HeldMessage> {
        Ok(HeldMessage {
            message_id: r.get(0)?, sender: r.get(1)?, body: r.get(2)?, derivation_prefix: r.get(3)?,
            derivation_suffix: r.get(4)?, txid: r.get(5)?, reason: r.get(6)?, held_at: r.get(7)?,
            lock_time: r.get(8)?, next_check_at: r.get(9)?, attempts: r.get(10)?,
        })
    }

    // --- Outbox: MessageBox delivery retry queue for sent PeerPay payments ---

    /// Insert a failed MessageBox delivery into the outbox for background retry.
    /// B5-T5-P7a: `delivered_hosts` — the hosts that already took the message; `message_request` — the
    /// exact request posted, re-posted by the retry so every host sees one message id.
    pub fn insert_outbox(
        conn: &Connection,
        txid: &str,
        recipient_pubkey_hex: &str,
        payload_bytes: &[u8],
        amount_satoshis: i64,
        delivered_hosts: &[String],
        message_request: Option<&[u8]>,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        conn.execute(
            "INSERT OR IGNORE INTO peerpay_outbox (
                txid, recipient_pubkey_hex, payload_bytes, amount_satoshis,
                status, retry_count, next_retry_at, created_at, updated_at, delivered_hosts, message_request
            ) VALUES (?1, ?2, ?3, ?4, 'pending', 0, ?5, ?6, ?6, ?7, ?8)",
            params![txid, recipient_pubkey_hex, payload_bytes, amount_satoshis, now + 60, now,
                serde_json::to_string(delivered_hosts).unwrap_or_else(|_| "[]".into()), message_request],
        )?;
        Ok(())
    }

    /// B5-T5-P7a (review 178-6 #5): the outbox row of a payment.
    pub fn outbox_id_by_txid(conn: &Connection, txid: &str) -> Result<i64> {
        conn.query_row("SELECT id FROM peerpay_outbox WHERE txid = ?1", params![txid], |r| r.get(0))
    }

    /// B5-T5-P7a: record the hosts that have taken an outbox message so far, and the request posted.
    /// `message_request` `None` keeps the stored one.
    pub fn record_outbox_delivery(conn: &Connection, id: i64, delivered_hosts: &[String], message_request: Option<&[u8]>) -> Result<()> {
        conn.execute(
            "UPDATE peerpay_outbox SET delivered_hosts = ?1, message_request = COALESCE(?2, message_request) WHERE id = ?3",
            params![serde_json::to_string(delivered_hosts).unwrap_or_else(|_| "[]".into()), message_request, id],
        )?;
        Ok(())
    }

    /// Get outbox entries that are due for retry (status='pending' and next_retry_at <= now).
    /// beta.3 Phase 10d (`P10d-A4`) — the relay refused the message (4xx); the
    /// same bytes will never deliver, so stop retrying and show it. Also writes
    /// the one dismissable notification row the header dot and panel banner
    /// read (`undeliverable:{txid}`, `notification_type = 'undeliverable'`),
    /// so the dot can be cleared by Dismiss while the Activity row keeps the
    /// Retry / Copy-details affordances from the outbox row itself.
    pub fn mark_outbox_undeliverable(conn: &Connection, id: i64) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "UPDATE peerpay_outbox SET status = 'undeliverable', updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        let (txid, amount): (String, i64) = conn.query_row(
            "SELECT txid, amount_satoshis FROM peerpay_outbox WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Self::insert_undeliverable_notification(conn, &txid, amount)
    }

    fn insert_undeliverable_notification(conn: &Connection, txid: &str, amount: i64) -> Result<()> {
        let message_id = format!("undeliverable:{}", txid);
        conn.execute(
            "INSERT OR IGNORE INTO peerpay_received (
                message_id, sender_identity_key, amount_satoshis,
                derivation_prefix, derivation_suffix, txid,
                source, notification_type, price_usd_cents
            ) VALUES (?1, 'self', ?2, '', '', ?3, 'peerpay', 'undeliverable', NULL)",
            params![message_id, amount, txid],
        )?;
        Ok(())
    }

    /// One `undeliverable:{txid}` notification per outbox row whose recipient was
    /// not told (`exhausted` or `undeliverable`). `INSERT OR IGNORE`, so a row the
    /// user dismissed stays dismissed. Returns how many were added.
    pub fn backfill_undeliverable_notifications(conn: &Connection) -> Result<usize> {
        let rows = conn.execute(
            "INSERT OR IGNORE INTO peerpay_received (
                message_id, sender_identity_key, amount_satoshis,
                derivation_prefix, derivation_suffix, txid,
                source, notification_type, price_usd_cents
            )
            SELECT 'undeliverable:' || txid, 'self', amount_satoshis, '', '', txid,
                   'peerpay', 'undeliverable', NULL
            FROM peerpay_outbox WHERE status IN ('exhausted', 'undeliverable')",
            [],
        )?;
        Ok(rows)
    }

    /// Outbox rows in a given status, newest first — the Activity tab reads
    /// `undeliverable` rows to show the cause and the copy-details block.
    pub fn get_outbox_entries_by_status(conn: &Connection, status: &str) -> Result<Vec<OutboxEntry>> {
        let mut stmt = conn.prepare(
            "SELECT id, txid, recipient_pubkey_hex, payload_bytes, amount_satoshis, retry_count,
                    delivered_hosts, NULL
             FROM peerpay_outbox WHERE status = ?1 ORDER BY updated_at DESC"
        )?;
        let entries = stmt.query_map(params![status], |row| {
            Ok(OutboxEntry {
                id: row.get(0)?,
                txid: row.get(1)?,
                recipient_pubkey_hex: row.get(2)?,
                payload_bytes: row.get(3)?,
                amount_satoshis: row.get(4)?,
                retry_count: row.get(5)?,
                delivered_hosts: parse_delivered_hosts(row.get(6)?),
                message_request: row.get(7)?,
            })
        })?.collect::<Result<Vec<_>>>()?;
        Ok(entries)
    }

    pub fn get_due_outbox_entries(conn: &Connection) -> Result<Vec<OutboxEntry>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut stmt = conn.prepare(
            "SELECT id, txid, recipient_pubkey_hex, payload_bytes, amount_satoshis, retry_count,
                    delivered_hosts, message_request
             FROM peerpay_outbox
             WHERE status = 'pending' AND next_retry_at <= ?1
             ORDER BY next_retry_at ASC"
        )?;

        let entries = stmt.query_map(params![now], |row| {
            Ok(OutboxEntry {
                id: row.get(0)?,
                txid: row.get(1)?,
                recipient_pubkey_hex: row.get(2)?,
                payload_bytes: row.get(3)?,
                amount_satoshis: row.get(4)?,
                retry_count: row.get(5)?,
                delivered_hosts: parse_delivered_hosts(row.get(6)?),
                message_request: row.get(7)?,
            })
        })?.collect::<Result<Vec<_>>>()?;

        Ok(entries)
    }

    /// Update an outbox entry after a failed retry attempt.
    /// Increments retry_count and computes next_retry_at based on the schedule:
    ///   - retries 0-9: next in 60s (first 10 minutes)
    ///   - retries 10-19: next in 120s (next 20 minutes)
    ///   - retries >= 20: mark as 'exhausted' (give up)
    pub fn update_outbox_retry_failed(conn: &Connection, id: i64, current_retry_count: i32) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let new_count = current_retry_count + 1;

        if new_count >= OUTBOX_MAX_ATTEMPTS {
            conn.execute(
                "UPDATE peerpay_outbox SET status = 'exhausted', retry_count = ?1, updated_at = ?2 WHERE id = ?3",
                params![new_count, now, id],
            )?;
        } else {
            let delay = if new_count < 10 { 60 } else { 120 };
            conn.execute(
                "UPDATE peerpay_outbox SET retry_count = ?1, next_retry_at = ?2, updated_at = ?3 WHERE id = ?4",
                params![new_count, now + delay, now, id],
            )?;
        }
        Ok(())
    }

    /// Mark an outbox entry as successfully delivered.
    pub fn mark_outbox_delivered(conn: &Connection, id: i64) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        conn.execute(
            "UPDATE peerpay_outbox SET status = 'delivered', updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    /// Get summary of outbox entries: (exhausted_count, exhausted_total_sats, pending_count)
    pub fn get_outbox_summary(conn: &Connection) -> Result<(i64, i64, i64)> {
        // 'exhausted' (retries ran out) and 'undeliverable' (relay refused, 10d)
        // both mean "the recipient was not told" — one warning count.
        let (exhausted_count, exhausted_amount) = conn.query_row(
            "SELECT COALESCE(COUNT(*), 0), COALESCE(SUM(amount_satoshis), 0)
             FROM peerpay_outbox WHERE status IN ('exhausted', 'undeliverable')",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;

        let pending_count: i64 = conn.query_row(
            "SELECT COALESCE(COUNT(*), 0) FROM peerpay_outbox WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )?;

        Ok((exhausted_count, exhausted_amount, pending_count))
    }

    /// Reset an exhausted outbox entry for another retry cycle.
    /// Returns the number of rows affected (0 if no exhausted entry found for this txid).
    pub fn reset_outbox_for_retry(conn: &Connection, txid: &str) -> Result<usize> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows = conn.execute(
            "UPDATE peerpay_outbox SET status = 'pending', retry_count = 0, next_retry_at = ?1, updated_at = ?1
             WHERE txid = ?2 AND status IN ('exhausted', 'undeliverable')",
            params![now + 5, txid],
        )?;
        Ok(rows)
    }

    /// Remove delivered outbox entries older than max_age_secs.
    pub fn remove_delivered_outbox(conn: &Connection, max_age_secs: i64) -> Result<usize> {
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 - max_age_secs;

        let rows = conn.execute(
            "DELETE FROM peerpay_outbox WHERE status = 'delivered' AND updated_at < ?1",
            params![cutoff],
        )?;
        Ok(rows)
    }

    /// Get summary of undismissed notifications filtered by type: (count, total_satoshis)
    pub fn get_undismissed_summary_by_type(conn: &Connection, notification_type: &str) -> Result<(i64, i64)> {
        let result = conn.query_row(
            "SELECT COALESCE(COUNT(*), 0), COALESCE(SUM(amount_satoshis), 0)
             FROM peerpay_received
             WHERE dismissed = 0 AND notification_type = ?1",
            params![notification_type],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        Ok(result)
    }
}

#[cfg(test)]
mod rejected_notice_tests {
    use super::*;
    use crate::database::migrations;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrations::create_schema_v1(&conn).unwrap();
        conn
    }
    fn visible(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM peerpay_received WHERE notification_type='rejected' AND dismissed=0",
                       [], |r| r.get(0)).unwrap()
    }
    const A: &str = "02aaaa1111111111111111111111111111111111111111111111111111111111aa";
    const B: &str = "03bbbb2222222222222222222222222222222222222222222222222222222222bb";

    /// 👤 Owner decision surface: how loud is this? One visible notice per sender at
    /// a time — a burst from one attacker does not nag.
    #[test]
    fn one_visible_notice_per_sender_at_a_time() {
        let c = db();
        assert!(PeerPayRepository::insert_rejected_notification(&c, A).unwrap(), "first is raised");
        assert!(!PeerPayRepository::insert_rejected_notification(&c, A).unwrap(), "second changes nothing");
        assert!(!PeerPayRepository::insert_rejected_notification(&c, A).unwrap());
        assert_eq!(visible(&c), 1);
        assert!(PeerPayRepository::insert_rejected_notification(&c, B).unwrap(), "a different sender is its own notice");
        assert_eq!(visible(&c), 2);
    }

    /// Panel `F3-10a` — ⛔ the defect. A sender the user dismissed used to be
    /// silenced FOREVER, for every later attack, for the life of the wallet.
    #[test]
    fn f3_a_dismissed_sender_can_raise_a_later_attack() {
        let c = db();
        PeerPayRepository::insert_rejected_notification(&c, A).unwrap();
        PeerPayRepository::dismiss_all(&c).unwrap();
        assert_eq!(visible(&c), 0, "the user dismissed it");

        assert!(PeerPayRepository::insert_rejected_notification(&c, A).unwrap(),
                "a LATER rejection from the same sender must be heard again");
        assert_eq!(visible(&c), 1);
        // ...and it still does not nag while it is on screen
        assert!(!PeerPayRepository::insert_rejected_notification(&c, A).unwrap());
        assert_eq!(visible(&c), 1);
    }

    /// Control: re-raising must not multiply rows, or the banner's count lies.
    #[test]
    fn re_raising_reuses_the_same_row() {
        let c = db();
        for _ in 0..3 {
            PeerPayRepository::insert_rejected_notification(&c, A).unwrap();
            PeerPayRepository::dismiss_all(&c).unwrap();
        }
        PeerPayRepository::insert_rejected_notification(&c, A).unwrap();
        let total: i64 = c.query_row("SELECT COUNT(*) FROM peerpay_received WHERE notification_type='rejected'",
                                     [], |r| r.get(0)).unwrap();
        assert_eq!(total, 1, "one sender must never occupy more than one row");
        assert_eq!(visible(&c), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("
            CREATE TABLE peerpay_received (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id TEXT NOT NULL UNIQUE,
                sender_identity_key TEXT NOT NULL,
                amount_satoshis INTEGER NOT NULL,
                derivation_prefix TEXT NOT NULL,
                derivation_suffix TEXT NOT NULL,
                txid TEXT,
                accepted_at TEXT NOT NULL DEFAULT (datetime('now')),
                dismissed INTEGER NOT NULL DEFAULT 0,
                source TEXT NOT NULL DEFAULT 'peerpay',
                price_usd_cents INTEGER,
                notification_type TEXT NOT NULL DEFAULT 'receive'
            );
            CREATE INDEX idx_peerpay_dismissed ON peerpay_received(dismissed);
            CREATE INDEX idx_peerpay_source ON peerpay_received(source);
        ").unwrap();
        conn
    }

    #[test]
    fn test_insert_and_query() {
        let conn = setup_test_db();
        PeerPayRepository::insert_received(&conn, "msg1", "02abc", 1000, "prefix1", "suffix1", Some("txid1"), "peerpay", None).unwrap();

        let payments = PeerPayRepository::get_undismissed(&conn).unwrap();
        assert_eq!(payments.len(), 1);
        assert_eq!(payments[0].message_id, "msg1");
        assert_eq!(payments[0].amount_satoshis, 1000);
        assert_eq!(payments[0].source, "peerpay");
    }

    #[test]
    fn test_deduplication() {
        let conn = setup_test_db();
        PeerPayRepository::insert_received(&conn, "msg1", "02abc", 1000, "p", "s", None, "peerpay", None).unwrap();
        PeerPayRepository::insert_received(&conn, "msg1", "02abc", 1000, "p", "s", None, "peerpay", None).unwrap(); // duplicate

        assert!(PeerPayRepository::is_already_processed(&conn, "msg1").unwrap());
        assert!(!PeerPayRepository::is_already_processed(&conn, "msg2").unwrap());

        let payments = PeerPayRepository::get_undismissed(&conn).unwrap();
        assert_eq!(payments.len(), 1);
    }

    #[test]
    fn test_dismiss_all() {
        let conn = setup_test_db();
        PeerPayRepository::insert_received(&conn, "msg1", "02abc", 1000, "p", "s", None, "peerpay", None).unwrap();
        PeerPayRepository::insert_received(&conn, "msg2", "02def", 2000, "p", "s", None, "peerpay", None).unwrap();

        let (count, total) = PeerPayRepository::get_undismissed_summary(&conn).unwrap();
        assert_eq!(count, 2);
        assert_eq!(total, 3000);

        PeerPayRepository::dismiss_all(&conn).unwrap();

        let (count, total) = PeerPayRepository::get_undismissed_summary(&conn).unwrap();
        assert_eq!(count, 0);
        assert_eq!(total, 0);
    }

    #[test]
    fn test_address_sync_notification() {
        let conn = setup_test_db();

        // Insert address sync notification
        PeerPayRepository::insert_address_sync_notification(&conn, "abc123", 0, 5000, None).unwrap();

        let payments = PeerPayRepository::get_undismissed(&conn).unwrap();
        assert_eq!(payments.len(), 1);
        assert_eq!(payments[0].message_id, "utxo:abc123:0");
        assert_eq!(payments[0].source, "address_sync");
        assert_eq!(payments[0].amount_satoshis, 5000);
        assert_eq!(payments[0].txid, Some("abc123".to_string()));

        // Deduplication: inserting same UTXO again is a no-op
        PeerPayRepository::insert_address_sync_notification(&conn, "abc123", 0, 5000, None).unwrap();
        let payments = PeerPayRepository::get_undismissed(&conn).unwrap();
        assert_eq!(payments.len(), 1);
    }

    #[test]
    fn test_mixed_sources_summary() {
        let conn = setup_test_db();

        // Insert both PeerPay and address sync notifications
        PeerPayRepository::insert_received(&conn, "msg1", "02abc", 1000, "p", "s", None, "peerpay", None).unwrap();
        PeerPayRepository::insert_address_sync_notification(&conn, "txid1", 0, 3000, None).unwrap();

        let (count, total) = PeerPayRepository::get_undismissed_summary(&conn).unwrap();
        assert_eq!(count, 2);
        assert_eq!(total, 4000);

        // Dismiss clears both
        PeerPayRepository::dismiss_all(&conn).unwrap();
        let (count, _) = PeerPayRepository::get_undismissed_summary(&conn).unwrap();
        assert_eq!(count, 0);
    }
}

#[cfg(test)]
mod t5p7a_outbox_tests {
    use super::*;

    fn db() -> ((), crate::database::WalletDatabase) {
        let path = std::env::temp_dir().join(format!("hodos_t5p7a_{}.db", uuid::Uuid::new_v4()));
        ((), crate::database::WalletDatabase::new(path).unwrap())
    }

    /// `T5P7a-A14` — V31: an outbox row keeps the hosts that took it and the exact request, and a
    /// later attempt's record replaces them.
    #[test]
    fn t5p7a_a14_outbox_keeps_hosts_and_request() {
        let (_d, db) = db();
        let conn = db.connection();
        let hosts = vec!["https://a".to_string(), "https://b".to_string()];
        PeerPayRepository::insert_outbox(conn, "tx1", "02ab", b"payload", 1000, &hosts, Some(b"request")).unwrap();
        let e = PeerPayRepository::get_outbox_entries_by_status(conn, "pending").unwrap().remove(0);
        assert_eq!(e.delivered_hosts, hosts);
        let stored: Option<Vec<u8>> = conn.query_row("SELECT message_request FROM peerpay_outbox WHERE id = ?1", [e.id], |r| r.get(0)).unwrap();
        assert_eq!(stored, Some(b"request".to_vec()));
        assert!(e.message_request.is_none(), "the Activity reader does not load the request (review 178-2 #12)");

        let more = vec!["https://a".to_string(), "https://b".to_string(), "https://c".to_string()];
        PeerPayRepository::record_outbox_delivery(conn, e.id, &more, None).unwrap();
        let kept: Option<Vec<u8>> = conn.query_row("SELECT message_request FROM peerpay_outbox WHERE id = ?1", [e.id], |r| r.get(0)).unwrap();
        assert_eq!(kept, Some(b"request".to_vec()), "None keeps the stored request");
        let e = PeerPayRepository::get_outbox_entries_by_status(conn, "pending").unwrap().remove(0);
        assert_eq!(e.delivered_hosts, more);
    }

    /// `T5P7a-A14` — a row from before V31 (columns NULL) reads as "no host has it".
    #[test]
    fn t5p7a_a14_a_pre_v31_row_reads_as_none_taken() {
        let (_d, db) = db();
        let conn = db.connection();
        conn.execute("INSERT INTO peerpay_outbox (txid, recipient_pubkey_hex, payload_bytes, amount_satoshis, status, retry_count, next_retry_at, created_at, updated_at)
                      VALUES ('tx0', '02ab', x'00', 5, 'pending', 0, 0, 0, 0)", []).unwrap();
        let e = PeerPayRepository::get_outbox_entries_by_status(conn, "pending").unwrap().remove(0);
        assert!(e.delivered_hosts.is_empty() && e.message_request.is_none());
    }
}
