# Vendor-Neutral Paymail Architecture

## Abstract

A decentralized, confederated payment addressing system that enables human-readable identifiers (`alice@paymail`) **without domain dependencies**. The system uses BitcoinSV blockchain for registration and overlay networks for resolution, eliminating the need for DNS and domain registration. **Confederation enables vendor-neutrality** - multiple independent operators run overlay service nodes, ensuring no single vendor controls the system. Payments authenticate operations through micropayments, eliminating API keys and vendor lock-in.

**Core Innovation**: Protocol identifiers (`@paymail`) replace domain names, and blockchain-based overlay resolution replaces DNS lookup. This removes centralization points (registrars, ICANN, DNS) while maintaining human-readable addresses.

---

## Vision

**Core Principles:**
- **No domains required** - Protocol identifiers replace DNS
- **No API keys** - Micropayment-based authentication
- **No vendor lock-in** - Any wallet can implement the protocol
- **Blockchain-based** - BitcoinSV provides permanent, censorship-resistant registry
- **Standards-based** - Built on established BRC protocols

**Example**: `alice@paymail` - The `@paymail` suffix signals overlay resolution, not DNS lookup. No domain registration needed.

---

## Architecture Overview

The system consists of three layers:

1. **Wallet Layer** - Recognizes paymail addresses and queries overlay services
2. **Overlay Network** - BRC-22 Topic Managers validate transactions, BRC-24 Lookup Services index aliases
3. **Blockchain** - Permanent, censorship-resistant registry of paymail registrations

Wallets query BRC-24 Lookup Services to resolve aliases to public keys. The overlay network uses blockchain as the source of truth, eliminating DNS dependencies. BRC-33 message relay handles offline delivery.

---

## How BitcoinSV Removes Domain Dependency

### Traditional Paymail (Domain-Based)

**Traditional approach**: `alice@example.com`
- Requires domain registration (`example.com`)
- Uses DNS to resolve domain to server IP address
- Centralized: DNS controlled by ICANN/registrars
- Censorship risk: Domains can be seized

### Vendor-Neutral Paymail (Blockchain-Based)

**New approach**: `alice@paymail`
- **No domain registration** - `@paymail` is a protocol identifier, not a domain
- **No DNS lookup** - Overlay network resolves aliases using blockchain data
- **Decentralized** - Blockchain provides permanent, censorship-resistant registry
- **No centralization** - No ICANN, no registrars, no DNS dependency

### The Key Difference

| Aspect | Traditional Paymail | Vendor-Neutral Paymail |
|--------|--------------------|--------------------|
| Address format | `alice@domain.com` | `alice@paymail` |
| Resolution | DNS lookup | Overlay network lookup |
| Registry | Domain registrar | BitcoinSV blockchain |
| Dependency | DNS, ICANN, registrars | Blockchain only |
| Censorship | Domain seizure possible | Very difficult |

**Why this matters**: By using blockchain as the registry instead of DNS, we eliminate all centralization points. The `@paymail` suffix is just a signal to wallets: "resolve via overlay, not DNS."

---

## Registration Process

### Transaction Format

Users register paymail aliases by creating on-chain transactions using PushDrop format to embed registration data in transaction outputs. The PushDrop script contains:
- Protocol identifier: `"paymail"`
- Version: `0x01`
- Action: `"register"`
- Alias: The desired identifier (e.g., `"alice"`)
- Public key: 33-byte compressed secp256k1 key (derived via BRC-42/43)
- Optional server address: For direct connectivity
- Optional relay preferences: Fallback nodes for offline delivery

PushDrop allows structured data to be embedded in spendable outputs without affecting spending rules. Unlike OP_RETURN (which creates unspendable outputs), PushDrop tokens can be transferred, traded, or revoked by spending them - giving users full control over their registration via their private key.

**Transaction structure:**
- **Output 0**: Registration fee (1000 sats) paid to overlay operator
- **Output 1**: PushDrop token (1 sat) containing registration data - this IS the paymail registration

The fee payment and registration are atomic - both occur in the same transaction.

### Key Derivation

Paymail identities use BRC-42/43 key derivation for secure key management. Keys are derived from the wallet's master key using standardized protocol IDs.

**Optional: BRC-84 Linked Key Derivation** - Enables deriving child public keys from only the master public key, without requiring private key access. Useful for non-custodial scenarios where a service needs to generate addresses but should never hold keys. Derived keys remain cryptographically linked to the master for auditability.

