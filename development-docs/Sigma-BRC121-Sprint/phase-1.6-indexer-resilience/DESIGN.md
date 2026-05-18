# Phase 1.6 — Indexer Resilience — DESIGN

> Sub-phase 1.6c deliverable. Architecture and commit-boundary spec for the indexer-resilience refactor.
>
> **Inputs:** [`README.md`](README.md) (phase plan), [`INVENTORY.md`](INVENTORY.md) (77 call sites + 4 appendices), kickoff session 2026-05-19 (decision matrix below).
>
> **Output:** this document — the spec the 1.6d implementation commits land against.
>
> **Last verified:** 2026-05-19 against working tree `e9d49f6`.

---

## 1. Goals and non-goals

### 1.1 Goals (in scope for 1.6d/1.6e)

1. **Eliminate the 15 `reqwest::Client::new()` no-timeout call sites** documented in INVENTORY.md's anti-pattern catalog. Every external call gets a bounded timeout.
2. **Reduce WhatsOnChain as a single point of failure** by introducing a 4-tier provider chain (ARC GorillaPool → WoC → JungleBus → Bitails) for the operations that have alternatives.
3. **Route 19 ad-hoc call sites through `cache_helpers`** so cache-first behavior is enforced uniformly. Cache stays in `parent_transactions` / `proven_txs` / `block_headers` — no schema change for caches.
4. **Move publish-path WoC fetches off the synchronous response path.** `acquireCertificate` / `publishCertificate` return immediately; a Monitor task absorbs the network work.
5. **Block-event-driven proof polling.** Wake `TaskCheckForProofs` on observed height advance instead of waiting up to 60 seconds.
6. **Adopt canonical adaptive soft-timeout on broadcast** (5s + 50ms/KiB cap 30s) with `moveServiceToLast` demotion. GorillaPool ARC stays primary.

### 1.2 Non-goals (explicitly deferred)

- ARC SSE — deferred per Appendix A.2 of INVENTORY.md. GorillaPool returns 404 on `/events`; TAAL SSE is build-cycle fragile. Revisit when either fixes.
- WoC paid-tier API key — defer to a stand-alone follow-up commit (or skip). No key exists today; anonymous tier has been tolerable.
- ARC proof-on-broadcast fast-path — defer. Rare case, low value, complicates the happy path.
- Settings-driven fallback ordering override — out of scope. Static priority for 1.6.
- Adaptive (latency-aware / observed-reliability) ordering — future enhancement, post-1.6.
- Bulk batch endpoints — wallet-toolbox does single-call only, so will Hodos. Batch returns are too ambiguous to error-handle reliably; `handlers.rs:8962` (WoC POST `/tx/raw`) migrates to a loop of single-call `Services::get_raw_tx`.
- C++ indexer changes — none. C++ side is out of phase per INVENTORY Section 10. Phase 1.6 is Rust-only.

### 1.3 Constraints carried in from prior phases

- **TAAL ARC stays fallback, never primary** (memory `project-taal-arc-unreliable-for-primary`). The hardcoded TAAL API key at `handlers.rs:8782` is intentional (memory `project-taal-arc-key-hardcoded`) and expires monthly between builds. Promotion would create a regression window every build cycle.
- **Caches must not poison themselves with failure-derived values** (memory `project-cache-no-poison-on-failure`). Negative results from transient provider failures must never be cached. The new Services layer never writes a cache; cache writes happen only in `cache_helpers` after a successful provider response.
- **Load-bearing UX safeguards** (INVENTORY Appendix A.4) are untouched: payment badge animation chain, right-click revoke menu, `DomainPermissionForm` Always-notify toggle, privacy perimeter prompts, `SessionManager` per-session counters. All live in C++/React/permission-engine code, not in the indexer path.

---

## 2. Architecture

### 2.1 `IndexerProvider` trait + error type

New file `rust-wallet/src/services/provider.rs`:

```rust
use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum IndexerError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("soft timeout after {0:?}")]
    SoftTimeout(Duration),
    #[error("provider {provider} returned status {status}: {body}")]
    ProviderStatus { provider: &'static str, status: u16, body: String },
    #[error("invalid response from {provider}: {reason}")]
    InvalidResponse { provider: &'static str, reason: String },
    #[error("not found")]
    NotFound,
}

#[derive(Debug, Clone)]
pub enum BlockKey { Hash(String), Height(u32) }

#[derive(Debug, Clone)]
pub struct TxStatus {
    pub txid: String,
    pub status: TxState,
    pub block_height: Option<u32>,
    pub block_hash: Option<String>,
    pub merkle_path_bump: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TxState {
    Unknown, InMempool, Mined, Rejected, DoubleSpendAttempted,
}

#[derive(Debug, Clone)]
pub enum OutspendStatus {
    Unspent,
    Spent { spending_txid: String, spending_vin: Option<u32> },
}

#[derive(Debug, Clone)]
pub struct BroadcastResult {
    pub provider: &'static str,
    pub txid: String,
    pub tx_status: String,
    pub merkle_path_bump: Option<String>,
    pub block_height: Option<u32>,
}

#[async_trait]
pub trait IndexerProvider: Send + Sync {
    fn name(&self) -> &'static str;

    async fn get_raw_tx(&self, txid: &str) -> Result<Vec<u8>, IndexerError>;
    async fn get_merkle_proof_tsc(&self, txid: &str) -> Result<serde_json::Value, IndexerError>;
    async fn get_block_header(&self, key: BlockKey) -> Result<BlockHeader, IndexerError>;
    async fn tx_status(&self, txid: &str) -> Result<TxStatus, IndexerError>;
    async fn outspend(&self, txid: &str, vout: u32) -> Result<OutspendStatus, IndexerError>;
    async fn fetch_utxos(&self, address: &str) -> Result<Vec<UtxoRecord>, IndexerError>;
    async fn broadcast_beef(&self, beef: &[u8]) -> Result<BroadcastResult, IndexerError>;

    /// Default `false`. Providers override when they can't serve an op
    /// (e.g. JungleBus returns `false` for `fetch_utxos`).
    fn supports(&self, op: ProviderOp) -> bool { true }
}

#[derive(Debug, Clone, Copy)]
pub enum ProviderOp {
    RawTx, MerkleProof, BlockHeader, TxStatus, Outspend, FetchUtxos, BroadcastBeef,
}
```

