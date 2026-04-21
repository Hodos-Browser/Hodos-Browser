# Create Action Lock Restructure — Implementation Plan

## Problem

`create_action` holds the DB mutex for its entire execution (UTXO selection → signing → BEEF building → broadcast). Any network call during this window (parent tx fetch, block header lookup, broadcast) blocks ALL other wallet operations — balance checks, address generation, sync polling, monitor tasks.

**User impact:** UI freezes for 500ms–10s during sends. Black screen in wallet overlay. Balance and sync status stop updating.

**Reference:** Performance audit issue #56, finding 1.2 (CRITICAL).

---

## Current Architecture

```
create_action_lock.lock()           ← held for ENTIRE handler
│
├── DB lock #1: read wallet/addresses (lines 3819-3852)
├── DB lock #2: read spendable UTXOs (lines 3858-3902)
├── utxo_selection_lock.lock()      ← nested inside create_action_lock
│   ├── DB lock #3: re-read UTXOs (lines 3973-4004)
│   └── DB lock #4: mark_multiple_spent with placeholder (lines 4048-4067)
│   utxo_selection_lock.unlock()
│
├── Build transaction (CPU, no lock needed)
├── Sign transaction (CPU, no lock needed)
├── Build BEEF ancestry (NETWORK — may call WoC API)  ← BLOCKS EVERYTHING
├── DB lock #5: save transaction record
├── DB lock #6: update_spending_description (placeholder → real txid)
├── Broadcast to ARC (NETWORK)                        ← BLOCKS EVERYTHING
├── DB lock #7: update status
└── DB lock #8: broadcast failure cleanup (if needed)
create_action_lock.unlock()
```

**Total DB lock acquisitions:** 8 separate locks within one handler call.
**Total time under create_action_lock:** 500ms–10s (network dependent).

---

## Proposed Architecture: Reserve → Release → Build → Broadcast

```
Phase 1: RESERVE (short DB lock)
├── DB lock: read wallet, addresses, spendable UTXOs
├── Select UTXOs (coin selection algorithm)
├── mark_multiple_spent(placeholder) — UTXOs now spendable=0
├── Insert preliminary transaction record (status='unsigned')
└── DB unlock
    ← Other requests can proceed. Reserved UTXOs won't be selected.

Phase 2: BUILD (no lock, CPU only)
├── Build transaction from reserved UTXOs
├── Sign inputs
└── Compute final txid

Phase 3: BEEF (no lock, network)
├── Build BEEF ancestry chain
├── Fetch any missing parent txs from WoC (cached most of the time)
└── Assemble BEEF bytes

Phase 4: PERSIST (short DB lock)
├── DB lock: save signed raw_tx, update txid, record change output
├── update_spending_description(placeholder → real_txid)
└── DB unlock

Phase 5: BROADCAST (no lock, network)
├── Send BEEF to ARC
├── DB lock (brief): update status to 'unproven' or handle failure
└── On failure: restore reserved UTXOs (cleanup)
```

**Key change:** Network calls (BEEF ancestry, broadcast) happen OUTSIDE any lock. The DB is only locked for short read/write bursts.

---

## Why This Is Safe (No Race Conditions)

The reserve pattern (`mark_multiple_spent` with placeholder) already exists and prevents races:

1. **Phase 1** atomically selects AND reserves UTXOs (`spendable=0`).
2. Any concurrent `create_action` won't select the same UTXOs because they query `WHERE spendable=1`.
3. If the handler crashes between Phase 1 and Phase 5, `TaskFailAbandoned` (monitor task, 5-minute interval) finds the `unsigned` transaction and restores the reserved UTXOs.
4. If broadcast fails in Phase 5, explicit cleanup restores UTXOs immediately.

**This is the same pattern monitor tasks already use** (short lock → network call → short lock). We're just applying it to `create_action`.

---

## Implementation Steps

### Step 1: Extract UTXO Selection into Atomic Reserve Function

Create a new function that does the minimal DB work:

