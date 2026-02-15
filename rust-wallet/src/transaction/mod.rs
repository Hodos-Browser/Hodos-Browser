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

/// Decode a Bitcoin varint from a byte slice.
/// Returns (value, bytes_consumed).
pub fn decode_varint(data: &[u8]) -> Result<(u64, usize), TransactionError> {
    if data.is_empty() {
        return Err(TransactionError::InvalidFormat("empty varint".into()));
    }
    match data[0] {
        0..=0xFC => Ok((data[0] as u64, 1)),
        0xFD => {
            if data.len() < 3 { return Err(TransactionError::InvalidFormat("truncated varint".into())); }
            Ok((u16::from_le_bytes([data[1], data[2]]) as u64, 3))
        }
        0xFE => {
            if data.len() < 5 { return Err(TransactionError::InvalidFormat("truncated varint".into())); }
            Ok((u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as u64, 5))
        }
        0xFF => {
            if data.len() < 9 { return Err(TransactionError::InvalidFormat("truncated varint".into())); }
            Ok((u64::from_le_bytes([data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8]]), 9))
        }
    }
}

/// Extract input outpoints (prev_txid, prev_vout) from a raw transaction hex string.
/// Used by TaskUnFail to re-mark inputs as spent when recovering a false failure.
pub fn extract_input_outpoints(raw_tx_hex: &str) -> Result<Vec<(String, u32)>, TransactionError> {
    let bytes = hex::decode(raw_tx_hex)
        .map_err(|e| TransactionError::InvalidFormat(format!("hex decode: {}", e)))?;

    if bytes.len() < 5 {
        return Err(TransactionError::InvalidFormat("tx too short".into()));
    }

    let mut pos = 4; // skip version (4 bytes)

    let (num_inputs, varint_len) = decode_varint(&bytes[pos..])?;
    pos += varint_len;

    let mut outpoints = Vec::with_capacity(num_inputs as usize);

    for _ in 0..num_inputs {
        if pos + 36 > bytes.len() {
            return Err(TransactionError::InvalidFormat("truncated input".into()));
        }

        // 32 bytes prev_txid in little-endian
        let txid_bytes: Vec<u8> = bytes[pos..pos + 32].iter().rev().cloned().collect();
        let txid = hex::encode(&txid_bytes);
        pos += 32;

        // 4 bytes prev_vout in little-endian
        let vout = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
        pos += 4;

        // Skip script_sig (varint length + data)
        let (script_len, varint_len) = decode_varint(&bytes[pos..])?;
        pos += varint_len + script_len as usize;

        // Skip sequence (4 bytes)
        pos += 4;

        outpoints.push((txid, vout));
    }

    Ok(outpoints)
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
