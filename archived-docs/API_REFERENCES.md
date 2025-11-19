# API References - Bitcoin Browser

## 🎯 Overview

This document outlines all API interfaces in the Babbage Browser project, including frontend-backend communication, wallet operations, and blockchain integration.

## 🌐 HTTP Request Interception APIs ✅ PRODUCTION READY (2025-10-02)

### External Website Communication
```
External Website → HTTP Request → CEF Interceptor → Go Wallet Daemon → Response → Frontend
```

### Supported Endpoints:
- `GET /wallet/status` - Wallet status and availability
- `GET /brc100/status` - BRC-100 service status
- `POST /brc100/auth/challenge` - Authentication challenges
- `POST /brc100/auth/authenticate` - Authentication responses
- `POST /brc100/beef/create` - BEEF transaction creation
- `POST /brc100/beef/broadcast` - BEEF transaction broadcasting
- `POST /domain/whitelist/add` - Add domain to whitelist
- `GET /domain/whitelist/check?domain=<domain>` - Check if domain is whitelisted
- `POST /domain/whitelist/record` - Record request from domain
- `GET /domain/whitelist/list` - List all whitelisted domains
- `POST /domain/whitelist/remove` - Remove domain from whitelist
- `POST /.well-known/auth` - BRC-104 authentication endpoint (BRC-42/43 signatures)
- `GET /socket.io/` - Engine.IO polling endpoint for Socket.IO
- `POST /listMessages` - BRC-33 PeerServ message listing
- `POST /sendMessage` - BRC-33 PeerServ message sending
- `POST /acknowledgeMessage` - BRC-33 PeerServ message acknowledgment
- All other Go wallet daemon endpoints

### Technical Implementation:
- **Thread-Safe**: Uses CEF's task system for proper thread communication
- **Async Operations**: Non-blocking HTTP requests using `CefURLRequest`
- **CORS Support**: Proper cross-origin headers for external websites
- **Domain Verification**: Automatic domain whitelist checking before request processing
- **Domain Extraction**: Uses main frame URL for consistent domain identification
- **User Approval**: Domain approval modal for non-whitelisted domains (placeholder)
- **Error Handling**: Comprehensive error handling with fallback responses
- **Resource Management**: Proper cleanup and memory management
- **Logging**: Structured logging with project's Logger class

## 📱 Frontend ↔ CEF Bridge APIs

### Identity Management
```typescript
// Wallet identity operations
window.bitcoinBrowser.identity.get(): Promise<IdentityData>
window.bitcoinBrowser.identity.create(): Promise<IdentityData>
window.bitcoinBrowser.identity.markBackedUp(): Promise<boolean>
window.bitcoinBrowser.identity.authenticate(challenge: string): Promise<AuthResponse>
```

## 🔐 BRC-100 Wallet Integration APIs 🎯 IN DEVELOPMENT

### BRC-100 Wallet Interface (Ports 3301/3321)
```
BRC-100 Website → HTTP POST → localhost:3301/3321 → Go Wallet Daemon → Response
```

### Planned Endpoints:
- `GET /getVersion` - Get wallet version and capabilities
- `GET /getPublicKey` - Get wallet's public key for BRC-100 operations
- `POST /createAction` - Create BRC-100 actions (transfers, etc.)
- `POST /signAction` - Sign BRC-100 actions
- `POST /processAction` - Process completed BRC-100 actions

### Expected Response Format:
```json
{
  "version": "BitcoinBrowserWallet v0.0.1",
  "capabilities": ["getVersion", "getPublicKey", "createAction", "signAction", "processAction"],
  "brc100": true,
  "timestamp": "2024-01-01T00:00:00Z"
}
```

### Security Features:
- **Domain Whitelist**: All BRC-100 requests checked against domain whitelist
- **User Approval**: Authentication modal for non-whitelisted domains
- **CORS Support**: Proper cross-origin headers for external websites
- **Request Validation**: JSON validation and domain verification

