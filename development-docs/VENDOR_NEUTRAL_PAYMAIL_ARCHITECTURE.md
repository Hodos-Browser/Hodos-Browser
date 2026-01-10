# Vendor-Neutral Paymail Architecture

## Vision

A paymail hosting service that:
- **No API keys** - Payment is the authentication
- **No vendor lock-in** - Any wallet can use it
- **Micropayment funded** - Pay-per-use, fractions of a cent
- **Open protocol** - Documented so any wallet can integrate
- **Decentralized** - On-chain registry, anyone can run a node
- **Built on open source** - BSV Association's SPV Wallet as foundation

**Example**: `alice@openpaymail.com` - works with any BSV wallet, costs ~$0.001/month in actual usage

---

## The Problem with Current Paymail

| Provider | Lock-in | Cost Model | Data Ownership |
|----------|---------|------------|----------------|
| HandCash | High - their ecosystem | Free (they monetize data/services) | They control |
| Centbee | High - their app | Free (they monetize) | They control |
| Self-hosted | None | Infrastructure cost (~$50+/mo) | You control |

**Gap**: No middle ground between "free but locked-in" and "self-host everything"

---

## Proposed Solution: Hybrid Architecture

Combine on-chain registry with overlay indexing and micropayment gateway:

```
┌─────────────────────────────────────────────────────────────┐
│                     Paymail Service                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐   │
│  │   Registry   │    │   Overlay    │    │   Payment    │   │
│  │  (on-chain)  │◄──►│   Indexer    │◄──►│   Gateway    │   │
│  └──────────────┘    └──────────────┘    └──────────────┘   │
│         │                   │                   │            │
│         │                   │                   │            │
│         ▼                   ▼                   ▼            │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              Standard Paymail Endpoints              │    │
│  │  /.well-known/bsvalias  (free - public discovery)   │    │
│  │  /p2p-destination       (micropayment required)     │    │
│  │  /receive-tx            (free - sender paid)        │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Three Components

| Component | Purpose | Decentralized? |
|-----------|---------|----------------|
| **On-Chain Registry** | Permanent record of paymail registrations | Yes - blockchain |
| **Overlay Indexer** | Fast lookups, anyone can run a node | Yes - federated |
| **Payment Gateway** | Process micropayments for operations | Semi - per operator |

---

## On-Chain Registry

### How Registration Works

When a user registers `alice@openpaymail.com`:

```
Transaction:
├── Input: User's UTXO (covers fees + registration cost)
├── Output 0: Service fee (1000 sats to operator)
└── Output 1: OP_RETURN
              ├── Protocol ID: "paymail-registry"
              ├── Alias: "alice"
              ├── Domain: "openpaymail.com"
              └── Public Key: <33-byte compressed secp256k1>
```

**Key insight**: The payment IS the registration. No separate payment step needed.

### Why On-Chain?

| Benefit | Explanation |
|---------|-------------|
| **Permanent** | Records exist as long as Bitcoin exists |
| **Censorship-resistant** | No one can delete your registration |
| **Auditable** | Anyone can verify the registry |
| **Portable** | User owns their record, can prove ownership |
| **Trustless** | Don't have to trust any single operator |

### Record Types

```
# Registration
OP_FALSE OP_RETURN "paymail" "register" <alias> <domain> <pubkey>

# Key Update (must be signed by current key)
OP_FALSE OP_RETURN "paymail" "update-key" <alias> <domain> <new-pubkey> <signature>

# Transfer (change owner)
OP_FALSE OP_RETURN "paymail" "transfer" <alias> <domain> <new-owner-pubkey> <signature>
```

---

## Overlay Network

### What is an Overlay?

An overlay network is a layer of **indexing nodes** that:
1. Watch the blockchain for relevant transactions
2. Parse and index the data
3. Provide fast query APIs
4. Sync with each other

```
┌─────────────────────────────────────────────────────────────┐
│                      Blockchain                              │
│  [Block 800000]──[Block 800001]──[Block 800002]──...        │
│       │               │               │                      │
│    paymail TX      paymail TX      paymail TX               │
│    (alice)         (bob)           (carol)                  │
└─────────────────────────────────────────────────────────────┘
              │               │               │
              ▼               ▼               ▼
        ┌─────────────────────────────────────────┐
        │         Overlay Topic: "paymail"        │
        ├─────────────────────────────────────────┤
        │                                         │
        │  ┌─────────┐  ┌─────────┐  ┌─────────┐ │
        │  │ Node 1  │  │ Node 2  │  │ Node 3  │ │
        │  │ (US)    │  │ (EU)    │  │ (Asia)  │ │
        │  └─────────┘  └─────────┘  └─────────┘ │
        │       ▲            ▲            ▲      │
        │       └────────────┼────────────┘      │
        │              Peer Sync                 │
        └─────────────────────────────────────────┘
