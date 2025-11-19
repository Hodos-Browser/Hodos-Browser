//! Bitcoin Transaction Implementation
//!
//! Simplified transaction types for Babbage Browser wallet.
//! Based on wallet-toolbox-rs transaction module.
//!
//! ## Modules
//! - `types` - Core transaction structures
//! - `sighash` - Signature hash calculation
//! - `builder` - Transaction building logic (Phase 2)
//! - `signer` - Transaction signing logic (Phase 3)

pub mod types;
pub mod sighash;

pub use types::{
    Transaction,
    TxInput,
    TxOutput,
    OutPoint,
    Script,
    TransactionError,
};

pub use sighash::{
    calculate_sighash,
    SIGHASH_ALL,
    SIGHASH_FORKID,
    SIGHASH_ALL_FORKID,
};

/// Encode variable-length integer (Bitcoin varint)
///
/// Used in transaction serialization for counts and lengths
pub fn encode_varint(n: u64) -> Vec<u8> {
    if n < 0xFD {
        vec![n as u8]
    } else if n <= 0xFFFF {
        let mut buf = vec![0xFD];
        buf.extend_from_slice(&(n as u16).to_le_bytes());
        buf
    } else if n <= 0xFFFFFFFF {
        let mut buf = vec![0xFE];
        buf.extend_from_slice(&(n as u32).to_le_bytes());
        buf
    } else {
        let mut buf = vec![0xFF];
        buf.extend_from_slice(&n.to_le_bytes());
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_encoding() {
        assert_eq!(encode_varint(0), vec![0]);
        assert_eq!(encode_varint(252), vec![252]);
        assert_eq!(encode_varint(253), vec![0xFD, 253, 0]);
        assert_eq!(encode_varint(65535), vec![0xFD, 0xFF, 0xFF]);
        assert_eq!(encode_varint(65536), vec![0xFE, 0, 0, 1, 0]);
    }
}
