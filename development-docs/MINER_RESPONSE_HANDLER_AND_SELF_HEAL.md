# Miner Response Handler & Self-Heal System

## Overview

This document outlines a comprehensive plan for handling all possible miner responses and implementing a self-healing mechanism that uses the blockchain as the source of truth to correct local database inconsistencies.

**Goals:**
1. Categorize and handle every possible miner response appropriately
2. Implement intelligent retry with alternative inputs when transactions fail
3. Self-heal local state by reconciling with on-chain truth
4. Prevent user-facing errors from transient network issues

---

## Part 1: Complete Miner Response Taxonomy

### 1.1 ARC (Authoritative Response Component) Statuses

ARC is our primary broadcast endpoint. Here are ALL possible status responses:

| Status | Category | Meaning | Current Handling | Proposed Handling |
|--------|----------|---------|------------------|-------------------|
| `QUEUED` | Pending | Transaction queued for processing | None | Log, wait |
| `RECEIVED` | Pending | Properly received by processor | None | Log, wait |
| `STORED` | Pending | Stored for retry if not immediately mined | Wait in mempool | Same |
| `ANNOUNCED_TO_NETWORK` | Pending | Announced to network nodes | Wait in mempool | Same |
| `REQUESTED_BY_NETWORK` | Pending | Requested by network nodes | Wait in mempool | Same |
| `SENT_TO_NETWORK` | Pending | Sent to network peers | Wait in mempool | Same |
| `ACCEPTED_BY_NETWORK` | Pending | Accepted by peers | Wait in mempool | Same |
| `SEEN_ON_NETWORK` | Pending | Propagated via INV messages | Wait in mempool | Same |
| `MINED` | Success | Transaction mined into a block | Mark confirmed | Same + fetch proof |
| `CONFIRMED` | Success | 100+ confirmations | N/A | Mark deep-confirmed |
| `SEEN_IN_ORPHAN_MEMPOOL` | Warning | Parent tx missing from ARC's view | Retry, then fail after 30m | **SELF-HEAL: Check if input spent elsewhere** |
| `MINED_IN_STALE_BLOCK` | Warning | Mined but block orphaned | N/A | Re-broadcast, check for double-spend |
| `DOUBLE_SPEND_ATTEMPTED` | Failure | Conflicting transaction detected | Mark failed | **SELF-HEAL: Input spent elsewhere** |
| `REJECTED` | Failure | Rejected by network | Mark failed | **SELF-HEAL: Analyze reason, fix DB** |
| `UNKNOWN` | Error | No processing occurred | N/A | Retry with backoff |

### 1.2 Script/Validation Errors (From Any Broadcaster)

These are returned in error messages, not status codes:

| Error Pattern | Category | Meaning | Current Handling | Proposed Handling |
|---------------|----------|---------|------------------|-------------------|
| `ERROR: 16: mandatory-script-verify-flag-failed` | Fatal | Invalid signature/script | Mark failed | **SELF-HEAL: Key derivation mismatch?** |
| `missing inputs` / `missingorspent` | Fatal | Input UTXO doesn't exist | Mark failed | **SELF-HEAL: Mark input as spent** |
| `txn-mempool-conflict` | Fatal | Double-spend in mempool | Mark failed | **SELF-HEAL: Check which tx won** |
| `double spend` / `double-spend` | Fatal | Input already spent on-chain | Mark failed | **SELF-HEAL: Mark input as spent** |
| `dust` | Fatal | Output below dust threshold | Mark failed | Adjust output amount |
| `tx-size` | Fatal | Transaction too large | Mark failed | Split into multiple txs |
| `non-mandatory-script-verify` | Fatal | Policy rejection | Mark failed | Check script compliance |
| `bad-txns-inputs-missingorspent` | Fatal | Bitcoin Core format | Mark failed | **SELF-HEAL: Mark input as spent** |
| `insufficient fee` / `min relay fee not met` | Recoverable | Fee too low | Mark failed | **RETRY: Rebuild with higher fee** |
| `mempool full` | Transient | Node mempool at capacity | Retry | Retry with exponential backoff |
| `timeout` / `connection refused` | Transient | Network issue | Retry | Retry with exponential backoff |

### 1.3 HTTP Status Codes