```rust
/// Atomically select and reserve UTXOs for a transaction.
/// Returns selected UTXOs and a placeholder ID for later cleanup.
///
/// After this returns, the UTXOs are spendable=0 and safe from
/// concurrent selection. The DB lock is NOT held on return.
fn reserve_utxos_for_action(
    state: &AppState,
    required_satoshis: i64,
    required_outputs: &[RequestedOutput],  // user-specified inputs
    user_id: i64,
) -> Result<ReservedUtxos, Error> {
    let db = state.database.lock().unwrap();
    let output_repo = OutputRepository::new(db.connection());

    // 1. Read spendable UTXOs
    let available = output_repo.get_spendable_by_user(user_id)?;

    // 2. Run coin selection
    let selected = select_optimal_utxos(&available, required_satoshis, required_outputs)?;

    // 3. Reserve with placeholder
    let placeholder = format!("pending-{}", chrono::Utc::now().timestamp_millis());
    let utxo_refs: Vec<(String, u32)> = selected.iter()
        .map(|u| (u.txid.clone(), u.vout as u32))
        .collect();
    output_repo.mark_multiple_spent(&utxo_refs, &placeholder)?;

    // 4. Create preliminary transaction record
    let tx_repo = TransactionRepository::new(db.connection());
    // Insert with status='unsigned' so TaskFailAbandoned can clean up if we crash

    state.balance_cache.invalidate();

    Ok(ReservedUtxos {
        outputs: selected,
        placeholder,
        total_satoshis: selected.iter().map(|u| u.satoshis).sum(),
    })
    // DB lock dropped here
}
```

**Files to modify:** `handlers.rs` (new function)

### Step 2: Extract Build & Sign into Lock-Free Function

```rust
/// Build and sign a transaction from reserved UTXOs.
/// No DB lock needed — all data passed in.
fn build_and_sign_transaction(
    reserved: &ReservedUtxos,
    outputs: &[TransactionOutput],
    change_address: &str,
    // ... signing keys passed in, not fetched from DB
) -> Result<SignedTransaction, Error> {
    // Pure computation — no DB, no network
}
```

**Files to modify:** `handlers.rs` (extract from existing code)

### Step 3: Make BEEF Building Lock-Free

BEEF ancestry building already works with passed-in data. The only issue is `enhance_tsc_with_height` (line 6426) which fetches block headers on cache miss. This needs to happen outside the DB lock.

```rust
/// Build BEEF for a signed transaction.
/// May make network calls (WoC) for uncached parent txs/headers.
/// No DB lock held during network calls.
async fn build_beef_outside_lock(
    state: &AppState,
    client: &reqwest::Client,
    txid: &str,
    raw_tx: &[u8],
) -> Result<Vec<u8>, Error> {
    // Read parent tx data (short DB lock)
    let parent_data = {
        let db = state.database.lock().unwrap();
        // read parent_transactions, proven_txs
    }; // lock dropped

    // Build BEEF (may hit network for missing parents)
    build_beef_from_data(parent_data, txid, raw_tx).await
}
```

**Files to modify:** `handlers.rs`, `beef_helpers.rs` (refactor `build_beef_for_txid`)

### Step 4: Persist Results with Short Lock

```rust
/// Save the signed transaction and update UTXO state.
/// Short DB lock — no network calls.
fn persist_signed_transaction(
    state: &AppState,
    reserved: &ReservedUtxos,
    signed_tx: &SignedTransaction,
    beef_bytes: &[u8],
) -> Result<i64, Error> {
    let db = state.database.lock().unwrap();
    let tx_repo = TransactionRepository::new(db.connection());
    let output_repo = OutputRepository::new(db.connection());

    // Update transaction record (unsigned → sending)
    // Insert change output
    // update_spending_description(placeholder → real_txid)
    // link_outputs_to_transaction

    state.balance_cache.invalidate();
    Ok(transaction_id)
    // DB lock dropped
}
```

**Files to modify:** `handlers.rs` (new function)

### Step 5: Broadcast with Cleanup

```rust
/// Broadcast and handle success/failure.
/// Network call happens outside lock. Brief lock for status update.
async fn broadcast_and_finalize(
    state: &AppState,
    beef_bytes: &[u8],
    txid: &str,
    reserved: &ReservedUtxos,
) -> Result<BroadcastResult, Error> {
    // No lock during broadcast
    match broadcast_to_arc(beef_bytes).await {
        Ok(result) => {
            // Brief lock: update status to 'unproven'
            let db = state.database.lock().unwrap();
            // ...
            Ok(result)
        }
        Err(e) => {
            // Brief lock: cleanup (existing pattern)
            let db = state.database.lock().unwrap();
            // delete ghost outputs
            // restore_by_spending_description(placeholder)
            // restore_spent_by_txid(txid)
            state.balance_cache.invalidate();
            Err(e)
        }
    }
}
```

