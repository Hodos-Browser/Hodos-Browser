//! BRC-43: Security Levels, Protocol IDs, Key IDs and Counterparties
//!
//! Implements the BRC-43 specification for invoice number formatting
//! and protocol ID normalization.
//!
//! **Reference**: BRC-43 specification
//! https://github.com/bitcoin-sv/BRCs/blob/master/key-derivation/0043.md

/// Security levels for BRC-43 invoice numbers
///
/// **BRC-43 Spec**: Security level component
///
/// - Level 0: No permissions required
/// - Level 1: Permission required for protocol (any counterparty)
/// - Level 2: Permission required per counterparty
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLevel {
    /// No permissions - always allowed
    NoPermissions = 0,
    
    /// Protocol permission - granted once for all counterparties
    ProtocolLevel = 1,
    
    /// Counterparty-specific permission - granted per counterparty
    CounterpartyLevel = 2,
}

impl SecurityLevel {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
    
    pub fn from_u8(level: u8) -> Option<Self> {
        match level {
            0 => Some(SecurityLevel::NoPermissions),
            1 => Some(SecurityLevel::ProtocolLevel),
            2 => Some(SecurityLevel::CounterpartyLevel),
            _ => None,
        }
    }
}

impl std::fmt::Display for SecurityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_u8())
    }
}

/// Invoice number structure for BRC-43
///
/// **BRC-43 Spec**: Format `<securityLevel>-<protocolID>-<keyID>`
///
/// ## Example
/// ```
/// use wallet_core::keys::brc43::{InvoiceNumber, SecurityLevel};
///
/// let invoice = InvoiceNumber {
///     security_level: SecurityLevel::NoPermissions,
///     protocol_id: "hello world".to_string(),
///     key_id: "1".to_string(),
/// };
///
/// assert_eq!(invoice.to_string(), "0-hello world-1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceNumber {
    pub security_level: SecurityLevel,
    pub protocol_id: String,
    pub key_id: String,
}

impl InvoiceNumber {
    /// Create a new invoice number with validation
    ///
    /// **BRC-43 Spec**: Validation rules for protocol ID and key ID
    pub fn new(
        security_level: SecurityLevel,
        protocol_id: impl Into<String>,
        key_id: impl Into<String>,
    ) -> Result<Self, String> {
        let protocol_id = protocol_id.into();
        let key_id = key_id.into();
        
        // Validate protocol ID
        let normalized_protocol_id = normalize_protocol_id(&protocol_id)?;
        
        // Validate key ID length (1-1033 bytes)
        if key_id.is_empty() || key_id.len() > 1033 {
            return Err(format!(
                "Key ID must be 1-1033 bytes, got {} bytes",
                key_id.len()
            ));
        }
        
        Ok(Self {
            security_level,
            protocol_id: normalized_protocol_id,
            key_id,
        })
    }
    
    /// Parse invoice number from string
    ///
    /// **BRC-43 Spec**: Parse format `<securityLevel>-<protocolID>-<keyID>`
    pub fn from_string(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.splitn(3, '-').collect();
        
        if parts.len() != 3 {
            return Err(format!(
                "Invalid invoice number format. Expected '<level>-<protocol>-<keyid>', got: {}",
                s
            ));
        }
        
        let level = parts[0].parse::<u8>()
            .map_err(|_| format!("Invalid security level: {}", parts[0]))?;
        
        let security_level = SecurityLevel::from_u8(level)
            .ok_or_else(|| format!("Invalid security level: {}", level))?;
        
        Self::new(security_level, parts[1], parts[2])
    }
}

impl std::fmt::Display for InvoiceNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}-{}", 
            self.security_level.as_u8(),
            self.protocol_id,
            self.key_id
        )
    }
}

