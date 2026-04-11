# BSV Rust Ecosystem Alignment Plan

**Created:** 2026-04-11
**Status:** Planning — adoption decisions captured below
**Parent:** [`wallet-efficiency-and-bsv-alignment.md`](./wallet-efficiency-and-bsv-alignment.md)
**Source doc:** [`../BSV_RUST_ECOSYSTEM_COMPARISON.md`](../BSV_RUST_ECOSYSTEM_COMPARISON.md)

---

## Purpose

The BSV ecosystem comparison doc surfaced 12 candidate improvements from sibling Rust repos (`bsv-wallet-toolbox-rs`, `bsv-rs`, `bsv-rust-sdk`). This plan locks in adopt/skip/defer decisions for each, with effort estimates, risk analysis, and acceptance criteria, so they can be implemented in a focused sprint without re-litigating the bucketing.

**Important context from the parent plan:** I initially miscategorized item 1c (BEEF Compaction Task) as a "backup-shrinker" — verified by reading `backup.rs` that this is wrong. `parent_transactions` is already entirely cleared from the on-chain backup at `backup.rs:991`. BEEF compaction is still useful as a runtime/in-memory state win, but it does NOT shrink the backup. Bucket assignment corrected below.

---

## Decision matrix

### ADOPT — order of implementation

| Order | Item | Effort | Why now | Acceptance criteria |
|---|---|---|---|---|
| 1 | **3a** Constant-time comparisons | ~30 min | No-brainer security improvement, zero risk | `subtle::ConstantTimeEq` used in `verify_hmac_sha256` and any other equality checks on secret-derived values |
| 2 | **1a** Broadcast failure classification | 1–2 days | Cleans up ad-hoc retry logic across `task_send_waiting`, `task_check_for_proofs`, and the broadcast handler | Single classification function used by all 3 broadcast paths; permanent errors never retried; transient errors always retried |
| 3 | **1c** BEEF compaction task | 1 day | Reduces in-memory state and speeds up sends; benefit compounds over time | New monitor task; `parent_transactions` table size shrinks measurably after 1 hour of activity; no broken sends |
| 4 | **1d** WhatsOnChain BUMP endpoint | 1–2 days | Removes TSC↔BUMP conversion complexity in proof acquisition flow | `cache_helpers.rs` fetches BUMP directly from WoC where available; fallback to TSC remains; conversions deleted from happy path |
| 5 | **1b** Adaptive service timeouts | 2–3 days | Real (but invisible) responsiveness win; handles slow APIs gracefully | EMA-based timeout in `cache_helpers.rs`, `utxo_fetcher.rs`, `price_cache.rs`; bounded 5–60s; per-provider tracking |

**Total effort: ~6–9 days for the full ADOPT bucket.**

### SKIP — explicit reasoning so we don't relitigate

