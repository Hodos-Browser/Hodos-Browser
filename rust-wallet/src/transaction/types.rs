//! Core Transaction Types
//!
//! Simplified Bitcoin transaction structures based on wallet-toolbox-rs.

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// Transaction error types
#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("invalid transaction format: {0}")]
    InvalidFormat(String),

    #[error("invalid script: {0}")]
    InvalidScript(String),

    #[error("hex decode error: {0}")]
    HexDecode(#[from] hex::FromHexError),
}

pub type TransactionResult<T> = Result<T, TransactionError>;

// ============================================================================
// OutPoint - Reference to a previous transaction output
// ============================================================================

/// Transaction output point
///
/// References a specific output in a previous transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutPoint {
    /// Transaction ID (32 bytes as hex string)
    pub txid: String,

    /// Output index
    pub vout: u32,
}

impl OutPoint {
    /// Create a new OutPoint
    pub fn new(txid: impl Into<String>, vout: u32) -> Self {
        Self {
            txid: txid.into(),
            vout,
        }
    }

    /// Get txid as bytes (reversed for Bitcoin wire format)
    pub fn txid_bytes(&self) -> Result<Vec<u8>, hex::FromHexError> {
        let bytes = hex::decode(&self.txid)?;
        // Reverse for wire format (Bitcoin uses little-endian for txid)
        Ok(bytes.into_iter().rev().collect())
    }

    /// Serialize to 36 bytes (32 txid + 4 vout)
    pub fn serialize(&self) -> Result<Vec<u8>, hex::FromHexError> {
        let mut buffer = Vec::with_capacity(36);
        buffer.extend_from_slice(&self.txid_bytes()?);
        buffer.extend_from_slice(&self.vout.to_le_bytes());
        Ok(buffer)
    }
}

impl std::fmt::Display for OutPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.txid, self.vout)
    }
}

// ============================================================================
// Script - Bitcoin script operations
// ============================================================================

/// Bitcoin script
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    /// Create empty script
    pub fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    /// Create from bytes
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Create from hex
    pub fn from_hex(hex: &str) -> Result<Self, hex::FromHexError> {
        Ok(Self {
            bytes: hex::decode(hex)?,
        })
    }

    /// Get as bytes
    pub fn to_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Get as hex
    pub fn to_hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    /// Build P2PKH locking script
    ///
    /// Format: OP_DUP OP_HASH160 <pubKeyHash> OP_EQUALVERIFY OP_CHECKSIG
    pub fn p2pkh_locking_script(pub_key_hash: &[u8]) -> Result<Self, TransactionError> {
        if pub_key_hash.len() != 20 {
            return Err(TransactionError::InvalidScript(
                format!("Public key hash must be 20 bytes, got {}", pub_key_hash.len())
            ));
        }

        let mut bytes = Vec::with_capacity(25);
        bytes.push(0x76); // OP_DUP
        bytes.push(0xa9); // OP_HASH160
        bytes.push(0x14); // Push 20 bytes
        bytes.extend_from_slice(pub_key_hash);
        bytes.push(0x88); // OP_EQUALVERIFY
        bytes.push(0xac); // OP_CHECKSIG

        Ok(Self { bytes })
    }

    /// Build P2PKH unlocking script
    ///
    /// Format: <signature> <publicKey>
    pub fn p2pkh_unlocking_script(signature: &[u8], public_key: &[u8]) -> Self {
        let mut bytes = Vec::new();

        // Push signature
        bytes.push(signature.len() as u8);
        bytes.extend_from_slice(signature);

        // Push public key
        bytes.push(public_key.len() as u8);
        bytes.extend_from_slice(public_key);

        Self { bytes }
    }
}

impl Default for Script {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TxInput - Transaction input
// ============================================================================

/// Transaction input
///
/// Spends a previous transaction output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxInput {
    /// Previous output being spent
    pub prev_out: OutPoint,

    /// Unlocking script (scriptSig)
    #[serde(rename = "scriptSig")]
    pub script_sig: Vec<u8>,

    /// Sequence number (default: 0xFFFFFFFF)
    pub sequence: u32,
}

impl TxInput {
    /// Create new input (unsigned)
    pub fn new(prev_out: OutPoint) -> Self {
        Self {
            prev_out,
            script_sig: Vec::new(),
            sequence: 0xFFFFFFFF,
        }
    }

