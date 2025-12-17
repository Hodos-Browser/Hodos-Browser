//! Certificate management module
//!
//! Handles BRC-52 identity certificates including parsing, verification,
//! storage, and selective disclosure.

pub mod types;
pub mod parser;
pub mod verifier;
pub mod selective_disclosure;

#[cfg(test)]
pub mod test_utils;

pub use verifier::{serialize_certificate_preimage, verify_certificate_signature, check_revocation_status};
pub use types::{Certificate, CertificateField, CertificateError};
pub use parser::parse_certificate_from_json;

// Re-export commonly used types (for future use)
// pub use types::{Certificate, CertificateField, CertificateError};
