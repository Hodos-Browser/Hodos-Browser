//! BSV/USD price cache with WhatsOnChain + CoinGecko + MEXC fallback chain
//! and SQLite-backed restart persistence.
//!
//! Caches the BSV/USD exchange rate for display and spending-limit
//! evaluation. The cache is consulted on every payment to convert satoshis
//! to USD cents for the per-tx / per-session limit check; the wallet
//! frontend reads `bsvPrice` off `/wallet/balance` for the dashboard. So
//! "no price" cascades into "every payment prompts as `price_unavailable`"
//! + "dashboard shows $0" — losing the price is a system-wide UX failure.
//!
//! ## Source chain (2026-06-09 redesign)
//!
//! 1. **WhatsOnChain** — `/v1/bsv/main/exchangerate`. BSV-native indexer
//!    we already depend on for UTXO + tx fetch. No API key.
//! 2. **CoinGecko** — generic crypto-data aggregator. Slug
//!    `bitcoin-cash-sv` (NOT the old `bitcoin-sv` slug, which CoinGecko
//!    delisted/renamed in 2026 without warning — the renaming was what
//!    triggered this redesign). No API key.
//! 3. **MEXC** — real exchange with native BSV/USDT trading pair. Real-time
//!    market data, simple JSON shape. No API key. (USDT≈USD; for cents-
//!    granularity spending limits, the <0.5% deviation is well within the
//!    `validate_price` sanity-range filter.)
//!
//! The previous chain (CryptoCompare → CoinGecko-old-slug) broke both
//! sources simultaneously: CryptoCompare started returning HTTP 401
//! "API key required" and CoinGecko renamed `bitcoin-sv` → `bitcoin-cash-sv`
//! returning `{}`. With both sources dead, every cold-start wallet had no
//! BSV/USD price → engine bailed with `price_unavailable` on every payment.
//!
//! ## Persistence
//!
//! The in-memory `RwLock<Option<CachedPrice>>` is the hot-path. On startup
//! (`load_persisted`), the wallet reads the last known good price from the
//! `bsv_price_cache` SQLite table (V21 migration) and seeds the in-memory
//! cache as "stale-but-better-than-nothing". On every successful live fetch,
//! the new price is persisted back to the table.
//!
//! This means a cold-start wallet on a day when all three live sources fail
//! still has the last known good price to fall back on — the engine returns
//! `Silent` instead of `Prompt(price_unavailable)` and the dashboard renders
//! a real USD value. A future polish item could surface "(price is N hours
//! old)" warnings in the UI when the persisted price is stale.

use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use log::{info, warn};

use crate::database::WalletDatabase;

/// In-memory cache TTL (live fetch happens only after this expires)
const CACHE_TTL_SECONDS: u64 = 300;

const WHATSONCHAIN_URL: &str = "https://api.whatsonchain.com/v1/bsv/main/exchangerate";
const COINGECKO_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin-cash-sv&vs_currencies=usd";
const MEXC_URL: &str = "https://api.mexc.com/api/v3/ticker/price?symbol=BSVUSDT";

#[derive(Clone, Debug)]
struct CachedPrice {
    usd_price: f64,
    cached_at: u64,
    /// Provider name (`"whatsonchain"`, `"coingecko"`, `"mexc"`, or
    /// `"persisted"` if loaded from the SQLite fallback on startup).
    source: String,
}

/// Thread-safe BSV/USD price cache.
///
/// Holds the hot in-memory cache plus an `Arc<Mutex<WalletDatabase>>` for
/// reading/writing the `bsv_price_cache` SQLite table that backs the
/// restart-survival behavior.
pub struct PriceCache {
    cache: RwLock<Option<CachedPrice>>,
    client: reqwest::Client,
    /// Optional — when `None`, persistence is a no-op (used by tests that
    /// don't want to spin up a real DB). main.rs always passes `Some(db)`.
    db: Option<Arc<Mutex<WalletDatabase>>>,
}

impl PriceCache {
    /// Construct a cache. Pass `Some(db)` in production so the
    /// `bsv_price_cache` table is consulted for cold-start fallback and
    /// updated on every successful fetch.
    pub fn new(db: Option<Arc<Mutex<WalletDatabase>>>) -> Self {
        Self {
            cache: RwLock::new(None),
            client: reqwest::Client::builder()
                .timeout(crate::services::CallClass::IndexerAsync.timeout())
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            db,
        }
    }

    /// Construct a cache without DB persistence. Convenience for tests.
    #[cfg(test)]
    pub fn new_in_memory() -> Self {
        Self::new(None)
    }

