use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Represents the status of a transaction action (LEGACY - use TransactionStatus for new code)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionStatus {
    #[serde(rename = "created")]
    Created,      // Transaction created but not signed
    #[serde(rename = "signed")]
    Signed,       // Transaction signed but not broadcast
    #[serde(rename = "unconfirmed")]
    Unconfirmed,  // Broadcast to network, waiting for confirmation
    #[serde(rename = "pending")]
    Pending,      // 1-5 confirmations
    #[serde(rename = "confirmed")]
    Confirmed,    // 6+ confirmations
    #[serde(rename = "aborted")]
    Aborted,      // Cancelled by user
    #[serde(rename = "failed")]
    Failed,       // Broadcast failed or double-spent
}

impl ActionStatus {
    pub fn to_string(&self) -> String {
        match self {
            ActionStatus::Created => "created".to_string(),
            ActionStatus::Signed => "signed".to_string(),
            ActionStatus::Unconfirmed => "unconfirmed".to_string(),
            ActionStatus::Pending => "pending".to_string(),
            ActionStatus::Confirmed => "confirmed".to_string(),
            ActionStatus::Aborted => "aborted".to_string(),
            ActionStatus::Failed => "failed".to_string(),
        }
    }
}

/// Consolidated transaction status aligned with BSV SDK wallet-toolbox.
///
/// Single `status` column on the transactions table.
///
/// Mapping from old dual status:
///   (created, pending)      → Unsigned
///   (signed, broadcast)     → Sending
///   (unconfirmed, broadcast)→ Unproven
///   (pending, *)            → Unproven
///   (confirmed, confirmed)  → Completed
///   (aborted, *)            → Nosend
///   (failed, failed)        → Failed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    #[serde(rename = "completed")]
    Completed,     // Proven/confirmed transaction (has merkle proof or 6+ confirmations)
    #[serde(rename = "unprocessed")]
    Unprocessed,   // Created but not yet handled
    #[serde(rename = "sending")]
    Sending,       // Being broadcast to network
    #[serde(rename = "unproven")]
    Unproven,      // Broadcast but not yet confirmed/proven
    #[serde(rename = "unsigned")]
    Unsigned,      // Created but not yet signed
    #[serde(rename = "nosend")]
    Nosend,        // Signed but intentionally not broadcast (aborted / data carrier)
    #[serde(rename = "nonfinal")]
    Nonfinal,      // Has future locktime, not yet finalized
    #[serde(rename = "failed")]
    Failed,        // Broadcast failed or rejected
}

impl TransactionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionStatus::Completed => "completed",
            TransactionStatus::Unprocessed => "unprocessed",
            TransactionStatus::Sending => "sending",
            TransactionStatus::Unproven => "unproven",
            TransactionStatus::Unsigned => "unsigned",
            TransactionStatus::Nosend => "nosend",
            TransactionStatus::Nonfinal => "nonfinal",
            TransactionStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "completed" => TransactionStatus::Completed,
            "unprocessed" => TransactionStatus::Unprocessed,
            "sending" => TransactionStatus::Sending,
            "unproven" => TransactionStatus::Unproven,
            "unsigned" => TransactionStatus::Unsigned,
            "nosend" => TransactionStatus::Nosend,
            "nonfinal" => TransactionStatus::Nonfinal,
            "failed" => TransactionStatus::Failed,
            _ => TransactionStatus::Unprocessed, // safe default
        }
    }

    /// Convert from legacy ActionStatus + broadcast_status to unified TransactionStatus
    pub fn from_legacy(action_status: &ActionStatus, broadcast_status: Option<&str>) -> Self {
        match (action_status, broadcast_status) {
            (ActionStatus::Created, _) => TransactionStatus::Unsigned,
            (ActionStatus::Signed, Some("broadcast")) => TransactionStatus::Sending,
            (ActionStatus::Signed, _) => TransactionStatus::Sending,
            (ActionStatus::Unconfirmed, _) => TransactionStatus::Unproven,
            (ActionStatus::Pending, Some("confirmed")) => TransactionStatus::Completed,
            (ActionStatus::Pending, _) => TransactionStatus::Unproven,
            (ActionStatus::Confirmed, _) => TransactionStatus::Completed,
            (ActionStatus::Aborted, _) => TransactionStatus::Nosend,
            (ActionStatus::Failed, _) => TransactionStatus::Failed,
        }
    }

    /// Convert to legacy ActionStatus (for backward compatibility with StoredAction/JSON)
    pub fn to_action_status(&self) -> ActionStatus {
        match self {
            TransactionStatus::Completed => ActionStatus::Confirmed,
            TransactionStatus::Unprocessed => ActionStatus::Created,
            TransactionStatus::Sending => ActionStatus::Signed,
            TransactionStatus::Unproven => ActionStatus::Unconfirmed,
            TransactionStatus::Unsigned => ActionStatus::Created,
            TransactionStatus::Nosend => ActionStatus::Aborted,
            TransactionStatus::Nonfinal => ActionStatus::Created,
            TransactionStatus::Failed => ActionStatus::Failed,
        }
    }
}