| Item | Why skip | When to revisit |
|---|---|---|
| **2a** bsv-rs migration | Unpublished crate (`path = "../rust-sdk"` in toolbox), massive refactor, irreversible if buggy in security-critical signing path. Current `secp256k1` 0.28 is itself audited (Bitcoin Core's reference impl bindings), well-maintained. The "audited k256 vs raw secp256k1" framing in the comparison doc is misleading — both are audited; it's a question of which audit you trust. **Skip indefinitely** unless `secp256k1` becomes unmaintained or we find a concrete bug the migration would fix. | Only if `secp256k1` upstream goes unmaintained or has an unfixable security advisory |
| **2b** Storage trait hierarchy | 5-level trait hierarchy enables swappable backends (SQLite, MySQL, remote JSON-RPC). Heavy refactor of all 19 repos. We're a single-device wallet — MySQL and remote storage are not requirements. | Only if a future feature requires a non-SQLite backend |
| **3c** FIFO spend lock | `tokio::sync::Mutex` for FIFO ordering of spend operations. Hodos already has `utxo_selection_lock` + `create_action_lock`. The delta is "marginally better fairness under contention." Not worth a refactor for a marginal improvement. | Skip indefinitely |

### DEFER — good ideas, wrong moment

| Item | Why defer | Trigger to revisit |
|---|---|---|
| **2c** BRC-29 RemittanceManager | Formal `RemittanceManager` with injectable `NonceProvider`/`LockingScriptProvider` traits and full wire-format types. PeerPay works as-is in `handlers.rs`. Refactoring for testability/extensibility is valuable but only when we're actually extending PeerPay. | Next time PeerPay needs significant new features (e.g., recurring payments, multi-recipient, partial settlement) |
| **3b** Fuzz testing for parsers | `cargo-fuzz` targets for `beef.rs` (77KB), transaction parser, script parser. Real value but 1–2 day setup cost is hard to justify before MVP launch. | Post-launch hardening sprint |

---

## Detailed adoption notes

### 1. `3a` Constant-time comparisons (FIRST — easiest, zero risk)

**What:** Use `subtle::ConstantTimeEq` for HMAC and token comparisons to prevent timing side-channels where an attacker measures comparison time to infer secret bytes.

**Where:** Audit needed but starting point is `crate::crypto::signing::verify_hmac_sha256`. Also any place that does `secret_a == secret_b` byte comparison.

**Implementation:**
```toml
# Cargo.toml
[dependencies]
subtle = "2.5"
```

```rust
// In signing.rs:
use subtle::ConstantTimeEq;

pub fn verify_hmac_sha256(key: &[u8], data: &[u8], expected: &[u8]) -> bool {
    let computed = hmac_sha256(key, data);
    computed.ct_eq(expected).into()
}
```

**Verification:** Existing tests for `verify_hmac_sha256` should still pass. Add one regression test confirming `ct_eq` returns false for differing inputs and true for identical inputs. No timing test needed for our purposes — we're trusting the `subtle` crate's constant-time guarantee.

**Risk:** Essentially zero. `subtle` is widely used (it's the same crate `k256` uses internally). API is drop-in.

---

### 2. `1a` Broadcast failure classification (SECOND — quality win)

**What:** Centralize the logic that decides "is this broadcast error permanent (don't retry) or transient (do retry)?" Currently scattered across `task_send_waiting`, `task_check_for_proofs`, and the broadcast handler with subtle differences.

**Source pattern:** `wallet-toolbox` `process_action`. Categories:
- **Permanent:** double-spend (ARC error -25), invalid tx, missing inputs, script failures
- **Transient:** orphan mempool (BEEF BUMP validation pending), service timeout, error 460/465/473, HTTP 5xx

**Where:** New module `rust-wallet/src/broadcast_classification.rs` (or extend `cache_errors.rs`). Functions used by:
- `handlers.rs::send_transaction` (the main broadcast path)
- `monitor/task_send_waiting.rs` (already has `is_permanent_error` per code reading — formalize and reuse)
- `monitor/task_check_for_proofs.rs` (the cleanup-on-rejection path)

**Hodos already has partial logic:** `task_send_waiting.rs` defines `is_permanent_error` (verified earlier). The work is to (a) move it to a shared location, (b) extend the classification with the toolbox's known-error catalog, (c) wire all 3 broadcast paths to use it.

**Acceptance criteria:**
- Single function `classify_broadcast_error(arc_response: &str) -> BroadcastFailureKind` with variants `Permanent` / `Transient` / `Unknown`
- All 3 broadcast paths call it
- `Permanent` failures: tx marked failed immediately, no retry, ghost output cleanup runs
- `Transient` failures: tx stays in `sending` status, monitor retries on next tick
- `Unknown`: treated as `Transient` by default (safer to retry than to lose) but logged at WARN level so we can identify gaps

**Risk:** Medium. This touches the broadcast path, which is critical. Mitigation: run the existing send_waiting tests + add new tests for each error code.

---

### 3. `1c` BEEF compaction task (THIRD — runtime win, NOT a backup-shrinker)

**What:** New monitor task that retroactively trims proven ancestors from the `parent_transactions` table. Once a parent transaction has its own merkle proof in `proven_txs`, its raw bytes no longer need to live in BEEF ancestry storage.

**Where:** New file `rust-wallet/src/monitor/task_compact_beef.rs`. Register in `monitor/mod.rs` with a slow interval (e.g., 1 hour).

**Why this is NOT a backup-shrinker (correction from earlier analysis):** `compress_for_onchain` at `backup.rs:991` already calls `payload.parent_transactions.clear()` — the entire table is wiped from the on-chain backup. BEEF compaction reduces the LOCAL `parent_transactions` table size, which:
- ✅ Speeds up `build_beef_for_txid` ancestry walks (fewer rows to iterate)
- ✅ Reduces local SQLite DB size (faster startup, smaller working set)
- ✅ Reduces memory usage in `cache_helpers.rs` parent fetches
- ❌ Does NOT shrink the on-chain backup (already stripped)

**Pseudocode:**
```rust
// task_compact_beef.rs
pub async fn run(state: &web::Data<AppState>) -> TaskOutcome {
    // For each row in parent_transactions:
    //   if proven_txs has a proof for this txid AND the proof is older than 24h:
    //     DELETE FROM parent_transactions WHERE txid = ?
    //
    // Hold DB lock briefly per batch (e.g., 50 rows), drop between batches.
    // Stop after compacting up to 1000 rows per run to avoid long lock holds.
}
```

**Acceptance criteria:**
- Task runs every 1 hour
- Trims rows where `parent_transactions.txid` has a corresponding `proven_txs` row with `created_at` > 24h ago
- Local DB `parent_transactions` row count drops measurably after 1 hour of test activity
- No broken sends — BEEF building falls back to fetching from `cache_helpers.rs` for parents that were compacted

**Risk:** Low if conservative threshold (24h gives plenty of time for any in-flight tx to settle). Higher if aggressive — could break sends if a parent is compacted while a child is mid-broadcast. Conservative threshold is the right call.

---

### 4. `1d` WhatsOnChain BUMP endpoint (FOURTH — cleanup)

**What:** WhatsOnChain now exposes `/v1/bsv/main/tx/{txid}/proof/bump` returning merkle proofs in BUMP format directly. Currently we fetch TSC from `/proof/tsc` and convert internally. ARC already returns BUMP. Switching the WoC fallback to BUMP standardizes on BRC-74 BUMP as the canonical wire format.

**Where:** `cache_helpers.rs::fetch_tsc_proof_from_api` (the WoC fallback path). Possibly simplify TSC↔BUMP code paths in `beef.rs`.

**Implementation:** Mostly mechanical. Replace the TSC URL + parsing with BUMP URL + parsing. Keep the TSC code path as a fallback for backward compatibility (some older WoC instances may not have the BUMP endpoint yet).

**Acceptance criteria:**
- New WoC fetches use BUMP endpoint by default
- TSC fallback still works if BUMP endpoint returns 404
- Existing tests for proof acquisition still pass
- One new test confirming BUMP endpoint parsing

**Risk:** Low. The endpoint is new but well-specified. Worst case: BUMP fetch fails, we fall back to TSC.

---

### 5. `1b` Adaptive service timeouts (FIFTH — invisible but real win)

**What:** EMA-based per-provider timeout tracking. Tracks last 32 call durations, multiplies the EMA by 2.0x to get a target timeout, bounds it to 5–60s. Slow providers get longer timeouts automatically; fast providers get tighter timeouts (faster failure detection).

**Where:** Three call sites:
- `cache_helpers.rs` — proof and parent-tx fetches
- `utxo_fetcher.rs` — UTXO queries
- `price_cache.rs` — price API calls

**Source pattern:** `wallet-toolbox::ServiceCollection`. Each provider (WoC, ARC, GorillaPool, CryptoCompare, CoinGecko) gets its own `AdaptiveTimeout` instance.

**Implementation:** New module `rust-wallet/src/adaptive_timeout.rs`. Per-provider state is a `Mutex<VecDeque<Duration>>` of recent call durations + an `AtomicU64` cached EMA.

**Acceptance criteria:**
- New `AdaptiveTimeout` struct with `record_duration()` and `current_timeout()` methods
- Used in all 3 sites listed above
- Bounded 5–60s
- Tested: starting from cold, stays at default until 4+ samples, then converges
- Existing 30s/10s constants removed from those 3 sites

**Risk:** Medium. Timeouts that are too tight cause spurious failures; too loose cause user-visible hangs. The 5–60s bound prevents extreme misbehavior. Mitigation: roll out with conservative initial state (start at 30s, not the EMA target) and let it converge over the first few minutes of usage.

---

## Files affected (cross-reference with parent doc deconfliction map)

| File | Items that touch it | Coordination notes |
|---|---|---|
| `rust-wallet/src/handlers.rs` | 1a (broadcast classification) | Large file. Backup efficiency plan also touches this. Coordinate via small focused commits. |
| `rust-wallet/src/cache_helpers.rs` | 1b, 1d | Single sprint touches both — merge together |
| `rust-wallet/src/utxo_fetcher.rs` | 1b | Standalone |
| `rust-wallet/src/price_cache.rs` | 1b | Standalone |
| `rust-wallet/src/crypto/signing.rs` | 3a | Standalone, first item |
| `rust-wallet/src/monitor/mod.rs` | 1c (registration) | Standalone |
| `rust-wallet/src/monitor/task_compact_beef.rs` | 1c (new file) | New file |
| `rust-wallet/src/monitor/task_send_waiting.rs` | 1a | Existing logic to refactor |
| `rust-wallet/src/monitor/task_check_for_proofs.rs` | 1a | Wire to shared classifier |
| `rust-wallet/src/broadcast_classification.rs` | 1a (new file) | New file |
| `rust-wallet/src/adaptive_timeout.rs` | 1b (new file) | New file |
| `rust-wallet/Cargo.toml` | 3a (`subtle`) | Standalone |

## Implementation order rationale

1. **3a first** — zero-risk warmup, builds confidence
2. **1a second** — touches the most files, do it while context is fresh, unblocks 1c
3. **1c third** — small standalone task, easy verification
4. **1d fourth** — minor cleanup, prepares for 1b
5. **1b last** — most complex change, most testing required

Items 1a and 1c could be swapped (1c is faster but 1a unblocks more downstream work). Default to the order above unless 1a's broadcast logic refactor proves harder than estimated.

## Verification protocol per item

Each item must pass before moving to the next:

1. **Build clean** — `cargo build --release` from `rust-wallet/`. No new warnings related to the change.
2. **Existing tests pass** — `cargo test` from `rust-wallet/`. No regressions.
3. **New tests added** — Each item has at least one test specific to its acceptance criteria.
4. **Manual smoke test** — Trigger the affected code path on a real wallet, observe expected behavior.
5. **Commit message includes the acceptance criteria** so future readers know what "done" meant.

## Out of scope

- **Refactoring `handlers.rs` into smaller modules** — Tempting given its 13K+ line size, but a separate concern. Don't bundle.
- **Adding more BSV protocol support (BRC-95+, etc.)** — Roadmap item, not in this sprint.
- **Replacing `actix-web` with `axum` or other framework** — Not on the table.

---

## Acceptance criteria for the whole plan

This plan is "done" when:

1. All 5 ADOPT items have been implemented and merged
2. Each item has its own commit with documented acceptance criteria
3. All existing tests pass; new tests added for each item
4. The 3 SKIP items remain skipped (no scope creep)
5. The 2 DEFER items have explicit "revisit when X" trigger conditions documented
6. `BSV_RUST_ECOSYSTEM_COMPARISON.md` has a "Status as of YYYY-MM-DD" section pointing back to this doc and noting which items were adopted
7. The post-beta3-cleanup doc has a session log entry summarizing the results

---

*Implementation can run in parallel with the backup efficiency plan after the measurement step is complete. See parent doc § "Affected files (deconfliction map)" for file conflict warnings.*