| Code | Meaning | Current Handling | Proposed Handling |
|------|---------|------------------|-------------------|
| 200 | Success | Process response | Same |
| 400 | Bad request (invalid tx) | Fatal error | Parse body for specific error |
| 401 | Unauthorized | Retry other endpoint | Add auth headers if required |
| 404 | Transaction not found | Check WoC fallback | Same |
| 409 | Conflict (double-spend) | Fatal error | **SELF-HEAL: Identify conflicting tx** |
| 429 | Rate limited | Retry | Exponential backoff, respect Retry-After |
| 500 | Server error | Retry | Retry with backoff |
| 503 | Service unavailable | Retry | Retry with backoff, try alt endpoint |

---

## Part 2: Self-Heal System Design

### 2.1 Core Principle: Blockchain is Source of Truth

When a miner tells us something is wrong with our transaction, we should:
1. **NOT** simply mark as failed and move on
2. **INSTEAD** investigate WHY it failed
3. **RECONCILE** our database with on-chain reality
4. **RETRY** with corrected state (if applicable)

### 2.2 Self-Heal Triggers (All Reactive - After Miner Rejection)

| Miner Response | What It Means | Investigation Action |
|----------------|---------------|---------------------|
| `missing inputs` | UTXO miner can't find | Query chain: is it spent? By whom? Is it our tx or external? |
| `double-spend` | Conflicting tx in mempool | Find the conflicting tx. Is it ours (race condition) or external? |
| `SEEN_IN_ORPHAN_MEMPOOL` | Parent tx unknown to ARC | Is parent just slow to propagate, or is input actually spent? |
| Script verification failed | Signature invalid | Check: did we derive the right key? Is the UTXO ours? |
| Transaction not found (after timeout) | Never mined | Check: are inputs still valid, or did something else spend them? |

> **Key principle**: These investigations only happen AFTER a miner rejects. We never query chain state proactively or before building a transaction.

### 2.3 Self-Heal Implementation Plan

> ⚠️ **CRITICAL: Reactive Only, Never Proactive**
>
> We do NOT validate UTXOs before broadcast or periodically. This is dangerous for token wallets because:
> - **Push/drops**: Token UTXOs can be "spent" to create new outputs you still control. WoC sees the original as spent, but the token moved to a new outpoint you own.
> - **BRC-42 derived addresses**: The addresses/scripts may not match what WoC's simple UTXO API expects.
> - **Token semantics**: Raw UTXO APIs don't understand token transfers - GorillaPool/1sat-stack is the source of truth for tokens.
>
> **The ONLY safe time to investigate is AFTER a miner rejects a transaction.** The miner is the authority on current mempool/chain state.

#### Core Flow: Trust DB → Broadcast → Investigate on Rejection

```
1. Trust our DB → build transaction
2. Broadcast to miner
3. IF miner accepts → done
4. IF miner rejects → INVESTIGATE (don't blindly mark as bad)
5. Understand WHY it failed
6. Reconcile based on findings
7. THEN decide: retry with different inputs, or alert user
```

#### Post-Failure Investigation (Not "Mark as Bad")

The miner error is a **signal to investigate**, not an instruction to blindly update state.

