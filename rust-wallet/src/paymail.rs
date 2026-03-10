//! Paymail (bsvalias) Client
//!
//! Resolves human-readable paymail addresses (alice@handcash.io, $handle) to
//! P2PKH output scripts via the bsvalias protocol. Supports both P2P payment
//! destinations (instant receiver notification) and basic paymentDestination fallback.
//!
//! Reference: https://docs.moneybutton.com/docs/paymail-overview.html

use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;

/// Known DNS SRV overrides — avoids needing a DNS resolver crate.
/// HandCash uses cloud.handcash.io but SRV record is on handcash.io.
const SRV_OVERRIDES: &[(&str, &str)] = &[
    ("handcash.io", "cloud.handcash.io"),
];

/// BRFC capability IDs
const CAP_P2P_DESTINATION: &str = "2a40af698840";  // P2P Payment Destination
const CAP_P2P_RECEIVE_TX: &str = "5f1323cddf31";   // P2P Receive Transaction
const CAP_PUBLIC_PROFILE: &str = "f12f968c92d6";    // Public Profile

/// Cache TTL for capability discovery (1 hour)
const CAPABILITY_CACHE_TTL_SECS: u64 = 3600;

/// Paymail client errors
#[derive(Debug, thiserror::Error)]
pub enum PaymailError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid paymail format: {0}")]
    InvalidFormat(String),

    #[error("Capability discovery failed for {0}: {1}")]
    CapabilityDiscovery(String, String),

    #[error("Address resolution failed: {0}")]
    AddressResolution(String),

    #[error("P2P destination failed: {0}")]
    P2PDestination(String),

    #[error("Transaction submission failed: {0}")]
    TransactionSubmission(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("No payment capability found for {0}")]
    NoPaymentCapability(String),
}

/// Cached capability URLs for a paymail domain
#[derive(Debug, Clone)]
struct CachedCapabilities {
    capabilities: PaymailCapabilities,
    fetched_at: Instant,
}

/// Parsed capability URLs from .well-known/bsvalias
#[derive(Debug, Clone)]
pub struct PaymailCapabilities {
    /// Basic payment destination URL template (paymentDestination)
    pub payment_destination_url: Option<String>,
    /// P2P payment destination URL template (2a40af698840)
    pub p2p_destination_url: Option<String>,
    /// P2P receive transaction URL template (5f1323cddf31)
    pub p2p_receive_tx_url: Option<String>,
    /// Public profile URL template (f12f968c92d6)
    pub public_profile_url: Option<String>,
}

/// P2P payment destination response
#[derive(Debug, Clone)]
pub struct P2PDestination {
    pub outputs: Vec<PaymailOutput>,
    pub reference: String,
}

/// Single output from a P2P destination response
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PaymailOutput {
    #[serde(rename = "script")]
    pub script_hex: String,
    pub satoshis: i64,
}

/// Public profile information
#[derive(Debug, Clone)]
pub struct PaymailProfile {
    pub name: String,
    pub avatar_url: Option<String>,
}

/// Combined resolution result for the resolve endpoint
#[derive(Debug, Clone, serde::Serialize)]
pub struct PaymailResolution {
    pub valid: bool,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub has_p2p: bool,
}

/// Paymail client with capability caching
pub struct PaymailClient {
    http_client: reqwest::Client,
    capability_cache: RwLock<HashMap<String, CachedCapabilities>>,
}

impl PaymailClient {
    /// Create a new PaymailClient with a 15-second HTTP timeout
    pub fn new() -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            http_client,
            capability_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Parse a paymail string into (alias, domain).
    ///
    /// Accepts:
    /// - Standard: `alice@handcash.io`
    /// - HandCash handle: `$alice` → `("alice", "handcash.io")`
    pub fn parse_paymail(input: &str) -> Result<(String, String), PaymailError> {
        let trimmed = input.trim();

        // HandCash $handle shorthand
        if trimmed.starts_with('$') {
            let alias = &trimmed[1..];
            if alias.is_empty() || alias.contains('@') || alias.contains(' ') {
                return Err(PaymailError::InvalidFormat(
                    format!("Invalid handle: {}", trimmed),
                ));
            }
            return Ok((alias.to_lowercase(), "handcash.io".to_string()));
        }

        // Standard alias@domain
        let parts: Vec<&str> = trimmed.splitn(2, '@').collect();
        if parts.len() != 2 {
            return Err(PaymailError::InvalidFormat(
                format!("Expected alias@domain, got: {}", trimmed),
            ));
        }

        let alias = parts[0].trim();
        let domain = parts[1].trim();

        if alias.is_empty() {
            return Err(PaymailError::InvalidFormat("Empty alias".to_string()));
        }
        if domain.is_empty() || !domain.contains('.') {
            return Err(PaymailError::InvalidFormat(
                format!("Invalid domain: {}", domain),
            ));
        }

        Ok((alias.to_lowercase(), domain.to_lowercase()))
    }

