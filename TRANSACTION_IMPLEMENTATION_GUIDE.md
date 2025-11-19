# Transaction & Payment Implementation Guide

> **Purpose**: Complete guide for implementing BRC-100 Group B (Transaction Management) in the Rust wallet

## 📋 Table of Contents

1. [Current Status](#current-status)
2. [Core Concepts](#core-concepts)
3. [Critical Specifications](#critical-specifications)
4. [Group B Methods Overview](#group-b-methods-overview)
5. [Implementation Plan](#implementation-plan)
6. [Technical Deep Dive](#technical-deep-dive)
7. [Testing Strategy](#testing-strategy)

---

## 🎯 Current Status

### ✅ **GROUP B COMPLETE!** (October 27, 2025)

**All Group B endpoints have been implemented and tested:**

#### 1. `abortAction` (Call Code 4) ✅
- Cancels pending or unconfirmed transactions
- Updates status to `Aborted`
- Prevents aborting confirmed transactions
- Full error handling for edge cases

#### 2. `listActions` (Call Code 5) ✅
- Returns transaction history with full filtering
- Supports label filtering (any/all modes)
- Pagination with offset/limit
- Optional field inclusion (labels, inputs, outputs)
- Confirmation status tracking

#### 3. `internalizeAction` (Call Code 18) ✅
- Accepts incoming BEEF transactions
- Parses raw transactions as fallback
- Detects output ownership
- Calculates received amounts
- Stores with full metadata

#### 4. Complete Transaction Lifecycle ✅
- **`createAction`** (Call Code 2) - Creates unsigned transactions with UTXO selection
- **`signAction`** (Call Code 3) - Signs transactions using BSV ForkID SIGHASH
- **`processAction`** (Call Code 17) - Complete flow: create → sign → broadcast
- **Confirmed mainnet transactions**: Multiple successful broadcasts to BSV network

### 🎉 What's Now Available:
- ✅ Full transaction lifecycle tracking
- ✅ Transaction cancellation (abort)
- ✅ Receiving and tracking incoming payments
- ✅ Action storage system (JSON-based)
- ✅ BEEF Phase 2 parser with output ownership detection
- ✅ Transaction history with filtering and pagination

### 🧪 Testing Status:
- **Internal Tests**: All passing ✅
- **Integration Tests**: Complete test suite in `rust-wallet/test_*.ps1` ✅
- **Real-World Testing**: Ready for apps like ToolBSV and Thryll ⏳

### 🎯 Next Goals:
Move to Group C (Output/Basket Management) to implement:
- `listOutputs` - List available UTXOs
- `relinquishOutput` - Release UTXO control
- `getHeight` / `getHeaderForHeight` - Blockchain queries
- `getNetwork` - Network identification

---

## 🧠 Core Concepts

### 1. **What is BEEF?** (BRC-62)

**BEEF = Background Evaluation Extended Format**

**Key Concept**: When apps send transactions, they don't just send the raw transaction. They send:
- The transaction itself
- **ALL parent transactions** (the full ancestry/dependency chain)
- Merkle proofs for confirmed ancestors

**Why BEEF Exists:**
```
Traditional Problem:
App creates TX → Sends to wallet → Wallet needs parent TXs to validate inputs

BEEF Solution:
App packages TX + ALL parents → Wallet can validate independently → No database lookups
```

**BEEF Structure:**
```
BEEF Package
├── Version byte (0x0100BEEF)
├── BUMPs (Merkle proofs for confirmed TXs)
├── Transactions array
│   ├── Parent TX 1
│   ├── Parent TX 2
│   └── New TX (references parents)
```

**Critical for Implementation:**
- `internalizeAction` receives BEEF from apps
- Your wallet must **parse** BEEF format
- Your wallet must **validate** transaction ancestry
- Your wallet must **store** transactions and their relationships

### 2. **What is SPV?** (BRC-67)

**SPV = Simplified Payment Verification**

**Key Concept**: Prove a transaction is in a block **WITHOUT downloading the entire blockchain**.

**How SPV Works:**
```
Block Header (80 bytes)
├── Previous block hash
├── Merkle root ← THIS IS THE KEY!
├── Timestamp
└── Other metadata

Merkle Proof (compact)
├── Transaction hash
├── Path to merkle root (siblings only)
└── Validates TX is in block
```

**SPV Verification Process:**
1. Take transaction hash
2. Use merkle proof siblings to compute path
3. Final result must match block header's merkle root
4. If match → TX is confirmed in that block!

**Critical for Implementation:**
- `listActions` needs to show "confirmed" vs "unconfirmed"
- Confirmed = has valid SPV proof + sufficient block depth
- Your wallet must **verify Merkle proofs**
- Your wallet must **track block headers** (or query them)

### 3. **Transaction Lifecycle States**

```
┌─────────────────────────────────────────────────────────┐
│                  Transaction States                      │
└─────────────────────────────────────────────────────────┘

1. CREATED (unsigned)
   ↓ signAction()
2. SIGNED (not broadcast)
   ↓ broadcast
3. UNCONFIRMED (in mempool)
   ↓ mined into block
4. PENDING (1-5 confirmations)
   ↓ sufficient depth
5. CONFIRMED (6+ confirmations)

Alternative paths:
- CREATED → abortAction() → CANCELLED
- SIGNED → abortAction() → CANCELLED
- UNCONFIRMED → double-spend → FAILED
```

**Implementation Requirement:**
- Track transaction state in storage
- Update state based on blockchain events
- Show current state in `listActions`

### 4. **Actions vs Transactions**

**Important Distinction:**

| Actions (BRC-100) | Transactions (Bitcoin) |
|-------------------|------------------------|
| User-facing concept | Technical concept |
| Can have metadata (labels, description) | Raw bytes only |
| Tracked in wallet | Tracked on blockchain |
| Has lifecycle (created/signed/confirmed) | Binary (confirmed or not) |
| `listActions` returns these | Raw TXs are inside actions |

**Example Action Object:**
```json
{
  "txid": "7dce601f...",
  "rawTx": "0100000001...",
  "description": "Payment to merchant",
  "labels": ["shopping", "online"],
  "version": 1,
  "lockTime": 0,
  "inputs": [...],
  "outputs": [...],
  "referenceNumber": "abc123",
  "status": "unconfirmed",
  "timestamp": 1698765432
}
```

---

## 📚 Critical Specifications

### Priority 1: MUST READ (Core Understanding)

#### [BRC-62: BEEF Transactions](https://bsv.brc.dev/transactions/0062) 🔥
**What**: Background Evaluation Extended Format
**Why Critical**:
- `internalizeAction` receives transactions in BEEF format
- Apps package transactions with full ancestry
- You must parse and validate BEEF

**Key Sections:**
- BEEF binary format specification
- BUMP (Merkle proof) encoding
- Transaction ordering and validation
- Atomic BEEF (multi-party transactions)

**Implementation Impact:**
- Need BEEF parser in Rust
- Need ancestry validation logic
- Need to extract transactions from BEEF packages

#### [BRC-67: SPV Verification](https://bsv.brc.dev/transactions/0067) 🔥
**What**: Simplified Payment Verification
**Why Critical**:
- Determine if transactions are confirmed
- `listActions` must show confirmation status
- Validate Merkle proofs for security

**Key Sections:**
- Merkle tree structure
- Proof verification algorithm
- Block header validation
- Confirmation depth requirements

**Implementation Impact:**
- Need Merkle proof verification in Rust
- Need block header queries (WhatsOnChain API)
- Need confirmation counting logic

#### [BRC-8: Raw Transaction Format](https://bsv.brc.dev/transactions/0008)
**What**: Bitcoin transaction binary structure
**Why Important**:
- Foundation for understanding TXs
- You're already using this in signing

**Key Sections:**
- Transaction version, inputs, outputs, locktime
- Input script structure
- Output script structure
- Serialization format

**Current Status**: ✅ Already implemented (you're signing transactions)

#### [BRC-9: TXO Transaction Object Format](https://bsv.brc.dev/transactions/0009)
**What**: JSON representation of transactions for BRC-100
**Why Critical**:
- `listActions` returns this format
- `createAction` works with this format
- Standard way to represent TXs in BRC-100

**Key Sections:**
- TXO JSON structure
- Input/output representation
- Metadata fields
- Conversion between raw hex and TXO

**Implementation Impact:**
- Need TXO serialization/deserialization in Rust
- Need to store actions as TXO format
- Need to convert between raw TX and TXO

---

### Priority 2: Supporting Specifications

#### [BRC-10: Merkle Proof Format](https://bsv.brc.dev/transactions/0010)
**What**: Standardized merkle proof JSON format
**Use**: Parsing merkle proofs from blockchain APIs

#### [BRC-30: Merkle Path JSON](https://bsv.brc.dev/transactions/0030)
**What**: JSON representation of merkle paths
**Use**: Working with SPV proofs in JSON

#### [BRC-74: BUMP Format](https://bsv.brc.dev/transactions/0074)
**What**: BSV Unified Merkle Path - binary merkle proof format
**Use**: BEEF uses BUMP for encoding proofs

#### [BRC-76: Graph Aware Sync Protocol](https://bsv.brc.dev/transactions/0076)
**What**: Synchronizing transaction graphs
**Use**: Advanced - for syncing transaction history

---

### Priority 3: Payment Protocols (Defer for Now)

These are for **app-to-app** payments, not core wallet functionality:

- **[BRC-27: Direct Payment Protocol](https://bsv.brc.dev/payments/0027)** - Merchant invoices
- **[BRC-28: Paymail](https://bsv.brc.dev/payments/0028)** - Paymail addressing
- **[BRC-29: Simple P2PKH Payment](https://bsv.brc.dev/payments/0029)** - Basic payments
- **[BRC-54: Hybrid Payment Mode](https://bsv.brc.dev/payments/0054)** - Multi-asset payments
- **[BRC-70: Paymail BEEF](https://bsv.brc.dev/payments/0070)** - BEEF via paymail

**When You'll Need These:** When implementing merchant payment flows, paymail support, or app-to-app payment protocols. Not required for core BRC-100 wallet functionality.

---

## 📋 Group B Methods Overview

### Call Code 2: `createAction` ✅ WORKING

**Purpose**: Create unsigned transaction

**Current Implementation:**
- UTXO selection from WhatsOnChain
- Transaction construction with inputs/outputs
- Fee calculation
- Returns TXO format

**What's Working:**
```rust
// rust-wallet/src/handlers.rs
pub async fn create_action(req: CreateActionRequest) -> CreateActionResponse {
    // 1. Fetch UTXOs from WhatsOnChain
    // 2. Select UTXOs to cover outputs + fee
    // 3. Build transaction structure
    // 4. Return unsigned transaction in TXO format
}
```

**No Changes Needed** - Already production-ready!

---

### Call Code 3: `signAction` ✅ WORKING

**Purpose**: Sign transaction inputs

**Current Implementation:**
- BSV ForkID SIGHASH algorithm
- ECDSA signing with private keys
- Multi-input signing
- Returns signed transaction

**What's Working:**
```rust
// rust-wallet/src/transaction/sighash.rs
pub fn sign_transaction(tx: Transaction, privkey: PrivateKey) -> Transaction {
    // 1. Calculate SIGHASH for each input (BSV ForkID)
    // 2. Sign with private key
    // 3. Insert signature into input script
    // 4. Return fully signed transaction
}
```

**No Changes Needed** - Confirmed working on mainnet!

---

### Call Code 4: `abortAction` ❌ TO IMPLEMENT

**Purpose**: Cancel a pending transaction before broadcast or if unconfirmed

**Specification**: [BRC-100 abortAction](https://bsv.brc.dev/wallet/0100)

**Request:**
```json
{
  "referenceNumber": "abc123"
}
```

**Response:**
```json
{
  "aborted": true
}
```

**Implementation Requirements:**

1. **Find Action by Reference Number**
   - Search action storage for matching referenceNumber
   - Return error if not found

2. **Check if Abortable**
   - Can abort: CREATED, SIGNED states
   - Cannot abort: CONFIRMED state
   - UNCONFIRMED: Attempt RBF (Replace-By-Fee) or just mark as aborted

3. **Update State**
   - Mark action as "aborted" or "cancelled"
   - Remove from pending transactions list
   - Optionally: attempt double-spend to cancel unconfirmed TX

4. **Return Confirmation**
   - Return `{"aborted": true}`

**Complexity**: 🟢 Low (mainly database update)

**Implementation Steps:**
```rust
pub async fn abort_action(
    data: web::Data<AppState>,
    req: web::Json<AbortActionRequest>,
) -> impl Responder {
    // 1. Load action by referenceNumber
    // 2. Check current state
    // 3. If CREATED/SIGNED: mark as aborted
    // 4. If UNCONFIRMED: mark as aborted (optionally RBF)
    // 5. If CONFIRMED: return error "cannot abort confirmed TX"
    // 6. Return success
}
```

---

### Call Code 5: `listActions` ❌ TO IMPLEMENT

**Purpose**: List transaction history with filtering and pagination

**Specification**: [BRC-100 listActions](https://bsv.brc.dev/wallet/0100)

**Request:**
```json
{
  "labels": ["shopping"],
  "labelQueryMode": "any",
  "includeLabels": true,
  "includeInputs": true,
  "includeInputSourceLockingScripts": false,
  "includeInputUnlockingScripts": false,
  "includeOutputs": true,
  "includeOutputLockingScripts": false,
  "limit": 25,
  "offset": 0
}
```

**Response:**
```json
{
  "totalActions": 142,
  "actions": [
    {
      "txid": "7dce601f...",
      "satoshis": 50000,
      "status": "confirmed",
      "isOutgoing": true,
      "description": "Payment to merchant",
      "labels": ["shopping"],
      "version": 1,
      "lockTime": 0,
      "inputs": [...],
      "outputs": [...],
      "referenceNumber": "abc123"
    }
  ]
}
```

**Implementation Requirements:**

1. **Storage System**
   - Need persistent action storage (JSON file or SQLite?)
   - Schema: txid, referenceNumber, description, labels, status, timestamp, rawTx, inputs, outputs

2. **Query Filtering**
   - Filter by labels (AND/OR mode)
   - Filter by date range
   - Pagination (limit/offset)

3. **Status Determination**
   - Query blockchain API for confirmation status
   - Calculate confirmations (current height - tx block height)
   - Classify: "unconfirmed", "pending", "confirmed"

4. **Field Inclusion Logic**
   - Conditionally include inputs/outputs based on flags
   - Include/exclude locking/unlocking scripts
   - Optimize response size

5. **SPV Verification** (Optional but Recommended)
   - Verify Merkle proofs for confirmed transactions
   - Show verification status

**Complexity**: 🟡 Medium (requires storage + blockchain queries)

**Implementation Steps:**
```rust
pub async fn list_actions(
    data: web::Data<AppState>,
    req: web::Json<ListActionsRequest>,
) -> impl Responder {
    // 1. Load all actions from storage
    // 2. Filter by labels (if specified)
    // 3. For each action:
    //    - Query confirmation status from blockchain API
    //    - Determine status (unconfirmed/pending/confirmed)
    // 4. Apply pagination (offset/limit)
    // 5. Build response with requested fields
    // 6. Return actions array + totalActions
}
```

**Key Challenge**: Need to implement action storage system first!

---

### Call Code 17: `processAction` ✅ PARTIALLY WORKING

**Purpose**: Complete flow - create, sign, and broadcast

**Current Status:** Working but may need updates for action tracking

**Enhancement Needed:**
- After broadcast, store action in action storage
- Track transaction status
- Update status as confirmations arrive

---

### Call Code 18: `internalizeAction` ❌ TO IMPLEMENT

**Purpose**: Accept incoming payment (receive transaction from another party)

**Specification**: [BRC-100 internalizeAction](https://bsv.brc.dev/wallet/0100)

**Request:**
```json
{
  "tx": "beef0100...",  // BEEF format
  "outputs": [
    {
      "outputIndex": 1,
      "protocol": "wallet",
      "paymentRemittance": {
        "derivationPrefix": "m/0/1/2",
        "senderIdentityKey": "02abc..."
      }
    }
  ],
  "description": "Payment from Alice",
  "labels": ["payment", "alice"]
}
```

**Response:**
```json
{
  "txid": "7dce601f...",
  "status": "unconfirmed"
}
```

**Implementation Requirements:**

1. **Parse BEEF Format**
   - Extract BEEF version
   - Extract BUMP (Merkle proof) data
   - Extract transaction array
   - Find the main transaction (last in array)

2. **Validate Transaction Ancestry**
   - Verify all parent transactions are present
   - Validate parent-child relationships
   - Verify no missing inputs

3. **Validate Merkle Proofs** (if present)
   - For confirmed parent TXs, verify BUMP proofs
   - Ensure proofs match block headers

4. **Identify Our Outputs**
   - Check which outputs belong to our wallet
   - Match against our addresses/scripts
   - Or use provided `outputs` array

5. **Store Transaction**
   - Save to action storage
   - Mark as incoming transaction
   - Store description and labels
   - Add outputs to UTXO pool

6. **Broadcast (if needed)**
   - If transaction is unconfirmed, broadcast to network
   - If already confirmed, just store

**Complexity**: 🔴 High (BEEF parsing + validation)

**Implementation Steps:**
```rust
pub async fn internalize_action(
    data: web::Data<AppState>,
    req: web::Json<InternalizeActionRequest>,
) -> impl Responder {
    // 1. Parse BEEF format from req.tx
    // 2. Extract main transaction
    // 3. Validate transaction ancestry
    // 4. Verify Merkle proofs (if present)
    // 5. Identify outputs that belong to us
    // 6. Store transaction in action storage
    // 7. Add outputs to UTXO pool
    // 8. Broadcast if unconfirmed
    // 9. Return txid and status
}
```

**Key Challenge**: Need BEEF parser library or implement BEEF parsing!

---

## 🚀 Implementation Plan

### Phase 0: Prerequisites (1-2 days)

**Read Documentation:**
- [ ] Read [BRC-62: BEEF](https://bsv.brc.dev/transactions/0062) - Understand BEEF format
- [ ] Read [BRC-67: SPV](https://bsv.brc.dev/transactions/0067) - Understand Merkle proofs
- [ ] Read [BRC-9: TXO Format](https://bsv.brc.dev/transactions/0009) - Understand action representation
- [ ] Review [BRC-100 Group B](https://bsv.brc.dev/wallet/0100) - Understand method requirements

**Design Decisions:**
- [ ] Decide on action storage format (JSON file? SQLite? In-memory?)
- [ ] Design action schema (fields, indexes, relationships)
- [ ] Decide on BEEF parsing approach (library vs manual)
- [ ] Plan SPV verification strategy (local headers? API queries?)

---

### Phase 1: Action Storage System (2-3 days)

**Goal**: Persistent storage for transaction actions

**Tasks:**

1. **Design Action Schema**
```rust
// rust-wallet/src/action_storage.rs

pub struct StoredAction {
    pub txid: String,
    pub reference_number: String,
    pub raw_tx: String,
    pub description: Option<String>,
    pub labels: Vec<String>,
    pub status: ActionStatus,  // Created, Signed, Unconfirmed, Confirmed, Aborted
    pub is_outgoing: bool,
    pub satoshis: i64,
    pub timestamp: i64,
    pub block_height: Option<u32>,
    pub confirmations: u32,

    // TXO format fields
    pub version: u32,
    pub lock_time: u32,
    pub inputs: Vec<ActionInput>,
    pub outputs: Vec<ActionOutput>,
}

pub enum ActionStatus {
    Created,
    Signed,
    Unconfirmed,
    Pending,  // 1-5 confirmations
    Confirmed,  // 6+ confirmations
    Aborted,
    Failed,
}
```

2. **Implement Storage Backend**
   - Option A: JSON file (`%APPDATA%/BabbageBrowser/actions.json`)
   - Option B: SQLite database (`%APPDATA%/BabbageBrowser/actions.db`)
   - **Recommendation**: Start with JSON, migrate to SQLite later

3. **Implement CRUD Operations**
```rust
pub struct ActionStorage {
    file_path: PathBuf,
    actions: HashMap<String, StoredAction>,  // key = txid
}

impl ActionStorage {
    pub fn load() -> Result<Self>;
    pub fn save(&self) -> Result<()>;
    pub fn add_action(&mut self, action: StoredAction) -> Result<()>;
    pub fn get_action(&self, txid: &str) -> Option<&StoredAction>;
    pub fn get_action_by_reference(&self, ref_num: &str) -> Option<&StoredAction>;
    pub fn update_status(&mut self, txid: &str, status: ActionStatus) -> Result<()>;
    pub fn list_actions(&self, filter: ActionFilter) -> Vec<&StoredAction>;
    pub fn delete_action(&mut self, txid: &str) -> Result<()>;
}
```

4. **Update Existing Methods**
   - Modify `createAction` to store action
   - Modify `signAction` to update action status
   - Modify `processAction` to update action after broadcast

**Success Criteria:**
- Actions persist across wallet restarts
- Can query actions by txid or reference number
- Can filter actions by labels

---

### Phase 2: Implement `abortAction` (1 day)

**Goal**: Allow cancellation of pending transactions

**Implementation:**
```rust
// rust-wallet/src/handlers.rs

#[derive(Deserialize)]
pub struct AbortActionRequest {
    pub reference_number: String,
}

#[derive(Serialize)]
pub struct AbortActionResponse {
    pub aborted: bool,
}

pub async fn abort_action(
    data: web::Data<AppState>,
    req: web::Json<AbortActionRequest>,
) -> impl Responder {
    log::info!("📋 abortAction called for: {}", req.reference_number);

    // 1. Load action storage
    let mut storage = data.action_storage.lock().unwrap();

    // 2. Find action by reference number
    let action = match storage.get_action_by_reference(&req.reference_number) {
        Some(a) => a,
        None => {
            return HttpResponse::NotFound().json(json!({
                "status": "error",
                "code": "ERR_ACTION_NOT_FOUND",
                "description": "Action not found"
            }));
        }
    };

    // 3. Check if action can be aborted
    match action.status {
        ActionStatus::Confirmed => {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "code": "ERR_CANNOT_ABORT_CONFIRMED",
                "description": "Cannot abort confirmed transaction"
            }));
        }
        ActionStatus::Aborted => {
            return HttpResponse::Ok().json(AbortActionResponse { aborted: true });
        }
        _ => {}
    }

    // 4. Update status to aborted
    storage.update_status(&action.txid, ActionStatus::Aborted).unwrap();
    storage.save().unwrap();

    log::info!("✅ Action aborted: {}", action.txid);

    HttpResponse::Ok().json(AbortActionResponse { aborted: true })
}
```

**Testing:**
- Create action
- Abort before signing
- Verify status is "aborted"
- Try aborting confirmed TX (should fail)

---

### Phase 3: Implement `listActions` (2-3 days)

**Goal**: Return transaction history with filtering

**Implementation:**
```rust
// rust-wallet/src/handlers.rs

#[derive(Deserialize)]
pub struct ListActionsRequest {
    pub labels: Option<Vec<String>>,
    pub label_query_mode: Option<String>,  // "any" or "all"
    pub include_labels: Option<bool>,
    pub include_inputs: Option<bool>,
    pub include_input_source_locking_scripts: Option<bool>,
    pub include_input_unlocking_scripts: Option<bool>,
    pub include_outputs: Option<bool>,
    pub include_output_locking_scripts: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Serialize)]
pub struct ListActionsResponse {
    pub total_actions: usize,
    pub actions: Vec<ActionSummary>,
}

pub async fn list_actions(
    data: web::Data<AppState>,
    req: web::Json<ListActionsRequest>,
) -> impl Responder {
    log::info!("📋 listActions called");

    // 1. Load action storage
    let storage = data.action_storage.lock().unwrap();

    // 2. Filter by labels
    let mut actions: Vec<_> = storage.list_actions(ActionFilter {
        labels: req.labels.clone(),
        label_mode: req.label_query_mode.as_deref(),
    });

    // 3. Update statuses from blockchain
    for action in &mut actions {
        update_action_status(action).await;
    }

    // 4. Apply pagination
    let total = actions.len();
    let offset = req.offset.unwrap_or(0);
    let limit = req.limit.unwrap_or(25);
    let actions: Vec<_> = actions.into_iter()
        .skip(offset)
        .take(limit)
        .collect();

    // 5. Build response with requested fields
    let actions: Vec<_> = actions.into_iter()
        .map(|a| build_action_summary(a, &req))
        .collect();

    HttpResponse::Ok().json(ListActionsResponse {
        total_actions: total,
        actions,
    })
}

async fn update_action_status(action: &mut StoredAction) {
    // Query WhatsOnChain for confirmation status
    if action.status == ActionStatus::Unconfirmed ||
       action.status == ActionStatus::Pending {

        // TODO: Query blockchain API
        // - Get current block height
        // - Get TX block height (if confirmed)
        // - Calculate confirmations
        // - Update status
    }
}
```

**Key Components:**

1. **Label Filtering**
```rust
fn filter_by_labels(
    actions: &[StoredAction],
    labels: &[String],
    mode: &str,
) -> Vec<&StoredAction> {
    match mode {
        "any" => actions.iter()
            .filter(|a| a.labels.iter().any(|l| labels.contains(l)))
            .collect(),
        "all" => actions.iter()
            .filter(|a| labels.iter().all(|l| a.labels.contains(l)))
            .collect(),
        _ => actions.iter().collect(),
    }
}
```

2. **Confirmation Status Query**
```rust
async fn get_confirmation_status(txid: &str) -> Result<ConfirmationStatus> {
    // Query WhatsOnChain API
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/out", txid);
    let response = reqwest::get(&url).await?;

    if response.status().is_success() {
        let data: serde_json::Value = response.json().await?;
        let confirmations = data["confirmations"].as_u64().unwrap_or(0);

        Ok(ConfirmationStatus {
            confirmed: confirmations > 0,
            confirmations: confirmations as u32,
            block_height: data["blockheight"].as_u64().map(|h| h as u32),
        })
    } else {
        Ok(ConfirmationStatus {
            confirmed: false,
            confirmations: 0,
            block_height: None,
        })
    }
}
```

**Testing:**
- Create multiple actions with different labels
- Query all actions
- Filter by labels (any/all mode)
- Test pagination
- Verify confirmation status updates

---

### Phase 4: BEEF Parser (3-4 days)

**Goal**: Parse BEEF format for `internalizeAction`

**Option A: Use Existing Library**
- Check if BSV Rust libraries have BEEF parsing
- `bsv` crate on crates.io?
- Community implementations?

**Option B: Implement BEEF Parser**

**BEEF Format Overview:**
```
BEEF Binary Format:
┌─────────────────────────────────────┐
│ Version (4 bytes)    | 0x0100BEEF   │
├─────────────────────────────────────┤
│ BUMPs Array                          │
│ ├─ Block Height (VarInt)             │
│ ├─ Merkle Root (32 bytes)            │
│ ├─ Transaction Count (VarInt)        │
│ └─ Merkle Path (variable)            │
├─────────────────────────────────────┤
│ Transactions Array                   │
│ ├─ Transaction 1 (parent)            │
│ ├─ Transaction 2 (parent)            │
│ └─ Transaction N (main)              │
└─────────────────────────────────────┘
```

**Implementation:**
```rust
// rust-wallet/src/beef/parser.rs

pub struct BEEFPackage {
    pub version: u32,
    pub bumps: Vec<BUMP>,
    pub transactions: Vec<Transaction>,
}

pub struct BUMP {
    pub block_height: u64,
    pub merkle_root: [u8; 32],
    pub tree_height: u8,
    pub path: Vec<([u8; 32], bool)>,  // (hash, is_left)
}

impl BEEFPackage {
    pub fn parse(beef_hex: &str) -> Result<Self> {
        let bytes = hex::decode(beef_hex)?;
        let mut cursor = Cursor::new(bytes);

        // 1. Read version
        let version = cursor.read_u32::<LittleEndian>()?;
        if version != 0x0100BEEF {
            return Err("Invalid BEEF version");
        }

        // 2. Read BUMPs array
        let bumps = Self::read_bumps(&mut cursor)?;

        // 3. Read transactions array
        let transactions = Self::read_transactions(&mut cursor)?;

        Ok(BEEFPackage {
            version,
            bumps,
            transactions,
        })
    }

    fn read_bumps(cursor: &mut Cursor<Vec<u8>>) -> Result<Vec<BUMP>> {
        let count = read_varint(cursor)?;
        let mut bumps = Vec::new();

        for _ in 0..count {
            bumps.push(BUMP::parse(cursor)?);
        }

        Ok(bumps)
    }

    fn read_transactions(cursor: &mut Cursor<Vec<u8>>) -> Result<Vec<Transaction>> {
        let count = read_varint(cursor)?;
        let mut transactions = Vec::new();

        for _ in 0..count {
            transactions.push(Transaction::parse(cursor)?);
        }

        Ok(transactions)
    }
}
```

**Testing:**
- Parse sample BEEF from test vectors
- Validate transaction extraction
- Test with real BEEF from apps

---

### Phase 5: SPV Verification (2-3 days)

**Goal**: Verify Merkle proofs for confirmed transactions

**Implementation:**
```rust
// rust-wallet/src/spv/verifier.rs

pub struct MerkleProof {
    pub txid: [u8; 32],
    pub merkle_root: [u8; 32],
    pub path: Vec<([u8; 32], bool)>,  // (hash, is_left)
}

impl MerkleProof {
    pub fn verify(&self) -> bool {
        let mut current_hash = self.txid;

        // Walk up the merkle tree
        for (sibling, is_left) in &self.path {
            current_hash = if *is_left {
                // Sibling is on left, we're on right
                merkle_parent(&sibling, &current_hash)
            } else {
                // Sibling is on right, we're on left
                merkle_parent(&current_hash, sibling)
            };
        }

        // Final hash should match merkle root
        current_hash == self.merkle_root
    }
}

fn merkle_parent(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    // Concatenate and double SHA256
    let mut combined = Vec::with_capacity(64);
    combined.extend_from_slice(left);
    combined.extend_from_slice(right);

    let hash1 = Sha256::digest(&combined);
    let hash2 = Sha256::digest(&hash1);

    let mut result = [0u8; 32];
    result.copy_from_slice(&hash2);
    result
}
```

**Testing:**
- Create test merkle proofs
- Verify valid proofs return true
- Verify invalid proofs return false
- Test with real blockchain data

---

### Phase 6: Implement `internalizeAction` (3-4 days)

**Goal**: Accept incoming BEEF transactions

**Implementation:**
```rust
// rust-wallet/src/handlers.rs

#[derive(Deserialize)]
pub struct InternalizeActionRequest {
    pub tx: String,  // BEEF hex
    pub outputs: Vec<OutputToRedeem>,
    pub description: Option<String>,
    pub labels: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct InternalizeActionResponse {
    pub txid: String,
    pub status: String,
}

pub async fn internalize_action(
    data: web::Data<AppState>,
    req: web::Json<InternalizeActionRequest>,
) -> impl Responder {
    log::info!("📥 internalizeAction called");

    // 1. Parse BEEF
    let beef = match BEEFPackage::parse(&req.tx) {
        Ok(b) => b,
        Err(e) => {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "code": "ERR_INVALID_BEEF",
                "description": format!("Failed to parse BEEF: {}", e)
            }));
        }
    };

    // 2. Extract main transaction (last in array)
    let tx = beef.transactions.last().unwrap();
    let txid = tx.txid();

    // 3. Validate transaction ancestry
    if !validate_ancestry(&beef) {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "code": "ERR_INVALID_ANCESTRY",
            "description": "Transaction ancestry validation failed"
        }));
    }

    // 4. Verify Merkle proofs for confirmed parents
    for bump in &beef.bumps {
        if !bump.verify() {
            log::warn!("⚠️ Merkle proof verification failed");
        }
    }

    // 5. Identify our outputs
    let our_outputs = identify_our_outputs(tx, &req.outputs);

    // 6. Store action
    let mut storage = data.action_storage.lock().unwrap();
    storage.add_action(StoredAction {
        txid: hex::encode(txid),
        reference_number: generate_reference(),
        raw_tx: hex::encode(tx.serialize()),
        description: req.description.clone(),
        labels: req.labels.clone().unwrap_or_default(),
        status: ActionStatus::Unconfirmed,
        is_outgoing: false,
        satoshis: our_outputs.iter().map(|o| o.value).sum(),
        timestamp: Utc::now().timestamp(),
        block_height: None,
        confirmations: 0,
        version: tx.version,
        lock_time: tx.lock_time,
        inputs: convert_inputs(&tx.inputs),
        outputs: convert_outputs(&tx.outputs),
    }).unwrap();
    storage.save().unwrap();

    // 7. Add outputs to UTXO pool
    let mut utxo_manager = data.utxo_manager.lock().unwrap();
    for output in our_outputs {
        utxo_manager.add_utxo(output);
    }

    // 8. Broadcast if unconfirmed
    if !is_confirmed(&beef) {
        broadcast_transaction(tx).await?;
    }

    log::info!("✅ Action internalized: {}", hex::encode(txid));

    HttpResponse::Ok().json(InternalizeActionResponse {
        txid: hex::encode(txid),
        status: "unconfirmed".to_string(),
    })
}
```

**Key Functions:**

1. **Validate Ancestry**
```rust
fn validate_ancestry(beef: &BEEFPackage) -> bool {
    // Check that all input transactions are present
    let tx_map: HashMap<_, _> = beef.transactions.iter()
        .map(|tx| (tx.txid(), tx))
        .collect();

    for tx in &beef.transactions {
        for input in &tx.inputs {
            if !tx_map.contains_key(&input.previous_output.txid) {
                // Parent transaction missing
                return false;
            }
        }
    }

    true
}
```

2. **Identify Our Outputs**
```rust
fn identify_our_outputs(
    tx: &Transaction,
    output_hints: &[OutputToRedeem],
) -> Vec<UTXO> {
    let mut our_outputs = Vec::new();

    for hint in output_hints {
        if let Some(output) = tx.outputs.get(hint.output_index) {
            // Verify we can unlock this output
            if can_unlock_output(output) {
                our_outputs.push(UTXO {
                    txid: tx.txid(),
                    vout: hint.output_index as u32,
                    value: output.value,
                    script: output.script_pubkey.clone(),
                });
            }
        }
    }

    our_outputs
}
```

**Testing:**
- Create test BEEF packages
- Test with valid ancestry
- Test with missing parent transactions
- Test Merkle proof verification
- Test output identification

---

## 🧪 Testing Strategy

### Unit Tests

**Action Storage:**
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_action_storage_persistence() {
        let mut storage = ActionStorage::new_temp();
        storage.add_action(create_test_action());
        storage.save().unwrap();

        let loaded = ActionStorage::load().unwrap();
        assert_eq!(loaded.actions.len(), 1);
    }

    #[test]
    fn test_filter_by_labels_any() {
        let storage = create_test_storage();
        let results = storage.list_actions(ActionFilter {
            labels: vec!["shopping".to_string()],
            label_mode: "any",
        });
        assert_eq!(results.len(), 2);
    }
}
```

**BEEF Parser:**
```rust
#[test]
fn test_beef_parsing() {
    let beef_hex = "0100beef...";
    let beef = BEEFPackage::parse(beef_hex).unwrap();

    assert_eq!(beef.version, 0x0100BEEF);
    assert_eq!(beef.transactions.len(), 3);
}

#[test]
fn test_ancestry_validation() {
    let beef = create_test_beef_with_parents();
    assert!(validate_ancestry(&beef));

    let beef_missing_parent = create_test_beef_missing_parent();
    assert!(!validate_ancestry(&beef_missing_parent));
}
```

**SPV Verification:**
```rust
#[test]
fn test_merkle_proof_verification() {
    let proof = create_test_merkle_proof();
    assert!(proof.verify());

    let invalid_proof = create_invalid_merkle_proof();
    assert!(!invalid_proof.verify());
}
```

### Integration Tests

**End-to-End Transaction Flow:**
```rust
#[tokio::test]
async fn test_complete_transaction_flow() {
    // 1. Create action
    let create_req = CreateActionRequest { /* ... */ };
    let create_resp = create_action(create_req).await;

    // 2. Sign action
    let sign_req = SignActionRequest {
        reference_number: create_resp.reference_number.clone(),
    };
    let sign_resp = sign_action(sign_req).await;

    // 3. List actions (should see unconfirmed)
    let list_resp = list_actions(ListActionsRequest::default()).await;
    assert_eq!(list_resp.actions[0].status, "unconfirmed");

    // 4. Abort action
    let abort_req = AbortActionRequest {
        reference_number: create_resp.reference_number,
    };
    let abort_resp = abort_action(abort_req).await;
    assert!(abort_resp.aborted);

    // 5. List actions (should see aborted)
    let list_resp = list_actions(ListActionsRequest::default()).await;
    assert_eq!(list_resp.actions[0].status, "aborted");
}
```

**Receive Payment Flow:**
```rust
#[tokio::test]
async fn test_receive_payment() {
    // 1. Create BEEF package
    let beef = create_test_beef();

    // 2. Internalize action
    let req = InternalizeActionRequest {
        tx: beef.to_hex(),
        outputs: vec![OutputToRedeem { output_index: 1 }],
        description: Some("Test payment".to_string()),
        labels: Some(vec!["test".to_string()]),
    };
    let resp = internalize_action(req).await;

    // 3. Verify action is stored
    let list_resp = list_actions(ListActionsRequest::default()).await;
    assert_eq!(list_resp.total_actions, 1);
    assert_eq!(list_resp.actions[0].is_outgoing, false);

    // 4. Verify UTXO is added
    let utxos = list_utxos().await;
    assert!(utxos.iter().any(|u| u.txid == resp.txid));
}
```

### Real-World Testing

**Test Sites:**
1. **ToolBSV** - Test sending transactions
2. **Your Test Website** - Create custom payment flows
3. **Internal Tools** - Build testing utilities

**Test Scenarios:**
- Create and sign transaction
- Abort before broadcast
- Send transaction and track confirmations
- Receive payment from external source
- Filter transaction history
- Handle failed transactions

---

## 📊 Success Criteria

### Phase 1 Complete:
- [ ] Actions persist across restarts
- [ ] Can create, read, update actions
- [ ] Action storage tests passing

### Phase 2 Complete:
- [ ] Can abort created/signed actions
- [ ] Cannot abort confirmed actions
- [ ] Abort updates action status correctly

### Phase 3 Complete:
- [ ] Can list all actions
- [ ] Can filter by labels (any/all mode)
- [ ] Pagination works correctly
- [ ] Confirmation status updates automatically

### Phase 4 Complete:
- [ ] Can parse BEEF format
- [ ] Can extract transactions from BEEF
- [ ] Ancestry validation works

### Phase 5 Complete:
- [ ] Can verify Merkle proofs
- [ ] SPV verification tests passing
- [ ] Integration with blockchain APIs

### Phase 6 Complete:
- [ ] Can receive incoming payments
- [ ] BEEF transactions stored correctly
- [ ] Outputs added to UTXO pool
- [ ] Works with real apps

### Final Success:
- [ ] All Group B methods implemented
- [ ] All unit tests passing
- [ ] All integration tests passing
- [ ] Real-world testing successful
- [ ] Documentation complete

---

## 📝 Next Steps

### Immediate (Today):
1. ✅ Read [BRC-62: BEEF](https://bsv.brc.dev/transactions/0062)
2. ✅ Read [BRC-67: SPV](https://bsv.brc.dev/transactions/0067)
3. ✅ Read [BRC-9: TXO Format](https://bsv.brc.dev/transactions/0009)

### This Week:
1. Design action storage schema
2. Implement action storage system
3. Implement `abortAction`
4. Start `listActions` implementation

### Next Week:
1. Complete `listActions`
2. Start BEEF parser
3. Start SPV verification

### Following Week:
1. Complete BEEF parser
2. Complete SPV verification
3. Implement `internalizeAction`
4. Integration testing

---

## 🔗 Quick Reference Links

### Core Specifications:
- [BRC-100: Wallet Interface](https://bsv.brc.dev/wallet/0100) - Main specification
- [BRC-62: BEEF Transactions](https://bsv.brc.dev/transactions/0062) - Transaction packaging
- [BRC-67: SPV Verification](https://bsv.brc.dev/transactions/0067) - Transaction verification
- [BRC-8: Raw Transaction Format](https://bsv.brc.dev/transactions/0008) - Bitcoin transaction structure
- [BRC-9: TXO Format](https://bsv.brc.dev/transactions/0009) - BRC-100 transaction representation

### Supporting Specifications:
- [BRC-10: Merkle Proof Format](https://bsv.brc.dev/transactions/0010)
- [BRC-30: Merkle Path JSON](https://bsv.brc.dev/transactions/0030)
- [BRC-74: BUMP Format](https://bsv.brc.dev/transactions/0074)
- [BRC-76: Graph Aware Sync](https://bsv.brc.dev/transactions/0076)

### Payment Protocols (Future):
- [BRC-27: Direct Payment Protocol](https://bsv.brc.dev/payments/0027)
- [BRC-28: Paymail](https://bsv.brc.dev/payments/0028)
- [BRC-54: Hybrid Payment Mode](https://bsv.brc.dev/payments/0054)

---

**Last Updated**: October 27, 2025
**Current Focus**: Group B Transaction Management
**Next Milestone**: Complete action storage and `abortAction`