```rust
/// Investigate why a broadcast failed - understand before taking action
async fn investigate_broadcast_failure(
    txid: &str, 
    error: &str, 
    inputs: &[OutPoint]
) -> InvestigationResult {
    let error_lower = error.to_lowercase();
    let mut findings = Vec::new();
    
    // === STEP 1: Classify the error ===
    let error_type = classify_miner_error(&error_lower);
    
    // === STEP 2: Investigate each input mentioned ===
    if matches!(error_type, ErrorType::MissingInputs | ErrorType::DoubleSpend) {
        for input in inputs {
            let finding = investigate_single_input(input).await;
            findings.push(finding);
        }
    }
    
    // === STEP 3: Understand the situation ===
    InvestigationResult {
        error_type,
        findings,
        recommended_action: determine_action(&findings),
    }
}

/// Investigate a single input - what's its actual state?
async fn investigate_single_input(input: &OutPoint) -> InputFinding {
    // Query chain: does the parent tx exist?
    let parent_exists = check_tx_exists(&input.txid).await;
    if !parent_exists {
        return InputFinding::ParentTxNotFound {
            // Bad data in our DB? How did we get this UTXO?
            outpoint: input.clone(),
            recommendation: "Check how this UTXO entered our database",
        };
    }
    
    // Query chain: is this specific output spent?
    match check_output_spent(&input.txid, input.vout).await {
        SpendStatus::Unspent => {
            // Miner said missing but chain says unspent?
            // Could be: miner doesn't have parent tx yet (transient)
            InputFinding::StillUnspent {
                outpoint: input.clone(),
                recommendation: "Retry - likely transient propagation issue",
            }
        }
        SpendStatus::SpentBy(spending_txid) => {
            // WHO spent it? Was it us or external?
            let is_our_tx = check_if_our_transaction(&spending_txid).await;
            
            if is_our_tx {
                InputFinding::SpentByOurTx {
                    outpoint: input.clone(),
                    spending_txid,
                    recommendation: "Race condition - check if that tx succeeded",
                }
            } else {
                InputFinding::SpentExternally {
                    outpoint: input.clone(),
                    spending_txid,
                    recommendation: "External spend - update DB and notify user",
                }
            }
        }
        SpendStatus::Unknown => {
            InputFinding::CouldNotDetermine {
                outpoint: input.clone(),
                recommendation: "API error - retry investigation later",
            }
        }
    }
}

/// Based on investigation findings, decide what to do
fn determine_action(findings: &[InputFinding]) -> RecommendedAction {
    // If all inputs are still unspent, it's likely transient - retry same tx
    if findings.iter().all(|f| matches!(f, InputFinding::StillUnspent { .. })) {
        return RecommendedAction::RetrySameTx { reason: "Transient propagation issue" };
    }
    
    // If any input was spent externally, update DB and retry with different inputs
    let external_spends: Vec<_> = findings.iter()
        .filter_map(|f| match f {
            InputFinding::SpentExternally { outpoint, spending_txid, .. } => {
                Some((outpoint.clone(), spending_txid.clone()))
            }
            _ => None
        })
        .collect();
    
    if !external_spends.is_empty() {
        return RecommendedAction::UpdateDbAndRetry {
            inputs_to_mark_spent: external_spends,
            notify_user: true,
        };
    }
    
    // If our own tx spent it, check if that tx succeeded
    if findings.iter().any(|f| matches!(f, InputFinding::SpentByOurTx { .. })) {
        return RecommendedAction::CheckOurOtherTx { 
            reason: "Possible race condition with our own transaction" 
        };
    }
    
    // Default: need manual investigation
    RecommendedAction::ManualReview { findings: findings.to_vec() }
}
```

#### Retry Logic Based on Investigation

```rust
async fn create_action_with_investigation(
    request: CreateActionRequest,
    max_retries: u32,
) -> Result<CreateActionResponse, Error> {
    let mut excluded_outpoints: HashSet<OutPoint> = HashSet::new();
    
    for attempt in 1..=max_retries {
        // Select UTXOs, excluding any we've confirmed are bad
        let selected_utxos = select_utxos(
            request.satoshis,
            &excluded_outpoints,
        )?;
        
        // Build and sign transaction (trust our DB)
        let tx = build_transaction(&selected_utxos, &request)?;
        
        // Attempt broadcast
        match broadcast_transaction(&tx).await {
            Ok(result) => return Ok(result),
            
            Err(e) if is_input_error(&e) => {
                // === INVESTIGATE, don't blindly mark ===
                let investigation = investigate_broadcast_failure(
                    &tx.txid, &e, &selected_utxos
                ).await;
                
                log::info!("Investigation result: {:?}", investigation);
                
                match investigation.recommended_action {
                    RecommendedAction::RetrySameTx { reason } => {
                        // Transient issue - retry with same inputs after delay
                        log::info!("Retrying same tx: {}", reason);
                        tokio::time::sleep(backoff_duration(attempt)).await;
                        continue;
                    }
                    
                    RecommendedAction::UpdateDbAndRetry { inputs_to_mark_spent, notify_user } => {
                        // We confirmed these inputs are spent elsewhere
                        for (outpoint, spending_txid) in &inputs_to_mark_spent {
                            // NOW we can safely update DB - we investigated first
                            mark_output_spent_by_external(&outpoint, &spending_txid);
                            excluded_outpoints.insert(outpoint.clone());
                        }
                        invalidate_balance_cache();
                        
                        if notify_user {
                            emit_balance_change_event();
                        }
                        
                        log::info!("Retrying with different inputs (attempt {}/{})", 
                            attempt + 1, max_retries);
                        continue;
                    }
                    
                    RecommendedAction::CheckOurOtherTx { reason } => {
                        // Our own tx might have succeeded - check before retrying
                        log::info!("Checking our other tx: {}", reason);
                        // Could be a race condition success - don't retry blindly
                        return Err(Error::PossibleRaceCondition(investigation));
                    }
                    
                    RecommendedAction::ManualReview { findings } => {
                        // Can't determine automatically - need human review
                        log::warn!("Manual review needed: {:?}", findings);
                        return Err(Error::NeedsManualReview(findings));
                    }
                }
            },
            
            Err(e) if is_transient_error(&e) => {
                // Network issue - retry same tx
                log::info!("Transient error, retrying: {}", e);
                tokio::time::sleep(backoff_duration(attempt)).await;
                continue;
            },
            
            Err(e) => {
                // Fatal error (dust, tx-size, script error) - don't retry
                return Err(e);
            }
        }
    }
    
    Err(Error::MaxRetriesExceeded)
}
```

