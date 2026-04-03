//! Centralized ARC transaction status classification.
//!
//! Single source of truth for ARC miner response handling.
//! All status string matching and error classification lives here.
//! See: https://bitcoin-sv.github.io/arc/api.html

/// Canonical ARC transaction status codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArcTxStatus {
    // Pre-network states (tx accepted by ARC, not yet propagated)
    Queued,
    Received,

    // Success states (tx accepted by network)
    Mined,
    SeenOnNetwork,
    AnnouncedToNetwork,
    RequestedByNetwork,
    SentToNetwork,
    AcceptedByNetwork,
    Stored,

    // Error states
    /// BEEF validation failure — ARC couldn't verify merkle proofs.
    /// Inputs are NOT spent on-chain. Safe to restore for re-broadcast.
    SeenInOrphanMempool,
    /// Inputs spent by a competing transaction on-chain.
    DoubleSpendAttempted,
    /// General rejection by miner.
    Rejected,
    /// Was mined in a block that got orphaned. Tx may still be valid.
    MinedInStaleBlock,

    /// Status not recognized.
    Unknown(String),
}

impl ArcTxStatus {
    /// Parse an ARC status string into the enum.
    pub fn parse(s: &str) -> Self {
        match s {
            "QUEUED" => Self::Queued,
            "RECEIVED" => Self::Received,
            "MINED" => Self::Mined,
            "SEEN_ON_NETWORK" => Self::SeenOnNetwork,
            "ANNOUNCED_TO_NETWORK" => Self::AnnouncedToNetwork,
            "REQUESTED_BY_NETWORK" => Self::RequestedByNetwork,
            "SENT_TO_NETWORK" => Self::SentToNetwork,
            "ACCEPTED_BY_NETWORK" => Self::AcceptedByNetwork,
            "STORED" => Self::Stored,
            "SEEN_IN_ORPHAN_MEMPOOL" => Self::SeenInOrphanMempool,
            "DOUBLE_SPEND_ATTEMPTED" => Self::DoubleSpendAttempted,
            "REJECTED" => Self::Rejected,
            "MINED_IN_STALE_BLOCK" => Self::MinedInStaleBlock,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Is this status a success (tx is queued, in mempool, or mined)?
    pub fn is_accepted(&self) -> bool {
        matches!(
            self,
            Self::Queued
                | Self::Received
                | Self::Mined
                | Self::SeenOnNetwork
                | Self::AnnouncedToNetwork
                | Self::RequestedByNetwork
                | Self::SentToNetwork
                | Self::AcceptedByNetwork
                | Self::Stored
        )
    }

    /// Is this status an error (tx will never mine from its current state)?
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            Self::SeenInOrphanMempool
                | Self::DoubleSpendAttempted
                | Self::Rejected
                | Self::MinedInStaleBlock
        )
    }

    /// Are the inputs provably spent on-chain by a *different* transaction?
    /// If true, inputs must NOT be restored as spendable.
    pub fn inputs_spent_on_chain(&self) -> bool {
        matches!(self, Self::DoubleSpendAttempted)
    }

    /// Is this a BEEF validation failure (inputs exist but BEEF ancestry was wrong)?
    /// Inputs are safe to restore and re-broadcast is worthwhile.
    pub fn is_beef_validation_failure(&self) -> bool {
        matches!(self, Self::SeenInOrphanMempool | Self::MinedInStaleBlock)
    }

    /// Is the tx in mempool (accepted, not yet mined)?
    pub fn is_in_mempool(&self) -> bool {
        matches!(
            self,
            Self::SeenOnNetwork
                | Self::AnnouncedToNetwork
                | Self::RequestedByNetwork
                | Self::SentToNetwork
                | Self::AcceptedByNetwork
                | Self::Stored
        )
    }
}