### Topic Manager Validation

Wallets submit registration transactions to BRC-22 Topic Managers, which validate and admit them. Topic Managers enforce these rules:

- **Alias format**: Alphanumeric characters, underscores, and hyphens only
- **Uniqueness**: First valid registration seen on-chain (in a block) wins - blockchain provides canonical ordering for race conditions
- **Signature verification**: Transaction must be signed by the key embedded in the registration data
- **Fee validation**: Sufficient registration fee must be included in Output 0

Topic Managers reject invalid or duplicate registrations. Once a registration is mined into a block, that alias is permanently claimed - the blockchain's ordering resolves any race conditions.

### Lookup Service Indexing

BRC-24 Lookup Services index admitted transactions by alias for fast resolution. Services synchronize with peer nodes using BRC-88 overlay synchronization protocols. The lookup service maintains a queryable index of all valid paymail registrations, using blockchain as the authoritative source.

---

## Address Resolution

### Suffix Recognition

Wallets recognize standardized suffixes that signal overlay resolution:
- `@paymail` - Generic paymail protocol (recommended default)
- `@bitcoin` - Broader appeal, same protocol
- `@bsv` - Explicit BSV network identifier

**Important**: These suffixes are **protocol identifiers**, not domain names. They require **no domain registration, no DNS setup, and no payment to registrars**. The suffix signals "use overlay lookup, not DNS lookup" - the wallet queries overlay service nodes instead of performing DNS resolution. All suffixes resolve through the same overlay network.

### Lookup Process

1. Wallet parses address into alias and suffix
2. Wallet queries BRC-24 Lookup Service via overlay service nodes
3. Lookup service returns public key and optional server address
4. Wallet uses public key to create payment or connects via BRC-33 relay if recipient is offline

### Overlay Service Node Discovery

Wallets need to discover overlay service nodes (Topic Managers and Lookup Services) to begin querying. These are standard BRC-22/BRC-24 overlay nodes.

**Node Discovery Approaches**:

**Hardcoded List**: Simple fallback list of overlay service nodes embedded in wallet software. Each node entry includes its server address (IP address or domain name). Requires wallet updates to add nodes.

**On-Chain Registry (BRC-23 CHIP)**: Overlay service nodes advertise themselves in blockchain transactions using BRC-23 (Confederacy Host Interconnect Protocol). Nodes create CHIP tokens that contain their service addresses, allowing wallets to discover available Topic Managers and Lookup Services dynamically. This is fully decentralized and self-updating.

**Hybrid**: Hardcoded fallback with periodic on-chain updates. Wallets start with hardcoded nodes, then query blockchain for updated CHIP advertisements. Provides resilience with graceful degradation.

**How CHIP Works**: Nodes create on-chain transactions containing their service information (IP addresses, ports, capabilities). Wallets scan the blockchain for CHIP tokens to discover available overlay nodes. This eliminates the need for centralized node directories while maintaining discoverability.

**Note on Trust**: Overlay service nodes are standard overlay infrastructure. The low entry cost (~$50/month for VPS) does not provide significant costly signaling. Trust derives from operational transparency, on-chain node advertisements via CHIP, and the ability for users to choose among multiple service providers. The decentralized nature of the overlay network reduces reliance on any single node operator.

---

## Payment Delivery

### Payment Flow

Wallets follow standard paymail protocols (BRC-28) to request payment destinations and deliver transactions. The payment flow works through overlay service nodes, which handle routing and delivery.

### BRC-33 Message Relay

When recipients are offline, senders use BRC-33 PeerServ message relay. The relay stores transactions until recipients poll for messages. This provides store-and-forward capability without custom relay protocols.

### Transaction Format

All paymail transactions use BEEF (BRC-58) format for SPV validation. Recipients can verify transactions before broadcasting to the network. BRC-70 specifies the paymail BEEF transaction protocol.

---

## Overlay Network Architecture

### SHIP Pattern

The overlay follows the SHIP (Submit, Host, Index, Prove) architecture:

- **Submit**: Wallets submit registration transactions to BRC-22 Topic Managers
- **Host**: BRC-24 Lookup Services host indexed alias data
- **Index**: Lookup services maintain queryable indexes by alias
- **Prove**: On-chain transactions provide cryptographic proof of ownership

