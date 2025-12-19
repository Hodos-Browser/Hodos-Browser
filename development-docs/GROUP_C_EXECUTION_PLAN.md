# Group C: Output/Basket & Certificate Management - Execution Plan

> **Status**: 🚧 Implementation In Progress - Part 3 Implemented, Testing Needed
> **Last Updated**: 2025-12-19
> **Current Phase**: Part 3 Complete (all 4 methods implemented), Testing & Discovery Methods Next

## 📋 Table of Contents

1. [Overview](#overview)
2. [Current Status Assessment](#current-status-assessment)
3. [Method Breakdown & Research Plan](#method-breakdown--research-plan)
4. [Implementation Phases](#implementation-phases)
5. [Research Questions & Answers](#research-questions--answers)
6. [Dependencies & Prerequisites](#dependencies--prerequisites)
7. [Reference Documentation](#reference-documentation)
8. [Implementation Checklist](#implementation-checklist)

---

## Overview

Group C consists of **10 BRC-100 methods** covering:
- **Output Management** (2 methods): List and manage UTXOs
- **Blockchain Queries** (3 methods): Get height, headers, network info
- **Certificate Management** (4 methods): BRC-52 identity certificates
- **Certificate Discovery** (2 methods): Find certificates by identity/attributes
- **Async Support** (1 method): Wait for authentication

**Total Methods**: 10
**Database Schema**: ✅ Ready (baskets, certificates, utxos, block_headers tables exist)
**Estimated Timeline**: 3-4 weeks

### 📋 Key Research Findings (2025-12-08)

**Part 1: Output Management** ✅ **COMPLETE**:
- ✅ `listOutputs` - **IMPLEMENTED** - Supports basket filtering, tags, labels, BEEF generation
- ✅ `relinquishOutput` - **IMPLEMENTED** - Removes output from basket tracking
- ✅ Tag storage: Many-to-many tables (`output_tags` + `output_tag_map`) - **IMPLEMENTED**
- ✅ BEEF generation: Working code exists (`rust-wallet/src/beef.rs`) - **INTEGRATED**
- ✅ **Default basket name**: **NO DEFAULT** - basket is required parameter, "default" is prohibited

**Part 2: Blockchain Queries** ✅ **COMPLETE**:
- ✅ `getHeight` - **IMPLEMENTED** - Fetches current chain tip from WhatsOnChain `/chain/info` API
- ✅ `getHeaderForHeight` - **IMPLEMENTED** - Cache-first with API fallback, constructs 80-byte headers from API fields
- ✅ `getNetwork` - **IMPLEMENTED** - Returns "mainnet" (hardcoded, can be enhanced with config later)

**BRC-52 Certificates** ✅ **IMPLEMENTED**:
- ✅ Certificate structure understood and implemented (type, subject, validationKey, fields, certifier, signature, keyring)
- ✅ Fields are encrypted using BRC-2 (AES-GCM with BRC-42 key derivation) - fully implemented
- ✅ Selective disclosure via `keyring` for privacy-preserving field revelation - implemented
- ✅ UTXO-based revocation (check if `revocationOutpoint` is spent) - working
- ✅ ECDSA signature verification required (certifier signs certificate data) - working
- ✅ All four certificate handlers implemented (`acquireCertificate`, `listCertificates`, `proveCertificate`, `relinquishCertificate`)
- ✅ BRC-53 issuance protocol working (certifier creates transaction)
- ✅ Transaction ID extraction from revocationOutpoint (fixes "Not on Chain" issue)
- ⏳ End-to-end testing needed with real-world apps

**waitForAuthentication**:
- ✅ HTTP polling approach (not WebSocket)
- ✅ Polls `isAuthenticated` until true (1-second intervals)
- ✅ Timeout after 5 minutes recommended
- ✅ **Clarified**: NOT about authenticating with server/client (that's `/.well-known/auth`)
- ✅ **Purpose**: Waits for wallet initialization/readiness
- ⏳ Need to review metanet-desktop TypeScript implementation

**BRC-33 Message Relay**:
- ⚠️ Endpoints implemented but need real-world testing
- ✅ HTTP POST (not WebSocket)
- ⏳ Database persistence recommended (currently in-memory)

**Reference Implementations**:
- ⏳ go-wallet: https://github.com/BSVArchie/Babbage-Browser/tree/main/go-wallet
- ⏳ metanet-desktop: https://github.com/BSVArchie/metanet-desktop

---

## Current Status Assessment

### ✅ Completed Prerequisites

- **Group A (Authentication)**: ✅ Complete
- **Group B (Transactions)**: ✅ Complete
- **Database Migration**: ✅ Complete (Phases 1-9)
- **Database Schema**: ✅ Ready
  - `baskets` table exists
  - `certificates` table exists
  - `utxos` table exists (with `basket_id` foreign key)
  - `block_headers` table exists
- **UTXO Infrastructure**: ✅ Working (used by `createAction`)

### ⚠️ Partially Complete

- **BRC-33 Message Relay**: ✅ Endpoints implemented, ⚠️ In-memory storage only
  - `/sendMessage` - ✅ Working
  - `/listMessages` - ✅ Working
  - `/acknowledgeMessage` - ✅ Working
  - **Note**: Currently uses in-memory `MessageStore`, should migrate to database `messages` table

### ❌ Not Started

- All 10 Group C methods
- BRC-52 certificate parsing/verification
- Certificate discovery logic
- `waitForAuthentication` async support

---

## Method Breakdown & Research Plan

### Part 1: Output Management (Priority: HIGH)

#### Method 6: `listOutputs`
**Call Code**: 6
**Status**: ⏳ Database ready, endpoint pending
**Complexity**: Medium

**What It Does**:
- Lists spendable outputs (UTXOs) within a specific basket
- Supports filtering by basket (required) and tags (optional)
- Returns UTXO details with optional metadata (tags, labels, scripts, customInstructions)
- Can include locking scripts or entire transactions (BEEF format)

**Research Findings** ✅ (from [BRC-100 spec](https://bsv.brc.dev/wallet/0100)):

**Parameters** (from spec):
- `basket` (required): Basket name to filter outputs
- `tags` (optional): Array of output tags to filter by
- `tagQueryMode` (optional): 'all' (all tags must match) or 'any' (any tag matches) - default 'any'
- `include` (optional): 'locking scripts' or 'entire transactions' (BEEF)
- `includeCustomInstructions` (optional): Include custom instructions in response
- `includeTags` (optional): Include tags in response
- `includeLabels` (optional): Include transaction labels in response
- `limit` (optional): Maximum number of outputs (default 10, max 10000)
- `offset` (optional): Number of outputs to skip (pagination)
- `seekPermission` (optional): Request user permission (default true)

**Return Format**:
```json
{
  "totalOutputs": 42,
  "outputs": [
    {
      "outpoint": "txid.vout",
      "satoshis": 1000,
      "spendable": true,
      "lockingScript": "hex_string",  // if include='locking scripts'
      "customInstructions": "...",    // if includeCustomInstructions=true
      "tags": ["tag1", "tag2"],       // if includeTags=true
      "labels": ["label1"]            // if includeLabels=true (from transaction)
    }
  ],
  "BEEF": "hex_bytes"  // if include='entire transactions'
}
```

**Key Insights**:
- ✅ **Tags vs Labels**: Tags are for outputs (used in `listOutputs`), labels are for transactions (used in `listActions`)
- ✅ **Basket is Required**: Must specify a basket name (no "all baskets" option, no default basket)
- ✅ **"default" is Prohibited**: BRC-100 spec explicitly prohibits "default" as basket name (line 381)
- ✅ **Basket Name Rules**: Must be 5-400 chars, lowercase letters/numbers/spaces, can't start with "admin" or "p", can't end with "basket"
- ✅ **Spendable Always True**: All returned outputs are spendable (unspent)
- ✅ **BEEF Support**: Can return entire transactions in BEEF format if requested

**Dependencies**:
- ✅ Database UTXO repository (exists)
- ✅ Basket table (exists)
- ✅ UTXO table has `basket_id` foreign key
- ⏳ Tag support (need to add `output_tags` and `output_tag_map` tables)
- ✅ Label support (from transactions - `transaction_labels` table exists)

**Implementation Notes**:
- Query `utxos` table filtered by `basket_id` (resolve basket name to ID)
- Filter by tags if provided (JOIN with `output_tag_map` and `output_tags` tables)
- Apply pagination (limit/offset)
- Include optional fields based on request parameters:
  - `includeTags=true`: JOIN to get tags array
  - `includeLabels=true`: JOIN to `transactions` → `transaction_labels` (via txid)
  - `include='locking scripts'`: Include `script` field
  - `include='entire transactions'`: Build BEEF (see BEEF generation below)
- Return `totalOutputs` count (before pagination)
- **BEEF Generation** (if `include='entire transactions'`):
  - For each output, fetch its transaction from `transactions` table (by txid)
  - Fetch parent transactions for each input (from `parent_transactions` cache or API)
  - Build BEEF using `Beef::new()`, `add_parent_transaction()`, `set_main_transaction()`
  - Serialize with `to_bytes()` and return as hex string in response `BEEF` field

---

#### Method 7: `relinquishOutput`
**Call Code**: 7
**Status**: ❌ Not started
**Complexity**: Low

**What It Does**:
- Removes an output from a basket (stops tracking it)
- Output is NOT spent - just removed from wallet's basket tracking
- Used when user wants to stop tracking a UTXO in a specific basket

**Research Findings** ✅ (from [BRC-100 spec](https://bsv.brc.dev/wallet/0100)):

**Parameters** (from spec):
- `basket` (required): Basket name where output should be removed
- `output` (required): Outpoint string (format: "txid.vout")

**Return Format**:
```json
{
  "relinquished": true
}
```

**Key Insights**:
- ✅ **Not About Spending**: Output is NOT spent - just removed from basket tracking
- ✅ **Basket-Specific**: Removes output from specified basket (output may still exist in wallet if in other baskets)
- ✅ **Use Case**: User wants to stop tracking a UTXO in a basket without spending it
- ✅ **Simple Operation**: Just remove `basket_id` from UTXO record (set to NULL)

**Dependencies**:
- ✅ Database UTXO repository (exists)
- ✅ UTXO table has `basket_id` column (nullable foreign key)
- ✅ Basket table (exists)

**Implementation Notes**:
- Resolve basket name to `basket_id`
- Find UTXO by outpoint (parse "txid.vout" format)
- Update UTXO: set `basket_id = NULL` (removes from basket)
- Return `{ relinquished: true }`
- **No schema changes needed** - `basket_id` is already nullable

---

### Part 2: Blockchain Queries (Priority: MEDIUM) ✅ **COMPLETE!**

#### Method 25: `getHeight`
**Call Code**: 25
**Status**: ✅ **COMPLETE**
**Complexity**: Low

**What It Does**:
- Returns current blockchain height (chain tip)
- Simple utility method

**Implementation** ✅:
- ✅ Fetches from WhatsOnChain `/chain/info` API
- ✅ Extracts `blocks` field from response
- ✅ Returns `{ height: number }`
- ✅ No caching (height changes every ~10 minutes, API call is fast)

**Dependencies**:
- ✅ WhatsOnChain API (already used)

**Implementation Notes**:
- Single API call to WhatsOnChain `/chain/info`
- Returns current chain tip height (not specific transaction height)
- **File**: `rust-wallet/src/handlers.rs` - `get_height()` function

---

#### Method 26: `getHeaderForHeight`
**Call Code**: 26
**Status**: ✅ **COMPLETE**
**Complexity**: Medium

**What It Does**:
- Returns 80-byte block header for a given height
- Uses cached block headers from database

**Implementation** ✅:
- ✅ Checks database cache first using `block_header_repo.get_by_height()`
- ✅ Falls back to WhatsOnChain API if not cached
- ✅ Constructs 80-byte header from API response fields (version, previousblockhash, merkleroot, time, bits, nonce)
- ✅ Caches result for future use
- ✅ Returns `{ header: "hex_string" }` (80 bytes, hex-encoded)

**Key Implementation Details**:
- Uses `/block/height/{height}` to get block hash
- Then uses `/block/{hash}/header` to get header fields (as per ts-brc100 reference)
- Constructs 80-byte header: version (4) + prev_hash (32) + merkle_root (32) + time (4) + bits (4) + nonce (4)
- All fields are little-endian encoded (Bitcoin format)

**Dependencies**:
- ✅ Database `block_headers` table (exists)
- ✅ `block_header_repo.rs` (exists)
- ✅ Background cache sync (already populating block headers)

**Implementation Notes**:
- **File**: `rust-wallet/src/handlers.rs` - `get_header_for_height()` function
- **Reference**: Follows ts-brc100 implementation pattern
- **API Endpoint**: `/block/{hash}/header` (not `/block/hash/{hash}`)

---

#### Method 27: `getNetwork`
**Call Code**: 27
**Status**: ✅ **COMPLETE**
**Complexity**: Very Low

**What It Does**:
- Returns network name ("mainnet" or "testnet")
- Simple configuration value

**Implementation** ✅:
- ✅ Returns hardcoded `{ network: "mainnet" }`
- ✅ Simple implementation (can be enhanced later with config file)

**Dependencies**:
- None (configuration value)

**Implementation Notes**:
- **File**: `rust-wallet/src/handlers.rs` - `get_network()` function
- **Future Enhancement**: Could read from config file or environment variable
- **Estimated Time**: 30 minutes (as estimated) ✅

---

### Part 3: Certificate Management (Priority: MEDIUM)

#### Method 17: `acquireCertificate`
**Call Code**: 17
**Status**: ✅ **IMPLEMENTED** - Working with socialcert.net
**Complexity**: High

**What It Does**:
- Acquires a BRC-52 identity certificate via 'direct' or 'issuance' protocol
- Parses certificate from JSON (not BEEF transaction - certificate data provided directly)
- Verifies certificate signature using BRC-52 verification
- Checks revocation status on-chain
- Stores certificate in database with correct transaction ID
- Returns certificate in flat format (matching BRC-52 spec)

**Implementation Status** ✅:
- ✅ Supports both 'direct' and 'issuance' protocols
- ✅ BRC-53 issuance protocol fully implemented (initialRequest → CSR → certificate)
- ✅ Certifier creates blockchain transaction (not wallet)
- ✅ Signature verification working
- ✅ On-chain revocation checking working
- ✅ Transaction ID extraction from revocationOutpoint (fixes "Not on Chain" issue)
- ✅ Working with socialcert.net

**Dependencies**:
- ✅ Database `certificates` table (exists)
- ❌ BRC-52 certificate parser (needs implementation)
- ❌ Certificate signature verification (needs implementation - ECDSA with secp256k1)
- ❌ Field decryption logic (BRC-2, needs implementation - AES-GCM with BRC-42 key derivation)
- ❌ BRC-2 encryption module (needs implementation)
- ❌ UTXO revocation checking (needs implementation - check if revocationOutpoint is spent)

**Implementation Notes** (Updated from BRC-52 spec):
1. **Parse Certificate**: Extract JSON structure from BEEF transaction output
2. **Verify Signature**:
   - Use certifier's public key (from `certifier` field)
   - Verify ECDSA signature covers: type, subject, validationKey, fields, certifier
   - Use secp256k1 signature verification
3. **Check Revocation**:
   - Query blockchain for `revocationOutpoint` (txid.vout)
   - If UTXO is spent, certificate is revoked (reject)
4. **Decrypt Fields** (optional, for storage):
   - Use BRC-2 decryption with BRC-42 key derivation
   - Decrypt `fields` and `keyring` if needed
   - Store encrypted or decrypted (based on privacy requirements)
5. **Store in Database**:
   - Store certificate metadata
   - Store encrypted fields (or decrypted if privacy allows)
   - Store `identity_key` (subject) for discovery
6. **Return Certificate Data**: Return certificate structure to caller

**Key Questions** - ✅ **ANSWERED** (from BRC-52 spec review):
- ✅ **How do we receive certificates?**: Via `acquireCertificate` method call from apps
- ✅ **What transaction format?**: BEEF transaction with certificate data in outputs
- ✅ **How do we validate signatures?**: ECDSA signature verification using certifier's public key
- ✅ **Are fields encrypted?**: Yes, using BRC-2 encryption (AES-GCM with BRC-42 key derivation)
- ✅ **What is selective disclosure?**: Privacy feature allowing field-by-field revelation via `keyring`

---

#### Method 18: `listCertificates`
**Call Code**: 18
**Status**: ✅ **IMPLEMENTED** - Needs testing
**Complexity**: Low

**What It Does**:
- Lists all certificates owned by wallet
- Supports filtering (by type, certifier, etc.)
- Returns certificate metadata

**Research Needed**:
- [ ] Review BRC-100 spec for `listCertificates`
- [ ] Understand filtering parameters
- [ ] Check if we need to decrypt fields or return encrypted

**Dependencies**:
- ✅ Database `certificates` table (exists)
- ⏳ Certificate repository (needs implementation)

**Implementation Notes**:
- Query database for certificates
- Support optional filters (type, certifier, active/relinquished)
- Return array of certificate objects
- Format: `{ certificates: [...] }`

---

#### Method 19: `proveCertificate`
**Call Code**: 19
**Status**: ✅ **IMPLEMENTED** - Needs testing
**Complexity**: Medium

**What It Does**:
- Proves ownership of a certificate
- Creates proof that wallet owns certificate
- Used for selective disclosure

**Research Needed**:
- [ ] Review BRC-100 spec for `proveCertificate`
- [ ] Understand proof format
- [ ] Review BRC-52 selective disclosure mechanism
- [ ] Check reference implementation

**Dependencies**:
- ✅ Database `certificates` table (exists)
- ⏳ Certificate proof generation (needs implementation)
- ⏳ BRC-52 selective disclosure logic (needs research)

**Implementation Notes**:
- Query certificate from database
- Generate proof (signature? hash? need to research)
- Return proof data
- Format: `{ proof: "..." }`

---

#### Method 20: `relinquishCertificate`
**Call Code**: 20
**Status**: ✅ **IMPLEMENTED** - Needs testing
**Complexity**: Low

**What It Does**:
- Releases a certificate (marks as relinquished)
- Wallet no longer claims ownership

**Research Needed**:
- [ ] Review BRC-100 spec for `relinquishCertificate`
- [ ] Understand use cases
- [ ] Check if this is permanent or reversible

**Dependencies**:
- ✅ Database `certificates` table (exists, has `relinquished` flag)
- ⏳ Certificate repository (needs implementation)

**Implementation Notes**:
- Update database: set `relinquished = 1`, `relinquished_at = NOW()`
- Return success
- Format: `{ relinquished: true }`

---

### Part 4: Certificate Discovery (Priority: LOW)

#### Method 21: `discoverByIdentityKey`
**Call Code**: 21
**Status**: ⏳ Database ready, endpoint pending
**Complexity**: Medium

**What It Does**:
- Discovers certificates by identity key
- Searches for certificates where `subject` matches identity key
- Used to find certificates issued to a specific identity

**Research Needed**:
- [ ] Review BRC-100 spec for `discoverByIdentityKey`
- [ ] Understand search scope (local database only? blockchain?)
- [ ] Check if we need to query blockchain or just database

**Dependencies**:
- ✅ Database `certificates` table (exists, has `identity_key` index)
- ⏳ Certificate repository (needs implementation)

**Implementation Notes**:
- Query database: `SELECT * FROM certificates WHERE identity_key = ?`
- Return array of matching certificates
- Format: `{ certificates: [...] }`

---

#### Method 22: `discoverByAttributes`
**Call Code**: 22
**Status**: ⏳ Database ready, endpoint pending
**Complexity**: High

**What It Does**:
- Discovers certificates by attribute values
- Searches certificate fields for matching attributes
- Supports complex queries (multiple attributes)

**Research Needed**:
- [ ] Review BRC-100 spec for `discoverByAttributes`
- [ ] Understand attribute query format
- [ ] Check if fields are encrypted (need decryption for search?)
- [ ] Review reference implementation

**Dependencies**:
- ✅ Database `certificates` table (exists, has `fields` JSON column)
- ⏳ Certificate repository (needs implementation)
- ⏳ Field decryption logic (if fields are encrypted)

**Implementation Notes**:
- Parse attribute query parameters
- Query database with JSON field matching
- May need to decrypt fields before searching
- Return array of matching certificates
- Format: `{ certificates: [...] }`

---

### Part 5: Async Support (Priority: LOW)

#### Method 24: `waitForAuthentication`
**Call Code**: 24
**Status**: ❌ Not started
**Complexity**: Medium

**What It Does**:
- Waits for **wallet to be initialized and ready**
- Returns when wallet is set up (has keys, addresses, etc.)
- Used by apps that need to wait for wallet initialization before making requests
- **NOT about authenticating with server/client** - that's `/.well-known/auth` (BRC-104)
- **NOT about mutual authentication** - that's `/.well-known/auth` (BRC-103/104)

**Research Needed**:
- [ ] **CRITICAL**: Review BRC-100 spec for `waitForAuthentication`
- [ ] Understand use case (when is this called?)
- [ ] Check if this requires WebSocket or polling
- [ ] Review relationship with BRC-33 message relay
- [ ] Check if BRC-34 federation uses WebSocket

**Current Understanding** (from spec):
- Simple method: waits until `isAuthenticated` returns true
- Returns: `{ authenticated: true }`
- No parameters required

**Key Distinction - `waitForAuthentication` vs `/.well-known/auth`**:

| Aspect | `waitForAuthentication` (BRC-100) | `/.well-known/auth` (BRC-104) |
|--------|-----------------------------------|-------------------------------|
| **Purpose** | Wait for wallet to be initialized | Mutual authentication between wallet and app |
| **When Used** | App needs wallet to be ready | App wants to authenticate with wallet |
| **What It Checks** | Wallet exists and is set up | Can wallet and app authenticate each other? |
| **Returns** | `{ authenticated: true }` | Authentication response with nonces and signatures |
| **Protocol** | BRC-100 (wallet interface) | BRC-103/104 (mutual authentication) |
| **Use Case** | "Is the wallet ready?" | "Can I authenticate with this wallet?" |
| **Implementation** | HTTP polling | HTTP POST with BRC-103 protocol |

**Example Flow**:
1. App calls `waitForAuthentication` → Waits for wallet to be initialized
2. Once wallet is ready, app calls `/.well-known/auth` → Mutual authentication
3. After authentication, app can make BRC-100 requests

**Dependencies**:
- ✅ `isAuthenticated` method (already implemented)
- ⏳ Async/polling mechanism (needs research)

**Implementation Approach** (Based on BRC-100 spec and metanet-desktop reference):

**Recommended: HTTP Polling** ✅
```rust
// Poll isAuthenticated every 1 second until true
// Timeout after 5 minutes
let timeout = Duration::from_secs(300); // 5 minutes
let start = Instant::now();
let poll_interval = Duration::from_secs(1);

loop {
    // Check timeout
    if start.elapsed() > timeout {
        return Err("Timeout waiting for authentication");
    }

    // Check authentication status
    if is_authenticated(&state).await? {
        return Ok(json!({ "authenticated": true }));
    }

    // Wait before next poll
    tokio::time::sleep(poll_interval).await;
}
```

**Key Points**:
- No WebSocket required (BRC-100 spec doesn't mention WebSocket)
- Simple HTTP polling is sufficient
- Timeout prevents infinite waiting
- Non-blocking async implementation

**Key Questions**:
- ❓ Does this need WebSocket, or is HTTP polling sufficient?
- ❓ Is this related to BRC-33 message relay WebSocket?
- ❓ What triggers authentication completion? (user approval? wallet creation?)

**Research Sources**:
- [BRC-100 Spec](https://bsv.brc.dev/wallet/0100) - `waitForAuthentication` section (Call Code 24)
- [BRC-104 Spec](https://bsv.brc.dev/peer-to-peer/0104) - `/.well-known/auth` (mutual authentication)
- [BRC-33 Spec](https://bsv.brc.dev/peer-to-peer/0033) - Message relay (HTTP POST, not WebSocket)
- [BRC-34 Spec](https://bsv.brc.dev/peer-to-peer/0034) - Federation (may use WebSocket?)
- **metanet-desktop** - https://github.com/BSVArchie/metanet-desktop - TypeScript reference implementation

**Implementation Notes**:
- Start with simple polling (Option 1)
- Can enhance to WebSocket later if needed
- Timeout after reasonable duration (e.g., 5 minutes)

---

## Implementation Phases

### Phase 1: Output Management (Week 1)
**Goal**: Complete UTXO listing and management

**Methods**:
1. `listOutputs` (Call Code 6) - 4-6 hours
2. `relinquishOutput` (Call Code 7) - 2-3 hours

**Deliverables**:
- ✅ `listOutputs` endpoint implemented
- ✅ `relinquishOutput` endpoint implemented
- ✅ Basket filtering support
- ✅ Unit tests
- ✅ Integration tests

**Success Criteria**:
- Can list all UTXOs
- Can filter by basket
- Can relinquish UTXOs
- Database updates correctly

---

### Phase 2: Blockchain Queries (Week 1-2) ✅ **COMPLETE!**
**Goal**: Complete blockchain utility methods

**Methods**:
1. ✅ `getNetwork` (Call Code 27) - 30 minutes - **COMPLETE**
2. ✅ `getHeight` (Call Code 25) - 1-2 hours - **COMPLETE**
3. ✅ `getHeaderForHeight` (Call Code 26) - 2-3 hours - **COMPLETE**

**Deliverables**:
- ✅ All three methods implemented
- ✅ Database-first lookup for `getHeaderForHeight` (cache then API)
- ✅ Block header construction from API fields
- ✅ Routes added to `main.rs`

**Success Criteria** - **ALL MET**:
- ✅ Returns correct network name ("mainnet")
- ✅ Returns current blockchain height from API
- ✅ Returns block headers from cache or API
- ✅ Fast response times (cache hits are instant)

**Testing**:
- ✅ Tested with PowerShell `Invoke-RestMethod` commands
- ✅ `getHeight` returns current chain tip (e.g., 926607)
- ✅ `getNetwork` returns "mainnet"
- ✅ `getHeaderForHeight` constructs 80-byte headers correctly

---

### Phase 3: Certificate Management (Week 2-3) ✅ **IMPLEMENTED** - Testing Needed
**Goal**: Complete BRC-52 certificate system

**Methods**:
1. ✅ `acquireCertificate` (Call Code 17) - **COMPLETE** - Working with socialcert.net
2. ✅ `listCertificates` (Call Code 18) - **COMPLETE** - Needs testing
3. ✅ `proveCertificate` (Call Code 19) - **COMPLETE** - Needs testing
4. ✅ `relinquishCertificate` (Call Code 20) - **COMPLETE** - Needs testing

**Deliverables**:
- ✅ BRC-52 certificate parser
- ✅ Certificate signature verification
- ✅ Field encryption/decryption (BRC-2)
- ✅ All four methods implemented
- ✅ Certificate repository
- ✅ BRC-53 issuance protocol
- ✅ Transaction ID extraction from revocationOutpoint
- ⏳ Unit tests (deferred to after real-world testing)
- ⏳ Integration tests with real certificates

**Success Criteria** - **MOSTLY MET**:
- ✅ Can acquire certificates via 'direct' and 'issuance' protocols
- ✅ Can list certificates (implemented, needs testing)
- ✅ Can prove certificate ownership (implemented, needs testing)
- ✅ Can relinquish certificates (implemented, needs testing)
- ✅ Certificate validation working

**Remaining Work**:
- ⏳ End-to-end testing with real-world apps
- ⏳ Certificate discovery methods (not yet implemented)

---

### Phase 4: Certificate Discovery (Week 3-4)
**Goal**: Complete certificate search functionality

**Methods**:
10. `discoverByIdentityKey` (Call Code 21) - 2-3 hours
11. `discoverByAttributes` (Call Code 22) - 4-6 hours

**Deliverables**:
- ✅ Identity key search
- ✅ Attribute-based search
- ✅ Field decryption for search (if needed)
- ✅ Unit tests

**Success Criteria**:
- Can find certificates by identity key
- Can find certificates by attributes
- Search is fast and accurate

---

### Phase 5: Async Support (Optional - Week 4)
**Goal**: Complete async authentication wait

**Methods**:
12. `waitForAuthentication` (Call Code 24) - 2-4 hours

**Deliverables**:
- ✅ Polling-based implementation
- ✅ Timeout handling
- ✅ Unit tests

**Success Criteria**:
- Returns when wallet is authenticated
- Handles timeouts gracefully
- Non-blocking implementation

---

## Research Questions & Answers

### BRC-33 Message Relay

**Q: Is BRC-33 related to WebSocket?**
**A**: ❓ **NEEDS RESEARCH**
- BRC-33 spec says HTTP POST (not WebSocket)
- We have WebSocket server on port 3302 (unclear purpose)
- BRC-34 (federation) may use WebSocket
- **Action**: Review BRC-33, BRC-34 specs to clarify

**Q: Should BRC-33 messages be persisted to database?**
**A**: ✅ **YES** (recommended)
- Currently in-memory only (lost on restart)
- Database `messages` table exists
- **Action**: Migrate `MessageStore` to database (separate task)

**Q: Is BRC-33 part of Group C?**
**A**: ❌ **NO**
- BRC-33 is separate from BRC-100
- Already implemented (endpoints working)
- Just needs database persistence (optional enhancement)

---

### waitForAuthentication

**Q: Does `waitForAuthentication` require WebSocket?**
**A**: ✅ **NO - HTTP Polling** (Based on BRC-100 spec)
- BRC-100 spec says simple wait (no WebSocket mentioned)
- **Reference**: metanet-desktop TypeScript implementation (needs review)
- **Implementation**: Poll `isAuthenticated` until it returns true
- **Action**: Implement polling-based approach (1-second intervals)

**Q: What triggers authentication completion?**
**A**: ✅ **Wallet Initialization Complete** (Based on spec and current implementation)
- Wallet must be created and initialized (has master key, addresses, etc.)
- `isAuthenticated` returns true when wallet is ready to use
- **NOT about authenticating with server/client** - that's `/.well-known/auth`
- **NOT about domain approval** - that's separate
- **Action**: Check `isAuthenticated` implementation - currently returns `true` always (may need to check wallet exists)

**Q: How is `waitForAuthentication` different from `/.well-known/auth`?**
**A**: ✅ **COMPLETELY DIFFERENT PURPOSES**
- **`waitForAuthentication` (BRC-100 Call Code 24)**:
  - Waits for **WALLET** to be initialized/ready
  - Checks if wallet exists and is set up
  - Used by apps that need to wait for wallet initialization
  - Returns when `isAuthenticated` returns true
  - **Purpose**: Wallet readiness check

- **`/.well-known/auth` (BRC-104)**:
  - **Mutual authentication** between wallet and app
  - Wallet authenticates with the app (and vice versa)
  - Creates authentication session
  - Uses BRC-103 protocol (nonce exchange, signature verification)
  - **Purpose**: Wallet-to-app authentication

**Key Difference**:
- `waitForAuthentication`: "Is the wallet ready to use?" (wallet setup)
- `/.well-known/auth`: "Can this app authenticate with the wallet?" (wallet-to-app auth)

**Q: Is this related to BRC-33 message relay?**
**A**: ✅ **NO**
- BRC-33 is for peer-to-peer messages (HTTP POST)
- `waitForAuthentication` is for wallet initialization (HTTP polling)
- WebSocket on port 3302 is likely for BRC-34 federation (separate feature)
- **Action**: Verify in BRC-34 spec

**Q: How does metanet-desktop implement this?**
**A**: ❓ **NEEDS REVIEW**
- Reference: https://github.com/BSVArchie/metanet-desktop
- TypeScript implementation using BSV/SDK
- Need to review implementation to understand approach
- **Action**: Review metanet-desktop code for `waitForAuthentication`

---

### Certificate Management

**Q: How do we receive certificates?**
**A**: ✅ **Via `acquireCertificate` method** (Based on BRC-100 spec)
- Apps call `acquireCertificate` with certificate transaction data
- Certificate is parsed from BEEF transaction
- Stored in database after validation
- **Action**: Implement `acquireCertificate` handler

**Q: What transaction format contains certificates?**
**A**: ✅ **BEEF Transaction with Certificate Data** (Based on BRC-52 spec)
- Certificates are embedded in transaction outputs
- Certificate structure includes: type, subject, validationKey, fields, certifier, signature
- Certificate data is JSON structure (may be in OP_RETURN or output script)
- **Action**: Review BRC-52 spec for exact transaction format

**Q: How do we validate certifier signatures?**
**A**: ✅ **ECDSA Signature Verification** (Based on BRC-52 spec)
- Certificate includes `certifier` public key (33-byte compressed)
- Certificate includes `signature` (DER-encoded ECDSA signature)
- Verify signature using certifier's public key
- Signature covers certificate data (type, subject, validationKey, fields, etc.)
- **Action**: Implement ECDSA signature verification using secp256k1

**Q: Are certificate fields encrypted?**
**A**: ✅ **YES - BRC-2 Encryption** (Based on BRC-52 spec)
- Certificate fields can be encrypted for privacy
- Uses BRC-2 encryption (AES-GCM with BRC-42 key derivation)
- `keyring` field contains encrypted revelation keys for selective disclosure
- Fields are base64-encoded encrypted data
- **Action**: Implement BRC-2 decryption for certificate fields

**Q: What is selective disclosure?**
**A**: ✅ **Privacy Feature** (Based on BRC-52 spec)
- Certificate holder can reveal specific fields without revealing all fields
- Uses `keyring` to generate revelation keys for each field
- Verifier can request specific fields to be revealed
- **Action**: Implement selective disclosure mechanism for `proveCertificate`

---

### Output Management

**Q: What is the difference between baskets and tags?**
**A**: ✅ **ANSWERED** (from BRC-100 spec)
- **Baskets**: Organize UTXOs into groups (like folders) - required for `listOutputs`
- **Tags**: Filter/search labels for outputs (like keywords) - optional filtering
- **Labels**: Transaction-level categorization (used in `listActions`, not `listOutputs`)
- **Key Difference**: Tags are for outputs, labels are for transactions
- **Action**: ✅ Complete - spec clarifies distinction

**Q: Can a UTXO be in multiple baskets?**
**A**: ✅ **NO** (from database schema and spec)
- Database schema has single `basket_id` (foreign key, nullable)
- Spec requires single basket name in `listOutputs` and `relinquishOutput`
- **Action**: ✅ Complete - single basket per UTXO is correct

**Q: How are tags stored?**
**A**: ✅ **ANSWERED** (verified in ts-brc100 TypeScript SDK)
- **Storage Method**: Separate tables (many-to-many relationship)
- **Tables**:
  - `output_tags` table: `outputTagId`, `userId`, `tag`, `isDeleted`, timestamps
  - `output_tags_map` table: `outputTagId`, `outputId`, `isDeleted`, timestamps
- **Reference**: `reference/ts-brc100/src/storage/schema/tables/TableOutputTag.ts` and `TableOutputTagMap.ts`
- **Query Pattern**: JOIN `outputs` → `output_tags_map` → `output_tags` (see `listOutputsKnex.ts` lines 96-107, 247-248)
- **Action**: ✅ Complete - need to add these tables to our schema

**Q: What is the default basket name?**
**A**: ✅ **NO DEFAULT BASKET** (verified in BRC-100 spec + ts-brc100 SDK)
- Basket parameter is **REQUIRED** in `listOutputs` and `relinquishOutput`
  - Verified in ts-brc100: `validationHelpers.ts` line 909 validates basket as required (no optional check)
  - Interface: `ValidListOutputsArgs` line 872 shows `basket: BasketStringUnder300Bytes` (required)
- **"default" is PROHIBITED** as a basket name (BRC-100 spec line 381: "Must not be default")
- Users/apps must explicitly provide a basket name (1-300 bytes in ts-brc100, 5-400 chars in spec - use spec rules)
- **Action**: ✅ Complete - basket is required, no default needed

---

## Dependencies & Prerequisites

### Required Reading (Before Implementation)

**Priority 1 - Must Read**:
1. [BRC-100 Spec](https://bsv.brc.dev/wallet/0100) - Group C methods (Call Codes 6, 7, 17-22, 24-27)
2. [BRC-52 Spec](https://bsv.brc.dev/peer-to-peer/0052) - Identity Certificates (for methods 17-22)
3. [BRC-2 Spec](https://bsv.brc.dev/transactions/0002) - Encryption (for certificate field decryption)

**Priority 2 - Should Read**:
4. [BRC-31 Spec](https://bsv.brc.dev/peer-to-peer/0031) - Authrite (already implemented, review for context)
5. [BRC-33 Spec](https://bsv.brc.dev/peer-to-peer/0033) - Message Relay (already implemented, review for WebSocket relationship)
6. [BRC-34 Spec](https://bsv.brc.dev/peer-to-peer/0034) - Federation (for WebSocket understanding)

**Priority 3 - Reference**:
7. TypeScript SDK - `reference/ts-brc100/` - **PRIMARY REFERENCE** (verified implementation)
8. Database Schema - `development-docs/RUST_WALLET_DB_ARCHITECTURE.md`
9. **metanet-desktop** - https://github.com/BSVArchie/metanet-desktop - ⭐ **PRIMARY REFERENCE** - TypeScript implementation with all Group C methods

---

### Code Dependencies

**Existing Code to Review**:
- ✅ `rust-wallet/src/database/utxo_repo.rs` - UTXO queries
- ✅ `rust-wallet/src/database/block_header_repo.rs` - Block header queries
- ✅ `rust-wallet/src/database/migrations.rs` - Database schema
- ✅ `rust-wallet/src/handlers.rs` - Existing BRC-100 handlers
- ✅ `rust-wallet/src/message_relay.rs` - BRC-33 implementation (for reference)

**New Code Needed**:
- ⏳ `rust-wallet/src/database/certificate_repo.rs` - Certificate repository
- ⏳ `rust-wallet/src/certificate/` - Certificate parsing/verification module
- ⏳ Certificate-related handlers in `handlers.rs`

---

## Reference Documentation

### Official Specifications

- **[BRC-100: Wallet Interface](https://bsv.brc.dev/wallet/0100)** - Main specification
- **[BRC-52: Identity Certificates](https://bsv.brc.dev/peer-to-peer/0052)** - Certificate format
- **[BRC-2: Encryption](https://bsv.brc.dev/transactions/0002)** - Field encryption
- **[BRC-31: Authrite](https://bsv.brc.dev/peer-to-peer/0031)** - Authentication (already implemented)
- **[BRC-33: Message Relay](https://bsv.brc.dev/peer-to-peer/0033)** - Message relay (already implemented)
- **[BRC-34: Federation](https://bsv.brc.dev/peer-to-peer/0034)** - Multi-server federation

### Internal Documentation

- `development-docs/BRC100_IMPLEMENTATION_GUIDE.md` - Overall implementation guide
- `development-docs/RUST_WALLET_DB_ARCHITECTURE.md` - Database schema
- `reference/BRC100_spec.md` - Full BRC-100 specification (local copy)
- `Developer_notes.md` - Implementation notes and discoveries

### Reference Implementations

- `reference/ts-brc100/` - TypeScript SDK reference
- `reference/go-wallet-toolbox/` - Go SDK reference (if available)

---

## Implementation Checklist

### Phase 1: Output Management

- [ ] **Research**
  - [ ] Review BRC-100 spec for `listOutputs` (Call Code 6)
  - [ ] Review BRC-100 spec for `relinquishOutput` (Call Code 7)
  - [ ] Understand basket vs tags difference
  - [ ] Review reference implementation

- [ ] **Implementation**
  - [ ] Implement `listOutputs` endpoint
  - [ ] Add basket filtering support
  - [ ] Implement `relinquishOutput` endpoint
  - [ ] Add database schema updates if needed (relinquish flag)

- [ ] **Testing**
  - [ ] Unit tests for `listOutputs`
  - [ ] Unit tests for `relinquishOutput`
  - [ ] Integration tests with real UTXOs
  - [ ] Test basket filtering

---

### Phase 2: Blockchain Queries

- [ ] **Research**
  - [ ] Review BRC-100 spec for `getNetwork` (Call Code 27)
  - [ ] Review BRC-100 spec for `getHeight` (Call Code 25)
  - [ ] Review BRC-100 spec for `getHeaderForHeight` (Call Code 26)
  - [ ] Check WhatsOnChain API endpoints

- [ ] **Implementation**
  - [ ] Implement `getNetwork` (30 min - trivial)
  - [ ] Implement `getHeight` with caching
  - [ ] Implement `getHeaderForHeight` (database-first, API fallback)

- [ ] **Testing**
  - [ ] Unit tests for all three methods
  - [ ] Test caching behavior
  - [ ] Test API fallback

---

### Phase 3: Certificate Management

- [ ] **Research** ⚠️ **CRITICAL - DO THIS FIRST**
  - [ ] Read [BRC-52 spec](https://bsv.brc.dev/peer-to-peer/0052) completely
  - [ ] Understand certificate structure
  - [ ] Understand certificate parsing from transactions
  - [ ] Understand signature verification
  - [ ] Understand field encryption (BRC-2)
  - [ ] Review reference implementation

- [ ] **Certificate Infrastructure**
  - [ ] Create `rust-wallet/src/certificate/` module
  - [ ] Implement certificate parser
  - [ ] Implement signature verifier
  - [ ] Implement field decryption (BRC-2)
  - [ ] Create `certificate_repo.rs`

- [ ] **Implementation**
  - [ ] Implement `listCertificates` (Call Code 18)
  - [ ] Implement `relinquishCertificate` (Call Code 20)
  - [ ] Implement `proveCertificate` (Call Code 19)
  - [ ] Implement `acquireCertificate` (Call Code 17)

- [ ] **Testing**
  - [ ] Unit tests for certificate parser
  - [ ] Unit tests for signature verification
  - [ ] Unit tests for all four methods
  - [ ] Integration tests with real certificates

---

### Phase 4: Certificate Discovery

- [ ] **Research**
  - [ ] Review BRC-100 spec for `discoverByIdentityKey` (Call Code 21)
  - [ ] Review BRC-100 spec for `discoverByAttributes` (Call Code 22)
  - [ ] Understand attribute query format

- [ ] **Implementation**
  - [ ] Implement `discoverByIdentityKey`
  - [ ] Implement `discoverByAttributes` with JSON field matching

- [ ] **Testing**
  - [ ] Unit tests for both methods
  - [ ] Test with encrypted fields (if applicable)

---

### Phase 5: Async Support

- [ ] **Research**
  - [ ] Review BRC-100 spec for `waitForAuthentication` (Call Code 24)
  - [ ] Review BRC-104 spec for `/.well-known/auth` (to understand difference)
  - [ ] Understand use case: wallet initialization vs wallet-to-app authentication
  - [ ] Review metanet-desktop TypeScript implementation
  - [ ] Determine if `isAuthenticated` needs to check wallet existence

- [ ] **Implementation**
  - [ ] Implement polling-based `waitForAuthentication`
  - [ ] Add timeout handling
  - [ ] (Optional) Enhance to WebSocket if needed

- [ ] **Testing**
  - [ ] Unit tests
  - [ ] Test timeout behavior

---

## Next Steps

### Immediate Actions (This Week)

1. **Update BRC-33 Status in Guide**
   - [ ] Update `BRC100_IMPLEMENTATION_GUIDE.md` to reflect BRC-33 is implemented
   - [ ] Note that database persistence is optional enhancement

2. **Research Phase**
   - [ ] Read BRC-52 spec completely (2-3 hours)
   - [ ] Read BRC-100 Group C methods (1-2 hours)
   - [ ] Review reference implementations (1-2 hours)
   - [ ] Answer research questions above

3. **Planning Refinement**
   - [ ] Update this document with research findings
   - [ ] Refine implementation estimates
   - [ ] Identify any missing dependencies

### Week 1: Start Implementation

1. **Phase 1: Output Management**
   - Start with `listOutputs` (simpler, more commonly used)
   - Then `relinquishOutput`

2. **Phase 2: Blockchain Queries**
   - Quick wins: `getNetwork`, `getHeight`, `getHeaderForHeight`

---

## Notes & Discoveries

*This section will be updated as we research and implement*

### 2025-12-XX: Planning Session
- Created comprehensive execution plan
- Identified 10 methods to implement
- Database schema is ready
- BRC-33 already implemented (just needs database persistence)

### 2025-12-XX: Spec Review Session

#### BRC-52 Identity Certificates - Key Findings

**Certificate Structure** (from [BRC-52 spec](https://bsv.brc.dev/peer-to-peer/0052)):
```json
{
  "type": "base64_encoded_certificate_type",
  "subject": "33-byte_compressed_public_key_hex",
  "validationKey": "base64_encoded_validation_key",
  "serialNumber": "base64_encoded_serial",
  "fields": {
    "field_name": "base64_encrypted_field_value"
  },
  "certifier": "33-byte_compressed_public_key_hex",
  "revocationOutpoint": "txid.vout",
  "signature": "DER_encoded_ECDSA_signature_hex",
  "keyring": {
    "field_name": "base64_encrypted_revelation_key"
  }
}
```

**Key Insights**:
1. **Privacy-First Design**: Fields are encrypted by default (BRC-2 encryption)
2. **Selective Disclosure**: `keyring` allows revealing specific fields without exposing all data
3. **UTXO-Based Revocation**: `revocationOutpoint` points to UTXO - if spent, certificate is revoked
4. **Signature Verification**: Certifier signs certificate data using ECDSA
5. **Field Encryption**: Uses BRC-2 (AES-GCM) with BRC-42 key derivation

**Implementation Requirements**:
- BRC-2 encryption/decryption module (for field decryption)
- ECDSA signature verification (for certifier validation)
- UTXO checking (for revocation verification)
- Selective disclosure key generation (for `proveCertificate`)

#### BRC-2 Encryption - Key Findings

**Purpose**: Data encryption/decryption for certificate fields
- Uses AES-GCM encryption
- Key derivation via BRC-42 (ECDH shared secret)
- Base64 encoding for encrypted data
- **Action**: Need to implement BRC-2 encryption module

#### BRC-34 Federation - Key Findings

**Purpose**: Multi-server federation for message relay
- Allows users on different servers to communicate
- May use WebSocket for real-time communication
- **Note**: WebSocket on port 3302 may be for BRC-34, not BRC-33
- **Action**: Review BRC-34 spec for WebSocket details

#### waitForAuthentication - Key Findings

**Purpose Clarification**:
- **NOT about authenticating with server/client** - that's `/.well-known/auth` (BRC-104)
- **IS about wallet initialization** - waits for wallet to be ready
- Checks if wallet exists and is set up (has keys, addresses, etc.)

**Implementation Approach** (from BRC-100 spec):
- Simple polling mechanism
- Polls `isAuthenticated` until it returns true
- No WebSocket required
- **Reference**: metanet-desktop TypeScript implementation (needs review)
- **Action**: Implement HTTP polling (1-second intervals, timeout after 5 minutes)

**Current Implementation Note**:
- `isAuthenticated` currently always returns `true` (stub)
- May need to check if wallet actually exists in database
- Should verify wallet has master key and at least one address

### Reference Implementations to Review

1. **metanet-desktop** (https://github.com/BSVArchie/metanet-desktop) ⭐ **PRIMARY REFERENCE**
   - TypeScript implementation using BSV/SDK
   - Certificate management (methods 17-22)
   - `waitForAuthentication` approach (Call Code 24)
   - Output management (`listOutputs`, `relinquishOutput`)
   - Blockchain queries (`getHeight`, `getHeaderForHeight`, `getNetwork`)
   - All Group C methods implemented

### Research Findings

#### 2025-12-XX: Part 1 Research Complete ✅

**listOutputs (Call Code 6)** - ✅ **RESEARCHED**:
- **Parameters**: basket (required), tags (optional), tagQueryMode ('all'/'any'), include, includeCustomInstructions, includeTags, includeLabels, limit, offset, seekPermission
- **Return Format**: `{ totalOutputs, outputs[], BEEF? }`
- **Output Object**: outpoint, satoshis, spendable (always true), lockingScript?, customInstructions?, tags?, labels?
- **Key Insights**:
  - Basket is required (no "all baskets" option)
  - Tags are for outputs, labels are for transactions
  - Can return locking scripts or entire transactions (BEEF)
  - All returned outputs are spendable (unspent UTXOs)

**relinquishOutput (Call Code 7)** - ✅ **RESEARCHED**:
- **Parameters**: basket (required), output (outpoint string "txid.vout")
- **Return Format**: `{ relinquished: true }`
- **Purpose**: Remove output from basket tracking (NOT spending it)
- **Key Insights**:
  - Simply sets `basket_id = NULL` in database
  - Output remains in wallet, just not tracked in that basket
  - No schema changes needed (basket_id is already nullable)

**Remaining Questions**:
- ✅ **Tag Storage**: Found in wallet-toolbox-rs reference! (separate tables, many-to-many)
- ✅ **BEEF Generation**: We have BEEF generation code! (`rust-wallet/src/beef.rs`)
- ✅ **Default Basket Name**: **NO DEFAULT BASKET** - basket is required parameter

**Tag Storage Research** ✅ (verified in ts-brc100 TypeScript SDK):
- **Storage Method**: Separate tables (many-to-many relationship)
- **Tables Needed**:
  1. `output_tags` table:
     - `output_tag_id` (PK)
     - `user_id` (or wallet_id in our case)
     - `tag` (tag name string, max 300 chars)
     - `created_at`, `updated_at`, `deleted_at` (soft delete)
  2. `output_tag_map` table (join table):
     - `output_id` (references utxos.id)
     - `output_tag_id` (references output_tags.id)
     - Composite unique key: `(output_id, output_tag_id)`
- **Query Pattern**:
  - To find outputs with tags: JOIN `utxos` → `output_tag_map` → `output_tags`
  - Filter by tag names: `WHERE tag IN ('tag1', 'tag2')`
  - Tag query mode 'all': All tags must match (INNER JOIN for each tag)
  - Tag query mode 'any': Any tag matches (OR conditions)
- **Implementation**: Need to add these tables to our database schema

**BEEF Generation Research** ✅ (from our codebase):
- **Location**: `rust-wallet/src/beef.rs`
- **Methods Available**:
  - `Beef::new()` - Create empty BEEF structure
  - `beef.add_parent_transaction(tx_bytes)` - Add parent transaction
  - `beef.set_main_transaction(tx_bytes)` - Set main transaction
  - `beef.to_bytes()` - Serialize to bytes
  - `beef.to_atomic_beef_hex(txid)` - Create Atomic BEEF (BRC-95)
- **Current Usage**: Used in `signAction` handler (lines 3008-3361 in `handlers.rs`)
- **Process**:
  1. Create `Beef::new()`
  2. Fetch parent transactions (from cache or API)
  3. Add each parent with `add_parent_transaction()`
  4. Set main transaction with `set_main_transaction()`
  5. Serialize with `to_bytes()` or `to_atomic_beef_hex()`
- **For `listOutputs` with `include='entire transactions'`**:
  - Need to build BEEF for each output's transaction
  - Fetch parent transactions for each output's inputs
  - Combine into single BEEF structure
  - Return as hex string in response

**Action**: Review metanet-desktop implementation for tag storage and default basket

---

**Last Updated**: 2025-12-08
**Status**: Part 1 & Part 2 Complete ✅
**Next Review**: Part 3 - Certificate Management