/// Classify whether a broadcast error string is fatal (tx invalid, stop retrying)
/// or transient (network issue, worth retrying).
///
/// This is the SINGLE source of truth, replacing the duplicate functions in
/// handlers.rs and task_send_waiting.rs.
pub fn is_fatal_broadcast_error(error: &str) -> bool {
    let lower = error.to_lowercase();

    // Script verification failures
    if lower.contains("error: 16") || lower.contains("mandatory-script-verify") {
        return true;
    }
    // Double-spend / conflicting transaction
    if lower.contains("double spend") || lower.contains("double-spend")
        || lower.contains("txn-mempool-conflict")
    {
        return true;
    }
    // Missing inputs (UTXOs already consumed or BEEF ancestry failure)
    if lower.contains("missing inputs") || lower.contains("missingorspent") {
        return true;
    }
    // Transaction too large or dust outputs
    if lower.contains("tx-size") || lower.contains("dust") {
        return true;
    }
    // Non-standard / policy rejection
    if lower.contains("non-mandatory-script-verify") || lower.contains("scriptpubkey") {
        return true;
    }
    // Orphan mempool (BEEF ancestry validation failure)
    if lower.contains("orphan mempool") || lower.contains("orphan_mempool") {
        return true;
    }
    // Frozen inputs (policy or consensus blacklist — inputs can never be spent)
    if lower.contains("input frozen") || lower.contains("frozen policy")
        || lower.contains("frozen consensus")
    {
        return true;
    }

    false
}

/// Classify whether a broadcast error indicates frozen/blacklisted inputs.
/// Frozen inputs can NEVER be spent — they should be marked as permanently unspendable.
pub fn is_frozen_input_error(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("input frozen") || lower.contains("frozen policy")
        || lower.contains("frozen consensus")
}