### 2.4 Why We Do NOT Do Proactive/Periodic Validation

> ⛔ **DO NOT implement periodic UTXO validation against raw chain APIs**

We previously attempted proactive UTXO validation and it **corrupted the database**, marking valid spendable tokens as spent. The problems:

1. **Push/drops**: Ordinals/tokens use push/drop patterns where a UTXO is "spent" but the token moves to a new output we control. Raw UTXO APIs see "spent" and incorrectly mark our tokens as gone.

2. **BRC-42 derived addresses**: Our wallet uses BRC-42 key derivation. WhatsOnChain doesn't understand these derivation paths and may not recognize our outputs.

3. **Token indexers are the source of truth**: For BSV-21 tokens, GorillaPool/1sat-stack track the token DAG. Raw UTXO APIs don't understand token semantics.

4. **False positives destroy user funds**: If we mark a valid token UTXO as "spent" based on incorrect API data, the user loses access to their tokens.

**The ONLY safe validation is reactive** - when a miner actually rejects a transaction, we investigate those specific inputs.

---

## Part 3: Token-Specific Considerations

### 3.1 BSV-21 Token Input Selection

Tokens require special handling because:
1. **Token UTXOs are specific** - can't substitute one token for another
2. **Token transfers must preserve amounts** - inputs must match outputs
3. **Origin tracking** - some tokens have specific origin requirements

```rust
async fn select_token_inputs_with_retry(
    token_id: &str,
    amount: u64,
    excluded: &HashSet<OutPoint>,
) -> Result<Vec<TokenUtxo>, Error> {
    let available = get_token_utxos(token_id)
        .into_iter()
        .filter(|u| !excluded.contains(&u.outpoint()))
        .collect();
    
    if sum_amounts(&available) < amount {
        // Not enough tokens after excluding bad UTXOs
        // Trigger full token sync from GorillaPool/1sat-stack
        sync_token_utxos(token_id).await?;
        
        // Retry selection
        let refreshed = get_token_utxos(token_id)
            .into_iter()
            .filter(|u| !excluded.contains(&u.outpoint()))
            .collect();
        
        if sum_amounts(&refreshed) < amount {
            return Err(Error::InsufficientTokenBalance);
        }
        
        return select_optimal_token_inputs(&refreshed, amount);
    }
    
    select_optimal_token_inputs(&available, amount)
}
```

### 3.2 Token UTXO Validation

```rust
async fn validate_token_utxo(
    token_id: &str,
    outpoint: &OutPoint,
) -> TokenValidationResult {
    // Check 1sat-stack / GorillaPool for current ownership
    let api_result = query_token_utxo(token_id, outpoint).await?;
    
    match api_result {
        Some(utxo) if utxo.owner == our_address => {
            TokenValidationResult::Valid(utxo)
        }
        Some(utxo) => {
            // Token transferred to someone else
            TokenValidationResult::TransferredAway(utxo.owner)
        }
        None => {
            // Token doesn't exist or was burned
            TokenValidationResult::NotFound
        }
    }
}
```

---

## Part 4: API Endpoints for Self-Heal

### 4.1 Query Endpoints Needed

| Endpoint | Provider | Purpose |
|----------|----------|---------|
| `GET /tx/{txid}` | WoC / 1sat-stack | Check if tx exists, get confirmations |
| `GET /tx/{txid}/spent/{vout}` | WoC | Check if specific output is spent |
| `GET /address/{addr}/unspent` | WoC / 1sat-stack | Get current UTXOs for address |
| `GET /beef/{txid}` | 1sat-stack | Get full BEEF for proof |
| `POST /txo/spends` | 1sat-stack | Bulk check spend status |
| `GET /bsv21/{id}/p2pkh/{addr}/unspent` | 1sat-stack | Token UTXO status |

