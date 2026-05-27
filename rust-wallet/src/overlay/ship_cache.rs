//! SHIP host discovery cache with stale-while-revalidate (SWR) semantics.
//!
//! SHIP discovery against SLAP trackers is the single biggest cost on every
//! certificate publish/unpublish (~75s wallclock per call, measured 2026-05-27).
//! This cache eliminates that cost on the hot path by serving recent results
//! synchronously and refreshing in the background.
//!
//! Decision tree per `get_hosts` call:
//!
//! ```text
//! age < FRESH_TTL (5 min)        → return cached, no fetch
//! FRESH_TTL <= age < STALE_TTL   → return cached + spawn bg refresh (dedup'd)
//! age >= STALE_TTL (30 min)      → block on fresh fetch
//!                                  on fail: return stale-but-served entry
//! no entry                       → block on fresh fetch
//!                                  on fail: return empty (no poison)
//! ```
//!
//! No-poison invariant: empty fetch results NEVER overwrite a cached entry,
//! and failed fetches NEVER store an empty default. The cache either reflects
//! ground truth or holds whatever the last successful fetch returned. See
//! `[[project_cache_no_poison_on_failure]]` for the prior bug pattern this
//! avoids.
//!
//! Mirrors `PriceCache` / `FeeRateCache` style: `RwLock<HashMap>` for state,
//! plus a small per-topic `Mutex<HashSet>` for background-refresh dedup.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use log::{debug, info, warn};

/// Fresh window — cached entry served synchronously, no fetch.
pub const FRESH_TTL: Duration = Duration::from_secs(300); // 5 min

/// Stale window — cached entry served + background refresh spawned.
/// Beyond this, the next call blocks on a fresh fetch.
pub const STALE_TTL: Duration = Duration::from_secs(1800); // 30 min

/// Classified entry status used to decide what `get_hosts` should do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheStatus {
    /// Within FRESH_TTL — return immediately, no refresh.
    Fresh(Vec<String>),
    /// FRESH_TTL ≤ age < STALE_TTL — return cached, spawn background refresh.
    Stale(Vec<String>),
    /// age ≥ STALE_TTL — must block-fetch; this stale value is the fallback
    /// if the fetch fails.
    VeryStale(Vec<String>),
    /// No cached entry — must block-fetch; empty is the fallback on failure.
    Empty,
}

#[derive(Debug, Clone)]
struct CachedEntry {
    hosts: Vec<String>,
    fetched_at: Instant,
}

/// Stale-while-revalidate cache for SHIP-discovered overlay hosts, keyed by topic.
///
/// Lives on `AppState` as `Arc<ShipDiscoveryCache>`. All callers go through
/// `get_hosts()` (synchronous-feeling, may block on miss/very-stale) or
/// `refresh()` (background refresh, used by the monitor task).
pub struct ShipDiscoveryCache {
    entries: RwLock<HashMap<String, CachedEntry>>,
    /// Per-topic flag tracking whether a background refresh is in flight.
    /// Only used to dedup the spawn from `get_hosts` Stale-branch; foreground
    /// fetches (VeryStale / Empty branches) are not deduped.
    refresh_in_flight: Mutex<HashSet<String>>,
}

