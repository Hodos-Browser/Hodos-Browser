# BRC-100 Implementation Guide

> **Official Specification**: [BRC-100 Wallet Interface](https://bsv.brc.dev/wallet/0100)
> **Full Spec Document**: `reference/BRC100_spec.md`

## 🎯 Current Status: Database Migration Complete! 🎉

**Latest Achievement**: Complete database migration with backup/recovery and performance optimization!
**Previous Achievement**: BRC-29 payments working with ToolBSV - real-world tested ✅

**Current Focus**: All database phases complete (Phases 1-9). Browser database (Phase 8) deferred to separate sprint.

---

## 🗄️ **Database Migration Complete!**

**Status**: ✅ All wallet database phases complete (Phases 1-9)

**Achievement**: Successfully migrated from JSON file storage to SQLite database with full backup/recovery support.

**Key Features Implemented**:
- ✅ **SQLite Database** - Single-file database at `%APPDATA%/HodosBrowser/wallet/wallet.db`
- ✅ **UTXO Caching** - Eliminates API calls during transactions
- ✅ **BEEF/SPV Caching** - Parent transactions, Merkle proofs, and block headers cached
- ✅ **Background Sync** - Automatic UTXO and cache updates (every 5-10 minutes)
- ✅ **Performance Optimization** - Database indexes and in-memory balance cache
- ✅ **Backup & Recovery** - File-based backup, JSON export, and recovery from mnemonic
- ✅ **Schema Ready** - Baskets, certificates, and messages tables ready for future features

**Implementation Guide**: See `development-docs/DATABASE_IMPLEMENTATION_GUIDE.md` for complete details.

**Completed Phases**:
1. ✅ Database Foundation
2. ✅ Data Migration (JSON → SQLite)
3. ✅ Core Functionality Migration
4. ✅ UTXO Management & Caching
5. ✅ BEEF/SPV Caching
6. ✅ Performance Optimization
7. ✅ Backup & Recovery
8. ⏳ Browser Database (deferred to separate sprint)

---

## ⚠️ **BRC-33 Message Relay - IMPLEMENTED (Needs Real-World Testing)**

**Status**: ✅ Endpoints implemented (in-memory storage), ❓ Real-world testing unknown
**Note**: BRC-33 is **separate from BRC-100** but required by many apps

### BRC-33 Message Relay Endpoints

| Endpoint | Status | Real-World Test | Spec | Notes |
|----------|--------|-----------------|------|-------|
| `/sendMessage` | ✅ | ❓ Unknown | [BRC-33](https://bsv.brc.dev/peer-to-peer/0033) | Send messages to recipients - **Implemented, needs testing** |
| `/listMessages` | ✅ | ❓ Unknown | [BRC-33](https://bsv.brc.dev/peer-to-peer/0033) | List messages from inbox - **Implemented, needs testing** |
| `/acknowledgeMessage` | ✅ | ❓ Unknown | [BRC-33](https://bsv.brc.dev/peer-to-peer/0033) | Acknowledge received messages - **Implemented, needs testing** |

**Authentication**: Uses BRC-31 (Authrite) - same as `/.well-known/auth` ✅
**Storage**: Currently in-memory (`MessageStore`) - database persistence optional enhancement
**Implementation**: `rust-wallet/src/message_relay.rs` and handlers in `handlers.rs`

**Real-World Testing Needed**:
- Test with apps that use BRC-33 (Coinflip, Thryll, etc.)
- Verify message delivery and retrieval
- Test message acknowledgment flow
- Verify authentication works correctly

---

## 📋 BRC-100 Method Checklist (28 Methods)

### Legend:
- ✅ **Working** - Internal tests passing, real-world tested
- 🔧 **Implemented** - Code complete, needs testing
- ⚠️ **Partial** - Stubbed or incomplete implementation
- ❌ **Not Started** - No implementation yet

### Implementation Priority Groups:

#### **Group A: Core Identity & Authentication (Priority 1)** ✅ **COMPLETE!**
These are foundational - apps need these to identify and authenticate with the wallet.

| Call Code | Method | Status | Internal Test | Real-World Test | Notes |
|-----------|--------|--------|---------------|-----------------|-------|
| 28 | `getVersion` | ✅ | ✅ | ✅ | Returns wallet version info |
| 8 | `getPublicKey` | ✅ | ✅ | ✅ | Returns master public key |
| 23 | `isAuthenticated` | ✅ | ✅ | ✅ | Check auth status |
| 13 | `createHmac` | ✅ | ✅ | ✅ | Base64 keyID encoding + raw key for self |
| 14 | `verifyHmac` | ✅ | ✅ | ✅ | Base64 keyID encoding + raw key for self |
| 15 | `createSignature` | ✅ | ✅ | ✅ | Master key + BRC-42 + session validation |
| 16 | `verifySignature` | ✅ | ✅ | ✅ | **Derives signer's child public key!** |
| - | `/.well-known/auth` | ✅ | ✅ | ✅ | BRC-103/104 authentication |

**Status**: 🎉 **AUTHENTICATION COMPLETE!** All 7 Critical Breakthroughs:
1. ✅ 32-byte random nonces (was 48 bytes)
2. ✅ `/verifySignature` implemented (was stubbed)
3. ✅ Master key consistency (all operations use master key)
4. ✅ BRC-42 "self" counterparty (uses raw key per BRC-56)
5. ✅ KeyID base64 encoding (was corrupting binary data!)
6. ✅ **BRC-42 signature verification (derives signer's child public key!)**

**Real-World Testing**: ✅ ToolBSV fully functional with identity tokens, image/video history!

#### **Group B: Transaction Operations (Priority 2)** ✅ **COMPLETE!**
Once authenticated, apps need these to create and sign transactions.

| Call Code | Method | Status | Internal Test | Real-World Test | Notes |
|-----------|--------|--------|---------------|-----------------|-------|
| 1 | `createAction` | ✅ | ✅ | ✅ | **BRC-29 payment support added!** |
| 2 | `signAction` | ✅ | ✅ | ✅ | Atomic BEEF with TSC proofs working |
| 3 | `abortAction` | ✅ | ✅ | ❌ | Cancel pending/unconfirmed transactions |
| 4 | `listActions` | ✅ | ✅ | ❌ | List transaction history with filters |
| 5 | `internalizeAction` | ✅ | ✅ | ❌ | Accept incoming BEEF |

**Status**: ✅ **BRC-29 payments working with ToolBSV and real sites!**

---

## ✅ **RECENT BREAKTHROUGHS: Transaction System Complete!**

### Latest Achievement: BRC-29 Payments Working!
**Status**: Successfully completing payments with ToolBSV and other real BRC-100 sites!

### Key Implementations:

1. **Complete Transaction Creation (`createAction`)**
   - UTXO selection from WhatsOnChain
   - Fee calculation with dust limit handling
   - Multiple output support
   - Automatic change output generation
   - Action history storage

2. **BRC-29 Payment Protocol**
   - Automatic detection via `customInstructions`
   - BRC-42 key derivation for unique addresses
   - P2PKH script generation from derived public key
   - Privacy-preserving micropayments

3. **Transaction Signing (`signAction`)**
   - BSV ForkID SIGHASH (0x41)
   - Multi-input signing with correct private keys
   - Parent transaction fetching
   - TSC Merkle proof generation with block height resolution
   - Atomic BEEF (BRC-95) format

4. **BEEF & SPV Support**
   - Standard BEEF V2 format with parent transactions
   - BUMP (Block Unspent Merkle Proof) conversion
   - TSC to BUMP format conversion
   - Atomic BEEF wrapper (BRC-95)
   - Full SPV validation support

**See [Developer_notes.md](Developer_notes.md) for complete technical details and code examples.**

---

#### **Group C: Output/Basket & Certificate Management (Priority 3)**
For managing UTXOs, tracking digital assets, and identity certificates.

| Call Code | Method | Status | Internal Test | Real-World Test | Notes |
|-----------|--------|--------|---------------|-----------------|-------|
| 6 | `listOutputs` | ⏳ | ❌ | ❌ | **Database schema ready** - Will support basket filtering<br>⚠️ **Note**: Add `/listOutputs` to HTTP interceptor when implementing |
| 7 | `relinquishOutput` | ❌ | ❌ | ❌ | Release UTXO control<br>⚠️ **Note**: Add `/relinquishOutput` to HTTP interceptor when implementing |
| 17 | `acquireCertificate` | ⏳ | ❌ | ❌ | **Database schema ready** - `certificates` table added<br>⚠️ **Note**: Add `/acquireCertificate` to HTTP interceptor when implementing |
| 18 | `listCertificates` | ⏳ | ❌ | ❌ | **Database schema ready** - `certificates` table added<br>⚠️ **Note**: Add `/listCertificates` to HTTP interceptor when implementing |
| 19 | `proveCertificate` | ⏳ | ❌ | ❌ | **Database schema ready** - `certificates` table added<br>⚠️ **Note**: Add `/proveCertificate` to HTTP interceptor when implementing |
| 20 | `relinquishCertificate` | ⏳ | ❌ | ❌ | **Database schema ready** - `certificates` table added<br>⚠️ **Note**: Add `/relinquishCertificate` to HTTP interceptor when implementing |
| 21 | `discoverByIdentityKey` | ⏳ | ❌ | ❌ | **Database schema ready** - `certificates` table added<br>⚠️ **CRITICAL**: Add `/discoverByIdentityKey` to HTTP interceptor (microblog.bitspv.com uses this!) |
| 22 | `discoverByAttributes` | ⏳ | ❌ | ❌ | **Database schema ready** - `certificates` table added<br>⚠️ **Note**: Add `/discoverByAttributes` to HTTP interceptor when implementing |
| 24 | `waitForAuthentication` | ❌ | ❌ | ❌ | Async auth wait<br>✅ **Already in HTTP interceptor** |
| 25 | `getHeight` | ✅ | ✅ | ❌ | Get blockchain height<br>✅ **COMPLETE** - Fetches from WhatsOnChain `/chain/info`<br>✅ **Already in HTTP interceptor** |
| 26 | `getHeaderForHeight` | ✅ | ✅ | ❌ | Get block header by height<br>✅ **COMPLETE** - Cache-first with API fallback, constructs 80-byte header<br>✅ **Already in HTTP interceptor** |
| 27 | `getNetwork` | ✅ | ✅ | ❌ | Return "mainnet" or "testnet"<br>✅ **COMPLETE** - Returns hardcoded "mainnet"<br>✅ **Already in HTTP interceptor** |

**Database Support**: ✅ Schema includes `baskets`, `certificates`, `utxos`, `block_headers` tables - ready for implementation after database migration.

**⚠️ Important Notes**:
- **Database Migrations**: Migrations run automatically when the wallet starts (`WalletDatabase::new()`). After adding new tables/columns, restart the wallet to apply migrations.
- **HTTP Interceptor**: When implementing new endpoints, add them to `isWalletEndpoint()` in `cef-native/src/core/HttpRequestInterceptor.cpp` to ensure requests are intercepted.
- **Migration Safety**: Migrations use `CREATE TABLE IF NOT EXISTS` and `ALTER TABLE` with existence checks - they won't overwrite existing data.

#### **Group D: Encryption & Advanced Crypto (Priority 4)**
Privacy features and advanced cryptography.

| Call Code | Method | Status | Internal Test | Real-World Test | Notes |
|-----------|--------|--------|---------------|-----------------|-------|
| 9 | `revealCounterpartyKeyLinkage` | ❌ | ❌ | ❌ | BRC-69 counterparty key linkage |
| 10 | `revealSpecificKeyLinkage` | ❌ | ❌ | ❌ | BRC-69 specific key linkage |
| 11 | `encrypt` | ❌ | ❌ | ❌ | BRC-2 encryption |
| 12 | `decrypt` | ❌ | ❌ | ❌ | BRC-2 decryption |

#### **Group E: Specialized Features (Priority 5)**
Advanced wallet features for specific use cases.

*Note: Some of these call codes are not in the standard BRC-100 spec (codes 1-28)*

| Call Code | Method | Status | Internal Test | Real-World Test | Notes |
|-----------|--------|--------|---------------|-----------------|-------|
| ? | `getTransactionWithOutputs` | ❌ | ❌ | ❌ | Get full transaction data (non-standard?) |
| ? | `getSpendingLimits` | ❌ | ❌ | ❌ | Query spending limits (non-standard?) |
| ? | `getProtocolRestrictions` | ❌ | ❌ | ❌ | Query protocol permissions (non-standard?) |
| ? | `getBasketRestrictions` | ❌ | ❌ | ❌ | Query basket permissions (non-standard?) |

---

## 🎯 Implementation Strategy

### ~~Phase 1: Fix Authentication~~ ✅ **COMPLETE!** (Oct 22-23)
**Goal**: ✅ Get `verifySignature` and BRC-104 authentication working with ToolBSV.

**Completed Tasks**:
1. ✅ **Implemented `/verifySignature`** - Full BRC-3 compliant verification
2. ✅ **Fixed BRC-104 Auth** - All 7 breakthroughs implemented
3. ✅ **Tested with ToolBSV** - Complete authentication handshake working
4. ✅ **Documented Solution** - All breakthroughs documented in Developer_notes.md

**Success Criteria - ALL MET**:
- ✅ ToolBSV frontend accepts our signatures
- ✅ Complete BRC-104 mutual authentication
- ✅ Internal signature verification tests passing
- ✅ Real-world testing: identity tokens, image/video history working!

### ~~Phase 2: Core Transaction Methods~~ ✅ **COMPLETE!** (Oct 27-30)
**Goal**: ✅ Complete transaction lifecycle support.

**Completed Tasks**:
1. ✅ **Transaction Creation** - UTXO selection, fee calculation, BRC-29 support
2. ✅ **Transaction Signing** - BSV ForkID SIGHASH, parent transaction fetching
3. ✅ **Atomic BEEF Generation** - Standard BEEF + Atomic BEEF (BRC-95) format
4. ✅ **TSC Merkle Proofs** - Automatic fetching and BUMP conversion
5. ✅ **BRC-29 Payments** - Automatic detection and script derivation
6. ✅ **Action History** - Complete transaction tracking with metadata
7. ✅ **Real-World Testing** - ToolBSV payments working successfully!

**Success Criteria - ALL MET**:
- ✅ Can create and sign transactions
- ✅ Full transaction history tracking
- ✅ Atomic BEEF format correct with SPV proofs
- ✅ BRC-29 payment protocol working
- ✅ Real-world testing: ToolBSV payments complete successfully!

### ~~Phase 3: Database Migration & UTXO Management~~ ✅ **COMPLETE!** (2025-12-06)
**Goal**: ✅ Migrate to SQLite database and implement UTXO caching.

**Database Migration**:
- ✅ Complete database schema design (15 tables)
- ✅ Database foundation implementation
- ✅ JSON → SQLite data migration
- ✅ Core functionality migration

**UTXO Management**:
- ✅ UTXO caching and sync service (background sync every 5 minutes)
- ✅ Balance calculation from database cache
- ✅ UTXO spending tracking
- ✅ New address detection (pending cache)
- ⏳ `listOutputs` - List available UTXOs (database ready, endpoint pending)
- ⏳ `relinquishOutput` - Release UTXO control (database ready, endpoint pending)

**BEEF/SPV Caching**:
- ✅ Parent transaction caching
- ✅ Merkle proof caching (TSC/BUMP format)
- ✅ Block header caching
- ✅ Background cache sync (every 10 minutes)
- ✅ Cache-first transaction signing

**Performance Optimization**:
- ✅ Database indexes (schema v4)
- ✅ In-memory balance cache (30-second TTL)
- ✅ Query optimization

**Backup & Recovery**:
- ✅ File-based backup (database + WAL + SHM)
- ✅ JSON export (non-sensitive data)
- ✅ Recovery from mnemonic
- ✅ Restore functionality

**Basket Implementation**:
- ✅ Database schema ready (`baskets` table, `basket_id` in `utxos`)
- ⏳ Basket assignment logic (pending)
- ⏳ Basket queries and filtering (pending)

**Success Criteria - ALL MET**:
- ✅ Database migration complete
- ✅ UTXO caching working (no API calls during transactions)
- ✅ BEEF building uses cached data
- ✅ Performance optimized with indexes and caching
- ✅ Backup and recovery systems operational

**See**: `development-docs/DATABASE_IMPLEMENTATION_GUIDE.md` for complete details.

### Phase 4: Certificates & Identity (Week 4)
**Goal**: Complete identity certificate system.

**Methods to Implement**:
- `acquireCertificate` - BRC-52 certificate acquisition
- `listCertificates` - List identity certificates
- `proveCertificate` - Prove certificate ownership
- `relinquishCertificate` - Release certificates

**Success Criteria**:
- ✅ BRC-52 certificate support
- ✅ Identity management complete
- ✅ Certificate verification working

### Phase 5: Encryption & Advanced Features (Week 5+)
**Goal**: Complete remaining methods.

**Methods to Implement**:
- `encrypt` / `decrypt` - BRC-2 encryption
- `waitForAuthentication` - Async auth support
- `getTransactionWithOutputs` - Full transaction retrieval
- Key linkage revelation methods
- Permission/restriction queries

---

## 🤔 Understanding the Authentication Flow

### Key Questions We Need to Answer:

1. **What is `verifySignature` actually for?**
   - Is it part of mutual authentication?
   - When would an app call this endpoint?
   - Do we need it for ToolBSV authentication?

2. **What is mutual authentication in BRC-103/104?**
   - Who verifies whose signature?
   - When does each party verify the other?
   - What signatures are being created and verified?

3. **Why is ToolBSV rejecting our signature?**
   - What are we signing incorrectly?
   - What format does ToolBSV expect?
   - Are we using the right key derivation?

4. **What's the difference between:**
   - `createSignature` (Call Code 32) - We create a signature
   - `verifySignature` (Call Code 33) - We verify someone else's signature
   - Signature in `/.well-known/auth` - We sign nonces for authentication

### Documentation We Need to Study:

**Priority 1 (Read NOW)**:
- [ ] **BRC-103: Mutual Authentication** - Understand the full auth flow
- [ ] **BRC-104: HTTP Transport** - Understand `/.well-known/auth` endpoint
- [ ] **BRC-3: Digital Signatures** - Understand signature creation/verification

**Priority 2 (Read SOON)**:
- [ ] **BRC-42: BKDS** - Review our key derivation implementation
- [ ] **BRC-43: Security Levels** - Understand invoice number format
- [ ] **BRC-77: Signature Format** - DER vs Compact signatures

**Reference Implementations**:
- [ ] TypeScript SDK `Peer.ts` - See how they verify signatures
- [ ] BSV Go SDK - See signature verification implementation

---

## 🔍 **BRC-33 Message Relay System - Deep Dive**

### What is BRC-33?
**[BRC-33 PeerServ Message Relay](https://bsv.brc.dev/peer-to-peer/0033)** is a **message inbox system** that allows:
- Asynchronous peer-to-peer communication
- Message delivery when parties are offline
- Message boxes (like email inboxes) for different purposes

### Why Do Apps Need It?
Apps like Coinflip and Thryll use message boxes to:
- Communicate with backends asynchronously
- Store game state, payment requests, notifications
- Enable peer-to-peer interactions without direct connections

### How is it Different from BRC-100?
| BRC-100 | BRC-33 |
|---------|--------|
| Wallet-to-App interface | Peer-to-peer message relay |
| Transaction signing, keys | Message inbox/delivery |
| 28 standardized methods | 3 message endpoints |
| Required for wallets | Optional add-on service |

---

## 📋 **BRC-33 Implementation Questions**

### Critical Questions to Answer:

1. **Storage**:
   - ❓ Where do we store messages? In-memory? File? Database?
   - ❓ How long do we keep messages?
   - ❓ What happens when storage fills up?

2. **Authentication**:
   - ✅ Uses BRC-31 (Authrite) - we already have this working!
   - ✅ Each message box is tied to an identity key
   - ❓ Do we need to verify sender signatures?

3. **Message Boxes**:
   - ❓ Are message boxes pre-created or created on-demand?
   - ❓ Examples we've seen: `coinflip_inbox`, `payment_inbox`
   - ❓ Can users create custom message box names?

4. **Federation (BRC-34/35)**:
   - ❓ Do we need to implement federation NOW?
   - ❓ Is this what the port 3302 WebSocket server is for?
   - ❓ Can we start without federation and add it later?

5. **WebSocket vs HTTP**:
   - ✅ BRC-33 core endpoints are **HTTP POST** (port 3301)
   - ❓ Is WebSocket (port 3302) for real-time notifications?
   - ❓ Do apps poll `/listMessages` or get push notifications?

6. **Implementation**:
   - ✅ BRC-33 uses **HTTP POST**, not WebSocket
   - ✅ **Decision**: Implement BRC-33 in Rust wallet (port 3301) as HTTP POST endpoints
   - ❓ WebSocket (port 3302?) for real-time push notifications (optional enhancement)

---

## 📚 **Required Reading (BRC-33 Message Relay)**

### Must Read (In Order):

1. **[BRC-31: Authrite Mutual Authentication](https://bsv.brc.dev/peer-to-peer/0031)** ✅ DONE
   - We already implemented this for `/.well-known/auth`
   - BRC-33 uses the same authentication mechanism

2. **[BRC-33: PeerServ Message Relay](https://bsv.brc.dev/peer-to-peer/0033)** 🔴 CRITICAL
   - **Specification**: 3 endpoints (`/sendMessage`, `/listMessages`, `/acknowledgeMessage`)
   - **Request/Response formats**: JSON structures for each endpoint
   - **Message Authenticity**: Optional signature verification
   - **Limitations**: "Not for long-term storage, only transport"

3. **[BRC-34: PeerServ Host Interconnect (CHIP)](https://bsv.brc.dev/peer-to-peer/0034)** 🟡 OPTIONAL (for now)
   - Federation between message relay servers
   - Allows users on different servers to communicate
   - **Question**: Is this what port 3302 is for?

4. **[BRC-35: Confederacy Lookup Availability Protocol (CLAP)](https://bsv.brc.dev/peer-to-peer/0035)** 🟡 OPTIONAL
   - Service discovery for federated servers
   - **Can skip for now** - focus on local implementation first

5. **[BRC-77: Message Signature Creation](https://bsv.brc.dev/peer-to-peer/0077)** 🟡 OPTIONAL
   - For signing message contents (not just HTTP auth)
   - **May not be needed** for basic implementation

---

## 🏗️ **BRC-33 Implementation Architecture**

### Option 1: Rust Wallet Handles Everything (Port 3301) ⭐ RECOMMENDED
```
App → HTTP POST → Rust Wallet (3301)
                  ├─ BRC-100 endpoints (✅ working)
                  ├─ /.well-known/auth (✅ working)
                  └─ BRC-33 message relay (❌ add these)
```

**Pros**: Simple, all in one place, Rust performance
**Cons**: Need to implement storage system


### Option 3: CEF C++ Backend (Port 3302)
```
App → HTTP POST → C++ Backend (3302) → Message Storage
```

**Pros**: Centralized message handling
**Cons**: C++ implementation, separate from wallet

---

## 🛠️ **Recommended Implementation Plan**

### Phase 0: BRC-33 Message Relay (URGENT - Week 1)

**Goal**: Get Coinflip and Thryll working by implementing message inbox system.

#### Step 1: Study BRC-33 Spec (Today - 2 hours)
- [ ] Read [BRC-33](https://bsv.brc.dev/peer-to-peer/0033) completely
- [ ] Understand request/response formats for all 3 endpoints
- [ ] Check reference TS wallet implementation in `reference/ts-brc100/`
- [ ] Answer our "Critical Questions" above

#### Step 2: Design Storage System (Today - 1 hour)
- [ ] Decide: In-memory HashMap? JSON file? SQLite?
- [ ] Message structure: `{ messageId, sender, messageBox, body, timestamp }`
- [ ] Start simple: In-memory HashMap, persist to JSON later

#### Step 3: Implement 3 Endpoints (Tomorrow - 4 hours)
```rust
// rust-wallet/src/handlers.rs

/// BRC-33: Send a message to a recipient's message box
pub async fn send_message(
    data: web::Data<AppState>,
    req: web::Json<SendMessageRequest>,
) -> impl Responder {
    // 1. Authenticate sender (already done by middleware)
    // 2. Store message in recipient's message box
    // 3. Return success
}

/// BRC-33: List messages from message box
pub async fn list_messages(
    data: web::Data<AppState>,
    req: web::Json<ListMessagesRequest>,
) -> impl Responder {
    // 1. Authenticate caller (already done)
    // 2. Retrieve messages from their message box
    // 3. Return message array
}

/// BRC-33: Acknowledge (delete) messages
pub async fn acknowledge_message(
    data: web::Data<AppState>,
    req: web::Json<AckMessageRequest>,
) -> impl Responder {
    // 1. Authenticate caller
    // 2. Delete specified messages
    // 3. Return success
}
```

#### Step 4: Test with Coinflip (Tomorrow afternoon)
- [ ] Start Rust wallet with new endpoints
- [ ] Test Coinflip - should see 200 responses for `/listMessages`
- [ ] Verify message flow works end-to-end

---

## 📚 Documentation References

### Peer-to-Peer Message Relay (NEW!):
- **[BRC-31: Authrite Authentication](https://bsv.brc.dev/peer-to-peer/0031)** - Authentication layer (✅ implemented)
- **[BRC-33: PeerServ Message Relay](https://bsv.brc.dev/peer-to-peer/0033)** - 3 message endpoints (❌ blocking apps!)
- **[BRC-34: CHIP Federation](https://bsv.brc.dev/peer-to-peer/0034)** - Multi-server federation (optional)
- **[BRC-77: Message Signatures](https://bsv.brc.dev/peer-to-peer/0077)** - Message authenticity (optional)

### Core Specifications:
- **[BRC-100: Wallet Interface](https://bsv.brc.dev/wallet/0100)** - Main specification (28 methods)
- **[BRC-3: Digital Signatures](https://bsv.brc.dev/transactions/0003)** - Message signing (`createSignature`, `verifySignature`)
- **[BRC-2: Encryption](https://bsv.brc.dev/transactions/0002)** - Data encryption/decryption
- **[BRC-42: Key Derivation (BKDS)](https://bsv.brc.dev/key-derivation/0042)** - BSV Key Derivation Scheme
- **[BRC-43: Security Levels](https://bsv.brc.dev/key-derivation/0043)** - Protocol IDs and security levels
- **[BRC-52: Identity Certificates](https://bsv.brc.dev/peer-to-peer/0052)** - Certificate management
- **[BRC-69: Key Linkage](https://bsv.brc.dev/key-derivation/0069)** - Revealing key relationships
- **[BRC-84: Linked Keys](https://bsv.brc.dev/key-derivation/0084)** - Linked key derivation
- **[BRC-103: Mutual Authentication](https://bsv.brc.dev/peer-to-peer/0103)** - P2P authentication protocol
- **[BRC-104: HTTP Transport](https://bsv.brc.dev/peer-to-peer/0104)** - HTTP transport for BRC-103

### Transaction Formats:
- **[BRC-62: BEEF Transactions](https://bsv.brc.dev/transactions/0062)** - Background Evaluation Extended Format
- **[BRC-67: SPV](https://bsv.brc.dev/transactions/0067)** - Simplified Payment Verification
- **[BRC-8: Transaction Format](https://bsv.brc.dev/transactions/0008)** - Raw transaction format

### Testing Resources:
- **[ToolBSV.com](https://toolbsv.com)** - Real-world testing site
- **[Thryll.online](https://thryll.online)** - BRC-100 compliant app

### Reference Implementations:
- **TypeScript SDK**: `reference/ts-brc100/` - Reference implementation
- **BSV Go SDK**: `github.com/bsv-blockchain/go-sdk` - Official Go SDK

---

## 🔍 Current Implementation Analysis

### ✅ What's Working (Rust Wallet):

**Transaction System** (Group B):
- `createAction` - Creates unsigned transactions with UTXO selection
- `signAction` - Signs transactions using BSV ForkID SIGHASH
- `processAction` - Complete flow: create → sign → broadcast
- **2 confirmed mainnet transactions!** 🎉

**Partial Authentication** (Group A):
- `getVersion` - Returns wallet version
- `getPublicKey` - Returns master public key
- `createHmac` - HMAC creation using master private key
- `verifyHmac` - HMAC verification

### ❌ What's Broken (Current Blocker):

**Authentication Issues** (Group A):
- `verifySignature` - **CRITICAL** - Not implemented, causing ToolBSV to fail
- `/.well-known/auth` - Signature verification failing
- `isAuthenticated` - Not tested

**Root Cause**: Signature verification in BRC-104 authentication flow.

### 📂 Key Implementation Files (Rust):

```
rust-wallet/src/
├── main.rs              # Actix-web server, route definitions
├── handlers.rs          # ALL BRC-100 endpoint handlers (2171 lines)
├── json_storage.rs      # wallet.json management
├── crypto/
│   ├── brc42.rs        # BRC-42 key derivation (ECDH-based)
│   └── brc43.rs        # BRC-43 invoice number formatting
├── transaction/
│   ├── types.rs        # Transaction structures
│   └── sighash.rs      # BSV ForkID SIGHASH (working!)
├── utxo_fetcher.rs     # WhatsOnChain UTXO fetching
└── domain_whitelist.rs # Domain whitelisting system
```

---

## 🎯 Next Steps (Immediate)

### 1. **Understand the Authentication Flow** (Today - Before Coding!)
   - Read BRC-103 (Mutual Authentication) - understand WHO verifies WHOSE signature
   - Read BRC-104 (HTTP Transport) - understand `/.well-known/auth` flow
   - Read BRC-3 (Digital Signatures) - understand signature format
   - Answer our key questions above ☝️

### 2. **Fix Our Signature Creation in `/.well-known/auth`** (Today)
   - This is where the bug is - not in `verifySignature` endpoint!
   - Add detailed logging of our signing process
   - Compare with TypeScript SDK's signature creation
   - Verify we're signing the right data in the right format
   - Test with ToolBSV

### 3. **Clarify `verifySignature` Purpose** (Tomorrow)
   - Understand when apps call this endpoint
   - Is it needed for authentication? (probably not)
   - Is it a utility for apps to verify signatures?
   - Decide if we need to implement it now or later

### 4. **Create Test Suite** (After Auth Works)
   - Unit tests for signature creation
   - Test ECDH shared secret calculation
   - Test nonce concatenation
   - Integration test with ToolBSV

---

## 📝 Method Implementation Template

When implementing each method, follow this structure:

```rust
// File: rust-wallet/src/handlers.rs

/// BRC-100 Call Code XX: methodName
/// Specification: https://bsv.brc.dev/wallet/0100#methodname
///
/// Description: [What this method does]
///
/// Parameters:
/// - param1: Description
/// - param2: Description
///
/// Returns:
/// - Success: { ... }
/// - Error: { code, description }
pub async fn method_name(
    data: web::Data<AppState>,
    req: web::Json<MethodRequest>,
) -> impl Responder {
    // 1. Validate parameters
    // 2. Perform operation
    // 3. Return result in BRC-100 format
    // 4. Log for debugging
}

// Unit test
#[cfg(test)]
mod tests {
    #[test]
    fn test_method_name_success() {
        // Test successful case
    }

    #[test]
    fn test_method_name_error() {
        // Test error cases
    }
}
```

---

## 🚀 Long-Term Plan

### Milestone 1: Authentication Complete (Week 1)
- All Group A methods working
- ToolBSV authentication successful
- Internal tests passing

### Milestone 2: Transaction Complete (Week 2)
- All Group B methods working
- Transaction history tracking
- Payment internalization

### Milestone 3: Output Management (Week 3)
- All Group C methods working
- UTXO tracking complete
- Blockchain queries working

### Milestone 4: Full BRC-100 Support (Week 5)
- All 28 methods implemented
- All internal tests passing
- Real-world testing with multiple apps
- Documentation complete


---

## 🔄 Documentation to Consolidate/Remove

### Keep (Updated):
- ✅ `Developer_notes.md` - Current session notes (streamlined!)
- ✅ `BRC100_IMPLEMENTATION_GUIDE.md` - **THIS FILE** (new, comprehensive)
- ✅ `RUST_WALLET_SESSION_SUMMARY.md` - Technical details of BSV SIGHASH breakthrough

### Archive (Move to `reference/` or remove):
- 📦 `BRC100_IMPLEMENTATION_PLAN.md` - Superseded by this guide
- 📦 `BRC100_WALLET_INTEGRATION_PLAN.md` - Merged into this guide
- 📦 `RUST_TRANSACTION_IMPLEMENTATION_PLAN.md` - Complete, archive for reference
- 📦 `BRC-100` (file) - Basic overview, can remove

---

## ⚠️ Testing Strategy & Concerns

### Internal Testing Problem: Confirmation Bias in Tests

**Problem**: When implementing to a protocol specification, there's a risk of writing tests that have the same fundamental misunderstandings as the implementation code. This is sometimes called:
- **"Confirmation bias in testing"** - Tests confirm our (potentially incorrect) understanding
- **"Self-validating tests"** - Tests pass even when implementation is wrong
- **"Circular validation"** - Tests mirror implementation bugs

**Example**: If we misunderstand how BEEF format should work, we might:
1. Implement BEEF generation incorrectly
2. Write tests that expect the incorrect format
3. Tests pass, but real-world apps fail

**Our Approach**:
- ✅ **Real-world testing first** - Test with actual BRC-100 apps (ToolBSV, Thryll, etc.)
- ⏳ **Internal tests deferred** - Will add comprehensive unit tests after consulting with protocol developers
- 📋 **Documentation** - Keep detailed notes on what works in real-world scenarios
- 🔍 **Reference implementation** - Compare against `ts-brc100` TypeScript SDK

**Status**: Internal test suite is minimal. Focus is on real-world compatibility testing.

**Challenges**:
- Limited real-world BRC-100 apps available for testing
- Need to validate understanding with protocol developers before writing comprehensive tests
- Real-world testing is slower but more reliable than potentially flawed internal tests

**Next Steps**:
- Consult with BRC-100 protocol developers to validate our understanding
- Build comprehensive test suite based on confirmed understanding
- Use real-world app testing as primary validation method

---

**Last Updated**: December 8, 2025
**Current Focus**: Group C - Output/Basket & Certificate Management
**Progress**:
- ✅ Part 1: Output Management (`listOutputs`, `relinquishOutput`) - **COMPLETE**
- ✅ Part 2: Blockchain Queries (`getHeight`, `getHeaderForHeight`, `getNetwork`) - **COMPLETE**
- ⏳ Part 3: Certificate Management - **NEXT**
**Next Milestone**: Complete Part 3 (Certificate Management)
