//! PushDrop encoding/decoding (BRC-48)
//!
//! PushDrop is a Bitcoin script pattern that embeds arbitrary data
//! in transaction outputs while maintaining spendability.
//!
//! Pattern: `<data> OP_DROP <public_key> OP_CHECKSIG`
//!
//! For certificates, the certificate JSON is stored in the first PushDrop field.

use super::parser::{parse_script_chunks, ScriptChunk, opcodes, ScriptParseError};
use std::fmt;

/// Position of the locking script (pubkey + OP_CHECKSIG) relative to fields
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockPosition {
    /// Lock before fields: [pubkey, OP_CHECKSIG, field1, field2, ..., OP_DROP]
    Before,
    /// Lock after fields: [field1, field2, ..., OP_DROP, pubkey, OP_CHECKSIG]
    After,
}

/// Decoded PushDrop script
#[derive(Debug, Clone)]
pub struct PushDropDecoded {
    /// 33-byte public key used for locking
    pub locking_public_key: Vec<u8>,
    /// Extracted fields (certificate JSON is typically in fields[0])
    pub fields: Vec<Vec<u8>>,
}

/// Error types for PushDrop operations
#[derive(Debug, Clone)]
pub enum PushDropError {
    ParseError(ScriptParseError),
    InvalidScriptStructure(String),
    MissingPublicKey,
    MissingChecksig,
    Other(String),
}

impl fmt::Display for PushDropError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PushDropError::ParseError(e) => write!(f, "Script parse error: {}", e),
            PushDropError::InvalidScriptStructure(msg) => {
                write!(f, "Invalid script structure: {}", msg)
            }
            PushDropError::MissingPublicKey => write!(f, "Missing public key in script"),
            PushDropError::MissingChecksig => write!(f, "Missing OP_CHECKSIG in script"),
            PushDropError::Other(msg) => write!(f, "PushDrop error: {}", msg),
        }
    }
}

impl std::error::Error for PushDropError {}

impl From<ScriptParseError> for PushDropError {
    fn from(err: ScriptParseError) -> Self {
        PushDropError::ParseError(err)
    }
}

/// Decode a PushDrop-encoded script
///
/// Extracts the public key and embedded fields from a PushDrop script.
/// Supports both 'before' and 'after' lock positions.
pub fn decode(script: &[u8]) -> Result<PushDropDecoded, PushDropError> {
    let chunks = parse_script_chunks(script)?;

    if chunks.is_empty() {
        return Err(PushDropError::InvalidScriptStructure(
            "Empty script".to_string(),
        ));
    }

    // Determine lock position by checking first and last chunks
    let lock_position = if chunks.len() >= 2
        && chunks[0].data.is_some()
        && chunks[0].data.as_ref().unwrap().len() == 33
        && chunks[1].op == opcodes::OP_CHECKSIG
    {
        LockPosition::Before
    } else if chunks.len() >= 2
        && chunks[chunks.len() - 1].op == opcodes::OP_CHECKSIG
        && chunks[chunks.len() - 2].data.is_some()
        && chunks[chunks.len() - 2].data.as_ref().unwrap().len() == 33
    {
        LockPosition::After
    } else {
        return Err(PushDropError::InvalidScriptStructure(
            "Could not determine lock position".to_string(),
        ));
    };

    // Extract public key
    let locking_public_key = match lock_position {
        LockPosition::Before => {
            chunks[0]
                .data
                .clone()
                .ok_or(PushDropError::MissingPublicKey)?
        }
        LockPosition::After => {
            chunks[chunks.len() - 2]
                .data
                .clone()
                .ok_or(PushDropError::MissingPublicKey)?
        }
    };

    // Extract fields
    let fields = match lock_position {
        LockPosition::Before => {
            // Fields start at index 2 (skip pubkey at 0, OP_CHECKSIG at 1)
            extract_fields(&chunks, 2)?
        }
        LockPosition::After => {
            // Fields start at index 0, stop before last 2 chunks (pubkey + OP_CHECKSIG)
            extract_fields(&chunks, 0)?
        }
    };

    Ok(PushDropDecoded {
        locking_public_key,
        fields,
    })
}