impl ShipDiscoveryCache {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            entries: RwLock::new(HashMap::new()),
            refresh_in_flight: Mutex::new(HashSet::new()),
        })
    }

    /// Pure state-machine classification — no I/O. Exposed for unit testing.
    pub fn classify(&self, topic: &str) -> CacheStatus {
        let entries = match self.entries.read() {
            Ok(g) => g,
            Err(_) => return CacheStatus::Empty,
        };
        match entries.get(topic) {
            None => CacheStatus::Empty,
            Some(entry) => {
                let age = entry.fetched_at.elapsed();
                if age < FRESH_TTL {
                    CacheStatus::Fresh(entry.hosts.clone())
                } else if age < STALE_TTL {
                    CacheStatus::Stale(entry.hosts.clone())
                } else {
                    CacheStatus::VeryStale(entry.hosts.clone())
                }
            }
        }
    }

    /// Get the hosts for a topic, applying SWR semantics.
    ///
    /// Takes `&Arc<Self>` so the Stale branch can clone the Arc into a
    /// `tokio::spawn`'d refresh task.
    pub async fn get_hosts(self: &Arc<Self>, topic: &str) -> Vec<String> {
        match self.classify(topic) {
            CacheStatus::Fresh(hosts) => {
                debug!("ShipCache: FRESH hit for '{}' ({} host(s))", topic, hosts.len());
                hosts
            }
            CacheStatus::Stale(hosts) => {
                debug!(
                    "ShipCache: STALE hit for '{}' ({} host(s)); spawning bg refresh",
                    topic,
                    hosts.len()
                );
                if self.try_claim_refresh(topic) {
                    let cache = Arc::clone(self);
                    let topic_owned = topic.to_string();
                    tokio::spawn(async move {
                        cache.fetch_and_store(&topic_owned).await;
                        cache.release_refresh(&topic_owned);
                    });
                }
                hosts
            }
            CacheStatus::VeryStale(stale_hosts) => {
                info!(
                    "ShipCache: VERY-STALE for '{}' ({} host(s) cached); blocking on fresh fetch",
                    topic,
                    stale_hosts.len()
                );
                match self.fetch_and_store(topic).await {
                    Some(fresh) => fresh,
                    None => {
                        warn!(
                            "ShipCache: fresh fetch failed for '{}'; falling back to stale ({} host(s))",
                            topic,
                            stale_hosts.len()
                        );
                        stale_hosts
                    }
                }
            }
            CacheStatus::Empty => {
                info!("ShipCache: MISS for '{}'; blocking on fresh fetch", topic);
                self.fetch_and_store(topic).await.unwrap_or_default()
            }
        }
    }

    /// Force a refresh of a topic. Used by the Monitor task to keep the cache
    /// warm independent of usage.
    ///
    /// No-op if a refresh is already in flight (dedup against bg-refresh from
    /// `get_hosts` Stale branch). On fetch failure, the cached entry is left
    /// untouched (no-poison).
    pub async fn refresh(&self, topic: &str) {
        if !self.try_claim_refresh(topic) {
            debug!("ShipCache: refresh for '{}' skipped (already in flight)", topic);
            return;
        }
        let result = self.fetch_and_store(topic).await;
        self.release_refresh(topic);
        match result {
            Some(hosts) => info!(
                "ShipCache: refresh OK for '{}' ({} host(s))",
                topic,
                hosts.len()
            ),
            None => warn!(
                "ShipCache: refresh got 0 hosts for '{}'; cache unchanged",
                topic
            ),
        }
    }

    /// Execute the SHIP query, store the result, and return the host list.
    ///
    /// Returns `None` if discovery returned zero hosts (does NOT touch the
    /// cache — no-poison invariant). Returns `Some(hosts)` and writes them
    /// to the cache on any non-empty result.
    async fn fetch_and_store(&self, topic: &str) -> Option<Vec<String>> {
        let discovered = super::query_ship_advertisements(&[topic.to_string()]).await;
        let hosts: Vec<String> = discovered.keys().cloned().collect();
        if hosts.is_empty() {
            return None;
        }
        self.set(topic, hosts.clone());
        Some(hosts)
    }

    /// Internal: write an entry with the current timestamp.
    fn set(&self, topic: &str, hosts: Vec<String>) {
        let mut entries = match self.entries.write() {
            Ok(g) => g,
            Err(_) => return,
        };
        entries.insert(
            topic.to_string(),
            CachedEntry {
                hosts,
                fetched_at: Instant::now(),
            },
        );
    }

    /// Try to claim the in-flight refresh slot for `topic`. Returns `true`
    /// if we claimed it (caller must `release_refresh` when done), `false`
    /// if another task already holds it.
    fn try_claim_refresh(&self, topic: &str) -> bool {
        let mut set = match self.refresh_in_flight.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        set.insert(topic.to_string())
    }

    fn release_refresh(&self, topic: &str) {
        if let Ok(mut set) = self.refresh_in_flight.lock() {
            set.remove(topic);
        }
    }
}

#[cfg(test)]
impl ShipDiscoveryCache {
    /// Test helper: insert an entry with an explicit `fetched_at`, bypassing
    /// the live `Instant::now()` clock. Used by unit tests to simulate
    /// fresh/stale/very-stale ages.
    pub fn set_for_test(&self, topic: &str, hosts: Vec<String>, fetched_at: Instant) {
        if let Ok(mut entries) = self.entries.write() {
            entries.insert(
                topic.to_string(),
                CachedEntry { hosts, fetched_at },
            );
        }
    }