### Node Synchronization

Overlay nodes synchronize using BRC-88 protocols. New nodes can:
- **Peer sync**: Fast synchronization from existing nodes
- **Chain scan**: Trustless validation by scanning blockchain

Best practice combines both: peer sync for speed, optional chain scan for verification.

### Node Discovery

BRC-23 (CHIP) enables node discovery through on-chain advertisements. Nodes advertise their services via CHIP tokens, allowing wallets to discover available Topic Managers and Lookup Services dynamically.

---

## Micropayment Economics

### Operation Costs

| Operation | Cost (sats) | Who Pays |
|-----------|------------|----------|
| Register alias | 1000 | User (on-chain) |
| Lookup | 1 | Requesting wallet |
| Payment destination | 5 | Sender wallet |
| Relay message | 10 | Sender wallet |

### Payment Validation

Wallets include micropayments in request headers using BRC-41 HTTP Service Monetization patterns. Overlay nodes validate payments before processing requests, ensuring sustainable operation through micropayment funding.

### Economic Sustainability

Operator costs: $5-30/month per node (VPS + bandwidth). Revenue scales with usage volume. Break-even point approximately 30,000 active users per node. Micropayments enable sustainable operation without subscription fees.

---

## Security Model

### Identity Verification

Registration transactions are signed by registrant's keys using BRC-42/43 derivation. Key updates require signatures from current keys. On-chain records provide tamper-proof proof of ownership.

**Certificates Optional**: BRC-52 certificates can enhance identity verification but are not required for basic paymail functionality. The system prioritizes simplicity while allowing optional certificate integration.

### Squatting Prevention

Topic Manager validation enforces first-come-first-served registration. Registration fees (1000 sats) discourage mass squatting. On-chain proof prevents disputes. Alias format restrictions limit abuse.

### Spam Prevention

Micropayments required for every operation. Rate limiting per IP address. Higher fees for suspicious patterns. Double-spend protection through immediate transaction broadcasting and TXID tracking.

---

## BRC Standards Integration

### Core Standards