/// Status lifecycle for proof acquisition requests.
/// Tracks the state of a proven_tx_reqs record from broadcast to proof.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProvenTxReqStatus {
    #[serde(rename = "unknown")]
    Unknown,     // Initial state
    #[serde(rename = "sending")]
    Sending,     // Transaction being broadcast
    #[serde(rename = "unsent")]
    Unsent,      // Created but not sent
    #[serde(rename = "nosend")]
    Nosend,      // Not intended for broadcast
    #[serde(rename = "unproven")]
    Unproven,    // Broadcast but no proof yet
    #[serde(rename = "invalid")]
    Invalid,     // Invalid transaction
    #[serde(rename = "unmined")]
    Unmined,     // Confirmed unmined
    #[serde(rename = "callback")]
    Callback,    // Awaiting callback
    #[serde(rename = "completed")]
    Completed,   // Proof acquired
}

impl ProvenTxReqStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProvenTxReqStatus::Unknown => "unknown",
            ProvenTxReqStatus::Sending => "sending",
            ProvenTxReqStatus::Unsent => "unsent",
            ProvenTxReqStatus::Nosend => "nosend",
            ProvenTxReqStatus::Unproven => "unproven",
            ProvenTxReqStatus::Invalid => "invalid",
            ProvenTxReqStatus::Unmined => "unmined",
            ProvenTxReqStatus::Callback => "callback",
            ProvenTxReqStatus::Completed => "completed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "unknown" => ProvenTxReqStatus::Unknown,
            "sending" => ProvenTxReqStatus::Sending,
            "unsent" => ProvenTxReqStatus::Unsent,
            "nosend" => ProvenTxReqStatus::Nosend,
            "unproven" => ProvenTxReqStatus::Unproven,
            "invalid" => ProvenTxReqStatus::Invalid,
            "unmined" => ProvenTxReqStatus::Unmined,
            "callback" => ProvenTxReqStatus::Callback,
            "completed" => ProvenTxReqStatus::Completed,
            _ => ProvenTxReqStatus::Unknown,
        }
    }

    /// Whether this status is terminal (no further transitions expected)
    pub fn is_terminal(&self) -> bool {
        matches!(self, ProvenTxReqStatus::Completed | ProvenTxReqStatus::Invalid)
    }
}

/// Input for a stored action (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionInput {
    pub txid: String,
    pub vout: u32,
    pub satoshis: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
}

/// Output for a stored action (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionOutput {
    pub vout: u32,
    pub satoshis: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

/// Represents a stored transaction action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAction {
    pub txid: String,
    #[serde(rename = "referenceNumber")]
    pub reference_number: String,
    #[serde(rename = "rawTx")]
    pub raw_tx: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub status: ActionStatus,
    #[serde(rename = "isOutgoing")]
    pub is_outgoing: bool,
    pub satoshis: i64,
    pub timestamp: i64,
    #[serde(rename = "blockHeight")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u32>,
    #[serde(default)]
    pub confirmations: u32,

    // Transaction details
    pub version: u32,
    #[serde(rename = "lockTime")]
    pub lock_time: u32,
    pub inputs: Vec<ActionInput>,
    pub outputs: Vec<ActionOutput>,

    /// BSV/USD price in cents at transaction time (e.g. 4523 = $45.23/BSV)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub price_usd_cents: Option<i64>,
}

/// Storage for transaction actions
pub struct ActionStorage {
    file_path: PathBuf,
    actions: HashMap<String, StoredAction>, // key = txid
}

impl ActionStorage {
    /// Create new action storage at the specified path
    pub fn new(actions_path: PathBuf) -> Result<Self, String> {
        let mut storage = ActionStorage {
            file_path: actions_path,
            actions: HashMap::new(),
        };

        // Try to load existing actions, create empty if doesn't exist
        if storage.file_path.exists() {
            storage.load()?;
        } else {
            log::info!("📁 Creating new actions.json at: {:?}", storage.file_path);
            storage.save()?;
        }

        Ok(storage)
    }

