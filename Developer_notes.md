# Developer Notes - Bitcoin Browser

> **Quick Reference:** See [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md) for architecture details and [README.md](README.md) for setup instructions.

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

**Last Updated:** October 30, 2025
**Current Focus:** ✅ **BRC-29 PAYMENTS WORKING!** Complete transaction system with real-world testing!
**Major Achievement:** BRC-29 payment protocol, TSC Merkle proofs, and Atomic BEEF all working with ToolBSV!
**Next Session:** Additional testing with other sites, then move to Phase 3 (Output/UTXO Management)
