//! Bitcoin Script parser
//!
//! Parses Bitcoin scripts into chunks (opcodes and data pushes).

use std::fmt;

/// Represents a single chunk of a Bitcoin script
#[derive(Debug, Clone)]
pub struct ScriptChunk {
    pub op: u8,
    pub data: Option<Vec<u8>>,
}

impl ScriptChunk {
    pub fn new(op: u8, data: Option<Vec<u8>>) -> Self {
        ScriptChunk { op, data }
    }
}

/// Bitcoin Script opcodes
pub mod opcodes {
    // Push value opcodes
    pub const OP_0: u8 = 0x00;
    pub const OP_FALSE: u8 = 0x00;
    pub const OP_PUSHDATA1: u8 = 0x4c;
    pub const OP_PUSHDATA2: u8 = 0x4d;
    pub const OP_PUSHDATA4: u8 = 0x4e;
    pub const OP_1NEGATE: u8 = 0x4f;
    pub const OP_RESERVED: u8 = 0x50;
    pub const OP_TRUE: u8 = 0x51;
    pub const OP_1: u8 = 0x51;
    pub const OP_2: u8 = 0x52;
    pub const OP_3: u8 = 0x53;
    pub const OP_4: u8 = 0x54;
    pub const OP_5: u8 = 0x55;
    pub const OP_6: u8 = 0x56;
    pub const OP_7: u8 = 0x57;
    pub const OP_8: u8 = 0x58;
    pub const OP_9: u8 = 0x59;
    pub const OP_10: u8 = 0x5a;
    pub const OP_11: u8 = 0x5b;
    pub const OP_12: u8 = 0x5c;
    pub const OP_13: u8 = 0x5d;
    pub const OP_14: u8 = 0x5e;
    pub const OP_15: u8 = 0x5f;
    pub const OP_16: u8 = 0x60;

    // Stack operations
    pub const OP_DROP: u8 = 0x75;
    pub const OP_2DROP: u8 = 0x6d;
    pub const OP_CHECKSIG: u8 = 0xac;
}

/// Error types for script parsing
#[derive(Debug, Clone)]
pub enum ScriptParseError {
    UnexpectedEndOfScript,
    InvalidPushDataLength,
    InvalidOpcode(u8),
    Other(String),
}

impl fmt::Display for ScriptParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptParseError::UnexpectedEndOfScript => {
                write!(f, "Unexpected end of script")
            }
            ScriptParseError::InvalidPushDataLength => {
                write!(f, "Invalid push data length")
            }
            ScriptParseError::InvalidOpcode(op) => {
                write!(f, "Invalid opcode: 0x{:02x}", op)
            }
            ScriptParseError::Other(msg) => {
                write!(f, "Script parse error: {}", msg)
            }
        }
    }
}

impl std::error::Error for ScriptParseError {}

