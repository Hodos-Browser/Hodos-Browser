# ARC Migration Plan: Broadcasting & Transaction Chaining

## Problem Statement

Our wallet has a **transaction chaining race condition**: when two concurrent `createAction` calls fire, the second spends change from the first. The first transaction broadcasts to miners, but the second fails with "Missing inputs" because miners haven't seen the first transaction yet.

**Current broadcast flow (broken for chained transactions):**
```
createAction → build BEEF → extract raw tx → POST raw tx to GorillaPool mAPI / WhatsOnChain
```

The BEEF contains the full SPV proof chain, but we strip it and only send the raw transaction. Miners and overlay services don't get the parent transaction data they need.

**Target broadcast flow (ARC + BEEF):**
```
createAction → build BEEF (including unconfirmed parents) → POST BEEF to ARC endpoint
```

ARC natively understands BEEF and can validate the entire ancestry chain, including unconfirmed parents.

---

## Research Findings

### 1. ARC API Specification

ARC (A Record of Commitments) is the standard BSV transaction processor that replaced mAPI. Open source at [github.com/bitcoin-sv/arc](https://github.com/bitcoin-sv/arc). API docs at [bitcoin-sv.github.io/arc/api.html](https://bitcoin-sv.github.io/arc/api.html).

#### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/tx` | Submit single transaction (raw hex, EF, or BEEF V1) |
| `POST` | `/v1/txs` | Submit batch of transactions |
| `GET` | `/v1/tx/{txid}` | Query transaction status |
| `GET` | `/v1/policy` | Get mining fee policy |
| `GET` | `/v1/health` | Health check |

#### POST /v1/tx - Submit Transaction

**Accepted formats (auto-detected from hex content):**
1. Single serialized raw transaction hex
2. Single EF (Extended Format) serialized raw transaction hex
3. **V1 serialized BEEF** (this is what we need)
4. V2 BEEF is NOT supported - must convert to V1

**Content-Type:** `application/json` with body `{ "rawTx": "<hex_string>" }`

The `rawTx` field accepts all three formats. ARC auto-detects whether the hex is a raw tx, EF tx, or BEEF.

**Request Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `Content-Type` | Yes | `application/json` |
| `Authorization` | TAAL only | `Bearer <api_key>` |
| `X-WaitFor` | No | Status to wait for: `RECEIVED`, `STORED`, `ANNOUNCED_TO_NETWORK`, `SENT_TO_NETWORK`, `ACCEPTED_BY_NETWORK`, `SEEN_ON_NETWORK` |
| `X-MaxTimeout` | No | Max wait seconds (default 5, max 30) |
| `X-CallbackUrl` | No | URL for status update webhooks |
| `X-CallbackToken` | No | Auth token for callback endpoint |
| `X-FullStatusUpdates` | No | Include orphan/network statuses in callbacks |
| `X-SkipFeeValidation` | No | Bypass fee check |
| `X-CumulativeFeeValidation` | No | Validate cumulative fees for consolidation txs |

**Success Response (HTTP 200):**
```json
{
  "txid": "b68b064b336b9a4abdb173f3e32f27b38a222cb2102f51b8c92563e816b12b4a",
  "txStatus": "STORED",
  "status": 200,
  "title": "OK",
  "blockHash": "",
  "blockHeight": 0,
  "merklePath": "fe54251800...",
  "extraInfo": "",
  "timestamp": "2023-03-09T12:03:48.382910514Z",
  "competingTxs": null
}
```

**Key response fields:**
- `txid` - The transaction ID
- `txStatus` - Human-readable status (see Transaction Status Model below)
- `merklePath` - BUMP-format merkle path (BRC-74), populated when tx is mined
- `competingTxs` - Array of competing TXIDs if double-spend detected

**Error Codes:**

| Code | Title | Meaning |
|------|-------|---------|
| 400 | Bad Request | General validation failure |
| 401 | Unauthorized | Missing/invalid API key |
| 409 | Conflict | Transaction already known (treat as success) |
| 422 | Unprocessable | Invalid format |
| 460 | Not extended format | Expected BEEF but got raw |
| 461 | Malformed transaction | Invalid scripts |
| 462 | Invalid inputs | Missing or spent inputs |
| 465 | Fee too low | Insufficient fee |
| 467 | Mined ancestors not found | BEEF validation: missing confirmed ancestors |
| 468 | Invalid BUMPs | Cannot calculate Merkle roots from BUMPs |
| 469 | Merkle roots validation failed | BUMP doesn't match block header |

#### GET /v1/tx/{txid} - Transaction Status

Returns same JSON format as submission response. Use this to poll status.

#### GET /v1/policy - Fee Policy

```json
{
  "policy": {
    "maxscriptsizepolicy": 500000,
    "maxtxsigopscountspolicy": 4294967295,
    "maxtxsizepolicy": 10000000,
    "miningFee": { "satoshis": 1, "bytes": 1000 }
  },
  "timestamp": "2024-01-15T12:00:00Z"
}
```

This gives us the miner's current fee rate (satoshis per 1000 bytes), replacing the MAPI fee quote we have as a TODO in handlers.rs.

#### Transaction Status Model

| Status | Name | Description |
|--------|------|-------------|
| 0 | UNKNOWN | Not found |
| 1 | QUEUED | In ARC's processing queue |
| 2 | RECEIVED | Validated format received |
| 3 | STORED | Persisted in ARC's store |
| 4 | ANNOUNCED_TO_NETWORK | INV sent to peers |
| 5 | REQUESTED_BY_NETWORK | Peers requested the tx |
| 6 | SENT_TO_NETWORK | Sent to at least one peer |
| 7 | ACCEPTED_BY_NETWORK | Accepted into peer mempool |
| 8 | SEEN_ON_NETWORK | Seen in connected node mempools |
| 106 | SEEN_IN_ORPHAN_MEMPOOL | In orphan pool (missing parent) |
| 108 | MINED | Included in a block |
| -1 | REJECTED | Transaction rejected |
| -3 | DOUBLE_SPEND_ATTEMPTED | Double-spend detected |

### 2. ARC Providers

| Provider | URL | API Key | Status |
|----------|-----|---------|--------|
| **GorillaPool** | `https://arc.gorillapool.io` | **Not required** (free) | Primary - use this |
| **TAAL** | `https://arc.taal.com` | Required (`console.taal.com`) | Fallback |
| **GorillaPool (old mAPI)** | `https://mapi.gorillapool.io/mapi/tx` | Not required | **DEPRECATED** - what we currently use |

The BSV TypeScript SDK uses GorillaPool ARC as its **default broadcaster** with no API key:
```typescript
// From ts-sdk/src/transaction/broadcasters/DefaultBroadcaster.ts
return new ARC('https://arc.gorillapool.io', config)
```

**Recommendation:** Use GorillaPool ARC as primary (no API key needed), TAAL ARC as fallback (needs key, add later).

### 3. How the TS BSV SDK Uses ARC

From `bsv-blockchain/ts-sdk`:

**ARC Broadcaster (`src/transaction/broadcasters/ARC.ts`):**
- POST to `{url}/v1/tx` with `Content-Type: application/json`
- Body: `{ rawTx: tx.toHexEF() }` - tries Extended Format first
- Falls back to `tx.toHex()` if EF unavailable
- Returns `{ txid, txStatus, merklePath }` on success

**Wallet-Toolbox (`src/services/providers/ARC.ts`):**
- Posts BEEF V1 hex to `/v1/tx` via same `{ rawTx: beef_hex }` body
- Auto-detects format: ARC determines if hex is raw tx, EF, or BEEF
- V2 BEEF auto-converted to V1 before sending (V2 not supported by ARC)
- Retrieves status via GET `/v1/tx/{txid}`
- Handles HTTP 409 (already known) as success

**Default Broadcaster:**
- `DefaultBroadcaster.ts` creates `new ARC('https://arc.gorillapool.io')`
- No API key, no special configuration

### 4. How BEEF Solves Transaction Chaining

**This is the core insight that solves our race condition.**

BEEF can include **both confirmed and unconfirmed parent transactions**:
- Confirmed parents get a BUMP (merkle proof) linking to a block header
- Unconfirmed parents have NO BUMP, but THEIR parents must also be in the BEEF
- The chain continues recursively until every branch terminates at a confirmed ancestor with a BUMP

**Example of our race condition scenario:**

```
Block 918980:  [Grandparent TX]  ← confirmed, has merkle proof
                    |
               [TX-A]            ← unconfirmed (just created by first createAction)
                    |
               [TX-B]            ← new transaction (second createAction, spends TX-A's change)
```

**The BEEF for TX-B contains all three:**
```
BEEF:
  BUMPs: [BUMP for Grandparent TX (block height + merkle path)]
  Transactions:
    0: Grandparent TX  (has BUMP at index 0)  ← confirmed ancestor
    1: TX-A            (no BUMP)               ← unconfirmed parent
    2: TX-B            (no BUMP)               ← the new transaction
```

When ARC receives this BEEF, it can:
1. Verify Grandparent TX against its block header via the BUMP
2. Verify TX-A's inputs reference valid Grandparent outputs
3. Verify TX-B's inputs reference valid TX-A outputs
4. Accept and broadcast both TX-A (if not already broadcast) and TX-B

**This eliminates the "Missing inputs" error entirely.**

### 5. Building BEEF from Local DB

Our database already stores everything needed:

| Data | DB Table | Field |
|------|----------|-------|
| Raw parent transaction bytes | `parent_transactions` | `raw_hex` |
| Our own transaction bytes | `transactions` | `raw_tx` |
| Merkle proof: block height | `merkle_proofs` | `block_height` |
| Merkle proof: tx index | `merkle_proofs` | `tx_index` |
| Merkle proof: nodes | `merkle_proofs` | `nodes` (JSON array) |
| Merkle proof: block hash | `merkle_proofs` | `target_hash` |

**BEEF building process from local DB:**
1. For the main transaction: already have the signed raw bytes
2. For each input's parent transaction: check `transactions` table first, then `parent_transactions` cache
3. For each parent: check `merkle_proofs` table for cached BUMP data
4. If parent is unconfirmed (no merkle proof): include it as-is AND recurse into its parents
5. If parent is confirmed: include it with BUMP, stop recursing

**When will TSC lookups still be needed?**
- Only when a parent transaction's merkle proof is NOT yet cached
- Over time, as we store merkle proofs from ARC responses, this becomes less frequent
- ARC returns `merklePath` in the response once a transaction is mined - we can cache this

### 6. Key Format Notes

**BEEF V1 vs V2:** ARC only accepts V1. Our code defaults to V2. We need to output V1 when submitting to ARC. The `Beef` struct in `beef.rs` already supports both - we just need to set the version bytes.

**Atomic BEEF:** Our `sign_action` currently outputs Atomic BEEF (BRC-95 with 36-byte header). For ARC submission, we should submit the inner BEEF (strip the atomic header). For overlay service submission and `createAction` response, keep the Atomic BEEF format.

---

## Implementation Plan

### Phase 1: Add ARC Broadcaster (Minimal Change)

**Goal:** Replace GorillaPool mAPI with GorillaPool ARC. Submit BEEF directly instead of extracting raw tx.

#### Step 1.1: Add `broadcast_to_arc()` function

**File:** `rust-wallet/src/handlers.rs`

Add a new broadcast function alongside the existing ones:

```rust
async fn broadcast_to_arc(
    client: &reqwest::Client,
    beef_hex: &str,       // Full BEEF hex (V1 format)
    raw_tx_hex: &str,     // Fallback: raw tx hex
) -> Result<ArcResponse, String>
```

- POST to `https://arc.gorillapool.io/v1/tx`
- Content-Type: `application/json`
- Body: `{ "rawTx": "<beef_v1_hex>" }`
- Parse response for `txid`, `txStatus`, `merklePath`
- Handle HTTP 409 as success (already known)
- Handle error codes 460-469

Return type captures useful ARC response data:
```rust
struct ArcResponse {
    txid: String,
    tx_status: String,
    merkle_path: Option<String>,  // BUMP hex, populated when mined
    block_height: Option<u64>,
}
```

#### Step 1.2: Ensure BEEF V1 output

**File:** `rust-wallet/src/beef.rs`

Our `Beef::to_hex()` currently outputs V2. Add a `to_v1_hex()` method or a parameter to control version. The V1 format is simpler (no format byte before transactions, just raw_tx + has_bump flag).

#### Step 1.3: Update `broadcast_transaction()` to prefer ARC

**File:** `rust-wallet/src/handlers.rs`

Modify the existing `broadcast_transaction()` function:

1. If input is BEEF format, send BEEF V1 hex directly to ARC (don't extract raw tx)
2. If ARC accepts (200/409), use the response
3. If ARC fails, fall back to extracting raw tx and sending to WhatsOnChain
4. Store ARC response data (merkle_path if present)

#### Step 1.4: Update `broadcast_status` from ARC responses

When ARC returns status, map to our `broadcast_status`:
- `STORED`/`ANNOUNCED_TO_NETWORK`/`SENT_TO_NETWORK` → `broadcast`
- `MINED` → `confirmed`
- `REJECTED` → `failed`

### Phase 2: Include Unconfirmed Parents in BEEF

**Goal:** When building BEEF for a transaction that spends unconfirmed outputs, include the unconfirmed parent transactions in the BEEF.

#### Step 2.1: Modify BEEF builder to include unconfirmed parents

**File:** `rust-wallet/src/beef_helpers.rs`

The `build_beef_for_txid` function already walks the ancestry chain. The issue is that for unconfirmed parents (no TSC proof available), it currently skips adding the merkle proof and may not recurse further.

Change behavior:
1. If parent has merkle proof in DB: add with BUMP (existing behavior)
2. If parent has no merkle proof but is in `transactions` or `parent_transactions` table: add WITHOUT BUMP, and recurse into its inputs
3. Continue until all branches reach confirmed ancestors with BUMPs

#### Step 2.2: Ensure newly created transactions are stored before next spend

**File:** `rust-wallet/src/handlers.rs`

In `create_action` and `sign_action`, ensure:
1. The signed transaction's raw bytes are stored in `transactions` table BEFORE the HTTP response is sent
2. Change UTXOs are stored in `utxos` table
3. This data is available for the NEXT createAction call that might spend the change

This should already be happening, but verify the timing - the second concurrent call must be able to find the first call's transaction data.

#### Step 2.3: Handle concurrent access

For two truly concurrent `createAction` calls that both try to spend the same UTXO:
- The `utxo_selection_lock` (tokio::Mutex) in AppState should serialize UTXO selection
- Verify this lock is held during the entire UTXO selection + reservation process
- The first call gets the UTXO, the second call gets the change from the first (or another UTXO)

### Phase 3: Cache Merkle Proofs from ARC

**Goal:** When ARC returns merkle paths, store them for future BEEF building.

#### Step 3.1: Parse and store merkle paths from ARC responses

When ARC returns `merklePath` (BUMP hex format per BRC-74):
1. Parse the BUMP hex to extract block_height, tx_index, and nodes
2. Store in `merkle_proofs` table linked to `parent_transactions`
3. This eliminates future TSC API calls for this transaction

#### Step 3.2: Poll ARC for status of pending transactions

Add a background task (similar to `utxo_sync`) that:
1. Queries `transactions` where `broadcast_status = 'broadcast'`
2. For each, GET `/v1/tx/{txid}` from ARC
3. If status is `MINED`, update `broadcast_status` to `confirmed`
4. Store the returned `merklePath` in the merkle proofs cache
5. Update `block_height` and `confirmations` in the transactions table

### Phase 4: Replace WhatsOnChain Fetching with ARC (Optional)

**Goal:** Use ARC for transaction lookups instead of WhatsOnChain where possible.

- GET `/v1/tx/{txid}` returns transaction status and merkle proof
- Keep WhatsOnChain as fallback for historical data not in ARC

### Phase 5: Dynamic Fee Rates from ARC Policy

**Goal:** Replace hardcoded fee rate with ARC policy endpoint.

- GET `/v1/policy` returns `miningFee: { satoshis: 1, bytes: 1000 }`
- Cache with TTL (1 hour)
- Fall back to `DEFAULT_SATS_PER_KB` on error

---

## Files to Modify

| File | Changes |
|------|---------|
| `rust-wallet/src/handlers.rs` | Add `broadcast_to_arc()`, modify `broadcast_transaction()`, store ARC merkle paths |
| `rust-wallet/src/beef.rs` | Add `to_v1_hex()` method for ARC-compatible output |
| `rust-wallet/src/beef_helpers.rs` | Modify to include unconfirmed parents in BEEF ancestry chain |
| `rust-wallet/src/database/merkle_proof_repo.rs` | Add method to store BUMP-format merkle paths from ARC |

## Files for Reference Only (DO NOT MODIFY)

| File | Purpose |
|------|---------|
| `rust-wallet/src/beef_ancestors.rs` | Disabled. Contains recursive ancestor collection (broken) |
| `rust-wallet/src/utxo_validation.rs` | Disabled. Contains on-chain validation (caused false positives) |

---

## Migration Path: Getting Back to Basket/Tag Testing

### Immediate (Phase 1 + 2)

1. **Add `broadcast_to_arc()`** - New function, minimal risk to existing code
2. **BEEF V1 output** - Small change in beef.rs
3. **Update `broadcast_transaction()`** - Send BEEF to ARC instead of raw tx to mAPI
4. **Include unconfirmed parents in BEEF** - Modify beef_helpers.rs
5. **Build and test** - Run through the same createAction test that was failing
6. **Resume Basket/Tag testing** - The createAction calls should now succeed even for chained transactions

### Deferred (Phase 3-5)

3. Cache merkle proofs from ARC responses (nice-to-have, reduces WhatsOnChain calls)
4. ARC status polling (nice-to-have, improves confirmation tracking)
5. Dynamic fee rates (nice-to-have, replaces hardcoded fees)

### Test Plan

1. Start wallet, create a todo token (single createAction) - should work with ARC
2. Create two todo tokens rapidly (concurrent createAction) - second should succeed because BEEF includes first's unconfirmed tx
3. Check ARC response: should include txid, txStatus, and eventually merklePath
4. Verify overlay services accept the BEEF (babbage.systems/submit, bsvb.tech/submit)
5. Continue with Basket/Tag testing once broadcasting works reliably

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| GorillaPool ARC endpoint down | Fall back to WhatsOnChain raw broadcast |
| ARC rejects BEEF V1 format | Log detailed error, fall back to raw tx broadcast |
| BEEF too large for concurrent chains | Limit ancestry depth, same as current max_depth |
| Overlay services still reject | Overlay rejection is separate from miner acceptance. Debug overlay errors independently |
| V2→V1 conversion breaks BEEF | Test BEEF V1 output against ARC before removing V2 path |

---

## Key Differences: Old (mAPI) vs New (ARC)

| Aspect | Old (mAPI) | New (ARC) |
|--------|-----------|-----------|
| Endpoint | `mapi.gorillapool.io/mapi/tx` | `arc.gorillapool.io/v1/tx` |
| Request body | `{ "rawtx": "<raw_tx_hex>" }` | `{ "rawTx": "<beef_v1_hex>" }` |
| Response | Nested JSON with `payload` string field | Direct JSON with `txid`, `txStatus` |
| Merkle proof return | Not returned | `merklePath` field (BUMP hex) |
| Transaction chaining | Not supported (parent must be mined) | **Supported via BEEF** (includes parent in submission) |
| Status tracking | No | GET `/v1/tx/{txid}` + callbacks |
| Fee policy | Separate MAPI feeQuote endpoint | GET `/v1/policy` |
| Already-known handling | `returnResult: "failure"` with text parsing | HTTP 409 with valid status |

---

## References

- [ARC GitHub Repository](https://github.com/bitcoin-sv/arc)
- [ARC API Documentation](https://bitcoin-sv.github.io/arc/api.html)
- [BSV TS SDK (bsv-blockchain/ts-sdk)](https://github.com/bsv-blockchain/ts-sdk)
- [BSV Wallet Toolbox (bsv-blockchain/wallet-toolbox)](https://github.com/bsv-blockchain/wallet-toolbox)
- [GorillaPool](https://gorillapool.io/) - Primary ARC provider (free, no API key)
- [GorillaPool Docs](https://docs.gorillapool.io/)
- [TAAL ARC](https://arc.taal.com) - Secondary ARC provider (API key required)
- [BRC-62: BEEF Format](https://bsv.brc.dev/transactions/0062)
- [BRC-74: BUMP Merkle Path Format](https://bsv.brc.dev/transactions/0074)
- [BRC-95: Atomic BEEF](https://bsv.brc.dev/transactions/0095)
- [BRC-96: BEEF V2](https://bsv.brc.dev/transactions/0096)