**Files to modify:** `handlers.rs` (extract from existing broadcast code)

### Step 6: Remove utxo_selection_lock

Once the reserve pattern properly prevents concurrent selection, the separate `utxo_selection_lock` is redundant. The `create_action_lock` can also be removed or downgraded to a lighter semaphore if we want to allow limited concurrency.

**Decision point:** Keep `create_action_lock` as a safety net initially. Remove in a follow-up after testing confirms no races.

**Files to modify:** `handlers.rs` (remove lock), `main.rs` (remove from AppState)

---

## Cleanup / Crash Recovery

Already handled by existing monitor tasks — no changes needed:

| Scenario | Recovery |
|----------|----------|
| Crash after Phase 1 (reserved, not signed) | `TaskFailAbandoned` finds `unsigned` tx after 5 min, restores UTXOs |
| Crash after Phase 4 (signed, not broadcast) | `TaskSendWaiting` finds `sending` tx after 2 min, re-broadcasts or cleans up |
| Broadcast failure | Immediate cleanup in Phase 5 error handler (existing code) |
| Broadcast success, no proof | `TaskCheckForProofs` acquires merkle proof (existing, 60s interval) |

---

## Files Modified

| File | Changes |
|------|---------|
| `rust-wallet/src/handlers.rs` | Restructure `create_action_internal` into 5 phases. Extract 4 new functions. |
| `rust-wallet/src/beef_helpers.rs` | Refactor `build_beef_for_txid` to accept pre-fetched data (avoid DB lock during network) |
| `rust-wallet/src/main.rs` | Potentially remove `utxo_selection_lock` from `AppState` (Step 6) |
| `rust-wallet/src/database/output_repo.rs` | No changes — existing reserve/restore functions are sufficient |

---

## Testing Plan

### Unit Tests
- Reserve UTXOs → verify spendable=0 and placeholder set
- Concurrent reserve calls → verify no overlap in selected UTXOs
- Restore on failure → verify spendable=1 restored
- Placeholder → real txid update works correctly

### Integration Tests
- Send transaction while balance is being polled → no hang
- Send two transactions rapidly → second waits for first reserve, no double-spend
- Kill wallet mid-transaction → restart → TaskFailAbandoned restores UTXOs
- Broadcast failure → UTXOs restored → retry succeeds with same inputs

### Manual Tests
- Open wallet panel, send BSV → no black screen or UI freeze
- Check balance updates during send (should not stall)
- Monitor tasks continue running during long BEEF builds

---

## Risk Assessment

| Risk | Mitigation |
|------|-----------|
| TOCTOU: data changes between Phase 1 and Phase 4 | Phase 1 reserves UTXOs atomically. No other request can spend them. |
| Crash between reserve and cleanup | TaskFailAbandoned (5-min interval) restores unsigned reservations |
| Double-spend from concurrent create_action | Reserve pattern makes UTXOs invisible to concurrent selectors |
| Signing key unavailable after lock drop | Read signing keys in Phase 1 (same lock scope as UTXO selection) |
| BEEF build fails after reserve | Same as broadcast failure — restore reserved UTXOs |

---

## Estimated Effort

| Step | Effort | Risk |
|------|--------|------|
| Step 1: Extract reserve function | Small | Low — same logic, just extracted |
| Step 2: Extract build/sign | Small | Low — pure computation, no state |
| Step 3: Lock-free BEEF | Medium | Low — refactor data passing |
| Step 4: Persist function | Small | Low — same writes, just grouped |
| Step 5: Broadcast cleanup | Small | Low — existing pattern |
| Step 6: Remove utxo_selection_lock | Small | Medium — needs confidence from testing |
| **Total** | **~1 day coding + 1 day testing** | |

---

## Relationship to Other Issues

- **#56 Finding 1.4 (UTXO selection timeout):** Adding `tokio::time::timeout` to the lock is a quick independent fix. Do it separately — it's one line and provides immediate safety.
- **#56 Finding 3.4 (mutex poisoning):** Orthogonal. Can be addressed independently with `parking_lot::Mutex`.
- **Self-heal system:** Independent. The self-heal investigates after miner rejection; this restructure prevents UI freezes during normal operation.
- **Send transaction black screen (#48 area):** This restructure is the proper fix. The C++ async IPC conversion is the other half (moving `send_transaction` off the CEF UI thread).
