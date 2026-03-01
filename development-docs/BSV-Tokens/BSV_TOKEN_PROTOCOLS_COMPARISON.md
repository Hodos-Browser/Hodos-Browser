# BSV Token Protocols Comparison

> **Purpose**: Compare BSV-20, BSV-21, STAS, and BRC Push Drop token protocols. Analyze implementation requirements for Hodos Browser's BRC-100 wallet.
>
> **Created**: March 2026
> **Status**: Research Complete - Recommendation Included

---

## Executive Summary

| Protocol | Type | Validation | Current Status | Recommendation |
|----------|------|------------|----------------|----------------|
| **BSV-20** | Fungible (ticker-based) | External indexer | Legacy, less popular | ❌ Skip |
| **BSV-21** | Fungible (tickerless) | External indexer | Active ecosystem | ✅ Implement |
| **STAS** | Fungible/NFT | Miner-enforced | Enterprise use, MIT licensed | ⏳ Consider later |
| **BRC Push Drop** | Data carrier | Overlay network | **Already implemented** | ✅ Leverage existing |

**Bottom Line**: Implement BSV-21 support by leveraging our existing Push Drop infrastructure. BSV-21 tokens can be tracked in baskets alongside our existing BRC-100 outputs. STAS is a longer-term consideration for enterprise use cases.

---

## Protocol Deep Dive

### 1. BSV-20 (First-Is-First Mode)

**Overview**: The original ordinals-based fungible token protocol, modeled after BTC's BRC-20.

**How It Works**:
1. Deploy: Create inscription with `{"p":"bsv-20", "op":"deploy", "tick":"PEPE", "max":"21000000"}`
2. Mint: Anyone creates mint inscriptions referencing the ticker
3. Transfer: Spend UTXO with transfer inscription

**Technical Details**:
```json
// Deploy
{"p": "bsv-20", "op": "deploy", "tick": "PEPE", "max": "21000000", "lim": "1000"}

// Mint
{"p": "bsv-20", "op": "mint", "tick": "PEPE", "amt": "1000"}

// Transfer
{"p": "bsv-20", "op": "transfer", "tick": "PEPE", "amt": "500"}
```

**Data Embedding**: OP_FALSE OP_IF envelope (ordinals style)
```
OP_FALSE OP_IF
  "ord"
  OP_1 "application/bsv-20"
  OP_0 <json_data>
OP_ENDIF
<P2PKH locking script>
```

**Validation Challenges**:
- Requires full blockchain scan to validate ticker ownership
- First-is-first race conditions
- Ticker squatting problems
- Complex indexer requirements

**Pros**:
- Simple to understand
- BRC-20 familiarity for BTC users

**Cons**:
- Validation nightmare (full chain scan)
- Race condition vulnerabilities
- No programmatic control over distribution
- Legacy - BSV-21 supersedes it

---

### 2. BSV-21 (Tickerless Mode)

**Overview**: Evolution of BSV-20 that eliminates ticker-based identification. Entire supply minted in one transaction, forming a traceable DAG.

**How It Works**:
1. Deploy+Mint: Single transaction creates entire token supply
2. Token ID = `<txid>_<vout>` of the mint output
3. Transfer: Spend and create new outputs with transfer inscriptions
4. Validation: Follow DAG back to genesis (no full chain scan)

**Technical Details**:
```json
// Deploy+Mint (creates entire supply)
{
  "p": "bsv-20",
  "op": "deploy+mint",
  "sym": "TEST",
  "amt": "21000000",
  "dec": "8",
  "icon": "<icon_origin>"
}

// Transfer
{
  "p": "bsv-20",
  "op": "transfer",
  "id": "abc123...def_0",  // Token ID from deploy
  "amt": "1000"
}
```

**Data Embedding**: Same OP_FALSE OP_IF envelope as BSV-20.

**Key Innovation - Tickerless Mode**:
- Token identified by genesis outpoint (`txid_vout`), not ticker
- Can lock initial supply with any Bitcoin Script:
  - P2PKH (admin controls distribution)
  - Proof-of-Work (mining distribution)
  - Proof-of-Stake
  - Custom smart contracts
- Every transfer forms single DAG → easy validation

