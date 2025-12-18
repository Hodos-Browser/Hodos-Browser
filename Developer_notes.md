# Developer Notes - Bitcoin Browser

> **Quick Reference:** See [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md) for architecture details and [README.md](README.md) for setup instructions.

---

## 🔐 **Group C - Part 3: Certificate Management - IN PROGRESS** (2025-12-17)

### **Current Status: Core Features Working, Issuance Protocol Needs Review**

Successfully implemented certificate acquisition, signature verification, and database storage. Certificates are being stored in the database. However, the issuance protocol needs review - social cert works with other wallets and creates Bitcoin transactions, so we need to investigate our process.

### **What We've Implemented:**

#### **1. Database Schema (Migration v7)** ✅
**File**: `rust-wallet/src/database/migrations.rs`

**Tables Created:**
- `certificates` - Stores BRC-52 certificate metadata (type, serialNumber, certifier, subject, signature, revocationOutpoint, etc.)
- `certificate_fields` - Stores encrypted certificate field data (field_name, encrypted_value, encrypted_revelation_key)

**Key Design Decisions:**
- Separate `certificate_fields` table for better querying and selective disclosure support
- Foreign key relationship: `certificate_fields.certificate_id → certificates.id`
- Supports both master keyring (no serial_number) and verifier keyrings (with serial_number)

#### **2. Certificate Infrastructure** ✅
**Files**: `rust-wallet/src/certificate/`

**Modules Implemented:**
- `types.rs` - Certificate and CertificateField data structures
- `parser.rs` - BRC-52 certificate JSON parsing
- `verifier.rs` - BRC-52 signature verification and revocation checking
- `selective_disclosure.rs` - Keyring generation for verifiers

**Key Features:**
- BRC-52 signature verification using BRC-42 key derivation
- Revocation checking via UTXO spending status (WhatsOnChain API)
- Certificate preimage serialization matching TypeScript SDK exactly
- Selective disclosure keyring generation

#### **3. BRC-2 Encryption Implementation** ✅
**Files**: `rust-wallet/src/crypto/brc2.rs`, `rust-wallet/src/crypto/aesgcm_custom.rs`, `rust-wallet/src/crypto/ghash.rs`

**Custom AES-GCM Implementation:**
- Full custom AESGCM matching TypeScript SDK byte-for-byte
- Custom GHASH implementation for 32-byte IV processing
- GCTR (Galois Counter) mode encryption/decryption
- Proper IV processing: 32-byte IVs processed through GHASH to derive preCounterBlock
- Increment least significant 32 bits for counter blocks

**Key Technical Details:**
- **IV Processing**: TypeScript SDK uses GHASH to process 32-byte IVs (non-standard GCM behavior)
- **Key Format**: Always uses 32-byte keys (`toArray('be', 32)` pads with leading zeros)
- **Revelation Key**: Uses `toArray()` without length parameter, which strips leading zeros
- **Format**: `[32-byte IV][ciphertext][16-byte auth tag]`

#### **4. Certificate Handlers** ✅
**File**: `rust-wallet/src/handlers/certificate_handlers.rs`

**All Four Handlers Implemented:**
- ✅ `relinquishCertificate` - Marks certificates as deleted
- ✅ `listCertificates` - Lists certificates with filtering
- ✅ `acquireCertificate` - Supports both 'direct' and 'issuance' protocols
- ✅ `proveCertificate` - Selective disclosure with keyring generation

#### **5. BRC-53 Issuance Protocol** ✅
**File**: `rust-wallet/src/handlers/certificate_handlers.rs` (lines 629-1995)

**Full Two-Step Protocol:**
1. **Step 1: Initial Request** (BRC-31 Peer Protocol)
   - Sends unsigned `initialRequest` to `/.well-known/auth` (or `/initialRequest`)
   - Receives `initialResponse` with server's identity key and nonce
   - Verifies server's signature on `initialResponse` (mutual authentication)
   - Uses JavaScript-compatible base64 decoding for server signature verification

2. **Step 2: Certificate Signing Request (CSR)**
   - Encrypts certificate fields using BRC-2
   - Creates `masterKeyring` with encrypted revelation keys
   - Signs CSR using BRC-3 (BRC-42 key derivation)
   - Sends to `/signCertificate` with BRC-31 authentication headers