/// Extract fields from script chunks until OP_DROP or OP_2DROP
fn extract_fields(chunks: &[ScriptChunk], start_index: usize) -> Result<Vec<Vec<u8>>, PushDropError> {
    let mut fields = Vec::new();

    for i in start_index..chunks.len() {
        // Check if next chunk is OP_DROP or OP_2DROP (stop condition)
        let next_opcode = if i + 1 < chunks.len() {
            Some(chunks[i + 1].op)
        } else {
            None
        };

        if let Some(next_op) = next_opcode {
            if next_op == opcodes::OP_DROP || next_op == opcodes::OP_2DROP {
                // This is the last field before DROP
                let field = extract_field_value(&chunks[i])?;
                fields.push(field);
                break;
            }
        }

        // Extract field value
        let field = extract_field_value(&chunks[i])?;
        fields.push(field);
    }

    Ok(fields)
}

/// Extract field value from a script chunk, handling special opcodes
fn extract_field_value(chunk: &ScriptChunk) -> Result<Vec<u8>, PushDropError> {
    // If chunk has data, return it
    if let Some(data) = &chunk.data {
        if !data.is_empty() {
            return Ok(data.clone());
        }
    }

    // Handle special opcodes that push values directly
    match chunk.op {
        // OP_0
        opcodes::OP_0 | opcodes::OP_FALSE => Ok(vec![0]),
        // OP_1 through OP_16 (0x51-0x60)
        op if op >= opcodes::OP_1 && op <= opcodes::OP_16 => {
            Ok(vec![op - 0x50]) // Convert 0x51-0x60 to 1-16
        }
        // OP_1NEGATE
        opcodes::OP_1NEGATE => Ok(vec![0x81]),
        // Other opcodes with no data (empty field)
        _ => {
            // For opcodes without data, return empty field
            Ok(vec![])
        }
    }
}

/// Create a minimally encoded script chunk for a data push
///
/// Implements the minimal encoding logic from TypeScript:
/// - Empty or [0] → OP_0
/// - [1-16] → OP_1 through OP_16
/// - [0x81] → OP_1NEGATE
/// - length <= 75 → Direct push (opcode = length)
/// - length <= 255 → OP_PUSHDATA1 + length + data
/// - length <= 65535 → OP_PUSHDATA2 + length (2 bytes) + data
/// - length > 65535 → OP_PUSHDATA4 + length (4 bytes) + data
pub fn create_minimally_encoded_chunk(data: &[u8]) -> Vec<u8> {
    // Empty or [0] → OP_0
    if data.is_empty() || (data.len() == 1 && data[0] == 0) {
        return vec![opcodes::OP_0];
    }

    // [1-16] → OP_1 through OP_16
    if data.len() == 1 && data[0] >= 1 && data[0] <= 16 {
        return vec![opcodes::OP_1 + (data[0] - 1)];
    }

    // [0x81] → OP_1NEGATE
    if data.len() == 1 && data[0] == 0x81 {
        return vec![opcodes::OP_1NEGATE];
    }

    // length <= 75 → Direct push (opcode = length)
    if data.len() <= 75 {
        let mut result = vec![data.len() as u8];
        result.extend_from_slice(data);
        return result;
    }

    // length <= 255 → OP_PUSHDATA1 + length + data
    if data.len() <= 255 {
        let mut result = vec![opcodes::OP_PUSHDATA1, data.len() as u8];
        result.extend_from_slice(data);
        return result;
    }

    // length <= 65535 → OP_PUSHDATA2 + length (2 bytes LE) + data
    if data.len() <= 65535 {
        let mut result = vec![opcodes::OP_PUSHDATA2];
        result.extend_from_slice(&(data.len() as u16).to_le_bytes());
        result.extend_from_slice(data);
        return result;
    }

    // length > 65535 → OP_PUSHDATA4 + length (4 bytes LE) + data
    let mut result = vec![opcodes::OP_PUSHDATA4];
    result.extend_from_slice(&(data.len() as u32).to_le_bytes());
    result.extend_from_slice(data);
    result
}