    /// Resolve the actual host for a paymail domain.
    /// Uses hardcoded SRV overrides for known providers (avoids DNS crate).
    fn discover_host(domain: &str) -> String {
        for &(src, dst) in SRV_OVERRIDES {
            if domain.eq_ignore_ascii_case(src) {
                debug!("PaymailClient: SRV override {} → {}", domain, dst);
                return dst.to_string();
            }
        }
        domain.to_string()
    }

    /// Fetch and cache capability URLs from .well-known/bsvalias
    async fn discover_capabilities(
        &self,
        domain: &str,
    ) -> Result<PaymailCapabilities, PaymailError> {
        // Check cache (read lock, dropped before any async work)
        {
            let cache = self.capability_cache.read().unwrap();
            if let Some(cached) = cache.get(domain) {
                if cached.fetched_at.elapsed().as_secs() < CAPABILITY_CACHE_TTL_SECS {
                    debug!("PaymailClient: capability cache hit for {}", domain);
                    return Ok(cached.capabilities.clone());
                }
            }
        }

        let host = Self::discover_host(domain);
        let url = format!("https://{}/.well-known/bsvalias", host);
        debug!("PaymailClient: discovering capabilities at {}", url);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                PaymailError::CapabilityDiscovery(domain.to_string(), e.to_string())
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            return Err(PaymailError::CapabilityDiscovery(
                domain.to_string(),
                format!("HTTP {}", status),
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| {
                PaymailError::CapabilityDiscovery(domain.to_string(), e.to_string())
            })?;

        let caps_obj = json.get("capabilities").unwrap_or(&json);

        let capabilities = PaymailCapabilities {
            payment_destination_url: caps_obj
                .get("paymentDestination")
                .or_else(|| caps_obj.get("5f1323cddf31")) // fallback BRFC
                .and_then(|v| v.as_str())
                .map(String::from),
            p2p_destination_url: caps_obj
                .get(CAP_P2P_DESTINATION)
                .and_then(|v| v.as_str())
                .map(String::from),
            p2p_receive_tx_url: caps_obj
                .get(CAP_P2P_RECEIVE_TX)
                .and_then(|v| v.as_str())
                .map(String::from),
            public_profile_url: caps_obj
                .get(CAP_PUBLIC_PROFILE)
                .and_then(|v| v.as_str())
                .map(String::from),
        };

        info!(
            "PaymailClient: {} capabilities — P2P: {}, basic: {}, profile: {}",
            domain,
            capabilities.p2p_destination_url.is_some(),
            capabilities.payment_destination_url.is_some(),
            capabilities.public_profile_url.is_some(),
        );

        // Update cache (write lock, dropped immediately)
        {
            let mut cache = self.capability_cache.write().unwrap();
            cache.insert(
                domain.to_string(),
                CachedCapabilities {
                    capabilities: capabilities.clone(),
                    fetched_at: Instant::now(),
                },
            );
        }

        Ok(capabilities)
    }

    /// Expand a bsvalias URL template, replacing {alias} and {domain.tld}
    fn expand_url(template: &str, alias: &str, domain: &str) -> String {
        template
            .replace("{alias}", alias)
            .replace("{domain.tld}", domain)
            .replace("{name}", alias)
    }

    /// Get P2P payment destination (preferred path).
    ///
    /// POST to the P2P destination endpoint with the payment amount.
    /// Returns output scripts and a reference string for receiver notification.
    pub async fn get_p2p_destination(
        &self,
        alias: &str,
        domain: &str,
        satoshis: i64,
    ) -> Result<P2PDestination, PaymailError> {
        let caps = self.discover_capabilities(domain).await?;

        let url_template = caps.p2p_destination_url.ok_or_else(|| {
            PaymailError::P2PDestination(format!("{} does not support P2P", domain))
        })?;

        let url = Self::expand_url(&url_template, alias, domain);
        debug!("PaymailClient: P2P destination POST {}", url);

        let body = serde_json::json!({ "satoshis": satoshis });

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| PaymailError::P2PDestination(e.to_string()))?;

        let status = response.status().as_u16();
        if status != 200 {
            let text = response.text().await.unwrap_or_default();
            return Err(PaymailError::P2PDestination(format!(
                "HTTP {} — {}",
                status, text
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| PaymailError::P2PDestination(e.to_string()))?;

        let outputs: Vec<PaymailOutput> = serde_json::from_value(
            json.get("outputs")
                .cloned()
                .ok_or_else(|| PaymailError::P2PDestination("missing outputs".to_string()))?,
        )
        .map_err(|e| PaymailError::P2PDestination(format!("bad outputs: {}", e)))?;

        let reference = json
            .get("reference")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if outputs.is_empty() {
            return Err(PaymailError::P2PDestination(
                "empty outputs array".to_string(),
            ));
        }

        info!(
            "PaymailClient: P2P destination OK — {} outputs, ref={}...",
            outputs.len(),
            &reference[..reference.len().min(20)]
        );

        Ok(P2PDestination { outputs, reference })
    }

    /// Resolve a basic payment destination (fallback path).
    ///
    /// POST to the paymentDestination endpoint.
    /// Returns a single output script hex string.
    pub async fn resolve_address(
        &self,
        alias: &str,
        domain: &str,
        satoshis: i64,
        sender_display_name: &str,
    ) -> Result<String, PaymailError> {
        let caps = self.discover_capabilities(domain).await?;

        let url_template = caps.payment_destination_url.ok_or_else(|| {
            PaymailError::NoPaymentCapability(format!("{}@{}", alias, domain))
        })?;

        let url = Self::expand_url(&url_template, alias, domain);
        debug!("PaymailClient: basic paymentDestination POST {}", url);

        // Basic paymentDestination requires senderName and dt
        let dt = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let sender_label = format!("{}'s Hodos Wallet", sender_display_name);

        let body = serde_json::json!({
            "senderName": sender_label,
            "senderHandle": sender_label,
            "dt": dt,
            "amount": satoshis,
            "purpose": "payment",
        });

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| PaymailError::AddressResolution(e.to_string()))?;

        let status = response.status().as_u16();
        if status != 200 {
            let text = response.text().await.unwrap_or_default();
            return Err(PaymailError::AddressResolution(format!(
                "HTTP {} — {}",
                status, text
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| PaymailError::AddressResolution(e.to_string()))?;

        // Response has "output" (script hex) field
        let script_hex = json
            .get("output")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                PaymailError::AddressResolution("missing 'output' in response".to_string())
            })?
            .to_string();

        if script_hex.is_empty() {
            return Err(PaymailError::AddressResolution(
                "empty output script".to_string(),
            ));
        }

        info!(
            "PaymailClient: basic resolution OK — script {}...{}",
            &script_hex[..script_hex.len().min(10)],
            &script_hex[script_hex.len().saturating_sub(6)..]
        );

        Ok(script_hex)
    }

    /// Submit a transaction to the receiver's P2P receive-tx endpoint.
    ///
    /// This notifies the receiver that a payment was made. Non-fatal if it fails
    /// since the transaction is already broadcast on-chain.
    pub async fn submit_transaction(
        &self,
        alias: &str,
        domain: &str,
        raw_tx_hex: &str,
        reference: &str,
        sender_label: &str,
    ) -> Result<(), PaymailError> {
        let caps = self.discover_capabilities(domain).await?;

        let url_template = match caps.p2p_receive_tx_url {
            Some(u) => u,
            None => {
                debug!("PaymailClient: no receive-tx endpoint for {}", domain);
                return Ok(());
            }
        };

        let url = Self::expand_url(&url_template, alias, domain);
        debug!("PaymailClient: submit_transaction POST {}", url);

        let body = serde_json::json!({
            "hex": raw_tx_hex,
            "reference": reference,
            "metadata": {
                "sender": sender_label,
                "note": format!("Payment from {}", sender_label)
            }
        });

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| PaymailError::TransactionSubmission(e.to_string()))?;

        let status = response.status().as_u16();
        if status != 200 {
            let text = response.text().await.unwrap_or_default();
            warn!(
                "PaymailClient: receive-tx returned HTTP {} — {}",
                status, text
            );
            return Err(PaymailError::TransactionSubmission(format!(
                "HTTP {}",
                status
            )));
        }

        info!("PaymailClient: receive-tx submitted OK to {}@{}", alias, domain);
        Ok(())
    }

    /// Fetch the public profile for a paymail address.
    /// Returns None on any failure (non-fatal).
    pub async fn get_profile(
        &self,
        alias: &str,
        domain: &str,
    ) -> Option<PaymailProfile> {
        let caps = match self.discover_capabilities(domain).await {
            Ok(c) => c,
            Err(_) => return None,
        };

        let url_template = caps.public_profile_url?;
        let url = Self::expand_url(&url_template, alias, domain);
        debug!("PaymailClient: profile GET {}", url);

        let response = match self.http_client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => return None,
        };

        if response.status().as_u16() != 200 {
            return None;
        }

        let json: serde_json::Value = match response.json().await {
            Ok(j) => j,
            Err(_) => return None,
        };

        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let avatar_url = json
            .get("avatar")
            .or_else(|| json.get("avatarUrl"))
            .or_else(|| json.get("avatar_url"))
            .and_then(|v| v.as_str())
            .map(String::from);

        Some(PaymailProfile { name, avatar_url })
    }

    /// Combined validation + profile resolution for the resolve endpoint.
    ///
    /// Returns a PaymailResolution struct (always succeeds at HTTP level;
    /// `valid: false` when the paymail can't be resolved).
    pub async fn resolve(&self, paymail_input: &str) -> PaymailResolution {
        let (alias, domain) = match Self::parse_paymail(paymail_input) {
            Ok(p) => p,
            Err(_) => {
                return PaymailResolution {
                    valid: false,
                    name: None,
                    avatar_url: None,
                    has_p2p: false,
                };
            }
        };

        let caps = match self.discover_capabilities(&domain).await {
            Ok(c) => c,
            Err(e) => {
                debug!("PaymailClient: resolve failed for {}@{}: {}", alias, domain, e);
                return PaymailResolution {
                    valid: false,
                    name: None,
                    avatar_url: None,
                    has_p2p: false,
                };
            }
        };

        let has_p2p = caps.p2p_destination_url.is_some();
        let has_basic = caps.payment_destination_url.is_some();

        if !has_p2p && !has_basic {
            return PaymailResolution {
                valid: false,
                name: None,
                avatar_url: None,
                has_p2p: false,
            };
        }

        // Fetch profile (non-fatal)
        let profile = self.get_profile(&alias, &domain).await;

        PaymailResolution {
            valid: true,
            name: profile.as_ref().map(|p| p.name.clone()),
            avatar_url: profile.and_then(|p| p.avatar_url),
            has_p2p,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_paymail_standard() {
        let (alias, domain) = PaymailClient::parse_paymail("alice@handcash.io").unwrap();
        assert_eq!(alias, "alice");
        assert_eq!(domain, "handcash.io");
    }

    #[test]
    fn test_parse_paymail_uppercase() {
        let (alias, domain) = PaymailClient::parse_paymail("Alice@HandCash.IO").unwrap();
        assert_eq!(alias, "alice");
        assert_eq!(domain, "handcash.io");
    }

    #[test]
    fn test_parse_paymail_handcash_handle() {
        let (alias, domain) = PaymailClient::parse_paymail("$alice").unwrap();
        assert_eq!(alias, "alice");
        assert_eq!(domain, "handcash.io");
    }

    #[test]
    fn test_parse_paymail_handcash_handle_uppercase() {
        let (alias, domain) = PaymailClient::parse_paymail("$Brandon").unwrap();
        assert_eq!(alias, "brandon");
        assert_eq!(domain, "handcash.io");
    }

    #[test]
    fn test_parse_paymail_whitespace() {
        let (alias, domain) = PaymailClient::parse_paymail("  bob@example.com  ").unwrap();
        assert_eq!(alias, "bob");
        assert_eq!(domain, "example.com");
    }

    #[test]
    fn test_parse_paymail_invalid_no_at() {
        assert!(PaymailClient::parse_paymail("notapaymail").is_err());
    }

    #[test]
    fn test_parse_paymail_invalid_empty_alias() {
        assert!(PaymailClient::parse_paymail("@handcash.io").is_err());
    }

    #[test]
    fn test_parse_paymail_invalid_no_dot_domain() {
        assert!(PaymailClient::parse_paymail("alice@localhost").is_err());
    }

    #[test]
    fn test_parse_paymail_invalid_empty_handle() {
        assert!(PaymailClient::parse_paymail("$").is_err());
    }

    #[test]
    fn test_parse_paymail_invalid_handle_with_at() {
        assert!(PaymailClient::parse_paymail("$alice@domain").is_err());
    }

    #[test]
    fn test_discover_host_override() {
        assert_eq!(
            PaymailClient::discover_host("handcash.io"),
            "cloud.handcash.io"
        );
    }

    #[test]
    fn test_discover_host_passthrough() {
        assert_eq!(
            PaymailClient::discover_host("simply.cash"),
            "simply.cash"
        );
    }

    #[test]
    fn test_discover_host_case_insensitive() {
        assert_eq!(
            PaymailClient::discover_host("HandCash.IO"),
            "cloud.handcash.io"
        );
    }

    #[test]
    fn test_expand_url_both_placeholders() {
        let template = "https://example.com/api/v1/address/{alias}@{domain.tld}";
        let result = PaymailClient::expand_url(template, "alice", "example.com");
        assert_eq!(result, "https://example.com/api/v1/address/alice@example.com");
    }

    #[test]
    fn test_expand_url_name_placeholder() {
        let template = "https://example.com/profile/{name}";
        let result = PaymailClient::expand_url(template, "bob", "example.com");
        assert_eq!(result, "https://example.com/profile/bob");
    }

    #[test]
    fn test_expand_url_no_placeholders() {
        let template = "https://example.com/static";
        let result = PaymailClient::expand_url(template, "alice", "example.com");
        assert_eq!(result, "https://example.com/static");
    }
}