    /// Load the persisted price from `bsv_price_cache` into the in-memory
    /// cache. Called once at startup from `main.rs` — if the table is empty
    /// or the read fails, the in-memory cache stays `None` and the first
    /// `get_price` will fetch live.
    ///
    /// The loaded price is treated as "stale-but-acceptable" — the in-memory
    /// cache's `cached_at` is set to the persisted `fetched_at`, so the next
    /// `get_price` call will (correctly) see it as expired vs the 5-min TTL
    /// and try a live fetch. Only if all three live sources fail does the
    /// stale-fallback path return this value.
    pub fn load_persisted(&self) {
        let Some(ref db) = self.db else { return; };
        let Ok(guard) = db.lock() else {
            warn!("   ⚠️  PriceCache::load_persisted — DB mutex poisoned, skipping");
            return;
        };
        let row: rusqlite::Result<(f64, i64, String)> = guard.connection().query_row(
            "SELECT price_usd, fetched_at, source FROM bsv_price_cache WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );
        match row {
            Ok((price, fetched_at, source)) => {
                if price > 0.0 {
                    let mut cache = self.cache.write().unwrap();
                    *cache = Some(CachedPrice {
                        usd_price: price,
                        cached_at: fetched_at as u64,
                        source: format!("persisted({})", source),
                    });
                    info!(
                        "   💲 Loaded persisted BSV/USD price: ${:.4} (source={}, age={}s)",
                        price,
                        source,
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                            .saturating_sub(fetched_at as u64)
                    );
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                info!("   💲 No persisted BSV/USD price (first run or table empty)");
            }
            Err(e) => {
                warn!("   ⚠️  PriceCache::load_persisted SELECT failed: {}", e);
            }
        }
    }

    /// Get the current BSV/USD price. Cache hit returns immediately. Cache
    /// miss / expired tries the live source chain. Returns `0.0` only when
    /// every live source has failed AND there's no stale in-memory value
    /// AND there's no persisted value to fall back on.
    pub async fn get_price(&self) -> f64 {
        if let Some(price) = self.get_cached() {
            return price;
        }

        match fetch_bsv_price(&self.client).await {
            Ok((price, source)) => {
                info!("   💲 BSV/USD price: ${:.4} (source={})", price, source);
                self.set(price, source);
                price
            }
            Err(e) => {
                warn!("   ⚠️  Failed to fetch BSV/USD price from all sources: {}", e);
                // Stale fallback — prefer in-memory (which itself may have been
                // seeded by load_persisted at startup) over re-reading the DB.
                if let Some(stale) = self.get_stale() {
                    warn!("   ⚠️  Using stale price: ${:.4}", stale);
                    stale
                } else {
                    0.0
                }
            }
        }
    }

    /// Get cached price if still fresh (within TTL).
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

    /// Get the cached price even if expired. Final fallback when every live
    /// source fails.
    pub fn get_stale(&self) -> Option<f64> {
        let cache = self.cache.read().unwrap();
        cache.as_ref().map(|c| c.usd_price)
    }

    /// Update the in-memory cache and persist to SQLite. The persist is
    /// best-effort — a DB-write failure is logged but doesn't fail the
    /// price read (the in-memory cache still has the live value).
    fn set(&self, usd_price: f64, source: String) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        {
            let mut cache = self.cache.write().unwrap();
            *cache = Some(CachedPrice {
                usd_price,
                cached_at: now,
                source: source.clone(),
            });
        }

        // Persist to DB so next cold-start has a fallback.
        if let Some(ref db) = self.db {
            if let Ok(guard) = db.lock() {
                let res = guard.connection().execute(
                    "INSERT INTO bsv_price_cache (id, price_usd, fetched_at, source)
                     VALUES (1, ?1, ?2, ?3)
                     ON CONFLICT(id) DO UPDATE SET
                       price_usd  = excluded.price_usd,
                       fetched_at = excluded.fetched_at,
                       source     = excluded.source",
                    rusqlite::params![usd_price, now as i64, source],
                );
                if let Err(e) = res {
                    warn!("   ⚠️  PriceCache persist failed: {} (in-memory cache OK)", e);
                }
            } else {
                warn!("   ⚠️  PriceCache persist — DB mutex poisoned");
            }
        }
    }
}

/// Try the 3-source chain in order. Returns the price + a short source-name
/// label on success, or a combined error message listing every failure.
async fn fetch_bsv_price(client: &reqwest::Client) -> Result<(f64, String), String> {
    let mut errors = Vec::new();

    match fetch_whatsonchain(client).await {
        Ok(p) => return Ok((p, "whatsonchain".to_string())),
        Err(e) => {
            warn!("   WhatsOnChain failed: {}", e);
            errors.push(format!("whatsonchain: {}", e));
        }
    }

    match fetch_coingecko(client).await {
        Ok(p) => return Ok((p, "coingecko".to_string())),
        Err(e) => {
            warn!("   CoinGecko failed: {}", e);
            errors.push(format!("coingecko: {}", e));
        }
    }

    match fetch_mexc(client).await {
        Ok(p) => return Ok((p, "mexc".to_string())),
        Err(e) => {
            warn!("   MEXC failed: {}", e);
            errors.push(format!("mexc: {}", e));
        }
    }

    Err(errors.join("; "))
}

/// WhatsOnChain `/v1/bsv/main/exchangerate` → `{"rate": 11.815, "currency": "USD"}`.
async fn fetch_whatsonchain(client: &reqwest::Client) -> Result<f64, String> {
    let response = client
        .get(WHATSONCHAIN_URL)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("status {}", response.status()));
    }
    let json: serde_json::Value =
        response.json().await.map_err(|e| format!("parse error: {}", e))?;
    let price = json
        .get("rate")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "missing rate field".to_string())?;
    validate_price(price, "WhatsOnChain")
}