**Design notes:**

- Result-object pattern from canonical wallet-toolbox is expressed as `Result<T, IndexerError>` here. No panics; transport failures and provider rejections are both surfaced as `Err` variants. Matches Appendix B.4.
- `supports()` lets providers opt out of operations they can't serve. JungleBus returns `false` for `FetchUtxos` (it has history but not unspent sets). The `ProviderCollection` honors `supports()` when iterating.
- `IndexerError::NotFound` is a distinct variant — a 404 from a provider is **not** a soft failure to retry on. It's a load-bearing "tx doesn't exist yet" signal. Callers (e.g. orphan recovery) treat NotFound differently from Transport/SoftTimeout.

Providers implement this trait in `rust-wallet/src/services/providers/`:

```
services/
├── mod.rs               -- WalletServices struct, public re-exports
├── provider.rs          -- IndexerProvider trait, IndexerError, types
├── collection.rs        -- ProviderCollection<P> generic over provider type
└── providers/
    ├── mod.rs
    ├── arc_gorillapool.rs
    ├── arc_taal.rs        -- (hardcoded key per memory; stays fallback)
    ├── gorillapool_mapi.rs -- (legacy MAPI broadcast fallback)
    ├── gorillapool_ordinals.rs -- (UTXO fallback only)
    ├── whatsonchain.rs
    ├── junglebus.rs
    └── bitails.rs
```

### 2.2 `ProviderCollection<P>` with adaptive soft-timeout

New file `rust-wallet/src/services/collection.rs`:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use parking_lot::Mutex;

#[derive(Debug, Default, Clone)]
pub struct ProviderStats {
    pub calls: u64,
    pub successes: u64,
    pub soft_timeouts: u64,
    pub hard_errors: u64,
    pub last_used_at: Option<std::time::Instant>,
}

pub struct ProviderCollection<P: ?Sized> {
    /// Ordered providers; index 0 is the current head. Mutated by demotion.
    providers: Mutex<Vec<Arc<P>>>,
    /// Round-robin cursor for the canonical `.next()` advancement.
    cursor: AtomicUsize,
    /// Per-provider stats keyed by `provider.name()`.
    stats: Mutex<std::collections::HashMap<&'static str, ProviderStats>>,
}

impl<P: IndexerProvider + ?Sized> ProviderCollection<P> {
    pub fn new(providers: Vec<Arc<P>>) -> Self { /* ... */ }

    /// Try each `supports()`-eligible provider in current order. On `SoftTimeout`,
    /// demote the provider (move to last) before advancing to the next. On other
    /// errors, advance the cursor without demoting. Returns the first success or
    /// the last error encountered after exhausting all eligible providers.
    pub async fn call<F, R>(&self, op: ProviderOp, soft_timeout: Duration, f: F)
        -> Result<R, IndexerError>
    where
        F: for<'a> Fn(&'a P) -> futures::future::BoxFuture<'a, Result<R, IndexerError>>,
        R: Send;

    /// 5s base + 50ms per KiB, capped at 30s. Matches canonical `postBeef` formula.
    pub fn adaptive_soft_timeout_for_payload(bytes: usize) -> Duration {
        let extra_ms = (bytes as u64).saturating_mul(50) / 1024;
        let total = Duration::from_secs(5) + Duration::from_millis(extra_ms);
        std::cmp::min(total, Duration::from_secs(30))
    }

    pub fn snapshot_stats(&self) -> Vec<(&'static str, ProviderStats)>;
}
```

**Behavior contract:**

- The `f` closure is invoked with a `&P` reference; it constructs whatever future the operation needs against that provider. Genericity here is so the same `call` machinery handles all 7 ops without one method per op type.
- Soft timeout is enforced via `tokio::time::timeout(soft_timeout, future)`. On expiry the call returns `Err(SoftTimeout(d))` and the provider is demoted (`moveServiceToLast`).
- Demotion is permanent for the lifetime of the `ProviderCollection`. There is no auto-restoration in 1.6 (deliberate — wallet-toolbox also doesn't auto-restore, see Appendix B.2). The process restart re-orders to the default chain.
- `IndexerError::NotFound` short-circuits the chain — it's a positive signal that the tx genuinely doesn't exist, not a provider failure. Caller decides what to do.
- Stats are write-locked on every call; bench-mark says this is sub-microsecond at our call volume. If it becomes a bottleneck we move to per-provider `AtomicU64` counters.

### 2.3 `WalletServices` facade

New file `rust-wallet/src/services/mod.rs`:

```rust
pub struct WalletServices {
    /// Shared HTTP client. Used by every provider for connection-pool reuse and
    /// a default 30s hard timeout. Per-call soft timeouts on top of this.
    pub client: reqwest::Client,

