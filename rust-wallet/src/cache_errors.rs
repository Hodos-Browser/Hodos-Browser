//! Unified error types for BEEF/SPV caching operations
//!
//! Provides a consistent error type that can be used across all cache operations,
//! allowing the `?` operator to work seamlessly.

use std::fmt;

#[derive(Debug)]
pub enum CacheError {
    Database(rusqlite::Error),
    Api(String),
    InvalidData(String),
    HexDecode(hex::FromHexError),
    Json(serde_json::Error),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CacheError::Database(e) => write!(f, "Database error: {}", e),
            CacheError::Api(e) => write!(f, "API error: {}", e),
            CacheError::InvalidData(e) => write!(f, "Invalid data: {}", e),
            CacheError::HexDecode(e) => write!(f, "Hex decode error: {}", e),
            CacheError::Json(e) => write!(f, "JSON error: {}", e),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<rusqlite::Error> for CacheError {
    fn from(err: rusqlite::Error) -> Self {
        CacheError::Database(err)
    }
}

impl From<hex::FromHexError> for CacheError {
    fn from(err: hex::FromHexError) -> Self {
        CacheError::HexDecode(err)
    }
}

impl From<serde_json::Error> for CacheError {
    fn from(err: serde_json::Error) -> Self {
        CacheError::Json(err)
    }
}

pub type CacheResult<T> = Result<T, CacheError>;