```

### How New Nodes Sync

**Anyone can start an overlay node.** New nodes catch up via:

**Method 1: Peer Sync (Fast)**
```
New Node                    Existing Node
    │                            │
    │  "Give me all paymail     │
    │   records since block 0"  │
    │───────────────────────────►│
    │                            │
    │  [alice, bob, carol, ...]  │
    │◄───────────────────────────│
    │                            │
    │  "What's latest block?"    │
    │───────────────────────────►│
    │                            │
    │  "Block 850000"            │
    │◄───────────────────────────│
    │                            │
    │  (Now synced, watch new)   │
```

**Method 2: Chain Scan (Trustless)**
```
New Node                    Blockchain
    │                            │
    │  Scan all blocks for TXs   │
    │  with "paymail" prefix     │
    │───────────────────────────►│
    │                            │
    │  Parse, validate, index    │
    │  each registration TX      │
    │                            │
    │  (Slower but trustless)    │
```

**In practice**: Use peer sync for speed, optionally verify with chain scan.

### Node Incentives

Why would someone run a node?

| Incentive | Explanation |
|-----------|-------------|
| **Service fees** | Collect micropayments for lookups |
| **Own users** | Host your own paymail users |
| **Altruism** | Support the ecosystem |
| **Business need** | Your app needs reliable lookups |

---

## Payment Gateway

### Micropayment Pricing

| Operation | Cost (sats) | ~USD | Who Pays |
|-----------|-------------|------|----------|
| Register paymail | 1000 | $0.01 | User (on-chain) |
| Get payment destination | 5 | $0.00005 | Sender wallet |
| Receive transaction | 0 | Free | Sender already paid |
| Key update | 10 | $0.0001 | User (on-chain) |
| Lookup/resolve | 1 | $0.00001 | Requesting wallet |

### How Payment Works

**For on-chain operations** (register, update):
- Payment is built into the transaction
- Output pays the service operator
- Processed when TX confirms

**For API operations** (lookups, destinations):
- Wallet includes payment proof in request header
- Server validates before processing

```http
POST /api/p2p-destination/alice@openpaymail.com HTTP/1.1
Content-Type: application/json
X-BSV-Payment: beef=0100beef...,txid=abc123,vout=0

{"satoshis": 10000}
```

### Payment Validation Flow

```
┌──────────┐                              ┌──────────────────┐
│  Wallet  │                              │  Paymail Server  │
└────┬─────┘                              └────────┬─────────┘
     │                                             │
     │  1. Request + Payment BEEF                  │
     │────────────────────────────────────────────►│
     │                                             │
     │                    2. Validate payment:     │
     │                       - Correct amount?     │
     │                       - Pays our address?   │
     │                       - Valid signatures?   │
     │                       - Not double-spent?   │
     │                                             │
     │                    3. Broadcast payment TX  │
     │                                             │
     │                    4. Process request       │
     │                                             │
     │  5. Response                                │
     │◄────────────────────────────────────────────│