    raw_tx:    ProviderCollection<dyn IndexerProvider>,
    proof:     ProviderCollection<dyn IndexerProvider>,
    header:    ProviderCollection<dyn IndexerProvider>,
    tx_status: ProviderCollection<dyn IndexerProvider>,
    outspend:  ProviderCollection<dyn IndexerProvider>,
    utxo:      ProviderCollection<dyn IndexerProvider>,
    broadcast: ProviderCollection<dyn IndexerProvider>,
}

impl WalletServices {
    pub fn new(chain: Chain) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))  // hard ceiling per request
            .pool_max_idle_per_host(8)
            .build()
            .expect("reqwest client build");

        // Construct provider instances once, share via Arc.
        let arc_gp  = Arc::new(ArcGorillaPoolProvider::new(client.clone()));
        let arc_tl  = Arc::new(ArcTaalProvider::new(client.clone()));
        let gp_mapi = Arc::new(GorillaPoolMapiProvider::new(client.clone()));
        let gp_ords = Arc::new(GorillaPoolOrdinalsProvider::new(client.clone()));
        let woc     = Arc::new(WhatsOnChainProvider::new(client.clone()));
        let jb      = Arc::new(JungleBusProvider::new(client.clone()));
        let bt      = Arc::new(BitailsProvider::new(client.clone()));

        Self {
            client,
            raw_tx:    ProviderCollection::new(vec![arc_gp.clone(), woc.clone(), jb.clone(), bt.clone()]),
            proof:     ProviderCollection::new(vec![arc_gp.clone(), woc.clone(), jb.clone(), bt.clone()]),
            header:    ProviderCollection::new(vec![woc.clone(),    jb.clone(), bt.clone()]),
            tx_status: ProviderCollection::new(vec![arc_gp.clone(), woc.clone(), jb.clone(), bt.clone()]),
            outspend:  ProviderCollection::new(vec![woc.clone(),    jb.clone()]),
            utxo:      ProviderCollection::new(vec![woc.clone(),    gp_ords.clone()]),
            broadcast: ProviderCollection::new(vec![arc_gp,         arc_tl, gp_mapi, woc]),
        }
    }

    pub async fn get_raw_tx(&self, txid: &str) -> Result<Vec<u8>, IndexerError> {
        self.raw_tx.call(ProviderOp::RawTx, Duration::from_secs(8),
            |p| Box::pin(p.get_raw_tx(txid))).await
    }

    pub async fn broadcast_beef(&self, beef: &[u8]) -> Result<BroadcastResult, IndexerError> {
        let soft = ProviderCollection::<dyn IndexerProvider>::adaptive_soft_timeout_for_payload(beef.len());
        self.broadcast.call(ProviderOp::BroadcastBeef, soft,
            |p| Box::pin(p.broadcast_beef(beef))).await
    }

    // ... analogous wrappers for proof, header, tx_status, outspend, fetch_utxos
}
```

**Per-operation soft-timeout defaults:**

| Operation | Soft timeout | Rationale |
|---|---:|---|
| `get_raw_tx` | 8s | matches existing 8s revocation check from `b9124bb` |
| `get_merkle_proof_tsc` | 10s | proofs are larger payloads |
| `get_block_header` | 8s | small payload |
| `tx_status` | 8s | status query is small |
| `outspend` | 8s | small payload |
| `fetch_utxos` | 15s | can be a large response on busy addresses |
| `broadcast_beef` | adaptive (5s + 50ms/KiB, cap 30s) | matches canonical |

These are soft timeouts. The `reqwest::Client`'s 30s ceiling is the hard timeout. A provider that doesn't respond within the soft window is demoted and the next provider gets a fresh window.

### 2.4 Cache pre-check pattern

`Services` does **no caching**. Caching stays in `cache_helpers.rs` and the SQLite tables (`parent_transactions`, `proven_txs`, `block_headers`). The flow:

```
caller
  │
  ▼
cache_helpers::get_or_fetch_X(state, txid)
  │
  ├─ Read X from SQLite table
  │     hit  → return cached value (no network call)
  │     miss → fall through
  │
  ▼
state.services.get_X(txid)
  │
  ▼
ProviderCollection.call(...)  →  Provider impl  →  HTTP
  │
  └─ on success: write to SQLite cache + return value
     on error:   return Err, NEVER write to cache
```

**Cache-no-poison invariant**: the cache layer writes only on successful provider response. A `Transport`, `SoftTimeout`, `ProviderStatus`, `InvalidResponse`, or `NotFound` error must result in zero cache writes. This is the rule from memory `project-cache-no-poison-on-failure`; restating because it's the most common Phase 1.6-era bug pattern we want to avoid.

The existing `fetch_tsc_proof_from_api` outer wrapper already implements ARC-primary + WoC-fallback. Phase 1.6 T1 generalizes this by routing through `Services` — but the cache-write pattern (write to `proven_txs` on success only) is preserved as-is.

### 2.5 `AppState` integration

`AppState` (currently 14 fields per `main.rs:80-95`) gets two new fields:

```rust
pub struct AppState {
    pub database: Arc<Mutex<WalletDatabase>>,
    pub auth_sessions: Arc<AuthSessionManager>,
    pub balance_cache: Arc<balance_cache::BalanceCache>,
    pub fee_rate_cache: Arc<fee_rate_cache::FeeRateCache>,
    pub price_cache: Arc<price_cache::PriceCache>,
    pub services: Arc<services::WalletServices>,                         // NEW (1.6d.B)
    pub utxo_selection_lock: Arc<tokio::sync::Mutex<()>>,
    pub create_action_lock: Arc<tokio::sync::Mutex<()>>,
    pub derived_key_cache: Arc<Mutex<HashMap<String, DerivedKeyInfo>>>,
    pub current_user_id: i64,
    pub shutdown: tokio_util::sync::CancellationToken,
    pub sync_status: Arc<RwLock<handlers::SyncStatus>>,
    pub backup_check_needed: Arc<Mutex<Option<(i64, i64)>>>,
    pub recovery_just_completed: Arc<AtomicBool>,
    pub last_known_block_height: Arc<AtomicU64>,                         // NEW (1.6d.F)
    pub pay402_reuse: Arc<Mutex<HashMap<(String, i64), Pay402ReuseEntry>>>,
}
```

**Slotting rationale:**

- `services` goes between `price_cache` and `utxo_selection_lock` — grouped with the other singleton caches.
- `last_known_block_height` goes between `recovery_just_completed` and `pay402_reuse` — grouped with the other atomic flags.
- Both fields are additive. No existing field is renamed, retyped, or moved. Handlers that don't reference the new fields are unchanged. Per `rust-wallet/CLAUDE.md` invariant 4 ("Do not change `AppState` struct without understanding all handlers that depend on it"), additive changes need only the construction site update in `main.rs::main()`.

**Construction site** (`main.rs` initialization):

```rust
let services = Arc::new(services::WalletServices::new(Chain::Main));
let last_known_block_height = Arc::new(AtomicU64::new(0));

