//! Message Relay Repository for BRC-33 PeerServ
//!
//! Handles persistent storage of relay messages in SQLite database.
//! Replaces in-memory storage for message relay system.

use rusqlite::{Connection, Result, params};
use log::info;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// A message stored in the relay database
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelayMessage {
    /// Database ID (used as message_id in API responses)
    pub id: i64,
    /// Recipient's public key (hex)
    pub recipient: String,
    /// Message box name
    pub message_box: String,
    /// Sender's public key (hex)
    pub sender: String,
    /// Message body content
    pub body: String,
    /// Unix timestamp when created
    pub created_at: i64,
    /// Optional expiry timestamp
    pub expires_at: Option<i64>,
}

/// Repository for relay message operations
pub struct MessageRelayRepository<'a> {
    conn: &'a Connection,
}

impl<'a> MessageRelayRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        MessageRelayRepository { conn }
    }

    /// Send a message (store in database)
    ///
    /// # Arguments
    /// * `recipient` - Recipient's public key (hex string)
    /// * `message_box` - Name of the message box
    /// * `sender` - Sender's public key (hex string)
    /// * `body` - Message content
    ///
    /// # Returns
    /// The message ID of the newly created message
    pub fn send_message(
        &self,
        recipient: &str,
        message_box: &str,
        sender: &str,
        body: &str,
    ) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO relay_messages (recipient, message_box, sender, body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![recipient, message_box, sender, body, now],
        )?;

        let message_id = self.conn.last_insert_rowid();

        info!(
            "📨 Stored message {} to {}/{} from {}",
            message_id, recipient, message_box, sender
        );

        Ok(message_id)
    }

    /// List all messages in a recipient's message box
    ///
    /// # Arguments
    /// * `recipient` - Recipient's public key (hex string)
    /// * `message_box` - Name of the message box
    ///
    /// # Returns
    /// Vector of messages (empty if none found)
    pub fn list_messages(&self, recipient: &str, message_box: &str) -> Result<Vec<RelayMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, recipient, message_box, sender, body, created_at, expires_at
             FROM relay_messages
             WHERE recipient = ?1 AND message_box = ?2
             ORDER BY created_at ASC"
        )?;

        let messages = stmt.query_map(params![recipient, message_box], |row| {
            Ok(RelayMessage {
                id: row.get(0)?,
                recipient: row.get(1)?,
                message_box: row.get(2)?,
                sender: row.get(3)?,
                body: row.get(4)?,
                created_at: row.get(5)?,
                expires_at: row.get(6)?,
            })
        })?;

        let result: Vec<RelayMessage> = messages.filter_map(|r| r.ok()).collect();

        info!(
            "📬 Listed {} messages for {}/{}",
            result.len(), recipient, message_box
        );

        Ok(result)
    }

    /// Acknowledge (delete) messages
    ///
    /// # Arguments
    /// * `recipient` - Recipient's public key (to verify ownership)
    /// * `message_ids` - Array of message IDs to delete
    ///
    /// # Returns
    /// Number of messages actually deleted
    pub fn acknowledge_messages(&self, recipient: &str, message_ids: &[i64]) -> Result<usize> {
        if message_ids.is_empty() {
            return Ok(0);
        }

        let mut deleted = 0;

        for id in message_ids {
            let rows_affected = self.conn.execute(
                "DELETE FROM relay_messages WHERE id = ?1 AND recipient = ?2",
                params![id, recipient],
            )?;
            deleted += rows_affected;
        }

        info!(
            "✅ Acknowledged {} messages for {} (requested: {})",
            deleted, recipient, message_ids.len()
        );

        Ok(deleted)
    }

    /// Delete expired messages
    ///
    /// # Returns
    /// Number of messages deleted
    pub fn cleanup_expired(&self) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let deleted = self.conn.execute(
            "DELETE FROM relay_messages WHERE expires_at IS NOT NULL AND expires_at < ?1",
            params![now],
        )?;

        if deleted > 0 {
            info!("🗑️  Cleaned up {} expired messages", deleted);
        }

        Ok(deleted)
    }

    /// Delete messages older than a certain age
    ///
    /// # Arguments
    /// * `max_age_days` - Maximum age in days
    ///
    /// # Returns
    /// Number of messages deleted
    pub fn cleanup_old_messages(&self, max_age_days: i64) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let cutoff = now - (max_age_days * 24 * 60 * 60);

        let deleted = self.conn.execute(
            "DELETE FROM relay_messages WHERE created_at < ?1",
            params![cutoff],
        )?;

        if deleted > 0 {
            info!("🗑️  Cleaned up {} messages older than {} days", deleted, max_age_days);
        }

        Ok(deleted)
    }

    /// Get statistics about stored messages
    pub fn get_stats(&self) -> Result<MessageRelayStats> {
        let total_messages: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM relay_messages",
            [],
            |row| row.get(0),
        )?;

        let total_recipients: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT recipient) FROM relay_messages",
            [],
            |row| row.get(0),
        )?;

        let total_boxes: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT recipient || '/' || message_box) FROM relay_messages",
            [],
            |row| row.get(0),
        )?;

        Ok(MessageRelayStats {
            total_messages: total_messages as usize,
            total_recipients: total_recipients as usize,
            total_message_boxes: total_boxes as usize,
        })
    }
}