### Implementation Status:
- **Phase 1**: HTTP JSON-RPC endpoints (getVersion, getPublicKey, createAction) 🎯 **CURRENT**
- **Phase 2**: Domain whitelist integration and security
- **Phase 3**: Frontend authentication modal integration
- **Phase 4**: Real-world testing with BRC-100 websites

## 🔐 BRC-100 Authentication APIs ✅ PRODUCTION READY

### Identity Certificate Management
```typescript
// Generate BRC-100 identity certificate
window.bitcoinBrowser.brc100.identity.generate(data: IdentityRequest): Promise<BRC100Response>

// Validate identity certificate
window.bitcoinBrowser.brc100.identity.validate(certificate: IdentityCertificate): Promise<BRC100Response>

// Create selective disclosure
window.bitcoinBrowser.brc100.identity.selectiveDisclosure(data: SelectiveDisclosureRequest): Promise<BRC100Response>

// Request structures
interface IdentityRequest {
  subject: string;
  attributes: Record<string, any>;
}

interface SelectiveDisclosureRequest {
  identityData: Record<string, any>;
  fields: string[];
}

interface BRC100Response {
  success: boolean;
  data?: Record<string, any>;
  error?: string;
}
```

### Authentication Flow
```typescript
// Generate authentication challenge
window.bitcoinBrowser.brc100.auth.challenge(appId: string): Promise<BRC100Response>

// Authenticate with challenge response
window.bitcoinBrowser.brc100.auth.authenticate(request: AuthRequest): Promise<BRC100Response>

// Derive Type-42 keys for P2P communication
window.bitcoinBrowser.brc100.auth.type42(keys: KeyDerivationRequest): Promise<BRC100Response>

interface AuthRequest {
  appId: string;
  challenge: string;
  response: string;
  sessionId?: string;
  identityId?: string;
}

interface KeyDerivationRequest {
  walletPublicKey: string;
  appPublicKey: string;
}
```

### Session Management
```typescript
// Create authentication session
window.bitcoinBrowser.brc100.session.create(request: SessionRequest): Promise<BRC100Response>

// Validate session
window.bitcoinBrowser.brc100.session.validate(sessionId: string): Promise<BRC100Response>

// Revoke session
window.bitcoinBrowser.brc100.session.revoke(sessionId: string): Promise<BRC100Response>

interface SessionRequest {
  identityId: string;
  appId: string;
}
```

### BEEF Transactions
```typescript
// Create BRC-100 BEEF transaction
window.bitcoinBrowser.brc100.beef.create(request: BEEFRequest): Promise<BRC100Response>

// Verify BRC-100 BEEF transaction
window.bitcoinBrowser.brc100.beef.verify(transaction: BRC100BEEFTransaction): Promise<BRC100Response>

// Convert and broadcast BEEF
window.bitcoinBrowser.brc100.beef.broadcast(transaction: BRC100BEEFTransaction): Promise<BRC100Response>

interface BEEFRequest {
  actions: BRC100Action[];
  sessionId?: string;
}
```

### SPV Verification
```typescript
// Verify identity with SPV
window.bitcoinBrowser.brc100.spv.verify(request: SPVRequest): Promise<BRC100Response>

// Create SPV identity proof
window.bitcoinBrowser.brc100.spv.proof(request: SPVRequest): Promise<BRC100Response>

interface SPVRequest {
  transactionId: string;
  identityData: Record<string, any>;
}
```

### Service Status
```typescript
// Get BRC-100 service status
window.bitcoinBrowser.brc100.status(): Promise<BRC100Response>
```

