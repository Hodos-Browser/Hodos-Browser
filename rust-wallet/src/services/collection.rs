//! `ProviderCollection<P>` — ordered fallback chain of `IndexerProvider` implementations.
//!
//! Per Phase 1.6 DESIGN §2.2:
//! - `call(...)` tries each `supports()`-eligible provider in current order.
//! - On `SoftTimeout` the offending provider is demoted (moved to last) for the rest
//!   of the process lifetime, then the next provider is tried.
//! - On other errors the cursor advances without demoting.
//! - `NotFound` short-circuits the chain — that's a positive "tx doesn't exist" signal,
//!   not a provider failure.
//! - `supports()=false` providers are skipped without being counted as a call.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::provider::{IndexerError, IndexerProvider, ProviderOp};

/// Local `BoxFuture` alias so callers don't need a `futures` dep. Futures returned
/// from `call`'s closure are bounded `'static` so callers must capture owned data
/// (clone &str → String). Providers are passed in by `Arc<P>`, so the future can
/// hold the provider for as long as it needs without borrowing-lifetime puzzles.
pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

#[derive(Debug, Default, Clone)]
pub struct ProviderStats {
    pub calls: u64,
    pub successes: u64,
    pub soft_timeouts: u64,
    pub hard_errors: u64,
    pub not_found: u64,
    pub last_used_at: Option<Instant>,
}

/// How one provider failed to answer. Kept for callers that must judge the whole
/// chain rather than its last link (B5-T1-P2: a broadcast is permanently refused only
/// if every provider that answered refused it — the last error alone can be a
/// raw-tx-only "Missing inputs" after an earlier provider timed out holding the tx).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureKind {
    SoftTimeout,
    Transport,
    Status(u16),
    Invalid,
    NotFound,
}

#[derive(Debug, Clone)]
pub struct ProviderFailure {
    pub provider: &'static str,
    pub kind: FailureKind,
    pub message: String,
}

pub struct ProviderCollection<P: ?Sized> {
    /// Ordered provider list. Index 0 is the head. Demotion (`SoftTimeout`) moves the
    /// offending provider to the tail.
    providers: Mutex<Vec<Arc<P>>>,
    /// Per-provider stats, keyed by `provider.name()`.
    stats: Mutex<HashMap<&'static str, ProviderStats>>,
}

impl<P: IndexerProvider + ?Sized> ProviderCollection<P> {
    pub fn new(providers: Vec<Arc<P>>) -> Self {
        Self {
            providers: Mutex::new(providers),
            stats: Mutex::new(HashMap::new()),
        }
    }

    /// Try each `supports()`-eligible provider in current order, wrapping each call in
    /// `tokio::time::timeout(soft_timeout, ...)`. Returns the first success.
    ///
    /// Demotes (`moveServiceToLast`) the provider on `SoftTimeout`. Short-circuits on
    /// `NotFound`. Returns the last error seen if every eligible provider fails.
    ///
    /// `op` is used solely to filter by `supports(op)` — the closure `f` decides which
    /// provider method to invoke.
    pub async fn call<F, R>(
        &self,
        op: ProviderOp,
        soft_timeout: Duration,
        f: F,
    ) -> Result<R, IndexerError>
    where
        F: Fn(Arc<P>) -> BoxFut<Result<R, IndexerError>>,
        R: Send + 'static,
    {
        self.call_tracking(op, soft_timeout, f).await.0
    }

