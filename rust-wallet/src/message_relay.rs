/// BRC-33 PeerServ Message Relay - Message Storage System
/// Specification: https://bsv.brc.dev/peer-to-peer/0033
///
/// This module provides in-memory storage for messages between peers.
/// Messages are organized by recipient -> message_box -> messages

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A single message in the relay system
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for this message
    #[serde(rename = "messageId")]
    pub message_id: u64,

    /// Sender's public key
    pub sender: String,

    /// Message content/body
    pub body: String,

    /// Unix timestamp when message was created
    #[serde(skip_serializing)]
    pub timestamp: i64,
}

/// Thread-safe message storage
/// Structure: recipient_pubkey -> message_box_name -> Vec<Message>
#[derive(Clone)]
pub struct MessageStore {
    /// Nested HashMap: recipient -> message_box -> messages
    messages: Arc<Mutex<HashMap<String, HashMap<String, Vec<Message>>>>>,

    /// Auto-incrementing message ID
    next_id: Arc<Mutex<u64>>,
}

impl MessageStore {
    /// Create a new empty message store
    pub fn new() -> Self {
        log::info!("📬 Initializing BRC-33 message relay storage");
        Self {
            messages: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Send a message to a recipient's message box
    ///
    /// # Arguments
    /// * `recipient` - Recipient's public key (hex string)
    /// * `message_box` - Name of the message box (e.g., "coinflip_inbox")
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
    ) -> u64 {
        let mut messages = self.messages.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();

        let message = Message {
            message_id: *next_id,
            sender: sender.to_string(),
            body: body.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        log::info!(
            "📨 Sending message {} to {}/{} from {}",
            message.message_id,
            recipient,
            message_box,
            sender
        );

        // Get or create recipient's message boxes
        let recipient_boxes = messages
            .entry(recipient.to_string())
            .or_insert_with(HashMap::new);

        // Get or create the specific message box
        let box_messages = recipient_boxes
            .entry(message_box.to_string())
            .or_insert_with(Vec::new);

        // Add the message
        box_messages.push(message);

        // Increment message ID for next message
        let current_id = *next_id;
        *next_id += 1;

        log::info!(
            "✅ Message {} stored successfully (box now has {} messages)",
            current_id,
            box_messages.len()
        );

        current_id
    }

    /// List all messages from a recipient's message box
    ///
    /// # Arguments
    /// * `recipient` - Recipient's public key (hex string)
    /// * `message_box` - Name of the message box
    ///
    /// # Returns
    /// Vector of messages (empty if no messages or box doesn't exist)
    pub fn list_messages(&self, recipient: &str, message_box: &str) -> Vec<Message> {
        let messages = self.messages.lock().unwrap();

        let result = messages
            .get(recipient)
            .and_then(|boxes| boxes.get(message_box))
            .cloned()
            .unwrap_or_default();

        log::info!(
            "📬 Listing messages for {}/{}: found {} messages",
            recipient,
            message_box,
            result.len()
        );

        result
    }

    /// Acknowledge (delete) messages from a message box
    ///
    /// # Arguments
    /// * `recipient` - Recipient's public key (hex string)
    /// * `message_box` - Name of the message box
    /// * `message_ids` - Array of message IDs to delete
    pub fn acknowledge_messages(
        &self,
        recipient: &str,
        message_box: &str,
        message_ids: &[u64],
    ) {
        let mut messages = self.messages.lock().unwrap();

        if let Some(recipient_boxes) = messages.get_mut(recipient) {
            if let Some(box_messages) = recipient_boxes.get_mut(message_box) {
                let original_count = box_messages.len();

                // Remove messages with matching IDs
                box_messages.retain(|msg| !message_ids.contains(&msg.message_id));

                let removed_count = original_count - box_messages.len();

                log::info!(
                    "✅ Acknowledged {} messages in {}/{} (requested: {}, found: {})",
                    removed_count,
                    recipient,
                    message_box,
                    message_ids.len(),
                    removed_count
                );

                // Clean up empty message boxes
                if box_messages.is_empty() {
                    recipient_boxes.remove(message_box);
                    log::info!("🗑️  Removed empty message box: {}/{}", recipient, message_box);
                }
            } else {
                log::warn!("❌ Message box not found: {}/{}", recipient, message_box);
            }
        } else {
            log::warn!("❌ Recipient not found: {}", recipient);
        }
    }

    /// Get statistics about the message store
    pub fn get_stats(&self) -> MessageStoreStats {
        let messages = self.messages.lock().unwrap();

        let mut stats = MessageStoreStats {
            total_recipients: messages.len(),
            total_message_boxes: 0,
            total_messages: 0,
        };

        for boxes in messages.values() {
            stats.total_message_boxes += boxes.len();
            for msgs in boxes.values() {
                stats.total_messages += msgs.len();
            }
        }

        stats
    }
}

/// Statistics about the message store
#[derive(Debug, Serialize)]
pub struct MessageStoreStats {
    pub total_recipients: usize,
    pub total_message_boxes: usize,
    pub total_messages: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_and_list_messages() {
        let store = MessageStore::new();
        let recipient = "02abc123";
        let sender = "02def456";
        let message_box = "test_inbox";

        // Send a message
        let msg_id = store.send_message(recipient, message_box, sender, "Hello!");

        // List messages
        let messages = store.list_messages(recipient, message_box);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id, msg_id);
        assert_eq!(messages[0].sender, sender);
        assert_eq!(messages[0].body, "Hello!");
    }

    #[test]
    fn test_acknowledge_messages() {
        let store = MessageStore::new();
        let recipient = "02abc123";
        let sender = "02def456";
        let message_box = "test_inbox";

        // Send two messages
        let msg_id1 = store.send_message(recipient, message_box, sender, "Message 1");
        let msg_id2 = store.send_message(recipient, message_box, sender, "Message 2");

        // Verify we have 2 messages
        let messages = store.list_messages(recipient, message_box);
        assert_eq!(messages.len(), 2);

        // Acknowledge first message
        store.acknowledge_messages(recipient, message_box, &[msg_id1]);

        // Verify only one message remains
        let messages = store.list_messages(recipient, message_box);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id, msg_id2);
    }

    #[test]
    fn test_multiple_message_boxes() {
        let store = MessageStore::new();
        let recipient = "02abc123";
        let sender = "02def456";

        // Send to different message boxes
        store.send_message(recipient, "inbox1", sender, "Message 1");
        store.send_message(recipient, "inbox2", sender, "Message 2");
        store.send_message(recipient, "inbox1", sender, "Message 3");

        // Check inbox1 has 2 messages
        let inbox1 = store.list_messages(recipient, "inbox1");
        assert_eq!(inbox1.len(), 2);

        // Check inbox2 has 1 message
        let inbox2 = store.list_messages(recipient, "inbox2");
        assert_eq!(inbox2.len(), 1);
    }
}
