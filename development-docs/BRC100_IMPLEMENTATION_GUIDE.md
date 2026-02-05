# BRC-100 Implementation Guide

> **Official Specification**: [BRC-100 Wallet Interface](https://bsv.brc.dev/wallet/0100)
> **Full Spec Document**: `reference/BRC100_spec.md`

## Current Status: 26/28 BRC-100 Methods Implemented (93%)

**Latest Achievement**: Basket & tag system complete with SDK-style optimistic locking, background cleanup, and output description storage (V14 migration).
**Previous Achievement**: `discoverByAttributes` (Call Code 22) - Certificate attribute search complete.

**Current Focus**: Real-world testing of tag functionality and internalizeAction basket insertion. Only 2 BRC-69 methods remaining (low priority).

---

## Database Migration Complete

**Status**: All wallet database phases complete (Phases 1-9), schema at V14.

**Key Features Implemented**:
- **SQLite Database** - Single-file database at `%APPDATA%/HodosBrowser/wallet/wallet.db`
- **UTXO Caching** - Eliminates API calls during transactions
- **BEEF/SPV Caching** - Parent transactions, Merkle proofs, and block headers cached
- **Background Sync** - Automatic UTXO and cache updates (every 5 minutes)
- **Background Cleanup** - Periodic stale reservation recovery, failed broadcast restoration, pending tx timeout
- **Performance Optimization** - Database indexes and in-memory balance cache
- **Backup & Recovery** - File-based backup, JSON export, and recovery from mnemonic
- **Basket & Tag System** - Full BRC-100 basket/tag output tracking with SDK-style optimistic locking
- **Output Description** - `outputDescription` field stored and returned via `listOutputs`

**Implementation Guide**: See `development-docs/DATABASE_IMPLEMENTATION_GUIDE.md` for complete details.

**Schema Migrations** (in `rust-wallet/src/database/migrations.rs`):

| Version | Content |
|---------|---------|
| V1 | Foundation: wallets, addresses, utxos (with basket_id FK), baskets, parent_transactions, merkle_proofs, block_headers |
| V2-V4 | Data migration, UTXO management, indexes |
| V5 | custom_instructions column on utxos |
| V6 | output_tags table, output_tag_map table, soft-delete support |
| V7-V8 | Certificates, backup |
| V9 | UTXO status column, basket partial index, basket/tag normalization |
| V10 | broadcast_status on transactions (pending/broadcast/confirmed/failed) |
| V11 | Transaction tracking enhancements |
| V12 | Nullable address_id on utxos (for basket outputs without HD wallet addresses) |
| V13 | Persistent derived key cache (PushDrop signing across restarts) |
| V14 | output_description column on utxos (BRC-100 output description storage) |

---

## ✅ **BRC-33 Message Relay - COMPLETE (Core Implementation)**

**Status**: ✅ Core implementation complete with SQLite persistence
**Note**: BRC-33 is **separate from BRC-100** but required by many apps

### BRC-33 Message Relay Endpoints

| Endpoint | Status | Spec | Notes |
|----------|--------|------|-------|
| `/sendMessage` | ✅ Complete | [BRC-33](https://bsv.brc.dev/peer-to-peer/0033) | Send messages to recipients |
| `/listMessages` | ✅ Complete | [BRC-33](https://bsv.brc.dev/peer-to-peer/0033) | List messages from inbox |
| `/acknowledgeMessage` | ✅ Complete | [BRC-33](https://bsv.brc.dev/peer-to-peer/0033) | Acknowledge and delete messages |

**Authentication**: Uses BRC-31 (Authrite) - same as `/.well-known/auth` ✅
**Storage**: SQLite persistence (`relay_messages` table) ✅
**Implementation**: `rust-wallet/src/message_relay.rs` and handlers in `handlers.rs`

### Implementation Details

| Feature | Status | Notes |
|---------|--------|-------|
| HTTP endpoints | ✅ Complete | All 3 endpoints working |
| SQLite persistence | ✅ Complete | `relay_messages` table with indexes |
| Message expiry | ✅ Complete | Auto-cleanup of expired messages |
| `MessageRelayRepository` | ✅ Complete | Full CRUD operations |
| WebSocket push | ❌ Not implemented | Optional - polling works for now (server-side feature) |
| End-to-end encryption | ✅ Available | BRC-2 `/encrypt` `/decrypt` endpoints work - apps use them before/after relay |
| Federation (BRC-34/35) | ❌ Not implemented | Server-side feature - not needed for client wallet |

### Remaining Work (Optional Enhancements)
- **WebSocket/Socket.IO**: Server-side feature for real-time push (we're a client, not a relay server)
- **Federation**: Server-side multi-relay infrastructure (BRC-34/35)

### Clarification: End-to-End Encryption
The message relay is a "dumb pipe" - it stores and forwards message bodies without knowing their contents. **End-to-end encryption is an APPLICATION-level concern**:
1. App encrypts message body using our `/encrypt` endpoint (BRC-2)
2. App sends encrypted blob via `/sendMessage`
3. Recipient fetches via `/listMessages`
4. Recipient decrypts using our `/decrypt` endpoint

This is the correct architecture - the relay never sees plaintext. Our BRC-2 implementation is **fully working** with ToolBSV.

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
| 6 | `listOutputs` | ✅ | ✅ | ✅ | **COMPLETE** - Basket/tag SQL filtering, BEEF support, pagination, outputDescription<br>✅ Tested with todo.metanet.app |
| 7 | `relinquishOutput` | ✅ | ✅ | ⏳ | **COMPLETE** - Removes output from basket tracking |
| 17 | `acquireCertificate` | ✅ | ✅ | ✅ | **COMPLETE** - Working with socialcert.net |
| 18 | `listCertificates` | ✅ | ⏳ | ❌ | **IMPLEMENTED** - Needs testing with real-world apps |
| 19 | `proveCertificate` | ✅ | ⏳ | ❌ | **IMPLEMENTED** - Needs testing with real verifiers |
| 20 | `relinquishCertificate` | ✅ | ⏳ | ❌ | **IMPLEMENTED** - Needs testing |
| 21 | `discoverByIdentityKey` | ✅ | ⏳ | ❌ | **IMPLEMENTED** - Searches certificates by subject public key |
| 22 | `discoverByAttributes` | ✅ | ⏳ | ❌ | **IMPLEMENTED** - Searches certificates by decrypted field values |
| 24 | `waitForAuthentication` | ✅ | ⏳ | ❌ | **IMPLEMENTED** - Validates wallet exists in database |
| 25 | `getHeight` | ✅ | ✅ | ❌ | **COMPLETE** - Fetches from WhatsOnChain `/chain/info` |
| 26 | `getHeaderForHeight` | ✅ | ✅ | ❌ | **COMPLETE** - Cache-first with API fallback, constructs 80-byte header |
| 27 | `getNetwork` | ✅ | ✅ | ❌ | **COMPLETE** - Returns hardcoded "mainnet" |

**Important Notes**:
- **Database Migrations**: Migrations run automatically when the wallet starts (`WalletDatabase::new()`). After adding new tables/columns, restart the wallet to apply migrations.
- **HTTP Interceptor**: When implementing new endpoints, add them to `isWalletEndpoint()` in `cef-native/src/core/HttpRequestInterceptor.cpp` to ensure requests are intercepted.
- **Migration Safety**: Migrations use `CREATE TABLE IF NOT EXISTS` and `ALTER TABLE` with existence checks - they won't overwrite existing data.

#### **Group D: Encryption & Advanced Crypto (Priority 4)** ✅ **ENCRYPTION COMPLETE!**
Privacy features and advanced cryptography.

| Call Code | Method | Status | Internal Test | Real-World Test | Notes |
|-----------|--------|--------|---------------|-----------------|-------|
| 9 | `revealCounterpartyKeyLinkage` | ❌ | ❌ | ❌ | BRC-69 counterparty key linkage |
| 10 | `revealSpecificKeyLinkage` | ❌ | ❌ | ❌ | BRC-69 specific key linkage |
| 11 | `encrypt` | ✅ | ✅ | ✅ | **COMPLETE** - BRC-2 encryption with BRC-42 key derivation |
| 12 | `decrypt` | ✅ | ✅ | ✅ | **COMPLETE** - BRC-2 decryption, ToolBSV image generation working |

#### **~~Group E: Non-Standard Methods~~** ❌ **REMOVED**

*The following methods were listed in error - they are NOT part of BRC-100:*
- ~~`getTransactionWithOutputs`~~ - Not in BRC-100 spec
- ~~`getSpendingLimits`~~ - Not in BRC-100 spec
- ~~`getProtocolRestrictions`~~ - Not in BRC-100 spec
- ~~`getBasketRestrictions`~~ - Not in BRC-100 spec

These may have been from a different implementation reference or placeholder ideas. BRC-100 defines exactly 28 methods (call codes 1-28), all of which are covered in Groups A-D above.

---

## Basket, Tag & Output Tracking System

**Status**: Core implementation complete and tested with real apps (todo.metanet.app). SDK-compared and verified against BSV TypeScript SDK (wallet-toolbox).

### Overview

Baskets and tags are BRC-100 organizational tools for categorizing and querying UTXOs:
- **Baskets**: Named containers grouping outputs by purpose (e.g., "todo tokens", "game items")
- **Tags**: Additional labels for fine-grained filtering (e.g., "weapon", "rare", "level-10")
- **Output Description**: Short description (5-50 bytes UTF-8) stored per output

### Database Schema

| Migration | Tables/Columns | Purpose |
|-----------|---------------|---------|
| V1 | `baskets` table, `basket_id` FK on `utxos` | Basket storage and UTXO association |
| V5 | `custom_instructions` on `utxos` | BRC-78 payment protocol data |
| V6 | `output_tags`, `output_tag_map` tables | Tag storage with soft-delete support |
| V9 | `status` on `utxos`, basket partial index | UTXO lifecycle tracking |
| V10 | `broadcast_status` on `transactions` | Transaction broadcast state tracking |
| V12 | `address_id` nullable on `utxos` | Basket outputs without HD wallet addresses |
| V13 | `derived_key_cache` table | PushDrop signing key persistence across restarts |
| V14 | `output_description` on `utxos` | BRC-100 output description storage |

### Repository Layer

**`basket_repo.rs`**:
- `validate_and_normalize_basket_name()` — trim, lowercase, BRC-99 rules (rejects "default", "p " prefix)
- `find_or_insert()` — auto-normalizes, updates `last_used`, creates on demand

**`tag_repo.rs`**:
- `validate_and_normalize_tag()` — trim, lowercase, 1-300 bytes UTF-8
- `assign_tag_to_output()` — find-or-insert tag, create mapping, soft-delete aware
- `find_tag_ids()` — batch tag name to ID resolution for queries
- `get_tags_for_output()` — JOIN tag_map, returns tag name strings

**`utxo_repo.rs`**:
- `insert_output_with_basket()` — nullable `address_id`, `basket_id`, `status`, `custom_instructions`, `output_description`
- `get_unspent_by_basket()` — `WHERE basket_id = ? AND is_spent = 0`
- `get_unspent_by_basket_with_tags()` — SQL JOINs with `all`/`any` mode (single query, no N+1)
- `mark_multiple_spent()` / `restore_spent_by_txid()` / `update_spent_txid_batch()` — optimistic locking primitives

### createAction Flow

1. **Validation** (~line 2942 in `handlers.rs`): Validates/normalizes basket names, tags, and output descriptions. Returns 400 on invalid input.
2. **Output loop** (~line 3706): Collects `PendingBasketOutput` structs with vout, satoshis, script_hex, basket_name, tags, custom_instructions, output_description.
3. **Change UTXO** (~line 4014): Change output inserted with `basket_repo.find_or_insert("default")`.
4. **Basket output insertion** (~line 4093): For each pending basket output:
   - `basket_repo.find_or_insert(&basket_name)` to get/create basket
   - `utxo_repo.insert_output_with_basket(...)` with all fields including output_description
   - `tag_repo.assign_tag_to_output(utxo_id, &tag_name)` per tag
5. **Txid reconciliation**: After signing, pre-signing txid updated to final signed txid.

### SDK-Style Optimistic Locking

When a basket output is consumed as a user-provided input in a subsequent `createAction`:

1. **createAction**: After parsing user inputs, marks matching local UTXOs as `is_spent=1, spent_txid="pending-{timestamp}"`. Shares the same placeholder as wallet UTXO reservation.
2. **PendingTransaction**: Stores `reservation_placeholder: Option<String>` so `signAction` can reference it.
3. **signAction**: Updates `spent_txid` from placeholder to final signed txid via `update_spent_txid_batch()`. Covers both wallet UTXOs and user-provided basket outputs.
4. **Failure recovery**: `restore_spent_by_txid()` restores all UTXOs sharing the `spent_txid` on broadcast failure. `restore_pending_placeholders()` catches stale reservations on startup.

### listOutputs (Call Code 6)

- Validates basket name (rejects "default" per BRC-100)
- Resolves tag names to IDs via `tag_repo.find_tag_ids()`
- SQL-based filtering with `all`/`any` tag query modes
- Pagination (offset/limit, max 10000)
- Optional fields: `includeTags`, `includeCustomInstructions`, `includeOutputDescription`, `includeLabels`
- Optional BEEF building for `include: "entire transactions"`

### internalizeAction Basket Insertion

- Parses `InternalizeOutput` with `InsertionRemittance` (basket, tags, custom_instructions)
- For `protocol: "basket insertion"`: validates basket/tags, inserts UTXO with basket, assigns tags
- Uses `address_id = None` for externally-received basket outputs

### Background Cleanup (`utxo_sync.rs`)

Runs every 5 minutes alongside the existing background UTXO sync:

| Task | SQL Query | SDK Equivalent |
|------|-----------|---------------|
| Restore stale pending reservations (>5 min) | `UPDATE utxos SET is_spent = 0 WHERE spent_txid LIKE 'pending-%' AND spent_at < now - 300` | TaskFailAbandoned (8 min) |
| Restore outputs from failed broadcasts | `UPDATE utxos SET is_spent = 0 WHERE spent_txid IN (SELECT txid FROM transactions WHERE broadcast_status = 'failed')` | TaskReviewStatus Query 2 |
| Mark stale pending transactions as failed (>15 min) | `UPDATE transactions SET broadcast_status = 'failed' WHERE broadcast_status = 'pending' AND timestamp < now - 900` | TaskFailAbandoned tx status update |
| Delete failed UTXOs | `DELETE FROM utxos WHERE status = 'failed'` | (existing) |
| Timeout unproven UTXOs (>1 hour) | `UPDATE utxos SET status = 'failed' WHERE status = 'unproven' AND first_seen < now - 3600` | (existing) |
| Clean old spent UTXOs (>30 days) | `cleanup_old_spent(30)` | (existing) |

### SDK Comparison Summary

All implemented functions have been compared against the BSV TypeScript SDK (wallet-toolbox):

| Area | Status |
|------|--------|
| Tag normalization (trim + lowercase) | Matches SDK |
| Tag deduplication | Matches SDK (explicit dedup vs implicit unique key) |
| Tag assignment in createAction | Functionally equivalent |
| Tag filtering in listOutputs (all/any modes) | Matches SDK; our tag normalization on queries is *more correct* |
| internalizeAction basket insertion | Matches SDK pattern |
| Basket name validation (BRC-99) | Matches SDK |
| Optimistic locking (reserve in createAction, update in signAction) | Matches SDK |
| Broadcast failure rollback (immediate) | Matches SDK |
| Periodic background cleanup | Matches SDK (stale reservation + failed broadcast restoration) |
| Output description storage | Matches SDK (V14 migration) |
| Permissioned baskets ("p " prefix) | Rejection matches SDK minimal behavior |

### Testing Status

**Tested with real app (todo.metanet.app)**:
- Create token with basket, listOutputs by basket, spend token (two-phase PushDrop)
- Optimistic locking, change output basket, txid reconciliation, BEEF ancestry
- Startup recovery of stale placeholder reservations

**Not yet tested with real app**:
- Tag creation, filtering (`any`/`all` modes), `include_tags` response
- internalizeAction basket insertion (needs app sending basket outputs)
- Broadcast failure rollback (needs simulated failure)
- Output description round-trip (create with description, retrieve via listOutputs)

### Key Files

| File | Purpose |
|------|---------|
| `rust-wallet/src/handlers.rs` | createAction basket/tag validation & insertion, signAction placeholder update, listOutputs, internalizeAction |
| `rust-wallet/src/database/basket_repo.rs` | Basket validation, normalization, find_or_insert |
| `rust-wallet/src/database/tag_repo.rs` | Tag validation, normalization, assignment, filtering |
| `rust-wallet/src/database/utxo_repo.rs` | insert_output_with_basket, basket/tag SQL queries, optimistic locking primitives |
| `rust-wallet/src/database/migrations.rs` | V1 (baskets), V6 (tags), V9 (status), V10 (broadcast), V12 (nullable addr), V14 (output_description) |
| `rust-wallet/src/utxo_sync.rs` | Background cleanup: stale reservations, failed broadcasts, pending tx timeout |

---

## Implementation Strategy

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
- ✅ `listOutputs` - **VERIFIED** - List UTXOs with basket/tag filtering, BEEF support
- ✅ `relinquishOutput` - **VERIFIED** - Remove output from basket tracking

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

**Basket & Tag Implementation** (completed Feb 2026):
- ✅ Database schema: `baskets`, `output_tags`, `output_tag_map` tables with indexes
- ✅ Basket assignment in `createAction` and `internalizeAction`
- ✅ Tag assignment with normalization and deduplication
- ✅ `listOutputs` with basket/tag SQL filtering (all/any modes)
- ✅ SDK-style optimistic locking for user-provided basket inputs
- ✅ Background cleanup job for stale reservations and failed broadcasts
- ✅ Output description storage (V14 migration)
- See "Basket, Tag & Output Tracking System" section below for full details.

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

### ~~Phase 5: Encryption & Advanced Features~~ ✅ **MOSTLY COMPLETE!**
**Goal**: Complete remaining methods.

**Methods Implemented**:
- ✅ `encrypt` / `decrypt` - BRC-2 encryption (Dec 27, 2024)
- ✅ `waitForAuthentication` - Async auth support

**Methods Remaining** (Low Priority):
- ❌ `revealCounterpartyKeyLinkage` - BRC-69 key linkage (rarely used)
- ❌ `revealSpecificKeyLinkage` - BRC-69 key linkage (rarely used)

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

## 🔌 **Socket.IO & Real-Time Notifications**

### What is Socket.IO?
Socket.IO is a protocol for **real-time, bidirectional communication** between clients and servers.

**Two ways apps can check for messages:**

| Method | How It Works | Pros | Cons |
|--------|--------------|------|------|
| **HTTP Polling** | App repeatedly asks "any messages?" | Simple | Wasteful, delayed |
| **Socket.IO** | Server pushes "you have a message!" | Efficient, instant | Requires persistent connection |

Socket.IO uses two transports:
1. **Long-polling** (`/socket.io/?transport=polling`) - HTTP fallback
2. **WebSocket** - Persistent TCP connection for true real-time

### Architecture: Who Runs the Socket.IO Server?

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         MESSAGE RELAY ARCHITECTURE                          │
└─────────────────────────────────────────────────────────────────────────────┘

When someone pays you:
1. Their wallet creates a transaction
2. Their wallet sends a message to messagebox.babbage.systems:
   "Tell recipient X that I just paid them"

When you want to know about payments:

┌──────────────┐         ┌─────────────────────────────┐
│  Your App    │ ──────► │ messagebox.babbage.systems  │  ◄── Babbage runs this!
│  (PeerPay)   │         │   (Socket.IO Server)        │
└──────────────┘         └─────────────────────────────┘
       │                            │
       │  Socket.IO connection      │
       │ ◄────────────────────────► │
       │                            │
       │  "New message arrived!"    │
       │ ◄──────────────────────────│  (push notification)
```

### Do We Need a Socket.IO Server?

**As a CLIENT wallet (current focus): NO**

We are a client wallet that:
- **Connects TO** PeerServ servers (like messagebox.babbage.systems)
- **Receives** push notifications about payments
- Uses **Babbage's infrastructure** for the heavy lifting

Our implementation:
- ✅ HTTP REST endpoints for local message storage (`/sendMessage`, `/listMessages`)
- ✅ SQLite persistence for messages (`relay_messages` table)
- ✅ Let Socket.IO requests pass through to real Babbage servers
- ❌ No Socket.IO server needed

**As a SERVER-SIDE wallet (future): YES, in Rust**

If building server-side wallet functionality (your own PeerServ):
```
┌────────────────────────────────────────────────────────────────────┐
│                  SERVER-SIDE WALLET (Future)                       │
└────────────────────────────────────────────────────────────────────┘

                    ┌────────────────────────────────┐
                    │   YOUR MESSAGE RELAY SERVER    │
                    │   (Rust - Actix-web + WS)      │
                    │                                │
                    │  - Store messages for users    │
                    │  - Push notifications via WS   │
                    │  - Handle federation (BRC-34)  │
                    └────────────────────────────────┘
                              ▲           │
                              │           │ Socket.IO/WebSocket
                              │           ▼
              ┌───────────────┴───────────────────────┐
              │                                       │
        ┌─────┴─────┐                           ┌─────┴─────┐
        │  User A   │                           │  User B   │
        │  Wallet   │                           │  Wallet   │
        └───────────┘                           └───────────┘
```

**Implementation location**: Rust wallet (Actix-web has excellent WebSocket support)
**NOT in C++**: The C++ layer is for browser functionality, not wallet services

### What Was Removed

Previously we had a **stubbed C++ WebSocket server** on port 3302 that:
- Only echoed messages back or returned 404
- Had TODO comments: "we'll implement proper proxying later"
- Was never functional

This has been **removed** (January 2025) because:
1. Client wallets don't need to run a Socket.IO server
2. If we need server-side functionality, it belongs in Rust
3. Keeping dead code causes confusion

### Future Implementation (If Needed)

If we build server-side PeerServ functionality:

```rust
// rust-wallet/src/websocket.rs (FUTURE - NOT IMPLEMENTED)
// Would use actix-web-actors for WebSocket support
//
// Features needed:
// - Socket.IO protocol (Engine.IO + Socket.IO layers)
// - Message push notifications
// - Connection management
// - Integration with relay_messages table
```

**Status**: Not implemented. Will build if/when needed for server-side apps.

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
   - ✅ **Answer**: No - federation is for multi-server setups, not needed for client wallet
   - ❓ Can we start without federation and add it later?
   - ✅ **Answer**: Yes - implement locally first, add federation if building server-side

5. **WebSocket vs HTTP**:
   - ✅ BRC-33 core endpoints are **HTTP POST** (port 3301)
   - ✅ Socket.IO is for real-time push notifications from PeerServ SERVERS
   - ✅ Apps use Socket.IO to connect to messagebox.babbage.systems for push notifications
   - ✅ We pass Socket.IO requests through to real Babbage servers

6. **Implementation**:
   - ✅ BRC-33 uses **HTTP POST** for message operations
   - ✅ **Decision**: Implement BRC-33 REST endpoints in Rust wallet (port 3301)
   - ✅ Socket.IO passthrough to messagebox.babbage.systems for real-time notifications
   - ✅ No local Socket.IO server needed for client wallet

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

### Implementation: Rust Wallet (Port 3301) ✅ IMPLEMENTED
```
App → HTTP POST → Rust Wallet (3301)
                  ├─ BRC-100 endpoints (✅ working)
                  ├─ /.well-known/auth (✅ working)
                  └─ BRC-33 message relay (✅ implemented)
                       ├─ /sendMessage
                       ├─ /listMessages
                       └─ /acknowledgeMessage

App → Socket.IO → messagebox.babbage.systems (passthrough)
                  └─ Real-time push notifications
```

**Pros**: Simple, all in one place, Rust performance, SQLite storage
**Implementation**: `rust-wallet/src/handlers.rs` + `database/message_relay_repo.rs`

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

**Last Updated**: February 3, 2026
**Current Status**: 26/28 BRC-100 methods implemented (93%), database schema at V14
**Progress**:
- Group A: Identity & Authentication - **COMPLETE** (8/8) - includes waitForAuthentication
- Group B: Transaction Operations - **COMPLETE** (5/5) - BRC-29 payments, BEEF/SPV
- Group C: Output Management & Blockchain Queries - **COMPLETE** (5/5) - basket/tag system with SDK-style optimistic locking
- Group C: Certificate Management - **COMPLETE** (6/6) - including discoverByAttributes
- Group D: Encryption - **COMPLETE** (2/2) - encrypt/decrypt working with ToolBSV
- Group D: Key Linkage - NOT STARTED (0/2) - Low priority, client audit/compliance feature

**Recent Additions (Feb 2026)**:
- Basket & tag system fully compared against BSV TypeScript SDK (wallet-toolbox)
- SDK-style optimistic locking (reserve user-provided inputs in createAction, update in signAction)
- Background cleanup job (stale reservations, failed broadcast restoration, pending tx timeout)
- Output description storage (V14 migration, validation, listOutputs response)
- Derived key cache (V13 migration, PushDrop signing persistence across restarts)

---

## 📋 Not Implemented: Server-Side Features

The following features are **not implemented** in HodosBrowser. These are primarily useful for **applications running servers** (relays, certificate authorities, hosted services) rather than client-side wallets.

### Why These Aren't Needed for Client Wallets

A client wallet's job is to:
- Store keys securely and sign transactions
- Authenticate with apps using BRC-100/104
- Send/receive messages through existing relays
- Store/present certificates issued by external certifiers

A client wallet does NOT need to:
- Host a message relay server
- Run a certificate authority
- Provide discovery services
- Handle multi-server federation

### Server-Side Features (Not Implemented)

| Feature | BRC Spec | Purpose | Why Server-Only |
|---------|----------|---------|-----------------|
| **Message Relay Server** | BRC-33 | Host relay for other users | Clients connect to existing relays, they don't run one |
| **WebSocket Push** | BRC-33 | Real-time message delivery | Server feature; clients use polling |
| **Federation** | BRC-34/35 | Multi-relay discovery/redundancy | Infrastructure concern for relay operators |
| **Certificate Authority** | BRC-52 | Issue identity certificates | Certifiers like socialcert.net do this, not wallets |
| **Key Linkage Revelation** | BRC-69 | Audit/compliance key disclosure | Specialized auditing feature, rarely used |
| `revealCounterpartyKeyLinkage` | BRC-69 | Reveal linkage to counterparty | Compliance/audit scenarios |
| `revealSpecificKeyLinkage` | BRC-69 | Reveal specific key derivation | Compliance/audit scenarios |

### End-to-End Encryption Note

**Q: Does E2E encryption require coordination between both parties?**

**A: Yes, but it's an APPLICATION-level concern, not a wallet/relay concern.**

How it works:
1. The BRC-33 relay just stores/delivers message bodies as-is (it's a dumb pipe)
2. Applications decide whether to encrypt their message bodies before sending
3. Both apps must agree ahead of time (typically by convention of the protocol they're using)
4. The wallet provides BRC-2 encryption primitives (`crypto/brc2.rs`) that apps CAN use
5. The relay doesn't need to know or care if messages are encrypted

Example: If CoinflipApp and ThryllGame both use encrypted messages, they:
- Define "messages in our protocol are always BRC-2 encrypted"
- Sender encrypts with recipient's public key before calling `/sendMessage`
- Recipient decrypts after calling `/listMessages`
- The relay never sees plaintext

**For HodosBrowser**: Our BRC-2 encryption (`/encrypt`, `/decrypt` endpoints) already supports this. Apps can encrypt message bodies before sending. No additional wallet work needed.

### Future SDK Consideration

If this codebase is converted to a general-purpose SDK/library:
- **Client SDK**: Current implementation is sufficient
- **Server SDK**: Would need to add relay hosting, WebSocket, federation, certificate issuance
- **Hybrid**: Could split into `hodos-wallet-client` and `hodos-wallet-server` crates