### Transaction Management ✅ PRODUCTION READY
```typescript
// Unified transaction operations (create + sign + broadcast)
window.bitcoinAPI.sendTransaction(data: TransactionData): Promise<TransactionResponse>

// Transaction data structure
interface TransactionData {
  toAddress: string;
  amount: number;        // in satoshis
  feeRate: number;       // sat/byte
}

// Transaction response structure
interface TransactionResponse {
  success: boolean;
  txid: string;
  message: string;
  whatsOnChainUrl: string;
}

// Legacy transaction operations (DEPRECATED - use sendTransaction instead)
window.bitcoinBrowser.transactions.create(txData: TransactionData): Promise<Transaction>
window.bitcoinBrowser.transactions.sign(txId: string): Promise<Signature>
window.bitcoinBrowser.transactions.broadcast(tx: Transaction): Promise<BroadcastResult>
window.bitcoinBrowser.transactions.getHistory(): Promise<Transaction[]>
```

### Balance & UTXO Management ✅ PRODUCTION READY
```typescript
// Balance operations (total across all addresses)
window.bitcoinAPI.getBalance(): Promise<BalanceResponse>

// Balance response structure
interface BalanceResponse {
  balance: number;        // total balance in satoshis
  usdValue?: number;      // USD equivalent (if price available)
}

// Address operations
window.bitcoinBrowser.address.generate(): Promise<AddressData>
window.bitcoinBrowser.address.getCurrent(): Promise<string>
window.bitcoinBrowser.address.getAll(): Promise<AddressData[]>

// Address data structure
interface AddressData {
  address: string;
  index: number;
  balance: number;        // balance for this specific address
}

// Legacy UTXO operations (DEPRECATED - use getBalance for total balance)
window.bitcoinBrowser.utxos.list(): Promise<UTXO[]>
window.bitcoinBrowser.utxos.refresh(): Promise<void>
```

### Process-Per-Overlay Management
```typescript
// Process-per-overlay operations (NEW ARCHITECTURE)
window.cefMessage.send(messageName: string, args: any[]): void

// Overlay-specific messages
window.cefMessage.send('overlay_show_settings', []): void
window.cefMessage.send('overlay_show_wallet', []): void
window.cefMessage.send('overlay_show_backup', []): void
window.cefMessage.send('overlay_close', []): void

// Legacy overlay operations (DEPRECATED - being phased out)
window.bitcoinBrowser.overlay.show(panelName: string): void
window.bitcoinBrowser.overlay.hide(): void
window.bitcoinBrowser.overlay.openPanel(panelName: string): void
```

### Navigation Control
```typescript
// Browser navigation
window.bitcoinBrowser.navigation.navigate(url: string): void
window.bitcoinBrowser.navigation.goBack(): void
window.bitcoinBrowser.navigation.goForward(): void
window.bitcoinBrowser.navigation.reload(): void
```

## 🔄 CEF Message System (UPDATED FOR PROCESS-PER-OVERLAY)

### Process Message Types
```cpp
// CEF process messages for frontend-backend communication

// Overlay Management (NEW ARCHITECTURE)
"overlay_show_settings"    // Create settings overlay in separate process
"overlay_show_wallet"      // Create wallet overlay in separate process
"overlay_show_backup"      // Create backup modal overlay in separate process
"overlay_close"           // Close current overlay window

// Identity Management (NEW SYSTEM)
"identity_status_check"        // Check if identity exists and needs backup
"identity_status_check_response" // Response with identity status
"create_identity"              // Create new identity via Go daemon
"create_identity_response"     // Response with identity data
"mark_identity_backed_up"      // Mark identity as backed up
"mark_identity_backed_up_response" // Response confirmation

// Utility Messages
"force_repaint"           // Force CEF to repaint overlay content

// Legacy Messages (DEPRECATED)
"navigate"           // Navigate to URL
"overlay_open_panel" // Open overlay panel (DEPRECATED)
"overlay_show"       // Show overlay window (DEPRECATED)
"overlay_hide"       // Hide overlay window (DEPRECATED)
"overlay_input"      // Toggle mouse input
"address_generate"   // Generate new address
"identity_get"       // Get wallet identity (DEPRECATED)
"identity_backup"    // Mark wallet as backed up (DEPRECATED)
```

