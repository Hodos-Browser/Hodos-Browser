# BSV Academy Concepts Summary

This document summarizes concepts from BSV Academy documentation and analyzes how HodosBrowser's wallet implementation aligns with these designs.

---

## Table of Contents

1. [Digital Wallet Architecture](#1-digital-wallet-architecture)
2. [Tokens, Baskets, and Custom Instructions](#2-tokens-baskets-and-custom-instructions)
3. [PushDrop Tokenization](#3-pushdrop-tokenization)
4. [State Management Paradigms](#4-state-management-paradigms)
5. [Overlay Services](#5-overlay-services)
6. [Identity and Certificates](#6-identity-and-certificates)
7. [Payment Libraries](#7-payment-libraries)
8. [UHRP Content Hosting](#8-uhrp-content-hosting)
9. [HodosBrowser Alignment Analysis](#9-hodosbrowser-alignment-analysis)

---

## 1. Digital Wallet Architecture

### Core Principle: Separation of Concerns

BSV wallet architecture separates functionality into two distinct components:

| Component | Responsibility | Resource Requirements |
|-----------|----------------|----------------------|
| **Signing Component** | Manages private keys, executes transaction signatures | Lightweight, can run on IoT/embedded devices |
| **Storage Component** | Tracks transaction data, block headers, SPV validation | Heavy lifting, can manage large datasets |

### Why This Matters

- **Security**: Private keys isolated in minimal attack surface
- **Scalability**: Storage can scale independently
- **Flexibility**: Different deployment topologies (local, network, cloud, multi-provider)

### Storage Deployment Scenarios

1. **Local Deployment** - Storage on same machine as signer (simplest)
2. **Network Deployment** - Storage on LAN, accessible by multiple devices
3. **Internet Deployment** - Cloud storage with authentication
4. **Third-Party Services** - Paid storage services with encryption and sync

### HodosBrowser Implementation

Our Rust wallet combines both roles in a single process:
- `rust-wallet/src/handlers.rs` - Signing operations (create_action, sign_action)
- `rust-wallet/src/database/` - Storage operations (UTXO tracking, certificates)
- The separation exists logically but not as separate deployable components

---

## 2. Tokens, Baskets, and Custom Instructions

### Token Representation

In BSV, tokens are represented by **transaction outputs** consisting of:
- **Satoshis**: Base layer value (even 1 satoshi can represent a token)
- **Scripts**: Contain spending constraints AND higher-layer metadata

### Baskets: Token Organization

Baskets are a **user-driven grouping mechanism** supported by wallets:

```
User's Wallet
├── basket: "payment_tokens"
│   ├── UTXO: 50000 sats (basic payment)
│   └── UTXO: 25000 sats (basic payment)
├── basket: "game_collectibles"
│   ├── UTXO: 1 sat (sword token with metadata)
│   └── UTXO: 1 sat (armor token with metadata)
└── basket: "certificates"
    └── UTXO: 1 sat (identity certificate)
```

**Benefits**:
- Organization: Categorize tokens by purpose
- Interoperability: Multiple apps can access same basket
- UI Competition: Different apps can present same tokens differently

### Custom Instructions

Additional metadata attached to tokens in baskets:
- Guide applications on how to handle/spend tokens
- Include history or context required for future transactions
- Should follow common format for cross-app interoperability

### HodosBrowser Implementation

Our `listOutputs` endpoint (BRC-100 Call Code 6) implements basket queries:

```rust
// From handlers.rs - list_outputs handler
pub async fn list_outputs(
    state: web::Data<AppState>,
    req: web::Json<ListOutputsRequest>,
) -> HttpResponse {
    // Queries UTXOs by basket with tag filtering
    // Supports include/exclude labels (tags)
    // Returns spendable outputs with customInstructions
}
```

**Database Tables**:
- `utxos` - Stores outputs with `basket` column
- `tags` - Supports tag-based filtering
- `custom_instructions` field per UTXO

---

## 3. PushDrop Tokenization

### The Problem

How do you embed structured data in a Bitcoin output without affecting spending rules?

### The Solution: Push and Drop

PushDrop uses Bitcoin Script operations to:
1. **Push** data fields onto the stack
2. **Drop** them immediately (OP_DROP)
3. Continue with actual locking script (signatures, etc.)

```
<data_field_1> <data_field_2> OP_DROP OP_DROP <actual_locking_script>
```

**Result**: Data is stored in the script but doesn't affect validation.

### Workflow

1. **Token Creation**
   - Pass structured data fields to PushDrop library
   - Include protocol/key identifiers for locking
   - Generate output script and add to transaction

2. **Token Storage**
   - Add output to transaction with specified satoshi amount
   - Associate with a basket for tracking

3. **Token Reading**
   - Retrieve outputs from basket
   - Decode PushDrop script to extract data fields

4. **Token Redemption**
   - Generate unlocking script with same protocol/key identifiers
   - Spend the UTXO (removes from basket)

### HodosBrowser Implementation

Our PushDrop support is in `rust-wallet/src/script/pushdrop.rs`:

```rust
// Decode PushDrop scripts to extract embedded data
pub fn decode_pushdrop_fields(script: &[u8]) -> Result<Vec<Vec<u8>>, PushDropError>

// Encoding for creating new PushDrop tokens
pub fn create_pushdrop_script(fields: &[&[u8]], locking_script: &[u8]) -> Vec<u8>
```

Used by certificate handlers to embed certificate data in transactions.

---

## 4. State Management Paradigms

BSV applications have three primary patterns for managing state:

### 4.1 Local State (Baskets)

**What**: State stored entirely in user's wallet using baskets.

**When to Use**:
- Single-user applications
- User's private data
- Simple CRUD operations on tokens

**Example**: To-Do List App
- Each task = one token in "tasks" basket
- Add task = create new token
- Complete task = spend token (remove from basket)

**Advantages**: Simple, user-centric, no coordination needed
**Limitations**: Not shared across users

### 4.2 Peer-to-Peer (MessageBox)

**What**: Direct message exchange between users via relay service.

**When to Use**:
- Two-party coordination
- Payment notifications
- Token transfers between known parties

**Example**: Alice pays Bob
- Alice creates transaction giving Bob a UTXO
- Alice sends transaction to Bob via MessageBox
- Bob receives, validates, stores in his wallet

**Advantages**: Direct, efficient, works offline (store-and-forward)
**Limitations**: Doesn't scale to many parties

### 4.3 Overlay Services

**What**: Specialized services that track and coordinate tokens at scale.

**When to Use**:
- Multi-party coordination
- Business rule enforcement
- Public token registries
- Complex applications (betting, trading, etc.)

**Components**:
- **Topic Manager**: Validates transactions, enforces rules
- **Lookup Service**: Indexes tokens, responds to queries
- **SHIP (Submit, Host, Index, Prove)**: Manages token lifecycle

**Example**: Sports Betting
- Overlay tracks all active bets
- Topic Manager enforces betting rules
- Lookup Service shows current odds/positions
- Regulatory compliance built into rules

### HodosBrowser Implementation

| Paradigm | Implementation Status |
|----------|----------------------|
| Local/Baskets | ✅ Full - `listOutputs`, `relinquishOutput`, basket DB tables |
| Peer-to-Peer | ✅ Basic - `sendMessage`, `listMessages`, `acknowledgeMessage` (in-memory) |
| Overlay Services | ⚠️ Partial - HTTP uplinks to external services, no local Topic Manager |

---

## 5. Overlay Services

### Architecture

```
         Applications
              │
              ▼
    ┌─────────────────────┐
    │   Lookup Service    │  ← Answers queries about token state
    └─────────────────────┘
              │
              ▼
    ┌─────────────────────┐
    │   Topic Manager     │  ← Validates & admits transactions
    └─────────────────────┘
              │
              ▼
    ┌─────────────────────┐
    │   Storage Layer     │  ← Stores admitted transactions
    └─────────────────────┘
```

### Topic Manager Responsibilities

1. **Identify Admissible Outputs** - Which outputs belong to this topic?
2. **Enforce Business Rules** - Validate transactions against protocol rules
3. **Admit Transactions** - Accept valid transactions into the overlay
4. **Prune Spent Outputs** - Remove consumed UTXOs from tracking

### Lookup Service Responsibilities

1. **Index Tokens** - Store queryable data about tracked tokens
2. **Answer Queries** - Respond to `getUTXOs`, `lookup` requests
3. **Provide Metadata** - Return documentation and capability info
4. **Support Synchronization** - Help wallets sync state

### UHRP (Universal Hash Resolution Protocol)

Special overlay for large file hosting:
- Files stored off-chain, hash stored on-chain
- Multiple hosts provide redundancy
- Content integrity verified via hash comparison
- Hosts create commitment tokens advertising availability

### HodosBrowser Implementation

We don't run our own overlay services but connect to external ones:
- WhatsOnChain for transaction lookup
- GorillaPool for UTXO queries
- Future: Connect to UHRP hosts for content

---

## 6. Identity and Certificates

### The Identity Problem

Public keys are secure but user-unfriendly (long hex strings). How do we map friendly names to keys without centralized servers?

### Distributed Trust Network Solution

Instead of one central authority:
1. Users configure **trusted certifiers** in their wallet
2. Certifiers issue **digital certificates** binding attributes to keys
3. A **trust overlay** registers these certificates
4. Applications query for identities, only accepting trusted certifiers

### Certificate Structure (BRC-52)

```
{
  "type": "certificate_type_id",
  "subject": "<user_public_key>",
  "serialNumber": "<unique_id>",
  "fields": {
    "name": "<encrypted>",
    "email": "<encrypted>",
    ...
  },
  "certifier": "<certifier_public_key>",
  "signature": "<certifier_signature>"
}
```

**Key Features**:
- Fields are encrypted (selective disclosure)
- UTXO-based revocation (spend certificate = revoke)
- Certifier specialization (different certs for different attributes)

### Discovery Types

| Type | Input | Output |
|------|-------|--------|
| **Key Discovery** | Attributes (name, email) | Matching public keys |
| **Attribute Discovery** | Public key | Associated attributes |

### Running a Certifier

To become an identity certifier:
1. Generate secure signing keypair
2. Set up backend verification system
3. Define certification criteria
4. Issue certificates to verified users
5. Collaborate with industry for interoperability

### HodosBrowser Implementation

Certificate handlers in `rust-wallet/src/handlers/certificate_handlers.rs`:

| Endpoint | Status | Purpose |
|----------|--------|---------|
| `/listCertificates` | ✅ | Query local certificates |
| `/acquireCertificate` | ✅ | Direct or issuance acquisition |
| `/proveCertificate` | ✅ | Generate proofs with selective disclosure |
| `/discoverByIdentityKey` | ✅ | Attribute discovery |
| `/discoverByAttributes` | ✅ | Key discovery |
| `/relinquishCertificate` | ✅ | Delete certificates |

---

## 7. Payment Libraries

### Architecture

```
┌──────────────────────────────────────────────────────────┐
│                      APPLICATION                         │
├──────────────────────────────────────────────────────────┤
│  Client-Side Payment Library                             │
│  • Monitors requests to backend                          │
│  • Initiates wallet approval flow                        │
│  • Encapsulates payment in request                       │
│  • Handles responses                                     │
└──────────────────────────────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────┐
│                       BACKEND                            │
├──────────────────────────────────────────────────────────┤
│  Server-Side Payment Library                             │
│  • Determines charge for requests                        │
│  • Blocks unpaid requests                                │
│  • Sends payment instructions                            │
│  • Validates incoming payments                           │
│  • Notifies backend code of receipt                      │
└──────────────────────────────────────────────────────────┘
```

### Client-Side Workflow

1. Application makes request to backend
2. Backend responds with payment requirement
3. Payment library initiates wallet approval
4. User approves in wallet UI
5. Payment library encapsulates payment in retry request
6. Backend validates, processes, responds

### Server-Side Workflow

1. Examine incoming request
2. Determine if payment required
3. If unpaid, respond with payment instructions
4. Validate received payment
5. Expose sender + amount to business logic
6. Process request and respond

### HodosBrowser Implementation

BRC-100 provides the wallet interface for payments:
- `createAction` - Creates transactions for payments
- `signAction` - Signs transactions
- `isAuthenticated` - Verifies user identity

The HTTP interceptor (`HttpRequestInterceptor.cpp`) handles authentication headers for backend payment integration.

---

## 8. UHRP Content Hosting

### Problem

Large files (images, videos, 3D models) shouldn't be stored directly on blockchain - too expensive, too slow.

### Solution: Hash Reference

1. Store file with hosting provider
2. Store **hash** of file on blockchain
3. Hash acts as content-addressable reference (`uhrp://<hash>`)

### Upload Flow

```
Application → UHRP Library → Hosting Service
                  │
                  ├─ 1. Compute file hash
                  ├─ 2. Request hosting invoice
                  ├─ 3. Pay invoice via wallet
                  ├─ 4. Upload file
                  ├─ 5. Host creates commitment token
                  └─ 6. Return uhrp:// URL
```

### Download Flow

```
Application → UHRP Library → Lookup Service → Hosts
                  │
                  ├─ 1. Query lookup service for hash
                  ├─ 2. Get list of hosting tokens
                  ├─ 3. Verify signatures, check commitments
                  ├─ 4. Download from available host
                  ├─ 5. Verify hash matches
                  └─ 6. Return file data
```

### Reputable Hosting

To run a UHRP host:
1. Set up storage backend
2. Generate hosting keypair
3. Accept uploads (with payment)
4. Create commitment tokens
5. Propagate to UHRP overlay
6. Respond to legal requests transparently

### HodosBrowser Implementation

Currently no direct UHRP integration. Future considerations:
- Avatar image hosting for identity cards
- Content-addressed attachments
- Decentralized file references in transactions

---

## 9. HodosBrowser Alignment Analysis

### What We've Implemented Well

| BSV Concept | Our Implementation | Notes |
|-------------|-------------------|-------|
| Signing/Storage Separation | Logical separation in Rust | Could be physically separated in future |
| Baskets | Full UTXO basket support | `listOutputs`, `relinquishOutput` |
| Certificates (BRC-52) | Complete handlers | All 6 certificate endpoints |
| Key Derivation (BRC-42/43) | Full implementation | `crypto/brc42.rs`, `crypto/brc43.rs` |
| Authentication (BRC-31) | Partial in HTTP interceptor | Headers processed, not full Authrite |
| PushDrop | Script parsing | `script/pushdrop.rs` |

### What Needs Improvement

| BSV Concept | Current State | Recommendation |
|-------------|---------------|----------------|
| Message Relay (BRC-33) | In-memory only | Add persistent storage, federation support |
| Overlay Integration | External APIs only | Consider Topic Manager for local validation |
| UHRP | Not implemented | Add for avatar/content hosting |
| Payment Libraries | Manual integration | Add client-side payment library support |
| Multi-Provider Storage | Single local DB | Support multiple storage backends |

### Architecture Comparison

**BSV Academy Reference Architecture**:
```
┌─────────────┐     ┌─────────────┐
│   Signer    │────▶│   Storage   │
│  (minimal)  │     │  (scalable) │
└─────────────┘     └─────────────┘
                          │
                    ┌─────┴─────┐
                    ▼           ▼
             ┌──────────┐ ┌──────────┐
             │ Overlay  │ │   P2P    │
             │ Services │ │ Network  │
             └──────────┘ └──────────┘
```

**HodosBrowser Current Architecture**:
```
┌────────────────────────────────┐
│       Rust Wallet Process      │
│  ┌──────────┐  ┌────────────┐  │
│  │  Signer  │  │  Storage   │  │
│  │  (keys)  │  │  (SQLite)  │  │
│  └──────────┘  └────────────┘  │
└────────────────────────────────┘
              │
        ┌─────┴─────┐
        ▼           ▼
 ┌────────────┐ ┌────────────┐
 │ WhatsOnChain│ │ GorillaPool│
 └────────────┘ └────────────┘
```

### Recommended Next Steps

1. **BRC-33 Persistence** - Move message relay from in-memory to SQLite
2. **Federation Support** - Allow connecting to multiple message relay servers
3. **Overlay Client** - Add client library for overlay service queries
4. **UHRP Integration** - Support content-addressed file references
5. **Storage Abstraction** - Prepare for multi-backend storage support

---

## Appendix: BSV Academy Document Sources

| Document | Key Topics |
|----------|------------|
| Components of Digital Wallets | Signing vs Storage separation |
| Tokens, Baskets, and Custom Instructions | Token representation, basket organization |
| Tokenizing Structured Data with PushDrop | Data embedding in scripts |
| Managing Data Efficiently | State management paradigms |
| Creating a Topic Manager | Transaction validation rules |
| Creating a Lookup Service | Query indexing |
| Deploying Overlay Services | SHIP architecture |
| Managing Backend Wallets | Backend funding patterns |
| Backends and User Authentication | Auth library integration |
| Using Payment Libraries | Client/server payment flow |
| Running an Identity Certifier | Certificate authority setup |
| Managing Wallet Storage Systems | SPV and storage deployment |
| UHRP and Identity Resolution | Content hosting, identity discovery |
| Reputable UHRP Hosting | Running a content host |
