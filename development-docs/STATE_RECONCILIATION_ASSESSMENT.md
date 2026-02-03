# State Reconciliation Assessment and Upgrade Plan

**Created**: 2026-01-30
**Last Updated**: 2026-02-03
**Status**: Comprehensive assessment complete. Ready for implementation prioritization.
**Objective**: Assess and upgrade UTXO/transaction state management to meet or exceed industry standards on accuracy, privacy, security, and dependability (zero token loss).

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current Architecture](#current-architecture)
3. [State Machines](#state-machines)
4. [Background Services](#background-services)
5. [Concurrency & Crash Safety](#concurrency--crash-safety)
6. [Gap Analysis vs BSV SDK wallet-toolbox](#gap-analysis-vs-bsv-sdk-wallet-toolbox)
7. [Database Schema Comparison](#database-schema-comparison)
8. [Potential Bugs & Edge Cases](#potential-bugs--edge-cases)
9. [Assessment by Pillar](#assessment-by-pillar)
10. [Research Findings](#research-findings)
11. [Recommendations](#recommendations)
12. [Implementation Priority](#implementation-priority)
13. [Success Criteria](#success-criteria)

---

## Executive Summary

The Hodos Browser wallet has a solid state reconciliation foundation with multiple defensive layers. The core design decisions — serialization lock on createAction, never-overwrite-spent invariant on UTXO sync, optimistic reservation with `pending-*` placeholders, and startup crash recovery — are sound and align with industry practices.

**Three critical areas were identified since the original assessment that have been implemented:**
1. ARC Status Polling (`arc_status_poller.rs`) — now active, 60s interval
2. BEEF Cache Pre-population (`cache_sync.rs`) — now active, 10-min interval
3. Dynamic Fee Rate (`fee_rate_cache.rs`) — now active, ARC policy-based

**Remaining gaps of concern (ordered by risk to user funds):**
1. **No UnFail mechanism** — transactions marked failed may have actually been mined (MEDIUM-HIGH risk)
2. **No chain reconciliation for disappeared UTXOs** — UTXOs spent externally are never detected (MEDIUM risk)
3. **No reorg handling** — confirmed transactions could become orphaned (LOW risk on BSV)
4. **Ancestry validation disabled** — `utxo_validation.rs` disabled due to false positives (LOW risk, ARC validates)

---

## Current Architecture

### Data Flow

```
createAction (serialized via lock)
  ├─ Validate baskets/tags
  ├─ Parse inputBEEF
  ├─ Select wallet UTXOs (from DB cache, fallback to WhatsOnChain API)
  ├─ Reserve UTXOs (is_spent=1, spent_txid='pending-{timestamp}')
  ├─ Build unsigned transaction
  ├─ Store in PENDING_TRANSACTIONS (in-memory HashMap)
  └─ Return reference number

signAction
  ├─ Retrieve pending transaction by reference
  ├─ Apply SDK-provided spends (for two-phase PushDrop flow)
  ├─ Sign wallet inputs (derive key per address index, SIGHASH_ALL_FORKID)
  ├─ Build Atomic BEEF (BRC-95):
  │   ├─ Copy inputBEEF chain (if provided)
  │   ├─ Fetch wallet parent transactions (cache → DB → API)
  │   ├─ Fetch/cache Merkle proofs (ARC priority → WhatsOnChain)
  │   ├─ Build ancestry chains for unconfirmed parents (30s timeout)
  │   └─ Sort topologically (BRC-62)
  ├─ Update database state:
  │   ├─ Update txid (unsigned → signed)
  │   ├─ Update UTXO txids for change outputs
  │   ├─ Update broadcast_status: pending → broadcast
  │   ├─ Update spent_txid: pending-{ts} → real txid
  │   ├─ Cache signed tx in parent_transactions (for future BEEF)
  │   └─ Invalidate balance cache
  └─ Return Atomic BEEF + txid

Background Services (concurrent):
  ├─ UTXO sync (5 min) — fetch UTXOs from WhatsOnChain, upsert to DB
  ├─ ARC status poller (60 sec) — check broadcast tx status
  ├─ BEEF cache sync (10 min) — pre-fetch parent txs and merkle proofs
  └─ Cleanup (runs with UTXO sync):
      ├─ Delete failed UTXOs
      ├─ Mark stale unproven (>1 hr) as failed
      ├─ Restore stale pending-* reservations (>5 min)
      ├─ Restore inputs of failed broadcasts
      └─ Mark stale pending transactions (>15 min) as failed

Startup Cleanup:
  ├─ Find transactions stuck in 'pending' >5 min → fail + restore UTXOs
  └─ Restore all 'pending-*' placeholder reservations
```

### Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `src/handlers.rs` | ~6300 | All HTTP handlers including createAction (L2871), signAction (L5286) |
| `src/utxo_sync.rs` | ~305 | Background UTXO sync + cleanup_failed_utxos |
| `src/arc_status_poller.rs` | ~180 | ARC status polling for broadcast transactions |
| `src/balance_cache.rs` | ~170 | In-memory balance cache with 30s TTL |
| `src/cache_sync.rs` | ~200 | Background BEEF cache pre-population |
| `src/fee_rate_cache.rs` | ~130 | ARC policy-based dynamic fee rates |
| `src/database/utxo_repo.rs` | ~740 | UTXO CRUD, reservation, restoration |
| `src/database/transaction_repo.rs` | ~400 | Transaction CRUD, broadcast status |
| `src/database/models.rs` | ~115 | Data models for all tables |
| `src/database/migrations.rs` | ~900 | Schema v1-v14 |
| `src/main.rs` | ~425 | Startup, cleanup, service initialization |
| `src/utxo_validation.rs` | ~300 | Ancestry validation (DISABLED) |

---

## State Machines

### Transaction State Machine (4 states)

```
               ┌──────────┐
 createAction  │ pending  │──── startup cleanup (>5 min) ───────┐
               └────┬─────┘                                     │
                    │ signAction completes                       │
               ┌────▼─────┐                                     │
               │broadcast │──── periodic cleanup (>15 min) ─────┤
               └────┬─────┘                                     │
                    │ ARC MINED              ARC REJECTED ──────┤
               ┌────▼─────┐                              ┌─────▼────┐
               │confirmed │                              │  failed  │
               └──────────┘                              └──────────┘
```

**Stored in**: `transactions.broadcast_status` (migration v10)

### UTXO Lifecycle State Machine

```
 API fetch / ┌──────────┐  createAction   ┌──────────────────┐
 upsert      │ unspent  │ ──────────────► │ reserved         │
             │(is_spent │                 │(is_spent=1,      │
             │  =0)     │                 │ spent_txid=      │
             └────┬─────┘                 │ 'pending-{ts}')  │
                  │                       └────────┬─────────┘
                  │                                │ signAction
                  │                       ┌────────▼─────────┐
                  │                       │ spent            │
                  │                       │(is_spent=1,      │
                  │                       │ spent_txid=      │
                  │                       │ real txid)       │
                  │                       └────────┬─────────┘
                  │                                │ cleanup (30 days)
                  │                       ┌────────▼─────────┐
                  │                       │ purged (deleted) │
                  │                       └──────────────────┘
                  │
                  │ failed broadcast cleanup
                  ◄──────────────────────────── restore from spent
```

### UTXO Status Field (for optimistic outputs, migration v9)

```
  signAction creates output   ┌───────────┐
  ──────────────────────────►  │ unproven  │
                               └─────┬─────┘
                                     │
                    ┌────────────────┼────────────────┐
                    │ ARC MINED     │                 │ timeout (>1 hr)
               ┌────▼─────┐        │           ┌─────▼────┐
               │completed │        │           │  failed  │ → deleted by cleanup
               └──────────┘        │           └──────────┘
                                   │
                    ARC REJECTED───┘
```

### wallet-toolbox Comparison: ActionStatus (8 states)

```
 'unprocessed' → 'unsigned' → 'sending' → 'unproven' → 'completed'
                                  │              │
                                  ▼              ▼
                              'failed'       'nosend'
                                              'nonfinal'
```

**Key difference**: wallet-toolbox separates `unprocessed` (created, no signing started) from `unsigned` (partially signed, waiting for spends) from `sending` (actively broadcasting). We collapse these into `pending` and `broadcast`.

**Overlapping status systems**: Note that our codebase has TWO partially-overlapping status systems:
1. `ActionStatus` enum in `action_storage.rs`: `Created`, `Signed`, `Unconfirmed`, `Pending`, `Confirmed`, `Aborted`, `Failed` (7 states)
2. `broadcast_status` column on `transactions` table (migration v10): `pending`, `broadcast`, `confirmed`, `failed` (4 states)

The wallet-toolbox uses a single clean `TransactionStatus` with 9 states plus a separate `ProvenTxReqStatus` for proof tracking. Our two overlapping systems can cause confusion — for instance, a transaction could have `ActionStatus::Confirmed` but `broadcast_status='broadcast'` if the enum and column get out of sync. Consider consolidating into a single status field in a future refactor.

### wallet-toolbox ProvenTxReqStatus (10+ states)

```
 'unknown' → 'sending' → 'unmined' → 'unconfirmed' → 'notifying' → 'completed'
                              │              │
                              ▼              ▼
                         'doubleSpend'   'invalid'
                                              │
                                              ▼
                                          'unfail' → (re-check) → 'unmined' or 'invalid'
```

**Key difference**: The `unfail` state is a recovery mechanism with no equivalent in our system. When a tx is marked invalid/failed, it can transition to `unfail` for re-verification, then either recover to `unmined` or stay `invalid`.

---

## Background Services

### Service Comparison

| Service | Hodos | wallet-toolbox | Notes |
|---------|-------|---------------|-------|
| UTXO sync from chain | `utxo_sync.rs` every 5 min | TaskSyncWhenIdle | Ours fetches from WhatsOnChain; theirs synchronizes with storage provider |
| Proof checking | `arc_status_poller.rs` every 60s | TaskCheckForProofs every 2 hrs (or on new block) | Ours polls ARC directly; theirs uses getMerklePath service |
| Abandoned tx detection | Startup + periodic cleanup (5 min) | TaskFailAbandoned every 8 min | Theirs is continuous; ours runs during UTXO sync cleanup |
| Failed tx recovery | None | TaskUnFail every 10 min | **Gap**: we never re-check failed transactions |
| Reorg handling | None | TaskReorg (deactivated header queue) | **Gap**: BSV reorgs are rare but not impossible |
| Status propagation | Implicit (via broadcast_status filter) | TaskReviewStatus every 5 min | Theirs explicitly propagates tx status to outputs |
| Broadcast retry | None | TaskSendWaiting every 8 sec | **Gap**: we broadcast once and hope; they retry |
| BEEF cache | `cache_sync.rs` every 10 min | N/A (on-demand) | We pre-populate; they fetch on demand |
| Fee rate | `fee_rate_cache.rs` 1-hr TTL | Service-level | Similar approach |
| Block monitoring | None | TaskNewHeader + TaskClock | **Gap**: we use fixed intervals only |
| Purge old data | 30-day spent UTXO cleanup | TaskPurge | Similar |

### Hodos Background Service Details

**UTXO Sync** (`utxo_sync.rs`):
- Interval: 300 seconds (5 minutes), 30-second startup delay
- Scans addresses 0 through `highest_used_index + 20` (gap limit)
- Calls WhatsOnChain `GET /v1/bsv/main/address/{address}/unspent`
- `upsert_utxos()`: inserts new UTXOs, updates `last_updated` on existing — **never changes `is_spent`**
- Marks addresses as `used=1` when UTXOs found
- Runs `cleanup_failed_utxos()` after sync (5 steps, see architecture diagram above)

**ARC Status Poller** (`arc_status_poller.rs`):
- Interval: 60 seconds, 45-second startup delay
- Queries: transactions with `broadcast_status='broadcast'`, max 20 per batch
- Rate limiting: 200ms delay between ARC API calls
- Status handling:
  - MINED → `confirmed` + cache merkle proof (block height + BUMP)
  - SEEN_ON_NETWORK / ACCEPTED / STORED → stay in `broadcast` (normal)
  - DOUBLE_SPEND_ATTEMPTED / REJECTED → `failed`
  - 404 → skip (expected for new transactions)

**BEEF Cache Sync** (`cache_sync.rs`):
- Interval: 600 seconds (10 minutes)
- Batch size: 50 UTXOs per cycle
- For unspent UTXOs without cached proofs:
  1. Fetch parent transaction (3-tier: local transactions → parent_transactions cache → WhatsOnChain API)
  2. Fetch TSC Merkle proof (WhatsOnChain, enhanced with block height)
  3. Cache both in database
- Careful lock management: DB lock dropped before every async API call

---

## Concurrency & Crash Safety

### Locks

| Lock | Type | Scope | Purpose |
|------|------|-------|---------|
| `create_action_lock` | `tokio::sync::Mutex<()>` | Entire createAction handler | Prevents concurrent UTXO selection and conflicting transaction chains |
| `utxo_selection_lock` | `tokio::sync::Mutex<()>` | Declared but unused | Originally for UTXO selection; superseded by create_action_lock |
| `database` | `Arc<Mutex<WalletDatabase>>` | All DB operations | Single connection protected by std Mutex |
| `balance_cache` | `Arc<RwLock<Option<CachedBalance>>>` | Balance reads/writes | RwLock allows concurrent reads |
| `derived_key_cache` | `Arc<Mutex<HashMap<...>>>` | PushDrop signing cache | Maps derived pubkey → derivation params |

### Crash Recovery Points

| Crash Point | State Left | Recovery |
|-------------|-----------|----------|
| After UTXO reservation, before signing | UTXOs marked `pending-{ts}` | Startup: `restore_pending_placeholders()` |
| After signing, before broadcast_status update | Transaction in memory, UTXOs reserved | Startup: find pending tx >5 min, restore UTXOs, mark failed |
| After broadcast_status='broadcast', before ARC confirms | UTXOs spent with real txid | ARC poller will eventually confirm or fail; periodic cleanup marks stale pending txs failed after 15 min |
| Process killed during BEEF building (30s timeout) | UTXOs reserved with pending-* | Startup: restore_pending_placeholders() |
| Database write fails mid-transaction | Partial state | SQLite atomic writes per statement; no multi-statement transactions used |

### Critical Invariant: Never Overwrite is_spent

In `utxo_repo.rs:37-44`, `upsert_utxos()` only updates `last_updated` for existing UTXOs:
```rust
// IMPORTANT: Do NOT change is_spent status here!
// UTXOs marked as spent should only be un-spent if the spending transaction
// was rejected by the network (handled separately in failed tx cleanup)
```

This prevents the sync from accidentally "resurrecting" spent UTXOs that the wallet knows about but WhatsOnChain hasn't processed yet.

---

## Gap Analysis vs BSV SDK wallet-toolbox

### Updated Gap Table (post code review)

| Area | Hodos Implementation | wallet-toolbox | Gap Severity | Impact |
|------|---------------------|---------------|-------------|--------|
| **Transaction states** | 4 states (pending/broadcast/confirmed/failed) | 8 ActionStatus + 10+ ProvenTxReqStatus | Moderate | Less granular debugging, but functionally adequate for our single-user desktop model |
| **ARC status polling** | `arc_status_poller.rs` — 60s interval, 20 tx batch | TaskCheckForProofs — triggered on new block or every 2 hrs | **Resolved** | Our approach polls more frequently but without block-height triggers |
| **ARC callbacks** | Not implemented | X-CallbackUrl webhooks | Low | Desktop wallet can't receive callbacks; polling is the correct approach |
| **Abandoned tx detection** | Startup + periodic cleanup (5-min threshold) | TaskFailAbandoned every 8 min (configurable abandonedMsecs) | Low | Our cleanup runs during UTXO sync (every 5 min) AND at startup; functionally equivalent |
| **Source of truth** | External API (WhatsOnChain) is authoritative for UTXO state | Local wallet is authoritative; blockchain APIs only used for proof acquisition | **Medium** | If WhatsOnChain is unavailable, we cannot determine our balance. The SDK knows its balance immediately from local state. |
| **Chain reconciliation** | Sync adds new UTXOs only; never removes disappeared ones | TaskReviewStatus propagates tx status to outputs | **Medium** | UTXOs spent externally (e.g., imported seed used elsewhere) are never detected |
| **Failed tx recovery (UnFail)** | None — failed is a terminal state | TaskUnFail every 10 min: re-checks merkle path, recovers to 'unmined' | **Medium-High** | Risk of permanent fund loss if a tx was prematurely marked failed but actually mined |
| **Reorg handling** | None | TaskReorg: queues deactivated headers, reproves with 10-min aging, 3 retries | **Low** | BSV reorgs are extremely rare; ~0 blocks reorged in normal operation |
| **Broadcast retry** | Single broadcast attempt (in SDK/overlay) | TaskSendWaiting every 8 sec retries unsent txs | **Low-Medium** | Our architecture delegates broadcasting to the SDK/overlay network, not the wallet |
| **Block-height triggers** | Fixed intervals only | TaskNewHeader + TaskClock | **Low** | Block triggers would improve proof checking efficiency but polling works |
| **Status propagation to outputs** | Implicit via SQL JOIN filter on broadcast_status | Explicit TaskReviewStatus updates output spendable/spentBy | **Low** | Our JOIN-based approach achieves the same result without separate propagation |
| **UTXO management targets** | No minimum UTXO count or value targets | OutputBasket has numberOfDesiredUTXOs (default 6) and minimumDesiredUTXOValue (default 10000 sat) | **Low** | Nice optimization for wallets with high transaction volume; not critical |
| **UTXO selection strategy** | Select all unspent, sort by satoshis descending, greedy | Three-tier: exact match → closest above target → largest below target | **Low** | SDK's approach minimizes change output fragmentation. Our greedy approach works but may accumulate dust |
| **Ancestry validation** | `utxo_validation.rs` (DISABLED) | Implicit via BEEF validation | **Low** | ARC validates BEEF on broadcast; local validation caused false positives |

### Gaps That Were Closed Since Original Assessment

1. **ARC Status Polling**: Implemented in `arc_status_poller.rs` with 60s polling, MINED/REJECTED handling, merkle proof caching
2. **Dynamic Fee Calculation**: Implemented in `fee_rate_cache.rs` with ARC `/v1/policy` endpoint, 1-hour TTL, sanity bounds
3. **Continuous Abandoned Detection**: The `cleanup_failed_utxos()` function now runs every 5 minutes (with UTXO sync), not just at startup
4. **Broadcast Status Filtering**: The `get_unspent_by_addresses()` and `calculate_balance()` queries now exclude UTXOs from `pending`/`failed` broadcast_status transactions via LEFT JOIN

---

## Database Schema Comparison

### Hodos vs wallet-toolbox Table Mapping

| Hodos Table | wallet-toolbox Equivalent | Key Differences |
|-------------|-------------------------|-----------------|
| `wallets` | `users` | Ours stores mnemonic directly; theirs uses identityKey reference |
| `addresses` | (implicit in outputs) | Ours has explicit address table with HD indices; theirs derive addresses from outputs |
| `utxos` | `outputs` | Ours combines UTXO + output; theirs separates with `spendable`, `spentBy`, `senderIdentityKey`, `derivationPrefix/Suffix` |
| `transactions` | `transactions` | Theirs has `provenTxId` FK to proven_txs; `isOutgoing` flag; `inputBEEF` stored; `rawTx` stored |
| `parent_transactions` | (implicit in proven_txs) | Ours caches raw hex; theirs stores in proven_txs after proof |
| `merkle_proofs` | `proven_txs` | Ours: separate table linked to parent_transactions. Theirs: proven_txs are immutable records with height, index, merklePath, blockHash, merkleRoot |
| `block_headers` | (in chaintracker service) | Ours: cached block headers. Theirs: managed by Chaintracks service |
| `baskets` | `output_baskets` | Similar. Theirs adds `numberOfDesiredUTXOs`, `minimumDesiredUTXOValue` |
| `output_tags` + `output_tag_map` | `output_tags` + `output_tags_map` | Equivalent |
| N/A | `proven_tx_reqs` | **Gap**: Request tracking for proof acquisition with retry logic, attempt counts, and status state machine |
| N/A | `commissions` | **Gap**: Fee tracking for Paymail/P2P payments |
| N/A | `tx_labels` + `tx_labels_map` | **Gap**: Transaction-level labeling (we have output tags but not tx labels) |
| N/A | `certificates` + `certificate_fields` | We have certificates table (migration v7) |
| N/A | `monitor_events` | **Gap**: Event logging for debugging |
| N/A | `sync_states` | **Gap**: Multi-device sync state tracking |
| N/A | `settings` | **Gap**: Persistent settings (chain, dbType, scriptLimits) |
| `transaction_inputs` + `transaction_outputs` | (implicit in rawTx) | Ours stores parsed inputs/outputs; theirs stores rawTx blob |
| `domain_whitelist` | N/A | Browser-specific, not in wallet-toolbox |
| `messages` | N/A | BRC-33 message relay, not in wallet-toolbox |

### Schema Differences That Impact State Reconciliation

1. **No `proven_tx_reqs` table**: wallet-toolbox tracks each proof request with attempt counts, status, and history. This enables the unfail mechanism (re-checking proofs for failed transactions). We could add this without schema changes by using existing transaction fields, but a dedicated tracking mechanism would be cleaner.

2. **No `isOutgoing` flag on transactions**: wallet-toolbox distinguishes incoming vs outgoing transactions. This matters for status propagation — an incoming transaction's failure should NOT restore any UTXOs (they weren't ours to spend). We don't have this distinction.

3. **No `spendable` field on UTXOs**: wallet-toolbox has an explicit `spendable` boolean separate from spend status. This allows temporarily marking outputs as non-spendable (e.g., during unfail review) without changing the spent state.

4. **No `rawTx` stored on transactions**: wallet-toolbox stores the full raw transaction. This is useful for unfail (re-broadcasting a failed tx that may have actually been mined). We store signed txs in `parent_transactions` but only for BEEF ancestry purposes.

---

## Potential Bugs & Edge Cases

### Critical (Could Lose Funds)

#### Bug 1: No UnFail Mechanism
**Location**: `cleanup_failed_utxos()` in `utxo_sync.rs:194-206`
**Scenario**: A transaction is broadcast, ARC returns a transient error (network timeout, 5xx), we mark it as `failed`, and the cleanup deletes the failed UTXOs and restores inputs. But the transaction actually propagated and gets mined. Result: the wallet thinks it still has the input UTXOs (double-counted) while the change output was deleted (lost).
**Impact**: Balance inflation (shows more than actually available) + lost change output.
**Likelihood**: Low but non-zero. Network errors during broadcast do happen.
**Fix**: Before permanently cleaning up a failed transaction, check WhatsOnChain or ARC one more time to verify it wasn't actually mined. Implement a `review` state between `failed` and cleanup.

#### Bug 2: Race Between ARC Poller and Periodic Cleanup
**Location**: `arc_status_poller.rs:147-159` and `utxo_sync.rs:271-286`
**Scenario**: The ARC poller marks a transaction as `failed` (DOUBLE_SPEND_ATTEMPTED). Concurrently, the cleanup in `utxo_sync.rs` runs and finds pending transactions older than 15 minutes, marking them failed too. If both run at the same time on different threads, they could both try to restore UTXOs, but this is safe because `restore_spent_by_txid()` is idempotent (only restores UTXOs with matching spent_txid). However, if the ARC poller marks it failed and the cleanup runs before the ARC poller's next cycle, the cleanup may delete the ghost UTXOs before we've confirmed whether the tx was actually mined.
**Impact**: Potential loss of change outputs if timing aligns poorly.
**Likelihood**: Very low. The std::Mutex on the database serializes these operations.
**Fix**: Add the UnFail mechanism (see Recommendation 1).

#### Bug 3: UTXO Sync Never Removes Disappeared UTXOs
**Location**: `utxo_repo.rs:37-44` (upsert_utxos never changes is_spent)
**Scenario**: User imports their mnemonic into another wallet and spends funds from there. Hodos's UTXO sync fetches the unspent set from WhatsOnChain, which no longer includes the spent UTXO. But `upsert_utxos()` only inserts new UTXOs and updates timestamps on existing ones — it never marks an existing unspent UTXO as spent.
**Impact**: Balance inflation (shows UTXOs that no longer exist on chain). User tries to spend them, ARC rejects with "missing inputs".
**Likelihood**: Low for single-wallet users; higher if seed is shared or recovered.
**Fix**: During sync, compare the API's unspent set against local unspent UTXOs for each address. Any local UTXO not in the API response should be flagged for review (not immediately marked spent — the API could have lag).

### Moderate (Degraded Experience)

#### Bug 4: Balance Cache Shows Stale Data After External Events
**Location**: `balance_cache.rs`
**Scenario**: The balance cache has a 30-second TTL. If the user receives funds (new UTXO appears on-chain), the balance won't update until: (a) the cache expires, AND (b) the UTXO sync runs (every 5 minutes). So the balance could be up to 5 minutes stale for incoming transactions.
**Impact**: User sees old balance; no fund loss.
**Fix**: After UTXO sync, explicitly invalidate the balance cache. Currently `cache_sync.rs` doesn't invalidate it. The UTXO sync in `utxo_sync.rs` also doesn't invalidate it after inserting new UTXOs.

#### Bug 5: Stale Pending Transactions Marked Failed May Have Unproven Outputs
**Location**: `utxo_sync.rs:228-242`
**Scenario**: A `pending-*` reservation older than 5 minutes is restored. But what if the transaction was actually being processed by the SDK (long two-phase PushDrop flow)? The wallet restores the UTXOs, but the SDK may still call signAction with the original reference, find the pending transaction, and try to complete it — with UTXOs that may have been re-spent.
**Impact**: Potential double-spend attempt (ARC would reject).
**Likelihood**: Low. The PENDING_TRANSACTIONS in-memory map is also cleared on restart.
**Fix**: The 5-minute timeout is reasonable for most flows. For very long two-phase operations, consider a longer timeout or a "heartbeat" mechanism.

#### Bug 6: `utxo_selection_lock` Is Declared but Never Used
**Location**: `main.rs:56`
**Scenario**: The `utxo_selection_lock` field exists in `AppState` but `createAction` uses `create_action_lock` instead. The unused lock is dead code.
**Impact**: None (the create_action_lock is the correct serialization mechanism).
**Fix**: Remove `utxo_selection_lock` from AppState to reduce confusion.

### Low (Edge Cases)

#### Bug 7: Ancestry Chain Timeout May Produce Invalid BEEF
**Location**: `handlers.rs:5959-6025`
**Scenario**: The ancestry chain building has a 30-second timeout. If it times out, we proceed with "partial BEEF". But ARC may reject partial BEEF if an unconfirmed parent has no proven ancestors.
**Impact**: Transaction broadcast failure. User can retry.
**Fix**: On timeout, log a warning and consider falling back to single-parent BEEF (simpler but may need ARC retry).

#### Bug 8: No SQLite Transaction Wrapping for Multi-Statement State Changes
**Location**: `handlers.rs:6064-6162` (post-signing state updates in signAction)
**Scenario**: The post-signing state updates in signAction perform ~8 separate SQL operations (update txid, update UTXOs, update status, update broadcast_status, update spent_txid, cache signed tx, etc.). If the process crashes mid-sequence, some updates may be applied and others not.
**Impact**: Inconsistent state requiring manual cleanup.
**Likelihood**: Very low (all operations are fast and sequential).
**Fix**: Wrap the post-signing state updates in a SQLite transaction (`BEGIN`/`COMMIT`). This would make all updates atomic.

---

## Assessment by Pillar

### 1. Accuracy (Balance Must Always Be Correct)

**Current Score: 7/10**

**Strengths:**
- Balance calculation excludes UTXOs from pending/failed transactions via SQL JOIN filter
- Balance cache with 30s TTL prevents showing very stale data
- `upsert_utxos()` never overwrites `is_spent` — prevents sync from undoing local state
- UTXO reservation with `pending-*` placeholders prevents double-selection

**Weaknesses:**
- Balance cache not invalidated after UTXO sync inserts new UTXOs (Bug 4)
- UTXOs spent externally are never detected (Bug 3) — balance shows phantom funds
- No verified balance (could cross-check sum against blockchain API)
- 5-minute sync interval means incoming funds visible after delay

**Recommendations:**
- Invalidate balance cache after UTXO sync completes
- Implement a lightweight "disappeared UTXO" detection during sync (compare API unspent set with local unspent set per address)
- Consider on-demand sync when user opens wallet panel (in addition to periodic)

### 2. Privacy (ECC with Many Addresses)

**Current Score: 6/10**

**Strengths:**
- HD wallet with gap limit (20 unused addresses ahead) — standard BIP-32 practice
- BRC-42 ECDH key derivation for per-counterparty addresses
- Master key address (index -1) kept separate
- Private keys never leave Rust process

**Weaknesses:**
- UTXO sync queries WhatsOnChain for EVERY address up to gap limit, sequentially. WhatsOnChain sees all addresses belonging to the same wallet and can correlate them by timing and IP.
- No Tor/proxy support for API calls
- Gap limit of 20 may be insufficient for high-volume BRC-100 usage (each protocol interaction generates new addresses via BRC-42)
- All API calls go through a single IP — easy to fingerprint as "one wallet"

**Recommendations:**
- Batch address queries to reduce timing correlation (WhatsOnChain may support batch APIs)
- Consider randomizing the order of address queries
- Add optional proxy/SOCKS5 support for API calls
- Consider using JungleBus or a local SPV node for address monitoring instead of WhatsOnChain
- Evaluate if gap limit of 20 is sufficient for heavy BRC-100 usage; if not, increase to 40

### 3. Security (No Unauthorized Spending)

**Current Score: 8/10**

**Strengths:**
- Private keys only in Rust process memory (compile-time memory safety)
- No `unsafe` blocks in key-handling code
- Serialization lock prevents concurrent UTXO selection (prevents self-double-spend)
- BSV ForkID SIGHASH prevents cross-chain replay
- TXID verification for cached parent transactions
- Merkle proof caching for SPV validation
- 100% of signing happens locally (no remote signing service)

**Weaknesses:**
- API responses (WhatsOnChain, ARC) are not cryptographically verified — a MITM could inject fake UTXOs
- CORS is `allow_any_origin()` in development — should be locked down for production
- Mnemonic stored in plaintext in SQLite database (encrypted at rest by OS, but not by wallet)
- No rate limiting on wallet endpoints (localhost:3301 only accessible locally, but still)
- `cleanup_old_spent()` deletes spent UTXOs after 30 days — this could delete audit trail data

**Recommendations:**
- Verify UTXO existence against block headers where possible (SPV verification)
- Encrypt mnemonic at rest using a user-provided passphrase (or OS keychain)
- Lock down CORS for production builds
- Add rate limiting to sensitive endpoints (createAction, signAction)
- Consider extending the spent UTXO retention period or keeping a separate audit log

### 4. Dependability (Never Lose Tokens)

**Current Score: 7/10**

**Strengths:**
- Crash recovery at startup (restore pending-* placeholders, clean stale txs)
- Periodic cleanup catches mid-flight failures
- Failed broadcast cleanup restores input UTXOs and deletes ghost outputs
- Serialization lock prevents conflicting transaction chains
- Transaction + UTXO state changes are consistent (same thread holds DB lock)

**Weaknesses:**
- **No UnFail mechanism** (Bug 1) — a prematurely failed tx could cause token loss
- **No reorg handling** — a confirmed tx that gets orphaned could lead to spending non-existent UTXOs
- Post-signing state updates not wrapped in SQLite transaction (Bug 8)
- Signed tx only cached in parent_transactions if all inputs are signed — partially signed txs lose their raw data
- `PENDING_TRANSACTIONS` is an in-memory HashMap — lost on restart (by design, but limits crash recovery for active transactions)

**Recommendations:**
- Implement UnFail: before cleaning up failed transactions, re-check ARC/WhatsOnChain
- Wrap post-signing state updates in SQLite transactions
- Add a `failed_at` timestamp to enable delayed cleanup (e.g., don't clean up failed txs for 30 minutes, giving time for re-verification)
- Store pending transaction data in database instead of in-memory HashMap (for crash recovery during active operations)

---

## Research Findings

### 1. ARC Status Polling — RESOLVED

**Finding**: Implemented in `arc_status_poller.rs`. Polls every 60 seconds, handles MINED (→confirmed + cache proof), REJECTED/DOUBLE_SPEND (→failed). Max 20 txs per batch with 200ms rate limiting.

**Comparison to wallet-toolbox**: wallet-toolbox's TaskCheckForProofs runs every 2 hours by default BUT is triggered immediately on new block events via Chaintracks. Our polling is more frequent (60s vs 2h) which compensates for not having block-event triggers.

**Assessment**: Adequate. The 60-second polling provides faster confirmation detection than the wallet-toolbox's block-triggered approach in practice. No changes needed.

### 2. Abandoned Transaction Detection — LARGELY RESOLVED

**Finding**: Continuous detection now runs via `cleanup_failed_utxos()` every 5 minutes during UTXO sync. Also runs at startup. Three-tier timeout: pending-* reservations after 5 min, unproven UTXOs after 1 hour, pending broadcast_status transactions after 15 min.

**Comparison to wallet-toolbox**: TaskFailAbandoned runs every 8 minutes (5-minute threshold). Our approach is similar (5-minute interval, 5-minute threshold).

**Assessment**: Adequate. The timeouts are reasonable. Consider making the 15-minute pending-broadcast threshold configurable.

### 3. Chain Reconciliation — STILL NEEDED

**Finding**: Current sync only adds new UTXOs; never removes disappeared ones. The `upsert_utxos()` function explicitly preserves local `is_spent` state, which is correct for preventing premature marking but means externally-spent UTXOs accumulate.

**wallet-toolbox approach**: TaskReviewStatus delegates to `storage.reviewStatus()` which propagates transaction status to outputs. This handles the case where a transaction status changes and its outputs need updating, but doesn't directly address external spending detection.

**Recommended approach**: During UTXO sync, after fetching the API's unspent set for each address, compare against local unspent UTXOs. For any local UTXO NOT in the API response:
1. If `last_updated` < now - 2 sync cycles: flag as "possibly spent externally"
2. On the 3rd consecutive miss: mark as spent with `spent_txid = 'external-reconciliation'`
3. This is conservative (requires 3 consecutive sync cycles = 15 minutes of absence) to avoid false positives from API lag

### 4. Failed Transaction Recovery — STILL NEEDED (Priority: High)

**Finding**: No equivalent to wallet-toolbox's TaskUnFail. Failed transactions are immediately cleaned up (ghost UTXOs deleted, inputs restored) with no re-verification window.

**wallet-toolbox approach**: TaskUnFail runs every 10 minutes:
1. Queries for transactions with `unfail` status
2. Attempts to retrieve a merkle path via `getMerklePath()`
3. If found: transitions to `unmined` and resets attempt counter
4. If not found: transitions to `invalid`
5. Reconciles input/output spendability with blockchain state via `isUtxo()`

**Recommended approach for Hodos**:
1. Add a `failed_at` timestamp column to transactions table (or reuse existing `timestamp`)
2. Before cleanup deletes ghost UTXOs, check if `failed_at > now - 30 min`
3. If recently failed: query WhatsOnChain `GET /v1/bsv/main/tx/{txid}` to check if tx exists on-chain
4. If found on-chain: recover the transaction (mark as `confirmed`, keep UTXOs as-is)
5. If not found after 30 minutes: proceed with normal cleanup

### 5. Block Reorg Handling — DEFERRED (Low Priority)

**Finding**: BSV reorgs are extremely rare. The wallet-toolbox's TaskReorg handles deactivated headers with a 10-minute aging period and 3 retries for reproof. The core logic is in `storage.reproveHeader()` which re-validates merkle proofs against new chain state.

**Assessment**: For a desktop wallet, the risk is negligible. If a reorg does occur, the wallet's UTXO sync will eventually pick up the correct state within a few sync cycles. The worst case is a brief period of incorrect balance display. No implementation needed at this time.

### 6. ARC Callbacks — DEFERRED (Not Needed)

**Finding**: Desktop wallets cannot receive webhooks. Polling is the correct approach for our architecture. The 60-second ARC polling provides adequate confirmation tracking.

**Assessment**: No implementation needed. Polling-only is the right choice.

---

## Recommendations

### Priority 1: Implement UnFail Mechanism (Dependability)

**What**: Before permanently cleaning up a failed transaction, delay cleanup and re-verify with blockchain.

**How**:
1. When a transaction is marked `failed`, record `failed_at` timestamp
2. In `cleanup_failed_utxos()`, skip transactions with `failed_at > now - 30 minutes`
3. Add a separate cleanup step that queries WhatsOnChain for transactions failed >5 minutes but <30 minutes
4. If the tx is found on-chain: recover it (change broadcast_status to `confirmed`, keep UTXOs)
5. After 30 minutes with no on-chain presence: proceed with normal cleanup (delete ghost UTXOs, restore inputs)

**Impact**: Prevents the most dangerous failure mode: permanent fund loss from prematurely marked-failed transactions.

### Priority 2: Chain Reconciliation for Disappeared UTXOs (Accuracy)

**What**: Detect UTXOs that exist locally as unspent but are no longer in the blockchain's unspent set.

**How**:
1. After `upsert_utxos()` in the UTXO sync, compare the API's unspent set against local unspent UTXOs for each address
2. Track "consecutive misses" (add `sync_miss_count` field to UTXOs or use a temporary in-memory map)
3. After 3 consecutive misses (15 minutes): mark UTXO as spent with `spent_txid = 'external-reconciliation'`
4. Log the event for debugging

**Impact**: Prevents balance inflation from externally-spent UTXOs. Important for seed-sharing scenarios.

### Priority 3: Balance Cache Invalidation After Sync (Accuracy)

**What**: Invalidate the balance cache after UTXO sync completes, so new incoming UTXOs are reflected immediately.

**How**:
1. Pass the `BalanceCache` (or `AppState`) to the UTXO sync service
2. After `sync_utxos()` completes, call `balance_cache.invalidate()`
3. Also invalidate after `cleanup_failed_utxos()` restores any UTXOs

**Impact**: Ensures balance updates within one sync cycle of new UTXOs appearing on-chain.

### Priority 4: SQLite Transaction Wrapping for State Updates (Dependability)

**What**: Wrap the post-signing state updates in signAction in a SQLite transaction.

**How**:
1. Use `conn.execute("BEGIN", [])` before the state update block
2. Perform all updates (txid, UTXOs, status, broadcast_status, spent_txid, cache)
3. Use `conn.execute("COMMIT", [])` at the end
4. On error: `conn.execute("ROLLBACK", [])` and return error

**Impact**: Makes the post-signing state transition atomic. Prevents inconsistent state from mid-sequence crashes.

### Priority 5: Remove Dead Code (`utxo_selection_lock`) (Maintenance)

**What**: Remove the unused `utxo_selection_lock` from AppState.

**How**: Delete the field from AppState struct and its initialization in main.rs.

**Impact**: Reduces confusion for future developers.

### Priority 6: Privacy Improvements for UTXO Sync (Privacy)

**What**: Reduce address correlation leakage to WhatsOnChain.

**How** (incremental):
1. Randomize the order of address queries in each sync cycle
2. Add random delays between address queries (100-500ms)
3. Investigate WhatsOnChain batch API or consider JungleBus for event-based monitoring
4. Add optional SOCKS5 proxy configuration

**Impact**: Makes it harder for WhatsOnChain to correlate addresses belonging to the same wallet.

---

## Implementation Priority

| # | Item | Pillar | Severity | Effort |
|---|------|--------|----------|--------|
| 1 | UnFail mechanism | Dependability | Medium-High | Medium |
| 2 | Chain reconciliation (disappeared UTXOs) | Accuracy | Medium | Medium |
| 3 | Balance cache invalidation after sync | Accuracy | Low | Low |
| 4 | SQLite transaction wrapping | Dependability | Low | Low |
| 5 | Remove dead code (utxo_selection_lock) | Maintenance | None | Trivial |
| 6 | Privacy improvements for sync | Privacy | Low | Medium |

**Note**: Items 3, 4, and 5 are quick wins that can be done immediately. Items 1 and 2 require design decisions and should be planned.

---

## Success Criteria

- Zero permanent UTXO/token loss from any failure mode (crash, timeout, rejection, reorg)
- Balance accuracy within 1 sync cycle (5 minutes) of on-chain state changes
- No stale pending transactions surviving more than 15 minutes without resolution
- Failed transactions re-verified before permanent cleanup (30-minute window)
- Meet or exceed wallet-toolbox's state management capabilities for desktop wallet use case
- Privacy: address correlation minimized through query randomization
- Security: critical state transitions are atomic (SQLite transactions)
- Efficiency: minimize API calls while maintaining accuracy targets

---

## Appendix: wallet-toolbox Monitor Tasks Reference

| Task | Interval | Purpose |
|------|----------|---------|
| TaskSendWaiting | 8 sec | Retry broadcast for unsent transactions |
| TaskCheckForProofs | 2 hr (or on new block) | Retrieve merkle proofs for broadcast transactions |
| TaskFailAbandoned | 8 min | Fail transactions stuck in unprocessed/unsigned |
| TaskUnFail | 10 min | Re-check failed transactions for actual mining |
| TaskReviewStatus | 5 min | Propagate transaction status to outputs |
| TaskReorg | On deactivated header | Reprove headers after chain reorganization |
| TaskNewHeader | On new block | Trigger proof checking and sync |
| TaskClock | Configurable | Periodic timer for general health checks |
| TaskPurge | Configurable | Clean up old data |
| TaskSyncWhenIdle | On idle | Background sync during low-activity periods |
| TaskCheckNoSends | Configurable | Verify no-send transactions are properly handled |
| TaskMonitorCallHistory | Configurable | Log monitoring call history for debugging |

**ActionStatus values**: `completed`, `unprocessed`, `sending`, `unproven`, `unsigned`, `nosend`, `nonfinal`, `failed`

**ProvenTxReqStatus values**: `unknown`, `sending`, `unmined`, `unconfirmed`, `completed`, `callback`, `doubleSpend`, `unfail`, `invalid`, `notified`