### Message Response System
```typescript
// Frontend listens for responses via CustomEvent
window.addEventListener('cefMessageResponse', (event) => {
    const { message, args } = event.detail;

    switch (message) {
        case 'identity_status_check_response':
            const status = JSON.parse(args[0]);
            // Handle identity status
            break;
        case 'create_identity_response':
            const identity = JSON.parse(args[0]);
            // Handle identity creation
            break;
        case 'mark_identity_backed_up_response':
            const result = JSON.parse(args[0]);
            // Handle backup confirmation
            break;
    }
});

// API Ready Event
window.addEventListener('bitcoinBrowserReady', () => {
    // bitcoinBrowser API is fully injected and ready
    // Safe to make API calls
});
```

## 🔧 CEF ↔ Go Bridge APIs

### Wallet Operations
```cpp
// C++ to Go wallet communication
class BitcoinWalletHandler {
    // Create new wallet
    std::string createWallet(const std::string& password);

    // Load existing wallet
    std::string loadWallet(const std::string& privateKey);

    // Sign transaction
    std::string signTransaction(const std::string& txData);

    // Get wallet balance
    std::string getBalance();

    // Generate new address
    std::string generateAddress();
};
```

### BRC-100 Authentication
```cpp
// BRC-100 authentication operations
class BRC100Handler {
    // Authenticate with BRC-100
    std::string authenticate(const std::string& challenge);

    // Verify certificate
    bool verifyCertificate(const std::string& certData);

    // Create selective disclosure
    std::string selectiveDisclosure(const std::vector<std::string>& fields);
};
```

## 🐹 Go Wallet Backend APIs

### Core Wallet Functions
```go
type BitcoinWallet struct {
    password string
    // ... other fields
}

func NewBitcoinWallet(password string) *BitcoinWallet {
    // Initialize wallet with password
}

func (w *BitcoinWallet) CreateWallet() (map[string]interface{}, error) {
    // Create new wallet and return identity data
}

func (w *BitcoinWallet) LoadWallet(privateKey string) (map[string]interface{}, error) {
    // Load existing wallet from private key
}

func (w *BitcoinWallet) SignTransaction(txData map[string]interface{}) (string, error) {
    // Sign transaction using bitcoin-sv/go-sdk
}

func (w *BitcoinWallet) GetBalance() (int64, error) {
    // Get wallet balance from blockchain
}

func (w *BitcoinWallet) GenerateAddress() (string, error) {
    // Generate new receiving address
}

func (w *BitcoinWallet) GetUTXOs() ([]UTXO, error) {
    // Get unspent transaction outputs
}
```

### BEEF Transaction Support
```go
type BEEFHandler struct {
    // ... fields
}

func (h *BEEFHandler) CreateBEEFTransaction(inputs []UTXO, outputs []Output) (string, error) {
    // Create BEEF format transaction
}

func (h *BEEFHandler) VerifyBEEFTransaction(beefData string) (bool, error) {
    // Verify BEEF transaction
}

func (h *BEEFHandler) BroadcastBEEFTransaction(beefData string) (string, error) {
    // Broadcast BEEF transaction to miners
}
```

### SPV Verification
```go
type SPVHandler struct {
    // ... fields
}

func (h *SPVHandler) VerifyTransaction(txID string) (bool, error) {
    // Verify transaction using SPV
}

func (h *SPVHandler) GetMerkleProof(txID string) (map[string]interface{}, error) {
    // Get merkle proof for transaction
}

func (h *SPVHandler) VerifyMerkleProof(proof map[string]interface{}) (bool, error) {
    // Verify merkle proof
}
```

## 🔗 Go Daemon HTTP APIs ✅ PRODUCTION READY

### Wallet Management
```http
# Get total balance across all addresses
GET /wallet/balance
Response: {"balance": 29391}

# Generate new HD address
POST /wallet/address/generate
Response: {"address": "1ABC...", "index": 5}

# Get current address
GET /wallet/address/current
Response: {"address": "1ABC...", "index": 5}

# Get all addresses
GET /wallet/addresses
Response: [{"address": "1ABC...", "index": 0, "balance": 1000}, ...]
```