/// Parse a Bitcoin script into chunks
///
/// Handles:
/// - Direct pushes (opcode 1-75 = data length)
/// - OP_PUSHDATA1 (0x4c) - 1-byte length
/// - OP_PUSHDATA2 (0x4d) - 2-byte length (little-endian)
/// - OP_PUSHDATA4 (0x4e) - 4-byte length (little-endian)
/// - Special opcodes (OP_0, OP_1-OP_16, OP_1NEGATE)
pub fn parse_script_chunks(script: &[u8]) -> Result<Vec<ScriptChunk>, ScriptParseError> {
    let mut chunks = Vec::new();
    let mut i = 0;

    while i < script.len() {
        let op = script[i];
        i += 1;

        // Direct push (opcode 1-75 = data length)
        if op >= 1 && op <= 75 {
            let data_len = op as usize;
            if i + data_len > script.len() {
                return Err(ScriptParseError::UnexpectedEndOfScript);
            }
            let data = script[i..i + data_len].to_vec();
            i += data_len;
            chunks.push(ScriptChunk::new(op, Some(data)));
        }
        // OP_PUSHDATA1 - 1-byte length
        else if op == opcodes::OP_PUSHDATA1 {
            if i >= script.len() {
                return Err(ScriptParseError::UnexpectedEndOfScript);
            }
            let data_len = script[i] as usize;
            i += 1;
            if i + data_len > script.len() {
                return Err(ScriptParseError::UnexpectedEndOfScript);
            }
            let data = script[i..i + data_len].to_vec();
            i += data_len;
            chunks.push(ScriptChunk::new(opcodes::OP_PUSHDATA1, Some(data)));
        }
        // OP_PUSHDATA2 - 2-byte length (little-endian)
        else if op == opcodes::OP_PUSHDATA2 {
            if i + 2 > script.len() {
                return Err(ScriptParseError::UnexpectedEndOfScript);
            }
            let data_len = u16::from_le_bytes([script[i], script[i + 1]]) as usize;
            i += 2;
            if i + data_len > script.len() {
                return Err(ScriptParseError::UnexpectedEndOfScript);
            }
            let data = script[i..i + data_len].to_vec();
            i += data_len;
            chunks.push(ScriptChunk::new(opcodes::OP_PUSHDATA2, Some(data)));
        }
        // OP_PUSHDATA4 - 4-byte length (little-endian)
        else if op == opcodes::OP_PUSHDATA4 {
            if i + 4 > script.len() {
                return Err(ScriptParseError::UnexpectedEndOfScript);
            }
            let data_len = u32::from_le_bytes([
                script[i],
                script[i + 1],
                script[i + 2],
                script[i + 3],
            ]) as usize;
            i += 4;
            if i + data_len > script.len() {
                return Err(ScriptParseError::UnexpectedEndOfScript);
            }
            let data = script[i..i + data_len].to_vec();
            i += data_len;
            chunks.push(ScriptChunk::new(opcodes::OP_PUSHDATA4, Some(data)));
        }
        // Special opcodes (no data)
        else {
            chunks.push(ScriptChunk::new(op, None));
        }
    }

    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_direct_push() {
        // Simple push: OP_PUSHDATA(3) + "abc"
        let script = vec![0x03, 0x61, 0x62, 0x63];
        let chunks = parse_script_chunks(&script).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].op, 0x03);
        assert_eq!(chunks[0].data, Some(vec![0x61, 0x62, 0x63]));
    }

    #[test]
    fn test_parse_op_0() {
        // OP_0
        let script = vec![opcodes::OP_0];
        let chunks = parse_script_chunks(&script).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].op, opcodes::OP_0);
        assert_eq!(chunks[0].data, None);
    }

    #[test]
    fn test_parse_op_1_through_16() {
        // OP_1 through OP_16
        for i in 1..=16 {
            let opcode = 0x50 + i;
            let script = vec![opcode];
            let chunks = parse_script_chunks(&script).unwrap();
            assert_eq!(chunks.len(), 1);
            assert_eq!(chunks[0].op, opcode);
            assert_eq!(chunks[0].data, None);
        }
    }

    #[test]
    fn test_parse_pushdata1() {
        // OP_PUSHDATA1 + length(100) + 100 bytes of data
        let mut script = vec![opcodes::OP_PUSHDATA1, 100];
        script.extend(vec![0x42; 100]);
        let chunks = parse_script_chunks(&script).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].op, opcodes::OP_PUSHDATA1);
        assert_eq!(chunks[0].data.as_ref().unwrap().len(), 100);
    }

    #[test]
    fn test_parse_pushdata2() {
        // OP_PUSHDATA2 + length(300) + 300 bytes of data
        let mut script = vec![opcodes::OP_PUSHDATA2];
        script.extend((300u16).to_le_bytes());
        script.extend(vec![0x42; 300]);
        let chunks = parse_script_chunks(&script).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].op, opcodes::OP_PUSHDATA2);
        assert_eq!(chunks[0].data.as_ref().unwrap().len(), 300);
    }
}
