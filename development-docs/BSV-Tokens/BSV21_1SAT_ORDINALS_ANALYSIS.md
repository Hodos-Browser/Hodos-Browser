# BSV-21 and 1Sat Ordinals Analysis

## Overview

This document analyzes the BSV-21 fungible token protocol and 1Sat Ordinals inscription system, comparing them to our existing BRC-100 implementation, and evaluating integration options for HodosBrowser.

---

## What is 1Sat Ordinals?

[1Sat Ordinals](https://1satordinals.com/) is an implementation of the Ordinals protocol on BSV. It enables inscribing arbitrary data onto individual satoshis, creating unique digital artifacts.

### Key Concept: Origin-Based Indexing

BSV is unique among ordinals-supporting blockchains because it allows **1 satoshi outputs**. This enables a simpler indexing model:

- **Origin** = First outpoint where a satoshi exists alone in a 1-sat output
- Format: `<txid>_<vout>`
- If the satoshi is bundled into a multi-sat output, the origin "burns"
- If later spent into another 1-sat output, a new origin is created

This avoids the complex ordinal number tracking used in BTC ordinals.

### Inscription Envelope Format

Inscriptions use an **OP_FALSE OP_IF envelope**:

```
OP_FALSE OP_IF
  6f7264                    // "ord" in hex
  OP_1 <content-type>       // Field 1: content type
  OP_0 <data>               // Field 0: content data
OP_ENDIF
<locking_script>            // P2PKH or other lock
```

**Why OP_FALSE OP_IF?**
- `OP_FALSE` ensures the IF block never executes
- Data is stored but doesn't affect script execution
- Output remains spendable (unlike OP_RETURN)
- Can be combined with any locking script

### BSV vs BTC Ordinals Differences

| Aspect | BTC Ordinals | BSV 1Sat Ordinals |
|--------|--------------|-------------------|
| Location | Input scripts (Taproot witness) | Output scripts |
| ID format | `<txid>i<vin>` | `<txid>_<vout>` |
| Push data limit | 520 bytes (concatenated) | Unlimited |
| Minting | Commit + reveal (2 tx) | Single transaction |
| Min output | 546 sats (dust limit) | 1 sat |

---

## What is BSV-21?

[BSV-21](https://docs.1satordinals.com/fungible-tokens/bsv-21) is a **fungible token standard** built on 1Sat Ordinals. It evolved from BSV-20 to support "tickerless" mode with smart contract control.

### Key Innovation: Tickerless Tokens

Traditional token protocols (BRC-20, BSV-20 v1) use ticker-based identification:
- Deploy with ticker "PEPE"
- Anyone mints by referencing ticker
- Requires full chain scan to validate

BSV-21 tickerless mode:
- Deploy + mint entire supply in one transaction
- Token ID = `<txid>_<vout>` of mint output
- Forms DAG back to genesis
- No chain scanning needed for validation

### BSV-21 Operations

**Deploy+Mint** (create token):
```json
{
  "p": "bsv-20",
  "op": "deploy+mint",
  "amt": "1000000",
  "sym": "TEST",
  "icon": "<outpoint>",
  "dec": "8"
}
```

**Transfer**:
```json
{
  "p": "bsv-20",
  "op": "transfer",
  "id": "<txid>_<vout>",
  "amt": "100"
}
```

### Token Accounting Rules

- Tokens live in UTXOs (like native satoshis)
- Transfer: Spend token UTXO, create new outputs with transfer inscriptions
- **Over-transfer**: If outputs exceed inputs → transaction invalid, tokens burn
- **Under-transfer**: Unallocated tokens burn
- **Parallelization**: UTXO model allows splitting (like making change)

---

## Comparison: 1Sat Ordinals vs Our BRC Implementation

### Protocol Layer Comparison

| Aspect | 1Sat Ordinals / BSV-21 | Our BRC Implementation |
|--------|------------------------|------------------------|
| **Purpose** | Token/NFT protocol | Wallet interface standard |
| **Data embedding** | OP_FALSE OP_IF envelope | PushDrop format |
| **Token tracking** | External indexers | Wallet baskets |
| **Validation** | Indexer-based | Topic Manager (overlay) |
| **Standardization** | Community standard | BRC specifications |

### Data Embedding: OP_IF Envelope vs PushDrop

**1Sat OP_IF Envelope:**
```
OP_FALSE OP_IF "ord" <fields...> OP_ENDIF <locking_script>
```
- Never executed (OP_FALSE skips IF block)
- Data stored in output
- Spendable output

**Our PushDrop:**
```
<data1> <data2> OP_DROP OP_DROP <locking_script>
```
- Data pushed then dropped from stack
- Also never affects execution
- Also spendable output

**Key difference**: Different envelope structure, but same concept - embed data without affecting spending.

### Token Organization

| Aspect | BSV-21 | BRC-100 Baskets |
|--------|--------|-----------------|
| Grouping | By token ID (txid_vout) | By basket name |
| Discovery | Query indexer | Query wallet |
| Metadata | In inscription JSON | In customInstructions |
| Ownership | Whoever holds UTXO | Whoever holds UTXO |

**Observation**: Baskets could organize BSV-21 tokens - they're complementary, not competing.

### Validation Architecture

**BSV-21:**
```
Wallet → Transaction → Blockchain
                          ↓
                      Indexer (GorillaPool, etc.)
                          ↓
                      Validates token transfers
```

**BRC-100 with Overlay:**
```
Wallet → Topic Manager → Blockchain
             ↓
         Lookup Service
             ↓
         Validates & indexes
```

**Key insight**: BSV-21 relies on external indexers. BRC overlay services could theoretically index BSV-21 tokens.

---

## Why BSV-21 is Popular

1. **Simplicity**: JSON-based, easy to understand
2. **Tooling**: Mature libraries (js-1sat-ord, Go SDK)
3. **Infrastructure**: GorillaPool indexer, public APIs
4. **Community**: Active ecosystem, trading platforms
5. **BTC compatibility**: Similar to BRC-20, familiar to users
6. **NFT support**: Same protocol for fungible and non-fungible

### Ecosystem Libraries

| Library | Language | Purpose |
|---------|----------|---------|
| [js-1sat-ord](https://js.1satordinals.com/) | TypeScript | Full SDK |
| [go-1sat-ord](https://github.com/BitcoinSchema/go-1sat-ord) | Go | Go SDK |
| [sCrypt-ord](https://scrypt.io/) | TypeScript | Smart contract integration |
| [bsv20-indexer](https://github.com/BitcoinSchema/bsv20-indexer) | Go | Indexing |

### Public APIs

| API | Endpoint | Purpose |
|-----|----------|---------|
| GorillaPool | ordinals.gorillapool.io | Primary indexer |
| sCrypt Oracle | api.witnessonchain.com | Token validation |
| WhatsOnChain | plugins.whatsonchain.com | Basic inscription data |

---

## Integration Options for HodosBrowser

### Option 1: External Indexer Integration (Simplest)

Use existing GorillaPool APIs to query/display BSV-21 tokens.

**Pros:**
- Minimal implementation effort
- Leverage existing infrastructure
- Immediate functionality

**Cons:**
- Depends on third-party service
- No offline validation
- Trust GorillaPool's indexing

**Implementation:**
```rust
// rust-wallet/src/ordinals/
mod api_client;      // GorillaPool API client
mod types;           // Token/inscription types
mod display;         // Format for UI

// Endpoints
GET /ordinals/tokens/{address}     → List BSV-21 tokens
GET /ordinals/inscriptions/{address} → List inscriptions
GET /ordinals/token/{id}           → Token details
```

### Option 2: Inscription Parsing + External Validation (Medium)

Parse inscriptions locally, validate via external indexer.

**Pros:**
- Can display inscription data offline
- More control over presentation
- Partial independence from indexers

**Cons:**
- Can't validate token balances offline
- Still need indexer for DAG validation

**Implementation:**
```rust
// rust-wallet/src/ordinals/
mod envelope;        // OP_FALSE OP_IF parser
mod bsv21;           // BSV-21 JSON parser
mod api_client;      // External validation

// Parse inscription from script
pub fn parse_inscription(script: &[u8]) -> Option<Inscription>

// Parse BSV-21 token data
pub fn parse_bsv21(inscription: &Inscription) -> Option<Bsv21Token>
```

### Option 3: Full Local Indexing (Most Complete)

Run local indexer, validate everything locally.

**Pros:**
- Full sovereignty
- Offline validation
- No third-party trust

**Cons:**
- Significant implementation effort
- Storage requirements (DAG history)
- Need to sync chain data

**Implementation:**
- Port bsv20-indexer to Rust, or
- Run Go indexer as sidecar process

### Recommended Approach: Option 2 with Path to Option 3

1. **Phase 1**: External API integration (quick win)
2. **Phase 2**: Local inscription parsing
3. **Phase 3**: Optional local indexing for power users

---

## Reference Repositories (Cloned)

We cloned the following repos to `reference/` for studying the implementation:

```
reference/js-1sat-ord/    # TypeScript SDK
reference/go-1sat-ord/    # Go SDK
```

### Key Findings from TypeScript SDK

**Inscription envelope construction** (`src/templates/ordP2pkh.ts:51`):
```typescript
ordAsm = `OP_0 OP_IF ${ordHex} OP_1 ${fileMediaType} OP_0 ${fileHex} OP_ENDIF`;
```

**BSV-21 deploy+mint** (`src/deployBsv21.ts:86-104`):
```typescript
const fileData: DeployMintTokenInscription = {
  p: "bsv-20",
  op: "deploy+mint",
  sym: symbol,
  icon: iconValue,
  amt: tsatAmt.toString(),
};

const b64File = Buffer.from(JSON.stringify(fileData)).toString("base64");
const sendTxOut = {
  satoshis: 1,
  lockingScript: new OrdP2PKH().lock(destinationAddress, {
    dataB64: b64File,
    contentType: "application/bsv-20",
  }),
};
```

### Key Findings from Go SDK

Both SDKs rely on external BSV libraries for the actual script construction:
- TypeScript: `@bsv/sdk` with `OrdP2PKH` template extending `P2PKH`
- Go: `github.com/bitcoin-sv/go-templates/template/bsv21` with `Bsv21.Lock()`

The SDKs are thin wrappers that:
1. Construct JSON payloads for token operations
2. Delegate envelope/script construction to platform SDKs
3. Handle transaction building, fee calculation, signing

### Script Byte Pattern (for Rust parser)

```
OP_FALSE (0x00)
OP_IF (0x63)
PUSH "ord" (0x03 0x6f 0x72 0x64)
OP_1 (0x51)                        # Field 1 = content type
PUSH <content-type bytes>
OP_0 (0x00)                        # Field 0 = content data
PUSH <data bytes>
OP_ENDIF (0x68)
<P2PKH locking script>
```

---

## Technical Implementation Details

### Parsing OP_IF Envelope

```rust
// rust-wallet/src/ordinals/envelope.rs

pub struct Inscription {
    pub content_type: Option<String>,
    pub content: Vec<u8>,
    pub fields: HashMap<u8, Vec<u8>>,
}

pub fn parse_ord_envelope(script: &[u8]) -> Option<Inscription> {
    // Look for: OP_FALSE OP_IF "ord" ...
    // 0x00 0x63 0x036f7264 ...

    let mut i = 0;

    // Find OP_FALSE (0x00) OP_IF (0x63)
    while i < script.len() - 1 {
        if script[i] == 0x00 && script[i + 1] == 0x63 {
            // Found envelope start
            i += 2;
            break;
        }
        i += 1;
    }

    // Check for "ord" (0x03 0x6f 0x72 0x64)
    if script[i] != 0x03 || &script[i+1..i+4] != b"ord" {
        return None;
    }
    i += 4;

    // Parse field/value pairs until OP_ENDIF (0x68)
    let mut fields = HashMap::new();
    let mut content_type = None;
    let mut content = Vec::new();

    while i < script.len() && script[i] != 0x68 {
        let (field, field_len) = parse_push_data(&script[i..])?;
        i += field_len;

        let (value, value_len) = parse_push_data(&script[i..])?;
        i += value_len;

        match field[0] {
            0x01 => content_type = Some(String::from_utf8_lossy(&value).to_string()),
            0x00 => content = value,
            n => { fields.insert(n, value); }
        }
    }

    Some(Inscription { content_type, content, fields })
}
```

### Parsing BSV-21 JSON

```rust
// rust-wallet/src/ordinals/bsv21.rs

#[derive(Debug, Deserialize)]
pub struct Bsv21Token {
    pub p: String,           // "bsv-20"
    pub op: String,          // "deploy+mint" or "transfer"
    #[serde(default)]
    pub id: Option<String>,  // "<txid>_<vout>" for transfers
    pub amt: String,         // Amount as string (big number)
    #[serde(default)]
    pub sym: Option<String>, // Symbol
    #[serde(default)]
    pub dec: Option<u8>,     // Decimals (default 0)
    #[serde(default)]
    pub icon: Option<String>, // Icon reference
}

pub fn parse_bsv21(inscription: &Inscription) -> Option<Bsv21Token> {
    if inscription.content_type.as_deref() != Some("application/bsv-20") {
        return None;
    }

    serde_json::from_slice(&inscription.content).ok()
}
```

### Integrating with Baskets

BSV-21 tokens could be tracked in baskets:

```rust
// When receiving a BSV-21 token
let utxo = Utxo {
    txid: "abc123...",
    vout: 0,
    satoshis: 1,
    script: "...",
    basket: Some("bsv21_tokens".to_string()),
    custom_instructions: Some(json!({
        "protocol": "bsv-21",
        "token_id": "def456_0",
        "symbol": "TEST",
        "amount": "1000"
    }).to_string()),
};

// Query tokens via listOutputs
let tokens = list_outputs(ListOutputsRequest {
    basket: Some("bsv21_tokens".to_string()),
    ..Default::default()
})?;
```

---

## Comparison: Should We Implement?

### Arguments For

1. **User demand**: Popular protocol, users expect support
2. **Ecosystem compatibility**: Trade on existing marketplaces
3. **NFT support**: Same protocol handles both fungible and NFTs
4. **Mature tooling**: Can reference existing implementations
5. **Basket integration**: Fits naturally with our architecture

### Arguments Against

1. **Indexer dependency**: Full validation requires external indexer
2. **Not BRC-standard**: Different from overlay model
3. **Maintenance burden**: Another protocol to support
4. **Competing approaches**: BRC-48 (STAS) is the "official" token standard

### Recommendation

**Implement Option 2** (parsing + external validation):

1. Users can see their BSV-21 tokens
2. Can send tokens with proper inscription format
3. Validation via GorillaPool ensures correctness
4. Path to local indexing if needed later
5. Relatively low implementation effort

---

## Implementation Plans

Detailed implementation plans have been created for parallel development:

| Plan | Focus | Document |
|------|-------|----------|
| **Plan A** | Backend/Wallet (Rust) | [BSV21_PLAN_A_BACKEND.md](BSV21_PLAN_A_BACKEND.md) |
| **Plan B** | Frontend/UI (React) | [BSV21_PLAN_B_FRONTEND.md](BSV21_PLAN_B_FRONTEND.md) |

### Plan A: Backend (Rust Wallet)

- GorillaPool API client
- OP_IF envelope parser
- BSV-21 JSON parser
- HTTP endpoints for token queries
- Transfer transaction builder
- **Can be developed and tested independently**

### Plan B: Frontend (React UI)

- Token display components
- Wallet panel integration
- Send token form
- **Can use mock data initially, then connect to Plan A**

### Plan C: Platform Integration (Future)

- Marketplace connectivity
- Trading/listing tokens
- **Depends on platforms adopting BRC-100**

---

## Code Reference Strategy

### Should We Clone Repos?

**Recommendation**: Clone for reference, but rewrite in Rust.

**Clone to `build-reference/`:**
```bash
git clone https://github.com/BitcoinSchema/js-1sat-ord build-reference/js-1sat-ord
git clone https://github.com/BitcoinSchema/go-1sat-ord build-reference/go-1sat-ord
```

**Why rewrite in Rust:**
1. Consistency with our codebase
2. Memory safety for key operations
3. No FFI complexity
4. Can optimize for our use case
5. TypeScript/Go libs are thin wrappers anyway

**What to reference:**
- Envelope parsing logic
- BSV-21 validation rules
- API response formats
- Test vectors

---

## API Endpoints (Proposed)

### Token Queries

```
GET /ordinals/tokens/{address}
→ List BSV-21 tokens for address

GET /ordinals/token/{id}
→ Get token metadata (symbol, supply, etc.)

GET /ordinals/token/{id}/holders
→ Get token holder list (from indexer)
```

### Inscription Queries

```
GET /ordinals/inscriptions/{address}
→ List all inscriptions for address

GET /ordinals/inscription/{origin}
→ Get inscription content and metadata
```

### Token Operations

```
POST /ordinals/transfer
→ Create token transfer transaction

POST /ordinals/inscribe
→ Create new inscription

POST /ordinals/deploy
→ Deploy new BSV-21 token
```

---

## Wallet Connection Patterns (Platform Integration Research)

Understanding how other wallets connect to dApps is important for future platform integration.

### Yours Wallet (Browser Extension)

- **Model**: MetaMask-style injection
- **Object**: `window.yours` injected into page JavaScript
- **Flow**: dApp calls `yours.connect()`, user approves, dApp gets addresses
- **Methods**: `getAddresses()`, `sendBsv()`, `signMessage()`, `getBalance()`, `encryptDecrypt()`
- **Documentation**: [yours-wallet.gitbook.io/provider-api](https://yours-wallet.gitbook.io/provider-api/)

### Handcash Connect

- **Model**: OAuth-style redirect flow
- **Flow**:
  1. App generates secp256k1 keypair
  2. Redirect user to Handcash with public key
  3. User authorizes app
  4. App stores private key, uses for API calls
- **Key Storage**: App stores private key (session/local storage or server-side)
- **Documentation**: [docs.handcash.io](https://docs.handcash.io)

### BRC-100 (HodosBrowser)

- **Model**: Transparent HTTP interception
- **Flow**:
  1. Site calls standard HTTP endpoints (e.g., `/.well-known/auth`)
  2. Browser intercepts request
  3. User approves via overlay
  4. Browser forwards to wallet, returns response
- **Key Storage**: Never exposed to JavaScript
- **Advantage**: No site-specific integration needed

### Comparison

| Aspect | Yours | Handcash | BRC-100 |
|--------|-------|----------|---------|
| Integration effort | Medium (call API) | High (OAuth flow) | Low (standard HTTP) |
| User approval | Per-connection | One-time | Per-request |
| Key exposure | Never | App holds keypair | Never |
| Works offline | No | No | Partial |

### Implications for Platform Integration

BSV-21 platforms currently support Yours wallet via `window.yours`. For them to support BRC-100:

1. Replace `yours.sendBsv()` with POST to `/createAction`
2. Replace `yours.signMessage()` with POST to `/createSignature`
3. Replace `yours.getAddresses()` with POST to `/getPublicKey`

The API surface is similar; the difference is injection vs HTTP.

---

## Open Questions

1. **Indexer fallback**: What if GorillaPool goes down? Run our own?

2. **Token validation depth**: How much do we trust indexer vs verify ourselves?

3. **Basket auto-detection**: Should we auto-categorize tokens or let users organize?

4. **NFT display**: How to render different content types (images, HTML, etc.)?

5. **Marketplace integration**: Should we support listing/buying directly?

---

## References

### Documentation
- [1Sat Ordinals Docs](https://docs.1satordinals.com/)
- [BSV-21 Specification](https://docs.1satordinals.com/fungible-tokens/bsv-21)
- [js-1sat-ord Library](https://js.1satordinals.com/)

### Code Repositories
- [1sat-ordinals (spec)](https://github.com/BitcoinSchema/1sat-ordinals)
- [js-1sat-ord](https://github.com/BitcoinSchema/js-1sat-ord)
- [go-1sat-ord](https://github.com/BitcoinSchema/go-1sat-ord)
- [bsv20-indexer](https://github.com/BitcoinSchema/bsv20-indexer)

### APIs
- [GorillaPool Ordinals API](https://ordinals.gorillapool.io/api/docs)
- [sCrypt Oracle API](https://api.witnessonchain.com)
- [BMAP API](https://b.map.sv)

### Related Standards
- [BSV Token Protocols Overview](https://bsvblockchain.org/features/token-protocols/)
- [STAS Token Protocol](https://bsvblockchain.org/features/token-protocols/)

---

## Implementation Documents

| Document | Description | Status |
|----------|-------------|--------|
| `BSV21_PLAN_A_BACKEND.md` | Rust wallet implementation | Ready for Implementation |
| `BSV21_PLAN_B_FRONTEND.md` | React UI implementation | Pending UI/UX Design |
| `BSV21_UX_DESIGN_OUTLINE.md` | UI/UX design considerations | Pending Design Review |

---

**Created**: January 2025
**Updated**: January 2025
**Status**: Analysis Complete - Implementation Plans Created