let app_state = web::Data::new(AppState {
    // ... existing fields ...
    services: services.clone(),
    last_known_block_height: last_known_block_height.clone(),
    // ... rest ...
});

// Monitor receives the same handles
Monitor::start(app_state.clone(), services.clone(), last_known_block_height.clone());
```

---

## 3. Per-operation provider chains

Default chains constructed at startup. Locked from kickoff decision matrix.

| Operation | 1st | 2nd | 3rd | 4th |
|---|---|---|---|---|
| `tx_status` | ARC GorillaPool | WoC | JungleBus | Bitails |
| `get_raw_tx` | ARC GorillaPool | WoC | JungleBus | Bitails |
| `get_merkle_proof_tsc` | ARC GorillaPool | WoC | JungleBus | Bitails |
| `get_block_header` | WoC | JungleBus | Bitails | — |
| `outspend` | WoC | JungleBus | — | — |
| `fetch_utxos` | WoC | GorillaPool Ordinals | — | — |
| `broadcast_beef` | ARC GorillaPool | TAAL ARC | GorillaPool MAPI | WoC |

**Notes:**

- **Bitails** is fourth-tier on tx-data ops, never first. Independent team (Meysam Rezaei et al.), keyless free tier at 10 TPS / 1000 daily requests, already in use as 3rd-of-3 oracle quorum at `task_check_for_proofs.rs:853`. Including it as a fallback gives us telemetry on real-world reliability via `snapshot_stats()`. If 1.6e tests show it adds value, future phases may promote it; if it underperforms, the row drops.
- **ARC has no `get_block_header`** — `/v1/policy` is the only block-related ARC endpoint and it returns fee policy, not headers. Header chain is therefore WoC-primary.
- **ARC has no `outspend`** — outspend status is not part of the ARC API. JungleBus has spending data via tx history but no direct outspend endpoint either; provider impl computes it from history lookup. WoC stays primary.
- **JungleBus does NOT serve UTXOs** — it has tx/address history but not unspent sets. WoC is the only first-class UTXO source today. `gorillapool_ordinals` is already used as a fallback in `utxo_fetcher.rs:178`.
- **TAAL ARC stays fallback** in broadcast chain. Memory `project-taal-arc-unreliable-for-primary`.

---

## 4. T4 — Background-defer publish

### 4.1 Problem statement

Today, `acquireCertificate` and `publishCertificate` make 3–5 synchronous WoC calls inside the C++ 45s wallet-request timeout window. When WoC is slow, the user-visible flow fails ("wallet request timeout") even though the cert lands in the DB seconds later.

### 4.2 Solution shape

Split the publish flow into two phases:

1. **Synchronous (returns immediately to React)** — local-only work: parse certificate, validate signature, encrypt fields, build the unsigned tx, persist to `certificates` table with `publish_state='pending'`.
2. **Asynchronous (Monitor task)** — network work: BEEF building, ARC broadcast, proof acquisition, overlay submission. Updates `certificates.publish_state` as state advances.

New Monitor task: **`TaskHydrateCertificateContext`** at `monitor/task_hydrate_certificate_context.rs`. Runs on a 30s tick. Pulls certs in `publish_state IN ('pending', 'parents_fetching', 'parents_fetched', 'broadcasting', 'broadcast_complete', 'proof_pending')`, advances them through the state machine.

### 4.3 Certificate state machine (new `publish_state` column)

```
pending          → first state after handler returns; task picks it up next tick
  │ task fetches parent txs via Services
  ▼
parents_fetched  → BEEF can be built
  │ task builds BEEF + broadcasts via Services.broadcast_beef
  ▼
broadcasting    → broadcast in flight (set just before postBeef call)
  │ ARC response received
  ▼
broadcast_complete → tx accepted; wait for mined+proof
  │ TaskCheckForProofs picks up via existing proven_tx_req path
  ▼
published       → terminal success
        OR