### BRC-104 Authentication (Babbage Protocol)
```http
# Mutual authentication endpoint
POST /.well-known/auth
Content-Type: application/json

Request:
{
  "version": "0.1",
  "messageType": "initialRequest",
  "identityKey": "03d575090cc073ecf448ad49fae79993fdaf8d1643ec2c5762655ed400e20333e3",
  "initialNonce": "base64_encoded_nonce",
  "requestedCertificates": {
    "certifiers": [],
    "types": {}
  }
}

Response:
{
  "version": "0.1",
  "messageType": "initialResponse",
  "identityKey": "03d575090cc073ecf448ad49fae79993fdaf8d1643ec2c5762655ed400e20333e3",
  "nonce": "our_base64_nonce",
  "yourNonce": "their_initial_nonce",
  "signature": "hex_encoded_signature"
}
```

**Technical Details:**
- **Signature**: BRC-42 derived key signs concatenated nonces (theirNonce + ourNonce)
- **Invoice Number**: `2-auth message signature-{initialNonce} {sessionNonce}`
- **Format**: DER-encoded ECDSA signature (not compact)
- **Curve**: secp256k1 (Bitcoin curve)
- **Key Derivation**: Uses master private key (not index 0 derived key)
- **Session Management**: Stores authentication nonces for subsequent API calls
- **Concurrent Sessions**: Supports multiple simultaneous auth sessions per identity

**Critical Implementation Notes:**
- **Nonce Generation**: Simple 32-byte random nonces (not HMAC-based)
- **KeyID Encoding**: Uses base64 to preserve binary data integrity
- **"self" Counterparty**: Uses raw master key (no BRC-42 derivation for HMAC)
- **Signature Verification**: Derives SIGNER's child public key (not our own)
- **External Backend Calls**: Skips session validation for app-to-backend API requests

### BRC-33 PeerServ Message Relay
```http
# Send message to recipient
POST /sendMessage
Content-Type: application/json

Request:
{
  "message": {
    "recipient": "028d37b9...",
    "messageBox": "payment_inbox",
    "body": "hello"
  }
}

Response:
{
  "status": "success",
  "messageId": 3301
}

# List messages from message box
POST /listMessages
Content-Type: application/json

Request:
{
  "messageBox": "payment_inbox"
}

Response:
{
  "status": "success",
  "messages": [
    {
      "messageId": 3301,
      "body": "hello",
      "sender": "028d37b9..."
    }
  ]
}

# Acknowledge received messages
POST /acknowledgeMessage
Content-Type: application/json

Request:
{
  "messageIds": [3301, 3302]
}

Response:
{
  "status": "success"
}
```

**Technical Details:**
- **Authentication**: Requires BRC-31 authentication (via `/.well-known/auth`)
- **Storage**: In-memory (not persistent across daemon restarts)
- **CORS**: Full CORS support for cross-origin requests
- **Thread-Safe**: Mutex-protected message storage

### Socket.IO / Engine.IO
```http
# Engine.IO polling handshake
GET /socket.io/?EIO=4&transport=polling&t={timestamp}

Response:
40{"sid":"session_1759866753834844300","upgrades":["websocket"],"pingTimeout":60000,"pingInterval":25000}
```

**Protocol Details:**
- **Packet Format**: `40` = Engine.IO open packet (4 = message, 0 = open)
- **Session ID**: Unique per connection
- **Upgrades**: Advertises WebSocket upgrade capability
- **Timeouts**: 60s ping timeout, 25s ping interval
- **Current Status**: Polling works ✅, WebSocket upgrade not attempted ❌