    /// `call`, except that `NotFound` means **every** eligible provider said not found.
    ///
    /// `call` returns the *last* error, so `Transport, Transport, NotFound` reads as
    /// `NotFound` even though two providers never answered. That is fine for "is it
    /// there?" and wrong for a caller that releases coins on "it is not there"
    /// (`abort_action`, beta.6 P2). Here any provider that failed to answer turns a
    /// `NotFound` into an error.
    pub async fn call_unanimous_not_found<F, R>(
        &self,
        op: ProviderOp,
        soft_timeout: Duration,
        f: F,
    ) -> Result<R, IndexerError>
    where
        F: Fn(Arc<P>) -> BoxFut<Result<R, IndexerError>>,
        R: Send + 'static,
    {
        let (result, failures) = self.call_tracking(op, soft_timeout, f).await;
        let failure = failures
            .iter()
            .rev()
            .find(|f| f.kind != FailureKind::NotFound)
            .map(|f| format!("{}: {}", f.provider, f.message));
        match (result, failure) {
            (Err(IndexerError::NotFound), Some(failure)) => Err(IndexerError::InvalidResponse {
                provider: "ProviderCollection",
                reason: format!("not every provider answered: {}", failure),
            }),
            (result, _) => result,
        }
    }

    /// `call`, plus every provider that failed before the answer (or before giving up),
    /// in chain order. Same fallback behaviour as `call`.
    pub async fn call_traced<F, R>(
        &self,
        op: ProviderOp,
        soft_timeout: Duration,
        f: F,
    ) -> (Result<R, IndexerError>, Vec<ProviderFailure>)
    where
        F: Fn(Arc<P>) -> BoxFut<Result<R, IndexerError>>,
        R: Send + 'static,
    {
        self.call_tracking(op, soft_timeout, f).await
    }

    /// The fallback loop. Also returns every provider failure, in chain order.
    async fn call_tracking<F, R>(
        &self,
        op: ProviderOp,
        soft_timeout: Duration,
        f: F,
    ) -> (Result<R, IndexerError>, Vec<ProviderFailure>)
    where
        F: Fn(Arc<P>) -> BoxFut<Result<R, IndexerError>>,
        R: Send + 'static,
    {
        // Snapshot the current provider order so we don't hold the lock across awaits.
        let snapshot: Vec<Arc<P>> = {
            let guard = self.providers.lock().expect("providers mutex poisoned");
            guard.iter().cloned().collect()
        };

        let mut last_err: Option<IndexerError> = None;
        let mut failures: Vec<ProviderFailure> = Vec::new();

        for provider in &snapshot {
            if !provider.supports(op) {
                continue;
            }

            self.bump(provider.name(), |s| s.calls += 1);

            let future = f(provider.clone());
            match tokio::time::timeout(soft_timeout, future).await {
                Ok(Ok(value)) => {
                    self.bump(provider.name(), |s| s.successes += 1);
                    return (Ok(value), failures);
                }
                Ok(Err(IndexerError::NotFound)) => {
                    self.bump(provider.name(), |s| s.not_found += 1);
                    failures.push(ProviderFailure {
                        provider: provider.name(),
                        kind: FailureKind::NotFound,
                        message: "not found".to_string(),
                    });
                    last_err = Some(IndexerError::NotFound);
                    // Advance to next provider. DESIGN §2.2 originally said NotFound
                    // should short-circuit as a positive "tx doesn't exist" signal — but
                    // that was wrong. A provider's NotFound only means *that provider*
                    // doesn't have the tx. ARC (a tx processor, not an archive) returns
                    // NotFound for old confirmed txs that WoC has — short-circuiting
                    // there hides them. We only return NotFound to the caller after
                    // every eligible provider has said NotFound.
                }
                Ok(Err(err)) => {
                    self.bump(provider.name(), |s| s.hard_errors += 1);
                    let kind = match &err {
                        IndexerError::Transport(_) => FailureKind::Transport,
                        IndexerError::SoftTimeout(_) => FailureKind::SoftTimeout,
                        IndexerError::ProviderStatus { status, .. } => FailureKind::Status(*status),
                        IndexerError::InvalidResponse { .. } => FailureKind::Invalid,
                        IndexerError::NotFound => FailureKind::NotFound,
                    };
                    failures.push(ProviderFailure {
                        provider: provider.name(),
                        kind,
                        message: err.to_string(),
                    });
                    last_err = Some(err);
                    // Advance to next provider without demoting (transport/4xx/5xx is
                    // not the "slow provider" signal that demotion targets).
                }
                Err(_elapsed) => {
                    self.bump(provider.name(), |s| s.soft_timeouts += 1);
                    self.demote(provider.name());
                    failures.push(ProviderFailure {
                        provider: provider.name(),
                        kind: FailureKind::SoftTimeout,
                        message: "soft timeout".to_string(),
                    });
                    last_err = Some(IndexerError::SoftTimeout(soft_timeout));
                    // Advance to next provider; the demoted one is now at the tail for
                    // subsequent calls.
                }
            }
        }

        // Every eligible provider failed (or none were eligible).
        (Err(last_err.unwrap_or(IndexerError::InvalidResponse {
            provider: "ProviderCollection",
            reason: "no eligible providers for this op".to_string(),
        })), failures)
    }

