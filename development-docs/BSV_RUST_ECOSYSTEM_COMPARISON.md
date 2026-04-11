# BSV Rust Ecosystem Comparison — Improvement Opportunities

> **Date**: 2026-04-07  
> **Repos analyzed**: [bsv-wallet-toolbox-rs](https://github.com/Calhooon/bsv-wallet-toolbox-rs), [bsv-rs](https://github.com/Calhooon/bsv-rs), [bsv-rust-sdk](https://github.com/b1narydt/bsv-rust-sdk)  
> **Compared against**: Hodos `rust-wallet`

---

## Priority 1 — High Value, Moderate Effort

### 1a. Broadcast Failure Classification
**Source**: wallet-toolbox `process_action`  
**What**: Classify ARC broadcast errors as **permanent** (double-spend error -25, invalid tx) vs **transient** (orphan mempool, service timeout, error 460/465/473). Only retry transient failures.  
**Why**: Hodos already partially does this (orphan mempool handling) but doesn't have a systematic classification. The toolbox's pattern prevents wasting retries on permanent failures and ensures transient ones always get retried.  
**Where to apply**: `handlers.rs` broadcast logic, `monitor/task_send_waiting.rs`, `monitor/task_check_for_proofs.rs`  
**Effort**: 1-2 days

### 1b. Adaptive Service Timeouts
**Source**: wallet-toolbox `ServiceCollection`  
**What**: EMA-based timeout adjustment per API provider. Tracks last 32 call durations, multiplies EMA by 2.0x, bounds to 5-60s. Providers that slow down get longer timeouts automatically; consistently fast providers get tighter timeouts.  
**Why**: Hodos uses fixed 30s/10s timeouts. When WoC is slow, we wait the full timeout. When it's fast, we're being too generous with failure detection.  
**Where to apply**: `utxo_fetcher.rs`, `cache_helpers.rs`, `price_cache.rs`  
**Effort**: 2-3 days

### 1c. BEEF Compaction Task
**Source**: wallet-toolbox `compact_beef`  
**What**: Background task that retroactively trims proven ancestors from stored BEEF data. Once a parent tx has its own merkle proof in `proven_txs`, its raw bytes no longer need to be in the BEEF ancestry chain.  
**Why**: `parent_transactions` table grows unbounded. This would reclaim storage over time as proofs are acquired.  
**Where to apply**: New monitor task `task_compact_beef.rs`  
**Effort**: 1 day

### 1d. Use WhatsOnChain BUMP Proof Endpoint Directly
**Source**: WhatsOnChain (raised by @deggen on X, 2026-04-10)  
**What**: WhatsOnChain now exposes `/tx/{txid}/proof/bump` which returns merkle proofs in BUMP format directly. Currently we fetch TSC from WoC's `/tx/{txid}/proof/tsc` and convert internally — but for ARC's BUMP responses we already convert BUMP → TSC.  
**Why**: 
- Lets us skip the TSC↔BUMP conversion when fetching from WoC fallback
- Standardizes on BUMP as the canonical wire format (BRC-74)
- Reduces complexity in `cache_helpers.rs` proof acquisition flow
- Aligns with how ARC already returns proofs

**Example URL**: `https://api.whatsonchain.com/v1/bsv/main/tx/{txid}/proof/bump`  
**Where to apply**: `cache_helpers.rs` (proof fetching), `beef.rs` (potentially simplify TSC code paths)  
**Effort**: 1-2 days  
**Note**: Should be evaluated alongside the broader BSV Rust ecosystem comparison sprint.

---

## Priority 2 — Worth Evaluating Deeper

### 2a. Consider bsv-rs as a Dependency (replacing hand-rolled crypto)
**Source**: bsv-rs  
**What**: bsv-rs provides BRC-42, BRC-43, signing, AES-GCM, BEEF parsing, transaction building — all with `k256` (audited, constant-time, zeroize-on-drop).  
**Status**: bsv-rs is NOT on crates.io yet (uses local path `../rust-sdk` in toolbox's Cargo.toml). Can't depend on it until published.  
**Risk**: Massive refactor touching every crypto module. Current code works and has been battle-tested.  
**Recommendation**: Monitor for crates.io publication. Don't adopt now, but plan for eventual migration when it becomes a stable published crate. The security benefits of audited `k256` over raw `secp256k1` 0.28 are meaningful long-term.

### 2b. Storage Trait Hierarchy
**Source**: wallet-toolbox  
**What**: 5-level trait hierarchy (`Reader -> Writer -> Sync -> Provider -> MonitorStorage`) allowing swappable backends (SQLite, MySQL, remote JSON-RPC).  
**Why**: Would enable future multi-backend support (e.g., encrypted remote backup storage).  
**Risk**: Heavy refactor of all 19 repos. Current direct-SQLite approach is simpler and works fine for a single-device wallet.  
**Recommendation**: Not now. Only worth it if you need MySQL or remote storage.

### 2c. BRC-29 RemittanceManager Pattern
**Source**: bsv-rust-sdk  
**What**: Formal `RemittanceManager` with injectable `NonceProvider` and `LockingScriptProvider` traits, full wire-format types (OptionTerms, SettlementArtifact, ReceiptData, RefundData).  
**Why**: PeerPay implementation works but is tightly coupled in `handlers.rs`. The trait-based approach would make it testable and extensible.  
**Recommendation**: Review when PeerPay needs enhancements. Not urgent.

---

## Priority 3 — Nice to Have

### 3a. Constant-Time Comparisons
**Source**: bsv-rs (`subtle::ConstantTimeEq`)  
**What**: Constant-time HMAC and token comparisons to prevent timing side-channels.  
**Current Hodos**: `verify_hmac_sha256` in `signing.rs` — needs verification on whether it already uses constant-time comparison.  
**Effort**: Very low — add `subtle` crate, change a few comparison sites.

### 3b. Fuzz Testing for Parsers
**Source**: bsv-rs  
**What**: Fuzz targets for BEEF parser, transaction parser, script parser.  
**Why**: `beef.rs` (77KB) handles complex binary parsing. Fuzz testing would catch edge cases.  
**Effort**: Low — add `cargo-fuzz` targets for BEEF + transaction deserialization.

### 3c. FIFO Spend Lock
**Source**: wallet-toolbox  
**What**: `tokio::sync::Mutex` that serializes spending operations in FIFO order.  
**Current Hodos**: Uses `utxo_selection_lock` (std Mutex) + `create_action_lock`. Same concept.  
**Delta**: Small. FIFO ordering is marginally better for fairness under contention.

---

## Not Worth Adopting

| Pattern | From | Why Skip |
|---------|------|----------|
| Hand-rolled crypto (BigNumber, secp256k1, ECDSA) | bsv-rust-sdk | Security risk. No constant-time guarantees, no zeroize. Our `secp256k1` crate is better. |
| Zero-dependency crypto | bsv-rust-sdk | Philosophically interesting but practically dangerous for production wallet software handling real money. |
| 19 feature flags | bsv-rs | Overkill for an application (vs. a library). We're not a published crate. |
| Per-module error enums | bsv-rust-sdk | Our unified `thiserror` approach is fine for an application. |

---

## Repo Profiles

| Repo | Author | Scale | Maturity | License | Standout Feature |
|------|--------|-------|----------|---------|-----------------|
| **bsv-wallet-toolbox-rs** | Calhooon (official) | ~51K lines, 702 tests | Production-grade | MIT | Storage trait hierarchy, adaptive timeouts, BEEF compaction |
| **bsv-rs** | Calhooon (official) | ~100K lines, 2,578 tests | Production-grade | MIT/Apache-2.0 | Audited crypto (k256), fuzz targets, 2,044 cross-SDK test vectors |
| **bsv-rust-sdk** | b1narydt (3rd party) | ~30K lines est. | Beta | Open BSV License | Zero-dep crypto (risky), BRC-29 RemittanceManager, benchmarks |
| **Hodos rust-wallet** | Us | ~100K+ lines | Production | Proprietary | Full browser-wallet integration, AuthFetch, MessageBox, PeerPay, multi-platform |

---

## Key Relationships

- **bsv-wallet-toolbox-rs** and **bsv-rs** are official Babbage/MetaNet ecosystem — Rust ports of the TypeScript SDK and wallet-toolbox
- **bsv-rust-sdk** is an independent third-party reimplementation by b1narydt
- **Hodos** adopted wallet-toolbox patterns during Phase 3-5 alignment work (schema, monitor tasks, output model)
- bsv-rs is NOT on crates.io yet — uses local path dependency in toolbox
- All repos implement the same BSV protocol specs (BRC-42/43/2/62/74/100), so structural similarity is expected but implementations are independent