### Transaction Management
```http
# Send complete transaction (create + sign + broadcast)
POST /transaction/send
Content-Type: application/json

{
  "toAddress": "1ABC...",
  "amount": 1000,
  "feeRate": 5
}

Response: {
  "success": true,
  "txid": "bf089bece19a7fac4d7977ba95361075ecc0b87b76a5a68be3ed0e32e0b36286",
  "message": "Transaction sent successfully",
  "whatsOnChainUrl": "https://whatsonchain.com/tx/bf089bece19a7fac4d7977ba95361075ecc0b87b76a5a68be3ed0e32e0b36286"
}
```

### UTXO Management
```http
# Fetch UTXOs for specific address
GET /utxos/{address}
Response: [{"txid": "abc...", "vout": 0, "amount": 1000, "script": "76a9..."}]

# Get UTXO manager status
GET /utxos/status
Response: {"status": "active", "lastUpdate": "2025-09-27T12:43:16Z"}
```

## 🔧 Two Wallet Implementations (2025-10-16)

**⚠️ Both use Port 3301 - Only run ONE at a time!**

### **Comparing Two Implementations:**

#### 1. Go Wallet (BSV SDK Implementation)
**Purpose:** Leverage official BSV Go SDK for production-ready wallet
**Technology:** Go with `github.com/bsv-blockchain/go-sdk@v1.2.9`
**Port:** 3301
**Status:** ✅ Production-ready

**Endpoints:**
- `GET /health` - Health check
- `GET /wallet/info` - Wallet information
- `GET /wallet/balance` - Total balance across all addresses
- `POST /transaction/send` - Create, sign, and broadcast transaction
- `GET /brc100/status` - BRC-100 service status
- Plus all BRC-100 authentication endpoints

**Pros:**
- Official BSV SDK (tested and maintained)
- Comprehensive BEEF/SPV support
- Well-documented API

#### 2. Rust Wallet (Custom Implementation)
**Purpose:** Custom BRC-100 implementation for learning and flexibility
**Technology:** Rust with Actix-web, custom cryptography
**Port:** 3301
**Status:** ✅ **PRODUCTION READY** - Transaction signing working, authentication complete with 7 breakthroughs!