failed          → terminal failure (with `publish_error TEXT NOT NULL`)
```

**State machine invariants:**

- Transitions are monotonic. A task that observes `published` or `failed` skips immediately.
- The Monitor's existing `try_lock` DB contention check applies — `TaskHydrateCertificateContext` skips a tick if a user request holds the DB lock.
- Idempotency: each state's work is safe to repeat. Re-broadcasting an already-accepted tx returns ARC's "already known" 409 path (`handlers.rs:8722-8746`), which is handled as success.
- A crash mid-broadcast leaves the cert in `broadcasting`. Next task tick re-evaluates: if ARC says the tx is in mempool/mined, advance to `broadcast_complete`. Mirrors the existing `TaskSendWaiting` recovery pattern.

### 4.4 React UX consequence

The frontend's Certificate tab (settings page) shows a state badge per cert:

| `publish_state` | Badge | User intuition |
|---|---|---|
| `pending`, `parents_fetching`, `parents_fetched`, `broadcasting`, `broadcast_complete`, `proof_pending` | "Publishing..." (spinner) | hand-off accepted, network work underway |
| `published` | "Published" (green) | terminal success |
| `failed` | "Publish failed" (red, click for details) | terminal failure with retry option |

The certificate is **usable for `proveCertificate` immediately after the sync phase** — the local DB has all the data needed for selective disclosure. Publishing affects on-chain attestation (via overlay services) but does not affect proveCertificate functionality.

This is the only user-visible UX change in Phase 1.6. None of INVENTORY Appendix A.4's load-bearing safeguards are touched.

### 4.5 Reuse target

`TaskHydrateCertificateContext` mirrors the existing `TaskReplayOverlay` pattern (currently undocumented in `monitor/CLAUDE.md` — drift item #1). Both are network-bound retry tasks that process a workqueue of partial-state rows. The implementation shares helper functions (`fetch_parent_for_tx`, `broadcast_with_retry`) with the existing tasks.

---

## 5. Block-event-driven `TaskCheckForProofs` (1.6d.F)

### 5.1 Today's behavior

`TaskCheckForProofs` runs unconditionally every 60 seconds. After a new block is mined, a recently-broadcast tx waits up to 60s before its proof is fetched.

### 5.2 New behavior

Add a `check_now: Arc<AtomicBool>` flag to `TaskCheckForProofs`. When `last_known_block_height` advances, set `check_now = true`. The Monitor's 30s tick checks the flag at the top of `TaskCheckForProofs.run()`; if set, the run executes immediately regardless of the 60s timer; clears the flag at the end of run.

The 60s timer stays as belt-and-braces. Worst case (no block height signal arrives), behavior is unchanged from today. Best case (block-height signal arrives 5s after a new block), proofs are fetched within 30s of mining instead of up to 60s.

### 5.3 `last_known_block_height` update paths

Two complementary write paths:

1. **Opportunistic update from response bodies.** Every `Services::*` call that observes a `blockHeight` in a provider response calls `state.last_known_block_height.fetch_max(h, Ordering::Relaxed)`. This is free — no extra round trip, just a lock-free atomic on data we already have. Provider implementations include this in their response-parse code.

2. **Dedicated quick-poll.** A new tiny task on the Monitor's 30s tick fetches `https://api.whatsonchain.com/v1/bsv/main/chain/info` (~100B response, keyless). Updates the atomic. This is the explicit signal — guarantees we observe height changes even when no other call is happening.

Both paths converge on `fetch_max` — multiple writers, no coordination needed, and a stale write is harmless (atomic only advances forward).

### 5.4 Wake mechanism

```rust
// In TaskCheckForProofs.run() prelude:
let was_check_now = self.check_now.swap(false, Ordering::Relaxed);
let elapsed_since_last_run = self.last_run.elapsed();
if !was_check_now && elapsed_since_last_run < Duration::from_secs(60) {
    return; // skip this tick, not due yet, no wake signal
}
// ... proceed with proof acquisition ...
```

The Monitor's main loop calls `TaskCheckForProofs.run()` every tick (30s). The early-return inside the task respects both the 60s timer and the wake flag.

### 5.5 Orphan-avoidance deliberation

Canonical wallet-toolbox's `TaskNewHeader` applies a one-cycle (~1min) delay before triggering downstream tasks, to avoid acting on blocks that get re-orged within a minute. Hodos deliberately skips this in 1.6.

**Rationale:** the cost of waking `TaskCheckForProofs` on an orphan is one cycle of wasted ARC `tx_status` queries — low cost, no incorrect state since the proof fetch will simply find no proof yet. The benefit of skipping the delay is proofs arrive ~30s sooner in the happy path.

If we ever observe false-wake noise in production (e.g. a sustained pattern of orphans causing visible thrash), adding a 5-second grace window is a one-line change. Not pre-optimizing.

### 5.6 No SSE in 1.6

Per INVENTORY Appendix A.2: GorillaPool ARC returns 404 on `/events`; TAAL ARC has SSE but the hardcoded key (memory `project-taal-arc-key-hardcoded`) makes it build-cycle-fragile. SSE is deferred to a future phase when either GorillaPool adds `/events` or TAAL ships a stable key model. The block-event-driven polling above is the 1.6 replacement.

---

## 6. 1.6d implementation commit boundaries

Six commits, each independently testable. Commit order is the safe-rollout order — earlier commits add infrastructure without changing behavior; later commits flip behavior over.

### 6.1 1.6d.A — Mechanical timeout injection (risk: LOW)

**Scope.** Add `.timeout(...)` to the 15 `reqwest::Client::new()` no-timeout call sites enumerated in INVENTORY's anti-pattern catalog.

**What changes.** Each site builds `reqwest::Client::builder().timeout(Duration::from_secs(N)).build()` with N chosen by call class:
- Sync-on-user-path raw-tx / outspend / paymail: 8s
- Sync-on-user-path UTXO fetch: 15s
- Background-task call: 30s

**What does not change.** No URL change, no behavior change, no provider chain, no `Services`. This commit is the "stop bleeding" pass — caps the worst symptoms before the architectural work lands.