**UTXO Model Benefits**:
- Same parallelization as native BSV
- Split large holdings into smaller UTXOs for concurrent transfers
- No sequential bottleneck (unlike ERC-20 account model)

**Validation**:
- External indexer (GorillaPool) tracks the DAG
- Much simpler than BSV-20: follow outputs back to genesis
- Input sum must equal output sum (or tokens burn)

**Pros**:
- Clean DAG-based validation
- Programmatic distribution control
- Active ecosystem with tooling
- UTXO parallelization

**Cons**:
- Still requires external indexer (not miner-enforced)
- Indexer dependency for balance validation

---

### 3. STAS (Satoshi Token Allocation Standard)

**Overview**: Smart contract-based protocol with **miner-enforced validation**. Embeds metadata directly onto satoshis.

**How It Works**:
1. Each token unit requires 1 satoshi backing
2. Metadata attached to UTXO (origin-based indexing)
3. Transfers use STAS-specific transaction format
4. **Miners validate** - invalid transactions are rejected

**Technical Details**:
```
Token ID Format: <issuerAddress>-<symbol>
Example: 1A2B3C...XYZ-TokenABC

STAS Output Specification (DPP):
{
  "tokenId": "issuerAddress-symbol",
  "amount": 100,
  "recipient": "address_or_paymail"
}
```

**Variants**:
| Variant | Purpose | Max Outputs |
|---------|---------|-------------|
| STAS-20 | Fungible tokens | Standard |
| STAS-50 | High-throughput | 50 per tx |
| STAS NFT | Non-fungible | `splittable: false` |

**Key Feature - Miner Enforcement**:
- STAS uses Bitcoin Script constraints
- Non-compliant transactions **fail at miner level**
- No external indexer needed for validity
- Token can't be "accidentally burned" by sending to wrong address

**Tokenization**:
- 1 token unit = 1 satoshi minimum
- 1 million tokens requires 1 million satoshis
- Metadata permanently embedded on-chain

**Pros**:
- **Miner-enforced** (no indexer trust)
- Self-contained validation
- Legal compliance friendly
- MIT licensed (2025)

**Cons**:
- Larger transaction sizes (embedded metadata)
- Higher satoshi requirement
- Less ecosystem tooling than BSV-21
- More complex implementation

---

### 4. BRC Push Drop (BRC-48)

**Overview**: A **script template** (not a token protocol) for data-rich, spendable UTXOs. Used by BRC overlay networks for tokenization.

**How It Works**:
```
<data1> <data2> ... <dataN> OP_DROP OP_2DROP ... <pubkey> OP_CHECKSIG
```

1. Push arbitrary data onto stack
2. Drop all data (ignored during validation)
3. Standard P2PK lock for ownership
4. Unlock with owner's signature

**Key Distinction**: Push Drop is a **data carrier**, not a token standard. Higher-layer overlays define token semantics.

**Technical Details**:
```
// Example Push Drop output script:
<token_metadata> <amount> <token_id> OP_DROP OP_DROP OP_DROP <owner_pubkey> OP_CHECKSIG

// Unlocking:
<owner_signature>
```

**Relationship to BRC-100**:
- Push Drop is the **output format** used by overlay tokens
- Hodos already implements Push Drop via baskets
- `customInstructions` field stores arbitrary metadata
- Tokens represented as UTXOs tracked in baskets

**Pros**:
- **Already implemented in Hodos** ✅
- Flexible - any data can be attached
- Standard UTXO model
- Compatible with overlay validation

**Cons**:
- Not a complete token standard
- Requires overlay for validation rules
- No built-in ecosystem (custom per use case)

---

## Comparison Matrix

| Feature | BSV-20 | BSV-21 | STAS | BRC Push Drop |
|---------|--------|--------|------|---------------|
| **Data Format** | JSON inscription | JSON inscription | Custom metadata | Arbitrary bytes |
| **Embedding** | OP_FALSE OP_IF | OP_FALSE OP_IF | UTXO metadata | OP_DROP |
| **Token ID** | Ticker string | `txid_vout` | `issuer-symbol` | Overlay-defined |
| **Validation** | Full chain scan | DAG trace | Miner-enforced | Overlay |
| **Indexer Needed** | Yes (complex) | Yes (simpler) | No | Depends |
| **Min Satoshis** | 1 per UTXO | 1 per UTXO | 1 per token unit | 1 per UTXO |
| **Parallelization** | UTXO model | UTXO model | UTXO model | UTXO model |
| **Smart Contracts** | No | Yes (locking) | Yes (script) | No |
| **NFT Support** | Via ordinals | Via ordinals | STAS NFT | Via overlay |
| **Ecosystem** | Legacy | Active (GorillaPool) | Enterprise | Custom |
| **Complexity** | Medium | Medium | High | Low |