**Endpoints (BRC-100):**
- `GET /wallet/status` - Wallet status
- `POST /getVersion` - Wallet version and capabilities ✅
- `POST /getPublicKey` - Public key for identity ✅
- `POST /isAuthenticated` - Authentication status ✅
- `POST /createHmac` - Create HMAC for nonce verification ✅
- `POST /verifyHmac` - Verify HMAC ✅
- `POST /createSignature` - Sign arbitrary messages ✅ (with session validation)
- `POST /verifySignature` - Verify signatures ✅ (derives signer's child public key)
- `POST /.well-known/auth` - BRC-104 mutual authentication ✅
- `POST /createAction` - Build unsigned transaction ✅
- `POST /signAction` - Sign transaction with BSV ForkID SIGHASH ✅
- `POST /processAction` - Create + sign + broadcast transaction ✅
- `POST /abortAction` - Cancel pending transactions ✅
- `POST /listActions` - Transaction history with filters ✅
- `POST /internalizeAction` - Accept incoming BEEF transactions ✅
- `POST /updateConfirmations` - Manual confirmation status update ✅

**Endpoints (BRC-33 Message Relay):**
- `POST /sendMessage` - Send message to recipient ✅
- `POST /listMessages` - List messages from message box ✅
- `POST /acknowledgeMessage` - Acknowledge received messages ✅

**Pros:**
- Full control over implementation
- Custom BSV ForkID SIGHASH (verified working)
- Confirmed mainnet transactions
- Memory safe (Rust)
- **Complete BRC-103/104 authentication** (all 7 breakthroughs)
- **Complete transaction management** (Groups A & B)
- **Action storage system** (transaction history)
- **BEEF Phase 2 parser** (output ownership detection)
- **BRC-42 signature verification** (correctly derives signer's child public key)
- **Session management** (concurrent sessions supported)
- **Real-world tested** (ToolBSV working, Thryll ready)

**Shared Storage:**
Both wallets use the same `wallet.json` file:
- Location: `%APPDATA%/BabbageBrowser/wallet/wallet.json`
- Contains: Mnemonic, HD addresses, public keys, WIF private keys

**Production Decision Pending:**
- Testing both implementations
- Will choose one for final release
- Currently comparing performance, maintainability, and features

---

## 🌐 Bitcoin SV Blockchain APIs

### Miner Integration ✅ PRODUCTION READY

#### WhatsOnChain (Primary)
**Used by:** Both Go and Rust wallets
```http
POST https://api.whatsonchain.com/v1/bsv/main/tx/raw
Content-Type: application/json

{
  "txhex": "hex_encoded_transaction"
}
```

#### GorillaPool mAPI (Secondary)
**Used by:** Both Go and Rust wallets
```http
POST https://mapi.gorillapool.io/mapi/tx
Content-Type: application/json

{
  "rawtx": "hex_encoded_transaction"
}
```

**Response Format:**
```json
{
  "payload": "{\"txid\":\"155c2539...\",\"returnResult\":\"success\",...}",
  "signature": "30450221...",
  "publicKey": "03ad7801...",
  "encoding": "UTF-8",
  "mimetype": "application/json"
}
```

#### TAAL ARC (Not Used)
**Status:** Requires authentication, not included in current implementation
```http
POST https://arc.taal.com/v1/tx
Authorization: Bearer <API_KEY>
```

### Balance & UTXO Queries ✅ PRODUCTION READY

#### Address Balance (WhatsOnChain)
```http
GET https://api.whatsonchain.com/v1/bsv/main/address/{address}/balance
Response: {"balance": 29391, "unconfirmed": 0}
```

#### UTXO List (WhatsOnChain)
```http
GET https://api.whatsonchain.com/v1/bsv/main/address/{address}/unspent
Response: [{"txid": "abc...", "vout": 0, "amount": 1000, "script": "76a9..."}]
```

#### Transaction Details (WhatsOnChain)
```http
GET https://api.whatsonchain.com/v1/bsv/main/tx/{txid}/hex
Response: "0100000001..."
```

#### Legacy APIs (Deprecated)
```http
# TAAL Miner (deprecated)
GET https://api.taal.com/arc/address/{address}/balance
GET https://api.taal.com/arc/address/{address}/utxos
```

## 🔄 Message Flow Architecture ✅ PRODUCTION READY

### Frontend → C++ Bridge → Go Daemon
```typescript
// 1. Frontend calls unified API
const result = await window.bitcoinAPI.sendTransaction({
  toAddress: "1ABC...",
  amount: 1000,
  feeRate: 5
});

// 2. C++ bridge processes message
window.cefMessage.send('send_transaction', [JSON.stringify(data)]);

// 3. Go daemon handles complete transaction flow
POST /transaction/send → Create → Sign → Broadcast → Response
```

## 📊 Current Implementation Status

### ✅ Completed Features
- **HD Wallet System**: BIP44 hierarchical deterministic wallet
- **Transaction Flow**: Complete create + sign + broadcast pipeline
- **Balance Management**: Total balance calculation across all addresses
- **Address Generation**: HD address generation with proper indexing
- **Real Blockchain Integration**: Working with WhatsOnChain and GorillaPool
- **BRC-100 Authentication**: Complete BRC-100 protocol implementation
- **BEEF/SPV Integration**: Real blockchain transactions with SPV verification
- **Frontend Integration**: React UI fully connected to backend
- **Process Isolation**: Each overlay runs in dedicated CEF subprocess

### 🚧 In Development
- **Window Management**: Keyboard commands and overlay HWND movement
- **Transaction Receipt UI**: Improved confirmation and receipt display
- **Frontend BRC-100 Integration**: React authentication modals and approval flows

### 📋 Future Features
- **Transaction History**: Local storage and display
- **Advanced Address Management**: Gap limit, pruning, high-volume generation
- **SPV Verification**: Simplified Payment Verification implementation

## 🔐 BRC-100 Protocol APIs (Future)

### Authentication Flow
```typescript
// 1. App requests identity certificate
const certificate = await window.bitcoinBrowser.identity.getCertificate();

// 2. Wallet provides selective disclosure
const selectiveData = await window.bitcoinBrowser.identity.selectiveDisclosure([
  'publicKey',
  'address'
]);

// 3. App verifies certificate
const isValid = await window.bitcoinBrowser.identity.verifyCertificate(certificate);

// 4. Both parties derive shared keys (Type-42)
const sharedKey = await window.bitcoinBrowser.identity.deriveSharedKey(appPublicKey);

// 5. App creates BEEF transaction
const beefTx = await window.bitcoinBrowser.transactions.createBeef(transactionData);

// 6. Wallet signs BEEF transaction
const signedTx = await window.bitcoinBrowser.transactions.signBeef(beefTx);

// 7. App broadcasts transaction
const txId = await window.bitcoinBrowser.transactions.broadcast(signedTx);
```

## 📊 Data Types

### Identity Data
```typescript
interface IdentityData {
  publicKey: string;
  privateKey: string;  // Only in secure contexts
  address: string;
  backedUp: boolean;
  certificates?: Certificate[];
}
```

### Transaction Data
```typescript
interface TransactionData {
  inputs: UTXO[];
  outputs: Output[];
  fee: number;
  format: 'standard' | 'beef' | 'arc';
}
```

### UTXO
```typescript
interface UTXO {
  txid: string;
  vout: number;
  value: number;
  scriptPubKey: string;
}
```

### Certificate (BRC-52/103)
```typescript
interface Certificate {
  issuer: string;
  subject: string;
  publicKey: string;
  signature: string;
  selectiveDisclosure: string[];
  expiry?: Date;
}
```

## 📊 Current Implementation Status

### ✅ PRODUCTION READY (2025-10-03):
- **HTTP Request Interception**: Thread-safe async CEF HTTP client with domain verification
- **External Website Communication**: External websites can communicate with wallet daemon
- **Domain Whitelist Management**: Complete domain verification and whitelist system
- **Domain Verification in HTTP Interceptor**: C++ domain checking before request processing
- **BRC-100 Authentication APIs**: Complete BRC-100 protocol implementation
- **BEEF/SPV Integration**: Real blockchain transactions with SPV verification
- **Transaction Management**: Complete transaction creation, signing, and broadcasting
- **Balance & UTXO Management**: Real-time UTXO fetching and balance calculation
- **Go Daemon HTTP APIs**: All wallet and BRC-100 endpoints operational
- **Message Flow Architecture**: Complete frontend ↔ CEF ↔ Go daemon communication
- **CORS Support**: Cross-origin request handling for external websites

### 🎯 NEXT DEVELOPMENT PRIORITIES:
1. **Domain Approval Modal Integration**: Frontend modal system for domain verification
2. **JSON Validation & Security**: Enhanced request validation and security
3. **BRC-100 Standard Compliance**: Full compliance verification
4. **Production Security Features**: Rate limiting, logging, advanced CORS

## 🚀 Future API Considerations

### Multi-Platform Support
- 🟡 **Windows**: Current CEF implementation
- 🟡 **macOS**: CEF with Cocoa integration
- 🟡 **Mobile**: React Native with native wallet modules

### Advanced Features
- 🟡 **Hardware Security Module (HSM)** integration
- 🟡 **Multi-signature** wallet support
- 🟡 **Smart contract** interaction APIs
- 🟡 **Token gating** and access control

### Performance Optimizations
- 🟡 **Caching** strategies for balance and UTXO data
- 🟡 **Batch operations** for multiple transactions
- 🟡 **WebSocket** connections for real-time updates
- 🟡 **Offline mode** with transaction queuing

---

*This API reference will be updated as the project evolves and new features are implemented.*