    /// 5s base + 50ms per KiB, capped at 30s. Matches canonical `postBeef` formula.
    /// Used by `broadcast_beef` callers who pass the BEEF byte length.
    pub fn adaptive_soft_timeout_for_payload(bytes: usize) -> Duration {
        let extra_ms = (bytes as u64).saturating_mul(50) / 1024;
        let total = Duration::from_secs(5) + Duration::from_millis(extra_ms);
        std::cmp::min(total, Duration::from_secs(30))
    }

    /// Snapshot of provider stats for telemetry / debugging.
    pub fn snapshot_stats(&self) -> Vec<(&'static str, ProviderStats)> {
        let guard = self.stats.lock().expect("stats mutex poisoned");
        guard.iter().map(|(k, v)| (*k, v.clone())).collect()
    }

    fn demote(&self, name: &'static str) {
        let mut guard = self.providers.lock().expect("providers mutex poisoned");
        if let Some(pos) = guard.iter().position(|p| p.name() == name) {
            // Don't demote if already at tail (no-op micro-opt).
            if pos + 1 < guard.len() {
                let provider = guard.remove(pos);
                guard.push(provider);
            }
        }
    }

    fn bump<F: FnOnce(&mut ProviderStats)>(&self, name: &'static str, f: F) {
        let mut guard = self.stats.lock().expect("stats mutex poisoned");
        let entry = guard.entry(name).or_default();
        f(entry);
        entry.last_used_at = Some(Instant::now());
    }

