# MNEE Stablecoin Implementation Guide

> **Purpose**: Document what's needed to implement MNEE (USD stablecoin) support in Hodos Browser's BRC-100 wallet.
>
> **Created**: March 2026
> **Status**: Research Complete

---

## Executive Summary

**MNEE** (pronounced "money") is a USD-backed stablecoin on BSV using the 1Sat Ordinals / BSV-21 protocol. It's fully collateralized with US Treasury bills and cash equivalents, regulated in Antigua, and designed to be GENIUS Act compliant.

### Why MNEE is Interesting for Hodos

| Factor | Details |
|--------|---------|
| **Protocol** | BSV-21 (same as general token support) |
| **Real utility** | Stablecoin = actual payment use case |
| **SDK available** | TypeScript + Go SDKs, well-documented |
| **Ecosystem** | HandCash, exchanges (BitMart, LBank), Banxa on-ramp |
| **Gasless** | No separate gas token needed (unlike ETH) |
| **Fees** | Under 1 cent per transaction |

### Implementation Verdict

**MNEE support comes "free" with BSV-21 implementation.** MNEE is just a BSV-21 token with a specific token ID. Once we support BSV-21 tokens generally, MNEE works automatically.

However, we could add **MNEE-specific features** to enhance the experience:
- Display balance in USD (not just token units)
- On-ramp/off-ramp integration (Banxa)
- MNEE Pay merchant checkout support

---

## What is MNEE?

### Technical Specs

| Spec | Value |
|------|-------|
| **Protocol** | BSV-21 (1Sat Ordinals) |
| **Peg** | 1 MNEE = 1 USD |
| **Atomic units** | 1 MNEE = 100,000 atomic units |
| **Decimals** | 5 |
| **Backing** | US Treasury bills, USD cash, cash equivalents |
| **Auditor** | Wolf & Company, P.C. (monthly attestations) |
| **Regulator** | FSRC (Antigua and Barbuda) |
| **Chains** | BSV (primary), Ethereum ERC-20 (secondary) |

### Key Features

- **Gasless UX**: No need for users to hold separate gas tokens
- **Instant finality**: Transactions settle in <1 second
- **Sub-penny fees**: Typical tx cost < $0.01
- **HD wallet compatible**: Standard BIP32/BIP44 derivation (`m/44'/236'/0'`)

---

## MNEE SDK Overview

MNEE provides official SDKs that abstract away BSV-21 complexity:

### TypeScript SDK (`@mnee/ts-sdk`)

```typescript
import Mnee from '@mnee/ts-sdk';

const mnee = new Mnee({
  environment: 'production', // or 'sandbox'
  apiKey: 'your-api-key',    // optional but recommended
});

// Check balance
const balance = await mnee.balance('1YourAddress...');
console.log(`Balance: ${balance.decimalAmount} MNEE`);

// Transfer
const recipients = [{ address: '1Recipient...', amount: 10.50 }];
const response = await mnee.transfer(recipients, 'private-key-wif');
const status = await mnee.getTxStatus(response.ticketId);
console.log('TX ID:', status.tx_id);
```

### Go SDK (`go-mnee-1sat-sdk`)

Available for backend services.

### Key SDK Methods

| Method | Purpose |
|--------|---------|
| `balance(address)` | Get MNEE balance for single address |
| `balances(addresses[])` | Batch balance check |
| `transfer(recipients, wif)` | Simple transfer with auto UTXO selection |
| `transferMulti({inputs, recipients})` | Multi-source transfer (consolidation) |
| `getUtxos(addresses)` | Get MNEE UTXOs |
| `getTxStatus(ticketId)` | Check transaction status |
| `parseTx(txid)` | Parse transaction details |
| `toAtomicAmount(decimal)` | Convert MNEE to atomic units |
| `fromAtomicAmount(atomic)` | Convert atomic to MNEE |

---

## Implementation Options

### Option 1: Native BSV-21 Support (Recommended)

Implement general BSV-21 token support (as outlined in `BSV_TOKEN_PROTOCOLS_COMPARISON.md`). MNEE becomes one of many supported tokens.

**Advantages**:
- One implementation supports all BSV-21 tokens
- Consistent architecture
- MNEE works automatically

**What we build**:
- Inscription parser (OP_FALSE OP_IF envelope)
- GorillaPool API client for token queries
- Token sync (discover MNEE UTXOs we own)
- Transfer builder (create inscription outputs)
- Token display in wallet UI

**MNEE-specific additions**:
- Recognize MNEE token ID → show USD symbol/icon
- Display balance as "$X.XX" instead of "X MNEE"
- Optional: Stablecoin section in wallet UI

