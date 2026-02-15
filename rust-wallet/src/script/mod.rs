//! Bitcoin Script parsing and PushDrop encoding/decoding
//!
//! This module provides utilities for parsing Bitcoin scripts and
//! encoding/decoding PushDrop-encoded data (BRC-48).

pub mod pushdrop;
pub mod parser;

#[cfg(test)]
mod pushdrop_tests;

pub use pushdrop::{decode, encode, PushDropDecoded, LockPosition, PushDropError};
pub use parser::{ScriptChunk, parse_script_chunks, ScriptParseError};