    /// Load actions from JSON file
    pub fn load(&mut self) -> Result<(), String> {
        let data = fs::read_to_string(&self.file_path)
            .map_err(|e| format!("Failed to read actions file: {}", e))?;

        self.actions = serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse actions file: {}", e))?;

        log::info!("📂 Loaded {} actions from storage", self.actions.len());
        Ok(())
    }

    /// Save actions to JSON file
    pub fn save(&self) -> Result<(), String> {
        let data = serde_json::to_string_pretty(&self.actions)
            .map_err(|e| format!("Failed to serialize actions: {}", e))?;

        fs::write(&self.file_path, data)
            .map_err(|e| format!("Failed to write actions file: {}", e))?;

        Ok(())
    }

    /// Add a new action to storage
    pub fn add_action(&mut self, action: StoredAction) -> Result<(), String> {
        let txid = action.txid.clone();

        if self.actions.contains_key(&txid) {
            return Err(format!("Action already exists: {}", txid));
        }

        self.actions.insert(txid.clone(), action);
        self.save()?;

        log::info!("✅ Added action: {}", txid);
        Ok(())
    }

    /// Get action by txid
    pub fn get_action(&self, txid: &str) -> Option<&StoredAction> {
        self.actions.get(txid)
    }

    /// Get action by reference number
    pub fn get_action_by_reference(&self, reference_number: &str) -> Option<&StoredAction> {
        self.actions.values().find(|a| a.reference_number == reference_number)
    }

    /// Update action status
    pub fn update_status(&mut self, txid: &str, status: ActionStatus) -> Result<(), String> {
        let action = self.actions.get_mut(txid)
            .ok_or(format!("Action not found: {}", txid))?;

        action.status = status.clone();
        let status_str = action.status.to_string();

        self.save()?;

        log::info!("📝 Updated action status: {} -> {}", txid, status_str);
        Ok(())
    }

    /// Update the TXID of an action (needed after signing changes the transaction)
    /// Uses reference number since the TXID changes after signing
    pub fn update_txid(&mut self, reference_number: &str, new_txid: String, new_raw_tx: String) -> Result<(), String> {
        // Find action by reference number
        let old_action = self.actions.values()
            .find(|a| a.reference_number == reference_number)
            .ok_or(format!("Action not found with reference: {}", reference_number))?
            .clone();

        let old_txid = old_action.txid.clone();

        // Remove old entry
        self.actions.remove(&old_txid);

        // Create updated action with new TXID
        let mut updated_action = old_action;
        updated_action.txid = new_txid.clone();
        updated_action.raw_tx = new_raw_tx;

        // Insert with new TXID
        self.actions.insert(new_txid.clone(), updated_action);

        self.save()?;
        log::info!("📝 Updated TXID: {} → {}", old_txid, new_txid);
        Ok(())
    }

    /// Update confirmation count and block height
    pub fn update_confirmations(&mut self, txid: &str, confirmations: u32, block_height: Option<u32>) -> Result<(), String> {
        let action = self.actions.get_mut(txid)
            .ok_or(format!("Action not found: {}", txid))?;

        action.confirmations = confirmations;
        action.block_height = block_height;

        // Update status based on confirmations
        action.status = if confirmations == 0 {
            ActionStatus::Unconfirmed
        } else if confirmations < 6 {
            ActionStatus::Pending
        } else {
            ActionStatus::Confirmed
        };

        self.save()?;
        Ok(())
    }

    /// List all actions (optionally filtered by labels)
    pub fn list_actions(&self, label_filter: Option<&Vec<String>>, label_mode: Option<&str>) -> Vec<&StoredAction> {
        let mut results: Vec<&StoredAction> = self.actions.values().collect();

        // Filter by labels if provided
        if let Some(labels) = label_filter {
            if !labels.is_empty() {
                let mode = label_mode.unwrap_or("any");
                results.retain(|action| {
                    match mode {
                        "all" => labels.iter().all(|l| action.labels.contains(l)),
                        _ => action.labels.iter().any(|l| labels.contains(l)), // "any" mode
                    }
                });
            }
        }

        // Sort by timestamp (newest first)
        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        results
    }

    /// Get total count of actions
    pub fn count(&self) -> usize {
        self.actions.len()
    }