**BRC-31 Authentication Headers:**
- `x-bsv-auth-version: 0.1`
- `x-bsv-auth-identity-key: <hex>`
- `x-bsv-auth-nonce: <base64>` (separate signing nonce)
- `x-bsv-auth-your-nonce: <base64>` (server's nonce from initialResponse)
- `x-bsv-auth-request-id: <base64>` (first 32 bytes of serialized request)
- `x-bsv-auth-signature: <hex>` (DER-encoded ECDSA signature)

**Request Serialization:**
- Binary format with VarInt encoding (matching `AuthFetch.serializeRequest()`)
- Format: `[32-byte nonce][VarInt method length][method][VarInt path length][path][VarInt header count][headers][VarInt body length][body]`
- Headers encoded as `[VarInt key length][key][VarInt value length][value]`

### **What We've Learned About CSR Format:**

#### **CSR JSON Structure:**
```json
{
  "clientNonce": "<base64>",  // Original nonce from initialRequest
  "type": "<base64>",         // Certificate type (32 bytes, base64-encoded)
  "fields": {                 // Encrypted field values
    "<fieldName>": "<base64>" // Encrypted: [32-byte IV][ciphertext][16-byte tag]
  },
  "masterKeyring": {          // Encrypted revelation keys for certifier
    "<fieldName>": "<base64>" // Encrypted: [32-byte IV][ciphertext][16-byte tag]
  }
}
```

**Key Points:**
- **Minimal Fields**: TypeScript SDK only sends `clientNonce`, `type`, `fields`, `masterKeyring`
- **No Optional Fields**: Does NOT include `messageType`, `serverSerialNonce`, `validationKey`, etc.
- **Field Values**: Must be strings (booleans/numbers converted to string representation)
- **Base64 Encoding**: All encrypted values are base64-encoded strings

#### **Field Encryption Process:**
1. **Generate Random Symmetric Key**: 32 random bytes for each field
2. **Encrypt Field Value**: AES-256-GCM with the symmetric key
   - Plaintext: Field value as UTF-8 string (e.g., `"true"` → `[0x74, 0x72, 0x75, 0x65]`)
   - IV: 32 random bytes
   - Output: `[32-byte IV][ciphertext][16-byte auth tag]`
3. **Encrypt Revelation Key**: Encrypt the symmetric key for the certifier using BRC-2
   - Plaintext: Symmetric key with leading zeros stripped (matching `SymmetricKey.toArray()`)
   - Invoice number: `"2-certificate field encryption-<fieldName>"`
   - Counterparty: Certifier's public key
   - Output: `[32-byte IV][ciphertext][16-byte auth tag]`

### **What We've Learned About Encryption:**

#### **BRC-2 Encryption Details:**
- **Symmetric Key Derivation**: Uses BRC-42 ECDH shared secret, extracts x-coordinate (32 bytes)
- **Invoice Number Format**: `"<securityLevel>-<protocolName>-<keyID>"` (protocolName lowercased and trimmed)
- **Key Derivation**: `derive_symmetric_key(sender_private, recipient_public, invoice_number)`
- **Encryption**: Custom AES-256-GCM with 32-byte IV processing through GHASH

#### **TypeScript SDK Behavior:**
- **SymmetricKey.encrypt()**: Always uses 32-byte key (`toArray('be', 32)` pads with leading zeros)
- **SymmetricKey.toArray()**: Without length parameter, strips leading zeros
- **IV Generation**: `Random(32)` generates 32 random bytes
- **IV Processing**: 32-byte IVs processed through GHASH to derive preCounterBlock (non-standard GCM)

#### **Critical Discoveries:**
1. **Revelation Key Length**: Must strip leading zeros from symmetric key before encrypting (matches `toArray()` behavior)
2. **Field Value Serialization**: Convert JSON values to string representation (`true` → `"true"`, not `[1]` or `\"true\"`)
3. **Base64 Decoding**: Server uses JavaScript-compatible lenient base64 decoding (handles invalid padding)
4. **Nonce Order**: Server signature verification uses `client_nonce + server_nonce` (in that order)
5. **Invoice Number**: Protocol name must be lowercased and trimmed (BRC-43 specification)

### **Current Status: Certificate Acquisition & Storage Working ✅**

**What's Working:**
- ✅ Certificate acquisition via 'direct' protocol - successfully acquiring certificates
- ✅ Certificate signature verification - BRC-52 verification working (fixed `anyone_private_key` bug)
- ✅ Certificate storage - certificates stored in database with encrypted fields
- ✅ UI display - fixed to return JSON object instead of base64 string
- ✅ Database schema - migration v7 complete with proper certificate storage

**Critical Bug Fixed:**
- **`anyone_private_key` Initialization**: Was incorrectly `[1u8; 32]` (all bytes = 1), fixed to `[0u8; 32]` with last byte = 1 (private key with value 1)
- This was causing signature verification failures - now working correctly

**Known Issues / Needs Review:**
- ⚠️ **Issuance Protocol**: Social cert works with other wallets and creates Bitcoin transactions
  - Need to review our issuance process and compare with working implementations
  - May need to investigate how certificates are embedded in blockchain transactions
  - Our process may be missing steps for creating the actual Bitcoin transaction

**Next Steps:**
- Review issuance protocol implementation
- Compare with working wallet that successfully creates Bitcoin transactions
- Test with social cert to identify any missing steps
- Complete end-to-end testing with various certificate types

**Files Modified:**
- `rust-wallet/src/database/migrations.rs` - Added v7 migration for certificates
- `rust-wallet/src/database/certificate_repo.rs` - Certificate CRUD operations
- `rust-wallet/src/certificate/` - All certificate infrastructure modules
- `rust-wallet/src/crypto/brc2.rs` - BRC-2 encryption/decryption
- `rust-wallet/src/crypto/aesgcm_custom.rs` - Custom AES-GCM implementation
- `rust-wallet/src/crypto/ghash.rs` - GHASH implementation
- `rust-wallet/src/handlers/certificate_handlers.rs` - All four certificate handlers

**Documentation Created:**
- `development-docs/CSR_FORMAT_AND_IMPLEMENTATION.md` - Comprehensive CSR format documentation
- `development-docs/ENCRYPTION_AND_DB_STORAGE.md` - Encryption and storage flow
- `development-docs/ENCRYPTION_VERIFICATION.md` - Encryption verification details

---

## 🎉 **Group C - Part 2: Blockchain Queries Complete!** (2025-12-08)

### **Latest Achievement: All Three Blockchain Query Methods Implemented!**

Successfully implemented all three blockchain utility methods for Group C Part 2:
- ✅ `getHeight` (Call Code 25) - Returns current blockchain height
- ✅ `getHeaderForHeight` (Call Code 26) - Returns 80-byte block header by height
- ✅ `getNetwork` (Call Code 27) - Returns network name ("mainnet")

**Key Implementation Details**:
- `getHeight`: Fetches from WhatsOnChain `/chain/info` API, extracts `blocks` field
- `getHeaderForHeight`: Cache-first approach (database → API), constructs 80-byte header from API fields
- `getNetwork`: Simple hardcoded "mainnet" return (can be enhanced with config later)

**Testing**: All three methods tested successfully with PowerShell commands. `getHeaderForHeight` required fixing API endpoint format (uses `/block/{hash}/header` as per ts-brc100 reference).

**Files Modified**:
- `rust-wallet/src/handlers.rs` - Added three handler functions
- `rust-wallet/src/main.rs` - Added routes for all three methods

**Next**: Part 3 - Certificate Management (BRC-52)

---

## 🗄️ **Database Migration Complete!** (2025-12-06)

### **Latest Achievement: Phase 9 Backup & Recovery Complete!**

The wallet has been fully migrated from JSON file storage to SQLite database. All wallet data (addresses, transactions, UTXOs, BEEF/SPV cache) is now stored in `%APPDATA%/HodosBrowser/wallet/wallet.db`. All database phases (1-9) are complete:

**Phase 1-3**: Database foundation, schema, and data migration ✅
**Phase 4**: UTXO management with background sync ✅
**Phase 5**: BEEF/SPV caching (parent transactions, Merkle proofs, block headers) ✅
**Phase 6**: Performance optimization (indexes, in-memory balance cache) ✅
**Phase 7**: Backup & recovery (file backup, JSON export, mnemonic recovery) ✅
**Phase 8**: Browser database (deferred to separate sprint)
**Phase 9**: Cleanup & documentation ✅

**Key Features:**
- Fast balance checks (database cache + 30-second in-memory cache)
- Background UTXO sync (every 5 minutes with gap limit)
- BEEF/SPV caching with automatic population (every 10 minutes)
- Cache-first transaction signing (no API delays)
- File-based backup and restore functionality
- Recovery from mnemonic (re-derive addresses, re-discover UTXOs)
- Performance indexes for optimized queries

---

## 🗄️ **Database Migration Planning Complete!** (2025-11-XX)

### **Historical: Complete Database Architecture & Migration Plan**

After comprehensive research and alignment with course notes and metanet-desktop reference implementation, we finalized and implemented a complete database migration plan.

### **What We Planned:**

#### **1. Database Architecture** ✅
- **Technology**: SQLite (embedded, single-file, ACID-compliant)
- **Location**: `%APPDATA%/HodosBrowser/wallet/wallet.db`
- **Library**: `rusqlite` with migrations feature
- **Schema**: 15 tables covering all wallet data

#### **2. Key Schema Additions** ✅
- **Baskets Table**: Token organization (required for token management)
- **Certificates Table**: BRC-52 support (prevents migration issues later)
- **Messages Table**: BRC-33 persistence (currently in-memory only)
- **Custom Instructions**: BRC-29 storage in transactions (delete after confirmation)
- **UTXO Caching**: Eliminate API calls during transactions
- **BEEF/SPV Caching**: Cache parent transactions and Merkle proofs

#### **3. Implementation Phases** ✅
1. Database Foundation (Current)
2. Data Migration (JSON → SQLite)
3. Core Functionality Migration
4. UTXO Management & Caching
5. BEEF/SPV Caching
6. Basket Implementation
7. Additional Features

**Key Decisions**:
- ✅ Add baskets now (easier than retrofitting)
- ✅ Add certificates table now (no migration risk)
- ✅ Add messages table now (BRC-33 persistence)
- ✅ Store custom instructions (delete after confirmation)
- ⏳ Caching implementation (separate phase after migration)

**Documentation Created**:
- `development-docs/DATABASE_MIGRATION_IMPLEMENTATION_GUIDE.md` - Complete step-by-step guide
- `development-docs/BASKET_IMPLEMENTATION_PLAN.md` - Basket design details
- `development-docs/DATABASE_SCHEMA_DECISIONS.md` - All schema decisions
- `development-docs/COURSE_NOTES_ALIGNMENT_ANALYSIS.md` - Alignment with course notes

**Next Steps**: Begin Phase 1 - Database Foundation implementation

---

## 🗄️ **Phase 6: BEEF/SPV Caching Complete!** (2025-12-05)

### **Achievement: Cache-First Transaction Signing**

Implemented comprehensive BEEF/SPV caching system to eliminate API delays during transaction signing.

**Key Features:**
- **Parent Transaction Caching**: Pre-fetched and stored in database
- **Merkle Proof Caching**: TSC/BUMP format proofs cached with block height
- **Block Header Caching**: Fast height resolution for proofs
- **Background Cache Sync**: Automatic population every 10 minutes
- **Cache-First Approach**: Uses cached data, falls back to API only on miss
- **Schema Migration v3**: Nullable `utxo_id` for external parent transactions

**Impact**: Transaction signing now uses cached data, eliminating API delays and improving user experience.

---

## 🗄️ **Phase 7: Performance Optimization Complete!** (2025-12-05)

### **Achievement: Fast Queries and Instant Balance Checks**

Implemented performance optimizations including database indexes and in-memory caching.

**Key Features:**
- **Database Indexes (Schema v4)**: Optimized indexes on frequently queried columns
  - UTXO lookups: `(address_id, is_spent)`, `(txid, vout)`
  - Transaction queries: `txid`, `reference_number`
  - Merkle proof lookups: `parent_txn_id`, `block_hash`, `height`
- **In-Memory Balance Cache**: 30-second TTL for instant balance checks
- **Cache Invalidation**: Automatic invalidation on UTXO changes
- **Query Optimization**: All queries use indexes for fast retrieval

**Impact**: Balance checks are now instant (cached), and all database queries are optimized with indexes.

---

## 🗄️ **Phase 9: Backup & Recovery Complete!** (2025-12-06)

### **Achievement: Complete Backup and Recovery System**

Implemented comprehensive backup, restore, and recovery functionality for wallet data protection.

**Key Features:**
- **File-Based Backup**: Copies database + WAL + SHM files to user-specified location
- **JSON Export**: Exports non-sensitive data (addresses, transactions, UTXOs) for debugging
- **Recovery from Mnemonic**: Re-derives addresses deterministically and re-discovers UTXOs from blockchain
- **Restore Functionality**: Restores database from backup with safety verification
- **Backup Verification**: Validates backup file integrity before restore

**Frontend Integration Note:**
- Backup/restore endpoints require file picker dialog to let user choose backup location
- User selects destination path via frontend, which is passed to backend API (`POST /wallet/backup`)

**Recovery Process:**
- Re-derives addresses from mnemonic (tries both BIP32 and BRC-42)
- Checks blockchain for UTXOs on each address
- Uses gap limit (default: 20) to determine when to stop
- Returns discovered addresses and UTXOs (read-only, doesn't modify database)

**Impact**: Users can now backup their wallet, restore from backup, and recover from mnemonic if database is lost.

---

## 🎉 **Wallet Panel UI Enhancements & USD Conversion Complete!** (2025-12-XX)

### **Latest Achievement: Enhanced User Experience in Wallet Panel**

The wallet panel now provides excellent visual feedback and supports USD conversion for sending transactions!

### **What We Built:**

#### **1. Button Click Feedback System** ✅
**Files:** `frontend/src/components/panels/WalletPanelContent.tsx`, `frontend/src/components/WalletPanel.css`

**Key Features:**
- **Immediate Visual Feedback:** All buttons now show instant visual response on click
  - CSS `:active` states for immediate press feedback
  - Click animation classes with React state management
  - Scale animations and color transitions
- **Receive Button:** Maintains visual feedback throughout async address generation
  - Pulsing animation during processing
  - State persists until operation completes
- **Send Button:** Shows active state when form is open
  - Text changes: "Send" → "Close Send"
  - Visual indicator with gold border glow
- **Navigation Buttons:** Click animations with color flash
- **Copy Buttons:** Success confirmations ("✓ Copied!" for 2 seconds)

#### **2. USD Conversion for Send Transactions** ✅
**File:** `frontend/src/components/TransactionForm.tsx`

**Key Features:**
- **Toggle Button:** Switch between BSV and USD input modes
- **Automatic Price Fetching:** Fetches BSV price from CryptoCompare API when switching to USD mode
- **Real-time Conversion Hints:** Shows equivalent values as user types
  - USD mode: Shows satoshis and BSV equivalent
  - BSV mode: Shows USD equivalent
- **Automatic Amount Conversion:** Converts amount when switching modes
- **MAX Button:** Works in both modes (fills with balance in selected currency)
- **Form Submission:** Automatically converts USD to satoshis before sending to backend

**Implementation Details:**
- Uses same CryptoCompare API as balance display
- Caches price to avoid refetching
- Validation works for both input modes
- All USD amounts converted to BSV format internally before backend submission

---

## 🎉 **BRC-29 PAYMENTS WORKING!** Transaction System Complete! (2025-10-30)

### **Latest Achievement: Complete Transaction Lifecycle with Real-World Testing!**

After extensive debugging and implementation, the Rust wallet now successfully completes payments with real BRC-100 sites like ToolBSV!

### **What We Built:**

#### **1. Complete Transaction Creation (`createAction`)** ✅
**File:** `rust-wallet/src/handlers.rs` (lines 1621-2139)

**Key Features:**
- **UTXO Selection:** Automatically fetches and selects UTXOs from WhatsOnChain API
- **Fee Calculation:** Smart fee estimation with dust limit handling
- **Multiple Output Support:** Handles multiple outputs with different script types
- **Change Output Generation:** Creates proper change outputs when needed
- **Action Storage:** Automatically stores transactions in action history
- **BRC-29 Payment Detection:** Automatically detects and handles BRC-29 payments

#### **2. BRC-29 Payment Protocol Support** ✅
**File:** `rust-wallet/src/handlers.rs` (lines 1723-1832)

**How It Works:**
1. **Detection:** Checks for `customInstructions` in output request
2. **Extraction:** Parses `derivationPrefix`, `derivationSuffix`, and `payee` public key
3. **BRC-42 Derivation:** Uses wallet's master private key + payee public key to derive unique P2PKH script
4. **Invoice Number:** Formats as `"2-3241645161d8-<prefix> <suffix>"` (BRC-29 protocol ID)
5. **Script Generation:** Creates standard P2PKH locking script from derived public key

**Code Example:**
```rust
if let (Some(prefix), Some(suffix), Some(payee)) = (
    instr_json["derivationPrefix"].as_str(),
    instr_json["derivationSuffix"].as_str(),
    instr_json["payee"].as_str()
) {
    // BRC-29 invoice number format
    let invoice_number = format!("2-3241645161d8-{} {}", prefix, suffix);

    // Derive child public key using BRC-42
    let derived_pubkey = derive_child_public_key(&master_key_bytes, &payee_bytes, &invoice_number)?;

    // Create P2PKH script from derived public key
    let script = create_p2pkh_script_from_pubkey(&derived_pubkey);
}
```

**Key Insight:** BRC-29 uses BRC-42 key derivation to create unique, unlinkable payment addresses for each transaction. This enables privacy-preserving micropayments without exposing the wallet's identity.

#### **3. Transaction Signing with BSV ForkID SIGHASH** ✅
**File:** `rust-wallet/src/handlers.rs` (lines 2367-2757)

**Key Features:**
- **Multi-Input Signing:** Signs each input with correct private key from its originating address
- **BSV ForkID SIGHASH:** Uses `SIGHASH_ALL_FORKID (0x41)` for BSV compatibility
- **Parent Transaction Fetching:** Automatically fetches parent transactions from WhatsOnChain
- **TSC Merkle Proof Generation:** Fetches and adds SPV proofs for transaction validation
- **Atomic BEEF Creation:** Wraps standard BEEF with Atomic BEEF (BRC-95) format

**Important Discovery - TSC Proof Parsing:**
- **Problem:** TSC proofs from WhatsOnChain don't include `block_height`, only `block_hash`
- **Solution:** Make separate API call to `/block/hash/{hash}` to get block height
- **Reference:** BSV/SDK does this too! Found in TypeScript reference implementation
- **Code:** Lines 2598-2641

```rust
// Fetch block height from block hash (BSV/SDK does this too)
let block_header_url = format!("https://api.whatsonchain.com/v1/bsv/main/block/hash/{}", target);
let header_response = client.get(&block_header_url).send().await?;
let header_json: serde_json::Value = header_response.json().await?;
let height = header_json["height"].as_u64().unwrap();

// Create enhanced TSC object with height field
let mut enhanced_tsc = tsc_obj.clone();
enhanced_tsc["height"] = serde_json::json!(height);

beef.add_tsc_merkle_proof(&utxo.txid, tx_index, &enhanced_tsc)?;
```

#### **4. Atomic BEEF (BRC-95) Format** ✅
**File:** `rust-wallet/src/beef.rs`

**Format:**
```
[4 bytes]   0x01 0x01 0x01 0x01   ← Magic prefix
[32 bytes]  Subject TXID           ← Transaction being validated
[variable]  Standard BEEF          ← Parent transactions + Main transaction
```

**Why Atomic BEEF:**
- Enables SPV validation of single transaction
- Includes full ancestry chain and Merkle proofs
- Standard format for BSV transaction submission
- Required by BRC-100 specification

#### **5. BEEF Structure & Merkle Proofs** ✅
**File:** `rust-wallet/src/beef.rs`

**BEEF V2 Format:**
- **Parent Transactions:** All inputs' source transactions
- **Main Transaction:** The signed transaction we're validating
- **BUMPs (Block Unspent Merkle Proofs):** SPV proofs for each parent transaction
- **Mapping:** Links transactions to their proofs

**BUMP (Block Unspent Merkle Proof) Format:**
- **Block Height:** Block containing the transaction
- **Tree Height:** Height of Merkle tree (number of levels)
- **Levels:** Vector of nodes at each level
- **Nodes:** Each node is `[varint offset][flags][32-byte hash]`

**TSC to BUMP Conversion:**
- TSC format: Simple array of hex hashes
- BUMP format: Encoded with offsets and flags for efficient serialization
- Our code converts TSC proofs to BUMP format automatically

#### **6. What We Cleaned Up** ✅

**Removed Unused Code:**
1. **`beef_to_brc29_message` function** - Was never called after reverting BRC-29 JSON conversion
2. **`tsc_nodes` field from `MerkleProof`** - Was storing TSC nodes for BRC-29 conversion that we reverted
3. **Unused BRC-29 JSON conversion logic** - We return Atomic BEEF for all transactions

**Key Decision:** After extensive investigation, we confirmed that BRC-29 payments should return **Atomic BEEF** (binary format), not a BRC-29 JSON envelope. The BRC-29 specific data (`derivationPrefix`, `derivationSuffix`, `senderIdentityKey`) are sent by the **browser** to the **server** in the `x-bsv-payment` header - the wallet just derives the correct locking script!

### **Current Status:**
- ✅ **Transaction Creation:** Working with UTXO selection and fee calculation
- ✅ **Transaction Signing:** BSV ForkID SIGHASH working correctly
- ✅ **Atomic BEEF Generation:** Format correct, includes parent transactions
- ✅ **TSC Merkle Proofs:** Fetching and converting correctly with block height resolution
- ✅ **BRC-29 Payments:** Automatic detection and script derivation working
- ✅ **Real-World Testing:** ToolBSV payments completing successfully!
- ✅ **Action History:** Transactions stored with full metadata
- ✅ **BEEF Parsing:** Phase 2 parser for incoming transactions

### **What Still Needs Work:**
- 🔄 **`inputBEEF` handling:** Parse input BEEF to validate parent transactions
- 🔄 **Multiple miner broadcasting:** Currently only using WhatsOnChain + GorillaPool
- 🔄 **`noSend` mode:** Support for creating unsigned transactions for later signing
- 🔄 **`sendWith` batching:** Support for chained transaction creation

---

## 🚨🚨 **BRC-33 Message Relay Discovery!** (2025-10-22 Evening)

### **What Happened:**
After fixing authentication, tested with Coinflip and Thryll apps → still failing!

**Wallet Logs:**
```
[2025-10-22T19:07:22Z INFO] 127.0.0.1 "POST /listMessages HTTP/1.1" 404
[2025-10-22T19:07:22Z INFO] 127.0.0.1 "POST /listMessages HTTP/1.1" 404
[2025-10-22T19:07:22Z INFO] 127.0.0.1 "POST /listMessages HTTP/1.1" 404
```

**App Error:**
```javascript
Error: object null is not iterable (cannot read property Symbol(Symbol.iterator))
```

### **The Discovery:**
Apps are calling **[BRC-33 PeerServ Message Relay](https://bsv.brc.dev/peer-to-peer/0033)** endpoints!

**Missing Endpoints:**
- `/sendMessage` - Send messages to recipients
- `/listMessages` - List messages from inbox ⚠️ **BLOCKING COINFLIP!**
- `/acknowledgeMessage` - Delete acknowledged messages

**Key Insights:**
- ✅ BRC-33 is **NOT part of BRC-100** - separate message relay specification
- ✅ Uses **HTTP POST**, not WebSocket
- ✅ Authentication already working (BRC-31/Authrite) - same as `/.well-known/auth`
- ✅ HTTP interceptor already catching these routes
- ❌ Rust wallet just needs handlers and message storage!

**Documentation Created:**
- See `BRC33_MESSAGE_RELAY_DISCOVERY.md` for comprehensive analysis
- See `BRC100_IMPLEMENTATION_GUIDE.md` for updated implementation plan

**Next Steps:**
1. Read [BRC-33 spec](https://bsv.brc.dev/peer-to-peer/0033)
2. Design message storage system
3. Implement 3 endpoints in Rust wallet
4. Test with Coinflip and Thryll

---

## 🎉 **AUTHENTICATION COMPLETE!** Rust Wallet BRC-103/104 (2025-10-23)

### **7 CRITICAL BREAKTHROUGHS - AUTHENTICATION NOW FULLY WORKING!**

#### **Breakthrough #1: Fixed the Nonce Bug** ✅
**Problem:** 48-byte nonces (16 random + 32 HMAC) instead of 32 bytes
**Solution:** Simple 32-byte random nonces
**Status:** ✅ FIXED

#### **Breakthrough #2: Implemented `/verifySignature`** ✅
**Problem:** ToolBSV was calling `/verifySignature` to verify OUR signature, but it was stubbed
**Root Cause:** Misunderstood BRC-3 protocol - apps call the WALLET's endpoint
**Solution:** Fully implemented BRC-3 signature verification with BRC-42 child key derivation
**Status:** ✅ IMPLEMENTED & TESTED

#### **Breakthrough #3: Fixed Master Key Consistency** ✅
**Problem:** Using INDEX 0 key for HMAC but MASTER key for authentication
**Solution:** Changed all operations to consistently use master key
**Status:** ✅ FIXED & COMPILED

#### **Breakthrough #4: BRC-42 "self" Counterparty** ✅
**Problem:** HMAC failures persisted due to incorrect "self" interpretation
**Root Cause:** According to [BRC-56](https://bsv.brc.dev/wallet/0056#hmacs), `counterparty="self"` means NOT a two-party interaction
**Solution:** For `counterparty="self"`, use RAW master key (NO BRC-42 derivation for HMAC)
**Status:** ✅ FIXED & COMPILED

#### **Breakthrough #5: KeyID Base64 Encoding** ✅
**Problem:** HMAC verification still failing with "nonce verification failed"
**Root Cause:** `String::from_utf8_lossy()` was CORRUPTING binary keyID bytes (e.g., byte 227 → �)
**Impact:** Invoice numbers didn't match between createHmac and verifyHmac!
**Solution:** Use `base64::encode()` to preserve ALL bytes exactly
**Status:** ✅ FIXED & COMPILED

#### **Breakthrough #6: BRC-42 Signature Verification** ✅ **[CRITICAL!]**
**Problem:** `/verifySignature` was deriving OUR child public key when verifying external signatures
**Root Cause:** Misunderstood asymmetric nature of BRC-42 for signature verification
**Impact:** All signature verifications from external parties (like Thoth backend) were failing!
**Solution:**
- Changed from deriving our child private key → extracting public key
- To: Directly deriving the SIGNER's child public key using `derive_child_public_key()`
- **Key Insight**: For verification, we derive `signer_public + G * HMAC_scalar`, NOT our key!
**Status:** ✅ FIXED & WORKING

#### **Breakthrough #7: External Backend Session Validation** ✅ **[FINAL FIX!]**
**Problem:** Wallet rejecting `/createSignature` requests to Thoth backend with 401 Unauthorized
**Root Cause:** Session validation logic didn't distinguish between wallet-to-app auth and app-to-backend API calls
**Impact:** Apps couldn't make authenticated API calls to their backends using our wallet signatures!
**Solution:**
- Added logic to detect external backend requests (counterparty != our identity key)
- Skip session nonce validation for external backend requests
- **Key Insight**: Apps authenticate with their backends independently; we just sign the requests!
**Status:** ✅ FIXED & WORKING

**Current Status - AUTHENTICATION COMPLETE!** 🎉
- ✅ **Working**: All 7 breakthroughs implemented and tested
- ✅ **Working**: ToolBSV authentication successful
- ✅ **Working**: Identity token retrieval from Thoth backend
- ✅ **Working**: Fetching image/video history from streaming API
- ✅ **Working**: BRC-42 signature verification (both directions)
- ✅ **Working**: External backend API signing
- ✅ **Working**: Session management with concurrent support
- ✅ **Working**: BRC-33 message relay endpoints (3/3)
- ✅ **Working**: Domain whitelisting system

### What Changed (Oct 22-23):

#### **Final Session (Oct 23 Evening): Breakthroughs #6 & #7**

#### **Change #7: Fixed BRC-42 Signature Verification** (lines 1147-1180, rust-wallet/src/handlers.rs)

**Problem**: When verifying signatures from external signers (like Thoth's backend), we were deriving OUR child public key instead of the SIGNER's child public key.

**Before**:
```rust
// WRONG: Derive our child private key, then extract public key
let our_child_privkey = derive_child_private_key(&our_master_privkey, &counterparty_pubkey, &invoice);
let child_pubkey = PublicKey::from_secret_key(&secp, &our_child_privkey);
```

**After**:
```rust
// CORRECT: Directly derive the signer's child public key
use crate::crypto::brc42::derive_child_public_key as derive_child_pub;
let signer_child_pubkey_bytes = derive_child_pub(&our_master_privkey, &counterparty_pubkey, &invoice);
let signer_child_pubkey = PublicKey::from_slice(&signer_child_pubkey_bytes);
```

**Key Insight**: BRC-42 derivation is asymmetric for signatures. The signer derives their child private key, and the verifier derives the signer's child public key using the formula: `signer_public + G * HMAC_scalar`.

#### **Change #8: External Backend Session Validation** (lines 1309-1380, rust-wallet/src/handlers.rs)

**Problem**: Session nonce validation was rejecting legitimate API requests to external backends (like Thoth).

**Solution**: Added logic to detect external backend requests and skip session validation:
```rust
// Get our wallet's identity key for comparison
let our_identity_key = { /* ... */ };

// Determine if this is a request to an external backend
let is_external_backend = match &req.counterparty {
    serde_json::Value::String(s) if s != "self" && s != "anyone" && s != &our_identity_key => {
        log::info!("   🌐 External backend detected: {}", s);
        true
    }
    _ => false
};

if req.key_id.contains(' ') && !is_external_backend {
    // Validate session only for wallet-to-app authentication
    ...
} else if is_external_backend {
    log::info!("   ℹ️  External backend request - skipping session validation");
}
```

**Key Insight**: Apps authenticate with their backends independently. Our wallet just signs the requests; we don't need to validate sessions for those external authentications.

---

#### **Change #1: Fixed Nonce Generation** (lines 128-162, Oct 22)

**File**: `rust-wallet/src/handlers.rs`

**Removed**:
- 48-byte HMAC-based nonce generation (16 random + 32 HMAC)
- Confusing BRC-84 comments (BRC-84 is about linked keys, not nonces)
- Unnecessary HMAC complexity

**Added**:
- Simple 32-byte random nonce generation
- Clear comments explaining why (wallet = client, not high-volume server)
- TODO for nonce tracking (replay attack prevention for later)

#### **Change #2: Implemented `/verifySignature`** (lines 954-1214)

**File**: `rust-wallet/src/handlers.rs`

**Before**: Stub implementation returning `false`

**After**: Full BRC-3 compliant verification:
1. Parses signature verification request (protocolID, keyID, counterparty, signature, data)
2. Computes BRC-43 invoice number
3. Derives our child private key using BRC-42: `our_master_priv + counterparty_pub + invoice`
4. Extracts public key from child private key
5. Verifies signature using ECDSA with derived child public key
6. Returns `{"valid": true/false}`

**Key Insight**: ToolBSV calls OUR `/verifySignature` endpoint to verify signatures WE create! They don't do BRC-42 derivation themselves - they ask us to do it via this endpoint.

#### **Change #3: Fixed Master Key Consistency** (lines 548, 842, 1334)

**File**: `rust-wallet/src/handlers.rs`

**Problem**: Using different keys for different operations:
- Authentication: MASTER key (`020b95...`)
- HMAC operations: INDEX 0 key (`030fe8...`)

**Impact**: BRC-42 derivation produced different child keys, causing HMAC verification failures!

**Fixed**:
1. `/createHmac`: Now uses `get_master_private_key()` instead of `derive_private_key(0)`
2. `/verifyHmac`: Now uses `get_master_private_key()` instead of `derive_private_key(0)`
3. `/createSignature`: Now uses `get_master_private_key()` instead of `derive_private_key(0)`

**Why This Matters**: BRC-42 shared secret = `your_priv * their_pub`. Using different base keys produces different shared secrets and different child keys!

### Next Steps:
1. ✅ **COMPLETE** - BRC-104 authentication fully working!
2. ✅ **COMPLETE** - ToolBSV working with identity tokens!
3. 🎯 **Next Priority** - Complete remaining BRC-100 endpoints (see BRC100_IMPLEMENTATION_GUIDE.md)

---

## ✅ MAJOR ACHIEVEMENTS: Rust Wallet Transaction System (2025-10-16)

### Breakthrough: BSV ForkID SIGHASH Implementation

Successfully implemented **production-ready Rust wallet** with full BSV transaction support!

**What's Working:**
- ✅ Complete BRC-103/104 mutual authentication (working with ToolBSV)
- ✅ HMAC-based nonce verification (`/createHmac`, `/verifyHmac`)
- ✅ Transaction creation with UTXO selection from WhatsOnChain
- ✅ **BSV ForkID SIGHASH** signing algorithm (the breakthrough!)
- ✅ Multi-miner broadcasting (WhatsOnChain + GorillaPool)
- ✅ **Confirmed on-chain transactions**: `7dce601f...` and `155c2539...`

### Critical Technical Discovery: BSV ForkID SIGHASH

**The Problem:** Initial implementations using Legacy Bitcoin SIGHASH and BIP143 (SegWit) failed.

**The Solution:** BSV uses a unique ForkID SIGHASH algorithm that includes:
- `prev_value` (8 bytes) in the preimage - **THE MISSING PIECE**
- SIGHASH_ALL_FORKID = 0x41 (0x01 | 0x40)
- Double SHA256 (SHA256d) for final hash

**Reference:** Discovered by examining BSV Go SDK source code:
- `github.com/bsv-blockchain/go-sdk@v1.2.9/transaction/signaturehash.go`
- Function: `CalcInputPreimage()`

See [RUST_WALLET_SESSION_SUMMARY.md](RUST_WALLET_SESSION_SUMMARY.md) for complete technical details.

---

## 🏗️ Current Architecture

### Rust Wallet Backend (Production)

```
┌─────────────────────────────────────────┐
│          Rust Wallet (Port 3301)        │
│                                         │
│ • Actix-web HTTP Server                │
│ • BRC-100 Groups A & B (Complete)      │
│ • Custom BSV ForkID SIGHASH            │
│ • BRC-103/104 Authentication           │
│ • BRC-29 Payment Protocol              │
│ • Transaction History & Actions        │
│ • BEEF Phase 2 Parser                  │
│ • BRC-33 Message Relay                 │
│                                         │
│ ✅ PRODUCTION READY                    │
└──────────────────┬──────────────────────┘
                   │
                   ▼
         wallet.json Storage
      (%APPDATA%/HodosBrowser/wallet/)
```

### System Components:

```
┌─────────────────────────────────────────┐
│  CEF Browser Shell (C++)                │
│  • HTTP Request Interceptor             │
│  • Domain Whitelist Integration         │
│  • Routes to Port 3301                  │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│  Wallet Backend (Port 3301)             │
│  [Go OR Rust - pick one]                │
│  • BRC-100 Authentication               │
│  • Transaction Signing                  │
│  • Multi-Miner Broadcasting             │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│  Bitcoin SV Network                     │
│  • WhatsOnChain API                     │
│  • GorillaPool mAPI                     │
└─────────────────────────────────────────┘
```

**Key Architecture Details:**
- **Process-Per-Overlay**: Each overlay (settings, wallet, backup) runs in isolated CEF subprocess
- **Domain Whitelist**: C++ HTTP interceptor checks whitelist before allowing requests
- **Async CEF HTTP Client**: Thread-safe HTTP communication using CEF's native methods

For complete architecture documentation, see [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md).

---

## 📁 Current File Structure

```
babbage-browser/
├── cef-native/              # C++ CEF browser shell
│   ├── src/
│   │   ├── core/
│   │   │   └── HttpRequestInterceptor.cpp  # HTTP interception
│   │   └── handlers/
│   │       └── simple_handler.cpp          # CEF event handlers
│   └── build/               # CMake build artifacts
│
├── frontend/                # React + Vite UI
│   └── src/
│       ├── components/      # Wallet UI components
│       ├── hooks/           # React hooks
│       └── types/           # TypeScript types
│
├── rust-wallet/            # Rust wallet implementation ✅ PRODUCTION READY
│   ├── src/
│   │   ├── main.rs         # Actix-web server
│   │   ├── handlers.rs     # BRC-100 endpoints (2171 lines)
│   │   ├── json_storage.rs # wallet.json management
│   │   ├── crypto/         # BRC-42/43 implementations
│   │   │   ├── brc42.rs
│   │   │   └── brc43.rs
│   │   └── transaction/    # Transaction signing
│   │       ├── types.rs
│   │       └── sighash.rs  # BSV ForkID SIGHASH
│   └── Cargo.toml
│
└── reference/              # Reference implementations
    ├── ts-brc100/          # TypeScript SDK reference
    └── go-wallet-toolbox/  # Go SDK reference
```

---

## 🎯 Development Priorities

### Immediate (Current Session):
1. **Fix BRC-104 Signature Verification** - Debug signature verification failure
2. **Implement `/verifySignature`** - Complete BRC-84 signature verification
3. **Test with ToolBSV** - Validate complete authentication flow

### Short-term (Next 2-3 Sessions):
1. **Frontend Integration** - Connect React UI to Rust wallet endpoints
2. **Transaction UI** - Complete send/receive flows in browser
3. **Balance Display** - Real-time balance updates with USD conversion
4. **Transaction History** - Display past transactions

### Medium-term (Next Month):
1. **Domain Approval Modal** - Replace placeholder with real modal
2. **Wallet UI Polish** - Improve design and user experience
3. **Error Handling** - Comprehensive error messages and recovery
4. **Testing** - Complete end-to-end testing with multiple BRC-100 sites

### Long-term (Production):
1. **Complete BRC-100** - Implement remaining Groups C, D, E
2. **Security Audit** - Professional security review
3. **Performance Optimization** - Profile and optimize hot paths
4. **Build System** - Production build configuration

---

## 🔑 Key Technical Decisions

### 1. Native Wallet Backend (Not JavaScript)
**Decision:** Wallet operations run in isolated Rust daemon, not in browser JavaScript.

**Rationale:**
- **Security**: Private keys never exposed to render process
- **Process Isolation**: Even if website compromises render process, wallet is safe
- **Memory Protection**: Native processes provide stronger memory protection
- **Attack Surface**: Significantly reduced compared to JavaScript-based wallets

### 2. Process-Per-Overlay Architecture
**Decision:** Each overlay (settings, wallet, backup) runs in separate CEF subprocess.

**Rationale:**
- **State Isolation**: No state pollution between overlays
- **Security**: Process boundaries provide additional security
- **Stability**: Crash in one overlay doesn't affect others
- **Mimics Brave**: Based on Brave Browser's security architecture

### 3. Shared Wallet Storage
**Decision:** Rust wallet uses `wallet.json` file for storage.

**Rationale:**
- **Persistence**: JSON file provides simple, portable storage
- **Portability**: Easy to backup and migrate wallet data
- **Development**: Simple to inspect and debug wallet state

### 4. Port 3301 Standard
**Decision:** Use port 3301 for wallet daemon (BRC-100 standard).

**Rationale:**
- **Standard**: Matches other BRC-100 wallets (Metanet Desktop uses 3321)
- **Discovery**: Websites can discover wallet on standard port
- **Compatibility**: Works with existing BRC-100 sites

---

## 🧪 Testing Sites

### ✅ Working Sites:
- **ToolBSV.com** - Standard BRC-100 endpoints working
- **thryll.online** - Domain whitelisting working
- **Rust Wallet Transactions** - Multiple confirmed on-chain transactions

### ❌ Sites with Issues:
- **ToolBSV.com** - BRC-104 signature verification failing (current issue)

### 🎯 Test Coverage:
- ✅ Domain whitelisting
- ✅ HTTP request interception
- ✅ Transaction creation and signing
- ✅ Multi-miner broadcasting
- ✅ UTXO fetching and management
- ❌ Complete BRC-104 authentication flow (debugging)

---

## 📋 Quick Command Reference

### Start Rust Wallet:
```bash
cd rust-wallet
cargo run
# Server starts on http://127.0.0.1:3301
```


### Build CEF Browser:
```bash
cd cef-native/build
cmake --build . --config Release
./bin/Release/BitcoinBrowserShell.exe
```

### Start Frontend Dev Server:
```bash
cd frontend
npm install
npm run dev
# Frontend at http://127.0.0.1:5137
```

---

## 🔗 Documentation References

- **[PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md)** - Complete architecture and design philosophy
- **[README.md](README.md)** - Project overview and setup instructions
- **[RUST_WALLET_SESSION_SUMMARY.md](RUST_WALLET_SESSION_SUMMARY.md)** - Detailed Rust wallet implementation
- **[API_REFERENCES.md](API_REFERENCES.md)** - API endpoint documentation
- **[BUILD_INSTRUCTIONS.md](BUILD_INSTRUCTIONS.md)** - Build system configuration

---

## 🚧 Known Issues

### Active Issues:
1. **BRC-104 Signature Verification** - ToolBSV frontend signature verification failing
2. **`/verifySignature` Endpoint** - Stubbed out, needs BRC-84 implementation

### Deferred Issues:
1. **Overlay HWND Movement** - Overlay windows don't follow main window (low priority)
2. **Transaction History** - Not yet implemented
3. **Advanced Address Management** - Gap limit, pruning, high-volume generation

---

## 📝 Session Notes

### Current Session (2025-10-21):
- **Started**: Debugging BRC-104 signature verification
- **Progress**: Tried multiple fixes for nonce concatenation and key derivation
- **Current**: Still failing signature verification
- **Next**: Add detailed logging to compare with TypeScript SDK

### Previous Session (2025-10-16):
- **Completed**: BSV ForkID SIGHASH implementation
- **Achieved**: Multiple confirmed on-chain transactions
- **Status**: Rust wallet transaction system fully working

### Current Session (2025-10-27):
- **Completed**: BRC-100 Group B Transaction Management (Complete!)
- **Achieved**: Action storage system, transaction history, BEEF Phase 2 parsing
- **Status**: Ready for real-world testing with production apps

#### **What We Built This Session:**

**1. Action Storage System** ✅
- Created `action_storage.rs` - Complete transaction history management
- JSON file persistence with CRUD operations
- Transaction status tracking (Created → Signed → Unconfirmed → Confirmed)
- TXID update handling (transactions change ID after signing)
- Thread-safe Mutex integration with AppState

**2. BRC-100 Group B Endpoints** ✅
- `abortAction` - Cancel pending/unconfirmed transactions
- `listActions` - Transaction history with label filtering and pagination
- `internalizeAction` Phase 2 - Full BEEF parsing with output ownership detection

**3. Transaction Lifecycle Integration** ✅
- `createAction` now stores actions with `Created` status
- `signAction` updates TXID and status to `Signed`
- `processAction` updates status to `Unconfirmed` or `Failed`
- Labels support for categorizing transactions
- Address parsing from P2PKH scripts

**4. Confirmation Tracking** ✅
- WhatsOnChain API integration for confirmation status
- `update_confirmations()` function to query transaction status
- Manual `/updateConfirmations` endpoint for triggering updates
- Automatic status transitions: Unconfirmed → Confirmed

**5. BEEF Phase 2 - Full Transaction Parsing** ✅
- Created `beef.rs` module for BEEF and raw transaction parsing
- `ParsedTransaction` with detailed input/output structures
- Output ownership detection using wallet addresses
- Received amount calculation for incoming transactions
- Fallback to raw transaction if not BEEF format

**6. Testing & Debugging** ✅
- Created 5 PowerShell test scripts for integration testing
- Fixed UTF-8 BOM issue in JSON file creation
- Fixed TXID immutability issue (ID changes after signing)
- Fixed PowerShell parsing errors and emoji encoding issues
- Tested complete transaction flow end-to-end

#### **Key Technical Achievements:**

**TXID Immutability Handling:**
```rust
// Critical discovery: TXID changes after signing inputs
// Solution: update_txid method to track reference number → new TXID
pub fn update_txid(&mut self, reference_number: &str, new_txid: String, new_raw_tx: String)
```

**Output Ownership Detection:**
```rust
// Determines if transaction outputs belong to our wallet
fn is_output_ours(script_bytes: &[u8], our_addresses: &[AddressInfo]) -> bool {
    // Extract pubkey hash from P2PKH script
    // Compare against all wallet addresses
}
```

**BEEF Format Detection:**
```rust
// Try BEEF first, fall back to raw transaction
match crate::beef::Beef::from_hex(&req.tx) {
    Ok(beef) => /* Extract main transaction from BEEF */,
    Err(_) => /* Parse as raw hex */,
}
```

#### **Implementation Status:**

**BRC-100 Group B (Transaction Operations) - COMPLETE!** ✅
- ✅ `createAction` - Build unsigned transactions (with action storage)
- ✅ `signAction` - Sign transactions (with TXID update)
- ✅ `processAction` - Full flow: create + sign + broadcast
- ✅ `abortAction` - Cancel pending transactions
- ✅ `listActions` - Transaction history with filtering
- ✅ `internalizeAction` - Accept incoming BEEF transactions (Phase 2)

**Additional Features:**
- ✅ Transaction status tracking (7 states)
- ✅ Labels for transaction categorization
- ✅ Address extraction from scripts
- ✅ Confirmation tracking via WhatsOnChain
- ✅ BEEF format parsing with ancestry
- ✅ Output ownership detection
- ✅ Received amount calculation

#### **Files Created/Modified:**

**New Files:**
- `rust-wallet/src/action_storage.rs` - Transaction storage system (416 lines)
- `rust-wallet/src/beef.rs` - BEEF and transaction parser (359 lines)
- `rust-wallet/test_actions.ps1` - Action storage tests
- `rust-wallet/test_transaction_flow.ps1` - End-to-end transaction tests
- `rust-wallet/test_internalize.ps1` - Incoming transaction tests
- `rust-wallet/test_labels.ps1` - Label filtering tests
- `rust-wallet/test_beef_phase2.ps1` - BEEF Phase 2 tests
- `rust-wallet/BEEF_IMPLEMENTATION.md` - BEEF Phase 2 documentation

**Modified Files:**
- `rust-wallet/src/main.rs` - Integrated action_storage into AppState
- `rust-wallet/src/handlers.rs` - Implemented all Group B endpoints (3103 lines)
- `rust-wallet/Cargo.toml` - Added dependencies (uuid, chrono, bs58)

#### **Testing Results:**

All integration tests passing! ✅
- ✓ Action storage CRUD operations
- ✓ Transaction creation with labels
- ✓ Transaction signing with TXID update
- ✓ Transaction abortion
- ✓ History listing with filters
- ✓ Confirmation status updates
- ✓ BEEF parsing with raw fallback
- ✓ Output ownership detection
- ✓ Received amount calculation

---

**Last Updated:** December 2, 2025
**Current Focus:** ✅ **Phase 4 Complete!** Ready for Phase 5 (BEEF/SPV Caching)
**Major Achievement:** ✅ **Phase 4 UTXO Management Complete!** Database-backed UTXO caching, background sync, new address detection, and error handling all working!
**Next Session:** Phase 5 - Parent transaction and TSC proof caching for BEEF building

---

## 🗄️ **Phase 4: UTXO Management - COMPLETE!** (2025-12-02)

### **Latest Achievement: Database-Backed UTXO Caching with Background Sync**

Successfully implemented Phase 4 of the database migration, enabling fast UTXO lookups, background synchronization, and proper spending tracking!

### **What We Built:**

#### **1. UTXO Repository** ✅
**File**: `rust-wallet/src/database/utxo_repo.rs`

**Key Features:**
- `upsert_utxos()` - Insert or update UTXOs in database
- `get_unspent_by_addresses()` - Fast lookup of unspent UTXOs
- `mark_spent()` - Track UTXO spending with `spent_txid` and `spent_at`
- `calculate_balance()` - Sum unspent UTXOs for balance calculation
- `cleanup_spent_utxos()` - Optional cleanup of old spent UTXOs

#### **2. Database Integration** ✅
**Files**: `rust-wallet/src/handlers.rs`

**Updated Handlers:**
- `wallet_balance` - Now calculates from database cache, fetches from API if cache empty
- `createAction` - Uses database UTXOs first, falls back to API if needed
- `signAction` - Marks spent UTXOs in database when transaction is signed

**Key Improvements:**
- ✅ Fast balance checks (no API calls if cache populated)
- ✅ Automatic cache population on first use
- ✅ Change address generation (privacy: new address for each change)
- ✅ UTXO spending tracking (prevents double-spend attempts)

#### **3. Address Management** ✅
**File**: `rust-wallet/src/database/address_repo.rs`

**Features:**
- Automatic address marking as "used" when UTXOs are found
- Proper address indexing and tracking
- Support for gap limit scanning (ready for background sync)

### **Current Status:**
- ✅ **UTXO Caching**: Working - UTXOs stored in database
- ✅ **Balance Calculation**: Working - Fast calculation from cache (only fetches if cache empty)
- ✅ **UTXO Spending**: Working - Spent UTXOs tracked in database
- ✅ **Change Address Privacy**: Fixed - New address for each change output
- ✅ **Background Sync**: Complete - Runs every 5 minutes with gap limit (20 addresses)
- ✅ **New Address Detection**: Complete - Pending address cache checks new addresses on balance requests
- ✅ **Error Handling**: Complete - Retry logic with exponential backoff for API failures
- ✅ **Rate Limiting Protection**: Complete - 100ms delay between requests

### **Performance:**
- ✅ **Fast balance checks** - Uses database cache (no API calls unless cache empty)
- ✅ **Fast transaction creation** - Uses cached UTXOs
- ✅ **Automatic updates** - Background sync keeps cache fresh every 5 minutes
- ✅ **Immediate detection** - New addresses checked on next balance request

### **What's Next (Phase 5):**
- 🔄 **Parent Transaction Caching**: Cache parent transactions for BEEF building
- 🔄 **TSC Proof Caching**: Cache Merkle proofs for SPV verification
- 🔄 **Block Header Caching**: Cache block headers for height resolution

---

## ⚠️ **Testing Strategy & Concerns** (2025-01-XX)

### **Internal Testing Problem: Confirmation Bias in Tests**

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

## 📋 **Development Process Notes** (2025-12-08)

### **Database Migrations**

**How Migrations Work**:
- Migrations run **automatically** when the wallet starts (`WalletDatabase::new()` in `rust-wallet/src/database/connection.rs`)
- Migration system tracks version numbers in `schema_version` table
- Migrations are incremental (v1, v2, v3, v4, v5, v6, v7...)
- Uses `CREATE TABLE IF NOT EXISTS` and `ALTER TABLE` with existence checks - **safe for existing data**

**After Adding New Tables/Columns**:
1. Add migration function to `rust-wallet/src/database/migrations.rs` (e.g., `create_schema_v7()`)
2. Add migration step to `WalletDatabase::migrate()` in `connection.rs`
3. **Restart the wallet** - migrations run automatically on startup
4. ✅ **No manual migration command needed** - it's automatic!

**Migration Safety**:
- ✅ Uses `CREATE TABLE IF NOT EXISTS` - won't overwrite existing tables
- ✅ Uses existence checks before `ALTER TABLE` - won't duplicate columns
- ✅ Tracks version numbers - won't run migrations twice
- ✅ **Existing data is preserved** - migrations only add new structures

**Example**: After adding `output_tags` tables in migration v1, the wallet will automatically create them on next startup. No manual intervention needed!

**Note**: The `output_tags` and `output_tag_map` tables are in migration v1, but if the database was created before v1 included them, you need to restart the wallet to run migrations. The error "no such table: output_tags" indicates migrations haven't run yet.

### **Database Export/Import Procedures** ⚠️ **TODO: Review After Schema Changes**

**Status**: Database schema has been updated with new tables and columns (v7: certificate_fields, enhanced certificates table).

**Action Required**:
- ⚠️ **Review database export/import procedures** to ensure they handle:
  - New `certificate_fields` table
  - New BRC-52 columns in `certificates` table (`type`, `serial_number`, `certifier`, `subject`, `verifier`, `revocation_outpoint`, `signature`, `is_deleted`)
  - Data migration from `relinquished` to `is_deleted` (if applicable)
  - Foreign key relationships between `certificates` and `certificate_fields`

**Files to Review**:
- `rust-wallet/src/handlers.rs` - Backup/restore endpoints
- `rust-wallet/src/database/connection.rs` - Database initialization and migration
- Any JSON export functionality that includes certificate data

**When**: Review before implementing certificate management features (Part 3) to ensure backup/restore works correctly with new schema.

### **HTTP Interceptor Updates**

**When Implementing New Endpoints**:
- Add endpoint to `isWalletEndpoint()` in `cef-native/src/core/HttpRequestInterceptor.cpp`
- This ensures requests to the endpoint are intercepted and routed to the wallet
- **Example**: Added `/discoverByIdentityKey` after finding it in real-world testing logs

**Current Status**: All Group C endpoints have been added to the interceptor proactively:
- ✅ `/listOutputs`, `/relinquishOutput`
- ✅ `/acquireCertificate`, `/listCertificates`, `/proveCertificate`, `/relinquishCertificate`
- ✅ `/discoverByIdentityKey`, `/discoverByAttributes`
- ✅ `/getHeight`, `/getHeaderForHeight`
- ✅ `/waitForAuthentication`, `/getNetwork` (already existed)

---

## ✅ **FIXED: Transaction Error Handling** (2025-12-02)

### **Status:** ✅ **RESOLVED**

**What Was Fixed:**
- Frontend now properly checks `result.success === false || result.status === 'failed'` instead of HTTP status codes
- TransactionForm.tsx (line 221) and WalletPanelContent.tsx (line 284-285) now correctly display error messages when transactions fail
- Error modal properly shows failure status with error details when backend returns failure

**Remaining Investigation:**
- Script verification errors (e.g., `OP_EQUALVERIFY` failure) may still occur at backend level and need investigation, but frontend now correctly displays these errors to users