```

---

## Wallet Integration

### What Wallets Need to Support

For wallets to use this service:

**Minimum (sending to paymail)**:
1. Resolve paymail via standard protocol
2. Detect payment requirement (402 or capability field)
3. Include micropayment in request

**Full (owning a paymail)**:
1. Create registration transaction
2. Broadcast to network
3. Sign key updates when needed

### Capability Advertisement

Extended `/.well-known/bsvalias`:

```json
{
  "bsvalias": "1.0",
  "capabilities": {
    "2a40af698840": "https://openpaymail.com/api/p2p-dest/{alias}@{domain.tld}",
    "5f1323cddf31": "https://openpaymail.com/api/receive-tx/{alias}@{domain.tld}",
    "pki": "https://openpaymail.com/api/id/{alias}@{domain.tld}"
  },
  "paymentRequired": {
    "enabled": true,
    "pricing": {
      "p2p-destination": 5,
      "lookup": 1
    },
    "address": "1ServiceAddress...",
    "acceptedFormats": ["beef", "rawtx"]
  },
  "registry": {
    "type": "on-chain",
    "protocol": "paymail-registry",
    "registrationCost": 1000
  }
}
```

---

## Economic Model

### Operator Costs

| Item | Monthly Cost |
|------|--------------|
| VPS (2 CPU, 4GB RAM) | $20-40 |
| Domain | ~$1 |
| SSL | Free (Let's Encrypt) |
| Bandwidth | $5-20 |
| **Total** | **~$30-60/month** |

### Revenue Model

Example with 10,000 active users:

| Operation | Volume/month | Sats | USD |
|-----------|--------------|------|-----|
| New registrations | 500 | 500,000 | $5.00 |
| Payment destinations | 50,000 | 250,000 | $2.50 |
| Lookups | 100,000 | 100,000 | $1.00 |
| Key updates | 100 | 1,000 | $0.01 |
| **Total** | | **851,000** | **~$8.50** |

**Break-even**: ~50,000 active users at current pricing
**Profitable**: Scale to 100k+ users, costs stay relatively flat

### Why This Works

- **Micropayments are viable on BSV** - TX fees are fractions of a cent
- **No billing infrastructure** - Payment IS authentication
- **Scales horizontally** - Add more overlay nodes as needed
- **Competition keeps prices low** - Anyone can run a competing service

---

## Implementation Phases

### Phase 1: MVP (2-3 months)

**Goal**: Working service with on-chain registration

**Deliverables**:
- Fork SPV Wallet codebase
- Add on-chain registry parser
- Implement payment validation
- Deploy single-node service
- Test with HodosBrowser

**Architecture**:
```
[Single Server]
├── Blockchain watcher (index registrations)
├── SQLite database (fast lookups)
├── Payment gateway (validate micropayments)
└── Standard paymail endpoints
```

### Phase 2: Decentralization (2-3 months)

**Goal**: Federated overlay network

**Deliverables**:
- Peer sync protocol
- Multi-node deployment
- Node discovery mechanism
- Write BRC specification

**Architecture**:
```
[Node 1] ◄──► [Node 2] ◄──► [Node 3]
    │             │             │
    └─────────────┴─────────────┘
                  │
            [Blockchain]
```

### Phase 3: Ecosystem (Ongoing)

**Goal**: Adoption by other wallets

**Deliverables**:
- TypeScript client library
- Rust client library
- Integration guides
- Reference implementations

---

## Open Questions

### Technical

1. **Domain verification**: How to prove ownership of domain for custom domains?
   - DNS TXT record?
   - Start with single shared domain?

2. **Alias disputes**: What if someone squats on a name?
   - First-come-first-served?
   - Higher registration fees?
   - Dispute resolution process?

3. **Key recovery**: What if user loses private key?
   - Social recovery?
   - Backup key during registration?

### Business

4. **Initial operator**: Who runs the first nodes?
   - HodosBrowser team?
   - BSV Association?
   - Community-funded?

5. **Governance**: How are protocol changes decided?
   - BRC process?
   - Node operator voting?

6. **Naming**: What's the domain?
   - openpaymail.com?
   - neutralpaymail.com?
   - paymail.bsv?

---

## Why This Matters

### For Users
- **Own your identity** - Not locked to any wallet
- **Portable** - Switch wallets, keep your paymail
- **Cheap** - Fractions of a cent per year
- **Censorship-resistant** - On-chain registration

### For Wallet Developers
- **No vendor lock-in** - Compete on features, not lock-in
- **Standard protocol** - Implement once, works everywhere
- **No API keys** - Just send micropayments

### For the Ecosystem
- **True peer-to-peer** - No central identity providers
- **Sustainable** - Pays for itself via micropayments
- **Open** - Anyone can run a node or compete

---

## Next Steps

1. **Get feedback** on this architecture
2. **Find collaborators** interested in building/operating
3. **Choose domain** and initial operator
4. **Fork SPV Wallet** and start building
5. **Test with HodosBrowser** as first wallet
6. **Write BRC specification** for standardization
7. **Launch beta** with limited users

---

## References

- [SPV Wallet (BSV Association)](https://github.com/bitcoin-sv/spv-wallet)
- [BRC-28: Paymail Payment Destinations](https://bsv.brc.dev/payments/0028)
- [BSV Overlay Networks](https://docs.bsvblockchain.org/network-topology/overlay-networks)
- [Paymail Specification](https://docs.bsvblockchain.org/paymail)

---

**Created**: January 9, 2025
**Status**: Architecture Proposal
**Feedback**: [Contact info here]
