+----------------------------+
|        React UI Layer     |
|  - Panels / Pages / Hooks |
|  - TypeScript + Vite      |
|  - Transaction Forms      |
|  - Balance Display        |
|  - Address Management     |
+----------------------------+
            ↓
+----------------------------+
|   JS ↔ Native Bridge Layer |
|  - window.bitcoinBrowser   |
|  - window.cefMessage       |
|  - Process Communication   |
+----------------------------+
            ↓
+----------------------------+
|     Native CEF Shell       |
|  - C++ / Chromium          |
|  - CEF Handlers            |
|  - Process-Per-Overlay     |
|  - Message Routing         |
|  - HTTP Request Interception|
|  - Async HTTP Client       |
+----------------------------+
            ↓
+----------------------------+
|   Wallet Backend Layer     |
|  DUAL IMPLEMENTATION:      |
+----------------------------+
            |
   +--------+--------+
   |                 |
   v                 v
+----------------------------+  +----------------------------+
|   Go Wallet (Port 3301)    |  | Rust Wallet (Port 3301)    |
|  - bitcoin-sv/go-sdk       |  | - Actix-web HTTP server    |
|  - HD Wallet (BIP44)       |  | - BRC-103/104 auth         |
|  - BSV SDK tx signing      |  | - BSV ForkID SIGHASH       |
|  - Transaction handling    |  | - Custom crypto impl       |
|  - UTXO Management         |  | - Confirmed mainnet txs    |
+----------------------------+  +----------------------------+
            |                            |
            | (Only ONE runs at a time)  |
            +------------+---------------+
                         |
                         v
              Shared wallet.json
           (%APPDATA%/BabbageBrowser/wallet/)
            ↓
+----------------------------+
| Bitcoin SV Blockchain      |
|  - WhatsOnChain API        |
|  - GorillaPool mAPI        |
|  - Real Transaction IDs    |
|  - On-chain Verification   |
+----------------------------+
            ↓
+----------------------------+
| Identity & Auth Layer      |
|  - BRC-100 Auth Framework  |
|  - BRC-52/103 Certificates |
|  - Type-42 Key Derivation  |
|  - Selective Disclosure    |
|  - SPV Identity Validation |
|  - HTTP API Endpoints      |
|  - Session Management      |
|  - BEEF Transaction Support|
+----------------------------+



Process-Per-Overlay Communication Architecture
┌─────────────────┐    V8 Injection    ┌─────────────────┐
│   C++ Backend   │ ──────────────────→ │  Render Process │
│                 │                     │                 │
│ • Wallet APIs   │                     │ • window.bitcoinAPI.sendTransaction() │
│ • Address Mgmt  │                     │ • window.bitcoinAPI.getBalance() │
│ • Overlay Mgmt  │                     │ • window.cefMessage.send() │
│ • Message Handlers│                   │ • window.bitcoinBrowser.address.* │
└─────────────────┘                     └─────────────────┘
         │                                        │
         │ Process Messages                       │ JavaScript Execution
         │                                        │
         ▼                                        ▼
┌─────────────────┐                     ┌─────────────────┐
│ Browser Process │                     │   React App     │
│ Message Handler │                     │                 │
│                 │                     │ • Transaction UI │
│ • send_transaction│                   │ • Balance Display│
│ • get_balance   │                     │ • Address Gen   │
│ • overlay_show_*│                     │ • Process isolation │
│ • overlay_close │                     │ • Real-time Updates│
└─────────────────┘                     └─────────────────┘

## 🌐 HTTP Request Interception Architecture (2025-10-02)

### Async CEF HTTP Client System
```
External Website → HTTP Request → CEF Interceptor → UI Thread Task → Go Daemon → Response → Frontend
```