/// Normalize protocol ID according to BRC-43 rules
///
/// **BRC-43 Spec**: Protocol ID normalization rules:
/// - Only letters, numbers and spaces
/// - No multiple spaces
/// - All lower case
/// - Maximum 280 characters
/// - Minimum 5 characters
/// - Must not end with " protocol"
/// - Leading and trailing spaces removed
///
/// ## Example
/// ```
/// use wallet_core::keys::brc43::normalize_protocol_id;
///
/// let normalized = normalize_protocol_id("  Hello   World  ").unwrap();
/// assert_eq!(normalized, "hello world");
/// ```
pub fn normalize_protocol_id(protocol_id: &str) -> Result<String, String> {
    // Trim leading and trailing spaces
    let mut result = protocol_id.trim().to_lowercase();
    
    // Remove multiple spaces
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }
    
    // Validate characters (only letters, numbers, spaces)
    if !result.chars().all(|c| c.is_alphanumeric() || c == ' ') {
        return Err(format!(
            "Protocol ID contains invalid characters. Only letters, numbers and spaces allowed"
        ));
    }
    
    // Check length constraints
    if result.len() < 5 {
        return Err(format!(
            "Protocol ID must be at least 5 characters, got {}",
            result.len()
        ));
    }
    
    if result.len() > 280 {
        return Err(format!(
            "Protocol ID must be at most 280 characters, got {}",
            result.len()
        ));
    }
    
    // Must not end with " protocol"
    if result.ends_with(" protocol") {
        return Err(
            "Protocol ID must not end with ' protocol'".to_string()
        );
    }
    
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_security_level_values() {
        // BRC-43: Security level constants
        assert_eq!(SecurityLevel::NoPermissions.as_u8(), 0);
        assert_eq!(SecurityLevel::ProtocolLevel.as_u8(), 1);
        assert_eq!(SecurityLevel::CounterpartyLevel.as_u8(), 2);
    }
    
    #[test]
    fn test_security_level_from_u8() {
        // BRC-43: Parse security levels
        assert_eq!(SecurityLevel::from_u8(0), Some(SecurityLevel::NoPermissions));
        assert_eq!(SecurityLevel::from_u8(1), Some(SecurityLevel::ProtocolLevel));
        assert_eq!(SecurityLevel::from_u8(2), Some(SecurityLevel::CounterpartyLevel));
        assert_eq!(SecurityLevel::from_u8(3), None);
    }
    
    #[test]
    fn test_normalize_protocol_id_basic() {
        // BRC-43: Basic normalization
        let normalized = normalize_protocol_id("Hello World").unwrap();
        assert_eq!(normalized, "hello world");
    }
    
    #[test]
    fn test_normalize_protocol_id_trim_spaces() {
        // BRC-43: Trim leading and trailing spaces
        let normalized = normalize_protocol_id("  Hello World  ").unwrap();
        assert_eq!(normalized, "hello world");
    }
    
    #[test]
    fn test_normalize_protocol_id_multiple_spaces() {
        // BRC-43: Remove multiple spaces
        let normalized = normalize_protocol_id("Hello   World").unwrap();
        assert_eq!(normalized, "hello world");
    }
    
    #[test]
    fn test_normalize_protocol_id_too_short() {
        // BRC-43: Minimum 5 characters
        let result = normalize_protocol_id("test");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_normalize_protocol_id_too_long() {
        // BRC-43: Maximum 280 characters
        let long_id = "a".repeat(281);
        let result = normalize_protocol_id(&long_id);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_normalize_protocol_id_invalid_chars() {
        // BRC-43: Only letters, numbers, spaces
        let result = normalize_protocol_id("hello-world");
        assert!(result.is_err());
        
        let result = normalize_protocol_id("hello@world");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_normalize_protocol_id_ends_with_protocol() {
        // BRC-43: Must not end with " protocol"
        let result = normalize_protocol_id("test protocol");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_invoice_number_creation() {
        // BRC-43: Create invoice number
        let invoice = InvoiceNumber::new(
            SecurityLevel::NoPermissions,
            "hello world",
            "1"
        ).unwrap();
        
        assert_eq!(invoice.security_level, SecurityLevel::NoPermissions);
        assert_eq!(invoice.protocol_id, "hello world");
        assert_eq!(invoice.key_id, "1");
    }
    
    #[test]
    fn test_invoice_number_to_string() {
        // BRC-43: Format as string
        let invoice = InvoiceNumber::new(
            SecurityLevel::NoPermissions,
            "hello world",
            "1"
        ).unwrap();
        
        assert_eq!(invoice.to_string(), "0-hello world-1");
    }
    
    #[test]
    fn test_invoice_number_from_string() {
        // BRC-43: Parse from string
        let invoice = InvoiceNumber::from_string("0-hello world-1").unwrap();
        
        assert_eq!(invoice.security_level, SecurityLevel::NoPermissions);
        assert_eq!(invoice.protocol_id, "hello world");
        assert_eq!(invoice.key_id, "1");
    }
    
    #[test]
    fn test_invoice_number_with_dashes_in_keyid() {
        // BRC-43: Key ID can contain dashes (splitn(3) handles this)
        let invoice = InvoiceNumber::from_string("1-document signing-key-with-dashes").unwrap();
        
        assert_eq!(invoice.security_level, SecurityLevel::ProtocolLevel);
        assert_eq!(invoice.protocol_id, "document signing");
        assert_eq!(invoice.key_id, "key-with-dashes");
    }
    
    #[test]
    fn test_invoice_number_key_id_validation() {
        // BRC-43: Key ID must be 1-1033 bytes
        let result = InvoiceNumber::new(SecurityLevel::NoPermissions, "hello world", "");
        assert!(result.is_err());
        
        let long_key = "a".repeat(1034);
        let result = InvoiceNumber::new(SecurityLevel::NoPermissions, "hello world", long_key);
        assert!(result.is_err());
    }
}