### 4.2 WhatsOnChain Spent Check

```rust
async fn check_if_output_spent(
    client: &reqwest::Client,
    txid: &str,
    vout: u32,
) -> Result<Option<String>, Error> {
    let url = format!(
        "https://api.whatsonchain.com/v1/bsv/main/tx/{}/out/{}/spent",
        txid, vout
    );
    
    let resp = client.get(&url).send().await?;
    
    match resp.status() {
        StatusCode::OK => {
            // Output is spent - response contains spending txid
            let spending_tx: SpentResponse = resp.json().await?;
            Ok(Some(spending_tx.txid))
        }
        StatusCode::NOT_FOUND => {
            // Output is unspent
            Ok(None)
        }
        _ => Err(Error::ApiError(resp.status()))
    }
}
```

### 4.3 1sat-stack Bulk Spend Check

```rust
async fn bulk_check_spends(
    client: &reqwest::Client,
    outpoints: &[String], // format: "txid.vout"
) -> Result<HashMap<String, SpendStatus>, Error> {
    let url = "https://api.1sat.app/1sat/txo/spends";
    
    let resp = client
        .post(url)
        .json(&outpoints)
        .send()
        .await?;
    
    let results: Vec<SpendResult> = resp.json().await?;
    
    Ok(results.into_iter()
        .map(|r| (r.outpoint, r.status))
        .collect())
}
```

---

## Part 5: Implementation Phases

### Phase 1: Enhanced Error Classification (Week 1)
- [ ] Create comprehensive error enum with all known patterns
- [ ] Add structured error parsing for ARC responses
- [ ] Add structured error parsing for WoC responses
- [ ] Add structured error parsing for GorillaPool responses
- [ ] Unit tests for error classification

### Phase 2: Investigation Infrastructure (Week 2)
- [ ] Implement `check_if_output_spent()` helper (for post-rejection investigation only)
- [ ] Implement `check_if_our_transaction()` helper
- [ ] Create `investigate_broadcast_failure()` function
- [ ] Create `InputFinding` and `InvestigationResult` types
- [ ] Unit tests for investigation logic

### Phase 3: Reactive Self-Heal (Week 3)
- [ ] Add `mark_output_spent_by_external()` to OutputRepository
- [ ] Integrate investigation into `broadcast_transaction()` error path
- [ ] Only update DB after investigation confirms external spend
- [ ] Add balance cache invalidation after confirmed external spends
- [ ] Logging to `monitor_events` for audit trail

