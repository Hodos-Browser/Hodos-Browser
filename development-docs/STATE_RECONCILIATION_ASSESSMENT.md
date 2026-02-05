# State Reconciliation Assessment and Upgrade Plan

**Created**: 2026-01-30
**Status**: Assessment — to be revisited after basket sprint
**Objective**: Assess and upgrade UTXO/transaction state management to meet or exceed industry standards on accuracy, privacy, security, and efficiency.

---

## Current State

### What We Have
- SQLite-based UTXO tracking with `is_spent`, `spent_txid`, `spent_at` fields
- Transaction table with `broadcast_status` (pending, broadcast, confirmed, failed)
- Background UTXO sync (5-min interval) fetching from WhatsOnChain API
- Startup cleanup of stale pending transactions (>5 min)
- Startup cleanup of placeholder-reserved UTXOs (`pending-*` pattern)
- Broadcast failure rollback (delete ghost change UTXOs, restore input UTXOs)
- Post-signing txid reconciliation (updates DB records from unsigned to signed txid)
- ARC broadcasting with BEEF format

### Known Gaps (vs. wallet-toolbox)
| Area | Our Implementation | wallet-toolbox | Gap |
|------|-------------------|---------------|-----|
| Transaction states | 4 states (pending/broadcast/confirmed/failed) | 10+ granular states | Significant |
| ARC status polling | Not implemented | TaskCheckForProofs polls after blocks | Significant |
| ARC callbacks | Not implemented | X-CallbackUrl webhooks | Moderate (hard for desktop) |
| Abandoned tx detection | Time-based (startup only) | TaskFailAbandoned (continuous) | Moderate |
| Chain reconciliation | Sync adds new UTXOs, never removes | TaskReviewStatus reconciles | Significant |
| Failed tx recovery | Deletes immediately | TaskUnFail re-checks if actually mined | Significant |
| Reorg handling | None | TaskReorg updates proofs | Significant |
| Block-height triggers | None (fixed intervals only) | New block detection | Moderate |

---

## Research Items

### 1. ARC Status Polling (Priority: High)
**Research**: How does ARC's `GET /v1/tx/{txid}` status endpoint work? What are the rate limits? How do we efficiently poll for multiple transactions?

**References**:
- ARC API: `GET /v1/tx/{txid}` returns `{txStatus, blockHash, blockHeight, merklePath}`
- ARC status codes: QUEUED(1), RECEIVED(2), STORED(3), ANNOUNCED(4), REQUESTED(5), SENT(6), ACCEPTED(7), SEEN_ON_NETWORK(8), MINED(108), REJECTED(-1), DOUBLE_SPEND(-3)
- We have `rust-wallet/src/arc_status_poller.rs` — assess its current state and what needs to be added
- wallet-toolbox: `TaskCheckForProofs` implementation

**Goal**: After broadcasting, poll ARC periodically. Transition pending->confirmed when MINED. Transition pending->failed when REJECTED or timeout exceeded.

### 2. Abandoned Transaction Detection (Priority: High)
**Research**: What is the right timeout threshold? How does the wallet-toolbox's TaskFailAbandoned work? Should we use block height or wall-clock time?

**References**:
- BSV blocks average ~10 minutes
- wallet-toolbox uses configurable timeout thresholds
- ARC keeps transactions in mempool for a limited time
- Research: What is ARC's mempool expiry policy?

**Goal**: Continuously detect and fail transactions that have been pending beyond a reasonable threshold. Release their reserved inputs back to spendable.

### 3. Chain Reconciliation — Detecting "Actually Spent" UTXOs (Priority: Medium)
**Research**: How to efficiently detect UTXOs that are marked unspent locally but spent on-chain? The current sync fetches the unspent set but doesn't compare against local records.

**Options to investigate**:
- Compare local unspent set against WhatsOnChain's unspent set per address (expensive)
- Use ARC double-spend detection (DOUBLE_SPEND_ATTEMPTED status code)
- Detect on spend failure (ARC returns REJECTED with "missing inputs")
- Periodic full reconciliation sweep (low frequency, high cost)

