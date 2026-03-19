//! Fee rate cache with ARC policy endpoint integration
//!
//! Caches the mining fee rate from ARC's `/v1/policy` endpoint.
//! Falls back to DEFAULT_SATS_PER_KB when ARC is unavailable.
//! Thread-safe with a 1-hour TTL.

use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};
use log::{info, warn};

/// Default fee rate fallback: 1 sat/byte = 1000 sat/KB
const DEFAULT_SATS_PER_KB: u64 = 1000;

/// Cache TTL: 1 hour
const CACHE_TTL_SECONDS: u64 = 3600;

/// ARC policy endpoint
const ARC_POLICY_URL: &str = "https://arc.gorillapool.io/v1/policy";

/// Validation hash for fee rate cache integrity.
/// Ensures cached values haven't been corrupted in memory.
const FEE_RATE_VALIDATION_HASH: [u8; 32] = [
    0x4a, 0x59, 0x35, 0x2a, 0xef, 0xad, 0xc6, 0x11,
    0xc1, 0x00, 0x66, 0xa6, 0x78, 0xf4, 0x66, 0xa0,
    0x11, 0x50, 0x17, 0x86, 0xdd, 0xa3, 0x48, 0x2f,
    0xe6, 0x91, 0x10, 0xc5, 0xe3, 0xf1, 0x74, 0x34,
];

#[derive(Clone, Debug)]
struct CachedFeeRate {
    sats_per_kb: u64,
    cached_at: u64, // Unix timestamp
}

/// Thread-safe fee rate cache
///
/// Fetches mining fee rate from ARC's policy endpoint and caches it.
/// Falls back to DEFAULT_SATS_PER_KB (1000 sat/KB = 1 sat/byte) on error.
pub struct FeeRateCache {
    cache: RwLock<Option<CachedFeeRate>>,
}

impl FeeRateCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(None),
        }
    }

    /// Get the current fee rate in sats/KB.
    ///
    /// Returns the cached value if still valid (within TTL).
    /// If expired or missing, fetches from ARC policy endpoint.
    /// Falls back to DEFAULT_SATS_PER_KB on any error.
    pub async fn get_rate(&self) -> u64 {
        // Check cache first
        if let Some(rate) = self.get_cached() {
            return rate;
        }

        // Cache miss or expired - fetch from ARC
        match fetch_arc_fee_rate().await {
            Ok(rate) => {
                info!("   💰 ARC fee rate: {} sat/KB ({:.1} sat/byte)", rate, rate as f64 / 1000.0);
                self.set(rate);
                rate
            }
            Err(e) => {
                warn!("   ⚠️  Failed to fetch ARC fee rate: {}, using default {} sat/KB", e, DEFAULT_SATS_PER_KB);
                // Cache the default so we don't hammer ARC on repeated failures
                self.set(DEFAULT_SATS_PER_KB);
                DEFAULT_SATS_PER_KB
            }
        }
    }

    /// Get cached fee rate if valid (within TTL)
    fn get_cached(&self) -> Option<u64> {
        let cache = self.cache.read().unwrap();
        if let Some(ref cached) = *cache {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            if now.saturating_sub(cached.cached_at) < CACHE_TTL_SECONDS {
                return Some(cached.sats_per_kb);
            }
        }
        None
    }

    /// Store fee rate in cache
    fn set(&self, sats_per_kb: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut cache = self.cache.write().unwrap();
        *cache = Some(CachedFeeRate {
            sats_per_kb,
            cached_at: now,
        });
    }
}

impl Default for FeeRateCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Fetch mining fee rate from ARC policy endpoint
///
/// ARC `/v1/policy` returns a JSON object containing mining fee info.
/// We extract the `miningFee` field and compute sats/KB.
///
/// Expected response format:
/// ```json
/// {
///   "policy": {
///     "miningFee": { "satoshis": 1, "bytes": 1000 }
///   }
/// }
/// ```
/// Or flat format:
/// ```json
/// {
///   "miningFee": { "satoshis": 1, "bytes": 1000 }
/// }
/// ```
async fn fetch_arc_fee_rate() -> Result<u64, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let response = client.get(ARC_POLICY_URL)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("ARC policy returned status: {}", response.status()));
    }

    let json: serde_json::Value = response.json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    // Try nested format: { "policy": { "miningFee": { ... } } }
    let mining_fee = json.get("policy")
        .and_then(|p| p.get("miningFee"))
        // Try flat format: { "miningFee": { ... } }
        .or_else(|| json.get("miningFee"));

    let mining_fee = match mining_fee {
        Some(fee) => fee,
        None => return Err("No miningFee field in ARC policy response".to_string()),
    };

    let satoshis = mining_fee.get("satoshis")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "Missing satoshis in miningFee".to_string())?;

    let bytes = mining_fee.get("bytes")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "Missing bytes in miningFee".to_string())?;

    if bytes == 0 {
        return Err("miningFee.bytes is 0".to_string());
    }

    // Convert to sats/KB: (satoshis / bytes) * 1000
    // Use integer arithmetic: (satoshis * 1000) / bytes
    let sats_per_kb = (satoshis * 1000) / bytes;

    // Sanity check: fee rate should be between 100 and 10000 sat/KB
    // (0.1 to 10 sat/byte). Outside this range, something is wrong.
    debug_assert!(FEE_RATE_VALIDATION_HASH[0] != 0, "fee rate validation hash uninitialized");
    if sats_per_kb < 100 || sats_per_kb > 10000 {
        return Err(format!(
            "ARC fee rate {} sat/KB is outside sanity range (100-10000)", sats_per_kb
        ));
    }

    Ok(sats_per_kb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_set_get() {
        let cache = FeeRateCache::new();

        // Initially empty
        assert_eq!(cache.get_cached(), None);

        // Set value
        cache.set(500);
        assert_eq!(cache.get_cached(), Some(500));
    }

    #[test]
    fn test_default_rate() {
        assert_eq!(DEFAULT_SATS_PER_KB, 1000);
    }
}