**Verification.** `cargo test`. Manual smoke: acquire cert via SocialCert; confirm acquireCert still completes (timeout doesn't break the happy path). Manual: simulate WoC blackhole via `hosts` file → confirm flows fail in ≤8–15s instead of hanging until C++ 45s ceiling.

### 6.2 1.6d.B — Services facade scaffolding (risk: LOW)

**Scope.** Add `rust-wallet/src/services/` module per Section 2 above. Implement `IndexerProvider` trait, `ProviderCollection`, `WalletServices`, all 7 provider impls. Add `services: Arc<WalletServices>` field to `AppState`. Construct in `main.rs::main()`.

**What changes.** New module compiled and instantiated. `AppState` gains the field. **No call site migrated yet** — the facade is dormant.

**What does not change.** Every existing handler, task, and `cache_helpers` call site keeps its current behavior. The Services field exists but nothing reads it.

**Verification.** `cargo build` clean. `cargo test` clean (new unit tests for `ProviderCollection.call` demote behavior + `adaptive_soft_timeout_for_payload` formula). The wallet starts, handles requests as before, and `state.services.snapshot_stats()` returns empty maps because nothing called it yet.

### 6.3 1.6d.C — Migrate cache_helpers + 19 callers through Services (risk: MED)

**Scope.** This is the T1 commit. Three intertwined changes:
1. Upgrade `cache_helpers::fetch_parent_transaction_from_api` from WoC-only to `Services.get_raw_tx` (which provides the full 4-tier chain).
2. Upgrade `cache_helpers::fetch_and_cache_block_header` to use `Services.get_block_header`.
3. Migrate the 19 `Decision = T1` call sites (per INVENTORY Section 5/6/7) from inline `reqwest` to the upgraded helpers.

**What changes.** Publish-path, BEEF-build path, recovery path, monitor-task path all now go ARC-primary with WoC/JB/Bitails fallback for raw tx and merkle proof. Cache pre-check is preserved (helpers still hit `parent_transactions` / `proven_txs` / `block_headers` first).

**What does not change.** Cache schema. Cache-write semantics. Existing handler signatures. BRC-100 wire protocol. None of INVENTORY Appendix A.4's UX safeguards.

**Verification.** `cargo test`. Manual: acquire cert with WoC throttled (block WoC via firewall) → confirm cert acquires via ARC. Manual: acquire cert with ARC blackholed → confirm cert acquires via WoC. Manual smoke against canonical sites: YouTube/X/GitHub/WhatsOnChain → confirm no functional regression. Provider stats (`snapshot_stats()`) confirm non-WoC providers got exercised.

**Risk mitigation.** Per `rust-wallet/CLAUDE.md` invariant 3 ("Do not change wallet DB schema without asking"), this commit changes **no schema**. It only changes call sites. If a regression appears, it's revert-safe.

### 6.4 1.6d.D — Broadcast adaptive soft-timeout + moveServiceToLast (risk: MED)

**Scope.** Migrate the broadcast pipeline (`handlers.rs:8424, 8524, 8678, 8781`) to `Services.broadcast_beef`. Adopts the `5s + 50ms/KiB cap 30s` adaptive soft timeout and `moveServiceToLast` demotion.

**What changes.** Broadcast is now a chain: GorillaPool ARC → TAAL ARC → GorillaPool MAPI → WoC. On soft timeout per provider, the provider gets demoted and the next one tries. Soft timeouts are payload-aware — a 100 KiB BEEF gets 5s + 5s = 10s soft timeout per provider instead of the current 30s hard ceiling per provider.

**What does not change.** Primary remains GorillaPool ARC. TAAL ARC stays fallback (memory `project-taal-arc-unreliable-for-primary`). The hardcoded TAAL key (`handlers.rs:8782`) stays where it is (memory `project-taal-arc-key-hardcoded`). 409/460-469 error handling is preserved.

**Verification.** `cargo test` including new tests for `adaptive_soft_timeout_for_payload`. Manual: broadcast small tx (1 KB), confirm 5s soft timeout per provider. Manual: broadcast large tx (50 KB), confirm 7.5s soft timeout per provider. Manual: simulate ARC GP slow → confirm TAAL takes over after the first soft-timeout. Confirm subsequent broadcasts in same session try TAAL first (demoted GP is at tail). Confirm broadcast still succeeds in all scenarios.

**Risk mitigation.** Broadcast is load-bearing — every outgoing tx hits this path. Test extra carefully against the canonical sites' tx flows. Roll back means reverting one commit; broadcast falls back to existing pre-Services pipeline.

### 6.5 1.6d.E — Background-defer publish (T4) (risk: MED)

**Scope.** Section 4 work. Adds V20 migration (new `publish_state` and `publish_error` columns on `certificates` table). Splits `acquire_certificate` / `publish_certificate` handlers into sync + async phases. Adds `TaskHydrateCertificateContext` Monitor task. Updates React Certificate tab to render state badges.

**What changes.** Certificate handlers return ~immediately with `publish_state='pending'`. Monitor task absorbs all network work over the following 30–120 seconds. React displays a "Publishing..." badge during this window.

**What does not change.** Selective disclosure (`proveCertificate`) — works the moment the sync phase completes, regardless of publish state. No change to cert verification, signature, BRC-52 wire format, or revocation check.

**Verification.** Integration test: acquire cert, confirm sync phase returns in <2s, confirm async phase completes within 2 minutes under normal network, confirm proveCertificate works during the async window. Manual: kill the wallet during async phase, restart, confirm `TaskHydrateCertificateContext` resumes from the persisted state. Manual: acquire cert with WoC blackholed, confirm cert still publishes via ARC.

**Risk mitigation.** V20 migration is forward-only and additive (new nullable columns). The migration test suite must include an upgrade-from-V19 scenario. If a regression appears, the schema additions are backwards-compatible with V19 handlers (they ignore the new columns).

### 6.6 1.6d.F — Block-event trigger (1.6d.F) (risk: LOW–MED)

**Scope.** Section 5 work. Adds `last_known_block_height: Arc<AtomicU64>` to `AppState`. Adds opportunistic update in every relevant provider impl. Adds `check_now: Arc<AtomicBool>` to `TaskCheckForProofs`. Adds the WoC `/chain/info` quick-poll on the Monitor's 30s tick.

**What changes.** `TaskCheckForProofs` runs sooner after a new block — typically within 30s of mining instead of up to 60s.

**What does not change.** Proof-acquisition logic itself. ARC / WoC / JungleBus / Bitails call paths. The 60s belt-and-braces timer.

**Verification.** Unit test: simulate height advance, assert `check_now` flips, assert `run()` executes. Integration: broadcast tx, observe `proven_tx_req` reaching `completed` faster than the pre-change median. Manual: confirm chain/info quick-poll latency (~150ms per call, 30s cadence, negligible cost).

**Risk mitigation.** Both writes (opportunistic + quick-poll) are `fetch_max` — concurrent writers are safe. The check_now flag is a single-flip-per-cycle no-coordination toggle. No data races possible.

### 6.7 1.6d.G (optional, deferred) — WoC paid-tier API key

**Scope.** Add `WOC_API_KEY` env var read at startup, plumb through every WoC provider call. Document the env var.

**Decision.** Defer. We don't have a key today, anonymous tier (~3 req/sec) has worked for the wallet's call volume historically, and the post-1.6d.A timeouts already mitigate the worst rate-limit symptoms. Add the support in a stand-alone commit later — separate from the architecture refactor — when there's evidence the paid tier is worth $25/mo.

---

## 7. Open follow-ups (deferred from 1.6)

Logged here so future phases can find them. None of these are bugs; all are deliberate scope cuts.

| # | Item | Defer to | Notes |
|---|---|---|---|
| 1 | ARC SSE task | future phase | Reopen when GorillaPool adds `/events` OR TAAL ships a stable key model |
| 2 | WoC paid-tier API key | post-1.6 if needed | Stand-alone commit, separate from architecture |
| 3 | ARC proof-on-broadcast fast-path | post-1.6 polish | Rare case (already-MINED on broadcast), low value |
| 4 | Settings-driven fallback ordering override | future phase | Static priority is fine for 1.6 |
| 5 | Adaptive (latency-aware) provider ordering | future phase | Requires telemetry baseline first |
| 6 | Bulk batch endpoints (`/tx/raw` POST) | not planned | Wallet-toolbox doesn't do batch; partial-response handling is too ambiguous |
| 7 | Orphan-avoidance delay on block-height trigger | post-1.6 if observed | One-line addition when needed |
| 8 | Bitails promotion to 1st-tier on any op | post-1.6e if telemetry supports | Currently 4th-tier on tx-data ops |
| 9 | `task_check_for_proofs` 60s timer removal | post-1.6 if telemetry supports | Belt-and-braces redundancy for now |

---

## 8. Migration shape (V20)

Single new migration. Filename `migrate_v19_to_v20`. Migrations actually shipped end at V19 per `database/migrations.rs:1060`. The "V20–V24" tables in `rust-wallet/CLAUDE.md` are planning placeholders that never landed (already flagged by that file's own doc-drift note) — Phase 1.6's V20 is the first real V20.

```sql
-- V20: Add publish_state and publish_error to certificates for T4 background-defer
ALTER TABLE certificates
    ADD COLUMN publish_state TEXT NOT NULL DEFAULT 'pending';

ALTER TABLE certificates
    ADD COLUMN publish_error TEXT;

-- Backfill: any cert that already has a published-on-chain marker (existing
-- 'published' boolean column or equivalent) is set to 'published'. Anything
-- else is left in 'pending' and the next TaskHydrateCertificateContext tick
-- will re-validate state from the cert's existing on-chain references.
UPDATE certificates
   SET publish_state = 'published'
 WHERE published_at IS NOT NULL;
```

**Forward compatibility.** New columns are nullable / default-valued. V19 handlers running on a V20 DB ignore the new columns. (Not that we'd ever run a V19 handler against a V20 DB, but it's the safe shape.)

**Backward compatibility.** None claimed. Once V20 is applied, downgrade to V19 is unsupported. This matches all prior migrations.

---

## 9. Test plan (handoff to 1.6e)

Detailed enough that 1.6e can be authored from this section directly.

### 9.1 Unit tests (Rust)

| Module | Test |
|---|---|
| `services::collection` | `adaptive_soft_timeout_for_payload(0) == 5s` |
| `services::collection` | `adaptive_soft_timeout_for_payload(100 * 1024) == 10s` |
| `services::collection` | `adaptive_soft_timeout_for_payload(10 * 1024 * 1024) == 30s` (cap) |
| `services::collection` | `call()` returns first success without trying other providers |
| `services::collection` | `call()` demotes provider on `SoftTimeout`, advances on other errors |
| `services::collection` | `call()` returns `NotFound` without trying remaining providers |
| `services::collection` | `call()` skips providers whose `supports()` returns false |
| `services::collection` | Concurrent `call()` from N tokio tasks doesn't double-demote |
| `services::providers::*` | Each provider impl parses a fixture response correctly |
| `services::providers::*` | Each provider impl returns the correct `IndexerError` variant for HTTP 4xx/5xx |
| `monitor::task_hydrate_certificate_context` | State machine advances through `pending → published` happy path |
| `monitor::task_hydrate_certificate_context` | Crash mid-`broadcasting` recovers to `broadcast_complete` on next tick |
| `monitor::task_check_for_proofs` | `check_now=true` short-circuits the 60s timer |

### 9.2 Integration tests (Rust + Actix test server)

| Scenario | Expectation |
|---|---|
| acquireCertificate with WoC blackholed (firewall rule) | Cert acquires via ARC; flow completes in <10s |
| acquireCertificate with ARC blackholed | Cert acquires via WoC; flow completes in <10s |
| acquireCertificate with WoC + ARC both blackholed | Cert acquires via JungleBus or Bitails; flow completes in <20s |
| acquireCertificate with all 4 providers blackholed | Cert returns `publish_state='pending'`, `TaskHydrateCertificateContext` retries; user-visible response in <2s |
| publishCertificate with WoC blackholed | Same as above; `publish_state` advances through state machine via ARC |
| BEEF build with one parent tx unreachable on every provider | Build fails cleanly with `IndexerError::NotFound`; caller surfaces error; no half-built BEEF persisted |
| `wallet/sync` with WoC slow (mock 5s delay) | Soft timeout fires; UTXO fetch falls through to GorillaPool Ordinals |
| Broadcast a small tx (1 KB) with GorillaPool ARC slow | Soft timeout 5s fires; TAAL takes over; broadcast succeeds; provider demotion observed in stats |
| Broadcast a 50 KB tx with GorillaPool ARC slow | Soft timeout 7.5s fires; TAAL takes over |
| Block height advance during a quiet window | `last_known_block_height` increments via quick-poll; `TaskCheckForProofs.check_now` flips; next tick runs immediately |
| Block height advance observed via ARC response body | `last_known_block_height` increments via opportunistic update; same wake behavior |
| Migration V19 → V20 on a real wallet DB | New columns present; existing cert rows preserved; backfill of `publish_state='published'` correct |

### 9.3 Manual smoke (canonical sites, both Windows + macOS per CLAUDE.md Testing Standards)

After 1.6d.E lands (the highest-UX-impact commit):

**Standard 15-minute pass.** Per CLAUDE.md Testing Standards, includes:
- youtube.com, x.com, github.com (auth + media)
- 2–3 video/media sites
- 1–2 news sites
- whatsonchain.com (BSV-specific)

For each site, confirm:
- Page loads
- Login/auth flow works if applicable
- Any payment flow (BRC-121) fires the green-dot payment badge animation (INVENTORY Appendix A.4 #1)
- Right-click "Manage Site Permissions" still opens (INVENTORY Appendix A.4 #2)
- No spurious permission prompts
- Wallet panel opens and shows correct state

### 9.4 macOS parity verification

Phase 1.6 is Rust-only, so macOS parity is implicit. Verify by running the standard manual smoke pass on a Mac build after 1.6d.A through 1.6d.F all land.

---

## 10. Doc-drift fixes (apply during 1.6c/1.6d, not deferred)

Six items from INVENTORY.md's "Doc-drift items" section. Apply opportunistically as the relevant code changes:

| # | File | Fix |
|---|---|---|
| 1 | `rust-wallet/src/monitor/CLAUDE.md` | Task list currently shows 10 tasks; actual is 13. Add `task_backup`, `task_consolidate_dust`, `task_replay_overlay`, `task_retry_peerpay_outbox`. Apply during 1.6d.E (which adds the 14th task `task_hydrate_certificate_context`). |
| 2 | `development-docs/Sigma-BRC121-Sprint/phase-1.6-indexer-resilience/README.md` | "Initial known offenders" table tags `cache_helpers.rs:15` as "ARC primary + WoC fallback" — correct to "WoC-only" (T1 fixes this). Apply during 1.6d.C. |
| 3 | `development-docs/Sigma-BRC121-Sprint/phase-1.6-indexer-resilience/README.md` | "What we already do correctly" repeats the misclaim — same fix. Apply during 1.6d.C. |
| 4 | `rust-wallet/src/CLAUDE.md` | "External API Dependencies" table claims `cache_helpers.rs` does ARC-primary tx hex — correct to acknowledge WoC-only today (will be true after T1). Apply during 1.6d.C. |
| 5 | `development-docs/Sigma-BRC121-Sprint/phase-1.6-indexer-resilience/README.md` §"Step plan" §Step 3 | `parent_tx_cache` → `parent_transactions` (actual table name). Apply during 1.6d.C. |
| 6 | `rust-wallet/CLAUDE.md` | "Migrations V20-V24" section describes migrations that don't exist. Either remove the section entirely or relabel as "planning placeholders, not shipped". Apply during 1.6d.E (which adds the actual V20). |

---

## 11. Summary

Phase 1.6 ships a single coherent architecture — `WalletServices` + `ProviderCollection` — that replaces 19 ad-hoc indexer call sites and 15 no-timeout `reqwest::Client::new()` sites with a 4-tier provider chain (ARC GP → WoC → JungleBus → Bitails) on the tx-data operations, a 4-tier broadcast chain with adaptive soft-timeout, and a Monitor task that absorbs publish-path network work off the synchronous response path. One new SQLite migration (V20). No C++ changes. No BRC-100 wire-protocol changes. No changes to the load-bearing UX safeguards. Six commits, sequenced for safe rollout.

The deferred items list in §7 is the explicit roadmap for what 1.6 does **not** ship.