### Option 2: MNEE SDK Integration

Use MNEE's TypeScript SDK directly in the frontend, bypassing our Rust backend.

**Advantages**:
- Faster to implement
- SDK handles all complexity
- Direct access to MNEE features

**Disadvantages**:
- SDK manages keys (privacy concern)
- Separate from our BRC-100 architecture
- Duplicate UTXO management
- Frontend complexity

**Not recommended** - goes against our architecture where Rust wallet manages all keys/UTXOs.

### Option 3: Hybrid (Native + SDK for Advanced Features)

Use native BSV-21 for core token support, but integrate MNEE SDK for advanced features like:
- On-ramp via Banxa
- Merchant checkout (MNEE Pay)
- Batch operations

**When to consider**: If MNEE-specific features become important beyond basic hold/send.

---

## Implementation Plan (Option 1)

### Phase 1: General BSV-21 Support

This is the same work as the general token implementation:

| Component | Effort | Description |
|-----------|--------|-------------|
| Inscription parser | 2-3 days | Parse OP_FALSE OP_IF envelopes |
| GorillaPool client | 2-3 days | API client for token queries |
| Token UTXO sync | 2-3 days | Discover tokens we own |
| Transfer builder | 2-3 days | Create BSV-21 transfer inscriptions |
| DB schema | 1 day | `token_metadata`, `token_utxos` tables |
| Token list UI | 2-3 days | Display tokens in wallet |
| Token send UI | 2-3 days | Transfer form |

**Subtotal**: 12-17 days (same as general BSV-21 estimate)

### Phase 2: MNEE-Specific Enhancements

| Feature | Effort | Description |
|---------|--------|-------------|
| MNEE recognition | 0.5 day | Detect MNEE token ID, show USD formatting |
| Stablecoin section | 1 day | Dedicated UI area for stablecoins |
| USD display | 0.5 day | Show "$10.50" instead of "10.50 MNEE" |

**Subtotal**: 2 days

### Phase 3: Advanced Features (Optional)

| Feature | Effort | Description |
|---------|--------|-------------|
| On-ramp integration | 3-5 days | Banxa widget for buying MNEE |
| MNEE Pay support | 2-3 days | Merchant checkout integration |
| Batch operations | 2 days | Consolidation, multi-recipient |

**Subtotal**: 7-10 days (if needed)

---

## Database Schema

Same as general BSV-21 support, with MNEE-specific metadata:

```sql
-- Token metadata cache
CREATE TABLE token_metadata (
    token_id TEXT PRIMARY KEY,
    symbol TEXT,
    decimals INTEGER DEFAULT 0,
    icon_origin TEXT,
    max_supply TEXT,
    deploy_height INTEGER,
    is_stablecoin BOOLEAN DEFAULT FALSE,  -- NEW: Flag for special treatment
    fiat_symbol TEXT,                      -- NEW: e.g., "USD" for MNEE
    cached_at TEXT
);

-- Seed MNEE metadata
INSERT INTO token_metadata (token_id, symbol, decimals, is_stablecoin, fiat_symbol)
VALUES ('<mnee_token_id>', 'MNEE', 5, TRUE, 'USD');
```

---

## MNEE Token ID

The MNEE token ID (BSV-21 format: `<txid>_<vout>`) needs to be discovered from GorillaPool or MNEE documentation. This is the genesis transaction where MNEE was deployed+minted.

Once we have it, we can:
1. Hardcode it for recognition
2. Auto-detect via GorillaPool token metadata

```rust
const MNEE_TOKEN_ID: &str = "<mnee_deploy_txid>_0";

fn is_mnee_token(token_id: &str) -> bool {
    token_id == MNEE_TOKEN_ID
}

fn format_token_amount(token_id: &str, amount: &str, decimals: u8) -> String {
    if is_mnee_token(token_id) {
        // Format as USD
        format!("${:.2}", parse_decimal(amount, decimals))
    } else {
        // Format as token
        format!("{} {}", parse_decimal(amount, decimals), get_symbol(token_id))
    }
}
```

---

## Integration with BRC-100 Wallet

### How MNEE Fits Our Architecture

```
MNEE UTXO
├── Stored in: utxos table (satoshis = 1)
├── Basket: "stablecoins" or "bsv21_tokens"  
├── customInstructions: {
│     "protocol": "bsv-21",
│     "token_id": "<mnee_token_id>",
│     "amount": "1050000",  // 10.50 MNEE in atomic
│     "is_stablecoin": true
│   }
├── outputDescription: "$10.50 MNEE"
└── Linked via: token_utxos table → token_metadata
```

### Wallet UI Display

