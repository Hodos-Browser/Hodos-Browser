//! PeerPay Repository — Persistent storage for received PeerPay payments
//!
//! Replaces the in-memory MessageStore with SQLite-backed tracking.
//! Supports deduplication (message_id uniqueness), undismissed summaries
//! for the notification badge, and dismiss-all for clearing notifications.

use rusqlite::{Connection, Result, params};
use serde::Serialize;

/// A received PeerPay payment record
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
    ) -> Result<()> {
        conn.execute(
            "INSERT OR IGNORE INTO peerpay_received (message_id, sender_identity_key, amount_satoshis, derivation_prefix, derivation_suffix, txid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![message_id, sender, amount, prefix, suffix, txid],
        )?;
        Ok(())
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
            "SELECT id, message_id, sender_identity_key, amount_satoshis, derivation_prefix, derivation_suffix, txid, accepted_at, dismissed
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
                dismissed INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX idx_peerpay_dismissed ON peerpay_received(dismissed);
        ").unwrap();
        conn
    }

    #[test]
    fn test_insert_and_query() {
        let conn = setup_test_db();
        PeerPayRepository::insert_received(&conn, "msg1", "02abc", 1000, "prefix1", "suffix1", Some("txid1")).unwrap();

        let payments = PeerPayRepository::get_undismissed(&conn).unwrap();
        assert_eq!(payments.len(), 1);
        assert_eq!(payments[0].message_id, "msg1");
        assert_eq!(payments[0].amount_satoshis, 1000);
    }

    #[test]
    fn test_deduplication() {
        let conn = setup_test_db();
        PeerPayRepository::insert_received(&conn, "msg1", "02abc", 1000, "p", "s", None).unwrap();
        PeerPayRepository::insert_received(&conn, "msg1", "02abc", 1000, "p", "s", None).unwrap(); // duplicate

        assert!(PeerPayRepository::is_already_processed(&conn, "msg1").unwrap());
        assert!(!PeerPayRepository::is_already_processed(&conn, "msg2").unwrap());

        let payments = PeerPayRepository::get_undismissed(&conn).unwrap();
        assert_eq!(payments.len(), 1);
    }

    #[test]
    fn test_dismiss_all() {
        let conn = setup_test_db();
        PeerPayRepository::insert_received(&conn, "msg1", "02abc", 1000, "p", "s", None).unwrap();
        PeerPayRepository::insert_received(&conn, "msg2", "02def", 2000, "p", "s", None).unwrap();

        let (count, total) = PeerPayRepository::get_undismissed_summary(&conn).unwrap();
        assert_eq!(count, 2);
        assert_eq!(total, 3000);

        PeerPayRepository::dismiss_all(&conn).unwrap();

        let (count, total) = PeerPayRepository::get_undismissed_summary(&conn).unwrap();
        assert_eq!(count, 0);
        assert_eq!(total, 0);
    }
}