### Key Components:
```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        HTTP Request Interception Layer                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │ HttpRequest     │  │ AsyncWallet     │  │ AsyncHTTPClient            │  │
│  │ Interceptor     │  │ ResourceHandler │  │ - CefURLRequestClient      │  │
│  │ - Intercepts    │  │ - Request Lifecycle│ │ - Response Handling       │  │
│  │   localhost:8080│  │ - Response Stream│  │ - Data Streaming           │  │
│  │ - Resource      │  │ - CORS Headers  │  │ - Thread Safety            │  │
│  │   Handler       │  │ - Error Handling│  │ - Async Communication      │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
│           │                    │                    │                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │ URLRequest      │  │ CEF Task System │  │ Go Wallet Daemon            │  │
│  │ CreationTask    │  │ - CefPostTask   │  │ - HTTP API Endpoints        │  │
│  │ - UI Thread Post│  │ - Thread Safety │  │ - BRC-100 Services         │  │
│  │ - CefURLRequest │  │ - Async Handling│  │ - Real Blockchain APIs      │  │
│  │   Creation      │  │ - Error Recovery│  │ - Transaction Processing    │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Thread-Safe Communication Flow:
1. **IO Thread**: `HttpRequestInterceptor` receives HTTP request from external website
2. **UI Thread**: `URLRequestCreationTask` posts `CefURLRequest::Create` to UI thread
3. **HTTP Request**: `AsyncHTTPClient` makes async request to Go daemon
4. **Response**: `AsyncWalletResourceHandler` streams response back to frontend
5. **Frontend**: External website receives response data

### Technical Implementation:
- **Thread Safety**: Uses CEF's task system to ensure proper thread communication
- **Async Operations**: Non-blocking HTTP requests using `CefURLRequest`
- **Error Handling**: Comprehensive error handling with fallback responses
- **CORS Support**: Proper CORS headers for cross-origin requests
- **Resource Management**: Proper cleanup and memory management

## 🔐 BRC-100 Authentication Architecture

### BRC-100 Component Structure
```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         BRC-100 Service Layer                               │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐  ┌────────────┐  │
│  │ Identity Manager│  │ Auth Manager    │  │ Session Mgr │  │ Message    │  │
│  │ - Certificates  │  │ - Challenges    │  │ - Sessions  │  │ Relay      │  │
│  │ - Validation    │  │ - Type-42 Keys  │  │ - Cleanup   │  │ (BRC-33)   │  │
│  │ - Selective     │  │ - P2P Comm      │  │ - Security  │  │ - PeerServ │  │
│  └─────────────────┘  └─────────────────┘  └─────────────┘  └────────────┘  │
│           │                    │                    │              │         │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐  ┌────────────┐  │
│  │ BEEF Manager    │  │ SPV Manager     │  │ HTTP APIs   │  │ Socket.IO  │  │
│  │ - BRC-100 BEEF  │  │ - Verification  │  │ - REST      │  │ Handler    │  │
│  │ - Conversion    │  │ - Merkle Proofs │  │ - JSON APIs │  │ - Engine.IO│  │
│  │ - Broadcasting  │  │ - SDK Integration│  │ - BRC-104   │  │ - Polling  │  │
│  └─────────────────┘  └─────────────────┘  └─────────────┘  └────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### BRC-104 Authentication Flow (Babbage Protocol)
```
1. Client Request → 2. BRC-42 Derivation → 3. Sign → 4. Response
      ↓                    ↓                   ↓          ↓
   POST                Compute ECDH        Sign with   Return
   /.well-known/auth   Shared Secret       Derived Key  Signature
      ↓                    ↓                   ↓          ↓
   {initialNonce,      HMAC over          Concatenated  {version,
    identityKey}       invoice number     nonces        nonce,
                                                        yourNonce,
                                                        signature}
```

**BRC-42 Key Derivation Process:**
```
1. ECDH Shared Secret = privateKey * counterpartyPublicKey
2. HMAC = HMAC-SHA256(invoiceNumber, sharedSecret)
3. childPrivateKey = (rootPrivateKey + HMAC) mod N
4. signature = Sign(data, childPrivateKey)
```

**BRC-43 Invoice Number:**
```
Format: {securityLevel}-{protocolID}-{keyID}
Example: 2-auth message signature-{initialNonce} {sessionNonce}
```

### BRC-33 PeerServ Message Flow
```
1. Send Message → 2. Store → 3. List → 4. Acknowledge
      ↓              ↓          ↓          ↓
   POST           In-Memory   POST       POST
   /sendMessage   Storage     /listMessages  /acknowledgeMessage
      ↓              ↓          ↓          ↓
   {recipient,    Thread-Safe {messageBox} {messageIds[]}
    messageBox,   Map
    body}
```