/// Encode fields into a PushDrop script
///
/// Creates a PushDrop-encoded script with the specified fields and public key.
pub fn encode(
    fields: &[Vec<u8>],
    public_key: &[u8],
    lock_position: LockPosition,
) -> Result<Vec<u8>, PushDropError> {
    if public_key.len() != 33 {
        return Err(PushDropError::InvalidScriptStructure(
            "Public key must be 33 bytes".to_string(),
        ));
    }

    // Create lock chunks: [pubkey, OP_CHECKSIG]
    let mut lock_chunks = Vec::new();
    lock_chunks.push(create_minimally_encoded_chunk(public_key));
    lock_chunks.push(vec![opcodes::OP_CHECKSIG]);

    // Encode fields
    let mut pushdrop_chunks = Vec::new();
    for field in fields {
        pushdrop_chunks.push(create_minimally_encoded_chunk(field));
    }

    // Add OP_2DROP for pairs, OP_DROP for remaining
    let mut not_yet_dropped = fields.len();
    while not_yet_dropped > 1 {
        pushdrop_chunks.push(vec![opcodes::OP_2DROP]);
        not_yet_dropped -= 2;
    }
    if not_yet_dropped != 0 {
        pushdrop_chunks.push(vec![opcodes::OP_DROP]);
    }

    // Combine based on lock position
    let mut script = Vec::new();
    match lock_position {
        LockPosition::Before => {
            // [lock_chunks, pushdrop_chunks]
            for chunk in lock_chunks {
                script.extend_from_slice(&chunk);
            }
            for chunk in pushdrop_chunks {
                script.extend_from_slice(&chunk);
            }
        }
        LockPosition::After => {
            // [pushdrop_chunks, lock_chunks]
            for chunk in pushdrop_chunks {
                script.extend_from_slice(&chunk);
            }
            for chunk in lock_chunks {
                script.extend_from_slice(&chunk);
            }
        }
    }

    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_before_position() {
        // Create a simple PushDrop script: [pubkey(33), OP_CHECKSIG, field1, OP_DROP]
        let pubkey = vec![0x02; 33];
        let field1 = b"hello world".to_vec();

        let script = encode(&[field1.clone()], &pubkey, LockPosition::Before).unwrap();
        let decoded = decode(&script).unwrap();

        assert_eq!(decoded.locking_public_key, pubkey);
        assert_eq!(decoded.fields.len(), 1);
        assert_eq!(decoded.fields[0], field1);
    }

    #[test]
    fn test_decode_after_position() {
        // Create a PushDrop script with lock after: [field1, OP_DROP, pubkey(33), OP_CHECKSIG]
        let pubkey = vec![0x02; 33];
        let field1 = b"test field".to_vec();

        let script = encode(&[field1.clone()], &pubkey, LockPosition::After).unwrap();
        let decoded = decode(&script).unwrap();

        assert_eq!(decoded.locking_public_key, pubkey);
        assert_eq!(decoded.fields.len(), 1);
        assert_eq!(decoded.fields[0], field1);
    }

    #[test]
    fn test_decode_multiple_fields() {
        let pubkey = vec![0x02; 33];
        let field1 = b"field1".to_vec();
        let field2 = b"field2".to_vec();
        let field3 = b"field3".to_vec();

        let script = encode(&[field1.clone(), field2.clone(), field3.clone()], &pubkey, LockPosition::Before).unwrap();
        let decoded = decode(&script).unwrap();

        assert_eq!(decoded.fields.len(), 3);
        assert_eq!(decoded.fields[0], field1);
        assert_eq!(decoded.fields[1], field2);
        assert_eq!(decoded.fields[2], field3);
    }

    #[test]
    fn test_minimal_encoding() {
        // Test OP_0
        assert_eq!(create_minimally_encoded_chunk(&[]), vec![opcodes::OP_0]);
        assert_eq!(create_minimally_encoded_chunk(&[0]), vec![opcodes::OP_0]);

        // Test OP_1 through OP_16
        assert_eq!(create_minimally_encoded_chunk(&[1]), vec![opcodes::OP_1]);
        assert_eq!(create_minimally_encoded_chunk(&[16]), vec![opcodes::OP_16]);

        // Test OP_1NEGATE
        assert_eq!(create_minimally_encoded_chunk(&[0x81]), vec![opcodes::OP_1NEGATE]);

        // Test direct push (length <= 75)
        let data_75 = vec![0x42; 75];
        let encoded = create_minimally_encoded_chunk(&data_75);
        assert_eq!(encoded[0], 75);
        assert_eq!(&encoded[1..], &data_75);

        // Test OP_PUSHDATA1 (length <= 255)
        let data_200 = vec![0x42; 200];
        let encoded = create_minimally_encoded_chunk(&data_200);
        assert_eq!(encoded[0], opcodes::OP_PUSHDATA1);
        assert_eq!(encoded[1], 200);
        assert_eq!(&encoded[2..], &data_200);
    }
}