### Phase 4: Retry with Alternative Inputs (Week 4)
- [ ] Add `excluded_outpoints` parameter to UTXO selection
- [ ] Implement retry loop that uses investigation results
- [ ] Handle "retry same tx" vs "retry with different inputs" logic
- [ ] Handle token-specific retry (can't substitute tokens)
- [ ] Integration tests with mock miner responses

### Phase 5: Monitoring & Alerts (Week 5)
- [ ] Add investigation events to `monitor_events` table
- [ ] Create metrics for:
  - Investigation trigger count by error type
  - Confirmed external spends
  - Retry success/failure rates
- [ ] User notification for confirmed balance changes
- [ ] Optional: webhook for significant events

---

## Part 6: Database Schema Changes

### 6.1 New Columns (Optional)

These columns are **optional** - the core self-heal logic can work without them by using existing columns (`spendable=0`, `spending_description='external-spend'`). Add only if you want better audit trail.

```sql
-- Track when outputs were confirmed externally spent (after investigation)
-- Only set AFTER miner rejection AND investigation confirms external spend
ALTER TABLE outputs ADD COLUMN external_spend_txid TEXT;
ALTER TABLE outputs ADD COLUMN external_spend_detected_at INTEGER;

-- Track retry attempts for failed transactions
ALTER TABLE transactions ADD COLUMN retry_count INTEGER DEFAULT 0;
ALTER TABLE transactions ADD COLUMN last_retry_at INTEGER;
ALTER TABLE transactions ADD COLUMN failure_reason TEXT;  -- Structured reason from investigation
```

> ⚠️ **Removed**: `last_validated_at` and `validation_status` columns. We do NOT do proactive validation, so these aren't needed.

### 6.2 New Table: Self-Heal Events

```sql
CREATE TABLE self_heal_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,  -- 'input_spent', 'utxo_invalid', 'balance_corrected'
    output_txid TEXT,
    output_vout INTEGER,
    external_txid TEXT,        -- The tx that spent our output
    details TEXT,              -- JSON with additional context
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX idx_self_heal_events_type ON self_heal_events(event_type);
CREATE INDEX idx_self_heal_events_created ON self_heal_events(created_at);
```

---

## Part 7: User-Facing Considerations

### 7.1 When to Notify Users

| Event | Notify? | Message |
|-------|---------|---------|
| Retry succeeded with alt inputs | No | Silent recovery |
| Balance decreased (external spend) | Yes | "Your balance has been updated" |
| Token transferred externally | Yes | "Token balance updated" |
| Transaction failed permanently | Yes | Show error with reason |
| Self-heal recovered lost funds | Yes | "Balance corrected" |

### 7.2 UI Implications

- Balance should refresh after any self-heal event
- Transaction history should show external spends
- Failed transactions should show actionable error messages
- "Sync" button should trigger manual reconciliation

---

## Part 8: Security Considerations

1. **Rate Limiting**: Don't trigger DOS on blockchain APIs
2. **Trust but Verify**: Cross-check API responses when possible
3. **Atomic Updates**: Use database transactions for multi-table updates
4. **Audit Trail**: Log all self-heal actions for debugging
5. **Replay Protection**: Don't re-mark already-processed events

---

## Part 9: Testing Strategy

### Unit Tests
- Error classification for all known patterns
- UTXO selection with exclusions
- Self-heal database operations

### Integration Tests
- Mock API responses for various error scenarios
- End-to-end retry flow
- Reconciliation task with mock chain state

### Manual Testing
- Intentionally spend UTXO externally, verify self-heal
- Create double-spend scenario, verify detection
- Test with real testnet transactions

---

## Appendix A: Current Code Locations

| File | Relevant Functions |
|------|-------------------|
| `handlers.rs` | `broadcast_transaction()`, `is_fatal_broadcast_error()` |
| `monitor/task_check_for_proofs.rs` | `mark_failed()`, ARC status handling |
| `monitor/task_send_waiting.rs` | `is_permanent_error()`, crash recovery |
| `monitor/task_unfail.rs` | False failure recovery |
| `monitor/task_validate_utxos.rs` | UTXO validation (exists, needs expansion) |
| `database/output_repo.rs` | `restore_spent_by_txid()`, `delete_by_txid()` |

## Appendix B: External API Documentation

- **ARC API**: https://arc.bitcoinsv.com/docs
- **WhatsOnChain**: https://developers.whatsonchain.com
- **1sat-stack**: https://github.com/b-open-io/1sat-stack
- **GorillaPool**: https://www.gorillapool.io/docs

---

## Summary

This plan transforms our error handling from "mark failed and give up" to "investigate why, understand the situation, and act based on evidence."

### Core Philosophy: Reactive, Not Proactive

```
❌ DON'T: Periodically validate UTXOs against chain APIs
❌ DON'T: Pre-validate inputs before building transactions
❌ DON'T: Blindly mark inputs as "bad" when miner rejects

✅ DO: Trust our DB to build transactions
✅ DO: Broadcast to miner (they know current state)
✅ DO: Investigate specific inputs ONLY after miner rejection
✅ DO: Understand WHY before updating any state
✅ DO: Only update DB after confirming findings
```

### Key Deliverables

1. **Comprehensive error taxonomy** - Know exactly what each miner response means
2. **Investigation infrastructure** - Tools to query chain and understand input state
3. **Reactive self-heal** - Only update DB after miner rejection AND investigation confirms findings
4. **Intelligent retry** - Use investigation results to decide: retry same tx, retry with different inputs, or alert user
5. **Audit trail** - Log what we investigated and what we found

### Why No Proactive Validation?

Previous attempt at periodic UTXO validation corrupted the database, marking valid tokens as spent. Token wallets have unique challenges (push/drops, BRC-42 derivation, token indexer semantics) that raw UTXO APIs don't understand. The miner is the only authority we can trust, and only at the moment of broadcast.

This ensures users don't see unnecessary failures while protecting against false positives that could destroy access to their funds.