/// Classify whether a broadcast error indicates inputs are permanently unspendable.
///
/// Covers two cases:
/// 1. Double-spend: inputs spent on-chain by a competing transaction
/// 2. Frozen inputs: inputs blacklisted by policy or consensus (can never be spent)
///
/// Used to decide between restore-inputs (safe default) vs mark-as-externally-spent.
/// "Missing inputs" is intentionally NOT included — it's ambiguous (could be BEEF
/// validation failure OR actual double-spend). The safe default is to restore inputs;
/// TaskUnFail/TaskValidateUtxos will catch genuine on-chain spends.
pub fn is_double_spend_error(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("double spend")
        || lower.contains("double-spend")
        || lower.contains("txn-mempool-conflict")
        || is_frozen_input_error(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_known_statuses() {
        assert_eq!(ArcTxStatus::parse("QUEUED"), ArcTxStatus::Queued);
        assert_eq!(ArcTxStatus::parse("RECEIVED"), ArcTxStatus::Received);
        assert_eq!(ArcTxStatus::parse("MINED"), ArcTxStatus::Mined);
        assert_eq!(ArcTxStatus::parse("SEEN_ON_NETWORK"), ArcTxStatus::SeenOnNetwork);
        assert_eq!(ArcTxStatus::parse("SEEN_IN_ORPHAN_MEMPOOL"), ArcTxStatus::SeenInOrphanMempool);
        assert_eq!(ArcTxStatus::parse("DOUBLE_SPEND_ATTEMPTED"), ArcTxStatus::DoubleSpendAttempted);
        assert_eq!(ArcTxStatus::parse("REJECTED"), ArcTxStatus::Rejected);
        assert_eq!(ArcTxStatus::parse("MINED_IN_STALE_BLOCK"), ArcTxStatus::MinedInStaleBlock);
        assert_eq!(ArcTxStatus::parse("STORED"), ArcTxStatus::Stored);
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(ArcTxStatus::parse("SOMETHING_NEW"), ArcTxStatus::Unknown("SOMETHING_NEW".to_string()));
    }

    #[test]
    fn test_accepted_statuses() {
        assert!(ArcTxStatus::Queued.is_accepted());
        assert!(ArcTxStatus::Received.is_accepted());
        assert!(ArcTxStatus::Mined.is_accepted());
        assert!(ArcTxStatus::SeenOnNetwork.is_accepted());
        assert!(ArcTxStatus::Stored.is_accepted());
        assert!(!ArcTxStatus::SeenInOrphanMempool.is_accepted());
        assert!(!ArcTxStatus::DoubleSpendAttempted.is_accepted());
        assert!(!ArcTxStatus::Rejected.is_accepted());
        assert!(!ArcTxStatus::MinedInStaleBlock.is_accepted());
    }

    #[test]
    fn test_error_statuses() {
        assert!(ArcTxStatus::SeenInOrphanMempool.is_error());
        assert!(ArcTxStatus::DoubleSpendAttempted.is_error());
        assert!(ArcTxStatus::Rejected.is_error());
        assert!(ArcTxStatus::MinedInStaleBlock.is_error());
        assert!(!ArcTxStatus::Mined.is_error());
        assert!(!ArcTxStatus::SeenOnNetwork.is_error());
    }

    #[test]
    fn test_inputs_spent_on_chain() {
        assert!(ArcTxStatus::DoubleSpendAttempted.inputs_spent_on_chain());
        assert!(!ArcTxStatus::SeenInOrphanMempool.inputs_spent_on_chain());
        assert!(!ArcTxStatus::Rejected.inputs_spent_on_chain());
        assert!(!ArcTxStatus::MinedInStaleBlock.inputs_spent_on_chain());
    }

    #[test]
    fn test_beef_validation_failure() {
        assert!(ArcTxStatus::SeenInOrphanMempool.is_beef_validation_failure());
        assert!(ArcTxStatus::MinedInStaleBlock.is_beef_validation_failure());
        assert!(!ArcTxStatus::DoubleSpendAttempted.is_beef_validation_failure());
        assert!(!ArcTxStatus::Rejected.is_beef_validation_failure());
    }

    #[test]
    fn test_fatal_broadcast_error() {
        assert!(is_fatal_broadcast_error("ERROR: 16, something bad"));
        assert!(is_fatal_broadcast_error("mandatory-script-verify-flag-failed"));
        assert!(is_fatal_broadcast_error("Transaction double spend attempted"));
        assert!(is_fatal_broadcast_error("double-spend detected"));
        assert!(is_fatal_broadcast_error("txn-mempool-conflict"));
        assert!(is_fatal_broadcast_error("Missing inputs for tx"));
        assert!(is_fatal_broadcast_error("Transaction in orphan mempool"));
        assert!(is_fatal_broadcast_error("Input Frozen (471): policy blacklist"));
        assert!(is_fatal_broadcast_error("Frozen Policy violation"));
        assert!(is_fatal_broadcast_error("Frozen Consensus: blacklisted asset"));
        assert!(!is_fatal_broadcast_error("Connection timeout"));
        assert!(!is_fatal_broadcast_error("HTTP 502 Bad Gateway"));
        // Fee-too-low is NOT fatal (could succeed with higher fee)
        assert!(!is_fatal_broadcast_error("Cumulative fee too low"));
    }

    #[test]
    fn test_double_spend_error() {
        assert!(is_double_spend_error("Transaction double spend attempted"));
        assert!(is_double_spend_error("double-spend detected"));
        assert!(is_double_spend_error("txn-mempool-conflict"));
        // Frozen inputs are permanently unspendable — same treatment as double-spend
        assert!(is_double_spend_error("Input Frozen (471): policy blacklist"));
        assert!(is_double_spend_error("Frozen Consensus: blacklisted"));
        // Missing inputs is NOT double-spend (could be BEEF validation failure)
        assert!(!is_double_spend_error("Missing inputs"));
        assert!(!is_double_spend_error("missingorspent"));
        assert!(!is_double_spend_error("orphan mempool"));
    }

    #[test]
    fn test_frozen_input_error() {
        assert!(is_frozen_input_error("Input Frozen (471): policy blacklist"));
        assert!(is_frozen_input_error("Input Frozen (472): consensus blacklist"));
        assert!(is_frozen_input_error("Frozen Policy violation"));
        assert!(is_frozen_input_error("Frozen Consensus: blacklisted asset"));
        assert!(!is_frozen_input_error("double spend"));
        assert!(!is_frozen_input_error("missing inputs"));
    }
}