### HTTP API Endpoints (22 Total)
```
Identity Management:
- POST /brc100/identity/generate
- POST /brc100/identity/validate
- POST /brc100/identity/selective-disclosure

Authentication:
- POST /brc100/auth/challenge
- POST /brc100/auth/authenticate
- POST /brc100/auth/type42
- POST /.well-known/auth (BRC-104 with BRC-42/43 signatures)

Session Management:
- POST /brc100/session/create
- POST /brc100/session/validate
- POST /brc100/session/revoke

BEEF Transactions:
- POST /brc100/beef/create
- POST /brc100/beef/verify
- POST /brc100/beef/broadcast

SPV Verification:
- POST /brc100/spv/verify
- POST /brc100/spv/proof

BRC-33 PeerServ Message Relay:
- POST /sendMessage (send message to recipient)
- POST /listMessages (list messages from message box)
- POST /acknowledgeMessage (delete acknowledged messages)

Socket.IO / Engine.IO:
- GET /socket.io/ (Engine.IO polling handshake)

Status:
- GET /brc100/status
```

## 🔄 Transaction Flow Architecture

### Complete Transaction Pipeline
```
React UI → C++ Bridge → Go Daemon → Blockchain
    ↓           ↓           ↓           ↓
1. User Input  2. Message   3. Create    4. Broadcast
   (Amount)    Routing      Transaction  to Miners
    ↓           ↓           ↓           ↓
5. Confirmation 6. Response 7. Sign     8. Real TxID
   Modal        Handling    Transaction  Returned
    ↓           ↓           ↓           ↓
9. Success     10. UI      11. UTXO    12. On-chain
   Display      Update      Selection   Verification
```

### Process Isolation Architecture
┌─────────────────────────────────────────────────────────────┐
│                    Main Browser Process                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐  │
│  │   Header HWND   │  │  WebView HWND   │  │ Main Shell  │  │
│  │ React App (/)   │  │ Web Content     │  │ Management  │  │
│  └─────────────────┘  └─────────────────┘  └─────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Process-Per-Overlay Architecture               │
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐  │
│  │ Settings Overlay│  │  Wallet Overlay │  │Backup Modal │  │
│  │   Process       │  │    Process      │  │   Process   │  │
│  │                 │  │                 │  │             │  │
│  │ ┌─────────────┐ │  │ ┌─────────────┐ │  │┌───────────┐│  │
│  │ │Settings HWND│ │  │ │ Wallet HWND │ │  ││Backup HWND││  │
│  │ │/settings    │ │  │ │ /wallet     │ │  ││/backup    ││  │
│  │ │Fresh V8     │ │  │ │Fresh V8     │ │  ││Fresh V8   ││  │
│  │ │Context      │ │  │ │Context      │ │  ││Context    ││  │
│  │ └─────────────┘ │  │ └─────────────┘ │  │└───────────┘│  │
│  └─────────────────┘  └─────────────────┘  └─────────────┘  │
└─────────────────────────────────────────────────────────────┘