    #[cfg(test)]
    pub(crate) fn current_order(&self) -> Vec<&'static str> {
        let guard = self.providers.lock().expect("providers mutex poisoned");
        guard.iter().map(|p| p.name()).collect()
    }

    /// Test-only (B5-T1-P7b): the providers `call` would try for `op`, in order.
    #[cfg(test)]
    pub(crate) fn eligible_order(&self, op: ProviderOp) -> Vec<&'static str> {
        let guard = self.providers.lock().expect("providers mutex poisoned");
        guard.iter().filter(|p| p.supports(op)).map(|p| p.name()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::provider::IndexerProvider;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A test provider with configurable name, op support, and a behavior toggle for
    /// the `tx_status` call. Counts invocations so tests can assert which providers
    /// were tried.
    struct TestProvider {
        name: &'static str,
        supports_op: bool,
        /// 0 = success, 1 = NotFound, 2 = transport err, 3 = slow (loops a sleep so
        /// the soft-timeout fires).
        behavior: u8,
        calls: Arc<AtomicUsize>,
    }

    impl TestProvider {
        fn new(name: &'static str, behavior: u8) -> Self {
            Self {
                name,
                supports_op: true,
                behavior,
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn unsupported(name: &'static str) -> Self {
            Self {
                name,
                supports_op: false,
                behavior: 0,
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn counter(&self) -> Arc<AtomicUsize> {
            self.calls.clone()
        }
    }

    #[async_trait]
    impl IndexerProvider for TestProvider {
        fn name(&self) -> &'static str {
            self.name
        }

        fn supports(&self, _op: ProviderOp) -> bool {
            self.supports_op
        }

        async fn get_raw_tx(&self, _txid: &str) -> Result<Vec<u8>, IndexerError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.behavior {
                0 => Ok(vec![0xde, 0xad, 0xbe, 0xef]),
                1 => Err(IndexerError::NotFound),
                2 => Err(IndexerError::Transport("simulated".to_string())),
                3 => {
                    // Sleep longer than any reasonable test soft-timeout.
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    Ok(vec![])
                }
                _ => Err(IndexerError::InvalidResponse {
                    provider: self.name,
                    reason: "unknown behavior".into(),
                }),
            }
        }
    }

    fn collection_with(providers: Vec<Arc<TestProvider>>) -> ProviderCollection<dyn IndexerProvider>
    {
        let cast: Vec<Arc<dyn IndexerProvider>> = providers
            .into_iter()
            .map(|p| p as Arc<dyn IndexerProvider>)
            .collect();
        ProviderCollection::new(cast)
    }

    // --- call_unanimous_not_found (beta.6 P2) ---

    async fn unanimous(behaviors: &[u8]) -> Result<Vec<u8>, IndexerError> {
        let names = ["a", "b", "c", "d"];
        let providers = behaviors.iter().zip(names).map(|(b, n)| Arc::new(TestProvider::new(n, *b))).collect();
        collection_with(providers)
            .call_unanimous_not_found(ProviderOp::RawTx, Duration::from_millis(100), |p| {
                Box::pin(async move { p.get_raw_tx("anything").await })
            })
            .await
    }

    #[tokio::test]
    async fn p2_unanimous_not_found_only_when_every_provider_says_so() {
        assert!(matches!(unanimous(&[1, 1, 1, 1]).await, Err(IndexerError::NotFound)));
        // `call` would say NotFound for these: a provider that never answered is not a "no".
        for b in [[2, 1, 1, 1], [1, 2, 1, 1], [3, 1, 1, 1], [2, 2, 2, 1]] {
            let r = unanimous(&b).await;
            assert!(matches!(r, Err(IndexerError::InvalidResponse { .. })), "{:?}: {:?}", b, r);
        }
        // Any answer still wins, and an all-error chain stays an error.
        assert!(unanimous(&[2, 1, 0, 1]).await.is_ok());
        assert!(matches!(unanimous(&[1, 1, 1, 2]).await, Err(IndexerError::Transport(_))));
    }

    #[tokio::test]
    async fn p2_call_keeps_its_last_error_semantics() {
        let providers = [2u8, 1].iter().zip(["a", "b"]).map(|(b, n)| Arc::new(TestProvider::new(n, *b))).collect();
        let r = collection_with(providers)
            .call(ProviderOp::RawTx, Duration::from_millis(100), |p| {
                Box::pin(async move { p.get_raw_tx("anything").await })
            })
            .await;
        assert!(matches!(r, Err(IndexerError::NotFound)), "{:?}", r);
    }

    // --- adaptive_soft_timeout_for_payload ---

    #[test]
    fn adaptive_timeout_zero_bytes_is_5s() {
        let d = ProviderCollection::<dyn IndexerProvider>::adaptive_soft_timeout_for_payload(0);
        assert_eq!(d, Duration::from_secs(5));
    }

    #[test]
    fn adaptive_timeout_100kib_is_10s() {
        let d = ProviderCollection::<dyn IndexerProvider>::adaptive_soft_timeout_for_payload(
            100 * 1024,
        );
        // 5000 + 100 * 50 = 5000 + 5000 = 10000 ms
        assert_eq!(d, Duration::from_millis(10_000));
    }

    #[test]
    fn adaptive_timeout_10mib_capped_at_30s() {
        let d = ProviderCollection::<dyn IndexerProvider>::adaptive_soft_timeout_for_payload(
            10 * 1024 * 1024,
        );
        assert_eq!(d, Duration::from_secs(30));
    }

    #[test]
    fn adaptive_timeout_huge_payload_still_capped() {
        let d = ProviderCollection::<dyn IndexerProvider>::adaptive_soft_timeout_for_payload(
            usize::MAX / 2,
        );
        assert_eq!(d, Duration::from_secs(30));
    }

    // --- call() basic behaviors ---

    #[tokio::test]
    async fn call_returns_first_provider_success_without_trying_others() {
        let a = Arc::new(TestProvider::new("a", 0));
        let b = Arc::new(TestProvider::new("b", 0));
        let a_count = a.counter();
        let b_count = b.counter();
        let coll = collection_with(vec![a, b]);

        let r = coll
            .call(ProviderOp::RawTx, Duration::from_secs(1), |p| {
                Box::pin(async move { p.get_raw_tx("anything").await })
            })
            .await
            .expect("should succeed");

        assert_eq!(r, vec![0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(a_count.load(Ordering::SeqCst), 1);
        assert_eq!(b_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn call_advances_to_next_provider_on_transport_error() {
        let a = Arc::new(TestProvider::new("a", 2)); // transport err
        let b = Arc::new(TestProvider::new("b", 0)); // success
        let a_count = a.counter();
        let b_count = b.counter();
        let coll = collection_with(vec![a, b]);

        let r = coll
            .call(ProviderOp::RawTx, Duration::from_secs(1), |p| {
                Box::pin(async move { p.get_raw_tx("anything").await })
            })
            .await
            .expect("should succeed via b");

        assert_eq!(r, vec![0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(a_count.load(Ordering::SeqCst), 1);
        assert_eq!(b_count.load(Ordering::SeqCst), 1);
        // a was NOT demoted on transport error — must still be first.
        assert_eq!(coll.current_order(), vec!["a", "b"]);
    }

    #[tokio::test]
    async fn call_demotes_provider_on_soft_timeout_and_advances() {
        let a = Arc::new(TestProvider::new("a", 3)); // slow → soft timeout
        let b = Arc::new(TestProvider::new("b", 0)); // success
        let c = Arc::new(TestProvider::new("c", 0));
        let coll = collection_with(vec![a, b, c]);

        let r = coll
            .call(ProviderOp::RawTx, Duration::from_millis(100), |p| {
                Box::pin(async move { p.get_raw_tx("anything").await })
            })
            .await
            .expect("should succeed via b");

        assert_eq!(r, vec![0xde, 0xad, 0xbe, 0xef]);
        // After soft-timeout, "a" should be demoted to the tail.
        assert_eq!(coll.current_order(), vec!["b", "c", "a"]);
    }

    #[tokio::test]
    async fn call_advances_past_not_found_to_next_provider() {
        // Regression test: ARC returns NotFound for old confirmed txs because ARC is
        // a tx processor not an archive. We must NOT short-circuit on NotFound — WoC
        // (the next provider) has the proof and would have answered.
        let a = Arc::new(TestProvider::new("a", 1)); // NotFound
        let b = Arc::new(TestProvider::new("b", 0)); // success — MUST be tried
        let a_count = a.counter();
        let b_count = b.counter();
        let coll = collection_with(vec![a, b]);

        let r = coll
            .call(ProviderOp::RawTx, Duration::from_secs(1), |p| {
                Box::pin(async move { p.get_raw_tx("anything").await })
            })
            .await
            .expect("should advance past NotFound and succeed via b");

        assert_eq!(r, vec![0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(a_count.load(Ordering::SeqCst), 1, "a should have been tried");
        assert_eq!(b_count.load(Ordering::SeqCst), 1, "b must be tried after a's NotFound");
    }

    #[tokio::test]
    async fn call_returns_not_found_only_when_all_providers_say_not_found() {
        let a = Arc::new(TestProvider::new("a", 1)); // NotFound
        let b = Arc::new(TestProvider::new("b", 1)); // NotFound too
        let coll = collection_with(vec![a, b]);

        let err = coll
            .call(ProviderOp::RawTx, Duration::from_secs(1), |p| {
                Box::pin(async move { p.get_raw_tx("anything").await })
            })
            .await
            .expect_err("should return NotFound after all providers said NotFound");

        assert!(matches!(err, IndexerError::NotFound));
    }

    #[tokio::test]
    async fn call_skips_providers_whose_supports_returns_false() {
        let a = Arc::new(TestProvider::unsupported("a")); // supports=false
        let b = Arc::new(TestProvider::new("b", 0));
        let a_count = a.counter();
        let b_count = b.counter();
        let coll = collection_with(vec![a, b]);

        let r = coll
            .call(ProviderOp::RawTx, Duration::from_secs(1), |p| {
                Box::pin(async move { p.get_raw_tx("anything").await })
            })
            .await
            .expect("should succeed via b");

        assert_eq!(r, vec![0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(a_count.load(Ordering::SeqCst), 0, "a must not be called");
        assert_eq!(b_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn call_returns_last_err_when_every_provider_fails() {
        let a = Arc::new(TestProvider::new("a", 2));
        let b = Arc::new(TestProvider::new("b", 2));
        let coll = collection_with(vec![a, b]);

        let err = coll
            .call(ProviderOp::RawTx, Duration::from_secs(1), |p| {
                Box::pin(async move { p.get_raw_tx("anything").await })
            })
            .await
            .expect_err("should fail");

        assert!(matches!(err, IndexerError::Transport(_)));
    }

    #[tokio::test]
    async fn call_returns_synthesized_err_when_no_provider_supports_op() {
        let a = Arc::new(TestProvider::unsupported("a"));
        let b = Arc::new(TestProvider::unsupported("b"));
        let coll = collection_with(vec![a, b]);

        let err = coll
            .call(ProviderOp::RawTx, Duration::from_secs(1), |p| {
                Box::pin(async move { p.get_raw_tx("anything").await })
            })
            .await
            .expect_err("should fail with synthesized err");

        match err {
            IndexerError::InvalidResponse { provider, reason } => {
                assert_eq!(provider, "ProviderCollection");
                assert!(reason.contains("no eligible providers"));
            }
            other => panic!("expected InvalidResponse, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn concurrent_calls_dont_double_demote() {
        // a is slow, b/c are fast. Fire several concurrent calls; a should end up at
        // the tail (single demotion event per `position()` lookup — repeated demotion
        // of an already-demoted provider is a no-op).
        let a = Arc::new(TestProvider::new("a", 3));
        let b = Arc::new(TestProvider::new("b", 0));
        let c = Arc::new(TestProvider::new("c", 0));
        let coll = Arc::new(collection_with(vec![a, b, c]));

        let mut handles = Vec::new();
        for _ in 0..5 {
            let coll = coll.clone();
            handles.push(tokio::spawn(async move {
                coll.call(ProviderOp::RawTx, Duration::from_millis(100), |p| {
                    Box::pin(async move { p.get_raw_tx("anything").await })
                })
                .await
            }));
        }
        for h in handles {
            let _ = h.await;
        }

        // Order must be a contiguous permutation of {a,b,c} with "a" at the tail.
        let order = coll.current_order();
        assert_eq!(order.len(), 3);
        assert_eq!(order[2], "a", "a should be demoted to tail; got order {:?}", order);
    }

    #[tokio::test]
    async fn snapshot_stats_records_call_counts() {
        let a = Arc::new(TestProvider::new("a", 0));
        let b = Arc::new(TestProvider::new("b", 2));
        let coll = collection_with(vec![b.clone(), a.clone()]);

        // 3 calls — b fails each time, a succeeds each time.
        for _ in 0..3 {
            let _ = coll
                .call(ProviderOp::RawTx, Duration::from_secs(1), |p| {
                    Box::pin(async move { p.get_raw_tx("anything").await })
                })
                .await;
        }

        let stats: HashMap<&'static str, ProviderStats> =
            coll.snapshot_stats().into_iter().collect();
        let a_stats = stats.get("a").expect("a stats present");
        let b_stats = stats.get("b").expect("b stats present");
        assert_eq!(a_stats.calls, 3);
        assert_eq!(a_stats.successes, 3);
        assert_eq!(b_stats.calls, 3);
        assert_eq!(b_stats.hard_errors, 3);
    }
}