```
┌─────────────────────────────────────────┐
│  HODOS WALLET                           │
├─────────────────────────────────────────┤
│  BSV Balance                            │
│  ₿ 0.00234521 (~$1.23)                 │
├─────────────────────────────────────────┤
│  Stablecoins                            │
│  💵 $152.50 MNEE                        │
├─────────────────────────────────────────┤
│  Tokens                                 │
│  🪙 1,000 TEST                          │
│  🪙 500 PEPE                            │
└─────────────────────────────────────────┘
```

### Transfer Flow

1. User selects MNEE in wallet
2. Enters recipient address + amount (in USD)
3. We convert to atomic units internally
4. Build BSV-21 transfer inscription via `createAction`
5. Sign and broadcast
6. Update local token UTXOs

---

## API Considerations

### MNEE Has Its Own API

MNEE SDK uses their own backend (not GorillaPool directly):
- `api.mnee.io` (production)
- `sandbox.api.mnee.io` (testing)

For basic BSV-21 support, we can use GorillaPool. But for MNEE-specific features (on-ramp, merchant), we'd call MNEE API.

### Rate Limits & API Keys

MNEE recommends API keys for production use. Free tier has rate limits.

**For basic support**: GorillaPool is sufficient
**For advanced features**: MNEE API key needed

---

## Partnerships & Ecosystem

### Where MNEE is Integrated

| Partner | Integration |
|---------|-------------|
| **HandCash** | Wallet support |
| **Banxa** | Fiat on-ramp (150+ countries) |
| **BitMart** | Exchange trading |
| **LBank** | Exchange trading |
| **io.finnet** | Enterprise treasury |
| **TextBSV** | SMS payments |
| **MNEE Pay** | Merchant checkout |

### What This Means for Hodos

- Users can buy MNEE via Banxa, send to Hodos
- Trade on exchanges, withdraw to Hodos
- Eventually: Direct Banxa integration in browser

---

## Testing Strategy

### Sandbox Environment

MNEE provides a sandbox for testing:
```typescript
const mnee = new Mnee({
  environment: 'sandbox',
  apiKey: 'test-key'
});
```

### Test Cases

1. **Balance query**: Check MNEE balance for test address
2. **Receive**: Send MNEE to Hodos address, verify detection
3. **Send**: Transfer MNEE out, verify inscription format
4. **Display**: Confirm USD formatting works
5. **UTXO management**: Multiple receive/sends, verify consolidation

---

## Recommendation

### My Take

**Implement BSV-21 first, MNEE comes free.**

The work for general BSV-21 support (12-17 days) automatically enables MNEE. Adding MNEE-specific polish (USD display, stablecoin section) is ~2 more days.

**Priority order**:
1. Sprint 10 (Scriptlet compat) - fixes x.com auth
2. BSV-21 token support - enables MNEE + all ordinal tokens
3. MNEE polish - stablecoin UX

**Future (if demand exists)**:
- Banxa on-ramp integration
- MNEE Pay merchant support

### Why This Matters

A BRC-100 wallet with stablecoin support is compelling:
- **Users can hold USD value** without exchange custody
- **Payment utility** beyond just holding crypto
- **First mover** - no other BRC-100 wallets have this
- **Real-world use case** for the browser/wallet combo

---

## References

### Official Resources
- [MNEE Website](https://www.mnee.io)
- [MNEE Documentation](https://docs.mnee.io)
- [MNEE GitHub](https://github.com/mnee-xyz)
- [MNEE TypeScript SDK](https://github.com/mnee-xyz/mnee)

### APIs
- Production: `api.mnee.io`
- Sandbox: `sandbox.api.mnee.io`
- [MNEE Pay (Merchants)](https://mneepay.io)

### Related
- [BSV-21 Specification](https://docs.1satordinals.com/fungible-tokens/bsv-21)
- [GorillaPool Ordinals API](https://ordinals.gorillapool.io/api/docs)
- [Banxa Integration](https://banxa.com)

---

## Appendix: MNEE vs Other Stablecoins

| Feature | MNEE | USDT (ETH) | USDC (ETH) |
|---------|------|------------|------------|
| **Chain** | BSV | Ethereum | Ethereum |
| **Fee** | <$0.01 | $1-50+ | $1-50+ |
| **Speed** | <1 sec | 15-60 sec | 15-60 sec |
| **Gas token** | None | ETH required | ETH required |
| **Backing** | T-bills + cash | Mixed | Cash + T-bills |
| **Audit** | Monthly (Wolf) | Quarterly | Monthly |
| **Regulatory** | FSRC (Antigua) | Varies | NY DFS |

MNEE's BSV-native advantages (speed, cost, no gas) make it compelling for micropayments and frequent transactions.