flowchart TD
    A["Babbage-Browser (BitcoinBrowser)"]

    A --> B["Backend (C++ Shell)"]
    A --> C["Frontend (React + Vite)"]
    A --> D["Build System (CMake)"]
    A --> E["External Dependencies"]
    A --> F["Documentation"]
    A --> G["Project Configuration"]

    B --> B1["CEF Browser Engine"]
    B --> B2["Core Business Logic"]
    B --> B3["CEF Event Hooks"]
    B --> B4["Overlay System"]
    B --> B5["V8 Integration"]

    B1 --> B1a["Main Shell (cef_browser_shell.cpp)"]
    B1 --> B1b["Browser Process Handler (SimpleApp)"]
    B1 --> B1c["Render Process Handler (SimpleRenderProcessHandler)"]
    B1 --> B1d["Client Handler (SimpleHandler)"]

    B2 --> B2a["Wallet Manager (WalletManager.cpp)"]
    B2 --> B2b["Identity Handler (IdentityHandler.cpp)"]
    B2 --> B2c["Navigation Handler (NavigationHandler.cpp)"]
    B2 --> B2d["Panel Handler (PanelHandler.cpp)"]

    B2a --> B2a1["Identity Management"]
    B2a --> B2a2["Key Generation (EC, OpenSSL)"]
    B2a --> B2a3["AES Encryption/Decryption"]
    B2a --> B2a4["Base58 Encoding"]
    B2a --> B2a5["File I/O (wallet.json)"]

    B2b --> B2b1["get() - Retrieve wallet identity"]
    B2b --> B2b2["markBackedUp() - Mark wallet as backed up"]

    B2c --> B2c1["navigate() - Handle URL navigation"]

    B2d --> B2d1["open() - Open overlay panels"]

    B3 --> B3a["Life Span Handler (OnAfterCreated, OnLoadingStateChange)"]
    B3 --> B3b["Display Handler (OnTitleChange)"]
    B3 --> B3c["Load Handler (OnLoadError, OnLoadingStateChange)"]
    B3 --> B3d["Process Message Handler (OnProcessMessageReceived)"]

    B4 --> B4a["Process-Per-Overlay System"]
    B4 --> B4b["Dedicated HWND Management"]
    B4 --> B4c["Custom Render Handler (MyOverlayRenderHandler)"]
    B4 --> B4d["Window Message Handlers (WndProc)"]

    B5 --> B5a["Context Creation (OnContextCreated)"]
    B5 --> B5b["Native API Injection (window.bitcoinBrowser)"]
    B5 --> B5c["Function Binding (CefV8Value::CreateFunction)"]

    C --> C1["Application Structure"]
    C --> C2["Pages"]
    C --> C3["Components"]
    C --> C4["Hooks"]
    C --> C5["Types"]
    C --> C6["Bridge Layer"]
    C --> C7["Development Server"]

    C1 --> C1a["Main Entry (main.tsx)"]
    C1 --> C1b["App Component (App.tsx)"]
    C1 --> C1c["Router Configuration (BrowserRouter)"]

    C2 --> C2a["Welcome (/welcome) - Initial wallet setup"]
    C2 --> C2b["Main Browser View (/) - Primary dashboard"]
    C2 --> C2c["Settings Overlay (/settings) - Settings panel"]
    C2 --> C2d["Wallet Overlay (/wallet) - Wallet panel"]
    C2 --> C2e["Backup Modal (/backup) - Identity backup"]

    C3 --> C3a["Panels"]
    C3 --> C3b["Main Browser Interface"]

    C3a --> C3a1["Wallet Panel Layout (WalletPanelLayout.tsx)"]
    C3a --> C3a2["Wallet Panel Content (WalletPanelContent.tsx)"]
    C3a --> C3a3["Settings Panel Layout (SettingsPanelLayout.tsx)"]
    C3a --> C3a4["Backup Modal (BackupModal.tsx)"]

### ✅ Completed Components
- **React UI Layer**: Complete with transaction forms, balance display, address management
- **C++ Bridge Layer**: Full message handling and API injection
- **Go Wallet Daemon**: Complete HD wallet with transaction processing
- **Rust Wallet**: Groups A & B complete (14/31 BRC-100 methods)
- **BRC-100 Authentication**: Complete BRC-100 protocol implementation
- **Transaction Management**: Full lifecycle with history tracking
- **Action Storage**: JSON-based transaction history
- **BEEF Parser**: Phase 2 with output ownership detection
- **BEEF/SPV Integration**: Real blockchain transactions with SPV verification
- **Process Isolation**: Each overlay runs in dedicated CEF subprocess
- **Blockchain Integration**: Working with real Bitcoin SV network

### 🚧 In Development
- **Window Management**: Keyboard commands and overlay HWND movement
- **Transaction Receipt UI**: Improved confirmation and receipt display
- **Frontend BRC-100 Integration**: React authentication modals and approval flows

### 📋 Future Components
- **Transaction History**: Local storage and display
- **Advanced Address Management**: Gap limit, pruning, high-volume generation
- **SPV Verification**: Simplified Payment Verification implementation