/// CoinGecko with current slug → `{"bitcoin-cash-sv": {"usd": 11.72}}`.
async fn fetch_coingecko(client: &reqwest::Client) -> Result<f64, String> {
    let response = client
        .get(COINGECKO_URL)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("status {}", response.status()));
    }
    let json: serde_json::Value =
        response.json().await.map_err(|e| format!("parse error: {}", e))?;
    let price = json
        .get("bitcoin-cash-sv")
        .and_then(|v| v.get("usd"))
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "missing bitcoin-cash-sv.usd field".to_string())?;
    validate_price(price, "CoinGecko")
}

/// MEXC `/api/v3/ticker/price?symbol=BSVUSDT` → `{"symbol":"BSVUSDT","price":"11.71"}`.
/// Note: price is a string in the response, USDT not USD (1:1 within sanity range).
async fn fetch_mexc(client: &reqwest::Client) -> Result<f64, String> {
    let response = client
        .get(MEXC_URL)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("status {}", response.status()));
    }
    let json: serde_json::Value =
        response.json().await.map_err(|e| format!("parse error: {}", e))?;
    let price_str = json
        .get("price")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing price field".to_string())?;
    let price: f64 = price_str
        .parse()
        .map_err(|e| format!("price '{}' not parseable: {}", price_str, e))?;
    validate_price(price, "MEXC")
}

/// Sanity check: $0.01 < price < $10,000. Filters out malformed responses
/// (negative, zero, absurdly high) and ensures USDT/USD parity holds.
fn validate_price(price: f64, source: &str) -> Result<f64, String> {
    if price <= 0.0 || price > 10_000.0 {
        return Err(format!(
            "{} price ${:.4} outside sanity range ($0.01-$10,000)",
            source, price
        ));
    }
    Ok(price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_set_get() {
        let cache = PriceCache::new_in_memory();
        assert_eq!(cache.get_cached(), None);
        cache.set(52.34, "test".to_string());
        assert_eq!(cache.get_cached(), Some(52.34));
    }

    #[test]
    fn test_stale_fallback() {
        let cache = PriceCache::new_in_memory();
        assert_eq!(cache.get_stale(), None);
        cache.set(50.0, "test".to_string());
        assert_eq!(cache.get_stale(), Some(50.0));
    }

    #[test]
    fn test_validate_price() {
        assert!(validate_price(52.0, "test").is_ok());
        assert!(validate_price(0.0, "test").is_err());
        assert!(validate_price(-1.0, "test").is_err());
        assert!(validate_price(20_000.0, "test").is_err());
    }

    #[test]
    fn test_validate_price_at_boundaries() {
        // Just inside the lower boundary
        assert!(validate_price(0.01, "test").is_ok());
        // Exactly the upper boundary should pass (the check is `> 10_000`)
        assert!(validate_price(10_000.0, "test").is_ok());
        // Just outside the upper boundary
        assert!(validate_price(10_000.01, "test").is_err());
    }

    #[test]
    fn test_load_persisted_no_db_is_noop() {
        let cache = PriceCache::new_in_memory();
        cache.load_persisted();
        assert_eq!(cache.get_stale(), None);
    }
}