---

## What Hodos Already Has

### Current BRC-100 Wallet Capabilities

| Capability | Status | Relevance to Tokens |
|------------|--------|---------------------|
| **Baskets** | ✅ Complete | Group token UTXOs by type |
| **Tags** | ✅ Complete | Additional categorization |
| **Push Drop** | ✅ Complete | Data-carrying output format |
| **customInstructions** | ✅ Complete | Store token metadata |
| **outputDescription** | ✅ Complete | Human-readable token info |
| **listOutputs** | ✅ Complete | Query tokens by basket/tag |
| **createAction** | ✅ Complete | Build token transfers |
| **BEEF/SPV** | ✅ Complete | Merkle proofs for tokens |
| **HD Key Derivation** | ✅ Complete | Per-address key control |
| **BRC-42 Derivation** | ✅ Complete | Privacy-preserving addresses |

### What's Missing for BSV-21

| Required | Description | Effort |
|----------|-------------|--------|
| **Inscription Parser** | Parse OP_FALSE OP_IF envelopes | 2-3 days |
| **BSV-21 JSON Parser** | Extract token data from inscriptions | 1 day |
| **GorillaPool API Client** | Fetch balances, validate DAG | 2-3 days |
| **Token Sync** | Discover token UTXOs we own | 2-3 days |
| **Transfer Builder** | Create inscription outputs | 2-3 days |
| **UI Components** | Display tokens, send form | 3-5 days |

**Total Estimate**: 12-17 days for full BSV-21 support

---

## Wallet Database Requirements

### Current Schema (V14)

Already have:
- `utxos` table with `basket_id`, `custom_instructions`, `output_description`
- `baskets` table for categorization
- `output_tags` for additional labeling

### Proposed Additions for BSV-21

```sql
-- Token metadata cache (avoid repeated API calls)
CREATE TABLE token_metadata (
    token_id TEXT PRIMARY KEY,      -- e.g., "abc123_0"
    symbol TEXT,
    decimals INTEGER DEFAULT 0,
    icon_origin TEXT,
    max_supply TEXT,                -- BigInt as string
    deploy_height INTEGER,
    cached_at TEXT                  -- ISO timestamp
);

-- Link UTXOs to tokens (a UTXO can hold one token type)
CREATE TABLE token_utxos (
    id INTEGER PRIMARY KEY,
    utxo_id INTEGER NOT NULL REFERENCES utxos(id) ON DELETE CASCADE,
    token_id TEXT NOT NULL,
    amount TEXT NOT NULL,           -- BigInt as string
    UNIQUE(utxo_id)
);

CREATE INDEX idx_token_utxos_token_id ON token_utxos(token_id);
```

### Basket Strategy

Use baskets to organize tokens:
```
default           → Regular BSV UTXOs
bsv21_tokens      → BSV-21 fungible tokens
ordinals_nfts     → NFT inscriptions
stas_tokens       → STAS tokens (future)
```

---

## Implementation Recommendation

### Phase 1: BSV-21 Support (Recommended First)

**Why BSV-21**:
1. **Active ecosystem** - GorillaPool indexer, trading platforms, existing users
2. **Simpler validation** - DAG tracing vs full chain scan
3. **Tooling available** - Reference implementations in TypeScript/Go
4. **Market demand** - Popular tokens already exist
5. **Compatible architecture** - Fits our UTXO/basket model

**Implementation Approach** (already documented in Plan A/B):
1. External indexer (GorillaPool) for balance validation
2. Local inscription parsing for display
3. Transfer via `createAction` with inscription outputs
4. Store in `bsv21_tokens` basket

### Phase 2: Ordinals/NFT Display (Optional)

If users want to display NFT inscriptions:
- Same inscription parsing infrastructure
- Grid display for images
- Content-type aware rendering

