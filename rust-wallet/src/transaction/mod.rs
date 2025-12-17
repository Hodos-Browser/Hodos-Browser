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

/// Encode a signed 64-bit integer as VarInt (matching TypeScript SDK's writeVarIntNum)
/// For negative numbers, adds 2^64 to convert to unsigned representation
pub fn encode_varint_signed(n: i64) -> Vec<u8> {
    if n >= 0 {
        encode_varint(n as u64)
    } else {
        // For negative numbers, TypeScript SDK adds 2^64
        // -1 becomes 2^64 - 1 = 0xFFFFFFFFFFFFFFFF = u64::MAX
        // Casting a negative i64 to u64 directly gives us the correct value
        // because the bit pattern is already correct (two's complement)
        let unsigned = n as u64;
        encode_varint(unsigned)
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

    #[test]
    fn test_varint_signed_encoding() {
        // Test -1 encoding (should match TypeScript SDK)
        // TypeScript SDK encodes -1 as: 0xFF followed by 8 bytes of 0xFF
        // This represents 2^64 - 1 = 0xFFFFFFFFFFFFFFFF in little-endian
        let encoded_neg1 = encode_varint_signed(-1);
        assert_eq!(encoded_neg1.len(), 9, "Expected 9 bytes for -1 encoding");
        assert_eq!(encoded_neg1[0], 0xFF, "First byte should be 0xFF marker");
        assert_eq!(encoded_neg1[1..], vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                   "Remaining 8 bytes should all be 0xFF");

        // Test other negative values
        let encoded_neg2 = encode_varint_signed(-2);
        assert_eq!(encoded_neg2.len(), 9, "Expected 9 bytes for -2 encoding");
        assert_eq!(encoded_neg2[0], 0xFF, "First byte should be 0xFF marker");

        // Test positive values (should match unsigned encoding)
        assert_eq!(encode_varint_signed(0), encode_varint(0));
        assert_eq!(encode_varint_signed(100), encode_varint(100));
    }
}
