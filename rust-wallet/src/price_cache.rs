//! BSV/USD price cache with CryptoCompare + CoinGecko fallback
//!
//! Caches the BSV/USD exchange rate for display and spending limit evaluation.
//! Primary source: CryptoCompare. Fallback: CoinGecko.
//! Thread-safe with a 5-minute TTL.

use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};
use log::{info, warn};

/// Cache TTL: 5 minutes
const CACHE_TTL_SECONDS: u64 = 300;

/// CryptoCompare API endpoint
const CRYPTOCOMPARE_URL: &str = "https://min-api.cryptocompare.com/data/price?fsym=BSV&tsyms=USD";

/// CoinGecko API endpoint (fallback)
const COINGECKO_URL: &str = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin-sv&vs_currencies=usd";

#[derive(Clone, Debug)]
struct CachedPrice {
    usd_price: f64,
    cached_at: u64, // Unix timestamp
}

/// Thread-safe BSV/USD price cache
///
/// Fetches BSV price from CryptoCompare (primary) with CoinGecko fallback.
/// Returns 0.0 if both APIs fail and no cached value exists.
pub struct PriceCache {
    cache: RwLock<Option<CachedPrice>>,
}

impl PriceCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(None),
        }
    }

    /// Get the current BSV/USD price.
    ///
    /// Returns the cached value if still valid (within TTL).
    /// If expired or missing, fetches from external APIs.
    /// Returns 0.0 only if all fetches fail and no cached value exists.
    pub async fn get_price(&self) -> f64 {
        // Check cache first
        if let Some(price) = self.get_cached() {
            return price;
        }

        // Cache miss or expired — fetch from APIs
        match fetch_bsv_price().await {
            Ok(price) => {
                info!("   💲 BSV/USD price: ${:.2}", price);
                self.set(price);
                price
            }
            Err(e) => {
                warn!("   ⚠️  Failed to fetch BSV/USD price: {}", e);
                // Return stale cache if available (better than nothing)
                if let Some(stale) = self.get_stale() {
                    warn!("   ⚠️  Using stale price: ${:.2}", stale);
                    stale
                } else {
                    0.0
                }
            }
        }
    }

    /// Get cached price if valid (within TTL)
    pub fn get_cached(&self) -> Option<f64> {
        let cache = self.cache.read().unwrap();
        if let Some(ref cached) = *cache {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            if now.saturating_sub(cached.cached_at) < CACHE_TTL_SECONDS {
                return Some(cached.usd_price);
            }
        }
        None
    }

    /// Get cached price even if expired (for fallback when APIs fail)
    fn get_stale(&self) -> Option<f64> {
        let cache = self.cache.read().unwrap();
        cache.as_ref().map(|c| c.usd_price)
    }

    /// Store price in cache
    fn set(&self, usd_price: f64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut cache = self.cache.write().unwrap();
        *cache = Some(CachedPrice {
            usd_price,
            cached_at: now,
        });
    }
}

impl Default for PriceCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Fetch BSV/USD price from external APIs
///
/// Tries CryptoCompare first, falls back to CoinGecko.
async fn fetch_bsv_price() -> Result<f64, String> {
    // Primary: CryptoCompare
    match fetch_cryptocompare().await {
        Ok(price) => return Ok(price),
        Err(e) => {
            warn!("   CryptoCompare failed: {}, trying CoinGecko...", e);
        }
    }

    // Fallback: CoinGecko
    fetch_coingecko().await
}

/// Fetch from CryptoCompare: { "USD": 52.34 }
async fn fetch_cryptocompare() -> Result<f64, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let response = client.get(CRYPTOCOMPARE_URL)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("CryptoCompare returned status: {}", response.status()));
    }

    let json: serde_json::Value = response.json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let price = json.get("USD")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "Missing USD field in CryptoCompare response".to_string())?;

    validate_price(price, "CryptoCompare")
}

/// Fetch from CoinGecko: { "bitcoin-sv": { "usd": 52.34 } }
async fn fetch_coingecko() -> Result<f64, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let response = client.get(COINGECKO_URL)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("CoinGecko returned status: {}", response.status()));
    }

    let json: serde_json::Value = response.json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let price = json.get("bitcoin-sv")
        .and_then(|v| v.get("usd"))
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "Missing bitcoin-sv.usd field in CoinGecko response".to_string())?;

    validate_price(price, "CoinGecko")
}

/// Sanity check: price should be between $0.01 and $10,000
fn validate_price(price: f64, source: &str) -> Result<f64, String> {
    if price <= 0.0 || price > 10_000.0 {
        return Err(format!(
            "{} price ${:.2} is outside sanity range ($0.01-$10,000)", source, price
        ));
    }
    Ok(price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_set_get() {
        let cache = PriceCache::new();

        // Initially empty
        assert_eq!(cache.get_cached(), None);

        // Set value
        cache.set(52.34);
        assert_eq!(cache.get_cached(), Some(52.34));
    }

    #[test]
    fn test_stale_fallback() {
        let cache = PriceCache::new();

        // No stale value initially
        assert_eq!(cache.get_stale(), None);

        // After set, stale returns value
        cache.set(50.0);
        assert_eq!(cache.get_stale(), Some(50.0));
    }

    #[test]
    fn test_validate_price() {
        assert!(validate_price(52.0, "test").is_ok());
        assert!(validate_price(0.0, "test").is_err());
        assert!(validate_price(-1.0, "test").is_err());
        assert!(validate_price(20_000.0, "test").is_err());
    }
}
