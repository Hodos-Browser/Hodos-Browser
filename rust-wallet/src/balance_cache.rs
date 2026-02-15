//! In-memory balance cache with smart invalidation
//!
//! Provides fast balance retrieval by caching calculated balance in memory.
//! Cache is invalidated on all balance-changing events to ensure accuracy.

use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
struct CachedBalance {
    balance: i64,
    cached_at: u64,  // Unix timestamp
    version: u64,    // Increment on invalidation
    stale: bool,     // True if invalidated but not yet recalculated
}

/// Thread-safe in-memory balance cache
///
/// Caches the calculated wallet balance to avoid repeated database queries.
/// Cache is invalidated on all balance-changing events (transactions, UTXO sync, etc.)
/// to ensure accuracy.
pub struct BalanceCache {
    cache: Arc<RwLock<Option<CachedBalance>>>,
    ttl_seconds: u64,  // Time-to-live (default: 30 seconds)
}

impl BalanceCache {
    /// Create a new balance cache
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(None)),
            ttl_seconds: 30,  // 30 second TTL as safety net
        }
    }

    /// Get cached balance if fresh, None if expired/missing/stale
    ///
    /// Returns the cached balance only if it's fresh (not invalidated, within TTL).
    /// Use `get_or_stale()` when you need a fallback value even if stale.
    pub fn get(&self) -> Option<i64> {
        let cache = self.cache.read().unwrap();
        if let Some(cached) = cache.as_ref() {
            if cached.stale {
                return None;
            }
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            if now.saturating_sub(cached.cached_at) < self.ttl_seconds {
                Some(cached.balance)
            } else {
                None  // Expired
            }
        } else {
            None
        }
    }

    /// Get cached balance, returning stale value as fallback
    ///
    /// Returns the balance even if invalidated (stale). Only returns None
    /// if no balance has ever been cached. Use this when blocking on the
    /// DB lock is unacceptable (e.g. the balance endpoint).
    pub fn get_or_stale(&self) -> Option<i64> {
        let cache = self.cache.read().unwrap();
        cache.as_ref().map(|c| c.balance)
    }

    /// Set cached balance
    ///
    /// Updates the cache with a new balance value and current timestamp.
    pub fn set(&self, balance: i64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut cache = self.cache.write().unwrap();
        *cache = Some(CachedBalance {
            balance,
            cached_at: now,
            version: cache.as_ref()
                .map(|c| c.version + 1)
                .unwrap_or(0),
            stale: false,
        });
    }

    /// Invalidate cache (force refresh on next request)
    ///
    /// Marks the cache as stale so the next balance request will recalculate
    /// from database. The old value is kept as a fallback via `get_or_stale()`
    /// so the balance endpoint never blocks waiting for the DB lock.
    ///
    /// Call this whenever balance might have changed:
    /// - Transaction created (outgoing)
    /// - UTXO sync completed
    /// - New UTXO detected
    /// - UTXO marked as spent
    pub fn invalidate(&self) {
        let mut cache = self.cache.write().unwrap();
        if let Some(ref mut cached) = *cache {
            cached.stale = true;
        }
    }

    /// Invalidate and update with new balance (atomic)
    ///
    /// Convenience method that invalidates and sets a new balance in one operation.
    pub fn update(&self, balance: i64) {
        self.set(balance);
    }
}

impl Default for BalanceCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_cache_set_get() {
        let cache = BalanceCache::new();

        // Initially empty
        assert_eq!(cache.get(), None);

        // Set value
        cache.set(100000);
        assert_eq!(cache.get(), Some(100000));
    }

    #[test]
    fn test_cache_invalidate() {
        let cache = BalanceCache::new();

        cache.set(100000);
        assert_eq!(cache.get(), Some(100000));

        cache.invalidate();
        assert_eq!(cache.get(), None);  // Fresh get returns None
        assert_eq!(cache.get_or_stale(), Some(100000));  // Stale fallback still works
    }

    #[test]
    fn test_cache_update() {
        let cache = BalanceCache::new();

        cache.set(100000);
        cache.update(200000);
        assert_eq!(cache.get(), Some(200000));
    }

    #[test]
    fn test_cache_thread_safety() {
        let cache = Arc::new(BalanceCache::new());

        let cache1 = cache.clone();
        let handle1 = thread::spawn(move || {
            cache1.set(100000);
        });

        let cache2 = cache.clone();
        let handle2 = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            cache2.set(200000);
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        // Should have the last value set
        assert_eq!(cache.get(), Some(200000));
    }
}