    /// Delete action by txid
    pub fn delete_action(&mut self, txid: &str) -> Result<(), String> {
        self.actions.remove(txid)
            .ok_or(format!("Action not found: {}", txid))?;

        self.save()?;
        log::info!("🗑️ Deleted action: {}", txid);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_action_storage_new() {
        let temp_dir = env::temp_dir();
        let test_path = temp_dir.join("test_actions.json");

        // Clean up if exists
        let _ = fs::remove_file(&test_path);

        let storage = ActionStorage::new(test_path.clone()).unwrap();
        assert_eq!(storage.count(), 0);
        assert!(test_path.exists());

        // Clean up
        let _ = fs::remove_file(&test_path);
    }

    #[test]
    fn test_add_and_get_action() {
        let temp_dir = env::temp_dir();
        let test_path = temp_dir.join("test_actions_add.json");
        let _ = fs::remove_file(&test_path);

        let mut storage = ActionStorage::new(test_path.clone()).unwrap();

        let action = StoredAction {
            txid: "abc123".to_string(),
            reference_number: "ref123".to_string(),
            raw_tx: "010000...".to_string(),
            description: Some("Test transaction".to_string()),
            labels: vec!["test".to_string()],
            status: ActionStatus::Created,
            is_outgoing: true,
            satoshis: 50000,
            timestamp: 1698765432,
            block_height: None,
            confirmations: 0,
            version: 1,
            lock_time: 0,
            inputs: vec![],
            outputs: vec![],
            price_usd_cents: None,
        };

        storage.add_action(action.clone()).unwrap();

        let retrieved = storage.get_action("abc123").unwrap();
        assert_eq!(retrieved.reference_number, "ref123");
        assert_eq!(retrieved.satoshis, 50000);

        // Clean up
        let _ = fs::remove_file(&test_path);
    }

    #[test]
    fn test_update_status() {
        let temp_dir = env::temp_dir();
        let test_path = temp_dir.join("test_actions_update.json");
        let _ = fs::remove_file(&test_path);

        let mut storage = ActionStorage::new(test_path.clone()).unwrap();

        let action = StoredAction {
            txid: "abc123".to_string(),
            reference_number: "ref123".to_string(),
            raw_tx: "010000...".to_string(),
            description: None,
            labels: vec![],
            status: ActionStatus::Created,
            is_outgoing: true,
            satoshis: 50000,
            timestamp: 1698765432,
            block_height: None,
            confirmations: 0,
            version: 1,
            lock_time: 0,
            inputs: vec![],
            outputs: vec![],
            price_usd_cents: None,
        };

        storage.add_action(action).unwrap();
        storage.update_status("abc123", ActionStatus::Signed).unwrap();

        let retrieved = storage.get_action("abc123").unwrap();
        assert_eq!(retrieved.status, ActionStatus::Signed);

        // Clean up
        let _ = fs::remove_file(&test_path);
    }

    #[test]
    fn test_filter_by_labels() {
        let temp_dir = env::temp_dir();
        let test_path = temp_dir.join("test_actions_filter.json");
        let _ = fs::remove_file(&test_path);

        let mut storage = ActionStorage::new(test_path.clone()).unwrap();

        // Add multiple actions with different labels
        let action1 = StoredAction {
            txid: "tx1".to_string(),
            reference_number: "ref1".to_string(),
            raw_tx: "010000...".to_string(),
            description: None,
            labels: vec!["shopping".to_string(), "online".to_string()],
            status: ActionStatus::Confirmed,
            is_outgoing: true,
            satoshis: 50000,
            timestamp: 1698765432,
            block_height: Some(850000),
            confirmations: 10,
            version: 1,
            lock_time: 0,
            inputs: vec![],
            outputs: vec![],
            price_usd_cents: None,
        };

        let action2 = StoredAction {
            txid: "tx2".to_string(),
            reference_number: "ref2".to_string(),
            raw_tx: "010000...".to_string(),
            description: None,
            labels: vec!["payment".to_string()],
            status: ActionStatus::Confirmed,
            is_outgoing: false,
            satoshis: 100000,
            timestamp: 1698765433,
            block_height: Some(850001),
            confirmations: 9,
            version: 1,
            lock_time: 0,
            inputs: vec![],
            outputs: vec![],
            price_usd_cents: None,
        };

        storage.add_action(action1).unwrap();
        storage.add_action(action2).unwrap();

        // Test filtering
        let shopping_filter = vec!["shopping".to_string()];
        let results = storage.list_actions(Some(&shopping_filter), Some("any"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].txid, "tx1");

        // Clean up
        let _ = fs::remove_file(&test_path);
    }
}