| BRC | Purpose | Usage |
|-----|---------|-------|
| [BRC-22](https://bsv.brc.dev/overlays/0022) | Topic Manager | Validates and admits registration transactions |
| [BRC-23](https://bsv.brc.dev/overlays/0023) | CHIP | Node discovery and advertisement |
| [BRC-24](https://bsv.brc.dev/overlays/0024) | Lookup Service | Indexes aliases, responds to queries |
| [BRC-25](https://bsv.brc.dev/overlays/0025) | CLAP | Lookup service availability |
| [BRC-33](https://bsv.brc.dev/peer-to-peer/0033) | PeerServ | Message relay for offline delivery |
| [BRC-42/43](https://bsv.brc.dev/key-derivation/0042) | Key Derivation | Secure key management |
| [BRC-58](https://bsv.brc.dev/transactions/0058) | BEEF | Transaction format for SPV |
| [BRC-70](https://bsv.brc.dev/payments/0070) | Paymail BEEF | BEEF transactions via paymail |
| [BRC-100](https://bsv.brc.dev/wallet/0100) | Wallet Interface | Standard wallet API |

### Supporting Standards

| BRC | Purpose | Usage |
|-----|---------|-------|
| [BRC-28](https://bsv.brc.dev/payments/0028) | Paymail Destinations | Payment destination protocol |
| [BRC-29](https://bsv.brc.dev/payments/0029) | Simple P2PKH | Payment script format |
| [BRC-41](https://bsv.brc.dev/payments/0041) | HTTP Monetization | Micropayment validation |
| [BRC-83](https://bsv.brc.dev/transactions/0083) | Scalable TX Processing | High-volume optimization |
| [BRC-87](https://bsv.brc.dev/overlays/0087) | Naming Conventions | Service naming standards |
| [BRC-88](https://bsv.brc.dev/overlays/0088) | Overlay Sync | Node synchronization |

### Optional Standards

| BRC | Purpose | When to Use |
|-----|---------|-------------|
| [BRC-26](https://bsv.brc.dev/overlays/0026) | UHRP | Content hosting for profiles |
| [BRC-52](https://bsv.brc.dev/peer-to-peer/0052) | Certificates | Enhanced identity verification |
| [BRC-84](https://bsv.brc.dev/key-derivation/0084) | Linked Keys | Enhanced privacy |

**Note on BRC-31**: BRC-31 (Authrite) may be deprecated in favor of BRC-103/104 for authentication. Topic Managers use BRC-31 for transaction submission authentication, but future implementations should consider BRC-103/104 alternatives.

---

## Comparison: Traditional vs Vendor-Neutral Paymail

| Aspect | Traditional Paymail | Vendor-Neutral Paymail |
|--------|--------------------|--------------------|
| Identity format | `alice@domain.com` | `alice@paymail` |
| Resolution | DNS lookup | Overlay lookup |
| Registry | Domain registrar | BitcoinSV blockchain |
| Dependency | DNS, ICANN, registrars | Blockchain only |
| Censorship | Domain seizure possible | Very difficult |
| Cost (operator) | $119-179/year | $60/year |
| Cost (user) | Vendor-dependent | ~$0.01/year |
| Offline support | Always online server | BRC-33 relay fallback |
| BSV-native | No | Yes |

---

## Design Rationale

### Why Overlay Instead of DNS

DNS requires domain registration, creating centralization points and censorship risks. Overlay networks use blockchain as the source of truth, eliminating registrar dependencies. Protocol identifiers (`@paymail`) signal resolution method without requiring domain ownership.

### Why Blockchain Registry

BitcoinSV blockchain provides a permanent, censorship-resistant registry. Once a registration is mined, it cannot be altered or removed by any central authority. The blockchain's ordering resolves race conditions and provides cryptographic proof of ownership.

### Why Micropayments

BSV's low transaction fees make micropayment-funded services viable. Payment-as-authentication eliminates API keys and simplifies wallet integration. Micropayments scale with usage, creating sustainable economic models.

### Why BRC Standards

Leveraging existing BRC standards ensures interoperability and reduces implementation complexity. Standards provide proven patterns for overlay networks, message relay, and key management. Vendor-neutral standards prevent lock-in.

---

## References

### BSV Standards (BRC)
- [BRC-22: Overlay Network Data Synchronization](https://bsv.brc.dev/overlays/0022)
- [BRC-23: Confederacy Host Interconnect Protocol (CHIP)](https://bsv.brc.dev/overlays/0023)
- [BRC-24: Overlay Network Lookup Services](https://bsv.brc.dev/overlays/0024)
- [BRC-25: Confederacy Lookup Availability Protocol (CLAP)](https://bsv.brc.dev/overlays/0025)
- [BRC-26: Universal Hash Resolution Protocol](https://bsv.brc.dev/overlays/0026)
- [BRC-28: Paymail Payment Destinations](https://bsv.brc.dev/payments/0028)
- [BRC-29: Simple Authenticated BSV P2PKH Payment Protocol](https://bsv.brc.dev/payments/0029)
- [BRC-33: PeerServ Message Relay Interface](https://bsv.brc.dev/peer-to-peer/0033)
- [BRC-41: HTTP Service Monetization Framework](https://bsv.brc.dev/payments/0041)
- [BRC-42: BSV Key Derivation Scheme](https://bsv.brc.dev/key-derivation/0042)
- [BRC-43: Security Levels, Protocol IDs, Key IDs and Counterparties](https://bsv.brc.dev/key-derivation/0043)
- [BRC-58: Background Evaluation Extended Format (BEEF) Transactions](https://bsv.brc.dev/transactions/0058)
- [BRC-70: Paymail BEEF Transaction](https://bsv.brc.dev/payments/0070)
- [BRC-83: Scalable Transaction Processing in the BSV Network](https://bsv.brc.dev/transactions/0083)
- [BRC-87: Standardized Naming Conventions for BRC-22 Topic Managers and BRC-24 Lookup Services](https://bsv.brc.dev/overlays/0087)
- [BRC-88: Overlay Services Synchronization Architecture](https://bsv.brc.dev/overlays/0088)
- [BRC-100: Unified Abstract Wallet-to-Application Messaging Layer](https://bsv.brc.dev/wallet/0100)

### General References
- [SPV Wallet (BSV Association)](https://github.com/bitcoin-sv/spv-wallet)
- [BSV Overlay Networks](https://docs.bsvblockchain.org/network-topology/overlay-services)

---

**Document Status**: Architecture Proposal for Peer Review
**Last Updated**: January 2026