/// Statistics about the message relay store
#[derive(Debug, Serialize)]
pub struct MessageRelayStats {
    pub total_messages: usize,
    pub total_recipients: usize,
    pub total_message_boxes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE relay_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                recipient TEXT NOT NULL,
                message_box TEXT NOT NULL,
                sender TEXT NOT NULL,
                body TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER
            )",
            [],
        ).unwrap();
        conn
    }

    #[test]
    fn test_send_and_list_messages() {
        let conn = setup_test_db();
        let repo = MessageRelayRepository::new(&conn);

        let recipient = "02abc123";
        let sender = "02def456";
        let message_box = "test_inbox";

        // Send a message
        let msg_id = repo.send_message(recipient, message_box, sender, "Hello!").unwrap();
        assert!(msg_id > 0);

        // List messages
        let messages = repo.list_messages(recipient, message_box).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, msg_id);
        assert_eq!(messages[0].sender, sender);
        assert_eq!(messages[0].body, "Hello!");
    }

    #[test]
    fn test_acknowledge_messages() {
        let conn = setup_test_db();
        let repo = MessageRelayRepository::new(&conn);

        let recipient = "02abc123";
        let sender = "02def456";
        let message_box = "test_inbox";

        // Send two messages
        let msg_id1 = repo.send_message(recipient, message_box, sender, "Message 1").unwrap();
        let msg_id2 = repo.send_message(recipient, message_box, sender, "Message 2").unwrap();

        // Verify we have 2 messages
        let messages = repo.list_messages(recipient, message_box).unwrap();
        assert_eq!(messages.len(), 2);

        // Acknowledge first message
        let deleted = repo.acknowledge_messages(recipient, &[msg_id1]).unwrap();
        assert_eq!(deleted, 1);

        // Verify only one message remains
        let messages = repo.list_messages(recipient, message_box).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, msg_id2);
    }

    #[test]
    fn test_acknowledge_wrong_recipient() {
        let conn = setup_test_db();
        let repo = MessageRelayRepository::new(&conn);

        let recipient = "02abc123";
        let wrong_recipient = "02wrong";
        let sender = "02def456";
        let message_box = "test_inbox";

        // Send a message
        let msg_id = repo.send_message(recipient, message_box, sender, "Hello!").unwrap();

        // Try to acknowledge with wrong recipient
        let deleted = repo.acknowledge_messages(wrong_recipient, &[msg_id]).unwrap();
        assert_eq!(deleted, 0);

        // Message should still exist
        let messages = repo.list_messages(recipient, message_box).unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_multiple_message_boxes() {
        let conn = setup_test_db();
        let repo = MessageRelayRepository::new(&conn);

        let recipient = "02abc123";
        let sender = "02def456";

        // Send to different message boxes
        repo.send_message(recipient, "inbox1", sender, "Message 1").unwrap();
        repo.send_message(recipient, "inbox2", sender, "Message 2").unwrap();
        repo.send_message(recipient, "inbox1", sender, "Message 3").unwrap();

        // Check inbox1 has 2 messages
        let inbox1 = repo.list_messages(recipient, "inbox1").unwrap();
        assert_eq!(inbox1.len(), 2);

        // Check inbox2 has 1 message
        let inbox2 = repo.list_messages(recipient, "inbox2").unwrap();
        assert_eq!(inbox2.len(), 1);
    }

    #[test]
    fn test_stats() {
        let conn = setup_test_db();
        let repo = MessageRelayRepository::new(&conn);

        // Initial stats
        let stats = repo.get_stats().unwrap();
        assert_eq!(stats.total_messages, 0);

        // Add messages
        repo.send_message("rec1", "box1", "sender", "msg1").unwrap();
        repo.send_message("rec1", "box2", "sender", "msg2").unwrap();
        repo.send_message("rec2", "box1", "sender", "msg3").unwrap();

        let stats = repo.get_stats().unwrap();
        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.total_recipients, 2);
        assert_eq!(stats.total_message_boxes, 3);
    }
}