    /// Set unlocking script
    pub fn set_script(&mut self, script: Vec<u8>) {
        self.script_sig = script;
    }

    /// Serialize input for transaction
    pub fn serialize(&self) -> Result<Vec<u8>, hex::FromHexError> {
        let mut buffer = Vec::new();

        // Outpoint (36 bytes)
        buffer.extend_from_slice(&self.prev_out.serialize()?);

        // Script length (varint)
        buffer.extend_from_slice(&super::encode_varint(self.script_sig.len() as u64));

        // Script bytes
        buffer.extend_from_slice(&self.script_sig);

        // Sequence (4 bytes)
        buffer.extend_from_slice(&self.sequence.to_le_bytes());

        Ok(buffer)
    }
}

// ============================================================================
// TxOutput - Transaction output
// ============================================================================

/// Transaction output
///
/// Contains value and locking script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxOutput {
    /// Value in satoshis
    pub value: i64,

    /// Locking script (scriptPubKey)
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: Vec<u8>,
}

impl TxOutput {
    /// Create new output
    pub fn new(value: i64, script_pubkey: Vec<u8>) -> Self {
        Self {
            value,
            script_pubkey,
        }
    }

    /// Create from hex-encoded script
    pub fn from_hex_script(value: i64, script_hex: &str) -> Result<Self, hex::FromHexError> {
        let script_pubkey = hex::decode(script_hex)?;
        Ok(Self::new(value, script_pubkey))
    }

    /// Serialize output
    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Value (8 bytes, little-endian)
        buffer.extend_from_slice(&self.value.to_le_bytes());

        // Script length (varint)
        buffer.extend_from_slice(&super::encode_varint(self.script_pubkey.len() as u64));

        // Script bytes
        buffer.extend_from_slice(&self.script_pubkey);

        buffer
    }
}

// ============================================================================
// Transaction - Complete Bitcoin transaction
// ============================================================================

/// Bitcoin transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Transaction version (typically 1 or 2)
    pub version: u32,

    /// Transaction inputs
    pub inputs: Vec<TxInput>,

    /// Transaction outputs
    pub outputs: Vec<TxOutput>,

    /// Lock time
    #[serde(rename = "lockTime")]
    pub lock_time: u32,
}

impl Transaction {
    /// Create new empty transaction
    pub fn new() -> Self {
        Self {
            version: 1,
            inputs: Vec::new(),
            outputs: Vec::new(),
            lock_time: 0,
        }
    }

    /// Add input
    pub fn add_input(&mut self, input: TxInput) {
        self.inputs.push(input);
    }

    /// Add output
    pub fn add_output(&mut self, output: TxOutput) {
        self.outputs.push(output);
    }

    /// Serialize transaction to bytes
    ///
    /// Format: version + inputs + outputs + lockTime
    pub fn serialize(&self) -> TransactionResult<Vec<u8>> {
        let mut buffer = Vec::new();

        // Version (4 bytes, little-endian)
        buffer.extend_from_slice(&self.version.to_le_bytes());

        // Input count (varint)
        buffer.extend_from_slice(&super::encode_varint(self.inputs.len() as u64));

        // Inputs
        for input in &self.inputs {
            buffer.extend_from_slice(&input.serialize()?);
        }

        // Output count (varint)
        buffer.extend_from_slice(&super::encode_varint(self.outputs.len() as u64));

        // Outputs
        for output in &self.outputs {
            buffer.extend_from_slice(&output.serialize());
        }

        // Lock time (4 bytes, little-endian)
        buffer.extend_from_slice(&self.lock_time.to_le_bytes());

        Ok(buffer)
    }

    /// Get transaction as hex string
    pub fn to_hex(&self) -> TransactionResult<String> {
        Ok(hex::encode(self.serialize()?))
    }

    /// Calculate transaction ID (double SHA-256 of serialized tx)
    ///
    /// **Note:** This is the hash in reverse byte order (display format)
    pub fn txid(&self) -> TransactionResult<String> {
        let serialized = self.serialize()?;

        // Double SHA-256
        let hash1 = Sha256::digest(&serialized);
        let hash2 = Sha256::digest(&hash1);

        // Reverse for display format
        let reversed: Vec<u8> = hash2.into_iter().rev().collect();

        Ok(hex::encode(reversed))
    }
}

impl Default for Transaction {
    fn default() -> Self {
        Self::new()
    }
}