    /// Test helper: read in-flight set size.
    pub fn refresh_in_flight_count(&self) -> usize {
        self.refresh_in_flight.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Test helper: check whether a specific topic is currently in flight.
    pub fn is_refresh_in_flight(&self, topic: &str) -> bool {
        self.refresh_in_flight
            .lock()
            .map(|g| g.contains(topic))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn classify_empty_when_no_entry() {
        let cache = ShipDiscoveryCache::new();
        assert_eq!(cache.classify("tm_identity"), CacheStatus::Empty);
    }

    #[test]
    fn classify_fresh_for_recent_entry() {
        let cache = ShipDiscoveryCache::new();
        cache.set_for_test("tm_identity", hosts(&["https://a"]), Instant::now());
        match cache.classify("tm_identity") {
            CacheStatus::Fresh(h) => assert_eq!(h, hosts(&["https://a"])),
            other => panic!("expected Fresh, got {:?}", other),
        }
    }

    #[test]
    fn classify_stale_after_fresh_ttl() {
        let cache = ShipDiscoveryCache::new();
        // 10 min old: past FRESH_TTL (5 min), within STALE_TTL (30 min)
        let ten_min_ago = Instant::now() - Duration::from_secs(600);
        cache.set_for_test("tm_identity", hosts(&["https://a"]), ten_min_ago);
        match cache.classify("tm_identity") {
            CacheStatus::Stale(h) => assert_eq!(h, hosts(&["https://a"])),
            other => panic!("expected Stale, got {:?}", other),
        }
    }

    #[test]
    fn classify_very_stale_after_stale_ttl() {
        let cache = ShipDiscoveryCache::new();
        // 1 hour old: past STALE_TTL (30 min)
        let hour_ago = Instant::now() - Duration::from_secs(3600);
        cache.set_for_test("tm_identity", hosts(&["https://a"]), hour_ago);
        match cache.classify("tm_identity") {
            CacheStatus::VeryStale(h) => assert_eq!(h, hosts(&["https://a"])),
            other => panic!("expected VeryStale, got {:?}", other),
        }
    }

    #[test]
    fn classify_uses_per_topic_state() {
        let cache = ShipDiscoveryCache::new();
        cache.set_for_test("tm_identity", hosts(&["https://a"]), Instant::now());
        // Different topic — no entry
        assert_eq!(cache.classify("tm_other"), CacheStatus::Empty);
    }

    #[test]
    fn try_claim_refresh_dedupes() {
        let cache = ShipDiscoveryCache::new();
        assert!(cache.try_claim_refresh("tm_identity"));
        // Second claim while first is held → false
        assert!(!cache.try_claim_refresh("tm_identity"));
        // Different topic still claimable
        assert!(cache.try_claim_refresh("tm_other"));
    }

    #[test]
    fn release_refresh_allows_reclaim() {
        let cache = ShipDiscoveryCache::new();
        assert!(cache.try_claim_refresh("tm_identity"));
        cache.release_refresh("tm_identity");
        assert!(cache.try_claim_refresh("tm_identity"));
    }

    #[test]
    fn set_overwrites_with_new_timestamp() {
        let cache = ShipDiscoveryCache::new();
        let old = Instant::now() - Duration::from_secs(3600);
        cache.set_for_test("tm_identity", hosts(&["https://old"]), old);
        // Fresh write
        cache.set("tm_identity", hosts(&["https://new"]));
        match cache.classify("tm_identity") {
            CacheStatus::Fresh(h) => assert_eq!(h, hosts(&["https://new"])),
            other => panic!("expected Fresh after overwrite, got {:?}", other),
        }
    }

    #[test]
    fn in_flight_count_tracks_claims() {
        let cache = ShipDiscoveryCache::new();
        assert_eq!(cache.refresh_in_flight_count(), 0);
        cache.try_claim_refresh("a");
        cache.try_claim_refresh("b");
        assert_eq!(cache.refresh_in_flight_count(), 2);
        cache.release_refresh("a");
        assert_eq!(cache.refresh_in_flight_count(), 1);
    }

    // Note: fetch_and_store and the network-backed paths in get_hosts/refresh
    // are not exercised by unit tests — they require live SLAP trackers.
    // They are covered by integration smoke (manual publish/unpublish timing
    // observations) per the Step 1 test plan.
}