### Phase 3: STAS Support (Future Consideration)

**When to consider STAS**:
- Enterprise customers requiring miner-enforced validation
- Regulated asset tokenization
- Use cases requiring legal compliance

**Complexity**: Higher than BSV-21 due to:
- Script template requirements
- Miner validation constraints
- Less ecosystem tooling

### What NOT to Implement

**Skip BSV-20 (first-is-first mode)**:
- Superseded by BSV-21
- Complex validation
- Ticker squatting issues
- No advantages over BSV-21

---

## BRC-100 Wallet + BSV-21: How They Fit Together

### The Key Insight

BSV-21 tokens are just **inscribed UTXOs**. Our BRC-100 wallet already manages UTXOs via baskets. The integration is natural:

```
BSV-21 Token UTXO
├── Tracked in basket: "bsv21_tokens"
├── customInstructions: {"protocol":"bsv-21", "token_id":"...", "amount":"..."}
├── outputDescription: "100 TEST tokens"
└── Spendable via createAction (with inscription in output)
```

### No BRC-100 Wallet Handles BSV-21 Yet

This is an opportunity. If Hodos ships BSV-21 support:
- First BRC-100 wallet with token support
- Can trade on existing 1Sat Ordinals marketplaces
- Differentiator from other BSV wallets

### Architecture Compatibility

| BRC-100 Concept | BSV-21 Mapping |
|-----------------|----------------|
| Basket | Token type grouping |
| UTXO | Token balance unit |
| customInstructions | Token metadata |
| createAction | Token transfer builder |
| listOutputs | Token balance query |

---

## Answering Your Questions

### Can we implement BSV-20/21 in our BRC-100 wallet?

**Yes, absolutely.** The architecture aligns well:

1. **UTXO model**: Both BRC-100 and BSV-21 use UTXOs
2. **Baskets**: Natural fit for token categorization
3. **createAction**: Can build inscription outputs
4. **Metadata storage**: `customInstructions` holds token data

### What about Push Drop functionality?

**You already have it.** Push Drop is a script template for attaching data to UTXOs. Your basket system with `customInstructions` is essentially this - data attached to spendable outputs.

BSV-21 uses a different envelope (OP_FALSE OP_IF), but the concept is similar. You'd add envelope parsing, not replace Push Drop.

### Ordinals popularity vs no BRC-100 support?

This is the gap to fill. BSV-21/ordinals have:
- Active trading (1satordinals.com)
- Established tokens
- User demand

But no BRC-100 wallet supports them. Hodos could be first.

### Protocol Priority?

1. **BSV-21** - Active ecosystem, clear demand, reasonable effort
2. **NFT Display** - Same infrastructure, visual appeal
3. **STAS** - Enterprise use case, higher complexity

---

## Files in This Directory

| File | Description |
|------|-------------|
| `BSV_TOKEN_PROTOCOLS_COMPARISON.md` | This document |
| `BSV21_1SAT_ORDINALS_ANALYSIS.md` | Original BSV-21 research (Jan 2026) |
| `BSV21_PLAN_A_BACKEND.md` | Rust implementation plan |
| `BSV21_PLAN_B_FRONTEND.md` | React UI plan |
| `BSV21_UX_DESIGN_OUTLINE.md` | UX considerations |

---

## References

### Official Documentation
- [BSV-21 Specification](https://docs.1satordinals.com/fungible-tokens/bsv-21)
- [BRC-48 Push Drop](https://bsv.brc.dev/scripts/0048)
- [STAS Token Protocol](https://stastoken.com)
- [BSV Technical Standards (DPP)](https://tsc.bsvblockchain.org/standards/direct-payment-protocol/)

### APIs & Tools
- [GorillaPool Ordinals API](https://ordinals.gorillapool.io/api/docs)
- [js-1sat-ord Library](https://github.com/BitcoinSchema/js-1sat-ord)
- [WhatsOnChain](https://whatsonchain.com)

### Code References
- [1sat-ordinals Spec](https://github.com/BitcoinSchema/1sat-ordinals)
- [bsv20-indexer](https://github.com/BitcoinSchema/bsv20-indexer)
- [PushDrop Library](https://github.com/p2ppsr/pushdrop)