**References**:
- WhatsOnChain API: `GET /v1/bsv/main/address/{address}/unspent`
- wallet-toolbox: TaskReviewStatus
- Research: Does WhatsOnChain have a webhook/SSE for address activity?

**Goal**: Detect UTXOs that disappeared from the chain and mark them spent locally.

### 4. Failed Transaction Recovery (Priority: Medium)
**Research**: What if we marked a transaction as failed but it actually made it to the chain? The wallet-toolbox has TaskUnFail for this.

**Options to investigate**:
- After marking failed, poll ARC one more time after a delay
- Check WhatsOnChain for the txid before final cleanup
- Keep failed transactions in a "review" state before permanent cleanup

**References**:
- wallet-toolbox: TaskUnFail implementation
- ARC: Transaction may propagate despite initial error response

**Goal**: Never permanently lose money by prematurely marking a broadcast transaction as failed.

### 5. Block Reorg Handling (Priority: Low)
**Research**: How does the wallet-toolbox handle chain reorganizations? What happens to confirmed transactions that get orphaned?

**References**:
- wallet-toolbox: TaskReorg
- BSV reorgs are rare but possible
- Research: How deep do we need to track confirmations for safety?

**Goal**: Handle the edge case where a confirmed transaction becomes unconfirmed due to reorg.

### 6. ARC Callbacks for Desktop Wallet (Priority: Low)
**Research**: ARC supports `X-CallbackUrl` webhooks, but desktop wallets don't have public URLs. Options:

**Options to investigate**:
- Polling-only approach (simpler, our current direction)
- Local tunnel (ngrok-style) for development/testing
- WebSocket connection to a relay service
- Is there a BSV infrastructure service that proxies ARC callbacks?

**References**:
- ARC Callbacker microservice documentation
- wallet-toolbox: callback vs polling dual approach

**Goal**: Determine if callbacks are worth implementing for a desktop wallet or if polling is sufficient.

---

## Periodic Sync Assessment

**Current implementation**: `rust-wallet/src/utxo_sync.rs`
- Runs every 5 minutes
- Fetches unspent UTXOs from WhatsOnChain for all tracked addresses
- Inserts new UTXOs, updates timestamps on existing ones
- Does NOT override local `is_spent` flags (by design — comment at utxo_repo.rs:39)
- Cleans up failed UTXOs (status='failed'), marks unproven > 1 hour as failed

**Assessment objectives**:
1. **Accuracy**: Can the sync detect and correct all categories of state drift?
   - New UTXOs appearing on-chain: YES (inserts new)
   - UTXOs disappearing from chain (spent externally): NO
   - UTXOs stuck as spent locally (failed broadcast): Partially (startup cleanup only)
2. **Privacy**: Does the sync leak information?
   - Currently queries WhatsOnChain for every address — consider batching or using our own node
   - Research: What address data does WhatsOnChain log? Privacy implications?
3. **Security**: Can the sync be exploited?
   - API responses are not cryptographically verified (MITM risk)
   - Research: Should we verify UTXOs against block headers?
4. **Efficiency**: Is the sync optimal?
   - 5-minute interval may be too frequent or too infrequent depending on usage
   - Fetches ALL addresses every cycle — consider only checking recently-used addresses
   - Research: Can we use address activity notifications instead of polling?

---

## Implementation Priority Order

1. **ARC Status Polling** — highest impact, directly addresses "is the miner seeing our transaction?" question
2. **Abandoned Transaction Detection** — prevents permanent UTXO loss from stuck pending state
3. **Chain Reconciliation** — detects externally-spent UTXOs
4. **Failed Transaction Recovery** — safety net for premature failure marking
5. **Block Reorg Handling** — edge case, low priority
6. **ARC Callbacks** — nice-to-have, polling may be sufficient

---

## Success Criteria

- Zero permanent UTXO loss from any failure mode (crash, timeout, rejection, reorg)
- Balance accuracy within 1 block confirmation lag
- No stale pending transactions surviving more than 2x the expected confirmation time
- Meet or exceed wallet-toolbox's state management capabilities
- Privacy: minimize address leakage to third-party APIs
- Security: verify critical state transitions against cryptographic proofs where possible
- Efficiency: minimize API calls while maintaining accuracy targets
